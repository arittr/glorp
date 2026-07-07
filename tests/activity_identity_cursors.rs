use glorp::game::runtime::apply_usage_poll;
use glorp::storage::state::PetState;
use glorp::storage::usage_store::{ProviderCursorUpdate, UsageStore};
use glorp::usage::identity::SourceIdentity;
use glorp::usage::normalize::RawTokenTotals;
use glorp::usage::provider::{UsageDelta, UsagePollResult};
use tempfile::tempdir;
use time::OffsetDateTime;

#[test]
fn repeated_poll_after_first_contact_emits_zero_new_deltas() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("seed", "buddy");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = OffsetDateTime::now_utc();

    let poll = UsagePollResult {
        deltas: vec![UsageDelta {
            provider_surface: "gemini".into(),
            source_identity: SourceIdentity::from_provider_surface("gemini"),
            command: "ccusage daily".into(),
            effective_tokens: 10_000.0,
            total_tokens: 10_000.0,
            token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
            confidence: "local-log-derived".into(),
            period_start: now - time::Duration::days(1),
            observed_at: now,
            model: Some("gemini-2.5-flash".into()),
            cursor_update: ProviderCursorUpdate {
                provider_surface: "gemini".into(),
                cursor_key: "gemini-contact".into(),
                cursor_value: "totals-v1".into(),
                provider_version: "20".into(),
                parser_version: "20".into(),
            },
            token_totals: Some(RawTokenTotals {
                uncached_input: 10_000,
                output: 0,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            }),
            feed_highwater_updates: Vec::new(),
        }],
        diagnostics: vec![],
        total_effective_tokens: 10_000.0,
        total_tokens: 10_000.0,
    };

    let first = apply_usage_poll(&mut state, &mut store, &poll, now).unwrap();
    assert_eq!(first.recent_effective_tokens, 0.0);

    let second = apply_usage_poll(&mut state, &mut store, &poll, now).unwrap();
    assert_eq!(second.recent_effective_tokens, 0.0);
}

#[test]
fn unknown_source_first_contact_does_not_unlock_codex_signal_lamp() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("seed", "buddy");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = OffsetDateTime::now_utc();

    let poll = UsagePollResult {
        deltas: vec![UsageDelta {
            provider_surface: "unknown-agent".into(),
            source_identity: SourceIdentity::from_provider_surface("unknown-agent"),
            command: "ccusage daily".into(),
            effective_tokens: 10_000.0,
            total_tokens: 10_000.0,
            token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
            confidence: "local-log-derived".into(),
            period_start: now - time::Duration::days(1),
            observed_at: now,
            model: None,
            cursor_update: ProviderCursorUpdate {
                provider_surface: "unknown-agent".into(),
                cursor_key: "unknown-contact".into(),
                cursor_value: "totals-v1".into(),
                provider_version: "20".into(),
                parser_version: "20".into(),
            },
            token_totals: Some(RawTokenTotals {
                uncached_input: 10_000,
                output: 0,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            }),
            feed_highwater_updates: Vec::new(),
        }],
        diagnostics: vec![],
        total_effective_tokens: 10_000.0,
        total_tokens: 10_000.0,
    };

    apply_usage_poll(&mut state, &mut store, &poll, now).unwrap();

    assert!(
        !state
            .habitat
            .earned_props
            .iter()
            .any(|p| p.id.as_str() == "codex_signal_lamp"),
        "unknown source must not unlock the legacy codex signal lamp"
    );
}
