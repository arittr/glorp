# Glorp Lives In Time — Branch T1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `DayContext` foundation (local-day mapper, canonical day axis, activity rhythm, maturity gate, sleep predicate) and the flagship day/night + sleep/wake experience across the watch TUI and menubar.

**Spec:** `docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md` — requirements, semantics, and every named constant's meaning live there. Sections referenced per task. Branch context: work happens on `feat/lives-in-time`.

**Architecture:** A new injectable `LocalDayMapper` (timezone seam) feeds new applied-only SQL aggregates in `UsageStore`; `src/tui/day.rs` derives a `DayContext` once per poll inside `build_watch_view_model_at` and carries it on `WatchViewModel`; presentation consumers (render eyes, breath, wander, speech, narration, ambient sky, calm, menubar) read only the vm-carried context and precomputed UTC instants. No new persisted semantic state; one new index + a compaction-predicate fix.

**Tech Stack:** Rust, rusqlite 0.32 (bundled), time 0.3 (`local-offset`, `macros`), ratatui, insta snapshots, Preview Lab (`dev-preview`).

**House rules that bind every task:** TDD (test first, watch it fail, then implement); run `cargo fmt` and `cargo clippy --all-targets --all-features -- -D warnings` before every commit; never call `apply_usage_poll` from production code; sleep catch-up tests MUST drive `stage_usage_poll_deltas`, never hand-set old `bucket_at` values (spec: Sleep semantics).

**Verbatim-anchor warning:** Three extraction excerpts had known transcription drift — when editing, copy `old_string` anchors from the file at: `src/storage/usage_store.rs:773-784` (`today_effective_tokens` — SQL is a multi-line string literal), `src/game/runtime.rs:283-306` (`signal_freshness` — has `let daily = ...` at line 301), `src/storage/usage_store.rs:6-26` (`cost_usd: Option<f64>`). Always Read the target region before editing.

---

## File structure

| File | Status | Responsibility |
|---|---|---|
| `src/storage/day_axis.rs` | **create** | `LocalDayMapper` (Fixed / System / Scripted), local-date math, day-boundary instants. Pure time logic, no SQL. |
| `src/storage/mod.rs` (or `lib.rs` module decls) | modify | register `day_axis` module |
| `src/storage/usage_store.rs` | modify | `bucket_at` index; compaction `bucket_at` floor; new applied-only aggregates; reroute `today_effective_tokens` + `seven_day_token_history` |
| `src/tui/day.rs` | **create** | `DayPhase`, `DaySummary`, `DayContext`, rhythm/phase derivation, maturity gate, sleep predicate, season, date_seed |
| `src/tui/mod.rs` | modify | register `day` module |
| `src/tui/view_model.rs` | modify | `WatchViewModel.day_context` field + fixture |
| `src/commands/watch.rs` | modify | mapper param, DayContext build + stamping, speech/breath/eyes call sites, `rerender_pet_for_view_model` |
| `src/commands/status.rs` | modify | rerouted today reader; `scene_asleep` for narration |
| `src/game/runtime.rs` | modify | `apply_unapplied_usage` gains `scene_asleep: bool`; idle-narration partition |
| `src/pet/narration.rs` | modify | partition idle vocabulary (sleep-claiming vs neutral) |
| `src/pet/speech.rs` | modify | zzz cadence, sleep petting pool, asleep suppression |
| `src/pet/render.rs` | modify | `AnimationFrame.hold_eyes_closed` |
| `src/pet/animator.rs` | modify | sleep breath rhythm (onset-anchored); wander/facing hold + settle ease |
| `src/tui/app.rs` | modify | calm wiring, hold-eyes computation (milestone exemption), cursor gate, petting branch |
| `src/tui/panels/pet.rs` | modify | day-phase sky palettes/warmth/blend; sleep wander hold consumption; cursor gate |
| `src/menubar/app.rs` | modify | calm wiring, hold-eyes in `animate_pet` |
| `src/menubar/render.rs` | modify | asleep palette dim + BMP test |
| `src/dev_preview/watch.rs` + `src/dev_preview/scenarios.rs` | modify | DayContext fixtures, mapper injection, ordered pins |
| `tests/dev_preview.rs` | modify | night-asleep whole-frame snapshot |

Named constants introduced (all module-level with doc comments, per house style): `RHYTHM_WINDOW_DAYS=30`, `RHYTHM_QUIET_SHARE=0.01`, `MIN_NIGHT_RUN_HOURS=5`, `MAX_NIGHT_RUN_HOURS=12`, `PHASE_SHOULDER_HOURS=2`, `MIN_ACTIVE_DAYS=5`, `MIN_DISTINCT_ACTIVE_HOURS=3`, `SLEEP_IDLE_MINUTES=20`, `SLEEP_SPEECH_CYCLE_N=3`, `PHASE_BLEND_MINUTES=30`, `WANDER_SETTLE_SECS=8`, plus clock defaults (dawn 07–09, day 09–18, dusk 18–22, night 22–07).

---

### Task 1: `LocalDayMapper` — the timezone seam

**Spec section:** "Local-day mapper (the timezone seam)".

**Files:**
- Create: `src/storage/day_axis.rs`
- Modify: `src/storage/mod.rs` (add `pub mod day_axis;` next to the existing `pub mod usage_store;` — Read the file first; if module decls live in `lib.rs`/`main.rs`, add it there instead)
- Test: in-module `#[cfg(test)]` at the bottom of `day_axis.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// src/storage/day_axis.rs — bottom of the new file
#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn fixed_mapper_maps_instants_to_local_dates() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        // 2026-06-09 03:00 UTC is 2026-06-08 19:00 local at UTC-8.
        let instant = datetime!(2026-06-09 03:00 UTC);
        assert_eq!(
            mapper.local_date(instant),
            time::Date::from_calendar_date(2026, time::Month::June, 8).unwrap()
        );
    }

    #[test]
    fn local_day_start_is_local_midnight_in_utc() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        let date = time::Date::from_calendar_date(2026, time::Month::June, 8).unwrap();
        // Local midnight at UTC-8 == 08:00 UTC the same calendar day.
        assert_eq!(mapper.local_day_start(date), datetime!(2026-06-08 08:00 UTC));
    }

    #[test]
    fn day_starts_back_returns_ascending_boundaries_inclusive_of_today() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::UTC);
        let now = datetime!(2026-06-09 15:00 UTC);
        let starts = mapper.day_starts_back(now, 3);
        assert_eq!(
            starts,
            vec![
                datetime!(2026-06-07 00:00 UTC),
                datetime!(2026-06-08 00:00 UTC),
                datetime!(2026-06-09 00:00 UTC),
                datetime!(2026-06-10 00:00 UTC), // exclusive end boundary
            ]
        );
    }

    #[test]
    fn scripted_mapper_groups_dst_days_correctly() {
        // Simulated US spring-forward: UTC-8 before 2026-03-08 10:00 UTC,
        // UTC-7 after. The mapper resolves the offset per day boundary, so a
        // row at 2026-03-09 06:30 UTC (23:30 local UTC-7) groups to March 9,
        // while the same wall reading under the stale UTC-8 offset would have
        // grouped it to... (22:30 local, still March 9th); the discriminating
        // case is 2026-03-10 06:30 UTC: 23:30 local at UTC-7 -> March 9?? —
        // use the boundary instant itself: local midnight March 10 at UTC-7
        // is 07:00 UTC, at UTC-8 it would be 08:00 UTC. A row at 07:30 UTC
        // belongs to March 10 post-DST, but to March 9 under stale UTC-8.
        fn offset(at: time::OffsetDateTime) -> time::UtcOffset {
            if at < datetime!(2026-03-08 10:00 UTC) {
                time::UtcOffset::from_hms(-8, 0, 0).unwrap()
            } else {
                time::UtcOffset::from_hms(-7, 0, 0).unwrap()
            }
        }
        let mapper = LocalDayMapper::Scripted(offset);
        let row = datetime!(2026-03-10 07:30 UTC);
        assert_eq!(
            mapper.local_date(row),
            time::Date::from_calendar_date(2026, time::Month::March, 10).unwrap()
        );
        // Boundary instants on either side of the transition use their own
        // day's offset:
        let pre = time::Date::from_calendar_date(2026, time::Month::March, 7).unwrap();
        let post = time::Date::from_calendar_date(2026, time::Month::March, 10).unwrap();
        assert_eq!(mapper.local_day_start(pre), datetime!(2026-03-07 08:00 UTC));
        assert_eq!(mapper.local_day_start(post), datetime!(2026-03-10 07:00 UTC));
    }

    #[test]
    fn local_hour_uses_the_instants_own_offset() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(5, 30, 0).unwrap());
        let instant = datetime!(2026-06-09 03:00 UTC); // 08:30 local
        assert_eq!(mapper.local_hour(instant), 8);
    }
}
```

- [ ] **Step 2: Run tests, verify they fail to compile**

