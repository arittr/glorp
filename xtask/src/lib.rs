use std::path::{Component, Path};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStep {
    pub program: String,
    pub args: Vec<String>,
    pub best_effort: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XtaskCommand {
    CompanionFresh {
        release: bool,
    },
    CompanionReviewPair {
        size: u16,
        state: Option<String>,
        dimmed: bool,
        live_values: bool,
        out: String,
    },
    CompanionSceneBaseline {
        duration_ms: u64,
        out: String,
    },
    CompanionSceneLifetime {
        frames: u64,
        out: String,
    },
    CompanionSceneFaultSoak {
        out: String,
    },
    CompanionSceneNativeSmoke {
        duration_ms: u64,
        auto: bool,
        out: String,
    },
    RendererSpikeValidate {
        out: String,
    },
    RendererSpikeRun {
        candidate: String,
        track: String,
        size: u16,
        duration_ms: u64,
        out: String,
    },
    RendererSpikeQualify {
        binary: String,
        target: String,
        candidate: String,
        track: String,
        size: u16,
        duration_ms: u64,
        out: String,
    },
}

const USAGE: &str = "Usage:\n  cargo xtask companion fresh [--debug|--release]\n  cargo xtask companion review-pair --size N [--state STATE] [--dimmed] [--live-values] --out DIR\n  cargo xtask companion scene-baseline --duration-ms N --out PATH\n  cargo xtask companion scene-lifetime --frames N --out DIR\n  cargo xtask companion scene-fault-soak --out DIR\n  cargo xtask companion scene-native-smoke --duration-ms N [--auto] --out DIR\n  cargo xtask renderer-spike validate --out DIR\n  cargo xtask renderer-spike run --candidate smooth|wgpu|software --track TRACK --size 360|720 --duration-ms N --out DIR\n  cargo xtask renderer-spike qualify --binary target/renderer-spikes/bin/FILE --target TARGET --candidate smooth|wgpu --track TRACK --size 360|720 --duration-ms N --out target/renderer-spikes/DIR";

pub fn parse_args<I, S>(args: I) -> Result<XtaskCommand, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect::<Vec<_>>();

    match args.as_slice() {
        [companion, fresh] if companion == "companion" && fresh == "fresh" => {
            // The companion is an always-on CPU-rendered app. Debug objc2 builds
            // validate Objective-C message encodings at runtime and are unsuitable
            // as the default bundle developers leave running all day.
            Ok(XtaskCommand::CompanionFresh { release: true })
        }
        [companion, fresh, flag]
            if companion == "companion" && fresh == "fresh" && flag == "--release" =>
        {
            Ok(XtaskCommand::CompanionFresh { release: true })
        }
        [companion, fresh, flag]
            if companion == "companion" && fresh == "fresh" && flag == "--debug" =>
        {
            Ok(XtaskCommand::CompanionFresh { release: false })
        }
        [companion, subcommand, rest @ ..]
            if companion == "companion" && subcommand == "review-pair" =>
        {
            parse_companion_review_pair(rest)
        }
        [companion, subcommand, rest @ ..]
            if companion == "companion" && subcommand == "scene-baseline" =>
        {
            parse_companion_scene_baseline(rest)
        }
        [companion, subcommand, rest @ ..]
            if companion == "companion" && subcommand == "scene-lifetime" =>
        {
            parse_companion_scene_lifetime(rest)
        }
        [companion, subcommand, rest @ ..]
            if companion == "companion" && subcommand == "scene-fault-soak" =>
        {
            parse_companion_scene_fault_soak(rest)
        }
        [companion, subcommand, rest @ ..]
            if companion == "companion" && subcommand == "scene-native-smoke" =>
        {
            parse_companion_scene_native_smoke(rest)
        }
        [renderer, validate, out_flag, out]
            if renderer == "renderer-spike" && validate == "validate" && out_flag == "--out" =>
        {
            Ok(XtaskCommand::RendererSpikeValidate { out: out.clone() })
        }
        [renderer, run, rest @ ..] if renderer == "renderer-spike" && run == "run" => {
            parse_renderer_spike_run(rest)
        }
        [renderer, qualify, rest @ ..] if renderer == "renderer-spike" && qualify == "qualify" => {
            parse_renderer_spike_qualify(rest)
        }
        [flag] if flag == "--help" || flag == "-h" => Err(USAGE.to_string()),
        [] => Err(USAGE.to_string()),
        _ => Err(format!("unknown xtask command\n\n{USAGE}")),
    }
}

fn parse_companion_scene_baseline(args: &[String]) -> Result<XtaskCommand, String> {
    let mut duration_ms = None;
    let mut out = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for `{}`\n\n{USAGE}", args[index]))?;
        match args[index].as_str() {
            "--duration-ms" => {
                duration_ms = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid duration\n\n{USAGE}"))?,
                )
            }
            "--out" => out = Some(value.clone()),
            flag => return Err(format!("unknown scene-baseline flag `{flag}`\n\n{USAGE}")),
        }
        index += 2;
    }
    let duration_ms = duration_ms.unwrap_or(120_000);
    if duration_ms == 0 {
        return Err(format!("duration must be greater than zero\n\n{USAGE}"));
    }
    let out = out.ok_or_else(|| format!("missing --out\n\n{USAGE}"))?;
    require_owned_relative_path(&out, "docs/superpowers/measurements", "out")?;
    Ok(XtaskCommand::CompanionSceneBaseline { duration_ms, out })
}

fn parse_companion_scene_lifetime(args: &[String]) -> Result<XtaskCommand, String> {
    let mut frames = None;
    let mut out = None;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for `{flag}`\n\n{USAGE}"))?;
        match flag {
            "--frames" => {
                frames = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid frame count\n\n{USAGE}"))?,
                )
            }
            "--out" => out = Some(value.clone()),
            other => return Err(format!("unknown scene-lifetime flag `{other}`\n\n{USAGE}")),
        }
        index += 2;
    }
    let frames = frames.ok_or_else(|| format!("missing --frames\n\n{USAGE}"))?;
    if frames != 4_500 {
        return Err(format!(
            "scene-lifetime requires the frozen 4500-frame protocol\n\n{USAGE}"
        ));
    }
    let out = parse_scene_gate_out(out)?;
    Ok(XtaskCommand::CompanionSceneLifetime { frames, out })
}

fn parse_companion_scene_fault_soak(args: &[String]) -> Result<XtaskCommand, String> {
    match args {
        [out_flag, out] if out_flag == "--out" => {
            require_owned_relative_path(out, "target/glorp-scene-gates", "out")?;
            Ok(XtaskCommand::CompanionSceneFaultSoak { out: out.clone() })
        }
        [flag] if flag == "--out" => Err(format!("missing value for `--out`\n\n{USAGE}")),
        [] => Err(format!("missing --out\n\n{USAGE}")),
        [flag, ..] => Err(format!("unknown scene-fault-soak flag `{flag}`\n\n{USAGE}")),
    }
}

fn parse_companion_scene_native_smoke(args: &[String]) -> Result<XtaskCommand, String> {
    let mut duration_ms = None;
    let mut auto = false;
    let mut out = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--auto" => {
                if auto {
                    return Err(format!("duplicate --auto\n\n{USAGE}"));
                }
                auto = true;
                index += 1;
            }
            flag => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for `{flag}`\n\n{USAGE}"))?;
                match flag {
                    "--duration-ms" => {
                        duration_ms = Some(
                            value
                                .parse::<u64>()
                                .map_err(|_| format!("invalid duration\n\n{USAGE}"))?,
                        )
                    }
                    "--out" => out = Some(value.clone()),
                    other => {
                        return Err(format!(
                            "unknown scene-native-smoke flag `{other}`\n\n{USAGE}"
                        ))
                    }
                }
                index += 2;
            }
        }
    }
    let duration_ms = duration_ms.ok_or_else(|| format!("missing --duration-ms\n\n{USAGE}"))?;
    let required_duration_ms = if auto {
        NATIVE_AUTO_QUALIFICATION_DURATION_MS
    } else {
        NATIVE_LIVE_QUALIFICATION_DURATION_MS
    };
    if duration_ms != required_duration_ms {
        return Err(format!(
            "scene-native-smoke requires exactly {required_duration_ms} ms for this protocol\n\n{USAGE}"
        ));
    }
    let out = parse_scene_gate_out(out)?;
    Ok(XtaskCommand::CompanionSceneNativeSmoke { duration_ms, auto, out })
}

fn parse_scene_gate_out(out: Option<String>) -> Result<String, String> {
    let out = out.ok_or_else(|| format!("missing --out\n\n{USAGE}"))?;
    require_owned_relative_path(&out, "target/glorp-scene-gates", "out")?;
    Ok(out)
}

fn parse_renderer_spike_run(args: &[String]) -> Result<XtaskCommand, String> {
    let mut candidate = None;
    let mut track = None;
    let mut size = None;
    let mut duration_ms = None;
    let mut out = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for `{}`\n\n{USAGE}", args[index]))?;
        match args[index].as_str() {
            "--candidate" => candidate = Some(value.clone()),
            "--track" => track = Some(value.clone()),
            "--size" => {
                size = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| format!("invalid size\n\n{USAGE}"))?,
                )
            }
            "--duration-ms" => {
                duration_ms = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid duration\n\n{USAGE}"))?,
                )
            }
            "--out" => out = Some(value.clone()),
            flag => return Err(format!("unknown renderer-spike flag `{flag}`\n\n{USAGE}")),
        }
        index += 2;
    }
    let candidate = candidate.ok_or_else(|| format!("missing --candidate\n\n{USAGE}"))?;
    if !matches!(candidate.as_str(), "smooth" | "wgpu" | "software") {
        return Err(format!(
            "candidate must be smooth, wgpu, or software\n\n{USAGE}"
        ));
    }
    let track = track.ok_or_else(|| format!("missing --track\n\n{USAGE}"))?;
    let size = size.ok_or_else(|| format!("missing --size\n\n{USAGE}"))?;
    if !matches!(size, 360 | 720) {
        return Err(format!("size must be 360 or 720\n\n{USAGE}"));
    }
    let duration_ms = duration_ms.unwrap_or(2_000);
    if duration_ms == 0 {
        return Err(format!("duration must be greater than zero\n\n{USAGE}"));
    }
    let out = out.ok_or_else(|| format!("missing --out\n\n{USAGE}"))?;
    require_owned_relative_path(&out, "target/renderer-spikes", "out")?;
    Ok(XtaskCommand::RendererSpikeRun { candidate, track, size, duration_ms, out })
}

fn parse_renderer_spike_qualify(args: &[String]) -> Result<XtaskCommand, String> {
    let mut binary = None;
    let mut target = None;
    let mut candidate = None;
    let mut track = None;
    let mut size = None;
    let mut duration_ms = None;
    let mut out = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for `{}`\n\n{USAGE}", args[index]))?;
        match args[index].as_str() {
            "--binary" => binary = Some(value.clone()),
            "--target" => target = Some(value.clone()),
            "--candidate" => candidate = Some(value.clone()),
            "--track" => track = Some(value.clone()),
            "--size" => {
                size = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| format!("invalid size\n\n{USAGE}"))?,
                )
            }
            "--duration-ms" => {
                duration_ms = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("invalid duration\n\n{USAGE}"))?,
                )
            }
            "--out" => out = Some(value.clone()),
            flag => return Err(format!("unknown renderer-spike flag `{flag}`\n\n{USAGE}")),
        }
        index += 2;
    }
    let binary = binary.ok_or_else(|| format!("missing --binary\n\n{USAGE}"))?;
    require_owned_relative_path(&binary, "target/renderer-spikes/bin", "binary")?;
    let out = out.ok_or_else(|| format!("missing --out\n\n{USAGE}"))?;
    require_owned_relative_path(&out, "target/renderer-spikes", "out")?;
    let target = target.ok_or_else(|| format!("missing --target\n\n{USAGE}"))?;
    if target.trim().is_empty() || target.contains('/') || target.contains('\\') {
        return Err(format!("invalid target triple\n\n{USAGE}"));
    }
    let candidate = candidate.ok_or_else(|| format!("missing --candidate\n\n{USAGE}"))?;
    if !matches!(candidate.as_str(), "smooth" | "wgpu") {
        return Err(format!(
            "qualification candidate must be smooth or wgpu\n\n{USAGE}"
        ));
    }
    let track = track.ok_or_else(|| format!("missing --track\n\n{USAGE}"))?;
    let size = size.ok_or_else(|| format!("missing --size\n\n{USAGE}"))?;
    if !matches!(size, 360 | 720) {
        return Err(format!("size must be 360 or 720\n\n{USAGE}"));
    }
    let duration_ms = duration_ms.unwrap_or(2_000);
    if duration_ms == 0 {
        return Err(format!("duration must be greater than zero\n\n{USAGE}"));
    }
    Ok(XtaskCommand::RendererSpikeQualify {
        binary,
        target,
        candidate,
        track,
        size,
        duration_ms,
        out,
    })
}

fn parse_companion_review_pair(args: &[String]) -> Result<XtaskCommand, String> {
    let mut size = None;
    let mut state = None;
    let mut dimmed = false;
    let mut live_values = false;
    let mut out = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dimmed" => {
                dimmed = true;
                index += 1;
            }
            "--live-values" => {
                live_values = true;
                index += 1;
            }
            flag => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for `{flag}`\n\n{USAGE}"))?;
                match flag {
                    "--size" => {
                        size = Some(
                            value
                                .parse::<u16>()
                                .map_err(|_| format!("invalid size\n\n{USAGE}"))?,
                        )
                    }
                    "--state" => state = Some(value.clone()),
                    "--out" => out = Some(value.clone()),
                    other => return Err(format!("unknown review-pair flag `{other}`\n\n{USAGE}")),
                }
                index += 2;
            }
        }
    }
    let size = size.ok_or_else(|| format!("missing --size\n\n{USAGE}"))?;
    if size < 260 {
        return Err(format!("review-pair size must be at least 260\n\n{USAGE}"));
    }
    if let Some(state) = state.as_deref() {
        if !matches!(
            state,
            "normal" | "active-pulse" | "asleep-calm" | "helper-trouble"
        ) {
            return Err(format!(
                "state must be normal, active-pulse, asleep-calm, or helper-trouble\n\n{USAGE}"
            ));
        }
    }
    let out = out.ok_or_else(|| format!("missing --out\n\n{USAGE}"))?;
    // Redacted pairs live under the review root; sensitive live-value pairs are
    // confined to the sensitive review root.
    let review_root = if live_values {
        "target/glorp-review-sensitive"
    } else {
        "target/glorp-review"
    };
    require_owned_relative_path(&out, review_root, "out")?;
    Ok(XtaskCommand::CompanionReviewPair { size, state, dimmed, live_values, out })
}

fn require_owned_relative_path(value: &str, prefix: &str, label: &str) -> Result<(), String> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || !path.starts_with(prefix)
        || path == Path::new(prefix)
    {
        return Err(format!(
            "{label} must be a private-data-free relative path below `{prefix}`"
        ));
    }
    Ok(())
}

pub fn companion_fresh_steps(release: bool, retained: bool) -> Vec<ProcessStep> {
    let profile = if release { "release" } else { "debug" };
    let mut build_args = vec![
        "scripts/build-macos-companion-app.mjs".to_string(),
        "--profile".to_string(),
        profile.to_string(),
    ];
    // Apple Silicon compiles the retained backend into the fresh dev bundle so
    // an explicit `--renderer retained` has an ActiveRetainedHost to drive.
    if retained {
        build_args.push("--features".to_string());
        build_args.push("retained-renderer".to_string());
    }
    vec![
        ProcessStep {
            program: "node".to_string(),
            args: build_args,
            best_effort: false,
        },
        ProcessStep {
            program: "osascript".to_string(),
            args: vec!["-e".to_string(), "quit app \"Glorp\"".to_string()],
            best_effort: true,
        },
        ProcessStep {
            program: "pkill".to_string(),
            args: vec!["-f".to_string(), "Glorp.app/Contents/MacOS".to_string()],
            best_effort: true,
        },
        ProcessStep {
            program: "sleep".to_string(),
            args: vec!["1".to_string()],
            best_effort: true,
        },
        ProcessStep {
            program: "open".to_string(),
            args: vec!["target/macos/Glorp.app".to_string()],
            best_effort: false,
        },
    ]
}

pub fn run_xtask<I, S>(args: I, repo_root: &Path) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match parse_args(args)? {
        XtaskCommand::CompanionFresh { release } => {
            if std::env::consts::OS != "macos" {
                return Err("cargo xtask companion fresh is only supported on macOS".to_string());
            }
            let retained = cfg!(target_arch = "aarch64");
            run_steps(&companion_fresh_steps(release, retained), repo_root)
        }
        XtaskCommand::CompanionReviewPair { size, state, dimmed, live_values, out } => {
            run_companion_review_pair(repo_root, size, state.as_deref(), dimmed, live_values, &out)
        }
        XtaskCommand::CompanionSceneBaseline { duration_ms, out } => {
            run_companion_scene_baseline(repo_root, duration_ms, &out)
        }
        XtaskCommand::CompanionSceneLifetime { frames, out } => {
            run_companion_scene_lifetime(repo_root, frames, &out)
        }
        XtaskCommand::CompanionSceneFaultSoak { out } => {
            run_companion_scene_fault_soak(repo_root, &out)
        }
        XtaskCommand::CompanionSceneNativeSmoke { duration_ms, auto, out } => {
            run_companion_scene_native_smoke(repo_root, duration_ms, auto, &out)
        }
        XtaskCommand::RendererSpikeValidate { out } => validate_renderer_spike(repo_root, &out),
        XtaskCommand::RendererSpikeRun { candidate, track, size, duration_ms, out } => {
            if std::env::consts::OS != "macos" {
                return Err("cargo xtask renderer-spike run is only supported on macOS".to_string());
            }
            let features = if candidate == "wgpu" {
                "renderer-spike-wgpu"
            } else {
                "renderer-spike"
            };
            let duration = duration_ms.to_string();
            let size = size.to_string();
            let steps = vec![ProcessStep {
                program: "cargo".to_string(),
                args: vec![
                    "build".into(),
                    "--release".into(),
                    "--features".into(),
                    features.into(),
                ],
                best_effort: false,
            }];
            run_steps(&steps, repo_root)?;
            run_bounded_process(
                &repo_root.join("target/release/glorp"),
                &[
                    "renderer-spike-app".into(),
                    "--candidate".into(),
                    candidate,
                    "--track".into(),
                    track,
                    "--logical-size".into(),
                    size,
                    "--duration-ms".into(),
                    duration,
                    "--out".into(),
                    out.clone(),
                ],
                repo_root,
                Duration::from_millis(duration_ms.saturating_add(30_000)),
            )?;
            validate_renderer_spike(repo_root, &out)
        }
        XtaskCommand::RendererSpikeQualify {
            binary,
            target,
            candidate,
            track,
            size,
            duration_ms,
            out,
        } => run_renderer_spike_qualification(
            repo_root,
            &binary,
            &target,
            &candidate,
            &track,
            size,
            duration_ms,
            &out,
        ),
    }
}

/// How long the companion review process paints before it captures and exits, and
/// the extra head-room granted to the bounded process before it is force-killed.
const REVIEW_PAIR_DURATION_MS: u64 = 2_000;
const REVIEW_PAIR_TIMEOUT_HEADROOM_MS: u64 = 60_000;
const SCENE_GATE_TIMEOUT_HEADROOM_MS: u64 = 60_000;
const SCENE_GATE_REVIEW_DURATION_MS: u64 = 2_000;
const SCENE_LIFETIME_TIMEOUT_MS: u64 = 600_000;
const NATIVE_SAMPLE_INTERVAL_MS: u64 = 10_000;
const NATIVE_LIVE_QUALIFICATION_DURATION_MS: u64 = 300_000;
const NATIVE_AUTO_QUALIFICATION_DURATION_MS: u64 = 14_400_000;
const RUNTIME_METRICS_SCHEMA_VERSION: u64 = 10;
const DIRECT_SCENE_ARTIFACTS: [&str; 6] = [
    "scene.png",
    "scene-manifest.json",
    "scene-snapshot.json",
    "scene-version.json",
    "scene-metrics.json",
    "scene-artifacts.json",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct SceneCommandPlan {
    build: ProcessStep,
    init_args: Vec<String>,
    companion_args: Vec<String>,
    timeout: Duration,
}

type SceneGateEnvironment = Vec<(&'static str, String)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SceneFaultCase {
    injection: &'static str,
    expected_category: &'static str,
    expected_success: bool,
}

impl SceneFaultCase {
    fn produces_retained_host(self) -> bool {
        self.injection != "initialization"
    }

    fn is_capture_failure(self) -> bool {
        matches!(
            self.injection,
            "map-failure" | "blank-capture" | "write-failure"
        )
    }
}

const SCENE_FAULT_CASES: [SceneFaultCase; 11] = [
    SceneFaultCase {
        injection: "initialization",
        expected_category: "retained-device-unavailable",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "surface-loss",
        expected_category: "retained-surface-lost",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "validation",
        expected_category: "retained-device-validation",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "internal",
        expected_category: "retained-device-internal",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "out-of-memory",
        expected_category: "retained-device-out-of-memory",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "device-loss",
        expected_category: "retained-device-unavailable",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "resource-failure",
        expected_category: "retained-atlas-unavailable",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "unsupported-raster",
        expected_category: "retained-unsupported-raster",
        expected_success: true,
    },
    SceneFaultCase {
        injection: "map-failure",
        expected_category: "retained-capture-map-failed",
        expected_success: false,
    },
    SceneFaultCase {
        injection: "blank-capture",
        expected_category: "retained-capture-buffer-too-short",
        expected_success: false,
    },
    SceneFaultCase {
        injection: "write-failure",
        expected_category: "retained-capture-write-failed",
        expected_success: false,
    },
];

fn scene_gate_build(all_features: bool) -> ProcessStep {
    let mut args = vec!["build".into(), "--release".into()];
    if all_features {
        args.push("--all-features".into());
    } else {
        args.push("--features".into());
        args.push("retained-renderer".into());
    }
    ProcessStep {
        program: "cargo".into(),
        args,
        best_effort: false,
    }
}

fn scene_gate_init_args(seed: &str) -> Vec<String> {
    vec![
        "init".into(),
        "--seed".into(),
        seed.into(),
        "--name".into(),
        "SceneGate".into(),
        "--yes".into(),
    ]
}

fn scene_gate_companion_args(
    duration_ms: u64,
    out: &str,
    auto: bool,
    fault: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "companion-app".into(),
        "--renderer".into(),
        if auto {
            "auto".into()
        } else {
            "retained".into()
        },
    ];
    if !auto {
        args.push("--retained-scene-runtime".into());
        args.push("live".into());
    }
    args.extend([
        "--review-size".into(),
        "360x360".into(),
        "--review-duration-ms".into(),
        duration_ms.to_string(),
        "--review-capture-dir".into(),
        out.into(),
        "--review-runtime-metrics-out".into(),
        format!("{out}/scene-metrics.json"),
    ]);
    if let Some(fault) = fault {
        args.push("--review-inject-retained-fault".into());
        args.push(fault.into());
    }
    args
}

