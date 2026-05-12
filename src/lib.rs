pub mod cli;
pub mod commands;
pub mod config;
#[cfg(feature = "dev-preview")]
pub mod dev_preview;
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
        Command::Rename { name } => commands::rename::run(name)?,
        Command::Reset { yes } => commands::reset::run(yes)?,
        Command::Doctor => commands::doctor::run()?,
        #[cfg(feature = "dev-preview")]
        Command::DevPreview { out, scenario } => commands::dev_preview::run(out, scenario)?,
        Command::Help => {
            Cli::command().print_help()?;
            println!();
        }
    }
    Ok(())
}
