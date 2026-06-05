use time::{Duration, OffsetDateTime};

use crate::storage::state::HabitatPropId;

/// Dedupe-applied usage signal used for presentation profile derivation.
/// This represents usage that changed or reflected pet state, not raw provider
/// output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedUsageSignal {
    pub applied_effective_tokens: f64,
    pub raw_effective_tokens: Option<f64>,
    pub source_mix: Option<AppliedSourceMix>,
    pub token_shape: Option<TokenShapeDelta>,
    pub observed_at: OffsetDateTime,
    pub elapsed_since_successful_poll: Duration,
    pub freshness: UsageSignalFreshness,
}

/// Effective-token split for the current app surfaces: Claude Code and Codex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedSourceMix {
    pub claude_effective_tokens: f64,
    pub codex_effective_tokens: f64,
}

/// Applied token-bucket deltas in token units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenShapeDelta {
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_creation_tokens: f64,
    pub cache_read_tokens: f64,
    pub reasoning_output_tokens: f64,
}

/// Freshness classification used to decide whether live visual bursts are
/// appropriate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageSignalFreshness {
    /// Recent applied usage from a normal poll cadence; burst effects may react.
    Live,
    /// First poll or app-session startup without enough live-session context.
    ColdStart,
    /// Historical or delayed catch-up activity; reflect calmly without bursts.
    Backfill,
    /// Diagnostics-only output or no applied pet-state change; keep life quiet.
    DiagnosticsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAccent {
    Claude,
    Codex,
    Balanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkWeather {
    Clear,
    CacheMist,
    OutputSparks,
    ReasoningPulse,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropReactionKind {
    Glow,
    Bloom,
    Pulse,
    Orbit,
}

/// Presentation reaction for an earned habitat prop.
#[derive(Debug, Clone, PartialEq)]
pub struct PropReaction {
    pub prop_id: HabitatPropId,
    /// Normalized intensity clamped by producers to 0.0..=1.0.
    pub intensity: f32,
    pub kind: PropReactionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleLifeState {
    pub idle_minutes: u32,
    pub is_recently_active: bool,
}

/// Presentation-only life profile consumed by watch and menubar renderers.
#[derive(Debug, Clone, PartialEq)]
pub struct PetLifeProfile {
    /// Finite activity level clamped by producers to roughly 0.0..=2.0.
    pub activity_level: f32,
    /// Finite leading-edge burst level clamped by producers to roughly
    /// 0.0..=1.5.
    pub burst_level: f32,
    pub source_accent: Option<SourceAccent>,
    pub work_weather: WorkWeather,
    pub prop_reactions: Vec<PropReaction>,
    pub idle: IdleLifeState,
    pub calm_mode: bool,
}

impl PetLifeProfile {
    pub fn idle() -> Self {
        Self {
            activity_level: 0.0,
            burst_level: 0.0,
            source_accent: None,
            work_weather: WorkWeather::Clear,
            prop_reactions: Vec::new(),
            idle: IdleLifeState {
                idle_minutes: 0,
                is_recently_active: false,
            },
            calm_mode: false,
        }
    }
}

impl Default for PetLifeProfile {
    fn default() -> Self {
        Self::idle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_quiet_and_clear() {
        let profile = PetLifeProfile::default();

        assert_eq!(profile.activity_level, 0.0);
        assert_eq!(profile.burst_level, 0.0);
        assert_eq!(profile.source_accent, None);
        assert_eq!(profile.work_weather, WorkWeather::Clear);
        assert!(profile.prop_reactions.is_empty());
        assert_eq!(profile.idle.idle_minutes, 0);
        assert!(!profile.idle.is_recently_active);
        assert!(!profile.calm_mode);
    }
}
