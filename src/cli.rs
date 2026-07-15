#[cfg(feature = "dev-preview")]
use clap::ValueEnum;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::commands::companion_mode::{
    CompanionRendererRequest, CompanionReviewDepth, CompanionReviewOptions, CompanionReviewSize,
    CompanionReviewState,
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
        #[arg(long, value_enum, hide = true, default_value_t = CompanionRendererRequest::Auto)]
        renderer: CompanionRendererRequest,
        #[arg(long, hide = true)]
        review_size: Option<CompanionReviewSize>,
        #[arg(long, hide = true)]
        review_active_pulse: bool,
        #[arg(long, value_enum, hide = true)]
        review_state: Option<CompanionReviewState>,
        #[arg(long, hide = true)]
        review_duration_ms: Option<u64>,
        #[arg(long, hide = true)]
        review_capture_dir: Option<PathBuf>,
        #[arg(long, hide = true)]
        review_runtime_metrics_out: Option<PathBuf>,
        #[arg(long, hide = true)]
        review_lifetime_frames: Option<u64>,
        #[arg(long, value_enum, hide = true)]
        review_depth: Option<CompanionReviewDepth>,
        #[arg(long, hide = true)]
        review_capture_live_values: bool,
        #[arg(long, hide = true)]
        review_force_dim: bool,
        #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
        #[arg(long, value_enum, hide = true)]
        retained_scene_runtime: Option<crate::commands::companion_mode::SceneRuntimeRollout>,
        /// Dev/test-only bounded retained fault injection (see
        /// `RetainedFaultInjection`). Compiled only with retained-renderer plus
        /// dev-preview, so it is absent from a release build.
        #[cfg(all(
            target_os = "macos",
            feature = "retained-renderer",
            feature = "dev-preview"
        ))]
        #[arg(long, value_enum, hide = true)]
        review_inject_retained_fault:
            Option<crate::commands::companion_mode::RetainedFaultInjection>,
        /// Dev-preview-only deterministic retained host lifecycle/fault harness.
        /// Emits one sanitized JSON report and exits nonzero on a mismatch.
        #[cfg(all(
            target_os = "macos",
            feature = "retained-renderer",
            feature = "dev-preview"
        ))]
        #[arg(long, value_enum, hide = true)]
        review_scene_soak: Option<crate::commands::companion_mode::ReviewSceneSoakScenario>,
    },
    #[command(hide = true)]
    CompanionApp {
        #[arg(long, value_enum, hide = true, default_value_t = CompanionRendererRequest::Auto)]
        renderer: CompanionRendererRequest,
        /// Print this build's compiled renderer capabilities and Auto policy in a
        /// stable machine-readable form, then exit without opening a window. Used
        /// by the staged release smoke to read the shipped capability matrix.
        #[arg(long, hide = true)]
        print_capabilities: bool,
        #[arg(long, hide = true)]
        review_size: Option<CompanionReviewSize>,
        #[arg(long, hide = true)]
        review_active_pulse: bool,
        #[arg(long, value_enum, hide = true)]
        review_state: Option<CompanionReviewState>,
        #[arg(long, hide = true)]
        review_duration_ms: Option<u64>,
        #[arg(long, hide = true)]
        review_capture_dir: Option<PathBuf>,
        #[arg(long, hide = true)]
        review_runtime_metrics_out: Option<PathBuf>,
        #[arg(long, hide = true)]
        review_lifetime_frames: Option<u64>,
        #[arg(long, value_enum, hide = true)]
        review_depth: Option<CompanionReviewDepth>,
        #[arg(long, hide = true)]
        review_capture_live_values: bool,
        #[arg(long, hide = true)]
        review_force_dim: bool,
        #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
        #[arg(long, value_enum, hide = true)]
        retained_scene_runtime: Option<crate::commands::companion_mode::SceneRuntimeRollout>,
        /// Dev/test-only bounded retained fault injection (see
        /// `RetainedFaultInjection`). Compiled only with retained-renderer plus
        /// dev-preview, so it is absent from a release build.
        #[cfg(all(
            target_os = "macos",
            feature = "retained-renderer",
            feature = "dev-preview"
        ))]
        #[arg(long, value_enum, hide = true)]
        review_inject_retained_fault:
            Option<crate::commands::companion_mode::RetainedFaultInjection>,
        /// Dev-preview-only deterministic retained host lifecycle/fault harness.
        /// Emits one sanitized JSON report and exits nonzero on a mismatch.
        #[cfg(all(
            target_os = "macos",
            feature = "retained-renderer",
            feature = "dev-preview"
        ))]
        #[arg(long, value_enum, hide = true)]
        review_scene_soak: Option<crate::commands::companion_mode::ReviewSceneSoakScenario>,
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
    #[cfg(feature = "renderer-spike")]
    #[command(hide = true)]
    RendererSpikeApp {
        #[arg(long, value_enum)]
        candidate: crate::renderer_spike::RendererSpikeCandidate,
        #[arg(long, value_enum)]
        track: crate::renderer_spike::RendererSpikeTrack,
        #[arg(long, value_parser = crate::renderer_spike::parse_logical_size)]
        logical_size: u16,
        #[arg(long, default_value_t = 2_000)]
        duration_ms: u64,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_enum)]
        inject_fault: Option<crate::renderer_spike::RendererSpikeFault>,
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
    #[allow(clippy::too_many_arguments)] // Flat review-flag plumbing from clap args.
    pub(crate) fn companion_review_options(
        review_size: Option<CompanionReviewSize>,
        review_active_pulse: bool,
        review_state: Option<CompanionReviewState>,
        review_duration_ms: Option<u64>,
        review_capture_dir: Option<PathBuf>,
        review_runtime_metrics_out: Option<PathBuf>,
        review_lifetime_frames: Option<u64>,
        review_depth: Option<CompanionReviewDepth>,
        review_capture_live_values: bool,
        review_force_dim: bool,
    ) -> CompanionReviewOptions {
        CompanionReviewOptions {
            initial_size: review_size,
            active_pulse: review_active_pulse,
            state: review_state,
            duration_ms: review_duration_ms,
            capture_dir: review_capture_dir,
            runtime_metrics_out: review_runtime_metrics_out,
            review_lifetime_frames,
            depth: review_depth,
            review_capture_live_values,
            force_dim_overlay: review_force_dim,
            retained_scene_runtime: None,
            // Fault injection is threaded in by the command dispatcher, not this
            // shared review-flag builder.
            #[cfg(all(
                target_os = "macos",
                feature = "retained-renderer",
                feature = "dev-preview"
            ))]
            retained_fault_injection: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use crate::commands::companion_mode::CompanionRendererRequest;
    use clap::Parser;

    #[test]
    fn companion_commands_default_to_auto_renderer() {
        for arguments in [["glorp", "companion"], ["glorp", "companion-app"]] {
            let cli = Cli::try_parse_from(arguments).expect("companion command should parse");
            let renderer = match cli.command {
                Command::Companion { renderer, .. } | Command::CompanionApp { renderer, .. } => {
                    renderer
                }
                command => panic!("expected companion command, got {command:?}"),
            };
            assert_eq!(renderer, CompanionRendererRequest::Auto);
        }
    }

    #[cfg(all(
        target_os = "macos",
        feature = "retained-renderer",
        feature = "dev-preview"
    ))]
    #[test]
    fn companion_commands_parse_typed_review_scene_soak_scenarios() {
        use crate::commands::companion_mode::ReviewSceneSoakScenario;

        for subcommand in ["companion", "companion-app"] {
            let cli = Cli::try_parse_from([
                "glorp",
                subcommand,
                "--review-scene-soak",
                "surface-outdated",
            ])
            .expect("typed scene soak should parse");
            let scenario = match cli.command {
                Command::Companion { review_scene_soak, .. }
                | Command::CompanionApp { review_scene_soak, .. } => review_scene_soak,
                command => panic!("expected companion command, got {command:?}"),
            };
            assert_eq!(scenario, Some(ReviewSceneSoakScenario::SurfaceOutdated));
        }
    }

    #[test]
    fn companion_commands_keep_explicit_legacy_renderer_selection() {
        for (subcommand, renderer_name, expected) in [
            ("companion", "classic", CompanionRendererRequest::Classic),
            ("companion", "pixel", CompanionRendererRequest::Pixel),
            (
                "companion-app",
                "classic",
                CompanionRendererRequest::Classic,
            ),
            ("companion-app", "pixel", CompanionRendererRequest::Pixel),
        ] {
            let cli = Cli::try_parse_from(["glorp", subcommand, "--renderer", renderer_name])
                .expect("explicit renderer should parse");
            let renderer = match cli.command {
                Command::Companion { renderer, .. } | Command::CompanionApp { renderer, .. } => {
                    renderer
                }
                command => panic!("expected companion command, got {command:?}"),
            };
            assert_eq!(renderer, expected);
        }
    }
}

