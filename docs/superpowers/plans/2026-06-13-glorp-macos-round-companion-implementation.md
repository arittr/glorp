# Glorp macOS Round Companion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the V1 macOS round companion: a Dock-visible `Glorp.app` porthole window launched by `glorp companion`, proven first in Preview Lab, and fed by the same local watch state and polling path.

**Spec:** `docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md`.

**Architecture:** Add a pure `round` scene/model/layout layer consumed by both Preview Lab and the native companion. Keep watch as the ingestion owner, extract shared live presentation stamping for menubar and companion, launch the real companion through a macOS `.app` bundle, and keep the visible round surface free of exact counts, dashboard rows, raw source names, paths, prompts, responses, and diagnostics text.

**Tech Stack:** Rust 2021, Ratatui preview cells, serde/serde_json, objc2 AppKit/Foundation, Node package scripts, assert_cmd integration tests, npm smoke tests, Preview Lab.

**Branch guidance:** Work on the current branch unless Drew explicitly creates a branch. Commit after each task that reaches green verification. Do not stage unrelated dirty files, including existing species dialect preview work.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `src/round/mod.rs` | Create | Public module boundary for pure round model, layout, and preview rendering. |
| `src/round/model.rs` | Create | `RoundSceneModel`, privacy-safe derivation from stamped `WatchViewModel`, and scene moments. |
| `src/round/layout.rs` | Create | Aperture geometry, safe radius, anchors, degradation policy, and motion budget. |
| `src/round/preview.rs` | Create | Deterministic Ratatui-cell preview renderer for round scenarios. |
| `src/lib.rs` | Modify | Export `round` and macOS companion modules. |
| `src/dev_preview/round.rs` | Create | Round Preview Lab fixtures for normal, active, asleep, trouble, flat, Glitch, and Crystal states. |
| `src/dev_preview/mod.rs` | Modify | Expose the round preview module. |
| `src/dev_preview/export.rs` | Modify | Bump manifest schema to 3 and add first-class `round` metadata. |
| `src/dev_preview/scenarios.rs` | Modify | Add `PreviewSelection::Round`, generate round frames, and write round artifacts. |
| `src/cli.rs` | Modify | Add public `companion`, hidden `companion-app`, and hidden `dev-preview --scenario round`. |
| `src/commands/mod.rs` | Modify | Expose companion commands. |
| `src/commands/dev_preview.rs` | Modify | Route round preview selection. |
| `src/commands/companion.rs` | Create | User-facing launcher that records helper paths, opens `Glorp.app`, and exits. |
| `src/commands/companion_app.rs` | Create | Hidden command run from the `.app` bundle to start the native facade. |
| `src/watch_live.rs` | Create | Shared poll worker and presentation-stamping utilities for AppKit facades. |
| `src/menubar/app.rs` | Modify | Replace menubar-specific worker/stamping code with `watch_live` helpers. |
| `src/usage/helper_locator.rs` | Create | Persist and read helper/node paths for no-env Finder/Dock launches. |
| `src/usage/mod.rs` | Modify | Expose helper locator. |
| `src/usage/ccusage.rs` | Modify | Let `HelperDiscovery` fall back to helper-locator config after env/PATH discovery. |
| `src/companion/mod.rs` | Create | macOS companion facade module gate. |
| `src/companion/app.rs` | Create | `NSApplication`, window lifecycle, poll timer, and app reopen behavior. |
| `src/companion/render.rs` | Create | AppKit drawing bridge from `RoundSceneModel` and `RoundSceneLayout`. |
| `Cargo.toml` | Modify | Enable AppKit/Foundation features required by the companion window. |
| `scripts/build-macos-companion-app.mjs` | Create | Build a regular Dock-visible `Glorp.app` bundle that runs hidden `companion-app`. |
| `scripts/build-platform-package.mjs` | Modify | Include `app/Glorp.app` in darwin platform packages. |
| `npm/platform/darwin-arm64/package.json` | Modify | Include `app/Glorp.app` in package files. |
| `npm/platform/darwin-x64/package.json` | Modify | Include `app/Glorp.app` in package files. |
| `npm/glorp/bin/glorp.js` | Modify | Set `GLORP_COMPANION_APP` for darwin packages and keep helper env injection. |
| `npm/glorp/test/smoke.mjs` | Modify | Prove helper env and companion app env are passed to native commands. |
| `tests/dev_preview.rs` | Modify | Round scenario artifacts, schema, metadata, privacy, and visual invariant tests. |
| `tests/cli_smoke.rs` | Modify | `companion` help/unsupported-platform behavior and hidden `companion-app` visibility. |
| `tests/round_scene.rs` | Create | Integration-level round model privacy and derivation tests. |
| `tests/helper_locator.rs` | Create | Persisted helper locator read/write and discovery fallback tests. |

## Implementation Decisions

- V1 ships `Glorp.app` inside the darwin platform npm packages at `npm/platform/darwin-*/app/Glorp.app`.
- `glorp companion` is the public launcher. It records the current npm-wrapper helper paths into Glorp config, opens the app bundle with LaunchServices, and exits.
- The `.app` executable runs hidden `glorp companion-app`. That command owns the AppKit run loop and is omitted from help.
- Finder/Dock launches do not rely on inherited npm wrapper environment. They read the persisted helper locator. Missing or stale helper paths become the existing helper-trouble/degraded visual signal.
- V1 uses `NSApplicationActivationPolicy::Regular`. It does not use `LSUIElement` or the menubar accessory activation policy.
- V1 default window level is normal. Floating behavior is not part of this plan.
- Window placement persists through `NSWindow` frame autosave or Foundation user defaults under the companion bundle identity.

## Task 1: Pure Round Scene Model

**Files:**
- Create: `src/round/mod.rs`
- Create: `src/round/model.rs`
- Modify: `src/lib.rs`
- Test: `tests/round_scene.rs`

- [ ] **Step 1: Write failing round scene privacy and derivation tests**

Create `tests/round_scene.rs`:

```rust
use glorp::round::model::{
    derive_round_scene_model, RoundActivityPulse, RoundHelperHealth, RoundSourceDiversity,
};
use glorp::tui::identity::SourceDiversity;
use glorp::tui::view_model::{EventView, SourceHealthView, SourceStatus, WatchViewModel};
use time::macros::datetime;

#[test]
fn round_scene_excludes_watch_dashboard_and_private_fields() {
    let mut vm = WatchViewModel::fixture_with_events();
    vm.helper_status = "provider poll failed: /Users/drew/private/project".into();
    vm.errors = vec!["secret prompt response tool payload /tmp/private.rs".into()];
    vm.recent_events = vec![EventView {
        timestamp: "13:40".into(),
        kind: glorp::tui::style::LogKind::Usage,
        text: "opened /Users/drew/private/project/main.rs".into(),
    }];
    vm.source_breakdown[0].display_name = "client-secret-project".into();
    vm.today_effective_tokens = 123_456.0;
    vm.progress.rate_per_hour = 99_999.0;

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
    let debug = format!("{scene:?}");

    assert!(!debug.contains("secret"));
    assert!(!debug.contains("/Users/drew"));
    assert!(!debug.contains("prompt"));
    assert!(!debug.contains("response"));
    assert!(!debug.contains("tool payload"));
    assert!(!debug.contains("123456"));
    assert!(!debug.contains("99999"));
}

#[test]
fn round_scene_maps_required_v1_signals() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.activity_identity.source_diversity = SourceDiversity::DualLane;
    vm.last_feed_pulse_at = Some(datetime!(2026-06-13 17:59:59 UTC));
    vm.source_health.push(SourceHealthView {
        name: "codex".into(),
        display_name: "codex".into(),
        status: SourceStatus::Diagnostic,
        today_effective_tokens: 0.0,
        bucket_effective_tokens: 0.0,
        diagnostic_code: Some("missing_helper".into()),
        diagnostic_message: Some("private helper path".into()),
    });

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));

    assert_eq!(scene.pet.seed, "fixture-seed");
    assert_eq!(scene.pet.species, vm.pet_render.generated_species);
    assert_eq!(scene.pet.stage, vm.pet_render.stage);
    assert_eq!(scene.room.prop_landmarks.len(), 2);
    assert_eq!(scene.halo.source_diversity, RoundSourceDiversity::Dual);
    assert_eq!(scene.halo.helper_health, RoundHelperHealth::Trouble);
    assert!(matches!(scene.halo.activity_pulse, RoundActivityPulse::Recent { .. }));
}

#[test]
fn round_scene_uses_night_calm_for_asleep_state() {
    let mut vm = WatchViewModel::fixture();
    vm.day_context.asleep = true;
    vm.life_profile.calm_mode = true;

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 08:00 UTC));

    assert!(scene.lifecycle.asleep);
    assert!(scene.lifecycle.calm);
    assert!(scene.halo.activity_pulse.is_quiet());
}
```

