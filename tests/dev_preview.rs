#![cfg(feature = "dev-preview")]

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

const LIVELINESS_WATCH_IDS: [&str; 7] = [
    "watch-liveliness-s6-idle-dawn",
    "watch-liveliness-s6-warm-midday",
    "watch-liveliness-s6-hot-midday",
    "watch-liveliness-s6-cooling-evening",
    "watch-liveliness-compact-s6-hot",
    "watch-liveliness-flat-s6-hot",
    "watch-liveliness-calm-mode-s6-hot",
];

const DAY_CONTEXT_WATCH_IDS: [&str; 19] = [
    "watch-daycontext-night-asleep",
    "watch-daycontext-dawn-crossing",
    "watch-daycontext-night-wake-catchup",
    "watch-daycontext-hatch-at-night",
    "watch-daycontext-dream-night",
    "watch-daycontext-heavy-day-evening",
    "watch-daycontext-light-day-morning",
    "watch-daycontext-weekend-midday",
    "watch-daycontext-climate-cache-week",
    "watch-daycontext-prop-resonance-planter",
    "watch-daycontext-midnight-mid-session",
    "watch-daycontext-dawn-fresh",
    "watch-daycontext-dusk-heavy",
    "watch-daycontext-night-quiet",
    "watch-daycontext-work-output-sparks",
    "watch-daycontext-work-reasoning-pulse",
    "watch-daycontext-work-cache-mist",
    "watch-daycontext-work-mixed",
    "watch-daycontext-work-clear",
];

const ALIVE_ROOM_WATCH_IDS: [&str; 8] = [
    "room-starter-day-clear",
    "room-botanical-cache-evening",
    "room-technical-output-active",
    "room-celestial-artifact-night",
    "room-cozy-weekend-quiet",
    "room-mixed-full-wide",
    "room-heavy-day-cozy-large",
    "room-dawn-wake-small",
];

const ACTIVITY_IDENTITY_WATCH_IDS: [&str; 2] = [
    "watch-activity-identity-ensemble",
    "watch-activity-identity-unknown",
];

const TANK_LIFE_IDS: [&str; 8] = [
    "tank-life-age-empty",
    "tank-life-age-first",
    "tank-life-age-early",
    "tank-life-age-full",
    "tank-life-date-2026-07-07",
    "tank-life-date-2026-07-08",
    "tank-life-round-projection",
    "tank-life-anemone-morphs",
];

const PIXEL_CAST_IDS: [&str; 6] = [
    "pixel-fuzz-s3-locket",
    "pixel-blob-s3-body",
    "pixel-ghost-s3-wisp",
    "pixel-glitch-s4-repair",
    "pixel-crystal-s5-facets",
    "pixel-mech-s5-hardbody",
];

const SMOOTH_BASELINE_ID: &str = "round-smooth-classic-baseline";
const SMOOTH_PARITY_ID: &str = "round-smooth-classic-parity";
const SMOOTH_MOTION_ID: &str = "round-smooth-motion";
const SMOOTH_CANONICAL_LAYER_BINDINGS: [(&str, &str, Option<&str>); 19] = [
    ("depth-rings", "fixed", None),
    ("biome-wash", "parallax", Some("far")),
    ("room-glyphs", "parallax", Some("far")),
    ("ambient", "parallax", Some("mid")),
    ("motes", "parallax", Some("mid")),
    ("activity-glyphs", "parallax", Some("mid")),
    ("props-behind", "parallax", Some("behind")),
    ("tank-life-behind", "parallax", Some("behind")),
    ("chest-bubble", "parallax", Some("behind")),
    ("wall-shadow", "pet-attached", None),
    ("floor-projection", "floor-projected", None),
    ("pet-body", "pet-attached", None),
    ("performance-cue", "pet-attached", None),
    ("props-foreground", "parallax", Some("foreground")),
    ("tank-life-foreground", "parallax", Some("foreground")),
    ("status-halo", "fixed", None),
    ("trouble-indicator", "fixed", None),
    ("mood-aura", "pet-attached", None),
    ("dim-overlay", "fixed", None),
];

const HABITAT_PROPS_ORBIT_ID: &str = "watch-habitat-props-orbit";

const GLITCH_PERSISTENCE_PET_ID: &str = "pet-glitch-persistence-states";

const GLITCH_PERSISTENCE_WATCH_IDS: [&str; 4] = [
    "watch-glitch-patched-quiet",
    "watch-glitch-patched-active",
    "watch-glitch-burst",
    "watch-glitch-calm-hot",
];

const GLITCH_PERSISTENCE_ROUND_ID: &str = "round-glitch-patched-s6";

const SPECIES_DIALECT_WATCH_IDS: [&str; 8] = [
    "watch-species-dialect-fuzz",
    "watch-species-dialect-blob",
    "watch-species-dialect-ghost",
    "watch-species-dialect-glitch",
    "watch-species-dialect-crystal",
    "watch-species-dialect-mech",
    "watch-species-dialect-glitch-flat",
    "watch-species-dialect-crystal-flat",
];

const SPECIES_DIALECT_STRICT_IDS: [&str; 4] = [
    "watch-species-dialect-glitch",
    "watch-species-dialect-crystal",
    "watch-species-dialect-glitch-flat",
    "watch-species-dialect-crystal-flat",
];

const SPECIES_DIALECT_MATRIX_IDS: [&str; 6] = [
    "watch-species-dialect-fuzz",
    "watch-species-dialect-blob",
    "watch-species-dialect-ghost",
    "watch-species-dialect-glitch",
    "watch-species-dialect-crystal",
    "watch-species-dialect-mech",
];

struct PreviewRun {
    _dir: TempDir,
    out: PathBuf,
    config_dir: PathBuf,
}

impl PreviewRun {
    fn new() -> Self {
        let dir = tempdir().unwrap();
        Self {
            out: dir.path().join("preview"),
            config_dir: dir.path().join("config"),
            _dir: dir,
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("glorp").unwrap();
        cmd.arg("dev-preview")
            .arg("--out")
            .arg(&self.out)
            .env("GLORP_CONFIG_DIR", &self.config_dir);
        cmd
    }

    fn run_success(&self, scenario: &str) {
        self.command()
            .arg("--scenario")
            .arg(scenario)
            .assert()
            .success()
            .stdout(predicate::str::contains(self.out.display().to_string()));
    }

    fn manifest(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.out.join("manifest.json")).unwrap())
            .unwrap()
    }

    fn read_json(&self, path: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.out.join(path)).unwrap()).unwrap()
    }
}

fn collect_pixel_json_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_pixel_json_paths(&path, paths);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".pixel.json"))
        {
            paths.push(path);
        }
    }
}

fn collect_pixel_review_artifact_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_pixel_review_artifact_paths(&path, paths);
            continue;
        }

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let path_text = path.to_string_lossy();
        let is_pixel_frame = name.starts_with("pixel-")
            && (name.ends_with(".txt")
                || name.ends_with(".cells.json")
                || name.ends_with(".pixel.json")
                || name.ends_with(".pixel-art.json")
                || name.ends_with(".pixel-composition.json")
                || name.ends_with(".pixel-fit.json"));
        let is_pixel_strip = path_text.contains("strips/pixel-")
            && (name.ends_with(".txt")
                || name.ends_with(".cells.json")
                || name.ends_with(".pixel.json"));
        if is_pixel_frame || is_pixel_strip {
            paths.push(path);
        }
    }
}

fn collect_smooth_sidecar_paths(dir: &Path, paths: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_smooth_sidecar_paths(&path, paths);
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with(".smooth-plan.json")
            || name.ends_with(".smooth-parity.json")
            || name.ends_with(".smooth-motion.json")
        {
            paths.push(path);
        }
    }
}

const SIDECAR_FORBIDDEN_PRIVACY_VALUE_TOKENS: &[&str] = &[
    "fixture-seed",
    "art_text",
    "claude",
    "codex",
    "openai",
    "source_name",
    "display_name",
    "/users/",
    "/tmp/",
    "prompt",
    "response",
    "transcript",
    "tool payload",
    "diagnostic",
    "very-secret-seed",
];

fn assert_sidecar_json_values_are_sanitized(sidecar: &Value, surface: &str) {
    assert_json_value_is_sanitized(sidecar, surface, "$");
}

fn validate_smooth_enum_string_paths(sidecar: &Value, surface: &str) -> Result<(), String> {
    fn is_canonical_layer_field(path: &str, collection_prefix: Option<&str>, field: &str) -> bool {
        collection_prefix.is_some_and(|prefix| {
            path.strip_prefix(prefix)
                .and_then(|path| path.strip_suffix(&format!("].{field}")))
                .is_some_and(|index| {
                    !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
    }

    fn walk(
        value: &Value,
        surface: &str,
        path: &str,
        collection_prefix: Option<&str>,
    ) -> Result<(), String> {
        match value {
            Value::String(text)
                if matches!(
                    text.as_str(),
                    "fixed" | "pet-attached" | "floor-projected" | "parallax"
                ) =>
            {
                if !is_canonical_layer_field(path, collection_prefix, "motion_binding") {
                    return Err(format!(
                        "{surface} sidecar exposed motion binding {text} outside a canonical smooth layer field at {path}"
                    ));
                }
            }
            Value::String(text)
                if matches!(text.as_str(), "far" | "mid" | "behind" | "foreground") =>
            {
                if !is_canonical_layer_field(path, collection_prefix, "depth_plane") {
                    return Err(format!(
                        "{surface} sidecar exposed depth plane {text} outside a canonical smooth layer field at {path}"
                    ));
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    walk(
                        child,
                        surface,
                        &format!("{path}[{index}]"),
                        collection_prefix,
                    )?;
                }
            }
            Value::Object(map) => {
                for (key, child) in map {
                    walk(child, surface, &format!("{path}.{key}"), collection_prefix)?;
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }

        Ok(())
    }

    let collection_prefix = if sidecar.get("strip_id").is_some() {
        Some("$.layer_transforms[")
    } else if sidecar.get("frame_id").is_some() && sidecar.get("layers").is_some() {
        Some("$.layers[")
    } else {
        None
    };
    walk(sidecar, surface, "$", collection_prefix)
}

fn assert_smooth_enum_strings_only_in_typed_fields(sidecar: &Value, surface: &str) {
    if let Err(error) = validate_smooth_enum_string_paths(sidecar, surface) {
        panic!("{error}");
    }
}

fn assert_canonical_smooth_layer_mapping(layers: &Value, surface: &str) {
    let layers = layers
        .as_array()
        .unwrap_or_else(|| panic!("{surface} layers should be an array"));
    let actual = layers
        .iter()
        .map(|layer| {
            (
                layer["role"].as_str().unwrap(),
                layer["motion_binding"].as_str().unwrap(),
                layer["depth_plane"].as_str(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual, SMOOTH_CANONICAL_LAYER_BINDINGS,
        "{surface} should serialize the canonical 19-role motion mapping"
    );

    for (layer, (_, expected_binding, _)) in layers.iter().zip(SMOOTH_CANONICAL_LAYER_BINDINGS) {
        if expected_binding != "parallax" {
            let x = layer["parallax_translation"]["x"].as_f64().unwrap();
            let y = layer["parallax_translation"]["y"].as_f64().unwrap();
            assert_eq!(
                x, 0.0,
                "{surface} role {} should have zero x",
                layer["role"]
            );
            assert_eq!(
                y, 0.0,
                "{surface} role {} should have zero y",
                layer["role"]
            );
        }
    }
}

#[test]
fn dev_preview_smooth_canonical_mapping_accepts_safety_clamped_parallax_layer() {
    let layers = Value::Array(
        SMOOTH_CANONICAL_LAYER_BINDINGS
            .into_iter()
            .map(|(role, motion_binding, depth_plane)| {
                let parallax_translation = if motion_binding == "parallax" && role != "props-behind"
                {
                    serde_json::json!({ "x": 0.25, "y": 0.125 })
                } else {
                    serde_json::json!({ "x": 0.0, "y": 0.0 })
                };
                serde_json::json!({
                    "role": role,
                    "motion_binding": motion_binding,
                    "depth_plane": depth_plane,
                    "parallax_translation": parallax_translation
                })
            })
            .collect(),
    );

    assert_canonical_smooth_layer_mapping(&layers, "safety-clamped-fixture");
}

#[test]
fn dev_preview_smooth_enum_path_validation_rejects_abstract_state_motion_binding() {
    let sidecar = serde_json::json!({
        "frame_id": "smooth-plan",
        "layers": [],
        "abstract_state": {
            "motion_binding": "fixed"
        }
    });

    let error = validate_smooth_enum_string_paths(&sidecar, "smooth-test").unwrap_err();
    assert!(error.contains(
        "motion binding fixed outside a canonical smooth layer field at $.abstract_state.motion_binding"
    ));
}

#[test]
fn dev_preview_smooth_enum_path_validation_rejects_nested_depth_plane() {
    let sidecar = serde_json::json!({
        "strip_id": "smooth-motion",
        "layer_transforms": [{
            "metadata": {
                "depth_plane": "far"
            }
        }]
    });

    let error = validate_smooth_enum_string_paths(&sidecar, "smooth-test").unwrap_err();
    assert!(error.contains(
        "depth plane far outside a canonical smooth layer field at $.layer_transforms[0].metadata.depth_plane"
    ));
}

#[test]
fn dev_preview_smooth_enum_path_validation_rejects_motion_field_in_plan_artifact() {
    let sidecar = serde_json::json!({
        "frame_id": "smooth-plan",
        "layer_transforms": [{
            "motion_binding": "fixed"
        }]
    });

    let error = validate_smooth_enum_string_paths(&sidecar, "smooth-test").unwrap_err();
    assert!(error.contains(
        "motion binding fixed outside a canonical smooth layer field at $.layer_transforms[0].motion_binding"
    ));
}

fn assert_json_value_is_sanitized(value: &Value, surface: &str, path: &str) {
    match value {
        Value::String(text) => {
            let text = text.to_ascii_lowercase();
            for forbidden in SIDECAR_FORBIDDEN_PRIVACY_VALUE_TOKENS {
                assert!(
                    !text.contains(forbidden),
                    "{surface} sidecar leaked {forbidden} at {path}: {text}"
                );
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                assert_json_value_is_sanitized(child, surface, &format!("{path}[{index}]"));
            }
        }
        Value::Object(map) => {
            for (key, child) in map {
                assert_json_value_is_sanitized(child, surface, &format!("{path}.{key}"));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn pixel_alpha_sum_for_rgb(pixel: &Value, rgb: &str) -> u32 {
    pixel["pixels"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .filter(|hex| hex.starts_with(rgb))
        .map(|hex| u32::from_str_radix(&hex[7..9], 16).unwrap())
        .sum()
}

#[test]
fn dev_preview_command_is_callable() {
    let run = PreviewRun::new();

    run.run_success("watch");
}

#[test]
fn dev_preview_defaults_to_target_output_and_all_scenario() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join("config");

    Command::cargo_bin("glorp")
        .unwrap()
        .current_dir(dir.path())
        .arg("dev-preview")
        .env("GLORP_CONFIG_DIR", &config_dir)
        .assert()
        .success()
        .stdout(predicate::str::contains("target/glorp-preview"));

    let out = dir.path().join("target/glorp-preview");
    assert!(out.join("frames/watch-wide-normal.txt").is_file());
    assert!(out.join("frames/watch-tall-wide.txt").is_file());
    assert!(out.join("frames/watch-compact-normal.txt").is_file());
    assert!(out.join("frames/pet-species-stage.txt").is_file());
    assert!(!config_dir.exists());
}

#[test]
fn dev_preview_watch_writes_expected_artifacts() {
    let run = PreviewRun::new();

    run.run_success("watch");

    assert!(run.out.join(".glorp-preview").is_file());
    assert!(run.out.join("manifest.json").is_file());
    assert!(run.out.join("review.md").is_file());
    assert!(run.out.join("index.html").is_file());
    assert!(run.out.join("assets/preview.css").is_file());
    assert!(run.out.join("assets/preview.js").is_file());
    assert!(run.out.join("frames/watch-wide-normal.txt").is_file());
    assert!(run
        .out
        .join("frames/watch-wide-normal.cells.json")
        .is_file());
    assert!(run
        .out
        .join("frames/watch-wide-normal.layout.json")
        .is_file());
    assert!(run.out.join("frames/watch-tall-wide.txt").is_file());
    assert!(run.out.join("frames/watch-tall-wide.cells.json").is_file());
    assert!(run.out.join("frames/watch-tall-wide.layout.json").is_file());
    assert!(run.out.join("frames/watch-compact-normal.txt").is_file());
    assert!(run
        .out
        .join("frames/watch-compact-normal.cells.json")
        .is_file());
    assert!(run
        .out
        .join("frames/watch-compact-normal.layout.json")
        .is_file());
    for id in LIVELINESS_WATCH_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id} text artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id} cells artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.layout.json")).is_file(),
            "missing {id} layout artifact"
        );
    }
    for id in DAY_CONTEXT_WATCH_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id} text artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id} cells artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.layout.json")).is_file(),
            "missing {id} layout artifact"
        );
    }
    for id in ALIVE_ROOM_WATCH_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id} text artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id} cells artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.layout.json")).is_file(),
            "missing {id} layout artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.room.txt")).is_file(),
            "missing {id} room text artifact"
        );
    }
    for id in SPECIES_DIALECT_WATCH_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id} text artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id} cells artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.layout.json")).is_file(),
            "missing {id} layout artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.room.txt")).is_file(),
            "missing {id} room text artifact"
        );
    }
    for id in SPECIES_DIALECT_STRICT_IDS {
        assert!(
            run.out
                .join(format!("frames/{id}.room-masked.txt"))
                .is_file(),
            "missing {id} room-masked text artifact"
        );
    }
    for id in ACTIVITY_IDENTITY_WATCH_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id} text artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id} cells artifact"
        );
        assert!(
            run.out.join(format!("frames/{id}.layout.json")).is_file(),
            "missing {id} layout artifact"
        );
    }
    assert!(
        run.out
            .join(format!("frames/{HABITAT_PROPS_ORBIT_ID}.txt"))
            .is_file(),
        "missing {HABITAT_PROPS_ORBIT_ID} text artifact"
    );
    assert!(
        run.out
            .join(format!("frames/{HABITAT_PROPS_ORBIT_ID}.cells.json"))
            .is_file(),
        "missing {HABITAT_PROPS_ORBIT_ID} cells artifact"
    );
    assert!(
        run.out
            .join(format!("frames/{HABITAT_PROPS_ORBIT_ID}.layout.json"))
            .is_file(),
        "missing {HABITAT_PROPS_ORBIT_ID} layout artifact"
    );

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    assert_eq!(manifest["producer"], "glorp-dev-preview");
    assert!(!manifest["glorp_version"].as_str().unwrap().is_empty());
    assert!(manifest["generated_at"].as_str().unwrap().ends_with('Z'));
    assert_scenario(
        &manifest,
        "watch-wide-normal",
        "watch",
        (120, 32),
        (
            "frames/watch-wide-normal.txt",
            "frames/watch-wide-normal.cells.json",
            Some("frames/watch-wide-normal.layout.json"),
        ),
    );
    assert_scenario(
        &manifest,
        "watch-tall-wide",
        "watch",
        (180, 50),
        (
            "frames/watch-tall-wide.txt",
            "frames/watch-tall-wide.cells.json",
            Some("frames/watch-tall-wide.layout.json"),
        ),
    );
    assert_scenario(
        &manifest,
        "watch-compact-normal",
        "watch",
        (72, 24),
        (
            "frames/watch-compact-normal.txt",
            "frames/watch-compact-normal.cells.json",
            Some("frames/watch-compact-normal.layout.json"),
        ),
    );
    assert_artifact_type(&manifest, "watch-wide-normal", "text");
    assert_artifact_type(&manifest, "watch-wide-normal-cells", "cells");
    assert_artifact_type(&manifest, "watch-wide-normal-layout", "layout");
    assert_artifact_type(&manifest, "watch-tall-wide", "text");
    assert_artifact_type(&manifest, "watch-tall-wide-cells", "cells");
    assert_artifact_type(&manifest, "watch-tall-wide-layout", "layout");
    for id in LIVELINESS_WATCH_IDS {
        let (width, height) = liveliness_dimensions(id);
        assert_scenario(
            &manifest,
            id,
            "watch",
            (width, height),
            (
                &format!("frames/{id}.txt"),
                &format!("frames/{id}.cells.json"),
                Some(&format!("frames/{id}.layout.json")),
            ),
        );
        assert_artifact_type(&manifest, id, "text");
        assert_artifact_type(&manifest, &format!("{id}-cells"), "cells");
        assert_artifact_type(&manifest, &format!("{id}-layout"), "layout");
    }
    for id in ALIVE_ROOM_WATCH_IDS {
        let (width, height) = alive_room_dimensions(id);
        assert_scenario(
            &manifest,
            id,
            "watch",
            (width, height),
            (
                &format!("frames/{id}.txt"),
                &format!("frames/{id}.cells.json"),
                Some(&format!("frames/{id}.layout.json")),
            ),
        );
        assert_artifact_type(&manifest, id, "text");
        assert_artifact_type(&manifest, &format!("{id}-cells"), "cells");
        assert_artifact_type(&manifest, &format!("{id}-layout"), "layout");
        assert_artifact_type(&manifest, &format!("{id}-room"), "text");
    }
    for id in SPECIES_DIALECT_WATCH_IDS {
        assert_scenario(
            &manifest,
            id,
            "watch",
            (120, 32),
            (
                &format!("frames/{id}.txt"),
                &format!("frames/{id}.cells.json"),
                Some(&format!("frames/{id}.layout.json")),
            ),
        );
        assert_artifact_type(&manifest, id, "text");
        assert_artifact_type(&manifest, &format!("{id}-cells"), "cells");
        assert_artifact_type(&manifest, &format!("{id}-layout"), "layout");
        assert_artifact_type(&manifest, &format!("{id}-room"), "text");
    }
    for id in SPECIES_DIALECT_STRICT_IDS {
        assert_artifact_type(&manifest, &format!("{id}-room-masked"), "text");
    }
}

