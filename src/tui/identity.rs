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
}
