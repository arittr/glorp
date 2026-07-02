# Glorp Calibrated Evolution Curve Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make newborn evolution progress from calibrated recent Tokenmaxxing usage, with first-run and reinit paths seeding cursors/history without feeding the pet.

**Architecture:** Keep lifecycle math in `src/game`, ledger/feedability in `src/storage/usage_store.rs`, and provider cursor identity in `src/usage`. Runtime applies lifecycle XP from canonical apply-window totals; smear buckets remain presentation/ledger pacing only. Init and cutover seed provider cursors before the pet can eat any historical rows.

**Tech Stack:** Rust, `rusqlite`, `time`, `serde_json`, `assert_cmd`, `proptest`, existing ccusage fixture helpers.

## Global Constraints

- No wall-clock gate for evolution.
- Tokenmaxxing `total_tokens` is `input + output + cache_creation + cache_read`.
- All baseline, active-hour, XP, progress-rate, first-run snapshot, and lifecycle paths use `TOKENMAXXING_TOTAL_V1`.
- `active_hours_per_day` is fixed at `8` for v1.
- Starting from S0: `B / 8` tokens reaches S1, `(B / 8) * (5.0 / 60.0)` tokens remains S0, `6 * B / 8` tokens reaches S2, and `B` tokens reaches S3.
- Historical rows may calibrate and seed cursors, but they must not grant XP, lifetime tokens, vitals, habitat props, source activity effects, today-summary activity, or stage transitions.
- First-contact and history seeding are evaluated per `(provider_surface, cursor_key)`, not only per provider surface.
- Confirmed `init --yes` uses reset-style usage-cache replacement before saving the newborn pet.
- Do not modify unrelated dirty files: `src/pet/render.rs` and `docs/superpowers/specs/2026-07-02-glitch-persistent-corruption-design.md`.

---

## File Structure

- Modify `src/game/calibration.rs`: deterministic active-day baseline, sparse-history median, refresh clamp, exported constants.
- Modify `src/game/evolution.rs`: active-hour constants, stage thresholds, lifecycle XP formula, shared stage-progress helpers.
- Modify `src/commands/status.rs`: use shared stage-progress helpers instead of the old threshold array.
- Modify `src/commands/watch.rs`: use shared stage-progress helpers instead of local threshold helpers.
- Modify `src/game/runtime.rs`: per-cursor first-contact classification, feedable-only application, aggregate XP from the canonical apply window.
- Modify `src/storage/usage_store.rs`: add `feedable` schema column, write seed rows as non-feedable, filter activity/lifecycle queries, add test helpers.
- Modify `src/usage/provider.rs`: keep `ProviderCursorKey` as the normative serialized key shape.
- Modify `src/usage/ccusage.rs`: include `TOKENMAXXING_TOTAL_V1` in snapshot and poll cursor keys, using the same builder in both paths.
- Modify `src/usage/cutover.rs`: refresh calibration with the clamp and seed cursors from current snapshots.
- Modify `src/commands/init.rs`: reset usage cache during confirmed reinit before creating the replacement pet.
- Modify `tests/game_rules.rs`: baseline and active-hour lifecycle tests.
- Modify `tests/runtime_integration.rs`: per-cursor first contact, feedable-only lifecycle, smear aggregate invariance, backfill clipping tests.
- Modify `tests/usage_provider.rs`: byte-identical snapshot/poll cursor key tests and clean first-run no-delta tests.
- Modify `tests/cli_smoke.rs`: reinit cache replacement and init no-feed assertions.
- Modify `tests/watch_integration.rs`: seeded history excluded from today/source/feed/activity views.
- Modify `README.md` and `npm/glorp/README.md`: calibration wording.

---

### Task 1: Deterministic Baseline Calibration

**Files:**
- Modify: `src/game/calibration.rs`
- Test: `tests/game_rules.rs`

**Interfaces:**
- Consumes: `DailyUsage { day, effective_tokens, activity_timestamp }`
- Produces:
  - `pub const DEFAULT_DAILY_EFFECTIVE_TOKENS: f64`
  - `pub const RECENT_ACTIVE_DAY_LIMIT: usize`
  - `pub const BASELINE_REFRESH_MIN_MULTIPLIER: f64`
  - `pub const BASELINE_REFRESH_MAX_MULTIPLIER: f64`
  - `impl CalibrationBaseline { pub fn from_history(history: &[DailyUsage]) -> Self }`
  - `impl CalibrationBaseline { pub fn refresh_from_history(self, history: &[DailyUsage]) -> Self }`

- [ ] **Step 1: Write failing baseline tests**

Add these tests to `tests/game_rules.rs` near the existing calibration tests:

