use glorp::{
    game::{
        evolution::Stage,
        runtime::{
            apply_unapplied_usage, apply_usage_poll, stage_usage_poll_deltas,
            DISCONTINUITY_GUARD_RATIO,
        },
    },
    storage::{
        state::{NarrativeEvent, PetState, Vitals},
        usage_store::{NormalizedUsageEvent, ProviderCursorUpdate, UsageStore},
    },
    usage::{
        identity::SourceIdentity,
        normalize::RawTokenTotals,
        provider::{UsageDelta, UsagePollResult, UsageProvider},
    },
};
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::tempdir;
use time::{macros::datetime, Duration};

static POLL_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn provider_delta_updates_pet_state_and_records_evolution_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.vitals = Vitals { fed: 40.0, happiness: 40.0, energy: 40.0 };
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);
    let poll = poll_with_delta(100_000.0, now);

    // Two polls with distinct cursor_values represent successive bumps in provider totals.
    // The unapplied ledger's idempotency would collapse identical polls into one row.
    let poll2 = poll_with_delta(100_000.0, now);
    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();
    apply_usage_poll(&mut state, &mut usage_store, &poll2, now).unwrap();

    // Two polls of one calibrated active day each reach s3, with the second
    // poll staying in the post-day diminishing-return range.
    // Smearing still writes multiple ledger rows, but XP is based on the
    // aggregate poll total so the bucket split cannot accelerate evolution.
    assert_eq!(state.lifetime_effective_tokens, 200_000.0);
    assert_eq!(state.stage, Stage::S3);
    assert!(state.xp >= 1.0);
    assert!(state.xp < 4.0);
    assert!(state.vitals.fed > 40.0);
    assert_eq!(state.last_usage_poll_at, Some(now));
    assert_eq!(state.last_updated_at, now);
    // PetState::new_for_test defaults to Species::Fuzz; S1=fuzzling, S2=kit.
    for label in ["fuzzling", "kit", "pup"] {
        let expected_text = format!("mochi evolved into {label}");
        assert_eq!(
            state
                .recent_events
                .iter()
                .filter(|event| event.text.contains(&expected_text))
                .count(),
            1,
            "expected '{expected_text}' recorded once",
        );
    }
    assert_eq!(
        state.seen_stage_transitions,
        vec![Stage::S1, Stage::S2, Stage::S3]
    );
}

#[test]
fn no_delta_poll_applies_rhythm_decay_without_granting_xp() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.vitals = Vitals { fed: 70.0, happiness: 70.0, energy: 70.0 };
    state.last_usage_poll_at = Some(datetime!(2026 - 05 - 09 09:00 UTC));
    let now = datetime!(2026 - 05 - 09 13:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(state.xp, 0.0);
    assert_eq!(state.lifetime_effective_tokens, 0.0);
    assert!(state.vitals.fed < 70.0);
    assert!(state.vitals.fed > 35.0);
    assert_eq!(state.last_usage_poll_at, Some(now));
    assert_eq!(state.last_updated_at, now);
}

#[test]
fn rapid_token_polls_do_not_narrate_every_feed() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    let start = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", start);

    for tick in 0..6 {
        let now = start + Duration::seconds(tick * 10);
        let poll = poll_with_delta(2_000.0, now);
        apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();
    }

    let eating_narrations = state
        .recent_events
        .iter()
        .filter(|event| is_eating_narration(&event.text, "mochi"))
        .count();

    assert!(
        eating_narrations <= 1,
        "rapid token polls should not narrate every feed, got {eating_narrations}: {:?}",
        state
            .recent_events
            .iter()
            .map(|event| &event.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn apply_reconciles_saved_stage_when_xp_outranks_it() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    // Simulate state from before a threshold change: xp passes the new s1
    // threshold (0.125) but stage was last saved as "s0".
    state.xp = 0.30;
    state.stage = Stage::S0;
    state.seen_stage_transitions = Vec::new();

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(state.stage, Stage::S1);
    assert_eq!(state.seen_stage_transitions, vec![Stage::S1]);
}

#[test]
fn runtime_compacts_old_usage_events_after_poll() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    usage_store
        .insert_event(&NormalizedUsageEvent::for_test_at(
            now - Duration::days(91),
            1000.0,
        ))
        .unwrap();
    usage_store
        .insert_event(&NormalizedUsageEvent::for_test_at(now, 250.0))
        .unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(usage_store.recent_event_count().unwrap(), 1);
    assert_eq!(
        usage_store
            .daily_aggregate_effective_tokens("claude-code")
            .unwrap(),
        1000.0
    );
}

