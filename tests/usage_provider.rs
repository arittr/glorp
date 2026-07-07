use glorp::game::effective_tokens::EffectiveTokenWeights;
use glorp::game::runtime::apply_usage_poll;
use glorp::storage::state::PetState;
use glorp::storage::usage_store::{ProviderCursorUpdate, UsageStore};
use glorp::usage::ccusage::{CcusageCommandProvider, HelperDiscovery, HelperPaths};
use glorp::usage::identity::SourceFamily;
use glorp::usage::normalize::RawTokenTotals;
use glorp::usage::provider::{ProviderCursorKey, UsagePollResult, UsageProvider};
use glorp::usage::snapshot::{ProviderSnapshotBatchInput, ProviderSnapshotRowInput};
use serde::Serialize;
use serde_json::Value;
use tempfile::tempdir;
use time::{
    macros::{date, datetime},
    OffsetDateTime,
};

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/helpers")
        .join(name)
}

fn agentsview_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/helpers")
        .join(name)
}

fn fixture_json(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn provider(claude: Option<&str>, codex: Option<&str>) -> CcusageCommandProvider {
    provider_at(claude, codex, datetime!(2026 - 05 - 09 12:00 UTC))
}

fn provider_at(
    claude: Option<&str>,
    codex: Option<&str>,
    now: OffsetDateTime,
) -> CcusageCommandProvider {
    CcusageCommandProvider::new_with_now_for_test(
        HelperPaths {
            unified: None,
            claude: claude.map(fixture),
            codex: codex.map(fixture),
            node: None,
        },
        now,
    )
}

fn unified_provider(name: &str) -> CcusageCommandProvider {
    CcusageCommandProvider::new_with_now_for_test(
        HelperPaths {
            unified: Some(fixture(name)),
            claude: None,
            codex: None,
            node: None,
        },
        datetime!(2026 - 06 - 11 12:00 UTC),
    )
}

fn provider_with_unified_at(
    unified: Option<&str>,
    claude: Option<&str>,
    codex: Option<&str>,
    now: OffsetDateTime,
) -> CcusageCommandProvider {
    CcusageCommandProvider::new_with_now_for_test(
        HelperPaths {
            unified: unified.map(fixture),
            claude: claude.map(fixture),
            codex: codex.map(fixture),
            node: None,
        },
        now,
    )
}

fn agentsview_provider(name: &str) -> glorp::usage::agentsview::AgentsviewCommandProvider {
    glorp::usage::agentsview::AgentsviewCommandProvider::new_with_now_for_test(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(agentsview_fixture(name)),
        },
        datetime!(2026 - 06 - 18 20:00 UTC),
    )
}

// Run a full poll/stage/apply/mark lifecycle so the provider cursor advances,
// matching what `glorp poll` does in production. Tests that issue back-to-back
// `provider.poll` calls without applying would otherwise see the same totals
// re-emitted because the cursor only advances after pet state is saved.
fn complete_poll_lifecycle(
    provider: &CcusageCommandProvider,
    store: &mut UsageStore,
) -> UsagePollResult {
    record_common_fixture_sources(store);
    let result = provider.poll(store).unwrap();
    let mut state = PetState::new_for_test("test-seed", "test");
    state.calibration.daily_effective_tokens = 100_000.0;
    apply_usage_poll(&mut state, store, &result, OffsetDateTime::now_utc()).unwrap();
    result
}

fn cursor_key_values(
    updates: &[ProviderCursorUpdate],
) -> std::collections::BTreeSet<(String, String)> {
    updates
        .iter()
        .map(|update| (update.provider_surface.clone(), update.cursor_key.clone()))
        .collect()
}

fn record_known_sources(store: &mut UsageStore, sources: &[&str]) {
    for source in sources {
        store
            .record_source_contact(
                glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                source,
                glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
                OffsetDateTime::now_utc(),
            )
            .unwrap();
    }
}

fn record_common_fixture_sources(store: &mut UsageStore) {
    record_known_sources(
        store,
        &[
            "claude-code",
            "codex",
            "gemini",
            "kimi",
            "opencode",
            "claude",
        ],
    );
}

fn seed_source_snapshot(
    store: &mut UsageStore,
    provider_day: time::Date,
    accounting_source: &str,
    total_tokens: f64,
) {
    let observed_at = OffsetDateTime::now_utc();
    let cursor_update = ProviderCursorUpdate {
        provider_surface: accounting_source.to_string(),
        cursor_key: format!("test:{accounting_source}:{provider_day}"),
        cursor_value: "{}".to_string(),
        provider_version: "test".to_string(),
        parser_version: "test".to_string(),
    };
    let row = ProviderSnapshotRowInput {
        replacement_scope_id: format!("{accounting_source}:local-usage"),
        collector_scope_id: "test:seed".to_string(),
        collector_surface: "test:seed".to_string(),
        command: "test".to_string(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
        accounting_source: accounting_source.to_string(),
        provider_day,
        model: Some("seed-model".to_string()),
        source_surface: "daily".to_string(),
        provider_period: provider_day.to_string(),
        raw_source_id_hash: None,
        cursor_key_hash: format!("test-hash:{accounting_source}:{provider_day}"),
        cursor_update,
        raw_token_buckets: None,
        total_tokens,
        cost_usd: None,
        confidence: "test".to_string(),
    };
    let batch = ProviderSnapshotBatchInput {
        collector_scope_id: "test:seed".to_string(),
        collector_surface: "test:seed".to_string(),
        command: "test".to_string(),
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
        requested_provider_days: vec![provider_day],
        covered_accounting_sources: Some(vec![accounting_source.to_string()]),
        provider_version: "test".to_string(),
        parser_version: "test".to_string(),
        observed_at,
    };
    store
        .write_provider_snapshot_batch(&batch, &[row], &[])
        .unwrap();
}

#[test]
fn provider_normalizes_claude_and_codex_records() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude-code", "codex"]);
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
    assert!(result.deltas.iter().all(|d| {
        d.token_contract == glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1
            && d.effective_tokens == d.total_tokens
    }));
}

