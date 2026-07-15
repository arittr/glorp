use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::mpsc::{Receiver, Sender};

/// One observable checkpoint in the retained render path. The ordering is the
/// ladder a frame climbs; `mark` only accepts the milestone that immediately
/// follows the highest one already observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FrameMilestone {
    Prepared,
    Encoded,
    Submitted,
    SurfacePresentCalled,
    GpuCompleted,
    /// Reached once the readback capture path lands (Task 6).
    ReadbackCompleted,
}

impl FrameMilestone {
    /// The milestone that must already be observed before this one may be
    /// marked. `Prepared` opens the ladder and has no predecessor.
    const fn predecessor(self) -> Option<Self> {
        match self {
            Self::Prepared => None,
            Self::Encoded => Some(Self::Prepared),
            Self::Submitted => Some(Self::Encoded),
            Self::SurfacePresentCalled => Some(Self::Submitted),
            Self::GpuCompleted => Some(Self::SurfacePresentCalled),
            Self::ReadbackCompleted => Some(Self::GpuCompleted),
        }
    }
}

/// Why a frame reached a terminal state without presenting its surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkipReason {
    ResourcePreparation,
    Outdated,
    Timeout,
    Occluded,
}

/// The single terminal outcome of a retained render attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameDisposition {
    SurfacePresentCalled,
    /// Reached once the readback capture path lands (Task 6).
    #[allow(dead_code)]
    Captured,
    Skipped(SkipReason),
    Failed(RetainedFailureCategory),
}

/// Static failure category for a retained render outcome or an asynchronous GPU
/// device fault. It stays `'static` so it can travel through the GPU error
/// mailbox from the wgpu callback to the main thread without allocating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetainedFailureCategory {
    SurfaceCreate,
    AdapterUnavailable,
    DeviceUnavailable,
    SurfaceUnavailable,
    AtlasUnavailable,
    FontUnavailable,
    RasterWorkerUnavailable,
    #[allow(dead_code)] // Produced by the Task 12 host path once Task 14 routes it live.
    SceneCandidateEncode,
    PresentationStalled,
    UnsupportedRaster,
    SurfaceLost,
    SurfaceValidation,
    DeviceOutOfMemory,
    DeviceValidation,
    DeviceInternal,
    /// The frozen review frame's renderer is not the Smooth/Retained path, so
    /// there is no GPU scene to read back.
    CaptureUnsupportedVariant,
    /// The bounded device poll did not observe the readback submission finish.
    CapturePollTimeout,
    /// Mapping the readback staging buffer failed or its callback never returned.
    CaptureMapFailed,
    /// The mapped readback buffer was shorter than the frame's row layout.
    CaptureBufferTooShort,
    /// The bounded lifetime harness could not complete a submitted GPU frame.
    LifetimeGpuPoll,
    /// The bounded lifetime harness could not obtain a nonzero current RSS sample.
    LifetimeRssUnavailable,
    /// The deterministic lifetime fixture failed production frame preparation.
    LifetimeFramePreparation,
    /// Writing a capture artifact (png/manifest) to disk failed. Only produced by
    /// the dev/test write-failure fault injection.
    #[cfg(feature = "dev-preview")]
    CaptureWriteFailed,
}

impl RetainedFailureCategory {
    pub(crate) const fn category(self) -> &'static str {
        match self {
            Self::SurfaceCreate => "retained-surface-create",
            Self::AdapterUnavailable => "retained-adapter-unavailable",
            Self::DeviceUnavailable => "retained-device-unavailable",
            Self::SurfaceUnavailable => "retained-surface-unavailable",
            Self::AtlasUnavailable => "retained-atlas-unavailable",
            Self::FontUnavailable => "retained-font-unavailable",
            Self::RasterWorkerUnavailable => "retained-raster-worker-unavailable",
            Self::SceneCandidateEncode => "retained-scene-candidate-encode",
            Self::PresentationStalled => "retained-presentation-stalled",
            Self::UnsupportedRaster => "retained-unsupported-raster",
            Self::SurfaceLost => "retained-surface-lost",
            Self::SurfaceValidation => "retained-surface-validation",
            Self::DeviceOutOfMemory => "retained-device-out-of-memory",
            Self::DeviceValidation => "retained-device-validation",
            Self::DeviceInternal => "retained-device-internal",
            Self::CaptureUnsupportedVariant => "retained-capture-unsupported-variant",
            Self::CapturePollTimeout => "retained-capture-poll-timeout",
            Self::CaptureMapFailed => "retained-capture-map-failed",
            Self::CaptureBufferTooShort => "retained-capture-buffer-too-short",
            Self::LifetimeGpuPoll => "retained-lifetime-gpu-poll",
            Self::LifetimeRssUnavailable => "retained-lifetime-rss-unavailable",
            Self::LifetimeFramePreparation => "retained-lifetime-frame-preparation",
            #[cfg(feature = "dev-preview")]
            Self::CaptureWriteFailed => "retained-capture-write-failed",
        }
    }
}

