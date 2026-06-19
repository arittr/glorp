# Glorp Tokenmaxxing Token Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Glorp's visible stats, provider deltas, calibration, and pet progression use Tokenmaxxing-style total tokens, with cached input counted fully.

**Architecture:** Add a contract-tagged total-token path beside the legacy effective-token path, then route canonical provider, storage, calibration, watch, status, and doctor behavior through the new contract. Introduce `agentsview` as the required canonical provider for Claude and Codex, perform a one-time provider-contract cutover that seeds `agentsview` cursors without feeding old history, and leave deep internal `effective_*` field renames for a later cleanup once behavior is correct.

**Tech Stack:** Rust 2021, rusqlite, serde/serde_json, time 0.3, wait-timeout, assert_cmd, predicates, npm wrapper tests.

## Global Constraints

- Canonical unit: `total_tokens = uncached_input + output + cache_creation + cache_read`.
- Reasoning output is metadata and is not added to `total_tokens`.
- `agentsview` is required for canonical Tokenmaxxing-compatible Claude and Codex totals.
- Provider commands use `--timezone America/Los_Angeles`.
- Current `tkmx-client` source does not pass `--timezone`; Glorp intentionally aligns with the Tokenmaxxing rendered profile day axis.
- Missing `agentsview` blocks canonical usage ingestion instead of feeding from `ccusage`.
- Legacy `ccusage` and `ccusage-codex` rows may remain available for diagnostics, but they must not feed canonical progression or visible Tokenmaxxing totals.
- Existing historical usage must not replay as new pet food during migration.
- The first `agentsview` cutover run for an existing pet must not change XP, lifetime food, stage, vitals, or recent feed events.
- Source labels in Tokenmaxxing-compatible visible stats should be `claude` and `codex`.
- Glorp must consume only daily numeric `agentsview` JSON and sanitize persisted diagnostics.
- First implementation uses a hard external `agentsview` dependency, resolved by `GLORP_AGENTSVIEW_BIN` first and `agentsview` on `PATH` second.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `src/usage/token_contract.rs` | Create | Token contract names and total-token math |
| `src/usage/agentsview.rs` | Create | `agentsview` command provider, discovery, diagnostics, cursor keys |
| `src/usage/day_axis.rs` | Create | Tokenmaxxing Los Angeles accounting date helpers |
| `src/usage/cutover.rs` | Create | One-time provider-contract activation and cursor seeding |
| `src/usage/mod.rs` | Modify | Export new usage modules |
| `src/usage/provider.rs` | Modify | Add contract and `total_tokens` fields to provider deltas and snapshots |
| `src/usage/normalize.rs` | Modify | Add total-token helpers, zero-default bucket parser, Tokenmaxxing source labels |
| `src/usage/identity.rs` | Modify | Add Tokenmaxxing source identity mapping for `claude` and `codex` |
| `src/usage/helper_locator.rs` | Modify | Carry optional `agentsview` path |
| `src/storage/usage_store.rs` | Modify | Add `token_contract` and `total_tokens`, contract activation, canonical queries |
| `src/game/calibration.rs` | Modify | Keep API shape but document and test that values are canonical token totals |
| `src/game/runtime.rs` | Modify | Feed, smear, guard, XP, vitals, and live signal from canonical total tokens |
| `src/commands/init.rs` | Modify | Seed Tokenmaxxing calibration and cursors during init |
| `src/commands/status.rs` | Modify | Use canonical provider and print `tokens`, not `effective tokens` |
| `src/commands/watch.rs` | Modify | Use canonical provider, Tokenmaxxing day axis, canonical source totals |
| `src/commands/doctor.rs` | Modify | Report `agentsview` version, missing helper guidance, legacy fallback data |
| `src/tui/view_model.rs` | Modify | Keep existing field names for now, add comments that values are canonical totals |
| `src/tui/life.rs` | Modify | Keep type names for now, use canonical totals as signal values |
| `npm/glorp/bin/glorp.js` | Modify | Preserve `GLORP_AGENTSVIEW_BIN` and report it in smoke fixtures |
| `npm/glorp/README.md` | Modify | Document external `agentsview` requirement |
| `README.md` | Modify | Replace normal-provider docs and user-facing effective-token wording |
| `docs/superpowers/stories/story-001-usage-provider-ccusage.md` | Modify | Mark as legacy story |
| `docs/superpowers/stories/story-004-effective-token-model.md` | Modify | Mark weighted effective tokens as legacy diagnostic |
| `docs/superpowers/stories/story-010-npm-rust-packaging.md` | Modify | Document first-pass external `agentsview` dependency |
| `tests/fixtures/agentsview-claude-daily.json` | Create | Raw Claude fixture with model breakdowns |
| `tests/fixtures/agentsview-codex-daily.json` | Create | Raw Codex fixture with model breakdowns |
| `tests/fixtures/agentsview-omitted-zeros.json` | Create | Raw fixture proving omitted counters default to zero |
| `tests/fixtures/agentsview-drew-2026-06-18-tokenmaxxing.json` | Create | Captured server/API regression fixture |
| `tests/fixtures/helpers/agentsview-ok.mjs` | Create | Test helper for both agent commands and version |
| `tests/fixtures/helpers/agentsview-next.mjs` | Create | Incremented totals for delta tests |
| `tests/fixtures/helpers/agentsview-invalid-json.mjs` | Create | Invalid JSON diagnostic helper |
| `tests/fixtures/helpers/agentsview-fails.mjs` | Create | Non-zero exit diagnostic helper |
| `tests/fixtures/helpers/agentsview-secret-stderr.mjs` | Create | Sanitization helper |
| `tests/usage_provider.rs` | Modify | Add agentsview parser/provider/cursor tests |
| `tests/runtime_integration.rs` | Modify | Add canonical total-token feeding and cutover tests |
| `tests/watch_integration.rs` | Modify | Add Tokenmaxxing day-axis and source-label tests |
| `tests/doctor_status.rs` | Modify | Add agentsview doctor/status tests |
| `tests/storage_privacy.rs` | Modify | Add schema migration, contract filtering, privacy tests |
| `tests/helper_locator.rs` | Modify | Add `agentsview` locator coverage |
| `tests/acceptance_matrix.rs` | Modify | Update provider and packaging contract checks |
| `npm/glorp/test/smoke.mjs` | Modify | Add external agentsview smoke behavior |

---

## Task 1: Add Token Contract Primitives And Storage Columns

**Files:**
- Create: `src/usage/token_contract.rs`
- Modify: `src/usage/mod.rs`
- Modify: `src/usage/provider.rs`
- Modify: `src/usage/normalize.rs`
- Modify: `src/storage/usage_store.rs`
- Modify: `tests/game_rules.rs`
- Modify: `tests/storage_privacy.rs`

**Interfaces:**
- Produces: `TOKENMAXXING_TOTAL_V1`, `WEIGHTED_EFFECTIVE_V1`, `RawTokenTotals::total_tokens()`, `UsageDelta.total_tokens`, `UsageDelta.token_contract`
- Produces: `NormalizedUsageEvent.total_tokens`, `NormalizedUsageEvent.token_contract`
- Produces: `UsageStore::canonical_total_tokens_between(start, end) -> Result<f64>`
- Produces: `UsageStore::canonical_total_tokens_by_source_between(start, end) -> Result<Vec<(String, f64)>>`

- [ ] **Step 1: Write failing token-contract math tests**

Append to `tests/game_rules.rs`:

```rust
#[test]
fn tokenmaxxing_total_counts_cache_reads_fully_and_excludes_reasoning() {
    let totals = RawTokenTotals {
        uncached_input: 100,
        output: 200,
        cache_creation: 300,
        cache_read: 4_000,
        reasoning_output: 9_999,
    };

    assert_eq!(totals.total_tokens(), 4_600.0);
}

#[test]
fn legacy_cache_read_weight_does_not_define_canonical_total_tokens() {
    let config = AppConfig {
        cache_read_weight: 0.05,
        ..AppConfig::default()
    };
    let weights = EffectiveTokenWeights::from_config(config);
    let buckets = TokenBuckets {
        uncached_input: 0,
        output: 0,
        cache_creation: 0,
        cache_read: 1_000,
        reasoning_output: 999_999,
    };
    let totals = RawTokenTotals {
        cache_read: 1_000,
        reasoning_output: 999_999,
        ..RawTokenTotals::default()
    };

    assert_eq!(weights.compute(buckets), 50.0);
    assert_eq!(totals.total_tokens(), 1_000.0);
}
```

Add this import near the top of `tests/game_rules.rs`:

```rust
use glorp::usage::normalize::RawTokenTotals;
```

- [ ] **Step 2: Run the math tests and verify failure**

Run:

```bash
cargo test --test game_rules tokenmaxxing_total_counts_cache_reads_fully_and_excludes_reasoning
cargo test --test game_rules legacy_cache_read_weight_does_not_define_canonical_total_tokens
```

Expected: compile failure because `RawTokenTotals::total_tokens` does not exist.

- [ ] **Step 3: Implement token contract constants and total-token math**

Create `src/usage/token_contract.rs`:

```rust
pub const TOKENMAXXING_TOTAL_V1: &str = "tokenmaxxing_total_v1";
pub const WEIGHTED_EFFECTIVE_V1: &str = "weighted_effective_v1";
```