#[test]
fn provider_deltas_carry_raw_token_bucket_detail() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude-code"]);
    let provider = provider(Some("ccusage-ok.mjs"), None);

    let poll = provider.poll(&mut store).unwrap();
    let claude_delta = poll
        .deltas
        .iter()
        .find(|delta| delta.provider_surface == "claude-code")
        .expect("expected claude delta");
    let buckets = claude_delta
        .token_totals
        .expect("provider delta should include raw token bucket detail");

    assert!(
        buckets.uncached_input > 0 || buckets.output > 0 || buckets.cache_creation > 0,
        "expected non-empty bucket detail: {buckets:?}"
    );
}

#[test]
fn repeated_poll_does_not_double_count_unchanged_totals() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), None);
    let first = complete_poll_lifecycle(&provider, &mut store);
    let second = complete_poll_lifecycle(&provider, &mut store);
    assert!(first.total_effective_tokens > 0.0);
    assert_eq!(second.total_effective_tokens, 0.0);
}

#[test]
fn poll_with_increased_same_day_total_emits_only_increment() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let original_provider = provider(Some("ccusage-ok.mjs"), None);
    let first = complete_poll_lifecycle(&original_provider, &mut store);
    assert!(first.total_effective_tokens > 0.0);

    let next_provider = provider(Some("ccusage-next.mjs"), None);
    let second = next_provider.poll(&mut store).unwrap();

    // 2026-05-09 increased by input 200 + output 600 + cache creation 200
    // + cache reads 10000. Cached input now counts fully for canonical food.
    assert_eq!(second.total_effective_tokens, 11_000.0);
    assert_eq!(second.deltas.len(), 1);
}

#[test]
fn decreasing_totals_emit_sanitized_diagnostic_without_negative_delta() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let next_provider = provider(Some("ccusage-next.mjs"), None);

    let first = complete_poll_lifecycle(&next_provider, &mut store);
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
    assert!(second.deltas.is_empty());
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 05 - 09))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(snapshot.value.unwrap().total_tokens, 84_500.0);
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
    assert_eq!(result.diagnostics.len(), 3);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.provider_surface == "unified" && diagnostic.code == "missing_helper"
    }));
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
        unified: None,
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
    record_known_sources(&mut store, &["claude-code", "codex"]);
    let provider = provider(Some("ccusage-prompts.mjs"), Some("ccusage-codex-ok.mjs"));
    let result = provider.poll(&mut store).unwrap();
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code == "missing_helper"),
        "expected only optional-helper-missing diagnostics, got {:?}",
        result.diagnostics
    );

    let mut state = PetState::new_for_test("test-seed", "test");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = OffsetDateTime::now_utc();
    store
        .advance_cursors(
            result
                .deltas
                .iter()
                .map(|delta| ProviderCursorUpdate {
                    provider_surface: delta.cursor_update.provider_surface.clone(),
                    cursor_key: delta.cursor_update.cursor_key.clone(),
                    cursor_value: "seeded".to_string(),
                    provider_version: "test-provider".to_string(),
                    parser_version: "test-parser".to_string(),
                })
                .collect(),
            now,
        )
        .unwrap();
    apply_usage_poll(&mut state, &mut store, &result, now).unwrap();

    let stored = store.recent_events(50).unwrap();
    assert!(
        !stored.is_empty(),
        "stored events should not be empty after apply"
    );
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

#[test]
fn provider_ignores_legacy_cache_read_weight_for_canonical_deltas() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = CcusageCommandProvider::new_with_now_for_test(
        HelperPaths {
            unified: None,
            claude: Some(fixture("ccusage-ok.mjs")),
            codex: None,
            node: None,
        },
        datetime!(2026 - 05 - 09 12:00 UTC),
    )
    .with_weights(EffectiveTokenWeights { cache_read_weight: 0.05 });

    complete_poll_lifecycle(&provider, &mut store);
    let next_provider = CcusageCommandProvider::new_with_now_for_test(
        HelperPaths {
            unified: None,
            claude: Some(fixture("ccusage-next.mjs")),
            codex: None,
            node: None,
        },
        datetime!(2026 - 05 - 09 12:00 UTC),
    )
    .with_weights(EffectiveTokenWeights { cache_read_weight: 0.05 });

    let second = next_provider.poll(&mut store).unwrap();
    assert_eq!(second.total_effective_tokens, 11_000.0);
    assert!(second.deltas.iter().all(|delta| {
        delta.token_contract == glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1
            && delta.effective_tokens == delta.total_tokens
    }));
}

