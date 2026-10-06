//! `check`: exit 3 when a workbook holds something a reader must know about.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;

use crate::commands::print_json;
use crate::render::MAX_GRID_CELLS;
use crate::types::{
    CheckFileReport, CheckReport, CheckStatus, Finding, FindingKind, SheetKind, Visibility,
};
use crate::workbook::Workbook;

pub fn run(files: &[PathBuf], json: bool) -> Result<ExitCode> {
    let report = CheckReport {
        files: files.iter().map(|f| check_one(f)).collect(),
    };
    let reports = &report.files;

    if json {
        print_json(&report)?;
    } else {
        for r in reports {
            match r.status {
                CheckStatus::Clean => println!("clean     {}", r.file.display()),
                CheckStatus::Findings => {
                    println!("findings  {}", r.file.display());
                    for f in &r.findings {
                        println!("  sheet {} {:?}: {}", f.sheet_index, f.sheet, f.detail);
                    }
                }
                CheckStatus::Failed => println!("failed    {}", r.file.display()),
            }
        }
    }
    for r in reports {
        if let Some(e) = &r.error {
            eprintln!("error: {e}");
        }
    }
    // A failed read outranks findings: the tool did not run over everything it was given.
    Ok(if reports.iter().any(|r| r.status == CheckStatus::Failed) {
        ExitCode::from(1)
    } else if reports.iter().any(|r| r.status == CheckStatus::Findings) {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    })
}

fn check_one(file: &Path) -> CheckFileReport {
    let mut report = CheckFileReport {
        file: file.to_path_buf(),
        status: CheckStatus::Clean,
        error: None,
        findings: Vec::new(),
    };
    let mut wb = match Workbook::open(file) {
        Ok(wb) => wb,
        Err(e) => {
            report.status = CheckStatus::Failed;
            report.error = Some(format!("{e:#}"));
            return report;
        }
    };
    let mut findings = Vec::new();
    let mut unreadable = Vec::new();
    for sheet in wb.sheets.clone() {
        let mut add = |kind, count, detail: String| {
            findings.push(Finding {
                kind,
                sheet: sheet.name.clone(),
                sheet_index: sheet.index,
                count,
                detail,
            })
        };
        match sheet.visibility {
            Visibility::Hidden => {
                add(FindingKind::HiddenSheet, 1, "hidden sheet".into());
            }
            Visibility::VeryHidden => add(
                FindingKind::VeryHiddenSheet,
                1,
                "very-hidden sheet (not unhideable from the application's menus)".into(),
            ),
            Visibility::Visible => {}
        }
        if sheet.kind != SheetKind::Worksheet {
            add(
                FindingKind::NoCells,
                1,
                format!(
                    "a {}: its content is not cells and is not dumped",
                    sheet.kind.as_str()
                ),
            );
            continue;
        }
        match wb.read(&sheet) {
            Ok(content) => {
                let uncached = content.uncached_count();
                if uncached > 0 {
                    add(
                        FindingKind::UncachedFormulas,
                        uncached,
                        format!("{uncached} formula(s) with no cached value"),
                    );
                }
                let unrecognised = content.unrecognised_count();
                if unrecognised > 0 {
                    add(
                        FindingKind::UnrecognisedFormulas,
                        unrecognised,
                        format!(
                            "{unrecognised} formula(s) whose text the reader could not recover"
                        ),
                    );
                }
                let (rows, cols) = content.extent();
                if rows.saturating_mul(cols) > MAX_GRID_CELLS {
                    add(
                        FindingKind::OversizedGrid,
                        1,
                        format!(
                            "a {rows} × {cols} grid, too large for `csv` (more than {MAX_GRID_CELLS} \
                             cells); `cells` reads it"
                        ),
                    );
                }
            }
            Err(e) => {
                // Listed with the findings so the record is complete, but a sheet that could not be
                // read makes the file a failure (exit 1), never findings: the tool did not run
                // over all of the data (positive-evidence-of-absence.md).
                add(FindingKind::UnreadableSheet, 1, format!("{e:#}"));
                unreadable.push(format!("{e:#}"));
            }
        }
    }
    if !unreadable.is_empty() {
        report.status = CheckStatus::Failed;
        report.error = Some(unreadable.join("; "));
    } else if !findings.is_empty() {
        report.status = CheckStatus::Findings;
    }
    report.findings = findings;
    report
}