```rust
#[test]
fn sparse_active_days_use_observed_median_instead_of_default() {
    let history = vec![
        DailyUsage::new(date!(2026 - 05 - 01), 200_000_000.0),
        DailyUsage::new(date!(2026 - 05 - 02), 300_000_000.0),
        DailyUsage::new(date!(2026 - 05 - 03), 400_000_000.0),
    ];

    let baseline = CalibrationBaseline::from_history(&history);

    assert_eq!(baseline.daily_effective_tokens, 300_000_000.0);
}

#[test]
fn calibration_filters_bad_totals_groups_by_day_and_uses_latest_thirty_active_days() {
    let mut history = Vec::new();
    history.push(DailyUsage::new(date!(2026 - 04 - 01), f64::NAN));
    history.push(DailyUsage::new(date!(2026 - 04 - 02), f64::INFINITY));
    history.push(DailyUsage::new(date!(2026 - 04 - 03), 0.0));
    history.push(DailyUsage::new(date!(2026 - 04 - 04), -10.0));
    history.push(DailyUsage::new(date!(2026 - 04 - 05), 999_999_999.0));
    for day in 1..=30 {
        let date = time::Date::from_calendar_date(2026, time::Month::May, day).unwrap();
        history.push(DailyUsage::new(date, day as f64 * 1_000.0));
    }
    history.push(DailyUsage::new(date!(2026 - 05 - 30), 10_000.0));

    let baseline = CalibrationBaseline::from_history(&history);

    assert_eq!(baseline.daily_effective_tokens, 15_500.0);
}

#[test]
fn empty_or_only_bad_history_uses_default_baseline() {
    assert_eq!(
        CalibrationBaseline::from_history(&[]).daily_effective_tokens,
        100_000.0
    );
    assert_eq!(
        CalibrationBaseline::from_history(&[
            DailyUsage::new(date!(2026 - 05 - 01), 0.0),
            DailyUsage::new(date!(2026 - 05 - 02), f64::NEG_INFINITY),
        ])
        .daily_effective_tokens,
        100_000.0
    );
}

#[test]
fn baseline_refresh_clamps_existing_pet_changes() {
    let old = CalibrationBaseline { daily_effective_tokens: 1_000_000.0 };

    let high = old.refresh_from_history(&[
        DailyUsage::new(date!(2026 - 05 - 01), 10_000_000.0),
        DailyUsage::new(date!(2026 - 05 - 02), 11_000_000.0),
        DailyUsage::new(date!(2026 - 05 - 03), 12_000_000.0),
        DailyUsage::new(date!(2026 - 05 - 04), 13_000_000.0),
        DailyUsage::new(date!(2026 - 05 - 05), 14_000_000.0),
    ]);
    let low = old.refresh_from_history(&[
        DailyUsage::new(date!(2026 - 05 - 01), 10_000.0),
        DailyUsage::new(date!(2026 - 05 - 02), 11_000.0),
        DailyUsage::new(date!(2026 - 05 - 03), 12_000.0),
        DailyUsage::new(date!(2026 - 05 - 04), 13_000.0),
        DailyUsage::new(date!(2026 - 05 - 05), 14_000.0),
    ]);

    assert_eq!(high.daily_effective_tokens, 2_000_000.0);
    assert_eq!(low.daily_effective_tokens, 500_000.0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test game_rules sparse_active_days_use_observed_median_instead_of_default
cargo test --test game_rules baseline_refresh_clamps_existing_pet_changes
```

Expected: the sparse-history test fails because `from_history` still returns the default for fewer than five active days, and the refresh test fails because `refresh_from_history` does not exist.

- [ ] **Step 3: Implement deterministic baseline rules**

Edit `src/game/calibration.rs` to expose constants and replace `from_history` with this shape:

```rust
pub const DEFAULT_DAILY_EFFECTIVE_TOKENS: f64 = 100_000.0;
pub const RECENT_ACTIVE_DAY_LIMIT: usize = 30;
pub const BASELINE_REFRESH_MIN_MULTIPLIER: f64 = 0.5;
pub const BASELINE_REFRESH_MAX_MULTIPLIER: f64 = 2.0;

impl CalibrationBaseline {
    pub fn from_history(history: &[DailyUsage]) -> Self {
        let mut by_day = std::collections::BTreeMap::<Date, f64>::new();
        for day in history
            .iter()
            .copied()
            .filter(|day| day.effective_tokens.is_finite() && day.effective_tokens > 0.0)
        {
            *by_day.entry(day.day).or_insert(0.0) += day.effective_tokens;
        }

        let mut active_days = by_day
            .into_iter()
            .map(|(day, effective_tokens)| DailyUsage::new(day, effective_tokens))
            .collect::<Vec<_>>();
        active_days.sort_by_key(|day| day.day);

        let recent_start = active_days.len().saturating_sub(RECENT_ACTIVE_DAY_LIMIT);
        let mut recent_values = active_days[recent_start..]
            .iter()
            .map(|day| day.effective_tokens)
            .filter(|value| value.is_finite() && *value > 0.0)
            .collect::<Vec<_>>();

        if recent_values.is_empty() {
            return Self::default();
        }

        recent_values.sort_by(f64::total_cmp);
        Self {
            daily_effective_tokens: median(&recent_values).max(1.0),
        }
    }

    pub fn refresh_from_history(self, history: &[DailyUsage]) -> Self {
        let candidate = Self::from_history(history);
        let current = self.daily_effective_tokens.max(1.0);
        Self {
            daily_effective_tokens: candidate.daily_effective_tokens.clamp(
                current * BASELINE_REFRESH_MIN_MULTIPLIER,
                current * BASELINE_REFRESH_MAX_MULTIPLIER,
            ),
        }
    }
}
```

