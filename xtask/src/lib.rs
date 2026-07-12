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

const USAGE: &str = "Usage:\n  cargo xtask companion fresh [--debug|--release]\n  cargo xtask companion review-pair --size N [--state STATE] [--dimmed] [--live-values] --out DIR\n  cargo xtask companion scene-baseline --duration-ms N --out PATH\n  cargo xtask renderer-spike validate --out DIR\n  cargo xtask renderer-spike run --candidate smooth|wgpu|software --track TRACK --size 360|720 --duration-ms N --out DIR\n  cargo xtask renderer-spike qualify --binary target/renderer-spikes/bin/FILE --target TARGET --candidate smooth|wgpu --track TRACK --size 360|720 --duration-ms N --out target/renderer-spikes/DIR";

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
    let config_dir = work.join("config");
    let metrics_path = work.join("runtime-metrics.json");
    let capture_dir = repo_root.join("target/glorp-review/scene-baseline");
    match std::fs::remove_dir_all(&capture_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }

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
        Duration::from_millis(duration_ms.saturating_add(60_000)),
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
    validate_runtime_snapshot(&snapshot)?;
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
        "--review-size".into(),
        "360x360".into(),
        "--review-duration-ms".into(),
        duration_ms.to_string(),
        "--review-capture-dir".into(),
        "target/glorp-review/scene-baseline".into(),
        "--review-runtime-metrics-out".into(),
        metrics_path.to_string_lossy().into_owned(),
    ]
}

struct BaselineSourceIdentity {
    commit: String,
    tracked_tree_state: String,
}

