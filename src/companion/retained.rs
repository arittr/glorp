#![cfg(all(target_os = "macos", feature = "retained-renderer"))]

use std::time::Instant;

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
mod compiler;
mod host;
#[allow(dead_code)] // Pure sealed HUD preparation; GPU binding lands in the activation slice.
pub(crate) mod hud;
mod metrics;
mod parity;
mod presentation;
mod render;
mod resources;
#[allow(dead_code)] // Integrated with RetainedHost in the follow-up activation change.
mod worker;

#[cfg(test)]
use buffers::{persistent_instance_capacity, FIXED_INSTANCE_RING_MIN};
use buffers::{
    PersistentCaptureResources, PersistentFrameBuffers, INSTANCE_RING_LEN, INSTANCE_STRIDE,
};
pub(crate) use capture::CanonicalRgbaFrame;
#[cfg(feature = "dev-preview")]
pub(crate) use host::run_review_scene_soak;
pub(crate) use host::DirectSceneCapture;
use host::Pipelines;
#[cfg(test)]
use host::{physical_dimension, LayerActivationGuard, LayerActivationState};
pub(super) use host::{
    ActiveRetainedHost, PreparedRetainedHost, SceneGenerationServiceTick, ScenePresentOutcome,
};
pub(crate) use metrics::{
    duration_us, CapacityContract, CompanionCapacityInventory, CompanionRuntimeMetrics,
    CompanionRuntimeMetricsSnapshot, GpuAllocationKind, LifetimeAuditSnapshot,
    RuntimeFixtureIdentity, RuntimeIdentity, RuntimeWorkCounters,
};
pub(crate) use presentation::{
    FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox, RetainedFailureCategory,
    SkipReason,
};

#[cfg(test)]
pub(crate) fn direct_scene_capture_fixture(
    receipt: crate::presentation::companion_scene::contract::PresentedSceneVersion,
    rgba: Vec<u8>,
) -> DirectSceneCapture {
    let cpu = compiler::compile_projected_full_scene_for_render_test(0);
    DirectSceneCapture {
        receipt,
        source: cpu.capture_source_identity().unwrap(),
        scene_artifacts: cpu.scene_artifacts().unwrap(),
        logical_state_alias:
            crate::presentation::companion_scene::contract::CompanionCaptureStateAlias::Normal,
        rgba,
        presented_scene_count: 5,
        last_present_age_ms: 7,
    }
}

use resources::{
    CompiledGlyphAtlas, CompiledRetainedResources, FragmentGlyphMode, GlyphAtlasEntry, GlyphKey,
    GlyphRepertoireManifest, RetainedResourceCounters, RETAINED_ATLAS_POINT_SIZE,
};
use worker::CpuSceneBuildCandidate;
use worker::{RasterJob, RasterReply, RasterSubmitError, SceneBuildWorker};

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

#[allow(dead_code)] // Owned by the Task 12 host seam; live reads begin with Task 14 routing.
struct ReadyGpuCandidate {
    identity: crate::presentation::companion_scene::runtime::RequestIdentity,
    version: crate::presentation::companion_scene::SceneVersion,
    backing_scale: f64,
    cpu: compiler::CpuSceneCandidate,
    gpu: render::GpuSceneCandidate,
    atlas: resources::PreparedSceneAtlas,
}

#[allow(dead_code)] // Owned by the Task 12 host seam; live reads begin with Task 14 routing.
struct ActiveSceneGeneration {
    version: crate::presentation::companion_scene::SceneVersion,
    backing_scale: f64,
    cpu: compiler::CpuSceneCandidate,
    gpu: render::GpuSceneCandidate,
    atlas: resources::PreparedSceneAtlas,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::companion) struct SceneReplacementIdentity {
    generation: crate::presentation::companion_scene::SceneGenerationKey,
    surface: crate::presentation::companion_scene::SurfaceEpoch,
    source: crate::presentation::companion_scene::AppliedRevisions,
}

#[cfg(test)]
impl SceneReplacementIdentity {
    pub(in crate::companion) fn for_test(serial: u64) -> Self {
        Self {
            generation: crate::presentation::companion_scene::SceneGenerationKey {
                device: crate::presentation::companion_scene::DeviceEpoch(serial),
                layout: crate::presentation::companion_scene::LayoutGeneration(serial),
                resources: crate::presentation::companion_scene::ResourceGeneration(serial),
            },
            surface: crate::presentation::companion_scene::SurfaceEpoch(serial),
            source: crate::presentation::companion_scene::AppliedRevisions::new(serial, serial),
        }
    }
}

fn should_defer_scene_reveal(
    external: Option<crate::presentation::companion_scene::SceneVersion>,
    logical: Option<crate::presentation::companion_scene::SceneVersion>,
) -> bool {
    matches!((external, logical), (Some(external), Some(logical)) if external != logical)
}

fn logical_viewport_matches_surface(
    logical_viewport_points: [f32; 2],
    physical_extent: [u32; 2],
    backing_scale: f64,
) -> bool {
    logical_viewport_points
        .into_iter()
        .enumerate()
        .all(|(axis, logical)| {
            host::physical_dimension(f64::from(logical), backing_scale) == physical_extent[axis]
        })
}

#[derive(Debug)]
#[allow(dead_code)] // Surfaced by the dormant Task 12 host scene service until Task 14 routing.
enum SceneCandidatePreparationError {
    MissingCpuCandidate,
    StaleCandidate,
    Rebase(crate::presentation::companion_scene::runtime::CandidateRebaseError),
    CpuDelta(compiler::MirrorDeltaError),
    Upload(render::SceneUploadError),
    Gpu(render::SceneGpuError),
}

impl std::fmt::Display for SceneCandidatePreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCpuCandidate => formatter.write_str("no CPU scene candidate is ready"),
            Self::StaleCandidate => formatter.write_str("the CPU scene candidate is stale"),
            Self::Rebase(error) => write!(formatter, "scene candidate rebase failed: {error:?}"),
            Self::CpuDelta(error) => {
                write!(formatter, "scene candidate mirror update failed: {error:?}")
            }
            Self::Upload(error) => write!(formatter, "scene upload preparation failed: {error:?}"),
            Self::Gpu(error) => write!(formatter, "scene GPU materialization failed: {error:?}"),
        }
    }
}

impl std::error::Error for SceneCandidatePreparationError {}

#[allow(dead_code)] // Concrete host coordinator is intentionally not called by ui_tick until Task 14.
struct RetainedSceneGenerationState {
    runtime: crate::presentation::companion_scene::runtime::CompanionSceneRuntimeState,
    cpu_candidate: Option<Box<CpuReadySceneCandidate>>,
    ready_candidate: Option<ReadyGpuCandidate>,
    active: Option<ActiveSceneGeneration>,
    #[cfg(test)]
    gpu_materializations: u64,
}

