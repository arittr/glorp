# Glorp Presentation Contract Freeze Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Preview Lab strong enough to protect the upcoming Glorp presentation refactor by adding typed scene, round layout, and round command artifacts without changing existing visual output.

**Spec:** `docs/superpowers/specs/2026-06-15-glorp-presentation-architecture-design.md`.

**Architecture:** Keep `WatchViewModel` as the source-of-truth compatibility anchor and add sanitized dev-preview contract DTOs at the preview edge. Watch and round frames attach typed contract artifacts while they still have access to the source `WatchViewModel` or derived `RoundSceneModel`; `generate_preview_bundle` writes those artifacts and advertises them through the manifest, review markdown, and HTML. Extract the existing pure round draw-command builder from the macOS-gated companion module into `src/round/draw.rs` so Preview Lab can compare the same command vocabulary used by native companion rendering on every CI platform.

**Tech Stack:** Rust 2021, serde/serde_json, ratatui Preview Lab cells, existing `RoundSceneModel` and `RoundSceneLayout`, existing `RoundDrawCommand` vocabulary, assert_cmd integration tests, Cargo dev-preview feature.

**Branch guidance:** Work on the current branch unless Drew explicitly creates another branch. Commit after each task that reaches green verification. Stage only the files listed in the task being committed.

---

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `tests/dev_preview.rs` | Modify | Add failing contract tests for scene artifacts, round layout artifacts, round command artifacts, privacy redaction, HTML/review links, and deterministic animation strips. |
| `src/dev_preview/contract.rs` | Create | Sanitized serializable DTOs for `PreviewSceneArtifact`, `PreviewRoundLayoutArtifact`, and `PreviewRoundCommandsArtifact`; conversion helpers from `WatchViewModel`, `RoundSceneModel`, `RoundSceneLayout`, and round draw commands. |
| `src/dev_preview/mod.rs` | Modify | Expose the new `contract` module behind the existing `dev-preview` feature. |
| `src/dev_preview/frame.rs` | Modify | Add a skipped `PreviewFrameContract` field so frame producers can attach typed artifacts without changing cell JSON. |
| `src/dev_preview/watch.rs` | Modify | Attach a sanitized scene artifact to every watch frame derived from `WatchViewModel`. |
| `src/dev_preview/round.rs` | Modify | Keep current fixture IDs but ensure all round preview frames carry scene, layout, and command contract artifacts. |
| `src/round/preview.rs` | Modify | Build round frame contract artifacts at the same time as the preview cells. |
| `src/round/draw.rs` | Create | Cross-platform pure round draw command module moved from macOS companion rendering. |
| `src/round/mod.rs` | Modify | Export `draw`. |
| `src/companion/render.rs` | Modify | Re-export the pure draw command API from `src/round/draw.rs` so existing macOS imports remain stable. |
| `src/dev_preview/export.rs` | Modify | Add manifest file fields, artifact types, JSON writers, review markdown links, and HTML links for typed artifacts. |
| `src/dev_preview/scenarios.rs` | Modify | Write typed artifacts, register them in the manifest, and use the deterministic preview clock for scene strips. |
| `src/dev_preview/strips.rs` | Modify | Accept a fixed preview clock for real scene strips and build strip render contexts with `WatchClock::fixed`. |
| `AGENTS.md` | Modify | Update the Preview Lab artifact list and manifest schema text from schema `2` to the current schema `3` plus additive typed artifacts. |
| `docs/superpowers/specs/2026-05-12-glorp-preview-lab-animation-strips-design.md` | Modify | Add a short note near the schema `2` section that the historical slice is superseded by schema `3`. |

## Allowed Writes

- Additive Preview Lab artifacts under `frames/<id>.scene.json`, `frames/<id>.round-layout.json`, and `frames/<id>.round-commands.json`.
- Additive manifest fields under `PreviewScenarioFiles`.
- Additive artifact types in `PreviewManifest.artifacts`.
- A pure module move for round draw commands from the macOS-gated companion layer to `src/round/draw.rs`.
- Documentation corrections for living Preview Lab instructions and a small supersession note in the historical animation-strip spec.

## Forbidden Changes

- Do not rename or remove any existing `frames/*.txt`, `frames/*.cells.json`, `frames/*.layout.json`, `frames/*.room.txt`, `frames/*.room-masked.txt`, or `strips/**` artifact path.
- Do not change rendered watch, pet matrix, props, round, or animation strip text/cells except for eliminating wall-clock nondeterminism from real scene strips.
- Do not introduce `src/presentation/` in this plan.
- Do not replace `WatchViewModel`.
- Do not change usage, XP, mood, storage, provider, native companion lifecycle, or menubar behavior.
- Do not serialize raw `WatchViewModel` values into Preview Lab artifacts.

## Task 1: Add Failing Preview Contract Tests

**Files:**
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Add typed artifact path helpers**

Add these helpers near `read_layout`:

```rust
fn read_scene(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.scene.json")))
}

fn read_round_layout_artifact(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.round-layout.json")))
}

fn read_round_commands_artifact(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.round-commands.json")))
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
```

- [ ] **Step 2: Add the scene artifact coverage test**

Add this test near the other manifest/artifact tests:

```rust
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
        assert!(scene["room"]["primary_biome"].is_string(), "{id} primary biome");
        assert!(scene["privacy_projection"]["surface"].is_string(), "{id} surface");
        assert!(scene["privacy_projection"]["source_names_visible"].is_boolean());
        assert!(scene["targets"].is_object(), "{id} target map");
    }
}
```

- [ ] **Step 3: Add a privacy/redaction backstop for scene artifacts**

Add:

```rust
#[test]
fn dev_preview_scene_artifacts_are_sanitized_contracts_not_raw_runtime_state() {
    let run = PreviewRun::new();

    run.run_success("all");

    let manifest = run.manifest();
    for id in preview_scenarios_with_contract_scene(&manifest) {
        let scene_text =
            std::fs::read_to_string(run.out.join(format!("frames/{id}.scene.json"))).unwrap();
        for forbidden in [
            "/Users/",
            "/tmp/",
            "prompt",
            "response",
            "tool payload",
            "transcript",
            "client-secret-project",
            "123456",
            "99999",
        ] {
            assert!(
                !scene_text.to_ascii_lowercase().contains(forbidden),
                "{id}.scene.json leaked forbidden text {forbidden}: {scene_text}"
            );
        }
    }
}
```