#[test]
fn snapshot_and_poll_serialize_byte_identical_cursor_keys() {
    let dir = tempdir().unwrap();
    let mut snapshot_store = UsageStore::open(&dir.path().join("snapshot.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), Some("ccusage-codex-ok.mjs"));

    let snapshot = provider
        .snapshot_for_calibration(&mut snapshot_store)
        .unwrap();
    let snapshot_keys = cursor_key_values(
        &snapshot
            .cursor_updates
            .into_iter()
            .filter(|update| {
                serde_json::from_str::<ProviderCursorKey>(&update.cursor_key)
                    .map(|key| key.period_start == "2026-05-09")
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>(),
    );

    let mut poll_store = UsageStore::open(&dir.path().join("poll.sqlite")).unwrap();
    record_known_sources(&mut poll_store, &["claude-code", "codex"]);
    let poll = provider.poll(&mut poll_store).unwrap();
    let poll_keys = poll
        .deltas
        .iter()
        .map(|delta| {
            (
                delta.cursor_update.provider_surface.clone(),
                delta.cursor_update.cursor_key.clone(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(snapshot_keys, poll_keys);
    for (_, key) in snapshot_keys {
        let parsed: ProviderCursorKey = serde_json::from_str(&key).unwrap();
        assert_eq!(
            parsed.token_contract.as_deref(),
            Some(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1)
        );
    }
}

#[test]
fn helper_version_change_does_not_create_new_food_for_same_totals() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let first = complete_poll_lifecycle(&provider(Some("ccusage-ok.mjs"), None), &mut store);
    assert!(first.total_effective_tokens > 0.0);

    let second = provider(Some("ccusage-ok-v2.mjs"), None)
        .poll(&mut store)
        .unwrap();
    assert_eq!(second.total_effective_tokens, 0.0);
}

#[test]
fn legacy_cursor_with_parser_version_migrates_without_double_feeding() {
    #[derive(Serialize)]
    struct LegacyKey {
        provider_surface: String,
        command: String,
        parser_version: String,
        period_start: String,
        model: Option<String>,
    }

    fn legacy_key_json(period_start: &str, model: Option<&str>) -> String {
        serde_json::to_string(&LegacyKey {
            provider_surface: "claude-code".to_string(),
            command: "ccusage".to_string(),
            parser_version: "ccusage 18.0.11".to_string(),
            period_start: period_start.to_string(),
            model: model.map(str::to_string),
        })
        .unwrap()
    }

    fn new_key_json(period_start: &str, model: Option<&str>) -> String {
        serde_json::to_string(&ProviderCursorKey {
            provider_surface: "claude-code".to_string(),
            token_contract: Some(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string()),
            command: "ccusage".to_string(),
            source_surface: "daily".to_string(),
            period_start: period_start.to_string(),
            model: model.map(str::to_string),
            raw_source_id: None,
        })
        .unwrap()
    }

    fn totals_json(
        uncached_input: u64,
        output: u64,
        cache_creation: u64,
        cache_read: u64,
    ) -> String {
        serde_json::to_string(&RawTokenTotals {
            uncached_input,
            output,
            cache_creation,
            cache_read,
            reasoning_output: 0,
        })
        .unwrap()
    }

    // Totals match the (date, model) records emitted by ccusage-ok.mjs / ccusage-daily.json.
    let seeded = [
        (
            "2026-05-08",
            Some("claude-sonnet-4"),
            totals_json(1000, 2000, 300, 40000),
        ),
        (
            "2026-05-09",
            Some("claude-opus-4"),
            totals_json(1000, 1500, 300, 50000),
        ),
        (
            "2026-05-09",
            Some("claude-sonnet-4"),
            totals_json(500, 1000, 200, 30000),
        ),
    ];

    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();

    for (period_start, model, value) in &seeded {
        store
            .set_provider_cursor(
                "claude-code",
                &legacy_key_json(period_start, *model),
                value,
                "ccusage 18.0.11",
                "ccusage 18.0.11",
            )
            .unwrap();
    }

    let provider = provider(Some("ccusage-ok.mjs"), None);
    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_effective_tokens, 0.0);

    for (period_start, model, value) in &seeded {
        if *period_start != "2026-05-09" {
            continue;
        }
        let migrated = store
            .provider_cursor("claude-code", &new_key_json(period_start, *model))
            .unwrap();
        assert_eq!(migrated.as_deref(), Some(value.as_str()));
    }

    let second = provider.poll(&mut store).unwrap();
    assert_eq!(second.total_effective_tokens, 0.0);
}

#[test]
fn snapshot_for_calibration_returns_daily_usage_without_inserting_events() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), Some("ccusage-codex-ok.mjs"));

    let snapshot = provider.snapshot_for_calibration(&mut store).unwrap();

    assert!(!snapshot.daily_usage.is_empty());
    assert!(snapshot
        .daily_usage
        .iter()
        .all(|day| day.effective_tokens > 0.0));
    assert!(!snapshot.cursor_updates.is_empty());
    assert!(snapshot
        .cursor_updates
        .iter()
        .any(|update| update.provider_surface == "claude-code"));
    assert!(snapshot
        .cursor_updates
        .iter()
        .any(|update| update.provider_surface == "codex"));

    assert_eq!(store.recent_event_count().unwrap(), 0);
    assert_eq!(store.lifetime_effective_tokens().unwrap(), 0.0);
    assert_eq!(store.unapplied_events(50).unwrap().len(), 0);

    for update in &snapshot.cursor_updates {
        let pre_advance = store
            .provider_cursor(&update.provider_surface, &update.cursor_key)
            .unwrap();
        assert!(pre_advance.is_none());
    }

    store
        .advance_cursors(snapshot.cursor_updates.clone(), OffsetDateTime::now_utc())
        .unwrap();

    for update in &snapshot.cursor_updates {
        let post_advance = store
            .provider_cursor(&update.provider_surface, &update.cursor_key)
            .unwrap();
        assert_eq!(post_advance.as_deref(), Some(update.cursor_value.as_str()));
    }

    let after_calibration_poll = provider.poll(&mut store).unwrap();
    assert_eq!(after_calibration_poll.total_effective_tokens, 0.0);
}

#[test]
fn ccusage_v20_uses_the_claude_scoped_subcommand() {
    // ccusage >= 20 turned bare `daily` into an all-agents aggregator
    // (gpt/gemini usage included, `date` renamed to `period`); the provider
    // must invoke `claude daily` there, or months of non-claude usage appear
    // as new cursor keys and feed the pet (observed live 2026-06-10).
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude-code"]);
    let provider = provider_at(
        Some("ccusage-v20-multiagent.mjs"),
        Some("ccusage-codex-ok.mjs"),
        datetime!(2026 - 06 - 10 12:00 UTC),
    );
    let result = provider.poll(&mut store).unwrap();
    let claude: Vec<_> = result
        .deltas
        .iter()
        .filter(|d| d.provider_surface == "claude-code")
        .collect();
    assert!(!claude.is_empty(), "scoped subcommand must yield deltas");
    assert!(
        claude
            .iter()
            .all(|d| d.model.as_deref() == Some("claude-fable-5")),
        "non-claude agent rows must never reach the ledger: {claude:?}"
    );
    let total: f64 = claude.iter().map(|d| d.effective_tokens).sum();
    // 100 + 200 + 0 + 1000 = 1300 from the claude-scoped payload; the
    // all-agents payload would be ~3M.
    assert!((total - 1300.0).abs() < 1.0, "got {total}");
}

#[test]
fn unified_multi_source_emits_deltas_after_seeded_cursors() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = unified_provider("ccusage-unified-multi.mjs");

    let snapshot = provider.snapshot_for_calibration(&mut store).unwrap();
    assert_eq!(snapshot.cursor_updates.len(), 6);
    let unique_keys: std::collections::HashSet<_> = snapshot
        .cursor_updates
        .iter()
        .map(|u| u.cursor_key.clone())
        .collect();
    assert_eq!(unique_keys.len(), snapshot.cursor_updates.len());

    let now = OffsetDateTime::now_utc();
    store.advance_cursors(snapshot.cursor_updates, now).unwrap();

    let next = unified_provider("ccusage-unified-multi-next.mjs");
    let result = next.poll(&mut store).unwrap();
    let surfaces: std::collections::HashSet<_> = result
        .deltas
        .iter()
        .map(|d| d.provider_surface.as_str())
        .collect();

    assert!(surfaces.contains("claude-code"));
    assert!(surfaces.contains("codex"));
    assert!(surfaces.contains("gemini"));
    assert!(surfaces.contains("kimi"));
    assert!(surfaces.contains("opencode"));
    assert!(!surfaces.contains("all"));
    assert!(!surfaces.contains("unknown-bad"));

    assert!(result.deltas.iter().all(|d| {
        d.source_identity.provider_surface == d.provider_surface
            && d.cursor_update.provider_surface == d.provider_surface
    }));
    assert!(result.deltas.iter().any(|d| {
        d.provider_surface == "opencode"
            && d.source_identity.source_family == SourceFamily::UnknownCodingAgent
    }));
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "aggregate_all_source_ignored"));
}