fn scene_lifetime_plan(frames: u64, out: &str) -> SceneCommandPlan {
    let mut companion_args =
        scene_gate_companion_args(SCENE_GATE_REVIEW_DURATION_MS, out, false, None);
    companion_args.push("--review-lifetime-frames".into());
    companion_args.push(frames.to_string());
    SceneCommandPlan {
        build: scene_gate_build(false),
        init_args: scene_gate_init_args("glorp-scene-lifetime-v1"),
        companion_args,
        // The frozen 4,500-sample protocol performs 67,500 real offscreen GPU
        // submissions across warmup and measured phases. Keep the process
        // bounded, but do not apply the short smoke/fault headroom to that work.
        timeout: Duration::from_millis(SCENE_LIFETIME_TIMEOUT_MS),
    }
}

fn scene_native_smoke_plan(duration_ms: u64, auto: bool, out: &str) -> SceneCommandPlan {
    SceneCommandPlan {
        build: scene_gate_build(false),
        init_args: scene_gate_init_args("glorp-scene-native-smoke-v1"),
        companion_args: scene_gate_companion_args(duration_ms, out, auto, None),
        timeout: Duration::from_millis(duration_ms.saturating_add(SCENE_GATE_TIMEOUT_HEADROOM_MS)),
    }
}

fn scene_fault_plan(case: SceneFaultCase, out: &str) -> SceneCommandPlan {
    let mut companion_args = scene_gate_companion_args(
        SCENE_GATE_REVIEW_DURATION_MS,
        out,
        false,
        Some(case.injection),
    );
    if !case.produces_retained_host() {
        // A pre-host initialization failure has no retained recorder from which
        // truthful terminal metrics could be published. The bounded review still
        // proves the acknowledged Smooth paint and sanitized failure category.
        let metrics_flag = companion_args
            .iter()
            .position(|arg| arg == "--review-runtime-metrics-out")
            .expect("scene fault plan includes runtime metrics");
        companion_args.drain(metrics_flag..=metrics_flag + 1);
    }
    SceneCommandPlan {
        build: scene_gate_build(true),
        init_args: scene_gate_init_args(&format!("glorp-scene-fault-{}", case.injection)),
        companion_args,
        timeout: Duration::from_millis(
            SCENE_GATE_REVIEW_DURATION_MS.saturating_add(SCENE_GATE_TIMEOUT_HEADROOM_MS),
        ),
    }
}

fn reset_scene_gate_out_dir(root: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale scene-gate output {}: {error}",
            root.display()
        )),
    }
}

fn prepare_scene_gate_run(
    repo_root: &Path,
    out: &str,
    plan: &SceneCommandPlan,
) -> Result<(std::path::PathBuf, SceneGateEnvironment), String> {
    let out_dir = repo_root.join(out);
    reset_scene_gate_out_dir(&out_dir)?;
    std::fs::create_dir_all(&out_dir).map_err(|error| error.to_string())?;
    let config_dir = out_dir.join("config");
    std::fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
    run_steps(std::slice::from_ref(&plan.build), repo_root)?;
    let environment = vec![(
        "GLORP_CONFIG_DIR",
        config_dir.to_string_lossy().into_owned(),
    )];
    let binary = repo_root.join("target/release/glorp");
    run_bounded_process_with_env(
        &binary,
        &plan.init_args,
        repo_root,
        Duration::from_secs(30),
        &environment,
    )?;
    Ok((binary, environment))
}

fn run_companion_scene_lifetime(repo_root: &Path, frames: u64, out: &str) -> Result<(), String> {
    require_macos_scene_gate("scene-lifetime")?;
    let plan = scene_lifetime_plan(frames, out);
    let (binary, environment) = prepare_scene_gate_run(repo_root, out, &plan)?;
    std::fs::write(
        repo_root.join(out).join("lifetime-request.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "requested_frames": frames,
            "cadence_ms": 250,
        }))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    run_bounded_process_with_env(
        &binary,
        &plan.companion_args,
        repo_root,
        plan.timeout,
        &environment,
    )?;
    validate_scene_gate_output(&repo_root.join(out), Some(frames), true)
}

fn run_companion_scene_native_smoke(
    repo_root: &Path,
    duration_ms: u64,
    auto: bool,
    out: &str,
) -> Result<(), String> {
    require_macos_scene_gate("scene-native-smoke")?;
    let plan = scene_native_smoke_plan(duration_ms, auto, out);
    let (binary, environment) = prepare_scene_gate_run(repo_root, out, &plan)?;
    let samples = run_bounded_scene_process_with_samples(
        &binary,
        &plan.companion_args,
        repo_root,
        plan.timeout,
        &environment,
        &repo_root.join(out).join("scene-metrics.json"),
    )?;
    validate_native_samples(&samples, duration_ms)?;
    std::fs::write(
        repo_root.join(out).join("native-samples.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "sample_interval_ms": NATIVE_SAMPLE_INTERVAL_MS,
            "requested_duration_ms": duration_ms,
            "samples": samples,
        }))
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    validate_scene_gate_output(&repo_root.join(out), None, true)
}