Run: `cargo test --lib day_axis 2>&1 | head -30`
Expected: compile error — `LocalDayMapper` not found (module doesn't exist yet).

- [ ] **Step 3: Implement the mapper**

```rust
//! Local-day axis mapping (the timezone seam).
//!
//! All "today / yesterday / trailing-N-day" math goes through one injectable
//! mapper so unit tests and Preview Lab can pin offsets while production
//! resolves the OS timezone. `System` resolves the UTC offset per calendar-day
//! boundary (one `localtime_r` per day in a window, never per row), so DST
//! days group correctly. Resolution failure falls back to UTC — a named
//! decision in the spec, not an accident.

use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalDayMapper {
    /// One constant offset. Tests, Preview Lab, and dev fixtures.
    Fixed(UtcOffset),
    /// Resolve via the OS per requested instant (UTC fallback on failure).
    System,
    /// Offset as a pure function of the instant. DST tests.
    Scripted(fn(OffsetDateTime) -> UtcOffset),
}

impl LocalDayMapper {
    pub fn offset_at(self, instant: OffsetDateTime) -> UtcOffset {
        match self {
            Self::Fixed(offset) => offset,
            Self::System => UtcOffset::local_offset_at(instant).unwrap_or(UtcOffset::UTC),
            Self::Scripted(f) => f(instant),
        }
    }

    /// The local calendar date containing `instant`.
    pub fn local_date(self, instant: OffsetDateTime) -> Date {
        instant.to_offset(self.offset_at(instant)).date()
    }

    /// Local hour-of-day (0-23) of `instant`.
    pub fn local_hour(self, instant: OffsetDateTime) -> u8 {
        instant.to_offset(self.offset_at(instant)).hour()
    }

    /// UTC instant of local midnight starting `date`. The offset is resolved
    /// once at (approximately) that boundary — per-day-boundary resolution is
    /// what makes DST windows group correctly without per-row libc calls.
    pub fn local_day_start(self, date: Date) -> OffsetDateTime {
        let approx = PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_utc();
        let offset = self.offset_at(approx);
        PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_offset(offset)
    }

    /// Ascending UTC boundaries for the `days_back` local days ending with the
    /// day containing `now`. Returns `days_back + 1` instants: index `i` is
    /// the start of day `i`, and the final element is the exclusive end of the
    /// last day (start of tomorrow). Day windows are half-open
    /// `[starts[i], starts[i+1])`.
    pub fn day_starts_back(self, now: OffsetDateTime, days_back: usize) -> Vec<OffsetDateTime> {
        let today = self.local_date(now);
        let mut starts = Vec::with_capacity(days_back + 1);
        for i in (0..days_back).rev() {
            let date = today - time::Duration::days(i as i64);
            starts.push(self.local_day_start(date));
        }
        starts.push(self.local_day_start(today + time::Duration::days(1)));
        starts
    }
}
```

Register the module: Read `src/storage/mod.rs` (or wherever `pub mod usage_store;` is declared) and add `pub mod day_axis;` alongside it.

Note on `day_starts_back`: it returns `days_back + 1` boundaries but the loop above pushes `days_back` starts + 1 end = correct length; the test pins 3 days → 4 instants.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test --lib day_axis -- --nocapture`
Expected: 5 passed.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/storage/day_axis.rs src/storage/mod.rs
git commit -m "feat(storage): add LocalDayMapper timezone seam for the canonical day axis"
```

---

### Task 2: `bucket_at` index + compaction retention fix

**Spec sections:** "Performance mandate" (index ruling) and "Retention fix (absorbed)".

**Files:**
- Modify: `src/storage/usage_store.rs` (`migrate()` tail batch at ~1006-1017; `compact_before` at ~246-294)
- Test: in-module tests at the bottom of `usage_store.rs`

- [ ] **Step 1: Write the failing retention test**

The bug: a long-gap resume writes rows with `period_start` >90 days old but `bucket_at = now`; today's `compact_before(now - 90d)` deletes them while they're still inside every DayContext window. The fix floors deletion on BOTH axes.

```rust
    #[test]
    fn compact_before_keeps_rows_with_recent_bucket_at_even_when_period_start_is_ancient() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let ancient_period = now - time::Duration::days(120);
        // Long-gap resume shape: provider day is 120 days old, but the smear
        // anchored the bucket at poll time (now).
        let event = NormalizedUsageEvent {
            period_start: ancient_period,
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 9_999.0)
        };
        store.insert_event(&event).unwrap(); // insert_event rows are born applied
        store.compact_before(now - time::Duration::days(90)).unwrap();
        let totals = store
            .token_totals_by_source_between(now - time::Duration::minutes(10), now)
            .unwrap();
        let sum: f64 = totals.iter().map(|(_, v)| v).sum();
        assert_eq!(
            sum, 9_999.0,
            "rows still inside live bucket_at windows must survive compaction"
        );
    }

    #[test]
    fn compact_before_still_compacts_rows_old_on_both_axes() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let old = now - time::Duration::days(120);
        store.insert_event(&sample_event_at(old, 4_444.0)).unwrap();
        store.compact_before(now - time::Duration::days(90)).unwrap();
        let totals = store
            .token_totals_by_source_between(old - time::Duration::minutes(10), now)
            .unwrap();
        let sum: f64 = totals.iter().map(|(_, v)| v).sum();
        assert_eq!(sum, 0.0, "both-axes-old rows must move into daily_aggregates");
    }

    #[test]
    fn migrate_creates_bucket_at_index() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_usage_events_bucket_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
```

- [ ] **Step 2: Run, verify failures**

Run: `cargo test --lib compact_before_keeps -- --nocapture && cargo test --lib migrate_creates_bucket_at_index`
Expected: `compact_before_keeps_rows_with_recent_bucket_at...` FAILS (sum is 0.0 — row was deleted); `migrate_creates_bucket_at_index` FAILS (count 0).

- [ ] **Step 3: Implement both fixes**

In `migrate()`'s tail `execute_batch` (Read `src/storage/usage_store.rs:1006-1017` for the exact anchor), add to the batch string:

```sql
            CREATE INDEX IF NOT EXISTS idx_usage_events_bucket_at
                ON usage_events(bucket_at);
```

In `compact_before` (Read lines 246-294), change both WHERE clauses:

```sql
            WHERE period_start < ?1 AND bucket_at < ?1 AND applied_at IS NOT NULL
```

(the SELECT-into-aggregates statement and the DELETE statement — both; the `?1` param is already bound via `format_time(cutoff)?` in each statement).

- [ ] **Step 4: Run, verify all storage tests pass**

Run: `cargo test --lib usage_store`
Expected: all pass, including the pre-existing compaction tests (rows old on both axes still compact).

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/storage/usage_store.rs
git commit -m "fix(storage): floor compaction on bucket_at and index the bucket_at axis"
```

---

### Task 3: Applied-only day-window aggregates on `UsageStore`

**Spec sections:** "Performance mandate" (SQL-side aggregation, once per poll) and "DayContext" field sources.

**Files:**
- Modify: `src/storage/usage_store.rs` (new methods near `token_totals_by_source_between`)
- Test: in-module tests

Design notes locked here: all reads filter `applied_at IS NOT NULL`; day windows are half-open `[start, end)` (`bucket_at >= ?1 AND bucket_at < ?2`) — safe against the RFC3339 `Z`-vs-fractional lexical gotcha because `bucket_at` values are always 10-minute-floored (no fractional seconds) and boundaries are local midnights, so formats align; each row lands in exactly one day. The sleep-recency read uses `bucket_at >= ?1` with NO upper bound — future-dated rows (clock set backwards) therefore count as recent activity automatically, which is the spec's fail-awake rule. The histogram source returns 10-minute `GROUP BY bucket_at` sums (≤ ~4.3k rows for 30 days), not raw rows — Rust then buckets by local hour via the mapper; this satisfies "never row-fetch-and-bucket" (the 130k-row anti-pattern) while keeping SQL date-math out of the codebase (no `strftime`/`datetime()` anywhere today; preserve that).

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn applied_effective_tokens_between_excludes_unapplied_rows() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1_000.0)).unwrap(); // applied
        let staged = NormalizedUsageEvent {
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 500.0)
        };
        store
            .insert_unapplied_event_bucket(
                &staged,
                &ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "k".into(),
                    cursor_value: "v".into(),
                    provider_version: "p".into(),
                    parser_version: "q".into(),
                },
                0,
                1,
            )
            .unwrap();
        let sum = store
            .applied_effective_tokens_between(now - time::Duration::hours(1), now + time::Duration::seconds(1))
            .unwrap();
        assert_eq!(sum, 1_000.0, "staged rows must not leak into DayContext reads");
    }

    #[test]
    fn applied_effective_tokens_between_is_half_open() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let end = start + time::Duration::hours(24);
        store.insert_event(&sample_event_at(start, 1.0)).unwrap(); // == start: in
        store.insert_event(&sample_event_at(end, 2.0)).unwrap(); // == end: out
        assert_eq!(store.applied_effective_tokens_between(start, end).unwrap(), 1.0);
    }

    #[test]
    fn applied_bucket_sums_between_groups_by_bucket() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let t0 = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(t0, 100.0)).unwrap();
        store.insert_event(&sample_event_at(t0, 50.0)).unwrap(); // same bucket
        store
            .insert_event(&sample_event_at(t0 + time::Duration::minutes(10), 25.0))
            .unwrap();
        let sums = store
            .applied_bucket_sums_between(t0 - time::Duration::minutes(10), t0 + time::Duration::hours(1))
            .unwrap();
        assert_eq!(sums.len(), 2);
        assert_eq!(sums[0].1, 150.0);
        assert_eq!(sums[1].1, 25.0);
    }

    #[test]
    fn applied_token_shape_between_sums_components() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mut event = sample_event_at(now, 1_030.0);
        event.input_tokens = 500.0;
        event.output_tokens = 400.0;
        event.cache_creation_tokens = 100.0;
        event.cache_read_tokens = 1_000.0; // effective contribution = 30 at 0.03
        event.reasoning_output_tokens = 0.0;
        store.insert_event(&event).unwrap();
        let shape = store
            .applied_token_shape_between(now - time::Duration::hours(1), now + time::Duration::seconds(1))
            .unwrap();
        assert_eq!(shape.input_tokens, 500.0);
        assert_eq!(shape.output_tokens, 400.0);
        assert_eq!(shape.cache_creation_tokens, 100.0);
        assert_eq!(shape.cache_read_tokens, 1_000.0);
        assert_eq!(shape.effective_tokens, 1_030.0);
    }

    #[test]
    fn latest_applied_bucket_at_and_existence_probes() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        assert!(!store.has_any_applied_events().unwrap());
        assert_eq!(store.latest_applied_bucket_at().unwrap(), None);
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1.0)).unwrap();
        store
            .insert_event(&sample_event_at(now - time::Duration::hours(2), 1.0))
            .unwrap();
        assert!(store.has_any_applied_events().unwrap());
        assert_eq!(store.latest_applied_bucket_at().unwrap(), Some(now));
        assert_eq!(
            store.latest_applied_bucket_at_before(now).unwrap(),
            Some(now - time::Duration::hours(2))
        );
    }
```

- [ ] **Step 2: Run, verify compile failures** (methods don't exist)

Run: `cargo test --lib applied_ 2>&1 | head -20`

- [ ] **Step 3: Implement the five readers**

Add near `token_totals_by_source_between` (match its style — prepared statement, `params![]`, `format_time`):

```rust
    /// Applied-only effective-token sum over the half-open bucket_at window
    /// `[start, end)`. Day boundaries are local midnights and bucket_at values
    /// are 10-minute-floored, so formats align and each row lands in exactly
    /// one day window (the inclusive-bounds Z-gotcha only bites when mixing
    /// fractional and whole-second timestamps).
    pub fn applied_effective_tokens_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<f64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(effective_tokens), 0.0)
                 FROM usage_events
                 WHERE applied_at IS NOT NULL AND bucket_at >= ?1 AND bucket_at < ?2",
                params![format_time(start)?, format_time(end)?],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    /// Applied-only per-10-minute-bucket sums over `[start, end)`, ascending.
    /// At most ~144 rows/day — the pre-aggregated source for the local-hour
    /// rhythm histogram (Rust maps buckets to local hours via the mapper).
    pub fn applied_bucket_sums_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<Vec<(OffsetDateTime, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT bucket_at, SUM(effective_tokens)
             FROM usage_events
             WHERE applied_at IS NOT NULL AND bucket_at >= ?1 AND bucket_at < ?2
             GROUP BY bucket_at
             ORDER BY bucket_at ASC",
        )?;
        let rows = stmt
            .query_map(params![format_time(start)?, format_time(end)?], |row| {
                let at: String = row.get(0)?;
                Ok((parse_time_for_sql(&at)?, row.get::<_, f64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Applied-only token-shape component sums over `[start, end)`.
    pub fn applied_token_shape_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<AppliedShapeSums> {
        self.conn
            .query_row(
                "SELECT
                    COALESCE(SUM(input_tokens), 0.0),
                    COALESCE(SUM(output_tokens), 0.0),
                    COALESCE(SUM(cache_creation_tokens), 0.0),
                    COALESCE(SUM(cache_read_tokens), 0.0),
                    COALESCE(SUM(reasoning_output_tokens), 0.0),
                    COALESCE(SUM(effective_tokens), 0.0)
                 FROM usage_events
                 WHERE applied_at IS NOT NULL AND bucket_at >= ?1 AND bucket_at < ?2",
                params![format_time(start)?, format_time(end)?],
                |row| {
                    Ok(AppliedShapeSums {
                        input_tokens: row.get(0)?,
                        output_tokens: row.get(1)?,
                        cache_creation_tokens: row.get(2)?,
                        cache_read_tokens: row.get(3)?,
                        reasoning_output_tokens: row.get(4)?,
                        effective_tokens: row.get(5)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    /// Newest applied bucket_at, if any. No upper bound: future-dated rows
    /// (clock set backwards) surface here, which is the fail-awake rule.
    pub fn latest_applied_bucket_at(&self) -> crate::error::Result<Option<OffsetDateTime>> {
        let max: Option<String> = self.conn.query_row(
            "SELECT MAX(bucket_at) FROM usage_events WHERE applied_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        max.map(|s| parse_time_for_sql(&s).map_err(Into::into))
            .transpose()
    }

    /// Newest applied bucket_at strictly before `at` (wake-resume easing).
    pub fn latest_applied_bucket_at_before(
        &self,
        at: OffsetDateTime,
    ) -> crate::error::Result<Option<OffsetDateTime>> {
        let max: Option<String> = self.conn.query_row(
            "SELECT MAX(bucket_at) FROM usage_events
             WHERE applied_at IS NOT NULL AND bucket_at < ?1",
            params![format_time(at)?],
            |row| row.get(0),
        )?;
        max.map(|s| parse_time_for_sql(&s).map_err(Into::into))
            .transpose()
    }

    /// Whether the pet has ever eaten (any applied row). Newborn sleep gate.
    pub fn has_any_applied_events(&self) -> crate::error::Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM usage_events WHERE applied_at IS NOT NULL)",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v != 0)
            .map_err(Into::into)
    }
```

And the sums struct (next to `NormalizedUsageEvent`):

```rust
/// Component sums for a day-window token-shape read (DaySummary/climate input).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AppliedShapeSums {
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_creation_tokens: f64,
    pub cache_read_tokens: f64,
    pub reasoning_output_tokens: f64,
    pub effective_tokens: f64,
}
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test --lib usage_store`
Expected: all pass.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/storage/usage_store.rs
git commit -m "feat(storage): add applied-only day-window aggregates for DayContext"
```

---

### Task 4: Reroute the existing readers onto the canonical axis

**Spec section:** "Canonical local-day axis (prerequisite, absorbed into T1)". The eat-time attribution trade-off is recorded there; do not re-litigate it in code comments.

**Files:**
- Modify: `src/storage/usage_store.rs` (`today_effective_tokens` ~773-784; `seven_day_token_history` ~819-863 and its superseded test ~1243-1275)
- Modify: `src/commands/status.rs` (~43)
- Modify: `src/commands/watch.rs` (wrapper + `build_watch_view_model_at` signature: `local_offset: time::UtcOffset` → `mapper: LocalDayMapper`; callers at ~84-96, ~151-154; test seam ~296-303)
- Modify: `src/dev_preview/watch.rs` (~157: `UtcOffset::UTC` → `LocalDayMapper::Fixed(UtcOffset::UTC)`)
- Test: in-module tests in `usage_store.rs` and `watch.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    // usage_store.rs tests
    #[test]
    fn today_effective_tokens_groups_on_local_bucket_at_day_applied_only() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        // 2026-06-09 01:00 UTC == 2026-06-08 17:00 local at UTC-8: yesterday locally.
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        let now = datetime!(2026-06-09 18:00 UTC); // 10:00 local June 9
        store
            .insert_event(&sample_event_at(datetime!(2026-06-09 01:00 UTC), 7_777.0))
            .unwrap(); // local June 8 — must NOT count
        store
            .insert_event(&sample_event_at(datetime!(2026-06-09 17:00 UTC), 1_111.0))
            .unwrap(); // local June 9 09:00 — counts
        assert_eq!(store.today_effective_tokens(now, mapper).unwrap(), 1_111.0);
    }

    #[test]
    fn seven_day_token_history_uses_local_bucket_at_days_and_no_aggregates_union() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::UTC);
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1_234.0)).unwrap();
        store
            .insert_event(&sample_event_at(now - time::Duration::days(6), 7_777.0))
            .unwrap();
        // A daily_aggregates row must NOT surface: compaction cutoff is 90
        // days, so aggregates cannot occur inside a 7-day window. This
        // supersedes seven_day_token_history_includes_compacted_days.
        store
            .conn
            .execute(
                "INSERT INTO daily_aggregates (
                    provider_surface, period_date, source_surface,
                    input_tokens, output_tokens, cache_creation_tokens,
                    cache_read_tokens, reasoning_output_tokens,
                    effective_tokens, cost_usd, event_count
                ) VALUES ('claude-code', ?1, 'daily', 0, 0, 0, 0, 0, 5555.0, 0, 1)",
                rusqlite::params![(now.date() - time::Duration::days(3)).to_string()],
            )
            .unwrap();
        let history = store.seven_day_token_history(now, mapper).unwrap();
        assert_eq!(history.len(), 7);
        assert_eq!(history[0], 7_777.0);
        assert_eq!(history[3], 0.0, "aggregates must not leak into the 7-day window");
        assert_eq!(history[6], 1_234.0);
    }
```

In `watch.rs` tests, a midnight-agreement test:

```rust
    #[test]
    fn status_today_and_watch_today_agree_across_a_midnight_boundary() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage = UsageStore::open(&db_path).unwrap();
        let mapper = crate::storage::day_axis::LocalDayMapper::Fixed(
            time::UtcOffset::from_hms(-8, 0, 0).unwrap(),
        );
        // 23:30 local June 8 (07:30 UTC June 9) — late-night work.
        let late = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::June, 9).unwrap(),
            Time::from_hms(7, 30, 0).unwrap(),
        )
        .assume_utc();
        usage
            .insert_event(&NormalizedUsageEvent {
                observed_at: late,
                bucket_at: late,
                ..NormalizedUsageEvent::for_test_at(late, 3_000.0)
            })
            .unwrap();
        // Now = 00:30 local June 9 (08:30 UTC): the late-night row is YESTERDAY.
        let now = late + Duration::hours(1);
        let state = PetState::new_for_test("test", "buddy");
        let vm = build_watch_view_model_at(&state, &db_path, now, mapper).unwrap();
        let status_today = usage.today_effective_tokens(now, mapper).unwrap();
        assert_eq!(vm.today_effective_tokens, status_today);
        assert_eq!(status_today, 0.0, "yesterday's local work must not be today");
    }
```

And the spec-listed backlog-convergence test (applied-only reads make a large staged backfill visible only as it applies — `apply_unapplied_usage` caps at 500 rows/run):

```rust
    // watch.rs tests
    #[test]
    fn oversized_staged_backlog_becomes_visible_over_successive_applies() {
        use crate::game::runtime::{apply_unapplied_usage, stage_usage_poll_deltas};
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage = UsageStore::open(&db_path).unwrap();
        let mut state = PetState::new_for_test("seed", "buddy");
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mapper = crate::storage::day_axis::LocalDayMapper::Fixed(time::UtcOffset::UTC);
        // Stage > 500 rows via the REAL path: 60 deltas x ~6-12 smear buckets.
        for i in 0..60 {
            let poll = catchup_poll_result_for_test_n(i, 50_000.0); // distinct cursor per delta
            stage_usage_poll_deltas(&mut usage, &poll, state.calibration, now).unwrap();
        }
        let before = usage.today_effective_tokens(now, mapper).unwrap();
        let update = apply_unapplied_usage(&mut state, &mut usage, now, false).unwrap();
        usage
            .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
            .unwrap();
        let after_one = usage.today_effective_tokens(now, mapper).unwrap();
        assert_eq!(before, 0.0, "staged rows are invisible to applied-only reads");
        assert!(after_one > 0.0, "the first apply makes <=500 rows visible");
        // Drain the backlog with successive apply/mark cycles; totals converge.
        for _ in 0..20 {
            let u = apply_unapplied_usage(&mut state, &mut usage, now, false).unwrap();
            usage
                .mark_events_applied_and_advance_cursors(&u.applied_event_ids, now)
                .unwrap();
        }
        let drained = usage.today_effective_tokens(now, mapper).unwrap();
        assert!(
            drained > after_one,
            "successive polls converge the backlog into the visible total"
        );
    }