#[test]
fn unified_first_contact_seeds_cursors_without_feeding() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = unified_provider("ccusage-unified-multi.mjs");

    let snapshot = provider.snapshot_for_calibration(&mut store).unwrap();
    assert!(!snapshot.cursor_updates.is_empty());
    let now = OffsetDateTime::now_utc();
    store.advance_cursors(snapshot.cursor_updates, now).unwrap();

    let result = provider.poll(&mut store).unwrap();
    assert_eq!(result.total_effective_tokens, 0.0);
    assert!(store
        .latest_cursor_updated_at("claude-code")
        .unwrap()
        .is_some());
    assert!(store.latest_cursor_updated_at("codex").unwrap().is_some());
    assert!(store.latest_cursor_updated_at("gemini").unwrap().is_some());
}

#[test]
fn repeated_unified_poll_after_cursor_advance_emits_zero_deltas() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = unified_provider("ccusage-unified-multi.mjs");
    complete_poll_lifecycle(&provider, &mut store);
    let second = provider.poll(&mut store).unwrap();
    assert_eq!(second.total_effective_tokens, 0.0);
}

#[test]
fn unified_aggregate_model_breakdowns_without_source_do_not_feed() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider_with_unified_at(
        Some("ccusage-unified-aggregate-no-source.mjs"),
        None,
        None,
        datetime!(2026 - 07 - 03 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_effective_tokens, 0.0);
    assert!(
        result.deltas.is_empty(),
        "unidentified unified aggregate rows must not become a billable source: {:?}",
        result.deltas
    );
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.provider_surface == "unified"
            && diagnostic.code == "aggregate_unidentified_source_ignored"
    }));
}

#[test]
fn unusable_unified_rows_fall_back_without_writing_zero_snapshot() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_with_unified_at(
        Some("ccusage-unified-aggregate-requested.mjs"),
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert!(result.total_tokens > 0.0);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.provider_surface == "unified"
            && diagnostic.code == "aggregate_unidentified_source_ignored"
    }));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 05 - 09))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert!(snapshot.value.unwrap().total_tokens > 0.0);
}