#[test]
fn privacy_value_scan_ignores_allowed_claim_field_names() {
    let sidecar = serde_json::json!({
        "privacy": {
            "prompt_text_visible": false,
            "response_text_visible": false,
            "raw_diagnostics_visible": false,
        }
    });

    assert_sidecar_json_values_are_sanitized(&sidecar, "smooth");
}

#[test]
fn dev_preview_watch_manifest_lists_habitat_prop_fixture_ids() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in [
        "watch-wide-normal",
        "watch-tall-wide",
        "watch-compact-normal",
    ] {
        let watch_scenario = scenario(&manifest, id);
        let prop_ids = watch_scenario["inputs"]["habitat_props"]
            .as_array()
            .unwrap_or_else(|| panic!("{id} habitat_props input should be an array"))
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();

        for expected in [
            "codex_signal_lamp",
            "heavy_session_planter",
            "token_pebble_25k",
            "token_shell_100k",
        ] {
            assert!(
                prop_ids.contains(&expected),
                "{id} habitat_props should include {expected}"
            );
        }
    }
}

#[test]
fn dev_preview_watch_includes_liveliness_profile_inputs() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in LIVELINESS_WATCH_IDS {
        let watch_scenario = scenario(&manifest, id);
        let life_profile = &watch_scenario["inputs"]["life_profile"];

        assert!(
            life_profile["activity_level"].is_number(),
            "{id} activity_level should be numeric"
        );
        assert!(
            life_profile["burst_level"].is_number(),
            "{id} burst_level should be numeric"
        );
        assert!(
            life_profile["freshness"].is_string(),
            "{id} freshness should be a string"
        );
    }
}

#[test]
fn dev_preview_watch_includes_day_context_inputs() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in DAY_CONTEXT_WATCH_IDS {
        let watch_scenario = scenario(&manifest, id);
        let day_context = &watch_scenario["inputs"]["day_context"];

        assert!(
            day_context.is_object(),
            "{id} day_context should be an object"
        );
        assert!(
            day_context["day_phase"].is_string(),
            "{id} day_phase should be a string"
        );
        assert!(
            day_context["asleep"].is_boolean(),
            "{id} asleep should be a boolean"
        );
        let sleep_onset = &day_context["sleep_onset_utc"];
        assert!(
            sleep_onset.is_string() || sleep_onset.is_null(),
            "{id} sleep_onset_utc should be a string or null"
        );
    }
}

#[test]
fn dev_preview_watch_manifest_records_activity_identity_intent() {
    let run = PreviewRun::new();
    run.run_success("watch");

    let manifest = run.manifest();
    for id in [
        "watch-activity-identity-ensemble",
        "watch-activity-identity-unknown",
    ] {
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["intent"], "Review activity-identity driven watch layout with multi-source or unknown-source usage.");
        let source_mix = scenario["inputs"]["source_mix"].as_array().unwrap();
        assert!(!source_mix.is_empty(), "{id} should list source_mix");
        assert!(
            scenario["inputs"]["activity_profile"].is_object(),
            "{id} should record activity_profile"
        );
        assert!(scenario["inputs"]["activity_profile"]["source_diversity"].is_string());
        assert!(
            scenario["inputs"]["raw_helper_output"].is_null(),
            "{id} must not include raw payloads"
        );
    }
}

#[test]
fn dev_preview_watch_includes_habitat_props_orbit_frame() {
    let run = PreviewRun::new();
    run.run_success("watch");

    let manifest = run.manifest();
    // Frame must exist with correct dimensions
    assert_scenario(
        &manifest,
        HABITAT_PROPS_ORBIT_ID,
        "watch",
        (120, 32),
        (
            &format!("frames/{HABITAT_PROPS_ORBIT_ID}.txt"),
            &format!("frames/{HABITAT_PROPS_ORBIT_ID}.cells.json"),
            Some(&format!("frames/{HABITAT_PROPS_ORBIT_ID}.layout.json")),
        ),
    );
    // The frame must record habitat_props inputs (including the orbit prop)
    let s = scenario(&manifest, HABITAT_PROPS_ORBIT_ID);
    let prop_ids = s["inputs"]["habitat_props"]
        .as_array()
        .unwrap_or_else(|| panic!("{HABITAT_PROPS_ORBIT_ID} should have habitat_props input"))
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        prop_ids.contains(&"token_orbit_5m"),
        "{HABITAT_PROPS_ORBIT_ID} habitat_props must include token_orbit_5m (orbit-class prop for golden coverage)"
    );
}

#[test]
fn dev_preview_includes_room_phase_scenarios() {
    let run = PreviewRun::new();
    run.run_success("watch");
    let manifest = run.manifest();
    for id in [
        "watch-daycontext-dawn-fresh",
        "watch-daycontext-dusk-heavy",
        "watch-daycontext-night-quiet",
    ] {
        let s = scenario(&manifest, id);
        assert!(
            s["inputs"]["day_context"]["day_phase"].is_string(),
            "{id} needs a day_phase"
        );
    }
}

#[test]
fn dev_preview_liveliness_changes_pet_scene_cells_not_only_text() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let warm_cells = read_cells(&run, "watch-liveliness-s6-warm-midday");
    let warm_layout = read_layout(&run, "watch-liveliness-s6-warm-midday");
    let hot_cells = read_cells(&run, "watch-liveliness-s6-hot-midday");
    let hot_layout = read_layout(&run, "watch-liveliness-s6-hot-midday");

    assert_eq!(
        warm_layout["targets"]["watch.pet.habitat"], hot_layout["targets"]["watch.pet.habitat"],
        "same-clock warm/hot liveliness fixtures should compare the same habitat rect"
    );

    let warm_habitat = cells_for_target(&warm_cells, &warm_layout, "watch.pet.habitat");
    let hot_habitat = cells_for_target(&hot_cells, &hot_layout, "watch.pet.habitat");
    let changed = changed_cells_by_symbol_or_fg(&warm_habitat, &hot_habitat);

    assert!(
        changed >= 8,
        "liveliness profile changes should visibly alter at least 8 habitat cells; changed {changed}"
    );
}

#[test]
fn alive_room_fixtures_differ_by_symbols_in_multiple_room_zones() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let botanical_cells = read_cells(&run, "room-botanical-cache-evening");
    let botanical_layout = read_layout(&run, "room-botanical-cache-evening");
    let technical_cells = read_cells(&run, "room-technical-output-active");
    let technical_layout = read_layout(&run, "room-technical-output-active");

    let botanical_room = cells_for_target(&botanical_cells, &botanical_layout, "watch.room.effect");
    let technical_room = cells_for_target(&technical_cells, &technical_layout, "watch.room.effect");
    let changed = changed_cells_by_symbol(&botanical_room, &technical_room);
    let rect = &botanical_layout["targets"]["watch.room.effect"];
    let zones = changed_room_zones(
        &botanical_room,
        &technical_room,
        rect["width"].as_u64().unwrap(),
        rect["height"].as_u64().unwrap(),
    );

    assert!(
        changed >= 24,
        "room states should differ by symbols; changed {changed}"
    );
    assert!(
        zones.len() >= 2,
        "room states should differ across zones; got {zones:?}"
    );
}

#[test]
fn alive_room_pet_performance_fixtures_change_pet_adjacent_symbols() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let heavy = read_cells(&run, "room-heavy-day-cozy-large");
    let heavy_layout = read_layout(&run, "room-heavy-day-cozy-large");
    let dawn = read_cells(&run, "room-dawn-wake-small");
    let dawn_layout = read_layout(&run, "room-dawn-wake-small");
    let heavy_pet = cells_for_target(&heavy, &heavy_layout, "watch.pet.art");
    let dawn_pet = cells_for_target(&dawn, &dawn_layout, "watch.pet.art");

    assert!(
        changed_cells_by_symbol(&heavy_pet, &dawn_pet) >= 2,
        "pet performance fixtures should produce readable pet-local differences"
    );
}

#[test]
fn dev_preview_alive_room_fixtures_include_room_profile_inputs() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in ALIVE_ROOM_WATCH_IDS {
        let scenario = scenario(&manifest, id);
        assert!(
            scenario["inputs"]["room_life_profile"].is_object(),
            "{id} missing room_life_profile"
        );
        assert!(
            scenario["inputs"]["expected_room_life_profile"].is_object(),
            "{id} missing expected profile"
        );
        assert!(
            scenario["review_prompts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|prompt| prompt.as_str().unwrap().contains("primary biome")),
            "{id} review prompts should mention primary biome"
        );
    }
}

