//! Opening a workbook and reading its sheets into a format-independent, sparse cell list.
//!
//! Every read failure here is loud (`DESIGN.md` § 6): an encrypted, corrupt, unsupported, or
//! sheetless workbook is an error naming the file, never an empty result.  The format is chosen by
//! sniffing the container, not by trusting `open_workbook_auto`, which discards the
//! "password protected" error when the extension is unexpected.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufReader, Read};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use calamine::{
    Data, DataRef, Dimensions, ExcelDateTime, Ods, OdsError, Range, Reader, SheetType,
    SheetVisible, Xls, XlsError, Xlsb, XlsbError, Xlsx, XlsxError, XlsxFormulaMetadata,
    expand_shared_formula, open_workbook,
};

use crate::dates;
use crate::types::{SheetKind, Visibility};

type Source = BufReader<File>;

const OLE_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const ZIP_MAGIC: [u8; 4] = *b"PK\x03\x04";

/// The prefix calamine puts in the formula grid for an `.xls` formula it cannot decode.
const XLS_UNRECOGNISED: &str = "Unrecognised formula for cell";

enum Inner {
    Xlsx(Xlsx<Source>),
    Xlsb(Xlsb<Source>),
    Xls(Xls<Source>),
    Ods(Ods<Source>),
}

/// One sheet as the workbook declares it.
#[derive(Debug, Clone)]
pub struct SheetMeta {
    /// 1-based position in the workbook.
    pub index: usize,
    pub name: String,
    pub visibility: Visibility,
    pub kind: SheetKind,
}

/// A cell's value, typed.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// No value: an empty cell, or a formula with no cached value.
    Empty,
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    /// An error value, as its text (`#DIV/0!`).
    Error(String),
    /// A date or date-time, as ISO 8601 text, with its serial when the format stores one.
    Date {
        iso: String,
        serial: Option<f64>,
    },
    /// An elapsed time, as an ISO 8601 duration, with its serial when the format stores one.
    Duration {
        iso: String,
        serial: Option<f64>,
    },
}

/// A cell's formula.
#[derive(Debug, Clone, PartialEq)]
pub enum Formula {
    None,
    /// The formula text, with a leading `=`.
    Text(String),
    /// The cell holds a formula whose text the reader could not recover (an `.xls` formula it
    /// cannot decode, or an `.xlsx` shared formula whose anchor is missing).
    Unrecognised,
}

#[derive(Debug, Clone)]
pub struct Cell {
    /// 0-based position.
    pub row: u32,
    pub col: u32,
    pub value: Value,
    pub formula: Formula,
}

/// One worksheet's content: its non-empty cells in row-major order, and its merged regions.
#[derive(Debug, Default)]
pub struct SheetContent {
    pub cells: Vec<Cell>,
    /// `None` when the format does not report merged regions (`.xlsb`, `.ods`).
    pub merged: Option<Vec<Dimensions>>,
}

impl SheetContent {
    /// The grid's size anchored at A1: (last used row + 1, last used column + 1).
    pub fn extent(&self) -> (usize, usize) {
        let rows = self
            .cells
            .iter()
            .map(|c| c.row as usize + 1)
            .max()
            .unwrap_or(0);
        let cols = self
            .cells
            .iter()
            .map(|c| c.col as usize + 1)
            .max()
            .unwrap_or(0);
        (rows, cols)
    }

    /// The used range as A1 notation (`C3:F70`), or `None` when the sheet holds no cells.
    pub fn used_range(&self) -> Option<String> {
        let min_row = self.cells.iter().map(|c| c.row).min()?;
        let min_col = self.cells.iter().map(|c| c.col).min()?;
        let (rows, cols) = self.extent();
        Some(format!(
            "{}:{}",
            cell_ref(min_row, min_col),
            cell_ref(rows as u32 - 1, cols as u32 - 1)
        ))
    }

    pub fn formula_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| c.formula != Formula::None)
            .count()
    }

    /// Formula cells with no cached value.
    pub fn uncached_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| matches!(c.formula, Formula::Text(_)) && c.value == Value::Empty)
            .count()
    }

    pub fn unrecognised_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| c.formula == Formula::Unrecognised)
            .count()
    }

    /// The merged region whose top-left cell is (row, col), as A1 notation.
    pub fn merged_at(&self, row: u32, col: u32) -> Option<String> {
        self.merged
            .as_ref()?
            .iter()
            .find(|d| d.start == (row, col))
            .map(|d| {
                format!(
                    "{}:{}",
                    cell_ref(d.start.0, d.start.1),
                    cell_ref(d.end.0, d.end.1)
                )
            })
    }
}