#[test]
fn staged_usage_apportions_token_buckets_across_smear_rows() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);
    let mut poll = poll_with_delta(12_000.0, now);
    poll.deltas[0].token_totals = Some(RawTokenTotals {
        uncached_input: 6_000,
        output: 3_000,
        cache_creation: 2_000,
        cache_read: 1_000,
        reasoning_output: 500,
    });

    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();
    assert!(
        ids.len() > 1,
        "test expects catchup smear to create multiple rows"
    );

    let rows = usage_store.unapplied_events(100).unwrap();
    let input_sum: f64 = rows.iter().map(|row| row.event.input_tokens).sum();
    let output_sum: f64 = rows.iter().map(|row| row.event.output_tokens).sum();
    let cache_creation_sum: f64 = rows.iter().map(|row| row.event.cache_creation_tokens).sum();
    let cache_read_sum: f64 = rows.iter().map(|row| row.event.cache_read_tokens).sum();
    let reasoning_sum: f64 = rows
        .iter()
        .map(|row| row.event.reasoning_output_tokens)
        .sum();

    assert!((input_sum - 6_000.0).abs() < 0.01);
    assert!((output_sum - 3_000.0).abs() < 0.01);
    assert!((cache_creation_sum - 2_000.0).abs() < 0.01);
    assert!((cache_read_sum - 1_000.0).abs() < 0.01);
    assert!((reasoning_sum - 500.0).abs() < 0.01);
}

#[test]
fn smear_buckets_do_not_change_lifecycle_xp_for_one_apply_window() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 800_000.0;
    let poll = poll_with_delta(800_000.0, now);

    let direct = glorp::game::evolution::apply_xp_delta(0.0, 800_000.0, state.calibration).xp;
    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();

    assert_eq!(direct, 1.0);
    assert_eq!(state.xp, direct);
    assert_eq!(state.stage, Stage::S3);
}

#[test]
fn runtime_feeds_cached_tokens_at_full_value_for_tokenmaxxing_deltas() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 1_000_000_000.0;
    establish_provider_contact(
        &mut usage_store,
        "codex",
        datetime!(2026 - 06 - 18 20:00 UTC),
    );

    let now = datetime!(2026 - 06 - 18 20:10 UTC);
    let poll = UsagePollResult {
        deltas: vec![UsageDelta {
            provider_surface: "codex".into(),
            source_identity: SourceIdentity::from_tokenmaxxing_source("codex"),
            command: glorp::usage::agentsview::AGENTSVIEW_COMMAND.into(),
            effective_tokens: 21_006_000.0,
            total_tokens: 700_000_000.0,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            confidence: "local-log-derived".into(),
            period_start: datetime!(2026 - 06 - 18 07:00 UTC),
            observed_at: now,
            model: Some("gpt-5.5".into()),
            cursor_update: ProviderCursorUpdate {
                provider_surface: "codex".into(),
                cursor_key: provider_cursor_key_for_test("codex"),
                cursor_value: "v1".into(),
                provider_version: "agentsview v0.32.1".into(),
                parser_version: "agentsview v0.32.1".into(),
            },
            token_totals: Some(RawTokenTotals {
                uncached_input: 1_000,
                output: 2_000,
                cache_creation: 3_000,
                cache_read: 699_994_000,
                reasoning_output: 123_456,
            }),
        }],
        diagnostics: Vec::new(),
        total_effective_tokens: 21_006_000.0,
        total_tokens: 700_000_000.0,
    };

    let update = apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();

    assert_eq!(update.recent_effective_tokens, 700_000_000.0);
    assert_eq!(state.lifetime_effective_tokens, 700_000_000.0);
    let rows = usage_store.recent_events(20).unwrap();
    assert_eq!(
        rows.iter().map(|row| row.total_tokens).sum::<f64>(),
        700_000_000.0
    );
    assert!(rows.iter().all(|row| {
        row.token_contract == glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1
            && row.effective_tokens == row.total_tokens
    }));
    let reasoning_sum = rows
        .iter()
        .map(|row| row.reasoning_output_tokens)
        .sum::<f64>();
    assert!((reasoning_sum - 123_456.0).abs() < 0.01);
}

