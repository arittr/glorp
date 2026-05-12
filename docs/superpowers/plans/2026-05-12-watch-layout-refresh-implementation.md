# Watch Layout Refresh — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reshape `glorp watch` so every region carries meaningful content. Surface stage progress as a first-class panel, give each vital its own color, anchor an inline bio under vitals, drop redundant panels, and prepare the pet column to be filled with species-flavored habitat ambient motion.

**Architecture:** The watch pipeline stays unchanged at the top level (`Frame → outer chrome → inner Rect → wide/compact layout → panels`). What changes:
- `WatchViewModel` gains `progress: ProgressView` and `bio: BioView`.
- Two new panels (`ProgressPanel`, `BioCardPanel`), one new shared module (`bars`), one new color set in `style.rs`, two panels deleted (`spark`, `helpers`).
- `layout.rs` rebuilds the wide + compact constraint sequences so the left column anchors vitals/bio to the bottom while the pet panel uses `Fill(1)` to absorb slack; the right column packs to the top with a bounded `Length` feed (`MAX_EVENT_ROWS = 6`).
- Two PRs: **PR1** ships layout refactor + color + Fill (habitat stub returns empty Vec). **PR2** fills in `ambient_glyphs_for` per-species and wall-clock drift.

**Tech Stack:** Rust 2021, ratatui 0.x, rusqlite, time crate. Tests via `cargo test` (`assert_cmd` for integration, `TestBackend` for panel snapshots).

**Source spec:** [docs/superpowers/specs/2026-05-12-watch-layout-refresh-design.md](../specs/2026-05-12-watch-layout-refresh-design.md) — revision 3.

---

## File Structure

**New files:**
- `src/tui/panels/bars.rs` — shared `bar_spans_solid`, `bar_spans_ramped`, `build_spark_line`, `format_tokens_full`, `format_tokens_short`
- `src/tui/panels/progress.rs` — `ProgressPanel`
- `src/tui/panels/bio_card.rs` — `BioCardPanel`

**Deleted files:**
- `src/tui/panels/spark.rs` — logic absorbed into `bars` + today footer
- `src/tui/panels/helpers.rs` — signal absorbed into today's `⚠` marker

**Modified files:**
- `src/tui/style.rs` — add 6 color role functions (fed/happy/energy/xp/claude/codex)
- `src/tui/view_model.rs` — add `ProgressView`, `BioView`, plumbing on `WatchViewModel`
- `src/tui/panels/mod.rs` — register new modules, drop removed ones
- `src/tui/panels/pet.rs` — 2-pass paint scaffolding, `Fill(1)` preferred constraint, no-op `ambient_glyphs_for` stub
- `src/tui/panels/vitals.rs` — drop xp row; each row uses its stat color via new bars helper
- `src/tui/panels/today.rs` — add 7-day inline footer, `⚠` marker, source colors
- `src/tui/panels/feed.rs` — `MAX_EVENT_ROWS = 6`, source label colors
- `src/tui/layout.rs` — drop SparkPanel + HelpersPanel; reorder right column; new constraint sequences; pet_panel_rect update
- `src/commands/watch.rs` — compute `ProgressView` + `BioView` + EMA rate
- `src/storage/usage_store.rs` — add `events_within`, `best_day_effective_tokens`, fix `seven_day_token_history`
- `tests/tui_render.rs` — delete helper-assertions, update xp/compact tests
- `tests/watch_integration.rs` — add bio/⚠/EMA integration cases

---

## Phase 0 — Storage layer

### Task 1: Add `events_within(duration)` query

**Files:**
- Modify: `src/storage/usage_store.rs`

- [ ] **Step 1: Write the failing test**

Add inside the existing `#[cfg(test)] mod tests { ... }` block in `src/storage/usage_store.rs`:

```rust
#[test]
fn events_within_returns_events_inside_window_only() {
    let store = UsageStore::open_in_memory().unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let inside_a  = sample_event_at(now - Duration::minutes(10), "claude-code", 1_000.0);
    let inside_b  = sample_event_at(now - Duration::hours(1),    "codex",       2_000.0);
    let outside   = sample_event_at(now - Duration::hours(3),    "claude-code", 9_999.0);
    for e in [&inside_a, &inside_b, &outside] {
        store.insert_event(e).unwrap();
    }
    let got = store.events_within(Duration::hours(2), now).unwrap();
    let totals: Vec<f64> = got.iter().map(|e| e.effective_tokens).collect();
    assert!(totals.contains(&1_000.0), "inside_a must be present");
    assert!(totals.contains(&2_000.0), "inside_b must be present");
    assert!(!totals.contains(&9_999.0), "outside must be excluded");
}

#[test]
fn events_within_boundary_is_inclusive_at_lower_bound() {
    let store = UsageStore::open_in_memory().unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let on_boundary = sample_event_at(now - Duration::hours(2), "codex", 5_555.0);
    store.insert_event(&on_boundary).unwrap();
    let got = store.events_within(Duration::hours(2), now).unwrap();
    assert_eq!(got.len(), 1, "boundary event must be included (>= comparison)");
}
```

If `sample_event_at` doesn't already exist, add this helper near the top of the test module:

```rust
fn sample_event_at(observed_at: OffsetDateTime, source: &str, tokens: f64) -> NormalizedUsageEvent {
    NormalizedUsageEvent {
        observed_at,
        provider: source.to_string(),
        period_date: observed_at.date().to_string(),
        provider_delta_id: format!("{source}-{}", observed_at.unix_timestamp()),
        bucket_index: 0,
        effective_tokens: tokens,
        cost_usd: 0.0,
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage_store::tests::events_within -- --nocapture`
Expected: FAIL — `no method named events_within`.

- [ ] **Step 3: Implement `events_within`**

Add a public method on `UsageStore` (next to the existing `recent_events` near `usage_store.rs:444`):

```rust
pub fn events_within(
    &self,
    duration: Duration,
    now: OffsetDateTime,
) -> Result<Vec<NormalizedUsageEvent>> {
    let cutoff = (now - duration).unix_timestamp();
    let mut stmt = self.conn.prepare(
        "SELECT observed_at, provider, period_date, provider_delta_id,
                bucket_index, effective_tokens, cost_usd
         FROM usage_events
         WHERE observed_at >= ?1
         ORDER BY observed_at DESC",
    )?;
    let rows = stmt.query_map([cutoff], |row| {
        Ok(NormalizedUsageEvent {
            observed_at: OffsetDateTime::from_unix_timestamp(row.get(0)?).unwrap(),
            provider: row.get(1)?,
            period_date: row.get(2)?,
            provider_delta_id: row.get(3)?,
            bucket_index: row.get(4)?,
            effective_tokens: row.get(5)?,
            cost_usd: row.get(6)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}
```

Verify the actual `usage_events` column names + types match by reading the migration around the top of `usage_store.rs` (`CREATE TABLE usage_events`). If the row mapping needs different `row.get` types, fix it to match.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage_store::tests::events_within -- --nocapture`
Expected: PASS (both cases).

- [ ] **Step 5: Commit**

```bash
git add src/storage/usage_store.rs
git commit -m "feat(storage): add events_within(duration) query

Used by the new EMA rate calculation in ProgressView (next commits).
Replaces recent_events(500) for time-window use cases where the 500-row
cap would silently truncate the tail."
```

---

### Task 2: Add `best_day_effective_tokens()` query

**Files:**
- Modify: `src/storage/usage_store.rs`

- [ ] **Step 1: Write the failing test**

Add to the same test module:

```rust
#[test]
fn best_day_returns_largest_daily_total_from_events_only() {
    let store = UsageStore::open_in_memory().unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    store.insert_event(&sample_event_at(now, "claude-code", 3_000.0)).unwrap();
    store.insert_event(&sample_event_at(now, "codex",       2_000.0)).unwrap();
    store.insert_event(&sample_event_at(now - Duration::days(1), "claude-code", 4_000.0)).unwrap();
    let best = store.best_day_effective_tokens().unwrap();
    assert_eq!(best, 5_000.0, "today sums to 5k, beats yesterday's 4k");
}

#[test]
fn best_day_returns_zero_when_empty() {
    let store = UsageStore::open_in_memory().unwrap();
    assert_eq!(store.best_day_effective_tokens().unwrap(), 0.0);
}

#[test]
fn best_day_sums_overlap_between_events_and_aggregates() {
    // Compaction window: same period_date appears in both tables.
    let store = UsageStore::open_in_memory().unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let period_date = now.date().to_string();
    store.insert_event(&sample_event_at(now, "claude-code", 1_000.0)).unwrap();
    store
        .upsert_daily_aggregate(&period_date, "claude-code", 2_000.0, 0.0)
        .unwrap();
    let best = store.best_day_effective_tokens().unwrap();
    assert_eq!(best, 3_000.0, "events 1k + aggregate 2k = 3k");
}
```

If `upsert_daily_aggregate` has a different signature, adjust the call to match — read the existing API in `usage_store.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage_store::tests::best_day -- --nocapture`
Expected: FAIL — `no method named best_day_effective_tokens`.

- [ ] **Step 3: Implement `best_day_effective_tokens`**

Add public method on `UsageStore`:

```rust
pub fn best_day_effective_tokens(&self) -> Result<f64> {
    let mut stmt = self.conn.prepare(
        "SELECT COALESCE(MAX(daily_total), 0.0) FROM (
            SELECT period_date, SUM(effective_tokens) AS daily_total
            FROM (
                SELECT period_date, effective_tokens FROM usage_events
                UNION ALL
                SELECT period_date, effective_tokens FROM daily_aggregates
            )
            GROUP BY period_date
        )",
    )?;
    let best: f64 = stmt.query_row([], |row| row.get(0))?;
    Ok(best)
}
```

`UNION ALL` is intentional — duplicates between events and aggregates must NOT be deduped (they represent legitimately separate accumulations during the compaction overlap).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage_store::tests::best_day -- --nocapture`
Expected: PASS (all three cases).

- [ ] **Step 5: Commit**

```bash
git add src/storage/usage_store.rs
git commit -m "feat(storage): add best_day_effective_tokens()

UNION ALL + SUM per period_date so the compaction overlap window
sums correctly rather than picking max of either side. Added for
completeness; not consumed by the bio card in this revision."
```

---

### Task 3: Fix `seven_day_token_history` to be aggregate-aware

**Files:**
- Modify: `src/storage/usage_store.rs:734-...` (the existing function)

- [ ] **Step 1: Write the failing test**

Find the existing `seven_day_token_history_*` tests around line 956. Add a new test:

```rust
#[test]
fn seven_day_token_history_includes_compacted_days() {
    let store = UsageStore::open_in_memory().unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    // Day -6 only in daily_aggregates (simulating post-compaction).
    let six_days_ago = (now - Duration::days(6)).date().to_string();
    store
        .upsert_daily_aggregate(&six_days_ago, "claude-code", 7_777.0, 0.0)
        .unwrap();
    // Today only in usage_events.
    store.insert_event(&sample_event_at(now, "codex", 1_234.0)).unwrap();
    let history = store.seven_day_token_history(now).unwrap();
    assert_eq!(history.len(), 7);
    assert!(history[0] > 7_700.0, "day-6 (oldest) must surface from aggregates");
    assert!(history[6] > 1_200.0, "today (newest) must surface from events");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib usage_store::tests::seven_day_token_history_includes_compacted_days -- --nocapture`
Expected: FAIL — day-6 returns 0.0 because the current query reads only `usage_events`.

- [ ] **Step 3: Rewrite the SQL inside `seven_day_token_history`**