/// An open workbook.
pub struct Workbook {
    pub path: PathBuf,
    pub format: &'static str,
    pub sheets: Vec<SheetMeta>,
    is_1904: bool,
    inner: Inner,
    /// Set when the reader panicked: its state may be inconsistent, so nothing more is read.
    poisoned: bool,
}

impl Workbook {
    /// Open and classify a workbook.  Every failure names the file.
    pub fn open(path: &Path) -> Result<Self> {
        guarded(|| open_inner(path)).with_context(|| format!("cannot read {}", path.display()))
    }

    /// Read one worksheet's cells.  A sheet that is not a worksheet has no cells to read and is an
    /// error; callers decide beforehand whether to ask.
    pub fn read(&mut self, sheet: &SheetMeta) -> Result<SheetContent> {
        if sheet.kind != SheetKind::Worksheet {
            bail!(
                "sheet {:?} is a {}, which holds no cells",
                sheet.name,
                sheet.kind.as_str()
            );
        }
        if self.poisoned {
            bail!(
                "sheet {:?} of {} was not read: the reader failed earlier on this workbook",
                sheet.name,
                self.path.display()
            );
        }
        let is_1904 = self.is_1904;
        let name = sheet.name.as_str();
        let inner = &mut self.inner;
        let outcome = guarded(|| read_inner(inner, name, is_1904));
        if matches!(&outcome, Err(e) if e.is::<ReaderPanic>()) {
            self.poisoned = true;
        }
        outcome.with_context(|| {
            format!(
                "cannot read sheet {:?} of {}",
                sheet.name,
                self.path.display()
            )
        })
    }

    /// A sheet by its exact name.
    pub fn sheet_named(&self, name: &str) -> Option<&SheetMeta> {
        self.sheets.iter().find(|s| s.name == name)
    }

    /// The sheet names, quoted and comma-separated, for error messages.
    pub fn sheet_list(&self) -> String {
        self.sheets
            .iter()
            .map(|s| format!("{:?}", s.name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A panic caught inside a reader call.
#[derive(Debug)]
struct ReaderPanic(String);

impl std::fmt::Display for ReaderPanic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the reader failed on this file ({}); the file is malformed, or the reader has a \
             defect",
            self.0
        )
    }
}

impl std::error::Error for ReaderPanic {}

/// Run a calamine call, turning a panic into an error.  calamine indexes some binary records
/// without bounds checks; a crafted or damaged file must be exit 1, not a crash.  This relies on
/// `panic = "unwind"`, which `Cargo.toml` pins for that reason.
fn guarded<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(previous);
    match outcome {
        Ok(result) => result,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown cause".into());
            Err(ReaderPanic(msg).into())
        }
    }
}

fn open_inner(path: &Path) -> Result<Workbook> {
    cli_contract::require_input_path(path)?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext == "numbers" {
        bail!("an Apple Numbers file cannot be read; export it to .xlsx from Numbers first");
    }
    let mut head = [0u8; 8];
    let len = read_head(path, &mut head)?;
    let head = &head[..len];

    let (inner, format) = if head == OLE_MAGIC {
        open_ole(path)?
    } else if head.starts_with(&ZIP_MAGIC) {
        open_zip(path, &ext)?
    } else {
        bail!(
            "not a workbook: neither a zip container (.xlsx, .xlsm, .xlsb, .ods) nor an OLE \
             container (.xls)"
        );
    };

    let is_1904 = match &inner {
        Inner::Xlsx(wb) => wb.has_1904_epoch(),
        Inner::Xlsb(wb) => wb.has_1904_epoch(),
        Inner::Xls(wb) => wb.has_1904_epoch(),
        Inner::Ods(_) => false, // ODS stores ISO dates, not serials
    };
    let metadata = match &inner {
        Inner::Xlsx(wb) => wb.sheets_metadata().to_vec(),
        Inner::Xlsb(wb) => wb.sheets_metadata().to_vec(),
        Inner::Xls(wb) => wb.sheets_metadata().to_vec(),
        Inner::Ods(wb) => wb.sheets_metadata().to_vec(),
    };
    if metadata.is_empty() {
        // calamine opens some damaged packages cleanly and finds nothing: Unknown, not empty.
        bail!("the {format} container holds no sheets; it is damaged or not a workbook");
    }
    let sheets = metadata
        .into_iter()
        .enumerate()
        .map(|(i, s)| SheetMeta {
            index: i + 1,
            name: s.name,
            visibility: match s.visible {
                SheetVisible::Visible => Visibility::Visible,
                SheetVisible::Hidden => Visibility::Hidden,
                SheetVisible::VeryHidden => Visibility::VeryHidden,
            },
            kind: match s.typ {
                SheetType::WorkSheet => SheetKind::Worksheet,
                SheetType::ChartSheet => SheetKind::ChartSheet,
                SheetType::DialogSheet => SheetKind::DialogSheet,
                SheetType::MacroSheet => SheetKind::MacroSheet,
                SheetType::Vba => SheetKind::Vba,
            },
        })
        .collect();
    Ok(Workbook {
        path: path.to_path_buf(),
        format,
        sheets,
        is_1904,
        inner,
        poisoned: false,
    })
}