```

(`catchup_poll_result_for_test_n` = the Task 6 helper parameterized with a distinct `cursor_value` per call so the idempotency index doesn't collapse the deltas. Note the 4-arg `apply_unapplied_usage` — same Task 11 dependency as the catch-up wake test; `#[ignore]` until then if executing strictly in order.)

- [ ] **Step 2: Run, verify failures** (signature mismatches / old behavior)

Run: `cargo test --lib today_effective 2>&1 | head -20`

- [ ] **Step 3: Implement the reroutes**

`today_effective_tokens` (replace the whole body — Read lines 773-784 first):

```rust
    /// Today's applied effective tokens on the canonical local-day axis
    /// (local day of bucket_at). Half-open [local midnight, local midnight+1d)
    /// so late-night rows never double-count across the boundary.
    pub fn today_effective_tokens(
        &self,
        now: OffsetDateTime,
        mapper: crate::storage::day_axis::LocalDayMapper,
    ) -> crate::error::Result<f64> {
        let today = mapper.local_date(now);
        let start = mapper.local_day_start(today);
        let end = mapper.local_day_start(today + time::Duration::days(1));
        self.applied_effective_tokens_between(start, end)
    }
```

`seven_day_token_history` (replace the whole body):

```rust
    /// Effective tokens per local day for the trailing 7 local days (oldest
    /// first, today last), on the canonical bucket_at axis, applied rows only.
    /// daily_aggregates is deliberately NOT consulted: compaction's cutoff is
    /// 90 days, so aggregate rows cannot occur inside a 7-day window.
    pub fn seven_day_token_history(
        &self,
        now: OffsetDateTime,
        mapper: crate::storage::day_axis::LocalDayMapper,
    ) -> crate::error::Result<Vec<f64>> {
        let starts = mapper.day_starts_back(now, 7);
        let mut out = Vec::with_capacity(7);
        for pair in starts.windows(2) {
            out.push(self.applied_effective_tokens_between(pair[0], pair[1])?);
        }
        Ok(out)
    }
```

Delete the superseded test `seven_day_token_history_includes_compacted_days` (lines ~1243-1275) — it is replaced by `seven_day_token_history_uses_local_bucket_at_days_and_no_aggregates_union` above, which pins the inverse behavior for the reason documented in the new doc comment. This is an explicit supersession required by the spec, not a deleted-failing-test.

`build_watch_view_model_at`: change the parameter `local_offset: time::UtcOffset` to `mapper: LocalDayMapper`, and inside (Read lines 84-96 first) replace the `now_local`/`today_start` computation:

```rust
    let local_offset = mapper.offset_at(now);
    let now_local = now.to_offset(local_offset);
    let today_start = mapper.local_day_start(mapper.local_date(now));
```

(keep `now_local` — downstream display code uses it). Update the `seven_day_token_history` call (~151-154) to `usage_store.seven_day_token_history(now, mapper)`. Update the wrapper:

```rust
pub fn build_watch_view_model(state: &PetState, usage_db: &Path) -> Result<WatchViewModel> {
    build_watch_view_model_at(
        state,
        usage_db,
        OffsetDateTime::now_utc(),
        LocalDayMapper::System,
    )
}
```

and the `#[doc(hidden)]` test seam (~296-303) to pass `LocalDayMapper::Fixed(time::UtcOffset::UTC)`. Update `src/dev_preview/watch.rs:157` the same way. The 1-hour rate window (~176-182) and `last_10m_start` stay on `token_totals_by_source_between` — they are rolling windows, NOT day-axis reads; do not touch them. The today-panel read (`token_totals_by_source_between(today_start, now)`) keeps its per-source grouping and now gets a mapper-resolved `today_start` — that is the entire reroute for watch's today.

`status.rs` (~43): `now` is out of scope there (it lives inside the poll match arm); re-resolve:

```rust
                let status_now = OffsetDateTime::now_utc();
                today_effective = usage_store
                    .today_effective_tokens(
                        status_now,
                        crate::storage::day_axis::LocalDayMapper::System,
                    )
                    .unwrap_or(0.0);
```

The compiler will flag every remaining caller of the old signatures — fix each to pass `(now, mapper)`; tests pass `LocalDayMapper::Fixed(time::UtcOffset::UTC)`.

- [ ] **Step 4: Run the full suite** (this task ripples)

Run: `cargo test`
Expected: all pass. Pay attention to watch snapshot/vm tests — today/7-day semantics changed from UTC-`period_date` to local-`bucket_at` (and applied-only); any test that fixtured unapplied rows into "today" now correctly sees 0 and must be updated to insert applied rows (`insert_event`) instead — update the fixture, never weaken the assertion.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(storage): route today and 7-day readers onto the canonical local-day axis"
```

---

### Task 5: Activity rhythm — histogram to phase windows

**Spec section:** "Activity rhythm (day_phase derivation)" — the algorithm, clamps, tie/fallback rules, and the honesty note about poll-anchored bucket_at all live there.

**Files:**
- Create: `src/tui/day.rs` (first half: constants, `DayPhase`, `PhaseWindows`, derivation)
- Modify: `src/tui/mod.rs` (add `pub mod day;`)
- Test: in-module

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn histogram_active(hours: &[u8]) -> [f64; 24] {
        let mut h = [0.0_f64; 24];
        for &hour in hours {
            h[hour as usize] = 1_000.0;
        }
        h
    }

    #[test]
    fn typical_nine_to_six_worker_gets_carved_windows() {
        // Active 9..18 → quiet run 18..9 (15h) → clamped to 12h centered on
        // the quiet midpoint (1.5 ≈ hour 1; window 19..7 with wraparound),
        // dusk = first 2h of the clamped window, dawn = last 2h.
        let w = derive_phase_windows(&histogram_active(&[9, 10, 11, 12, 13, 14, 15, 16, 17]));
        assert_eq!(w, PhaseWindows { dusk_start: 19, night_start: 21, dawn_start: 5, day_start: 7 });
    }

    #[test]
    fn night_owl_windows_invert() {
        // Active 22..04 (wraps) → quiet run 4..22 (18h) → clamp to 12h
        // centered on quiet midpoint (hour 13; window 7..19).
        let w = derive_phase_windows(&histogram_active(&[22, 23, 0, 1, 2, 3]));
        assert_eq!(w, PhaseWindows { dusk_start: 7, night_start: 9, dawn_start: 17, day_start: 19 });
    }

    #[test]
    fn short_active_day_user_keeps_a_real_day() {
        // Active only 10..14 → quiet run 14..10 (20h) → clamped to 12h.
        let w = derive_phase_windows(&histogram_active(&[10, 11, 12, 13]));
        // Quiet midpoint of 14..10 is hour 0; clamped window 18..06.
        assert_eq!(w, PhaseWindows { dusk_start: 18, night_start: 20, dawn_start: 4, day_start: 6 });
        // Day must exist: from day_start (6) to dusk_start (18) = 12 hours.
    }

    #[test]
    fn no_quiet_run_falls_back_to_clock_defaults() {
        let w = derive_phase_windows(&histogram_active(&(0..24).collect::<Vec<_>>()));
        assert_eq!(w, PhaseWindows::clock_defaults());
    }

    #[test]
    fn short_quiet_run_falls_back_to_clock_defaults() {
        // Only a 4-hour quiet gap (< MIN_NIGHT_RUN_HOURS=5).
        let active: Vec<u8> = (0..24).filter(|h| !(2..6).contains(h)).collect();
        let w = derive_phase_windows(&histogram_active(&active));
        assert_eq!(w, PhaseWindows::clock_defaults());
    }

    #[test]
    fn equal_length_quiet_runs_fall_back_to_clock_defaults() {
        // Two 9-hour quiet runs: active at 3,4,5 and 15,16,17 → quiet runs
        // 6..15 and 18..03, both 9h. Ambiguous → defaults.
        let w = derive_phase_windows(&histogram_active(&[3, 4, 5, 15, 16, 17]));
        assert_eq!(w, PhaseWindows::clock_defaults());
    }

    #[test]
    fn split_shift_picks_the_longest_quiet_run() {
        // Active 8..12 and 18..22: quiet runs 12..18 (6h) and 22..8 (10h).
        // Longest = 22..8 (10h, no clamp needed), carve shoulders.
        let w = derive_phase_windows(&histogram_active(&[8, 9, 10, 11, 18, 19, 20, 21]));
        assert_eq!(w, PhaseWindows { dusk_start: 22, night_start: 0, dawn_start: 6, day_start: 8 });
    }

    #[test]
    fn quiet_share_threshold_ignores_trace_activity() {
        // One hour with 0.5% of total volume still counts as quiet.
        let mut h = histogram_active(&[9, 10, 11, 12, 13, 14, 15, 16, 17]);
        let total: f64 = h.iter().sum();
        h[2] = total * 0.005; // below RHYTHM_QUIET_SHARE = 1%
        let with_trace = derive_phase_windows(&h);
        let without = derive_phase_windows(&histogram_active(&[9, 10, 11, 12, 13, 14, 15, 16, 17]));
        assert_eq!(with_trace, without);
    }

    #[test]
    fn phase_for_hour_maps_circular_ranges() {
        let w = PhaseWindows::clock_defaults(); // dawn 7, day 9, dusk 18, night 22
        assert_eq!(w.phase_for_hour(7), DayPhase::Dawn);
        assert_eq!(w.phase_for_hour(8), DayPhase::Dawn);
        assert_eq!(w.phase_for_hour(9), DayPhase::Day);
        assert_eq!(w.phase_for_hour(17), DayPhase::Day);
        assert_eq!(w.phase_for_hour(18), DayPhase::Dusk);
        assert_eq!(w.phase_for_hour(21), DayPhase::Dusk);
        assert_eq!(w.phase_for_hour(22), DayPhase::Night);
        assert_eq!(w.phase_for_hour(3), DayPhase::Night);
        assert_eq!(w.phase_for_hour(6), DayPhase::Night);
    }
}
```

Note for the implementer: the expected window values in the first three tests are DERIVED from the algorithm below (clamp → center on quiet midpoint → carve 2h shoulders). After implementing, if a value differs by one hour due to midpoint rounding, verify the rounding rule (round-half-down via integer arithmetic, below) and fix the TEST EXPECTATION ONLY if your hand-derivation confirms the algorithm output — document the derivation in a comment. Do not bend the algorithm to the tests.

- [ ] **Step 2: Run, verify compile failure**

Run: `cargo test --lib tui::day 2>&1 | head -20`

- [ ] **Step 3: Implement**

```rust
//! DayContext: the derived time-of-day presentation layer.
//!
//! See docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md.
//! Everything here is a pure function of (clock, mapper, ledger aggregates);
//! nothing is persisted.

use time::OffsetDateTime;

/// Trailing window for the activity-rhythm histogram, in local days.
pub const RHYTHM_WINDOW_DAYS: usize = 30;
/// An hour is "quiet" when its share of window volume is below this.
pub const RHYTHM_QUIET_SHARE: f64 = 0.01;
/// Quiet runs shorter than this can't be a night — fall back to defaults.
pub const MIN_NIGHT_RUN_HOURS: usize = 5;
/// Quiet runs longer than this are clamped (a 4h/day user must keep a Day).
pub const MAX_NIGHT_RUN_HOURS: usize = 12;
/// Dawn/Dusk shoulders carved from the quiet window's edges.
pub const PHASE_SHOULDER_HOURS: u8 = 2;
/// Personalization needs at least this many distinct active local days...
pub const MIN_ACTIVE_DAYS: usize = 5;
/// ...and this many distinct active hours (hour diversity).
pub const MIN_DISTINCT_ACTIVE_HOURS: usize = 3;
/// The pet sleeps only after this many minutes of night-phase ledger quiet,
/// and re-arms on the same window after a wake (symmetric by construction).
pub const SLEEP_IDLE_MINUTES: i64 = 20;
/// Phase palettes interpolate over this window after a phase boundary.
pub const PHASE_BLEND_MINUTES: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

/// Local-hour starts of each phase, circular. dusk_start..night_start = Dusk,
/// night_start..dawn_start = Night, dawn_start..day_start = Dawn, rest = Day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseWindows {
    pub dusk_start: u8,
    pub night_start: u8,
    pub dawn_start: u8,
    pub day_start: u8,
}

impl PhaseWindows {
    /// Hand-set defaults until the ledger is mature: dawn 07-09, day 09-18,
    /// dusk 18-22, night 22-07.
    pub fn clock_defaults() -> Self {
        Self { dusk_start: 18, night_start: 22, dawn_start: 7, day_start: 9 }
    }

    pub fn phase_for_hour(&self, hour: u8) -> DayPhase {
        let h = hour % 24;
        if in_circular_range(h, self.dusk_start, self.night_start) {
            DayPhase::Dusk
        } else if in_circular_range(h, self.night_start, self.dawn_start) {
            DayPhase::Night
        } else if in_circular_range(h, self.dawn_start, self.day_start) {
            DayPhase::Dawn
        } else {
            DayPhase::Day
        }
    }
}

/// Half-open circular hour range test: start <= h < end, wrapping midnight.
fn in_circular_range(h: u8, start: u8, end: u8) -> bool {
    if start <= end {
        (start..end).contains(&h)
    } else {
        h >= start || h < end
    }
}

/// Derive phase windows from a local-hour volume histogram. Returns clock
/// defaults when the histogram has no usable quiet run (empty, all-active,
/// too-short run, or an ambiguous tie). See spec "Activity rhythm".
pub fn derive_phase_windows(histogram: &[f64; 24]) -> PhaseWindows {
    let total: f64 = histogram.iter().sum();
    if total <= 0.0 {
        return PhaseWindows::clock_defaults();
    }
    let quiet: Vec<bool> = histogram
        .iter()
        .map(|&v| v / total < RHYTHM_QUIET_SHARE)
        .collect();

    // Longest contiguous circular quiet run; ties are ambiguous.
    let mut best_start = 0_usize;
    let mut best_len = 0_usize;
    let mut tie = false;
    for start in 0..24 {
        if !quiet[start] || quiet[(start + 23) % 24] {
            continue; // only run heads (previous hour active)
        }
        let mut len = 0;
        while len < 24 && quiet[(start + len) % 24] {
            len += 1;
        }
        if len > best_len {
            best_len = len;
            best_start = start;
            tie = false;
        } else if len == best_len && len > 0 {
            tie = true;
        }
    }
    // All 24 quiet (no run head found) or nothing quiet:
    if best_len == 0 || best_len == 24 || tie {
        return PhaseWindows::clock_defaults();
    }
    if best_len < MIN_NIGHT_RUN_HOURS {
        return PhaseWindows::clock_defaults();
    }

    // Clamp over-long quiet runs to MAX, centered on the run midpoint.
    let (q_start, q_len) = if best_len > MAX_NIGHT_RUN_HOURS {
        let midpoint = (best_start + best_len / 2) % 24;
        let clamped_start = (midpoint + 24 - MAX_NIGHT_RUN_HOURS / 2) % 24;
        (clamped_start, MAX_NIGHT_RUN_HOURS)
    } else {
        (best_start, best_len)
    };

    let shoulder = PHASE_SHOULDER_HOURS as usize;
    PhaseWindows {
        dusk_start: (q_start % 24) as u8,
        night_start: ((q_start + shoulder) % 24) as u8,
        dawn_start: ((q_start + q_len - shoulder) % 24) as u8,
        day_start: ((q_start + q_len) % 24) as u8,
    }
}
```

