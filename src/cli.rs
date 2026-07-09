#[cfg(feature = "dev-preview")]
use clap::ValueEnum;
use clap::{Parser, Subcommand};
#[cfg(feature = "dev-preview")]
use std::path::PathBuf;

use crate::commands::companion_mode::{
    CompanionRendererMode, CompanionReviewOptions, CompanionReviewSize,
};

#[derive(Debug, Parser)]
#[command(
    name = "glorp",
    version,
    about = "A terminal pet fed by real AI coding token usage",
    after_help = "TUI keys:\n  q  quit watch mode\n  r  refresh usage\n  ?  toggle help",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create local state and hatch the first pet.
    Init {
        #[arg(long)]
        seed: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Run the live terminal pet beside your coding session.
    Watch {
        /// Dev-only: render a synthetic pet of the chosen species instead of
        /// loading on-disk state. Useful for iterating on visuals without
        /// touching the real pet.
        #[cfg(feature = "dev-preview")]
        #[arg(long, value_enum, hide = true)]
        pet: Option<DevPetSpecies>,
    },
    /// Run as a macOS menu bar app (status item + popover).
    Menubar,
    /// Open the native macOS round companion app.
    Companion {
        #[arg(long, value_enum, hide = true, default_value_t = CompanionRendererMode::Classic)]
        renderer: CompanionRendererMode,
        #[arg(long, hide = true)]
        review_size: Option<CompanionReviewSize>,
        #[arg(long, hide = true)]
        review_active_pulse: bool,
    },
    #[command(hide = true)]
    CompanionApp {
        #[arg(long, value_enum, hide = true, default_value_t = CompanionRendererMode::Classic)]
        renderer: CompanionRendererMode,
        #[arg(long, hide = true)]
        review_size: Option<CompanionReviewSize>,
        #[arg(long, hide = true)]
        review_active_pulse: bool,
    },
    /// Print a compact non-interactive pet and usage summary.
    Status,
    /// Rename the current pet without changing its seed-derived traits.
    Rename { name: String },
    /// Confirmed full reset of Glorp pet state.
    Reset {
        #[arg(long)]
        yes: bool,
    },
    /// Inspect helper availability, config paths, parser health, and diagnostics.
    Doctor {
        #[arg(long)]
        refresh_usage_snapshots: bool,
    },
    /// Show command help.
    Help,
    #[cfg(feature = "dev-preview")]
    #[command(hide = true)]
    DevPreview {
        #[arg(long, default_value = "target/glorp-preview")]
        out: PathBuf,

        #[arg(long, value_enum, default_value_t = PreviewScenarioArg::All)]
        scenario: PreviewScenarioArg,
    },
}

#[cfg(feature = "dev-preview")]
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PreviewScenarioArg {
    All,
    Watch,
    Pets,
    Props,
    Animation,
    Round,
    Smooth,
    TankLife,
    Pixel,
}

#[cfg(feature = "dev-preview")]
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DevPetSpecies {
    Fuzz,
    Blob,
    Ghost,
    Glitch,
    Crystal,
    Mech,
}

#[cfg(feature = "dev-preview")]
impl From<DevPetSpecies> for crate::pet::generation::Species {
    fn from(value: DevPetSpecies) -> Self {
        match value {
            DevPetSpecies::Fuzz => Self::Fuzz,
            DevPetSpecies::Blob => Self::Blob,
            DevPetSpecies::Ghost => Self::Ghost,
            DevPetSpecies::Glitch => Self::Glitch,
            DevPetSpecies::Crystal => Self::Crystal,
            DevPetSpecies::Mech => Self::Mech,
        }
    }
}

impl Cli {
    pub(crate) const fn companion_review_options(
        review_size: Option<CompanionReviewSize>,
        review_active_pulse: bool,
    ) -> CompanionReviewOptions {
        CompanionReviewOptions {
            initial_size: review_size,
            active_pulse: review_active_pulse,
        }
    }
}