#[test]
fn tokenmaxxing_cutover_seeds_agentsview_cursors_without_feeding_existing_pet() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.xp = 0.25;
    state.lifetime_effective_tokens = 12_345.0;
    state.stage = Stage::S2;
    state.vitals.fed = 44.0;
    state.pet.generated_species = glorp::pet::generation::Species::Mech;
    state.seen_stage_transitions = vec![Stage::S1, Stage::S2];
    state.habitat.reconciled_lifetime_tokens_at = Some(12_345.0);
    state.recent_events.push(NarrativeEvent {
        observed_at: datetime!(2026 - 06 - 18 12:00 UTC),
        text: "existing event".into(),
    });
    let pre_cutover_identity = state.pet.clone();
    let pre_cutover_habitat = state.habitat.clone();
    let pre_cutover_seen_stage_transitions = state.seen_stage_transitions.clone();
    usage_store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: "codex".into(),
                cursor_key: "old-ccusage-cursor".into(),
                cursor_value: "old".into(),
                provider_version: "ccusage-codex".into(),
                parser_version: "ccusage-codex".into(),
            }],
            datetime!(2026 - 06 - 18 12:00 UTC),
        )
        .unwrap();

    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/helpers/agentsview-ok.mjs"),
            ),
        },
    );

    let outcome = glorp::usage::cutover::ensure_tokenmaxxing_contract_active(
        &mut state,
        &mut usage_store,
        &provider,
        datetime!(2026 - 06 - 18 20:00 UTC),
    )
    .unwrap();

    assert!(outcome.activated);
    assert_eq!(state.xp, 0.25);
    assert_eq!(state.lifetime_effective_tokens, 12_345.0);
    assert_eq!(state.stage, Stage::S2);
    assert_eq!(state.vitals.fed, 44.0);
    assert_eq!(state.pet, pre_cutover_identity);
    assert_eq!(state.habitat, pre_cutover_habitat);
    assert_eq!(
        state.seen_stage_transitions,
        pre_cutover_seen_stage_transitions
    );
    assert_eq!(state.recent_events.last().unwrap().text, "existing event");
    assert!(usage_store
        .is_token_contract_active(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1)
        .unwrap());

    let after_cutover_poll = provider.poll(&mut usage_store).unwrap();
    assert_eq!(after_cutover_poll.total_tokens, 0.0);
}

#[test]
fn tokenmaxxing_cutover_missing_agentsview_does_not_activate_contract() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(dir.path().join("missing-agentsview")),
        },
    );

    let outcome = glorp::usage::cutover::ensure_tokenmaxxing_contract_active(
        &mut state,
        &mut usage_store,
        &provider,
        datetime!(2026 - 06 - 18 20:00 UTC),
    )
    .unwrap();

    assert!(!outcome.activated);
    assert!(!usage_store
        .is_token_contract_active(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1)
        .unwrap());
}

// Regression test: cutover with a snapshot that has diagnostics must still advance cursors.
// Before the fix, any benign diagnostic (e.g. a malformed model breakdown alongside valid
// records) caused ensure_tokenmaxxing_contract_active to early-return without advancing
// cursors, so the next poll saw a zero-cursor diff and applied the full usage history as a
// bolus — rocketing the pet to S6.
#[test]
fn tokenmaxxing_cutover_advances_cursors_when_snapshot_has_benign_diagnostic() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.xp = 0.25;
    state.stage = Stage::S2;
    state.vitals.fed = 44.0;

    // Fixture returns valid daily records plus one malformed model breakdown that
    // triggers a diagnostic but leaves cursor_updates/daily_usage populated.
    let provider = glorp::usage::agentsview::AgentsviewCommandProvider::new(
        glorp::usage::agentsview::AgentsviewPaths {
            agentsview: Some(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/helpers/agentsview-data-with-diagnostic.mjs"),
            ),
        },
    );

    let outcome = glorp::usage::cutover::ensure_tokenmaxxing_contract_active(
        &mut state,
        &mut usage_store,
        &provider,
        datetime!(2026 - 06 - 18 20:00 UTC),
    )
    .unwrap();

    // Contract is NOT activated (diagnostics present), but cursors MUST be advanced.
    assert!(!outcome.activated);
    assert!(!usage_store
        .is_token_contract_active(glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1)
        .unwrap());

    // The critical assertion: a subsequent poll must yield ~0 new tokens because the
    // cursors were advanced to the current totals during cutover. Without the fix,
    // this poll would return the full historical usage as a bolus.
    let after_cutover_poll = provider.poll(&mut usage_store).unwrap();
    assert_eq!(
        after_cutover_poll.total_tokens, 0.0,
        "cursors must be advanced even when diagnostics are present; \
        a non-zero poll means the full history would be applied as a bolus"
    );

    // Pet state must be unchanged.
    assert_eq!(state.xp, 0.25);
    assert_eq!(state.stage, Stage::S2);
    assert_eq!(state.vitals.fed, 44.0);
}