#[test]
fn dev_preview_alive_room_writes_cropped_room_artifacts() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in ALIVE_ROOM_WATCH_IDS {
        assert!(
            run.out.join(format!("frames/{id}.room.txt")).is_file(),
            "missing cropped room for {id}"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(
            scenario["files"]["room_text"],
            format!("frames/{id}.room.txt")
        );
    }
}

#[test]
fn dev_preview_watch_writes_layout_artifacts_and_manifest_entries() {
    let run = PreviewRun::new();

    run.run_success("watch");

    assert!(run
        .out
        .join("frames/watch-wide-normal.layout.json")
        .is_file());
    assert!(run.out.join("frames/watch-tall-wide.layout.json").is_file());
    assert!(run
        .out
        .join("frames/watch-compact-normal.layout.json")
        .is_file());

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    let wide = scenario(&manifest, "watch-wide-normal");
    assert_eq!(
        wide["files"]["layout"],
        "frames/watch-wide-normal.layout.json"
    );
    assert_artifact_type(&manifest, "watch-wide-normal-layout", "layout");

    let layout: Value = serde_json::from_str(
        &std::fs::read_to_string(run.out.join("frames/watch-wide-normal.layout.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(layout["schema_version"], 2);
    // Pet now spans the full body width (118 on a 120-col terminal).
    assert_eq!(layout["components"]["watch.pet"]["width"], 118);
    assert!(layout["components"]["watch.pet"].is_object());
    assert!(layout["targets"]["watch.pet.art"].is_object());

    let compact_layout: Value = serde_json::from_str(
        &std::fs::read_to_string(run.out.join("frames/watch-compact-normal.layout.json")).unwrap(),
    )
    .unwrap();
    assert!(compact_layout["targets"]["watch.pet.art"].is_object());
    assert!(
        compact_layout["components"]["watch.feed"]["height"]
            .as_u64()
            .unwrap()
            > 0,
        "compact feed should have drawable height in the preview layout"
    );
    assert!(compact_layout["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|decision| decision["path"] == "watch.bio" && decision["reason"] == "CompactMode"));
    assert!(compact_layout["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|decision| decision["path"] == "watch.feed" && decision["reason"] == "RowLimit"));
    assert!(!compact_layout["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|decision| decision["path"] == "watch.pet"
            && decision["reason"] == "InsufficientHeight"));

    let compact_text =
        std::fs::read_to_string(run.out.join("frames/watch-compact-normal.txt")).unwrap();
    assert!(
        compact_text.contains("feed"),
        "compact preview text should include feed"
    );
}

#[test]
fn dev_preview_watch_includes_real_tall_wide_frame() {
    let run = PreviewRun::new();

    run.run_success("watch");

    assert!(run.out.join("frames/watch-tall-wide.txt").is_file());
    assert!(run.out.join("frames/watch-tall-wide.cells.json").is_file());
    assert!(run.out.join("frames/watch-tall-wide.layout.json").is_file());

    let manifest = run.manifest();
    assert_scenario(
        &manifest,
        "watch-tall-wide",
        "watch",
        (180, 50),
        (
            "frames/watch-tall-wide.txt",
            "frames/watch-tall-wide.cells.json",
            Some("frames/watch-tall-wide.layout.json"),
        ),
    );

    let scenario = scenario(&manifest, "watch-tall-wide");
    assert_eq!(scenario["inputs"]["terminal_width"], 180);
    assert_eq!(scenario["inputs"]["terminal_height"], 50);

    let layout: Value = serde_json::from_str(
        &std::fs::read_to_string(run.out.join("frames/watch-tall-wide.layout.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(layout["frame"]["height"], 50);
    assert!(
        layout["components"]["watch.pet"]["height"]
            .as_u64()
            .unwrap()
            > 18,
        "tall-wide pet component should absorb vertical slack"
    );
}

#[test]
fn dev_preview_html_contains_layout_overlay_controls() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    assert!(html.contains("data-overlay-toggle=\"components\""));
    assert!(html.contains("data-overlay-toggle=\"targets\""));
    assert!(html.contains("data-layout-for=\"watch-wide-normal\""));
}

#[test]
fn dev_preview_layout_targets_include_owner_role_clip_and_layer() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let layout = read_layout(&run, "watch-wide-normal");
    let room = &layout["targets"]["watch.room.effect"];
    assert_eq!(room["owner"], "watch.pet");
    assert_eq!(room["role"], "RoomEffect");
    assert_eq!(room["layer"], "room-background");
    assert!(room["clip"].is_object());

    let pet = &layout["targets"]["watch.pet.effect"];
    assert_eq!(pet["role"], "Effect");
    assert_eq!(pet["layer"], "component");
}

#[test]
fn dev_preview_pets_writes_species_stage_matrix() {
    let run = PreviewRun::new();

    run.run_success("pets");

    let frame = std::fs::read_to_string(run.out.join("frames/pet-species-stage.txt")).unwrap();
    for species in ["fuzz", "blob", "ghost", "glitch", "crystal", "mech"] {
        assert!(frame.contains(species), "missing species {species}");
    }
    for label in [
        "fluff",
        "droplet",
        "whisper",
        "bit",
        "grain",
        "chip",
        "mythic-fuzz",
        "primordial",
        "revenant",
        "kernel",
        "lodestar",
        "titan",
    ] {
        assert!(frame.contains(label), "missing stage label {label}");
    }

    let manifest = run.manifest();
    assert_scenario(
        &manifest,
        "pet-species-stage",
        "pet-matrix",
        (120, 86),
        (
            "frames/pet-species-stage.txt",
            "frames/pet-species-stage.cells.json",
            None,
        ),
    );
    let matrix = scenario(&manifest, "pet-species-stage");
    assert_eq!(matrix["inputs"]["species"][0], "fuzz");
    assert_eq!(
        matrix["inputs"]["stages"][6]["labels_by_species"]["mech"],
        "titan"
    );
    assert!(run.out.join("frames/pet-glitch-live-states.txt").is_file());
    let glitch = scenario(&manifest, "pet-glitch-live-states");
    assert_eq!(glitch["inputs"]["species"], "glitch");
    assert_eq!(glitch["inputs"]["states"][2]["mood"], "ecstatic");
    assert_eq!(glitch["inputs"]["states"][2]["work_accent"], "dreamy");
}

#[test]
fn dev_preview_glitch_persistence_pet_frame_records_patch_contract() {
    let run = PreviewRun::new();

    run.run_success("pets");

    assert!(run
        .out
        .join(format!("frames/{GLITCH_PERSISTENCE_PET_ID}.txt"))
        .is_file());
    let manifest = run.manifest();
    let scenario = scenario(&manifest, GLITCH_PERSISTENCE_PET_ID);
    assert_eq!(scenario["kind"], "pet-matrix");
    assert_eq!(scenario["inputs"]["species"], "glitch");
    assert!(scenario["inputs"]["date_seed"].as_u64().unwrap() > 0);
    assert_eq!(scenario["inputs"]["same_day_restart"], true);
    assert_eq!(scenario["inputs"]["next_dawn_reset"], true);
    assert!(
        scenario["inputs"]["selected_patch_cells"]
            .as_array()
            .unwrap()
            .len()
            >= 3
    );
    // Span-derived face protection: the 3-cell {eyes} span + the 1-cell
    // {mouth} span (the old 13-cell static elder island is gone).
    assert!(
        scenario["inputs"]["protected_face_cells"]
            .as_array()
            .unwrap()
            .len()
            >= 4
    );
}

#[test]
fn dev_preview_glitch_watch_frames_record_patch_inputs() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in GLITCH_PERSISTENCE_WATCH_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id}.txt"
        );
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id}.cells.json"
        );
        assert!(
            run.out.join(format!("frames/{id}.layout.json")).is_file(),
            "missing {id}.layout.json"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "watch");
        assert_eq!(scenario["inputs"]["species"], "glitch");
        assert!(scenario["inputs"]["date_seed"].as_u64().unwrap() > 0);
        assert!(scenario["inputs"]["patch_tier"].is_string());
        assert!(scenario["inputs"]["burst_level"].is_string());
        assert!(scenario["inputs"]["expected_patch_count"].as_u64().unwrap() <= 3);
        assert!(scenario["inputs"]["selected_patch_cells"].is_array());
        assert!(scenario["inputs"]["protected_face_cells"].is_array());
    }
}

#[test]
fn dev_preview_round_glitch_patched_s6_records_patch_contract() {
    let run = PreviewRun::new();

    run.run_success("round");

    assert!(run
        .out
        .join(format!("frames/{GLITCH_PERSISTENCE_ROUND_ID}.txt"))
        .is_file());
    let manifest = run.manifest();
    let scenario = scenario(&manifest, GLITCH_PERSISTENCE_ROUND_ID);
    assert_eq!(scenario["kind"], "round");
    assert_eq!(scenario["inputs"]["species"], "glitch");
    assert_eq!(scenario["inputs"]["stage"], "s6");
    assert!(!scenario["inputs"]["selected_patch_cells"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(scenario["round"]["aperture"]["shape"], "circle");
}

#[test]
fn dev_preview_props_writes_habitat_prop_gallery_and_watch_variants() {
    let run = PreviewRun::new();

    run.run_success("props");

    for file in [
        "frames/habitat-props-catalog.txt",
        "frames/watch-habitat-early.txt",
        "frames/watch-habitat-lived-in.txt",
        "frames/watch-habitat-full-phase-a.txt",
        "frames/watch-habitat-full-phase-b.txt",
    ] {
        assert!(run.out.join(file).is_file(), "missing {file}");
    }

    let catalog =
        std::fs::read_to_string(run.out.join("frames/habitat-props-catalog.txt")).unwrap();
    for prop_id in [
        "token_pebble_25k",
        "token_shell_100k",
        "token_moss_tuft_250k",
        "token_spark_500k",
        "token_friendly_cloud_750k",
        "token_shard_1m",
        "token_treasure_chest_2m",
        "token_orbit_5m",
        "token_hanging_vine_25m",
        "token_lantern_10m",
        "codex_signal_lamp",
        "heavy_session_planter",
        "wilt_recovery_sprout",
    ] {
        assert!(catalog.contains(prop_id), "missing prop id {prop_id}");
    }

    let lived_in_layout: Value = serde_json::from_str(
        &std::fs::read_to_string(run.out.join("frames/watch-habitat-lived-in.layout.json"))
            .unwrap(),
    )
    .unwrap();
    let prop_targets: Vec<(&String, &Value)> = lived_in_layout["targets"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| k.starts_with("watch.prop."))
        .collect();
    assert!(
        !prop_targets.is_empty(),
        "watch-habitat-lived-in should export prop effect targets"
    );
    for (id, target) in &prop_targets {
        assert_eq!(target["role"], "PropEffect", "{id} role");
        assert_eq!(target["layer"], "prop", "{id} layer");
        assert!(target["cell_count"].is_number(), "{id} cell_count");
    }

    let manifest = run.manifest();
    assert_scenario(
        &manifest,
        "habitat-props-catalog",
        "habitat-props",
        (120, 70),
        (
            "frames/habitat-props-catalog.txt",
            "frames/habitat-props-catalog.cells.json",
            None,
        ),
    );
    let scenario = scenario(&manifest, "habitat-props-catalog");
    assert_eq!(scenario["inputs"]["prop_count"], 21);
    assert_eq!(scenario["inputs"]["motion_phases"][0], 1_760_000_000i64);
    assert_eq!(scenario["inputs"]["motion_phases"][1], 1_760_000_004i64);
    assert_eq!(scenario["inputs"]["motion_phases"][2], 1_760_000_010i64);
}

#[test]
fn dev_preview_manifest_paths_remain_stable_during_builder_cleanup() {
    let run = PreviewRun::new();
    run.run_success("all");
    let manifest = run.manifest();

    for (id, expected_text) in [
        ("watch-wide-normal", "frames/watch-wide-normal.txt"),
        (
            "watch-species-dialect-glitch",
            "frames/watch-species-dialect-glitch.txt",
        ),
        ("round-normal", "frames/round-normal.txt"),
        ("pet-species-stage", "frames/pet-species-stage.txt"),
        ("habitat-props-catalog", "frames/habitat-props-catalog.txt"),
    ] {
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["files"]["text"], expected_text);
    }

    let ids = scenario_ids(&manifest);
    assert_eq!(ids.first().unwrap(), "watch-wide-normal");
    assert!(ids.contains(&"round-normal".to_string()));
}

#[test]
fn dev_preview_all_writes_watch_and_pet_artifacts() {
    let run = PreviewRun::new();

    run.run_success("all");

    for file in [
        "frames/watch-wide-normal.txt",
        "frames/watch-tall-wide.txt",
        "frames/watch-compact-normal.txt",
        "frames/watch-liveliness-s6-idle-dawn.txt",
        "frames/watch-liveliness-s6-warm-midday.txt",
        "frames/watch-liveliness-s6-hot-midday.txt",
        "frames/watch-liveliness-s6-cooling-evening.txt",
        "frames/watch-liveliness-compact-s6-hot.txt",
        "frames/watch-liveliness-flat-s6-hot.txt",
        "frames/watch-liveliness-calm-mode-s6-hot.txt",
        "frames/watch-daycontext-night-asleep.txt",
        "frames/watch-daycontext-dawn-crossing.txt",
        "frames/watch-daycontext-night-wake-catchup.txt",
        "frames/watch-daycontext-hatch-at-night.txt",
        "frames/watch-daycontext-dream-night.txt",
        "frames/watch-daycontext-heavy-day-evening.txt",
        "frames/watch-daycontext-light-day-morning.txt",
        "frames/watch-daycontext-weekend-midday.txt",
        "frames/watch-daycontext-climate-cache-week.txt",
        "frames/watch-daycontext-prop-resonance-planter.txt",
        "frames/watch-daycontext-midnight-mid-session.txt",
        "frames/watch-daycontext-dawn-fresh.txt",
        "frames/watch-daycontext-dusk-heavy.txt",
        "frames/watch-daycontext-night-quiet.txt",
        "frames/watch-daycontext-work-output-sparks.txt",
        "frames/watch-daycontext-work-reasoning-pulse.txt",
        "frames/watch-daycontext-work-cache-mist.txt",
        "frames/watch-daycontext-work-mixed.txt",
        "frames/watch-daycontext-work-clear.txt",
        "frames/watch-glitch-patched-quiet.txt",
        "frames/watch-glitch-patched-active.txt",
        "frames/watch-glitch-burst.txt",
        "frames/watch-glitch-calm-hot.txt",
        "frames/room-starter-day-clear.txt",
        "frames/room-botanical-cache-evening.txt",
        "frames/room-technical-output-active.txt",
        "frames/room-celestial-artifact-night.txt",
        "frames/room-cozy-weekend-quiet.txt",
        "frames/room-mixed-full-wide.txt",
        "frames/room-heavy-day-cozy-large.txt",
        "frames/room-dawn-wake-small.txt",
        "frames/watch-species-dialect-fuzz.txt",
        "frames/watch-species-dialect-blob.txt",
        "frames/watch-species-dialect-ghost.txt",
        "frames/watch-species-dialect-glitch.txt",
        "frames/watch-species-dialect-crystal.txt",
        "frames/watch-species-dialect-mech.txt",
        "frames/watch-species-dialect-glitch-flat.txt",
        "frames/watch-species-dialect-crystal-flat.txt",
        "frames/watch-species-dialect-glitch.cells.json",
        "frames/watch-species-dialect-glitch.layout.json",
        "frames/watch-species-dialect-glitch.room-masked.txt",
        "frames/watch-species-dialect-crystal.cells.json",
        "frames/watch-species-dialect-crystal.layout.json",
        "frames/watch-species-dialect-crystal.room-masked.txt",
        "frames/watch-species-dialect-glitch-flat.cells.json",
        "frames/watch-species-dialect-glitch-flat.layout.json",
        "frames/watch-species-dialect-glitch-flat.room-masked.txt",
        "frames/watch-species-dialect-crystal-flat.cells.json",
        "frames/watch-species-dialect-crystal-flat.layout.json",
        "frames/watch-species-dialect-crystal-flat.room-masked.txt",
        "frames/watch-activity-identity-ensemble.txt",
        "frames/watch-activity-identity-ensemble.cells.json",
        "frames/watch-activity-identity-ensemble.layout.json",
        "frames/watch-activity-identity-unknown.txt",
        "frames/watch-activity-identity-unknown.cells.json",
        "frames/watch-activity-identity-unknown.layout.json",
        "frames/watch-habitat-props-orbit.txt",
        "frames/watch-habitat-props-orbit.cells.json",
        "frames/watch-habitat-props-orbit.layout.json",
        "frames/habitat-props-catalog.txt",
        "frames/watch-habitat-early.txt",
        "frames/watch-habitat-lived-in.txt",
        "frames/watch-habitat-full-phase-a.txt",
        "frames/watch-habitat-full-phase-b.txt",
        "frames/pet-species-stage.txt",
        "frames/pet-species-stage-flat.txt",
        "frames/pet-texture-variants.txt",
        "frames/pet-mood-set.txt",
        "frames/pet-glitch-live-states.txt",
        "frames/pet-glitch-persistence-states.txt",
        "frames/round-normal.txt",
        "frames/round-normal.cells.json",
        "frames/round-active-pulse.txt",
        "frames/round-asleep-night.txt",
        "frames/round-helper-trouble.txt",
        "frames/round-flat-color.txt",
        "frames/round-glitch-dialect.txt",
        "frames/round-crystal-dialect.txt",
        "frames/round-glitch-patched-s6.txt",
        "frames/round-smooth-classic-baseline.txt",
        "frames/round-smooth-classic-baseline.cells.json",
        "frames/round-smooth-classic-parity.txt",
        "frames/round-smooth-classic-parity.cells.json",
        "frames/round-smooth-classic-parity.smooth-plan.json",
        "frames/round-smooth-classic-parity.smooth-parity.json",
        "frames/tank-life-age-empty.txt",
        "frames/tank-life-age-first.txt",
        "frames/tank-life-age-early.txt",
        "frames/tank-life-age-full.txt",
        "frames/tank-life-date-2026-07-07.txt",
        "frames/tank-life-date-2026-07-08.txt",
        "frames/tank-life-round-projection.txt",
        "frames/tank-life-anemone-morphs.txt",
        "frames/pixel-fuzz-s3-content-idle.pixel.json",
        "frames/pixel-glitch-s4-feed-pulse.pixel.json",
        "frames/pixel-species-matrix.pixel.json",
        "frames/pixel-fuzz-s3-locket.pixel.json",
        "frames/pixel-fuzz-s3-locket.pixel-art.json",
        "frames/pixel-fuzz-s3-locket.pixel-fit.json",
        "frames/pixel-blob-s3-body.pixel.json",
        "frames/pixel-blob-s3-body.pixel-art.json",
        "frames/pixel-blob-s3-body.pixel-fit.json",
        "frames/pixel-ghost-s3-wisp.pixel.json",
        "frames/pixel-ghost-s3-wisp.pixel-art.json",
        "frames/pixel-ghost-s3-wisp.pixel-fit.json",
        "frames/pixel-glitch-s4-repair.pixel.json",
        "frames/pixel-glitch-s4-repair.pixel-art.json",
        "frames/pixel-glitch-s4-repair.pixel-fit.json",
        "frames/pixel-crystal-s5-facets.pixel.json",
        "frames/pixel-crystal-s5-facets.pixel-art.json",
        "frames/pixel-crystal-s5-facets.pixel-fit.json",
        "frames/pixel-mech-s5-hardbody.pixel.json",
        "frames/pixel-mech-s5-hardbody.pixel-art.json",
        "frames/pixel-mech-s5-hardbody.pixel-fit.json",
        "frames/pixel-tank-composition.pixel-composition.json",
        "strips/round-smooth-motion/frame-000.smooth-motion.json",
    ] {
        assert!(run.out.join(file).is_file(), "missing {file}");
    }

    let manifest = run.manifest();
    let ids = scenario_ids(&manifest);
    assert_eq!(
        ids,
        vec![
            "watch-wide-normal".to_string(),
            "watch-tall-wide".to_string(),
            "watch-compact-normal".to_string(),
            "watch-liveliness-s6-idle-dawn".to_string(),
            "watch-liveliness-s6-warm-midday".to_string(),
            "watch-liveliness-s6-hot-midday".to_string(),
            "watch-liveliness-s6-cooling-evening".to_string(),
            "watch-liveliness-compact-s6-hot".to_string(),
            "watch-liveliness-flat-s6-hot".to_string(),
            "watch-liveliness-calm-mode-s6-hot".to_string(),
            "watch-daycontext-night-asleep".to_string(),
            "watch-daycontext-dawn-crossing".to_string(),
            "watch-daycontext-night-wake-catchup".to_string(),
            "watch-daycontext-hatch-at-night".to_string(),
            "watch-daycontext-dream-night".to_string(),
            "watch-daycontext-heavy-day-evening".to_string(),
            "watch-daycontext-light-day-morning".to_string(),
            "watch-daycontext-weekend-midday".to_string(),
            "watch-daycontext-climate-cache-week".to_string(),
            "watch-daycontext-prop-resonance-planter".to_string(),
            "watch-daycontext-midnight-mid-session".to_string(),
            "watch-daycontext-dawn-fresh".to_string(),
            "watch-daycontext-dusk-heavy".to_string(),
            "watch-daycontext-night-quiet".to_string(),
            "watch-daycontext-work-output-sparks".to_string(),
            "watch-daycontext-work-reasoning-pulse".to_string(),
            "watch-daycontext-work-cache-mist".to_string(),
            "watch-daycontext-work-mixed".to_string(),
            "watch-daycontext-work-clear".to_string(),
            "watch-glitch-patched-quiet".to_string(),
            "watch-glitch-patched-active".to_string(),
            "watch-glitch-burst".to_string(),
            "watch-glitch-calm-hot".to_string(),
            "room-starter-day-clear".to_string(),
            "room-botanical-cache-evening".to_string(),
            "room-technical-output-active".to_string(),
            "room-celestial-artifact-night".to_string(),
            "room-cozy-weekend-quiet".to_string(),
            "room-mixed-full-wide".to_string(),
            "room-heavy-day-cozy-large".to_string(),
            "room-dawn-wake-small".to_string(),
            "watch-species-dialect-fuzz".to_string(),
            "watch-species-dialect-blob".to_string(),
            "watch-species-dialect-ghost".to_string(),
            "watch-species-dialect-glitch".to_string(),
            "watch-species-dialect-crystal".to_string(),
            "watch-species-dialect-mech".to_string(),
            "watch-species-dialect-glitch-flat".to_string(),
            "watch-species-dialect-crystal-flat".to_string(),
            "watch-activity-identity-ensemble".to_string(),
            "watch-activity-identity-unknown".to_string(),
            "watch-habitat-props-orbit".to_string(),
            "habitat-props-catalog".to_string(),
            "watch-habitat-early".to_string(),
            "watch-habitat-lived-in".to_string(),
            "watch-habitat-full-phase-a".to_string(),
            "watch-habitat-full-phase-b".to_string(),
            "pet-species-stage".to_string(),
            "pet-species-stage-flat".to_string(),
            "pet-texture-variants".to_string(),
            "pet-mood-set".to_string(),
            "pet-glitch-live-states".to_string(),
            "pet-glitch-persistence-states".to_string(),
            "round-normal".to_string(),
            "round-hud-missing-yesterday".to_string(),
            "round-hud-stale-yesterday".to_string(),
            "round-hud-zero-yesterday".to_string(),
            "round-hud-over-yesterday".to_string(),
            "round-hud-idle-pace".to_string(),
            "round-hud-burst-pace".to_string(),
            "round-active-pulse".to_string(),
            "round-asleep-night".to_string(),
            "round-helper-trouble".to_string(),
            "round-flat-color".to_string(),
            "round-glitch-dialect".to_string(),
            "round-crystal-dialect".to_string(),
            "round-glitch-patched-s6".to_string(),
            "round-smooth-classic-baseline".to_string(),
            "round-smooth-classic-parity".to_string(),
            "tank-life-age-empty".to_string(),
            "tank-life-age-first".to_string(),
            "tank-life-age-early".to_string(),
            "tank-life-age-full".to_string(),
            "tank-life-date-2026-07-07".to_string(),
            "tank-life-date-2026-07-08".to_string(),
            "tank-life-round-projection".to_string(),
            "tank-life-anemone-morphs".to_string(),
            "pixel-fuzz-s3-content-idle".to_string(),
            "pixel-glitch-s4-feed-pulse".to_string(),
            "pixel-species-matrix".to_string(),
            "pixel-fuzz-s3-locket".to_string(),
            "pixel-blob-s3-body".to_string(),
            "pixel-ghost-s3-wisp".to_string(),
            "pixel-glitch-s4-repair".to_string(),
            "pixel-crystal-s5-facets".to_string(),
            "pixel-mech-s5-hardbody".to_string(),
            "pixel-cast-identity-matrix".to_string(),
            "pixel-tank-composition".to_string(),
        ]
    );
}

#[test]
fn dev_preview_watch_and_round_frames_write_scene_artifacts() {
    let run = PreviewRun::new();

    run.run_success("all");

    let manifest = run.manifest();
    let scene_ids = preview_scenarios_with_contract_scene(&manifest);
    assert!(
        scene_ids.len() >= 50,
        "expected every watch and round scenario to carry a scene artifact; got {}",
        scene_ids.len()
    );

    for id in scene_ids {
        assert!(
            run.out.join(format!("frames/{id}.scene.json")).is_file(),
            "missing {id}.scene.json"
        );
        let scenario = scenario(&manifest, &id);
        assert_eq!(
            scenario["files"]["scene"],
            format!("frames/{id}.scene.json"),
            "{id} manifest files.scene"
        );
        assert_artifact_type(&manifest, &format!("{id}-scene"), "scene");

        let scene = read_scene(&run, &id);
        assert_eq!(scene["schema_version"], 1);
        assert_eq!(scene["frame_id"], id);
        assert!(scene["pet"]["species"].is_string(), "{id} pet species");
        assert!(scene["pet"]["stage"].is_string(), "{id} pet stage");
        assert!(
            scene["room"]["primary_biome"].is_string(),
            "{id} primary biome"
        );
        assert!(
            scene["privacy_projection"]["surface"].is_string(),
            "{id} surface"
        );
        assert!(scene["privacy_projection"]["source_names_visible"].is_boolean());
        assert!(scene["targets"].is_object(), "{id} target map");
    }
}

#[test]
fn dev_preview_scene_artifacts_are_sanitized_contracts_not_raw_runtime_state() {
    let run = PreviewRun::new();

    run.run_success("all");

    let manifest = run.manifest();
    for id in preview_scenarios_with_contract_scene(&manifest) {
        let scene_text =
            std::fs::read_to_string(run.out.join(format!("frames/{id}.scene.json"))).unwrap();
        let scene: Value = serde_json::from_str(&scene_text).unwrap();
        assert_eq!(
            scene["pet"]["seed"], "redacted",
            "{id}.scene.json should redact stable pet seed"
        );
        assert!(
            scene["room"]["prop_landmarks"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id}.scene.json should omit stable prop landmark ids"
        );
        for forbidden in [
            "/users/",
            "/tmp/",
            "prompt",
            "response",
            "tool payload",
            "transcript",
            "client-secret-project",
            "123456",
            "99999",
            "fixture-seed",
            "codex_signal_lamp",
            "token_pebble_25k",
        ] {
            assert!(
                !scene_text.to_ascii_lowercase().contains(forbidden),
                "{id}.scene.json leaked forbidden text {forbidden}: {scene_text}"
            );
        }
    }
}

#[test]
fn dev_preview_species_dialect_fixtures_have_manifest_contract() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in SPECIES_DIALECT_STRICT_IDS {
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "watch");
        assert_eq!(
            scenario["inputs"]["comparison_group"],
            "species-dialect-glitch-crystal"
        );
        assert!(
            scenario["inputs"]["room_dialect"].is_string(),
            "{id} missing room_dialect"
        );
        assert!(
            scenario["inputs"]["dialect_status"].is_string(),
            "{id} missing dialect_status"
        );
        assert!(
            scenario["inputs"]["shared_input_invariants"].is_object(),
            "{id} missing shared invariants"
        );
        assert!(
            scenario["inputs"]["prop_identity_invariants"].is_array(),
            "{id} missing prop invariants"
        );
        assert_eq!(
            scenario["files"]["room_masked_text"],
            format!("frames/{id}.room-masked.txt")
        );
        assert!(
            run.out
                .join(format!("frames/{id}.room-masked.txt"))
                .is_file(),
            "missing masked room artifact for {id}"
        );
    }
}

#[test]
fn dev_preview_species_dialect_matrix_lists_all_species() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    let species: Vec<&str> = SPECIES_DIALECT_MATRIX_IDS
        .iter()
        .map(|id| {
            scenario(&manifest, id)["inputs"]["species"]
                .as_str()
                .unwrap()
        })
        .collect();

    assert_eq!(
        species,
        vec!["fuzz", "blob", "ghost", "glitch", "crystal", "mech"]
    );

    for id in SPECIES_DIALECT_MATRIX_IDS {
        let scenario = scenario(&manifest, id);
        assert!(
            scenario["inputs"]["room_dialect"].is_string(),
            "{id} missing dialect"
        );
        assert!(
            scenario["inputs"]["dialect_status"].is_string(),
            "{id} missing status"
        );
    }
}