fn run_companion_scene_fault_soak(repo_root: &Path, out: &str) -> Result<(), String> {
    require_macos_scene_gate("scene-fault-soak")?;
    let root = repo_root.join(out);
    reset_scene_gate_out_dir(&root)?;
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    run_steps(&[scene_gate_build(true)], repo_root)?;
    let mut outcomes = Vec::with_capacity(SCENE_FAULT_CASES.len());
    for case in SCENE_FAULT_CASES {
        let case_out = format!("{out}/{}", case.injection);
        let plan = scene_fault_plan(case, &case_out);
        let case_dir = repo_root.join(&case_out);
        std::fs::create_dir_all(&case_dir).map_err(|error| error.to_string())?;
        let config_dir = case_dir.join("config");
        std::fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
        let environment = &[(
            "GLORP_CONFIG_DIR",
            config_dir.to_string_lossy().into_owned(),
        )];
        let binary = repo_root.join("target/release/glorp");
        run_bounded_process_with_env(
            &binary,
            &plan.init_args,
            repo_root,
            Duration::from_secs(30),
            environment,
        )?;
        let observed = run_expected_fault_process(
            &binary,
            &plan.companion_args,
            repo_root,
            plan.timeout,
            environment,
            case.expected_category,
            case.expected_success,
        )?;
        let evidence = validate_fault_case_evidence(case, &case_dir)?;
        let outcome = serde_json::json!({
            "injection": case.injection,
            "expected_category": case.expected_category,
            "observed_category": observed,
            "sanitized": true,
            "expected_exit_success": case.expected_success,
            "process_status_matched": true,
            "evidence": evidence,
        });
        std::fs::write(
            case_dir.join("fault-outcome.json"),
            serde_json::to_vec_pretty(&outcome).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        outcomes.push(outcome);
    }
    let summary = serde_json::json!({
        "schema_version": 1,
        "build_profile": "release",
        "build_features": "all-features",
        "outcomes": outcomes,
    });
    validate_fault_soak_summary(&summary)?;
    std::fs::write(
        root.join("fault-soak.json"),
        serde_json::to_vec_pretty(&summary).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn require_macos_scene_gate(command: &str) -> Result<(), String> {
    if std::env::consts::OS == "macos" {
        Ok(())
    } else {
        Err(format!(
            "cargo xtask companion {command} is only supported on macOS"
        ))
    }
}

fn run_bounded_scene_process_with_samples(
    program: &Path,
    args: &[String],
    repo_root: &Path,
    timeout: Duration,
    environment: &[(&str, String)],
    metrics_path: &Path,
) -> Result<Vec<serde_json::Value>, String> {
    println!("xtask: {} {}", program.display(), args.join(" "));
    let mut child = Command::new(program)
        .args(args)
        .envs(environment.iter().map(|(key, value)| (*key, value)))
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("failed to launch `{}`: {error}", program.display()))?;
    let started = Instant::now();
    let mut next_sample = Duration::from_millis(NATIVE_SAMPLE_INTERVAL_MS);
    let pid = child.id();
    let mut samples = vec![observe_native_sample(
        Duration::ZERO,
        metrics_path,
        true,
        native_process_rss_bytes(pid)?,
    )];
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            if !status.success() {
                return Err(format!("scene native smoke exited with {status}"));
            }
            samples.push(observe_native_sample(
                started.elapsed(),
                metrics_path,
                false,
                None,
            ));
            return Ok(samples);
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            child
                .kill()
                .map_err(|error| format!("failed to kill timed-out scene native smoke: {error}"))?;
            let _ = child.wait();
            return Err(format!(
                "scene native smoke exceeded bounded timeout of {} ms",
                timeout.as_millis()
            ));
        }
        if elapsed >= next_sample {
            samples.push(observe_native_sample(
                elapsed,
                metrics_path,
                true,
                native_process_rss_bytes(pid)?,
            ));
            next_sample =
                next_sample.saturating_add(Duration::from_millis(NATIVE_SAMPLE_INTERVAL_MS));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn observe_native_sample(
    elapsed: Duration,
    metrics_path: &Path,
    process_running: bool,
    rss_bytes: Option<u64>,
) -> serde_json::Value {
    let metrics = std::fs::read(metrics_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    serde_json::json!({
        "elapsed_ms": elapsed.as_millis().min(u128::from(u64::MAX)) as u64,
        "process_running": process_running,
        "rss_bytes": rss_bytes,
        "terminal_metrics_available": metrics.is_some(),
        "schema_version": metrics.as_ref().and_then(|value| value.get("schema_version")),
        "fallback_count": metrics.as_ref().and_then(|value| value.get("fallback_count")),
        "persistent_gpu_objects_created": metrics.as_ref().and_then(|value| value.get("persistent_gpu_objects_created")),
        "static_upload_bytes": metrics.as_ref().and_then(|value| value.get("static_upload_bytes")),
    })
}

fn native_process_rss_bytes(pid: u32) -> Result<Option<u64>, String> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("failed to sample native smoke RSS: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "native smoke RSS sampler exited with {}",
            output.status
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let kib = trimmed
        .parse::<u64>()
        .map_err(|_| "native smoke RSS sampler returned a non-numeric value".to_string())?;
    Ok(Some(kib.saturating_mul(1_024)))
}

fn validate_native_samples(
    samples: &[serde_json::Value],
    requested_duration_ms: u64,
) -> Result<(), String> {
    if requested_duration_ms < NATIVE_SAMPLE_INTERVAL_MS.saturating_mul(2) || samples.len() < 3 {
        return Err(
            "native smoke did not produce initial, running, and terminal samples".to_string(),
        );
    }
    let mut previous = None;
    for sample in samples {
        let elapsed = value_u64(sample, "elapsed_ms")?;
        if previous.is_some_and(|previous| elapsed < previous) {
            return Err("native smoke sample timestamps are not monotonic".to_string());
        }
        previous = Some(elapsed);
    }
    let running_rss = samples
        .iter()
        .filter(|sample| {
            sample
                .get("process_running")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        })
        .filter_map(|sample| sample.get("rss_bytes").and_then(serde_json::Value::as_u64))
        .collect::<Vec<_>>();
    if running_rss.len() + 1 != samples.len() || running_rss.iter().any(|rss| *rss == 0) {
        return Err("native smoke did not capture RSS for every running sample".to_string());
    }
    let warmup_sample_count = running_rss.len().div_ceil(5).max(2).min(running_rss.len());
    let rss_warmup_high_water = running_rss[..warmup_sample_count]
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    let rss_post_warmup_high_water = running_rss[warmup_sample_count..]
        .iter()
        .copied()
        .max()
        .unwrap_or(rss_warmup_high_water);
    if rss_warmup_high_water == 0
        || rss_post_warmup_high_water
            > rss_warmup_high_water.saturating_add(rss_warmup_high_water.div_ceil(100))
    {
        return Err(format!(
            "native smoke RSS grew beyond warmup high-water + 1% (warmup={rss_warmup_high_water}, post-warmup={rss_post_warmup_high_water})"
        ));
    }
    if samples
        .first()
        .and_then(|sample| sample.get("process_running"))
        .and_then(serde_json::Value::as_bool)
        != Some(true)
        || samples
            .last()
            .and_then(|sample| sample.get("process_running"))
            .and_then(serde_json::Value::as_bool)
            != Some(false)
        || samples
            .last()
            .and_then(|sample| sample.get("terminal_metrics_available"))
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        || samples
            .last()
            .and_then(|sample| sample.get("schema_version"))
            .and_then(serde_json::Value::as_u64)
            != Some(RUNTIME_METRICS_SCHEMA_VERSION)
    {
        return Err(
            "native smoke terminal sample is incomplete or has the wrong schema".to_string(),
        );
    }
    Ok(())
}

fn run_expected_fault_process(
    program: &Path,
    args: &[String],
    repo_root: &Path,
    timeout: Duration,
    environment: &[(&str, String)],
    expected_category: &str,
    expected_success: bool,
) -> Result<String, String> {
    println!("xtask: {} {}", program.display(), args.join(" "));
    let mut child = Command::new(program)
        .args(args)
        .envs(environment.iter().map(|(key, value)| (*key, value)))
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to launch `{}`: {error}", program.display()))?;
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .map_err(|error| format!("failed to collect fault output: {error}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout}\n{stderr}");
            if output.status.success() != expected_success {
                return Err(format!(
                    "fault process had unexpected status {} for category `{expected_category}`",
                    output.status
                ));
            }
            let observed = combined
                .lines()
                .flat_map(|line| line.split(|character: char| character.is_ascii_whitespace()))
                .map(|token| {
                    token.trim_matches(|character: char| {
                        !character.is_ascii_alphanumeric() && character != '-'
                    })
                })
                .filter(|token| token.starts_with("retained-"))
                .collect::<std::collections::BTreeSet<_>>();
            if observed != std::collections::BTreeSet::from([expected_category]) {
                return Err(format!(
                    "fault process did not report exactly one expected sanitized category `{expected_category}` (status {}; observed {:?})",
                    output.status, observed
                ));
            }
            return Ok(observed.into_iter().next().unwrap().to_string());
        }
        if started.elapsed() >= timeout {
            child
                .kill()
                .map_err(|error| format!("failed to kill timed-out fault process: {error}"))?;
            let _ = child.wait();
            return Err(format!(
                "fault process exceeded bounded timeout of {} ms",
                timeout.as_millis()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn validate_fault_case_evidence(
    case: SceneFaultCase,
    case_dir: &Path,
) -> Result<serde_json::Value, String> {
    let render_log = read_json_artifact(&case_dir.join("render-log.json"), "fault render log")?;
    let paint_frames = value_u64(&render_log, "paint_frame_count")?;
    let frame_count = value_u64(&render_log, "frame_count")?;
    let nonblank_shape_frames = value_u64(&render_log, "nonblank_shape_frame_count")?;
    let requested_width = snapshot_u64(&render_log, &["requested_size", "width"])?;
    let requested_height = snapshot_u64(&render_log, &["requested_size", "height"])?;
    if paint_frames == 0 || frame_count == 0 {
        return Err(format!(
            "fault soak {} did not record a bounded AppKit paint",
            case.injection
        ));
    }

    let screenshot = std::fs::read(case_dir.join("screenshot.png")).map_err(|error| {
        format!(
            "fault soak {} diagnostic screenshot is missing: {error}",
            case.injection
        )
    })?;
    let screenshot_extent = png_dimensions(&screenshot)?;
    let screenshot_width = u64::from(screenshot_extent.0);
    let screenshot_height = u64::from(screenshot_extent.1);
    let scale = screenshot_width.checked_div(requested_width).unwrap_or(0);
    let screenshot_nonblank = png_has_nonblank_pixels(&screenshot)?;
    if requested_width == 0
        || requested_height == 0
        || scale == 0
        || scale > 4
        || screenshot_width != requested_width.saturating_mul(scale)
        || screenshot_height != requested_height.saturating_mul(scale)
        || !png_has_nonempty_image_data(&screenshot)?
    {
        return Err(format!(
            "fault soak {} diagnostic screenshot is structurally incomplete",
            case.injection
        ));
    }

    for forbidden in [
        "scene.png",
        "scene-manifest.json",
        "scene-snapshot.json",
        "scene-version.json",
        "scene-artifacts.json",
    ] {
        if case_dir.join(forbidden).exists() {
            return Err(format!(
                "fault soak {} unexpectedly published direct artifact {forbidden}",
                case.injection
            ));
        }
    }

    let metrics_path = case_dir.join("scene-metrics.json");
    let (metrics_present, acknowledged_fallback, capture_failure_observed) = if !case
        .produces_retained_host()
    {
        if metrics_path.exists() || nonblank_shape_frames == 0 || !screenshot_nonblank {
            return Err(
                    "initialization fault requires a nonblank acknowledged Smooth paint and no retained metrics"
                        .to_string(),
                );
        }
        (false, true, false)
    } else {
        let metrics = read_json_artifact(&metrics_path, "fault scene metrics")?;
        validate_runtime_metrics_schema(&metrics)?;
        let fallback_count = snapshot_u64(&metrics, &["fallback_count"])?;
        let fallback_pending = snapshot_u64(&metrics, &["fallback_pending_transitions"])?;
        let fallback_painted = snapshot_u64(&metrics, &["fallback_painted_transitions"])?;
        let capture_attempted = snapshot_u64(&metrics, &["capture_attempted"])?;
        let capture_succeeded = snapshot_u64(&metrics, &["capture_succeeded"])?;
        let capture_failed = snapshot_u64(&metrics, &["capture_failed"])?;
        let capture_nonblank = snapshot_u64(&metrics, &["capture_nonblank_validated"])?;
        if case.is_capture_failure() {
            if fallback_count != 0
                || fallback_pending != 0
                || fallback_painted != 0
                || capture_attempted != 1
                || capture_succeeded != 0
                || capture_failed != 1
                || capture_nonblank != 0
            {
                return Err(format!(
                        "fault soak {} did not record the expected failed direct capture without fallback",
                        case.injection
                    ));
            }
            (true, false, true)
        } else {
            if fallback_count != 1
                || fallback_pending != 1
                || fallback_painted != 1
                || nonblank_shape_frames == 0
                || !screenshot_nonblank
                || capture_attempted != 0
                || capture_succeeded != 0
                || capture_failed != 0
            {
                return Err(format!(
                        "fault soak {} did not record exactly one acknowledged nonblank Smooth fallback",
                        case.injection
                    ));
            }
            (true, true, false)
        }
    };

    Ok(serde_json::json!({
        "render_log_valid": true,
        "paint_frame_count": paint_frames,
        "frame_count": frame_count,
        "nonblank_shape_frame_count": nonblank_shape_frames,
        "diagnostic_appkit_screenshot_structurally_valid": true,
        "diagnostic_appkit_screenshot_nonblank": screenshot_nonblank,
        "diagnostic_appkit_screenshot_extent": [screenshot_extent.0, screenshot_extent.1],
        "runtime_metrics_present": metrics_present,
        "acknowledged_fallback_paint": acknowledged_fallback,
        "capture_failure_observed": capture_failure_observed,
        "direct_artifacts_absent": true,
    }))
}

/// Removes a prior review-pair output directory (if any) so a run that legitimately
/// produces no manifest — e.g. a graceful retained-unavailable fallback that exits
/// before writing one — can never be validated against a stale manifest left behind
/// by an earlier successful run. A missing directory is not an error; the app (or
/// `create_dir_all`) recreates it on the way in.
fn reset_review_out_dir(root: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn run_companion_review_pair(
    repo_root: &Path,
    size: u16,
    state: Option<&str>,
    dimmed: bool,
    live_values: bool,
    out: &str,
) -> Result<(), String> {
    if std::env::consts::OS != "macos" {
        return Err("cargo xtask companion review-pair is only supported on macOS".to_string());
    }
    reset_review_out_dir(&repo_root.join(out))?;
    run_steps(
        &[ProcessStep {
            program: "cargo".to_string(),
            args: vec![
                "build".into(),
                "--release".into(),
                "--features".into(),
                "retained-renderer".into(),
            ],
            best_effort: false,
        }],
        repo_root,
    )?;
    // Force the retained renderer so an ActiveRetainedHost exists for the retained
    // half; a runtime fallback then honestly fails manifest validation below.
    let mut args: Vec<String> = vec![
        "companion-app".into(),
        "--renderer".into(),
        "retained".into(),
        "--review-capture-dir".into(),
        out.to_string(),
        "--review-size".into(),
        format!("{size}x{size}"),
        "--review-duration-ms".into(),
        REVIEW_PAIR_DURATION_MS.to_string(),
    ];
    if let Some(state) = state {
        args.push("--review-state".into());
        args.push(state.to_string());
    }
    if dimmed {
        args.push("--review-force-dim".into());
    }
    if live_values {
        args.push("--review-capture-live-values".into());
    }
    run_bounded_process(
        &repo_root.join("target/release/glorp"),
        &args,
        repo_root,
        Duration::from_millis(
            REVIEW_PAIR_DURATION_MS.saturating_add(REVIEW_PAIR_TIMEOUT_HEADROOM_MS),
        ),
    )?;
    validate_pair_manifest(repo_root, out)
}

fn run_companion_scene_baseline(
    repo_root: &Path,
    duration_ms: u64,
    out: &str,
) -> Result<(), String> {
    if std::env::consts::OS != "macos" {
        return Err("cargo xtask companion scene-baseline is only supported on macOS".to_string());
    }
    let source_commit = required_command_output(repo_root, "git", &["rev-parse", "HEAD"])?;
    let tracked_state = required_command_output(
        repo_root,
        "git",
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !tracked_state.is_empty() {
        return Err(format!(
            "scene baseline requires a clean committed source tree before build; tracked changes:\n{tracked_state}"
        ));
    }
    let source_identity = BaselineSourceIdentity {
        commit: source_commit,
        tracked_tree_state: "clean".to_string(),
    };
    let work = repo_root.join("target/glorp-scene-baseline");
    match std::fs::remove_dir_all(&work) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    std::fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    let capture_dir = repo_root.join("target/glorp-review/scene-baseline");
    match std::fs::remove_dir_all(&capture_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    let config_dir = work.join("config");
    let metrics_path = capture_dir.join("scene-metrics.json");

    run_steps(
        &[ProcessStep {
            program: "cargo".into(),
            args: vec![
                "build".into(),
                "--release".into(),
                "--features".into(),
                "retained-renderer".into(),
            ],
            best_effort: false,
        }],
        repo_root,
    )?;

    let binary = repo_root.join("target/release/glorp");
    let environment = &[(
        "GLORP_CONFIG_DIR",
        config_dir.to_string_lossy().into_owned(),
    )];
    run_bounded_process_with_env(
        &binary,
        &[
            "init".into(),
            "--seed".into(),
            "glorp-scene-baseline-v1".into(),
            "--name".into(),
            "Baseline".into(),
            "--yes".into(),
        ],
        repo_root,
        Duration::from_secs(30),
        environment,
    )?;
    let companion_args = scene_baseline_companion_args(duration_ms, &metrics_path);
    run_bounded_process_with_env(
        &binary,
        &companion_args,
        repo_root,
        Duration::from_millis(duration_ms.saturating_add(SCENE_LIFETIME_TIMEOUT_MS)),
        environment,
    )?;

    let snapshot: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metrics_path).map_err(|error| {
            format!(
                "runtime metrics snapshot missing at {}: {error}",
                metrics_path.display()
            )
        })?)
        .map_err(|error| format!("runtime metrics snapshot is invalid JSON: {error}"))?;
    validate_runtime_snapshot(&snapshot, true)?;
    validate_lifetime_audit(&snapshot, 4_500)?;
    validate_scene_runtime_invariants(&snapshot, true)?;
    validate_direct_scene_artifacts(&capture_dir, &snapshot)?;
    let gates = evaluate_baseline_gates(&snapshot)?;
    validate_gate_results(&gates)?;
    let report =
        render_scene_baseline_report(repo_root, duration_ms, &snapshot, &source_identity, &gates)?;
    let out_path = repo_root.join(out);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&out_path, report).map_err(|error| error.to_string())?;
    println!("xtask: wrote {}", out_path.display());
    Ok(())
}

fn scene_baseline_companion_args(duration_ms: u64, metrics_path: &Path) -> Vec<String> {
    vec![
        "companion-app".into(),
        "--renderer".into(),
        "retained".into(),
        "--retained-scene-runtime".into(),
        "live".into(),
        "--review-size".into(),
        "360x360".into(),
        "--review-duration-ms".into(),
        duration_ms.to_string(),
        "--review-capture-dir".into(),
        "target/glorp-review/scene-baseline".into(),
        "--review-runtime-metrics-out".into(),
        metrics_path.to_string_lossy().into_owned(),
        "--review-lifetime-frames".into(),
        "4500".into(),
    ]
}

struct BaselineSourceIdentity {
    commit: String,
    tracked_tree_state: String,
}

fn validate_runtime_snapshot(
    snapshot: &serde_json::Value,
    require_activation_sample: bool,
) -> Result<(), String> {
    validate_runtime_metrics_schema(snapshot)?;
    let required_metrics: &[&str] = &[
        "ui_tick_us",
        "projection_us",
        "reconcile_us",
        "delta_write_us",
        "encode_us",
        "submit_us",
        "capture_us",
        "worker_active_compile_us",
        "generation_service_ui_us",
        "gpu_materialize_publish_us",
    ];
    for metric in required_metrics {
        for percentile in ["p50", "p95", "p99", "max"] {
            if snapshot
                .get(metric)
                .and_then(|value| value.get(percentile))
                .and_then(serde_json::Value::as_u64)
                .is_none()
            {
                return Err(format!(
                    "runtime metrics snapshot missing {metric}.{percentile}"
                ));
            }
        }
    }
    if require_activation_sample {
        for percentile in ["p50", "p95", "p99", "max"] {
            if snapshot
                .get("activation_render_owner_us")
                .and_then(|value| value.get(percentile))
                .and_then(serde_json::Value::as_u64)
                .is_none()
            {
                return Err(format!(
                    "runtime metrics snapshot missing activation_render_owner_us.{percentile}"
                ));
            }
        }
    }
    let inventory = snapshot
        .get("inventory")
        .ok_or_else(|| "runtime metrics snapshot missing inventory".to_string())?;
    let fixture = snapshot
        .get("fixture")
        .ok_or_else(|| "runtime metrics snapshot missing fixture".to_string())?;
    if fixture
        .get("fixture_id")
        .and_then(serde_json::Value::as_str)
        != Some("glorp-scene-baseline-v2")
    {
        return Err(
            "runtime metrics snapshot has unknown baseline fixture disposition".to_string(),
        );
    }
    fixture_semantic_cadence_ms(fixture)?;
    for (field, expected) in [
        ("matrix_fixture_count", 630),
        ("dimmed_fixture_count", 126),
        ("full_props_tank_fixture_count", 630),
    ] {
        if value_u64(inventory, field)? != expected {
            return Err(format!("runtime inventory {field} is not {expected}"));
        }
    }
    for (field, limit) in [
        ("max_prepared_gpu_primitives", 1_024),
        ("max_nodes", 128),
        ("max_static_primitives", 768),
        ("max_pet_slots", 130),
        ("max_visible_props", 10),
        ("max_round_tank_inhabitants", 2),
        ("max_ambient_instances", 64),
        ("max_blended_draws", 256),
        ("max_lights", 2),
        ("max_attachments", 32),
    ] {
        let contract = inventory
            .get(field)
            .ok_or_else(|| format!("runtime inventory missing {field}"))?;
        let observed = contract
            .get("observed")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let reservation = value_u64(contract, "reservation")?;
        let headroom = value_u64(contract, "headroom")?;
        let recorded_limit = value_u64(contract, "limit")?;
        if recorded_limit != limit
            || observed
                .saturating_add(reservation)
                .saturating_add(headroom)
                > limit
        {
            return Err(format!(
                "runtime inventory {field} observed={observed} reservation={reservation} headroom={headroom} limit={recorded_limit} violates frozen limit {limit}"
            ));
        }
    }
    for field in ["fixture", "hidden_segment", "gpu_accounting"] {
        if snapshot.get(field).is_none()
            || snapshot.get(field).is_some_and(serde_json::Value::is_null)
        {
            return Err(format!("runtime metrics snapshot missing {field}"));
        }
    }
    for field in [
        "capture_attempted",
        "capture_succeeded",
        "capture_failed",
        "main_thread_raster_calls",
        "worker_raster_calls",
        "worker_submissions",
        "worker_completions",
        "worker_cancellations",
        "worker_coalesces",
        "worker_stale_rejections",
        "worker_failures",
        "gpu_materializations",
        "generation_count",
    ] {
        snapshot_u64(snapshot, &[field])?;
    }
    let gpu_accounting = snapshot
        .get("gpu_accounting")
        .ok_or_else(|| "runtime metrics snapshot missing gpu_accounting".to_string())?;
    for field in [
        "peak_total_bytes",
        "peak_total_objects",
        "objects_created_total",
        "objects_destroyed_total",
    ] {
        value_u64(gpu_accounting, field)?;
    }
    if let Some(lifetime) = snapshot
        .get("lifetime_audit")
        .filter(|value| !value.is_null())
    {
        for field in [
            "semantic_samples",
            "warmup_semantic_samples",
            "presentation_ticks",
            "warmup_presentation_ticks",
            "semantic_cadence_ms",
            "presentation_cadence_hz",
            "virtual_elapsed_ms",
            "snapshot_projections",
            "semantic_reconciles",
            "frame_projections",
            "frame_reconciles",
            "encoded_ticks",
            "submitted_ticks",
            "draw_calls",
            "poll_count",
            "rss_warmup_bytes",
            "rss_warmup_peak_bytes",
            "rss_final_bytes",
            "rss_peak_bytes",
            "gpu_warmup_bytes",
            "gpu_warmup_peak_bytes",
            "gpu_final_bytes",
            "gpu_peak_bytes",
        ] {
            value_u64(lifetime, field)?;
        }
    }
    Ok(())
}

fn fixture_semantic_cadence_ms(fixture: &serde_json::Value) -> Result<u64, String> {
    fixture
        .get("semantic_cadence_ms")
        .or_else(|| fixture.get("cadence_ms"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "runtime snapshot missing fixture.semantic_cadence_ms".to_string())
}

/// Intentionally isolated because the shared lifetime producer is expected to
/// advance this contract independently of xtask. Update the one constant and
/// schema-specific lifetime helper together when that producer lands.
fn validate_runtime_metrics_schema(snapshot: &serde_json::Value) -> Result<(), String> {
    let observed = snapshot
        .get("schema_version")
        .and_then(serde_json::Value::as_u64);
    if observed != Some(RUNTIME_METRICS_SCHEMA_VERSION) {
        return Err(format!(
            "runtime metrics snapshot schema_version is not {RUNTIME_METRICS_SCHEMA_VERSION} (observed {})",
            observed.map_or_else(|| "missing".to_string(), |value| value.to_string())
        ));
    }
    Ok(())
}

fn validate_scene_gate_output(
    root: &Path,
    requested_lifetime_frames: Option<u64>,
    require_direct_capture: bool,
) -> Result<(), String> {
    let metrics_path = root.join("scene-metrics.json");
    let metrics = read_json_artifact(&metrics_path, "scene metrics")?;
    validate_runtime_snapshot(&metrics, requested_lifetime_frames.is_none())?;
    if let Some(frames) = requested_lifetime_frames {
        validate_lifetime_audit(&metrics, frames)?;
        validate_scene_runtime_invariants(&metrics, true)?;
        let gates = evaluate_lifetime_gates(&metrics)?;
        validate_gate_results(&gates)?;
    } else {
        validate_scene_runtime_invariants(&metrics, false)?;
        let gates = evaluate_native_smoke_gates(&metrics)?;
        validate_gate_results(&gates)?;
    }
    if require_direct_capture {
        validate_direct_scene_artifacts(root, &metrics)?;
    }
    Ok(())
}

fn read_json_artifact(path: &Path, label: &str) -> Result<serde_json::Value, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("{label} missing at {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{label} at {} is invalid JSON: {error}", path.display()))
}

fn validate_scene_runtime_invariants(
    snapshot: &serde_json::Value,
    lifetime_requested: bool,
) -> Result<(), String> {
    for field in [
        "fallback_count",
        "fallback_pending_transitions",
        "fallback_painted_transitions",
        "main_thread_raster_calls",
        "worker_failures",
        "generation_failures",
        "generation_retries",
        "generation_stale_drops",
    ] {
        if snapshot_u64(snapshot, &[field])? != 0 {
            return Err(format!("scene runtime invariant requires {field}=0"));
        }
    }
    if !lifetime_requested {
        for field in ["persistent_gpu_objects_created", "static_upload_bytes"] {
            if snapshot_u64(snapshot, &[field])? != 0 {
                return Err(format!("scene runtime invariant requires {field}=0"));
            }
        }
    }
    for (field, limit) in [
        ("node_high_water", 128),
        ("primitive_high_water", 768),
        ("blended_draw_high_water", 256),
    ] {
        let observed = snapshot_u64(snapshot, &[field])?;
        if observed > limit {
            return Err(format!(
                "scene runtime {field}={observed} exceeds frozen limit {limit}"
            ));
        }
    }
    let hidden = snapshot
        .get("hidden_segment")
        .ok_or_else(|| "runtime snapshot missing hidden_segment".to_string())?;
    let hidden_delta = hidden
        .get("steady_delta")
        .ok_or_else(|| "runtime snapshot missing hidden_segment.steady_delta".to_string())?;
    for field in [
        "prepare",
        "worker_submissions",
        "gpu_materializations",
        "queue_writes",
        "surface_acquires",
        "encode",
        "submit",
    ] {
        if value_u64(hidden_delta, field)? != 0 {
            return Err(format!(
                "scene runtime performed hidden steady-state work: {field}"
            ));
        }
    }
    Ok(())
}

fn validate_lifetime_audit(snapshot: &serde_json::Value, frames: u64) -> Result<(), String> {
    let lifetime = snapshot
        .get("lifetime_audit")
        .filter(|value| !value.is_null())
        .ok_or_else(|| "runtime snapshot missing lifetime_audit".to_string())?;
    validate_lifetime_audit_v10_shape(lifetime, frames)
}

fn validate_lifetime_audit_v10_shape(
    lifetime: &serde_json::Value,
    frames: u64,
) -> Result<(), String> {
    if value_u64(lifetime, "semantic_samples")? != frames
        || value_u64(lifetime, "warmup_semantic_samples")? != frames
        || value_u64(lifetime, "semantic_cadence_ms")? != 250
        || value_u64(lifetime, "virtual_elapsed_ms")? != frames.saturating_mul(250)
    {
        return Err("lifetime semantic schedule does not match requested frames".to_string());
    }
    let expected_presentation_ticks = frames.saturating_mul(15).div_ceil(2);
    let presentation_ticks = value_u64(lifetime, "presentation_ticks")?;
    if presentation_ticks != expected_presentation_ticks
        || value_u64(lifetime, "warmup_presentation_ticks")? != presentation_ticks
        || value_u64(lifetime, "presentation_cadence_hz")? != 30
        || value_u64(lifetime, "snapshot_projections")? != frames
        || value_u64(lifetime, "semantic_reconciles")? != frames
        || value_u64(lifetime, "frame_projections")? != presentation_ticks
        || value_u64(lifetime, "frame_reconciles")? != presentation_ticks
        || value_u64(lifetime, "encoded_ticks")? != presentation_ticks
        || value_u64(lifetime, "submitted_ticks")? != presentation_ticks
    {
        return Err("lifetime presentation schedule is incomplete".to_string());
    }
    let work_delta = lifetime
        .get("work_delta")
        .ok_or_else(|| "lifetime audit missing work_delta".to_string())?;
    if value_u64(work_delta, "encode")? != presentation_ticks
        || value_u64(work_delta, "submit")? != presentation_ticks
    {
        return Err("lifetime measured work does not match presentation ticks".to_string());
    }
    let work_per_second = lifetime
        .get("work_per_second")
        .ok_or_else(|| "lifetime audit missing work_per_second".to_string())?;
    if value_u64(work_per_second, "encode")? != 30 || value_u64(work_per_second, "submit")? != 30 {
        return Err("lifetime per-second work is inconsistent with 30 Hz".to_string());
    }
    for field in [
        "capacity_growth_events",
        "stale_mutations",
        "stale_rejections",
        "stale_regenerations",
        "post_warmup_resource_creations",
        "post_warmup_static_upload_bytes",
    ] {
        if value_u64(lifetime, field)? != 0 {
            return Err(format!("lifetime audit recorded forbidden {field}"));
        }
    }
    for field in [
        "direct_target_prewarmed",
        "direct_target_reused",
        "direct_readback_prewarmed",
        "direct_readback_reused",
    ] {
        if lifetime.get(field).and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(format!("lifetime audit missing true {field}"));
        }
    }
    for field in [
        "terminal_direct_capture_attempted",
        "terminal_direct_capture_succeeded",
        "terminal_direct_capture_nonblank",
    ] {
        if value_u64(lifetime, field)? != 1 {
            return Err(format!("lifetime audit requires {field}=1"));
        }
    }
    for field in ["draw_calls", "poll_count"] {
        if value_u64(lifetime, field)? == 0 {
            return Err(format!(
                "lifetime audit has no production work evidence in {field}"
            ));
        }
    }
    validate_lifetime_memory(lifetime)
}

fn validate_lifetime_memory(lifetime: &serde_json::Value) -> Result<(), String> {
    for (label, warmup, final_value, peak) in [
        (
            "rss",
            value_u64(lifetime, "rss_warmup_peak_bytes")?,
            value_u64(lifetime, "rss_final_bytes")?,
            value_u64(lifetime, "rss_peak_bytes")?,
        ),
        (
            "gpu",
            value_u64(lifetime, "gpu_warmup_peak_bytes")?,
            value_u64(lifetime, "gpu_final_bytes")?,
            value_u64(lifetime, "gpu_peak_bytes")?,
        ),
    ] {
        let limit = warmup.saturating_add(warmup.div_ceil(100));
        if warmup == 0 || final_value > limit || peak > limit {
            return Err(format!(
                "lifetime {label} growth exceeds warmup high-water plus 1%"
            ));
        }
    }
    Ok(())
}

fn validate_direct_scene_artifacts(
    root: &Path,
    expected_metrics: &serde_json::Value,
) -> Result<(), String> {
    for artifact in DIRECT_SCENE_ARTIFACTS {
        let path = root.join(artifact);
        let bytes = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if bytes == 0 {
            return Err(format!(
                "direct scene capture artifact missing or empty: {}",
                path.display()
            ));
        }
    }
    let manifest = read_json_artifact(&root.join("scene-manifest.json"), "scene manifest")?;
    validate_direct_scene_manifest(&manifest)?;
    let snapshot = read_json_artifact(&root.join("scene-snapshot.json"), "scene snapshot")?;
    let version = read_json_artifact(&root.join("scene-version.json"), "scene version")?;
    let metrics = read_json_artifact(&root.join("scene-metrics.json"), "scene metrics")?;
    let artifacts = read_json_artifact(&root.join("scene-artifacts.json"), "scene artifacts")?;
    if metrics != *expected_metrics {
        return Err("scene metrics changed while validating direct artifacts".to_string());
    }
    validate_runtime_metrics_schema(&metrics)?;
    validate_direct_scene_receipts(&manifest, &snapshot, &version)?;
    validate_scene_artifact_capacities(&artifacts)?;
    validate_nonblank_scene_png(&root.join("scene.png"), &manifest)
}

fn validate_direct_scene_manifest(manifest: &serde_json::Value) -> Result<(), String> {
    const MAX_PRESENT_AGE_MS: u64 = 2_000;
    for (field, expected) in [
        ("route", "direct-retained-scene"),
        ("effective_renderer", "retained"),
        ("nonblank_validation", "valid"),
    ] {
        if manifest.get(field).and_then(serde_json::Value::as_str) != Some(expected) {
            return Err(format!("direct scene manifest {field} is not `{expected}`"));
        }
    }
    if manifest
        .get("fallback_occurred")
        .and_then(serde_json::Value::as_bool)
        != Some(false)
        || manifest
            .get("fallback_transition_count")
            .and_then(serde_json::Value::as_u64)
            != Some(0)
        || !manifest
            .get("last_fallback_reason")
            .is_some_and(serde_json::Value::is_null)
    {
        return Err("direct scene manifest recorded a fallback".to_string());
    }
    if manifest
        .get("failure_category")
        .is_some_and(|value| !value.is_null())
        || manifest
            .get("skip_category")
            .is_some_and(|value| !value.is_null())
    {
        return Err("direct scene manifest recorded a failure or skip".to_string());
    }
    let presented_frames = value_u64(manifest, "presented_frame_count")?;
    let present_age_ms = value_u64(manifest, "last_present_age_ms")?;
    if presented_frames == 0 || present_age_ms > MAX_PRESENT_AGE_MS {
        return Err(format!(
            "direct scene manifest has no recent genuine present (frames={presented_frames}, age={present_age_ms} ms)"
        ));
    }
    if manifest
        .get("capture_checksum_sha256")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|value| {
            value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err("direct scene manifest has no valid capture checksum".to_string());
    }
    Ok(())
}

fn validate_direct_scene_receipts(
    manifest: &serde_json::Value,
    snapshot: &serde_json::Value,
    version: &serde_json::Value,
) -> Result<(), String> {
    let receipt = manifest
        .get("receipt")
        .ok_or_else(|| "direct scene manifest missing receipt".to_string())?;
    if version.get("receipt") != Some(receipt) {
        return Err("scene version receipt does not match manifest".to_string());
    }
    let scene_version = receipt
        .get("scene_version")
        .ok_or_else(|| "direct scene receipt missing scene_version".to_string())?;
    if snapshot.pointer("/request/requested_version") != Some(scene_version)
        || snapshot.get("readback_version") != Some(scene_version)
    {
        return Err("scene snapshot does not preserve the presented scene version".to_string());
    }
    if manifest
        .get("privacy_disposition")
        .and_then(serde_json::Value::as_str)
        != Some("external-redacted")
    {
        return Err("direct scene capture is not externally redacted".to_string());
    }
    Ok(())
}

fn validate_scene_artifact_capacities(artifacts: &serde_json::Value) -> Result<(), String> {
    let template = artifacts
        .get("template")
        .ok_or_else(|| "scene artifacts missing template".to_string())?;
    let content = artifacts
        .get("content")
        .ok_or_else(|| "scene artifacts missing content".to_string())?;
    let frame = artifacts
        .get("frame")
        .ok_or_else(|| "scene artifacts missing frame".to_string())?;
    for (label, observed, limit) in [
        (
            "scene primitives",
            value_u64(template, "primitive_count")?,
            768,
        ),
        (
            "blended draws",
            value_u64(template, "blended_draw_count")?,
            256,
        ),
        (
            "pet art slots",
            json_array_len(content, "occupied_pet_art_slots")?,
            130,
        ),
        (
            "visible props",
            json_array_len(content, "occupied_prop_slots")?,
            10,
        ),
        (
            "tank inhabitants",
            json_array_len(content, "occupied_tank_slots")?,
            2,
        ),
        (
            "ambient instances",
            json_array_len(content, "active_ambient_slots")?,
            64,
        ),
        ("scene nodes", json_array_len(frame, "nodes")?, 128),
        ("lights", value_u64(frame, "light_count")?, 2),
    ] {
        if observed > limit {
            return Err(format!(
                "direct {label} count {observed} exceeds frozen limit {limit}"
            ));
        }
    }
    Ok(())
}

fn json_array_len(value: &serde_json::Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .and_then(|values| u64::try_from(values.len()).ok())
        .ok_or_else(|| format!("runtime snapshot missing {field}"))
}

fn validate_nonblank_scene_png(path: &Path, manifest: &serde_json::Value) -> Result<(), String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("failed to read scene png {}: {error}", path.display()))?;
    let (width, height) = png_dimensions(&bytes)?;
    let physical = manifest
        .get("physical_pixels")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "direct scene manifest missing physical_pixels".to_string())?;
    if physical.first().and_then(serde_json::Value::as_u64) != Some(u64::from(width))
        || physical.get(1).and_then(serde_json::Value::as_u64) != Some(u64::from(height))
    {
        return Err("scene png dimensions do not match direct manifest".to_string());
    }
    // Pixel nonblank is asserted by the producer's closed readback validation and
    // the required `nonblank_validation: valid` manifest field above. Xtask also
    // verifies this is a structurally complete PNG with image payload rather than
    // treating a nonempty or signature-only file as capture evidence.
    if !png_has_nonempty_image_data(&bytes)? {
        return Err("scene png has no image data".to_string());
    }
    Ok(())
}

fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.get(..8) != Some(PNG_SIGNATURE) || bytes.get(12..16) != Some(b"IHDR") {
        return Err("scene png has no valid PNG signature/IHDR".to_string());
    }
    let width = bytes
        .get(16..20)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map(u32::from_be_bytes)
        .unwrap_or(0);
    let height = bytes
        .get(20..24)
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map(u32::from_be_bytes)
        .unwrap_or(0);
    if width == 0 || height == 0 {
        return Err("scene png has an empty extent".to_string());
    }
    Ok((width, height))
}

fn png_has_nonempty_image_data(bytes: &[u8]) -> Result<bool, String> {
    let mut offset = 8_usize;
    let mut image_bytes = 0_u64;
    let mut ended = false;
    while offset < bytes.len() {
        let length_bytes = bytes
            .get(offset..offset + 4)
            .and_then(|value| <[u8; 4]>::try_from(value).ok())
            .ok_or_else(|| "scene png has a truncated chunk length".to_string())?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| "scene png chunk is too large".to_string())?;
        let kind = bytes
            .get(offset + 4..offset + 8)
            .ok_or_else(|| "scene png has a truncated chunk type".to_string())?;
        let next = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .ok_or_else(|| "scene png chunk length overflow".to_string())?;
        if next > bytes.len() {
            return Err("scene png has a truncated chunk".to_string());
        }
        if kind == b"IDAT" {
            image_bytes = image_bytes.saturating_add(length as u64);
        } else if kind == b"IEND" {
            ended = true;
            break;
        }
        offset = next;
    }
    Ok(ended && image_bytes > 0)
}

fn png_has_nonblank_pixels(bytes: &[u8]) -> Result<bool, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut decoder = png::Decoder::new(cursor);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("failed to decode diagnostic PNG metadata: {error}"))?;
    let buffer_size = reader
        .output_buffer_size()
        .ok_or_else(|| "diagnostic PNG decoded size exceeds platform limits".to_string())?;
    let mut pixels = vec![0; buffer_size];
    let output = reader
        .next_frame(&mut pixels)
        .map_err(|error| format!("failed to decode diagnostic PNG pixels: {error}"))?;
    let pixels = &pixels[..output.buffer_size()];
    let nonblank = match output.color_type {
        png::ColorType::Grayscale | png::ColorType::Rgb => {
            pixels.iter().any(|channel| *channel != 0)
        }
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .any(|pixel| pixel[1] != 0 && pixel[0] != 0),
        png::ColorType::Rgba => pixels
            .chunks_exact(4)
            .any(|pixel| pixel[3] != 0 && pixel[..3].iter().any(|channel| *channel != 0)),
        png::ColorType::Indexed => {
            return Err("diagnostic PNG palette did not expand during decode".to_string());
        }
    };
    Ok(nonblank)
}

