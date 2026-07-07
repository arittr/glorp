use assert_cmd::Command;
use glorp::storage::state::{write_state_for_test, PetState, PetStateFixture};
use glorp::storage::usage_store::{NormalizedUsageEvent, ProviderCursorUpdate, UsageStore};
use glorp::usage::snapshot::{ProviderSnapshotBatchInput, ProviderSnapshotRowInput};
use glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1;
use predicates::prelude::*;
use tempfile::tempdir;
use time::{Date, Duration, OffsetDateTime};

const AGENTSVIEW_OK: &str = "tests/fixtures/helpers/agentsview-ok.mjs";
const AGENTSVIEW_NEXT: &str = "tests/fixtures/helpers/agentsview-next.mjs";
const AGENTSVIEW_FAILS: &str = "tests/fixtures/helpers/agentsview-fails.mjs";
const AGENTSVIEW_INVALID_JSON: &str = "tests/fixtures/helpers/agentsview-invalid-json.mjs";
const AGENTSVIEW_SECRET_STDERR: &str = "tests/fixtures/helpers/agentsview-secret-stderr.mjs";
const USAGE_NOW_AGENTSVIEW_FIXTURE: &str = "2026-06-18T20:00:00Z";

#[test]
fn status_is_pipe_friendly_when_pet_exists() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi"))
        .stdout(predicate::str::contains("stage progress:"))
        .stdout(predicate::str::contains("provider today"))
        .stdout(predicate::str::contains("accepted recent food"))
        .stdout(predicate::str::contains("pet lifetime food"))
        .stdout(predicate::str::contains("effective tokens").not())
        .stdout(predicate::str::contains("local-log-derived"))
        .stdout(predicate::str::contains("provider health:"))
        .stdout(predicate::str::contains("billing").not());
}

#[test]
fn status_labels_provider_today_recent_food_and_pet_lifetime_separately() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider today"))
        .stdout(predicate::str::contains(
            "provider today (current provider snapshot): 1300",
        ))
        .stdout(predicate::str::contains("accepted recent food"))
        .stdout(predicate::str::contains("pet lifetime food"));
}

#[test]
fn doctor_refresh_usage_snapshots_reports_before_after_without_feeding_pet() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();
    let state_before_refresh = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    let lifetime_before_refresh = UsageStore::open(&dir.path().join("usage.sqlite"))
        .unwrap()
        .lifetime_effective_tokens()
        .unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .args(["doctor", "--refresh-usage-snapshots"])
        .assert()
        .success()
        .stdout(predicate::str::contains("refresh usage snapshots"))
        .stdout(predicate::str::contains("before provider today"))
        .stdout(predicate::str::contains("after provider today"))
        .stdout(predicate::str::contains("after provider today: 1300"))
        .stdout(predicate::str::contains("pet state unchanged"));

    let state_after_refresh = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    let usage_after_refresh = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    assert_eq!(
        state_after_refresh, state_before_refresh,
        "snapshot-only doctor repair must not rewrite pet state"
    );
    assert_eq!(
        usage_after_refresh.lifetime_effective_tokens().unwrap(),
        lifetime_before_refresh,
        "snapshot-only doctor repair must not feed the pet"
    );
    assert_eq!(
        usage_after_refresh
            .snapshot_totals_for_provider_day(time::macros::date!(2026 - 06 - 10))
            .unwrap()
            .value
            .unwrap()
            .total_tokens,
        1300.0
    );
}

#[test]
fn doctor_reports_missing_helpers_with_setup_instructions() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: agentsview"))
        .stdout(predicate::str::contains("required usage helper: blocked"))
        .stdout(predicate::str::contains("Default provider blocked."))
        .stdout(predicate::str::contains("GLORP_AGENTSVIEW_BIN"));
}

#[test]
fn doctor_reports_agentsview_provider_as_required_default() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: agentsview"))
        .stdout(predicate::str::contains("required usage helper: found"))
        .stdout(predicate::str::contains(
            "helper version: agentsview provider=agentsview v0.32.1 parser=agentsview v0.32.1",
        ));
}

#[test]
fn doctor_reports_missing_agentsview_as_default_provider_blocked() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("required usage helper: blocked"))
        .stdout(predicate::str::contains("Default provider blocked."))
        .stdout(predicate::str::contains("agentsview helper was not found"))
        .stdout(predicate::str::contains("GLORP_AGENTSVIEW_BIN"));
}