fn snapshot_depth_composition(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> Result<crate::round::depth::CompanionDepthComposition, hud::HudPreparationError> {
    let lifecycle_scale = crate::presentation::companion_effects::depth_lifecycle_scale(
        snapshot.frame.asleep,
        snapshot.frame.calm,
    );
    let effective_depth = crate::presentation::companion_effects::effective_depth(
        snapshot.frame.pet_depth,
        lifecycle_scale,
    );
    crate::round::depth::CompanionDepthComposition::resolve(effective_depth)
        .map_err(|_| hud::HudPreparationError::InvalidGeometry)
}

#[allow(dead_code)] // Held only by the dormant Task 12 host coordinator until Task 14 routing.
struct CpuReadySceneCandidate {
    identity: crate::presentation::companion_scene::runtime::RequestIdentity,
    backing_scale: f64,
    cpu: compiler::CpuSceneCandidate,
    atlas: resources::PreparedSceneAtlas,
}

#[allow(dead_code)] // Host-owned production seam; installed by the Task 14 live route.
pub(in crate::companion) struct RetainedSceneActivation {
    generations: RetainedSceneGenerationState,
    gpu: Option<RetainedSceneGpuState>,
}

struct RetainedSceneGpuState {
    shared: render::SceneGpuShared,
    renderer: render::SceneRenderer,
}

#[allow(dead_code)] // Exercised by Task 12 tests; production calls begin with Task 14 routing.
impl RetainedSceneGenerationState {
    fn ready_hud_depth_composition(
        &self,
    ) -> Result<crate::round::depth::CompanionDepthComposition, hud::HudPreparationError> {
        let snapshot = self
            .runtime
            .pending_desired_snapshot()
            .ok_or(hud::HudPreparationError::InvalidGeometry)?;
        snapshot_depth_composition(snapshot)
    }

    fn active_hud_depth_composition(
        &self,
    ) -> Result<crate::round::depth::CompanionDepthComposition, hud::HudPreparationError> {
        let lease = self
            .runtime
            .capture_lease()
            .map_err(|_| hud::HudPreparationError::InvalidGeometry)?;
        snapshot_depth_composition(lease.source_snapshot())
    }

    fn active_hud_depth_composition_for_version(
        &self,
        expected: crate::presentation::companion_scene::SceneVersion,
    ) -> Result<crate::round::depth::CompanionDepthComposition, hud::HudPreparationError> {
        let lease = self
            .runtime
            .capture_lease()
            .map_err(|_| hud::HudPreparationError::InvalidGeometry)?;
        if lease.version() != expected {
            return Err(hud::HudPreparationError::InvalidGeometry);
        }
        snapshot_depth_composition(lease.source_snapshot())
    }
    fn new(
        runtime: crate::presentation::companion_scene::runtime::CompanionSceneRuntimeState,
    ) -> Self {
        Self {
            runtime,
            cpu_candidate: None,
            ready_candidate: None,
            active: None,
            #[cfg(test)]
            gpu_materializations: 0,
        }
    }

    fn invalidate_resources(
        &mut self,
        invalidation: crate::presentation::companion_scene::runtime::ResourceInvalidation,
    ) -> Result<
        crate::presentation::companion_scene::runtime::RuntimeEffects,
        crate::presentation::companion_scene::runtime::RuntimeError,
    > {
        self.runtime.invalidate_resources(invalidation)
    }

    fn reconcile_snapshot(
        &mut self,
        snapshot: std::sync::Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
        backing_scale_changed: bool,
    ) -> Result<
        crate::presentation::companion_scene::runtime::RuntimeEffects,
        RetainedFailureCategory,
    > {
        let prepared = self
            .runtime
            .prepare_snapshot_with_resource_invalidation(
                snapshot,
                backing_scale_changed.then_some(
                    crate::presentation::companion_scene::runtime::ResourceInvalidation::BackingScaleAtlas,
                ),
            )
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        self.runtime
            .commit_prepared(prepared)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)
    }

    fn reconcile_frame_projection(
        &mut self,
        mut projection: crate::presentation::companion_scene::CompanionFrameProjection,
        backing_scale_changed: bool,
    ) -> Result<
        (
            crate::presentation::companion_scene::runtime::RuntimeEffects,
            bool,
        ),
        RetainedFailureCategory,
    > {
        let mut regenerated = false;
        let invalidation = backing_scale_changed.then_some(
            crate::presentation::companion_scene::runtime::ResourceInvalidation::BackingScaleAtlas,
        );
        let prepared = match self
            .runtime
            .prepare_frame_projection_with_resource_invalidation(projection.clone(), invalidation)
        {
            Ok(prepared) => prepared,
            Err(
                crate::presentation::companion_scene::runtime::RuntimeError::StaleSemanticBase {
                    ..
                },
            ) => {
                projection = self
                    .project_frame(projection.clock, projection.options)
                    .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
                regenerated = true;
                self.runtime
                    .prepare_frame_projection_with_resource_invalidation(projection, invalidation)
                    .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?
            }
            Err(_) => return Err(RetainedFailureCategory::SceneCandidateEncode),
        };
        let effects = self
            .runtime
            .commit_frame_projection(prepared)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        Ok((effects, regenerated))
    }

    fn project_frame(
        &self,
        clock: crate::presentation::companion_scene::CompanionProjectionClock,
        options: crate::presentation::companion_scene::input::CompanionPresentationOptions,
    ) -> Result<
        crate::presentation::companion_scene::CompanionFrameProjection,
        crate::presentation::companion_scene::CompanionSceneProjectionError,
    > {
        let revisions = self.runtime.applied_revisions();
        self.runtime
            .snapshot()
            .project_presentation_frame(revisions.semantic, clock, options)
    }

    fn set_hidden(&mut self) -> crate::presentation::companion_scene::runtime::RuntimeEffects {
        self.runtime.set_hidden()
    }

    fn coalesce_hidden_snapshot(
        &mut self,
        snapshot: std::sync::Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    ) -> Result<
        crate::presentation::companion_scene::runtime::RuntimeEffects,
        crate::presentation::companion_scene::runtime::RuntimeError,
    > {
        self.runtime.coalesce_hidden_snapshot(snapshot)
    }

    fn reveal(
        &mut self,
        backing_scale_changed: bool,
    ) -> Result<
        crate::presentation::companion_scene::runtime::RuntimeEffects,
        RetainedFailureCategory,
    > {
        let prepared = self
            .runtime
            .prepare_reveal_with_resource_invalidation(
                backing_scale_changed.then_some(
                    crate::presentation::companion_scene::runtime::ResourceInvalidation::BackingScaleAtlas,
                ),
            )
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
        self.runtime
            .commit_reveal(prepared)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)
    }

    fn rebind_surface(
        &mut self,
        surface: crate::presentation::companion_scene::SurfaceEpoch,
    ) -> Result<
        crate::presentation::companion_scene::runtime::RuntimeEffects,
        RetainedFailureCategory,
    > {
        self.runtime
            .acknowledge_operational_surface_rebound_to(surface)
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)
    }

    fn retry_current_generation(
        &mut self,
    ) -> Result<
        crate::presentation::companion_scene::runtime::RuntimeEffects,
        RetainedFailureCategory,
    > {
        self.runtime
            .retry_current_generation()
            .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)
    }

    fn shutdown(&mut self) -> crate::presentation::companion_scene::runtime::RuntimeEffects {
        self.runtime.shutdown()
    }

    fn accept_worker_candidate(
        &mut self,
        candidate: Box<CpuSceneBuildCandidate>,
    ) -> crate::presentation::companion_scene::runtime::RuntimeEffects {
        let CpuSceneBuildCandidate {
            identity,
            accepted,
            backing_scale,
            cpu,
            atlas,
            timing: _,
        } = *candidate;
        let current = self.runtime.pending_request_identity() == Some(identity);
        let effects = self.runtime.complete_candidate(accepted);
        if current
            && effects.disposition()
                == crate::presentation::companion_scene::runtime::RuntimeDisposition::CandidateReady(
                    identity.request_id(),
                )
        {
            self.cpu_candidate = Some(Box::new(CpuReadySceneCandidate {
                identity,
                backing_scale,
                cpu,
                atlas,
            }));
        }
        effects
    }

    fn pending_identity(
        &self,
    ) -> Option<crate::presentation::companion_scene::runtime::RequestIdentity> {
        self.runtime.pending_request_identity()
    }

    fn replacement_identity(&self) -> Option<SceneReplacementIdentity> {
        let identity = self.runtime.pending_request_identity()?;
        Some(SceneReplacementIdentity {
            generation: identity.key(),
            surface: identity.surface(),
            source: self
                .runtime
                .pending_desired_source()
                .unwrap_or_else(|| identity.source()),
        })
    }

    fn materialize_ready_candidate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &render::SceneGpuShared,
    ) -> Result<(), SceneCandidatePreparationError> {
        let candidate = self
            .cpu_candidate
            .take()
            .ok_or(SceneCandidatePreparationError::MissingCpuCandidate)?;
        let CpuReadySceneCandidate { identity, backing_scale, mut cpu, atlas } = *candidate;
        if self.runtime.pending_request_identity() != Some(identity) {
            return Err(SceneCandidatePreparationError::StaleCandidate);
        }
        let rebase = self
            .runtime
            .rebase_ready_candidate()
            .map_err(SceneCandidatePreparationError::Rebase)?;
        if rebase.identity() != identity {
            return Err(SceneCandidatePreparationError::StaleCandidate);
        }
        let prepared = cpu
            .prepare_deltas(rebase.content(), rebase.frame())
            .map_err(SceneCandidatePreparationError::CpuDelta)?;
        cpu.commit_prepared(prepared);
        let upload = render::prepare_scene_upload(&cpu, &atlas)
            .map_err(SceneCandidatePreparationError::Upload)?;
        let gpu = render::materialize_gpu_candidate(device, queue, shared, &upload, &atlas)
            .map_err(SceneCandidatePreparationError::Gpu)?;
        self.ready_candidate = Some(ReadyGpuCandidate {
            identity,
            version: rebase.version(),
            backing_scale,
            cpu,
            gpu,
            atlas,
        });
        #[cfg(test)]
        {
            self.gpu_materializations = self.gpu_materializations.saturating_add(1);
        }
        Ok(())
    }

    fn rebase_materialized_candidate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &render::SceneGpuShared,
    ) -> Result<(), SceneCandidatePreparationError> {
        let mut candidate = self
            .ready_candidate
            .take()
            .ok_or(SceneCandidatePreparationError::MissingCpuCandidate)?;
        if self.runtime.pending_request_identity() != Some(candidate.identity) {
            return Err(SceneCandidatePreparationError::StaleCandidate);
        }
        let rebase = self
            .runtime
            .rebase_ready_candidate()
            .map_err(SceneCandidatePreparationError::Rebase)?;
        if rebase.identity() != candidate.identity {
            return Err(SceneCandidatePreparationError::StaleCandidate);
        }
        let prepared = candidate
            .cpu
            .prepare_deltas(rebase.content(), rebase.frame())
            .map_err(SceneCandidatePreparationError::CpuDelta)?;
        candidate.cpu.commit_prepared(prepared);
        let upload = render::prepare_scene_upload(&candidate.cpu, &candidate.atlas)
            .map_err(SceneCandidatePreparationError::Upload)?;
        candidate.gpu =
            render::materialize_gpu_candidate(device, queue, shared, &upload, &candidate.atlas)
                .map_err(SceneCandidatePreparationError::Gpu)?;
        candidate.version = rebase.version();
        self.ready_candidate = Some(candidate);
        #[cfg(test)]
        {
            self.gpu_materializations = self.gpu_materializations.saturating_add(1);
        }
        Ok(())
    }

    fn begin_activation(
        &mut self,
    ) -> Result<
        crate::presentation::companion_scene::runtime::ActivationAttempt,
        crate::presentation::companion_scene::runtime::ActivationStartError,
    > {
        use crate::presentation::companion_scene::runtime::ActivationStartError;

        if self.ready_candidate.is_none() {
            return Err(ActivationStartError::NoReadyCandidate);
        }
        self.runtime.begin_activation()
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_active_to_surface(
        &mut self,
        renderer: &mut render::SceneRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &render::SceneGpuShared,
        request_extent: [u32; 2],
        backing_scale: f64,
        prepared_hud: &hud::SensitivePreparedHudFrame,
        surface_view: &wgpu::TextureView,
    ) -> Result<
        (
            crate::presentation::companion_scene::SceneVersion,
            Option<compiler::SceneDirtyMetrics>,
            u64,
            render::SceneSurfaceTimings,
        ),
        render::SceneRenderError,
    > {
        let Self { runtime, active, .. } = self;
        let lease = runtime.capture_lease().map_err(|_| {
            render::SceneRenderError::Delta(render::SceneDeltaRenderError::RevisionMismatch)
        })?;
        let version = lease.version();
        let active = active.as_mut().ok_or({
            render::SceneRenderError::Delta(render::SceneDeltaRenderError::RevisionMismatch)
        })?;
        if active.version.generation != version.generation {
            return Err(render::SceneRenderError::Delta(
                render::SceneDeltaRenderError::GenerationMismatch,
            ));
        }
        let request = render::SceneRenderRequest::new(version, request_extent, backing_scale);
        let draw_count = active
            .gpu
            .submitted_draw_count(prepared_hud.statistics_interaction().enabled());
        let (dirty_metrics, timings) = if active.version.applied == version.applied {
            let (_, timings) = renderer.submit_active_to_surface(
                device,
                queue,
                shared,
                &mut active.gpu,
                request,
                prepared_hud,
                surface_view,
            )?;
            (None, timings)
        } else {
            let (_, dirty_metrics, timings) = renderer.submit_active_to_surface_with_delta(
                device,
                queue,
                shared,
                &mut active.cpu,
                &mut active.gpu,
                lease.content_delta(),
                lease.frame_delta(),
                request,
                prepared_hud,
                surface_view,
            )?;
            (Some(dirty_metrics), timings)
        };
        active.version = version;
        Ok((version, dirty_metrics, draw_count, timings))
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_active_offscreen(
        &mut self,
        renderer: &mut render::SceneRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &render::SceneGpuShared,
        request_extent: [u32; 2],
        backing_scale: f64,
        prepared_hud: &hud::SensitivePreparedHudFrame,
    ) -> Result<
        (
            crate::presentation::companion_scene::SceneVersion,
            Option<compiler::SceneDirtyMetrics>,
            u64,
            render::SceneSurfaceTimings,
            wgpu::SubmissionIndex,
        ),
        render::SceneRenderError,
    > {
        let Self { runtime, active, .. } = self;
        let lease = runtime.capture_lease().map_err(|_| {
            render::SceneRenderError::Delta(render::SceneDeltaRenderError::RevisionMismatch)
        })?;
        let version = lease.version();
        let active = active.as_mut().ok_or({
            render::SceneRenderError::Delta(render::SceneDeltaRenderError::RevisionMismatch)
        })?;
        if active.version.generation != version.generation {
            return Err(render::SceneRenderError::Delta(
                render::SceneDeltaRenderError::GenerationMismatch,
            ));
        }
        let request = render::SceneRenderRequest::new(version, request_extent, backing_scale);
        let draw_count = active
            .gpu
            .submitted_draw_count(prepared_hud.statistics_interaction().enabled());
        let (submission, dirty_metrics, timings) = if active.version.applied == version.applied {
            let (submission, timings) = renderer.submit_active_offscreen(
                device,
                queue,
                shared,
                &mut active.gpu,
                request,
                prepared_hud,
            )?;
            (submission, None, timings)
        } else {
            let (submission, dirty_metrics, timings) = renderer
                .submit_active_offscreen_with_delta(
                    device,
                    queue,
                    shared,
                    &mut active.cpu,
                    &mut active.gpu,
                    lease.content_delta(),
                    lease.frame_delta(),
                    request,
                    prepared_hud,
                )?;
            (submission, Some(dirty_metrics), timings)
        };
        active.version = version;
        Ok((version, dirty_metrics, draw_count, timings, submission))
    }

    fn prewarm_offscreen_readback(
        &mut self,
        renderer: &mut render::SceneRenderer,
        device: &wgpu::Device,
        shared: &render::SceneGpuShared,
        request_extent: [u32; 2],
        backing_scale: f64,
    ) -> Result<(), render::SceneRenderError> {
        let version = self
            .runtime
            .capture_lease()
            .map_err(|_| {
                render::SceneRenderError::Delta(render::SceneDeltaRenderError::RevisionMismatch)
            })?
            .version();
        let active = self.active.as_ref().ok_or({
            render::SceneRenderError::Delta(render::SceneDeltaRenderError::RevisionMismatch)
        })?;
        renderer.prewarm_offscreen_readback(
            device,
            shared,
            render::SceneRenderRequest::new(version, request_extent, backing_scale),
            &active.gpu,
        )
    }

    fn active_version(&self) -> Option<crate::presentation::companion_scene::SceneVersion> {
        self.active.as_ref().map(|active| active.version)
    }

    fn ready_candidate_draw_count(&self, hud_interaction_enabled: bool) -> Option<u64> {
        self.ready_candidate
            .as_ref()
            .map(|candidate| candidate.gpu.submitted_draw_count(hud_interaction_enabled))
    }

    fn active_backing_scale(&self) -> Option<f64> {
        self.active.as_ref().map(|active| active.backing_scale)
    }

    fn active_delta_pending(&self) -> bool {
        should_defer_scene_reveal(self.active_version(), self.runtime.active_version())
    }

    fn active_surface_extent_matches(&self, physical_extent: [u32; 2], backing_scale: f64) -> bool {
        self.active.as_ref().is_some_and(|active| {
            active.backing_scale.to_bits() == backing_scale.to_bits()
                && logical_viewport_matches_surface(
                    active.gpu.logical_viewport_points,
                    physical_extent,
                    backing_scale,
                )
        })
    }

    fn active_present_compatible(
        &self,
        surface: crate::presentation::companion_scene::SurfaceEpoch,
        physical_extent: [u32; 2],
        backing_scale: f64,
    ) -> bool {
        self.active.as_ref().is_some_and(|active| {
            active.version.surface == surface
                && self.active_surface_extent_matches(physical_extent, backing_scale)
        })
    }

    fn metrics_version(&self) -> Option<crate::presentation::companion_scene::SceneVersion> {
        self.active_version()
            .or_else(|| {
                self.ready_candidate
                    .as_ref()
                    .map(|candidate| candidate.version)
            })
            .or_else(|| {
                let identity = self.runtime.pending_request_identity()?;
                Some(crate::presentation::companion_scene::SceneVersion {
                    generation: identity.key(),
                    surface: identity.surface(),
                    applied: self
                        .runtime
                        .pending_desired_source()
                        .unwrap_or_else(|| identity.source()),
                })
            })
    }

    fn prepare_ready_hud(
        &self,
        text: &crate::round::hud::CompanionHudText,
        geometry: hud::HudPreparationGeometry,
    ) -> Result<hud::SensitivePreparedHudFrame, hud::HudPreparationError> {
        let candidate = self
            .ready_candidate
            .as_ref()
            .ok_or(hud::HudPreparationError::ResourceGenerationMismatch)?;
        let sealed = hud::SealedHudFrame::from_live(text)?;
        candidate
            .gpu
            .hud
            .prepared_atlas()
            .prepare_sensitive(&sealed, geometry)
    }

    fn prepare_active_hud(
        &self,
        text: &crate::round::hud::CompanionHudText,
        geometry: hud::HudPreparationGeometry,
    ) -> Result<hud::SensitivePreparedHudFrame, hud::HudPreparationError> {
        let active = self
            .active
            .as_ref()
            .ok_or(hud::HudPreparationError::ResourceGenerationMismatch)?;
        let sealed = hud::SealedHudFrame::from_live(text)?;
        active
            .gpu
            .hud
            .prepared_atlas()
            .prepare_sensitive(&sealed, geometry)
    }

    fn finish_candidate_activation(
        &mut self,
        attempt: crate::presentation::companion_scene::runtime::ActivationAttempt,
        progress: FrameProgress,
        immediate_errors: &GpuErrorMailbox,
    ) -> crate::presentation::companion_scene::runtime::RuntimeEffects {
        use crate::presentation::companion_scene::runtime::{
            ActivationAttemptOutcome, ActivationTransition, EpochFailure, RuntimeDisposition,
        };

        let outcome = activation_outcome(attempt, &progress, immediate_errors);
        let effects = self.runtime.finish_activation(attempt, outcome);
        match effects.disposition() {
            RuntimeDisposition::Activation(ActivationTransition::Committed) => {
                let candidate = self
                    .ready_candidate
                    .take()
                    .expect("committed runtime activation must own a GPU-ready candidate");
                debug_assert_eq!(candidate.identity.request_id(), attempt.request_id());
                debug_assert_eq!(candidate.identity.key(), attempt.key());
                self.active = Some(ActiveSceneGeneration {
                    version: candidate.version,
                    backing_scale: candidate.backing_scale,
                    cpu: candidate.cpu,
                    gpu: candidate.gpu,
                    atlas: candidate.atlas,
                });
            }
            RuntimeDisposition::Activation(ActivationTransition::RetryLater) => {}
            RuntimeDisposition::Activation(
                ActivationTransition::CandidateDestroyedRetainingActive
                | ActivationTransition::DroppedStale,
            ) => {
                self.drop_ready_candidate_for(attempt);
            }
            RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending) => {
                self.drop_ready_candidate_for(attempt);
                if matches!(
                    outcome,
                    ActivationAttemptOutcome::Fatal(
                        EpochFailure::DeviceLost
                            | EpochFailure::Internal
                            | EpochFailure::OutOfMemory
                            | EpochFailure::UncertainPostSubmit
                            | EpochFailure::ImmediateGpuError
                            | EpochFailure::DelayedGpuError
                    )
                ) {
                    self.active = None;
                }
            }
            _ => {}
        }
        effects
    }

    fn observe_delayed_gpu_error(
        &mut self,
        mailbox: &GpuErrorMailbox,
    ) -> Option<crate::presentation::companion_scene::runtime::RuntimeEffects> {
        let device = self.active.as_ref()?.version.generation.device;
        match mailbox.drain_for(device)? {
            RetainedFailureCategory::DeviceOutOfMemory
            | RetainedFailureCategory::DeviceValidation
            | RetainedFailureCategory::DeviceInternal => {
                let effects = self.runtime.observe_delayed_gpu_error(device);
                self.cpu_candidate = None;
                self.ready_candidate = None;
                self.active = None;
                Some(effects)
            }
            _ => None,
        }
    }

    fn reject_materialization_failure(
        &mut self,
        error: &SceneCandidatePreparationError,
    ) -> crate::presentation::companion_scene::runtime::RuntimeEffects {
        use crate::presentation::companion_scene::runtime::{
            ActivationAttemptOutcome, CandidateFailure, EpochFailure,
        };

        let device = self
            .runtime
            .pending_request_identity()
            .map(|identity| identity.key().device)
            .or_else(|| {
                self.active
                    .as_ref()
                    .map(|active| active.version.generation.device)
            })
            .expect("materialization failure belongs to a pending or active device generation");
        let Ok(attempt) = self.runtime.begin_activation() else {
            self.cpu_candidate = None;
            self.ready_candidate = None;
            self.active = None;
            return self.runtime.observe_delayed_gpu_error(device);
        };
        let outcome = match error {
            SceneCandidatePreparationError::Gpu(render::SceneGpuError::Gpu(
                render::ScopedGpuErrorCategory::OutOfMemory,
            )) => ActivationAttemptOutcome::Fatal(EpochFailure::OutOfMemory),
            SceneCandidatePreparationError::Gpu(render::SceneGpuError::Gpu(
                render::ScopedGpuErrorCategory::Internal,
            )) => ActivationAttemptOutcome::Fatal(EpochFailure::Internal),
            SceneCandidatePreparationError::Gpu(_) => {
                ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Resource)
            }
            _ => ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Validation),
        };
        let effects = self.runtime.finish_activation(attempt, outcome);
        self.cpu_candidate = None;
        self.ready_candidate = None;
        if matches!(outcome, ActivationAttemptOutcome::Fatal(_)) {
            self.active = None;
        }
        effects
    }

    fn drop_ready_candidate_for(
        &mut self,
        attempt: crate::presentation::companion_scene::runtime::ActivationAttempt,
    ) {
        if self.ready_candidate.as_ref().is_some_and(|candidate| {
            candidate.identity.request_id() == attempt.request_id()
                && candidate.identity.key() == attempt.key()
        }) {
            self.ready_candidate = None;
        }
    }

    #[cfg(test)]
    fn active_checksum(&self) -> Option<u64> {
        self.active.as_ref().map(|generation| {
            debug_assert_eq!(generation.version.generation, generation.gpu.generation_key);
            debug_assert_eq!(
                generation.cpu.static_checksum,
                generation.gpu.static_checksum
            );
            generation.gpu.static_checksum
        })
    }

    #[cfg(test)]
    const fn gpu_materialization_count(&self) -> u64 {
        self.gpu_materializations
    }

    const fn has_cpu_candidate(&self) -> bool {
        self.cpu_candidate.is_some()
    }

    const fn has_ready_candidate(&self) -> bool {
        self.ready_candidate.is_some()
    }
}

