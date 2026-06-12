use crate::game::calibration::CalibrationBaseline;
use crate::game::effective_tokens::EffectiveTokenWeights;
use crate::storage::usage_store::{AppliedShapeSums, UsageStore};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceDiversity {
    SingleLane,
    DualLane,
    Ensemble,
    Quiet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkRhythm {
    Steady,
    Bursty,
    Sporadic,
    Returning,
    Quiet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenShapePersonality {
    CacheHeavy,
    OutputHeavy,
    ReasoningHeavy,
    Balanced,
    UnknownShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeIntensity {
    Quiet,
    Normal,
    Heavy,
    Huge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryPattern {
    Sustained,
    Returned,
    Fading,
    Dormant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityMilestone {
    FirstEnsembleDay,
    ReturnSprout,
    CacheCraft,
    SteadyWeek,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityIdentityProfile {
    pub source_diversity: SourceDiversity,
    pub rhythm: WorkRhythm,
    pub token_shape: TokenShapePersonality,
    pub relative_intensity: RelativeIntensity,
    pub recovery: RecoveryPattern,
    /// Phase E will populate durable milestones; leave empty here.
    pub long_term_milestones: Vec<ActivityMilestone>,
}

impl Default for ActivityIdentityProfile {
    fn default() -> Self {
        Self {
            source_diversity: SourceDiversity::Quiet,
            rhythm: WorkRhythm::Quiet,
            token_shape: TokenShapePersonality::UnknownShape,
            relative_intensity: RelativeIntensity::Quiet,
            recovery: RecoveryPattern::Dormant,
            long_term_milestones: Vec::new(),
        }
    }
}

pub fn derive_source_diversity(per_source: &[(String, f64)]) -> SourceDiversity {
    let total: f64 = per_source.iter().map(|(_, v)| v.max(0.0)).sum();
    if total <= 0.0 || !total.is_finite() {
        return SourceDiversity::Quiet;
    }
    let mut shares: Vec<(String, f64)> = per_source
        .iter()
        .map(|(name, v)| (name.clone(), v.max(0.0) / total))
        .filter(|(_, share)| share.is_finite() && *share > 0.0)
        .collect();
    shares.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    let above = |threshold: f64| shares.iter().filter(|(_, s)| *s >= threshold).count();
    if above(0.10) >= 3 {
        SourceDiversity::Ensemble
    } else if shares.len() >= 2
        && shares[0].1 >= 0.20
        && shares[1].1 >= 0.20
        && shares[0].1 + shares[1].1 >= 0.80
    {
        SourceDiversity::DualLane
    } else if shares.first().is_some_and(|(_, s)| *s >= 0.85) {
        SourceDiversity::SingleLane
    } else {
        SourceDiversity::Quiet
    }
}

pub fn derive_token_shape_personality(shape: AppliedShapeSums) -> TokenShapePersonality {
    let weights = EffectiveTokenWeights::default();
    let weighted_input = shape.input_tokens;
    let weighted_output = shape.output_tokens;
    let weighted_cache_creation = shape.cache_creation_tokens;
    let weighted_cache_read = shape.cache_read_tokens * weights.cache_read_weight;
    let weighted_reasoning = shape.reasoning_output_tokens;
    let total = weighted_input
        + weighted_output
        + weighted_cache_creation
        + weighted_cache_read
        + weighted_reasoning;
    if total <= 0.0 || !total.is_finite() {
        return TokenShapePersonality::UnknownShape;
    }
    let cache = (weighted_cache_creation + weighted_cache_read) / total;
    let output = weighted_output / total;
    let reasoning = weighted_reasoning / total;
    if cache >= 0.55 {
        TokenShapePersonality::CacheHeavy
    } else if output >= 0.45 {
        TokenShapePersonality::OutputHeavy
    } else if reasoning >= 0.30 {
        TokenShapePersonality::ReasoningHeavy
    } else if cache >= 0.20 || output >= 0.20 || reasoning >= 0.10 {
        TokenShapePersonality::Balanced
    } else {
        TokenShapePersonality::UnknownShape
    }
}

pub fn derive_relative_intensity(
    today_tokens: f64,
    baseline: CalibrationBaseline,
) -> RelativeIntensity {
    let daily = baseline.daily_effective_tokens.max(1.0);
    match today_tokens / daily {
        r if r < 0.25 => RelativeIntensity::Quiet,
        r if r <= 1.25 => RelativeIntensity::Normal,
        r if r <= 3.0 => RelativeIntensity::Heavy,
        _ => RelativeIntensity::Huge,
    }
}

const RHYTHM_BURST_DOMINANCE: f64 = 0.70;
const RHYTHM_STEADY_BUCKETS: usize = 4;
const RHYTHM_STEADY_TOP_SHARE: f64 = 0.40;
const RETURNING_IDLE_GAP_MINUTES: i64 = 120;
const SPORADIC_GAP_MINUTES: i64 = 60;
const RECENT_BUCKET_MINUTES: i64 = 30;

pub fn derive_work_rhythm(
    usage_store: &UsageStore,
    window_start: OffsetDateTime,
    window_end: OffsetDateTime,
) -> WorkRhythm {
    let buckets = usage_store
        .applied_bucket_sums_between(window_start, window_end)
        .unwrap_or_default();
    let active: Vec<(OffsetDateTime, f64)> = buckets
        .into_iter()
        .filter(|(_, t)| *t > 0.0 && t.is_finite())
        .collect();
    if active.is_empty() {
        return WorkRhythm::Quiet;
    }
    let total: f64 = active.iter().map(|(_, t)| *t).sum();
    if total <= 0.0 {
        return WorkRhythm::Quiet;
    }

    // Returning: latest bucket is recent and follows a long idle gap.
    let latest = active.last().map(|(at, _)| *at);
    for window in active.windows(2) {
        let gap = window[1].0 - window[0].0;
        if gap.whole_minutes() >= RETURNING_IDLE_GAP_MINUTES
            && latest.is_some_and(|l| l == window[1].0)
        {
            return WorkRhythm::Returning;
        }
    }

    let mut by_tokens = active.clone();
    by_tokens.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    let top_share = by_tokens[0].1 / total;
    let top_two_share: f64 = by_tokens.iter().take(2).map(|(_, t)| *t).sum::<f64>() / total;

    if top_two_share >= RHYTHM_BURST_DOMINANCE {
        return WorkRhythm::Bursty;
    }
    if active.len() >= RHYTHM_STEADY_BUCKETS && top_share < RHYTHM_STEADY_TOP_SHARE {
        return WorkRhythm::Steady;
    }
    if active
        .windows(2)
        .any(|w| (w[1].0 - w[0].0).whole_minutes() >= SPORADIC_GAP_MINUTES)
    {
        return WorkRhythm::Sporadic;
    }
    WorkRhythm::Steady
}

pub fn derive_recovery_pattern(usage_store: &UsageStore, now: OffsetDateTime) -> RecoveryPattern {
    let Some(latest) = usage_store.latest_applied_bucket_at().unwrap_or(None) else {
        return RecoveryPattern::Dormant;
    };
    let recent_cutoff = now - Duration::minutes(RECENT_BUCKET_MINUTES);
    if latest < recent_cutoff {
        let has_history = usage_store
            .applied_effective_tokens_between(now - Duration::days(7), recent_cutoff)
            .unwrap_or(0.0)
            > 0.0;
        return if has_history {
            RecoveryPattern::Fading
        } else {
            RecoveryPattern::Dormant
        };
    }
    let previous = usage_store
        .latest_applied_bucket_at_before(latest)
        .unwrap_or(None);
    let gap = previous.map(|p| latest - p);
    if gap.is_some_and(|g| g.whole_minutes() >= RETURNING_IDLE_GAP_MINUTES) {
        RecoveryPattern::Returned
    } else {
        RecoveryPattern::Sustained
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::usage_store::NormalizedUsageEvent;

    fn insert_applied(store: &mut UsageStore, at: OffsetDateTime, tokens: f64) {
        store
            .insert_event(&NormalizedUsageEvent {
                observed_at: at,
                bucket_at: at,
                ..NormalizedUsageEvent::for_test_at(at, tokens)
            })
            .unwrap();
    }

    #[test]
    fn activity_profile_defaults_are_quiet() {
        let p = ActivityIdentityProfile::default();
        assert_eq!(p.source_diversity, SourceDiversity::Quiet);
        assert_eq!(p.rhythm, WorkRhythm::Quiet);
        assert_eq!(p.token_shape, TokenShapePersonality::UnknownShape);
        assert_eq!(p.relative_intensity, RelativeIntensity::Quiet);
        assert_eq!(p.recovery, RecoveryPattern::Dormant);
        assert!(p.long_term_milestones.is_empty());
    }

    #[test]
    fn source_diversity_classifies_shapes() {
        assert_eq!(derive_source_diversity(&[]), SourceDiversity::Quiet);
        assert_eq!(
            derive_source_diversity(&[("claude".into(), 90.0), ("codex".into(), 10.0)]),
            SourceDiversity::SingleLane
        );
        assert_eq!(
            derive_source_diversity(&[("claude".into(), 50.0), ("codex".into(), 50.0)]),
            SourceDiversity::DualLane
        );
        assert_eq!(
            derive_source_diversity(&[
                ("claude".into(), 40.0),
                ("codex".into(), 40.0),
                ("gemini".into(), 20.0)
            ]),
            SourceDiversity::Ensemble
        );
        assert_eq!(
            derive_source_diversity(&[
                ("claude".into(), 40.0),
                ("codex".into(), 30.0),
                ("gemini".into(), 30.0)
            ]),
            SourceDiversity::Ensemble
        );
        assert_eq!(
            derive_source_diversity(&[
                ("claude".into(), 70.0),
                ("codex".into(), 20.0),
                ("gemini".into(), 10.0)
            ]),
            SourceDiversity::Ensemble
        );
        assert_eq!(
            derive_source_diversity(&[("claude".into(), 60.0), ("codex".into(), 30.0)]),
            SourceDiversity::DualLane
        );
    }

    #[test]
    fn token_shape_and_intensity() {
        let cache_heavy = AppliedShapeSums {
            input_tokens: 10_000.0,
            output_tokens: 10_000.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 1_000_000.0,
            reasoning_output_tokens: 0.0,
            effective_tokens: 50_000.0,
        };
        assert_eq!(
            derive_token_shape_personality(cache_heavy),
            TokenShapePersonality::CacheHeavy
        );

        let output_heavy = AppliedShapeSums {
            input_tokens: 10_000.0,
            output_tokens: 60_000.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 0.0,
            reasoning_output_tokens: 0.0,
            effective_tokens: 70_000.0,
        };
        assert_eq!(
            derive_token_shape_personality(output_heavy),
            TokenShapePersonality::OutputHeavy
        );

        let reasoning = AppliedShapeSums {
            input_tokens: 10_000.0,
            output_tokens: 10_000.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 0.0,
            reasoning_output_tokens: 40_000.0,
            effective_tokens: 60_000.0,
        };
        assert_eq!(
            derive_token_shape_personality(reasoning),
            TokenShapePersonality::ReasoningHeavy
        );

        assert_eq!(
            derive_token_shape_personality(AppliedShapeSums::default()),
            TokenShapePersonality::UnknownShape
        );

        let baseline = CalibrationBaseline::default();
        assert_eq!(
            derive_relative_intensity(0.0, baseline),
            RelativeIntensity::Quiet
        );
        assert_eq!(
            derive_relative_intensity(50_000.0, baseline),
            RelativeIntensity::Normal
        );
        assert_eq!(
            derive_relative_intensity(150_000.0, baseline),
            RelativeIntensity::Heavy
        );
        assert_eq!(
            derive_relative_intensity(400_000.0, baseline),
            RelativeIntensity::Huge
        );
    }

    #[test]
    fn rhythm_and_recovery() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let today_start = now - Duration::hours(8);

        assert_eq!(
            derive_work_rhythm(&store, today_start, now + Duration::seconds(1)),
            WorkRhythm::Quiet
        );
        assert_eq!(
            derive_recovery_pattern(&store, now),
            RecoveryPattern::Dormant
        );

        // Bursty: all activity in one bucket.
        insert_applied(&mut store, now - Duration::minutes(5), 100_000.0);
        assert_eq!(
            derive_work_rhythm(&store, today_start, now + Duration::seconds(1)),
            WorkRhythm::Bursty
        );
        assert_eq!(
            derive_recovery_pattern(&store, now),
            RecoveryPattern::Sustained
        );

        // Additional buckets keep the day bursty because the first bucket still
        // dominates the share under the plan's constants.
        insert_applied(&mut store, now - Duration::hours(2), 20_000.0);
        insert_applied(&mut store, now - Duration::hours(3), 20_000.0);
        insert_applied(&mut store, now - Duration::hours(4), 20_000.0);
        assert_eq!(
            derive_work_rhythm(&store, today_start, now + Duration::seconds(1)),
            WorkRhythm::Bursty
        );

        // Older activity after a long gap is still classified as bursty because
        // the latest bucket is not the one immediately following the gap.
        insert_applied(&mut store, now - Duration::hours(7), 5_000.0);
        assert_eq!(
            derive_work_rhythm(&store, today_start, now + Duration::seconds(1)),
            WorkRhythm::Bursty
        );
        assert_eq!(
            derive_recovery_pattern(&store, now),
            RecoveryPattern::Sustained
        );
    }
}
