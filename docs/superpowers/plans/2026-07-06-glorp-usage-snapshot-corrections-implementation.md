# Glorp Usage Snapshot Corrections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Glorp show current provider-day token truth from complete snapshots while keeping pet progression on the accepted-food ledger and preventing corrected or rebounding provider totals from double-feeding the pet.

**Architecture:** Add snapshot storage and typed query APIs beside the existing `usage_events` feed ledger. Providers write complete requested provider-day snapshots before feed evaluation, then a source-day-first high-water evaluator emits only safe positive food deltas. Watch, status, doctor, and repair paths read snapshot-backed provider truth separately from accepted-food rate windows.

**Tech Stack:** Rust 2021, rusqlite, serde/serde_json, time 0.3, clap, assert_cmd, predicates, tempfile.

## Global Constraints

- Visible accounting answers current provider truth for a Tokenmaxxing America/Los_Angeles provider day.
- Pet progression answers what positive usage deltas Glorp already accepted as food.
- No negative usage events in the pet feed ledger.
- No retroactive XP, lifetime-token, prop, stage, or vital rollback.
- Snapshot, diagnostic, and correction storage must not persist prompts, responses, file paths, project names, raw transcript content, or unsanitized raw source ids.
- Missing or blocked snapshots are typed states, not numeric zeroes.
- A complete zero-row snapshot run is current truth with numeric zero.
- Feed high-water totals never move down during normal correction handling.
- Source first contact is source-level, not provider-day-level.
- Configured collectors, health rows, and snapshot-only repair rows do not count as source registration.
- Existing feed-ledger APIs remain the source for rate momentum, current bucket, live reactions, recent feed events, lifetime pet food, XP, vitals, and props.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `src/usage/snapshot.rs` | Create | Snapshot request/result domain types shared by storage, providers, runtime, and UI |
| `src/usage/feed_high_water.rs` | Create | Source-day-first feed eligibility and total-only correction decisions |
| `src/usage/mod.rs` | Modify | Export snapshot and feed-high-water modules |
| `src/usage/provider.rs` | Modify | Carry requested snapshot scopes and snapshot rows through provider poll results |
| `src/usage/day_axis.rs` | Modify | Expose Tokenmaxxing provider-day helpers for snapshot queries |
| `src/usage/ccusage.rs` | Modify | Write requested-day snapshots before feed deltas; treat extra days and blocked rows as diagnostics |
| `src/usage/agentsview.rs` | Modify | Same snapshot-first poll flow for agentsview |
| `src/usage/cutover.rs` | Modify | Seed canonical source-day high-waters during Tokenmaxxing collector cutover |
| `src/storage/usage_store.rs` | Modify | Add snapshot, correction, diagnostic, source-contact, and high-water tables plus query/write APIs |
| `src/game/runtime.rs` | Modify | Replace cursor-only first-contact logic with source-contact-aware high-water staging |
| `src/tui/view_model.rs` | Modify | Add typed snapshot state fields to today/source health views |
| `src/commands/watch.rs` | Modify | Read snapshot-backed visible totals and feed-ledger-backed rate windows |
| `src/commands/status.rs` | Modify | Print provider today/current snapshot separately from accepted recent and pet lifetime food |
| `src/commands/doctor.rs` | Modify | Add `--refresh-usage-snapshots`, correction notices, blocked-snapshot reporting |
| `src/cli.rs` | Modify | Add `doctor --refresh-usage-snapshots` flag |
| `src/lib.rs` | Modify | Route `Command::Doctor { refresh_usage_snapshots }` |
| `tests/usage_snapshots.rs` | Create | Storage/query transaction tests for snapshot truth and state precedence |
| `tests/feed_high_water.rs` | Create | Feed high-water and source-day-first evaluator tests |
| `tests/usage_provider.rs` | Modify | Provider snapshot-write and requested-day scope tests |
| `tests/runtime_integration.rs` | Modify | Pet-state non-rollback and source-first-contact regression tests |
| `tests/watch_integration.rs` | Modify | Watch snapshot-vs-feed surface tests |
| `tests/doctor_status.rs` | Modify | Status/doctor command output and repair tests |
| `tests/storage_privacy.rs` | Modify | Snapshot schema privacy and migration checks |
| `tests/fixtures/helpers/ccusage-drop-day.mjs` | Create | Helper fixture that omits a previously snapshotted provider day |
| `tests/fixtures/helpers/ccusage-extra-day.mjs` | Create | Helper fixture that returns an unrequested provider day |
| `tests/fixtures/helpers/ccusage-malformed-row.mjs` | Create | Helper fixture with one malformed required row in an otherwise parseable response |
| `tests/fixtures/helpers/ccusage-model-remap.mjs` | Create | Helper fixture that remaps model labels without changing source-day aggregate |
| `tests/fixtures/helpers/agentsview-drop-day.mjs` | Create | Agentsview variant of disappeared-day fixture |

---

## Task 1: Add Snapshot Domain Types And Storage Schema

**Files:**
- Create: `src/usage/snapshot.rs`
- Modify: `src/usage/mod.rs`
- Modify: `src/usage/day_axis.rs`
- Modify: `src/storage/usage_store.rs`
- Modify: `tests/storage_privacy.rs`

**Interfaces:**
- Produces: `SnapshotState`, `SnapshotResult<T>`, `DayTotals`, `SourceTotals`, `SourceSnapshotHealth`
- Produces: `ProviderSnapshotBatchInput`, `ProviderSnapshotRowInput`, `ProviderSnapshotDiagnosticInput`
- Produces schema tables: `provider_snapshot_batches`, `provider_snapshot_runs`, `provider_snapshot_rows`, `provider_corrections`, `provider_snapshot_diagnostics`, `provider_canonical_collectors`, `provider_source_contacts`, `provider_feed_highwaters`
- Consumed by Tasks 2-7.

- [ ] **Step 1: Write failing schema and privacy tests**

Append to `tests/storage_privacy.rs`:

```rust
#[test]
fn snapshot_tables_exist_without_raw_transcript_columns() {
    let dir = tempfile::tempdir().unwrap();
    let store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let conn = store.raw_connection_for_test();

    let tables = [
        "provider_snapshot_batches",
        "provider_snapshot_runs",
        "provider_snapshot_rows",
        "provider_corrections",
        "provider_snapshot_diagnostics",
        "provider_canonical_collectors",
        "provider_source_contacts",
        "provider_feed_highwaters",
    ];

    for table in tables {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "missing table {table}");

        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for forbidden in [
            "prompt",
            "response",
            "raw_prompt",
            "raw_response",
            "file_path",
            "project_path",
        ] {
            assert!(
                !names.iter().any(|name| name.contains(forbidden)),
                "{table} contains forbidden column {forbidden}: {names:?}"
            );
        }
        if table == "provider_snapshot_rows" {
            assert!(
                names.iter().any(|name| name == "raw_source_id_hash"),
                "sanitized raw source identity hash column must exist: {names:?}"
            );
        }
    }
}
```

Add this test-support accessor near the existing storage helpers if it does not already exist:

```rust
impl UsageStore {
    #[doc(hidden)]
    pub fn raw_connection_for_test(&self) -> &rusqlite::Connection {
        &self.conn
    }
}
```

- [ ] **Step 2: Run the schema test and verify it fails**

Run:

```bash
cargo test --test storage_privacy snapshot_tables_exist_without_raw_transcript_columns
```

Expected: FAIL because the snapshot tables do not exist.

- [ ] **Step 3: Add snapshot domain types**

Create `src/usage/snapshot.rs`:

```rust
use crate::storage::usage_store::ProviderCursorUpdate;
use crate::usage::normalize::RawTokenTotals;
use time::{Date, OffsetDateTime};

pub const SNAPSHOT_STATE_CURRENT: &str = "current";
pub const SNAPSHOT_STATE_STALE: &str = "stale";
pub const SNAPSHOT_STATE_MISSING: &str = "missing";
pub const SNAPSHOT_STATE_BLOCKED: &str = "blocked";
pub const BUCKET_CONFIDENCE_EXACT: &str = "exact";
pub const BUCKET_CONFIDENCE_CORRECTED_TOTAL_ONLY: &str = "corrected-total-only";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotState {
    Current,
    Stale,
    Missing,
    Blocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotResult<T> {
    pub state: SnapshotState,
    pub value: Option<T>,
    pub provider_day: Date,
    pub observed_at: Option<OffsetDateTime>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayTotals {
    pub total_tokens: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceTotals {
    pub sources: Vec<SourceTotal>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceTotal {
    pub accounting_source: String,
    pub total_tokens: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceSnapshotHealth {
    pub accounting_source: String,
    pub display_name: String,
    pub snapshot_state: SnapshotState,
    pub snapshot_total_tokens: Option<f64>,
    pub recent_accepted_tokens: f64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSnapshotBatchInput {
    pub collector_scope_id: String,
    pub collector_surface: String,
    pub command: String,
    pub token_contract: String,
    pub requested_provider_days: Vec<Date>,
    pub provider_version: String,
    pub parser_version: String,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSnapshotRowInput {
    pub replacement_scope_id: String,
    pub collector_scope_id: String,
    pub collector_surface: String,
    pub command: String,
    pub token_contract: String,
    pub accounting_source: String,
    pub provider_day: Date,
    pub model: Option<String>,
    pub source_surface: String,
    pub provider_period: String,
    pub raw_source_id_hash: Option<String>,
    pub cursor_key_hash: String,
    pub cursor_update: ProviderCursorUpdate,
    pub raw_token_buckets: Option<RawTokenTotals>,
    pub total_tokens: f64,
    pub cost_usd: Option<f64>,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSnapshotDiagnosticInput {
    pub diagnostic_kind: String,
    pub collector_scope_id: String,
    pub replacement_scope_id: Option<String>,
    pub requested_provider_days: Vec<Date>,
    pub provider_day: Option<Date>,
    pub reason_code: String,
    pub message: String,
    pub observed_at: OffsetDateTime,
}
```