#[allow(dead_code)] // Called by the dormant Task 12 host activation seam until Task 14.
fn activation_outcome(
    attempt: crate::presentation::companion_scene::runtime::ActivationAttempt,
    progress: &FrameProgress,
    immediate_errors: &GpuErrorMailbox,
) -> crate::presentation::companion_scene::runtime::ActivationAttemptOutcome {
    use crate::presentation::companion_scene::runtime::{
        AcquireDeferral, ActivationAttemptOutcome, CandidateFailure,
    };

    if let Some(failure) = immediate_errors.drain_for(attempt.key().device) {
        return failure_activation_outcome(failure);
    }
    match progress.disposition() {
        Some(FrameDisposition::SurfacePresentCalled)
            if progress.observed(FrameMilestone::Prepared)
                && progress.observed(FrameMilestone::Encoded)
                && progress.observed(FrameMilestone::Submitted)
                && progress.observed(FrameMilestone::SurfacePresentCalled) =>
        {
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface() }
        }
        Some(FrameDisposition::Skipped(SkipReason::Outdated)) => {
            ActivationAttemptOutcome::Deferred(AcquireDeferral::OutdatedReconfigured)
        }
        Some(FrameDisposition::Skipped(SkipReason::Timeout)) => {
            ActivationAttemptOutcome::Deferred(AcquireDeferral::Timeout)
        }
        Some(FrameDisposition::Skipped(SkipReason::Occluded)) => {
            ActivationAttemptOutcome::Deferred(AcquireDeferral::Occluded)
        }
        Some(FrameDisposition::Skipped(SkipReason::ResourcePreparation)) => {
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Resource)
        }
        Some(FrameDisposition::Failed(failure)) => failure_activation_outcome(failure),
        Some(FrameDisposition::Captured) | Some(FrameDisposition::SurfacePresentCalled) | None => {
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::PreSubmitEncode)
        }
    }
}

