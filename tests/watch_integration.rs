use glorp::{
    commands::watch::{build_watch_view_model_for_test, build_watch_view_model_for_test_at},
    storage::{
        state::{NarrativeEvent, PetState},
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
    seed_snapshot_for_test(
        &mut usage_store,
        glorp::usage::day_axis::tokenmaxxing_provider_day(now),
        "claude-code",
        5_000.0,
        now,
    );

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
fn legacy_applied_tokenmaxxing_rows_do_not_inflate_snapshot_today() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    usage
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            total_tokens: 1_060.0,
            effective_tokens: 1_060.0,
            ..NormalizedUsageEvent::for_test_at(now, 1_060.0)
        })
        .unwrap();
    seed_snapshot_for_test(
        &mut usage,
        time::macros::date!(2026 - 07 - 06),
        "claude-code",
        531.0,
        now,
    );

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(
        vm.today_effective_tokens, 531.0,
        "legacy applied tokenmaxxing rows must not inflate snapshot-backed provider truth"
    );
    assert_eq!(vm.current_bucket_effective_tokens, 1_060.0);
}

#[test]
fn missing_snapshot_does_not_render_zero_provider_truth() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let _usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(
        vm.today_snapshot_state,
        glorp::usage::snapshot::SnapshotState::Missing
    );
    assert_eq!(vm.today_effective_tokens, 0.0);
    assert!(vm
        .source_health
        .iter()
        .all(|source| source.snapshot_state != glorp::usage::snapshot::SnapshotState::Current));
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
    seed_snapshot_for_test(
        &mut usage_store,
        glorp::usage::day_axis::tokenmaxxing_provider_day(observed_at),
        "claude-code",
        1_300.0,
        observed_at,
    );

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
    state.seen_stage_transitions = vec![glorp::game::evolution::Stage::S1];

    let vm = build_watch_view_model_for_test(&state, &usage_db).unwrap();
    assert_eq!(vm.latest_evolution.as_deref(), Some("s1"));
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
    let codex_health = vm.source_health.iter().find(|s| s.name == "codex").unwrap();
    assert_ne!(codex_health.status, SourceStatus::Ready);
}

#[test]
fn watch_view_model_exposes_catalog_backed_habitat_props() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let _usage_store = UsageStore::open(&usage_db).unwrap();
    let mut state = mech_state();
    state
        .habitat
        .earned_props
        .push(glorp::storage::state::EarnedHabitatProp {
            id: glorp::storage::state::HabitatPropId::new("codex_signal_lamp"),
            earned_at: datetime!(2026-05-11 12:00:00 UTC),
            source: glorp::storage::state::HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
        });
    state
        .habitat
        .earned_props
        .push(glorp::storage::state::EarnedHabitatProp {
            id: glorp::storage::state::HabitatPropId::new("non_catalog_prop_for_filter_test"),
            earned_at: datetime!(2026-05-11 12:01:00 UTC),
            source: glorp::storage::state::HabitatPropSource::HeavySession,
        });

    let vm =
        build_watch_view_model_for_test_at(&state, &usage_db, datetime!(2026-05-11 12:02:00 UTC))
            .unwrap();

    assert_eq!(vm.habitat.earned_props.len(), 1);
    assert_eq!(vm.habitat.earned_props[0].id.as_str(), "codex_signal_lamp");
    assert_eq!(
        vm.habitat.earned_props[0].earned_at,
        datetime!(2026-05-11 12:00:00 UTC)
    );
    assert_eq!(vm.habitat.earned_props[0].display_priority, 70);
    assert_eq!(
        vm.habitat.earned_props[0].kind,
        glorp::game::habitat::HabitatPropKind::Trophy
    );
}

fn mech_state() -> PetState {
    let mut state = PetState::new_for_test("servo-watch-seed", "bolt");
    state.pet.generated_species = glorp::pet::generation::Species::Mech;
    state.stage = glorp::game::evolution::Stage::S4;
    state.xp = 8.5;
    state.lifetime_effective_tokens = 123_456.0;
    state.vitals.fed = 88.0;
    state.vitals.happiness = 82.0;
    state.vitals.energy = 61.0;
    state.created_at = datetime!(2026-05-01 12:00 UTC);
    state.recent_events = vec![NarrativeEvent {
        observed_at: time::OffsetDateTime::UNIX_EPOCH,
        text: "bolt tightened a tiny gear".into(),
    }];
    state
}

