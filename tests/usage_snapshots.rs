use glorp::{
    storage::usage_store::{ProviderCursorUpdate, UsageStore},
    usage::{
        normalize::RawTokenTotals,
        snapshot::{
            DayTotals, ProviderSnapshotBatchInput, ProviderSnapshotDiagnosticInput,
            ProviderSnapshotRowInput, SnapshotState,
        },
        token_contract::TOKENMAXXING_TOTAL_V1,
    },
};
use rusqlite::params;
use tempfile::tempdir;
use time::{macros::date, macros::datetime, Date, OffsetDateTime};

fn batch(day: Date, observed_at: OffsetDateTime) -> ProviderSnapshotBatchInput {
    batch_in_scope(day, observed_at, "claude-code:local-usage")
}

fn batch_in_scope(
    day: Date,
    observed_at: OffsetDateTime,
    collector_scope_id: &str,
) -> ProviderSnapshotBatchInput {
    ProviderSnapshotBatchInput {
        collector_scope_id: collector_scope_id.into(),
        collector_surface: "ccusage:claude-code".into(),
        command: "ccusage claude daily --json --offline".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        requested_provider_days: vec![day],
        provider_version: "ccusage 20.0.6".into(),
        parser_version: "ccusage 20.0.6".into(),
        observed_at,
    }
}

fn row(
    day: Date,
    model: &str,
    total: f64,
    _observed_at: OffsetDateTime,
) -> ProviderSnapshotRowInput {
    row_in_scope(
        day,
        model,
        total,
        _observed_at,
        "claude-code:local-usage",
        "claude-code:local-usage",
    )
}

fn row_in_scope(
    day: Date,
    model: &str,
    total: f64,
    _observed_at: OffsetDateTime,
    replacement_scope_id: &str,
    collector_scope_id: &str,
) -> ProviderSnapshotRowInput {
    ProviderSnapshotRowInput {
        replacement_scope_id: replacement_scope_id.into(),
        collector_scope_id: collector_scope_id.into(),
        collector_surface: "ccusage:claude-code".into(),
        command: "ccusage claude daily --json --offline".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: "claude-code".into(),
        provider_day: day,
        model: Some(model.into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some("hash:source".into()),
        cursor_key_hash: format!("hash:{model}"),
        cursor_update: ProviderCursorUpdate {
            provider_surface: "claude-code".into(),
            cursor_key: format!("cursor:{model}"),
            cursor_value: format!("raw:{total}"),
            provider_version: "ccusage 20.0.6".into(),
            parser_version: "ccusage 20.0.6".into(),
        },
        raw_token_buckets: Some(RawTokenTotals {
            uncached_input: total as u64,
            output: 0,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        }),
        total_tokens: total,
        cost_usd: None,
        confidence: "local-log-derived".into(),
    }
}

fn non_blocking_diagnostic(
    day: Date,
    observed_at: OffsetDateTime,
) -> ProviderSnapshotDiagnosticInput {
    ProviderSnapshotDiagnosticInput {
        diagnostic_kind: "optional_parse_warning".into(),
        collector_scope_id: "claude-code:local-usage".into(),
        replacement_scope_id: Some("claude-code:local-usage".into()),
        requested_provider_days: vec![day],
        provider_day: Some(day),
        reason_code: "optional_field_ignored".into(),
        message: "optional field ignored while parsing ccusage".into(),
        observed_at,
    }
}

fn blocked(day: Date, observed_at: OffsetDateTime) -> ProviderSnapshotDiagnosticInput {
    ProviderSnapshotDiagnosticInput {
        diagnostic_kind: "run_blocked".into(),
        collector_scope_id: "claude-code:local-usage".into(),
        replacement_scope_id: Some("claude-code:local-usage".into()),
        requested_provider_days: vec![day],
        provider_day: Some(day),
        reason_code: "malformed_required_fields".into(),
        message: "ccusage malformed_required_fields".into(),
        observed_at,
    }
}

#[test]
fn canonical_replacement_scope_cutover_uses_new_scope_total_and_records_decrease() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let first_at = datetime!(2026 - 07 - 06 20:00 UTC);
    let second_at = datetime!(2026 - 07 - 06 21:00 UTC);

    store
        .write_provider_snapshot_batch(
            &batch_in_scope(day, first_at, "collector:scope-a"),
            &[row_in_scope(
                day,
                "claude-fable-5",
                100.0,
                first_at,
                "replacement:scope-a",
                "collector:scope-a",
            )],
            &[],
        )
        .unwrap();
    store
        .write_provider_snapshot_batch(
            &batch_in_scope(day, second_at, "collector:scope-b"),
            &[row_in_scope(
                day,
                "claude-fable-5",
                60.0,
                second_at,
                "replacement:scope-b",
                "collector:scope-b",
            )],
            &[],
        )
        .unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Current);
    assert_eq!(result.value, Some(DayTotals { total_tokens: 60.0 }));

    let correction = store
        .raw_connection_for_test()
        .query_row(
            "SELECT previous_total_tokens, current_total_tokens, decrease_tokens
             FROM provider_corrections
             WHERE token_contract = ?1 AND accounting_source = ?2 AND provider_day = ?3",
            params![TOKENMAXXING_TOTAL_V1, "claude-code", day.to_string()],
            |row| {
                Ok((
                    row.get::<_, f64>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, f64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(correction, (100.0, 60.0, 40.0));
}

#[test]
fn complete_run_persists_non_blocking_diagnostic_and_returns_current_snapshot() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let observed_at = datetime!(2026 - 07 - 06 20:00 UTC);

    store
        .write_provider_snapshot_batch(
            &batch(day, observed_at),
            &[row(day, "claude-fable-5", 531.0, observed_at)],
            &[non_blocking_diagnostic(day, observed_at)],
        )
        .unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Current);
    assert_eq!(result.value, Some(DayTotals { total_tokens: 531.0 }));

    let diagnostic = store
        .raw_connection_for_test()
        .query_row(
            "SELECT diagnostic_kind, reason_code, run_id IS NOT NULL
             FROM provider_snapshot_diagnostics
             WHERE provider_day = ?1",
            params![day.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(
        diagnostic,
        (
            "optional_parse_warning".to_string(),
            "optional_field_ignored".to_string(),
            true
        )
    );
}

#[test]
fn complete_zero_row_run_replaces_prior_visible_day_with_current_zero() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let first_at = datetime!(2026 - 07 - 06 20:00 UTC);
    let second_at = datetime!(2026 - 07 - 06 21:00 UTC);

    store
        .write_provider_snapshot_batch(
            &batch(day, first_at),
            &[row(day, "claude-fable-5", 531.0, first_at)],
            &[],
        )
        .unwrap();
    store
        .write_provider_snapshot_batch(&batch(day, second_at), &[], &[])
        .unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Current);
    assert_eq!(result.value, Some(DayTotals { total_tokens: 0.0 }));
}

#[test]
fn blocked_latest_attempt_with_prior_snapshot_returns_stale_value() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let first_at = datetime!(2026 - 07 - 06 20:00 UTC);
    let blocked_at = datetime!(2026 - 07 - 06 21:00 UTC);

    store
        .write_provider_snapshot_batch(
            &batch(day, first_at),
            &[row(day, "claude-fable-5", 531.0, first_at)],
            &[],
        )
        .unwrap();
    store
        .record_snapshot_failure(&blocked(day, blocked_at))
        .unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Stale);
    assert_eq!(result.value, Some(DayTotals { total_tokens: 531.0 }));
    assert_eq!(result.reason.as_deref(), Some("malformed_required_fields"));
}

