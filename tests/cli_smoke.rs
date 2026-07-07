use assert_cmd::Command;
use predicates::prelude::*;

const AGENTSVIEW_OK: &str = "tests/fixtures/helpers/agentsview-ok.mjs";
const AGENTSVIEW_WITH_DIAGNOSTIC: &str =
    "tests/fixtures/helpers/agentsview-data-with-diagnostic.mjs";

#[test]
fn help_hides_dev_preview_command() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("dev-preview").not());
}

#[test]
fn help_lists_mvp_commands_and_no_manual_feed() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("watch"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("reset"))
        .stdout(predicate::str::contains("rename"))
        .stdout(predicate::str::contains("TUI keys"))
        .stdout(predicate::str::contains("q"))
        .stdout(predicate::str::contains("r"))
        .stdout(predicate::str::contains("?"))
        .stdout(predicate::str::contains("feed").not());
}

#[cfg(feature = "dev-preview")]
#[test]
fn watch_accepts_hidden_dev_pet_selector() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    let usage = if cfg!(windows) {
        "Usage: glorp.exe watch"
    } else {
        "Usage: glorp watch"
    };
    cmd.args(["watch", "--pet", "ghost", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(usage))
        .stdout(predicate::str::contains("--pet").not());
}

#[test]
fn init_without_name_presents_generated_name_and_uses_it_noninteractively() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "mochi-7f3a"])
        .assert()
        .success()
        .stdout(predicate::str::contains("generated name: cog-92"))
        .stdout(predicate::str::contains("cog-92 has hatched"));

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("\"accepted_name\": \"cog-92\""));
}

#[test]
fn init_creates_state_and_blocks_second_init() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi has hatched"));

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "other"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already has a pet"));
}

#[test]
fn reset_requires_confirmation_and_removes_pet_state() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .arg("reset")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));

    // Plant a usage.sqlite to confirm reset clears it too.
    std::fs::write(dir.path().join("usage.sqlite"), "sentinel usage db").unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["reset", "--yes"])
        .assert()
        .success();

    assert!(!dir.path().join("state.json").exists());
    assert!(!dir.path().join("usage.sqlite").exists());
}

#[test]
fn init_with_confirmed_reinit_replaces_pet_state_and_resets_usage_db() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();
    std::fs::write(dir.path().join("usage.sqlite"), "sentinel usage db").unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "ori-shard", "--name", "ori", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ori has hatched"));

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("ori-shard"));

    let usage_store =
        glorp::storage::usage_store::UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    assert_eq!(usage_store.recent_event_count().unwrap(), 0);
}

#[test]
fn rename_changes_display_name_without_changing_seed() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_AGENTSVIEW_BIN")
        .env("PATH", "/bin")
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["rename", "sprig"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sprig"));

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("mochi-7f3a"));
    assert!(state.contains("sprig"));
    assert!(!state.contains("\"accepted_name\": \"mochi\""));
}

#[test]
fn help_lists_companion_but_hides_companion_app() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("companion"))
        .stdout(predicate::str::contains("companion-app").not());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn companion_reports_macos_only_on_other_platforms() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("companion")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "glorp companion is only available on macOS",
        ));
}

#[test]
fn init_uses_historical_usage_for_calibration_without_initial_xp() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("\"stage\": \"s0\""));
    assert!(state.contains("\"xp\": 0.0"));
    assert!(state.contains("\"calibration\""));
    assert!(state.contains("\"daily_effective_tokens\""));
}

#[test]
fn init_does_not_feed_pet_or_persist_history_as_usage_events() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_OK)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    let state_json = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(
        state_json.contains(r#""xp": 0.0"#) || state_json.contains(r#""xp":0.0"#),
        "expected xp to be zero after init, got: {state_json}"
    );
    assert!(
        state_json.contains(r#""lifetime_effective_tokens": 0.0"#)
            || state_json.contains(r#""lifetime_effective_tokens":0.0"#),
        "expected lifetime_effective_tokens to be zero after init, got: {state_json}"
    );

    let usage_store =
        glorp::storage::usage_store::UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    assert_eq!(usage_store.recent_event_count().unwrap(), 0);
}

#[test]
fn init_seeds_source_contact_and_highwaters_without_feeding_pet() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", CCUSAGE_OK)
        .env("GLORP_CCUSAGE_CODEX_BIN", CCUSAGE_CODEX_EMPTY)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    let state_json = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(
        state_json.contains(r#""xp": 0.0"#) || state_json.contains(r#""xp":0.0"#),
        "expected xp to be zero after init, got: {state_json}"
    );

    let usage_store =
        glorp::storage::usage_store::UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    assert_eq!(usage_store.recent_event_count().unwrap(), 0);
    assert!(usage_store
        .source_has_feed_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
        )
        .unwrap());
    assert_eq!(
        usage_store
            .source_day_highwater_for_test(
                glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                "claude-code",
                time::macros::date!(2026 - 05 - 08),
            )
            .unwrap(),
        43_300.0
    );
    assert_eq!(
        usage_store
            .source_day_highwater_for_test(
                glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                "claude-code",
                time::macros::date!(2026 - 05 - 09),
            )
            .unwrap(),
        84_500.0
    );
}

// Regression test: init must advance cursors to current totals even when the
// calibration snapshot has benign diagnostics alongside valid records.
//
// Before the fix: any diagnostic caused init to skip advance_cursors → cursors
// stayed at zero. This test verifies directly that provider_cursors rows exist
// in usage.sqlite after init completes with a diagnostic fixture.
//
// The bolus scenario: if cursors are NOT advanced, the next poll (e.g. watch
// or status on a day where the diagnostic clears and cutover fires) would diff
// the full history against zero cursors and apply it as pet food → S6.
#[test]
fn init_with_diagnostic_snapshot_advances_cursors_in_usage_db() {
    let dir = tempfile::tempdir().unwrap();

    // Init with an AgentsView fixture that has valid data + a benign diagnostic.
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_AGENTSVIEW_BIN", AGENTSVIEW_WITH_DIAGNOSTIC)
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success()
        .stdout(predicates::str::contains("mochi has hatched"));

    // The fix: init must advance cursors even when diagnostics are present.
    // If cursors are not advanced, the usage.sqlite will have no provider_cursors
    // rows, and the next poll will see the full history as a bolus.
    let usage_store =
        glorp::storage::usage_store::UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let versions = usage_store.provider_versions().unwrap();
    assert!(
        !versions.is_empty(),
        "init must advance provider cursors into usage.sqlite even when the \
        calibration snapshot has diagnostics; found no cursor rows. \
        Without this, the next clean poll applies full history as a bolus."
    );
}
