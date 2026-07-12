#![cfg(all(target_os = "macos", feature = "retained-renderer"))]

use std::ffi::c_void;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::NSSize;
use objc2_quartz_core::CAMetalLayer;

use crate::presentation::smooth::{
    SmoothBlendMode, SmoothClip, SmoothCompanionLayer, SmoothCompanionScenePlan, SmoothFill,
    SmoothLayerItem, SmoothLayerMotionBinding, SmoothPoint, SmoothRgba8, SmoothShapeGeometry,
};
use crate::round::draw::{RoundColor, RoundDrawCommand, RoundDrawKind};
use crate::round::hud::{
    perimeter_gauge_colors, perimeter_gauge_layout, prepare_hud_layout,
    prepared_perimeter_gauge_arcs, stat_gap_box, tank_core_color, CompanionHudText, GaugeFractions,
    GaugeLane, HudLineMetrics, LineCap, COMPANION_GAUGE_GAP_DEG,
};
use crate::round::layout::RoundAperture;

use super::app::{CompanionGridMetrics, PreparedGaugeFrame};

mod capture;
mod metrics;
mod parity;
mod presentation;
mod resources;

pub(crate) use capture::CanonicalRgbaFrame;
pub(crate) use metrics::{
    duration_us, CapacityContract, CompanionCapacityInventory, CompanionRuntimeMetrics,
    CompanionRuntimeMetricsSnapshot, GpuAllocationKind, LifetimeAuditSnapshot,
    RuntimeFixtureIdentity, RuntimeIdentity, RuntimeWorkCounters,
};
pub(crate) use presentation::{
    FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox, RetainedFailureCategory,
    SkipReason,
};
use resources::{
    CompiledGlyphAtlas, CompiledRetainedResources, FragmentGlyphMode, GlyphAtlasEntry, GlyphKey,
    GlyphRepertoireManifest, RetainedResourceCounters, RETAINED_ATLAS_POINT_SIZE,
};

use crate::round::smooth::CompanionContentIdentity;

/// The point size the retained atlas rasterizes glyphs at; the on-screen quad
/// scale divides the display font size by this. Matches the manifest's
/// production atlas point size.
const GLYPH_FONT_SIZE: f64 = RETAINED_ATLAS_POINT_SIZE;
/// Logical-pixel margin the analytic-arc primitive rect extends past the stroke's
/// outer radius, so the outer edge's one-physical-pixel coverage band has room to
/// fall off inside the rect instead of being clipped at its boundary. A couple of
/// logical pixels covers the outer half-band at every backing scale.
const ARC_AA_MARGIN: f64 = 2.0;

/// Primitive kind for a translucent [`SmoothFill::RadialGradient`]: a premultiplied
/// interpolation from the authored inner colour at the shape's centre to the outer
/// colour at its rim, with the round primitive's analytic edge coverage. This is a
/// distinct path from the opaque tank falloff (kind 3): the tank base holds an
/// output-space sRGB dither and a constant opaque alpha, so it cannot express the
/// inner→outer alpha falloff a soft cast shadow needs. The value is shared with the
/// shader's radial branch in `retained.wgsl`.
const RADIAL_GRADIENT_KIND: f32 = 6.0;

pub(super) struct RetainedChrome<'a> {
    pub(super) mood_aura: [f32; 4],
    pub(super) pet_center_col: f64,
    pub(super) pet_center_row: f64,
    pub(super) pet_width_cells: f64,
    pub(super) gauges: PreparedGaugeFrame,
    pub(super) overlays: &'a [RoundDrawCommand],
    pub(super) hud: &'a CompanionHudText,
    pub(super) hud_font_size: f64,
    pub(super) dim_overlay: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuPrimitive {
    rect: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    uv: [f32; 4],
    params: [f32; 4],
    clip_rect: [f32; 4],
    clip_ellipse: [f32; 4],
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
}

impl GpuPrimitive {
    const ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4
    ];

    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
}

struct PreparedGpuFrame {
    primitives: Vec<GpuPrimitive>,
    blends: Vec<SmoothBlendMode>,
}

/// Instance buffers held in a small ring so a frame writes into a slot the GPU is
/// unlikely to still be reading from the previous present, avoiding a
/// write-vs-read stall without CPU-side fences.
const INSTANCE_RING_LEN: usize = 3;

/// One instance's stride in the persistent buffer.
const INSTANCE_STRIDE: usize = std::mem::size_of::<GpuPrimitive>();

/// Maximum primitive count observed by the deterministic full current-renderer
/// fixture matrix. The fixed minimum ring adds explicit non-growth headroom;
/// larger generation requests remain possible but are not ordinary frames.
const FULL_FIXTURE_INSTANCE_MAX: usize = 782;
const FULL_FIXTURE_INSTANCE_HEADROOM: usize = 242;
const FIXED_INSTANCE_RING_MIN: usize = FULL_FIXTURE_INSTANCE_MAX + FULL_FIXTURE_INSTANCE_HEADROOM;

/// A capacity-bounded ring of persistent `VERTEX | COPY_DST` instance buffers.
///
/// A frame writes its instances into the next ring slot with `queue.write_buffer`
/// and draws only that slot's current instance count. The ring grows — every
/// buffer reallocated to the larger capacity — only when a frame's instance count
/// exceeds the current capacity, which is a declared layout/semantic change, not
/// ordinary motion. Once warmed to the steady-state high-water mark, ordinary
/// animation reuses the buffers and only writes, so no buffer is ever recreated.
struct PersistentFrameBuffers {
    ring: Vec<wgpu::Buffer>,
    capacity_instances: usize,
    cursor: usize,
}

impl PersistentFrameBuffers {
    fn new() -> Self {
        Self {
            ring: Vec::new(),
            capacity_instances: 0,
            cursor: 0,
        }
    }

