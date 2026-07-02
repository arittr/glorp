use crate::error::Result;
use crate::game::metabolism::RhythmProfile;
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
    // Advance cursors regardless of diagnostics. Benign diagnostics (e.g. a
    // malformed model breakdown alongside valid records) leave cursor_updates
    // populated with the current totals; skipping advance_cursors here would
    // leave cursors at zero so the next clean poll applies the full usage history
    // as a bolus. Cursor advancement is bolus-prevention and must always run.
    usage_store.advance_cursors(snapshot.cursor_updates, now)?;
    if !snapshot.diagnostics.is_empty() {
        // Cannot compute a reliable baseline with a degraded snapshot. Leave the
        // contract inactive so a later clean poll computes the baseline and activates.
        return Ok(CutoverOutcome { activated: false });
    }
    if !snapshot.daily_usage.is_empty() {
        state.calibration = state
            .calibration
            .refresh_from_history(&snapshot.daily_usage);
        state.rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
    }
    usage_store.mark_token_contract_active(TOKENMAXXING_TOTAL_V1, now)?;

    Ok(CutoverOutcome { activated: true })
}
