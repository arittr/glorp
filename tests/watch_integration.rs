use glorp::{
    commands::watch::{build_watch_view_model_for_test, build_watch_view_model_for_test_at},
    storage::{
        state::PetState,
        usage_store::{NormalizedUsageEvent, ProviderCursorUpdate, ProviderDiagnostic, UsageStore},
    },
    tui::{style::LogKind, view_model::SourceStatus},
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
    assert_eq!(vm.stage, "mech");
    // Mech templates use box-drawing characters (U+2500..U+257F) and block
    // elements (U+2580..U+259F); the presence of any of those proves the
    // render path produced a real Mech template, not the fixture cat.
    assert!(
        art.chars()
            .any(|c| ('\u{2500}'..='\u{257F}').contains(&c)
                || ('\u{2580}'..='\u{259F}').contains(&c)),
        "pet_art should contain block/box-drawing glyphs, got: {art:?}"
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
    // Pin `now` to a mid-day UTC instant so the `now - 9min` event below
    // doesn't cross midnight when the test happens to run in the first
    // few minutes of UTC.
    let now = datetime!(2026-05-11 12:00:00 UTC);
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
    let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

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

#[test]
fn log_aggregates_smeared_buckets_into_one_event() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let observed_at = OffsetDateTime::now_utc();
    let bucket_count = 8usize;
    let cursor = ProviderCursorUpdate {
        provider_surface: "claude-code".into(),
        cursor_key: "test-cursor".into(),
        cursor_value: "test-value".into(),
        provider_version: "test-provider".into(),
        parser_version: "test-parser".into(),
    };
    for bucket_index in 0..bucket_count {
        let bucket_at = observed_at
            - Duration::minutes(bucket_count.saturating_sub(bucket_index + 1) as i64 * 10);
        usage_store
            .insert_unapplied_event_bucket(
                &NormalizedUsageEvent {
                    observed_at,
                    bucket_at,
                    effective_tokens: 25_000.0,
                    ..NormalizedUsageEvent::for_test_at(observed_at, 25_000.0)
                },
                &cursor,
                bucket_index,
                bucket_count,
            )
            .unwrap();
    }

    let vm = build_watch_view_model_for_test(&mech_state(), &usage_db).unwrap();
    let aggregated: Vec<_> = vm
        .recent_events
        .iter()
        .filter(|event| {
            matches!(event.kind, LogKind::Usage) && event.text.contains("claude-code added")
        })
        .collect();
    assert_eq!(
        aggregated.len(),
        1,
        "expected one aggregated usage entry per provider delta, got: {:#?}",
        aggregated
    );
    assert!(
        aggregated[0].text.contains("200.0k"),
        "summed effective tokens should be 8 * 25_000 = 200_000, got: {:?}",
        aggregated[0].text
    );
}

#[test]
fn diagnostic_log_dedupes_by_surface_and_code() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let usage_store = UsageStore::open(&usage_db).unwrap();
    let now = OffsetDateTime::now_utc();
    for offset_seconds in 0..5 {
        usage_store
            .insert_diagnostic(&ProviderDiagnostic {
                provider_surface: "codex".into(),
                code: "invalid_period_start".into(),
                message: format!("repeat #{offset_seconds}"),
                recorded_at: now - Duration::seconds(offset_seconds),
            })
            .unwrap();
    }

    let vm = build_watch_view_model_for_test(&mech_state(), &usage_db).unwrap();
    let diagnostics: Vec<_> = vm
        .recent_events
        .iter()
        .filter(|event| matches!(event.kind, LogKind::Diagnostic))
        .collect();
    assert_eq!(
        diagnostics.len(),
        1,
        "expected one deduped diagnostic entry per (surface, code), got: {:#?}",
        diagnostics
    );
    assert!(
        diagnostics[0].text.contains("invalid_period_start"),
        "diagnostic entry should preserve the code text, got: {:?}",
        diagnostics[0].text
    );
}

#[test]
fn bio_view_renders_from_real_pet_state() {
    use time::{Date, Duration, Month, PrimitiveDateTime, Time};

    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let _store = UsageStore::open(&usage_db).unwrap();

    let base = PrimitiveDateTime::new(
        Date::from_calendar_date(2026, Month::May, 11).unwrap(),
        Time::from_hms(4, 0, 0).unwrap(),
    )
    .assume_utc();

    let mut state = PetState::new_for_test("test", "buddy");
    state.created_at = base;
    let now = base + Duration::days(7) + Duration::hours(13);

    let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();
    assert!(
        vm.bio.hatched_label.contains("may"),
        "expected hatched_label to contain 'may', got {}",
        vm.bio.hatched_label
    );
    assert_eq!(vm.bio.age_label, "7d");
}

#[test]
fn blocked_source_surfaces_via_source_health() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let usage_store = UsageStore::open(&usage_db).unwrap();
    let now = OffsetDateTime::now_utc();
    usage_store
        .insert_diagnostic(&ProviderDiagnostic {
            provider_surface: "codex".into(),
            code: "blocked".into(),
            message: "binary missing".into(),
            recorded_at: now,
        })
        .unwrap();
    drop(usage_store);

    let state = PetState::new_for_test("test", "buddy");
    let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();
    let codex_health = vm
        .source_health
        .iter()
        .find(|s| s.name == "codex")
        .unwrap();
    assert_ne!(codex_health.status, SourceStatus::Ready);
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
