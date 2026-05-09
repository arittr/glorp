pub mod cli;
pub mod error;

use clap::{CommandFactory, Parser};
use cli::{Cli, Command};
use error::Result;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Help => {
            Cli::command().print_help()?;
            println!();
        }
        other => {
            println!("glorp command parsed: {other:?}");
        }
    }
    Ok(())
}
