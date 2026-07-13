//! Pure contracts and owned CPU preparation for the retained scene renderer.
#![allow(dead_code)] // This checkpoint validates contracts before live GPU materialization.

use super::buffers::ByteSpan;
use super::compiler::{
    ContentUploadValue, CpuSceneCandidate, FrameUploadSources, PrimitiveUploadSource,
};
use super::resources::{GlyphAtlasResolveError, GlyphEntryKind, GlyphKey, PreparedSceneAtlas};
use bytemuck::{Pod, Zeroable};
use std::ops::Range;
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

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PreparedSceneUpload {
    pub(super) generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    pub(super) source_revisions: crate::presentation::companion_scene::AppliedRevisions,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnsupportedSceneFeature {
    ShallowCardPrimitive,
    LitShallowCardMaterial,
    ShallowCardGeometryResource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SceneUploadError {
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

impl SceneContentGpuValue {
    pub(super) fn translate(
        family: ContentMirrorFamily,
        value: ContentUploadValue,
        atlas: &PreparedSceneAtlas,
    ) -> Result<Self, SceneUploadError> {
        if matches!(
            family,
            ContentMirrorFamily::Globals | ContentMirrorFamily::Analytics
        ) {
            return Err(SceneUploadError::NonGlyphContentFamily);
        }
        let glyph_entry_index = if value.glyph_scalar == NONE_U32 {
            NONE_U32
        } else {
            let scalar =
                char::from_u32(value.glyph_scalar).ok_or(SceneUploadError::InvalidGlyphScalar {
                    family,
                    slot: value.slot,
                    scalar: value.glyph_scalar,
                })?;
            let key = GlyphKey::new(
                scalar.to_string(),
                SceneGlyphWeightPolicy::content_is_bold(family, value.flags, value.signed_data),
            );
            match atlas.resolve_key(&key) {
                Ok(entry) => entry.id,
                Err(GlyphAtlasResolveError::MissingKey(_)) => {
                    return Err(SceneUploadError::MissingGlyphKey {
                        family,
                        slot: value.slot,
                        key,
                    });
                }
                Err(
                    GlyphAtlasResolveError::MissingSingleScalar(_)
                    | GlyphAtlasResolveError::AmbiguousSingleScalar { .. },
                ) => unreachable!("full GlyphKey lookup cannot return scalar-diagnostic errors"),
            }
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
    // Reject deferred card work before allocating or copying upload-owned data.
    for primitive_index in 0..candidate.primitive_count() {
        let source = candidate
            .primitive_upload_source(primitive_index)
            .ok_or(SceneUploadError::InvalidPrimitiveReference { primitive_index })?;
        preflight_primitive(primitive_index, source)?;
    }
    let glyph_entries = prepare_glyph_entries(atlas)?;

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

    let (content_globals_bytes, scene_content_bytes) = prepare_content_buffers(candidate, atlas)?;
    let sources = candidate.frame_upload_sources();
    let node_bytes = exact_owned(sources.nodes, PackedMirrorLayout::NODE_BYTES)?;
    let frame_bytes = prepare_frame_bytes(sources)?;
    let phase_sources = candidate.phase_upload_sources();
    Ok(PreparedSceneUpload {
        generation_key: candidate.generation_key,
        source_revisions: candidate.source_revisions,
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
    atlas: &PreparedSceneAtlas,
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
            .map(|value| SceneContentGpuValue::translate(family, value, atlas))
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
        pipelines: 10,
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
        textures: 2,
        texture_views: 2,
        bind_groups: 1,
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
    AtlasGenerationMismatch {
        upload: crate::presentation::companion_scene::ResourceGeneration,
        atlas: crate::presentation::companion_scene::ResourceGeneration,
    },
    DeviceEpochMismatch {
        shared: crate::presentation::companion_scene::DeviceEpoch,
        requested: crate::presentation::companion_scene::DeviceEpoch,
    },
    InvalidAtlas,
    InvalidHudAtlas,
    InvalidUpload,
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
fn create_in_gpu_error_scopes<T>(
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
    pub(super) final_surface: wgpu::RenderPipeline,
}

/// Device-epoch shared handles. The render owner supplies `Device`/`Queue` to
/// operations; neither is retained here.
pub(super) struct SceneGpuShared {
    pub(super) device_epoch: crate::presentation::companion_scene::DeviceEpoch,
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
        final_surface,
    }
}

pub(super) struct GpuSceneCandidate {
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
    pub(super) static_checksum: u64,
    pub(super) draws: Vec<SceneDrawRecord>,
    pub(super) phases: ScenePhaseTable,
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
    validate_gpu_candidate_preflight(shared, upload, atlas)?;
    let prepared_hud_atlas = super::hud::PreparedHudAtlas::from_scene_atlas(atlas)
        .map_err(|_| SceneGpuError::InvalidHudAtlas)?;

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
            static_checksum: upload.static_checksum,
            draws: upload.draws.clone(),
            phases: upload.phases.clone(),
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
    pub(super) intermediate_texture: wgpu::Texture,
    pub(super) intermediate_view: wgpu::TextureView,
    pub(super) depth_texture: wgpu::Texture,
    pub(super) depth_view: wgpu::TextureView,
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
            let intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("glorp-scene-intermediate"),
                size: key.extent,
                mip_level_count: 1,
                sample_count: key.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: key.intermediate_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
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
                intermediate_texture,
                intermediate_view,
                depth_texture,
                depth_view,
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
        AnalyticGeometry, AnalyticSemantic, AuthoredGlyph, ContentDelta, FrameDelta,
        InstanceGroupBinding, InstanceLayer, MaterialKind, PetArtFilter, PetPaletteRole,
        PrimitiveBinding, PrimitiveKind, ResourceKind, SceneFixture,
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
        let production = include_str!("render.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
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
        for aperture_contract in [
            "let aperture = frame_buffer.analytics[0u];",
            "let aperture_content = scene_content_buffer.analytics[0u];",
            "valid_analytic_role(0u, aperture, aperture_content)",
            "input.point_position - aperture.payload[0].xy",
            "aperture.payload[0].z",
            "aperture.payload[0].w",
        ] {
            assert!(
                dim.contains(aperture_contract),
                "dim aperture: {aperture_contract}"
            );
        }

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
        let regular = SceneContentGpuValue::translate(
            ContentMirrorFamily::Pet,
            super::super::compiler::ContentUploadValue::fixture(u32::from('^'), 1),
            &atlas,
        )
        .unwrap();
        let bold = SceneContentGpuValue::translate(
            ContentMirrorFamily::Pet,
            super::super::compiler::ContentUploadValue::fixture(u32::from('^'), 3),
            &atlas,
        )
        .unwrap();
        assert_ne!(regular.glyph_entry_index, bold.glyph_entry_index);

        let none = SceneContentGpuValue::translate(
            ContentMirrorFamily::Ambient,
            super::super::compiler::ContentUploadValue::fixture(u32::MAX, 0),
            &atlas,
        )
        .unwrap();
        assert_eq!(none.glyph_entry_index, u32::MAX);
        assert!(matches!(
            SceneContentGpuValue::translate(
                ContentMirrorFamily::Ambient,
                super::super::compiler::ContentUploadValue::fixture(0x11_0000, 0),
                &atlas,
            ),
            Err(SceneUploadError::InvalidGlyphScalar { scalar: 0x11_0000, .. })
        ));
        assert!(matches!(
            SceneContentGpuValue::translate(
                ContentMirrorFamily::Ambient,
                super::super::compiler::ContentUploadValue::fixture(u32::from('x'), 0),
                &atlas,
            ),
            Err(SceneUploadError::MissingGlyphKey { .. })
        ));
    }

    #[test]
    fn tank_atlas_weight_uses_authored_packed_bit() {
        let atlas = two_weight_atlas('^');
        let mut regular = super::super::compiler::ContentUploadValue::fixture(u32::from('^'), 0);
        regular.kind = 3;
        let packed_color = 126 | (238 << 8) | (255 << 16);
        regular.signed_data = [packed_color, 0];
        let mut bold = regular;
        bold.signed_data[1] = 1;

        let regular =
            SceneContentGpuValue::translate(ContentMirrorFamily::TankGlyphs, regular, &atlas)
                .unwrap();
        let bold =
            SceneContentGpuValue::translate(ContentMirrorFamily::TankGlyphs, bold, &atlas).unwrap();
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
        let atlas = two_weight_atlas('^');

        let upload = prepare_scene_upload(&candidate, &atlas).unwrap();

        assert_eq!(upload.generation_key, candidate.generation_key);
        assert_eq!(upload.source_revisions, candidate.source_revisions);
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
        let upload = prepare_scene_upload(&candidate, &two_weight_atlas('^')).unwrap();

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
            let upload = prepare_scene_upload(&candidate, &two_weight_atlas('^')).unwrap();
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
        let upload = prepare_scene_upload(
            &super::super::compiler::compile_static_fixture_for_render_test(&analytic),
            &two_weight_atlas('^'),
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
        let upload = prepare_scene_upload(
            &super::super::compiler::compile_static_fixture_for_render_test(&wall),
            &two_weight_atlas('^'),
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
            let upload = prepare_scene_upload(&candidate, &two_weight_atlas('^')).unwrap();
            assert_eq!(upload.primitives[0].instance_base, expected_base);
            assert_eq!(upload.draws[0].index_range, 0..6);
            assert_eq!(upload.draws[0].instance_range, expected_instances);
            assert_eq!(upload.draws[0].source, expected_source);
        }

        let static_upload = prepare_scene_upload(
            &compile_fixture(&SceneFixture::valid()),
            &two_weight_atlas('^'),
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
        let analytic_upload =
            prepare_scene_upload(&analytic_candidate, &two_weight_atlas('^')).unwrap();
        assert_eq!(analytic_upload.primitives[0].instance_base, u32::MAX);
        assert_eq!(analytic_upload.draws[0].source, PrimitiveSource::Analytic);
        assert_eq!(analytic_upload.draws[0].instance_range, 0..1);
    }

    #[test]
    fn role_only_delta_changes_upload_entry_without_resource_generation_or_mirror_rewrite() {
        let fixture = SceneFixture::valid();
        let mut candidate = compile_fixture(&fixture);
        let atlas = two_weight_atlas('^');
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
                prepare_scene_upload(&candidate, &two_weight_atlas('^')),
                Err(SceneUploadError::UnsupportedPrimitive { primitive_index: 0, feature: kind })
            );
        }

        prepare_scene_upload(
            &compile_fixture(&SceneFixture::valid()),
            &two_weight_atlas('^'),
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
            "textureLoad(intermediate_texture",
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
    fn analytic_fragment_discards_zero_coverage_before_depth_can_be_written() {
        let aperture = SCENE_SHADER_SOURCE
            .split("fn fs_room_aperture(")
            .nth(1)
            .expect("room analytic role exists")
            .split("fn fs_floor_projection(")
            .next()
            .expect("room role has a bounded body");
        let coverage = aperture.find("let coverage = 1.0 - smoothstep(").unwrap();
        let output = aperture.find("return analytic_premultiply(").unwrap();
        assert!(coverage < output, "coverage contributes before role output");

        let entry = SCENE_SHADER_SOURCE.split("fn fs_analytic(").nth(1).unwrap();
        assert!(entry.contains("if (output.a <= 0.0) {\n        discard;\n    }"));
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
        assert_eq!(SceneGpuSharedFacts::EXPECTED.pipelines, 10);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.buffers, 10);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.textures, 2);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.texture_views, 2);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.bind_groups, 4);
        assert_eq!(GpuSceneCandidateFacts::EXPECTED.static_uploads, 10);
        assert_eq!(SceneTargetFacts::EXPECTED.textures, 2);
        assert_eq!(SceneTargetFacts::EXPECTED.texture_views, 2);
        assert_eq!(SceneTargetFacts::EXPECTED.bind_groups, 1);
        assert_eq!(
            SceneGpuSharedFacts::EXPECTED.persistent_owned_handles()
                + GpuSceneCandidateFacts::EXPECTED.persistent_owned_handles()
                + SceneTargetFacts::EXPECTED.persistent_owned_handles(),
            38,
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
        assert_eq!(SceneBufferUsages::VERTEX, wgpu::BufferUsages::VERTEX);
        assert_eq!(SceneBufferUsages::INDEX, wgpu::BufferUsages::INDEX);
        assert_eq!(SceneBufferUsages::PRIMITIVE, wgpu::BufferUsages::STORAGE);
        assert_eq!(SceneBufferUsages::GLYPH_ENTRY, wgpu::BufferUsages::STORAGE);
        for usage in [
            SceneBufferUsages::NODE,
            SceneBufferUsages::CONTENT_GLOBALS,
            SceneBufferUsages::FRAME,
            SceneBufferUsages::SCENE_CONTENT,
        ] {
            assert_eq!(
                usage,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );
        }
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
    #[test]
    fn native_shared_and_candidate_materialization_validate_without_a_surface() {
        let (device, queue) = native_device();
        let candidate = compile_fixture(&SceneFixture::valid());
        let upload = prepare_scene_upload(
            &candidate,
            &full_hud_atlas_for('^', candidate.generation_key.resources, None, None),
        )
        .unwrap();
        let atlas = full_hud_atlas_for('^', upload.generation_key.resources, None, None);
        let shared = SceneGpuShared::create(&device, upload.generation_key.device).unwrap();
        assert_eq!(shared.facts(), SceneGpuSharedFacts::EXPECTED);

        let gpu = materialize_gpu_candidate(&device, &queue, &shared, &upload, &atlas).unwrap();
        assert_eq!(gpu.facts(), GpuSceneCandidateFacts::EXPECTED);
        assert_eq!(gpu.generation_key, upload.generation_key);
        assert_eq!(gpu.source_revisions, upload.source_revisions);
        assert_eq!(gpu.static_checksum, upload.static_checksum);
        assert_eq!(gpu.draws, upload.draws);
        assert_eq!(gpu.phases, upload.phases);
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
        let cpu = compile_fixture(&SceneFixture::valid());
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
    fn native_candidate_rejects_atlas_and_device_epoch_mismatch_before_allocation() {
        let (device, queue) = native_device();
        let candidate = compile_fixture(&SceneFixture::valid());
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
        let cpu_candidate = compile_fixture(&SceneFixture::valid());
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
        assert_eq!(cache.current().unwrap().facts(), SceneTargetFacts::EXPECTED);
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