fn seed_snapshot_for_test(
    usage: &mut UsageStore,
    day: time::Date,
    source: &str,
    total: f64,
    observed_at: OffsetDateTime,
) {
    let batch = glorp::usage::snapshot::ProviderSnapshotBatchInput {
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        covered_accounting_sources: None,
        provider_version: "test".into(),
        parser_version: "test".into(),
        observed_at,
    };
    let row = glorp::usage::snapshot::ProviderSnapshotRowInput {
        replacement_scope_id: format!("{source}:local-usage"),
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: source.into(),
        provider_day: day,
        model: Some("test-model".into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some("hash:test".into()),
        cursor_key_hash: "hash:cursor".into(),
        cursor_update: ProviderCursorUpdate {
            provider_surface: source.into(),
            cursor_key: "cursor".into(),
            cursor_value: "value".into(),
            provider_version: "test".into(),
            parser_version: "test".into(),
        },
        raw_token_buckets: None,
        total_tokens: total,
        cost_usd: None,
        confidence: "local-log-derived".into(),
    };
    usage
        .write_provider_snapshot_batch(&batch, &[row], &[])
        .unwrap();
}

#[test]
fn rate_per_hour_grows_with_more_recent_events() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

    for i in 0..10_i64 {
        let observed_at = now - Duration::minutes(15 * i + 5);
        store
            .insert_event(&NormalizedUsageEvent {
                observed_at,
                bucket_at: observed_at,
                effective_tokens: 10_000.0,
                ..NormalizedUsageEvent::for_test_at(observed_at, 10_000.0)
            })
            .unwrap();
    }

    let state = PetState::new_for_test("test", "buddy");
    let vm_a = build_watch_view_model_for_test_at(&state, &db_path, now).unwrap();
    let rate_a = vm_a.progress.rate_per_hour;

    // Add one large event just before now so it lands inside the canonical
    // half-open 1-hour window.
    let recent_at = now - Duration::seconds(1);
    store
        .insert_event(&NormalizedUsageEvent {
            observed_at: recent_at,
            bucket_at: recent_at,
            effective_tokens: 50_000.0,
            provider_surface: "codex".to_string(),
            ..NormalizedUsageEvent::for_test_at(recent_at, 50_000.0)
        })
        .unwrap();
    drop(store);

    let vm_b = build_watch_view_model_for_test_at(&state, &db_path, now).unwrap();
    let rate_b = vm_b.progress.rate_per_hour;
    assert!(
        rate_b > rate_a,
        "rate must grow with more recent contribution (a={rate_a}, b={rate_b})"
    );
}

#[test]
fn rate_per_hour_uses_only_canonical_tokenmaxxing_totals() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = datetime!(2026-06-19 18:00:00 UTC);
    let canonical_at = now - Duration::minutes(5);
    let legacy_at = now - Duration::minutes(10);

    store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".to_string(),
            observed_at: canonical_at,
            bucket_at: canonical_at,
            total_tokens: 42_000.0,
            effective_tokens: 7.0,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
            ..NormalizedUsageEvent::for_test_at(canonical_at, 7.0)
        })
        .unwrap();
    store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "legacy-claude".to_string(),
            observed_at: legacy_at,
            bucket_at: legacy_at,
            total_tokens: 1_000_000.0,
            effective_tokens: 1_000_000.0,
            token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
            ..NormalizedUsageEvent::for_test_at(legacy_at, 1_000_000.0)
        })
        .unwrap();
    drop(store);

    let vm = build_watch_view_model_for_test_at(&mech_state(), &db_path, now).unwrap();

    assert_eq!(vm.progress.rate_per_hour, 42_000.0);
}

#[test]
fn rate_momentum_uses_canonical_windows_and_directions() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = datetime!(2026-06-19 18:00:00 UTC);

    for (at, tokens) in [
        (now - Duration::minutes(5), 12_000.0),
        (now - Duration::minutes(15), 2_000.0),
        (now - Duration::minutes(30), 20_000.0),
        (now - Duration::minutes(90), 80_000.0),
    ] {
        store
            .insert_event(&NormalizedUsageEvent {
                provider_surface: "codex".to_string(),
                observed_at: at,
                bucket_at: at,
                total_tokens: tokens,
                effective_tokens: 1.0,
                token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
                ..NormalizedUsageEvent::for_test_at(at, 1.0)
            })
            .unwrap();
    }
    store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "legacy".to_string(),
            observed_at: now - Duration::minutes(4),
            bucket_at: now - Duration::minutes(4),
            total_tokens: 999_999.0,
            effective_tokens: 999_999.0,
            token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
            ..NormalizedUsageEvent::for_test_at(now - Duration::minutes(4), 999_999.0)
        })
        .unwrap();

    let vm = build_watch_view_model_for_test_at(&mech_state(), &db_path, now).unwrap();

    assert_eq!(vm.rate_momentum.pulse.current_tokens, 12_000.0);
    assert_eq!(vm.rate_momentum.pulse.previous_tokens, 2_000.0);
    assert_eq!(
        vm.rate_momentum.pulse.direction,
        glorp::tui::view_model::RateDirection::Up
    );
    assert_eq!(vm.rate_momentum.hour.current_tokens, 34_000.0);
    assert_eq!(vm.rate_momentum.hour.previous_tokens, 80_000.0);
    assert_eq!(
        vm.rate_momentum.hour.direction,
        glorp::tui::view_model::RateDirection::Down
    );
    assert_eq!(
        vm.rate_momentum.companion_direction,
        glorp::tui::view_model::RateDirection::Up
    );
}