#[test]
fn doctor_sanitizes_agentsview_helper_stderr() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_SECRET_STDERR)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("helper_exit"))
        .stdout(predicate::str::contains("secret prompt").not())
        .stdout(predicate::str::contains("secret response").not())
        .stdout(predicate::str::contains("tool payload").not())
        .stdout(predicate::str::contains("/Users/drew/private").not());
}

#[test]
fn doctor_reports_helper_versions_when_available() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("helpers: found"))
        .stdout(predicate::str::contains("provider command health: ok"))
        .stdout(predicate::str::contains("provider: agentsview"))
        .stdout(predicate::str::contains(
            "helper version: agentsview provider=agentsview v0.32.1 parser=agentsview v0.32.1",
        ));
}

#[test]
fn diagnostics_do_not_print_raw_transcript_content() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_SECRET_STDERR)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("secret prompt").not())
        .stdout(predicate::str::contains("secret response").not())
        .stdout(predicate::str::contains("secret tool payload").not());
}

#[test]
fn doctor_sanitizes_invalid_json_and_helper_stderr() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_INVALID_JSON)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid_json"))
        .stdout(predicate::str::contains("secret prompt").not());

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_SECRET_STDERR)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("helper_exit"))
        .stdout(predicate::str::contains("secret response").not());
}

#[test]
fn repeated_provider_failures_keep_last_known_pet_state() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    for _ in 0..2 {
        Command::cargo_bin("glorp")
            .unwrap()
            .env("GLORP_CONFIG_DIR", dir.path())
            .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_FAILS)
            .arg("status")
            .assert()
            .success()
            .stdout(predicate::str::contains("mochi"))
            .stdout(
                predicate::str::contains("helper_exit").or(predicate::str::contains("blocked")),
            );
    }

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("mochi"));
}

#[test]
fn status_includes_recent_evolution_event_when_present() {
    let dir = tempdir().unwrap();
    write_state_for_test(
        dir.path(),
        PetStateFixture::named("mochi")
            .with_stage(glorp::game::evolution::Stage::S3)
            .with_recent_event("evolved from sprout to bytebuddy"),
    )
    .unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi"))
        .stdout(predicate::str::contains("evolved from sprout to bytebuddy"));
}

#[test]
fn status_clamps_zero_usage_display() {
    let dir = tempdir().unwrap();
    write_state_for_test(dir.path(), PetStateFixture::named("mochi")).unwrap();
    let now_for_test = "2026-07-06T20:00:00Z";

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("GLORP_USAGE_NOW_FOR_TEST", now_for_test)
        .env("PATH", "/bin")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "provider today (snapshot blocked): 0",
        ))
        .stdout(predicate::str::contains("accepted recent food: 0"))
        .stdout(predicate::str::contains("pet lifetime food: 0"))
        .stdout(predicate::str::contains("effective tokens").not())
        .stdout(predicate::str::contains("provider health: blocked"))
        .stdout(predicate::str::contains("-0").not());
    let provider_day = glorp::usage::day_axis::tokenmaxxing_provider_day(
        time::macros::datetime!(2026 - 07 - 06 20:00 UTC),
    );
    let snapshot = UsageStore::open(&dir.path().join("usage.sqlite"))
        .unwrap()
        .snapshot_totals_for_provider_day(provider_day)
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Blocked
    );
    assert!(snapshot.value.is_none());
}

#[test]
fn status_uses_tokenmaxxing_day_axis_under_non_los_angeles_tz() {
    let dir = tempdir().unwrap();
    write_state_for_test(dir.path(), PetStateFixture::named("mochi")).unwrap();

    let now = OffsetDateTime::now_utc();
    let (today_start, today_end) = glorp::usage::day_axis::tokenmaxxing_today_window(now);
    let utc_today = now.date();
    let event_at = if today_start.date() == utc_today {
        today_end - Duration::seconds(1)
    } else {
        today_start + Duration::seconds(1)
    };
    assert_ne!(
        event_at.date(),
        utc_today,
        "fixture must be outside the process UTC day"
    );

    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".into(),
            observed_at: event_at,
            bucket_at: event_at,
            total_tokens: 123_456.0,
            effective_tokens: 123_456.0,
            ..NormalizedUsageEvent::for_test_at(event_at, 123_456.0)
        })
        .unwrap();
    seed_status_snapshot_for_test(
        &mut usage_store,
        glorp::usage::day_axis::tokenmaxxing_provider_day(now),
        "codex",
        123_456.0,
        event_at,
    );
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("TZ", "UTC")
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_FAILS)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "provider today (current provider snapshot): 123456",
        ))
        .stdout(predicate::str::contains("accepted recent food: 0"))
        .stdout(predicate::str::contains("pet lifetime food: 0"))
        .stdout(predicate::str::contains("codex: 123456"))
        .stdout(predicate::str::contains("effective tokens").not());
}