- [ ] **Step 2: Run the tests and confirm they fail on missing module**

Run:

```bash
cargo test --test round_scene
```

Expected: compile failure because `glorp::round` does not exist.

- [ ] **Step 3: Add the module boundary**

Modify `src/lib.rs`:

```rust
pub mod round;
```

Create `src/round/mod.rs`:

```rust
pub mod model;
```

- [ ] **Step 4: Implement the privacy-safe model types**

Create `src/round/model.rs`:

```rust
use crate::game::evolution::Stage;
use crate::pet::generation::Species;
use crate::storage::state::HabitatPropId;
use crate::tui::identity::SourceDiversity;
use crate::tui::life::WorkWeather;
use crate::tui::room::{derive_room_life_profile, RoomBiome, RoomDialectKey};
use crate::tui::view_model::{SourceStatus, WatchViewModel};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, PartialEq)]
pub struct RoundSceneModel {
    pub pet: RoundPetModel,
    pub room: RoundRoomModel,
    pub halo: RoundHaloModel,
    pub lifecycle: RoundLifecycleModel,
    pub moments: Vec<RoundSceneMoment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundPetModel {
    pub seed: String,
    pub species: Species,
    pub stage: Stage,
    pub mood: crate::game::metabolism::Mood,
    pub asleep: bool,
    pub breath_offset_y: u8,
    pub facing: i8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundRoomModel {
    pub biome: RoomBiome,
    pub dialect: RoomDialectKey,
    pub work_weather: WorkWeather,
    pub day_phase: crate::tui::day::DayPhase,
    pub prop_landmarks: Vec<HabitatPropId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundHaloModel {
    pub activity_pulse: RoundActivityPulse,
    pub source_diversity: RoundSourceDiversity,
    pub helper_health: RoundHelperHealth,
    pub vitals: RoundVitals,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundVitals {
    pub fed: RoundVitalBucket,
    pub happiness: RoundVitalBucket,
    pub energy: RoundVitalBucket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundVitalBucket {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundSourceDiversity {
    Quiet,
    Single,
    Dual,
    Ensemble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundHelperHealth {
    Ok,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundActivityPulse {
    Quiet,
    Recent { age_ms: u16 },
}

impl RoundActivityPulse {
    pub fn is_quiet(self) -> bool {
        matches!(self, RoundActivityPulse::Quiet)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundLifecycleModel {
    pub asleep: bool,
    pub calm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundSceneMoment {
    pub kind: RoundSceneMomentKind,
    pub trigger_id: String,
    pub anchor: RoundMomentAnchor,
    pub duration_ms: u16,
    pub replay_policy: RoundReplayPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundSceneMomentKind {
    FeedSweep,
    PropResonance,
    DawnWake,
    DreamGlimmer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundMomentAnchor {
    Halo,
    Pet,
    Room,
    Prop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundReplayPolicy {
    OncePerTrigger,
}

pub fn derive_round_scene_model(vm: &WatchViewModel, now: OffsetDateTime) -> RoundSceneModel {
    let room_profile = derive_room_life_profile(vm, now);
    let pulse = derive_activity_pulse(vm, now);
    let moments = derive_moments(vm, &pulse);
    RoundSceneModel {
        pet: RoundPetModel {
            seed: vm.pet_render.seed.clone(),
            species: vm.pet_render.generated_species,
            stage: vm.pet_render.stage,
            mood: vm.pet_render.mood,
            asleep: vm.day_context.asleep,
            breath_offset_y: vm.breath_offset_y,
            facing: vm.facing,
        },
        room: RoundRoomModel {
            biome: room_profile.biome,
            dialect: room_profile.species_dialect.key,
            work_weather: vm.life_profile.work_weather,
            day_phase: vm.day_context.day_phase,
            prop_landmarks: room_profile.identity_prop_ids.into_iter().take(2).collect(),
        },
        halo: RoundHaloModel {
            activity_pulse: pulse,
            source_diversity: match vm.activity_identity.source_diversity {
                SourceDiversity::Quiet => RoundSourceDiversity::Quiet,
                SourceDiversity::SingleLane => RoundSourceDiversity::Single,
                SourceDiversity::DualLane => RoundSourceDiversity::Dual,
                SourceDiversity::Ensemble => RoundSourceDiversity::Ensemble,
            },
            helper_health: if vm.source_health.iter().any(|s| s.status == SourceStatus::Diagnostic)
            {
                RoundHelperHealth::Trouble
            } else {
                RoundHelperHealth::Ok
            },
            vitals: RoundVitals {
                fed: vital_bucket(vm.fed),
                happiness: vital_bucket(vm.happiness),
                energy: vital_bucket(vm.energy),
            },
        },
        lifecycle: RoundLifecycleModel {
            asleep: vm.day_context.asleep,
            calm: vm.life_profile.calm_mode || vm.day_context.asleep,
        },
        moments,
    }
}

fn derive_activity_pulse(vm: &WatchViewModel, now: OffsetDateTime) -> RoundActivityPulse {
    if vm.day_context.asleep {
        return RoundActivityPulse::Quiet;
    }
    let Some(last) = vm.last_feed_pulse_at else {
        return RoundActivityPulse::Quiet;
    };
    let age = now - last;
    if age < Duration::ZERO || age > Duration::seconds(2) {
        return RoundActivityPulse::Quiet;
    }
    RoundActivityPulse::Recent {
        age_ms: age.whole_milliseconds().clamp(0, u16::MAX as i128) as u16,
    }
}

fn derive_moments(vm: &WatchViewModel, pulse: &RoundActivityPulse) -> Vec<RoundSceneMoment> {
    let mut moments = Vec::new();
    if matches!(pulse, RoundActivityPulse::Recent { .. }) {
        moments.push(RoundSceneMoment {
            kind: RoundSceneMomentKind::FeedSweep,
            trigger_id: "feed-pulse".to_string(),
            anchor: RoundMomentAnchor::Halo,
            duration_ms: 1200,
            replay_policy: RoundReplayPolicy::OncePerTrigger,
        });
    }
    if vm.day_context.asleep {
        moments.push(RoundSceneMoment {
            kind: RoundSceneMomentKind::DreamGlimmer,
            trigger_id: "asleep".to_string(),
            anchor: RoundMomentAnchor::Room,
            duration_ms: 1600,
            replay_policy: RoundReplayPolicy::OncePerTrigger,
        });
    }
    moments
}

fn vital_bucket(value: f64) -> RoundVitalBucket {
    if value < 0.34 {
        RoundVitalBucket::Low
    } else if value < 0.67 {
        RoundVitalBucket::Medium
    } else {
        RoundVitalBucket::High
    }
}
```

- [ ] **Step 5: Add a fixture with habitat props if it is not already public**

If `WatchViewModel::fixture_with_habitat_props()` is not public in `src/tui/view_model.rs`, expose it with the same style as `fixture()` and do not change existing fixture values.

- [ ] **Step 6: Run the focused tests**

Run:

```bash
cargo test --test round_scene
cargo test round::model
```

Expected: all round model tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs src/round/mod.rs src/round/model.rs tests/round_scene.rs src/tui/view_model.rs
git commit -m "feat: add round scene model"
```

## Task 2: Round Layout And Motion Budget

**Files:**
- Create: `src/round/layout.rs`
- Modify: `src/round/model.rs`
- Test: module tests in `src/round/layout.rs`

- [ ] **Step 1: Write failing layout tests**

Create `src/round/layout.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::model::{derive_round_scene_model, RoundHelperHealth};
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    #[test]
    fn layout_keeps_pet_inside_safe_inner_circle() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(52, 52),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert!(layout.safe_inner_radius > 18.0);
        assert!(layout.aperture.contains(layout.pet_anchor.x, layout.pet_anchor.y));
        assert!(layout.pet_anchor.radius <= layout.safe_inner_radius);
    }

    #[test]
    fn layout_drops_optional_halo_before_pet_legibility() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(28, 28),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert_eq!(layout.detail_level, RoundDetailLevel::Compact);
        assert!(layout.halo_anchors.len() <= 2);
        assert!(layout.pet_anchor.radius >= 7.0);
    }

    #[test]
    fn helper_trouble_keeps_one_visible_rim_anchor_in_compact_layout() {
        let mut vm = WatchViewModel::fixture();
        vm.source_health[0].status = crate::tui::view_model::SourceStatus::Diagnostic;
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        assert_eq!(scene.halo.helper_health, RoundHelperHealth::Trouble);

        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(28, 28),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert!(layout
            .halo_anchors
            .iter()
            .any(|anchor| anchor.kind == RoundAnchorKind::HelperTrouble));
    }
}
```

- [ ] **Step 2: Run layout tests and confirm missing types**

Run:

```bash
cargo test round::layout
```

Expected: compile failure for missing layout types and functions.

- [ ] **Step 3: Implement layout types and deterministic geometry**

Fill `src/round/layout.rs`:

```rust
use crate::round::model::{RoundHelperHealth, RoundSceneModel, RoundSourceDiversity};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundAperture {
    pub width: u16,
    pub height: u16,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
}