fn validate_runtime_snapshot(snapshot: &serde_json::Value) -> Result<(), String> {
    if snapshot
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(2)
    {
        return Err("runtime metrics snapshot schema_version is not 2".to_string());
    }
    for metric in [
        "ui_tick_us",
        "state_prepare_us",
        "gpu_translate_us",
        "encode_us",
        "queue_wait_us",
        "compile_us",
        "activation_render_owner_us",
    ] {
        for percentile in ["p50", "p95", "p99"] {
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
    for field in [
        "fixture",
        "hidden_segment",
        "gpu_accounting",
        "lifetime_audit",
    ] {
        if snapshot.get(field).is_none()
            || snapshot.get(field).is_some_and(serde_json::Value::is_null)
        {
            return Err(format!("runtime metrics snapshot missing {field}"));
        }
    }
    for field in ["capture_attempted", "capture_succeeded", "capture_failed"] {
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
    let lifetime = snapshot
        .get("lifetime_audit")
        .ok_or_else(|| "runtime metrics snapshot missing lifetime_audit".to_string())?;
    for field in [
        "frames",
        "warmup_frames",
        "cadence_ms",
        "virtual_elapsed_ms",
        "prepared_frames",
        "encoded_frames",
        "semantic_frame_changes",
        "gpu_frame_hash_changes",
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
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateStatus {
    Pass,
    AcceptedPreexisting,
    Fail,
}

impl GateStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::AcceptedPreexisting => "ACCEPTED_PREEXISTING",
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
    const APPKIT_TARGET_US: u64 = 4_000;
    const APPKIT_STAGE0_EXCEPTION_CEILING_US: u64 = 20_000;
    const APPKIT_STAGE0_DISPOSITION: &str = "stage0-appkit-raster-v1";
    let ui_p95 = snapshot_u64(snapshot, &["ui_tick_us", "p95"])?;
    let ui_p99 = snapshot_u64(snapshot, &["ui_tick_us", "p99"])?;
    let encode_p95 = snapshot_u64(snapshot, &["encode_us", "p95"])?;
    let compile_p95 = snapshot_u64(snapshot, &["compile_us", "p95"])?;
    let activation_p95 = snapshot_u64(snapshot, &["activation_render_owner_us", "p95"])?;
    let ui_p95_limit = 8_000_u64.min(((ui_p95 * 110).div_ceil(100)).max(ui_p95 + 500));
    let ui_p99_limit = 16_000_u64.min(((ui_p99 * 115).div_ceil(100)).max(ui_p99 + 1_000));
    let encode_limit = ((encode_p95 * 110).div_ceil(100)).max(encode_p95 + 250);
    let overhead_ns = snapshot_u64(snapshot, &["metrics_overhead_control", "net_ns_per_tick"])?;
    let overhead_limit_ns = ui_p95.saturating_mul(1_000).saturating_mul(2) / 100;
    let hidden = snapshot
        .get("hidden_segment")
        .ok_or_else(|| "runtime snapshot missing hidden_segment".to_string())?;
    let hidden_delta = hidden
        .get("steady_delta")
        .ok_or_else(|| "runtime snapshot missing hidden_segment.steady_delta".to_string())?;
    let hidden_zero = [
        "prepare",
        "queue_writes",
        "surface_acquires",
        "encode",
        "submit",
    ]
    .iter()
    .all(|field| value_u64(hidden_delta, field) == Ok(0));
    let hidden_ticks = value_u64(hidden, "steady_ticks")?;
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

    let mut appkit = gate(
        "appkit-raster-slice",
        compile_p95 <= APPKIT_TARGET_US,
        format!("{compile_p95} us"),
        format!("{APPKIT_TARGET_US} us target; {APPKIT_STAGE0_EXCEPTION_CEILING_US} us frozen exception ceiling"),
    );
    if compile_p95 > APPKIT_TARGET_US && compile_p95 <= APPKIT_STAGE0_EXCEPTION_CEILING_US {
        appkit.status = GateStatus::AcceptedPreexisting;
        appkit.disposition = format!(
            "{APPKIT_STAGE0_DISPOSITION}: bounded pre-existing Stage-0 miss; post-Task-6 <=4000us sliced AppKit preparation prerequisite must pass before Task 7"
        );
    }

    Ok(vec![
        gate(
            "ui-tick-p95",
            ui_p95 <= ui_p95_limit,
            format!("{ui_p95} us"),
            format!("{ui_p95_limit} us"),
        ),
        gate(
            "ui-tick-p99",
            ui_p99 <= ui_p99_limit,
            format!("{ui_p99} us"),
            format!("{ui_p99_limit} us"),
        ),
        gate(
            "encode-p95",
            encode_p95 <= encode_limit,
            format!("{encode_p95} us"),
            format!("{encode_limit} us"),
        ),
        appkit,
        gate(
            "activation-render-owner-slice",
            activation_p95 <= 16_000,
            format!("{activation_p95} us"),
            "16000 us",
        ),
        gate(
            "metrics-overhead-control",
            overhead_ns <= overhead_limit_ns,
            format!("{overhead_ns} ns/tick"),
            format!("{overhead_limit_ns} ns/tick (2% UI p95)"),
        ),
        gate(
            "hidden-steady-state",
            hidden_ticks >= 2 && hidden_zero,
            format!("{hidden_ticks} steady ticks; delta={hidden_delta}"),
            "after 1 transition tick, >=2 steady ticks and all work counters 0",
        ),
        gate(
            "post-warmup-persistent-gpu-creations",
            snapshot_u64(snapshot, &["persistent_gpu_objects_created"])? == 0,
            snapshot_u64(snapshot, &["persistent_gpu_objects_created"])?.to_string(),
            "0",
        ),
        gate(
            "post-warmup-static-upload-bytes",
            snapshot_u64(snapshot, &["static_upload_bytes"])? == 0,
            snapshot_u64(snapshot, &["static_upload_bytes"])?.to_string(),
            "0",
        ),
        gate(
            "lifetime-frame-count-and-cadence",
            value_u64(lifetime, "frames")? == 4_500
                && value_u64(lifetime, "warmup_frames")? == 4_500
                && value_u64(lifetime, "cadence_ms")? == 250
                && value_u64(lifetime, "virtual_elapsed_ms")? == 1_125_000,
            format!(
                "{} warmup + {} measured frames @ {} ms; {} ms elapsed",
                value_u64(lifetime, "warmup_frames")?,
                value_u64(lifetime, "frames")?,
                value_u64(lifetime, "cadence_ms")?,
                value_u64(lifetime, "virtual_elapsed_ms")?,
            ),
            "identical 4500 warmup + measured schedules @ 250 ms; 1125000 ms elapsed",
        ),
        gate(
            "lifetime-production-work",
            value_u64(lifetime, "prepared_frames")? == 4_500
                && value_u64(lifetime, "encoded_frames")? == 4_500
                && value_u64(lifetime, "semantic_frame_changes")? > 0
                && value_u64(lifetime, "gpu_frame_hash_changes")? > 0
                && value_u64(lifetime, "draw_calls")? > 0
                && value_u64(lifetime, "poll_count")? > 0,
            format!(
                "prepared={} encoded={} semantic-changes={} gpu-hash-changes={} draws={} polls={}",
                value_u64(lifetime, "prepared_frames")?,
                value_u64(lifetime, "encoded_frames")?,
                value_u64(lifetime, "semantic_frame_changes")?,
                value_u64(lifetime, "gpu_frame_hash_changes")?,
                value_u64(lifetime, "draw_calls")?,
                value_u64(lifetime, "poll_count")?,
            ),
            "4500 actual prepares/encodes, varying semantic/GPU hashes, nonzero draws/polls",
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
    ])
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
    let hardware = required_command_output(repo_root, "sysctl", &["-n", "hw.model"])?;
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
| AppKit raster/compile slice | {} | {} | {} |\n\
| First-present activation render-owner boundary (excluding separately measured AppKit raster) | {} | {} | {} |\n\n\
Metrics overhead uses {} alternating on/off-control trials of {} representative complete metric ticks ({} control / {} instrumented); \
control {} ns/tick, instrumented {} ns/tick, net {} ns/tick.\n\n\
Accounted persistent GPU bytes: atlas {}, instance ring {}, capture {}, current total {}, concurrent-replacement peak {}. \
Persistent GPU objects: host {}, atlas {}, instance ring {}, capture {}, current total {}, concurrent-replacement peak {}; lifecycle created/destroyed {}/{}. \
Opaque driver allocations are covered by process RSS, not guessed into GPU byte accounting.\n\n\
Lifetime segment: {} warmup + {} measured real prepared/encoded GPU frames at {} ms virtual cadence ({} ms semantic time); \
prepared {}, encoded {}, semantic changes {}, GPU hash changes {}, draws {}, polls {}; \
RSS warmup-end/warmup-high-water/final/peak {}/{}/{}/{}, accounted GPU warmup-end/warmup-high-water/final/peak {}/{}/{}/{}.\n\n\
## Structured gate results\n\n\
| Gate | Status | Measured | Limit | Disposition |\n\
|---|---|---|---|---|\n\
{gate_rows}\n\n\
The AppKit result is a versioned, bounded pre-existing Stage-0 disposition, not a pass. The explicit post-Task-6 <=4000 us sliced AppKit preparation prerequisite must pass before Task 7 begins. Stage 0 is therefore not globally green. Every other failed or unmeasured gate makes the command fail.\n\n\
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
        value_u64(fixture, "cadence_ms")?,
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
        snapshot_u64(snapshot, &["compile_us", "p50"])? ,
        snapshot_u64(snapshot, &["compile_us", "p95"])? ,
        snapshot_u64(snapshot, &["compile_us", "p99"])? ,
        snapshot_u64(snapshot, &["activation_render_owner_us", "p50"])? ,
        snapshot_u64(snapshot, &["activation_render_owner_us", "p95"])? ,
        snapshot_u64(snapshot, &["activation_render_owner_us", "p99"])? ,
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
        value_u64(lifetime, "warmup_frames")?,
        value_u64(lifetime, "frames")?,
        value_u64(lifetime, "cadence_ms")?,
        value_u64(lifetime, "virtual_elapsed_ms")?,
        value_u64(lifetime, "prepared_frames")?,
        value_u64(lifetime, "encoded_frames")?,
        value_u64(lifetime, "semantic_frame_changes")?,
        value_u64(lifetime, "gpu_frame_hash_changes")?,
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
    fn scene_baseline_requests_a_real_paired_gpu_capture() {
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
    }

    #[test]
    fn baseline_gates_accept_only_the_versioned_appkit_disposition() {
        let snapshot = baseline_gate_fixture();
        let gates = evaluate_baseline_gates(&snapshot).unwrap();
        assert!(gates.iter().all(|gate| {
            gate.status == GateStatus::Pass
                || (gate.id == "appkit-raster-slice"
                    && gate.status == GateStatus::AcceptedPreexisting)
        }));
        assert!(validate_gate_results(&gates).is_ok());
    }

    #[test]
    fn baseline_gates_reject_any_other_miss() {
        let mut snapshot = baseline_gate_fixture();
        snapshot["activation_render_owner_us"]["p95"] = serde_json::json!(16_001);
        let gates = evaluate_baseline_gates(&snapshot).unwrap();
        let error = validate_gate_results(&gates).unwrap_err();
        assert!(error.contains("activation-render-owner-slice"));
    }

    #[test]
    fn baseline_gates_keep_appkit_miss_as_non_pass_at_unchanged_limit() {
        let mut snapshot = baseline_gate_fixture();
        snapshot["compile_us"]["p95"] = serde_json::json!(12_000);
        let gates = evaluate_baseline_gates(&snapshot).unwrap();
        let appkit = gates
            .iter()
            .find(|gate| gate.id == "appkit-raster-slice")
            .unwrap();
        assert!(appkit.limit.contains("4000 us target"));
        assert!(appkit.limit.contains("20000 us frozen exception ceiling"));
        assert_eq!(appkit.status, GateStatus::AcceptedPreexisting);
        assert!(validate_gate_results(&gates).is_ok());
    }

    #[test]
    fn baseline_gates_reject_appkit_regressions_above_versioned_stage0_ceiling() {
        let mut snapshot = baseline_gate_fixture();
        snapshot["compile_us"]["p95"] = serde_json::json!(20_001);
        let gates = evaluate_baseline_gates(&snapshot).unwrap();
        let appkit = gates
            .iter()
            .find(|gate| gate.id == "appkit-raster-slice")
            .unwrap();
        assert_eq!(appkit.status, GateStatus::Fail);
        assert!(validate_gate_results(&gates).is_err());
    }

    fn baseline_gate_fixture() -> serde_json::Value {
        serde_json::json!({
            "ui_tick_us": {"p95": 1000, "p99": 1500},
            "encode_us": {"p95": 100},
            "compile_us": {"p95": 12000},
            "activation_render_owner_us": {"p95": 8000},
            "metrics_overhead_control": {"net_ns_per_tick": 1000},
            "capture_attempted": 1,
            "capture_succeeded": 1,
            "capture_failed": 0,
            "persistent_gpu_objects_created": 0,
            "static_upload_bytes": 0,
            "hidden_segment": {
                "transition_ticks": 1,
                "steady_ticks": 2,
                "steady_delta": {"prepare": 0, "queue_writes": 0, "surface_acquires": 0, "encode": 0, "submit": 0}
            },
            "lifetime_audit": {
                "frames": 4500,
                "warmup_frames": 4500,
                "cadence_ms": 250,
                "virtual_elapsed_ms": 1125000,
                "prepared_frames": 4500,
                "encoded_frames": 4500,
                "semantic_frame_changes": 10,
                "gpu_frame_hash_changes": 10,
                "draw_calls": 10000,
                "poll_count": 10,
                "rss_warmup_bytes": 1000000,
                "rss_warmup_peak_bytes": 1000000,
                "rss_final_bytes": 1005000,
                "rss_peak_bytes": 1009000,
                "gpu_warmup_bytes": 1000000,
                "gpu_warmup_peak_bytes": 1000000,
                "gpu_final_bytes": 1005000,
                "gpu_peak_bytes": 1009000
            }
        })
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