Modify `src/usage/mod.rs`:

```rust
pub mod agentsview;
pub mod ccusage;
pub mod cutover;
pub mod day_axis;
pub mod helper_locator;
pub mod identity;
pub mod normalize;
pub mod provider;
pub mod snapshot;
pub mod token_contract;

pub use identity::{normalize_source_label, SourceFamily, SourceIdentity};
```

- [ ] **Step 4: Expose provider-day helpers**

Add to `src/usage/day_axis.rs`:

```rust
pub fn tokenmaxxing_provider_day(now: OffsetDateTime) -> Date {
    tokenmaxxing_date(now)
}

pub fn tokenmaxxing_days_back(now: OffsetDateTime, count: usize) -> Vec<Date> {
    let today = tokenmaxxing_provider_day(now);
    let start = count.saturating_sub(1) as i64;
    (0..count)
        .map(|index| today - time::Duration::days(start - index as i64))
        .collect()
}
```

Add tests to the existing `#[cfg(test)]` module in `src/usage/day_axis.rs` or create the module if absent:

```rust
#[test]
fn provider_day_uses_los_angeles_date() {
    let before_la_midnight = time::macros::datetime!(2026 - 07 - 06 06:59:00 UTC);
    let after_la_midnight = time::macros::datetime!(2026 - 07 - 06 07:00:00 UTC);

    assert_eq!(
        tokenmaxxing_provider_day(before_la_midnight),
        time::macros::date!(2026 - 07 - 05)
    );
    assert_eq!(
        tokenmaxxing_provider_day(after_la_midnight),
        time::macros::date!(2026 - 07 - 06)
    );
}

#[test]
fn days_back_returns_oldest_to_newest_provider_days() {
    let now = time::macros::datetime!(2026 - 07 - 06 20:00 UTC);
    assert_eq!(
        tokenmaxxing_days_back(now, 3),
        vec![
            time::macros::date!(2026 - 07 - 04),
            time::macros::date!(2026 - 07 - 05),
            time::macros::date!(2026 - 07 - 06),
        ]
    );
}
```

- [ ] **Step 5: Add migration DDL**

Inside `UsageStore::migrate`, add the following `CREATE TABLE` statements to the existing `execute_batch` block after `provider_diagnostics`:

```sql
CREATE TABLE IF NOT EXISTS provider_snapshot_batches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collector_scope_id TEXT NOT NULL,
    collector_surface TEXT NOT NULL,
    command TEXT NOT NULL,
    token_contract TEXT NOT NULL,
    requested_provider_days_json TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    parser_version TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    completion_status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_snapshot_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id INTEGER,
    replacement_scope_id TEXT NOT NULL,
    collector_scope_id TEXT NOT NULL,
    collector_surface TEXT NOT NULL,
    command TEXT NOT NULL,
    token_contract TEXT NOT NULL,
    provider_day TEXT NOT NULL,
    provider_version TEXT NOT NULL,
    parser_version TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    completion_status TEXT NOT NULL,
    reason_code TEXT,
    FOREIGN KEY(batch_id) REFERENCES provider_snapshot_batches(id)
);

CREATE TABLE IF NOT EXISTS provider_snapshot_rows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL,
    replacement_scope_id TEXT NOT NULL,
    collector_scope_id TEXT NOT NULL,
    collector_surface TEXT NOT NULL,
    command TEXT NOT NULL,
    token_contract TEXT NOT NULL,
    accounting_source TEXT NOT NULL,
    provider_day TEXT NOT NULL,
    model TEXT,
    source_surface TEXT NOT NULL,
    provider_period TEXT NOT NULL,
    raw_source_id_hash TEXT,
    cursor_key_hash TEXT NOT NULL,
    input_tokens REAL,
    output_tokens REAL,
    cache_creation_tokens REAL,
    cache_read_tokens REAL,
    reasoning_output_tokens REAL,
    total_tokens REAL NOT NULL,
    cost_usd REAL,
    confidence TEXT NOT NULL,
    status TEXT NOT NULL,
    first_observed_at TEXT NOT NULL,
    last_observed_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES provider_snapshot_runs(id)
);

CREATE TABLE IF NOT EXISTS provider_corrections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    correction_kind TEXT NOT NULL,
    token_contract TEXT NOT NULL,
    accounting_source TEXT NOT NULL,
    provider_day TEXT NOT NULL,
    model TEXT,
    previous_total_tokens REAL NOT NULL,
    current_total_tokens REAL NOT NULL,
    decrease_tokens REAL NOT NULL,
    previous_raw_buckets_json TEXT,
    current_raw_buckets_json TEXT,
    collector_surface TEXT NOT NULL,
    cursor_key_hash TEXT,
    batch_id INTEGER,
    run_id INTEGER,
    recorded_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_snapshot_diagnostics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    diagnostic_kind TEXT NOT NULL,
    collector_scope_id TEXT NOT NULL,
    replacement_scope_id TEXT,
    requested_provider_days_json TEXT NOT NULL,
    provider_day TEXT,
    reason_code TEXT NOT NULL,
    message TEXT NOT NULL,
    batch_id INTEGER,
    run_id INTEGER,
    recorded_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_canonical_collectors (
    token_contract TEXT NOT NULL,
    accounting_source TEXT NOT NULL,
    provider_day TEXT NOT NULL,
    collector_scope_id TEXT NOT NULL,
    replacement_scope_id TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (token_contract, accounting_source, provider_day)
);

CREATE TABLE IF NOT EXISTS provider_source_contacts (
    token_contract TEXT NOT NULL,
    accounting_source TEXT NOT NULL,
    contact_kind TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY (token_contract, accounting_source)
);

CREATE TABLE IF NOT EXISTS provider_feed_highwaters (
    highwater_kind TEXT NOT NULL,
    token_contract TEXT NOT NULL,
    accounting_source TEXT NOT NULL,
    provider_day TEXT,
    provider_day_key TEXT NOT NULL DEFAULT '',
    model TEXT,
    model_key TEXT NOT NULL DEFAULT '',
    provider_surface TEXT,
    provider_surface_key TEXT NOT NULL DEFAULT '',
    cursor_key_hash TEXT,
    cursor_key_hash_key TEXT NOT NULL DEFAULT '',
    total_high_water REAL NOT NULL,
    latest_raw_buckets_json TEXT,
    exact_raw_buckets_json TEXT,
    bucket_confidence TEXT NOT NULL,
    unshaped_total_only_tokens REAL NOT NULL DEFAULT 0.0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (
        highwater_kind,
        token_contract,
        accounting_source,
        provider_day_key,
        model_key,
        provider_surface_key,
        cursor_key_hash_key
    )
);
```

Add indexes after the existing index block:

```sql
CREATE INDEX IF NOT EXISTS idx_provider_snapshot_rows_visible
    ON provider_snapshot_rows(token_contract, accounting_source, provider_day, status);
CREATE INDEX IF NOT EXISTS idx_provider_snapshot_runs_scope
    ON provider_snapshot_runs(replacement_scope_id, token_contract, provider_day, observed_at);
CREATE INDEX IF NOT EXISTS idx_provider_snapshot_diagnostics_scope
    ON provider_snapshot_diagnostics(collector_scope_id, provider_day, recorded_at);
CREATE INDEX IF NOT EXISTS idx_provider_corrections_day
    ON provider_corrections(token_contract, accounting_source, provider_day, recorded_at);
```

- [ ] **Step 6: Run schema test**

Run:

```bash
cargo test usage::day_axis::provider_day_uses_los_angeles_date
cargo test usage::day_axis::days_back_returns_oldest_to_newest_provider_days
cargo test --test storage_privacy snapshot_tables_exist_without_raw_transcript_columns
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/usage/snapshot.rs src/usage/mod.rs src/usage/day_axis.rs src/storage/usage_store.rs tests/storage_privacy.rs
git commit -m "feat: add usage snapshot schema"
```

---

## Task 2: Implement Snapshot Write Transaction And Typed Queries

**Files:**
- Modify: `src/storage/usage_store.rs`
- Modify: `src/usage/snapshot.rs`
- Create: `tests/usage_snapshots.rs`

**Interfaces:**
- Consumes: Task 1 snapshot input/result types.
- Produces: `UsageStore::write_provider_snapshot_batch(batch, rows, diagnostics) -> Result<ProviderSnapshotWriteOutcome>`
- Produces: `UsageStore::record_snapshot_failure(diagnostic) -> Result<()>`
- Produces: `UsageStore::snapshot_totals_for_provider_day(day) -> Result<SnapshotResult<DayTotals>>`
- Produces: `UsageStore::snapshot_totals_by_source_for_provider_day(day) -> Result<SnapshotResult<SourceTotals>>`
- Produces: `UsageStore::snapshot_token_history_for_provider_days(days) -> Result<Vec<SnapshotResult<DayTotals>>>`
- Produces: `UsageStore::snapshot_health_for_provider_day(day, recent_accepted) -> Result<Vec<SourceSnapshotHealth>>`

- [ ] **Step 1: Write failing zero-row and state-precedence tests**

Create `tests/usage_snapshots.rs`:

