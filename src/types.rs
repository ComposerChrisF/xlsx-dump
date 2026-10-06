//! Serde types for `--json` and JSONL output.  Field names are a contract with the consumers
//! (`DESIGN.md` § 2): add fields freely, never rename or remove one without asking them.

use std::path::PathBuf;

use serde::Serialize;

/// Whether a sheet is shown.  Every supported format reports it (an `.ods` sheet is never
/// very-hidden; that state exists only in Excel formats).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    Visible,
    Hidden,
    VeryHidden,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Visible => "visible",
            Visibility::Hidden => "hidden",
            Visibility::VeryHidden => "very-hidden",
        }
    }
}

/// What a sheet is.  Only a worksheet has cells this tool can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SheetKind {
    Worksheet,
    ChartSheet,
    DialogSheet,
    MacroSheet,
    Vba,
}

impl SheetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SheetKind::Worksheet => "worksheet",
            SheetKind::ChartSheet => "chart-sheet",
            SheetKind::DialogSheet => "dialog-sheet",
            SheetKind::MacroSheet => "macro-sheet",
            SheetKind::Vba => "vba",
        }
    }
}

/// `sheets --json`.
#[derive(Debug, Serialize)]
pub struct SheetsReport {
    pub file: PathBuf,
    pub format: &'static str,
    pub sheets: Vec<SheetReport>,
}

#[derive(Debug, Serialize)]
pub struct SheetReport {
    /// 1-based position in the workbook.
    pub index: usize,
    pub name: String,
    pub visibility: Visibility,
    pub kind: SheetKind,
    /// The used range, e.g. `C3:F70`; `null` when the sheet holds no cells.
    pub range: Option<String>,
    /// Rows and columns of the A1-anchored grid (`F70` → 70 rows, 6 columns); 0 when empty.
    pub rows: usize,
    pub cols: usize,
    pub formulas: usize,
    /// Formula cells with no value cached by the authoring application.
    pub uncached_formulas: usize,
    /// Formula cells whose text the reader could not recover; their values are still read.
    pub unrecognised_formulas: usize,
    /// `null` when the format does not report merged regions (`.xlsb`, `.ods`).
    pub merged_regions: Option<usize>,
    /// True when the sheet's cells could be read (false for a chart sheet and the like).
    pub readable: bool,
    /// Why the sheet could not be read, or what is unknown about it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `csv --json`.
#[derive(Debug, Serialize)]
pub struct CsvReport {
    pub dry_run: bool,
    pub files: Vec<CsvFileReport>,
}

#[derive(Debug, Serialize)]
pub struct CsvFileReport {
    pub file: PathBuf,
    pub status: FileStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub outputs: Vec<CsvOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileStatus {
    /// Every output was written, would be written (dry run), or skipped as existing.
    Ok,
    /// The workbook or one of its outputs failed; see `error` and each output's `reason`.
    Failed,
}

#[derive(Debug, Serialize)]
pub struct CsvOutput {
    pub sheet: String,
    pub sheet_index: usize,
    pub visibility: Visibility,
    pub path: PathBuf,
    pub status: OutputStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub rows: usize,
    pub cols: usize,
    pub uncached_formulas: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputStatus {
    Written,
    /// Dry run: would have been written.
    WouldWrite,
    /// `--skip-existing`: the file exists and was left alone.
    Skipped,
    /// This output could not be written: it exists, collides, or its path could not be probed.
    Refused,
    /// Not written because another output of the same workbook was refused.
    Withheld,
    /// Not written because the sheet holds no cells (a chart sheet and the like); see `reason`.
    NoCells,
    /// Not written because the sheet could not be read; see `reason`.  The file is `failed`.
    Unreadable,
}

/// `check --json`.
#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub files: Vec<CheckFileReport>,
}

#[derive(Debug, Serialize)]
pub struct CheckFileReport {
    pub file: PathBuf,
    pub status: CheckStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Clean,
    Findings,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub sheet: String,
    pub sheet_index: usize,
    /// How many (for the formula findings); 1 otherwise.
    pub count: usize,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingKind {
    HiddenSheet,
    VeryHiddenSheet,
    UncachedFormulas,
    UnrecognisedFormulas,
    /// A chart sheet, dialog sheet, or the like: its content is not cells and is not dumped.
    NoCells,
    /// A worksheet too large for a CSV grid (`render::MAX_GRID_CELLS`); `cells` reads it.
    OversizedGrid,
    /// A worksheet whose cells could not be read inside an otherwise readable workbook.  Listed
    /// for completeness; it makes the file `failed` (exit 1), never `findings`.
    UnreadableSheet,
}

/// One `cells` JSONL record.
#[derive(Debug, Serialize)]
pub struct CellRecord<'a> {
    pub sheet: &'a str,
    pub sheet_index: usize,
    pub visibility: Visibility,
    #[serde(rename = "ref")]
    pub cell_ref: String,
    pub row: u32,
    pub col: u32,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// Present (true) when the cell holds a formula whose text the reader could not recover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula_unrecognised: Option<bool>,
    /// Present on formula cells only: false when no value was cached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged: Option<String>,
}
