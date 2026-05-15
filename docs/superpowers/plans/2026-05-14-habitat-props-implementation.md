# Habitat Props Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a small habitat prop system that unlocks from real Glorp milestones, persists earned prop facts, and renders subtle lived-in objects inside the existing pet scene.

**Architecture:** Persist earned prop facts on `PetState`; keep prop catalog and unlock detection in `game::habitat`; expose catalog-backed prop data through `WatchViewModel`; derive placement and motion at render time from `PetSceneLayout`, the fixed watch clock, and the earned prop list. Rendering stays in the existing pet panel pass order: ambient texture, habitat props, then pet art/effects.

**Tech Stack:** Rust 2021, serde, time, ratatui buffers, existing `rand_pcg`/`rand` patterns, existing dev-preview fixtures and tests.

---

## File Map

- Modify `src/storage/state.rs`: add `HabitatState`, `EarnedHabitatProp`, `HabitatPropId`, `HabitatPropSource`; add `PetState.habitat` with serde default; seed fixture state with empty habitat.
- Create `src/game/habitat.rs`: define the V1 prop catalog, ladder thresholds, duplicate-safe unlock helpers, and runtime unlock detection.
- Modify `src/game/mod.rs`: export `habitat`.
- Modify `src/game/runtime.rs`: call habitat unlock detection after usage/mood changes are known.
- Modify `src/tui/view_model.rs`: add `HabitatView` and `EarnedHabitatPropView`; update fixtures.
- Modify `src/commands/watch.rs`: convert stored habitat props into catalog-backed `HabitatView`.
- Create `src/tui/component/habitat_props.rs`: pure placement and motion renderer for prop cells.
- Modify `src/tui/component/mod.rs`: export habitat prop renderer types/functions for tests and pet panel use.
- Modify `src/tui/panels/pet.rs`: render habitat prop cells between ambient glyphs and pet art.
- Modify `src/dev_preview/watch.rs`: seed preview state with representative habitat props.
- Modify `src/dev_preview/scenarios.rs`: include preview fixture prop ids in manifest scenario inputs.
- Modify `tests/runtime_integration.rs`: runtime unlock and persistence coverage.
- Modify `tests/watch_integration.rs`: view-model filtering and catalog-backed prop coverage.
- Modify `tests/tui_render.rs`: render integration and draw-order coverage.
- Modify `tests/dev_preview.rs`: manifest and artifact coverage for prop fixture ids.

---

### Task 1: Add Habitat State And Catalog Foundation

**Files:**
- Modify: `src/storage/state.rs`
- Create: `src/game/habitat.rs`
- Modify: `src/game/mod.rs`
- Test: `tests/runtime_integration.rs`

- [ ] **Step 1: Write failing state and catalog tests**

Append these tests to `tests/runtime_integration.rs`:

```rust
#[test]
fn fresh_pet_state_starts_with_empty_habitat_state() {
    let state = PetState::new_for_test("mochi-7f3a", "mochi");

    assert!(state.habitat.earned_props.is_empty());
    assert_eq!(state.habitat.reconciled_lifetime_tokens_at, None);
}

#[test]
fn habitat_catalog_exposes_v1_prop_ids_and_kinds() {
    use glorp::game::habitat::{catalog_prop, HabitatPropKind};
    use glorp::storage::state::HabitatPropId;

    let codex = catalog_prop(&HabitatPropId::new("codex_signal_lamp")).unwrap();
    assert_eq!(codex.kind, HabitatPropKind::Trophy);
    assert_eq!(codex.display_priority, 70);

    let pebble = catalog_prop(&HabitatPropId::new("token_pebble_25k")).unwrap();
    assert_eq!(pebble.kind, HabitatPropKind::Accent);
    assert_eq!(pebble.lifetime_threshold, Some(25_000.0));

    assert!(catalog_prop(&HabitatPropId::new("non_catalog_prop_for_filter_test")).is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_integration habitat -- --nocapture
```

Expected: FAIL to compile with missing `PetState.habitat`, `game::habitat`, and `HabitatPropId`.

- [ ] **Step 3: Add state types**

In `src/storage/state.rs`, extend the imports and add these types near `NarrativeEvent`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HabitatPropId(String);

