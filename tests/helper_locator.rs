use glorp::usage::helper_locator::{read_helper_locator, write_helper_locator, HelperLocator};

#[test]
fn helper_locator_round_trips_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("helper-locator.json");
    let locator = HelperLocator {
        ccusage_bin: Some(dir.path().join("ccusage/bin/helper.js")),
        ccusage_codex_bin: Some(dir.path().join("ccusage-codex/bin/helper.js")),
        node_bin: Some(dir.path().join("node/bin/node")),
    };

    write_helper_locator(&file, &locator).unwrap();
    let loaded = read_helper_locator(&file).unwrap().unwrap();

    assert_eq!(loaded, locator);
}

#[test]
fn missing_helper_locator_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = read_helper_locator(&dir.path().join("missing.json")).unwrap();

    assert_eq!(loaded, None);
}

#[test]
fn helper_locator_creates_parent_directory_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a/b/c/helper-locator.json");
    let locator = HelperLocator {
        ccusage_bin: Some(dir.path().join("ccusage/bin/helper.js")),
        ccusage_codex_bin: Some(dir.path().join("ccusage-codex/bin/helper.js")),
        node_bin: Some(dir.path().join("node/bin/node")),
    };

    write_helper_locator(&file, &locator).unwrap();
    assert!(file.exists());
    let loaded = read_helper_locator(&file).unwrap().unwrap();

    assert_eq!(loaded, locator);
}
