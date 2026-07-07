use glorp::config::AppConfig;
use glorp::paths::AppPaths;
use glorp::storage::state::{PetState, StateStore, Vitals};
use glorp::storage::usage_store::{NormalizedUsageEvent, UsageStore};
use rusqlite::Connection;
use tempfile::tempdir;
use time::{macros::datetime, Duration, OffsetDateTime};

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
        Vitals { fed: 70.0, happiness: 70.0, energy: 70.0 }
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
fn snapshot_tables_exist_without_raw_transcript_columns() {
    let dir = tempfile::tempdir().unwrap();
    let store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let conn = store.raw_connection_for_test();

    let tables = [
        "provider_snapshot_batches",
        "provider_snapshot_runs",
        "provider_snapshot_rows",
        "provider_corrections",
        "provider_snapshot_diagnostics",
        "provider_canonical_collectors",
        "provider_source_contacts",
        "provider_feed_highwaters",
    ];

    for table in tables {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "missing table {table}");

        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for forbidden in [
            "prompt",
            "response",
            "raw_prompt",
            "raw_response",
            "file_path",
            "project_path",
        ] {
            assert!(
                !names.iter().any(|name| name.contains(forbidden)),
                "{table} contains forbidden column {forbidden}: {names:?}"
            );
        }
        if table == "provider_snapshot_rows" {
            assert!(
                names.iter().any(|name| name == "raw_source_id_hash"),
                "sanitized raw source identity hash column must exist: {names:?}"
            );
        }
    }
}

#[test]
fn usage_events_store_observed_and_bucket_times_separately_from_period_start() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let period_start = datetime!(2026-05-09 00:00 UTC);
    let observed_at = datetime!(2026-05-09 19:17 UTC);
    let bucket_at = datetime!(2026-05-09 19:10 UTC);

    store
        .insert_event(&NormalizedUsageEvent {
            observed_at,
            bucket_at,
            ..NormalizedUsageEvent::for_test_at(period_start, 420.0)
        })
        .unwrap();

    let events = store.recent_events(1).unwrap();
    assert_eq!(events[0].period_start, period_start);
    assert_eq!(events[0].observed_at, observed_at);
    assert_eq!(events[0].bucket_at, bucket_at);
}

#[test]
fn canonical_total_queries_exclude_legacy_weighted_rows() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let now = datetime!(2026-06-18 19:10 UTC);

    let legacy = NormalizedUsageEvent {
        token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
        total_tokens: 999_999.0,
        effective_tokens: 999_999.0,
        ..NormalizedUsageEvent::for_test_at(now, 999_999.0)
    };
    let canonical = NormalizedUsageEvent {
        provider_surface: "codex".to_string(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
        total_tokens: 715_380_912.0,
        effective_tokens: 715_380_912.0,
        ..NormalizedUsageEvent::for_test_at(now, 715_380_912.0)
    };

    store.insert_event(&legacy).unwrap();
    store.insert_event(&canonical).unwrap();

    let total = store
        .canonical_total_tokens_between(now - Duration::minutes(1), now + Duration::minutes(1))
        .unwrap();
    let by_source = store
        .canonical_total_tokens_by_source_between(
            now - Duration::minutes(1),
            now + Duration::minutes(1),
        )
        .unwrap();

    assert_eq!(total, 715_380_912.0);
    assert_eq!(by_source, vec![("codex".to_string(), 715_380_912.0)]);
}

#[test]
fn old_usage_rows_migrate_with_conservative_event_times() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("usage.sqlite");
    {
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE usage_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_surface TEXT NOT NULL,
                provider_version TEXT NOT NULL,
                parser_version TEXT NOT NULL,
                command TEXT NOT NULL,
                source_surface TEXT NOT NULL,
                period_start TEXT NOT NULL,
                period_date TEXT NOT NULL,
                model TEXT,
                input_tokens REAL NOT NULL,
                output_tokens REAL NOT NULL,
                cache_creation_tokens REAL NOT NULL,
                cache_read_tokens REAL NOT NULL,
                reasoning_output_tokens REAL NOT NULL,
                effective_tokens REAL NOT NULL,
                cost_usd REAL,
                confidence TEXT NOT NULL
            );
            INSERT INTO usage_events (
                provider_surface, provider_version, parser_version, command,
                source_surface, period_start, period_date, model,
                input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                reasoning_output_tokens, effective_tokens, cost_usd, confidence
            ) VALUES (
                'claude-code', '18.0.11', '18.0.11', 'ccusage',
                'daily', '2026-05-08T00:00:00Z', '2026-05-08', 'claude-opus-4',
                1, 2, 3, 4, 0, 6, NULL, 'local-log-derived'
            );
            ",
        )
        .unwrap();
    }

    let store = UsageStore::open(&db).unwrap();
    let events = store.recent_events(5).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].observed_at, datetime!(2026-05-08 00:00 UTC));
    assert_eq!(events[0].bucket_at, datetime!(2026-05-08 00:00 UTC));
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
