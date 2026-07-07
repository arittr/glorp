use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

fn repo_path(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn read(path: &str) -> String {
    fs::read_to_string(repo_path(path)).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn read_json(path: &str) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|err| panic!("failed to parse {path}: {err}"))
}

#[test]
fn build_report_tracks_every_story_card_and_tokenpet_source() {
    let report = read("docs/superpowers/build-report.yaml");
    let story_dir = repo_path("docs/superpowers/stories");
    let mut story_ids = Vec::new();

    for entry in fs::read_dir(story_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("story-") || !name.ends_with(".md") {
            continue;
        }
        let story = fs::read_to_string(&path).unwrap();
        let id = story
            .lines()
            .find_map(|line| line.strip_prefix("id: "))
            .unwrap_or_else(|| panic!("{name} is missing an id"));
        story_ids.push(id.to_string());
    }

    story_ids.sort();
    assert_eq!(story_ids.len(), 10);
    for id in &story_ids {
        assert!(
            report.contains(id),
            "build report is missing acceptance evidence for {id}"
        );
    }

    for source in [
        "docs/tokenpet/README.md",
        "docs/tokenpet/glorp.html",
        "docs/tokenpet/chats/chat1.md",
        "docs/tokenpet/project/tokenpet.html",
        "docs/tokenpet/project/app.jsx",
        "docs/tokenpet/project/pet.jsx",
        "docs/tokenpet/project/extras.jsx",
        "docs/tokenpet/project/tweaks-panel.jsx",
    ] {
        assert!(
            repo_path(source).exists(),
            "missing TokenPet handoff source {source}"
        );
    }
}

#[test]
fn one_shot_tokenpet_mockup_remains_the_hifi_style_source() {
    let html = read("docs/tokenpet/glorp.html");
    for expected in [
        "--bg: oklch(0.18 0.005 60)",
        "--surface: oklch(0.22 0.006 60)",
        "--accent: oklch(0.78 0.14 70)",
        "--good: oklch(0.74 0.10 145)",
        ".terminal",
        ".chrome",
        ".columns",
        ".pet-stage",
        ".stat-row",
        ".today-grid",
        ".hint-strip",
    ] {
        assert!(
            html.contains(expected),
            "one-shot TokenPet mockup is missing style anchor {expected}"
        );
    }
}

#[test]
fn npm_wrapper_contract_matches_platform_package_topology() {
    let root = read_json("package.json");
    assert_eq!(
        root["workspaces"],
        serde_json::json!(["npm/glorp"]),
        "root workspace should keep the launcher package installable while platform packages stay optional"
    );

    let launcher = read_json("npm/glorp/package.json");
    assert_eq!(launcher["bin"]["glorp"], "bin/glorp.js");
    assert_eq!(launcher["dependencies"]["ccusage"], "^20.0.14");
    assert_eq!(launcher["dependencies"]["@ccusage/codex"], "^19.0.0");

    let optional = launcher["optionalDependencies"]
        .as_object()
        .expect("launcher optionalDependencies must be an object");
    let optional_names = optional.keys().cloned().collect::<BTreeSet<_>>();

    let mut platform_names = BTreeSet::new();
    for entry in fs::read_dir(repo_path("npm/platform")).unwrap() {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("package.json");
        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        let name = manifest["name"]
            .as_str()
            .unwrap_or_else(|| panic!("{} missing package name", manifest_path.display()));
        platform_names.insert(name.to_string());
        assert_eq!(
            optional.get(name).and_then(Value::as_str),
            manifest["version"].as_str(),
            "{name} version must match launcher optional dependency"
        );
        assert!(
            manifest["files"].as_array().is_some_and(|files| {
                files.iter().any(|file| {
                    file.as_str()
                        .is_some_and(|file| file.starts_with("bin/glorp"))
                })
            }),
            "{name} must package a native binary path"
        );
    }

    assert_eq!(optional_names, platform_names);
}