- [ ] **Step 4: Add round layout and command artifact tests**

Add:

```rust
#[test]
fn dev_preview_round_writes_layout_and_command_artifacts() {
    let run = PreviewRun::new();

    run.run_success("round");

    let manifest = run.manifest();
    for id in ROUND_IDS {
        assert!(
            run.out.join(format!("frames/{id}.round-layout.json")).is_file(),
            "missing {id}.round-layout.json"
        );
        assert!(
            run.out.join(format!("frames/{id}.round-commands.json")).is_file(),
            "missing {id}.round-commands.json"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(
            scenario["files"]["round_layout"],
            format!("frames/{id}.round-layout.json")
        );
        assert_eq!(
            scenario["files"]["round_commands"],
            format!("frames/{id}.round-commands.json")
        );
        assert_artifact_type(&manifest, &format!("{id}-round-layout"), "round-layout");
        assert_artifact_type(
            &manifest,
            &format!("{id}-round-commands"),
            "round-commands",
        );

        let layout = read_round_layout_artifact(&run, id);
        assert_eq!(layout["schema_version"], 1);
        assert_eq!(layout["frame_id"], id);
        assert_eq!(layout["aperture"]["width"], 52);
        assert_eq!(layout["aperture"]["height"], 52);
        assert!(layout["safe_inner_radius"].as_f64().unwrap() > 0.0);
        assert_eq!(layout["pet_anchor"]["kind"], "pet");
        assert!(layout["prop_anchors"].as_array().unwrap().len() <= 2);
        assert!(layout["motion_budget"]["pet_breath"].is_boolean());

        let commands = read_round_commands_artifact(&run, id);
        assert_eq!(commands["schema_version"], 1);
        assert_eq!(commands["frame_id"], id);
        assert!(commands["command_counts"]["background"].as_u64().unwrap() >= 1);
        assert!(commands["command_counts"]["pet-glyph"].as_u64().unwrap() >= 1);
        assert!(
            commands["command_counts"]["room-glyph"].as_u64().unwrap() >= 1,
            "{id} should expose room glyph commands"
        );
        assert_eq!(
            commands["privacy_projection"]["source_names_visible"],
            false,
            "{id} command artifact should be glanceable-safe"
        );
    }
}
```

- [ ] **Step 5: Add a round semantic equivalence test**

Add:

```rust
#[test]
fn dev_preview_round_artifacts_match_scene_semantics() {
    let run = PreviewRun::new();

    run.run_success("round");

    for id in ROUND_IDS {
        let scene = read_scene(&run, id);
        let layout = read_round_layout_artifact(&run, id);
        let commands = read_round_commands_artifact(&run, id);

        assert_eq!(layout["fixture_id"], scene["fixture"]["id"], "{id} fixture id");
        assert_eq!(
            commands["fixture_id"], scene["fixture"]["id"],
            "{id} command fixture id"
        );
        assert_eq!(
            layout["pet_anchor"]["kind"], "pet",
            "{id} pet anchor should be semantic"
        );
        assert_eq!(
            commands["pet"]["text"],
            scene["pet"]["art_text"],
            "{id} pet glyph command should use scene pet text"
        );
        assert_eq!(
            commands["pet"]["span_count"],
            scene["pet"]["span_count"],
            "{id} pet span count"
        );
        assert_eq!(
            commands["room"]["glyph_vocabulary"],
            scene["room"]["glyph_vocabulary"],
            "{id} room glyph vocabulary"
        );
        assert_eq!(
            commands["privacy_projection"],
            scene["privacy_projection"],
            "{id} privacy projection"
        );
    }
}
```

- [ ] **Step 6: Add HTML and review link checks**

Add:

```rust
#[test]
fn dev_preview_review_surfaces_link_typed_artifacts() {
    let run = PreviewRun::new();

    run.run_success("round");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    let review = std::fs::read_to_string(run.out.join("review.md")).unwrap();
    for needle in [
        "frames/round-normal.scene.json",
        "frames/round-normal.round-layout.json",
        "frames/round-normal.round-commands.json",
    ] {
        assert!(html.contains(needle), "index.html missing {needle}");
        assert!(review.contains(needle), "review.md missing {needle}");
    }
}
```

- [ ] **Step 7: Add deterministic scene strip regression coverage**

Add:

```rust
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
```

- [ ] **Step 8: Run tests and confirm the intended failure**

Run:

```bash
cargo test --test dev_preview --features dev-preview dev_preview_watch_and_round_frames_write_scene_artifacts
```

Expected: FAIL because `files.scene`, `*.scene.json`, and `ArtifactType::Scene` do not exist.

- [ ] **Step 9: Commit the failing tests**

```bash
git add tests/dev_preview.rs
git commit -m "test: require typed Preview Lab artifacts"
```

## Task 2: Add Preview Contract DTOs and Frame Attachment

**Files:**
- Create: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/mod.rs`
- Modify: `src/dev_preview/frame.rs`
- Test: `src/dev_preview/contract.rs`

- [ ] **Step 1: Expose the new contract module**

Modify `src/dev_preview/mod.rs`:

```rust
pub mod contract;
```

- [ ] **Step 2: Add the skipped contract field to `PreviewFrame`**

Modify `src/dev_preview/frame.rs`:

```rust
use crate::dev_preview::contract::PreviewFrameContract;
```

Extend `PreviewFrame`:

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewFrame {
    pub id: String,
    pub title: String,
    pub width: u16,
    pub height: u16,
    pub cells: Vec<PreviewCell>,
    pub layout: Option<PreviewLayout>,
    #[serde(skip)]
    pub extra_inputs: BTreeMap<String, Value>,
    #[serde(skip)]
    pub contract: PreviewFrameContract,
}
```

Update every `PreviewFrame { ... }` initializer in `src/dev_preview/frame.rs`, `src/dev_preview/pets.rs`, `src/dev_preview/habitat_props.rs`, `src/dev_preview/scenarios.rs`, `src/dev_preview/export.rs`, `src/round/preview.rs`, and tests in `src/dev_preview/frame.rs` to include:

```rust
contract: PreviewFrameContract::default(),
```

- [ ] **Step 3: Create the scene and artifact DTOs**

Create `src/dev_preview/contract.rs`:

```rust
use crate::pet::render::StyledSegment;
use crate::round::layout::{
    RoundAnchor, RoundAnchorKind, RoundMotionBudget, RoundSceneLayout,
};
use crate::round::model::RoundSceneModel;
use crate::tui::component::PreviewLayout;
use crate::tui::room::{biome_symbols, derive_room_life_profile, RoomSpeciesDialect};
use crate::tui::view_model::WatchViewModel;
use serde::Serialize;
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub const CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewFrameContract {
    pub scene: Option<PreviewSceneArtifact>,
    pub round_layout: Option<PreviewRoundLayoutArtifact>,
    pub round_commands: Option<PreviewRoundCommandsArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewSceneArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub fixture: PreviewFixtureArtifact,
    pub privacy_projection: PreviewPrivacyProjection,
    pub pet: PreviewPetArtifact,
    pub room: PreviewRoomArtifact,
    pub activity: PreviewActivityArtifact,
    pub targets: BTreeMap<String, PreviewTargetArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewFixtureArtifact {
    pub id: String,
    pub source: String,
    pub fixed_now_unix: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPrivacyProjection {
    pub surface: String,
    pub source_names_visible: bool,
    pub exact_counts_visible: bool,
    pub diagnostic_text_visible: bool,
    pub feed_rows_visible: bool,
    pub file_paths_visible: bool,
    pub project_names_visible: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPetArtifact {
    pub seed: String,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub asleep: bool,
    pub art_text: String,
    pub span_count: usize,
    pub roles: Vec<String>,
    pub facing: i8,
    pub breath_offset_y: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoomArtifact {
    pub primary_biome: String,
    pub secondary_biome: Option<String>,
    pub species_dialect: String,
    pub dialect_status: Option<String>,
    pub work_weather: String,
    pub day_phase: String,
    pub prop_landmarks: Vec<String>,
    pub glyph_vocabulary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewActivityArtifact {
    pub source_diversity: String,
    pub helper_health: String,
    pub recent_activity: String,
    pub vitals: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTargetArtifact {
    pub role: String,
    pub layer: String,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}
```

- [ ] **Step 4: Add scene artifact builders**

Append to `src/dev_preview/contract.rs`:

```rust
impl PreviewSceneArtifact {
    pub fn from_watch_view_model(
        frame_id: &str,
        vm: &WatchViewModel,
        now: OffsetDateTime,
        layout: Option<&PreviewLayout>,
    ) -> Self {
        let room_profile = derive_room_life_profile(vm, now);
        let dialect = &room_profile.species_dialect;
        let room_dialect = RoomSpeciesDialect::for_species(vm.pet_render.generated_species);
        let glyph_vocabulary = biome_symbols(room_profile.biome.primary, room_dialect)
            .into_iter()
            .map(|ch| ch.to_string())
            .collect();

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture: PreviewFixtureArtifact {
                id: frame_id.to_string(),
                source: "watch-view-model".to_string(),
                fixed_now_unix: now.unix_timestamp(),
            },
            privacy_projection: PreviewPrivacyProjection::sanitized("preview-lab-scene"),
            pet: PreviewPetArtifact::from_watch(vm),
            room: PreviewRoomArtifact {
                primary_biome: format!("{:?}", room_profile.biome.primary),
                secondary_biome: room_profile.biome.secondary.map(|tag| format!("{tag:?}")),
                species_dialect: dialect.key.as_str().to_string(),
                dialect_status: Some(dialect.status.as_str().to_string()),
                work_weather: format!("{:?}", vm.life_profile.work_weather),
                day_phase: format!("{:?}", vm.day_context.day_phase),
                prop_landmarks: room_profile
                    .identity_prop_ids
                    .iter()
                    .map(|id| id.as_str().to_string())
                    .collect(),
                glyph_vocabulary,
            },
            activity: PreviewActivityArtifact::from_watch(vm),
            targets: layout.map(targets_from_preview_layout).unwrap_or_default(),
        }
    }

    pub fn from_round_scene(
        frame_id: &str,
        scene: &RoundSceneModel,
        now: OffsetDateTime,
    ) -> Self {
        let room_dialect = RoomSpeciesDialect::for_species(scene.pet.species);
        let glyph_vocabulary = biome_symbols(scene.room.biome.primary, room_dialect)
            .into_iter()
            .map(|ch| ch.to_string())
            .collect();

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture: PreviewFixtureArtifact {
                id: frame_id.to_string(),
                source: "round-scene-model".to_string(),
                fixed_now_unix: now.unix_timestamp(),
            },
            privacy_projection: PreviewPrivacyProjection::sanitized("round-preview"),
            pet: PreviewPetArtifact {
                seed: scene.pet.seed.clone(),
                species: scene.pet.species.as_str().to_string(),
                stage: format!("{:?}", scene.pet.stage).to_lowercase(),
                mood: format!("{:?}", scene.pet.mood).to_lowercase(),
                asleep: scene.pet.asleep,
                art_text: scene.pet.art_lines.join("\n"),
                span_count: scene.pet.art_spans.len(),
                roles: role_names(&scene.pet.art_spans),
                facing: scene.pet.facing,
                breath_offset_y: scene.pet.breath_offset_y,
            },
            room: PreviewRoomArtifact {
                primary_biome: format!("{:?}", scene.room.biome.primary),
                secondary_biome: scene.room.biome.secondary.map(|tag| format!("{tag:?}")),
                species_dialect: scene.room.dialect.as_str().to_string(),
                dialect_status: None,
                work_weather: format!("{:?}", scene.room.work_weather),
                day_phase: format!("{:?}", scene.room.day_phase),
                prop_landmarks: scene
                    .room
                    .prop_landmarks
                    .iter()
                    .map(|id| id.as_str().to_string())
                    .collect(),
                glyph_vocabulary,
            },
            activity: PreviewActivityArtifact::from_round(scene),
            targets: BTreeMap::new(),
        }
    }
}

impl PreviewPrivacyProjection {
    pub fn sanitized(surface: &str) -> Self {
        Self {
            surface: surface.to_string(),
            source_names_visible: false,
            exact_counts_visible: false,
            diagnostic_text_visible: false,
            feed_rows_visible: false,
            file_paths_visible: false,
            project_names_visible: false,
        }
    }
}
```

