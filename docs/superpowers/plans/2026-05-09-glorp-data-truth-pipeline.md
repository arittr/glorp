# Glorp Data Truth Pipeline Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every newly observed provider delta durable, idempotent, correctly weighted, correctly bucketed, and applied to pet state exactly once.

**Architecture:** Keep `CcusageCommandProvider` as the source adapter, but move it toward diff-only output: it reads provider cursors, computes stable deltas, and returns cursor updates without applying pet state. `UsageStore` becomes the durable ledger for unapplied usage events; runtime applies unapplied ledger rows, the command saves pet state, and only then the store marks rows applied and advances provider cursors. Event-time fields separate source period time from pet-food observation and display-bucket time.

**Tech Stack:** Rust 2021, `rusqlite`, `serde`, `time`, existing `ccusage`/`@ccusage/codex` fixtures, `assert_cmd`, `tempfile`.

---

## Source Material

- Spec: `docs/superpowers/specs/2026-05-09-glorp-core-mvp-repair-design.md`
- Stories: `docs/superpowers/stories/story-001-usage-provider-ccusage.md`, `story-002-local-persistence.md`, `story-004-effective-token-model.md`, `story-005-calibration-and-evolution.md`, `story-009-status-doctor-and-errors.md`
- Current code seams:
  - `src/usage/ccusage.rs` writes provider cursors before `UsageStore::insert_event`.
  - `src/storage/usage_store.rs` stores only `period_start`, and `insert_event` always increments `lifetime_counters`.
  - `src/game/runtime.rs` applies `poll.deltas` directly and cannot distinguish unapplied durable food from transient provider output.
  - `src/commands/init.rs` polls helpers for calibration through the same path as runtime feeding.

## File Structure

Modify:

- `src/storage/usage_store.rs`: event-time columns, unapplied ledger methods, idempotent provider delta insertion, applied-row marking, cursor advancement.
- `src/usage/provider.rs`: provider delta shape, stable cursor key shape, cursor update payloads.
- `src/usage/ccusage.rs`: configured weights, version-stable cursor keys, diff-only positive delta handling, calibration snapshot support.
- `src/game/runtime.rs`: apply unapplied ledger rows rather than transient `poll.deltas`; return applied event IDs and cursor updates for post-save marking.
- `src/game/calibration.rs`: group historical rows by active day before limiting and median calculation.
- `src/game/evolution.rs`: expose calibrated XP helper already present; keep existing stage thresholds.
- `src/commands/init.rs`: calibrate from helper snapshot without feeding or persisting pet-food events.
- `src/commands/status.rs`: load config weights and use the same runtime apply boundary as watch.
- `src/commands/watch.rs`: load config weights and use the same runtime apply boundary as status.
- `tests/usage_provider.rs`: configured weight, stable cursor, idempotent provider diff tests.
- `tests/storage_privacy.rs`: migration, event-time, unapplied ledger, privacy tests.
- `tests/runtime_integration.rs`: state-save failure simulation, catch-up smear, duplicate-transition tests.
- `tests/game_rules.rs`: calibration grouping and 49-day catch-up acceptance tests.

Create:

- `src/game/catchup.rs`: split newly observed deltas into display/metabolism buckets.
- `tests/fixtures/helpers/ccusage-ok-v2.mjs`: same usage payload as `ccusage-ok.mjs`, different helper version.

---

## Task 1: Event-Time Storage And Durable Usage Ledger

**Files:**
- Modify: `src/storage/usage_store.rs`
- Test: `tests/storage_privacy.rs`

- [ ] **Step 1: Write failing storage tests for event-time fields**

Append these tests to `tests/storage_privacy.rs`:

