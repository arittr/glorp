use time::{Duration, OffsetDateTime};

use crate::{
    error::Result,
    game::{
        evolution::{apply_xp_delta, Stage, StageTransition},
        metabolism::{apply_decay, apply_food, MetabolismResult},
    },
    storage::{
        state::{PetState, Vitals as StoredVitals},
        usage_store::UsageStore,
    },
    usage::provider::UsagePollResult,
};

const USAGE_RETENTION_DAYS: i64 = 90;
const RECENT_EVENT_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeUpdate {
    pub recent_effective_tokens: f64,
}

pub fn apply_usage_poll(
    state: &mut PetState,
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    now: OffsetDateTime,
) -> Result<RuntimeUpdate> {
    let recent_effective_tokens = poll
        .deltas
        .iter()
        .map(|delta| delta.effective_tokens.max(0.0))
        .sum::<f64>();

    if recent_effective_tokens > 0.0 {
        apply_effective_delta(state, recent_effective_tokens);
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
    })
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