Remove the private `DEFAULT_DAILY_EFFECTIVE_TOKENS`, `MIN_ACTIVE_DAYS_FOR_MEDIAN`, and `RECENT_ACTIVE_DAY_LIMIT` definitions that conflict with the public constants.

- [ ] **Step 4: Update existing calibration test expectations**

In `tests/game_rules.rs`, update `calibration_groups_multiple_rows_on_the_same_active_day_before_median` so it asserts the new sparse median:

```rust
assert_eq!(baseline.daily_effective_tokens, 200_000.0);
```

Update the explanatory comment above that assertion to say grouping makes this four active days and sparse history now uses the observed median.

- [ ] **Step 5: Verify Task 1**

Run:

```bash
cargo test --test game_rules
```

Expected: the focused test file passes.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/game/calibration.rs tests/game_rules.rs
git commit -m "fix(game): calibrate baseline from sparse active days"
```

---

### Task 2: Active-Hour XP Thresholds And Shared Progress Helpers

**Files:**
- Modify: `src/game/evolution.rs`
- Modify: `src/commands/status.rs`
- Modify: `src/commands/watch.rs`
- Test: `tests/game_rules.rs`
- Test: `tests/runtime_integration.rs`

**Interfaces:**
- Consumes: `CalibrationBaseline`
- Produces:
  - `pub const ACTIVE_HOURS_PER_DAY: f64`
  - `pub const STAGE_THRESHOLDS: [f64; 7]`
  - `pub fn stage_start_xp(stage: Stage) -> f64`
  - `pub fn next_stage_xp_target(stage: Stage) -> f64`
  - `pub fn calibrated_xp_units(delta_effective: f64, baseline: CalibrationBaseline) -> f64`

- [ ] **Step 1: Write failing active-hour lifecycle tests**

Add to `tests/game_rules.rs` near the XP tests:

```rust
#[test]
fn active_hour_equivalents_drive_early_stages() {
    let baseline = CalibrationBaseline { daily_effective_tokens: 800_000.0 };
    let active_hour = baseline.daily_effective_tokens / 8.0;

    let five_minute_fraction = apply_xp_delta(0.0, active_hour * (5.0 / 60.0), baseline);
    assert_eq!(stage_for_xp(five_minute_fraction.xp), Stage::S0);

    let one_active_hour = apply_xp_delta(0.0, active_hour, baseline);
    assert_eq!(stage_for_xp(one_active_hour.xp), Stage::S1);

    let six_active_hours = apply_xp_delta(0.0, active_hour * 6.0, baseline);
    assert_eq!(stage_for_xp(six_active_hours.xp), Stage::S2);

    let one_active_day = apply_xp_delta(0.0, baseline.daily_effective_tokens, baseline);
    assert_eq!(stage_for_xp(one_active_day.xp), Stage::S3);
}

#[test]
fn xp_is_linear_through_one_active_day() {
    let baseline = CalibrationBaseline { daily_effective_tokens: 800_000.0 };

    assert_eq!(apply_xp_delta(0.0, 100_000.0, baseline).xp, 0.125);
    assert_eq!(apply_xp_delta(0.0, 600_000.0, baseline).xp, 0.75);
    assert_eq!(apply_xp_delta(0.0, 800_000.0, baseline).xp, 1.0);
}
```

Add to `tests/runtime_integration.rs` near `staged_usage_apportions_token_buckets_across_smear_rows`:

```rust
#[test]
fn smear_buckets_do_not_change_lifecycle_xp_for_one_apply_window() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 800_000.0;
    let poll = poll_with_delta(800_000.0, now);

    let direct = glorp::game::evolution::apply_xp_delta(0.0, 800_000.0, state.calibration).xp;
    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();

    assert_eq!(direct, 1.0);
    assert_eq!(state.xp, direct);
    assert_eq!(state.stage, Stage::S3);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test game_rules active_hour_equivalents_drive_early_stages
cargo test --test game_rules xp_is_linear_through_one_active_day
cargo test --test runtime_integration smear_buckets_do_not_change_lifecycle_xp_for_one_apply_window
```

Expected: failures show the old thresholds and the old diminishing-return formula above `0.25` active days.

- [ ] **Step 3: Implement active-hour thresholds and XP formula**

In `src/game/evolution.rs`, replace the private threshold constant and add public helpers:

```rust
pub const ACTIVE_HOURS_PER_DAY: f64 = 8.0;
pub const STAGE_THRESHOLDS: [f64; 7] = [
    0.0,
    1.0 / ACTIVE_HOURS_PER_DAY,
    6.0 / ACTIVE_HOURS_PER_DAY,
    1.0,
    4.0,
    14.0,
    60.0,
];

pub fn stage_start_xp(stage: Stage) -> f64 {
    STAGE_THRESHOLDS[stage.index()]
}

