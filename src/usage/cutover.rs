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
    if !snapshot.diagnostics.is_empty() {
        return Ok(CutoverOutcome { activated: false });
    }
    if !snapshot.daily_usage.is_empty() {
        state.calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
        state.rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
    }
    usage_store.advance_cursors(snapshot.cursor_updates, now)?;
    usage_store.mark_token_contract_active(TOKENMAXXING_TOTAL_V1, now)?;

    Ok(CutoverOutcome { activated: true })
}
