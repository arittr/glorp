# Glorp Ambient Tank Life Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add calendar-age-earned tank inhabitants and a deterministic daily visible cast so Glorp's watch and round companion tanks gain day-scale ecosystem change without token rewards, streaks, or customization UI.

**Architecture:** Keep durable facts in `PetState.habitat`, put age reconciliation in pure game/state helpers that run immediately after state load, derive catalog-backed view data in `WatchViewModel`, and add a shared `src/tui/component/tank_life.rs` renderer that performs canonical daily cast selection, surface projection, route motion, glyph validation, and depth-layered cell output. Integrate those cells into the existing pet scene draw list so watch, Preview Lab, and round companion reuse one catalog and one movement grammar.

**Tech Stack:** Rust, serde/serde_json, `time`, ratatui, `unicode-width`, Preview Lab, existing `LocalDayMapper`, existing `HabitatPetLayer`, existing `SceneDrawList`.

## Global Constraints

- Source design: `docs/superpowers/specs/2026-07-07-glorp-ambient-tank-life-design.md`.
- Do not create a branch unless Drew asks for one.
- Do not create Linear tickets for this Glorp work unless Drew explicitly asks.
- Do not touch unrelated dirty work. At plan time `src/companion/app.rs` and `src/round/hud.rs` are modified in the worktree; implementation agents must inspect and preserve those edits before touching either file.
- No schema bump. Existing `schema_version: 1` state JSON must deserialize through serde defaults.
- Age unlocks use calendar local-day difference from `PetState.created_at` to `now` through `LocalDayMapper`, not elapsed 24-hour duration and not existing display `age_days`.
- Reconciliation always scans the catalog and appends only missing qualified ids. Do not add a `reconciled_age_days_at` guard.
- Provider failures must not block age-earned inhabitants. Run reconciliation after state load and before usage-provider success is required.
- Activity may affect speed, brightness, and pause rhythm only. It must not affect earned inhabitants, canonical cast ids, rendered count, or inhabitant identity.
- Canonical daily cast is surface-independent and stable for a local date. Surface projection may cap or skip, but must not rerandomize.
- Round companion default budget is two moving slots. It may not add a literal floor or substrate.
- HUD, perimeter gauges, pet face, and speech must remain protected above tank-life cells.
- Every rendered glyph cell must be one Unicode scalar and terminal display width 1 under `unicode_width::UnicodeWidthChar`.
- Preview Lab must use shipped target surfaces (`Watch`, `Round`, `Menubar`) rather than a preview-only surface.
- Commit only if Drew asks. If committing later, stage only the files touched for that implementation step.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/storage/state.rs` | Add `TankInhabitantId`, `EarnedTankInhabitant`, `TankInhabitantSource`, and `HabitatState.earned_inhabitants`. |
| `src/game/habitat.rs` | Add tank-life catalog, calendar-age helper, catalog lookup, and age reconciliation. |
| `src/storage/day_axis.rs` | Reuse `LocalDayMapper`; do not duplicate local-date logic unless tests expose a missing UTC fallback. |
| `src/commands/watch.rs` | Reconcile state after load, persist newly earned inhabitants, and pass tank-life view data/local date into `WatchViewModel`. |
| `src/companion/app.rs` | Ensure companion-owned state load/poll paths use the same reconciliation; preserve existing dirty edits. |
| `src/tui/view_model.rs` | Extend `HabitatView` with earned inhabitant views, `tank_life_local_date`, and `tank_life_calendar_age_days`. |
| `src/tui/component/tank_life.rs` | New shared tank-life cast, projection, sprite, route, layer, and validation module. |
| `src/tui/component/mod.rs` | Export tank-life helpers needed by pet scene, preview, and tests. |
| `src/tui/panels/pet.rs` | Declare the `pet/tank_life.rs` helper module. |
| `src/tui/panels/pet/tank_life.rs` | Convert `TankLifeCell` values into `DrawCell` values while preserving style and layer filtering. |
| `src/tui/panels/pet/draw.rs` | Insert tank-life render passes in the approved draw order. |
| `src/round/scene.rs` | Build round-safe `TankLifeSurfaceGeometry` and pass it through the shared scene path. |
| `src/dev_preview/tank_life.rs` | Deterministic tank-life preview fixtures, surface geometries, and artifact helpers. |
| `src/dev_preview/mod.rs` | Expose the new preview module. |
| `src/dev_preview/scenarios.rs` | Add `PreviewSelection::TankLife`, write `*.tank-life.json`, and attach artifacts to relevant frames. |
| `src/dev_preview/export.rs` | Add scenario kind `TankLife`, artifact type `TankLife`, `files.tank_life`, and bump manifest schema from `6` to `7`. |
| `src/dev_preview/contract.rs` | Add typed `PreviewTankLifeArtifact` structs. |
| `src/dev_preview/watch.rs` | Add watch tank-life fixtures for age progression and daily cast rotation. |
| `src/dev_preview/round.rs` | Add round tank-life fixtures proving projection, no-go regions, and foreground safety. |
| `src/cli.rs` / `src/commands/dev_preview.rs` | Accept hidden `--scenario tank-life`. |
| `tests/dev_preview.rs` | Assert tank-life preview artifacts, schema version, scenario coverage, and artifact fields. |
| `tests/watch_integration.rs` | Prove reconciliation persists before provider success and view model filters unknown ids. |
| `tests/round_scene.rs` | Prove round tank-life stays inside aperture/no-go geometry and avoids protected face/HUD regions. |

## Task 1: Add State, Catalog, And Calendar-Age Unlocks

**Spec sections:** State Model, Runtime Data Flow, Catalog V1, Error Handling And Fallback.

**Files:**
- Modify: `src/storage/state.rs`
- Modify: `src/game/habitat.rs`
- Test: `src/game/habitat.rs`
- Test: `src/storage/state.rs`

**Interfaces:**
- Produces: `HabitatState.earned_inhabitants: Vec<EarnedTankInhabitant>`
- Produces: `TankInhabitantId`, `EarnedTankInhabitant`, `TankInhabitantSource`
- Produces: `calendar_age_days(created_at, now, mapper) -> i64`
- Produces: `reconcile_age_earned_inhabitants(state, now, mapper) -> bool`
- Produces: `tank_inhabitant_spec(id) -> Option<&'static TankInhabitantSpec>`

- [ ] **Step 1: Write failing calendar-age and migration tests**

Add tests in `src/game/habitat.rs` under the existing `#[cfg(test)]` module:

```rust
#[test]
fn calendar_age_days_uses_local_dates_not_elapsed_hours() {
    use crate::storage::day_axis::LocalDayMapper;
    use time::{macros::datetime, UtcOffset};

    let mapper = LocalDayMapper::Fixed(UtcOffset::from_hms(-8, 0, 0).unwrap());
    let created = datetime!(2026-07-07 07:30 UTC); // 2026-07-06 local
    let now = datetime!(2026-07-08 07:00 UTC);     // 2026-07-07 local, less than 24h later

    assert_eq!(calendar_age_days(created, now, &mapper), 1);
}

#[test]
fn calendar_age_days_clamps_future_created_at_to_zero() {
    use crate::storage::day_axis::LocalDayMapper;
    use time::{macros::datetime, UtcOffset};

    let mapper = LocalDayMapper::Fixed(UtcOffset::UTC);

    assert_eq!(
        calendar_age_days(
            datetime!(2026-07-09 00:00 UTC),
            datetime!(2026-07-08 00:00 UTC),
            &mapper,
        ),
        0,
    );
}

#[test]
fn age_reconciliation_backfills_catalog_order_and_is_idempotent() {
    use crate::storage::day_axis::LocalDayMapper;
    use crate::storage::state::PetState;
    use time::{macros::datetime, UtcOffset};

    let mapper = LocalDayMapper::Fixed(UtcOffset::UTC);
    let mut state = PetState::new_for_test("tank-life-seed", "Glorp");
    state.created_at = datetime!(2026-06-01 00:00 UTC);
    let now = datetime!(2026-07-02 00:00 UTC);

    assert!(reconcile_age_earned_inhabitants(&mut state, now, &mapper));
    assert_eq!(
        state
            .habitat
            .earned_inhabitants
            .iter()
            .map(|earned| earned.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "glass_shrimp",
            "needlefish",
            "glass_snail",
            "burrower",
            "rim_skimmer",
            "sand_ray",
            "schoollet",
        ],
    );
    assert_eq!(
        state.habitat.earned_inhabitants[0].source,
        crate::storage::state::TankInhabitantSource::PetAgeThreshold { threshold_days: 1 },
    );

    assert!(!reconcile_age_earned_inhabitants(&mut state, now, &mapper));
    assert_eq!(state.habitat.earned_inhabitants.len(), 7);
}
```

Add a serde-default migration test in `src/storage/state.rs`:

```rust
#[test]
fn habitat_state_deserializes_without_earned_inhabitants() {
    let json = r#"{
      "earned_props": [],
      "reconciled_lifetime_tokens_at": null
    }"#;

    let habitat: HabitatState = serde_json::from_str(json).unwrap();

    assert!(habitat.earned_props.is_empty());
    assert!(habitat.earned_inhabitants.is_empty());
}
```

- [ ] **Step 2: Run tests and confirm expected failures**

Run:

```bash
cargo test game::habitat::tests::calendar_age_days_uses_local_dates_not_elapsed_hours
cargo test game::habitat::tests::age_reconciliation_backfills_catalog_order_and_is_idempotent
cargo test storage::state::tests::habitat_state_deserializes_without_earned_inhabitants
```

