//! AppKit layer activation and live wgpu surface ownership.

use std::ffi::c_void;
use std::sync::Arc;
use std::time::Instant;

use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::NSSize;
use objc2_quartz_core::CAMetalLayer;

use super::*;

/// Main-thread timer callback gate. AppKit may run a nested tracking loop while a
/// callback is active; this gate drops the nested tick instead of allowing the
/// retained host state machine to be entered recursively.
#[derive(Debug, Default)]
pub(in crate::companion) struct NonReentrantTickGate {
    active: std::cell::Cell<bool>,
}

impl NonReentrantTickGate {
    pub(in crate::companion) fn enter(&self) -> Option<NonReentrantTickGuard<'_>> {
        if self.active.replace(true) {
            None
        } else {
            Some(NonReentrantTickGuard { gate: self })
        }
    }
}

pub(in crate::companion) struct NonReentrantTickGuard<'a> {
    gate: &'a NonReentrantTickGate,
}

impl Drop for NonReentrantTickGuard<'_> {
    fn drop(&mut self) {
        self.gate.active.set(false);
    }
}

#[cfg(feature = "dev-preview")]
const NO_NATIVE_INTERACTIONS: &[&str] = &[];
#[cfg(feature = "dev-preview")]
const NATIVE_RESIZE_INTERACTIONS: &[&str] = &[
    "AppKit live-resize/fullscreen events",
    "physical display scale migration",
];
#[cfg(feature = "dev-preview")]
const NATIVE_TRACKING_INTERACTIONS: &[&str] = &[
    "AppKit menu tracking loop",
    "AppKit live-resize tracking loop",
];
#[cfg(feature = "dev-preview")]
const NATIVE_SURFACE_INTERACTIONS: &[&str] = &[
    "real CAMetalLayer acquisition result",
    "real wgpu device callback",
];

/// Deterministic model of the host-owned lifecycle boundary. It consumes the
/// same typed frame dispositions/categories as the surface host, but never
/// claims to synthesize AppKit gestures or a real GPU fault. Those native
/// interactions are named explicitly in the report for manual Task10 coverage.
#[cfg(feature = "dev-preview")]
struct ReviewSceneHostHarness {
    phase: ReviewSceneHostPhase,
    physical_extent: [u32; 2],
    backing_scale: f64,
    surface_epoch: u64,
    active_generation: Option<u64>,
    candidate_generation: Option<u64>,
    hidden_semantic_pending: bool,
    counters: crate::commands::companion_mode::ReviewSceneSoakCounters,
}

#[cfg(feature = "dev-preview")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewSceneHostPhase {
    Idle,
    Preparing,
    Ready,
    Activating,
    Active,
    Hidden,
    Shutdown,
}

#[cfg(feature = "dev-preview")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeterministicSurfaceOutcome {
    Presented,
    Outdated,
    Timeout,
    Occluded,
    Lost,
    Validation,
}

#[cfg(feature = "dev-preview")]
impl ReviewSceneHostHarness {
    fn new() -> Self {
        Self {
            phase: ReviewSceneHostPhase::Idle,
            physical_extent: [360, 360],
            backing_scale: 1.0,
            surface_epoch: 1,
            active_generation: None,
            candidate_generation: None,
            hidden_semantic_pending: false,
            counters: crate::commands::companion_mode::ReviewSceneSoakCounters::default(),
        }
    }

    fn begin_preparing(&mut self, generation: u64) {
        self.phase = ReviewSceneHostPhase::Preparing;
        self.candidate_generation = Some(generation);
    }

    fn finish_preparing(&mut self) {
        assert_eq!(self.phase, ReviewSceneHostPhase::Preparing);
        self.phase = ReviewSceneHostPhase::Ready;
    }

    fn begin_activation(&mut self) {
        assert_eq!(self.phase, ReviewSceneHostPhase::Ready);
        self.phase = ReviewSceneHostPhase::Activating;
    }

    fn resize(&mut self, logical: u32, scale: f64) {
        self.counters.resize_requests = self.counters.resize_requests.saturating_add(1);
        let extent = [
            physical_dimension(f64::from(logical), scale),
            physical_dimension(f64::from(logical), scale),
        ];
        if extent != self.physical_extent || scale.to_bits() != self.backing_scale.to_bits() {
            self.physical_extent = extent;
            self.backing_scale = scale;
            self.surface_epoch = self.surface_epoch.saturating_add(1);
            self.counters.surface_reconfigurations =
                self.counters.surface_reconfigurations.saturating_add(1);
        }
    }

    fn activate_with_surface(&mut self, outcome: DeterministicSurfaceOutcome) -> FrameDisposition {
        assert_eq!(self.phase, ReviewSceneHostPhase::Activating);
        let mut progress = FrameProgress::new(0, self.candidate_generation.unwrap_or_default());
        progress
            .mark(FrameMilestone::Prepared)
            .expect("activation begins with a prepared candidate");
        match outcome {
            DeterministicSurfaceOutcome::Presented => {
                progress
                    .mark(FrameMilestone::Encoded)
                    .expect("present encodes after prepare");
                progress
                    .mark(FrameMilestone::Submitted)
                    .expect("present submits after encode");
                progress
                    .finish(FrameDisposition::SurfacePresentCalled)
                    .expect("first present terminates exactly once");
                self.active_generation = self.candidate_generation.take();
                self.phase = ReviewSceneHostPhase::Active;
                self.counters.presents = self.counters.presents.saturating_add(1);
            }
            DeterministicSurfaceOutcome::Outdated => {
                skip(&mut progress, SkipReason::Outdated);
                self.phase = ReviewSceneHostPhase::Ready;
                self.surface_epoch = self.surface_epoch.saturating_add(1);
                self.counters.surface_reconfigurations =
                    self.counters.surface_reconfigurations.saturating_add(1);
                self.counters.skips = self.counters.skips.saturating_add(1);
            }
            DeterministicSurfaceOutcome::Timeout => {
                skip(&mut progress, SkipReason::Timeout);
                self.phase = ReviewSceneHostPhase::Ready;
                self.counters.skips = self.counters.skips.saturating_add(1);
            }
            DeterministicSurfaceOutcome::Occluded => {
                skip(&mut progress, SkipReason::Occluded);
                self.phase = ReviewSceneHostPhase::Ready;
                self.counters.skips = self.counters.skips.saturating_add(1);
            }
            DeterministicSurfaceOutcome::Lost => {
                fail(&mut progress, RetainedFailureCategory::SurfaceLost);
                self.phase = ReviewSceneHostPhase::Idle;
                self.candidate_generation = None;
                self.counters.fallbacks = self.counters.fallbacks.saturating_add(1);
            }
            DeterministicSurfaceOutcome::Validation => {
                fail(&mut progress, RetainedFailureCategory::SurfaceValidation);
                self.phase = ReviewSceneHostPhase::Idle;
                self.candidate_generation = None;
                self.counters.fallbacks = self.counters.fallbacks.saturating_add(1);
            }
        }
        progress
            .disposition()
            .expect("every deterministic acquisition is terminal")
    }

    fn inject_device_failure(&mut self, category: RetainedFailureCategory) {
        assert!(matches!(
            category,
            RetainedFailureCategory::DeviceUnavailable
                | RetainedFailureCategory::DeviceValidation
                | RetainedFailureCategory::DeviceOutOfMemory
        ));
        let mailbox = GpuErrorMailbox::new();
        mailbox
            .sender_for(crate::presentation::companion_scene::DeviceEpoch(1))
            .send(category)
            .expect("deterministic device fault mailbox remains connected");
        assert_eq!(mailbox.drain(), Some(category));
        self.phase = ReviewSceneHostPhase::Idle;
        self.candidate_generation = None;
        self.active_generation = None;
        self.counters.fallbacks = self.counters.fallbacks.saturating_add(1);
    }

    fn capture(&mut self) -> Option<u64> {
        if self.phase == ReviewSceneHostPhase::Activating {
            self.counters.capture_deferred = self.counters.capture_deferred.saturating_add(1);
            None
        } else {
            let generation = self.active_generation;
            if generation.is_some() {
                self.counters.capture_bound = self.counters.capture_bound.saturating_add(1);
            } else {
                self.counters.capture_deferred = self.counters.capture_deferred.saturating_add(1);
            }
            generation
        }
    }

    fn hide(&mut self) {
        assert_eq!(self.phase, ReviewSceneHostPhase::Active);
        self.phase = ReviewSceneHostPhase::Hidden;
    }

    fn coalesce_hidden_semantic_change(&mut self) {
        assert_eq!(self.phase, ReviewSceneHostPhase::Hidden);
        self.hidden_semantic_pending = true;
        self.counters.hidden_updates_coalesced =
            self.counters.hidden_updates_coalesced.saturating_add(1);
    }

    fn reveal(&mut self) {
        assert_eq!(self.phase, ReviewSceneHostPhase::Hidden);
        self.hidden_semantic_pending = false;
        self.phase = ReviewSceneHostPhase::Active;
        self.counters.reveals = self.counters.reveals.saturating_add(1);
    }

    fn shutdown(&mut self) {
        if self.phase == ReviewSceneHostPhase::Preparing {
            self.counters.worker_cancel_requests =
                self.counters.worker_cancel_requests.saturating_add(1);
        }
        self.phase = ReviewSceneHostPhase::Shutdown;
        self.candidate_generation = None;
        self.counters.shutdowns = self.counters.shutdowns.saturating_add(1);
    }

    fn run_virtual_common_mode_tracking(&mut self, duration_ms: u64, interval_ms: u64) {
        assert!(interval_ms > 0);
        while self.counters.virtual_elapsed_ms < duration_ms {
            self.counters.ticks_attempted = self.counters.ticks_attempted.saturating_add(1);
            self.counters.ticks_completed = self.counters.ticks_completed.saturating_add(1);
            self.counters.virtual_elapsed_ms = self
                .counters
                .virtual_elapsed_ms
                .saturating_add(interval_ms)
                .min(duration_ms);
        }
    }
}

#[cfg(feature = "dev-preview")]
struct SoakObservation {
    expected: &'static str,
    observed: &'static str,
    category: Option<RetainedFailureCategory>,
    counters: crate::commands::companion_mode::ReviewSceneSoakCounters,
    native_interactions_deferred: &'static [&'static str],
}

