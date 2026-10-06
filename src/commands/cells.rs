//! `cells`: every cell as a JSON Lines record.

use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, bail};

use crate::cli::SheetSelect;
use crate::commands::{hidden_note, is_explicit, select};
use crate::render::json_parts;
use crate::types::{CellRecord, SheetKind};
use crate::workbook::{Formula, Workbook, cell_ref};

pub fn run(file: &Path, sel: &SheetSelect, formulas_only: bool) -> Result<ExitCode> {
    let mut wb = Workbook::open(file)?;
    let sheets = select(&wb, sel)?;
    let explicit = is_explicit(sel);
    let mut out = BufWriter::new(std::io::stdout().lock());
    let mut failed = false;
    for sheet in sheets {
        if sheet.kind != SheetKind::Worksheet {
            let msg = format!(
                "{}: sheet {:?} is a {}, which holds no cells",
                file.display(),
                sheet.name,
                sheet.kind.as_str()
            );
            if explicit {
                bail!(msg);
            }
            eprintln!("note: {msg}; skipped");
            continue;
        }
        if let Some(note) = hidden_note(&wb, &sheet) {
            eprintln!("{note}");
        }
        let content = match wb.read(&sheet) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e:#}");
                failed = true;
                continue;
            }
        };
        for cell in &content.cells {
            if formulas_only && cell.formula == Formula::None {
                continue;
            }
            let (kind, value, serial) = json_parts(&cell.value);
            let (formula, cached) = match &cell.formula {
                Formula::None => (None, None),
                Formula::Text(f) => (Some(f.clone()), Some(kind != "none")),
                Formula::Unrecognised => (None, Some(kind != "none")),
            };
            let record = CellRecord {
                sheet: &sheet.name,
                sheet_index: sheet.index,
                visibility: sheet.visibility,
                cell_ref: cell_ref(cell.row, cell.col),
                row: cell.row + 1,
                col: cell.col + 1,
                kind,
                value,
                serial,
                formula,
                formula_unrecognised: (cell.formula == Formula::Unrecognised).then_some(true),
                cached,
                merged: content.merged_at(cell.row, cell.col),
            };
            serde_json::to_writer(&mut out, &record)?;
            writeln!(out)?;
        }
        let uncached = content.uncached_count();
        if uncached > 0 {
            eprintln!(
                "warning: {}: sheet {:?} has {uncached} formula(s) with no cached value (type \
                 \"none\")",
                file.display(),
                sheet.name
            );
        }
    }
    out.flush()?;
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
