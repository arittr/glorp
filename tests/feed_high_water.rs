use glorp::{
    storage::usage_store::{ProviderCursorUpdate, UsageStore},
    usage::{
        normalize::RawTokenTotals, snapshot::ProviderSnapshotRowInput,
        token_contract::TOKENMAXXING_TOTAL_V1,
    },
};
use tempfile::tempdir;
use time::{macros::date, macros::datetime, Date};

fn row(day: Date, model: &str, total: u64, buckets: RawTokenTotals) -> ProviderSnapshotRowInput {
    row_with_buckets(day, model, total, Some(buckets))
}

fn row_without_buckets(day: Date, model: &str, total: u64) -> ProviderSnapshotRowInput {
    row_with_buckets(day, model, total, None)
}

fn row_with_buckets(
    day: Date,
    model: &str,
    total: u64,
    buckets: Option<RawTokenTotals>,
) -> ProviderSnapshotRowInput {
    ProviderSnapshotRowInput {
        replacement_scope_id: "claude-code:local-usage".into(),
        collector_scope_id: "claude-code:local-usage".into(),
        collector_surface: "ccusage:claude-code".into(),
        command: "ccusage claude daily --json --offline".into(),
        token_contract: TOKENMAXXING_TOTAL_V1.into(),
        accounting_source: "claude-code".into(),
        provider_day: day,
        model: Some(model.into()),
        source_surface: "daily".into(),
        provider_period: day.to_string(),
        raw_source_id_hash: Some(format!("hash:{model}")),
        cursor_key_hash: format!("hash:{model}"),
        cursor_update: ProviderCursorUpdate {
            provider_surface: "claude-code".into(),
            cursor_key: format!("cursor:{model}"),
            cursor_value: serde_json::to_string(&buckets.unwrap_or_default()).unwrap(),
            provider_version: "ccusage 20.0.6".into(),
            parser_version: "ccusage 20.0.6".into(),
        },
        raw_token_buckets: buckets,
        total_tokens: total as f64,
        cost_usd: None,
        confidence: "local-log-derived".into(),
    }
}

#[test]
fn known_source_new_day_feeds_from_zero_instead_of_first_contact_seeding() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 07 18:00 UTC);
    store
        .record_source_contact(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            "source_first_contact",
            now,
        )
        .unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 07),
                "claude-fable-5",
                100,
                RawTokenTotals {
                    uncached_input: 100,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            now,
        )
        .unwrap();

    assert_eq!(plan.deltas.len(), 1);
    assert_eq!(plan.deltas[0].total_tokens, 100.0);
    assert!(plan.cursor_seeds.is_empty());
}

#[test]
fn first_contact_snapshot_seeds_same_day_highwaters_without_feeding_history() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let first = datetime!(2026 - 07 - 07 18:00 UTC);
    let later = datetime!(2026 - 07 - 07 18:05 UTC);

    let first_plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 07),
                "claude-fable-5",
                100,
                RawTokenTotals {
                    uncached_input: 100,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            first,
        )
        .unwrap();

    assert!(first_plan.deltas.is_empty());
    assert_eq!(first_plan.cursor_seeds.len(), 1);

    let later_plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 07),
                "claude-fable-5",
                120,
                RawTokenTotals {
                    uncached_input: 120,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            later,
        )
        .unwrap();

    assert_eq!(later_plan.deltas.len(), 1);
    assert_eq!(later_plan.deltas[0].total_tokens, 20.0);
    assert_eq!(
        store
            .source_day_highwater_for_test(
                TOKENMAXXING_TOTAL_V1,
                "claude-code",
                date!(2026 - 07 - 07),
            )
            .unwrap(),
        120.0
    );
}