#[allow(dead_code)] // Called by the dormant Task 12 host activation seam until Task 14.
fn failure_activation_outcome(
    failure: RetainedFailureCategory,
) -> crate::presentation::companion_scene::runtime::ActivationAttemptOutcome {
    use crate::presentation::companion_scene::runtime::{
        ActivationAttemptOutcome, CandidateFailure, EpochFailure,
    };

    match failure {
        RetainedFailureCategory::SceneCandidateEncode
        | RetainedFailureCategory::PresentationStalled => {
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::PreSubmitEncode)
        }
        RetainedFailureCategory::AtlasUnavailable
        | RetainedFailureCategory::FontUnavailable
        | RetainedFailureCategory::RasterWorkerUnavailable
        | RetainedFailureCategory::SurfaceUnavailable
        | RetainedFailureCategory::AdapterUnavailable
        | RetainedFailureCategory::DeviceUnavailable
        | RetainedFailureCategory::SurfaceCreate => {
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Resource)
        }
        RetainedFailureCategory::SurfaceLost => {
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost)
        }
        RetainedFailureCategory::SurfaceValidation => {
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceValidation)
        }
        RetainedFailureCategory::DeviceOutOfMemory => {
            ActivationAttemptOutcome::Fatal(EpochFailure::OutOfMemory)
        }
        RetainedFailureCategory::DeviceValidation => {
            ActivationAttemptOutcome::Fatal(EpochFailure::ImmediateGpuError)
        }
        RetainedFailureCategory::DeviceInternal => {
            ActivationAttemptOutcome::Fatal(EpochFailure::Internal)
        }
        RetainedFailureCategory::UnsupportedRaster
        | RetainedFailureCategory::CaptureUnsupportedVariant
        | RetainedFailureCategory::CapturePollTimeout
        | RetainedFailureCategory::CaptureMapFailed
        | RetainedFailureCategory::CaptureBufferTooShort
        | RetainedFailureCategory::LifetimeGpuPoll
        | RetainedFailureCategory::LifetimeRssUnavailable
        | RetainedFailureCategory::LifetimeFramePreparation => {
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Validation)
        }
        #[cfg(feature = "dev-preview")]
        RetainedFailureCategory::CaptureWriteFailed => {
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Validation)
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifetimeAuditEvent {
    Semantic { sample: u64, elapsed_nanos: u64 },
    Presentation { tick: u64, elapsed_nanos: u64 },
}

impl LifetimeAuditEvent {
    const fn elapsed_nanos(self) -> u64 {
        match self {
            Self::Semantic { elapsed_nanos, .. } | Self::Presentation { elapsed_nanos, .. } => {
                elapsed_nanos
            }
        }
    }
}