fn validate_fault_soak_summary(summary: &serde_json::Value) -> Result<(), String> {
    let outcomes = summary
        .get("outcomes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "fault soak summary missing outcomes".to_string())?;
    if outcomes.len() != SCENE_FAULT_CASES.len() {
        return Err("fault soak summary does not cover every typed fault".to_string());
    }
    for case in SCENE_FAULT_CASES {
        let outcome = outcomes
            .iter()
            .find(|outcome| {
                outcome.get("injection").and_then(serde_json::Value::as_str) == Some(case.injection)
            })
            .ok_or_else(|| format!("fault soak missing `{}`", case.injection))?;
        for field in ["expected_category", "observed_category"] {
            if outcome.get(field).and_then(serde_json::Value::as_str)
                != Some(case.expected_category)
            {
                return Err(format!(
                    "fault soak {} {field} is not sanitized category {}",
                    case.injection, case.expected_category
                ));
            }
        }
        if outcome
            .get("sanitized")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Err(format!("fault soak {} was not sanitized", case.injection));
        }
        if outcome
            .get("expected_exit_success")
            .and_then(serde_json::Value::as_bool)
            != Some(case.expected_success)
            || outcome
                .get("process_status_matched")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
        {
            return Err(format!(
                "fault soak {} did not prove the expected process status",
                case.injection
            ));
        }
        let evidence = outcome
            .get("evidence")
            .ok_or_else(|| format!("fault soak {} missing evidence", case.injection))?;
        for field in [
            "render_log_valid",
            "diagnostic_appkit_screenshot_structurally_valid",
            "direct_artifacts_absent",
        ] {
            if evidence.get(field).and_then(serde_json::Value::as_bool) != Some(true) {
                return Err(format!(
                    "fault soak {} did not prove {field}",
                    case.injection
                ));
            }
        }
        if value_u64(evidence, "paint_frame_count")? == 0
            || value_u64(evidence, "frame_count")? == 0
            || evidence
                .get("runtime_metrics_present")
                .and_then(serde_json::Value::as_bool)
                != Some(case.produces_retained_host())
            || evidence
                .get("acknowledged_fallback_paint")
                .and_then(serde_json::Value::as_bool)
                != Some(!case.is_capture_failure())
            || evidence
                .get("capture_failure_observed")
                .and_then(serde_json::Value::as_bool)
                != Some(case.is_capture_failure())
        {
            return Err(format!(
                "fault soak {} evidence does not match the injected fault contract",
                case.injection
            ));
        }
        if !case.is_capture_failure() && value_u64(evidence, "nonblank_shape_frame_count")? == 0 {
            return Err(format!(
                "fault soak {} did not prove a nonblank fallback paint",
                case.injection
            ));
        }
        if !case.is_capture_failure()
            && evidence
                .get("diagnostic_appkit_screenshot_nonblank")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
        {
            return Err(format!(
                "fault soak {} did not prove decoded nonblank AppKit fallback pixels",
                case.injection
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateStatus {
    Pass,
    Fail,
}

impl GateStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BaselineGateResult {
    id: &'static str,
    status: GateStatus,
    measured: String,
    limit: String,
    disposition: String,
}

fn gate(
    id: &'static str,
    passed: bool,
    measured: impl Into<String>,
    limit: impl Into<String>,
) -> BaselineGateResult {
    BaselineGateResult {
        id,
        status: if passed {
            GateStatus::Pass
        } else {
            GateStatus::Fail
        },
        measured: measured.into(),
        limit: limit.into(),
        disposition: String::new(),
    }
}

fn evaluate_baseline_gates(
    snapshot: &serde_json::Value,
) -> Result<Vec<BaselineGateResult>, String> {
    evaluate_scene_gates(snapshot, true, true)
}

fn evaluate_lifetime_gates(
    snapshot: &serde_json::Value,
) -> Result<Vec<BaselineGateResult>, String> {
    // The direct lifetime protocol starts after launch activation has already
    // completed, so it cannot produce an activation render-owner sample. Keep
    // every other frozen gate that this process does exercise; the qualified
    // baseline run remains responsible for the activation slice.
    evaluate_scene_gates(snapshot, false, true)
}

fn evaluate_native_smoke_gates(
    snapshot: &serde_json::Value,
) -> Result<Vec<BaselineGateResult>, String> {
    // The real-time smoke exercises launch activation and every frozen runtime
    // percentile, but it does not request the virtual lifetime protocol.
    let mut gates = evaluate_scene_gates(snapshot, true, false)?;
    gates.push(gate(
        "terminal-capture",
        snapshot_u64(snapshot, &["capture_attempted"])? == 1
            && snapshot_u64(snapshot, &["capture_succeeded"])? == 1
            && snapshot_u64(snapshot, &["capture_failed"])? == 0
            && snapshot_u64(snapshot, &["capture_nonblank_validated"])? == 1,
        format!(
            "attempted={} succeeded={} failed={} nonblank={}",
            snapshot_u64(snapshot, &["capture_attempted"])?,
            snapshot_u64(snapshot, &["capture_succeeded"])?,
            snapshot_u64(snapshot, &["capture_failed"])?,
            snapshot_u64(snapshot, &["capture_nonblank_validated"])?,
        ),
        "exactly one attempted, successful, nonblank terminal GPU capture; zero failures",
    ));
    Ok(gates)
}

fn evaluate_scene_gates(
    snapshot: &serde_json::Value,
    require_activation_sample: bool,
    require_lifetime_audit: bool,
) -> Result<Vec<BaselineGateResult>, String> {
    const UI_P95_LIMIT_US: u64 = 1_422;
    const UI_P99_LIMIT_US: u64 = 2_070;
    const ENCODE_P95_LIMIT_US: u64 = 282;
    const GENERATION_SERVICE_UI_LIMIT_US: u64 = 4_000;
    const GPU_MATERIALIZE_PUBLISH_LIMIT_US: u64 = 16_000;
    const ACTIVATION_RENDER_OWNER_LIMIT_US: u64 = 16_000;
    const METRICS_OVERHEAD_LIMIT_NS: u64 = 28_440;
    let ui_p95 = snapshot_u64(snapshot, &["ui_tick_us", "p95"])?;
    let ui_p99 = snapshot_u64(snapshot, &["ui_tick_us", "p99"])?;
    let encode_p95 = snapshot_u64(snapshot, &["encode_us", "p95"])?;
    let main_thread_raster_calls = snapshot_u64(snapshot, &["main_thread_raster_calls"])?;
    let worker_raster_calls = snapshot_u64(snapshot, &["worker_raster_calls"])?;
    let worker_submissions = snapshot_u64(snapshot, &["worker_submissions"])?;
    let worker_completions = snapshot_u64(snapshot, &["worker_completions"])?;
    let gpu_materializations = snapshot_u64(snapshot, &["gpu_materializations"])?;
    let generation_count = snapshot_u64(snapshot, &["generation_count"])?;
    let generation_service_ui_max = snapshot_u64(snapshot, &["generation_service_ui_us", "max"])?;
    let gpu_materialize_publish_max =
        snapshot_u64(snapshot, &["gpu_materialize_publish_us", "max"])?;
    let activation_gate = require_activation_sample
        .then(|| {
            let activation_p95 = snapshot_u64(snapshot, &["activation_render_owner_us", "p95"])?;
            let activation_max = snapshot_u64(snapshot, &["activation_render_owner_us", "max"])?;
            Ok::<_, String>(gate(
                "activation-render-owner-slice",
                activation_p95 <= ACTIVATION_RENDER_OWNER_LIMIT_US
                    && activation_max <= ACTIVATION_RENDER_OWNER_LIMIT_US,
                format!("{activation_p95} us p95; {activation_max} us max"),
                format!("{ACTIVATION_RENDER_OWNER_LIMIT_US} us p95 and max"),
            ))
        })
        .transpose()?;
    let overhead_ns = snapshot_u64(snapshot, &["metrics_overhead_control", "net_ns_per_tick"])?;
    let hidden = snapshot
        .get("hidden_segment")
        .ok_or_else(|| "runtime snapshot missing hidden_segment".to_string())?;
    let hidden_delta = hidden
        .get("steady_delta")
        .ok_or_else(|| "runtime snapshot missing hidden_segment.steady_delta".to_string())?;
    let hidden_zero = [
        "prepare",
        "worker_submissions",
        "gpu_materializations",
        "queue_writes",
        "surface_acquires",
        "encode",
        "submit",
    ]
    .iter()
    .all(|field| value_u64(hidden_delta, field) == Ok(0));
    let hidden_ticks = value_u64(hidden, "steady_ticks")?;
    let mut gates = vec![
        gate(
            "ui-tick-p95",
            ui_p95 <= UI_P95_LIMIT_US,
            format!("{ui_p95} us"),
            format!("{UI_P95_LIMIT_US} us"),
        ),
        gate(
            "ui-tick-p99",
            ui_p99 <= UI_P99_LIMIT_US,
            format!("{ui_p99} us"),
            format!("{UI_P99_LIMIT_US} us"),
        ),
        gate(
            "encode-p95",
            encode_p95 <= ENCODE_P95_LIMIT_US,
            format!("{encode_p95} us"),
            format!("{ENCODE_P95_LIMIT_US} us"),
        ),
        gate(
            "main-thread-raster-calls",
            main_thread_raster_calls == 0,
            main_thread_raster_calls.to_string(),
            "0",
        ),
        gate(
            "worker-generation-evidence",
            worker_raster_calls > 0
                && worker_submissions > 0
                && worker_completions > 0
                && gpu_materializations > 0
                && generation_count > 0
                && gpu_materializations == generation_count
                && worker_completions >= generation_count,
            format!(
                "raster_calls={worker_raster_calls} submissions={worker_submissions} completions={worker_completions} materializations={gpu_materializations} generations={generation_count}"
            ),
            "all counts > 0; materializations = generations; completions >= generations",
        ),
        gate(
            "generation-service-ui-max",
            generation_service_ui_max <= GENERATION_SERVICE_UI_LIMIT_US,
            format!("{generation_service_ui_max} us"),
            format!("{GENERATION_SERVICE_UI_LIMIT_US} us"),
        ),
        gate(
            "gpu-materialize-publish-max",
            gpu_materialize_publish_max <= GPU_MATERIALIZE_PUBLISH_LIMIT_US,
            format!("{gpu_materialize_publish_max} us"),
            format!("{GPU_MATERIALIZE_PUBLISH_LIMIT_US} us"),
        ),
        gate(
            "metrics-overhead-control",
            overhead_ns <= METRICS_OVERHEAD_LIMIT_NS,
            format!("{overhead_ns} ns/tick"),
            format!("{METRICS_OVERHEAD_LIMIT_NS} ns/tick"),
        ),
        gate(
            "hidden-steady-state",
            hidden_ticks >= 2 && hidden_zero,
            format!("{hidden_ticks} steady ticks; delta={hidden_delta}"),
            "after 1 transition tick, >=2 steady ticks and all work counters 0",
        ),
    ];
    if let Some(activation_gate) = activation_gate {
        gates.insert(7, activation_gate);
    }
    if !require_lifetime_audit {
        return Ok(gates);
    }

    let lifetime = snapshot
        .get("lifetime_audit")
        .ok_or_else(|| "runtime snapshot missing lifetime_audit".to_string())?;
    let rss_warmup = value_u64(lifetime, "rss_warmup_bytes")?;
    let rss_warmup_peak = value_u64(lifetime, "rss_warmup_peak_bytes")?;
    let rss_final = value_u64(lifetime, "rss_final_bytes")?;
    let rss_peak = value_u64(lifetime, "rss_peak_bytes")?;
    let gpu_warmup = value_u64(lifetime, "gpu_warmup_bytes")?;
    let gpu_warmup_peak = value_u64(lifetime, "gpu_warmup_peak_bytes")?;
    let gpu_final = value_u64(lifetime, "gpu_final_bytes")?;
    let gpu_peak = value_u64(lifetime, "gpu_peak_bytes")?;
    let within_one_percent = |value: u64, warmup: u64| {
        warmup > 0 && value <= warmup.saturating_add(warmup.div_ceil(100))
    };

    gates.extend([
        gate(
            "post-warmup-persistent-gpu-creations",
            value_u64(lifetime, "post_warmup_resource_creations")? == 0,
            value_u64(lifetime, "post_warmup_resource_creations")?.to_string(),
            "0",
        ),
        gate(
            "post-warmup-static-upload-bytes",
            value_u64(lifetime, "post_warmup_static_upload_bytes")? == 0,
            value_u64(lifetime, "post_warmup_static_upload_bytes")?.to_string(),
            "0",
        ),
        gate(
            "lifetime-frame-count-and-cadence",
            value_u64(lifetime, "semantic_samples")? == 4_500
                && value_u64(lifetime, "warmup_semantic_samples")? == 4_500
                && value_u64(lifetime, "presentation_ticks")? == 33_750
                && value_u64(lifetime, "warmup_presentation_ticks")? == 33_750
                && value_u64(lifetime, "semantic_cadence_ms")? == 250
                && value_u64(lifetime, "presentation_cadence_hz")? == 30
                && value_u64(lifetime, "virtual_elapsed_ms")? == 1_125_000,
            format!(
                "{} warmup + {} measured semantic samples @ {} ms; {} warmup + {} measured presentation ticks @ {} Hz; {} ms elapsed",
                value_u64(lifetime, "warmup_semantic_samples")?,
                value_u64(lifetime, "semantic_samples")?,
                value_u64(lifetime, "semantic_cadence_ms")?,
                value_u64(lifetime, "warmup_presentation_ticks")?,
                value_u64(lifetime, "presentation_ticks")?,
                value_u64(lifetime, "presentation_cadence_hz")?,
                value_u64(lifetime, "virtual_elapsed_ms")?,
            ),
            "identical 4500-sample/33750-tick warmup + measured schedules @ 4 Hz/30 Hz; 1125000 ms elapsed",
        ),
        gate(
            "lifetime-production-work",
            value_u64(lifetime, "snapshot_projections")? == 4_500
                && value_u64(lifetime, "semantic_reconciles")? == 4_500
                && value_u64(lifetime, "frame_projections")? == 33_750
                && value_u64(lifetime, "frame_reconciles")? == 33_750
                && value_u64(lifetime, "encoded_ticks")? == 33_750
                && value_u64(lifetime, "submitted_ticks")? == 33_750
                && value_u64(lifetime, "draw_calls")? > 0
                && value_u64(lifetime, "poll_count")? > 0,
            format!(
                "snapshots={} semantic-reconciles={} frame-projections={} frame-reconciles={} encoded={} submitted={} draws={} polls={}",
                value_u64(lifetime, "snapshot_projections")?,
                value_u64(lifetime, "semantic_reconciles")?,
                value_u64(lifetime, "frame_projections")?,
                value_u64(lifetime, "frame_reconciles")?,
                value_u64(lifetime, "encoded_ticks")?,
                value_u64(lifetime, "submitted_ticks")?,
                value_u64(lifetime, "draw_calls")?,
                value_u64(lifetime, "poll_count")?,
            ),
            "4500 actual semantic projections/reconciles, 33750 frame projections/reconciles/encodes/submits, nonzero draws/polls",
        ),
        gate(
            "terminal-capture",
            snapshot_u64(snapshot, &["capture_attempted"])? == 1
                && snapshot_u64(snapshot, &["capture_succeeded"])? == 1
                && snapshot_u64(snapshot, &["capture_failed"])? == 0,
            format!(
                "attempted={} succeeded={} failed={}",
                snapshot_u64(snapshot, &["capture_attempted"])? ,
                snapshot_u64(snapshot, &["capture_succeeded"])? ,
                snapshot_u64(snapshot, &["capture_failed"])? ,
            ),
            "exactly one attempted and successful terminal GPU capture; zero failures",
        ),
        gate(
            "lifetime-rss",
            within_one_percent(rss_final, rss_warmup_peak)
                && within_one_percent(rss_peak, rss_warmup_peak),
            format!("warmup-end={rss_warmup} warmup-high-water={rss_warmup_peak} final={rss_final} peak={rss_peak}"),
            "final and peak <= warmup high-water + 1%",
        ),
        gate(
            "lifetime-accounted-gpu",
            within_one_percent(gpu_final, gpu_warmup_peak)
                && within_one_percent(gpu_peak, gpu_warmup_peak),
            format!("warmup-end={gpu_warmup} warmup-high-water={gpu_warmup_peak} final={gpu_final} peak={gpu_peak}"),
            "final and peak <= warmup high-water + 1%",
        ),
    ]);
    Ok(gates)
}

fn validate_gate_results(gates: &[BaselineGateResult]) -> Result<(), String> {
    let failed = gates
        .iter()
        .filter(|gate| gate.status == GateStatus::Fail)
        .map(|gate| gate.id)
        .collect::<Vec<_>>();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "scene baseline gate failures: {}",
            failed.join(", ")
        ))
    }
}

fn render_scene_baseline_report(
    repo_root: &Path,
    duration_ms: u64,
    snapshot: &serde_json::Value,
    source: &BaselineSourceIdentity,
    gates: &[BaselineGateResult],
) -> Result<String, String> {
    let rustc = required_command_output(repo_root, "rustc", &["--version"])?;
    let os = required_command_output(repo_root, "sw_vers", &["-productVersion"])?;
    let arch = required_command_output(repo_root, "uname", &["-m"])?;
    let hardware = command_output(repo_root, "sysctl", &["-n", "hw.model"])
        .unwrap_or_else(|| "unavailable".to_string());
    let inventory = snapshot
        .get("inventory")
        .ok_or_else(|| "runtime snapshot missing inventory".to_string())?;
    let fixture = snapshot
        .get("fixture")
        .ok_or_else(|| "runtime snapshot missing fixture".to_string())?;
    let lifetime = snapshot
        .get("lifetime_audit")
        .ok_or_else(|| "runtime snapshot missing lifetime_audit".to_string())?;
    let gpu = snapshot
        .get("gpu_accounting")
        .ok_or_else(|| "runtime snapshot missing gpu_accounting".to_string())?;
    let current_gpu = gpu
        .get("current_bytes")
        .ok_or_else(|| "runtime snapshot missing gpu_accounting.current_bytes".to_string())?;
    let current_gpu_objects = gpu
        .get("current_objects")
        .ok_or_else(|| "runtime snapshot missing gpu_accounting.current_objects".to_string())?;
    let overhead = snapshot
        .get("metrics_overhead_control")
        .ok_or_else(|| "runtime snapshot missing metrics_overhead_control".to_string())?;
    let gate_rows = gates
        .iter()
        .map(|gate| {
            format!(
                "| `{}` | {} | {} | {} | {} |",
                gate.id,
                gate.status.as_str(),
                gate.measured,
                gate.limit,
                if gate.disposition.is_empty() {
                    "—"
                } else {
                    &gate.disposition
                },
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let capacity_rows = [
        ("Prepared legacy GPU primitives", "max_prepared_gpu_primitives", "observed by compiling every production-prepared matrix frame through the retained GPU translator"),
        ("Nodes", "max_nodes", "Task 2 scene nodes unavailable; versioned contract reservation"),
        ("Static primitives", "max_static_primitives", "Task 2 scene primitives unavailable; versioned contract reservation"),
        ("Pet art slots", "max_pet_slots", "observed across 6 species x 7 stages x 5 states x 3 depths"),
        ("Visible props", "max_visible_props", "observed from complete habitat prop catalog through current 6 trophy + 4 accent selectors"),
        ("Round tank inhabitants", "max_round_tank_inhabitants", "observed from complete tank cast through current round surface budget"),
        ("Ambient instances", "max_ambient_instances", "Task 2 scene ambient instances unavailable; versioned contract reservation"),
        ("Blended draws", "max_blended_draws", "Task 2 ordered blend records unavailable; versioned contract reservation"),
        ("Lights", "max_lights", "Task 2 scene lights unavailable; versioned contract reservation"),
        ("Attachments", "max_attachments", "Task 2 scene attachments unavailable; versioned contract reservation"),
    ]
    .iter()
    .map(|(label, field, source_label)| {
        let contract = inventory.get(*field).expect("validated capacity field");
        let observed = contract.get("observed").and_then(serde_json::Value::as_u64)
            .map_or_else(|| "—".to_string(), |value| value.to_string());
        format!(
            "| {label} | {observed} | {} | {} | {} | {source_label} |",
            value_u64(contract, "reservation").expect("validated reservation"),
            value_u64(contract, "headroom").expect("validated headroom"),
            value_u64(contract, "limit").expect("validated limit"),
        )
    })
    .collect::<Vec<_>>()
    .join("\n");

    Ok(format!(
        "# Glorp Companion Scene Runtime Baseline\n\n\
Generated by `cargo xtask companion scene-baseline` from a release `retained-renderer` build. \
The command rejects tracked source changes before build. The fixture uses an isolated Glorp config, \
a fixed initial update source with live polling disabled, deterministic seed, redacted HUD, and a fixed 4 Hz cadence. \
The first 20 visible ticks are discarded before steady-state sampling.\n\n\
## Build and host identity\n\n\
- Source commit captured before build: `{}`\n\
- Tracked tree state captured before build: `{}`\n\
- Rust: `{rustc}`\n\
- macOS: `{os}`\n\
- Architecture: `{arch}`\n\
- Hardware model: `{hardware}`\n\
- Build: `release`, feature `retained-renderer`\n\
- Requested duration: {duration_ms} ms\n\
- Fixture: `{}`; seed: `{}`; update source: `{}`\n\
- Cadence: {} ms (4 Hz)\n\
- Logical size: {} x {}; physical size: {} x {}; backing scale: {}\n\
- Steady visible samples: {}\n\n\
## Measured baseline\n\n\
| Metric | p50 (us) | p95 (us) | p99 (us) |\n\
|---|---:|---:|---:|\n\
| UI tick | {} | {} | {} |\n\
| App state/frame preparation | {} | {} | {} |\n\
| GPU translation | {} | {} | {} |\n\
| Encode | {} | {} | {} |\n\
| Queue submit wait | {} | {} | {} |\n\
| Worker active compile | {} | {} | {} |\n\
| Raster request wall time | {} | {} | {} |\n\
| Generation-service UI work | {} | {} | {} |\n\
| GPU materialize/upload/publish | {} | {} | {} |\n\
| First-present activation render-owner boundary | {} | {} | {} |\n\n\
Observed maxima: generation-service UI work {} us; GPU materialize/upload/publish {} us; first-present activation render-owner boundary {} us.\n\n\
Metrics overhead uses {} alternating on/off-control trials of {} representative complete metric ticks ({} control / {} instrumented); \
control {} ns/tick, instrumented {} ns/tick, net {} ns/tick.\n\n\
Accounted persistent GPU bytes: atlas {}, instance ring {}, capture {}, current total {}, concurrent-replacement peak {}. \
Persistent GPU objects: host {}, atlas {}, instance ring {}, capture {}, current total {}, concurrent-replacement peak {}; lifecycle created/destroyed {}/{}. \
Opaque driver allocations are covered by process RSS, not guessed into GPU byte accounting.\n\n\
	Lifetime segment: {} warmup + {} measured semantic samples at {} ms plus {} warmup + {} measured presentation ticks at {} Hz ({} ms semantic time); \
	semantic projections/reconciles {}/{}, frame projections/reconciles {}/{}, encoded {}, submitted {}, draws {}, polls {}; \
RSS warmup-end/warmup-high-water/final/peak {}/{}/{}/{}, accounted GPU warmup-end/warmup-high-water/final/peak {}/{}/{}/{}.\n\n\
Main-thread/worker raster calls: {}/{}. GPU materializations: {}.\n\
Worker submissions/completions/cancellations/coalesces/stale rejections/failures: {}/{}/{}/{}/{}/{}.\n\n\
## Structured gate results\n\n\
| Gate | Status | Measured | Limit | Disposition |\n\
|---|---|---|---|---|\n\
{gate_rows}\n\n\
Raster work is worker-owned: main-thread raster calls must remain zero. Worker compile and request wall time are diagnostic only; the UI-safety gates apply to generation-service UI work and render-owner GPU materialize/upload/publish maxima.\n\n\
## Capacity inventory\n\n\
| Capacity | Observed | Reservation | Headroom | Limit | Evidence |\n\
|---|---:|---:|---:|---:|---|\n\
{capacity_rows}\n\n\
Matrix construction prepared {} actual frames, including {} dimmed frames and {} frames carrying the full prop catalog plus full tank cast. \
An em dash means the future renderer-neutral category does not exist yet and is represented by an explicit contract reservation. Each observed value plus reservation plus explicit headroom fits its frozen limit. Zero headroom is intentional for the already-full pet lattice, visible prop budget, and round tank cast; expanding any requires measured evidence and a spec amendment.\n",
        source.commit,
        source.tracked_tree_state,
        fixture["fixture_id"].as_str().unwrap_or("missing"),
        fixture["seed"].as_str().unwrap_or("missing"),
        fixture["update_source"].as_str().unwrap_or("missing"),
        fixture_semantic_cadence_ms(fixture)?,
        fixture["logical_width"].as_f64().ok_or("missing logical_width")?,
        fixture["logical_height"].as_f64().ok_or("missing logical_height")?,
        value_u64(fixture, "physical_width")?,
        value_u64(fixture, "physical_height")?,
        fixture["backing_scale"].as_f64().ok_or("missing backing_scale")?,
        snapshot_u64(snapshot, &["visible_samples"])? ,
        snapshot_u64(snapshot, &["ui_tick_us", "p50"])? ,
        snapshot_u64(snapshot, &["ui_tick_us", "p95"])? ,
        snapshot_u64(snapshot, &["ui_tick_us", "p99"])? ,
        snapshot_u64(snapshot, &["state_prepare_us", "p50"])? ,
        snapshot_u64(snapshot, &["state_prepare_us", "p95"])? ,
        snapshot_u64(snapshot, &["state_prepare_us", "p99"])? ,
        snapshot_u64(snapshot, &["gpu_translate_us", "p50"])? ,
        snapshot_u64(snapshot, &["gpu_translate_us", "p95"])? ,
        snapshot_u64(snapshot, &["gpu_translate_us", "p99"])? ,
        snapshot_u64(snapshot, &["encode_us", "p50"])? ,
        snapshot_u64(snapshot, &["encode_us", "p95"])? ,
        snapshot_u64(snapshot, &["encode_us", "p99"])? ,
        snapshot_u64(snapshot, &["queue_wait_us", "p50"])? ,
        snapshot_u64(snapshot, &["queue_wait_us", "p95"])? ,
        snapshot_u64(snapshot, &["queue_wait_us", "p99"])? ,
        snapshot_u64(snapshot, &["worker_active_compile_us", "p50"])? ,
        snapshot_u64(snapshot, &["worker_active_compile_us", "p95"])? ,
        snapshot_u64(snapshot, &["worker_active_compile_us", "p99"])? ,
        snapshot_u64(snapshot, &["raster_request_wall_us", "p50"])? ,
        snapshot_u64(snapshot, &["raster_request_wall_us", "p95"])? ,
        snapshot_u64(snapshot, &["raster_request_wall_us", "p99"])? ,
        snapshot_u64(snapshot, &["generation_service_ui_us", "p50"])? ,
        snapshot_u64(snapshot, &["generation_service_ui_us", "p95"])? ,
        snapshot_u64(snapshot, &["generation_service_ui_us", "p99"])? ,
        snapshot_u64(snapshot, &["gpu_materialize_publish_us", "p50"])? ,
        snapshot_u64(snapshot, &["gpu_materialize_publish_us", "p95"])? ,
        snapshot_u64(snapshot, &["gpu_materialize_publish_us", "p99"])? ,
        snapshot_u64(snapshot, &["activation_render_owner_us", "p50"])? ,
        snapshot_u64(snapshot, &["activation_render_owner_us", "p95"])? ,
        snapshot_u64(snapshot, &["activation_render_owner_us", "p99"])? ,
        snapshot_u64(snapshot, &["generation_service_ui_us", "max"])? ,
        snapshot_u64(snapshot, &["gpu_materialize_publish_us", "max"])? ,
        snapshot_u64(snapshot, &["activation_render_owner_us", "max"])? ,
        value_u64(overhead, "trials")?,
        value_u64(overhead, "iterations")?,
        value_u64(overhead, "control_ticks")?,
        value_u64(overhead, "instrumented_ticks")?,
        value_u64(overhead, "control_ns_per_tick")?,
        value_u64(overhead, "instrumented_ns_per_tick")?,
        value_u64(overhead, "net_ns_per_tick")?,
        value_u64(current_gpu, "atlas_bytes")?,
        value_u64(current_gpu, "instance_ring_bytes")?,
        value_u64(current_gpu, "capture_bytes")?,
        value_u64(current_gpu, "total_bytes")?,
        value_u64(gpu, "peak_total_bytes")?,
        value_u64(current_gpu_objects, "host_infrastructure")?,
        value_u64(current_gpu_objects, "atlas")?,
        value_u64(current_gpu_objects, "instance_ring")?,
        value_u64(current_gpu_objects, "capture")?,
        value_u64(current_gpu_objects, "total_objects")?,
        value_u64(gpu, "peak_total_objects")?,
        value_u64(gpu, "objects_created_total")?,
        value_u64(gpu, "objects_destroyed_total")?,
	        value_u64(lifetime, "warmup_semantic_samples")?,
	        value_u64(lifetime, "semantic_samples")?,
	        value_u64(lifetime, "semantic_cadence_ms")?,
	        value_u64(lifetime, "warmup_presentation_ticks")?,
	        value_u64(lifetime, "presentation_ticks")?,
	        value_u64(lifetime, "presentation_cadence_hz")?,
        value_u64(lifetime, "virtual_elapsed_ms")?,
	        value_u64(lifetime, "snapshot_projections")?,
	        value_u64(lifetime, "semantic_reconciles")?,
	        value_u64(lifetime, "frame_projections")?,
	        value_u64(lifetime, "frame_reconciles")?,
	        value_u64(lifetime, "encoded_ticks")?,
	        value_u64(lifetime, "submitted_ticks")?,
        value_u64(lifetime, "draw_calls")?,
        value_u64(lifetime, "poll_count")?,
        value_u64(lifetime, "rss_warmup_bytes")?,
        value_u64(lifetime, "rss_warmup_peak_bytes")?,
        value_u64(lifetime, "rss_final_bytes")?,
        value_u64(lifetime, "rss_peak_bytes")?,
        value_u64(lifetime, "gpu_warmup_bytes")?,
        value_u64(lifetime, "gpu_warmup_peak_bytes")?,
        value_u64(lifetime, "gpu_final_bytes")?,
        value_u64(lifetime, "gpu_peak_bytes")?,
        snapshot_u64(snapshot, &["main_thread_raster_calls"])? ,
        snapshot_u64(snapshot, &["worker_raster_calls"])? ,
        snapshot_u64(snapshot, &["gpu_materializations"])? ,
        snapshot_u64(snapshot, &["worker_submissions"])? ,
        snapshot_u64(snapshot, &["worker_completions"])? ,
        snapshot_u64(snapshot, &["worker_cancellations"])? ,
        snapshot_u64(snapshot, &["worker_coalesces"])? ,
        snapshot_u64(snapshot, &["worker_stale_rejections"])? ,
        snapshot_u64(snapshot, &["worker_failures"])? ,
        value_u64(inventory, "matrix_fixture_count")?,
        value_u64(inventory, "dimmed_fixture_count")?,
        value_u64(inventory, "full_props_tank_fixture_count")?,
    ))
}

fn snapshot_u64(value: &serde_json::Value, path: &[&str]) -> Result<u64, String> {
    let mut current = value;
    for component in path {
        current = current
            .get(*component)
            .ok_or_else(|| format!("runtime snapshot missing {}", path.join(".")))?;
    }
    current.as_u64().ok_or_else(|| {
        format!(
            "runtime snapshot {} is not an unsigned integer",
            path.join(".")
        )
    })
}

fn value_u64(value: &serde_json::Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("runtime snapshot missing {field}"))
}

fn required_command_output(
    repo_root: &Path,
    program: &str,
    args: &[&str],
) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo_root)
        .output()
        .map_err(|error| format!("failed to run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed with {}",
            args.join(" "),
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Validates the paired-review manifest and both PNG artifacts. Rejects a missing
/// or non-success manifest so a successful process launch cannot hide an app-side
/// capture failure.
fn validate_pair_manifest(repo_root: &Path, out: &str) -> Result<(), String> {
    let root = repo_root.join(out);
    let manifest_path = root.join("pair-manifest.json");
    if !manifest_path.is_file() {
        return Err(format!(
            "paired review missing manifest: {}",
            manifest_path.display()
        ));
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|err| err.to_string())?)
            .map_err(|err| format!("failed to parse pair manifest: {err}"))?;
    validate_pair_manifest_value(&manifest)?;
    for section in ["smooth", "retained"] {
        let png = manifest
            .get(section)
            .and_then(|value| value.get("png_path"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("pair manifest {section} section has no png path"))?;
        let bytes = std::fs::metadata(root.join(png))
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if bytes == 0 {
            return Err(format!("paired review artifact missing or empty: {png}"));
        }
    }
    Ok(())
}

/// The manifest contract xtask enforces: a successful pair whose retained half is
/// a genuine retained capture that observed both the GPU and readback milestones,
/// paired with a Smooth half. Mirrors the main crate's `validate_review_pair`
/// against the untyped manifest JSON so xtask stays free of the GPU crate deps.
fn validate_pair_manifest_value(manifest: &serde_json::Value) -> Result<(), String> {
    let status = manifest.get("status").and_then(serde_json::Value::as_str);
    if status != Some("success") {
        return Err(format!(
            "paired capture status is {}",
            status.unwrap_or("missing")
        ));
    }
    let retained = manifest
        .get("retained")
        .ok_or_else(|| "pair manifest has no retained section".to_string())?;
    let retained_effective = retained
        .get("effective_renderer")
        .and_then(serde_json::Value::as_str);
    if retained_effective == Some("smooth") {
        return Err("retained capture fell back to the smooth renderer".to_string());
    }
    if retained_effective != Some("retained") {
        return Err("retained section is not the retained renderer".to_string());
    }
    let milestones = retained
        .get("milestones")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "retained section has no milestones".to_string())?;
    let observed = |milestone: &str| {
        milestones
            .iter()
            .any(|value| value.as_str() == Some(milestone))
    };
    if !observed("readback-completed") {
        return Err("retained capture missing readback-completed".to_string());
    }
    if !observed("gpu-completed") {
        return Err("retained capture missing gpu-completed".to_string());
    }
    let smooth_effective = manifest
        .get("smooth")
        .and_then(|section| section.get("effective_renderer"))
        .and_then(serde_json::Value::as_str);
    if smooth_effective != Some("smooth") {
        return Err("smooth section is not the smooth renderer".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_renderer_spike_qualification(
    repo_root: &Path,
    binary: &str,
    target: &str,
    candidate: &str,
    track: &str,
    size: u16,
    duration_ms: u64,
    out: &str,
) -> Result<(), String> {
    if std::env::consts::OS != "macos" {
        return Err("cargo xtask renderer-spike qualify is only supported on macOS".to_string());
    }
    let binary_path = repo_root.join(binary);
    if !binary_path.is_file() {
        return Err(format!("missing frozen renderer spike binary: {binary}"));
    }
    let out_path = repo_root.join(out);
    if out_path.exists() {
        return Err(format!("refusing to overwrite qualification output: {out}"));
    }
    std::fs::create_dir_all(&out_path).map_err(|err| err.to_string())?;
    let bytes = std::fs::read(&binary_path).map_err(|err| err.to_string())?;
    let metadata = std::fs::metadata(&binary_path).map_err(|err| err.to_string())?;
    let modified_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64);
    let command = vec![
        binary.to_string(),
        "renderer-spike-app".to_string(),
        "--candidate".to_string(),
        candidate.to_string(),
        "--track".to_string(),
        track.to_string(),
        "--logical-size".to_string(),
        size.to_string(),
        "--duration-ms".to_string(),
        duration_ms.to_string(),
        "--out".to_string(),
        out.to_string(),
    ];
    let protocol = serde_json::json!({
        "schema_version": 1,
        "run_id": out_path.file_name().and_then(|name| name.to_str()).unwrap_or("qualification"),
        "command": command,
        "binary": {
            "path": binary,
            "bytes": bytes.len(),
            "sha256": format!("{:x}", Sha256::digest(&bytes)),
            "mtime_unix_ms": modified_unix_ms,
            "target": target,
            "profile": "release",
            "features": ["renderer-spike", "renderer-spike-wgpu"],
            "wgpu": {"version": "30.0.0", "features": ["metal", "std", "wgsl"]},
        },
        "repository": {
            "commit": command_output(repo_root, "git", &["rev-parse", "HEAD"]),
            "dirty": command_output(repo_root, "git", &["status", "--porcelain"])
                .is_some_and(|value| !value.trim().is_empty()),
        },
        "host": {
            "rustc": command_output(repo_root, "rustc", &["-vV"]),
            "frontmost": command_output(repo_root, "lsappinfo", &["front"]),
            "power_source": command_output(repo_root, "pmset", &["-g", "batt"]),
            "display": display_identity(repo_root),
        },
        "matched_protocol": {
            "sizes": [360, 720],
            "rotation": [["smooth", "wgpu"], ["wgpu", "smooth"], ["smooth", "wgpu"]],
            "warmup_seconds": 30,
            "measurement_seconds": 300,
            "sample_interval_seconds": 1,
            "samples_per_run": 300,
            "cooldown_seconds": 30,
            "poll_window_exclusion": null,
            "poll_exclusion_reason": "synthetic fixture performs no usage polls",
            "p95_method": "nearest-rank",
            "missed_frame_denominator": "requested_visible_frames",
            "maximum_run_median_divergence_percent": 20,
        },
    });
    std::fs::write(
        out_path.join("qualification-protocol.json"),
        serde_json::to_vec_pretty(&protocol).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    run_bounded_process_with_env(
        &binary_path,
        &command[1..],
        repo_root,
        Duration::from_millis(duration_ms.saturating_add(30_000)),
        &[(
            "GLORP_RENDERER_SPIKE_RUNNER_ENTRY_MICROS",
            monotonic_micros().to_string(),
        )],
    )?;
    validate_renderer_spike(repo_root, out)
}

fn command_output(repo_root: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn display_identity(repo_root: &Path) -> serde_json::Value {
    let Some(raw) = command_output(
        repo_root,
        "system_profiler",
        &["SPDisplaysDataType", "-json"],
    ) else {
        return serde_json::Value::Null;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return serde_json::Value::Null;
    };
    let displays = value
        .get("SPDisplaysDataType")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|gpu| {
            gpu.get("spdisplays_ndrvs")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
        })
        .map(|display| {
            serde_json::json!({
                "name": display.get("_name"),
                "resolution": display.get("_spdisplays_resolution"),
                "main": display.get("spdisplays_main"),
                "retina": display.get("spdisplays_retina"),
                "backing_scale": display.get("spdisplays_retina")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| *value == "spdisplays_yes")
                    .map(|_| 2.0),
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(displays)
}

fn validate_renderer_spike(repo_root: &Path, out: &str) -> Result<(), String> {
    let root = repo_root.join(out);
    let manifest = root.join("run-manifest.json");
    let cleanup = root.join("process-cleanup.json");
    let privacy = root.join("privacy-scan.json");
    for path in [&manifest, &cleanup, &privacy] {
        if !path.is_file() {
            return Err(format!(
                "renderer spike missing required artifact: {}",
                path.display()
            ));
        }
    }
    let cleanup_text = std::fs::read_to_string(cleanup).map_err(|err| err.to_string())?;
    if !cleanup_text.contains("\"process_exited\": true")
        || !cleanup_text.contains("\"surviving_pids\": []")
    {
        return Err("renderer spike process cleanup did not pass".to_string());
    }
    let privacy_text = std::fs::read_to_string(privacy).map_err(|err| err.to_string())?;
    if !privacy_text.contains("\"passed\": true") {
        return Err("renderer spike privacy scan did not pass".to_string());
    }
    let manifest_value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&manifest).map_err(|err| format!("failed to read manifest: {err}"))?,
    )
    .map_err(|err| format!("failed to parse manifest: {err}"))?;
    let artifacts = manifest_value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "renderer spike manifest has no artifact list".to_string())?;
    for artifact in artifacts {
        let relative = artifact
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "renderer spike artifact has no path".to_string())?;
        let expected_bytes = artifact
            .get("bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("renderer spike artifact `{relative}` has no byte count"))?;
        let expected_hash = artifact
            .get("sha256")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("renderer spike artifact `{relative}` has no hash"))?;
        let bytes = std::fs::read(root.join(relative))
            .map_err(|err| format!("failed to read renderer spike artifact `{relative}`: {err}"))?;
        if bytes.len() as u64 != expected_bytes {
            return Err(format!("renderer spike artifact byte mismatch: {relative}"));
        }
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));
        if actual_hash != expected_hash {
            return Err(format!("renderer spike artifact hash mismatch: {relative}"));
        }
    }
    validate_software_artifacts(&root)?;
    validate_corrected_wgpu_uploads(&root)?;
    Ok(())
}

fn validate_corrected_wgpu_uploads(root: &Path) -> Result<(), String> {
    let environment: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("environment.json")).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    if environment
        .get("candidate")
        .and_then(serde_json::Value::as_str)
        != Some("wgpu")
    {
        return Ok(());
    }
    validate_wgpu_startup_artifact(root)?;
    validate_wgpu_accessibility_artifact(root)?;
    let metrics =
        std::fs::read_to_string(root.join("frame-metrics.jsonl")).map_err(|err| err.to_string())?;
    let mut static_upload_seen = false;
    for (line_index, line) in metrics.lines().enumerate() {
        let metric: serde_json::Value = serde_json::from_str(line)
            .map_err(|err| format!("invalid wgpu frame metric line {}: {err}", line_index + 1))?;
        let Some(static_bytes) = metric
            .get("static_upload_bytes")
            .and_then(serde_json::Value::as_u64)
        else {
            // Preserve validation of historical pre-correction evidence. Final
            // qualification roots are required to carry the corrected fields.
            return Ok(());
        };
        let dynamic_bytes = metric
            .get("dynamic_upload_bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "wgpu metric has no dynamic upload byte count".to_string())?;
        let atlas_bytes = metric
            .get("atlas_upload_bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "wgpu metric has no atlas upload byte count".to_string())?;
        let uniform_bytes = metric
            .get("uniform_upload_bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "wgpu metric has no uniform upload byte count".to_string())?;
        let total_bytes = metric
            .get("upload_bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "wgpu metric has no total upload byte count".to_string())?;
        let draw_calls = metric
            .get("draw_calls")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "wgpu metric has no draw-call count".to_string())?;
        if total_bytes
            != static_bytes
                .saturating_add(dynamic_bytes)
                .saturating_add(atlas_bytes)
                .saturating_add(uniform_bytes)
        {
            return Err(format!(
                "wgpu upload total mismatch on metric line {}",
                line_index + 1
            ));
        }
        if draw_calls == 0 {
            if total_bytes != 0 {
                return Err(format!(
                    "wgpu skipped frame uploaded bytes on metric line {}",
                    line_index + 1
                ));
            }
            continue;
        }
        if draw_calls != 1 || dynamic_bytes != 2_400 || uniform_bytes != 0 {
            return Err(format!(
                "wgpu bounded frame upload contract failed on metric line {}",
                line_index + 1
            ));
        }
        if static_bytes != 0 {
            if static_upload_seen || static_bytes != 19_200 {
                return Err(format!(
                    "wgpu static source/style upload repeated or changed on metric line {}",
                    line_index + 1
                ));
            }
            static_upload_seen = true;
            if atlas_bytes != 256 || total_bytes != 21_856 {
                return Err(format!(
                    "wgpu activation upload accounting failed on metric line {}",
                    line_index + 1
                ));
            }
        }
        if !matches!(atlas_bytes, 0 | 256 | 512) {
            return Err(format!(
                "wgpu semantic atlas upload is not a bounded 16-slot update on metric line {}",
                line_index + 1
            ));
        }
    }
    if !metrics.trim().is_empty() && !static_upload_seen {
        return Err("wgpu metrics contain no immutable source/style activation upload".to_string());
    }
    Ok(())
}

fn validate_wgpu_accessibility_artifact(root: &Path) -> Result<(), String> {
    let audit: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("accessibility-audit.json")).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    let summary: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("summary.json")).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    let early_surface_fault = summary.get("verdict").and_then(serde_json::Value::as_str)
        == Some("reject-injected-surface-unavailable");
    let expected_true = [
        "sanitized",
        "synthetic_mouse_event_delivered",
        "synthetic_key_event_delivered",
        "stale_snapshot_rejected",
        "current_snapshot_accepted",
        "first_responder",
        "close_children_detached",
    ];
    if audit.get("group_count").and_then(serde_json::Value::as_u64) != Some(1)
        || audit.get("value_count").and_then(serde_json::Value::as_u64) != Some(3)
        || audit.get("child_count").and_then(serde_json::Value::as_u64) != Some(4)
        || audit
            .get("per_glyph_children")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
        || audit.get("sanitized").and_then(serde_json::Value::as_bool) != Some(true)
        || (!early_surface_fault
            && expected_true
                .iter()
                .any(|field| audit.get(*field).and_then(serde_json::Value::as_bool) != Some(true)))
    {
        return Err("wgpu automated accessibility/input audit did not pass".to_string());
    }
    let environment: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("environment.json")).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    if environment.get("track").and_then(serde_json::Value::as_str) == Some("occlusion")
        && (audit
            .get("hide_children_detached")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
            || audit
                .get("reveal_children_restored")
                .and_then(serde_json::Value::as_bool)
                != Some(true))
    {
        return Err("wgpu accessibility children did not cleanly hide/reveal".to_string());
    }
    Ok(())
}

fn validate_wgpu_startup_artifact(root: &Path) -> Result<(), String> {
    let startup: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("startup.json")).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    if startup.get("clock").and_then(serde_json::Value::as_str)
        != Some("mach-continuous-time-micros")
    {
        return Err("wgpu startup artifact has the wrong monotonic clock".to_string());
    }
    let runner = startup
        .get("runner_entry_micros")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "wgpu startup artifact has no runner entry".to_string())?;
    let harness = startup
        .get("harness_entry_micros")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "wgpu startup artifact has no harness entry".to_string())?;
    let host = startup
        .get("host_ready_micros")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "wgpu startup artifact has no host-ready checkpoint".to_string())?;
    if harness < runner || host < harness {
        return Err("wgpu startup checkpoints are not monotonic".to_string());
    }
    let first = startup
        .get("first_present_micros")
        .and_then(serde_json::Value::as_u64);
    if first.is_some_and(|value| value < host) {
        return Err("wgpu first-present checkpoint precedes host-ready".to_string());
    }
    let expected_first = first.map(|value| value - runner);
    if startup
        .get("runner_to_harness_micros")
        .and_then(serde_json::Value::as_u64)
        != Some(harness - runner)
        || startup
            .get("runner_to_host_ready_micros")
            .and_then(serde_json::Value::as_u64)
            != Some(host - runner)
        || startup
            .get("runner_to_first_present_micros")
            .and_then(serde_json::Value::as_u64)
            != expected_first
    {
        return Err("wgpu startup derived durations do not match checkpoints".to_string());
    }
    Ok(())
}

