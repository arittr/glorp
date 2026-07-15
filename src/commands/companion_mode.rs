use clap::ValueEnum;
use std::path::PathBuf;

#[cfg(all(target_os = "macos", feature = "retained-renderer"))]
use crate::companion::retained::{FrameDisposition, RetainedFailureCategory};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompanionReviewOptions {
    pub initial_size: Option<CompanionReviewSize>,
    pub active_pulse: bool,
    pub state: Option<CompanionReviewState>,
    pub duration_ms: Option<u64>,
    pub capture_dir: Option<PathBuf>,
    /// Hidden automation output for a redacted runtime-metrics snapshot. This
    /// never contains pet names, HUD values, helper output, or usage data.
    pub runtime_metrics_out: Option<PathBuf>,
    /// Hidden native qualification request. Runs the exact direct retained scene
    /// through an equal warmup/measured dual-cadence virtual schedule before the
    /// terminal direct capture and metrics write.
    pub review_lifetime_frames: Option<u64>,
    /// Pins the pet's depth plane for deterministic captures. Never persisted, and
    /// consumed only by Smooth scene preparation.
    pub depth: Option<CompanionReviewDepth>,
    /// Opt-in to capturing the live (unredacted) HUD. Off by default; when on, a
    /// paired capture must land in the sensitive review root. Threaded from the
    /// hidden `--review-capture-live-values` flag; enforcement lives in the
    /// paired-capture coordinator, not the single-renderer capture path.
    pub review_capture_live_values: bool,
    /// Forces the resting dim composition onto the frozen live frame. Threaded
    /// from the hidden `--review-force-dim` flag (the xtask `--dimmed` matrix
    /// variant); never persisted.
    pub force_dim_overlay: bool,
    /// Hidden retained-scene rollout override. `None` preserves the shipping
    /// policy: direct Live while the Gate D switch is enabled, or the legacy
    /// translator after the one-line rollback. Explicit `off`, `shadow`, and
    /// `live` values are forwarded to the native companion process for bounded
    /// review only.
    pub retained_scene_runtime: Option<SceneRuntimeRollout>,
    /// Dev/test-only bounded retained fault injection. Threaded from the hidden
    /// `--review-inject-retained-fault` flag, compiled only with the retained
    /// renderer plus dev-preview so it never ships in a release build. Drives the
    /// acknowledged Smooth fallback or a failed capture without any real device
    /// fault.
    #[cfg(all(
        target_os = "macos",
        feature = "retained-renderer",
        feature = "dev-preview"
    ))]
    pub retained_fault_injection: Option<RetainedFaultInjection>,
}

/// One deterministic retained-host lifecycle or fault case. This automation is
/// compiled only into local dev-preview builds; release companion binaries have
/// neither the CLI value nor the harness entrypoint.
#[cfg(all(
    target_os = "macos",
    feature = "retained-renderer",
    feature = "dev-preview"
))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReviewSceneSoakScenario {
    ResizePreparing,
    ResizeReady,
    ResizeActivating,
    ResizeStorm,
    BackingScale,
    HiddenSemanticReveal,
    CaptureSwap,
    ShutdownWorker,
    ShutdownActivation,
    TrackingRunLoop,
    SlowTick,
    SurfaceOutdated,
    SurfaceTimeout,
    SurfaceOccluded,
    SurfaceLost,
    SurfaceValidation,
    DeviceLoss,
    DeviceValidation,
    DeviceOutOfMemory,
}

#[cfg(all(
    target_os = "macos",
    feature = "retained-renderer",
    feature = "dev-preview"
))]
impl ReviewSceneSoakScenario {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResizePreparing => "resize-preparing",
            Self::ResizeReady => "resize-ready",
            Self::ResizeActivating => "resize-activating",
            Self::ResizeStorm => "resize-storm",
            Self::BackingScale => "backing-scale",
            Self::HiddenSemanticReveal => "hidden-semantic-reveal",
            Self::CaptureSwap => "capture-swap",
            Self::ShutdownWorker => "shutdown-worker",
            Self::ShutdownActivation => "shutdown-activation",
            Self::TrackingRunLoop => "tracking-run-loop",
            Self::SlowTick => "slow-tick",
            Self::SurfaceOutdated => "surface-outdated",
            Self::SurfaceTimeout => "surface-timeout",
            Self::SurfaceOccluded => "surface-occluded",
            Self::SurfaceLost => "surface-lost",
            Self::SurfaceValidation => "surface-validation",
            Self::DeviceLoss => "device-loss",
            Self::DeviceValidation => "device-validation",
            Self::DeviceOutOfMemory => "device-out-of-memory",
        }
    }
}