#[test]
fn status_persists_real_usage_delta_into_pet_state() {
    let dir = tempdir().unwrap();
    let mut state = PetState::new_for_test("fixture-seed", "mochi");
    state.stage = glorp::game::evolution::Stage::S0;
    state.calibration.daily_effective_tokens = 10_000.0;
    glorp::storage::state::StateStore::new(dir.path().join("state.json"))
        .save(&state)
        .unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: local-log-derived"))
        .stdout(predicate::str::contains("provider today"))
        .stdout(predicate::str::contains("accepted recent food"))
        .stdout(predicate::str::contains("pet lifetime food"))
        .stdout(predicate::str::contains("effective tokens").not());

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_NEXT)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: local-log-derived"))
        .stdout(predicate::str::contains("provider today"))
        .stdout(predicate::str::contains("accepted recent food"))
        .stdout(predicate::str::contains("pet lifetime food"))
        .stdout(predicate::str::contains("effective tokens").not());

    let state: PetState =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("state.json")).unwrap())
            .unwrap();
    assert!(state.lifetime_effective_tokens > 0.0);
    assert!(state.xp > 0.0);
    // The narration system now emits character-driven entries instead of
    // "gained X effective tokens". Verify that at least one narration event
    // was recorded (feast/munch/nibble/sip or a stage evolution entry).
    assert!(
        !state.recent_events.is_empty(),
        "expected at least one narrative event after token activity"
    );
}

#[test]
fn provider_failure_does_not_decay_or_overwrite_last_known_pet_state() {
    let dir = tempdir().unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.stage = glorp::game::evolution::Stage::S3;
    state.xp = 3.5;
    state.vitals.fed = 64.0;
    state.vitals.happiness = 65.0;
    state.vitals.energy = 66.0;
    glorp::storage::state::StateStore::new(dir.path().join("state.json"))
        .save(&state)
        .unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_FAILS)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("helper_exit"));

    let saved: PetState =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("state.json")).unwrap())
            .unwrap();
    assert_eq!(saved.stage, glorp::game::evolution::Stage::S3);
    assert_eq!(saved.xp, 3.5);
    assert_eq!(saved.vitals.fed, 64.0);
    assert_eq!(saved.vitals.happiness, 65.0);
    assert_eq!(saved.vitals.energy, 66.0);
}

#[test]
fn status_surfaces_first_contact_without_claiming_blocked() {
    let dir = tempdir().unwrap();
    let mut state = PetState::new_for_test("fixture-seed", "mochi");
    state.calibration.daily_effective_tokens = 10_000.0;
    glorp::storage::state::StateStore::new(dir.path().join("state.json"))
        .save(&state)
        .unwrap();
    // Deliberately no pre-seeded AgentsView cursors: cutover should seed
    // history without feeding the pet.

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .env("GLORP_USAGE_NOW_FOR_TEST", USAGE_NOW_AGENTSVIEW_FIXTURE)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: local-log-derived"))
        .stdout(predicate::str::contains("provider health: ok"))
        .stdout(predicate::str::contains("blocked").not());

    let saved: PetState =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("state.json")).unwrap())
            .unwrap();
    assert_eq!(
        saved.lifetime_effective_tokens, 0.0,
        "seeded first-contact tokens never feed"
    );
}

#[test]
fn status_lists_today_sources_generically() {
    let dir = tempdir().unwrap();
    write_state_for_test(dir.path(), PetStateFixture::named("mochi")).unwrap();

    let now = OffsetDateTime::now_utc();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider_day = glorp::usage::day_axis::tokenmaxxing_provider_day(now);
    for (surface, tokens) in [("gemini", 12_000.0), ("opencode", 8_000.0)] {
        seed_status_snapshot_for_test(&mut usage_store, provider_day, surface, tokens, now);
    }
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("sources today:"))
        .stdout(predicate::str::contains("gemini"))
        .stdout(predicate::str::contains("opencode"))
        .stdout(predicate::str::contains("claude-code").not());
}

