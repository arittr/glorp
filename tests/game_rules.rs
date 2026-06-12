use glorp::config::AppConfig;
use glorp::game::calibration::{CalibrationBaseline, DailyUsage};
use glorp::game::catchup::smear_catchup_delta;
use glorp::game::effective_tokens::{EffectiveTokenWeights, TokenBuckets};
use glorp::game::evolution::{apply_xp_delta, stage_for_xp, Stage};
use glorp::game::metabolism::{apply_decay, apply_food, Mood, RhythmProfile, Vitals};
use glorp::storage::state::HabitatPropSource;
use time::macros::{date, datetime};

#[test]
fn effective_tokens_count_cache_reads_lightly() {
    let weights = EffectiveTokenWeights::default();
    let regular = TokenBuckets {
        uncached_input: 1000,
        output: 1000,
        cache_creation: 500,
        cache_read: 0,
        reasoning_output: 0,
    };
    let cache_heavy = TokenBuckets {
        cache_read: 2500,
        ..regular
    };
    assert_eq!(weights.compute(regular), 2500.0);
    assert_eq!(weights.cache_read_weight, 0.03);
    assert!(weights.compute(cache_heavy) < 5000.0);
}

#[test]
fn cache_read_weight_is_configurable() {
    let config = AppConfig {
        cache_read_weight: 0.05,
        ..AppConfig::default()
    };
    let weights = EffectiveTokenWeights::from_config(config);
    let buckets = TokenBuckets {
        uncached_input: 0,
        output: 0,
        cache_creation: 0,
        cache_read: 1000,
        reasoning_output: 999_999,
    };
    assert_eq!(weights.compute(buckets), 50.0);
}

#[test]
fn cost_never_changes_effective_tokens() {
    let weights = EffectiveTokenWeights::default();
    let buckets = TokenBuckets {
        uncached_input: 100,
        output: 200,
        cache_creation: 300,
        cache_read: 1000,
        reasoning_output: 50,
    };
    let with_low_cost = weights.compute_with_display_cost(buckets, Some(0.01));
    let with_high_cost = weights.compute_with_display_cost(buckets, Some(999.99));
    assert_eq!(
        with_low_cost.effective_tokens,
        with_high_cost.effective_tokens
    );
}

#[test]
fn missing_buckets_default_to_zero() {
    let buckets = TokenBuckets::default();
    assert_eq!(EffectiveTokenWeights::default().compute(buckets), 0.0);
}

#[test]
fn historical_usage_calibrates_but_does_not_grant_initial_xp() {
    let history = vec![
        DailyUsage::new(date!(2026 - 04 - 27), 100_000.0),
        DailyUsage::new(date!(2026 - 04 - 28), 120_000.0),
        DailyUsage::new(date!(2026 - 04 - 29), 500_000.0),
    ];
    let baseline = CalibrationBaseline::from_history(&history);
    assert!(baseline.daily_effective_tokens >= 100_000.0);
    assert_eq!(stage_for_xp(0.0), Stage::S0);
}

#[test]
fn calibration_groups_multiple_rows_on_the_same_active_day_before_median() {
    // Without grouping, this is 5 records (>= MIN_ACTIVE_DAYS_FOR_MEDIAN)
    // sorted as [50k, 50k, 200k, 200k, 200k] with median 200_000.
    // With grouping, it is 4 distinct active days (< MIN_ACTIVE_DAYS_FOR_MEDIAN)
    // and the baseline falls back to DEFAULT_DAILY_EFFECTIVE_TOKENS (100_000).
    let history = vec![
        DailyUsage::new(date!(2026 - 05 - 01), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 01), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 02), 200_000.0),
        DailyUsage::new(date!(2026 - 05 - 03), 200_000.0),
        DailyUsage::new(date!(2026 - 05 - 04), 200_000.0),
    ];

    let baseline = CalibrationBaseline::from_history(&history);
    assert_eq!(baseline.daily_effective_tokens, 100_000.0);
}

#[test]
fn low_and_high_usage_users_progress_by_relative_effort() {
    let low = CalibrationBaseline {
        daily_effective_tokens: 50_000.0,
    };
    let high = CalibrationBaseline {
        daily_effective_tokens: 500_000_000.0,
    };
    let low_xp = (0..35).fold(0.0, |xp, _| apply_xp_delta(xp, 50_000.0, low).xp);
    let high_xp = (0..35).fold(0.0, |xp, _| apply_xp_delta(xp, 500_000_000.0, high).xp);
    assert_eq!(stage_for_xp(low_xp), stage_for_xp(high_xp));
}

#[test]
fn extreme_bucket_cannot_skip_most_of_lifecycle() {
    let baseline = CalibrationBaseline {
        daily_effective_tokens: 100_000.0,
    };
    let result = apply_xp_delta(0.0, 100_000_000.0, baseline);
    assert!(result.stage_transitions.len() <= 3);
    assert!(result.mood_food_benefit <= 25.0);
}

