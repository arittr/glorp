use glorp::config::AppConfig;
use glorp::paths::AppPaths;
use glorp::storage::state::{PetState, StateStore, Vitals};
use glorp::storage::usage_store::{NormalizedUsageEvent, UsageStore};
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

#[test]
fn state_files_stay_inside_config_override() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    assert!(paths.config_file.starts_with(dir.path()));
    assert!(paths.state_file.starts_with(dir.path()));
    assert!(paths.usage_db.starts_with(dir.path()));
}

#[test]
fn default_paths_are_under_home_config_glorp() {
    let home = tempdir().unwrap();
    let paths = AppPaths::from_home_dir(home.path().to_path_buf());
    assert_eq!(paths.config_dir, home.path().join(".config/glorp"));
    assert_eq!(
        paths.config_file,
        home.path().join(".config/glorp/config.toml")
    );
    assert_eq!(
        paths.state_file,
        home.path().join(".config/glorp/state.json")
    );
    assert_eq!(
        paths.usage_db,
        home.path().join(".config/glorp/usage.sqlite")
    );
}

#[test]
fn config_dir_env_override_wins_when_resolving_paths() {
    let dir = tempdir().unwrap();
    std::env::set_var("GLORP_CONFIG_DIR", dir.path());
    let paths = AppPaths::resolve().unwrap();
    std::env::remove_var("GLORP_CONFIG_DIR");
    assert_eq!(paths.config_dir, dir.path());
    assert_eq!(paths.config_file, dir.path().join("config.toml"));
    assert_eq!(paths.state_file, dir.path().join("state.json"));
    assert_eq!(paths.usage_db, dir.path().join("usage.sqlite"));
}

#[test]
fn pet_state_round_trips_schema_and_vitals() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let store = StateStore::new(paths.state_file.clone());
    let state = PetState::new_for_test("mochi-7f3a", "mochi");
    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.pet.seed, "mochi-7f3a");
    assert_eq!(loaded.pet.accepted_name, "mochi");
    assert_eq!(
        loaded.vitals,
        Vitals {
            fed: 70.0,
            happiness: 70.0,
            energy: 70.0
        }
    );
}

#[test]
fn malformed_or_unsupported_state_returns_error_without_resetting() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let store = StateStore::new(paths.state_file.clone());

    std::fs::write(&paths.state_file, "{not valid json").unwrap();
    let malformed = store.load().unwrap_err().to_string();
    assert!(malformed.contains("malformed") || malformed.contains("JSON"));
    assert!(paths.state_file.exists());

    std::fs::write(&paths.state_file, r#"{"schema_version":999}"#).unwrap();
    let unsupported = store.load().unwrap_err().to_string();
    assert!(unsupported.contains("unsupported schema version"));
    assert!(paths.state_file.exists());
}

#[test]
fn config_defaults_and_cache_read_weight_override_load() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let default_config = AppConfig::load_or_default(&paths.config_file).unwrap();
    assert_eq!(default_config.cache_read_weight, 0.03);

    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(&paths.config_file, "cache_read_weight = 0.05\n").unwrap();
    let overridden = AppConfig::load_or_default(&paths.config_file).unwrap();
    assert_eq!(overridden.cache_read_weight, 0.05);
}

#[test]
fn normalized_usage_storage_never_persists_transcript_payloads() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let event = NormalizedUsageEvent::for_test_with_ignored_payloads(
        "claude-code",
        "prompt text must not persist",
        "response text must not persist",
        "tool payload must not persist",
    );
    store.insert_event(&event).unwrap();
    let raw_db = std::fs::read(&paths.usage_db).unwrap();
    let text = String::from_utf8_lossy(&raw_db);
    assert!(!text.contains("prompt text must not persist"));
    assert!(!text.contains("response text must not persist"));
    assert!(!text.contains("tool payload must not persist"));
}

#[test]
fn compacts_events_older_than_ninety_days_without_losing_lifetime_counters() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let now = OffsetDateTime::parse(
        "2026-05-09T12:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    store
        .insert_event(&NormalizedUsageEvent::for_test_at(
            now - Duration::days(91),
            1000.0,
        ))
        .unwrap();
    store
        .insert_event(&NormalizedUsageEvent::for_test_at(now, 250.0))
        .unwrap();
    store.compact_before(now - Duration::days(90)).unwrap();
    assert_eq!(store.recent_event_count().unwrap(), 1);
    assert_eq!(
        store
            .daily_aggregate_effective_tokens("claude-code")
            .unwrap(),
        1000.0
    );
    assert_eq!(store.lifetime_effective_tokens().unwrap(), 1250.0);
}
