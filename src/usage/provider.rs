use crate::error::Result;
use crate::storage::usage_store::UsageStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct UsagePollResult {
    pub deltas: Vec<UsageDelta>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub total_effective_tokens: f64,
}

#[derive(Debug, Clone)]
pub struct UsageDelta {
    pub provider_surface: String,
    pub effective_tokens: f64,
    pub confidence: String,
    pub period_start: String,
    pub model: Option<String>,
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
    pub parser_version: String,
    pub period_start: String,
    pub model: Option<String>,
}

pub trait UsageProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult>;
}
