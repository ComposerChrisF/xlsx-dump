//! Cell values as CSV text and as typed JSON, and a sheet as an A1-anchored CSV grid.

use anyhow::{Result, bail};
use serde_json::Value as Json;

use crate::workbook::{SheetContent, Value, cell_ref};

/// The UTF-8 byte-order mark (`csv-utf8-bom.md`).
pub const BOM: &[u8] = b"\xEF\xBB\xBF";

/// The largest grid `csv` will write, in cells.  A sheet with one stray value far from its data
/// would otherwise pad out to gigabytes of commas; `cells` reads such a sheet sparsely.
pub const MAX_GRID_CELLS: usize = 50_000_000;

/// A number at Excel's own precision, 15 significant digits, written in its shortest form
/// (`0.3`, never `0.30000000000000004`), with no exponent, separators, or currency, and no
/// negative zero.
///
/// Shortest round-trip alone is not enough: a formula's cached result carries binary noise
/// (`=0.1+0.2` caches the double `0.30000000000000004`), and the shortest text for that double
/// is still the noisy one.  Excel treats 15 digits as its precision and shows that value as
/// `0.3`; rounding there first loses nothing the authoring application kept.
pub fn excel_precision(f: f64) -> f64 {
    format!("{f:.14e}").parse().unwrap_or(f)
}

pub fn number_text(f: f64) -> String {
    let f = excel_precision(f);
    if f == 0.0 {
        "0".to_string()
    } else {
        f.to_string()
    }
}

/// A value as one CSV field.
pub fn csv_text(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => number_text(*f),
        Value::Str(s) => s.clone(),
        Value::Bool(true) => "TRUE".into(),
        Value::Bool(false) => "FALSE".into(),
        Value::Error(e) => e.clone(),
        Value::Date { iso, .. } | Value::Duration { iso, .. } => iso.clone(),
    }
}

/// A value's JSON `type`, `value`, and `serial`.
pub fn json_parts(v: &Value) -> (&'static str, Json, Option<f64>) {
    match v {
        Value::Empty => ("none", Json::Null, None),
        Value::Int(i) => ("number", Json::from(*i), None),
        Value::Float(f) => (
            "number",
            serde_json::Number::from_f64(excel_precision(*f)).map_or(Json::Null, Json::Number),
            None,
        ),
        Value::Str(s) => ("string", Json::from(s.as_str()), None),
        Value::Bool(b) => ("bool", Json::from(*b), None),
        Value::Error(e) => ("error", Json::from(e.as_str()), None),
        Value::Date { iso, serial } => ("date", Json::from(iso.as_str()), *serial),
        Value::Duration { iso, serial } => ("duration", Json::from(iso.as_str()), *serial),
    }
}

/// A sheet as CSV bytes, anchored at A1: column N of the CSV is column N of the sheet, and every
/// row has the same number of fields.  An empty sheet is an empty file (the BOM alone, if asked).
pub fn csv_bytes(content: &SheetContent, bom: bool) -> Result<Vec<u8>> {
    let (rows, cols) = content.extent();
    if rows.saturating_mul(cols) > MAX_GRID_CELLS {
        bail!(
            "the used range ends at {}, a grid of {rows} × {cols} cells, more than the {} a CSV \
             will hold; `xlsx-dump cells` reads such a sheet sparsely",
            cell_ref(rows as u32 - 1, cols as u32 - 1),
            MAX_GRID_CELLS
        );
    }
    let mut out = Vec::new();
    if bom {
        out.extend_from_slice(BOM);
    }
    let mut writer = csv::WriterBuilder::new().from_writer(out);
    let mut cells = content.cells.iter().peekable();
    let mut record: Vec<String> = vec![String::new(); cols];
    for row in 0..rows as u32 {
        record.iter_mut().for_each(String::clear);
        while let Some(cell) = cells.next_if(|c| c.row == row) {
            record[cell.col as usize] = csv_text(&cell.value);
        }
        writer.write_record(&record)?;
    }
    Ok(writer.into_inner().map_err(|e| e.into_error())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbook::{Cell, Formula};

    fn cell(row: u32, col: u32, value: Value) -> Cell {
        Cell {
            row,
            col,
            value,
            formula: Formula::None,
        }
    }

    #[test]
    fn numbers_are_shortest_round_trip() {
        assert_eq!(number_text(1234.56), "1234.56");
        assert_eq!(number_text(-42.5), "-42.5");
        assert_eq!(number_text(0.875), "0.875");
        assert_eq!(number_text(5.0), "5");
        assert_eq!(number_text(-0.0), "0");
        assert_eq!(number_text(1e21), "1000000000000000000000");
        assert_eq!(number_text(1.1 * 3.0), "3.3");
        assert_eq!(number_text(0.1 + 0.2), "0.3");
        assert_eq!(number_text(123_456_789_012_345.0), "123456789012345");
        assert_eq!(number_text(1.0 / 3.0), "0.333333333333333");
    }

    #[test]
    fn grid_is_anchored_at_a1_with_bom() {
        let content = SheetContent {
            cells: vec![
                cell(2, 2, Value::Str("x, y".into())),
                cell(3, 3, Value::Float(1.5)),
            ],
            merged: None,
        };
        let bytes = csv_bytes(&content, true).unwrap();
        assert!(bytes.starts_with(BOM));
        let text = std::str::from_utf8(&bytes[3..]).unwrap();
        assert_eq!(text, ",,,\n,,,\n,,\"x, y\",\n,,,1.5\n");
    }

    #[test]
    fn empty_sheet_is_an_empty_file() {
        let content = SheetContent::default();
        assert_eq!(csv_bytes(&content, true).unwrap(), BOM);
        assert!(csv_bytes(&content, false).unwrap().is_empty());
    }
}
