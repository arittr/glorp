use glorp::{
    game::runtime::{
        apply_unapplied_usage, apply_usage_poll, stage_usage_poll_deltas, DISCONTINUITY_GUARD_RATIO,
    },
    storage::state::PetState,
    storage::usage_store::{ProviderCursorUpdate, UsageStore},
    usage::{
        normalize::RawTokenTotals,
        provider::{ProviderCursorKey, UsagePollResult},
        snapshot::ProviderSnapshotRowInput,
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
fn known_source_feed_highwater_advances_only_after_usage_apply() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
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
    assert_eq!(
        store
            .source_day_highwater_for_test(
                TOKENMAXXING_TOTAL_V1,
                "claude-code",
                date!(2026 - 07 - 07),
            )
            .unwrap(),
        0.0
    );

    let total_tokens = plan
        .deltas
        .iter()
        .map(|delta| delta.total_tokens)
        .sum::<f64>();
    let poll = UsagePollResult {
        deltas: plan.deltas,
        diagnostics: Vec::new(),
        total_effective_tokens: total_tokens,
        total_tokens,
    };
    apply_usage_poll(&mut state, &mut store, &poll, now).unwrap();

    assert_eq!(
        store
            .source_day_highwater_for_test(
                TOKENMAXXING_TOTAL_V1,
                "claude-code",
                date!(2026 - 07 - 07),
            )
            .unwrap(),
        100.0
    );
}

#[test]
fn json_provider_cursor_counts_as_feed_contact_without_source_contact_row() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 07 18:00 UTC);

    store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: "claude-code".into(),
                cursor_key: "helper_version::ccusage".into(),
                cursor_value: "20.0.6".into(),
                provider_version: "ccusage 20.0.6".into(),
                parser_version: "ccusage 20.0.6".into(),
            }],
            now,
        )
        .unwrap();
    assert!(!store
        .source_has_feed_contact(TOKENMAXXING_TOTAL_V1, "claude-code")
        .unwrap());

    let cursor_key = serde_json::to_string(&ProviderCursorKey {
        provider_surface: "claude-code".into(),
        token_contract: Some(TOKENMAXXING_TOTAL_V1.into()),
        command: "ccusage claude daily --json --offline".into(),
        source_surface: "daily".into(),
        period_start: "2026-07-07".into(),
        model: Some("claude-fable-5".into()),
        raw_source_id: None,
    })
    .unwrap();
    store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: "claude-code".into(),
                cursor_key,
                cursor_value: serde_json::to_string(&RawTokenTotals {
                    uncached_input: 100,
                    output: 0,
                    cache_creation: 0,
                    cache_read: 0,
                    reasoning_output: 0,
                })
                .unwrap(),
                provider_version: "ccusage 20.0.6".into(),
                parser_version: "ccusage 20.0.6".into(),
            }],
            now,
        )
        .unwrap();

    assert!(store
        .source_has_feed_contact(TOKENMAXXING_TOTAL_V1, "claude-code")
        .unwrap());
}

#[test]
fn existing_provider_cursor_seeds_known_source_snapshot_baseline() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    let seed = datetime!(2026 - 07 - 07 18:00 UTC);
    let now = datetime!(2026 - 07 - 07 18:05 UTC);

    let cursor_key = serde_json::to_string(&ProviderCursorKey {
        provider_surface: "claude-code".into(),
        token_contract: Some(TOKENMAXXING_TOTAL_V1.into()),
        command: "ccusage claude daily --json --offline".into(),
        source_surface: "daily".into(),
        period_start: "2026-07-07".into(),
        model: Some("claude-fable-5".into()),
        raw_source_id: None,
    })
    .unwrap();
    let prior_buckets = RawTokenTotals {
        uncached_input: 100,
        output: 0,
        cache_creation: 0,
        cache_read: 0,
        reasoning_output: 0,
    };
    store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: "claude-code".into(),
                cursor_key: cursor_key.clone(),
                cursor_value: serde_json::to_string(&prior_buckets).unwrap(),
                provider_version: "ccusage 20.0.6".into(),
                parser_version: "ccusage 20.0.6".into(),
            }],
            seed,
        )
        .unwrap();
    assert!(store
        .source_has_feed_contact(TOKENMAXXING_TOTAL_V1, "claude-code")
        .unwrap());

    let mut snapshot_row = row(
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
    );
    snapshot_row.cursor_update.cursor_key = cursor_key;
    snapshot_row.cursor_update.cursor_value =
        serde_json::to_string(&snapshot_row.raw_token_buckets.unwrap()).unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(&[snapshot_row], now)
        .unwrap();

    assert_eq!(plan.deltas.len(), 1);
    assert_eq!(plan.deltas[0].total_tokens, 20.0);
    assert!(plan.deltas[0].token_totals.is_some());

    let total_tokens = plan
        .deltas
        .iter()
        .map(|delta| delta.total_tokens)
        .sum::<f64>();
    let poll = UsagePollResult {
        deltas: plan.deltas,
        diagnostics: Vec::new(),
        total_effective_tokens: total_tokens,
        total_tokens,
    };
    apply_usage_poll(&mut state, &mut store, &poll, now).unwrap();

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
fn corrected_total_only_baseline_resyncs_from_no_feed_exact_snapshot() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let first = datetime!(2026 - 07 - 07 18:00 UTC);
    let resync = datetime!(2026 - 07 - 07 18:05 UTC);
    let later = datetime!(2026 - 07 - 07 18:10 UTC);

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

    let resync_plan = store
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
            resync,
        )
        .unwrap();
    assert!(resync_plan.deltas.is_empty());

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
    assert!(later_plan.deltas[0].token_totals.is_some());
    assert_ne!(later_plan.deltas[0].confidence, "corrected-total-only");
}

