use glorp::storage::usage_store::UsageStore;
use glorp::usage::ccusage::{CcusageCommandProvider, HelperDiscovery, HelperPaths};
use glorp::usage::provider::UsageProvider;
use tempfile::tempdir;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/helpers")
        .join(name)
}

fn provider(claude: Option<&str>, codex: Option<&str>) -> CcusageCommandProvider {
    CcusageCommandProvider::new(HelperPaths {
        claude: claude.map(fixture),
        codex: codex.map(fixture),
        node: None,
    })
}

#[test]
fn provider_normalizes_claude_and_codex_records() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), Some("ccusage-codex-ok.mjs"));
    let result = provider.poll(&mut store).unwrap();
    assert!(result
        .deltas
        .iter()
        .any(|d| d.provider_surface == "claude-code"));
    assert!(result.deltas.iter().any(|d| d.provider_surface == "codex"));
    assert!(result
        .deltas
        .iter()
        .all(|d| d.confidence == "local-log-derived"));
    assert!(result
        .deltas
        .iter()
        .any(|d| d.model.as_deref() == Some("gpt-5.2-codex")));
    assert!(result
        .deltas
        .iter()
        .any(|d| d.model.as_deref() == Some("claude-opus-4")));
    assert!(result
        .deltas
        .iter()
        .any(|d| d.model.as_deref() == Some("claude-sonnet-4")));
}

#[test]
fn repeated_poll_does_not_double_count_unchanged_totals() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), None);
    let first = provider.poll(&mut store).unwrap();
    let second = provider.poll(&mut store).unwrap();
    assert!(first.total_effective_tokens > 0.0);
    assert_eq!(second.total_effective_tokens, 0.0);
}

#[test]
fn poll_with_increased_same_day_total_emits_only_increment() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let original_provider = provider(Some("ccusage-ok.mjs"), None);
    let first = original_provider.poll(&mut store).unwrap();
    assert!(first.total_effective_tokens > 0.0);

    let next_provider = provider(Some("ccusage-next.mjs"), None);
    let second = next_provider.poll(&mut store).unwrap();

    // 2026-05-09 increased by input 200 + output 600 + cache creation 200
    // + cache reads 10000 * 0.03.
    assert_eq!(second.total_effective_tokens, 1300.0);
    assert_eq!(second.deltas.len(), 1);
}

#[test]
fn decreasing_totals_emit_sanitized_diagnostic_without_negative_delta() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let next_provider = provider(Some("ccusage-next.mjs"), None);

    let first = next_provider.poll(&mut store).unwrap();
    assert!(first.total_effective_tokens > 0.0);

    let provider = provider(Some("ccusage-ok.mjs"), None);
    let second = provider.poll(&mut store).unwrap();
    let rendered = second
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(second.total_effective_tokens, 0.0);
    assert!(rendered.contains("cursor_total_decreased"));
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
    assert!(!rendered.contains("inputTokens"));
}

#[test]
fn missing_helpers_return_diagnostics_without_delta() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = CcusageCommandProvider::new(HelperPaths::default());

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_effective_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert_eq!(result.diagnostics.len(), 2);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.provider_surface == "claude-code" && diagnostic.code == "missing_helper"
    }));
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.provider_surface == "codex" && diagnostic.code == "missing_helper"
    }));
}

#[test]
fn missing_node_for_javascript_helper_returns_diagnostic_without_delta() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = CcusageCommandProvider::new(HelperPaths {
        claude: Some(fixture("ccusage-ok.mjs")),
        codex: None,
        node: Some(dir.path().join("missing-node")),
    });

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_effective_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.provider_surface == "claude-code" && diagnostic.code == "missing_helper"
    }));
}

#[test]
fn helper_failure_returns_diagnostic_without_delta() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-fails.mjs"), None);
    let result = provider.poll(&mut store).unwrap();
    assert_eq!(result.total_effective_tokens, 0.0);
    assert!(result.diagnostics.iter().any(|d| d.code == "helper_exit"));
}

#[test]
fn transcript_like_fields_are_ignored() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-prompts.mjs"), Some("ccusage-codex-ok.mjs"));
    let result = provider.poll(&mut store).unwrap();
    assert_eq!(result.diagnostics.len(), 0);
    let stored = store.recent_events(10).unwrap();
    let rendered = serde_json::to_string(&stored).unwrap();
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
    assert!(!rendered.contains("secret tool payload"));
}

#[test]
fn invalid_json_and_helper_stderr_are_sanitized() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let invalid = provider(Some("ccusage-invalid-json.mjs"), None)
        .poll(&mut store)
        .unwrap();
    let stderr = provider(Some("ccusage-secret-stderr.mjs"), None)
        .poll(&mut store)
        .unwrap();
    let rendered = format!("{:?}{:?}", invalid.diagnostics, stderr.diagnostics);
    assert!(rendered.contains("invalid_json"));
    assert!(rendered.contains("helper_exit"));
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
}

#[test]
fn helper_discovery_prefers_env_then_path_without_reading_real_logs() {
    let env_path = fixture("ccusage-ok.mjs");
    let path_path = fixture("ccusage-fails.mjs");
    let discovered = HelperDiscovery::from_sources(
        [("GLORP_CCUSAGE_BIN", env_path.as_path())],
        [path_path.as_path()],
    )
    .unwrap();
    assert_eq!(discovered.claude.unwrap(), env_path);
}