```rust
use glorp::{
    storage::usage_store::{ProviderCursorUpdate, UsageStore},
    usage::{
        normalize::RawTokenTotals,
        snapshot::{
            DayTotals, ProviderSnapshotBatchInput, ProviderSnapshotDiagnosticInput,
            ProviderSnapshotRowInput, SnapshotState,
        },
        token_contract::TOKENMAXXING_TOTAL_V1,
    },
};
use rusqlite::params;
use tempfile::tempdir;
use time::{macros::date, macros::datetime, Date, OffsetDateTime};

fn batch(day: Date, observed_at: OffsetDateTime) -> ProviderSnapshotBatchInput {
    ProviderSnapshotBatchInput {
        collector_scope_id: "claude-code:local-usage".into(),
        collector_surface: "ccusage:claude-code".into(),
        command: "ccusage claude daily --json --offline".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        provider_version: "ccusage 20.0.6".into(),
        parser_version: "ccusage 20.0.6".into(),
        observed_at,
    }
}

fn row(day: Date, model: &str, total: f64, observed_at: OffsetDateTime) -> ProviderSnapshotRowInput {
    ProviderSnapshotRowInput {
        replacement_scope_id: "claude-code:local-usage".into(),
        collector_scope_id: "claude-code:local-usage".into(),
        collector_surface: "ccusage:claude-code".into(),
        command: "ccusage claude daily --json --offline".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: "claude-code".into(),
        provider_day: day,
        model: Some(model.into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some("hash:source".into()),
        cursor_key_hash: format!("hash:{model}"),
        cursor_update: ProviderCursorUpdate {
            provider_surface: "claude-code".into(),
            cursor_key: format!("cursor:{model}"),
            cursor_value: format!("raw:{total}"),
            provider_version: "ccusage 20.0.6".into(),
            parser_version: "ccusage 20.0.6".into(),
        },
        raw_token_buckets: Some(RawTokenTotals {
            uncached_input: total as u64,
            output: 0,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        }),
        total_tokens: total,
        cost_usd: None,
        confidence: "local-log-derived".into(),
    }
}

fn blocked(day: Date, observed_at: OffsetDateTime) -> ProviderSnapshotDiagnosticInput {
    ProviderSnapshotDiagnosticInput {
        diagnostic_kind: "run_blocked".into(),
        collector_scope_id: "claude-code:local-usage".into(),
        replacement_scope_id: Some("claude-code:local-usage".into()),
        requested_provider_days: vec![day],
        provider_day: Some(day),
        reason_code: "malformed_required_fields".into(),
        message: "ccusage malformed_required_fields".into(),
        observed_at,
    }
}

#[test]
fn complete_zero_row_run_replaces_prior_visible_day_with_current_zero() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let first_at = datetime!(2026 - 07 - 06 20:00 UTC);
    let second_at = datetime!(2026 - 07 - 06 21:00 UTC);

    store
        .write_provider_snapshot_batch(&batch(day, first_at), &[row(day, "claude-fable-5", 531.0, first_at)], &[])
        .unwrap();
    store
        .write_provider_snapshot_batch(&batch(day, second_at), &[], &[])
        .unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Current);
    assert_eq!(result.value, Some(DayTotals { total_tokens: 0.0 }));
}

#[test]
fn blocked_latest_attempt_with_prior_snapshot_returns_stale_value() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let first_at = datetime!(2026 - 07 - 06 20:00 UTC);
    let blocked_at = datetime!(2026 - 07 - 06 21:00 UTC);

    store
        .write_provider_snapshot_batch(&batch(day, first_at), &[row(day, "claude-fable-5", 531.0, first_at)], &[])
        .unwrap();
    store.record_snapshot_failure(&blocked(day, blocked_at)).unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Stale);
    assert_eq!(result.value, Some(DayTotals { total_tokens: 531.0 }));
    assert_eq!(result.reason.as_deref(), Some("malformed_required_fields"));
}

#[test]
fn top_level_failure_before_any_snapshot_returns_blocked_with_no_value() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    store.record_snapshot_failure(&blocked(day, datetime!(2026 - 07 - 06 21:00 UTC))).unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Blocked);
    assert_eq!(result.value, None);
    assert_eq!(result.reason.as_deref(), Some("malformed_required_fields"));
}

#[test]
fn model_remap_with_same_source_day_total_is_identity_diagnostic_not_downward_correction() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let first_at = datetime!(2026 - 07 - 06 20:00 UTC);
    let second_at = datetime!(2026 - 07 - 06 21:00 UTC);

    store
        .write_provider_snapshot_batch(&batch(day, first_at), &[row(day, "old-model", 531.0, first_at)], &[])
        .unwrap();
    store
        .write_provider_snapshot_batch(&batch(day, second_at), &[row(day, "new-model", 531.0, second_at)], &[])
        .unwrap();

    let correction_count: i64 = store
        .raw_connection_for_test()
        .query_row(
            "SELECT COUNT(*) FROM provider_corrections
             WHERE token_contract = ?1 AND accounting_source = ?2 AND provider_day = ?3",
            params![TOKENMAXXING_TOTAL_V1, "claude-code", day.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        correction_count, 0,
        "unchanged source-day aggregate must not create downward correction"
    );

    let identity_diagnostic_count: i64 = store
        .raw_connection_for_test()
        .query_row(
            "SELECT COUNT(*) FROM provider_snapshot_diagnostics
             WHERE diagnostic_kind = 'identity_remap' AND provider_day = ?1",
            params![day.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(identity_diagnostic_count, 1);
}

#[test]
fn no_attempt_returns_missing_not_polled() {
    let dir = tempdir().unwrap();
    let store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let result = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(result.state, SnapshotState::Missing);
    assert_eq!(result.value, None);
    assert_eq!(result.reason.as_deref(), Some("not_polled"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test usage_snapshots
```

Expected: compile failure because the write/query APIs do not exist.

- [ ] **Step 3: Implement write outcome and APIs**

Add to `src/usage/snapshot.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSnapshotWriteOutcome {
    pub batch_id: i64,
    pub complete_run_ids: Vec<i64>,
    pub blocked_run_ids: Vec<i64>,
}
```

In `src/storage/usage_store.rs`, implement `write_provider_snapshot_batch` as one transaction:

```rust
pub fn write_provider_snapshot_batch(
    &mut self,
    batch: &crate::usage::snapshot::ProviderSnapshotBatchInput,
    rows: &[crate::usage::snapshot::ProviderSnapshotRowInput],
    diagnostics: &[crate::usage::snapshot::ProviderSnapshotDiagnosticInput],
) -> crate::error::Result<crate::usage::snapshot::ProviderSnapshotWriteOutcome> {
    let tx = self.conn.transaction()?;
    let requested_days_json = serde_json::to_string(
        &batch
            .requested_provider_days
            .iter()
            .map(Date::to_string)
            .collect::<Vec<_>>(),
    )?;
    tx.execute(
        "INSERT INTO provider_snapshot_batches (
            collector_scope_id, collector_surface, command, token_contract,
            requested_provider_days_json, provider_version, parser_version,
            observed_at, completion_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'complete')",
        params![
            batch.collector_scope_id,
            batch.collector_surface,
            batch.command,
            batch.token_contract,
            requested_days_json,
            batch.provider_version,
            batch.parser_version,
            format_time(batch.observed_at)?,
        ],
    )?;
    let batch_id = tx.last_insert_rowid();
    let mut complete_run_ids = Vec::new();
    let mut blocked_run_ids = Vec::new();

    for day in &batch.requested_provider_days {
        let day_rows = rows
            .iter()
            .filter(|row| row.provider_day == *day)
            .collect::<Vec<_>>();
        let day_diagnostics = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.provider_day == Some(*day))
            .collect::<Vec<_>>();
        let replacement_scope_id = day_rows
            .first()
            .map(|row| row.replacement_scope_id.as_str())
            .or_else(|| day_diagnostics.first().and_then(|d| d.replacement_scope_id.as_deref()))
            .unwrap_or(batch.collector_scope_id.as_str());
        let completion_status = if day_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.diagnostic_kind == "run_blocked")
        {
            "blocked"
        } else {
            "complete"
        };
        tx.execute(
            "INSERT INTO provider_snapshot_runs (
                batch_id, replacement_scope_id, collector_scope_id, collector_surface,
                command, token_contract, provider_day, provider_version, parser_version,
                observed_at, completion_status, reason_code
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                batch_id,
                replacement_scope_id,
                batch.collector_scope_id,
                batch.collector_surface,
                batch.command,
                batch.token_contract,
                day.to_string(),
                batch.provider_version,
                batch.parser_version,
                format_time(batch.observed_at)?,
                completion_status,
                day_diagnostics.first().map(|d| d.reason_code.as_str()),
            ],
        )?;
        let run_id = tx.last_insert_rowid();
        if completion_status == "blocked" {
            blocked_run_ids.push(run_id);
            insert_snapshot_diagnostics(&tx, batch_id, Some(run_id), &day_diagnostics)?;
            continue;
        }
        complete_run_ids.push(run_id);
        supersede_previous_snapshot_rows(&tx, replacement_scope_id, &batch.token_contract, *day)?;
        insert_snapshot_rows(&tx, run_id, &day_rows)?;
        refresh_canonical_collectors(&tx, replacement_scope_id, &batch.token_contract, *day, &day_rows, batch.observed_at)?;
        record_snapshot_corrections(&tx, run_id, replacement_scope_id, &batch.token_contract, *day, &day_rows, batch.observed_at)?;
    }

    tx.commit()?;
    Ok(crate::usage::snapshot::ProviderSnapshotWriteOutcome {
        batch_id,
        complete_run_ids,
        blocked_run_ids,
    })
}
```

Implement the helper functions named in that method in the same file. Use SQL aggregates over `provider_snapshot_rows` for prior/current source-day totals. Insert user-facing `provider_corrections` rows only when source-day aggregate totals decrease. When the source-day aggregate total is unchanged but row identity changes, insert a sanitized `provider_snapshot_diagnostics` row with `diagnostic_kind = 'identity_remap'` and no `provider_corrections` row.

- [ ] **Step 4: Implement failure diagnostics and typed queries**

Add this public method:

```rust
pub fn record_snapshot_failure(
    &mut self,
    diagnostic: &crate::usage::snapshot::ProviderSnapshotDiagnosticInput,
) -> crate::error::Result<()> {
    let requested_days_json = serde_json::to_string(
        &diagnostic
            .requested_provider_days
            .iter()
            .map(Date::to_string)
            .collect::<Vec<_>>(),
    )?;
    self.conn.execute(
        "INSERT INTO provider_snapshot_diagnostics (
            diagnostic_kind, collector_scope_id, replacement_scope_id,
            requested_provider_days_json, provider_day, reason_code, message,
            batch_id, run_id, recorded_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8)",
        params![
            diagnostic.diagnostic_kind,
            diagnostic.collector_scope_id,
            diagnostic.replacement_scope_id,
            requested_days_json,
            diagnostic.provider_day.map(|day| day.to_string()),
            diagnostic.reason_code,
            diagnostic.message,
            format_time(diagnostic.observed_at)?,
        ],
    )?;
    Ok(())
}
```

Implement `snapshot_totals_for_provider_day` with this state precedence:

```rust
// current: newest attempt is a complete run and active rows can be summed
// stale: newest attempt is diagnostic/blocked and older complete active rows exist
// blocked: newest attempt is diagnostic/blocked and no complete value exists
// missing: no attempt exists
```

Keep `complete zero-row` separate from missing by treating a complete run with no active rows as `SnapshotState::Current` and `DayTotals { total_tokens: 0.0 }`.

- [ ] **Step 5: Run snapshot tests**

Run:

```bash
cargo test --test usage_snapshots
cargo test --test storage_privacy snapshot_tables_exist_without_raw_transcript_columns
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/storage/usage_store.rs src/usage/snapshot.rs tests/usage_snapshots.rs tests/storage_privacy.rs
git commit -m "feat: store provider usage snapshots"
```

---

## Task 3: Add Source-Day-First Feed High-Water Evaluation

**Files:**
- Create: `src/usage/feed_high_water.rs`
- Modify: `src/usage/mod.rs`
- Modify: `src/storage/usage_store.rs`
- Modify: `src/game/runtime.rs`
- Modify: `src/usage/cutover.rs`
- Create: `tests/feed_high_water.rs`
- Modify: `tests/runtime_integration.rs`

**Interfaces:**
- Consumes: snapshot row inputs from Task 2.
- Produces: `UsageStore::source_has_feed_contact(token_contract, accounting_source) -> Result<bool>`
- Produces: `UsageStore::record_source_contact(token_contract, accounting_source, contact_kind, now) -> Result<()>`
- Produces: `UsageStore::feed_deltas_for_snapshot_rows(rows, now) -> Result<FeedHighWaterPlan>`
- Produces: `FeedHighWaterPlan { deltas, diagnostics, cursor_seeds }`
- Updates runtime staging so source first contact is source-level and provider-day high-waters start at zero for known sources.
- Updates Tokenmaxxing collector cutover so calibration cursor updates seed source contacts and source-day high-waters before the contract is activated.

- [ ] **Step 1: Write failing feed-high-water tests**

Create `tests/feed_high_water.rs`:

```rust
use glorp::{
    storage::usage_store::{ProviderCursorUpdate, UsageStore},
    usage::{
        normalize::RawTokenTotals,
        snapshot::ProviderSnapshotRowInput,
        token_contract::TOKENMAXXING_TOTAL_V1,
    },
};
use tempfile::tempdir;
use time::{macros::date, macros::datetime, Date, OffsetDateTime};

fn row(day: Date, model: &str, total: u64, buckets: RawTokenTotals) -> ProviderSnapshotRowInput {
    ProviderSnapshotRowInput {
        replacement_scope_id: "claude-code:local-usage".into(),
        collector_scope_id: "claude-code:local-usage".into(),
        collector_surface: "ccusage:claude-code".into(),
        command: "ccusage claude daily --json --offline".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: "claude-code".into(),
        provider_day: day,
        model: Some(model.into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some(format!("hash:{model}")),
        cursor_key_hash: format!("hash:{model}"),
        cursor_update: ProviderCursorUpdate {
            provider_surface: "claude-code".into(),
            cursor_key: format!("cursor:{model}"),
            cursor_value: serde_json::to_string(&buckets).unwrap(),
            provider_version: "ccusage 20.0.6".into(),
            parser_version: "ccusage 20.0.6".into(),
        },
        raw_token_buckets: Some(buckets),
        total_tokens: total as f64,
        cost_usd: None,
        confidence: "local-log-derived".into(),
    }
}

#[test]
fn known_source_new_day_feeds_from_zero_instead_of_first_contact_seeding() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 07 18:00 UTC);
    store
        .record_source_contact(TOKENMAXXING_TOTAL_V1, "claude-code", "source_first_contact", now)
        .unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 07),
                "claude-fable-5",
                100,
                RawTokenTotals {
                    uncached_input: 100,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            now,
        )
        .unwrap();

    assert_eq!(plan.deltas.len(), 1);
    assert_eq!(plan.deltas[0].total_tokens, 100.0);
    assert!(plan.cursor_seeds.is_empty());
}

#[test]
fn source_day_aggregate_highwater_blocks_model_remap_double_feed() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    store
        .seed_source_day_highwater_for_test(TOKENMAXXING_TOTAL_V1, "claude-code", date!(2026 - 07 - 06), 1_060.0, now)
        .unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 06),
                "renamed-model",
                531,
                RawTokenTotals {
                    uncached_input: 531,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            now,
        )
        .unwrap();

    assert!(plan.deltas.is_empty());
}

#[test]
fn mixed_bucket_rebound_feeds_total_only_without_token_shape() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    store
        .seed_exact_row_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 06),
            Some("claude-fable-5"),
            RawTokenTotals {
                uncached_input: 60,
                output: 40,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            },
            now,
        )
        .unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 06),
                "claude-fable-5",
                110,
                RawTokenTotals {
                    uncached_input: 50,
                    output: 60,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            now,
        )
        .unwrap();

    assert_eq!(plan.deltas.len(), 1);
    assert_eq!(plan.deltas[0].total_tokens, 10.0);
    assert_eq!(plan.deltas[0].confidence, "corrected-total-only");
    assert_eq!(plan.deltas[0].token_totals, None);
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "mixed_bucket_correction"));
}
```

- [ ] **Step 2: Run feed tests and verify failure**

Run:

```bash
cargo test --test feed_high_water
```

Expected: compile failure because the high-water APIs do not exist.

- [ ] **Step 3: Implement feed plan types**

Create `src/usage/feed_high_water.rs`:

```rust
use crate::storage::usage_store::{ProviderCursorUpdate, ProviderDiagnostic};
use crate::usage::provider::UsageDelta;

#[derive(Debug, Clone, PartialEq)]
pub struct FeedHighWaterPlan {
    pub deltas: Vec<UsageDelta>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub cursor_seeds: Vec<ProviderCursorUpdate>,
}

impl FeedHighWaterPlan {
    pub fn empty() -> Self {
        Self {
            deltas: Vec::new(),
            diagnostics: Vec::new(),
            cursor_seeds: Vec::new(),
        }
    }
}
```

Export it in `src/usage/mod.rs`:

```rust
pub mod feed_high_water;
```

- [ ] **Step 4: Implement source contact and high-water helpers**

In `src/storage/usage_store.rs`, add:

```rust
pub fn source_has_feed_contact(
    &self,
    token_contract: &str,
    accounting_source: &str,
) -> crate::error::Result<bool> {
    self.conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM provider_source_contacts
                WHERE token_contract = ?1 AND accounting_source = ?2
            )",
            params![token_contract, accounting_source],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(Into::into)
}

pub fn record_source_contact(
    &mut self,
    token_contract: &str,
    accounting_source: &str,
    contact_kind: &str,
    now: OffsetDateTime,
) -> crate::error::Result<()> {
    self.conn.execute(
        "INSERT INTO provider_source_contacts (
            token_contract, accounting_source, contact_kind, recorded_at
        ) VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(token_contract, accounting_source) DO UPDATE SET
            contact_kind = excluded.contact_kind,
            recorded_at = excluded.recorded_at",
        params![token_contract, accounting_source, contact_kind, format_time(now)?],
    )?;
    Ok(())
}
```

Add doc-hidden high-water seed helpers in the same file using `INSERT OR REPLACE INTO provider_feed_highwaters`. They are public test support because integration tests compile Glorp as a normal library dependency.
When inserting high-water rows, set `provider_day_key`, `model_key`, `provider_surface_key`, and `cursor_key_hash_key` to the corresponding nullable value or `""`. SQLite primary keys cannot use expression keys directly, so these key columns are the stable uniqueness surface.

```rust
#[doc(hidden)]
pub fn seed_source_day_highwater_for_test(
    &mut self,
    token_contract: &str,
    accounting_source: &str,
    provider_day: Date,
    total_high_water: f64,
    now: OffsetDateTime,
) -> crate::error::Result<()> {
    self.conn.execute(
        "INSERT OR REPLACE INTO provider_feed_highwaters (
            highwater_kind, token_contract, accounting_source, provider_day,
            provider_day_key, model, model_key, provider_surface,
            provider_surface_key, cursor_key_hash, cursor_key_hash_key, total_high_water,
            latest_raw_buckets_json, exact_raw_buckets_json, bucket_confidence,
            unshaped_total_only_tokens, updated_at
        ) VALUES (
            'source_day', ?1, ?2, ?3, ?3, NULL, '', NULL, '',
            NULL, '', ?4, NULL, NULL, 'exact', 0.0, ?5
        )",
        params![
            token_contract,
            accounting_source,
            provider_day.to_string(),
            total_high_water,
            format_time(now)?,
        ],
    )?;
    self.record_source_contact(token_contract, accounting_source, "test_seed", now)?;
    Ok(())
}

#[doc(hidden)]
pub fn seed_exact_row_highwater_for_test(
    &mut self,
    token_contract: &str,
    accounting_source: &str,
    provider_day: Date,
    model: Option<&str>,
    raw_buckets: RawTokenTotals,
    now: OffsetDateTime,
) -> crate::error::Result<()> {
    let model_key = model.unwrap_or("");
    self.seed_source_day_highwater_for_test(
        token_contract,
        accounting_source,
        provider_day,
        raw_buckets.total_tokens(),
        now,
    )?;
    self.conn.execute(
        "INSERT OR REPLACE INTO provider_feed_highwaters (
            highwater_kind, token_contract, accounting_source, provider_day,
            provider_day_key, model, model_key, provider_surface,
            provider_surface_key, cursor_key_hash, cursor_key_hash_key, total_high_water,
            latest_raw_buckets_json, exact_raw_buckets_json, bucket_confidence,
            unshaped_total_only_tokens, updated_at
        ) VALUES (
            'row', ?1, ?2, ?3, ?3, ?4, ?5, NULL, '',
            NULL, '', ?6, ?7, ?7, 'exact', 0.0, ?8
        )",
        params![
            token_contract,
            accounting_source,
            provider_day.to_string(),
            model,
            model_key,
            raw_buckets.total_tokens(),
            serde_json::to_string(&raw_buckets)?,
            format_time(now)?,
        ],
    )?;
    Ok(())
}
```