#[cfg(test)]
mod companion_review_depth_tests {
    use super::Cli;
    use crate::commands::companion_mode::CompanionReviewDepth;
    use clap::Parser;

    fn parse_depth(command: &str, value: &str) -> Option<CompanionReviewDepth> {
        let cli = Cli::try_parse_from(["glorp", command, "--review-depth", value])
            .expect("hidden review-depth must parse");
        match cli.command {
            super::Command::Companion { review_depth, .. }
            | super::Command::CompanionApp { review_depth, .. } => review_depth,
            other => panic!("expected a companion command, got {other:?}"),
        }
    }

    #[test]
    fn companion_review_depth_parses_all_three_planes_for_both_commands() {
        for command in ["companion", "companion-app"] {
            assert_eq!(parse_depth(command, "far"), Some(CompanionReviewDepth::Far));
            assert_eq!(
                parse_depth(command, "neutral"),
                Some(CompanionReviewDepth::Neutral)
            );
            assert_eq!(
                parse_depth(command, "near"),
                Some(CompanionReviewDepth::Near)
            );
        }
    }

    #[test]
    fn companion_review_depth_normalizes_onto_the_raw_depth_contract() {
        assert_eq!(CompanionReviewDepth::Far.normalized(), -1.0);
        assert_eq!(CompanionReviewDepth::Neutral.normalized(), 0.0);
        assert_eq!(CompanionReviewDepth::Near.normalized(), 1.0);
    }

    #[test]
    fn companion_review_depth_alone_counts_as_a_review_launch() {
        let options = Cli::companion_review_options(
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            Some(CompanionReviewDepth::Near),
            false,
            false,
        );
        assert!(options.has_review_launch_options());
        assert_eq!(options.depth, Some(CompanionReviewDepth::Near));
    }

    #[test]
    fn companion_app_parses_hidden_runtime_metrics_output() {
        let cli = Cli::try_parse_from([
            "glorp",
            "companion-app",
            "--review-runtime-metrics-out",
            "target/glorp-scene-baseline/runtime-metrics.json",
        ])
        .expect("hidden runtime metrics output must parse");
        let super::Command::CompanionApp { review_runtime_metrics_out, .. } = cli.command else {
            panic!("expected companion-app");
        };
        assert_eq!(
            review_runtime_metrics_out,
            Some("target/glorp-scene-baseline/runtime-metrics.json".into())
        );
    }
}