Register `pub mod day;` in `src/tui/mod.rs` (Read it first to match declaration style).

- [ ] **Step 4: Run, verify pass; hand-check the three derived expectations**

Run: `cargo test --lib tui::day -- --nocapture`
Expected: all pass. For each of the first three tests, re-derive on paper (quiet run → clamp center → carve) and confirm the constants in the assertions match the implementation's integer rounding; adjust test expectations only with a written derivation comment.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/tui/day.rs src/tui/mod.rs
git commit -m "feat(tui): derive day-phase windows from the activity rhythm histogram"
```

---

### Task 6: `DayContext` — struct, builder, and vm wiring

**Spec sections:** "DayContext: the new derived layer" (field table + derivation notes), "Sleep semantics" (predicate, newborn gate, accepted catch-up wake), "Honesty and degradation rules" (maturity gate).

**Files:**
- Modify: `src/tui/day.rs` (second half: `DayContext`, `DaySummary`, `Season`, builder, sleep predicate)
- Modify: `src/tui/view_model.rs` (new `day_context` field; update `WatchViewModel::fixture()`)
- Modify: `src/commands/watch.rs` (`build_watch_view_model_at`: build DayContext early, stamp on vm)
- Test: in-module tests in `day.rs` + `watch.rs`

Design notes locked here:
- The builder runs **once per poll** inside `build_watch_view_model_at` (worker thread) — never per frame. It issues: 1 bucket-sums query (30-day window, ≤ ~4.3k aggregated rows), 1 today sum, 8 shape sums (yesterday + 7 climate days), 2 MAX probes, 1 EXISTS probe — all index hits on `idx_usage_events_bucket_at`.
- **Shape-less detection** must check RAW components before any weighting: legacy rows can have all five component columns zero while `effective_tokens > 0`; deriving the cache-effective contribution first would misread those as pure cache. (`classify_work_weather` returns `Clear` for empty shapes — it must never be used to encode absence.)
- **Effective-weighted cache share without config access**: the rows already store `effective_tokens` computed with the configured `cache_read_weight` at write time, so the cache-read *effective* contribution is `max(0, effective − input − output − cache_creation − reasoning)`. No `AppConfig` plumbing into the vm builder, and the weight used is the one actually applied when the tokens were eaten.
- `today_ratio` window end is `now + 1s` (half-open) so the current 10-minute bucket (floored, hence ≤ now) is included.
- `yesterday` is `None` iff the pet did not exist yesterday (`state.created_at >= today_start`); an observed idle yesterday is `Some { ratio: 0.0, dominant_shape: None }` (spec field table).

- [ ] **Step 1: Write the failing tests** (in `day.rs`; the catch-up-wake test in `watch.rs` because it drives the runtime path)

```rust
    // day.rs tests (extend the module from Task 5)
    use crate::storage::day_axis::LocalDayMapper;
    use crate::storage::usage_store::{NormalizedUsageEvent, UsageStore};
    use time::macros::datetime;

    fn utc_mapper() -> LocalDayMapper {
        LocalDayMapper::Fixed(time::UtcOffset::UTC)
    }

    fn store_with_applied(rows: &[(time::OffsetDateTime, f64)]) -> UsageStore {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        for &(at, tokens) in rows {
            store
                .insert_event(&NormalizedUsageEvent {
                    observed_at: at,
                    bucket_at: at,
                    ..NormalizedUsageEvent::for_test_at(at, tokens)
                })
                .unwrap();
        }
        store
    }

    #[test]
    fn newborn_that_never_ate_stays_awake_at_night() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        let now = datetime!(2026-06-09 23:30 UTC); // clock-default Night
        state.created_at = now - time::Duration::minutes(5);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx.day_phase, DayPhase::Night);
        assert!(!ctx.asleep, "a pet that has never eaten must stay awake");
    }

    #[test]
    fn pet_sleeps_after_idle_night_window_and_only_at_night() {
        let now = datetime!(2026-06-09 23:30 UTC);
        let store = store_with_applied(&[(now - time::Duration::hours(3), 5_000.0)]);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!(ctx.asleep, "night + 3h ledger quiet => asleep");
        assert_eq!(
            ctx.sleep_onset_utc,
            Some(now - time::Duration::hours(3) + time::Duration::minutes(SLEEP_IDLE_MINUTES)).map(|t| t.max(ctx.phase_started_at_utc)),
        );
        // Same quiet ledger at midday: never asleep outside Night.
        let midday = datetime!(2026-06-09 13:00 UTC);
        let ctx2 = build_day_context(&store, &state, midday, utc_mapper());
        assert!(!ctx2.asleep);
    }

    #[test]
    fn recent_tokens_keep_the_pet_awake_including_future_dated_rows() {
        let now = datetime!(2026-06-09 23:30 UTC);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let recent = store_with_applied(&[(now - time::Duration::minutes(5), 100.0)]);
        assert!(!build_day_context(&recent, &state, now, utc_mapper()).asleep);
        // Clock-set-backwards: a future bucket counts as recent (fail-awake).
        let future = store_with_applied(&[(now + time::Duration::hours(2), 100.0)]);
        assert!(!build_day_context(&future, &state, now, utc_mapper()).asleep);
    }

    #[test]
    fn today_ratio_divides_by_baseline_and_yesterday_distinguishes_idle_from_no_coverage() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let store = store_with_applied(&[(now - time::Duration::hours(1), 50_000.0)]);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(3);
        // Default test baseline is 100k (CalibrationBaseline::default()).
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!((ctx.today_ratio - 0.5).abs() < 1e-6);
        // Pet existed yesterday but ate nothing: observed idle day.
        assert_eq!(
            ctx.yesterday,
            Some(DaySummary { ratio: 0.0, dominant_shape: None })
        );
        // Pet created today: no coverage.
        state.created_at = now - time::Duration::hours(1);
        let ctx2 = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx2.yesterday, None);
    }

    #[test]
    fn day_shape_classification_is_effective_weighted_and_detects_shapeless_rows() {
        // Raw cache dominates (1M cache reads) but its effective contribution
        // is 30k at the 0.03 write-time weight — output should win the class.
        let sums = crate::storage::usage_store::AppliedShapeSums {
            input_tokens: 10_000.0,
            output_tokens: 60_000.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 1_000_000.0,
            reasoning_output_tokens: 0.0,
            effective_tokens: 100_000.0, // 10k + 60k + 0.03 * 1M = 100k
        };
        assert_eq!(
            classify_day_shape(sums),
            Some(crate::tui::life::WorkWeather::OutputSparks)
        );
        // Shape-less legacy rows: components zero, effective nonzero -> None.
        let shapeless = crate::storage::usage_store::AppliedShapeSums {
            effective_tokens: 42_000.0,
            ..Default::default()
        };
        assert_eq!(classify_day_shape(shapeless), None);
        // Zero day -> None.
        assert_eq!(
            classify_day_shape(crate::storage::usage_store::AppliedShapeSums::default()),
            None
        );
    }

    #[test]
    fn climate_is_modal_over_prior_days_and_mixed_detail_weeks_ignore_shapeless_days() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        // 3 prior days of output-heavy shape; 2 shape-less days; 2 empty days.
        for back in 1..=3_i64 {
            let at = now - time::Duration::days(back);
            let mut e = NormalizedUsageEvent {
                observed_at: at,
                bucket_at: at,
                ..NormalizedUsageEvent::for_test_at(at, 70_000.0)
            };
            e.input_tokens = 10_000.0;
            e.output_tokens = 60_000.0;
            store.insert_event(&e).unwrap();
        }
        for back in 4..=5_i64 {
            let at = now - time::Duration::days(back);
            store
                .insert_event(&NormalizedUsageEvent {
                    observed_at: at,
                    bucket_at: at,
                    ..NormalizedUsageEvent::for_test_at(at, 9_000.0)
                })
                .unwrap(); // for_test_at leaves components zero: shape-less
        }
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx.climate, Some(crate::tui::life::WorkWeather::OutputSparks));
    }

    #[test]
    fn maturity_gate_needs_active_days_and_hour_diversity() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let mut rows = Vec::new();
        // 4 active days at 3 distinct hours: below MIN_ACTIVE_DAYS.
        for back in 1..=4_i64 {
            for hour in [9_i64, 13, 17] {
                rows.push((
                    now - time::Duration::days(back) - time::Duration::hours(hour - 12),
                    10_000.0,
                ));
            }
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        assert!(!build_day_context(&store, &state, now, utc_mapper()).mature);
        // Add a 5th day: exactly MIN_ACTIVE_DAYS with >=3 distinct hours.
        let mut rows5 = rows.clone();
        for hour in [9_i64, 13, 17] {
            rows5.push((
                now - time::Duration::days(5) - time::Duration::hours(hour - 12),
                10_000.0,
            ));
        }
        let store5 = store_with_applied(&rows5);
        assert!(build_day_context(&store5, &state, now, utc_mapper()).mature);
    }

    #[test]
    fn date_seed_rolls_at_dawn_not_midnight() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        let before_dawn = datetime!(2026-06-09 03:00 UTC); // clock defaults: dawn 07
        state.created_at = before_dawn - time::Duration::days(2);
        let after_dawn = datetime!(2026-06-09 08:00 UTC);
        let prev_evening = datetime!(2026-06-08 20:00 UTC);
        let seed_pre = build_day_context(&store, &state, before_dawn, utc_mapper()).date_seed;
        let seed_prev = build_day_context(&store, &state, prev_evening, utc_mapper()).date_seed;
        let seed_post = build_day_context(&store, &state, after_dawn, utc_mapper()).date_seed;
        assert_eq!(seed_pre, seed_prev, "pre-dawn hours belong to the previous day's character");
        assert_ne!(seed_post, seed_pre, "the day's character rolls at dawn");
    }
```

And in `watch.rs` tests — the catch-up wake driven through the REAL staging path (spec: never fixture old `bucket_at`):

```rust
    #[test]
    fn cold_start_catchup_wakes_the_pet_once_through_the_real_smear_path() {
        use crate::game::runtime::{apply_unapplied_usage, stage_usage_poll_deltas};
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage = UsageStore::open(&db_path).unwrap();
        let mut state = PetState::new_for_test("seed", "buddy");
        let now = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::June, 9).unwrap(),
            Time::from_hms(23, 30, 0).unwrap(),
        )
        .assume_utc();
        state.created_at = now - Duration::days(3);
        state.last_usage_poll_at = Some(now - Duration::hours(6)); // long gap => Backfill
        // Give the pet sleep-eligible history: one applied row hours ago.
        usage
            .insert_event(&NormalizedUsageEvent {
                observed_at: now - Duration::hours(5),
                bucket_at: now - Duration::hours(5),
                ..NormalizedUsageEvent::for_test_at(now - Duration::hours(5), 1_000.0)
            })
            .unwrap();
        let mapper = crate::storage::day_axis::LocalDayMapper::Fixed(time::UtcOffset::UTC);
        let pre = crate::tui::day::build_day_context(&usage, &state, now, mapper);
        assert!(pre.asleep, "pet is asleep before the catch-up poll");

        // Drive the REAL smear: a poll result with one fat 6h-old delta.
        let poll = catchup_poll_result_for_test(120_000.0); // see helper below
        stage_usage_poll_deltas(&mut usage, &poll, state.calibration, now).unwrap();
        let update = apply_unapplied_usage(&mut state, &mut usage, now, false).unwrap();
        usage
            .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
            .unwrap();

        let post = crate::tui::day::build_day_context(&usage, &state, now, mapper);
        assert!(
            !post.asleep,
            "the accepted catch-up wake: newly applied tokens wake the pet"
        );
        // ...but the wake is gentle: backfill cannot fire burst animations.
        assert!(!update.applied_signal.can_burst());
        // And it is bounded: SLEEP_IDLE_MINUTES later with no new rows, back asleep.
        let later = now + Duration::minutes(crate::tui::day::SLEEP_IDLE_MINUTES + 11);
        let resettled = crate::tui::day::build_day_context(&usage, &state, later, mapper);
        assert!(resettled.asleep, "one wake, then re-sleep after the idle window");
    }
```

(`catchup_poll_result_for_test` builds a `UsagePollResult` with one `UsageDelta` carrying a `ProviderCursorUpdate` — copy the construction shape from the existing runtime test `delayed_applied_usage_is_backfill_not_live_burst` at `src/game/runtime.rs:577-618`, lifting the delta/cursor literals into a small helper. Note `apply_unapplied_usage` already takes the new `scene_asleep` parameter here — Task 11 adds it; if executing tasks in order, write this test with the 3-arg signature and let Task 11's mechanical update extend it, or implement Task 11's signature change first. The subagent executing this should add the 4th arg when the compiler demands it.)

- [ ] **Step 2: Run, verify compile failures**

Run: `cargo test --lib tui::day 2>&1 | head -20`

- [ ] **Step 3: Implement `DayContext`**

Add to `src/tui/day.rs`:

```rust
use crate::storage::day_axis::LocalDayMapper;
use crate::storage::state::PetState;
use crate::storage::usage_store::{AppliedShapeSums, UsageStore};
use crate::tui::life::{classify_work_weather, TokenShapeDelta, WorkWeather};
use std::collections::{HashMap, HashSet};
use time::{Duration, PrimitiveDateTime, Time};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// One prior local day's observed character. `ratio` is vs the calibration
/// baseline; `dominant_shape` is None when the day had no token-shape detail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DaySummary {
    pub ratio: f32,
    pub dominant_shape: Option<WorkWeather>,
}

/// Wake-resume easing inputs for the wander hold (panel-side, pure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeResume {
    /// Evaluate the frozen wander position at this instant...
    pub from_eval_utc: time::OffsetDateTime,
    /// ...and ease toward the live curve starting here.
    pub woke_at_utc: time::OffsetDateTime,
}