- [ ] **Step 5: Add helper conversions**

Append:

```rust
impl PreviewPetArtifact {
    fn from_watch(vm: &WatchViewModel) -> Self {
        Self {
            seed: vm.pet_render.seed.clone(),
            species: vm.pet_render.generated_species.as_str().to_string(),
            stage: format!("{:?}", vm.pet_render.stage).to_lowercase(),
            mood: format!("{:?}", vm.pet_render.mood).to_lowercase(),
            asleep: vm.day_context.asleep,
            art_text: vm.pet_art.join("\n"),
            span_count: vm.pet_spans.len(),
            roles: role_names(&vm.pet_spans),
            facing: vm.facing,
            breath_offset_y: vm.breath_offset_y,
        }
    }
}

impl PreviewActivityArtifact {
    fn from_watch(vm: &WatchViewModel) -> Self {
        let helper_health = if vm
            .source_health
            .iter()
            .any(|health| matches!(health.status, crate::tui::view_model::SourceStatus::Diagnostic))
        {
            "trouble"
        } else {
            "ok"
        };

        Self {
            source_diversity: format!("{:?}", vm.activity_identity.source_diversity),
            helper_health: helper_health.to_string(),
            recent_activity: vm
                .last_feed_pulse_at
                .map(|_| "recent")
                .unwrap_or("quiet")
                .to_string(),
            vitals: BTreeMap::from([
                ("fed".to_string(), vital_bucket(vm.fed).to_string()),
                ("happiness".to_string(), vital_bucket(vm.happiness).to_string()),
                ("energy".to_string(), vital_bucket(vm.energy).to_string()),
            ]),
        }
    }

    fn from_round(scene: &RoundSceneModel) -> Self {
        Self {
            source_diversity: format!("{:?}", scene.halo.source_diversity),
            helper_health: format!("{:?}", scene.halo.helper_health).to_lowercase(),
            recent_activity: format!("{:?}", scene.halo.activity_pulse).to_lowercase(),
            vitals: BTreeMap::from([
                ("fed".to_string(), format!("{:?}", scene.halo.vitals.fed).to_lowercase()),
                (
                    "happiness".to_string(),
                    format!("{:?}", scene.halo.vitals.happiness).to_lowercase(),
                ),
                (
                    "energy".to_string(),
                    format!("{:?}", scene.halo.vitals.energy).to_lowercase(),
                ),
            ]),
        }
    }
}

fn targets_from_preview_layout(layout: &PreviewLayout) -> BTreeMap<String, PreviewTargetArtifact> {
    layout
        .targets
        .iter()
        .map(|(id, target)| {
            (
                id.to_string(),
                PreviewTargetArtifact {
                    role: target.role.clone(),
                    layer: target.layer.clone(),
                    x: target.x,
                    y: target.y,
                    width: target.width,
                    height: target.height,
                },
            )
        })
        .collect()
}

fn role_names(spans: &[StyledSegment]) -> Vec<String> {
    let mut roles = spans
        .iter()
        .map(|span| format!("{:?}", span.role).to_lowercase())
        .collect::<Vec<_>>();
    roles.sort();
    roles.dedup();
    roles
}

fn vital_bucket(value: f64) -> &'static str {
    if value < 34.0 {
        "low"
    } else if value < 67.0 {
        "medium"
    } else {
        "high"
    }
}
```

- [ ] **Step 6: Add round layout artifact DTOs**

