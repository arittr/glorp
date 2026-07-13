//! AppKit layer activation and live wgpu surface ownership.

use std::ffi::c_void;
use std::sync::Arc;
use std::time::Instant;

use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::NSSize;
use objc2_quartz_core::CAMetalLayer;

use super::*;

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
    raster_worker: RasterWorker,
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
    pub(super) metrics: CompanionRuntimeMetrics,
    surface_epoch: u64,
}

pub(super) struct Pipelines {
    pub(super) normal: wgpu::RenderPipeline,
    pub(super) multiply: wgpu::RenderPipeline,
    pub(super) screen: wgpu::RenderPipeline,
    pub(super) add: wgpu::RenderPipeline,
    pub(super) replace: wgpu::RenderPipeline,
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

impl PreparedRetainedHost {
    /// Builds the CAMetalLayer, wgpu surface, device, configuration, and
    /// pipelines against a layer that stays detached from the view. A
    /// CAMetalLayer renders fine while detached; installing it on the view later
    /// is what makes it visible. Any failure here leaves the view untouched.
    pub(in crate::companion) fn prepare(
        view: &NSView,
        mailbox: GpuErrorMailbox,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let raster_worker =
            RasterWorker::launch().map_err(|_| RetainedFailureCategory::RasterWorkerUnavailable)?;
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
        let gpu_error_sender = mailbox.sender();
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
        // Composite in gamma space to match CoreGraphics/Smooth: a linear
        // (non-sRGB) target blends the stored premultiplied-sRGB values directly,
        // with no sRGB→linear→sRGB round-trip. The default surface format is the
        // sRGB variant; drop the sRGB suffix so the raw sRGB-space values are what
        // get blended. Metal's CAMetalLayer surface supports both variants.
        config.format = config.format.remove_srgb_suffix();
        surface.configure(&device, &config);
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
                raster_worker,
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
                metrics,
                surface_epoch: 1,
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
        let result = capture::RetainedCaptureTarget::new(&mut self.host).capture(frame);
        if result.is_ok() {
            self.host.metrics.record_capture_success();
        } else {
            self.host.metrics.record_capture_failure();
        }
        result
    }

    pub(crate) fn record_injected_capture_failure(&mut self) {
        self.host.metrics.record_capture_attempt();
        self.host.metrics.record_capture_failure();
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

    /// Drives a real retained GPU instance ring and queue through a bounded
    /// 4,500-frame virtual-time segment. Wall time is intentionally decoupled
    /// from the fixed 4 Hz semantic cadence; each iteration advances 250 ms of
    /// virtual time and waits for its GPU submission to complete without sleeping,
    /// measuring steady-state lifetime without manufacturing an in-flight backlog.
    pub(crate) fn run_virtual_lifetime_audit(
        &mut self,
        frames: u64,
        prepare: impl FnMut(
            LifetimeAuditPhase,
            u64,
            time::OffsetDateTime,
        ) -> std::result::Result<
            (crate::companion::app::PreparedCompanionFrame, u64),
            RetainedFailureCategory,
        >,
    ) -> std::result::Result<(), RetainedFailureCategory> {
        let mut executor = GpuLifetimeAuditExecutor {
            host: &mut self.host,
            prepare,
            last_submission: None,
        };
        let audit = run_lifetime_schedule(&mut executor, frames)?;
        executor.host.metrics.record_lifetime_audit(audit);
        Ok(())
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

    pub(crate) fn record_hidden_tick(&mut self, tick_start: RuntimeWorkCounters) {
        self.host.metrics.record_hidden_tick(tick_start);
    }

    pub(crate) fn record_fallback(&mut self) {
        self.host.metrics.record_fallback();
    }

    pub(crate) fn runtime_metrics_snapshot(
        &self,
        inventory: CompanionCapacityInventory,
    ) -> CompanionRuntimeMetricsSnapshot {
        let resource_generation = self.current_resource_generation();
        self.host.metrics.snapshot(
            RuntimeIdentity {
                device_epoch: None,
                surface_epoch: self.host.surface_epoch,
                layout_generation: None,
                resource_generation: (resource_generation != 0).then_some(resource_generation),
                semantic_revision: None,
                frame_revision: None,
                present_attempt: self.host.frame_counter,
            },
            inventory,
            RuntimeFixtureIdentity {
                fixture_id: "glorp-scene-baseline-v2",
                seed: "glorp-scene-baseline-v1",
                update_source: "fixed-initial-state-no-live-polling",
                cadence_ms: 250,
                logical_width: f64::from(self.host.physical_width) / self.host.backing_scale,
                logical_height: f64::from(self.host.physical_height) / self.host.backing_scale,
                physical_width: self.host.physical_width,
                physical_height: self.host.physical_height,
                backing_scale: self.host.backing_scale,
            },
        )
    }
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
        self.gpu_errors.drain()
    }

    /// Dev/test-only: posts a static category to this host's own error mailbox so
    /// the next main-thread [`drain_gpu_error`](Self::drain_gpu_error) observes it
    /// exactly as it would a real asynchronous device fault. Compiled only with
    /// dev-preview so a release build cannot inject faults.
    #[cfg(feature = "dev-preview")]
    pub(in crate::companion) fn inject_gpu_fault(&self, category: RetainedFailureCategory) {
        let _ = self.gpu_errors.sender().send(category);
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
                if let Err(category) = self.resize_if_needed(view) {
                    let mut progress = FrameProgress::new(frame_id, 0);
                    fail(&mut progress, category);
                    return progress;
                }
            }
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
                self.record_metrics(CompanionRuntimeMetrics::record_skip);
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
                    self.record_metrics(CompanionRuntimeMetrics::record_skip);
                    skip(&mut progress, SkipReason::Outdated);
                    return progress;
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    self.record_metrics(CompanionRuntimeMetrics::record_skip);
                    skip(&mut progress, SkipReason::Timeout);
                    return progress;
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    self.record_metrics(CompanionRuntimeMetrics::record_skip);
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
            if self.raster_worker.cancel_with_epoch(epoch).is_err() {
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
        match self.raster_worker.try_recv() {
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
                .raster_worker
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
            if self.raster_worker.cancel_with_epoch(epoch).is_err() {
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
        self.record_metrics(CompanionRuntimeMetrics::record_skip);
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

    fn resize_if_needed(
        &mut self,
        view: &NSView,
    ) -> std::result::Result<(), RetainedFailureCategory> {
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
            return Ok(());
        }
        self.physical_width = width;
        self.physical_height = height;
        self.backing_scale = scale;
        self.config.width = width;
        self.config.height = height;
        unsafe {
            self.layer
                .setDrawableSize(NSSize::new(f64::from(width), f64::from(height)))
        };
        self.surface.configure(&self.device, &self.config);
        self.surface_epoch = self.surface_epoch.saturating_add(1);
        Ok(())
    }
}
pub(super) fn physical_dimension(logical: f64, scale: f64) -> u32 {
    (logical * scale).round().clamp(1.0, f64::from(u32::MAX)) as u32
}
