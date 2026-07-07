use crate::storage::usage_store::{ProviderCursorUpdate, ProviderDiagnostic};
use crate::usage::provider::UsageDelta;

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
