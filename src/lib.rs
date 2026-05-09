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
        Command::Init { seed, name, yes } => commands::init::run(seed, name, yes)?,
        Command::Watch => commands::watch::run()?,
        Command::Status => commands::status::run()?,
        Command::Rename { name } => {
            println!("glorp rename is not implemented yet: {name}");
        }
        Command::Reset { yes } => commands::reset::run(yes)?,
        Command::Doctor => commands::doctor::run()?,
        Command::Help => {
            Cli::command().print_help()?;
            println!();
        }
    }
    Ok(())
}