/// A rejected progress transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameProgressError {
    /// `mark` was called without its immediately-preceding milestone observed.
    MilestoneOutOfOrder,
    /// A terminal disposition was already assigned.
    TerminalAlreadyAssigned,
}

/// Observable progress of a single retained render attempt. Milestones are
/// recorded monotonically up the ladder and exactly one terminal disposition is
/// assigned. A skipped or failed disposition leaves the present and readback
/// milestones unobserved so downstream capture never treats a skip as a paint.
pub(crate) struct FrameProgress {
    #[allow(dead_code)] // Read by Task 6 capture correlation.
    frame_id: u64,
    #[allow(dead_code)] // Read by Task 9 resource-generation reconciliation.
    resource_generation: u64,
    milestones: BTreeSet<FrameMilestone>,
    disposition: Option<FrameDisposition>,
}

impl FrameProgress {
    pub(super) fn new(frame_id: u64, resource_generation: u64) -> Self {
        Self {
            frame_id,
            resource_generation,
            milestones: BTreeSet::new(),
            disposition: None,
        }
    }

    /// Records reaching `milestone`. Fails if a terminal disposition is already
    /// assigned, or if the immediately-preceding milestone is not yet observed.
    pub(super) fn mark(&mut self, milestone: FrameMilestone) -> Result<(), FrameProgressError> {
        if self.disposition.is_some() {
            return Err(FrameProgressError::TerminalAlreadyAssigned);
        }
        if let Some(previous) = milestone.predecessor() {
            if !self.milestones.contains(&previous) {
                return Err(FrameProgressError::MilestoneOutOfOrder);
            }
        }
        self.milestones.insert(milestone);
        Ok(())
    }

    /// Assigns the terminal disposition. Fails if one is already assigned. A
    /// `SurfacePresentCalled` disposition additionally observes that milestone;
    /// skips and failures observe nothing further.
    pub(super) fn finish(
        &mut self,
        disposition: FrameDisposition,
    ) -> Result<(), FrameProgressError> {
        if self.disposition.is_some() {
            return Err(FrameProgressError::TerminalAlreadyAssigned);
        }
        if matches!(disposition, FrameDisposition::SurfacePresentCalled) {
            self.milestones.insert(FrameMilestone::SurfacePresentCalled);
        }
        self.disposition = Some(disposition);
        Ok(())
    }

    #[allow(dead_code)] // Exercised by the transition tests; Task 6 reads capture milestones.
    pub(super) fn observed(&self, milestone: FrameMilestone) -> bool {
        self.milestones.contains(&milestone)
    }

    pub(crate) fn disposition(&self) -> Option<FrameDisposition> {
        self.disposition
    }
}

