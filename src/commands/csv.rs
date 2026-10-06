//! `csv`: write a derived CSV copy of each sheet.
//!
//! Each workbook is planned completely — every selected sheet read and rendered, every output path
//! named and probed — before anything is written.  If any output would be refused, none of that
//! workbook's files is written.  A write that itself fails afterwards (a full disk, a file that
//! appeared meanwhile) leaves the outputs already written in place, reported and exit 1.  A batch
//! carries on past a failed workbook and exits 1 at the end.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use cli_contract::{DestPolicy, check_destination, ensure_destination, ignore_broken_pipe};

use crate::cli::CsvArgs;
use crate::commands::{hidden_note, is_explicit, select};
use crate::naming::{self, NameParts};
use crate::output::{self, FileId, Plan, Policy};
use crate::render::csv_bytes;
use crate::types::{CsvFileReport, CsvOutput, CsvReport, FileStatus, OutputStatus, SheetKind};
use crate::workbook::Workbook;

/// State shared by every workbook of one run.
struct Run<'a> {
    args: &'a CsvArgs,
    /// The output directory, made absolute (cli-contract bug-0004 mis-resolves a relative
    /// single-component path); `None` for beside-each-workbook.
    output_dir: Option<PathBuf>,
    /// False in a dry run whose `--output-dir` would be created: nothing can be in it yet.
    dir_exists: bool,
    inputs: HashSet<FileId>,
    /// Output names claimed so far, keyed on the canonical directory and the case-folded name.
    claimed: HashSet<String>,
    /// Files this run has written, by identity.
    written: HashSet<FileId>,
}

pub fn run(args: &CsvArgs) -> Result<ExitCode> {
    let mut dir_exists = true;
    let output_dir = match &args.output_dir {
        Some(dir) => {
            let dir = std::path::absolute(dir)?;
            let policy = DestPolicy::Create(args.create_destination);
            if args.dry_run {
                check_destination(&dir, &policy)?;
                dir_exists = dir.is_dir();
            } else {
                ensure_destination(&dir, &policy)?;
            }
            Some(dir)
        }
        None => None,
    };
    // An input that cannot be inspected fails when it is opened; one that can is protected from
    // ever being replaced by an output of this run.
    let inputs = args
        .files
        .iter()
        .filter_map(|f| FileId::of(f).ok().flatten())
        .collect();
    let mut run = Run {
        args,
        output_dir,
        dir_exists,
        inputs,
        claimed: HashSet::new(),
        written: HashSet::new(),
    };
    let report = CsvReport {
        dry_run: args.dry_run,
        files: args.files.iter().map(|f| convert(f, &mut run)).collect(),
    };

    // Errors first, so a closed stdout cannot swallow them; then the report, whose writes
    // tolerate a closed pipe — the files are written, and the exit code still carries failure.
    for file in &report.files {
        if let Some(e) = &file.error {
            eprintln!("error: {e}");
        }
    }
    let text = if args.json {
        serde_json::to_string_pretty(&report)? + "\n"
    } else {
        report_text(&report)
    };
    ignore_broken_pipe(std::io::stdout().lock().write_all(text.as_bytes()))?;
    Ok(
        if report.files.iter().any(|f| f.status == FileStatus::Failed) {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        },
    )
}

fn report_text(report: &CsvReport) -> String {
    let mut out = String::new();
    for file in &report.files {
        for o in &file.outputs {
            let path = o.path.display();
            let reason = o.reason.as_deref().unwrap_or_default();
            out.push_str(&match o.status {
                OutputStatus::Written => format!("wrote        {path}\n"),
                OutputStatus::WouldWrite => format!("would write  {path}\n"),
                OutputStatus::Skipped => format!("skipped      {path} (exists)\n"),
                OutputStatus::Withheld => format!("withheld     {path}\n"),
                OutputStatus::Refused => format!("refused      {path}: {reason}\n"),
                OutputStatus::NoCells => format!("no cells     {path}: {reason}\n"),
                OutputStatus::Unreadable => format!("unreadable   {path}: {reason}\n"),
            });
        }
    }
    out
}

/// One planned output, with the bytes to write when it is to be written.
struct Pending {
    output: CsvOutput,
    bytes: Option<Vec<u8>>,
}