fn read_head(path: &Path, buf: &mut [u8; 8]) -> Result<usize> {
    let mut file = File::open(path)?;
    let mut len = 0;
    while len < buf.len() {
        match file.read(&mut buf[len..])? {
            0 => break,
            n => len += n,
        }
    }
    Ok(len)
}

const ENCRYPTED: &str = "the workbook is encrypted (password-protected); remove the password in the \
                         authoring application and save again";

/// An OLE container: an encrypted OOXML package, or a legacy `.xls`.
fn open_ole(path: &Path) -> Result<(Inner, &'static str)> {
    match open_workbook::<Xlsx<_>, _>(path) {
        Err(XlsxError::Password) => bail!(ENCRYPTED),
        Ok(wb) => return Ok((Inner::Xlsx(wb), "xlsx")),
        Err(_) => {} // not an encrypted package; try the legacy format
    }
    match open_workbook::<Xls<_>, _>(path) {
        Ok(wb) => Ok((Inner::Xls(wb), "xls")),
        Err(XlsError::Password) => bail!(ENCRYPTED),
        Err(XlsError::Cfb(e)) => bail!(
            "an OLE container, but not an Excel workbook (perhaps a Word file or another \
             Office format): {e}"
        ),
        Err(e) => Err(anyhow!("not a readable .xls workbook: {e}")),
    }
}

/// A zip container: `.xlsx`/`.xlsm`, `.xlsb`, or `.ods`, chosen by extension; an unexpected
/// extension tries each in turn and reports the `.xlsx` reader's error.
fn open_zip(path: &Path, ext: &str) -> Result<(Inner, &'static str)> {
    let as_xlsx = || -> Result<(Inner, &'static str)> {
        match open_workbook::<Xlsx<_>, _>(path) {
            Ok(wb) => Ok((Inner::Xlsx(wb), "xlsx")),
            Err(XlsxError::Password) => bail!(ENCRYPTED),
            Err(e) => Err(anyhow!("not a readable .xlsx workbook: {e}")),
        }
    };
    let as_xlsb = || -> Result<(Inner, &'static str)> {
        match open_workbook::<Xlsb<_>, _>(path) {
            Ok(wb) => Ok((Inner::Xlsb(wb), "xlsb")),
            Err(XlsbError::Password) => bail!(ENCRYPTED),
            Err(e) => Err(anyhow!("not a readable .xlsb workbook: {e}")),
        }
    };
    let as_ods = || -> Result<(Inner, &'static str)> {
        match open_workbook::<Ods<_>, _>(path) {
            Ok(wb) => Ok((Inner::Ods(wb), "ods")),
            Err(OdsError::Password) => bail!(ENCRYPTED),
            Err(e) => Err(anyhow!("not a readable .ods spreadsheet: {e}")),
        }
    };
    match ext {
        "xlsb" => as_xlsb(),
        "ods" => as_ods(),
        "xlsx" | "xlsm" | "xltx" | "xltm" => as_xlsx(),
        _ => as_xlsx().or_else(|first| as_ods().or_else(|_| as_xlsb()).map_err(|_| first)),
    }
}