/// Derived time-of-day presentation contract. Built once per poll in
/// build_watch_view_model_at; per-frame logic only compares clock.now_utc()
/// against the precomputed UTC instants carried here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayContext {
    pub day_phase: DayPhase,
    pub phase_started_at_utc: time::OffsetDateTime,
    pub phase_ends_at_utc: time::OffsetDateTime,
    /// hash(local date of the most recent Dawn entry, pet seed) — visual
    /// texture only, never personality content (locked rule).
    pub date_seed: u64,
    pub today_ratio: f32,
    pub yesterday: Option<DaySummary>,
    pub climate: Option<WorkWeather>,
    pub is_weekend: bool,
    pub weekend_share: f32,
    pub season: Season,
    /// Rhythm/baseline personalization gate (spec: Maturity gate).
    pub mature: bool,
    pub asleep: bool,
    pub sleep_onset_utc: Option<time::OffsetDateTime>,
    pub wake_resume: Option<WakeResume>,
    /// Next local-day rollover (T2 motes tidy fade).
    pub local_day_rollover_utc: time::OffsetDateTime,
}

impl Default for DayContext {
    /// Neutral daytime context for fixtures and pre-first-poll frames.
    fn default() -> Self {
        let epoch = time::OffsetDateTime::UNIX_EPOCH;
        Self {
            day_phase: DayPhase::Day,
            phase_started_at_utc: epoch,
            phase_ends_at_utc: epoch,
            date_seed: 0,
            today_ratio: 0.0,
            yesterday: None,
            climate: None,
            is_weekend: false,
            weekend_share: 0.0,
            season: Season::Summer,
            mature: false,
            asleep: false,
            sleep_onset_utc: None,
            wake_resume: None,
            local_day_rollover_utc: epoch,
        }
    }
}
```

The builder and helpers (every read degrades to a quiet default on error — vm-build idiom is `.unwrap_or_default()`-style resilience, a broken read must never crash the watch loop):

```rust
/// Build the DayContext from the applied ledger. Errors in any single read
/// degrade that field to its quiet default — DayContext must never take the
/// watch loop down.
pub fn build_day_context(
    usage_store: &UsageStore,
    state: &PetState,
    now: time::OffsetDateTime,
    mapper: LocalDayMapper,
) -> DayContext {
    let today = mapper.local_date(now);
    let now_local = now.to_offset(mapper.offset_at(now));

    // --- rhythm window: one aggregated query powers histogram + maturity +
    // weekend share ---
    let rhythm_starts = mapper.day_starts_back(now, RHYTHM_WINDOW_DAYS);
    let bucket_sums = usage_store
        .applied_bucket_sums_between(rhythm_starts[0], *rhythm_starts.last().unwrap())
        .unwrap_or_default();
    let mut histogram = [0.0_f64; 24];
    let mut active_days: HashSet<time::Date> = HashSet::new();
    let mut active_hours: HashSet<u8> = HashSet::new();
    let mut weekend_volume = 0.0_f64;
    let mut total_volume = 0.0_f64;
    for &(bucket_at, tokens) in &bucket_sums {
        if tokens <= 0.0 {
            continue;
        }
        let hour = mapper.local_hour(bucket_at);
        let date = mapper.local_date(bucket_at);
        histogram[hour as usize] += tokens;
        active_days.insert(date);
        active_hours.insert(hour);
        total_volume += tokens;
        if matches!(date.weekday(), time::Weekday::Saturday | time::Weekday::Sunday) {
            weekend_volume += tokens;
        }
    }
    let mature =
        active_days.len() >= MIN_ACTIVE_DAYS && active_hours.len() >= MIN_DISTINCT_ACTIVE_HOURS;
    let windows = if mature {
        derive_phase_windows(&histogram)
    } else {
        PhaseWindows::clock_defaults()
    };
    let weekend_share = if total_volume > 0.0 {
        (weekend_volume / total_volume) as f32
    } else {
        0.0
    };

    // --- phase + boundary instants ---
    let hour = now_local.hour();
    let day_phase = windows.phase_for_hour(hour);
    let (start_hour, end_hour) = phase_bounds_for(&windows, day_phase);
    let phase_started_at_utc = instant_of_local_hour_at_or_before(now, start_hour, mapper);
    let phase_ends_at_utc = instant_of_local_hour_after(now, end_hour, mapper);

    // --- today / yesterday / climate ---
    let today_start = mapper.local_day_start(today);
    let tomorrow_start = mapper.local_day_start(today + Duration::days(1));
    let baseline = state.calibration.daily_effective_tokens.max(1.0);
    let today_tokens = usage_store
        .applied_effective_tokens_between(today_start, now + Duration::seconds(1))
        .unwrap_or(0.0);
    let today_ratio = (today_tokens / baseline) as f32;

    let yesterday = if state.created_at >= today_start {
        None
    } else {
        let y_start = mapper.local_day_start(today - Duration::days(1));
        let tokens = usage_store
            .applied_effective_tokens_between(y_start, today_start)
            .unwrap_or(0.0);
        let shape = usage_store
            .applied_token_shape_between(y_start, today_start)
            .unwrap_or_default();
        Some(DaySummary {
            ratio: (tokens / baseline) as f32,
            dominant_shape: classify_day_shape(shape),
        })
    };

    let climate = {
        let starts = mapper.day_starts_back(now, RHYTHM_WINDOW_DAYS.min(8));
        // last pair is today; the 7 pairs before it are the complete days.
        let mut counts: HashMap<WorkWeather, usize> = HashMap::new();
        let pairs = starts.windows(2).collect::<Vec<_>>();
        for pair in pairs.iter().rev().skip(1).take(7) {
            let shape = usage_store
                .applied_token_shape_between(pair[0], pair[1])
                .unwrap_or_default();
            if let Some(class) = classify_day_shape(shape) {
                *counts.entry(class).or_insert(0) += 1;
            }
        }
        modal_climate(&counts)
    };

    // --- sleep predicate (spec: Sleep semantics) ---
    let latest_bucket = usage_store.latest_applied_bucket_at().unwrap_or(None);
    let has_eaten = latest_bucket.is_some();
    let recently_active = latest_bucket
        .map(|b| b >= now - Duration::minutes(SLEEP_IDLE_MINUTES))
        .unwrap_or(false);
    let asleep = day_phase == DayPhase::Night && !recently_active && has_eaten;
    let sleep_onset_utc = if asleep {
        let onset_from_idle =
            latest_bucket.map(|b| b + Duration::minutes(SLEEP_IDLE_MINUTES));
        Some(match onset_from_idle {
            Some(t) => t.max(phase_started_at_utc),
            None => phase_started_at_utc,
        })
    } else {
        None
    };
    let wake_resume = derive_wake_resume(
        usage_store,
        day_phase,
        asleep,
        latest_bucket,
        phase_started_at_utc,
        now,
    );

    DayContext {
        day_phase,
        phase_started_at_utc,
        phase_ends_at_utc,
        date_seed: date_seed_for(now_local, windows.dawn_start, &state.pet.seed),
        today_ratio,
        yesterday,
        climate,
        is_weekend: matches!(
            now_local.date().weekday(),
            time::Weekday::Saturday | time::Weekday::Sunday
        ),
        weekend_share,
        season: season_for_month(now_local.date().month()),
        mature,
        asleep,
        sleep_onset_utc,
        wake_resume,
        local_day_rollover_utc: tomorrow_start,
    }
}

/// The poll-path variant for narration: just the asleep bit, same predicate.
pub fn scene_asleep_for_poll(
    usage_store: &UsageStore,
    state: &PetState,
    now: time::OffsetDateTime,
    mapper: LocalDayMapper,
) -> bool {
    build_day_context(usage_store, state, now, mapper).asleep
}

/// Classify a day's token shape, effective-weighted. The cache-read effective
/// contribution is recovered from stored effective totals (no config access):
/// effective = input + output + cache_creation + weight*cache_read + reasoning.
/// Raw-component all-zero (shape-less legacy rows) and empty days are None —
/// the classifier's Clear must never encode absence.
pub fn classify_day_shape(s: AppliedShapeSums) -> Option<WorkWeather> {
    let raw_components = s.input_tokens
        + s.output_tokens
        + s.cache_creation_tokens
        + s.cache_read_tokens
        + s.reasoning_output_tokens;
    if raw_components <= 0.0 {
        return None;
    }
    let cache_read_effective = (s.effective_tokens
        - s.input_tokens
        - s.output_tokens
        - s.cache_creation_tokens
        - s.reasoning_output_tokens)
        .max(0.0);
    Some(classify_work_weather(Some(TokenShapeDelta {
        input_tokens: s.input_tokens,
        output_tokens: s.output_tokens,
        cache_creation_tokens: s.cache_creation_tokens,
        cache_read_tokens: cache_read_effective,
        reasoning_output_tokens: s.reasoning_output_tokens,
    })))
}

fn modal_climate(counts: &HashMap<WorkWeather, usize>) -> Option<WorkWeather> {
    let max = counts.values().copied().max()?;
    let winners: Vec<WorkWeather> = counts
        .iter()
        .filter(|(_, &c)| c == max)
        .map(|(&w, _)| w)
        .collect();
    match winners.as_slice() {
        [only] => Some(*only),
        _ => Some(WorkWeather::Clear), // tie -> Clear (renders nothing)
    }
}

fn season_for_month(month: time::Month) -> Season {
    use time::Month::*;
    match month {
        December | January | February => Season::Winter,
        March | April | May => Season::Spring,
        June | July | August => Season::Summer,
        September | October | November => Season::Autumn,
    }
}