#[test]
fn dev_preview_glitch_and_crystal_dialects_differ_after_masking() {
    let run = PreviewRun::new();

    run.run_success("watch");

    assert_species_dialect_pair_differs(
        &run,
        "watch-species-dialect-glitch",
        "watch-species-dialect-crystal",
    );
    assert_species_dialect_pair_differs(
        &run,
        "watch-species-dialect-glitch-flat",
        "watch-species-dialect-crystal-flat",
    );
}

fn assert_species_dialect_pair_differs(run: &PreviewRun, left_id: &str, right_id: &str) {
    let left_cells = read_cells(run, left_id);
    let left_layout = read_layout(run, left_id);
    let right_cells = read_cells(run, right_id);
    let right_layout = read_layout(run, right_id);

    let left_room = masked_room_cells(&left_cells, &left_layout, &right_layout);
    let right_room = masked_room_cells(&right_cells, &right_layout, &left_layout);
    let changed = changed_cells_by_symbol(&left_room, &right_room);
    let rect = target_rect(&left_layout, "watch.room.effect");
    let zones = changed_room_zones(&left_room, &right_room, rect.width, rect.height);

    assert!(
        changed >= MIN_DIALECT_SYMBOL_DIFFERENCES,
        "{left_id} and {right_id} should differ by at least {MIN_DIALECT_SYMBOL_DIFFERENCES} masked room symbols; changed {changed}"
    );
    assert!(
        zones.len() >= MIN_DIALECT_DIFFERENT_ZONES,
        "{left_id} and {right_id} should differ across at least {MIN_DIALECT_DIFFERENT_ZONES} zones; got {zones:?}"
    );
    assert!(
        zones.contains("floor") || zones.contains("left-anchor") || zones.contains("right-anchor"),
        "expected floor or anchor-zone dialect difference; got {zones:?}"
    );
    assert!(
        zones.contains("upper-air") || zones.contains("pet-adjacent"),
        "expected upper-air or pet-adjacent dialect difference; got {zones:?}"
    );
}