Append:

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundLayoutArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub fixture_id: String,
    pub aperture: PreviewRoundApertureArtifact,
    pub safe_inner_radius: f32,
    pub detail_level: String,
    pub pet_anchor: PreviewRoundAnchorArtifact,
    pub prop_anchors: Vec<PreviewRoundAnchorArtifact>,
    pub halo_anchors: Vec<PreviewRoundAnchorArtifact>,
    pub motion_budget: PreviewRoundMotionBudgetArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundApertureArtifact {
    pub width: u16,
    pub height: u16,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundAnchorArtifact {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoundMotionBudgetArtifact {
    pub pet_breath: bool,
    pub pet_blink: bool,
    pub activity_sweep: bool,
    pub prop_resonance: bool,
}

impl PreviewRoundLayoutArtifact {
    pub fn from_layout(frame_id: &str, layout: &RoundSceneLayout) -> Self {
        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture_id: frame_id.to_string(),
            aperture: PreviewRoundApertureArtifact {
                width: layout.aperture.width,
                height: layout.aperture.height,
                center_x: layout.aperture.center_x,
                center_y: layout.aperture.center_y,
                radius: layout.aperture.radius,
            },
            safe_inner_radius: layout.safe_inner_radius,
            detail_level: format!("{:?}", layout.detail_level).to_lowercase(),
            pet_anchor: round_anchor_artifact(&layout.pet_anchor),
            prop_anchors: layout.prop_anchors.iter().map(round_anchor_artifact).collect(),
            halo_anchors: layout.halo_anchors.iter().map(round_anchor_artifact).collect(),
            motion_budget: round_motion_budget_artifact(layout.motion_budget),
        }
    }
}

fn round_anchor_artifact(anchor: &RoundAnchor) -> PreviewRoundAnchorArtifact {
    PreviewRoundAnchorArtifact {
        kind: match anchor.kind {
            RoundAnchorKind::Pet => "pet",
            RoundAnchorKind::Prop => "prop",
            RoundAnchorKind::ActivityPulse => "activity-pulse",
            RoundAnchorKind::SourceDiversity => "source-diversity",
            RoundAnchorKind::Vital => "vital",
            RoundAnchorKind::HelperTrouble => "helper-trouble",
        }
        .to_string(),
        x: anchor.x,
        y: anchor.y,
        radius: anchor.radius,
    }
}

fn round_motion_budget_artifact(
    budget: RoundMotionBudget,
) -> PreviewRoundMotionBudgetArtifact {
    PreviewRoundMotionBudgetArtifact {
        pet_breath: budget.pet_breath,
        pet_blink: budget.pet_blink,
        activity_sweep: budget.activity_sweep,
        prop_resonance: budget.prop_resonance,
    }
}
```

- [ ] **Step 7: Add round command artifact DTO shapes**

Append the serializable command artifact types. The conversion from real draw commands is added in Task 3 after `src/round/draw.rs` exists.

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundCommandsArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub fixture_id: String,
    pub privacy_projection: PreviewPrivacyProjection,
    pub command_counts: BTreeMap<String, usize>,
    pub room: PreviewRoundRoomCommandSummary,
    pub pet: PreviewRoundPetCommandSummary,
    pub commands: Vec<PreviewRoundCommandArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoundRoomCommandSummary {
    pub glyph_vocabulary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoundPetCommandSummary {
    pub text: String,
    pub span_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundCommandArtifact {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub label: Option<String>,
    pub text_len: usize,
    pub span_count: usize,
    pub color_rgba: [f32; 4],
}
```

- [ ] **Step 8: Run focused compile checks**

Run:

```bash
cargo test --features dev-preview dev_preview::contract
cargo test --features dev-preview dev_preview::frame
```

Expected: PASS. If compile fails because `PreviewFrame` initializers are missing `contract`, update those initializers and rerun both commands.

- [ ] **Step 9: Commit the frame contract DTOs**

```bash
git add src/dev_preview/contract.rs src/dev_preview/mod.rs src/dev_preview/frame.rs src/round/preview.rs
git commit -m "feat: add Preview Lab contract DTOs"
```

## Task 3: Move Round Draw Commands to a Pure Round Module

**Files:**
- Create: `src/round/draw.rs`
- Modify: `src/round/mod.rs`
- Modify: `src/companion/render.rs`
- Modify: `src/dev_preview/contract.rs`
- Test: `src/round/draw.rs`

- [ ] **Step 1: Move the pure draw command implementation**

Create `src/round/draw.rs` by moving the pure contents of `src/companion/render.rs` into the new file. The top of the new file starts with:

```rust
use crate::pet::render::StyledSegment;
use crate::round::layout::{RoundAnchorKind, RoundSceneLayout};
use crate::round::model::RoundSceneModel;

#[derive(Debug, Clone, PartialEq)]
pub struct RoundDrawCommand {
    pub kind: RoundDrawKind,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub label: Option<char>,
    pub text: Option<String>,
    pub spans: Vec<StyledSegment>,
    pub color: RoundColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundDrawKind {
    Background,
    RoomGlyph,
    PropGlyph,
    PetGlyph,
    Halo,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundColor(pub f32, pub f32, pub f32, pub f32);
```

Move the existing constants and functions from `src/companion/render.rs` into `src/round/draw.rs` unchanged:

```rust
const PET_GLYPH_COLOR: RoundColor = RoundColor(0.93, 0.92, 0.89, 1.0);
const PROP_GLYPH_COLOR: RoundColor = RoundColor(0.70, 0.82, 0.52, 1.0);
const HALO_CALM_COLOR: RoundColor = RoundColor(0.36, 0.40, 0.55, 0.8);
const HALO_ACTIVE_COLOR: RoundColor = RoundColor(0.94, 0.65, 0.28, 0.9);
const TROUBLE_GLYPH_COLOR: RoundColor = RoundColor(0.92, 0.30, 0.25, 0.95);

pub(crate) fn biome_background_color(tag: crate::tui::room::RoomBiomeTag) -> RoundColor {
    use crate::tui::room::RoomBiomeTag;
    match tag {
        RoomBiomeTag::Starter => RoundColor(0.08, 0.09, 0.10, 1.0),
        RoomBiomeTag::Botanical => RoundColor(0.07, 0.11, 0.08, 1.0),
        RoomBiomeTag::Technical => RoundColor(0.07, 0.09, 0.13, 1.0),
        RoomBiomeTag::Celestial => RoundColor(0.08, 0.08, 0.14, 1.0),
        RoomBiomeTag::Artifact => RoundColor(0.12, 0.10, 0.07, 1.0),
        RoomBiomeTag::Cozy => RoundColor(0.13, 0.09, 0.08, 1.0),
    }
}

pub(crate) fn phase_dim_background(
    tag: crate::tui::room::RoomBiomeTag,
    phase: crate::tui::day::DayPhase,
) -> RoundColor {
    use crate::tui::day::DayPhase;
    let base = biome_background_color(tag);
    let k = match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.85,
        DayPhase::Dusk => 0.8,
        DayPhase::Night => 0.6,
    };
    RoundColor(base.0 * k, base.1 * k, base.2 * k, base.3)
}
```

Move `build_draw_commands`, `push_room_glyph_commands`, `push_pet_art_command`, and the current `#[cfg(test)] mod tests` from `src/companion/render.rs` to `src/round/draw.rs` without changing their bodies. This is a mechanical move; the function names and public structs stay the same.

- [ ] **Step 2: Export the draw module**

Modify `src/round/mod.rs`:

```rust
pub mod draw;
```

- [ ] **Step 3: Keep the companion import path stable**

Replace the contents of `src/companion/render.rs` with a re-export:

```rust
pub use crate::round::draw::{
    build_draw_commands, RoundColor, RoundDrawCommand, RoundDrawKind,
};
```

This preserves the existing `crate::companion::render::{...}` imports in `src/companion/app.rs` while making the implementation available to Preview Lab on every platform.

- [ ] **Step 4: Add command artifact conversion**

Append to `src/dev_preview/contract.rs`:

```rust
use crate::round::draw::{RoundDrawCommand, RoundDrawKind};

impl PreviewRoundCommandsArtifact {
    pub fn from_commands(
        frame_id: &str,
        scene: &RoundSceneModel,
        commands: &[RoundDrawCommand],
    ) -> Self {
        let mut command_counts = BTreeMap::new();
        for command in commands {
            *command_counts.entry(round_draw_kind_name(command.kind).to_string()).or_insert(0) += 1;
        }
        let pet = commands
            .iter()
            .find(|command| command.kind == RoundDrawKind::PetGlyph)
            .map(|command| PreviewRoundPetCommandSummary {
                text: command.text.clone().unwrap_or_default(),
                span_count: command.spans.len(),
            })
            .unwrap_or_else(|| PreviewRoundPetCommandSummary {
                text: String::new(),
                span_count: 0,
            });
        let room_dialect = RoomSpeciesDialect::for_species(scene.pet.species);
        let glyph_vocabulary = biome_symbols(scene.room.biome.primary, room_dialect)
            .into_iter()
            .map(|ch| ch.to_string())
            .collect();

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture_id: frame_id.to_string(),
            privacy_projection: PreviewPrivacyProjection::sanitized("round-preview"),
            command_counts,
            room: PreviewRoundRoomCommandSummary { glyph_vocabulary },
            pet,
            commands: commands.iter().map(round_command_artifact).collect(),
        }
    }
}

fn round_command_artifact(command: &RoundDrawCommand) -> PreviewRoundCommandArtifact {
    PreviewRoundCommandArtifact {
        kind: round_draw_kind_name(command.kind).to_string(),
        x: command.x,
        y: command.y,
        radius: command.radius,
        label: command.label.map(|ch| ch.to_string()),
        text_len: command.text.as_ref().map(|text| text.len()).unwrap_or(0),
        span_count: command.spans.len(),
        color_rgba: [command.color.0, command.color.1, command.color.2, command.color.3],
    }
}

fn round_draw_kind_name(kind: RoundDrawKind) -> &'static str {
    match kind {
        RoundDrawKind::Background => "background",
        RoundDrawKind::RoomGlyph => "room-glyph",
        RoundDrawKind::PropGlyph => "prop-glyph",
        RoundDrawKind::PetGlyph => "pet-glyph",
        RoundDrawKind::Halo => "halo",
        RoundDrawKind::Trouble => "trouble",
    }
}
```

- [ ] **Step 5: Run draw module tests**

Run:

```bash
cargo test --lib round::draw
```

Expected: PASS with the tests that formerly lived under `companion::render`.

- [ ] **Step 6: Run macOS companion compile check**

Run:

```bash
cargo test --lib companion::render
```

Expected on macOS: PASS or zero tests with successful compile. On non-macOS: the module is cfg-gated and the command should not be used; run `cargo test --lib round::draw` instead.

- [ ] **Step 7: Commit the pure draw command module**

```bash
git add src/round/draw.rs src/round/mod.rs src/companion/render.rs src/dev_preview/contract.rs
git commit -m "refactor: expose round draw commands outside companion"
```

## Task 4: Attach Contract Artifacts to Watch and Round Frames

**Files:**
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/round/preview.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Attach scene artifacts in the watch frame renderer**

Modify `render_watch_frame_from_state_with_life` in `src/dev_preview/watch.rs` after `frame.layout = Some(preview_layout(id, &layout));`:

```rust
    frame.contract.scene = Some(crate::dev_preview::contract::PreviewSceneArtifact::from_watch_view_model(
        id,
        &vm,
        now,
        frame.layout.as_ref(),
    ));
```

Keep this inside the function that has the fully-stamped `WatchViewModel`, after all life/day/context overrides and pet rerendering have happened.

- [ ] **Step 2: Attach scene, layout, and command artifacts in the round preview renderer**

Modify `render_round_preview_frame_from_vm` in `src/round/preview.rs` after `mark_continuations(&mut cells, width);`:

```rust
    let commands = crate::round::draw::build_draw_commands(&scene, &layout);
    let mut frame = PreviewFrame {
        id: id.into(),
        title: title.into(),
        width,
        height,
        cells,
        layout: None,
        extra_inputs: Default::default(),
        contract: Default::default(),
    };
    frame.contract.scene = Some(crate::dev_preview::contract::PreviewSceneArtifact::from_round_scene(
        &frame.id,
        &scene,
        now,
    ));
    frame.contract.round_layout = Some(
        crate::dev_preview::contract::PreviewRoundLayoutArtifact::from_layout(&frame.id, &layout),
    );
    frame.contract.round_commands = Some(
        crate::dev_preview::contract::PreviewRoundCommandsArtifact::from_commands(
            &frame.id,
            &scene,
            &commands,
        ),
    );
    frame
```

Remove the old direct `PreviewFrame { ... }` return from this function.

- [ ] **Step 3: Run focused tests and confirm artifact files are still missing**

Run:

```bash
cargo test --test dev_preview --features dev-preview dev_preview_watch_and_round_frames_write_scene_artifacts
```

Expected: still FAIL because the frame contracts are attached in memory but `generate_preview_bundle` has not written them yet.

- [ ] **Step 4: Commit frame attachment**

```bash
git add src/dev_preview/watch.rs src/round/preview.rs
git commit -m "feat: attach Preview Lab frame contracts"
```

## Task 5: Write Typed Artifacts and Advertise Them

**Files:**
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Extend manifest file fields and artifact types**

Modify `PreviewScenarioFiles` in `src/dev_preview/export.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct PreviewScenarioFiles {
    pub text: PathBuf,
    pub cells: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_text: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_masked_text: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round_layout: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round_commands: Option<PathBuf>,
}
```

Extend `ArtifactType`:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactType {
    Text,
    Cells,
    Layout,
    Scene,
    RoundLayout,
    RoundCommands,
    Html,
    Review,
    Asset,
}
```

- [ ] **Step 2: Add JSON writer and path helpers**

Add to `src/dev_preview/export.rs`:

```rust
pub fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}
```

Add to `src/dev_preview/scenarios.rs` near `text_path`, `cells_path`, and `layout_path`:

```rust
fn scene_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.scene.json", frame.id))
}

