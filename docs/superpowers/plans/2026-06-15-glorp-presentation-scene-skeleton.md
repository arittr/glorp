# Glorp Presentation Scene Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the backend-neutral `src/presentation/` scene skeleton with owned target IDs and explicit privacy projections.

**Architecture:** Keep `WatchViewModel` as the input and derive a small `PresentationScene` without moving rendering behavior. The first scene skeleton mirrors the sanitized Preview Lab contract from Plan 1 and gives later plans a stable domain module for pet, room, prop, and adapter work.

**Tech Stack:** Rust 2021, serde, existing `WatchViewModel`, existing `RoomLifeProfile`, Preview Lab contract tests.

---

## Dependency Gate

- Plan 1 and Plan 2 must be committed.
- Run:

```bash
test -f src/tui/panels/pet/ambient.rs
test -f target/glorp-preview/frames/watch-wide-normal.scene.json
cargo test --test pet_panel_structure
cargo test --test dev_preview --features dev-preview dev_preview_watch_and_round_frames_write_scene_artifacts
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/presentation/mod.rs` | Create | Public module boundary for presentation-domain data. |
| `src/presentation/privacy.rs` | Create | Surface privacy projection enum and policy. |
| `src/presentation/target.rs` | Create | Owned neutral target IDs, separate from watch `TargetPath`. |
| `src/presentation/scene.rs` | Create | `PresentationScene` and derivation from `WatchViewModel + now`. |
| `src/lib.rs` | Modify | Export `presentation`. |
| `tests/presentation_scene.rs` | Create | Privacy, target ID, and scene skeleton tests. |

## Forbidden Changes

- Do not migrate renderers in this plan.
- Do not change `RoundSceneModel`.
- Do not change Preview Lab artifact paths.
- Do not depend on `crate::tui::component::TargetPath` from `src/presentation`.
- Do not include raw source names, feed rows, exact counts, diagnostics, file paths, project names, prompts, responses, or transcript-like strings in glanceable projections.

## Task 1: Add Failing Presentation Scene Tests

**Files:**
- Create: `tests/presentation_scene.rs`

- [ ] **Step 1: Write privacy and neutral-target tests**

Create `tests/presentation_scene.rs`:

```rust
use glorp::presentation::privacy::{PresentationSurface, PrivacyProjection};
use glorp::presentation::scene::PresentationScene;
use glorp::presentation::target::SurfaceTargetId;
use glorp::tui::view_model::{EventView, SourceStatus, WatchViewModel};
use time::macros::datetime;

#[test]
fn presentation_scene_glanceable_projection_excludes_private_runtime_text() {
    let mut vm = WatchViewModel::fixture_with_events();
    vm.helper_status = "helper failed in /Users/drew/private/project".into();
    vm.errors = vec!["prompt response tool payload /tmp/raw.log".into()];
    vm.recent_events = vec![EventView {
        timestamp: "11:22".into(),
        kind: glorp::tui::style::LogKind::Usage,
        text: "opened /Users/drew/private/project/src/main.rs".into(),
    }];
    vm.source_breakdown[0].display_name = "client-secret-project".into();
    vm.source_health[0].status = SourceStatus::Diagnostic;
    vm.source_health[0].diagnostic_message = Some("secret helper path".into());
    vm.today_effective_tokens = 123_456.0;

    let scene = PresentationScene::from_watch_view_model(
        &vm,
        datetime!(2026-06-15 12:00 UTC),
        PresentationSurface::RoundCompanion,
    );
    let debug = format!("{scene:?}").to_ascii_lowercase();

    for forbidden in [
        "/users/drew",
        "/tmp/",
        "prompt",
        "response",
        "tool payload",
        "client-secret-project",
        "123456",
        "secret helper path",
    ] {
        assert!(!debug.contains(forbidden), "scene leaked {forbidden}: {debug}");
    }
}

#[test]
fn presentation_targets_are_owned_ids_not_watch_paths() {
    let pet = SurfaceTargetId::new("pet.art");
    let room = SurfaceTargetId::new("room.effect");

    assert_eq!(pet.as_str(), "pet.art");
    assert_eq!(room.as_str(), "room.effect");
    assert!(
        !pet.as_str().starts_with("watch."),
        "presentation IDs must not encode watch target paths"
    );
}

#[test]
fn privacy_projection_is_surface_specific() {
    let watch = PrivacyProjection::for_surface(PresentationSurface::WatchTui);
    let round = PrivacyProjection::for_surface(PresentationSurface::RoundCompanion);
    let menubar = PrivacyProjection::for_surface(PresentationSurface::MenubarPopover);

    assert!(watch.source_names_visible);
    assert!(watch.exact_counts_visible);
    assert!(!round.source_names_visible);
    assert!(!round.exact_counts_visible);
    assert!(menubar.source_names_visible);
    assert!(menubar.exact_counts_visible);
}
```