Open `seven_day_token_history` (around line 734). Replace the inner SQL query with the same `UNION ALL` pattern used in Task 2. Keep the existing return shape (`[f64; 7]` or `Vec<f64>` — match what's already there). New body:

```rust
pub fn seven_day_token_history(
    &self,
    now: OffsetDateTime,
) -> Result<Vec<f64>> {
    // 7 day buckets, oldest first (index 0 = 6 days ago, index 6 = today).
    let mut totals: Vec<f64> = vec![0.0; 7];
    let mut stmt = self.conn.prepare(
        "SELECT period_date, SUM(effective_tokens) AS daily_total
         FROM (
             SELECT period_date, effective_tokens FROM usage_events
             UNION ALL
             SELECT period_date, effective_tokens FROM daily_aggregates
         )
         WHERE period_date >= ?1
         GROUP BY period_date",
    )?;
    let earliest = (now - Duration::days(6)).date().to_string();
    let rows = stmt.query_map([&earliest], |row| {
        let date: String = row.get(0)?;
        let total: f64 = row.get(1)?;
        Ok((date, total))
    })?;
    for row in rows {
        let (date_str, total) = row?;
        let date = Date::parse(&date_str, &Iso8601::DEFAULT).map_err(|e| {
            Error::Database(format!("bad period_date {date_str}: {e}"))
        })?;
        let days_ago = (now.date() - date).whole_days();
        if (0..=6).contains(&days_ago) {
            let idx = (6 - days_ago) as usize;
            totals[idx] = total;
        }
    }
    Ok(totals)
}
```

If the file already imports `Date` and `Iso8601`, you're set; if not, add `use time::{Date, format_description::well_known::Iso8601};`. If the existing return type was `[f64; 7]`, convert with `let arr: [f64; 7] = totals.try_into().unwrap(); Ok(arr)`. Read the existing function signature first to match.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib usage_store::tests::seven_day_token_history -- --nocapture`
Expected: PASS for the new case AND all existing seven_day tests.

- [ ] **Step 5: Commit**

```bash
git add src/storage/usage_store.rs
git commit -m "fix(storage): seven_day_token_history reads aggregates too

Previously the query hit usage_events only, so days beyond compaction
(default 90d retention) returned 0 even though daily_aggregates had
the row. Use the same UNION ALL pattern as best_day_effective_tokens.
The new 7-day inline strip in TodayPanel depends on this."
```

---

## Phase 1 — View model types

### Task 4: Add `ProgressView` struct

**Files:**
- Modify: `src/tui/view_model.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `src/tui/view_model.rs`:

```rust
#[test]
fn progress_view_has_required_fields() {
    let p = ProgressView {
        stage_label: "shard".to_string(),
        next_stage_label: "fractal".to_string(),
        fraction: 0.33,
        xp_in_stage: 0.33,
        xp_to_next: 1.0,
        rate_per_hour: 109_000.0,
        is_max_stage: false,
    };
    assert_eq!(p.stage_label, "shard");
    assert_eq!(p.next_stage_label, "fractal");
    assert!((p.fraction - 0.33).abs() < 1e-6);
    assert!(!p.is_max_stage);
}

#[test]
fn progress_view_at_max_stage() {
    let p = ProgressView {
        stage_label: "aurora".to_string(),
        next_stage_label: "—".to_string(),
        fraction: 1.0,
        xp_in_stage: 60.0,
        xp_to_next: 60.0,
        rate_per_hour: 0.0,
        is_max_stage: true,
    };
    assert!(p.is_max_stage);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib view_model::tests::progress_view -- --nocapture`
Expected: FAIL — `cannot find type ProgressView`.

- [ ] **Step 3: Add the struct**

In `src/tui/view_model.rs`, near the existing struct definitions:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressView {
    /// Current stage's species-specific label (e.g. "shard", "fractal").
    pub stage_label: String,
    /// Next stage's label, or "—" at S6.
    pub next_stage_label: String,
    /// 0.0..=1.0; saturates at 1.0.
    pub fraction: f32,
    /// state.xp - stage_start_xp(state.stage), in stage-progress units.
    pub xp_in_stage: f64,
    /// next_stage_xp_target(state.stage), in stage-progress units.
    pub xp_to_next: f64,
    /// 6h-half-life EMA, effective tokens / hour.
    pub rate_per_hour: f64,
    /// True at S6; ProgressPanel renders "max evolved" instead of a bar.
    pub is_max_stage: bool,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib view_model::tests::progress_view -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/view_model.rs
git commit -m "feat(view_model): add ProgressView struct"
```

---

### Task 5: Add `BioView` struct + age_label formatter

**Files:**
- Modify: `src/tui/view_model.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn bio_view_age_label_sub_day_formats_as_hours() {
    assert_eq!(BioView::format_age(Duration::ZERO),           "0d 0h");
    assert_eq!(BioView::format_age(Duration::hours(1)),       "0d 1h");
    assert_eq!(BioView::format_age(Duration::hours(23)),      "0d 23h");
}

#[test]
fn bio_view_age_label_day_or_more_drops_hours() {
    assert_eq!(BioView::format_age(Duration::hours(24)),      "1d");
    assert_eq!(BioView::format_age(Duration::hours(25)),      "1d");
    assert_eq!(BioView::format_age(Duration::days(7)),        "7d");
    assert_eq!(BioView::format_age(Duration::days(90)),       "90d");
    assert_eq!(BioView::format_age(Duration::days(365)),      "365d");
}
```

Add `use time::Duration;` at the top of the test module if not already imported.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib view_model::tests::bio_view -- --nocapture`
Expected: FAIL — `cannot find type BioView`.

- [ ] **Step 3: Add the struct + formatter**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BioView {
    /// Pre-formatted "may 11 04:00" — local TZ, computed at vm-build time.
    pub hatched_label: String,
    /// "0d 4h" if < 24h, otherwise "12d".
    pub age_label: String,
}

impl BioView {
    pub fn format_age(age: time::Duration) -> String {
        let total_hours = age.whole_hours();
        if total_hours < 24 {
            format!("0d {total_hours}h")
        } else {
            let days = age.whole_days();
            format!("{days}d")
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib view_model::tests::bio_view -- --nocapture`
Expected: PASS (both cases).

- [ ] **Step 5: Commit**

```bash
git add src/tui/view_model.rs
git commit -m "feat(view_model): add BioView struct with age_label formatter"
```

---

### Task 6: Plumb `progress` + `bio` onto `WatchViewModel` and fixtures

**Files:**
- Modify: `src/tui/view_model.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn watch_view_model_fixture_has_progress_and_bio() {
    let vm = WatchViewModel::fixture();
    assert!(!vm.progress.stage_label.is_empty(), "progress.stage_label must be non-empty in fixture");
    assert!(!vm.bio.hatched_label.is_empty(), "bio.hatched_label must be non-empty in fixture");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib view_model::tests::watch_view_model_fixture_has_progress_and_bio -- --nocapture`
Expected: FAIL — `no field 'progress' / 'bio' on WatchViewModel`.

- [ ] **Step 3: Add fields and update fixtures**

In the `WatchViewModel` struct definition, add:

```rust
pub progress: ProgressView,
pub bio: BioView,
```

In `WatchViewModel::fixture()` and `WatchViewModel::fixture_with_events(...)` (find these in the same file), add sensible defaults:

```rust
progress: ProgressView {
    stage_label: "fuzz".to_string(),
    next_stage_label: "archfuzz".to_string(),
    fraction: 0.61,
    xp_in_stage: 8.5,
    xp_to_next: 14.0,
    rate_per_hour: 109_000.0,
    is_max_stage: false,
},
bio: BioView {
    hatched_label: "apr 24 14:32".to_string(),
    age_label: "18d".to_string(),
},
```

- [ ] **Step 4: Run test to verify it passes + ensure all callers compile**

Run: `cargo build --tests`
Expected: builds cleanly. If any external caller constructs `WatchViewModel` directly (not via fixtures), compilation will fail there — add the same defaults at each construction site.

Run: `cargo test --lib view_model::tests -- --nocapture`
Expected: PASS for the new test and all existing view-model tests.

- [ ] **Step 5: Commit**

```bash
git add src/tui/view_model.rs
git commit -m "feat(view_model): plumb progress + bio onto WatchViewModel

Fixtures populate sensible defaults so snapshot tests don't break."
```

---

## Phase 2 — Watch builder: EMA + ProgressView + BioView

### Task 7: Add `progress_rate_ema` helper

**Files:**
- Modify: `src/commands/watch.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `src/commands/watch.rs`:

```rust
#[test]
fn progress_rate_ema_empty_returns_zero() {
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    assert_eq!(progress_rate_ema(&[], now), 0.0);
}

#[test]
fn progress_rate_ema_single_event_at_now_is_tokens_over_tau() {
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let event = sample_event_at_for_test(now, "claude-code", 100_000.0);
    let rate = progress_rate_ema(&[event], now);
    // tau ≈ 6h / ln(2) ≈ 8.656h, so rate ≈ 100k / 8.656 ≈ 11.5k
    let tau = 6.0 / 2.0_f64.ln();
    let expected = 100_000.0 / tau;
    assert!((rate - expected).abs() < 1e-3, "got {rate}, want {expected}");
}

#[test]
fn progress_rate_ema_event_six_hours_old_is_weighted_half() {
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let recent = sample_event_at_for_test(now, "claude-code", 100_000.0);
    let aged   = sample_event_at_for_test(now - Duration::hours(6), "codex", 100_000.0);
    let rate_recent_only = progress_rate_ema(&[recent.clone()], now);
    let rate_both        = progress_rate_ema(&[recent, aged], now);
    let aged_contribution = rate_both - rate_recent_only;
    let expected_half = rate_recent_only * 0.5;
    assert!(
        (aged_contribution - expected_half).abs() / expected_half < 0.05,
        "6h-old contribution should be ~half (within 5%): got {aged_contribution}, want {expected_half}"
    );
}

#[test]
fn progress_rate_ema_50k_events_does_not_overflow() {
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let events: Vec<NormalizedUsageEvent> = (0..50_000)
        .map(|i| sample_event_at_for_test(now - Duration::seconds(i), "claude-code", 100.0))
        .collect();
    let rate = progress_rate_ema(&events, now);
    assert!(rate.is_finite() && rate > 0.0);
}

fn sample_event_at_for_test(observed_at: OffsetDateTime, source: &str, tokens: f64) -> NormalizedUsageEvent {
    NormalizedUsageEvent {
        observed_at,
        provider: source.to_string(),
        period_date: observed_at.date().to_string(),
        provider_delta_id: format!("{source}-{}", observed_at.unix_timestamp()),
        bucket_index: 0,
        effective_tokens: tokens,
        cost_usd: 0.0,
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib commands::watch::tests::progress_rate_ema -- --nocapture`
Expected: FAIL — `cannot find function progress_rate_ema`.

- [ ] **Step 3: Implement `progress_rate_ema`**

In `src/commands/watch.rs`, add a private function near the existing `current_bucket_effective_tokens` (around line 333):

```rust
/// 6h-half-life EMA of effective tokens, returned in tokens/hour.
///
/// Properties: monotonic increase during active use, smooth decay during idle,
/// no persisted state. See spec "EMA rate" for burst-behavior caveat.
fn progress_rate_ema(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    const TAU_HOURS: f64 = 6.0 / std::f64::consts::LN_2;
    let weighted: f64 = events
        .iter()
        .map(|e| {
            let dt_h = (now - e.observed_at).as_seconds_f64() / 3600.0;
            e.effective_tokens * (-dt_h / TAU_HOURS).exp()
        })
        .sum();
    weighted / TAU_HOURS
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib commands::watch::tests::progress_rate_ema -- --nocapture`
Expected: PASS (all four cases).

- [ ] **Step 5: Commit**

```bash
git add src/commands/watch.rs
git commit -m "feat(watch): add progress_rate_ema helper

6h-half-life EMA over effective tokens, returns tokens/hour. Used by
the next commit to populate ProgressView.rate_per_hour."
```

---

### Task 8: Build `ProgressView` in `build_watch_view_model_at`

**Files:**
- Modify: `src/commands/watch.rs`

- [ ] **Step 1: Write the failing test**

Add a test that exercises the builder end-to-end:

```rust
#[test]
fn build_watch_view_model_populates_progress_view() {
    use crate::pet::Species;
    use crate::storage::PetState;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let store = UsageStore::open(&db_path).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    store.insert_event(&sample_event_at_for_test(now, "claude-code", 50_000.0)).unwrap();
    drop(store);

    let mut state = PetState::seeded("test");
    state.species = Species::Fuzz;
    state.stage = crate::pet::Stage::S4;
    state.xp = 8.5; // 61% toward S5 target of 14.0
    state.name = "Mochi".to_string();

    let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
    assert_eq!(vm.progress.stage_label, "fuzz");
    assert_eq!(vm.progress.next_stage_label, "archfuzz");
    assert!(vm.progress.fraction > 0.5 && vm.progress.fraction < 0.7);
    assert!(vm.progress.rate_per_hour > 0.0);
    assert!(!vm.progress.is_max_stage);
}

#[test]
fn build_watch_view_model_progress_at_s6_is_max_stage() {
    use crate::pet::Species;
    use crate::storage::PetState;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    UsageStore::open(&db_path).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let mut state = PetState::seeded("test");
    state.species = Species::Fuzz;
    state.stage = crate::pet::Stage::S6;
    state.xp = 100.0;

    let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
    assert!(vm.progress.is_max_stage);
    assert_eq!(vm.progress.next_stage_label, "—");
}
```

If `PetState::seeded` doesn't exist, use whatever the canonical test constructor is — grep for `impl PetState` to find it.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib commands::watch::tests::build_watch_view_model_populates_progress_view -- --nocapture`
Expected: FAIL — `progress` field exists on the struct (Task 6) but the builder still uses the default fixture/placeholder; the assertions on `stage_label == "fuzz"` etc. won't hold.

- [ ] **Step 3: Compute `ProgressView` inside `build_watch_view_model_at`**

In `build_watch_view_model_at` (around `watch.rs:56`), after the existing recent-events query, add:

```rust
let ema_events = usage_store.events_within(time::Duration::hours(48), now)?;
let rate_per_hour = progress_rate_ema(&ema_events, now);

let stage = state.stage;
let species = state.species;
let is_max = matches!(stage, Stage::S6);
let xp_to_next = next_stage_xp_target(stage);
let xp_in_stage = if is_max { state.xp } else { state.xp };
let fraction = if xp_to_next <= 0.0 || is_max {
    1.0
} else {
    (xp_in_stage / xp_to_next).clamp(0.0, 1.0) as f32
};
let stage_label_now = crate::pet::generation::stage_label(species, stage).to_string();
let next_stage_label = if is_max {
    "—".to_string()
} else {
    let next = match stage {
        Stage::S0 => Stage::S1,
        Stage::S1 => Stage::S2,
        Stage::S2 => Stage::S3,
        Stage::S3 => Stage::S4,
        Stage::S4 => Stage::S5,
        Stage::S5 => Stage::S6,
        Stage::S6 => Stage::S6,
    };
    crate::pet::generation::stage_label(species, next).to_string()
};

let progress = ProgressView {
    stage_label: stage_label_now,
    next_stage_label,
    fraction,
    xp_in_stage,
    xp_to_next,
    rate_per_hour,
    is_max_stage: is_max,
};
```

Then wire `progress` into the returned `WatchViewModel { ..., progress, ... }`.

Verify `next_stage_xp_target` is already in scope (it is — defined at watch.rs:313). Verify the `Stage` enum import path. Use `crate::pet::generation::stage_label` (the species-aware one at generation.rs:114).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib commands::watch::tests::build_watch_view_model -- --nocapture`
Expected: PASS for both new cases.

- [ ] **Step 5: Commit**

```bash
git add src/commands/watch.rs
git commit -m "feat(watch): populate ProgressView in build_watch_view_model_at

Derives stage_label/next_stage_label from species+stage, fraction from
state.xp + next_stage_xp_target, and rate_per_hour from the EMA helper.
Handles S6 as max_stage with '—' next label and no rate-driven bar."
```

---

### Task 9: Build `BioView` in `build_watch_view_model_at`

**Files:**
- Modify: `src/commands/watch.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn build_watch_view_model_populates_bio_view() {
    use crate::storage::PetState;
    use tempfile::tempdir;
    use time::{Date, Month, PrimitiveDateTime, Time};

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    UsageStore::open(&db_path).unwrap();

    let created_at = PrimitiveDateTime::new(
        Date::from_calendar_date(2026, Month::April, 24).unwrap(),
        Time::from_hms(14, 32, 0).unwrap(),
    )
    .assume_utc();
    let now = created_at + Duration::days(18);

    let mut state = PetState::seeded("test");
    state.created_at = created_at;

    let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
    assert_eq!(vm.bio.age_label, "18d");
    assert!(vm.bio.hatched_label.contains("apr"), "got {}", vm.bio.hatched_label);
    assert!(vm.bio.hatched_label.contains("24"));
}

#[test]
fn build_watch_view_model_bio_sub_day_age() {
    use crate::storage::PetState;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    UsageStore::open(&db_path).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

    let mut state = PetState::seeded("test");
    state.created_at = now - Duration::hours(4);

    let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
    assert_eq!(vm.bio.age_label, "0d 4h");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib commands::watch::tests::build_watch_view_model_populates_bio_view -- --nocapture`
Expected: FAIL — bio field exists but uses fixture placeholder, not computed from state.

- [ ] **Step 3: Compute `BioView` inside `build_watch_view_model_at`**

Below the `progress` computation, add:

```rust
let age = now - state.created_at;
let age_label = BioView::format_age(age);

// Format hatched as "mon dd HH:MM" in local TZ.
let local = state
    .created_at
    .to_offset(time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC));
let month_name = match local.month() {
    time::Month::January => "jan", time::Month::February => "feb",
    time::Month::March => "mar",   time::Month::April => "apr",
    time::Month::May => "may",     time::Month::June => "jun",
    time::Month::July => "jul",    time::Month::August => "aug",
    time::Month::September => "sep", time::Month::October => "oct",
    time::Month::November => "nov", time::Month::December => "dec",
};
let hatched_label = format!(
    "{} {:02} {:02}:{:02}",
    month_name,
    local.day(),
    local.hour(),
    local.minute(),
);

let bio = BioView { hatched_label, age_label };
```

Then wire `bio` into the returned `WatchViewModel { ..., bio, ... }`.

Imports to verify at the top of `watch.rs`: `use crate::tui::view_model::{ProgressView, BioView, WatchViewModel};`

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib commands::watch::tests::build_watch_view_model -- --nocapture`
Expected: PASS for both new cases.

- [ ] **Step 5: Commit**

```bash
git add src/commands/watch.rs
git commit -m "feat(watch): populate BioView in build_watch_view_model_at

age_label uses sub-day (0d 4h) vs day-only (18d) formatting; hatched_label
formats state.created_at as 'apr 24 14:32' in local TZ."
```

---

## Phase 3 — Color roles in style.rs

### Task 10: Add 6 per-stat / per-source color functions

**Files:**
- Modify: `src/tui/style.rs`

- [ ] **Step 1: Write the failing test**

Add a new test module at the bottom of `src/tui/style.rs`:

```rust
#[cfg(test)]
mod stat_color_tests {
    use super::*;

    #[test]
    fn fed_color_is_amber() {
        assert_eq!(fed_color(), Color::Rgb(0xe8, 0xc4, 0x74));
    }

    #[test]
    fn happy_color_is_pink() {
        assert_eq!(happy_color(), Color::Rgb(0xe8, 0xa3, 0xc2));
    }

    #[test]
    fn energy_color_is_cyan() {
        assert_eq!(energy_color(), Color::Rgb(0x7f, 0xc8, 0xd6));
    }

    #[test]
    fn xp_color_is_coral() {
        assert_eq!(xp_color(), Color::Rgb(0xef, 0x8e, 0x6c));
    }

    #[test]
    fn claude_color_is_violet() {
        assert_eq!(claude_color(), Color::Rgb(0xb3, 0x9d, 0xf0));
    }

    #[test]
    fn codex_color_is_green() {
        assert_eq!(codex_color(), Color::Rgb(0x8f, 0xcf, 0x90));
    }

    #[test]
    fn xp_color_is_distinct_from_fed_color() {
        assert_ne!(xp_color(), fed_color(), "xp must not collide with fed");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib style::stat_color_tests -- --nocapture`
Expected: FAIL — `cannot find function fed_color` etc.

- [ ] **Step 3: Add the 6 color functions**

In `src/tui/style.rs`, near the bottom (before the test modules), add:

```rust
/// Per-stat semantic color roles (revision-3 layout refresh).
///
/// Each function returns a single solid `Color::Rgb`; the same color paints
/// the label, the filled bar segments, and the trailing value for that stat.
/// Empty bar cells continue to use `SemanticStyles.empty_bar`.
///
/// Dark-background truecolor tuned. Color-blind / 8-color palette tuning is
/// deferred — ratatui's color downgrade handles low-capability terminals.

pub fn fed_color() -> Color {
    Color::Rgb(0xe8, 0xc4, 0x74)
}

pub fn happy_color() -> Color {
    Color::Rgb(0xe8, 0xa3, 0xc2)
}

pub fn energy_color() -> Color {
    Color::Rgb(0x7f, 0xc8, 0xd6)
}

pub fn xp_color() -> Color {
    Color::Rgb(0xef, 0x8e, 0x6c)
}

pub fn claude_color() -> Color {
    Color::Rgb(0xb3, 0x9d, 0xf0)
}

pub fn codex_color() -> Color {
    Color::Rgb(0x8f, 0xcf, 0x90)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib style::stat_color_tests -- --nocapture`
Expected: PASS for all 7 cases.

- [ ] **Step 5: Commit**

```bash
git add src/tui/style.rs
git commit -m "feat(style): add per-stat color role functions

6 new semantic colors: fed (amber), happy (pink), energy (cyan),
xp (coral), claude (violet), codex (green). Consumed by vitals,
progress, today, and feed in upcoming commits."
```

---

## Phase 4 — Shared `bars` module

### Task 11: Create `panels/bars.rs` with `bar_spans_solid`

**Files:**
- Create: `src/tui/panels/bars.rs`
- Modify: `src/tui/panels/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `src/tui/panels/bars.rs` with the test module first:

```rust
use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::tui::style::SemanticStyles;

pub const BAR_CELLS: usize = 12;

/// Render a single-color bar row: `  <label:<6> <bar> <value>`.
/// Used by VitalsPanel rows and ProgressPanel's xp bar. The same color paints
/// the label, the filled cells, and the value. Empty cells use `empty_bar`.
pub fn bar_spans_solid<'a>(
    label: &'a str,
    fill_fraction: f64,
    color: Color,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    let clamped = fill_fraction.clamp(0.0, 1.0);
    let n_filled = ((clamped * BAR_CELLS as f64).round() as usize).min(BAR_CELLS);
    let n_empty = BAR_CELLS - n_filled;
    let value_pct = (clamped * 100.0).round() as u32;
    let stat_style = Style::default().fg(color);

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(BAR_CELLS + 6);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{label:<6}"), stat_style));
    spans.push(Span::raw(" "));
    if n_filled > 0 {
        spans.push(Span::styled("█".repeat(n_filled), stat_style));
    }
    if n_empty > 0 {
        spans.push(Span::styled("░".repeat(n_empty), styles.empty_bar));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{value_pct}"), stat_style));
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::{fed_color, semantic_styles};

    #[test]
    fn bar_spans_solid_zero_fill_renders_twelve_empty_cells() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 0.0, fed_color(), &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '░').count(), 12);
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 0);
    }

    #[test]
    fn bar_spans_solid_full_fill_renders_twelve_solid_cells() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 1.0, fed_color(), &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 12);
    }

    #[test]
    fn bar_spans_solid_label_and_value_share_stat_color() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 0.5, fed_color(), &styles);
        let label_style = spans[1].style;
        let value_style = spans.last().unwrap().style;
        assert_eq!(label_style.fg, Some(fed_color()));
        assert_eq!(value_style.fg, Some(fed_color()));
    }

    #[test]
    fn bar_spans_solid_filled_cells_share_stat_color() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 0.5, fed_color(), &styles);
        let filled_span = spans.iter().find(|s| s.content.contains('█')).unwrap();
        assert_eq!(filled_span.style.fg, Some(fed_color()));
    }
}
```

Add to `src/tui/panels/mod.rs`:

```rust
pub mod bars;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::bars::tests -- --nocapture`
Expected: PASS, actually — the function is defined inline. The point of this task is the unit conversion + new helper; tests will all pass on first run.

Skip to step 3 — there's no fail-first because the helper is greenfield. We compensate by snapshot-testing this helper through consumers in later tasks.

- [ ] **Step 3 (skipped — see above)**

- [ ] **Step 4: Run test to verify all green**

Run: `cargo test --lib panels::bars -- --nocapture`
Expected: PASS for all four cases.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/bars.rs src/tui/panels/mod.rs
git commit -m "feat(panels): add bars module with bar_spans_solid helper

Single-color bar (label + fill + value share the stat color, empty
cells stay muted). Consumed by VitalsPanel and ProgressPanel next."
```