#[test]
fn wrapper_wires_bundled_helpers_without_disabling_path_fallback() {
    let wrapper = read("npm/glorp/bin/glorp.js");
    assert!(wrapper.contains("GLORP_CCUSAGE_BIN ??= resolvePackageBin(\"ccusage\", \"ccusage\")"));
    assert!(wrapper.contains(
        "GLORP_CCUSAGE_CODEX_BIN ??= resolvePackageBin(\"@ccusage/codex\", \"ccusage-codex\")"
    ));
    assert!(wrapper.contains("GLORP_NODE_BIN ??= process.execPath"));
    assert!(wrapper.contains("GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST"));

    let provider = read("src/usage/ccusage.rs");
    assert!(provider.contains("GLORP_CCUSAGE_BIN"));
    assert!(provider.contains("GLORP_CCUSAGE_CODEX_BIN"));
    assert!(provider.contains("find_on_path(\"ccusage\")"));
    assert!(provider.contains("find_on_path(\"ccusage-codex\")"));
}

#[test]
fn default_usage_provider_is_required_agentsview() {
    for command in [
        "src/commands/init.rs",
        "src/commands/watch.rs",
        "src/commands/status.rs",
        "src/commands/doctor.rs",
    ] {
        let source = read(command);
        assert!(
            source.contains("AgentsviewCommandProvider::from_environment"),
            "{command} should use AgentsView as the default usage provider"
        );
        assert!(
            !source.contains("CcusageCommandProvider::from_environment"),
            "{command} should not use ccusage as the default usage provider"
        );
    }

    let readme = read("README.md");
    assert!(readme.contains("The npm package bundles the native binary for your platform"));
    assert!(readme.contains("Glorp requires\nAgentsView for live usage accounting"));
    assert!(readme.contains("Glorp polls AgentsView every ten seconds"));

    let npm_readme = read("npm/glorp/README.md");
    assert!(npm_readme.contains("The npm package bundles the native binary for your platform"));
    assert!(npm_readme.contains("Glorp requires\nAgentsView for live usage accounting"));
}

#[test]
fn required_agentsview_provider_contract_remains_available() {
    let provider = read("src/usage/agentsview.rs");
    assert!(provider.contains("GLORP_AGENTSVIEW_BIN"));
    assert!(provider.contains("which::which(\"agentsview\")"));
    assert!(provider.contains("--timezone"));
    assert!(provider.contains("TOKENMAXXING_TIMEZONE"));
    assert!(read("src/usage/day_axis.rs").contains("America/Los_Angeles"));

    let readme = read("README.md");
    assert!(readme.contains("agentsview"));
    assert!(readme.contains("GLORP_AGENTSVIEW_BIN"));
    assert!(readme.contains("`cache_read_weight` is accepted for older local config files"));

    let npm_readme = read("npm/glorp/README.md");
    assert!(npm_readme.contains("agentsview"));
    assert!(npm_readme.contains("GLORP_AGENTSVIEW_BIN"));

    let legacy_note = "Legacy note: current canonical usage accounting is Tokenmaxxing-compatible `agentsview` total tokens.";
    for story in [
        "docs/superpowers/stories/story-001-usage-provider-ccusage.md",
        "docs/superpowers/stories/story-004-effective-token-model.md",
        "docs/superpowers/stories/story-010-npm-rust-packaging.md",
    ] {
        assert!(
            read(story).contains(legacy_note),
            "{story} is missing the legacy note"
        );
    }
}

#[test]
fn cli_surface_excludes_forbidden_prototype_features() {
    let cli = read("src/cli.rs");
    for forbidden in [
        "Feed",
        "Ship",
        "Treat",
        "Graveyard",
        "Revive",
        "Litter",
        "StageOverride",
        "SpeciesOverride",
        "Tweak",
    ] {
        assert!(
            !cli.contains(forbidden),
            "MVP CLI must not expose prototype-only command {forbidden}"
        );
    }

    let metabolism = read("src/game/metabolism.rs");
    assert!(!metabolism.contains("Death"));
    assert!(!metabolism.contains("Permadeath"));
}
