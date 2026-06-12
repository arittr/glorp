use crate::game::calibration::CalibrationBaseline;
use crate::game::effective_tokens::EffectiveTokenWeights;
use crate::storage::usage_store::AppliedShapeSums;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