Modify `src/usage/mod.rs`:

```rust
pub mod ccusage;
pub mod helper_locator;
pub mod identity;
pub mod normalize;
pub mod provider;
pub mod token_contract;

pub use identity::{normalize_source_label, SourceFamily, SourceIdentity};
```

Add this method to `impl RawTokenTotals` in `src/usage/normalize.rs`:

```rust
    pub fn total_tokens(self) -> f64 {
        self.uncached_input as f64
            + self.output as f64
            + self.cache_creation as f64
            + self.cache_read as f64
    }
```

- [ ] **Step 4: Add provider-level canonical fields**

Modify `UsagePollResult` and `UsageDelta` in `src/usage/provider.rs`:

```rust
pub struct UsagePollResult {
    pub deltas: Vec<UsageDelta>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub total_effective_tokens: f64,
    pub total_tokens: f64,
}

pub struct UsageDelta {
    pub provider_surface: String,
    pub source_identity: SourceIdentity,
    pub command: String,
    pub effective_tokens: f64,
    pub total_tokens: f64,
    pub token_contract: String,
    pub confidence: String,
    pub period_start: OffsetDateTime,
    pub observed_at: OffsetDateTime,
    pub model: Option<String>,
    pub cursor_update: ProviderCursorUpdate,
    pub token_totals: Option<RawTokenTotals>,
}
```

Update existing `UsagePollResult` construction sites by setting `total_tokens` equal to `total_effective_tokens` for legacy `ccusage` code. Update existing `UsageDelta` construction sites by setting `total_tokens: effective_tokens` and `token_contract: WEIGHTED_EFFECTIVE_V1.to_string()` until later tasks switch canonical providers.

- [ ] **Step 5: Write failing storage contract-filtering tests**

Append to `tests/storage_privacy.rs`:

```rust
#[test]
fn canonical_total_queries_exclude_legacy_weighted_rows() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let now = datetime!(2026-06-18 19:10 UTC);

    let legacy = NormalizedUsageEvent {
        token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
        total_tokens: 999_999.0,
        effective_tokens: 999_999.0,
        ..NormalizedUsageEvent::for_test_at(now, 999_999.0)
    };
    let canonical = NormalizedUsageEvent {
        provider_surface: "codex".to_string(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
        total_tokens: 715_380_912.0,
        effective_tokens: 715_380_912.0,
        ..NormalizedUsageEvent::for_test_at(now, 715_380_912.0)
    };

    store.insert_event(&legacy).unwrap();
    store.insert_event(&canonical).unwrap();

    let total = store
        .canonical_total_tokens_between(now - Duration::minutes(1), now + Duration::minutes(1))
        .unwrap();
    let by_source = store
        .canonical_total_tokens_by_source_between(
            now - Duration::minutes(1),
            now + Duration::minutes(1),
        )
        .unwrap();

    assert_eq!(total, 715_380_912.0);
    assert_eq!(by_source, vec![("codex".to_string(), 715_380_912.0)]);
}
```

- [ ] **Step 6: Run the storage test and verify failure**

Run:

```bash
cargo test --test storage_privacy canonical_total_queries_exclude_legacy_weighted_rows
```

Expected: compile failure because storage fields and query methods do not exist.

- [ ] **Step 7: Add storage fields and migration**

Modify `NormalizedUsageEvent` in `src/storage/usage_store.rs`:

```rust
pub struct NormalizedUsageEvent {
    pub provider_surface: String,
    pub provider_version: String,
    pub parser_version: String,
    pub command: String,
    pub source_surface: String,
    pub period_start: OffsetDateTime,
    pub observed_at: OffsetDateTime,
    pub bucket_at: OffsetDateTime,
    pub model: Option<String>,
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_creation_tokens: f64,
    pub cache_read_tokens: f64,
    pub reasoning_output_tokens: f64,
    pub effective_tokens: f64,
    pub total_tokens: f64,
    pub token_contract: String,
    pub cost_usd: Option<f64>,
    pub confidence: String,
    pub provider_delta_id: Option<String>,
}
```

In `NormalizedUsageEvent::for_test_at`, set:

```rust
            total_tokens: effective_tokens,
            token_contract: crate::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
```

Add `total_tokens` and `token_contract` to every `INSERT`, `SELECT`, and row-mapping path for `usage_events`.

Add these migration calls after the existing `provider_cursor_value` migration:

```rust
        ensure_usage_event_column(
            &self.conn,
            "total_tokens",
            "ALTER TABLE usage_events ADD COLUMN total_tokens REAL NOT NULL DEFAULT 0.0;",
            "UPDATE usage_events SET total_tokens = effective_tokens WHERE total_tokens = 0.0;",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "token_contract",
            "ALTER TABLE usage_events ADD COLUMN token_contract TEXT NOT NULL DEFAULT 'weighted_effective_v1';",
            "",
        )?;
```

- [ ] **Step 8: Add canonical query methods**

Add to `impl UsageStore`:

```rust
    pub fn canonical_total_tokens_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<f64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(total_tokens), 0.0)
                 FROM usage_events
                 WHERE applied_at IS NOT NULL
                   AND token_contract = ?1
                   AND bucket_at >= ?2
                   AND bucket_at < ?3",
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    format_time(start)?,
                    format_time(end)?,
                ],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn canonical_total_tokens_by_source_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<Vec<(String, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_surface, COALESCE(SUM(total_tokens), 0.0)
             FROM usage_events
             WHERE applied_at IS NOT NULL
               AND token_contract = ?1
               AND bucket_at >= ?2
               AND bucket_at < ?3
             GROUP BY provider_surface
             ORDER BY provider_surface",
        )?;
        let rows = stmt
            .query_map(
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    format_time(start)?,
                    format_time(end)?,
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
```

- [ ] **Step 9: Run focused tests**

Run:

```bash
cargo test --test game_rules tokenmaxxing_total_counts_cache_reads_fully_and_excludes_reasoning
cargo test --test game_rules legacy_cache_read_weight_does_not_define_canonical_total_tokens
cargo test --test storage_privacy canonical_total_queries_exclude_legacy_weighted_rows
cargo test --test usage_provider
```

Expected: all listed tests pass after construction sites are updated.

- [ ] **Step 10: Commit**

```bash
git add src/usage/token_contract.rs src/usage/mod.rs src/usage/provider.rs src/usage/normalize.rs src/storage/usage_store.rs tests/game_rules.rs tests/storage_privacy.rs tests/usage_provider.rs
git commit -m "feat: add tokenmaxxing token contract storage"
```

---

## Task 2: Add Tokenmaxxing Day Axis Helpers

**Files:**
- Create: `src/usage/day_axis.rs`
- Modify: `src/usage/mod.rs`
- Modify: `tests/watch_integration.rs`

**Interfaces:**
- Produces: `TOKENMAXXING_TIMEZONE: &str`
- Produces: `tokenmaxxing_day_start(date: Date) -> OffsetDateTime`
- Produces: `tokenmaxxing_today_window(now: OffsetDateTime) -> (OffsetDateTime, OffsetDateTime)`
- Produces: `parse_agentsview_period_date(period: &str) -> Result<(Date, OffsetDateTime)>`

- [ ] **Step 1: Write failing day-axis tests**

Append to `tests/watch_integration.rs`:

```rust
#[test]
fn tokenmaxxing_day_axis_interprets_date_as_los_angeles_midnight() {
    use glorp::usage::day_axis::{parse_agentsview_period_date, tokenmaxxing_day_start};
    use time::{Date, Month};

    let date = Date::from_calendar_date(2026, Month::June, 18).unwrap();
    assert_eq!(tokenmaxxing_day_start(date), datetime!(2026-06-18 07:00 UTC));

    let (parsed_date, parsed_start) = parse_agentsview_period_date("2026-06-18").unwrap();
    assert_eq!(parsed_date, date);
    assert_eq!(parsed_start, datetime!(2026-06-18 07:00 UTC));
}

#[test]
fn tokenmaxxing_day_axis_handles_los_angeles_dst_boundaries() {
    use glorp::usage::day_axis::tokenmaxxing_day_start;
    use time::{Date, Month};

    let before_spring = Date::from_calendar_date(2026, Month::March, 8).unwrap();
    let after_spring = Date::from_calendar_date(2026, Month::March, 9).unwrap();
    let fall_back_day = Date::from_calendar_date(2026, Month::November, 1).unwrap();
    let after_fall = Date::from_calendar_date(2026, Month::November, 2).unwrap();

    assert_eq!(tokenmaxxing_day_start(before_spring), datetime!(2026-03-08 08:00 UTC));
    assert_eq!(tokenmaxxing_day_start(after_spring), datetime!(2026-03-09 07:00 UTC));
    assert_eq!(tokenmaxxing_day_start(fall_back_day), datetime!(2026-11-01 07:00 UTC));
    assert_eq!(tokenmaxxing_day_start(after_fall), datetime!(2026-11-02 08:00 UTC));
}
```

- [ ] **Step 2: Run the day-axis tests and verify failure**

Run:

```bash
cargo test --test watch_integration tokenmaxxing_day_axis
```