fn validate_software_artifacts(root: &Path) -> Result<(), String> {
    let environment: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("environment.json")).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    if environment
        .get("candidate")
        .and_then(serde_json::Value::as_str)
        != Some("software")
    {
        return Ok(());
    }
    let resource: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("software-resource.json")).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())?;
    let generation = resource
        .get("generation")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "software resource artifact has no generation".to_string())?;
    let bitmap_creations = resource
        .get("native_bitmap_creations")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "software resource artifact has no bitmap creation count".to_string())?;
    let image_creations = resource
        .get("native_image_creations")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "software resource artifact has no image creation count".to_string())?;
    if generation != bitmap_creations || generation != image_creations {
        return Err("software native resource generation/count mismatch".to_string());
    }
    let metrics_path = root.join("frame-metrics.jsonl");
    let metrics = std::fs::read_to_string(metrics_path).map_err(|err| err.to_string())?;
    for (line_index, line) in metrics.lines().enumerate() {
        let metric: serde_json::Value = serde_json::from_str(line).map_err(|err| {
            format!(
                "invalid software frame metric line {}: {err}",
                line_index + 1
            )
        })?;
        if metric
            .get("atlas_misses")
            .and_then(serde_json::Value::as_u64)
            != Some(0)
        {
            return Err(format!(
                "software atlas miss on metric line {}",
                line_index + 1
            ));
        }
    }
    if environment.get("track").and_then(serde_json::Value::as_str) == Some("capture") {
        let logical_size = environment
            .get("logical_size")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "software capture environment has no logical size".to_string())?;
        let capture_path = root.join(format!("captures/capture-{logical_size}-frame-000005.json"));
        if capture_path.is_file() {
            let capture: serde_json::Value = serde_json::from_slice(
                &std::fs::read(capture_path).map_err(|err| err.to_string())?,
            )
            .map_err(|err| err.to_string())?;
            let expected_physical = logical_size.saturating_mul(2);
            if capture
                .get("physical_width")
                .and_then(serde_json::Value::as_u64)
                != Some(expected_physical)
                || capture
                    .get("physical_height")
                    .and_then(serde_json::Value::as_u64)
                    != Some(expected_physical)
            {
                return Err(
                    "software capture physical dimensions do not match Retina contract".to_string(),
                );
            }
        }
    }
    Ok(())
}

