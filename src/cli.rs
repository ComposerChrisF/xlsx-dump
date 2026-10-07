use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use cli_contract::CreateMode;

use crate::naming;

const EXIT_CODES_HELP: &str = "\
Exit codes:
  0  success / clean (including dry-run, a skipped existing output, and an empty sheet)
  1  tool error — the tool itself failed: a missing, unreadable, encrypted, corrupt, or
     unsupported workbook (.numbers included); a requested sheet that is not there; a missing
     output directory; an output refused because it exists.  A batch processes every file
     first and exits 1 at the end if any failed.  Never 3: a failed read has no findings.
  2  usage error — a command line wrong on its face (decidable from the arguments
     alone; a path that does not match the filesystem is 1)
  3  findings — `check` only: the workbook was read in full and holds something a reader must
     know (a hidden sheet, a formula with no cached value, a sheet whose content is not dumped).
     A sheet that could not be read makes the file a failure (1), never a finding.
  4  setup needed — not used by this tool

A reader, never a writer or evaluator: formulas are reported as text with the value the
authoring application cached; nothing is recomputed, no macro runs, no external link is followed.

Known limits of the underlying reader (calamine), reported rather than guessed:
  - Merged regions are reported for .xlsx and .xls only; for .xlsb and .ods they are unknown.
  - In .xls and .xlsb, cells using a shared or array formula show their value but no formula;
    in .xlsx an array formula's text is on its first cell only.  An .xls formula the reader
    cannot decode, or an .xlsx shared formula whose first cell is missing, is counted as
    unrecognised, never dropped silently.
  - Charts and images on an ordinary worksheet are not inspected; a chart *sheet* is reported.
  - Only .xlsx is read cell by cell; the other formats are read whole, so a stray value far
    from the data (say at XFD1048576) can exhaust memory there.
  - A date-formatted 0 (Excel's 1900-01-00) is written 00:00:00.
  - An .xls dialog sheet reads as a worksheet; an .xls protected only by XOR obfuscation (not
    real encryption) is not recognized as protected.
  - ODS formulas keep OpenFormula reference syntax (=[.B1]*2); ODS times read as durations.";

const CSV_HELP: &str = "\
Output naming (default; Obsidian-Brain's derived-copy convention):
  one-sheet workbook:   <stem>-from<Ext>.csv            Budget.xlsx -> Budget-fromXlsx.csv
  multi-sheet workbook: <stem>-from<Ext>-<sheet>.csv    one file per sheet
The form follows the workbook's sheet count (hidden sheets included), never the selection, so
a sheet's derived copy has one name however it was made.  --name replaces both forms; tokens:
  {stem}     the workbook's file name without its extension
  {FromExt}  'from' plus the extension, capitalized: fromXlsx, fromXls, fromOds, fromXlsm
  {sheet}    the sheet name, sanitized: / \\ : * ? \" < > | and control characters become _,
             leading and trailing dots and spaces are trimmed, an empty result is Sheet<index>
  {index}    the sheet's 1-based position in the workbook
Two sheets whose names collide (case-insensitively) are refused, never overwritten.

CSV content: UTF-8 with a BOM (--no-bom omits it); RFC 4180 quoting, LF line endings; anchored
at A1 (column N of the CSV is column N of the sheet, row N is row N; a row that is empty in a
one-column sheet is \"\" so readers cannot skip it); raw values — numbers at Excel's precision
of 15 significant digits in their shortest form (0.3, never 0.30000000000000004; no
separators, currency, or exponent), TRUE / FALSE, error cells as their text (#DIV/0!), dates as
ISO 8601 (YYYY-MM-DD, with THH:MM:SS when a time part exists; HH:MM:SS for a time alone),
durations as ISO 8601 (PT36H30M); a merged region's value in its top-left cell only.  A formula
with no cached value is an empty cell, counted on stderr.  A grid of more than 50 million cells
(a stray value far from the data) is refused; `cells` reads such a sheet without the grid.

Hidden and very-hidden sheets are written like any other and named on stderr; --visible-only
skips them (and a sheet named with --sheet that it would skip is exit 1).

Writing: each file is written atomically (temporary file, then rename), with the mode a new
file normally gets, and never replaces an existing file unless --overwrite is given.  Even with
--overwrite, an output is refused if it is one of the run's input workbooks, or a file this run
already wrote under a name the filesystem treats as the same (case, Unicode normalization); and
while any input cannot be inspected (permission denied, a path too long), no existing file is
replaced, since it cannot be ruled out as that input.
Every output of a workbook is planned before any is written: if one would be refused, none of
that workbook's files is written.  A write that fails afterwards (a full disk, a file that
appeared meanwhile) leaves the files already written; the report lists each, and exit is 1.

Symlinks: an input workbook is read through a symlink; a symlink at an output path is never
followed or replaced — refused, or skipped under --skip-existing.";

const CELLS_HELP: &str = "\
Prints JSON Lines on stdout, one record per non-empty cell (and per formula cell, cached value
or not), in sheet order then row-major order:
  {\"sheet\":\"Budget\",\"sheet_index\":1,\"visibility\":\"visible\",\"ref\":\"B3\",\"row\":3,\"col\":2,
   \"type\":\"number\",\"value\":1234.56,\"formula\":\"=SUM(B1:B2)\",\"merged\":\"B3:C3\"}
type is one of string, number, bool, error, date, duration, none.  Numbers are at Excel's 15
significant digits, as in `csv`.  A date or duration carries its ISO text in value and, when the
format stores one, its serial number in serial (.ods stores none).  A formula cell with no cached
value has type none, value null, and \"cached\": false; a formula whose text the reader could
not recover has \"formula_unrecognised\": true and no \"formula\".  \"formula\", \"cached\", and \"merged\" (on a merged
region's top-left cell) appear only when they apply; row, col, and sheet_index are 1-based.
visibility is visible, hidden, or very-hidden.  Field names are a contract: new fields may be
added, none is renamed or removed.";

const CHECK_HELP: &str = "\
Findings (exit 3), by kind: hidden-sheet, very-hidden-sheet, uncached-formulas (a formula with
no cached value), unrecognised-formulas (a formula whose text could not be recovered), no-cells
(a chart sheet or the like, whose content is not dumped), oversized-grid (a sheet too large for
`csv`; `cells` reads it).  An empty worksheet is not a finding.  A worksheet that fails to read
is listed as unreadable-sheet in --json, but makes the file failed: exit 1, not 3.  A workbook that cannot be read at all is
exit 1, not a finding; with several files, every file is checked first.";

#[derive(Debug, Parser)]
#[command(
    name = "xlsx-dump",
    version,
    about = "Dump spreadsheet workbooks (.xlsx, .xlsm, .xlsb, .xls, .ods) to CSV and JSON — values and formulas — for agents and spreadsheet apps.",
    after_long_help = EXIT_CODES_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List a workbook's sheets: visibility, kind, used range, formulas, merged regions.
    #[command(after_long_help = EXIT_CODES_HELP)]
    Sheets {
        /// The workbook to read.
        file: PathBuf,
        /// One JSON document on stdout.
        #[arg(long)]
        json: bool,
    },

    /// Write CSV derived copies, one per sheet; by default every sheet, beside the workbook.
    #[command(after_long_help = format!("{CSV_HELP}\n\n{EXIT_CODES_HELP}"))]
    Csv(CsvArgs),

    /// Print every cell as JSON Lines: typed value, formula, merged region, sheet visibility.
    #[command(after_long_help = format!("{CELLS_HELP}\n\n{EXIT_CODES_HELP}"))]
    Cells {
        /// The workbook to read.
        file: PathBuf,
        #[command(flatten)]
        select: SheetSelect,
        /// Only cells that hold a formula.
        #[arg(long)]
        formulas_only: bool,
    },

    /// Exit 3 if a workbook holds something a reader must know about (hidden sheets, formulas
    /// with no cached value, unreadable sheets); 0 if clean.
    #[command(after_long_help = format!("{CHECK_HELP}\n\n{EXIT_CODES_HELP}"))]
    Check {
        /// The workbooks to check.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// One JSON document on stdout (emitted on exit 3 as well).
        #[arg(long)]
        json: bool,
    },
}