fn poll_with_delta(effective_tokens: f64, now: time::OffsetDateTime) -> UsagePollResult {
    // Each call yields a distinct cursor_value so the unapplied ledger's idempotency
    // (keyed on provider_surface|cursor_key|cursor_value) treats each call as a new poll.
    let counter = POLL_COUNTER.fetch_add(1, Ordering::Relaxed);
    UsagePollResult {
        deltas: vec![UsageDelta {
            provider_surface: "claude-code".to_string(),
            source_identity: SourceIdentity::claude_code(),
            command: "ccusage".to_string(),
            effective_tokens,
            total_tokens: effective_tokens,
            token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
            confidence: "local-log-derived".to_string(),
            period_start: now,
            observed_at: now,
            model: Some("test-model".to_string()),
            cursor_update: ProviderCursorUpdate {
                provider_surface: "claude-code".to_string(),
                cursor_key: provider_cursor_key_for_test("claude-code"),
                cursor_value: format!(
                    r#"{{"uncached_input":{},"output":0,"cache_creation":0,"cache_read":0,"reasoning_output":0,"_counter":{}}}"#,
                    effective_tokens as u64, counter
                ),
                provider_version: "test-provider".to_string(),
                parser_version: "test-parser".to_string(),
            },
            token_totals: None,
        }],
        diagnostics: Vec::new(),
        total_effective_tokens: effective_tokens,
        total_tokens: effective_tokens,
    }
}

fn usage_delta_with_cursor(
    provider_surface: &str,
    cursor_key: &str,
    cursor_value: &str,
    total_tokens: f64,
    now: time::OffsetDateTime,
) -> UsageDelta {
    let sequence = POLL_COUNTER.fetch_add(1, Ordering::Relaxed);
    UsageDelta {
        provider_surface: provider_surface.into(),
        source_identity: SourceIdentity::from_provider_surface(provider_surface),
        command: "ccusage".into(),
        effective_tokens: total_tokens,
        total_tokens,
        token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
        confidence: "local-log-derived".into(),
        period_start: now + Duration::seconds(sequence as i64),
        observed_at: now,
        model: Some("claude-sonnet-4".into()),
        cursor_update: ProviderCursorUpdate {
            provider_surface: provider_surface.into(),
            cursor_key: cursor_key.into(),
            cursor_value: cursor_value.into(),
            provider_version: "test-provider".into(),
            parser_version: "test-parser".into(),
        },
        token_totals: Some(RawTokenTotals {
            uncached_input: total_tokens as u64,
            output: 0,
            cache_creation: 0,
            cache_read: 0,
            reasoning_output: 0,
        }),
    }
}

fn provider_cursor_key_for_test(surface: &str) -> String {
    format!("{surface}-cursor")
}

fn habitat_prop_ids(state: &PetState) -> Vec<&str> {
    state
        .habitat
        .earned_props
        .iter()
        .map(|prop| prop.id.as_str())
        .collect()
}

fn poll_with_surface(
    provider_surface: &str,
    effective_tokens: f64,
    now: time::OffsetDateTime,
) -> UsagePollResult {
    let mut poll = poll_with_delta(effective_tokens, now);
    for delta in &mut poll.deltas {
        delta.provider_surface = provider_surface.to_string();
        delta.cursor_update.provider_surface = provider_surface.to_string();
        delta.cursor_update.cursor_key = provider_cursor_key_for_test(provider_surface);
    }
    poll
}

fn empty_poll() -> UsagePollResult {
    UsagePollResult {
        deltas: Vec::new(),
        diagnostics: Vec::new(),
        total_effective_tokens: 0.0,
        total_tokens: 0.0,
    }
}

fn establish_provider_contact(
    usage_store: &mut UsageStore,
    surface: &str,
    now: time::OffsetDateTime,
) {
    usage_store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: surface.to_string(),
                cursor_key: provider_cursor_key_for_test(surface),
                cursor_value: "seeded".to_string(),
                provider_version: "test-provider".to_string(),
                parser_version: "test-parser".to_string(),
            }],
            now,
        )
        .unwrap();
}