impl RoundAperture {
    pub fn new(width: u16, height: u16) -> Self {
        let radius = (width.min(height) as f32 / 2.0) - 1.0;
        Self {
            width,
            height,
            center_x: (width as f32 - 1.0) / 2.0,
            center_y: (height as f32 - 1.0) / 2.0,
            radius,
        }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        let dx = x - self.center_x;
        let dy = y - self.center_y;
        (dx * dx + dy * dy).sqrt() <= self.radius
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundSceneLayout {
    pub aperture: RoundAperture,
    pub safe_inner_radius: f32,
    pub detail_level: RoundDetailLevel,
    pub pet_anchor: RoundAnchor,
    pub prop_anchors: Vec<RoundAnchor>,
    pub halo_anchors: Vec<RoundAnchor>,
    pub motion_budget: RoundMotionBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundDetailLevel {
    Full,
    Compact,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundAnchor {
    pub kind: RoundAnchorKind,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundAnchorKind {
    Pet,
    Prop,
    ActivityPulse,
    SourceDiversity,
    Vital,
    HelperTrouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundMotionBudget {
    pub pet_breath: bool,
    pub pet_blink: bool,
    pub activity_sweep: bool,
    pub prop_resonance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundRenderCapabilities {
    pub truecolor: bool,
    pub transparent_outside_aperture: bool,
}

impl RoundRenderCapabilities {
    pub fn preview_truecolor() -> Self {
        Self {
            truecolor: true,
            transparent_outside_aperture: true,
        }
    }
}

pub fn layout_round_scene(
    scene: &RoundSceneModel,
    aperture: RoundAperture,
    _capabilities: RoundRenderCapabilities,
) -> RoundSceneLayout {
    let detail_level = if aperture.radius >= 22.0 {
        RoundDetailLevel::Full
    } else if aperture.radius >= 13.0 {
        RoundDetailLevel::Compact
    } else {
        RoundDetailLevel::Minimal
    };
    let safe_inner_radius = (aperture.radius * 0.78).max(5.0);
    let pet_anchor = RoundAnchor {
        kind: RoundAnchorKind::Pet,
        x: aperture.center_x,
        y: aperture.center_y + aperture.radius * 0.10,
        radius: (safe_inner_radius * 0.58).max(5.0),
    };
    let prop_limit = match detail_level {
        RoundDetailLevel::Full => 2,
        RoundDetailLevel::Compact => 1,
        RoundDetailLevel::Minimal => 0,
    };
    let prop_anchors: Vec<_> = scene
        .room
        .prop_landmarks
        .iter()
        .take(prop_limit)
        .enumerate()
        .map(|(index, _)| RoundAnchor {
            kind: RoundAnchorKind::Prop,
            x: aperture.center_x + if index == 0 { -safe_inner_radius * 0.46 } else { safe_inner_radius * 0.46 },
            y: aperture.center_y + safe_inner_radius * 0.48,
            radius: safe_inner_radius * 0.16,
        })
        .collect();
    let has_prop_anchors = !prop_anchors.is_empty();
    let mut halo_anchors = Vec::new();
    if matches!(detail_level, RoundDetailLevel::Full | RoundDetailLevel::Compact) {
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::ActivityPulse,
            x: aperture.center_x,
            y: aperture.center_y - aperture.radius,
            radius: 1.0,
        });
    }
    if matches!(detail_level, RoundDetailLevel::Full)
        && scene.halo.source_diversity != RoundSourceDiversity::Quiet
    {
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::SourceDiversity,
            x: aperture.center_x + aperture.radius * 0.66,
            y: aperture.center_y - aperture.radius * 0.66,
            radius: 1.0,
        });
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::Vital,
            x: aperture.center_x,
            y: aperture.center_y + aperture.radius,
            radius: 1.0,
        });
    }
    if scene.halo.helper_health == RoundHelperHealth::Trouble {
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::HelperTrouble,
            x: aperture.center_x - aperture.radius * 0.66,
            y: aperture.center_y - aperture.radius * 0.66,
            radius: 1.0,
        });
    }
    RoundSceneLayout {
        aperture,
        safe_inner_radius,
        detail_level,
        pet_anchor,
        prop_anchors,
        halo_anchors,
        motion_budget: RoundMotionBudget {
            pet_breath: true,
            pet_blink: true,
            activity_sweep: !scene.halo.activity_pulse.is_quiet(),
            prop_resonance: has_prop_anchors,
        },
    }
}
```

- [ ] **Step 4: Expose the layout module**

Modify `src/round/mod.rs`:

```rust
pub mod layout;
pub mod model;
```

- [ ] **Step 5: Run layout tests**

Run:

```bash
cargo test round::layout
cargo test --test round_scene
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/round/layout.rs src/round/model.rs
git commit -m "feat: add round scene layout"
```

## Task 3: Round Preview Lab Contract

**Files:**
- Create: `src/round/preview.rs`
- Create: `src/dev_preview/round.rs`
- Modify: `src/dev_preview/mod.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/cli.rs`
- Modify: `src/commands/dev_preview.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Add failing Preview Lab tests**

Append to `tests/dev_preview.rs` near the other scenario contract tests:

```rust
const ROUND_IDS: [&str; 7] = [
    "round-normal",
    "round-active-pulse",
    "round-asleep-night",
    "round-helper-trouble",
    "round-flat-color",
    "round-glitch-dialect",
    "round-crystal-dialect",
];

#[test]
fn dev_preview_round_writes_manifest_cells_and_round_metadata() {
    let run = PreviewRun::new();

    run.run_success("round");

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 3);
    for id in ROUND_IDS {
        assert!(run.out.join(format!("frames/{id}.txt")).is_file(), "missing {id}.txt");
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id}.cells.json"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "round");
        assert_eq!(scenario["round"]["target_renderer"], "preview-cells");
        assert_eq!(scenario["round"]["aperture"]["shape"], "circle");
        assert!(scenario["round"]["aperture"]["safe_inner_radius"].as_f64().unwrap() > 0.0);
        assert_eq!(scenario["round"]["privacy"]["source_names_visible"], false);
        assert_eq!(scenario["round"]["privacy"]["exact_counts_visible"], false);
        assert_eq!(scenario["round"]["privacy"]["diagnostic_text_visible"], false);
    }
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
}

#[test]
fn dev_preview_round_glitch_and_crystal_differ_by_symbols_in_flat_mode() {
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
```

Add `outside_aperture` handling to `read_cells` assertions after the exporter changes. Keep existing helpers intact.

- [ ] **Step 2: Run tests and confirm expected failures**

Run:

```bash
cargo test --test dev_preview dev_preview_round_writes_manifest_cells_and_round_metadata
cargo test --test dev_preview dev_preview_round_output_has_no_dashboard_labels_or_private_source_text
cargo test --test dev_preview dev_preview_round_aperture_corners_are_masked
```

Expected: `invalid value 'round' for '--scenario <SCENARIO>'`.

- [ ] **Step 3: Add the hidden scenario selector**

Modify `src/cli.rs`:

```rust
pub enum PreviewScenarioArg {
    All,
    Watch,
    Pets,
    Props,
    Animation,
    Round,
}
```

Modify `src/commands/dev_preview.rs` to map `PreviewScenarioArg::Round`:

```rust
PreviewScenarioArg::Round => PreviewSelection::Round,
```

- [ ] **Step 4: Add preview cell support for aperture masking**

Modify `src/dev_preview/frame.rs` by adding an optional flag to `PreviewCell`:

```rust
#[serde(skip_serializing_if = "is_false")]
pub outside_aperture: bool,
```

Add this helper in the same file:

```rust
fn is_false(value: &bool) -> bool {
    !*value
}
```

Set `outside_aperture: false` in `frame_from_buffer` and existing tests.

- [ ] **Step 5: Bump manifest schema and add round metadata**

Modify `src/dev_preview/export.rs`:

