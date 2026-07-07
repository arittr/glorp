use crate::storage::usage_store::{ProviderCursorUpdate, ProviderDiagnostic};
use crate::usage::normalize::RawTokenTotals;
use crate::usage::provider::UsageDelta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FeedHighWaterUpdate {
    SourceDay(FeedSourceDayHighWaterUpdate),
    Row(FeedRowHighWaterUpdate),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedSourceDayHighWaterUpdate {
    pub token_contract: String,
    pub accounting_source: String,
    pub provider_day: String,
    pub total_high_water: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedRowHighWaterUpdate {
    pub token_contract: String,
    pub accounting_source: String,
    pub provider_day: String,
    pub model: Option<String>,
    pub total_high_water: f64,
    pub latest_raw_buckets: Option<RawTokenTotals>,
    pub exact_raw_buckets: Option<RawTokenTotals>,
    pub bucket_confidence: String,
    pub unshaped_total_only_tokens: f64,
}

#[derive(Debug, Clone)]
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
