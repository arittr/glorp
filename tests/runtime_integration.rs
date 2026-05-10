use glorp::{
    game::runtime::{apply_unapplied_usage, apply_usage_poll},
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
    assert_eq!(state.stage, "s3");
    assert!(state.xp >= 1.0);
    assert!(state.vitals.fed > 40.0);
    assert_eq!(state.last_usage_poll_at, Some(now));
    assert_eq!(state.last_updated_at, now);
    for transition in [
        "evolved from s0 to s1",
        "evolved from s1 to s2",
        "evolved from s2 to s3",
    ] {
        assert_eq!(
            state
                .recent_events
                .iter()
                .filter(|event| event.contains(transition))
                .count(),
            1,
            "expected {transition} recorded once",
        );
    }
    assert_eq!(
        state.seen_stage_transitions,
        vec!["s0->s1", "s1->s2", "s2->s3"]
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
fn apply_reconciles_saved_stage_when_xp_outranks_it() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    // Simulate state from before a threshold change: xp passes the new s1 and
    // s2 thresholds (0.04 and 0.25) but stage was last saved as "s0".
    state.xp = 0.30;
    state.stage = "s0".into();
    state.seen_stage_transitions = Vec::new();

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(state.stage, "s2");
    assert_eq!(state.seen_stage_transitions, vec!["s0->s1", "s1->s2"]);
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

fn empty_poll() -> UsagePollResult {
    UsagePollResult {
        deltas: Vec::new(),
        diagnostics: Vec::new(),
        total_effective_tokens: 0.0,
    }
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

    assert_eq!(state.stage, "s6");
    assert_eq!(state.seen_stage_transitions.len(), 6);
    assert_eq!(
        state.seen_stage_transitions,
        vec!["s0->s1", "s1->s2", "s2->s3", "s3->s4", "s4->s5", "s5->s6"]
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
    let inserted_id = usage_store.insert_unapplied_event(&event, &cursor).unwrap();

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