Expected: compile failure because `usage::day_axis` does not exist.

- [ ] **Step 3: Implement the Los Angeles accounting-date helper**

Create `src/usage/day_axis.rs`:

```rust
use crate::error::{GlorpError, Result};
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset, Weekday};

pub const TOKENMAXXING_TIMEZONE: &str = "America/Los_Angeles";

pub fn parse_agentsview_period_date(period: &str) -> Result<(Date, OffsetDateTime)> {
    let mut parts = period.split('-');
    let year = parts
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| GlorpError::Message(format!("invalid agentsview period date {period}")))?;
    let month = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .and_then(|value| Month::try_from(value).ok())
        .ok_or_else(|| GlorpError::Message(format!("invalid agentsview period date {period}")))?;
    let day = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .ok_or_else(|| GlorpError::Message(format!("invalid agentsview period date {period}")))?;
    if parts.next().is_some() {
        return Err(GlorpError::Message(format!(
            "invalid agentsview period date {period}"
        )));
    }
    let date = Date::from_calendar_date(year, month, day)
        .map_err(|err| GlorpError::Message(format!("invalid agentsview period date {period}: {err}")))?;
    Ok((date, tokenmaxxing_day_start(date)))
}

pub fn tokenmaxxing_today_window(now: OffsetDateTime) -> (OffsetDateTime, OffsetDateTime) {
    let date = tokenmaxxing_date(now);
    (
        tokenmaxxing_day_start(date),
        tokenmaxxing_day_start(date + time::Duration::days(1)),
    )
}

pub fn tokenmaxxing_date(now: OffsetDateTime) -> Date {
    now.to_offset(los_angeles_offset_for_instant(now)).date()
}

pub fn tokenmaxxing_day_start(date: Date) -> OffsetDateTime {
    PrimitiveDateTime::new(date, Time::MIDNIGHT)
        .assume_offset(los_angeles_offset_for_local_midnight(date))
        .to_offset(UtcOffset::UTC)
}

fn los_angeles_offset_for_local_midnight(date: Date) -> UtcOffset {
    let start = nth_weekday_of_month(date.year(), Month::March, Weekday::Sunday, 2);
    let end = nth_weekday_of_month(date.year(), Month::November, Weekday::Sunday, 1);
    if date > start && date <= end {
        UtcOffset::from_hms(-7, 0, 0).unwrap()
    } else {
        UtcOffset::from_hms(-8, 0, 0).unwrap()
    }
}

fn los_angeles_offset_for_instant(instant: OffsetDateTime) -> UtcOffset {
    let pacific = instant.to_offset(UtcOffset::from_hms(-8, 0, 0).unwrap()).date();
    let start = nth_weekday_of_month(pacific.year(), Month::March, Weekday::Sunday, 2);
    let end = nth_weekday_of_month(pacific.year(), Month::November, Weekday::Sunday, 1);
    let dst_start_utc = PrimitiveDateTime::new(start, Time::from_hms(2, 0, 0).unwrap())
        .assume_offset(UtcOffset::from_hms(-8, 0, 0).unwrap())
        .to_offset(UtcOffset::UTC);
    let dst_end_utc = PrimitiveDateTime::new(end, Time::from_hms(2, 0, 0).unwrap())
        .assume_offset(UtcOffset::from_hms(-7, 0, 0).unwrap())
        .to_offset(UtcOffset::UTC);
    if instant >= dst_start_utc && instant < dst_end_utc {
        UtcOffset::from_hms(-7, 0, 0).unwrap()
    } else {
        UtcOffset::from_hms(-8, 0, 0).unwrap()
    }
}

fn nth_weekday_of_month(year: i32, month: Month, weekday: Weekday, nth: u8) -> Date {
    let mut date = Date::from_calendar_date(year, month, 1).unwrap();
    let mut seen = 0_u8;
    loop {
        if date.weekday() == weekday {
            seen += 1;
            if seen == nth {
                return date;
            }
        }
        date = date + time::Duration::days(1);
    }
}
```

This first pass intentionally keeps the timezone implementation narrow and test-pinned to the Tokenmaxxing axis. If Glorp later supports arbitrary user-selected accounting zones, replace this module with an IANA timezone library in a separate change.

- [ ] **Step 4: Export the day-axis module**

Add this line to `src/usage/mod.rs`:

```rust
pub mod day_axis;
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test --test watch_integration tokenmaxxing_day_axis
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/usage/day_axis.rs src/usage/mod.rs tests/watch_integration.rs
git commit -m "feat: add tokenmaxxing day axis"
```

---

## Task 3: Add Agentsview Fixtures, Normalization, And Provider

**Files:**
- Create: `src/usage/agentsview.rs`
- Modify: `src/usage/identity.rs`
- Modify: `src/usage/normalize.rs`
- Modify: `src/usage/mod.rs`
- Modify: `tests/usage_provider.rs`
- Create: `tests/fixtures/agentsview-claude-daily.json`
- Create: `tests/fixtures/agentsview-codex-daily.json`
- Create: `tests/fixtures/agentsview-omitted-zeros.json`
- Create: `tests/fixtures/agentsview-drew-2026-06-18-tokenmaxxing.json`
- Create: `tests/fixtures/helpers/agentsview-ok.mjs`
- Create: `tests/fixtures/helpers/agentsview-next.mjs`
- Create: `tests/fixtures/helpers/agentsview-invalid-json.mjs`
- Create: `tests/fixtures/helpers/agentsview-fails.mjs`
- Create: `tests/fixtures/helpers/agentsview-secret-stderr.mjs`

**Interfaces:**
- Consumes: `RawTokenTotals::total_tokens()`, Tokenmaxxing day-axis helpers, token contract constants
- Produces: `AgentsviewCommandProvider::new(AgentsviewPaths)`
- Produces: `AgentsviewDiscovery::discover()`
- Produces: `GLORP_AGENTSVIEW_BIN` discovery before `PATH`

- [ ] **Step 1: Add raw fixture files**

Create `tests/fixtures/agentsview-claude-daily.json`:

```json
{
  "daily": [
    {
      "date": "2026-06-18",
      "inputTokens": 615837,
      "outputTokens": 1073546,
      "cacheCreationTokens": 5918072,
      "cacheReadTokens": 38404437,
      "totalCost": 80.32341310000001,
      "modelsUsed": ["claude-opus-4-8", "claude-haiku-4-5-20251001"],
      "modelBreakdowns": [
        {
          "modelName": "claude-opus-4-8",
          "inputTokens": 612992,
          "outputTokens": 1072059,
          "cacheCreationTokens": 5083568,
          "cacheReadTokens": 34477061,
          "cost": 78.87726550000001
        },
        {
          "modelName": "claude-haiku-4-5-20251001",
          "inputTokens": 2845,
          "outputTokens": 1487,
          "cacheCreationTokens": 834504,
          "cacheReadTokens": 3927376,
          "cost": 1.4461476000000002
        }
      ]
    }
  ]
}
```

Create `tests/fixtures/agentsview-codex-daily.json`:

```json
{
  "daily": [
    {
      "date": "2026-06-18",
      "inputTokens": 42530006,
      "outputTokens": 4084648,
      "cacheCreationTokens": 0,
      "cacheReadTokens": 697197568,
      "totalCost": 539.397275,
      "modelsUsed": ["gpt-5.5", "gpt-5.4", "gpt-5.4-mini"],
      "modelBreakdowns": [
        {
          "modelName": "gpt-5.5",
          "inputTokens": 31028179,
          "outputTokens": 2463075,
          "cacheCreationTokens": 0,
          "cacheReadTokens": 517477376,
          "cost": 487.7718330000001
        },
        {
          "modelName": "gpt-5.4",
          "inputTokens": 10928897,
          "outputTokens": 1578707,
          "cacheCreationTokens": 0,
          "cacheReadTokens": 176103808,
          "cost": 51.002847499999945
        },
        {
          "modelName": "gpt-5.4-mini",
          "inputTokens": 572930,
          "outputTokens": 42866,
          "cacheCreationTokens": 0,
          "cacheReadTokens": 3616384,
          "cost": 0.6225945
        }
      ]
    }
  ]
}
```

Create `tests/fixtures/agentsview-omitted-zeros.json`:

```json
{
  "daily": [
    {
      "date": "2026-06-18",
      "modelBreakdowns": [
        {
          "modelName": "gpt-5.4-mini",
          "inputTokens": 10,
          "outputTokens": 20
        }
      ]
    }
  ]
}
```

Create `tests/fixtures/agentsview-drew-2026-06-18-tokenmaxxing.json`:

```json
{
  "date": "2026-06-18",
  "captured_from": "https://tokenmaxxing.odio.dev/api/user/drew",
  "sources": {
    "claude": 46011892,
    "codex": 669369020
  },
  "total": 715380912
}
```

- [ ] **Step 2: Add helper scripts**

Create `tests/fixtures/helpers/agentsview-ok.mjs`:

```javascript
#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.resolve(here, "..");
const args = process.argv.slice(2);

if (args.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}

const agent = args[args.indexOf("--agent") + 1];
if (!args.includes("usage") || !args.includes("daily") || !args.includes("--json")) {
  console.error("unexpected agentsview argv");
  process.exit(2);
}
if (!args.includes("--timezone") || args[args.indexOf("--timezone") + 1] !== "America/Los_Angeles") {
  console.error("missing timezone");
  process.exit(2);
}

const file = agent === "claude" ? "agentsview-claude-daily.json" : "agentsview-codex-daily.json";
process.stdout.write(fs.readFileSync(path.join(fixtures, file), "utf8"));
```

Create `tests/fixtures/helpers/agentsview-next.mjs`:

```javascript
#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.resolve(here, "..");
const args = process.argv.slice(2);

if (args.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}

const agent = args[args.indexOf("--agent") + 1];
const file = agent === "claude" ? "agentsview-claude-daily.json" : "agentsview-codex-daily.json";
const payload = JSON.parse(fs.readFileSync(path.join(fixtures, file), "utf8"));
payload.daily[0].modelBreakdowns[0].inputTokens += 100;
payload.daily[0].modelBreakdowns[0].outputTokens += 200;
payload.daily[0].modelBreakdowns[0].cacheCreationTokens += 300;
payload.daily[0].modelBreakdowns[0].cacheReadTokens += 400;
process.stdout.write(JSON.stringify(payload, null, 2));
```

Create `tests/fixtures/helpers/agentsview-invalid-json.mjs`:

```javascript
#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}
console.log("{not valid json");
```

Create `tests/fixtures/helpers/agentsview-fails.mjs`:

```javascript
#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}
console.error("agentsview fixture failed");
process.exit(7);
```

Create `tests/fixtures/helpers/agentsview-secret-stderr.mjs`:

```javascript
#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}
console.error("secret prompt secret response /Users/drew/private/session.jsonl");
process.exit(9);
```

- [ ] **Step 3: Write failing agentsview normalization tests**

Append to `tests/usage_provider.rs`:

```rust
fn agentsview_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/helpers")
        .join(name)
}

#[test]
fn agentsview_provider_normalizes_full_cache_totals_and_external_source_labels() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(agentsview_fixture("agentsview-ok.mjs")),
        },
    );

    let result = provider.poll(&mut store).unwrap();
    let codex = result
        .deltas
        .iter()
        .find(|delta| delta.provider_surface == "codex" && delta.model.as_deref() == Some("gpt-5.5"))
        .unwrap();
    let claude = result
        .deltas
        .iter()
        .find(|delta| delta.provider_surface == "claude" && delta.model.as_deref() == Some("claude-opus-4-8"))
        .unwrap();

    assert_eq!(codex.source_identity.display_name, "codex");
    assert_eq!(claude.source_identity.display_name, "claude");
    assert_eq!(codex.token_contract, glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1);
    assert_eq!(codex.total_tokens, 31028179.0 + 2463075.0 + 517477376.0);
    assert_eq!(claude.total_tokens, 612992.0 + 1072059.0 + 5083568.0 + 34477061.0);
    assert_eq!(result.total_tokens, result.deltas.iter().map(|d| d.total_tokens).sum::<f64>());
}

#[test]
fn agentsview_provider_requires_los_angeles_timezone_arg() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(agentsview_fixture("agentsview-ok.mjs")),
        },
    );

    let result = provider.poll(&mut store).unwrap();

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}
```

- [ ] **Step 4: Run agentsview tests and verify failure**

Run:

```bash
cargo test --test usage_provider agentsview_provider_normalizes_full_cache_totals_and_external_source_labels
cargo test --test usage_provider agentsview_provider_requires_los_angeles_timezone_arg
```

Expected: compile failure because `usage::agentsview` does not exist.

- [ ] **Step 5: Add Tokenmaxxing source identity mapping**

Add to `impl SourceIdentity` in `src/usage/identity.rs`:

```rust
    pub fn from_tokenmaxxing_source(surface: &str) -> Self {
        let normalized = surface.trim().to_ascii_lowercase();
        let provider_surface = match normalized.as_str() {
            "claude" | "claude-code" => "claude".to_string(),
            "codex" | "ccusage-codex" => "codex".to_string(),
            other if other.is_empty() || other == "all" => "unknown".to_string(),
            other => other.to_string(),
        };
        let source_family = match provider_surface.as_str() {
            "claude" | "codex" => SourceFamily::KnownCodingAgent,
            _ => SourceFamily::UnknownCodingAgent,
        };
        Self {
            display_name: provider_surface.clone(),
            provider_surface,
            raw_agent: Some(surface.to_string()),
            source_family,
        }
    }
```

- [ ] **Step 6: Add zero-default parser helper**

In `src/usage/normalize.rs`, add:

```rust
pub fn normalize_agentsview_json(
    agent: &str,
    text: &str,
) -> std::result::Result<NormalizedUsageBatch, ProviderDiagnostic> {
    let value: Value = serde_json::from_str(text).map_err(|_| ProviderDiagnostic {
        provider_surface: agent.to_string(),
        code: "invalid_json".to_string(),
        message: format!("agentsview {agent} returned invalid_json"),
    })?;
    Ok(normalize_agentsview_value(agent, &value))
}

fn normalize_agentsview_value(agent: &str, value: &Value) -> NormalizedUsageBatch {
    let mut batch = NormalizedUsageBatch::default();
    let Some(rows) = value.get("daily").and_then(Value::as_array) else {
        batch.diagnostics.push(ProviderDiagnostic {
            provider_surface: agent.to_string(),
            code: "missing_daily".to_string(),
            message: format!("agentsview {agent} missing daily"),
        });
        return batch;
    };
    let source = SourceIdentity::from_tokenmaxxing_source(agent);
    for row in rows {
        let period_start = match period_start_field(&source.provider_surface, row) {
            Ok(period) => period,
            Err(diagnostic) => {
                batch.diagnostics.push(diagnostic);
                continue;
            }
        };
        if let Some(breakdowns) = row.get("modelBreakdowns").and_then(Value::as_array) {
            for model_row in breakdowns {
                batch.records.push(NormalizedUsageRecord {
                    source_identity: source.clone(),
                    period_start: period_start.clone(),
                    model: optional_string(model_row, "modelName"),
                    raw_totals: RawTokenTotals {
                        uncached_input: optional_u64(model_row, "inputTokens").unwrap_or(0),
                        output: optional_u64(model_row, "outputTokens").unwrap_or(0),
                        cache_creation: optional_u64(model_row, "cacheCreationTokens").unwrap_or(0),
                        cache_read: optional_u64(model_row, "cacheReadTokens").unwrap_or(0),
                        reasoning_output: optional_u64(model_row, "reasoningOutputTokens").unwrap_or(0),
                    },
                    display_cost_usd: optional_f64(model_row, "cost"),
                    confidence: "local-log-derived".to_string(),
                });
            }
        }
    }
    batch
}
```

- [ ] **Step 7: Implement `AgentsviewCommandProvider`**

Create `src/usage/agentsview.rs`:

```rust
use crate::error::{GlorpError, Result};
use crate::storage::usage_store::{ProviderCursorUpdate, ProviderDiagnostic as StoredProviderDiagnostic, UsageStore};
use crate::usage::ccusage::{run_command_with_timeout, HELPER_SUBPROCESS_TIMEOUT};
use crate::usage::day_axis::{parse_agentsview_period_date, TOKENMAXXING_TIMEZONE};
use crate::usage::normalize::{normalize_agentsview_json, RawTokenTotals};
use crate::usage::provider::{ProviderCursorKey, ProviderDiagnostic, UsageDelta, UsagePollResult, UsageProvider, UsageSnapshot};
use crate::usage::token_contract::TOKENMAXXING_TOTAL_V1;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::OffsetDateTime;

pub const AGENTSVIEW_COMMAND: &str = "agentsview usage daily";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentsviewPaths {
    pub agentsview: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentsviewDiscovery {
    pub agentsview: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct AgentsviewCommandProvider {
    paths: AgentsviewPaths,
}

impl AgentsviewCommandProvider {
    pub fn new(paths: AgentsviewPaths) -> Self {
        Self { paths }
    }

    pub fn from_environment() -> Self {
        Self::new(AgentsviewDiscovery::discover().into())
    }

    fn poll_agent(&self, store: &mut UsageStore, agent: &str, no_sync: bool) -> Result<UsagePollResult> {
        let Some(helper) = self.paths.agentsview.as_deref() else {
            let diagnostic = diagnostic(agent, "missing_helper", "agentsview helper was not found");
            persist_diagnostic(store, &diagnostic)?;
            return Ok(empty_poll(vec![diagnostic]));
        };
        let version = self.version(helper).unwrap_or_else(|| "unknown".to_string());
        let output = self.run_usage(helper, agent, no_sync)?;
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let diagnostic = diagnostic(agent, "helper_exit", &format!("agentsview exited with status {code}"));
            persist_diagnostic(store, &diagnostic)?;
            return Ok(empty_poll(vec![diagnostic]));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let batch = normalize_agentsview_json(agent, &stdout).map_err(|diagnostic| GlorpError::Message(diagnostic.message))?;
        for diagnostic in &batch.diagnostics {
            persist_diagnostic(store, diagnostic)?;
        }

        let mut deltas = Vec::new();
        let observed_at = OffsetDateTime::now_utc();
        for record in batch.records {
            let Ok((_date, period_start)) = parse_agentsview_period_date(&record.period_start) else {
                let diagnostic = diagnostic(agent, "invalid_period_start", &format!("agentsview {agent} invalid_period_start {}", record.period_start));
                persist_diagnostic(store, &diagnostic)?;
                continue;
            };
            let key = ProviderCursorKey {
                provider_surface: record.source_identity.provider_surface.clone(),
                command: AGENTSVIEW_COMMAND.to_string(),
                source_surface: "daily".to_string(),
                period_start: record.period_start.clone(),
                model: record.model.clone(),
                raw_source_id: Some(record.source_identity.display_name.clone()),
            };
            let cursor_key = serde_json::to_string(&key)?;
            let previous_raw = store.provider_cursor(&record.source_identity.provider_surface, &cursor_key)?;
            let previous = previous_raw
                .as_deref()
                .and_then(|value| serde_json::from_str::<RawTokenTotals>(value).ok())
                .unwrap_or_default();
            let Some(delta_totals) = record.raw_totals.positive_delta_since(previous) else {
                let diagnostic = diagnostic(agent, "cursor_total_decreased", &format!("agentsview {agent} cursor_total_decreased for {cursor_key}"));
                persist_diagnostic(store, &diagnostic)?;
                write_cursor(store, &record.source_identity.provider_surface, &cursor_key, record.raw_totals, &version)?;
                continue;
            };
            if !delta_totals.has_positive_effective_bucket() {
                continue;
            }
            let cursor_value = serde_json::to_string(&record.raw_totals)?;
            deltas.push(UsageDelta {
                provider_surface: record.source_identity.provider_surface.clone(),
                source_identity: record.source_identity,
                command: AGENTSVIEW_COMMAND.to_string(),
                effective_tokens: delta_totals.total_tokens(),
                total_tokens: delta_totals.total_tokens(),
                token_contract: TOKENMAXXING_TOTAL_V1.to_string(),
                confidence: record.confidence,
                period_start,
                observed_at,
                model: record.model,
                cursor_update: ProviderCursorUpdate {
                    provider_surface: agent.to_string(),
                    cursor_key,
                    cursor_value,
                    provider_version: version.clone(),
                    parser_version: version.clone(),
                },
                token_totals: Some(delta_totals),
            });
        }
        let total_tokens = deltas.iter().map(|delta| delta.total_tokens).sum();
        Ok(UsagePollResult {
            total_effective_tokens: total_tokens,
            total_tokens,
            deltas,
            diagnostics: batch.diagnostics,
        })
    }

    fn version(&self, helper: &Path) -> Option<String> {
        let output = Command::new(helper).arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout).lines().next().map(str::to_string)
    }

    fn run_usage(&self, helper: &Path, agent: &str, no_sync: bool) -> Result<std::process::Output> {
        let mut command = Command::new(helper);
        command.args([
            "usage",
            "daily",
            "--json",
            "--breakdown",
            "--agent",
            agent,
            "--since",
            "1970-01-01",
            "--timezone",
            TOKENMAXXING_TIMEZONE,
        ]);
        if no_sync {
            command.arg("--no-sync");
        }
        command.stdin(Stdio::null());
        run_command_with_timeout(&mut command, HELPER_SUBPROCESS_TIMEOUT)
    }
}

impl UsageProvider for AgentsviewCommandProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
        let claude = self.poll_agent(store, "claude", false)?;
        let codex = self.poll_agent(store, "codex", true)?;
        let mut deltas = claude.deltas;
        deltas.extend(codex.deltas);
        let mut diagnostics = claude.diagnostics;
        diagnostics.extend(codex.diagnostics);
        let total_tokens = deltas.iter().map(|delta| delta.total_tokens).sum();
        Ok(UsagePollResult {
            deltas,
            diagnostics,
            total_effective_tokens: total_tokens,
            total_tokens,
        })
    }

    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot> {
        let poll = self.poll(store)?;
        let daily_usage = poll
            .deltas
            .iter()
            .map(|delta| crate::game::calibration::DailyUsage::with_activity_timestamp(delta.period_start, delta.total_tokens))
            .collect();
        let cursor_updates = poll.deltas.iter().map(|delta| delta.cursor_update.clone()).collect();
        Ok(UsageSnapshot {
            daily_usage,
            cursor_updates,
            diagnostics: poll.diagnostics,
        })
    }
}

impl AgentsviewDiscovery {
    pub fn discover() -> Self {
        let agentsview = std::env::var_os("GLORP_AGENTSVIEW_BIN")
            .map(PathBuf::from)
            .or_else(|| which::which("agentsview").ok());
        Self { agentsview }
    }
}

impl From<AgentsviewDiscovery> for AgentsviewPaths {
    fn from(value: AgentsviewDiscovery) -> Self {
        Self {
            agentsview: value.agentsview,
        }
    }
}

fn empty_poll(diagnostics: Vec<ProviderDiagnostic>) -> UsagePollResult {
    UsagePollResult {
        deltas: Vec::new(),
        diagnostics,
        total_effective_tokens: 0.0,
        total_tokens: 0.0,
    }
}

fn diagnostic(provider_surface: &str, code: &str, message: &str) -> ProviderDiagnostic {
    ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn persist_diagnostic(store: &UsageStore, diagnostic: &ProviderDiagnostic) -> Result<()> {
    store.insert_diagnostic(&StoredProviderDiagnostic {
        provider_surface: diagnostic.provider_surface.clone(),
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        recorded_at: OffsetDateTime::now_utc(),
    })
}

fn write_cursor(
    store: &UsageStore,
    provider_surface: &str,
    cursor_key: &str,
    raw_totals: RawTokenTotals,
    version: &str,
) -> Result<()> {
    store.set_provider_cursor(
        provider_surface,
        cursor_key,
        &serde_json::to_string(&raw_totals)?,
        version,
        version,
    )
}
```

After adding the file, run `cargo check --tests`, remove any compiler-reported unused imports, and keep diagnostic messages limited to the provider, code, and exit status. Do not include stderr content in messages.

- [ ] **Step 8: Export the agentsview module**

Add this line to `src/usage/mod.rs`:

```rust
pub mod agentsview;
```

- [ ] **Step 9: Run focused agentsview tests**

Run:

```bash
cargo test --test usage_provider agentsview_provider_normalizes_full_cache_totals_and_external_source_labels
cargo test --test usage_provider agentsview_provider_requires_los_angeles_timezone_arg
cargo test --test usage_provider invalid_json_and_helper_stderr_are_sanitized
```

Expected: pass.

- [ ] **Step 10: Commit**

```bash
git add src/usage/agentsview.rs src/usage/identity.rs src/usage/normalize.rs src/usage/mod.rs tests/usage_provider.rs tests/fixtures/agentsview-claude-daily.json tests/fixtures/agentsview-codex-daily.json tests/fixtures/agentsview-omitted-zeros.json tests/fixtures/agentsview-drew-2026-06-18-tokenmaxxing.json tests/fixtures/helpers/agentsview-ok.mjs tests/fixtures/helpers/agentsview-next.mjs tests/fixtures/helpers/agentsview-invalid-json.mjs tests/fixtures/helpers/agentsview-fails.mjs tests/fixtures/helpers/agentsview-secret-stderr.mjs
git commit -m "feat: add agentsview token provider"
```

---

## Task 4: Add Provider Contract Cutover And Cursor Seeding

**Files:**
- Create: `src/usage/cutover.rs`
- Modify: `src/storage/usage_store.rs`
- Modify: `src/commands/init.rs`
- Modify: `src/commands/status.rs`
- Modify: `src/commands/watch.rs`
- Modify: `tests/runtime_integration.rs`
- Modify: `tests/doctor_status.rs`

**Interfaces:**
- Consumes: `UsageProvider::snapshot_for_calibration`
- Produces: `UsageStore::is_token_contract_active(contract: &str) -> Result<bool>`
- Produces: `UsageStore::mark_token_contract_active(contract: &str, now: OffsetDateTime) -> Result<()>`
- Produces: `ensure_tokenmaxxing_contract_active(state, usage_store, provider, now) -> Result<CutoverOutcome>`

- [ ] **Step 1: Write failing migration fixture test**

Append to `tests/runtime_integration.rs`:

```rust
#[test]
fn tokenmaxxing_cutover_seeds_agentsview_cursors_without_feeding_existing_pet() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.xp = 0.25;
    state.lifetime_effective_tokens = 12_345.0;
    state.stage = glorp::game::evolution::Stage::S2;
    state.vitals.fed = 44.0;
    state.recent_events.push(glorp::storage::state::NarrativeEvent {
        observed_at: datetime!(2026-06-18 12:00 UTC),
        text: "existing event".into(),
    });
    usage_store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: "codex".into(),
                cursor_key: "old-ccusage-cursor".into(),
                cursor_value: "old".into(),
                provider_version: "ccusage-codex".into(),
                parser_version: "ccusage-codex".into(),
            }],
            datetime!(2026-06-18 12:00 UTC),
        )
        .unwrap();

    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/helpers/agentsview-ok.mjs")),
        },
    );

    let outcome = glorp::usage::cutover::ensure_tokenmaxxing_contract_active(
        &mut state,
        &mut usage_store,
        &provider,
        datetime!(2026-06-18 20:00 UTC),
    )
    .unwrap();

    assert_eq!(outcome.activated, true);
    assert_eq!(state.xp, 0.25);
    assert_eq!(state.lifetime_effective_tokens, 12_345.0);
    assert_eq!(state.stage, glorp::game::evolution::Stage::S2);
    assert_eq!(state.vitals.fed, 44.0);
    assert_eq!(state.recent_events.last().unwrap().text, "existing event");
    assert!(usage_store
        .is_token_contract_active(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1)
        .unwrap());

    let after_cutover_poll = provider.poll(&mut usage_store).unwrap();
    assert_eq!(after_cutover_poll.total_tokens, 0.0);
}
```