- [ ] **Step 5: Implement `feed_deltas_for_snapshot_rows` source-day-first**

In `src/storage/usage_store.rs`, implement `feed_deltas_for_snapshot_rows` with this structure:

```rust
pub fn feed_deltas_for_snapshot_rows(
    &mut self,
    rows: &[crate::usage::snapshot::ProviderSnapshotRowInput],
    now: OffsetDateTime,
) -> crate::error::Result<crate::usage::feed_high_water::FeedHighWaterPlan> {
    let mut plan = crate::usage::feed_high_water::FeedHighWaterPlan::empty();
    let mut groups: std::collections::BTreeMap<(String, String, Date), Vec<&crate::usage::snapshot::ProviderSnapshotRowInput>> =
        std::collections::BTreeMap::new();
    for row in rows {
        groups
            .entry((row.token_contract.clone(), row.accounting_source.clone(), row.provider_day))
            .or_default()
            .push(row);
    }

    for ((token_contract, accounting_source, provider_day), group_rows) in groups {
        if !self.source_has_feed_contact(&token_contract, &accounting_source)? {
            let seed_updates = group_rows
                .iter()
                .map(|row| row.cursor_update.clone())
                .collect::<Vec<_>>();
            self.record_source_contact(&token_contract, &accounting_source, "source_first_contact", now)?;
            plan.cursor_seeds.extend(seed_updates);
            continue;
        }

        let aggregate_total = group_rows.iter().map(|row| row.total_tokens.max(0.0)).sum::<f64>();
        let aggregate_highwater = self.source_day_highwater(&token_contract, &accounting_source, provider_day)?;
        let aggregate_excess = aggregate_total - aggregate_highwater;
        if aggregate_excess <= 0.0 {
            continue;
        }

        let exact_candidates = self.exact_feed_candidates(&group_rows)?;
        let exact_sum = exact_candidates.iter().map(|candidate| candidate.delta_total).sum::<f64>();
        if exact_candidates.len() == group_rows.len()
            && (exact_sum - aggregate_excess).abs() <= 0.000_001
        {
            plan.deltas.extend(exact_candidates.into_iter().map(|candidate| candidate.into_usage_delta()));
            self.advance_exact_highwaters(&group_rows, aggregate_total, now)?;
        } else {
            let delta = self.total_only_usage_delta(
                &token_contract,
                &accounting_source,
                provider_day,
                aggregate_excess,
                group_rows.first().expect("group_rows is non-empty"),
                now,
            )?;
            plan.diagnostics.push(ProviderDiagnostic {
                provider_surface: accounting_source.clone(),
                code: "mixed_bucket_correction".into(),
                message: format!("{accounting_source} emitted {aggregate_excess:.0} corrected-total-only tokens"),
                recorded_at: now,
            });
            plan.deltas.push(delta);
            self.advance_total_only_highwaters(&group_rows, aggregate_total, now)?;
        }
    }

    Ok(plan)
}
```

Implement the private helpers named above. Keep the source-day aggregate high-water as the guard. Exact row deltas are allowed only when every feeding row has complete raw buckets, row confidence is `exact`, raw buckets do not decrease below exact bucket high-water, and exact candidate sum equals aggregate excess.

- [ ] **Step 6: Update runtime first-contact handling and cutover seeding**

Modify `handle_first_contact_and_discontinuity` in `src/game/runtime.rs` so it uses `UsageStore::source_has_feed_contact` before treating a missing cursor as first contact. Existing `provider_cursor(None)` for a known source and a new provider day must not enter `first_contact_deltas`.

Add to `tests/runtime_integration.rs`:

```rust
#[test]
fn known_source_new_provider_day_is_not_seeded_as_first_contact() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    let day_one = datetime!(2026 - 07 - 06 12:00 UTC);
    let day_two = datetime!(2026 - 07 - 07 12:00 UTC);
    usage_store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            day_one,
        )
        .unwrap();

    let poll = poll_with_surface("claude-code", 100.0, day_two);
    let update = apply_usage_poll(&mut state, &mut usage_store, &poll, day_two).unwrap();

    assert_eq!(update.recent_effective_tokens, 100.0);
    assert_eq!(state.lifetime_effective_tokens, 100.0);
}
```

Add to `src/storage/usage_store.rs`:

```rust
pub fn seed_cutover_highwaters_from_cursor_updates(
    &mut self,
    token_contract: &str,
    cursor_updates: &[ProviderCursorUpdate],
    now: OffsetDateTime,
) -> crate::error::Result<()> {
    for update in cursor_updates {
        let key: crate::usage::provider::ProviderCursorKey = serde_json::from_str(&update.cursor_key)?;
        let buckets: RawTokenTotals = serde_json::from_str(&update.cursor_value)?;
        let provider_day = provider_day_from_cursor_period(&key.period_start)?;
        self.seed_source_day_highwater_for_cutover(
            token_contract,
            &update.provider_surface,
            provider_day,
            buckets.total_tokens(),
            now,
        )?;
    }
    Ok(())
}
```

Implement `provider_day_from_cursor_period` so it accepts both RFC3339 timestamps and plain `YYYY-MM-DD` provider-day strings:

```rust
fn provider_day_from_cursor_period(period: &str) -> crate::error::Result<Date> {
    if let Ok(timestamp) = period.parse::<OffsetDateTime>() {
        return Ok(crate::usage::day_axis::tokenmaxxing_provider_day(timestamp));
    }
    let (day, _) = crate::usage::day_axis::parse_agentsview_period_date(period)?;
    Ok(day)
}
```

Implement `seed_source_day_highwater_for_cutover` with the same uniqueness keys as `seed_source_day_highwater_for_test`, but keep it private and record source contact with `contact_kind = "cutover_calibration"`.

Modify `src/usage/cutover.rs` after cursor advancement:

```rust
usage_store.advance_cursors(snapshot.cursor_updates.clone(), now)?;
usage_store.seed_cutover_highwaters_from_cursor_updates(
    TOKENMAXXING_TOTAL_V1,
    &snapshot.cursor_updates,
    now,
)?;
```

Add to `tests/runtime_integration.rs`:

```rust
#[test]
fn tokenmaxxing_cutover_seeds_source_contact_and_source_day_highwater() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    let provider = provider_with_calibration_cursor(
        "claude-code",
        datetime!(2026 - 07 - 06 12:00 UTC),
        RawTokenTotals {
            uncached_input: 531,
            output: 0,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        },
    );

    ensure_tokenmaxxing_contract_active(&mut state, &mut usage_store, &provider, now).unwrap();

    assert!(usage_store
        .source_has_feed_contact(TOKENMAXXING_TOTAL_V1, "claude-code")
        .unwrap());
    assert_eq!(
        usage_store
            .source_day_highwater_for_test(TOKENMAXXING_TOTAL_V1, "claude-code", date!(2026 - 07 - 06))
            .unwrap(),
        531.0
    );
}
```

- [ ] **Step 7: Run feed and runtime tests**

Run:

```bash
cargo test --test feed_high_water
cargo test --test runtime_integration known_source_new_provider_day_is_not_seeded_as_first_contact
cargo test --test runtime_integration tokenmaxxing_cutover_seeds_source_contact_and_source_day_highwater
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/usage/feed_high_water.rs src/usage/mod.rs src/storage/usage_store.rs src/game/runtime.rs src/usage/cutover.rs tests/feed_high_water.rs tests/runtime_integration.rs
git commit -m "feat: add usage feed high-water accounting"
```

---

## Task 4: Integrate Providers With Snapshot-First Polling

**Files:**
- Modify: `src/usage/provider.rs`
- Modify: `src/usage/ccusage.rs`
- Modify: `src/usage/agentsview.rs`
- Modify: `tests/usage_provider.rs`
- Create: `tests/fixtures/helpers/ccusage-drop-day.mjs`
- Create: `tests/fixtures/helpers/ccusage-extra-day.mjs`
- Create: `tests/fixtures/helpers/ccusage-malformed-row.mjs`
- Create: `tests/fixtures/helpers/ccusage-model-remap.mjs`
- Create: `tests/fixtures/helpers/agentsview-drop-day.mjs`

**Interfaces:**
- Consumes: `UsageStore::write_provider_snapshot_batch`, `UsageStore::record_snapshot_failure`, and `UsageStore::feed_deltas_for_snapshot_rows`.
- Produces provider behavior: helper invocation -> requested-day snapshot write -> high-water feed plan -> `UsagePollResult`.
- Produces snapshot-only repair behavior: helper invocation -> requested-day snapshot write -> diagnostics, without feed high-water advancement.

- [ ] **Step 1: Add provider fixture helpers**

Create `tests/fixtures/helpers/ccusage-extra-day.mjs`:

```javascript
#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-07-06",
      model: "claude-fable-5",
      inputTokens: 100,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      cost: 0.01
    },
    {
      date: "2026-07-05",
      model: "claude-fable-5",
      inputTokens: 999999,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      cost: 0.01
    }
  ]
}));
```

Create `tests/fixtures/helpers/ccusage-drop-day.mjs`:

```javascript
#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({ daily: [] }));
```

Create `tests/fixtures/helpers/ccusage-malformed-row.mjs`:

```javascript
#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-07-06",
      model: "claude-fable-5",
      inputTokens: "not-a-number",
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0
    }
  ]
}));
```

Create `tests/fixtures/helpers/ccusage-model-remap.mjs`:

```javascript
#!/usr/bin/env node
if (process.argv.includes("--version")) {
  console.log("ccusage 20.0.6");
  process.exit(0);
}
console.log(JSON.stringify({
  daily: [
    {
      date: "2026-07-06",
      model: "claude-renamed",
      inputTokens: 531,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0
    }
  ]
}));
```

Create `tests/fixtures/helpers/agentsview-drop-day.mjs` with the same `daily: []` behavior but `agentsview v0.32.1` version output.

- [ ] **Step 2: Write failing provider tests**

Append to `tests/usage_provider.rs`:

```rust
fn provider_at(claude: Option<&str>, codex: Option<&str>, now: OffsetDateTime) -> CcusageCommandProvider {
    CcusageCommandProvider::new_with_now_for_test(
        HelperPaths {
            unified: None,
            claude: claude.map(fixture),
            codex: codex.map(fixture),
            node: None,
        },
        now,
    )
}

#[test]
fn provider_writes_snapshot_before_emitting_feed_deltas() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert!(result.total_tokens > 0.0);
    let today = time::Date::from_calendar_date(2026, time::Month::May, 9).unwrap();
    let snapshot = store.snapshot_totals_for_provider_day(today).unwrap();
    assert_eq!(snapshot.state, glorp::usage::snapshot::SnapshotState::Current);
    assert!(snapshot.value.unwrap().total_tokens > 0.0);
}

#[test]
fn unexpected_extra_provider_day_does_not_write_snapshot_or_feed() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-extra-day.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert!(result.total_tokens < 1_000.0);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unexpected_provider_day"));
    let extra_day = time::Date::from_calendar_date(2026, time::Month::July, 5).unwrap();
    let snapshot = store.snapshot_totals_for_provider_day(extra_day).unwrap();
    assert_eq!(snapshot.state, glorp::usage::snapshot::SnapshotState::Missing);
}

#[test]
fn disappeared_requested_provider_day_writes_current_zero_without_negative_food() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let first_provider = provider_at(
        Some("ccusage-extra-day.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );
    first_provider.refresh_snapshots_only(&mut store).unwrap();
    let second_provider = provider_at(
        Some("ccusage-drop-day.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = second_provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    let requested_day = date!(2026 - 07 - 06);
    let snapshot = store.snapshot_totals_for_provider_day(requested_day).unwrap();
    assert_eq!(snapshot.state, glorp::usage::snapshot::SnapshotState::Current);
    assert_eq!(snapshot.value.unwrap().total_tokens, 0.0);
}

#[test]
fn malformed_requested_row_blocks_snapshot_and_does_not_feed_valid_looking_rows() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider_at(
        Some("ccusage-malformed-row.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
}
```

- [ ] **Step 3: Run provider tests and verify failure**

Run:

```bash
cargo test --test usage_provider provider_writes_snapshot_before_emitting_feed_deltas
cargo test --test usage_provider unexpected_extra_provider_day_does_not_write_snapshot_or_feed
cargo test --test usage_provider disappeared_requested_provider_day_writes_current_zero_without_negative_food
cargo test --test usage_provider malformed_requested_row_blocks_snapshot_and_does_not_feed_valid_looking_rows
```

Expected: FAIL because providers do not write snapshots yet.

- [ ] **Step 4: Add requested-day scoping to provider flow**

In `src/usage/provider.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSnapshotScope {
    pub collector_scope_id: String,
    pub replacement_scope_id: String,
    pub requested_provider_days: Vec<time::Date>,
}
```

Add to `UsageProvider`:

```rust
fn refresh_snapshots_only(&self, store: &mut UsageStore) -> Result<Vec<ProviderDiagnostic>>;
```

Add a doc-hidden test constructor to `CcusageCommandProvider`:

```rust
#[doc(hidden)]
pub fn new_with_now_for_test(paths: HelperPaths, now: OffsetDateTime) -> Self {
    Self::new_with_clock(paths, move || now)
}
```

Keep the production constructor on `OffsetDateTime::now_utc`. Use the injected clock only to compute requested provider days; do not use it for helper subprocess timeouts or filesystem behavior.

In `src/usage/ccusage.rs`, compute requested days for normal polling:

```rust
fn requested_provider_days_for_poll(now: OffsetDateTime) -> Vec<time::Date> {
    vec![crate::usage::day_axis::tokenmaxxing_provider_day(now)]
}
```

Use the same helper in `src/usage/agentsview.rs`. Filter normalized records to requested days before snapshot write. For records outside the requested set, persist a `ProviderSnapshotDiagnosticInput` with `diagnostic_kind = "unexpected_provider_day"` and exclude the record from feed evaluation.

- [ ] **Step 5: Replace direct cursor-delta emission with snapshot-first evaluation**

In both providers:

1. Invoke helper.
2. Normalize records.
3. Convert requested-day records to `ProviderSnapshotRowInput`.
4. Write `UsageStore::write_provider_snapshot_batch`.
5. Call `UsageStore::feed_deltas_for_snapshot_rows` with rows from complete, unblocked requested runs.
6. Return `UsagePollResult` from the feed plan deltas and diagnostics.

Keep helper version metadata cursor writes for doctor output, but do not let metadata cursors feed.

Implement `refresh_snapshots_only` in both providers with the same requested-day scoping, parsing, diagnostics, and snapshot writes as `poll`, but do not call `feed_deltas_for_snapshot_rows` and do not advance provider feed high-waters. A successful helper response with no row for the requested day is a complete zero-row snapshot. A row for an unrequested day is an `unexpected_provider_day` diagnostic and must not write a snapshot for that unrequested day.

- [ ] **Step 6: Run provider tests**

Run:

```bash
cargo test --test usage_provider
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/usage/provider.rs src/usage/ccusage.rs src/usage/agentsview.rs tests/usage_provider.rs tests/fixtures/helpers/ccusage-drop-day.mjs tests/fixtures/helpers/ccusage-extra-day.mjs tests/fixtures/helpers/ccusage-malformed-row.mjs tests/fixtures/helpers/ccusage-model-remap.mjs tests/fixtures/helpers/agentsview-drop-day.mjs
git commit -m "feat: write provider snapshots before feeding"
```

---

## Task 5: Migrate Watch View Model To Snapshot-Backed Visible Accounting

**Files:**
- Modify: `src/tui/view_model.rs`
- Modify: `src/commands/watch.rs`
- Modify: `tests/watch_integration.rs`

**Interfaces:**
- Consumes: `UsageStore::snapshot_totals_for_provider_day`, `snapshot_totals_by_source_for_provider_day`, `snapshot_token_history_for_provider_days`, and `snapshot_health_for_provider_day`.
- Produces: `WatchViewModel.today_snapshot_state`, `SourceHealthView.snapshot_state`, and snapshot-backed today/source/history values.
- Keeps feed-ledger APIs for `current_bucket_effective_tokens`, `rate_momentum`, recent events, and token-shape personality.

- [ ] **Step 1: Write failing watch tests**

Append to `tests/watch_integration.rs`:

```rust
#[test]
fn legacy_applied_tokenmaxxing_rows_do_not_inflate_snapshot_today() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    usage
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            total_tokens: 1_060.0,
            effective_tokens: 1_060.0,
            ..NormalizedUsageEvent::for_test_at(now, 1_060.0)
        })
        .unwrap();
    seed_snapshot_for_test(&mut usage, time::macros::date!(2026 - 07 - 06), "claude-code", 531.0, now);

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(
        vm.today_effective_tokens, 531.0,
        "legacy applied tokenmaxxing rows must not inflate snapshot-backed provider truth"
    );
    assert_eq!(vm.current_bucket_effective_tokens, 1_060.0);
}

#[test]
fn missing_snapshot_does_not_render_zero_provider_truth() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let _usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(vm.today_snapshot_state, glorp::usage::snapshot::SnapshotState::Missing);
    assert_eq!(vm.today_effective_tokens, 0.0);
    assert!(vm
        .source_health
        .iter()
        .all(|source| source.snapshot_state != glorp::usage::snapshot::SnapshotState::Current));
}
```

Add the local helper:

```rust
fn seed_snapshot_for_test(
    usage: &mut UsageStore,
    day: time::Date,
    source: &str,
    total: f64,
    observed_at: OffsetDateTime,
) {
    let batch = glorp::usage::snapshot::ProviderSnapshotBatchInput {
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        provider_version: "test".into(),
        parser_version: "test".into(),
        observed_at,
    };
    let row = glorp::usage::snapshot::ProviderSnapshotRowInput {
        replacement_scope_id: format!("{source}:local-usage"),
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: source.into(),
        provider_day: day,
        model: Some("test-model".into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some("hash:test".into()),
        cursor_key_hash: "hash:cursor".into(),
        cursor_update: ProviderCursorUpdate {
            provider_surface: source.into(),
            cursor_key: "cursor".into(),
            cursor_value: "value".into(),
            provider_version: "test".into(),
            parser_version: "test".into(),
        },
        raw_token_buckets: None,
        total_tokens: total,
        cost_usd: None,
        confidence: "local-log-derived".into(),
    };
    usage.write_provider_snapshot_batch(&batch, &[row], &[]).unwrap();
}
```

