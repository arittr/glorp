use crate::error::Result;
use crate::storage::usage_store::{ProviderCursorUpdate, UsageStore};
use crate::usage::identity::SourceIdentity;
use crate::usage::normalize::RawTokenTotals;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct UsagePollResult {
    pub deltas: Vec<UsageDelta>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub total_effective_tokens: f64,
}

#[derive(Debug, Clone)]
pub struct UsageDelta {
    /// Stable provider surface used for grouping and storage lookups. Mirrors
    /// `source_identity.provider_surface` for convenient downstream access.
    pub provider_surface: String,
    /// Resolved identity of the source that produced this usage.
    pub source_identity: SourceIdentity,
    pub command: String,
    pub effective_tokens: f64,
    pub confidence: String,
    pub period_start: OffsetDateTime,
    pub observed_at: OffsetDateTime,
    pub model: Option<String>,
    pub cursor_update: ProviderCursorUpdate,
    pub token_totals: Option<RawTokenTotals>,
}

#[derive(Debug, Clone)]
pub struct UsageSnapshot {
    pub daily_usage: Vec<crate::game::calibration::DailyUsage>,
    pub cursor_updates: Vec<ProviderCursorUpdate>,
    pub diagnostics: Vec<ProviderDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct ProviderDiagnostic {
    pub provider_surface: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCursorKey {
    pub provider_surface: String,
    pub command: String,
    pub source_surface: String,
    pub period_start: String,
    pub model: Option<String>,
}

pub trait UsageProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult>;
    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot>;
}