fn round_layout_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.round-layout.json", frame.id))
}

fn round_commands_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.round-commands.json", frame.id))
}
```

- [ ] **Step 3: Write attached artifacts during bundle generation**

Modify the main frame-writing loop in `generate_preview_bundle` in `src/dev_preview/scenarios.rs`:

```rust
        if let Some(scene) = &frame.contract.scene {
            write_json_artifact(&staging_dir.join(scene_path(frame)), scene)?;
        }
        if let Some(round_layout) = &frame.contract.round_layout {
            write_json_artifact(&staging_dir.join(round_layout_path(frame)), round_layout)?;
        }
        if let Some(round_commands) = &frame.contract.round_commands {
            write_json_artifact(&staging_dir.join(round_commands_path(frame)), round_commands)?;
        }
```

Add `write_json_artifact` to the `use crate::dev_preview::export::{ ... }` import list.

- [ ] **Step 4: Populate `PreviewScenarioFiles`**

Modify `scenario_metadata` in `src/dev_preview/scenarios.rs`:

```rust
        files: PreviewScenarioFiles {
            text: text_path(frame),
            cells: cells_path(frame),
            layout: frame.layout.as_ref().map(|_| layout_path(frame)),
            room_text: frame.layout.as_ref().and_then(|layout| {
                if layout.targets.contains_key("watch.room.effect") {
                    Some(room_text_path(frame))
                } else {
                    None
                }
            }),
            room_masked_text: frame.layout.as_ref().and_then(|layout| {
                if layout.targets.contains_key("watch.room.effect")
                    && has_masked_room_artifact(&frame.id)
                {
                    Some(room_masked_text_path(frame))
                } else {
                    None
                }
            }),
            scene: frame.contract.scene.as_ref().map(|_| scene_path(frame)),
            round_layout: frame
                .contract
                .round_layout
                .as_ref()
                .map(|_| round_layout_path(frame)),
            round_commands: frame
                .contract
                .round_commands
                .as_ref()
                .map(|_| round_commands_path(frame)),
        },
