use glorp::{
    game::{
        evolution::Stage,
        runtime::{apply_unapplied_usage, apply_usage_poll, stage_usage_poll_deltas},
    },
    storage::{
        state::{PetState, Vitals},
        usage_store::{NormalizedUsageEvent, ProviderCursorUpdate, UsageStore},
    },
    usage::provider::{UsageDelta, UsagePollResult},
};
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::tempdir;
use time::{macros::datetime, Duration};

static POLL_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn provider_delta_updates_pet_state_and_records_evolution_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.vitals = Vitals {
        fed: 40.0,
        happiness: 40.0,
        energy: 40.0,
    };
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let poll = poll_with_delta(100_000.0, now);

    // Two polls with distinct cursor_values represent successive bumps in provider totals.
    // The unapplied ledger's idempotency would collapse identical polls into one row.
    let poll2 = poll_with_delta(100_000.0, now);
    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();
    apply_usage_poll(&mut state, &mut usage_store, &poll2, now).unwrap();

    // Two polls of one calibrated active day each smear into ledger buckets,
    // crossing s0->s1, s1->s2, and s2->s3. Each transition records once.
    assert_eq!(state.lifetime_effective_tokens, 200_000.0);
    assert_eq!(state.stage, Stage::S3);
    assert!(state.xp >= 1.0);
    assert!(state.vitals.fed > 40.0);
    assert_eq!(state.last_usage_poll_at, Some(now));
    assert_eq!(state.last_updated_at, now);
    // PetState::new_for_test defaults to Species::Fuzz; S1=fuzzling, S2=kit, S3=pup.
    for label in ["fuzzling", "kit", "pup"] {
        let expected_text = format!("mochi evolved into {label}");
        assert_eq!(
            state
                .recent_events
                .iter()
                .filter(|event| event.text.contains(&expected_text))
                .count(),
            1,
            "expected '{expected_text}' recorded once",
        );
    }
    assert_eq!(
        state.seen_stage_transitions,
        vec![Stage::S1, Stage::S2, Stage::S3]
    );
}

#[test]
fn no_delta_poll_applies_rhythm_decay_without_granting_xp() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.vitals = Vitals {
        fed: 70.0,
        happiness: 70.0,
        energy: 70.0,
    };
    state.last_usage_poll_at = Some(datetime!(2026 - 05 - 09 09:00 UTC));
    let now = datetime!(2026 - 05 - 09 13:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(state.xp, 0.0);
    assert_eq!(state.lifetime_effective_tokens, 0.0);
    assert!(state.vitals.fed < 70.0);
    assert!(state.vitals.fed > 35.0);
    assert_eq!(state.last_usage_poll_at, Some(now));
    assert_eq!(state.last_updated_at, now);
}

#[test]
fn rapid_token_polls_do_not_narrate_every_feed() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    let start = datetime!(2026 - 05 - 09 12:00 UTC);

    for tick in 0..6 {
        let now = start + Duration::seconds(tick * 10);
        let poll = poll_with_delta(2_000.0, now);
        apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();
    }

    let eating_narrations = state
        .recent_events
        .iter()
        .filter(|event| is_eating_narration(&event.text, "mochi"))
        .count();

    assert!(
        eating_narrations <= 1,
        "rapid token polls should not narrate every feed, got {eating_narrations}: {:?}",
        state
            .recent_events
            .iter()
            .map(|event| &event.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn apply_reconciles_saved_stage_when_xp_outranks_it() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    // Simulate state from before a threshold change: xp passes the new s1 and
    // s2 thresholds (0.04 and 0.25) but stage was last saved as "s0".
    state.xp = 0.30;
    state.stage = Stage::S0;
    state.seen_stage_transitions = Vec::new();

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(state.stage, Stage::S2);
    assert_eq!(state.seen_stage_transitions, vec![Stage::S1, Stage::S2]);
}

#[test]
fn runtime_compacts_old_usage_events_after_poll() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    usage_store
        .insert_event(&NormalizedUsageEvent::for_test_at(
            now - Duration::days(91),
            1000.0,
        ))
        .unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent::for_test_at(now, 250.0))
        .unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(usage_store.recent_event_count().unwrap(), 1);
    assert_eq!(
        usage_store
            .daily_aggregate_effective_tokens("claude-code")
            .unwrap(),
        1000.0
    );
}

