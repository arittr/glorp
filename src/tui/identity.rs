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
}