```rust
pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct PreviewScenario {
    pub id: String,
    pub kind: PreviewScenarioKind,
    pub title: String,
    pub intent: String,
    pub dimensions: PreviewDimensions,
    pub files: PreviewScenarioFiles,
    pub inputs: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<PreviewRoundMetadata>,
    pub review_prompts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRoundMetadata {
    pub target_renderer: &'static str,
    pub aperture: PreviewRoundAperture,
    pub privacy: PreviewRoundPrivacy,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRoundAperture {
    pub shape: &'static str,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub safe_inner_radius: f32,
    pub transparent_outside_aperture: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRoundPrivacy {
    pub source_names_visible: bool,
    pub exact_counts_visible: bool,
    pub diagnostic_text_visible: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewScenarioKind {
    Watch,
    PetMatrix,
    HabitatProps,
    Round,
}
```

Update `scenario_kind_label` with `PreviewScenarioKind::Round => "round"`.
Update `sample_manifest()` in `src/dev_preview/export.rs` tests so non-round
scenarios set `round: None`.

- [ ] **Step 6: Implement the preview renderer with current pet art**

Create `src/round/preview.rs`:

```rust
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundHelperHealth};
use crate::dev_preview::frame::{PreviewCell, PreviewFrame};
use crate::tui::view_model::WatchViewModel;
use ratatui::text::Line;

pub fn render_round_preview_frame_from_vm(
    id: impl Into<String>,
    title: impl Into<String>,
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    width: u16,
    height: u16,
) -> PreviewFrame {
    let scene = derive_round_scene_model(vm, now);
    let aperture = RoundAperture::new(width, height);
    let layout = layout_round_scene(&scene, aperture, RoundRenderCapabilities::preview_truecolor());
    let mut cells = blank_cells(width, height, aperture);
    paint_room(&mut cells, width, &scene, &layout);
    paint_pet_art(&mut cells, width, vm, &layout);
    paint_halo(&mut cells, width, &scene, &layout);
    mark_continuations(&mut cells, width);
    PreviewFrame {
        id: id.into(),
        title: title.into(),
        width,
        height,
        cells,
        layout: None,
        extra_inputs: Default::default(),
    }
}

fn blank_cells(width: u16, height: u16, aperture: RoundAperture) -> Vec<PreviewCell> {
    let mut cells = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let outside = !aperture.contains(x as f32, y as f32);
            cells.push(PreviewCell {
                x,
                y,
                symbol: " ".to_string(),
                display_width: 1,
                continuation: false,
                fg: None,
                bg: None,
                modifiers: Vec::new(),
                outside_aperture: outside,
            });
        }
    }
    cells
}

fn paint_room(
    cells: &mut [PreviewCell],
    width: u16,
    scene: &crate::round::model::RoundSceneModel,
    layout: &crate::round::layout::RoundSceneLayout,
) {
    for y in 0..layout.aperture.height {
        for x in 0..layout.aperture.width {
            let idx = y as usize * width as usize + x as usize;
            if cells[idx].outside_aperture {
                continue;
            }
            if (x + y) % 5 == 0 {
                let (symbol, fg) = room_symbol(scene);
                set_cell(cells, width, x as i32, y as i32, symbol, Some(fg));
            }
        }
    }
}

fn paint_pet_art(
    cells: &mut [PreviewCell],
    width: u16,
    vm: &WatchViewModel,
    layout: &crate::round::layout::RoundSceneLayout,
) {
    let art_width = vm.pet_art.iter().map(|line| line.chars().count()).max().unwrap_or(0) as i32;
    let art_height = vm.pet_art.len() as i32;
    let start_x = layout.pet_anchor.x.round() as i32 - art_width / 2;
    let start_y = layout.pet_anchor.y.round() as i32 - art_height / 2;
    for (row, line) in vm.pet_art.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch != ' ' {
                set_cell(
                    cells,
                    width,
                    start_x + col as i32,
                    start_y + row as i32,
                    ch.to_string(),
                    Some("#efebe4".to_string()),
                );
            }
        }
    }
}

fn paint_halo(
    cells: &mut [PreviewCell],
    width: u16,
    scene: &crate::round::model::RoundSceneModel,
    layout: &crate::round::layout::RoundSceneLayout,
) {
    if scene.halo.helper_health == RoundHelperHealth::Trouble
    {
        for anchor in layout.halo_anchors.iter().filter(|a| {
            a.kind == crate::round::layout::RoundAnchorKind::HelperTrouble
        }) {
            set_cell(
                cells,
                width,
                anchor.x.round() as i32,
                anchor.y.round() as i32,
                "!".to_string(),
                Some("#f0a646".to_string()),
            );
        }
    }
}

fn room_symbol(scene: &crate::round::model::RoundSceneModel) -> (String, String) {
    match scene.room.dialect {
        crate::tui::room::RoomDialectKey::Glitch => ("#".to_string(), "#86d9ef".to_string()),
        crate::tui::room::RoomDialectKey::Crystal => ("^".to_string(), "#b39dff".to_string()),
        _ => (".".to_string(), "#808080".to_string()),
    }
}

fn set_cell(
    cells: &mut [PreviewCell],
    width: u16,
    x: i32,
    y: i32,
    symbol: String,
    fg: Option<String>,
) {
    if x < 0 || y < 0 {
        return;
    }
    let x = x as u16;
    let y = y as u16;
    let idx = y as usize * width as usize + x as usize;
    if idx >= cells.len() || cells[idx].outside_aperture {
        return;
    }
    cells[idx].display_width = Line::from(symbol.clone()).width();
    cells[idx].symbol = symbol;
    cells[idx].fg = fg;
}

fn mark_continuations(cells: &mut [PreviewCell], width: u16) {
    for index in 0..cells.len() {
        let display_width = cells[index].display_width;
        if display_width <= 1 {
            continue;
        }
        for offset in 1..display_width {
            let continuation_index = index + offset;
            if continuation_index >= cells.len() {
                break;
            }
            if cells[continuation_index].y != cells[index].y {
                break;
            }
            if cells[continuation_index].x >= width {
                break;
            }
            cells[continuation_index].continuation = true;
        }
    }
}
```

- [ ] **Step 7: Add round fixtures**

Create `src/dev_preview/round.rs`:

```rust
use crate::dev_preview::frame::PreviewFrame;
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::round::preview::render_round_preview_frame_from_vm;
use crate::tui::identity::SourceDiversity;
use crate::tui::view_model::{SourceStatus, WatchViewModel};
use time::Duration;

pub fn round_frames(ctx: &PreviewRenderContext) -> Vec<PreviewFrame> {
    let mut frames = Vec::new();

    let normal = WatchViewModel::fixture_with_habitat_props();
    frames.push(frame("round-normal", "Round Normal", &normal, ctx));

    let mut active = WatchViewModel::fixture_with_habitat_props();
    active.activity_identity.source_diversity = SourceDiversity::DualLane;
    active.last_feed_pulse_at = Some(ctx.fixed_now - Duration::milliseconds(400));
    frames.push(frame("round-active-pulse", "Round Active Pulse", &active, ctx));

    let mut asleep = WatchViewModel::fixture_with_habitat_props();
    asleep.day_context.asleep = true;
    asleep.life_profile.calm_mode = true;
    frames.push(frame("round-asleep-night", "Round Asleep Night", &asleep, ctx));

    let mut trouble = WatchViewModel::fixture_with_habitat_props();
    trouble.source_health[0].status = SourceStatus::Diagnostic;
    frames.push(frame("round-helper-trouble", "Round Helper Trouble", &trouble, ctx));

    let flat = WatchViewModel::fixture_with_habitat_props();
    frames.push(frame("round-flat-color", "Round Flat Color", &flat, ctx));

    let mut glitch = WatchViewModel::fixture_with_habitat_props();
    glitch.pet_render.generated_species = crate::pet::generation::Species::Glitch;
    frames.push(frame("round-glitch-dialect", "Round Glitch Dialect", &glitch, ctx));

    let mut crystal = WatchViewModel::fixture_with_habitat_props();
    crystal.pet_render.generated_species = crate::pet::generation::Species::Crystal;
    frames.push(frame("round-crystal-dialect", "Round Crystal Dialect", &crystal, ctx));

    frames
}

fn frame(id: &str, title: &str, vm: &WatchViewModel, ctx: &PreviewRenderContext) -> PreviewFrame {
    render_round_preview_frame_from_vm(id, title, vm, ctx.fixed_now, 52, 52)
}
```

- [ ] **Step 8: Wire round scenario generation**

Modify `src/dev_preview/mod.rs`:

```rust
pub mod round;
```

Modify `src/round/mod.rs`:

```rust
pub mod layout;
pub mod model;
#[cfg(feature = "dev-preview")]
pub mod preview;
```