---

### Task 12: Add `build_spark_line` to `bars.rs`

**Files:**
- Modify: `src/tui/panels/bars.rs`

- [ ] **Step 1: Write the failing test**

Add to the test module in `bars.rs`:

```rust
#[test]
fn build_spark_line_seven_days_uses_block_heights() {
    let styles = semantic_styles();
    let history = vec![0.0, 0.0, 0.0, 1_000.0, 5_000.0, 10_000.0, 20_000.0];
    let spans = build_spark_line(&history, &styles);
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    // Zero days render as '·', non-zero days render as block-height glyphs.
    assert_eq!(text.chars().filter(|c| *c == '·').count(), 3);
    assert!(text.contains('█'), "max day should hit highest block");
}

#[test]
fn build_spark_line_all_zero_renders_seven_dots() {
    let styles = semantic_styles();
    let spans = build_spark_line(&vec![0.0; 7], &styles);
    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text.chars().filter(|c| *c == '·').count(), 7);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::bars::tests::build_spark_line -- --nocapture`
Expected: FAIL — `cannot find function build_spark_line`.

- [ ] **Step 3: Implement `build_spark_line`**

Add to `bars.rs`:

```rust
/// Render a 7-day token history as a row of height-quantized block glyphs.
/// Mirrors what the dropped SparkPanel produced, so the visual is byte-identical
/// when this is rendered inside TodayPanel's footer.
pub fn build_spark_line<'a>(
    history: &[f64],
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    const GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = history.iter().copied().fold(0.0_f64, f64::max);
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(history.len() * 2);
    for (i, &v) in history.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        if v <= 0.0 || max <= 0.0 {
            spans.push(Span::styled(".".to_string(), styles.sparkline_past));
        } else {
            let frac = (v / max).clamp(0.0, 1.0);
            let idx = ((frac * (GLYPHS.len() - 1) as f64).round() as usize).min(GLYPHS.len() - 1);
            let glyph = GLYPHS[idx];
            let style = if i == history.len() - 1 {
                styles.sparkline_today
            } else {
                styles.sparkline_past
            };
            spans.push(Span::styled(glyph.to_string(), style));
        }
    }
    spans
}
```