    /// Guarantees every ring buffer can hold at least `instances` instances.
    /// Reallocates the whole ring — counting one buffer creation per slot — only
    /// when the request exceeds the current capacity; a request at or below the
    /// current capacity reuses the existing buffers and creates nothing.
    fn ensure_instance_capacity(
        &mut self,
        instances: usize,
        device: &wgpu::Device,
        counters: &mut RetainedResourceCounters,
    ) {
        if !self.ring.is_empty() && instances <= self.capacity_instances {
            return;
        }
        let capacity = persistent_instance_capacity(instances);
        let size = (capacity * INSTANCE_STRIDE) as wgpu::BufferAddress;
        self.ring = (0..INSTANCE_RING_LEN)
            .map(|_| {
                counters.buffer_creations += 1;
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("glorp-retained-instances"),
                    size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        self.capacity_instances = capacity;
        self.cursor = 0;
    }

    /// Writes `instances` into the next ring slot and records the write. The
    /// caller must have called [`Self::ensure_instance_capacity`] for at least
    /// `instances.len()` first. `instance_writes`/`instance_write_bytes` advance
    /// only after the queue write is issued.
    fn write_frame_instances(
        &mut self,
        queue: &wgpu::Queue,
        instances: &[GpuPrimitive],
        counters: &mut RetainedResourceCounters,
    ) {
        debug_assert!(
            !self.ring.is_empty() && instances.len() <= self.capacity_instances,
            "write_frame_instances requires ensure_instance_capacity first",
        );
        self.cursor = (self.cursor + 1) % self.ring.len();
        let bytes: &[u8] = bytemuck::cast_slice(instances);
        queue.write_buffer(&self.ring[self.cursor], 0, bytes);
        counters.instance_writes += 1;
        counters.instance_write_bytes += bytes.len() as u64;
    }

    /// The ring buffer holding the current frame's instances.
    fn current_buffer(&self) -> &wgpu::Buffer {
        &self.ring[self.cursor]
    }
}

fn persistent_instance_capacity(instances: usize) -> usize {
    if instances <= FIXED_INSTANCE_RING_MIN {
        FIXED_INSTANCE_RING_MIN
    } else {
        instances.next_power_of_two()
    }
}

/// The off-screen capture intermediate and its mappable staging buffer, keyed by
/// the physical size and surface format they were built for. A resize or
/// backing-scale change replaces them once; ordinary captures reuse them.
struct PersistentCaptureResources {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    intermediate: wgpu::Texture,
    intermediate_view: wgpu::TextureView,
    staging: wgpu::Buffer,
}

/// The GPU resources for one active resource generation: the compiled atlas plus
/// the declared-content identity and backing scale it was compiled for, so the
/// host knows when a generation change requires recompiling, and the uploaded
/// texture + bind group.
struct ActiveGlyphResources {
    identity: CompanionContentIdentity,
    backing_scale: f64,
    resources: CompiledRetainedResources,
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

pub(super) struct RetainedHost {
    // Surface must drop before the retained CAMetalLayer.
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    layer: Retained<CAMetalLayer>,
    pipelines: Pipelines,
    atlas_layout: wgpu::BindGroupLayout,
    glyph_resources: Option<ActiveGlyphResources>,
    frame_buffers: PersistentFrameBuffers,
    capture_resources: Option<PersistentCaptureResources>,
    counters: RetainedResourceCounters,
    physical_width: u32,
    physical_height: u32,
    backing_scale: f64,
    frame_counter: u64,
    activation_render_owner_us: u64,
    activation_excluded_appkit_us: u64,
    activation_recorded: bool,
    gpu_errors: GpuErrorMailbox,
    metrics: CompanionRuntimeMetrics,
    surface_epoch: u64,
}

struct Pipelines {
    normal: wgpu::RenderPipeline,
    multiply: wgpu::RenderPipeline,
    screen: wgpu::RenderPipeline,
    add: wgpu::RenderPipeline,
    replace: wgpu::RenderPipeline,
}

impl Pipelines {
    fn get(&self, blend: SmoothBlendMode) -> &wgpu::RenderPipeline {
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
struct LayerActivationState {
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
    fn preflight_failed(&mut self) {
        self.attached = false;
        self.appkit_restored = true;
    }

    fn attached(&self) -> bool {
        self.attached
    }

    #[allow(dead_code)] // Read by the activation-state tests.
    fn appkit_restored(&self) -> bool {
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
struct LayerActivationGuard<'a> {
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
    fn for_test(flag: std::rc::Rc<std::cell::Cell<bool>>) -> Self {
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
pub(super) struct PreparedRetainedHost {
    host: RetainedHost,
}

/// A retained host whose Metal layer is installed on the view and rendering.
/// Derefs to the inner [`RetainedHost`] so `render` and the mailbox drain read
/// through transparently.
pub(super) struct ActiveRetainedHost {
    host: RetainedHost,
}

impl PreparedRetainedHost {
    /// Builds the CAMetalLayer, wgpu surface, device, configuration, and
    /// pipelines against a layer that stays detached from the view. A
    /// CAMetalLayer renders fine while detached; installing it on the view later
    /// is what makes it visible. Any failure here leaves the view untouched.
    pub(super) fn prepare(
        view: &NSView,
        mailbox: GpuErrorMailbox,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
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
        let mut metrics = CompanionRuntimeMetrics::default();
        metrics.discard_initial_visible_ticks(20);
        metrics.record_persistent_gpu_create(resource_object_count(counters));
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
                frame_buffers: PersistentFrameBuffers::new(),
                capture_resources: None,
                counters,
                physical_width: width,
                physical_height: height,
                backing_scale: scale,
                frame_counter: 0,
                activation_render_owner_us: 0,
                activation_excluded_appkit_us: 0,
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
    pub(super) fn activate(
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
    pub(super) fn restore_appkit(view: &NSView) {
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
        let result = capture::RetainedCaptureTarget::new(&mut self.host).capture(frame);
        self.host.metrics.record_capture();
        result
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

    /// Drives a real retained GPU instance ring and queue through a bounded
    /// 4,500-frame virtual-time segment. Wall time is intentionally decoupled
    /// from the fixed 4 Hz semantic cadence; each iteration advances 250 ms of
    /// virtual time while queue work is submitted as fast as the device allows.
    pub(crate) fn run_virtual_lifetime_audit(&mut self, frames: u64) {
        const CADENCE_MS: u64 = 250;
        const DRIVER_WARMUP_FRAMES: u64 = 4_500;
        let instances = vec![GpuPrimitive::zeroed(); FIXED_INSTANCE_RING_MIN];
        let submit = |host: &mut RetainedHost| {
            host.frame_buffers
                .write_frame_instances(&host.queue, &instances, &mut host.counters);
            let encoder = host
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("glorp-retained-lifetime-audit"),
                });
            host.queue.submit([encoder.finish()])
        };
        let mut last_submission = None;
        let mut rss_warmup_peak_bytes = 0_u64;
        let mut gpu_warmup_peak_bytes = 0_u64;
        for frame in 0..DRIVER_WARMUP_FRAMES {
            last_submission = Some(submit(&mut self.host));
            if (frame + 1) % 64 == 0 {
                if let Some(submission) = last_submission.take() {
                    let _ = self.host.device.poll(wgpu::PollType::Wait {
                        submission_index: Some(submission),
                        timeout: Some(Duration::from_secs(5)),
                    });
                }
            }
            if (frame + 1) % 256 == 0 {
                rss_warmup_peak_bytes =
                    rss_warmup_peak_bytes.max(current_process_rss_bytes().unwrap_or(0));
                gpu_warmup_peak_bytes = gpu_warmup_peak_bytes.max(
                    self.host
                        .metrics
                        .gpu_accounting_snapshot()
                        .current
                        .total_bytes,
                );
            }
        }
        if let Some(submission) = last_submission.take() {
            let _ = self.host.device.poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            });
        }
        let rss_warmup_bytes = current_process_rss_bytes().unwrap_or(0);
        let gpu_warmup_bytes = self
            .host
            .metrics
            .gpu_accounting_snapshot()
            .current
            .total_bytes;
        rss_warmup_peak_bytes = rss_warmup_peak_bytes.max(rss_warmup_bytes);
        gpu_warmup_peak_bytes = gpu_warmup_peak_bytes.max(gpu_warmup_bytes);
        let mut rss_peak_bytes = rss_warmup_bytes;
        let mut gpu_peak_bytes = gpu_warmup_bytes;
        for frame in 0..frames {
            last_submission = Some(submit(&mut self.host));
            if (frame + 1) % 256 == 0 {
                if let Some(submission) = last_submission.take() {
                    let _ = self.host.device.poll(wgpu::PollType::Wait {
                        submission_index: Some(submission),
                        timeout: Some(Duration::from_secs(5)),
                    });
                }
                rss_peak_bytes = rss_peak_bytes.max(current_process_rss_bytes().unwrap_or(0));
                gpu_peak_bytes = gpu_peak_bytes.max(
                    self.host
                        .metrics
                        .gpu_accounting_snapshot()
                        .current
                        .total_bytes,
                );
            }
        }
        if let Some(submission) = last_submission {
            let _ = self.host.device.poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            });
        }
        let rss_final_bytes = current_process_rss_bytes().unwrap_or(0);
        let gpu_final_bytes = self
            .host
            .metrics
            .gpu_accounting_snapshot()
            .current
            .total_bytes;
        self.host
            .metrics
            .record_lifetime_audit(LifetimeAuditSnapshot {
                frames,
                cadence_ms: CADENCE_MS,
                virtual_elapsed_ms: frames.saturating_mul(CADENCE_MS),
                rss_warmup_bytes,
                rss_warmup_peak_bytes,
                rss_final_bytes,
                rss_peak_bytes: rss_peak_bytes.max(rss_final_bytes),
                gpu_warmup_bytes,
                gpu_warmup_peak_bytes,
                gpu_final_bytes,
                gpu_peak_bytes: gpu_peak_bytes.max(gpu_final_bytes),
            });
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

fn current_process_rss_bytes() -> Option<u64> {
    #[repr(C)]
    #[derive(Default)]
    struct RUsageInfoV0 {
        uuid: [u8; 16],
        user_time: u64,
        system_time: u64,
        package_idle_wakeups: u64,
        interrupt_wakeups: u64,
        pageins: u64,
        wired_size: u64,
        resident_size: u64,
        physical_footprint: u64,
        process_start_abstime: u64,
        process_exit_abstime: u64,
    }

    #[link(name = "proc")]
    extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut std::ffi::c_void) -> i32;
    }

    const RUSAGE_INFO_V0: i32 = 0;
    let mut usage = RUsageInfoV0::default();
    // SAFETY: `usage` has the documented RUSAGE_INFO_V0 layout and remains live
    // for the duration of the call. The current process id fits Darwin's pid_t.
    let rc = unsafe {
        proc_pid_rusage(
            std::process::id() as i32,
            RUSAGE_INFO_V0,
            std::ptr::from_mut(&mut usage).cast(),
        )
    };
    (rc == 0 && usage.resident_size > 0).then_some(usage.resident_size)
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
    pub(super) fn drain_gpu_error(&self) -> Option<RetainedFailureCategory> {
        self.gpu_errors.drain()
    }

    /// Dev/test-only: posts a static category to this host's own error mailbox so
    /// the next main-thread [`drain_gpu_error`](Self::drain_gpu_error) observes it
    /// exactly as it would a real asynchronous device fault. Compiled only with
    /// dev-preview so a release build cannot inject faults.
    #[cfg(feature = "dev-preview")]
    pub(super) fn inject_gpu_fault(&self, category: RetainedFailureCategory) {
        let _ = self.gpu_errors.sender().send(category);
    }

    #[allow(clippy::too_many_arguments)] // Explicit prepared-frame inputs keep retained independent of AppState.
    pub(super) fn render(
        &mut self,
        view: &NSView,
        plan: &SmoothCompanionScenePlan,
        draw_order: &[usize],
        metrics: CompanionGridMetrics,
        aperture: RoundAperture,
        background: [f32; 4],
        chrome: RetainedChrome<'_>,
        identity: &CompanionContentIdentity,
    ) -> FrameProgress {
        let activation_attempt_started = (!self.activation_recorded).then(Instant::now);
        let progress = (|| {
            let frame_id = self.next_frame_id();
            if let Err(category) = self.resize_if_needed(view) {
                let mut progress = FrameProgress::new(frame_id, 0);
                fail(&mut progress, category);
                return progress;
            }
            // Compile the full declared repertoire once per resource generation. A
            // per-frame glyph-set change never rebuilds; only a generation change
            // (species, font policy, or backing scale) does.
            if let Err(category) = self.ensure_resources(identity) {
                let mut progress = FrameProgress::new(frame_id, 0);
                fail(&mut progress, category);
                return progress;
            }
            let Some(generation) = self
                .glyph_resources
                .as_ref()
                .map(|active| active.resources.generation().value())
            else {
                let mut progress = FrameProgress::new(frame_id, 0);
                fail(&mut progress, RetainedFailureCategory::AtlasUnavailable);
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
            self.prepare_frame(&frame);
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
            self.queue.submit([encoder.finish()]);
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
                let activation_us = self
                    .activation_render_owner_us
                    .saturating_sub(self.activation_excluded_appkit_us)
                    .min(u64::from(u32::MAX)) as u32;
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
    /// count steady, so this only issues a `write_buffer` and never allocates.
    fn prepare_frame(&mut self, frame: &PreparedGpuFrame) {
        let before = self.counters;
        self.frame_buffers.ensure_instance_capacity(
            frame.primitives.len(),
            &self.device,
            &mut self.counters,
        );
        self.frame_buffers.write_frame_instances(
            &self.queue,
            &frame.primitives,
            &mut self.counters,
        );
        let delta = self.counters - before;
        let primitives = frame.primitives.len() as u32;
        let blended_draws = frame.blends.len() as u32;
        let cpu_bytes = (frame.primitives.capacity() * std::mem::size_of::<GpuPrimitive>()) as u64;
        let gpu_bytes =
            (self.frame_buffers.capacity_instances * INSTANCE_RING_LEN * INSTANCE_STRIDE) as u64;
        self.record_metrics(|metrics| {
            metrics.record_persistent_gpu_create(resource_object_count(delta));
            metrics.record_queue_write(delta.instance_write_bytes);
            metrics.observe_primitives(primitives);
            metrics.observe_blended_draws(blended_draws);
            metrics.observe_cpu_bytes(cpu_bytes);
            metrics.replace_gpu_allocation(GpuAllocationKind::InstanceRing, gpu_bytes);
        });
    }

    /// Encodes the prepared companion scene into `target_view` on `encoder`: one
    /// clear-loaded render pass that draws every primitive with its blend
    /// pipeline. Shared verbatim by the live surface [`Self::render`] path and
    /// the capture intermediate path so both rasterize identical geometry.
    fn encode_scene(
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

    /// Ensures the active glyph resources match the pet's declared-content
    /// `identity` at the current backing scale. Reuses the active generation when
    /// nothing changed; otherwise compiles the full declared repertoire into a
    /// fresh atlas and uploads it. Compile-before-replace: on a compile failure
    /// the previous generation stays active so the caller can keep rendering it or
    /// fall back explicitly.
    fn ensure_resources(
        &mut self,
        identity: &CompanionContentIdentity,
    ) -> std::result::Result<(), RetainedFailureCategory> {
        if let Some(active) = &self.glyph_resources {
            if active.identity == *identity
                && (active.backing_scale - self.backing_scale).abs() < f64::EPSILON
            {
                return Ok(());
            }
        }
        // The first compile is the activation build; any later one is a
        // legitimate, rare rebuild a resource generation change (e.g. a resize's
        // backing-scale change) caused — never per-frame animation churn. A
        // rebuild while a generation is already active is post-activation churn
        // the counters surface for the churn contract.
        let rebuilding = self.glyph_resources.is_some();
        let manifest =
            GlyphRepertoireManifest::for_active_pet(identity.clone(), self.backing_scale);
        let compile_started_at = Instant::now();
        let resources = CompiledRetainedResources::compile(&manifest)?;
        let compile_us = duration_us(compile_started_at.elapsed());
        self.metrics.record_compile_us(compile_us);
        if !self.activation_recorded {
            self.activation_excluded_appkit_us = self
                .activation_excluded_appkit_us
                .saturating_add(u64::from(compile_us));
        }
        let before = self.counters;
        let static_bytes = resources.atlas().rgba.len() as u64;
        let (texture, bind_group) = upload_glyph_atlas(
            &self.device,
            &self.queue,
            &self.atlas_layout,
            resources.atlas(),
            &mut self.counters,
        );
        let delta = self.counters - before;
        self.metrics
            .record_persistent_gpu_create(resource_object_count(delta));
        self.metrics.record_static_upload(static_bytes);
        self.metrics
            .replace_gpu_allocation(GpuAllocationKind::Atlas, static_bytes);
        if rebuilding {
            self.counters.atlas_builds_after_activation += 1;
            self.counters.atlas_uploads_after_activation += 1;
        }
        self.glyph_resources = Some(ActiveGlyphResources {
            identity: identity.clone(),
            backing_scale: self.backing_scale,
            resources,
            _texture: texture,
            bind_group,
        });
        Ok(())
    }

    /// Ensures the persistent capture intermediate and staging buffer match the
    /// current physical size and surface format, replacing them once on a change.
    /// Ordinary same-size captures reuse them.
    fn ensure_capture_resources(&mut self, width: u32, height: u32) {
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
            .replace_gpu_allocation(GpuAllocationKind::Capture, capture_bytes);
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

fn resource_object_count(counters: RetainedResourceCounters) -> u64 {
    u64::from(counters.buffer_creations)
        .saturating_add(u64::from(counters.texture_creations))
        .saturating_add(u64::from(counters.sampler_creations))
        .saturating_add(u64::from(counters.bind_group_creations))
        .saturating_add(u64::from(counters.pipeline_creations))
}

/// Terminates a frame that could not present because a render resource failed.
fn fail(progress: &mut FrameProgress, category: RetainedFailureCategory) {
    progress
        .finish(FrameDisposition::Failed(category))
        .expect("a frame reaches its terminal disposition exactly once");
}

/// Terminates a frame the surface asked us to drop this tick without failing.
fn skip(progress: &mut FrameProgress, reason: SkipReason) {
    progress
        .finish(FrameDisposition::Skipped(reason))
        .expect("a frame reaches its terminal disposition exactly once");
}

fn physical_dimension(logical: f64, scale: f64) -> u32 {
    (logical * scale).round().clamp(1.0, f64::from(u32::MAX)) as u32
}

/// Builds the fragment-stage bind-group layout for the glyph atlas texture and
/// sampler. Shared by the surface-bearing host and the headless resource harness.
fn create_atlas_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("glorp-retained-atlas-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Uploads a compiled glyph atlas into a GPU texture and builds its bind group,
/// counting the texture, sampler, bind group, and the one static upload. Shared
/// by generation activation on the surface-bearing host and the headless resource
/// harness so both count creations identically.
fn upload_glyph_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    atlas_layout: &wgpu::BindGroupLayout,
    atlas: &CompiledGlyphAtlas,
    counters: &mut RetainedResourceCounters,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glorp-retained-glyph-atlas"),
        size: wgpu::Extent3d {
            width: atlas.width,
            height: atlas.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // A linear (non-sRGB) format so `textureSample` returns the raw stored
        // premultiplied-sRGB atlas bytes without an sRGB→linear decode. Mask
        // glyphs read only alpha (unaffected by format); native-color emoji pass
        // their premultiplied-sRGB pixels straight through, keeping the gamma
        // convention every other primitive obeys.
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    counters.texture_creations += 1;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &atlas.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(atlas.width * 4),
            rows_per_image: Some(atlas.height),
        },
        wgpu::Extent3d {
            width: atlas.width,
            height: atlas.height,
            depth_or_array_layers: 1,
        },
    );
    counters.static_uploads += 1;
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("glorp-retained-atlas-sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    counters.sampler_creations += 1;
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("glorp-retained-atlas-bind-group"),
        layout: atlas_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    counters.bind_group_creations += 1;
    (texture, bind_group)
}

fn create_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    atlas_layout: &wgpu::BindGroupLayout,
    counters: &mut RetainedResourceCounters,
) -> Pipelines {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("glorp-retained-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("retained.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glorp-retained-pipeline-layout"),
        bind_group_layouts: &[Some(atlas_layout)],
        immediate_size: 0,
    });
    let mut create = |label: &'static str, blend: Option<wgpu::BlendState>| {
        counters.pipeline_creations += 1;
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(GpuPrimitive::LAYOUT)],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    // Every pipeline's blend equation comes from the premultiplied gamma-space
    // BlendContract, so the color convention lives in exactly one place.
    let blend = |mode: SmoothBlendMode| {
        parity::BlendContract::for_mode(mode)
            .expect("BlendContract covers every SmoothBlendMode")
            .blend_state()
    };
    Pipelines {
        normal: create("glorp-retained-normal", blend(SmoothBlendMode::Normal)),
        multiply: create("glorp-retained-multiply", blend(SmoothBlendMode::Multiply)),
        screen: create("glorp-retained-screen", blend(SmoothBlendMode::Screen)),
        add: create("glorp-retained-add", blend(SmoothBlendMode::Add)),
        replace: create("glorp-retained-replace", blend(SmoothBlendMode::Replace)),
    }
}

fn prepare_gpu_frame(
    plan: &SmoothCompanionScenePlan,
    draw_order: &[usize],
    metrics: CompanionGridMetrics,
    aperture: RoundAperture,
    background: [f32; 4],
    chrome: &RetainedChrome<'_>,
    atlas: &CompiledGlyphAtlas,
) -> std::result::Result<PreparedGpuFrame, RetainedFailureCategory> {
    let mut primitives = Vec::new();
    let mut blends = Vec::new();
    let viewport_aperture = viewport_aperture(aperture);
    let aperture_radius = aperture_radius(aperture);
    push_tank_background(
        &mut primitives,
        &mut blends,
        aperture,
        background,
        viewport_aperture,
        aperture_radius,
    );
    push_mood_aura(
        &mut primitives,
        &mut blends,
        metrics,
        aperture,
        chrome,
        viewport_aperture,
        aperture_radius,
    );
    for &index in draw_order {
        let Some(layer) = plan.layers.get(index) else {
            continue;
        };
        if layer.opacity <= 0.0 {
            continue;
        }
        let blend_code = match layer.blend {
            SmoothBlendMode::Multiply => 1.0,
            SmoothBlendMode::Screen => 2.0,
            _ => 0.0,
        };
        for item in &layer.items {
            match item {
                SmoothLayerItem::LocalCell(cell) => {
                    let rect = cell_rect(layer, cell.col, cell.row, metrics);
                    let (clip_rect, clip_ellipse, clip_kind) = clip_params(layer.clip, metrics);
                    if let Some(bg) = cell.bg {
                        primitives.push(GpuPrimitive {
                            rect,
                            color_a: rgb(bg.r, bg.g, bg.b, layer.opacity),
                            color_b: [0.0; 4],
                            uv: [0.0; 4],
                            params: [1.0, clip_kind, blend_code, 0.0],
                            clip_rect,
                            clip_ellipse,
                            viewport_aperture,
                            aperture_radius,
                        });
                        blends.push(layer.blend);
                    }
                    if let Some(glyph) = cell.glyph.as_ref() {
                        let entry = atlas
                            .entries
                            .get(&GlyphKey::new(glyph.clone(), cell.bold))
                            .copied()
                            .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
                        let fg = cell.fg.map_or_else(
                            || parity::premultiply_gamma_srgb([1.0, 1.0, 1.0, layer.opacity]),
                            |fg| rgb(fg.r, fg.g, fg.b, layer.opacity),
                        );
                        if let Some(glyph_rect) = glyph_ink_rect(
                            [rect[0], rect[1]],
                            metrics.font_size as f32 * layer.transform.scale.x,
                            entry,
                        ) {
                            let uv = entry
                                .visible_uv
                                .expect("a visible ink rect implies a visible uv");
                            primitives.push(GpuPrimitive {
                                rect: glyph_rect,
                                color_a: fg,
                                color_b: [0.0; 4],
                                uv,
                                params: [0.0, clip_kind, blend_code, glyph_mode_flag(entry)],
                                clip_rect,
                                clip_ellipse,
                                viewport_aperture,
                                aperture_radius,
                            });
                            blends.push(layer.blend);
                        }
                    }
                }
                SmoothLayerItem::Shape(shape) => {
                    let SmoothShapeGeometry::Ellipse { bounds } = shape.geometry;
                    let rect = shape_rect(layer, bounds.min, bounds.max, metrics);
                    let (color_a, color_b, kind) = match shape.fill {
                        SmoothFill::Solid(color) => (rgba(color, layer.opacity), [0.0; 4], 2.0),
                        SmoothFill::RadialGradient { inner, outer } => (
                            rgba(inner, layer.opacity),
                            rgba(outer, layer.opacity),
                            RADIAL_GRADIENT_KIND,
                        ),
                        SmoothFill::LinearGradientY { top, bottom } => {
                            (rgba(top, layer.opacity), rgba(bottom, layer.opacity), 4.0)
                        }
                    };
                    let (clip_rect, clip_ellipse, clip_kind) = clip_params(layer.clip, metrics);
                    primitives.push(GpuPrimitive {
                        rect,
                        color_a,
                        color_b,
                        uv: [0.0; 4],
                        params: [kind, clip_kind, blend_code, 0.0],
                        clip_rect,
                        clip_ellipse,
                        viewport_aperture,
                        aperture_radius,
                    });
                    blends.push(layer.blend);
                }
                SmoothLayerItem::Raster(_) => {
                    return Err(RetainedFailureCategory::UnsupportedRaster)
                }
            }
        }
    }
    push_gauges(
        &mut primitives,
        &mut blends,
        aperture,
        chrome.gauges,
        viewport_aperture,
        aperture_radius,
    );
    push_overlays(
        &mut primitives,
        &mut blends,
        chrome.overlays,
        viewport_aperture,
        aperture_radius,
    );
    push_hud(
        &mut primitives,
        &mut blends,
        atlas,
        aperture,
        chrome.hud,
        chrome.hud_font_size,
        viewport_aperture,
        aperture_radius,
    )?;
    if chrome.dim_overlay {
        push_solid_primitive(
            &mut primitives,
            &mut blends,
            [0.0, 0.0, aperture.width as f32, aperture.height as f32],
            display_rgba([0.05, 0.06, 0.10, 0.35]),
            1.0,
            viewport_aperture,
            aperture_radius,
        );
    }
    Ok(PreparedGpuFrame { primitives, blends })
}

fn viewport_aperture(aperture: RoundAperture) -> [f32; 4] {
    [
        aperture.width as f32,
        aperture.height as f32,
        aperture.center_x,
        aperture.center_y,
    ]
}

fn aperture_radius(aperture: RoundAperture) -> [f32; 4] {
    [aperture.radius, 0.0, 0.0, 0.0]
}

fn push_tank_background(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    aperture: RoundAperture,
    background: [f32; 4],
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    // The opaque depth core is the shared, backend-neutral tank tint (see
    // `round::hud::tank_core_color`); Smooth derives its bitmap core the same way.
    let core = tank_core_color(RoundColor(
        background[0],
        background[1],
        background[2],
        background[3],
    ));
    primitives.push(GpuPrimitive {
        rect: [
            aperture.center_x - aperture.radius,
            aperture.center_y - aperture.radius,
            aperture.radius * 2.0,
            aperture.radius * 2.0,
        ],
        color_a: display_round_color(core),
        color_b: display_rgba(background),
        uv: [0.0; 4],
        // Kind 3 reproduces Smooth's output-level radial dither in the shader.
        params: [3.0, 0.0, 0.0, 0.0],
        clip_rect: [0.0; 4],
        clip_ellipse: [0.0; 4],
        viewport_aperture,
        aperture_radius,
    });
    blends.push(SmoothBlendMode::Replace);
}

fn push_mood_aura(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    metrics: CompanionGridMetrics,
    aperture: RoundAperture,
    chrome: &RetainedChrome<'_>,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let center_x = metrics.origin_x + chrome.pet_center_col * metrics.cell_w;
    let center_y = metrics.origin_y - (chrome.pet_center_row + 1.0) * metrics.cell_h;
    let max_radius = chrome.pet_width_cells * metrics.cell_w * 0.95;
    for index in 0..8 {
        let t = index as f64 / 8.0;
        let radius = max_radius * (1.0 - t);
        push_solid_primitive(
            primitives,
            blends,
            [
                (center_x - radius) as f32,
                (center_y - radius) as f32,
                (radius * 2.0) as f32,
                (radius * 2.0) as f32,
            ],
            display_rgba([
                chrome.mood_aura[0],
                chrome.mood_aura[1],
                chrome.mood_aura[2],
                0.05,
            ]),
            2.0,
            viewport_aperture,
            aperture_radius,
        );
    }
    let _ = aperture;
}

fn push_gauges(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    aperture: RoundAperture,
    gauges: PreparedGaugeFrame,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let layout = perimeter_gauge_layout(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        f64::from(aperture.radius),
        COMPANION_GAUGE_GAP_DEG,
    );
    let colors = perimeter_gauge_colors();
    // The whole gauge geometry (which arcs, their angles, colours, order) comes
    // from the shared `prepared_perimeter_gauge_arcs` — the same list the AppKit
    // painter strokes — so neither backend re-derives the gauge math.
    let arcs = prepared_perimeter_gauge_arcs(
        &layout,
        &colors,
        GaugeFractions {
            xp: gauges.xp_fraction,
            daily: gauges.daily_fraction,
            daily_overage: gauges.daily_overage_fraction,
            pace: gauges.pace_fraction,
        },
    );
    for arc in &arcs {
        let lane = GaugeLane {
            ring: arc.ring,
            stroke_width: arc.stroke_width,
            cap: arc.cap,
        };
        push_analytic_arc(
            primitives,
            blends,
            &lane,
            arc.color,
            arc.start_deg,
            arc.end_deg,
            viewport_aperture,
            aperture_radius,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_analytic_arc(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    lane: &GaugeLane,
    color: RoundColor,
    start_deg: f64,
    end_deg: f64,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let stroke_outer = lane.ring.radius + lane.stroke_width / 2.0;
    if stroke_outer <= 0.0 || end_deg <= start_deg {
        return;
    }
    // The rect extends `ARC_AA_MARGIN` past the stroke so the analytic outer edge
    // antialiases inside the primitive; centerline and half-width are normalized by
    // this padded radius, so the stroke stays at the same physical position.
    let outer_radius = (stroke_outer + ARC_AA_MARGIN) as f32;
    primitives.push(GpuPrimitive {
        rect: [
            lane.ring.cx as f32 - outer_radius,
            lane.ring.cy as f32 - outer_radius,
            outer_radius * 2.0,
            outer_radius * 2.0,
        ],
        color_a: display_round_color(color),
        color_b: [0.0; 4],
        // Analytic arc parameters: start/sweep radians, centerline and half-width
        // normalized by the primitive's outer radius.
        uv: [
            start_deg.to_radians() as f32,
            (end_deg - start_deg).to_radians() as f32,
            lane.ring.radius as f32 / outer_radius,
            lane.stroke_width as f32 / (2.0 * outer_radius),
        ],
        params: [
            5.0,
            0.0,
            0.0,
            if lane.cap == LineCap::Round { 1.0 } else { 0.0 },
        ],
        clip_rect: [0.0; 4],
        clip_ellipse: [0.0; 4],
        viewport_aperture,
        aperture_radius,
    });
    blends.push(SmoothBlendMode::Normal);
}

fn push_overlays(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    overlays: &[RoundDrawCommand],
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    for command in overlays
        .iter()
        .filter(|command| matches!(command.kind, RoundDrawKind::Halo | RoundDrawKind::Trouble))
    {
        push_solid_primitive(
            primitives,
            blends,
            [
                command.x - command.radius,
                command.y - command.radius,
                command.radius * 2.0,
                command.radius * 2.0,
            ],
            display_round_color(command.color),
            2.0,
            viewport_aperture,
            aperture_radius,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_hud(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    atlas: &CompiledGlyphAtlas,
    aperture: RoundAperture,
    hud: &CompanionHudText,
    hud_font_size: f64,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) -> std::result::Result<(), RetainedFailureCategory> {
    let gauge_layout = perimeter_gauge_layout(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        f64::from(aperture.radius),
        COMPANION_GAUGE_GAP_DEG,
    );
    let gap = stat_gap_box(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        gauge_layout.pace.ring.radius - gauge_layout.pace.stroke_width / 2.0,
        COMPANION_GAUGE_GAP_DEG,
    );
    let big_color = display_rgba([0.93, 0.93, 0.97, 1.0]);
    let sub_color = display_rgba([0.62, 0.63, 0.77, 1.0]);
    // The shrink policy and stacking come from the shared `prepare_hud_layout`; the
    // only backend-specific input is how the glyph atlas measures each run.
    let layout = prepare_hud_layout(
        gap,
        f64::from(aperture.radius),
        f64::from(aperture.height),
        hud_font_size,
        |sizes| {
            let texts = [&hud.today_total, &hud.daily_percent, &hud.pace];
            let mut metrics = [HudLineMetrics { width: 0.0, height: 0.0 }; 3];
            for (index, metric) in metrics.iter_mut().enumerate() {
                let font_size = sizes[index] as f32;
                *metric = HudLineMetrics {
                    width: f64::from(glyph_run_width(atlas, texts[index], font_size)),
                    height: f64::from(glyph_run_height(atlas, texts[index], font_size)),
                };
            }
            metrics
        },
    );
    let lines = [
        (&hud.today_total, layout.lines[0], big_color),
        (&hud.daily_percent, layout.lines[1], sub_color),
        (&hud.pace, layout.lines[2], sub_color),
    ];
    for (text, line, color) in lines {
        debug_assert!(text.is_ascii(), "HUD text is ASCII by contract: {text:?}");
        let font_size = line.font_size as f32;
        let y = line.baseline_y;
        let mut x = line.origin_x;
        for scalar in text.chars() {
            let entry = atlas
                .entries
                .get(&GlyphKey::new(scalar.to_string(), false))
                .copied()
                .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
            if let Some(rect) = glyph_ink_rect([x as f32, y as f32], font_size, entry) {
                let uv = entry
                    .visible_uv
                    .expect("a visible ink rect implies a visible uv");
                primitives.push(GpuPrimitive {
                    rect,
                    color_a: color,
                    color_b: [0.0; 4],
                    uv,
                    params: [0.0, 0.0, 0.0, glyph_mode_flag(entry)],
                    clip_rect: [0.0; 4],
                    clip_ellipse: [0.0; 4],
                    viewport_aperture,
                    aperture_radius,
                });
                blends.push(SmoothBlendMode::Normal);
            }
            x += f64::from(glyph_advance(entry, font_size));
        }
    }
    Ok(())
}

/// The glyph-mode flag a glyph primitive carries in `params[3]`: 0.0 for a
/// coverage mask (tinted by the authored color), 1.0 for native color (the
/// shader samples the premultiplied RGBA and bypasses the tint).
fn glyph_mode_flag(entry: GlyphAtlasEntry) -> f32 {
    match entry.fragment_mode() {
        FragmentGlyphMode::Mask => 0.0,
        FragmentGlyphMode::NativeColor => 1.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_solid_primitive(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    rect: [f32; 4],
    color: [f32; 4],
    kind: f32,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    primitives.push(GpuPrimitive {
        rect,
        color_a: color,
        color_b: [0.0; 4],
        uv: [0.0; 4],
        params: [kind, 0.0, 0.0, 0.0],
        clip_rect: [0.0; 4],
        clip_ellipse: [0.0; 4],
        viewport_aperture,
        aperture_radius,
    });
    blends.push(SmoothBlendMode::Normal);
}

fn display_round_color(color: RoundColor) -> [f32; 4] {
    display_rgba([color.0, color.1, color.2, color.3])
}

/// Projects an authored straight-sRGB RGBA color into the premultiplied-gamma
/// RGBA every GPU primitive carries. This is the single color convention; see
/// [`parity::premultiply_gamma_srgb`].
fn display_rgba(color: [f32; 4]) -> [f32; 4] {
    parity::premultiply_gamma_srgb(color)
}

fn glyph_scale(font_size: f32) -> f32 {
    font_size / GLYPH_FONT_SIZE as f32
}

fn glyph_advance(entry: GlyphAtlasEntry, font_size: f32) -> f32 {
    entry.advance * glyph_scale(font_size)
}

fn glyph_run_width(atlas: &CompiledGlyphAtlas, text: &str, font_size: f32) -> f32 {
    debug_assert!(text.is_ascii(), "HUD text is ASCII by contract: {text:?}");
    text.chars()
        .filter_map(|scalar| {
            atlas
                .entries
                .get(&GlyphKey::new(scalar.to_string(), false))
                .copied()
        })
        .map(|entry| glyph_advance(entry, font_size))
        .sum()
}

fn glyph_run_height(atlas: &CompiledGlyphAtlas, text: &str, font_size: f32) -> f32 {
    debug_assert!(text.is_ascii(), "HUD text is ASCII by contract: {text:?}");
    text.chars()
        .filter_map(|scalar| {
            atlas
                .entries
                .get(&GlyphKey::new(scalar.to_string(), false))
                .copied()
        })
        .map(|entry| entry.line_height * glyph_scale(font_size))
        .fold(0.0, f32::max)
}

fn glyph_ink_rect(
    draw_origin: [f32; 2],
    font_size: f32,
    entry: GlyphAtlasEntry,
) -> Option<[f32; 4]> {
    if entry.ink_size[0] <= 0.0 || entry.ink_size[1] <= 0.0 {
        return None;
    }
    let scale = glyph_scale(font_size);
    // `draw_origin` is the run/cell box bottom, matching where Smooth's unflipped
    // `drawAtPoint` places a glyph's layout box. `ink_origin[1]`/`ink_size[1]` are
    // measured top-down from the cell top, while the GPU rect's y is the y-up
    // bottom edge, so the ink bottom's height above the box bottom is the top-down
    // span from the box bottom row (`raster_size[1] - safe_padding`) up to the ink
    // bottom (`ink_origin[1] + safe_padding + ink_size[1]`). Placing the ink by its
    // baseline this way — rather than by the raw top-down `ink_origin[1]` — keeps
    // decimals, unit letters, and descenders on the same baseline Smooth draws.
    let ink_bottom_above_box =
        entry.raster_size[1] - 2.0 * entry.safe_padding - entry.ink_origin[1] - entry.ink_size[1];
    Some([
        draw_origin[0] + entry.ink_origin[0] * scale,
        draw_origin[1] + ink_bottom_above_box * scale,
        entry.ink_size[0] * scale,
        entry.ink_size[1] * scale,
    ])
}

fn layer_point(layer: &SmoothCompanionLayer, local: SmoothPoint) -> SmoothPoint {
    let pivot = SmoothPoint {
        x: layer.anchor.x + layer.transform_origin.x,
        y: layer.anchor.y + layer.transform_origin.y,
    };
    SmoothPoint {
        x: pivot.x
            + (layer.anchor.x + local.x - pivot.x) * layer.transform.scale.x
            + layer.transform.translation.x,
        y: pivot.y
            + (layer.anchor.y + local.y - pivot.y) * layer.transform.scale.y
            + layer.transform.translation.y,
    }
}

fn cell_rect(
    layer: &SmoothCompanionLayer,
    col: u16,
    row: u16,
    metrics: CompanionGridMetrics,
) -> [f32; 4] {
    let p = layer_point(layer, SmoothPoint { x: f32::from(col), y: f32::from(row) });
    let scale = layer.transform.scale.x;
    let fractional = matches!(
        layer.motion_binding,
        SmoothLayerMotionBinding::PetAttached
            | SmoothLayerMotionBinding::FloorProjected
            | SmoothLayerMotionBinding::Parallax(_)
    );
    let x = metrics.origin_x as f32 + p.x * metrics.cell_w as f32;
    let y = if fractional {
        metrics.origin_y as f32 - (p.y + scale) * metrics.cell_h as f32
    } else {
        metrics.origin_y as f32 - (p.y.round() + 1.0) * metrics.cell_h as f32
    };
    [
        x,
        y,
        metrics.cell_w as f32 * scale,
        metrics.cell_h as f32 * scale,
    ]
}

fn shape_rect(
    layer: &SmoothCompanionLayer,
    min: SmoothPoint,
    max: SmoothPoint,
    metrics: CompanionGridMetrics,
) -> [f32; 4] {
    let min = layer_point(layer, min);
    let max = layer_point(layer, max);
    [
        metrics.origin_x as f32 + min.x * metrics.cell_w as f32,
        metrics.origin_y as f32 - max.y * metrics.cell_h as f32,
        (max.x - min.x) * metrics.cell_w as f32,
        (max.y - min.y) * metrics.cell_h as f32,
    ]
}

fn clip_params(clip: SmoothClip, metrics: CompanionGridMetrics) -> ([f32; 4], [f32; 4], f32) {
    match clip {
        SmoothClip::None => ([0.0; 4], [0.0; 4], 0.0),
        SmoothClip::Rect(bounds) => (
            [
                metrics.origin_x as f32 + bounds.min.x * metrics.cell_w as f32,
                metrics.origin_y as f32 - bounds.max.y * metrics.cell_h as f32,
                (bounds.max.x - bounds.min.x) * metrics.cell_w as f32,
                (bounds.max.y - bounds.min.y) * metrics.cell_h as f32,
            ],
            [0.0; 4],
            1.0,
        ),
        SmoothClip::Circle { center, radius } => (
            [0.0; 4],
            [
                metrics.origin_x as f32 + center.x * metrics.cell_w as f32,
                metrics.origin_y as f32 - center.y * metrics.cell_h as f32,
                radius * metrics.cell_w as f32,
                radius * metrics.cell_h as f32,
            ],
            2.0,
        ),
        SmoothClip::Ellipse { center, radii } => (
            [0.0; 4],
            [
                metrics.origin_x as f32 + center.x * metrics.cell_w as f32,
                metrics.origin_y as f32 - center.y * metrics.cell_h as f32,
                radii.x * metrics.cell_w as f32,
                radii.y * metrics.cell_h as f32,
            ],
            2.0,
        ),
    }
}

fn rgba(color: SmoothRgba8, opacity: f32) -> [f32; 4] {
    parity::premultiply_gamma_srgb([
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0 * opacity,
    ])
}

fn rgb(r: u8, g: u8, b: u8, opacity: f32) -> [f32; 4] {
    parity::premultiply_gamma_srgb([
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        opacity,
    ])
}

#[cfg(test)]
mod tests {
    use super::parity::srgb_channel_to_linear;
    use super::resources::GlyphEntryKind;
    use super::{
        create_atlas_bind_group_layout, create_pipelines, current_process_rss_bytes, glyph_advance,
        glyph_ink_rect, glyph_run_height, glyph_run_width, persistent_instance_capacity,
        physical_dimension, push_analytic_arc, upload_glyph_atlas, CompiledGlyphAtlas,
        CompiledRetainedResources, GlyphAtlasEntry, GlyphKey, GlyphRepertoireManifest,
        GpuPrimitive, LayerActivationGuard, LayerActivationState, PersistentFrameBuffers,
        Pipelines, PreparedGpuFrame, RetainedFailureCategory, RetainedResourceCounters,
        SmoothBlendMode, FULL_FIXTURE_INSTANCE_HEADROOM, FULL_FIXTURE_INSTANCE_MAX,
        GLYPH_FONT_SIZE, RETAINED_ATLAS_POINT_SIZE,
    };
    use crate::pet::generation::Species;
    use crate::round::smooth::CompanionContentIdentity;
    use std::collections::BTreeMap;

    /// The surface format the headless resource harness builds its pipelines and
    /// capture intermediate against — the linear (non-sRGB) `Bgra8Unorm` the
    /// production surface now composites into for gamma-space blending. No surface
    /// is created, so this only selects the pipeline color-target format.
    const TEST_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

    /// The fixed instance count of a synthetic ambient frame. Ambient idle motion
    /// wobbles a fixed set of primitives, so the count never changes and the
    /// persistent ring is written but never grown after warmup.
    const AMBIENT_PRIMITIVE_COUNT: usize = 96;

    /// A deterministic ambient animation strip as prepared GPU frames. Each frame
    /// carries the same primitive count with per-frame position wobble — the
    /// resource-lifecycle shape of ordinary idle motion, isolated from scene
    /// content so the counters measure only buffer reuse.
    fn deterministic_ambient_frames(count: usize) -> Vec<PreparedGpuFrame> {
        (0..count)
            .map(|frame_index| {
                let phase = frame_index as f32 * 0.05;
                let primitives = (0..AMBIENT_PRIMITIVE_COUNT)
                    .map(|i| {
                        let base = i as f32;
                        let wobble = (phase + base * 0.1).sin() * 2.0;
                        GpuPrimitive {
                            rect: [40.0 + base + wobble, 40.0 + base - wobble, 8.0, 8.0],
                            color_a: [0.20, 0.30, 0.45, 1.0],
                            color_b: [0.0; 4],
                            uv: [0.0; 4],
                            params: [2.0, 0.0, 0.0, 0.0],
                            clip_rect: [0.0; 4],
                            clip_ellipse: [0.0; 4],
                            viewport_aperture: [360.0, 360.0, 180.0, 180.0],
                            aperture_radius: [180.0, 0.0, 0.0, 0.0],
                        }
                    })
                    .collect();
                let blends = vec![SmoothBlendMode::Normal; AMBIENT_PRIMITIVE_COUNT];
                PreparedGpuFrame { primitives, blends }
            })
            .collect()
    }

    fn prepared_frame_with_count(count: usize) -> PreparedGpuFrame {
        let primitive = GpuPrimitive {
            rect: [0.0; 4],
            color_a: [0.0; 4],
            color_b: [0.0; 4],
            uv: [0.0; 4],
            params: [0.0; 4],
            clip_rect: [0.0; 4],
            clip_ellipse: [0.0; 4],
            viewport_aperture: [360.0, 360.0, 180.0, 180.0],
            aperture_radius: [180.0, 0.0, 0.0, 0.0],
        };
        PreparedGpuFrame {
            primitives: vec![primitive; count],
            blends: vec![SmoothBlendMode::Normal; count],
        }
    }

    /// A warmed, surfaceless mirror of the retained host's GPU resource surface:
    /// a headless Metal device, the production pipelines, an uploaded glyph atlas,
    /// and the persistent instance ring primed to the ambient high-water mark. It
    /// drives the same production `PersistentFrameBuffers` and
    /// [`RetainedResourceCounters`] the surface-bearing host does, so it proves the
    /// steady-state resource contract without a window.
    struct TestRetainedResources {
        device: wgpu::Device,
        queue: wgpu::Queue,
        frame_buffers: PersistentFrameBuffers,
        counters: RetainedResourceCounters,
        // Held to mirror a warm host's live GPU resources during steady state.
        _pipelines: Pipelines,
        _atlas_texture: wgpu::Texture,
        _atlas_bind_group: wgpu::BindGroup,
    }

    impl TestRetainedResources {
        /// Builds a surfaceless device, the production pipelines and glyph atlas,
        /// and primes the instance ring to the ambient high-water mark. Every GPU
        /// object created here lands in the warmup baseline.
        fn warm() -> Self {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::METAL;
            let instance = wgpu::Instance::new(descriptor);
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    force_fallback_adapter: false,
                    compatible_surface: None,
                    ..Default::default()
                }))
                .expect("a surfaceless Metal adapter is available on this machine");
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("glorp-retained-test-device"),
                    ..Default::default()
                }))
                .expect("a surfaceless Metal device is available on this machine");

            let mut counters = RetainedResourceCounters::default();
            let atlas_layout = create_atlas_bind_group_layout(&device);
            let pipelines =
                create_pipelines(&device, TEST_SURFACE_FORMAT, &atlas_layout, &mut counters);

            // Compile and upload a real declared repertoire exactly as generation
            // activation does, so the static upload and atlas objects land in the
            // warmup baseline.
            let manifest = GlyphRepertoireManifest::for_active_pet(
                CompanionContentIdentity::for_pet(Species::Crystal),
                2.0,
            );
            let resources = CompiledRetainedResources::compile(&manifest)
                .expect("the declared repertoire fits the preflighted atlas");
            let (atlas_texture, atlas_bind_group) = upload_glyph_atlas(
                &device,
                &queue,
                &atlas_layout,
                resources.atlas(),
                &mut counters,
            );

            // Prime the instance ring to the ambient high-water mark so steady-state
            // frames reuse it without growing.
            let mut frame_buffers = PersistentFrameBuffers::new();
            let high_water = deterministic_ambient_frames(1)
                .iter()
                .map(|frame| frame.primitives.len())
                .max()
                .unwrap_or(0);
            frame_buffers.ensure_instance_capacity(high_water, &device, &mut counters);

            Self {
                device,
                queue,
                frame_buffers,
                counters,
                _pipelines: pipelines,
                _atlas_texture: atlas_texture,
                _atlas_bind_group: atlas_bind_group,
            }
        }

        fn counters(&self) -> RetainedResourceCounters {
            self.counters
        }

        /// Stages a prepared frame's instances into the persistent ring, exactly
        /// as the surface-bearing host's `prepare_frame` does.
        fn prepare_frame(
            &mut self,
            frame: &PreparedGpuFrame,
        ) -> std::result::Result<(), RetainedFailureCategory> {
            self.frame_buffers.ensure_instance_capacity(
                frame.primitives.len(),
                &self.device,
                &mut self.counters,
            );
            self.frame_buffers.write_frame_instances(
                &self.queue,
                &frame.primitives,
                &mut self.counters,
            );
            Ok(())
        }
    }

    #[test]
    fn ambient_strip_allocates_no_gpu_objects_after_warmup() {
        let mut host = TestRetainedResources::warm();
        let before = host.counters();
        for frame in deterministic_ambient_frames(300) {
            host.prepare_frame(&frame).unwrap();
        }
        let delta = host.counters() - before;
        assert_eq!(delta.buffer_creations, 0);
        assert_eq!(delta.texture_creations, 0);
        assert_eq!(delta.sampler_creations, 0);
        assert_eq!(delta.bind_group_creations, 0);
        assert_eq!(delta.pipeline_creations, 0);
        assert_eq!(delta.static_uploads, 0);
        assert!(delta.instance_writes > 0);
    }

    #[test]
    fn first_instance_ring_allocation_has_bounded_fixture_headroom() {
        assert_eq!(FULL_FIXTURE_INSTANCE_MAX, 782);
        assert_eq!(FULL_FIXTURE_INSTANCE_HEADROOM, 242);
        assert_eq!(persistent_instance_capacity(1), 1_024);
        assert_eq!(persistent_instance_capacity(782), 1_024);
    }

    #[test]
    fn varying_full_fixture_counts_never_recreate_the_instance_ring() {
        let mut host = TestRetainedResources::warm();
        let before = host.counters();
        for count in [1, 96, 782, 12, 781, 0, 512, 782] {
            host.prepare_frame(&prepared_frame_with_count(count))
                .unwrap();
        }
        let delta = host.counters() - before;
        assert_eq!(delta.buffer_creations, 0);
    }

    #[test]
    fn process_rss_sampler_reads_current_process_without_spawning() {
        assert!(current_process_rss_bytes().is_some_and(|bytes| bytes > 0));
    }

    /// A visible coverage-mask entry with the given ink geometry; the metric
    /// fields the retained layout does not read stay neutral.
    fn mask_entry(
        visible_uv: Option<[f32; 4]>,
        ink_origin: [f32; 2],
        ink_size: [f32; 2],
        advance: f32,
        line_height: f32,
    ) -> GlyphAtlasEntry {
        GlyphAtlasEntry {
            visible_uv,
            ink_origin,
            ink_size,
            baseline: 0.0,
            ascent: 0.0,
            descent: 0.0,
            line_height,
            advance,
            raster_size: [80.0, 80.0],
            safe_padding: 6.0,
            font_policy_id: 0,
            kind: GlyphEntryKind::Mask,
        }
    }

    #[test]
    fn failed_preflight_never_marks_layer_attached() {
        let mut state = LayerActivationState::default();
        state.preflight_failed();
        assert!(!state.attached());
        assert!(state.appkit_restored());
    }

    #[test]
    fn activation_guard_restores_uncommitted_attachment() {
        let state = std::rc::Rc::new(std::cell::Cell::new(true));
        {
            let _guard = LayerActivationGuard::for_test(state.clone());
        }
        assert!(!state.get());
    }

    #[test]
    fn physical_dimensions_follow_backing_scale() {
        assert_eq!(physical_dimension(360.0, 2.0), 720);
        assert_eq!(physical_dimension(0.0, 2.0), 1);
    }

    #[test]
    fn srgb_transfer_curve_matches_the_standard_eotf() {
        // The gamma render convention no longer linearizes at upload; this pins the
        // sRGB transfer curve the parity oracle's linear-space prediction relies on.
        assert_eq!(srgb_channel_to_linear(0.0), 0.0);
        assert_eq!(srgb_channel_to_linear(1.0), 1.0);
        assert!((srgb_channel_to_linear(0.5) - 0.214_041_14).abs() < 0.000_001);
    }

    #[test]
    fn glyph_quad_uses_ink_bounds_instead_of_filling_the_cell() {
        let entry = mask_entry(
            Some([0.1, 0.2, 0.3, 0.4]),
            [3.0, 7.0],
            [20.0, 31.0],
            29.0,
            52.0,
        );

        // The quad matches the ink bounds, and its y is the baseline-relative
        // bottom edge: `(raster_size - 2*padding - ink_origin_y - ink_size_y)`
        // above the box-bottom `draw_origin`, i.e. (80 - 12 - 7 - 31) * 0.5 = 15.
        assert_eq!(
            glyph_ink_rect([100.0, 200.0], 24.0, entry),
            Some([101.5, 215.0, 10.0, 15.5])
        );
        assert_eq!(glyph_advance(entry, 24.0), 14.5);
    }

    #[test]
    fn blank_glyph_has_advance_without_a_visible_quad() {
        let entry = GlyphAtlasEntry::whitespace(28.0, 52.0);

        assert_eq!(glyph_ink_rect([12.0, 34.0], 48.0, entry), None);
        assert_eq!(glyph_advance(entry, 48.0), 28.0);
    }

    /// Rasterizes one HUD glyph through the production atlas rasterizer at the
    /// fixed device point size, so its `GlyphAtlasEntry` carries the real
    /// attributed-measured ink geometry the renderer reads back.
    fn rasterize_hud_glyph(text: &str) -> GlyphAtlasEntry {
        let cell = 80u32;
        let padding = 6u32;
        let key = GlyphKey::new(text, false);
        let target = super::resources::GlyphRasterTarget {
            cell,
            padding,
            point_size: RETAINED_ATLAS_POINT_SIZE,
        };
        let mut atlas = vec![0u8; (cell * cell * 4) as usize];
        super::resources::rasterize_glyph_entry(&key, &target, 0, &mut atlas, cell, cell, 0, 0)
            .expect("offscreen HUD glyph rasterization succeeds")
    }

    #[test]
    fn hud_decimal_point_rests_on_the_digit_baseline_like_smooth() {
        // Smooth paints each HUD line as one attributed run, so the decimal point
        // sits on the text baseline exactly like the digits around it. The retained
        // per-glyph placement must land the '.' ink on that same baseline, not
        // raised to a middle-dot (·) height.
        let draw_origin = [100.0_f32, 200.0_f32];
        let font_size = RETAINED_ATLAS_POINT_SIZE as f32; // atlas point size -> scale 1
        let scale = font_size / GLYPH_FONT_SIZE as f32;

        let digit = rasterize_hud_glyph("6");
        let dot = rasterize_hud_glyph(".");
        let digit_bottom = glyph_ink_rect(draw_origin, font_size, digit).expect("visible digit")[1];
        let dot_bottom = glyph_ink_rect(draw_origin, font_size, dot).expect("visible dot")[1];

        // Typographic baseline (the Smooth oracle): the run box bottom sits at
        // `draw_origin`, so a baseline-resting glyph's ink bottom lands `descent`
        // above it in the y-up render.
        let baseline = draw_origin[1] + digit.descent * scale;
        assert!(
            (digit_bottom - baseline).abs() <= 1.5,
            "digit ink rests on the baseline: bottom {digit_bottom} vs baseline {baseline}",
        );
        assert!(
            (dot_bottom - digit_bottom).abs() <= 1.5,
            "the decimal point shares the digit baseline (a period, not a raised \
             middle dot): dot bottom {dot_bottom} vs digit bottom {digit_bottom}",
        );
    }

    #[test]
    fn hud_unit_glyphs_share_the_digit_baseline_and_are_not_superscripted() {
        // The compact HUD's unit/label glyphs (k, M, m, d, a, ...) are drawn at the
        // same size and baseline as the digits in Smooth's single attributed run.
        // The retained per-glyph placement must rest them on the same baseline, not
        // shift them up into a smaller/raised superscript-looking position.
        let draw_origin = [100.0_f32, 200.0_f32];
        let font_size = RETAINED_ATLAS_POINT_SIZE as f32;

        let digit_bottom =
            glyph_ink_rect(draw_origin, font_size, rasterize_hud_glyph("6")).expect("digit")[1];
        for unit in ["k", "M", "m", "d", "a"] {
            let bottom = glyph_ink_rect(draw_origin, font_size, rasterize_hud_glyph(unit))
                .expect("visible unit glyph")[1];
            assert!(
                (bottom - digit_bottom).abs() <= 2.0,
                "unit {unit:?} rests on the digit baseline (not raised): bottom {bottom} \
                 vs digit bottom {digit_bottom}",
            );
        }

        // A descender still drops below the shared baseline, proving glyphs align by
        // baseline rather than being flattened to a common bottom edge.
        let descender_bottom =
            glyph_ink_rect(draw_origin, font_size, rasterize_hud_glyph("y")).expect("descender")[1];
        assert!(
            descender_bottom < digit_bottom - 2.0,
            "a descender drops below the baseline: bottom {descender_bottom} vs digit \
             bottom {digit_bottom}",
        );
    }

    #[test]
    fn atlas_generation_keys_on_declared_content_not_the_per_frame_glyph_set() {
        use crate::pet::generation::Species;
        let crystal = || {
            GlyphRepertoireManifest::for_active_pet(
                CompanionContentIdentity::for_pet(Species::Crystal),
                2.0,
            )
            .generation_key()
        };
        // Same declared content -> same generation (the per-minute room reshuffle
        // changes which glyph is painted, never the identity, so no rebuild).
        assert_eq!(crystal(), crystal());
        // A different species is a real generation change.
        assert_ne!(
            crystal(),
            GlyphRepertoireManifest::for_active_pet(
                CompanionContentIdentity::for_pet(Species::Mech),
                2.0,
            )
            .generation_key(),
        );
        // A backing-scale change is a real generation change (Task-8 finding #3).
        assert_ne!(
            crystal(),
            GlyphRepertoireManifest::for_active_pet(
                CompanionContentIdentity::for_pet(Species::Crystal),
                1.0,
            )
            .generation_key(),
        );
    }

    #[test]
    fn retained_hud_measures_runs_at_the_shared_big_and_subline_font_policy() {
        let entry = mask_entry(Some([0.0; 4]), [0.0; 2], [10.0, 20.0], 30.0, 50.0);
        let atlas = CompiledGlyphAtlas {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
            entries: [GlyphKey::new("A", false), GlyphKey::new("B", false)]
                .into_iter()
                .map(|key| (key, entry))
                .collect::<BTreeMap<_, _>>(),
        };
        let hud = crate::round::hud::CompanionHudText {
            today_total: "AA".into(),
            daily_percent: "A".into(),
            pace: "BB".into(),
        };

        // The retained HUD scales its three lines with the shared font policy, so a
        // stack of 20 gives the big line 20*1.08 and the sub-lines 20*0.68.
        let sizes = crate::round::hud::hud_line_font_sizes(20.0);
        assert!((sizes[0] - 21.6).abs() < 0.001);
        assert!((sizes[1] - 13.6).abs() < 0.001);
        assert_eq!(sizes[1], sizes[2]);

        // Retained measures each run against the glyph atlas at those shared sizes;
        // the taller big line and the widest-run policy fall out of the measurement.
        let big_height = glyph_run_height(&atlas, &hud.today_total, sizes[0] as f32);
        let sub_height = glyph_run_height(&atlas, &hud.daily_percent, sizes[1] as f32);
        assert!(big_height > sub_height);
        let big_width = glyph_run_width(&atlas, &hud.today_total, sizes[0] as f32);
        let single_width = glyph_run_width(&atlas, &hud.daily_percent, sizes[1] as f32);
        assert!(big_width > single_width);
    }

    #[test]
    fn gauge_arc_is_one_analytic_primitive_with_exact_shared_angles_and_round_cap() {
        let lane = crate::round::hud::perimeter_gauge_layout(
            180.0,
            180.0,
            180.0,
            crate::round::hud::COMPANION_GAUGE_GAP_DEG,
        )
        .xp;
        let mut primitives = Vec::new();
        let mut blends = Vec::new();
        push_analytic_arc(
            &mut primitives,
            &mut blends,
            &lane,
            crate::round::draw::RoundColor(1.0, 0.0, 0.0, 1.0),
            lane.ring.track_start_deg,
            crate::round::hud::growth_ring_fill_end_deg(&lane.ring, 0.25),
            [360.0, 360.0, 180.0, 180.0],
            [180.0, 0.0, 0.0, 0.0],
        );

        assert_eq!(primitives.len(), 1);
        assert_eq!(
            blends,
            vec![crate::presentation::smooth::SmoothBlendMode::Normal]
        );
        let primitive = primitives[0];
        assert_eq!(primitive.params[0], 5.0);
        assert_eq!(primitive.params[3], 1.0);
        assert!((primitive.uv[0] - lane.ring.track_start_deg.to_radians() as f32).abs() < 1e-6);
        assert!(
            (primitive.uv[1] - (lane.ring.track_sweep_deg * 0.25).to_radians() as f32).abs() < 1e-6
        );
    }
}