Modify `src/dev_preview/scenarios.rs`:

```rust
pub enum PreviewSelection {
    All,
    Watch,
    Pets,
    Props,
    Animation,
    Round,
}
```

In `generate_preview_bundle`:

```rust
PreviewSelection::All => {
    frames.extend(watch_frames(&ctx, &scratch_dir)?);
    frames.extend(habitat_prop_frames(&ctx, &scratch_dir)?);
    frames.extend(pet_frames(&ctx)?);
    frames.extend(crate::dev_preview::round::round_frames(&ctx));
    strips.push(crate::dev_preview::strips::scene_strip_smoke());
    strips.extend(crate::dev_preview::strips::scene_strips());
}
PreviewSelection::Round => frames.extend(crate::dev_preview::round::round_frames(&ctx)),
```

In `scenario_metadata`, add a `round-` match arm that sets `kind` to `PreviewScenarioKind::Round`, records privacy inputs, and fills `round: Some(...)`.

- [ ] **Step 9: Add round artifacts to manifest**

In `scenario_metadata`, set:

```rust
round: if frame.id.starts_with("round-") {
    Some(PreviewRoundMetadata {
        target_renderer: "preview-cells",
        aperture: PreviewRoundAperture {
            shape: "circle",
            center_x: (frame.width as f32 - 1.0) / 2.0,
            center_y: (frame.height as f32 - 1.0) / 2.0,
            radius: (frame.width.min(frame.height) as f32 / 2.0) - 1.0,
            safe_inner_radius: ((frame.width.min(frame.height) as f32 / 2.0) - 1.0) * 0.78,
            transparent_outside_aperture: true,
        },
        privacy: PreviewRoundPrivacy {
            source_names_visible: false,
            exact_counts_visible: false,
            diagnostic_text_visible: false,
        },
    })
} else {
    None
},
```

- [ ] **Step 10: Run a local pet-art render smoke**

Inspect `frames/round-normal.txt` after generation and confirm it contains
the current fixture pet glyphs from `WatchViewModel::fixture_with_habitat_props()`,
not a simplified stand-in. Keep `RoundSceneModel` free of terminal cell
coordinates and use the preview renderer as the place that consumes `vm.pet_art`.

The preview renderer entry point stays:

```rust
pub fn render_round_preview_frame_from_vm(
    id: impl Into<String>,
    title: impl Into<String>,
    vm: &crate::tui::view_model::WatchViewModel,
    now: time::OffsetDateTime,
    width: u16,
    height: u16,
) -> PreviewFrame
```

Then update `src/dev_preview/round.rs` to call `render_round_preview_frame_from_vm`.

- [ ] **Step 11: Run preview tests and generate review artifacts**

Run:

```bash
cargo test --test dev_preview dev_preview_round_writes_manifest_cells_and_round_metadata
cargo test --test dev_preview dev_preview_round_output_has_no_dashboard_labels_or_private_source_text
cargo test --test dev_preview dev_preview_round_aperture_corners_are_masked
cargo test --test dev_preview dev_preview_round_glitch_and_crystal_differ_by_symbols_in_flat_mode
cargo run -- dev-preview --scenario round --out target/glorp-preview-round
```

Expected: tests pass and `target/glorp-preview-round/index.html` opens with seven round fixtures.

- [ ] **Step 12: Commit**

```bash
git add src/cli.rs src/commands/dev_preview.rs src/dev_preview/mod.rs src/dev_preview/export.rs src/dev_preview/scenarios.rs src/dev_preview/frame.rs src/dev_preview/round.rs src/round/preview.rs tests/dev_preview.rs
git commit -m "feat: add round preview lab scenarios"
```

## Task 4: Shared Live Watch Presentation Pipeline

**Files:**
- Create: `src/watch_live.rs`
- Modify: `src/lib.rs`
- Modify: `src/menubar/app.rs`
- Test: module tests in `src/watch_live.rs`

- [ ] **Step 1: Write tests for shared presentation stamping**

Create `src/watch_live.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::life::AppliedUsageSignal;
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    #[test]
    fn install_live_signal_sets_feed_pulse_only_for_bursting_usage() {
        let mut state = WatchPresentationState::default();
        let mut vm = WatchViewModel::fixture();
        let signal = AppliedUsageSignal {
            applied_effective_tokens: 42_000.0,
            raw_effective_tokens: Some(42_000.0),
            source_mix: None,
            token_shape: None,
            observed_at: datetime!(2026-06-13 18:00 UTC),
            elapsed_since_successful_poll: time::Duration::minutes(1),
            freshness: crate::tui::life::UsageSignalFreshness::Live,
        };

        stamp_live_presentation(&mut state, &mut vm, signal, datetime!(2026-06-13 18:00 UTC));

        assert!(vm.life_profile.activity_level > 0.0);
        assert_eq!(vm.last_feed_pulse_at, Some(datetime!(2026-06-13 18:00 UTC)));
    }

    #[test]
    fn diagnostics_only_signal_does_not_create_feed_pulse() {
        let mut state = WatchPresentationState::default();
        let mut vm = WatchViewModel::fixture();
        let now = datetime!(2026-06-13 18:00 UTC);

        stamp_live_presentation(
            &mut state,
            &mut vm,
            AppliedUsageSignal::diagnostics_only(now, time::Duration::minutes(1)),
            now,
        );

        assert_eq!(vm.last_feed_pulse_at, None);
    }
}
```

- [ ] **Step 2: Run tests and confirm missing module**

Run:

```bash
cargo test watch_live
```

Expected: compile failure until `src/lib.rs` exports the module.

- [ ] **Step 3: Implement shared presentation state**

Create `src/watch_live.rs`:

```rust
use crate::commands::watch::{build_watch_view_model, poll_usage_and_apply};
use crate::error::Result;
use crate::paths::AppPaths;
use crate::storage::state::{PetState, StateStore};
use crate::tui::life::{AppliedUsageSignal, LifeSignalState};
use crate::tui::view_model::WatchViewModel;
use std::sync::mpsc;
use std::thread;
use std::time::Duration as StdDuration;
use time::OffsetDateTime;

#[derive(Debug, Default)]
pub struct WatchPresentationState {
    life_signal_state: LifeSignalState,
}

pub struct LiveWatchUpdate {
    pub pet_state: PetState,
    pub vm: WatchViewModel,
    pub applied_signal: AppliedUsageSignal,
}

pub fn stamp_live_presentation(
    state: &mut WatchPresentationState,
    vm: &mut WatchViewModel,
    applied_signal: AppliedUsageSignal,
    now: OffsetDateTime,
) {
    let profile = state
        .life_signal_state
        .observe(applied_signal, &vm.activity_identity, now);
    vm.life_profile = profile;
    vm.life_profile.calm_mode = vm.day_context.asleep;
    vm.last_feed_pulse_at = applied_signal.can_burst().then_some(now);
}

pub fn spawn_live_watch_worker(paths: AppPaths, interval: StdDuration, name: &str) -> mpsc::Receiver<LiveWatchUpdate> {
    let (tx, rx) = mpsc::channel::<LiveWatchUpdate>();
    let thread_name = name.to_string();
    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let state_store = StateStore::new(paths.state_file.clone());
            loop {
                thread::sleep(interval);
                let outcome = match poll_usage_and_apply(&state_store, &paths.usage_db, &paths.config_file) {
                    Ok(Some(outcome)) => outcome,
                    Ok(None) | Err(_) => continue,
                };
                let vm = match build_watch_view_model(&outcome.state, &paths.usage_db) {
                    Ok(vm) => vm,
                    Err(_) => continue,
                };
                if tx
                    .send(LiveWatchUpdate {
                        pet_state: outcome.state,
                        vm,
                        applied_signal: outcome.applied_signal,
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
        .expect("spawn glorp live watch worker");
    rx
}
```

Modify `src/lib.rs`:

```rust
pub mod watch_live;
```

- [ ] **Step 4: Migrate menubar to shared stamping and worker**

In `src/menubar/app.rs`:

- Replace `life_signal_state: crate::tui::life::LifeSignalState` with `presentation_state: crate::watch_live::WatchPresentationState`.
- Replace the local `PollResult` struct with `crate::watch_live::LiveWatchUpdate`.
- Replace `spawn_poll_worker(paths)` call with:

```rust
let poll_rx = crate::watch_live::spawn_live_watch_worker(
    paths,
    POLL_INTERVAL,
    "glorp-menubar-poll",
);
```

- In `drain_poll_results`, replace the inline `LifeSignalState::observe` block with:

```rust
crate::watch_live::stamp_live_presentation(
    &mut s.presentation_state,
    &mut vm,
    result.applied_signal,
    time::OffsetDateTime::now_utc(),
);
```