#[cfg(feature = "dev-preview")]
pub(crate) fn review_scene_soak_report(
    scenario: crate::commands::companion_mode::ReviewSceneSoakScenario,
) -> crate::commands::companion_mode::ReviewSceneSoakReport {
    use crate::commands::companion_mode::ReviewSceneSoakScenario as Scenario;

    let mut host = ReviewSceneHostHarness::new();
    let observation = match scenario {
        Scenario::ResizePreparing => {
            host.begin_preparing(1);
            host.resize(480, 1.0);
            SoakObservation {
                expected: "preparing-retained-after-resize",
                observed: if host.phase == ReviewSceneHostPhase::Preparing
                    && host.candidate_generation == Some(1)
                {
                    "preparing-retained-after-resize"
                } else {
                    "preparing-resize-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NATIVE_RESIZE_INTERACTIONS,
            }
        }
        Scenario::ResizeReady => {
            host.begin_preparing(1);
            host.finish_preparing();
            host.resize(480, 1.0);
            SoakObservation {
                expected: "ready-retained-for-rebound-surface",
                observed: if host.phase == ReviewSceneHostPhase::Ready
                    && host.candidate_generation == Some(1)
                {
                    "ready-retained-for-rebound-surface"
                } else {
                    "ready-resize-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NATIVE_RESIZE_INTERACTIONS,
            }
        }
        Scenario::ResizeActivating => {
            host.begin_preparing(1);
            host.finish_preparing();
            host.begin_activation();
            host.resize(720, 1.0);
            let disposition = host.activate_with_surface(DeterministicSurfaceOutcome::Outdated);
            SoakObservation {
                expected: "activation-deferred-after-resize",
                observed: if disposition == FrameDisposition::Skipped(SkipReason::Outdated)
                    && host.phase == ReviewSceneHostPhase::Ready
                {
                    "activation-deferred-after-resize"
                } else {
                    "activation-resize-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NATIVE_RESIZE_INTERACTIONS,
            }
        }
        Scenario::ResizeStorm => {
            for logical in [260, 360, 480, 720] {
                host.resize(logical, 1.0);
            }
            SoakObservation {
                expected: "resize-storm-260-360-480-720",
                observed: if host.physical_extent == [720, 720]
                    && host.counters.resize_requests == 4
                    && host.counters.surface_reconfigurations == 4
                {
                    "resize-storm-260-360-480-720"
                } else {
                    "resize-storm-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NATIVE_RESIZE_INTERACTIONS,
            }
        }
        Scenario::BackingScale => {
            host.resize(360, 1.0);
            host.resize(360, 2.0);
            host.resize(360, 1.0);
            SoakObservation {
                expected: "backing-scale-1x-2x-1x",
                observed: if host.physical_extent == [360, 360]
                    && host.backing_scale == 1.0
                    && host.counters.surface_reconfigurations == 2
                {
                    "backing-scale-1x-2x-1x"
                } else {
                    "backing-scale-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NATIVE_RESIZE_INTERACTIONS,
            }
        }
        Scenario::HiddenSemanticReveal => {
            host.active_generation = Some(1);
            host.phase = ReviewSceneHostPhase::Active;
            host.hide();
            host.coalesce_hidden_semantic_change();
            host.coalesce_hidden_semantic_change();
            host.reveal();
            SoakObservation {
                expected: "latest-hidden-semantic-revealed-once",
                observed: if host.phase == ReviewSceneHostPhase::Active
                    && !host.hidden_semantic_pending
                    && host.counters.hidden_updates_coalesced == 2
                    && host.counters.reveals == 1
                {
                    "latest-hidden-semantic-revealed-once"
                } else {
                    "hidden-reveal-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NO_NATIVE_INTERACTIONS,
            }
        }
        Scenario::CaptureSwap => {
            host.active_generation = Some(1);
            host.phase = ReviewSceneHostPhase::Active;
            let before = host.capture();
            host.begin_preparing(2);
            host.finish_preparing();
            host.begin_activation();
            let during = host.capture();
            let first_present = host.activate_with_surface(DeterministicSurfaceOutcome::Presented);
            let after = host.capture();
            SoakObservation {
                expected: "capture-bound-before-deferred-during-bound-after",
                observed: if before == Some(1)
                    && during.is_none()
                    && first_present == FrameDisposition::SurfacePresentCalled
                    && after == Some(2)
                {
                    "capture-bound-before-deferred-during-bound-after"
                } else {
                    "capture-swap-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NO_NATIVE_INTERACTIONS,
            }
        }
        Scenario::ShutdownWorker => {
            host.begin_preparing(1);
            host.shutdown();
            SoakObservation {
                expected: "worker-cancel-requested-before-shutdown",
                observed: if host.phase == ReviewSceneHostPhase::Shutdown
                    && host.counters.worker_cancel_requests == 1
                    && host.candidate_generation.is_none()
                {
                    "worker-cancel-requested-before-shutdown"
                } else {
                    "shutdown-worker-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NO_NATIVE_INTERACTIONS,
            }
        }
        Scenario::ShutdownActivation => {
            host.begin_preparing(1);
            host.finish_preparing();
            host.begin_activation();
            host.shutdown();
            SoakObservation {
                expected: "activation-dropped-before-shutdown",
                observed: if host.phase == ReviewSceneHostPhase::Shutdown
                    && host.candidate_generation.is_none()
                    && host.counters.shutdowns == 1
                {
                    "activation-dropped-before-shutdown"
                } else {
                    "shutdown-activation-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NO_NATIVE_INTERACTIONS,
            }
        }
        Scenario::TrackingRunLoop => {
            // Virtual tracking time exceeds the two-minute review watchdog. The
            // production timer is registered in NSRunLoopCommonModes; Task10 must
            // still perform the actual menu/live-resize gesture.
            const WATCHDOG_MS: u64 = 120_000;
            let tracking_elapsed_ms = WATCHDOG_MS + 1_000;
            host.run_virtual_common_mode_tracking(tracking_elapsed_ms, 250);
            SoakObservation {
                expected: "common-mode-cadence-beyond-watchdog",
                observed: if host.counters.virtual_elapsed_ms > WATCHDOG_MS
                    && host.counters.ticks_completed > 0
                {
                    "common-mode-cadence-beyond-watchdog"
                } else {
                    "tracking-run-loop-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NATIVE_TRACKING_INTERACTIONS,
            }
        }
        Scenario::SlowTick => {
            let gate = NonReentrantTickGate::default();
            host.counters.ticks_attempted = host.counters.ticks_attempted.saturating_add(1);
            let first = gate.enter();
            if first.is_some() {
                host.counters.ticks_completed = host.counters.ticks_completed.saturating_add(1);
            }
            host.counters.ticks_attempted = host.counters.ticks_attempted.saturating_add(1);
            if gate.enter().is_none() {
                host.counters.ticks_suppressed = host.counters.ticks_suppressed.saturating_add(1);
            }
            drop(first);
            host.counters.ticks_attempted = host.counters.ticks_attempted.saturating_add(1);
            if gate.enter().is_some() {
                host.counters.ticks_completed = host.counters.ticks_completed.saturating_add(1);
            }
            SoakObservation {
                expected: "nested-slow-tick-suppressed-next-tick-runs",
                observed: if host.counters.ticks_attempted == 3
                    && host.counters.ticks_completed == 2
                    && host.counters.ticks_suppressed == 1
                {
                    "nested-slow-tick-suppressed-next-tick-runs"
                } else {
                    "slow-tick-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NO_NATIVE_INTERACTIONS,
            }
        }
        Scenario::SurfaceOutdated | Scenario::SurfaceTimeout | Scenario::SurfaceOccluded => {
            host.begin_preparing(1);
            host.finish_preparing();
            host.begin_activation();
            let (surface, reason, expected) = match scenario {
                Scenario::SurfaceOutdated => (
                    DeterministicSurfaceOutcome::Outdated,
                    SkipReason::Outdated,
                    "skip-outdated-retry-later",
                ),
                Scenario::SurfaceTimeout => (
                    DeterministicSurfaceOutcome::Timeout,
                    SkipReason::Timeout,
                    "skip-timeout-retry-later",
                ),
                Scenario::SurfaceOccluded => (
                    DeterministicSurfaceOutcome::Occluded,
                    SkipReason::Occluded,
                    "skip-occluded-retry-later",
                ),
                _ => unreachable!(),
            };
            let disposition = host.activate_with_surface(surface);
            SoakObservation {
                expected,
                observed: if disposition == FrameDisposition::Skipped(reason)
                    && host.phase == ReviewSceneHostPhase::Ready
                    && host.counters.fallbacks == 0
                {
                    expected
                } else {
                    "surface-skip-mismatch"
                },
                category: None,
                counters: host.counters,
                native_interactions_deferred: NATIVE_SURFACE_INTERACTIONS,
            }
        }
        Scenario::SurfaceLost | Scenario::SurfaceValidation => {
            host.begin_preparing(1);
            host.finish_preparing();
            host.begin_activation();
            let (surface, category, expected) = match scenario {
                Scenario::SurfaceLost => (
                    DeterministicSurfaceOutcome::Lost,
                    RetainedFailureCategory::SurfaceLost,
                    "fallback-surface-lost",
                ),
                Scenario::SurfaceValidation => (
                    DeterministicSurfaceOutcome::Validation,
                    RetainedFailureCategory::SurfaceValidation,
                    "fallback-surface-validation",
                ),
                _ => unreachable!(),
            };
            let disposition = host.activate_with_surface(surface);
            SoakObservation {
                expected,
                observed: if disposition == FrameDisposition::Failed(category)
                    && host.counters.fallbacks == 1
                {
                    expected
                } else {
                    "surface-failure-mismatch"
                },
                category: Some(category),
                counters: host.counters,
                native_interactions_deferred: NATIVE_SURFACE_INTERACTIONS,
            }
        }
        Scenario::DeviceLoss | Scenario::DeviceValidation | Scenario::DeviceOutOfMemory => {
            let (category, expected) = match scenario {
                Scenario::DeviceLoss => (
                    RetainedFailureCategory::DeviceUnavailable,
                    "fallback-device-lost",
                ),
                Scenario::DeviceValidation => (
                    RetainedFailureCategory::DeviceValidation,
                    "fallback-device-validation",
                ),
                Scenario::DeviceOutOfMemory => (
                    RetainedFailureCategory::DeviceOutOfMemory,
                    "fallback-device-out-of-memory",
                ),
                _ => unreachable!(),
            };
            host.active_generation = Some(1);
            host.phase = ReviewSceneHostPhase::Active;
            host.inject_device_failure(category);
            SoakObservation {
                expected,
                observed: if host.active_generation.is_none()
                    && host.counters.fallbacks == 1
                    && host.phase == ReviewSceneHostPhase::Idle
                {
                    expected
                } else {
                    "device-failure-mismatch"
                },
                category: Some(category),
                counters: host.counters,
                native_interactions_deferred: NATIVE_SURFACE_INTERACTIONS,
            }
        }
    };
    crate::commands::companion_mode::ReviewSceneSoakReport {
        schema_version: 1,
        scenario: scenario.as_str(),
        execution_mode: "deterministic-host-harness",
        expected_outcome: observation.expected,
        observed_outcome: observation.observed,
        sanitized_category: observation.category.map(RetainedFailureCategory::category),
        counters: observation.counters,
        native_interactions_deferred: observation.native_interactions_deferred,
        passed: observation.expected == observation.observed,
    }
}

#[cfg(feature = "dev-preview")]
pub(crate) fn run_review_scene_soak(
    scenario: crate::commands::companion_mode::ReviewSceneSoakScenario,
    writer: &mut dyn std::io::Write,
) -> crate::error::Result<()> {
    let report = review_scene_soak_report(scenario);
    serde_json::to_writer_pretty(&mut *writer, &report)?;
    writeln!(writer)?;
    if report.passed {
        Ok(())
    } else {
        Err(crate::error::GlorpError::Message(format!(
            "review scene soak mismatch ({})",
            report.scenario
        )))
    }
}

#[cfg(all(test, feature = "dev-preview"))]
mod review_scene_soak_tests {
    use super::*;
    use crate::commands::companion_mode::ReviewSceneSoakScenario as Scenario;

    const ALL_SCENARIOS: [Scenario; 19] = [
        Scenario::ResizePreparing,
        Scenario::ResizeReady,
        Scenario::ResizeActivating,
        Scenario::ResizeStorm,
        Scenario::BackingScale,
        Scenario::HiddenSemanticReveal,
        Scenario::CaptureSwap,
        Scenario::ShutdownWorker,
        Scenario::ShutdownActivation,
        Scenario::TrackingRunLoop,
        Scenario::SlowTick,
        Scenario::SurfaceOutdated,
        Scenario::SurfaceTimeout,
        Scenario::SurfaceOccluded,
        Scenario::SurfaceLost,
        Scenario::SurfaceValidation,
        Scenario::DeviceLoss,
        Scenario::DeviceValidation,
        Scenario::DeviceOutOfMemory,
    ];

    #[test]
    fn first_present_commits_candidate_and_binds_followup_capture() {
        let mut host = ReviewSceneHostHarness::new();
        host.begin_preparing(2);
        host.finish_preparing();
        host.begin_activation();

        assert_eq!(
            host.activate_with_surface(DeterministicSurfaceOutcome::Presented),
            FrameDisposition::SurfacePresentCalled
        );
        assert_eq!(host.active_generation, Some(2));
        assert_eq!(host.candidate_generation, None);
        assert_eq!(host.capture(), Some(2));
        assert_eq!(host.counters.presents, 1);
        assert_eq!(host.counters.capture_bound, 1);
    }

    #[test]
    fn every_typed_soak_scenario_matches_its_deterministic_host_seam() {
        for scenario in ALL_SCENARIOS {
            let report = review_scene_soak_report(scenario);
            assert!(
                report.passed,
                "{}: expected {}, observed {}",
                report.scenario, report.expected_outcome, report.observed_outcome
            );
            assert_eq!(report.execution_mode, "deterministic-host-harness");
        }
    }

    #[test]
    fn surface_skip_variants_remain_skips_not_gpu_failures() {
        for (scenario, expected) in [
            (Scenario::SurfaceOutdated, "skip-outdated-retry-later"),
            (Scenario::SurfaceTimeout, "skip-timeout-retry-later"),
            (Scenario::SurfaceOccluded, "skip-occluded-retry-later"),
        ] {
            let report = review_scene_soak_report(scenario);
            assert!(report.passed);
            assert_eq!(report.observed_outcome, expected);
            assert_eq!(report.sanitized_category, None);
            assert_eq!(report.counters.skips, 1);
            assert_eq!(report.counters.fallbacks, 0);
        }
    }

    #[test]
    fn fatal_surface_and_device_cases_emit_only_static_sanitized_categories() {
        for (scenario, expected) in [
            (Scenario::SurfaceLost, "retained-surface-lost"),
            (Scenario::SurfaceValidation, "retained-surface-validation"),
            (Scenario::DeviceLoss, "retained-device-unavailable"),
            (Scenario::DeviceValidation, "retained-device-validation"),
            (Scenario::DeviceOutOfMemory, "retained-device-out-of-memory"),
        ] {
            let report = review_scene_soak_report(scenario);
            assert!(report.passed);
            assert_eq!(report.sanitized_category, Some(expected));
            assert_eq!(report.counters.fallbacks, 1);
            assert_eq!(report.counters.skips, 0);
        }
    }

    #[test]
    fn resize_scale_capture_hidden_shutdown_and_tick_counters_are_exact() {
        let resize = review_scene_soak_report(Scenario::ResizeStorm);
        assert_eq!(resize.counters.resize_requests, 4);
        assert_eq!(resize.counters.surface_reconfigurations, 4);

        let scale = review_scene_soak_report(Scenario::BackingScale);
        assert_eq!(scale.counters.resize_requests, 3);
        assert_eq!(scale.counters.surface_reconfigurations, 2);

        let capture = review_scene_soak_report(Scenario::CaptureSwap);
        assert_eq!(capture.counters.capture_bound, 2);
        assert_eq!(capture.counters.capture_deferred, 1);
        assert_eq!(capture.counters.presents, 1);

        let hidden = review_scene_soak_report(Scenario::HiddenSemanticReveal);
        assert_eq!(hidden.counters.hidden_updates_coalesced, 2);
        assert_eq!(hidden.counters.reveals, 1);

        let shutdown = review_scene_soak_report(Scenario::ShutdownWorker);
        assert_eq!(shutdown.counters.worker_cancel_requests, 1);
        assert_eq!(shutdown.counters.shutdowns, 1);

        let tracking = review_scene_soak_report(Scenario::TrackingRunLoop);
        assert_eq!(tracking.counters.virtual_elapsed_ms, 121_000);
        assert_eq!(tracking.counters.ticks_attempted, 484);
        assert_eq!(tracking.counters.ticks_completed, 484);

        let slow_tick = review_scene_soak_report(Scenario::SlowTick);
        assert_eq!(slow_tick.counters.ticks_attempted, 3);
        assert_eq!(slow_tick.counters.ticks_completed, 2);
        assert_eq!(slow_tick.counters.ticks_suppressed, 1);
    }

    #[test]
    fn report_json_names_native_interactions_that_the_host_harness_does_not_fake() {
        let mut output = Vec::new();
        run_review_scene_soak(Scenario::TrackingRunLoop, &mut output).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["scenario"], "tracking-run-loop");
        assert_eq!(json["passed"], true);
        assert_eq!(
            json["native_interactions_deferred"],
            serde_json::json!([
                "AppKit menu tracking loop",
                "AppKit live-resize tracking loop"
            ])
        );
    }
}

pub(in crate::companion) struct RetainedHost {
    // Surface must drop before the retained CAMetalLayer.
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) config: wgpu::SurfaceConfiguration,
    layer: Retained<CAMetalLayer>,
    pipelines: Pipelines,
    atlas_layout: wgpu::BindGroupLayout,
    pub(super) glyph_resources: Option<ActiveGlyphResources>,
    scene_build_worker: SceneBuildWorker,
    resource_preparation: ResourcePreparationController,
    failed_glyph_preparation: Option<FailedGlyphPreparation>,
    pub(super) frame_buffers: PersistentFrameBuffers,
    pub(super) capture_resources: Option<PersistentCaptureResources>,
    counters: RetainedResourceCounters,
    pub(super) physical_width: u32,
    pub(super) physical_height: u32,
    pub(super) backing_scale: f64,
    frame_counter: u64,
    activation_render_owner_us: u64,
    activation_recorded: bool,
    gpu_errors: GpuErrorMailbox,
    device_epoch: crate::presentation::companion_scene::DeviceEpoch,
    scene_config: Option<wgpu::SurfaceConfiguration>,
    configured_surface: ConfiguredSurface,
    #[allow(dead_code)] // Installed and driven by Task 14 without changing Task 12's live route.
    scene_activation: Option<RetainedSceneActivation>,
    pub(super) metrics: CompanionRuntimeMetrics,
    surface_epoch: crate::presentation::companion_scene::SurfaceEpoch,
    last_presented_scene:
        Option<crate::presentation::companion_scene::contract::PresentedSceneVersion>,
    presented_scene_count: u64,
    last_scene_presented_at: Option<Instant>,
    visible_present_interval_anchor: Option<Instant>,
    next_hud_revision: u64,
    last_presented_hud_text: Option<crate::round::hud::CompanionHudText>,
    last_presented_hud_font_size: Option<f64>,
    last_presented_state_alias:
        Option<crate::presentation::companion_scene::contract::CompanionCaptureStateAlias>,
    last_scene_activation_skip: Option<SkipReason>,
}

pub(super) struct Pipelines {
    pub(super) normal: wgpu::RenderPipeline,
    pub(super) multiply: wgpu::RenderPipeline,
    pub(super) screen: wgpu::RenderPipeline,
    pub(super) add: wgpu::RenderPipeline,
    pub(super) replace: wgpu::RenderPipeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Scene mode is selected only once Task 14 routes live frames here.
enum ConfiguredSurface {
    Legacy,
    Scene,
}

impl Pipelines {
    pub(super) fn get(&self, blend: SmoothBlendMode) -> &wgpu::RenderPipeline {
        match blend {
            SmoothBlendMode::Normal => &self.normal,
            SmoothBlendMode::Multiply => &self.multiply,
            SmoothBlendMode::Screen => &self.screen,
            SmoothBlendMode::Add => &self.add,
            SmoothBlendMode::Replace => &self.replace,
        }
    }
}

/// Tracks whether the Metal layer is installed on the AppKit view and whether
/// the view still holds its original AppKit layer state. The activation guard
/// consults it to decide whether a dropped, uncommitted activation must roll the
/// attach back.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct LayerActivationState {
    attached: bool,
    appkit_restored: bool,
}

impl LayerActivationState {
    /// Records that the Metal layer was installed on the view.
    fn mark_attached(&mut self) {
        self.attached = true;
        self.appkit_restored = false;
    }

    /// Records that preparation failed before the layer was ever installed, so
    /// the view keeps its original AppKit layer untouched.
    #[allow(dead_code)] // Models the never-attached invariant the activation-state tests pin.
    pub(super) fn preflight_failed(&mut self) {
        self.attached = false;
        self.appkit_restored = true;
    }

    pub(super) fn attached(&self) -> bool {
        self.attached
    }

    #[allow(dead_code)] // Read by the activation-state tests.
    pub(super) fn appkit_restored(&self) -> bool {
        self.appkit_restored
    }
}

/// Where a dropped, uncommitted activation guard sends its rollback.
enum ActivationRollback<'a> {
    /// Production rollback restores the view's prior AppKit layer state.
    View(&'a NSView),
    /// Test rollback clears an observable attachment flag.
    #[cfg(test)]
    TestFlag(std::rc::Rc<std::cell::Cell<bool>>),
}

/// RAII guard for the AppKit layer attach performed by
/// [`PreparedRetainedHost::activate`]. Dropping it before
/// [`LayerActivationGuard::commit`] rolls the attach back, so a failure after
/// the layer is installed never leaves the view half-attached.
pub(super) struct LayerActivationGuard<'a> {
    rollback: ActivationRollback<'a>,
    state: LayerActivationState,
    committed: bool,
}

impl<'a> LayerActivationGuard<'a> {
    /// Arms a guard that restores the view's prior AppKit layer state on drop
    /// until committed.
    fn install(view: &'a NSView) -> Self {
        let mut state = LayerActivationState::default();
        state.mark_attached();
        Self {
            rollback: ActivationRollback::View(view),
            state,
            committed: false,
        }
    }

    /// Test constructor whose rollback clears the supplied attachment flag.
    #[cfg(test)]
    pub(super) fn for_test(flag: std::rc::Rc<std::cell::Cell<bool>>) -> Self {
        let mut state = LayerActivationState::default();
        state.mark_attached();
        Self {
            rollback: ActivationRollback::TestFlag(flag),
            state,
            committed: false,
        }
    }

    /// Keeps the attach; the guard no longer rolls back on drop.
    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for LayerActivationGuard<'_> {
    fn drop(&mut self) {
        if self.committed || !self.state.attached() {
            return;
        }
        match &self.rollback {
            ActivationRollback::View(view) => ActiveRetainedHost::restore_appkit(view),
            #[cfg(test)]
            ActivationRollback::TestFlag(flag) => flag.set(false),
        }
    }
}

/// A fully built retained host whose Metal layer is not yet installed on the
/// view. All fallible GPU work is done; [`PreparedRetainedHost::activate`] is the
/// only step that touches the AppKit view.
pub(in crate::companion) struct PreparedRetainedHost {
    host: RetainedHost,
}

/// A retained host whose Metal layer is installed on the view and rendering.
/// Derefs to the inner [`RetainedHost`] so `render` and the mailbox drain read
/// through transparently.
pub(in crate::companion) struct ActiveRetainedHost {
    host: RetainedHost,
}

pub(crate) struct DirectSceneCapture {
    pub(crate) receipt: crate::presentation::companion_scene::contract::PresentedSceneVersion,
    pub(crate) source: crate::presentation::companion_scene::contract::CaptureSourceIdentity,
    pub(crate) scene_artifacts: crate::presentation::companion_scene::contract::SceneArtifacts,
    pub(crate) logical_state_alias:
        crate::presentation::companion_scene::contract::CompanionCaptureStateAlias,
    pub(crate) rgba: Vec<u8>,
    pub(crate) presented_scene_count: u64,
    pub(crate) last_present_age_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Returned by the Task 12 production seam once Task 14 calls it.
pub(in crate::companion) enum SceneActivationError {
    NoSceneRuntime,
    UnsupportedSurfaceContract,
    Start(crate::presentation::companion_scene::runtime::ActivationStartError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Returned by the Task 12 production seam once Task 14 calls it.
pub(in crate::companion) enum SceneGenerationServiceTick {
    Idle,
    Preparing,
    CandidateReady,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::companion) enum ScenePresentOutcome {
    Presented(crate::presentation::companion_scene::SceneVersion),
    Skipped(SkipReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::companion) struct SceneActivationOutcome {
    pub(in crate::companion) disposition:
        crate::presentation::companion_scene::runtime::RuntimeDisposition,
    pub(in crate::companion) skipped: Option<SkipReason>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SurfaceContractChange {
    epoch: crate::presentation::companion_scene::SurfaceEpoch,
    scale_changed: bool,
}

impl PreparedRetainedHost {
    /// Builds the CAMetalLayer, wgpu surface, device, configuration, and
    /// pipelines against a layer that stays detached from the view. A
    /// CAMetalLayer renders fine while detached; installing it on the view later
    /// is what makes it visible. Any failure here leaves the view untouched.
    pub(in crate::companion) fn prepare(
        view: &NSView,
        mailbox: GpuErrorMailbox,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let scene_build_worker = SceneBuildWorker::launch()
            .map_err(|_| RetainedFailureCategory::RasterWorkerUnavailable)?;
        let window = view
            .window()
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        let scale = window.backingScaleFactor();
        let bounds = view.bounds();
        let width = physical_dimension(bounds.size.width, scale);
        let height = physical_dimension(bounds.size.height, scale);
        let layer = unsafe { CAMetalLayer::new() };
        unsafe {
            layer.setDrawableSize(NSSize::new(f64::from(width), f64::from(height)));
        }

        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let layer_pointer = std::ptr::from_ref(layer.as_ref())
            .cast_mut()
            .cast::<c_void>();
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_pointer))
        }
        .map_err(|_| RetainedFailureCategory::SurfaceCreate)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|_| RetainedFailureCategory::AdapterUnavailable)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glorp-retained-device"),
            ..Default::default()
        }))
        .map_err(|_| RetainedFailureCategory::DeviceUnavailable)?;
        let device_epoch = crate::presentation::companion_scene::DeviceEpoch(1);
        let gpu_error_sender = mailbox.sender_for(device_epoch);
        device.on_uncaptured_error(Arc::new(move |error| {
            let category = match error {
                wgpu::Error::OutOfMemory { .. } => RetainedFailureCategory::DeviceOutOfMemory,
                wgpu::Error::Validation { .. } => RetainedFailureCategory::DeviceValidation,
                wgpu::Error::Internal { .. } => RetainedFailureCategory::DeviceInternal,
            };
            let _ = gpu_error_sender.send(category);
        }));
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        let scene_surface_contract =
            render::SceneSurfaceContract::select(&surface.get_capabilities(&adapter)).ok();
        // Composite in gamma space to match CoreGraphics/Smooth: a linear
        // (non-sRGB) target blends the stored premultiplied-sRGB values directly,
        // with no sRGB→linear→sRGB round-trip. The default surface format is the
        // sRGB variant; drop the sRGB suffix so the raw sRGB-space values are what
        // get blended. Metal's CAMetalLayer surface supports both variants.
        config.format = config.format.remove_srgb_suffix();
        surface.configure(&device, &config);
        let scene_config = scene_surface_contract.map(|contract| {
            let mut scene = config.clone();
            scene.format = contract.format;
            scene.color_space = contract.color_space;
            scene.alpha_mode = contract.alpha_mode;
            scene
        });
        let mut counters = RetainedResourceCounters::default();
        let atlas_layout = create_atlas_bind_group_layout(&device);
        let pipelines = create_pipelines(&device, config.format, &atlas_layout, &mut counters);
        let frame_buffers = PersistentFrameBuffers::new(&device);
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.discard_initial_visible_ticks(20);
        metrics.replace_gpu_allocation(
            GpuAllocationKind::HostInfrastructure,
            0,
            resource_object_count(counters),
        );
        Ok(Self {
            host: RetainedHost {
                surface,
                device,
                queue,
                config,
                layer,
                pipelines,
                atlas_layout,
                glyph_resources: None,
                scene_build_worker,
                resource_preparation: ResourcePreparationController::new(),
                failed_glyph_preparation: None,
                frame_buffers,
                capture_resources: None,
                counters,
                physical_width: width,
                physical_height: height,
                backing_scale: scale,
                frame_counter: 0,
                activation_render_owner_us: 0,
                activation_recorded: false,
                gpu_errors: mailbox,
                device_epoch,
                scene_config,
                configured_surface: ConfiguredSurface::Legacy,
                scene_activation: None,
                metrics,
                surface_epoch: crate::presentation::companion_scene::SurfaceEpoch(1),
                last_presented_scene: None,
                presented_scene_count: 0,
                last_scene_presented_at: None,
                visible_present_interval_anchor: None,
                next_hud_revision: 1,
                last_presented_hud_text: None,
                last_presented_hud_font_size: None,
                last_presented_state_alias: None,
                last_scene_activation_skip: None,
            },
        })
    }

    /// Installs the Metal layer on the view under a rollback guard. This is the
    /// only code that calls `setWantsLayer`/`setLayer`; if a fallible post-attach
    /// step is ever added and fails, the dropped guard restores the view's prior
    /// AppKit layer state before the error propagates.
    pub(in crate::companion) fn activate(
        self,
        view: &NSView,
    ) -> std::result::Result<ActiveRetainedHost, RetainedFailureCategory> {
        let guard = LayerActivationGuard::install(view);
        view.setWantsLayer(true);
        unsafe { view.setLayer(Some(&self.host.layer)) };
        guard.commit();
        Ok(ActiveRetainedHost { host: self.host })
    }
}

impl ActiveRetainedHost {
    pub(in crate::companion) fn install_scene_runtime(
        &mut self,
        snapshot: Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    ) -> Result<SceneGenerationServiceTick, RetainedFailureCategory> {
        use crate::presentation::companion_scene::runtime::ResourceInvalidation;

        if self.host.scene_build_worker.is_busy() {
            return Err(RetainedFailureCategory::RasterWorkerUnavailable);
        }
        let runtime = crate::presentation::companion_scene::runtime::CompanionSceneRuntimeState::cold_start_on_surface(
            snapshot,
            self.host.surface_epoch,
        )
        .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        let mut generations = RetainedSceneGenerationState::new(runtime);
        let effects = generations
            .invalidate_resources(ResourceInvalidation::BackingScaleAtlas)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        let mut activation = RetainedSceneActivation { generations, gpu: None };
        self.host
            .apply_scene_runtime_effects(&mut activation, effects)?;
        self.host.scene_activation = Some(activation);
        Ok(SceneGenerationServiceTick::Preparing)
    }

    pub(in crate::companion) fn has_scene_runtime(&self) -> bool {
        self.host.scene_activation.is_some()
    }

    pub(in crate::companion) fn scene_has_active_generation(&self) -> bool {
        self.host
            .scene_activation
            .as_ref()
            .and_then(|activation| activation.generations.active_version())
            .is_some()
    }

    pub(in crate::companion) fn scene_active_delta_pending(&self) -> bool {
        self.host
            .scene_activation
            .as_ref()
            .is_some_and(|activation| activation.generations.active_delta_pending())
    }

    pub(in crate::companion) fn project_scene_frame(
        &mut self,
        clock: crate::presentation::companion_scene::CompanionProjectionClock,
        options: crate::presentation::companion_scene::input::CompanionPresentationOptions,
    ) -> Result<
        crate::presentation::companion_scene::CompanionFrameProjection,
        RetainedFailureCategory,
    > {
        let started_at = Instant::now();
        let activation = self
            .host
            .scene_activation
            .as_ref()
            .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
        let projection = activation
            .generations
            .project_frame(clock, options)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        self.host.metrics.record_snapshot_projection();
        self.host
            .metrics
            .record_projection_us(duration_us(started_at.elapsed()));
        Ok(projection)
    }

    pub(in crate::companion) fn record_scene_snapshot_projection(&mut self, elapsed_us: u32) {
        self.host.metrics.record_snapshot_projection();
        self.host.metrics.record_projection_us(elapsed_us);
    }

    pub(in crate::companion) fn reconcile_scene_snapshot(
        &mut self,
        view: &NSView,
        snapshot: Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    ) -> Result<SceneGenerationServiceTick, RetainedFailureCategory> {
        let started_at = Instant::now();
        if self.host.scene_activation.is_none() {
            let _ = self.host.resize_surface_if_needed(view)?;
            self.host.metrics.record_semantic_reconcile();
            let result = self.install_scene_runtime(snapshot);
            self.host
                .metrics
                .record_reconcile_us(duration_us(started_at.elapsed()));
            return result;
        }
        let surface_change = self.host.resize_surface_if_needed(view)?;
        let mut activation = self
            .host
            .scene_activation
            .take()
            .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
        let result = (|| {
            if let Some(change) = surface_change {
                let effects = activation.generations.rebind_surface(change.epoch)?;
                self.host
                    .apply_scene_runtime_effects(&mut activation, effects)?;
                self.host.clear_presented_scene_receipt();
            }
            let effects = activation.generations.reconcile_snapshot(
                snapshot,
                surface_change.is_some_and(|change| change.scale_changed),
            )?;
            let unchanged = matches!(
                effects.disposition(),
                crate::presentation::companion_scene::runtime::RuntimeDisposition::Unchanged
            );
            self.host.metrics.record_semantic_reconcile();
            if unchanged {
                self.host.metrics.record_unchanged_tick();
            }
            self.host
                .apply_scene_runtime_effects(&mut activation, effects)?;
            Ok(SceneGenerationServiceTick::Preparing)
        })();
        self.host.scene_activation = Some(activation);
        self.host
            .metrics
            .record_reconcile_us(duration_us(started_at.elapsed()));
        result
    }

    pub(in crate::companion) fn reconcile_scene_frame(
        &mut self,
        view: &NSView,
        projection: crate::presentation::companion_scene::CompanionFrameProjection,
        defer_surface_resize: bool,
    ) -> Result<SceneGenerationServiceTick, RetainedFailureCategory> {
        let started_at = Instant::now();
        let surface_change = if defer_surface_resize {
            None
        } else {
            self.host.resize_surface_if_needed(view)?
        };
        let mut activation = self
            .host
            .scene_activation
            .take()
            .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
        let result = (|| {
            if let Some(change) = surface_change {
                let effects = activation.generations.rebind_surface(change.epoch)?;
                self.host
                    .apply_scene_runtime_effects(&mut activation, effects)?;
                self.host.clear_presented_scene_receipt();
            }
            let (effects, regenerated) = activation.generations.reconcile_frame_projection(
                projection,
                surface_change.is_some_and(|change| change.scale_changed),
            )?;
            if regenerated {
                self.host.metrics.record_snapshot_projection();
            }
            let unchanged = matches!(
                effects.disposition(),
                crate::presentation::companion_scene::runtime::RuntimeDisposition::Unchanged
            );
            self.host.metrics.record_frame_reconcile();
            if unchanged {
                self.host.metrics.record_unchanged_tick();
            }
            self.host
                .apply_scene_runtime_effects(&mut activation, effects)?;
            Ok(SceneGenerationServiceTick::Preparing)
        })();
        self.host.scene_activation = Some(activation);
        self.host
            .metrics
            .record_reconcile_us(duration_us(started_at.elapsed()));
        result
    }

    pub(in crate::companion) fn hide_scene_runtime(
        &mut self,
    ) -> Result<(), RetainedFailureCategory> {
        self.host.visible_present_interval_anchor = None;
        let Some(mut activation) = self.host.scene_activation.take() else {
            return Ok(());
        };
        let effects = activation.generations.set_hidden();
        let result = self
            .host
            .apply_scene_runtime_effects(&mut activation, effects);
        self.host.scene_activation = Some(activation);
        result
    }

    pub(in crate::companion) fn reveal_scene_runtime(
        &mut self,
        view: &NSView,
        snapshot: Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    ) -> Result<bool, RetainedFailureCategory> {
        let Some(mut activation) = self.host.scene_activation.take() else {
            return Ok(true);
        };
        let surface_change = self.host.resize_surface_if_needed(view)?;
        let result = (|| {
            if activation.generations.active_delta_pending() {
                return Ok(false);
            }
            let coalesced = activation
                .generations
                .coalesce_hidden_snapshot(snapshot)
                .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
            self.host.metrics.record_semantic_reconcile();
            self.host
                .apply_scene_runtime_effects(&mut activation, coalesced)?;
            if let Some(change) = surface_change {
                let effects = activation.generations.rebind_surface(change.epoch)?;
                self.host
                    .apply_scene_runtime_effects(&mut activation, effects)?;
                self.host.clear_presented_scene_receipt();
            }
            let effects = activation
                .generations
                .reveal(surface_change.is_some_and(|change| change.scale_changed))?;
            self.host
                .apply_scene_runtime_effects(&mut activation, effects)?;
            Ok(true)
        })();
        self.host.scene_activation = Some(activation);
        result
    }

    pub(in crate::companion) fn retry_scene_replacement(
        &mut self,
    ) -> Result<(), RetainedFailureCategory> {
        let Some(mut activation) = self.host.scene_activation.take() else {
            return Err(RetainedFailureCategory::SceneCandidateEncode);
        };
        let result = (|| {
            let effects = activation.generations.retry_current_generation()?;
            self.host
                .apply_scene_runtime_effects(&mut activation, effects)
        })();
        if result.is_ok() {
            self.host.metrics.record_generation_retry();
        }
        self.host.scene_activation = Some(activation);
        result
    }

    pub(in crate::companion) fn scene_active_is_present_compatible(&self) -> bool {
        self.host
            .scene_activation
            .as_ref()
            .is_some_and(|activation| {
                activation.generations.active_present_compatible(
                    self.host.surface_epoch,
                    [self.host.physical_width, self.host.physical_height],
                    self.host.backing_scale,
                )
            })
    }

    pub(in crate::companion) fn scene_replacement_identity(
        &self,
    ) -> Option<(SceneReplacementIdentity, u64)> {
        self.host.scene_activation.as_ref().and_then(|activation| {
            activation
                .generations
                .replacement_identity()
                .map(|identity| (identity, self.host.backing_scale.to_bits()))
        })
    }

    pub(in crate::companion) fn shutdown_scene_runtime(&mut self) {
        if let Some(mut activation) = self.host.scene_activation.take() {
            let effects = activation.generations.shutdown();
            let _ = self
                .host
                .apply_scene_runtime_effects(&mut activation, effects);
        }
    }

    pub(in crate::companion) fn advance_scene_generation(
        &mut self,
        materialize_candidate: bool,
    ) -> Result<SceneGenerationServiceTick, RetainedFailureCategory> {
        let started_at = Instant::now();
        let result = self.host.advance_scene_generation();
        self.host
            .metrics
            .record_generation_service_ui_us(duration_us(started_at.elapsed()));
        let tick = result?;
        if materialize_candidate && tick == SceneGenerationServiceTick::CandidateReady {
            self.host.materialize_scene_candidate()?;
        }
        // Candidate readiness is a level, not a one-tick worker-reply edge. A
        // drawable can be temporarily unavailable during the first activation
        // attempt (occluded, outdated, or timed out); the coordinator deliberately
        // retains that GPU-ready candidate for RetryLater. Keep returning
        // CandidateReady until activation commits or rejects it so a later visible
        // tick retries the exact candidate without rasterizing or materializing it
        // again.
        Ok(
            if materialize_candidate
                && self
                    .host
                    .scene_activation
                    .as_ref()
                    .is_some_and(|activation| activation.generations.has_ready_candidate())
            {
                SceneGenerationServiceTick::CandidateReady
            } else {
                tick
            },
        )
    }

    pub(in crate::companion) fn activate_candidate(
        &mut self,
        hud_text: &crate::round::hud::CompanionHudText,
        hud_font_size: f64,
        presentation_privacy:
            crate::presentation::companion_scene::contract::PresentedCapturePrivacy,
    ) -> Result<SceneActivationOutcome, SceneActivationError> {
        // Measure the complete render-owner transaction, including every
        // nonblocking acquisition retry, until the candidate reaches its first
        // real surface present. The legacy path records the same boundary in
        // `RetainedHost::render`; the shared one-shot fields keep the two routes
        // mutually exclusive.
        let activation_attempt_started = (!self.host.activation_recorded).then(Instant::now);
        let mut activation = self
            .host
            .scene_activation
            .take()
            .ok_or(SceneActivationError::NoSceneRuntime)?;
        let result = (|| {
            let version = activation
                .generations
                .ready_candidate
                .as_ref()
                .map(|candidate| candidate.version)
                .ok_or(SceneActivationError::Start(
                    crate::presentation::companion_scene::runtime::ActivationStartError::NoReadyCandidate,
                ))?;
            let geometry = self.host.scene_hud_geometry(
                version.generation.resources,
                hud_font_size,
                activation
                    .generations
                    .ready_hud_depth_composition()
                    .map_err(|_| SceneActivationError::UnsupportedSurfaceContract)?,
            );
            let prepared_hud = activation
                .generations
                .prepare_ready_hud(hud_text, geometry)
                .map_err(|_| SceneActivationError::UnsupportedSurfaceContract)?;
            let gpu = activation
                .gpu
                .as_mut()
                .ok_or(SceneActivationError::UnsupportedSurfaceContract)?;
            let effects = self.host.activate_candidate(
                &mut activation.generations,
                &mut gpu.renderer,
                &gpu.shared,
                &prepared_hud,
            )?;
            let disposition = effects.disposition();
            let skipped = self.host.last_scene_activation_skip.take();
            self.host
                .apply_scene_runtime_effects(&mut activation, effects)
                .map_err(|_| SceneActivationError::UnsupportedSurfaceContract)?;
            if disposition
                == crate::presentation::companion_scene::runtime::RuntimeDisposition::Activation(
                    crate::presentation::companion_scene::runtime::ActivationTransition::Committed,
                )
            {
                let version = activation
                    .generations
                    .active_version()
                    .ok_or(SceneActivationError::UnsupportedSurfaceContract)?;
                let logical_state_alias = activation
                    .generations
                    .runtime
                    .capture_lease()
                    .ok()
                    .filter(|lease| lease.version() == version)
                    .map(|lease| lease.logical_state_alias())
                    .ok_or(SceneActivationError::UnsupportedSurfaceContract)?;
                self.host
                    .record_scene_presented(
                        version,
                        hud_text,
                        hud_font_size,
                        presentation_privacy,
                        logical_state_alias,
                    )
                    .map_err(|_| SceneActivationError::UnsupportedSurfaceContract)?;
            }
            Ok(SceneActivationOutcome { disposition, skipped })
        })();
        self.host.scene_activation = Some(activation);
        if let Some(started_at) = activation_attempt_started {
            self.host.activation_render_owner_us = self
                .host
                .activation_render_owner_us
                .saturating_add(u64::from(duration_us(started_at.elapsed())));
            if result.as_ref().is_ok_and(|outcome| {
                outcome.disposition
                    == crate::presentation::companion_scene::runtime::RuntimeDisposition::Activation(
                        crate::presentation::companion_scene::runtime::ActivationTransition::Committed,
                    )
            }) {
                let activation_us = self
                    .host
                    .activation_render_owner_us
                    .min(u64::from(u32::MAX)) as u32;
                self.host
                    .metrics
                    .record_activation_render_owner_us(activation_us);
                self.host.activation_recorded = true;
            }
        }
        result
    }

    pub(in crate::companion) fn present_active_scene(
        &mut self,
        view: &NSView,
        hud_text: &crate::round::hud::CompanionHudText,
        hud_font_size: f64,
        presentation_privacy:
            crate::presentation::companion_scene::contract::PresentedCapturePrivacy,
    ) -> Result<ScenePresentOutcome, RetainedFailureCategory> {
        let mut activation = self
            .host
            .scene_activation
            .take()
            .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
        let version = activation
            .generations
            .active_version()
            .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
        let geometry = self.host.scene_hud_geometry(
            version.generation.resources,
            hud_font_size,
            activation
                .generations
                .active_hud_depth_composition()
                .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?,
        );
        let prepared_hud = activation
            .generations
            .prepare_active_hud(hud_text, geometry)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        let gpu = activation
            .gpu
            .as_mut()
            .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
        let result = self.host.present_active_scene(
            view,
            &mut activation.generations,
            &mut gpu.renderer,
            &gpu.shared,
            &prepared_hud,
        );
        if let Ok(ScenePresentOutcome::Presented(version)) = result {
            let logical_state_alias = activation
                .generations
                .runtime
                .capture_lease()
                .ok()
                .filter(|lease| lease.version() == version)
                .map(|lease| lease.logical_state_alias())
                .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
            self.host.record_scene_presented(
                version,
                hud_text,
                hud_font_size,
                presentation_privacy,
                logical_state_alias,
            )?;
        }
        self.host.scene_activation = Some(activation);
        result
    }

    /// Restores the view's prior AppKit layer state. Idempotent, so a redundant
    /// call after fallback is harmless.
    pub(in crate::companion) fn restore_appkit(view: &NSView) {
        unsafe { view.setLayer(None) };
        view.setWantsLayer(false);
        unsafe { view.setNeedsDisplay(true) };
    }

    /// Renders the frozen paired-review frame into an off-screen intermediate and
    /// reads it back as a [`CanonicalRgbaFrame`]. Reuses the live host's
    /// device/queue/pipelines so the capture rasterizes with the identical
    /// pipeline as the on-screen present.
    pub(crate) fn capture(
        &mut self,
        frame: &crate::companion::paired_review::PairedReviewFrame,
    ) -> std::result::Result<CanonicalRgbaFrame, RetainedFailureCategory> {
        self.host.metrics.record_capture_attempt();
        let started_at = Instant::now();
        let result = capture::RetainedCaptureTarget::new(&mut self.host).capture(frame);
        self.host
            .metrics
            .record_capture_us(duration_us(started_at.elapsed()));
        if result.is_ok() {
            self.host.metrics.record_capture_success();
        } else {
            self.host.metrics.record_capture_failure();
        }
        result
    }

    pub(crate) fn capture_presented_scene(
        &mut self,
        sensitive_live_values: bool,
    ) -> std::result::Result<DirectSceneCapture, RetainedFailureCategory> {
        self.host.metrics.record_capture_attempt();
        let started_at = Instant::now();
        let capture = self.capture_presented_scene_inner(sensitive_live_values);
        let mailbox = self.host.drain_current_gpu_error();
        let result = capture.and_then(|capture| mailbox.map(|()| capture));
        self.host
            .metrics
            .record_capture_us(duration_us(started_at.elapsed()));
        result
    }

    pub(crate) fn record_direct_capture_success(&mut self) {
        self.host.metrics.record_capture_nonblank_validated();
        self.host.metrics.record_capture_success();
    }

    pub(crate) fn record_direct_capture_failure(&mut self) {
        self.host.metrics.record_capture_failure();
    }

    fn capture_presented_scene_inner(
        &mut self,
        sensitive_live_values: bool,
    ) -> std::result::Result<DirectSceneCapture, RetainedFailureCategory> {
        let receipt = self
            .host
            .last_presented_scene
            .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?;
        let hud_text = self
            .host
            .last_presented_hud_text
            .as_ref()
            .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?;
        let hud_font_size = self
            .host
            .last_presented_hud_font_size
            .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?;
        let logical_state_alias = self
            .host
            .last_presented_state_alias
            .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?;
        let expected_privacy = if sensitive_live_values {
            crate::presentation::companion_scene::contract::PresentedCapturePrivacy::SensitiveLiveValues
        } else {
            crate::presentation::companion_scene::contract::PresentedCapturePrivacy::ExternalRedacted
        };
        if receipt.privacy != expected_privacy {
            return Err(RetainedFailureCategory::CaptureUnsupportedVariant);
        }
        let geometry = self.host.scene_hud_geometry(
            receipt.scene_version.generation.resources,
            hud_font_size,
            self.host
                .scene_activation
                .as_ref()
                .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?
                .generations
                .active_hud_depth_composition()
                .map_err(|_| RetainedFailureCategory::CaptureUnsupportedVariant)?,
        );
        let activation = self
            .host
            .scene_activation
            .as_mut()
            .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?;
        let gpu = activation
            .gpu
            .as_mut()
            .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?;
        let active = activation
            .generations
            .active
            .as_ref()
            .ok_or(RetainedFailureCategory::CaptureUnsupportedVariant)?;
        if active.version != receipt.scene_version
            || active.gpu.source_revisions != receipt.scene_version.applied
        {
            return Err(RetainedFailureCategory::CaptureUnsupportedVariant);
        }
        let source = active
            .cpu
            .capture_source_identity()
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        let scene_artifacts = active
            .cpu
            .scene_artifacts()
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        let capture_cpu = if sensitive_live_values {
            active.cpu.clone()
        } else {
            active
                .cpu
                .capture_safe_clone()
                .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?
        };
        let upload = render::prepare_scene_upload(&capture_cpu, &active.atlas)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        let mut capture_gpu = render::materialize_gpu_candidate(
            &self.host.device,
            &self.host.queue,
            &gpu.shared,
            &upload,
            &active.atlas,
        )
        .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        let request = render::SceneRenderRequest::new(
            receipt.scene_version,
            receipt.physical_pixels,
            f64::from(receipt.backing_scale),
        );
        let outcome = if sensitive_live_values {
            let sealed = hud::SealedHudFrame::from_live(hud_text)
                .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
            let prepared = capture_gpu
                .hud
                .prepared_atlas()
                .prepare_sensitive(&sealed, geometry)
                .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
            gpu.renderer.render_offscreen_sensitive(
                &self.host.device,
                &self.host.queue,
                &gpu.shared,
                &mut capture_gpu,
                request,
                &prepared,
            )
        } else {
            let sealed =
                hud::SealedHudFrame::<hud::RedactedCaptureHudProjection>::redacted_capture()
                    .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
            let prepared = capture_gpu
                .hud
                .prepared_atlas()
                .prepare_redacted_capture(&sealed, geometry)
                .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
            gpu.renderer.render_offscreen(
                &self.host.device,
                &self.host.queue,
                &gpu.shared,
                &mut capture_gpu,
                request,
                &prepared,
            )
        }
        .map_err(scene_render_failure)?;
        if outcome.version != receipt.scene_version
            || outcome.physical_extent_pixels != receipt.physical_pixels
        {
            return Err(RetainedFailureCategory::CaptureBufferTooShort);
        }
        let last_present_age_ms = self
            .host
            .last_scene_presented_at
            .map(|presented| u64::try_from(presented.elapsed().as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX);
        Ok(DirectSceneCapture {
            receipt,
            source,
            scene_artifacts,
            logical_state_alias,
            rgba: outcome.rgba,
            presented_scene_count: self.host.presented_scene_count,
            last_present_age_ms,
        })
    }

    pub(crate) fn record_injected_capture_failure(&mut self) {
        self.host.metrics.record_capture_attempt();
        self.host.metrics.record_capture_failure();
    }

    pub(crate) fn record_injected_capture_attempt(&mut self) {
        self.host.metrics.record_capture_attempt();
    }

    pub(crate) fn prewarm_capture_resources(&mut self) {
        let (width, height) = self.physical_size();
        self.host.ensure_capture_resources(width, height);
    }

    /// The physical-pixel drawable size the retained surface is configured for.
    pub(crate) fn physical_size(&self) -> (u32, u32) {
        (self.host.physical_width, self.host.physical_height)
    }

    /// The window backing scale the host resolved its physical size from.
    pub(crate) fn backing_scale(&self) -> f64 {
        self.host.backing_scale
    }

    /// The id of the next frame the host would render. A paired capture stamps
    /// this onto the frozen review frame so both artifacts share one id.
    pub(crate) fn current_frame_id(&self) -> u64 {
        self.host.frame_counter
    }

    /// The resource generation the host currently renders against — the hash of
    /// the active pet's declared content, repertoire, and font policy. Zero before
    /// the first generation is compiled.
    pub(crate) fn current_resource_generation(&self) -> u64 {
        self.host
            .glyph_resources
            .as_ref()
            .map(|active| active.resources.generation().value())
            .unwrap_or(0)
    }

    pub(crate) fn advance_resource_preparation(
        &mut self,
        identity: &CompanionContentIdentity,
        desired_backing_scale: f64,
    ) -> ResourcePreparationTick {
        self.host
            .advance_resource_preparation(identity, desired_backing_scale)
    }

    pub(crate) fn suspend_resource_preparation(
        &mut self,
        identity: &CompanionContentIdentity,
        desired_backing_scale: f64,
    ) {
        self.host
            .suspend_resource_preparation(identity, desired_backing_scale);
    }

    pub(crate) fn backing_scale_for_resource_preparation(
        view: &NSView,
    ) -> std::result::Result<f64, RetainedFailureCategory> {
        view.window()
            .map(|window| window.backingScaleFactor())
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)
    }

    pub(crate) fn active_identity_for_resource_preparation(
        &self,
        desired_identity: &CompanionContentIdentity,
        desired_backing_scale: f64,
    ) -> Option<CompanionContentIdentity> {
        self.host.glyph_resources.as_ref().and_then(|active| {
            (active.identity != *desired_identity
                || active.backing_scale.to_bits() != desired_backing_scale.to_bits())
            .then(|| active.identity.clone())
        })
    }

    pub(crate) fn record_resource_preparation_skip(&mut self) -> FrameProgress {
        self.host.record_resource_preparation_skip()
    }

    pub(crate) fn record_ui_tick_us(&mut self, value: u32) {
        let started_at = Instant::now();
        self.host.metrics.record_ui_tick_us(value);
        self.host
            .metrics
            .record_metrics_overhead(started_at.elapsed());
    }

    pub(crate) fn begin_visible_tick(&mut self) {
        self.host.metrics.begin_visible_tick();
    }

    pub(crate) fn record_state_prepare_us(&mut self, value: u32) {
        let started_at = Instant::now();
        self.host.metrics.record_state_prepare_us(value);
        self.host
            .metrics
            .record_metrics_overhead(started_at.elapsed());
    }

    pub(crate) fn runtime_work_counters(&self) -> RuntimeWorkCounters {
        self.host.metrics.work_counters()
    }

    #[allow(clippy::too_many_arguments)] // Frozen dual-cadence protocol plus projection callback and HUD contract.
    pub(crate) fn run_direct_lifetime_audit(
        &mut self,
        view: &NSView,
        starts_hidden: bool,
        semantic_samples: u64,
        presentation_ticks: u64,
        virtual_elapsed_ms: u64,
        semantic: impl FnMut(
            LifetimeAuditPhase,
            u64,
            time::OffsetDateTime,
        ) -> std::result::Result<
            Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
            RetainedFailureCategory,
        >,
        hud: crate::round::hud::CompanionHudText,
        hud_font_size: f64,
        reduce_motion: bool,
    ) -> std::result::Result<(), RetainedFailureCategory> {
        let surface_change = self.host.resize_surface_if_needed(view)?;
        let extent = [self.host.physical_width, self.host.physical_height];
        let scale = self.host.backing_scale;
        let mut activation = self
            .host
            .scene_activation
            .take()
            .ok_or(RetainedFailureCategory::LifetimeFramePreparation)?;
        let result = (|| {
            let gpu = activation
                .gpu
                .as_mut()
                .ok_or(RetainedFailureCategory::LifetimeFramePreparation)?;
            // Prewarm persistent target/readback storage without submitting a
            // frame. Hidden reveal and its first physical delta are owned by the
            // counted warmup sample/tick below, never by setup work.
            activation
                .generations
                .prewarm_offscreen_readback(
                    &mut gpu.renderer,
                    &self.host.device,
                    &gpu.shared,
                    extent,
                    scale,
                )
                .map_err(scene_render_failure)?;
            let (direct_bytes, direct_objects) = gpu.renderer.offscreen_cache_allocation();
            let legacy_bytes = self
                .host
                .capture_resources
                .as_ref()
                .map(|capture| {
                    u64::from(capture.width)
                        .saturating_mul(u64::from(capture.height))
                        .saturating_mul(4)
                        .saturating_add(capture::staging_buffer_size(capture.width, capture.height))
                })
                .unwrap_or(0);
            let legacy_objects = u64::from(self.host.capture_resources.is_some()) * 2;
            self.host.metrics.replace_gpu_allocation(
                GpuAllocationKind::Capture,
                legacy_bytes.saturating_add(direct_bytes),
                legacy_objects.saturating_add(direct_objects),
            );
            let mut executor = DirectLifetimeAuditExecutor {
                host: &mut self.host,
                activation: &mut activation,
                semantic,
                hud,
                hud_font_size,
                reduce_motion,
                last_submission: None,
                pending_semantic: None,
                reveal_pending: starts_hidden,
                reveal_submission_pending: false,
                surface_change,
            };
            let audit = run_lifetime_schedule(
                &mut executor,
                semantic_samples,
                presentation_ticks,
                virtual_elapsed_ms,
            )?;
            executor.host.metrics.record_lifetime_audit(audit);
            Ok(())
        })();
        self.host.scene_activation = Some(activation);
        result
    }

    pub(crate) fn record_lifetime_terminal_capture(&mut self, succeeded: bool) {
        self.host
            .metrics
            .record_lifetime_terminal_capture(succeeded);
    }

    pub(crate) fn record_hidden_tick(&mut self, tick_start: RuntimeWorkCounters) {
        self.host.metrics.record_hidden_tick(tick_start);
    }

    pub(crate) fn runtime_metrics_snapshot(
        &self,
        inventory: CompanionCapacityInventory,
    ) -> CompanionRuntimeMetricsSnapshot {
        let scene_version = self
            .host
            .scene_activation
            .as_ref()
            .and_then(|activation| activation.generations.metrics_version());
        let legacy_resource_generation = self.current_resource_generation();
        self.host.metrics.snapshot(
            RuntimeIdentity {
                device_epoch: scene_version.map(|version| version.generation.device.0),
                surface_epoch: scene_version
                    .map(|version| version.surface.0)
                    .unwrap_or(self.host.surface_epoch.0),
                layout_generation: scene_version.map(|version| version.generation.layout.0),
                resource_generation: scene_version
                    .map(|version| version.generation.resources.0)
                    .or_else(|| {
                        (legacy_resource_generation != 0).then_some(legacy_resource_generation)
                    }),
                semantic_revision: scene_version.map(|version| version.applied.semantic.0),
                frame_revision: scene_version.map(|version| version.applied.frame.0),
                present_attempt: self.host.frame_counter,
            },
            inventory,
            RuntimeFixtureIdentity {
                fixture_id: "glorp-scene-baseline-v2",
                seed: "glorp-scene-baseline-v1",
                update_source: "fixed-initial-state-no-live-polling",
                semantic_cadence_ms: 250,
                logical_width: f64::from(self.host.physical_width) / self.host.backing_scale,
                logical_height: f64::from(self.host.physical_height) / self.host.backing_scale,
                physical_width: self.host.physical_width,
                physical_height: self.host.physical_height,
                backing_scale: self.host.backing_scale,
            },
        )
    }
}

struct DirectLifetimeAuditExecutor<'a, Semantic> {
    host: &'a mut RetainedHost,
    activation: &'a mut RetainedSceneActivation,
    semantic: Semantic,
    hud: crate::round::hud::CompanionHudText,
    hud_font_size: f64,
    reduce_motion: bool,
    last_submission: Option<wgpu::SubmissionIndex>,
    pending_semantic: Option<Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>>,
    reveal_pending: bool,
    reveal_submission_pending: bool,
    surface_change: Option<SurfaceContractChange>,
}

impl<Semantic> DirectLifetimeAuditExecutor<'_, Semantic> {
    fn reconcile_pending_semantic(
        &mut self,
        now: time::OffsetDateTime,
    ) -> std::result::Result<bool, RetainedFailureCategory> {
        let Some(mut snapshot) = self.pending_semantic.take() else {
            return Ok(false);
        };
        let projection = snapshot
            .project_presentation_frame(
                self.activation
                    .generations
                    .runtime
                    .applied_revisions()
                    .semantic,
                crate::presentation::companion_scene::CompanionProjectionClock::new(
                    now,
                    lifetime_elapsed_ms(now),
                ),
                crate::presentation::companion_scene::input::CompanionPresentationOptions {
                    reduce_motion: self.reduce_motion,
                },
            )
            .map_err(|_| RetainedFailureCategory::LifetimeFramePreparation)?;
        Arc::make_mut(&mut snapshot).frame = projection.frame;
        let effects = self
            .activation
            .generations
            .reconcile_snapshot(snapshot, false)
            .map_err(|_| RetainedFailureCategory::LifetimeFramePreparation)?;
        self.host.metrics.record_semantic_reconcile();
        self.host.metrics.record_frame_reconcile();
        self.host
            .apply_scene_runtime_effects(self.activation, effects)?;
        Ok(true)
    }

    fn submit_active_frame(
        &mut self,
        semantic_reconciled: bool,
    ) -> std::result::Result<LifetimePresentationObservation, RetainedFailureCategory> {
        let version = self
            .activation
            .generations
            .active_version()
            .ok_or(RetainedFailureCategory::LifetimeFramePreparation)?;
        let geometry = self.host.scene_hud_geometry(
            version.generation.resources,
            self.hud_font_size,
            self.activation
                .generations
                .active_hud_depth_composition()
                .map_err(|_| RetainedFailureCategory::LifetimeFramePreparation)?,
        );
        let prepared_hud = self
            .activation
            .generations
            .prepare_active_hud(&self.hud, geometry)
            .map_err(|_| RetainedFailureCategory::LifetimeFramePreparation)?;
        let gpu = self
            .activation
            .gpu
            .as_mut()
            .ok_or(RetainedFailureCategory::LifetimeFramePreparation)?;
        let (_, dirty, draws, timings, submission) = self
            .activation
            .generations
            .submit_active_offscreen(
                &mut gpu.renderer,
                &self.host.device,
                &self.host.queue,
                &gpu.shared,
                [self.host.physical_width, self.host.physical_height],
                self.host.backing_scale,
                &prepared_hud,
            )
            .map_err(scene_render_failure)?;
        if let Some(dirty) = dirty {
            self.host.metrics.record_scene_dirty_upload(
                dirty.content_ranges,
                dirty.content_bytes,
                dirty.frame_ranges,
                dirty.frame_bytes,
            );
            self.host
                .metrics
                .record_delta_write_us(duration_us(timings.delta_write));
        }
        self.host
            .metrics
            .record_encode_us(duration_us(timings.encode));
        self.host
            .metrics
            .record_submit_us(duration_us(timings.submit));
        self.host.metrics.record_submit();
        self.host.metrics.record_draws(draws);
        self.last_submission = Some(submission);
        Ok(LifetimePresentationObservation {
            semantic_reconciled,
            frame_projected: true,
            frame_reconciled: true,
            encoded: true,
            submitted: true,
            draw_calls: draws,
            gpu_bytes: self
                .host
                .metrics
                .gpu_accounting_snapshot()
                .current_bytes
                .total_bytes,
        })
    }
}

impl<Semantic> LifetimeAuditExecutor for DirectLifetimeAuditExecutor<'_, Semantic>
where
    Semantic: FnMut(
        LifetimeAuditPhase,
        u64,
        time::OffsetDateTime,
    ) -> std::result::Result<
        Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
        RetainedFailureCategory,
    >,
{
    fn semantic_sample(
        &mut self,
        phase: LifetimeAuditPhase,
        sample: u64,
        now: time::OffsetDateTime,
    ) -> std::result::Result<LifetimeSemanticObservation, RetainedFailureCategory> {
        let snapshot = (self.semantic)(phase, sample, now)?;
        if self.reveal_pending {
            if self.activation.generations.active_delta_pending() {
                return Err(RetainedFailureCategory::LifetimeFramePreparation);
            }
            let coalesced = self
                .activation
                .generations
                .coalesce_hidden_snapshot(snapshot)
                .map_err(|_| RetainedFailureCategory::LifetimeFramePreparation)?;
            self.host.metrics.record_semantic_reconcile();
            self.host
                .apply_scene_runtime_effects(self.activation, coalesced)?;
            let scale_changed = self
                .surface_change
                .is_some_and(|change| change.scale_changed);
            if let Some(change) = self.surface_change.take() {
                let effects = self.activation.generations.rebind_surface(change.epoch)?;
                self.host
                    .apply_scene_runtime_effects(self.activation, effects)?;
                self.host.clear_presented_scene_receipt();
            }
            let effects = self.activation.generations.reveal(scale_changed)?;
            self.host
                .apply_scene_runtime_effects(self.activation, effects)?;
            self.reveal_pending = false;
            self.reveal_submission_pending = true;
            self.host.metrics.record_snapshot_projection();
            return Ok(LifetimeSemanticObservation {
                snapshot_projected: true,
                semantic_reconciled: true,
                stale_mutations: 0,
                stale_rejections: 0,
                stale_regenerations: 0,
                gpu_bytes: self
                    .host
                    .metrics
                    .gpu_accounting_snapshot()
                    .current_bytes
                    .total_bytes,
            });
        }
        if self.pending_semantic.replace(snapshot).is_some() {
            return Err(RetainedFailureCategory::LifetimeFramePreparation);
        }
        self.host.metrics.record_snapshot_projection();
        Ok(LifetimeSemanticObservation {
            snapshot_projected: true,
            semantic_reconciled: false,
            stale_mutations: 0,
            stale_rejections: 0,
            stale_regenerations: 0,
            gpu_bytes: self
                .host
                .metrics
                .gpu_accounting_snapshot()
                .current_bytes
                .total_bytes,
        })
    }

    fn presentation_tick(
        &mut self,
        _phase: LifetimeAuditPhase,
        _tick: u64,
        now: time::OffsetDateTime,
    ) -> std::result::Result<LifetimePresentationObservation, RetainedFailureCategory> {
        if std::mem::take(&mut self.reveal_submission_pending) {
            return self.submit_active_frame(true);
        }
        if self.reconcile_pending_semantic(now)? {
            return self.submit_active_frame(true);
        }
        let projection = self
            .activation
            .generations
            .project_frame(
                crate::presentation::companion_scene::CompanionProjectionClock::new(
                    now,
                    lifetime_elapsed_ms(now),
                ),
                crate::presentation::companion_scene::input::CompanionPresentationOptions {
                    reduce_motion: self.reduce_motion,
                },
            )
            .map_err(|_| RetainedFailureCategory::LifetimeFramePreparation)?;
        let (effects, regenerated) = self
            .activation
            .generations
            .reconcile_frame_projection(projection, false)
            .map_err(|_| RetainedFailureCategory::LifetimeFramePreparation)?;
        if regenerated {
            return Err(RetainedFailureCategory::LifetimeFramePreparation);
        }
        self.host.metrics.record_frame_reconcile();
        self.host
            .apply_scene_runtime_effects(self.activation, effects)?;
        self.submit_active_frame(false)
    }

    fn poll(&mut self) -> std::result::Result<(), RetainedFailureCategory> {
        let Some(submission) = self.last_submission.take() else {
            return Ok(());
        };
        let poll_result = self
            .host
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(5)),
            })
            .map_err(|_| RetainedFailureCategory::LifetimeGpuPoll);
        let mailbox_result = self.host.drain_current_gpu_error();
        poll_result.and(mailbox_result)
    }

    fn rss_bytes(&mut self) -> std::result::Result<u64, RetainedFailureCategory> {
        current_process_rss_bytes()
    }

    fn work_counters(&self) -> RuntimeWorkCounters {
        self.host.metrics.work_counters()
    }

    fn persistent_resource_creations(&self) -> u64 {
        self.host.metrics.persistent_gpu_objects_created()
    }

    fn static_upload_bytes(&self) -> u64 {
        self.host.metrics.static_upload_bytes()
    }

    fn offscreen_cache_events(&self) -> (u64, u64) {
        self.activation
            .gpu
            .as_ref()
            .map(|gpu| gpu.renderer.offscreen_cache_events())
            .unwrap_or_default()
    }

    fn storage_capacity_signature(&self) -> u64 {
        self.activation
            .generations
            .active
            .as_ref()
            .map(|active| active.cpu.storage_capacity_signature())
            .unwrap_or_default()
    }
}

fn lifetime_elapsed_ms(now: time::OffsetDateTime) -> u64 {
    let base = time::macros::datetime!(2026-06-13 18:00 UTC);
    (now - base)
        .whole_milliseconds()
        .max(0)
        .min(i128::from(u64::MAX)) as u64
}

impl std::ops::Deref for ActiveRetainedHost {
    type Target = RetainedHost;

    fn deref(&self) -> &Self::Target {
        &self.host
    }
}

impl std::ops::DerefMut for ActiveRetainedHost {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.host
    }
}

