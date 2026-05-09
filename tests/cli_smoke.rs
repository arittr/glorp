use assert_cmd::Command;
use predicates::prelude::*;

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

#[test]
fn init_without_name_presents_generated_name_and_uses_it_noninteractively() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
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
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi has hatched"));

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
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

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["reset", "--yes"])
        .assert()
        .success();

    assert!(!dir.path().join("state.json").exists());
}

#[test]
fn init_with_confirmed_reinit_replaces_pet_state_without_touching_usage_db() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();
    std::fs::write(dir.path().join("usage.sqlite"), "sentinel usage db").unwrap();

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "ori-shard", "--name", "ori", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ori has hatched"));

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("ori-shard"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("usage.sqlite")).unwrap(),
        "sentinel usage db"
    );
}

#[test]
fn rename_changes_display_name_without_changing_seed() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
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
fn init_uses_historical_usage_for_calibration_without_initial_xp() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-ok.mjs")
        .env(
            "GLORP_CCUSAGE_CODEX_BIN",
            "tests/fixtures/helpers/ccusage-codex-ok.mjs",
        )
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("\"stage\": \"s0\""));
    assert!(state.contains("\"xp\": 0.0"));
    assert!(state.contains("\"calibration\""));
    assert!(state.contains("\"daily_effective_tokens\""));
}