fn is_eating_narration(text: &str, pet_name: &str) -> bool {
    text.starts_with(&format!("{pet_name} "))
        && [
            "feasted", "devoured", "munched", "gobbled", "nibbled", "snacked", "sipped", "tasted",
        ]
        .iter()
        .any(|verb| text.contains(verb))
}

#[test]
fn cold_start_does_not_narrate_initial_mood() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    // Precondition: fresh state has no prior mood recorded.
    assert!(state.last_seen_mood.is_none());

    // Apply one poll with tokens so the mood-comparison code path executes.
    let poll = poll_with_delta(1_000.0, now);
    apply_usage_poll(&mut state, &mut usage_store, &poll, now).unwrap();

    // After the first poll, mood should be recorded.
    assert!(
        state.last_seen_mood.is_some(),
        "should have recorded the mood"
    );

    // No mood narration should have fired because there was no prior mood to
    // transition *from*.
    let mood_narrations: Vec<_> = state
        .recent_events
        .iter()
        .filter(|e| {
            e.text.contains("brightened")
                || e.text.contains("settled")
                || e.text.contains("drowsy")
                || e.text.contains("hungry")
                || e.text.contains("slumped")
                || e.text.contains("faded")
                || e.text.contains("yawned")
                || e.text.contains("peckish")
                || e.text.contains("looks down")
                || e.text.contains("dimmed")
                || e.text.contains("looks great")
                || e.text.contains("relaxed")
        })
        .collect();
    assert!(
        mood_narrations.is_empty(),
        "cold start should not produce mood narration, got: {:?}",
        mood_narrations.iter().map(|e| &e.text).collect::<Vec<_>>()
    );
}

#[test]
fn catchup_application_records_each_stage_transition_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;

    for day in 0..60 {
        let observed_at = now + Duration::days(day);
        let buckets = glorp::game::catchup::smear_catchup_delta(100_000.0, state.calibration);
        let bucket_count = buckets.len();
        for (bucket_index, effective_tokens) in buckets.into_iter().enumerate() {
            let bucket_at = observed_at
                - Duration::minutes(bucket_count.saturating_sub(bucket_index + 1) as i64 * 10);
            usage_store
                .insert_unapplied_event_bucket(
                    &NormalizedUsageEvent {
                        observed_at,
                        bucket_at,
                        effective_tokens,
                        ..NormalizedUsageEvent::for_test_at(observed_at, effective_tokens)
                    },
                    &ProviderCursorUpdate {
                        provider_surface: "claude-code".into(),
                        cursor_key: format!("cursor-{day}"),
                        cursor_value: format!("value-{day}"),
                        provider_version: "test-provider".into(),
                        parser_version: "test-parser".into(),
                    },
                    bucket_index,
                    bucket_count,
                )
                .unwrap();
        }
    }

    let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();

    assert_eq!(state.stage, Stage::S3);
    assert_eq!(state.seen_stage_transitions.len(), 3);
    assert_eq!(
        state.seen_stage_transitions,
        vec![Stage::S1, Stage::S2, Stage::S3]
    );
}

#[test]
fn unapplied_usage_survives_state_save_failure_and_applies_once_next_run() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let event = NormalizedUsageEvent {
        observed_at: now,
        bucket_at: now,
        ..NormalizedUsageEvent::for_test_at(now, 100_000.0)
    };
    let cursor = ProviderCursorUpdate {
        provider_surface: "claude-code".into(),
        cursor_key: "test-cursor".into(),
        cursor_value: r#"{"uncached_input":100000,"output":0,"cache_creation":0,"cache_read":0,"reasoning_output":0}"#.into(),
        provider_version: "test-provider".into(),
        parser_version: "test-parser".into(),
    };
    let inserted_id = usage_store
        .insert_unapplied_event_bucket(&event, &cursor, 0, 1)
        .unwrap();

    let mut failed_state = PetState::new_for_test("mochi-7f3a", "mochi");
    failed_state.calibration.daily_effective_tokens = 100_000.0;
    let failed_update =
        apply_unapplied_usage(&mut failed_state, &mut usage_store, now, false).unwrap();
    assert_eq!(failed_update.applied_event_ids, vec![inserted_id]);

    let mut retried_state = PetState::new_for_test("mochi-7f3a", "mochi");
    retried_state.calibration.daily_effective_tokens = 100_000.0;
    let retry_update =
        apply_unapplied_usage(&mut retried_state, &mut usage_store, now, false).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&retry_update.applied_event_ids, now)
        .unwrap();

    assert_eq!(retried_state.lifetime_effective_tokens, 100_000.0);
    assert_eq!(usage_store.unapplied_events(10).unwrap().len(), 0);
    assert_eq!(
        usage_store
            .provider_cursor("claude-code", "test-cursor")
            .unwrap()
            .unwrap(),
        cursor.cursor_value
    );
}