Note: the first test asserts `·` but the code uses `.` — fix the test or code to agree. The dropped SparkPanel actually uses `·` (middle dot). Read `src/tui/panels/spark.rs:58,70` to confirm and copy the same glyph.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib panels::bars::tests::build_spark_line -- --nocapture`
Expected: PASS for both cases.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/bars.rs
git commit -m "feat(panels/bars): extract build_spark_line helper

Ports SparkPanel's block-height quantization into the shared module so
TodayPanel can embed a 7-day footer in the next commit. Same glyph set
and zero-day placeholder as the dropped panel."
```

---

### Task 13: Add `format_tokens_short` + `format_tokens_full` to `bars.rs`

**Files:**
- Modify: `src/tui/panels/bars.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn format_tokens_short_rounds_to_k_with_one_decimal() {
    assert_eq!(format_tokens_short(0.0), "0");
    assert_eq!(format_tokens_short(950.0), "950");
    assert_eq!(format_tokens_short(1_500.0), "1.5k");
    assert_eq!(format_tokens_short(16_700.0), "16.7k");
    assert_eq!(format_tokens_short(109_842.0), "109.8k");
    assert_eq!(format_tokens_short(1_234_567.0), "1.2M");
}

#[test]
fn format_tokens_full_uses_thousands_separators() {
    assert_eq!(format_tokens_full(0.0), "0");
    assert_eq!(format_tokens_full(16_700.0), "16,700");
    assert_eq!(format_tokens_full(1_234_567.0), "1,234,567");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::bars::tests::format_tokens -- --nocapture`
Expected: FAIL — `cannot find function format_tokens_short`.

- [ ] **Step 3: Implement formatters**

Look first at the existing duplicate implementations:

```bash
rg -n "fn format_tokens" src/
```

Copy the canonical one (likely in `today.rs` and/or `spark.rs`) into `bars.rs`:

```rust
pub fn format_tokens_short(n: f64) -> String {
    if n < 1_000.0 {
        format!("{:.0}", n)
    } else if n < 1_000_000.0 {
        format!("{:.1}k", n / 1_000.0)
    } else {
        format!("{:.1}M", n / 1_000_000.0)
    }
}

pub fn format_tokens_full(n: f64) -> String {
    let rounded = n.round() as i64;
    let mut s = rounded.to_string();
    let mut out = String::new();
    let bytes = s.as_bytes().to_vec();
    let mut count = 0;
    for b in bytes.iter().rev() {
        if count > 0 && count % 3 == 0 {
            out.insert(0, ',');
        }
        out.insert(0, *b as char);
        count += 1;
    }
    let _ = &mut s; // silence unused
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib panels::bars::tests::format_tokens -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/bars.rs
git commit -m "feat(panels/bars): add format_tokens_short / format_tokens_full

Consolidates the two formatters duplicated across panels. Existing
inline copies in today.rs and spark.rs will be deleted when those
panels are migrated in following commits."
```

---

## Phase 5 — New panels

### Task 14: New `ProgressPanel`

**Files:**
- Create: `src/tui/panels/progress.rs`
- Modify: `src/tui/panels/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `src/tui/panels/progress.rs` with both the (eventual) impl and tests:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::bars::{bar_spans_solid, format_tokens_short};
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, xp_color, SemanticStyles};
use crate::tui::view_model::{ProgressView, WatchViewModel};

pub struct ProgressPanel;

impl Panel for ProgressPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 2 content rows (stage line + xp bar).
        Constraint::Length(3)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, _ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" progress ");
        let inner = block.inner(area);
        block.render(area, buf);

        let styles = semantic_styles();
        let lines = build_progress_lines(&vm.progress, &styles);
        Paragraph::new(lines).render(inner, buf);
    }
}

fn build_progress_lines<'a>(
    progress: &'a ProgressView,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    if progress.is_max_stage {
        return vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(progress.stage_label.clone(), styles.primary_text),
                Span::raw("  "),
                Span::styled("✦ max evolved", styles.section_header),
            ]),
            Line::from(Span::raw("")),
        ];
    }
    let stage_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(progress.stage_label.clone(), styles.primary_text),
        Span::raw(" "),
        Span::styled("➜", styles.section_header),
        Span::raw(" "),
        Span::styled(progress.next_stage_label.clone(), styles.primary_text),
    ]);
    let mut xp_spans = bar_spans_solid("xp", progress.fraction as f64, xp_color(), styles);
    if progress.rate_per_hour > 0.0 {
        xp_spans.push(Span::raw("   "));
        xp_spans.push(Span::styled("↑", styles.section_header));
        xp_spans.push(Span::raw(" "));
        xp_spans.push(Span::styled(
            format!("{}/hr", format_tokens_short(progress.rate_per_hour)),
            Style::default().fg(xp_color()),
        ));
    }
    vec![stage_line, Line::from(xp_spans)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use crate::tui::style::ColorCapability;

    fn ctx() -> RenderContext {
        RenderContext::new(ColorCapability::Truecolor)
    }

    fn render(vm: &WatchViewModel) -> String {
        let panel = ProgressPanel;
        let backend = TestBackend::new(60, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), vm, &ctx())).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    #[test]
    fn progress_panel_renders_stage_and_next_stage_label() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.stage_label = "fuzz".to_string();
        vm.progress.next_stage_label = "archfuzz".to_string();
        vm.progress.is_max_stage = false;
        let s = render(&vm);
        assert!(s.contains("progress"), "section title");
        assert!(s.contains("fuzz"), "stage label");
        assert!(s.contains("archfuzz"), "next stage label");
        assert!(s.contains("➜"), "stage arrow");
        assert!(s.contains("xp"), "xp bar label");
    }

    #[test]
    fn progress_panel_at_s6_renders_max_evolved() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.is_max_stage = true;
        vm.progress.stage_label = "mythic-fuzz".to_string();
        let s = render(&vm);
        assert!(s.contains("mythic-fuzz"));
        assert!(s.contains("max evolved"));
        assert!(!s.contains("➜"), "no arrow at max stage");
    }

    #[test]
    fn progress_panel_idle_hides_rate_segment() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.rate_per_hour = 0.0;
        let s = render(&vm);
        assert!(!s.contains("↑"));
        assert!(!s.contains("/hr"));
    }

    #[test]
    fn progress_panel_preferred_constraint_is_three() {
        let vm = WatchViewModel::fixture();
        let panel = ProgressPanel;
        assert_eq!(panel.preferred_constraint(&vm), Constraint::Length(3));
    }
}
```

Register in `src/tui/panels/mod.rs`:

```rust
pub mod progress;
pub use progress::ProgressPanel;
```

- [ ] **Step 2: Run test to verify it fails initially OR builds + passes**

Run: `cargo test --lib panels::progress -- --nocapture`
Expected: PASS (greenfield panel; tests verify the impl works).

- [ ] **Step 3: (already implemented above)**

- [ ] **Step 4: Confirm test pass**

Already done in step 2.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/progress.rs src/tui/panels/mod.rs
git commit -m "feat(panels): new ProgressPanel

Renders 'stage ➜ next' on row 1 and the xp bar with EMA rate on row 2.
S6 pets show '✦ max evolved' instead of a bar. Rate segment is hidden
when EMA = 0.0 (idle)."
```

---

### Task 15: New `BioCardPanel`

**Files:**
- Create: `src/tui/panels/bio_card.rs`
- Modify: `src/tui/panels/mod.rs`

- [ ] **Step 1: Write impl + tests**

Create `src/tui/panels/bio_card.rs`:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, SemanticStyles};
use crate::tui::view_model::{BioView, WatchViewModel};

pub struct BioCardPanel;

impl Panel for BioCardPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 2 content rows (hatched, age).
        Constraint::Length(3)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, _ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" bio ");
        let inner = block.inner(area);
        block.render(area, buf);

        let styles = semantic_styles();
        let lines = build_bio_lines(&vm.bio, &styles);
        Paragraph::new(lines).render(inner, buf);
    }
}

fn build_bio_lines<'a>(bio: &'a BioView, styles: &'a SemanticStyles) -> Vec<Line<'a>> {
    vec![
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<8}", "hatched"), styles.label),
            Span::raw("  "),
            Span::styled(bio.hatched_label.clone(), styles.primary_text),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<8}", "age"), styles.label),
            Span::raw("  "),
            Span::styled(bio.age_label.clone(), styles.primary_text),
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use crate::tui::style::ColorCapability;

    fn ctx() -> RenderContext {
        RenderContext::new(ColorCapability::Truecolor)
    }

    fn render(vm: &WatchViewModel) -> String {
        let panel = BioCardPanel;
        let backend = TestBackend::new(40, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), vm, &ctx())).unwrap();
        terminal.backend().buffer().content().iter()
            .map(|c| c.symbol().to_string()).collect()
    }

    #[test]
    fn bio_panel_renders_title_and_two_rows() {
        let vm = WatchViewModel::fixture();
        let s = render(&vm);
        assert!(s.contains("bio"), "title");
        assert!(s.contains("hatched"), "hatched label");
        assert!(s.contains("age"), "age label");
    }

    #[test]
    fn bio_panel_renders_sub_day_age() {
        let mut vm = WatchViewModel::fixture();
        vm.bio.age_label = "0d 4h".to_string();
        let s = render(&vm);
        assert!(s.contains("0d 4h"));
    }

    #[test]
    fn bio_panel_preferred_constraint_is_three() {
        let vm = WatchViewModel::fixture();
        let panel = BioCardPanel;
        assert_eq!(panel.preferred_constraint(&vm), Constraint::Length(3));
    }
}
```

