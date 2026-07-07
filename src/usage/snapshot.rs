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
    pub covered_accounting_sources: Option<Vec<String>>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSnapshotWriteOutcome {
    pub batch_id: i64,
    pub complete_run_ids: Vec<i64>,
    pub blocked_run_ids: Vec<i64>,
}