```rust
use rusqlite::Connection;
use time::macros::datetime;

#[test]
fn usage_events_store_observed_and_bucket_times_separately_from_period_start() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let period_start = datetime!(2026-05-09 00:00 UTC);
    let observed_at = datetime!(2026-05-09 19:17 UTC);
    let bucket_at = datetime!(2026-05-09 19:10 UTC);

    store
        .insert_event(&NormalizedUsageEvent {
            observed_at,
            bucket_at,
            ..NormalizedUsageEvent::for_test_at(period_start, 420.0)
        })
        .unwrap();

    let events = store.recent_events(1).unwrap();
    assert_eq!(events[0].period_start, period_start);
    assert_eq!(events[0].observed_at, observed_at);
    assert_eq!(events[0].bucket_at, bucket_at);
}

#[test]
fn old_usage_rows_migrate_with_conservative_event_times() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("usage.sqlite");
    {
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE usage_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_surface TEXT NOT NULL,
                provider_version TEXT NOT NULL,
                parser_version TEXT NOT NULL,
                command TEXT NOT NULL,
                source_surface TEXT NOT NULL,
                period_start TEXT NOT NULL,
                period_date TEXT NOT NULL,
                model TEXT,
                input_tokens REAL NOT NULL,
                output_tokens REAL NOT NULL,
                cache_creation_tokens REAL NOT NULL,
                cache_read_tokens REAL NOT NULL,
                reasoning_output_tokens REAL NOT NULL,
                effective_tokens REAL NOT NULL,
                cost_usd REAL,
                confidence TEXT NOT NULL
            );
            INSERT INTO usage_events (
                provider_surface, provider_version, parser_version, command,
                source_surface, period_start, period_date, model,
                input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                reasoning_output_tokens, effective_tokens, cost_usd, confidence
            ) VALUES (
                'claude-code', '18.0.11', '18.0.11', 'ccusage',
                'daily', '2026-05-08T00:00:00Z', '2026-05-08', 'claude-opus-4',
                1, 2, 3, 4, 0, 6, NULL, 'local-log-derived'
            );
            ",
        )
        .unwrap();
    }

    let store = UsageStore::open(&db).unwrap();
    let events = store.recent_events(5).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].observed_at, datetime!(2026-05-08 00:00 UTC));
    assert_eq!(events[0].bucket_at, datetime!(2026-05-08 00:00 UTC));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test usage_events_store_observed_and_bucket_times_separately_from_period_start old_usage_rows_migrate_with_conservative_event_times
```

Expected: compile failure because `NormalizedUsageEvent` has no `observed_at` or `bucket_at`.

- [ ] **Step 3: Add event-time fields and migration helpers**

In `src/storage/usage_store.rs`, extend `NormalizedUsageEvent`:

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
    pub cost_usd: Option<f64>,
    pub confidence: String,
}
```

Update `for_test_at` to set `observed_at` and `bucket_at` to `period_start`.

Add migration helpers near `migrate`:

```rust
fn column_exists(conn: &Connection, table: &str, column: &str) -> crate::error::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_usage_event_column(
    conn: &Connection,
    name: &str,
    definition: &str,
    backfill_sql: &str,
) -> crate::error::Result<()> {
    if !column_exists(conn, "usage_events", name)? {
        conn.execute_batch(definition)?;
        conn.execute_batch(backfill_sql)?;
    }
    Ok(())
}
```

In `migrate`, after the `CREATE TABLE IF NOT EXISTS usage_events` block, call:

```rust
ensure_usage_event_column(
    &self.conn,
    "observed_at",
    "ALTER TABLE usage_events ADD COLUMN observed_at TEXT;",
    "UPDATE usage_events SET observed_at = period_start WHERE observed_at IS NULL;",
)?;
ensure_usage_event_column(
    &self.conn,
    "bucket_at",
    "ALTER TABLE usage_events ADD COLUMN bucket_at TEXT;",
    "UPDATE usage_events SET bucket_at = period_start WHERE bucket_at IS NULL;",
)?;
```

Then rebuild the table if needed or validate non-null values before reads. For the current local schema, the backfill is enough because query methods always parse populated values.

- [ ] **Step 4: Include event-time fields in inserts and reads**

Update `insert_event` SQL to include `observed_at` and `bucket_at`, and update `recent_events` to select and parse them. Order recent events by `observed_at DESC, id DESC`.

Use this query shape:

```sql
SELECT
    provider_surface,
    provider_version,
    parser_version,
    command,
    source_surface,
    period_start,
    observed_at,
    bucket_at,
    model,
    input_tokens,
    output_tokens,
    cache_creation_tokens,
    cache_read_tokens,
    reasoning_output_tokens,
    effective_tokens,
    cost_usd,
    confidence
FROM usage_events
ORDER BY observed_at DESC, id DESC
LIMIT ?1
```

- [ ] **Step 5: Run focused storage tests**

Run:

```bash
cargo test usage_events_store_observed_and_bucket_times_separately_from_period_start old_usage_rows_migrate_with_conservative_event_times normalized_usage_storage_never_persists_transcript_payloads
```

Expected: all named tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/storage/usage_store.rs tests/storage_privacy.rs
git commit -m "feat: add usage event time fields"
```

---

## Task 2: Stable Provider Deltas, Configured Weights, And Cursor Updates