fn poll_with_delta(effective_tokens: f64, now: time::OffsetDateTime) -> UsagePollResult {
    // Each call yields a distinct cursor_value so the unapplied ledger's idempotency
    // (keyed on provider_surface|cursor_key|cursor_value) treats each call as a new poll.
    let counter = POLL_COUNTER.fetch_add(1, Ordering::Relaxed);
    UsagePollResult {
        deltas: vec![UsageDelta {
            provider_surface: "claude-code".to_string(),
            command: "ccusage".to_string(),
            effective_tokens,
            confidence: "local-log-derived".to_string(),
            period_start: now,
            observed_at: now,
            model: Some("test-model".to_string()),
            cursor_update: ProviderCursorUpdate {
                provider_surface: "claude-code".to_string(),
                cursor_key: format!("test-cursor-{}", now.unix_timestamp()),
                cursor_value: format!(
                    r#"{{"uncached_input":{},"output":0,"cache_creation":0,"cache_read":0,"reasoning_output":0,"_counter":{}}}"#,
                    effective_tokens as u64, counter
                ),
                provider_version: "test-provider".to_string(),
                parser_version: "test-parser".to_string(),
            },
        }],
        diagnostics: Vec::new(),
        total_effective_tokens: effective_tokens,
    }
}

fn habitat_prop_ids(state: &PetState) -> Vec<&str> {
    state
        .habitat
        .earned_props
        .iter()
        .map(|prop| prop.id.as_str())
        .collect()
}

fn poll_with_surface(
    provider_surface: &str,
    effective_tokens: f64,
    now: time::OffsetDateTime,
) -> UsagePollResult {
    let mut poll = poll_with_delta(effective_tokens, now);
    for delta in &mut poll.deltas {
        delta.provider_surface = provider_surface.to_string();
        delta.cursor_update.provider_surface = provider_surface.to_string();
        delta.cursor_update.cursor_key =
            format!("{provider_surface}-cursor-{}", now.unix_timestamp());
    }
    poll
}

fn empty_poll() -> UsagePollResult {
    UsagePollResult {
        deltas: Vec::new(),
        diagnostics: Vec::new(),
        total_effective_tokens: 0.0,
    }
}

fn is_eating_narration(text: &str, pet_name: &str) -> bool {
    text.starts_with(&format!("{pet_name} "))
        && [
            "feasted", "devoured", "munched", "gobbled", "nibbled", "snacked", "sipped", "tasted",
        ]
        .iter()
        .any(|verb| text.contains(verb))
}

#[test]
fn cold_start_does_not_narrate_initial_mood() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    // Precondition: fresh state has no prior mood recorded.
    assert!(state.last_seen_mood.is_none());

    // Apply one poll with tokens so the mood-comparison code path executes.
    let poll = poll_with_delta(1_000.0, now);
    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();

    // After the first poll, mood should be recorded.
    assert!(
        state.last_seen_mood.is_some(),
        "should have recorded the mood"
    );

    // No mood narration should have fired because there was no prior mood to
    // transition *from*.
    let mood_narrations: Vec<_> = state
        .recent_events
        .iter()
        .filter(|e| {
            e.text.contains("brightened")
                || e.text.contains("settled")
                || e.text.contains("drowsy")
                || e.text.contains("hungry")
                || e.text.contains("slumped")
                || e.text.contains("faded")
                || e.text.contains("yawned")
                || e.text.contains("peckish")
                || e.text.contains("looks down")
                || e.text.contains("dimmed")
                || e.text.contains("looks great")
                || e.text.contains("relaxed")
        })
        .collect();
    assert!(
        mood_narrations.is_empty(),
        "cold start should not produce mood narration, got: {:?}",
        mood_narrations.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
}

