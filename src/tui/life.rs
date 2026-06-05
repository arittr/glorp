use time::{Duration, OffsetDateTime};

use crate::storage::state::HabitatPropId;

const DEFAULT_REFERENCE_TOKENS_PER_MINUTE: f64 = 60_000.0;
const MIN_REFERENCE_TOKENS_PER_MINUTE: f64 = 5_000.0;
const MAX_ACTIVITY_LEVEL: f32 = 2.0;
const MAX_BURST_LEVEL: f32 = 1.5;
const EMA_ALPHA: f64 = 0.35;
const IDLE_DECAY_PER_MINUTE: f32 = 0.82;

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

#[derive(Debug, Clone)]
pub struct LifeSignalState {
    reference_tokens_per_minute: f64,
    ema_activity_level: f32,
    last_observed_at: Option<OffsetDateTime>,
}

impl Default for LifeSignalState {
    fn default() -> Self {
        Self {
            reference_tokens_per_minute: DEFAULT_REFERENCE_TOKENS_PER_MINUTE,
            ema_activity_level: 0.0,
            last_observed_at: None,
        }
    }
}

impl LifeSignalState {
    pub fn observe(&mut self, signal: AppliedUsageSignal, now: OffsetDateTime) -> PetLifeProfile {
        let freshness = self.profile_freshness(signal);
        let elapsed_secs = signal.elapsed_since_successful_poll.whole_seconds().max(1) as f64;
        let tokens_per_minute = signal.applied_effective_tokens.max(0.0) / elapsed_secs * 60.0;
        if freshness == UsageSignalFreshness::Live && tokens_per_minute > 0.0 {
            self.reference_tokens_per_minute = ((1.0 - EMA_ALPHA)
                * self.reference_tokens_per_minute
                + EMA_ALPHA * tokens_per_minute)
                .clamp(
                    MIN_REFERENCE_TOKENS_PER_MINUTE,
                    DEFAULT_REFERENCE_TOKENS_PER_MINUTE * 20.0,
                );
        }

        let target = activity_from_rate(tokens_per_minute, self.reference_tokens_per_minute);
        if freshness == UsageSignalFreshness::Live {
            self.ema_activity_level = ((1.0 - EMA_ALPHA as f32) * self.ema_activity_level
                + EMA_ALPHA as f32 * target)
                .clamp(0.0, MAX_ACTIVITY_LEVEL);
        } else {
            self.ema_activity_level = decay_activity(
                self.ema_activity_level,
                signal.elapsed_since_successful_poll,
            );
        }
        if signal.applied_effective_tokens == 0.0 {
            self.ema_activity_level = decay_activity(
                self.ema_activity_level,
                signal.elapsed_since_successful_poll,
            );
        }
        self.last_observed_at = Some(now);

        let burst_level =
            if freshness == UsageSignalFreshness::Live && signal.applied_effective_tokens > 0.0 {
                activity_from_rate(tokens_per_minute, self.reference_tokens_per_minute)
                    .clamp(0.0, MAX_BURST_LEVEL)
            } else {
                0.0
            };

        PetLifeProfile {
            activity_level: self.ema_activity_level,
            burst_level,
            source_accent: if freshness == UsageSignalFreshness::Live {
                classify_source_accent(signal.source_mix)
            } else {
                None
            },
            work_weather: if freshness == UsageSignalFreshness::Live {
                classify_work_weather(signal.token_shape)
            } else {
                WorkWeather::Clear
            },
            prop_reactions: Vec::new(),
            idle: IdleLifeState {
                idle_minutes: idle_minutes(signal.elapsed_since_successful_poll),
                is_recently_active: signal.applied_effective_tokens > 0.0,
            },
            calm_mode: false,
        }
    }

    fn profile_freshness(&self, signal: AppliedUsageSignal) -> UsageSignalFreshness {
        if self.last_observed_at.is_none() && signal.applied_effective_tokens > 0.0 {
            UsageSignalFreshness::ColdStart
        } else {
            signal.freshness
        }
    }
}

fn activity_from_rate(tokens_per_minute: f64, reference_tokens_per_minute: f64) -> f32 {
    if !tokens_per_minute.is_finite() || tokens_per_minute <= 0.0 {
        return 0.0;
    }
    let reference = reference_tokens_per_minute
        .max(MIN_REFERENCE_TOKENS_PER_MINUTE)
        .max(1.0);
    let ratio = tokens_per_minute / reference;
    let level = (2.0 * ratio / (1.0 + ratio)) as f32;
    if level.is_finite() {
        level.clamp(0.0, MAX_ACTIVITY_LEVEL)
    } else {
        0.0
    }
}

fn decay_activity(current: f32, elapsed: Duration) -> f32 {
    let minutes = (elapsed.whole_seconds().max(0) as f32) / 60.0;
    (current * IDLE_DECAY_PER_MINUTE.powf(minutes)).clamp(0.0, MAX_ACTIVITY_LEVEL)
}

