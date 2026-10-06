pub mod cli;
pub mod types;

use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::Parser;

use crate::cli::{Cli, Command};

pub fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    dispatch(cli)
}

fn dispatch(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        // Scaffold only: fail loudly rather than print a plausible empty listing.
        Command::Sheets { file } => {
            bail!(
                "`sheets` is not implemented yet (see DESIGN.md): {}",
                file.display()
            )
        }
    }
}
