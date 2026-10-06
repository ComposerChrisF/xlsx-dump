//! `sheets`: list a workbook's sheets.

use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;

use crate::commands::print_json;
use crate::types::{SheetKind, SheetReport, SheetsReport};
use crate::workbook::Workbook;

pub fn run(file: &Path, json: bool) -> Result<ExitCode> {
    let mut wb = Workbook::open(file)?;
    let mut failed = false;
    let mut reports = Vec::new();
    for sheet in wb.sheets.clone() {
        let mut report = SheetReport {
            index: sheet.index,
            name: sheet.name.clone(),
            visibility: sheet.visibility,
            kind: sheet.kind,
            range: None,
            rows: 0,
            cols: 0,
            formulas: 0,
            uncached_formulas: 0,
            unrecognised_formulas: 0,
            merged_regions: None,
            readable: false,
            note: None,
        };
        if sheet.kind != SheetKind::Worksheet {
            report.note = Some(format!(
                "a {}: its content is not cells and is not dumped",
                sheet.kind.as_str()
            ));
        } else {
            match wb.read(&sheet) {
                Ok(content) => {
                    let (rows, cols) = content.extent();
                    report.range = content.used_range();
                    report.rows = rows;
                    report.cols = cols;
                    report.formulas = content.formula_count();
                    report.uncached_formulas = content.uncached_count();
                    report.unrecognised_formulas = content.unrecognised_count();
                    report.merged_regions = content.merged.as_ref().map(Vec::len);
                    report.readable = true;
                    if content.cells.is_empty() {
                        report.note = Some(
                            "empty: no cells (charts or images on a worksheet are not inspected)"
                                .into(),
                        );
                    }
                }
                Err(e) => {
                    failed = true;
                    eprintln!("error: {e:#}");
                    report.note = Some(format!("{e:#}"));
                }
            }
        }
        reports.push(report);
    }

    if json {
        print_json(&SheetsReport {
            file: file.to_path_buf(),
            format: wb.format,
            sheets: reports,
        })?;
    } else {
        print_table(&reports)?;
    }
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn print_table(reports: &[SheetReport]) -> Result<()> {
    use std::io::Write;
    let header = [
        "#",
        "sheet",
        "visibility",
        "kind",
        "range",
        "formulas",
        "uncached",
        "merged",
        "note",
    ];
    let rows: Vec<[String; 9]> = reports
        .iter()
        .map(|r| {
            [
                r.index.to_string(),
                r.name.clone(),
                r.visibility.as_str().into(),
                r.kind.as_str().into(),
                r.range.clone().unwrap_or_else(|| "-".into()),
                r.formulas.to_string(),
                r.uncached_formulas.to_string(),
                r.merged_regions
                    .map_or_else(|| "unknown".into(), |n| n.to_string()),
                r.note.clone().unwrap_or_default(),
            ]
        })
        .collect();
    let mut widths = header.map(|h| h.chars().count());
    for row in &rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let mut out = std::io::stdout().lock();
    let line = |cells: &[String]| -> String {
        let mut s = String::new();
        for (i, (cell, w)) in cells.iter().zip(widths).enumerate() {
            if i + 1 == cells.len() {
                s.push_str(cell);
            } else {
                s.push_str(cell);
                s.push_str(&" ".repeat(w - cell.chars().count() + 2));
            }
        }
        s.trim_end().to_string()
    };
    writeln!(out, "{}", line(&header.map(String::from)))?;
    for row in &rows {
        writeln!(out, "{}", line(row))?;
    }
    Ok(())
}
