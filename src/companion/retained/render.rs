//! Pure contracts and owned CPU preparation for the retained scene renderer.
#![allow(dead_code)] // This checkpoint validates contracts before live GPU materialization.

use super::buffers::{ByteSpan, DirtySpanSet};
use super::compiler::{
    ContentGpuValue, ContentUploadValue, CpuSceneCandidate, FrameUploadSources, MirrorDeltaError,
    PreparedMirrorFamily, PreparedSceneDelta, PrimitiveUploadSource, SceneDirtySpans,
};
use super::resources::{GlyphEntryKind, GlyphKey, PreparedSceneAtlas};
use bytemuck::{Pod, Zeroable};
use std::ops::Range;
use std::sync::mpsc;
use std::time::Duration;
use wgpu::util::DeviceExt;

const NONE_U32: u32 = u32::MAX;
const SHALLOW_CARD_PRIMITIVE_TAG: u32 = 3;
const ATLAS_QUAD_PRIMITIVE_TAG: u32 = 1;
const ANALYTIC_PRIMITIVE_TAG: u32 = 2;
const INSTANCE_QUAD_PRIMITIVE_TAG: u32 = 4;
const LIT_SHALLOW_CARD_MATERIAL_TAG: u32 = 3;
const SHALLOW_CARD_GEOMETRY_RESOURCE_TAG: u32 = 4;
const PET_EYE_PALETTE_ROLE_TAG: u32 = 3;

pub(super) const GLYPH_FLAG_VISIBLE: u32 = 1 << 0;
pub(super) const GLYPH_FLAG_COLOR: u32 = 1 << 1;
pub(super) const SCENE_SHADER_SOURCE: &str = include_str!("scene.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub(super) struct PrimitiveGpuValue {
    pub(super) node_index: u32,
    pub(super) material_index: u32,
    pub(super) aux_node_index: u32,
    pub(super) primitive_kind: u32,
    pub(super) material_kind: u32,
    pub(super) resource_kind: u32,
    pub(super) blend: u32,
    pub(super) depth: u32,
    pub(super) space: u32,
    pub(super) instance_group: u32,
    pub(super) instance_base: u32,
    pub(super) binding_index: u32,
    pub(super) authored_order: u32,
    pub(super) content_base: u32,
    pub(super) frame_base: u32,
    pub(super) aux_content_base: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub(super) struct SceneContentGpuValue {
    pub(super) kind: u32,
    pub(super) glyph_entry_index: u32,
    pub(super) slot: u32,
    pub(super) subslot: u32,
    pub(super) signed_data: [i32; 2],
    pub(super) flags: u32,
    pub(super) variant: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(super) struct GlyphAtlasGpuEntry {
    pub(super) visible_uv: [f32; 4],
    /// Horizontal bearing, precomputed Y-up bottom bearing, width, and height.
    pub(super) ink_origin_size: [f32; 4],
    /// Advance, line height, and baseline in raster pixels.
    pub(super) metrics: [f32; 3],
    pub(super) flags: u32,
    /// Integer `[origin_x, origin_y, width, height]` of the allocated cell.
    pub(super) allocated_cell: [u32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstanceSource {
    PetBody,
    PetParticles,
    RoomGlyphs,
    PropGlyphs {
        slot: u32,
    },
    TankCells {
        slot: u32,
        layer: crate::presentation::companion_scene::scene::InstanceLayer,
    },
    Ambient,
    WallShadowGlyphMask,
    Hud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PrimitiveSource {
    None,
    StaticAtlas,
    Instances(InstanceSource),
    Analytic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SceneDrawRecord {
    pub(super) index_range: Range<u32>,
    pub(super) instance_range: Range<u32>,
    pub(super) source: PrimitiveSource,
    pub(super) authored_order: u32,
}

/// Closed draw-to-pipeline classification for the companion scene v2 ABI.
/// Every axis is checked because a plausible fallback can silently turn a
/// multiply mask into ordinary color, or route private HUD geometry through a
/// scene draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScenePipelineClass {
    WorldOpaqueAnalytic,
    WorldSourceOverAnalytic,
    WorldSourceOverGlyph,
    WorldMultiplyAnalytic,
    WorldMultiplyGlyphMask,
    WorldAdditiveGlyph,
    /// Materialized to keep the pipeline family complete. Scene v2 authors no
    /// additive analytic primitive, so the selector never returns this class.
    WorldAdditiveAnalyticReserved,
    ChromeAnalytic,
    SealedHudHook,
}

fn scene_pipeline_class(
    primitive: PrimitiveGpuValue,
    draw: &SceneDrawRecord,
) -> Option<ScenePipelineClass> {
    use ScenePipelineClass::*;

    let (expected_source, expected_instance_count) = expected_draw_source(primitive)?;
    if draw.source != expected_source || draw.instance_range != (0..expected_instance_count) {
        return None;
    }

    let analytic_axes = primitive.primitive_kind == ANALYTIC_PRIMITIVE_TAG
        && primitive.resource_kind == 3
        && primitive.instance_group == 0
        && primitive.instance_base == NONE_U32;
    if analytic_axes
        && primitive.material_kind == 2
        && primitive.blend == 1
        && primitive.depth == 1
        && primitive.space == 1
        && primitive.binding_index == 0
        && draw.source == PrimitiveSource::Analytic
    {
        return Some(WorldOpaqueAnalytic);
    }
    // Wall shadow is intentionally checked before ordinary analytics. Its
    // primitive kind is analytic, but its draw source is the pet glyph mask.
    if analytic_axes
        && primitive.material_kind == 4
        && primitive.blend == 4
        && primitive.depth == 2
        && primitive.space == 1
        && primitive.binding_index == 1
        && draw.source == PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask)
    {
        return Some(WorldMultiplyGlyphMask);
    }
    if analytic_axes
        && primitive.material_kind == 4
        && primitive.blend == 4
        && primitive.depth == 2
        && primitive.space == 1
        && primitive.binding_index == 2
        && draw.source == PrimitiveSource::Analytic
    {
        return Some(WorldMultiplyAnalytic);
    }
    if analytic_axes
        && primitive.material_kind == 2
        && primitive.blend == 3
        && primitive.depth == 2
        && primitive.space == 1
        && primitive.binding_index == 4
        && draw.source == PrimitiveSource::Analytic
    {
        return Some(WorldSourceOverAnalytic);
    }
    if analytic_axes
        && primitive.material_kind == 6
        && primitive.blend == 3
        && primitive.depth == 3
        && primitive.space == 2
        && matches!(primitive.binding_index, 3 | 5 | 6 | 7)
        && draw.source == PrimitiveSource::Analytic
    {
        return Some(ChromeAnalytic);
    }

    let glyph_resource = matches!(primitive.resource_kind, 1 | 2);
    let source_over_glyph = match draw.source {
        PrimitiveSource::Instances(InstanceSource::PetBody) => {
            primitive.primitive_kind == INSTANCE_QUAD_PRIMITIVE_TAG
                && primitive.instance_group == 1
                && primitive.binding_index == 0
        }
        PrimitiveSource::Instances(InstanceSource::RoomGlyphs) => {
            primitive.primitive_kind == INSTANCE_QUAD_PRIMITIVE_TAG
                && primitive.instance_group == 4
                && primitive.binding_index == 0
        }
        PrimitiveSource::Instances(InstanceSource::PropGlyphs { slot }) => {
            primitive.primitive_kind == INSTANCE_QUAD_PRIMITIVE_TAG
                && primitive.instance_group == 3
                && primitive.binding_index == slot
        }
        PrimitiveSource::Instances(InstanceSource::TankCells { slot, layer }) => {
            let expected_group = match layer {
                crate::presentation::companion_scene::scene::InstanceLayer::Behind => 5,
                crate::presentation::companion_scene::scene::InstanceLayer::Foreground => 6,
            };
            primitive.primitive_kind == INSTANCE_QUAD_PRIMITIVE_TAG
                && primitive.instance_group == expected_group
                && primitive.binding_index == slot
        }
        _ => false,
    };
    if source_over_glyph
        && glyph_resource
        && primitive.material_kind == 1
        && primitive.blend == 3
        && primitive.depth == 2
        && primitive.space == 1
    {
        return Some(WorldSourceOverGlyph);
    }
    let additive_source_matches = matches!(
        draw.source,
        PrimitiveSource::Instances(InstanceSource::PetParticles)
            if primitive.instance_group == 2 && primitive.binding_index == 0
    ) || matches!(
        draw.source,
        PrimitiveSource::Instances(InstanceSource::Ambient)
            if primitive.instance_group == 7 && primitive.binding_index == 0
    );
    if primitive.primitive_kind == INSTANCE_QUAD_PRIMITIVE_TAG
        && glyph_resource
        && primitive.material_kind == 5
        && primitive.blend == 5
        && primitive.depth == 2
        && primitive.space == 1
        && additive_source_matches
    {
        return Some(WorldAdditiveGlyph);
    }
    if primitive.primitive_kind == INSTANCE_QUAD_PRIMITIVE_TAG
        && glyph_resource
        && primitive.material_kind == 6
        && primitive.blend == 3
        && primitive.depth == 3
        && primitive.space == 2
        && primitive.instance_group == 8
        && primitive.binding_index == 0
        && draw.source == PrimitiveSource::Instances(InstanceSource::Hud)
    {
        return Some(SealedHudHook);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneBlendContract {
    SourceOver,
    Multiply,
    Additive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScenePipelineContract {
    vertex_entry: &'static str,
    fragment_entry: &'static str,
    blend: Option<SceneBlendContract>,
    depth_write_enabled: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ApertureCompositePipelineContract {
    pipeline: ScenePipelineContract,
    scene_storage_group: u8,
    sampled_raw_group: u8,
    target_format: wgpu::TextureFormat,
}

const APERTURE_COMPOSITE_PIPELINE_CONTRACT: ApertureCompositePipelineContract =
    ApertureCompositePipelineContract {
        pipeline: ScenePipelineContract {
            vertex_entry: "vs_final",
            fragment_entry: "fs_aperture_composite",
            blend: None,
            depth_write_enabled: None,
        },
        scene_storage_group: 0,
        sampled_raw_group: 2,
        target_format: SceneTextureContract::INTERMEDIATE,
    };

const fn scene_pipeline_contract(class: ScenePipelineClass) -> ScenePipelineContract {
    use ScenePipelineClass::*;
    match class {
        WorldOpaqueAnalytic => ScenePipelineContract {
            vertex_entry: "vs_world_analytic",
            fragment_entry: "fs_analytic",
            blend: None,
            depth_write_enabled: Some(true),
        },
        WorldSourceOverAnalytic => ScenePipelineContract {
            vertex_entry: "vs_world_analytic",
            fragment_entry: "fs_analytic",
            blend: Some(SceneBlendContract::SourceOver),
            depth_write_enabled: Some(false),
        },
        WorldSourceOverGlyph => ScenePipelineContract {
            vertex_entry: "vs_world_glyph",
            fragment_entry: "fs_glyph",
            blend: Some(SceneBlendContract::SourceOver),
            depth_write_enabled: Some(false),
        },
        WorldMultiplyAnalytic => ScenePipelineContract {
            vertex_entry: "vs_world_analytic",
            fragment_entry: "fs_analytic",
            blend: Some(SceneBlendContract::Multiply),
            depth_write_enabled: Some(false),
        },
        WorldMultiplyGlyphMask => ScenePipelineContract {
            vertex_entry: "vs_world_glyph",
            fragment_entry: "fs_wall_shadow_glyph",
            blend: Some(SceneBlendContract::Multiply),
            depth_write_enabled: Some(false),
        },
        WorldAdditiveGlyph => ScenePipelineContract {
            vertex_entry: "vs_world_glyph",
            fragment_entry: "fs_glyph",
            blend: Some(SceneBlendContract::Additive),
            depth_write_enabled: Some(false),
        },
        WorldAdditiveAnalyticReserved => ScenePipelineContract {
            vertex_entry: "vs_world_analytic",
            fragment_entry: "fs_analytic",
            blend: Some(SceneBlendContract::Additive),
            depth_write_enabled: Some(false),
        },
        ChromeAnalytic => ScenePipelineContract {
            vertex_entry: "vs_screen_analytic",
            fragment_entry: "fs_analytic",
            blend: Some(SceneBlendContract::SourceOver),
            depth_write_enabled: None,
        },
        SealedHudHook => ScenePipelineContract {
            vertex_entry: "vs_hud",
            fragment_entry: "fs_hud",
            blend: Some(SceneBlendContract::SourceOver),
            depth_write_enabled: None,
        },
    }
}

const fn scene_blend_state(contract: SceneBlendContract) -> wgpu::BlendState {
    let alpha = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    };
    let color = match contract {
        SceneBlendContract::SourceOver => alpha,
        SceneBlendContract::Multiply => wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Dst,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        SceneBlendContract::Additive => wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    };
    wgpu::BlendState { color, alpha }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ScenePhaseTable {
    pub(super) opaque_cutout: Vec<u32>,
    pub(super) world_blended_unsorted: Vec<u32>,
    pub(super) chrome_authored: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ScenePlannedDraw {
    pub(super) primitive_index: u32,
    pub(super) pipeline: ScenePipelineClass,
    pub(super) index_range: Range<u32>,
    pub(super) instance_range: Range<u32>,
    pub(super) authored_order: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneHudMarker {
    pub(super) primitive_index: u32,
    pub(super) authored_order: u32,
}

/// Fixed scene-v2 screen-space schedule. The sealed HUD marker is deliberately
/// not a [`ScenePlannedDraw`], so no general scene encoder can accidentally
/// submit private HUD instances through the ordinary storage-buffer path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SceneChromePlan {
    /// Gauges, status, trouble.
    pub(super) prefix: [ScenePlannedDraw; 3],
    pub(super) hud: SceneHudMarker,
    /// Dim overlay.
    pub(super) suffix: [ScenePlannedDraw; 1],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SceneDrawPlan {
    pub(super) opaque: Vec<ScenePlannedDraw>,
    /// Task 11 replaces this compiler order with one camera-depth stream.
    pub(super) world_blended_unsorted: Vec<ScenePlannedDraw>,
    pub(super) chrome: SceneChromePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneDrawPlanError {
    MetadataLengthMismatch,
    PrimitiveIndexOutOfBounds,
    DuplicatePrimitive,
    MissingPrimitive,
    AuthoredOrderMismatch,
    InvalidPipelineClass,
    InvalidPhaseClass,
    InvalidChromeSchedule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneDrawPhase {
    Opaque,
    WorldBlended,
    Chrome,
}

/// Converts the complete CPU-side candidate metadata into a closed render
/// schedule before any command encoder is allowed to begin a render pass.
pub(super) fn validate_scene_draw_plan(
    primitives: &[PrimitiveGpuValue],
    draws: &[SceneDrawRecord],
    phases: &ScenePhaseTable,
) -> Result<SceneDrawPlan, SceneDrawPlanError> {
    const CHROME_PREFIX_BINDINGS: [u32; 3] = [5, 3, 6];
    const CHROME_SUFFIX_BINDINGS: [u32; 1] = [7];
    const CHROME_DRAW_COUNT: usize =
        CHROME_PREFIX_BINDINGS.len() + 1 + CHROME_SUFFIX_BINDINGS.len();

    if primitives.len() != draws.len() {
        return Err(SceneDrawPlanError::MetadataLengthMismatch);
    }
    if phases.chrome_authored.len() != CHROME_DRAW_COUNT {
        return Err(SceneDrawPlanError::InvalidChromeSchedule);
    }

    let mut seen = vec![false; primitives.len()];
    let mut opaque = Vec::with_capacity(phases.opaque_cutout.len());
    for primitive_index in phases.opaque_cutout.iter().copied() {
        opaque.push(validate_planned_draw(
            primitives,
            draws,
            &mut seen,
            primitive_index,
            SceneDrawPhase::Opaque,
        )?);
    }
    let mut world_blended_unsorted = Vec::with_capacity(phases.world_blended_unsorted.len());
    for primitive_index in phases.world_blended_unsorted.iter().copied() {
        world_blended_unsorted.push(validate_planned_draw(
            primitives,
            draws,
            &mut seen,
            primitive_index,
            SceneDrawPhase::WorldBlended,
        )?);
    }

    let mut prefix: [Option<ScenePlannedDraw>; CHROME_PREFIX_BINDINGS.len()] =
        std::array::from_fn(|_| None);
    let mut hud = None;
    let mut suffix: [Option<ScenePlannedDraw>; CHROME_SUFFIX_BINDINGS.len()] =
        std::array::from_fn(|_| None);
    let mut previous_authored_order: Option<u32> = None;
    for (position, primitive_index) in phases.chrome_authored.iter().copied().enumerate() {
        let planned = validate_planned_draw(
            primitives,
            draws,
            &mut seen,
            primitive_index,
            SceneDrawPhase::Chrome,
        )?;
        let primitive = primitives[primitive_index as usize];
        if let Some(previous) = previous_authored_order {
            if previous.checked_add(1) != Some(primitive.authored_order) {
                return Err(SceneDrawPlanError::InvalidChromeSchedule);
            }
        }
        previous_authored_order = Some(primitive.authored_order);
        match position {
            0..=2
                if planned.pipeline == ScenePipelineClass::ChromeAnalytic
                    && primitive.binding_index == CHROME_PREFIX_BINDINGS[position] =>
            {
                prefix[position] = Some(planned);
            }
            3 if planned.pipeline == ScenePipelineClass::SealedHudHook => {
                hud = Some(SceneHudMarker {
                    primitive_index,
                    authored_order: primitive.authored_order,
                });
            }
            4 if planned.pipeline == ScenePipelineClass::ChromeAnalytic
                && primitive.binding_index == CHROME_SUFFIX_BINDINGS[0] =>
            {
                suffix[0] = Some(planned);
            }
            _ => return Err(SceneDrawPlanError::InvalidChromeSchedule),
        }
    }
    if seen.contains(&false) {
        return Err(SceneDrawPlanError::MissingPrimitive);
    }

    Ok(SceneDrawPlan {
        opaque,
        world_blended_unsorted,
        chrome: SceneChromePlan {
            prefix: prefix.map(|value| value.expect("validated chrome prefix is complete")),
            hud: hud.ok_or(SceneDrawPlanError::InvalidChromeSchedule)?,
            suffix: suffix.map(|value| value.expect("validated chrome suffix is complete")),
        },
    })
}

fn validate_planned_draw(
    primitives: &[PrimitiveGpuValue],
    draws: &[SceneDrawRecord],
    seen: &mut [bool],
    primitive_index: u32,
    phase: SceneDrawPhase,
) -> Result<ScenePlannedDraw, SceneDrawPlanError> {
    let index = usize::try_from(primitive_index)
        .ok()
        .filter(|index| *index < primitives.len())
        .ok_or(SceneDrawPlanError::PrimitiveIndexOutOfBounds)?;
    if std::mem::replace(&mut seen[index], true) {
        return Err(SceneDrawPlanError::DuplicatePrimitive);
    }
    let primitive = primitives[index];
    let draw = &draws[index];
    if primitive.authored_order != draw.authored_order {
        return Err(SceneDrawPlanError::AuthoredOrderMismatch);
    }
    let pipeline =
        scene_pipeline_class(primitive, draw).ok_or(SceneDrawPlanError::InvalidPipelineClass)?;
    let valid_phase = match phase {
        SceneDrawPhase::Opaque => pipeline == ScenePipelineClass::WorldOpaqueAnalytic,
        SceneDrawPhase::WorldBlended => matches!(
            pipeline,
            ScenePipelineClass::WorldSourceOverAnalytic
                | ScenePipelineClass::WorldSourceOverGlyph
                | ScenePipelineClass::WorldMultiplyAnalytic
                | ScenePipelineClass::WorldMultiplyGlyphMask
                | ScenePipelineClass::WorldAdditiveGlyph
        ),
        SceneDrawPhase::Chrome => matches!(
            pipeline,
            ScenePipelineClass::ChromeAnalytic | ScenePipelineClass::SealedHudHook
        ),
    };
    if !valid_phase {
        return Err(SceneDrawPlanError::InvalidPhaseClass);
    }
    Ok(ScenePlannedDraw {
        primitive_index,
        pipeline,
        index_range: draw.index_range.clone(),
        instance_range: draw.instance_range.clone(),
        authored_order: draw.authored_order,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PreparedSceneUpload {
    pub(super) generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    pub(super) source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    pub(super) logical_viewport_points: [f32; 2],
    pub(super) static_checksum: u64,
    pub(super) vertex_bytes: Vec<u8>,
    pub(super) index_bytes: Vec<u8>,
    pub(super) primitives: Vec<PrimitiveGpuValue>,
    pub(super) draws: Vec<SceneDrawRecord>,
    pub(super) phases: ScenePhaseTable,
    pub(super) node_bytes: Vec<u8>,
    pub(super) content_globals_bytes: Vec<u8>,
    pub(super) frame_bytes: Vec<u8>,
    pub(super) scene_content_bytes: Vec<u8>,
    pub(super) glyph_entries: Vec<GlyphAtlasGpuEntry>,
    glyph_lookup: SceneGlyphLookup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnsupportedSceneFeature {
    ShallowCardPrimitive,
    LitShallowCardMaterial,
    ShallowCardGeometryResource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SceneUploadError {
    AtlasGenerationMismatch {
        candidate: crate::presentation::companion_scene::ResourceGeneration,
        atlas: crate::presentation::companion_scene::ResourceGeneration,
    },
    InvalidPrimitiveReference {
        primitive_index: usize,
    },
    UnsupportedPrimitive {
        primitive_index: usize,
        feature: UnsupportedSceneFeature,
    },
    MirrorSizeMismatch,
    NonGlyphContentFamily,
    InvalidGlyphScalar {
        family: ContentMirrorFamily,
        slot: u32,
        scalar: u32,
    },
    MissingGlyphKey {
        family: ContentMirrorFamily,
        slot: u32,
        key: GlyphKey,
    },
    InvalidGlyphEntry {
        key: GlyphKey,
    },
}

pub(super) struct SceneGlyphWeightPolicy;

impl SceneGlyphWeightPolicy {
    pub(super) const fn pet_is_bold(palette_role_tag: u32) -> bool {
        palette_role_tag == PET_EYE_PALETTE_ROLE_TAG
    }

    pub(super) const fn content_is_bold(
        family: ContentMirrorFamily,
        flags: u32,
        signed_data: [i32; 2],
    ) -> bool {
        match family {
            ContentMirrorFamily::Pet | ContentMirrorFamily::PetParticles => {
                Self::pet_is_bold(flags)
            }
            ContentMirrorFamily::TankGlyphs => signed_data[1] == 1,
            ContentMirrorFamily::Globals
            | ContentMirrorFamily::PropGlyphs
            | ContentMirrorFamily::Ambient
            | ContentMirrorFamily::RoomGlyphs
            | ContentMirrorFamily::Analytics => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SceneGlyphLookupEntry {
    scalar: u32,
    bold: bool,
    atlas_id: u32,
}

impl SceneGlyphLookupEntry {
    const fn key(&self) -> (u32, bool) {
        (self.scalar, self.bold)
    }
}

/// Generation-owned scalar-to-atlas-id delta lookup. Multi-scalar keys remain
/// in the complete atlas for the HUD and immutable scene upload, but cannot be
/// authored by the fixed scalar scene mirrors and are intentionally omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SceneGlyphLookup {
    resource_generation: crate::presentation::companion_scene::ResourceGeneration,
    entries: Vec<SceneGlyphLookupEntry>,
}

impl SceneGlyphLookup {
    fn from_atlas(atlas: &PreparedSceneAtlas) -> Self {
        let mut entries = atlas
            .entries
            .iter()
            .filter_map(|entry| {
                let mut scalars = entry.key.sequence.as_str().chars();
                let scalar = scalars.next()?;
                scalars.next().is_none().then_some(SceneGlyphLookupEntry {
                    scalar: u32::from(scalar),
                    bold: entry.key.bold,
                    atlas_id: entry.id,
                })
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.key());
        debug_assert!(entries.windows(2).all(|pair| pair[0].key() < pair[1].key()));
        Self {
            resource_generation: atlas.resource_generation,
            entries,
        }
    }

    fn resolve(&self, scalar: u32, bold: bool) -> Option<u32> {
        if scalar == NONE_U32 {
            return Some(NONE_U32);
        }
        self.entries
            .binary_search_by_key(&(scalar, bold), SceneGlyphLookupEntry::key)
            .ok()
            .map(|index| self.entries[index].atlas_id)
    }
}

impl SceneContentGpuValue {
    fn translate(
        family: ContentMirrorFamily,
        value: ContentUploadValue,
        glyph_lookup: &SceneGlyphLookup,
    ) -> Result<Self, SceneUploadError> {
        if matches!(
            family,
            ContentMirrorFamily::Globals | ContentMirrorFamily::Analytics
        ) {
            return Err(SceneUploadError::NonGlyphContentFamily);
        }
        let glyph_entry_index = if value.glyph_scalar == NONE_U32 {
            glyph_lookup
                .resolve(value.glyph_scalar, false)
                .expect("the NONE sentinel always resolves")
        } else {
            let scalar =
                char::from_u32(value.glyph_scalar).ok_or(SceneUploadError::InvalidGlyphScalar {
                    family,
                    slot: value.slot,
                    scalar: value.glyph_scalar,
                })?;
            let bold =
                SceneGlyphWeightPolicy::content_is_bold(family, value.flags, value.signed_data);
            glyph_lookup
                .resolve(value.glyph_scalar, bold)
                .ok_or_else(|| SceneUploadError::MissingGlyphKey {
                    family,
                    slot: value.slot,
                    key: GlyphKey::new(scalar.to_string(), bold),
                })?
        };
        Ok(Self {
            kind: value.kind,
            glyph_entry_index,
            slot: value.slot,
            subslot: value.subslot,
            signed_data: value.signed_data,
            flags: value.flags,
            variant: value.variant,
        })
    }
}

pub(super) fn prepare_scene_upload(
    candidate: &CpuSceneCandidate,
    atlas: &PreparedSceneAtlas,
) -> Result<PreparedSceneUpload, SceneUploadError> {
    if atlas.resource_generation != candidate.generation_key.resources {
        return Err(SceneUploadError::AtlasGenerationMismatch {
            candidate: candidate.generation_key.resources,
            atlas: atlas.resource_generation,
        });
    }
    // Reject deferred card work before allocating or copying upload-owned data.
    for primitive_index in 0..candidate.primitive_count() {
        let source = candidate
            .primitive_upload_source(primitive_index)
            .ok_or(SceneUploadError::InvalidPrimitiveReference { primitive_index })?;
        preflight_primitive(primitive_index, source)?;
    }
    let glyph_entries = prepare_glyph_entries(atlas)?;
    let glyph_lookup = SceneGlyphLookup::from_atlas(atlas);

    let mut primitives = Vec::with_capacity(candidate.primitive_count());
    let mut draws = Vec::with_capacity(candidate.primitive_count());
    for primitive_index in 0..candidate.primitive_count() {
        let source = candidate
            .primitive_upload_source(primitive_index)
            .ok_or(SceneUploadError::InvalidPrimitiveReference { primitive_index })?;
        let (content_base, frame_base, aux_content_base) = primitive_arena_bases(source)
            .ok_or(SceneUploadError::InvalidPrimitiveReference { primitive_index })?;
        primitives.push(PrimitiveGpuValue {
            node_index: source.node_index,
            aux_node_index: source.aux_node_index,
            material_index: source.material_index,
            primitive_kind: source.primitive_kind,
            material_kind: source.material_kind,
            resource_kind: source.resource_kind,
            blend: source.blend,
            depth: source.depth,
            space: source.space,
            instance_group: source.instance_group,
            instance_base: source.instance_base,
            binding_index: source.instance_slot,
            authored_order: source.authored_order,
            content_base,
            frame_base,
            aux_content_base,
        });
        draws.push(
            prepare_draw_record(source)
                .ok_or(SceneUploadError::InvalidPrimitiveReference { primitive_index })?,
        );
    }

    let (content_globals_bytes, scene_content_bytes) =
        prepare_content_buffers(candidate, &glyph_lookup)?;
    let sources = candidate.frame_upload_sources();
    let node_bytes = exact_owned(sources.nodes, PackedMirrorLayout::NODE_BYTES)?;
    let frame_bytes = prepare_frame_bytes(sources)?;
    let phase_sources = candidate.phase_upload_sources();
    Ok(PreparedSceneUpload {
        generation_key: candidate.generation_key,
        source_revisions: candidate.source_revisions,
        logical_viewport_points: candidate.logical_viewport_points(),
        static_checksum: candidate.static_checksum,
        vertex_bytes: candidate.vertex_bytes().to_vec(),
        index_bytes: candidate.index_bytes().to_vec(),
        primitives,
        draws,
        phases: ScenePhaseTable {
            opaque_cutout: phase_sources.opaque_cutout.to_vec(),
            world_blended_unsorted: phase_sources.world_blended_unsorted.to_vec(),
            chrome_authored: phase_sources.chrome_authored.to_vec(),
        },
        node_bytes,
        content_globals_bytes,
        frame_bytes,
        scene_content_bytes,
        glyph_entries,
        glyph_lookup,
    })
}

fn prepare_draw_record(source: PrimitiveUploadSource) -> Option<SceneDrawRecord> {
    let index_range = source
        .first_index
        .checked_add(source.index_count)
        .map(|end| source.first_index..end)?;
    let (primitive_source, instance_count) = match source.primitive_kind {
        ATLAS_QUAD_PRIMITIVE_TAG => (PrimitiveSource::StaticAtlas, 1),
        ANALYTIC_PRIMITIVE_TAG if is_wall_shadow_glyph_mask(source) => (
            PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask),
            u32::try_from(crate::presentation::companion_scene::scene::MAX_PET_ART_SLOTS).ok()?,
        ),
        ANALYTIC_PRIMITIVE_TAG => (PrimitiveSource::Analytic, 1),
        SHALLOW_CARD_PRIMITIVE_TAG => (PrimitiveSource::None, 0),
        INSTANCE_QUAD_PRIMITIVE_TAG => {
            let (instance_source, count) = instance_source_and_count(source)?;
            (PrimitiveSource::Instances(instance_source), count)
        }
        _ => return None,
    };
    Some(SceneDrawRecord {
        index_range,
        instance_range: 0..instance_count,
        source: primitive_source,
        authored_order: source.authored_order,
    })
}

const fn is_wall_shadow_glyph_mask(source: PrimitiveUploadSource) -> bool {
    source.primitive_kind == ANALYTIC_PRIMITIVE_TAG
        && source.instance_group == 0
        && source.instance_slot == 1
}

fn primitive_arena_bases(source: PrimitiveUploadSource) -> Option<(u32, u32, u32)> {
    arena_bases_from_tags(
        source.primitive_kind,
        source.instance_group,
        source.instance_base,
        source.instance_slot,
    )
}

fn arena_bases_from_tags(
    primitive_kind: u32,
    instance_group: u32,
    instance_base: u32,
    binding_index: u32,
) -> Option<(u32, u32, u32)> {
    use crate::presentation::companion_scene::scene::{
        MAX_ANALYTIC_PARAMS, MAX_PROP_GLYPHS_PER_SLOT, MAX_STATIC_ATLAS_RECIPES,
        MAX_TANK_GLYPHS_PER_SLOT,
    };

    let content = |family| {
        PackedMirrorLayout::scene_content_offset(family)
            .and_then(|offset| offset.checked_div(std::mem::size_of::<SceneContentGpuValue>()))
            .and_then(|offset| u32::try_from(offset).ok())
    };
    let frame = |family| {
        PackedMirrorLayout::frame_offset(family)
            .checked_sub(PackedMirrorLayout::frame_offset(FrameMirrorFamily::Props))
            .and_then(|offset| {
                offset.checked_div(std::mem::size_of::<super::compiler::FrameGpuValue>())
            })
            .and_then(|offset| u32::try_from(offset).ok())
    };
    let relative = |base: u32| base.checked_add(instance_base);
    match instance_group {
        0 if primitive_kind == ANALYTIC_PRIMITIVE_TAG
            && binding_index < u32::try_from(MAX_ANALYTIC_PARAMS).ok()? =>
        {
            Some((
                NONE_U32,
                binding_index,
                if binding_index == 1 {
                    content(ContentMirrorFamily::Pet)?
                } else {
                    NONE_U32
                },
            ))
        }
        0 if primitive_kind == ATLAS_QUAD_PRIMITIVE_TAG
            && binding_index < u32::try_from(MAX_STATIC_ATLAS_RECIPES).ok()? =>
        {
            Some((NONE_U32, NONE_U32, NONE_U32))
        }
        0 if primitive_kind == SHALLOW_CARD_PRIMITIVE_TAG && binding_index == NONE_U32 => {
            Some((NONE_U32, NONE_U32, NONE_U32))
        }
        1 if binding_index == 0 && instance_base == 0 => Some((
            relative(content(ContentMirrorFamily::Pet)?)?,
            NONE_U32,
            NONE_U32,
        )),
        2 if binding_index == 0 && instance_base == 0 => Some((
            relative(content(ContentMirrorFamily::PetParticles)?)?,
            NONE_U32,
            NONE_U32,
        )),
        3 if instance_base
            == binding_index.checked_mul(u32::try_from(MAX_PROP_GLYPHS_PER_SLOT).ok()?)? =>
        {
            Some((
                relative(content(ContentMirrorFamily::PropGlyphs)?)?,
                frame(FrameMirrorFamily::Props)?.checked_add(binding_index)?,
                NONE_U32,
            ))
        }
        4 if binding_index == 0 && instance_base == 0 => Some((
            relative(content(ContentMirrorFamily::RoomGlyphs)?)?,
            frame(FrameMirrorFamily::RoomGlyphs)?,
            NONE_U32,
        )),
        5 | 6
            if instance_base
                == binding_index.checked_mul(u32::try_from(MAX_TANK_GLYPHS_PER_SLOT).ok()?)? =>
        {
            Some((
                relative(content(ContentMirrorFamily::TankGlyphs)?)?,
                relative(frame(FrameMirrorFamily::TankCells)?)?,
                NONE_U32,
            ))
        }
        7 if binding_index == 0 && instance_base == 0 => Some((
            relative(content(ContentMirrorFamily::Ambient)?)?,
            frame(FrameMirrorFamily::Ambient)?,
            NONE_U32,
        )),
        8 if binding_index == 0 && instance_base == 0 => Some((NONE_U32, NONE_U32, NONE_U32)),
        _ => None,
    }
}

fn instance_source_and_count(source: PrimitiveUploadSource) -> Option<(InstanceSource, u32)> {
    instance_source_and_count_from_tags(source.instance_group, source.instance_base)
}

fn instance_source_and_count_from_tags(
    instance_group: u32,
    instance_base: u32,
) -> Option<(InstanceSource, u32)> {
    use crate::presentation::companion_scene::scene::{
        InstanceLayer, MAX_AMBIENT_INSTANCES, MAX_PET_ART_SLOTS, MAX_PROP_GLYPHS_PER_SLOT,
        MAX_ROOM_GLYPH_SLOTS, MAX_ROUND_TANK_INHABITANTS, MAX_TANK_GLYPHS_PER_SLOT,
        MAX_VISIBLE_PROPS,
    };

    let count = |value: usize| u32::try_from(value).expect("scene capacity fits in u32");
    match instance_group {
        1 if instance_base == 0 => Some((InstanceSource::PetBody, count(MAX_PET_ART_SLOTS))),
        2 if instance_base == 0 => Some((InstanceSource::PetParticles, count(MAX_PET_ART_SLOTS))),
        3 => {
            let width = count(MAX_PROP_GLYPHS_PER_SLOT);
            let slot = instance_base.checked_div(width)?;
            (instance_base.is_multiple_of(width) && slot < count(MAX_VISIBLE_PROPS))
                .then_some((InstanceSource::PropGlyphs { slot }, width))
        }
        5 | 6 => {
            let width = count(MAX_TANK_GLYPHS_PER_SLOT);
            let slot = instance_base.checked_div(width)?;
            let layer = if instance_group == 5 {
                InstanceLayer::Behind
            } else {
                InstanceLayer::Foreground
            };
            (instance_base.is_multiple_of(width) && slot < count(MAX_ROUND_TANK_INHABITANTS))
                .then_some((InstanceSource::TankCells { slot, layer }, width))
        }
        7 if instance_base == 0 => Some((InstanceSource::Ambient, count(MAX_AMBIENT_INSTANCES))),
        4 if instance_base == 0 => Some((InstanceSource::RoomGlyphs, count(MAX_ROOM_GLYPH_SLOTS))),
        // The authored HUD hook is retained, but its sensitive instances are
        // prepared through the sealed HUD path rather than general scene mirrors.
        8 if instance_base == 0 => Some((InstanceSource::Hud, 0)),
        _ => None,
    }
}

fn preflight_primitive(
    primitive_index: usize,
    source: PrimitiveUploadSource,
) -> Result<(), SceneUploadError> {
    for (present, feature) in [
        (
            source.primitive_kind == SHALLOW_CARD_PRIMITIVE_TAG,
            UnsupportedSceneFeature::ShallowCardPrimitive,
        ),
        (
            source.material_kind == LIT_SHALLOW_CARD_MATERIAL_TAG,
            UnsupportedSceneFeature::LitShallowCardMaterial,
        ),
        (
            source.resource_kind == SHALLOW_CARD_GEOMETRY_RESOURCE_TAG,
            UnsupportedSceneFeature::ShallowCardGeometryResource,
        ),
    ] {
        if present {
            return Err(SceneUploadError::UnsupportedPrimitive { primitive_index, feature });
        }
    }
    Ok(())
}

fn prepare_content_buffers(
    candidate: &CpuSceneCandidate,
    glyph_lookup: &SceneGlyphLookup,
) -> Result<(Vec<u8>, Vec<u8>), SceneUploadError> {
    let sources = candidate.content_upload_sources();
    let globals = exact_owned(sources.globals, PackedMirrorLayout::CONTENT_GLOBALS_BYTES)?;
    let mut scene_content = vec![0; PackedMirrorLayout::SCENE_CONTENT_BYTES];
    for (family, values) in [
        (ContentMirrorFamily::Pet, sources.pet),
        (ContentMirrorFamily::PropGlyphs, sources.prop_glyphs),
        (ContentMirrorFamily::TankGlyphs, sources.tank_glyphs),
        (ContentMirrorFamily::Ambient, sources.ambient),
        (ContentMirrorFamily::PetParticles, sources.pet_particles),
        (ContentMirrorFamily::RoomGlyphs, sources.room_glyphs),
    ] {
        let translated = values
            .iter()
            .copied()
            .map(ContentUploadValue::from)
            .map(|value| SceneContentGpuValue::translate(family, value, glyph_lookup))
            .collect::<Result<Vec<_>, _>>()?;
        copy_family(
            &mut scene_content,
            PackedMirrorLayout::scene_content_offset(family)
                .expect("glyph families live in the scene-content buffer"),
            family.byte_len(),
            bytemuck::cast_slice(&translated),
        )?;
    }
    copy_family(
        &mut scene_content,
        PackedMirrorLayout::scene_content_offset(ContentMirrorFamily::Analytics)
            .expect("analytic content lives in the scene-content buffer"),
        ContentMirrorFamily::Analytics.byte_len(),
        bytemuck::cast_slice(sources.analytics),
    )?;
    Ok((globals, scene_content))
}

fn prepare_frame_bytes(sources: FrameUploadSources<'_>) -> Result<Vec<u8>, SceneUploadError> {
    let mut packed = vec![0; PackedMirrorLayout::FRAME_BYTES];
    for (family, bytes) in [
        (FrameMirrorFamily::Globals, sources.globals),
        (FrameMirrorFamily::Props, sources.props),
        (FrameMirrorFamily::TankCells, sources.tank_cells),
        (FrameMirrorFamily::Ambient, sources.ambient),
        (FrameMirrorFamily::Lights, sources.lights),
        (FrameMirrorFamily::RoomGlyphs, sources.room_glyphs),
        (FrameMirrorFamily::Analytics, sources.analytics),
    ] {
        copy_family(
            &mut packed,
            PackedMirrorLayout::frame_offset(family),
            family.byte_len(),
            bytes,
        )?;
    }
    Ok(packed)
}

fn exact_owned(bytes: &[u8], expected: usize) -> Result<Vec<u8>, SceneUploadError> {
    (bytes.len() == expected)
        .then(|| bytes.to_vec())
        .ok_or(SceneUploadError::MirrorSizeMismatch)
}

fn copy_family(
    packed: &mut [u8],
    offset: usize,
    expected: usize,
    bytes: &[u8],
) -> Result<(), SceneUploadError> {
    if bytes.len() != expected {
        return Err(SceneUploadError::MirrorSizeMismatch);
    }
    let end = offset
        .checked_add(expected)
        .filter(|end| *end <= packed.len())
        .ok_or(SceneUploadError::MirrorSizeMismatch)?;
    packed[offset..end].copy_from_slice(bytes);
    Ok(())
}

pub(super) fn prepare_glyph_entries(
    atlas: &PreparedSceneAtlas,
) -> Result<Vec<GlyphAtlasGpuEntry>, SceneUploadError> {
    atlas
        .entries
        .iter()
        .map(|source| {
            let entry = source.entry;
            if !valid_scene_glyph_entry(entry, atlas.width, atlas.height) {
                return Err(SceneUploadError::InvalidGlyphEntry { key: source.key.clone() });
            }
            Ok(GlyphAtlasGpuEntry {
                visible_uv: entry.visible_uv.unwrap_or([0.0; 4]),
                ink_origin_size: [
                    entry.ink_origin[0],
                    entry.raster_size[1]
                        - 2.0 * entry.safe_padding
                        - entry.ink_origin[1]
                        - entry.ink_size[1],
                    entry.ink_size[0],
                    entry.ink_size[1],
                ],
                metrics: [entry.advance, entry.line_height, entry.baseline],
                flags: (u32::from(entry.visible_uv.is_some()) * GLYPH_FLAG_VISIBLE)
                    | (u32::from(entry.kind == GlyphEntryKind::PremultipliedColorRgba)
                        * GLYPH_FLAG_COLOR),
                allocated_cell: [
                    entry.allocated_cell.origin[0],
                    entry.allocated_cell.origin[1],
                    entry.allocated_cell.extent[0],
                    entry.allocated_cell.extent[1],
                ],
            })
        })
        .collect()
}

fn valid_scene_glyph_entry(
    entry: super::resources::GlyphAtlasEntry,
    atlas_width: u32,
    atlas_height: u32,
) -> bool {
    if atlas_width == 0 || atlas_height == 0 {
        return false;
    }
    let finite = entry
        .visible_uv
        .into_iter()
        .flatten()
        .chain(entry.ink_origin)
        .chain(entry.ink_size)
        .chain([
            entry.line_height,
            entry.advance,
            entry.baseline,
            entry.ascent,
            entry.descent,
        ])
        .chain(entry.raster_size)
        .chain([entry.safe_padding])
        .all(f32::is_finite);
    let has_positive_ink = entry.ink_size[0] > 0.0 && entry.ink_size[1] > 0.0;
    let has_any_ink = entry.ink_size[0] != 0.0 || entry.ink_size[1] != 0.0;
    if !finite
        || entry.advance <= 0.0
        || entry.line_height <= 0.0
        || entry.safe_padding < 0.0
        || has_any_ink != has_positive_ink
        || entry.visible_uv.is_some() != has_positive_ink
    {
        return false;
    }

    if let Some([u_min, v_min, u_max, v_max]) = entry.visible_uv {
        let cell = entry.allocated_cell;
        let Some(cell_end_x) = cell.origin[0].checked_add(cell.extent[0]) else {
            return false;
        };
        let Some(cell_end_y) = cell.origin[1].checked_add(cell.extent[1]) else {
            return false;
        };
        let cell_u_min = cell.origin[0] as f32 / atlas_width as f32;
        let cell_v_min = cell.origin[1] as f32 / atlas_height as f32;
        let cell_u_max = cell_end_x as f32 / atlas_width as f32;
        let cell_v_max = cell_end_y as f32 / atlas_height as f32;
        let valid_uv = u_min >= 0.0
            && v_min >= 0.0
            && u_min < u_max
            && v_min < v_max
            && u_max <= 1.0
            && v_max <= 1.0
            && u_min >= cell_u_min
            && v_min >= cell_v_min
            && u_max <= cell_u_max
            && v_max <= cell_v_max;
        let left = entry.ink_origin[0] + entry.safe_padding;
        let top = entry.ink_origin[1] + entry.safe_padding;
        let right = left + entry.ink_size[0];
        let bottom = top + entry.ink_size[1];
        let valid_raster = entry.raster_size[0] > 0.0
            && entry.raster_size[1] > 0.0
            && left >= 0.0
            && top >= 0.0
            && right <= entry.raster_size[0]
            && bottom <= entry.raster_size[1];
        valid_uv && valid_raster
    } else {
        entry.ink_origin == [0.0; 2]
            && entry.ink_size == [0.0; 2]
            && entry.raster_size == [0.0; 2]
            && entry.safe_padding == 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneSurfaceContract {
    pub(super) format: wgpu::TextureFormat,
    pub(super) color_space: wgpu::SurfaceColorSpace,
    pub(super) alpha_mode: wgpu::CompositeAlphaMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneSurfaceContractError {
    MissingBgra8UnormSrgb,
    MissingSrgbColorSpace,
    MissingPostMultipliedAlpha,
    MissingRenderAttachmentUsage,
}

impl SceneSurfaceContract {
    pub(super) fn select(
        capabilities: &wgpu::SurfaceCapabilities,
    ) -> Result<Self, SceneSurfaceContractError> {
        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let format_capabilities = capabilities
            .format_capabilities
            .iter()
            .filter(|candidate| candidate.format == format)
            .collect::<Vec<_>>();
        if format_capabilities.is_empty() {
            return Err(SceneSurfaceContractError::MissingBgra8UnormSrgb);
        }
        if !format_capabilities.iter().any(|candidate| {
            candidate
                .color_spaces
                .contains(wgpu::SurfaceColorSpaces::SRGB)
        }) {
            return Err(SceneSurfaceContractError::MissingSrgbColorSpace);
        }
        if !capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            return Err(SceneSurfaceContractError::MissingPostMultipliedAlpha);
        }
        if !capabilities
            .usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            return Err(SceneSurfaceContractError::MissingRenderAttachmentUsage);
        }
        Ok(Self {
            format,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            alpha_mode: wgpu::CompositeAlphaMode::PostMultiplied,
        })
    }
}

pub(super) struct SceneTextureContract;

pub(super) struct SceneTargetTextureUsages;

impl SceneTargetTextureUsages {
    pub(super) const RAW_SCENE: wgpu::TextureUsages =
        wgpu::TextureUsages::RENDER_ATTACHMENT.union(wgpu::TextureUsages::TEXTURE_BINDING);
    pub(super) const INTERMEDIATE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
        .union(wgpu::TextureUsages::TEXTURE_BINDING)
        .union(wgpu::TextureUsages::COPY_SRC);
    pub(super) const DEPTH: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneTextureContractError {
    Usage {
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    },
    NotFilterable {
        format: wgpu::TextureFormat,
    },
    NotBlendable {
        format: wgpu::TextureFormat,
    },
}

impl SceneTextureContract {
    pub(super) const INTERMEDIATE: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    pub(super) const COVERAGE: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
    pub(super) const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    pub(super) const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
    pub(super) const SAMPLE_COUNT: u32 = 1;

    pub(super) fn validate_with(
        mut features_for: impl FnMut(wgpu::TextureFormat) -> wgpu::TextureFormatFeatures,
    ) -> Result<(), SceneTextureContractError> {
        for (format, usages, filterable, blendable) in [
            (
                Self::INTERMEDIATE,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                true,
                true,
            ),
            (
                Self::COVERAGE,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                true,
                false,
            ),
            (
                Self::COLOR,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                true,
                false,
            ),
            (
                Self::DEPTH,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                false,
                false,
            ),
        ] {
            let features = features_for(format);
            if !features.allowed_usages.contains(usages) {
                return Err(SceneTextureContractError::Usage { format, usage: usages });
            }
            if filterable
                && !features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
            {
                return Err(SceneTextureContractError::NotFilterable { format });
            }
            if blendable
                && !features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
            {
                return Err(SceneTextureContractError::NotBlendable { format });
            }
        }
        Ok(())
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContentMirrorFamily {
    Globals,
    Pet,
    PropGlyphs,
    TankGlyphs,
    Ambient,
    PetParticles,
    RoomGlyphs,
    Analytics,
}

impl ContentMirrorFamily {
    pub(super) const ALL: [Self; 8] = [
        Self::Globals,
        Self::Pet,
        Self::PropGlyphs,
        Self::TankGlyphs,
        Self::Ambient,
        Self::PetParticles,
        Self::RoomGlyphs,
        Self::Analytics,
    ];

    pub(super) const fn record_size(self) -> usize {
        super::compiler::CpuMirrorShape::CONTENT_RECORD_BYTES[self as usize]
    }

    const fn byte_len(self) -> usize {
        self.record_size() * super::compiler::CpuMirrorShape::CONTENT_COUNTS[self as usize]
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameMirrorFamily {
    Globals,
    Props,
    TankCells,
    Ambient,
    Lights,
    RoomGlyphs,
    Analytics,
}

impl FrameMirrorFamily {
    pub(super) const ALL: [Self; 7] = [
        Self::Globals,
        Self::Props,
        Self::TankCells,
        Self::Ambient,
        Self::Lights,
        Self::RoomGlyphs,
        Self::Analytics,
    ];

    pub(super) const fn record_size(self) -> usize {
        super::compiler::CpuMirrorShape::FRAME_RECORD_BYTES[self as usize]
    }

    const fn byte_len(self) -> usize {
        self.record_size() * super::compiler::CpuMirrorShape::FRAME_COUNTS[self as usize]
    }
}

pub(super) struct PackedMirrorLayout;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneMutableBuffer {
    Nodes,
    ContentGlobals,
    Frame,
    SceneContent,
}

impl SceneMutableBuffer {
    const ALL: [Self; 4] = [
        Self::Nodes,
        Self::ContentGlobals,
        Self::Frame,
        Self::SceneContent,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SceneMutableBufferLayout {
    staging_offset: usize,
    len: usize,
    copy_alignment: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackedMirrorFamilyLayout {
    buffer: SceneMutableBuffer,
    offset: usize,
    len: usize,
    span_alignment: usize,
}

/// Physical dirty ranges after logical families have been translated into the
/// four mutable storage buffers. This stays independent of compiler deltas so
/// staging can validate and aggregate one family at a time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ScenePhysicalDirtySpans {
    nodes: DirtySpanSet,
    content_globals: DirtySpanSet,
    frame: DirtySpanSet,
    scene_content: DirtySpanSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PackedMirrorLayoutError {
    NonSceneContentFamily,
    MisalignedSpan,
    SpanOutOfBounds,
}

impl PackedMirrorLayout {
    pub(super) const NODE_BYTES: usize = super::compiler::CpuMirrorShape::NODE_RECORD_BYTES
        * super::compiler::CpuMirrorShape::NODE_COUNT;
    pub(super) const CONTENT_GLOBALS_BYTES: usize = ContentMirrorFamily::Globals.byte_len();
    pub(super) const SCENE_CONTENT_BYTES: usize = scene_content_end(ContentMirrorFamily::Analytics);
    pub(super) const FRAME_BYTES: usize = frame_end(FrameMirrorFamily::Analytics);
    const MUTABLE_BUFFER_BYTES: usize = Self::NODE_BYTES
        + Self::CONTENT_GLOBALS_BYTES
        + Self::FRAME_BYTES
        + Self::SCENE_CONTENT_BYTES;
    const FULL_FRAME_STAGING_BYTES: usize =
        Self::MUTABLE_BUFFER_BYTES + super::hud::HUD_GPU_BUFFER_BYTES as usize;

    const fn mutable_buffer_layout(buffer: SceneMutableBuffer) -> SceneMutableBufferLayout {
        let staging_offset = match buffer {
            SceneMutableBuffer::Nodes => 0,
            SceneMutableBuffer::ContentGlobals => Self::NODE_BYTES,
            SceneMutableBuffer::Frame => Self::NODE_BYTES + Self::CONTENT_GLOBALS_BYTES,
            SceneMutableBuffer::SceneContent => {
                Self::NODE_BYTES + Self::CONTENT_GLOBALS_BYTES + Self::FRAME_BYTES
            }
        };
        let len = match buffer {
            SceneMutableBuffer::Nodes => Self::NODE_BYTES,
            SceneMutableBuffer::ContentGlobals => Self::CONTENT_GLOBALS_BYTES,
            SceneMutableBuffer::Frame => Self::FRAME_BYTES,
            SceneMutableBuffer::SceneContent => Self::SCENE_CONTENT_BYTES,
        };
        SceneMutableBufferLayout {
            staging_offset,
            len,
            copy_alignment: wgpu::COPY_BUFFER_ALIGNMENT as usize,
        }
    }

    const fn content_family_layout(family: ContentMirrorFamily) -> PackedMirrorFamilyLayout {
        let (buffer, offset) = match Self::scene_content_offset(family) {
            Some(offset) => (SceneMutableBuffer::SceneContent, offset),
            None => (
                SceneMutableBuffer::ContentGlobals,
                Self::content_globals_offset(),
            ),
        };
        PackedMirrorFamilyLayout {
            buffer,
            offset,
            len: family.byte_len(),
            span_alignment: family.record_size(),
        }
    }

    const fn frame_family_layout(family: FrameMirrorFamily) -> PackedMirrorFamilyLayout {
        PackedMirrorFamilyLayout {
            buffer: SceneMutableBuffer::Frame,
            offset: Self::frame_offset(family),
            len: family.byte_len(),
            span_alignment: family.record_size(),
        }
    }

    pub(super) const fn content_globals_offset() -> usize {
        0
    }

    pub(super) const fn scene_content_offset(family: ContentMirrorFamily) -> Option<usize> {
        match family {
            ContentMirrorFamily::Globals => None,
            ContentMirrorFamily::Pet => Some(0),
            ContentMirrorFamily::PropGlyphs => Some(scene_content_end(ContentMirrorFamily::Pet)),
            ContentMirrorFamily::TankGlyphs => {
                Some(scene_content_end(ContentMirrorFamily::PropGlyphs))
            }
            ContentMirrorFamily::Ambient => {
                Some(scene_content_end(ContentMirrorFamily::TankGlyphs))
            }
            ContentMirrorFamily::PetParticles => {
                Some(scene_content_end(ContentMirrorFamily::Ambient))
            }
            ContentMirrorFamily::RoomGlyphs => {
                Some(scene_content_end(ContentMirrorFamily::PetParticles))
            }
            ContentMirrorFamily::Analytics => {
                Some(scene_content_end(ContentMirrorFamily::RoomGlyphs))
            }
        }
    }

    pub(super) const fn frame_offset(family: FrameMirrorFamily) -> usize {
        match family {
            FrameMirrorFamily::Globals => 0,
            FrameMirrorFamily::Props => frame_end(FrameMirrorFamily::Globals),
            FrameMirrorFamily::TankCells => frame_end(FrameMirrorFamily::Props),
            FrameMirrorFamily::Ambient => frame_end(FrameMirrorFamily::TankCells),
            FrameMirrorFamily::Lights => frame_end(FrameMirrorFamily::Ambient),
            FrameMirrorFamily::RoomGlyphs => frame_end(FrameMirrorFamily::Lights),
            FrameMirrorFamily::Analytics => frame_end(FrameMirrorFamily::RoomGlyphs),
        }
    }

    pub(super) fn translate_node_span(span: ByteSpan) -> Result<ByteSpan, PackedMirrorLayoutError> {
        translate_span(
            0,
            Self::NODE_BYTES,
            super::compiler::CpuMirrorShape::NODE_RECORD_BYTES,
            span,
        )
    }

    pub(super) fn translate_content_globals_span(
        span: ByteSpan,
    ) -> Result<ByteSpan, PackedMirrorLayoutError> {
        translate_span(
            Self::content_globals_offset(),
            Self::CONTENT_GLOBALS_BYTES,
            ContentMirrorFamily::Globals.record_size(),
            span,
        )
    }

    pub(super) fn translate_scene_content_span(
        family: ContentMirrorFamily,
        span: ByteSpan,
    ) -> Result<ByteSpan, PackedMirrorLayoutError> {
        let offset = Self::scene_content_offset(family)
            .ok_or(PackedMirrorLayoutError::NonSceneContentFamily)?;
        translate_span(offset, family.byte_len(), family.record_size(), span)
    }

    pub(super) fn translate_frame_span(
        family: FrameMirrorFamily,
        span: ByteSpan,
    ) -> Result<ByteSpan, PackedMirrorLayoutError> {
        translate_span(
            Self::frame_offset(family),
            family.byte_len(),
            family.record_size(),
            span,
        )
    }
}

impl ScenePhysicalDirtySpans {
    fn from_logical(dirty: SceneDirtySpans) -> Result<Self, PackedMirrorLayoutError> {
        let mut physical = Self::default();
        for span in dirty.nodes.as_slice() {
            physical.insert_node(*span)?;
        }
        for (family, spans) in [
            (ContentMirrorFamily::Globals, dirty.content_globals),
            (ContentMirrorFamily::Pet, dirty.pet_body),
            (ContentMirrorFamily::PetParticles, dirty.pet_particles),
            (ContentMirrorFamily::RoomGlyphs, dirty.room_content),
            (ContentMirrorFamily::PropGlyphs, dirty.prop_glyphs),
            (ContentMirrorFamily::TankGlyphs, dirty.tank_glyphs),
            (ContentMirrorFamily::Ambient, dirty.content_ambient),
            (ContentMirrorFamily::Analytics, dirty.content_analytics),
        ] {
            for span in spans.as_slice() {
                physical.insert_content(family, *span)?;
            }
        }
        for (family, spans) in [
            (FrameMirrorFamily::Globals, dirty.frame_globals),
            (FrameMirrorFamily::RoomGlyphs, dirty.room_frame),
            (FrameMirrorFamily::Props, dirty.props),
            (FrameMirrorFamily::TankCells, dirty.tank_cells),
            (FrameMirrorFamily::Ambient, dirty.frame_ambient),
            (FrameMirrorFamily::Analytics, dirty.frame_analytics),
            (FrameMirrorFamily::Lights, dirty.lights),
        ] {
            for span in spans.as_slice() {
                physical.insert_frame(family, *span)?;
            }
        }
        Ok(physical)
    }

    fn copy_count(self) -> usize {
        self.nodes.as_slice().len()
            + self.content_globals.as_slice().len()
            + self.frame.as_slice().len()
            + self.scene_content.as_slice().len()
    }

    fn insert_node(&mut self, span: ByteSpan) -> Result<(), PackedMirrorLayoutError> {
        self.nodes
            .insert(PackedMirrorLayout::translate_node_span(span)?);
        Ok(())
    }

    fn insert_content(
        &mut self,
        family: ContentMirrorFamily,
        span: ByteSpan,
    ) -> Result<(), PackedMirrorLayoutError> {
        match family {
            ContentMirrorFamily::Globals => self
                .content_globals
                .insert(PackedMirrorLayout::translate_content_globals_span(span)?),
            _ => self
                .scene_content
                .insert(PackedMirrorLayout::translate_scene_content_span(
                    family, span,
                )?),
        }
        Ok(())
    }

    fn insert_frame(
        &mut self,
        family: FrameMirrorFamily,
        span: ByteSpan,
    ) -> Result<(), PackedMirrorLayoutError> {
        self.frame
            .insert(PackedMirrorLayout::translate_frame_span(family, span)?);
        Ok(())
    }
}

const fn scene_content_end(family: ContentMirrorFamily) -> usize {
    match PackedMirrorLayout::scene_content_offset(family) {
        Some(offset) => offset + family.byte_len(),
        None => 0,
    }
}

const fn frame_end(family: FrameMirrorFamily) -> usize {
    PackedMirrorLayout::frame_offset(family) + family.byte_len()
}

fn translate_span(
    family_offset: usize,
    family_len: usize,
    record_size: usize,
    span: ByteSpan,
) -> Result<ByteSpan, PackedMirrorLayoutError> {
    if !span.offset.is_multiple_of(record_size) || !span.len.is_multiple_of(record_size) {
        return Err(PackedMirrorLayoutError::MisalignedSpan);
    }
    let end = span
        .offset
        .checked_add(span.len)
        .ok_or(PackedMirrorLayoutError::SpanOutOfBounds)?;
    if end > family_len {
        return Err(PackedMirrorLayoutError::SpanOutOfBounds);
    }
    Ok(ByteSpan {
        offset: family_offset + span.offset,
        len: span.len,
    })
}

/// Exact persistent handles retained after shared construction. Shader modules
/// and pipeline layouts are transient construction objects and intentionally do
/// not appear in this owned-handle inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneGpuSharedFacts {
    pub(super) bind_group_layouts: u8,
    pub(super) samplers: u8,
    pub(super) pipelines: u8,
}

impl SceneGpuSharedFacts {
    pub(super) const EXPECTED: Self = Self {
        bind_group_layouts: 4,
        samplers: 1,
        pipelines: 11,
    };

    pub(super) const fn persistent_owned_handles(self) -> u8 {
        self.bind_group_layouts + self.samplers + self.pipelines
    }
}

/// Exact persistent candidate handles and one-time upload events. Uploads are
/// events, not owned handles, so they are excluded from
/// [`Self::persistent_owned_handles`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GpuSceneCandidateFacts {
    pub(super) buffers: u8,
    pub(super) textures: u8,
    pub(super) texture_views: u8,
    pub(super) bind_groups: u8,
    pub(super) static_uploads: u8,
}

impl GpuSceneCandidateFacts {
    pub(super) const EXPECTED: Self = Self {
        buffers: 10,
        textures: 2,
        texture_views: 2,
        bind_groups: 4,
        static_uploads: 10,
    };

    pub(super) const fn persistent_owned_handles(self) -> u8 {
        self.buffers + self.textures + self.texture_views + self.bind_groups
    }
}

pub(super) struct SceneBufferUsages;

impl SceneBufferUsages {
    pub(super) const VERTEX: wgpu::BufferUsages = wgpu::BufferUsages::VERTEX;
    pub(super) const INDEX: wgpu::BufferUsages = wgpu::BufferUsages::INDEX;
    pub(super) const NODE: wgpu::BufferUsages =
        wgpu::BufferUsages::STORAGE.union(wgpu::BufferUsages::COPY_DST);
    pub(super) const CONTENT_GLOBALS: wgpu::BufferUsages = Self::NODE;
    pub(super) const FRAME: wgpu::BufferUsages = Self::NODE;
    pub(super) const PRIMITIVE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE;
    pub(super) const SCENE_CONTENT: wgpu::BufferUsages = Self::NODE;
    pub(super) const GLYPH_ENTRY: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneTargetFacts {
    pub(super) textures: u8,
    pub(super) texture_views: u8,
    pub(super) bind_groups: u8,
}

impl SceneTargetFacts {
    pub(super) const EXPECTED: Self = Self {
        textures: 3,
        texture_views: 3,
        bind_groups: 2,
    };

    pub(super) const fn persistent_owned_handles(self) -> u8 {
        self.textures + self.texture_views + self.bind_groups
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScopedGpuErrorCategory {
    Validation,
    Internal,
    OutOfMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneTargetKeyError {
    Extent,
    Formats,
    SampleCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneGpuError {
    ActualDeviceMismatch,
    AtlasGenerationMismatch {
        upload: crate::presentation::companion_scene::ResourceGeneration,
        atlas: crate::presentation::companion_scene::ResourceGeneration,
    },
    GlyphLookupGenerationMismatch {
        upload: crate::presentation::companion_scene::ResourceGeneration,
        lookup: crate::presentation::companion_scene::ResourceGeneration,
        atlas: crate::presentation::companion_scene::ResourceGeneration,
    },
    DeviceEpochMismatch {
        shared: crate::presentation::companion_scene::DeviceEpoch,
        requested: crate::presentation::companion_scene::DeviceEpoch,
    },
    InvalidAtlas,
    InvalidHudAtlas,
    InvalidUpload,
    InvalidDrawPlan(SceneDrawPlanError),
    InvalidTargetKey(SceneTargetKeyError),
    Gpu(ScopedGpuErrorCategory),
}

fn sanitize_gpu_error(error: wgpu::Error) -> ScopedGpuErrorCategory {
    match error {
        wgpu::Error::Validation { .. } => ScopedGpuErrorCategory::Validation,
        wgpu::Error::Internal { .. } => ScopedGpuErrorCategory::Internal,
        wgpu::Error::OutOfMemory { .. } => ScopedGpuErrorCategory::OutOfMemory,
    }
}

fn select_scoped_gpu_error(
    validation: Option<ScopedGpuErrorCategory>,
    out_of_memory: Option<ScopedGpuErrorCategory>,
    internal: Option<ScopedGpuErrorCategory>,
) -> Option<ScopedGpuErrorCategory> {
    out_of_memory.or(internal).or(validation)
}

/// Opens the required nested scopes and explicitly pops all of them in reverse
/// order on this caller/render-owner thread. Only the typed category escapes;
/// backend descriptions are dropped with the original `wgpu::Error` values.
pub(super) fn create_in_gpu_error_scopes<T>(
    device: &wgpu::Device,
    create: impl FnOnce() -> T,
) -> Result<T, SceneGpuError> {
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let out_of_memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let value = create();

    let validation_result = validation.pop();
    let out_of_memory_result = out_of_memory.pop();
    let internal_result = internal.pop();
    let validation_error = pollster::block_on(validation_result).map(sanitize_gpu_error);
    let out_of_memory_error = pollster::block_on(out_of_memory_result).map(sanitize_gpu_error);
    let internal_error = pollster::block_on(internal_result).map(sanitize_gpu_error);
    match select_scoped_gpu_error(validation_error, out_of_memory_error, internal_error) {
        Some(category) => Err(SceneGpuError::Gpu(category)),
        None => Ok(value),
    }
}

/// Persistent handles for the closed companion scene v2 pipeline matrix.
pub(super) struct SceneBasePipelines {
    pub(super) world_opaque_analytic: wgpu::RenderPipeline,
    pub(super) world_source_over_analytic: wgpu::RenderPipeline,
    pub(super) world_source_over_glyph: wgpu::RenderPipeline,
    pub(super) world_multiply_analytic: wgpu::RenderPipeline,
    pub(super) world_multiply_glyph_mask: wgpu::RenderPipeline,
    pub(super) world_additive_glyph: wgpu::RenderPipeline,
    pub(super) world_additive_analytic_reserved: wgpu::RenderPipeline,
    pub(super) chrome_analytic: wgpu::RenderPipeline,
    pub(super) chrome_hud: wgpu::RenderPipeline,
    pub(super) aperture_composite: wgpu::RenderPipeline,
    pub(super) final_surface: wgpu::RenderPipeline,
}

impl SceneBasePipelines {
    /// Closed typed lookup. Callers must first obtain the class from
    /// [`validate_scene_draw_plan`]; raw material/blend tags never select a
    /// production pipeline directly.
    pub(super) const fn for_class(&self, class: ScenePipelineClass) -> &wgpu::RenderPipeline {
        match class {
            ScenePipelineClass::WorldOpaqueAnalytic => &self.world_opaque_analytic,
            ScenePipelineClass::WorldSourceOverAnalytic => &self.world_source_over_analytic,
            ScenePipelineClass::WorldSourceOverGlyph => &self.world_source_over_glyph,
            ScenePipelineClass::WorldMultiplyAnalytic => &self.world_multiply_analytic,
            ScenePipelineClass::WorldMultiplyGlyphMask => &self.world_multiply_glyph_mask,
            ScenePipelineClass::WorldAdditiveGlyph => &self.world_additive_glyph,
            ScenePipelineClass::WorldAdditiveAnalyticReserved => {
                &self.world_additive_analytic_reserved
            }
            ScenePipelineClass::ChromeAnalytic => &self.chrome_analytic,
            ScenePipelineClass::SealedHudHook => &self.chrome_hud,
        }
    }
}

/// Device-epoch shared handles. A cheap `Device` handle clone is retained only
/// as an exact identity seal; the render owner still supplies the active
/// `Device`/`Queue` to operations.
pub(super) struct SceneGpuShared {
    pub(super) device_epoch: crate::presentation::companion_scene::DeviceEpoch,
    device_identity: wgpu::Device,
    /// Immutable adapter/device limit used to reject impossible targets before
    /// any texture creation reaches wgpu validation.
    pub(super) max_texture_dimension_2d: u32,
    pub(super) scene_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) atlas_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) final_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) hud_bind_group_layout: wgpu::BindGroupLayout,
    pub(super) linear_sampler: wgpu::Sampler,
    pub(super) pipelines: SceneBasePipelines,
}

impl SceneGpuShared {
    pub(super) fn create(
        device: &wgpu::Device,
        device_epoch: crate::presentation::companion_scene::DeviceEpoch,
    ) -> Result<Self, SceneGpuError> {
        create_in_gpu_error_scopes(device, || Self::create_unscoped(device, device_epoch))
    }

    fn create_unscoped(
        device: &wgpu::Device,
        device_epoch: crate::presentation::companion_scene::DeviceEpoch,
    ) -> Self {
        let scene_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("glorp-scene-storage-layout"),
                entries: &(0..6)
                    .map(|binding| wgpu::BindGroupLayoutEntry {
                        binding,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                scene_storage_min_binding_size(binding),
                            ),
                        },
                        count: None,
                    })
                    .collect::<Vec<_>>(),
            });
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("glorp-scene-atlas-layout"),
                entries: &[
                    filterable_texture_layout_entry(0),
                    filterable_texture_layout_entry(1),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let final_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("glorp-scene-final-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        let hud_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("glorp-scene-hud-storage-layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(super::hud::HUD_GPU_BUFFER_BYTES),
                    },
                    count: None,
                }],
            });
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glorp-scene-linear-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let pipelines = create_scene_base_pipelines(
            device,
            &scene_bind_group_layout,
            &atlas_bind_group_layout,
            &final_bind_group_layout,
            &hud_bind_group_layout,
        );
        Self {
            device_epoch,
            device_identity: device.clone(),
            max_texture_dimension_2d: device.limits().max_texture_dimension_2d,
            scene_bind_group_layout,
            atlas_bind_group_layout,
            final_bind_group_layout,
            hud_bind_group_layout,
            linear_sampler,
            pipelines,
        }
    }

    pub(super) const fn facts(&self) -> SceneGpuSharedFacts {
        SceneGpuSharedFacts::EXPECTED
    }
}

const fn scene_storage_min_binding_size(binding: u32) -> u64 {
    match binding {
        0 => PackedMirrorLayout::NODE_BYTES as u64,
        1 => PackedMirrorLayout::CONTENT_GLOBALS_BYTES as u64,
        2 => PackedMirrorLayout::FRAME_BYTES as u64,
        3 => std::mem::size_of::<PrimitiveGpuValue>() as u64,
        4 => PackedMirrorLayout::SCENE_CONTENT_BYTES as u64,
        5 => std::mem::size_of::<GlyphAtlasGpuEntry>() as u64,
        _ => 0,
    }
}

fn filterable_texture_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn create_scene_base_pipelines(
    device: &wgpu::Device,
    scene_layout: &wgpu::BindGroupLayout,
    atlas_layout: &wgpu::BindGroupLayout,
    final_layout: &wgpu::BindGroupLayout,
    hud_layout: &wgpu::BindGroupLayout,
) -> SceneBasePipelines {
    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2,
        2 => Float32x3,
        3 => Uint32,
        4 => Uint32
    ];
    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<super::compiler::StaticVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    };
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("glorp-scene-base-shader"),
        source: wgpu::ShaderSource::Wgsl(SCENE_SHADER_SOURCE.into()),
    });
    let scene_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glorp-scene-base-pipeline-layout"),
        bind_group_layouts: &[Some(scene_layout), Some(atlas_layout)],
        immediate_size: 0,
    });
    // `scene.wgsl` declares the final texture at group 2. Preserve all three
    // slots even though the final entry points consume only group 2.
    let final_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glorp-scene-final-pipeline-layout"),
        bind_group_layouts: &[None, None, Some(final_layout)],
        immediate_size: 0,
    });
    let aperture_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glorp-scene-aperture-pipeline-layout"),
        bind_group_layouts: &[Some(scene_layout), None, Some(final_layout)],
        immediate_size: 0,
    });
    let hud_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glorp-scene-hud-pipeline-layout"),
        bind_group_layouts: &[
            Some(scene_layout),
            Some(atlas_layout),
            None,
            Some(hud_layout),
        ],
        immediate_size: 0,
    });
    let scene_pipeline = |label: &'static str,
                          vertex_entry: &'static str,
                          fragment_entry: &'static str,
                          blend: Option<wgpu::BlendState>,
                          depth_write_enabled: Option<bool>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(vertex_entry),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: depth_write_enabled.map(|depth_write_enabled| wgpu::DepthStencilState {
                format: SceneTextureContract::DEPTH,
                depth_write_enabled: Some(depth_write_enabled),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(fragment_entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SceneTextureContract::INTERMEDIATE,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let pipeline_for_class = |label, class| {
        let contract = scene_pipeline_contract(class);
        scene_pipeline(
            label,
            contract.vertex_entry,
            contract.fragment_entry,
            contract.blend.map(scene_blend_state),
            contract.depth_write_enabled,
        )
    };
    let world_opaque_analytic = pipeline_for_class(
        "glorp-scene-world-opaque-analytic",
        ScenePipelineClass::WorldOpaqueAnalytic,
    );
    let world_source_over_analytic = pipeline_for_class(
        "glorp-scene-world-source-over-analytic",
        ScenePipelineClass::WorldSourceOverAnalytic,
    );
    let world_source_over_glyph = pipeline_for_class(
        "glorp-scene-world-source-over-glyph",
        ScenePipelineClass::WorldSourceOverGlyph,
    );
    let world_multiply_analytic = pipeline_for_class(
        "glorp-scene-world-multiply-analytic",
        ScenePipelineClass::WorldMultiplyAnalytic,
    );
    let world_multiply_glyph_mask = pipeline_for_class(
        "glorp-scene-world-multiply-glyph-mask",
        ScenePipelineClass::WorldMultiplyGlyphMask,
    );
    let world_additive_glyph = pipeline_for_class(
        "glorp-scene-world-additive-glyph",
        ScenePipelineClass::WorldAdditiveGlyph,
    );
    let world_additive_analytic_reserved = pipeline_for_class(
        "glorp-scene-world-additive-analytic-reserved",
        ScenePipelineClass::WorldAdditiveAnalyticReserved,
    );
    let chrome_analytic = pipeline_for_class(
        "glorp-scene-chrome-analytic",
        ScenePipelineClass::ChromeAnalytic,
    );
    let hud_contract = scene_pipeline_contract(ScenePipelineClass::SealedHudHook);
    let chrome_hud = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("glorp-scene-chrome-hud"),
        layout: Some(&hud_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(hud_contract.vertex_entry),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(hud_contract.fragment_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: SceneTextureContract::INTERMEDIATE,
                blend: hud_contract.blend.map(scene_blend_state),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let aperture_contract = APERTURE_COMPOSITE_PIPELINE_CONTRACT.pipeline;
    let aperture_composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("glorp-scene-aperture-composite"),
        layout: Some(&aperture_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(aperture_contract.vertex_entry),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(aperture_contract.fragment_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: APERTURE_COMPOSITE_PIPELINE_CONTRACT.target_format,
                blend: aperture_contract.blend.map(scene_blend_state),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let final_surface = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("glorp-scene-final-surface"),
        layout: Some(&final_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_final"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_final"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    SceneBasePipelines {
        world_opaque_analytic,
        world_source_over_analytic,
        world_source_over_glyph,
        world_multiply_analytic,
        world_multiply_glyph_mask,
        world_additive_glyph,
        world_additive_analytic_reserved,
        chrome_analytic,
        chrome_hud,
        aperture_composite,
        final_surface,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SceneBufferShadow {
    committed: Box<[u8]>,
    pending: Box<[u8]>,
}

impl SceneBufferShadow {
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            committed: Box::from(bytes),
            pending: Box::from(bytes),
        }
    }

    /// Promotes a fully prepared pending image without allocating or changing
    /// either fixed-size backing allocation. Task 10G-C owns synchronization
    /// of the now-spare pending image after a successful GPU submission.
    fn commit(&mut self) {
        std::mem::swap(&mut self.committed, &mut self.pending);
    }

    fn reset_pending(&mut self) {
        self.pending.copy_from_slice(&self.committed);
    }
}

/// CPU metadata owned by exactly one materialized GPU scene generation. Only
/// buffers carrying ordinary-frame deltas have shadows; immutable geometry,
/// primitive, and glyph-entry buffers deliberately have none.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GpuSceneGenerationState {
    glyph_lookup: SceneGlyphLookup,
    nodes: SceneBufferShadow,
    content_globals: SceneBufferShadow,
    frame: SceneBufferShadow,
    scene_content: SceneBufferShadow,
}

impl GpuSceneGenerationState {
    fn from_upload(upload: &PreparedSceneUpload) -> Self {
        Self {
            glyph_lookup: upload.glyph_lookup.clone(),
            nodes: SceneBufferShadow::from_bytes(&upload.node_bytes),
            content_globals: SceneBufferShadow::from_bytes(&upload.content_globals_bytes),
            frame: SceneBufferShadow::from_bytes(&upload.frame_bytes),
            scene_content: SceneBufferShadow::from_bytes(&upload.scene_content_bytes),
        }
    }

    fn reset_pending(&mut self) {
        self.nodes.reset_pending();
        self.content_globals.reset_pending();
        self.frame.reset_pending();
        self.scene_content.reset_pending();
    }

    fn commit_pending(&mut self) {
        self.nodes.commit();
        self.content_globals.commit();
        self.frame.commit();
        self.scene_content.commit();
        self.reset_pending();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedMirrorDestination {
    Nodes,
    Content(ContentMirrorFamily),
    Frame(FrameMirrorFamily),
}

fn prepared_mirror_destination(family: PreparedMirrorFamily) -> PreparedMirrorDestination {
    match family {
        PreparedMirrorFamily::Nodes => PreparedMirrorDestination::Nodes,
        PreparedMirrorFamily::ContentGlobals => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::Globals)
        }
        PreparedMirrorFamily::PetBody => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::Pet)
        }
        PreparedMirrorFamily::PetParticles => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::PetParticles)
        }
        PreparedMirrorFamily::RoomContent => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::RoomGlyphs)
        }
        PreparedMirrorFamily::PropGlyphs => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::PropGlyphs)
        }
        PreparedMirrorFamily::TankGlyphs => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::TankGlyphs)
        }
        PreparedMirrorFamily::ContentAmbient => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::Ambient)
        }
        PreparedMirrorFamily::ContentAnalytics => {
            PreparedMirrorDestination::Content(ContentMirrorFamily::Analytics)
        }
        PreparedMirrorFamily::FrameGlobals => {
            PreparedMirrorDestination::Frame(FrameMirrorFamily::Globals)
        }
        PreparedMirrorFamily::RoomFrame => {
            PreparedMirrorDestination::Frame(FrameMirrorFamily::RoomGlyphs)
        }
        PreparedMirrorFamily::Props => PreparedMirrorDestination::Frame(FrameMirrorFamily::Props),
        PreparedMirrorFamily::TankCells => {
            PreparedMirrorDestination::Frame(FrameMirrorFamily::TankCells)
        }
        PreparedMirrorFamily::FrameAmbient => {
            PreparedMirrorDestination::Frame(FrameMirrorFamily::Ambient)
        }
        PreparedMirrorFamily::FrameAnalytics => {
            PreparedMirrorDestination::Frame(FrameMirrorFamily::Analytics)
        }
        PreparedMirrorFamily::Lights => PreparedMirrorDestination::Frame(FrameMirrorFamily::Lights),
    }
}

fn stage_prepared_scene_delta(
    state: &mut GpuSceneGenerationState,
    prepared: &PreparedSceneDelta,
) -> Result<ScenePhysicalDirtySpans, SceneDeltaRenderError> {
    let physical = ScenePhysicalDirtySpans::from_logical(prepared.dirty_spans())
        .map_err(SceneDeltaRenderError::Layout)?;
    state.reset_pending();
    let mut result = Ok(());
    prepared.visit_mirror_updates(|family, slot, bytes| {
        if result.is_ok() {
            result = stage_prepared_mirror_record(state, family, slot, bytes);
        }
    });
    if let Err(error) = result {
        state.reset_pending();
        return Err(error);
    }
    Ok(physical)
}

fn stage_prepared_mirror_record(
    state: &mut GpuSceneGenerationState,
    family: PreparedMirrorFamily,
    slot: usize,
    bytes: &[u8],
) -> Result<(), SceneDeltaRenderError> {
    let destination = prepared_mirror_destination(family);
    let (shadow, family_offset, family_len, record_size, glyph_family) = match destination {
        PreparedMirrorDestination::Nodes => (
            &mut state.nodes,
            0,
            PackedMirrorLayout::NODE_BYTES,
            super::compiler::CpuMirrorShape::NODE_RECORD_BYTES,
            None,
        ),
        PreparedMirrorDestination::Content(family) => {
            let layout = PackedMirrorLayout::content_family_layout(family);
            let shadow = match layout.buffer {
                SceneMutableBuffer::ContentGlobals => &mut state.content_globals,
                SceneMutableBuffer::SceneContent => &mut state.scene_content,
                SceneMutableBuffer::Nodes | SceneMutableBuffer::Frame => {
                    unreachable!("content family has a content buffer")
                }
            };
            let glyph_family = (!matches!(
                family,
                ContentMirrorFamily::Globals | ContentMirrorFamily::Analytics
            ))
            .then_some(family);
            (
                shadow,
                layout.offset,
                layout.len,
                layout.span_alignment,
                glyph_family,
            )
        }
        PreparedMirrorDestination::Frame(family) => {
            let layout = PackedMirrorLayout::frame_family_layout(family);
            (
                &mut state.frame,
                layout.offset,
                layout.len,
                layout.span_alignment,
                None,
            )
        }
    };
    if bytes.len() != record_size {
        return Err(SceneDeltaRenderError::Upload(
            SceneUploadError::MirrorSizeMismatch,
        ));
    }
    let relative = slot
        .checked_mul(record_size)
        .filter(|offset| *offset < family_len)
        .ok_or(SceneDeltaRenderError::Upload(
            SceneUploadError::MirrorSizeMismatch,
        ))?;
    let offset = family_offset
        .checked_add(relative)
        .ok_or(SceneDeltaRenderError::Upload(
            SceneUploadError::MirrorSizeMismatch,
        ))?;
    let end = offset
        .checked_add(record_size)
        .filter(|end| *end <= shadow.pending.len())
        .ok_or(SceneDeltaRenderError::Upload(
            SceneUploadError::MirrorSizeMismatch,
        ))?;
    if let Some(family) = glyph_family {
        let value = bytemuck::try_from_bytes::<ContentGpuValue>(bytes)
            .map_err(|_| SceneDeltaRenderError::Upload(SceneUploadError::MirrorSizeMismatch))?;
        let translated = SceneContentGpuValue::translate(
            family,
            ContentUploadValue::from(*value),
            &state.glyph_lookup,
        )
        .map_err(SceneDeltaRenderError::Upload)?;
        shadow.pending[offset..end].copy_from_slice(bytemuck::bytes_of(&translated));
    } else {
        shadow.pending[offset..end].copy_from_slice(bytes);
    }
    Ok(())
}

pub(super) struct GpuSceneCandidate {
    device_identity: wgpu::Device,
    queue_identity: wgpu::Queue,
    pub(super) vertex_buffer: wgpu::Buffer,
    pub(super) index_buffer: wgpu::Buffer,
    pub(super) node_buffer: wgpu::Buffer,
    pub(super) content_globals_buffer: wgpu::Buffer,
    pub(super) frame_buffer: wgpu::Buffer,
    pub(super) primitive_buffer: wgpu::Buffer,
    pub(super) scene_content_buffer: wgpu::Buffer,
    pub(super) glyph_entry_buffer: wgpu::Buffer,
    pub(super) coverage_texture: wgpu::Texture,
    pub(super) coverage_view: wgpu::TextureView,
    pub(super) color_texture: wgpu::Texture,
    pub(super) color_view: wgpu::TextureView,
    pub(super) scene_bind_group: wgpu::BindGroup,
    pub(super) atlas_bind_group: wgpu::BindGroup,
    pub(super) hud: super::hud::GpuHudResources,
    pub(super) generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    pub(super) source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    pub(super) logical_viewport_points: [f32; 2],
    pub(super) static_checksum: u64,
    generation_state: GpuSceneGenerationState,
    /// Frozen once during materialization. Ordinary frames read this closed
    /// schedule directly and perform no full validation or heap allocation.
    pub(super) draw_plan: SceneDrawPlan,
}

impl GpuSceneCandidate {
    pub(super) const fn facts(&self) -> GpuSceneCandidateFacts {
        GpuSceneCandidateFacts::EXPECTED
    }
}

pub(super) fn encode_sensitive_hud_hook(
    encoder: &mut wgpu::CommandEncoder,
    staging_belt: &mut wgpu::util::StagingBelt,
    target: &wgpu::TextureView,
    shared: &SceneGpuShared,
    candidate: &mut GpuSceneCandidate,
    prepared: &super::hud::SensitivePreparedHudFrame,
) -> Result<(), super::hud::HudGpuStagingError> {
    let bindings = super::hud::HudDrawBindings::new(
        &shared.pipelines.chrome_hud,
        &candidate.scene_bind_group,
        &candidate.atlas_bind_group,
    );
    candidate
        .hud
        .encode_sensitive(staging_belt, encoder, target, bindings, prepared)
}

pub(super) fn encode_redacted_hud_hook(
    encoder: &mut wgpu::CommandEncoder,
    staging_belt: &mut wgpu::util::StagingBelt,
    target: &wgpu::TextureView,
    shared: &SceneGpuShared,
    candidate: &mut GpuSceneCandidate,
    prepared: &super::hud::CaptureSafePreparedHudFrame,
) -> Result<(), super::hud::HudGpuStagingError> {
    let bindings = super::hud::HudDrawBindings::new(
        &shared.pipelines.chrome_hud,
        &candidate.scene_bind_group,
        &candidate.atlas_bind_group,
    );
    candidate
        .hud
        .encode_redacted_capture(staging_belt, encoder, target, bindings, prepared)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneUploadPhase {
    OpaqueCutout,
    WorldBlended,
    Chrome,
}

fn expected_draw_source(primitive: PrimitiveGpuValue) -> Option<(PrimitiveSource, u32)> {
    if primitive.resource_kind == 0 {
        return None;
    }
    match primitive.primitive_kind {
        ATLAS_QUAD_PRIMITIVE_TAG
            if matches!(primitive.resource_kind, 1 | 2)
                && primitive.instance_group == 0
                && primitive.instance_base == NONE_U32 =>
        {
            Some((PrimitiveSource::StaticAtlas, 1))
        }
        ANALYTIC_PRIMITIVE_TAG
            if primitive.resource_kind == 3
                && primitive.instance_group == 0
                && primitive.instance_base == NONE_U32
                && primitive.binding_index == 1 =>
        {
            Some((
                PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask),
                u32::try_from(crate::presentation::companion_scene::scene::MAX_PET_ART_SLOTS)
                    .ok()?,
            ))
        }
        ANALYTIC_PRIMITIVE_TAG
            if primitive.resource_kind == 3
                && primitive.instance_group == 0
                && primitive.instance_base == NONE_U32 =>
        {
            Some((PrimitiveSource::Analytic, 1))
        }
        INSTANCE_QUAD_PRIMITIVE_TAG if matches!(primitive.resource_kind, 1 | 2) => {
            let (source, count) = instance_source_and_count_from_tags(
                primitive.instance_group,
                primitive.instance_base,
            )?;
            Some((PrimitiveSource::Instances(source), count))
        }
        _ => None,
    }
}

fn expected_upload_phase(primitive: PrimitiveGpuValue) -> Option<SceneUploadPhase> {
    if primitive.primitive_kind == ANALYTIC_PRIMITIVE_TAG
        && primitive.instance_group == 0
        && primitive.instance_base == NONE_U32
        && primitive.binding_index == 1
    {
        return (primitive.material_kind == 4
            && primitive.resource_kind == 3
            && primitive.blend == 4
            && primitive.depth == 2
            && primitive.space == 1)
            .then_some(SceneUploadPhase::WorldBlended);
    }
    let material_matches_primitive = match primitive.material_kind {
        1 => matches!(
            primitive.primitive_kind,
            ATLAS_QUAD_PRIMITIVE_TAG | INSTANCE_QUAD_PRIMITIVE_TAG
        ),
        2 => primitive.primitive_kind == ANALYTIC_PRIMITIVE_TAG,
        4..=6 => matches!(
            primitive.primitive_kind,
            ATLAS_QUAD_PRIMITIVE_TAG | ANALYTIC_PRIMITIVE_TAG | INSTANCE_QUAD_PRIMITIVE_TAG
        ),
        _ => false,
    };
    if !material_matches_primitive {
        return None;
    }
    if primitive.material_kind == 6 {
        return (primitive.blend == 3 && primitive.depth == 3 && primitive.space == 2)
            .then_some(SceneUploadPhase::Chrome);
    }
    if primitive.space != 1
        || (primitive.material_kind == 4 && primitive.blend != 4)
        || (primitive.material_kind == 5 && primitive.blend != 5)
    {
        return None;
    }
    match primitive.blend {
        1 | 2 if primitive.depth == 1 => Some(SceneUploadPhase::OpaqueCutout),
        3..=5 if primitive.depth == 2 => Some(SceneUploadPhase::WorldBlended),
        _ => None,
    }
}

pub(super) fn materialize_gpu_candidate(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shared: &SceneGpuShared,
    upload: &PreparedSceneUpload,
    atlas: &PreparedSceneAtlas,
) -> Result<GpuSceneCandidate, SceneGpuError> {
    if shared.device_identity != *device {
        return Err(SceneGpuError::ActualDeviceMismatch);
    }
    validate_gpu_candidate_preflight(shared, upload, atlas)?;
    let draw_plan = validate_scene_draw_plan(&upload.primitives, &upload.draws, &upload.phases)
        .map_err(SceneGpuError::InvalidDrawPlan)?;
    let prepared_hud_atlas = super::hud::PreparedHudAtlas::from_scene_atlas(atlas)
        .map_err(|_| SceneGpuError::InvalidHudAtlas)?;
    let generation_state = GpuSceneGenerationState::from_upload(upload);

    create_in_gpu_error_scopes(device, || {
        let vertex_buffer = create_initial_buffer(
            device,
            "glorp-scene-vertices",
            &upload.vertex_bytes,
            SceneBufferUsages::VERTEX,
        );
        let index_buffer = create_initial_buffer(
            device,
            "glorp-scene-indices",
            &upload.index_bytes,
            SceneBufferUsages::INDEX,
        );
        let node_buffer = create_initial_buffer(
            device,
            "glorp-scene-nodes",
            &upload.node_bytes,
            SceneBufferUsages::NODE,
        );
        let content_globals_buffer = create_initial_buffer(
            device,
            "glorp-scene-content-globals",
            &upload.content_globals_bytes,
            SceneBufferUsages::CONTENT_GLOBALS,
        );
        let frame_buffer = create_initial_buffer(
            device,
            "glorp-scene-frame",
            &upload.frame_bytes,
            SceneBufferUsages::FRAME,
        );
        let primitive_buffer = create_initial_buffer(
            device,
            "glorp-scene-primitives",
            bytemuck::cast_slice(&upload.primitives),
            SceneBufferUsages::PRIMITIVE,
        );
        let scene_content_buffer = create_initial_buffer(
            device,
            "glorp-scene-content",
            &upload.scene_content_bytes,
            SceneBufferUsages::SCENE_CONTENT,
        );
        let glyph_entry_buffer = create_initial_buffer(
            device,
            "glorp-scene-glyph-entries",
            bytemuck::cast_slice(&upload.glyph_entries),
            SceneBufferUsages::GLYPH_ENTRY,
        );
        let scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glorp-scene-storage-bind-group"),
            layout: &shared.scene_bind_group_layout,
            entries: &[
                buffer_bind_group_entry(0, &node_buffer),
                buffer_bind_group_entry(1, &content_globals_buffer),
                buffer_bind_group_entry(2, &frame_buffer),
                buffer_bind_group_entry(3, &primitive_buffer),
                buffer_bind_group_entry(4, &scene_content_buffer),
                buffer_bind_group_entry(5, &glyph_entry_buffer),
            ],
        });
        let atlas_extent = wgpu::Extent3d {
            width: atlas.width,
            height: atlas.height,
            depth_or_array_layers: 1,
        };
        let coverage_texture = create_atlas_texture(
            device,
            "glorp-scene-coverage-atlas",
            atlas_extent,
            SceneTextureContract::COVERAGE,
        );
        let coverage_view = coverage_texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &coverage_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas.coverage_r8,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.width),
                rows_per_image: Some(atlas.height),
            },
            atlas_extent,
        );
        let color_texture = create_atlas_texture(
            device,
            "glorp-scene-color-atlas",
            atlas_extent,
            SceneTextureContract::COLOR,
        );
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas.straight_color_rgba_srgb,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.width * 4),
                rows_per_image: Some(atlas.height),
            },
            atlas_extent,
        );
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glorp-scene-atlas-bind-group"),
            layout: &shared.atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&coverage_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shared.linear_sampler),
                },
            ],
        });
        let hud = super::hud::GpuHudResources::create_unscoped(
            device,
            &shared.hud_bind_group_layout,
            prepared_hud_atlas,
        );
        GpuSceneCandidate {
            device_identity: device.clone(),
            queue_identity: queue.clone(),
            vertex_buffer,
            index_buffer,
            node_buffer,
            content_globals_buffer,
            frame_buffer,
            primitive_buffer,
            scene_content_buffer,
            glyph_entry_buffer,
            coverage_texture,
            coverage_view,
            color_texture,
            color_view,
            scene_bind_group,
            atlas_bind_group,
            hud,
            generation_key: upload.generation_key,
            source_revisions: upload.source_revisions,
            logical_viewport_points: upload.logical_viewport_points,
            static_checksum: upload.static_checksum,
            generation_state,
            draw_plan,
        }
    })
}

fn validate_gpu_candidate_preflight(
    shared: &SceneGpuShared,
    upload: &PreparedSceneUpload,
    atlas: &PreparedSceneAtlas,
) -> Result<(), SceneGpuError> {
    if atlas.resource_generation != upload.generation_key.resources {
        return Err(SceneGpuError::AtlasGenerationMismatch {
            upload: upload.generation_key.resources,
            atlas: atlas.resource_generation,
        });
    }
    if upload.glyph_lookup.resource_generation != upload.generation_key.resources {
        return Err(SceneGpuError::GlyphLookupGenerationMismatch {
            upload: upload.generation_key.resources,
            lookup: upload.glyph_lookup.resource_generation,
            atlas: atlas.resource_generation,
        });
    }
    if shared.device_epoch != upload.generation_key.device {
        return Err(SceneGpuError::DeviceEpochMismatch {
            shared: shared.device_epoch,
            requested: upload.generation_key.device,
        });
    }
    let primitive_count = upload.primitives.len();
    if primitive_count == 0
        || primitive_count > crate::presentation::companion_scene::scene::MAX_STATIC_PRIMITIVES
    {
        return Err(SceneGpuError::InvalidUpload);
    }
    let pixel_count = usize::try_from(atlas.width)
        .ok()
        .and_then(|width| {
            usize::try_from(atlas.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(SceneGpuError::InvalidAtlas)?;
    if pixel_count == 0
        || atlas.coverage_r8.len() != pixel_count
        || atlas.straight_color_rgba_srgb.len() != pixel_count.saturating_mul(4)
    {
        return Err(SceneGpuError::InvalidAtlas);
    }
    let expected_vertices = primitive_count
        .checked_mul(4)
        .and_then(|count| count.checked_mul(std::mem::size_of::<super::compiler::StaticVertex>()));
    let expected_indices = primitive_count
        .checked_mul(6)
        .and_then(|count| count.checked_mul(std::mem::size_of::<super::compiler::StaticIndex>()));
    if upload.node_bytes.len() != PackedMirrorLayout::NODE_BYTES
        || upload.content_globals_bytes.len() != PackedMirrorLayout::CONTENT_GLOBALS_BYTES
        || upload.frame_bytes.len() != PackedMirrorLayout::FRAME_BYTES
        || upload.scene_content_bytes.len() != PackedMirrorLayout::SCENE_CONTENT_BYTES
        || Some(upload.vertex_bytes.len()) != expected_vertices
        || Some(upload.index_bytes.len()) != expected_indices
        || upload.draws.len() != primitive_count
        || upload.glyph_entries.is_empty()
        || upload.glyph_entries.len() != atlas.entries.len()
    {
        return Err(SceneGpuError::InvalidUpload);
    }
    // Frame globals begin the packed frame mirror. Keep the separately carried
    // request-validation extent bit-identical to the bytes the shader reads.
    const FRAME_VIEWPORT_POINTS_OFFSET: usize =
        super::compiler::CpuMirrorShape::FRAME_GLOBALS_VIEWPORT_POINTS_OFFSET;
    let uploaded_viewport_points = [
        f32::from_ne_bytes(
            upload.frame_bytes[FRAME_VIEWPORT_POINTS_OFFSET..FRAME_VIEWPORT_POINTS_OFFSET + 4]
                .try_into()
                .expect("preflight checked the fixed frame byte count"),
        ),
        f32::from_ne_bytes(
            upload.frame_bytes[FRAME_VIEWPORT_POINTS_OFFSET + 4..FRAME_VIEWPORT_POINTS_OFFSET + 8]
                .try_into()
                .expect("preflight checked the fixed frame byte count"),
        ),
    ];
    if uploaded_viewport_points.map(f32::to_bits)
        != upload.logical_viewport_points.map(f32::to_bits)
    {
        return Err(SceneGpuError::InvalidUpload);
    }
    if upload.primitives.iter().any(|primitive| {
        primitive.node_index as usize >= super::compiler::CpuMirrorShape::NODE_COUNT
    }) {
        return Err(SceneGpuError::InvalidUpload);
    }
    let vertex_stride = std::mem::size_of::<super::compiler::StaticVertex>();
    for (primitive_index, vertices) in upload
        .vertex_bytes
        .chunks_exact(vertex_stride * 4)
        .enumerate()
    {
        let expected_primitive =
            u32::try_from(primitive_index).map_err(|_| SceneGpuError::InvalidUpload)?;
        let expected_material = upload.primitives[primitive_index].material_index;
        for vertex in vertices.chunks_exact(vertex_stride) {
            let embedded_primitive =
                u32::from_ne_bytes(vertex[32..36].try_into().expect("vertex primitive index"));
            let embedded_material =
                u32::from_ne_bytes(vertex[36..40].try_into().expect("vertex material index"));
            if embedded_primitive != expected_primitive || embedded_material != expected_material {
                return Err(SceneGpuError::InvalidUpload);
            }
        }
    }
    let indices = upload
        .index_bytes
        .chunks_exact(std::mem::size_of::<super::compiler::StaticIndex>())
        .map(|bytes| u32::from_ne_bytes(bytes.try_into().expect("four-byte index chunk")))
        .collect::<Vec<_>>();
    let pet_body_node = upload
        .primitives
        .iter()
        .find(|primitive| primitive.instance_group == 1)
        .map(|primitive| primitive.node_index);
    for (primitive_index, (primitive, draw)) in
        upload.primitives.iter().zip(&upload.draws).enumerate()
    {
        let Some(first_index) = primitive_index
            .checked_mul(6)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return Err(SceneGpuError::InvalidUpload);
        };
        let Some(index_end) = first_index.checked_add(6) else {
            return Err(SceneGpuError::InvalidUpload);
        };
        let Some(first_vertex) = primitive_index
            .checked_mul(4)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return Err(SceneGpuError::InvalidUpload);
        };
        let Some(vertex_end) = first_vertex.checked_add(4) else {
            return Err(SceneGpuError::InvalidUpload);
        };
        let Some((expected_source, expected_instances)) = expected_draw_source(*primitive) else {
            return Err(SceneGpuError::InvalidUpload);
        };
        let Some(expected_bases) = arena_bases_from_tags(
            primitive.primitive_kind,
            primitive.instance_group,
            primitive.instance_base,
            primitive.binding_index,
        ) else {
            return Err(SceneGpuError::InvalidUpload);
        };
        let expected_aux_node = if primitive.primitive_kind == ANALYTIC_PRIMITIVE_TAG
            && primitive.instance_group == 0
            && primitive.binding_index == 1
        {
            pet_body_node.ok_or(SceneGpuError::InvalidUpload)?
        } else {
            NONE_U32
        };
        if draw.index_range != (first_index..index_end)
            || draw.instance_range != (0..expected_instances)
            || draw.source != expected_source
            || draw.authored_order != primitive.authored_order
            || (
                primitive.content_base,
                primitive.frame_base,
                primitive.aux_content_base,
            ) != expected_bases
            || primitive.aux_node_index != expected_aux_node
            || indices[first_index as usize..index_end as usize]
                .iter()
                .any(|index| !(first_vertex..vertex_end).contains(index))
        {
            return Err(SceneGpuError::InvalidUpload);
        }
    }
    let glyph_count =
        u32::try_from(upload.glyph_entries.len()).map_err(|_| SceneGpuError::InvalidUpload)?;
    let analytic_offset = PackedMirrorLayout::scene_content_offset(ContentMirrorFamily::Analytics)
        .ok_or(SceneGpuError::InvalidUpload)?;
    if upload.scene_content_bytes[..analytic_offset]
        .chunks_exact(std::mem::size_of::<SceneContentGpuValue>())
        .any(|record| {
            let glyph = u32::from_ne_bytes(record[4..8].try_into().expect("four-byte glyph id"));
            glyph != NONE_U32 && glyph >= glyph_count
        })
    {
        return Err(SceneGpuError::InvalidUpload);
    }
    let expected_phases = upload
        .primitives
        .iter()
        .copied()
        .map(expected_upload_phase)
        .collect::<Option<Vec<_>>>()
        .ok_or(SceneGpuError::InvalidUpload)?;
    let mut classified = vec![false; primitive_count];
    for (phase, primitive_indices) in [
        (SceneUploadPhase::OpaqueCutout, &upload.phases.opaque_cutout),
        (
            SceneUploadPhase::WorldBlended,
            &upload.phases.world_blended_unsorted,
        ),
        (SceneUploadPhase::Chrome, &upload.phases.chrome_authored),
    ] {
        for primitive_index in primitive_indices {
            let Some(index) = usize::try_from(*primitive_index).ok() else {
                return Err(SceneGpuError::InvalidUpload);
            };
            let Some(seen) = classified.get_mut(index) else {
                return Err(SceneGpuError::InvalidUpload);
            };
            if *seen || expected_phases[index] != phase {
                return Err(SceneGpuError::InvalidUpload);
            }
            *seen = true;
        }
    }
    if classified.contains(&false) {
        return Err(SceneGpuError::InvalidUpload);
    }
    Ok(())
}

fn create_initial_buffer(
    device: &wgpu::Device,
    label: &'static str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage,
    })
}

fn buffer_bind_group_entry<'a>(binding: u32, buffer: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn create_atlas_texture(
    device: &wgpu::Device,
    label: &'static str,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Complete host-owned inputs for one companion scene render. Logical extent
/// remains compiler-owned and aperture remains the role-0 analytic; the host
/// contributes only versioning and physical surface facts here.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SceneRenderRequest {
    pub(super) version: crate::presentation::companion_scene::SceneVersion,
    pub(super) physical_extent_pixels: [u32; 2],
    pub(super) backing_scale: f64,
}

impl SceneRenderRequest {
    pub(super) const fn new(
        version: crate::presentation::companion_scene::SceneVersion,
        physical_extent_pixels: [u32; 2],
        backing_scale: f64,
    ) -> Self {
        Self {
            version,
            physical_extent_pixels,
            backing_scale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneRequestAxis {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneRenderRequestError {
    InvalidBackingScale,
    InvalidLogicalViewport,
    EmptyPhysicalExtent {
        axis: SceneRequestAxis,
    },
    PhysicalExtentMismatch {
        axis: SceneRequestAxis,
        requested: u32,
        expected: u32,
    },
    PhysicalDimensionOverflow {
        axis: SceneRequestAxis,
    },
    PhysicalDimensionLimitExceeded {
        axis: SceneRequestAxis,
        required: u32,
        maximum: u32,
    },
    GenerationMismatch {
        requested: crate::presentation::companion_scene::SceneGenerationKey,
        candidate: crate::presentation::companion_scene::SceneGenerationKey,
    },
    AppliedRevisionsMismatch {
        requested: crate::presentation::companion_scene::AppliedRevisions,
        candidate: crate::presentation::companion_scene::AppliedRevisions,
    },
    SharedDeviceEpochMismatch {
        shared: crate::presentation::companion_scene::DeviceEpoch,
        candidate: crate::presentation::companion_scene::DeviceEpoch,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SceneTargetKey {
    pub(super) device_epoch: crate::presentation::companion_scene::DeviceEpoch,
    pub(super) surface_epoch: crate::presentation::companion_scene::SurfaceEpoch,
    pub(super) extent: wgpu::Extent3d,
    pub(super) surface_format: wgpu::TextureFormat,
    pub(super) intermediate_format: wgpu::TextureFormat,
    pub(super) depth_format: wgpu::TextureFormat,
    pub(super) sample_count: u32,
}

/// A request after it has been bound to one frozen GPU candidate and shared
/// device generation. The target key contains only fixed renderer formats.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SceneRenderBinding {
    pub(super) request: SceneRenderRequest,
    pub(super) target_key: SceneTargetKey,
}

pub(super) fn bind_scene_render_request(
    shared: &SceneGpuShared,
    candidate: &GpuSceneCandidate,
    request: SceneRenderRequest,
) -> Result<SceneRenderBinding, SceneRenderRequestError> {
    let target_key = derive_scene_target_key(
        shared.device_epoch,
        candidate.generation_key,
        candidate.source_revisions,
        candidate.logical_viewport_points,
        shared.max_texture_dimension_2d,
        &request,
    )?;
    Ok(SceneRenderBinding { request, target_key })
}

fn derive_scene_target_key(
    shared_device_epoch: crate::presentation::companion_scene::DeviceEpoch,
    candidate_generation: crate::presentation::companion_scene::SceneGenerationKey,
    candidate_revisions: crate::presentation::companion_scene::AppliedRevisions,
    logical_viewport_points: [f32; 2],
    max_texture_dimension_2d: u32,
    request: &SceneRenderRequest,
) -> Result<SceneTargetKey, SceneRenderRequestError> {
    if !request.backing_scale.is_finite() || request.backing_scale <= 0.0 {
        return Err(SceneRenderRequestError::InvalidBackingScale);
    }
    if logical_viewport_points
        .iter()
        .any(|dimension| !dimension.is_finite() || *dimension <= 0.0)
    {
        return Err(SceneRenderRequestError::InvalidLogicalViewport);
    }
    for (axis, requested) in [
        (SceneRequestAxis::Width, request.physical_extent_pixels[0]),
        (SceneRequestAxis::Height, request.physical_extent_pixels[1]),
    ] {
        if requested == 0 {
            return Err(SceneRenderRequestError::EmptyPhysicalExtent { axis });
        }
    }
    if request.version.generation != candidate_generation {
        return Err(SceneRenderRequestError::GenerationMismatch {
            requested: request.version.generation,
            candidate: candidate_generation,
        });
    }
    if request.version.applied != candidate_revisions {
        return Err(SceneRenderRequestError::AppliedRevisionsMismatch {
            requested: request.version.applied,
            candidate: candidate_revisions,
        });
    }
    if shared_device_epoch != candidate_generation.device {
        return Err(SceneRenderRequestError::SharedDeviceEpochMismatch {
            shared: shared_device_epoch,
            candidate: candidate_generation.device,
        });
    }
    for (index, axis) in [SceneRequestAxis::Width, SceneRequestAxis::Height]
        .into_iter()
        .enumerate()
    {
        let physical = f64::from(logical_viewport_points[index]) * request.backing_scale;
        let rounded = physical.round();
        if !physical.is_finite() || rounded > f64::from(u32::MAX) {
            return Err(SceneRenderRequestError::PhysicalDimensionOverflow { axis });
        }
        let expected = super::host::physical_dimension(
            f64::from(logical_viewport_points[index]),
            request.backing_scale,
        );
        if expected > max_texture_dimension_2d {
            return Err(SceneRenderRequestError::PhysicalDimensionLimitExceeded {
                axis,
                required: expected,
                maximum: max_texture_dimension_2d,
            });
        }
        let requested = request.physical_extent_pixels[index];
        if requested != expected {
            return Err(SceneRenderRequestError::PhysicalExtentMismatch {
                axis,
                requested,
                expected,
            });
        }
    }

    Ok(SceneTargetKey::new(
        candidate_generation.device,
        request.version.surface,
        wgpu::Extent3d {
            width: request.physical_extent_pixels[0],
            height: request.physical_extent_pixels[1],
            depth_or_array_layers: 1,
        },
        wgpu::TextureFormat::Bgra8UnormSrgb,
        SceneTextureContract::INTERMEDIATE,
        SceneTextureContract::DEPTH,
        SceneTextureContract::SAMPLE_COUNT,
    )
    .expect("validated request and fixed scene target formats form a valid key"))
}

impl SceneTargetKey {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        device_epoch: crate::presentation::companion_scene::DeviceEpoch,
        surface_epoch: crate::presentation::companion_scene::SurfaceEpoch,
        extent: wgpu::Extent3d,
        surface_format: wgpu::TextureFormat,
        intermediate_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Result<Self, SceneTargetKeyError> {
        let key = Self {
            device_epoch,
            surface_epoch,
            extent,
            surface_format,
            intermediate_format,
            depth_format,
            sample_count,
        };
        key.validate()?;
        Ok(key)
    }

    fn validate(self) -> Result<(), SceneTargetKeyError> {
        if self.extent.width == 0
            || self.extent.height == 0
            || self.extent.depth_or_array_layers != 1
        {
            return Err(SceneTargetKeyError::Extent);
        }
        if self.surface_format != wgpu::TextureFormat::Bgra8UnormSrgb
            || self.intermediate_format != SceneTextureContract::INTERMEDIATE
            || self.depth_format != SceneTextureContract::DEPTH
        {
            return Err(SceneTargetKeyError::Formats);
        }
        if self.sample_count != SceneTextureContract::SAMPLE_COUNT {
            return Err(SceneTargetKeyError::SampleCount);
        }
        Ok(())
    }
}

pub(super) struct SceneTargets {
    pub(super) key: SceneTargetKey,
    pub(super) raw_scene_texture: wgpu::Texture,
    pub(super) raw_scene_view: wgpu::TextureView,
    pub(super) intermediate_texture: wgpu::Texture,
    pub(super) intermediate_view: wgpu::TextureView,
    pub(super) depth_texture: wgpu::Texture,
    pub(super) depth_view: wgpu::TextureView,
    pub(super) aperture_bind_group: wgpu::BindGroup,
    pub(super) final_bind_group: wgpu::BindGroup,
}

impl SceneTargets {
    pub(super) const fn facts(&self) -> SceneTargetFacts {
        SceneTargetFacts::EXPECTED
    }

    fn create(
        device: &wgpu::Device,
        shared: &SceneGpuShared,
        key: SceneTargetKey,
        fault: SceneTargetTestFault,
    ) -> Result<Self, SceneGpuError> {
        key.validate().map_err(SceneGpuError::InvalidTargetKey)?;
        if shared.device_epoch != key.device_epoch {
            return Err(SceneGpuError::DeviceEpochMismatch {
                shared: shared.device_epoch,
                requested: key.device_epoch,
            });
        }
        create_in_gpu_error_scopes(device, || {
            let raw_scene_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("glorp-scene-raw"),
                size: key.extent,
                mip_level_count: 1,
                sample_count: key.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: key.intermediate_format,
                usage: SceneTargetTextureUsages::RAW_SCENE,
                view_formats: &[],
            });
            let raw_scene_view =
                raw_scene_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("glorp-scene-intermediate"),
                size: key.extent,
                mip_level_count: 1,
                sample_count: key.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: key.intermediate_format,
                usage: SceneTargetTextureUsages::INTERMEDIATE,
                view_formats: &[],
            });
            let intermediate_view =
                intermediate_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("glorp-scene-depth"),
                size: key.extent,
                mip_level_count: 1,
                sample_count: key.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: key.depth_format,
                usage: SceneTargetTextureUsages::DEPTH,
                view_formats: &[],
            });
            let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let aperture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("glorp-scene-aperture-bind-group"),
                layout: &shared.final_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&raw_scene_view),
                }],
            });
            let final_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("glorp-scene-final-bind-group"),
                layout: &shared.final_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&intermediate_view),
                }],
            });
            if fault == SceneTargetTestFault::ValidationAfterAllocation {
                let _invalid = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("glorp-scene-test-invalid-final-bind-group"),
                    layout: &shared.final_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&depth_view),
                    }],
                });
            }
            Self {
                key,
                raw_scene_texture,
                raw_scene_view,
                intermediate_texture,
                intermediate_view,
                depth_texture,
                depth_view,
                aperture_bind_group,
                final_bind_group,
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneTargetTestFault {
    None,
    ValidationAfterAllocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneTargetUpdate {
    Reused,
    Created,
}

#[derive(Default)]
pub(super) struct SceneTargetCache {
    current: Option<SceneTargets>,
    creation_events: u64,
}

impl SceneTargetCache {
    pub(super) fn ensure(
        &mut self,
        device: &wgpu::Device,
        shared: &SceneGpuShared,
        key: SceneTargetKey,
    ) -> Result<SceneTargetUpdate, SceneGpuError> {
        self.ensure_with_fault(device, shared, key, SceneTargetTestFault::None)
    }

    #[cfg(test)]
    fn ensure_with_test_fault(
        &mut self,
        device: &wgpu::Device,
        shared: &SceneGpuShared,
        key: SceneTargetKey,
        fault: SceneTargetTestFault,
    ) -> Result<SceneTargetUpdate, SceneGpuError> {
        self.ensure_with_fault(device, shared, key, fault)
    }

    fn ensure_with_fault(
        &mut self,
        device: &wgpu::Device,
        shared: &SceneGpuShared,
        key: SceneTargetKey,
        fault: SceneTargetTestFault,
    ) -> Result<SceneTargetUpdate, SceneGpuError> {
        key.validate().map_err(SceneGpuError::InvalidTargetKey)?;
        if shared.device_epoch != key.device_epoch {
            return Err(SceneGpuError::DeviceEpochMismatch {
                shared: shared.device_epoch,
                requested: key.device_epoch,
            });
        }
        if self
            .current
            .as_ref()
            .is_some_and(|targets| targets.key == key)
        {
            return Ok(SceneTargetUpdate::Reused);
        }
        let replacement = SceneTargets::create(device, shared, key, fault)?;
        self.current = Some(replacement);
        self.creation_events = self.creation_events.saturating_add(1);
        Ok(SceneTargetUpdate::Created)
    }

    pub(super) const fn current(&self) -> Option<&SceneTargets> {
        self.current.as_ref()
    }

    pub(super) const fn creation_events(&self) -> u64 {
        self.creation_events
    }
}

const SCENE_STAGING_BELT_CHUNK_BYTES: wgpu::BufferAddress = 64 * 1024;
const SCENE_READBACK_TIMEOUT: Duration = Duration::from_secs(5);
const SCENE_MAP_CALLBACK_TIMEOUT: Duration = Duration::from_millis(100);

/// Successful output from one synthetic offscreen scene render. This is the
/// renderer's canonical top-left, straight-alpha RGBA capture seam; it does not
/// imply that a host surface was activated or presented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SceneRenderOutcome {
    pub(super) version: crate::presentation::companion_scene::SceneVersion,
    pub(super) physical_extent_pixels: [u32; 2],
    pub(super) rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SceneDeltaRenderError {
    ActualDeviceMismatch,
    ActualQueueMismatch,
    GenerationMismatch,
    StaticChecksumMismatch,
    RevisionMismatch,
    LogicalViewportMismatch,
    PreparedRevisionMismatch,
    Layout(PackedMirrorLayoutError),
    Upload(SceneUploadError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SceneRenderError {
    RendererDeviceEpochMismatch {
        renderer: crate::presentation::companion_scene::DeviceEpoch,
        shared: crate::presentation::companion_scene::DeviceEpoch,
    },
    Request(SceneRenderRequestError),
    DeltaPreparation(MirrorDeltaError),
    Delta(SceneDeltaRenderError),
    Hud(super::hud::HudGpuStagingError),
    Target(SceneGpuError),
    Readback(super::capture::SceneReadbackError),
    Gpu(ScopedGpuErrorCategory),
    PollTimeout,
    PollWrongSubmissionIndex,
    MapFailed,
    MappedRangeFailed,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SceneRenderTestFault {
    #[default]
    None,
    ScopedValidationAfterHudWrite,
    PollTimeout,
    PollWrongSubmissionIndex,
    MapCallbackCancelled,
    MappedRangeFailure,
    NormalizeShortBuffer,
}

/// Persistent offscreen execution state for scene-v2. Device and queue remain
/// host-owned; only size/generation-keyed targets, the readback buffer, and one
/// fixed-size upload belt are retained here.
pub(super) struct SceneRenderer {
    device_epoch: crate::presentation::companion_scene::DeviceEpoch,
    device_identity: wgpu::Device,
    queue_identity: wgpu::Queue,
    targets: SceneTargetCache,
    readback: super::capture::SceneReadbackCache,
    staging_belt: wgpu::util::StagingBelt,
    #[cfg(test)]
    submission_events: u64,
    #[cfg(test)]
    scene_buffer_copy_events: u64,
    #[cfg(test)]
    test_fault: SceneRenderTestFault,
}

struct SceneDeltaTransaction<'candidate> {
    cpu: &'candidate mut CpuSceneCandidate,
    prepared: PreparedSceneDelta,
}

impl SceneRenderer {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, shared: &SceneGpuShared) -> Self {
        Self {
            device_epoch: shared.device_epoch,
            device_identity: device.clone(),
            queue_identity: queue.clone(),
            targets: SceneTargetCache::default(),
            readback: super::capture::SceneReadbackCache::default(),
            staging_belt: wgpu::util::StagingBelt::new(
                device.clone(),
                SCENE_STAGING_BELT_CHUNK_BYTES,
            ),
            #[cfg(test)]
            submission_events: 0,
            #[cfg(test)]
            scene_buffer_copy_events: 0,
            #[cfg(test)]
            test_fault: SceneRenderTestFault::None,
        }
    }

    /// Renders the frozen candidate without touching a host surface. All
    /// request, sealed-HUD, and readback geometry checks run before an encoder
    /// exists or the staging belt is written.
    pub(super) fn render_offscreen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &SceneGpuShared,
        candidate: &mut GpuSceneCandidate,
        request: SceneRenderRequest,
        prepared_hud: &super::hud::CaptureSafePreparedHudFrame,
    ) -> Result<SceneRenderOutcome, SceneRenderError> {
        self.render_offscreen_inner(
            device,
            queue,
            shared,
            candidate,
            request,
            prepared_hud,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_offscreen_with_delta(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &SceneGpuShared,
        cpu: &mut CpuSceneCandidate,
        candidate: &mut GpuSceneCandidate,
        content_delta: &crate::presentation::companion_scene::scene::ContentDelta,
        frame_delta: &crate::presentation::companion_scene::scene::FrameDelta,
        request: SceneRenderRequest,
        prepared_hud: &super::hud::CaptureSafePreparedHudFrame,
    ) -> Result<SceneRenderOutcome, SceneRenderError> {
        self.validate_actual_identity(device, queue, shared, candidate)?;
        if cpu.generation_key != candidate.generation_key {
            return Err(SceneRenderError::Delta(
                SceneDeltaRenderError::GenerationMismatch,
            ));
        }
        if cpu.static_checksum != candidate.static_checksum {
            return Err(SceneRenderError::Delta(
                SceneDeltaRenderError::StaticChecksumMismatch,
            ));
        }
        if cpu.source_revisions != candidate.source_revisions {
            return Err(SceneRenderError::Delta(
                SceneDeltaRenderError::RevisionMismatch,
            ));
        }
        if cpu.logical_viewport_points() != candidate.logical_viewport_points {
            return Err(SceneRenderError::Delta(
                SceneDeltaRenderError::LogicalViewportMismatch,
            ));
        }
        let prepared = cpu
            .prepare_deltas(content_delta, frame_delta)
            .map_err(SceneRenderError::DeltaPreparation)?;
        let dirty = prepared.dirty_spans();
        if dirty.from != candidate.source_revisions || dirty.to != request.version.applied {
            return Err(SceneRenderError::Delta(
                SceneDeltaRenderError::PreparedRevisionMismatch,
            ));
        }
        self.render_offscreen_inner(
            device,
            queue,
            shared,
            candidate,
            request,
            prepared_hud,
            Some(SceneDeltaTransaction { cpu, prepared }),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_offscreen_inner(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &SceneGpuShared,
        candidate: &mut GpuSceneCandidate,
        request: SceneRenderRequest,
        prepared_hud: &super::hud::CaptureSafePreparedHudFrame,
        mut delta: Option<SceneDeltaTransaction<'_>>,
    ) -> Result<SceneRenderOutcome, SceneRenderError> {
        if self.device_epoch != shared.device_epoch {
            return Err(SceneRenderError::RendererDeviceEpochMismatch {
                renderer: self.device_epoch,
                shared: shared.device_epoch,
            });
        }
        self.validate_actual_identity(device, queue, shared, candidate)?;
        let (revisions, logical_viewport_points) = delta
            .as_ref()
            .map(|transaction| {
                (
                    transaction.prepared.dirty_spans().to,
                    transaction.prepared.prospective_logical_viewport_points(),
                )
            })
            .unwrap_or((
                candidate.source_revisions,
                candidate.logical_viewport_points,
            ));
        let target_key = derive_scene_target_key(
            shared.device_epoch,
            candidate.generation_key,
            revisions,
            logical_viewport_points,
            shared.max_texture_dimension_2d,
            &request,
        )
        .map_err(SceneRenderError::Request)?;
        let binding = SceneRenderBinding { request, target_key };
        candidate
            .hud
            .validate_redacted_capture(prepared_hud)
            .map_err(SceneRenderError::Hud)?;
        let readback_key = super::capture::SceneReadbackKey::new(
            binding.target_key.device_epoch,
            binding.request.physical_extent_pixels,
        );
        // Derive the exact copy layout before either cache can allocate.
        super::capture::SceneReadbackLayout::checked(
            readback_key.physical_extent_pixels[0],
            readback_key.physical_extent_pixels[1],
            readback_key.intermediate_format,
            Some(device.limits().max_buffer_size),
        )
        .map_err(SceneRenderError::Readback)?;

        let physical_delta = delta
            .as_ref()
            .map(|transaction| {
                stage_prepared_scene_delta(&mut candidate.generation_state, &transaction.prepared)
            })
            .transpose()
            .map_err(SceneRenderError::Delta)?;

        if let Err(error) = self.targets.ensure(device, shared, binding.target_key) {
            if physical_delta.is_some() {
                candidate.generation_state.reset_pending();
            }
            return Err(SceneRenderError::Target(error));
        }
        if let Err(error) = self.readback.ensure(device, shared, readback_key) {
            if physical_delta.is_some() {
                candidate.generation_state.reset_pending();
            }
            return Err(SceneRenderError::Readback(error));
        }
        let targets = self
            .targets
            .current()
            .expect("successful target ensure installs the requested target");
        let readback = self
            .readback
            .current()
            .expect("successful readback ensure installs the requested buffer");

        #[cfg(test)]
        let test_fault = std::mem::take(&mut self.test_fault);
        let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let out_of_memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let submission_result = (|| {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glorp-scene-offscreen-encoder"),
            });
            let _scene_buffer_copies = physical_delta
                .map(|physical| {
                    encode_scene_delta_copies(
                        &mut encoder,
                        &mut self.staging_belt,
                        candidate,
                        physical,
                    )
                })
                .unwrap_or(0);
            encode_scene_world(&mut encoder, targets, shared, candidate);
            encode_scene_draws_without_depth(
                &mut encoder,
                "glorp-scene-chrome-prefix",
                &targets.raw_scene_view,
                shared,
                candidate,
                &candidate.draw_plan.chrome.prefix,
            );
            // This is the only HUD draw in the general scene schedule. The hook
            // rechecks generation immediately before its one fixed upload.
            encode_redacted_hud_hook(
                &mut encoder,
                &mut self.staging_belt,
                &targets.raw_scene_view,
                shared,
                candidate,
                prepared_hud,
            )?;
            encode_scene_draws_without_depth(
                &mut encoder,
                "glorp-scene-chrome-suffix",
                &targets.raw_scene_view,
                shared,
                candidate,
                &candidate.draw_plan.chrome.suffix,
            );
            encode_aperture_composite(&mut encoder, targets, shared, candidate);
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &targets.intermediate_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback.buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(readback.layout.aligned_bytes_per_row()),
                        rows_per_image: Some(readback.layout.height()),
                    },
                },
                binding.target_key.extent,
            );
            #[cfg(test)]
            if test_fault == SceneRenderTestFault::ScopedValidationAfterHudWrite {
                // The immutable node buffer intentionally lacks COPY_SRC. This
                // deterministic command is captured by the validation scope
                // after the HUD belt has reserved and written its chunk.
                encoder.copy_buffer_to_buffer(
                    &candidate.node_buffer,
                    0,
                    &candidate.frame_buffer,
                    0,
                    wgpu::COPY_BUFFER_ALIGNMENT,
                );
            }
            self.staging_belt.finish();
            let submission = queue.submit([encoder.finish()]);
            #[cfg(test)]
            {
                self.submission_events = self.submission_events.saturating_add(1);
                self.scene_buffer_copy_events = self
                    .scene_buffer_copy_events
                    .saturating_add(u64::try_from(_scene_buffer_copies).unwrap_or(u64::MAX));
            }
            self.staging_belt.recall();
            Ok::<_, super::hud::HudGpuStagingError>(submission)
        })();

        let validation_error = pollster::block_on(validation.pop()).map(sanitize_gpu_error);
        let out_of_memory_error = pollster::block_on(out_of_memory.pop()).map(sanitize_gpu_error);
        let internal_error = pollster::block_on(internal.pop()).map(sanitize_gpu_error);
        if let Some(category) =
            select_scoped_gpu_error(validation_error, out_of_memory_error, internal_error)
        {
            // A belt whose transfer participated in a failed encoder/submission
            // is never reused. Dropping it cancels outstanding remaps; the next
            // call starts from one clean fixed-size chunk arena.
            self.reset_staging_belt(device);
            if physical_delta.is_some() {
                candidate.generation_state.reset_pending();
            }
            return Err(SceneRenderError::Gpu(category));
        }
        let submission = match submission_result {
            Ok(submission) => submission,
            Err(error) => {
                self.reset_staging_belt(device);
                if physical_delta.is_some() {
                    candidate.generation_state.reset_pending();
                }
                return Err(SceneRenderError::Hud(error));
            }
        };

        if let Some(transaction) = delta.take() {
            let expected_dirty = transaction.prepared.dirty_spans();
            let applied = transaction.cpu.commit_prepared(transaction.prepared);
            debug_assert_eq!(applied.dirty, expected_dirty);
            debug_assert_eq!(applied.generation_key, candidate.generation_key);
            debug_assert_eq!(applied.static_checksum, candidate.static_checksum);
            candidate.generation_state.commit_pending();
            candidate.source_revisions = applied.to;
            candidate.logical_viewport_points = applied.prospective_logical_viewport_points;
        }

        let (sender, receiver) = mpsc::sync_channel(1);
        readback
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        let map_guard = SceneReadbackMapGuard::new(&readback.buffer);
        #[cfg(test)]
        match test_fault {
            SceneRenderTestFault::PollTimeout => return Err(SceneRenderError::PollTimeout),
            SceneRenderTestFault::PollWrongSubmissionIndex => {
                return Err(SceneRenderError::PollWrongSubmissionIndex);
            }
            SceneRenderTestFault::MapCallbackCancelled => {
                return Err(SceneRenderError::MapFailed);
            }
            _ => {}
        }
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(SCENE_READBACK_TIMEOUT),
            })
            .map_err(scene_poll_error)?;
        receiver
            .recv_timeout(SCENE_MAP_CALLBACK_TIMEOUT)
            .map_err(|_| SceneRenderError::MapFailed)?
            .map_err(|_| SceneRenderError::MapFailed)?;
        #[cfg(test)]
        if test_fault == SceneRenderTestFault::MappedRangeFailure {
            return Err(SceneRenderError::MappedRangeFailed);
        }
        let mapped = readback
            .buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|_| SceneRenderError::MappedRangeFailed)?;
        #[cfg(test)]
        let normalize_source = if test_fault == SceneRenderTestFault::NormalizeShortBuffer {
            &mapped[..mapped.len().saturating_sub(1)]
        } else {
            &mapped
        };
        #[cfg(not(test))]
        let normalize_source = &mapped;
        let rgba = super::capture::normalize_scene_readback(normalize_source, readback.layout)
            .map_err(SceneRenderError::Readback)?;
        drop(mapped);
        drop(map_guard);

        Ok(SceneRenderOutcome {
            version: binding.request.version,
            physical_extent_pixels: binding.request.physical_extent_pixels,
            rgba,
        })
    }

    #[cfg(test)]
    const fn cache_and_submission_events_for_test(&self) -> (u64, u64, u64) {
        (
            self.targets.creation_events(),
            self.readback.creation_events(),
            self.submission_events,
        )
    }

    #[cfg(test)]
    const fn delta_events_for_test(&self) -> (u64, u64) {
        (self.submission_events, self.scene_buffer_copy_events)
    }

    fn validate_actual_identity(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shared: &SceneGpuShared,
        candidate: &GpuSceneCandidate,
    ) -> Result<(), SceneRenderError> {
        if self.device_identity != *device
            || shared.device_identity != *device
            || candidate.device_identity != *device
        {
            return Err(SceneRenderError::Delta(
                SceneDeltaRenderError::ActualDeviceMismatch,
            ));
        }
        if self.queue_identity != *queue || candidate.queue_identity != *queue {
            return Err(SceneRenderError::Delta(
                SceneDeltaRenderError::ActualQueueMismatch,
            ));
        }
        Ok(())
    }

    fn reset_staging_belt(&mut self, device: &wgpu::Device) {
        self.staging_belt =
            wgpu::util::StagingBelt::new(device.clone(), SCENE_STAGING_BELT_CHUNK_BYTES);
    }

    #[cfg(test)]
    fn inject_test_fault(&mut self, fault: SceneRenderTestFault) {
        assert_eq!(self.test_fault, SceneRenderTestFault::None);
        self.test_fault = fault;
    }
}

fn scene_poll_error(error: wgpu::PollError) -> SceneRenderError {
    match error {
        wgpu::PollError::Timeout => SceneRenderError::PollTimeout,
        wgpu::PollError::WrongSubmissionIndex(_, _) => SceneRenderError::PollWrongSubmissionIndex,
    }
}

struct SceneReadbackMapGuard<'buffer> {
    buffer: &'buffer wgpu::Buffer,
}

impl<'buffer> SceneReadbackMapGuard<'buffer> {
    const fn new(buffer: &'buffer wgpu::Buffer) -> Self {
        Self { buffer }
    }
}

impl Drop for SceneReadbackMapGuard<'_> {
    fn drop(&mut self) {
        // Also cancels a pending map on timeout/failure, returning the cached
        // buffer to a reusable unmapped state.
        self.buffer.unmap();
    }
}

fn encode_scene_delta_copies(
    encoder: &mut wgpu::CommandEncoder,
    staging_belt: &mut wgpu::util::StagingBelt,
    candidate: &GpuSceneCandidate,
    physical: ScenePhysicalDirtySpans,
) -> usize {
    let mut copies = 0;
    for (target, pending, spans) in [
        (
            &candidate.node_buffer,
            candidate.generation_state.nodes.pending.as_ref(),
            physical.nodes,
        ),
        (
            &candidate.content_globals_buffer,
            candidate.generation_state.content_globals.pending.as_ref(),
            physical.content_globals,
        ),
        (
            &candidate.frame_buffer,
            candidate.generation_state.frame.pending.as_ref(),
            physical.frame,
        ),
        (
            &candidate.scene_content_buffer,
            candidate.generation_state.scene_content.pending.as_ref(),
            physical.scene_content,
        ),
    ] {
        for span in spans.as_slice() {
            let offset = u64::try_from(span.offset).expect("validated scene span offset fits u64");
            let size = wgpu::BufferSize::new(
                u64::try_from(span.len).expect("validated scene span length fits u64"),
            )
            .expect("dirty scene spans are non-empty");
            let mut write = staging_belt.write_buffer(encoder, target, offset, size);
            write.copy_from_slice(&pending[span.offset..span.offset + span.len]);
            drop(write);
            copies += 1;
        }
    }
    debug_assert_eq!(copies, physical.copy_count());
    copies
}

fn encode_scene_world(
    encoder: &mut wgpu::CommandEncoder,
    targets: &SceneTargets,
    shared: &SceneGpuShared,
    candidate: &GpuSceneCandidate,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("glorp-scene-world"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &targets.raw_scene_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &targets.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        ..Default::default()
    });
    bind_scene_geometry(&mut pass, candidate);
    for draw in candidate
        .draw_plan
        .opaque
        .iter()
        .chain(candidate.draw_plan.world_blended_unsorted.iter())
    {
        encode_planned_draw(&mut pass, shared, draw);
    }
}

fn encode_scene_draws_without_depth<const N: usize>(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    target: &wgpu::TextureView,
    shared: &SceneGpuShared,
    candidate: &GpuSceneCandidate,
    draws: &[ScenePlannedDraw; N],
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    bind_scene_geometry(&mut pass, candidate);
    for draw in draws {
        encode_planned_draw(&mut pass, shared, draw);
    }
}

fn bind_scene_geometry<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    candidate: &'pass GpuSceneCandidate,
) {
    pass.set_vertex_buffer(0, candidate.vertex_buffer.slice(..));
    pass.set_index_buffer(candidate.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    pass.set_bind_group(0, &candidate.scene_bind_group, &[]);
    pass.set_bind_group(1, &candidate.atlas_bind_group, &[]);
}

fn encode_planned_draw<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    shared: &'pass SceneGpuShared,
    draw: &ScenePlannedDraw,
) {
    pass.set_pipeline(shared.pipelines.for_class(draw.pipeline));
    pass.draw_indexed(draw.index_range.clone(), 0, draw.instance_range.clone());
}

fn encode_aperture_composite(
    encoder: &mut wgpu::CommandEncoder,
    targets: &SceneTargets,
    shared: &SceneGpuShared,
    candidate: &GpuSceneCandidate,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("glorp-scene-aperture-composite"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &targets.intermediate_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(&shared.pipelines.aperture_composite);
    pass.set_bind_group(0, &candidate.scene_bind_group, &[]);
    pass.set_bind_group(2, &targets.aperture_bind_group, &[]);
    pass.draw(0..3, 0..1);
}

/// IEC 61966-2-1 sRGB electro-optical transfer for scene-owned color math.
pub(super) fn scene_srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// IEC 61966-2-1 sRGB opto-electrical transfer for scene-owned color math.
pub(super) fn scene_linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub(super) fn scene_unpremultiply_final(color: [f32; 4]) -> [f32; 4] {
    let alpha = color[3];
    if alpha == 0.0 {
        [0.0; 4]
    } else {
        [color[0] / alpha, color[1] / alpha, color[2] / alpha, alpha]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::companion_scene::scene::{
        AnalyticGeometry, AnalyticParamId, AnalyticSemantic, AuthoredGlyph, Bounds3,
        CanonicalAlias, ContentDelta, DepthBehavior, FrameDelta, InstanceGroupBinding,
        InstanceLayer, MaterialId, MaterialKind, MaterialTemplate, PetArtFilter, PetPaletteRole,
        PrimitiveBinding, PrimitiveKind, PrimitiveSpace, PrimitiveTemplate, ResourceId,
        ResourceKind, ResourceTemplate, SceneFixture, WorldBlend,
    };

    /// CPU-side reference vectors for the family-aware WGSL glyph placement
    /// contract. The live renderer performs these operations in
    /// `vs_world_glyph`; these helpers lock exact points without expanding the
    /// production surface.
    struct SceneGlyphPlacementContract;

    impl SceneGlyphPlacementContract {
        fn pet_cell_base(slot: u32, cell_extent: [f32; 2]) -> Option<[f32; 2]> {
            if slot >= 130 {
                return None;
            }
            let column = slot % 13;
            let row = slot / 13;
            Some([
                column as f32 * cell_extent[0],
                (9 - row) as f32 * cell_extent[1],
            ])
        }

        fn metric_ink_offset(
            quad_corner: [f32; 2],
            entry: GlyphAtlasGpuEntry,
            cell_extent: [f32; 2],
        ) -> Option<[f32; 2]> {
            let scale = Self::one_cell_scale(entry, cell_extent)?;
            Some([
                (entry.ink_origin_size[0] + quad_corner[0] * entry.ink_origin_size[2]) * scale,
                (entry.ink_origin_size[1] + quad_corner[1] * entry.ink_origin_size[3]) * scale,
            ])
        }

        fn one_cell_scale(entry: GlyphAtlasGpuEntry, cell_extent: [f32; 2]) -> Option<f32> {
            if entry.metrics[0] <= 0.0
                || entry.metrics[1] <= 0.0
                || entry.ink_origin_size[2] <= 0.0
                || entry.ink_origin_size[3] <= 0.0
                || cell_extent[0] <= 0.0
                || cell_extent[1] <= 0.0
            {
                return None;
            }
            Some((cell_extent[0] / entry.metrics[0]).min(cell_extent[1] / entry.metrics[1]))
        }

        fn prop_cell_base(
            origin: [f32; 2],
            motion: [f32; 2],
            local_cell: [i32; 2],
            cell_extent: [f32; 2],
        ) -> [f32; 2] {
            [
                origin[0] + motion[0] + local_cell[0] as f32 * cell_extent[0],
                origin[1] + motion[1] - local_cell[1] as f32 * cell_extent[1],
            ]
        }

        fn tank_cell_base(center: [f32; 2], cell_extent: [f32; 2]) -> [f32; 2] {
            [
                center[0] - 0.5 * cell_extent[0],
                center[1] - 0.5 * cell_extent[1],
            ]
        }

        const fn direct_cell_base(position: [f32; 2]) -> [f32; 2] {
            position
        }

        fn frame_opacity(flags: u32, opacity: f32) -> Option<f32> {
            ((flags & 1) != 0).then_some(opacity)
        }

        const fn tank_cell_visible(flags: u32, layer: u32, instance_group: u32) -> bool {
            matches!(instance_group, 5 | 6) && flags & 3 == 3 && layer == instance_group - 4
        }

        fn wall_xy(local: [f32; 2], aux_affine: [f32; 6], offset: [f32; 2]) -> [f32; 2] {
            [
                aux_affine[0] * local[0] + aux_affine[1] * local[1] + aux_affine[4] + offset[0],
                aux_affine[2] * local[0] + aux_affine[3] * local[1] + aux_affine[5] + offset[1],
            ]
        }
    }

    fn surface_capabilities() -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            format_capabilities: vec![wgpu::SurfaceFormatCapabilities {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                color_spaces: wgpu::SurfaceColorSpaces::SRGB,
            }],
            present_modes: vec![wgpu::PresentMode::Fifo],
            alpha_modes: vec![wgpu::CompositeAlphaMode::PostMultiplied],
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }

    #[test]
    fn scene_surface_contract_selects_only_srgb_postmultiplied_render_attachment() {
        let selected = SceneSurfaceContract::select(&surface_capabilities()).unwrap();
        assert_eq!(selected.format, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert_eq!(selected.color_space, wgpu::SurfaceColorSpace::Srgb);
        assert_eq!(
            selected.alpha_mode,
            wgpu::CompositeAlphaMode::PostMultiplied
        );

        let mut missing_format = surface_capabilities();
        missing_format.format_capabilities.clear();
        assert_eq!(
            SceneSurfaceContract::select(&missing_format),
            Err(SceneSurfaceContractError::MissingBgra8UnormSrgb)
        );

        let mut missing_srgb = surface_capabilities();
        missing_srgb.format_capabilities[0].color_spaces = wgpu::SurfaceColorSpaces::DISPLAY_P3;
        assert_eq!(
            SceneSurfaceContract::select(&missing_srgb),
            Err(SceneSurfaceContractError::MissingSrgbColorSpace)
        );

        let mut missing_alpha = surface_capabilities();
        missing_alpha.alpha_modes = vec![wgpu::CompositeAlphaMode::Opaque];
        assert_eq!(
            SceneSurfaceContract::select(&missing_alpha),
            Err(SceneSurfaceContractError::MissingPostMultipliedAlpha)
        );

        let mut missing_usage = surface_capabilities();
        missing_usage.usages = wgpu::TextureUsages::TEXTURE_BINDING;
        assert_eq!(
            SceneSurfaceContract::select(&missing_usage),
            Err(SceneSurfaceContractError::MissingRenderAttachmentUsage)
        );
    }

    fn required_features(format: wgpu::TextureFormat) -> wgpu::TextureFormatFeatures {
        match format {
            wgpu::TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormatFeatures {
                allowed_usages: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                flags: wgpu::TextureFormatFeatureFlags::FILTERABLE
                    | wgpu::TextureFormatFeatureFlags::BLENDABLE,
            },
            wgpu::TextureFormat::R8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
                wgpu::TextureFormatFeatures {
                    allowed_usages: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST,
                    flags: wgpu::TextureFormatFeatureFlags::FILTERABLE,
                }
            }
            wgpu::TextureFormat::Depth24Plus => wgpu::TextureFormatFeatures {
                allowed_usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
                flags: wgpu::TextureFormatFeatureFlags::empty(),
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn scene_texture_contract_freezes_formats_and_validates_required_features() {
        assert_eq!(
            SceneTextureContract::INTERMEDIATE,
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
        assert_eq!(SceneTextureContract::COVERAGE, wgpu::TextureFormat::R8Unorm);
        assert_eq!(
            SceneTextureContract::COLOR,
            wgpu::TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            SceneTextureContract::DEPTH,
            wgpu::TextureFormat::Depth24Plus
        );
        assert_eq!(SceneTextureContract::SAMPLE_COUNT, 1);
        SceneTextureContract::validate_with(required_features).unwrap();
        SceneTextureContract::validate_with(|format| {
            format.guaranteed_format_features(wgpu::Features::empty())
        })
        .unwrap();

        let missing_usage = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::COLOR {
                features
                    .allowed_usages
                    .remove(wgpu::TextureUsages::COPY_DST);
            }
            features
        });
        assert!(matches!(
            missing_usage,
            Err(SceneTextureContractError::Usage {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                ..
            })
        ));

        let missing_intermediate_copy = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::INTERMEDIATE {
                features
                    .allowed_usages
                    .remove(wgpu::TextureUsages::COPY_SRC);
            }
            features
        });
        assert!(matches!(
            missing_intermediate_copy,
            Err(SceneTextureContractError::Usage {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                ..
            })
        ));

        let missing_filterable = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::COVERAGE {
                features
                    .flags
                    .remove(wgpu::TextureFormatFeatureFlags::FILTERABLE);
            }
            features
        });
        assert_eq!(
            missing_filterable,
            Err(SceneTextureContractError::NotFilterable { format: wgpu::TextureFormat::R8Unorm })
        );

        let missing_intermediate_filterable = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::INTERMEDIATE {
                features
                    .flags
                    .remove(wgpu::TextureFormatFeatureFlags::FILTERABLE);
            }
            features
        });
        assert_eq!(
            missing_intermediate_filterable,
            Err(SceneTextureContractError::NotFilterable {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
            })
        );

        let missing_blendable = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::INTERMEDIATE {
                features
                    .flags
                    .remove(wgpu::TextureFormatFeatureFlags::BLENDABLE);
            }
            features
        });
        assert_eq!(
            missing_blendable,
            Err(SceneTextureContractError::NotBlendable {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
            })
        );
    }

    #[test]
    fn packed_mirror_layouts_have_frozen_offsets_sizes_and_checked_translation() {
        assert_eq!(PackedMirrorLayout::NODE_BYTES, 12_288);
        assert_eq!(PackedMirrorLayout::CONTENT_GLOBALS_BYTES, 160);
        assert_eq!(PackedMirrorLayout::SCENE_CONTENT_BYTES, 15_552);
        assert_eq!(PackedMirrorLayout::FRAME_BYTES, 7_680);
        assert_eq!(PackedMirrorLayout::content_globals_offset(), 0);
        assert_eq!(
            [
                ContentMirrorFamily::Pet,
                ContentMirrorFamily::PropGlyphs,
                ContentMirrorFamily::TankGlyphs,
                ContentMirrorFamily::Ambient,
                ContentMirrorFamily::PetParticles,
                ContentMirrorFamily::RoomGlyphs,
                ContentMirrorFamily::Analytics,
            ]
            .map(|family| PackedMirrorLayout::scene_content_offset(family).unwrap()),
            [0, 4_160, 7_040, 7_552, 9_600, 13_760, 14_784]
        );
        assert_eq!(
            PackedMirrorLayout::scene_content_offset(ContentMirrorFamily::Globals),
            None
        );
        assert_eq!(
            FrameMirrorFamily::ALL.map(PackedMirrorLayout::frame_offset),
            [0, 192, 672, 1_440, 4_512, 4_608, 6_144]
        );
        assert_eq!(
            PackedMirrorLayout::translate_content_globals_span(ByteSpan { offset: 0, len: 160 }),
            Ok(ByteSpan { offset: 0, len: 160 })
        );
        for family in [
            ContentMirrorFamily::Pet,
            ContentMirrorFamily::PropGlyphs,
            ContentMirrorFamily::TankGlyphs,
            ContentMirrorFamily::Ambient,
            ContentMirrorFamily::PetParticles,
            ContentMirrorFamily::RoomGlyphs,
            ContentMirrorFamily::Analytics,
        ] {
            let translated = PackedMirrorLayout::translate_scene_content_span(
                family,
                super::super::buffers::ByteSpan { offset: 0, len: family.record_size() },
            )
            .unwrap();
            assert_eq!(
                translated.offset,
                PackedMirrorLayout::scene_content_offset(family).unwrap()
            );
            assert_eq!(translated.len, family.record_size());
            assert_eq!(translated.offset % 16, 0);
        }
        assert_eq!(
            PackedMirrorLayout::translate_scene_content_span(
                ContentMirrorFamily::Pet,
                super::super::buffers::ByteSpan { offset: 1, len: 32 },
            ),
            Err(PackedMirrorLayoutError::MisalignedSpan)
        );
        assert_eq!(
            PackedMirrorLayout::translate_scene_content_span(
                ContentMirrorFamily::Globals,
                ByteSpan { offset: 0, len: 160 },
            ),
            Err(PackedMirrorLayoutError::NonSceneContentFamily)
        );
        assert_eq!(
            PackedMirrorLayout::translate_frame_span(
                FrameMirrorFamily::Lights,
                super::super::buffers::ByteSpan { offset: 96, len: 48 },
            ),
            Err(PackedMirrorLayoutError::SpanOutOfBounds)
        );
        assert_eq!(
            PackedMirrorLayout::translate_node_span(super::super::buffers::ByteSpan {
                offset: 96,
                len: 96,
            }),
            Ok(super::super::buffers::ByteSpan { offset: 96, len: 96 })
        );
    }

    #[test]
    fn mutable_buffer_and_family_layouts_freeze_offsets_lengths_alignment_and_staging_bound() {
        assert_eq!(
            SceneMutableBuffer::ALL.map(PackedMirrorLayout::mutable_buffer_layout),
            [
                SceneMutableBufferLayout {
                    staging_offset: 0,
                    len: 12_288,
                    copy_alignment: 4,
                },
                SceneMutableBufferLayout {
                    staging_offset: 12_288,
                    len: 160,
                    copy_alignment: 4,
                },
                SceneMutableBufferLayout {
                    staging_offset: 12_448,
                    len: 7_680,
                    copy_alignment: 4,
                },
                SceneMutableBufferLayout {
                    staging_offset: 20_128,
                    len: 15_552,
                    copy_alignment: 4,
                },
            ],
        );
        assert_eq!(PackedMirrorLayout::MUTABLE_BUFFER_BYTES, 35_680);
        assert_eq!(PackedMirrorLayout::FULL_FRAME_STAGING_BYTES, 36_512);
        assert!(
            PackedMirrorLayout::FULL_FRAME_STAGING_BYTES
                <= usize::try_from(SCENE_STAGING_BELT_CHUNK_BYTES).unwrap()
        );

        assert_eq!(
            ContentMirrorFamily::ALL.map(PackedMirrorLayout::content_family_layout),
            [
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::ContentGlobals,
                    offset: 0,
                    len: 160,
                    span_alignment: 160,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::SceneContent,
                    offset: 0,
                    len: 4_160,
                    span_alignment: 32,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::SceneContent,
                    offset: 4_160,
                    len: 2_880,
                    span_alignment: 32,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::SceneContent,
                    offset: 7_040,
                    len: 512,
                    span_alignment: 32,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::SceneContent,
                    offset: 7_552,
                    len: 2_048,
                    span_alignment: 32,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::SceneContent,
                    offset: 9_600,
                    len: 4_160,
                    span_alignment: 32,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::SceneContent,
                    offset: 13_760,
                    len: 1_024,
                    span_alignment: 32,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::SceneContent,
                    offset: 14_784,
                    len: 768,
                    span_alignment: 48,
                },
            ],
        );
        assert_eq!(
            FrameMirrorFamily::ALL.map(PackedMirrorLayout::frame_family_layout),
            [
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::Frame,
                    offset: 0,
                    len: 192,
                    span_alignment: 192,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::Frame,
                    offset: 192,
                    len: 480,
                    span_alignment: 48,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::Frame,
                    offset: 672,
                    len: 768,
                    span_alignment: 48,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::Frame,
                    offset: 1_440,
                    len: 3_072,
                    span_alignment: 48,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::Frame,
                    offset: 4_512,
                    len: 96,
                    span_alignment: 48,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::Frame,
                    offset: 4_608,
                    len: 1_536,
                    span_alignment: 48,
                },
                PackedMirrorFamilyLayout {
                    buffer: SceneMutableBuffer::Frame,
                    offset: 6_144,
                    len: 1_536,
                    span_alignment: 96,
                },
            ],
        );
    }

    #[test]
    fn physical_dirty_spans_map_logical_families_into_only_four_buffers() {
        let mut dirty = ScenePhysicalDirtySpans::default();
        dirty.insert_node(ByteSpan { offset: 96, len: 96 }).unwrap();
        dirty
            .insert_content(
                ContentMirrorFamily::Globals,
                ByteSpan { offset: 0, len: 160 },
            )
            .unwrap();
        dirty
            .insert_content(
                ContentMirrorFamily::PropGlyphs,
                ByteSpan { offset: 32, len: 64 },
            )
            .unwrap();
        dirty
            .insert_frame(FrameMirrorFamily::Lights, ByteSpan { offset: 0, len: 48 })
            .unwrap();

        assert_eq!(dirty.nodes.as_slice(), &[ByteSpan { offset: 96, len: 96 }]);
        assert_eq!(
            dirty.content_globals.as_slice(),
            &[ByteSpan { offset: 0, len: 160 }]
        );
        assert_eq!(
            dirty.frame.as_slice(),
            &[ByteSpan { offset: 4_512, len: 48 }]
        );
        assert_eq!(
            dirty.scene_content.as_slice(),
            &[ByteSpan { offset: 4_192, len: 64 }]
        );
    }

    #[test]
    fn delta_execution_orders_copies_submission_commit_and_mapping_once() {
        let source = include_str!("render.rs");
        let inner = source
            .split("fn render_offscreen_inner(")
            .nth(1)
            .expect("render_offscreen_inner exists")
            .split("\n    #[cfg(test)]\n    const fn cache_and_submission_events_for_test")
            .next()
            .expect("render_offscreen_inner has a test-helper boundary");

        for one_shot in [
            "encode_scene_delta_copies(",
            "self.staging_belt.finish();",
            "queue.submit([encoder.finish()])",
            "self.staging_belt.recall();",
            "transaction.cpu.commit_prepared(transaction.prepared)",
            ".map_async(wgpu::MapMode::Read",
        ] {
            assert_eq!(
                inner.matches(one_shot).count(),
                1,
                "expected one `{one_shot}` in the delta render transaction"
            );
        }

        let copies = inner.find("encode_scene_delta_copies(").unwrap();
        let first_draw = inner.find("encode_scene_world(").unwrap();
        let finish = inner.find("self.staging_belt.finish();").unwrap();
        let submit = inner.find("queue.submit([encoder.finish()])").unwrap();
        let recall = inner.find("self.staging_belt.recall();").unwrap();
        let commit = inner
            .find("transaction.cpu.commit_prepared(transaction.prepared)")
            .unwrap();
        let map = inner.find(".map_async(wgpu::MapMode::Read").unwrap();

        assert!(
            copies < first_draw,
            "scene copies must precede the first draw"
        );
        assert!(
            first_draw < finish && finish < submit && submit < recall,
            "draws, belt finish, submission, and recall must stay ordered"
        );
        assert!(
            recall < commit && commit < map,
            "canonical commit must follow clean submission and precede mapping"
        );
    }

    #[test]
    fn scene_color_math_uses_exact_iec_thresholds_and_zero_safe_unpremultiply() {
        assert!((scene_srgb_to_linear(0.04045) - 0.003_130_805).abs() < 1.0e-8);
        assert!((scene_linear_to_srgb(0.003_130_8) - 0.040_449_936).abs() < 1.0e-7);
        assert_eq!(scene_unpremultiply_final([0.0, 0.0, 0.0, 0.0]), [0.0; 4]);
        assert_eq!(
            scene_unpremultiply_final([0.20, 0.10, 0.05, 0.25]),
            [0.80, 0.40, 0.20, 0.25]
        );
    }

    #[allow(clippy::too_many_arguments)] // Compact axis table fixture for selector tests.
    fn pipeline_primitive(
        primitive_kind: u32,
        material_kind: u32,
        resource_kind: u32,
        blend: u32,
        depth: u32,
        space: u32,
        instance_group: u32,
        binding_index: u32,
    ) -> PrimitiveGpuValue {
        PrimitiveGpuValue {
            node_index: 0,
            material_index: 0,
            aux_node_index: NONE_U32,
            primitive_kind,
            material_kind,
            resource_kind,
            blend,
            depth,
            space,
            instance_group,
            instance_base: match instance_group {
                0 => NONE_U32,
                3 => binding_index * 9,
                5 | 6 => binding_index * 8,
                _ => 0,
            },
            binding_index,
            authored_order: 0,
            content_base: 0,
            frame_base: binding_index,
            aux_content_base: NONE_U32,
        }
    }

    fn pipeline_draw(source: PrimitiveSource) -> SceneDrawRecord {
        let instance_count = match source {
            PrimitiveSource::None => 0,
            PrimitiveSource::StaticAtlas | PrimitiveSource::Analytic => 1,
            PrimitiveSource::Instances(InstanceSource::PetBody)
            | PrimitiveSource::Instances(InstanceSource::PetParticles)
            | PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask) => 130,
            PrimitiveSource::Instances(InstanceSource::RoomGlyphs) => 32,
            PrimitiveSource::Instances(InstanceSource::PropGlyphs { .. }) => 9,
            PrimitiveSource::Instances(InstanceSource::TankCells { .. }) => 8,
            PrimitiveSource::Instances(InstanceSource::Ambient) => 64,
            PrimitiveSource::Instances(InstanceSource::Hud) => 0,
        };
        SceneDrawRecord {
            index_range: 0..6,
            instance_range: 0..instance_count,
            source,
            authored_order: 0,
        }
    }

    fn sealed_hud_draw() -> SceneDrawRecord {
        pipeline_draw(PrimitiveSource::Instances(InstanceSource::Hud))
    }

    #[test]
    fn typed_pipeline_selector_is_exhaustive_for_the_production_v2_matrix() {
        use InstanceSource::*;
        use ScenePipelineClass::*;

        let cases = [
            (
                pipeline_primitive(2, 2, 3, 1, 1, 1, 0, 0),
                pipeline_draw(PrimitiveSource::Analytic),
                WorldOpaqueAnalytic,
            ),
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 4, 0),
                pipeline_draw(PrimitiveSource::Instances(RoomGlyphs)),
                WorldSourceOverGlyph,
            ),
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 1, 0),
                pipeline_draw(PrimitiveSource::Instances(PetBody)),
                WorldSourceOverGlyph,
            ),
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 3, 2),
                pipeline_draw(PrimitiveSource::Instances(PropGlyphs { slot: 2 })),
                WorldSourceOverGlyph,
            ),
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 5, 1),
                pipeline_draw(PrimitiveSource::Instances(TankCells {
                    slot: 1,
                    layer: InstanceLayer::Behind,
                })),
                WorldSourceOverGlyph,
            ),
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 6, 1),
                pipeline_draw(PrimitiveSource::Instances(TankCells {
                    slot: 1,
                    layer: InstanceLayer::Foreground,
                })),
                WorldSourceOverGlyph,
            ),
            (
                pipeline_primitive(2, 4, 3, 4, 2, 1, 0, 2),
                pipeline_draw(PrimitiveSource::Analytic),
                WorldMultiplyAnalytic,
            ),
            (
                pipeline_primitive(2, 4, 3, 4, 2, 1, 0, 1),
                pipeline_draw(PrimitiveSource::Instances(WallShadowGlyphMask)),
                WorldMultiplyGlyphMask,
            ),
            (
                pipeline_primitive(2, 2, 3, 3, 2, 1, 0, 4),
                pipeline_draw(PrimitiveSource::Analytic),
                WorldSourceOverAnalytic,
            ),
            (
                pipeline_primitive(4, 5, 1, 5, 2, 1, 7, 0),
                pipeline_draw(PrimitiveSource::Instances(Ambient)),
                WorldAdditiveGlyph,
            ),
            (
                pipeline_primitive(4, 5, 1, 5, 2, 1, 2, 0),
                pipeline_draw(PrimitiveSource::Instances(PetParticles)),
                WorldAdditiveGlyph,
            ),
            (
                pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 3),
                pipeline_draw(PrimitiveSource::Analytic),
                ChromeAnalytic,
            ),
            (
                pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 5),
                pipeline_draw(PrimitiveSource::Analytic),
                ChromeAnalytic,
            ),
            (
                pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 6),
                pipeline_draw(PrimitiveSource::Analytic),
                ChromeAnalytic,
            ),
            (
                pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 7),
                pipeline_draw(PrimitiveSource::Analytic),
                ChromeAnalytic,
            ),
            (
                pipeline_primitive(4, 6, 1, 3, 3, 2, 8, 0),
                sealed_hud_draw(),
                SealedHudHook,
            ),
        ];
        for (primitive, draw, expected) in &cases {
            assert_eq!(scene_pipeline_class(*primitive, draw), Some(*expected));
        }
        assert!(!cases
            .iter()
            .any(|(_, _, expected)| { *expected == WorldAdditiveAnalyticReserved }));
    }

    fn canonical_draw_plan_fixture() -> (
        Vec<PrimitiveGpuValue>,
        Vec<SceneDrawRecord>,
        ScenePhaseTable,
    ) {
        let mut primitives = vec![
            pipeline_primitive(2, 2, 3, 1, 1, 1, 0, 0),
            pipeline_primitive(2, 4, 3, 4, 2, 1, 0, 2),
            pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 5),
            pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 3),
            pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 6),
            pipeline_primitive(4, 6, 1, 3, 3, 2, 8, 0),
            pipeline_primitive(2, 6, 3, 3, 3, 2, 0, 7),
        ];
        let mut draws = vec![
            pipeline_draw(PrimitiveSource::Analytic),
            pipeline_draw(PrimitiveSource::Analytic),
            pipeline_draw(PrimitiveSource::Analytic),
            pipeline_draw(PrimitiveSource::Analytic),
            pipeline_draw(PrimitiveSource::Analytic),
            sealed_hud_draw(),
            pipeline_draw(PrimitiveSource::Analytic),
        ];
        for (authored_order, (primitive, draw)) in primitives.iter_mut().zip(&mut draws).enumerate()
        {
            primitive.authored_order = authored_order as u32;
            draw.authored_order = authored_order as u32;
        }
        (
            primitives,
            draws,
            ScenePhaseTable {
                opaque_cutout: vec![0],
                world_blended_unsorted: vec![1],
                chrome_authored: vec![2, 3, 4, 5, 6],
            },
        )
    }

    #[test]
    fn draw_plan_preserves_world_phases_and_seals_the_canonical_chrome_schedule() {
        let (primitives, draws, phases) = canonical_draw_plan_fixture();
        let plan = validate_scene_draw_plan(&primitives, &draws, &phases).unwrap();

        assert_eq!(
            plan.opaque,
            vec![ScenePlannedDraw {
                primitive_index: 0,
                pipeline: ScenePipelineClass::WorldOpaqueAnalytic,
                index_range: 0..6,
                instance_range: 0..1,
                authored_order: 0,
            }]
        );
        assert_eq!(
            plan.world_blended_unsorted,
            vec![ScenePlannedDraw {
                primitive_index: 1,
                pipeline: ScenePipelineClass::WorldMultiplyAnalytic,
                index_range: 0..6,
                instance_range: 0..1,
                authored_order: 1,
            }]
        );
        assert_eq!(
            plan.chrome.prefix.map(|draw| draw.primitive_index),
            [2, 3, 4],
            "gauges, status, and trouble stay before the sealed HUD hook",
        );
        assert_eq!(plan.chrome.hud.primitive_index, 5);
        assert_eq!(
            plan.chrome.suffix.map(|draw| draw.primitive_index),
            [6],
            "dim is the only post-HUD chrome draw",
        );
    }

    #[test]
    fn draw_plan_fails_closed_on_missing_duplicate_misplaced_or_untyped_draws() {
        let assert_invalid = |primitives: &[PrimitiveGpuValue],
                              draws: &[SceneDrawRecord],
                              phases: &ScenePhaseTable,
                              expected| {
            assert_eq!(
                validate_scene_draw_plan(primitives, draws, phases),
                Err(expected),
            );
        };

        let (primitives, draws, mut phases) = canonical_draw_plan_fixture();
        phases.chrome_authored.remove(3);
        assert_invalid(
            &primitives,
            &draws,
            &phases,
            SceneDrawPlanError::InvalidChromeSchedule,
        );

        let (primitives, draws, mut phases) = canonical_draw_plan_fixture();
        phases.chrome_authored.insert(3, 5);
        assert_invalid(
            &primitives,
            &draws,
            &phases,
            SceneDrawPlanError::InvalidChromeSchedule,
        );

        let (primitives, draws, mut phases) = canonical_draw_plan_fixture();
        phases.chrome_authored.swap(4, 3);
        assert_invalid(
            &primitives,
            &draws,
            &phases,
            SceneDrawPlanError::InvalidChromeSchedule,
        );

        let (mut primitives, mut draws, phases) = canonical_draw_plan_fixture();
        primitives[1] = pipeline_primitive(1, 1, 1, 3, 2, 1, 0, 0);
        primitives[1].authored_order = 1;
        draws[1] = pipeline_draw(PrimitiveSource::StaticAtlas);
        draws[1].authored_order = 1;
        assert_invalid(
            &primitives,
            &draws,
            &phases,
            SceneDrawPlanError::InvalidPipelineClass,
        );

        let (mut primitives, draws, phases) = canonical_draw_plan_fixture();
        primitives[1] = pipeline_primitive(2, 5, 3, 5, 2, 1, 0, 4);
        primitives[1].authored_order = 1;
        assert_invalid(
            &primitives,
            &draws,
            &phases,
            SceneDrawPlanError::InvalidPipelineClass,
        );
    }

    #[test]
    fn draw_plan_walks_every_primitive_once_and_bounds_checks_phase_indices() {
        let (primitives, draws, mut phases) = canonical_draw_plan_fixture();
        phases.world_blended_unsorted.push(0);
        assert_eq!(
            validate_scene_draw_plan(&primitives, &draws, &phases),
            Err(SceneDrawPlanError::DuplicatePrimitive),
        );

        let (primitives, draws, mut phases) = canonical_draw_plan_fixture();
        phases.opaque_cutout[0] = u32::MAX;
        assert_eq!(
            validate_scene_draw_plan(&primitives, &draws, &phases),
            Err(SceneDrawPlanError::PrimitiveIndexOutOfBounds),
        );

        let (mut primitives, mut draws, phases) = canonical_draw_plan_fixture();
        primitives.push(pipeline_primitive(2, 4, 3, 4, 2, 1, 0, 2));
        draws.push(pipeline_draw(PrimitiveSource::Analytic));
        assert_eq!(
            validate_scene_draw_plan(&primitives, &draws, &phases),
            Err(SceneDrawPlanError::MissingPrimitive),
        );

        let (primitives, mut draws, phases) = canonical_draw_plan_fixture();
        draws[0].authored_order = 99;
        assert_eq!(
            validate_scene_draw_plan(&primitives, &draws, &phases),
            Err(SceneDrawPlanError::AuthoredOrderMismatch),
        );
    }

    #[test]
    fn pipeline_selector_fails_closed_on_axis_and_source_mutations() {
        let primitive = pipeline_primitive(2, 4, 3, 4, 2, 1, 0, 2);
        let draw = pipeline_draw(PrimitiveSource::Analytic);
        assert_eq!(
            scene_pipeline_class(primitive, &draw),
            Some(ScenePipelineClass::WorldMultiplyAnalytic)
        );
        for mutate in [
            |value: &mut PrimitiveGpuValue| value.primitive_kind = 4,
            |value: &mut PrimitiveGpuValue| value.material_kind = 2,
            |value: &mut PrimitiveGpuValue| value.resource_kind = 1,
            |value: &mut PrimitiveGpuValue| value.blend = 3,
            |value: &mut PrimitiveGpuValue| value.depth = 1,
            |value: &mut PrimitiveGpuValue| value.space = 2,
            |value: &mut PrimitiveGpuValue| value.instance_group = 1,
            |value: &mut PrimitiveGpuValue| value.binding_index = 1,
        ] {
            let mut changed = primitive;
            mutate(&mut changed);
            assert_eq!(scene_pipeline_class(changed, &draw), None);
        }
        assert_eq!(
            scene_pipeline_class(
                primitive,
                &pipeline_draw(PrimitiveSource::Instances(
                    InstanceSource::WallShadowGlyphMask,
                )),
            ),
            None,
        );
        let wall = pipeline_primitive(2, 4, 3, 4, 2, 1, 0, 1);
        assert_eq!(
            scene_pipeline_class(wall, &pipeline_draw(PrimitiveSource::Analytic)),
            None
        );

        let reserved = pipeline_primitive(2, 5, 3, 5, 2, 1, 0, 4);
        assert_eq!(
            scene_pipeline_class(reserved, &pipeline_draw(PrimitiveSource::Analytic)),
            None,
            "v2 cannot select the reserved additive-analytic handle",
        );

        for (primitive, source) in [
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 3, 2),
                PrimitiveSource::Instances(InstanceSource::PetBody),
            ),
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 3, 2),
                PrimitiveSource::Instances(InstanceSource::PropGlyphs { slot: 1 }),
            ),
            (
                pipeline_primitive(4, 1, 1, 3, 2, 1, 5, 1),
                PrimitiveSource::Instances(InstanceSource::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Foreground,
                }),
            ),
            (
                pipeline_primitive(4, 5, 1, 5, 2, 1, 7, 0),
                PrimitiveSource::Instances(InstanceSource::PetParticles),
            ),
        ] {
            assert_eq!(
                scene_pipeline_class(primitive, &pipeline_draw(source)),
                None
            );
        }
        let mut nonsealed_hud = pipeline_draw(PrimitiveSource::Instances(InstanceSource::Hud));
        nonsealed_hud.instance_range = 0..1;
        assert_eq!(
            scene_pipeline_class(pipeline_primitive(4, 6, 1, 3, 3, 2, 8, 0), &nonsealed_hud,),
            None,
            "HUD must remain the sealed zero-instance hook",
        );
        assert_eq!(
            scene_pipeline_class(
                pipeline_primitive(1, 1, 1, 3, 2, 1, 0, 0),
                &pipeline_draw(PrimitiveSource::StaticAtlas),
            ),
            None,
            "v2 has no typed static-atlas recipe",
        );

        let mut identity_only = primitive;
        identity_only.node_index = 47;
        identity_only.material_index = 29;
        identity_only.authored_order = 101;
        let mut identity_draw = draw;
        identity_draw.authored_order = 101;
        identity_draw.index_range = 300..306;
        assert_eq!(
            scene_pipeline_class(identity_only, &identity_draw),
            Some(ScenePipelineClass::WorldMultiplyAnalytic),
            "pipeline selection is semantic and does not depend on aliases or dense ids",
        );
    }

    #[test]
    fn premultiplied_blend_contracts_match_closed_scene_equations() {
        let component = |src_factor, dst_factor| wgpu::BlendComponent {
            src_factor,
            dst_factor,
            operation: wgpu::BlendOperation::Add,
        };
        let alpha = component(wgpu::BlendFactor::One, wgpu::BlendFactor::OneMinusSrcAlpha);
        assert_eq!(
            scene_blend_state(SceneBlendContract::SourceOver),
            wgpu::BlendState { color: alpha, alpha },
        );
        assert_eq!(
            scene_blend_state(SceneBlendContract::Multiply),
            wgpu::BlendState {
                color: component(wgpu::BlendFactor::Dst, wgpu::BlendFactor::OneMinusSrcAlpha,),
                alpha,
            },
        );
        assert_eq!(
            scene_blend_state(SceneBlendContract::Additive),
            wgpu::BlendState {
                color: component(wgpu::BlendFactor::One, wgpu::BlendFactor::One),
                alpha,
            },
        );
    }

    #[test]
    fn pipeline_contracts_lock_entrypoints_blend_and_depth_behavior() {
        use ScenePipelineClass::*;
        let cases = [
            (
                WorldOpaqueAnalytic,
                "vs_world_analytic",
                "fs_analytic",
                None,
                Some(true),
            ),
            (
                WorldSourceOverAnalytic,
                "vs_world_analytic",
                "fs_analytic",
                Some(SceneBlendContract::SourceOver),
                Some(false),
            ),
            (
                WorldSourceOverGlyph,
                "vs_world_glyph",
                "fs_glyph",
                Some(SceneBlendContract::SourceOver),
                Some(false),
            ),
            (
                WorldMultiplyAnalytic,
                "vs_world_analytic",
                "fs_analytic",
                Some(SceneBlendContract::Multiply),
                Some(false),
            ),
            (
                WorldMultiplyGlyphMask,
                "vs_world_glyph",
                "fs_wall_shadow_glyph",
                Some(SceneBlendContract::Multiply),
                Some(false),
            ),
            (
                WorldAdditiveGlyph,
                "vs_world_glyph",
                "fs_glyph",
                Some(SceneBlendContract::Additive),
                Some(false),
            ),
            (
                WorldAdditiveAnalyticReserved,
                "vs_world_analytic",
                "fs_analytic",
                Some(SceneBlendContract::Additive),
                Some(false),
            ),
            (
                ChromeAnalytic,
                "vs_screen_analytic",
                "fs_analytic",
                Some(SceneBlendContract::SourceOver),
                None,
            ),
            (
                SealedHudHook,
                "vs_hud",
                "fs_hud",
                Some(SceneBlendContract::SourceOver),
                None,
            ),
        ];
        for (class, vertex_entry, fragment_entry, blend, depth_write_enabled) in cases {
            assert_eq!(
                scene_pipeline_contract(class),
                ScenePipelineContract {
                    vertex_entry,
                    fragment_entry,
                    blend,
                    depth_write_enabled,
                },
            );
        }
        assert_eq!(
            APERTURE_COMPOSITE_PIPELINE_CONTRACT,
            ApertureCompositePipelineContract {
                pipeline: ScenePipelineContract {
                    vertex_entry: "vs_final",
                    fragment_entry: "fs_aperture_composite",
                    blend: None,
                    depth_write_enabled: None,
                },
                scene_storage_group: 0,
                sampled_raw_group: 2,
                target_format: SceneTextureContract::INTERMEDIATE,
            },
        );
        let production = include_str!("render.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        assert!(production
            .contains("bind_group_layouts: &[Some(scene_layout), None, Some(final_layout)]"));
        assert!(!production.contains("chrome_glyph"));
    }

    #[test]
    fn analytic_shader_contract_is_closed_and_wall_mask_ignores_native_color() {
        for required in [
            "fn vs_world_analytic(",
            "fn vs_screen_analytic(",
            "frame_buffer.analytics[primitive.binding_index]",
            "analytic.rect_points.xy\n        + input.local_position.xy * analytic.rect_points.zw",
            "fn valid_analytic_role(",
            "fn fs_room_aperture(",
            "fn fs_floor_projection(",
            "fn fs_status_tone(",
            "fn fs_mood_rings(",
            "fn fs_gauges(",
            "fn fs_trouble(",
            "fn fs_dim(",
            "fn fs_aperture_composite(",
            "fn fs_wall_shadow_glyph(",
        ] {
            assert!(SCENE_SHADER_SOURCE.contains(required), "missing {required}");
        }
        let wall = SCENE_SHADER_SOURCE
            .split("fn fs_wall_shadow_glyph(")
            .nth(1)
            .unwrap()
            .split("@fragment")
            .next()
            .unwrap();
        assert!(wall.contains("coverage_texture"));
        assert!(wall.contains("content.payload[0].y"));
        assert!(wall.contains("color_texture"));
        assert!(wall.contains(").a"));
        assert!(!wall.contains(".rgb"));
        assert!(!wall.contains("palette_linear"));
        assert!(!wall.contains("analytic.payload[0].w"));

        let dim = SCENE_SHADER_SOURCE
            .split("fn fs_dim(")
            .nth(1)
            .unwrap()
            .split("@fragment")
            .next()
            .unwrap();
        assert!(
            !dim.contains("dim_amount"),
            "node opacity already owns dim amount"
        );
        assert!(dim.contains("1.0,"));
        assert!(!dim.contains("aperture"));

        let mood = SCENE_SHADER_SOURCE
            .split("fn fs_mood_rings(")
            .nth(1)
            .unwrap()
            .split("fn normalized_degrees(")
            .next()
            .unwrap();
        assert!(mood.contains("ring < 8u"));
        assert!(mood.contains("smoothstep(radius - edge, radius + edge, distance)"));
        assert!(!mood.contains("ceil("));
    }

    #[test]
    fn gauge_frame_flat_pack_matches_wgsl_vec4_reconstruction() {
        let fixture = SceneFixture::valid();
        let gauge = fixture.frame.analytic_slots[5].value.unwrap();
        let AnalyticGeometry::PerimeterGaugeSet { center_points, xp, daily, pace } = gauge.geometry
        else {
            panic!("gauge role geometry");
        };
        let expected = [
            center_points[0],
            center_points[1],
            xp.radius_points,
            xp.stroke_width_points,
            xp.track_start_degrees,
            xp.track_sweep_degrees,
            daily.radius_points,
            daily.stroke_width_points,
            daily.track_start_degrees,
            daily.track_sweep_degrees,
            pace.radius_points,
            pace.stroke_width_points,
            pace.track_start_degrees,
            pace.track_sweep_degrees,
        ];
        let candidate = compile_fixture(&fixture);
        let atlas = two_weight_atlas_for('^', candidate.generation_key.resources);
        let upload = prepare_scene_upload(&candidate, &atlas).unwrap();
        let record = PackedMirrorLayout::frame_offset(FrameMirrorFamily::Analytics)
            + 5 * std::mem::size_of::<super::super::compiler::AnalyticFrameGpuValue>();
        let payload = &upload.frame_bytes[record + 32..record + 32 + 16 * 4];
        let actual = payload
            .chunks_exact(4)
            .map(|bytes| f32::from_ne_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(&actual[..expected.len()], &expected);
        for reconstruction in [
            "analytic.payload[0].z,\n                analytic.payload[0].w,\n                analytic.payload[1].x,\n                analytic.payload[1].y",
            "analytic.payload[1].z,\n                analytic.payload[1].w,\n                analytic.payload[2].x,\n                analytic.payload[2].y",
            "analytic.payload[2].z,\n                analytic.payload[2].w,\n                analytic.payload[3].x,\n                analytic.payload[3].y",
        ] {
            assert!(SCENE_SHADER_SOURCE.contains(reconstruction));
        }
        assert!(!SCENE_SHADER_SOURCE.contains("analytic.payload[4]"));
    }

    fn two_weight_atlas(scalar: char) -> super::super::resources::PreparedSceneAtlas {
        two_weight_atlas_for(
            scalar,
            crate::presentation::companion_scene::ResourceGeneration(0),
        )
    }

    fn two_weight_atlas_for(
        scalar: char,
        resource_generation: crate::presentation::companion_scene::ResourceGeneration,
    ) -> super::super::resources::PreparedSceneAtlas {
        use super::super::resources::{
            AtlasCell, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphEntryKind, GlyphKey,
            PreparedSceneAtlas,
        };
        let entries = [false, true]
            .into_iter()
            .enumerate()
            .map(|(column, bold)| {
                let mut entry = GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell {
                        origin: [column as u32, 0],
                        extent: [1, 1],
                    },
                );
                entry.visible_uv = Some([column as f32 / 2.0, 0.0, (column + 1) as f32 / 2.0, 1.0]);
                (GlyphKey::new(scalar.to_string(), bold), entry)
            })
            .collect();
        PreparedSceneAtlas::from_compiled_for_generation(
            &CompiledGlyphAtlas {
                width: 2,
                height: 1,
                rgba: vec![255; 8],
                entries,
            },
            resource_generation,
        )
        .unwrap()
    }

    fn full_hud_atlas_for(
        scalar: char,
        resource_generation: crate::presentation::companion_scene::ResourceGeneration,
        missing_regular: Option<char>,
        color_regular: Option<char>,
    ) -> super::super::resources::PreparedSceneAtlas {
        use super::super::resources::{
            AtlasCell, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphEntryKind, GlyphKey,
            PreparedSceneAtlas,
        };
        let mut glyphs = crate::round::hud::COMPANION_HUD_GLYPH_REPERTOIRE
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        glyphs.insert(scalar);
        let keys = glyphs
            .into_iter()
            .flat_map(|glyph| [false, true].map(move |bold| (glyph, bold)))
            .filter(|(glyph, bold)| *bold || missing_regular != Some(*glyph))
            .collect::<Vec<_>>();
        const CELL_EXTENT: u32 = 4;
        let width = u32::try_from(keys.len()).unwrap() * CELL_EXTENT;
        let height = CELL_EXTENT;
        let mut entries = std::collections::BTreeMap::new();
        let mut rgba = vec![0; usize::try_from(width * height).unwrap() * 4];
        for (entry_index, (glyph, bold)) in keys.into_iter().enumerate() {
            let origin_x = u32::try_from(entry_index).unwrap() * CELL_EXTENT;
            let cell = AtlasCell {
                origin: [origin_x, 0],
                extent: [CELL_EXTENT; 2],
            };
            let mut entry = if glyph == ' ' {
                GlyphAtlasEntry::whitespace(29.0, 52.0, cell)
            } else {
                let kind = if !bold && color_regular == Some(glyph) {
                    GlyphEntryKind::PremultipliedColorRgba
                } else {
                    GlyphEntryKind::Mask
                };
                GlyphAtlasEntry::synthetic_visible(kind, cell)
            };
            if glyph != ' ' {
                entry.visible_uv = Some([
                    (origin_x as f32 + 1.5) / width as f32,
                    1.5 / height as f32,
                    (origin_x as f32 + 2.5) / width as f32,
                    2.5 / height as f32,
                ]);
                for row in 1..=2_u32 {
                    let alpha = match (glyph, row) {
                        ('r', 1) => 64,
                        ('r', 2) => 192,
                        ('e', _) => 128,
                        _ => 255,
                    };
                    for column in 1..=2_u32 {
                        let pixel = ((row * width + origin_x + column) * 4) as usize;
                        rgba[pixel..pixel + 4].copy_from_slice(&[alpha, alpha, alpha, alpha]);
                    }
                }
            }
            entries.insert(GlyphKey::new(glyph.to_string(), bold), entry);
        }
        PreparedSceneAtlas::from_compiled_for_generation(
            &CompiledGlyphAtlas { width, height, rgba, entries },
            resource_generation,
        )
        .unwrap()
    }

    fn compile_fixture(fixture: &SceneFixture) -> super::super::compiler::CpuSceneCandidate {
        super::super::compiler::compile_fixture_for_render_test(fixture)
    }

    fn canonical_materialization_fixture() -> SceneFixture {
        let mut fixture = SceneFixture::valid();
        let alias = |value: &str| CanonicalAlias::new(value).unwrap();
        let unlit_alias = alias("material.scene-unlit-analytic");
        let multiply_alias = alias("material.scene-multiply");
        let chrome_alias = alias("material.scene-chrome");
        let analytic_resource_alias = alias("resource.scene-analytic");
        let hud_resource_alias = alias("resource.scene-hud-atlas");
        let unlit = MaterialId::from_alias(&unlit_alias);
        let multiply = MaterialId::from_alias(&multiply_alias);
        let chrome = MaterialId::from_alias(&chrome_alias);
        let analytic_resource = ResourceId::from_alias(&analytic_resource_alias);
        let hud_resource = ResourceId::from_alias(&hud_resource_alias);
        fixture.template.materials = vec![
            MaterialTemplate {
                id: unlit,
                alias: unlit_alias,
                kind: MaterialKind::UnlitAnalytic,
            },
            MaterialTemplate {
                id: multiply,
                alias: multiply_alias,
                kind: MaterialKind::MultiplyShadow,
            },
            MaterialTemplate {
                id: chrome,
                alias: chrome_alias,
                kind: MaterialKind::ScreenChrome,
            },
        ];
        fixture.template.resources = vec![
            ResourceTemplate {
                id: analytic_resource,
                alias: analytic_resource_alias,
                kind: ResourceKind::AnalyticGeometry,
            },
            ResourceTemplate {
                id: hud_resource,
                alias: hud_resource_alias,
                kind: ResourceKind::GlyphAtlas,
            },
        ];
        let node = fixture.template.nodes[0].id;
        let bounds = Bounds3 { min: [0.0; 3], max: [1.0, 1.0, 0.0] };
        let analytic = |binding: u8,
                        material: MaterialId,
                        blend: WorldBlend,
                        depth: DepthBehavior,
                        space: PrimitiveSpace,
                        authored_order: u16| PrimitiveTemplate {
            node,
            kind: PrimitiveKind::AnalyticShape,
            material,
            resource: Some(analytic_resource),
            blend,
            depth,
            binding: PrimitiveBinding::Analytic(AnalyticParamId(binding)),
            authored_order,
            local_geometry: bounds,
            space,
        };
        fixture.template.primitives = vec![
            analytic(
                0,
                unlit,
                WorldBlend::Opaque,
                DepthBehavior::WorldWrite,
                PrimitiveSpace::World,
                0,
            ),
            analytic(
                2,
                multiply,
                WorldBlend::Multiply,
                DepthBehavior::WorldReadOnly,
                PrimitiveSpace::World,
                1,
            ),
            analytic(
                5,
                chrome,
                WorldBlend::PremultipliedAlpha,
                DepthBehavior::ScreenNoDepth,
                PrimitiveSpace::Screen,
                2,
            ),
            analytic(
                3,
                chrome,
                WorldBlend::PremultipliedAlpha,
                DepthBehavior::ScreenNoDepth,
                PrimitiveSpace::Screen,
                3,
            ),
            analytic(
                6,
                chrome,
                WorldBlend::PremultipliedAlpha,
                DepthBehavior::ScreenNoDepth,
                PrimitiveSpace::Screen,
                4,
            ),
            PrimitiveTemplate {
                node,
                kind: PrimitiveKind::InstanceQuad,
                material: chrome,
                resource: Some(hud_resource),
                blend: WorldBlend::PremultipliedAlpha,
                depth: DepthBehavior::ScreenNoDepth,
                binding: PrimitiveBinding::Instances(InstanceGroupBinding::Hud),
                authored_order: 5,
                local_geometry: bounds,
                space: PrimitiveSpace::Screen,
            },
            analytic(
                7,
                chrome,
                WorldBlend::PremultipliedAlpha,
                DepthBehavior::ScreenNoDepth,
                PrimitiveSpace::Screen,
                6,
            ),
        ];
        // Keep the synthetic GPU fixture observable through its final dim pass:
        // binding dim to the fractional-opacity child prevents it from hiding
        // every preceding world, chrome, and HUD contribution.
        fixture.template.primitives[6].node = fixture.template.nodes[1].id;
        fixture.frame.nodes[1].opacity = 0.25;
        for slot in &mut fixture.template.static_atlas_recipes {
            slot.recipe = None;
        }
        fixture
    }

    #[test]
    fn gpu_upload_records_have_locked_sizes_alignments_and_offsets() {
        assert_eq!(std::mem::size_of::<PrimitiveGpuValue>(), 64);
        assert_eq!(std::mem::align_of::<PrimitiveGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, node_index), 0);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, material_index), 4);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, aux_node_index), 8);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, primitive_kind), 12);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, material_kind), 16);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, resource_kind), 20);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, blend), 24);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, depth), 28);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, space), 32);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, instance_group), 36);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, instance_base), 40);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, binding_index), 44);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, authored_order), 48);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, content_base), 52);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, frame_base), 56);
        assert_eq!(
            std::mem::offset_of!(PrimitiveGpuValue, aux_content_base),
            60
        );

        assert_eq!(std::mem::size_of::<SceneContentGpuValue>(), 32);
        assert_eq!(std::mem::align_of::<SceneContentGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(SceneContentGpuValue, kind), 0);
        assert_eq!(
            std::mem::offset_of!(SceneContentGpuValue, glyph_entry_index),
            4
        );
        assert_eq!(std::mem::offset_of!(SceneContentGpuValue, slot), 8);
        assert_eq!(std::mem::offset_of!(SceneContentGpuValue, subslot), 12);
        assert_eq!(std::mem::offset_of!(SceneContentGpuValue, signed_data), 16);
        assert_eq!(std::mem::offset_of!(SceneContentGpuValue, flags), 24);
        assert_eq!(std::mem::offset_of!(SceneContentGpuValue, variant), 28);

        assert_eq!(std::mem::size_of::<GlyphAtlasGpuEntry>(), 64);
        assert_eq!(std::mem::align_of::<GlyphAtlasGpuEntry>(), 4);
        assert_eq!(std::mem::offset_of!(GlyphAtlasGpuEntry, visible_uv), 0);
        assert_eq!(
            std::mem::offset_of!(GlyphAtlasGpuEntry, ink_origin_size),
            16
        );
        assert_eq!(std::mem::offset_of!(GlyphAtlasGpuEntry, metrics), 32);
        assert_eq!(std::mem::offset_of!(GlyphAtlasGpuEntry, flags), 44);
        assert_eq!(std::mem::offset_of!(GlyphAtlasGpuEntry, allocated_cell), 48);
        assert_eq!(
            (0..6)
                .map(scene_storage_min_binding_size)
                .collect::<Vec<_>>(),
            vec![12_288, 160, 7_680, 64, 15_552, 64],
        );
    }

    #[test]
    fn scene_glyph_weight_policy_covers_every_role_layer_and_regular_family() {
        for (tag, role) in [
            (1, PetPaletteRole::Body),
            (2, PetPaletteRole::BodyGlow),
            (3, PetPaletteRole::Eye),
            (4, PetPaletteRole::Mouth),
            (5, PetPaletteRole::Accent),
            (6, PetPaletteRole::Pattern),
            (7, PetPaletteRole::Particle),
            (8, PetPaletteRole::Corruption),
        ] {
            assert_eq!(
                SceneGlyphWeightPolicy::pet_is_bold(tag),
                role == PetPaletteRole::Eye,
                "{role:?}",
            );
        }
        assert!(!SceneGlyphWeightPolicy::content_is_bold(
            ContentMirrorFamily::TankGlyphs,
            0,
            [0, 0],
        ));
        assert!(SceneGlyphWeightPolicy::content_is_bold(
            ContentMirrorFamily::TankGlyphs,
            0,
            [0, 1],
        ));
        for family in [
            ContentMirrorFamily::PropGlyphs,
            ContentMirrorFamily::Ambient,
            ContentMirrorFamily::RoomGlyphs,
        ] {
            assert!(!SceneGlyphWeightPolicy::content_is_bold(family, 0, [0; 2]));
        }
        assert!(SceneGlyphWeightPolicy::content_is_bold(
            ContentMirrorFamily::PetParticles,
            3,
            [0; 2],
        ));
    }

    #[test]
    fn full_key_translation_distinguishes_weights_preserves_none_and_types_failures() {
        let atlas = two_weight_atlas('^');
        let lookup = SceneGlyphLookup::from_atlas(&atlas);
        let regular = SceneContentGpuValue::translate(
            ContentMirrorFamily::Pet,
            super::super::compiler::ContentUploadValue::fixture(u32::from('^'), 1),
            &lookup,
        )
        .unwrap();
        let bold = SceneContentGpuValue::translate(
            ContentMirrorFamily::Pet,
            super::super::compiler::ContentUploadValue::fixture(u32::from('^'), 3),
            &lookup,
        )
        .unwrap();
        assert_ne!(regular.glyph_entry_index, bold.glyph_entry_index);

        let none = SceneContentGpuValue::translate(
            ContentMirrorFamily::Ambient,
            super::super::compiler::ContentUploadValue::fixture(u32::MAX, 0),
            &lookup,
        )
        .unwrap();
        assert_eq!(none.glyph_entry_index, u32::MAX);
        assert!(matches!(
            SceneContentGpuValue::translate(
                ContentMirrorFamily::Ambient,
                super::super::compiler::ContentUploadValue::fixture(0x11_0000, 0),
                &lookup,
            ),
            Err(SceneUploadError::InvalidGlyphScalar { scalar: 0x11_0000, .. })
        ));
        assert!(matches!(
            SceneContentGpuValue::translate(
                ContentMirrorFamily::Ambient,
                super::super::compiler::ContentUploadValue::fixture(u32::from('x'), 0),
                &lookup,
            ),
            Err(SceneUploadError::MissingGlyphKey { .. })
        ));
    }

    #[test]
    fn scalar_glyph_lookup_is_generation_bound_sorted_stable_and_omits_sequences() {
        use super::super::resources::{
            AtlasCell, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphEntryKind, GlyphKey,
            PreparedSceneAtlas,
        };
        use std::collections::BTreeMap;

        let generation = crate::presentation::companion_scene::ResourceGeneration(37);
        let keys = [
            GlyphKey::new("^^", false),
            GlyphKey::new("z", true),
            GlyphKey::new("^", true),
            GlyphKey::new("z", false),
            GlyphKey::new("^", false),
        ];
        let mut entries = BTreeMap::new();
        for (column, key) in keys.into_iter().enumerate() {
            entries.insert(
                key,
                GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell {
                        origin: [column as u32, 0],
                        extent: [1, 1],
                    },
                ),
            );
        }
        let atlas = PreparedSceneAtlas::from_compiled_for_generation(
            &CompiledGlyphAtlas {
                width: 5,
                height: 1,
                rgba: vec![255; 20],
                entries,
            },
            generation,
        )
        .unwrap();
        let lookup = SceneGlyphLookup::from_atlas(&atlas);

        assert_eq!(lookup.resource_generation, generation);
        assert_eq!(lookup.entries.len(), 4, "multi-scalar atlas keys stay out");
        assert!(
            lookup
                .entries
                .windows(2)
                .all(|pair| pair[0].key() < pair[1].key()),
            "lookup keys remain strictly sorted for binary search",
        );
        for (scalar, bold) in [('^', false), ('^', true), ('z', false), ('z', true)] {
            let expected = atlas
                .resolve_key(&GlyphKey::new(scalar.to_string(), bold))
                .unwrap()
                .id;
            assert_eq!(lookup.resolve(u32::from(scalar), bold), Some(expected));
        }
        assert_eq!(lookup.resolve(NONE_U32, false), Some(NONE_U32));
        assert_eq!(lookup.resolve(0x11_0000, false), None);
        assert_eq!(lookup.resolve(u32::from('x'), false), None);
    }

    #[test]
    fn scene_upload_rejects_same_shape_atlas_from_another_resource_generation() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let candidate_generation = candidate.generation_key.resources;
        let matching = two_weight_atlas_for('^', candidate_generation);
        let other_generation =
            crate::presentation::companion_scene::ResourceGeneration(candidate_generation.0 + 1);
        let mismatched = two_weight_atlas_for('^', other_generation);

        assert_eq!(matching.entries.len(), mismatched.entries.len());
        assert_eq!(matching.width, mismatched.width);
        assert_eq!(matching.height, mismatched.height);
        assert_eq!(matching.coverage_r8.len(), mismatched.coverage_r8.len());
        assert_eq!(
            matching.straight_color_rgba_srgb.len(),
            mismatched.straight_color_rgba_srgb.len(),
        );
        assert!(prepare_scene_upload(&candidate, &matching).is_ok());
        assert_eq!(
            prepare_scene_upload(&candidate, &mismatched),
            Err(SceneUploadError::AtlasGenerationMismatch {
                candidate: candidate_generation,
                atlas: other_generation,
            }),
        );
    }

    #[test]
    fn scalar_lookup_happy_path_is_primitive_binary_search_without_key_allocation() {
        let production = include_str!("render.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap();
        let resolve = production
            .split("fn resolve(&self, scalar: u32, bold: bool)")
            .nth(1)
            .expect("scalar lookup resolver")
            .split("\n    }")
            .next()
            .unwrap();
        assert!(resolve.contains("binary_search_by_key"));
        assert!(!resolve.contains("GlyphKey"));
        assert!(!resolve.contains("String"));
        assert!(!resolve.contains("to_string"));
    }

    #[test]
    fn tank_atlas_weight_uses_authored_packed_bit() {
        let atlas = two_weight_atlas('^');
        let lookup = SceneGlyphLookup::from_atlas(&atlas);
        let mut regular = super::super::compiler::ContentUploadValue::fixture(u32::from('^'), 0);
        regular.kind = 3;
        let packed_color = 126 | (238 << 8) | (255 << 16);
        regular.signed_data = [packed_color, 0];
        let mut bold = regular;
        bold.signed_data[1] = 1;

        let regular =
            SceneContentGpuValue::translate(ContentMirrorFamily::TankGlyphs, regular, &lookup)
                .unwrap();
        let bold = SceneContentGpuValue::translate(ContentMirrorFamily::TankGlyphs, bold, &lookup)
            .unwrap();
        assert_ne!(regular.glyph_entry_index, bold.glyph_entry_index);
        assert_eq!(bold.signed_data, [packed_color, 1]);

        let packed = bold.signed_data[0] as u32;
        let authored_srgb8 = [
            (packed & 0xff) as u8,
            ((packed >> 8) & 0xff) as u8,
            ((packed >> 16) & 0xff) as u8,
        ];
        assert_eq!(authored_srgb8, [126, 238, 255]);
        let linear = authored_srgb8.map(|channel| scene_srgb_to_linear(f32::from(channel) / 255.0));
        assert!((linear[0] - 0.208_636_87).abs() < 1.0e-7);
        assert!((linear[1] - 0.854_992_6).abs() < 1.0e-7);
        assert_eq!(linear[2], 1.0);
    }

    #[test]
    fn tank_shader_decode_preserves_rgb_lanes_and_uses_255_normalization() {
        let source = SCENE_SHADER_SOURCE;
        let start = source
            .find("fn tank_paint_linear")
            .expect("tank paint function");
        let end = source[start..]
            .find("\n}\n")
            .map(|offset| start + offset + 3)
            .expect("tank paint function end");
        let tank_paint = &source[start..end];
        for required in [
            "f32(packed & 0xffu)",
            "f32((packed >> 8u) & 0xffu)",
            "f32((packed >> 16u) & 0xffu)",
            ") / 255.0;",
            "srgb_to_linear(straight_srgb)",
        ] {
            assert!(
                tank_paint.contains(required),
                "missing tank decode contract: {required}"
            );
        }
        assert!(!tank_paint.contains("/ 256.0"));

        let packed = 17 | (101 << 8) | (233 << 16);
        let packed = packed as u32;
        let shader_mirror = [
            (packed & 0xff) as u8,
            ((packed >> 8) & 0xff) as u8,
            ((packed >> 16) & 0xff) as u8,
        ]
        .map(|channel| scene_srgb_to_linear(f32::from(channel) / 255.0));
        assert!((shader_mirror[0] - 0.005_605_391_6).abs() < 1.0e-8);
        assert!((shader_mirror[1] - 0.130_136_48).abs() < 1.0e-7);
        assert!((shader_mirror[2] - 0.814_846_6).abs() < 2.0e-7);
    }

    #[test]
    fn candidate_translation_owns_exact_packed_bytes_and_leaves_candidate_unchanged() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let before = candidate.clone();
        let atlas = two_weight_atlas_for('^', candidate.generation_key.resources);

        let upload = prepare_scene_upload(&candidate, &atlas).unwrap();

        assert_eq!(upload.generation_key, candidate.generation_key);
        assert_eq!(upload.source_revisions, candidate.source_revisions);
        assert_eq!(upload.logical_viewport_points, [360.0, 360.0]);
        assert_eq!(
            upload.logical_viewport_points,
            candidate.logical_viewport_points()
        );
        assert_eq!(upload.static_checksum, candidate.static_checksum);
        assert_eq!(upload.node_bytes.len(), PackedMirrorLayout::NODE_BYTES);
        assert_eq!(
            upload.content_globals_bytes.len(),
            PackedMirrorLayout::CONTENT_GLOBALS_BYTES
        );
        assert_eq!(
            upload.scene_content_bytes.len(),
            PackedMirrorLayout::SCENE_CONTENT_BYTES
        );
        assert_eq!(upload.frame_bytes.len(), PackedMirrorLayout::FRAME_BYTES);
        assert_eq!(
            upload.vertex_bytes.len() % std::mem::size_of::<super::super::compiler::StaticVertex>(),
            0
        );
        assert_eq!(upload.index_bytes.len() % 4, 0);
        assert_eq!(upload.primitives.len(), candidate.primitive_count());
        assert_eq!(upload.draws.len(), candidate.primitive_count());
        assert_eq!(upload.glyph_entries.len(), 2);
        let candidate_buffer_lengths = [
            upload.vertex_bytes.len(),
            upload.index_bytes.len(),
            upload.node_bytes.len(),
            upload.content_globals_bytes.len(),
            upload.frame_bytes.len(),
            std::mem::size_of_val(upload.primitives.as_slice()),
            upload.scene_content_bytes.len(),
            std::mem::size_of_val(upload.glyph_entries.as_slice()),
        ];
        assert_eq!(candidate_buffer_lengths.len(), 8);
        assert_eq!(candidate_buffer_lengths[3], 160);
        assert_eq!(candidate_buffer_lengths[4], 7_680);
        assert_eq!(
            candidate_buffer_lengths[5],
            candidate.primitive_count() * 64
        );
        assert_eq!(candidate_buffer_lengths[6], 15_552);
        assert_eq!(candidate_buffer_lengths[7], 2 * 64);
        assert_eq!(candidate, before);
    }

    #[test]
    fn v2_room_and_analytic_families_translate_into_the_existing_two_dynamic_buffers() {
        let mut fixture = SceneFixture::valid();
        fixture.content.room_glyph_slots[0].glyph = Some(AuthoredGlyph::new('^').unwrap());
        fixture.content.room_glyph_slots[0].color_srgb8 = Some([10, 20, 30]);
        let candidate = super::super::compiler::compile_static_fixture_for_render_test(&fixture);
        let sources = candidate.frame_upload_sources();
        let upload = prepare_scene_upload(
            &candidate,
            &two_weight_atlas_for('^', candidate.generation_key.resources),
        )
        .unwrap();

        let room_content_offset =
            PackedMirrorLayout::scene_content_offset(ContentMirrorFamily::RoomGlyphs).unwrap();
        let room = bytemuck::from_bytes::<SceneContentGpuValue>(
            &upload.scene_content_bytes[room_content_offset
                ..room_content_offset + std::mem::size_of::<SceneContentGpuValue>()],
        );
        assert_eq!(room.kind, 5);
        assert_ne!(room.glyph_entry_index, NONE_U32);
        assert_eq!(room.variant, 10 | (20 << 8) | (30 << 16) | (255 << 24));

        let room_frame_offset = PackedMirrorLayout::frame_offset(FrameMirrorFamily::RoomGlyphs);
        assert_eq!(
            &upload.frame_bytes[room_frame_offset..room_frame_offset + sources.room_glyphs.len()],
            sources.room_glyphs,
        );
        let analytic_content_offset =
            PackedMirrorLayout::scene_content_offset(ContentMirrorFamily::Analytics).unwrap();
        let content_sources = candidate.content_upload_sources();
        let analytic_content_bytes: &[u8] = bytemuck::cast_slice(content_sources.analytics);
        assert_eq!(
            &upload.scene_content_bytes
                [analytic_content_offset..analytic_content_offset + analytic_content_bytes.len()],
            analytic_content_bytes,
        );
        let analytic_frame_offset = PackedMirrorLayout::frame_offset(FrameMirrorFamily::Analytics);
        assert_eq!(
            &upload.frame_bytes
                [analytic_frame_offset..analytic_frame_offset + sources.analytics.len()],
            sources.analytics,
        );
    }

    #[test]
    fn primitive_upload_preserves_authored_order_binding_ids_and_v2_arena_bases() {
        let cases = [
            (
                PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body)),
                PrimitiveKind::InstanceQuad,
                0,
                NONE_U32,
            ),
            (
                PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Particles)),
                PrimitiveKind::InstanceQuad,
                300,
                NONE_U32,
            ),
            (
                PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs),
                PrimitiveKind::InstanceQuad,
                430,
                92,
            ),
            (
                PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(4)),
                PrimitiveKind::InstanceQuad,
                166,
                4,
            ),
            (
                PrimitiveBinding::Instances(InstanceGroupBinding::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Behind,
                }),
                PrimitiveKind::InstanceQuad,
                228,
                18,
            ),
            (
                PrimitiveBinding::Instances(InstanceGroupBinding::Ambient),
                PrimitiveKind::InstanceQuad,
                236,
                26,
            ),
        ];
        for (binding, kind, content_base, frame_base) in cases {
            let mut fixture = SceneFixture::valid();
            fixture.template.primitives[0].binding = binding;
            fixture.template.primitives[0].kind = kind;
            fixture.template.primitives[0].authored_order = 77;
            let candidate =
                super::super::compiler::compile_static_fixture_for_render_test(&fixture);
            let upload = prepare_scene_upload(
                &candidate,
                &two_weight_atlas_for('^', candidate.generation_key.resources),
            )
            .unwrap();
            let primitive = upload.primitives[0];
            assert_eq!(primitive.content_base, content_base, "{binding:?}");
            assert_eq!(primitive.frame_base, frame_base, "{binding:?}");
            assert_eq!(primitive.authored_order, 77, "{binding:?}");
            assert_eq!(upload.draws[0].authored_order, 77, "{binding:?}");
        }

        let mut analytic = SceneFixture::valid();
        analytic.template.primitives[0].kind = PrimitiveKind::AnalyticShape;
        analytic.template.primitives[0].binding =
            PrimitiveBinding::Analytic(AnalyticSemantic::Gauges.id());
        analytic.template.materials[0].kind = MaterialKind::UnlitAnalytic;
        analytic.template.resources[0].kind = ResourceKind::AnalyticGeometry;
        analytic.template.primitives[0].authored_order = 91;
        let analytic_candidate =
            super::super::compiler::compile_static_fixture_for_render_test(&analytic);
        let upload = prepare_scene_upload(
            &analytic_candidate,
            &two_weight_atlas_for('^', analytic_candidate.generation_key.resources),
        )
        .unwrap();
        assert_eq!(upload.primitives[0].binding_index, 5);
        assert_eq!(upload.primitives[0].content_base, NONE_U32);
        assert_eq!(upload.primitives[0].frame_base, 5);
        assert_eq!(upload.primitives[0].aux_content_base, NONE_U32);
        assert_eq!(upload.primitives[0].authored_order, 91);

        let mut wall = SceneFixture::valid();
        let mut body = wall.template.primitives[0].clone();
        body.kind = PrimitiveKind::InstanceQuad;
        body.binding =
            PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body));
        wall.template.primitives[0].kind = PrimitiveKind::AnalyticShape;
        wall.template.primitives[0].binding =
            PrimitiveBinding::Analytic(AnalyticSemantic::WallShadow.id());
        wall.template.materials[0].kind = MaterialKind::MultiplyShadow;
        wall.template.resources[0].kind = ResourceKind::AnalyticGeometry;
        wall.template.primitives.push(body);
        let wall_candidate = super::super::compiler::compile_static_fixture_for_render_test(&wall);
        let upload = prepare_scene_upload(
            &wall_candidate,
            &two_weight_atlas_for('^', wall_candidate.generation_key.resources),
        )
        .unwrap();
        assert_eq!(upload.primitives[0].binding_index, 1);
        assert_eq!(upload.primitives[0].aux_content_base, 0);
        assert_eq!(
            upload.primitives[0].aux_node_index,
            upload.primitives[1].node_index
        );
        assert_eq!(upload.draws[0].instance_range, 0..130);
        assert_eq!(
            upload.draws[0].source,
            PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask),
            "the wall silhouette is a typed fixed glyph-mask draw, not an analytic quad",
        );
    }

    #[test]
    fn primitive_dispatch_flattens_instance_bases_and_locks_exact_ranges() {
        let cases = [
            (
                InstanceGroupBinding::PetArt(PetArtFilter::Body),
                0,
                PrimitiveSource::Instances(InstanceSource::PetBody),
                0..130,
            ),
            (
                InstanceGroupBinding::PetArt(PetArtFilter::Particles),
                0,
                PrimitiveSource::Instances(InstanceSource::PetParticles),
                0..130,
            ),
            (
                InstanceGroupBinding::RoomGlyphs,
                0,
                PrimitiveSource::Instances(InstanceSource::RoomGlyphs),
                0..32,
            ),
            (
                InstanceGroupBinding::PropGlyphs(4),
                4 * 9,
                PrimitiveSource::Instances(InstanceSource::PropGlyphs { slot: 4 }),
                0..9,
            ),
            (
                InstanceGroupBinding::TankCells { slot: 1, layer: InstanceLayer::Behind },
                8,
                PrimitiveSource::Instances(InstanceSource::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Behind,
                }),
                0..8,
            ),
            (
                InstanceGroupBinding::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Foreground,
                },
                8,
                PrimitiveSource::Instances(InstanceSource::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Foreground,
                }),
                0..8,
            ),
            (
                InstanceGroupBinding::Ambient,
                0,
                PrimitiveSource::Instances(InstanceSource::Ambient),
                0..64,
            ),
            (
                InstanceGroupBinding::Hud,
                0,
                PrimitiveSource::Instances(InstanceSource::Hud),
                0..0,
            ),
        ];

        for (binding, expected_base, expected_source, expected_instances) in cases {
            let mut fixture = SceneFixture::valid();
            fixture.template.primitives[0].kind = PrimitiveKind::InstanceQuad;
            fixture.template.primitives[0].binding = PrimitiveBinding::Instances(binding);
            let candidate =
                super::super::compiler::compile_static_fixture_for_render_test(&fixture);
            let upload = prepare_scene_upload(
                &candidate,
                &two_weight_atlas_for('^', candidate.generation_key.resources),
            )
            .unwrap();
            assert_eq!(upload.primitives[0].instance_base, expected_base);
            assert_eq!(upload.draws[0].index_range, 0..6);
            assert_eq!(upload.draws[0].instance_range, expected_instances);
            assert_eq!(upload.draws[0].source, expected_source);
        }

        let static_candidate = compile_fixture(&SceneFixture::valid());
        let static_upload = prepare_scene_upload(
            &static_candidate,
            &two_weight_atlas_for('^', static_candidate.generation_key.resources),
        )
        .unwrap();
        assert_eq!(static_upload.primitives[0].instance_base, u32::MAX);
        assert_eq!(static_upload.draws[0].source, PrimitiveSource::StaticAtlas);
        assert_eq!(static_upload.draws[0].instance_range, 0..1);

        let mut analytic_fixture = SceneFixture::valid();
        analytic_fixture.template.primitives[0].kind = PrimitiveKind::AnalyticShape;
        analytic_fixture.template.primitives[0].binding =
            PrimitiveBinding::Analytic(AnalyticSemantic::RoomBackground.id());
        analytic_fixture.template.materials[0].kind = MaterialKind::UnlitAnalytic;
        analytic_fixture.template.resources[0].kind = ResourceKind::AnalyticGeometry;
        let analytic_candidate =
            super::super::compiler::compile_static_fixture_for_render_test(&analytic_fixture);
        let analytic_upload = prepare_scene_upload(
            &analytic_candidate,
            &two_weight_atlas_for('^', analytic_candidate.generation_key.resources),
        )
        .unwrap();
        assert_eq!(analytic_upload.primitives[0].instance_base, u32::MAX);
        assert_eq!(analytic_upload.draws[0].source, PrimitiveSource::Analytic);
        assert_eq!(analytic_upload.draws[0].instance_range, 0..1);
    }

    #[test]
    fn role_only_delta_changes_upload_entry_without_resource_generation_or_mirror_rewrite() {
        let fixture = SceneFixture::valid();
        let mut candidate = compile_fixture(&fixture);
        let atlas = two_weight_atlas_for('^', candidate.generation_key.resources);
        let regular = prepare_scene_upload(&candidate, &atlas).unwrap();
        let generation = candidate.generation_key;

        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            candidate.source_revisions.semantic.0 + 1,
            candidate.source_revisions.frame.0 + 1,
        );
        let mut content = ContentDelta::empty();
        content.generation_key = generation;
        content.from = candidate.source_revisions;
        content.to = to;
        let mut changed = fixture.content.pet_art_slots[0];
        changed.palette_role = PetPaletteRole::Eye;
        content.pet_art_slots.push(changed);
        let mut frame = FrameDelta::empty();
        frame.generation_key = generation;
        frame.from = candidate.source_revisions;
        frame.to = to;
        candidate.apply_deltas(&content, &frame).unwrap();
        let after_delta = candidate.clone();

        let bold = prepare_scene_upload(&candidate, &atlas).unwrap();

        let glyph_word =
            PackedMirrorLayout::scene_content_offset(ContentMirrorFamily::Pet).unwrap() + 4;
        assert_ne!(
            &regular.scene_content_bytes[glyph_word..glyph_word + 4],
            &bold.scene_content_bytes[glyph_word..glyph_word + 4]
        );
        assert_eq!(candidate.generation_key, generation);
        assert_eq!(candidate, after_delta);
    }

    #[test]
    fn shallow_card_components_fail_preflight_with_typed_primitive_index() {
        for kind in [
            UnsupportedSceneFeature::ShallowCardPrimitive,
            UnsupportedSceneFeature::LitShallowCardMaterial,
            UnsupportedSceneFeature::ShallowCardGeometryResource,
        ] {
            let mut fixture = SceneFixture::valid();
            match kind {
                UnsupportedSceneFeature::ShallowCardPrimitive => {
                    fixture.template.primitives[0].kind = PrimitiveKind::ShallowCard;
                }
                UnsupportedSceneFeature::LitShallowCardMaterial => {
                    fixture.template.materials[0].kind = MaterialKind::LitShallowCard;
                }
                UnsupportedSceneFeature::ShallowCardGeometryResource => {
                    fixture.template.resources[0].kind = ResourceKind::ShallowCardGeometry;
                }
            }
            let candidate =
                super::super::compiler::compile_static_fixture_for_render_test(&fixture);
            assert_eq!(
                prepare_scene_upload(
                    &candidate,
                    &two_weight_atlas_for('^', candidate.generation_key.resources),
                ),
                Err(SceneUploadError::UnsupportedPrimitive { primitive_index: 0, feature: kind })
            );
        }

        let candidate = compile_fixture(&SceneFixture::valid());
        prepare_scene_upload(
            &candidate,
            &two_weight_atlas_for('^', candidate.generation_key.resources),
        )
        .unwrap();
    }

    #[test]
    fn glyph_gpu_table_marks_visible_color_and_whitespace_without_losing_metrics() {
        use super::super::resources::{
            AtlasCell, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphEntryKind, GlyphKey,
            PreparedSceneAtlas,
        };
        let mut visible = GlyphAtlasEntry::synthetic_visible(
            GlyphEntryKind::PremultipliedColorRgba,
            AtlasCell { origin: [0, 0], extent: [1, 1] },
        );
        visible.visible_uv = Some([0.0, 0.0, 0.5, 1.0]);
        let whitespace =
            GlyphAtlasEntry::whitespace(12.0, 20.0, AtlasCell { origin: [1, 0], extent: [1, 1] });
        let atlas = PreparedSceneAtlas::from_compiled(&CompiledGlyphAtlas {
            width: 2,
            height: 1,
            rgba: vec![0; 8],
            entries: [
                (GlyphKey::new(" ", false), whitespace),
                (GlyphKey::new("^", false), visible),
            ]
            .into_iter()
            .collect(),
        })
        .unwrap();

        let table = prepare_glyph_entries(&atlas).unwrap();
        assert_eq!(table[0].flags & GLYPH_FLAG_VISIBLE, 0);
        assert_eq!(table[0].metrics[0], 12.0);
        assert_eq!(table[1].flags, GLYPH_FLAG_VISIBLE | GLYPH_FLAG_COLOR);
        assert_eq!(table[1].visible_uv, visible.visible_uv.unwrap());
        assert_eq!(
            table[1].ink_origin_size,
            [1.0, 80.0 - 2.0 * 6.0 - 2.0 - 20.0, 10.0, 20.0],
            "the GPU entry stores a Y-up bottom bearing without growing its 64-byte ABI",
        );
    }

    #[test]
    fn glyph_gpu_table_rejects_nonfinite_reversed_and_out_of_bounds_metrics() {
        use super::super::resources::{
            AtlasCell, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphEntryKind, GlyphKey,
            PreparedSceneAtlas,
        };

        let invalid_entries = [
            {
                let mut entry = GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell { origin: [0, 0], extent: [1, 1] },
                );
                entry.advance = f32::NAN;
                entry
            },
            {
                let mut entry = GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell { origin: [0, 0], extent: [1, 1] },
                );
                entry.line_height = f32::INFINITY;
                entry
            },
            {
                let mut entry = GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell { origin: [0, 0], extent: [1, 1] },
                );
                entry.ink_origin[0] = f32::NAN;
                entry
            },
            {
                let mut entry = GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell { origin: [0, 0], extent: [1, 1] },
                );
                entry.visible_uv = Some([0.8, 0.0, 0.2, 1.0]);
                entry
            },
            {
                let mut entry = GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell { origin: [0, 0], extent: [1, 1] },
                );
                entry.visible_uv = Some([0.0, 0.0, 1.01, 1.0]);
                entry
            },
            {
                let mut entry = GlyphAtlasEntry::synthetic_visible(
                    GlyphEntryKind::Mask,
                    AtlasCell { origin: [0, 0], extent: [1, 1] },
                );
                entry.ink_size = [100.0, 100.0];
                entry
            },
        ];

        for entry in invalid_entries {
            let key = GlyphKey::new("^", false);
            let atlas = PreparedSceneAtlas::from_compiled(&CompiledGlyphAtlas {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
                entries: [(key.clone(), entry)].into_iter().collect(),
            })
            .unwrap();
            assert_eq!(
                prepare_glyph_entries(&atlas).unwrap_err(),
                SceneUploadError::InvalidGlyphEntry { key },
            );
        }

        let wrong_key = GlyphKey::new("!", false);
        let mut wrong_cell = GlyphAtlasEntry::synthetic_visible(
            GlyphEntryKind::Mask,
            AtlasCell { origin: [0, 0], extent: [1, 1] },
        );
        wrong_cell.visible_uv = Some([0.5, 0.0, 1.0, 1.0]);
        let mut valid_other_cell = GlyphAtlasEntry::synthetic_visible(
            GlyphEntryKind::Mask,
            AtlasCell { origin: [1, 0], extent: [1, 1] },
        );
        valid_other_cell.visible_uv = Some([0.5, 0.0, 1.0, 1.0]);
        let atlas = PreparedSceneAtlas::from_compiled(&CompiledGlyphAtlas {
            width: 2,
            height: 1,
            rgba: vec![0; 8],
            entries: [
                (wrong_key.clone(), wrong_cell),
                (GlyphKey::new("^", false), valid_other_cell),
            ]
            .into_iter()
            .collect(),
        })
        .unwrap();
        assert_eq!(
            prepare_glyph_entries(&atlas).unwrap_err(),
            SceneUploadError::InvalidGlyphEntry { key: wrong_key },
            "an in-range UV rectangle may not escape its allocated atlas cell",
        );
    }

    #[test]
    fn glyph_placement_contract_locks_pet_grid_metric_ink_and_family_origins() {
        let cell = [8.0, 12.0];
        assert_eq!(
            SceneGlyphPlacementContract::pet_cell_base(0, cell),
            Some([0.0, 108.0])
        );
        assert_eq!(
            SceneGlyphPlacementContract::pet_cell_base(12, cell),
            Some([96.0, 108.0])
        );
        assert_eq!(
            SceneGlyphPlacementContract::pet_cell_base(13, cell),
            Some([0.0, 96.0])
        );
        assert_eq!(
            SceneGlyphPlacementContract::pet_cell_base(129, cell),
            Some([96.0, 0.0])
        );
        assert_eq!(SceneGlyphPlacementContract::pet_cell_base(130, cell), None);

        let entry = GlyphAtlasGpuEntry {
            visible_uv: [0.0, 0.0, 1.0, 1.0],
            ink_origin_size: [2.0, 4.0, 10.0, 20.0],
            metrics: [20.0, 40.0, 18.0],
            flags: GLYPH_FLAG_VISIBLE,
            allocated_cell: [0, 0, 16, 24],
        };
        for (actual, expected) in [
            (
                SceneGlyphPlacementContract::metric_ink_offset([0.0, 0.0], entry, cell),
                [0.6, 1.2],
            ),
            (
                SceneGlyphPlacementContract::metric_ink_offset([1.0, 1.0], entry, cell),
                [3.6, 7.2],
            ),
        ] {
            let actual = actual.expect("valid metrics produce an ink offset");
            assert!((actual[0] - expected[0]).abs() < 1e-5);
            assert!((actual[1] - expected[1]).abs() < 1e-5);
        }
        let mut wide = entry;
        wide.ink_origin_size = [0.0, 0.0, 80.0, 20.0];
        wide.metrics = [80.0, 40.0, 18.0];
        let regular_scale = SceneGlyphPlacementContract::one_cell_scale(entry, cell).unwrap();
        let wide_scale = SceneGlyphPlacementContract::one_cell_scale(wide, cell).unwrap();
        assert!((regular_scale - 0.3).abs() < 1e-5);
        assert!((wide_scale - 0.1).abs() < 1e-5);
        for (candidate, scale) in [(entry, regular_scale), (wide, wide_scale)] {
            let fitted = [
                candidate.ink_origin_size[2] * scale,
                candidate.ink_origin_size[3] * scale,
            ];
            assert!(fitted[0] <= cell[0] && fitted[1] <= cell[1]);
            assert!(
                (fitted[0] / fitted[1]
                    - candidate.ink_origin_size[2] / candidate.ink_origin_size[3])
                    .abs()
                    < 1e-5,
                "one-cell fitting remains uniform and preserves glyph aspect",
            );
        }
        assert_eq!(
            SceneGlyphPlacementContract::prop_cell_base([100.0, 200.0], [3.0, -4.0], [2, 1], cell,),
            [119.0, 184.0],
        );
        assert_eq!(
            SceneGlyphPlacementContract::tank_cell_base([90.0, 70.0], cell),
            [86.0, 64.0],
        );
        assert_eq!(
            SceneGlyphPlacementContract::direct_cell_base([23.0, 41.0]),
            [23.0, 41.0],
        );
    }

    #[test]
    fn glyph_placement_contract_locks_visibility_opacity_and_wall_projection() {
        assert_eq!(
            SceneGlyphPlacementContract::frame_opacity(1, 0.625),
            Some(0.625)
        );
        assert_eq!(SceneGlyphPlacementContract::frame_opacity(0, 0.625), None);
        assert!(SceneGlyphPlacementContract::tank_cell_visible(3, 1, 5));
        assert!(!SceneGlyphPlacementContract::tank_cell_visible(3, 2, 5));
        assert!(SceneGlyphPlacementContract::tank_cell_visible(3, 2, 6));
        assert!(!SceneGlyphPlacementContract::tank_cell_visible(1, 1, 5));
        assert!(!SceneGlyphPlacementContract::tank_cell_visible(3, 1, 3));

        // A negative X scale around a translated pivot mirrors the pet cell.
        // The wall offset is applied after that auxiliary pet transform; the
        // primary wall node supplies only Z/opacity in the GPU contract.
        assert_eq!(
            SceneGlyphPlacementContract::wall_xy(
                [12.0, 7.0],
                [-1.0, 0.0, 0.0, 1.0, 80.0, 30.0],
                [5.0, -3.0],
            ),
            [73.0, 34.0],
        );
    }

    #[test]
    fn scene_wgsl_locks_buffer_abi_entrypoints_and_color_responsibilities() {
        let source = SCENE_SHADER_SOURCE;
        for required in [
            "const GLYPH_FLAG_COLOR: u32 = 2u;",
            "struct PrimitiveGpuValue",
            "aux_node_index: u32",
            "instance_base: u32",
            "binding_index: u32",
            "authored_order: u32",
            "content_base: u32",
            "frame_base: u32",
            "aux_content_base: u32",
            "struct SceneContentGpuValue",
            "struct AnalyticContentGpuValue",
            "struct AnalyticFrameGpuValue",
            "glyph_entry_index: u32",
            "struct GlyphAtlasGpuEntry",
            "struct NodeBuffer",
            "struct ContentGlobalsBuffer",
            "struct FrameBuffer",
            "values: array<FrameGpuValue, 124>",
            "analytics: array<AnalyticFrameGpuValue, 16>",
            "struct PrimitiveBuffer",
            "struct SceneContentBuffer",
            "values: array<SceneContentGpuValue, 462>",
            "analytics: array<AnalyticContentGpuValue, 16>",
            "struct GlyphEntryBuffer",
            "@group(0) @binding(0) var<storage, read> node_buffer: NodeBuffer;",
            "@group(0) @binding(1) var<storage, read> content_globals_buffer: ContentGlobalsBuffer;",
            "@group(0) @binding(2) var<storage, read> frame_buffer: FrameBuffer;",
            "@group(0) @binding(3) var<storage, read> primitive_buffer: PrimitiveBuffer;",
            "@group(0) @binding(4) var<storage, read> scene_content_buffer: SceneContentBuffer;",
            "@group(0) @binding(5) var<storage, read> glyph_entry_buffer: GlyphEntryBuffer;",
            "world_position.y = world_position.y + node.depth_cue.y;",
            "output.opacity = node.opacity * f32(node.visible);",
            "point_position.y * 2.0 / frame_buffer.globals.viewport_points.y - 1.0",
            "if (primitive.content_base == NONE_U32)",
            "return primitive.content_base + instance_index;",
            "fn vs_world(",
            "fn vs_world_glyph(",
            "fn vs_screen(",
            "fn fs_analytic(",
            "fn fs_glyph(",
            "let atlas_local = vec2<f32>(input.uv.x, 1.0 - input.uv.y);",
            "return mix(entry.visible_uv.xy, entry.visible_uv.zw, atlas_local);",
            "textureSampleLevel(coverage_texture",
            "textureSampleLevel(color_texture",
            "fn tank_paint_linear(content: SceneContentGpuValue) -> vec4<f32>",
            "let packed = u32(content.signed_data.x);",
            "if (content.kind == 3u)",
            "return tank_paint_linear(content);",
            "fn glyph_instance_placement(",
            "primitive.aux_content_base,\n            input.instance_index",
            "input.instance_index % 13u",
            "input.instance_index / 13u",
            "9u - pet_row",
            "primitive.frame_base + input.instance_index",
            "let frame = frame_buffer.values[primitive.frame_base];",
            "content.signed_data.x",
            "- f32(content.signed_data.y) * cell_extent.y",
            "frame.values[2] - 0.5 * cell_extent.x",
            "frame.values[3] - 0.5 * cell_extent.y",
            "frame.values[0]",
            "frame.values[1]",
            "analytic.payload[0].y",
            "analytic.payload[0].z",
            "let aux_node = node_buffer.values[primitive.aux_node_index];",
            "if (placement.valid == 0u)",
            "output.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);",
            "if (content_index >= 462u)",
            "if (frame_index >= 124u)",
            "content.glyph_entry_index >= arrayLength(&glyph_entry_buffer.values)",
            "fn explicit_packed_paint_linear(content: SceneContentGpuValue)",
            "(content.flags & 64u) != 0u",
            "(content.flags & 1u) != 0u",
            "(content.flags & 256u) != 0u",
            "srgb_to_linear(straight_srgb)",
            "discard;",
            "fn vs_final(",
            "fn fs_final(",
            "textureLoad(scene_sampled_texture",
            "if (sampled.a == 0.0)",
        ] {
            assert!(
                source.contains(required),
                "missing WGSL contract: {required}"
            );
        }
        for forbidden in [
            "remove_srgb_suffix",
            "premultiply_gamma_srgb",
            "depth_cue.z * node.opacity",
            "node.opacity * node.depth_cue.z",
            "1.0 - point_position.y",
            "fn fs_coverage(",
            "fn fs_color(",
            "var<uniform> frame_globals",
            "clip.z =",
            "return 300u;",
            "frame_buffer.values[primitive.frame_base + input.instance_index] // prop",
        ] {
            assert!(
                !source.contains(forbidden),
                "forbidden WGSL contract: {forbidden}"
            );
        }
        let module = wgpu::naga::front::wgsl::parse_str(source).expect("scene WGSL parses");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("scene WGSL validates");
    }

    #[test]
    fn aperture_composite_is_the_only_circular_clip_authority() {
        let aperture = SCENE_SHADER_SOURCE
            .split("fn fs_room_aperture(")
            .nth(1)
            .expect("room analytic role exists")
            .split("fn fs_floor_projection(")
            .next()
            .expect("room role has a bounded body");
        assert!(!aperture.contains("1.0 - smoothstep"));
        assert!(!aperture.contains("let coverage"));

        let dim = SCENE_SHADER_SOURCE
            .split("fn fs_dim(")
            .nth(1)
            .unwrap()
            .split("@fragment")
            .next()
            .unwrap();
        assert!(!dim.contains("aperture"));

        let composite = SCENE_SHADER_SOURCE
            .split("fn fs_aperture_composite(")
            .nth(1)
            .expect("aperture composite exists")
            .split("@fragment")
            .next()
            .expect("aperture composite has a bounded body");
        for required in [
            "frame_buffer.analytics[0u]",
            "scene_content_buffer.analytics[0u]",
            "valid_analytic_role(0u, aperture, aperture_content)",
            "textureDimensions(scene_sampled_texture)",
            "frame_buffer.globals.viewport_points",
            "textureLoad(scene_sampled_texture",
            "sampled * coverage",
        ] {
            assert!(
                composite.contains(required),
                "missing composite contract: {required}"
            );
        }
        for forbidden in ["frame_buffer.globals.aperture", "viewport_pixels"] {
            assert!(
                !composite.contains(forbidden),
                "reserved global used: {forbidden}"
            );
        }

        let entry = SCENE_SHADER_SOURCE.split("fn fs_analytic(").nth(1).unwrap();
        assert!(entry.contains("if (output.a <= 0.0) {\n        discard;\n    }"));

        let final_vertex = SCENE_SHADER_SOURCE
            .split("fn vs_final(")
            .nth(1)
            .unwrap()
            .split("@fragment")
            .next()
            .unwrap();
        assert!(final_vertex.contains("vertex_index & 1u) * 4 - 1"));
        assert!(final_vertex.contains("vertex_index & 2u) * 2 - 1"));
        let oversized_triangle = (0..3_u32)
            .map(|index| (((index & 1) as i32 * 4 - 1), ((index & 2) as i32 * 2 - 1)))
            .collect::<Vec<_>>();
        assert_eq!(oversized_triangle, [(-1, -1), (3, -1), (-1, 3)]);

        let rust_source = include_str!("render.rs");
        let aperture_encoder = rust_source
            .split("fn encode_aperture_composite(")
            .nth(1)
            .unwrap()
            .split("/// IEC 61966-2-1")
            .next()
            .unwrap();
        assert!(aperture_encoder.contains("pass.draw(0..3, 0..1);"));
        assert!(!aperture_encoder.contains("pass.draw(0..6"));

        let world_encoder = rust_source
            .split("fn encode_scene_world(")
            .nth(1)
            .unwrap()
            .split("fn encode_scene_draws_without_depth")
            .next()
            .unwrap();
        assert!(world_encoder.contains("load: wgpu::LoadOp::Clear(1.0)"));
        assert!(world_encoder.contains("store: wgpu::StoreOp::Discard"));
    }

    #[test]
    fn analytic_fragment_discards_zero_alpha_output_before_returning() {
        let analytic = SCENE_SHADER_SOURCE
            .split("fn fs_analytic(")
            .nth(1)
            .expect("analytic fragment entry point exists")
            .split("@fragment")
            .next()
            .expect("analytic fragment has a bounded body");
        let compute = analytic
            .find("var output = vec4<f32>(0.0);")
            .expect("analytic output starts fail-closed");
        let discard = analytic
            .find("if (output.a <= 0.0) {\n        discard;\n    }")
            .expect("zero-alpha analytic output is discarded");
        let returned = analytic
            .find("return output;")
            .expect("validated analytic output is returned");
        assert!(compute < discard && discard < returned);
    }

    #[test]
    fn gpu_resource_accounting_is_exact_and_not_a_live_global_metric() {
        assert_eq!(SceneGpuSharedFacts::EXPECTED.bind_group_layouts, 4);
        assert_eq!(SceneGpuSharedFacts::EXPECTED.samplers, 1);
        assert_eq!(SceneGpuSharedFacts::EXPECTED.pipelines, 11);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.buffers, 10);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.textures, 2);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.texture_views, 2);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.bind_groups, 4);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.static_uploads, 10);
        assert_eq!(SceneTargetFacts::EXPECTED.textures, 3);
        assert_eq!(SceneTargetFacts::EXPECTED.texture_views, 3);
        assert_eq!(SceneTargetFacts::EXPECTED.bind_groups, 2);
        assert_eq!(
            SceneGpuSharedFacts::EXPECTED.persistent_owned_handles()
                + GpuSceneCandidateFacts::EXPECTED.persistent_owned_handles()
                + SceneTargetFacts::EXPECTED.persistent_owned_handles(),
            42,
        );
    }

    #[test]
    fn dedicated_hud_shader_contract_matches_fixed_rust_abi_and_private_group() {
        for required in [
            "struct HudGlyphGpuValue {",
            "rect_points: vec4<f32>",
            "glyph_entry_index: u32",
            "role: u32",
            "visible: u32",
            "padding: u32",
            "@group(3) @binding(0) var<storage, read> hud_glyph_buffer",
            "fn vs_hud(",
            "@builtin(vertex_index) vertex_index: u32",
            "@builtin(instance_index) instance_index: u32",
            "fn fs_hud(",
            "arrayLength(&glyph_entry_buffer.values)",
            "vec4<f32>(0.93, 0.93, 0.97, 1.0)",
            "vec4<f32>(0.62, 0.63, 0.77, 1.0)",
        ] {
            assert!(SCENE_SHADER_SOURCE.contains(required), "missing {required}");
        }

        let vertex = SCENE_SHADER_SOURCE
            .split("fn vs_hud(")
            .nth(1)
            .expect("HUD vertex entry")
            .split("@fragment")
            .next()
            .expect("bounded HUD vertex body");
        let invisible = vertex
            .find("instance.visible == 0u")
            .expect("invisible guard");
        let off_clip = vertex
            .find("vec4<f32>(2.0, 2.0, 0.0, 1.0)")
            .expect("off-clip output");
        assert!(invisible < off_clip);

        let fragment = SCENE_SHADER_SOURCE
            .split("fn fs_hud(")
            .nth(1)
            .expect("HUD fragment entry");
        let visible = fragment.find("input.visible == 0u").expect("visible guard");
        let bounds = fragment
            .find("arrayLength(&glyph_entry_buffer.values)")
            .expect("glyph bounds guard");
        let lookup = fragment
            .find("glyph_entry_buffer.values[input.glyph_entry_index]")
            .expect("glyph lookup");
        assert!(visible < bounds && bounds < lookup);
    }

    #[test]
    fn candidate_buffer_usages_preserve_immutable_and_dirty_span_contracts() {
        let immutable = [
            SceneBufferUsages::VERTEX,
            SceneBufferUsages::INDEX,
            SceneBufferUsages::PRIMITIVE,
            SceneBufferUsages::GLYPH_ENTRY,
        ];
        assert_eq!(immutable[0], wgpu::BufferUsages::VERTEX);
        assert_eq!(immutable[1], wgpu::BufferUsages::INDEX);
        assert_eq!(immutable[2], wgpu::BufferUsages::STORAGE);
        assert_eq!(immutable[3], wgpu::BufferUsages::STORAGE);
        assert!(immutable
            .into_iter()
            .all(|usage| !usage.contains(wgpu::BufferUsages::COPY_DST)));
        let mutable = [
            SceneBufferUsages::NODE,
            SceneBufferUsages::CONTENT_GLOBALS,
            SceneBufferUsages::FRAME,
            SceneBufferUsages::SCENE_CONTENT,
        ];
        for usage in mutable {
            assert_eq!(
                usage,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );
        }
        assert_eq!(mutable.len(), 4);
    }

    #[test]
    fn committed_pending_shadow_commit_swaps_storage_without_growth_or_reallocation() {
        let initial = (0_u8..32).collect::<Vec<_>>();
        let mut shadow = SceneBufferShadow::from_bytes(&initial);
        assert_eq!(shadow.committed.as_ref(), initial.as_slice());
        assert_eq!(shadow.pending.as_ref(), initial.as_slice());
        assert_eq!(shadow.committed.len(), 32);
        assert_eq!(shadow.pending.len(), 32);

        let committed_ptr = shadow.committed.as_ptr();
        let pending_ptr = shadow.pending.as_ptr();
        assert_ne!(committed_ptr, pending_ptr);
        shadow.pending[4..8].copy_from_slice(&[90, 91, 92, 93]);
        shadow.commit();

        assert_eq!(shadow.committed.as_ptr(), pending_ptr);
        assert_eq!(shadow.pending.as_ptr(), committed_ptr);
        assert_eq!(&shadow.committed[4..8], &[90, 91, 92, 93]);
        assert_eq!(&shadow.pending[4..8], &[4, 5, 6, 7]);
        assert_eq!(shadow.committed.len(), 32);
        assert_eq!(shadow.pending.len(), 32);
    }

    #[test]
    fn final_pipeline_layout_preserves_only_the_group_two_binding_slot() {
        let production = include_str!("render.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("render production source precedes tests");
        assert!(production.contains("bind_group_layouts: &[None, None, Some(final_layout)]"));
        assert!(!production.contains(
            "bind_group_layouts: &[Some(scene_layout), Some(atlas_layout), Some(final_layout)]"
        ));
    }

    #[test]
    fn gpu_error_category_priority_is_oom_then_internal_then_validation() {
        assert_eq!(
            select_scoped_gpu_error(
                Some(ScopedGpuErrorCategory::Validation),
                Some(ScopedGpuErrorCategory::OutOfMemory),
                Some(ScopedGpuErrorCategory::Internal),
            ),
            Some(ScopedGpuErrorCategory::OutOfMemory),
        );
        assert_eq!(
            select_scoped_gpu_error(
                Some(ScopedGpuErrorCategory::Validation),
                None,
                Some(ScopedGpuErrorCategory::Internal),
            ),
            Some(ScopedGpuErrorCategory::Internal),
        );
        assert_eq!(
            select_scoped_gpu_error(Some(ScopedGpuErrorCategory::Validation), None, None),
            Some(ScopedGpuErrorCategory::Validation),
        );
        assert_eq!(std::mem::size_of::<ScopedGpuErrorCategory>(), 1);
    }

    #[test]
    fn target_key_rejects_non_2d_extent_wrong_formats_and_multisampling() {
        let valid = SceneTargetKey::new(
            crate::presentation::companion_scene::DeviceEpoch(4),
            crate::presentation::companion_scene::SurfaceEpoch(9),
            wgpu::Extent3d {
                width: 260,
                height: 260,
                depth_or_array_layers: 1,
            },
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Depth24Plus,
            1,
        )
        .unwrap();
        assert_eq!(valid.extent.width, 260);

        for extent in [
            wgpu::Extent3d {
                width: 0,
                height: 260,
                depth_or_array_layers: 1,
            },
            wgpu::Extent3d {
                width: 260,
                height: 0,
                depth_or_array_layers: 1,
            },
            wgpu::Extent3d {
                width: 260,
                height: 260,
                depth_or_array_layers: 0,
            },
            wgpu::Extent3d {
                width: 260,
                height: 260,
                depth_or_array_layers: 2,
            },
        ] {
            assert_eq!(
                SceneTargetKey::new(
                    valid.device_epoch,
                    valid.surface_epoch,
                    extent,
                    valid.surface_format,
                    valid.intermediate_format,
                    valid.depth_format,
                    valid.sample_count,
                ),
                Err(SceneTargetKeyError::Extent),
            );
        }
        assert_eq!(
            SceneTargetKey::new(
                valid.device_epoch,
                valid.surface_epoch,
                valid.extent,
                valid.surface_format,
                valid.intermediate_format,
                valid.depth_format,
                4,
            ),
            Err(SceneTargetKeyError::SampleCount),
        );
        assert_eq!(
            SceneTargetKey::new(
                valid.device_epoch,
                valid.surface_epoch,
                valid.extent,
                wgpu::TextureFormat::Bgra8Unorm,
                valid.intermediate_format,
                valid.depth_format,
                valid.sample_count,
            ),
            Err(SceneTargetKeyError::Formats),
        );
    }

    fn render_request_fixture(
        generation: crate::presentation::companion_scene::SceneGenerationKey,
        applied: crate::presentation::companion_scene::AppliedRevisions,
        logical: [f32; 2],
        scale: f64,
    ) -> SceneRenderRequest {
        SceneRenderRequest::new(
            crate::presentation::companion_scene::SceneVersion {
                generation,
                surface: crate::presentation::companion_scene::SurfaceEpoch(23),
                applied,
            },
            [
                super::super::host::physical_dimension(f64::from(logical[0]), scale),
                super::super::host::physical_dimension(f64::from(logical[1]), scale),
            ],
            scale,
        )
    }

    fn paired_render_deltas(
        candidate: &CpuSceneCandidate,
        fixture: &SceneFixture,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> (ContentDelta, FrameDelta) {
        let mut content = ContentDelta::empty();
        content.generation_key = candidate.generation_key;
        content.from = candidate.source_revisions;
        content.to = to;
        content.palette = Some([
            [12, 18, 31],
            [30, 42, 68],
            [59, 81, 122],
            [126, 238, 255],
            [255, 191, 105],
            [255, 126, 184],
            [172, 255, 141],
            [236, 239, 255],
        ]);
        let mut pet = fixture.content.pet_art_slots[0];
        pet.palette_role = PetPaletteRole::Eye;
        content.pet_art_slots.push(pet);

        let mut frame = FrameDelta::empty();
        frame.generation_key = candidate.generation_key;
        frame.from = candidate.source_revisions;
        frame.to = to;
        frame.gauges = Some([0.15, 0.35, 0.55, 0.75]);
        frame.dim_amount = Some(0.22);
        (content, frame)
    }

    fn assert_scene_shadows_synchronized(state: &GpuSceneGenerationState) {
        for shadow in [
            &state.nodes,
            &state.content_globals,
            &state.frame,
            &state.scene_content,
        ] {
            assert_eq!(shadow.committed, shadow.pending);
            assert_ne!(shadow.committed.as_ptr(), shadow.pending.as_ptr());
        }
    }

    #[test]
    fn render_request_derives_only_fixed_target_facts_across_size_and_scale_matrix() {
        let generation = crate::presentation::companion_scene::SceneGenerationKey {
            device: crate::presentation::companion_scene::DeviceEpoch(4),
            layout: crate::presentation::companion_scene::LayoutGeneration(5),
            resources: crate::presentation::companion_scene::ResourceGeneration(6),
        };
        let applied = crate::presentation::companion_scene::AppliedRevisions::new(7, 8);
        for size in [260.0_f32, 360.0, 480.0, 720.0] {
            for scale in [1.0_f64, 2.0] {
                let request = render_request_fixture(generation, applied, [size, size], scale);
                let SceneRenderRequest {
                    version,
                    physical_extent_pixels,
                    backing_scale,
                } = request.clone();
                assert_eq!(version.generation, generation);
                assert_eq!(physical_extent_pixels, request.physical_extent_pixels);
                assert_eq!(backing_scale, scale);
                let key = derive_scene_target_key(
                    generation.device,
                    generation,
                    applied,
                    [size, size],
                    16_384,
                    &request,
                )
                .unwrap();
                let expected = (f64::from(size) * scale).round() as u32;
                assert_eq!([key.extent.width, key.extent.height], [expected; 2]);
                assert_eq!(key.device_epoch, generation.device);
                assert_eq!(key.surface_epoch, request.version.surface);
                assert_eq!(key.surface_format, wgpu::TextureFormat::Bgra8UnormSrgb);
                assert_eq!(key.intermediate_format, SceneTextureContract::INTERMEDIATE);
                assert_eq!(key.depth_format, SceneTextureContract::DEPTH);
                assert_eq!(key.sample_count, SceneTextureContract::SAMPLE_COUNT);
            }
        }
    }

    #[test]
    fn render_request_rejects_each_physical_axis_and_invalid_scale_exactly() {
        let generation = crate::presentation::companion_scene::SceneGenerationKey {
            device: crate::presentation::companion_scene::DeviceEpoch(4),
            layout: crate::presentation::companion_scene::LayoutGeneration(5),
            resources: crate::presentation::companion_scene::ResourceGeneration(6),
        };
        let applied = crate::presentation::companion_scene::AppliedRevisions::new(7, 8);
        let logical = [360.0, 260.0];
        let valid = render_request_fixture(generation, applied, logical, 2.0);
        let derive = |request: &SceneRenderRequest| {
            derive_scene_target_key(
                generation.device,
                generation,
                applied,
                logical,
                16_384,
                request,
            )
        };

        for (index, axis) in [SceneRequestAxis::Width, SceneRequestAxis::Height]
            .into_iter()
            .enumerate()
        {
            let mut empty = valid.clone();
            empty.physical_extent_pixels[index] = 0;
            assert_eq!(
                derive(&empty),
                Err(SceneRenderRequestError::EmptyPhysicalExtent { axis }),
            );

            let mut resized = valid.clone();
            resized.physical_extent_pixels[index] += 1;
            assert_eq!(
                derive(&resized),
                Err(SceneRenderRequestError::PhysicalExtentMismatch {
                    axis,
                    requested: valid.physical_extent_pixels[index] + 1,
                    expected: valid.physical_extent_pixels[index],
                }),
            );
        }
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut invalid = valid.clone();
            invalid.backing_scale = scale;
            assert_eq!(
                derive(&invalid),
                Err(SceneRenderRequestError::InvalidBackingScale),
            );
        }
        let tiny_scale = render_request_fixture(generation, applied, logical, f64::MIN_POSITIVE);
        let tiny_key = derive(&tiny_scale).unwrap();
        assert_eq!([tiny_key.extent.width, tiny_key.extent.height], [1, 1]);

        let product_overflow = render_request_fixture(generation, applied, logical, f64::MAX);
        assert_eq!(
            derive(&product_overflow),
            Err(SceneRenderRequestError::PhysicalDimensionOverflow {
                axis: SceneRequestAxis::Width,
            }),
        );
        let finite_clamp_scale = (f64::from(u32::MAX) + 1024.0) / f64::from(logical[0]);
        let finite_clamp = render_request_fixture(generation, applied, logical, finite_clamp_scale);
        assert_eq!(
            derive(&finite_clamp),
            Err(SceneRenderRequestError::PhysicalDimensionOverflow {
                axis: SceneRequestAxis::Width,
            }),
        );

        let width_limited = render_request_fixture(generation, applied, logical, 2.0);
        assert_eq!(
            derive_scene_target_key(
                generation.device,
                generation,
                applied,
                logical,
                719,
                &width_limited,
            ),
            Err(SceneRenderRequestError::PhysicalDimensionLimitExceeded {
                axis: SceneRequestAxis::Width,
                required: 720,
                maximum: 719,
            }),
        );
        let tall_logical = [260.0, 360.0];
        let height_limited = render_request_fixture(generation, applied, tall_logical, 2.0);
        assert_eq!(
            derive_scene_target_key(
                generation.device,
                generation,
                applied,
                tall_logical,
                719,
                &height_limited,
            ),
            Err(SceneRenderRequestError::PhysicalDimensionLimitExceeded {
                axis: SceneRequestAxis::Height,
                required: 720,
                maximum: 719,
            }),
        );
    }

    #[test]
    fn render_request_rejects_candidate_version_and_device_drift() {
        let generation = crate::presentation::companion_scene::SceneGenerationKey {
            device: crate::presentation::companion_scene::DeviceEpoch(4),
            layout: crate::presentation::companion_scene::LayoutGeneration(5),
            resources: crate::presentation::companion_scene::ResourceGeneration(6),
        };
        let applied = crate::presentation::companion_scene::AppliedRevisions::new(7, 8);
        let valid = render_request_fixture(generation, applied, [360.0; 2], 1.0);

        let mut wrong_generation = valid.clone();
        wrong_generation.version.generation.layout =
            crate::presentation::companion_scene::LayoutGeneration(99);
        assert!(matches!(
            derive_scene_target_key(
                generation.device,
                generation,
                applied,
                [360.0; 2],
                16_384,
                &wrong_generation,
            ),
            Err(SceneRenderRequestError::GenerationMismatch { .. })
        ));

        let mut wrong_applied = valid.clone();
        wrong_applied.version.applied =
            crate::presentation::companion_scene::AppliedRevisions::new(7, 99);
        assert!(matches!(
            derive_scene_target_key(
                generation.device,
                generation,
                applied,
                [360.0; 2],
                16_384,
                &wrong_applied,
            ),
            Err(SceneRenderRequestError::AppliedRevisionsMismatch { .. })
        ));

        assert!(matches!(
            derive_scene_target_key(
                crate::presentation::companion_scene::DeviceEpoch(99),
                generation,
                applied,
                [360.0; 2],
                16_384,
                &valid,
            ),
            Err(SceneRenderRequestError::SharedDeviceEpochMismatch { .. })
        ));
        assert_eq!(
            derive_scene_target_key(
                generation.device,
                generation,
                applied,
                [f32::NAN, 360.0],
                16_384,
                &valid,
            ),
            Err(SceneRenderRequestError::InvalidLogicalViewport),
        );
    }

    #[cfg(target_os = "macos")]
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
        .expect("a surfaceless Metal adapter is available");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glorp-scene-resource-test-device"),
            ..Default::default()
        }))
        .expect("a surfaceless Metal device is available")
    }

    #[cfg(target_os = "macos")]
    fn native_device_pair() -> [(wgpu::Device, wgpu::Queue); 2] {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("a surfaceless Metal adapter is available");
        std::array::from_fn(|index| {
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some(if index == 0 {
                    "glorp-scene-identity-test-device-a"
                } else {
                    "glorp-scene-identity-test-device-b"
                }),
                ..Default::default()
            }))
            .expect("two surfaceless Metal devices are available from one adapter")
        })
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_shared_and_candidate_materialization_validate_without_a_surface() {
        let (device, queue) = native_device();
        let candidate = compile_fixture(&canonical_materialization_fixture());
        let upload = prepare_scene_upload(
            &candidate,
            &full_hud_atlas_for('^', candidate.generation_key.resources, None, None),
        )
        .unwrap();
        let atlas = full_hud_atlas_for('^', upload.generation_key.resources, None, None);
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        assert_eq!(shared.facts(), SceneGpuSharedFacts::EXPECTED);
        assert_eq!(
            shared.max_texture_dimension_2d,
            device.limits().max_texture_dimension_2d,
        );

        let gpu = materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        assert_eq!(gpu.facts(), GpuSceneCandidateFacts::EXPECTED);
        assert_eq!(gpu.generation_key, upload.generation_key);
        assert_eq!(gpu.source_revisions, upload.source_revisions);
        assert_eq!(gpu.logical_viewport_points, upload.logical_viewport_points);
        assert_eq!(gpu.static_checksum, upload.static_checksum);
        let GpuSceneGenerationState {
            glyph_lookup: _,
            nodes: _,
            content_globals: _,
            frame: _,
            scene_content: _,
        } = &gpu.generation_state;
        assert_eq!(gpu.generation_state.glyph_lookup, upload.glyph_lookup);
        let assert_shadow = |shadow: &SceneBufferShadow, expected: &[u8]| {
            assert_eq!(shadow.committed.as_ref(), expected);
            assert_eq!(shadow.pending.as_ref(), expected);
            assert_eq!(shadow.committed.len(), expected.len());
            assert_eq!(shadow.pending.len(), expected.len());
            assert_ne!(shadow.committed.as_ptr(), shadow.pending.as_ptr());
        };
        assert_shadow(&gpu.generation_state.nodes, &upload.node_bytes);
        assert_shadow(
            &gpu.generation_state.content_globals,
            &upload.content_globals_bytes,
        );
        assert_shadow(&gpu.generation_state.frame, &upload.frame_bytes);
        assert_shadow(
            &gpu.generation_state.scene_content,
            &upload.scene_content_bytes,
        );
        assert_eq!(
            gpu.draw_plan,
            validate_scene_draw_plan(&upload.primitives, &upload.draws, &upload.phases).unwrap(),
        );
        for (class, expected) in [
            (
                ScenePipelineClass::WorldOpaqueAnalytic,
                &shared.pipelines.world_opaque_analytic,
            ),
            (
                ScenePipelineClass::WorldSourceOverAnalytic,
                &shared.pipelines.world_source_over_analytic,
            ),
            (
                ScenePipelineClass::WorldSourceOverGlyph,
                &shared.pipelines.world_source_over_glyph,
            ),
            (
                ScenePipelineClass::WorldMultiplyAnalytic,
                &shared.pipelines.world_multiply_analytic,
            ),
            (
                ScenePipelineClass::WorldMultiplyGlyphMask,
                &shared.pipelines.world_multiply_glyph_mask,
            ),
            (
                ScenePipelineClass::WorldAdditiveGlyph,
                &shared.pipelines.world_additive_glyph,
            ),
            (
                ScenePipelineClass::WorldAdditiveAnalyticReserved,
                &shared.pipelines.world_additive_analytic_reserved,
            ),
            (
                ScenePipelineClass::ChromeAnalytic,
                &shared.pipelines.chrome_analytic,
            ),
            (
                ScenePipelineClass::SealedHudHook,
                &shared.pipelines.chrome_hud,
            ),
        ] {
            assert!(std::ptr::eq(shared.pipelines.for_class(class), expected));
        }
        let request = render_request_fixture(
            gpu.generation_key,
            gpu.source_revisions,
            gpu.logical_viewport_points,
            2.0,
        );
        let binding = bind_scene_render_request(&shared, &gpu, request.clone()).unwrap();
        assert_eq!(binding.request, request);
        assert_eq!(binding.target_key.extent.width, 720);
        assert_eq!(binding.target_key.extent.height, 720);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_delta_render_matches_fresh_upload_shadows_and_pixels() {
        let (device, queue) = native_device();
        let fixture = canonical_materialization_fixture();
        let mut cpu = compile_fixture(&fixture);
        let mut expected_cpu = cpu.clone();
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            cpu.source_revisions.semantic.0 + 1,
            cpu.source_revisions.frame.0 + 1,
        );
        let (content, frame) = paired_render_deltas(&cpu, &fixture, to);
        expected_cpu.apply_deltas(&content, &frame).unwrap();
        let expected_upload = prepare_scene_upload(&expected_cpu, &atlas).unwrap();
        let request = render_request_fixture(
            cpu.generation_key,
            to,
            expected_cpu.logical_viewport_points(),
            1.0,
        );
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);

        let incremental = renderer
            .render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            )
            .unwrap();

        assert_eq!(cpu, expected_cpu);
        assert_eq!(candidate.source_revisions, to);
        assert_eq!(
            candidate.logical_viewport_points,
            expected_cpu.logical_viewport_points()
        );
        assert_eq!(
            candidate.generation_state.nodes.committed.as_ref(),
            expected_upload.node_bytes.as_slice(),
        );
        assert_eq!(
            candidate
                .generation_state
                .content_globals
                .committed
                .as_ref(),
            expected_upload.content_globals_bytes.as_slice(),
        );
        assert_eq!(
            candidate.generation_state.frame.committed.as_ref(),
            expected_upload.frame_bytes.as_slice(),
        );
        assert_eq!(
            candidate.generation_state.scene_content.committed.as_ref(),
            expected_upload.scene_content_bytes.as_slice(),
        );
        assert_scene_shadows_synchronized(&candidate.generation_state);

        let mut fresh =
            materialize_gpu_candidate(&device, &queue, &shared, &expected_upload, &atlas).unwrap();
        let fresh_hud = fresh
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(expected_upload.generation_key.resources),
            )
            .unwrap();
        let mut fresh_renderer = SceneRenderer::new(&device, &queue, &shared);
        let fresh_outcome = fresh_renderer
            .render_offscreen(&device, &queue, &shared, &mut fresh, request, &fresh_hud)
            .unwrap();
        assert_eq!(incremental, fresh_outcome);
        assert_eq!(renderer.delta_events_for_test(), (1, 4));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_noop_delta_advances_revisions_without_scene_buffer_copies() {
        let (device, queue) = native_device();
        let cpu_fixture = canonical_materialization_fixture();
        let mut cpu = compile_fixture(&cpu_fixture);
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            cpu.source_revisions.semantic.0 + 1,
            cpu.source_revisions.frame.0 + 1,
        );
        let mut content = ContentDelta::empty();
        content.generation_key = cpu.generation_key;
        content.from = cpu.source_revisions;
        content.to = to;
        let mut frame = FrameDelta::empty();
        frame.generation_key = cpu.generation_key;
        frame.from = cpu.source_revisions;
        frame.to = to;
        let before = candidate.generation_state.clone();
        let request =
            render_request_fixture(cpu.generation_key, to, cpu.logical_viewport_points(), 1.0);
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);

        renderer
            .render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request,
                &hud,
            )
            .unwrap();

        assert_eq!(cpu.source_revisions, to);
        assert_eq!(candidate.source_revisions, to);
        assert_eq!(candidate.generation_state, before);
        assert_scene_shadows_synchronized(&candidate.generation_state);
        assert_eq!(renderer.delta_events_for_test(), (1, 0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_delta_glyph_preflight_is_atomic_before_cache_encoder_and_belt_writes() {
        let (device, queue) = native_device();
        let fixture = canonical_materialization_fixture();
        let mut cpu = compile_fixture(&fixture);
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            cpu.source_revisions.semantic.0 + 1,
            cpu.source_revisions.frame.0 + 1,
        );
        let mut content = ContentDelta::empty();
        content.generation_key = cpu.generation_key;
        content.from = cpu.source_revisions;
        content.to = to;
        let mut room_content = fixture.content.room_glyph_slots[0];
        room_content.glyph = Some(AuthoredGlyph::new('◆').unwrap());
        room_content.color_srgb8 = Some([215, 121, 255]);
        content.room_glyph_slots.push(room_content);
        let mut frame = FrameDelta::empty();
        frame.generation_key = cpu.generation_key;
        frame.from = cpu.source_revisions;
        frame.to = to;
        let mut room_frame = fixture.frame.room_glyph_slots[0];
        room_frame.visible = true;
        room_frame.grid_cell = [1, 2];
        room_frame.position_points = [12.0, 324.0];
        room_frame.opacity = 1.0;
        frame.room_glyph_slots.push(room_frame);
        let before_cpu = cpu.clone();
        let before_gpu = candidate.generation_state.clone();
        let before_hud = candidate.hud.staging_facts_for_test();
        let request =
            render_request_fixture(cpu.generation_key, to, cpu.logical_viewport_points(), 1.0);
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);

        let error = renderer
            .render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request,
                &hud,
            )
            .unwrap_err();
        assert!(
            matches!(
                error,
                SceneRenderError::Delta(SceneDeltaRenderError::Upload(
                    SceneUploadError::MissingGlyphKey { .. }
                ))
            ),
            "unexpected preflight error: {error:?}"
        );
        assert_eq!(cpu, before_cpu);
        assert_eq!(candidate.source_revisions, before_cpu.source_revisions);
        assert_eq!(candidate.generation_state, before_gpu);
        assert_eq!(candidate.hud.staging_facts_for_test(), before_hud);
        assert_eq!(renderer.cache_and_submission_events_for_test(), (0, 0, 0));
        assert_eq!(renderer.delta_events_for_test(), (0, 0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_delta_scoped_failure_preserves_canonical_state_and_identical_retry_commits() {
        let (device, queue) = native_device();
        let fixture = canonical_materialization_fixture();
        let mut cpu = compile_fixture(&fixture);
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            cpu.source_revisions.semantic.0 + 1,
            cpu.source_revisions.frame.0 + 1,
        );
        let (content, frame) = paired_render_deltas(&cpu, &fixture, to);
        let request =
            render_request_fixture(cpu.generation_key, to, cpu.logical_viewport_points(), 1.0);
        let before_cpu = cpu.clone();
        let before_gpu = candidate.generation_state.clone();
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);
        renderer.inject_test_fault(SceneRenderTestFault::ScopedValidationAfterHudWrite);

        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Gpu(ScopedGpuErrorCategory::Validation)),
        );
        assert_eq!(cpu, before_cpu);
        assert_eq!(candidate.source_revisions, before_cpu.source_revisions);
        assert_eq!(candidate.generation_state, before_gpu);
        assert_eq!(renderer.delta_events_for_test(), (1, 4));

        renderer
            .render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request,
                &hud,
            )
            .unwrap();
        assert_eq!(cpu.source_revisions, to);
        assert_eq!(candidate.source_revisions, to);
        assert_ne!(candidate.generation_state, before_gpu);
        assert_scene_shadows_synchronized(&candidate.generation_state);
        assert_eq!(renderer.delta_events_for_test(), (2, 8));

        let to_again = crate::presentation::companion_scene::AppliedRevisions::new(
            to.semantic.0 + 1,
            to.frame.0 + 1,
        );
        let mut content_again = ContentDelta::empty();
        content_again.generation_key = cpu.generation_key;
        content_again.from = to;
        content_again.to = to_again;
        content_again.palette = Some([[91, 73, 151]; 8]);
        let mut frame_again = FrameDelta::empty();
        frame_again.generation_key = cpu.generation_key;
        frame_again.from = to;
        frame_again.to = to_again;
        frame_again.dim_amount = Some(0.47);
        let request_again = render_request_fixture(
            cpu.generation_key,
            to_again,
            cpu.logical_viewport_points(),
            1.0,
        );
        renderer
            .render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content_again,
                &frame_again,
                request_again,
                &hud,
            )
            .unwrap();
        assert_eq!(cpu.source_revisions, to_again);
        assert_eq!(candidate.source_revisions, to_again);
        assert_scene_shadows_synchronized(&candidate.generation_state);
        assert_eq!(renderer.delta_events_for_test(), (3, 10));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_delta_post_submit_map_failure_leaves_cpu_and_gpu_committed() {
        let (device, queue) = native_device();
        let fixture = canonical_materialization_fixture();
        let mut cpu = compile_fixture(&fixture);
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            cpu.source_revisions.semantic.0 + 1,
            cpu.source_revisions.frame.0 + 1,
        );
        let (content, frame) = paired_render_deltas(&cpu, &fixture, to);
        let mut expected = cpu.clone();
        expected.apply_deltas(&content, &frame).unwrap();
        let expected_upload = prepare_scene_upload(&expected, &atlas).unwrap();
        let request =
            render_request_fixture(cpu.generation_key, to, cpu.logical_viewport_points(), 1.0);
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);
        renderer.inject_test_fault(SceneRenderTestFault::MapCallbackCancelled);

        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::MapFailed),
        );
        assert_eq!(cpu, expected);
        assert_eq!(candidate.source_revisions, to);
        assert_eq!(
            candidate.generation_state.scene_content.committed.as_ref(),
            expected_upload.scene_content_bytes.as_slice(),
        );
        assert_eq!(renderer.delta_events_for_test(), (1, 4));
        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::DeltaPreparation(
                MirrorDeltaError::StaleBase
            )),
        );
        assert_eq!(renderer.delta_events_for_test(), (1, 4));

        let recovered = renderer
            .render_offscreen(&device, &queue, &shared, &mut candidate, request, &hud)
            .unwrap();
        assert_eq!(recovered.version.applied, to);
        assert_scene_shadows_synchronized(&candidate.generation_state);
        assert_eq!(renderer.delta_events_for_test(), (2, 4));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_delta_rejects_distinct_actual_device_and_queue_with_same_logical_epoch() {
        let [(device_a, queue_a), (device_b, queue_b)] = native_device_pair();
        let fixture = canonical_materialization_fixture();
        let mut cpu = compile_fixture(&fixture);
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device_a, upload.generation_key.device).unwrap();
        let foreign_shared =
            SceneGpuShared::create(&device_b, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device_a, &queue_a, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            cpu.source_revisions.semantic.0 + 1,
            cpu.source_revisions.frame.0 + 1,
        );
        let (content, frame) = paired_render_deltas(&cpu, &fixture, to);
        let request =
            render_request_fixture(cpu.generation_key, to, cpu.logical_viewport_points(), 1.0);
        let before_cpu = cpu.clone();
        let before_gpu = candidate.generation_state.clone();
        let mut renderer = SceneRenderer::new(&device_a, &queue_a, &shared);

        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device_b,
                &queue_b,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Delta(
                SceneDeltaRenderError::ActualDeviceMismatch,
            )),
        );
        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device_a,
                &queue_b,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Delta(
                SceneDeltaRenderError::ActualQueueMismatch,
            )),
        );
        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device_a,
                &queue_a,
                &foreign_shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Delta(
                SceneDeltaRenderError::ActualDeviceMismatch,
            )),
        );
        assert_eq!(cpu, before_cpu);
        assert_eq!(candidate.generation_state, before_gpu);
        assert_eq!(renderer.cache_and_submission_events_for_test(), (0, 0, 0));

        renderer
            .render_offscreen_with_delta(
                &device_a,
                &queue_a,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request,
                &hud,
            )
            .unwrap();
        assert_eq!(cpu.source_revisions, to);
        assert_eq!(renderer.delta_events_for_test(), (1, 4));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_delta_rejects_cpu_gpu_generation_static_revision_and_viewport_drift_pre_cache() {
        let (device, queue) = native_device();
        let fixture = canonical_materialization_fixture();
        let mut cpu = compile_fixture(&fixture);
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(
            cpu.source_revisions.semantic.0 + 1,
            cpu.source_revisions.frame.0 + 1,
        );
        let (content, frame) = paired_render_deltas(&cpu, &fixture, to);
        let request =
            render_request_fixture(cpu.generation_key, to, cpu.logical_viewport_points(), 1.0);
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);

        candidate.generation_key.layout =
            crate::presentation::companion_scene::LayoutGeneration(cpu.generation_key.layout.0 + 1);
        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Delta(
                SceneDeltaRenderError::GenerationMismatch,
            )),
        );
        candidate.generation_key = cpu.generation_key;

        candidate.static_checksum = candidate.static_checksum.wrapping_add(1);
        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Delta(
                SceneDeltaRenderError::StaticChecksumMismatch,
            )),
        );
        candidate.static_checksum = cpu.static_checksum;

        candidate.source_revisions = to;
        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Delta(
                SceneDeltaRenderError::RevisionMismatch,
            )),
        );
        candidate.source_revisions = cpu.source_revisions;

        candidate.logical_viewport_points[0] += 1.0;
        assert_eq!(
            renderer.render_offscreen_with_delta(
                &device,
                &queue,
                &shared,
                &mut cpu,
                &mut candidate,
                &content,
                &frame,
                request,
                &hud,
            ),
            Err(SceneRenderError::Delta(
                SceneDeltaRenderError::LogicalViewportMismatch,
            )),
        );
        candidate.logical_viewport_points = cpu.logical_viewport_points();
        assert_eq!(renderer.cache_and_submission_events_for_test(), (0, 0, 0));
        assert_eq!(renderer.delta_events_for_test(), (0, 0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_synthetic_offscreen_renderer_captures_pixels_and_reuses_keyed_resources() {
        let (device, queue) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let prepared_hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let initial_staging = candidate.hud.staging_facts_for_test();
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);
        let request = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            1.0,
        );

        let outcome = renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &prepared_hud,
            )
            .unwrap();
        assert_eq!(outcome.version, request.version);
        assert_eq!(
            outcome.physical_extent_pixels,
            request.physical_extent_pixels
        );
        let [width, height] = outcome.physical_extent_pixels;
        assert_eq!(outcome.rgba.len(), (width * height * 4) as usize);
        assert_eq!(&outcome.rgba[..4], &[0, 0, 0, 0]);
        let center = (((height / 2) * width + width / 2) * 4) as usize;
        let center_pixel: [u8; 4] = outcome.rgba[center..center + 4].try_into().unwrap();
        for (actual, expected) in center_pixel.into_iter().zip([21, 23, 34, 255]) {
            assert!(
                actual.abs_diff(expected) <= 2,
                "synthetic center pixel actual={center_pixel:?}"
            );
        }
        let staged = candidate.hud.staging_facts_for_test();
        assert_eq!(staged.sensitive_copies, initial_staging.sensitive_copies);
        assert_eq!(staged.redacted_copies, initial_staging.redacted_copies + 1);
        assert_eq!(renderer.cache_and_submission_events_for_test(), (1, 1, 1));

        renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &prepared_hud,
            )
            .unwrap();
        assert_eq!(renderer.cache_and_submission_events_for_test(), (1, 1, 2));

        let mut new_surface = request.clone();
        new_surface.version.surface =
            crate::presentation::companion_scene::SurfaceEpoch(new_surface.version.surface.0 + 1);
        renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                new_surface,
                &prepared_hud,
            )
            .unwrap();
        assert_eq!(renderer.cache_and_submission_events_for_test(), (2, 1, 3));

        let resized = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            0.5,
        );
        renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                resized,
                &prepared_hud,
            )
            .unwrap();
        assert_eq!(renderer.cache_and_submission_events_for_test(), (3, 2, 4));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_production_scene_renders_complete_fuzz_s3_inventory_with_redacted_hud() {
        use crate::pet::generation::Species;
        use crate::round::smooth::CompanionContentIdentity;

        let (device, queue) = native_device();
        let cpu = super::super::compiler::compile_projected_full_scene_for_render_test(0);
        let manifest = super::super::resources::GlyphRepertoireManifest::for_active_pet(
            CompanionContentIdentity::for_pet(Species::Fuzz),
            2.0,
        );
        let resources = super::super::resources::CompiledRetainedResources::compile(&manifest)
            .expect("the active Fuzz repertoire compiles");
        let atlas = super::super::resources::PreparedSceneAtlas::from_compiled_for_generation(
            resources.atlas(),
            cpu.generation_key.resources,
        )
        .expect("the active Fuzz atlas prepares for the projected scene generation");
        let upload = prepare_scene_upload(&cpu, &atlas).expect("production scene upload prepares");

        assert_eq!(cpu.logical_viewport_points(), [360.0, 360.0]);
        assert_eq!(upload.primitives.len(), 19);
        assert_eq!(upload.draws.len(), 19);
        assert_eq!(upload.phases.opaque_cutout.len(), 1);
        assert_eq!(upload.phases.world_blended_unsorted.len(), 13);
        assert_eq!(upload.phases.chrome_authored.len(), 5);
        assert_eq!(
            upload
                .draws
                .iter()
                .map(|draw| draw.authored_order)
                .collect::<Vec<_>>(),
            (0..19).collect::<Vec<_>>(),
        );

        let classified = upload
            .primitives
            .iter()
            .copied()
            .zip(&upload.draws)
            .map(|(primitive, draw)| {
                scene_pipeline_class(primitive, draw)
                    .expect("every production draw has one closed pipeline class")
            })
            .collect::<Vec<_>>();
        let ordered_schedule = upload
            .primitives
            .iter()
            .zip(&upload.draws)
            .zip(&classified)
            .enumerate()
            .map(|(index, ((primitive, draw), pipeline))| {
                let primitive_index = u32::try_from(index).unwrap();
                let phase = if upload.phases.opaque_cutout.contains(&primitive_index) {
                    SceneDrawPhase::Opaque
                } else if upload
                    .phases
                    .world_blended_unsorted
                    .contains(&primitive_index)
                {
                    SceneDrawPhase::WorldBlended
                } else {
                    assert!(upload.phases.chrome_authored.contains(&primitive_index));
                    SceneDrawPhase::Chrome
                };
                (
                    primitive_index,
                    phase,
                    *pipeline,
                    draw.source,
                    primitive.binding_index,
                    draw.authored_order,
                )
            })
            .collect::<Vec<_>>();
        use crate::presentation::companion_scene::scene::InstanceLayer;
        assert_eq!(
            ordered_schedule,
            vec![
                (
                    0,
                    SceneDrawPhase::Opaque,
                    ScenePipelineClass::WorldOpaqueAnalytic,
                    PrimitiveSource::Analytic,
                    0,
                    0
                ),
                (
                    1,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::RoomGlyphs),
                    0,
                    1
                ),
                (
                    2,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldMultiplyAnalytic,
                    PrimitiveSource::Analytic,
                    2,
                    2
                ),
                (
                    3,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldAdditiveGlyph,
                    PrimitiveSource::Instances(InstanceSource::Ambient),
                    0,
                    3
                ),
                (
                    4,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::PropGlyphs { slot: 0 }),
                    0,
                    4
                ),
                (
                    5,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::PropGlyphs { slot: 1 }),
                    1,
                    5
                ),
                (
                    6,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::TankCells {
                        slot: 0,
                        layer: InstanceLayer::Behind
                    }),
                    0,
                    6
                ),
                (
                    7,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::TankCells {
                        slot: 1,
                        layer: InstanceLayer::Behind
                    }),
                    1,
                    7
                ),
                (
                    8,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldMultiplyGlyphMask,
                    PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask),
                    1,
                    8
                ),
                (
                    9,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverAnalytic,
                    PrimitiveSource::Analytic,
                    4,
                    9
                ),
                (
                    10,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::PetBody),
                    0,
                    10
                ),
                (
                    11,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldAdditiveGlyph,
                    PrimitiveSource::Instances(InstanceSource::PetParticles),
                    0,
                    11
                ),
                (
                    12,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::TankCells {
                        slot: 0,
                        layer: InstanceLayer::Foreground
                    }),
                    0,
                    12
                ),
                (
                    13,
                    SceneDrawPhase::WorldBlended,
                    ScenePipelineClass::WorldSourceOverGlyph,
                    PrimitiveSource::Instances(InstanceSource::TankCells {
                        slot: 1,
                        layer: InstanceLayer::Foreground
                    }),
                    1,
                    13
                ),
                (
                    14,
                    SceneDrawPhase::Chrome,
                    ScenePipelineClass::ChromeAnalytic,
                    PrimitiveSource::Analytic,
                    5,
                    14
                ),
                (
                    15,
                    SceneDrawPhase::Chrome,
                    ScenePipelineClass::ChromeAnalytic,
                    PrimitiveSource::Analytic,
                    3,
                    15
                ),
                (
                    16,
                    SceneDrawPhase::Chrome,
                    ScenePipelineClass::ChromeAnalytic,
                    PrimitiveSource::Analytic,
                    6,
                    16
                ),
                (
                    17,
                    SceneDrawPhase::Chrome,
                    ScenePipelineClass::SealedHudHook,
                    PrimitiveSource::Instances(InstanceSource::Hud),
                    0,
                    17
                ),
                (
                    18,
                    SceneDrawPhase::Chrome,
                    ScenePipelineClass::ChromeAnalytic,
                    PrimitiveSource::Analytic,
                    7,
                    18
                ),
            ],
        );
        let class_count = |class| classified.iter().filter(|actual| **actual == class).count();
        assert_eq!(class_count(ScenePipelineClass::WorldOpaqueAnalytic), 1);
        assert_eq!(class_count(ScenePipelineClass::WorldSourceOverAnalytic), 1);
        assert_eq!(class_count(ScenePipelineClass::WorldSourceOverGlyph), 8);
        assert_eq!(class_count(ScenePipelineClass::WorldMultiplyAnalytic), 1);
        assert_eq!(class_count(ScenePipelineClass::WorldMultiplyGlyphMask), 1);
        assert_eq!(class_count(ScenePipelineClass::WorldAdditiveGlyph), 2);
        assert_eq!(class_count(ScenePipelineClass::ChromeAnalytic), 4);
        assert_eq!(class_count(ScenePipelineClass::SealedHudHook), 1);
        assert_eq!(
            class_count(ScenePipelineClass::WorldAdditiveAnalyticReserved),
            0,
        );
        let bindings_for = |class| {
            upload
                .primitives
                .iter()
                .zip(&classified)
                .filter_map(|(primitive, actual)| {
                    (*actual == class).then_some(primitive.binding_index)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(bindings_for(ScenePipelineClass::WorldOpaqueAnalytic), [0],);
        assert_eq!(
            bindings_for(ScenePipelineClass::WorldSourceOverAnalytic),
            [4],
        );
        assert_eq!(bindings_for(ScenePipelineClass::WorldMultiplyAnalytic), [2],);
        assert_eq!(
            bindings_for(ScenePipelineClass::WorldMultiplyGlyphMask),
            [1],
        );
        assert_eq!(bindings_for(ScenePipelineClass::WorldAdditiveGlyph), [0, 0],);
        assert_eq!(
            bindings_for(ScenePipelineClass::ChromeAnalytic),
            [5, 3, 6, 7],
        );
        assert_eq!(bindings_for(ScenePipelineClass::SealedHudHook), [0]);

        let source_count = |source| {
            upload
                .draws
                .iter()
                .filter(|draw| draw.source == source)
                .count()
        };
        assert_eq!(source_count(PrimitiveSource::Analytic), 7);
        assert_eq!(
            source_count(PrimitiveSource::Instances(InstanceSource::RoomGlyphs)),
            1,
        );
        assert_eq!(
            source_count(PrimitiveSource::Instances(InstanceSource::PetBody)),
            1,
        );
        assert_eq!(
            source_count(PrimitiveSource::Instances(InstanceSource::PetParticles)),
            1,
        );
        assert_eq!(
            source_count(PrimitiveSource::Instances(InstanceSource::Ambient)),
            1,
        );
        assert_eq!(
            source_count(PrimitiveSource::Instances(
                InstanceSource::WallShadowGlyphMask,
            )),
            1,
        );
        assert_eq!(
            source_count(PrimitiveSource::Instances(InstanceSource::Hud)),
            1,
        );
        assert_eq!(
            upload
                .draws
                .iter()
                .filter(|draw| matches!(
                    draw.source,
                    PrimitiveSource::Instances(InstanceSource::PropGlyphs { .. })
                ))
                .count(),
            2,
        );
        assert_eq!(
            upload
                .draws
                .iter()
                .filter(|draw| matches!(
                    draw.source,
                    PrimitiveSource::Instances(InstanceSource::TankCells { .. })
                ))
                .count(),
            4,
        );
        assert_eq!(
            upload
                .draws
                .iter()
                .filter_map(|draw| match draw.source {
                    PrimitiveSource::Instances(InstanceSource::PropGlyphs { slot }) => Some(slot),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [0, 1],
        );
        assert_eq!(
            upload
                .draws
                .iter()
                .filter_map(|draw| match draw.source {
                    PrimitiveSource::Instances(InstanceSource::TankCells { slot, layer }) => {
                        Some((slot, layer))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [
                (0, InstanceLayer::Behind),
                (1, InstanceLayer::Behind),
                (0, InstanceLayer::Foreground),
                (1, InstanceLayer::Foreground),
            ],
        );
        assert!(upload.draws.iter().all(|draw| !matches!(
            draw.source,
            PrimitiveSource::None | PrimitiveSource::StaticAtlas
        )));

        let content = cpu.content_upload_sources();
        let has_glyph = |slots: &[super::super::compiler::ContentGpuValue]| {
            slots
                .iter()
                .copied()
                .map(super::super::compiler::ContentUploadValue::from)
                .any(|slot| slot.glyph_scalar != NONE_U32)
        };
        assert!(has_glyph(content.pet));
        assert!(has_glyph(content.pet_particles));
        assert!(has_glyph(content.room_glyphs));
        for (slot, glyphs) in content
            .prop_glyphs
            .chunks_exact(crate::presentation::companion_scene::scene::MAX_PROP_GLYPHS_PER_SLOT)
            .take(2)
            .enumerate()
        {
            assert!(has_glyph(glyphs), "empty production prop slot {slot}");
        }
        for (slot, glyphs) in content
            .tank_glyphs
            .chunks_exact(crate::presentation::companion_scene::scene::MAX_TANK_GLYPHS_PER_SLOT)
            .take(2)
            .enumerate()
        {
            assert!(has_glyph(glyphs), "empty production tank slot {slot}");
        }
        assert!(content
            .ambient
            .iter()
            .copied()
            .map(super::super::compiler::ContentUploadValue::from)
            .all(|slot| slot.glyph_scalar == NONE_U32));

        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        assert_eq!(
            candidate
                .draw_plan
                .opaque
                .iter()
                .map(|draw| draw.primitive_index)
                .collect::<Vec<_>>(),
            [0],
        );
        assert_eq!(
            candidate
                .draw_plan
                .world_blended_unsorted
                .iter()
                .map(|draw| draw.primitive_index)
                .collect::<Vec<_>>(),
            (1..=13).collect::<Vec<_>>(),
        );
        assert_eq!(
            candidate
                .draw_plan
                .chrome
                .prefix
                .iter()
                .map(|draw| draw.primitive_index)
                .collect::<Vec<_>>(),
            [14, 15, 16],
        );
        assert_eq!(candidate.draw_plan.chrome.hud.primitive_index, 17);
        assert_eq!(candidate.draw_plan.chrome.suffix[0].primitive_index, 18);

        let prepared_hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let request = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            2.0,
        );
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);
        let outcome = renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &prepared_hud,
            )
            .expect("the production-derived scene renders without fallback");
        let staged = candidate.hud.staging_facts_for_test();
        assert_eq!(staged.sensitive_copies, 0);
        assert_eq!(staged.redacted_copies, 1);
        assert_eq!(staged.copied_bytes, super::super::hud::HUD_GPU_BUFFER_BYTES);
        assert_eq!(renderer.cache_and_submission_events_for_test(), (1, 1, 1));

        assert_eq!(outcome.version, request.version);
        assert_eq!(outcome.physical_extent_pixels, [720, 720]);
        assert_eq!(outcome.rgba.len(), 720 * 720 * 4);
        let pixel = |x: u32, y: u32| -> [u8; 4] {
            let offset = ((y * 720 + x) * 4) as usize;
            outcome.rgba[offset..offset + 4].try_into().unwrap()
        };
        for corner in [(0, 0), (719, 0), (0, 719), (719, 719)] {
            assert_eq!(pixel(corner.0, corner.1), [0, 0, 0, 0], "corner={corner:?}");
        }
        for room_probe in [(180, 360), (540, 360)] {
            assert_eq!(
                pixel(room_probe.0, room_probe.1),
                [20, 24, 37, 255],
                "room_probe={room_probe:?}",
            );
        }
        let pet_center = pixel(360, 360);
        assert_eq!(pet_center[3], 255);
        assert_ne!(pet_center, [20, 24, 37, 255]);
        let nontransparent = outcome
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[3] != 0)
            .count();
        assert!(nontransparent > 350_000, "nontransparent={nontransparent}");

        let baseline_plan = candidate.draw_plan.clone();
        let suppress = |plan: &mut SceneDrawPlan, primitive_index: u32| {
            let mut found = false;
            for draw in plan
                .opaque
                .iter_mut()
                .chain(plan.world_blended_unsorted.iter_mut())
                .chain(plan.chrome.prefix.iter_mut())
                .chain(plan.chrome.suffix.iter_mut())
            {
                if draw.primitive_index == primitive_index {
                    draw.instance_range = 0..0;
                    found = true;
                }
            }
            assert!(found, "missing planned primitive {primitive_index}");
        };
        let active_layer_omissions: &[(&str, &[u32])] = &[
            ("room background", &[0]),
            ("room glyphs", &[1]),
            ("floor multiply", &[2]),
            ("wall shadow", &[8]),
            ("aura", &[9]),
            ("tank inhabitant 0", &[6, 12]),
            ("tank inhabitant 1", &[7, 13]),
            ("pet body", &[10]),
            ("prop 0", &[4]),
            ("prop 1", &[5]),
            ("pet particles", &[11]),
            ("gauges", &[14]),
            ("status", &[15]),
        ];
        for (label, primitive_indices) in active_layer_omissions {
            candidate.draw_plan = baseline_plan.clone();
            for primitive_index in *primitive_indices {
                suppress(&mut candidate.draw_plan, *primitive_index);
            }
            let without_layer = renderer
                .render_offscreen(
                    &device,
                    &queue,
                    &shared,
                    &mut candidate,
                    request.clone(),
                    &prepared_hud,
                )
                .unwrap_or_else(|error| panic!("{label} omission render failed: {error:?}"));
            assert!(
                without_layer.rgba != outcome.rgba,
                "active production layer was inert: {label}",
            );
        }
        candidate.draw_plan = baseline_plan;

        let zero_hud = super::super::hud::CaptureSafePreparedHudFrame::zeroed_for_test(
            upload.generation_key.resources,
        );
        let without_hud = renderer
            .render_offscreen(&device, &queue, &shared, &mut candidate, request, &zero_hud)
            .expect("zeroed redacted HUD renders");
        assert!(without_hud.rgba != outcome.rgba, "redacted HUD was inert");
        let final_staging = candidate.hud.staging_facts_for_test();
        assert_eq!(final_staging.sensitive_copies, 0);
        assert_eq!(final_staging.redacted_copies, 15);
        assert_eq!(
            final_staging.copied_bytes,
            15 * super::super::hud::HUD_GPU_BUFFER_BYTES,
        );
        assert_eq!(renderer.cache_and_submission_events_for_test(), (1, 1, 15));

        // The normal production frame intentionally has no trouble or dim
        // contribution. A second unmodified production projection activates
        // both states and proves their real GPU paths independently.
        let dimmed_cpu = super::super::compiler::compile_projected_full_scene_for_render_test(45);
        let dimmed_upload = prepare_scene_upload(&dimmed_cpu, &atlas).unwrap();
        assert_eq!(dimmed_upload.primitives, upload.primitives);
        assert_eq!(dimmed_upload.draws, upload.draws);
        assert_eq!(dimmed_upload.phases, upload.phases);
        let mut dimmed_candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &dimmed_upload, &atlas).unwrap();
        let dimmed_request = render_request_fixture(
            dimmed_candidate.generation_key,
            dimmed_candidate.source_revisions,
            dimmed_candidate.logical_viewport_points,
            2.0,
        );
        let dimmed_baseline = renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut dimmed_candidate,
                dimmed_request.clone(),
                &prepared_hud,
            )
            .expect("dimmed production frame renders");
        let dimmed_plan = dimmed_candidate.draw_plan.clone();
        for (label, primitive_index) in [("trouble", 16), ("dim", 18)] {
            dimmed_candidate.draw_plan = dimmed_plan.clone();
            suppress(&mut dimmed_candidate.draw_plan, primitive_index);
            let without_layer = renderer
                .render_offscreen(
                    &device,
                    &queue,
                    &shared,
                    &mut dimmed_candidate,
                    dimmed_request.clone(),
                    &prepared_hud,
                )
                .unwrap_or_else(|error| panic!("dimmed {label} omission render failed: {error:?}"));
            assert!(
                without_layer.rgba != dimmed_baseline.rgba,
                "active dimmed production layer was inert: {label}",
            );
        }
        dimmed_candidate.draw_plan = dimmed_plan;
        let dimmed_staging = dimmed_candidate.hud.staging_facts_for_test();
        assert_eq!(dimmed_staging.sensitive_copies, 0);
        assert_eq!(dimmed_staging.redacted_copies, 3);
        assert_eq!(
            dimmed_staging.copied_bytes,
            3 * super::super::hud::HUD_GPU_BUFFER_BYTES,
        );
        assert_eq!(renderer.cache_and_submission_events_for_test(), (1, 1, 18));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_offscreen_preflight_rejects_request_and_hud_before_staging_or_submission() {
        let (device, queue) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let valid_hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);
        let mut malformed = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            1.0,
        );
        malformed.backing_scale = f64::NAN;
        let staging = candidate.hud.staging_facts_for_test();
        assert_eq!(
            renderer.render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                malformed,
                &valid_hud,
            ),
            Err(SceneRenderError::Request(
                SceneRenderRequestError::InvalidBackingScale
            )),
        );
        assert_eq!(candidate.hud.staging_facts_for_test(), staging);
        assert_eq!(renderer.cache_and_submission_events_for_test(), (0, 0, 0));

        let request = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            1.0,
        );
        let wrong_hud = super::super::hud::CaptureSafePreparedHudFrame::zeroed_for_test(
            crate::presentation::companion_scene::ResourceGeneration(
                upload.generation_key.resources.0 + 1,
            ),
        );
        assert_eq!(
            renderer.render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request,
                &wrong_hud,
            ),
            Err(SceneRenderError::Hud(
                super::super::hud::HudGpuStagingError::ResourceGenerationMismatch
            )),
        );
        assert_eq!(candidate.hud.staging_facts_for_test(), staging);
        assert_eq!(renderer.cache_and_submission_events_for_test(), (0, 0, 0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_renderer_is_device_epoch_bound_before_cache_encoder_or_staging_work() {
        let (device_a, queue_a) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload_a = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared_a = SceneGpuShared::create(&device_a, upload_a.generation_key.device).unwrap();
        let mut candidate_a =
            materialize_gpu_candidate(&device_a, &queue_a, &shared_a, &upload_a, &atlas).unwrap();
        let hud_a = candidate_a
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload_a.generation_key.resources),
            )
            .unwrap();
        let request_a = render_request_fixture(
            candidate_a.generation_key,
            candidate_a.source_revisions,
            candidate_a.logical_viewport_points,
            1.0,
        );
        let mut old_renderer = SceneRenderer::new(&device_a, &queue_a, &shared_a);
        old_renderer
            .render_offscreen(
                &device_a,
                &queue_a,
                &shared_a,
                &mut candidate_a,
                request_a,
                &hud_a,
            )
            .unwrap();
        assert_eq!(
            old_renderer.cache_and_submission_events_for_test(),
            (1, 1, 1)
        );

        let (device_b, queue_b) = native_device();
        let mut upload_b = upload_a.clone();
        upload_b.generation_key.device =
            crate::presentation::companion_scene::DeviceEpoch(upload_a.generation_key.device.0 + 1);
        let shared_b = SceneGpuShared::create(&device_b, upload_b.generation_key.device).unwrap();
        let mut candidate_b =
            materialize_gpu_candidate(&device_b, &queue_b, &shared_b, &upload_b, &atlas).unwrap();
        let hud_b = candidate_b
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload_b.generation_key.resources),
            )
            .unwrap();
        let request_b = render_request_fixture(
            candidate_b.generation_key,
            candidate_b.source_revisions,
            candidate_b.logical_viewport_points,
            1.0,
        );
        let staging_before = candidate_b.hud.staging_facts_for_test();
        assert_eq!(
            old_renderer.render_offscreen(
                &device_b,
                &queue_b,
                &shared_b,
                &mut candidate_b,
                request_b.clone(),
                &hud_b,
            ),
            Err(SceneRenderError::RendererDeviceEpochMismatch {
                renderer: shared_a.device_epoch,
                shared: shared_b.device_epoch,
            }),
        );
        assert_eq!(
            old_renderer.cache_and_submission_events_for_test(),
            (1, 1, 1)
        );
        assert_eq!(candidate_b.hud.staging_facts_for_test(), staging_before);

        let mut new_renderer = SceneRenderer::new(&device_b, &queue_b, &shared_b);
        let outcome = new_renderer
            .render_offscreen(
                &device_b,
                &queue_b,
                &shared_b,
                &mut candidate_b,
                request_b.clone(),
                &hud_b,
            )
            .unwrap();
        assert_eq!(outcome.version, request_b.version);
        assert_eq!(
            new_renderer.cache_and_submission_events_for_test(),
            (1, 1, 1)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_scoped_encode_failure_resets_the_belt_and_same_renderer_recovers() {
        let (device, queue) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let request = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            1.0,
        );
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);
        renderer.inject_test_fault(SceneRenderTestFault::ScopedValidationAfterHudWrite);
        assert_eq!(
            renderer.render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &hud,
            ),
            Err(SceneRenderError::Gpu(ScopedGpuErrorCategory::Validation)),
        );
        assert_eq!(renderer.cache_and_submission_events_for_test(), (1, 1, 1));
        let recovered = renderer
            .render_offscreen(&device, &queue, &shared, &mut candidate, request, &hud)
            .unwrap();
        assert_eq!(recovered.rgba.len(), 360 * 360 * 4);
        assert_eq!(renderer.cache_and_submission_events_for_test(), (1, 1, 2));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_post_submit_failures_unmap_and_recover_with_both_caches_reused() {
        let (device, queue) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let request = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            1.0,
        );
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);

        for (fault, expected) in [
            (
                SceneRenderTestFault::PollTimeout,
                SceneRenderError::PollTimeout,
            ),
            (
                SceneRenderTestFault::PollWrongSubmissionIndex,
                SceneRenderError::PollWrongSubmissionIndex,
            ),
            (
                SceneRenderTestFault::MapCallbackCancelled,
                SceneRenderError::MapFailed,
            ),
            (
                SceneRenderTestFault::MappedRangeFailure,
                SceneRenderError::MappedRangeFailed,
            ),
            (
                SceneRenderTestFault::NormalizeShortBuffer,
                SceneRenderError::Readback(
                    super::super::capture::SceneReadbackError::SourceBufferTooShort,
                ),
            ),
        ] {
            renderer.inject_test_fault(fault);
            assert_eq!(
                renderer.render_offscreen(
                    &device,
                    &queue,
                    &shared,
                    &mut candidate,
                    request.clone(),
                    &hud,
                ),
                Err(expected),
                "fault={fault:?}",
            );
            let before_recovery = renderer.cache_and_submission_events_for_test();
            assert_eq!((before_recovery.0, before_recovery.1), (1, 1));
            let recovered = renderer
                .render_offscreen(
                    &device,
                    &queue,
                    &shared,
                    &mut candidate,
                    request.clone(),
                    &hud,
                )
                .unwrap();
            assert_eq!(recovered.version, request.version);
            let after_recovery = renderer.cache_and_submission_events_for_test();
            assert_eq!((after_recovery.0, after_recovery.1), (1, 1));
            assert_eq!(after_recovery.2, before_recovery.2 + 1);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_synthetic_full_pass_changes_when_each_major_stage_is_omitted() {
        let (device, queue) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let hud = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(upload.generation_key.resources),
            )
            .unwrap();
        let zero_hud = super::super::hud::CaptureSafePreparedHudFrame::zeroed_for_test(
            upload.generation_key.resources,
        );
        let request = render_request_fixture(
            candidate.generation_key,
            candidate.source_revisions,
            candidate.logical_viewport_points,
            1.0,
        );
        let mut renderer = SceneRenderer::new(&device, &queue, &shared);
        let baseline_plan = candidate.draw_plan.clone();
        let baseline = renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &hud,
            )
            .unwrap();

        candidate
            .draw_plan
            .world_blended_unsorted
            .retain(|draw| draw.pipeline != ScenePipelineClass::WorldMultiplyAnalytic);
        assert_eq!(
            baseline_plan.world_blended_unsorted.len(),
            candidate.draw_plan.world_blended_unsorted.len() + 1,
        );
        let without_floor = renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &hud,
            )
            .unwrap();
        candidate.draw_plan = baseline_plan.clone();
        assert!(
            without_floor.rgba != baseline.rgba,
            "floor omission was inert"
        );
        assert_eq!(candidate.draw_plan, baseline_plan);

        for draw in &mut candidate.draw_plan.chrome.prefix {
            draw.instance_range = 0..0;
        }
        let without_chrome_prefix = renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &hud,
            )
            .unwrap();
        candidate.draw_plan = baseline_plan.clone();
        assert!(
            without_chrome_prefix.rgba != baseline.rgba,
            "chrome-prefix omission was inert"
        );
        assert_eq!(candidate.draw_plan, baseline_plan);

        let without_hud = renderer
            .render_offscreen(
                &device,
                &queue,
                &shared,
                &mut candidate,
                request.clone(),
                &zero_hud,
            )
            .unwrap();
        assert!(without_hud.rgba != baseline.rgba, "HUD omission was inert");
        assert_eq!(candidate.draw_plan, baseline_plan);

        candidate.draw_plan.chrome.suffix[0].instance_range = 0..0;
        let without_dim = renderer
            .render_offscreen(&device, &queue, &shared, &mut candidate, request, &hud)
            .unwrap();
        candidate.draw_plan = baseline_plan.clone();
        assert!(without_dim.rgba != baseline.rgba, "dim omission was inert");
        assert_eq!(candidate.draw_plan, baseline_plan);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_materialization_rejects_malformed_static_and_reserved_plans_before_allocation() {
        let (device, queue) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();

        let mut malformed_upload = upload.clone();
        malformed_upload.primitives[3].binding_index = 5;
        malformed_upload.primitives[3].frame_base = 5;
        assert!(matches!(
            materialize_gpu_candidate(&device, &queue, &shared, &malformed_upload, &atlas),
            Err(SceneGpuError::InvalidDrawPlan(
                SceneDrawPlanError::InvalidChromeSchedule
            )),
        ));

        let mut static_upload = upload.clone();
        static_upload.primitives[0].primitive_kind = ATLAS_QUAD_PRIMITIVE_TAG;
        static_upload.primitives[0].material_kind = 1;
        static_upload.primitives[0].resource_kind = 1;
        static_upload.primitives[0].binding_index = 0;
        static_upload.primitives[0].content_base = NONE_U32;
        static_upload.primitives[0].frame_base = NONE_U32;
        static_upload.primitives[0].aux_content_base = NONE_U32;
        static_upload.draws[0].source = PrimitiveSource::StaticAtlas;
        assert!(matches!(
            materialize_gpu_candidate(&device, &queue, &shared, &static_upload, &atlas),
            Err(SceneGpuError::InvalidDrawPlan(
                SceneDrawPlanError::InvalidPipelineClass
            )),
        ));

        let mut reserved_upload = upload;
        reserved_upload.primitives[1].material_kind = 5;
        reserved_upload.primitives[1].blend = 5;
        reserved_upload.primitives[1].binding_index = 4;
        reserved_upload.primitives[1].frame_base = 4;
        assert!(matches!(
            materialize_gpu_candidate(&device, &queue, &shared, &reserved_upload, &atlas),
            Err(SceneGpuError::InvalidDrawPlan(
                SceneDrawPlanError::InvalidPipelineClass
            )),
        ));
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }

    #[cfg(target_os = "macos")]
    enum TestPreparedHud<'prepared> {
        Sensitive(&'prepared super::super::hud::SensitivePreparedHudFrame),
        Redacted(&'prepared super::super::hud::CaptureSafePreparedHudFrame),
    }

    #[cfg(target_os = "macos")]
    fn finish_hud_readback(
        gpu: (&wgpu::Device, &wgpu::Queue),
        shared: &SceneGpuShared,
        candidate: &mut GpuSceneCandidate,
        staging_belt: &mut wgpu::util::StagingBelt,
        prepared: TestPreparedHud<'_>,
        clear: wgpu::Color,
    ) -> Vec<u8> {
        const EXTENT: u32 = 360;
        let (device, queue) = gpu;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glorp-hud-hook-test-target"),
            size: wgpu::Extent3d {
                width: EXTENT,
                height: EXTENT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SceneTextureContract::INTERMEDIATE,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("glorp-hud-hook-test-encoder"),
        });
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glorp-hud-hook-test-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        match prepared {
            TestPreparedHud::Sensitive(prepared) => encode_sensitive_hud_hook(
                &mut encoder,
                staging_belt,
                &view,
                shared,
                candidate,
                prepared,
            ),
            TestPreparedHud::Redacted(prepared) => encode_redacted_hud_hook(
                &mut encoder,
                staging_belt,
                &view,
                shared,
                candidate,
                prepared,
            ),
        }
        .expect("valid HUD hook encode");

        let bytes_per_row = (EXTENT * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glorp-hud-hook-test-readback"),
            size: u64::from(bytes_per_row) * u64::from(EXTENT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(EXTENT),
                },
            },
            wgpu::Extent3d {
                width: EXTENT,
                height: EXTENT,
                depth_or_array_layers: 1,
            },
        );
        staging_belt.finish();
        let submission = queue.submit([encoder.finish()]);
        staging_belt.recall();
        readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .expect("HUD hook readback poll");
        let mapped = readback.slice(..).get_mapped_range().expect("map HUD hook");
        let mut pixels = Vec::with_capacity((EXTENT * EXTENT * 4) as usize);
        for row in mapped.chunks_exact(bytes_per_row as usize) {
            pixels.extend_from_slice(&row[..(EXTENT * 4) as usize]);
        }
        drop(mapped);
        readback.unmap();
        pixels
    }

    #[cfg(target_os = "macos")]
    fn hud_geometry(
        generation: crate::presentation::companion_scene::ResourceGeneration,
    ) -> super::super::hud::HudPreparationGeometry {
        super::super::hud::HudPreparationGeometry {
            gap: crate::round::hud::StatGap {
                center_x: 180.0,
                baseline_y: 180.0,
                max_width: 300.0,
            },
            aperture_radius: 180.0,
            view_width: 360.0,
            view_height: 360.0,
            hud_font_size: 32.0,
            resource_generation: generation,
        }
    }

    #[cfg(target_os = "macos")]
    fn hud_pixel_y_up(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
        const EXTENT: u32 = 360;
        let row = EXTENT - 1 - y;
        let offset = ((row * EXTENT + x) * 4) as usize;
        pixels[offset..offset + 4].try_into().unwrap()
    }

    #[cfg(target_os = "macos")]
    fn expected_hud_bgra(straight_srgb: [f32; 3], coverage: f32, clear: wgpu::Color) -> [u8; 4] {
        let clear_linear = [clear.r as f32, clear.g as f32, clear.b as f32];
        let output_linear: [f32; 3] = std::array::from_fn(|channel| {
            scene_srgb_to_linear(straight_srgb[channel]) * coverage
                + clear_linear[channel] * (1.0 - coverage)
        });
        let encoded = output_linear
            .map(|channel| (scene_linear_to_srgb(channel).clamp(0.0, 1.0) * 255.0).round() as u8);
        let alpha = (coverage + clear.a as f32 * (1.0 - coverage)).clamp(0.0, 1.0);
        [
            encoded[2],
            encoded[1],
            encoded[0],
            (alpha * 255.0).round() as u8,
        ]
    }

    #[cfg(target_os = "macos")]
    fn assert_bgra_close(actual: [u8; 4], expected: [u8; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                actual.abs_diff(expected) <= 2,
                "actual={actual} expected={expected}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_hud_hooks_reuse_caller_belt_render_redaction_and_keep_zero_slots_blank() {
        let (device, queue) = native_device();
        let cpu = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let mut candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        assert_eq!(
            candidate.hud.buffer_contract_for_test(),
            [(
                super::super::hud::HUD_GPU_BUFFER_BYTES,
                super::super::hud::HudGpuBufferUsages::RECORDS,
            ); 2]
        );

        let generation = upload.generation_key.resources;
        let redacted_a = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(generation),
            )
            .unwrap();
        let redacted_b = candidate
            .hud
            .prepared_atlas()
            .prepare_redacted_capture(
                &super::super::hud::SealedHudFrame::redacted_capture().unwrap(),
                hud_geometry(generation),
            )
            .unwrap();
        let live_text_a = crate::round::hud::companion_hud_text(12.0, Some(0.1), 34.0);
        let live_text_b =
            crate::round::hud::companion_hud_text(98_700_000.0, Some(7.0), 6_500_000.0);
        let live_a = candidate
            .hud
            .prepared_atlas()
            .prepare_sensitive(
                &super::super::hud::SealedHudFrame::from_live(&live_text_a).unwrap(),
                hud_geometry(generation),
            )
            .unwrap();
        let live_b = candidate
            .hud
            .prepared_atlas()
            .prepare_sensitive(
                &super::super::hud::SealedHudFrame::from_live(&live_text_b).unwrap(),
                hud_geometry(generation),
            )
            .unwrap();
        let zero = super::super::hud::CaptureSafePreparedHudFrame::zeroed_for_test(generation);
        let shader_contract = candidate
            .hud
            .prepared_atlas()
            .shader_contract_fixture_for_test();
        let mut belt =
            wgpu::util::StagingBelt::new(device.clone(), super::super::hud::HUD_GPU_BUFFER_BYTES);
        let mut mismatched = candidate
            .hud
            .prepared_atlas()
            .prepare_sensitive(
                &super::super::hud::SealedHudFrame::from_live(&live_text_a).unwrap(),
                hud_geometry(generation),
            )
            .unwrap();
        mismatched.set_resource_generation_for_test(
            crate::presentation::companion_scene::ResourceGeneration(generation.0 + 1),
        );
        let before_failed_stage = candidate.hud.staging_facts_for_test();
        let mut failed_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("glorp-rejected-hud-stage-test"),
        });
        let failed_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glorp-rejected-hud-stage-target"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SceneTextureContract::INTERMEDIATE,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let failed_view = failed_target.create_view(&wgpu::TextureViewDescriptor::default());
        let error = encode_sensitive_hud_hook(
            &mut failed_encoder,
            &mut belt,
            &failed_view,
            &shared,
            &mut candidate,
            &mismatched,
        )
        .unwrap_err();
        assert_eq!(
            format!("{error:?} {error}"),
            "HudGpuStagingError::ResourceGenerationMismatch companion HUD GPU generation mismatch"
        );
        assert_eq!(candidate.hud.staging_facts_for_test(), before_failed_stage);
        belt.finish();
        queue.submit([failed_encoder.finish()]);
        belt.recall();

        let redacted_pixels_a = finish_hud_readback(
            (&device, &queue),
            &shared,
            &mut candidate,
            &mut belt,
            TestPreparedHud::Redacted(&redacted_a),
            wgpu::Color::TRANSPARENT,
        );
        let redacted_pixels_b = finish_hud_readback(
            (&device, &queue),
            &shared,
            &mut candidate,
            &mut belt,
            TestPreparedHud::Redacted(&redacted_b),
            wgpu::Color::TRANSPARENT,
        );
        let zero_pixels = finish_hud_readback(
            (&device, &queue),
            &shared,
            &mut candidate,
            &mut belt,
            TestPreparedHud::Redacted(&zero),
            wgpu::Color::TRANSPARENT,
        );
        let live_pixels_a = finish_hud_readback(
            (&device, &queue),
            &shared,
            &mut candidate,
            &mut belt,
            TestPreparedHud::Sensitive(&live_a),
            wgpu::Color::TRANSPARENT,
        );
        let live_pixels_b = finish_hud_readback(
            (&device, &queue),
            &shared,
            &mut candidate,
            &mut belt,
            TestPreparedHud::Sensitive(&live_b),
            wgpu::Color::TRANSPARENT,
        );
        let contract_clear = wgpu::Color { r: 0.10, g: 0.20, b: 0.30, a: 0.50 };
        let shader_pixels = finish_hud_readback(
            (&device, &queue),
            &shared,
            &mut candidate,
            &mut belt,
            TestPreparedHud::Redacted(&shader_contract),
            contract_clear,
        );

        assert_eq!(redacted_pixels_a, redacted_pixels_b);
        assert!(redacted_pixels_a
            .chunks_exact(4)
            .any(|pixel| pixel != [0, 0, 0, 0]));
        assert!(zero_pixels
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 0]));
        assert_ne!(live_pixels_a, live_pixels_b);
        assert_ne!(live_pixels_a, redacted_pixels_a);
        assert_ne!(live_pixels_b, redacted_pixels_a);

        // The fixed shader probe uses an asymmetric 'r' cell (top 25%, bottom
        // 75%), then two uniform 50%-coverage 'e' cells in total/subline roles.
        // These known regions jointly lock entry indexing, the atlas Y flip,
        // role paint selection, fractional coverage, BGRA storage, and
        // premultiplied source-over blending over a nontransparent destination.
        let r_bottom = hud_pixel_y_up(&shader_pixels, 40, 34);
        let r_top = hud_pixel_y_up(&shader_pixels, 40, 46);
        let total_half = hud_pixel_y_up(&shader_pixels, 72, 40);
        let subline_half = hud_pixel_y_up(&shader_pixels, 104, 40);
        assert!(r_bottom[3] > total_half[3] && total_half[3] > r_top[3]);
        assert_bgra_close(
            total_half,
            expected_hud_bgra([0.93, 0.93, 0.97], 128.0 / 255.0, contract_clear),
        );
        assert_bgra_close(
            subline_half,
            expected_hud_bgra([0.62, 0.63, 0.77], 128.0 / 255.0, contract_clear),
        );
        assert_ne!(total_half, subline_half);
        assert_eq!(
            candidate.hud.staging_facts_for_test(),
            super::super::hud::HudStagingFacts {
                sensitive_copies: 2,
                redacted_copies: 4,
                copied_bytes: 6 * super::super::hud::HUD_GPU_BUFFER_BYTES,
            }
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_materialization_rejects_foreign_same_epoch_shared_device_before_allocation() {
        let [(device_a, queue_a), (device_b, _queue_b)] = native_device_pair();
        let candidate = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', candidate.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&candidate, &atlas).unwrap();
        let foreign_shared =
            SceneGpuShared::create(&device_b, upload.generation_key.device).unwrap();

        assert!(matches!(
            materialize_gpu_candidate(&device_a, &queue_a, &foreign_shared, &upload, &atlas,),
            Err(SceneGpuError::ActualDeviceMismatch),
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_candidate_rejects_atlas_and_device_epoch_mismatch_before_allocation() {
        let (device, queue) = native_device();
        let candidate = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', candidate.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&candidate, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        let wrong_atlas = full_hud_atlas_for(
            '^',
            crate::presentation::companion_scene::ResourceGeneration(
                upload.generation_key.resources.0 + 1,
            ),
            None,
            None,
        );
        assert!(matches!(
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &wrong_atlas),
            Err(SceneGpuError::AtlasGenerationMismatch { .. })
        ));

        let mut mismatched_lookup = upload.clone();
        mismatched_lookup.glyph_lookup.resource_generation =
            crate::presentation::companion_scene::ResourceGeneration(
                upload.generation_key.resources.0 + 1,
            );
        assert_eq!(
            validate_gpu_candidate_preflight(&shared, &mismatched_lookup, &atlas),
            Err(SceneGpuError::GlyphLookupGenerationMismatch {
                upload: upload.generation_key.resources,
                lookup: mismatched_lookup.glyph_lookup.resource_generation,
                atlas: atlas.resource_generation,
            }),
        );

        let wrong_shared = SceneGpuShared::create(
            &device,
            crate::presentation::companion_scene::DeviceEpoch(upload.generation_key.device.0 + 1),
        )
        .unwrap();
        assert!(matches!(
            materialize_gpu_candidate(&device, &queue, &wrong_shared, &upload, &atlas),
            Err(SceneGpuError::DeviceEpochMismatch { .. })
        ));

        let mut empty_upload = upload.clone();
        empty_upload.vertex_bytes.clear();
        assert!(matches!(
            materialize_gpu_candidate(&device, &queue, &shared, &empty_upload, &atlas),
            Err(SceneGpuError::InvalidUpload),
        ));

        let mut inconsistent_viewport = upload.clone();
        inconsistent_viewport.logical_viewport_points[0] = 480.0;
        assert_eq!(
            validate_gpu_candidate_preflight(&shared, &inconsistent_viewport, &atlas),
            Err(SceneGpuError::InvalidUpload),
        );

        for invalid_hud_atlas in [
            full_hud_atlas_for('^', upload.generation_key.resources, Some('w'), None),
            full_hud_atlas_for('^', upload.generation_key.resources, None, Some('w')),
        ] {
            let invalid_upload = prepare_scene_upload(&candidate, &invalid_hud_atlas).unwrap();
            let error = match materialize_gpu_candidate(
                &device,
                &queue,
                &shared,
                &invalid_upload,
                &invalid_hud_atlas,
            ) {
                Err(error) => error,
                Ok(_) => panic!("invalid HUD atlas allocated a candidate"),
            };
            assert_eq!(error, SceneGpuError::InvalidHudAtlas);
            assert_eq!(format!("{error:?}"), "InvalidHudAtlas");
        }

        let source = include_str!("render.rs");
        let hud_preflight = source
            .find("let prepared_hud_atlas = super::hud::PreparedHudAtlas::from_scene_atlas")
            .expect("HUD atlas preflight");
        let gpu_allocation_scope = source[hud_preflight..]
            .find("create_in_gpu_error_scopes(device")
            .map(|offset| hud_preflight + offset)
            .expect("candidate allocation scope");
        assert!(hud_preflight < gpu_allocation_scope);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn candidate_preflight_rejects_nonzero_malformed_upload_abi_before_gpu_scopes() {
        let (device, _queue) = native_device();
        let candidate = compile_fixture(&SceneFixture::valid());
        let atlas = two_weight_atlas_for('^', candidate.generation_key.resources);
        let upload = prepare_scene_upload(&candidate, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        assert_eq!(
            validate_gpu_candidate_preflight(&shared, &upload, &atlas),
            Ok(()),
        );
        let assert_invalid = |candidate: PreparedSceneUpload| {
            assert_eq!(
                validate_gpu_candidate_preflight(&shared, &candidate, &atlas),
                Err(SceneGpuError::InvalidUpload),
            );
        };

        let mut malformed = upload.clone();
        malformed.node_bytes.extend_from_slice(&[0; 4]);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed.content_globals_bytes.truncate(140);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed.frame_bytes.extend_from_slice(&[0; 4]);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed
            .scene_content_bytes
            .truncate(PackedMirrorLayout::SCENE_CONTENT_BYTES - 4);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed.vertex_bytes.push(0);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed
            .vertex_bytes
            .truncate(malformed.vertex_bytes.len() - 40);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed.index_bytes.push(0);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed
            .index_bytes
            .truncate(malformed.index_bytes.len() - 4);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed.primitives.push(malformed.primitives[0]);
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed.draws.push(malformed.draws[0].clone());
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        malformed.draws[0].index_range.end = u32::MAX;
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        let phase = [
            &mut malformed.phases.opaque_cutout,
            &mut malformed.phases.world_blended_unsorted,
            &mut malformed.phases.chrome_authored,
        ]
        .into_iter()
        .find(|phase| !phase.is_empty())
        .expect("valid upload has a classified primitive");
        phase[0] = u32::MAX;
        assert_invalid(malformed);

        let mut malformed = upload.clone();
        let phase = [
            &mut malformed.phases.opaque_cutout,
            &mut malformed.phases.world_blended_unsorted,
            &mut malformed.phases.chrome_authored,
        ]
        .into_iter()
        .find(|phase| !phase.is_empty())
        .expect("valid upload has a classified primitive");
        phase.push(phase[0]);
        assert_invalid(malformed);

        let mut malformed = upload;
        malformed.glyph_entries.pop();
        assert!(!malformed.glyph_entries.is_empty());
        assert_invalid(malformed);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn candidate_preflight_rejects_draw_instance_index_glyph_and_phase_mismatches() {
        use crate::presentation::companion_scene::scene::{
            DepthBehavior, InstanceGroupBinding, MaterialKind, PrimitiveKind, PrimitiveSpace,
            ResourceKind, WorldBlend,
        };

        let (device, _queue) = native_device();
        let shared = SceneGpuShared::create(
            &device,
            crate::presentation::companion_scene::DeviceEpoch(1),
        )
        .unwrap();
        let prepare = |fixture: &SceneFixture| {
            let candidate = super::super::compiler::compile_static_fixture_for_render_test(fixture);
            let atlas = two_weight_atlas_for('^', candidate.generation_key.resources);
            let upload = prepare_scene_upload(&candidate, &atlas).unwrap();
            (upload, atlas)
        };
        let assert_invalid =
            |candidate: &PreparedSceneUpload,
             atlas: &super::super::resources::PreparedSceneAtlas| {
                assert_eq!(
                    validate_gpu_candidate_preflight(&shared, candidate, atlas),
                    Err(SceneGpuError::InvalidUpload),
                );
            };

        for (binding, expected_instances) in [
            (InstanceGroupBinding::PetArt(PetArtFilter::Body), 130),
            (InstanceGroupBinding::PetArt(PetArtFilter::Particles), 130),
            (InstanceGroupBinding::RoomGlyphs, 32),
            (InstanceGroupBinding::PropGlyphs(2), 9),
            (
                InstanceGroupBinding::TankCells {
                    slot: 1,
                    layer: crate::presentation::companion_scene::scene::InstanceLayer::Behind,
                },
                8,
            ),
            (
                InstanceGroupBinding::TankCells {
                    slot: 1,
                    layer: crate::presentation::companion_scene::scene::InstanceLayer::Foreground,
                },
                8,
            ),
            (InstanceGroupBinding::Ambient, 64),
            (InstanceGroupBinding::Hud, 0),
        ] {
            let mut fixture = SceneFixture::valid();
            fixture.template.primitives[0].kind = PrimitiveKind::InstanceQuad;
            fixture.template.primitives[0].binding = PrimitiveBinding::Instances(binding);
            let (upload, atlas) = prepare(&fixture);
            validate_gpu_candidate_preflight(&shared, &upload, &atlas).unwrap();
            assert_eq!(upload.draws[0].instance_range, 0..expected_instances);

            let mut malformed = upload.clone();
            malformed.draws[0].instance_range = 0..expected_instances.saturating_add(1);
            assert_invalid(&malformed, &atlas);

            let mut malformed = upload.clone();
            malformed.draws[0].source = PrimitiveSource::Analytic;
            assert_invalid(&malformed, &atlas);

            let mut malformed = upload.clone();
            malformed.primitives[0].content_base =
                malformed.primitives[0].content_base.wrapping_add(1);
            assert_invalid(&malformed, &atlas);

            let mut malformed = upload.clone();
            malformed.draws[0].authored_order = malformed.draws[0].authored_order.wrapping_add(1);
            assert_invalid(&malformed, &atlas);

            let mut malformed = upload;
            malformed.primitives[0].instance_base =
                if malformed.primitives[0].instance_base == NONE_U32 {
                    0
                } else {
                    NONE_U32
                };
            assert_invalid(&malformed, &atlas);
        }

        for kind in [PrimitiveKind::AtlasQuad, PrimitiveKind::AnalyticShape] {
            let mut fixture = SceneFixture::valid();
            fixture.template.primitives[0].kind = kind;
            if kind == PrimitiveKind::AnalyticShape {
                fixture.template.primitives[0].binding =
                    PrimitiveBinding::Analytic(AnalyticSemantic::RoomBackground.id());
                fixture.template.materials[0].kind = MaterialKind::UnlitAnalytic;
                fixture.template.resources[0].kind = ResourceKind::AnalyticGeometry;
            } else {
                fixture.template.primitives[0].binding = PrimitiveBinding::StaticAtlas(
                    crate::presentation::companion_scene::scene::StaticAtlasRecipeId(0),
                );
            }
            let (upload, atlas) = prepare(&fixture);
            validate_gpu_candidate_preflight(&shared, &upload, &atlas).unwrap();

            let mut malformed = upload.clone();
            malformed.draws[0].instance_range = 0..2;
            assert_invalid(&malformed, &atlas);

            let mut malformed = upload.clone();
            malformed.draws[0].source = match kind {
                PrimitiveKind::AtlasQuad => PrimitiveSource::Analytic,
                PrimitiveKind::AnalyticShape => PrimitiveSource::StaticAtlas,
                _ => unreachable!(),
            };
            assert_invalid(&malformed, &atlas);

            let mut malformed = upload;
            malformed.primitives[0].instance_group = 1;
            malformed.primitives[0].instance_base = 0;
            assert_invalid(&malformed, &atlas);
        }

        let mut wall = SceneFixture::valid();
        let mut body = wall.template.primitives[0].clone();
        body.kind = PrimitiveKind::InstanceQuad;
        body.binding =
            PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body));
        wall.template.primitives[0].kind = PrimitiveKind::AnalyticShape;
        wall.template.primitives[0].binding =
            PrimitiveBinding::Analytic(AnalyticSemantic::WallShadow.id());
        wall.template.primitives[0].blend = WorldBlend::Multiply;
        wall.template.primitives[0].depth = DepthBehavior::WorldReadOnly;
        wall.template.materials[0].kind = MaterialKind::MultiplyShadow;
        wall.template.resources[0].kind = ResourceKind::AnalyticGeometry;
        wall.template.primitives.push(body);
        let (mut upload, atlas) = prepare(&wall);
        // This compact fixture has one material/resource id. Normalize the body
        // record to the independent glyph material/resource that the production
        // template supplies; every other field remains compiler-authored.
        upload.primitives[1].material_kind = 1;
        upload.primitives[1].resource_kind = 1;
        validate_gpu_candidate_preflight(&shared, &upload, &atlas).unwrap();
        assert_eq!(upload.draws[0].instance_range, 0..130);
        assert_eq!(
            upload.draws[0].source,
            PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask),
        );

        let mut malformed = upload.clone();
        malformed.primitives[0].aux_node_index = NONE_U32;
        assert_invalid(&malformed, &atlas);

        let mut malformed = upload.clone();
        malformed.primitives[0].aux_content_base = NONE_U32;
        assert_invalid(&malformed, &atlas);

        let mutations: [fn(&mut PrimitiveGpuValue); 4] = [
            |primitive| primitive.blend = 3,
            |primitive| primitive.material_kind = 2,
            |primitive| primitive.depth = 1,
            |primitive| primitive.space = 2,
        ];
        for mutate in mutations {
            let mut malformed = upload.clone();
            mutate(&mut malformed.primitives[0]);
            assert_invalid(&malformed, &atlas);
        }

        let mut two = SceneFixture::valid();
        let mut second = two.template.primitives[0].clone();
        second.authored_order = second.authored_order.saturating_add(1);
        two.template.primitives.push(second);
        let (upload, atlas) = prepare(&two);
        validate_gpu_candidate_preflight(&shared, &upload, &atlas).unwrap();

        let mut malformed = upload.clone();
        malformed.draws[1].index_range = 0..6;
        assert_invalid(&malformed, &atlas);

        let mut malformed = upload.clone();
        malformed.index_bytes[24..28].copy_from_slice(&0_u32.to_ne_bytes());
        assert_invalid(&malformed, &atlas);

        let mut malformed = upload.clone();
        let glyph_count = u32::try_from(malformed.glyph_entries.len()).unwrap();
        malformed.scene_content_bytes[4..8].copy_from_slice(&glyph_count.to_ne_bytes());
        assert_invalid(&malformed, &atlas);

        for phase in [0_u8, 1, 2] {
            let mut fixture = SceneFixture::valid();
            match phase {
                0 => {}
                1 => {
                    fixture.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
                    fixture.template.primitives[0].depth = DepthBehavior::WorldReadOnly;
                }
                2 => {
                    fixture.template.materials[0].kind = MaterialKind::ScreenChrome;
                    fixture.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
                    fixture.template.primitives[0].depth = DepthBehavior::ScreenNoDepth;
                    fixture.template.primitives[0].space = PrimitiveSpace::Screen;
                }
                _ => unreachable!(),
            }
            let (upload, atlas) = prepare(&fixture);
            validate_gpu_candidate_preflight(&shared, &upload, &atlas).unwrap();

            let mut wrong_phase = upload.clone();
            wrong_phase.phases.opaque_cutout.clear();
            wrong_phase.phases.world_blended_unsorted.clear();
            wrong_phase.phases.chrome_authored.clear();
            match phase {
                0 => wrong_phase.phases.world_blended_unsorted.push(0),
                1 => wrong_phase.phases.chrome_authored.push(0),
                2 => wrong_phase.phases.opaque_cutout.push(0),
                _ => unreachable!(),
            }
            assert_invalid(&wrong_phase, &atlas);

            let mut incompatible = upload;
            match phase {
                0 => incompatible.primitives[0].space = 2,
                1 => incompatible.primitives[0].depth = 1,
                2 => incompatible.primitives[0].blend = 1,
                _ => unreachable!(),
            }
            assert_invalid(&incompatible, &atlas);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn candidate_preflight_rejects_capacity_node_vertex_and_resource_corruption() {
        let (device, _queue) = native_device();
        let candidate = compile_fixture(&SceneFixture::valid());
        let atlas = two_weight_atlas_for('^', candidate.generation_key.resources);
        let upload = prepare_scene_upload(&candidate, &atlas).unwrap();
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        validate_gpu_candidate_preflight(&shared, &upload, &atlas).unwrap();
        let assert_invalid = |candidate: &PreparedSceneUpload| {
            assert_eq!(
                validate_gpu_candidate_preflight(&shared, candidate, &atlas),
                Err(SceneGpuError::InvalidUpload),
            );
        };

        let mut malformed = upload.clone();
        malformed.primitives = vec![
            malformed.primitives[0];
            crate::presentation::companion_scene::scene::MAX_STATIC_PRIMITIVES
                + 1
        ];
        assert_invalid(&malformed);

        let mut malformed = upload.clone();
        malformed.primitives[0].node_index =
            u32::try_from(super::super::compiler::CpuMirrorShape::NODE_COUNT).unwrap();
        assert_invalid(&malformed);

        let mut malformed = upload.clone();
        malformed.primitives[0].aux_node_index = 0;
        assert_invalid(&malformed);

        let mut malformed = upload.clone();
        malformed.vertex_bytes[32..36].copy_from_slice(&1_u32.to_ne_bytes());
        assert_invalid(&malformed);

        let mut malformed = upload.clone();
        let wrong_material = malformed.primitives[0].material_index.saturating_add(1);
        malformed.vertex_bytes[36..40].copy_from_slice(&wrong_material.to_ne_bytes());
        assert_invalid(&malformed);

        let mut malformed = upload.clone();
        malformed.primitives[0].resource_kind = 3;
        assert_invalid(&malformed);

        let mut malformed = upload;
        malformed.primitives[0].resource_kind = 0;
        assert_invalid(&malformed);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_target_cache_reuses_replaces_and_retains_on_scoped_failure() {
        let (device, queue) = native_device();
        let cpu_candidate = compile_fixture(&canonical_materialization_fixture());
        let atlas = full_hud_atlas_for('^', cpu_candidate.generation_key.resources, None, None);
        let upload = prepare_scene_upload(&cpu_candidate, &atlas).unwrap();
        let device_epoch = upload.generation_key.device;
        let shared = SceneGpuShared::create(&device, device_epoch).unwrap();
        let gpu_candidate =
            materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        let candidate_generation = gpu_candidate.generation_key;
        let candidate_facts = gpu_candidate.facts();
        let key = SceneTargetKey::new(
            device_epoch,
            crate::presentation::companion_scene::SurfaceEpoch(10),
            wgpu::Extent3d {
                width: 260,
                height: 260,
                depth_or_array_layers: 1,
            },
            SceneTextureContract::INTERMEDIATE,
            SceneTextureContract::INTERMEDIATE,
            SceneTextureContract::DEPTH,
            SceneTextureContract::SAMPLE_COUNT,
        )
        .unwrap();
        let mut cache = SceneTargetCache::default();
        assert_eq!(
            cache.ensure(&device, &shared, key).unwrap(),
            SceneTargetUpdate::Created
        );
        assert_eq!(cache.creation_events(), 1);
        let targets = cache.current().unwrap();
        assert_eq!(targets.facts(), SceneTargetFacts::EXPECTED);
        assert_eq!(
            targets.raw_scene_texture.format(),
            SceneTextureContract::INTERMEDIATE
        );
        assert_eq!(targets.raw_scene_texture.size(), key.extent);
        assert_eq!(
            targets.raw_scene_texture.usage(),
            SceneTargetTextureUsages::RAW_SCENE
        );
        assert_eq!(
            targets.intermediate_texture.format(),
            SceneTextureContract::INTERMEDIATE
        );
        assert_eq!(targets.intermediate_texture.size(), key.extent);
        assert_eq!(
            targets.intermediate_texture.usage(),
            SceneTargetTextureUsages::INTERMEDIATE
        );
        assert_eq!(targets.depth_texture.format(), SceneTextureContract::DEPTH);
        assert_eq!(targets.depth_texture.size(), key.extent);
        assert_eq!(
            targets.depth_texture.usage(),
            SceneTargetTextureUsages::DEPTH
        );
        assert_eq!(
            cache.ensure(&device, &shared, key).unwrap(),
            SceneTargetUpdate::Reused
        );
        assert_eq!(cache.creation_events(), 1);

        let replacement_key = SceneTargetKey::new(
            key.device_epoch,
            crate::presentation::companion_scene::SurfaceEpoch(key.surface_epoch.0 + 1),
            wgpu::Extent3d {
                width: 360,
                height: 360,
                depth_or_array_layers: 1,
            },
            key.surface_format,
            key.intermediate_format,
            key.depth_format,
            key.sample_count,
        )
        .unwrap();
        assert_eq!(
            cache.ensure(&device, &shared, replacement_key).unwrap(),
            SceneTargetUpdate::Created,
        );
        assert_eq!(cache.creation_events(), 2);
        assert_eq!(cache.current().unwrap().key, replacement_key);
        assert_eq!(gpu_candidate.generation_key, candidate_generation);
        assert_eq!(gpu_candidate.facts(), candidate_facts);
        assert_eq!(shared.facts(), SceneGpuSharedFacts::EXPECTED);

        let failed_key = SceneTargetKey::new(
            replacement_key.device_epoch,
            crate::presentation::companion_scene::SurfaceEpoch(replacement_key.surface_epoch.0 + 1),
            wgpu::Extent3d {
                width: 480,
                height: 480,
                depth_or_array_layers: 1,
            },
            replacement_key.surface_format,
            replacement_key.intermediate_format,
            replacement_key.depth_format,
            replacement_key.sample_count,
        )
        .unwrap();
        assert_eq!(
            cache.ensure_with_test_fault(
                &device,
                &shared,
                failed_key,
                SceneTargetTestFault::ValidationAfterAllocation,
            ),
            Err(SceneGpuError::Gpu(ScopedGpuErrorCategory::Validation)),
        );
        assert_eq!(cache.creation_events(), 2);
        assert_eq!(cache.current().unwrap().key, replacement_key);

        let malformed_key = SceneTargetKey { sample_count: 4, ..failed_key };
        assert_eq!(
            cache.ensure(&device, &shared, malformed_key),
            Err(SceneGpuError::InvalidTargetKey(
                SceneTargetKeyError::SampleCount,
            )),
        );
        assert_eq!(cache.creation_events(), 2);
        assert_eq!(cache.current().unwrap().key, replacement_key);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}