#[test]
fn stage_transition_event_is_recorded_once() {
    let baseline = CalibrationBaseline {
        daily_effective_tokens: 100_000.0,
    };
    let before = apply_xp_delta(0.0, 100_000.0, baseline);
    let after = apply_xp_delta(before.xp, 100_000.0, baseline);
    let mut all = before.stage_transitions.clone();
    all.extend(after.stage_transitions.iter().copied());
    let unique_keys: std::collections::HashSet<(usize, usize)> = all
        .iter()
        .map(|t| (t.from as usize, t.to as usize))
        .collect();
    assert_eq!(unique_keys.len(), all.len());
}

#[test]
fn effective_usage_improves_vitals_without_exceeding_caps() {
    let vitals = Vitals {
        fed: 35.0,
        happiness: 40.0,
        energy: 30.0,
    };
    let out = apply_food(vitals, 25_000.0, 100_000.0);
    assert!(out.vitals.fed > vitals.fed);
    assert!(out.vitals.happiness > vitals.happiness);
    assert!(out.vitals.energy > vitals.energy);
    assert!(out.vitals.fed <= 100.0);
    assert!(out.vitals.happiness <= 100.0);
    assert!(out.vitals.energy <= 100.0);
}

#[test]
fn same_day_gap_does_not_jump_to_wilted() {
    let vitals = Vitals {
        fed: 70.0,
        happiness: 70.0,
        energy: 70.0,
    };
    let out = apply_decay(
        vitals,
        datetime!(2026 - 05 - 09 09:00 UTC),
        datetime!(2026 - 05 - 09 13:00 UTC),
        RhythmProfile::default(),
    );
    assert_ne!(out.mood, Mood::Wilted);
    assert!(out.vitals.fed > 35.0);
}

#[test]
fn overnight_and_weekend_decay_is_slower() {
    let vitals = Vitals {
        fed: 70.0,
        happiness: 70.0,
        energy: 70.0,
    };
    let overnight = apply_decay(
        vitals,
        datetime!(2026 - 05 - 08 22:00 UTC),
        datetime!(2026 - 05 - 09 06:00 UTC),
        RhythmProfile::default(),
    );
    let workday = apply_decay(
        vitals,
        datetime!(2026 - 05 - 08 09:00 UTC),
        datetime!(2026 - 05 - 08 17:00 UTC),
        RhythmProfile::default(),
    );
    assert!(overnight.vitals.fed > workday.vitals.fed);
}

#[test]
fn weekend_heavy_profile_learns_weekend_activity() {
    let profile = RhythmProfile::from_active_hours(&[
        datetime!(2026 - 05 - 03 11:00 UTC),
        datetime!(2026 - 05 - 04 12:00 UTC),
        datetime!(2026 - 05 - 10 13:00 UTC),
        datetime!(2026 - 05 - 11 14:00 UTC),
        datetime!(2026 - 05 - 16 15:00 UTC),
    ]);
    assert!(profile.weekend_activity_weight > RhythmProfile::default().weekend_activity_weight);
}

#[test]
fn sparse_active_hours_keep_conservative_defaults() {
    let profile = RhythmProfile::from_active_hours(&[
        datetime!(2026 - 05 - 03 11:00 UTC),
        datetime!(2026 - 05 - 04 12:00 UTC),
        datetime!(2026 - 05 - 10 13:00 UTC),
        datetime!(2026 - 05 - 11 14:00 UTC),
    ]);
    assert_eq!(profile, RhythmProfile::default());
}

#[test]
fn historically_inactive_hours_decay_slowly() {
    let profile = RhythmProfile::from_active_hours(&[
        datetime!(2026 - 05 - 05 09:00 UTC),
        datetime!(2026 - 05 - 06 10:00 UTC),
        datetime!(2026 - 05 - 07 11:00 UTC),
        datetime!(2026 - 05 - 08 09:00 UTC),
        datetime!(2026 - 05 - 12 10:00 UTC),
    ]);
    let vitals = Vitals {
        fed: 70.0,
        happiness: 70.0,
        energy: 70.0,
    };
    let inactive_window = apply_decay(
        vitals,
        datetime!(2026 - 05 - 08 02:00 UTC),
        datetime!(2026 - 05 - 08 05:00 UTC),
        profile,
    );
    let active_window = apply_decay(
        vitals,
        datetime!(2026 - 05 - 08 09:00 UTC),
        datetime!(2026 - 05 - 08 12:00 UTC),
        profile,
    );
    assert!(inactive_window.vitals.fed > active_window.vitals.fed);
}

#[test]
fn sparse_history_keeps_conservative_defaults() {
    let history = vec![
        DailyUsage::new(date!(2026 - 05 - 03), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 04), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 10), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 11), 50_000.0),
    ];
    assert_eq!(
        RhythmProfile::from_history(&history),
        RhythmProfile::default()
    );
}

