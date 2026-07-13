//! Pure contracts and owned CPU preparation for the retained scene renderer.
#![allow(dead_code)] // This checkpoint validates contracts before live GPU materialization.

use super::buffers::ByteSpan;
use super::compiler::{
    ContentUploadValue, CpuSceneCandidate, FrameUploadSources, PrimitiveUploadSource,
};
use super::resources::{GlyphAtlasResolveError, GlyphEntryKind, GlyphKey, PreparedSceneAtlas};
use bytemuck::{Pod, Zeroable};
use std::ops::Range;

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
    pub(super) resource_index: u32,
    pub(super) primitive_kind: u32,
    pub(super) material_kind: u32,
    pub(super) resource_kind: u32,
    pub(super) blend: u32,
    pub(super) depth: u32,
    pub(super) space: u32,
    pub(super) instance_group: u32,
    pub(super) instance_base: u32,
    _padding: u32,
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
    pub(super) ink_origin_size: [f32; 4],
    /// Advance, line height, and top-down baseline in raster pixels.
    pub(super) metrics: [f32; 3],
    pub(super) flags: u32,
    /// Integer `[origin_x, origin_y, width, height]` of the allocated cell.
    pub(super) allocated_cell: [u32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstanceSource {
    PetBody,
    PetParticles,
    PropGlyphs {
        slot: u32,
    },
    TankCells {
        slot: u32,
        layer: crate::presentation::companion_scene::scene::InstanceLayer,
    },
    Ambient,
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
}

pub(super) struct SceneGlyphWeightPolicy;

impl SceneGlyphWeightPolicy {
    pub(super) const fn pet_is_bold(palette_role_tag: u32) -> bool {
        palette_role_tag == PET_EYE_PALETTE_ROLE_TAG
    }

    pub(super) const fn tank_layer_is_bold(
        _layer: crate::presentation::companion_scene::scene::InstanceLayer,
    ) -> bool {
        true
    }

    pub(super) const fn content_is_bold(family: ContentMirrorFamily, flags: u32) -> bool {
        match family {
            ContentMirrorFamily::Pet => Self::pet_is_bold(flags),
            ContentMirrorFamily::TankGlyphs => true,
            ContentMirrorFamily::Globals
            | ContentMirrorFamily::PropGlyphs
            | ContentMirrorFamily::Ambient
            | ContentMirrorFamily::Hud => false,
        }
    }
}

impl SceneContentGpuValue {
    pub(super) fn translate(
        family: ContentMirrorFamily,
        value: ContentUploadValue,
        atlas: &PreparedSceneAtlas,
    ) -> Result<Self, SceneUploadError> {
        if family == ContentMirrorFamily::Globals {
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
                SceneGlyphWeightPolicy::content_is_bold(family, value.flags),
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

    let mut primitives = Vec::with_capacity(candidate.primitive_count());
    let mut draws = Vec::with_capacity(candidate.primitive_count());
    for primitive_index in 0..candidate.primitive_count() {
        let source = candidate
            .primitive_upload_source(primitive_index)
            .ok_or(SceneUploadError::InvalidPrimitiveReference { primitive_index })?;
        primitives.push(PrimitiveGpuValue {
            node_index: source.node_index,
            material_index: source.material_index,
            resource_index: source.resource_index,
            primitive_kind: source.primitive_kind,
            material_kind: source.material_kind,
            resource_kind: source.resource_kind,
            blend: source.blend,
            depth: source.depth,
            space: source.space,
            instance_group: source.instance_group,
            instance_base: source.instance_base,
            _padding: 0,
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
        glyph_entries: prepare_glyph_entries(atlas),
    })
}

fn prepare_draw_record(source: PrimitiveUploadSource) -> Option<SceneDrawRecord> {
    let index_range = source
        .first_index
        .checked_add(source.index_count)
        .map(|end| source.first_index..end)?;
    let (primitive_source, instance_count) = match source.primitive_kind {
        ATLAS_QUAD_PRIMITIVE_TAG => (PrimitiveSource::StaticAtlas, 1),
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
    })
}

fn instance_source_and_count(source: PrimitiveUploadSource) -> Option<(InstanceSource, u32)> {
    use crate::presentation::companion_scene::scene::{
        InstanceLayer, MAX_AMBIENT_INSTANCES, MAX_HUD_GLYPH_SLOTS, MAX_PET_ART_SLOTS,
        MAX_PROP_GLYPHS_PER_SLOT, MAX_TANK_GLYPHS_PER_SLOT,
    };

    let count = |value: usize| u32::try_from(value).expect("scene capacity fits in u32");
    match source.instance_group {
        1 => Some((InstanceSource::PetBody, count(MAX_PET_ART_SLOTS))),
        // Task 9.5 owns the filtered particle stream; until then this source is
        // typed but deliberately has no drawable instance or content range.
        2 if source.instance_base == NONE_U32 => Some((InstanceSource::PetParticles, 0)),
        3 => {
            let width = count(MAX_PROP_GLYPHS_PER_SLOT);
            source.instance_base.is_multiple_of(width).then_some((
                InstanceSource::PropGlyphs { slot: source.instance_base / width },
                width,
            ))
        }
        5 | 6 => {
            let width = count(MAX_TANK_GLYPHS_PER_SLOT);
            let layer = if source.instance_group == 5 {
                InstanceLayer::Behind
            } else {
                InstanceLayer::Foreground
            };
            source.instance_base.is_multiple_of(width).then_some((
                InstanceSource::TankCells {
                    slot: source.instance_base / width,
                    layer,
                },
                width,
            ))
        }
        7 if source.instance_base == 0 => {
            Some((InstanceSource::Ambient, count(MAX_AMBIENT_INSTANCES)))
        }
        8 if source.instance_base == 0 => Some((InstanceSource::Hud, count(MAX_HUD_GLYPH_SLOTS))),
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
        (ContentMirrorFamily::Hud, sources.hud),
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
    Ok((globals, scene_content))
}

fn prepare_frame_bytes(sources: FrameUploadSources<'_>) -> Result<Vec<u8>, SceneUploadError> {
    let mut packed = vec![0; PackedMirrorLayout::FRAME_BYTES];
    for (family, bytes) in [
        (FrameMirrorFamily::Globals, sources.globals),
        (FrameMirrorFamily::Props, sources.props),
        (FrameMirrorFamily::TankCells, sources.tank_cells),
        (FrameMirrorFamily::Ambient, sources.ambient),
        (FrameMirrorFamily::Hud, sources.hud),
        (FrameMirrorFamily::Lights, sources.lights),
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

pub(super) fn prepare_glyph_entries(atlas: &PreparedSceneAtlas) -> Vec<GlyphAtlasGpuEntry> {
    atlas
        .entries
        .iter()
        .map(|source| {
            let entry = source.entry;
            GlyphAtlasGpuEntry {
                visible_uv: entry.visible_uv.unwrap_or([0.0; 4]),
                ink_origin_size: [
                    entry.ink_origin[0],
                    entry.ink_origin[1],
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
            }
        })
        .collect()
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
    Hud,
}

impl ContentMirrorFamily {
    pub(super) const ALL: [Self; 6] = [
        Self::Globals,
        Self::Pet,
        Self::PropGlyphs,
        Self::TankGlyphs,
        Self::Ambient,
        Self::Hud,
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
    Hud,
    Lights,
}

impl FrameMirrorFamily {
    pub(super) const ALL: [Self; 6] = [
        Self::Globals,
        Self::Props,
        Self::TankCells,
        Self::Ambient,
        Self::Hud,
        Self::Lights,
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
    pub(super) const SCENE_CONTENT_BYTES: usize = scene_content_end(ContentMirrorFamily::Hud);
    pub(super) const FRAME_BYTES: usize = frame_end(FrameMirrorFamily::Lights);

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
            ContentMirrorFamily::Hud => Some(scene_content_end(ContentMirrorFamily::Ambient)),
        }
    }

    pub(super) const fn frame_offset(family: FrameMirrorFamily) -> usize {
        match family {
            FrameMirrorFamily::Globals => 0,
            FrameMirrorFamily::Props => frame_end(FrameMirrorFamily::Globals),
            FrameMirrorFamily::TankCells => frame_end(FrameMirrorFamily::Props),
            FrameMirrorFamily::Ambient => frame_end(FrameMirrorFamily::TankCells),
            FrameMirrorFamily::Hud => frame_end(FrameMirrorFamily::Ambient),
            FrameMirrorFamily::Lights => frame_end(FrameMirrorFamily::Hud),
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
        ContentDelta, FrameDelta, InstanceLayer, MaterialKind, PetPaletteRole, PrimitiveKind,
        ResourceKind, SceneFixture,
    };

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
        assert_eq!(PackedMirrorLayout::CONTENT_GLOBALS_BYTES, 144);
        assert_eq!(PackedMirrorLayout::SCENE_CONTENT_BYTES, 10_368);
        assert_eq!(PackedMirrorLayout::FRAME_BYTES, 5_760);
        assert_eq!(PackedMirrorLayout::content_globals_offset(), 0);
        assert_eq!(
            [
                ContentMirrorFamily::Pet,
                ContentMirrorFamily::PropGlyphs,
                ContentMirrorFamily::TankGlyphs,
                ContentMirrorFamily::Ambient,
                ContentMirrorFamily::Hud,
            ]
            .map(|family| PackedMirrorLayout::scene_content_offset(family).unwrap()),
            [0, 4_160, 7_040, 7_552, 9_600]
        );
        assert_eq!(
            PackedMirrorLayout::scene_content_offset(ContentMirrorFamily::Globals),
            None
        );
        assert_eq!(
            FrameMirrorFamily::ALL.map(PackedMirrorLayout::frame_offset),
            [0, 192, 672, 1_440, 4_512, 5_664]
        );
        assert_eq!(
            PackedMirrorLayout::translate_content_globals_span(ByteSpan { offset: 0, len: 144 }),
            Ok(ByteSpan { offset: 0, len: 144 })
        );
        for family in [
            ContentMirrorFamily::Pet,
            ContentMirrorFamily::PropGlyphs,
            ContentMirrorFamily::TankGlyphs,
            ContentMirrorFamily::Ambient,
            ContentMirrorFamily::Hud,
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
                ByteSpan { offset: 0, len: 144 },
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

    fn two_weight_atlas(scalar: char) -> super::super::resources::PreparedSceneAtlas {
        use super::super::resources::{
            AtlasCell, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphEntryKind, GlyphKey,
            PreparedSceneAtlas,
        };
        let entries = [false, true]
            .into_iter()
            .enumerate()
            .map(|(column, bold)| {
                (
                    GlyphKey::new(scalar.to_string(), bold),
                    GlyphAtlasEntry::synthetic_visible(
                        GlyphEntryKind::Mask,
                        AtlasCell {
                            origin: [column as u32, 0],
                            extent: [1, 1],
                        },
                    ),
                )
            })
            .collect();
        PreparedSceneAtlas::from_compiled(&CompiledGlyphAtlas {
            width: 2,
            height: 1,
            rgba: vec![0; 8],
            entries,
        })
        .unwrap()
    }

    fn compile_fixture(fixture: &SceneFixture) -> super::super::compiler::CpuSceneCandidate {
        super::super::compiler::compile_fixture_for_render_test(fixture)
    }

    #[test]
    fn gpu_upload_records_have_locked_sizes_alignments_and_offsets() {
        assert_eq!(std::mem::size_of::<PrimitiveGpuValue>(), 48);
        assert_eq!(std::mem::align_of::<PrimitiveGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, node_index), 0);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, material_index), 4);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, resource_index), 8);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, primitive_kind), 12);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, material_kind), 16);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, resource_kind), 20);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, blend), 24);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, depth), 28);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, space), 32);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, instance_group), 36);
        assert_eq!(std::mem::offset_of!(PrimitiveGpuValue, instance_base), 40);

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
        assert!(SceneGlyphWeightPolicy::tank_layer_is_bold(
            InstanceLayer::Behind
        ));
        assert!(SceneGlyphWeightPolicy::tank_layer_is_bold(
            InstanceLayer::Foreground
        ));
        for family in [
            ContentMirrorFamily::PropGlyphs,
            ContentMirrorFamily::Ambient,
            ContentMirrorFamily::Hud,
        ] {
            assert!(!SceneGlyphWeightPolicy::content_is_bold(family, 0));
        }
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
            ContentMirrorFamily::Hud,
            super::super::compiler::ContentUploadValue::fixture(u32::MAX, 0),
            &atlas,
        )
        .unwrap();
        assert_eq!(none.glyph_entry_index, u32::MAX);
        assert!(matches!(
            SceneContentGpuValue::translate(
                ContentMirrorFamily::Hud,
                super::super::compiler::ContentUploadValue::fixture(0x11_0000, 0),
                &atlas,
            ),
            Err(SceneUploadError::InvalidGlyphScalar { scalar: 0x11_0000, .. })
        ));
        assert!(matches!(
            SceneContentGpuValue::translate(
                ContentMirrorFamily::Hud,
                super::super::compiler::ContentUploadValue::fixture(u32::from('x'), 0),
                &atlas,
            ),
            Err(SceneUploadError::MissingGlyphKey { .. })
        ));
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
        assert_eq!(candidate_buffer_lengths[3], 144);
        assert_eq!(candidate_buffer_lengths[4], 5_760);
        assert_eq!(
            candidate_buffer_lengths[5],
            candidate.primitive_count() * 48
        );
        assert_eq!(candidate_buffer_lengths[6], 10_368);
        assert_eq!(candidate_buffer_lengths[7], 2 * 64);
        assert_eq!(candidate, before);
    }

    #[test]
    fn primitive_dispatch_flattens_instance_bases_and_locks_exact_ranges() {
        use crate::presentation::companion_scene::scene::InstanceGroupBinding;

        let cases = [
            (
                Some(InstanceGroupBinding::PetBody),
                0,
                PrimitiveSource::Instances(InstanceSource::PetBody),
                0..130,
            ),
            (
                Some(InstanceGroupBinding::PetParticles),
                u32::MAX,
                PrimitiveSource::Instances(InstanceSource::PetParticles),
                0..0,
            ),
            (
                Some(InstanceGroupBinding::PropGlyphs(4)),
                4 * 9,
                PrimitiveSource::Instances(InstanceSource::PropGlyphs { slot: 4 }),
                0..9,
            ),
            (
                Some(InstanceGroupBinding::TankCells { slot: 1, layer: InstanceLayer::Behind }),
                8,
                PrimitiveSource::Instances(InstanceSource::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Behind,
                }),
                0..8,
            ),
            (
                Some(InstanceGroupBinding::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Foreground,
                }),
                8,
                PrimitiveSource::Instances(InstanceSource::TankCells {
                    slot: 1,
                    layer: InstanceLayer::Foreground,
                }),
                0..8,
            ),
            (
                Some(InstanceGroupBinding::Ambient),
                0,
                PrimitiveSource::Instances(InstanceSource::Ambient),
                0..64,
            ),
            (
                Some(InstanceGroupBinding::Hud),
                0,
                PrimitiveSource::Instances(InstanceSource::Hud),
                0..24,
            ),
        ];

        for (binding, expected_base, expected_source, expected_instances) in cases {
            let mut fixture = SceneFixture::valid();
            fixture.template.primitives[0].kind = PrimitiveKind::InstanceQuad;
            fixture.template.primitives[0].instance_group = binding;
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
        let visible = GlyphAtlasEntry::synthetic_visible(
            GlyphEntryKind::PremultipliedColorRgba,
            AtlasCell { origin: [0, 0], extent: [1, 1] },
        );
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

        let table = prepare_glyph_entries(&atlas);
        assert_eq!(table[0].flags & GLYPH_FLAG_VISIBLE, 0);
        assert_eq!(table[0].metrics[0], 12.0);
        assert_eq!(table[1].flags, GLYPH_FLAG_VISIBLE | GLYPH_FLAG_COLOR);
        assert_eq!(table[1].visible_uv, visible.visible_uv.unwrap());
        assert_eq!(table[1].ink_origin_size, [1.0, 2.0, 10.0, 20.0]);
    }

    #[test]
    fn scene_wgsl_locks_buffer_abi_entrypoints_and_color_responsibilities() {
        let source = SCENE_SHADER_SOURCE;
        for required in [
            "const GLYPH_FLAG_COLOR: u32 = 2u;",
            "struct PrimitiveGpuValue",
            "instance_base: u32",
            "struct SceneContentGpuValue",
            "glyph_entry_index: u32",
            "struct GlyphAtlasGpuEntry",
            "struct NodeBuffer",
            "struct ContentGlobalsBuffer",
            "struct FrameBuffer",
            "struct PrimitiveBuffer",
            "struct SceneContentBuffer",
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
            "if (primitive.instance_group == 2u)",
            "return NONE_U32;",
            "fn vs_world(",
            "fn vs_screen(",
            "fn fs_analytic(",
            "fn fs_glyph(",
            "let atlas_local = vec2<f32>(input.uv.x, 1.0 - input.uv.y);",
            "return mix(entry.visible_uv.xy, entry.visible_uv.zw, atlas_local);",
            "textureSampleLevel(coverage_texture",
            "textureSampleLevel(color_texture",
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
}