#[test]
fn tokenmaxxing_comparison_fixture_preserves_captured_public_totals() {
    let comparison: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_json("agentsview-drew-2026-06-18-tokenmaxxing.json"))
            .unwrap(),
    )
    .unwrap();

    assert_eq!(comparison["date"], "2026-06-18");
    assert_eq!(
        comparison["captured_from"],
        "https://tokenmaxxing.odio.dev/api/user/drew"
    );
    assert_eq!(
        comparison["sources"]["claude"].as_u64().unwrap(),
        46_011_892
    );
    assert_eq!(
        comparison["sources"]["codex"].as_u64().unwrap(),
        669_369_020
    );
    assert_eq!(comparison["total"].as_u64().unwrap(), 715_380_912);

    let source_labels = comparison["sources"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        source_labels,
        std::collections::HashSet::from(["claude", "codex"])
    );
}

#[test]
fn provider_writes_snapshot_before_emitting_feed_deltas() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert!(result.total_tokens > 0.0);
    let today = time::Date::from_calendar_date(2026, time::Month::May, 9).unwrap();
    let snapshot = store.snapshot_totals_for_provider_day(today).unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert!(snapshot.value.unwrap().total_tokens > 0.0);
}

#[test]
fn snapshot_only_refresh_does_not_seed_cursor_before_feed_poll() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    provider.refresh_snapshots_only(&mut store).unwrap();
    let result = provider.poll(&mut store).unwrap();

    assert!(result.total_tokens > 0.0);
}

#[test]
fn snapshot_only_refresh_does_not_migrate_legacy_cursor_before_feed_poll() {
    #[derive(Serialize)]
    struct LegacyKey {
        provider_surface: String,
        command: String,
        parser_version: String,
        period_start: String,
        model: Option<String>,
    }

    fn legacy_key_json(period_start: &str, model: Option<&str>) -> String {
        serde_json::to_string(&LegacyKey {
            provider_surface: "claude-code".to_string(),
            command: "ccusage".to_string(),
            parser_version: "ccusage 18.0.11".to_string(),
            period_start: period_start.to_string(),
            model: model.map(str::to_string),
        })
        .unwrap()
    }

    fn new_key_json(period_start: &str, model: Option<&str>) -> String {
        serde_json::to_string(&ProviderCursorKey {
            provider_surface: "claude-code".to_string(),
            token_contract: Some(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string()),
            command: "ccusage".to_string(),
            source_surface: "daily".to_string(),
            period_start: period_start.to_string(),
            model: model.map(str::to_string),
            raw_source_id: None,
        })
        .unwrap()
    }

    fn totals_json(
        uncached_input: u64,
        output: u64,
        cache_creation: u64,
        cache_read: u64,
    ) -> String {
        serde_json::to_string(&RawTokenTotals {
            uncached_input,
            output,
            cache_creation,
            cache_read,
            reasoning_output: 0,
        })
        .unwrap()
    }

    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let seeded = [
        (
            "2026-05-09",
            Some("claude-opus-4"),
            totals_json(1000, 1500, 300, 50000),
        ),
        (
            "2026-05-09",
            Some("claude-sonnet-4"),
            totals_json(500, 1000, 200, 30000),
        ),
    ];
    for (period_start, model, value) in &seeded {
        store
            .set_provider_cursor(
                "claude-code",
                &legacy_key_json(period_start, *model),
                value,
                "ccusage 18.0.11",
                "ccusage 18.0.11",
            )
            .unwrap();
    }
    let provider = provider_at(
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    provider.refresh_snapshots_only(&mut store).unwrap();

    for (period_start, model, _) in &seeded {
        let migrated = store
            .provider_cursor("claude-code", &new_key_json(period_start, *model))
            .unwrap();
        assert_eq!(migrated, None);
    }

    let result = provider.poll(&mut store).unwrap();
    assert_eq!(result.total_effective_tokens, 0.0);
    for (period_start, model, value) in &seeded {
        let migrated = store
            .provider_cursor("claude-code", &new_key_json(period_start, *model))
            .unwrap();
        assert_eq!(migrated.as_deref(), Some(value.as_str()));
    }
}

#[test]
fn unexpected_extra_provider_day_does_not_write_snapshot_or_feed() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-extra-day.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert!(result.total_tokens < 1_000.0);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unexpected_provider_day"));
    let extra_day = time::Date::from_calendar_date(2026, time::Month::July, 5).unwrap();
    let snapshot = store.snapshot_totals_for_provider_day(extra_day).unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Missing
    );
}

#[test]
fn unrequested_malformed_ccusage_row_writes_requested_zero_snapshot() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider_at(
        Some("ccusage-unrequested-malformed-row.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unexpected_provider_day"));
    assert!(!result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(snapshot.value.unwrap().total_tokens, 0.0);
}

#[test]
fn unrequested_unsupported_ccusage_row_does_not_block_requested_valid_row() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-unrequested-unsupported-shape-with-valid-requested.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 100.0);
    assert!(result
        .deltas
        .iter()
        .any(|delta| delta.provider_surface == "claude-code"));
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unexpected_provider_day"));
    assert!(!result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(snapshot.value.unwrap().total_tokens, 100.0);
}

#[test]
fn unrequested_unidentified_unified_row_does_not_force_scoped_fallback() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_with_unified_at(
        Some("ccusage-unified-aggregate-unrequested.mjs"),
        Some("ccusage-extra-day.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.provider_surface == "unified" && diagnostic.code == "unexpected_provider_day"
    }));
    assert!(!result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "aggregate_unidentified_source_ignored"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(snapshot.value.unwrap().total_tokens, 0.0);
}