- [ ] **Step 2: Run tests and confirm missing module failure**

Run:

```bash
cargo test --test presentation_scene
```

Expected: FAIL because `glorp::presentation` does not exist.

- [ ] **Step 3: Commit failing tests**

```bash
git add tests/presentation_scene.rs
git commit -m "test: require presentation scene skeleton"
```

## Task 2: Add Module Boundary, Privacy, and Target IDs

**Files:**
- Modify: `src/lib.rs`
- Create: `src/presentation/mod.rs`
- Create: `src/presentation/privacy.rs`
- Create: `src/presentation/target.rs`

- [ ] **Step 1: Export presentation module**

Add to `src/lib.rs`:

```rust
pub mod presentation;
```

Create `src/presentation/mod.rs`:

```rust
pub mod privacy;
pub mod scene;
pub mod target;
```

- [ ] **Step 2: Add privacy projections**

Create `src/presentation/privacy.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationSurface {
    WatchTui,
    RoundCompanion,
    RoundPreviewLab,
    MenubarPopover,
    PreviewLabArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyProjection {
    pub surface: PresentationSurface,
    pub source_names_visible: bool,
    pub exact_counts_visible: bool,
    pub diagnostic_text_visible: bool,
    pub feed_rows_visible: bool,
    pub file_paths_visible: bool,
    pub project_names_visible: bool,
}

impl PrivacyProjection {
    pub fn for_surface(surface: PresentationSurface) -> Self {
        match surface {
            PresentationSurface::WatchTui => Self {
                surface,
                source_names_visible: true,
                exact_counts_visible: true,
                diagnostic_text_visible: true,
                feed_rows_visible: true,
                file_paths_visible: false,
                project_names_visible: false,
            },
            PresentationSurface::MenubarPopover => Self {
                surface,
                source_names_visible: true,
                exact_counts_visible: true,
                diagnostic_text_visible: false,
                feed_rows_visible: false,
                file_paths_visible: false,
                project_names_visible: false,
            },
            PresentationSurface::RoundCompanion
            | PresentationSurface::RoundPreviewLab
            | PresentationSurface::PreviewLabArtifact => Self::sanitized(surface),
        }
    }

    fn sanitized(surface: PresentationSurface) -> Self {
        Self {
            surface,
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

- [ ] **Step 3: Add owned target IDs**

Create `src/presentation/target.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceTargetId(String);