impl RetainedHost {
    fn record_scene_presented(
        &mut self,
        version: crate::presentation::companion_scene::SceneVersion,
        hud_text: &crate::round::hud::CompanionHudText,
        hud_font_size: f64,
        privacy: crate::presentation::companion_scene::contract::PresentedCapturePrivacy,
        logical_state_alias:
            crate::presentation::companion_scene::contract::CompanionCaptureStateAlias,
    ) -> Result<(), RetainedFailureCategory> {
        let presented_at = Instant::now();
        let visible_no_present = self
            .visible_present_interval_anchor
            .map_or_else(std::time::Duration::default, |previous| {
                presented_at.saturating_duration_since(previous)
            });
        let logical_points = [
            (f64::from(self.physical_width) / self.backing_scale) as f32,
            (f64::from(self.physical_height) / self.backing_scale) as f32,
        ];
        let receipt =
            crate::presentation::companion_scene::contract::PresentedSceneVersion::try_new(
                version,
                logical_points,
                [self.physical_width, self.physical_height],
                self.backing_scale as f32,
                self.next_hud_revision,
                privacy,
            )
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        self.last_presented_scene = Some(receipt);
        self.presented_scene_count = self.presented_scene_count.saturating_add(1);
        self.last_scene_presented_at = Some(presented_at);
        self.visible_present_interval_anchor = Some(presented_at);
        self.metrics.record_present(visible_no_present);
        self.next_hud_revision = self.next_hud_revision.saturating_add(1);
        self.last_presented_hud_text = Some(hud_text.clone());
        self.last_presented_hud_font_size = Some(hud_font_size);
        self.last_presented_state_alias = Some(logical_state_alias);
        Ok(())
    }