#[test]
fn disappeared_requested_provider_day_writes_current_zero_without_negative_food() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let first_provider = provider_at(
        Some("ccusage-extra-day.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );
    first_provider.refresh_snapshots_only(&mut store).unwrap();
    let second_provider = provider_at(
        Some("ccusage-drop-day.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = second_provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    let requested_day = date!(2026 - 07 - 06);
    let snapshot = store
        .snapshot_totals_for_provider_day(requested_day)
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(snapshot.value.unwrap().total_tokens, 0.0);
}

#[test]
fn malformed_requested_row_blocks_snapshot_and_does_not_feed_valid_looking_rows() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider_at(
        Some("ccusage-malformed-row.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
}

#[test]
fn mixed_malformed_and_valid_requested_rows_block_without_feeding() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-mixed-malformed-row.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Blocked
    );
}

#[test]
fn unsupported_token_shape_requested_row_blocks_valid_sibling_rows() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-unsupported-token-shape-with-valid-sibling.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unsupported_token_shape"));
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Blocked
    );
}

#[test]
fn malformed_ccusage_period_blocks_zero_snapshot() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider_at(
        Some("ccusage-malformed-period-only.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_period_start"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Blocked
    );
}

#[test]
fn malformed_ccusage_period_diagnostic_omits_raw_period_and_model() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let provider = provider_at(
        Some("ccusage-malformed-period-sensitive.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();
    let rendered = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let persisted = rusqlite::Connection::open(&db_path)
        .unwrap()
        .prepare("SELECT message FROM provider_diagnostics ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    let snapshot_persisted = rusqlite::Connection::open(&db_path)
        .unwrap()
        .prepare("SELECT message FROM provider_snapshot_diagnostics ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    let all_messages = format!("{rendered}\n{persisted}\n{snapshot_persisted}");

    assert!(all_messages.contains("invalid_period_start"));
    assert!(!all_messages.contains("/Users/drew/private"));
    assert!(!all_messages.contains("secret transcript"));
    assert!(!all_messages.contains("secret-model-project-name"));
}

#[test]
fn malformed_ccusage_raw_agent_diagnostics_do_not_persist_raw_source_content() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let provider = provider_at(
        Some("ccusage-malformed-raw-agent.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
    let provider_persisted = rusqlite::Connection::open(&db_path)
        .unwrap()
        .prepare("SELECT provider_surface || ' ' || message FROM provider_diagnostics ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    let snapshot_persisted = rusqlite::Connection::open(&db_path)
        .unwrap()
        .prepare("SELECT message FROM provider_snapshot_diagnostics ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    let persisted = format!("{provider_persisted}\n{snapshot_persisted}");

    assert!(persisted.contains("malformed"));
    assert!(!persisted.contains("/Users/drew/private/project-secret"));
    assert!(!persisted.contains("project-secret"));
    assert!(!persisted.contains("secret-model-project-name"));
}

#[test]
fn claude_only_scoped_refresh_preserves_uncovered_codex_snapshot_truth() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude-code", "codex"]);
    let full_provider = provider_at(
        Some("ccusage-ok.mjs"),
        Some("ccusage-codex-ok.mjs"),
        datetime!(2026 - 05 - 09 12:00 UTC),
    );
    full_provider.refresh_snapshots_only(&mut store).unwrap();
    let before = store
        .snapshot_totals_by_source_for_provider_day(date!(2026 - 05 - 09))
        .unwrap()
        .value
        .unwrap()
        .sources;
    assert!(before
        .iter()
        .any(|source| source.accounting_source == "codex"));
    let codex_before = before
        .iter()
        .find(|source| source.accounting_source == "codex")
        .unwrap()
        .total_tokens;
    let claude_only_provider = provider_at(
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    claude_only_provider
        .refresh_snapshots_only(&mut store)
        .unwrap();

    let snapshot = store
        .snapshot_totals_by_source_for_provider_day(date!(2026 - 05 - 09))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    let after = snapshot.value.unwrap().sources;
    let codex_after = after
        .iter()
        .find(|source| source.accounting_source == "codex")
        .map(|source| source.total_tokens);
    assert_eq!(codex_after, Some(codex_before), "{after:?}");
}

#[test]
fn malformed_ccusage_period_blocks_valid_sibling_rows() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-malformed-period-with-valid-sibling.mjs"),
        None,
        datetime!(2026 - 07 - 06 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_period_start"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Blocked
    );
}

#[test]
fn ccusage_scoped_fallback_writes_one_complete_snapshot_for_sibling_sources() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude-code", "codex"]);
    let provider = provider(Some("ccusage-ok.mjs"), Some("ccusage-codex-ok.mjs"));

    let result = provider.poll(&mut store).unwrap();

    assert!(result
        .deltas
        .iter()
        .any(|delta| delta.provider_surface == "claude-code"));
    assert!(result
        .deltas
        .iter()
        .any(|delta| delta.provider_surface == "codex"));
    let snapshot = store
        .snapshot_totals_by_source_for_provider_day(date!(2026 - 05 - 09))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    let sources = snapshot
        .value
        .unwrap()
        .sources
        .iter()
        .map(|source| source.accounting_source.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(sources.contains("claude-code"), "{sources:?}");
    assert!(sources.contains("codex"), "{sources:?}");
}

#[test]
fn agentsview_poll_writes_one_complete_snapshot_for_sibling_sources() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude", "codex"]);
    let provider = agentsview_provider("agentsview-ok.mjs");

    let result = provider.poll(&mut store).unwrap();

    assert!(result
        .deltas
        .iter()
        .any(|delta| delta.provider_surface == "claude"));
    assert!(result
        .deltas
        .iter()
        .any(|delta| delta.provider_surface == "codex"));
    let snapshot = store
        .snapshot_totals_by_source_for_provider_day(date!(2026 - 06 - 18))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    let sources = snapshot
        .value
        .unwrap()
        .sources
        .iter()
        .map(|source| source.accounting_source.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(sources.contains("claude"), "{sources:?}");
    assert!(sources.contains("codex"), "{sources:?}");
}

#[test]
fn agentsview_scoped_refresh_preserves_uncovered_snapshot_truth() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude", "codex", "gemini"]);
    seed_source_snapshot(&mut store, date!(2026 - 06 - 18), "gemini", 4242.0);
    let provider = agentsview_provider("agentsview-ok.mjs");

    provider.refresh_snapshots_only(&mut store).unwrap();

    let snapshot = store
        .snapshot_totals_by_source_for_provider_day(date!(2026 - 06 - 18))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    let sources = snapshot.value.unwrap().sources;
    assert!(sources
        .iter()
        .any(|source| source.accounting_source == "claude"));
    assert!(sources
        .iter()
        .any(|source| source.accounting_source == "codex"));
    let gemini_after = sources
        .iter()
        .find(|source| source.accounting_source == "gemini")
        .map(|source| source.total_tokens);
    assert_eq!(gemini_after, Some(4242.0), "{sources:?}");
}

#[test]
fn unrequested_malformed_agentsview_row_writes_requested_zero_snapshot() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude", "codex"]);
    let provider = agentsview_provider("agentsview-unrequested-malformed-row.mjs");

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unexpected_provider_day"));
    assert!(!result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 06 - 18))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(snapshot.value.unwrap().total_tokens, 0.0);
}

#[test]
fn malformed_unified_ccusage_blocks_scoped_fallback() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    store
        .record_source_contact(
            glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1,
            "claude-code",
            glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
    let provider = provider_with_unified_at(
        Some("ccusage-unified-malformed-required-requested.mjs"),
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "malformed_required_fields"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 05 - 09))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Blocked
    );
}