fn read_inner(inner: &mut Inner, name: &str, is_1904: bool) -> Result<SheetContent> {
    match inner {
        Inner::Xlsx(wb) => read_xlsx(wb, name, is_1904),
        Inner::Xls(wb) => {
            let values = wb.worksheet_range(name)?;
            let formulas = wb.worksheet_formula(name)?;
            let merged = wb.merge_cells_by_sheet_name(name)?;
            Ok(from_ranges(&values, &formulas, Some(merged), is_1904))
        }
        Inner::Xlsb(wb) => {
            let values = wb.worksheet_range(name)?;
            let formulas = wb.worksheet_formula(name)?;
            Ok(from_ranges(&values, &formulas, None, is_1904))
        }
        Inner::Ods(wb) => {
            let values = wb.worksheet_range(name)?;
            let formulas = wb.worksheet_formula(name)?;
            Ok(from_ranges(&values, &formulas, None, is_1904))
        }
    }
}

/// `.xlsx` streams cells with their formulas in one pass: no dense allocation (a stray cell at
/// XFD1048576 would otherwise cost hundreds of gigabytes), and a formula whose value is empty is
/// exactly a formula with no cached value.
///
/// A shared formula's derived cells carry only the group's index, and a file not written by Excel
/// may place a derived cell before its anchor; those are resolved after the pass.  A derived cell
/// whose anchor never appears, or whose expansion fails, is a formula still: `Unrecognised`, never
/// dropped (a dropped formula with no cached value would vanish from every dump).
fn read_xlsx(wb: &mut Xlsx<Source>, name: &str, is_1904: bool) -> Result<SheetContent> {
    let merged = wb.merge_cells_by_sheet_name(name)?;
    let mut reader = wb.worksheet_cells_reader(name)?;
    let mut cells = Vec::new();
    let mut anchors: HashMap<usize, ((u32, u32), String)> = HashMap::new();
    let mut derived: Vec<(usize, usize)> = Vec::new(); // (index into cells, shared index)
    while let Some(record) = reader.next_cell_with_formula_metadata()? {
        let pos = record.pos;
        let formula = match record.formula {
            None => Formula::None,
            Some(XlsxFormulaMetadata::Normal { formula }) => text_formula(&formula),
            Some(XlsxFormulaMetadata::Shared {
                shared_index,
                formula,
                ..
            }) => {
                let f = text_formula(&formula);
                anchors.insert(shared_index, (pos, formula));
                f
            }
            Some(XlsxFormulaMetadata::SharedDerived { shared_index }) => {
                derived.push((cells.len(), shared_index));
                Formula::Unrecognised // until resolved below
            }
            // A formula kind this calamine version adds: a formula still, never dropped.
            Some(_) => Formula::Unrecognised,
        };
        let value = value_from_ref(&record.value, is_1904);
        if value == Value::Empty && formula == Formula::None {
            continue; // a styled but empty cell
        }
        cells.push(Cell {
            row: pos.0,
            col: pos.1,
            value,
            formula,
        });
    }
    for (i, si) in derived {
        let cell = &mut cells[i];
        if let Some((anchor, template)) = anchors.get(&si)
            && let Ok(f) = expand_shared_formula(template, *anchor, (cell.row, cell.col))
        {
            cell.formula = text_formula(&f);
        }
    }
    cells.sort_by_key(|c| (c.row, c.col));
    Ok(SheetContent {
        cells,
        merged: Some(merged),
    })
}

/// A formula from an Excel format (stored without `=`); an empty text is no formula text at all,
/// which for a cell the file marks as a formula means the reader could not recover it.
fn text_formula(f: &str) -> Formula {
    if f.is_empty() {
        Formula::Unrecognised
    } else {
        Formula::Text(format!("={f}"))
    }
}

