// tests/activity_identity_runtime.rs
use glorp::game::habitat::{unlock_habitat_props, FIRST_ENSEMBLE_DAY};
use glorp::storage::state::PetState;
use glorp::storage::usage_store::{NormalizedUsageEvent, UsageStore};
use glorp::tui::identity::ActivityIdentityProfile;
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