    fn scene_hud_geometry(
        &self,
        resource_generation: crate::presentation::companion_scene::ResourceGeneration,
        hud_font_size: f64,
        depth_composition: crate::round::depth::CompanionDepthComposition,
    ) -> hud::HudPreparationGeometry {
        let view_width = f64::from(self.physical_width) / self.backing_scale;
        let view_height = f64::from(self.physical_height) / self.backing_scale;
        let aperture_radius = view_width.min(view_height) / 2.0;
        let center_x = view_width / 2.0;
        let center_y = view_height / 2.0;
        let gauge = crate::round::hud::perimeter_gauge_layout(
            center_x,
            center_y,
            aperture_radius,
            crate::round::hud::COMPANION_GAUGE_GAP_DEG,
        );
        let gap = crate::round::hud::stat_gap_box(
            center_x,
            center_y,
            gauge.pace.ring.radius - gauge.pace.stroke_width / 2.0,
            crate::round::hud::COMPANION_GAUGE_GAP_DEG,
        );
        hud::HudPreparationGeometry {
            gap,
            aperture_radius,
            view_width,
            view_height,
            hud_font_size,
            resource_generation,
            depth_composition,
        }
    }

    /// Presents the current active scene version. Compatible logical deltas are
    /// staged and committed by the renderer transaction; topology changes keep
    /// the prior active generation visible until candidate activation commits.
    fn present_active_scene(
        &mut self,
        _view: &NSView,
        generations: &mut RetainedSceneGenerationState,
        renderer: &mut render::SceneRenderer,
        shared: &render::SceneGpuShared,
        prepared_hud: &hud::SensitivePreparedHudFrame,
    ) -> Result<ScenePresentOutcome, RetainedFailureCategory> {
        let attempted_at = Instant::now();
        self.metrics.record_present_attempt();
        self.visible_present_interval_anchor
            .get_or_insert(attempted_at);
        let no_present_interval = attempted_at
            .saturating_duration_since(*self.visible_present_interval_anchor.as_ref().unwrap());
        self.metrics.observe_visible_no_present(no_present_interval);
        // The live coordinator performs resize/rebind before reconciliation. A
        // present may only consume that already-bound host contract.
        if let Some(category) = self.gpu_errors.drain() {
            return Err(category);
        }
        if !generations.active_surface_extent_matches(
            [self.physical_width, self.physical_height],
            self.backing_scale,
        ) {
            self.metrics.record_skip(SkipReason::Outdated);
            return Ok(ScenePresentOutcome::Skipped(SkipReason::Outdated));
        }
        let scene_config = self
            .scene_config
            .clone()
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        if self.configured_surface != ConfiguredSurface::Scene {
            self.surface.configure(&self.device, &scene_config);
            self.configured_surface = ConfiguredSurface::Scene;
        }
        let mut progress = FrameProgress::new(
            self.next_frame_id(),
            generations
                .active_version()
                .map(|version| version.generation.resources.0)
                .unwrap_or(0),
        );
        progress
            .mark(FrameMilestone::Prepared)
            .expect("an active scene opens the present ladder");
        self.metrics.record_surface_acquire();
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &scene_config);
                self.metrics.record_skip(SkipReason::Outdated);
                return Ok(ScenePresentOutcome::Skipped(SkipReason::Outdated));
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.metrics.record_skip(SkipReason::Timeout);
                return Ok(ScenePresentOutcome::Skipped(SkipReason::Timeout));
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                self.metrics.record_skip(SkipReason::Occluded);
                return Ok(ScenePresentOutcome::Skipped(SkipReason::Occluded));
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err(RetainedFailureCategory::SurfaceLost);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RetainedFailureCategory::SurfaceValidation);
            }
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let (version, dirty_metrics, draw_count, timings) = generations
            .submit_active_to_surface(
                renderer,
                &self.device,
                &self.queue,
                shared,
                [self.physical_width, self.physical_height],
                self.backing_scale,
                prepared_hud,
                &surface_view,
            )
            .map_err(scene_render_failure)?;
        self.metrics.record_encode_us(duration_us(timings.encode));
        if dirty_metrics.is_some() {
            self.metrics
                .record_delta_write_us(duration_us(timings.delta_write));
        }
        if let Some(dirty) = dirty_metrics {
            self.metrics.record_scene_dirty_upload(
                dirty.content_ranges,
                dirty.content_bytes,
                dirty.frame_ranges,
                dirty.frame_bytes,
            );
        }
        progress
            .mark(FrameMilestone::Encoded)
            .expect("active scene encode follows preparation");
        progress
            .mark(FrameMilestone::Submitted)
            .expect("active scene submission follows encode");
        self.metrics.record_submit();
        self.metrics.record_submit_us(duration_us(timings.submit));
        self.metrics.record_draws(draw_count);
        self.queue.present(surface_texture);
        progress
            .finish(FrameDisposition::SurfacePresentCalled)
            .expect("an active scene submission presents exactly once");
        if let Some(category) = self.gpu_errors.drain() {
            return Err(category);
        }
        Ok(ScenePresentOutcome::Presented(version))
    }

    #[allow(dead_code)] // Reached through the dormant Task 12 entrypoint above.
    fn advance_scene_generation(
        &mut self,
    ) -> Result<SceneGenerationServiceTick, RetainedFailureCategory> {
        let reply = self
            .scene_build_worker
            .try_recv_build()
            .map_err(|_| RetainedFailureCategory::RasterWorkerUnavailable)?;
        let Some(reply) = reply else {
            return Ok(if self.scene_activation.is_some() {
                SceneGenerationServiceTick::Preparing
            } else {
                SceneGenerationServiceTick::Idle
            });
        };
        let mut activation = self
            .scene_activation
            .take()
            .ok_or(RetainedFailureCategory::RasterWorkerUnavailable)?;
        macro_rules! restore_on_error {
            ($result:expr) => {
                match $result {
                    Ok(value) => value,
                    Err(error) => {
                        self.scene_activation = Some(activation);
                        return Err(error);
                    }
                }
            };
        }
        let tick = match reply {
            worker::SceneBuildReply::SceneCompleted(candidate) => {
                let timing = candidate.timing;
                self.metrics.record_worker_terminal(
                    timing.active_compile,
                    timing.raster_calls,
                    timing.main_thread_raster_calls,
                );
                self.metrics.record_worker_completion();
                if timing.main_thread_raster_calls != 0 {
                    self.metrics.record_worker_failure();
                    let effects = activation
                        .generations
                        .runtime
                        .reject_worker_candidate(candidate.identity);
                    restore_on_error!(self.apply_scene_runtime_effects(&mut activation, effects));
                    SceneGenerationServiceTick::Failed
                } else {
                    let effects = activation.generations.accept_worker_candidate(candidate);
                    restore_on_error!(self.apply_scene_runtime_effects(&mut activation, effects));
                    if activation.generations.has_cpu_candidate() {
                        SceneGenerationServiceTick::CandidateReady
                    } else {
                        SceneGenerationServiceTick::Preparing
                    }
                }
            }
            worker::SceneBuildReply::SceneCancelled { identity, timing } => {
                self.metrics.record_worker_terminal(
                    timing.active_compile,
                    timing.raster_calls,
                    timing.main_thread_raster_calls,
                );
                let effects = activation
                    .generations
                    .runtime
                    .acknowledge_worker_cancelled(identity.request_id());
                restore_on_error!(self.apply_scene_runtime_effects(&mut activation, effects));
                SceneGenerationServiceTick::Preparing
            }
            worker::SceneBuildReply::SceneFailed { identity, failure: _, timing } => {
                self.metrics.record_worker_terminal(
                    timing.active_compile,
                    timing.raster_calls,
                    timing.main_thread_raster_calls,
                );
                self.metrics.record_worker_failure();
                let effects = activation
                    .generations
                    .runtime
                    .reject_worker_candidate(identity);
                restore_on_error!(self.apply_scene_runtime_effects(&mut activation, effects));
                SceneGenerationServiceTick::Failed
            }
            worker::SceneBuildReply::Raster(_) => {
                self.scene_activation = Some(activation);
                return Err(RetainedFailureCategory::RasterWorkerUnavailable);
            }
        };
        self.scene_activation = Some(activation);
        Ok(tick)
    }

    #[allow(dead_code)] // Reached through the dormant Task 12 entrypoint above.
    fn materialize_scene_candidate(&mut self) -> Result<(), RetainedFailureCategory> {
        let mut activation = self
            .scene_activation
            .take()
            .ok_or(RetainedFailureCategory::RasterWorkerUnavailable)?;
        let gpu_result = self.ensure_scene_gpu_state(&mut activation);
        if let Err(error) = gpu_result {
            self.scene_activation = Some(activation);
            return Err(error);
        }
        let started_at = Instant::now();
        let gpu = activation
            .gpu
            .as_ref()
            .expect("successful scene GPU initialization installs one GPU state");
        let materialized = activation.generations.materialize_ready_candidate(
            &self.device,
            &self.queue,
            &gpu.shared,
        );
        let result = match materialized {
            Ok(()) => {
                self.metrics
                    .record_gpu_materialize_publish_us(duration_us(started_at.elapsed()));
                self.metrics.record_generation_accepted();
                Ok(())
            }
            Err(error) => {
                let category = scene_candidate_preparation_failure(&error);
                let effects = activation
                    .generations
                    .reject_materialization_failure(&error);
                match self.apply_scene_runtime_effects(&mut activation, effects) {
                    Ok(()) => Err(category),
                    Err(effect_error) => Err(effect_error),
                }
            }
        };
        self.scene_activation = Some(activation);
        result
    }

    fn ensure_scene_gpu_state(
        &self,
        activation: &mut RetainedSceneActivation,
    ) -> Result<(), RetainedFailureCategory> {
        if activation.gpu.is_none() {
            let shared = render::SceneGpuShared::create(&self.device, self.device_epoch)
                .map_err(scene_gpu_failure)?;
            let renderer = render::SceneRenderer::new(&self.device, &self.queue, &shared);
            activation.gpu = Some(super::RetainedSceneGpuState { shared, renderer });
        }
        Ok(())
    }

    #[allow(dead_code)] // Reached through the dormant Task 12 entrypoint above.
    fn apply_scene_runtime_effects(
        &mut self,
        activation: &mut RetainedSceneActivation,
        mut effects: crate::presentation::companion_scene::runtime::RuntimeEffects,
    ) -> Result<(), RetainedFailureCategory> {
        if let Some(cancel) = effects.take_cancel_worker() {
            let epoch = cancel
                .request_id()
                .0
                .checked_add(1)
                .ok_or(RetainedFailureCategory::RasterWorkerUnavailable)?;
            self.scene_build_worker
                .cancel_with_epoch(epoch)
                .map_err(|_| RetainedFailureCategory::RasterWorkerUnavailable)?;
            self.metrics.record_worker_cancellation();
        }
        if let Some(request) = effects.take_start_worker() {
            let identity = request.identity();
            if let Err(job) = self
                .scene_build_worker
                .try_submit_scene(request, self.backing_scale)
            {
                let mut rejected = activation
                    .generations
                    .runtime
                    .reject_worker_candidate(job.identity());
                debug_assert_eq!(job.identity(), identity);
                debug_assert!(rejected.take_start_worker().is_none());
                self.metrics.record_worker_failure();
                return Err(RetainedFailureCategory::RasterWorkerUnavailable);
            }
            self.metrics.record_worker_submission();
        }
        Ok(())
    }

    /// Performs one complete scene-generation activation transaction on the
    /// render owner. Acquisition is nonblocking and attempted once; only a
    /// clean post-present mailbox permits the coordinator to publish the GPU
    /// candidate into its active slot.
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)] // Reached through the dormant Task 12 entrypoint above.
    pub(super) fn activate_candidate(
        &mut self,
        generations: &mut RetainedSceneGenerationState,
        renderer: &mut render::SceneRenderer,
        shared: &render::SceneGpuShared,
        prepared_hud: &hud::SensitivePreparedHudFrame,
    ) -> Result<crate::presentation::companion_scene::runtime::RuntimeEffects, SceneActivationError>
    {
        let attempted_at = Instant::now();
        self.metrics.record_present_attempt();
        self.visible_present_interval_anchor
            .get_or_insert(attempted_at);
        let no_present_interval = attempted_at
            .saturating_duration_since(*self.visible_present_interval_anchor.as_ref().unwrap());
        self.metrics.observe_visible_no_present(no_present_interval);
        self.last_scene_activation_skip = None;
        if let Some(delayed) = generations.observe_delayed_gpu_error(&self.gpu_errors) {
            self.ensure_legacy_surface();
            return Ok(delayed);
        }
        let attempt = match generations.begin_activation() {
            Ok(attempt) => attempt,
            Err(
                crate::presentation::companion_scene::runtime::ActivationStartError::CandidateNeedsRebase,
            ) => {
                if let Err(error) = generations.rebase_materialized_candidate(
                    &self.device,
                    &self.queue,
                    shared,
                ) {
                    let effects = generations.reject_materialization_failure(&error);
                    if effects.disposition()
                        == crate::presentation::companion_scene::runtime::RuntimeDisposition::Activation(
                            crate::presentation::companion_scene::runtime::ActivationTransition::HostFallbackPending,
                        )
                    {
                        self.ensure_legacy_surface();
                    }
                    return Ok(effects);
                }
                generations
                    .begin_activation()
                    .map_err(SceneActivationError::Start)?
            }
            Err(error) => return Err(SceneActivationError::Start(error)),
        };
        let scene_config = self
            .scene_config
            .clone()
            .ok_or(SceneActivationError::UnsupportedSurfaceContract)?;
        if self.configured_surface != ConfiguredSurface::Scene {
            self.surface.configure(&self.device, &scene_config);
            self.configured_surface = ConfiguredSurface::Scene;
        }
        let mut progress = FrameProgress::new(self.next_frame_id(), attempt.key().resources.0);
        progress
            .mark(FrameMilestone::Prepared)
            .expect("a materialized scene candidate opens the activation ladder");
        self.metrics.record_surface_acquire();
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &scene_config);
                self.last_scene_activation_skip = Some(SkipReason::Outdated);
                self.metrics.record_skip(SkipReason::Outdated);
                skip(&mut progress, SkipReason::Outdated);
                return Ok(self.finish_scene_activation(generations, attempt, progress));
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.last_scene_activation_skip = Some(SkipReason::Timeout);
                self.metrics.record_skip(SkipReason::Timeout);
                skip(&mut progress, SkipReason::Timeout);
                return Ok(self.finish_scene_activation(generations, attempt, progress));
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                self.last_scene_activation_skip = Some(SkipReason::Occluded);
                self.metrics.record_skip(SkipReason::Occluded);
                skip(&mut progress, SkipReason::Occluded);
                return Ok(self.finish_scene_activation(generations, attempt, progress));
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                fail(&mut progress, RetainedFailureCategory::SurfaceLost);
                return Ok(self.finish_scene_activation(generations, attempt, progress));
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                fail(&mut progress, RetainedFailureCategory::SurfaceValidation);
                return Ok(self.finish_scene_activation(generations, attempt, progress));
            }
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let draw_count = generations.ready_candidate_draw_count().unwrap_or(0);
        let encode_started_at = Instant::now();
        let encoded = {
            let candidate = generations
                .ready_candidate
                .as_mut()
                .expect("begin_activation requires one external GPU-ready candidate");
            let request = render::SceneRenderRequest::new(
                candidate.version,
                [self.physical_width, self.physical_height],
                self.backing_scale,
            );
            renderer.encode_candidate_to_surface(
                &self.device,
                &self.queue,
                shared,
                &mut candidate.gpu,
                request,
                prepared_hud,
                &surface_view,
            )
        };
        let command = match encoded {
            Ok(command) => command,
            Err(error) => {
                fail(&mut progress, scene_render_failure(error));
                return Ok(self.finish_scene_activation(generations, attempt, progress));
            }
        };
        self.metrics
            .record_encode_us(duration_us(encode_started_at.elapsed()));
        progress
            .mark(FrameMilestone::Encoded)
            .expect("scene encode follows prepared candidate");
        let submit_started_at = Instant::now();
        self.queue.submit([command]);
        renderer.recall_uploads();
        self.metrics
            .record_submit_us(duration_us(submit_started_at.elapsed()));
        self.metrics.record_submit();
        self.metrics.record_draws(draw_count);
        progress
            .mark(FrameMilestone::Submitted)
            .expect("scene submission follows encode");
        self.queue.present(surface_texture);
        progress
            .finish(FrameDisposition::SurfacePresentCalled)
            .expect("a submitted candidate presents exactly once");
        let effects = self.finish_scene_activation(generations, attempt, progress);
        Ok(effects)
    }

    #[allow(dead_code)] // Reached through the dormant Task 12 entrypoint above.
    fn finish_scene_activation(
        &mut self,
        generations: &mut RetainedSceneGenerationState,
        attempt: crate::presentation::companion_scene::runtime::ActivationAttempt,
        progress: FrameProgress,
    ) -> crate::presentation::companion_scene::runtime::RuntimeEffects {
        let effects = generations.finish_candidate_activation(attempt, progress, &self.gpu_errors);
        if effects.disposition()
            == crate::presentation::companion_scene::runtime::RuntimeDisposition::Activation(
                crate::presentation::companion_scene::runtime::ActivationTransition::Committed,
            )
        {
            if let Some(backing_scale) = generations.active_backing_scale() {
                self.metrics.record_generation_activation(backing_scale);
            }
        }
        if effects.disposition()
            == crate::presentation::companion_scene::runtime::RuntimeDisposition::Activation(
                crate::presentation::companion_scene::runtime::ActivationTransition::HostFallbackPending,
            )
        {
            self.ensure_legacy_surface();
        }
        effects
    }

    fn record_metrics(&mut self, record: impl FnOnce(&mut CompanionRuntimeMetrics)) {
        let started_at = Instant::now();
        record(&mut self.metrics);
        self.metrics.record_metrics_overhead(started_at.elapsed());
    }

    /// Advances the monotonic frame counter, returning the id for the frame
    /// about to be attempted.
    fn next_frame_id(&mut self) -> u64 {
        let id = self.frame_counter;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        id
    }

    /// Drains any GPU device fault reported asynchronously by the wgpu error
    /// callback. The main thread checks this before treating a present as good.
    pub(in crate::companion) fn drain_gpu_error(&self) -> Option<RetainedFailureCategory> {
        self.gpu_errors.drain_for(self.device_epoch)
    }

    pub(super) fn drain_current_gpu_error(
        &self,
    ) -> std::result::Result<(), RetainedFailureCategory> {
        match self.gpu_errors.drain_for(self.device_epoch) {
            Some(category) => Err(category),
            None => Ok(()),
        }
    }

    /// Dev/test-only: posts a static category to this host's own error mailbox so
    /// the next main-thread [`drain_gpu_error`](Self::drain_gpu_error) observes it
    /// exactly as it would a real asynchronous device fault. Compiled only with
    /// dev-preview so a release build cannot inject faults.
    #[cfg(feature = "dev-preview")]
    pub(in crate::companion) fn inject_gpu_fault(&self, category: RetainedFailureCategory) {
        let _ = self.gpu_errors.sender_for(self.device_epoch).send(category);
    }

    #[allow(clippy::too_many_arguments)] // Explicit prepared-frame inputs keep retained independent of AppState.
    pub(in crate::companion) fn render(
        &mut self,
        view: &NSView,
        plan: &SmoothCompanionScenePlan,
        draw_order: &[usize],
        metrics: CompanionGridMetrics,
        aperture: RoundAperture,
        background: [f32; 4],
        chrome: RetainedChrome<'_>,
        identity: &CompanionContentIdentity,
        refresh_surface: bool,
    ) -> FrameProgress {
        let activation_attempt_started = (!self.activation_recorded).then(Instant::now);
        let progress = (|| {
            let frame_id = self.next_frame_id();
            if refresh_surface {
                if let Err(category) = self.resize_surface_if_needed(view) {
                    let mut progress = FrameProgress::new(frame_id, 0);
                    fail(&mut progress, category);
                    return progress;
                }
            }
            self.ensure_legacy_surface();
            let Some(generation) = self
                .glyph_resources
                .as_ref()
                .filter(|active| {
                    active.identity == *identity
                        && active.backing_scale.to_bits() == self.backing_scale.to_bits()
                })
                .map(|active| active.resources.generation().value())
            else {
                let mut progress = FrameProgress::new(frame_id, 0);
                self.record_metrics(|metrics| metrics.record_skip(SkipReason::ResourcePreparation));
                skip(&mut progress, SkipReason::ResourcePreparation);
                return progress;
            };
            let mut progress = FrameProgress::new(frame_id, generation);
            let frame = {
                let active = self
                    .glyph_resources
                    .as_ref()
                    .expect("an established generation implies active glyph resources");
                let started_at = Instant::now();
                let result = prepare_gpu_frame(
                    plan,
                    draw_order,
                    metrics,
                    aperture,
                    background,
                    &chrome,
                    active.resources.atlas(),
                );
                let elapsed = duration_us(started_at.elapsed());
                self.record_metrics(|metrics| metrics.record_gpu_translate_us(elapsed));
                match result {
                    Ok(frame) => frame,
                    Err(category) => {
                        fail(&mut progress, category);
                        return progress;
                    }
                }
            };
            progress
                .mark(FrameMilestone::Prepared)
                .expect("prepared opens the frame ladder");
            self.record_metrics(CompanionRuntimeMetrics::record_surface_acquire);
            let surface_texture = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture)
                | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
                wgpu::CurrentSurfaceTexture::Outdated => {
                    self.surface.configure(&self.device, &self.config);
                    self.record_metrics(|metrics| metrics.record_skip(SkipReason::Outdated));
                    skip(&mut progress, SkipReason::Outdated);
                    return progress;
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    self.record_metrics(|metrics| metrics.record_skip(SkipReason::Timeout));
                    skip(&mut progress, SkipReason::Timeout);
                    return progress;
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    self.record_metrics(|metrics| metrics.record_skip(SkipReason::Occluded));
                    skip(&mut progress, SkipReason::Occluded);
                    return progress;
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    fail(&mut progress, RetainedFailureCategory::SurfaceLost);
                    return progress;
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    fail(&mut progress, RetainedFailureCategory::SurfaceValidation);
                    return progress;
                }
            };
            let target = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let encode_started_at = Instant::now();
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("glorp-retained-frame"),
                });
            self.prepare_frame(&mut encoder, &frame);
            {
                let active = self
                    .glyph_resources
                    .as_ref()
                    .expect("an established generation implies active glyph resources");
                self.encode_scene(
                    &mut encoder,
                    &target,
                    &active.bind_group,
                    self.frame_buffers.current_buffer(),
                    &frame.blends,
                    background,
                );
            }
            progress
                .mark(FrameMilestone::Encoded)
                .expect("encoded follows prepared");
            let encode_us = duration_us(encode_started_at.elapsed());
            self.record_metrics(|metrics| metrics.record_encode_us(encode_us));
            let submit_started_at = Instant::now();
            self.frame_buffers.finish_uploads();
            self.queue.submit([encoder.finish()]);
            self.frame_buffers.recall_uploads();
            let queue_wait_us = duration_us(submit_started_at.elapsed());
            let draws = frame.blends.len() as u64;
            self.record_metrics(|metrics| {
                metrics.record_queue_wait_us(queue_wait_us);
                metrics.record_submit();
                metrics.record_draws(draws);
            });
            progress
                .mark(FrameMilestone::Submitted)
                .expect("submitted follows encoded");
            self.queue.present(surface_texture);
            progress
                .finish(FrameDisposition::SurfacePresentCalled)
                .expect("a submitted frame presents exactly once");
            progress
        })();
        if let Some(started_at) = activation_attempt_started {
            self.activation_render_owner_us = self
                .activation_render_owner_us
                .saturating_add(u64::from(duration_us(started_at.elapsed())));
            if progress.disposition() == Some(FrameDisposition::SurfacePresentCalled) {
                let activation_us = self.activation_render_owner_us.min(u64::from(u32::MAX)) as u32;
                self.record_metrics(|metrics| {
                    metrics.record_activation_render_owner_us(activation_us)
                });
                self.activation_recorded = true;
            }
        }
        progress
    }

    /// Stages a prepared frame's instances into the persistent instance ring:
    /// grows the ring only if the instance count exceeds the current capacity,
    /// then writes the used prefix into the next slot. Ordinary motion holds the
    /// count steady, so this only stages a copy and never grows persistent resources.
    pub(super) fn prepare_frame(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &PreparedGpuFrame,
    ) {
        let before = self.counters;
        self.frame_buffers.ensure_instance_capacity(
            frame.primitives.len(),
            &self.device,
            &mut self.counters,
        );
        self.frame_buffers
            .write_frame_instances(encoder, &frame.primitives, &mut self.counters);
        let delta = self.counters - before;
        let primitives = frame.primitives.len() as u32;
        let blended_draws = frame.blends.len() as u32;
        let cpu_bytes = (frame.primitives.capacity() * std::mem::size_of::<GpuPrimitive>()) as u64;
        let gpu_bytes =
            (self.frame_buffers.capacity_instances * INSTANCE_RING_LEN * INSTANCE_STRIDE) as u64;
        self.record_metrics(|metrics| {
            if delta.buffer_creations > 0 {
                metrics.replace_gpu_allocation(
                    GpuAllocationKind::InstanceRing,
                    gpu_bytes,
                    INSTANCE_RING_LEN as u64,
                );
            }
            metrics.record_queue_write(delta.instance_write_bytes);
            metrics.observe_primitives(primitives);
            metrics.observe_blended_draws(blended_draws);
            metrics.observe_cpu_bytes(cpu_bytes);
        });
    }

    /// Encodes the prepared companion scene into `target_view` on `encoder`: one
    /// clear-loaded render pass that draws every primitive with its blend
    /// pipeline. Shared verbatim by the live surface [`Self::render`] path and
    /// the capture intermediate path so both rasterize identical geometry.
    pub(super) fn encode_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        atlas_bind_group: &wgpu::BindGroup,
        primitive_buffer: &wgpu::Buffer,
        blends: &[SmoothBlendMode],
        background: [f32; 4],
    ) {
        let attachment = Some(wgpu::RenderPassColorAttachment {
            view: target_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear({
                    // The premultiplied-gamma convention: the linear-format target
                    // holds sRGB-space values, so the clear is the straight-sRGB
                    // background premultiplied by its alpha (no sRGB→linear step).
                    let clear = parity::premultiply_gamma_srgb(background);
                    wgpu::Color {
                        r: f64::from(clear[0]),
                        g: f64::from(clear[1]),
                        b: f64::from(clear[2]),
                        a: f64::from(clear[3]),
                    }
                }),
                store: wgpu::StoreOp::Store,
            },
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("glorp-retained-pass"),
            color_attachments: &[attachment],
            ..Default::default()
        });
        pass.set_bind_group(0, atlas_bind_group, &[]);
        pass.set_vertex_buffer(0, primitive_buffer.slice(..));
        for (index, blend) in blends.iter().copied().enumerate() {
            pass.set_pipeline(self.pipelines.get(blend));
            pass.draw(0..6, index as u32..index as u32 + 1);
        }
    }

    fn advance_resource_preparation(
        &mut self,
        identity: &CompanionContentIdentity,
        desired_backing_scale: f64,
    ) -> ResourcePreparationTick {
        let service_started_at = Instant::now();
        let desired_key = ResourcePreparationKey::new(identity.clone(), desired_backing_scale);
        if self.resource_preparation.worker_unavailable {
            let active_matches = self.glyph_resources.as_ref().is_some_and(|active| {
                ResourcePreparationKey::new(active.identity.clone(), active.backing_scale)
                    == desired_key
            });
            let tick = terminal_worker_decision(active_matches, self.glyph_resources.is_some());
            return self.finish_generation_service(service_started_at, tick);
        }
        if self.resource_preparation.coalesces(&desired_key) {
            self.metrics.record_worker_coalesce();
        }
        let previous_id = self
            .resource_preparation
            .desired
            .as_ref()
            .map(|request| request.id);
        if let Some(epoch) = self
            .resource_preparation
            .set_visible_desired(desired_key.clone())
        {
            self.failed_glyph_preparation = None;
            if self.scene_build_worker.cancel_with_epoch(epoch).is_err() {
                let tick = self.worker_unavailable();
                return self.finish_generation_service(service_started_at, tick);
            }
            self.metrics.record_worker_cancellation();
        }
        if previous_id
            != self
                .resource_preparation
                .desired
                .as_ref()
                .map(|request| request.id)
        {
            self.failed_glyph_preparation = None;
        }
        let current = self
            .resource_preparation
            .desired
            .as_ref()
            .expect("visible desired request exists")
            .clone();

        let mut completed = None;
        match self.scene_build_worker.try_recv() {
            Ok(Some(reply)) => {
                let (reply_id, timing) = match &reply {
                    RasterReply::Completed { job_id, timing, .. }
                    | RasterReply::Cancelled { job_id, timing }
                    | RasterReply::Failed { job_id, timing, .. }
                    | RasterReply::WorkerPanicked { job_id, timing } => (*job_id, *timing),
                };
                self.metrics.record_worker_terminal(
                    timing.active_compile,
                    timing.raster_calls,
                    timing.main_thread_raster_calls,
                );
                let running = self.resource_preparation.finish_running(reply_id);
                let Some(running_request) = running.as_ref() else {
                    let tick = self.worker_unavailable();
                    return self.finish_generation_service(service_started_at, tick);
                };
                self.metrics.record_raster_request_wall_us(duration_us(
                    Instant::now().saturating_duration_since(running_request.enqueued_at),
                ));
                match reply {
                    RasterReply::Completed { resources, .. } => {
                        self.metrics.record_worker_completion();
                        if let Some(request) = running.filter(|request| {
                            request == &current
                                && self
                                    .resource_preparation
                                    .accepts_completed(request, &desired_key)
                        }) {
                            completed = Some((request, resources));
                        } else {
                            self.metrics.record_worker_stale_rejection();
                        }
                    }
                    RasterReply::Failed { category, .. } => {
                        self.metrics.record_worker_failure();
                        if let Some(request) = running.filter(|request| request == &current) {
                            self.failed_glyph_preparation = Some(FailedGlyphPreparation {
                                id: request.id,
                                key: request.key,
                                category,
                            });
                        } else {
                            self.metrics.record_worker_stale_rejection();
                        }
                    }
                    RasterReply::WorkerPanicked { .. } => {
                        let tick = self.worker_unavailable();
                        return self.finish_generation_service(service_started_at, tick);
                    }
                    RasterReply::Cancelled { .. } => {}
                }
            }
            Ok(None) => {}
            Err(_) => {
                let tick = self.worker_unavailable();
                return self.finish_generation_service(service_started_at, tick);
            }
        }

        if let Some((request, resources)) = completed {
            let still_current = self.resource_preparation.visible
                && self.resource_preparation.running.is_none()
                && self.resource_preparation.desired.as_ref() == Some(&request)
                && request.key
                    == ResourcePreparationKey::new(identity.clone(), desired_backing_scale);
            if still_current {
                self.resource_preparation.latest_pending = None;
                self.failed_glyph_preparation = None;
                self.metrics
                    .record_generation_service_ui_us(duration_us(service_started_at.elapsed()));
                let materialize_started_at = Instant::now();
                let tick = self.publish_prepared_resources(request, resources);
                self.metrics.record_gpu_materialize_publish_us(duration_us(
                    materialize_started_at.elapsed(),
                ));
                self.metrics.record_generation_accepted();
                return tick;
            }
            self.metrics.record_worker_stale_rejection();
        }

        let active_ready = self.glyph_resources.as_ref().is_some_and(|active| {
            ResourcePreparationKey::new(active.identity.clone(), active.backing_scale)
                == desired_key
        });
        if active_ready {
            self.resource_preparation.latest_pending = None;
            self.failed_glyph_preparation = None;
            return self
                .finish_generation_service(service_started_at, ResourcePreparationTick::Ready);
        }
        if let Some(category) =
            cached_current_failure(self.failed_glyph_preparation.as_ref(), &current)
        {
            let tick = self.failure_tick(category);
            return self.finish_generation_service(service_started_at, tick);
        }
        if let Some(request) = self.resource_preparation.take_pending_if_idle() {
            let manifest = GlyphRepertoireManifest::for_active_pet(
                request.key.identity.clone(),
                f64::from_bits(request.key.backing_scale_bits),
            );
            match self
                .scene_build_worker
                .try_submit(RasterJob::new(request.id, manifest))
            {
                Ok(()) => {
                    self.resource_preparation.mark_submitted(request);
                    self.metrics.record_worker_submission();
                }
                Err(
                    RasterSubmitError::Busy(_)
                    | RasterSubmitError::Stale(_)
                    | RasterSubmitError::Disconnected(_),
                ) => {
                    let tick = self.worker_unavailable();
                    return self.finish_generation_service(service_started_at, tick);
                }
            }
        }
        let tick = self.yield_tick();
        self.finish_generation_service(service_started_at, tick)
    }

    fn suspend_resource_preparation(
        &mut self,
        identity: &CompanionContentIdentity,
        desired_backing_scale: f64,
    ) {
        let key = ResourcePreparationKey::new(identity.clone(), desired_backing_scale);
        if let Some(epoch) = self.resource_preparation.suspend(key) {
            if self.scene_build_worker.cancel_with_epoch(epoch).is_err() {
                self.mark_worker_unavailable();
            } else {
                self.metrics.record_worker_cancellation();
            }
        }
    }

    fn finish_generation_service(
        &mut self,
        started_at: Instant,
        tick: ResourcePreparationTick,
    ) -> ResourcePreparationTick {
        self.metrics
            .record_generation_service_ui_us(duration_us(started_at.elapsed()));
        tick
    }

    fn publish_prepared_resources(
        &mut self,
        request: ResourcePreparationRequest,
        resources: CompiledRetainedResources,
    ) -> ResourcePreparationTick {
        let rebuilding = self.glyph_resources.is_some();
        let static_bytes = resources.atlas().rgba.len() as u64;
        let (texture, bind_group) = upload_glyph_atlas(
            &self.device,
            &self.queue,
            &self.atlas_layout,
            resources.atlas(),
            &mut self.counters,
        );
        self.metrics.record_static_upload(static_bytes);
        self.metrics
            .replace_gpu_allocation(GpuAllocationKind::Atlas, static_bytes, 2);
        if rebuilding {
            self.counters.atlas_builds_after_activation += 1;
            self.counters.atlas_uploads_after_activation += 1;
        }
        let published_backing_scale = f64::from_bits(request.key.backing_scale_bits);
        self.glyph_resources = Some(ActiveGlyphResources {
            identity: request.key.identity,
            backing_scale: published_backing_scale,
            resources,
            _texture: texture,
            bind_group,
        });
        self.backing_scale = published_backing_scale;
        ResourcePreparationTick::Ready
    }

    fn yield_tick(&self) -> ResourcePreparationTick {
        if self.glyph_resources.is_some() {
            ResourcePreparationTick::YieldedRetainingActive
        } else {
            ResourcePreparationTick::YieldedWithoutActive
        }
    }

    fn failure_tick(&self, category: RetainedFailureCategory) -> ResourcePreparationTick {
        resource_failure_tick(self.glyph_resources.is_some(), category)
    }

    fn worker_unavailable(&mut self) -> ResourcePreparationTick {
        let category = RetainedFailureCategory::RasterWorkerUnavailable;
        self.mark_worker_unavailable();
        if let Some(current) = self.resource_preparation.desired.clone() {
            self.failed_glyph_preparation = Some(FailedGlyphPreparation {
                id: current.id,
                key: current.key,
                category,
            });
        }
        self.failure_tick(category)
    }

    fn mark_worker_unavailable(&mut self) {
        if !self.resource_preparation.worker_unavailable {
            self.metrics.record_worker_failure();
            self.resource_preparation.worker_unavailable = true;
        }
    }

    fn record_resource_preparation_skip(&mut self) -> FrameProgress {
        let mut progress = FrameProgress::new(self.next_frame_id(), 0);
        self.record_metrics(|metrics| metrics.record_skip(SkipReason::ResourcePreparation));
        skip(&mut progress, SkipReason::ResourcePreparation);
        progress
    }

    /// Ensures the persistent capture intermediate and staging buffer match the
    /// current physical size and surface format, replacing them once on a change.
    /// Ordinary same-size captures reuse them.
    pub(super) fn ensure_capture_resources(&mut self, width: u32, height: u32) {
        let format = self.config.format;
        let fits = self.capture_resources.as_ref().is_some_and(|resources| {
            resources.width == width && resources.height == height && resources.format == format
        });
        if fits {
            return;
        }
        let intermediate = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glorp-retained-capture-intermediate"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.counters.texture_creations += 1;
        let intermediate_view = intermediate.create_view(&wgpu::TextureViewDescriptor::default());
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glorp-retained-capture-staging"),
            size: capture::staging_buffer_size(width, height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.counters.buffer_creations += 1;
        let capture_bytes = u64::from(width)
            .saturating_mul(u64::from(height))
            .saturating_mul(4)
            .saturating_add(capture::staging_buffer_size(width, height));
        self.metrics
            .replace_gpu_allocation(GpuAllocationKind::Capture, capture_bytes, 2);
        self.capture_resources = Some(PersistentCaptureResources {
            width,
            height,
            format,
            intermediate,
            intermediate_view,
            staging,
        });
    }

    /// The current GPU resource-lifecycle counters. A caller snapshots this,
    /// drives frames, and subtracts to prove the steady state created nothing.
    #[allow(dead_code)] // Production resource-counter accessor; the counters are the contract surface.
    fn counters(&self) -> RetainedResourceCounters {
        self.counters
    }

    fn resize_surface_if_needed(
        &mut self,
        view: &NSView,
    ) -> std::result::Result<Option<SurfaceContractChange>, RetainedFailureCategory> {
        let window = view
            .window()
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        let scale = window.backingScaleFactor();
        let bounds = view.bounds();
        let width = physical_dimension(bounds.size.width, scale);
        let height = physical_dimension(bounds.size.height, scale);
        if width == self.physical_width
            && height == self.physical_height
            && (scale - self.backing_scale).abs() < f64::EPSILON
        {
            return Ok(None);
        }
        let scale_changed = (scale - self.backing_scale).abs() >= f64::EPSILON;
        self.metrics.record_resize_invalidation();
        if scale_changed {
            self.metrics.record_scale_invalidation();
        }
        let next_surface_epoch = self
            .surface_epoch
            .0
            .checked_add(1)
            .map(crate::presentation::companion_scene::SurfaceEpoch)
            .ok_or(RetainedFailureCategory::SceneCandidateEncode)?;
        self.physical_width = width;
        self.physical_height = height;
        self.backing_scale = scale;
        self.config.width = width;
        self.config.height = height;
        if let Some(scene_config) = &mut self.scene_config {
            scene_config.width = width;
            scene_config.height = height;
        }
        unsafe {
            self.layer
                .setDrawableSize(NSSize::new(f64::from(width), f64::from(height)))
        };
        self.surface.configure(&self.device, &self.config);
        self.configured_surface = ConfiguredSurface::Legacy;
        self.surface_epoch = next_surface_epoch;
        Ok(Some(SurfaceContractChange {
            epoch: self.surface_epoch,
            scale_changed,
        }))
    }

    fn clear_presented_scene_receipt(&mut self) {
        self.last_presented_scene = None;
        self.last_presented_hud_text = None;
        self.last_presented_hud_font_size = None;
        self.last_presented_state_alias = None;
    }

    fn ensure_legacy_surface(&mut self) {
        if self.configured_surface != ConfiguredSurface::Legacy {
            self.surface.configure(&self.device, &self.config);
            self.configured_surface = ConfiguredSurface::Legacy;
        }
    }
}

