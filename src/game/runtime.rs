use time::{Duration, OffsetDateTime};

use crate::{
    error::Result,
    game::{
        calibration::CalibrationBaseline,
        evolution::{apply_xp_delta, stage_for_xp, Stage, StageTransition},
        habitat,
        metabolism::{apply_decay, apply_food, mood_for_vitals, MetabolismResult},
    },
    pet::narration,
    storage::{
        state::{NarrativeEvent, PetState, Vitals as StoredVitals},
        usage_store::{NormalizedUsageEvent, UsageStore},
    },
    usage::provider::{UsageDelta, UsagePollResult},
};

const USAGE_RETENTION_DAYS: i64 = 90;
const RECENT_EVENT_LIMIT: usize = 20;
const POLL_NARRATION_COOLDOWN: Duration = Duration::minutes(5);
const REFLECTED_USAGE_EVENT_ID_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeUpdate {
    pub recent_effective_tokens: f64,
    pub applied_event_ids: Vec<i64>,
}

pub fn stage_usage_poll_deltas(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    baseline: CalibrationBaseline,
    now: OffsetDateTime,
) -> Result<Vec<i64>> {
    let mut ids = Vec::new();
    let current_bucket = floor_to_ten_minute_bucket(now);
    for delta in &poll.deltas {
        let buckets = crate::game::catchup::smear_catchup_delta(delta.effective_tokens, baseline);
        let bucket_count = buckets.len();
        let total_effective: f64 = buckets.iter().sum();
        for (bucket_index, effective_tokens) in buckets.into_iter().enumerate() {
            let bucket_offset = bucket_count.saturating_sub(bucket_index + 1) as i64;
            let bucket_at = current_bucket - Duration::minutes(bucket_offset * 10);
            let mut event = event_for_delta(delta, now);
            event.observed_at = now;
            event.bucket_at = bucket_at;
            event.effective_tokens = effective_tokens;
            if let Some(totals) = delta.token_totals {
                event.input_tokens = scaled_token_bucket(
                    Some(totals.uncached_input),
                    effective_tokens,
                    total_effective,
                );
                event.output_tokens =
                    scaled_token_bucket(Some(totals.output), effective_tokens, total_effective);
                event.cache_creation_tokens = scaled_token_bucket(
                    Some(totals.cache_creation),
                    effective_tokens,
                    total_effective,
                );
                event.cache_read_tokens =
                    scaled_token_bucket(Some(totals.cache_read), effective_tokens, total_effective);
                event.reasoning_output_tokens = scaled_token_bucket(
                    Some(totals.reasoning_output),
                    effective_tokens,
                    total_effective,
                );
            }
            ids.push(usage_store.insert_unapplied_event_bucket(
                &event,
                &delta.cursor_update,
                bucket_index,
                bucket_count,
            )?);
        }
    }
    Ok(ids)
}

fn scaled_token_bucket(total: Option<u64>, effective_share: f64, total_effective: f64) -> f64 {
    let Some(total) = total else {
        return 0.0;
    };
    if !effective_share.is_finite() || !total_effective.is_finite() || total_effective <= 0.0 {
        return 0.0;
    }
    (total as f64) * (effective_share / total_effective).clamp(0.0, 1.0)
}

// `bucket_at` and `effective_tokens` are placeholders here because Rust struct
// literals require every field — the smear loop's `..` syntax overrides both
// with the per-bucket values that actually get inserted.
fn event_for_delta(delta: &UsageDelta, now: OffsetDateTime) -> NormalizedUsageEvent {
    NormalizedUsageEvent {
        provider_surface: delta.provider_surface.clone(),
        provider_version: delta.cursor_update.provider_version.clone(),
        parser_version: delta.cursor_update.parser_version.clone(),
        command: delta.command.clone(),
        source_surface: "daily".to_string(),
        period_start: delta.period_start,
        observed_at: now,
        bucket_at: now,
        model: delta.model.clone(),
        input_tokens: 0.0,
        output_tokens: 0.0,
        cache_creation_tokens: 0.0,
        cache_read_tokens: 0.0,
        reasoning_output_tokens: 0.0,
        effective_tokens: 0.0,
        cost_usd: None,
        confidence: delta.confidence.clone(),
        provider_delta_id: None,
    }
}