- [ ] **Step 2: Run watch tests and verify failure**

Run:

```bash
cargo test --test watch_integration legacy_applied_tokenmaxxing_rows_do_not_inflate_snapshot_today
cargo test --test watch_integration missing_snapshot_does_not_render_zero_provider_truth
```

Expected: compile failure or assertion failure because `WatchViewModel` has no snapshot state and watch still reads feed-ledger totals.

- [ ] **Step 3: Add snapshot state fields to view model**

Modify `src/tui/view_model.rs` by adding these fields to `WatchViewModel` immediately after `today_effective_tokens`:

```rust
pub today_snapshot_state: crate::usage::snapshot::SnapshotState,
pub today_snapshot_reason: Option<String>,
```

Add these fields to `SourceHealthView` immediately after `status`:

```rust
pub snapshot_state: crate::usage::snapshot::SnapshotState,
pub snapshot_reason: Option<String>,
```

Update fixture constructors with `SnapshotState::Current` for populated fixture rows and `None` reason.

- [ ] **Step 4: Update watch builder queries**

In `build_watch_view_model_at`:

```rust
let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(now);
let today_snapshot = usage_store.snapshot_totals_for_provider_day(provider_day)?;
let source_snapshot = usage_store.snapshot_totals_by_source_for_provider_day(provider_day)?;
let today_total_tokens = today_snapshot
    .value
    .as_ref()
    .map(|totals| totals.total_tokens)
    .unwrap_or(0.0);
let today_totals = source_snapshot
    .value
    .as_ref()
    .map(|totals| {
        totals
            .sources
            .iter()
            .map(|source| (source.accounting_source.clone(), source.total_tokens))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
```

Keep:

```rust
let last_10m_totals = usage_store
    .canonical_total_tokens_by_source_between(last_10m_start, window_end)
    .unwrap_or_default();
let pulse_window = build_rate_window(&usage_store, rate_anchor, Duration::minutes(10));
let hour_window = build_rate_window(&usage_store, rate_anchor, Duration::hours(1));
```

Set:

```rust
today_snapshot_state: today_snapshot.state,
today_snapshot_reason: today_snapshot.reason.clone(),
```

Update `source_health` to accept `Vec<SourceSnapshotHealth>` and build health rows from configured snapshot health plus recent accepted-food totals.

- [ ] **Step 5: Run watch tests**

Run:

```bash
cargo test --test watch_integration
cargo test --test watch_presentation_adapter
```

Expected: PASS. Snapshot output changes that affect preview snapshots are handled in Task 7.

- [ ] **Step 6: Commit**

```bash
git add src/tui/view_model.rs src/commands/watch.rs tests/watch_integration.rs
git commit -m "feat: show snapshot usage in watch"
```

---

## Task 6: Update Status, Doctor, And Repair Flow

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/lib.rs`
- Modify: `src/commands/status.rs`
- Modify: `src/commands/doctor.rs`
- Modify: `tests/doctor_status.rs`

**Interfaces:**
- Consumes: snapshot query APIs and provider snapshot poll flow.
- Produces: `glorp doctor --refresh-usage-snapshots`.
- Produces status output with provider today, accepted recent food, and pet lifetime food as separate concepts.

- [ ] **Step 1: Write failing CLI/status/doctor tests**

Append to `tests/doctor_status.rs`:

```rust
#[test]
fn status_labels_provider_today_recent_food_and_pet_lifetime_separately() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_EMPTY)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_EMPTY)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider today"))
        .stdout(predicate::str::contains("accepted recent food"))
        .stdout(predicate::str::contains("pet lifetime food"));
}

#[test]
fn doctor_refresh_usage_snapshots_reports_before_after_without_feeding_pet() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_EMPTY)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_EMPTY)
        .args(["doctor", "--refresh-usage-snapshots"])
        .assert()
        .success()
        .stdout(predicate::str::contains("refresh usage snapshots"))
        .stdout(predicate::str::contains("before provider today"))
        .stdout(predicate::str::contains("after provider today"))
        .stdout(predicate::str::contains("pet state unchanged"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test doctor_status status_labels_provider_today_recent_food_and_pet_lifetime_separately
cargo test --test doctor_status doctor_refresh_usage_snapshots_reports_before_after_without_feeding_pet
```

Expected: FAIL because the output labels and doctor flag do not exist.

- [ ] **Step 3: Add doctor CLI flag**

Modify `src/cli.rs`:

```rust
/// Inspect helper availability, config paths, parser health, and diagnostics.
Doctor {
    #[arg(long)]
    refresh_usage_snapshots: bool,
},
```

Modify `src/lib.rs`:

```rust
Command::Doctor { refresh_usage_snapshots } => {
    commands::doctor::run(refresh_usage_snapshots)?
}
```

- [ ] **Step 4: Update status labels**

In `src/commands/status.rs`, change `run()` to read snapshot state for provider today and feed ledger for recent food:

```rust
let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(status_now);
let snapshot = usage_store.snapshot_totals_for_provider_day(provider_day)?;
today_effective = snapshot
    .value
    .as_ref()
    .map(|totals| totals.total_tokens)
    .unwrap_or(0.0);
usage_confidence = match snapshot.state {
    crate::usage::snapshot::SnapshotState::Current => "provider snapshot".into(),
    crate::usage::snapshot::SnapshotState::Stale => "stale provider snapshot".into(),
    crate::usage::snapshot::SnapshotState::Missing => "snapshot pending".into(),
    crate::usage::snapshot::SnapshotState::Blocked => "snapshot blocked".into(),
};
```

Replace the token line with:

```rust
println!(
    "provider today ({usage_confidence}): {:.0}",
    display_tokens(today_effective)
);
println!("accepted recent food: {:.0}", display_tokens(recent_effective));
println!(
    "pet lifetime food: {:.0}",
    display_tokens(state.lifetime_effective_tokens)
);
```

- [ ] **Step 5: Add doctor repair path**

Change `src/commands/doctor.rs` signature:

```rust
pub fn run(refresh_usage_snapshots: bool) -> Result<()> {
```

At the end of doctor, call:

```rust
if refresh_usage_snapshots {
    refresh_usage_snapshots_for_doctor(&mut usage_store)?;
}
```

Implement:

```rust
fn refresh_usage_snapshots_for_doctor(usage_store: &mut UsageStore) -> Result<()> {
    let now = OffsetDateTime::now_utc();
    let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(now);
    let before = usage_store.snapshot_totals_for_provider_day(provider_day)?;
    let provider = CcusageCommandProvider::from_environment();
    let diagnostics = provider.refresh_snapshots_only(usage_store)?;
    let after = usage_store.snapshot_totals_for_provider_day(provider_day)?;

    println!("refresh usage snapshots: requested {provider_day}");
    println!(
        "before provider today: {}",
        before
            .value
            .as_ref()
            .map(|totals| format!("{:.0}", totals.total_tokens))
            .unwrap_or_else(|| format!("{:?}", before.state))
    );
    println!(
        "after provider today: {}",
        after
            .value
            .as_ref()
            .map(|totals| format!("{:.0}", totals.total_tokens))
            .unwrap_or_else(|| format!("{:?}", after.state))
    );
    if diagnostics.is_empty() {
        println!("blocked provider scopes: none");
    } else {
        for diagnostic in diagnostics {
            println!("blocked provider scope: {} {}", diagnostic.provider_surface, diagnostic.code);
        }
    }
    println!("pet state unchanged");
    Ok(())
}
```

Do not call `stage_usage_poll_deltas`, `apply_unapplied_usage`, or state save in the repair path.

- [ ] **Step 6: Run command tests**

Run:

```bash
cargo test --test doctor_status
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/lib.rs src/commands/status.rs src/commands/doctor.rs tests/doctor_status.rs
git commit -m "feat: expose usage snapshot diagnostics"
```

---

## Task 7: Snapshot History, Preview Fixtures, And Activity Identity Wiring

**Files:**
- Modify: `src/storage/usage_store.rs`
- Modify: `src/commands/watch.rs`
- Modify: `src/tui/view_model.rs`
- Modify: `src/dev_preview/watch.rs`
- Modify: `tests/watch_integration.rs`
- Modify: `tests/activity_identity_runtime.rs`
- Modify: `tests/snapshots/dev_preview__watch_wide_normal_frame.snap`
- Modify: `tests/snapshots/dev_preview__watch_daycontext_heavy_day_evening_frame.snap`
- Modify: `tests/snapshots/dev_preview__watch_daycontext_night_asleep_frame.snap`

**Interfaces:**
- Consumes: snapshot APIs from Tasks 2 and 5.
- Produces: 7-day visible history with unavailable missing days, Activity Identity snapshot traits from provider truth, and feed-ledger token-shape/rhythm/recovery.

- [ ] **Step 1: Write failing 7-day and activity identity tests**

Append to `tests/watch_integration.rs`:

```rust
#[test]
fn seven_day_history_uses_snapshot_days_and_degrades_missing_days() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    seed_snapshot_for_test(&mut usage, time::macros::date!(2026 - 07 - 06), "claude-code", 531.0, now);

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(vm.recent_daily_effective_tokens.last().copied(), Some(531.0));
    assert!(vm.recent_daily_snapshot_states.iter().any(|state| {
        *state == glorp::usage::snapshot::SnapshotState::Missing
    }));
}
```

Append to `tests/activity_identity_runtime.rs`:

```rust
#[test]
fn activity_identity_source_diversity_uses_snapshot_but_shape_uses_feed_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = time::macros::datetime!(2026 - 07 - 06 20:00 UTC);
    seed_snapshot_for_activity_test(&mut usage, time::macros::date!(2026 - 07 - 06), now);
    usage
        .insert_event(&glorp::storage::usage_store::NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            input_tokens: 0.0,
            output_tokens: 100.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 0.0,
            total_tokens: 100.0,
            effective_tokens: 100.0,
            ..glorp::storage::usage_store::NormalizedUsageEvent::for_test_at(now, 100.0)
        })
        .unwrap();

    let state = glorp::storage::state::PetState::new_for_test("identity", "miso");
    let vm = glorp::commands::watch::build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

    assert!(vm.activity_identity.source_diversity.active_sources >= 2);
    assert!(matches!(
        vm.activity_identity.token_shape,
        glorp::tui::identity::TokenShapePersonality::OutputHeavy
    ));
}

