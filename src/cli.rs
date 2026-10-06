use std::path::PathBuf;

use clap::{Parser, Subcommand};

const EXIT_CODES_HELP: &str = "\
Exit codes:
  0  success / clean (including dry-run and legitimate no-op)
  1  tool error — the tool itself failed
  2  usage error — a command line wrong on its face (decidable from the arguments
     alone; a path that does not match the filesystem is 1)
  3  findings — the tool ran correctly and found problems
  4  setup needed — no config found / not initialized";

#[derive(Debug, Parser)]
#[command(
    name = "xlsx-dump",
    version,
    about = "Dump spreadsheet workbooks (.xlsx, .xls, .ods) to CSV and JSON — values and formulas — for agents and spreadsheet apps.",
    after_long_help = EXIT_CODES_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the sheets in a workbook (scaffold — not implemented yet; see DESIGN.md).
    Sheets {
        /// The workbook to read.
        file: PathBuf,
    },
}