/// Produces two exact rational schedules over one shared interval. Event times
/// are compared as fractions before converting to integer nanoseconds, so 30 Hz
/// never accumulates truncation drift. A semantic event wins every exact tie.
fn dual_cadence_events(
    semantic_samples: u64,
    presentation_ticks: u64,
    elapsed_nanos: u64,
) -> Vec<LifetimeAuditEvent> {
    let mut events = Vec::with_capacity(
        usize::try_from(semantic_samples.saturating_add(presentation_ticks)).unwrap_or(usize::MAX),
    );
    let mut semantic = 0_u64;
    let mut presentation = 0_u64;
    while semantic < semantic_samples || presentation < presentation_ticks {
        let semantic_first = if semantic == semantic_samples {
            false
        } else if presentation == presentation_ticks {
            true
        } else {
            u128::from(semantic).saturating_mul(u128::from(presentation_ticks))
                <= u128::from(presentation).saturating_mul(u128::from(semantic_samples))
        };
        if semantic_first {
            let elapsed_nanos = rational_elapsed_nanos(semantic, semantic_samples, elapsed_nanos);
            events.push(LifetimeAuditEvent::Semantic { sample: semantic, elapsed_nanos });
            semantic = semantic.saturating_add(1);
        } else {
            let elapsed_nanos =
                rational_elapsed_nanos(presentation, presentation_ticks, elapsed_nanos);
            events.push(LifetimeAuditEvent::Presentation { tick: presentation, elapsed_nanos });
            presentation = presentation.saturating_add(1);
        }
    }
    events
}

fn rational_elapsed_nanos(index: u64, count: u64, elapsed_nanos: u64) -> u64 {
    if count == 0 {
        return 0;
    }
    u64::try_from(u128::from(index).saturating_mul(u128::from(elapsed_nanos)) / u128::from(count))
        .unwrap_or(u64::MAX)
}

#[derive(Debug, Clone, Copy, Default)]
struct LifetimeSemanticObservation {
    snapshot_projected: bool,
    semantic_reconciled: bool,
    stale_mutations: u64,
    stale_rejections: u64,
    stale_regenerations: u64,
    gpu_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct LifetimePresentationObservation {
    semantic_reconciled: bool,
    frame_projected: bool,
    frame_reconciled: bool,
    encoded: bool,
    submitted: bool,
    draw_calls: u64,
    gpu_bytes: u64,
}

trait LifetimeAuditExecutor {
    fn semantic_sample(
        &mut self,
        phase: LifetimeAuditPhase,
        sample: u64,
        now: time::OffsetDateTime,
    ) -> std::result::Result<LifetimeSemanticObservation, RetainedFailureCategory>;

    fn presentation_tick(
        &mut self,
        phase: LifetimeAuditPhase,
        tick: u64,
        now: time::OffsetDateTime,
    ) -> std::result::Result<LifetimePresentationObservation, RetainedFailureCategory>;

    fn poll(&mut self) -> std::result::Result<(), RetainedFailureCategory>;

    fn rss_bytes(&mut self) -> std::result::Result<u64, RetainedFailureCategory>;

    fn work_counters(&self) -> RuntimeWorkCounters;

    fn persistent_resource_creations(&self) -> u64;

    fn static_upload_bytes(&self) -> u64;

    fn offscreen_cache_events(&self) -> (u64, u64);

