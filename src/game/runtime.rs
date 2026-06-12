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
        usage_store::{NormalizedUsageEvent, ProviderDiagnostic, UsageLedgerRow, UsageStore},
    },
    tui::life::{AppliedSourceMix, AppliedUsageSignal, TokenShapeDelta, UsageSignalFreshness},
    usage::provider::{UsageDelta, UsagePollResult},
};

const USAGE_RETENTION_DAYS: i64 = 90;
const RECENT_EVENT_LIMIT: usize = 20;
const POLL_NARRATION_COOLDOWN: Duration = Duration::minutes(5);
const REFLECTED_USAGE_EVENT_ID_LIMIT: usize = 1_000;
const LIVE_SIGNAL_MAX_ELAPSED_SECONDS: i64 = 30 * 60;
const LIVE_SIGNAL_BACKFILL_DAILY_RATIO: f64 = 1.0;
/// A provider surface whose summed poll delta exceeds
/// `guard_ratio x baseline x days_factor` (floored below) is refused:
/// cursors advance, nothing stages. Config-overridable via
/// `discontinuity_guard_ratio` (spec Amendment 2026-06-10).
pub const DISCONTINUITY_GUARD_RATIO: f64 = 5.0;
/// Guard threshold floor — a same-day honest heavy catch-up over a
/// low-median baseline must pass while a true bolus still fires.
pub const DISCONTINUITY_GUARD_FLOOR_TOKENS: f64 = 50_000_000.0;
/// Diagnostic code persisted on a refused poll. Exempt from source-health
/// broken classification and the ready-today filter (the source is
/// healthy — one poll was refused).
pub const USAGE_DISCONTINUITY_CODE: &str = "usage_discontinuity";

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeUpdate {
    pub recent_effective_tokens: f64,
    pub applied_event_ids: Vec<i64>,
    pub applied_signal: AppliedUsageSignal,
}

