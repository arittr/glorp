pub mod cli;
pub mod commands;
#[cfg(target_os = "macos")]
pub mod companion;
pub mod config;
#[cfg(feature = "dev-preview")]
pub mod dev_preview;
pub mod error;
pub mod format;
pub mod game;
#[cfg(target_os = "macos")]
pub mod menubar;
pub mod paths;
pub mod pet;
pub mod presentation;
pub mod round;
pub mod storage;
pub mod time;
pub mod tui;
pub mod usage;
pub mod watch_live;

use clap::{CommandFactory, Parser};
use cli::{Cli, Command};
use error::Result;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { seed, name, yes } => commands::init::run(seed, name, yes)?,
        #[cfg(feature = "dev-preview")]
        Command::Watch { pet } => commands::watch::run(pet.map(Into::into))?,
        #[cfg(not(feature = "dev-preview"))]
        Command::Watch {} => commands::watch::run()?,
        Command::Menubar => commands::menubar::run()?,
        Command::Status => commands::status::run()?,
        Command::Rename { name } => commands::rename::run(name)?,
        Command::Reset { yes } => commands::reset::run(yes)?,
        Command::Doctor { refresh_usage_snapshots } => {
            commands::doctor::run(refresh_usage_snapshots)?
        }
        Command::Companion => commands::companion::run()?,
        Command::CompanionApp => commands::companion_app::run()?,
        #[cfg(feature = "dev-preview")]
        Command::DevPreview { out, scenario } => commands::dev_preview::run(out, scenario)?,
        Command::Help => {
            Cli::command().print_help()?;
            println!();
        }
    }
    Ok(())
}
