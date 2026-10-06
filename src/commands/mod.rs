//! The subcommands.

pub mod cells;
pub mod check;
pub mod csv;
pub mod sheets;

use std::io::Write;

use anyhow::{Result, bail};

use crate::cli::SheetSelect;
use crate::types::Visibility;
use crate::workbook::{SheetMeta, Workbook};

/// Resolve `--sheet`, `--sheet-index`, and `--visible-only` to sheets in workbook order, each once.
/// A named or numbered sheet the workbook lacks is an error naming what it does have.
pub fn select(wb: &Workbook, sel: &SheetSelect) -> Result<Vec<SheetMeta>> {
    let mut chosen: Vec<usize> = Vec::new();
    for name in &sel.names {
        match wb.sheet_named(name) {
            Some(s) => chosen.push(s.index),
            None => bail!(
                "{} has no sheet named {name:?}; its sheets are {}",
                wb.path.display(),
                wb.sheet_list()
            ),
        }
    }
    for &n in &sel.indexes {
        let n = n as usize;
        if n > wb.sheets.len() {
            bail!(
                "{} has {} sheet(s), so there is no sheet {n}",
                wb.path.display(),
                wb.sheets.len()
            );
        }
        chosen.push(n);
    }
    let explicit = !chosen.is_empty();
    let mut sheets: Vec<SheetMeta> = wb
        .sheets
        .iter()
        .filter(|s| !explicit || chosen.contains(&s.index))
        .cloned()
        .collect();
    if sel.visible_only {
        // A sheet the caller named is a claim; quietly filtering it away would answer with a
        // plausible empty result.
        if let Some(hidden) = sheets
            .iter()
            .find(|s| explicit && s.visibility != Visibility::Visible)
        {
            bail!(
                "{}: sheet {:?} was asked for, but it is {} and --visible-only excludes it",
                wb.path.display(),
                hidden.name,
                hidden.visibility.as_str()
            );
        }
        sheets.retain(|s| s.visibility == Visibility::Visible);
    }
    Ok(sheets)
}

/// Whether the sheets were picked by name or number (so a non-worksheet among them is an error,
/// not a note).
pub fn is_explicit(sel: &SheetSelect) -> bool {
    !sel.names.is_empty() || !sel.indexes.is_empty()
}

/// Print one JSON document on stdout.
pub fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}

/// The stderr note for a hidden sheet that is being read like any other.
pub fn hidden_note(wb: &Workbook, sheet: &SheetMeta) -> Option<String> {
    (sheet.visibility != Visibility::Visible).then(|| {
        format!(
            "note: {}: sheet {:?} is {} and is included (use --visible-only to skip it)",
            wb.path.display(),
            sheet.name,
            sheet.visibility.as_str()
        )
    })
}