#[test]
fn malformed_agentsview_period_blocks_valid_sibling_rows() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude", "codex"]);
    let provider = agentsview_provider("agentsview-malformed-period-with-valid-sibling.mjs");

    let result = provider.poll(&mut store).unwrap();

    assert_eq!(result.total_tokens, 0.0);
    assert!(result.deltas.is_empty());
    assert!(result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "invalid_period_start"));
    let snapshot = store
        .snapshot_totals_for_provider_day(date!(2026 - 06 - 18))
        .unwrap();
    assert_eq!(
        snapshot.state,
        glorp::usage::snapshot::SnapshotState::Blocked
    );
}

#[test]
fn ccusage_poll_does_not_migrate_legacy_cursor_before_durable_snapshot_write() {
    #[derive(Serialize)]
    struct LegacyKey {
        provider_surface: String,
        command: String,
        parser_version: String,
        period_start: String,
        model: Option<String>,
    }

    fn legacy_key_json(period_start: &str, model: Option<&str>) -> String {
        serde_json::to_string(&LegacyKey {
            provider_surface: "claude-code".to_string(),
            command: "ccusage".to_string(),
            parser_version: "ccusage 18.0.11".to_string(),
            period_start: period_start.to_string(),
            model: model.map(str::to_string),
        })
        .unwrap()
    }

    fn new_key_json(period_start: &str, model: Option<&str>) -> String {
        serde_json::to_string(&ProviderCursorKey {
            provider_surface: "claude-code".to_string(),
            token_contract: Some(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string()),
            command: "ccusage".to_string(),
            source_surface: "daily".to_string(),
            period_start: period_start.to_string(),
            model: model.map(str::to_string),
            raw_source_id: None,
        })
        .unwrap()
    }

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let legacy_value = serde_json::to_string(&RawTokenTotals {
        uncached_input: 1000,
        output: 1500,
        cache_creation: 300,
        cache_read: 50000,
        reasoning_output: 0,
    })
    .unwrap();
    store
        .set_provider_cursor(
            "claude-code",
            &legacy_key_json("2026-05-09", Some("claude-opus-4")),
            &legacy_value,
            "ccusage 18.0.11",
            "ccusage 18.0.11",
        )
        .unwrap();
    rusqlite::Connection::open(&db_path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_provider_snapshot_batch
             BEFORE INSERT ON provider_snapshot_batches
             BEGIN
               SELECT RAISE(FAIL, 'test snapshot write failure');
             END;",
        )
        .unwrap();
    let provider = provider_at(
        Some("ccusage-ok.mjs"),
        None,
        datetime!(2026 - 05 - 09 12:00 UTC),
    );

    let err = provider.poll(&mut store).unwrap_err();

    assert!(err.to_string().contains("test snapshot write failure"));
    let migrated = store
        .provider_cursor(
            "claude-code",
            &new_key_json("2026-05-09", Some("claude-opus-4")),
        )
        .unwrap();
    assert_eq!(migrated, None);
}

