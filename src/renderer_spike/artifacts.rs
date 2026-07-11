use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{GlorpError, Result};

use super::{RendererSpikeCandidate, RendererSpikeOptions, RendererSpikeTrack};

pub const ARTIFACT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema_version: u16,
    pub run_id: String,
    pub required: Vec<String>,
    pub artifacts: Vec<ArtifactEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentArtifact {
    pub schema_version: u16,
    pub candidate: RendererSpikeCandidate,
    pub track: RendererSpikeTrack,
    pub logical_size: u16,
    pub duration_ms: u64,
    pub git_commit: Option<String>,
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryArtifact {
    pub schema_version: u16,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub profile: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupArtifact {
    pub schema_version: u16,
    pub process_exited: bool,
    pub surviving_pids: Vec<u32>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupArtifact {
    pub schema_version: u16,
    pub clock: String,
    pub runner_entry_micros: u64,
    pub harness_entry_micros: u64,
    pub host_ready_micros: u64,
    pub first_present_micros: Option<u64>,
    pub runner_to_harness_micros: u64,
    pub runner_to_host_ready_micros: u64,
    pub runner_to_first_present_micros: Option<u64>,
    pub host_ready_to_first_present_micros: Option<u64>,
}

impl StartupArtifact {
    pub fn from_checkpoints(
        runner_entry_micros: u64,
        harness_entry_micros: u64,
        host_ready_micros: u64,
        first_present_micros: Option<u64>,
    ) -> Result<Self> {
        if harness_entry_micros < runner_entry_micros
            || host_ready_micros < harness_entry_micros
            || first_present_micros.is_some_and(|value| value < host_ready_micros)
        {
            return Err(GlorpError::Message(
                "renderer spike startup checkpoints are not monotonic".into(),
            ));
        }
        Ok(Self {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            clock: "mach-continuous-time-micros".to_string(),
            runner_entry_micros,
            harness_entry_micros,
            host_ready_micros,
            first_present_micros,
            runner_to_harness_micros: harness_entry_micros - runner_entry_micros,
            runner_to_host_ready_micros: host_ready_micros - runner_entry_micros,
            runner_to_first_present_micros: first_present_micros
                .map(|value| value - runner_entry_micros),
            host_ready_to_first_present_micros: first_present_micros
                .map(|value| value - host_ready_micros),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostBoundaryArtifact {
    pub schema_version: u16,
    pub candidate: RendererSpikeCandidate,
    pub owner: String,
    pub owner_thread: String,
    pub observed_threads: Vec<HostBoundaryObservation>,
    pub call_sequence: Vec<String>,
    pub owner_assertions_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostBoundaryObservation {
    pub operation: String,
    pub thread: String,
    pub main_thread: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameMetric {
    pub frame_index: u64,
    pub elapsed_ms: u64,
    pub end_to_end_cpu_micros: u64,
    pub requested_visible_frames: u64,
    pub completed_visible_frames: u64,
    pub submissions: u64,
    pub missed_deadlines: u64,
    pub primitive_count: u32,
    pub static_rebuilds: u64,
    pub atlas_misses: u64,
    pub upload_bytes: u64,
    pub static_upload_bytes: u64,
    pub dynamic_upload_bytes: u64,
    pub atlas_upload_bytes: u64,
    pub uniform_upload_bytes: u64,
    pub resource_generation: u64,
    pub draw_calls: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryArtifact {
    pub schema_version: u16,
    pub candidate: RendererSpikeCandidate,
    pub track: RendererSpikeTrack,
    pub verdict: String,
    pub cpu_measured: bool,
    pub sample_count: usize,
    pub cpu_mean: f64,
    pub cpu_median: f64,
    pub cpu_p95: f64,
    pub privacy_passed: bool,
    pub cleanup_passed: bool,
}

pub fn required_artifacts(
    candidate: RendererSpikeCandidate,
    track: RendererSpikeTrack,
    logical_size: u16,
) -> Vec<String> {
    let mut required = vec![
        "environment.json".to_string(),
        "binary.json".to_string(),
        "fixture.json".to_string(),
        "atlas.json".to_string(),
        "accessibility-tree.json".to_string(),
        "privacy-scan.json".to_string(),
        "process-cleanup.json".to_string(),
        "summary.json".to_string(),
    ];
    if matches!(
        candidate,
        RendererSpikeCandidate::Wgpu | RendererSpikeCandidate::Software
    ) {
        required.push("host-boundary.json".to_string());
        required.push("frame-metrics.jsonl".to_string());
    }
    if candidate == RendererSpikeCandidate::Wgpu {
        required.push("startup.json".to_string());
        required.push("accessibility-audit.json".to_string());
    }
    if candidate == RendererSpikeCandidate::Software {
        required.push("software-resource.json".to_string());
    }
    if matches!(track, RendererSpikeTrack::Capture) {
        required.push(format!("captures/capture-{logical_size}-frame-000005.png"));
        required.push(format!("captures/capture-{logical_size}-frame-000005.json"));
    }
    required
}

pub fn write_scaffold_run(options: &RendererSpikeOptions) -> Result<()> {
    write_common_artifacts(options)?;
    super::privacy::write_privacy_scan(&options.out)?;
    write_manifest(
        &options.out,
        options.candidate,
        options.track,
        options.logical_size,
    )?;
    Ok(())
}

pub fn write_common_artifacts(options: &RendererSpikeOptions) -> Result<()> {
    std::fs::create_dir_all(&options.out)?;
    let fixture = super::fixture::canonical_fixture();
    let atlas = super::fixture::canonical_atlas();
    let semantics = super::fixture::semantic_fixture(options.logical_size, false);
    write_json(&options.out.join("fixture.json"), &fixture)?;
    write_json(&options.out.join("atlas.json"), &atlas)?;
    write_json(&options.out.join("accessibility-tree.json"), &semantics)?;
    write_json(
        &options.out.join("environment.json"),
        &EnvironmentArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            candidate: options.candidate,
            track: options.track,
            logical_size: options.logical_size,
            duration_ms: options.duration_ms,
            git_commit: git_output(["rev-parse", "HEAD"]),
            dirty: git_output(["status", "--porcelain"]).is_some_and(|value| !value.is_empty()),
        },
    )?;
    let executable = std::env::current_exe()?;
    let executable_bytes = std::fs::read(&executable)?;
    let executable_display = std::env::current_dir()
        .ok()
        .and_then(|cwd| executable.strip_prefix(cwd).ok().map(normalize_path))
        .unwrap_or_else(|| {
            executable.file_name().map_or_else(
                || "glorp".to_string(),
                |name| name.to_string_lossy().to_string(),
            )
        });
    write_json(
        &options.out.join("binary.json"),
        &BinaryArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            path: executable_display,
            bytes: executable_bytes.len() as u64,
            sha256: sha256_hex(&executable_bytes),
            profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
            features: if cfg!(feature = "renderer-spike-wgpu") {
                vec![
                    "renderer-spike".to_string(),
                    "renderer-spike-wgpu".to_string(),
                ]
            } else {
                vec!["renderer-spike".to_string()]
            },
        },
    )?;
    write_json(
        &options.out.join("process-cleanup.json"),
        &CleanupArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            process_exited: true,
            surviving_pids: Vec::new(),
            timed_out: false,
        },
    )?;
    write_json(
        &options.out.join("summary.json"),
        &SummaryArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            candidate: options.candidate,
            track: options.track,
            verdict: "not-implemented".to_string(),
            cpu_measured: false,
            sample_count: 0,
            cpu_mean: 0.0,
            cpu_median: 0.0,
            cpu_p95: 0.0,
            privacy_passed: true,
            cleanup_passed: true,
        },
    )?;
    Ok(())
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn write_manifest(
    root: &Path,
    candidate: RendererSpikeCandidate,
    track: RendererSpikeTrack,
    logical_size: u16,
) -> Result<()> {
    let required = required_artifacts(candidate, track, logical_size);
    let required_set = required.iter().cloned().collect::<BTreeSet<_>>();
    let mut artifacts = Vec::new();
    collect_files(root, root, &mut artifacts)?;
    let found = artifacts
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    let missing = required_set.difference(&found).cloned().collect::<Vec<_>>();
    let manifest = RunManifest {
        schema_version: ARTIFACT_SCHEMA_VERSION,
        run_id: root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("renderer-spike")
            .to_string(),
        required,
        artifacts,
    };
    write_json(&root.join("run-manifest.json"), &manifest)?;
    if !missing.is_empty() {
        return Err(GlorpError::Message(format!(
            "renderer spike is missing required artifacts: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

pub fn validate_run(root: &Path) -> Result<RunManifest> {
    let bytes = std::fs::read(root.join("run-manifest.json"))?;
    let manifest: RunManifest = serde_json::from_slice(&bytes)?;
    let mut found = BTreeSet::new();
    for entry in &manifest.artifacts {
        let relative = Path::new(&entry.path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            || !found.insert(entry.path.clone())
        {
            return Err(GlorpError::Message(format!(
                "renderer spike manifest has unsafe or duplicate artifact path: {}",
                entry.path
            )));
        }
        let path = root.join(&entry.path);
        let bytes = std::fs::read(&path)?;
        if bytes.len() as u64 != entry.bytes || sha256_hex(&bytes) != entry.sha256 {
            return Err(GlorpError::Message(format!(
                "renderer spike artifact hash mismatch: {}",
                entry.path
            )));
        }
    }
    let missing = manifest
        .required
        .iter()
        .filter(|required| !found.contains(*required))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(GlorpError::Message(format!(
            "renderer spike manifest is missing required artifacts: {}",
            missing.join(", ")
        )));
    }
    Ok(manifest)
}

fn collect_files(root: &Path, directory: &Path, entries: &mut Vec<ArtifactEntry>) -> Result<()> {
    for item in std::fs::read_dir(directory)? {
        let item = item?;
        let path = item.path();
        if path == root.join("run-manifest.json") {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, entries)?;
        } else {
            let bytes = std::fs::read(&path)?;
            let relative = path.strip_prefix(root).map_err(|_| {
                GlorpError::Message("renderer spike artifact escaped owned directory".into())
            })?;
            entries.push(ArtifactEntry {
                path: normalize_path(relative),
                bytes: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            });
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(())
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn aggregate_samples(samples: &[f64]) -> Option<(f64, f64, f64)> {
    if samples.is_empty() || samples.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    let rank = ((sorted.len() as f64) * 0.95).ceil() as usize;
    let p95 = sorted[rank.saturating_sub(1).min(sorted.len() - 1)];
    Some((mean, median, p95))
}

pub fn missed_frame_percent(requested_visible_frames: u64, missed_deadlines: u64) -> Option<f64> {
    (requested_visible_frames != 0).then(|| {
        missed_deadlines.min(requested_visible_frames) as f64 / requested_visible_frames as f64
            * 100.0
    })
}

pub fn run_median_divergence_percent(run_medians: &[f64]) -> Option<f64> {
    let (_, median, _) = aggregate_samples(run_medians)?;
    if median == 0.0 {
        return run_medians.iter().all(|value| *value == 0.0).then_some(0.0);
    }
    let minimum = run_medians.iter().copied().reduce(f64::min)?;
    let maximum = run_medians.iter().copied().reduce(f64::max)?;
    Some((maximum - minimum) / median.abs() * 100.0)
}

pub fn owned_path(root: &Path, relative: &str) -> PathBuf {
    root.join(relative)
}

#[cfg(target_os = "macos")]
pub fn monotonic_micros() -> u64 {
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

#[cfg(not(target_os = "macos"))]
pub fn monotonic_micros() -> u64 {
    0
}