/// FNV-1a over (dawn-rolled local date, pet seed). Stable across runs and
/// Rust versions (std hashers are not guaranteed stable).
fn date_seed_for(now_local: time::OffsetDateTime, dawn_start: u8, pet_seed: &str) -> u64 {
    let date = if now_local.hour() >= dawn_start {
        now_local.date()
    } else {
        now_local.date() - Duration::days(1)
    };
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in date.to_string().bytes().chain(pet_seed.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}
```

Plus the three time helpers (same file):

```rust
fn phase_bounds_for(windows: &PhaseWindows, phase: DayPhase) -> (u8, u8) {
    match phase {
        DayPhase::Dusk => (windows.dusk_start, windows.night_start),
        DayPhase::Night => (windows.night_start, windows.dawn_start),
        DayPhase::Dawn => (windows.dawn_start, windows.day_start),
        DayPhase::Day => (windows.day_start, windows.dusk_start),
    }
}

/// Most recent UTC instant at which the local clock read `hour`:00.
fn instant_of_local_hour_at_or_before(
    now: time::OffsetDateTime,
    hour: u8,
    mapper: LocalDayMapper,
) -> time::OffsetDateTime {
    let local = now.to_offset(mapper.offset_at(now));
    let date = if local.hour() >= hour {
        local.date()
    } else {
        local.date() - Duration::days(1)
    };
    let offset = mapper.offset_at(mapper.local_day_start(date));
    PrimitiveDateTime::new(date, Time::from_hms(hour, 0, 0).expect("hour < 24"))
        .assume_offset(offset)
}

/// Next UTC instant at which the local clock will read `hour`:00.
fn instant_of_local_hour_after(
    now: time::OffsetDateTime,
    hour: u8,
    mapper: LocalDayMapper,
) -> time::OffsetDateTime {
    let local = now.to_offset(mapper.offset_at(now));
    let date = if local.hour() < hour {
        local.date()
    } else {
        local.date() + Duration::days(1)
    };
    let offset = mapper.offset_at(mapper.local_day_start(date));
    PrimitiveDateTime::new(date, Time::from_hms(hour, 0, 0).expect("hour < 24"))
        .assume_offset(offset)
}

/// Wake-resume: awake at night within the settle window of the apply that
/// woke the pet, with a sleep-qualifying quiet gap before the waking bucket.
fn derive_wake_resume(
    usage_store: &UsageStore,
    day_phase: DayPhase,
    asleep: bool,
    latest_bucket: Option<time::OffsetDateTime>,
    night_started_at: time::OffsetDateTime,
    now: time::OffsetDateTime,
) -> Option<WakeResume> {
    if asleep || day_phase != DayPhase::Night {
        return None;
    }
    let woke_bucket = latest_bucket?;
    let woke_at = usage_store.latest_applied_marked_at().ok().flatten()?;
    if now - woke_at > Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS + 60) {
        return None; // long past the ease window; no need to carry it
    }
    let prev = usage_store
        .latest_applied_bucket_at_before(woke_bucket)
        .ok()
        .flatten();
    let prev_onset = prev.map(|p| p + Duration::minutes(SLEEP_IDLE_MINUTES));
    let was_asleep = match prev_onset {
        Some(onset) => onset < woke_bucket, // a sleep-qualifying gap preceded the wake
        None => true,                       // first meal of the night after night start
    };
    if !was_asleep {
        return None;
    }
    let from_eval = prev_onset
        .map(|o| o.max(night_started_at))
        .unwrap_or(night_started_at);
    Some(WakeResume { from_eval_utc: from_eval, woke_at_utc: woke_at })
}
```

This needs one more tiny store method (add to Task 3's cluster in `usage_store.rs`, with a one-line test):

```rust
    /// When the most recent apply happened (MAX(applied_at)) — the wake
    /// instant for resume easing; bucket_at is 10-minute-floored and too
    /// coarse for an 8-second ease.
    pub fn latest_applied_marked_at(&self) -> crate::error::Result<Option<OffsetDateTime>> {
        let max: Option<String> = self.conn.query_row(
            "SELECT MAX(applied_at) FROM usage_events WHERE applied_at IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        max.map(|s| parse_time_for_sql(&s).map_err(Into::into))
            .transpose()
    }
```

(`WANDER_SETTLE_SECS` is added to `animator.rs` in Task 10; until then declare it where Task 10 says, or implement Task 10's constant first — it is a single `pub const WANDER_SETTLE_SECS: i64 = 8;` line.)

Wire into the vm: in `src/tui/view_model.rs` add to `WatchViewModel` (after `life_profile`):

```rust
    /// Derived time-of-day context. Built once per poll alongside the vm;
    /// per-frame consumers compare clock instants against its UTC boundaries.
    pub day_context: crate::tui::day::DayContext,
```

and add `day_context: crate::tui::day::DayContext::default(),` to `WatchViewModel::fixture()` (Read the fixture constructor in view_model.rs's test/fixture region first). In `build_watch_view_model_at`, build it EARLY (right after `UsageStore::open`, before `render_pet`, because the eyes need `asleep` in Task 8):

```rust
    let day_context = crate::tui::day::build_day_context(&usage_store, state, now, mapper);
```

and stamp `day_context,` into the `WatchViewModel { ... }` literal (anchor: the `life_profile:` line at watch.rs:142).

- [ ] **Step 4: Run, verify pass**

Run: `cargo test --lib tui::day && cargo test --lib watch`
Expected: all pass (the catch-up-wake test may be deferred to after Task 11's signature change — if so, mark it `#[ignore]` with a `// enabled by Task 11` note and remove the ignore there; do NOT delete it).

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u && git add src/tui/day.rs
git commit -m "feat(tui): derive DayContext (rhythm, maturity, sleep, climate) on the vm"
```

---

### Task 7: Night calm — `calm_mode = asleep`, ordered before consumers

**Spec section:** Branch T1, "Night calm" — ordering constraint and first-poll establishment.

**Files:**
- Modify: `src/tui/app.rs` (`install_poll_result`, ~495-516)
- Modify: `src/menubar/app.rs` (`drain_poll_results`, ~373-409)
- Test: in-module `app.rs` tests (the existing `SignalPoller` + `refresh_for_test` harness, see `app.rs:733-744`)

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn asleep_day_context_sets_calm_mode_and_it_survives_the_poll_install() {
        // Build a WatchApp via the SignalPoller test harness (copy the setup
        // from the neighboring burst-gating tests), with a vm whose
        // day_context.asleep is true in the poll result.
        let mut app = test_app_with_signal(AppliedUsageSignal::quiet(
            datetime!(2026-06-09 23:30 UTC),
            Duration::seconds(10),
        ));
        app.test_poll_result_vm().day_context.asleep = true; // fixture seam per harness
        app.refresh_for_test();
        assert!(
            app.vm_for_test().life_profile.calm_mode,
            "asleep must engage calm_mode on the installed profile"
        );
        // First-poll establishment: the very first install must already be calm.
    }
```

(Adapt mechanically to the actual harness shape — Read the existing tests around `app.rs:723+` first; the assertion contract is what matters: after `install_poll_result` with an asleep day_context, `vm.life_profile.calm_mode == true`, including on the first poll of a session.)

- [ ] **Step 2: Run, verify failure** — `calm_mode` stays false (observe hardcodes it).

- [ ] **Step 3: Implement**

In `install_poll_result` (anchor: Read `src/tui/app.rs:495-516`), immediately after `result.vm.life_profile = profile;` and BEFORE the `last_feed_pulse_at` / `current_speech` / `append_profile_pet_activities` lines:

```rust
        // Night calm: full quiet only while the pet actually sleeps (spec:
        // calm_mode = night && asleep; asleep already implies Night). Must be
        // set after observe (which hardcodes calm_mode: false) and before any
        // profile consumer in this install path.
        result.vm.life_profile.calm_mode = result.vm.day_context.asleep;
```

In `menubar/app.rs::drain_poll_results`, after the `APP_STATE.with` closure that assigns `vm.life_profile = profile;` and before `write_full_text(&text_view, &vm)`:

```rust
    vm.life_profile.calm_mode = vm.day_context.asleep;
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test --lib tui::app`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(tui): engage calm_mode from the asleep day context in both frontends"
```

---

### Task 8: Held-closed eyes — `AnimationFrame.hold_eyes_closed`

**Spec section:** "While asleep" (eyes bullet) — the mood-substitution shortcut is FORBIDDEN: asleep must not alter `vm.mood` or the mood passed to `render_pet` (it would fire a spurious MoodFade via the animator's mood-string diff at `src/pet/animator.rs:137-141` and lie on the vitals panel / frame title). Milestone exemption also lands here.

**Files:**
- Modify: `src/pet/render.rs` (`AnimationFrame` ~6-10; `render_pet` ~69-110; `should_blink` call)
- Modify: `src/commands/watch.rs` (AnimationFrame construction ~74-78; `rerender_pet_for_view_model` ~402-417 gains a `hold_eyes_closed: bool` param)
- Modify: `src/tui/app.rs` (`advance_animation_frame` ~245-256 — computes hold with milestone exemption)
- Modify: `src/menubar/app.rs` (`animate_pet` ~432)
- Test: in-module tests in `render.rs` and `app.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    // render.rs tests
    #[test]
    fn hold_eyes_closed_renders_closed_blink_eyes_without_touching_mood() {
        let pet = generate_pet("hold-eyes-seed");
        let frame = AnimationFrame {
            tick: 1, // a tick that does NOT blink on its own
            blink_suppression_ticks: 0,
            hold_eyes_closed: true,
        };
        let rendered = render_pet(&pet, Stage::S3, Mood::Content, frame);
        let art = rendered.lines.join("\n");
        assert!(
            art.contains(closed_blink_eyes(pet.species)),
            "held-closed eyes must use the species closed-blink glyphs, got:\n{art}"
        );
    }

    #[test]
    fn hold_eyes_closed_false_keeps_existing_blink_behavior() {
        let pet = generate_pet("hold-eyes-seed");
        let open = render_pet(
            &pet,
            Stage::S3,
            Mood::Content,
            AnimationFrame { tick: 1, blink_suppression_ticks: 0, hold_eyes_closed: false },
        );
        assert!(
            open.lines.join("\n").contains(&pet.traits.eyes),
            "non-blinking awake frame keeps the trait eyes"
        );
    }
```

```rust
    // app.rs tests
    #[test]
    fn evolution_overlay_renders_the_pet_awake_even_while_asleep() {
        // Milestone exemption: while the evolution overlay window is active,
        // hold_eyes_closed must be false despite day_context.asleep.
        // Use the harness: set vm.day_context.asleep = true, set
        // vm.latest_evolution to a new stage label, tick once, and assert the
        // computed hold flag (expose via a #[cfg(test)] accessor if needed —
        // follow the harness pattern of the evolution-overlay tests nearby).
    }
```

(Write the evolution test against the real harness — Read the existing evolution overlay tests near `app.rs:723+` first and mirror their setup; the contract: overlay active ⇒ eyes open.)

- [ ] **Step 2: Run, verify compile failure** (`hold_eyes_closed` field doesn't exist)

Run: `cargo test --lib render 2>&1 | head -20`

- [ ] **Step 3: Implement**

`render.rs`: add the field and force the blink branch:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
    /// Sleep presentation: force the species closed-blink eyes. Must never be
    /// implemented by substituting Mood::Sleepy — mood is the vitals contract.
    pub hold_eyes_closed: bool,
}
```

In `render_pet` (Read lines 69-110 first):

```rust
    let blinking = frame.hold_eyes_closed || should_blink(pet, mood, frame, profile);
```

The compiler now flags every `AnimationFrame { ... }` literal. Update each:
- `src/commands/watch.rs:74-78` (vm build): `hold_eyes_closed: day_context.asleep,` (the DayContext from Task 6 is built before this render — verify the ordering stands).
- `src/commands/watch.rs` `rerender_pet_for_view_model`: change signature to `pub fn rerender_pet_for_view_model(vm: &mut WatchViewModel, tick: u64, hold_eyes_closed: bool) -> Result<()>` and pass the flag through.
- `src/tui/app.rs::advance_animation_frame`: compute the hold with the milestone exemption and pass it:

```rust
        let hold_eyes_closed = self.vm.day_context.asleep
            && self.evolution_overlay_started_at.is_none()
            && self.overlay.is_none();
        let _ = crate::commands::watch::rerender_pet_for_view_model(
            &mut self.vm,
            self.animation_frame,
            hold_eyes_closed,
        );
```

- `src/menubar/app.rs::animate_pet` (~432): `rerender_pet_for_view_model(&mut vm, next_frame, vm.day_context.asleep)` (no overlays in the menubar).
- Every `AnimationFrame` literal in tests: add `hold_eyes_closed: false`.

- [ ] **Step 4: Run the full suite** (signature ripple)

Run: `cargo test`
Expected: all pass. Also confirm no MoodFade regression: the animator tests must still pass untouched — if any animator test changed behavior, you took the forbidden mood shortcut somewhere.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(pet): hold eyes closed while asleep without touching the mood contract"
```

---

### Task 9: Sleep gates the interactive channels — cursor eyes and petting

**Spec section:** "While asleep" (cursor + petting bullets).

**Files:**
- Modify: `src/tui/panels/pet.rs` (`cursor_normalized_x_within` ~794-808)
- Modify: `src/pet/speech.rs` (sleep petting pool)
- Modify: `src/tui/app.rs` (`pet_the_pet` ~329-337)
- Test: in-module tests in `pet.rs` (panel fns), `speech.rs`, `app.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    // panels/pet.rs tests
    #[test]
    fn cursor_eyes_are_disabled_while_asleep() {
        let mut vm = WatchViewModel::fixture();
        vm.mouse_tracking_enabled = true;
        vm.cursor_screen = Some((5, 5));
        vm.day_context.asleep = true;
        let area = Rect::new(0, 0, 20, 10);
        assert_eq!(
            cursor_normalized_x_within(&vm, area),
            None,
            "closed eyes must not pop open to follow the mouse"
        );
    }
```

```rust
    // speech.rs tests
    #[test]
    fn sleep_petting_phrases_come_from_the_sleep_pool() {
        let now = datetime!(2026-06-09 23:30 UTC);
        let phrase = pick_sleep_petting_phrase(now);
        assert!(
            SLEEP_PETTING_PHRASES.contains(&phrase.as_str()),
            "got {phrase}"
        );
    }
```

```rust
    // app.rs tests
    #[test]
    fn petting_a_sleeping_pet_uses_the_sleep_pool_and_does_not_wake_it() {
        // Harness: vm.day_context.asleep = true; press 'p' (call pet_the_pet);
        // assert vm.current_speech is one of the sleep phrases and
        // vm.day_context.asleep is still true (petting is not work — the
        // predicate is ledger-derived and pet_the_pet must not touch it).
    }
```

- [ ] **Step 2: Run, verify failures**

Run: `cargo test --lib cursor_eyes_are_disabled 2>&1 | head -10`

- [ ] **Step 3: Implement**

`cursor_normalized_x_within` (Read `pet.rs:794-808`): add as the first guard:

```rust
    if vm.day_context.asleep {
        return None;
    }
```

`speech.rs`: add next to `pick_petting_phrase`:

```rust
/// Reaction pool when the user pets a SLEEPING pet: it stirs but stays
/// asleep — petting is affection, not food, so it never wakes the pet.
const SLEEP_PETTING_PHRASES: &[&str] = &["*snore*", "*stirs*", "...zzz", "*curls up tighter*"];

pub fn pick_sleep_petting_phrase(now: OffsetDateTime) -> String {
    let idx = (now.unix_timestamp()).rem_euclid(SLEEP_PETTING_PHRASES.len() as i64) as usize;
    SLEEP_PETTING_PHRASES[idx].to_string()
}
```

`app.rs::pet_the_pet` (Read 329-337): branch the phrase pick:

```rust
        let phrase = if self.vm.day_context.asleep {
            crate::pet::speech::pick_sleep_petting_phrase(now_wall)
        } else {
            crate::pet::speech::pick_petting_phrase(now_wall)
        };
```

(The transient happiness/energy nudge stays for both — affection still lands.) Note the eyes stay closed for free: `pet_the_pet` never touches `day_context.asleep`, and Task 8's hold flag derives from it.

- [ ] **Step 4: Run, verify pass**

Run: `cargo test --lib speech && cargo test --lib panels && cargo test --lib tui::app`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(tui): gate cursor eyes and petting reactions on the sleeping pet"
```

---

### Task 10: Sleep motion — slowed breath (onset-anchored) and held wander/facing

**Spec section:** "While asleep" (breath + wander bullets). The vestigial `AnimationProfile.breath_period/breath_hold` fields in `render.rs` are confirmed dead — do NOT scale those; the real breathing knob is `animator.rs::compute_breath_offset`.

**Files:**
- Modify: `src/pet/animator.rs` (breath rhythm param; wander/facing hold + ease; `WANDER_SETTLE_SECS`)
- Modify: `src/tui/app.rs` (`advance_animation_frame` breath call)
- Modify: `src/commands/watch.rs` (vm-build breath call)
- Modify: `src/tui/panels/pet.rs` (render-time wander/facing consumption ~465-484)
- Test: in-module tests in `animator.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    // animator.rs tests
    #[test]
    fn sleep_breath_is_slower_and_continuous_at_onset() {
        let onset = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        // At the onset instant, the asleep phase starts at zero — identical
        // breath state to a fresh awake cycle start (no visible pop).
        let at_onset = compute_breath_offset_with_rhythm(
            Some(Species::Fuzz),
            onset,
            BreathRhythm::Asleep { onset },
        );
        assert_eq!(at_onset, 1, "phase 0 sits inside the inhale window");
        // The asleep cycle is SLEEP_BREATH_PERIOD_SCALE x longer: for fuzz
        // (period 40ds, inhale 8ds) the asleep inhale window is 16ds of a
        // 120ds cycle — at +5s (50ds) the pet must be at rest, and the next
        // inhale starts at +12s.
        let mid = compute_breath_offset_with_rhythm(
            Some(Species::Fuzz),
            onset + time::Duration::seconds(5),
            BreathRhythm::Asleep { onset },
        );
        assert_eq!(mid, 0);
        let next_cycle = compute_breath_offset_with_rhythm(
            Some(Species::Fuzz),
            onset + time::Duration::seconds(12),
            BreathRhythm::Asleep { onset },
        );
        assert_eq!(next_cycle, 1);
    }

    #[test]
    fn awake_rhythm_matches_the_existing_breath_function() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_123).unwrap();
        for species in [Species::Fuzz, Species::Glitch, Species::Crystal] {
            assert_eq!(
                compute_breath_offset_with_rhythm(Some(species), now, BreathRhythm::Awake),
                compute_breath_offset(Some(species), now),
            );
        }
    }

    #[test]
    fn wander_holds_at_sleep_onset_and_eases_back_on_wake() {
        let onset = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let width = 52_u16;
        let held = compute_wander_position_x(width, Species::Fuzz, onset);
        // Deep into sleep, position is fully held at the onset evaluation.
        let deep = onset + time::Duration::minutes(40);
        assert_eq!(
            compute_sleep_wander_x(width, Species::Fuzz, deep, onset),
            held,
            "asleep wander must hold the onset position"
        );
        // At the onset instant itself the blend starts from the live curve,
        // which equals the held value — continuous by construction.
        assert_eq!(
            compute_sleep_wander_x(width, Species::Fuzz, onset, onset),
            compute_wander_position_x(width, Species::Fuzz, onset),
        );
        // Wake resume: at the wake instant the position is the frozen eval;
        // after WANDER_SETTLE_SECS it equals the live curve.
        let woke_at = deep;
        let from_eval = onset;
        assert_eq!(
            compute_wake_wander_x(width, Species::Fuzz, woke_at, from_eval, woke_at),
            compute_wander_position_x(width, Species::Fuzz, from_eval),
        );
        let settled = woke_at + time::Duration::seconds(WANDER_SETTLE_SECS);
        assert_eq!(
            compute_wake_wander_x(width, Species::Fuzz, settled, from_eval, woke_at),
            compute_wander_position_x(width, Species::Fuzz, settled),
        );
    }
```

- [ ] **Step 2: Run, verify compile failures**

Run: `cargo test --lib animator 2>&1 | head -20`

- [ ] **Step 3: Implement** (in `animator.rs`)

```rust
/// Sleep onset/wake position easing window, seconds. Also consumed by
/// tui::day's wake-resume derivation.
pub const WANDER_SETTLE_SECS: i64 = 8;
/// Asleep breath: period x3, inhale window x2 — slow and deep.
const SLEEP_BREATH_PERIOD_SCALE: i64 = 3;
const SLEEP_BREATH_INHALE_SCALE: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreathRhythm {
    Awake,
    /// Slowed cycle whose phase is anchored at the sleep-onset instant so the
    /// period change is continuous, not a pop.
    Asleep { onset: time::OffsetDateTime },
}

pub fn compute_breath_offset_with_rhythm(
    species: Option<Species>,
    now: time::OffsetDateTime,
    rhythm: BreathRhythm,
) -> u8 {
    let (period_ds, inhale_ds) = species_breath_rhythm_decis(species);
    let (period_ds, inhale_ds, anchor_ds) = match rhythm {
        BreathRhythm::Awake => (period_ds, inhale_ds, 0),
        BreathRhythm::Asleep { onset } => (
            period_ds * SLEEP_BREATH_PERIOD_SCALE,
            inhale_ds * SLEEP_BREATH_INHALE_SCALE,
            onset.unix_timestamp() * 10 + i64::from(onset.millisecond() / 100),
        ),
    };
    let ts_ds = now.unix_timestamp() * 10 + i64::from(now.millisecond() / 100);
    let phase = (ts_ds - anchor_ds).rem_euclid(period_ds);
    if phase < inhale_ds {
        1
    } else {
        0
    }
}
```

Keep `compute_breath_offset` as a thin wrapper (`compute_breath_offset_with_rhythm(species, now, BreathRhythm::Awake)`) so existing call sites and tests stand. Wander hold/ease (pure helpers next to `compute_wander_position_x`):

```rust
/// Wander position while asleep: blends from the live curve into the held
/// onset evaluation over WANDER_SETTLE_SECS. At the onset instant the two
/// endpoints coincide (now == onset), so sleep onset is continuous.
pub fn compute_sleep_wander_x(
    habitat_width: u16,
    species: Species,
    now: time::OffsetDateTime,
    onset: time::OffsetDateTime,
) -> i16 {
    let held = compute_wander_position_x(habitat_width, species, onset);
    let live = compute_wander_position_x(habitat_width, species, now);
    let k = ease_fraction(now, onset);
    blend_positions(live, held, k)
}

/// Wander position right after a wake: eases from the frozen sleep position
/// back onto the live curve over WANDER_SETTLE_SECS.
pub fn compute_wake_wander_x(
    habitat_width: u16,
    species: Species,
    now: time::OffsetDateTime,
    from_eval: time::OffsetDateTime,
    woke_at: time::OffsetDateTime,
) -> i16 {
    let frozen = compute_wander_position_x(habitat_width, species, from_eval);
    let live = compute_wander_position_x(habitat_width, species, now);
    let k = ease_fraction(now, woke_at);
    blend_positions(frozen, live, k)
}

fn ease_fraction(now: time::OffsetDateTime, since: time::OffsetDateTime) -> f32 {
    let elapsed = (now - since).whole_milliseconds() as f32 / 1_000.0;
    (elapsed / WANDER_SETTLE_SECS as f32).clamp(0.0, 1.0)
}

fn blend_positions(from: i16, to: i16, k: f32) -> i16 {
    (f32::from(from) * (1.0 - k) + f32::from(to) * k).round() as i16
}
```

Consumption — `src/tui/panels/pet.rs` render head (Read 465-484; replace the wander/facing computation):

```rust
        let day = &vm.day_context;
        let (wander_x, facing) = match (day.asleep, day.sleep_onset_utc, day.wake_resume) {
            (true, Some(onset), _) => (
                compute_sleep_wander_x(area.width, species, now, onset),
                compute_facing(area.width, species, onset), // held facing: no mirror flips with shut eyes
            ),
            (false, _, Some(resume)) => (
                compute_wake_wander_x(
                    area.width,
                    species,
                    now,
                    resume.from_eval_utc,
                    resume.woke_at_utc,
                ),
                compute_facing(area.width, species, now),
            ),
            _ => (
                compute_wander_position_x(area.width, species, now),
                compute_facing(area.width, species, now),
            ),
        };
```

Breath call sites: `app.rs::advance_animation_frame` and the vm-build stamp in `watch.rs` both become:

```rust
        let rhythm = match (vm.day_context.asleep, vm.day_context.sleep_onset_utc) {
            (true, Some(onset)) => crate::pet::animator::BreathRhythm::Asleep { onset },
            _ => crate::pet::animator::BreathRhythm::Awake,
        };
        // then: compute_breath_offset_with_rhythm(Some(species), now, rhythm)
```

(adjust `vm` vs `self.vm` per site). Known determinism seam, unchanged from today: `advance_animation_frame` uses `OffsetDateTime::now_utc()` — Preview Lab proves sleep breath through the vm-build path and fixtures, which pin the clock.

- [ ] **Step 4: Run, verify pass**

Run: `cargo test --lib animator && cargo test --lib panels`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(pet): sleep breath rhythm and held wander with settle easing"
```

---

### Task 11: Sleep voice — zzz cadence, speech precedence, narration partition

**Spec sections:** "While asleep" (speech + feed-surface bullets) and T2's "Speech precedence stack" rungs 1-2-3 (the T1 subset: petting > zzz > munch/mood; dream windows are T2 and NOT built here — the zzz branch is written so T2 can splice dreams in).

**Files:**
- Modify: `src/pet/speech.rs` (asleep branch in both selectors)
- Modify: `src/pet/narration.rs` (`idle_phrase` partition)
- Modify: `src/game/runtime.rs` (`apply_unapplied_usage` gains `scene_asleep: bool`; idle-narration call passes it)
- Modify: callers of `apply_unapplied_usage`: `src/commands/status.rs`, `src/commands/watch.rs` (`poll_usage_and_apply`), plus the `#[doc(hidden)] apply_usage_poll` test wrapper and every runtime test (mechanical: pass `false` unless the test is about sleep)
- Test: in-module tests in `speech.rs`, `narration.rs`, `runtime.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    // speech.rs tests
    #[test]
    fn asleep_speech_is_a_sparse_zzz_cadence_and_suppresses_munch_and_mood_lines() {
        // Visible slot of every SLEEP_SPEECH_CYCLE_N-th 30s cycle only.
        let cycle0 = OffsetDateTime::from_unix_timestamp(90 * ((1_700_000_000) / 90)).unwrap();
        let hot_profile = crate::tui::life::PetLifeProfile {
            burst_level: 1.0, // would be a munch line awake
            ..Default::default()
        };
        let line = current_pet_speech_for_scene(Mood::Hungry, &hot_profile, true, cycle0);
        assert!(
            matches!(line.as_deref(), Some(l) if SLEEP_SPEECH_PHRASES.contains(&l)),
            "asleep at an eligible cycle: zzz, never munch or 'feed me?' — got {line:?}"
        );
        // The next cycle (not a multiple of SLEEP_SPEECH_CYCLE_N) is silent.
        let cycle1 = cycle0 + time::Duration::seconds(30);
        assert_eq!(current_pet_speech_for_scene(Mood::Hungry, &hot_profile, true, cycle1), None);
        // Awake delegates to the existing profile selector.
        let awake = current_pet_speech_for_scene(Mood::Hungry, &hot_profile, false, cycle0);
        assert_eq!(awake, current_pet_speech_for_profile(Mood::Hungry, &hot_profile, cycle0));
    }
```

```rust
    // narration.rs tests
    #[test]
    fn sleep_claiming_idle_lines_are_only_eligible_while_asleep() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        for offset in 0..4 {
            let at = now + time::Duration::seconds(offset * 30);
            let awake = idle_phrase("buddy", at, false);
            assert!(
                !awake.contains("drifted off") && !awake.contains("dreams"),
                "awake idle must not claim sleep: {awake}"
            );
            let asleep = idle_phrase("buddy", at, true);
            assert!(
                asleep.contains("drifted off") || asleep.contains("dreams"),
                "asleep idle uses the sleep vocabulary: {asleep}"
            );
        }
    }
```

- [ ] **Step 2: Run, verify compile failures**

Run: `cargo test --lib speech 2>&1 | head -10`

- [ ] **Step 3: Implement**

`speech.rs`:

```rust
/// Show the sleep bubble only on every Nth 30s speech cycle — night is calm.
const SLEEP_SPEECH_CYCLE_N: i64 = 3;
const SLEEP_SPEECH_PHRASES: &[&str] = &["zzz...", "...zzz", "z z z"];

/// Scene-aware speech selector: the T1 precedence subset. While asleep the
/// only voice is the sparse zzz cadence (T2 splices dream windows into this
/// branch); munch phrases and mood lines are suppressed — the petting
/// override sits above this at the app layer.
pub fn current_pet_speech_for_scene(
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    asleep: bool,
    now: OffsetDateTime,
) -> Option<String> {
    if !asleep {
        return current_pet_speech_for_profile(mood, profile, now);
    }
    let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
    let cycle_index = now.unix_timestamp().div_euclid(SPEECH_CYCLE_SECS);
    if cycle_pos >= SPEECH_VISIBLE_SECS || cycle_index.rem_euclid(SLEEP_SPEECH_CYCLE_N) != 0 {
        return None;
    }
    let idx = cycle_index.div_euclid(SLEEP_SPEECH_CYCLE_N).rem_euclid(SLEEP_SPEECH_PHRASES.len() as i64) as usize;
    Some(SLEEP_SPEECH_PHRASES[idx].to_string())
}
```

Call sites: in `install_poll_result` (app.rs, after the Task 7 calm line) replace the `current_pet_speech_for_profile` call with `current_pet_speech_for_scene(result.vm.pet_render.mood, &result.vm.life_profile, result.vm.day_context.asleep, now)`. In `build_watch_view_model_at` replace the build-time `current_pet_speech(...)` stamp with an asleep guard so the post-poll frame can't flash an awake line:

```rust
        current_speech: if day_context.asleep {
            crate::pet::speech::current_pet_speech_for_scene(
                mood,
                &crate::tui::life::PetLifeProfile::default(),
                true,
                now,
            )
        } else {
            crate::pet::speech::current_pet_speech(
                mood,
                recent_activity_tokens(&recent_usage, now),
                now,
            )
        },
```

`narration.rs` (Read 151-165 first; replace `idle_phrase`):

```rust
/// Produce an idle drift narration line. Sleep-claiming variants are only
/// eligible while the scene is actually asleep — the feed must never assert
/// a sleep the pet panel contradicts (and vice versa).
pub fn idle_phrase(name: &str, now: OffsetDateTime, asleep: bool) -> String {
    let variants: &[&str] = if asleep {
        &["{name} drifted off", "{name} dreams"]
    } else {
        &["{name} is quiet", "{name} settled in"]
    };
    let idx = pick_idx(now, variants.len());
    variants[idx].replace("{name}", name)
}
```

`runtime.rs`: `apply_unapplied_usage(state, usage_store, now)` → `apply_unapplied_usage(state, usage_store, now, scene_asleep: bool)`; the idle-narration branch (Read 146-164) passes it to `idle_phrase(&state.pet.accepted_name.clone(), now, scene_asleep)`. Callers:
- `src/commands/watch.rs::poll_usage_and_apply` and `src/commands/status.rs`: compute before applying —

```rust
        let scene_asleep = crate::tui::day::scene_asleep_for_poll(
            &usage_store,
            &state,
            now,
            crate::storage::day_axis::LocalDayMapper::System,
        );
```

then pass `scene_asleep`. (Computed BEFORE apply: the narration only fires on the empty-rows idle branch, where pre- and post-apply ledgers are identical.)
- The `#[doc(hidden)]` `apply_usage_poll` wrapper and all runtime/watch tests: pass `false` (mechanical; compiler enumerates). The Task 6 catch-up test passes `false` too (it asserts the predicate through `build_day_context`, not narration) — un-`#[ignore]` it now if it was deferred.

- [ ] **Step 4: Run the full suite** (signature ripple)

Run: `cargo test`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(pet): zzz speech cadence and asleep-partitioned idle narration"
```

---

### Task 12: Day/night habitat — per-phase sky families, warmth, and blend

**Spec sections:** Branch T1 "Habitat day/night cycle" + "Boundary behavior". Hard rules: sky glyphs RE-SKIN the existing ambient allocation (same count budget scaled DOWN at night, never up); Flat keeps ZERO ambient glyphs (the existing early-return stands untouched — day/night reaches Flat users only through pet timing cues); phase palettes interpolate over `PHASE_BLEND_MINUTES` after a boundary; the pet always wins (exclusions unchanged).

**Files:**
- Modify: `src/tui/panels/pet.rs` (`ambient_glyphs_for` ~272-347 gains day-phase params; new per-phase palettes + warmth; call site ~507-513)
- Test: in-module tests in `pet.rs`

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn night_sky_uses_the_night_family_and_a_smaller_budget() {
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let day = ambient_glyphs_for_phase(
            Species::Crystal, Stage::S6, habitat, &[], now,
            crate::tui::style::ColorCapability::Truecolor,
            DayPhase::Day, 1.0,
        );
        let night = ambient_glyphs_for_phase(
            Species::Crystal, Stage::S6, habitat, &[], now,
            crate::tui::style::ColorCapability::Truecolor,
            DayPhase::Night, 1.0,
        );
        // Night never adds: sky glyph count (excluding the floor row) must be
        // <= day's. Floor-row glyphs share a row coordinate — partition on it.
        let floor_row = habitat.y + habitat.height - 1;
        let day_sky = day.iter().filter(|g| g.row != floor_row).count();
        let night_sky = night.iter().filter(|g| g.row != floor_row).count();
        assert!(night_sky <= day_sky, "night {night_sky} > day {day_sky}");
        assert!(night_sky > 0, "the starfield exists");
        // And the night family differs from the day family for this species.
        let night_chars: std::collections::HashSet<char> =
            night.iter().filter(|g| g.row != floor_row).map(|g| g.glyph).collect();
        assert!(
            night_chars.iter().any(|c| !sky_palette_for(Species::Crystal).contains(c))
                || night.iter().filter(|g| g.row != floor_row).count() < day_sky,
            "night must read differently than day"
        );
    }

    #[test]
    fn flat_tier_still_renders_zero_ambient_glyphs_at_night() {
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let glyphs = ambient_glyphs_for_phase(
            Species::Crystal, Stage::S6, habitat, &[], now,
            crate::tui::style::ColorCapability::Flat,
            DayPhase::Night, 1.0,
        );
        assert!(glyphs.is_empty(), "Flat keeps the existing zero-ambient contract");
    }

    #[test]
    fn phase_blend_interpolates_the_sky_color() {
        // blend=0.0 (just crossed into dusk) renders the previous phase's
        // warmth; blend=1.0 the full dusk warmth; halfway sits between.
        let c0 = sky_color_for_phase(DayPhase::Dusk, 0.0);
        let c1 = sky_color_for_phase(DayPhase::Dusk, 1.0);
        let mid = sky_color_for_phase(DayPhase::Dusk, 0.5);
        assert_ne!(c0, c1);
        assert_ne!(mid, c0);
        assert_ne!(mid, c1);
    }
```

- [ ] **Step 2: Run, verify compile failures**

Run: `cargo test --lib night_sky 2>&1 | head -10`

- [ ] **Step 3: Implement**

Rename/extend `ambient_glyphs_for` → keep the existing function as a thin wrapper (existing tests stand) and add:

```rust
/// Per-phase sky glyph family. Re-skins the same allocation — night gets a
/// sparse starfield, dawn/dusk warm grain, day keeps the species default.
fn sky_palette_for_phase(species: Species, phase: DayPhase) -> &'static [char] {
    match phase {
        DayPhase::Day => sky_palette_for(species),
        DayPhase::Dawn | DayPhase::Dusk => match species {
            Species::Glitch => &['░', '▪', '·', ' '],
            _ => &['·', '\'', '~', ' '],
        },
        DayPhase::Night => match species {
            Species::Glitch => &['▪', '·', ' ', ' '],
            _ => &['✦', '·', '*', ' '],
        },
    }
}

/// Sky glyph budget scale per phase. Night <= day, always.
fn phase_count_scale(phase: DayPhase) -> f64 {
    match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.7,
        DayPhase::Dusk => 0.8,
        DayPhase::Night => 0.6,
    }
}

/// Sky color with phase warmth, interpolated by `blend` (0.0 at the phase
/// boundary -> 1.0 after PHASE_BLEND_MINUTES) so crossings are gradual.
fn sky_color_for_phase(phase: DayPhase, blend: f32) -> ratatui::style::Color {
    let p = crate::tui::style::tokenpet_palette();
    let base = p.dim.rgb;
    let target = match phase {
        DayPhase::Day => base,
        DayPhase::Dawn => warm_shift(base, 0.25),
        DayPhase::Dusk => warm_shift(base, 0.40),
        DayPhase::Night => dim_shift(base, 0.40),
    };
    lerp_color(base, target, blend.clamp(0.0, 1.0))
}
```

(`warm_shift` / `dim_shift` / `lerp_color` are small RGB helpers — write them next to the existing color utilities in this file, matching however `Rgb`/`Color` is represented there; Read the `sky_color`/palette usage at ~300-310 first and keep the same color type.) The new entry point:

```rust
pub fn ambient_glyphs_for_phase(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: crate::tui::style::ColorCapability,
    phase: DayPhase,
    phase_blend: f32,
) -> Vec<AmbientGlyph> { ... }
```

Implementation = the existing `ambient_glyphs_for` body with three substitutions: `let sky = sky_palette_for_phase(species, phase);`, `let count = ((stage_base_count(stage) + area_term) as f64 * phase_count_scale(phase)).round() as usize;`, and `let sky_color = sky_color_for_phase(phase, phase_blend);` (floor color: apply `dim_shift` at night only, same blend). The Flat early-return is FIRST and unchanged. Keep `ambient_glyphs_for(...)` delegating with `(DayPhase::Day, 1.0)`.

Call site (`PetPanel::render`, the Pass-1 block): compute the blend from vm-carried instants and call the phase variant:

```rust
        let day = &vm.day_context;
        let phase_blend = {
            let since = (now - day.phase_started_at_utc).whole_seconds() as f32;
            (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
        };
        let glyphs = ambient_glyphs_for_phase(
            species, stage, scene.habitat, &ambient_exclusions, now,
            ctx.color_capability, day.day_phase, phase_blend,
        );
```

- [ ] **Step 4: Run, verify pass + eyeball it**

Run: `cargo test --lib panels`
Then: `cargo run --features dev-preview -- dev-preview --scenario watch --out target/glorp-preview 2>/dev/null || cargo run -- dev-preview --scenario watch --out target/glorp-preview` and open `target/glorp-preview/index.html` — existing fixtures are daytime; the night look gets its fixture in Task 14. (Check how the dev-preview feature gate is invoked — CLAUDE.md uses plain `cargo run -- dev-preview`; use that.)

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(tui): day-phase sky families with blended warmth in the habitat"
```

---

### Task 13: Menubar sleep — dimmed palette + BMP invariant

**Spec section:** Branch T1 "Sleep" (menubar scope sentence): the popover shows sleep **eyes** (already flowing via Task 8's `animate_pet` change) and a **dimmed palette**; breath/wander/zzz are watch-TUI-only — the popover has no positioning or speech surface. Do not build more than this.

**Files:**
- Modify: `src/menubar/render.rs` (`role_color_for_profile` ~77-89)
- Test: in-module tests in `render.rs` (next to `menubar_profile_accent_is_poll_bound_and_bmp_safe`)

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn sleeping_pet_dims_the_menubar_palette_and_keeps_the_bmp_invariant() {
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = vec!["EAPB".to_string()];
        vm.pet_spans = vec![
            StyledSegment { line: 0, start: 0, end: 1, role: PaletteRoleName::Eye },
            StyledSegment { line: 0, start: 1, end: 2, role: PaletteRoleName::Accent },
            StyledSegment { line: 0, start: 2, end: 3, role: PaletteRoleName::Particle },
            StyledSegment { line: 0, start: 3, end: 4, role: PaletteRoleName::Body },
        ];
        vm.day_context.asleep = true;

        let block = render_pet_block(&vm);
        assert_eq!(
            block.char_len,
            block.attr.length(),
            "sleep dimming must not disturb the menubar BMP length invariant"
        );

        let dimmed = rgb_tuple(role_color_for_profile(PaletteRoleName::Body, &vm));
        let base = rgb_tuple(role_color(PaletteRoleName::Body));
        assert!(
            dimmed.0 < base.0 && dimmed.1 < base.1 && dimmed.2 < base.2,
            "asleep must dim every role: {dimmed:?} vs {base:?}"
        );
    }
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test --lib menubar 2>&1 | head -10`

- [ ] **Step 3: Implement**

In `role_color_for_profile` (Read `src/menubar/render.rs:77-89`), apply the dim as a final wrap over whatever color the existing accent logic picked:

```rust
/// Sleep dim factor for the popover pet (the menubar's only palette channel).
const SLEEP_DIM: f32 = 0.7;

fn role_color_for_profile(role: PaletteRoleName, vm: &WatchViewModel) -> Rgb {
    let base = role_color(role);
    let colored = if !matches!(role, PaletteRoleName::Accent | PaletteRoleName::Particle) {
        base
    } else {
        match vm.life_profile.source_accent {
            Some(SourceAccent::Codex) => Rgb(0x86, 0xd9, 0xef),
            Some(SourceAccent::Claude) => Rgb(0xb3, 0x9d, 0xff),
            Some(SourceAccent::Balanced) => Rgb(0xf0, 0xc4, 0x6a),
            None => base,
        }
    };
    if vm.day_context.asleep {
        Rgb(
            (f32::from(colored.0) * SLEEP_DIM) as u8,
            (f32::from(colored.1) * SLEEP_DIM) as u8,
            (f32::from(colored.2) * SLEEP_DIM) as u8,
        )
    } else {
        colored
    }
}
```

(Match the actual `Rgb` tuple-struct shape from the file — Read it first.)

- [ ] **Step 4: Run, verify pass** (macOS-only module — `cargo test` runs it on this machine)

Run: `cargo test --lib menubar`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(menubar): dim the popover palette while the pet sleeps"
```

---

### Task 14: Preview Lab fixtures — night-asleep, dawn-crossing, wake, hatch-at-night

**Spec section:** "Testing and proof" (Preview Lab list — T1 subset) + the manifest-inputs contract ("review is a contract, not a vibe").

**Files:**
- Modify: `src/dev_preview/watch.rs` (fixture registrations + a `DayContextFixture` override seam + builders)
- Modify: `src/dev_preview/scenarios.rs` (metadata arms for the new id prefix; manifest `day_context` inputs)
- Modify: BOTH ordered pins: `scenarios.rs::all_selection_writes_watch_and_pet_scenarios` (id vec) and `watch.rs::watch_frames_include_wide_tall_wide_and_compact` (`frames.len()` + per-index ids)
- Modify: `tests/dev_preview.rs` (night-asleep whole-frame snapshot)

T1 fixture set (ids use a new `watch-daycontext-` prefix so the `watch-liveliness-` metadata prefix-match doesn't absorb them):

| id | what it proves | day_context override |
|---|---|---|
| `watch-daycontext-night-asleep` | starfield, dimmed sky, held wander, closed eyes, calm, zzz | `asleep: true`, `day_phase: Night`, `sleep_onset_utc: Some(fixed_now - 25min)`, blend 1.0 |
| `watch-daycontext-dawn-crossing` | dawn family mid-blend | `day_phase: Dawn`, `phase_started_at_utc: fixed_now - 10min` (blend ≈ 0.33) |
| `watch-daycontext-night-wake-catchup` | awake at night, no burst FX, wake-resume wander | `day_phase: Night`, `asleep: false`, `wake_resume: Some(...)`, life profile freshness pinned to backfill (no pop) |
| `watch-daycontext-hatch-at-night` | newborn awake at night, eyes open | age-0 state, `day_phase: Night`, `asleep: false` |

- [ ] **Step 1: Add the failing ordered-pin expectations**

Extend the id vec in `all_selection_writes_watch_and_pet_scenarios` (insert the four ids after `"watch-liveliness-calm-mode-s6-hot"`) and the `watch_frames_include_wide_tall_wide_and_compact` assertions (`frames.len()` 10 → 14, four new indexed id/dimension asserts at 120x32). Run `cargo test --lib dev_preview 2>&1 | head -20` — both pins FAIL (fixtures don't exist).

- [ ] **Step 2: Implement the fixtures**

Follow the exact existing pattern (Read `src/dev_preview/watch.rs:173-200` and `:375-392`): add a `day_context: Option<DayContext>` field to the liveliness/watch fixture struct (or a parallel `DayContextFrameFixture` list if the struct is shared — match whichever is less invasive after Reading the file), apply it post-build exactly where `vm.life_profile` is overridden (`watch.rs:158-160`):

```rust
    let mut vm = build_watch_view_model_at(state, &usage_path, now, LocalDayMapper::Fixed(UtcOffset::UTC))?;
    if let Some(life) = life {
        vm.life_profile = life.profile.clone();
    }
    if let Some(day) = day_context {
        vm.day_context = day;
        vm.life_profile.calm_mode = day.asleep; // mirror the install-path rule
    }
```

Builders mirror `warm_life_profile`'s shape, e.g.:

```rust
fn night_asleep_day_context(fixed_now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: fixed_now - Duration::hours(2),
        phase_ends_at_utc: fixed_now + Duration::hours(6),
        asleep: true,
        sleep_onset_utc: Some(fixed_now - Duration::minutes(25)),
        ..DayContext::default()
    }
}
```

(For `night-asleep`, also rebuild the pet render with `hold_eyes_closed: true` after the override — re-call `rerender_pet_for_view_model(&mut vm, now.unix_timestamp().max(0) as u64, true)` so the frame shows closed eyes; the build-time render used the scratch store's un-asleep context.) In `scenarios.rs`, add a `watch-daycontext-` prefix arm beside the liveliness arm that emits a `day_context` manifest-input object (phase, asleep, onset, blend inputs) — do NOT let the wildcard `_` arm absorb the new ids with empty inputs.

- [ ] **Step 3: Add the whole-frame snapshot**

In `tests/dev_preview.rs`, next to `dev_preview_watch_wide_normal_frame_snapshot` (~650-662):

```rust
#[test]
fn dev_preview_watch_daycontext_night_asleep_frame_snapshot() {
    let run = PreviewRun::new();
    run.run_success("watch");
    let frame =
        std::fs::read_to_string(run.out.join("frames/watch-daycontext-night-asleep.txt")).unwrap();
    insta::assert_snapshot!("watch_daycontext_night_asleep_frame", frame);
}
```

Run `cargo test --test dev_preview` once to generate, `cargo insta review` to accept the new snapshot, run again to verify green.

- [ ] **Step 4: Human review gate**

Run: `cargo run -- dev-preview --scenario all --out target/glorp-preview && open target/glorp-preview/index.html`
Eyeball all four new frames against the table above (starfield ≤ day density; closed eyes; no burst FX in the wake frame; newborn awake). This is the spec's review-is-a-contract moment — do not skip it.

- [ ] **Step 5: Run full suite, fmt, clippy, commit**

```bash
cargo test && cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u && git add tests/snapshots/
git commit -m "feat(preview): day-context fixtures prove night, dawn, wake, and hatch scenes"
```

---

### Task 15: Final gate — full verification sweep

- [ ] **Step 1: Full local gate**

```bash
cargo test 2>&1 | tail -20
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
npm test
```

Expected: everything green, including the npm workspace smoke (menubar + Node helper paths). Any failure: fix before proceeding — test output must be pristine.

- [ ] **Step 2: Live smoke** (real ledger, real helpers — eyeball only, no state risk)

```bash
GLORP_CONFIG_DIR=$(mktemp -d) cargo run -- init --yes --seed t1-smoke --name smokey
GLORP_CONFIG_DIR=<same dir> cargo run -- watch   # confirm: day scene, no panics; quit with q
```

Then briefly run `cargo run -- watch` against the real config dir and confirm the scene renders normally during the day (sleep/night behavior verifies via fixtures; don't wait for midnight).

- [ ] **Step 3: Re-read the spec's T1 + sleep sections against the diff**

`git diff main --stat` and walk the spec's Branch T1 bullet list + "While asleep" rules one by one — every bullet maps to a commit from Tasks 1-14. Confirm the three explicitly-deferred items did NOT sneak in: dreams (T2), motes (T2), date_seed-driven sky variants (T3).

- [ ] **Step 4: Final commit if anything moved, then stop**

Implementation complete → invoke superpowers:finishing-a-development-branch to decide merge/PR.

---

## Plan self-review notes (already applied)

- **Spec coverage check:** T1 bullets → Tasks: DayContext layer (6), mapper (1), axis + readers + retention (2/3/4), rhythm (5), day/night sky + blend (12), night calm ordering (7), sleep eyes/breath/wander/speech (8/10/11), wake-on-burst + newborn + catch-up wake (6), cursor/petting gates (9), narration partition (11), milestone exemption (8), menubar scope (13), fixtures/snapshots/manifest (14). Boundary behavior: phase blend (12), dawn-rolled date_seed (6); mote fade and tiredness are T2 (not in this plan, deliberately).
- **Known cross-task signature dance:** Task 6's catch-up test compiles only after Task 11's `apply_unapplied_usage` arity change; the plan marks it `#[ignore]`-then-enable. Task 6 references `WANDER_SETTLE_SECS` (declared in Task 10) — add the single `pub const` line early if executing strictly in order.
- **Type consistency:** `DayContext` is `Copy` (all fields are `Copy`; `DaySummary`/`WakeResume` are `Copy`) — if the compiler disagrees (e.g. future non-Copy field), drop `Copy` and `.clone()` at the fixture override; do not fight it.
- **Numbers in rhythm tests are hand-derived** from the clamp/carve algorithm; the implementer must re-derive before trusting them (Task 5 Step 4 says how).
- **Two WatchApp-harness tests are contract-only by design** (Task 7 calm install, Task 8 evolution exemption, Task 9 sleeping-pet petting): the in-file test harness (`SignalPoller` / `refresh_for_test`, `app.rs:723+`) has seams this plan must not guess at. Each gives the exact arrange/assert contract; the implementer Reads the neighboring tests first and mirrors their setup. This is deliberate no-invented-details discipline, not a placeholder.