impl HabitatPropId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for HabitatPropId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarnedHabitatProp {
    pub id: HabitatPropId,
    pub earned_at: OffsetDateTime,
    pub source: HabitatPropSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HabitatPropSource {
    LifetimeTokens { threshold: f64 },
    ProviderFirstUse { provider_surface: String },
    HeavySession,
    WiltRecovery,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HabitatState {
    pub earned_props: Vec<EarnedHabitatProp>,
    pub reconciled_lifetime_tokens_at: Option<f64>,
}
```

Add this field to `PetState` after `recent_events`:

```rust
    /// Earned habitat props. Stores durable unlock facts only; placement and
    /// motion are derived by the watch renderer from layout and clock.
    #[serde(default)]
    pub habitat: HabitatState,
```

Add `habitat: HabitatState::default(),` to `PetState::new_for_test`.

- [ ] **Step 4: Add catalog module**

Create `src/game/habitat.rs`:

```rust
use crate::storage::state::HabitatPropId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HabitatPropKind {
    Trophy,
    Accent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HabitatPropSpec {
    pub id: &'static str,
    pub kind: HabitatPropKind,
    pub display_priority: i16,
    pub lifetime_threshold: Option<f64>,
}

pub const TOKEN_PEBBLE_25K: &str = "token_pebble_25k";
pub const TOKEN_SHELL_100K: &str = "token_shell_100k";
pub const TOKEN_SPARK_500K: &str = "token_spark_500k";
pub const TOKEN_SHARD_1M: &str = "token_shard_1m";
pub const TOKEN_ORBIT_5M: &str = "token_orbit_5m";
pub const TOKEN_LANTERN_10M: &str = "token_lantern_10m";
pub const CODEX_SIGNAL_LAMP: &str = "codex_signal_lamp";
pub const HEAVY_SESSION_PLANTER: &str = "heavy_session_planter";
pub const WILT_RECOVERY_SPROUT: &str = "wilt_recovery_sprout";

pub const HABITAT_PROP_CATALOG: &[HabitatPropSpec] = &[
    HabitatPropSpec {
        id: TOKEN_PEBBLE_25K,
        kind: HabitatPropKind::Accent,
        display_priority: 10,
        lifetime_threshold: Some(25_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_SHELL_100K,
        kind: HabitatPropKind::Accent,
        display_priority: 20,
        lifetime_threshold: Some(100_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_SPARK_500K,
        kind: HabitatPropKind::Accent,
        display_priority: 30,
        lifetime_threshold: Some(500_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_SHARD_1M,
        kind: HabitatPropKind::Accent,
        display_priority: 40,
        lifetime_threshold: Some(1_000_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_ORBIT_5M,
        kind: HabitatPropKind::Accent,
        display_priority: 50,
        lifetime_threshold: Some(5_000_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_LANTERN_10M,
        kind: HabitatPropKind::Accent,
        display_priority: 60,
        lifetime_threshold: Some(10_000_000.0),
    },
    HabitatPropSpec {
        id: CODEX_SIGNAL_LAMP,
        kind: HabitatPropKind::Trophy,
        display_priority: 70,
        lifetime_threshold: None,
    },
    HabitatPropSpec {
        id: HEAVY_SESSION_PLANTER,
        kind: HabitatPropKind::Trophy,
        display_priority: 80,
        lifetime_threshold: None,
    },
    HabitatPropSpec {
        id: WILT_RECOVERY_SPROUT,
        kind: HabitatPropKind::Trophy,
        display_priority: 90,
        lifetime_threshold: None,
    },
];

pub fn catalog_prop(id: &HabitatPropId) -> Option<&'static HabitatPropSpec> {
    HABITAT_PROP_CATALOG.iter().find(|prop| prop.id == id.as_str())
}

pub fn ladder_props() -> impl Iterator<Item = &'static HabitatPropSpec> {
    HABITAT_PROP_CATALOG
        .iter()
        .filter(|prop| prop.lifetime_threshold.is_some())
}
```

Add `pub mod habitat;` to `src/game/mod.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test --test runtime_integration habitat -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/storage/state.rs src/game/habitat.rs src/game/mod.rs tests/runtime_integration.rs
git commit -m "feat(habitat): add prop state and catalog"
```

---

### Task 2: Implement Runtime Prop Unlock Detection

**Files:**
- Modify: `src/game/habitat.rs`
- Modify: `src/game/runtime.rs`
- Test: `tests/runtime_integration.rs`

- [ ] **Step 1: Write failing runtime unlock tests**

Append these tests to `tests/runtime_integration.rs`:

```rust
#[test]
fn lifetime_threshold_unlocks_one_ladder_prop_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &poll_with_delta(25_000.0, now), now).unwrap();
    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now + Duration::minutes(10)).unwrap();

    let ids = habitat_prop_ids(&state);
    assert_eq!(ids, vec!["token_pebble_25k"]);
}

#[test]
fn one_large_poll_unlocks_ladder_props_in_threshold_order() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &poll_with_delta(1_100_000.0, now), now).unwrap();

    assert_eq!(
        habitat_prop_ids(&state),
        vec![
            "token_pebble_25k",
            "token_shell_100k",
            "token_spark_500k",
            "token_shard_1m",
        ]
    );
}

#[test]
fn existing_lifetime_counter_reconciles_ladder_props_without_usage_delta() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.lifetime_effective_tokens = 125_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(
        habitat_prop_ids(&state),
        vec!["token_pebble_25k", "token_shell_100k"]
    );
    assert_eq!(state.habitat.reconciled_lifetime_tokens_at, Some(125_000.0));
}

#[test]
fn first_codex_usage_unlocks_signal_lamp_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &poll_with_surface("codex", 1_000.0, now), now).unwrap();
    apply_usage_poll(&mut state, &mut usage_store, &poll_with_surface("codex", 1_000.0, now + Duration::minutes(10)), now + Duration::minutes(10)).unwrap();

    let lamp_count = state
        .habitat
        .earned_props
        .iter()
        .filter(|prop| prop.id.as_str() == "codex_signal_lamp")
        .count();
    assert_eq!(lamp_count, 1);
}

#[test]
fn heavy_session_unlocks_planter_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &poll_with_delta(49_999.0, now), now).unwrap();
    assert!(!habitat_prop_ids(&state).contains(&"heavy_session_planter"));

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(50_000.0, now + Duration::minutes(10)),
        now + Duration::minutes(10),
    )
    .unwrap();

    assert!(habitat_prop_ids(&state).contains(&"heavy_session_planter"));
}