pub fn stage_usage_poll_deltas(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    state: &mut PetState,
    guard_ratio: f64,
    now: OffsetDateTime,
) -> Result<Vec<i64>> {
    let baseline = state.calibration;
    let refused_surfaces =
        handle_first_contact_and_discontinuity(usage_store, poll, state, guard_ratio, now)?;
    let mut ids = Vec::new();
    let current_bucket = floor_to_ten_minute_bucket(now);
    for delta in &poll.deltas {
        if refused_surfaces.contains(&delta.provider_surface) {
            continue;
        }
        if usage_store
            .provider_cursor(
                &delta.cursor_update.provider_surface,
                &delta.cursor_update.cursor_key,
            )?
            .as_deref()
            == Some(delta.cursor_update.cursor_value.as_str())
        {
            continue;
        }
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
                event.input_tokens =
                    scaled_token_bucket(totals.uncached_input, effective_tokens, total_effective);
                event.output_tokens =
                    scaled_token_bucket(totals.output, effective_tokens, total_effective);
                event.cache_creation_tokens =
                    scaled_token_bucket(totals.cache_creation, effective_tokens, total_effective);
                event.cache_read_tokens =
                    scaled_token_bucket(totals.cache_read, effective_tokens, total_effective);
                event.reasoning_output_tokens =
                    scaled_token_bucket(totals.reasoning_output, effective_tokens, total_effective);
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

/// The usage discontinuity guard (spec Amendment 2026-06-10). Per provider
/// surface, a poll whose summed effective delta exceeds
/// `max(guard_ratio x baseline x days_factor, DISCONTINUITY_GUARD_FLOOR_TOKENS)`
/// is refused alone: its cursors advance with a persisted
/// `usage_discontinuity` diagnostic in one transaction, and nothing stages.
/// `days_factor` = whole days since that provider's newest cursor
/// `updated_at` + 1 — per-provider, so an honest multi-day catch-up after a
/// single-helper outage is not pinned at factor 1 by its healthy sibling. A
/// surface with no cursors at all is first contact and is refused outright
/// (the calibration never-feed-history rule). Refusal narrates once and
/// stamps `last_idle_narration_at` so the same pass cannot also narrate
/// boredom. Refused tokens are never retro-fed; the config ratio is the
/// escape hatch going forward.
fn handle_first_contact_and_discontinuity(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    state: &mut PetState,
    guard_ratio: f64,
    now: OffsetDateTime,
) -> Result<std::collections::BTreeSet<String>> {
    let mut surface_sums: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    let mut surface_deltas: std::collections::BTreeMap<String, Vec<&UsageDelta>> =
        std::collections::BTreeMap::new();

    for delta in &poll.deltas {
        *surface_sums
            .entry(delta.provider_surface.clone())
            .or_insert(0.0) += delta.effective_tokens.max(0.0);
        surface_deltas
            .entry(delta.provider_surface.clone())
            .or_default()
            .push(delta);
    }

    let mut skip_surfaces: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut discontinuity_surfaces: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();

    for (surface, sum) in surface_sums {
        match usage_store.latest_cursor_updated_at(&surface)? {
            None => {
                seed_first_contact_surface(usage_store, &surface, &surface_deltas[&surface], now)?;
                skip_surfaces.insert(surface);
            }
            Some(updated_at) => {
                let days_factor = ((now - updated_at).whole_days().max(0) + 1) as f64;
                let threshold =
                    (guard_ratio * state.calibration.daily_effective_tokens * days_factor)
                        .max(DISCONTINUITY_GUARD_FLOOR_TOKENS);
                if sum <= threshold {
                    continue;
                }
                let updates: Vec<_> = surface_deltas[&surface]
                    .iter()
                    .map(|delta| delta.cursor_update.clone())
                    .collect();
                usage_store.refuse_poll_discontinuity(
                    updates,
                    &ProviderDiagnostic {
                        provider_surface: surface.clone(),
                        code: USAGE_DISCONTINUITY_CODE.to_string(),
                        message: format!(
                            "refused {sum:.0} effective tokens (threshold {threshold:.0})"
                        ),
                        recorded_at: now,
                    },
                    now,
                )?;
                skip_surfaces.insert(surface.clone());
                discontinuity_surfaces.insert(surface);
            }
        }
    }

    if !discontinuity_surfaces.is_empty() {
        state.recent_events.push(NarrativeEvent {
            observed_at: now,
            text: format!("{} declined an implausible feast", state.pet.accepted_name),
        });
        state.last_idle_narration_at = Some(now);
    }

    Ok(skip_surfaces)
}

fn seed_first_contact_surface(
    usage_store: &mut UsageStore,
    surface: &str,
    deltas: &[&UsageDelta],
    now: OffsetDateTime,
) -> Result<()> {
    let mut events = Vec::new();

    for delta in deltas {
        let totals = delta.token_totals.unwrap_or_default();
        let event = NormalizedUsageEvent {
            provider_surface: delta.provider_surface.clone(),
            provider_version: delta.cursor_update.provider_version.clone(),
            parser_version: delta.cursor_update.parser_version.clone(),
            command: delta.command.clone(),
            source_surface: "daily".to_string(),
            period_start: delta.period_start,
            observed_at: now,
            bucket_at: delta.period_start,
            model: delta.model.clone(),
            input_tokens: totals.uncached_input as f64,
            output_tokens: totals.output as f64,
            cache_creation_tokens: totals.cache_creation as f64,
            cache_read_tokens: totals.cache_read as f64,
            reasoning_output_tokens: totals.reasoning_output as f64,
            effective_tokens: delta.effective_tokens,
            cost_usd: None,
            confidence: delta.confidence.clone(),
            provider_delta_id: None,
        };
        events.push((event, delta.cursor_update.clone()));
    }

    let diagnostic = ProviderDiagnostic {
        provider_surface: surface.to_string(),
        code: "source_first_contact".to_string(),
        message: format!(
            "first contact with {surface}: {} historical rows seeded without feeding",
            events.len()
        ),
        recorded_at: now,
    };

    usage_store.seed_source_history(&events, Some(&diagnostic), now)
}

fn scaled_token_bucket(total: u64, effective_share: f64, total_effective: f64) -> f64 {
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
    scene_asleep: bool,
) -> Result<RuntimeUpdate> {
    reconcile_stage_with_xp(state);
    let previous_poll_at = state.last_usage_poll_at;
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
                let text = narration::idle_phrase(&state.pet.accepted_name, now, scene_asleep);
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

    let applied_signal =
        applied_signal_from_rows(&rows_to_apply, now, previous_poll_at, state.calibration);
    state.last_usage_poll_at = Some(now);
    state.last_updated_at = now;
    trim_recent_events(state);
    usage_store.compact_before(now - Duration::days(USAGE_RETENTION_DAYS))?;

    Ok(RuntimeUpdate {
        recent_effective_tokens,
        applied_event_ids: rows.into_iter().map(|row| row.id).collect(),
        applied_signal,
    })
}

fn applied_signal_from_rows(
    rows: &[UsageLedgerRow],
    now: OffsetDateTime,
    previous_poll_at: Option<OffsetDateTime>,
    calibration: CalibrationBaseline,
) -> AppliedUsageSignal {
    let elapsed = previous_poll_at
        .map(|last| now - last)
        .unwrap_or_else(|| Duration::seconds(0));
    let applied_effective_tokens = rows
        .iter()
        .map(|row| row.event.effective_tokens.max(0.0))
        .sum::<f64>();

    let mut claude_effective_tokens = 0.0;
    let mut codex_effective_tokens = 0.0;
    for row in rows {
        let effective_tokens = row.event.effective_tokens.max(0.0);
        let provider_surface = row.event.provider_surface.to_ascii_lowercase();
        if provider_surface.contains("claude") {
            claude_effective_tokens += effective_tokens;
        }
        if provider_surface.contains("codex") {
            codex_effective_tokens += effective_tokens;
        }
    }
    let source_mix = if claude_effective_tokens > 0.0 || codex_effective_tokens > 0.0 {
        Some(AppliedSourceMix {
            claude_effective_tokens,
            codex_effective_tokens,
        })
    } else {
        None
    };

    let freshness = signal_freshness(
        applied_effective_tokens,
        elapsed,
        previous_poll_at,
        calibration,
    );

    AppliedUsageSignal {
        applied_effective_tokens,
        raw_effective_tokens: None,
        source_mix,
        token_shape: token_shape_from_rows(rows),
        observed_at: now,
        elapsed_since_successful_poll: elapsed,
        freshness,
    }
}

fn signal_freshness(
    applied_effective_tokens: f64,
    elapsed: Duration,
    previous_poll_at: Option<OffsetDateTime>,
    calibration: CalibrationBaseline,
) -> UsageSignalFreshness {
    if applied_effective_tokens <= 0.0 {
        return UsageSignalFreshness::Live;
    }
    if previous_poll_at.is_none() {
        return UsageSignalFreshness::ColdStart;
    }
    if elapsed.whole_seconds() < 0 {
        return UsageSignalFreshness::Backfill;
    }
    if elapsed.whole_seconds() > LIVE_SIGNAL_MAX_ELAPSED_SECONDS {
        return UsageSignalFreshness::Backfill;
    }
    let daily = calibration.daily_effective_tokens.max(1.0);
    if applied_effective_tokens > daily * LIVE_SIGNAL_BACKFILL_DAILY_RATIO {
        return UsageSignalFreshness::Backfill;
    }
    UsageSignalFreshness::Live
}

fn token_shape_from_rows(rows: &[UsageLedgerRow]) -> Option<TokenShapeDelta> {
    let shape = TokenShapeDelta {
        input_tokens: rows.iter().map(|row| row.event.input_tokens.max(0.0)).sum(),
        output_tokens: rows
            .iter()
            .map(|row| row.event.output_tokens.max(0.0))
            .sum(),
        cache_creation_tokens: rows
            .iter()
            .map(|row| row.event.cache_creation_tokens.max(0.0))
            .sum(),
        cache_read_tokens: rows
            .iter()
            .map(|row| row.event.cache_read_tokens.max(0.0))
            .sum(),
        reasoning_output_tokens: rows
            .iter()
            .map(|row| row.event.reasoning_output_tokens.max(0.0))
            .sum(),
    };

    if shape.input_tokens > 0.0
        || shape.output_tokens > 0.0
        || shape.cache_creation_tokens > 0.0
        || shape.cache_read_tokens > 0.0
        || shape.reasoning_output_tokens > 0.0
    {
        Some(shape)
    } else {
        None
    }
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
    stage_usage_poll_deltas(usage_store, poll, state, DISCONTINUITY_GUARD_RATIO, now)?;
    let update = apply_unapplied_usage(state, usage_store, now, false)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        storage::{
            state::PetState,
            usage_store::{ProviderCursorUpdate, UsageStore},
        },
        tui::life::{AppliedSourceMix, TokenShapeDelta, UsageSignalFreshness},
        usage::identity::SourceIdentity,
        usage::normalize::RawTokenTotals,
    };
    use tempfile::tempdir;
    use time::macros::datetime;
    use time::OffsetDateTime;

    #[test]
    fn idle_narration_uses_sleep_vocabulary_only_when_scene_asleep() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage = UsageStore::open(&db_path).unwrap();
        let mut state = PetState::new_for_test("seed", "buddy");
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        state.last_usage_poll_at = Some(now - Duration::minutes(30));

        apply_unapplied_usage(&mut state, &mut usage, now, true).unwrap();
        let last_asleep = state
            .recent_events
            .last()
            .map(|e| e.text.as_str())
            .unwrap_or("");
        assert!(
            last_asleep.contains("drifted off") || last_asleep.contains("dreams"),
            "expected sleep vocabulary, got: {last_asleep}"
        );

        state.recent_events.clear();
        state.last_idle_narration_at = None;
        state.last_usage_poll_at = Some(now - Duration::minutes(30));

        apply_unapplied_usage(&mut state, &mut usage, now, false).unwrap();
        let last_awake = state
            .recent_events
            .last()
            .map(|e| e.text.as_str())
            .unwrap_or("");
        assert!(
            !last_awake.contains("drifted off") && !last_awake.contains("dreams"),
            "awake idle must not claim sleep: {last_awake}"
        );
    }

    #[test]
    fn apply_unapplied_usage_returns_applied_signal_summary() {
        let dir = tempdir().unwrap();
        let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
        let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
        state.last_usage_poll_at = Some(datetime!(2026 - 05 - 09 11:50 UTC));
        let now = datetime!(2026 - 05 - 09 12:00 UTC);

        let claude_event = NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            input_tokens: 2_000.0,
            output_tokens: 3_000.0,
            cache_read_tokens: 5_000.0,
            effective_tokens: 10_000.0,
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 10_000.0)
        };
        usage_store
            .insert_unapplied_event_bucket(
                &claude_event,
                &ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "claude-test".into(),
                    cursor_value: "claude-row".into(),
                    provider_version: "test-provider".into(),
                    parser_version: "test-parser".into(),
                },
                0,
                1,
            )
            .unwrap();

        let codex_event = NormalizedUsageEvent {
            provider_surface: "codex".into(),
            input_tokens: 3_000.0,
            output_tokens: 6_000.0,
            cache_read_tokens: 11_000.0,
            effective_tokens: 20_000.0,
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 20_000.0)
        };
        usage_store
            .insert_unapplied_event_bucket(
                &codex_event,
                &ProviderCursorUpdate {
                    provider_surface: "codex".into(),
                    cursor_key: "codex-test".into(),
                    cursor_value: "codex-row".into(),
                    provider_version: "test-provider".into(),
                    parser_version: "test-parser".into(),
                },
                0,
                1,
            )
            .unwrap();

        let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();

        assert_eq!(update.applied_signal.freshness, UsageSignalFreshness::Live);
        assert_eq!(update.applied_signal.applied_effective_tokens, 30_000.0);
        assert_eq!(
            update.applied_signal.source_mix,
            Some(AppliedSourceMix {
                claude_effective_tokens: 10_000.0,
                codex_effective_tokens: 20_000.0,
            })
        );
        assert_eq!(
            update.applied_signal.token_shape,
            Some(TokenShapeDelta {
                input_tokens: 5_000.0,
                output_tokens: 9_000.0,
                cache_creation_tokens: 0.0,
                cache_read_tokens: 16_000.0,
                reasoning_output_tokens: 0.0,
            })
        );
    }

    #[test]
    fn delayed_applied_usage_is_backfill_not_live_burst() {
        let dir = tempdir().unwrap();
        let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
        let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
        state.last_usage_poll_at = Some(datetime!(2026 - 05 - 09 10:00 UTC));
        let now = datetime!(2026 - 05 - 09 12:00 UTC);

        let event = NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            effective_tokens: 20_000.0,
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 20_000.0)
        };
        usage_store
            .insert_unapplied_event_bucket(
                &event,
                &ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "claude-test".into(),
                    cursor_value: "delayed-row".into(),
                    provider_version: "test-provider".into(),
                    parser_version: "test-parser".into(),
                },
                0,
                1,
            )
            .unwrap();

        let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();

        assert_eq!(update.applied_signal.applied_effective_tokens, 20_000.0);
        assert_eq!(
            update.applied_signal.freshness,
            UsageSignalFreshness::Backfill
        );
        assert!(
            !update.applied_signal.can_burst(),
            "delayed helper/backfill rows should still feed Glorp without firing live burst"
        );
    }

    #[test]
    fn single_cadence_huge_usage_is_backfill_not_live_burst() {
        let dir = tempdir().unwrap();
        let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
        let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
        state.calibration.daily_effective_tokens = 100_000.0;
        state.last_usage_poll_at = Some(datetime!(2026 - 05 - 09 11:59:50 UTC));
        let now = datetime!(2026 - 05 - 09 12:00 UTC);

        let event = NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            effective_tokens: 250_000.0,
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 250_000.0)
        };
        usage_store
            .insert_unapplied_event_bucket(
                &event,
                &ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "claude-test".into(),
                    cursor_value: "huge-row".into(),
                    provider_version: "test-provider".into(),
                    parser_version: "test-parser".into(),
                },
                0,
                1,
            )
            .unwrap();

        let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();

        assert_eq!(
            update.applied_signal.freshness,
            UsageSignalFreshness::Backfill
        );
        assert!(!update.applied_signal.can_burst());
    }

    #[test]
    fn clock_skewed_previous_poll_is_backfill_not_live_burst() {
        assert_eq!(
            signal_freshness(
                5_000.0,
                Duration::seconds(-10),
                Some(datetime!(2026 - 05 - 09 12:00 UTC)),
                CalibrationBaseline::default(),
            ),
            UsageSignalFreshness::Backfill
        );
    }

    #[test]
    fn first_contact_seeds_history_without_feeding() {
        let dir = tempdir().unwrap();
        let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
        let mut state = PetState::new_for_test("seed", "buddy");
        state.calibration.daily_effective_tokens = 100_000.0;

        let now = datetime!(2026-06-11 12:00 UTC);
        let historical = datetime!(2026-06-10 00:00 UTC);

        let poll = UsagePollResult {
            deltas: vec![UsageDelta {
                provider_surface: "gemini".into(),
                source_identity: SourceIdentity::from_provider_surface("gemini"),
                command: "ccusage daily".into(),
                effective_tokens: 500_000.0,
                confidence: "local-log-derived".into(),
                period_start: historical,
                observed_at: now,
                model: Some("gemini-2.5-pro".into()),
                cursor_update: ProviderCursorUpdate {
                    provider_surface: "gemini".into(),
                    cursor_key: r#"{"provider_surface":"gemini","command":"ccusage daily","source_surface":"daily","period_start":"2026-06-10","model":"gemini-2.5-pro"}"#.into(),
                    cursor_value: r#"{"uncached_input":100000,"output":50000,"cache_creation":10000,"cache_read":200000,"reasoning_output":5000}"#.into(),
                    provider_version: "ccusage 20.0.6".into(),
                    parser_version: "ccusage 20.0.6".into(),
                },
                token_totals: Some(RawTokenTotals {
                    uncached_input: 100_000,
                    output: 50_000,
                    cache_creation: 10_000,
                    cache_read: 200_000,
                    reasoning_output: 5_000,
                }),
            }],
            diagnostics: vec![],
            total_effective_tokens: 500_000.0,
        };

        let ids = stage_usage_poll_deltas(
            &mut usage_store,
            &poll,
            &mut state,
            DISCONTINUITY_GUARD_RATIO,
            now,
        )
        .unwrap();

        assert!(
            ids.is_empty(),
            "first-contact deltas must not stage for feeding"
        );
        assert_eq!(state.lifetime_effective_tokens, 0.0);
        assert!(usage_store.has_any_applied_events().unwrap());

        let diags = usage_store.recent_diagnostics(10).unwrap();
        assert!(diags.iter().any(|d| d.code == "source_first_contact"));

        let totals = usage_store
            .applied_effective_tokens_by_source_between(
                now - time::Duration::days(3),
                now + time::Duration::seconds(1),
            )
            .unwrap();
        assert!(totals
            .iter()
            .any(|(s, v)| s == "gemini" && (*v - 500_000.0).abs() < 0.1));
    }

    #[test]
    fn discontinuity_guard_still_refuses_large_delta_after_cursor_exists() {
        let dir = tempdir().unwrap();
        let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
        let mut state = PetState::new_for_test("seed", "buddy");
        state.calibration.daily_effective_tokens = 100_000.0;
        let now = datetime!(2026-06-11 12:00 UTC);

        usage_store
            .advance_cursors(
                vec![ProviderCursorUpdate {
                    provider_surface: "gemini".into(),
                    cursor_key: "contact".into(),
                    cursor_value: "seeded".into(),
                    provider_version: "test".into(),
                    parser_version: "test".into(),
                }],
                now - time::Duration::hours(1),
            )
            .unwrap();

        let poll = UsagePollResult {
            deltas: vec![UsageDelta {
                provider_surface: "gemini".into(),
                source_identity: SourceIdentity::from_provider_surface("gemini"),
                command: "ccusage daily".into(),
                effective_tokens: 500_000_000.0,
                confidence: "local-log-derived".into(),
                period_start: now,
                observed_at: now,
                model: None,
                cursor_update: ProviderCursorUpdate {
                    provider_surface: "gemini".into(),
                    cursor_key: "contact".into(),
                    cursor_value: "totals".into(),
                    provider_version: "test".into(),
                    parser_version: "test".into(),
                },
                token_totals: None,
            }],
            diagnostics: vec![],
            total_effective_tokens: 500_000_000.0,
        };

        let ids = stage_usage_poll_deltas(
            &mut usage_store,
            &poll,
            &mut state,
            DISCONTINUITY_GUARD_RATIO,
            now,
        )
        .unwrap();
        assert!(ids.is_empty());
        let diags = usage_store.recent_diagnostics(10).unwrap();
        assert!(diags.iter().any(|d| d.code == "usage_discontinuity"));
    }
}
