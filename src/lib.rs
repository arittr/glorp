pub mod cli;
pub mod commands;
pub mod config;
pub mod error;
pub mod game;
pub mod paths;
pub mod pet;
pub mod storage;
pub mod time;
pub mod tui;
pub mod usage;

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
