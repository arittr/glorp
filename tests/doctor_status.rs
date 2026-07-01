use assert_cmd::Command;
use glorp::storage::state::{write_state_for_test, PetState, PetStateFixture};
use glorp::storage::usage_store::{NormalizedUsageEvent, UsageStore};
use predicates::prelude::*;
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

const CCUSAGE_OK: &str = "tests/fixtures/helpers/ccusage-v20-multiagent.mjs";
const CCUSAGE_CODEX_OK: &str = "tests/fixtures/helpers/ccusage-codex-ok.mjs";
const CCUSAGE_CODEX_EMPTY: &str = "tests/fixtures/helpers/ccusage-codex-empty.mjs";
const CCUSAGE_FAILS: &str = "tests/fixtures/helpers/ccusage-fails.mjs";
const CCUSAGE_LEGACY_OK: &str = "tests/fixtures/helpers/ccusage-ok.mjs";
const CCUSAGE_LEGACY_NEXT: &str = "tests/fixtures/helpers/ccusage-next.mjs";

#[test]
fn status_is_pipe_friendly_when_pet_exists() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_OK)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_OK)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi"))
        .stdout(predicate::str::contains("stage progress:"))
        .stdout(predicate::str::contains("tokens"))
        .stdout(predicate::str::contains("effective tokens").not())
        .stdout(predicate::str::contains("local-log-derived"))
        .stdout(predicate::str::contains("provider health:"))
        .stdout(predicate::str::contains("billing").not());
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
        .stdout(predicate::str::contains("provider: ccusage"))
        .stdout(predicate::str::contains("bundled usage helpers: blocked"))
        .stdout(predicate::str::contains("Default provider blocked."))
        .stdout(predicate::str::contains("GLORP_CCUSAGE_BIN"));
}

#[test]
fn doctor_reports_ccusage_provider_as_bundled_default() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_OK)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: ccusage"))
        .stdout(predicate::str::contains("bundled usage helpers: yes"))
        .stdout(predicate::str::contains("ccusage 20.0.6"));
}

#[test]
fn doctor_reports_missing_ccusage_as_default_provider_blocked() {
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
        .stdout(predicate::str::contains("bundled usage helpers: blocked"))
        .stdout(predicate::str::contains("Default provider blocked."))
        .stdout(predicate::str::contains("ccusage helper was not found"))
        .stdout(predicate::str::contains("GLORP_CCUSAGE_BIN"));
}

#[test]
fn doctor_sanitizes_ccusage_helper_stderr() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env(
            "GLORP_CCUSAGE_BIN",
            "tests/fixtures/helpers/ccusage-secret-stderr.mjs",
        )
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_FAILS)
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
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_OK)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("helpers: found"))
        .stdout(predicate::str::contains("provider command health: ok"))
        .stdout(predicate::str::contains("provider: ccusage"))
        .stdout(predicate::str::contains(
            "helper version: claude-code provider=ccusage 20.0.6 parser=ccusage 20.0.6",
        ));
}

#[test]
fn diagnostics_do_not_print_raw_transcript_content() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env(
            "GLORP_CCUSAGE_BIN",
            "tests/fixtures/helpers/ccusage-secret-stderr.mjs",
        )
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_FAILS)
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
        .env(
            "GLORP_CCUSAGE_BIN",
            "tests/fixtures/helpers/ccusage-invalid-json.mjs",
        )
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_FAILS)
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid_json"))
        .stdout(predicate::str::contains("secret prompt").not());

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env(
            "GLORP_CCUSAGE_BIN",
            "tests/fixtures/helpers/ccusage-secret-stderr.mjs",
        )
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_FAILS)
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
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_OK)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    for _ in 0..2 {
        Command::cargo_bin("glorp")
            .unwrap()
            .env("GLORP_CONFIG_DIR", dir.path())
            .env("GLORP_CCUSAGE_BIN", CCUSAGE_FAILS)
            .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_FAILS)
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
        .stdout(predicate::str::contains(
            "tokens (estimated): today 0 recent 0 lifetime 0",
        ))
        .stdout(predicate::str::contains("effective tokens").not())
        .stdout(predicate::str::contains("provider health: blocked"))
        .stdout(predicate::str::contains("-0").not());
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
    drop(usage_store);

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("TZ", "UTC")
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_FAILS)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_FAILS)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "tokens (estimated): today 123456 recent 0 lifetime 0",
        ))
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
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_LEGACY_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_EMPTY)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: local-log-derived"))
        .stdout(predicate::str::contains("tokens"))
        .stdout(predicate::str::contains("effective tokens").not());

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_LEGACY_NEXT)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_EMPTY)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: local-log-derived"))
        .stdout(predicate::str::contains("tokens"))
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
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_FAILS)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_FAILS)
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
    // Deliberately no pre-seeded ccusage cursors: cutover should seed
    // history without feeding the pet.

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_OK)
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
    for (surface, tokens) in [("gemini", 12_000.0), ("opencode", 8_000.0)] {
        usage_store
            .insert_event(&NormalizedUsageEvent {
                provider_surface: surface.to_string(),
                observed_at: now,
                bucket_at: now,
                effective_tokens: tokens,
                ..NormalizedUsageEvent::for_test_at(now, tokens)
            })
            .unwrap();
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
        .stdout(predicate::str::contains("provider: ccusage"))
        .stdout(predicate::str::contains("claude-code provider=").not());
}