pub fn apply_unapplied_usage(
    state: &mut PetState,
    usage_store: &mut UsageStore,
    now: OffsetDateTime,
) -> Result<RuntimeUpdate> {
    reconcile_stage_with_xp(state);
    let rows = usage_store.unapplied_events(500)?;
    let rows_to_apply = rows
        .iter()
        .filter(|row| !state.reflected_usage_event_ids.contains(&row.id))
        .cloned()
        .collect::<Vec<_>>();
    let recent_effective_tokens = rows_to_apply
        .iter()
        .map(|row| row.event.effective_tokens.max(0.0))
        .sum::<f64>();

    let initial_stage = state.stage;
    let initial_vitals = state.vitals;
    let initial_mood = mood_for_vitals(game_vitals(state.vitals));

    if recent_effective_tokens > 0.0 {
        for row in &rows_to_apply {
            apply_effective_delta(state, row.event.effective_tokens.max(0.0));
        }

        // Poll cycle narration: token rate bucket.
        if let Some(bucket) = narration::poll_bucket(recent_effective_tokens) {
            if should_narrate_poll_cycle(state, bucket, recent_effective_tokens, now) {
                let text = narration::poll_phrase(&state.pet.accepted_name, bucket, now);
                state.recent_events.push(NarrativeEvent {
                    observed_at: now,
                    text,
                });
            }
        }
    } else if rows.is_empty() {
        apply_idle_decay(state, now);

        // Idle narration: fires if idle ≥ 30 min and no idle narration in the last 6 hours.
        if let Some(last_poll) = state.last_usage_poll_at {
            let idle_for = now - last_poll;
            let last_narration_age = state.last_idle_narration_at.map(|t| now - t);
            if idle_for >= Duration::minutes(30)
                && last_narration_age.is_none_or(|age| age >= Duration::hours(6))
            {
                let text = narration::idle_phrase(&state.pet.accepted_name.clone(), now);
                state.recent_events.push(NarrativeEvent {
                    observed_at: now,
                    text,
                });
                state.last_idle_narration_at = Some(now);
            }
        }
    }

    // Stage transition narration: narrate any stages crossed during this poll.
    if state.stage != initial_stage {
        // Find transitions that were added during this apply pass.
        let new_stage_idx = state.stage.index();
        let old_stage_idx = initial_stage.index();
        let species = state.pet.generated_species;
        for idx in (old_stage_idx + 1)..=new_stage_idx {
            if let Some(new_stage) = Stage::from_index(idx) {
                let text =
                    narration::stage_phrase(&state.pet.accepted_name.clone(), species, new_stage);
                state.recent_events.push(NarrativeEvent {
                    observed_at: now,
                    text,
                });
            }
        }
    }

    // Mood transition narration.
    let new_mood = mood_for_vitals(game_vitals(state.vitals));
    if state.last_seen_mood.is_some() && state.last_seen_mood != Some(new_mood) {
        let text = narration::mood_phrase(&state.pet.accepted_name.clone(), new_mood, now);
        state.recent_events.push(NarrativeEvent {
            observed_at: now,
            text,
        });
    }
    state.last_seen_mood = Some(new_mood);

    // Vital threshold crossing narration.
    if let Some(prev) = state.previous_vitals {
        if let Some(crossing) = narration::vital_crossing(prev, state.vitals) {
            let text = narration::vital_phrase(&state.pet.accepted_name.clone(), crossing);
            state.recent_events.push(NarrativeEvent {
                observed_at: now,
                text,
            });
        }
    }
    habitat::unlock_habitat_props(
        state,
        &rows_to_apply,
        recent_effective_tokens,
        initial_mood,
        new_mood,
        now,
    );
    record_reflected_usage_event_ids(state, rows.iter().map(|row| row.id));
    state.previous_vitals = Some(initial_vitals);

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
    stage_usage_poll_deltas(usage_store, poll, state.calibration, now)?;
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
    let StageTransition { from: _, to } = transition;
    if state.seen_stage_transitions.contains(&to) {
        state.stage = to;
        return;
    }

    state.seen_stage_transitions.push(to);
    state.stage = to;
}

fn trim_recent_events(state: &mut PetState) {
    let extra = state.recent_events.len().saturating_sub(RECENT_EVENT_LIMIT);
    if extra > 0 {
        state.recent_events.drain(0..extra);
    }
}

fn should_narrate_poll_cycle(
    state: &PetState,
    bucket: narration::PollBucket,
    effective_tokens: f64,
    now: OffsetDateTime,
) -> bool {
    let recently_narrated = state
        .recent_events
        .iter()
        .rev()
        .find(|event| narration::is_poll_phrase(&state.pet.accepted_name, &event.text))
        .map(|event| now - event.observed_at)
        .is_some_and(|age| age < POLL_NARRATION_COOLDOWN);

    if recently_narrated {
        return false;
    }

    narration::should_sample_poll_phrase(&state.pet.seed, bucket, effective_tokens, now)
}

fn record_reflected_usage_event_ids(state: &mut PetState, ids: impl IntoIterator<Item = i64>) {
    state.reflected_usage_event_ids.extend(ids);
    state.reflected_usage_event_ids.sort_unstable();
    state.reflected_usage_event_ids.dedup();

    let extra = state
        .reflected_usage_event_ids
        .len()
        .saturating_sub(REFLECTED_USAGE_EVENT_ID_LIMIT);
    if extra > 0 {
        state.reflected_usage_event_ids.drain(0..extra);
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

// If saved state.xp now maps to a higher stage than state.stage records — for
// example after a threshold curve change between runs — emit the missing
// transitions so the pet catches up before new food gets applied. The saved
// stage is otherwise the source of truth; this only fires when the XP curve
// (not the data) changes between runs.
fn reconcile_stage_with_xp(state: &mut PetState) {
    let saved = state.stage.index();
    let current = stage_for_xp(state.xp).index();
    if current <= saved {
        return;
    }
    for index in (saved + 1)..=current {
        record_stage_transition(
            state,
            StageTransition {
                from: Stage::from_index(index - 1).unwrap_or(Stage::S6),
                to: Stage::from_index(index).unwrap_or(Stage::S6),
            },
        );
    }
}