#[test]
fn fresh_pet_state_starts_with_empty_habitat_state() {
    let state = PetState::new_for_test("mochi-7f3a", "mochi");

    assert!(state.habitat.earned_props.is_empty());
    assert_eq!(state.habitat.reconciled_lifetime_tokens_at, None);
}

#[test]
fn habitat_catalog_exposes_v1_prop_ids_and_kinds() {
    use glorp::game::habitat::{catalog_prop, HabitatPropKind, HabitatPropZone};
    use glorp::storage::state::HabitatPropId;

    let codex = catalog_prop(&HabitatPropId::new("codex_signal_lamp")).unwrap();
    assert_eq!(codex.kind, HabitatPropKind::Trophy);
    assert_eq!(codex.display_priority, 70);

    let pebble = catalog_prop(&HabitatPropId::new("token_pebble_25k")).unwrap();
    assert_eq!(pebble.kind, HabitatPropKind::Accent);
    // Front-loaded to 10k to give young pets honest early character (was 25k).
    assert_eq!(pebble.lifetime_threshold, Some(10_000.0));

    for (id, zone) in [
        ("token_moss_tuft_250k", HabitatPropZone::FloorMid),
        ("token_friendly_cloud_750k", HabitatPropZone::AirMid),
        ("token_treasure_chest_2m", HabitatPropZone::FloorMid),
        ("token_hanging_vine_25m", HabitatPropZone::Ceiling),
        ("token_reeds_5m", HabitatPropZone::FloorRight),
    ] {
        let prop = catalog_prop(&HabitatPropId::new(id)).unwrap();
        assert_eq!(prop.kind, HabitatPropKind::Trophy);
        assert_eq!(prop.zone, zone);
        assert!(prop.lifetime_threshold.is_some());
    }

    assert!(catalog_prop(&HabitatPropId::new("non_catalog_prop_for_filter_test")).is_none());
}

#[test]
fn lifetime_threshold_unlocks_one_ladder_prop_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(25_000.0, now),
        now,
    )
    .unwrap();
    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &empty_poll(),
        now + Duration::minutes(10),
    )
    .unwrap();

    let ids = habitat_prop_ids(&state);
    assert_eq!(ids, vec!["token_pebble_25k"]);
}

#[test]
fn one_large_poll_unlocks_ladder_props_in_threshold_order() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 10_000_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(1_100_000.0, now),
        now,
    )
    .unwrap();

    assert_eq!(
        habitat_prop_ids(&state),
        vec![
            "token_pebble_25k",
            "token_shell_100k",
            "token_moss_tuft_250k",
            "token_spark_500k",
            "token_friendly_cloud_750k",
            "token_shard_1m",
        ]
    );
}

#[test]
fn existing_lifetime_counter_reconciles_ladder_props_without_usage_delta() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.lifetime_effective_tokens = 125_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);

    apply_usage_poll(&mut state, &mut usage_store, &empty_poll(), now).unwrap();

    assert_eq!(
        habitat_prop_ids(&state),
        vec!["token_pebble_25k", "token_shell_100k"]
    );
    assert_eq!(state.habitat.reconciled_lifetime_tokens_at, Some(125_000.0));
}

#[test]
fn reflected_unapplied_usage_does_not_unlock_ladder_twice_on_mark_retry() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 1_000_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    stage_usage_poll_deltas(
        &mut usage_store,
        &poll_with_delta(60_000.0, now),
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();
    let first_update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();

    assert_eq!(state.lifetime_effective_tokens, 60_000.0);
    assert_eq!(habitat_prop_ids(&state), vec!["token_pebble_25k"]);

    let retry_update = apply_unapplied_usage(
        &mut state,
        &mut usage_store,
        now + Duration::minutes(1),
        false,
    )
    .unwrap();

    assert_eq!(
        retry_update.applied_event_ids,
        first_update.applied_event_ids
    );
    assert_eq!(state.lifetime_effective_tokens, 60_000.0);
    assert_eq!(habitat_prop_ids(&state), vec!["token_pebble_25k"]);
}