    fn storage_capacity_signature(&self) -> u64;
}

fn run_lifetime_schedule(
    executor: &mut impl LifetimeAuditExecutor,
    semantic_samples: u64,
    presentation_ticks: u64,
    virtual_elapsed_ms: u64,
) -> std::result::Result<LifetimeAuditSnapshot, RetainedFailureCategory> {
    const SAMPLE_INTERVAL: u64 = 256;
    let base = time::macros::datetime!(2026-06-13 18:00 UTC);
    let virtual_elapsed_nanos = virtual_elapsed_ms.saturating_mul(1_000_000);
    let events = dual_cadence_events(semantic_samples, presentation_ticks, virtual_elapsed_nanos);
    let mut audit = LifetimeAuditSnapshot {
        semantic_samples,
        warmup_semantic_samples: semantic_samples,
        presentation_ticks,
        warmup_presentation_ticks: presentation_ticks,
        semantic_cadence_ms: virtual_elapsed_ms
            .checked_div(semantic_samples)
            .unwrap_or(0),
        presentation_cadence_hz: u32::try_from(
            u128::from(presentation_ticks)
                .saturating_mul(1_000)
                .checked_div(u128::from(virtual_elapsed_ms))
                .unwrap_or(0),
        )
        .unwrap_or(u32::MAX),
        virtual_elapsed_ms,
        ..LifetimeAuditSnapshot::default()
    };

    for phase in [LifetimeAuditPhase::Warmup, LifetimeAuditPhase::Measured] {
        // Each phase runs the same virtual clock from the same production scene
        // state. Sample zero is already the first event at `base`; use that counted
        // event to rewind/reconcile rather than performing an uncounted duplicate
        // sample and submission before the phase baselines.
        let work_start = executor.work_counters();
        let resource_creations_start = executor.persistent_resource_creations();
        let static_upload_start = executor.static_upload_bytes();
        let cache_events_start = executor.offscreen_cache_events();
        let storage_start = executor.storage_capacity_signature();
        let mut phase_gpu_final = 0_u64;
        let mut phase_presentation_ticks = 0_u64;
        for event in &events {
            let now = base
                + time::Duration::nanoseconds(
                    i64::try_from(event.elapsed_nanos()).unwrap_or(i64::MAX),
                );
            match *event {
                LifetimeAuditEvent::Semantic { sample, .. } => {
                    let observation = executor.semantic_sample(phase, sample, now)?;
                    phase_gpu_final = observation.gpu_bytes;
                    if phase == LifetimeAuditPhase::Measured {
                        audit.snapshot_projections = audit
                            .snapshot_projections
                            .saturating_add(u64::from(observation.snapshot_projected));
                        audit.semantic_reconciles = audit
                            .semantic_reconciles
                            .saturating_add(u64::from(observation.semantic_reconciled));
                        audit.stale_mutations = audit
                            .stale_mutations
                            .saturating_add(observation.stale_mutations);
                        audit.stale_rejections = audit
                            .stale_rejections
                            .saturating_add(observation.stale_rejections);
                        audit.stale_regenerations = audit
                            .stale_regenerations
                            .saturating_add(observation.stale_regenerations);
                    }
                }
                LifetimeAuditEvent::Presentation { tick, .. } => {
                    debug_assert_eq!(tick, phase_presentation_ticks);
                    phase_presentation_ticks = phase_presentation_ticks.saturating_add(1);
                    let observation = executor.presentation_tick(phase, tick, now)?;
                    phase_gpu_final = observation.gpu_bytes;
                    if phase == LifetimeAuditPhase::Measured {
                        audit.frame_projections = audit
                            .frame_projections
                            .saturating_add(u64::from(observation.frame_projected));
                        audit.frame_reconciles = audit
                            .frame_reconciles
                            .saturating_add(u64::from(observation.frame_reconciled));
                        audit.semantic_reconciles = audit
                            .semantic_reconciles
                            .saturating_add(u64::from(observation.semantic_reconciled));
                        audit.encoded_ticks = audit
                            .encoded_ticks
                            .saturating_add(u64::from(observation.encoded));
                        audit.submitted_ticks = audit
                            .submitted_ticks
                            .saturating_add(u64::from(observation.submitted));
                        audit.draw_calls = audit.draw_calls.saturating_add(observation.draw_calls);
                    }
                    executor.poll()?;
                    audit.poll_count = audit.poll_count.saturating_add(1);
                    if phase_presentation_ticks.is_multiple_of(SAMPLE_INTERVAL) {
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
                }
            }
            match phase {
                LifetimeAuditPhase::Warmup => {
                    audit.gpu_warmup_peak_bytes = audit.gpu_warmup_peak_bytes.max(phase_gpu_final);
                }
                LifetimeAuditPhase::Measured => {
                    audit.gpu_peak_bytes = audit.gpu_peak_bytes.max(phase_gpu_final);
                }
            }
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
                let cache_events_end = executor.offscreen_cache_events();
                audit.direct_target_prewarmed = cache_events_end.0 > 0;
                audit.direct_readback_prewarmed = cache_events_end.1 > 0;
            }
            LifetimeAuditPhase::Measured => {
                audit.rss_final_bytes = rss_final;
                audit.rss_peak_bytes = audit.rss_peak_bytes.max(rss_final);
                audit.gpu_final_bytes = phase_gpu_final;
                audit.gpu_peak_bytes = audit.gpu_peak_bytes.max(phase_gpu_final);
                audit.work_delta = executor.work_counters().saturating_sub(work_start);
                audit.work_per_second = audit
                    .work_delta
                    .normalized_per_second(presentation_ticks, audit.presentation_cadence_hz);
                audit.post_warmup_resource_creations = executor
                    .persistent_resource_creations()
                    .saturating_sub(resource_creations_start);
                audit.post_warmup_static_upload_bytes = executor
                    .static_upload_bytes()
                    .saturating_sub(static_upload_start);
                let cache_events_end = executor.offscreen_cache_events();
                audit.direct_target_reused = cache_events_end.0 == cache_events_start.0;
                audit.direct_readback_reused = cache_events_end.1 == cache_events_start.1;
                audit.capacity_growth_events =
                    u64::from(executor.storage_capacity_signature() != storage_start);
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
    use super::RuntimeWorkCounters;
    use super::{
        cached_current_failure, create_atlas_bind_group_layout, create_pipelines,
        current_process_rss_bytes, dual_cadence_events, glyph_advance, glyph_ink_rect,
        glyph_run_height, glyph_run_width, logical_viewport_matches_surface,
        persistent_instance_capacity, physical_dimension, push_analytic_arc, resource_failure_tick,
        run_lifetime_schedule, should_defer_scene_reveal, terminal_worker_decision,
        upload_glyph_atlas, CompiledGlyphAtlas, CompiledRetainedResources, FailedGlyphPreparation,
        GlyphAtlasEntry, GlyphKey, GlyphRepertoireManifest, GpuPrimitive, LayerActivationGuard,
        LayerActivationState, LifetimeAuditEvent, LifetimeAuditExecutor, LifetimeAuditPhase,
        LifetimePresentationObservation, LifetimeSemanticObservation, PersistentFrameBuffers,
        Pipelines, PreparedGpuFrame, ResourcePreparationController, ResourcePreparationKey,
        ResourcePreparationTick, RetainedFailureCategory, RetainedResourceCounters,
        RetainedSceneGenerationState, SmoothBlendMode, FIXED_INSTANCE_RING_MIN, GLYPH_FONT_SIZE,
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
    fn active_logical_viewport_matches_only_its_physical_surface_extent() {
        assert!(logical_viewport_matches_surface(
            [360.0, 360.0],
            [720, 720],
            2.0,
        ));
        assert!(!logical_viewport_matches_surface(
            [360.0, 360.0],
            [6_016, 3_800],
            2.0,
        ));
    }

    #[test]
    fn stale_scene_worker_reply_is_rejected_before_gpu_materialization() {
        use super::worker::{SceneBuildReply, SceneBuildWorker};
        use crate::presentation::companion_scene::runtime::{
            CompanionSceneRuntimeState, ResourceInvalidation,
        };
        use std::time::{Duration, Instant};

        let snapshot =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(0));
        let runtime = CompanionSceneRuntimeState::cold_start(snapshot).unwrap();
        let mut generations = RetainedSceneGenerationState::new(runtime);
        let mut first = generations
            .invalidate_resources(ResourceInvalidation::BackingScaleAtlas)
            .unwrap();
        let old = first.take_start_worker().unwrap();
        let old_id = old.request_id();
        let mut worker = SceneBuildWorker::launch().unwrap();
        worker.try_submit_scene(old, 2.0).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let old = loop {
            match worker.try_recv_build().unwrap() {
                Some(SceneBuildReply::SceneCompleted(candidate)) => break candidate,
                Some(_) => panic!("old scene request did not complete"),
                None if Instant::now() < deadline => std::thread::yield_now(),
                None => panic!("old scene request timed out"),
            }
        };

        let mut superseding = generations
            .invalidate_resources(ResourceInvalidation::MaterialContract)
            .unwrap();
        assert_eq!(
            superseding.take_cancel_worker().unwrap().request_id(),
            old_id
        );
        let mut stale = generations.accept_worker_candidate(old);
        assert_eq!(generations.gpu_materialization_count(), 0);
        assert!(!generations.has_cpu_candidate());
        let newest = stale
            .take_start_worker()
            .expect("stale completion releases the coalesced newest request");
        worker.try_submit_scene(newest, 2.0).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let newest = loop {
            match worker.try_recv_build().unwrap() {
                Some(SceneBuildReply::SceneCompleted(candidate)) => break candidate,
                Some(_) => panic!("newest scene request did not complete"),
                None if Instant::now() < deadline => std::thread::yield_now(),
                None => panic!("newest scene request timed out"),
            }
        };
        generations.accept_worker_candidate(newest);

        let (device, queue) = native_device();
        let shared = super::render::SceneGpuShared::create(
            &device,
            generations.pending_identity().unwrap().key().device,
        )
        .unwrap();
        generations
            .materialize_ready_candidate(&device, &queue, &shared)
            .unwrap();
        assert_eq!(generations.gpu_materialization_count(), 1);
    }

    #[test]
    fn failed_candidate_first_present_retains_external_active_generation() {
        use crate::companion::retained::{
            FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox,
        };
        use crate::presentation::companion_scene::runtime::{
            ActivationTransition, ResourceInvalidation, RuntimeDisposition,
        };

        let (mut generations, device, queue, shared) = ready_scene_generation();
        let first_attempt = generations.begin_activation().unwrap();
        let mut first_present = FrameProgress::new(1, first_attempt.key().resources.0);
        first_present.mark(FrameMilestone::Prepared).unwrap();
        first_present.mark(FrameMilestone::Encoded).unwrap();
        first_present.mark(FrameMilestone::Submitted).unwrap();
        first_present
            .finish(FrameDisposition::SurfacePresentCalled)
            .unwrap();
        let committed = generations.finish_candidate_activation(
            first_attempt,
            first_present,
            &GpuErrorMailbox::new(),
        );
        assert_eq!(
            committed.disposition(),
            RuntimeDisposition::Activation(ActivationTransition::Committed)
        );
        let previous = generations.active_checksum().unwrap();

        let next = generations
            .invalidate_resources(ResourceInvalidation::MaterialContract)
            .unwrap();
        complete_scene_request(&mut generations, next, 2.0);
        generations
            .materialize_ready_candidate(&device, &queue, &shared)
            .unwrap();
        let attempt = generations.begin_activation().unwrap();
        let mut failed = FrameProgress::new(2, attempt.key().resources.0);
        failed.mark(FrameMilestone::Prepared).unwrap();
        failed
            .finish(FrameDisposition::Failed(
                RetainedFailureCategory::SceneCandidateEncode,
            ))
            .unwrap();
        let effects =
            generations.finish_candidate_activation(attempt, failed, &GpuErrorMailbox::new());
        assert_eq!(
            effects.disposition(),
            RuntimeDisposition::Activation(ActivationTransition::CandidateDestroyedRetainingActive)
        );
        assert_eq!(generations.active_checksum(), Some(previous));
    }

    #[test]
    fn active_generation_requires_the_exact_prepared_backing_scale() {
        use crate::companion::retained::{
            FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox,
        };

        let (mut generations, _device, _queue, _shared) = ready_scene_generation();
        let attempt = generations.begin_activation().unwrap();
        let mut presented = FrameProgress::new(1, attempt.key().resources.0);
        presented.mark(FrameMilestone::Prepared).unwrap();
        presented.mark(FrameMilestone::Encoded).unwrap();
        presented.mark(FrameMilestone::Submitted).unwrap();
        presented
            .finish(FrameDisposition::SurfacePresentCalled)
            .unwrap();
        generations.finish_candidate_activation(attempt, presented, &GpuErrorMailbox::new());

        let active = generations.active.as_ref().unwrap();
        let physical_extent = active.gpu.logical_viewport_points.map(|logical| {
            super::host::physical_dimension(f64::from(logical), active.backing_scale)
        });
        assert!(generations.active_present_compatible(
            active.version.surface,
            physical_extent,
            active.backing_scale,
        ));
        assert!(!generations.active_present_compatible(
            active.version.surface,
            physical_extent,
            active.backing_scale + 1.0,
        ));
    }

    #[test]
    fn first_candidate_encode_failure_falls_back_without_external_active() {
        use crate::companion::retained::{
            FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox,
        };
        use crate::presentation::companion_scene::runtime::{
            ActivationTransition, CompanionSceneRuntimeState, ResourceInvalidation,
            RuntimeDisposition,
        };

        let snapshot =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(0));
        let runtime = CompanionSceneRuntimeState::cold_start(snapshot).unwrap();
        let mut generations = RetainedSceneGenerationState::new(runtime);
        let request = generations
            .invalidate_resources(ResourceInvalidation::BackingScaleAtlas)
            .unwrap();
        complete_scene_request(&mut generations, request, 2.0);
        let (device, queue) = native_device();
        let shared = super::render::SceneGpuShared::create(
            &device,
            generations.pending_identity().unwrap().key().device,
        )
        .unwrap();
        generations
            .materialize_ready_candidate(&device, &queue, &shared)
            .unwrap();

        let attempt = generations.begin_activation().unwrap();
        let mut failed = FrameProgress::new(1, attempt.key().resources.0);
        failed.mark(FrameMilestone::Prepared).unwrap();
        failed
            .finish(FrameDisposition::Failed(
                RetainedFailureCategory::SceneCandidateEncode,
            ))
            .unwrap();
        let effects =
            generations.finish_candidate_activation(attempt, failed, &GpuErrorMailbox::new());
        assert_eq!(
            effects.disposition(),
            RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending)
        );
        assert_eq!(generations.active_checksum(), None);
    }

    #[test]
    fn delayed_gpu_error_invalidates_matching_device_epoch_and_falls_back() {
        use crate::companion::retained::{
            FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox,
        };
        use crate::presentation::companion_scene::runtime::{
            ActivationTransition, RuntimeDisposition,
        };

        let (mut generations, _device, _queue, _shared) = ready_scene_generation();
        let attempt = generations.begin_activation().unwrap();
        let mut first_present = FrameProgress::new(1, attempt.key().resources.0);
        first_present.mark(FrameMilestone::Prepared).unwrap();
        first_present.mark(FrameMilestone::Encoded).unwrap();
        first_present.mark(FrameMilestone::Submitted).unwrap();
        first_present
            .finish(FrameDisposition::SurfacePresentCalled)
            .unwrap();
        generations.finish_candidate_activation(attempt, first_present, &GpuErrorMailbox::new());
        assert!(generations.active_checksum().is_some());

        let mailbox = GpuErrorMailbox::new();
        mailbox
            .sender_for(attempt.key().device)
            .send(RetainedFailureCategory::DeviceValidation)
            .unwrap();
        let effects = generations
            .observe_delayed_gpu_error(&mailbox)
            .expect("the matching device fault is observed");
        assert_eq!(
            effects.disposition(),
            RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending)
        );
        assert_eq!(generations.active_checksum(), None);
    }

    #[test]
    fn materialization_failure_rejects_candidate_and_retains_external_active_generation() {
        use crate::companion::retained::{
            FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox,
            SceneCandidatePreparationError,
        };
        use crate::presentation::companion_scene::runtime::{
            ActivationTransition, ResourceInvalidation, RuntimeDisposition,
        };

        let (mut generations, _device, _queue, _shared) = ready_scene_generation();
        let attempt = generations.begin_activation().unwrap();
        let mut presented = FrameProgress::new(1, attempt.key().resources.0);
        presented.mark(FrameMilestone::Prepared).unwrap();
        presented.mark(FrameMilestone::Encoded).unwrap();
        presented.mark(FrameMilestone::Submitted).unwrap();
        presented
            .finish(FrameDisposition::SurfacePresentCalled)
            .unwrap();
        generations.finish_candidate_activation(attempt, presented, &GpuErrorMailbox::new());
        let previous = generations.active_checksum().unwrap();

        let next = generations
            .invalidate_resources(ResourceInvalidation::MaterialContract)
            .unwrap();
        complete_scene_request(&mut generations, next, 2.0);
        let effects =
            generations.reject_materialization_failure(&SceneCandidatePreparationError::Upload(
                super::render::SceneUploadError::MirrorSizeMismatch,
            ));
        assert_eq!(
            effects.disposition(),
            RuntimeDisposition::Activation(ActivationTransition::CandidateDestroyedRetainingActive)
        );
        assert_eq!(generations.active_checksum(), Some(previous));
    }

    #[test]
    fn compatible_update_after_gpu_materialization_rebases_the_ready_candidate() {
        use crate::presentation::companion_scene::runtime::{
            ActivationStartError, RuntimeDisposition,
        };

        let (mut generations, device, queue, shared) = ready_scene_generation();
        let newer =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(1));
        let prepared = generations.runtime.prepare_snapshot(newer).unwrap();
        let mut effects = generations.runtime.commit_prepared(prepared).unwrap();
        assert!(matches!(
            effects.disposition(),
            RuntimeDisposition::SnapshotCommitted(_)
        ));
        assert!(effects.take_start_worker().is_none());
        assert_eq!(
            generations.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );

        generations
            .rebase_materialized_candidate(&device, &queue, &shared)
            .unwrap();
        let desired = generations.runtime.pending_desired_source().unwrap();
        let candidate = generations.ready_candidate.as_ref().unwrap();
        assert_eq!(candidate.version.applied, desired);
        assert_eq!(candidate.cpu.source_revisions, desired);
        assert_eq!(candidate.gpu.source_revisions, desired);
        assert!(generations.begin_activation().is_ok());
    }