fn idle_minutes(elapsed: Duration) -> u32 {
    elapsed.whole_minutes().max(0) as u32
}

pub fn classify_source_accent(source_mix: Option<AppliedSourceMix>) -> Option<SourceAccent> {
    let mix = source_mix?;
    let total = mix.claude_effective_tokens + mix.codex_effective_tokens;
    if total <= 0.0 || !total.is_finite() {
        return None;
    }
    let claude_share = mix.claude_effective_tokens / total;
    if (0.4..=0.6).contains(&claude_share) {
        Some(SourceAccent::Balanced)
    } else if claude_share > 0.6 {
        Some(SourceAccent::Claude)
    } else {
        Some(SourceAccent::Codex)
    }
}

pub fn classify_work_weather(shape: Option<TokenShapeDelta>) -> WorkWeather {
    let Some(shape) = shape else {
        return WorkWeather::Clear;
    };
    let total = shape.input_tokens
        + shape.output_tokens
        + shape.cache_creation_tokens
        + shape.cache_read_tokens
        + shape.reasoning_output_tokens;
    if total <= 0.0 || !total.is_finite() {
        return WorkWeather::Clear;
    }
    let cache = (shape.cache_creation_tokens + shape.cache_read_tokens) / total;
    let output = shape.output_tokens / total;
    let reasoning = shape.reasoning_output_tokens / total;
    if cache >= 0.55 {
        WorkWeather::CacheMist
    } else if output >= 0.45 {
        WorkWeather::OutputSparks
    } else if reasoning >= 0.30 {
        WorkWeather::ReasoningPulse
    } else if cache >= 0.25 || output >= 0.25 || reasoning >= 0.15 {
        WorkWeather::Mixed
    } else {
        WorkWeather::Clear
    }
}

pub fn build_prop_reactions(
    mut profile: PetLifeProfile,
    earned: &[HabitatPropId],
    compact: bool,
) -> PetLifeProfile {
    let explicit_reactions = profile.prop_reactions.clone();
    let calm_scale = if profile.calm_mode { 0.35 } else { 1.0 };
    let generated_intensity =
        ((profile.activity_level.clamp(0.0, 2.0) / 2.0) * calm_scale).clamp(0.0, 1.0);
    profile.prop_reactions = earned
        .iter()
        .filter_map(|id| {
            if let Some(reaction) = explicit_reactions
                .iter()
                .find(|reaction| reaction.prop_id == *id)
            {
                return Some(normalize_prop_reaction(
                    reaction.clone(),
                    compact,
                    calm_scale,
                ));
            }
            let reaction = match (id.as_str(), profile.source_accent) {
                (
                    crate::game::habitat::CODEX_SIGNAL_LAMP,
                    Some(SourceAccent::Codex | SourceAccent::Balanced),
                ) => Some(PropReactionKind::Glow),
                (crate::game::habitat::TOKEN_SHELL_100K, Some(SourceAccent::Claude)) => {
                    Some(PropReactionKind::Glow)
                }
                (crate::game::habitat::HEAVY_SESSION_PLANTER, _) if profile.burst_level > 0.5 => {
                    Some(PropReactionKind::Bloom)
                }
                _ => None,
            }?;
            let kind = if compact && matches!(reaction, PropReactionKind::Orbit) {
                PropReactionKind::Glow
            } else {
                reaction
            };
            Some(PropReaction {
                prop_id: id.clone(),
                intensity: generated_intensity,
                kind,
            })
        })
        .collect();
    profile
}