- Remove the local `spawn_poll_worker` function and the local `PollResult` type.

- [ ] **Step 5: Run focused tests and macOS compile check**

Run:

```bash
cargo test watch_live
cargo test --test tui_render animation_advances_while_poll_is_in_flight
cargo check
```

Expected: all pass. On macOS, `cargo check` also compiles the menubar changes.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/watch_live.rs src/menubar/app.rs
git commit -m "refactor: share live watch presentation state"
```

## Task 5: Helper Locator, Companion Launcher, And App Bundle Packaging

**Files:**
- Create: `src/usage/helper_locator.rs`
- Create: `src/commands/companion.rs`
- Modify: `src/usage/mod.rs`
- Modify: `src/usage/ccusage.rs`
- Modify: `src/cli.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/lib.rs`
- Create: `scripts/build-macos-companion-app.mjs`
- Modify: `scripts/build-platform-package.mjs`
- Modify: `npm/platform/darwin-arm64/package.json`
- Modify: `npm/platform/darwin-x64/package.json`
- Modify: `npm/glorp/bin/glorp.js`
- Modify: `npm/glorp/test/smoke.mjs`
- Test: `tests/helper_locator.rs`
- Test: `tests/cli_smoke.rs`

- [ ] **Step 1: Write helper locator tests**

Create `tests/helper_locator.rs`:

```rust
use glorp::usage::helper_locator::{read_helper_locator, write_helper_locator, HelperLocator};

#[test]
fn helper_locator_round_trips_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("helper-locator.json");
    let locator = HelperLocator {
        ccusage_bin: Some(dir.path().join("ccusage/bin/helper.js")),
        ccusage_codex_bin: Some(dir.path().join("ccusage-codex/bin/helper.js")),
        node_bin: Some(dir.path().join("node/bin/node")),
    };

    write_helper_locator(&file, &locator).unwrap();
    let loaded = read_helper_locator(&file).unwrap().unwrap();

    assert_eq!(loaded, locator);
}

#[test]
fn missing_helper_locator_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = read_helper_locator(&dir.path().join("missing.json")).unwrap();

    assert_eq!(loaded, None);
}
```

- [ ] **Step 2: Add CLI smoke tests**

Append to `tests/cli_smoke.rs`:

```rust
#[test]
fn help_lists_companion_but_hides_companion_app() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("companion"))
        .stdout(predicate::str::contains("companion-app").not());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn companion_reports_macos_only_on_other_platforms() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("companion")
        .assert()
        .failure()
        .stderr(predicate::str::contains("glorp companion is only available on macOS"));
}
```

- [ ] **Step 3: Run tests and confirm failures**

Run:

```bash
cargo test --test helper_locator
cargo test --test cli_smoke help_lists_companion_but_hides_companion_app
```

Expected: compile failure for missing helper locator and missing `companion` command.

- [ ] **Step 4: Implement helper locator persistence**

Create `src/usage/helper_locator.rs`:

```rust
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const HELPER_LOCATOR_FILE: &str = "helper-locator.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperLocator {
    pub ccusage_bin: Option<PathBuf>,
    pub ccusage_codex_bin: Option<PathBuf>,
    pub node_bin: Option<PathBuf>,
}

impl HelperLocator {
    pub fn from_current_environment() -> Self {
        Self {
            ccusage_bin: std::env::var_os("GLORP_CCUSAGE_BIN").map(PathBuf::from),
            ccusage_codex_bin: std::env::var_os("GLORP_CCUSAGE_CODEX_BIN").map(PathBuf::from),
            node_bin: std::env::var_os("GLORP_NODE_BIN").map(PathBuf::from),
        }
    }

    pub fn has_any_path(&self) -> bool {
        self.ccusage_bin.is_some() || self.ccusage_codex_bin.is_some() || self.node_bin.is_some()
    }
}

pub fn write_helper_locator(path: &Path, locator: &HelperLocator) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(locator)?)?;
    Ok(())
}

pub fn read_helper_locator(path: &Path) -> Result<Option<HelperLocator>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}
```

Modify `src/usage/mod.rs`:

```rust
pub mod helper_locator;
```

- [ ] **Step 5: Add HelperDiscovery fallback**

In `src/usage/ccusage.rs`, update `HelperDiscovery::discover()` after env/PATH discovery:

```rust
let mut discovered = Self { claude, codex, node };
if discovered.claude.is_none() || discovered.codex.is_none() || discovered.node.is_none() {
    if let Ok(paths) = crate::paths::AppPaths::resolve() {
        let locator_path = paths.config_dir.join(crate::usage::helper_locator::HELPER_LOCATOR_FILE);
        if let Ok(Some(locator)) = crate::usage::helper_locator::read_helper_locator(&locator_path) {
            if discovered.claude.is_none() {
                discovered.claude = locator.ccusage_bin;
            }
            if discovered.codex.is_none() {
                discovered.codex = locator.ccusage_codex_bin;
            }
            if discovered.node.is_none() {
                discovered.node = locator.node_bin;
            }
        }
    }
}
discovered
```

- [ ] **Step 6: Add public `companion` and hidden `companion-app` commands**

Modify `src/cli.rs`:

```rust
/// Open the native macOS round companion app.
Companion,
#[command(hide = true)]
CompanionApp,
```

Modify `src/lib.rs` match:

```rust
Command::Companion => commands::companion::run()?,
Command::CompanionApp => commands::companion_app::run()?,
```

Create `src/commands/companion.rs`:

```rust
use crate::error::{GlorpError, Result};

#[cfg(target_os = "macos")]
pub fn run() -> Result<()> {
    let paths = crate::paths::AppPaths::resolve()?;
    paths.ensure()?;
    let locator = crate::usage::helper_locator::HelperLocator::from_current_environment();
    if locator.has_any_path() {
        crate::usage::helper_locator::write_helper_locator(
            &paths.config_dir.join(crate::usage::helper_locator::HELPER_LOCATOR_FILE),
            &locator,
        )?;
    }
    let app = companion_app_path()?;
    let status = std::process::Command::new("open").arg(&app).status()?;
    if !status.success() {
        return Err(GlorpError::Message(format!(
            "failed to open Glorp.app at {}",
            app.display()
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion is only available on macOS".into(),
    ))
}

#[cfg(target_os = "macos")]
fn companion_app_path() -> Result<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("GLORP_COMPANION_APP") {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    let dev = std::path::PathBuf::from("target/macos/Glorp.app");
    if dev.exists() {
        return Ok(dev);
    }
    let installed = std::path::PathBuf::from("/Applications/Glorp.app");
    if installed.exists() {
        return Ok(installed);
    }
    Err(GlorpError::Message(
        "Glorp.app was not found; run `node scripts/build-macos-companion-app.mjs --profile debug` in development".into(),
    ))
}
```

Create `src/commands/companion_app.rs`:

```rust
use crate::error::{GlorpError, Result};

#[cfg(target_os = "macos")]
pub fn run() -> Result<()> {
    crate::companion::run()
}

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion-app is only available on macOS".into(),
    ))
}
```

Modify `src/commands/mod.rs`:

```rust
pub mod companion;
pub mod companion_app;
```

- [ ] **Step 7: Add npm wrapper companion app env**

In `npm/glorp/bin/glorp.js`, add:

```js
function resolveCompanionApp(env) {
  if (process.platform !== "darwin") return undefined;
  if (env.GLORP_COMPANION_APP_FOR_TEST) return env.GLORP_COMPANION_APP_FOR_TEST;
  const pkgJson = resolvePackageJson(platformPackageName());
  if (!pkgJson) return undefined;
  const app = path.join(path.dirname(pkgJson), "app", "Glorp.app");
  return fs.existsSync(app) ? app : undefined;
}