/// Sanitized counters emitted by one deterministic host soak. They describe
/// only transitions actually driven by the typed harness; they never claim that
/// a native AppKit gesture or a real GPU fault occurred.
#[cfg(all(
    target_os = "macos",
    feature = "retained-renderer",
    feature = "dev-preview"
))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ReviewSceneSoakCounters {
    pub ticks_attempted: u64,
    pub ticks_completed: u64,
    pub ticks_suppressed: u64,
    pub virtual_elapsed_ms: u64,
    pub resize_requests: u64,
    pub surface_reconfigurations: u64,
    pub presents: u64,
    pub skips: u64,
    pub fallbacks: u64,
    pub capture_bound: u64,
    pub capture_deferred: u64,
    pub worker_cancel_requests: u64,
    pub hidden_updates_coalesced: u64,
    pub reveals: u64,
    pub shutdowns: u64,
}

/// Machine-readable result for one deterministic soak case. All strings come
/// from closed enums or static failure categories; no live state is serialized.
#[cfg(all(
    target_os = "macos",
    feature = "retained-renderer",
    feature = "dev-preview"
))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReviewSceneSoakReport {
    pub schema_version: u32,
    pub scenario: &'static str,
    pub execution_mode: &'static str,
    pub expected_outcome: &'static str,
    pub observed_outcome: &'static str,
    pub sanitized_category: Option<&'static str>,
    pub counters: ReviewSceneSoakCounters,
    pub native_interactions_deferred: &'static [&'static str],
    pub passed: bool,
}

/// A bounded, static retained fault the dev/test harness can inject to exercise
/// the acknowledged Smooth fallback and failed-capture paths without a real GPU
/// fault. Compiled only with `retained-renderer` plus `dev-preview`, so a release
/// build has neither the flag nor the injection behavior. Every variant maps to a
/// static, privacy-safe [`RetainedFailureCategory`].
#[cfg(all(
    target_os = "macos",
    feature = "retained-renderer",
    feature = "dev-preview"
))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RetainedFaultInjection {
    /// Retained host initialization fails at startup; the app starts on Smooth.
    Initialization,
    /// The surface is reported lost mid-run.
    SurfaceLoss,
    /// The device reports a validation error mid-run.
    Validation,
    /// The device reports an internal error mid-run.
    Internal,
    /// The device reports an out-of-memory error mid-run.
    OutOfMemory,
    /// The device is reported lost mid-run.
    DeviceLoss,
    /// A GPU resource (glyph atlas) is unavailable mid-run.
    ResourceFailure,
    /// The frozen scene declares content the raster path cannot serve.
    UnsupportedRaster,
    /// Mapping the readback staging buffer fails during a paired capture.
    MapFailure,
    /// The readback buffer is too short, yielding a blank capture.
    BlankCapture,
    /// Writing a capture artifact to disk fails.
    WriteFailure,
}

#[cfg(all(
    target_os = "macos",
    feature = "retained-renderer",
    feature = "dev-preview"
))]
impl RetainedFaultInjection {
    /// The static failure category this fault presents as.
    const fn category(self) -> RetainedFailureCategory {
        match self {
            Self::Initialization => RetainedFailureCategory::DeviceUnavailable,
            Self::SurfaceLoss => RetainedFailureCategory::SurfaceLost,
            Self::Validation => RetainedFailureCategory::DeviceValidation,
            Self::Internal => RetainedFailureCategory::DeviceInternal,
            Self::OutOfMemory => RetainedFailureCategory::DeviceOutOfMemory,
            Self::DeviceLoss => RetainedFailureCategory::DeviceUnavailable,
            Self::ResourceFailure => RetainedFailureCategory::AtlasUnavailable,
            Self::UnsupportedRaster => RetainedFailureCategory::UnsupportedRaster,
            Self::MapFailure => RetainedFailureCategory::CaptureMapFailed,
            Self::BlankCapture => RetainedFailureCategory::CaptureBufferTooShort,
            Self::WriteFailure => RetainedFailureCategory::CaptureWriteFailed,
        }
    }