#[test]
fn first_contact_snapshot_seeds_all_source_days_before_feeding() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let first = datetime!(2026 - 07 - 07 18:00 UTC);
    let later = datetime!(2026 - 07 - 07 18:05 UTC);

    let first_plan = store
        .feed_deltas_for_snapshot_rows(
            &[
                row(
                    date!(2026 - 07 - 06),
                    "claude-fable-5",
                    100,
                    RawTokenTotals {
                        uncached_input: 100,
                        output: 0,
                        cache_creation: 0,
                        cache_read: 0,
                        reasoning_output: 0,
                    },
                ),
                row(
                    date!(2026 - 07 - 07),
                    "claude-fable-5",
                    200,
                    RawTokenTotals {
                        uncached_input: 200,
                        output: 0,
                        cache_creation: 0,
                        cache_read: 0,
                        reasoning_output: 0,
                    },
                ),
            ],
            first,
        )
        .unwrap();

    assert!(first_plan.deltas.is_empty());
    assert_eq!(first_plan.cursor_seeds.len(), 2);
    assert_eq!(
        store
            .source_day_highwater_for_test(
                TOKENMAXXING_TOTAL_V1,
                "claude-code",
                date!(2026 - 07 - 06),
            )
            .unwrap(),
        100.0
    );
    assert_eq!(
        store
            .source_day_highwater_for_test(
                TOKENMAXXING_TOTAL_V1,
                "claude-code",
                date!(2026 - 07 - 07),
            )
            .unwrap(),
        200.0
    );

    let later_plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 07),
                "claude-fable-5",
                250,
                RawTokenTotals {
                    uncached_input: 250,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            later,
        )
        .unwrap();

    assert_eq!(later_plan.deltas.len(), 1);
    assert_eq!(later_plan.deltas[0].total_tokens, 50.0);
}

#[test]
fn first_contact_with_missing_raw_buckets_seeds_corrected_total_only() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let first = datetime!(2026 - 07 - 07 18:00 UTC);
    let later = datetime!(2026 - 07 - 07 18:05 UTC);

    let first_plan = store
        .feed_deltas_for_snapshot_rows(
            &[
                row(
                    date!(2026 - 07 - 07),
                    "claude-fable-5",
                    100,
                    RawTokenTotals {
                        uncached_input: 100,
                        output: 0,
                        cache_creation: 0,
                        cache_read: 0,
                        reasoning_output: 0,
                    },
                ),
                row_without_buckets(date!(2026 - 07 - 07), "unknown-breakdown", 0),
            ],
            first,
        )
        .unwrap();

    assert!(first_plan.deltas.is_empty());
    assert_eq!(first_plan.cursor_seeds.len(), 2);

    let later_plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 07),
                "claude-fable-5",
                110,
                RawTokenTotals {
                    uncached_input: 110,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            later,
        )
        .unwrap();

    assert_eq!(later_plan.deltas.len(), 1);
    assert_eq!(later_plan.deltas[0].total_tokens, 10.0);
    assert_eq!(later_plan.deltas[0].confidence, "corrected-total-only");
    assert_eq!(later_plan.deltas[0].token_totals, None);
}

#[test]
fn source_day_aggregate_highwater_blocks_model_remap_double_feed() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    store
        .seed_source_day_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 06),
            1_060.0,
            now,
        )
        .unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 06),
                "renamed-model",
                531,
                RawTokenTotals {
                    uncached_input: 531,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            now,
        )
        .unwrap();

    assert!(plan.deltas.is_empty());
}

#[test]
fn mixed_bucket_rebound_feeds_total_only_without_token_shape() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    store
        .seed_exact_row_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 06),
            Some("claude-fable-5"),
            RawTokenTotals {
                uncached_input: 60,
                output: 40,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            },
            now,
        )
        .unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(
            &[row(
                date!(2026 - 07 - 06),
                "claude-fable-5",
                110,
                RawTokenTotals {
                    uncached_input: 50,
                    output: 60,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                },
            )],
            now,
        )
        .unwrap();

    assert_eq!(plan.deltas.len(), 1);
    assert_eq!(plan.deltas[0].total_tokens, 10.0);
    assert_eq!(plan.deltas[0].confidence, "corrected-total-only");
    assert_eq!(plan.deltas[0].token_totals, None);
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "mixed_bucket_correction"));
}