/// Which sheets to read.  With neither --sheet nor --sheet-index, every sheet.
#[derive(Debug, Args, Clone, Default)]
pub struct SheetSelect {
    /// A sheet by its exact name (repeatable).  A name the workbook lacks is exit 1.
    #[arg(long = "sheet", value_name = "NAME")]
    pub names: Vec<String>,
    /// A sheet by its 1-based position, as `sheets` lists it (repeatable).
    #[arg(long = "sheet-index", value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub indexes: Vec<u32>,
    /// Skip hidden and very-hidden sheets (by default they are read like any other).
    #[arg(long)]
    pub visible_only: bool,
}

#[derive(Debug, Args)]
pub struct CsvArgs {
    /// The workbooks to convert.
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
    #[command(flatten)]
    pub select: SheetSelect,
    /// Write into this directory instead of beside each workbook.  It must exist unless
    /// --create-destination says otherwise.
    #[arg(long, value_name = "DIR")]
    pub output_dir: Option<PathBuf>,
    /// How much of a missing --output-dir to create: none (default; missing is exit 1), final
    /// (the last directory only), all.  A mount point is never created.
    #[arg(long, value_enum, default_value_t, requires = "output_dir")]
    pub create_destination: CreateMode,
    /// The output file name template (tokens under --help); replaces both default forms.
    #[arg(long = "name", value_name = "TEMPLATE", value_parser = naming::validate_template)]
    pub name: Option<String>,
    /// Replace an existing output file.
    #[arg(long, conflicts_with = "skip_existing")]
    pub overwrite: bool,
    /// Leave an existing output file alone and report it as skipped (exit 0) — for idempotent
    /// hooks.
    #[arg(long)]
    pub skip_existing: bool,
    /// Omit the UTF-8 byte-order mark.
    #[arg(long)]
    pub no_bom: bool,
    /// Report what would be written, skipped, and refused; write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// One JSON document on stdout: every workbook's outcome and every output's status.
    #[arg(long)]
    pub json: bool,
}