#[test]
fn first_codex_usage_unlocks_signal_lamp_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "codex", now);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_surface("codex", 1_000.0, now),
        now,
    )
    .unwrap();
    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_surface("codex", 1_000.0, now + Duration::minutes(10)),
        now + Duration::minutes(10),
    )
    .unwrap();

    let lamp_count = state
        .habitat
        .earned_props
        .iter()
        .filter(|prop| prop.id.as_str() == "codex_signal_lamp")
        .count();
    assert_eq!(lamp_count, 1);
}

#[test]
fn heavy_session_unlocks_planter_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(49_999.0, now),
        now,
    )
    .unwrap();
    assert!(!habitat_prop_ids(&state).contains(&"heavy_session_planter"));

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(50_000.0, now + Duration::minutes(10)),
        now + Duration::minutes(10),
    )
    .unwrap();

    assert!(habitat_prop_ids(&state).contains(&"heavy_session_planter"));
}

#[test]
fn wilted_recovery_unlocks_sprout_once() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    state.vitals = Vitals { fed: 2.0, happiness: 2.0, energy: 2.0 };
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now);

    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_delta(100_000.0, now),
        now,
    )
    .unwrap();

    assert!(habitat_prop_ids(&state).contains(&"wilt_recovery_sprout"));
}

#[test]
fn discontinuity_bolus_is_refused_alone_while_honest_sibling_feeds() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 19_770_000.0;
    let now = datetime!(2026 - 06 - 10 08:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now - Duration::hours(1));
    establish_provider_contact(&mut usage_store, "codex", now - Duration::hours(1));

    let mut poll = poll_with_delta(212_000_000.0, now);
    poll.deltas
        .extend(poll_with_surface("codex", 40_000.0, now).deltas);
    poll.total_effective_tokens = 212_040_000.0;

    let ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    let rows = usage_store.unapplied_events(100).unwrap();
    assert!(!rows.is_empty(), "the honest codex delta must stage");
    assert!(
        rows.iter().all(|row| row.event.provider_surface == "codex"),
        "no claude-code row may stage: {rows:?}"
    );
    assert_eq!(ids.len(), rows.len());
    let staged: f64 = rows.iter().map(|row| row.event.effective_tokens).sum();
    assert!((staged - 40_000.0).abs() < 0.01);

    assert_eq!(
        usage_store.latest_cursor_updated_at("claude-code").unwrap(),
        Some(now)
    );
    let diagnostics = usage_store.recent_diagnostics(5).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].provider_surface, "claude-code");
    assert_eq!(diagnostics[0].code, "usage_discontinuity");

    assert_eq!(
        state
            .recent_events
            .iter()
            .filter(|event| event.text == "mochi declined an implausible feast")
            .count(),
        1
    );
    assert_eq!(state.last_idle_narration_at, Some(now));

    let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();
    assert_eq!(
        usage_store.latest_cursor_updated_at("codex").unwrap(),
        Some(now)
    );
}

#[test]
fn multi_day_vacation_catchup_passes_the_guard_via_days_factor() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 20_000_000.0;
    let now = datetime!(2026 - 06 - 10 08:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now - Duration::days(6));

    let poll = poll_with_delta(120_000_000.0, now);
    stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    let rows = usage_store.unapplied_events(200).unwrap();
    let staged: f64 = rows.iter().map(|row| row.event.effective_tokens).sum();
    // Smear cap: 12 buckets × (20M × 0.25) = 60M maximum staged.
    assert!((staged - 60_000_000.0).abs() < 0.01);
    assert!(usage_store.recent_diagnostics(5).unwrap().is_empty());
    assert!(state.recent_events.is_empty(), "no refusal narration");
    assert_eq!(state.last_idle_narration_at, None);
}

#[test]
fn first_contact_provider_seeds_history_without_staging_history() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 06 - 10 08:00 UTC);

    let poll = poll_with_delta(1_000.0, now);
    let ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    assert!(ids.is_empty());
    assert_eq!(usage_store.unapplied_events(10).unwrap().len(), 0);
    assert_eq!(
        usage_store.latest_cursor_updated_at("claude-code").unwrap(),
        Some(now)
    );
    let diagnostics = usage_store.recent_diagnostics(5).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "source_first_contact");

    let next = now + Duration::minutes(10);
    let staged_ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll_with_delta(1_000.0, next),
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        next,
    )
    .unwrap();
    assert!(!staged_ids.is_empty());
}