#[test]
fn top_level_failure_before_any_snapshot_returns_blocked_with_no_value() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    store
        .record_snapshot_failure(&blocked(day, datetime!(2026 - 07 - 06 21:00 UTC)))
        .unwrap();

    let result = store.snapshot_totals_for_provider_day(day).unwrap();
    assert_eq!(result.state, SnapshotState::Blocked);
    assert_eq!(result.value, None);
    assert_eq!(result.reason.as_deref(), Some("malformed_required_fields"));
}

#[test]
fn model_remap_with_same_source_day_total_is_identity_diagnostic_not_downward_correction() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let day = date!(2026 - 07 - 06);
    let first_at = datetime!(2026 - 07 - 06 20:00 UTC);
    let second_at = datetime!(2026 - 07 - 06 21:00 UTC);

    store
        .write_provider_snapshot_batch(
            &batch(day, first_at),
            &[row(day, "old-model", 531.0, first_at)],
            &[],
        )
        .unwrap();
    store
        .write_provider_snapshot_batch(
            &batch(day, second_at),
            &[row(day, "new-model", 531.0, second_at)],
            &[],
        )
        .unwrap();

    let correction_count: i64 = store
        .raw_connection_for_test()
        .query_row(
            "SELECT COUNT(*) FROM provider_corrections
             WHERE token_contract = ?1 AND accounting_source = ?2 AND provider_day = ?3",
            params![TOKENMAXXING_TOTAL_V1, "claude-code", day.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        correction_count, 0,
        "unchanged source-day aggregate must not create downward correction"
    );

    let identity_diagnostic_count: i64 = store
        .raw_connection_for_test()
        .query_row(
            "SELECT COUNT(*) FROM provider_snapshot_diagnostics
             WHERE diagnostic_kind = 'identity_remap' AND provider_day = ?1",
            params![day.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(identity_diagnostic_count, 1);
}

#[test]
fn no_attempt_returns_missing_not_polled() {
    let dir = tempdir().unwrap();
    let store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let result = store
        .snapshot_totals_for_provider_day(date!(2026 - 07 - 06))
        .unwrap();
    assert_eq!(result.state, SnapshotState::Missing);
    assert_eq!(result.value, None);
    assert_eq!(result.reason.as_deref(), Some("not_polled"));
}