#[test]
fn catchup_application_records_each_stage_transition_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;

    for day in 0..60 {
        let observed_at = now + Duration::days(day);
        let buckets = glorp::game::catchup::smear_catchup_delta(100_000.0, state.calibration);
        let bucket_count = buckets.len();
        for (bucket_index, effective_tokens) in buckets.into_iter().enumerate() {
            let bucket_at = observed_at
                - Duration::minutes(bucket_count.saturating_sub(bucket_index + 1) as i64 * 10);
            usage_store
                .insert_unapplied_event_bucket(
                    &NormalizedUsageEvent {
                        observed_at,
                        bucket_at,
                        effective_tokens,
                        ..NormalizedUsageEvent::for_test_at(observed_at, effective_tokens)
                    },
                    &ProviderCursorUpdate {
                        provider_surface: "claude-code".into(),
                        cursor_key: format!("cursor-{day}"),
                        cursor_value: format!("value-{day}"),
                        provider_version: "test-provider".into(),
                        parser_version: "test-parser".into(),
                    },
                    bucket_index,
                    bucket_count,
                )
                .unwrap();
        }
    }

    let update = apply_unapplied_usage(&mut state, &mut usage_store, now).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();

    assert_eq!(state.stage, Stage::S6);
    assert_eq!(state.seen_stage_transitions.len(), 6);
    assert_eq!(
        state.seen_stage_transitions,
        vec![
            Stage::S1,
            Stage::S2,
            Stage::S3,
            Stage::S4,
            Stage::S5,
            Stage::S6
        ]
    );
}

#[test]
fn unapplied_usage_survives_state_save_failure_and_applies_once_next_run() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let event = NormalizedUsageEvent {
        observed_at: now,
        bucket_at: now,
        ..NormalizedUsageEvent::for_test_at(now, 100_000.0)
    };
    let cursor = ProviderCursorUpdate {
        provider_surface: "claude-code".into(),
        cursor_key: "test-cursor".into(),
        cursor_value: r#"{"uncached_input":100000,"output":0,"cache_creation":0,"cache_read":0,"reasoning_output":0}"#.into(),
        provider_version: "test-provider".into(),
        parser_version: "test-parser".into(),
    };
    let inserted_id = usage_store
        .insert_unapplied_event_bucket(&event, &cursor, 0, 1)
        .unwrap();

    let mut failed_state = PetState::new_for_test("mochi-7f3a", "mochi");
    failed_state.calibration.daily_effective_tokens = 100_000.0;
    let failed_update = apply_unapplied_usage(&mut failed_state, &mut usage_store, now).unwrap();
    assert_eq!(failed_update.applied_event_ids, vec![inserted_id]);

    let mut retried_state = PetState::new_for_test("mochi-7f3a", "mochi");
    retried_state.calibration.daily_effective_tokens = 100_000.0;
    let retry_update = apply_unapplied_usage(&mut retried_state, &mut usage_store, now).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&retry_update.applied_event_ids, now)
        .unwrap();

    assert_eq!(retried_state.lifetime_effective_tokens, 100_000.0);
    assert_eq!(usage_store.unapplied_events(10).unwrap().len(), 0);
    assert_eq!(
        usage_store
            .provider_cursor("claude-code", "test-cursor")
            .unwrap()
            .unwrap(),
        cursor.cursor_value
    );
}

#[test]
fn fresh_pet_state_starts_with_empty_habitat_state() {
    let state = PetState::new_for_test("mochi-7f3a", "mochi");

    assert!(state.habitat.earned_props.is_empty());
    assert_eq!(state.habitat.reconciled_lifetime_tokens_at, None);
}

