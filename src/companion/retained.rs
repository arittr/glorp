#![cfg(all(target_os = "macos", feature = "retained-renderer"))]

use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};

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

mod buffers;
mod capture;
mod host;
mod metrics;
mod parity;
mod presentation;
mod resources;
#[allow(dead_code)] // Integrated with RetainedHost in the follow-up activation change.
mod worker;

#[cfg(test)]
use buffers::{persistent_instance_capacity, FIXED_INSTANCE_RING_MIN};
use buffers::{
    PersistentCaptureResources, PersistentFrameBuffers, INSTANCE_RING_LEN, INSTANCE_STRIDE,
};
pub(crate) use capture::CanonicalRgbaFrame;
#[cfg(test)]
use host::{physical_dimension, LayerActivationGuard, LayerActivationState};
pub(super) use host::{ActiveRetainedHost, PreparedRetainedHost};
use host::{Pipelines, RetainedHost};
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
use worker::{RasterJob, RasterReply, RasterSubmitError, RasterWorker};

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

pub(super) struct PreparedGpuPrimitiveCollector {
    resources: CompiledRetainedResources,
}

impl PreparedGpuPrimitiveCollector {
    pub(super) fn for_species(
        species: crate::pet::generation::Species,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let manifest = GlyphRepertoireManifest::for_active_pet(
            CompanionContentIdentity::for_pet(species),
            2.0,
        );
        Ok(Self {
            resources: CompiledRetainedResources::for_capacity_inventory(&manifest),
        })
    }