#[test]
fn wilted_recovery_unlocks_sprout_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.vitals = Vitals {
        fed: 2.0,
        happiness: 2.0,
        energy: 2.0,
    };
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &poll_with_delta(100_000.0, now), now).unwrap();

    assert!(habitat_prop_ids(&state).contains(&"wilt_recovery_sprout"));
}
```

Add these helpers near `poll_with_delta`:

```rust
fn habitat_prop_ids(state: &PetState) -> Vec<&str> {
    state
        .habitat
        .earned_props
        .iter()
        .map(|prop| prop.id.as_str())
        .collect()
}

fn poll_with_surface(
    provider_surface: &str,
    effective_tokens: f64,
    now: time::OffsetDateTime,
) -> UsagePollResult {
    let mut poll = poll_with_delta(effective_tokens, now);
    for delta in &mut poll.deltas {
        delta.provider_surface = provider_surface.to_string();
        delta.cursor_update.provider_surface = provider_surface.to_string();
        delta.cursor_update.cursor_key = format!("{provider_surface}-cursor-{}", now.unix_timestamp());
    }
    poll
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_integration -- --nocapture
```

Expected: FAIL because unlock detection is not implemented.

- [ ] **Step 3: Implement unlock helpers in `src/game/habitat.rs`**

Add these imports and functions:

```rust
use time::OffsetDateTime;

use crate::game::metabolism::Mood;
use crate::storage::{
    state::{EarnedHabitatProp, HabitatPropSource, PetState},
    usage_store::UsageLedgerRow,
};

const HEAVY_SESSION_MIN_TOKENS: f64 = 50_000.0;
const HEAVY_SESSION_BASELINE_FRACTION: f64 = 0.5;

pub fn unlock_habitat_props(
    state: &mut PetState,
    rows: &[UsageLedgerRow],
    recent_effective_tokens: f64,
    initial_mood: Mood,
    new_mood: Mood,
    now: OffsetDateTime,
) -> Vec<HabitatPropId> {
    let mut unlocked = Vec::new();
    unlock_lifetime_ladder(state, now, &mut unlocked);
    unlock_first_codex(state, rows, now, &mut unlocked);
    unlock_heavy_session(state, recent_effective_tokens, now, &mut unlocked);
    unlock_wilt_recovery(state, initial_mood, new_mood, now, &mut unlocked);
    unlocked
}

fn unlock_lifetime_ladder(
    state: &mut PetState,
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    let lifetime = state.lifetime_effective_tokens.max(0.0);
    for prop in ladder_props() {
        let threshold = prop.lifetime_threshold.unwrap_or(f64::INFINITY);
        if lifetime >= threshold {
            record_prop(
                state,
                HabitatPropId::new(prop.id),
                HabitatPropSource::LifetimeTokens { threshold },
                now,
                unlocked,
            );
        }
    }
    state.habitat.reconciled_lifetime_tokens_at = Some(lifetime);
}

fn unlock_first_codex(
    state: &mut PetState,
    rows: &[UsageLedgerRow],
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    if rows
        .iter()
        .any(|row| row.event.provider_surface == "codex" && row.event.effective_tokens > 0.0)
    {
        record_prop(
            state,
            HabitatPropId::new(CODEX_SIGNAL_LAMP),
            HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
            now,
            unlocked,
        );
    }
}

fn unlock_heavy_session(
    state: &mut PetState,
    recent_effective_tokens: f64,
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    let baseline = state.calibration.daily_effective_tokens.max(1.0);
    let threshold = HEAVY_SESSION_MIN_TOKENS.max(baseline * HEAVY_SESSION_BASELINE_FRACTION);
    if recent_effective_tokens >= threshold {
        record_prop(
            state,
            HabitatPropId::new(HEAVY_SESSION_PLANTER),
            HabitatPropSource::HeavySession,
            now,
            unlocked,
        );
    }
}

fn unlock_wilt_recovery(
    state: &mut PetState,
    initial_mood: Mood,
    new_mood: Mood,
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    if initial_mood == Mood::Wilted && new_mood != Mood::Wilted {
        record_prop(
            state,
            HabitatPropId::new(WILT_RECOVERY_SPROUT),
            HabitatPropSource::WiltRecovery,
            now,
            unlocked,
        );
    }
}

fn record_prop(
    state: &mut PetState,
    id: HabitatPropId,
    source: HabitatPropSource,
    earned_at: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    if state
        .habitat
        .earned_props
        .iter()
        .any(|prop| prop.id == id)
    {
        return;
    }

    state.habitat.earned_props.push(EarnedHabitatProp {
        id: id.clone(),
        earned_at,
        source,
    });
    unlocked.push(id);
}
```

- [ ] **Step 4: Wire runtime to unlock detection**

In `src/game/runtime.rs`, add `habitat` to the `game` import list and capture the initial mood:

```rust
    let initial_stage = state.stage;
    let initial_vitals = state.vitals;
    let initial_mood = mood_for_vitals(game_vitals(state.vitals));
```

After vital threshold narration and before `state.previous_vitals = Some(initial_vitals);`, call:

```rust
    habitat::unlock_habitat_props(
        state,
        &rows,
        recent_effective_tokens,
        initial_mood,
        new_mood,
        now,
    );
```

Keep the return value unused for now. The first version does not add unlock narration or toasts.

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test --test runtime_integration habitat -- --nocapture
cargo test --test runtime_integration -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/game/habitat.rs src/game/runtime.rs tests/runtime_integration.rs
git commit -m "feat(habitat): unlock props from runtime milestones"
```

---

### Task 3: Add Habitat Props To The Watch View Model

**Files:**
- Modify: `src/tui/view_model.rs`
- Modify: `src/commands/watch.rs`
- Test: `tests/watch_integration.rs`

- [ ] **Step 1: Write failing view-model tests**

Append these tests to `tests/watch_integration.rs`:

```rust
#[test]
fn watch_view_model_exposes_catalog_backed_habitat_props() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let _usage_store = UsageStore::open(&usage_db).unwrap();
    let mut state = mech_state();
    state.habitat.earned_props.push(glorp::storage::state::EarnedHabitatProp {
        id: glorp::storage::state::HabitatPropId::new("codex_signal_lamp"),
        earned_at: datetime!(2026-05-11 12:00:00 UTC),
        source: glorp::storage::state::HabitatPropSource::ProviderFirstUse {
            provider_surface: "codex".to_string(),
        },
    });
    state.habitat.earned_props.push(glorp::storage::state::EarnedHabitatProp {
        id: glorp::storage::state::HabitatPropId::new("non_catalog_prop_for_filter_test"),
        earned_at: datetime!(2026-05-11 12:01:00 UTC),
        source: glorp::storage::state::HabitatPropSource::HeavySession,
    });

    let vm = build_watch_view_model_for_test_at(&state, &usage_db, datetime!(2026-05-11 12:02:00 UTC)).unwrap();

    assert_eq!(vm.habitat.earned_props.len(), 1);
    assert_eq!(vm.habitat.earned_props[0].id.as_str(), "codex_signal_lamp");
    assert_eq!(vm.habitat.earned_props[0].display_priority, 70);
    assert_eq!(
        vm.habitat.earned_props[0].kind,
        glorp::game::habitat::HabitatPropKind::Trophy
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test watch_integration watch_view_model_exposes_catalog_backed_habitat_props -- --nocapture
```

Expected: FAIL to compile because `WatchViewModel.habitat` and view types do not exist.

- [ ] **Step 3: Add habitat view-model types**

In `src/tui/view_model.rs`, import `HabitatPropKind` and `HabitatPropId`, then add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HabitatView {
    pub earned_props: Vec<EarnedHabitatPropView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EarnedHabitatPropView {
    pub id: HabitatPropId,
    pub earned_at: time::OffsetDateTime,
    pub kind: HabitatPropKind,
    pub display_priority: i16,
}
```

Add `pub habitat: HabitatView,` to `WatchViewModel` near `pet_render`.

In `WatchViewModel::fixture()`, add:

```rust
            habitat: HabitatView {
                earned_props: Vec::new(),
            },
```

- [ ] **Step 4: Populate habitat view in `build_watch_view_model_at`**

In `src/commands/watch.rs`, add a helper near the other view-model helper functions:

```rust
fn build_habitat_view(state: &PetState) -> crate::tui::view_model::HabitatView {
    let earned_props = state
        .habitat
        .earned_props
        .iter()
        .filter_map(|earned| {
            let spec = crate::game::habitat::catalog_prop(&earned.id)?;
            Some(crate::tui::view_model::EarnedHabitatPropView {
                id: earned.id.clone(),
                earned_at: earned.earned_at,
                kind: spec.kind,
                display_priority: spec.display_priority,
            })
        })
        .collect();

    crate::tui::view_model::HabitatView { earned_props }
}
```

Set `habitat: build_habitat_view(state),` in the `WatchViewModel` constructor.

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test --test watch_integration watch_view_model_exposes_catalog_backed_habitat_props -- --nocapture
cargo test --test watch_integration -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/view_model.rs src/commands/watch.rs tests/watch_integration.rs
git commit -m "feat(watch): expose habitat props in view model"
```

---

### Task 4: Build Pure Habitat Prop Placement And Motion

**Files:**
- Create: `src/tui/component/habitat_props.rs`
- Modify: `src/tui/component/mod.rs`
- Test: `src/tui/component/habitat_props.rs`

- [ ] **Step 1: Write failing unit tests in the new module**

Create `src/tui/component/habitat_props.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::HabitatPropKind;
    use crate::pet::generation::Species;
    use crate::storage::state::HabitatPropId;
    use crate::tui::component::{ComponentPath, PetSceneLayout, TargetPath};
    use crate::tui::render_context::{RenderContext, WatchClock};
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::{EarnedHabitatPropView, HabitatView};
    use ratatui::layout::Rect;
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn scene() -> PetSceneLayout {
        PetSceneLayout {
            id: ComponentPath::new("watch.pet"),
            panel: Rect::new(0, 0, 40, 12),
            speech: Some(Rect::new(0, 0, 40, 1)),
            content: Rect::new(0, 1, 40, 11),
            pet_art: Rect::new(14, 3, 13, 8),
            hit_area: Rect::new(0, 1, 40, 11),
            habitat: Rect::new(0, 0, 40, 12),
            exclusions: vec![Rect::new(0, 0, 40, 1), Rect::new(14, 3, 13, 8)],
            targets: BTreeMap::new(),
            effect_targets: vec![TargetPath::new("watch.pet.effect")],
        }
    }

    fn ctx(ts: time::OffsetDateTime) -> RenderContext {
        RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(ts))
    }

    fn earned(id: &str, kind: HabitatPropKind, priority: i16, minute: u8) -> EarnedHabitatPropView {
        EarnedHabitatPropView {
            id: HabitatPropId::new(id),
            earned_at: datetime!(2026-05-11 12:00 UTC) + time::Duration::minutes(i64::from(minute)),
            kind,
            display_priority: priority,
        }
    }

    #[test]
    fn prop_cells_stay_inside_habitat_and_outside_exclusions() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0),
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 1),
            ],
        };

        let cells = habitat_props_for(
            &habitat,
            &scene(),
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:10 UTC)),
        );

        assert!(!cells.is_empty());
        for cell in cells {
            assert!(Rect::new(0, 0, 40, 12).contains(ratatui::layout::Position::new(cell.col, cell.row)));
            assert!(!Rect::new(0, 0, 40, 1).contains(ratatui::layout::Position::new(cell.col, cell.row)));
            assert!(!Rect::new(14, 3, 13, 8).contains(ratatui::layout::Position::new(cell.col, cell.row)));
        }
    }

    #[test]
    fn trophy_selection_caps_at_three_by_priority_then_age() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0),
                earned("heavy_session_planter", HabitatPropKind::Trophy, 80, 1),
                earned("wilt_recovery_sprout", HabitatPropKind::Trophy, 90, 2),
                earned("extra_trophy_for_cap_test", HabitatPropKind::Trophy, 95, 3),
            ],
        };

        let selected = visible_trophy_ids(&habitat);

        assert_eq!(
            selected,
            vec!["extra_trophy_for_cap_test", "wilt_recovery_sprout", "heavy_session_planter"]
        );
    }

    #[test]
    fn accent_rotation_is_stable_within_ten_minute_window() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 0),
                earned("token_shell_100k", HabitatPropKind::Accent, 20, 1),
                earned("token_spark_500k", HabitatPropKind::Accent, 30, 2),
                earned("token_shard_1m", HabitatPropKind::Accent, 40, 3),
                earned("token_orbit_5m", HabitatPropKind::Accent, 50, 4),
            ],
        };

        let a = visible_accent_ids(&habitat, datetime!(2026-05-11 12:01 UTC));
        let b = visible_accent_ids(&habitat, datetime!(2026-05-11 12:09 UTC));
        let c = visible_accent_ids(&habitat, datetime!(2026-05-11 12:11 UTC));

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 4);
    }

    #[test]
    fn flat_color_omits_accents_but_keeps_trophy_shapes() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0),
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 1),
            ],
        };
        let flat_ctx = RenderContext::with_clock(
            ColorCapability::Flat,
            WatchClock::fixed(datetime!(2026-05-11 12:10 UTC)),
        );

        let cells = habitat_props_for(&habitat, &scene(), Species::Fuzz, "fixture-seed", &flat_ctx);
        let glyphs = cells.iter().map(|cell| cell.glyph).collect::<Vec<_>>();

        assert!(glyphs.iter().any(|glyph| *glyph == '◉' || *glyph == '○'));
        assert!(!glyphs.contains(&'▲'));
    }

    #[test]
    fn prop_visual_glyphs_are_single_scalar_values() {
        for glyph in prop_visual_glyphs_for_test() {
            assert_eq!(glyph.to_string().chars().count(), 1, "{glyph} must be one char");
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --lib tui::component::habitat_props -- --nocapture
```

Expected: FAIL to compile because the module is not exported and functions are missing.

- [ ] **Step 3: Implement pure renderer**

Add this implementation above the tests in `src/tui/component/habitat_props.rs`:

```rust
use crate::game::habitat::HabitatPropKind;
use crate::pet::generation::Species;
use crate::tui::component::PetSceneLayout;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{tokenpet_palette, ColorCapability};
use crate::tui::view_model::HabitatView;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

const MAX_TROPHIES: usize = 3;
const MAX_ACCENTS: usize = 4;
const ACCENT_ROTATION_SECS: i64 = 600;

#[derive(Debug, Clone, PartialEq)]
pub struct HabitatPropCell {
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
}

#[derive(Clone, Copy)]
struct SpriteCell {
    dx: i16,
    dy: i16,
    glyph: char,
}

pub fn habitat_props_for(
    habitat: &HabitatView,
    scene: &PetSceneLayout,
    species: Species,
    seed: &str,
    ctx: &RenderContext,
) -> Vec<HabitatPropCell> {
    let now = ctx.clock.now_utc();
    let mut occupied = scene.exclusions.clone();
    let mut cells = Vec::new();

    for id in visible_trophy_ids(habitat) {
        if let Some(anchor) = trophy_anchor(id, scene.habitat) {
            let sprite = trophy_sprite(id, species, now);
            let rendered = render_sprite(
                anchor,
                sprite,
                scene.habitat,
                &occupied,
                trophy_style(ctx.color_capability),
            );
            if !rendered.is_empty() {
                occupied.push(bounds_for_cells(&rendered));
                cells.extend(rendered);
            }
        }
    }

    if matches!(ctx.color_capability, ColorCapability::Truecolor) {
        let accents = visible_accent_ids(habitat, now);
        for (index, id) in accents.iter().enumerate() {
            if let Some(cell) =
                render_accent(id, index, scene.habitat, &occupied, species, seed, now)
            {
                cells.push(cell);
            }
        }
    }

    cells
}

pub(crate) fn visible_trophy_ids(habitat: &HabitatView) -> Vec<&str> {
    let mut props = habitat
        .earned_props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Trophy)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| {
        b.display_priority
            .cmp(&a.display_priority)
            .then_with(|| a.earned_at.cmp(&b.earned_at))
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });
    props
        .into_iter()
        .take(MAX_TROPHIES)
        .map(|prop| prop.id.as_str())
        .collect()
}

pub(crate) fn visible_accent_ids(
    habitat: &HabitatView,
    now: time::OffsetDateTime,
) -> Vec<&str> {
    let mut props = habitat
        .earned_props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Accent)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| {
        a.earned_at
            .cmp(&b.earned_at)
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });

    if props.len() <= MAX_ACCENTS {
        return props.into_iter().map(|prop| prop.id.as_str()).collect();
    }

    let start = ((now.unix_timestamp() / ACCENT_ROTATION_SECS).rem_euclid(props.len() as i64)) as usize;
    (0..MAX_ACCENTS)
        .map(|offset| props[(start + offset) % props.len()].id.as_str())
        .collect()
}

fn trophy_anchor(id: &str, habitat: Rect) -> Option<Position> {
    if habitat.width < 8 || habitat.height < 4 {
        return None;
    }
    let bottom = habitat.y + habitat.height.saturating_sub(2);
    match id {
        "wilt_recovery_sprout" => Some(Position::new(habitat.x + 2, bottom.saturating_sub(2))),
        "heavy_session_planter" => Some(Position::new(habitat.x + habitat.width.saturating_sub(8), bottom.saturating_sub(2))),
        "codex_signal_lamp" => Some(Position::new(habitat.x + habitat.width.saturating_sub(5), habitat.y + 2)),
        _ => Some(Position::new(habitat.x + 3, bottom.saturating_sub(2))),
    }
}

fn trophy_sprite(id: &str, _species: Species, now: time::OffsetDateTime) -> &'static [SpriteCell] {
    let phase = now.unix_timestamp().rem_euclid(8);
    match id {
        "codex_signal_lamp" if phase < 4 => &[
            SpriteCell { dx: 0, dy: 0, glyph: '╷' },
            SpriteCell { dx: 0, dy: 1, glyph: '◉' },
            SpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        "codex_signal_lamp" => &[
            SpriteCell { dx: 0, dy: 0, glyph: '╷' },
            SpriteCell { dx: 0, dy: 1, glyph: '○' },
            SpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        "heavy_session_planter" => &[
            SpriteCell { dx: 1, dy: 0, glyph: 'ѱ' },
            SpriteCell { dx: 0, dy: 1, glyph: '╲' },
            SpriteCell { dx: 1, dy: 1, glyph: '┃' },
            SpriteCell { dx: 2, dy: 1, glyph: '╱' },
            SpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        "wilt_recovery_sprout" => &[
            SpriteCell { dx: 1, dy: 0, glyph: '╿' },
            SpriteCell { dx: 0, dy: 1, glyph: '╲' },
            SpriteCell { dx: 1, dy: 1, glyph: '┃' },
            SpriteCell { dx: 2, dy: 1, glyph: '╱' },
        ],
        _ => &[
            SpriteCell { dx: 0, dy: 0, glyph: '◈' },
            SpriteCell { dx: 1, dy: 1, glyph: '▝' },
        ],
    }
}

fn render_sprite(
    anchor: Position,
    sprite: &'static [SpriteCell],
    habitat: Rect,
    exclusions: &[Rect],
    style: Style,
) -> Vec<HabitatPropCell> {
    let mut cells = Vec::new();
    for cell in sprite {
        let col = (i32::from(anchor.x) + i32::from(cell.dx)) as u16;
        let row = (i32::from(anchor.y) + i32::from(cell.dy)) as u16;
        let pos = Position::new(col, row);
        if habitat.contains(pos) && !exclusions.iter().any(|rect| rect.contains(pos)) {
            cells.push(HabitatPropCell {
                row,
                col,
                glyph: cell.glyph,
                style,
            });
        }
    }
    cells
}

fn render_accent(
    id: &str,
    index: usize,
    habitat: Rect,
    exclusions: &[Rect],
    _species: Species,
    seed: &str,
    now: time::OffsetDateTime,
) -> Option<HabitatPropCell> {
    if habitat.width < 4 || habitat.height < 3 {
        return None;
    }

    let glyph = accent_glyph(id, now);
    let phase = seed.bytes().fold(index as u16, |acc, b| acc.wrapping_add(u16::from(b)));
    let col = habitat.x + 2 + ((phase + index as u16 * 7) % habitat.width.saturating_sub(3));
    let row_span = habitat.height.saturating_sub(2);
    let row = habitat.y + 1 + ((phase / 3 + index as u16 * 2) % row_span);
    let pos = Position::new(col, row);
    if exclusions.iter().any(|rect| rect.contains(pos)) {
        return None;
    }

    Some(HabitatPropCell {
        row,
        col,
        glyph,
        style: accent_style(),
    })
}

fn accent_glyph(id: &str, now: time::OffsetDateTime) -> char {
    let twinkle = now.unix_timestamp().rem_euclid(12) < 2;
    match id {
        "token_pebble_25k" => '▲',
        "token_shell_100k" => '◌',
        "token_spark_500k" if twinkle => '✦',
        "token_spark_500k" => '·',
        "token_shard_1m" => '◆',
        "token_orbit_5m" => '°',
        "token_lantern_10m" if twinkle => '☼',
        "token_lantern_10m" => '○',
        _ => '·',
    }
}

#[cfg(test)]
fn prop_visual_glyphs_for_test() -> &'static [char] {
    &[
        '╷', '◉', '○', '╵', 'ѱ', '╲', '┃', '╱', '◌', '╿', '◈', '▝', '▲', '✦',
        '·', '◆', '°', '☼',
    ]
}

fn trophy_style(color_capability: ColorCapability) -> Style {
    match color_capability {
        ColorCapability::Truecolor => Style::default().fg(tokenpet_palette().accent.rgb),
        ColorCapability::Flat => Style::default(),
    }
}

fn accent_style() -> Style {
    Style::default().fg(tokenpet_palette().dim.rgb)
}

fn bounds_for_cells(cells: &[HabitatPropCell]) -> Rect {
    let min_x = cells.iter().map(|cell| cell.col).min().unwrap_or(0);
    let max_x = cells.iter().map(|cell| cell.col).max().unwrap_or(min_x);
    let min_y = cells.iter().map(|cell| cell.row).min().unwrap_or(0);
    let max_y = cells.iter().map(|cell| cell.row).max().unwrap_or(min_y);
    Rect::new(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
}
```

Add `pub mod habitat_props;` and `pub use habitat_props::{habitat_props_for, HabitatPropCell};` to `src/tui/component/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --lib tui::component::habitat_props -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/component/habitat_props.rs src/tui/component/mod.rs
git commit -m "feat(tui): derive habitat prop placement"
```

---

### Task 5: Render Habitat Props In The Pet Panel

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/tui/view_model.rs`
- Test: `tests/tui_render.rs`

- [ ] **Step 1: Add a fixture helper and failing render test**

In `src/tui/view_model.rs`, add this fixture helper after `fixture_with_events()`:

```rust
    #[doc(hidden)]
    pub fn fixture_with_habitat_props() -> Self {
        let mut vm = Self::fixture();
        vm.habitat.earned_props = vec![
            EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("codex_signal_lamp"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Trophy,
                display_priority: 70,
            },
            EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("token_pebble_25k"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Accent,
                display_priority: 10,
            },
        ];
        vm
    }
```

Append this test to `tests/tui_render.rs`:

```rust
#[test]
fn pet_panel_renders_habitat_props_behind_pet_art() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    terminal
        .draw(|frame| render_watch_frame_with_capability(frame, &vm, ColorCapability::Truecolor))
        .unwrap();

    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains('◉') || text.contains('○'),
        "codex_signal_lamp should render in pet habitat:\n{text}"
    );
    assert!(
        text.contains('▲'),
        "token_pebble_25k should render in pet habitat:\n{text}"
    );
    assert!(text.contains("/\\_/\\") || text.contains("> ^ <"));
}