#[test]
fn rate_momentum_normalizes_fractional_now_before_window_queries() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = datetime!(2026-06-19 18:00:00.5 UTC);
    let event_at = datetime!(2026-06-19 17:59:59 UTC);

    store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".to_string(),
            observed_at: event_at,
            bucket_at: event_at,
            total_tokens: 1_500.0,
            effective_tokens: 1.0,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
            ..NormalizedUsageEvent::for_test_at(event_at, 1.0)
        })
        .unwrap();

    let vm = build_watch_view_model_for_test_at(&mech_state(), &db_path, now).unwrap();

    assert_eq!(vm.rate_momentum.pulse.current_tokens, 1_500.0);
    assert_eq!(
        vm.rate_momentum.pulse.direction,
        glorp::tui::view_model::RateDirection::Up
    );
}

#[test]
fn tokenmaxxing_day_axis_interprets_date_as_los_angeles_midnight() {
    use glorp::usage::day_axis::{parse_agentsview_period_date, tokenmaxxing_day_start};
    use time::{Date, Month};

    let date = Date::from_calendar_date(2026, Month::June, 18).unwrap();
    assert_eq!(
        tokenmaxxing_day_start(date),
        datetime!(2026-06-18 07:00 UTC)
    );

    let (parsed_date, parsed_start) = parse_agentsview_period_date("2026-06-18").unwrap();
    assert_eq!(parsed_date, date);
    assert_eq!(parsed_start, datetime!(2026-06-18 07:00 UTC));
}

#[test]
fn tokenmaxxing_day_axis_handles_los_angeles_dst_boundaries() {
    use glorp::usage::day_axis::tokenmaxxing_day_start;
    use time::{Date, Month};

    let before_spring = Date::from_calendar_date(2026, Month::March, 8).unwrap();
    let after_spring = Date::from_calendar_date(2026, Month::March, 9).unwrap();
    let fall_back_day = Date::from_calendar_date(2026, Month::November, 1).unwrap();
    let after_fall = Date::from_calendar_date(2026, Month::November, 2).unwrap();

    assert_eq!(
        tokenmaxxing_day_start(before_spring),
        datetime!(2026-03-08 08:00 UTC)
    );
    assert_eq!(
        tokenmaxxing_day_start(after_spring),
        datetime!(2026-03-09 07:00 UTC)
    );
    assert_eq!(
        tokenmaxxing_day_start(fall_back_day),
        datetime!(2026-11-01 07:00 UTC)
    );
    assert_eq!(
        tokenmaxxing_day_start(after_fall),
        datetime!(2026-11-02 08:00 UTC)
    );
}

#[test]
fn watch_token_totals_use_tokenmaxxing_day_axis_and_external_source_labels() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026-06-19 06:30 UTC); // 2026-06-18 23:30 in Los Angeles.
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".into(),
            period_start: datetime!(2026-06-18 07:00 UTC),
            observed_at: datetime!(2026-06-18 23:30 UTC),
            bucket_at: datetime!(2026-06-18 23:30 UTC),
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            total_tokens: 669_369_020.0,
            effective_tokens: 12.0,
            ..NormalizedUsageEvent::for_test_at(now, 12.0)
        })
        .unwrap();
    seed_snapshot_for_test(
        &mut usage_store,
        glorp::usage::day_axis::tokenmaxxing_provider_day(now),
        "codex",
        669_369_020.0,
        now,
    );

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(vm.today_effective_tokens, 669_369_020.0);
    assert!(vm
        .source_breakdown
        .iter()
        .any(|source| source.name == "codex" && source.effective_tokens == 669_369_020.0));
}

#[test]
fn seeded_history_is_hidden_from_watch_activity_surfaces() {
    let dir = tempfile::tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 06 - 10 12:00 UTC);
    let historical = now - time::Duration::days(1);
    let event = NormalizedUsageEvent {
        provider_surface: "codex".into(),
        ..NormalizedUsageEvent::for_test_at(historical, 669_000_000.0)
    };
    let cursor = ProviderCursorUpdate {
        provider_surface: "codex".into(),
        cursor_key: "codex-seed-key".into(),
        cursor_value: "codex-seed-value".into(),
        provider_version: "test-provider".into(),
        parser_version: "test-parser".into(),
    };
    usage_store
        .seed_source_history(&[(event, cursor)], None, now)
        .unwrap();

    let vm = build_watch_view_model_for_test_at(
        &PetState::new_for_test("mochi-7f3a", "mochi"),
        &usage_db,
        now,
    )
    .unwrap();

    assert_eq!(vm.today_effective_tokens, 0.0);
    assert!(vm.source_breakdown.is_empty());
    assert!(vm
        .recent_events
        .iter()
        .all(|event| !event.text.contains("codex")));
}