/// Single-producer channel from the wgpu uncaptured-error callback to the main
/// thread. The callback holds only a cloned [`Sender`] and emits `'static`
/// categories; the main thread drains the [`Receiver`] before treating any
/// present as a success.
pub(crate) struct GpuErrorMailbox {
    sender: Sender<GpuErrorEvent>,
    receiver: Receiver<GpuErrorEvent>,
    deferred: RefCell<Vec<GpuErrorEvent>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GpuErrorEvent {
    device: crate::presentation::companion_scene::DeviceEpoch,
    category: RetainedFailureCategory,
}

#[derive(Clone)]
pub(super) struct DeviceGpuErrorSender {
    device: crate::presentation::companion_scene::DeviceEpoch,
    sender: Sender<GpuErrorEvent>,
}

impl DeviceGpuErrorSender {
    pub(super) fn send(&self, category: RetainedFailureCategory) -> Result<(), ()> {
        self.sender
            .send(GpuErrorEvent { device: self.device, category })
            .map_err(|_| ())
    }
}

impl GpuErrorMailbox {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            sender,
            receiver,
            deferred: RefCell::new(Vec::new()),
        }
    }

    /// A cloned sender for the wgpu error callback. The callback captures
    /// nothing else, keeping it free of live state and dynamic formatting.
    pub(super) fn sender_for(
        &self,
        device: crate::presentation::companion_scene::DeviceEpoch,
    ) -> DeviceGpuErrorSender {
        DeviceGpuErrorSender { device, sender: self.sender.clone() }
    }

    /// Drains every queued fault, returning the most recent category if any.
    pub(super) fn drain(&self) -> Option<RetainedFailureCategory> {
        let mut latest = None;
        for event in self.deferred.borrow_mut().drain(..) {
            latest = Some(event.category);
        }
        while let Ok(event) = self.receiver.try_recv() {
            latest = Some(event.category);
        }
        latest
    }

    /// Drains queued callbacks and returns only the latest fault emitted by the
    /// requested device generation. Late callbacks from retired devices are
    /// discarded so they cannot poison a successor activation.
    pub(super) fn drain_for(
        &self,
        device: crate::presentation::companion_scene::DeviceEpoch,
    ) -> Option<RetainedFailureCategory> {
        let mut latest = None;
        self.deferred.borrow_mut().retain(|event| {
            if event.device == device {
                latest = Some(event.category);
                false
            } else {
                event.device > device
            }
        });
        while let Ok(event) = self.receiver.try_recv() {
            if event.device == device {
                latest = Some(event.category);
            } else if event.device > device {
                self.deferred.borrow_mut().push(event);
            }
        }
        latest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skipped_frame_cannot_claim_surface_or_readback() {
        let mut progress = FrameProgress::new(7, 3);
        progress.mark(FrameMilestone::Prepared).unwrap();
        progress
            .finish(FrameDisposition::Skipped(SkipReason::Occluded))
            .unwrap();
        assert!(!progress.observed(FrameMilestone::SurfacePresentCalled));
        assert!(!progress.observed(FrameMilestone::ReadbackCompleted));
    }

    #[test]
    fn resource_preparation_skip_is_nonfatal_and_claims_no_frame_milestone() {
        let mut progress = FrameProgress::new(9, 0);
        progress
            .finish(FrameDisposition::Skipped(SkipReason::ResourcePreparation))
            .unwrap();
        assert_eq!(
            progress.disposition(),
            Some(FrameDisposition::Skipped(SkipReason::ResourcePreparation))
        );
        assert!(!progress.observed(FrameMilestone::Prepared));
        assert!(!progress.observed(FrameMilestone::SurfacePresentCalled));
    }

    #[test]
    fn milestones_are_monotonic_and_terminal_is_single_assignment() {
        let mut progress = FrameProgress::new(8, 4);
        progress.mark(FrameMilestone::Prepared).unwrap();
        assert!(progress.mark(FrameMilestone::Submitted).is_err());
        progress.mark(FrameMilestone::Encoded).unwrap();
        progress.mark(FrameMilestone::Submitted).unwrap();
        progress
            .finish(FrameDisposition::SurfacePresentCalled)
            .unwrap();
        assert!(progress
            .finish(FrameDisposition::Failed(
                RetainedFailureCategory::SurfaceLost,
            ))
            .is_err());
    }

    #[test]
    fn mailbox_drains_the_latest_fault_and_then_reports_empty() {
        let mailbox = GpuErrorMailbox::new();
        assert_eq!(mailbox.drain(), None);
        let sender = mailbox.sender_for(crate::presentation::companion_scene::DeviceEpoch(1));
        sender
            .send(RetainedFailureCategory::DeviceOutOfMemory)
            .unwrap();
        sender
            .send(RetainedFailureCategory::DeviceValidation)
            .unwrap();
        assert_eq!(
            mailbox.drain(),
            Some(RetainedFailureCategory::DeviceValidation)
        );
        assert_eq!(mailbox.drain(), None);
    }

    #[test]
    fn device_scoped_mailbox_discards_retired_callbacks_but_preserves_successors() {
        use crate::presentation::companion_scene::DeviceEpoch;

        let mailbox = GpuErrorMailbox::new();
        mailbox
            .sender_for(DeviceEpoch(2))
            .send(RetainedFailureCategory::DeviceValidation)
            .unwrap();
        assert_eq!(mailbox.drain_for(DeviceEpoch(1)), None);
        assert_eq!(
            mailbox.drain_for(DeviceEpoch(2)),
            Some(RetainedFailureCategory::DeviceValidation)
        );
        mailbox
            .sender_for(DeviceEpoch(1))
            .send(RetainedFailureCategory::DeviceInternal)
            .unwrap();
        assert_eq!(mailbox.drain_for(DeviceEpoch(2)), None);
    }
}