Register in `src/tui/panels/mod.rs`:

```rust
pub mod bio_card;
pub use bio_card::BioCardPanel;
```

- [ ] **Step 2-4: Build and run**

Run: `cargo test --lib panels::bio_card -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/bio_card.rs src/tui/panels/mod.rs
git commit -m "feat(panels): new BioCardPanel

Two rows under a 'bio' top-border title: hatched timestamp and age.
Uses existing label/primary_text role styles; no new colors needed."
```

---

## Phase 6 — Modify existing panels

### Task 16: VitalsPanel — drop xp row, apply per-stat colors

**Files:**
- Modify: `src/tui/panels/vitals.rs`

- [ ] **Step 1: Write the failing test**

In the existing `#[cfg(test)] mod tests` block in `vitals.rs`, replace the `vitals_panel_renders_all_four_labels` test and add new color assertions:

```rust
#[test]
fn vitals_panel_renders_three_labels_no_xp() {
    let vm = WatchViewModel::fixture();
    let panel = VitalsPanel;
    let ctx = test_context();
    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    assert!(s.contains("fed"));
    assert!(s.contains("happy"));
    assert!(s.contains("energy"));
    assert!(!s.contains("xp"), "xp moved to ProgressPanel");
}

#[test]
fn vitals_panel_preferred_constraint_is_four() {
    let vm = WatchViewModel::fixture();
    let panel = VitalsPanel;
    assert_eq!(
        panel.preferred_constraint(&vm),
        Constraint::Length(4),
        "1 border + 3 bar rows (xp dropped)"
    );
}

#[test]
fn vitals_panel_rows_use_per_stat_colors() {
    use crate::tui::style::{energy_color, fed_color, happy_color, semantic_styles};
    let styles = semantic_styles();
    let vm = WatchViewModel::fixture();
    let lines = build_vitals_lines(&vm, 40, ColorCapability::Truecolor, &styles);
    assert_eq!(lines.len(), 3);
    // First filled span on each line should carry that stat's color.
    let line_fg = |line: &Line| {
        line.spans
            .iter()
            .find(|s| s.content.contains('█'))
            .and_then(|s| s.style.fg)
    };
    assert_eq!(line_fg(&lines[0]), Some(fed_color()));
    assert_eq!(line_fg(&lines[1]), Some(happy_color()));
    assert_eq!(line_fg(&lines[2]), Some(energy_color()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::vitals -- --nocapture`
Expected: FAIL — xp row still present; constraint is `Length(5)`; bar still uses ramps.

- [ ] **Step 3: Rewrite VitalsPanel body**

Replace the body of `vitals.rs` after the imports with:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::bars::bar_spans_solid;
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{
    energy_color, fed_color, happy_color, semantic_styles, ColorCapability, SemanticStyles,
};
use crate::tui::view_model::WatchViewModel;

pub struct VitalsPanel;

impl Panel for VitalsPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 3 bar rows (fed, happy, energy). xp moved to ProgressPanel.
        Constraint::Length(4)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" vitals ");
        let inner = block.inner(area);
        block.render(area, buf);

        let styles = semantic_styles();
        let lines = build_vitals_lines(vm, inner.width, ctx.color_capability, &styles);
        Paragraph::new(lines).render(inner, buf);
    }
}

pub(crate) fn build_vitals_lines<'a>(
    vm: &'a WatchViewModel,
    _width: u16,
    _capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    vec![
        Line::from(bar_spans_solid("fed",    vm.fed,       fed_color(),    styles)),
        Line::from(bar_spans_solid("happy",  vm.happiness, happy_color(),  styles)),
        Line::from(bar_spans_solid("energy", vm.energy,    energy_color(), styles)),
    ]
}
```

Delete the old `bar_spans` and `xp_fraction` code. The Truecolor/Flat handling is no longer needed at this layer — `Color::Rgb` downgrades automatically. Drop the bar-ramp imports.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib panels::vitals -- --nocapture`
Expected: PASS for all three new tests + existing `vitals_panel_renders_into_area`.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/vitals.rs
git commit -m "feat(panels/vitals): drop xp row, per-stat colors