    /// The category to fail host initialization with, when this fault targets the
    /// startup path; `None` for every mid-run and capture fault.
    pub(crate) fn initialization_category(self) -> Option<RetainedFailureCategory> {
        matches!(self, Self::Initialization).then(|| self.category())
    }

    /// The category to raise as an asynchronous device fault mid-run, driving the
    /// acknowledged Smooth fallback; `None` for the initialization and capture
    /// faults.
    pub(crate) fn device_fault_category(self) -> Option<RetainedFailureCategory> {
        matches!(
            self,
            Self::SurfaceLoss
                | Self::Validation
                | Self::Internal
                | Self::OutOfMemory
                | Self::DeviceLoss
                | Self::ResourceFailure
                | Self::UnsupportedRaster
        )
        .then(|| self.category())
    }

    /// The category to fail the paired capture with, marking the manifest failed
    /// without changing the effective renderer; `None` for startup and mid-run
    /// device faults.
    pub(crate) fn capture_fault_category(self) -> Option<RetainedFailureCategory> {
        matches!(
            self,
            Self::MapFailure | Self::BlankCapture | Self::WriteFailure
        )
        .then(|| self.category())
    }
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
            || self.runtime_metrics_out.is_some()
            || self.review_lifetime_frames.is_some()
            || self.depth.is_some()
            || self.retained_scene_runtime.is_some()
    }
}

/// Temporary rollout switch for the direct retained scene runtime. This is not
/// a renderer backend: Retained remains the effective renderer in every mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SceneRuntimeRollout {
    Off,
    Shadow,
    Live,
}

impl SceneRuntimeRollout {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Shadow => "shadow",
            Self::Live => "live",
        }
    }
}

/// One-line rollback for the direct Retained scene route. The resize, display,
/// fullscreen, pacing, and composition blockers have been cleared, so capable
/// Apple-Silicon launches now present the direct scene by default.
pub const AUTO_SCENE_RUNTIME_ON_APPLE_SILICON: bool = true;

pub const fn resolve_scene_rollout(
    retained_scene_review: bool,
    live_enabled: bool,
) -> SceneRuntimeRollout {
    if !retained_scene_review {
        SceneRuntimeRollout::Off
    } else if live_enabled {
        SceneRuntimeRollout::Live
    } else {
        SceneRuntimeRollout::Shadow
    }
}

/// The scene path that actually presents the selected renderer. `Shadow` still
/// presents through the legacy retained translator; only `Live` presents the
/// direct scene runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveSceneRoute {
    Direct,
    Legacy,
    NotApplicable,
}

impl EffectiveSceneRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Legacy => "legacy",
            Self::NotApplicable => "not-applicable",
        }
    }
}

/// The authoritative retained scene-runtime policy used by both app startup and
/// capability reporting. An explicit review override wins; otherwise automatic
/// direct routing remains guarded by the one-line rollout constant.
pub const fn resolve_scene_runtime_rollout(
    request: CompanionRendererRequest,
    effective: EffectiveCompanionRenderer,
    retained_scene_runtime: Option<SceneRuntimeRollout>,
    auto_scene_runtime_enabled: bool,
) -> SceneRuntimeRollout {
    if !effective.is_retained() {
        return SceneRuntimeRollout::Off;
    }
    match retained_scene_runtime {
        Some(SceneRuntimeRollout::Off) => SceneRuntimeRollout::Off,
        Some(SceneRuntimeRollout::Shadow) => resolve_scene_rollout(true, false),
        Some(SceneRuntimeRollout::Live) => resolve_scene_rollout(true, true),
        None if auto_scene_runtime_enabled => SceneRuntimeRollout::Live,
        None => resolve_scene_rollout(request.is_explicit_retained(), false),
    }
}