- [ ] **Step 2: Run the cutover test and verify failure**

Run:

```bash
cargo test --test runtime_integration tokenmaxxing_cutover_seeds_agentsview_cursors_without_feeding_existing_pet
```

Expected: compile failure because `usage::cutover` and contract-state methods do not exist.

- [ ] **Step 3: Add storage contract-state table**

In `UsageStore::migrate`, add:

```rust
            CREATE TABLE IF NOT EXISTS token_contract_state (
                token_contract TEXT PRIMARY KEY,
                activated_at TEXT NOT NULL
            );
```

Add to `impl UsageStore`:

```rust
    pub fn is_token_contract_active(&self, contract: &str) -> crate::error::Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM token_contract_state WHERE token_contract = ?1)",
                params![contract],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(Into::into)
    }

    pub fn mark_token_contract_active(
        &self,
        contract: &str,
        now: OffsetDateTime,
    ) -> crate::error::Result<()> {
        self.conn.execute(
            "INSERT INTO token_contract_state (token_contract, activated_at)
             VALUES (?1, ?2)
             ON CONFLICT(token_contract) DO UPDATE SET activated_at = excluded.activated_at",
            params![contract, format_time(now)?],
        )?;
        Ok(())
    }
```

- [ ] **Step 4: Implement cutover helper**

Create `src/usage/cutover.rs`:

```rust
use crate::error::Result;
use crate::game::{calibration::CalibrationBaseline, metabolism::RhythmProfile};
use crate::storage::{state::PetState, usage_store::UsageStore};
use crate::usage::provider::UsageProvider;
use crate::usage::token_contract::TOKENMAXXING_TOTAL_V1;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutoverOutcome {
    pub activated: bool,
}

pub fn ensure_tokenmaxxing_contract_active(
    state: &mut PetState,
    usage_store: &mut UsageStore,
    provider: &impl UsageProvider,
    now: OffsetDateTime,
) -> Result<CutoverOutcome> {
    if usage_store.is_token_contract_active(TOKENMAXXING_TOTAL_V1)? {
        return Ok(CutoverOutcome { activated: false });
    }

    let snapshot = provider.snapshot_for_calibration(usage_store)?;
    if !snapshot.daily_usage.is_empty() {
        state.calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
        state.rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
    }
    usage_store.advance_cursors(snapshot.cursor_updates, now)?;
    usage_store.mark_token_contract_active(TOKENMAXXING_TOTAL_V1, now)?;

    Ok(CutoverOutcome { activated: true })
}
```

- [ ] **Step 5: Export the cutover module**

Add this line to `src/usage/mod.rs`:

```rust
pub mod cutover;
```

- [ ] **Step 6: Wire cutover into command flows**

In `src/commands/init.rs`, replace `CcusageCommandProvider` construction with:

```rust
        let provider = crate::usage::agentsview::AgentsviewCommandProvider::from_environment();
        if let Ok(snapshot) = provider.snapshot_for_calibration(&mut usage_store) {
            calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
            rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
            usage_store.advance_cursors(snapshot.cursor_updates, OffsetDateTime::now_utc())?;
            usage_store.mark_token_contract_active(
                crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                OffsetDateTime::now_utc(),
            )?;
        }
```

In `src/commands/status.rs` and `src/commands/watch.rs`, before `provider.poll`, call:

```rust
let provider = crate::usage::agentsview::AgentsviewCommandProvider::from_environment();
let cutover = crate::usage::cutover::ensure_tokenmaxxing_contract_active(
    &mut state,
    &mut usage_store,
    &provider,
    OffsetDateTime::now_utc(),
)?;
if cutover.activated {
    store.save(&state)?;
}
let result = provider.poll(&mut usage_store)?;
```

Use the existing state-store variable names in each command file.

- [ ] **Step 7: Run focused cutover tests**

Run:

```bash
cargo test --test runtime_integration tokenmaxxing_cutover_seeds_agentsview_cursors_without_feeding_existing_pet
cargo test --test doctor_status status_surfaces_first_contact_without_claiming_blocked
```

Expected: cutover test passes. Update the first-contact status test in Task 6 if it now expects Tokenmaxxing wording.

- [ ] **Step 8: Commit**

```bash
git add src/usage/cutover.rs src/storage/usage_store.rs src/commands/init.rs src/commands/status.rs src/commands/watch.rs tests/runtime_integration.rs tests/doctor_status.rs
git commit -m "feat: seed tokenmaxxing provider contract"
```

---

## Task 5: Route Runtime Feeding Through Canonical Total Tokens

**Files:**
- Modify: `src/game/runtime.rs`
- Modify: `src/game/calibration.rs`
- Modify: `src/game/catchup.rs`
- Modify: `src/game/metabolism.rs`
- Modify: `tests/runtime_integration.rs`
- Modify: `tests/activity_identity_cursors.rs`

**Interfaces:**
- Consumes: `UsageDelta.total_tokens`
- Produces: runtime rows whose `effective_tokens` and `total_tokens` match for Tokenmaxxing rows
- Keeps: `PetState.lifetime_effective_tokens` serialized name for this pass, storing canonical total tokens after cutover

- [ ] **Step 1: Write failing runtime feeding test**

Append to `tests/runtime_integration.rs`:

```rust
#[test]
fn runtime_feeds_cached_tokens_at_full_value_for_tokenmaxxing_deltas() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 1_000_000.0;
    establish_provider_contact(&mut usage_store, "codex", datetime!(2026-06-18 20:00 UTC));

    let now = datetime!(2026-06-18 20:10 UTC);
    let poll = UsagePollResult {
        deltas: vec![UsageDelta {
            provider_surface: "codex".into(),
            source_identity: SourceIdentity::from_tokenmaxxing_source("codex"),
            command: glorp::usage::agentsview::AGENTSVIEW_COMMAND.into(),
            effective_tokens: 700_000_000.0,
            total_tokens: 700_000_000.0,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            confidence: "local-log-derived".into(),
            period_start: datetime!(2026-06-18 07:00 UTC),
            observed_at: now,
            model: Some("gpt-5.5".into()),
            cursor_update: ProviderCursorUpdate {
                provider_surface: "codex".into(),
                cursor_key: "codex-tokenmaxxing".into(),
                cursor_value: "v1".into(),
                provider_version: "agentsview v0.32.1".into(),
                parser_version: "agentsview v0.32.1".into(),
            },
            token_totals: Some(RawTokenTotals {
                uncached_input: 1_000,
                output: 2_000,
                cache_creation: 3_000,
                cache_read: 699_994_000,
                reasoning_output: 123_456,
            }),
        }],
        diagnostics: Vec::new(),
        total_effective_tokens: 700_000_000.0,
        total_tokens: 700_000_000.0,
    };

    let update = apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();

    assert_eq!(update.recent_effective_tokens, 700_000_000.0);
    assert_eq!(state.lifetime_effective_tokens, 700_000_000.0);
    let rows = usage_store.recent_events(20).unwrap();
    assert_eq!(rows.iter().map(|row| row.total_tokens).sum::<f64>(), 700_000_000.0);
    assert_eq!(
        rows.iter().map(|row| row.reasoning_output_tokens).sum::<f64>(),
        123_456.0
    );
}
```

- [ ] **Step 2: Run the runtime test and verify failure**

Run:

```bash
cargo test --test runtime_integration runtime_feeds_cached_tokens_at_full_value_for_tokenmaxxing_deltas
```

Expected: failure if runtime still derives food from weighted effective fields or misses `total_tokens` in row storage.

- [ ] **Step 3: Use `delta.total_tokens` in staging and guards**

In `src/game/runtime.rs`, update these calculations:

```rust
*surface_sums.entry(delta.provider_surface.clone()).or_insert(0.0) += delta.total_tokens.max(0.0);
let buckets = crate::game::catchup::smear_catchup_delta(delta.total_tokens, baseline);
```

In `event_for_delta`, set:

```rust
        effective_tokens: delta.total_tokens,
        total_tokens: delta.total_tokens,
        token_contract: delta.token_contract.clone(),
```

Inside the smear loop, after `event.effective_tokens = effective_tokens;`, add:

```rust
            event.total_tokens = effective_tokens;
```

In `seed_first_contact_surface`, set:

```rust
            effective_tokens: delta.total_tokens,
            total_tokens: delta.total_tokens,
            token_contract: delta.token_contract.clone(),
```

- [ ] **Step 4: Keep live signal naming but canonicalize values**

