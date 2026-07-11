use clap::ValueEnum;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompanionReviewOptions {
    pub initial_size: Option<CompanionReviewSize>,
    pub active_pulse: bool,
    pub state: Option<CompanionReviewState>,
    pub duration_ms: Option<u64>,
    pub capture_dir: Option<PathBuf>,
    /// Pins the pet's depth plane for deterministic captures. Never persisted, and
    /// consumed only by Smooth scene preparation.
    pub depth: Option<CompanionReviewDepth>,
    /// Opt-in to capturing the live (unredacted) HUD. Off by default; when on, a
    /// paired capture must land in the sensitive review root. Threaded from the
    /// hidden `--review-capture-live-values` flag; enforcement lives in the
    /// paired-capture coordinator, not the single-renderer capture path.
    pub review_capture_live_values: bool,
}

/// The three depth planes a review capture can pin, normalized onto the raw depth
/// channel's `[-1, 1]` contract. Far is away and small; near is toward the glass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompanionReviewDepth {
    Far,
    Neutral,
    Near,
}

impl CompanionReviewDepth {
    pub const fn normalized(self) -> f32 {
        match self {
            Self::Far => -1.0,
            Self::Neutral => 0.0,
            Self::Near => 1.0,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Far => "far",
            Self::Neutral => "neutral",
            Self::Near => "near",
        }
    }
}

impl CompanionReviewOptions {
    pub const fn resolved_state(&self) -> CompanionReviewState {
        match self.state {
            Some(state) => state,
            None if self.active_pulse => CompanionReviewState::ActivePulse,
            None => CompanionReviewState::Normal,
        }
    }

