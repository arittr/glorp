use crate::error::Result;
use crate::storage::usage_store::{ProviderCursorUpdate, UsageStore};
use crate::usage::feed_high_water::FeedHighWaterUpdate;
use crate::usage::identity::SourceIdentity;
use crate::usage::normalize::RawTokenTotals;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct UsagePollResult {
    pub deltas: Vec<UsageDelta>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub total_effective_tokens: f64,
    pub total_tokens: f64,
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
    pub total_tokens: f64,
    pub token_contract: String,
    pub confidence: String,
    pub period_start: OffsetDateTime,
    pub observed_at: OffsetDateTime,
    pub model: Option<String>,
    pub cursor_update: ProviderCursorUpdate,
    pub token_totals: Option<RawTokenTotals>,
    pub feed_highwater_updates: Vec<FeedHighWaterUpdate>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSnapshotScope {
    pub collector_scope_id: String,
    pub replacement_scope_id: String,
    pub requested_provider_days: Vec<time::Date>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCursorKey {
    pub provider_surface: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_contract: Option<String>,
    pub command: String,
    pub source_surface: String,
    pub period_start: String,
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_source_id: Option<String>,
}

pub trait UsageProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult>;
    fn refresh_snapshots_only(&self, store: &mut UsageStore) -> Result<Vec<ProviderDiagnostic>>;
    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_key(raw_source_id: Option<&str>) -> ProviderCursorKey {
        ProviderCursorKey {
            provider_surface: "gemini".into(),
            token_contract: None,
            command: "ccusage daily".into(),
            source_surface: "daily".into(),
            period_start: "2026-06-11".into(),
            model: Some("gemini-2.5-pro".into()),
            raw_source_id: raw_source_id.map(str::to_string),
        }
    }

    #[test]
    fn cursor_key_omits_none_raw_source_id_and_parses_legacy_keys() {
        let key = sample_key(None);
        let json = serde_json::to_string(&key).unwrap();
        assert!(!json.contains("raw_source_id"));

        let legacy = r#"{"provider_surface":"claude-code","command":"ccusage","source_surface":"daily","period_start":"2026-06-11","model":"claude-opus-4"}"#;
        let parsed: ProviderCursorKey = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed.raw_source_id, None);
    }

    #[test]
    fn raw_source_id_changes_cursor_identity() {
        let base = serde_json::to_string(&sample_key(None)).unwrap();
        let with_id = serde_json::to_string(&sample_key(Some("harness-abc"))).unwrap();
        assert_ne!(base, with_id);
    }
}