#[allow(dead_code)] // Maps errors for the dormant Task 12 host activation entrypoint.
fn scene_render_failure(error: render::SceneRenderError) -> RetainedFailureCategory {
    use render::{SceneGpuError, SceneRenderError, ScopedGpuErrorCategory};

    let scoped = match error {
        SceneRenderError::Gpu(category)
        | SceneRenderError::Target(SceneGpuError::Gpu(category)) => Some(category),
        _ => None,
    };
    match scoped {
        Some(ScopedGpuErrorCategory::OutOfMemory) => RetainedFailureCategory::DeviceOutOfMemory,
        Some(ScopedGpuErrorCategory::Internal) => RetainedFailureCategory::DeviceInternal,
        Some(ScopedGpuErrorCategory::Validation) | None => {
            RetainedFailureCategory::SceneCandidateEncode
        }
    }
}

#[allow(dead_code)] // Maps errors for the dormant Task 12 host installation entrypoint.
fn scene_gpu_failure(error: render::SceneGpuError) -> RetainedFailureCategory {
    use render::{SceneGpuError, ScopedGpuErrorCategory};

    match error {
        SceneGpuError::Gpu(ScopedGpuErrorCategory::OutOfMemory) => {
            RetainedFailureCategory::DeviceOutOfMemory
        }
        SceneGpuError::Gpu(ScopedGpuErrorCategory::Internal) => {
            RetainedFailureCategory::DeviceInternal
        }
        SceneGpuError::Gpu(ScopedGpuErrorCategory::Validation) | SceneGpuError::InvalidUpload => {
            RetainedFailureCategory::SceneCandidateEncode
        }
        _ => RetainedFailureCategory::UnsupportedRaster,
    }
}

#[allow(dead_code)] // Maps errors for the dormant Task 12 host scene service.
fn scene_candidate_preparation_failure(
    error: &SceneCandidatePreparationError,
) -> RetainedFailureCategory {
    match error {
        SceneCandidatePreparationError::Gpu(render::SceneGpuError::Gpu(
            render::ScopedGpuErrorCategory::OutOfMemory,
        )) => RetainedFailureCategory::DeviceOutOfMemory,
        SceneCandidatePreparationError::Gpu(render::SceneGpuError::Gpu(
            render::ScopedGpuErrorCategory::Internal,
        )) => RetainedFailureCategory::DeviceInternal,
        _ => RetainedFailureCategory::SceneCandidateEncode,
    }
}
pub(super) fn physical_dimension(logical: f64, scale: f64) -> u32 {
    (logical * scale).round().clamp(1.0, f64::from(u32::MAX)) as u32
}