/// Merge a value grid and a formula grid by absolute position: the two ranges start
/// independently, and a formula with no cached value appears in the formula grid only.
fn from_ranges(
    values: &Range<Data>,
    formulas: &Range<String>,
    merged: Option<Vec<Dimensions>>,
    is_1904: bool,
) -> SheetContent {
    let mut map: BTreeMap<(u32, u32), Cell> = BTreeMap::new();
    if let Some((r0, c0)) = values.start() {
        for (r, c, v) in values.used_cells() {
            let (row, col) = (r0 + r as u32, c0 + c as u32);
            map.insert(
                (row, col),
                Cell {
                    row,
                    col,
                    value: value_from_data(v, is_1904),
                    formula: Formula::None,
                },
            );
        }
    }
    if let Some((r0, c0)) = formulas.start() {
        for (r, c, f) in formulas.used_cells() {
            let (row, col) = (r0 + r as u32, c0 + c as u32);
            let formula = if f.starts_with(XLS_UNRECOGNISED) {
                Formula::Unrecognised
            } else {
                Formula::Text(normalize_formula(f))
            };
            map.entry((row, col))
                .or_insert_with(|| Cell {
                    row,
                    col,
                    value: Value::Empty,
                    formula: Formula::None,
                })
                .formula = formula;
        }
    }
    SheetContent {
        cells: map.into_values().collect(),
        merged,
    }
}

/// Give every formula one leading `=`.  ODS formulas arrive as `of:=[.B1]*2` (OpenFormula, whose
/// references keep their own syntax); the `of:` namespace prefix is dropped.
fn normalize_formula(f: &str) -> String {
    let f = f.strip_prefix("of:").unwrap_or(f);
    if f.starts_with('=') {
        f.to_string()
    } else {
        format!("={f}")
    }
}

fn value_from_data(v: &Data, is_1904: bool) -> Value {
    match v {
        Data::Empty => Value::Empty,
        Data::Int(i) => Value::Int(*i),
        Data::Float(f) => Value::Float(*f),
        Data::String(s) => Value::Str(s.clone()),
        Data::Bool(b) => Value::Bool(*b),
        Data::Error(e) => Value::Error(e.to_string()),
        Data::DateTime(dt) => datetime(dt, is_1904),
        Data::DateTimeIso(s) => Value::Date {
            iso: s.clone(),
            serial: None,
        },
        Data::DurationIso(s) => Value::Duration {
            iso: s.clone(),
            serial: None,
        },
    }
}

fn value_from_ref(v: &DataRef<'_>, is_1904: bool) -> Value {
    match v {
        DataRef::Empty => Value::Empty,
        DataRef::Int(i) => Value::Int(*i),
        DataRef::Float(f) => Value::Float(*f),
        DataRef::String(s) => Value::Str(s.clone()),
        DataRef::SharedString(s) => Value::Str((*s).to_string()),
        DataRef::Bool(b) => Value::Bool(*b),
        DataRef::Error(e) => Value::Error(e.to_string()),
        DataRef::DateTime(dt) => datetime(dt, is_1904),
        DataRef::DateTimeIso(s) => Value::Date {
            iso: s.clone(),
            serial: None,
        },
        DataRef::DurationIso(s) => Value::Duration {
            iso: s.clone(),
            serial: None,
        },
    }
}

/// A date-formatted number.  One that is no real date (negative, beyond 9999) stays a number:
/// the JSON then says `number`, never a guessed date.
fn datetime(dt: &ExcelDateTime, is_1904: bool) -> Value {
    let serial = dt.as_f64();
    if dt.is_duration() {
        return Value::Duration {
            iso: dates::serial_to_iso_duration(serial),
            serial: Some(serial),
        };
    }
    match dates::serial_to_iso(serial, is_1904) {
        Some(iso) => Value::Date {
            iso,
            serial: Some(serial),
        },
        None => Value::Float(serial),
    }
}

/// A 0-based (row, col) as A1 notation.
pub fn cell_ref(row: u32, col: u32) -> String {
    let mut letters = Vec::new();
    let mut n = col + 1;
    while n > 0 {
        letters.push(char::from(b'A' + ((n - 1) % 26) as u8));
        n = (n - 1) / 26;
    }
    let letters: String = letters.iter().rev().collect();
    format!("{letters}{}", row + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_refs() {
        assert_eq!(cell_ref(0, 0), "A1");
        assert_eq!(cell_ref(2, 25), "Z3");
        assert_eq!(cell_ref(9, 26), "AA10");
        assert_eq!(cell_ref(1_048_575, 16_383), "XFD1048576");
    }

    #[test]
    fn formulas_get_one_leading_equals() {
        assert_eq!(normalize_formula("SUM(A1:A3)"), "=SUM(A1:A3)");
        assert_eq!(normalize_formula("of:=[.B1]*2"), "=[.B1]*2");
        assert_eq!(normalize_formula("=A1"), "=A1");
    }
}