#[test]
fn habitat_catalog_exposes_v1_prop_ids_and_kinds() {
    use glorp::game::habitat::{catalog_prop, HabitatPropKind};
    use glorp::storage::state::HabitatPropId;

    let codex = catalog_prop(&HabitatPropId::new("codex_signal_lamp")).unwrap();
    assert_eq!(codex.kind, HabitatPropKind::Trophy);
    assert_eq!(codex.display_priority, 70);

    let pebble = catalog_prop(&HabitatPropId::new("token_pebble_25k")).unwrap();
    assert_eq!(pebble.kind, HabitatPropKind::Accent);
    assert_eq!(pebble.lifetime_threshold, Some(25_000.0));

    assert!(catalog_prop(&HabitatPropId::new("non_catalog_prop_for_filter_test")).is_none());
}

#[test]
fn lifetime_threshold_unlocks_one_ladder_prop_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(25_000.0, now),
        now,
    )
    .unwrap();
    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &empty_poll(),
        now + Duration::minutes(10),
    )
    .unwrap();

    let ids = habitat_prop_ids(&state);
    assert_eq!(ids, vec!["token_pebble_25k"]);
}

#[test]
fn one_large_poll_unlocks_ladder_props_in_threshold_order() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 10_000_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(1_100_000.0, now),
        now,
    )
    .unwrap();

    assert_eq!(
        habitat_prop_ids(&state),
        vec![
            "token_pebble_25k",
            "token_shell_100k",
            "token_spark_500k",
            "token_shard_1m",
        ]
    );
}

#[test]
fn existing_lifetime_counter_reconciles_ladder_props_without_usage_delta() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.lifetime_effective_tokens = 125_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(
        habitat_prop_ids(&state),
        vec!["token_pebble_25k", "token_shell_100k"]
    );
    assert_eq!(state.habitat.reconciled_lifetime_tokens_at, Some(125_000.0));
}

#[test]
fn reflected_unapplied_usage_does_not_unlock_ladder_twice_on_mark_retry() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 1_000_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    stage_usage_poll_deltas(
        &mut usage_store,
        &poll_with_delta(60_000.0, now),
        state.calibration,
        now,
    )
    .unwrap();
    let first_update = apply_unapplied_usage(&mut state, &mut usage_store, now).unwrap();

    assert_eq!(state.lifetime_effective_tokens, 60_000.0);
    assert_eq!(habitat_prop_ids(&state), vec!["token_pebble_25k"]);

    let retry_update =
        apply_unapplied_usage(&mut state, &mut usage_store, now + Duration::minutes(1)).unwrap();

    assert_eq!(
        retry_update.applied_event_ids,
        first_update.applied_event_ids
    );
    assert_eq!(state.lifetime_effective_tokens, 60_000.0);
    assert_eq!(habitat_prop_ids(&state), vec!["token_pebble_25k"]);
}

#[test]
fn first_codex_usage_unlocks_signal_lamp_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_surface("codex", 1_000.0, now),
        now,
    )
    .unwrap();
    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_surface("codex", 1_000.0, now + Duration::minutes(10)),
        now + Duration::minutes(10),
    )
    .unwrap();

    let lamp_count = state
        .habitat
        .earned_props
        .iter()
        .filter(|prop| prop.id.as_str() == "codex_signal_lamp")
        .count();
    assert_eq!(lamp_count, 1);
}

#[test]
fn heavy_session_unlocks_planter_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(49_999.0, now),
        now,
    )
    .unwrap();
    assert!(!habitat_prop_ids(&state).contains(&"heavy_session_planter"));

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(50_000.0, now + Duration::minutes(10)),
        now + Duration::minutes(10),
    )
    .unwrap();

    assert!(habitat_prop_ids(&state).contains(&"heavy_session_planter"));
}

#[test]
fn wilted_recovery_unlocks_sprout_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.vitals = Vitals {
        fed: 2.0,
        happiness: 2.0,
        energy: 2.0,
    };
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(100_000.0, now),
        now,
    )
    .unwrap();

    assert!(habitat_prop_ids(&state).contains(&"wilt_recovery_sprout"));
}