    pub fn has_review_launch_options(&self) -> bool {
        self.initial_size.is_some()
            || self.active_pulse
            || self.state.is_some()
            || self.duration_ms.is_some()
            || self.capture_dir.is_some()
            || self.depth.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionReviewSize {
    pub width: u16,
    pub height: u16,
}

impl std::str::FromStr for CompanionReviewSize {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((width, height)) = value.split_once('x') else {
            return Err("expected WIDTHxHEIGHT, for example 260x260".to_string());
        };
        let width = width
            .parse::<u16>()
            .map_err(|_| "width must be an integer".to_string())?;
        let height = height
            .parse::<u16>()
            .map_err(|_| "height must be an integer".to_string())?;
        if width < 260 || height < 260 {
            return Err("review size must be at least 260x260".to_string());
        }
        Ok(Self { width, height })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum CompanionReviewState {
    #[default]
    Normal,
    ActivePulse,
    AsleepCalm,
    HelperTrouble,
}

impl CompanionReviewState {
    pub const fn as_str(self) -> &'static str {
        match self {
            CompanionReviewState::Normal => "normal",
            CompanionReviewState::ActivePulse => "active-pulse",
            CompanionReviewState::AsleepCalm => "asleep-calm",
            CompanionReviewState::HelperTrouble => "helper-trouble",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_renderer, CompanionRendererRequest, CompanionRendererTarget,
        CompanionReviewOptions, CompanionReviewSize, CompanionReviewState,
        EffectiveCompanionRenderer, RendererRuntimeState,
    };
    use std::str::FromStr;

    #[test]
    fn review_size_rejects_malformed_values() {
        assert_eq!(
            CompanionReviewSize::from_str("260").unwrap_err(),
            "expected WIDTHxHEIGHT, for example 260x260"
        );
        assert_eq!(
            CompanionReviewSize::from_str("wide x tall").unwrap_err(),
            "width must be an integer"
        );
    }

    #[test]
    fn review_size_rejects_dimensions_below_window_minimum() {
        assert_eq!(
            CompanionReviewSize::from_str("120x120").unwrap_err(),
            "review size must be at least 260x260"
        );
        assert_eq!(
            CompanionReviewSize::from_str("259x400").unwrap_err(),
            "review size must be at least 260x260"
        );
        assert_eq!(
            CompanionReviewSize::from_str("400x259").unwrap_err(),
            "review size must be at least 260x260"
        );
    }

    #[test]
    fn legacy_active_pulse_maps_to_active_pulse_when_state_is_absent() {
        let review = CompanionReviewOptions {
            active_pulse: true,
            ..CompanionReviewOptions::default()
        };

        assert_eq!(review.resolved_state(), CompanionReviewState::ActivePulse);
    }

    #[test]
    fn explicit_review_state_takes_precedence_over_legacy_active_pulse() {
        let review = CompanionReviewOptions {
            active_pulse: true,
            state: Some(CompanionReviewState::AsleepCalm),
            ..CompanionReviewOptions::default()
        };

        assert_eq!(review.resolved_state(), CompanionReviewState::AsleepCalm);
    }

    #[test]
    fn auto_is_the_default_companion_renderer_request() {
        assert_eq!(
            CompanionRendererRequest::default(),
            CompanionRendererRequest::Auto
        );
    }

    #[test]
    fn auto_without_retained_compiled_resolves_to_smooth() {
        assert_eq!(
            resolve_renderer(
                CompanionRendererRequest::Auto,
                CompanionRendererTarget::Other,
                false,
                true,
            ),
            Ok(EffectiveCompanionRenderer::Smooth),
        );
    }

    #[test]
    fn explicit_legacy_requests_resolve_to_their_effective_renderer() {
        for (request, effective) in [
            (
                CompanionRendererRequest::Classic,
                EffectiveCompanionRenderer::Classic,
            ),
            (
                CompanionRendererRequest::Pixel,
                EffectiveCompanionRenderer::Pixel,
            ),
            (
                CompanionRendererRequest::Smooth,
                EffectiveCompanionRenderer::Smooth,
            ),
        ] {
            assert_eq!(
                resolve_renderer(
                    request,
                    CompanionRendererTarget::AppleSiliconMac,
                    true,
                    true
                ),
                Ok(effective),
            );
        }
    }

    #[test]
    fn runtime_state_new_records_request_and_effective_without_fallback() {
        let state = RendererRuntimeState::new(
            CompanionRendererRequest::Smooth,
            EffectiveCompanionRenderer::Smooth,
        );
        assert_eq!(state.requested(), CompanionRendererRequest::Smooth);
        assert_eq!(state.effective(), EffectiveCompanionRenderer::Smooth);
        assert_eq!(state.transition_count(), 0);
        assert_eq!(state.last_fallback_reason(), None);
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn auto_policy_is_architecture_and_capability_aware() {
        assert_eq!(
            resolve_renderer(
                CompanionRendererRequest::Auto,
                CompanionRendererTarget::AppleSiliconMac,
                true,
                false,
            ),
            Ok(EffectiveCompanionRenderer::Smooth),
        );
        assert_eq!(
            resolve_renderer(
                CompanionRendererRequest::Auto,
                CompanionRendererTarget::AppleSiliconMac,
                true,
                true,
            ),
            Ok(EffectiveCompanionRenderer::Retained),
        );
        assert_eq!(
            resolve_renderer(
                CompanionRendererRequest::Auto,
                CompanionRendererTarget::IntelMac,
                false,
                true,
            ),
            Ok(EffectiveCompanionRenderer::Smooth),
        );
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn fallback_preserves_requested_renderer() {
        let mut state = RendererRuntimeState::new(
            CompanionRendererRequest::Retained,
            EffectiveCompanionRenderer::Retained,
        );
        state.fallback_to_smooth("retained-device-lost");
        assert_eq!(state.requested(), CompanionRendererRequest::Retained);
        assert_eq!(state.effective(), EffectiveCompanionRenderer::Smooth);
        assert_eq!(state.transition_count(), 1);
        assert_eq!(state.last_fallback_reason(), Some("retained-device-lost"));
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn explicit_retained_requires_compiled_support() {
        assert_eq!(
            resolve_renderer(
                CompanionRendererRequest::Retained,
                CompanionRendererTarget::AppleSiliconMac,
                true,
                false,
            ),
            Ok(EffectiveCompanionRenderer::Retained),
        );
        assert!(matches!(
            resolve_renderer(
                CompanionRendererRequest::Retained,
                CompanionRendererTarget::AppleSiliconMac,
                false,
                false,
            ),
            Err(super::RendererResolveError::RendererUnavailable(_)),
        ));
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn auto_stays_smooth_while_the_cutover_constant_is_disabled() {
        // Drives Auto with the real cutover constant: if it is ever flipped to
        // true, Apple Silicon resolves to Retained and this assertion fails.
        assert_eq!(
            resolve_renderer(
                CompanionRendererRequest::Auto,
                CompanionRendererTarget::AppleSiliconMac,
                true,
                super::AUTO_RETAINED_ON_APPLE_SILICON,
            ),
            Ok(EffectiveCompanionRenderer::Smooth),
        );
    }
}

/// What the operator asked for on the command line. `Auto` defers the choice to
/// [`resolve_renderer`], which weighs the machine and compiled capabilities.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum CompanionRendererRequest {
    #[default]
    Auto,
    Classic,
    Pixel,
    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    Retained,
    Smooth,
}

impl CompanionRendererRequest {
    /// The `--renderer` value to forward when the `companion` command spawns the
    /// native `companion-app` process. `Auto` forwards nothing so the child
    /// re-resolves the default itself.
    pub const fn forwarded_arg(self) -> Option<&'static str> {
        match self {
            CompanionRendererRequest::Auto => None,
            CompanionRendererRequest::Classic => Some("classic"),
            CompanionRendererRequest::Pixel => Some("pixel"),
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            CompanionRendererRequest::Retained => Some("retained"),
            CompanionRendererRequest::Smooth => Some("smooth"),
        }
    }
}

/// The renderer that actually drives a frame, after `Auto` has been resolved and
/// any unavailable request has been rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveCompanionRenderer {
    Classic,
    Pixel,
    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    Retained,
    Smooth,
}

impl EffectiveCompanionRenderer {
    pub const fn as_str(self) -> &'static str {
        match self {
            EffectiveCompanionRenderer::Classic => "classic",
            EffectiveCompanionRenderer::Pixel => "pixel",
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            EffectiveCompanionRenderer::Retained => "retained",
            EffectiveCompanionRenderer::Smooth => "smooth",
        }
    }

    pub const fn is_pixel(self) -> bool {
        matches!(self, EffectiveCompanionRenderer::Pixel)
    }

    pub const fn is_smooth(self) -> bool {
        matches!(self, EffectiveCompanionRenderer::Smooth)
    }

    pub const fn uses_smooth_scene(self) -> bool {
        match self {
            EffectiveCompanionRenderer::Smooth => true,
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            EffectiveCompanionRenderer::Retained => true,
            EffectiveCompanionRenderer::Classic | EffectiveCompanionRenderer::Pixel => false,
        }
    }

    pub const fn is_retained(self) -> bool {
        #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
        {
            matches!(self, EffectiveCompanionRenderer::Retained)
        }
        #[cfg(not(all(target_os = "macos", feature = "retained-renderer")))]
        {
            false
        }
    }
}

/// The machine class that `Auto` resolution keys off of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionRendererTarget {
    AppleSiliconMac,
    IntelMac,
    Other,
}

impl CompanionRendererTarget {
    /// Resolves the current build's target class from compile-time cfg.
    pub const fn current() -> Self {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            CompanionRendererTarget::AppleSiliconMac
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            CompanionRendererTarget::IntelMac
        } else {
            CompanionRendererTarget::Other
        }
    }
}

/// The single cutover switch. While this stays `false`, `Auto` never resolves to
/// Retained even on capable hardware; flipping it is a later, human-gated step.
pub const AUTO_RETAINED_ON_APPLE_SILICON: bool = false;

/// A request could not be honored. The category is a sanitized `&'static str`, so
/// no dynamic or user-derived text ever reaches an error surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererResolveError {
    RendererUnavailable(&'static str),
}

impl RendererResolveError {
    pub const fn category(self) -> &'static str {
        match self {
            RendererResolveError::RendererUnavailable(category) => category,
        }
    }
}