    #[test]
    fn stale_frame_projection_is_regenerated_once_against_newest_semantic_base() {
        use crate::presentation::companion_scene::input::CompanionPresentationOptions;
        use crate::presentation::companion_scene::runtime::{
            CompanionSceneRuntimeState, RuntimeDisposition,
        };
        use crate::presentation::companion_scene::{CompanionDayPhase, CompanionProjectionClock};

        let initial =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(0));
        let runtime = CompanionSceneRuntimeState::with_active(initial).unwrap();
        let mut generations = RetainedSceneGenerationState::new(runtime);
        let before = generations.runtime.active_version().unwrap();
        let clock = CompanionProjectionClock::new(time::OffsetDateTime::UNIX_EPOCH, 1_033);
        let stale = generations
            .project_frame(clock, CompanionPresentationOptions::STANDARD)
            .unwrap();

        let mut newest = (**generations.runtime.snapshot()).clone();
        newest.content.day_phase = match newest.content.day_phase {
            CompanionDayPhase::Day => CompanionDayPhase::Dusk,
            _ => CompanionDayPhase::Day,
        };
        let mut semantic = generations
            .reconcile_snapshot(std::sync::Arc::new(newest), false)
            .unwrap();
        assert!(semantic.take_start_worker().is_none());
        let semantic_version = generations.runtime.active_version().unwrap();
        assert_eq!(semantic_version.generation, before.generation);
        assert_ne!(semantic_version.applied.semantic, before.applied.semantic);