#[test]
fn status_sources_today_use_corrected_snapshot_not_accepted_feed() {
    let dir = tempdir().unwrap();
    write_state_for_test(dir.path(), PetStateFixture::named("mochi")).unwrap();

    let now = OffsetDateTime::now_utc();
    let provider_day = glorp::usage::day_axis::tokenmaxxing_provider_day(now);
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            total_tokens: 1_060.0,
            effective_tokens: 1_060.0,
            ..NormalizedUsageEvent::for_test_at(now, 1_060.0)
        })
        .unwrap();
    seed_status_snapshot_for_test(&mut usage_store, provider_day, "claude-code", 1_060.0, now);
    seed_status_snapshot_for_test(
        &mut usage_store,
        provider_day,
        "claude-code",
        531.0,
        now + Duration::minutes(10),
    );
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("sources today:"))
        .stdout(predicate::str::contains("claude-code: 531"))
        .stdout(predicate::str::contains("claude-code: 1060").not());
}

#[test]
fn doctor_lists_discovered_sources_generically() {
    let dir = tempdir().unwrap();
    let now = OffsetDateTime::now_utc();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "gemini".to_string(),
            observed_at: now,
            bucket_at: now,
            effective_tokens: 12_000.0,
            ..NormalizedUsageEvent::for_test_at(now, 12_000.0)
        })
        .unwrap();
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("source: gemini"))
        .stdout(predicate::str::contains("provider: agentsview"))
        .stdout(predicate::str::contains("claude provider=").not());
}

#[test]
fn doctor_lists_provider_snapshot_sources_and_recent_corrections() {
    let dir = tempdir().unwrap();
    let now = OffsetDateTime::now_utc();
    let provider_day = glorp::usage::day_axis::tokenmaxxing_provider_day(now);
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            total_tokens: 1_060.0,
            effective_tokens: 1_060.0,
            ..NormalizedUsageEvent::for_test_at(now, 1_060.0)
        })
        .unwrap();
    seed_status_snapshot_for_test(&mut usage_store, provider_day, "claude-code", 1_060.0, now);
    seed_status_snapshot_for_test(
        &mut usage_store,
        provider_day,
        "claude-code",
        531.0,
        now + Duration::minutes(10),
    );
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "provider source: claude-code today=531",
        ))
        .stdout(predicate::str::contains("recent correction: claude-code"))
        .stdout(predicate::str::contains("decreased=529"));
}

fn seed_status_snapshot_for_test(
    usage: &mut UsageStore,
    day: Date,
    source: &str,
    total: f64,
    observed_at: OffsetDateTime,
) {
    let batch = ProviderSnapshotBatchInput {
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        covered_accounting_sources: Some(vec![source.into()]),
        provider_version: "test".into(),
        parser_version: "test".into(),
        observed_at,
    };
    let row = ProviderSnapshotRowInput {
        replacement_scope_id: format!("{source}:local-usage"),
        collector_scope_id: format!("{source}:local-usage"),
        collector_surface: format!("ccusage:{source}"),
        command: "test snapshot".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: source.into(),
        provider_day: day,
        model: Some("test-model".into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some(format!("hash:{source}")),
        cursor_key_hash: format!("hash:{source}:cursor"),
        cursor_update: ProviderCursorUpdate {
            provider_surface: source.into(),
            cursor_key: "cursor".into(),
            cursor_value: format!("value:{total}"),
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
fn doctor_recent_24h_uses_canonical_tokenmaxxing_totals() {
    let dir = tempdir().unwrap();
    let now = OffsetDateTime::now_utc();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "gemini".to_string(),
            observed_at: now,
            bucket_at: now,
            token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
            total_tokens: 999_999.0,
            effective_tokens: 999_999.0,
            ..NormalizedUsageEvent::for_test_at(now, 999_999.0)
        })
        .unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "gemini".to_string(),
            observed_at: now,
            bucket_at: now,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
            total_tokens: 12_000.0,
            effective_tokens: 12_000.0,
            ..NormalizedUsageEvent::for_test_at(now, 12_000.0)
        })
        .unwrap();
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("source: gemini recent_24h=12000"));
}
