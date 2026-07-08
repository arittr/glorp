#![cfg(feature = "dev-preview")]

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::PathBuf;
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
    assert_eq!(manifest["schema_version"], 7);
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
    assert_eq!(manifest["schema_version"], 7);
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
        "frames/tank-life-age-empty.txt",
        "frames/tank-life-age-first.txt",
        "frames/tank-life-age-early.txt",
        "frames/tank-life-age-full.txt",
        "frames/tank-life-date-2026-07-07.txt",
        "frames/tank-life-date-2026-07-08.txt",
        "frames/tank-life-round-projection.txt",
        "frames/tank-life-anemone-morphs.txt",
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
            "tank-life-age-empty".to_string(),
            "tank-life-age-first".to_string(),
            "tank-life-age-early".to_string(),
            "tank-life-age-full".to_string(),
            "tank-life-date-2026-07-07".to_string(),
            "tank-life-date-2026-07-08".to_string(),
            "tank-life-round-projection".to_string(),
            "tank-life-anemone-morphs".to_string(),
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
    assert_eq!(manifest["schema_version"], 7);
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
    assert_eq!(manifest["schema_version"], 7);
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
    assert_eq!(manifest["schema_version"], 7);
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
    assert_eq!(manifest["schema_version"], 7);

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
