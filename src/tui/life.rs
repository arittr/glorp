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

impl AppliedUsageSignal {
    pub fn quiet(now: OffsetDateTime, elapsed_since_successful_poll: Duration) -> Self {
        Self {
            applied_effective_tokens: 0.0,
            raw_effective_tokens: None,
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll,
            freshness: UsageSignalFreshness::Live,
        }
    }

    pub fn diagnostics_only(now: OffsetDateTime, elapsed_since_successful_poll: Duration) -> Self {
        Self {
            freshness: UsageSignalFreshness::DiagnosticsOnly,
            ..Self::quiet(now, elapsed_since_successful_poll)
        }
    }

    pub fn can_burst(self) -> bool {
        self.freshness == UsageSignalFreshness::Live && self.applied_effective_tokens > 0.0
    }
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

    #[test]
    fn missing_detail_does_not_make_live_signal_non_live() {
        let now = OffsetDateTime::UNIX_EPOCH + Duration::hours(1);
        let signal = AppliedUsageSignal {
            applied_effective_tokens: 1_000.0,
            raw_effective_tokens: None,
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll: Duration::minutes(10),
            freshness: UsageSignalFreshness::Live,
        };

        assert_eq!(signal.freshness, UsageSignalFreshness::Live);
        assert!(signal.can_burst());
    }

    #[test]
    fn diagnostics_only_signal_is_non_live() {
        let now = OffsetDateTime::UNIX_EPOCH + Duration::hours(1);
        let signal = AppliedUsageSignal::diagnostics_only(now, Duration::minutes(10));

        assert_eq!(signal.applied_effective_tokens, 0.0);
        assert_eq!(signal.raw_effective_tokens, None);
        assert_eq!(signal.source_mix, None);
        assert_eq!(signal.token_shape, None);
        assert_eq!(signal.observed_at, now);
        assert_eq!(signal.elapsed_since_successful_poll, Duration::minutes(10));
        assert_eq!(signal.freshness, UsageSignalFreshness::DiagnosticsOnly);
        assert!(!signal.can_burst());
    }
}