pub fn next_stage_xp_target(stage: Stage) -> f64 {
    let next = (stage.index() + 1).min(Stage::S6.index());
    STAGE_THRESHOLDS[next]
}
```

Replace `calibrated_xp_units` with:

```rust
pub fn calibrated_xp_units(delta_effective: f64, baseline: CalibrationBaseline) -> f64 {
    let daily = baseline.daily_effective_tokens.max(1.0);
    let relative = (delta_effective / daily).max(0.0);
    let direct = relative.min(1.0);
    let excess = (relative - 1.0).max(0.0);
    direct + excess.sqrt() * 0.05
}
```

- [ ] **Step 4: Centralize status and watch progress thresholds**

In `src/commands/status.rs`, import the shared helpers:

```rust
use crate::game::evolution::{next_stage_xp_target, stage_start_xp, Stage};
```

Replace `stage_progress_line` with:

```rust
fn stage_progress_line(xp: f64) -> String {
    let xp = xp.max(0.0);
    let stage = crate::game::evolution::stage_for_xp(xp);
    if matches!(stage, Stage::S6) {
        return "stage progress: s6 complete".into();
    }

    let start = stage_start_xp(stage);
    let next = next_stage_xp_target(stage);
    let span = (next - start).max(f64::EPSILON);
    let percent = ((xp - start) / span * 100.0).clamp(0.0, 100.0);
    format!(
        "stage progress: {:.0}% to {}",
        percent,
        Stage::from_index(stage.index() + 1).unwrap_or(Stage::S6)
    )
}
```

In `src/commands/watch.rs`, replace the local `next_stage_xp_target` and `stage_start_xp` functions with imports:

```rust
use crate::game::evolution::{next_stage_xp_target, stage_start_xp, Stage};
```

Then delete the local helper definitions near the bottom of the file.

- [ ] **Step 5: Update runtime test expectations affected by threshold changes**

In `tests/runtime_integration.rs`, update `provider_delta_updates_pet_state_and_records_evolution_once` so two one-day polls now reach S3, not S2:

```rust
assert_eq!(state.stage, Stage::S3);
assert!(state.xp >= 1.0);
assert!(state.xp < 4.0);
for label in ["fuzzling", "kit", "pup"] {
    let expected_text = format!("mochi evolved into {label}");
    assert_eq!(
        state
            .recent_events
            .iter()
            .filter(|event| event.text.contains(&expected_text))
            .count(),
        1,
        "expected '{expected_text}' recorded once",
    );
}
assert_eq!(state.seen_stage_transitions, vec![Stage::S1, Stage::S2, Stage::S3]);
```

- [ ] **Step 6: Verify Task 2**

Run:

```bash
cargo test --test game_rules
cargo test --test runtime_integration
cargo test --test doctor_status
cargo test --test watch_integration
```

Expected: all tests in the focused files pass.

- [ ] **Step 7: Commit Task 2**

```bash
git add src/game/evolution.rs src/commands/status.rs src/commands/watch.rs tests/game_rules.rs tests/runtime_integration.rs
git commit -m "fix(game): express early evolution as active-hour thresholds"
```

---

### Task 3: Feedable Ledger Semantics For Seeded History

**Files:**
- Modify: `src/storage/usage_store.rs`
- Modify: `tests/watch_integration.rs`
- Test: `src/storage/usage_store.rs` unit tests
- Test: `tests/watch_integration.rs`

**Interfaces:**
- Consumes: `NormalizedUsageEvent`, `ProviderCursorUpdate`
- Produces:
  - SQLite column `usage_events.feedable INTEGER NOT NULL DEFAULT 1`
  - `UsageStore::seed_source_history(...)` writes `feedable = 0`
  - lifecycle/activity/read-model queries filter `feedable = 1`

- [ ] **Step 1: Write failing store tests for non-feedable seed rows**

Append these tests to `src/storage/usage_store.rs` inside the existing `mod tests`:

```rust
#[test]
fn seed_source_history_rows_are_non_feedable_for_activity_queries() {
    let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
    let now = datetime!(2026 - 06 - 10 12:00 UTC);
    let historical = now - time::Duration::days(1);
    let event = NormalizedUsageEvent {
        provider_surface: "claude-code".into(),
        ..NormalizedUsageEvent::for_test_at(historical, 50_000.0)
    };
    let cursor = ProviderCursorUpdate {
        provider_surface: "claude-code".into(),
        cursor_key: "seed-key".into(),
        cursor_value: "seed-value".into(),
        provider_version: "test-provider".into(),
        parser_version: "test-parser".into(),
    };

    store.seed_source_history(&[(event, cursor)], None, now).unwrap();

    assert_eq!(store.lifetime_effective_tokens().unwrap(), 0.0);
    assert_eq!(store.recent_event_count().unwrap(), 0);
    assert!(!store.has_any_applied_events().unwrap());
    assert_eq!(
        store
            .applied_effective_tokens_between(historical - time::Duration::hours(1), now)
            .unwrap(),
        0.0
    );
    assert!(store
        .applied_effective_tokens_by_source_between(historical - time::Duration::hours(1), now)
        .unwrap()
        .is_empty());
    assert!(store
        .recent_events(10)
        .unwrap()
        .is_empty());
}

