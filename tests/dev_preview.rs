#![cfg(feature = "dev-preview")]

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

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

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 2);
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
    assert_eq!(manifest["schema_version"], 2);
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
    assert_eq!(layout["schema_version"], 1);
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
    let scenario = scenario(&manifest, "pet-species-stage");
    assert_eq!(scenario["inputs"]["species"][0], "fuzz");
    assert_eq!(
        scenario["inputs"]["stages"][6]["labels_by_species"]["mech"],
        "titan"
    );
}

#[test]
fn dev_preview_all_writes_watch_and_pet_artifacts() {
    let run = PreviewRun::new();

    run.run_success("all");

    for file in [
        "frames/watch-wide-normal.txt",
        "frames/watch-tall-wide.txt",
        "frames/watch-compact-normal.txt",
        "frames/pet-species-stage.txt",
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
            "pet-species-stage".to_string(),
        ]
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