**Files:**
- Modify: `src/usage/provider.rs`
- Modify: `src/usage/ccusage.rs`
- Modify: `src/commands/status.rs`
- Modify: `src/commands/watch.rs`
- Create: `tests/fixtures/helpers/ccusage-ok-v2.mjs`
- Test: `tests/usage_provider.rs`

- [ ] **Step 1: Write failing provider tests**

Append these tests to `tests/usage_provider.rs`:

```rust
use glorp::game::effective_tokens::EffectiveTokenWeights;

#[test]
fn provider_uses_configured_cache_read_weight_for_real_deltas() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = CcusageCommandProvider::new(HelperPaths {
        claude: Some(fixture("ccusage-ok.mjs")),
        codex: None,
        node: None,
    })
    .with_weights(EffectiveTokenWeights {
        cache_read_weight: 0.05,
    });

    provider.poll(&mut store).unwrap();
    let next_provider = CcusageCommandProvider::new(HelperPaths {
        claude: Some(fixture("ccusage-next.mjs")),
        codex: None,
        node: None,
    })
    .with_weights(EffectiveTokenWeights {
        cache_read_weight: 0.05,
    });

    let second = next_provider.poll(&mut store).unwrap();
    assert_eq!(second.total_effective_tokens, 1500.0);
}

#[test]
fn helper_version_change_does_not_create_new_food_for_same_totals() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let first = provider(Some("ccusage-ok.mjs"), None)
        .poll(&mut store)
        .unwrap();
    assert!(first.total_effective_tokens > 0.0);

    let second = provider(Some("ccusage-ok-v2.mjs"), None)
        .poll(&mut store)
        .unwrap();
    assert_eq!(second.total_effective_tokens, 0.0);
}
```

Create `tests/fixtures/helpers/ccusage-ok-v2.mjs`:

```javascript
#!/usr/bin/env node
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("ccusage 18.0.99");
  process.exit(0);
}
await import("./ccusage-ok.mjs");
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test provider_uses_configured_cache_read_weight_for_real_deltas helper_version_change_does_not_create_new_food_for_same_totals
```

Expected: compile failure for missing `with_weights`, or an assertion failure showing default `0.03` weighting and version-sensitive cursor keys.

- [ ] **Step 3: Make provider weights configurable**

Change `CcusageCommandProvider`:

```rust
pub struct CcusageCommandProvider {
    helpers: HelperPaths,
    weights: EffectiveTokenWeights,
}

impl CcusageCommandProvider {
    pub fn new(helpers: HelperPaths) -> Self {
        Self {
            helpers,
            weights: EffectiveTokenWeights::default(),
        }
    }

    pub fn with_weights(mut self, weights: EffectiveTokenWeights) -> Self {
        self.weights = weights;
        self
    }

    pub fn from_environment_with_weights(weights: EffectiveTokenWeights) -> Self {
        Self::new(HelperDiscovery::discover().into()).with_weights(weights)
    }
}
```

Replace `let weights = EffectiveTokenWeights::default();` in `poll_helper` with `let weights = self.weights;`.

In `status.rs` and `watch.rs`, load the app config and construct the provider with `EffectiveTokenWeights::from_config(config)`:

```rust
let config = crate::config::AppConfig::load_or_default(&paths.config_file)?;
let weights = crate::game::effective_tokens::EffectiveTokenWeights::from_config(config);
let result = CcusageCommandProvider::from_environment_with_weights(weights).poll(&mut usage_store)?;
```

- [ ] **Step 4: Remove parser version from logical cursor keys**

Change `ProviderCursorKey` in `src/usage/provider.rs`:

```rust
pub struct ProviderCursorKey {
    pub provider_surface: String,
    pub command: String,
    pub source_surface: String,
    pub period_start: String,
    pub model: Option<String>,
}
```

When building keys in `ccusage.rs`, set `source_surface: "daily".to_string()` and do not include `parser_version`.

Add a legacy lookup helper so existing local databases do not replay old totals after upgrade:

```rust
fn legacy_cursor_key_with_parser_version(
    provider_surface: &str,
    command: &str,
    parser_version: &str,
    period_start: &str,
    model: Option<String>,
) -> Result<String> {
    #[derive(serde::Serialize)]
    struct LegacyKey {
        provider_surface: String,
        command: String,
        parser_version: String,
        period_start: String,
        model: Option<String>,
    }
    serde_json::to_string(&LegacyKey {
        provider_surface: provider_surface.to_string(),
        command: command.to_string(),
        parser_version: parser_version.to_string(),
        period_start: period_start.to_string(),
        model,
    })
    .map_err(GlorpError::from)
}
```