Expected: compilation fails because the tank inhabitant types and helpers do not exist.

- [ ] **Step 3: Add durable inhabitant state types**

In `src/storage/state.rs`, place the tank inhabitant id beside `HabitatPropId` and add the earned fact types beside `EarnedHabitatProp`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TankInhabitantId(String);

impl TankInhabitantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for TankInhabitantId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarnedTankInhabitant {
    pub id: TankInhabitantId,
    pub earned_at: OffsetDateTime,
    pub source: TankInhabitantSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TankInhabitantSource {
    PetAgeThreshold { threshold_days: i64 },
}
```

Extend `HabitatState` without changing schema version:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HabitatState {
    pub earned_props: Vec<EarnedHabitatProp>,
    pub reconciled_lifetime_tokens_at: Option<f64>,
    #[serde(default)]
    pub earned_inhabitants: Vec<EarnedTankInhabitant>,
}
```

- [ ] **Step 4: Add the V1 catalog**

In `src/game/habitat.rs`, update imports to include the new state types:

```rust
use crate::storage::state::{
    EarnedHabitatProp, EarnedTankInhabitant, HabitatPropSource, PetState, TankInhabitantId,
    TankInhabitantSource,
};
```

Add these catalog-facing types below `HabitatPropKind`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankInhabitantKind {
    Swimmer,
    LowerLane,
    Glass,
    Rim,
    LowerEdge,
    HostCombo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankLifeRouteFamily {
    CrossTankSwimmer,
    LowerLaneResident,
    GlassResident,
    RimResident,
    LowerEdgeResident,
    HostCombo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TankInhabitantSpec {
    pub id: &'static str,
    pub unlock_age_days: i64,
    pub kind: TankInhabitantKind,
    pub route_family: TankLifeRouteFamily,
    pub natural_layer: HabitatPetLayer,
    pub low_color_key: &'static str,
}
```

Add the V1 ids and catalog in the exact order used for backfill:

```rust
pub const GLASS_SHRIMP: &str = "glass_shrimp";
pub const NEEDLEFISH: &str = "needlefish";
pub const GLASS_SNAIL: &str = "glass_snail";
pub const BURROWER: &str = "burrower";
pub const RIM_SKIMMER: &str = "rim_skimmer";
pub const SAND_RAY: &str = "sand_ray";
pub const SCHOOLLET: &str = "schoollet";
pub const ANEMONE_HOST: &str = "anemone_host";

pub const TANK_INHABITANT_CATALOG: &[TankInhabitantSpec] = &[
    TankInhabitantSpec {
        id: GLASS_SHRIMP,
        unlock_age_days: 1,
        kind: TankInhabitantKind::LowerLane,
        route_family: TankLifeRouteFamily::LowerLaneResident,
        natural_layer: HabitatPetLayer::Foreground,
        low_color_key: "shrimp",
    },
    TankInhabitantSpec {
        id: NEEDLEFISH,
        unlock_age_days: 3,
        kind: TankInhabitantKind::Swimmer,
        route_family: TankLifeRouteFamily::CrossTankSwimmer,
        natural_layer: HabitatPetLayer::Behind,
        low_color_key: "fish",
    },
    TankInhabitantSpec {
        id: GLASS_SNAIL,
        unlock_age_days: 7,
        kind: TankInhabitantKind::Glass,
        route_family: TankLifeRouteFamily::GlassResident,
        natural_layer: HabitatPetLayer::Foreground,
        low_color_key: "snail",
    },
    TankInhabitantSpec {
        id: BURROWER,
        unlock_age_days: 10,
        kind: TankInhabitantKind::LowerEdge,
        route_family: TankLifeRouteFamily::LowerEdgeResident,
        natural_layer: HabitatPetLayer::Foreground,
        low_color_key: "burrower",
    },
    TankInhabitantSpec {
        id: RIM_SKIMMER,
        unlock_age_days: 14,
        kind: TankInhabitantKind::Rim,
        route_family: TankLifeRouteFamily::RimResident,
        natural_layer: HabitatPetLayer::Behind,
        low_color_key: "rim",
    },
    TankInhabitantSpec {
        id: SAND_RAY,
        unlock_age_days: 21,
        kind: TankInhabitantKind::LowerLane,
        route_family: TankLifeRouteFamily::LowerLaneResident,
        natural_layer: HabitatPetLayer::Foreground,
        low_color_key: "ray",
    },
    TankInhabitantSpec {
        id: SCHOOLLET,
        unlock_age_days: 28,
        kind: TankInhabitantKind::Swimmer,
        route_family: TankLifeRouteFamily::CrossTankSwimmer,
        natural_layer: HabitatPetLayer::Behind,
        low_color_key: "school",
    },
    TankInhabitantSpec {
        id: ANEMONE_HOST,
        unlock_age_days: 35,
        kind: TankInhabitantKind::HostCombo,
        route_family: TankLifeRouteFamily::HostCombo,
        natural_layer: HabitatPetLayer::Behind,
        low_color_key: "host",
    },
];

pub fn tank_inhabitant_spec(id: &TankInhabitantId) -> Option<&'static TankInhabitantSpec> {
    TANK_INHABITANT_CATALOG
        .iter()
        .find(|spec| spec.id == id.as_str())
}
```

- [ ] **Step 5: Add calendar-age reconciliation**

Add the helper in `src/game/habitat.rs`:

```rust
pub fn calendar_age_days(
    created_at: OffsetDateTime,
    now: OffsetDateTime,
    local_day_mapper: &crate::storage::day_axis::LocalDayMapper,
) -> i64 {
    let created = local_day_mapper.local_date(created_at);
    let current = local_day_mapper.local_date(now);
    (current - created).whole_days().max(0)
}
```

Then add reconciliation:

```rust
pub fn reconcile_age_earned_inhabitants(
    state: &mut PetState,
    now: OffsetDateTime,
    local_day_mapper: &crate::storage::day_axis::LocalDayMapper,
) -> bool {
    let age_days = calendar_age_days(state.created_at, now, local_day_mapper);
    let mut changed = false;

    for spec in TANK_INHABITANT_CATALOG {
        if age_days < spec.unlock_age_days {
            continue;
        }
        if state
            .habitat
            .earned_inhabitants
            .iter()
            .any(|earned| earned.id.as_str() == spec.id)
        {
            continue;
        }
        state.habitat.earned_inhabitants.push(EarnedTankInhabitant {
            id: TankInhabitantId::new(spec.id),
            earned_at: now,
            source: TankInhabitantSource::PetAgeThreshold {
                threshold_days: spec.unlock_age_days,
            },
        });
        changed = true;
    }

    changed
}
```

- [ ] **Step 6: Verify Task 1**

Run:

```bash
cargo test game::habitat::tests::calendar_age_days_
cargo test game::habitat::tests::age_reconciliation_backfills_catalog_order_and_is_idempotent
cargo test storage::state::tests::habitat_state_deserializes_without_earned_inhabitants
```

Expected: all listed tests pass.

## Task 2: Wire Reconciliation Into Runtime And View Model

**Spec sections:** Runtime Data Flow, Watch View Model, Error Handling And Fallback.

**Files:**
- Modify: `src/commands/watch.rs`
- Modify: `src/companion/app.rs`
- Modify: `src/tui/view_model.rs`
- Test: `tests/watch_integration.rs`

**Interfaces:**
- Produces: `HabitatView.earned_inhabitants`
- Produces: `HabitatView.tank_life_local_date`
- Produces: `HabitatView.tank_life_calendar_age_days`
- Produces: `EarnedTankInhabitantView`
- Produces: `reconcile_state_after_load(store, state, now, mapper) -> Result<bool>`

Implementation note: the design's sample `canonical_daily_cast` signature does not carry age, but the target-count rules include "day 60 onward." Carry `tank_life_calendar_age_days` into the view model so the renderer can implement the day-60 rule without persisted current-cast state.

- [ ] **Step 1: Write failing integration tests**

Add tests in `tests/watch_integration.rs`:

```rust
#[test]
fn watch_view_model_exposes_known_earned_tank_inhabitants_and_local_date() {
    use glorp::storage::state::{
        EarnedTankInhabitant, TankInhabitantId, TankInhabitantSource,
    };
    use time::macros::datetime;

    let dir = tempfile::tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut usage = glorp::storage::usage_store::UsageStore::open(&usage_db).unwrap();
    seed_snapshot_for_test(
        &mut usage,
        glorp::usage::day_axis::tokenmaxxing_provider_day(now),
        "claude-code",
        100.0,
        now,
    );
    let mut state = mech_state();
    state.created_at = datetime!(2026-07-01 00:00 UTC);
    state.habitat.earned_inhabitants.push(EarnedTankInhabitant {
        id: TankInhabitantId::new("glass_shrimp"),
        earned_at: datetime!(2026-07-02 00:00 UTC),
        source: TankInhabitantSource::PetAgeThreshold { threshold_days: 1 },
    });
    state.habitat.earned_inhabitants.push(EarnedTankInhabitant {
        id: TankInhabitantId::new("future_friend"),
        earned_at: datetime!(2026-07-02 00:00 UTC),
        source: TankInhabitantSource::PetAgeThreshold { threshold_days: 99 },
    });

    let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

    assert_eq!(vm.habitat.tank_life_local_date, time::macros::date!(2026-07-08));
    assert_eq!(vm.habitat.tank_life_calendar_age_days, 7);
    assert_eq!(vm.habitat.earned_inhabitants.len(), 1);
    assert_eq!(vm.habitat.earned_inhabitants[0].id.as_str(), "glass_shrimp");
    assert_eq!(vm.habitat.earned_inhabitants[0].unlock_age_days, 1);
}

#[test]
fn state_load_reconciliation_persists_before_provider_success_is_required() {
    use glorp::storage::day_axis::LocalDayMapper;
    use time::{macros::datetime, UtcOffset};

    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("state.json");
    let usage_path = dir.path().join("usage.sqlite");
    let store = glorp::storage::state::StateStore::new(state_path.clone());
    let mut state = glorp::storage::state::PetState::new_for_test("age-seed", "Glorp");
    state.created_at = datetime!(2026-07-01 00:00 UTC);
    store.save(&state).unwrap();

    let result = poll_usage_and_apply_for_test_with_failing_provider(
        &store,
        &usage_path,
        datetime!(2026-07-08 00:00 UTC),
        LocalDayMapper::Fixed(UtcOffset::UTC),
    );

    assert!(result.is_err(), "the provider path should still fail in this fixture");
    let saved = store.load().unwrap().unwrap();
    assert!(
        saved
            .habitat
            .earned_inhabitants
            .iter()
            .any(|earned| earned.id.as_str() == "glass_shrimp"),
        "age reconciliation must persist before provider success is required",
    );
}
```

Add this public test-only wrapper in `src/commands/watch.rs` so the integration test does not reach into `pub(crate)` internals:

```rust
#[doc(hidden)]
pub fn poll_usage_and_apply_for_test_with_failing_provider(
    state_store: &crate::storage::state::StateStore,
    usage_db: &std::path::Path,
    now: time::OffsetDateTime,
    mapper: crate::storage::day_axis::LocalDayMapper,
) -> crate::error::Result<()> {
    // Reuse the real load -> reconcile -> save ordering, then return a deliberate
    // provider-shaped error before any usage apply succeeds.
    let Some(mut state) = state_store.load()? else {
        return Ok(());
    };
    reconcile_state_after_load(state_store, &mut state, now, mapper)?;
    let _ = crate::storage::usage_store::UsageStore::open(usage_db)?;
    Err(crate::error::GlorpError::Message(
        "test provider failure after age reconciliation".to_string(),
    ))
}
```

The wrapper must not become a production call path; it exists to prove provider failure cannot block age reconciliation.

- [ ] **Step 2: Run tests and confirm expected failures**

Run:

```bash
cargo test --test watch_integration tank_inhabitants -- --nocapture
cargo test --test watch_integration state_load_reconciliation_persists_before_provider_success_is_required -- --nocapture
```

Expected: compilation fails because `HabitatView` and reconciliation wiring are missing.

- [ ] **Step 3: Extend `HabitatView`**

In `src/tui/view_model.rs`, update imports:

```rust
use crate::game::habitat::{HabitatPropKind, TankInhabitantKind};
use crate::storage::state::{
    HabitatPropId, HabitatPropSource, TankInhabitantId, TankInhabitantSource,
};
```

Replace `HabitatView` and add the view type:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HabitatView {
    pub earned_props: Vec<EarnedHabitatPropView>,
    pub earned_inhabitants: Vec<EarnedTankInhabitantView>,
    pub tank_life_local_date: time::Date,
    pub tank_life_calendar_age_days: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EarnedTankInhabitantView {
    pub id: TankInhabitantId,
    pub earned_at: time::OffsetDateTime,
    pub unlock_age_days: i64,
    pub kind: TankInhabitantKind,
    pub source: TankInhabitantSource,
}
```

Update every `HabitatView { ... }` fixture in `src/tui/view_model.rs` to include:

```rust
earned_inhabitants: Vec::new(),
tank_life_local_date: time::Date::from_calendar_date(1970, time::Month::January, 1).unwrap(),
tank_life_calendar_age_days: 0,
```

Add a focused fixture for round and preview tests under `impl WatchViewModel`:

```rust
pub fn fixture_with_tank_inhabitants_for_age(age_days: i64, local_date: time::Date) -> Self {
    let mut vm = Self::fixture_with_habitat_props();
    vm.habitat.tank_life_local_date = local_date;
    vm.habitat.tank_life_calendar_age_days = age_days;
    vm.habitat.earned_inhabitants = crate::game::habitat::TANK_INHABITANT_CATALOG
        .iter()
        .filter(|spec| spec.unlock_age_days <= age_days)
        .map(|spec| EarnedTankInhabitantView {
            id: TankInhabitantId::new(spec.id),
            earned_at: time::OffsetDateTime::UNIX_EPOCH,
            unlock_age_days: spec.unlock_age_days,
            kind: spec.kind,
            source: TankInhabitantSource::PetAgeThreshold {
                threshold_days: spec.unlock_age_days,
            },
        })
        .collect();
    vm
}
```

- [ ] **Step 4: Build catalog-backed view data**

Change `build_habitat_view` in `src/commands/watch.rs` from a state-only helper to:

```rust
fn build_habitat_view(
    state: &crate::storage::state::PetState,
    now: time::OffsetDateTime,
    mapper: crate::storage::day_axis::LocalDayMapper,
) -> HabitatView
```

Map known inhabitants only:

```rust
let earned_inhabitants = state
    .habitat
    .earned_inhabitants
    .iter()
    .filter_map(|earned| {
        let spec = crate::game::habitat::tank_inhabitant_spec(&earned.id)?;
        Some(crate::tui::view_model::EarnedTankInhabitantView {
            id: earned.id.clone(),
            earned_at: earned.earned_at,
            unlock_age_days: spec.unlock_age_days,
            kind: spec.kind,
            source: earned.source.clone(),
        })
    })
    .collect();
```

Set the non-persisted time context:

```rust
HabitatView {
    earned_props,
    earned_inhabitants,
    tank_life_local_date: mapper.local_date(now),
    tank_life_calendar_age_days: crate::game::habitat::calendar_age_days(
        state.created_at,
        now,
        &mapper,
    ),
}
```

- [ ] **Step 5: Reconcile immediately after state load**

In `src/commands/watch.rs`, add:

```rust
fn reconcile_state_after_load(
    state_store: &crate::storage::state::StateStore,
    state: &mut crate::storage::state::PetState,
    now: time::OffsetDateTime,
    mapper: crate::storage::day_axis::LocalDayMapper,
) -> crate::error::Result<bool> {
    if crate::game::habitat::reconcile_age_earned_inhabitants(state, now, &mapper) {
        state.last_updated_at = now;
        state_store.save(state)?;
        return Ok(true);
    }
    Ok(false)
}
```

Call this helper immediately after `StateStore::load()` returns `Some(mut state)` in:

- `run_against_real_state`
- `poll_usage_and_apply`
- any watch test helper that intentionally mirrors the real state-load path

Use `LocalDayMapper::System` in real watch/companion runtime paths unless the existing call already carries a mapper. Preserve test paths that pass `LocalDayMapper::Fixed`.

- [ ] **Step 6: Wire companion-owned state load paths**

Before editing, run:

```bash
git diff -- src/companion/app.rs
```

Expected: review the existing dirty changes and preserve them.

If `src/companion/app.rs` calls `poll_usage_and_apply`, no extra state mutation is needed there beyond ensuring it uses the reconciled function. If it directly loads `PetState`, call `reconcile_state_after_load` in the same after-load position as watch. Do not restructure the AppKit drawing code in this task.

- [ ] **Step 7: Verify Task 2**

Run:

```bash
cargo test --test watch_integration tank_inhabitants -- --nocapture
cargo test --test watch_integration state_load_reconciliation_persists_before_provider_success_is_required -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_watch_writes_expected_artifacts
```

Expected: all listed tests pass and existing Preview Lab watch fixtures still build with the extended `HabitatView`.

## Task 3: Add Canonical Daily Cast And Surface Projection

**Spec sections:** Daily Cast Selection, Surface Projection, Anemone Host Morphs, Error Handling And Fallback.

**Files:**
- Create: `src/tui/component/tank_life.rs`
- Modify: `src/tui/component/mod.rs`
- Test: `src/tui/component/tank_life.rs`

**Interfaces:**
- Produces: `TankLifeSurface`
- Produces: `TankLifeSurfaceGeometry`
- Produces: `RoundApertureMask`
- Produces: `RenderedTankLifeCast`
- Produces: `TankLifeSkip`, `TankLifeSkipReason`
- Produces: `canonical_daily_cast`
- Produces: `project_tank_life_cast`
- Produces: `anemone_morph_for_day`

- [ ] **Step 1: Write failing cast and projection tests**

Create `src/tui/component/tank_life.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::state::TankInhabitantId;
    use crate::tui::view_model::EarnedTankInhabitantView;
    use time::macros::date;

    const SEED: &str = "glorp-tank-life-fixture";

    fn earned(age_days: i64) -> Vec<EarnedTankInhabitantView> {
        crate::game::habitat::TANK_INHABITANT_CATALOG
            .iter()
            .filter(|spec| spec.unlock_age_days <= age_days)
            .map(|spec| EarnedTankInhabitantView {
                id: TankInhabitantId::new(spec.id),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                unlock_age_days: spec.unlock_age_days,
                kind: spec.kind,
                source: crate::storage::state::TankInhabitantSource::PetAgeThreshold {
                    threshold_days: spec.unlock_age_days,
                },
            })
            .collect()
    }

    fn ids(ids: &[TankInhabitantId]) -> Vec<&str> {
        ids.iter().map(|id| id.as_str()).collect()
    }

    #[test]
    fn canonical_cast_has_exact_age_and_date_fixtures() {
        assert_eq!(ids(&canonical_daily_cast(&earned(0), SEED, date!(2026-07-07), 0)), Vec::<&str>::new());
        assert_eq!(ids(&canonical_daily_cast(&earned(1), SEED, date!(2026-07-07), 1)), vec!["glass_shrimp"]);
        assert_eq!(ids(&canonical_daily_cast(&earned(3), SEED, date!(2026-07-07), 3)), vec!["glass_shrimp", "needlefish"]);
        assert_eq!(ids(&canonical_daily_cast(&earned(7), SEED, date!(2026-07-07), 7)), vec!["glass_snail", "glass_shrimp", "needlefish"]);
        assert_eq!(ids(&canonical_daily_cast(&earned(21), SEED, date!(2026-07-07), 21)), vec!["rim_skimmer", "burrower", "glass_snail", "sand_ray"]);
        assert_eq!(ids(&canonical_daily_cast(&earned(60), SEED, date!(2026-07-07), 60)), vec!["anemone_host", "rim_skimmer", "burrower", "glass_snail"]);
        assert_eq!(ids(&canonical_daily_cast(&earned(60), SEED, date!(2026-07-08), 60)), vec!["rim_skimmer", "glass_shrimp", "sand_ray", "burrower", "anemone_host"]);
        assert_eq!(ids(&canonical_daily_cast(&earned(60), SEED, date!(2026-07-09), 60)), vec!["sand_ray", "schoollet", "glass_snail", "glass_shrimp"]);
    }

    #[test]
    fn anemone_morph_has_exact_date_fixtures() {
        assert_eq!(anemone_morph_for_day(SEED, date!(2026-07-07)), AnemoneMorph::DotColony);
        assert_eq!(anemone_morph_for_day(SEED, date!(2026-07-08)), AnemoneMorph::Crown);
        assert_eq!(anemone_morph_for_day(SEED, date!(2026-07-09)), AnemoneMorph::Comb);
    }

    #[test]
    fn round_projection_caps_without_rerandomizing() {
        let canonical = canonical_daily_cast(&earned(60), SEED, date!(2026-07-08), 60);
        let geometry = TankLifeSurfaceGeometry::round_for_test(44, 18, 2);

        let projected = project_tank_life_cast(&canonical, &geometry);

        assert_eq!(ids(&projected.canonical_ids), vec!["rim_skimmer", "glass_shrimp", "sand_ray", "burrower", "anemone_host"]);
        assert_eq!(ids(&projected.rendered_ids), vec!["rim_skimmer", "glass_shrimp"]);
        assert_eq!(projected.skipped.len(), 3);
        assert!(projected
            .skipped
            .iter()
            .all(|skip| skip.reason == TankLifeSkipReason::SurfaceBudget));
    }
}
```

- [ ] **Step 2: Run tests and confirm expected failures**

Run:

```bash
cargo test tui::component::tank_life::tests::canonical_cast_has_exact_age_and_date_fixtures
cargo test tui::component::tank_life::tests::round_projection_caps_without_rerandomizing
```

Expected: module does not compile until exported and implemented.

- [ ] **Step 3: Add module exports**

Modify `src/tui/component/mod.rs`:

```rust
pub mod tank_life;
```

Add exports needed by callers:

```rust
pub use tank_life::{
    anemone_morph_for_day, canonical_daily_cast, project_tank_life_cast, AnemoneMorph,
    RenderedTankLifeCast, RoundApertureMask, TankLifeCell, TankLifePlacement,
    TankLifeSkip, TankLifeSkipReason, TankLifeSurface, TankLifeSurfaceGeometry,
};
```

- [ ] **Step 4: Implement public projection types**

At the top of `src/tui/component/tank_life.rs`:

```rust
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::game::habitat::HabitatPetLayer;
use crate::storage::state::TankInhabitantId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankLifeSurface {
    Watch,
    Round,
    Menubar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundApertureMask {
    pub center_col: i16,
    pub center_row: i16,
    pub radius_cols: u16,
    pub radius_rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeSurfaceGeometry {
    pub surface: TankLifeSurface,
    pub habitat: Rect,
    pub aperture_mask: Option<RoundApertureMask>,
    pub reserved_regions: Vec<Rect>,
    pub max_moving_slots: usize,
    pub literal_floor_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedTankLifeCast {
    pub canonical_ids: Vec<TankInhabitantId>,
    pub rendered_ids: Vec<TankInhabitantId>,
    pub skipped: Vec<TankLifeSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeSkip {
    pub id: TankInhabitantId,
    pub reason: TankLifeSkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankLifeSkipReason {
    UnknownCatalogId,
    SurfaceBudget,
    HabitatTooSmall,
    ReservedRegionCollision,
    ApertureCollision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnemoneMorph {
    Flower,
    Comb,
    Crown,
    DotColony,
}
```

Add the test constructor behind `#[cfg(test)]`:

```rust
impl TankLifeSurfaceGeometry {
    #[cfg(test)]
    pub fn round_for_test(cols: u16, rows: u16, max_moving_slots: usize) -> Self {
        Self {
            surface: TankLifeSurface::Round,
            habitat: Rect::new(0, 0, cols, rows),
            aperture_mask: Some(RoundApertureMask {
                center_col: (cols / 2) as i16,
                center_row: (rows / 2) as i16,
                radius_cols: cols / 2,
                radius_rows: rows / 2,
            }),
            reserved_regions: vec![Rect::new(0, rows.saturating_sub(4), cols, 4)],
            max_moving_slots,
            literal_floor_allowed: false,
        }
    }
}
```

- [ ] **Step 5: Implement stable hash and canonical selection**

Add a local stable FNV-1a helper. Do not use `DefaultHasher`, because its output is not a fixture contract.

```rust
fn stable_hash(input: &str) -> u64 {
    const OFFSET: u64 = 1_469_598_103_934_665_603;
    const PRIME: u64 = 1_099_511_628_211;

    input.bytes().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
        hash
    })
}
```

Implement:

```rust
pub fn canonical_daily_cast(
    unlocked: &[crate::tui::view_model::EarnedTankInhabitantView],
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> Vec<TankInhabitantId> {
    let mut known = unlocked
        .iter()
        .filter(|earned| crate::game::habitat::tank_inhabitant_spec(&earned.id).is_some())
        .collect::<Vec<_>>();
    known.sort_by(|a, b| a.id.cmp(&b.id));

    let target = canonical_target_count(known.len(), pet_seed, local_date, calendar_age_days);
    if target == 0 {
        return Vec::new();
    }

    let mut scored = known
        .into_iter()
        .map(|earned| {
            let score = stable_hash(&format!(
                "tank-life-cast-v1|{pet_seed}|{local_date}|{}",
                earned.id.as_str()
            ));
            (score, earned.id.clone())
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, id)| (*score, id.clone()));
    scored
        .into_iter()
        .take(target.min(5))
        .map(|(_, id)| id)
        .collect()
}

fn canonical_target_count(
    unlocked_len: usize,
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> usize {
    if unlocked_len <= 2 {
        return unlocked_len;
    }
    let flip = (stable_hash(&format!(
        "tank-life-target-v1|{pet_seed}|{local_date}|{calendar_age_days}"
    )) & 1) as usize;
    let target = if calendar_age_days < 21 {
        2 + flip
    } else if calendar_age_days < 60 {
        3 + flip
    } else {
        4 + flip
    };
    target.min(unlocked_len).min(5)
}
```

Implement morph selection:

```rust
pub fn anemone_morph_for_day(pet_seed: &str, local_date: time::Date) -> AnemoneMorph {
    match stable_hash(&format!("anemone-morph-v1|{pet_seed}|{local_date}")) % 4 {
        0 => AnemoneMorph::Flower,
        1 => AnemoneMorph::Comb,
        2 => AnemoneMorph::Crown,
        _ => AnemoneMorph::DotColony,
    }
}
```

- [ ] **Step 6: Implement projection**

Start with catalog existence, footprint, and budget checks. Do not place route cells in this task yet.

```rust
pub fn project_tank_life_cast(
    canonical_ids: &[TankInhabitantId],
    geometry: &TankLifeSurfaceGeometry,
) -> RenderedTankLifeCast {
    let mut rendered_ids = Vec::new();
    let mut skipped = Vec::new();

    for id in canonical_ids {
        let Some(spec) = crate::game::habitat::tank_inhabitant_spec(id) else {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::UnknownCatalogId,
            });
            continue;
        };
        if rendered_ids.len() >= geometry.max_moving_slots {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::SurfaceBudget,
            });
            continue;
        }
        if !footprint_can_fit(spec.id, geometry) {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::HabitatTooSmall,
            });
            continue;
        }
        rendered_ids.push(id.clone());
    }

    RenderedTankLifeCast {
        canonical_ids: canonical_ids.to_vec(),
        rendered_ids,
        skipped,
    }
}

fn footprint_can_fit(id: &str, geometry: &TankLifeSurfaceGeometry) -> bool {
    let min = match id {
        crate::game::habitat::ANEMONE_HOST => (8, 6),
        crate::game::habitat::SCHOOLLET => (8, 3),
        _ => (4, 3),
    };
    geometry.habitat.width >= min.0 && geometry.habitat.height >= min.1
}
```

- [ ] **Step 7: Verify Task 3**

Run:

```bash
cargo test tui::component::tank_life::tests::canonical_cast_has_exact_age_and_date_fixtures
cargo test tui::component::tank_life::tests::anemone_morph_has_exact_date_fixtures
cargo test tui::component::tank_life::tests::round_projection_caps_without_rerandomizing
```

Expected: all listed tests pass.

## Task 4: Add Sprite Cells, Routes, And Layer Segments

**Spec sections:** Rendering Model, Route Grammar, Motion And Activity, Catalog V1.

**Files:**
- Modify: `src/tui/component/tank_life.rs`
- Test: `src/tui/component/tank_life.rs`

**Interfaces:**
- Produces: `SpriteCell`
- Produces: `TankLifeCell`
- Produces: `TankLifePlacement`
- Produces: `TankLifeLayerSegmentSummary`
- Produces: `tank_life_placements_for`
- Produces: `validate_tank_life_catalog`

- [ ] **Step 1: Write failing sprite/route tests**

Append tests in `src/tui/component/tank_life.rs`:

```rust
#[test]
fn catalog_glyphs_are_single_width_cells() {
    validate_tank_life_catalog().unwrap();
}

#[test]
fn anemone_host_morphs_share_host_behavior_but_unique_anchor_cells() {
    let morphs = [
        AnemoneMorph::Flower,
        AnemoneMorph::Comb,
        AnemoneMorph::Crown,
        AnemoneMorph::DotColony,
    ];
    let anchors = morphs
        .into_iter()
        .map(anemone_anchor_sprite)
        .collect::<Vec<_>>();

    assert!(anchors.windows(2).all(|pair| pair[0] != pair[1]));
    assert_eq!(host_fish_sprite(), vec![SpriteCell { row: 0, col: 0, glyph: '›' }, SpriteCell { row: 0, col: 1, glyph: '·' }]);
}

#[test]
fn route_dependent_swimmer_has_behind_and_foreground_segments() {
    let geometry = TankLifeSurfaceGeometry::round_for_test(44, 18, 3);
    let placements = tank_life_placements_for(&TankLifeRenderInput::for_test(
        vec![TankInhabitantId::new(crate::game::habitat::NEEDLEFISH)],
        &geometry,
        time::macros::date!(2026-07-08),
        1_800,
    ));

    let layers = placements
        .iter()
        .flat_map(|placement| placement.cells.iter().map(|cell| cell.pet_layer))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(layers.contains(&crate::game::habitat::HabitatPetLayer::Behind));
    assert!(layers.contains(&crate::game::habitat::HabitatPetLayer::Foreground));
}

#[test]
fn round_routes_avoid_reserved_regions() {
    let geometry = TankLifeSurfaceGeometry::round_for_test(44, 18, 3);
    let input = TankLifeRenderInput::for_test(
        vec![
            TankInhabitantId::new(crate::game::habitat::GLASS_SHRIMP),
            TankInhabitantId::new(crate::game::habitat::RIM_SKIMMER),
            TankInhabitantId::new(crate::game::habitat::ANEMONE_HOST),
        ],
        &geometry,
        time::macros::date!(2026-07-08),
        2_400,
    );

    for placement in tank_life_placements_for(&input) {
        for cell in placement.cells {
            assert!(!input.geometry.reserved_regions.iter().any(|region| rect_contains(*region, cell.col, cell.row)));
            assert!(input.geometry.cell_inside_aperture(cell.col, cell.row));
        }
    }
}
```

- [ ] **Step 2: Run tests and confirm expected failures**

Run:

```bash
cargo test tui::component::tank_life::tests::catalog_glyphs_are_single_width_cells
cargo test tui::component::tank_life::tests::route_dependent_swimmer_has_behind_and_foreground_segments
cargo test tui::component::tank_life::tests::round_routes_avoid_reserved_regions
```

Expected: compilation fails because sprite and route output are not implemented.

- [ ] **Step 3: Add cell/sprite output types**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteCell {
    pub row: i16,
    pub col: i16,
    pub glyph: char,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TankLifeCell {
    pub inhabitant_id: TankInhabitantId,
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
    pub pet_layer: HabitatPetLayer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TankLifePlacement {
    pub inhabitant_id: TankInhabitantId,
    pub cells: Vec<TankLifeCell>,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeLayerSegmentSummary {
    pub inhabitant_id: TankInhabitantId,
    pub pet_layer: HabitatPetLayer,
    pub cell_count: usize,
}

#[derive(Debug, Clone)]
pub struct TankLifeRenderInput<'a> {
    pub rendered_ids: Vec<TankInhabitantId>,
    pub pet_seed: &'a str,
    pub local_date: time::Date,
    pub now: time::OffsetDateTime,
    pub geometry: &'a TankLifeSurfaceGeometry,
    pub pet_protected_regions: &'a [Rect],
    pub color_capability: crate::tui::style::ColorCapability,
    pub life_profile: crate::tui::life::PetLifeProfile,
}
```

- [ ] **Step 4: Add explicit sprites**

Implement each glyph family as `Vec<SpriteCell>`, not strings:

```rust
fn sprite_for(id: &str, phase: u64, morph: Option<AnemoneMorph>) -> Vec<SpriteCell> {
    match id {
        crate::game::habitat::GLASS_SHRIMP => {
            if phase % 2 == 0 {
                vec![SpriteCell { row: 0, col: 0, glyph: ',' }, SpriteCell { row: 0, col: 1, glyph: '~' }]
            } else {
                vec![SpriteCell { row: 0, col: 0, glyph: ',' }, SpriteCell { row: 0, col: 1, glyph: '≈' }]
            }
        }
        crate::game::habitat::NEEDLEFISH => vec![SpriteCell { row: 0, col: 0, glyph: '‹' }, SpriteCell { row: 0, col: 1, glyph: '·' }],
        crate::game::habitat::GLASS_SNAIL => vec![SpriteCell { row: 0, col: 0, glyph: '◔' }],
        crate::game::habitat::BURROWER => vec![SpriteCell { row: 0, col: 0, glyph: '▴' }],
        crate::game::habitat::RIM_SKIMMER => vec![SpriteCell { row: 0, col: 0, glyph: '◜' }],
        crate::game::habitat::SAND_RAY => vec![SpriteCell { row: 0, col: 0, glyph: '▱' }],
        crate::game::habitat::SCHOOLLET => vec![SpriteCell { row: 0, col: 0, glyph: '‹' }, SpriteCell { row: 0, col: 2, glyph: '‹' }],
        crate::game::habitat::ANEMONE_HOST => anemone_anchor_sprite(morph.unwrap_or(AnemoneMorph::Flower)),
        _ => Vec::new(),
    }
}
```

Use these exact anemone anchors:

```rust
pub fn anemone_anchor_sprite(morph: AnemoneMorph) -> Vec<SpriteCell> {
    match morph {
        AnemoneMorph::Flower => vec![
            SpriteCell { row: 0, col: 1, glyph: '✺' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '╯' },
        ],
        AnemoneMorph::Comb => vec![
            SpriteCell { row: 0, col: 0, glyph: '╵' },
            SpriteCell { row: 0, col: 1, glyph: '╷' },
            SpriteCell { row: 0, col: 2, glyph: '╵' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '┬' },
            SpriteCell { row: 1, col: 2, glyph: '╯' },
        ],
        AnemoneMorph::Crown => vec![
            SpriteCell { row: 0, col: 0, glyph: '⌁' },
            SpriteCell { row: 0, col: 1, glyph: '⌁' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '╮' },
            SpriteCell { row: 2, col: 0, glyph: '╱' },
            SpriteCell { row: 2, col: 1, glyph: '╲' },
        ],
        AnemoneMorph::DotColony => vec![
            SpriteCell { row: 0, col: 0, glyph: '⁙' },
            SpriteCell { row: 0, col: 1, glyph: '⁙' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '╯' },
        ],
    }
}

pub fn host_fish_sprite() -> Vec<SpriteCell> {
    vec![SpriteCell { row: 0, col: 0, glyph: '›' }, SpriteCell { row: 0, col: 1, glyph: '·' }]
}
```

- [ ] **Step 5: Validate glyph width**

Add:

```rust
pub fn validate_tank_life_catalog() -> std::result::Result<(), String> {
    use unicode_width::UnicodeWidthChar;

    let samples = [
        sprite_for(crate::game::habitat::GLASS_SHRIMP, 0, None),
        sprite_for(crate::game::habitat::GLASS_SHRIMP, 1, None),
        sprite_for(crate::game::habitat::NEEDLEFISH, 0, None),
        sprite_for(crate::game::habitat::GLASS_SNAIL, 0, None),
        sprite_for(crate::game::habitat::BURROWER, 0, None),
        sprite_for(crate::game::habitat::RIM_SKIMMER, 0, None),
        sprite_for(crate::game::habitat::SAND_RAY, 0, None),
        sprite_for(crate::game::habitat::SCHOOLLET, 0, None),
        anemone_anchor_sprite(AnemoneMorph::Flower),
        anemone_anchor_sprite(AnemoneMorph::Comb),
        anemone_anchor_sprite(AnemoneMorph::Crown),
        anemone_anchor_sprite(AnemoneMorph::DotColony),
        host_fish_sprite(),
    ];

    for sprite in samples {
        for cell in sprite {
            if UnicodeWidthChar::width(cell.glyph) != Some(1) {
                return Err(format!("tank-life glyph {:?} is not terminal width 1", cell.glyph));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Implement route placement**

Implement `tank_life_placements_for` as a pure function. Route phase uses `(id, pet_seed, local_date, now)`, with activity only affecting timing scalar:

```rust
pub fn tank_life_placements_for(input: &TankLifeRenderInput<'_>) -> Vec<TankLifePlacement> {
    input
        .rendered_ids
        .iter()
        .filter_map(|id| placement_for_id(id, input))
        .collect()
}
```

Route behavior:

- `glass_shrimp`: lower-lane hop, foreground, never below `habitat.y + habitat.height - 3` on round; use lower arc when `literal_floor_allowed == false`.
- `needlefish`: cross-tank shallow arc; behind layer for first half, foreground for middle third, behind layer at exit.
- `glass_snail`: glass-wall creep on left or right edge, foreground.
- `burrower`: lower-edge peek with 0, 1, or 2 visible rows depending on phase; foreground.
- `rim_skimmer`: perimeter route; behind near rear arc, foreground near front arc.
- `sand_ray`: lower-lane glide, foreground, larger gap from pet protected rect than shrimp.
- `schoollet`: two or three grouped cross-tank cells; same layer rules as needlefish.
- `anemone_host`: anchor cells behind, host fish orbit cells route-dependent.

Every generated cell must pass these checks before inclusion:

```rust
fn cell_allowed(input: &TankLifeRenderInput<'_>, col: u16, row: u16, layer: HabitatPetLayer) -> bool {
    rect_contains(input.geometry.habitat, col, row)
        && input.geometry.cell_inside_aperture(col, row)
        && !input.geometry.reserved_regions.iter().any(|region| rect_contains(*region, col, row))
        && !(layer == HabitatPetLayer::Foreground
            && input.pet_protected_regions.iter().any(|region| rect_contains(*region, col, row)))
}
```

If no cells remain after filtering, return `None` for that placement.

- [ ] **Step 7: Add summaries for Preview Lab**

Add:

```rust
pub fn layer_segment_summaries(placements: &[TankLifePlacement]) -> Vec<TankLifeLayerSegmentSummary> {
    let mut summaries = Vec::new();
    for placement in placements {
        for layer in [
            HabitatPetLayer::Background,
            HabitatPetLayer::Behind,
            HabitatPetLayer::Foreground,
        ] {
            let cell_count = placement.cells.iter().filter(|cell| cell.pet_layer == layer).count();
            if cell_count > 0 {
                summaries.push(TankLifeLayerSegmentSummary {
                    inhabitant_id: placement.inhabitant_id.clone(),
                    pet_layer: layer,
                    cell_count,
                });
            }
        }
    }
    summaries
}
```

Add these small geometry helpers in the same module:

```rust
pub fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

impl TankLifeSurfaceGeometry {
    pub fn cell_inside_aperture(&self, col: u16, row: u16) -> bool {
        let Some(mask) = self.aperture_mask else {
            return true;
        };
        let dx = i32::from(col) - i32::from(mask.center_col);
        let dy = i32::from(row) - i32::from(mask.center_row);
        let rx = i32::from(mask.radius_cols.max(1));
        let ry = i32::from(mask.radius_rows.max(1));
        (dx * dx * ry * ry + dy * dy * rx * rx) <= rx * rx * ry * ry
    }
}

#[cfg(test)]
impl<'a> TankLifeRenderInput<'a> {
    fn for_test(
        rendered_ids: Vec<TankInhabitantId>,
        geometry: &'a TankLifeSurfaceGeometry,
        local_date: time::Date,
        elapsed_seconds: i64,
    ) -> Self {
        Self {
            rendered_ids,
            pet_seed: "glorp-tank-life-fixture",
            local_date,
            now: time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(elapsed_seconds),
            geometry,
            pet_protected_regions: &[],
            color_capability: crate::tui::style::ColorCapability::Truecolor,
            life_profile: crate::tui::life::PetLifeProfile::default(),
        }
    }
}
```

- [ ] **Step 8: Verify Task 4**

Run:

```bash
cargo test tui::component::tank_life::tests::catalog_glyphs_are_single_width_cells
cargo test tui::component::tank_life::tests::anemone_host_morphs_share_host_behavior_but_unique_anchor_cells
cargo test tui::component::tank_life::tests::route_dependent_swimmer_has_behind_and_foreground_segments
cargo test tui::component::tank_life::tests::round_routes_avoid_reserved_regions
```

Expected: all listed tests pass.

## Task 5: Integrate Tank Life Into Shared Pet Scene Rendering

**Spec sections:** Rendering Model, Surface Behavior, Route Grammar, Motion And Activity.

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Create: `src/tui/panels/pet/tank_life.rs`
- Modify: `src/tui/panels/pet/draw.rs`
- Modify: `src/round/scene.rs`
- Test: `src/tui/panels/pet/tank_life.rs`
- Test: `tests/round_scene.rs`

**Interfaces:**
- Produces: `tank_life_layer_cells`
- Produces: `watch_tank_life_geometry`
- Produces: `round_tank_life_geometry`
- Consumes: `TankLifeRenderInput`

- [ ] **Step 1: Write failing render-order and round-safety tests**

Add a test in `src/tui/panels/pet/tank_life.rs`:

```rust
#[test]
fn tank_life_layer_cells_filters_by_layer_and_habitat() {
    use crate::game::habitat::HabitatPetLayer;
    use crate::storage::state::TankInhabitantId;
    use crate::tui::component::TankLifeCell;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};

    let scene = make_scene(Rect::new(0, 0, 20, 10));
    let cells = vec![
        TankLifeCell {
            inhabitant_id: TankInhabitantId::new("glass_shrimp"),
            row: 2,
            col: 3,
            glyph: ',',
            style: Style::default().fg(Color::Rgb(200, 160, 220)),
            pet_layer: HabitatPetLayer::Foreground,
        },
        TankLifeCell {
            inhabitant_id: TankInhabitantId::new("needlefish"),
            row: 11,
            col: 3,
            glyph: '‹',
            style: Style::default(),
            pet_layer: HabitatPetLayer::Foreground,
        },
    ];

    let draw = tank_life_layer_cells(&cells, &scene, &[HabitatPetLayer::Foreground]);

    assert_eq!(draw.len(), 1);
    assert_eq!(draw[0].row, 2);
    assert_eq!(draw[0].col, 3);
    assert_eq!(draw[0].glyph.as_deref(), Some(","));
}
```

Add round safety tests in `tests/round_scene.rs`:

```rust
#[test]
fn round_scene_tank_life_foreground_avoids_pet_face_and_bottom_hud() {
    let now = time::macros::datetime!(2026-07-08 18:00 UTC);
    let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(60, now.date());
    vm.habitat.tank_life_local_date = time::macros::date!(2026-07-08);
    vm.habitat.tank_life_calendar_age_days = 60;

    let scene = glorp::round::scene::build_round_scene_draw_list(
        &vm,
        now,
        44,
        18,
        &glorp::round::scene::companion_roam_motion(),
    );

    let protected = glorp::round::scene::round_tank_life_protected_regions_for_test(scene.pet_rect, 44, 18);
    for cell in scene.draw_list.cells.iter().filter(|cell| cell.glyph.is_some()) {
        assert!(
            !protected.bottom_hud.iter().any(|region| rect_contains(*region, cell.col, cell.row)),
            "tank life and pet glyph cells must stay clear of bottom HUD reserve"
        );
    }
}
```

- [ ] **Step 2: Run tests and confirm expected failures**

Run:

```bash
cargo test tui::panels::pet::tank_life::tests::tank_life_layer_cells_filters_by_layer_and_habitat
cargo test --test round_scene round_scene_tank_life_foreground_avoids_pet_face_and_bottom_hud
```

Expected: compilation fails because render integration does not exist.

- [ ] **Step 3: Add the pet-panel tank-life helper module**

In `src/tui/panels/pet.rs`:

```rust
mod tank_life;
```

Create `src/tui/panels/pet/tank_life.rs` with a helper parallel to `props.rs`:

```rust
use ratatui::style::Color;

use crate::game::habitat::HabitatPetLayer;
use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use crate::tui::component::{PetSceneLayout, TankLifeCell};

fn habitat_contains(scene: &PetSceneLayout, cell: &TankLifeCell) -> bool {
    cell.col >= scene.habitat.x
        && cell.row >= scene.habitat.y
        && cell.col < scene.habitat.x.saturating_add(scene.habitat.width)
        && cell.row < scene.habitat.y.saturating_add(scene.habitat.height)
}

fn style_fg_to_rgb(color: Option<Color>) -> Option<Rgb> {
    match color {
        Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
        _ => None,
    }
}

pub(super) fn tank_life_layer_cells(
    tank_cells: &[TankLifeCell],
    scene: &PetSceneLayout,
    layers: &[HabitatPetLayer],
) -> Vec<DrawCell> {
    use ratatui::style::Modifier;

    tank_cells
        .iter()
        .filter(|cell| layers.contains(&cell.pet_layer) && habitat_contains(scene, cell))
        .map(|cell| DrawCell {
            row: cell.row,
            col: cell.col,
            glyph: Some(cell.glyph.to_string()),
            fg: style_fg_to_rgb(cell.style.fg),
            bg: match cell.style.bg {
                Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
                _ => None,
            },
            bold: cell.style.add_modifier.contains(Modifier::BOLD),
        })
        .collect()
}
```

- [ ] **Step 4: Add watch surface geometry and protected regions**

In `src/tui/component/tank_life.rs`, add:

```rust
pub fn watch_tank_life_geometry(scene: &crate::tui::component::PetSceneLayout) -> TankLifeSurfaceGeometry {
    TankLifeSurfaceGeometry {
        surface: TankLifeSurface::Watch,
        habitat: scene.habitat,
        aperture_mask: None,
        reserved_regions: scene.speech.into_iter().collect(),
        max_moving_slots: 5,
        literal_floor_allowed: true,
    }
}

pub fn pet_face_protected_regions(pet_art: Rect) -> Vec<Rect> {
    vec![Rect::new(
        pet_art.x + pet_art.width / 4,
        pet_art.y + 1,
        pet_art.width / 2,
        4.min(pet_art.height),
    )]
}
```

- [ ] **Step 5: Insert tank-life draw passes**

In `src/tui/panels/pet/draw.rs`, import the helper:

```rust
use super::tank_life::tank_life_layer_cells;
```

After `prop_cells` are computed and before contact shadow, build tank-life cells once:

```rust
let tank_geometry = crate::tui::component::watch_tank_life_geometry(scene);
let canonical_tank_life = crate::tui::component::canonical_daily_cast(
    &vm.habitat.earned_inhabitants,
    &vm.pet_render.seed,
    vm.habitat.tank_life_local_date,
    vm.habitat.tank_life_calendar_age_days,
);
let projected_tank_life =
    crate::tui::component::project_tank_life_cast(&canonical_tank_life, &tank_geometry);
let pet_protected = crate::tui::component::pet_face_protected_regions(scene.pet_art);
let tank_life_placements = crate::tui::component::tank_life_placements_for(
    &crate::tui::component::TankLifeRenderInput {
        rendered_ids: projected_tank_life.rendered_ids.clone(),
        pet_seed: &vm.pet_render.seed,
        local_date: vm.habitat.tank_life_local_date,
        now,
        geometry: &tank_geometry,
        pet_protected_regions: &pet_protected,
        color_capability: ctx.color_capability,
        life_profile: life_profile.clone(),
    },
);
let tank_life_cells = tank_life_placements
    .iter()
    .flat_map(|placement| placement.cells.clone())
    .collect::<Vec<_>>();
```

Add the passes in this order:

```rust
// after props Background/Behind
list.extend(tank_life_layer_cells(
    &tank_life_cells,
    scene,
    &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
));

// existing contact shadow and pet body stay here

// after props Foreground
list.extend(tank_life_layer_cells(
    &tank_life_cells,
    scene,
    &[HabitatPetLayer::Foreground],
));
```

This keeps the final order:

1. background ambient texture
2. background / behind props
3. background / behind inhabitants
4. pet
5. foreground props
6. foreground inhabitants

Speech and native companion HUD continue to render outside the shared draw list and stay above scene cells.

- [ ] **Step 6: Add round geometry helpers**

In `src/round/scene.rs`, add pure helpers near companion motion helpers:

```rust
pub struct RoundTankLifeProtectedRegions {
    pub pet_face: Vec<Rect>,
    pub bottom_hud: Vec<Rect>,
}

pub fn round_tank_life_geometry(grid_cols: u16, grid_rows: u16) -> crate::tui::component::TankLifeSurfaceGeometry {
    let bottom_hud_rows = 5.min(grid_rows / 3);
    crate::tui::component::TankLifeSurfaceGeometry {
        surface: crate::tui::component::TankLifeSurface::Round,
        habitat: Rect::new(0, 0, grid_cols, grid_rows),
        aperture_mask: Some(crate::tui::component::RoundApertureMask {
            center_col: (grid_cols / 2) as i16,
            center_row: (grid_rows / 2) as i16,
            radius_cols: grid_cols / 2,
            radius_rows: grid_rows / 2,
        }),
        reserved_regions: vec![Rect::new(
            0,
            grid_rows.saturating_sub(bottom_hud_rows),
            grid_cols,
            bottom_hud_rows,
        )],
        max_moving_slots: 2,
        literal_floor_allowed: false,
    }
}

pub fn round_tank_life_protected_regions_for_test(
    pet_rect: Rect,
    grid_cols: u16,
    grid_rows: u16,
) -> RoundTankLifeProtectedRegions {
    let geometry = round_tank_life_geometry(grid_cols, grid_rows);
    RoundTankLifeProtectedRegions {
        pet_face: crate::tui::component::pet_face_protected_regions(pet_rect),
        bottom_hud: geometry.reserved_regions,
    }
}
```

In `build_round_scene_draw_list`, after `layout.pet_art = new_pet_art`, override the geometry used by the shared renderer. The smallest implementation is to add a field to `RenderContext` only if one already exists for surface hints; otherwise, pass a `TankLifeSurfaceGeometry` override into a new `render_pet_to_draw_list_with_tank_geometry` wrapper and have the existing `render_pet_to_draw_list` call that wrapper with watch geometry.

Do not modify `src/round/hud.rs` in this task unless a test proves the no-go band must import a public constant from it. If touching it is required, inspect its dirty diff first.

- [ ] **Step 7: Verify Task 5**

Run:

```bash
cargo test tui::panels::pet::tank_life::tests::tank_life_layer_cells_filters_by_layer_and_habitat
cargo test --test round_scene round_scene_tank_life_foreground_avoids_pet_face_and_bottom_hud
cargo test --test round_scene
```

Expected: all listed tests pass. Round companion tests should not show tank-life cells in bottom HUD reserve or protected face regions.

## Task 6: Add Preview Lab Tank-Life Scenario And Typed Artifact

**Spec sections:** Preview Lab, Testing, Visual Review.

**Files:**
- Create: `src/dev_preview/tank_life.rs`
- Modify: `src/dev_preview/mod.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/round.rs`
- Modify: `src/cli.rs`
- Modify: `src/commands/dev_preview.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Produces: hidden `--scenario tank-life`
- Produces: `frames/<id>.tank-life.json`
- Produces: `PreviewTankLifeArtifact`
- Produces: manifest `schema_version: 7`

- [ ] **Step 1: Write failing Preview Lab tests**

Add constants and tests in `tests/dev_preview.rs`:

```rust
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

#[test]
fn dev_preview_tank_life_writes_typed_artifacts() {
    let run = PreviewRun::new();

    run.run_success("tank-life");

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 7);
    for id in TANK_LIFE_IDS {
        assert!(run.out.join(format!("frames/{id}.txt")).is_file(), "missing {id}.txt");
        assert!(
            run.out.join(format!("frames/{id}.tank-life.json")).is_file(),
            "missing {id}.tank-life.json"
        );
        let artifact: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(run.out.join(format!("frames/{id}.tank-life.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(artifact["schema_version"], 1);
        assert!(artifact["local_date"].as_str().is_some());
        assert!(artifact["calendar_age_days"].as_i64().is_some());
        assert!(artifact["target_surface"].as_str().is_some());
        assert!(artifact["canonical_ids"].as_array().is_some());
        assert!(artifact["rendered_ids"].as_array().is_some());
        assert!(artifact["skipped"].as_array().is_some());
        assert!(artifact["placements"].as_array().is_some());
        assert!(artifact["collision_status"]["reserved_region_clear"].as_bool().is_some());
    }
}

#[test]
fn dev_preview_all_includes_tank_life_artifacts() {
    let run = PreviewRun::new();

    run.run_success("all");

    assert!(run.out.join("frames/tank-life-round-projection.tank-life.json").is_file());
}
```

- [ ] **Step 2: Run tests and confirm expected failures**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_tank_life_writes_typed_artifacts
cargo test --features dev-preview --test dev_preview dev_preview_all_includes_tank_life_artifacts
```

Expected: `tank-life` is not an accepted scenario or artifacts are missing.

- [ ] **Step 3: Add preview export contract**

In `src/dev_preview/export.rs`:

- Bump `SCHEMA_VERSION` from `6` to `7`.
- Add `TankLife` to `PreviewScenarioKind`.
- Add `tank_life: Option<PathBuf>` to `PreviewScenarioFiles`.
- Add `TankLife` to `ArtifactType`.

In `src/dev_preview/contract.rs`, add:

```rust
pub const TANK_LIFE_CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewTankLifeArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub local_date: String,
    pub calendar_age_days: i64,
    pub target_surface: String,
    pub canonical_ids: Vec<String>,
    pub rendered_ids: Vec<String>,
    pub skipped: Vec<PreviewTankLifeSkipArtifact>,
    pub anemone_morph: Option<String>,
    pub placements: Vec<PreviewTankLifePlacementArtifact>,
    pub layer_segments: Vec<PreviewTankLifeLayerArtifact>,
    pub collision_status: PreviewTankLifeCollisionArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifeSkipArtifact {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifePlacementArtifact {
    pub id: String,
    pub route_family: String,
    pub bounds: PreviewTargetArtifact,
    pub cell_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifeLayerArtifact {
    pub id: String,
    pub pet_layer: String,
    pub cell_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifeCollisionArtifact {
    pub reserved_region_clear: bool,
    pub aperture_clear: bool,
    pub protected_pet_face_clear: bool,
}
```

Add `PreviewFrameContract.tank_life: Option<PreviewTankLifeArtifact>` beside `scene` and `hud`.

- [ ] **Step 4: Write tank-life artifacts in scenarios**

In `src/dev_preview/scenarios.rs`, write the artifact during the frame loop:

```rust
if let Some(tank_life) = &frame.contract.tank_life {
    write_json_artifact(&staging_dir.join(tank_life_path(frame)), tank_life)?;
}
```

Add path helpers:

```rust
fn tank_life_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.tank-life.json", frame.id))
}
```

Add the artifact to `artifacts_for_frames` with type `ArtifactType::TankLife`.

- [ ] **Step 5: Add the hidden scenario selector**

Add `TankLife` to `PreviewSelection` in `src/dev_preview/scenarios.rs`.

Update CLI parsing in `src/cli.rs` and `src/commands/dev_preview.rs` so:

```bash
cargo run -- dev-preview --scenario tank-life --out target/glorp-preview
```

is accepted.

`PreviewSelection::All` must include the tank-life bundles.

- [ ] **Step 6: Build tank-life fixture frames**

Create `src/dev_preview/tank_life.rs`:

```rust
pub fn tank_life_bundles(
    ctx: &crate::dev_preview::scenarios::PreviewRenderContext,
    scratch_dir: &std::path::Path,
) -> crate::error::Result<Vec<crate::dev_preview::scenarios::PreviewScenarioBundle>> {
    tank_life_fixtures()
        .iter()
        .map(|fixture| render_tank_life_fixture(ctx, scratch_dir, fixture))
        .collect()
}
```

Required frame ids:

- `tank-life-age-empty`: age 0, watch target, empty canonical/rendered ids.
- `tank-life-age-first`: age 1, watch target, `glass_shrimp`.
- `tank-life-age-early`: age 7, watch target, early cast.
- `tank-life-age-full`: age 60, watch target, mature cast.
- `tank-life-date-2026-07-07`: age 60, watch target, first daily cast fixture.
- `tank-life-date-2026-07-08`: age 60, watch target, second daily cast fixture.
- `tank-life-round-projection`: age 60, round target, max two rendered ids and budget skips.
- `tank-life-anemone-morphs`: four side-by-side static anchors plus host fish route proof.

Each frame must use the shipped `TankLifeSurface::Watch` or `TankLifeSurface::Round`. Do not introduce `TankLifeSurface::Preview`.

Implement the fixture table as data, not one-off branches:

```rust
struct TankLifeFixture {
    id: &'static str,
    title: &'static str,
    width: u16,
    height: u16,
    surface: crate::tui::component::TankLifeSurface,
    local_date: time::Date,
    age_days: i64,
}

fn tank_life_fixtures() -> Vec<TankLifeFixture> {
    vec![
        TankLifeFixture {
            id: "tank-life-age-empty",
            title: "Tank Life Age Empty",
            width: 120,
            height: 32,
            surface: crate::tui::component::TankLifeSurface::Watch,
            local_date: time::macros::date!(2026-07-07),
            age_days: 0,
        },
        TankLifeFixture {
            id: "tank-life-age-first",
            title: "Tank Life Age First",
            width: 120,
            height: 32,
            surface: crate::tui::component::TankLifeSurface::Watch,
            local_date: time::macros::date!(2026-07-07),
            age_days: 1,
        },
        TankLifeFixture {
            id: "tank-life-age-early",
            title: "Tank Life Age Early",
            width: 120,
            height: 32,
            surface: crate::tui::component::TankLifeSurface::Watch,
            local_date: time::macros::date!(2026-07-07),
            age_days: 7,
        },
        TankLifeFixture {
            id: "tank-life-age-full",
            title: "Tank Life Age Full",
            width: 120,
            height: 32,
            surface: crate::tui::component::TankLifeSurface::Watch,
            local_date: time::macros::date!(2026-07-07),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-date-2026-07-07",
            title: "Tank Life Date 2026-07-07",
            width: 120,
            height: 32,
            surface: crate::tui::component::TankLifeSurface::Watch,
            local_date: time::macros::date!(2026-07-07),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-date-2026-07-08",
            title: "Tank Life Date 2026-07-08",
            width: 120,
            height: 32,
            surface: crate::tui::component::TankLifeSurface::Watch,
            local_date: time::macros::date!(2026-07-08),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-round-projection",
            title: "Tank Life Round Projection",
            width: 44,
            height: 18,
            surface: crate::tui::component::TankLifeSurface::Round,
            local_date: time::macros::date!(2026-07-08),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-anemone-morphs",
            title: "Tank Life Anemone Morphs",
            width: 120,
            height: 32,
            surface: crate::tui::component::TankLifeSurface::Watch,
            local_date: time::macros::date!(2026-07-09),
            age_days: 60,
        },
    ]
}
```

Add these helpers in `src/dev_preview/tank_life.rs` with the exact names below:

- `render_tank_life_fixture(ctx, scratch_dir, fixture) -> Result<PreviewScenarioBundle>` creates `WatchViewModel::fixture_with_tank_inhabitants_for_age(fixture.age_days, fixture.local_date)`, renders through the real watch or round path for `fixture.surface`, attaches `frame.contract.tank_life`, and wraps it with `PreviewScenarioBundle::from_frame`.
- `tank_life_artifact_for_frame(frame_id, vm, surface, geometry, placements, projected) -> PreviewTankLifeArtifact` fills every artifact field from shared tank-life helpers: local date, calendar age, canonical/rendered ids, skipped reasons, selected morph, placement bounds, layer summaries, and collision booleans.

- [ ] **Step 7: Add scenario metadata**

In `scenario_metadata`, add `id if id.starts_with("tank-life-")`:

```rust
id if id.starts_with("tank-life-") => (
    PreviewScenarioKind::TankLife,
    "Review calendar-age-earned tank inhabitants, daily cast projection, route depth, and round safety.",
    frame.extra_inputs.clone(),
    vec![
        "Confirm each inhabitant silhouette is distinct at the target size.".to_string(),
        "Confirm daily cast ids change across dates while remaining stable for each date.".to_string(),
        "Confirm round projection avoids the pet face, bottom HUD, and aperture edge.".to_string(),
    ],
),
```

- [ ] **Step 8: Verify Task 6**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_tank_life_writes_typed_artifacts
cargo test --features dev-preview --test dev_preview dev_preview_all_includes_tank_life_artifacts
cargo test --features dev-preview --test dev_preview
```

Expected: all listed tests pass. Manifest schema is `7`, tank-life scenario artifacts exist, and `all` includes tank-life frames.

## Task 7: Final Verification And Visual Review

**Spec sections:** Testing, Visual Review, Non-Goals.

**Files:**
- Modify only if verification exposes a defect in files from Tasks 1-6.

- [ ] **Step 1: Run focused unit and integration tests**

Run:

```bash
cargo test game::habitat::tests::calendar_age_days_
cargo test game::habitat::tests::age_reconciliation_backfills_catalog_order_and_is_idempotent
cargo test tui::component::tank_life::tests
cargo test --test watch_integration tank_inhabitants -- --nocapture
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
```

Expected: all pass.

- [ ] **Step 2: Run broader checks**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: both pass without warnings.

- [ ] **Step 3: Generate Preview Lab bundle**

Run:

```bash
cargo run -- dev-preview --scenario tank-life --out target/glorp-preview-tank-life
```

Expected output includes:

```text
target/glorp-preview-tank-life
```

Verify files exist:

```bash
test -f target/glorp-preview-tank-life/index.html
test -f target/glorp-preview-tank-life/manifest.json
test -f target/glorp-preview-tank-life/frames/tank-life-round-projection.tank-life.json
```

Expected: all `test -f` commands exit `0`.

- [ ] **Step 4: Inspect preview artifacts**

Open:

```bash
open target/glorp-preview-tank-life/index.html
```

Review:

- no literal floor or substrate was added to round companion frames
- no foreground cell covers the pet face or bottom HUD reserve
- at least one route-dependent inhabitant clearly passes behind and in front
- `glass_shrimp`, `needlefish`, `glass_snail`, `burrower`, `rim_skimmer`, `sand_ray`, `schoollet`, and `anemone_host` are distinguishable
- Anemone Host morphs read as one family
- date fixtures show different rendered casts without crowding

- [ ] **Step 5: Inspect typed tank-life contract**

Run:

```bash
jq '.canonical_ids, .rendered_ids, .skipped, .collision_status' \
  target/glorp-preview-tank-life/frames/tank-life-round-projection.tank-life.json
```

Expected:

- `canonical_ids` has the mature daily cast
- `rendered_ids` has at most two ids for round
- `skipped` includes `surface_budget` for excess canonical ids
- `collision_status.reserved_region_clear`, `aperture_clear`, and `protected_pet_face_clear` are all `true`

- [ ] **Step 6: Run placeholder and diff hygiene checks**

Run:

```bash
rg -n "T[O]DO|T[B]D|PLACE[H]OLDER|FIX[ME]|X{3}|panic[!]\\(\"todo|unimplemented[!]" \
  src tests docs/superpowers/plans/2026-07-07-glorp-ambient-tank-life.md
git diff --check
git status --short
```

Expected:

- `rg` returns no new placeholder hits in touched files
- `git diff --check` prints no whitespace errors
- `git status --short` lists only intentional files for this feature plus any pre-existing unrelated dirty files Drew already had

- [ ] **Step 7: Self-review against design**

Confirm each item is true before calling implementation complete:

- age earns inhabitants from calendar days only
- no token field unlocks inhabitants
- canonical daily cast is deterministic for same seed/date/earned pool/age
- projection caps round to two moving slots by default
- activity cannot change earned pool, canonical ids, rendered count, or selected morph
- route output is cell-based and width-validated
- route-dependent inhabitants can emit behind and foreground cells
- round routes avoid aperture, bottom HUD reserve, and protected face rects
- unknown ids stay in state but are filtered from catalog-backed view data
- Preview Lab includes typed tank-life artifacts for watch and round fixtures
- no preview-only surface behavior was introduced