#[test]
fn untimestamped_history_keeps_default_active_hours_even_when_dense() {
    let history = vec![
        DailyUsage::new(date!(2026 - 05 - 04), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 05), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 06), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 07), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 08), 50_000.0),
    ];
    let profile = RhythmProfile::from_history(&history);
    assert_eq!(profile.active_hours, RhythmProfile::default().active_hours);
}

#[test]
fn timestamped_history_learns_active_and_inactive_hour_rhythm() {
    let history = vec![
        DailyUsage::with_activity_timestamp(datetime!(2026 - 05 - 04 20:00 UTC), 50_000.0),
        DailyUsage::with_activity_timestamp(datetime!(2026 - 05 - 05 21:00 UTC), 50_000.0),
        DailyUsage::with_activity_timestamp(datetime!(2026 - 05 - 06 20:00 UTC), 50_000.0),
        DailyUsage::with_activity_timestamp(datetime!(2026 - 05 - 07 21:00 UTC), 50_000.0),
        DailyUsage::with_activity_timestamp(datetime!(2026 - 05 - 08 20:00 UTC), 50_000.0),
    ];
    let profile = RhythmProfile::from_history(&history);

    assert!(profile.active_hours[20]);
    assert!(profile.active_hours[21]);
    assert!(!profile.active_hours[9]);

    let vitals = Vitals {
        fed: 70.0,
        happiness: 70.0,
        energy: 70.0,
    };
    let learned_active_window = apply_decay(
        vitals,
        datetime!(2026 - 05 - 08 20:00 UTC),
        datetime!(2026 - 05 - 08 22:00 UTC),
        profile,
    );
    let learned_inactive_window = apply_decay(
        vitals,
        datetime!(2026 - 05 - 08 09:00 UTC),
        datetime!(2026 - 05 - 08 11:00 UTC),
        profile,
    );
    assert!(learned_active_window.vitals.fed < learned_inactive_window.vitals.fed);
}

#[test]
fn history_learns_weekend_activity_from_enough_active_days() {
    let history = vec![
        DailyUsage::new(date!(2026 - 05 - 02), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 03), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 04), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 09), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 10), 50_000.0),
        DailyUsage::new(date!(2026 - 05 - 11), 0.0),
    ];
    let profile = RhythmProfile::from_history(&history);
    assert!(profile.weekend_activity_weight > RhythmProfile::default().weekend_activity_weight);
    assert_eq!(profile.active_hours, RhythmProfile::default().active_hours);
}

#[test]
fn sustained_absence_can_wilt_but_real_usage_recovers() {
    let vitals = Vitals {
        fed: 20.0,
        happiness: 25.0,
        energy: 20.0,
    };
    let wilted = apply_decay(
        vitals,
        datetime!(2026 - 05 - 01 09:00 UTC),
        datetime!(2026 - 05 - 09 09:00 UTC),
        RhythmProfile::default(),
    );
    assert_eq!(wilted.mood, Mood::Wilted);
    let recovered = apply_food(wilted.vitals, 50_000.0, 100_000.0);
    assert_ne!(recovered.mood, Mood::Wilted);
}

#[test]
fn there_is_no_death_transition() {
    let vitals = Vitals {
        fed: 0.0,
        happiness: 0.0,
        energy: 0.0,
    };
    let out = apply_decay(
        vitals,
        datetime!(2026 - 01 - 01 00:00 UTC),
        datetime!(2026 - 05 - 09 00:00 UTC),
        RhythmProfile::default(),
    );
    assert_eq!(out.mood, Mood::Wilted);
    assert!(!format!("{out:?}").to_lowercase().contains("death"));
}

#[test]
fn one_active_day_catchup_splits_into_six_to_twelve_buckets() {
    let baseline = CalibrationBaseline {
        daily_effective_tokens: 100_000.0,
    };
    let buckets = smear_catchup_delta(100_000.0, baseline);
    assert!((6..=12).contains(&buckets.len()));
    assert!(buckets.iter().all(|bucket| *bucket <= 25_000.0));
    let xp = buckets
        .iter()
        .fold(0.0, |xp, bucket| apply_xp_delta(xp, *bucket, baseline).xp);
    assert!((0.75..=1.25).contains(&xp));
}

#[test]
fn sixty_daily_catchups_reach_s6_without_duplicate_transition_pressure() {
    let baseline = CalibrationBaseline {
        daily_effective_tokens: 100_000.0,
    };
    let mut xp = 0.0;
    for _ in 0..60 {
        for bucket in smear_catchup_delta(100_000.0, baseline) {
            xp = apply_xp_delta(xp, bucket, baseline).xp;
        }
    }
    assert_eq!(stage_for_xp(xp), Stage::S6);
    assert!((60.0..=66.0).contains(&xp));
}

#[test]
fn activity_milestone_source_round_trips() {
    let source = HabitatPropSource::ActivityMilestone {
        milestone: "first_ensemble_day".into(),
    };
    let json = serde_json::to_string(&source).unwrap();
    let back: HabitatPropSource = serde_json::from_str(&json).unwrap();
    assert_eq!(back, source);
    assert!(json.contains("activity_milestone"));
}