#[test]
fn dev_preview_rerun_replaces_owned_output() {
    let run = PreviewRun::new();

    run.run_success("watch");
    std::fs::write(run.out.join("stale.txt"), "stale").unwrap();

    run.run_success("pets");

    assert!(!run.out.join("stale.txt").exists());
    assert!(!run.out.join("frames/watch-wide-normal.txt").exists());
    assert!(run.out.join("frames/pet-species-stage.txt").is_file());
}

#[test]
fn dev_preview_refuses_regular_file_output() {
    let run = PreviewRun::new();
    std::fs::write(&run.out, "not a directory").unwrap();

    run.command()
        .arg("--scenario")
        .arg("watch")
        .assert()
        .failure()
        .stderr(predicate::str::contains("regular file"));

    assert_eq!(
        std::fs::read_to_string(&run.out).unwrap(),
        "not a directory"
    );
}

#[test]
fn dev_preview_refuses_non_preview_directory() {
    let run = PreviewRun::new();
    std::fs::create_dir(&run.out).unwrap();
    std::fs::write(run.out.join("user-file.txt"), "mine").unwrap();

    run.command()
        .arg("--scenario")
        .arg("watch")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not owned"));

    assert_eq!(
        std::fs::read_to_string(run.out.join("user-file.txt")).unwrap(),
        "mine"
    );
}

#[test]
fn dev_preview_html_references_local_assets_that_exist() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    for asset in local_asset_refs(&html) {
        assert!(
            run.out.join(&asset).is_file(),
            "missing local asset reference {asset}"
        );
    }
    assert!(html.contains(r#"href="assets/preview.css""#));
    assert!(html.contains(r#"src="assets/preview.js""#));
    assert!(!html.contains("https://"));
    assert!(!html.contains("http://"));

    let manifest = run.manifest();
    assert_artifact_type(&manifest, "preview-css", "asset");
    assert_artifact_type(&manifest, "preview-js", "asset");
}

#[test]
fn dev_preview_does_not_use_user_config_dir() {
    let run = PreviewRun::new();

    run.run_success("watch");

    assert!(
        !run.config_dir.exists(),
        "dev-preview should not create or read the configured user state directory"
    );
}

#[test]
fn dev_preview_animation_writes_scene_strip_manifest_and_frames() {
    let run = PreviewRun::new();

    run.run_success("animation");

    assert!(run.out.join("manifest.json").is_file());
    assert!(run.out.join("review.md").is_file());
    assert!(run.out.join("index.html").is_file());
    assert!(run
        .out
        .join("strips/scene-strip-smoke/frame-000.txt")
        .is_file());
    assert!(run
        .out
        .join("strips/scene-strip-smoke/frame-000.cells.json")
        .is_file());
    assert!(!run.out.join("frames/watch-wide-normal.txt").exists());

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    assert!(
        manifest["scenarios"].as_array().unwrap().is_empty(),
        "animation-only bundles should not write static scenarios"
    );
    let strips = manifest["strips"]
        .as_array()
        .expect("strips should be an array");
    assert!(
        strips.len() >= 4,
        "animation bundle should include smoke + at least 3 real strips"
    );

    let mut target_ids = std::collections::HashSet::new();
    for strip in strips {
        target_ids.insert(strip["target_id"].as_str().unwrap().to_string());
    }
    assert!(
        target_ids.contains("watch.room.effect"),
        "expected room target_id"
    );
    assert!(
        target_ids.contains("watch.pet.effect"),
        "expected pet target_id"
    );
    assert!(
        target_ids.iter().any(|t| t.starts_with("watch.prop.")),
        "expected prop target_id"
    );

    assert_eq!(strips[0]["id"], "scene-strip-smoke");
    assert_eq!(strips[0]["kind"], "scene-moment");
    assert_eq!(strips[0]["dimensions"]["width"], 40);
    assert_eq!(strips[0]["dimensions"]["height"], 8);
    assert_eq!(strips[0]["frames"][0]["phase"], "start");
    assert_eq!(strips[0]["frames"][0]["elapsed_ms"], 0);
    assert_eq!(
        strips[0]["frames"][0]["files"]["text"],
        "strips/scene-strip-smoke/frame-000.txt"
    );
    assert_artifact_type(&manifest, "scene-strip-smoke-frame-000", "text");
    assert_artifact_type(&manifest, "scene-strip-smoke-frame-000-cells", "cells");
}

#[test]
fn dev_preview_all_includes_scene_strips() {
    let run = PreviewRun::new();

    run.run_success("all");

    let manifest = run.manifest();
    let strips = manifest["strips"].as_array().unwrap();
    assert!(
        strips.len() >= 4,
        "all preview should include smoke + at least 3 real strips"
    );
    assert!(run.out.join("frames/watch-wide-normal.txt").is_file());
    assert!(run
        .out
        .join("strips/scene-strip-smoke/frame-000.txt")
        .is_file());
    assert!(run
        .out
        .join("strips/scene-feed-sweep/frame-000.txt")
        .is_file());
    assert!(run
        .out
        .join("strips/scene-dawn-wake-wipe/frame-000.txt")
        .is_file());
}

#[test]
fn dev_preview_pixel_writes_schema_manifest_frames_and_canvas_links() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    assert!(run.out.join("manifest.json").is_file());
    assert!(run.out.join("index.html").is_file());
    assert!(run
        .out
        .join("frames/pixel-fuzz-s3-content-idle.pixel.json")
        .is_file());
    assert!(run
        .out
        .join("frames/pixel-glitch-s4-feed-pulse.pixel.json")
        .is_file());

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    let scenarios = manifest["scenarios"].as_array().unwrap();
    assert!(scenarios.iter().any(|scenario| {
        scenario["id"] == "pixel-fuzz-s3-content-idle"
            && scenario["kind"] == "pixel"
            && scenario["files"]["pixel"] == "frames/pixel-fuzz-s3-content-idle.pixel.json"
    }));
    assert_artifact_type(&manifest, "pixel-fuzz-s3-content-idle-pixel", "pixel-frame");

    let pixel = run.read_json("frames/pixel-fuzz-s3-content-idle.pixel.json");
    assert_eq!(pixel["schema_version"], 1);
    assert_eq!(pixel["width"], 96);
    assert_eq!(pixel["height"], 96);
    assert_eq!(pixel["pixels"].as_array().unwrap().len(), 96 * 96);
    assert!(pixel["pixels"].as_array().unwrap().iter().any(|value| {
        value
            .as_str()
            .is_some_and(|hex| hex.len() == 9 && hex.ends_with("ff"))
    }));

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    assert!(html.contains("data-pixel-frame=\"frames/pixel-fuzz-s3-content-idle.pixel.json\""));
    assert!(html.contains("<canvas"));
}

#[test]
fn dev_preview_pixel_manifest_dimensions_match_pixel_artifact() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let scenario = scenario(&manifest, "pixel-fuzz-s3-content-idle");
    let pixel = run.read_json("frames/pixel-fuzz-s3-content-idle.pixel.json");

    assert_eq!(scenario["dimensions"]["width"], 96);
    assert_eq!(scenario["dimensions"]["height"], 96);
    assert_eq!(scenario["dimensions"]["width"], pixel["width"]);
    assert_eq!(scenario["dimensions"]["height"], pixel["height"]);
}

#[test]
fn dev_preview_pixel_manifest_inputs_include_production_fit_producer() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let scenario = scenario(&manifest, "pixel-fuzz-s3-content-idle");

    assert_eq!(
        scenario["inputs"]["fit"]["producer"],
        "round::pixel_fit::pixel_companion_fit"
    );
}

#[test]
fn dev_preview_pixel_writes_art_and_fit_sidecars() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let scenario = scenario(&manifest, "pixel-fuzz-s3-content-idle");

    let art_path = run
        .out
        .join(scenario["files"]["pixel_art"].as_str().unwrap());
    let fit_path = run
        .out
        .join(scenario["files"]["pixel_fit"].as_str().unwrap());

    assert!(art_path.exists());
    assert!(fit_path.exists());

    let art_json = std::fs::read_to_string(art_path).unwrap();
    let art: Value = serde_json::from_str(&art_json).unwrap();
    assert_eq!(art["schema_version"], 2);
    assert!(art["role_cells"].as_array().unwrap().len() > 20);
    assert!(art["protected_bounds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|region| { region["id"] == "face" }));
    assert!(art["cue_coverage"]
        .as_object()
        .unwrap()
        .contains_key("locket"));
    assert!(art["signature_regions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|region| {
            region["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("signature-"))
        }));
    assert!(!art_json.contains("fixture-seed"));
    assert!(!art_json.contains("art_text"));

    let fit_json: Value =
        serde_json::from_str(&std::fs::read_to_string(fit_path).unwrap()).unwrap();
    assert_eq!(
        fit_json["producer"],
        "round::pixel_fit::pixel_companion_fit"
    );
    assert_eq!(fit_json["hud_overlap"]["body_eye_mouth_pixels"], 0);
}

#[test]
fn dev_preview_pixel_cast_identity_writes_six_real_frame_artifacts() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    for id in PIXEL_CAST_IDS {
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "pixel");
        assert_eq!(
            scenario["files"]["pixel"],
            format!("frames/{id}.pixel.json")
        );
        assert_eq!(
            scenario["files"]["pixel_art"],
            format!("frames/{id}.pixel-art.json")
        );
        assert_eq!(
            scenario["files"]["pixel_fit"],
            format!("frames/{id}.pixel-fit.json")
        );
        assert!(run.out.join(format!("frames/{id}.pixel.json")).is_file());
        assert!(run
            .out
            .join(format!("frames/{id}.pixel-art.json"))
            .is_file());
        assert!(run
            .out
            .join(format!("frames/{id}.pixel-fit.json"))
            .is_file());

        let art = run.read_json(&format!("frames/{id}.pixel-art.json"));
        assert_eq!(art["schema_version"], 2);
        assert!(art["role_cells"].as_array().unwrap().len() > 20);
        assert!(art["protected_bounds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|region| { region["id"] == "face" }));
    }
}

#[test]
fn dev_preview_pixel_cast_identity_frames_are_distinct() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let mut frame_payloads = BTreeSet::new();
    for id in PIXEL_CAST_IDS {
        let frame = run.read_json(&format!("frames/{id}.pixel.json"));
        let pixels = frame["pixels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("");

        assert!(
            frame_payloads.insert(pixels),
            "{id} must not render the same pixel payload as another cast fixture"
        );
    }

    assert_eq!(
        frame_payloads.len(),
        PIXEL_CAST_IDS.len(),
        "all six Pixel cast frames must be visually distinct"
    );
}

#[test]
fn dev_preview_pixel_cast_matrix_references_real_cast_frames() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let matrix = scenario(&manifest, "pixel-cast-identity-matrix");
    let referenced = matrix["inputs"]["cast_frame_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(referenced, PIXEL_CAST_IDS);
    assert!(matrix["files"].get("pixel").is_none());
    assert!(matrix["files"].get("pixel_art").is_none());
    assert!(matrix["files"].get("pixel_fit").is_none());
    assert!(matrix["files"].get("pixel_composition").is_none());

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    for id in PIXEL_CAST_IDS {
        assert!(
            html.contains(&format!("data-pixel-frame=\"frames/{id}.pixel.json\"")),
            "matrix review must expose canvas for {id}"
        );
    }
}

#[test]
fn dev_preview_pixel_hero_cues_have_expected_coverage() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    for (id, cue) in [
        ("pixel-fuzz-s3-locket", "locket"),
        ("pixel-glitch-s4-repair", "repair_mark"),
        ("pixel-crystal-s5-facets", "facet"),
    ] {
        let art = run.read_json(&format!("frames/{id}.pixel-art.json"));
        let coverage = &art["cue_coverage"][cue];
        assert!(
            coverage["expected"].as_u64().unwrap() > 0,
            "{id} missing expected {cue}"
        );
        assert_eq!(
            coverage["expected"], coverage["present"],
            "{id} did not promote {cue}"
        );
    }
}

#[test]
fn dev_preview_pixel_fit_sidecar_records_each_review_geometry() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let fit = run.read_json("frames/pixel-fuzz-s3-content-idle.pixel-fit.json");
    let evidence = fit["geometry_evidence"].as_array().unwrap();
    let labels = evidence
        .iter()
        .map(|entry| entry["label"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(labels, ["min", "default", "large", "fullscreen"]);
    assert_eq!(evidence[0]["geometry"]["width"], 260);
    assert_eq!(evidence[1]["geometry"]["width"], 360);
    assert_eq!(evidence[2]["geometry"]["width"], 480);
    assert_eq!(evidence[3]["geometry"]["width"], 900);
    for entry in evidence {
        assert_eq!(entry["producer"], "round::pixel_fit::pixel_companion_fit");
        assert_eq!(entry["hud_overlap"]["body_eye_mouth_pixels"], 0);
        assert_eq!(entry["hud_overlap"]["translucent_effect_pixels"], 0);
    }
}

#[test]
fn dev_preview_pixel_privacy_outputs_omit_terminal_reference_rows() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let text =
        std::fs::read_to_string(run.out.join("frames/pixel-fuzz-s3-content-idle.txt")).unwrap();
    let cells =
        std::fs::read_to_string(run.out.join("frames/pixel-fuzz-s3-content-idle.cells.json"))
            .unwrap();
    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();

    for content in [&text, &cells, &html] {
        assert!(!content.contains("terminal reference"));
        assert!(!content.contains("/\\_/\\\\"));
        assert!(!content.contains("( o.o )"));
        assert!(!content.contains("very-secret-seed"));
    }
}