#[test]
fn feedable_applied_rows_still_drive_activity_queries() {
    let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
    let now = datetime!(2026 - 06 - 10 12:00 UTC);
    store.insert_event(&NormalizedUsageEvent::for_test_at(now, 42_000.0)).unwrap();

    assert_eq!(store.recent_event_count().unwrap(), 1);
    assert!(store.has_any_applied_events().unwrap());
    assert_eq!(
        store
            .applied_effective_tokens_between(now - time::Duration::hours(1), now + time::Duration::seconds(1))
            .unwrap(),
        42_000.0
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test seed_source_history_rows_are_non_feedable_for_activity_queries
cargo test feedable_applied_rows_still_drive_activity_queries
```

Expected: the seed-history test fails because applied seeded rows are still counted by activity queries.

- [ ] **Step 3: Add the `feedable` schema column and write values**

In `src/storage/usage_store.rs`, add `feedable INTEGER NOT NULL DEFAULT 1` to the `CREATE TABLE usage_events` statement after `applied_at`.

Add migration support:

```rust
ensure_usage_event_column(
    &self.conn,
    "feedable",
    "ALTER TABLE usage_events ADD COLUMN feedable INTEGER NOT NULL DEFAULT 1;",
    "",
)?;
```

In `insert_event`, add `feedable` to the insert column list and pass `1_i64`.

In `insert_unapplied_event_bucket`, add `feedable` to the insert column list and pass `1_i64`.

In `seed_source_history`, add `feedable` to the insert column list and pass `0_i64`.

- [ ] **Step 4: Filter feedable-only read models**

In `src/storage/usage_store.rs`, add `feedable = 1` to every query whose result feeds lifecycle, pet effects, source activity, today summaries, recovery, rhythm, or visible recent usage:

```sql
WHERE feedable = 1
```

Apply the predicate to these methods:

- `recent_event_count`
- `compact_before`
- `events_within`
- `recent_events`
- `unapplied_events`
- `mark_events_applied_and_advance_cursors` select/update/counter path
- `token_totals_by_source_between`
- `applied_effective_tokens_between`
- `canonical_total_tokens_between`
- `canonical_total_tokens_by_source_between`
- `applied_effective_tokens_by_source_between`
- `applied_bucket_sums_between`
- `applied_token_shape_between`
- `latest_applied_bucket_at`
- `latest_applied_marked_at`
- `latest_applied_bucket_at_before`
- `has_any_applied_events`
- `best_day_effective_tokens`

For `daily_aggregates`, both insert and delete only feedable rows:

```sql
WHERE period_start < ?1
  AND bucket_at < ?1
  AND applied_at IS NOT NULL
  AND feedable = 1
```

Do not add a `feedable` column to `daily_aggregates` in this task. Non-feedable rows should not compact into that table.

- [ ] **Step 5: Update the existing seed-history store test**

In `seed_source_history_writes_applied_rows_and_cursors_without_feeding_lifetime`, replace the assertion that seeded history appears in `applied_effective_tokens_by_source_between` with cursor and non-feedable assertions:

```rust
assert_eq!(store.lifetime_effective_tokens().unwrap(), 0.0);
assert!(!store.has_any_applied_events().unwrap());
assert_eq!(
    store
        .provider_cursor("gemini", "gemini|daily|2026-06-09")
        .unwrap()
        .as_deref(),
    Some("totals-v1")
);
assert!(store
    .applied_effective_tokens_by_source_between(
        historical - time::Duration::hours(1),
        now + time::Duration::seconds(1),
    )
    .unwrap()
    .is_empty());
```

- [ ] **Step 6: Add watch integration coverage**

Add to `tests/watch_integration.rs`:

```rust
#[test]
fn seeded_history_is_hidden_from_watch_activity_surfaces() {
    let dir = tempfile::tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 06 - 10 12:00 UTC);
    let historical = now - time::Duration::days(1);
    let event = NormalizedUsageEvent {
        provider_surface: "codex".into(),
        ..NormalizedUsageEvent::for_test_at(historical, 669_000_000.0)
    };
    let cursor = ProviderCursorUpdate {
        provider_surface: "codex".into(),
        cursor_key: "codex-seed-key".into(),
        cursor_value: "codex-seed-value".into(),
        provider_version: "test-provider".into(),
        parser_version: "test-parser".into(),
    };
    usage_store.seed_source_history(&[(event, cursor)], None, now).unwrap();

    let vm = build_watch_view_model_for_test_at(&PetState::new_for_test("mochi-7f3a", "mochi"), &usage_db, now).unwrap();

    assert_eq!(vm.today_effective_tokens, 0.0);
    assert!(vm.source_breakdown.is_empty());
    assert!(vm.recent_events.iter().all(|event| !event.text.contains("codex")));
}
```

- [ ] **Step 7: Verify Task 3**

Run:

```bash
cargo test seed_source_history
cargo test feedable_applied_rows_still_drive_activity_queries
cargo test --test watch_integration seeded_history_is_hidden_from_watch_activity_surfaces
```

Expected: all focused tests pass.

- [ ] **Step 8: Commit Task 3**

```bash
git add src/storage/usage_store.rs tests/watch_integration.rs
git commit -m "fix(storage): keep seeded history non-feedable"
```

---

### Task 4: Per-Cursor First-Contact And Byte-Identical Cursor Keys

**Files:**
- Modify: `src/usage/ccusage.rs`
- Modify: `src/game/runtime.rs`
- Test: `tests/usage_provider.rs`
- Test: `tests/runtime_integration.rs`

**Interfaces:**
- Consumes: `UsagePollResult`, `UsageDelta`, `ProviderCursorKey`, `UsageStore::provider_cursor`
- Produces:
  - Snapshot and poll paths serialize identical `ProviderCursorKey` values.
  - First-contact seeding is per `(provider_surface, cursor_key)`.
  - Missing keys seed non-feedable history and do not skip known keys from the same surface.

- [ ] **Step 1: Write failing cursor-key byte-identity test**

Add this helper and test to `tests/usage_provider.rs`:

```rust
fn cursor_key_values(updates: &[ProviderCursorUpdate]) -> std::collections::BTreeSet<(String, String)> {
    updates
        .iter()
        .map(|update| (update.provider_surface.clone(), update.cursor_key.clone()))
        .collect()
}

#[test]
fn snapshot_and_poll_serialize_byte_identical_cursor_keys() {
    let dir = tempdir().unwrap();
    let mut snapshot_store = UsageStore::open(&dir.path().join("snapshot.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), Some("ccusage-codex-ok.mjs"));

    let snapshot = provider.snapshot_for_calibration(&mut snapshot_store).unwrap();
    let snapshot_keys = cursor_key_values(&snapshot.cursor_updates);

    let mut poll_store = UsageStore::open(&dir.path().join("poll.sqlite")).unwrap();
    let poll = provider.poll(&mut poll_store).unwrap();
    let poll_keys = poll
        .deltas
        .iter()
        .map(|delta| {
            (
                delta.cursor_update.provider_surface.clone(),
                delta.cursor_update.cursor_key.clone(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(snapshot_keys, poll_keys);
    for (_, key) in snapshot_keys {
        let parsed: ProviderCursorKey = serde_json::from_str(&key).unwrap();
        assert_eq!(
            parsed.token_contract.as_deref(),
            Some(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1)
        );
    }
}
```

- [ ] **Step 2: Run cursor-key test to verify it fails**

Run:

```bash
cargo test --test usage_provider snapshot_and_poll_serialize_byte_identical_cursor_keys
```

Expected: failure because `token_contract` is currently omitted from cursor keys.

- [ ] **Step 3: Add one cursor-key builder in `ccusage.rs`**

In `src/usage/ccusage.rs`, add a helper near `cursor_key`:

```rust
fn provider_cursor_key_for_record(
    record: &NormalizedUsageRecord,
    command_name: &str,
) -> ProviderCursorKey {
    ProviderCursorKey {
        provider_surface: record.source_identity.provider_surface.clone(),
        token_contract: Some(TOKENMAXXING_TOTAL_V1.to_string()),
        command: command_name.to_string(),
        source_surface: "daily".to_string(),
        period_start: record.period_start.clone(),
        model: record.model.clone(),
        raw_source_id: None,
    }
}
```

Replace the inline `ProviderCursorKey` literals in both `poll_helper` and `snapshot_helper` with:

```rust
let key = provider_cursor_key_for_record(&record, command_name);
```

Keep `cursor_update.provider_surface = record.source_identity.provider_surface.clone()` in both paths.

- [ ] **Step 4: Write failing per-cursor first-contact runtime test**

Add to `tests/runtime_integration.rs`:

```rust
#[test]
fn first_contact_is_per_cursor_key_not_entire_surface() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 800_000.0;

    usage_store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: "claude-code".into(),
                cursor_key: "known-key".into(),
                cursor_value: "old-known".into(),
                provider_version: "test-provider".into(),
                parser_version: "test-parser".into(),
            }],
            now - Duration::minutes(10),
        )
        .unwrap();

    let mut known = usage_delta_with_cursor("claude-code", "known-key", "new-known", 100_000.0, now);
    known.period_start = now;
    let mut missing = usage_delta_with_cursor("claude-code", "missing-key", "seeded-missing", 700_000.0, now);
    missing.period_start = now - Duration::days(1);

    let poll = UsagePollResult {
        deltas: vec![known, missing],
        diagnostics: Vec::new(),
        total_effective_tokens: 800_000.0,
        total_tokens: 800_000.0,
    };

    stage_usage_poll_deltas(&mut usage_store, &poll, &mut state, DISCONTINUITY_GUARD_RATIO, now).unwrap();
    let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();

    assert_eq!(state.lifetime_effective_tokens, 100_000.0);
    assert_eq!(state.stage, Stage::S1);
    assert_eq!(
        usage_store.provider_cursor("claude-code", "missing-key").unwrap().as_deref(),
        Some("seeded-missing")
    );
    assert!(usage_store
        .recent_diagnostics(5)
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic.code == glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE));
}
```

Add this test helper near existing runtime helpers:

```rust
fn usage_delta_with_cursor(
    provider_surface: &str,
    cursor_key: &str,
    cursor_value: &str,
    total_tokens: f64,
    now: time::OffsetDateTime,
) -> UsageDelta {
    let sequence = POLL_COUNTER.fetch_add(1, Ordering::Relaxed);
    UsageDelta {
        provider_surface: provider_surface.into(),
        source_identity: SourceIdentity::from_provider_surface(provider_surface),
        command: "ccusage".into(),
        effective_tokens: total_tokens,
        total_tokens,
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        confidence: "local-log-derived".into(),
        period_start: now + Duration::seconds(sequence as i64),
        observed_at: now,
        model: Some("claude-sonnet-4".into()),
        cursor_update: ProviderCursorUpdate {
            provider_surface: provider_surface.into(),
            cursor_key: cursor_key.into(),
            cursor_value: cursor_value.into(),
            provider_version: "test-provider".into(),
            parser_version: "test-parser".into(),
        },
        token_totals: Some(RawTokenTotals {
            uncached_input: total_tokens as u64,
            output: 0,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        }),
    }
}
```

- [ ] **Step 5: Run per-cursor test to verify it fails**

Run:

```bash
cargo test --test runtime_integration first_contact_is_per_cursor_key_not_entire_surface
```

Expected: failure because first contact is currently keyed by `latest_cursor_updated_at(surface)` and can skip or feed at the surface level.

- [ ] **Step 6: Refactor runtime staging to skip by cursor key**

In `src/game/runtime.rs`, replace the `BTreeSet<String>` surface skip with a cursor-key skip:

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CursorSkipKey {
    provider_surface: String,
    cursor_key: String,
}

fn cursor_skip_key(delta: &UsageDelta) -> CursorSkipKey {
    CursorSkipKey {
        provider_surface: delta.cursor_update.provider_surface.clone(),
        cursor_key: delta.cursor_update.cursor_key.clone(),
    }
}
```

Change `handle_first_contact_and_discontinuity` to return `BTreeSet<CursorSkipKey>`.

Inside that function:

1. For each delta, call `usage_store.provider_cursor(&delta.cursor_update.provider_surface, &delta.cursor_update.cursor_key)?`.
2. If the stored value equals `delta.cursor_update.cursor_value`, insert the cursor key into the skip set.
3. If no stored value exists, collect the delta for first-contact seeding and insert the cursor key into the skip set.
4. Only deltas with an existing cursor and a different value participate in discontinuity sums.
5. Call `seed_first_contact_surface` for each surface's missing-key deltas.

In `stage_usage_poll_deltas`, replace the current surface skip check with:

```rust
if skip_cursors.contains(&cursor_skip_key(delta)) {
    continue;
}
```

Keep the existing exact-cursor-value skip in the staging loop until the new classification is verified, then remove the duplicate check only if the tests still pass.

- [ ] **Step 7: Verify Task 4**

Run:

```bash
cargo test --test usage_provider
cargo test --test runtime_integration first_contact_is_per_cursor_key_not_entire_surface
```

Expected: all focused tests pass.

- [ ] **Step 8: Commit Task 4**

```bash
git add src/usage/ccusage.rs src/game/runtime.rs tests/usage_provider.rs tests/runtime_integration.rs
git commit -m "fix(usage): seed first contact per provider cursor"
```

---

### Task 5: Init And Reinit Cache Replacement

**Files:**
- Modify: `src/commands/init.rs`
- Modify: `src/usage/cutover.rs`
- Modify: `tests/cli_smoke.rs`
- Test: `tests/cli_smoke.rs`
- Test: `tests/usage_provider.rs`

**Interfaces:**
- Consumes: `AppPaths`, `UsageStore`, `CcusageCommandProvider::snapshot_for_calibration`
- Produces:
  - Clean init still saves S0, zero XP, zero lifetime tokens.
  - Confirmed `init --yes` removes the previous usage cache before creating the replacement pet.
  - Cursor seeding failure aborts before saving the newborn pet.

- [ ] **Step 1: Rewrite the reinit CLI smoke test to expect cache replacement**

In `tests/cli_smoke.rs`, replace `init_with_confirmed_reinit_replaces_pet_state_without_touching_usage_db` with:

```rust
#[test]
fn init_with_confirmed_reinit_replaces_pet_state_and_resets_usage_db() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();
    std::fs::write(dir.path().join("usage.sqlite"), "sentinel usage db").unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "ori-shard", "--name", "ori", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ori has hatched"));

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("ori-shard"));

    let usage_store =
        glorp::storage::usage_store::UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    assert_eq!(usage_store.recent_event_count().unwrap(), 0);
}
```

- [ ] **Step 2: Run reinit test to verify it fails**

Run:

```bash
cargo test --test cli_smoke init_with_confirmed_reinit_replaces_pet_state_and_resets_usage_db
```

Expected: failure because current `init --yes` leaves the sentinel usage DB untouched.

- [ ] **Step 3: Implement reset-style cache replacement in init**

In `src/commands/init.rs`, compute whether an existing pet is being replaced:

```rust
let replacing_existing = store.load()?.is_some();
if replacing_existing && !yes {
    return Err(GlorpError::Message(
        "glorp already has a pet; pass --yes to replace pet state".into(),
    ));
}
```

Before opening `UsageStore`, remove the usage DB when `replacing_existing && yes`:

```rust
if replacing_existing && yes && paths.usage_db.exists() {
    std::fs::remove_file(&paths.usage_db).map_err(|err| {
        GlorpError::Message(format!(
            "failed to remove usage cache at {}: {err}",
            paths.usage_db.display()
        ))
    })?;
}
```

Keep the provider snapshot behavior conservative:

```rust
let mut calibration = CalibrationBaseline::default();
let mut rhythm = RhythmProfile::default();
let usage_store = match UsageStore::open(&paths.usage_db) {
    Ok(store) => Some(store),
    Err(err) if replacing_existing && yes => return Err(err),
    Err(_) => None,
};
if let Some(mut usage_store) = usage_store {
    if let Ok(snapshot) =
        CcusageCommandProvider::from_environment().snapshot_for_calibration(&mut usage_store)
    {
        let now = OffsetDateTime::now_utc();
        usage_store.advance_cursors(snapshot.cursor_updates, now)?;
        if snapshot.diagnostics.is_empty() {
            calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
            rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
            usage_store.mark_token_contract_active(
                crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                now,
            )?;
        }
    }
}
```

Do not save the new `PetState` until after usage-cache removal and cursor advancement have completed.

- [ ] **Step 4: Refresh calibration with clamp in cutover**

In `src/usage/cutover.rs`, replace direct baseline assignment:

```rust
state.calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
```

with:

```rust
state.calibration = state.calibration.refresh_from_history(&snapshot.daily_usage);
```

Leave clean init using `CalibrationBaseline::from_history`, because clean init has no previous persisted baseline to clamp against.

- [ ] **Step 5: Verify init no-feed and cursor seeding behavior**

Run:

```bash
cargo test --test cli_smoke
cargo test --test usage_provider snapshot_for_calibration_returns_daily_usage_without_inserting_events
```

Expected: all focused tests pass.

- [ ] **Step 6: Commit Task 5**

```bash
git add src/commands/init.rs src/usage/cutover.rs tests/cli_smoke.rs
git commit -m "fix(init): reset usage cache on confirmed reinit"
```

---

### Task 6: Docs, Full Acceptance, And Regression Sweep

**Files:**
- Modify: `README.md`
- Modify: `npm/glorp/README.md`
- Modify tests only if a full-suite failure reveals an old assertion tied to the previous curve.

**Interfaces:**
- Consumes: completed Tasks 1-5.
- Produces: user-facing wording that avoids real-clock gating language and a clean verification bundle.

- [ ] **Step 1: Update README wording**

In `README.md`, replace the current stage wording:

```markdown
Stages are gated by **calibrated XP**: roughly "one active day at your typical pace." A 500M-token/day user and a 50k-token/day user evolve at the same wall-clock cadence.
```

with:

```markdown
Stages grow from calibrated Tokenmaxxing `total_tokens`: Glorp compares new work against your recent active-day baseline. Early stages are active-hour equivalents, not real-time locks, and historical usage calibrates a newborn pet without feeding it.
```

In `npm/glorp/README.md`, add the same sentence after the first paragraph that mentions Tokenmaxxing-style totals.

- [ ] **Step 2: Run focused acceptance tests**

Run:

```bash
cargo test --test game_rules
cargo test --test runtime_integration
cargo test --test usage_provider
cargo test --test cli_smoke
cargo test --test watch_integration
cargo test --lib
```

Expected: all tests pass.

- [ ] **Step 3: Run repo formatting and lint checks**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: both commands pass.

- [ ] **Step 4: Run final full Rust test suite**

Run:

```bash
cargo test --all-targets --all-features
```

Expected: all tests pass.

- [ ] **Step 5: Inspect final diff for accidental scope creep**

Run:

```bash
git status --short
git diff --stat HEAD
git diff --check
```

Expected:

- Only files from this plan are modified.
- `src/pet/render.rs` remains untouched by this work.
- `docs/superpowers/specs/2026-07-02-glitch-persistent-corruption-design.md` remains untouched by this work.
- `git diff --check` exits 0.

- [ ] **Step 6: Commit Task 6**

```bash
git add README.md npm/glorp/README.md
git commit -m "docs: clarify calibrated evolution pacing"
```

---

## Implementation Order

1. Task 1: baseline calibration.
2. Task 2: XP thresholds and progress helpers.
3. Task 3: feedable ledger semantics.
4. Task 4: per-cursor first contact and cursor key exactness.
5. Task 5: init/reinit cache replacement and cutover clamp.
6. Task 6: docs and verification sweep.

This order keeps tests meaningful: math first, storage semantics next, runtime/provider behavior after storage can represent non-feedable history, then command-level flows.

## Self-Review Checklist

- [ ] Every spec acceptance bullet maps to at least one task:
  - Baseline grouping/latest-30/median/default/clamp: Task 1.
  - `TOKENMAXXING_TOTAL_V1` lifecycle paths: Tasks 1, 2, 4.
  - `B`, `H = 8`, active-hour thresholds: Task 2.
  - Smear buckets do not change lifecycle XP: Task 2.
  - Clean `init -> status` no feed/no first-contact for primary fixtures: Tasks 4 and 5.
  - Byte-identical `ProviderCursorKey`: Task 4.
  - Seeded historical rows excluded from lifecycle/activity/today/source views: Task 3.
  - Confirmed `init --yes` reset-style cache replacement: Task 5.
  - Backfill/catchup clipped rows do not grant unbounded lifecycle progress: Tasks 2 and 3.
  - README wording: Task 6.
- [ ] Plan contains no placeholder words or deferred implementation markers.
- [ ] Shared helper names are consistent across tasks: `refresh_from_history`, `ACTIVE_HOURS_PER_DAY`, `stage_start_xp`, `next_stage_xp_target`, `feedable`.
- [ ] No task requires touching unrelated dirty files.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-02-glorp-calibrated-evolution-curve-implementation.md`. Two execution options:

1. **Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

Which approach?