fn convert(file: &Path, run: &mut Run<'_>) -> CsvFileReport {
    let args = run.args;
    let mut report = CsvFileReport {
        file: file.to_path_buf(),
        status: FileStatus::Ok,
        error: None,
        outputs: Vec::new(),
    };
    let fail = |mut report: CsvFileReport, e: anyhow::Error| {
        report.status = FileStatus::Failed;
        report.error = Some(format!("{e:#}"));
        report
    };
    let mut wb = match Workbook::open(file) {
        Ok(wb) => wb,
        Err(e) => return fail(report, e),
    };
    let sheets = match select(&wb, &args.select) {
        Ok(s) => s,
        Err(e) => return fail(report, e),
    };

    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let from_ext = naming::from_ext(file).unwrap_or_else(|| {
        naming::from_ext(Path::new(&format!("x.{}", wb.format))).unwrap_or_default()
    });
    let template = args.name.as_deref().unwrap_or(if wb.sheets.len() == 1 {
        naming::DEFAULT_SINGLE
    } else {
        naming::DEFAULT_MULTI
    });
    let dir: PathBuf = match &run.output_dir {
        Some(d) => d.clone(),
        None => match file.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        },
    };

    let mut pending: Vec<Pending> = Vec::new();
    let mut error: Option<anyhow::Error> = None;
    for sheet in &sheets {
        let path = dir.join(naming::render(
            template,
            &NameParts {
                stem: &stem,
                from_ext: &from_ext,
                sheet: &sheet.name,
                index: sheet.index,
            },
        ));
        let mut output = CsvOutput {
            sheet: sheet.name.clone(),
            sheet_index: sheet.index,
            visibility: sheet.visibility,
            path: path.clone(),
            status: OutputStatus::Withheld,
            reason: None,
            rows: 0,
            cols: 0,
            uncached_formulas: 0,
        };
        if sheet.kind != SheetKind::Worksheet {
            let why = format!("a {}: its content is not cells", sheet.kind.as_str());
            if is_explicit(&args.select) {
                error.get_or_insert_with(|| {
                    anyhow::anyhow!(
                        "{}: sheet {:?} is {why}; there is nothing to write as CSV",
                        file.display(),
                        sheet.name
                    )
                });
            }
            output.status = OutputStatus::NoCells;
            output.reason = Some(why);
            pending.push(Pending {
                output,
                bytes: None,
            });
            continue;
        }
        if let Some(note) = hidden_note(&wb, sheet) {
            eprintln!("{note}");
        }
        let rendered = wb.read(sheet).and_then(|content| {
            let bytes = csv_bytes(&content, !args.no_bom).with_context(|| {
                format!("cannot write sheet {:?} of {}", sheet.name, file.display())
            })?;
            Ok((bytes, content))
        });
        let (bytes, content) = match rendered {
            Ok(r) => r,
            Err(e) => {
                // Recorded and carried on, so the report still lists every selected sheet.
                output.status = OutputStatus::Unreadable;
                output.reason = Some(format!("{e:#}"));
                error.get_or_insert(e);
                pending.push(Pending {
                    output,
                    bytes: None,
                });
                continue;
            }
        };
        (output.rows, output.cols) = content.extent();
        output.uncached_formulas = content.uncached_count();

        // The directory canonical (so `d/` and `./d/` and a symlinked `d` agree) and the name
        // case-folded (the default macOS volume is case-insensitive).  Names that differ only in
        // Unicode normalization still slip past this key; write_atomic's identity check is the
        // backstop for those.
        let key = format!(
            "{}/{}",
            dir.canonicalize().unwrap_or_else(|_| dir.clone()).display(),
            path.file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        );
        let plan = if !run.claimed.insert(key) {
            Plan::Refuse(format!(
                "another output of this run has the same file name (sheet names that sanitize \
                 alike, or a --name without {{sheet}} or {{index}}): {}",
                path.display()
            ))
        } else {
            output::plan(
                &path,
                &Policy {
                    overwrite: args.overwrite,
                    skip_existing: args.skip_existing,
                    dir_exists: run.dir_exists,
                    inputs: &run.inputs,
                },
            )
        };
        let bytes = match plan {
            Plan::Write => Some(bytes),
            Plan::Skip => {
                output.status = OutputStatus::Skipped;
                None
            }
            Plan::Refuse(why) => {
                output.status = OutputStatus::Refused;
                output.reason = Some(why);
                None
            }
        };
        pending.push(Pending { output, bytes });
    }

    let refused = pending
        .iter()
        .any(|p| p.output.status == OutputStatus::Refused);
    if let Some(e) = error {
        report = fail(report, e);
    } else if refused {
        report.status = FileStatus::Failed;
        report.error = Some(format!(
            "{}: an output was refused, so none of this workbook's files was written",
            file.display()
        ));
    }

    for mut p in pending {
        if let Some(bytes) = p.bytes.take() {
            if report.status == FileStatus::Failed {
                // Withheld: another output of this workbook was refused.
            } else if args.dry_run {
                p.output.status = OutputStatus::WouldWrite;
            } else {
                match output::write_atomic(&p.output.path, &bytes, args.overwrite, &mut run.written)
                {
                    Ok(()) => p.output.status = OutputStatus::Written,
                    Err(e) => {
                        p.output.status = OutputStatus::Refused;
                        p.output.reason = Some(format!("{e:#}"));
                        report.status = FileStatus::Failed;
                        report.error.get_or_insert_with(|| {
                            format!("{}: an output could not be written", file.display())
                        });
                    }
                }
            }
        }
        if p.output.uncached_formulas > 0
            && matches!(
                p.output.status,
                OutputStatus::Written | OutputStatus::WouldWrite
            )
        {
            eprintln!(
                "warning: {}: sheet {:?} has {} formula(s) with no cached value, written as empty \
                 cells",
                file.display(),
                p.output.sheet,
                p.output.uncached_formulas
            );
        }
        report.outputs.push(p.output);
    }
    report
}