const companionApp = resolveCompanionApp(env);
if (companionApp) env.GLORP_COMPANION_APP ??= companionApp;
```

Update `npm/glorp/test/smoke.mjs` fake native env log:

```js
companionApp: process.env.GLORP_COMPANION_APP
```

Add assertion:

```js
const fakeApp = path.join(tempRoot, "Glorp.app");
fs.mkdirSync(fakeApp, { recursive: true });
const companion = run(["companion"], {
  GLORP_COMPANION_APP_FOR_TEST: fakeApp
});
assert.equal(companion.status, 0, companion.stderr);
const companionEnv = JSON.parse(fs.readFileSync(envLog, "utf8"));
assert.equal(companionEnv.companionApp, fakeApp);
```

- [ ] **Step 8: Create regular Dock app bundler**

Create `scripts/build-macos-companion-app.mjs` by copying the structure of `scripts/build-macos-app.mjs` with these required differences:

```js
const bundleIdentifier = "dev.glorp.companion";
const bundledBinaryName = "glorp-companion";
fs.writeFileSync(
  launcherPath,
  `#!/bin/sh\nexec "$(dirname "$0")/${bundledBinaryName}" companion-app "$@"\n`,
  { mode: 0o755 },
);
```

The generated `Info.plist` must contain:

```xml
<key>CFBundleIdentifier</key><string>dev.glorp.companion</string>
<key>CFBundleExecutable</key><string>Glorp</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>NSHighResolutionCapable</key><true/>
```

The generated `Info.plist` must not contain `LSUIElement`.

- [ ] **Step 9: Include app bundle in darwin platform packages**

Modify `scripts/build-platform-package.mjs`:

```js
if (platform.startsWith("darwin-")) {
  const appSource = path.join(repoRoot, "target", "macos", "Glorp.app");
  const appDest = path.join(repoRoot, "npm", "platform", platform, "app", "Glorp.app");
  if (!fs.existsSync(appSource)) {
    fail(`missing companion app ${appSource}; run \`node scripts/build-macos-companion-app.mjs\` first`);
  }
  fs.rmSync(appDest, { recursive: true, force: true });
  fs.cpSync(appSource, appDest, { recursive: true });
}
```

Modify both darwin package files:

```json
"files": [
  "bin/glorp",
  "app/Glorp.app"
]
```

- [ ] **Step 10: Run focused verification**

Run:

```bash
cargo test --test helper_locator
cargo test --test cli_smoke help_lists_companion_but_hides_companion_app
npm --workspace @arittr/glorp test
```

On macOS also run:

```bash
node scripts/build-macos-companion-app.mjs --profile debug
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' target/macos/Glorp.app/Contents/Info.plist
/usr/libexec/PlistBuddy -c 'Print :LSUIElement' target/macos/Glorp.app/Contents/Info.plist
```

Expected: identifier prints `dev.glorp.companion`; `LSUIElement` print fails because the key is absent.

- [ ] **Step 11: Commit**

```bash
git add src/usage/helper_locator.rs src/usage/mod.rs src/usage/ccusage.rs src/cli.rs src/lib.rs src/commands/mod.rs src/commands/companion.rs src/commands/companion_app.rs scripts/build-macos-companion-app.mjs scripts/build-platform-package.mjs npm/platform/darwin-arm64/package.json npm/platform/darwin-x64/package.json npm/glorp/bin/glorp.js npm/glorp/test/smoke.mjs tests/helper_locator.rs tests/cli_smoke.rs
git commit -m "feat: add macOS companion launcher packaging"
```

## Task 6: Native macOS Companion App And Renderer

**Files:**
- Create: `src/companion/mod.rs`
- Create: `src/companion/app.rs`
- Create: `src/companion/render.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Test: module tests in `src/companion/render.rs`

- [ ] **Step 1: Enable required AppKit/Foundation features**

Modify `Cargo.toml` target macOS dependencies:

```toml
objc2-foundation = { version = "0.2", features = [
    "NSAttributedString",
    "NSDictionary",
    "NSGeometry",
    "NSRange",
    "NSRunLoop",
    "NSString",
    "NSThread",
    "NSTimer",
    "NSUserDefaults",
    "NSValue",
] }
objc2-app-kit = { version = "0.2", features = [
    "NSAppearance",
    "NSApplication",
    "NSAttributedString",
    "NSBezierPath",
    "NSButton",
    "NSColor",
    "NSControl",
    "NSFont",
    "NSFontDescriptor",
    "NSGraphics",
    "NSGraphicsContext",
    "NSImage",
    "NSParagraphStyle",
    "NSPopover",
    "NSResponder",
    "NSRunningApplication",
    "NSScreen",
    "NSScrollView",
    "NSStatusBar",
    "NSStatusBarButton",
    "NSStatusItem",
    "NSText",
    "NSTextContainer",
    "NSTextStorage",
    "NSTextView",
    "NSView",
    "NSViewController",
    "NSWindow",
] }
```

- [ ] **Step 2: Write renderer command tests**

Create `src/companion/render.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
    use crate::round::model::derive_round_scene_model;
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    #[test]
    fn draw_commands_keep_all_points_inside_aperture() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(360, 360),
            RoundRenderCapabilities::preview_truecolor(),
        );

        let commands = build_draw_commands(&scene, &layout);

        assert!(commands.iter().all(|command| layout.aperture.contains(command.x, command.y)));
        assert!(commands.iter().any(|command| command.kind == RoundDrawKind::PetGlyph));
        assert!(commands.iter().any(|command| command.kind == RoundDrawKind::Halo));
    }
}
```

- [ ] **Step 3: Implement pure draw commands**

In `src/companion/render.rs`:

```rust
use crate::round::layout::{RoundAnchorKind, RoundSceneLayout};
use crate::round::model::RoundSceneModel;

#[derive(Debug, Clone, PartialEq)]
pub struct RoundDrawCommand {
    pub kind: RoundDrawKind,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub label: Option<char>,
    pub color: RoundColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn build_draw_commands(scene: &RoundSceneModel, layout: &RoundSceneLayout) -> Vec<RoundDrawCommand> {
    let mut commands = vec![RoundDrawCommand {
        kind: RoundDrawKind::Background,
        x: layout.aperture.center_x,
        y: layout.aperture.center_y,
        radius: layout.aperture.radius,
        label: None,
        color: RoundColor(0.08, 0.09, 0.10, 1.0),
    }];
    commands.push(RoundDrawCommand {
        kind: RoundDrawKind::PetGlyph,
        x: layout.pet_anchor.x,
        y: layout.pet_anchor.y,
        radius: layout.pet_anchor.radius,
        label: Some('g'),
        color: RoundColor(0.93, 0.92, 0.89, 1.0),
    });
    for anchor in &layout.prop_anchors {
        commands.push(RoundDrawCommand {
            kind: RoundDrawKind::PropGlyph,
            x: anchor.x,
            y: anchor.y,
            radius: anchor.radius,
            label: Some('*'),
            color: RoundColor(0.70, 0.82, 0.52, 1.0),
        });
    }
    for anchor in &layout.halo_anchors {
        commands.push(RoundDrawCommand {
            kind: if anchor.kind == RoundAnchorKind::HelperTrouble {
                RoundDrawKind::Trouble
            } else {
                RoundDrawKind::Halo
            },
            x: anchor.x,
            y: anchor.y,
            radius: anchor.radius,
            label: None,
            color: if scene.lifecycle.calm {
                RoundColor(0.36, 0.40, 0.55, 0.8)
            } else {
                RoundColor(0.94, 0.65, 0.28, 0.9)
            },
        });
    }
    commands
}
```

- [ ] **Step 4: Add companion module gate**

Create `src/companion/mod.rs`:

```rust
#![cfg(target_os = "macos")]

pub mod app;
pub mod render;

pub fn run() -> crate::error::Result<()> {
    app::run()
}
```

Modify `src/lib.rs`:

```rust
#[cfg(target_os = "macos")]
pub mod companion;
```

- [ ] **Step 5: Implement app skeleton with regular Dock lifecycle**

Create `src/companion/app.rs` using `src/menubar/app.rs` as the style reference, with these required differences:

```rust
#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::sync::mpsc;
use std::time::Duration;

use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString, NSTimer};

use crate::commands::watch::build_watch_view_model;
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundSceneModel};
use crate::storage::state::StateStore;
use crate::watch_live::{LiveWatchUpdate, WatchPresentationState};

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const UI_TICK_INTERVAL_SECS: f64 = 0.25;
const DEFAULT_WINDOW_SIZE: f64 = 360.0;

struct AppState {
    window: Retained<NSWindow>,
    view: Retained<RoundView>,
    poll_rx: mpsc::Receiver<LiveWatchUpdate>,
    presentation_state: WatchPresentationState,
    scene: RoundSceneModel,
}

thread_local! {
    static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

declare_class!(
    pub(super) struct Controller;

    unsafe impl ClassType for Controller {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "GlorpCompanionController";
    }

    impl DeclaredClass for Controller {}

    unsafe impl Controller {
        #[method(uiTick:)]
        fn ui_tick(&self, _sender: Option<&AnyObject>) {
            ui_tick();
        }
    }
);

declare_class!(
    pub(super) struct RoundView;

    unsafe impl ClassType for RoundView {
        type Super = NSView;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "GlorpRoundCompanionView";
    }

    impl DeclaredClass for RoundView {}
);

pub fn run() -> Result<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| GlorpError::Message("glorp companion must run on the main thread".into()))?;
    let paths = AppPaths::resolve()?;
    paths.ensure()?;
    let state_store = StateStore::new(paths.state_file.clone());
    let Some(initial_pet) = state_store.load()? else {
        return Err(GlorpError::Message("no glorp pet exists yet; run `glorp init` first".into()));
    };
    let initial_vm = build_watch_view_model(&initial_pet, &paths.usage_db)?;
    let scene = derive_round_scene_model(&initial_vm, time::OffsetDateTime::now_utc());

    let app: Retained<NSApplication> = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let controller: Retained<Controller> = unsafe { msg_send_id![Controller::class(), new] };
    let (window, view) = build_window(mtm);
    let poll_rx = crate::watch_live::spawn_live_watch_worker(
        paths,
        POLL_INTERVAL,
        "glorp-companion-poll",
    );

    APP_STATE.with(|cell| {
        *cell.borrow_mut() = Some(AppState {
            window,
            view,
            poll_rx,
            presentation_state: WatchPresentationState::default(),
            scene,
        });
    });

    let _timer: Retained<NSTimer> = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            UI_TICK_INTERVAL_SECS,
            &controller,
            sel!(uiTick:),
            None,
            true,
        )
    };

    unsafe { app.run() };
    Ok(())
}
```