pub const fn effective_scene_route(
    effective: EffectiveCompanionRenderer,
    rollout: SceneRuntimeRollout,
) -> EffectiveSceneRoute {
    if !effective.is_retained() {
        EffectiveSceneRoute::NotApplicable
    } else if matches!(rollout, SceneRuntimeRollout::Live) {
        EffectiveSceneRoute::Direct
    } else {
        EffectiveSceneRoute::Legacy
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
        effective_scene_route, resolve_renderer, resolve_scene_rollout,
        resolve_scene_runtime_rollout, CompanionRendererRequest, CompanionRendererTarget,
        CompanionReviewOptions, CompanionReviewSize, CompanionReviewState,
        EffectiveCompanionRenderer, EffectiveSceneRoute, RendererRuntimeState, SceneRuntimeRollout,
        AUTO_SCENE_RUNTIME_ON_APPLE_SILICON,
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
    fn scene_runtime_rollout_defaults_to_shadow_for_explicit_retained_review_only() {
        assert_eq!(
            resolve_scene_rollout(false, false),
            SceneRuntimeRollout::Off
        );
        assert_eq!(
            resolve_scene_rollout(true, false),
            SceneRuntimeRollout::Shadow
        );
        assert_eq!(resolve_scene_rollout(true, true), SceneRuntimeRollout::Live);
    }

    #[test]
    fn scene_runtime_cutover_enables_direct_retained_paths() {
        assert_eq!(
            resolve_scene_rollout(true, AUTO_SCENE_RUNTIME_ON_APPLE_SILICON),
            SceneRuntimeRollout::Live
        );
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn scene_runtime_one_line_rollback_rehearsal_restores_legacy_paths() {
        let auto = resolve_scene_runtime_rollout(
            CompanionRendererRequest::Auto,
            EffectiveCompanionRenderer::Retained,
            None,
            false,
        );
        assert_eq!(auto, SceneRuntimeRollout::Off);
        assert_eq!(
            effective_scene_route(EffectiveCompanionRenderer::Retained, auto),
            EffectiveSceneRoute::Legacy
        );

        let explicit = resolve_scene_runtime_rollout(
            CompanionRendererRequest::Retained,
            EffectiveCompanionRenderer::Retained,
            None,
            false,
        );
        assert_eq!(explicit, SceneRuntimeRollout::Shadow);
        assert_eq!(
            effective_scene_route(EffectiveCompanionRenderer::Retained, explicit),
            EffectiveSceneRoute::Legacy
        );
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn effective_scene_route_uses_the_app_startup_rollout_policy() {
        let auto_rollout = resolve_scene_runtime_rollout(
            CompanionRendererRequest::Auto,
            EffectiveCompanionRenderer::Retained,
            None,
            AUTO_SCENE_RUNTIME_ON_APPLE_SILICON,
        );
        assert_eq!(auto_rollout, SceneRuntimeRollout::Live);
        assert_eq!(
            effective_scene_route(EffectiveCompanionRenderer::Retained, auto_rollout),
            EffectiveSceneRoute::Direct
        );

        let explicit_retained = resolve_scene_runtime_rollout(
            CompanionRendererRequest::Retained,
            EffectiveCompanionRenderer::Retained,
            None,
            AUTO_SCENE_RUNTIME_ON_APPLE_SILICON,
        );
        assert_eq!(explicit_retained, SceneRuntimeRollout::Live);
        assert_eq!(
            effective_scene_route(EffectiveCompanionRenderer::Retained, explicit_retained),
            EffectiveSceneRoute::Direct
        );

        let explicit_live = resolve_scene_runtime_rollout(
            CompanionRendererRequest::Auto,
            EffectiveCompanionRenderer::Retained,
            Some(SceneRuntimeRollout::Live),
            AUTO_SCENE_RUNTIME_ON_APPLE_SILICON,
        );
        assert_eq!(explicit_live, SceneRuntimeRollout::Live);
        assert_eq!(
            effective_scene_route(EffectiveCompanionRenderer::Retained, explicit_live),
            EffectiveSceneRoute::Direct
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
    fn terminal_failure_preserves_retained_selection_and_first_category() {
        use crate::companion::retained::{FrameDisposition, RetainedFailureCategory};

        let mut state = RendererRuntimeState::fixture_retained();
        assert!(state.record_terminal_failure(RetainedFailureCategory::DeviceUnavailable));
        assert!(!state.record_terminal_failure(RetainedFailureCategory::DeviceValidation));
        assert_eq!(state.effective(), EffectiveCompanionRenderer::Retained);
        assert_eq!(
            state.terminal_failure(),
            Some(RetainedFailureCategory::DeviceUnavailable)
        );
        assert_eq!(
            state.disposition(),
            FrameDisposition::Failed(RetainedFailureCategory::DeviceUnavailable)
        );
        assert_eq!(state.transition_count(), 0);
        assert_eq!(state.last_fallback_reason(), None);
    }

    #[cfg(all(
        target_os = "macos",
        feature = "retained-renderer",
        feature = "dev-preview"
    ))]
    #[test]
    fn each_injected_fault_routes_to_exactly_one_seam() {
        use super::RetainedFaultInjection;
        use crate::companion::retained::RetainedFailureCategory;

        // A startup fault fails initialization only.
        let init = RetainedFaultInjection::Initialization;
        assert_eq!(
            init.initialization_category(),
            Some(RetainedFailureCategory::DeviceUnavailable)
        );
        assert_eq!(init.device_fault_category(), None);
        assert_eq!(init.capture_fault_category(), None);

        // Mid-run device faults drive the runtime fallback, never a capture failure.
        for fault in [
            RetainedFaultInjection::SurfaceLoss,
            RetainedFaultInjection::Validation,
            RetainedFaultInjection::Internal,
            RetainedFaultInjection::OutOfMemory,
            RetainedFaultInjection::DeviceLoss,
            RetainedFaultInjection::ResourceFailure,
            RetainedFaultInjection::UnsupportedRaster,
        ] {
            assert!(fault.device_fault_category().is_some(), "{fault:?}");
            assert_eq!(fault.initialization_category(), None, "{fault:?}");
            assert_eq!(fault.capture_fault_category(), None, "{fault:?}");
        }

        // Capture faults fail the paired capture without a runtime fallback.
        for (fault, expected) in [
            (
                RetainedFaultInjection::MapFailure,
                RetainedFailureCategory::CaptureMapFailed,
            ),
            (
                RetainedFaultInjection::BlankCapture,
                RetainedFailureCategory::CaptureBufferTooShort,
            ),
            (
                RetainedFaultInjection::WriteFailure,
                RetainedFailureCategory::CaptureWriteFailed,
            ),
        ] {
            assert_eq!(fault.capture_fault_category(), Some(expected), "{fault:?}");
            assert_eq!(fault.initialization_category(), None, "{fault:?}");
            assert_eq!(fault.device_fault_category(), None, "{fault:?}");
        }
    }

    #[cfg(all(
        target_os = "macos",
        feature = "retained-renderer",
        feature = "dev-preview"
    ))]
    #[test]
    fn review_scene_soak_names_are_stable_and_complete() {
        use super::ReviewSceneSoakScenario as Scenario;

        let names = [
            (Scenario::ResizePreparing, "resize-preparing"),
            (Scenario::ResizeReady, "resize-ready"),
            (Scenario::ResizeActivating, "resize-activating"),
            (Scenario::ResizeStorm, "resize-storm"),
            (Scenario::BackingScale, "backing-scale"),
            (Scenario::HiddenSemanticReveal, "hidden-semantic-reveal"),
            (Scenario::CaptureSwap, "capture-swap"),
            (Scenario::ShutdownWorker, "shutdown-worker"),
            (Scenario::ShutdownActivation, "shutdown-activation"),
            (Scenario::TrackingRunLoop, "tracking-run-loop"),
            (Scenario::SlowTick, "slow-tick"),
            (Scenario::SurfaceOutdated, "surface-outdated"),
            (Scenario::SurfaceTimeout, "surface-timeout"),
            (Scenario::SurfaceOccluded, "surface-occluded"),
            (Scenario::SurfaceLost, "surface-lost"),
            (Scenario::SurfaceValidation, "surface-validation"),
            (Scenario::DeviceLoss, "device-loss"),
            (Scenario::DeviceValidation, "device-validation"),
            (Scenario::DeviceOutOfMemory, "device-out-of-memory"),
        ];
        for (scenario, expected) in names {
            assert_eq!(scenario.as_str(), expected);
        }
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
    fn auto_resolves_to_retained_now_that_the_cutover_constant_is_enabled() {
        // Drives Auto with the real cutover constant: while it is true, Apple
        // Silicon resolves to Retained. Reverting the constant to false flips
        // this back to Smooth and is the one-line rollback.
        assert_eq!(
            resolve_renderer(
                CompanionRendererRequest::Auto,
                CompanionRendererTarget::AppleSiliconMac,
                true,
                super::AUTO_RETAINED_ON_APPLE_SILICON,
            ),
            Ok(EffectiveCompanionRenderer::Retained),
        );
    }

    #[test]
    fn capabilities_metadata_reports_auto_resolution_and_the_enabled_policy() {
        let mut out = Vec::new();
        super::print_companion_capabilities(CompanionRendererRequest::Auto, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("glorp-companion-capabilities: v1"), "{text}");
        assert!(text.contains("requested-renderer=auto"), "{text}");
        // The cutover constant is enabled, so the metadata always reports it on.
        assert!(
            text.contains("auto-retained-on-apple-silicon=true"),
            "{text}"
        );
        assert!(
            text.contains(&format!("retained-compiled={}", super::RETAINED_COMPILED)),
            "{text}"
        );
        // Auto's effective renderer is whatever the resolver picks for this
        // build's compiled capability and host target: Retained only on a
        // capable Apple-Silicon build, Smooth otherwise.
        let expected = resolve_renderer(
            CompanionRendererRequest::Auto,
            CompanionRendererTarget::current(),
            super::RETAINED_COMPILED,
            super::AUTO_RETAINED_ON_APPLE_SILICON,
        )
        .expect("auto resolves to a renderer");
        assert!(
            text.contains(&format!("effective-renderer={}", expected.as_str())),
            "{text}"
        );
        if expected.is_retained() {
            assert!(text.contains("effective-scene-route=direct"), "{text}");
        } else {
            assert!(
                text.contains("effective-scene-route=not-applicable"),
                "{text}"
            );
        }
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn capabilities_metadata_reports_explicit_retained_capability() {
        let mut out = Vec::new();
        super::print_companion_capabilities(CompanionRendererRequest::Retained, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("requested-renderer=retained"), "{text}");
        assert!(text.contains("retained-compiled=true"), "{text}");
        // Explicit Retained follows the enabled automatic direct-scene cutover.
        assert!(text.contains("effective-renderer=retained"), "{text}");
        assert!(text.contains("effective-scene-route=direct"), "{text}");
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    #[test]
    fn capabilities_metadata_honors_explicit_live_scene_runtime() {
        let mut out = Vec::new();
        super::print_companion_capabilities_with_scene_runtime(
            CompanionRendererRequest::Retained,
            Some(SceneRuntimeRollout::Live),
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("effective-renderer=retained"), "{text}");
        assert!(text.contains("effective-scene-route=direct"), "{text}");
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
    /// Whether this request explicitly selected the retained backend. Kept
    /// feature-neutral so capability code also compiles in Smooth-only builds.
    pub const fn is_explicit_retained(self) -> bool {
        #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
        {
            matches!(self, Self::Retained)
        }
        #[cfg(not(all(target_os = "macos", feature = "retained-renderer")))]
        {
            false
        }
    }

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

    /// The stable machine-readable label reported by the capabilities metadata
    /// command. Unlike [`forwarded_arg`](Self::forwarded_arg), `Auto` has a
    /// concrete label so the smoke can read back the requested renderer.
    pub const fn label(self) -> &'static str {
        match self {
            CompanionRendererRequest::Auto => "auto",
            CompanionRendererRequest::Classic => "classic",
            CompanionRendererRequest::Pixel => "pixel",
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            CompanionRendererRequest::Retained => "retained",
            CompanionRendererRequest::Smooth => "smooth",
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

    /// The stable machine-readable label reported by the capabilities metadata
    /// command.
    pub const fn as_str(self) -> &'static str {
        match self {
            CompanionRendererTarget::AppleSiliconMac => "apple-silicon-mac",
            CompanionRendererTarget::IntelMac => "intel-mac",
            CompanionRendererTarget::Other => "other",
        }
    }
}

/// Whether the retained renderer backend is compiled into this build. The staged
/// release smoke reads this as `retained-compiled=<bool>` to prove the shipped
/// capability matrix (Apple Silicon ships it; Intel does not).
pub const RETAINED_COMPILED: bool = cfg!(all(target_os = "macos", feature = "retained-renderer"));

/// Prints the compiled renderer capabilities and Auto policy for this build in a
/// stable `key=value` form, then returns. Metadata only: it resolves the renderer
/// through [`resolve_renderer`] but never opens a window or touches pet state, so
/// the release smoke can read the shipped capabilities and policy from a bounded,
/// GUI-less process. `request` is the operator's `--renderer` value; the reported
/// `effective-renderer` is what `Auto` (or that explicit request) resolves to on
/// the current machine under the current cutover policy.
pub fn print_companion_capabilities(
    request: CompanionRendererRequest,
    out: &mut impl std::io::Write,
) -> std::io::Result<()> {
    print_companion_capabilities_with_scene_runtime(request, None, out)
}

/// Capability reporting variant used by CLI dispatch so the bounded metadata
/// command evaluates the same explicit scene-runtime override as app startup.
pub fn print_companion_capabilities_with_scene_runtime(
    request: CompanionRendererRequest,
    retained_scene_runtime: Option<SceneRuntimeRollout>,
    out: &mut impl std::io::Write,
) -> std::io::Result<()> {
    let target = CompanionRendererTarget::current();
    let effective = resolve_renderer(
        request,
        target,
        RETAINED_COMPILED,
        AUTO_RETAINED_ON_APPLE_SILICON,
    );
    writeln!(out, "glorp-companion-capabilities: v1")?;
    writeln!(out, "requested-renderer={}", request.label())?;
    match effective {
        Ok(effective) => {
            writeln!(out, "effective-renderer={}", effective.as_str())?;
            let rollout = resolve_scene_runtime_rollout(
                request,
                effective,
                retained_scene_runtime,
                AUTO_SCENE_RUNTIME_ON_APPLE_SILICON,
            );
            writeln!(
                out,
                "effective-scene-route={}",
                effective_scene_route(effective, rollout).as_str()
            )?;
        }
        Err(error) => {
            writeln!(out, "effective-renderer=unavailable:{}", error.category())?;
            writeln!(out, "effective-scene-route=unavailable")?;
        }
    }
    writeln!(out, "retained-compiled={RETAINED_COMPILED}")?;
    writeln!(
        out,
        "auto-retained-on-apple-silicon={AUTO_RETAINED_ON_APPLE_SILICON}"
    )?;
    writeln!(out, "target-class={}", target.as_str())?;
    Ok(())
}

/// The single cutover switch. While this is `true`, `Auto` resolves to Retained
/// on capable Apple-Silicon hardware; reverting it to `false` is the one-line
/// rollback that returns Auto to Smooth everywhere.
pub const AUTO_RETAINED_ON_APPLE_SILICON: bool = true;

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

/// Live renderer state for one running companion. Renderer selection is fixed at
/// construction; retained renderer health is recorded independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererRuntimeState {
    requested: CompanionRendererRequest,
    effective: EffectiveCompanionRenderer,
    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    terminal_failure: Option<RetainedFailureCategory>,
}

impl RendererRuntimeState {
    pub fn new(requested: CompanionRendererRequest, effective: EffectiveCompanionRenderer) -> Self {
        Self {
            requested,
            effective,
            #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
            terminal_failure: None,
        }
    }

    pub fn requested(&self) -> CompanionRendererRequest {
        self.requested
    }

    pub fn effective(&self) -> EffectiveCompanionRenderer {
        self.effective
    }

    pub fn transition_count(&self) -> u64 {
        0
    }

    pub fn last_fallback_reason(&self) -> Option<&'static str> {
        None
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    pub(crate) fn record_terminal_failure(&mut self, category: RetainedFailureCategory) -> bool {
        if self.terminal_failure.is_some() {
            false
        } else {
            self.terminal_failure = Some(category);
            true
        }
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    pub(crate) fn terminal_failure(&self) -> Option<RetainedFailureCategory> {
        self.terminal_failure
    }

    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    // Typed review-evidence seam; production gating reads `terminal_failure`
    // directly and tests cover the corresponding disposition contract.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn disposition(&self) -> FrameDisposition {
        self.terminal_failure
            .map(FrameDisposition::Failed)
            .unwrap_or(FrameDisposition::SurfacePresentCalled)
    }

    /// A retained runtime that has resolved to the Retained renderer, for driving
    /// terminal-failure state transitions under test.
    #[cfg(all(test, target_os = "macos", feature = "retained-renderer"))]
    pub(crate) fn fixture_retained() -> Self {
        Self::new(
            CompanionRendererRequest::Retained,
            EffectiveCompanionRenderer::Retained,
        )
    }
}