#[test]
fn first_contact_is_per_cursor_key_not_entire_surface() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 800_000.0;

    usage_store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: "claude-code".into(),
                cursor_key: "known-key".into(),
                cursor_value: "old-known".into(),
                provider_version: "test-provider".into(),
                parser_version: "test-parser".into(),
            }],
            now - Duration::minutes(10),
        )
        .unwrap();

    let mut known =
        usage_delta_with_cursor("claude-code", "known-key", "new-known", 100_000.0, now);
    known.period_start = now;
    let mut missing = usage_delta_with_cursor(
        "claude-code",
        "missing-key",
        "seeded-missing",
        700_000.0,
        now,
    );
    missing.period_start = now - Duration::days(1);

    let poll = UsagePollResult {
        deltas: vec![known, missing],
        diagnostics: Vec::new(),
        total_effective_tokens: 800_000.0,
        total_tokens: 800_000.0,
    };

    stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();
    let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();

    assert_eq!(state.lifetime_effective_tokens, 100_000.0);
    assert_eq!(state.stage, Stage::S1);
    assert_eq!(
        usage_store
            .provider_cursor("claude-code", "missing-key")
            .unwrap()
            .as_deref(),
        Some("seeded-missing")
    );
    assert!(usage_store
        .recent_diagnostics(5)
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic.code == glorp::game::runtime::SOURCE_FIRST_CONTACT_CODE));
}

#[test]
fn guard_floor_passes_heavy_honest_days_over_a_low_median_baseline() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 2_000_000.0;
    let now = datetime!(2026 - 06 - 10 08:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now - Duration::hours(2));

    let poll = poll_with_delta(30_000_000.0, now);
    stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    let staged: f64 = usage_store
        .unapplied_events(200)
        .unwrap()
        .iter()
        .map(|row| row.event.effective_tokens)
        .sum();
    // Smear cap: 12 buckets × (2M × 0.25) = 6M maximum staged.
    assert!((staged - 6_000_000.0).abs() < 0.01);
    assert!(usage_store.recent_diagnostics(5).unwrap().is_empty());
}

#[test]
fn first_ensemble_day_unlocks_from_runtime() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 06 - 11 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 60_000.0;

    for (idx, surface) in ["claude-code", "codex", "gemini"].into_iter().enumerate() {
        let event = NormalizedUsageEvent {
            provider_surface: surface.to_string(),
            observed_at: now,
            bucket_at: now,
            effective_tokens: 20_000.0,
            ..NormalizedUsageEvent::for_test_at(now, 20_000.0)
        };
        usage_store
            .insert_unapplied_event_bucket(
                &event,
                &ProviderCursorUpdate {
                    provider_surface: surface.to_string(),
                    cursor_key: format!("{surface}-cursor"),
                    cursor_value: format!("{surface}-value"),
                    provider_version: "test-provider".to_string(),
                    parser_version: "test-parser".to_string(),
                },
                idx,
                3,
            )
            .unwrap();
    }

    let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();

    assert!(
        habitat_prop_ids(&state).contains(&glorp::game::habitat::FIRST_ENSEMBLE_DAY),
        "three significant sources should award first_ensemble_day"
    );
}

#[test]
fn first_contact_does_not_unlock_activity_milestones() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 06 - 11 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 60_000.0;

    let mut poll = poll_with_surface("claude-code", 20_000.0, now);
    poll.deltas
        .extend(poll_with_surface("codex", 20_000.0, now).deltas);
    poll.deltas
        .extend(poll_with_surface("gemini", 20_000.0, now).deltas);
    poll.total_effective_tokens = 60_000.0;

    let staged = stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();
    assert!(
        staged.is_empty(),
        "first-contact history should seed cursors only"
    );

    let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();
    assert!(update.applied_event_ids.is_empty());
    assert_eq!(state.lifetime_effective_tokens, 0.0);
    assert!(
        !habitat_prop_ids(&state).contains(&glorp::game::habitat::FIRST_ENSEMBLE_DAY),
        "first-contact history must not award activity milestones"
    );
}

#[test]
fn unknown_source_feeds_neutrally_without_milestone() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = datetime!(2026 - 06 - 11 12:00 UTC);
    establish_provider_contact(&mut usage_store, "gemini", now);

    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 20_000.0;
    apply_usage_poll(
        &mut state,
        &mut usage_store,
        &poll_with_surface("gemini", 20_000.0, now),
        now,
    )
    .unwrap();

    assert_eq!(state.lifetime_effective_tokens, 20_000.0);
    assert!(
        !habitat_prop_ids(&state).contains(&glorp::game::habitat::CODEX_SIGNAL_LAMP),
        "unknown source should feed but keep legacy codex milestone isolated"
    );
}