```

- [ ] **Step 5: Register artifacts in the manifest**

Modify `artifacts_for_frames` in `src/dev_preview/scenarios.rs`:

```rust
        if frame.contract.scene.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-scene", frame.id),
                title: format!("{} Scene", frame.title),
                artifact_type: ArtifactType::Scene,
                path: scene_path(frame),
                width: None,
                height: None,
            });
        }
        if frame.contract.round_layout.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-round-layout", frame.id),
                title: format!("{} Round Layout", frame.title),
                artifact_type: ArtifactType::RoundLayout,
                path: round_layout_path(frame),
                width: None,
                height: None,
            });
        }
        if frame.contract.round_commands.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-round-commands", frame.id),
                title: format!("{} Round Commands", frame.title),
                artifact_type: ArtifactType::RoundCommands,
                path: round_commands_path(frame),
                width: None,
                height: None,
            });
        }
```

- [ ] **Step 6: Link typed artifacts from review markdown**

Modify `write_review_markdown` in `src/dev_preview/export.rs` after the masked-room link:

```rust
            if let Some(scene) = &scenario.files.scene {
                markdown.push_str(&format!("- Scene: `{}`\n", scene.display()));
            }
            if let Some(round_layout) = &scenario.files.round_layout {
                markdown.push_str(&format!("- Round layout: `{}`\n", round_layout.display()));
            }
            if let Some(round_commands) = &scenario.files.round_commands {
                markdown.push_str(&format!("- Round commands: `{}`\n", round_commands.display()));
            }
```

- [ ] **Step 7: Link typed artifacts from HTML**

Modify `render_frame_artifact_links` in `src/dev_preview/export.rs`:

```rust
    if frame.contract.scene.is_some() {
        links.push(format!(
            r#"<a href="{}">scene</a>"#,
            escape_html(&format!("frames/{}.scene.json", frame.id))
        ));
    }
    if frame.contract.round_layout.is_some() {
        links.push(format!(
            r#"<a href="{}">round layout</a>"#,
            escape_html(&format!("frames/{}.round-layout.json", frame.id))
        ));
    }
    if frame.contract.round_commands.is_some() {
        links.push(format!(
            r#"<a href="{}">round commands</a>"#,
            escape_html(&format!("frames/{}.round-commands.json", frame.id))
        ));
    }
```

- [ ] **Step 8: Run the contract test set**

Run:

```bash
cargo test --test dev_preview --features dev-preview dev_preview_watch_and_round_frames_write_scene_artifacts
cargo test --test dev_preview --features dev-preview dev_preview_scene_artifacts_are_sanitized_contracts_not_raw_runtime_state
cargo test --test dev_preview --features dev-preview dev_preview_round_writes_layout_and_command_artifacts
cargo test --test dev_preview --features dev-preview dev_preview_round_artifacts_match_scene_semantics
cargo test --test dev_preview --features dev-preview dev_preview_review_surfaces_link_typed_artifacts
```

Expected: PASS.

- [ ] **Step 9: Commit artifact writing**

```bash
git add src/dev_preview/export.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "feat: write typed Preview Lab artifacts"
```

## Task 6: Make Real Scene Strips Use the Deterministic Preview Clock

**Files:**
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/strips.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Thread `PreviewRenderContext` into real strip builders**

Modify `generate_preview_bundle` in `src/dev_preview/scenarios.rs`:

```rust
            strips.push(crate::dev_preview::strips::scene_strip_smoke());
            strips.extend(crate::dev_preview::strips::scene_strips(&ctx));
```

Make the same change in the `PreviewSelection::Animation` arm.

- [ ] **Step 2: Change `scene_strips` to accept the preview context**

Modify `src/dev_preview/strips.rs`:

```rust
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::tui::render_context::{RenderContext, WatchClock};
```

Change the real strip builder signature:

```rust
fn scene_strip_bundle(
    strip_id: &'static str,
    title: &'static str,
    target_id: &'static str,
    intent: &'static str,
    vm: &WatchViewModel,
    width: u16,
    height: u16,
    moment: &crate::tui::room::SceneMoment,
    fixed_now: time::OffsetDateTime,
) -> PreviewStripBundle {
    let ctx = RenderContext::with_clock(
        ColorCapability::Truecolor,
        WatchClock::fixed(fixed_now),
    );
    /* existing body after the old ctx line */
}
```

Update every call to `scene_strip_bundle` to pass `ctx.fixed_now`.

- [ ] **Step 3: Change public strip constructors**

Change each real strip constructor to accept the preview context:

```rust
pub fn scene_prop_resonance_ripple(ctx: &PreviewRenderContext) -> PreviewStripBundle { /* existing body */ }
pub fn scene_feed_sweep(ctx: &PreviewRenderContext) -> PreviewStripBundle { /* existing body */ }
pub fn scene_dawn_wake_wipe(ctx: &PreviewRenderContext) -> PreviewStripBundle { /* existing body */ }
pub fn scene_heavy_session_shimmer(ctx: &PreviewRenderContext) -> PreviewStripBundle { /* existing body */ }

