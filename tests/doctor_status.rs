use assert_cmd::Command;
use glorp::storage::state::{write_state_for_test, PetState, PetStateFixture};
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn status_is_pipe_friendly_when_pet_exists() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-ok.mjs")
        .env(
            "GLORP_CCUSAGE_CODEX_BIN",
            "tests/fixtures/helpers/ccusage-codex-ok.mjs",
        )
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi"))
        .stdout(predicate::str::contains("stage progress:"))
        .stdout(predicate::str::contains("effective tokens"))
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
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("ccusage"))
        .stdout(predicate::str::contains("not found"))
        .stdout(predicate::str::contains("npm install -g glorp"));
}

#[test]
fn doctor_reports_helper_versions_when_available() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-ok.mjs")
        .env(
            "GLORP_CCUSAGE_CODEX_BIN",
            "tests/fixtures/helpers/ccusage-codex-ok.mjs",
        )
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("helpers: found"))
        .stdout(predicate::str::contains(
            "helper version: claude-code provider=ccusage 18.0.11 parser=ccusage 18.0.11",
        ))
        .stdout(predicate::str::contains(
            "helper version: codex provider=ccusage-codex 18.0.11 parser=ccusage-codex 18.0.11",
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
            "tests/fixtures/helpers/ccusage-prompts.mjs",
        )
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
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    for _ in 0..2 {
        Command::cargo_bin("glorp")
            .unwrap()
            .env("GLORP_CONFIG_DIR", dir.path())
            .env(
                "GLORP_CCUSAGE_BIN",
                "tests/fixtures/helpers/ccusage-fails.mjs",
            )
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
            .with_stage("s3")
            .with_recent_event("evolved from sprout to bytebuddy"),
    )
    .unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
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
        .env("PATH", "/bin")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "effective tokens (estimated): today 0 recent 0 lifetime 0",
        ))
        .stdout(predicate::str::contains("provider health: blocked"))
        .stdout(predicate::str::contains("-0").not());
}

#[test]
fn status_persists_real_usage_delta_into_pet_state() {
    let dir = tempdir().unwrap();
    let mut state = PetState::new_for_test("fixture-seed", "mochi");
    state.stage = "s0".into();
    state.calibration.daily_effective_tokens = 10_000.0;
    glorp::storage::state::StateStore::new(dir.path().join("state.json"))
        .save(&state)
        .unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-ok.mjs")
        .env(
            "GLORP_CCUSAGE_CODEX_BIN",
            "tests/fixtures/helpers/ccusage-codex-ok.mjs",
        )
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: local-log-derived"))
        .stdout(predicate::str::contains("effective tokens"));

    let state: PetState =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("state.json")).unwrap())
            .unwrap();
    assert!(state.lifetime_effective_tokens > 0.0);
    assert!(state.xp > 0.0);
    assert_ne!(state.stage, "s0");
    assert!(state
        .recent_events
        .iter()
        .any(|event| event.contains("effective tokens")));
}

#[test]
fn provider_failure_does_not_decay_or_overwrite_last_known_pet_state() {
    let dir = tempdir().unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.stage = "s3".into();
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
        .env(
            "GLORP_CCUSAGE_BIN",
            "tests/fixtures/helpers/ccusage-fails.mjs",
        )
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("helper_exit"));

    let saved: PetState =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("state.json")).unwrap())
            .unwrap();
    assert_eq!(saved.stage, "s3");
    assert_eq!(saved.xp, 3.5);
    assert_eq!(saved.vitals.fed, 64.0);
    assert_eq!(saved.vitals.happiness, 65.0);
    assert_eq!(saved.vitals.energy, 66.0);
}
