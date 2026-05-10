use time::{Duration, OffsetDateTime};

use crate::{
    error::Result,
    game::{
        evolution::{apply_xp_delta, Stage, StageTransition},
        metabolism::{apply_decay, apply_food, MetabolismResult},
    },
    storage::{
        state::{PetState, Vitals as StoredVitals},
        usage_store::{NormalizedUsageEvent, UsageStore},
    },
    usage::provider::UsagePollResult,
};

const USAGE_RETENTION_DAYS: i64 = 90;
const RECENT_EVENT_LIMIT: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeUpdate {
    pub recent_effective_tokens: f64,
    pub applied_event_ids: Vec<i64>,
}

pub fn stage_usage_poll_deltas(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
) -> Result<Vec<i64>> {
    let mut ids = Vec::new();
    for delta in &poll.deltas {
        let event = NormalizedUsageEvent {
            provider_surface: delta.provider_surface.clone(),
            provider_version: delta.cursor_update.provider_version.clone(),
            parser_version: delta.cursor_update.parser_version.clone(),
            command: delta.command.clone(),
            source_surface: "daily".to_string(),
            period_start: delta.period_start,
            observed_at: delta.observed_at,
            bucket_at: floor_to_ten_minute_bucket(delta.observed_at),
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

#[doc(hidden)]
/// Test-only convenience that stages, applies, and marks in one call.
/// Production must split the sequence on `state_store.save` — call
/// `stage_usage_poll_deltas` then `state_store.save` then
/// `mark_events_applied_and_advance_cursors` directly so a save failure
/// cannot strand the ledger and lose food.
pub fn apply_usage_poll(
    state: &mut PetState,
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    now: OffsetDateTime,
) -> Result<RuntimeUpdate> {
    stage_usage_poll_deltas(usage_store, poll)?;
    let update = apply_unapplied_usage(state, usage_store, now)?;
    usage_store.mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)?;
    Ok(update)
}

pub fn floor_to_ten_minute_bucket(timestamp: OffsetDateTime) -> OffsetDateTime {
    let minute = timestamp.minute();
    let bucketed_minute = (minute / 10) * 10;
    timestamp
        .replace_minute(bucketed_minute)
        .and_then(|t| t.replace_second(0))
        .and_then(|t| t.replace_nanosecond(0))
        .unwrap_or(timestamp)
}

fn apply_effective_delta(state: &mut PetState, effective_tokens: f64) {
    state.lifetime_effective_tokens += effective_tokens;

    let xp_result = apply_xp_delta(state.xp, effective_tokens, state.calibration);
    state.xp = xp_result.xp;
    for transition in xp_result.stage_transitions {
        record_stage_transition(state, transition);
    }

    let food = apply_food(
        game_vitals(state.vitals),
        effective_tokens,
        state.calibration.daily_effective_tokens,
    );
    state.vitals = stored_vitals(food);
}

fn apply_idle_decay(state: &mut PetState, now: OffsetDateTime) {
    let last_seen = state.last_usage_poll_at.unwrap_or(state.last_updated_at);
    let decay = apply_decay(game_vitals(state.vitals), last_seen, now, state.rhythm);
    state.vitals = stored_vitals(decay);
}

fn record_stage_transition(state: &mut PetState, transition: StageTransition) {
    let from = stage_name(transition.from);
    let to = stage_name(transition.to);
    let key = format!("{from}->{to}");
    if state.seen_stage_transitions.iter().any(|seen| seen == &key) {
        state.stage = to.to_string();
        return;
    }

    state.seen_stage_transitions.push(key);
    state
        .recent_events
        .push(format!("evolved from {from} to {to}"));
    state.stage = to.to_string();
}

fn trim_recent_events(state: &mut PetState) {
    let extra = state.recent_events.len().saturating_sub(RECENT_EVENT_LIMIT);
    if extra > 0 {
        state.recent_events.drain(0..extra);
    }
}

fn game_vitals(vitals: StoredVitals) -> crate::game::metabolism::Vitals {
    crate::game::metabolism::Vitals {
        fed: vitals.fed,
        happiness: vitals.happiness,
        energy: vitals.energy,
    }
}

fn stored_vitals(result: MetabolismResult) -> StoredVitals {
    StoredVitals {
        fed: result.vitals.fed,
        happiness: result.vitals.happiness,
        energy: result.vitals.energy,
    }
}

fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::S0 => "s0",
        Stage::S1 => "s1",
        Stage::S2 => "s2",
        Stage::S3 => "s3",
        Stage::S4 => "s4",
        Stage::S5 => "s5",
        Stage::S6 => "s6",
    }
}

fn format_tokens(value: f64) -> String {
    if value >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}