#[test]
fn dev_preview_pixel_composition_artifact_has_own_manifest_slot() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let scenario = scenario(&manifest, "pixel-tank-composition");

    assert_eq!(
        scenario["files"]["pixel"],
        "frames/pixel-tank-composition.pixel.json"
    );
    assert_eq!(
        scenario["files"]["pixel_art"],
        "frames/pixel-tank-composition.pixel-art.json"
    );
    assert_eq!(
        scenario["files"]["pixel_fit"],
        "frames/pixel-tank-composition.pixel-fit.json"
    );
    assert_eq!(
        scenario["files"]["pixel_composition"],
        "frames/pixel-tank-composition.pixel-composition.json"
    );
    assert!(run
        .out
        .join("frames/pixel-tank-composition.pixel.json")
        .is_file());
    assert!(run
        .out
        .join("frames/pixel-tank-composition.pixel-art.json")
        .is_file());
    assert!(run
        .out
        .join("frames/pixel-tank-composition.pixel-fit.json")
        .is_file());
    assert!(run
        .out
        .join("frames/pixel-tank-composition.pixel-composition.json")
        .is_file());
    assert_artifact_type(
        &manifest,
        "pixel-tank-composition-pixel-composition",
        "pixel-composition",
    );

    let composition = run.read_json("frames/pixel-tank-composition.pixel-composition.json");
    let art = run.read_json("frames/pixel-tank-composition.pixel-art.json");
    assert_eq!(composition["schema_version"], 1);
    assert_eq!(composition["frame_id"], "pixel-tank-composition");
    assert!(composition["protected_regions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|region| { region["id"] == "face" }));
    assert!(composition["context"]["surface"].is_string());
    assert_eq!(composition["context"]["props_available"], false);
    assert_eq!(composition["context"]["tank_life_available"], false);

    let deferred_contexts = composition["comparison"]["deferred_contexts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        deferred_contexts.contains(&"props-unavailable-for-pixel-runtime"),
        "unavailable prop comparison must be explicit"
    );
    assert!(
        deferred_contexts.contains(&"tank-life-unavailable-for-pixel-runtime"),
        "unavailable tank-life comparison must be explicit"
    );

    let composition_face = composition["protected_regions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|region| region["id"] == "face")
        .unwrap();
    let art_face = art["protected_bounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|region| region["id"] == "face")
        .unwrap();

    assert_ne!(
        composition_face["bounds"], art_face["bounds"],
        "composition sidecar must map protected regions into preview pixel coordinates"
    );
    assert!(
        composition_face["bounds"]["min_x"].as_u64().unwrap() > 10,
        "preview face bounds should be centered/scaled, not raw reference-cell coordinates"
    );
    assert!(
        composition_face["bounds"]["max_x"].as_u64().unwrap() < 96,
        "preview face bounds must stay inside the pixel frame"
    );
}

#[test]
fn dev_preview_pixel_summary_includes_fullscreen_fit_readiness() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let text =
        std::fs::read_to_string(run.out.join("frames/pixel-fuzz-s3-content-idle.txt")).unwrap();
    assert!(text.contains("fit fullscreen ready"));
}

#[test]
fn pixel_preview_uses_correct_fuzz_s3_label() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let text =
        std::fs::read_to_string(run.out.join("frames/pixel-fuzz-s3-content-idle.txt")).unwrap();
    assert!(text.contains("stage s3 pup"));
    assert!(!text.contains("archfuzz"));
}

#[test]
fn dev_preview_pixel_html_uses_pixel_dimensions_not_placeholder_cells() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    assert!(html.contains("96 x 96 logical pixels"));
    assert!(!html.contains("24 x 4 cells"));
}

#[test]
fn dev_preview_pixel_strips_meet_animation_contract() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let strips = manifest["strips"].as_array().unwrap();
    let idle = strips
        .iter()
        .find(|strip| strip["id"] == "pixel-idle")
        .expect("pixel idle strip");
    assert_eq!(idle["kind"], "pixel-animation");
    assert!(idle["frames"].as_array().unwrap().len() >= 48);
    assert_eq!(idle["frames"][0]["elapsed_ms"], 0);
    assert!(idle["frames"].as_array().unwrap().iter().any(|frame| {
        frame["phase"]
            .as_str()
            .is_some_and(|phase| phase.contains("blink"))
    }));
    assert!(run
        .out
        .join("strips/pixel-idle/frame-000.pixel.json")
        .is_file());
}

#[test]
fn dev_preview_pixel_feed_pulse_strip_decays_accent_aura() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let first = run.read_json("strips/pixel-feed-pulse/frame-000.pixel.json");
    let late = run.read_json("strips/pixel-feed-pulse/frame-047.pixel.json");
    let first_accent_alpha = pixel_alpha_sum_for_rgb(&first, "#f0a646");
    let late_accent_alpha = pixel_alpha_sum_for_rgb(&late, "#f0a646");

    assert!(first_accent_alpha > 0);
    assert!(
        late_accent_alpha < first_accent_alpha,
        "feed-pulse accent aura should decay across the exported strip: first={first_accent_alpha}, late={late_accent_alpha}"
    );
}

#[test]
fn dev_preview_pixel_artifacts_do_not_expose_raw_seed_or_private_fields() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let mut pixel_paths = Vec::new();
    collect_pixel_json_paths(&run.out, &mut pixel_paths);
    assert!(pixel_paths
        .iter()
        .any(|path| path.ends_with("strips/pixel-feed-pulse/frame-047.pixel.json")));

    let mut review_artifact_paths = Vec::new();
    collect_pixel_review_artifact_paths(&run.out, &mut review_artifact_paths);
    assert!(review_artifact_paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "pixel-fuzz-s3-content-idle.txt")
    }));
    assert!(review_artifact_paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "pixel-fuzz-s3-content-idle.cells.json")
    }));
    assert!(review_artifact_paths.iter().any(|path| {
        path.to_string_lossy()
            .contains("strips/pixel-feed-pulse/frame-047.cells.json")
    }));

    let mut text = std::fs::read_to_string(run.out.join("manifest.json")).unwrap();
    for path in pixel_paths.into_iter().chain(review_artifact_paths) {
        text.push_str(&std::fs::read_to_string(path).unwrap());
    }

    assert!(!text.contains("fixture-seed"));
    assert!(!text.contains("/Users/drew"));
    assert!(!text.contains("prompt"));
    assert!(!text.contains("response"));
    assert!(!text.contains("source_breakdown"));

    for entry in std::fs::read_dir(run.out.join("frames")).unwrap() {
        let path = entry.unwrap().path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(name.ends_with(".pixel-art.json")
            || name.ends_with(".pixel-composition.json")
            || name.ends_with(".pixel-fit.json"))
        {
            continue;
        }
        let sidecar = std::fs::read_to_string(&path).unwrap().to_lowercase();
        let sidecar: Value = serde_json::from_str(&sidecar).unwrap();
        assert_sidecar_json_values_are_sanitized(&sidecar, "pixel");
    }
}

#[test]
fn dev_preview_smooth_writes_manifest_and_review_artifacts() {
    let run = PreviewRun::new();

    run.run_success("smooth");

    for file in [
        format!("frames/{SMOOTH_BASELINE_ID}.txt"),
        format!("frames/{SMOOTH_BASELINE_ID}.cells.json"),
        format!("frames/{SMOOTH_PARITY_ID}.txt"),
        format!("frames/{SMOOTH_PARITY_ID}.cells.json"),
        format!("frames/{SMOOTH_PARITY_ID}.smooth-plan.json"),
        format!("frames/{SMOOTH_PARITY_ID}.smooth-parity.json"),
        format!("strips/{SMOOTH_MOTION_ID}/frame-000.txt"),
        format!("strips/{SMOOTH_MOTION_ID}/frame-000.cells.json"),
        format!("strips/{SMOOTH_MOTION_ID}/frame-000.smooth-motion.json"),
    ] {
        assert!(run.out.join(&file).is_file(), "missing {file}");
    }

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    assert_scenario(
        &manifest,
        SMOOTH_BASELINE_ID,
        "smooth",
        (52, 52),
        (
            &format!("frames/{SMOOTH_BASELINE_ID}.txt"),
            &format!("frames/{SMOOTH_BASELINE_ID}.cells.json"),
            None,
        ),
    );
    assert_scenario(
        &manifest,
        SMOOTH_PARITY_ID,
        "smooth",
        (52, 52),
        (
            &format!("frames/{SMOOTH_PARITY_ID}.txt"),
            &format!("frames/{SMOOTH_PARITY_ID}.cells.json"),
            None,
        ),
    );
    let parity = scenario(&manifest, SMOOTH_PARITY_ID);
    assert_eq!(
        parity["files"]["smooth_plan"],
        format!("frames/{SMOOTH_PARITY_ID}.smooth-plan.json")
    );
    assert_eq!(
        parity["files"]["smooth_parity"],
        format!("frames/{SMOOTH_PARITY_ID}.smooth-parity.json")
    );
    assert_artifact_type(
        &manifest,
        &format!("{SMOOTH_PARITY_ID}-smooth-plan"),
        "smooth-plan",
    );
    assert_artifact_type(
        &manifest,
        &format!("{SMOOTH_PARITY_ID}-smooth-parity"),
        "smooth-parity",
    );

    let strips = manifest["strips"].as_array().unwrap();
    let motion = strips
        .iter()
        .find(|strip| strip["id"] == SMOOTH_MOTION_ID)
        .expect("smooth motion strip");
    assert_eq!(motion["kind"], "smooth-motion");
    assert_eq!(motion["target_id"], "pet-body");
    assert!(motion["frames"].as_array().unwrap().len() >= 5);
    assert_eq!(
        motion["frames"][0]["files"]["smooth_motion"],
        format!("strips/{SMOOTH_MOTION_ID}/frame-000.smooth-motion.json")
    );
    assert_artifact_type(
        &manifest,
        &format!("{SMOOTH_MOTION_ID}-frame-000-smooth-motion"),
        "smooth-motion",
    );

    let review = std::fs::read_to_string(run.out.join("review.md")).unwrap();
    for needle in [
        format!("frames/{SMOOTH_PARITY_ID}.smooth-plan.json"),
        format!("frames/{SMOOTH_PARITY_ID}.smooth-parity.json"),
        format!("strips/{SMOOTH_MOTION_ID}/frame-000.smooth-motion.json"),
    ] {
        assert!(review.contains(&needle), "review.md missing {needle}");
    }
}

#[test]
fn dev_preview_smooth_sidecars_are_sanitized_and_report_parity() {
    let run = PreviewRun::new();

    run.run_success("smooth");

    let plan = run.read_json(&format!("frames/{SMOOTH_PARITY_ID}.smooth-plan.json"));
    let parity = run.read_json(&format!("frames/{SMOOTH_PARITY_ID}.smooth-parity.json"));
    let mut smooth_sidecars = Vec::new();
    collect_smooth_sidecar_paths(&run.out, &mut smooth_sidecars);

    assert_eq!(plan["schema_version"], 1);
    assert_eq!(plan["frame_id"], SMOOTH_PARITY_ID);
    assert!(plan["viewport"]["grid_cols"].is_u64());
    assert!(plan["parallax_focus_offset"]["x"].is_number());
    assert!(plan["parallax_focus_offset"]["y"].is_number());
    assert_eq!(plan["parallax_lifecycle_scale"], 1.0);
    for plane in ["far", "mid", "behind", "foreground"] {
        assert!(plan["parallax_planes"][plane]["x"].is_number());
        assert!(plan["parallax_planes"][plane]["y"].is_number());
    }
    assert!(plan["layers"].as_array().unwrap().len() >= 10);
    assert_canonical_smooth_layer_mapping(&plan["layers"], "smooth-plan");
    for layer in plan["layers"].as_array().unwrap() {
        let motion_binding = layer["motion_binding"].as_str().unwrap();
        assert!(matches!(
            motion_binding,
            "fixed" | "pet-attached" | "floor-projected" | "parallax"
        ));
        match motion_binding {
            "fixed" | "pet-attached" | "floor-projected" => {
                assert!(layer["depth_plane"].is_null())
            }
            "parallax" => assert!(matches!(
                layer["depth_plane"].as_str().unwrap(),
                "far" | "mid" | "behind" | "foreground"
            )),
            _ => unreachable!(),
        }
        assert!(layer["parallax_translation"]["x"].is_number());
        assert!(layer["parallax_translation"]["y"].is_number());
    }
    assert!(plan["chrome"]["hud_bounds"].is_array());
    assert!(plan["chrome"]["gauge_bounds"].is_array());
    assert_eq!(plan["privacy"]["source_names_visible"], false);
    assert_eq!(plan["privacy"]["exact_token_strings_visible"], false);
    assert_eq!(plan["privacy"]["project_names_visible"], false);
    assert_eq!(plan["privacy"]["file_paths_visible"], false);
    assert_eq!(plan["privacy"]["prompt_text_visible"], false);
    assert_eq!(plan["privacy"]["response_text_visible"], false);
    assert_eq!(plan["privacy"]["raw_diagnostics_visible"], false);
    assert_eq!(plan["privacy"]["unprojected_pet_seed_visible"], false);
    assert!(plan["layers"].as_array().unwrap().iter().any(|layer| {
        layer["role"] == "pet-body"
            && layer["item_count"].as_u64().unwrap() > 0
            && layer["transform"]["translation"]["y"].is_number()
    }));

    assert_eq!(parity["schema_version"], 1);
    assert_eq!(parity["frame_id"], SMOOTH_PARITY_ID);
    assert_eq!(parity["fixture_id"], SMOOTH_BASELINE_ID);
    assert_eq!(parity["exact_match"], true);
    assert_eq!(
        parity["classic_checksum"], parity["smooth_flatten_checksum"],
        "smooth parity checksum must exactly match classic baseline"
    );
    assert_eq!(parity["review_status"], "exact-match");
    assert_eq!(parity["missing_roles"], Value::Array(vec![]));
    assert_eq!(parity["required_roles"].as_array().unwrap().len(), 19);

    for path in smooth_sidecars {
        let sidecar: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_sidecar_json_values_are_sanitized(&sidecar, "smooth");
        assert_smooth_enum_strings_only_in_typed_fields(&sidecar, "smooth");
    }
}