#[test]
fn unchanged_exact_row_does_not_force_total_only_for_matching_group_excess() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 07 - 07 18:00 UTC);
    store
        .seed_exact_row_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 07),
            Some("unchanged-model"),
            RawTokenTotals {
                uncached_input: 100,
                output: 0,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            },
            now,
        )
        .unwrap();
    store
        .seed_exact_row_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 07),
            Some("growing-model"),
            RawTokenTotals {
                uncached_input: 50,
                output: 0,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            },
            now,
        )
        .unwrap();
    store
        .seed_source_day_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 07),
            150.0,
            now,
        )
        .unwrap();

    let plan = store
        .feed_deltas_for_snapshot_rows(
            &[
                row(
                    date!(2026 - 07 - 07),
                    "unchanged-model",
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
                    "growing-model",
                    70,
                    RawTokenTotals {
                        uncached_input: 70,
                        output: 0,
                        cache_creation: 0,
                        cache_read: 0,
                        reasoning_output: 0,
                    },
                ),
            ],
            now,
        )
        .unwrap();

    assert_eq!(plan.deltas.len(), 1);
    assert_eq!(plan.deltas[0].total_tokens, 20.0);
    assert!(plan.deltas[0].token_totals.is_some());
    assert_ne!(plan.deltas[0].confidence, "corrected-total-only");
}

#[test]
fn first_contact_snapshot_seeds_same_day_highwaters_without_feeding_history() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
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
        100.0
    );

    let total_tokens = later_plan
        .deltas
        .iter()
        .map(|delta| delta.total_tokens)
        .sum::<f64>();
    let poll = UsagePollResult {
        deltas: later_plan.deltas,
        diagnostics: Vec::new(),
        total_effective_tokens: total_tokens,
        total_tokens,
    };
    apply_usage_poll(&mut state, &mut store, &poll, later).unwrap();

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

#[test]
fn total_only_aggregate_uses_stageable_row_when_first_cursor_is_current() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 1_000_000.0;
    let seed = datetime!(2026 - 07 - 06 20:00 UTC);
    let now = datetime!(2026 - 07 - 06 20:10 UTC);

    store
        .seed_exact_row_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 06),
            Some("unchanged-model"),
            RawTokenTotals {
                uncached_input: 100,
                output: 0,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            },
            seed,
        )
        .unwrap();
    store
        .seed_exact_row_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 06),
            Some("mixed-model"),
            RawTokenTotals {
                uncached_input: 50,
                output: 0,
                cache_creation: 0,
                cache_read: 0,
                reasoning_output: 0,
            },
            seed,
        )
        .unwrap();
    store
        .seed_source_day_highwater_for_test(
            TOKENMAXXING_TOTAL_V1,
            "claude-code",
            date!(2026 - 07 - 06),
            150.0,
            seed,
        )
        .unwrap();

    let unchanged = row(
        date!(2026 - 07 - 06),
        "unchanged-model",
        100,
        RawTokenTotals {
            uncached_input: 100,
            output: 0,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        },
    );
    store
        .advance_cursors(vec![unchanged.cursor_update.clone()], seed)
        .unwrap();
    let mixed = row(
        date!(2026 - 07 - 06),
        "mixed-model",
        70,
        RawTokenTotals {
            uncached_input: 40,
            output: 30,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        },
    );

    let plan = store
        .feed_deltas_for_snapshot_rows(&[unchanged, mixed], now)
        .unwrap();
    assert_eq!(plan.deltas.len(), 1);
    assert_eq!(plan.deltas[0].total_tokens, 20.0);
    assert_eq!(plan.deltas[0].confidence, "corrected-total-only");
    assert_eq!(
        store
            .source_day_highwater_for_test(
                TOKENMAXXING_TOTAL_V1,
                "claude-code",
                date!(2026 - 07 - 06),
            )
            .unwrap(),
        150.0
    );

    let total_tokens = plan
        .deltas
        .iter()
        .map(|delta| delta.total_tokens)
        .sum::<f64>();
    let poll = UsagePollResult {
        deltas: plan.deltas,
        diagnostics: Vec::new(),
        total_effective_tokens: total_tokens,
        total_tokens,
    };
    let staged_ids = stage_usage_poll_deltas(
        &mut store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();
    assert!(
        !staged_ids.is_empty(),
        "aggregate delta should not be skipped by the first row's current cursor"
    );

    let update = apply_unapplied_usage(&mut state, &mut store, now, false).unwrap();
    assert_eq!(update.recent_effective_tokens, 20.0);
    store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();
    assert_eq!(state.lifetime_effective_tokens, 20.0);
    assert_eq!(
        store
            .source_day_highwater_for_test(
                TOKENMAXXING_TOTAL_V1,
                "claude-code",
                date!(2026 - 07 - 06),
            )
            .unwrap(),
        170.0
    );
}