impl SurfaceTargetId {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        assert!(
            !id.starts_with("watch."),
            "presentation target ids must not be watch TargetPath values"
        );
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test --test presentation_scene privacy_projection_is_surface_specific
cargo test --test presentation_scene presentation_targets_are_owned_ids_not_watch_paths
```

Expected: PASS for the two tests above; the scene derivation test still fails because `scene.rs` is not implemented.

- [ ] **Step 5: Commit module boundary**

```bash
git add src/lib.rs src/presentation/mod.rs src/presentation/privacy.rs src/presentation/target.rs tests/presentation_scene.rs
git commit -m "feat: add presentation privacy and target ids"
```

## Task 3: Add Presentation Scene Skeleton

**Files:**
- Create: `src/presentation/scene.rs`
- Modify: `src/presentation/mod.rs`

- [ ] **Step 1: Implement the scene skeleton**

Create `src/presentation/scene.rs`:

```rust
use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};
use crate::presentation::target::SurfaceTargetId;
use crate::tui::room::{biome_symbols, derive_room_life_profile, RoomSpeciesDialect};
use crate::tui::view_model::{SourceStatus, WatchViewModel};
use std::collections::BTreeMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationScene {
    pub privacy: PrivacyProjection,
    pub pet: PresentationPetSnapshot,
    pub room: PresentationRoomSnapshot,
    pub activity: PresentationActivitySnapshot,
    pub targets: BTreeMap<SurfaceTargetId, PresentationTargetAnchor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationPetSnapshot {
    pub seed: String,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub art_lines: Vec<String>,
    pub span_count: usize,
    pub facing: i8,
    pub breath_offset_y: u8,
    pub asleep: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationRoomSnapshot {
    pub primary_biome: String,
    pub secondary_biome: Option<String>,
    pub species_dialect: String,
    pub work_weather: String,
    pub day_phase: String,
    pub prop_landmarks: Vec<String>,
    pub glyph_vocabulary: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationActivitySnapshot {
    pub source_diversity: String,
    pub helper_health: PresentationHelperHealth,
    pub recent_activity: bool,
    pub fed_bucket: PresentationVitalBucket,
    pub happiness_bucket: PresentationVitalBucket,
    pub energy_bucket: PresentationVitalBucket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationHelperHealth {
    Ok,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationVitalBucket {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationTargetAnchor {
    pub layer: PresentationLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationLayer {
    Room,
    Prop,
    Pet,
    Halo,
    Overlay,
}
```

- [ ] **Step 2: Implement derivation from `WatchViewModel`**

Append to `src/presentation/scene.rs`:

```rust
impl PresentationScene {
    pub fn from_watch_view_model(
        vm: &WatchViewModel,
        now: OffsetDateTime,
        surface: PresentationSurface,
    ) -> Self {
        let privacy = PrivacyProjection::for_surface(surface);
        let room_profile = derive_room_life_profile(vm, now);
        let dialect = RoomSpeciesDialect::for_species(vm.pet_render.generated_species);
        let glyph_vocabulary = biome_symbols(room_profile.biome.primary, dialect)
            .into_iter()
            .map(|ch| ch.to_string())
            .collect();

        Self {
            privacy,
            pet: PresentationPetSnapshot {
                seed: vm.pet_render.seed.clone(),
                species: vm.pet_render.generated_species.as_str().to_string(),
                stage: format!("{:?}", vm.pet_render.stage).to_lowercase(),
                mood: format!("{:?}", vm.pet_render.mood).to_lowercase(),
                art_lines: vm.pet_art.clone(),
                span_count: vm.pet_spans.len(),
                facing: vm.facing,
                breath_offset_y: vm.breath_offset_y,
                asleep: vm.day_context.asleep,
            },
            room: PresentationRoomSnapshot {
                primary_biome: format!("{:?}", room_profile.biome.primary),
                secondary_biome: room_profile.biome.secondary.map(|tag| format!("{tag:?}")),
                species_dialect: room_profile.species_dialect.key.as_str().to_string(),
                work_weather: format!("{:?}", vm.life_profile.work_weather),
                day_phase: format!("{:?}", vm.day_context.day_phase),
                prop_landmarks: room_profile
                    .identity_prop_ids
                    .iter()
                    .map(|id| id.as_str().to_string())
                    .collect(),
                glyph_vocabulary,
            },
            activity: PresentationActivitySnapshot {
                source_diversity: format!("{:?}", vm.activity_identity.source_diversity),
                helper_health: if vm
                    .source_health
                    .iter()
                    .any(|health| health.status == SourceStatus::Diagnostic)
                {
                    PresentationHelperHealth::Trouble
                } else {
                    PresentationHelperHealth::Ok
                },
                recent_activity: vm.last_feed_pulse_at.is_some(),
                fed_bucket: vital_bucket(vm.fed),
                happiness_bucket: vital_bucket(vm.happiness),
                energy_bucket: vital_bucket(vm.energy),
            },
            targets: BTreeMap::from([
                (
                    SurfaceTargetId::new("room.effect"),
                    PresentationTargetAnchor {
                        layer: PresentationLayer::Room,
                    },
                ),
                (
                    SurfaceTargetId::new("pet.art"),
                    PresentationTargetAnchor {
                        layer: PresentationLayer::Pet,
                    },
                ),
            ]),
        }
    }
}

fn vital_bucket(value: f64) -> PresentationVitalBucket {
    if value < 34.0 {
        PresentationVitalBucket::Low
    } else if value < 67.0 {
        PresentationVitalBucket::Medium
    } else {
        PresentationVitalBucket::High
    }
}
```

- [ ] **Step 3: Run scene tests**

Run:

```bash
cargo test --test presentation_scene
```

Expected: PASS.

- [ ] **Step 4: Commit scene skeleton**

```bash
git add src/presentation/scene.rs src/presentation/mod.rs tests/presentation_scene.rs
git commit -m "feat: add presentation scene skeleton"
```

## Task 4: Final Verification

**Files:**
- No edits.

- [ ] **Step 1: Run focused checks**

```bash
cargo test --test presentation_scene
cargo test --test dev_preview --features dev-preview
cargo test --test round_scene
```

Expected: PASS.

- [ ] **Step 2: Run repository checks**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features dev-preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
git status --short --branch
```

Expected: all commands pass and git status is clean after the final commit.

## Stop Conditions

- Stop if `src/presentation` needs to import `crate::tui::component::TargetPath`.
- Stop if sanitized scene data needs raw private text to satisfy tests.
- Stop if renderer code must change to make the skeleton compile.