#[test]
fn dev_preview_smooth_privacy_scan_covers_motion_sidecars() {
    let run = PreviewRun::new();
    run.run_success("smooth");

    let manifest = run.manifest();
    let mut scanned = Vec::new();
    collect_smooth_sidecar_paths(&run.out, &mut scanned);

    let scanned_rel: BTreeSet<String> = scanned
        .into_iter()
        .map(|path| {
            path.strip_prefix(&run.out)
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let expected_motion: BTreeSet<String> = manifest["strips"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|strip| strip["id"] == SMOOTH_MOTION_ID)
        .flat_map(|strip| strip["frames"].as_array().unwrap().iter())
        .map(|frame| {
            frame["files"]["smooth_motion"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    assert!(
        !expected_motion.is_empty(),
        "smooth manifest should declare motion sidecars"
    );
    assert!(
        expected_motion.is_subset(&scanned_rel),
        "privacy scan missed smooth motion sidecars: expected {expected_motion:?}, scanned {scanned_rel:?}"
    );
}

#[test]
fn dev_preview_smooth_motion_sidecars_show_fractional_progression_and_all_bundle_includes_them() {
    let run = PreviewRun::new();
    run.run_success("smooth");

    let manifest = run.manifest();
    let motion = manifest["strips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|strip| strip["id"] == SMOOTH_MOTION_ID)
        .expect("smooth motion strip");
    let frames = motion["frames"].as_array().unwrap();
    assert!(
        frames.len() >= 5,
        "smooth motion strip should export at least five frames"
    );

    let mut pet_visual_checksums = BTreeSet::new();
    let mut base_anchors = Vec::new();
    let mut final_anchors = Vec::new();
    let mut classic_snap_anchors = BTreeSet::new();
    let mut bob_offsets = BTreeSet::new();
    let mut semantic_tick_indices = Vec::new();
    let mut checksums_by_semantic_tick = BTreeMap::<u64, BTreeSet<u64>>::new();
    let mut saw_nonzero_focus = false;
    let mut saw_nonzero_plane = BTreeMap::from([
        ("far", false),
        ("mid", false),
        ("behind", false),
        ("foreground", false),
    ]);
    let mut saw_strict_resolved_ordering = false;
    let mut parallax_plane_summaries = Vec::<[(f32, f32); 4]>::new();
    let mut exported_parallax_aggregates = Vec::<[(f32, f32); 4]>::new();
    for frame in frames {
        let path = frame["files"]["smooth_motion"].as_str().unwrap();
        let artifact = run.read_json(path);
        assert_eq!(artifact["schema_version"], 1);
        assert_eq!(artifact["strip_id"], SMOOTH_MOTION_ID);
        assert_eq!(artifact["parallax_lifecycle_scale"], 1.0);
        assert!(artifact["now_unix_ms"].as_i64().is_some());
        let semantic_tick_index = artifact["semantic_art_tick_index"].as_u64().unwrap();
        let pet_visual_checksum = artifact["pet_visual_checksum"].as_u64().unwrap();
        assert_eq!(artifact["privacy"]["source_names_visible"], false);
        assert_eq!(artifact["privacy"]["exact_token_strings_visible"], false);
        assert_sidecar_json_values_are_sanitized(&artifact, "smooth-motion");
        assert_smooth_enum_strings_only_in_typed_fields(&artifact, "smooth-motion");

        let focus_x = artifact["parallax_focus_offset"]["x"].as_f64().unwrap();
        let focus_y = artifact["parallax_focus_offset"]["y"].as_f64().unwrap();
        saw_nonzero_focus |= focus_x != 0.0 || focus_y != 0.0;

        let mut plane_points = BTreeMap::new();
        let mut exported_aggregate = [(0.0_f32, 0.0_f32); 4];
        for (plane_index, plane) in ["far", "mid", "behind", "foreground"]
            .into_iter()
            .enumerate()
        {
            let x = artifact["parallax_planes"][plane]["x"].as_f64().unwrap();
            let y = artifact["parallax_planes"][plane]["y"].as_f64().unwrap();
            *saw_nonzero_plane.get_mut(plane).unwrap() |= x != 0.0 || y != 0.0;
            plane_points.insert(plane, (x, y));

            let max_x = artifact["max_adjacent_parallax_delta_by_plane"][plane]["x"]
                .as_f64()
                .unwrap();
            let max_y = artifact["max_adjacent_parallax_delta_by_plane"][plane]["y"]
                .as_f64()
                .unwrap();
            exported_aggregate[plane_index] = (max_x as f32, max_y as f32);
            assert!(
                max_x <= 0.15,
                "{plane} adjacent parallax x delta exceeded 0.15: {max_x}"
            );
            assert!(
                max_y <= 0.10,
                "{plane} adjacent parallax y delta exceeded 0.10: {max_y}"
            );
        }
        parallax_plane_summaries.push(
            ["far", "mid", "behind", "foreground"]
                .map(|plane| (plane_points[plane].0 as f32, plane_points[plane].1 as f32)),
        );
        exported_parallax_aggregates.push(exported_aggregate);

        let [far, mid, behind, foreground] =
            ["far", "mid", "behind", "foreground"].map(|plane| plane_points[plane]);
        let strict_x = far.0 != 0.0
            && far.0.abs() < mid.0.abs()
            && mid.0.abs() < behind.0.abs()
            && behind.0.abs() < foreground.0.abs();
        let strict_y = far.1 != 0.0
            && far.1.abs() < mid.1.abs()
            && mid.1.abs() < behind.1.abs()
            && behind.1.abs() < foreground.1.abs();
        saw_strict_resolved_ordering |= strict_x || strict_y;

        let base = &artifact["pet_motion"]["base_anchor"];
        let final_anchor = &artifact["pet_motion"]["final_anchor"];
        let snap = &artifact["pet_motion"]["classic_snap_anchor"];
        let bob = &artifact["pet_motion"]["bob_offset"];

        base_anchors.push((base["x"].as_f64().unwrap(), base["y"].as_f64().unwrap()));
        final_anchors.push((
            final_anchor["x"].as_f64().unwrap(),
            final_anchor["y"].as_f64().unwrap(),
        ));
        classic_snap_anchors.insert(format!(
            "{:.1}:{:.1}",
            snap["x"].as_f64().unwrap(),
            snap["y"].as_f64().unwrap()
        ));
        bob_offsets.insert(format!(
            "{:.4}:{:.4}",
            bob["x"].as_f64().unwrap(),
            bob["y"].as_f64().unwrap()
        ));
        semantic_tick_indices.push(semantic_tick_index);
        pet_visual_checksums.insert(pet_visual_checksum);
        checksums_by_semantic_tick
            .entry(semantic_tick_index)
            .or_default()
            .insert(pet_visual_checksum);
        assert_canonical_smooth_layer_mapping(&artifact["layer_transforms"], "smooth-motion");
        let mut saw_pet_body = false;
        for layer in artifact["layer_transforms"].as_array().unwrap() {
            let motion_binding = layer["motion_binding"].as_str().unwrap();
            assert!(matches!(
                motion_binding,
                "fixed" | "pet-attached" | "floor-projected" | "parallax"
            ));
            let parallax_x = layer["parallax_translation"]["x"].as_f64().unwrap();
            let parallax_y = layer["parallax_translation"]["y"].as_f64().unwrap();
            match motion_binding {
                "fixed" | "pet-attached" | "floor-projected" => {
                    assert!(layer["depth_plane"].is_null());
                    assert_eq!(parallax_x, 0.0);
                    assert_eq!(parallax_y, 0.0);
                }
                "parallax" => assert!(matches!(
                    layer["depth_plane"].as_str().unwrap(),
                    "far" | "mid" | "behind" | "foreground"
                )),
                _ => unreachable!(),
            }
            saw_pet_body |= layer["role"] == "pet-body"
                && layer["translation"]["y"].is_number()
                && layer["item_count"].as_u64().unwrap() > 0;
        }
        assert!(saw_pet_body);
    }

    let recomputed_aggregate =
        parallax_plane_summaries
            .windows(2)
            .fold([(0.0_f32, 0.0_f32); 4], |mut maximum, pair| {
                for plane_index in 0..4 {
                    let adjacent_x = (pair[1][plane_index].0 - pair[0][plane_index].0).abs();
                    let adjacent_y = (pair[1][plane_index].1 - pair[0][plane_index].1).abs();
                    maximum[plane_index].0 = maximum[plane_index].0.max(adjacent_x);
                    maximum[plane_index].1 = maximum[plane_index].1.max(adjacent_y);
                }
                maximum
            });
    for (frame_index, exported_aggregate) in exported_parallax_aggregates.iter().enumerate() {
        assert_eq!(
            *exported_aggregate, recomputed_aggregate,
            "frame {frame_index} should export the exact component-wise adjacent maximum"
        );
        assert_eq!(
            exported_aggregate, &exported_parallax_aggregates[0],
            "every motion sidecar should carry the same strip aggregate"
        );
    }

    assert!(
        base_anchors.windows(2).any(|pair| pair[0] != pair[1]),
        "expected base anchors to move across adjacent frames, got {base_anchors:?}"
    );
    assert!(
        classic_snap_anchors.len() >= 2,
        "expected classic snap anchor to cross at least two rounded cells, got {classic_snap_anchors:?}"
    );
    assert!(
        saw_nonzero_focus,
        "expected at least one non-zero parallax focus"
    );
    assert!(
        saw_nonzero_plane.values().all(|saw_nonzero| *saw_nonzero),
        "expected non-zero evidence for all four parallax planes, got {saw_nonzero_plane:?}"
    );
    assert!(
        saw_strict_resolved_ordering,
        "expected strict resolved Far < Mid < Behind < Foreground ordering on a non-zero axis"
    );
    assert!(
        bob_offsets.len() >= 5,
        "expected at least five distinct bob offsets, got {bob_offsets:?}"
    );
    let unique_semantic_tick_count = semantic_tick_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len();
    assert!(
        unique_semantic_tick_count < frames.len(),
        "expected paint frames to outnumber semantic ticks, got {unique_semantic_tick_count} ticks across {} frames",
        frames.len()
    );
    let mut paint_frames_per_tick = BTreeMap::<u64, usize>::new();
    for semantic_tick_index in semantic_tick_indices {
        *paint_frames_per_tick
            .entry(semantic_tick_index)
            .or_default() += 1;
    }
    assert!(
        paint_frames_per_tick.values().any(|count| *count > 1),
        "expected at least one semantic tick bucket to include multiple paint frames, got {paint_frames_per_tick:?}"
    );
    assert_eq!(
        pet_visual_checksums.len(),
        1,
        "Preview strip should prove paint motion changes without semantic art flashing"
    );
    for (semantic_tick_index, checksums) in checksums_by_semantic_tick {
        assert_eq!(
            checksums.len(),
            1,
            "expected stable paint checksum within semantic tick bucket {semantic_tick_index}, got {checksums:?}"
        );
    }
    for pair in final_anchors.windows(2) {
        let dx = (pair[1].0 - pair[0].0).abs();
        let dy = (pair[1].1 - pair[0].1).abs();
        assert!(
            dx < 1.0,
            "adjacent smooth x delta should stay sub-cell: {dx}"
        );
        assert!(
            dy < 1.0,
            "adjacent smooth y delta should stay sub-cell: {dy}"
        );
    }

    let all = PreviewRun::new();
    all.run_success("all");
    assert!(all
        .out
        .join(format!("frames/{SMOOTH_PARITY_ID}.smooth-plan.json"))
        .is_file());
    assert!(all
        .out
        .join(format!(
            "strips/{SMOOTH_MOTION_ID}/frame-000.smooth-motion.json"
        ))
        .is_file());
}

#[test]
fn dev_preview_html_contains_paused_strip_controls() {
    let run = PreviewRun::new();

    run.run_success("animation");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    assert!(html.contains("data-strip-id=\"scene-strip-smoke\""));
    assert!(html.contains("data-strip-play"));
    assert!(html.contains("aria-pressed=\"false\""));
    assert!(html.contains("data-strip-next"));
    assert!(html.contains("data-strip-prev"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("http://"));
}

#[test]
fn dev_preview_animation_strip_text_and_cells_are_repeatable() {
    let first = PreviewRun::new();
    let second = PreviewRun::new();

    first.run_success("animation");
    second.run_success("animation");

    for strip_id in [
        "scene-prop-resonance-ripple",
        "scene-feed-sweep",
        "scene-dawn-wake-wipe",
        "scene-heavy-session-shimmer",
    ] {
        for index in 0..3 {
            let text_path = format!("strips/{strip_id}/frame-{index:03}.txt");
            let cells_path = format!("strips/{strip_id}/frame-{index:03}.cells.json");
            assert_eq!(
                std::fs::read_to_string(first.out.join(&text_path)).unwrap(),
                std::fs::read_to_string(second.out.join(&text_path)).unwrap(),
                "{text_path} should be deterministic"
            );
            assert_eq!(
                std::fs::read_to_string(first.out.join(&cells_path)).unwrap(),
                std::fs::read_to_string(second.out.join(&cells_path)).unwrap(),
                "{cells_path} should be deterministic"
            );
        }
    }
}

// Snapshot guard for the deterministic `watch-wide-normal` preview frame. Any
// silent rendering regression (palette swap, layout drift, off-by-one in the
// composer, etc.) will diff the .snap file and fail the test. To accept an
// intentional rendering change, run `cargo insta review`.
#[test]
fn dev_preview_watch_wide_normal_frame_snapshot() {
    let run = PreviewRun::new();
    run.run_success("watch");

    let frame = std::fs::read_to_string(run.out.join("frames/watch-wide-normal.txt")).unwrap();

    insta::assert_snapshot!("watch_wide_normal_frame", frame);
}

#[test]
fn dev_preview_watch_daycontext_night_asleep_frame_snapshot() {
    let run = PreviewRun::new();
    run.run_success("watch");

    let frame =
        std::fs::read_to_string(run.out.join("frames/watch-daycontext-night-asleep.txt")).unwrap();

    insta::assert_snapshot!("watch_daycontext_night_asleep_frame", frame);
}

#[test]
fn dev_preview_watch_daycontext_heavy_day_evening_frame_snapshot() {
    let run = PreviewRun::new();
    run.run_success("watch");

    let frame = std::fs::read_to_string(
        run.out
            .join("frames/watch-daycontext-heavy-day-evening.txt"),
    )
    .unwrap();

    insta::assert_snapshot!("watch_daycontext_heavy_day_evening_frame", frame);
}

const ROUND_IDS: [&str; 14] = [
    "round-normal",
    "round-hud-missing-yesterday",
    "round-hud-stale-yesterday",
    "round-hud-zero-yesterday",
    "round-hud-over-yesterday",
    "round-hud-idle-pace",
    "round-hud-burst-pace",
    "round-active-pulse",
    "round-asleep-night",
    "round-helper-trouble",
    "round-flat-color",
    "round-glitch-dialect",
    "round-crystal-dialect",
    "round-glitch-patched-s6",
];

#[test]
fn dev_preview_round_writes_manifest_cells_and_round_metadata() {
    let run = PreviewRun::new();

    run.run_success("round");

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    for id in ROUND_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id}.txt"
        );
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id}.cells.json"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "round");
        assert_eq!(scenario["round"]["target_renderer"], "preview-cells");
        assert_eq!(scenario["round"]["aperture"]["shape"], "circle");
        assert!(
            scenario["round"]["aperture"]["safe_inner_radius"]
                .as_f64()
                .unwrap()
                > 0.0
        );
        assert_eq!(scenario["round"]["privacy"]["source_names_visible"], false);
        assert_eq!(scenario["round"]["privacy"]["exact_counts_visible"], false);
        assert_eq!(
            scenario["round"]["privacy"]["diagnostic_text_visible"],
            false
        );
    }
}

#[test]
fn dev_preview_round_writes_companion_hud_artifacts() {
    let run = PreviewRun::new();

    run.run_success("round");

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    let expected = [
        "round-normal",
        "round-hud-missing-yesterday",
        "round-hud-stale-yesterday",
        "round-hud-zero-yesterday",
        "round-hud-over-yesterday",
        "round-hud-idle-pace",
        "round-hud-burst-pace",
    ];

    for id in expected {
        assert!(
            run.out.join(format!("frames/{id}.hud.json")).is_file(),
            "missing {id}.hud.json"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["files"]["hud"], format!("frames/{id}.hud.json"));
        assert_artifact_type(&manifest, &format!("{id}-hud"), "hud");

        let hud = read_hud(&run, id);
        assert_eq!(hud["schema_version"], 2);
        assert_eq!(hud["frame_id"], id);
        assert_eq!(hud["gap_deg"], 70.0);
        assert_eq!(hud["lanes"]["xp"]["cap"], "round");
        assert_eq!(hud["lanes"]["daily"]["cap"], "round");
        assert_eq!(hud["lanes"]["pace"]["cap"], "round");
        assert!(
            hud["lanes"]["xp"]["stroke_width"].as_f64().unwrap()
                > hud["lanes"]["daily"]["stroke_width"].as_f64().unwrap()
        );
        assert!(
            hud["lanes"]["daily"]["stroke_width"].as_f64().unwrap()
                > hud["lanes"]["pace"]["stroke_width"].as_f64().unwrap()
        );
        assert!(hud["text"]["today_total"].is_string());
        assert!(hud["text"]["daily_percent"]
            .as_str()
            .unwrap()
            .ends_with(" yday"));
        assert!(hud["text"]["pace"].as_str().unwrap().ends_with("/10m"));
    }
}

#[test]
fn dev_preview_hud_artifacts_cover_daily_and_pace_states() {
    let run = PreviewRun::new();

    run.run_success("round");

    let missing = read_hud(&run, "round-hud-missing-yesterday");
    assert_eq!(missing["lanes"]["daily"]["fill_fraction"], 0.0);
    assert_eq!(missing["text"]["daily_percent"], "--% yday");

    let zero = read_hud(&run, "round-hud-zero-yesterday");
    assert_eq!(zero["lanes"]["daily"]["fill_fraction"], 0.0);
    assert_eq!(zero["text"]["daily_percent"], "--% yday");

    let over = read_hud(&run, "round-hud-over-yesterday");
    assert_eq!(over["lanes"]["daily"]["fill_fraction"], 1.0);
    let expected_overfill = 842_000_000.0 / 678_000_000.0 - 1.0;
    assert!(
        (over["lanes"]["daily"]["overfill_fraction"]
            .as_f64()
            .unwrap()
            - expected_overfill)
            .abs()
            < 1e-9,
        "over-yesterday HUD lane should show only the extra fraction as bright fill"
    );
    assert_eq!(over["text"]["daily_percent"], "124% yday");

    let idle = read_hud(&run, "round-hud-idle-pace");
    assert_eq!(idle["lanes"]["pace"]["fill_fraction"], 0.0);
    assert_eq!(idle["text"]["pace"], "0/10m");

    let burst = read_hud(&run, "round-hud-burst-pace");
    assert!(
        burst["lanes"]["pace"]["fill_fraction"].as_f64().unwrap() > 0.80,
        "burst pace should visibly fill the amber lane"
    );
}

#[test]
fn dev_preview_scene_artifacts_do_not_gain_companion_hud_metrics() {
    let run = PreviewRun::new();

    run.run_success("round");

    let scene_text =
        std::fs::read_to_string(run.out.join("frames/round-hud-over-yesterday.scene.json"))
            .unwrap();
    for forbidden in [
        "daily_comparison",
        "fraction_of_yesterday",
        "124% yday",
        "/10m",
        "842M",
    ] {
        assert!(
            !scene_text.contains(forbidden),
            "scene artifact leaked companion HUD metric {forbidden}: {scene_text}"
        );
    }
}

#[test]
fn dev_preview_review_surfaces_link_typed_artifacts() {
    let run = PreviewRun::new();

    run.run_success("round");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    let review = std::fs::read_to_string(run.out.join("review.md")).unwrap();
    let needle = "frames/round-normal.scene.json";
    assert!(html.contains(needle), "index.html missing {needle}");
    assert!(review.contains(needle), "review.md missing {needle}");
}

#[test]
fn dev_preview_round_output_has_no_dashboard_labels_or_private_source_text() {
    let run = PreviewRun::new();

    run.run_success("round");

    for id in ROUND_IDS {
        let text = std::fs::read_to_string(run.out.join(format!("frames/{id}.txt"))).unwrap();
        for forbidden in ["today", "rate", "helper", "tokens", "claude", "codex", "xp"] {
            assert!(
                !text.to_ascii_lowercase().contains(forbidden),
                "{id} leaked dashboard text {forbidden}: {text}"
            );
        }
    }
}

#[test]
fn dev_preview_round_aperture_corners_are_masked() {
    let run = PreviewRun::new();

    run.run_success("round");

    let cells = read_cells(&run, "round-normal");
    let top_left = cells["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["x"] == 0 && cell["y"] == 0)
        .unwrap();
    assert_eq!(top_left["symbol"], " ");
    assert_eq!(top_left["outside_aperture"], true);

    let center = cells["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["x"] == 26 && cell["y"] == 26)
        .unwrap();
    assert!(
        !center["outside_aperture"].as_bool().unwrap_or(false),
        "center cell should be inside the aperture"
    );
}

#[test]
fn dev_preview_tank_life_writes_typed_artifacts() {
    let run = PreviewRun::new();

    run.run_success("tank-life");

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);

    for id in TANK_LIFE_IDS {
        assert!(
            run.out.join(format!("frames/{id}.txt")).is_file(),
            "missing {id}.txt"
        );
        assert!(
            run.out
                .join(format!("frames/{id}.tank-life.json"))
                .is_file(),
            "missing {id}.tank-life.json"
        );

        let artifact = read_tank_life(&run, id);
        assert_eq!(artifact["schema_version"], 1);
        assert_eq!(artifact["frame_id"], id);
        assert!(artifact["local_date"].is_string());
        assert!(artifact["calendar_age_days"].is_i64());
        assert!(artifact["target_surface"].is_string());
        assert!(artifact["canonical_ids"].is_array());
        assert!(artifact["rendered_ids"].is_array());
        assert!(artifact["skipped"].is_array());
        assert!(artifact["placements"].is_array());
        assert!(artifact["collision_status"]["reserved_region_clear"].is_boolean());
    }
}

#[test]
fn dev_preview_all_includes_tank_life_artifacts() {
    let run = PreviewRun::new();

    run.run_success("all");

    assert!(run
        .out
        .join("frames/tank-life-round-projection.tank-life.json")
        .is_file());
}

#[test]
fn dev_preview_tank_life_anemone_fixture_shows_all_morphs_and_host() {
    let run = PreviewRun::new();

    run.run_success("tank-life");

    let text =
        std::fs::read_to_string(run.out.join("frames/tank-life-anemone-morphs.txt")).unwrap();
    let artifact = read_tank_life(&run, "tank-life-anemone-morphs");
    assert_eq!(
        artifact["anemone_morph"],
        serde_json::Value::Null,
        "catalog fixture shows every morph, so it should not report one selected morph"
    );
    for glyph in ["✺", "┬", "⌁", "⁙", "›"] {
        assert!(
            text.contains(glyph),
            "anemone morph fixture should show glyph {glyph:?}; text was:\n{text}"
        );
        assert!(
            tank_life_artifact_mentions_glyph(&artifact, glyph),
            "anemone morph artifact should describe glyph {glyph:?}; artifact was:\n{artifact:#}"
        );
    }
}

#[test]
fn dev_preview_round_glitch_and_crystal_differ_by_symbols() {
    let run = PreviewRun::new();

    run.run_success("round");

    let glitch = read_cells(&run, "round-glitch-dialect");
    let crystal = read_cells(&run, "round-crystal-dialect");
    let glitch_cells = glitch["cells"].as_array().unwrap();
    let crystal_cells = crystal["cells"].as_array().unwrap();
    assert!(
        changed_cells_by_symbol(glitch_cells, crystal_cells) >= 6,
        "Glitch and Crystal round previews should differ by symbols"
    );
}

fn assert_scenario(
    manifest: &Value,
    id: &str,
    kind: &str,
    dimensions: (u64, u64),
    files: (&str, &str, Option<&str>),
) {
    let scenario = manifest["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scenario| scenario["id"] == id)
        .unwrap_or_else(|| panic!("missing scenario {id}"));

    assert_eq!(scenario["kind"], kind);
    assert!(!scenario["title"].as_str().unwrap().is_empty());
    assert!(!scenario["intent"].as_str().unwrap().is_empty());
    assert_eq!(scenario["dimensions"]["width"], dimensions.0);
    assert_eq!(scenario["dimensions"]["height"], dimensions.1);
    assert_eq!(scenario["files"]["text"], files.0);
    assert_eq!(scenario["files"]["cells"], files.1);
    match files.2 {
        Some(path) => assert_eq!(scenario["files"]["layout"], path),
        None => assert!(scenario["files"].get("layout").is_none()),
    }
    assert!(scenario["inputs"].is_object());
    assert!(!scenario["review_prompts"].as_array().unwrap().is_empty());
}

fn scenario<'a>(manifest: &'a Value, id: &str) -> &'a Value {
    manifest["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scenario| scenario["id"] == id)
        .unwrap_or_else(|| panic!("missing scenario {id}"))
}

fn assert_artifact_type(manifest: &Value, id: &str, artifact_type: &str) {
    let artifact = manifest["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["id"] == id)
        .unwrap_or_else(|| panic!("missing artifact {id}"));
    assert_eq!(artifact["type"], artifact_type);
}

fn scenario_ids(manifest: &Value) -> Vec<String> {
    manifest["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .map(|scenario| scenario["id"].as_str().unwrap().to_string())
        .collect()
}

fn liveliness_dimensions(id: &str) -> (u64, u64) {
    if id == "watch-liveliness-compact-s6-hot" {
        (72, 24)
    } else {
        (120, 32)
    }
}

fn alive_room_dimensions(id: &str) -> (u64, u64) {
    if id == "room-mixed-full-wide" {
        (180, 50)
    } else if id == "room-dawn-wake-small" {
        (72, 24)
    } else {
        (120, 32)
    }
}

fn read_cells(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.cells.json")))
}

fn read_layout(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.layout.json")))
}

fn read_scene(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.scene.json")))
}

fn read_hud(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.hud.json")))
}

fn read_tank_life(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.tank-life.json")))
}

fn tank_life_artifact_mentions_glyph(artifact: &Value, glyph: &str) -> bool {
    artifact["placements"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|placement| placement["cells"].as_array().into_iter().flatten())
        .any(|cell| cell["glyph"] == glyph)
}

fn preview_scenarios_with_contract_scene(manifest: &Value) -> Vec<String> {
    manifest["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|scenario| matches!(scenario["kind"].as_str(), Some("watch" | "round")))
        .map(|scenario| scenario["id"].as_str().unwrap().to_string())
        .collect()
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn cells_for_target(cells: &Value, layout: &Value, target: &str) -> Vec<Value> {
    let rect = &layout["targets"][target];
    let x = rect["x"].as_u64().unwrap();
    let y = rect["y"].as_u64().unwrap();
    let width = rect["width"].as_u64().unwrap();
    let height = rect["height"].as_u64().unwrap();

    cells["cells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|cell| {
            let cell_x = cell["x"].as_u64().unwrap();
            let cell_y = cell["y"].as_u64().unwrap();
            cell_x >= x && cell_x < x + width && cell_y >= y && cell_y < y + height
        })
        .cloned()
        .collect()
}

fn changed_cells_by_symbol_or_fg(a: &[Value], b: &[Value]) -> usize {
    assert_eq!(
        a.len(),
        b.len(),
        "cell captures must cover the same rect size"
    );
    a.iter()
        .zip(b)
        .filter(|(left, right)| left["symbol"] != right["symbol"] || left["fg"] != right["fg"])
        .count()
}

fn changed_cells_by_symbol(a: &[Value], b: &[Value]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .filter(|(left, right)| left["symbol"] != right["symbol"])
        .count()
}

fn changed_room_zones(
    a: &[Value],
    b: &[Value],
    width: u64,
    height: u64,
) -> std::collections::BTreeSet<&'static str> {
    let mut zones = std::collections::BTreeSet::new();
    for (left, right) in a.iter().zip(b) {
        if left["symbol"] == right["symbol"] {
            continue;
        }
        let x = left["x"].as_u64().unwrap();
        let y = left["y"].as_u64().unwrap();
        let zone = if y < height / 3 {
            "upper-air"
        } else if y > height * 2 / 3 {
            "floor"
        } else if x < width / 3 {
            "left-anchor"
        } else if x > width * 2 / 3 {
            "right-anchor"
        } else {
            "pet-adjacent"
        };
        zones.insert(zone);
    }
    zones
}

/// Minimum number of masked room symbols that must differ between two species dialects.
const MIN_DIALECT_SYMBOL_DIFFERENCES: usize = 12;
/// Minimum number of room zones that must show dialect differences.
const MIN_DIALECT_DIFFERENT_ZONES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestRect {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

fn target_rect(layout: &Value, target: &str) -> TestRect {
    let rect = &layout["targets"][target];
    TestRect {
        x: rect["x"].as_u64().unwrap(),
        y: rect["y"].as_u64().unwrap(),
        width: rect["width"].as_u64().unwrap(),
        height: rect["height"].as_u64().unwrap(),
    }
}

fn rect_contains(rect: TestRect, cell: &Value) -> bool {
    let x = cell["x"].as_u64().unwrap();
    let y = cell["y"].as_u64().unwrap();
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

fn prop_target_ids(layout: &Value) -> Vec<&str> {
    layout["targets"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|id| id.starts_with("watch.prop."))
        .map(|s| s.as_str())
        .collect()
}

/// Collects the union of prop mask regions from both layouts. Shared prop IDs
/// may produce duplicate rects, which is harmless for masking.
fn union_prop_mask_rects(left: &Value, right: &Value) -> Vec<TestRect> {
    let mut ids = prop_target_ids(left);
    ids.extend(prop_target_ids(right));
    ids.sort();
    ids.dedup();

    let mut rects = Vec::new();
    for id in ids {
        if left["targets"].get(id).is_some() {
            rects.push(target_rect(left, id));
        }
        if right["targets"].get(id).is_some() {
            rects.push(target_rect(right, id));
        }
    }
    rects
}

/// Blanks out pet art/speech and shared prop regions so only room-dialect differences remain.
fn masked_room_cells(cells: &Value, layout: &Value, pair_layout: &Value) -> Vec<Value> {
    let mut masks = vec![target_rect(layout, "watch.pet.art")];
    if layout["targets"].get("watch.pet.speech").is_some() {
        masks.push(target_rect(layout, "watch.pet.speech"));
    }
    masks.extend(union_prop_mask_rects(layout, pair_layout));

    cells_for_target(cells, layout, "watch.room.effect")
        .into_iter()
        .map(|mut cell| {
            if masks.iter().any(|mask| rect_contains(*mask, &cell)) {
                cell["symbol"] = Value::String(" ".to_string());
            }
            cell
        })
        .collect()
}

fn local_asset_refs(html: &str) -> Vec<String> {
    ["href=\"assets/", "src=\"assets/"]
        .into_iter()
        .flat_map(|needle| asset_refs_with_prefix(html, needle))
        .collect()
}

fn asset_refs_with_prefix(html: &str, needle: &str) -> Vec<String> {
    html.match_indices(needle)
        .map(|(index, _)| {
            let start = index + needle.find("assets/").unwrap();
            let rest = &html[start..];
            let end = rest.find('"').unwrap();
            rest[..end].to_string()
        })
        .collect()
}
