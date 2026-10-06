pub mod cli;
pub mod commands;
pub mod dates;
pub mod naming;
pub mod output;
pub mod render;
pub mod types;
pub mod workbook;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

pub fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    dispatch(cli)
}

fn dispatch(cli: Cli) -> Result<ExitCode> {
    // The read-only subcommands' output is the product: a closed pipe ends them quietly.  `csv`
    // writes files, so a closed pipe must not stop it midway: it prints its report last, through
    // a write that tolerates a closed pipe, and keeps its exit code.
    if !matches!(cli.command, Command::Csv(_)) {
        cli_contract::reset_sigpipe();
    }
    match cli.command {
        Command::Sheets { file, json } => commands::sheets::run(&file, json),
        Command::Csv(args) => commands::csv::run(&args),
        Command::Cells {
            file,
            select,
            formulas_only,
        } => commands::cells::run(&file, &select, formulas_only),
        Command::Check { files, json } => commands::check::run(&files, json),
    }
}
