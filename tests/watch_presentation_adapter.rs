use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn watch_preview_output_stays_stable_during_adapter_migration() {
    let dir = tempdir().unwrap();
    let out = dir.path().join("preview");

    Command::cargo_bin("glorp")
        .unwrap()
        .arg("dev-preview")
        .arg("--scenario")
        .arg("watch")
        .arg("--out")
        .arg(&out)
        .env("GLORP_CONFIG_DIR", dir.path().join("config"))
        .assert()
        .success()
        .stdout(predicate::str::contains(out.display().to_string()));

    assert!(out.join("frames/watch-wide-normal.txt").is_file());
    assert!(out.join("frames/watch-wide-normal.scene.json").is_file());
    assert!(out.join("frames/watch-wide-normal.layout.json").is_file());
}