On lookup, try the new key first; if absent, try the legacy key for the current helper version and copy the cursor value to the new key without emitting food.

- [ ] **Step 5: Run focused provider tests**

Run:

```bash
cargo test usage_provider
```

Expected: all provider tests pass, including the existing increment test updated to expect `1300.0` under default weights and the new configured-weight test expecting `1500.0`.

- [ ] **Step 6: Commit**

```bash
git add src/usage/provider.rs src/usage/ccusage.rs src/commands/status.rs src/commands/watch.rs tests/usage_provider.rs tests/fixtures/helpers/ccusage-ok-v2.mjs
git commit -m "feat: stabilize usage provider deltas"
```

---

## Task 3: Ledger Apply Boundary And State-Save Failure Safety

**Files:**
- Modify: `src/storage/usage_store.rs`
- Modify: `src/usage/provider.rs`
- Modify: `src/usage/ccusage.rs`
- Modify: `src/game/runtime.rs`
- Modify: `src/commands/status.rs`
- Modify: `src/commands/watch.rs`
- Test: `tests/runtime_integration.rs`

- [ ] **Step 1: Write failing ledger safety tests**

Append to `tests/runtime_integration.rs`:

```rust
use glorp::game::runtime::{apply_unapplied_usage, apply_usage_poll};
use glorp::storage::usage_store::ProviderCursorUpdate;

#[test]
fn unapplied_usage_survives_state_save_failure_and_applies_once_next_run() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026-05-09 12:00 UTC);
    let event = NormalizedUsageEvent {
        observed_at: now,
        bucket_at: now,
        ..NormalizedUsageEvent::for_test_at(now, 100_000.0)
    };
    let cursor = ProviderCursorUpdate {
        provider_surface: "claude-code".into(),
        cursor_key: "test-cursor".into(),
        cursor_value: r#"{"uncached_input":100000,"output":0,"cache_creation":0,"cache_read":0,"reasoning_output":0}"#.into(),
        provider_version: "test-provider".into(),
        parser_version: "test-parser".into(),
    };
    let inserted_id = usage_store.insert_unapplied_event(&event, &cursor).unwrap();

    let mut failed_state = PetState::new_for_test("mochi-7f3a", "mochi");
    failed_state.calibration.daily_effective_tokens = 100_000.0;
    let failed_update = apply_unapplied_usage(&mut failed_state, &mut usage_store, now).unwrap();
    assert_eq!(failed_update.applied_event_ids, vec![inserted_id]);

    let mut retried_state = PetState::new_for_test("mochi-7f3a", "mochi");
    retried_state.calibration.daily_effective_tokens = 100_000.0;
    let retry_update = apply_unapplied_usage(&mut retried_state, &mut usage_store, now).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&retry_update.applied_event_ids, now)
        .unwrap();

    assert_eq!(retried_state.lifetime_effective_tokens, 100_000.0);
    assert_eq!(usage_store.unapplied_events(10).unwrap().len(), 0);
    assert_eq!(
        usage_store
            .provider_cursor("claude-code", "test-cursor")
            .unwrap()
            .unwrap(),
        cursor.cursor_value
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test unapplied_usage_survives_state_save_failure_and_applies_once_next_run
```

Expected: compile failure for missing `ProviderCursorUpdate`, `insert_unapplied_event`, `unapplied_events`, `apply_unapplied_usage`, and `mark_events_applied_and_advance_cursors`.

- [ ] **Step 3: Add cursor update and ledger row types**

In `src/storage/usage_store.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCursorUpdate {
    pub provider_surface: String,
    pub cursor_key: String,
    pub cursor_value: String,
    pub provider_version: String,
    pub parser_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageLedgerRow {
    pub id: i64,
    pub event: NormalizedUsageEvent,
    pub cursor_update: ProviderCursorUpdate,
}
```

In `src/usage/provider.rs`, include the cursor update on each positive delta so
command paths can stage durable ledger rows before applying pet state:

```rust
pub struct UsageDelta {
    pub provider_surface: String,
    pub effective_tokens: f64,
    pub confidence: String,
    pub period_start: String,
    pub observed_at: OffsetDateTime,
    pub model: Option<String>,
    pub cursor_update: ProviderCursorUpdate,
}
```

Add nullable ledger columns to `usage_events`:

```sql
provider_delta_id TEXT,
bucket_index INTEGER NOT NULL DEFAULT 0,
bucket_count INTEGER NOT NULL DEFAULT 1,
applied_at TEXT,
provider_cursor_key TEXT,
provider_cursor_value TEXT
```

Add a unique index:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_usage_events_provider_delta_bucket
    ON usage_events(provider_delta_id, bucket_index)
    WHERE provider_delta_id IS NOT NULL;
```

- [ ] **Step 4: Implement idempotent unapplied event insertion**

Add:

```rust
pub fn insert_unapplied_event(
    &mut self,
    event: &NormalizedUsageEvent,
    cursor_update: &ProviderCursorUpdate,
) -> crate::error::Result<i64> {
    let provider_delta_id = format!(
        "{}|{}|{}",
        cursor_update.provider_surface, cursor_update.cursor_key, cursor_update.cursor_value
    );
    // INSERT OR IGNORE the event with applied_at NULL and cursor fields populated.
    // Then SELECT id by provider_delta_id and bucket_index.
}
```

Use `INSERT OR IGNORE`, then query the ID by `(provider_delta_id, bucket_index)`. Do not call `add_lifetime_counter` from this path.

Keep `insert_event` for fixture and historical test setup, but make it insert an already applied event with `applied_at = observed_at` and no cursor update.

- [ ] **Step 5: Implement poll staging, unapplied selection, and post-save marking**

Add:

```rust
pub fn stage_usage_poll_deltas(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    baseline: CalibrationBaseline,
    now: OffsetDateTime,
) -> Result<Vec<i64>> {
    let mut ids = Vec::new();
    for delta in &poll.deltas {
        let period_start = parse_runtime_period_start(&delta.period_start)?;
        let event = NormalizedUsageEvent {
            provider_surface: delta.provider_surface.clone(),
            provider_version: delta.cursor_update.provider_version.clone(),
            parser_version: delta.cursor_update.parser_version.clone(),
            command: "ccusage".to_string(),
            source_surface: "daily".to_string(),
            period_start,
            observed_at: now,
            bucket_at: floor_to_ten_minute_bucket(now),
            model: delta.model.clone(),
            input_tokens: 0.0,
            output_tokens: 0.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 0.0,
            reasoning_output_tokens: 0.0,
            effective_tokens: delta.effective_tokens,
            cost_usd: None,
            confidence: delta.confidence.clone(),
        };
        ids.push(usage_store.insert_unapplied_event(&event, &delta.cursor_update)?);
    }
    Ok(ids)
}

pub fn unapplied_events(&self, limit: u32) -> crate::error::Result<Vec<UsageLedgerRow>> {
    // SELECT rows where applied_at IS NULL ORDER BY observed_at ASC, id ASC LIMIT ?1.
}