/// The sole resolver from a CLI request to the renderer that will actually run.
/// `Auto` becomes Retained only on Apple Silicon with Retained compiled in and
/// the cutover enabled; every other `Auto` path is Smooth. Explicit Retained
/// fails with a static category when Retained is not compiled in.
pub fn resolve_renderer(
    request: CompanionRendererRequest,
    target: CompanionRendererTarget,
    retained_compiled: bool,
    auto_retained_enabled: bool,
) -> Result<EffectiveCompanionRenderer, RendererResolveError> {
    match request {
        CompanionRendererRequest::Auto => Ok(resolve_auto_renderer(
            target,
            retained_compiled,
            auto_retained_enabled,
        )),
        CompanionRendererRequest::Classic => Ok(EffectiveCompanionRenderer::Classic),
        CompanionRendererRequest::Pixel => Ok(EffectiveCompanionRenderer::Pixel),
        #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
        CompanionRendererRequest::Retained => {
            if retained_compiled {
                Ok(EffectiveCompanionRenderer::Retained)
            } else {
                Err(RendererResolveError::RendererUnavailable(
                    "retained-renderer-unavailable",
                ))
            }
        }
        CompanionRendererRequest::Smooth => Ok(EffectiveCompanionRenderer::Smooth),
    }
}

fn resolve_auto_renderer(
    target: CompanionRendererTarget,
    retained_compiled: bool,
    auto_retained_enabled: bool,
) -> EffectiveCompanionRenderer {
    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    if matches!(target, CompanionRendererTarget::AppleSiliconMac)
        && retained_compiled
        && auto_retained_enabled
    {
        return EffectiveCompanionRenderer::Retained;
    }
    #[cfg(not(all(target_os = "macos", feature = "retained-renderer")))]
    let _ = (target, retained_compiled, auto_retained_enabled);
    EffectiveCompanionRenderer::Smooth
}

/// Live renderer state for one running companion. The requested renderer is the
/// operator's intent and never changes; only the effective renderer degrades on
/// a runtime fallback, which is counted for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererRuntimeState {
    requested: CompanionRendererRequest,
    effective: EffectiveCompanionRenderer,
    transition_count: u64,
    last_fallback_reason: Option<&'static str>,
}

impl RendererRuntimeState {
    pub fn new(requested: CompanionRendererRequest, effective: EffectiveCompanionRenderer) -> Self {
        Self {
            requested,
            effective,
            transition_count: 0,
            last_fallback_reason: None,
        }
    }

    pub fn requested(&self) -> CompanionRendererRequest {
        self.requested
    }

    pub fn effective(&self) -> EffectiveCompanionRenderer {
        self.effective
    }

    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    pub fn last_fallback_reason(&self) -> Option<&'static str> {
        self.last_fallback_reason
    }

    /// Degrades the effective renderer to Smooth after a runtime failure. The
    /// requested renderer is preserved so intent survives the fallback.
    pub fn fallback_to_smooth(&mut self, reason: &'static str) {
        self.effective = EffectiveCompanionRenderer::Smooth;
        self.transition_count = self.transition_count.saturating_add(1);
        self.last_fallback_reason = Some(reason);
    }
}
