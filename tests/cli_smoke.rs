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
        .stdout(predicate::str::contains("feed").not());
}