In `apply_unapplied_usage`, continue using `row.event.effective_tokens` for this pass only if all Tokenmaxxing rows mirror `total_tokens`. Add this comment above `recent_effective_tokens`:

```rust
    // Transitional naming: for tokenmaxxing_total_v1 rows, effective_tokens is
    // deliberately equal to canonical total_tokens so old presentation structs
    // can keep their field names while the product contract has changed.
```

Add assertions in the runtime test that Tokenmaxxing rows have equal fields:

```rust
assert!(rows.iter().all(|row| {
    row.token_contract == glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1
        && row.effective_tokens == row.total_tokens
}));
```

- [ ] **Step 5: Run runtime tests**

Run:

```bash
cargo test --test runtime_integration runtime_feeds_cached_tokens_at_full_value_for_tokenmaxxing_deltas
cargo test --test runtime_integration
cargo test --test activity_identity_cursors
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/game/runtime.rs src/game/calibration.rs src/game/catchup.rs src/game/metabolism.rs tests/runtime_integration.rs tests/activity_identity_cursors.rs
git commit -m "feat: feed pet from canonical total tokens"
```

---

## Task 6: Move Watch And Status To Tokenmaxxing Totals And Source Labels

**Files:**
- Modify: `src/storage/usage_store.rs`
- Modify: `src/commands/status.rs`
- Modify: `src/commands/watch.rs`
- Modify: `src/tui/view_model.rs`
- Modify: `src/tui/life.rs`
- Modify: `src/tui/panels/today.rs`
- Modify: `tests/watch_integration.rs`
- Modify: `tests/doctor_status.rs`

**Interfaces:**
- Consumes: `tokenmaxxing_today_window`
- Consumes: canonical total-token query methods
- Produces: status output line `tokens (local-log-derived): today ...`
- Produces: watch source rows named `claude` and `codex`

- [ ] **Step 1: Write failing status copy test**

Modify `status_is_pipe_friendly_when_pet_exists` in `tests/doctor_status.rs`:

```rust
.stdout(predicate::str::contains("tokens"))
.stdout(predicate::str::contains("effective tokens").not())
```

Modify `status_clamps_zero_usage_display` expected output:

```rust
.stdout(predicate::str::contains(
    "tokens (estimated): today 0 recent 0 lifetime 0",
))
```

- [ ] **Step 2: Write failing watch day-axis/source-label test**

Append to `tests/watch_integration.rs`:

```rust
#[test]
fn watch_token_totals_use_tokenmaxxing_day_axis_and_external_source_labels() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026-06-19 06:30 UTC); // 2026-06-18 23:30 in Los Angeles
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".into(),
            period_start: datetime!(2026-06-18 07:00 UTC),
            observed_at: now,
            bucket_at: now,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            total_tokens: 669_369_020.0,
            effective_tokens: 669_369_020.0,
            ..NormalizedUsageEvent::for_test_at(now, 669_369_020.0)
        })
        .unwrap();

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(vm.today_effective_tokens, 669_369_020.0);
    assert!(vm
        .source_breakdown
        .iter()
        .any(|source| source.name == "codex" && source.effective_tokens == 669_369_020.0));
}
```

- [ ] **Step 3: Run focused UI tests and verify failure**

Run:

```bash
cargo test --test doctor_status status_is_pipe_friendly_when_pet_exists
cargo test --test doctor_status status_clamps_zero_usage_display
cargo test --test watch_integration watch_token_totals_use_tokenmaxxing_day_axis_and_external_source_labels
```

Expected: status copy and watch query tests fail until callers switch to canonical totals and Tokenmaxxing window.

- [ ] **Step 4: Switch status totals and copy**

In `src/commands/status.rs`, replace the local-day mapper block with:

```rust
let status_now = OffsetDateTime::now_utc();
let (today_start, today_end) = crate::usage::day_axis::tokenmaxxing_today_window(status_now);
today_effective = usage_store
    .canonical_total_tokens_between(today_start, today_end)
    .unwrap_or(0.0);
today_sources = usage_store
    .canonical_total_tokens_by_source_between(today_start, today_end)
    .unwrap_or_default();
```

Replace the printed label:

```rust
println!(
    "tokens ({usage_confidence}): today {:.0} recent {:.0} lifetime {:.0}",
    display_tokens(today_effective),
    display_tokens(recent_effective),
    display_tokens(state.lifetime_effective_tokens)
);
```

- [ ] **Step 5: Switch watch totals**

In `src/commands/watch.rs`, replace today-token and current-bucket total queries with canonical queries:

```rust
let (today_start, today_end) = crate::usage::day_axis::tokenmaxxing_today_window(now);
let today_totals = usage_store
    .canonical_total_tokens_by_source_between(today_start, today_end)
    .unwrap_or_default();
let last_10m_totals = usage_store
    .canonical_total_tokens_by_source_between(last_10m_start, window_end)
    .unwrap_or_default();
let today_total_tokens: f64 = today_totals.iter().map(|(_, v)| *v).sum();
let last_10m_total_tokens: f64 = last_10m_totals.iter().map(|(_, v)| *v).sum();
```

Keep `LocalDayMapper` for `day_context`, pet activity timestamps, birth labels, and sleep/ambiance behavior.

- [ ] **Step 6: Update transitional comments in view model types**

In `src/tui/view_model.rs`, add this comment above `today_effective_tokens`:

```rust
    /// Transitional field name. Values are canonical Tokenmaxxing total tokens
    /// for tokenmaxxing_total_v1 rows.
```

Add the same comment above `SourceUsageView.effective_tokens`, `SourceHealthView.today_effective_tokens`, and `SourceHealthView.bucket_effective_tokens`.

- [ ] **Step 7: Run UI tests**

Run:

```bash
cargo test --test doctor_status
cargo test --test watch_integration
cargo test --test tui_render
```

Expected: pass after fixture expected strings are updated to `tokens`.

- [ ] **Step 8: Commit**

```bash
git add src/storage/usage_store.rs src/commands/status.rs src/commands/watch.rs src/tui/view_model.rs src/tui/life.rs src/tui/panels/today.rs tests/watch_integration.rs tests/doctor_status.rs
git commit -m "feat: show tokenmaxxing totals in status and watch"
```

---

## Task 7: Doctor, Helper Locator, Privacy, And Packaging

**Files:**
- Modify: `src/usage/helper_locator.rs`
- Modify: `tests/helper_locator.rs`
- Modify: `src/commands/doctor.rs`
- Modify: `tests/doctor_status.rs`
- Modify: `npm/glorp/bin/glorp.js`
- Modify: `npm/glorp/test/smoke.mjs`
- Modify: `tests/acceptance_matrix.rs`
- Modify: `README.md`
- Modify: `npm/glorp/README.md`
- Modify: `docs/superpowers/stories/story-001-usage-provider-ccusage.md`
- Modify: `docs/superpowers/stories/story-004-effective-token-model.md`
- Modify: `docs/superpowers/stories/story-010-npm-rust-packaging.md`

**Interfaces:**
- Consumes: `GLORP_AGENTSVIEW_BIN`
- Produces: doctor output that says whether Tokenmaxxing provider is available
- Produces: docs that no longer claim `ccusage` is the normal provider

- [ ] **Step 1: Write failing helper locator test**

Modify `tests/helper_locator.rs` locator construction:

```rust
let locator = HelperLocator {
    agentsview_bin: Some(dir.path().join("agentsview/bin/agentsview")),
    ccusage_bin: Some(dir.path().join("ccusage/bin/helper.js")),
    ccusage_codex_bin: Some(dir.path().join("ccusage-codex/bin/helper.js")),
    node_bin: Some(dir.path().join("node/bin/node")),
};
```

Add:

```rust
#[test]
fn helper_locator_reads_agentsview_env_path() {
    let dir = tempfile::tempdir().unwrap();
    let agentsview = dir.path().join("agentsview");
    std::env::set_var("GLORP_AGENTSVIEW_BIN", &agentsview);
    let locator = HelperLocator::from_current_environment();
    std::env::remove_var("GLORP_AGENTSVIEW_BIN");

    assert_eq!(locator.agentsview_bin.as_deref(), Some(agentsview.as_path()));
}
```

- [ ] **Step 2: Implement locator field**

Modify `HelperLocator` in `src/usage/helper_locator.rs`:

```rust
pub struct HelperLocator {
    pub agentsview_bin: Option<PathBuf>,
    pub ccusage_bin: Option<PathBuf>,
    pub ccusage_codex_bin: Option<PathBuf>,
    pub node_bin: Option<PathBuf>,
}
```

Modify `from_current_environment`:

```rust
            agentsview_bin: std::env::var_os("GLORP_AGENTSVIEW_BIN").map(PathBuf::from),
```

Modify `has_any_path`:

```rust
self.agentsview_bin.is_some()
    || self.ccusage_bin.is_some()
    || self.ccusage_codex_bin.is_some()
    || self.node_bin.is_some()
```

- [ ] **Step 3: Write failing doctor tests**

Append to `tests/doctor_status.rs`:

```rust
#[test]
fn doctor_reports_agentsview_provider_as_tokenmaxxing_compatible() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", "tests/fixtures/helpers/agentsview-ok.mjs")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("agentsview"))
        .stdout(predicate::str::contains("Tokenmaxxing-compatible: yes"))
        .stdout(predicate::str::contains("agentsview v0.32.1"));
}

#[test]
fn doctor_reports_missing_agentsview_as_canonical_provider_blocked() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Tokenmaxxing-compatible: no"))
        .stdout(predicate::str::contains("agentsview helper was not found"))
        .stdout(predicate::str::contains("GLORP_AGENTSVIEW_BIN"));
}
```

- [ ] **Step 4: Update doctor implementation**

In `src/commands/doctor.rs`, construct `AgentsviewCommandProvider::from_environment()` instead of `CcusageCommandProvider::from_environment()` for canonical health. Print:

```rust
println!("provider: agentsview");
println!("Tokenmaxxing-compatible: yes");
```

when no agentsview diagnostics are returned. For missing helper, print:

```rust
println!("Tokenmaxxing-compatible: no");
println!("Canonical provider blocked.");
println!("Install agentsview or set GLORP_AGENTSVIEW_BIN to its executable path.");
```

Keep a short legacy section if `ccusage` versions exist in `provider_cursors`:

```rust
println!("legacy provider data: present");
```

- [ ] **Step 5: Update npm smoke test for external agentsview**

In `npm/glorp/test/smoke.mjs`, include `agentsview` in the fake native env log:

```javascript
agentsview: process.env.GLORP_AGENTSVIEW_BIN,
```

Add a run:

```javascript
const externalAgentsview = path.join(tempRoot, process.platform === "win32" ? "agentsview.cmd" : "agentsview");
fs.writeFileSync(externalAgentsview, process.platform === "win32" ? "@echo agentsview fixture\r\n" : "#!/bin/sh\necho agentsview fixture\n");
if (process.platform !== "win32") fs.chmodSync(externalAgentsview, 0o755);

const withAgentsview = run(["doctor"], {
  GLORP_AGENTSVIEW_BIN: externalAgentsview,
  GLORP_CONFIG_DIR: path.join(tempRoot, "config-agentsview")
});
assert.equal(withAgentsview.status, 0, withAgentsview.stderr);
const agentsviewEnv = JSON.parse(fs.readFileSync(envLog, "utf8"));
assert.equal(agentsviewEnv.agentsview, externalAgentsview);
```

- [ ] **Step 6: Update acceptance matrix**

In `tests/acceptance_matrix.rs`, replace checks that `ccusage` is the normal provider with:

```rust
let provider = read("src/usage/agentsview.rs");
assert!(provider.contains("GLORP_AGENTSVIEW_BIN"));
assert!(provider.contains("which::which(\"agentsview\")"));
assert!(provider.contains("--timezone"));
assert!(provider.contains("America/Los_Angeles"));

let readme = read("README.md");
assert!(readme.contains("agentsview"));
assert!(readme.contains("GLORP_AGENTSVIEW_BIN"));
assert!(!readme.contains("Glorp polls `ccusage` and `ccusage-codex` every ten seconds"));
```

Leave checks that the npm wrapper still wires ccusage only if README/stories identify ccusage as legacy diagnostics.

- [ ] **Step 7: Update docs**

In `README.md` and `npm/glorp/README.md`, replace normal-provider text with:

```markdown
Glorp's canonical usage provider is `agentsview`. Install it separately and make sure `agentsview` is on `PATH`, or set `GLORP_AGENTSVIEW_BIN` to the executable path. Glorp counts cached input fully so its visible totals match Tokenmaxxing-style token totals.
```

In the environment variable table, add:

```markdown
| `GLORP_AGENTSVIEW_BIN` | Pin a specific `agentsview` binary for canonical Tokenmaxxing-compatible usage. |
```

Mark `cache_read_weight` as legacy:

```markdown
`cache_read_weight` is accepted for older local config files but no longer affects canonical pet progression.
```

In `story-001`, `story-004`, and `story-010`, add a top note:

```markdown
> Legacy note: current canonical usage accounting is Tokenmaxxing-compatible `agentsview` total tokens. This story describes the original ccusage/effective-token MVP behavior.
```

- [ ] **Step 8: Run packaging and doctor tests**

Run:

```bash
cargo test --test helper_locator
cargo test --test doctor_status
cargo test --test acceptance_matrix
npm test
```

Expected: pass.

- [ ] **Step 9: Commit**

```bash
git add src/usage/helper_locator.rs tests/helper_locator.rs src/commands/doctor.rs tests/doctor_status.rs npm/glorp/bin/glorp.js npm/glorp/test/smoke.mjs tests/acceptance_matrix.rs README.md npm/glorp/README.md docs/superpowers/stories/story-001-usage-provider-ccusage.md docs/superpowers/stories/story-004-effective-token-model.md docs/superpowers/stories/story-010-npm-rust-packaging.md
git commit -m "feat: report agentsview token provider health"
```

---

## Task 8: Regression Sweep And Preview-Safe Cleanup

**Files:**
- Modify: `tests/cli_smoke.rs`
- Modify: `tests/watch_integration.rs`
- Modify: `tests/storage_privacy.rs`
- Modify: `tests/usage_provider.rs`
- Modify: `docs/superpowers/specs/2026-06-19-glorp-tokenmaxxing-token-contract-design.md`

**Interfaces:**
- Consumes: all earlier tasks
- Produces: final verification evidence for implementation

- [ ] **Step 1: Add Drew regression fixture assertion**

Append to `tests/usage_provider.rs`:

```rust
#[test]
fn drew_2026_06_18_tokenmaxxing_fixture_totals_are_documented() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/agentsview-drew-2026-06-18-tokenmaxxing.json");
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture_path).unwrap()).unwrap();

    assert_eq!(value["sources"]["claude"].as_u64().unwrap(), 46_011_892);
    assert_eq!(value["sources"]["codex"].as_u64().unwrap(), 669_369_020);
    assert_eq!(value["total"].as_u64().unwrap(), 715_380_912);
}
```

- [ ] **Step 2: Add non-Los-Angeles process TZ status test**

Append to `tests/doctor_status.rs`:

```rust
#[test]
fn status_uses_tokenmaxxing_day_axis_under_non_los_angeles_tz() {
    let dir = tempdir().unwrap();
    write_state_for_test(dir.path(), PetStateFixture::named("mochi")).unwrap();
    let now = OffsetDateTime::now_utc();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".into(),
            observed_at: now,
            bucket_at: now,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            total_tokens: 123_456.0,
            effective_tokens: 123_456.0,
            ..NormalizedUsageEvent::for_test_at(now, 123_456.0)
        })
        .unwrap();
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("TZ", "UTC")
        .env("GLORP_AGENTSVIEW_BIN", "tests/fixtures/helpers/agentsview-fails.mjs")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("tokens"))
        .stdout(predicate::str::contains("effective tokens").not());
}
```

- [ ] **Step 3: Run full relevant verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --test usage_provider
cargo test --test runtime_integration
cargo test --test watch_integration
cargo test --test doctor_status
cargo test --test storage_privacy
cargo test --test helper_locator
cargo test --test acceptance_matrix
cargo test --test cli_smoke
npm test
cargo run -- status
```

Expected: all commands pass. `cargo run -- status` should print `tokens`, `provider: local-log-derived` or a clear canonical provider block, and no visible `effective tokens` label.

- [ ] **Step 4: Run Preview Lab only if watch text snapshots changed**

Run when watch/status text changes affect preview artifacts:

```bash
cargo run -- dev-preview --scenario watch --out target/glorp-preview
cargo test --features dev-preview --test dev_preview
```

Expected: Preview Lab renders successfully and watch frames do not have overlapping text.

- [ ] **Step 5: Update spec status**

In `docs/superpowers/specs/2026-06-19-glorp-tokenmaxxing-token-contract-design.md`, change the status line to:

```markdown
- Status: implemented and verified
```

Only make this edit after the full verification in Step 3 passes.

- [ ] **Step 6: Final commit**

```bash
git add tests/cli_smoke.rs tests/watch_integration.rs tests/storage_privacy.rs tests/usage_provider.rs docs/superpowers/specs/2026-06-19-glorp-tokenmaxxing-token-contract-design.md
git commit -m "test: cover tokenmaxxing token contract"
```

---

## Execution Notes

- Prefer subagent-driven execution. The tasks have mostly disjoint ownership and each has its own tests.
- Do not rename every `effective_*` symbol during implementation. The planned behavior change is already large; deep naming cleanup belongs in a follow-up once canonical totals are green.
- Treat `ccusage` as legacy diagnostic data after Task 4. It should not become an automatic fallback food source again.
- When a test fixture must assert exact token totals, use checked-in fixture JSON rather than the live public Tokenmaxxing API or current local `agentsview` output.
- If the local `agentsview` output differs from the captured public profile, Glorp should match the live local collector for local behavior and keep the captured profile fixture as a historical regression contract.

## Self-Review Checklist

- Spec coverage: collector, cache math, progression, cutover, day axis, storage contract, packaging, privacy, status/watch, and docs are each mapped to a task.
- Placeholder scan: no open implementation questions are intentionally left in the task steps.
- Type consistency: `total_tokens` and `token_contract` are added at provider and storage boundaries before runtime and UI tasks consume them.
- Commit flow: each task ends with focused tests and an explicit commit.