fn normalize_prop_reaction(
    mut reaction: PropReaction,
    compact: bool,
    intensity_scale: f32,
) -> PropReaction {
    reaction.intensity = (reaction.intensity.clamp(0.0, 1.0) * intensity_scale).clamp(0.0, 1.0);
    if compact && matches!(reaction.kind, PropReactionKind::Orbit) {
        reaction.kind = PropReactionKind::Glow;
    }
    reaction
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_signal(tokens: f64, elapsed: Duration, now: OffsetDateTime) -> AppliedUsageSignal {
        AppliedUsageSignal {
            applied_effective_tokens: tokens,
            raw_effective_tokens: Some(tokens),
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll: elapsed,
            freshness: UsageSignalFreshness::Live,
        }
    }

    fn source_mix(claude: f64, codex: f64) -> AppliedSourceMix {
        AppliedSourceMix {
            claude_effective_tokens: claude,
            codex_effective_tokens: codex,
        }
    }

    fn token_shape(
        input: f64,
        output: f64,
        cache_creation: f64,
        cache_read: f64,
        reasoning: f64,
    ) -> TokenShapeDelta {
        TokenShapeDelta {
            input_tokens: input,
            output_tokens: output,
            cache_creation_tokens: cache_creation,
            cache_read_tokens: cache_read,
            reasoning_output_tokens: reasoning,
        }
    }

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

    #[test]
    fn life_signal_state_distinguishes_idle_warm_hot_and_cooling() {
        let start = time::macros::datetime!(2026-06-05 12:00 UTC);
        let mut state = LifeSignalState::default();

        let idle = state.observe(
            AppliedUsageSignal::quiet(start, Duration::seconds(10)),
            start,
        );
        assert_eq!(idle.activity_level, 0.0);
        assert_eq!(idle.burst_level, 0.0);

        let warm = state.observe(
            live_signal(
                5_000.0,
                Duration::seconds(10),
                start + Duration::seconds(10),
            ),
            start + Duration::seconds(10),
        );
        let hot = state.observe(
            live_signal(
                80_000.0,
                Duration::seconds(10),
                start + Duration::seconds(20),
            ),
            start + Duration::seconds(20),
        );
        let cooling = state.observe(
            AppliedUsageSignal::quiet(start + Duration::seconds(90), Duration::seconds(70)),
            start + Duration::seconds(90),
        );

        assert!(warm.activity_level > idle.activity_level);
        assert!(hot.activity_level > warm.activity_level);
        assert!(cooling.activity_level < hot.activity_level);
        assert!(hot.burst_level > 0.0);
        assert_eq!(cooling.burst_level, 0.0);
    }

    #[test]
    fn non_live_signal_suppresses_burst_but_not_missing_detail() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let mut state = LifeSignalState::default();
        let cold = AppliedUsageSignal {
            freshness: UsageSignalFreshness::ColdStart,
            source_mix: Some(source_mix(1_000.0, 9_000.0)),
            token_shape: Some(token_shape(100.0, 600.0, 0.0, 0.0, 0.0)),
            ..live_signal(80_000.0, Duration::seconds(10), now)
        };
        let cold_profile = state.observe(cold, now);
        assert_eq!(cold_profile.burst_level, 0.0);
        assert_eq!(cold_profile.source_accent, None);
        assert_eq!(cold_profile.work_weather, WorkWeather::Clear);

        let live_missing_detail =
            live_signal(80_000.0, Duration::seconds(10), now + Duration::seconds(10));
        let live_profile = state.observe(live_missing_detail, now + Duration::seconds(10));
        assert!(live_profile.burst_level > 0.0);
    }

    #[test]
    fn first_session_live_signal_suppresses_burst_and_detail_flare() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let mut state = LifeSignalState::default();
        let profile = state.observe(
            AppliedUsageSignal {
                source_mix: Some(source_mix(1_000.0, 9_000.0)),
                token_shape: Some(token_shape(100.0, 600.0, 0.0, 0.0, 0.0)),
                ..live_signal(80_000.0, Duration::seconds(10), now)
            },
            now,
        );

        assert_eq!(profile.burst_level, 0.0);
        assert_eq!(profile.source_accent, None);
        assert_eq!(profile.work_weather, WorkWeather::Clear);
    }

    #[test]
    fn life_signal_state_clamps_burst_to_profile_contract() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let mut state = LifeSignalState::default();

        state.observe(AppliedUsageSignal::quiet(now, Duration::seconds(10)), now);
        let profile = state.observe(
            live_signal(
                20_000_000.0,
                Duration::seconds(10),
                now + Duration::seconds(10),
            ),
            now + Duration::seconds(10),
        );

        assert_eq!(profile.burst_level, 1.5);
    }

    #[test]
    fn classify_source_accent_covers_source_mix_shapes() {
        assert_eq!(classify_source_accent(None), None);
        assert_eq!(
            classify_source_accent(Some(source_mix(9_000.0, 1_000.0))),
            Some(SourceAccent::Claude)
        );
        assert_eq!(
            classify_source_accent(Some(source_mix(1_000.0, 9_000.0))),
            Some(SourceAccent::Codex)
        );
        assert_eq!(
            classify_source_accent(Some(source_mix(4_000.0, 6_000.0))),
            Some(SourceAccent::Balanced)
        );
        assert_eq!(classify_source_accent(Some(source_mix(0.0, 0.0))), None);
        assert_eq!(
            classify_source_accent(Some(source_mix(f64::INFINITY, 1_000.0))),
            None
        );
    }

    #[test]
    fn classify_work_weather_covers_token_shape_buckets() {
        assert_eq!(classify_work_weather(None), WorkWeather::Clear);
        assert_eq!(
            classify_work_weather(Some(token_shape(0.0, 0.0, 0.0, 0.0, 0.0))),
            WorkWeather::Clear
        );
        assert_eq!(
            classify_work_weather(Some(token_shape(f64::NAN, 0.0, 0.0, 0.0, 0.0))),
            WorkWeather::Clear
        );
        assert_eq!(
            classify_work_weather(Some(token_shape(100.0, 100.0, 500.0, 100.0, 100.0))),
            WorkWeather::CacheMist
        );
        assert_eq!(
            classify_work_weather(Some(token_shape(300.0, 500.0, 0.0, 0.0, 100.0))),
            WorkWeather::OutputSparks
        );
        assert_eq!(
            classify_work_weather(Some(token_shape(500.0, 100.0, 0.0, 0.0, 400.0))),
            WorkWeather::ReasoningPulse
        );
        assert_eq!(
            classify_work_weather(Some(token_shape(500.0, 100.0, 200.0, 100.0, 100.0))),
            WorkWeather::Mixed
        );
        assert_eq!(
            classify_work_weather(Some(token_shape(800.0, 100.0, 50.0, 0.0, 50.0))),
            WorkWeather::Clear
        );
    }

    #[test]
    fn prop_reactions_target_only_earned_visible_props() {
        let earned = vec![
            HabitatPropId::new(crate::game::habitat::CODEX_SIGNAL_LAMP),
            HabitatPropId::new(crate::game::habitat::HEAVY_SESSION_PLANTER),
        ];
        let profile = build_prop_reactions(
            PetLifeProfile {
                activity_level: 1.5,
                burst_level: 1.0,
                source_accent: Some(SourceAccent::Codex),
                ..Default::default()
            },
            &earned,
            true,
        );

        assert!(profile
            .prop_reactions
            .iter()
            .any(|reaction| reaction.prop_id.as_str() == crate::game::habitat::CODEX_SIGNAL_LAMP));
        assert!(!profile
            .prop_reactions
            .iter()
            .any(|reaction| reaction.kind == PropReactionKind::Orbit));
    }

    #[test]
    fn prop_reactions_do_not_invent_unearned_props() {
        let earned = vec![HabitatPropId::new(
            crate::game::habitat::HEAVY_SESSION_PLANTER,
        )];
        let profile = build_prop_reactions(
            PetLifeProfile {
                activity_level: 1.0,
                burst_level: 1.0,
                source_accent: Some(SourceAccent::Codex),
                ..Default::default()
            },
            &earned,
            false,
        );

        assert_eq!(profile.prop_reactions.len(), 1);
        assert_eq!(
            profile.prop_reactions[0].prop_id.as_str(),
            crate::game::habitat::HEAVY_SESSION_PLANTER
        );
    }

    #[test]
    fn prop_reactions_preserve_explicit_earned_profile_reactions() {
        let earned = vec![
            HabitatPropId::new(crate::game::habitat::CODEX_SIGNAL_LAMP),
            HabitatPropId::new(crate::game::habitat::HEAVY_SESSION_PLANTER),
        ];
        let profile = build_prop_reactions(
            PetLifeProfile {
                activity_level: 1.5,
                burst_level: 1.0,
                source_accent: Some(SourceAccent::Codex),
                prop_reactions: vec![PropReaction {
                    prop_id: HabitatPropId::new(crate::game::habitat::CODEX_SIGNAL_LAMP),
                    intensity: 0.9,
                    kind: PropReactionKind::Pulse,
                }],
                ..Default::default()
            },
            &earned,
            false,
        );

        let codex_lamp = profile
            .prop_reactions
            .iter()
            .find(|reaction| reaction.prop_id.as_str() == crate::game::habitat::CODEX_SIGNAL_LAMP)
            .expect("explicit codex lamp reaction should remain");
        assert_eq!(codex_lamp.kind, PropReactionKind::Pulse);
        assert_eq!(codex_lamp.intensity, 0.9);
    }

    #[test]
    fn calm_mode_reduces_prop_reaction_intensity() {
        let earned = vec![HabitatPropId::new(
            crate::game::habitat::HEAVY_SESSION_PLANTER,
        )];
        let profile = build_prop_reactions(
            PetLifeProfile {
                activity_level: 1.8,
                burst_level: 1.2,
                calm_mode: true,
                ..Default::default()
            },
            &earned,
            false,
        );

        assert_eq!(profile.prop_reactions.len(), 1);
        assert!(
            profile.prop_reactions[0].intensity < 0.5,
            "calm mode should keep earned prop reactions visibly quieter"
        );
    }

    #[test]
    fn claude_source_accent_can_react_on_earned_shell() {
        let earned = vec![HabitatPropId::new(crate::game::habitat::TOKEN_SHELL_100K)];
        let profile = build_prop_reactions(
            PetLifeProfile {
                activity_level: 0.8,
                source_accent: Some(SourceAccent::Claude),
                ..Default::default()
            },
            &earned,
            false,
        );

        assert!(profile
            .prop_reactions
            .iter()
            .any(|reaction| reaction.prop_id.as_str() == crate::game::habitat::TOKEN_SHELL_100K));
    }
}