Fill `build_window` with:

```rust
let frame = NSRect::new(NSPoint::new(120.0, 120.0), NSSize::new(DEFAULT_WINDOW_SIZE, DEFAULT_WINDOW_SIZE));
let style = NSWindowStyleMask::Titled
    | NSWindowStyleMask::Closable
    | NSWindowStyleMask::Miniaturizable
    | NSWindowStyleMask::Resizable
    | NSWindowStyleMask::FullSizeContentView;
let window = NSWindow::initWithContentRect_styleMask_backing_defer(
    mtm.alloc(),
    frame,
    style,
    NSBackingStoreType::NSBackingStoreBuffered,
    false,
);
window.setTitle(&NSString::from_str("Glorp"));
window.setContentMinSize(NSSize::new(260.0, 260.0));
window.setReleasedWhenClosed(false);
```

Set a custom `RoundView` as content view and call `window.makeKeyAndOrderFront(None)`.

- [ ] **Step 6: Connect scene updates to the view**

In `ui_tick`, drain the receiver, stamp live presentation, derive `RoundSceneModel`, store it in `APP_STATE`, and call `view.setNeedsDisplay(true)`.

Use this pattern:

```rust
fn ui_tick() {
    let mut latest = None;
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow().as_ref() {
            while let Ok(update) = state.poll_rx.try_recv() {
                latest = Some(update);
            }
        }
    });
    if let Some(update) = latest {
        APP_STATE.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                let mut vm = update.vm;
                crate::watch_live::stamp_live_presentation(
                    &mut state.presentation_state,
                    &mut vm,
                    update.applied_signal,
                    time::OffsetDateTime::now_utc(),
                );
                state.scene = derive_round_scene_model(&vm, time::OffsetDateTime::now_utc());
                unsafe { state.view.setNeedsDisplay(true) };
            }
        });
    }
}
```

- [ ] **Step 7: Draw the scene in AppKit**

Implement `drawRect:` on `RoundView` by reading the current scene from `APP_STATE`, deriving layout from the view bounds, building draw commands with `build_draw_commands`, and mapping commands to `NSBezierPath` fills and text glyphs. Use only AppKit primitives and no WebView.

The first native renderer must draw:

- circular background;
- pet glyph cluster centered in the aperture;
- one or two prop landmarks;
- halo beads;
- helper trouble bead;
- asleep/calm dimming.

- [ ] **Step 8: Run focused checks**

Run:

```bash
cargo test companion::render
cargo check
node scripts/build-macos-companion-app.mjs --profile debug
target/debug/glorp companion-app
```

Expected: tests pass, app bundle builds, and `target/debug/glorp companion-app` opens a Dock-visible Glorp window on macOS. Quit the app manually after verifying the window.

- [ ] **Step 9: Smoke the launcher and no-env app path**

Run:

```bash
GLORP_CONFIG_DIR="$(mktemp -d)" cargo run -- init --seed mochi-7f3a --name mochi
node scripts/build-macos-companion-app.mjs --profile debug
cargo run -- companion
env -i HOME="$HOME" USER="$USER" PATH="/usr/bin:/bin" open target/macos/Glorp.app
```

Expected: `cargo run -- companion` exits after opening/focusing the app. The no-env `open` launches the same app and shows either the companion scene or a quiet helper-trouble state; it does not panic or open an empty blank window.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml src/lib.rs src/companion/mod.rs src/companion/app.rs src/companion/render.rs
git commit -m "feat: add native macOS companion app"
```

## Task 7: Final Contract Verification And Docs

**Files:**
- Modify: `docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md` only if implementation reveals a spec correction.
- Modify: `npm/glorp/README.md`
- Test: full relevant command set

- [ ] **Step 1: Add README companion entry without documenting hidden surfaces**

In `npm/glorp/README.md`, add a compact command entry:

```md
### Native macOS companion

On macOS, `glorp companion` opens the Dock-visible Glorp companion app. The
companion is a quiet round pet window for a normal display; detailed usage
diagnostics remain in `glorp watch`, `glorp status`, and `glorp doctor`.
```

Do not mention `companion-app` or `watch --view round`.

- [ ] **Step 2: Run full targeted verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test round_scene
cargo test --test helper_locator
cargo test --test dev_preview
cargo test --test cli_smoke
cargo test watch_live
cargo test round::
npm test
```

On macOS also run:

```bash
node scripts/build-macos-companion-app.mjs --profile debug
cargo run -- dev-preview --scenario round --out target/glorp-preview-round
cargo run -- companion
env -i HOME="$HOME" USER="$USER" PATH="/usr/bin:/bin" open target/macos/Glorp.app
```

- [ ] **Step 3: Inspect generated preview artifacts**

Open:

```bash
open target/glorp-preview-round/index.html
```

Manual acceptance:

- `round-active-pulse` is the V1 default direction.
- `round-normal` reads as quiet porthole.
- `round-asleep-night` reads as the night/calm variant.
- no dashboard labels are visible.
- helper trouble is visible without text.
- Glitch and Crystal differ by non-color symbols.
- pet art is centered and not clipped.
- outside aperture corners are blank/transparent.

- [ ] **Step 4: Inspect bundle contract**

Run:

```bash
/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' target/macos/Glorp.app/Contents/Info.plist
/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' target/macos/Glorp.app/Contents/Info.plist
/usr/libexec/PlistBuddy -c 'Print :LSUIElement' target/macos/Glorp.app/Contents/Info.plist
```

Expected:

- `CFBundleIdentifier` is `dev.glorp.companion`.
- `CFBundleExecutable` is `Glorp`.
- `LSUIElement` command fails because the key is absent.

- [ ] **Step 5: Confirm no V1 terminal round surface leaked**

Run:

```bash
cargo run -- help
cargo run -- help | rg "companion-app|watch --view round|round terminal"
rg -n "watch --view round|companion-app|round terminal" npm/glorp/README.md
rg -n "LSUIElement" scripts/build-macos-companion-app.mjs
```

Expected:

- Help lists `companion`.
- The help `rg` command exits nonzero because hidden/debug surfaces are absent.
- The README `rg` command exits nonzero because public docs omit hidden/debug surfaces.
- The companion bundler `rg` command exits nonzero because the companion app is not `LSUIElement`.

- [ ] **Step 6: Commit**

```bash
git add npm/glorp/README.md docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md
git commit -m "docs: document macOS companion"
```

Skip the spec path in `git add` when no spec correction was made.

## Spec Coverage Checklist

- Native macOS facade: Tasks 5 and 6.
- Normal Dock app lifecycle: Tasks 5 and 6.
- `glorp companion` opens app and exits: Task 5.
- No V1 `glorp watch --view round`: Task 7.
- Pure `RoundSceneModel`: Task 1.
- `RoundSceneLayout` and renderer-neutral moments: Task 2.
- Preview Lab round scenario and mask metadata: Task 3.
- Existing watch state and live presentation stamping: Task 4.
- Helper discovery without inherited environment: Task 5.
- No-pet and helper trouble handling: Tasks 5 and 6.
- Privacy allowlist and dashboard exclusions: Tasks 1, 3, and 7.
- AppKit main-thread ownership and worker polling: Tasks 4 and 6.
- Package/release artifact decision: Task 5.

## Execution Handoff

Plan execution should use small commits in task order. The recommended execution mode is subagent-driven development with one fresh subagent per task and review between tasks; the inline mode is acceptable for Tasks 1-3 if Drew wants tighter interactive control over the visual direction.
