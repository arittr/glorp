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
#[cfg(feature = "renderer-spike")]
pub mod renderer_spike;
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
        Command::Companion {
            renderer,
            review_size,
            review_active_pulse,
            review_state,
            review_duration_ms,
            review_capture_dir,
            review_runtime_metrics_out,
            review_depth,
            review_capture_live_values,
            review_force_dim,
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            retained_scene_runtime,
            #[cfg(all(
                target_os = "macos",
                feature = "retained-renderer",
                feature = "dev-preview"
            ))]
            review_inject_retained_fault,
        } => {
            let review = Cli::companion_review_options(
                review_size,
                review_active_pulse,
                review_state,
                review_duration_ms,
                review_capture_dir,
                review_runtime_metrics_out,
                review_depth,
                review_capture_live_values,
                review_force_dim,
            );
            #[cfg(all(
                target_os = "macos",
                feature = "retained-renderer",
                feature = "dev-preview"
            ))]
            let review = {
                let mut review = review;
                review.retained_fault_injection = review_inject_retained_fault;
                review
            };
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            let review = {
                let mut review = review;
                review.retained_scene_runtime = retained_scene_runtime;
                review
            };
            commands::companion::run(renderer, review)?
        }
        Command::CompanionApp { renderer, print_capabilities: true, .. } => {
            commands::companion_mode::print_companion_capabilities(
                renderer,
                &mut std::io::stdout(),
            )?;
        }
        Command::CompanionApp {
            renderer,
            print_capabilities: _,
            review_size,
            review_active_pulse,
            review_state,
            review_duration_ms,
            review_capture_dir,
            review_runtime_metrics_out,
            review_depth,
            review_capture_live_values,
            review_force_dim,
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            retained_scene_runtime,
            #[cfg(all(
                target_os = "macos",
                feature = "retained-renderer",
                feature = "dev-preview"
            ))]
            review_inject_retained_fault,
        } => {
            let review = Cli::companion_review_options(
                review_size,
                review_active_pulse,
                review_state,
                review_duration_ms,
                review_capture_dir,
                review_runtime_metrics_out,
                review_depth,
                review_capture_live_values,
                review_force_dim,
            );
            #[cfg(all(
                target_os = "macos",
                feature = "retained-renderer",
                feature = "dev-preview"
            ))]
            let review = {
                let mut review = review;
                review.retained_fault_injection = review_inject_retained_fault;
                review
            };
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            let review = {
                let mut review = review;
                review.retained_scene_runtime = retained_scene_runtime;
                review
            };
            commands::companion_app::run(renderer, review)?
        }
        #[cfg(feature = "dev-preview")]
        Command::DevPreview { out, scenario } => commands::dev_preview::run(out, scenario)?,
        #[cfg(feature = "renderer-spike")]
        Command::RendererSpikeApp {
            candidate,
            track,
            logical_size,
            duration_ms,
            out,
            inject_fault,
        } => {
            let runner_entry_micros = std::env::var("GLORP_RENDERER_SPIKE_RUNNER_ENTRY_MICROS")
                .ok()
                .and_then(|value| value.parse().ok());
            renderer_spike::run(renderer_spike::RendererSpikeOptions {
                candidate,
                track,
                logical_size,
                duration_ms,
                out,
                inject_fault,
                runner_entry_micros,
            })?
        }
        Command::Help => {
            Cli::command().print_help()?;
            println!();
        }
    }
    Ok(())
}