pub fn scene_strips(ctx: &PreviewRenderContext) -> Vec<PreviewStripBundle> {
    vec![
        scene_prop_resonance_ripple(ctx),
        scene_feed_sweep(ctx),
        scene_dawn_wake_wipe(ctx),
        scene_heavy_session_shimmer(ctx),
    ]
}
```

The synthetic `scene_strip_smoke()` stays argument-free because it does not use `RenderContext`.

- [ ] **Step 4: Run deterministic strip test**

Run:

```bash
cargo test --test dev_preview --features dev-preview dev_preview_animation_strip_text_and_cells_are_repeatable
```

Expected: PASS.

- [ ] **Step 5: Run animation scenario smoke test**

Run:

```bash
cargo test --test dev_preview --features dev-preview dev_preview_animation_writes_scene_strip_manifest_and_frames
cargo test --test dev_preview --features dev-preview dev_preview_all_includes_scene_strips
```

Expected: PASS.

- [ ] **Step 6: Commit deterministic strip fix**

```bash
git add src/dev_preview/scenarios.rs src/dev_preview/strips.rs tests/dev_preview.rs
git commit -m "fix: make Preview Lab scene strips deterministic"
```

## Task 7: Update Preview Lab Docs

**Files:**
- Modify: `AGENTS.md`
- Modify: `docs/superpowers/specs/2026-05-12-glorp-preview-lab-animation-strips-design.md`

- [ ] **Step 1: Update the living Preview Lab instructions**

In `AGENTS.md`, replace the Preview Lab artifact paragraph with:

```markdown
The bundle includes `index.html`, `review.md`, `manifest.json`, local assets,
`frames/*.txt` / `frames/*.cells.json` captures, optional
`frames/*.layout.json`, optional room crops, and typed contract artifacts such
as `frames/*.scene.json`, `frames/*.round-layout.json`, and
`frames/*.round-commands.json`. For animation scenarios the bundle includes
`strips/<id>/frame-NNN.txt` / `strips/<id>/frame-NNN.cells.json` captures.
Treat `manifest.json` as the review contract; it lists scenario intent,
dimensions, files, inputs, typed artifacts, and review prompts.
`manifest.json` uses `schema_version` 3 and includes a `strips` array whose
entries have `kind: "scene-moment"` along with `playback`, `target_id`, and
per-frame `phase` / `elapsed_ms` values.
```

- [ ] **Step 2: Add a historical supersession note**

In `docs/superpowers/specs/2026-05-12-glorp-preview-lab-animation-strips-design.md`, add this note immediately after the first sentence that says Slice 2 uses schema version `2`:

```markdown
Historical note: the current Preview Lab manifest is schema version `3`; this
slice documents the older schema `2` transition and should not be used as the
active schema reference.
```

- [ ] **Step 3: Verify doc references**

Run:

```bash
rg -n 'manifest.*schema.*2|schema version `2`|schema_version`? 2' AGENTS.md docs/superpowers/specs/2026-05-12-glorp-preview-lab-animation-strips-design.md
```

Expected: any remaining schema `2` hits in the historical spec are accompanied by the supersession note; `AGENTS.md` says schema `3`.

- [ ] **Step 4: Commit docs**

```bash
git add AGENTS.md docs/superpowers/specs/2026-05-12-glorp-preview-lab-animation-strips-design.md
git commit -m "docs: update Preview Lab artifact contract"
```

## Task 8: Final Verification

**Files:**
- No edits.

- [ ] **Step 1: Run focused Preview Lab tests**

```bash
cargo test --test dev_preview --features dev-preview
```

Expected: PASS.

- [ ] **Step 2: Run round scene tests**

```bash
cargo test --test round_scene
```

Expected: PASS.

- [ ] **Step 3: Run pure round draw tests**

```bash
cargo test --lib round::draw
```

Expected: PASS.

- [ ] **Step 4: Run full dev-preview bundle generation**

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Expected: exits 0 and prints the generated preview path. Confirm these files exist:

```bash
test -f target/glorp-preview/frames/watch-wide-normal.scene.json
test -f target/glorp-preview/frames/round-normal.scene.json
test -f target/glorp-preview/frames/round-normal.round-layout.json
test -f target/glorp-preview/frames/round-normal.round-commands.json
test -f target/glorp-preview/strips/scene-feed-sweep/frame-000.cells.json
```

- [ ] **Step 5: Run formatting and lint checks**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Inspect git status**

```bash
git status --short --branch
```

Expected: current branch with no unstaged or staged changes.

## Stop Conditions

- Stop if any existing preview text, cells, layout, room, masked-room, or strip path would need to be renamed or removed.
- Stop if watch or round rendered text/cell output changes outside the deterministic strip clock repair.
- Stop if `PreviewSceneArtifact` needs raw source names, exact token counts, feed row text, diagnostics, file paths, project names, prompts, responses, or transcript-like strings to satisfy a test.
- Stop if moving `RoundDrawCommand` out of `src/companion/render.rs` requires AppKit or macOS-only dependencies in `src/round/draw.rs`.
- Stop if this plan starts requiring a new `PresentationScene` module. That belongs to Plan 3 after this contract freeze and the PetPanel mechanical split land.

## Reviewer Checklist

- `manifest.json` remains schema `3`.
- Existing artifact paths are still present.
- New typed artifacts are additive and linked from `manifest.json`, `review.md`, and `index.html`.
- Every watch and round scenario derived from `WatchViewModel` has `files.scene`.
- Every round scenario has `files.round_layout` and `files.round_commands`.
- Scene artifacts are sanitized contract DTOs, not raw `WatchViewModel` dumps.
- Round layout artifacts expose aperture, safe radius, detail level, pet/prop/halo anchors, and motion budget.
- Round command artifacts expose command-kind counts, pet glyph text/span count, room glyph vocabulary, trouble/halo command presence, and privacy projection.
- Real scene strips use the deterministic preview clock.
- `AGENTS.md` describes schema `3`.