Vitals now shows only fed/happy/energy — xp moved to ProgressPanel.
Each row uses its stat's solid color (amber fed, pink happy, cyan
energy) via the new bar_spans_solid helper. preferred_constraint
shrinks from Length(5) to Length(4)."
```

---

### Task 17: FeedPanel — drop MAX_EVENT_ROWS to 6 + source colors

**Files:**
- Modify: `src/tui/panels/feed.rs`

- [ ] **Step 1: Write the failing test**

Read the existing `feed.rs` first (`cat src/tui/panels/feed.rs`) to understand the row-rendering helper.

Add tests:

```rust
#[test]
fn feed_panel_caps_at_six_events() {
    use crate::tui::view_model::EventView;
    let mut vm = WatchViewModel::fixture_with_events(/* count */ 12);
    let panel = FeedPanel;
    assert_eq!(
        panel.preferred_constraint(&vm),
        Constraint::Length(7),
        "1 border + 6 events, even when vm has 12"
    );
    let backend = TestBackend::new(60, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    let ctx = test_context();
    terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();
    let lines: Vec<String> = (0..buf.area().height)
        .map(|y| {
            (0..buf.area().width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect();
    // Border + 6 event rows + remaining padding = at most 6 rows with timestamps.
    let event_rows = lines.iter().filter(|l| l.contains(':') || l.contains("--")).count();
    assert!(event_rows <= 6, "feed must not render more than 6 events, got {event_rows}");
}

#[test]
fn feed_panel_source_label_colors() {
    use crate::tui::style::{claude_color, codex_color};
    let vm = WatchViewModel::fixture_with_events(3);
    let lines = build_feed_lines(&vm);
    let find_source = |needle: &str, color: ratatui::style::Color| {
        lines.iter().any(|l| {
            l.spans.iter().any(|s| s.content.contains(needle) && s.style.fg == Some(color))
        })
    };
    assert!(find_source("claude-code", claude_color()) || find_source("codex", codex_color()),
        "at least one source label must carry its source color");
}
```

If `WatchViewModel::fixture_with_events(n)` doesn't accept an arg yet, modify it (in `view_model.rs`) to accept event count and synthesize events accordingly.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::feed -- --nocapture`
Expected: FAIL — cap is 8 not 6; source labels not colored.

- [ ] **Step 3: Change cap + color source labels**

In `feed.rs`:

```rust
// Top of file, find the existing const:
const MAX_EVENT_ROWS: u16 = 6;  // was 8
```

In `build_feed_lines` (or wherever the row spans are built), when rendering an event source name, use the matching color. Pseudo-patch:

```rust
let source_style = match event.source.as_str() {
    "claude-code" | "claude" => Style::default().fg(claude_color()),
    "codex"                  => Style::default().fg(codex_color()),
    _                        => styles.label,
};
spans.push(Span::styled(event.source.clone(), source_style));
```

Add imports: `use crate::tui::style::{claude_color, codex_color};`.

If `build_feed_lines` is private, expose it as `pub(crate)` for the test in step 1 to import. If the test imports it from `super::*` (same module), no change needed.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib panels::feed -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/feed.rs src/tui/view_model.rs
git commit -m "feat(panels/feed): cap at 6 events, source label colors

MAX_EVENT_ROWS drops from 8 to 6 per spec — feed is a recency view,
not scrollback. Source labels (claude-code / codex) get their
source-role colors via the new style.rs roles."
```

---

### Task 18: TodayPanel — add 7-day footer, ⚠ marker, source colors

**Files:**
- Modify: `src/tui/panels/today.rs`

- [ ] **Step 1: Write the failing test**

Read the existing today.rs to find `build_today_lines` and source-row rendering. Then add tests:

```rust
#[test]
fn today_panel_renders_seven_day_inline_footer() {
    let vm = WatchViewModel::fixture();
    let panel = TodayPanel;
    let ctx = test_context();
    let backend = TestBackend::new(70, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    assert!(s.contains("7-day"), "footer row must carry '7-day' label");
}

#[test]
fn today_panel_renders_blocked_marker_on_unhealthy_source() {
    use crate::tui::view_model::{SourceHealthView, SourceStatus};
    let mut vm = WatchViewModel::fixture();
    vm.source_health = vec![
        SourceHealthView { provider: "codex".to_string(), status: SourceStatus::Blocked, ..Default::default() },
        SourceHealthView { provider: "claude-code".to_string(), status: SourceStatus::Ready, ..Default::default() },
    ];
    let panel = TodayPanel;
    let ctx = test_context();
    let backend = TestBackend::new(70, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    assert!(s.contains("⚠"), "blocked source must render the marker");
}

#[test]
fn today_panel_source_labels_use_source_colors() {
    use crate::tui::style::{claude_color, codex_color};
    let vm = WatchViewModel::fixture();
    let lines = build_today_lines(&vm);
    let has_color = |needle: &str, color: ratatui::style::Color| {
        lines.iter().any(|l| {
            l.spans.iter().any(|s| s.content.trim() == needle && s.style.fg == Some(color))
        })
    };
    assert!(has_color("claude", claude_color()));
    assert!(has_color("codex",  codex_color()));
}

#[test]
fn today_panel_preferred_constraint_is_six() {
    let vm = WatchViewModel::fixture();
    let panel = TodayPanel;
    assert_eq!(
        panel.preferred_constraint(&vm),
        Constraint::Length(6),
        "1 border + tokens + claude + codex + last_10m + 7-day footer"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::today -- --nocapture`
Expected: FAIL on each new assertion.

- [ ] **Step 3: Implement**

Modify `today.rs`:

1. Add imports:
   ```rust
   use crate::tui::panels::bars::build_spark_line;
   use crate::tui::style::{claude_color, codex_color};
   ```

2. In `build_today_lines` (or whatever the lines builder is):
   - For each source row, look up `vm.source_health` by provider name. If `status != Ready`, insert `Span::styled("⚠", styles.event_rail_diagnostic)` between the label and its numeric value. Reserve a 3-cell gutter whether or not the marker renders.
   - Color the source label using `claude_color()` / `codex_color()` (match on `source.as_str()`).

3. After the existing `last 10m / this 10m` row, append a 7-day footer row:
   ```rust
   let spark_spans = build_spark_line(&vm.recent_daily_effective_tokens, &styles);
   let mut footer = vec![Span::raw("  ")];
   footer.extend(spark_spans);
   footer.push(Span::raw("          "));
   footer.push(Span::styled("← 7-day", styles.section_header));
   lines.push(Line::from(footer));
   ```

4. Update `preferred_constraint` to return `Constraint::Length(6)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib panels::today -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/today.rs
git commit -m "feat(panels/today): 7-day footer, blocked marker, source colors

- Inline 7-day strip reuses build_spark_line from bars; visual is
  byte-identical to the dropped SparkPanel.
- Each source row reserves a 3-cell gutter; status != Ready inserts ⚠.
- Source labels get their source-role colors.
- preferred_constraint grows from Length(5) to Length(6)."
```

---

### Task 19: PetPanel — Fill(1) constraint + 2-pass paint scaffolding

**Files:**
- Modify: `src/tui/panels/pet.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pet_panel_preferred_constraint_is_fill() {
    let vm = WatchViewModel::fixture();
    let panel = PetPanel;
    assert_eq!(
        panel.preferred_constraint(&vm),
        Constraint::Fill(1),
        "pet panel absorbs vertical slack so habitat (PR2) can fill it"
    );
}

#[test]
fn pet_panel_renders_pet_centered_in_tall_rect() {
    let vm = WatchViewModel::fixture();
    let panel = PetPanel;
    let ctx = test_context();
    let backend = TestBackend::new(40, 24); // taller than pet (10 rows)
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    // The pet's eye glyphs should appear somewhere in the rendered area.
    // Pet templates use '*' as a common eye marker for content mood.
    assert!(s.contains('*') || s.contains('o') || s.contains('●'),
        "pet must render visibly in a tall panel rect");
}

#[test]
fn ambient_glyphs_for_returns_empty_in_pr1_stub() {
    use ratatui::layout::Rect;
    use crate::pet::Species;
    use time::OffsetDateTime;
    let panel_rect = Rect::new(0, 0, 40, 20);
    let pet_inner = Rect::new(13, 5, 13, 10);
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let glyphs = ambient_glyphs_for(Species::Fuzz, panel_rect, pet_inner, now);
    assert!(glyphs.is_empty(), "PR1 stub returns empty; PR2 fills this in");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::pet -- --nocapture`
Expected: FAIL — current constraint is `Length(...)`, and `ambient_glyphs_for` doesn't exist.

- [ ] **Step 3: Implement scaffolding**

In `src/tui/panels/pet.rs`:

1. Change `preferred_constraint`:
   ```rust
   fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
       Constraint::Fill(1)
   }
   ```

2. Add the ambient-glyph types and stub:
   ```rust
   use crate::pet::Species;
   use ratatui::style::Color;

   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub struct AmbientGlyph {
       pub row: u16,
       pub col: u16,
       pub glyph: char,
       pub color: Color,
   }

   /// PR1 stub — returns empty so the pet panel renders the same content as
   /// before, just inside a taller (Fill-driven) rect. PR2 fills this in.
   pub fn ambient_glyphs_for(
       _species: Species,
       _panel: Rect,
       _pet_inner_rect: Rect,
       _now: time::OffsetDateTime,
   ) -> Vec<AmbientGlyph> {
       Vec::new()
   }
   ```

3. Modify `render` to do two passes:
   ```rust
   fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
       // Pass 1: ambient backdrop. PR1 stub returns empty so this is a no-op.
       let pet_inner = pet_inner_rect_in_panel(area, vm);
       let now = time::OffsetDateTime::now_utc();
       let glyphs = ambient_glyphs_for(vm.species, area, pet_inner, now);
       for g in glyphs {
           if g.col < area.x + area.width && g.row < area.y + area.height {
               let cell = &mut buf[(g.col, g.row)];
               cell.set_char(g.glyph);
               cell.set_style(ratatui::style::Style::default().fg(g.color));
           }
       }

       // Pass 2: existing pet art rendering. Unchanged from prior implementation.
       // (Keep the existing render body here — only the wrapper changed.)
       render_pet_inside(area, buf, vm, ctx);
   }
   ```

   Refactor the previous `render` body into a `render_pet_inside(area, buf, vm, ctx)` private function. Centering inside `area` should account for `area.height` (which is now Fill-sized) — center the pet vertically in the panel.

4. Add `pet_inner_rect_in_panel(area, vm)`:
   ```rust
   fn pet_inner_rect_in_panel(area: Rect, vm: &WatchViewModel) -> Rect {
       const PET_W: u16 = 13;
       const PET_H: u16 = 10;
       let x = area.x + (area.width.saturating_sub(PET_W)) / 2
           + vm.wander_offset_x as i16 as u16;
       let y = area.y + (area.height.saturating_sub(PET_H)) / 2
           + vm.breath_offset_y as u16;
       Rect::new(x, y, PET_W, PET_H)
   }
   ```

   This function will also be used by `pet_panel_rect()` in layout.rs.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib panels::pet -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs
git commit -m "feat(panels/pet): Fill(1) constraint + 2-pass paint scaffolding

PetPanel now uses Constraint::Fill(1) so habitat (PR2) absorbs vertical
slack. Adds AmbientGlyph type and ambient_glyphs_for() returning empty
Vec in PR1; pet still centers in its rect and renders unchanged. PR2
fills in the per-species glyph placement."
```

---

## Phase 7 — Layout refactor + panel removal

### Task 20: Remove SparkPanel + HelpersPanel imports/usage in layout.rs

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Write the failing test**

Add (or update) a snapshot-style test in `tests/tui_render.rs` (or inline in `layout.rs` if that's where existing layout tests live). Skip writing a test if the existing test suite already asserts panel ordering; the next task's full-frame snapshot will catch regressions.

For now, add to `layout.rs` tests if any exist:

```rust
#[test]
fn render_wide_does_not_include_helpers_or_spark_strings() {
    let vm = WatchViewModel::fixture();
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    let ctx = test_context();
    terminal.draw(|f| {
        let frame_area = f.area();
        render_watch_frame_with_context(f, &vm, &ctx);
    }).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    assert!(!s.contains("helpers"), "helpers panel removed");
    assert!(!s.contains("7-day") || s.contains("7-day"), "7-day moves to today footer");
    // The body should still render the new sections.
    assert!(s.contains("today"));
    assert!(s.contains("progress"));
    assert!(s.contains("feed"));
    assert!(s.contains("bio"));
}
```

(Adjust to whatever the canonical render entry point is. The agent should grep `render_watch_frame` and `render_wide` first.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib layout::tests -- --nocapture` (or wherever it lives)
Expected: FAIL on "helpers" being absent.

- [ ] **Step 3: Update render_wide + render_compact**

In `layout.rs`, find `render_wide` (around line 180) and `render_compact` (around line 221). Replace each function's body to use the new panel set and constraint sequences.

For wide mode (illustrative — adapt to existing helper functions like `Layout::default().direction(...).constraints(...)`):

```rust
fn render_wide(area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
    use ratatui::layout::{Direction, Layout};

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(WIDE_LEFT_COL),
            Constraint::Length(WIDE_GUTTER),
            Constraint::Min(50),
        ])
        .split(area);

    let left_col = body[0];
    let right_col = body[2];

    let vitals = VitalsPanel;
    let bio = BioCardPanel;
    let pet = PetPanel;

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            pet.preferred_constraint(vm),                    // Fill(1)
            Constraint::Length(COLUMN_GAP),
            vitals.preferred_constraint(vm),                 // Length(4)
            Constraint::Length(COLUMN_GAP),
            bio.preferred_constraint(vm),                    // Length(3)
        ])
        .split(left_col);

    pet.render(left[0], buf, vm, ctx);
    vitals.render(left[2], buf, vm, ctx);
    bio.render(left[4], buf, vm, ctx);

    let today = TodayPanel;
    let progress = ProgressPanel;
    let feed = FeedPanel;

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            today.preferred_constraint(vm),                  // Length(6)
            Constraint::Length(COLUMN_GAP),
            progress.preferred_constraint(vm),               // Length(3)
            Constraint::Length(COLUMN_GAP),
            feed.preferred_constraint(vm),                   // Length(7)
            Constraint::Min(0),                              // trailing slack — accepted
        ])
        .split(right_col);

    today.render(right[0], buf, vm, ctx);
    progress.render(right[2], buf, vm, ctx);
    feed.render(right[4], buf, vm, ctx);
}
```

For compact mode, single vertical column with the same anchored layout but stacked:

```rust
fn render_compact(area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
    use ratatui::layout::{Direction, Layout};

    let pet = PetPanel;
    let vitals = VitalsPanel;
    let bio = BioCardPanel;
    let today = TodayPanel;
    let progress = ProgressPanel;
    let feed = FeedPanel;

    let stack = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            pet.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            vitals.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            today.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            progress.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            feed.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            bio.preferred_constraint(vm),
            Constraint::Min(0),
        ])
        .split(area);

    pet.render(stack[0],  buf, vm, ctx);
    vitals.render(stack[2],  buf, vm, ctx);
    today.render(stack[4],  buf, vm, ctx);
    progress.render(stack[6], buf, vm, ctx);
    feed.render(stack[8],  buf, vm, ctx);
    bio.render(stack[10], buf, vm, ctx);
}
```

Remove every reference to `SparkPanel` and `HelpersPanel` (imports + usage).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo build --lib` first to see compilation errors.
Expected error: `SparkPanel`/`HelpersPanel` modules referenced in `panels/mod.rs`. Fix in next task.

- [ ] **Step 5: (deferred to Task 21)**

---

### Task 21: Delete spark.rs and helpers.rs files; update mod.rs

**Files:**
- Delete: `src/tui/panels/spark.rs`
- Delete: `src/tui/panels/helpers.rs`
- Modify: `src/tui/panels/mod.rs`

- [ ] **Step 1: (no separate test — covered by Task 20's snapshot)**

- [ ] **Step 2: Verify build is currently broken**

Run: `cargo build --lib`
Expected: FAIL with `cannot find module spark` or similar (after Task 20's removal).

- [ ] **Step 3: Delete files + update mod.rs**

```bash
git rm src/tui/panels/spark.rs src/tui/panels/helpers.rs
```

In `src/tui/panels/mod.rs`, remove:
```rust
pub mod spark;
pub mod helpers;
pub use spark::SparkPanel;
pub use helpers::HelpersPanel;
```

- [ ] **Step 4: Build + test**

Run: `cargo build --lib && cargo test --lib panels -- --nocapture`
Expected: builds; layout snapshot test passes.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/mod.rs
git add -u src/tui/panels/spark.rs src/tui/panels/helpers.rs
git add src/tui/layout.rs
git commit -m "refactor(layout): drop SparkPanel + HelpersPanel; new constraint sequences

- 7-day data moved to TodayPanel's footer (via bars::build_spark_line).
- Source health surfaces inline as ⚠ on today's source rows.
- Left column: pet (Fill) → vitals → bio (anchored bottom).
- Right column: today → progress → feed (packed top, trailing slack).
- Compact mode: pet → vitals → today → progress → feed → bio."
```

---

### Task 22: Update `pet_panel_rect()` to account for BioCardPanel + return 13×10 sub-rect

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pet_panel_rect_returns_thirteen_by_ten_sub_rect() {
    let vm = WatchViewModel::fixture();
    let frame_area = Rect::new(0, 0, 120, 32);
    let rect = pet_panel_rect(frame_area, &vm);
    assert_eq!(rect.width, 13);
    assert_eq!(rect.height, 10);
}

#[test]
fn pet_panel_rect_accounts_for_bio_panel_height() {
    // Without BioCardPanel's 3-row contribution, the helper would return a
    // taller rect that overlaps vitals.
    let vm = WatchViewModel::fixture();
    let frame_area = Rect::new(0, 0, 120, 50);
    let rect = pet_panel_rect(frame_area, &vm);
    // The bottom of the pet sub-rect should sit above where vitals starts.
    assert!(rect.y + rect.height < frame_area.height - 3, // 3 = bio height
        "pet sub-rect must end before bio starts");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib layout::tests::pet_panel_rect -- --nocapture`
Expected: FAIL — current impl returns full panel rect, not 13×10 sub-rect.

- [ ] **Step 3: Update `pet_panel_rect`**

Find `pet_panel_rect` in `layout.rs` (around line 77). Replace with:

```rust
pub fn pet_panel_rect(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    use crate::tui::panels::pet::pet_inner_rect_in_panel;

    let inner = inner_frame_rect(frame_area); // existing helper that strips the outer chrome

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(WIDE_LEFT_COL),
            Constraint::Length(WIDE_GUTTER),
            Constraint::Min(50),
        ])
        .split(inner);
    let left_col = body[0];

    let vitals = VitalsPanel;
    let bio = BioCardPanel;
    let pet = PetPanel;

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            pet.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            vitals.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            bio.preferred_constraint(vm),
        ])
        .split(left_col);

    let pet_panel = left[0];
    pet_inner_rect_in_panel(pet_panel, vm)
}
```

Make `pet_inner_rect_in_panel` `pub` (or `pub(crate)`) in `pet.rs` if it isn't already.

If `inner_frame_rect` doesn't exist as a helper, factor the outer-chrome-stripping logic out of `render_watch_frame_with_context` so both `pet_panel_rect` and the render path use the same code.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib layout::tests::pet_panel_rect -- --nocapture`
Expected: PASS for both cases.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs src/tui/panels/pet.rs
git commit -m "fix(layout): pet_panel_rect returns 13x10 sub-rect

Previously returned the full PetPanel rect, which now grows with
Constraint::Fill(1). Without scoping to the 13x10 sub-rect, tachyonfx
effects would sweep across the entire (potentially 25+ row) pet column
including habitat. Sub-rect tracks wander/breath offsets so effects
follow the pet's on-screen position frame-by-frame."
```

---

## Phase 8 — Snapshot + integration tests + cleanup

### Task 23: Delete stale `helpers` assertions in `tests/tui_render.rs`

**Files:**
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Inventory existing failing assertions**

Run: `cargo test --test tui_render 2>&1 | head -40`
Expected: numerous failures referencing "helpers" or "xp" in vitals.

- [ ] **Step 2: Delete or rewrite**

Apply the spec's snapshot test plan literally:

```bash
# Find every occurrence
rg -n "helpers" tests/tui_render.rs
rg -n '\bxp\b' tests/tui_render.rs
```

For each match:
- Lines that assert `text.contains("helpers")` → delete the line (and the surrounding test if the test exists only to verify the helpers panel).
- `compact_threshold_switches_modes` — change the helper-string assertion to assert the new compact panel ordering. The compact frame must include `"today"`, `"progress"`, `"feed"`, `"bio"`, and must NOT include `"helpers"`.
- Any `text.contains("xp")` inside a vitals test — move to a new ProgressPanel test (one already exists in `progress.rs`; add a `cargo test --test tui_render` integration version if you want symmetry, but the panel-level test is sufficient).

- [ ] **Step 3: Run tests**

Run: `cargo test --test tui_render`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/tui_render.rs
git commit -m "test(tui_render): drop stale helpers + xp-in-vitals assertions

helpers panel is gone (signal moves to today's ⚠ marker). xp moved
from vitals to ProgressPanel; vitals-row tests no longer assert on it.
compact_threshold_switches_modes now asserts the new panel ordering."
```

---

### Task 24: New whole-frame snapshot — wide 120×32

**Files:**
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn watch_wide_120x32_has_no_left_column_dead_bands() {
    let vm = WatchViewModel::fixture();
    let ctx = test_context();
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_watch_frame_with_context(f, &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();

    // For each row, scan only the left-column cells (cols 1..=39 inside the frame).
    // A "dead row" is one where every cell in that range is whitespace.
    let dead_rows: Vec<u16> = (1..buf.area().height - 1)
        .filter(|&y| {
            (1..=39).all(|x| buf[(x, y)].symbol().chars().all(|c| c == ' ' || c == '·'))
        })
        .collect();
    // Above the pet and between pet/vitals, blank rows are acceptable as long
    // as none of them is below the pet — i.e. no fully-blank row past the bio.
    let bio_starts_at: Option<u16> = (1..buf.area().height)
        .find(|&y| (0..buf.area().width).any(|x| {
            let s = buf[(x, y)].symbol();
            s.contains("bio")
        }));
    if let Some(bio_y) = bio_starts_at {
        let bands_below_bio: Vec<u16> = dead_rows.iter().copied().filter(|&y| y > bio_y).collect();
        assert!(bands_below_bio.is_empty(),
            "left column must have no dead rows below bio panel (got rows {:?})", bands_below_bio);
    }
}

#[test]
fn watch_wide_120x32_feed_bounded_at_seven_rows() {
    let vm = WatchViewModel::fixture_with_events(20); // way more than the cap
    let ctx = test_context();
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_watch_frame_with_context(f, &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();

    // Count rows that look like feed event rows (contain a timestamp pattern).
    let feed_rows = (0..buf.area().height)
        .filter(|&y| {
            let line: String = (0..buf.area().width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            line.contains("claude-code")
                || line.contains("codex")
                || line.contains("inspected a fresh")
                || line.contains("warm token cache")
        })
        .count();
    assert!(feed_rows <= 6, "feed must not render more than 6 events, got {feed_rows}");
}
```

- [ ] **Step 2: Run test to verify it fails OR passes**

Run: `cargo test --test tui_render::watch_wide_120x32 -- --nocapture`
Expected: PASS if Tasks 16-22 landed correctly. If FAIL, root-cause via the buffer dump (use `buf.area()` + iterate to print each row).

- [ ] **Step 3-4: No implementation change needed (regression test only)**

- [ ] **Step 5: Commit**

```bash
git add tests/tui_render.rs
git commit -m "test(tui_render): whole-frame snapshot at 120x32

Locks in the new contract: no left-column dead bands below the bio
panel, and feed never exceeds 6 events even when the vm has more."
```

---

### Task 25: New whole-frame snapshot — wide 180×50 + compact 72×24

**Files:**
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn watch_wide_180x50_left_column_still_full() {
    let vm = WatchViewModel::fixture();
    let ctx = test_context();
    let backend = TestBackend::new(180, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_watch_frame_with_context(f, &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    assert!(s.contains("bio"), "bio panel must render at 180x50");
    assert!(s.contains("vitals"), "vitals must render");
    assert!(s.contains("progress"), "progress must render");
    assert!(s.contains("feed"), "feed must render");
}

#[test]
fn watch_compact_72x24_panels_in_order() {
    let vm = WatchViewModel::fixture();
    let ctx = test_context();
    let backend = TestBackend::new(72, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_watch_frame_with_context(f, &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();

    // For each section header, capture its row index. Order must be:
    // pet (no title) → vitals → today → progress → feed → bio
    let row_of = |needle: &str| -> Option<u16> {
        (0..buf.area().height).find(|&y| {
            let line: String = (0..buf.area().width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            line.contains(needle)
        })
    };
    let vitals_y = row_of("vitals").expect("vitals");
    let today_y = row_of("today").expect("today");
    let progress_y = row_of("progress").expect("progress");
    let feed_y = row_of("feed").expect("feed");
    let bio_y = row_of("bio").expect("bio");
    assert!(vitals_y < today_y, "vitals before today");
    assert!(today_y < progress_y, "today before progress");
    assert!(progress_y < feed_y, "progress before feed");
    assert!(feed_y < bio_y, "feed before bio");

    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    assert!(!s.contains("helpers"));
}
```

- [ ] **Step 2-4: Run, fix if needed, ensure pass**

Run: `cargo test --test tui_render::watch_wide_180x50 watch_compact_72x24 -- --nocapture`
Expected: PASS. If fail, inspect the buffer to find the misordering.

- [ ] **Step 5: Commit**

```bash
git add tests/tui_render.rs
git commit -m "test(tui_render): whole-frame snapshots at 180x50 and compact 72x24"
```

---

### Task 26: Integration test — BioView hatched + age from real PetState

**Files:**
- Modify: `tests/watch_integration.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn bio_view_renders_from_real_pet_state() {
    use glorp::storage::{PetState, UsageStore};
    use glorp::commands::watch::build_watch_view_model_at;
    use tempfile::tempdir;
    use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time};

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let store = UsageStore::open(&db_path).unwrap();
    // Seed 8 days of daily_aggregates so seven_day pulls from both tables.
    let base = PrimitiveDateTime::new(
        Date::from_calendar_date(2026, Month::May, 11).unwrap(),
        Time::from_hms(4, 0, 0).unwrap(),
    ).assume_utc();
    for i in 0..8 {
        let d = (base + Duration::days(i)).date().to_string();
        store.upsert_daily_aggregate(&d, "claude-code", 50_000.0 * (i + 1) as f64, 0.0).unwrap();
    }
    drop(store);

    let mut state = PetState::seeded("test");
    state.created_at = base;
    let now = base + Duration::days(7) + Duration::hours(13);

    let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
    assert!(vm.bio.hatched_label.starts_with("may"), "got {}", vm.bio.hatched_label);
    assert_eq!(vm.bio.age_label, "7d");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --test watch_integration bio_view_renders_from_real_pet_state -- --nocapture`
Expected: PASS (after the watch builder changes from Phase 2).

- [ ] **Step 5: Commit**

```bash
git add tests/watch_integration.rs
git commit -m "test(integration): bio view derives from real pet state"
```

---

### Task 27: Integration test — ⚠ marker via real provider_diagnostic

**Files:**
- Modify: `tests/watch_integration.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn blocked_source_surfaces_via_source_health() {
    use glorp::storage::{PetState, UsageStore};
    use glorp::commands::watch::build_watch_view_model_at;
    use glorp::tui::view_model::SourceStatus;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let store = UsageStore::open(&db_path).unwrap();
    // Insert a "blocked" diagnostic for codex.
    store
        .upsert_provider_diagnostic("codex", "blocked", "binary missing", OffsetDateTime::now_utc())
        .unwrap();
    drop(store);

    let state = PetState::seeded("test");
    let now = OffsetDateTime::now_utc();
    let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
    let codex_health = vm.source_health.iter().find(|s| s.provider == "codex").unwrap();
    assert_ne!(codex_health.status, SourceStatus::Ready);
}
```

The `upsert_provider_diagnostic` signature may differ — check existing tests using it (grep for `upsert_provider_diagnostic`) and match the real signature.

- [ ] **Step 2-4: Run, adjust signatures, pass**

Run: `cargo test --test watch_integration blocked_source -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/watch_integration.rs
git commit -m "test(integration): blocked source surfaces via source_health"
```

---

### Task 28: Integration test — EMA monotonicity

**Files:**
- Modify: `tests/watch_integration.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn ema_rate_grows_with_more_recent_events() {
    use glorp::storage::{NormalizedUsageEvent, PetState, UsageStore};
    use glorp::commands::watch::build_watch_view_model_at;
    use tempfile::tempdir;
    use time::{Duration, OffsetDateTime};

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let store = UsageStore::open(&db_path).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

    let mut events_baseline = Vec::new();
    for i in 0..10 {
        let evt = NormalizedUsageEvent {
            observed_at: now - Duration::minutes(15 * i + 5),
            provider: "claude-code".to_string(),
            period_date: now.date().to_string(),
            provider_delta_id: format!("test-{i}"),
            bucket_index: 0,
            effective_tokens: 10_000.0,
            cost_usd: 0.0,
        };
        store.insert_event(&evt).unwrap();
        events_baseline.push(evt);
    }
    let state = PetState::seeded("test");
    let vm_a = build_watch_view_model_at(&state, &db_path, now).unwrap();
    let rate_a = vm_a.progress.rate_per_hour;

    // Add one more recent event.
    let bonus = NormalizedUsageEvent {
        observed_at: now,
        provider: "codex".to_string(),
        period_date: now.date().to_string(),
        provider_delta_id: "bonus".to_string(),
        bucket_index: 0,
        effective_tokens: 50_000.0,
        cost_usd: 0.0,
    };
    store.insert_event(&bonus).unwrap();
    drop(store);

    let vm_b = build_watch_view_model_at(&state, &db_path, now).unwrap();
    let rate_b = vm_b.progress.rate_per_hour;
    assert!(rate_b > rate_a, "ema must grow with more recent contribution (a={rate_a}, b={rate_b})");
}
```

- [ ] **Step 2-4: Run, fix, pass**

Run: `cargo test --test watch_integration ema_rate_grows -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/watch_integration.rs
git commit -m "test(integration): EMA rate grows with recent events"
```

---

## Phase 9 — PR1 ship checkpoint

### Task 29: Full verification — cargo test, clippy, dev-preview

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: all green.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 3: Run fmt**

Run: `cargo fmt --check`
Expected: clean.

- [ ] **Step 4: Generate dev-preview and visually inspect**

Run: `cargo run -- dev-preview --scenario all --out target/glorp-preview && open target/glorp-preview/index.html`
Expected: the wide and compact watch scenarios show the new layout. Verify:
- No "helpers" panel anywhere.
- No standalone "7-day" section — the strip is inside `today`.
- `progress` panel exists with `fuzz ➜ archfuzz` and an xp bar.
- `bio` panel exists below vitals.
- Vitals shows fed/happy/energy (no xp), each colored amber/pink/cyan.
- Feed shows at most 6 events.
- Left column has no dead bands at the bottom (pet panel grows to fill).
- Right column has trailing space below feed at tall heights — accepted.

- [ ] **Step 5: Open a PR**

Branch + push:

```bash
git checkout -b watch-layout-refresh-pr1
git push -u origin watch-layout-refresh-pr1
gh pr create --title "feat(watch): layout refresh — color, Fill, bounded feed" --body "$(cat <<'EOF'
## Summary
- New panels: ProgressPanel, BioCardPanel
- Dropped panels: SparkPanel, HelpersPanel (signals absorbed into today's footer + ⚠ marker)
- New shared bars module (single-color bar + 7-day spark line + token formatters)
- 6 new semantic color roles (fed/happy/energy/xp + claude/codex)
- Left column: pet uses Constraint::Fill(1) so habitat (PR2) absorbs slack; vitals + bio anchor to bottom
- Right column: today → progress → feed, packed top, feed capped at MAX_EVENT_ROWS=6
- New EMA rate helper (6h half-life) + 2 new UsageStore queries (events_within, best_day_effective_tokens) + seven_day_token_history aggregate-aware fix

Spec: docs/superpowers/specs/2026-05-12-watch-layout-refresh-design.md (revision 3)

Habitat ambient glyphs are deferred to PR2 — ambient_glyphs_for() is a no-op stub. The pet panel rect is now taller, with the slack visibly empty above and below the pet. PR2 will fill it.

## Test plan
- [ ] cargo test
- [ ] cargo clippy --all-targets --all-features -- -D warnings
- [ ] cargo fmt --check
- [ ] glorp dev-preview --scenario all and visually QA
- [ ] glorp watch on a real account to sanity-check colors/contrast

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

This is the PR1 ship boundary. **Stop here and wait for PR1 to merge before starting PR2.**

---

## Phase 10 — PR2: Habitat

### Task 30: Implement `ambient_glyphs_for` for Fuzz species

**Files:**
- Modify: `src/tui/panels/pet.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn ambient_glyphs_for_fuzz_returns_glyphs_outside_pet_inner() {
    use crate::pet::Species;
    use ratatui::layout::Rect;
    use time::OffsetDateTime;
    let panel = Rect::new(0, 0, 40, 24);
    let pet_inner = Rect::new(13, 7, 13, 10);
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let glyphs = ambient_glyphs_for(Species::Fuzz, panel, pet_inner, now);
    assert!(!glyphs.is_empty(), "fuzz must produce ambient glyphs");
    // None overlap the pet inner rect.
    for g in &glyphs {
        let in_pet_x = g.col >= pet_inner.x && g.col < pet_inner.x + pet_inner.width;
        let in_pet_y = g.row >= pet_inner.y && g.row < pet_inner.y + pet_inner.height;
        assert!(!(in_pet_x && in_pet_y), "glyph at ({}, {}) overlaps pet inner rect", g.col, g.row);
    }
    // Density around 3%: 40 * 14 (panel rows outside pet) ≈ 17 cells expected.
    assert!(glyphs.len() >= 8 && glyphs.len() <= 30, "got {} glyphs", glyphs.len());
}

#[test]
fn ambient_glyphs_for_uses_fuzz_glyph_set() {
    use crate::pet::Species;
    use ratatui::layout::Rect;
    use time::OffsetDateTime;
    let panel = Rect::new(0, 0, 40, 24);
    let pet_inner = Rect::new(13, 7, 13, 10);
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let glyphs = ambient_glyphs_for(Species::Fuzz, panel, pet_inner, now);
    for g in &glyphs {
        assert!(
            matches!(g.glyph, '·' | '.' | ',' | '`'),
            "fuzz glyph set is `· . , \\``, got '{}'", g.glyph
        );
    }
}

#[test]
fn ambient_glyphs_for_is_deterministic_within_drift_phase() {
    use crate::pet::Species;
    use ratatui::layout::Rect;
    use time::{Duration, OffsetDateTime};
    let panel = Rect::new(0, 0, 40, 24);
    let pet_inner = Rect::new(13, 7, 13, 10);
    let t0 = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let t1 = t0 + Duration::seconds(3); // < DRIFT_SECS = 8
    let g0 = ambient_glyphs_for(Species::Fuzz, panel, pet_inner, t0);
    let g1 = ambient_glyphs_for(Species::Fuzz, panel, pet_inner, t1);
    assert_eq!(g0, g1, "same drift_phase must yield identical glyphs");
}

#[test]
fn ambient_glyphs_for_shifts_at_drift_phase_boundary() {
    use crate::pet::Species;
    use ratatui::layout::Rect;
    use time::{Duration, OffsetDateTime};
    let panel = Rect::new(0, 0, 40, 24);
    let pet_inner = Rect::new(13, 7, 13, 10);
    let t0 = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let t1 = t0 + Duration::seconds(9); // > DRIFT_SECS
    let g0 = ambient_glyphs_for(Species::Fuzz, panel, pet_inner, t0);
    let g1 = ambient_glyphs_for(Species::Fuzz, panel, pet_inner, t1);
    assert_ne!(g0, g1, "crossing drift boundary must change glyph placement");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib panels::pet::ambient_glyphs_for -- --nocapture`
Expected: FAIL — stub returns empty Vec.

- [ ] **Step 3: Implement Fuzz species placement**

Replace the stub body of `ambient_glyphs_for`:

```rust
const DRIFT_SECS: i64 = 8;

const FUZZ_GLYPHS: [char; 4] = ['·', '.', ',', '`'];

fn species_glyph_set(species: Species) -> &'static [char] {
    match species {
        Species::Fuzz    => &FUZZ_GLYPHS,
        Species::Blob    => &['o', '°', 'º'],
        Species::Ghost   => &['~', '・', '⋮'],
        Species::Glitch  => &['▒', '░', '▓', '▤'],
        Species::Crystal => &['✦', '✧', '◇', '⋄', '·'],
        Species::Mech    => &['·', '+', '╴', '╵', '╶'],
    }
}

fn species_seed(species: Species) -> u64 {
    // Stable hash of the species name — explicit so adding a species doesn't
    // shift existing seeds.
    match species {
        Species::Fuzz    => 0x0001_F4A0,
        Species::Blob    => 0x0001_F4A1,
        Species::Ghost   => 0x0001_F4A2,
        Species::Glitch  => 0x0001_F4A3,
        Species::Crystal => 0x0001_F4A4,
        Species::Mech    => 0x0001_F4A5,
    }
}

pub fn ambient_glyphs_for(
    species: Species,
    panel: Rect,
    pet_inner_rect: Rect,
    now: time::OffsetDateTime,
) -> Vec<AmbientGlyph> {
    let drift_phase = now.unix_timestamp() / DRIFT_SECS;
    let glyph_set = species_glyph_set(species);
    let seed = species_seed(species).wrapping_add(drift_phase as u64);

    let panel_cells = (panel.width as usize) * (panel.height as usize);
    let target_count = (panel_cells * 3 / 100).max(8); // ~3%, minimum 8

    let mut out = Vec::with_capacity(target_count);
    let mut rng = SplitMix64::new(seed);
    let mut attempts = 0_usize;
    while out.len() < target_count && attempts < target_count * 5 {
        attempts += 1;
        let col = panel.x + (rng.next() % panel.width as u64) as u16;
        let row = panel.y + (rng.next() % panel.height as u64) as u16;
        // Skip if it lands inside the pet inner rect.
        let in_pet_x = col >= pet_inner_rect.x && col < pet_inner_rect.x + pet_inner_rect.width;
        let in_pet_y = row >= pet_inner_rect.y && row < pet_inner_rect.y + pet_inner_rect.height;
        if in_pet_x && in_pet_y {
            continue;
        }
        // Skip duplicates.
        if out.iter().any(|g: &AmbientGlyph| g.row == row && g.col == col) {
            continue;
        }
        let glyph = glyph_set[(rng.next() as usize) % glyph_set.len()];
        let color = ambient_color_for(species);
        out.push(AmbientGlyph { row, col, glyph, color });
    }
    out
}

fn ambient_color_for(species: Species) -> Color {
    // Muted accent — palette-aware via tokenpet_palette()'s faint role.
    crate::tui::style::tokenpet_palette().faint.rgb
}

// Lightweight deterministic PRNG. Public API only inside this module.
struct SplitMix64 { state: u64 }
impl SplitMix64 {
    fn new(seed: u64) -> Self { Self { state: seed } }
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib panels::pet -- --nocapture`
Expected: PASS for all four ambient cases AND existing pet tests.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs
git commit -m "feat(panels/pet): ambient_glyphs_for — per-species placement

8-second wall-clock drift, ~3% density target, SplitMix64 RNG seeded
by (species, drift_phase). Glyphs never overlap the pet inner rect.
Per-species sets: fuzz '· . , \\`', blob 'o ° º', ghost '~ ・ ⋮',
glitch '▒ ░ ▓ ▤', crystal '✦ ✧ ◇ ⋄ ·', mech '· + ╴ ╵ ╶'."
```

---

### Task 31: Snapshot test — habitat renders for crystal without overlapping pet

**Files:**
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn pet_panel_crystal_renders_sparkles_around_pet_not_over_it() {
    use glorp::pet::{Species, Stage};
    let mut vm = WatchViewModel::fixture();
    vm.species = Species::Crystal;
    vm.stage = Stage::S2;
    let backend = TestBackend::new(40, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let ctx = test_context();
    let panel = PetPanel;
    terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx)).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
    // Crystal habitat glyph set contains ✦ — must appear in the rendered panel.
    assert!(s.contains('✦') || s.contains('✧') || s.contains('◇') || s.contains('⋄'),
        "crystal habitat should contribute at least one sparkle glyph");
}
```

- [ ] **Step 2-4: Run, fix, pass**

Run: `cargo test --test tui_render pet_panel_crystal -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/tui_render.rs
git commit -m "test(tui_render): crystal habitat renders sparkles around pet"
```

---

### Task 32: PR2 ship — full verification + dev-preview QA

- [ ] **Step 1: Full test + lint + fmt**

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```
All clean.

- [ ] **Step 2: dev-preview + QA**

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

Visually verify:
- Each species' pet panel shows its own glyph set scattered around the pet.
- Glyphs do not overlap the pet itself.
- Density looks ~3% (small numbers of subtle marks, not visual noise).
- Pet column has no dead bands.

- [ ] **Step 3: Open PR2**

```bash
git checkout -b watch-layout-refresh-pr2
git push -u origin watch-layout-refresh-pr2
gh pr create --title "feat(watch): habitat ambient glyphs (PR2)" --body "$(cat <<'EOF'
## Summary
- Fills in ambient_glyphs_for per-species: fuzz, blob, ghost, glitch, crystal, mech
- 8s wall-clock drift phase via SplitMix64 seeded by (species, drift_phase)
- ~3% panel density target; glyphs never overlap the pet inner rect
- Pure rendering — no view-model changes, no storage changes
- Builds on PR1 (#XXX)

Spec: docs/superpowers/specs/2026-05-12-watch-layout-refresh-design.md (revision 3, §"Habitat rendering")

## Test plan
- [ ] cargo test
- [ ] cargo clippy --all-targets --all-features -- -D warnings
- [ ] glorp dev-preview --scenario all and visually QA every species
- [ ] glorp watch — observe drift over ~10 seconds

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-review checklist

**Spec coverage:** Every spec section maps to a task:
- Storage layer (events_within, best_day, seven_day fix) → Tasks 1, 2, 3
- View model (ProgressView, BioView, plumbing, age_label formatter, XP unit clarification) → Tasks 4, 5, 6
- Watch builder (EMA, ProgressView, BioView, hatched format) → Tasks 7, 8, 9
- Color palette → Tasks 10, 16, 17, 18 (used across panels)
- Shared bars module → Tasks 11, 12, 13
- New panels (ProgressPanel, BioCardPanel) → Tasks 14, 15
- Modified panels (VitalsPanel, FeedPanel, TodayPanel, PetPanel scaffold) → Tasks 16-19
- Layout refactor (drop spark+helpers, new constraint sequences, pet_panel_rect) → Tasks 20, 21, 22
- Snapshot tests (120×32, 180×50, compact, panel-level) → Tasks 24, 25, plus inline in panel tasks
- Integration tests (bio, ⚠, EMA) → Tasks 26, 27, 28
- Habitat (per-species glyph placement, drift_phase) → Task 30
- Habitat snapshot test → Task 31
- PR1 + PR2 ship boundaries → Tasks 29, 32

**Placeholder scan:** No `TODO`, `TBD`, "implement later", or "similar to Task N" — every step contains the actual code or command needed.

**Type consistency:** `ProgressView`, `BioView`, `AmbientGlyph`, `ambient_glyphs_for`, `pet_inner_rect_in_panel`, `progress_rate_ema`, `bar_spans_solid`, `build_spark_line`, `format_tokens_short` / `_full`, `MAX_EVENT_ROWS=6`, `Stage::S6`, `Species::*` — names are stable across all tasks.

---

**Plan complete.** Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
