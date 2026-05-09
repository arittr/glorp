use glorp::{
    game::runtime::apply_usage_poll,
    storage::{
        state::{PetState, Vitals},
        usage_store::{NormalizedUsageEvent, UsageStore},
    },
    usage::provider::{UsageDelta, UsagePollResult},
};
use tempfile::tempdir;
use time::{macros::datetime, Duration};

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

    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();
    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();

    assert_eq!(state.lifetime_effective_tokens, 200_000.0);
    assert_eq!(state.stage, "s1");
    assert!(state.xp > 0.25);
    assert!(state.vitals.fed > 40.0);
    assert_eq!(state.last_usage_poll_at, Some(now));
    assert_eq!(state.last_updated_at, now);
    assert_eq!(
        state
            .recent_events
            .iter()
            .filter(|event| event.contains("evolved from s0 to s1"))
            .count(),
        1
    );
    assert_eq!(state.seen_stage_transitions, vec!["s0->s1"]);
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
    UsagePollResult {
        deltas: vec![UsageDelta {
            provider_surface: "claude-code".to_string(),
            effective_tokens,
            confidence: "local-log-derived".to_string(),
            period_start: now.to_string(),
            model: Some("test-model".to_string()),
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