fn run_bounded_process(
    program: &Path,
    args: &[String],
    repo_root: &Path,
    timeout: Duration,
) -> Result<(), String> {
    run_bounded_process_with_env(program, args, repo_root, timeout, &[])
}

fn run_bounded_process_with_env(
    program: &Path,
    args: &[String],
    repo_root: &Path,
    timeout: Duration,
    environment: &[(&str, String)],
) -> Result<(), String> {
    println!("xtask: {} {}", program.display(), args.join(" "));
    let mut child = Command::new(program)
        .args(args)
        .envs(environment.iter().map(|(key, value)| (*key, value)))
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| format!("failed to launch `{}`: {err}", program.display()))?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            return status
                .success()
                .then_some(())
                .ok_or_else(|| format!("renderer spike exited with {status}"));
        }
        if started.elapsed() >= timeout {
            child
                .kill()
                .map_err(|err| format!("failed to kill timed-out renderer spike: {err}"))?;
            let _ = child.wait();
            return Err(format!(
                "renderer spike exceeded bounded timeout of {} ms",
                timeout.as_millis()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(target_os = "macos")]
fn monotonic_micros() -> u64 {
    #[repr(C)]
    struct MachTimebaseInfo {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
    }
    let ticks = unsafe { mach_continuous_time() };
    let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
    let status = unsafe { mach_timebase_info(&raw mut info) };
    if status != 0 || info.denom == 0 {
        return ticks;
    }
    ((u128::from(ticks) * u128::from(info.numer)) / u128::from(info.denom) / 1_000)
        .min(u128::from(u64::MAX)) as u64
}

pub fn run_steps(steps: &[ProcessStep], repo_root: &Path) -> Result<(), String> {
    for step in steps {
        println!("xtask: {} {}", step.program, step.args.join(" "));
        let mut command = Command::new(&step.program);
        command.args(&step.args).current_dir(repo_root);
        if step.best_effort {
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
        } else {
            command
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }

        let status = command.status().map_err(|err| {
            format!(
                "failed to run `{}` from {}: {err}",
                step.program,
                repo_root.display()
            )
        })?;
        if !status.success() && !step.best_effort {
            return Err(format!(
                "`{} {}` failed with {status}",
                step.program,
                step.args.join(" ")
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_fresh_builds_release_bundle_by_default() {
        let steps = companion_fresh_steps(true, false);
        assert_eq!(
            steps.first(),
            Some(&ProcessStep {
                program: "node".to_string(),
                args: vec![
                    "scripts/build-macos-companion-app.mjs".to_string(),
                    "--profile".to_string(),
                    "release".to_string(),
                ],
                best_effort: false,
            })
        );
    }

    #[test]
    fn companion_fresh_uses_release_profile_when_requested() {
        let steps = companion_fresh_steps(true, false);
        assert_eq!(
            steps.first(),
            Some(&ProcessStep {
                program: "node".to_string(),
                args: vec![
                    "scripts/build-macos-companion-app.mjs".to_string(),
                    "--profile".to_string(),
                    "release".to_string(),
                ],
                best_effort: false,
            })
        );
    }

    #[test]
    fn companion_fresh_compiles_retained_backend_on_apple_silicon() {
        let steps = companion_fresh_steps(true, true);
        assert_eq!(
            steps.first(),
            Some(&ProcessStep {
                program: "node".to_string(),
                args: vec![
                    "scripts/build-macos-companion-app.mjs".to_string(),
                    "--profile".to_string(),
                    "release".to_string(),
                    "--features".to_string(),
                    "retained-renderer".to_string(),
                ],
                best_effort: false,
            })
        );
    }

    #[test]
    fn companion_fresh_omits_retained_backend_on_intel() {
        let steps = companion_fresh_steps(true, false);
        let node_step = steps
            .first()
            .expect("companion fresh must build the bundle first");
        assert!(
            !node_step.args.iter().any(|arg| arg == "retained-renderer"),
            "Intel fresh bundle must not compile the retained backend: {:?}",
            node_step.args,
        );
    }

    #[test]
    fn companion_fresh_relaunches_the_fresh_bundle() {
        let steps = companion_fresh_steps(false, false);
        assert_eq!(
            steps.last(),
            Some(&ProcessStep {
                program: "open".to_string(),
                args: vec!["target/macos/Glorp.app".to_string()],
                best_effort: false,
            })
        );
        assert!(steps
            .iter()
            .any(|step| step.program == "osascript" && step.best_effort));
        assert!(steps
            .iter()
            .any(|step| step.program == "pkill" && step.best_effort));
    }

    #[test]
    fn parses_companion_fresh() {
        assert_eq!(
            parse_args(["companion", "fresh"]),
            Ok(XtaskCommand::CompanionFresh { release: true })
        );
    }

    #[test]
    fn parses_companion_fresh_debug() {
        assert_eq!(
            parse_args(["companion", "fresh", "--debug"]),
            Ok(XtaskCommand::CompanionFresh { release: false })
        );
    }

    #[test]
    fn parses_companion_fresh_release() {
        assert_eq!(
            parse_args(["companion", "fresh", "--release"]),
            Ok(XtaskCommand::CompanionFresh { release: true })
        );
    }

    #[test]
    fn parses_companion_scene_baseline() {
        assert_eq!(
            parse_args([
                "companion",
                "scene-baseline",
                "--duration-ms",
                "120000",
                "--out",
                "docs/superpowers/measurements/baseline.md",
            ]),
            Ok(XtaskCommand::CompanionSceneBaseline {
                duration_ms: 120_000,
                out: "docs/superpowers/measurements/baseline.md".into(),
            })
        );
    }

    #[test]
    fn parses_scene_lifetime_exact_shape() {
        assert_eq!(
            parse_args([
                "companion",
                "scene-lifetime",
                "--frames",
                "4500",
                "--out",
                "target/glorp-scene-gates/lifetime",
            ]),
            Ok(XtaskCommand::CompanionSceneLifetime {
                frames: 4_500,
                out: "target/glorp-scene-gates/lifetime".into(),
            })
        );
    }

    #[test]
    fn parses_scene_fault_soak_exact_shape() {
        assert_eq!(
            parse_args([
                "companion",
                "scene-fault-soak",
                "--out",
                "target/glorp-scene-gates/faults",
            ]),
            Ok(XtaskCommand::CompanionSceneFaultSoak {
                out: "target/glorp-scene-gates/faults".into(),
            })
        );
    }

    #[test]
    fn parses_scene_native_smoke_with_and_without_auto() {
        for (auto_flag, auto, duration) in
            [(None, false, "300000"), (Some("--auto"), true, "14400000")]
        {
            let mut args = vec!["companion", "scene-native-smoke", "--duration-ms", duration];
            if let Some(flag) = auto_flag {
                args.push(flag);
            }
            args.extend(["--out", "target/glorp-scene-gates/native"]);
            assert_eq!(
                parse_args(args),
                Ok(XtaskCommand::CompanionSceneNativeSmoke {
                    duration_ms: duration.parse().unwrap(),
                    auto,
                    out: "target/glorp-scene-gates/native".into(),
                })
            );
        }
        for args in [
            vec![
                "companion",
                "scene-native-smoke",
                "--duration-ms",
                "20000",
                "--out",
                "target/glorp-scene-gates/native",
            ],
            vec![
                "companion",
                "scene-native-smoke",
                "--duration-ms",
                "300000",
                "--auto",
                "--out",
                "target/glorp-scene-gates/native",
            ],
        ] {
            assert!(parse_args(args).is_err());
        }
    }

    #[test]
    fn scene_gate_parsers_reject_missing_zero_unknown_and_unsafe_values() {
        for args in [
            vec![
                "companion",
                "scene-lifetime",
                "--frames",
                "0",
                "--out",
                "target/glorp-scene-gates/lifetime",
            ],
            vec![
                "companion",
                "scene-native-smoke",
                "--duration-ms",
                "0",
                "--out",
                "target/glorp-scene-gates/native",
            ],
            vec![
                "companion",
                "scene-native-smoke",
                "--duration-ms",
                "1",
                "--auto",
                "--auto",
                "--out",
                "target/glorp-scene-gates/native",
            ],
            vec![
                "companion",
                "scene-fault-soak",
                "--bogus",
                "target/glorp-scene-gates/faults",
            ],
            vec![
                "companion",
                "scene-lifetime",
                "--frames",
                "1",
                "--out",
                "target/glorp-scene-gates/../private",
            ],
        ] {
            assert!(parse_args(args).is_err());
        }
        assert!(parse_args(["companion", "scene-lifetime", "--frames", "4500"]).is_err());
        assert!(parse_args([
            "companion",
            "scene-native-smoke",
            "--out",
            "target/glorp-scene-gates/native",
        ])
        .is_err());
    }

    #[test]
    fn scene_command_plans_use_release_isolation_live_routing_and_exact_timeouts() {
        let lifetime = scene_lifetime_plan(4_500, "target/glorp-scene-gates/lifetime");
        assert_eq!(
            lifetime.build.args,
            ["build", "--release", "--features", "retained-renderer"]
        );
        assert!(lifetime
            .init_args
            .windows(2)
            .any(|pair| pair == ["--seed", "glorp-scene-lifetime-v1"]));
        assert!(lifetime
            .companion_args
            .windows(2)
            .any(|pair| pair == ["--retained-scene-runtime", "live"]));
        assert_eq!(lifetime.timeout, Duration::from_millis(600_000));

        let native = scene_native_smoke_plan(
            300_000,
            false,
            "target/glorp-scene-gates/native-five-minute",
        );
        assert!(native
            .companion_args
            .windows(2)
            .any(|pair| pair == ["--renderer", "retained"]));
        assert!(native
            .companion_args
            .windows(2)
            .any(|pair| pair == ["--retained-scene-runtime", "live"]));
        assert_eq!(native.timeout, Duration::from_millis(360_000));

        let auto =
            scene_native_smoke_plan(14_400_000, true, "target/glorp-scene-gates/auto-four-hour");
        assert!(auto
            .companion_args
            .windows(2)
            .any(|pair| pair == ["--renderer", "auto"]));
        assert!(!auto
            .companion_args
            .iter()
            .any(|arg| arg == "--retained-scene-runtime"));
        assert_eq!(auto.timeout, Duration::from_millis(14_460_000));
    }

    #[test]
    fn fault_plans_cover_every_existing_typed_category_with_all_features() {
        assert_eq!(SCENE_FAULT_CASES.len(), 11);
        let unique = SCENE_FAULT_CASES
            .iter()
            .map(|case| case.injection)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), SCENE_FAULT_CASES.len());
        for case in SCENE_FAULT_CASES {
            let out = format!("target/glorp-scene-gates/faults/{}", case.injection);
            let plan = scene_fault_plan(case, &out);
            assert_eq!(plan.build.args, ["build", "--release", "--all-features"]);
            assert!(plan
                .companion_args
                .windows(2)
                .any(|pair| pair == ["--retained-scene-runtime", "live"]));
            assert!(plan
                .companion_args
                .windows(2)
                .any(|pair| pair == ["--review-inject-retained-fault", case.injection]));
            assert_eq!(
                plan.companion_args
                    .iter()
                    .any(|arg| arg == "--review-runtime-metrics-out"),
                case.produces_retained_host(),
            );
            assert!(case.expected_category.starts_with("retained-"));
            assert_eq!(plan.timeout, Duration::from_millis(62_000));
        }
    }

    #[test]
    fn scene_baseline_requests_a_direct_live_gpu_capture() {
        let args = scene_baseline_companion_args(
            120_000,
            std::path::Path::new("target/glorp-scene-baseline/runtime-metrics.json"),
        );
        assert!(args.windows(2).any(|pair| {
            pair == [
                "--review-capture-dir".to_string(),
                "target/glorp-review/scene-baseline".to_string(),
            ]
        }));
        assert!(args
            .windows(2)
            .any(|pair| { pair == ["--retained-scene-runtime".to_string(), "live".to_string(),] }));
    }

    #[test]
    fn runtime_snapshot_requires_schema10_timing_and_worker_diagnostics() {
        let snapshot = runtime_snapshot_fixture();
        assert!(validate_runtime_snapshot(&snapshot, false).is_ok());

        let mut legacy = snapshot.clone();
        legacy["schema_version"] = serde_json::json!(6);
        assert_eq!(
            validate_runtime_snapshot(&legacy, false),
            Err("runtime metrics snapshot schema_version is not 10 (observed 6)".to_string())
        );

        for field in [
            "worker_active_compile_us",
            "generation_service_ui_us",
            "gpu_materialize_publish_us",
            "main_thread_raster_calls",
            "worker_raster_calls",
            "worker_submissions",
            "worker_completions",
            "worker_cancellations",
            "worker_coalesces",
            "worker_stale_rejections",
            "worker_failures",
            "gpu_materializations",
            "generation_count",
        ] {
            let mut missing = snapshot.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                validate_runtime_snapshot(&missing, false)
                    .unwrap_err()
                    .contains(field),
                "missing field {field} was not rejected"
            );
        }
        for metric in [
            "ui_tick_us",
            "projection_us",
            "reconcile_us",
            "delta_write_us",
            "encode_us",
            "submit_us",
            "capture_us",
            "worker_active_compile_us",
            "generation_service_ui_us",
            "gpu_materialize_publish_us",
        ] {
            let mut missing = snapshot.clone();
            missing[metric].as_object_mut().unwrap().remove("max");
            assert!(
                validate_runtime_snapshot(&missing, false)
                    .unwrap_err()
                    .contains(&format!("{metric}.max")),
                "missing max for {metric} was not rejected"
            );
        }

        let mut missing_activation_max = snapshot.clone();
        missing_activation_max["activation_render_owner_us"]
            .as_object_mut()
            .unwrap()
            .remove("max");
        assert_eq!(
            validate_runtime_snapshot(&missing_activation_max, true),
            Err("runtime metrics snapshot missing activation_render_owner_us.max".to_string())
        );

        for legacy_only in [
            "state_prepare_us",
            "gpu_translate_us",
            "queue_wait_us",
            "raster_request_wall_us",
        ] {
            let mut direct = snapshot.clone();
            for percentile in ["p50", "p95", "p99", "max"] {
                direct[legacy_only][percentile] = serde_json::Value::Null;
            }
            assert!(
                validate_runtime_snapshot(&direct, true).is_ok(),
                "direct runtime incorrectly required legacy-only metric {legacy_only}"
            );
        }

        let mut no_activation = snapshot;
        for percentile in ["p50", "p95", "p99", "max"] {
            no_activation["activation_render_owner_us"][percentile] = serde_json::Value::Null;
        }
        assert!(validate_runtime_snapshot(&no_activation, false).is_ok());
        assert_eq!(
            validate_runtime_snapshot(&no_activation, true),
            Err("runtime metrics snapshot missing activation_render_owner_us.p50".to_string())
        );
    }

    #[test]
    fn scene_runtime_invariants_reject_fallback_growth_hidden_work_and_capacity_breaches() {
        for (path, value) in [
            (&["fallback_count"][..], 1),
            (&["generation_failures"][..], 1),
            (&["persistent_gpu_objects_created"][..], 1),
            (&["static_upload_bytes"][..], 1),
            (&["node_high_water"][..], 129),
            (&["primitive_high_water"][..], 769),
            (&["blended_draw_high_water"][..], 257),
            (&["hidden_segment", "steady_delta", "submit"][..], 1),
        ] {
            let mut snapshot = runtime_snapshot_fixture();
            set_snapshot_u64(&mut snapshot, path, value);
            assert!(
                validate_scene_runtime_invariants(&snapshot, false).is_err(),
                "path {path:?} was not rejected"
            );
        }
        assert!(validate_scene_runtime_invariants(&runtime_snapshot_fixture(), false).is_ok());
    }

    #[test]
    fn lifetime_validator_enforces_requested_schedule_churn_capture_and_memory_limits() {
        let snapshot = runtime_snapshot_fixture();
        validate_lifetime_audit(&snapshot, 4_500).unwrap();

        let mut wrong_frames = snapshot.clone();
        wrong_frames["lifetime_audit"]["semantic_samples"] = serde_json::json!(4_499);
        assert!(validate_lifetime_audit(&wrong_frames, 4_500).is_err());

        let mut growth = snapshot.clone();
        growth["lifetime_audit"]["rss_peak_bytes"] = serde_json::json!(1_010_001);
        assert!(validate_lifetime_audit(&growth, 4_500).is_err());

        let mut evolved = snapshot;
        evolved["lifetime_audit"] = lifetime_v10_fixture(4_500);
        validate_lifetime_audit(&evolved, 4_500).unwrap();
        evolved["lifetime_audit"]["capacity_growth_events"] = serde_json::json!(1);
        assert!(validate_lifetime_audit(&evolved, 4_500).is_err());

        let mut wrong_cadence = runtime_snapshot_fixture();
        wrong_cadence["lifetime_audit"] = lifetime_v10_fixture(4_500);
        wrong_cadence["lifetime_audit"]["presentation_ticks"] = serde_json::json!(36_000);
        assert!(validate_lifetime_audit(&wrong_cadence, 4_500).is_err());

        let mut fake_work = runtime_snapshot_fixture();
        fake_work["lifetime_audit"] = lifetime_v10_fixture(4_500);
        fake_work["lifetime_audit"]["work_delta"]["submit"] = serde_json::json!(0);
        assert!(validate_lifetime_audit(&fake_work, 4_500).is_err());
    }

    #[test]
    fn direct_manifest_requires_live_retained_nonblank_no_fallback_route() {
        let manifest = direct_manifest_fixture();
        validate_direct_scene_manifest(&manifest).unwrap();
        for (field, value) in [
            ("route", serde_json::json!("legacy-paired")),
            ("effective_renderer", serde_json::json!("smooth")),
            ("nonblank_validation", serde_json::json!("blank")),
            ("fallback_occurred", serde_json::json!(true)),
            ("fallback_transition_count", serde_json::json!(1)),
            ("failure_category", serde_json::json!("private error text")),
        ] {
            let mut invalid = manifest.clone();
            invalid[field] = value;
            assert!(
                validate_direct_scene_manifest(&invalid).is_err(),
                "field {field} was not rejected"
            );
        }
        for (field, value) in [
            ("presented_frame_count", serde_json::json!(0)),
            ("last_present_age_ms", serde_json::json!(2_001)),
        ] {
            let mut invalid = manifest.clone();
            invalid[field] = value;
            assert!(
                validate_direct_scene_manifest(&invalid).is_err(),
                "presentation proof field {field} was not rejected"
            );
        }
    }

    #[test]
    fn direct_receipts_and_frozen_artifact_capacities_are_exact() {
        let manifest = direct_manifest_fixture();
        let scene_version = manifest["receipt"]["scene_version"].clone();
        let snapshot = serde_json::json!({
            "request": {"requested_version": scene_version},
            "readback_version": scene_version,
        });
        let version = serde_json::json!({"receipt": manifest["receipt"]});
        validate_direct_scene_receipts(&manifest, &snapshot, &version).unwrap();

        let artifacts = scene_artifacts_fixture();
        validate_scene_artifact_capacities(&artifacts).unwrap();
        let mut oversized = artifacts;
        oversized["template"]["primitive_count"] = serde_json::json!(769);
        assert!(validate_scene_artifact_capacities(&oversized).is_err());

        let mut mismatched = snapshot;
        mismatched["readback_version"]["surface"] = serde_json::json!(999);
        assert!(validate_direct_scene_receipts(&manifest, &mismatched, &version).is_err());
    }

    #[test]
    fn direct_artifact_validator_requires_every_exact_filename() {
        let root = unique_temp_dir("glorp-direct-artifacts");
        std::fs::create_dir_all(&root).unwrap();
        for artifact in DIRECT_SCENE_ARTIFACTS {
            if artifact.ends_with(".json") {
                std::fs::write(root.join(artifact), b"{}").unwrap();
            } else {
                std::fs::write(root.join(artifact), b"not-empty").unwrap();
            }
        }
        let metrics = runtime_snapshot_fixture();
        assert!(validate_direct_scene_artifacts(&root, &metrics).is_err());
        for missing in DIRECT_SCENE_ARTIFACTS {
            let path = root.join(missing);
            let saved = std::fs::read(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            let error = validate_direct_scene_artifacts(&root, &metrics).unwrap_err();
            assert!(error.contains(missing), "missing {missing}: {error}");
            std::fs::write(path, saved).unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fault_summary_rejects_missing_mismatched_or_unsanitized_outcomes() {
        let outcomes = SCENE_FAULT_CASES
            .iter()
            .map(|case| {
                serde_json::json!({
                    "injection": case.injection,
                    "expected_category": case.expected_category,
                    "observed_category": case.expected_category,
                    "sanitized": true,
                    "expected_exit_success": case.expected_success,
                    "process_status_matched": true,
                    "evidence": {
                        "render_log_valid": true,
                        "paint_frame_count": 1,
                        "frame_count": 1,
                        "nonblank_shape_frame_count": if case.is_capture_failure() { 0 } else { 1 },
                        "diagnostic_appkit_screenshot_structurally_valid": true,
                        "diagnostic_appkit_screenshot_nonblank": !case.is_capture_failure(),
                        "runtime_metrics_present": case.produces_retained_host(),
                        "acknowledged_fallback_paint": !case.is_capture_failure(),
                        "capture_failure_observed": case.is_capture_failure(),
                        "direct_artifacts_absent": true,
                    },
                })
            })
            .collect::<Vec<_>>();
        let mut summary = serde_json::json!({"outcomes": outcomes});
        validate_fault_soak_summary(&summary).unwrap();
        summary["outcomes"][0]["observed_category"] = serde_json::json!("raw driver text");
        assert!(validate_fault_soak_summary(&summary).is_err());
        summary["outcomes"][0]["observed_category"] =
            summary["outcomes"][0]["expected_category"].clone();
        summary["outcomes"][0]["sanitized"] = serde_json::json!(false);
        assert!(validate_fault_soak_summary(&summary).is_err());
        summary["outcomes"][0]["sanitized"] = serde_json::json!(true);
        summary["outcomes"][0]["evidence"]["direct_artifacts_absent"] = serde_json::json!(false);
        assert!(validate_fault_soak_summary(&summary).is_err());
        summary["outcomes"].as_array_mut().unwrap().pop();
        assert!(validate_fault_soak_summary(&summary).is_err());
    }

    #[test]
    fn diagnostic_png_validation_decodes_visible_pixels_instead_of_trusting_idat() {
        fn rgba_png(pixel: [u8; 4]) -> Vec<u8> {
            let mut bytes = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().unwrap();
                writer.write_image_data(&pixel).unwrap();
            }
            bytes
        }

        let transparent = rgba_png([0, 0, 0, 0]);
        assert!(png_has_nonempty_image_data(&transparent).unwrap());
        assert!(!png_has_nonblank_pixels(&transparent).unwrap());

        let visible = rgba_png([8, 4, 2, 255]);
        assert!(png_has_nonblank_pixels(&visible).unwrap());
    }

    #[test]
    fn scene_gate_reset_removes_all_stale_output() {
        let root = unique_temp_dir("glorp-scene-gate-reset");
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("scene-manifest.json"), b"stale").unwrap();
        std::fs::write(root.join("nested/stale.txt"), b"stale").unwrap();
        reset_scene_gate_out_dir(&root).unwrap();
        assert!(!root.exists());
        reset_scene_gate_out_dir(&root).unwrap();
    }

    #[test]
    fn baseline_gates_accept_exact_frozen_boundaries() {
        let gates = evaluate_baseline_gates(&baseline_gate_fixture()).unwrap();
        assert!(gates.iter().all(|gate| gate.status == GateStatus::Pass));
        assert!(validate_gate_results(&gates).is_ok());
    }

    #[test]
    fn lifetime_gates_skip_only_the_unexercised_activation_slice() {
        let mut snapshot = baseline_gate_fixture();
        snapshot["activation_render_owner_us"]["p95"] = serde_json::Value::Null;
        snapshot["activation_render_owner_us"]["max"] = serde_json::Value::Null;

        let gates = evaluate_lifetime_gates(&snapshot).unwrap();
        assert!(gates.iter().all(|gate| gate.status == GateStatus::Pass));
        assert!(gates
            .iter()
            .all(|gate| gate.id != "activation-render-owner-slice"));
        assert!(evaluate_baseline_gates(&snapshot).is_err());

        snapshot["ui_tick_us"]["p95"] = serde_json::json!(1_423);
        let gates = evaluate_lifetime_gates(&snapshot).unwrap();
        assert_eq!(
            gates
                .iter()
                .find(|gate| gate.id == "ui-tick-p95")
                .unwrap()
                .status,
            GateStatus::Fail
        );
    }

    #[test]
    fn native_smoke_gates_require_runtime_boundaries_without_lifetime_audit() {
        let mut snapshot = baseline_gate_fixture();
        snapshot["lifetime_audit"] = serde_json::Value::Null;

        let gates = evaluate_native_smoke_gates(&snapshot).unwrap();
        assert!(gates.iter().all(|gate| gate.status == GateStatus::Pass));
        assert!(gates
            .iter()
            .any(|gate| gate.id == "activation-render-owner-slice"));
        assert!(gates.iter().any(|gate| gate.id == "terminal-capture"));
        assert!(gates.iter().all(|gate| gate.id != "lifetime-rss"));

        snapshot["activation_render_owner_us"]["max"] = serde_json::json!(16_001);
        let gates = evaluate_native_smoke_gates(&snapshot).unwrap();
        assert_eq!(
            gates
                .iter()
                .find(|gate| gate.id == "activation-render-owner-slice")
                .unwrap()
                .status,
            GateStatus::Fail
        );

        let mut snapshot = baseline_gate_fixture();
        snapshot["lifetime_audit"] = serde_json::Value::Null;
        snapshot["capture_nonblank_validated"] = serde_json::json!(0);
        let gates = evaluate_native_smoke_gates(&snapshot).unwrap();
        assert_eq!(
            gates
                .iter()
                .find(|gate| gate.id == "terminal-capture")
                .unwrap()
                .status,
            GateStatus::Fail
        );
    }

    #[test]
    fn baseline_gates_reject_one_over_every_frozen_boundary() {
        let cases = [
            (&["ui_tick_us", "p95"][..], 1_423, "ui-tick-p95"),
            (&["ui_tick_us", "p99"][..], 2_071, "ui-tick-p99"),
            (&["encode_us", "p95"][..], 283, "encode-p95"),
            (
                &["main_thread_raster_calls"][..],
                1,
                "main-thread-raster-calls",
            ),
            (
                &["generation_service_ui_us", "max"][..],
                4_001,
                "generation-service-ui-max",
            ),
            (
                &["gpu_materialize_publish_us", "max"][..],
                16_001,
                "gpu-materialize-publish-max",
            ),
            (
                &["activation_render_owner_us", "p95"][..],
                16_001,
                "activation-render-owner-slice",
            ),
            (
                &["activation_render_owner_us", "max"][..],
                16_001,
                "activation-render-owner-slice",
            ),
            (
                &["metrics_overhead_control", "net_ns_per_tick"][..],
                28_441,
                "metrics-overhead-control",
            ),
        ];
        for (path, value, gate_id) in cases {
            let mut snapshot = baseline_gate_fixture();
            set_snapshot_u64(&mut snapshot, path, value);
            let gates = evaluate_baseline_gates(&snapshot).unwrap();
            let failed = gates.iter().find(|gate| gate.id == gate_id).unwrap();
            assert_eq!(failed.status, GateStatus::Fail, "path {path:?}");
            assert!(validate_gate_results(&gates).is_err(), "path {path:?}");
        }
    }

    #[test]
    fn worker_generation_evidence_gate_rejects_vacuous_or_inconsistent_runs() {
        for field in [
            "worker_raster_calls",
            "worker_submissions",
            "worker_completions",
            "gpu_materializations",
            "generation_count",
        ] {
            let mut snapshot = baseline_gate_fixture();
            snapshot[field] = serde_json::json!(0);
            let gates = evaluate_baseline_gates(&snapshot).unwrap();
            assert_eq!(
                gates
                    .iter()
                    .find(|gate| gate.id == "worker-generation-evidence")
                    .expect("worker generation evidence gate exists")
                    .status,
                GateStatus::Fail,
                "zero {field} must fail worker generation evidence",
            );
        }

        for (materializations, generations, completions) in [(2, 1, 2), (2, 2, 1)] {
            let mut snapshot = baseline_gate_fixture();
            snapshot["gpu_materializations"] = serde_json::json!(materializations);
            snapshot["generation_count"] = serde_json::json!(generations);
            snapshot["worker_completions"] = serde_json::json!(completions);
            let gates = evaluate_baseline_gates(&snapshot).unwrap();
            assert_eq!(
                gates
                    .iter()
                    .find(|gate| gate.id == "worker-generation-evidence")
                    .expect("worker generation evidence gate exists")
                    .status,
                GateStatus::Fail,
                "inconsistent materializations={materializations} generations={generations} completions={completions} must fail",
            );
        }
    }

    #[test]
    fn baseline_gate_limits_are_frozen_independent_of_observations() {
        let mut snapshot = baseline_gate_fixture();
        snapshot["ui_tick_us"]["p95"] = serde_json::json!(1);
        snapshot["ui_tick_us"]["p99"] = serde_json::json!(2);
        snapshot["encode_us"]["p95"] = serde_json::json!(3);
        let gates = evaluate_baseline_gates(&snapshot).unwrap();
        let limits = gates
            .iter()
            .map(|gate| (gate.id, gate.limit.as_str()))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(limits["ui-tick-p95"], "1422 us");
        assert_eq!(limits["ui-tick-p99"], "2070 us");
        assert_eq!(limits["encode-p95"], "282 us");
        assert_eq!(limits["metrics-overhead-control"], "28440 ns/tick");
    }

    #[test]
    fn hidden_gate_tracks_worker_submissions_and_gpu_materializations() {
        for field in ["worker_submissions", "gpu_materializations"] {
            let mut snapshot = baseline_gate_fixture();
            snapshot["hidden_segment"]["steady_delta"][field] = serde_json::json!(1);
            let gates = evaluate_baseline_gates(&snapshot).unwrap();
            assert_eq!(
                gates
                    .iter()
                    .find(|gate| gate.id == "hidden-steady-state")
                    .unwrap()
                    .status,
                GateStatus::Fail
            );
        }
    }

    #[test]
    fn scene_baseline_report_describes_worker_diagnostics_without_appkit_slices() {
        let snapshot = runtime_snapshot_fixture();
        let gates = evaluate_baseline_gates(&snapshot).unwrap();
        let report = render_scene_baseline_report(
            Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
            120_000,
            &snapshot,
            &BaselineSourceIdentity {
                commit: "test-commit".to_string(),
                tracked_tree_state: "clean".to_string(),
            },
            &gates,
        )
        .unwrap();
        assert!(report.contains("Worker active compile"));
        assert!(report.contains("Raster request wall time"));
        assert!(report.contains("Generation-service UI work"));
        assert!(report.contains("GPU materialize/upload/publish"));
        assert!(report.contains(
            "Worker submissions/completions/cancellations/coalesces/stale rejections/failures"
        ));
        assert!(report.contains("Main-thread/worker raster calls"));
        assert!(report.contains("| `worker-generation-evidence` | PASS |"));
        assert!(report.contains("| `ui-tick-p95` | PASS | 2 us | 1422 us |"));
        assert!(report.contains("| `ui-tick-p99` | PASS | 3 us | 2070 us |"));
        assert!(report.contains("| `encode-p95` | PASS | 2 us | 282 us |"));
        assert!(!report.contains("AppKit raster slice"));
        assert!(!report.contains("AppKit raster slice diagnostics"));
    }

    fn baseline_gate_fixture() -> serde_json::Value {
        let mut snapshot = serde_json::json!({
            "ui_tick_us": {"p95": 1422, "p99": 2070},
            "encode_us": {"p95": 282},
            "main_thread_raster_calls": 0,
            "worker_raster_calls": 1,
            "worker_submissions": 1,
            "worker_completions": 1,
            "gpu_materializations": 1,
            "generation_count": 1,
            "generation_service_ui_us": {"max": 4000},
            "gpu_materialize_publish_us": {"max": 16000},
            "activation_render_owner_us": {"p95": 16000, "max": 16000},
            "metrics_overhead_control": {"net_ns_per_tick": 28440},
            "capture_attempted": 1,
            "capture_succeeded": 1,
            "capture_failed": 0,
            "capture_nonblank_validated": 1,
            "persistent_gpu_objects_created": 0,
            "static_upload_bytes": 0,
            "hidden_segment": {
                "transition_ticks": 1,
                "steady_ticks": 2,
                "steady_delta": {"prepare": 0, "worker_submissions": 0, "gpu_materializations": 0, "queue_writes": 0, "surface_acquires": 0, "encode": 0, "submit": 0}
            },
            "lifetime_audit": null
        });
        snapshot["lifetime_audit"] = lifetime_v10_fixture(4_500);
        snapshot
    }

    fn runtime_snapshot_fixture() -> serde_json::Value {
        let mut snapshot = baseline_gate_fixture();
        let object = snapshot.as_object_mut().unwrap();
        object.insert(
            "schema_version".into(),
            serde_json::json!(RUNTIME_METRICS_SCHEMA_VERSION),
        );
        for metric in [
            "ui_tick_us",
            "projection_us",
            "reconcile_us",
            "state_prepare_us",
            "gpu_translate_us",
            "delta_write_us",
            "encode_us",
            "submit_us",
            "capture_us",
            "queue_wait_us",
            "worker_active_compile_us",
            "raster_request_wall_us",
            "generation_service_ui_us",
            "gpu_materialize_publish_us",
            "activation_render_owner_us",
        ] {
            object.insert(
                metric.into(),
                serde_json::json!({"p50": 1, "p95": 2, "p99": 3, "max": 4}),
            );
        }
        for metric in [
            "generation_service_ui_us",
            "gpu_materialize_publish_us",
            "activation_render_owner_us",
        ] {
            object.get_mut(metric).unwrap()["max"] = serde_json::json!(match metric {
                "generation_service_ui_us" => 4_000,
                _ => 16_000,
            });
        }
        for (field, value) in [
            ("main_thread_raster_calls", 0),
            ("worker_raster_calls", 1),
            ("worker_submissions", 1),
            ("worker_completions", 1),
            ("worker_cancellations", 0),
            ("worker_coalesces", 0),
            ("worker_stale_rejections", 0),
            ("worker_failures", 0),
            ("gpu_materializations", 1),
            ("generation_count", 1),
            ("generation_failures", 0),
            ("generation_retries", 0),
            ("generation_stale_drops", 0),
            ("fallback_count", 0),
            ("fallback_pending_transitions", 0),
            ("fallback_painted_transitions", 0),
            ("node_high_water", 128),
            ("primitive_high_water", 768),
            ("blended_draw_high_water", 256),
        ] {
            object.insert(field.into(), serde_json::json!(value));
        }
        object.insert("visible_samples".into(), serde_json::json!(100));
        object.insert(
            "fixture".into(),
            serde_json::json!({
                "fixture_id": "glorp-scene-baseline-v2",
                "seed": "test",
                "update_source": "fixed",
                "cadence_ms": 250,
                "logical_width": 360.0,
                "logical_height": 360.0,
                "physical_width": 720,
                "physical_height": 720,
                "backing_scale": 2.0
            }),
        );
        object.insert(
            "inventory".into(),
            serde_json::json!({
                "matrix_fixture_count": 630,
                "dimmed_fixture_count": 126,
                "full_props_tank_fixture_count": 630,
                "max_prepared_gpu_primitives": {"observed": 1, "reservation": 0, "headroom": 1023, "limit": 1024},
                "max_nodes": {"observed": 0, "reservation": 96, "headroom": 32, "limit": 128},
                "max_static_primitives": {"observed": 0, "reservation": 640, "headroom": 128, "limit": 768},
                "max_pet_slots": {"observed": 130, "reservation": 0, "headroom": 0, "limit": 130},
                "max_visible_props": {"observed": 10, "reservation": 0, "headroom": 0, "limit": 10},
                "max_round_tank_inhabitants": {"observed": 2, "reservation": 0, "headroom": 0, "limit": 2},
                "max_ambient_instances": {"observed": 0, "reservation": 48, "headroom": 16, "limit": 64},
                "max_blended_draws": {"observed": 0, "reservation": 192, "headroom": 64, "limit": 256},
                "max_lights": {"observed": 0, "reservation": 1, "headroom": 1, "limit": 2},
                "max_attachments": {"observed": 0, "reservation": 16, "headroom": 16, "limit": 32}
            }),
        );
        object.insert(
            "gpu_accounting".into(),
            serde_json::json!({
                "current_bytes": {"atlas_bytes": 1, "instance_ring_bytes": 1, "capture_bytes": 0, "total_bytes": 2},
                "current_objects": {"host_infrastructure": 1, "atlas": 1, "instance_ring": 1, "capture": 0, "total_objects": 3},
                "peak_total_bytes": 2,
                "peak_total_objects": 3,
                "objects_created_total": 3,
                "objects_destroyed_total": 0
            }),
        );
        object.insert(
            "metrics_overhead_control".into(),
            serde_json::json!({
                "trials": 5,
                "iterations": 100,
                "control_ticks": 500,
                "instrumented_ticks": 500,
                "control_ns_per_tick": 1,
                "instrumented_ns_per_tick": 2,
                "net_ns_per_tick": 1
            }),
        );
        object.insert("lifetime_audit".into(), lifetime_v10_fixture(4_500));
        snapshot
    }

    fn lifetime_v10_fixture(frames: u64) -> serde_json::Value {
        let presentation_ticks = frames.saturating_mul(15).div_ceil(2);
        serde_json::json!({
            "semantic_samples": frames,
            "warmup_semantic_samples": frames,
            "presentation_ticks": presentation_ticks,
            "warmup_presentation_ticks": presentation_ticks,
            "semantic_cadence_ms": 250,
            "presentation_cadence_hz": 30,
            "virtual_elapsed_ms": frames * 250,
            "snapshot_projections": frames,
            "semantic_reconciles": frames,
            "frame_projections": presentation_ticks,
            "frame_reconciles": presentation_ticks,
            "encoded_ticks": presentation_ticks,
            "submitted_ticks": presentation_ticks,
            "draw_calls": presentation_ticks,
            "poll_count": 10,
            "work_delta": {
                "prepare": 0,
                "worker_submissions": 0,
                "gpu_materializations": 0,
                "queue_writes": 1,
                "surface_acquires": 0,
                "encode": presentation_ticks,
                "submit": presentation_ticks
            },
            "work_per_second": {
                "prepare": 0,
                "worker_submissions": 0,
                "gpu_materializations": 0,
                "queue_writes": 0,
                "surface_acquires": 0,
                "encode": 30,
                "submit": 30
            },
            "capacity_growth_events": 0,
            "stale_mutations": 0,
            "stale_rejections": 0,
            "stale_regenerations": 0,
            "post_warmup_resource_creations": 0,
            "post_warmup_static_upload_bytes": 0,
            "direct_target_prewarmed": true,
            "direct_target_reused": true,
            "direct_readback_prewarmed": true,
            "direct_readback_reused": true,
            "terminal_direct_capture_attempted": 1,
            "terminal_direct_capture_succeeded": 1,
            "terminal_direct_capture_nonblank": 1,
            "rss_warmup_bytes": 1_000_000,
            "rss_warmup_peak_bytes": 1_000_000,
            "rss_final_bytes": 1_005_000,
            "rss_peak_bytes": 1_009_000,
            "gpu_warmup_bytes": 1_000_000,
            "gpu_warmup_peak_bytes": 1_000_000,
            "gpu_final_bytes": 1_005_000,
            "gpu_peak_bytes": 1_009_000,
        })
    }

    fn direct_manifest_fixture() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "requested_renderer": "retained",
            "effective_renderer": "retained",
            "fallback_occurred": false,
            "fallback_transition_count": 0,
            "last_fallback_reason": null,
            "route": "direct-retained-scene",
            "logical_points": [360.0, 360.0],
            "physical_pixels": [2, 2],
            "backing_scale": 1.0,
            "receipt": {"scene_version": {"generation": 1, "surface": 2}},
            "capture_checksum_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "presented_frame_count": 1,
            "last_present_age_ms": 0,
            "nonblank_validation": "valid",
            "privacy_disposition": "external-redacted",
            "failure_category": null,
            "skip_category": null,
        })
    }

    fn scene_artifacts_fixture() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "template": {
                "primitive_count": 768,
                "blended_draw_count": 256,
            },
            "content": {
                "occupied_pet_art_slots": (0..130).collect::<Vec<_>>(),
                "occupied_prop_slots": (0..10).collect::<Vec<_>>(),
                "occupied_tank_slots": (0..2).collect::<Vec<_>>(),
                "active_ambient_slots": (0..64).collect::<Vec<_>>(),
            },
            "frame": {
                "nodes": (0..128).collect::<Vec<_>>(),
                "light_count": 2,
            },
        })
    }

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn set_snapshot_u64(snapshot: &mut serde_json::Value, path: &[&str], value: u64) {
        let (last, parents) = path.split_last().unwrap();
        let mut current = snapshot;
        for component in parents {
            current = current.get_mut(*component).unwrap();
        }
        current[*last] = serde_json::json!(value);
    }

    #[test]
    fn rejects_unknown_command_with_usage_hint() {
        let err = parse_args(["nope"]).unwrap_err();
        assert!(err.contains("cargo xtask companion fresh"));
    }

    #[test]
    fn parses_companion_review_pair() {
        assert_eq!(
            parse_args([
                "companion",
                "review-pair",
                "--size",
                "360",
                "--state",
                "normal",
                "--out",
                "target/glorp-review/pair",
            ]),
            Ok(XtaskCommand::CompanionReviewPair {
                size: 360,
                state: Some("normal".into()),
                dimmed: false,
                live_values: false,
                out: "target/glorp-review/pair".into(),
            })
        );
    }

    #[test]
    fn parses_companion_review_pair_live_and_dimmed() {
        assert_eq!(
            parse_args([
                "companion",
                "review-pair",
                "--size",
                "720",
                "--dimmed",
                "--live-values",
                "--out",
                "target/glorp-review-sensitive/live-pair",
            ]),
            Ok(XtaskCommand::CompanionReviewPair {
                size: 720,
                state: None,
                dimmed: true,
                live_values: true,
                out: "target/glorp-review-sensitive/live-pair".into(),
            })
        );
    }

    #[test]
    fn review_pair_live_values_require_the_sensitive_root() {
        let error = parse_args([
            "companion",
            "review-pair",
            "--size",
            "360",
            "--live-values",
            "--out",
            "target/glorp-review/pair",
        ])
        .unwrap_err();
        assert!(error.contains("below `target/glorp-review-sensitive`"));
    }

    #[test]
    fn review_pair_rejects_output_outside_the_review_root() {
        for out in [
            "/tmp/glorp-review/pair",
            "target/glorp-review/../../private/pair",
            "target/not-glorp-review/pair",
        ] {
            let error = parse_args(["companion", "review-pair", "--size", "360", "--out", out])
                .unwrap_err();
            assert!(error.contains("below `target/glorp-review`"));
        }
    }

    #[test]
    fn review_pair_rejects_unfair_size_and_bad_state() {
        assert!(parse_args([
            "companion",
            "review-pair",
            "--size",
            "128",
            "--out",
            "target/glorp-review/pair",
        ])
        .unwrap_err()
        .contains("at least 260"));
        assert!(parse_args([
            "companion",
            "review-pair",
            "--size",
            "360",
            "--state",
            "bogus",
            "--out",
            "target/glorp-review/pair",
        ])
        .unwrap_err()
        .contains("state must be"));
    }

    fn valid_pair_manifest_value() -> serde_json::Value {
        serde_json::json!({
            "status": "success",
            "smooth": { "effective_renderer": "smooth", "png_path": "smooth.png" },
            "retained": {
                "effective_renderer": "retained",
                "milestones": ["gpu-completed", "readback-completed"],
                "png_path": "retained.png",
            },
        })
    }

    #[test]
    fn pair_manifest_validation_accepts_a_successful_pair() {
        validate_pair_manifest_value(&valid_pair_manifest_value()).unwrap();
    }

    #[test]
    fn pair_manifest_validation_rejects_a_smooth_fallback_retained_half() {
        let mut manifest = valid_pair_manifest_value();
        manifest["retained"]["effective_renderer"] = serde_json::json!("smooth");
        assert!(validate_pair_manifest_value(&manifest).is_err());
    }

    #[test]
    fn pair_manifest_validation_rejects_a_missing_readback_milestone() {
        let mut manifest = valid_pair_manifest_value();
        manifest["retained"]["milestones"] = serde_json::json!(["gpu-completed"]);
        assert_eq!(
            validate_pair_manifest_value(&manifest).unwrap_err(),
            "retained capture missing readback-completed"
        );
    }

    #[test]
    fn pair_manifest_validation_rejects_a_failed_status() {
        let mut manifest = valid_pair_manifest_value();
        manifest["status"] = serde_json::json!("failed");
        assert!(validate_pair_manifest_value(&manifest).is_err());
    }

    #[test]
    fn review_pair_reset_removes_a_stale_manifest_left_by_a_prior_run() {
        let unique = format!(
            "glorp-xtask-review-pair-reset-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("pair-manifest.json"),
            serde_json::to_vec(&valid_pair_manifest_value()).unwrap(),
        )
        .unwrap();

        reset_review_out_dir(&root).unwrap();

        assert!(!root.exists());
    }

    #[test]
    fn review_pair_reset_tolerates_an_output_directory_that_does_not_exist() {
        let unique = format!(
            "glorp-xtask-review-pair-reset-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        assert!(!root.exists());

        reset_review_out_dir(&root).unwrap();
    }

    #[test]
    fn parses_renderer_spike_run() {
        assert_eq!(
            parse_args([
                "renderer-spike",
                "run",
                "--candidate",
                "smooth",
                "--track",
                "capture",
                "--size",
                "360",
                "--duration-ms",
                "500",
                "--out",
                "target/renderer-spikes/test",
            ]),
            Ok(XtaskCommand::RendererSpikeRun {
                candidate: "smooth".into(),
                track: "capture".into(),
                size: 360,
                duration_ms: 500,
                out: "target/renderer-spikes/test".into(),
            })
        );
    }

    #[test]
    fn parses_software_renderer_spike_run() {
        assert_eq!(
            parse_args([
                "renderer-spike",
                "run",
                "--candidate",
                "software",
                "--track",
                "ambient",
                "--size",
                "720",
                "--out",
                "target/renderer-spikes/software-test",
            ]),
            Ok(XtaskCommand::RendererSpikeRun {
                candidate: "software".into(),
                track: "ambient".into(),
                size: 720,
                duration_ms: 2_000,
                out: "target/renderer-spikes/software-test".into(),
            })
        );
    }

    #[test]
    fn renderer_spike_run_rejects_unfair_size() {
        let err = parse_args([
            "renderer-spike",
            "run",
            "--candidate",
            "smooth",
            "--track",
            "capture",
            "--size",
            "480",
            "--out",
            "target/renderer-spikes/test",
        ])
        .unwrap_err();
        assert!(err.contains("size must be 360 or 720"));
    }

    #[test]
    fn renderer_spike_run_rejects_output_outside_owned_root() {
        for out in [
            "/tmp/glorp-renderer-spike",
            "target/renderer-spikes/../../../private/glorp",
            "target/not-renderer-spikes/test",
        ] {
            let error = parse_args([
                "renderer-spike",
                "run",
                "--candidate",
                "wgpu",
                "--track",
                "ambient",
                "--size",
                "360",
                "--out",
                out,
            ])
            .unwrap_err();
            assert!(error.contains("below `target/renderer-spikes`"));
        }
    }

    #[test]
    fn parses_renderer_spike_validate() {
        assert_eq!(
            parse_args([
                "renderer-spike",
                "validate",
                "--out",
                "target/renderer-spikes/test",
            ]),
            Ok(XtaskCommand::RendererSpikeValidate {
                out: "target/renderer-spikes/test".into(),
            })
        );
    }

    #[test]
    fn parses_renderer_spike_qualification_with_frozen_relative_binary() {
        assert_eq!(
            parse_args([
                "renderer-spike",
                "qualify",
                "--binary",
                "target/renderer-spikes/bin/glorp-wgpu-qualified",
                "--target",
                "aarch64-apple-darwin",
                "--candidate",
                "wgpu",
                "--track",
                "ambient",
                "--size",
                "720",
                "--duration-ms",
                "300000",
                "--out",
                "target/renderer-spikes/wgpu-qualified-matched-720/block-1/2-wgpu",
            ]),
            Ok(XtaskCommand::RendererSpikeQualify {
                binary: "target/renderer-spikes/bin/glorp-wgpu-qualified".into(),
                target: "aarch64-apple-darwin".into(),
                candidate: "wgpu".into(),
                track: "ambient".into(),
                size: 720,
                duration_ms: 300_000,
                out: "target/renderer-spikes/wgpu-qualified-matched-720/block-1/2-wgpu".into(),
            })
        );
    }

    #[test]
    fn qualification_rejects_absolute_or_parent_paths() {
        for binary in [
            "/Users/private/glorp",
            "target/renderer-spikes/bin/../../../private/glorp",
        ] {
            let error = parse_args([
                "renderer-spike",
                "qualify",
                "--binary",
                binary,
                "--target",
                "aarch64-apple-darwin",
                "--candidate",
                "wgpu",
                "--track",
                "ambient",
                "--size",
                "360",
                "--out",
                "target/renderer-spikes/qualified/test",
            ])
            .unwrap_err();
            assert!(error.contains("private-data-free relative path"));
        }
    }

    #[test]
    fn renderer_spike_validate_rejects_manifest_hash_mismatch() {
        let unique = format!(
            "glorp-xtask-renderer-spike-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("process-cleanup.json"),
            r#"{"process_exited": true, "surviving_pids": []}"#,
        )
        .unwrap();
        std::fs::write(root.join("privacy-scan.json"), r#"{"passed": true}"#).unwrap();
        std::fs::write(root.join("environment.json"), r#"{"candidate":"smooth"}"#).unwrap();
        std::fs::write(root.join("fixture.json"), "original").unwrap();
        let fixture = std::fs::read(root.join("fixture.json")).unwrap();
        let manifest = serde_json::json!({
            "artifacts": [{
                "path": "fixture.json",
                "bytes": fixture.len(),
                "sha256": format!("{:x}", Sha256::digest(&fixture)),
            }]
        });
        std::fs::write(
            root.join("run-manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(root.join("fixture.json"), "changed!").unwrap();

        let parent = root.parent().unwrap();
        let out = root.file_name().unwrap().to_string_lossy();
        let error = validate_renderer_spike(parent, &out).unwrap_err();
        assert!(
            error.contains("artifact byte mismatch") || error.contains("artifact hash mismatch")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrected_wgpu_upload_validation_accepts_one_static_generation() {
        let unique = format!(
            "glorp-xtask-wgpu-upload-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("environment.json"), r#"{"candidate":"wgpu"}"#).unwrap();
        write_valid_startup_fixture(&root);
        write_valid_accessibility_fixture(&root);
        let rows = [
            serde_json::json!({
                "upload_bytes": 0,
                "static_upload_bytes": 0,
                "dynamic_upload_bytes": 0,
                "atlas_upload_bytes": 0,
                "uniform_upload_bytes": 0,
                "draw_calls": 0,
            }),
            serde_json::json!({
                "upload_bytes": 21_856,
                "static_upload_bytes": 19_200,
                "dynamic_upload_bytes": 2_400,
                "atlas_upload_bytes": 256,
                "uniform_upload_bytes": 0,
                "draw_calls": 1,
            }),
            serde_json::json!({
                "upload_bytes": 2_400,
                "static_upload_bytes": 0,
                "dynamic_upload_bytes": 2_400,
                "atlas_upload_bytes": 0,
                "uniform_upload_bytes": 0,
                "draw_calls": 1,
            }),
        ];
        let metrics = rows
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");
        std::fs::write(root.join("frame-metrics.jsonl"), metrics).unwrap();
        validate_corrected_wgpu_uploads(&root).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrected_wgpu_upload_validation_rejects_repeated_static_bytes() {
        let unique = format!(
            "glorp-xtask-wgpu-repeat-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("environment.json"), r#"{"candidate":"wgpu"}"#).unwrap();
        write_valid_startup_fixture(&root);
        write_valid_accessibility_fixture(&root);
        let row = serde_json::json!({
            "upload_bytes": 21_856,
            "static_upload_bytes": 19_200,
            "dynamic_upload_bytes": 2_400,
            "atlas_upload_bytes": 256,
            "uniform_upload_bytes": 0,
            "draw_calls": 1,
        });
        std::fs::write(
            root.join("frame-metrics.jsonl"),
            format!("{}\n{}\n", row, row),
        )
        .unwrap();
        assert!(validate_corrected_wgpu_uploads(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wgpu_startup_validation_accepts_monotonic_derived_durations() {
        let unique = format!(
            "glorp-xtask-wgpu-startup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("startup.json"),
            serde_json::to_vec(&serde_json::json!({
                "clock": "mach-continuous-time-micros",
                "runner_entry_micros": 100,
                "harness_entry_micros": 125,
                "host_ready_micros": 225,
                "first_present_micros": 300,
                "runner_to_harness_micros": 25,
                "runner_to_host_ready_micros": 125,
                "runner_to_first_present_micros": 200,
                "host_ready_to_first_present_micros": 75,
            }))
            .unwrap(),
        )
        .unwrap();
        validate_wgpu_startup_artifact(&root).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    fn write_valid_startup_fixture(root: &Path) {
        std::fs::write(
            root.join("startup.json"),
            serde_json::to_vec(&serde_json::json!({
                "clock": "mach-continuous-time-micros",
                "runner_entry_micros": 100,
                "harness_entry_micros": 125,
                "host_ready_micros": 225,
                "first_present_micros": 300,
                "runner_to_harness_micros": 25,
                "runner_to_host_ready_micros": 125,
                "runner_to_first_present_micros": 200,
                "host_ready_to_first_present_micros": 75,
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn write_valid_accessibility_fixture(root: &Path) {
        std::fs::write(
            root.join("summary.json"),
            serde_json::to_vec(&serde_json::json!({
                "verdict": "host-functional-pass",
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            root.join("accessibility-audit.json"),
            serde_json::to_vec(&serde_json::json!({
                "group_count": 1,
                "value_count": 3,
                "child_count": 4,
                "per_glyph_children": false,
                "sanitized": true,
                "synthetic_mouse_event_delivered": true,
                "synthetic_key_event_delivered": true,
                "stale_snapshot_rejected": true,
                "current_snapshot_accepted": true,
                "first_responder": true,
                "hide_children_detached": false,
                "reveal_children_restored": false,
                "close_children_detached": true,
            }))
            .unwrap(),
        )
        .unwrap();
    }
}
