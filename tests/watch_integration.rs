use glorp::{
    commands::watch::build_watch_view_model_for_test,
    storage::{
        state::PetState,
        usage_store::{NormalizedUsageEvent, ProviderDiagnostic, UsageStore},
    },
    tui::view_model::SourceStatus,
};
use tempfile::tempdir;
use time::{macros::datetime, Duration, OffsetDateTime};

#[test]
fn watch_view_model_uses_rendered_mech_pet_art_instead_of_fixture_cat() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".into(),
            period_start: OffsetDateTime::now_utc(),
            effective_tokens: 4_200.0,
            ..NormalizedUsageEvent::for_test_at(OffsetDateTime::now_utc(), 4_200.0)
        })
        .unwrap();

    let state = mech_state();
    let vm = build_watch_view_model_for_test(&state, &usage_db).unwrap();
    let art = vm.pet_art.join("\n");

    assert_eq!(vm.species, "mech");
    assert_eq!(vm.stage, "brass walker");
    assert!(
        [".---.", "_[=]_", ".-^-.", "/====\\", "O--O", "d====b"]
            .iter()
            .any(|marker| art.contains(marker)),
        "expected rendered adult mech art, got:\n{art}"
    );
    assert!(!art.contains("/\\_/\\"));
    assert!(!art.contains("( o.o )"));
    assert!(!art.contains("> ^ <"));
}

#[test]
fn watch_view_model_uses_usage_store_totals_and_diagnostics_instead_of_fixture_activity() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let now = OffsetDateTime::now_utc();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            period_start: now,
            effective_tokens: 4_200.0,
            ..NormalizedUsageEvent::for_test_at(now, 4_200.0)
        })
        .unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            period_start: now - Duration::minutes(9),
            effective_tokens: 800.0,
            ..NormalizedUsageEvent::for_test_at(now - Duration::minutes(9), 800.0)
        })
        .unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".into(),
            period_start: now - Duration::days(1),
            effective_tokens: 2_000.0,
            ..NormalizedUsageEvent::for_test_at(now - Duration::days(1), 2_000.0)
        })
        .unwrap();
    usage_store
        .insert_diagnostic(&ProviderDiagnostic {
            provider_surface: "codex".into(),
            code: "missing_helper".into(),
            message: "ccusage-codex helper was not found".into(),
            recorded_at: now,
        })
        .unwrap();

    let state = mech_state();
    let vm = build_watch_view_model_for_test(&state, &usage_db).unwrap();

    assert_eq!(vm.today_effective_tokens, 5_000.0);
    assert_eq!(vm.current_bucket_effective_tokens, 5_000.0);
    assert!(vm
        .source_breakdown
        .iter()
        .any(|source| source.name == "claude-code" && source.effective_tokens == 5_000.0));
    assert!(!vm
        .source_breakdown
        .iter()
        .any(|source| source.name == "codex"));
    assert_ne!(vm.today_effective_tokens, 18_420.0);
    assert!(!vm
        .recent_events
        .iter()
        .any(|event| event.text.contains("watch loop settled")));
    assert!(!vm.is_blocked());
    assert!(vm.helper_status.contains("claude-code"));
    assert!(vm.helper_status.contains("codex"));
    assert!(vm
        .errors
        .iter()
        .any(|error| error.contains("ccusage-codex helper was not found")));
}

#[test]
fn watch_totals_use_observed_and_bucket_time_not_source_period_midnight() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let period_start = datetime!(2026-05-09 00:00 UTC);
    let observed_at = OffsetDateTime::now_utc();
    let bucket_at = observed_at - Duration::minutes(observed_at.minute() as i64 % 10);

    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            period_start,
            observed_at,
            bucket_at,
            effective_tokens: 1_300.0,
            ..NormalizedUsageEvent::for_test_at(period_start, 1_300.0)
        })
        .unwrap();

    let vm = build_watch_view_model_for_test(&mech_state(), &usage_db).unwrap();
    assert!(vm.today_effective_tokens >= 1_300.0);
    assert!(vm.current_bucket_effective_tokens >= 1_300.0);
    assert!(vm
        .recent_events
        .iter()
        .any(|event| event.timestamp != "00:00" && event.text.contains("1.3k")));
}

#[test]
fn mixed_provider_health_keeps_ready_source_and_diagnostic_source_visible() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let now = OffsetDateTime::now_utc();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            effective_tokens: 4_200.0,
            ..NormalizedUsageEvent::for_test_at(now, 4_200.0)
        })
        .unwrap();
    usage_store
        .insert_diagnostic(&ProviderDiagnostic {
            provider_surface: "codex".into(),
            code: "missing_helper".into(),
            message: "ccusage-codex helper was not found".into(),
            recorded_at: now,
        })
        .unwrap();

    let vm = build_watch_view_model_for_test(&mech_state(), &usage_db).unwrap();
    assert!(vm
        .source_health
        .iter()
        .any(|source| source.name == "claude-code" && source.status == SourceStatus::Ready));
    assert!(vm.source_health.iter().any(|source| {
        source.name == "codex"
            && source.status == SourceStatus::Diagnostic
            && source.diagnostic_code.as_deref() == Some("missing_helper")
    }));
    assert!(!vm.is_blocked());
}

#[test]
fn latest_evolution_renders_once_for_running_watch() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let _usage_store = UsageStore::open(&usage_db).unwrap();
    let mut state = mech_state();
    state.seen_stage_transitions = vec!["s0->s1".into()];

    let mut vm = build_watch_view_model_for_test(&state, &usage_db).unwrap();
    assert_eq!(vm.latest_evolution.as_deref(), Some("s0->s1"));
    assert!(!vm.acknowledged_evolution_for_test("s0->s1"));
    vm.acknowledge_latest_evolution_for_test();
    assert!(vm.acknowledged_evolution_for_test("s0->s1"));
}

fn mech_state() -> PetState {
    let mut state = PetState::new_for_test("servo-watch-seed", "bolt");
    state.pet.generated_species = "mech".into();
    state.stage = "s4".into();
    state.xp = 8.5;
    state.lifetime_effective_tokens = 123_456.0;
    state.vitals.fed = 88.0;
    state.vitals.happiness = 82.0;
    state.vitals.energy = 61.0;
    state.created_at = datetime!(2026-05-01 12:00 UTC);
    state.recent_events = vec!["bolt tightened a tiny gear".into()];
    state
}