fn seed_snapshot_for_activity_test(
    usage: &mut glorp::storage::usage_store::UsageStore,
    day: time::Date,
    observed_at: time::OffsetDateTime,
) {
    let batch = glorp::usage::snapshot::ProviderSnapshotBatchInput {
        collector_scope_id: "test:local-usage".into(),
        collector_surface: "test".into(),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        provider_version: "test".into(),
        parser_version: "test".into(),
        observed_at,
    };
    let rows = ["claude-code", "codex"]
        .iter()
        .map(|source| glorp::usage::snapshot::ProviderSnapshotRowInput {
            replacement_scope_id: "test:local-usage".into(),
            collector_scope_id: "test:local-usage".into(),
            collector_surface: "test".into(),
            command: "test snapshot".into(),
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            accounting_source: (*source).into(),
            provider_day: day,
            model: Some(format!("{source}-model")),
            source_surface: "daily".into(),
            provider_period: day.to_string(),
            raw_source_id_hash: Some(format!("hash:{source}")),
            cursor_key_hash: format!("hash:{source}"),
            cursor_update: glorp::storage::usage_store::ProviderCursorUpdate {
                provider_surface: (*source).into(),
                cursor_key: format!("cursor:{source}"),
                cursor_value: "value".into(),
                provider_version: "test".into(),
                parser_version: "test".into(),
            },
            raw_token_buckets: None,
            total_tokens: 100.0,
            cost_usd: None,
            confidence: "local-log-derived".into(),
        })
        .collect::<Vec<_>>();
    usage.write_provider_snapshot_batch(&batch, &rows, &[]).unwrap();
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test watch_integration seven_day_history_uses_snapshot_days_and_degrades_missing_days
cargo test --test activity_identity_runtime activity_identity_source_diversity_uses_snapshot_but_shape_uses_feed_ledger
```

Expected: FAIL because history state and snapshot-backed source diversity are not wired.

- [ ] **Step 3: Add history state to view model**

Modify `src/tui/view_model.rs` by adding this field to `WatchViewModel` immediately after `recent_daily_effective_tokens`:

```rust
pub recent_daily_snapshot_states: Vec<crate::usage::snapshot::SnapshotState>,
```

Update fixtures with seven `SnapshotState::Current` entries.

- [ ] **Step 4: Wire snapshot history and source diversity**

In `src/commands/watch.rs`, replace the history assignment:

```rust
let provider_days = crate::usage::day_axis::tokenmaxxing_days_back(now, 7);
let history = usage_store
    .snapshot_token_history_for_provider_days(&provider_days)
    .unwrap_or_default();
let recent_daily_effective_tokens = history
    .iter()
    .map(|result| result.value.as_ref().map(|day| day.total_tokens).unwrap_or(0.0))
    .collect();
let recent_daily_snapshot_states = history.iter().map(|result| result.state).collect();
```

Keep `token_shape`, `rhythm`, and `recovery` on existing applied/feed-ledger reads.

- [ ] **Step 5: Refresh dev preview snapshots**

Run:

```bash
cargo run --features dev-preview -- dev-preview --scenario watch --out target/glorp-preview
cargo insta review
```

Accept only the snapshot changes that follow from visible provider-truth labels/states. Do not accept unrelated art/layout changes.

- [ ] **Step 6: Run preview and watch checks**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview::scenarios
cargo test --test watch_integration
cargo test --test activity_identity_runtime
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/storage/usage_store.rs src/commands/watch.rs src/tui/view_model.rs src/dev_preview/watch.rs tests/watch_integration.rs tests/activity_identity_runtime.rs tests/snapshots/dev_preview__watch_wide_normal_frame.snap tests/snapshots/dev_preview__watch_daycontext_heavy_day_evening_frame.snap tests/snapshots/dev_preview__watch_daycontext_night_asleep_frame.snap
git commit -m "feat: use usage snapshots in watch history"
```

---

## Task 8: End-To-End Regression And Cleanup

**Files:**
- Modify: `tests/runtime_integration.rs`
- Modify: `tests/watch_integration.rs`
- Modify: `tests/doctor_status.rs`
- Modify: `tests/storage_privacy.rs`
- Modify: `docs/superpowers/specs/2026-07-06-glorp-usage-snapshot-corrections-design.md`

**Interfaces:**
- Consumes every earlier task.
- Produces full regression coverage for the overcount failure and final implementation notes in the design spec.

- [ ] **Step 1: Add overcount regression test**

Append to `tests/runtime_integration.rs`:

```rust
#[test]
fn provider_correction_updates_visible_truth_without_rolling_back_pet_progress() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.lifetime_effective_tokens = 1_060_000_000.0;
    state.xp = 5.0;
    state.stage = Stage::S5;
    let now = datetime!(2026 - 07 - 06 20:00 UTC);

    seed_snapshot_for_runtime_test(
        &mut usage_store,
        time::macros::date!(2026 - 07 - 06),
        "claude-code",
        531_000_000.0,
        now,
    );

    let visible = usage_store
        .snapshot_totals_for_provider_day(time::macros::date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(visible.value.unwrap().total_tokens, 531_000_000.0);
    assert_eq!(state.lifetime_effective_tokens, 1_060_000_000.0);
    assert_eq!(state.stage, Stage::S5);
}

fn seed_snapshot_for_runtime_test(
    usage: &mut UsageStore,
    day: time::Date,
    source: &str,
    total: f64,
    observed_at: time::OffsetDateTime,
) {
    let batch = glorp::usage::snapshot::ProviderSnapshotBatchInput {
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        provider_version: "test".into(),
        parser_version: "test".into(),
        observed_at,
    };
    let row = glorp::usage::snapshot::ProviderSnapshotRowInput {
        replacement_scope_id: format!("{source}:local-usage"),
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: source.into(),
        provider_day: day,
        model: Some("test-model".into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some("hash:test".into()),
        cursor_key_hash: "hash:cursor".into(),
        cursor_update: ProviderCursorUpdate {
            provider_surface: source.into(),
            cursor_key: "cursor".into(),
            cursor_value: "value".into(),
            provider_version: "test".into(),
            parser_version: "test".into(),
        },
        raw_token_buckets: None,
        total_tokens: total,
        cost_usd: None,
        confidence: "local-log-derived".into(),
    };
    usage.write_provider_snapshot_batch(&batch, &[row], &[]).unwrap();
}
```

- [ ] **Step 2: Add privacy regression for corrections and diagnostics**

Append to `tests/storage_privacy.rs`:

```rust
#[test]
fn snapshot_corrections_and_diagnostics_do_not_store_raw_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = time::macros::date!(2026 - 07 - 06);
    store
        .record_snapshot_failure(&glorp::usage::snapshot::ProviderSnapshotDiagnosticInput {
            diagnostic_kind: "helper_exit".into(),
            collector_scope_id: "claude-code:local-usage".into(),
            replacement_scope_id: None,
            requested_provider_days: vec![day],
            provider_day: Some(day),
            reason_code: "helper_exit".into(),
            message: "helper_exit sanitized".into(),
            observed_at: time::macros::datetime!(2026 - 07 - 06 20:00 UTC),
        })
        .unwrap();

    let rendered: String = store
        .raw_connection_for_test()
        .query_row(
            "SELECT IFNULL(group_concat(message, '\n'), '') FROM provider_snapshot_diagnostics",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
    assert!(!rendered.contains("/Users/drew/private"));
}
```

- [ ] **Step 3: Update design spec status**

In `docs/superpowers/specs/2026-07-06-glorp-usage-snapshot-corrections-design.md`, change:

```markdown
- Status: proposed, revised after adversarial review
```

to:

```markdown
- Status: implemented
```

Add a short implementation note under the status:

```markdown
- Implementation plan: `docs/superpowers/plans/2026-07-06-glorp-usage-snapshot-corrections-implementation.md`
```

- [ ] **Step 4: Run focused suites**

Run:

```bash
cargo test --test usage_snapshots
cargo test --test feed_high_water
cargo test --test usage_provider
cargo test --test runtime_integration
cargo test --test watch_integration
cargo test --test doctor_status
cargo test --test storage_privacy
```

Expected: PASS.

- [ ] **Step 5: Run broad local checks**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
npm test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/runtime_integration.rs tests/watch_integration.rs tests/doctor_status.rs tests/storage_privacy.rs docs/superpowers/specs/2026-07-06-glorp-usage-snapshot-corrections-design.md
git commit -m "test: cover usage snapshot corrections"
```

---

## Self-Review Checklist

- [ ] Storage has complete requested-day snapshot runs, zero-row runs, blocked attempts, stale state, corrections, diagnostics, and privacy constraints.
- [ ] Providers write snapshots before feed deltas and exclude unexpected extra days and blocked runs from feed.
- [ ] Feed evaluation is source-day-first and guarded by source-day aggregate high-water.
- [ ] Source first contact is source-level, while new days for known sources feed from zero.
- [ ] Watch/status provider truth reads snapshots; rate/recent/lifetime pet food stays ledger-backed.
- [ ] Doctor repair refreshes snapshots without applying pet food or saving pet state.
- [ ] Tests cover the July 6 overcount shape, disappeared rows, disappeared days, rebound, model remap with unchanged source-day aggregate, legacy applied tokenmaxxing rows, mixed-bucket total-only food, missing snapshot state, and privacy.