#[test]
fn live_local_agentsview_fixture_normalizes_its_own_full_cache_totals() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude", "codex"]);
    let provider = agentsview_provider("agentsview-ok.mjs");

    let result = provider.poll(&mut store).unwrap();
    let codex = result
        .deltas
        .iter()
        .find(|delta| {
            delta.provider_surface == "codex" && delta.model.as_deref() == Some("gpt-5.5")
        })
        .unwrap();
    let claude = result
        .deltas
        .iter()
        .find(|delta| {
            delta.provider_surface == "claude" && delta.model.as_deref() == Some("claude-opus-4-8")
        })
        .unwrap();

    let live_local_total = result
        .deltas
        .iter()
        .map(|delta| delta.total_tokens)
        .sum::<f64>();

    // This test asserts the live-local agentsview fixture's own normalization
    // semantics. The captured public Tokenmaxxing comparison fixture is
    // asserted independently in
    // `tokenmaxxing_comparison_fixture_preserves_captured_public_totals`.
    assert_eq!(
        live_local_total,
        46_011_892.0 + 743_812_222.0,
        "live-local fixture total should be derived from its own agentsview rows"
    );
    assert_eq!(codex.source_identity.display_name, "codex");
    assert_eq!(claude.source_identity.display_name, "claude");
    assert_eq!(
        codex.token_contract,
        glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1
    );
    assert_eq!(codex.total_tokens, 31028179.0 + 2463075.0 + 517477376.0);
    assert_eq!(
        claude.total_tokens,
        612992.0 + 1072059.0 + 5083568.0 + 34477061.0
    );
    let live_local_by_source = result.deltas.iter().fold(
        std::collections::BTreeMap::<&str, f64>::new(),
        |mut totals, delta| {
            *totals.entry(delta.provider_surface.as_str()).or_default() += delta.total_tokens;
            totals
        },
    );
    assert_eq!(live_local_by_source.get("claude"), Some(&46_011_892.0));
    assert_eq!(live_local_by_source.get("codex"), Some(&743_812_222.0));
    assert_eq!(
        result.total_tokens,
        result.deltas.iter().map(|d| d.total_tokens).sum::<f64>()
    );
}

#[test]
fn agentsview_provider_requires_los_angeles_timezone_arg() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = agentsview_provider("agentsview-ok.mjs");

    let result = provider.poll(&mut store).unwrap();

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn agentsview_invalid_json_and_helper_stderr_are_sanitized() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let invalid = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(agentsview_fixture("agentsview-invalid-json.mjs")),
        },
    )
    .poll(&mut store)
    .unwrap();
    let stderr = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(agentsview_fixture("agentsview-secret-stderr.mjs")),
        },
    )
    .poll(&mut store)
    .unwrap();
    let rendered = format!("{:?}{:?}", invalid.diagnostics, stderr.diagnostics);

    assert!(rendered.contains("invalid_json"));
    assert!(rendered.contains("helper_exit"));
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
    assert!(!rendered.contains("/Users/drew/private"));
}

#[test]
fn agentsview_present_malformed_token_field_rejects_row_with_sanitized_diagnostic() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new_with_now_for_test(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(agentsview_fixture("agentsview-malformed-number.mjs")),
        },
        datetime!(2026 - 06 - 18 20:00 UTC),
    );

    let result = provider.poll(&mut store).unwrap();
    let rendered = format!("{:?}", result.diagnostics);

    assert!(result.deltas.is_empty());
    assert!(rendered.contains("malformed_token_field"));
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
    assert!(!rendered.contains("inputTokens"));
}

#[test]
fn agentsview_omitted_token_bucket_fields_still_default_to_zero() {
    let text = std::fs::read_to_string(fixture_json("agentsview-omitted-zeros.json")).unwrap();
    let batch = glorp::usage::normalize::normalize_agentsview_json("codex", &text).unwrap();
    let record = batch.records.first().unwrap();

    assert!(batch.diagnostics.is_empty(), "{:?}", batch.diagnostics);
    assert_eq!(
        record.raw_totals,
        RawTokenTotals {
            uncached_input: 10,
            output: 20,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        }
    );
}

#[test]
fn agentsview_cursor_key_carries_token_contract_for_cutover_replay_protection() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    record_known_sources(&mut store, &["claude", "codex"]);
    let provider = agentsview_provider("agentsview-ok.mjs");

    let result = provider.poll(&mut store).unwrap();
    let codex = result
        .deltas
        .iter()
        .find(|delta| {
            delta.provider_surface == "codex" && delta.model.as_deref() == Some("gpt-5.5")
        })
        .unwrap();
    let cursor_key: Value = serde_json::from_str(&codex.cursor_update.cursor_key).unwrap();

    assert_eq!(
        cursor_key["token_contract"],
        glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1
    );
    assert_eq!(cursor_key["provider_surface"], "codex");
    assert_eq!(cursor_key["raw_source_id"], "codex");
    assert_eq!(cursor_key["command"], "agentsview usage daily");
    assert_ne!(cursor_key["command"], "ccusage-codex");
}

#[test]
fn agentsview_discovery_prefers_env_bin_before_path_candidate() {
    let env_path = agentsview_fixture("agentsview-ok.mjs");
    let path_path = agentsview_fixture("agentsview-fails.mjs");
    let discovered = glorp::usage::agentsview::AgentsviewDiscovery::from_sources(
        [("GLORP_AGENTSVIEW_BIN", env_path.as_path())],
        [path_path.as_path()],
    )
    .unwrap();

    assert_eq!(discovered.agentsview.unwrap(), env_path);
}

#[test]
fn agentsview_configured_missing_path_returns_sanitized_diagnostic() {
    let dir = tempdir().unwrap();
    let missing_helper = dir.path().join("missing-agentsview");
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths { agentsview: Some(missing_helper) },
    );

    let result = provider.poll(&mut store).unwrap();
    let rendered = format!("{:?}", result.diagnostics);

    assert!(result.deltas.is_empty());
    assert_eq!(result.diagnostics.len(), 2);
    assert!(result
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == "missing_helper"));
    assert!(!rendered.contains("missing-agentsview"));
}