pub fn mark_events_applied_and_advance_cursors(
    &mut self,
    event_ids: &[i64],
    applied_at: OffsetDateTime,
) -> crate::error::Result<()> {
    // In one SQLite transaction:
    // 1. load cursor updates for the given IDs,
    // 2. set applied_at,
    // 3. upsert provider_cursors from loaded updates.
}
```

The transaction should only advance cursors for rows whose `applied_at` is being set or is already set by the same event ID set.

- [ ] **Step 6: Apply unapplied ledger rows from runtime**

In `src/game/runtime.rs`, change `RuntimeUpdate`:

```rust
pub struct RuntimeUpdate {
    pub recent_effective_tokens: f64,
    pub applied_event_ids: Vec<i64>,
}
```

Add:

```rust
pub fn apply_unapplied_usage(
    state: &mut PetState,
    usage_store: &mut UsageStore,
    now: OffsetDateTime,
) -> Result<RuntimeUpdate> {
    let rows = usage_store.unapplied_events(500)?;
    let recent_effective_tokens = rows
        .iter()
        .map(|row| row.event.effective_tokens.max(0.0))
        .sum::<f64>();

    if recent_effective_tokens > 0.0 {
        for row in &rows {
            apply_effective_delta(state, row.event.effective_tokens.max(0.0));
        }
        state.recent_events.push(format!(
            "gained {} effective tokens",
            format_tokens(recent_effective_tokens)
        ));
    } else {
        apply_idle_decay(state, now);
    }

    state.last_usage_poll_at = Some(now);
    state.last_updated_at = now;
    trim_recent_events(state);
    usage_store.compact_before(now - Duration::days(USAGE_RETENTION_DAYS))?;

    Ok(RuntimeUpdate {
        recent_effective_tokens,
        applied_event_ids: rows.into_iter().map(|row| row.id).collect(),
    })
}
```

Keep `apply_usage_poll` as a compatibility wrapper during the refactor, but make command paths call `apply_unapplied_usage`.

- [ ] **Step 7: Move command save boundary to stage-then-save-then-mark**

In `status.rs` and `watch.rs`, after provider polling returns deltas:

```rust
let result = provider.poll(&mut usage_store)?;
stage_usage_poll_deltas(&mut usage_store, &result, state.calibration, now)?;
let update = apply_unapplied_usage(&mut state, &mut usage_store, now)?;
store.save(&state)?;
usage_store.mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)?;
```

If `store.save(&state)` returns an error, do not mark rows applied and do not advance provider cursors.

- [ ] **Step 8: Run focused runtime tests**

Run:

```bash
cargo test runtime_integration usage_provider
```

Expected: runtime ledger tests pass, provider tests still pass, and provider re-polling does not double-insert because unapplied event insertion is idempotent.

- [ ] **Step 9: Commit**

```bash
git add src/storage/usage_store.rs src/usage/provider.rs src/usage/ccusage.rs src/game/runtime.rs src/commands/status.rs src/commands/watch.rs tests/runtime_integration.rs tests/usage_provider.rs
git commit -m "feat: apply usage through durable ledger"
```

---

## Task 4: Calibration Mode, Active-Day Grouping, And No Historical Feeding

**Files:**
- Modify: `src/game/calibration.rs`
- Modify: `src/usage/provider.rs`
- Modify: `src/usage/ccusage.rs`
- Modify: `src/commands/init.rs`
- Test: `tests/game_rules.rs`
- Test: `tests/usage_provider.rs`
- Test: `tests/cli_smoke.rs`

- [ ] **Step 1: Write failing calibration grouping test**

Append to `tests/game_rules.rs`:

```rust
#[test]
fn calibration_groups_multiple_rows_on_the_same_active_day_before_median() {
    let history = vec![
        DailyUsage::new(date!(2026-05-01), 40_000.0),
        DailyUsage::new(date!(2026-05-01), 60_000.0),
        DailyUsage::new(date!(2026-05-02), 100_000.0),
        DailyUsage::new(date!(2026-05-03), 100_000.0),
        DailyUsage::new(date!(2026-05-04), 100_000.0),
        DailyUsage::new(date!(2026-05-05), 100_000.0),
    ];

    let baseline = CalibrationBaseline::from_history(&history);
    assert_eq!(baseline.daily_effective_tokens, 100_000.0);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test calibration_groups_multiple_rows_on_the_same_active_day_before_median
```

Expected: assertion failure or compile success with the wrong median if rows are counted separately.

- [ ] **Step 3: Group history by day before limiting**

In `CalibrationBaseline::from_history`, replace the direct filter with a `BTreeMap<Date, f64>`:

```rust
let mut by_day = std::collections::BTreeMap::<Date, f64>::new();
for day in history.iter().copied().filter(|day| day.effective_tokens > 0.0) {
    *by_day.entry(day.day).or_insert(0.0) += day.effective_tokens;
}
let mut active_days = by_day
    .into_iter()
    .map(|(day, effective_tokens)| DailyUsage::new(day, effective_tokens))
    .collect::<Vec<_>>();
```

Keep the existing active-day count check, recent-day limit, sort, and median.

- [ ] **Step 4: Add calibration snapshot path**

Add a provider method that returns current historical records plus cursor updates without inserting usage events:

```rust
pub struct UsageSnapshot {
    pub daily_usage: Vec<crate::game::calibration::DailyUsage>,
    pub cursor_updates: Vec<ProviderCursorUpdate>,
    pub diagnostics: Vec<ProviderDiagnostic>,
}

pub trait UsageProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult>;
    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot>;
}
```

For `CcusageCommandProvider`, reuse helper normalization and cursor-key construction. Convert normalized records into `DailyUsage` using configured weights. Return cursor updates at the current totals so init can prevent historical replay.

- [ ] **Step 5: Make init calibrate without pet-food events**

In `src/commands/init.rs`, replace the current `poll` plus `recent_events` path:

```rust
let snapshot = CcusageCommandProvider::from_environment_with_weights(weights)
    .snapshot_for_calibration(&mut usage_store)?;
calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
usage_store.advance_cursors(snapshot.cursor_updates, OffsetDateTime::now_utc())?;
```

Do not call `insert_event`, `insert_unapplied_event`, `apply_unapplied_usage`, or `mark_events_applied_and_advance_cursors` during init.

- [ ] **Step 6: Add a smoke assertion for no historical feeding on init**

In `tests/cli_smoke.rs`, extend the existing init test or add a focused one that runs `glorp init --yes` with helper fixtures and then loads `state.json`:

```rust
assert!(state_json.contains(r#""xp": 0.0"#) || state_json.contains(r#""xp":0.0"#));
assert!(state_json.contains(r#""lifetime_effective_tokens": 0.0"#) || state_json.contains(r#""lifetime_effective_tokens":0.0"#));
```

Also open the usage DB and assert `recent_event_count() == 0`.

- [ ] **Step 7: Run calibration and init tests**

Run:

```bash
cargo test calibration_groups_multiple_rows_on_the_same_active_day_before_median cli_smoke
```

Expected: all named tests pass, init leaves pet XP and lifetime food at zero, and the provider cursors are seeded for future diffs.

- [ ] **Step 8: Commit**

```bash
git add src/game/calibration.rs src/usage/provider.rs src/usage/ccusage.rs src/commands/init.rs tests/game_rules.rs tests/usage_provider.rs tests/cli_smoke.rs
git commit -m "feat: calibrate without historical feeding"
```

---

## Task 5: Catch-Up Smearing And Long-Arc Evolution Acceptance

**Files:**
- Create: `src/game/catchup.rs`
- Modify: `src/game/mod.rs`
- Modify: `src/game/runtime.rs`
- Test: `tests/game_rules.rs`
- Test: `tests/runtime_integration.rs`

- [ ] **Step 1: Write failing catch-up tests**

Append to `tests/game_rules.rs`:

```rust
use glorp::game::catchup::smear_catchup_delta;

#[test]
fn one_active_day_catchup_splits_into_six_to_twelve_buckets() {
    let baseline = CalibrationBaseline {
        daily_effective_tokens: 100_000.0,
    };
    let buckets = smear_catchup_delta(100_000.0, baseline);
    assert!((6..=12).contains(&buckets.len()));
    assert!(buckets.iter().all(|bucket| *bucket <= 25_000.0));
    let xp = buckets
        .iter()
        .fold(0.0, |xp, bucket| apply_xp_delta(xp, *bucket, baseline).xp);
    assert!((0.75..=1.25).contains(&xp));
}

#[test]
fn forty_nine_daily_catchups_reach_s6_without_duplicate_transition_pressure() {
    let baseline = CalibrationBaseline {
        daily_effective_tokens: 100_000.0,
    };
    let mut xp = 0.0;
    for _ in 0..49 {
        for bucket in smear_catchup_delta(100_000.0, baseline) {
            xp = apply_xp_delta(xp, bucket, baseline).xp;
        }
    }
    assert_eq!(stage_for_xp(xp), Stage::S6);
    assert!((49.0..=55.0).contains(&xp));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test one_active_day_catchup_splits_into_six_to_twelve_buckets forty_nine_daily_catchups_reach_s6_without_duplicate_transition_pressure
```

Expected: compile failure because `game::catchup` does not exist.

- [ ] **Step 3: Add catch-up smear helper**

Create `src/game/catchup.rs`:

```rust
use crate::game::calibration::CalibrationBaseline;

pub fn smear_catchup_delta(
    effective_tokens: f64,
    baseline: CalibrationBaseline,
) -> Vec<f64> {
    let effective_tokens = effective_tokens.max(0.0);
    if effective_tokens == 0.0 {
        return Vec::new();
    }

    let daily = baseline.daily_effective_tokens.max(1.0);
    let bucket_count = ((effective_tokens / (daily * 0.125)).ceil() as usize).clamp(6, 12);
    let mut buckets = vec![effective_tokens / bucket_count as f64; bucket_count];
    let max_bucket = daily * 0.25;
    for bucket in &mut buckets {
        *bucket = bucket.min(max_bucket);
    }
    buckets
}
```

Add `pub mod catchup;` to `src/game/mod.rs`.

- [ ] **Step 4: Persist smeared buckets before applying unapplied usage**

Change `stage_usage_poll_deltas` so it uses its `baseline: CalibrationBaseline`
argument and creates one ledger row per smeared bucket instead of one row per
provider delta. Each row keeps the same provider cursor update and a shared
`provider_delta_id`, but uses a distinct `bucket_index` and `bucket_at`.

Add this `UsageStore` helper beside `insert_unapplied_event`:

```rust
pub fn insert_unapplied_event_bucket(
    &mut self,
    event: &NormalizedUsageEvent,
    cursor_update: &ProviderCursorUpdate,
    bucket_index: usize,
    bucket_count: usize,
) -> crate::error::Result<i64> {
    // Same insert path as insert_unapplied_event, but persists bucket_index
    // and bucket_count instead of defaulting to 0 and 1.
}
```

```rust
let buckets = crate::game::catchup::smear_catchup_delta(
    delta.effective_tokens,
    baseline,
);
let bucket_count = buckets.len();
let current_bucket = floor_to_ten_minute_bucket(now);
for (bucket_index, effective_tokens) in buckets.into_iter().enumerate() {
    let bucket_offset = bucket_count.saturating_sub(bucket_index + 1) as i64;
    let bucket_at = current_bucket - Duration::minutes(bucket_offset * 10);
    let event = NormalizedUsageEvent {
        observed_at: now,
        bucket_at,
        effective_tokens,
        ..event_for_delta(delta, now)?
    };
    usage_store.insert_unapplied_event_bucket(
        &event,
        &delta.cursor_update,
        bucket_index,
        bucket_count,
    )?;
}
```

`apply_unapplied_usage` should apply ledger rows directly in
`bucket_at ASC, id ASC` order. Do not smear again while applying. The provider
cursor update is advanced once after all rows for the staged poll are applied
and pet state is saved.

- [ ] **Step 5: Add runtime duplicate-transition guard test**

Append to `tests/runtime_integration.rs`:

```rust
#[test]
fn catchup_application_records_each_stage_transition_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026-05-09 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;

    for day in 0..49 {
        let observed_at = now + Duration::days(day);
        let buckets = glorp::game::catchup::smear_catchup_delta(
            100_000.0,
            state.calibration,
        );
        let bucket_count = buckets.len();
        for (bucket_index, effective_tokens) in buckets.into_iter().enumerate() {
            let bucket_at = observed_at - Duration::minutes(
                bucket_count.saturating_sub(bucket_index + 1) as i64 * 10,
            );
            usage_store
                .insert_unapplied_event_bucket(
                    &NormalizedUsageEvent {
                        observed_at,
                        bucket_at,
                        effective_tokens,
                        ..NormalizedUsageEvent::for_test_at(observed_at, effective_tokens)
                    },
                    &ProviderCursorUpdate {
                        provider_surface: "claude-code".into(),
                        cursor_key: format!("cursor-{day}"),
                        cursor_value: format!("value-{day}"),
                        provider_version: "test-provider".into(),
                        parser_version: "test-parser".into(),
                    },
                    bucket_index,
                    bucket_count,
                )
                .unwrap();
        }
    }

    let update = apply_unapplied_usage(&mut state, &mut usage_store, now).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();

    assert_eq!(state.stage, "s6");
    assert_eq!(state.seen_stage_transitions.len(), 6);
    assert_eq!(
        state.seen_stage_transitions,
        vec!["s0->s1", "s1->s2", "s2->s3", "s3->s4", "s4->s5", "s5->s6"]
    );
}
```

- [ ] **Step 6: Run catch-up tests**

Run:

```bash
cargo test one_active_day_catchup_splits_into_six_to_twelve_buckets forty_nine_daily_catchups_reach_s6_without_duplicate_transition_pressure catchup_application_records_each_stage_transition_once
```

Expected: all named tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/game/catchup.rs src/game/mod.rs src/game/runtime.rs tests/game_rules.rs tests/runtime_integration.rs
git commit -m "feat: smear catch-up usage into pet buckets"
```

---

## Task 6: Data Pipeline Verification Gate

**Files:**
- Modify as needed from earlier tasks only.

- [ ] **Step 1: Run full Rust verification for data truth work**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit 0.

- [ ] **Step 2: Check scope**

Run:

```bash
git diff --stat HEAD~5..HEAD
git status --short
```

Expected: changes are limited to data pipeline source, tests, and the new helper fixture. Packaging files under `npm/` and Story 010 release plumbing are unchanged.

- [ ] **Step 3: Handoff to watch plan**

After this plan passes, move to `docs/superpowers/plans/2026-05-09-glorp-watch-presentation-interaction.md`. The watch plan assumes `NormalizedUsageEvent` has `observed_at` and `bucket_at`, provider diagnostics are source-specific, and command paths use the save-then-mark ledger boundary.