    pub(super) fn observe(
        &self,
        frame: &crate::companion::app::PreparedCompanionFrame,
    ) -> std::result::Result<u32, RetainedFailureCategory> {
        use crate::companion::paired_review::RendererIdentitySource;

        let RendererIdentitySource::Smooth {
            metrics,
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            plan,
            draw_order,
        } = frame.renderer_source()
        else {
            return Err(RetainedFailureCategory::CaptureUnsupportedVariant);
        };
        let background = frame.review_background();
        let mood_aura = frame.review_mood_aura();
        let chrome = RetainedChrome {
            mood_aura: [mood_aura.0, mood_aura.1, mood_aura.2, mood_aura.3],
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            gauges: frame.review_gauges(),
            overlays: frame.review_overlays(),
            hud: frame.review_hud(),
            hud_font_size: frame.review_hud_font_size(),
            dim_overlay: frame.review_dim_overlay(),
        };
        let prepared = prepare_gpu_frame(
            plan,
            draw_order,
            metrics,
            frame.review_aperture(),
            [background.0, background.1, background.2, background.3],
            &chrome,
            self.resources.atlas(),
        )?;
        Ok(prepared.primitives.len().min(u32::MAX as usize) as u32)
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourcePreparationKey {
    identity: CompanionContentIdentity,
    backing_scale_bits: u64,
}

impl ResourcePreparationKey {
    fn new(identity: CompanionContentIdentity, backing_scale: f64) -> Self {
        Self {
            identity,
            backing_scale_bits: backing_scale.to_bits(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourcePreparationRequest {
    id: u64,
    key: ResourcePreparationKey,
    enqueued_at: Instant,
}

struct FailedGlyphPreparation {
    id: u64,
    key: ResourcePreparationKey,
    category: RetainedFailureCategory,
}

struct ResourcePreparationController {
    next_id: u64,
    visible: bool,
    desired: Option<ResourcePreparationRequest>,
    running: Option<ResourcePreparationRequest>,
    latest_pending: Option<ResourcePreparationRequest>,
    hidden_desired: Option<ResourcePreparationKey>,
    worker_unavailable: bool,
}

impl ResourcePreparationController {
    fn new() -> Self {
        Self {
            next_id: 1,
            visible: true,
            desired: None,
            running: None,
            latest_pending: None,
            hidden_desired: None,
            worker_unavailable: false,
        }
    }

    fn fresh(&mut self, key: ResourcePreparationKey) -> ResourcePreparationRequest {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .filter(|next| *next < u64::MAX)
            .expect("resource preparation request id exhausted");
        ResourcePreparationRequest { id, key, enqueued_at: Instant::now() }
    }

    fn set_visible_desired(&mut self, key: ResourcePreparationKey) -> Option<u64> {
        let changed =
            !self.visible || self.desired.as_ref().map(|request| &request.key) != Some(&key);
        self.visible = true;
        self.hidden_desired = None;
        if !changed {
            return None;
        }
        let request = self.fresh(key);
        let cancel_epoch = self.running.as_ref().map(|_| request.id);
        self.desired = Some(request.clone());
        self.latest_pending = Some(request);
        cancel_epoch
    }

    fn suspend(&mut self, key: ResourcePreparationKey) -> Option<u64> {
        self.hidden_desired = Some(key);
        self.latest_pending = None;
        if !self.visible {
            return None;
        }
        self.visible = false;
        let running_key = self.running.as_ref()?.key.clone();
        Some(self.fresh(running_key).id)
    }

    fn finish_running(&mut self, id: u64) -> Option<ResourcePreparationRequest> {
        if self.running.as_ref().map(|request| request.id) != Some(id) {
            return None;
        }
        self.running.take()
    }

    fn take_pending_if_idle(&mut self) -> Option<ResourcePreparationRequest> {
        if self.running.is_some() {
            return None;
        }
        self.latest_pending.take()
    }

    fn mark_submitted(&mut self, request: ResourcePreparationRequest) {
        debug_assert!(self.running.is_none());
        self.running = Some(request);
    }

    fn coalesces(&self, key: &ResourcePreparationKey) -> bool {
        self.visible
            && self.desired.as_ref().map(|request| &request.key) == Some(key)
            && (self.running.is_some() || self.latest_pending.is_some())
    }

    fn accepts_completed(
        &self,
        request: &ResourcePreparationRequest,
        observed: &ResourcePreparationKey,
    ) -> bool {
        self.visible && self.desired.as_ref() == Some(request) && &request.key == observed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourcePreparationTick {
    Ready,
    YieldedRetainingActive,
    YieldedWithoutActive,
    FailedRetainingActive(RetainedFailureCategory),
    FailedWithoutActive(RetainedFailureCategory),
}

fn resource_failure_tick(
    has_active: bool,
    category: RetainedFailureCategory,
) -> ResourcePreparationTick {
    if has_active {
        ResourcePreparationTick::FailedRetainingActive(category)
    } else {
        ResourcePreparationTick::FailedWithoutActive(category)
    }
}

fn cached_current_failure(
    failed: Option<&FailedGlyphPreparation>,
    current: &ResourcePreparationRequest,
) -> Option<RetainedFailureCategory> {
    failed
        .filter(|failed| failed.id == current.id && failed.key == current.key)
        .map(|failed| failed.category)
}

fn terminal_worker_decision(
    active_matches_desired: bool,
    has_active: bool,
) -> ResourcePreparationTick {
    if active_matches_desired {
        ResourcePreparationTick::Ready
    } else {
        resource_failure_tick(has_active, RetainedFailureCategory::RasterWorkerUnavailable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LifetimeAuditPhase {
    Warmup,
    Measured,
}

#[derive(Debug, Clone, Copy)]
struct LifetimeFrameObservation {
    semantic_hash: u64,
    gpu_frame_hash: u64,
    draw_calls: u64,
    gpu_bytes: u64,
}

trait LifetimeAuditExecutor {
    fn render_frame(
        &mut self,
        phase: LifetimeAuditPhase,
        frame: u64,
        now: time::OffsetDateTime,
    ) -> std::result::Result<LifetimeFrameObservation, RetainedFailureCategory>;

    fn poll(&mut self) -> std::result::Result<(), RetainedFailureCategory>;

    fn rss_bytes(&mut self) -> std::result::Result<u64, RetainedFailureCategory>;
}

struct GpuLifetimeAuditExecutor<'host, Prepare> {
    host: &'host mut RetainedHost,
    prepare: Prepare,
    last_submission: Option<wgpu::SubmissionIndex>,
}

impl<Prepare> LifetimeAuditExecutor for GpuLifetimeAuditExecutor<'_, Prepare>
where
    Prepare: FnMut(
        LifetimeAuditPhase,
        u64,
        time::OffsetDateTime,
    ) -> std::result::Result<
        (crate::companion::app::PreparedCompanionFrame, u64),
        RetainedFailureCategory,
    >,
{
    fn render_frame(
        &mut self,
        phase: LifetimeAuditPhase,
        frame: u64,
        now: time::OffsetDateTime,
    ) -> std::result::Result<LifetimeFrameObservation, RetainedFailureCategory> {
        use crate::companion::paired_review::RendererIdentitySource;

        let (prepared, semantic_hash) = (self.prepare)(phase, frame, now)?;
        let RendererIdentitySource::Smooth {
            metrics,
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            plan,
            draw_order,
        } = prepared.renderer_source()
        else {
            return Err(RetainedFailureCategory::LifetimeFramePreparation);
        };
        let background_color = prepared.review_background();
        let background = [
            background_color.0,
            background_color.1,
            background_color.2,
            background_color.3,
        ];
        let mood_aura = prepared.review_mood_aura();
        let chrome = RetainedChrome {
            mood_aura: [mood_aura.0, mood_aura.1, mood_aura.2, mood_aura.3],
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            gauges: prepared.review_gauges(),
            overlays: prepared.review_overlays(),
            hud: prepared.review_hud(),
            hud_font_size: prepared.review_hud_font_size(),
            dim_overlay: prepared.review_dim_overlay(),
        };
        let gpu_frame = {
            let active = self
                .host
                .glyph_resources
                .as_ref()
                .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
            prepare_gpu_frame(
                plan,
                draw_order,
                metrics,
                prepared.review_aperture(),
                background,
                &chrome,
                active.resources.atlas(),
            )?
        };
        let gpu_frame_hash = hash_prepared_gpu_frame(&gpu_frame);
        let draw_calls = gpu_frame.blends.len() as u64;
        let width = self.host.physical_width;
        let height = self.host.physical_height;
        self.host.ensure_capture_resources(width, height);
        let mut encoder =
            self.host
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("glorp-retained-lifetime-audit"),
                });
        self.host.prepare_frame(&mut encoder, &gpu_frame);
        {
            let active = self
                .host
                .glyph_resources
                .as_ref()
                .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
            let capture = self
                .host
                .capture_resources
                .as_ref()
                .expect("lifetime capture target is prewarmed");
            self.host.encode_scene(
                &mut encoder,
                &capture.intermediate_view,
                &active.bind_group,
                self.host.frame_buffers.current_buffer(),
                &gpu_frame.blends,
                background,
            );
        }
        self.host.frame_buffers.finish_uploads();
        self.last_submission = Some(self.host.queue.submit([encoder.finish()]));
        self.host.frame_buffers.recall_uploads();
        let gpu_bytes = self
            .host
            .metrics
            .gpu_accounting_snapshot()
            .current_bytes
            .total_bytes;
        Ok(LifetimeFrameObservation {
            semantic_hash,
            gpu_frame_hash,
            draw_calls,
            gpu_bytes,
        })
    }

    fn poll(&mut self) -> std::result::Result<(), RetainedFailureCategory> {
        let Some(submission) = self.last_submission.take() else {
            return Ok(());
        };
        self.host
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            })
            .map(|_| ())
            .map_err(|_| RetainedFailureCategory::LifetimeGpuPoll)
    }

    fn rss_bytes(&mut self) -> std::result::Result<u64, RetainedFailureCategory> {
        current_process_rss_bytes()
    }
}

fn hash_prepared_gpu_frame(frame: &PreparedGpuFrame) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in bytemuck::cast_slice::<GpuPrimitive, u8>(&frame.primitives) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    for blend in &frame.blends {
        let byte = match blend {
            SmoothBlendMode::Normal => 0,
            SmoothBlendMode::Multiply => 1,
            SmoothBlendMode::Screen => 2,
            SmoothBlendMode::Add => 3,
            SmoothBlendMode::Replace => 4,
        };
        hash ^= byte;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn run_lifetime_schedule(
    executor: &mut impl LifetimeAuditExecutor,
    frames: u64,
) -> std::result::Result<LifetimeAuditSnapshot, RetainedFailureCategory> {
    const CADENCE_MS: i64 = 250;
    const SAMPLE_INTERVAL: u64 = 256;
    let base = time::macros::datetime!(2026-06-13 18:00 UTC);
    let mut audit = LifetimeAuditSnapshot {
        frames,
        warmup_frames: frames,
        cadence_ms: CADENCE_MS as u64,
        ..LifetimeAuditSnapshot::default()
    };
    let mut previous_semantic = None;
    let mut previous_gpu = None;

    for phase in [LifetimeAuditPhase::Warmup, LifetimeAuditPhase::Measured] {
        let mut now = base;
        let mut phase_gpu_final = 0_u64;
        for frame in 0..frames {
            let observation = executor.render_frame(phase, frame, now)?;
            phase_gpu_final = observation.gpu_bytes;
            match phase {
                LifetimeAuditPhase::Warmup => {
                    audit.gpu_warmup_peak_bytes =
                        audit.gpu_warmup_peak_bytes.max(observation.gpu_bytes);
                }
                LifetimeAuditPhase::Measured => {
                    audit.prepared_frames = audit.prepared_frames.saturating_add(1);
                    audit.encoded_frames = audit.encoded_frames.saturating_add(1);
                    audit.draw_calls = audit.draw_calls.saturating_add(observation.draw_calls);
                    if previous_semantic
                        .replace(observation.semantic_hash)
                        .is_some_and(|previous| previous != observation.semantic_hash)
                    {
                        audit.semantic_frame_changes =
                            audit.semantic_frame_changes.saturating_add(1);
                    }
                    if previous_gpu
                        .replace(observation.gpu_frame_hash)
                        .is_some_and(|previous| previous != observation.gpu_frame_hash)
                    {
                        audit.gpu_frame_hash_changes =
                            audit.gpu_frame_hash_changes.saturating_add(1);
                    }
                    audit.gpu_peak_bytes = audit.gpu_peak_bytes.max(observation.gpu_bytes);
                }
            }
            // This audit measures resource lifetime, not burst throughput. Waiting once per
            // virtual frame bounds macOS frame-pacing bookkeeping that the zero-delay synthetic
            // loop would otherwise accumulate between the companion's modeled 4 Hz frames.
            executor.poll()?;
            audit.poll_count = audit.poll_count.saturating_add(1);
            if (frame + 1) % SAMPLE_INTERVAL == 0 {
                let rss = executor.rss_bytes()?;
                match phase {
                    LifetimeAuditPhase::Warmup => {
                        audit.rss_warmup_peak_bytes = audit.rss_warmup_peak_bytes.max(rss)
                    }
                    LifetimeAuditPhase::Measured => {
                        audit.rss_peak_bytes = audit.rss_peak_bytes.max(rss)
                    }
                }
            }
            now += time::Duration::milliseconds(CADENCE_MS);
        }
        executor.poll()?;
        audit.poll_count = audit.poll_count.saturating_add(1);
        let rss_final = executor.rss_bytes()?;
        match phase {
            LifetimeAuditPhase::Warmup => {
                audit.rss_warmup_bytes = rss_final;
                audit.rss_warmup_peak_bytes = audit.rss_warmup_peak_bytes.max(rss_final);
                audit.gpu_warmup_bytes = phase_gpu_final;
                audit.gpu_warmup_peak_bytes = audit.gpu_warmup_peak_bytes.max(phase_gpu_final);
            }
            LifetimeAuditPhase::Measured => {
                audit.rss_final_bytes = rss_final;
                audit.rss_peak_bytes = audit.rss_peak_bytes.max(rss_final);
                audit.gpu_final_bytes = phase_gpu_final;
                audit.gpu_peak_bytes = audit.gpu_peak_bytes.max(phase_gpu_final);
                audit.virtual_elapsed_ms = (now - base)
                    .whole_milliseconds()
                    .max(0)
                    .min(i128::from(u64::MAX)) as u64;
            }
        }
    }
    Ok(audit)
}

fn current_process_rss_bytes() -> std::result::Result<u64, RetainedFailureCategory> {
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
    if rc == 0 && usage.resident_size > 0 {
        Ok(usage.resident_size)
    } else {
        Err(RetainedFailureCategory::LifetimeRssUnavailable)
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
        cached_current_failure, create_atlas_bind_group_layout, create_pipelines,
        current_process_rss_bytes, glyph_advance, glyph_ink_rect, glyph_run_height,
        glyph_run_width, persistent_instance_capacity, physical_dimension, push_analytic_arc,
        resource_failure_tick, run_lifetime_schedule, terminal_worker_decision, upload_glyph_atlas,
        CompiledGlyphAtlas, CompiledRetainedResources, FailedGlyphPreparation, GlyphAtlasEntry,
        GlyphKey, GlyphRepertoireManifest, GpuPrimitive, LayerActivationGuard,
        LayerActivationState, LifetimeAuditExecutor, LifetimeAuditPhase, LifetimeFrameObservation,
        PersistentFrameBuffers, Pipelines, PreparedGpuFrame, ResourcePreparationController,
        ResourcePreparationKey, ResourcePreparationTick, RetainedFailureCategory,
        RetainedResourceCounters, SmoothBlendMode, FIXED_INSTANCE_RING_MIN, GLYPH_FONT_SIZE,
        INSTANCE_RING_LEN, INSTANCE_STRIDE, RETAINED_ATLAS_POINT_SIZE,
    };
    use crate::pet::generation::Species;
    use crate::round::smooth::CompanionContentIdentity;
    use std::collections::BTreeMap;

    /// The surface format the headless resource harness builds its pipelines and
    /// capture intermediate against — the linear (non-sRGB) `Bgra8Unorm` the
    /// production surface now composites into for gamma-space blending. No surface
    /// is created, so this only selects the pipeline color-target format.
    const TEST_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

    fn preparation_key(species: Species, scale: f64) -> ResourcePreparationKey {
        ResourcePreparationKey::new(CompanionContentIdentity::for_pet(species), scale)
    }

    #[test]
    fn resource_controller_keeps_only_c_pending_while_a_runs() {
        let mut controller = ResourcePreparationController::new();
        controller.set_visible_desired(preparation_key(Species::Crystal, 2.0));
        let submitted = controller.take_pending_if_idle().unwrap();
        controller.mark_submitted(submitted);
        assert_eq!(
            controller.set_visible_desired(preparation_key(Species::Fuzz, 2.0)),
            Some(2)
        );
        assert_eq!(
            controller.set_visible_desired(preparation_key(Species::Blob, 2.0)),
            Some(3)
        );
        let pending = controller.latest_pending.as_ref().unwrap();
        assert_eq!(pending.id, 3);
        assert_eq!(pending.key, preparation_key(Species::Blob, 2.0));
        assert_eq!(controller.running.as_ref().unwrap().id, 1);
        let next = controller.finish_running(1).unwrap();
        assert_eq!(next.id, 1);
        let next = controller.take_pending_if_idle().unwrap();
        assert_eq!(next.id, 3);
    }

    #[test]
    fn resource_controller_coalesces_same_key_without_reusing_or_allocating_id() {
        let mut controller = ResourcePreparationController::new();
        let key = preparation_key(Species::Crystal, 2.0);
        assert_eq!(controller.set_visible_desired(key.clone()), None);
        let id = controller.desired.as_ref().unwrap().id;
        assert_eq!(controller.set_visible_desired(key), None);
        assert_eq!(controller.desired.as_ref().unwrap().id, id);
        assert_eq!(controller.next_id, 2);
    }

    #[test]
    fn hide_then_same_key_reveal_allocates_fresh_id_and_rejects_pre_hide_completion() {
        let mut controller = ResourcePreparationController::new();
        let key = preparation_key(Species::Crystal, 2.0);
        controller.set_visible_desired(key.clone());
        let submitted = controller.take_pending_if_idle().unwrap();
        controller.mark_submitted(submitted);
        let pre_hide = controller.running.as_ref().unwrap().clone();
        assert_eq!(controller.suspend(key.clone()), Some(2));
        assert_eq!(controller.set_visible_desired(key.clone()), Some(3));
        assert!(!controller.accepts_completed(&pre_hide, &key));
        assert_eq!(controller.latest_pending.as_ref().unwrap().id, 3);
    }

    #[test]
    fn hidden_key_churn_remembers_only_latest_for_reveal() {
        let mut controller = ResourcePreparationController::new();
        let a = preparation_key(Species::Crystal, 2.0);
        let b = preparation_key(Species::Fuzz, 2.0);
        let c = preparation_key(Species::Blob, 2.0);
        controller.set_visible_desired(a.clone());
        let submitted = controller.take_pending_if_idle().unwrap();
        controller.mark_submitted(submitted);
        controller.suspend(a);
        assert_eq!(controller.suspend(b), None);
        assert_eq!(controller.suspend(c.clone()), None);
        assert_eq!(controller.hidden_desired.as_ref(), Some(&c));
        assert_eq!(controller.set_visible_desired(c.clone()), Some(3));
        assert_eq!(controller.latest_pending.as_ref().unwrap().key, c);
    }

    #[test]
    fn stale_completed_request_has_zero_materializations() {
        let mut controller = ResourcePreparationController::new();
        let a = preparation_key(Species::Crystal, 2.0);
        let b = preparation_key(Species::Fuzz, 2.0);
        controller.set_visible_desired(a.clone());
        let stale = controller.take_pending_if_idle().unwrap();
        controller.mark_submitted(stale.clone());
        controller.set_visible_desired(b.clone());
        let mut materializations = 0;
        if controller.accepts_completed(&stale, &b) {
            materializations += 1;
        }
        assert_eq!(materializations, 0);
    }

    #[test]
    fn stale_failure_does_not_poison_current_request_but_current_failure_caches() {
        let mut controller = ResourcePreparationController::new();
        controller.set_visible_desired(preparation_key(Species::Crystal, 2.0));
        let stale = controller.desired.as_ref().unwrap().clone();
        controller.set_visible_desired(preparation_key(Species::Fuzz, 2.0));
        let current = controller.desired.as_ref().unwrap().clone();
        let stale_failure = FailedGlyphPreparation {
            id: stale.id,
            key: stale.key,
            category: RetainedFailureCategory::AtlasUnavailable,
        };
        assert_eq!(cached_current_failure(Some(&stale_failure), &current), None);
        let current_failure = FailedGlyphPreparation {
            id: current.id,
            key: current.key.clone(),
            category: RetainedFailureCategory::AtlasUnavailable,
        };
        assert_eq!(
            cached_current_failure(Some(&current_failure), &current),
            Some(RetainedFailureCategory::AtlasUnavailable)
        );
    }

    #[test]
    fn unavailable_worker_preserves_active_or_requests_fallback_without_active() {
        let category = RetainedFailureCategory::RasterWorkerUnavailable;
        assert_eq!(
            resource_failure_tick(true, category),
            ResourcePreparationTick::FailedRetainingActive(category)
        );
        assert_eq!(
            resource_failure_tick(false, category),
            ResourcePreparationTick::FailedWithoutActive(category)
        );
    }

    #[test]
    fn terminal_worker_allows_matching_active_but_fails_new_generation() {
        assert_eq!(
            terminal_worker_decision(true, true),
            ResourcePreparationTick::Ready
        );
        assert_eq!(
            terminal_worker_decision(false, true),
            ResourcePreparationTick::FailedRetainingActive(
                RetainedFailureCategory::RasterWorkerUnavailable
            )
        );
        assert_eq!(
            terminal_worker_decision(false, false),
            ResourcePreparationTick::FailedWithoutActive(
                RetainedFailureCategory::RasterWorkerUnavailable
            )
        );
    }

    #[test]
    fn retained_source_has_no_ui_thread_compiled_preparation_boundary() {
        let forbidden = ["Compiled", "RetainedResourcesPreparation"].concat();
        assert!(!include_str!("retained.rs").contains(&forbidden));
        assert!(include_str!("app.rs").contains("suspend_resource_preparation"));
    }

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
            let mut frame_buffers = PersistentFrameBuffers::new(&device);
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
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("glorp-retained-test-instance-upload"),
                });
            self.frame_buffers.ensure_instance_capacity(
                frame.primitives.len(),
                &self.device,
                &mut self.counters,
            );
            self.frame_buffers.write_frame_instances(
                &mut encoder,
                &frame.primitives,
                &mut self.counters,
            );
            self.frame_buffers.finish_uploads();
            self.queue.submit([encoder.finish()]);
            self.frame_buffers.recall_uploads();
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
        let inventory = crate::companion::paired_review::full_preview_capacity_inventory();
        let observed = inventory
            .max_prepared_gpu_primitives
            .observed
            .expect("the production matrix observes prepared primitives")
            as usize;
        assert_eq!(FIXED_INSTANCE_RING_MIN, 1_024);
        assert_eq!(
            inventory.max_prepared_gpu_primitives.headroom as usize,
            FIXED_INSTANCE_RING_MIN - observed,
        );
        assert_eq!(persistent_instance_capacity(1), 1_024);
        assert_eq!(persistent_instance_capacity(observed), 1_024);
    }

    #[test]
    fn varying_full_fixture_counts_never_recreate_the_instance_ring() {
        let mut host = TestRetainedResources::warm();
        let before = host.counters();
        let observed = crate::companion::paired_review::full_preview_capacity_inventory()
            .max_prepared_gpu_primitives
            .observed
            .expect("the production matrix observes prepared primitives")
            as usize;
        for count in [
            1,
            96,
            observed,
            12,
            observed.saturating_sub(1),
            0,
            512,
            observed,
        ] {
            host.prepare_frame(&prepared_frame_with_count(count))
                .unwrap();
        }
        let delta = host.counters() - before;
        assert_eq!(delta.buffer_creations, 0);
        assert_eq!(delta.instance_writes, 8);
        let expected_bytes = [
            1,
            96,
            observed,
            12,
            observed.saturating_sub(1),
            0,
            512,
            observed,
        ]
        .into_iter()
        .sum::<usize>()
            * INSTANCE_STRIDE;
        assert_eq!(delta.instance_write_bytes, expected_bytes as u64);
    }

    #[test]
    fn zero_instance_frame_advances_ring_and_records_one_zero_byte_write() {
        let mut host = TestRetainedResources::warm();
        let before = host.counters();
        let cursor = host.frame_buffers.cursor;
        host.prepare_frame(&prepared_frame_with_count(0)).unwrap();
        let delta = host.counters() - before;
        assert_eq!(delta.instance_writes, 1);
        assert_eq!(delta.instance_write_bytes, 0);
        assert_eq!(delta.buffer_creations, 0);
        assert_eq!(host.frame_buffers.cursor, (cursor + 1) % INSTANCE_RING_LEN);
    }

    #[test]
    fn process_rss_sampler_reads_current_process_without_spawning() {
        assert!(current_process_rss_bytes().is_ok_and(|bytes| bytes > 0));
    }

    #[derive(Default)]
    struct FakeLifetimeExecutor {
        calls: Vec<(LifetimeAuditPhase, u64, i128)>,
        polls: u64,
        rss_calls: u64,
        fail_poll: bool,
        fail_rss: bool,
    }

    impl LifetimeAuditExecutor for FakeLifetimeExecutor {
        fn render_frame(
            &mut self,
            phase: LifetimeAuditPhase,
            frame: u64,
            now: time::OffsetDateTime,
        ) -> std::result::Result<LifetimeFrameObservation, RetainedFailureCategory> {
            self.calls.push((phase, frame, now.unix_timestamp_nanos()));
            Ok(LifetimeFrameObservation {
                semantic_hash: frame % 2,
                gpu_frame_hash: frame % 3,
                draw_calls: frame + 1,
                gpu_bytes: 1_000,
            })
        }

        fn poll(&mut self) -> std::result::Result<(), RetainedFailureCategory> {
            self.polls += 1;
            if self.fail_poll {
                Err(RetainedFailureCategory::LifetimeGpuPoll)
            } else {
                Ok(())
            }
        }

        fn rss_bytes(&mut self) -> std::result::Result<u64, RetainedFailureCategory> {
            self.rss_calls += 1;
            if self.fail_rss {
                Err(RetainedFailureCategory::LifetimeRssUnavailable)
            } else {
                Ok(10_000)
            }
        }
    }

    #[test]
    fn lifetime_schedule_repeats_identical_warmup_and_measured_virtual_clocks() {
        let mut executor = FakeLifetimeExecutor::default();
        let audit = run_lifetime_schedule(&mut executor, 4).unwrap();
        assert_eq!(executor.calls.len(), 8);
        let warmup = &executor.calls[..4];
        let measured = &executor.calls[4..];
        for (warm, measured) in warmup.iter().zip(measured) {
            assert_eq!(warm.1, measured.1);
            assert_eq!(warm.2, measured.2);
        }
        assert_eq!(audit.frames, 4);
        assert_eq!(audit.warmup_frames, 4);
        assert_eq!(audit.prepared_frames, 4);
        assert_eq!(audit.encoded_frames, 4);
        assert!(audit.semantic_frame_changes > 0);
        assert!(audit.gpu_frame_hash_changes > 0);
        assert!(audit.draw_calls > 0);
        assert_eq!(audit.virtual_elapsed_ms, 1_000);
        assert_eq!(audit.poll_count, 10);
        assert_eq!(executor.polls, 10);
    }

    #[test]
    fn lifetime_schedule_fails_closed_on_poll_or_rss_errors() {
        let mut poll_failure = FakeLifetimeExecutor { fail_poll: true, ..Default::default() };
        assert_eq!(
            run_lifetime_schedule(&mut poll_failure, 1),
            Err(RetainedFailureCategory::LifetimeGpuPoll)
        );
        let mut rss_failure = FakeLifetimeExecutor { fail_rss: true, ..Default::default() };
        assert_eq!(
            run_lifetime_schedule(&mut rss_failure, 1),
            Err(RetainedFailureCategory::LifetimeRssUnavailable)
        );
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
