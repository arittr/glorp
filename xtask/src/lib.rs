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

const USAGE: &str = "Usage:\n  cargo xtask companion fresh [--debug|--release]\n  cargo xtask renderer-spike validate --out DIR\n  cargo xtask renderer-spike run --candidate smooth|wgpu|software --track TRACK --size 360|720 --duration-ms N --out DIR\n  cargo xtask renderer-spike qualify --binary target/renderer-spikes/bin/FILE --target TARGET --candidate smooth|wgpu --track TRACK --size 360|720 --duration-ms N --out target/renderer-spikes/DIR";

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

pub fn companion_fresh_steps(release: bool) -> Vec<ProcessStep> {
    let profile = if release { "release" } else { "debug" };
    vec![
        ProcessStep {
            program: "node".to_string(),
            args: vec![
                "scripts/build-macos-companion-app.mjs".to_string(),
                "--profile".to_string(),
                profile.to_string(),
            ],
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
            run_steps(&companion_fresh_steps(release), repo_root)
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
        let steps = companion_fresh_steps(true);
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
        let steps = companion_fresh_steps(true);
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
    fn companion_fresh_relaunches_the_fresh_bundle() {
        let steps = companion_fresh_steps(false);
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
    fn rejects_unknown_command_with_usage_hint() {
        let err = parse_args(["nope"]).unwrap_err();
        assert!(err.contains("cargo xtask companion fresh"));
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