        let (mut frame, regenerated) = generations
            .reconcile_frame_projection(stale, false)
            .unwrap();
        assert!(regenerated);
        assert!(frame.take_start_worker().is_none());
        assert!(matches!(
            frame.disposition(),
            RuntimeDisposition::SnapshotCommitted(_) | RuntimeDisposition::Unchanged
        ));
        let after = generations.runtime.active_version().unwrap();
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.applied.semantic, semantic_version.applied.semantic);
        assert_eq!(
            generations.runtime.snapshot().frame.elapsed_ms,
            clock.elapsed_ms
        );
    }

    #[test]
    fn hidden_reveal_retries_external_to_logical_before_committing_latest() {
        use crate::presentation::companion_scene::runtime::CompanionSceneRuntimeState;

        let initial =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(0));
        let mut runtime = CompanionSceneRuntimeState::with_active(initial).unwrap();
        let external = runtime.active_version().unwrap();

        let middle =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(1));
        let prepared = runtime
            .prepare_snapshot(std::sync::Arc::clone(&middle))
            .unwrap();
        runtime.commit_prepared(prepared).unwrap();
        let logical_middle = runtime.active_version().unwrap();
        assert!(should_defer_scene_reveal(
            Some(external),
            Some(logical_middle)
        ));
        let retry = runtime.capture_lease().unwrap();
        assert_eq!(retry.content_delta().from, external.applied);
        assert_eq!(retry.content_delta().to, logical_middle.applied);
        assert_eq!(retry.frame_delta().from, external.applied);
        assert_eq!(retry.frame_delta().to, logical_middle.applied);

        runtime.set_hidden();
        let mut latest = (*middle).clone();
        latest.frame.dimmed = !latest.frame.dimmed;
        latest.frame.dim_amount = if latest.frame.dimmed { 0.35 } else { 0.0 };
        runtime
            .coalesce_hidden_snapshot(std::sync::Arc::new(latest))
            .unwrap();

        assert!(!should_defer_scene_reveal(
            Some(logical_middle),
            runtime.active_version()
        ));
        let prepared = runtime.prepare_reveal().unwrap();
        runtime.commit_reveal(prepared).unwrap();
        let revealed = runtime.capture_lease().unwrap();
        assert_eq!(revealed.content_delta().from, logical_middle.applied);
        assert_eq!(revealed.content_delta().to, revealed.version().applied);
        assert_eq!(revealed.frame_delta().from, logical_middle.applied);
        assert_eq!(revealed.frame_delta().to, revealed.version().applied);
    }

    #[test]
    fn hidden_reduce_motion_change_reveals_one_settled_current_time_frame() {
        use crate::presentation::companion_scene::input::CompanionPresentationOptions;
        use crate::presentation::companion_scene::runtime::CompanionSceneRuntimeState;
        use crate::presentation::companion_scene::CompanionProjectionClock;

        let initial =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(0));
        let mut runtime =
            CompanionSceneRuntimeState::with_active(std::sync::Arc::clone(&initial)).unwrap();
        let semantic = runtime.active_version().unwrap().applied.semantic;
        let reveal_clock = CompanionProjectionClock::new(time::OffsetDateTime::UNIX_EPOCH, 65_000);
        let reduced = initial
            .project_presentation_frame(
                semantic,
                reveal_clock,
                CompanionPresentationOptions { reduce_motion: true },
            )
            .unwrap();
        let mut hidden_latest = (*initial).clone();
        hidden_latest.frame = reduced.frame.clone();

        let mut hidden = runtime.set_hidden();
        assert!(hidden.take_start_worker().is_none());
        let mut coalesced = runtime
            .coalesce_hidden_snapshot(std::sync::Arc::new(hidden_latest))
            .unwrap();
        assert!(coalesced.take_start_worker().is_none());
        let prepared = runtime.prepare_reveal().unwrap();
        let mut revealed = runtime.commit_reveal(prepared).unwrap();
        assert!(revealed.take_start_worker().is_none());

        let frame = &runtime.snapshot().frame;
        assert_eq!(*frame, reduced.frame);
        assert_eq!(frame.elapsed_ms, reveal_clock.elapsed_ms);
        assert_eq!(frame.bob_offset_y_points, 0.0);
        assert!(frame
            .prop_instances
            .iter()
            .all(|prop| prop.transition.is_none()));
        assert!(frame.tank_instances.iter().all(|tank| tank
            .cells
            .iter()
            .all(|cell| cell.position_points == cell.target_position_points)));
    }

    fn ready_scene_generation() -> (
        RetainedSceneGenerationState,
        wgpu::Device,
        wgpu::Queue,
        super::render::SceneGpuShared,
    ) {
        use crate::presentation::companion_scene::runtime::{
            CompanionSceneRuntimeState, ResourceInvalidation,
        };
        let snapshot =
            std::sync::Arc::new(super::compiler::projected_full_scene_snapshot_for_render_test(0));
        let runtime = CompanionSceneRuntimeState::cold_start(snapshot).unwrap();
        let mut generations = RetainedSceneGenerationState::new(runtime);
        let request = generations
            .invalidate_resources(ResourceInvalidation::BackingScaleAtlas)
            .unwrap();
        complete_scene_request(&mut generations, request, 2.0);
        let (device, queue) = native_device();
        let shared = super::render::SceneGpuShared::create(
            &device,
            generations.pending_identity().unwrap().key().device,
        )
        .unwrap();
        generations
            .materialize_ready_candidate(&device, &queue, &shared)
            .unwrap();
        (generations, device, queue, shared)
    }

    fn complete_scene_request(
        generations: &mut RetainedSceneGenerationState,
        mut effects: crate::presentation::companion_scene::runtime::RuntimeEffects,
        backing_scale: f64,
    ) {
        use super::worker::{SceneBuildReply, SceneBuildWorker};
        use std::time::{Duration, Instant};
        let request = effects.take_start_worker().unwrap();
        let mut worker = SceneBuildWorker::launch().unwrap();
        worker.try_submit_scene(request, backing_scale).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match worker.try_recv_build().unwrap() {
                Some(SceneBuildReply::SceneCompleted(candidate)) => {
                    generations.accept_worker_candidate(candidate);
                    return;
                }
                Some(_) => panic!("scene request did not complete"),
                None if Instant::now() < deadline => std::thread::yield_now(),
                None => panic!("scene request timed out"),
            }
        }
    }

    fn native_device() -> (wgpu::Device, wgpu::Queue) {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .unwrap();
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glorp-scene-lifecycle-test-device"),
            ..Default::default()
        }))
        .unwrap()
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
        calls: Vec<(LifetimeAuditPhase, LifetimeAuditEvent, i128)>,
        polls: u64,
        rss_calls: u64,
        fail_poll: bool,
        fail_rss: bool,
    }

    impl LifetimeAuditExecutor for FakeLifetimeExecutor {
        fn semantic_sample(
            &mut self,
            phase: LifetimeAuditPhase,
            sample: u64,
            now: time::OffsetDateTime,
        ) -> std::result::Result<LifetimeSemanticObservation, RetainedFailureCategory> {
            let elapsed_nanos = u64::try_from(
                now.unix_timestamp_nanos()
                    - time::macros::datetime!(2026-06-13 18:00 UTC).unix_timestamp_nanos(),
            )
            .unwrap();
            self.calls.push((
                phase,
                LifetimeAuditEvent::Semantic { sample, elapsed_nanos },
                now.unix_timestamp_nanos(),
            ));
            Ok(LifetimeSemanticObservation {
                snapshot_projected: true,
                semantic_reconciled: true,
                gpu_bytes: 1_000,
                ..LifetimeSemanticObservation::default()
            })
        }

        fn presentation_tick(
            &mut self,
            phase: LifetimeAuditPhase,
            tick: u64,
            now: time::OffsetDateTime,
        ) -> std::result::Result<LifetimePresentationObservation, RetainedFailureCategory> {
            let elapsed_nanos = u64::try_from(
                now.unix_timestamp_nanos()
                    - time::macros::datetime!(2026-06-13 18:00 UTC).unix_timestamp_nanos(),
            )
            .unwrap();
            self.calls.push((
                phase,
                LifetimeAuditEvent::Presentation { tick, elapsed_nanos },
                now.unix_timestamp_nanos(),
            ));
            Ok(LifetimePresentationObservation {
                semantic_reconciled: false,
                frame_projected: true,
                frame_reconciled: true,
                encoded: true,
                submitted: true,
                draw_calls: tick + 1,
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

        fn work_counters(&self) -> RuntimeWorkCounters {
            RuntimeWorkCounters {
                prepare: self.calls.len() as u64,
                encode: self
                    .calls
                    .iter()
                    .filter(|(_, event, _)| {
                        matches!(event, LifetimeAuditEvent::Presentation { .. })
                    })
                    .count() as u64,
                submit: self
                    .calls
                    .iter()
                    .filter(|(_, event, _)| {
                        matches!(event, LifetimeAuditEvent::Presentation { .. })
                    })
                    .count() as u64,
                ..RuntimeWorkCounters::default()
            }
        }

        fn persistent_resource_creations(&self) -> u64 {
            8
        }

        fn static_upload_bytes(&self) -> u64 {
            1_024
        }

        fn offscreen_cache_events(&self) -> (u64, u64) {
            (1, 1)
        }

        fn storage_capacity_signature(&self) -> u64 {
            7
        }
    }

    #[test]
    fn dual_cadence_scheduler_orders_semantic_first_on_ties() {
        let events = dual_cadence_events(7, 8, 7_000);
        assert_eq!(events.len(), 15);
        assert_eq!(
            events[0],
            LifetimeAuditEvent::Semantic { sample: 0, elapsed_nanos: 0 }
        );
        assert_eq!(
            events[1],
            LifetimeAuditEvent::Presentation { tick: 0, elapsed_nanos: 0 }
        );
        assert_eq!(
            events.last(),
            Some(&LifetimeAuditEvent::Presentation { tick: 7, elapsed_nanos: 6_125 })
        );
        assert!(events
            .windows(2)
            .all(|pair| pair[0].elapsed_nanos() <= pair[1].elapsed_nanos()));
    }

    #[test]
    fn production_dual_cadence_schedule_has_exact_counts_without_drift() {
        let events = dual_cadence_events(4_500, 33_750, 1_125_000_000_000);
        assert_eq!(events.len(), 38_250);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, LifetimeAuditEvent::Semantic { .. }))
                .count(),
            4_500
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, LifetimeAuditEvent::Presentation { .. }))
                .count(),
            33_750
        );
        assert_eq!(
            events.last(),
            Some(&LifetimeAuditEvent::Presentation {
                tick: 33_749,
                elapsed_nanos: 1_124_966_666_666,
            })
        );
        for semantic in 0_u64..4_500 {
            let elapsed = semantic * 250_000_000;
            let position = events
                .iter()
                .position(|event| {
                    *event
                        == LifetimeAuditEvent::Semantic { sample: semantic, elapsed_nanos: elapsed }
                })
                .unwrap();
            if semantic.is_multiple_of(2) {
                assert!(matches!(
                    events.get(position + 1),
                    Some(LifetimeAuditEvent::Presentation { elapsed_nanos, .. }) if *elapsed_nanos == elapsed
                ));
            }
        }
    }

    #[test]
    fn lifetime_schedule_repeats_identical_warmup_and_measured_virtual_clocks() {
        let mut executor = FakeLifetimeExecutor::default();
        let audit = run_lifetime_schedule(&mut executor, 4, 30, 1_000).unwrap();
        assert_eq!(executor.calls.len(), 68);
        let warmup = &executor.calls[..34];
        let measured = &executor.calls[34..];
        for (warm, measured) in warmup.iter().zip(measured) {
            assert_eq!(warm.1, measured.1);
            assert_eq!(warm.2, measured.2);
        }
        assert_eq!(audit.semantic_samples, 4);
        assert_eq!(audit.warmup_semantic_samples, 4);
        assert_eq!(audit.presentation_ticks, 30);
        assert_eq!(audit.warmup_presentation_ticks, 30);
        assert_eq!(audit.snapshot_projections, 4);
        assert_eq!(audit.semantic_reconciles, 4);
        assert_eq!(audit.frame_projections, 30);
        assert_eq!(audit.frame_reconciles, 30);
        assert_eq!(audit.encoded_ticks, 30);
        assert_eq!(audit.submitted_ticks, 30);
        assert!(audit.draw_calls > 0);
        assert_eq!(audit.virtual_elapsed_ms, 1_000);
        assert_eq!(audit.poll_count, 62);
        assert_eq!(executor.polls, 62);
    }

    #[test]
    fn lifetime_schedule_fails_closed_on_poll_or_rss_errors() {
        let mut poll_failure = FakeLifetimeExecutor { fail_poll: true, ..Default::default() };
        assert_eq!(
            run_lifetime_schedule(&mut poll_failure, 1, 1, 250),
            Err(RetainedFailureCategory::LifetimeGpuPoll)
        );
        let mut rss_failure = FakeLifetimeExecutor { fail_rss: true, ..Default::default() };
        assert_eq!(
            run_lifetime_schedule(&mut rss_failure, 1, 1, 250),
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
            allocated_cell: super::resources::AtlasCell { origin: [0, 0], extent: [80, 80] },
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
        let entry = GlyphAtlasEntry::whitespace(
            28.0,
            52.0,
            super::resources::AtlasCell { origin: [0, 0], extent: [80, 80] },
        );

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
