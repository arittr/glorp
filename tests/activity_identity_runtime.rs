// tests/activity_identity_runtime.rs
use glorp::game::habitat::{unlock_habitat_props, FIRST_ENSEMBLE_DAY};
use glorp::storage::state::PetState;
use glorp::storage::usage_store::{NormalizedUsageEvent, ProviderCursorUpdate, UsageStore};
use glorp::tui::identity::{ActivityIdentityProfile, SourceDiversity, TokenShapePersonality};
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

fn sample_event_at(
    surface: &str,
    observed_at: OffsetDateTime,
    tokens: f64,
) -> NormalizedUsageEvent {
    NormalizedUsageEvent {
        provider_surface: surface.to_string(),
        observed_at,
        bucket_at: observed_at,
        effective_tokens: tokens,
        ..NormalizedUsageEvent::for_test_at(observed_at, tokens)
    }
}

#[test]
fn first_contact_with_unknown_source_does_not_unlock_codex_signal_lamp() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();

    store
        .insert_event(&sample_event_at(
            "unknown",
            now - Duration::minutes(5),
            5_000.0,
        ))
        .unwrap();
    drop(store);

    let mut state = PetState::new_for_test("test", "Buddy");
    let rows = UsageStore::open(&db_path)
        .unwrap()
        .unapplied_events(100)
        .unwrap();
    let today_source_totals = vec![("unknown".to_string(), 5_000.0)];
    let profile = ActivityIdentityProfile::default();
    unlock_habitat_props(
        &mut state,
        &rows,
        5_000.0,
        glorp::game::metabolism::Mood::Content,
        glorp::game::metabolism::Mood::Content,
        now,
        &profile,
        &today_source_totals,
    );
    assert!(!state
        .habitat
        .earned_props
        .iter()
        .any(|p| p.id.as_str() == "codex_signal_lamp"));
}

#[test]
fn first_contact_historical_rows_do_not_award_first_ensemble_day_milestone() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();

    for surface in ["claude-code", "codex", "gemini", "opencode"] {
        store
            .insert_event(&sample_event_at(surface, now - Duration::hours(2), 5_000.0))
            .unwrap();
    }
    drop(store);

    let mut state = PetState::new_for_test("test", "Buddy");
    let rows = UsageStore::open(&db_path)
        .unwrap()
        .unapplied_events(100)
        .unwrap();
    let today_source_totals: Vec<_> = ["claude-code", "codex", "gemini", "opencode"]
        .iter()
        .map(|s| (s.to_string(), 5_000.0))
        .collect();
    let profile = ActivityIdentityProfile::default();
    unlock_habitat_props(
        &mut state,
        &rows,
        20_000.0,
        glorp::game::metabolism::Mood::Content,
        glorp::game::metabolism::Mood::Content,
        now,
        &profile,
        &today_source_totals,
    );
    assert!(!state
        .habitat
        .earned_props
        .iter()
        .any(|p| p.id.as_str() == FIRST_ENSEMBLE_DAY));
}

#[test]
fn activity_identity_source_diversity_uses_snapshot_but_shape_uses_feed_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = time::macros::datetime!(2026 - 07 - 06 20:00 UTC);
    seed_snapshot_for_activity_test(&mut usage, time::macros::date!(2026 - 07 - 06), now);
    usage
        .insert_event(&glorp::storage::usage_store::NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            input_tokens: 0.0,
            output_tokens: 100.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 0.0,
            total_tokens: 100.0,
            effective_tokens: 100.0,
            ..glorp::storage::usage_store::NormalizedUsageEvent::for_test_at(now, 100.0)
        })
        .unwrap();

    let state = glorp::storage::state::PetState::new_for_test("identity", "miso");
    let vm =
        glorp::commands::watch::build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

    assert_eq!(
        vm.activity_identity.source_diversity,
        SourceDiversity::DualLane
    );
    assert_eq!(
        vm.activity_identity.token_shape,
        TokenShapePersonality::OutputHeavy
    );
}

fn seed_snapshot_for_activity_test(
    usage: &mut glorp::storage::usage_store::UsageStore,
    day: time::Date,
    observed_at: time::OffsetDateTime,
) {
    let batch = glorp::usage::snapshot::ProviderSnapshotBatchInput {
        collector_scope_id: "test:local-usage".into(),
        collector_surface: "test".into(),
        command: "test snapshot".into(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        covered_accounting_sources: None,
        provider_version: "test".into(),
        parser_version: "test".into(),
        observed_at,
    };
    let rows = ["claude-code", "codex"]
        .iter()
        .map(|source| glorp::usage::snapshot::ProviderSnapshotRowInput {
            replacement_scope_id: "test:local-usage".into(),
            collector_scope_id: "test:local-usage".into(),
            collector_surface: "test".into(),
            command: "test snapshot".into(),
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            accounting_source: (*source).into(),
            provider_day: day,
            model: Some(format!("{source}-model")),
            source_surface: "daily".into(),
            provider_period: day.to_string(),
            raw_source_id_hash: Some(format!("hash:{source}")),
            cursor_key_hash: format!("hash:{source}"),
            cursor_update: ProviderCursorUpdate {
                provider_surface: (*source).into(),
                cursor_key: format!("cursor:{source}"),
                cursor_value: "value".into(),
                provider_version: "test".into(),
                parser_version: "test".into(),
            },
            raw_token_buckets: None,
            total_tokens: 100.0,
            cost_usd: None,
            confidence: "local-log-derived".into(),
        })
        .collect::<Vec<_>>();
    usage
        .write_provider_snapshot_batch(&batch, &rows, &[])
        .unwrap();
}