#[test]
fn pet_panel_draw_order_keeps_pet_above_habitat_props() {
    let source = std::fs::read_to_string("src/tui/panels/pet.rs").unwrap();
    let ambient = source.find("ambient_glyphs_for(").expect("ambient pass");
    let props = source.find("habitat_props_for(").expect("prop pass");
    let pet = source
        .find("render_pet_inside(buf, vm, &scene, now)")
        .expect("pet render pass");

    assert!(ambient < props, "ambient must render before props");
    assert!(props < pet, "props must render before pet art");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test tui_render pet_panel_renders_habitat_props_behind_pet_art -- --nocapture
```

Expected: FAIL because `PetPanel` does not call `habitat_props_for`.

- [ ] **Step 3: Render prop cells between ambient texture and pet art**

In `src/tui/panels/pet.rs`, import `habitat_props_for`:

```rust
use crate::tui::component::{habitat_props_for, PetScene, PetSceneLayout};
```

After the ambient glyph loop and before `render_pet_inside(buf, vm, &scene, now);`, add:

```rust
        for prop in habitat_props_for(
            &vm.habitat,
            &scene,
            species,
            &vm.pet_render.seed,
            ctx,
        ) {
            if prop.col >= scene.habitat.x
                && prop.row >= scene.habitat.y
                && prop.col < scene.habitat.x.saturating_add(scene.habitat.width)
                && prop.row < scene.habitat.y.saturating_add(scene.habitat.height)
            {
                let cell = &mut buf[(prop.col, prop.row)];
                cell.set_char(prop.glyph);
                cell.set_style(prop.style);
            }
        }
```

Keep `render_pet_inside` last so pet art overwrites prop cells.

- [ ] **Step 4: Run render tests**

Run:

```bash
cargo test --test tui_render pet_panel_renders_habitat_props_behind_pet_art -- --nocapture
cargo test --test tui_render -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs src/tui/view_model.rs tests/tui_render.rs
git commit -m "feat(watch): render habitat props in pet scene"
```

---

### Task 6: Add Preview Lab Prop Fixtures And Manifest Inputs

**Files:**
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing dev-preview test**

Append this test to `tests/dev_preview.rs`:

```rust
#[test]
fn dev_preview_watch_manifest_lists_habitat_prop_fixture_ids() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    let wide = scenario(&manifest, "watch-wide-normal");
    let prop_ids = wide["inputs"]["habitat_props"]
        .as_array()
        .expect("habitat_props input should be an array")
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();

    assert!(prop_ids.contains(&"codex_signal_lamp"));
    assert!(prop_ids.contains(&"heavy_session_planter"));
    assert!(prop_ids.contains(&"token_pebble_25k"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test dev_preview --features dev-preview dev_preview_watch_manifest_lists_habitat_prop_fixture_ids -- --nocapture
```

Expected: FAIL because manifest inputs do not include `habitat_props`.

- [ ] **Step 3: Seed preview habitat props**

In `src/dev_preview/watch.rs`, import habitat state types:

```rust
use crate::storage::state::{
    EarnedHabitatProp, HabitatPropId, HabitatPropSource, NarrativeEvent, PetState, Vitals,
};
```

In `seeded_pet_state`, after `state.recent_events = vec![...]`, add:

```rust
    state.habitat.earned_props = vec![
        EarnedHabitatProp {
            id: HabitatPropId::new("codex_signal_lamp"),
            earned_at: ctx.fixed_now - Duration::days(12),
            source: HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("heavy_session_planter"),
            earned_at: ctx.fixed_now - Duration::days(6),
            source: HabitatPropSource::HeavySession,
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_pebble_25k"),
            earned_at: ctx.fixed_now - Duration::days(10),
            source: HabitatPropSource::LifetimeTokens { threshold: 25_000.0 },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_shell_100k"),
            earned_at: ctx.fixed_now - Duration::days(4),
            source: HabitatPropSource::LifetimeTokens { threshold: 100_000.0 },
        },
    ];
```

Also change the preview lifetime counter from `52_000.0` to `125_000.0` so the
hand-seeded ladder props match the fixture's durable token counter:

```rust
    state.lifetime_effective_tokens = 125_000.0;
```

- [ ] **Step 4: Add manifest input metadata**

In `src/dev_preview/scenarios.rs`, add this helper:

```rust
fn preview_habitat_props() -> Value {
    json!([
        "codex_signal_lamp",
        "heavy_session_planter",
        "token_pebble_25k",
        "token_shell_100k"
    ])
}
```

For each watch scenario input map (`watch-wide-normal`, `watch-tall-wide`, and `watch-compact-normal`), add:

```rust
                ("habitat_props".to_string(), preview_habitat_props()),
```

- [ ] **Step 5: Run preview tests**

Run:

```bash
cargo test --test dev_preview --features dev-preview dev_preview_watch_manifest_lists_habitat_prop_fixture_ids -- --nocapture
cargo test --test dev_preview --features dev-preview -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Generate visual preview for review**

Run:

```bash
cargo run --features dev-preview -- dev-preview --scenario watch --out target/glorp-preview
```

Expected: command exits 0 and prints `target/glorp-preview`. Inspect:

```bash
sed -n '1,120p' target/glorp-preview/frames/watch-wide-normal.txt
sed -n '1,120p' target/glorp-preview/frames/watch-compact-normal.txt
```

Expected: wide frame includes visible habitat prop glyphs such as `◉`/`○`, `ѱ`, `◌`, or `▲`; compact remains readable and does not show prop glyphs over pet art.

- [ ] **Step 7: Commit**

```bash
git add src/dev_preview/watch.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "feat(dev-preview): show habitat prop fixtures"
```

---

### Task 7: Full Verification And Cleanup

**Files:**
- Modify only the files touched by earlier tasks when verification exposes a
  concrete formatting or correctness failure.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS. If it fails, run `cargo fmt`, inspect the diff, and include the formatting in the final verification commit.

- [ ] **Step 2: Run focused test suites**

Run:

```bash
cargo test --test runtime_integration -- --nocapture
cargo test --test watch_integration -- --nocapture
cargo test --test tui_render -- --nocapture
cargo test --test dev_preview --features dev-preview -- --nocapture
```

Expected: PASS for all four commands.

- [ ] **Step 3: Run broad Rust verification**

Run:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS for both commands.

- [ ] **Step 4: Run Preview Lab bundle**

Run:

```bash
cargo run --features dev-preview -- dev-preview --scenario all --out target/glorp-preview
```

Expected: PASS. Confirm these files exist:

```bash
test -f target/glorp-preview/manifest.json
test -f target/glorp-preview/frames/watch-wide-normal.txt
test -f target/glorp-preview/frames/watch-compact-normal.txt
test -f target/glorp-preview/frames/watch-wide-normal.cells.json
```

- [ ] **Step 5: Inspect git diff**

Run:

```bash
git status --short
git diff --stat
git diff -- src/game/habitat.rs src/game/runtime.rs src/tui/component/habitat_props.rs src/tui/panels/pet.rs
```

Expected: only habitat-props implementation files are changed; no unrelated rewrites.

- [ ] **Step 6: Final verification cleanup commit**

When verification caused formatting or small correctness edits, commit them:

```bash
git add src tests
git commit -m "chore(habitat): finish prop verification cleanup"
```

When there are no remaining unstaged changes, record that no cleanup commit was
needed in the implementation summary.

---

## Handoff Notes

- Keep the first version narrow. Do not add unlock narration, a shelf, a command, a setting, or a custom inventory view.
- Do not add dependencies for glyph width. Keep the V1 catalog to known single-column characters and pin that with tests.
- Do not persist coordinates or rotation state. Placement and motion are derived from clock and layout.
- If the Preview Lab frame feels too busy, reduce `MAX_ACCENTS` from 4 to 3 before changing the state model.
- If compact mode looks cramped, let trophy props disappear before shrinking the pet or moving ordinary panels.
