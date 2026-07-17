//! Pure-CPU compilation for fixed retained companion scene generations.

use bytemuck::{Pod, Zeroable};

use super::buffers::{ByteSpan, DirtySpanSet, FixedPodMirror, MirrorError};
use crate::presentation::companion_scene::scene::{
    is_world_blended, AmbientContentKind, AmbientContentSlot, AmbientGlyphPaintSlot,
    AnalyticContentSlot, AnalyticFrameSlot, AnalyticGeometry, AnalyticMaskSource, AnalyticPaint,
    AnalyticSemantic, AnalyticShape, AttachmentInstanceBinding, AttachmentMode, Bounds3,
    ContentDelta, DepthBehavior, FrameDelta, GaugeLineCap, InstanceGroupBinding, InstanceLayer,
    MaterialKind, MoodContentKind, NodeId, PetArtFilter, PetArtSlot, PetPaletteRole,
    PrimitiveBinding, PrimitiveKind, PrimitiveSpace, PropContentSlot, PropGlyphPaintSlot,
    ResourceKind, RoomGlyphContentSlot, SceneContent, SceneFrame, SceneGenerationData,
    SceneTemplate, StatusBeaconTone, TankContentSlot, WeatherContentKind, WorldBlend,
    MAX_AMBIENT_INSTANCES, MAX_ANALYTIC_PARAMS, MAX_ATTACHMENTS, MAX_BLENDED_DRAWS, MAX_LIGHTS,
    MAX_PET_ART_SLOTS, MAX_PROP_GLYPHS_PER_SLOT, MAX_ROOM_GLYPH_SLOTS, MAX_ROUND_TANK_INHABITANTS,
    MAX_SCENE_NODES, MAX_STATIC_PRIMITIVES, MAX_TANK_GLYPHS_PER_SLOT, MAX_VISIBLE_PROPS,
};

const NONE_U32: u32 = u32::MAX;
const PROP_GLYPH_CAPACITY: usize = MAX_VISIBLE_PROPS * MAX_PROP_GLYPHS_PER_SLOT;
const TANK_GLYPH_CAPACITY: usize = MAX_ROUND_TANK_INHABITANTS * MAX_TANK_GLYPHS_PER_SLOT;

// `FrameBuffer.values` begins immediately after the separately typed globals
// record. Props are the first fixed `FrameGpuValue` family and each slot owns
// exactly one record; later packed families follow the order in
// `CpuMirrorShape::FRAME_RECORD_BYTES` / `FRAME_COUNTS`.
pub(super) const PROP_FRAME_GPU_BASE: u32 = 0;
pub(super) const PROP_FRAME_GPU_STRIDE: u32 = 1;
pub(super) const PROP_FRAME_GPU_COUNT: u32 = CpuMirrorShape::FRAME_COUNTS[1] as u32;
pub(super) const FRAME_GPU_VALUE_COUNT: u32 = (CpuMirrorShape::FRAME_COUNTS[1]
    + CpuMirrorShape::FRAME_COUNTS[2]
    + CpuMirrorShape::FRAME_COUNTS[3]
    + CpuMirrorShape::FRAME_COUNTS[4]
    + CpuMirrorShape::FRAME_COUNTS[5]) as u32;

/// Compiler-owned record sizes and fixed capacities consumed by GPU packing.
/// The renderer derives packed offsets from this shape instead of duplicating
/// the CPU mirror ABI as independent literals.
pub(super) struct CpuMirrorShape;

impl CpuMirrorShape {
    pub(super) const NODE_RECORD_BYTES: usize = std::mem::size_of::<NodeGpuValue>();
    pub(super) const NODE_COUNT: usize = MAX_SCENE_NODES;
    pub(super) const CONTENT_RECORD_BYTES: [usize; 8] = [
        std::mem::size_of::<ContentGlobalsGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<AnalyticContentGpuValue>(),
    ];
    pub(super) const CONTENT_COUNTS: [usize; 8] = [
        1,
        MAX_PET_ART_SLOTS,
        PROP_GLYPH_CAPACITY,
        TANK_GLYPH_CAPACITY,
        MAX_AMBIENT_INSTANCES,
        MAX_PET_ART_SLOTS,
        MAX_ROOM_GLYPH_SLOTS,
        MAX_ANALYTIC_PARAMS,
    ];
    pub(super) const FRAME_RECORD_BYTES: [usize; 7] = [
        std::mem::size_of::<FrameGlobalsGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<AnalyticFrameGpuValue>(),
    ];
    pub(super) const FRAME_COUNTS: [usize; 7] = [
        1,
        MAX_VISIBLE_PROPS,
        TANK_GLYPH_CAPACITY,
        MAX_AMBIENT_INSTANCES,
        MAX_LIGHTS,
        MAX_ROOM_GLYPH_SLOTS,
        MAX_ANALYTIC_PARAMS,
    ];
    pub(super) const FRAME_GLOBALS_VIEWPORT_POINTS_OFFSET: usize =
        std::mem::offset_of!(FrameGlobalsGpuValue, viewport_points);
}

pub(super) type StaticIndex = u32;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing immutable vertex ABI; offsets are locked below and Task 9 must
/// declare an identical `wgpu::VertexBufferLayout`.
pub(super) struct StaticVertex {
    local_position: [f32; 3],
    uv: [f32; 2],
    normal: [f32; 3],
    primitive_index: u32,
    material_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing per-node upload ABI shared with the Task 9 shader layout.
pub(super) struct NodeGpuValue {
    pub(super) world: [[f32; 4]; 4],
    opacity: f32,
    visible: u32,
    material_parameter_offset: u32,
    material_parameter_count: u32,
    depth_cue: [f32; 4],
}

impl std::fmt::Debug for NodeGpuValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NodeGpuValue")
            .field("world", &self.world)
            .field("opacity", &"<redacted>")
            .field("visible", &self.visible)
            .field("material_parameter_offset", &self.material_parameter_offset)
            .field("material_parameter_count", &self.material_parameter_count)
            .field("depth_cue", &self.depth_cue)
            .finish()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
/// GPU-facing fixed content-slot upload ABI.
pub(super) struct ContentGpuValue {
    kind: u32,
    glyph_scalar: u32,
    slot: u32,
    subslot: u32,
    signed_data: [i32; 2],
    flags: u32,
    variant: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
/// GPU-facing fixed analytic-content ABI: identity header plus packed paint words.
pub(super) struct AnalyticContentGpuValue {
    id: u32,
    semantic: u32,
    shape: u32,
    flags: u32,
    payload: [u32; 8],
}

/// Narrow semantic projection used by the scene upload translator. The legacy
/// mirror remains unchanged; the translator replaces only `glyph_scalar` in its
/// distinct upload ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContentUploadValue {
    pub(super) kind: u32,
    pub(super) glyph_scalar: u32,
    pub(super) slot: u32,
    pub(super) subslot: u32,
    pub(super) signed_data: [i32; 2],
    pub(super) flags: u32,
    pub(super) variant: u32,
}

impl From<ContentGpuValue> for ContentUploadValue {
    fn from(value: ContentGpuValue) -> Self {
        Self {
            kind: value.kind,
            glyph_scalar: value.glyph_scalar,
            slot: value.slot,
            subslot: value.subslot,
            signed_data: value.signed_data,
            flags: value.flags,
            variant: value.variant,
        }
    }
}

#[cfg(test)]
impl ContentUploadValue {
    pub(super) fn fixture(glyph_scalar: u32, flags: u32) -> Self {
        Self {
            kind: 1,
            glyph_scalar,
            slot: 0,
            subslot: 0,
            signed_data: [0; 2],
            flags,
            variant: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing fixed frame-slot upload ABI.
pub(super) struct FrameGpuValue {
    kind: u32,
    slot: u32,
    flags: u32,
    variant: u32,
    values: [f32; 12],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing fixed analytic-frame ABI with exact point-space geometry.
pub(super) struct AnalyticFrameGpuValue {
    id: u32,
    semantic: u32,
    shape: u32,
    flags: u32,
    rect_points: [f32; 4],
    payload: [f32; 16],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing content-global upload ABI.
pub(super) struct ContentGlobalsGpuValue {
    palette_rgba: [[u32; 4]; 8],
    mood: u32,
    weather: u32,
    glyph_grid_dimensions: [u32; 2],
    glyph_grid_origin_points: [f32; 2],
    pub(super) glyph_cell_extent_points: [f32; 2],
    // Renderer-private mood tint for the retained pet rim. It is deliberately
    // packed into the GPU mirror rather than scene artifacts or diagnostics.
    pub(super) pet_rim_srgba8: u32,
    _private_spatial_padding: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing frame-global upload ABI.
pub(super) struct FrameGlobalsGpuValue {
    pub(super) view: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    pub(super) viewport_points: [f32; 2],
    viewport_pixels: [f32; 2],
    aperture: [f32; 4],
    gauges: [f32; 4],
    dim_amount: f32,
    light_count: u32,
    // Private retained spatial-cue inputs. They never enter scene artifacts,
    // checksums, or Debug output.
    pub(super) activity_opacity: f32,
    pub(super) reduce_motion: u32,
}

impl std::fmt::Debug for FrameGlobalsGpuValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FrameGlobalsGpuValue")
            .field("view", &self.view)
            .field("projection", &self.projection)
            .field("viewport_points", &self.viewport_points)
            .field("viewport_pixels", &self.viewport_pixels)
            .field("aperture", &self.aperture)
            .field("gauges", &"<redacted>")
            .field("dim_amount", &"<redacted>")
            .field("light_count", &self.light_count)
            .finish_non_exhaustive()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// CPU-only semantic metadata used for deterministic checksums and Task 9
/// materialization. This is not a WGSL upload layout; Task 9 must translate it
/// into an explicitly tested storage/uniform ABI before creating GPU buffers.
pub(super) struct CpuPrimitiveDescriptor {
    node_id: u32,
    node_dense_index: u32,
    material_id: u32,
    material_dense_index: u32,
    resource_id: u32,
    resource_dense_index: u32,
    primitive_kind: u32,
    blend: u32,
    depth: u32,
    instance_group: u32,
    instance_slot: u32,
    authored_order: u32,
    first_vertex: u32,
    first_index: u32,
    index_count: u32,
    space: u32,
    local_bounds_min: [f32; 3],
    local_bounds_max: [f32; 3],
    _bounds_padding: [u32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// CPU-only immutable-node metadata. Its Rust matrix/vector packing is suitable
/// for deterministic CPU checksums, not for direct WGSL storage-array upload.
pub(super) struct CpuNodeDescriptor {
    node_id: u32,
    parent_id: u32,
    parent_dense_index: u32,
    _header_padding: u32,
    base_transform: [[f32; 4]; 4],
    local_bounds_min: [f32; 3],
    local_bounds_max: [f32; 3],
    depth_cue: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
/// CPU-only material-table entry consumed by Task 9 materialization.
pub(super) struct CpuMaterialDescriptor {
    material_id: u32,
    kind: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
/// CPU-only resource-table entry consumed by Task 9 materialization.
pub(super) struct CpuResourceDescriptor {
    resource_id: u32,
    kind: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// CPU-only attachment metadata used by checksums and Task 9 materialization.
/// The embedded Rust matrix is deliberately not a claimed WGSL buffer ABI.
pub(super) struct CpuAttachmentDescriptor {
    attachment_id: u32,
    attachment_dense_index: u32,
    owner_id: u32,
    owner_dense_index: u32,
    local_transform: [[f32; 4]; 4],
    mode: u32,
    instance_binding: u32,
    instance_slot: u32,
    source_primitive_dense_index: u32,
    source_node_dense_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DenseSceneIndex {
    nodes: Vec<(NodeId, u32)>,
    materials: Vec<(crate::presentation::companion_scene::scene::MaterialId, u32)>,
    resources: Vec<(crate::presentation::companion_scene::scene::ResourceId, u32)>,
    attachments: Vec<(
        crate::presentation::companion_scene::scene::AttachmentId,
        u32,
    )>,
}

impl DenseSceneIndex {
    pub(super) fn node_offset(&self, id: NodeId) -> Option<u32> {
        self.nodes
            .iter()
            .find_map(|(candidate, offset)| (*candidate == id).then_some(*offset))
    }

    fn material_offset(
        &self,
        id: crate::presentation::companion_scene::scene::MaterialId,
    ) -> Option<u32> {
        self.materials
            .iter()
            .find_map(|(candidate, offset)| (*candidate == id).then_some(*offset))
    }

    fn resource_offset(
        &self,
        id: crate::presentation::companion_scene::scene::ResourceId,
    ) -> Option<u32> {
        self.resources
            .iter()
            .find_map(|(candidate, offset)| (*candidate == id).then_some(*offset))
    }

    fn attachment_offset(
        &self,
        id: crate::presentation::companion_scene::scene::AttachmentId,
    ) -> Option<u32> {
        self.attachments
            .iter()
            .find_map(|(candidate, offset)| (*candidate == id).then_some(*offset))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ContentMirrors {
    globals: FixedPodMirror<ContentGlobalsGpuValue, 1>,
    pet_body: FixedPodMirror<ContentGpuValue, MAX_PET_ART_SLOTS>,
    pet_particles: FixedPodMirror<ContentGpuValue, MAX_PET_ART_SLOTS>,
    room_glyphs: FixedPodMirror<ContentGpuValue, MAX_ROOM_GLYPH_SLOTS>,
    prop_glyphs: FixedPodMirror<ContentGpuValue, PROP_GLYPH_CAPACITY>,
    tank_glyphs: FixedPodMirror<ContentGpuValue, TANK_GLYPH_CAPACITY>,
    ambient: FixedPodMirror<ContentGpuValue, MAX_AMBIENT_INSTANCES>,
    analytics: FixedPodMirror<AnalyticContentGpuValue, MAX_ANALYTIC_PARAMS>,
}

impl ContentMirrors {
    fn zeroed() -> Self {
        Self {
            globals: FixedPodMirror::zeroed(),
            pet_body: FixedPodMirror::zeroed(),
            pet_particles: FixedPodMirror::zeroed(),
            room_glyphs: FixedPodMirror::zeroed(),
            prop_glyphs: FixedPodMirror::zeroed(),
            tank_glyphs: FixedPodMirror::zeroed(),
            ambient: FixedPodMirror::zeroed(),
            analytics: FixedPodMirror::zeroed(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct FrameMirrors {
    globals: FixedPodMirror<FrameGlobalsGpuValue, 1>,
    nodes: FixedPodMirror<NodeGpuValue, MAX_SCENE_NODES>,
    room_glyphs: FixedPodMirror<FrameGpuValue, MAX_ROOM_GLYPH_SLOTS>,
    props: FixedPodMirror<FrameGpuValue, MAX_VISIBLE_PROPS>,
    tank_cells: FixedPodMirror<FrameGpuValue, TANK_GLYPH_CAPACITY>,
    ambient: FixedPodMirror<FrameGpuValue, MAX_AMBIENT_INSTANCES>,
    analytics: FixedPodMirror<AnalyticFrameGpuValue, MAX_ANALYTIC_PARAMS>,
    lights: FixedPodMirror<FrameGpuValue, MAX_LIGHTS>,
}

impl FrameMirrors {
    fn zeroed() -> Self {
        Self {
            globals: FixedPodMirror::zeroed(),
            nodes: FixedPodMirror::zeroed(),
            room_glyphs: FixedPodMirror::zeroed(),
            props: FixedPodMirror::zeroed(),
            tank_cells: FixedPodMirror::zeroed(),
            ambient: FixedPodMirror::zeroed(),
            analytics: FixedPodMirror::zeroed(),
            lights: FixedPodMirror::zeroed(),
        }
    }
}

impl std::fmt::Debug for FrameMirrors {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FrameMirrors")
            .field("globals", &self.globals)
            .field("node_count", &self.nodes.as_slice().len())
            .field("room_glyph_slot_count", &self.room_glyphs.as_slice().len())
            .field("prop_slot_count", &self.props.as_slice().len())
            .field("tank_cell_count", &self.tank_cells.as_slice().len())
            .field("ambient_slot_count", &self.ambient.as_slice().len())
            .field("analytic_slot_count", &self.analytics.as_slice().len())
            .field("light_slot_count", &self.lights.as_slice().len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PhaseLists {
    opaque_cutout: Vec<u32>,
    world_blended_unsorted: Vec<u32>,
    chrome_authored: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BlendedDrawTemplate {
    pub(super) draw_index: u16,
    pub(super) node_index: u16,
    pub(super) semantic_order: u32,
    pub(super) primitive_index: u16,
}

impl BlendedDrawTemplate {
    const EMPTY: Self = Self {
        draw_index: 0,
        node_index: 0,
        semantic_order: 0,
        primitive_index: 0,
    };

    pub(super) const fn new(
        draw_index: u16,
        node_index: u16,
        semantic_order: u32,
        primitive_index: u16,
    ) -> Self {
        Self {
            draw_index,
            node_index,
            semantic_order,
            primitive_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BlendedDrawTemplates {
    records: [BlendedDrawTemplate; MAX_BLENDED_DRAWS],
    len: u16,
}

impl BlendedDrawTemplates {
    #[cfg(test)]
    pub(super) fn from_slice(records: &[BlendedDrawTemplate]) -> Result<Self, CompileError> {
        if records.len() > MAX_BLENDED_DRAWS {
            return Err(CompileError::CapacityExceeded);
        }
        let mut fixed = [BlendedDrawTemplate::EMPTY; MAX_BLENDED_DRAWS];
        fixed[..records.len()].copy_from_slice(records);
        Ok(Self {
            records: fixed,
            len: u16::try_from(records.len()).map_err(|_| CompileError::CapacityExceeded)?,
        })
    }

    pub(super) fn as_slice(&self) -> &[BlendedDrawTemplate] {
        &self.records[..usize::from(self.len)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FixedNodeTopology {
    count: usize,
    parent_before_child: [u16; MAX_SCENE_NODES],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneDirtySpans {
    pub(super) from: crate::presentation::companion_scene::AppliedRevisions,
    pub(super) to: crate::presentation::companion_scene::AppliedRevisions,
    pub(super) content_globals: DirtySpanSet,
    pub(super) pet_body: DirtySpanSet,
    pub(super) pet_particles: DirtySpanSet,
    pub(super) room_content: DirtySpanSet,
    pub(super) prop_glyphs: DirtySpanSet,
    pub(super) tank_glyphs: DirtySpanSet,
    pub(super) content_ambient: DirtySpanSet,
    pub(super) content_analytics: DirtySpanSet,
    pub(super) frame_globals: DirtySpanSet,
    pub(super) nodes: DirtySpanSet,
    pub(super) room_frame: DirtySpanSet,
    pub(super) props: DirtySpanSet,
    pub(super) tank_cells: DirtySpanSet,
    pub(super) frame_ambient: DirtySpanSet,
    pub(super) frame_analytics: DirtySpanSet,
    pub(super) lights: DirtySpanSet,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SceneDirtyMetrics {
    pub(super) content_ranges: u64,
    pub(super) content_bytes: u64,
    pub(super) frame_ranges: u64,
    pub(super) frame_bytes: u64,
}

impl SceneDirtySpans {
    fn empty(
        from: crate::presentation::companion_scene::AppliedRevisions,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> Self {
        Self {
            from,
            to,
            content_globals: DirtySpanSet::default(),
            pet_body: DirtySpanSet::default(),
            pet_particles: DirtySpanSet::default(),
            room_content: DirtySpanSet::default(),
            prop_glyphs: DirtySpanSet::default(),
            tank_glyphs: DirtySpanSet::default(),
            content_ambient: DirtySpanSet::default(),
            content_analytics: DirtySpanSet::default(),
            frame_globals: DirtySpanSet::default(),
            nodes: DirtySpanSet::default(),
            room_frame: DirtySpanSet::default(),
            props: DirtySpanSet::default(),
            tank_cells: DirtySpanSet::default(),
            frame_ambient: DirtySpanSet::default(),
            frame_analytics: DirtySpanSet::default(),
            lights: DirtySpanSet::default(),
        }
    }

    pub(super) fn metrics(self) -> SceneDirtyMetrics {
        let content = [
            self.content_globals,
            self.pet_body,
            self.pet_particles,
            self.room_content,
            self.prop_glyphs,
            self.tank_glyphs,
            self.content_ambient,
            self.content_analytics,
        ];
        let frame = [
            self.frame_globals,
            self.nodes,
            self.room_frame,
            self.props,
            self.tank_cells,
            self.frame_ambient,
            self.frame_analytics,
            self.lights,
        ];
        SceneDirtyMetrics {
            content_ranges: dirty_range_count(&content),
            content_bytes: dirty_byte_count(&content),
            frame_ranges: dirty_range_count(&frame),
            frame_bytes: dirty_byte_count(&frame),
        }
    }
}

fn dirty_range_count(sets: &[DirtySpanSet]) -> u64 {
    sets.iter().fold(0_u64, |total, set| {
        total.saturating_add(u64::try_from(set.as_slice().len()).unwrap_or(u64::MAX))
    })
}

fn dirty_byte_count(sets: &[DirtySpanSet]) -> u64 {
    sets.iter()
        .flat_map(DirtySpanSet::as_slice)
        .fold(0_u64, |total, span| {
            total.saturating_add(u64::try_from(span.len).unwrap_or(u64::MAX))
        })
}

impl Default for SceneDirtySpans {
    fn default() -> Self {
        Self::empty(
            crate::presentation::companion_scene::AppliedRevisions::new(0, 0),
            crate::presentation::companion_scene::AppliedRevisions::new(0, 0),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MirrorDeltaError {
    GenerationMismatch,
    PairMismatch,
    StaleBase,
    InvalidRevisionAdvance,
    Validation(crate::presentation::companion_scene::validate::SceneValidationError),
    Compile(CompileError),
    Mirror(MirrorError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CaptureCandidateError {
    Validation(crate::presentation::companion_scene::validate::SceneValidationError),
    Compile(CompileError),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CpuSceneCandidate {
    instance_identity: CandidateInstanceIdentity,
    pub(super) generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    pub(super) source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    pub(super) static_checksum: u64,
    pub(super) index: DenseSceneIndex,
    pub(super) vertices: Vec<StaticVertex>,
    pub(super) indices: Vec<StaticIndex>,
    pub(super) nodes: Vec<CpuNodeDescriptor>,
    pub(super) materials: Vec<CpuMaterialDescriptor>,
    pub(super) resources: Vec<CpuResourceDescriptor>,
    pub(super) primitives: Vec<CpuPrimitiveDescriptor>,
    pub(super) attachments: Vec<CpuAttachmentDescriptor>,
    pub(super) phases: PhaseLists,
    pub(super) blended_draw_templates: BlendedDrawTemplates,
    pub(super) content: ContentMirrors,
    pub(super) frame: FrameMirrors,
    logical_content: SceneContent,
    accepted: crate::presentation::companion_scene::validate::AcceptedSceneState,
    topology: FixedNodeTopology,
    private_spatial_frame: PrivateSpatialFrame,
    #[cfg(test)]
    last_node_resolves: usize,
}

/// Private source timing/accessibility inputs that travel only through the
/// retained GPU mirror. The public scene contract intentionally omits them.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct PrivateSpatialFrame {
    activity_opacity: f32,
    reduce_motion: bool,
}

impl PrivateSpatialFrame {
    #[cfg(test)]
    const NEUTRAL: Self = Self {
        activity_opacity: 0.0,
        reduce_motion: false,
    };

    pub(super) fn from_snapshot(
        snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    ) -> Self {
        Self {
            activity_opacity: snapshot.frame.activity_opacity,
            reduce_motion: snapshot.frame.reduce_motion,
        }
    }
}

impl std::fmt::Debug for PrivateSpatialFrame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivateSpatialFrame(<redacted>)")
    }
}

/// Runtime ownership capability. Moving preserves it, cloning creates a new
/// instance, and semantic candidate equality deliberately ignores it.
#[derive(Debug)]
struct CandidateInstanceIdentity(std::sync::Arc<()>);

impl CandidateInstanceIdentity {
    fn new() -> Self {
        Self(std::sync::Arc::new(()))
    }
}

impl Clone for CandidateInstanceIdentity {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl PartialEq for CandidateInstanceIdentity {
    fn eq(&self, _other: &Self) -> bool {
        // CpuSceneCandidate equality describes compiled scene state, not which
        // runtime instance owns that state.
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PrimitiveUploadSource {
    pub(super) node_index: u32,
    pub(super) aux_node_index: u32,
    pub(super) material_index: u32,
    pub(super) resource_index: u32,
    pub(super) primitive_kind: u32,
    pub(super) material_kind: u32,
    pub(super) resource_kind: u32,
    pub(super) blend: u32,
    pub(super) depth: u32,
    pub(super) space: u32,
    pub(super) instance_group: u32,
    pub(super) instance_slot: u32,
    pub(super) instance_base: u32,
    pub(super) authored_order: u32,
    pub(super) first_index: u32,
    pub(super) index_count: u32,
}

pub(super) struct ContentUploadSources<'a> {
    pub(super) globals: &'a [u8],
    /// Body-filtered pet records. The render slice still names this first family `pet`.
    pub(super) pet: &'a [ContentGpuValue],
    #[allow(dead_code)] // Consumed when the renderer binds the v2 mirror families.
    pub(super) pet_particles: &'a [ContentGpuValue],
    #[allow(dead_code)] // Consumed when the renderer binds the v2 mirror families.
    pub(super) room_glyphs: &'a [ContentGpuValue],
    pub(super) prop_glyphs: &'a [ContentGpuValue],
    pub(super) tank_glyphs: &'a [ContentGpuValue],
    pub(super) ambient: &'a [ContentGpuValue],
    #[allow(dead_code)] // Consumed when the renderer binds the v2 mirror families.
    pub(super) analytics: &'a [AnalyticContentGpuValue],
}

pub(super) struct FrameUploadSources<'a> {
    pub(super) globals: &'a [u8],
    pub(super) nodes: &'a [u8],
    #[allow(dead_code)] // Consumed when the renderer binds the v2 mirror families.
    pub(super) room_glyphs: &'a [u8],
    pub(super) props: &'a [u8],
    pub(super) tank_cells: &'a [u8],
    pub(super) ambient: &'a [u8],
    #[allow(dead_code)] // Consumed when the renderer binds the v2 mirror families.
    pub(super) analytics: &'a [u8],
    pub(super) lights: &'a [u8],
}

pub(super) struct PhaseUploadSources<'a> {
    pub(super) opaque_cutout: &'a [u32],
    pub(super) world_blended_unsorted: &'a [u32],
    pub(super) chrome_authored: &'a [u32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Task 10G-C consumes the sealed prepared-record visitor.
pub(super) enum PreparedMirrorFamily {
    Nodes,
    ContentGlobals,
    PetBody,
    PetParticles,
    RoomContent,
    PropGlyphs,
    TankGlyphs,
    ContentAmbient,
    ContentAnalytics,
    FrameGlobals,
    RoomFrame,
    Props,
    TankCells,
    FrameAmbient,
    FrameAnalytics,
    Lights,
}

struct PreparedMirrorDelta {
    dirty: SceneDirtySpans,
    content_globals: Option<ContentGlobalsGpuValue>,
    pet_body: [Option<ContentGpuValue>; MAX_PET_ART_SLOTS],
    pet_particles: [Option<ContentGpuValue>; MAX_PET_ART_SLOTS],
    room_content: [Option<ContentGpuValue>; MAX_ROOM_GLYPH_SLOTS],
    prop_glyphs: [Option<ContentGpuValue>; PROP_GLYPH_CAPACITY],
    tank_glyphs: [Option<ContentGpuValue>; TANK_GLYPH_CAPACITY],
    content_ambient: [Option<ContentGpuValue>; MAX_AMBIENT_INSTANCES],
    content_analytics: [Option<AnalyticContentGpuValue>; MAX_ANALYTIC_PARAMS],
    frame_globals: Option<FrameGlobalsGpuValue>,
    nodes: [Option<NodeGpuValue>; MAX_SCENE_NODES],
    room_frame: [Option<FrameGpuValue>; MAX_ROOM_GLYPH_SLOTS],
    props: [Option<FrameGpuValue>; MAX_VISIBLE_PROPS],
    tank_cells: [Option<FrameGpuValue>; TANK_GLYPH_CAPACITY],
    frame_ambient: [Option<FrameGpuValue>; MAX_AMBIENT_INSTANCES],
    frame_analytics: [Option<AnalyticFrameGpuValue>; MAX_ANALYTIC_PARAMS],
    lights: [Option<FrameGpuValue>; MAX_LIGHTS],
    #[cfg(test)]
    node_resolves: usize,
}

#[derive(Debug, Clone)]
struct PreparedSceneDeltaBinding {
    candidate_identity: std::sync::Arc<()>,
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    static_checksum: u64,
    from: crate::presentation::companion_scene::AppliedRevisions,
    to: crate::presentation::companion_scene::AppliedRevisions,
    state_epoch: u64,
}

impl PartialEq for PreparedSceneDeltaBinding {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.candidate_identity, &other.candidate_identity)
            && self.generation_key == other.generation_key
            && self.static_checksum == other.static_checksum
            && self.from == other.from
            && self.to == other.to
            && self.state_epoch == other.state_epoch
    }
}

impl Eq for PreparedSceneDeltaBinding {}

struct PreparedLogicalContentDelta {
    palette: Option<[[u8; 3]; 8]>,
    mood: Option<MoodContentKind>,
    weather: Option<WeatherContentKind>,
    day_phase: Option<crate::presentation::companion_scene::CompanionDayPhase>,
    pet_art_slots: [Option<PetArtSlot>; MAX_PET_ART_SLOTS],
    room_glyph_slots: [Option<RoomGlyphContentSlot>; MAX_ROOM_GLYPH_SLOTS],
    prop_slots: [Option<PropContentSlot>; MAX_VISIBLE_PROPS],
    tank_slots: [Option<TankContentSlot>; MAX_ROUND_TANK_INHABITANTS],
    ambient_slots: [Option<AmbientContentSlot>; MAX_AMBIENT_INSTANCES],
    prop_paint_slots: [Option<PropGlyphPaintSlot>; MAX_VISIBLE_PROPS],
    ambient_paint_slots: [Option<AmbientGlyphPaintSlot>; MAX_AMBIENT_INSTANCES],
    analytic_slots: [Option<AnalyticContentSlot>; MAX_ANALYTIC_PARAMS],
}

/// Sealed proof that both neutral scene state and packed mirrors can commit
/// without another fallible operation.
pub(super) struct PreparedSceneDelta {
    binding: PreparedSceneDeltaBinding,
    logical_content: PreparedLogicalContentDelta,
    accepted_frame: crate::presentation::companion_scene::validate::PreparedAcceptedFrameDelta,
    mirrors: PreparedMirrorDelta,
    prospective_logical_viewport_points: [f32; 2],
    blended_depth_dirty: bool,
    private_spatial_frame: PrivateSpatialFrame,
}

impl PreparedSceneDelta {
    #[allow(dead_code)] // Read before GPU staging lands in the next retained-renderer checkpoint.
    pub(super) fn prospective_logical_viewport_points(&self) -> [f32; 2] {
        self.prospective_logical_viewport_points
    }

    #[allow(dead_code)] // Task 10G-C maps these spans into four physical buffers.
    pub(super) const fn dirty_spans(&self) -> SceneDirtySpans {
        self.mirrors.dirty
    }

    pub(super) const fn blended_depth_dirty(&self) -> bool {
        self.blended_depth_dirty
    }

    #[allow(dead_code)] // Task 10G-C stages these sealed records after translation.
    pub(super) fn visit_mirror_updates(
        &self,
        visitor: impl FnMut(PreparedMirrorFamily, usize, &[u8]),
    ) {
        self.mirrors.visit_updates(visitor);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AppliedSceneDelta {
    pub(super) dirty: SceneDirtySpans,
    pub(super) generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    pub(super) static_checksum: u64,
    pub(super) to: crate::presentation::companion_scene::AppliedRevisions,
    pub(super) prospective_logical_viewport_points: [f32; 2],
}

impl PreparedMirrorDelta {
    fn empty(
        from: crate::presentation::companion_scene::AppliedRevisions,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> Self {
        Self {
            dirty: SceneDirtySpans::empty(from, to),
            content_globals: None,
            pet_body: [None; MAX_PET_ART_SLOTS],
            pet_particles: [None; MAX_PET_ART_SLOTS],
            room_content: [None; MAX_ROOM_GLYPH_SLOTS],
            prop_glyphs: [None; PROP_GLYPH_CAPACITY],
            tank_glyphs: [None; TANK_GLYPH_CAPACITY],
            content_ambient: [None; MAX_AMBIENT_INSTANCES],
            content_analytics: [None; MAX_ANALYTIC_PARAMS],
            frame_globals: None,
            nodes: [None; MAX_SCENE_NODES],
            room_frame: [None; MAX_ROOM_GLYPH_SLOTS],
            props: [None; MAX_VISIBLE_PROPS],
            tank_cells: [None; TANK_GLYPH_CAPACITY],
            frame_ambient: [None; MAX_AMBIENT_INSTANCES],
            frame_analytics: [None; MAX_ANALYTIC_PARAMS],
            lights: [None; MAX_LIGHTS],
            #[cfg(test)]
            node_resolves: 0,
        }
    }

    #[allow(dead_code)] // Reached through PreparedSceneDelta in Task 10G-C.
    fn visit_updates(&self, mut visitor: impl FnMut(PreparedMirrorFamily, usize, &[u8])) {
        visit_prepared_records(PreparedMirrorFamily::Nodes, &self.nodes, &mut visitor);
        if let Some(value) = self.content_globals.as_ref() {
            visitor(
                PreparedMirrorFamily::ContentGlobals,
                0,
                bytemuck::bytes_of(value),
            );
        }
        visit_prepared_records(PreparedMirrorFamily::PetBody, &self.pet_body, &mut visitor);
        visit_prepared_records(
            PreparedMirrorFamily::PetParticles,
            &self.pet_particles,
            &mut visitor,
        );
        visit_prepared_records(
            PreparedMirrorFamily::RoomContent,
            &self.room_content,
            &mut visitor,
        );
        visit_prepared_records(
            PreparedMirrorFamily::PropGlyphs,
            &self.prop_glyphs,
            &mut visitor,
        );
        visit_prepared_records(
            PreparedMirrorFamily::TankGlyphs,
            &self.tank_glyphs,
            &mut visitor,
        );
        visit_prepared_records(
            PreparedMirrorFamily::ContentAmbient,
            &self.content_ambient,
            &mut visitor,
        );
        visit_prepared_records(
            PreparedMirrorFamily::ContentAnalytics,
            &self.content_analytics,
            &mut visitor,
        );
        if let Some(value) = self.frame_globals.as_ref() {
            visitor(
                PreparedMirrorFamily::FrameGlobals,
                0,
                bytemuck::bytes_of(value),
            );
        }
        visit_prepared_records(
            PreparedMirrorFamily::RoomFrame,
            &self.room_frame,
            &mut visitor,
        );
        visit_prepared_records(PreparedMirrorFamily::Props, &self.props, &mut visitor);
        visit_prepared_records(
            PreparedMirrorFamily::TankCells,
            &self.tank_cells,
            &mut visitor,
        );
        visit_prepared_records(
            PreparedMirrorFamily::FrameAmbient,
            &self.frame_ambient,
            &mut visitor,
        );
        visit_prepared_records(
            PreparedMirrorFamily::FrameAnalytics,
            &self.frame_analytics,
            &mut visitor,
        );
        visit_prepared_records(PreparedMirrorFamily::Lights, &self.lights, &mut visitor);
    }
}

#[allow(dead_code)] // Reached through PreparedSceneDelta in Task 10G-C.
fn visit_prepared_records<T: Pod, const N: usize>(
    family: PreparedMirrorFamily,
    records: &[Option<T>; N],
    visitor: &mut impl FnMut(PreparedMirrorFamily, usize, &[u8]),
) {
    for (slot, value) in records.iter().enumerate() {
        if let Some(value) = value.as_ref() {
            visitor(family, slot, bytemuck::bytes_of(value));
        }
    }
}

fn prepare_fixed_overlay<T: Copy, const N: usize>(
    values: &[T],
    slot_of: impl Fn(T) -> usize,
) -> Result<[Option<T>; N], MirrorDeltaError> {
    let mut overlay = [None; N];
    for value in values.iter().copied() {
        let slot = slot_of(value);
        let destination = overlay
            .get_mut(slot)
            .ok_or(MirrorDeltaError::Mirror(MirrorError::CapacityExceeded))?;
        if destination.replace(value).is_some() {
            return Err(MirrorDeltaError::Validation(
                crate::presentation::companion_scene::validate::SceneValidationError::DuplicateSlot,
            ));
        }
    }
    Ok(overlay)
}

impl PreparedLogicalContentDelta {
    fn prepare(delta: &ContentDelta) -> Result<Self, MirrorDeltaError> {
        Ok(Self {
            palette: delta.palette,
            mood: delta.mood,
            weather: delta.weather,
            day_phase: delta.day_phase,
            pet_art_slots: prepare_fixed_overlay(&delta.pet_art_slots, |slot| {
                usize::from(slot.slot)
            })?,
            room_glyph_slots: prepare_fixed_overlay(&delta.room_glyph_slots, |slot| {
                usize::from(slot.slot)
            })?,
            prop_slots: prepare_fixed_overlay(&delta.prop_slots, |slot| usize::from(slot.slot))?,
            tank_slots: prepare_fixed_overlay(&delta.tank_slots, |slot| usize::from(slot.slot))?,
            ambient_slots: prepare_fixed_overlay(&delta.ambient_slots, |slot| {
                usize::from(slot.slot)
            })?,
            prop_paint_slots: prepare_fixed_overlay(&delta.prop_paint_slots, |slot| {
                usize::from(slot.slot)
            })?,
            ambient_paint_slots: prepare_fixed_overlay(&delta.ambient_paint_slots, |slot| {
                usize::from(slot.slot)
            })?,
            analytic_slots: prepare_fixed_overlay(&delta.analytic_slots, |slot| {
                usize::from(slot.id.0)
            })?,
        })
    }

    fn commit(self, content: &mut SceneContent) {
        if let Some(palette) = self.palette {
            content.palette = palette;
        }
        if let Some(mood) = self.mood {
            content.mood = mood;
        }
        if let Some(weather) = self.weather {
            content.weather = weather;
        }
        if let Some(day_phase) = self.day_phase {
            content.day_phase = day_phase;
        }
        for (slot, changed) in self.pet_art_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.pet_art_slots[slot] = changed;
            }
        }
        for (slot, changed) in self.room_glyph_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.room_glyph_slots[slot] = changed;
            }
        }
        for (slot, changed) in self.prop_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.prop_slots[slot] = changed;
            }
        }
        for (slot, changed) in self.tank_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.tank_slots[slot] = changed;
            }
        }
        for (slot, changed) in self.ambient_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.ambient_slots[slot] = changed;
            }
        }
        for (slot, changed) in self.prop_paint_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.prop_paint_slots[slot] = changed;
            }
        }
        for (slot, changed) in self.ambient_paint_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.ambient_paint_slots[slot] = changed;
            }
        }
        for (slot, changed) in self.analytic_slots.into_iter().enumerate() {
            if let Some(changed) = changed {
                content.analytic_slots[slot] = changed;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompileError {
    CapacityExceeded,
    MissingNode,
    MissingMaterial,
    MissingResource,
    MissingFrameNode,
    InvalidTransform,
    InvalidCamera,
    HierarchyCycle,
    MissingAttachmentSource,
    AmbiguousAttachmentSource,
}

impl CpuSceneCandidate {
    /// A stable fingerprint of every mutable vector/fixed-mirror allocation that
    /// ordinary compatible deltas are required to reuse. Lifetime qualification
    /// samples this before and after each phase; any pointer or capacity change is
    /// reported as real capacity growth instead of relying on a default counter.
    pub(super) fn storage_capacity_signature(&self) -> u64 {
        fn mix(signature: &mut u64, pointer: usize, capacity: usize) {
            *signature = signature
                .wrapping_mul(0x100_0000_01b3)
                .wrapping_add(pointer as u64)
                .wrapping_mul(0x100_0000_01b3)
                .wrapping_add(capacity as u64);
        }

        let mut signature = 0xcbf2_9ce4_8422_2325_u64;
        macro_rules! observe_vec {
            ($values:expr) => {{
                let values = &$values;
                mix(&mut signature, values.as_ptr() as usize, values.capacity());
            }};
        }
        observe_vec!(self.vertices);
        observe_vec!(self.indices);
        observe_vec!(self.nodes);
        observe_vec!(self.materials);
        observe_vec!(self.resources);
        observe_vec!(self.primitives);
        observe_vec!(self.attachments);
        observe_vec!(self.phases.opaque_cutout);
        observe_vec!(self.phases.world_blended_unsorted);
        observe_vec!(self.phases.chrome_authored);
        mix(
            &mut signature,
            self.content.globals.as_slice().as_ptr() as usize,
            self.content.globals.capacity(),
        );
        mix(
            &mut signature,
            self.frame.globals.as_slice().as_ptr() as usize,
            self.frame.globals.capacity(),
        );
        for (pointer, capacity) in [
            (
                self.content.pet_body.as_slice().as_ptr() as usize,
                self.content.pet_body.capacity(),
            ),
            (
                self.content.pet_particles.as_slice().as_ptr() as usize,
                self.content.pet_particles.capacity(),
            ),
            (
                self.content.room_glyphs.as_slice().as_ptr() as usize,
                self.content.room_glyphs.capacity(),
            ),
            (
                self.content.prop_glyphs.as_slice().as_ptr() as usize,
                self.content.prop_glyphs.capacity(),
            ),
            (
                self.content.tank_glyphs.as_slice().as_ptr() as usize,
                self.content.tank_glyphs.capacity(),
            ),
            (
                self.content.ambient.as_slice().as_ptr() as usize,
                self.content.ambient.capacity(),
            ),
            (
                self.content.analytics.as_slice().as_ptr() as usize,
                self.content.analytics.capacity(),
            ),
            (
                self.frame.nodes.as_slice().as_ptr() as usize,
                self.frame.nodes.capacity(),
            ),
            (
                self.frame.room_glyphs.as_slice().as_ptr() as usize,
                self.frame.room_glyphs.capacity(),
            ),
            (
                self.frame.props.as_slice().as_ptr() as usize,
                self.frame.props.capacity(),
            ),
            (
                self.frame.tank_cells.as_slice().as_ptr() as usize,
                self.frame.tank_cells.capacity(),
            ),
            (
                self.frame.ambient.as_slice().as_ptr() as usize,
                self.frame.ambient.capacity(),
            ),
            (
                self.frame.analytics.as_slice().as_ptr() as usize,
                self.frame.analytics.capacity(),
            ),
            (
                self.frame.lights.as_slice().as_ptr() as usize,
                self.frame.lights.capacity(),
            ),
        ] {
            mix(&mut signature, pointer, capacity);
        }
        signature
    }

    /// Recompiles the last-presented logical scene into an isolated candidate
    /// whose private frame inputs have the canonical external-capture
    /// projection. The live candidate and its packed mirrors are not modified.
    pub(super) fn capture_safe_clone(&self) -> Result<Self, CaptureCandidateError> {
        let template = self.accepted.template().template();
        let frame = self.accepted.frame().frame().capture_safe_clone(template);
        let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
            template,
            &self.logical_content,
            &frame,
        )
        .map_err(CaptureCandidateError::Validation)?;
        compile_cpu_parts_with_private(
            self.generation_key,
            self.source_revisions,
            template,
            &self.logical_content,
            &frame,
            accepted,
            self.private_spatial_frame,
        )
        .map_err(CaptureCandidateError::Compile)
    }

    pub(super) fn scene_artifacts(
        &self,
    ) -> Result<
        crate::presentation::companion_scene::contract::SceneArtifacts,
        crate::presentation::companion_scene::validate::SceneValidationError,
    > {
        crate::presentation::companion_scene::contract::SceneArtifacts::try_from_parts(
            self.accepted.template().template(),
            &self.logical_content,
            self.accepted.frame().frame(),
        )
    }

    pub(super) fn capture_source_identity(
        &self,
    ) -> Result<
        crate::presentation::companion_scene::contract::CaptureSourceIdentity,
        crate::presentation::companion_scene::scene::SceneGenerationError,
    > {
        let template = self.accepted.template().template();
        Ok(
            crate::presentation::companion_scene::contract::CaptureSourceIdentity::new(
                template.generation_checksum,
                crate::presentation::companion_scene::scene::checksum_content_for_capture(
                    &self.logical_content,
                )?,
                self.accepted
                    .frame()
                    .frame()
                    .capture_source_checksum(template)?,
            ),
        )
    }

    /// Logical point extent authored by the scene camera. The host combines
    /// this with its backing scale when validating a physical render request;
    /// no surface metadata enters the compiler-owned frame mirror.
    pub(super) fn logical_viewport_points(&self) -> [f32; 2] {
        self.frame.globals.as_slice()[0].viewport_points
    }

    #[cfg(test)]
    pub(super) fn accepted_frame_for_test(
        &self,
    ) -> &crate::presentation::companion_scene::scene::SceneFrame {
        self.accepted.frame().frame()
    }

    #[cfg(test)]
    pub(super) fn accepted_content_for_test(
        &self,
    ) -> &crate::presentation::companion_scene::scene::SceneContent {
        &self.logical_content
    }

    pub(super) fn primitive_count(&self) -> usize {
        self.primitives.len()
    }

    pub(super) fn primitive_upload_source(&self, index: usize) -> Option<PrimitiveUploadSource> {
        let primitive = *self.primitives.get(index)?;
        let material_kind = self
            .materials
            .get(usize::try_from(primitive.material_dense_index).ok()?)?
            .kind;
        let resource_kind = if primitive.resource_dense_index == NONE_U32 {
            0
        } else {
            self.resources
                .get(usize::try_from(primitive.resource_dense_index).ok()?)?
                .kind
        };
        let instance_base = match primitive.instance_group {
            1 | 2 | 4 | 7 | 8 => 0,
            3 => primitive.instance_slot.checked_mul(
                u32::try_from(MAX_PROP_GLYPHS_PER_SLOT).expect("prop glyph capacity fits in u32"),
            )?,
            5 | 6 => primitive.instance_slot.checked_mul(
                u32::try_from(MAX_TANK_GLYPHS_PER_SLOT).expect("tank glyph capacity fits in u32"),
            )?,
            0 => NONE_U32,
            _ => return None,
        };
        let aux_node_index = if primitive.primitive_kind
            == primitive_kind_tag(PrimitiveKind::AnalyticShape)
            && primitive.instance_group == 0
            && primitive.instance_slot == u32::from(AnalyticSemantic::WallShadow.id().0)
        {
            self.primitives
                .iter()
                .find(|candidate| candidate.instance_group == 1)?
                .node_dense_index
        } else {
            NONE_U32
        };
        Some(PrimitiveUploadSource {
            node_index: primitive.node_dense_index,
            aux_node_index,
            material_index: primitive.material_dense_index,
            resource_index: primitive.resource_dense_index,
            primitive_kind: primitive.primitive_kind,
            material_kind,
            resource_kind,
            blend: primitive.blend,
            depth: primitive.depth,
            space: primitive.space,
            instance_group: primitive.instance_group,
            instance_slot: primitive.instance_slot,
            instance_base,
            authored_order: primitive.authored_order,
            first_index: primitive.first_index,
            index_count: primitive.index_count,
        })
    }

    pub(super) fn vertex_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.vertices)
    }

    pub(super) fn index_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.indices)
    }

    pub(super) fn content_upload_sources(&self) -> ContentUploadSources<'_> {
        ContentUploadSources {
            globals: bytemuck::cast_slice(self.content.globals.as_slice()),
            pet: self.content.pet_body.as_slice(),
            pet_particles: self.content.pet_particles.as_slice(),
            room_glyphs: self.content.room_glyphs.as_slice(),
            prop_glyphs: self.content.prop_glyphs.as_slice(),
            tank_glyphs: self.content.tank_glyphs.as_slice(),
            ambient: self.content.ambient.as_slice(),
            analytics: self.content.analytics.as_slice(),
        }
    }

    pub(super) fn frame_upload_sources(&self) -> FrameUploadSources<'_> {
        FrameUploadSources {
            globals: bytemuck::cast_slice(self.frame.globals.as_slice()),
            nodes: bytemuck::cast_slice(self.frame.nodes.as_slice()),
            room_glyphs: bytemuck::cast_slice(self.frame.room_glyphs.as_slice()),
            props: bytemuck::cast_slice(self.frame.props.as_slice()),
            tank_cells: bytemuck::cast_slice(self.frame.tank_cells.as_slice()),
            ambient: bytemuck::cast_slice(self.frame.ambient.as_slice()),
            analytics: bytemuck::cast_slice(self.frame.analytics.as_slice()),
            lights: bytemuck::cast_slice(self.frame.lights.as_slice()),
        }
    }

    pub(super) fn phase_upload_sources(&self) -> PhaseUploadSources<'_> {
        PhaseUploadSources {
            opaque_cutout: &self.phases.opaque_cutout,
            world_blended_unsorted: &self.phases.world_blended_unsorted,
            chrome_authored: &self.phases.chrome_authored,
        }
    }

    #[allow(dead_code)] // Task 9 routes reconciler deltas through this paired transaction.
    pub(super) fn apply_deltas(
        &mut self,
        content_delta: &ContentDelta,
        frame_delta: &FrameDelta,
    ) -> Result<SceneDirtySpans, MirrorDeltaError> {
        let prepared = self.prepare_deltas(content_delta, frame_delta)?;
        Ok(self.commit_prepared(prepared).dirty)
    }

    pub(super) fn prepare_deltas(
        &self,
        content_delta: &ContentDelta,
        frame_delta: &FrameDelta,
    ) -> Result<PreparedSceneDelta, MirrorDeltaError> {
        self.prepare_deltas_with_private(content_delta, frame_delta, self.private_spatial_frame)
    }

    pub(super) fn private_spatial_frame_matches(&self, frame: PrivateSpatialFrame) -> bool {
        self.private_spatial_frame == frame
    }

    pub(super) fn prepare_deltas_with_private(
        &self,
        content_delta: &ContentDelta,
        frame_delta: &FrameDelta,
        private_spatial_frame: PrivateSpatialFrame,
    ) -> Result<PreparedSceneDelta, MirrorDeltaError> {
        if content_delta.generation_key != self.generation_key
            || frame_delta.generation_key != self.generation_key
        {
            return Err(MirrorDeltaError::GenerationMismatch);
        }
        if content_delta.generation_key != frame_delta.generation_key
            || content_delta.from != frame_delta.from
            || content_delta.to != frame_delta.to
        {
            return Err(MirrorDeltaError::PairMismatch);
        }
        if content_delta.from != self.source_revisions {
            return Err(MirrorDeltaError::StaleBase);
        }
        if content_delta.to.semantic < content_delta.from.semantic
            || content_delta.to.frame < content_delta.from.frame
            || (content_delta_has_values(content_delta)
                && content_delta.to.semantic == content_delta.from.semantic)
            || (frame_delta_has_values(frame_delta)
                && content_delta.to.frame == content_delta.from.frame)
        {
            return Err(MirrorDeltaError::InvalidRevisionAdvance);
        }

        crate::presentation::companion_scene::validate::validate_content_frame_delta(
            &self.logical_content,
            self.accepted.frame().frame(),
            content_delta,
            frame_delta,
        )
        .map_err(MirrorDeltaError::Validation)?;
        let accepted_frame = self
            .accepted
            .prepare_frame_delta(frame_delta)
            .map_err(MirrorDeltaError::Validation)?;
        let mut mirrors = self.prepare_mirror_delta(content_delta, frame_delta)?;
        if private_spatial_frame != self.private_spatial_frame {
            let mut globals = mirrors
                .frame_globals
                .unwrap_or(self.frame.globals.as_slice()[0]);
            globals.activity_opacity = private_spatial_frame.activity_opacity;
            globals.reduce_motion = u32::from(private_spatial_frame.reduce_motion);
            record_single(
                &self.frame.globals,
                &mut mirrors.frame_globals,
                &mut mirrors.dirty.frame_globals,
                globals,
            );
        }
        let blended_depth_dirty = self.blended_depth_dirty(frame_delta, &mirrors);
        let logical_content = PreparedLogicalContentDelta::prepare(content_delta)?;
        let prospective_logical_viewport_points = frame_delta
            .camera
            .map(|camera| [camera.width_points, camera.height_points])
            .unwrap_or_else(|| self.logical_viewport_points());
        Ok(PreparedSceneDelta {
            binding: self.prepared_delta_binding(content_delta.to),
            logical_content,
            accepted_frame,
            mirrors,
            prospective_logical_viewport_points,
            blended_depth_dirty,
            private_spatial_frame,
        })
    }

    pub(super) fn commit_prepared(&mut self, prepared: PreparedSceneDelta) -> AppliedSceneDelta {
        assert_eq!(
            prepared.binding,
            self.prepared_delta_binding(prepared.binding.to),
            "prepared scene delta does not belong to this candidate state"
        );
        let to = prepared.binding.to;
        let PreparedSceneDelta {
            logical_content,
            accepted_frame,
            mirrors,
            prospective_logical_viewport_points,
            private_spatial_frame,
            ..
        } = prepared;
        self.accepted.commit_prepared_frame_delta(accepted_frame);
        logical_content.commit(&mut self.logical_content);
        let dirty = mirrors.dirty;
        #[cfg(test)]
        let node_resolves = mirrors.node_resolves;
        self.commit_mirror_delta(mirrors);
        #[cfg(test)]
        {
            self.last_node_resolves = node_resolves;
        }
        self.source_revisions = to;
        self.private_spatial_frame = private_spatial_frame;
        AppliedSceneDelta {
            dirty,
            generation_key: self.generation_key,
            static_checksum: self.static_checksum,
            to,
            prospective_logical_viewport_points,
        }
    }

    fn prepared_delta_binding(
        &self,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> PreparedSceneDeltaBinding {
        PreparedSceneDeltaBinding {
            candidate_identity: std::sync::Arc::clone(&self.instance_identity.0),
            generation_key: self.generation_key,
            static_checksum: self.static_checksum,
            from: self.source_revisions,
            to,
            state_epoch: self.accepted.frame().epoch(),
        }
    }

    fn blended_depth_dirty(
        &self,
        frame_delta: &FrameDelta,
        prepared: &PreparedMirrorDelta,
    ) -> bool {
        if frame_delta.camera.is_some() {
            return true;
        }
        let view = self.frame.globals.as_slice()[0].view;
        self.blended_draw_templates
            .as_slice()
            .iter()
            .any(|template| {
                let node_index = usize::from(template.node_index);
                let Some(updated) = prepared.nodes[node_index] else {
                    return false;
                };
                let current = self.frame.nodes.as_slice()[node_index];
                camera_space_origin_depth(view, current.world).to_bits()
                    != camera_space_origin_depth(view, updated.world).to_bits()
            })
    }
}

fn camera_space_origin_depth(view: [[f32; 4]; 4], world: [[f32; 4]; 4]) -> f32 {
    let origin = world[3];
    view[0][2] * origin[0]
        + view[1][2] * origin[1]
        + view[2][2] * origin[2]
        + view[3][2] * origin[3]
}

fn content_delta_has_values(delta: &ContentDelta) -> bool {
    delta.palette.is_some()
        || delta.mood.is_some()
        || delta.weather.is_some()
        || delta.day_phase.is_some()
        || !delta.pet_art_slots.is_empty()
        || !delta.room_glyph_slots.is_empty()
        || !delta.prop_slots.is_empty()
        || !delta.tank_slots.is_empty()
        || !delta.ambient_slots.is_empty()
        || !delta.prop_paint_slots.is_empty()
        || !delta.ambient_paint_slots.is_empty()
        || !delta.analytic_slots.is_empty()
}

fn frame_delta_has_values(delta: &FrameDelta) -> bool {
    delta.camera.is_some()
        || !delta.nodes.is_empty()
        || !delta.room_glyph_slots.is_empty()
        || !delta.prop_slots.is_empty()
        || !delta.tank_slots.is_empty()
        || !delta.ambient_slots.is_empty()
        || !delta.analytic_slots.is_empty()
        || delta.gauges.is_some()
        || delta.dim_amount.is_some()
        || !delta.lights.is_empty()
}

impl CpuSceneCandidate {
    fn prepare_mirror_delta(
        &self,
        content_delta: &ContentDelta,
        frame_delta: &FrameDelta,
    ) -> Result<PreparedMirrorDelta, MirrorDeltaError> {
        let mut prepared = PreparedMirrorDelta::empty(content_delta.from, content_delta.to);

        if content_delta.palette.is_some()
            || content_delta.mood.is_some()
            || content_delta.weather.is_some()
        {
            let mut value = self.content.globals.as_slice()[0];
            if let Some(palette) = content_delta.palette {
                value.palette_rgba = palette
                    .map(|rgb| [u32::from(rgb[0]), u32::from(rgb[1]), u32::from(rgb[2]), 255]);
            }
            if let Some(mood) = content_delta.mood {
                value.mood = mood_tag(mood);
                value.pet_rim_srgba8 = mood_rim_srgba8(mood);
            }
            if let Some(weather) = content_delta.weather {
                value.weather = weather_tag(weather);
            }
            record_single(
                &self.content.globals,
                &mut prepared.content_globals,
                &mut prepared.dirty.content_globals,
                value,
            );
        }
        for changed in &content_delta.pet_art_slots {
            record_update(
                &self.content.pet_body,
                &mut prepared.pet_body,
                &mut prepared.dirty.pet_body,
                usize::from(changed.slot),
                pack_pet_content(*changed, PetArtFilter::Body),
            )?;
            record_update(
                &self.content.pet_particles,
                &mut prepared.pet_particles,
                &mut prepared.dirty.pet_particles,
                usize::from(changed.slot),
                pack_pet_content(*changed, PetArtFilter::Particles),
            )?;
        }
        for changed in &content_delta.room_glyph_slots {
            record_update(
                &self.content.room_glyphs,
                &mut prepared.room_content,
                &mut prepared.dirty.room_content,
                usize::from(changed.slot),
                pack_room_content(*changed),
            )?;
        }
        for changed in &content_delta.prop_slots {
            let slot = usize::from(changed.slot);
            if slot >= MAX_VISIBLE_PROPS {
                return Err(MirrorDeltaError::Mirror(MirrorError::CapacityExceeded));
            }
            let paint = content_delta
                .prop_paint_slots
                .iter()
                .find(|paint| paint.slot == changed.slot)
                .unwrap_or(&self.logical_content.prop_paint_slots[slot]);
            for subslot in 0..MAX_PROP_GLYPHS_PER_SLOT {
                let flat = slot * MAX_PROP_GLYPHS_PER_SLOT + subslot;
                record_update(
                    &self.content.prop_glyphs,
                    &mut prepared.prop_glyphs,
                    &mut prepared.dirty.prop_glyphs,
                    flat,
                    pack_prop_content(*changed, paint.paints[subslot], subslot),
                )?;
            }
        }
        for changed in &content_delta.tank_slots {
            let slot = usize::from(changed.slot);
            if slot >= MAX_ROUND_TANK_INHABITANTS {
                return Err(MirrorDeltaError::Mirror(MirrorError::CapacityExceeded));
            }
            for subslot in 0..MAX_TANK_GLYPHS_PER_SLOT {
                let flat = slot * MAX_TANK_GLYPHS_PER_SLOT + subslot;
                record_update(
                    &self.content.tank_glyphs,
                    &mut prepared.tank_glyphs,
                    &mut prepared.dirty.tank_glyphs,
                    flat,
                    pack_tank_content(*changed, subslot),
                )?;
            }
        }
        for changed in &content_delta.ambient_slots {
            let slot = usize::from(changed.slot);
            let paint = content_delta
                .ambient_paint_slots
                .iter()
                .find(|paint| paint.slot == changed.slot)
                .unwrap_or(&self.logical_content.ambient_paint_slots[slot]);
            record_update(
                &self.content.ambient,
                &mut prepared.content_ambient,
                &mut prepared.dirty.content_ambient,
                slot,
                pack_ambient_content(*changed, paint.paint),
            )?;
        }
        for changed in &content_delta.analytic_slots {
            record_update(
                &self.content.analytics,
                &mut prepared.content_analytics,
                &mut prepared.dirty.content_analytics,
                usize::from(changed.id.0),
                pack_analytic_content(*changed),
            )?;
        }

        if frame_delta.camera.is_some()
            || frame_delta.gauges.is_some()
            || frame_delta.dim_amount.is_some()
        {
            let mut value = self.frame.globals.as_slice()[0];
            if let Some(camera) = frame_delta.camera {
                value.projection = camera
                    .projection_matrix()
                    .map_err(|_| MirrorDeltaError::Compile(CompileError::InvalidCamera))?
                    .columns;
                value.viewport_points = [camera.width_points, camera.height_points];
            }
            if let Some(gauges) = frame_delta.gauges {
                value.gauges = gauges;
            }
            if let Some(dim_amount) = frame_delta.dim_amount {
                value.dim_amount = dim_amount;
            }
            record_single(
                &self.frame.globals,
                &mut prepared.frame_globals,
                &mut prepared.dirty.frame_globals,
                value,
            );
        }
        self.prepare_node_updates(frame_delta, &mut prepared)?;
        for changed in &frame_delta.room_glyph_slots {
            record_update(
                &self.frame.room_glyphs,
                &mut prepared.room_frame,
                &mut prepared.dirty.room_frame,
                usize::from(changed.slot),
                pack_room_frame(*changed),
            )?;
        }
        for changed in &frame_delta.prop_slots {
            record_update(
                &self.frame.props,
                &mut prepared.props,
                &mut prepared.dirty.props,
                usize::from(changed.slot),
                pack_prop_frame(*changed),
            )?;
        }
        for changed in &frame_delta.tank_slots {
            let slot = usize::from(changed.slot);
            if slot >= MAX_ROUND_TANK_INHABITANTS {
                return Err(MirrorDeltaError::Mirror(MirrorError::CapacityExceeded));
            }
            for subslot in 0..MAX_TANK_GLYPHS_PER_SLOT {
                let flat = slot * MAX_TANK_GLYPHS_PER_SLOT + subslot;
                record_update(
                    &self.frame.tank_cells,
                    &mut prepared.tank_cells,
                    &mut prepared.dirty.tank_cells,
                    flat,
                    pack_tank_frame(*changed, subslot),
                )?;
            }
        }
        for changed in &frame_delta.ambient_slots {
            record_update(
                &self.frame.ambient,
                &mut prepared.frame_ambient,
                &mut prepared.dirty.frame_ambient,
                usize::from(changed.slot),
                pack_ambient_frame(*changed),
            )?;
        }
        for changed in &frame_delta.analytic_slots {
            record_update(
                &self.frame.analytics,
                &mut prepared.frame_analytics,
                &mut prepared.dirty.frame_analytics,
                usize::from(changed.id.0),
                pack_analytic_frame(*changed),
            )?;
        }
        for (slot, changed) in &frame_delta.lights {
            record_update(
                &self.frame.lights,
                &mut prepared.lights,
                &mut prepared.dirty.lights,
                usize::from(*slot),
                pack_light(usize::from(*slot), Some(*changed)),
            )?;
        }
        Ok(prepared)
    }

    fn prepare_node_updates(
        &self,
        frame_delta: &FrameDelta,
        prepared: &mut PreparedMirrorDelta,
    ) -> Result<(), MirrorDeltaError> {
        if frame_delta.nodes.is_empty() {
            return Ok(());
        }
        let mut overlay = [None; MAX_SCENE_NODES];
        for changed in &frame_delta.nodes {
            let dense = usize::try_from(
                self.index
                    .node_offset(changed.node)
                    .ok_or(MirrorDeltaError::Validation(
                        crate::presentation::companion_scene::validate::SceneValidationError::NodeSlotOutOfBounds,
                    ))?,
            )
            .map_err(|_| MirrorDeltaError::Compile(CompileError::CapacityExceeded))?;
            if overlay[dense].replace(*changed).is_some() {
                return Err(MirrorDeltaError::Validation(
                    crate::presentation::companion_scene::validate::SceneValidationError::DuplicateSlot,
                ));
            }
        }

        let current = self.accepted.frame().frame();
        let mut worlds =
            [crate::presentation::companion_scene::scene::Mat4::IDENTITY; MAX_SCENE_NODES];
        let mut visibility = [false; MAX_SCENE_NODES];
        let mut opacity = [0.0; MAX_SCENE_NODES];
        let mut affected = [false; MAX_SCENE_NODES];
        for dense in self.topology.parent_before_child[..self.topology.count]
            .iter()
            .map(|dense| usize::from(*dense))
        {
            let descriptor = self.nodes[dense];
            let (parent_world, parent_visible, parent_opacity, parent_affected) =
                if descriptor.parent_dense_index == NONE_U32 {
                    (
                        crate::presentation::companion_scene::scene::Mat4::IDENTITY,
                        true,
                        1.0,
                        false,
                    )
                } else {
                    let parent = usize::try_from(descriptor.parent_dense_index)
                        .map_err(|_| MirrorDeltaError::Compile(CompileError::CapacityExceeded))?;
                    (
                        worlds[parent],
                        visibility[parent],
                        opacity[parent],
                        affected[parent],
                    )
                };
            affected[dense] = overlay[dense].is_some() || parent_affected;
            if !affected[dense] {
                let existing = self.frame.nodes.as_slice()[dense];
                worlds[dense] =
                    crate::presentation::companion_scene::scene::Mat4 { columns: existing.world };
                visibility[dense] = existing.visible != 0;
                opacity[dense] = existing.opacity;
                continue;
            }

            let state = overlay[dense].unwrap_or(current.nodes[dense]);
            let local = crate::presentation::companion_scene::scene::Mat4 {
                columns: descriptor.base_transform,
            } * state
                .local_transform
                .matrix()
                .map_err(|_| MirrorDeltaError::Compile(CompileError::InvalidTransform))?;
            worlds[dense] = parent_world * local;
            visibility[dense] = parent_visible && state.visible;
            opacity[dense] = parent_opacity * state.opacity * descriptor.depth_cue[2];
            if !worlds[dense]
                .columns
                .iter()
                .flatten()
                .all(|value| value.is_finite())
                || !opacity[dense].is_finite()
            {
                return Err(MirrorDeltaError::Compile(CompileError::InvalidTransform));
            }
            #[cfg(test)]
            {
                prepared.node_resolves += 1;
            }
            let mut value = self.frame.nodes.as_slice()[dense];
            value.world = worlds[dense].columns;
            value.opacity = opacity[dense];
            value.visible = u32::from(visibility[dense]);
            record_update(
                &self.frame.nodes,
                &mut prepared.nodes,
                &mut prepared.dirty.nodes,
                dense,
                value,
            )?;
        }
        Ok(())
    }

    fn commit_mirror_delta(&mut self, prepared: PreparedMirrorDelta) {
        if let Some(value) = prepared.content_globals {
            self.content.globals.set_fixed(0, value);
        }
        commit_updates(&mut self.content.pet_body, prepared.pet_body);
        commit_updates(&mut self.content.pet_particles, prepared.pet_particles);
        commit_updates(&mut self.content.room_glyphs, prepared.room_content);
        commit_updates(&mut self.content.prop_glyphs, prepared.prop_glyphs);
        commit_updates(&mut self.content.tank_glyphs, prepared.tank_glyphs);
        commit_updates(&mut self.content.ambient, prepared.content_ambient);
        commit_updates(&mut self.content.analytics, prepared.content_analytics);
        if let Some(value) = prepared.frame_globals {
            self.frame.globals.set_fixed(0, value);
        }
        commit_updates(&mut self.frame.nodes, prepared.nodes);
        commit_updates(&mut self.frame.room_glyphs, prepared.room_frame);
        commit_updates(&mut self.frame.props, prepared.props);
        commit_updates(&mut self.frame.tank_cells, prepared.tank_cells);
        commit_updates(&mut self.frame.ambient, prepared.frame_ambient);
        commit_updates(&mut self.frame.analytics, prepared.frame_analytics);
        commit_updates(&mut self.frame.lights, prepared.lights);
    }
}

fn record_single<T: Pod + Zeroable + Copy + PartialEq>(
    mirror: &FixedPodMirror<T, 1>,
    prepared: &mut Option<T>,
    dirty: &mut DirtySpanSet,
    value: T,
) {
    if mirror.as_slice()[0] != value {
        *prepared = Some(value);
        dirty.insert(ByteSpan::slots::<T>(0, 1));
    }
}

fn record_update<T: Pod + Zeroable + Copy + PartialEq, const N: usize>(
    mirror: &FixedPodMirror<T, N>,
    prepared: &mut [Option<T>; N],
    dirty: &mut DirtySpanSet,
    slot: usize,
    value: T,
) -> Result<(), MirrorDeltaError> {
    if slot >= N {
        return Err(MirrorDeltaError::Mirror(MirrorError::CapacityExceeded));
    }
    if mirror.as_slice()[slot] != value {
        prepared[slot] = Some(value);
        dirty.insert(ByteSpan::slots::<T>(slot, 1));
    }
    Ok(())
}

fn commit_updates<T: Pod + Zeroable + Copy, const N: usize>(
    mirror: &mut FixedPodMirror<T, N>,
    updates: [Option<T>; N],
) {
    for (slot, value) in updates.into_iter().enumerate() {
        if let Some(value) = value {
            mirror.set_fixed(slot, value);
        }
    }
}

fn pack_pet_content(
    value: crate::presentation::companion_scene::scene::PetArtSlot,
    filter: PetArtFilter,
) -> ContentGpuValue {
    ContentGpuValue {
        kind: 1,
        glyph_scalar: if filter.includes(value.palette_role) {
            option_glyph(value.glyph.map(|glyph| glyph.as_char()))
        } else {
            NONE_U32
        },
        slot: u32::from(value.slot),
        subslot: 0,
        signed_data: [0; 2],
        flags: palette_role_tag(value.palette_role),
        variant: 0,
    }
}

fn pack_prop_content(
    outer: crate::presentation::companion_scene::scene::PropContentSlot,
    paint: Option<crate::presentation::companion_scene::scene::GlyphPaintSource>,
    subslot: usize,
) -> ContentGpuValue {
    let (glyph_scalar, local_cell, semantic_flags) = match outer.content {
        Some(value) => {
            let glyph = value.glyphs[subslot];
            (
                option_glyph(glyph.glyph.map(|glyph| glyph.as_char())),
                glyph.local_cell.map(i32::from),
                option_bool(value.twinkle_active)
                    | (option_bool(value.lid_open) << 2)
                    | (option_bool(value.bloom_active) << 4),
            )
        }
        None => (NONE_U32, [0; 2], 0),
    };
    let phase_flags = outer
        .content
        .and_then(|value| value.sprite_phase)
        .map_or(0, |phase| (u32::from(phase) + 1) << 8);
    let (paint_flags, variant) = paint.map_or((0, 0), |paint| {
        (
            1 << 6,
            pack_rgba8([
                paint.color_srgb8[0],
                paint.color_srgb8[1],
                paint.color_srgb8[2],
                255,
            ]),
        )
    });
    ContentGpuValue {
        kind: 2,
        glyph_scalar,
        slot: u32::from(outer.slot),
        subslot: u32::try_from(subslot).expect("fixed prop subslot fits u32"),
        signed_data: local_cell,
        flags: semantic_flags | paint_flags | phase_flags,
        variant,
    }
}

fn pack_room_content(
    value: crate::presentation::companion_scene::scene::RoomGlyphContentSlot,
) -> ContentGpuValue {
    let (flags, variant) = value.color_srgb8.map_or((0, 0), |color| {
        (1, pack_rgba8([color[0], color[1], color[2], 255]))
    });
    ContentGpuValue {
        kind: 5,
        glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
        slot: u32::from(value.slot),
        subslot: 0,
        signed_data: [0; 2],
        flags,
        variant,
    }
}

fn pack_tank_content(
    outer: crate::presentation::companion_scene::scene::TankContentSlot,
    subslot: usize,
) -> ContentGpuValue {
    let (glyph_scalar, signed_data, flags, variant) = match outer.content {
        Some(value) => (
            option_glyph(value.glyphs[subslot].map(|glyph| glyph.as_char())),
            pack_tank_paint(value.color_srgb8, value.bold),
            value.morph.map_or(NONE_U32, u32::from),
            u32::from(value.sprite_variant),
        ),
        None => (NONE_U32, [0; 2], NONE_U32, NONE_U32),
    };
    ContentGpuValue {
        kind: 3,
        glyph_scalar,
        slot: u32::from(outer.slot),
        subslot: u32::try_from(subslot).expect("fixed tank subslot fits u32"),
        signed_data,
        flags,
        variant,
    }
}

const fn pack_tank_paint(color_srgb8: [u8; 3], bold: bool) -> [i32; 2] {
    [
        color_srgb8[0] as i32 | ((color_srgb8[1] as i32) << 8) | ((color_srgb8[2] as i32) << 16),
        bold as i32,
    ]
}

fn pack_ambient_content(
    value: crate::presentation::companion_scene::scene::AmbientContentSlot,
    paint: Option<crate::presentation::companion_scene::scene::GlyphPaintSource>,
) -> ContentGpuValue {
    let kind_flags = value.kind.map_or(0, ambient_kind_tag);
    let (paint_flags, variant) = paint.map_or((0, 0), |paint| {
        (
            1 << 8,
            pack_rgba8([
                paint.color_srgb8[0],
                paint.color_srgb8[1],
                paint.color_srgb8[2],
                255,
            ]),
        )
    });
    ContentGpuValue {
        kind: 4,
        glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
        slot: u32::from(value.slot),
        subslot: 0,
        signed_data: [0; 2],
        flags: kind_flags | paint_flags,
        variant,
    }
}

const fn pack_rgb8(value: [u8; 3]) -> u32 {
    value[0] as u32 | ((value[1] as u32) << 8) | ((value[2] as u32) << 16)
}

const fn pack_rgba8(value: [u8; 4]) -> u32 {
    pack_rgb8([value[0], value[1], value[2]]) | ((value[3] as u32) << 24)
}

fn pack_analytic_content(value: AnalyticContentSlot) -> AnalyticContentGpuValue {
    let Some(content) = value.value else {
        return AnalyticContentGpuValue {
            id: u32::from(value.id.0),
            semantic: NONE_U32,
            shape: NONE_U32,
            flags: 0,
            payload: [0; 8],
        };
    };
    let mut payload = [0; 8];
    match content.paint {
        AnalyticPaint::ApertureDepth {
            core_srgb8,
            rim_srgb8,
            bed_srgb8,
            fleck_srgb8,
        } => {
            payload[0] = pack_rgb8(core_srgb8);
            payload[1] = pack_rgb8(rim_srgb8);
            payload[2] = pack_rgb8(bed_srgb8);
            payload[3] = pack_rgb8(fleck_srgb8);
        }
        AnalyticPaint::PetShadowTint { color_srgb8, opacity_u8 } => {
            payload[0] = pack_rgb8(color_srgb8);
            payload[1] = u32::from(opacity_u8);
        }
        AnalyticPaint::FloorShadowMultiplySilhouette { color_srgba8 } => {
            payload[0] = pack_rgba8(color_srgba8);
        }
        AnalyticPaint::StatusBeacon { active_srgba8, calm_srgba8 } => {
            payload[0] = pack_rgba8(active_srgba8);
            payload[1] = pack_rgba8(calm_srgba8);
        }
        AnalyticPaint::PerimeterGaugeSet {
            xp,
            daily,
            pace,
            daily_overage_srgba8,
            daily_rollover_contract_unorm8,
        } => {
            for (offset, lane) in [xp, daily, pace].into_iter().enumerate() {
                payload[offset * 2] = pack_rgba8(lane.track_srgba8);
                payload[offset * 2 + 1] = pack_rgba8(lane.fill_srgba8);
            }
            payload[6] = pack_rgba8(daily_overage_srgba8);
            payload[7] = pack_rgba8(daily_rollover_contract_unorm8);
        }
        AnalyticPaint::TroubleBeacon { color_srgba8 } => {
            payload[0] = pack_rgba8(color_srgba8);
        }
        AnalyticPaint::DimOverlay { color_srgb8 } => {
            payload[0] = pack_rgb8(color_srgb8);
        }
        AnalyticPaint::PropShadowMultiply { color_srgb8 } => {
            payload[0] = pack_rgb8(color_srgb8);
        }
    }
    AnalyticContentGpuValue {
        id: u32::from(value.id.0),
        semantic: analytic_semantic_tag(content.semantic),
        shape: analytic_shape_tag(content.shape),
        flags: 1,
        payload,
    }
}

fn pack_analytic_frame(value: AnalyticFrameSlot) -> AnalyticFrameGpuValue {
    let Some(frame) = value.value else {
        return AnalyticFrameGpuValue {
            id: u32::from(value.id.0),
            semantic: NONE_U32,
            shape: NONE_U32,
            flags: 0,
            rect_points: [0.0; 4],
            payload: [0.0; 16],
        };
    };
    let mut flags = 1;
    let mut payload = [0.0; 16];
    match frame.geometry {
        AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points,
        } => payload[..4].copy_from_slice(&[
            center_points[0],
            center_points[1],
            radius_points,
            feather_points,
        ]),
        AnalyticGeometry::PetSilhouette { mask, offset_points, softness_points } => {
            payload[..4].copy_from_slice(&[
                analytic_mask_tag(mask) as f32,
                offset_points[0],
                offset_points[1],
                softness_points,
            ]);
        }
        AnalyticGeometry::PetFloorProjection { mask, facing } => {
            payload[..2].copy_from_slice(&[analytic_mask_tag(mask) as f32, f32::from(facing)]);
        }
        AnalyticGeometry::StatusBeacon {
            center_points,
            radius_points,
            thickness_points,
            tone,
        } => payload[..5].copy_from_slice(&[
            center_points[0],
            center_points[1],
            radius_points,
            thickness_points,
            status_beacon_tone_tag(tone) as f32,
        ]),
        AnalyticGeometry::PerimeterGaugeSet { center_points, xp, daily, pace } => {
            payload[..2].copy_from_slice(&center_points);
            for (index, lane) in [xp, daily, pace].into_iter().enumerate() {
                let offset = 2 + index * 4;
                payload[offset..offset + 4].copy_from_slice(&[
                    lane.radius_points,
                    lane.stroke_width_points,
                    lane.track_start_degrees,
                    lane.track_sweep_degrees,
                ]);
                flags |= gauge_cap_tag(lane.cap) << (8 + index * 4);
            }
        }
        AnalyticGeometry::TroubleBeacon {
            center_points,
            radius_points,
            thickness_points,
        } => payload[..4].copy_from_slice(&[
            center_points[0],
            center_points[1],
            radius_points,
            thickness_points,
        ]),
        AnalyticGeometry::SurfaceOverlay | AnalyticGeometry::PropShadowField => {}
    }
    AnalyticFrameGpuValue {
        id: u32::from(value.id.0),
        semantic: analytic_semantic_tag(frame.semantic),
        shape: analytic_shape_tag(frame.shape),
        flags,
        rect_points: frame.rect_points,
        payload,
    }
}

fn pack_room_frame(
    value: crate::presentation::companion_scene::scene::RoomGlyphFrameSlot,
) -> FrameGpuValue {
    FrameGpuValue {
        kind: 4,
        slot: u32::from(value.slot),
        flags: u32::from(value.visible),
        variant: 0,
        values: [
            f32::from(value.grid_cell[0]),
            f32::from(value.grid_cell[1]),
            value.position_points[0],
            value.position_points[1],
            value.opacity,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
    }
}

fn pack_prop_frame(
    value: crate::presentation::companion_scene::scene::PropFrameSlot,
) -> FrameGpuValue {
    FrameGpuValue {
        kind: 1,
        slot: u32::from(value.slot),
        flags: u32::from(value.visible),
        variant: 0,
        values: [
            value.origin_points[0],
            value.origin_points[1],
            value.motion_offset_points[0],
            value.motion_offset_points[1],
            value.opacity,
            value.footprint_points[0],
            value.footprint_points[1],
            value.contact_shadow_strength,
            value.cast_shadow_vector_points[0],
            value.cast_shadow_vector_points[1],
            value.cast_shadow_softness_points,
            value.cast_shadow_strength,
        ],
    }
}

fn pack_tank_frame(
    outer: crate::presentation::companion_scene::scene::TankFrameSlot,
    subslot: usize,
) -> FrameGpuValue {
    let cell = outer.cells[subslot];
    FrameGpuValue {
        kind: 2,
        slot: u32::from(outer.slot),
        flags: u32::from(outer.visible) | (u32::from(cell.visible) << 1),
        variant: instance_layer_tag(cell.layer)
            | (u32::try_from(subslot).expect("fixed tank subslot fits u32") << 16),
        values: [
            outer.origin_points[0],
            outer.origin_points[1],
            cell.position_points[0],
            cell.position_points[1],
            cell.bounds_points[0],
            cell.bounds_points[1],
            cell.bounds_points[2],
            cell.bounds_points[3],
            0.0,
            0.0,
            0.0,
            0.0,
        ],
    }
}

fn pack_ambient_frame(
    value: crate::presentation::companion_scene::scene::AmbientFrameSlot,
) -> FrameGpuValue {
    FrameGpuValue {
        kind: 3,
        slot: u32::from(value.slot),
        flags: u32::from(value.visible),
        variant: 0,
        values: [
            value.position_points[0],
            value.position_points[1],
            value.opacity,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
    }
}

fn pack_light(
    slot: usize,
    value: Option<crate::presentation::companion_scene::scene::LightFrame>,
) -> FrameGpuValue {
    match value {
        Some(value) => FrameGpuValue {
            kind: 5,
            slot: u32::try_from(slot).expect("fixed light slot fits u32"),
            flags: 1,
            variant: 0,
            values: [
                value.direction[0],
                value.direction[1],
                value.direction[2],
                value.color_linear[0],
                value.color_linear[1],
                value.color_linear[2],
                value.intensity,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
        },
        None => FrameGpuValue {
            kind: 5,
            slot: u32::try_from(slot).expect("fixed light slot fits u32"),
            flags: 0,
            variant: NONE_U32,
            values: [0.0; 12],
        },
    }
}

#[allow(dead_code)] // Called when retained-scene materialization lands in the next checkpoint.
pub(super) fn compile_cpu_generation(
    generation: &SceneGenerationData,
) -> Result<CpuSceneCandidate, CompileError> {
    compile_cpu_parts_with_private(
        generation.generation_key(),
        generation.source_revisions(),
        generation.template(),
        generation.content(),
        generation.frame(),
        generation.accepted_state().clone(),
        PrivateSpatialFrame::from_snapshot(generation.source_snapshot()),
    )
}

#[cfg(test)]
fn compile_cpu_parts(
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    template: &SceneTemplate,
    content: &SceneContent,
    frame: &SceneFrame,
    accepted: crate::presentation::companion_scene::validate::AcceptedSceneState,
) -> Result<CpuSceneCandidate, CompileError> {
    compile_cpu_parts_with_private(
        generation_key,
        source_revisions,
        template,
        content,
        frame,
        accepted,
        PrivateSpatialFrame::NEUTRAL,
    )
}

fn compile_cpu_parts_with_private(
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    template: &SceneTemplate,
    content: &SceneContent,
    frame: &SceneFrame,
    accepted: crate::presentation::companion_scene::validate::AcceptedSceneState,
    private_spatial_frame: PrivateSpatialFrame,
) -> Result<CpuSceneCandidate, CompileError> {
    let logical_content = content.clone();
    if template.nodes.len() > MAX_SCENE_NODES
        || template.primitives.len() > MAX_STATIC_PRIMITIVES
        || content.pet_art_slots.len() != MAX_PET_ART_SLOTS
        || content.room_glyph_slots.len() != MAX_ROOM_GLYPH_SLOTS
        || content.prop_slots.len() != MAX_VISIBLE_PROPS
        || content.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || content.ambient_slots.len() != MAX_AMBIENT_INSTANCES
        || content.prop_paint_slots.len() != MAX_VISIBLE_PROPS
        || content.ambient_paint_slots.len() != MAX_AMBIENT_INSTANCES
        || content.analytic_slots.len() != MAX_ANALYTIC_PARAMS
        || frame.nodes.len() > MAX_SCENE_NODES
        || frame.room_glyph_slots.len() != MAX_ROOM_GLYPH_SLOTS
        || frame.prop_slots.len() != MAX_VISIBLE_PROPS
        || frame.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || frame.ambient_slots.len() != MAX_AMBIENT_INSTANCES
        || frame.analytic_slots.len() != MAX_ANALYTIC_PARAMS
        || frame.lights.len() > MAX_LIGHTS
        || template.attachments.len() > MAX_ATTACHMENTS
    {
        return Err(CompileError::CapacityExceeded);
    }

    let index = DenseSceneIndex {
        nodes: template
            .nodes
            .iter()
            .enumerate()
            .map(|(offset, node)| Ok((node.id, u32_index(offset)?)))
            .collect::<Result<Vec<_>, CompileError>>()?,
        materials: template
            .materials
            .iter()
            .enumerate()
            .map(|(offset, material)| Ok((material.id, u32_index(offset)?)))
            .collect::<Result<Vec<_>, CompileError>>()?,
        resources: template
            .resources
            .iter()
            .enumerate()
            .map(|(offset, resource)| Ok((resource.id, u32_index(offset)?)))
            .collect::<Result<Vec<_>, CompileError>>()?,
        attachments: template
            .attachments
            .iter()
            .enumerate()
            .map(|(offset, attachment)| Ok((attachment.id, u32_index(offset)?)))
            .collect::<Result<Vec<_>, CompileError>>()?,
    };
    let topology = compile_node_topology(template, &index)?;
    let nodes = compile_node_descriptors(template, &index)?;
    let materials = template
        .materials
        .iter()
        .map(|material| CpuMaterialDescriptor {
            material_id: material.id.0,
            kind: material_kind_tag(material.kind),
        })
        .collect::<Vec<_>>();
    let resources = template
        .resources
        .iter()
        .map(|resource| CpuResourceDescriptor {
            resource_id: resource.id.0,
            kind: resource_kind_tag(resource.kind),
        })
        .collect::<Vec<_>>();
    let material_kinds = template
        .materials
        .iter()
        .map(|material| (material.id, material.kind))
        .collect::<Vec<_>>();

    let vertex_capacity = template
        .primitives
        .len()
        .checked_mul(4)
        .ok_or(CompileError::CapacityExceeded)?;
    let index_capacity = template
        .primitives
        .len()
        .checked_mul(6)
        .ok_or(CompileError::CapacityExceeded)?;
    let mut vertices = Vec::with_capacity(vertex_capacity);
    let mut indices = Vec::with_capacity(index_capacity);
    let mut primitives = Vec::with_capacity(template.primitives.len());
    let mut opaque_cutout = Vec::new();
    let mut world_blended_unsorted = Vec::new();
    let mut chrome_authored = Vec::new();

    for (primitive_offset, primitive) in template.primitives.iter().enumerate() {
        let primitive_index = u32_index(primitive_offset)?;
        let node_dense_index = index
            .node_offset(primitive.node)
            .ok_or(CompileError::MissingNode)?;
        let material_dense_index = index
            .material_offset(primitive.material)
            .ok_or(CompileError::MissingMaterial)?;
        let material_kind = material_kinds
            [usize::try_from(material_dense_index).map_err(|_| CompileError::CapacityExceeded)?]
        .1;
        let (resource_id, resource_dense_index) = match primitive.resource {
            Some(id) => (
                id.0,
                index
                    .resource_offset(id)
                    .ok_or(CompileError::MissingResource)?,
            ),
            None => (NONE_U32, NONE_U32),
        };
        let first_vertex = u32_index(vertices.len())?;
        let first_index = u32_index(indices.len())?;
        append_local_bounds_quad(
            &mut vertices,
            &mut indices,
            primitive.local_geometry,
            primitive_index,
            material_dense_index,
            first_vertex,
        );
        let (instance_group, instance_slot) = instance_group_tags(primitive.binding);
        primitives.push(CpuPrimitiveDescriptor {
            node_id: primitive.node.0,
            node_dense_index,
            material_id: primitive.material.0,
            material_dense_index,
            resource_id,
            resource_dense_index,
            primitive_kind: primitive_kind_tag(primitive.kind),
            blend: blend_tag(primitive.blend),
            depth: depth_tag(primitive.depth),
            instance_group,
            instance_slot,
            authored_order: u32::from(primitive.authored_order),
            first_vertex,
            first_index,
            index_count: 6,
            space: space_tag(primitive.space),
            local_bounds_min: primitive.local_geometry.min,
            local_bounds_max: primitive.local_geometry.max,
            _bounds_padding: [0; 2],
        });

        if material_kind == MaterialKind::ScreenChrome {
            chrome_authored.push(primitive_index);
        } else if is_world_blended(primitive.blend, Some(material_kind)) {
            world_blended_unsorted.push(primitive_index);
        } else {
            opaque_cutout.push(primitive_index);
        }
    }
    chrome_authored.sort_by_key(|primitive_index| {
        (
            template.primitives
                [usize::try_from(*primitive_index).expect("primitive index originated from usize")]
            .authored_order,
            *primitive_index,
        )
    });
    if world_blended_unsorted.len() > MAX_BLENDED_DRAWS {
        return Err(CompileError::CapacityExceeded);
    }
    let blended_draw_templates =
        compile_blended_draw_templates(&world_blended_unsorted, &primitives)?;

    let attachments = compile_attachment_descriptors(template, &index)?;

    let content = compile_content_mirrors(template, content)?;
    let frame = compile_frame_mirrors(template, frame, &index, private_spatial_frame)?;
    let phases = PhaseLists {
        opaque_cutout,
        world_blended_unsorted,
        chrome_authored,
    };
    let static_checksum = checksum_static(StaticChecksumInputs {
        vertices: &vertices,
        indices: &indices,
        nodes: &nodes,
        materials: &materials,
        resources: &resources,
        primitives: &primitives,
        attachments: &attachments,
        phases: &phases,
    });
    Ok(CpuSceneCandidate {
        instance_identity: CandidateInstanceIdentity::new(),
        generation_key,
        source_revisions,
        static_checksum,
        index,
        vertices,
        indices,
        nodes,
        materials,
        resources,
        primitives,
        attachments,
        phases,
        blended_draw_templates,
        content,
        frame,
        logical_content,
        accepted,
        topology,
        private_spatial_frame,
        #[cfg(test)]
        last_node_resolves: 0,
    })
}

fn compile_blended_draw_templates(
    primitive_indices: &[u32],
    primitives: &[CpuPrimitiveDescriptor],
) -> Result<BlendedDrawTemplates, CompileError> {
    if primitive_indices.len() > MAX_BLENDED_DRAWS {
        return Err(CompileError::CapacityExceeded);
    }
    let mut records = [BlendedDrawTemplate::EMPTY; MAX_BLENDED_DRAWS];
    for (draw_index, primitive_index) in primitive_indices.iter().copied().enumerate() {
        let primitive = primitives
            .get(usize::try_from(primitive_index).map_err(|_| CompileError::CapacityExceeded)?)
            .ok_or(CompileError::CapacityExceeded)?;
        records[draw_index] = BlendedDrawTemplate::new(
            u16::try_from(draw_index).map_err(|_| CompileError::CapacityExceeded)?,
            u16::try_from(primitive.node_dense_index)
                .map_err(|_| CompileError::CapacityExceeded)?,
            primitive.authored_order,
            u16::try_from(primitive_index).map_err(|_| CompileError::CapacityExceeded)?,
        );
    }
    Ok(BlendedDrawTemplates {
        records,
        len: u16::try_from(primitive_indices.len()).map_err(|_| CompileError::CapacityExceeded)?,
    })
}

fn compile_node_topology(
    template: &SceneTemplate,
    index: &DenseSceneIndex,
) -> Result<FixedNodeTopology, CompileError> {
    let mut topology = FixedNodeTopology {
        count: 0,
        parent_before_child: [0; MAX_SCENE_NODES],
    };
    let mut state = [0_u8; MAX_SCENE_NODES];
    for dense in 0..template.nodes.len() {
        append_topological_node(dense, template, index, &mut state, &mut topology)?;
    }
    Ok(topology)
}

fn append_topological_node(
    dense: usize,
    template: &SceneTemplate,
    index: &DenseSceneIndex,
    state: &mut [u8; MAX_SCENE_NODES],
    topology: &mut FixedNodeTopology,
) -> Result<(), CompileError> {
    match state[dense] {
        2 => return Ok(()),
        1 => return Err(CompileError::HierarchyCycle),
        _ => state[dense] = 1,
    }
    if let Some(parent) = template.nodes[dense].parent {
        let parent_dense =
            usize::try_from(index.node_offset(parent).ok_or(CompileError::MissingNode)?)
                .map_err(|_| CompileError::CapacityExceeded)?;
        append_topological_node(parent_dense, template, index, state, topology)?;
    }
    topology.parent_before_child[topology.count] =
        u16::try_from(dense).map_err(|_| CompileError::CapacityExceeded)?;
    topology.count += 1;
    state[dense] = 2;
    Ok(())
}

fn compile_node_descriptors(
    template: &SceneTemplate,
    index: &DenseSceneIndex,
) -> Result<Vec<CpuNodeDescriptor>, CompileError> {
    template
        .nodes
        .iter()
        .map(|node| {
            let (parent_id, parent_dense_index) = match node.parent {
                Some(parent) => (
                    parent.0,
                    index.node_offset(parent).ok_or(CompileError::MissingNode)?,
                ),
                None => (NONE_U32, NONE_U32),
            };
            Ok(CpuNodeDescriptor {
                node_id: node.id.0,
                parent_id,
                parent_dense_index,
                _header_padding: 0,
                base_transform: node
                    .base_transform
                    .matrix()
                    .map_err(|_| CompileError::InvalidTransform)?
                    .columns,
                local_bounds_min: node.local_bounds.min,
                local_bounds_max: node.local_bounds.max,
                depth_cue: [
                    node.depth_cue.scale,
                    node.depth_cue.y_offset_points_up,
                    node.depth_cue.opacity,
                    node.depth_cue.saturation,
                ],
            })
        })
        .collect()
}

fn compile_attachment_descriptors(
    template: &SceneTemplate,
    index: &DenseSceneIndex,
) -> Result<Vec<CpuAttachmentDescriptor>, CompileError> {
    template
        .attachments
        .iter()
        .map(|attachment| {
            let attachment_dense_index = index
                .attachment_offset(attachment.id)
                .ok_or(CompileError::MissingNode)?;
            let owner_dense_index = index
                .node_offset(attachment.owner)
                .ok_or(CompileError::MissingNode)?;
            let (instance_binding, instance_slot, source_primitive, source_node) =
                match attachment.instance_binding {
                    None => (0, NONE_U32, NONE_U32, NONE_U32),
                    Some(AttachmentInstanceBinding::PropGlyphs(slot)) => {
                        let mut matches =
                            template
                                .primitives
                                .iter()
                                .enumerate()
                                .filter(|(_, primitive)| {
                                    primitive.binding
                                        == PrimitiveBinding::Instances(
                                            InstanceGroupBinding::PropGlyphs(slot),
                                        )
                                });
                        let (source_primitive, primitive) = matches
                            .next()
                            .ok_or(CompileError::MissingAttachmentSource)?;
                        if matches.next().is_some() {
                            return Err(CompileError::AmbiguousAttachmentSource);
                        }
                        (
                            1,
                            u32::from(slot),
                            u32_index(source_primitive)?,
                            index
                                .node_offset(primitive.node)
                                .ok_or(CompileError::MissingNode)?,
                        )
                    }
                };
            Ok(CpuAttachmentDescriptor {
                attachment_id: attachment.id.0,
                attachment_dense_index,
                owner_id: attachment.owner.0,
                owner_dense_index,
                local_transform: attachment
                    .local
                    .matrix()
                    .map_err(|_| CompileError::InvalidTransform)?
                    .columns,
                mode: attachment_mode_tag(attachment.mode),
                instance_binding,
                instance_slot,
                source_primitive_dense_index: source_primitive,
                source_node_dense_index: source_node,
            })
        })
        .collect()
}

fn append_local_bounds_quad(
    vertices: &mut Vec<StaticVertex>,
    indices: &mut Vec<StaticIndex>,
    bounds: Bounds3,
    primitive_index: u32,
    material_index: u32,
    first: u32,
) {
    // Checkpoint A emits one provisional front-face quad at min Z. The raw
    // primitive descriptor retains the complete Bounds3 so shallow-card side
    // expansion can replace this geometry without losing authored thickness.
    let [min_x, min_y, min_z] = bounds.min;
    let [max_x, max_y, _max_z] = bounds.max;
    for (local_position, uv) in [
        ([min_x, min_y, min_z], [0.0, 0.0]),
        ([max_x, min_y, min_z], [1.0, 0.0]),
        ([max_x, max_y, min_z], [1.0, 1.0]),
        ([min_x, max_y, min_z], [0.0, 1.0]),
    ] {
        vertices.push(StaticVertex {
            local_position,
            uv,
            normal: [0.0, 0.0, 1.0],
            primitive_index,
            material_index,
        });
    }
    indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
}

fn compile_content_mirrors(
    template: &SceneTemplate,
    content: &SceneContent,
) -> Result<ContentMirrors, CompileError> {
    let mut result = ContentMirrors::zeroed();
    set_all(
        &mut result.globals,
        [ContentGlobalsGpuValue {
            palette_rgba: content
                .palette
                .map(|rgb| [u32::from(rgb[0]), u32::from(rgb[1]), u32::from(rgb[2]), 255]),
            mood: mood_tag(content.mood),
            weather: weather_tag(content.weather),
            glyph_grid_dimensions: [
                u32::from(template.glyph_grid.columns),
                u32::from(template.glyph_grid.rows),
            ],
            glyph_grid_origin_points: template.glyph_grid.y_up_origin_points,
            glyph_cell_extent_points: template.glyph_grid.cell_extent_points,
            pet_rim_srgba8: mood_rim_srgba8(content.mood),
            _private_spatial_padding: [0; 3],
        }],
    );
    set_all(
        &mut result.pet_body,
        std::array::from_fn(|slot| {
            pack_pet_content(content.pet_art_slots[slot], PetArtFilter::Body)
        }),
    );
    set_all(
        &mut result.pet_particles,
        std::array::from_fn(|slot| {
            pack_pet_content(content.pet_art_slots[slot], PetArtFilter::Particles)
        }),
    );
    set_all(
        &mut result.room_glyphs,
        std::array::from_fn(|slot| pack_room_content(content.room_glyph_slots[slot])),
    );
    set_all(
        &mut result.prop_glyphs,
        std::array::from_fn(|flat| {
            let slot = flat / MAX_PROP_GLYPHS_PER_SLOT;
            let subslot = flat % MAX_PROP_GLYPHS_PER_SLOT;
            pack_prop_content(
                content.prop_slots[slot],
                content.prop_paint_slots[slot].paints[subslot],
                subslot,
            )
        }),
    );
    set_all(
        &mut result.tank_glyphs,
        std::array::from_fn(|flat| {
            let slot = flat / MAX_TANK_GLYPHS_PER_SLOT;
            let subslot = flat % MAX_TANK_GLYPHS_PER_SLOT;
            let outer = content.tank_slots[slot];
            let (glyph_scalar, signed_data, flags, variant) = match outer.content {
                Some(value) => (
                    option_glyph(value.glyphs[subslot].map(|glyph| glyph.as_char())),
                    pack_tank_paint(value.color_srgb8, value.bold),
                    value.morph.map_or(NONE_U32, u32::from),
                    u32::from(value.sprite_variant),
                ),
                None => (NONE_U32, [0; 2], NONE_U32, NONE_U32),
            };
            ContentGpuValue {
                kind: 3,
                glyph_scalar,
                slot: u32::from(outer.slot),
                subslot: u32::try_from(subslot).expect("fixed tank subslot fits u32"),
                signed_data,
                flags,
                variant,
            }
        }),
    );
    set_all(
        &mut result.ambient,
        std::array::from_fn(|slot| {
            pack_ambient_content(
                content.ambient_slots[slot],
                content.ambient_paint_slots[slot].paint,
            )
        }),
    );
    set_all(
        &mut result.analytics,
        std::array::from_fn(|slot| pack_analytic_content(content.analytic_slots[slot])),
    );
    Ok(result)
}

fn compile_frame_mirrors(
    template: &SceneTemplate,
    frame: &SceneFrame,
    index: &DenseSceneIndex,
    private_spatial_frame: PrivateSpatialFrame,
) -> Result<FrameMirrors, CompileError> {
    let mut result = FrameMirrors::zeroed();
    let projection = frame
        .camera
        .projection_matrix()
        .map_err(|_| CompileError::InvalidCamera)?;
    set_all(
        &mut result.globals,
        [FrameGlobalsGpuValue {
            view: crate::presentation::companion_scene::scene::Mat4::IDENTITY.columns,
            projection: projection.columns,
            viewport_points: [frame.camera.width_points, frame.camera.height_points],
            // Backing scale and aperture are host-owned and unavailable in SceneFrame.
            viewport_pixels: [0.0; 2],
            aperture: [0.0; 4],
            gauges: frame.gauges,
            dim_amount: frame.dim_amount,
            light_count: u32::try_from(frame.lights.len()).expect("fixed light count fits u32"),
            activity_opacity: private_spatial_frame.activity_opacity,
            reduce_motion: u32::from(private_spatial_frame.reduce_motion),
        }],
    );

    let mut worlds = [crate::presentation::companion_scene::scene::Mat4::IDENTITY; MAX_SCENE_NODES];
    let mut effective_visibility = [false; MAX_SCENE_NODES];
    let mut effective_opacity = [0.0; MAX_SCENE_NODES];
    let mut resolve_state = [0_u8; MAX_SCENE_NODES];
    let mut node_values = [NodeGpuValue::zeroed(); MAX_SCENE_NODES];
    for dense in 0..template.nodes.len() {
        resolve_node_world(
            dense,
            template,
            frame,
            index,
            &mut resolve_state,
            &mut worlds,
            &mut effective_visibility,
            &mut effective_opacity,
        )?;
    }
    for (dense, node) in template.nodes.iter().enumerate() {
        node_values[dense] = NodeGpuValue {
            world: worlds[dense].columns,
            opacity: effective_opacity[dense],
            visible: u32::from(effective_visibility[dense]),
            material_parameter_offset: NONE_U32,
            material_parameter_count: 0,
            depth_cue: [
                node.depth_cue.scale,
                node.depth_cue.y_offset_points_up,
                node.depth_cue.opacity,
                node.depth_cue.saturation,
            ],
        };
    }
    set_all(&mut result.nodes, node_values);
    set_all(
        &mut result.room_glyphs,
        std::array::from_fn(|slot| pack_room_frame(frame.room_glyph_slots[slot])),
    );
    set_all(
        &mut result.props,
        std::array::from_fn(|slot| pack_prop_frame(frame.prop_slots[slot])),
    );
    set_all(
        &mut result.tank_cells,
        std::array::from_fn(|flat| {
            let slot = flat / MAX_TANK_GLYPHS_PER_SLOT;
            let subslot = flat % MAX_TANK_GLYPHS_PER_SLOT;
            let outer = frame.tank_slots[slot];
            let cell = outer.cells[subslot];
            FrameGpuValue {
                kind: 2,
                slot: u32::from(outer.slot),
                flags: u32::from(outer.visible) | (u32::from(cell.visible) << 1),
                // Low 16 bits are the layer tag; high 16 bits are the cell subslot.
                variant: instance_layer_tag(cell.layer)
                    | (u32::try_from(subslot).expect("fixed tank subslot fits u32") << 16),
                values: [
                    outer.origin_points[0],
                    outer.origin_points[1],
                    cell.position_points[0],
                    cell.position_points[1],
                    cell.bounds_points[0],
                    cell.bounds_points[1],
                    cell.bounds_points[2],
                    cell.bounds_points[3],
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ],
            }
        }),
    );
    set_all(
        &mut result.ambient,
        std::array::from_fn(|slot| {
            let value = frame.ambient_slots[slot];
            FrameGpuValue {
                kind: 3,
                slot: u32::from(value.slot),
                flags: u32::from(value.visible),
                variant: 0,
                values: [
                    value.position_points[0],
                    value.position_points[1],
                    value.opacity,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ],
            }
        }),
    );
    set_all(
        &mut result.analytics,
        std::array::from_fn(|slot| pack_analytic_frame(frame.analytic_slots[slot])),
    );
    set_all(
        &mut result.lights,
        std::array::from_fn(|slot| match frame.lights.get(slot) {
            Some(value) => FrameGpuValue {
                kind: 5,
                slot: u32::try_from(slot).expect("fixed light slot fits u32"),
                flags: 1,
                variant: 0,
                values: [
                    value.direction[0],
                    value.direction[1],
                    value.direction[2],
                    value.color_linear[0],
                    value.color_linear[1],
                    value.color_linear[2],
                    value.intensity,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ],
            },
            None => FrameGpuValue {
                kind: 5,
                slot: u32::try_from(slot).expect("fixed light slot fits u32"),
                flags: 0,
                variant: NONE_U32,
                values: [0.0; 12],
            },
        }),
    );
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn resolve_node_world(
    dense: usize,
    template: &SceneTemplate,
    frame: &SceneFrame,
    index: &DenseSceneIndex,
    state: &mut [u8; MAX_SCENE_NODES],
    worlds: &mut [crate::presentation::companion_scene::scene::Mat4; MAX_SCENE_NODES],
    visibility: &mut [bool; MAX_SCENE_NODES],
    opacity: &mut [f32; MAX_SCENE_NODES],
) -> Result<(), CompileError> {
    match state[dense] {
        2 => return Ok(()),
        1 => return Err(CompileError::HierarchyCycle),
        _ => state[dense] = 1,
    }
    let node = &template.nodes[dense];
    let dynamic = frame
        .nodes
        .iter()
        .find(|candidate| candidate.node == node.id)
        .ok_or(CompileError::MissingFrameNode)?;
    let local = node
        .base_transform
        .matrix()
        .map_err(|_| CompileError::InvalidTransform)?
        * dynamic
            .local_transform
            .matrix()
            .map_err(|_| CompileError::InvalidTransform)?;
    let (parent_world, parent_visible, parent_opacity) = match node.parent {
        Some(parent) => {
            let parent_dense =
                usize::try_from(index.node_offset(parent).ok_or(CompileError::MissingNode)?)
                    .map_err(|_| CompileError::CapacityExceeded)?;
            resolve_node_world(
                parent_dense,
                template,
                frame,
                index,
                state,
                worlds,
                visibility,
                opacity,
            )?;
            (
                worlds[parent_dense],
                visibility[parent_dense],
                opacity[parent_dense],
            )
        }
        None => (
            crate::presentation::companion_scene::scene::Mat4::IDENTITY,
            true,
            1.0,
        ),
    };
    worlds[dense] = parent_world * local;
    visibility[dense] = parent_visible && dynamic.visible;
    opacity[dense] = parent_opacity * dynamic.opacity * node.depth_cue.opacity;
    if !worlds[dense]
        .columns
        .iter()
        .flatten()
        .all(|value| value.is_finite())
        || !opacity[dense].is_finite()
    {
        return Err(CompileError::InvalidTransform);
    }
    state[dense] = 2;
    Ok(())
}

fn set_all<T: Pod + Zeroable + Copy, const N: usize>(
    mirror: &mut FixedPodMirror<T, N>,
    values: [T; N],
) {
    *mirror = FixedPodMirror::from_array(values);
}

fn u32_index(value: usize) -> Result<u32, CompileError> {
    u32::try_from(value).map_err(|_| CompileError::CapacityExceeded)
}

struct StaticChecksumInputs<'a> {
    vertices: &'a [StaticVertex],
    indices: &'a [StaticIndex],
    nodes: &'a [CpuNodeDescriptor],
    materials: &'a [CpuMaterialDescriptor],
    resources: &'a [CpuResourceDescriptor],
    primitives: &'a [CpuPrimitiveDescriptor],
    attachments: &'a [CpuAttachmentDescriptor],
    phases: &'a PhaseLists,
}

fn checksum_static(input: StaticChecksumInputs<'_>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_domain(&mut hash, 1, bytemuck::cast_slice(input.vertices));
    hash_domain(&mut hash, 2, bytemuck::cast_slice(input.indices));
    hash_domain(&mut hash, 3, bytemuck::cast_slice(input.nodes));
    hash_domain(&mut hash, 4, bytemuck::cast_slice(input.materials));
    hash_domain(&mut hash, 5, bytemuck::cast_slice(input.resources));
    hash_domain(&mut hash, 6, bytemuck::cast_slice(input.primitives));
    hash_domain(&mut hash, 7, bytemuck::cast_slice(input.attachments));
    hash_domain(
        &mut hash,
        8,
        bytemuck::cast_slice(&input.phases.opaque_cutout),
    );
    hash_domain(
        &mut hash,
        9,
        bytemuck::cast_slice(&input.phases.world_blended_unsorted),
    );
    hash_domain(
        &mut hash,
        10,
        bytemuck::cast_slice(&input.phases.chrome_authored),
    );
    hash
}

fn hash_domain(hash: &mut u64, tag: u8, bytes: &[u8]) {
    for byte in std::iter::once(tag)
        .chain(
            u64::try_from(bytes.len())
                .expect("static compiler domain length fits u64")
                .to_le_bytes(),
        )
        .chain(bytes.iter().copied())
    {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn option_glyph(value: Option<char>) -> u32 {
    value.map_or(NONE_U32, u32::from)
}

fn option_bool(value: Option<bool>) -> u32 {
    match value {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    }
}

fn primitive_kind_tag(value: PrimitiveKind) -> u32 {
    match value {
        PrimitiveKind::AtlasQuad => 1,
        PrimitiveKind::AnalyticShape => 2,
        PrimitiveKind::ShallowCard => 3,
        PrimitiveKind::InstanceQuad => 4,
    }
}

fn material_kind_tag(value: MaterialKind) -> u32 {
    match value {
        MaterialKind::UnlitGlyphSprite => 1,
        MaterialKind::UnlitAnalytic => 2,
        MaterialKind::LitShallowCard => 3,
        MaterialKind::MultiplyShadow => 4,
        MaterialKind::AdditiveGlow => 5,
        MaterialKind::ScreenChrome => 6,
    }
}

fn resource_kind_tag(value: ResourceKind) -> u32 {
    match value {
        ResourceKind::GlyphAtlas => 1,
        ResourceKind::ColorAtlas => 2,
        ResourceKind::AnalyticGeometry => 3,
        ResourceKind::ShallowCardGeometry => 4,
    }
}

fn attachment_mode_tag(value: AttachmentMode) -> u32 {
    match value {
        AttachmentMode::Follow => 1,
        AttachmentMode::SnapshotWorldOnSpawn => 2,
    }
}

fn blend_tag(value: WorldBlend) -> u32 {
    match value {
        WorldBlend::Opaque => 1,
        WorldBlend::AlphaCutout => 2,
        WorldBlend::PremultipliedAlpha => 3,
        WorldBlend::Multiply => 4,
        WorldBlend::Additive => 5,
    }
}

fn depth_tag(value: DepthBehavior) -> u32 {
    match value {
        DepthBehavior::WorldWrite => 1,
        DepthBehavior::WorldReadOnly => 2,
        DepthBehavior::ScreenNoDepth => 3,
    }
}

fn space_tag(value: PrimitiveSpace) -> u32 {
    match value {
        PrimitiveSpace::World => 1,
        PrimitiveSpace::Screen => 2,
    }
}

fn instance_group_tags(value: PrimitiveBinding) -> (u32, u32) {
    match value {
        PrimitiveBinding::ShallowCard => (0, NONE_U32),
        PrimitiveBinding::Analytic(id) => (0, u32::from(id.0)),
        PrimitiveBinding::StaticAtlas(id) => (0, u32::from(id.0)),
        PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs) => (4, 0),
        PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body)) => (1, 0),
        PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Particles)) => {
            (2, 0)
        }
        PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(slot)) => (3, u32::from(slot)),
        PrimitiveBinding::Instances(InstanceGroupBinding::TankCells { slot, layer }) => {
            (4 + instance_layer_tag(layer), u32::from(slot))
        }
        PrimitiveBinding::Instances(InstanceGroupBinding::Ambient) => (7, 0),
        PrimitiveBinding::Instances(InstanceGroupBinding::Hud) => (8, 0),
    }
}

fn instance_layer_tag(value: InstanceLayer) -> u32 {
    match value {
        InstanceLayer::Behind => 1,
        InstanceLayer::Foreground => 2,
    }
}

fn palette_role_tag(value: PetPaletteRole) -> u32 {
    match value {
        PetPaletteRole::Body => 1,
        PetPaletteRole::BodyGlow => 2,
        PetPaletteRole::Eye => 3,
        PetPaletteRole::Mouth => 4,
        PetPaletteRole::Accent => 5,
        PetPaletteRole::Pattern => 6,
        PetPaletteRole::Particle => 7,
        PetPaletteRole::Corruption => 8,
    }
}

fn mood_tag(value: MoodContentKind) -> u32 {
    match value {
        MoodContentKind::Happy => 1,
        MoodContentKind::Ecstatic => 2,
        MoodContentKind::Content => 3,
        MoodContentKind::Hungry => 4,
        MoodContentKind::Sad => 5,
        MoodContentKind::Sleepy => 6,
        MoodContentKind::Wilted => 7,
    }
}

/// The retained renderer needs the same mood tint as the round companion,
/// but only as an opaque GPU value. Keep the mapping here so WGSL never
/// duplicates product color constants.
fn mood_rim_srgba8(value: MoodContentKind) -> u32 {
    let color = match value {
        MoodContentKind::Content => crate::presentation::companion_effects::MOOD_CONTENT_SRGBA,
        MoodContentKind::Happy => crate::presentation::companion_effects::MOOD_HAPPY_SRGBA,
        MoodContentKind::Ecstatic => crate::presentation::companion_effects::MOOD_ECSTATIC_SRGBA,
        MoodContentKind::Hungry => crate::presentation::companion_effects::MOOD_HUNGRY_SRGBA,
        MoodContentKind::Sad => crate::presentation::companion_effects::MOOD_SAD_SRGBA,
        MoodContentKind::Sleepy => crate::presentation::companion_effects::MOOD_SLEEPY_SRGBA,
        MoodContentKind::Wilted => crate::presentation::companion_effects::MOOD_WILTED_SRGBA,
    };
    let [red, green, blue, alpha] = crate::presentation::companion_effects::srgba8(color);
    u32::from(red) | (u32::from(green) << 8) | (u32::from(blue) << 16) | (u32::from(alpha) << 24)
}

fn weather_tag(value: WeatherContentKind) -> u32 {
    match value {
        WeatherContentKind::Clear => 1,
        WeatherContentKind::CacheMist => 2,
        WeatherContentKind::OutputSparks => 3,
        WeatherContentKind::ReasoningPulse => 4,
        WeatherContentKind::Mixed => 5,
    }
}

fn ambient_kind_tag(value: AmbientContentKind) -> u32 {
    match value {
        AmbientContentKind::Mote => 1,
    }
}

fn analytic_semantic_tag(value: AnalyticSemantic) -> u32 {
    match value {
        AnalyticSemantic::RoomBackground => 1,
        AnalyticSemantic::WallShadow => 2,
        AnalyticSemantic::FloorProjection => 3,
        AnalyticSemantic::StatusHalo => 4,
        // Slot four remains reserved so packed semantic tags do not shift.
        AnalyticSemantic::MoodAura => 5,
        AnalyticSemantic::GaugePace => 6,
        AnalyticSemantic::Trouble => 7,
        AnalyticSemantic::Dim => 8,
        AnalyticSemantic::PropShadows => 9,
        AnalyticSemantic::GaugeDaily => 10,
        AnalyticSemantic::GaugeXp => 11,
    }
}

fn analytic_shape_tag(value: AnalyticShape) -> u32 {
    match value {
        AnalyticShape::ApertureRadial => 1,
        AnalyticShape::PetSilhouette => 2,
        AnalyticShape::PetFloorProjection => 3,
        AnalyticShape::StatusBeacon => 4,
        AnalyticShape::PerimeterGaugeSet => 6,
        AnalyticShape::TroubleBeacon => 7,
        AnalyticShape::SurfaceOverlay => 8,
        AnalyticShape::PropShadowField => 9,
    }
}

fn analytic_mask_tag(value: AnalyticMaskSource) -> u32 {
    match value {
        AnalyticMaskSource::PetBody => 1,
    }
}

fn status_beacon_tone_tag(value: StatusBeaconTone) -> u32 {
    match value {
        StatusBeaconTone::Active => 1,
        StatusBeaconTone::Calm => 2,
    }
}

fn gauge_cap_tag(value: GaugeLineCap) -> u32 {
    match value {
        GaugeLineCap::Round => 1,
    }
}

#[cfg(test)]
pub(super) fn compile_fixture_for_render_test(
    fixture: &crate::presentation::companion_scene::scene::SceneFixture,
) -> CpuSceneCandidate {
    let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
        &fixture.template,
        &fixture.content,
        &fixture.frame,
    )
    .unwrap();
    compile_cpu_parts(
        crate::presentation::companion_scene::SceneGenerationKey {
            device: crate::presentation::companion_scene::DeviceEpoch(1),
            layout: crate::presentation::companion_scene::LayoutGeneration(2),
            resources: crate::presentation::companion_scene::ResourceGeneration(3),
        },
        crate::presentation::companion_scene::AppliedRevisions::new(4, 5),
        &fixture.template,
        &fixture.content,
        &fixture.frame,
        accepted,
    )
    .unwrap()
}

#[cfg(test)]
pub(super) fn compile_static_fixture_for_render_test(
    fixture: &crate::presentation::companion_scene::scene::SceneFixture,
) -> CpuSceneCandidate {
    let accepted_fixture = crate::presentation::companion_scene::scene::SceneFixture::valid();
    let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
        &accepted_fixture.template,
        &accepted_fixture.content,
        &accepted_fixture.frame,
    )
    .unwrap();
    compile_cpu_parts(
        crate::presentation::companion_scene::SceneGenerationKey {
            device: crate::presentation::companion_scene::DeviceEpoch(1),
            layout: crate::presentation::companion_scene::LayoutGeneration(2),
            resources: crate::presentation::companion_scene::ResourceGeneration(3),
        },
        crate::presentation::companion_scene::AppliedRevisions::new(4, 5),
        &fixture.template,
        &fixture.content,
        &fixture.frame,
        accepted,
    )
    .unwrap()
}

#[cfg(test)]
fn lifetime_watch_fixture() -> crate::tui::view_model::WatchViewModel {
    let day = time::macros::date!(2026 - 07 - 12);
    let mut vm =
        crate::tui::view_model::WatchViewModel::fixture_with_tank_inhabitants_for_age(120, day);
    vm.habitat.earned_props = crate::tui::view_model::WatchViewModel::fixture_with_habitat_props()
        .habitat
        .earned_props;
    let pet = crate::pet::generation::generate_pet("retained-cpu-lifetime")
        .with_species(crate::pet::generation::Species::Fuzz);
    let rendered = crate::pet::render::render_pet(
        &pet,
        crate::game::evolution::Stage::S3,
        crate::game::metabolism::Mood::Content,
        crate::pet::render::AnimationFrame::default(),
    );
    vm.pet_render.seed = pet.seed;
    vm.pet_render.generated_species = crate::pet::generation::Species::Fuzz;
    vm.pet_render.stage = crate::game::evolution::Stage::S3;
    vm.pet_render.mood = crate::game::metabolism::Mood::Content;
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    vm
}

#[cfg(test)]
fn project_full_scene_snapshot(
    base: &crate::tui::view_model::WatchViewModel,
    frame_index: usize,
) -> crate::presentation::companion_scene::CompanionSceneSnapshot {
    let mut vm = base.clone();
    vm.pet_render.mood = if (frame_index / 25).is_multiple_of(2) {
        crate::game::metabolism::Mood::Content
    } else {
        crate::game::metabolism::Mood::Happy
    };
    vm.day_context.asleep = (frame_index / 40) % 2 == 1;
    vm.life_profile.calm_mode = (frame_index / 30) % 2 == 1;
    vm.progress.fraction = (frame_index % 101) as f32 / 100.0;
    vm.source_health[0].status = if (frame_index / 45).is_multiple_of(2) {
        crate::tui::view_model::SourceStatus::Ready
    } else {
        crate::tui::view_model::SourceStatus::Diagnostic
    };
    let wall_time = time::macros::datetime!(2026-07-11 12:00:55 UTC)
        + time::Duration::seconds(i64::try_from(frame_index).unwrap());
    vm.last_feed_pulse_at = (frame_index % 20 < 10).then_some(
        wall_time - time::Duration::milliseconds(i64::try_from(frame_index % 10).unwrap() * 150),
    );
    crate::presentation::companion_scene::CompanionSceneSnapshot::project_with_input(
        &vm,
        crate::presentation::companion_scene::CompanionSceneProjectionInput::round(
            crate::presentation::companion_scene::CompanionProjectionClock::new(
                wall_time,
                u64::try_from(frame_index).unwrap() * 33,
            ),
            crate::presentation::companion_scene::CompanionLogicalLayout::round(360.0, 360.0),
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        ),
    )
    .unwrap()
}

#[cfg(test)]
fn project_lifetime_snapshot(
    base: &crate::tui::view_model::WatchViewModel,
    frame_index: usize,
) -> crate::presentation::companion_scene::CompanionSceneSnapshot {
    let mut snapshot = project_full_scene_snapshot(base, frame_index);
    let semantic_phase = u8::try_from((frame_index / 20) % 2).unwrap();
    for prop in &mut snapshot.content.prop_animation_states {
        if prop.sprite_phase.is_some() {
            prop.sprite_phase = Some(semantic_phase);
        }
        if prop.twinkle_active.is_some() {
            prop.twinkle_active = Some(semantic_phase != 0);
        }
    }
    for tank in &mut snapshot.content.tank_animation_states {
        tank.sprite_variant = semantic_phase;
    }
    snapshot
}

/// Builds one complete test candidate through the real companion projection,
/// neutral scene generation, and CPU compiler path. This is deliberately a
/// test-only sibling seam: production activation continues to own its versions.
#[cfg(test)]
pub(super) fn compile_projected_full_scene_for_render_test(
    frame_index: usize,
) -> CpuSceneCandidate {
    let snapshot = std::sync::Arc::new(project_full_scene_snapshot(
        &lifetime_watch_fixture(),
        frame_index,
    ));
    assert_eq!(
        snapshot.topology.pet.species,
        crate::pet::generation::Species::Fuzz,
    );
    assert_eq!(
        snapshot.topology.pet.stage,
        crate::game::evolution::Stage::S3,
    );
    assert_eq!(snapshot.topology.visible_props.len(), 2);
    assert_eq!(snapshot.content.prop_animation_states.len(), 2);
    assert_eq!(snapshot.topology.visible_tank_inhabitants.len(), 2);
    assert_eq!(snapshot.content.tank_animation_states.len(), 2);
    let generation = crate::presentation::companion_scene::scene::build_scene_generation_owned(
        snapshot,
        crate::presentation::companion_scene::SceneGenerationKey {
            device: crate::presentation::companion_scene::DeviceEpoch(7),
            layout: crate::presentation::companion_scene::LayoutGeneration(8),
            resources: crate::presentation::companion_scene::ResourceGeneration(9),
        },
        crate::presentation::companion_scene::AppliedRevisions::new(10, 20),
    )
    .expect("production-derived test scene builds");
    compile_cpu_generation(&generation).expect("production-derived test scene compiles")
}

#[cfg(test)]
pub(super) fn projected_full_scene_snapshot_for_render_test(
    frame_index: usize,
) -> crate::presentation::companion_scene::CompanionSceneSnapshot {
    project_full_scene_snapshot(&lifetime_watch_fixture(), frame_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::companion_scene::scene::{
        AnalyticContent, AnalyticContentSlot, AnalyticFrame, AnalyticFrameSlot, AnalyticGeometry,
        AnalyticMaskSource, AnalyticPaint, AnalyticSemantic, AuthoredGlyph, GaugeLaneGeometry,
        GaugeLanePaint, GaugeLineCap, MaterialKind, SceneFixture, StatusBeaconTone,
    };

    const fn packed_rgb(value: [u8; 3]) -> u32 {
        value[0] as u32 | ((value[1] as u32) << 8) | ((value[2] as u32) << 16)
    }

    const fn packed_rgba(value: [u8; 4]) -> u32 {
        packed_rgb([value[0], value[1], value[2]]) | ((value[3] as u32) << 24)
    }

    fn compile_fixture(fixture: &SceneFixture) -> CpuSceneCandidate {
        let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
            &fixture.template,
            &fixture.content,
            &fixture.frame,
        )
        .unwrap();
        compile_cpu_parts(
            crate::presentation::companion_scene::SceneGenerationKey {
                device: crate::presentation::companion_scene::DeviceEpoch(1),
                layout: crate::presentation::companion_scene::LayoutGeneration(2),
                resources: crate::presentation::companion_scene::ResourceGeneration(3),
            },
            crate::presentation::companion_scene::AppliedRevisions::new(4, 5),
            &fixture.template,
            &fixture.content,
            &fixture.frame,
            accepted,
        )
        .unwrap()
    }

    fn compile_static_fixture(fixture: &SceneFixture) -> CpuSceneCandidate {
        let accepted_fixture = SceneFixture::valid();
        let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
            &accepted_fixture.template,
            &accepted_fixture.content,
            &accepted_fixture.frame,
        )
        .unwrap();
        compile_cpu_parts(
            crate::presentation::companion_scene::SceneGenerationKey {
                device: crate::presentation::companion_scene::DeviceEpoch(1),
                layout: crate::presentation::companion_scene::LayoutGeneration(2),
                resources: crate::presentation::companion_scene::ResourceGeneration(3),
            },
            crate::presentation::companion_scene::AppliedRevisions::new(4, 5),
            &fixture.template,
            &fixture.content,
            &fixture.frame,
            accepted,
        )
        .unwrap()
    }

    fn paired_deltas(
        candidate: &CpuSceneCandidate,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> (ContentDelta, FrameDelta) {
        let mut content = ContentDelta::empty();
        content.generation_key = candidate.generation_key;
        content.from = candidate.source_revisions;
        content.to = to;
        let mut frame = FrameDelta::empty();
        frame.generation_key = candidate.generation_key;
        frame.from = candidate.source_revisions;
        frame.to = to;
        (content, frame)
    }

    fn all_dirty_sets_are_empty(dirty: &SceneDirtySpans) -> bool {
        [
            &dirty.content_globals,
            &dirty.pet_body,
            &dirty.pet_particles,
            &dirty.room_content,
            &dirty.prop_glyphs,
            &dirty.tank_glyphs,
            &dirty.content_ambient,
            &dirty.content_analytics,
            &dirty.frame_globals,
            &dirty.nodes,
            &dirty.room_frame,
            &dirty.props,
            &dirty.tank_cells,
            &dirty.frame_ambient,
            &dirty.frame_analytics,
            &dirty.lights,
        ]
        .into_iter()
        .all(|spans| spans.as_slice().is_empty())
    }

    #[derive(Clone, Copy)]
    struct CandidateStorageIdentity {
        static_vecs: [(usize, usize); 21],
        fixed_mirrors: [(usize, usize); 16],
        logical_vecs: [(usize, usize); 15],
    }

    fn vec_identity<T>(values: &Vec<T>) -> (usize, usize) {
        (values.as_ptr() as usize, values.capacity())
    }

    fn candidate_storage_identity(candidate: &CpuSceneCandidate) -> CandidateStorageIdentity {
        let template = candidate.accepted.template().template();
        let accepted_frame = candidate.accepted.frame().frame();
        CandidateStorageIdentity {
            static_vecs: [
                vec_identity(&candidate.vertices),
                vec_identity(&candidate.indices),
                vec_identity(&candidate.nodes),
                vec_identity(&candidate.materials),
                vec_identity(&candidate.resources),
                vec_identity(&candidate.primitives),
                vec_identity(&candidate.attachments),
                vec_identity(&candidate.phases.opaque_cutout),
                vec_identity(&candidate.phases.world_blended_unsorted),
                vec_identity(&candidate.phases.chrome_authored),
                vec_identity(&candidate.index.nodes),
                vec_identity(&candidate.index.materials),
                vec_identity(&candidate.index.resources),
                vec_identity(&candidate.index.attachments),
                vec_identity(&template.nodes),
                vec_identity(&template.primitives),
                vec_identity(&template.materials),
                vec_identity(&template.resources),
                vec_identity(&template.attachments),
                vec_identity(&template.static_atlas_recipes),
                vec_identity(&template.analytic_templates),
            ],
            fixed_mirrors: [
                (
                    candidate.content.globals.as_slice().as_ptr() as usize,
                    candidate.content.globals.capacity(),
                ),
                (
                    candidate.content.pet_body.as_slice().as_ptr() as usize,
                    candidate.content.pet_body.capacity(),
                ),
                (
                    candidate.content.pet_particles.as_slice().as_ptr() as usize,
                    candidate.content.pet_particles.capacity(),
                ),
                (
                    candidate.content.room_glyphs.as_slice().as_ptr() as usize,
                    candidate.content.room_glyphs.capacity(),
                ),
                (
                    candidate.content.prop_glyphs.as_slice().as_ptr() as usize,
                    candidate.content.prop_glyphs.capacity(),
                ),
                (
                    candidate.content.tank_glyphs.as_slice().as_ptr() as usize,
                    candidate.content.tank_glyphs.capacity(),
                ),
                (
                    candidate.content.ambient.as_slice().as_ptr() as usize,
                    candidate.content.ambient.capacity(),
                ),
                (
                    candidate.content.analytics.as_slice().as_ptr() as usize,
                    candidate.content.analytics.capacity(),
                ),
                (
                    candidate.frame.globals.as_slice().as_ptr() as usize,
                    candidate.frame.globals.capacity(),
                ),
                (
                    candidate.frame.nodes.as_slice().as_ptr() as usize,
                    candidate.frame.nodes.capacity(),
                ),
                (
                    candidate.frame.room_glyphs.as_slice().as_ptr() as usize,
                    candidate.frame.room_glyphs.capacity(),
                ),
                (
                    candidate.frame.props.as_slice().as_ptr() as usize,
                    candidate.frame.props.capacity(),
                ),
                (
                    candidate.frame.tank_cells.as_slice().as_ptr() as usize,
                    candidate.frame.tank_cells.capacity(),
                ),
                (
                    candidate.frame.ambient.as_slice().as_ptr() as usize,
                    candidate.frame.ambient.capacity(),
                ),
                (
                    candidate.frame.analytics.as_slice().as_ptr() as usize,
                    candidate.frame.analytics.capacity(),
                ),
                (
                    candidate.frame.lights.as_slice().as_ptr() as usize,
                    candidate.frame.lights.capacity(),
                ),
            ],
            logical_vecs: [
                vec_identity(&candidate.logical_content.pet_art_slots),
                vec_identity(&candidate.logical_content.room_glyph_slots),
                vec_identity(&candidate.logical_content.prop_slots),
                vec_identity(&candidate.logical_content.tank_slots),
                vec_identity(&candidate.logical_content.ambient_slots),
                vec_identity(&candidate.logical_content.prop_paint_slots),
                vec_identity(&candidate.logical_content.ambient_paint_slots),
                vec_identity(&candidate.logical_content.analytic_slots),
                vec_identity(&accepted_frame.nodes),
                vec_identity(&accepted_frame.room_glyph_slots),
                vec_identity(&accepted_frame.prop_slots),
                vec_identity(&accepted_frame.tank_slots),
                vec_identity(&accepted_frame.ambient_slots),
                vec_identity(&accepted_frame.analytic_slots),
                vec_identity(&accepted_frame.lights),
            ],
        }
    }

    fn assert_candidate_storage_identity(
        expected: CandidateStorageIdentity,
        candidate: &CpuSceneCandidate,
    ) {
        let actual = candidate_storage_identity(candidate);
        assert_eq!(actual.static_vecs, expected.static_vecs);
        assert_eq!(actual.fixed_mirrors, expected.fixed_mirrors);
        assert_eq!(actual.logical_vecs, expected.logical_vecs);
    }

    #[test]
    fn static_layouts_are_explicit_and_stable() {
        assert_eq!(std::mem::size_of::<StaticVertex>(), 40);
        assert_eq!(std::mem::align_of::<StaticVertex>(), 4);
        assert_eq!(std::mem::offset_of!(StaticVertex, local_position), 0);
        assert_eq!(std::mem::offset_of!(StaticVertex, uv), 12);
        assert_eq!(std::mem::offset_of!(StaticVertex, normal), 20);
        assert_eq!(std::mem::offset_of!(StaticVertex, primitive_index), 32);
        assert_eq!(std::mem::offset_of!(StaticVertex, material_index), 36);
        assert_eq!(std::mem::size_of::<NodeGpuValue>(), 96);
        assert_eq!(std::mem::align_of::<NodeGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(NodeGpuValue, world), 0);
        assert_eq!(std::mem::offset_of!(NodeGpuValue, opacity), 64);
        assert_eq!(std::mem::offset_of!(NodeGpuValue, visible), 68);
        assert_eq!(
            std::mem::offset_of!(NodeGpuValue, material_parameter_offset),
            72
        );
        assert_eq!(
            std::mem::offset_of!(NodeGpuValue, material_parameter_count),
            76
        );
        assert_eq!(std::mem::offset_of!(NodeGpuValue, depth_cue), 80);
        assert_eq!(std::mem::size_of::<ContentGpuValue>(), 32);
        assert_eq!(std::mem::align_of::<ContentGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(ContentGpuValue, signed_data), 16);
        assert_eq!(std::mem::offset_of!(ContentGpuValue, flags), 24);
        assert_eq!(std::mem::size_of::<AnalyticContentGpuValue>(), 48);
        assert_eq!(std::mem::align_of::<AnalyticContentGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(AnalyticContentGpuValue, payload), 16);
        assert_eq!(std::mem::size_of::<FrameGpuValue>(), 64);
        assert_eq!(std::mem::align_of::<FrameGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(FrameGpuValue, values), 16);
        assert_eq!(std::mem::size_of::<AnalyticFrameGpuValue>(), 96);
        assert_eq!(std::mem::align_of::<AnalyticFrameGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(AnalyticFrameGpuValue, rect_points), 16);
        assert_eq!(std::mem::offset_of!(AnalyticFrameGpuValue, payload), 32);

        assert_eq!(std::mem::size_of::<ContentGlobalsGpuValue>(), 176);
        assert_eq!(std::mem::align_of::<ContentGlobalsGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(ContentGlobalsGpuValue, mood), 128);
        assert_eq!(std::mem::offset_of!(ContentGlobalsGpuValue, weather), 132);
        assert_eq!(
            std::mem::offset_of!(ContentGlobalsGpuValue, glyph_grid_dimensions),
            136
        );
        assert_eq!(
            std::mem::offset_of!(ContentGlobalsGpuValue, glyph_grid_origin_points),
            144
        );
        assert_eq!(
            std::mem::offset_of!(ContentGlobalsGpuValue, glyph_cell_extent_points),
            152
        );
        assert_eq!(
            std::mem::offset_of!(ContentGlobalsGpuValue, pet_rim_srgba8),
            160
        );
        assert_eq!(std::mem::size_of::<FrameGlobalsGpuValue>(), 192);
        assert_eq!(std::mem::align_of::<FrameGlobalsGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(FrameGlobalsGpuValue, projection), 64);
        assert_eq!(
            std::mem::offset_of!(FrameGlobalsGpuValue, viewport_points),
            128
        );
        assert_eq!(std::mem::offset_of!(FrameGlobalsGpuValue, aperture), 144);
        assert_eq!(std::mem::offset_of!(FrameGlobalsGpuValue, gauges), 160);
        assert_eq!(std::mem::offset_of!(FrameGlobalsGpuValue, dim_amount), 176);
        assert_eq!(
            std::mem::offset_of!(FrameGlobalsGpuValue, activity_opacity),
            184
        );
        assert_eq!(
            std::mem::offset_of!(FrameGlobalsGpuValue, reduce_motion),
            188
        );
    }

    #[test]
    fn prop_frame_packs_cast_shadow_tail() {
        let packed = pack_prop_frame(crate::presentation::companion_scene::scene::PropFrameSlot {
            slot: 3,
            visible: true,
            origin_points: [10.0, 20.0],
            motion_offset_points: [1.0, 2.0],
            opacity: 0.8,
            footprint_points: [12.0, 14.0],
            contact_shadow_strength: 0.6,
            cast_shadow_vector_points: [3.0, -15.0],
            cast_shadow_softness_points: 4.5,
            cast_shadow_strength: 0.25,
        });

        assert_eq!(std::mem::size_of::<FrameGpuValue>(), 64);
        assert_eq!(
            packed.values,
            [10.0, 20.0, 1.0, 2.0, 0.8, 12.0, 14.0, 0.6, 3.0, -15.0, 4.5, 0.25,]
        );
    }

    #[test]
    fn fixed_family_capacities_are_exact() {
        let content = ContentMirrors::zeroed();
        assert_eq!(content.pet_body.capacity(), 130);
        assert_eq!(content.pet_particles.capacity(), 130);
        assert_eq!(content.room_glyphs.capacity(), 32);
        assert_eq!(content.prop_glyphs.capacity(), 90);
        assert_eq!(content.tank_glyphs.capacity(), 16);
        assert_eq!(content.ambient.capacity(), 64);
        assert_eq!(content.analytics.capacity(), 16);
        let frame = FrameMirrors::zeroed();
        assert_eq!(frame.nodes.capacity(), 128);
        assert_eq!(frame.room_glyphs.capacity(), 32);
        assert_eq!(frame.props.capacity(), 10);
        assert_eq!(frame.tank_cells.capacity(), 16);
        assert_eq!(frame.ambient.capacity(), 64);
        assert_eq!(frame.analytics.capacity(), 16);
        assert_eq!(frame.lights.capacity(), 2);

        let production = compile_projected_full_scene_for_render_test(0);
        assert_eq!(production.primitive_count(), 21);
        assert_eq!(production.phases.opaque_cutout.len(), 1);
        assert_eq!(production.phases.world_blended_unsorted.len(), 17);
        assert_eq!(production.phases.chrome_authored.len(), 3);
    }

    #[test]
    fn analytic_packers_preserve_all_active_roles_exactly() {
        let lane = |base: u8| GaugeLanePaint {
            track_srgba8: [base, base + 1, base + 2, base + 3],
            fill_srgba8: [base + 4, base + 5, base + 6, base + 7],
        };
        let gauge_paint = AnalyticPaint::PerimeterGaugeSet {
            xp: lane(32),
            daily: lane(40),
            pace: lane(48),
            daily_overage_srgba8: [56, 57, 58, 59],
            daily_rollover_contract_unorm8: [64, 65, 66, 67],
        };
        let paints = [
            AnalyticPaint::ApertureDepth {
                core_srgb8: [1, 2, 3],
                rim_srgb8: [4, 5, 6],
                bed_srgb8: [7, 8, 9],
                fleck_srgb8: [10, 11, 12],
            },
            AnalyticPaint::PetShadowTint { color_srgb8: [7, 8, 9], opacity_u8: 10 },
            AnalyticPaint::FloorShadowMultiplySilhouette { color_srgba8: [11, 12, 13, 14] },
            AnalyticPaint::StatusBeacon {
                active_srgba8: [19, 20, 21, 22],
                calm_srgba8: [23, 24, 25, 26],
            },
            gauge_paint,
            AnalyticPaint::TroubleBeacon { color_srgba8: [60, 61, 62, 63] },
            AnalyticPaint::DimOverlay { color_srgb8: [64, 65, 66] },
            AnalyticPaint::PropShadowMultiply { color_srgb8: [68, 69, 70] },
            gauge_paint,
            gauge_paint,
        ];
        let expected_gauge_paint = [
            packed_rgba([32, 33, 34, 35]),
            packed_rgba([36, 37, 38, 39]),
            packed_rgba([40, 41, 42, 43]),
            packed_rgba([44, 45, 46, 47]),
            packed_rgba([48, 49, 50, 51]),
            packed_rgba([52, 53, 54, 55]),
            packed_rgba([56, 57, 58, 59]),
            packed_rgba([64, 65, 66, 67]),
        ];
        let expected_paints = [
            [
                packed_rgb([1, 2, 3]),
                packed_rgb([4, 5, 6]),
                packed_rgb([7, 8, 9]),
                packed_rgb([10, 11, 12]),
                0,
                0,
                0,
                0,
            ],
            [packed_rgb([7, 8, 9]), 10, 0, 0, 0, 0, 0, 0],
            [packed_rgba([11, 12, 13, 14]), 0, 0, 0, 0, 0, 0, 0],
            [
                packed_rgba([19, 20, 21, 22]),
                packed_rgba([23, 24, 25, 26]),
                0,
                0,
                0,
                0,
                0,
                0,
            ],
            expected_gauge_paint,
            [packed_rgba([60, 61, 62, 63]), 0, 0, 0, 0, 0, 0, 0],
            [packed_rgb([64, 65, 66]), 0, 0, 0, 0, 0, 0, 0],
            [packed_rgb([68, 69, 70]), 0, 0, 0, 0, 0, 0, 0],
            expected_gauge_paint,
            expected_gauge_paint,
        ];
        for ((semantic, paint), expected) in AnalyticSemantic::ALL
            .into_iter()
            .zip(paints)
            .zip(expected_paints)
        {
            let packed = pack_analytic_content(AnalyticContentSlot {
                id: semantic.id(),
                value: Some(AnalyticContent {
                    semantic,
                    shape: semantic.shape().expect("active semantic has a shape"),
                    paint,
                }),
            });
            assert_eq!(packed.id, u32::from(semantic.id().0));
            assert_eq!(packed.semantic, analytic_semantic_tag(semantic));
            assert_eq!(
                packed.shape,
                analytic_shape_tag(semantic.shape().expect("active semantic has a shape"))
            );
            assert_eq!(packed.flags, 1);
            assert_eq!(packed.payload, expected);
        }

        let lane_geometry = |base: f32| GaugeLaneGeometry {
            radius_points: base,
            stroke_width_points: base + 1.0,
            track_start_degrees: base + 2.0,
            track_sweep_degrees: base + 3.0,
            cap: GaugeLineCap::Round,
        };
        let gauge_geometry = AnalyticGeometry::PerimeterGaugeSet {
            center_points: [22.0, 23.0],
            xp: lane_geometry(24.0),
            daily: lane_geometry(28.0),
            pace: lane_geometry(32.0),
        };
        let geometries = [
            AnalyticGeometry::ApertureRadial {
                center_points: [1.0, 2.0],
                radius_points: 3.0,
                feather_points: 4.0,
            },
            AnalyticGeometry::PetSilhouette {
                mask: AnalyticMaskSource::PetBody,
                offset_points: [5.0, 6.0],
                softness_points: 7.0,
            },
            AnalyticGeometry::PetFloorProjection {
                mask: AnalyticMaskSource::PetBody,
                facing: -1,
            },
            AnalyticGeometry::StatusBeacon {
                center_points: [13.0, 14.0],
                radius_points: 15.0,
                thickness_points: 16.0,
                tone: StatusBeaconTone::Active,
            },
            gauge_geometry,
            AnalyticGeometry::TroubleBeacon {
                center_points: [36.0, 37.0],
                radius_points: 38.0,
                thickness_points: 39.0,
            },
            AnalyticGeometry::SurfaceOverlay,
            AnalyticGeometry::PropShadowField,
            gauge_geometry,
            gauge_geometry,
        ];
        let expected_gauge_geometry = [
            22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0, 30.0, 31.0, 32.0, 33.0, 34.0, 35.0,
            0.0, 0.0,
        ];
        let expected_geometry = [
            [
                1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            [
                1.0, 5.0, 6.0, 7.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            [
                1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            [
                13.0, 14.0, 15.0, 16.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            expected_gauge_geometry,
            [
                36.0, 37.0, 38.0, 39.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ],
            [0.0; 16],
            [0.0; 16],
            expected_gauge_geometry,
            expected_gauge_geometry,
        ];
        for (index, ((semantic, geometry), expected)) in AnalyticSemantic::ALL
            .into_iter()
            .zip(geometries)
            .zip(expected_geometry)
            .enumerate()
        {
            let rect_points = [index as f32, 40.0, 41.0, 42.0];
            let packed = pack_analytic_frame(AnalyticFrameSlot {
                id: semantic.id(),
                value: Some(AnalyticFrame {
                    semantic,
                    shape: semantic.shape().expect("active semantic has a shape"),
                    rect_points,
                    geometry,
                }),
            });
            assert_eq!(packed.id, u32::from(semantic.id().0));
            assert_eq!(packed.semantic, analytic_semantic_tag(semantic));
            assert_eq!(
                packed.shape,
                analytic_shape_tag(semantic.shape().expect("active semantic has a shape"))
            );
            let expected_flags = if semantic.gauge_lane().is_some() {
                0x1_1101
            } else {
                1
            };
            assert_eq!(packed.flags, expected_flags);
            assert_eq!(packed.rect_points, rect_points);
            assert_eq!(packed.payload, expected);
        }
    }

    #[test]
    fn glyph_content_packing_preserves_filters_grid_room_and_explicit_paints() {
        use crate::presentation::companion_scene::scene::{
            AmbientContentKind, GlyphPaintSource, InstanceGroupBinding, PetGlyph, PropGlyphContent,
            PropSemanticContent, RoomGlyphContentSlot, RoomGlyphFrameSlot,
        };

        let mut fixture = SceneFixture::valid();
        fixture.content.pet_art_slots[0].glyph =
            Some(PetGlyph::for_species('^', crate::pet::generation::Species::Fuzz).unwrap());
        fixture.content.pet_art_slots[0].palette_role = PetPaletteRole::Eye;
        fixture.content.pet_art_slots[1].glyph =
            Some(PetGlyph::for_species('^', crate::pet::generation::Species::Fuzz).unwrap());
        fixture.content.pet_art_slots[1].palette_role = PetPaletteRole::Particle;
        fixture.content.room_glyph_slots[0] = RoomGlyphContentSlot {
            slot: 0,
            glyph: Some(AuthoredGlyph::new('◆').unwrap()),
            color_srgb8: Some([1, 2, 3]),
        };
        fixture.frame.room_glyph_slots[0] = RoomGlyphFrameSlot {
            slot: 0,
            visible: true,
            grid_cell: [2, 3],
            position_points: [24.0, 312.0],
            opacity: 0.75,
        };
        fixture.content.prop_slots[0].content = Some(PropSemanticContent {
            sprite_phase: Some(7),
            twinkle_active: Some(true),
            lid_open: Some(false),
            bloom_active: None,
            glyphs: std::array::from_fn(|index| PropGlyphContent {
                glyph: (index == 0).then(|| AuthoredGlyph::new('◇').unwrap()),
                local_cell: if index == 0 { [0, -1] } else { [0; 2] },
            }),
        });
        fixture.content.prop_paint_slots[0].paints[0] =
            Some(GlyphPaintSource { color_srgb8: [4, 5, 6] });
        fixture.content.ambient_slots[0].kind = Some(AmbientContentKind::Mote);
        fixture.content.ambient_slots[0].glyph = Some(AuthoredGlyph::new('✦').unwrap());
        fixture.content.ambient_paint_slots[0].paint =
            Some(GlyphPaintSource { color_srgb8: [7, 8, 9] });

        let compiled = compile_fixture(&fixture);
        let globals = compiled.content.globals.as_slice()[0];
        assert_eq!(globals.glyph_grid_dimensions, [30, 30]);
        assert_eq!(globals.glyph_grid_origin_points, [0.0, 0.0]);
        assert_eq!(globals.glyph_cell_extent_points, [12.0, 12.0]);

        assert_eq!(
            compiled.content.pet_body.as_slice()[0].glyph_scalar,
            u32::from('^')
        );
        assert_eq!(
            compiled.content.pet_particles.as_slice()[0].glyph_scalar,
            NONE_U32
        );
        assert_eq!(
            compiled.content.pet_body.as_slice()[1].glyph_scalar,
            NONE_U32
        );
        assert_eq!(
            compiled.content.pet_particles.as_slice()[1].glyph_scalar,
            u32::from('^')
        );
        assert_eq!(compiled.content.pet_particles.as_slice()[1].slot, 1);

        let room = compiled.content.room_glyphs.as_slice()[0];
        assert_eq!(room.kind, 5);
        assert_eq!(room.glyph_scalar, u32::from('◆'));
        assert_eq!(room.flags, 1);
        assert_eq!(room.variant, 0xff03_0201);
        let room_frame = compiled.frame.room_glyphs.as_slice()[0];
        assert_eq!(room_frame.kind, 4);
        assert_eq!(room_frame.flags, 1);
        assert_eq!(
            room_frame.values,
            [2.0, 3.0, 24.0, 312.0, 0.75, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );

        let prop = compiled.content.prop_glyphs.as_slice()[0];
        assert_eq!(prop.flags, 2 | (1 << 2) | (1 << 6) | (8 << 8));
        assert_eq!(prop.variant, 0xff06_0504);
        let ambient = compiled.content.ambient.as_slice()[0];
        assert_eq!(ambient.flags, 1 | (1 << 8));
        assert_eq!(ambient.variant, 0xff09_0807);
        assert_eq!(
            instance_group_tags(PrimitiveBinding::Instances(
                InstanceGroupBinding::RoomGlyphs
            )),
            (4, 0)
        );
        let mut upload_fixture = SceneFixture::valid();
        upload_fixture.template.primitives[0].kind = PrimitiveKind::InstanceQuad;
        upload_fixture.template.primitives[0].binding =
            PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs);
        upload_fixture.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        upload_fixture.template.primitives[0].depth = DepthBehavior::WorldReadOnly;
        let upload = compile_fixture(&upload_fixture)
            .primitive_upload_source(0)
            .unwrap();
        assert_eq!(upload.instance_group, 4);
        assert_eq!(upload.instance_base, 0);

        let mut analytic_fixture = SceneFixture::valid();
        analytic_fixture.template.primitives[0].binding = PrimitiveBinding::Analytic(
            crate::presentation::companion_scene::scene::AnalyticParamId(5),
        );
        analytic_fixture.template.primitives[0].authored_order = 17;
        let analytic_upload = compile_static_fixture(&analytic_fixture)
            .primitive_upload_source(0)
            .unwrap();
        assert_eq!(analytic_upload.instance_group, 0);
        assert_eq!(analytic_upload.instance_base, NONE_U32);
        assert_eq!(analytic_upload.instance_slot, 5);
        assert_eq!(analytic_upload.authored_order, 17);

        let mut wall_fixture = SceneFixture::valid();
        let mut body = wall_fixture.template.primitives[0].clone();
        body.kind = PrimitiveKind::InstanceQuad;
        body.binding =
            PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body));
        wall_fixture.template.primitives[0].kind = PrimitiveKind::AnalyticShape;
        wall_fixture.template.primitives[0].binding =
            PrimitiveBinding::Analytic(AnalyticSemantic::WallShadow.id());
        wall_fixture.template.primitives.push(body);
        let wall = compile_static_fixture(&wall_fixture);
        assert_eq!(
            wall.primitive_upload_source(0).unwrap().aux_node_index,
            wall.primitive_upload_source(1).unwrap().node_index,
        );

        let static_upload = compile_fixture(&SceneFixture::valid())
            .primitive_upload_source(0)
            .unwrap();
        assert_eq!(static_upload.instance_slot, 0);
        assert_eq!(static_upload.authored_order, 0);
    }

    #[test]
    fn compiler_maps_semantic_ids_to_dense_offsets_deterministically() {
        let fixture = SceneFixture::valid();
        let a = compile_fixture(&fixture);
        let b = compile_fixture(&fixture);
        assert_eq!(a.static_checksum, b.static_checksum);
        for (expected, node) in fixture.template.nodes.iter().enumerate() {
            assert_eq!(a.index.node_offset(node.id), Some(expected as u32));
            assert_eq!(a.index.node_offset(node.id), b.index.node_offset(node.id));
        }
    }

    #[test]
    fn child_before_parent_keeps_template_dense_order_and_resolves_world_recursively() {
        let mut fixture = SceneFixture::valid();
        let root = fixture.template.nodes[0].id;
        let child = fixture.template.nodes[1].id;
        fixture.template.nodes[0].base_transform =
            crate::presentation::companion_scene::scene::Transform3::translated([10.0, 3.0, 0.0]);
        fixture
            .frame
            .nodes
            .iter_mut()
            .find(|node| node.node == child)
            .unwrap()
            .local_transform =
            crate::presentation::companion_scene::scene::Transform3::translated([2.0, 4.0, 0.0]);
        fixture.template.nodes.swap(0, 1);

        let compiled = compile_static_fixture(&fixture);
        assert_eq!(compiled.index.node_offset(child), Some(0));
        assert_eq!(compiled.index.node_offset(root), Some(1));
        assert_eq!(
            compiled.frame.nodes.as_slice()[0].world[3],
            [12.0, 7.0, 0.0, 1.0]
        );
        assert_eq!(compiled.nodes[0].parent_dense_index, 1);
    }

    #[test]
    fn checkpoint_a_emits_one_provisional_front_face_quad_per_primitive() {
        let mut fixture = SceneFixture::valid();
        let mut second = fixture.template.primitives[0].clone();
        second.authored_order = 1;
        second.local_geometry = Bounds3 {
            min: [-2.0, -3.0, 0.25],
            max: [4.0, 5.0, 0.25],
        };
        fixture.template.primitives.push(second);
        let compiled = compile_static_fixture(&fixture);
        assert_eq!(compiled.primitives.len(), 2);
        assert_eq!(compiled.vertices.len(), 8);
        assert_eq!(compiled.indices.len(), 12);
        assert_eq!(compiled.primitives[1].first_vertex, 4);
        assert_eq!(compiled.primitives[1].first_index, 6);
        assert_eq!(compiled.primitives[1].index_count, 6);
        assert_eq!(compiled.primitives[1].local_bounds_max, [4.0, 5.0, 0.25]);
        assert_eq!(compiled.vertices[4].local_position, [-2.0, -3.0, 0.25]);
        assert_eq!(&compiled.indices[6..], &[4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn phase_lists_keep_world_blends_unsorted_and_sort_chrome_by_authored_order() {
        let mut fixture = SceneFixture::valid();
        let base = fixture.template.primitives[0].clone();
        let mut blend = base.clone();
        blend.blend = WorldBlend::PremultipliedAlpha;
        blend.depth = DepthBehavior::WorldReadOnly;
        blend.authored_order = 30;
        let mut chrome_late = base.clone();
        chrome_late.material = crate::presentation::companion_scene::scene::MaterialId(100);
        chrome_late.blend = WorldBlend::PremultipliedAlpha;
        chrome_late.depth = DepthBehavior::ScreenNoDepth;
        chrome_late.space = PrimitiveSpace::Screen;
        chrome_late.authored_order = 20;
        let mut chrome_early = chrome_late.clone();
        chrome_early.authored_order = 10;
        fixture.template.materials.push(
            crate::presentation::companion_scene::scene::MaterialTemplate {
                id: chrome_late.material,
                alias: crate::presentation::companion_scene::scene::CanonicalAlias::new(
                    "material.chrome",
                )
                .unwrap(),
                kind: MaterialKind::ScreenChrome,
            },
        );
        fixture.template.primitives = vec![base, blend, chrome_late, chrome_early];
        let compiled = compile_static_fixture(&fixture);
        assert_eq!(compiled.phases.opaque_cutout, vec![0]);
        assert_eq!(compiled.phases.world_blended_unsorted, vec![1]);
        assert_eq!(compiled.phases.chrome_authored, vec![3, 2]);
    }

    #[test]
    fn initial_mirrors_preserve_nested_prop_and_tank_semantics() {
        let mut fixture = SceneFixture::valid();
        fixture.content.prop_slots[3].content = Some(
            crate::presentation::companion_scene::scene::PropSemanticContent {
                sprite_phase: Some(7),
                twinkle_active: Some(true),
                lid_open: Some(false),
                bloom_active: None,
                glyphs: std::array::from_fn(|index| {
                    crate::presentation::companion_scene::scene::PropGlyphContent {
                        glyph: (index == 4).then(|| AuthoredGlyph::new('◆').unwrap()),
                        local_cell: [index as i8 - 4, 2],
                    }
                }),
            },
        );
        fixture.content.tank_slots[1].content = Some(
            crate::presentation::companion_scene::scene::TankSemanticContent {
                sprite_variant: 9,
                morph: Some(6),
                color_srgb8: [126, 238, 255],
                bold: true,
                glyphs: std::array::from_fn(|index| {
                    (index == 7).then(|| AuthoredGlyph::new('◈').unwrap())
                }),
            },
        );
        let compiled = compile_static_fixture(&fixture);
        let prop = compiled.content.prop_glyphs.as_slice()[3 * 9 + 4];
        assert_eq!(prop.glyph_scalar, u32::from('◆'));
        assert_eq!(prop.signed_data, [0, 2]);
        assert_eq!(prop.flags, 2 | (1 << 2) | (8 << 8));
        assert_eq!(prop.variant, 0);
        let tank = compiled.content.tank_glyphs.as_slice()[8 + 7];
        assert_eq!(tank.glyph_scalar, u32::from('◈'));
        assert_eq!(tank.signed_data, [126 | (238 << 8) | (255 << 16), 1]);
        assert_eq!(tank.flags, 6);
        assert_eq!(tank.variant, 9);
        assert_eq!(
            compiled.content.prop_glyphs.as_slice()[0].glyph_scalar,
            NONE_U32
        );
        assert_eq!(compiled.content.tank_glyphs.as_slice()[0].variant, NONE_U32);
        assert_eq!(
            compiled.content.tank_glyphs.as_slice()[0].signed_data,
            [0; 2]
        );
    }

    #[test]
    fn every_catalog_tank_paint_survives_all_authored_sprite_records_and_layers() {
        const NO_MORPH: [Option<u8>; 1] = [None];
        const ANEMONE_MORPHS: [Option<u8>; 4] = [Some(0), Some(1), Some(2), Some(3)];

        for spec in crate::game::habitat::TANK_INHABITANT_CATALOG {
            let paint = crate::presentation::tank_life::tank_paint_for(spec.id)
                .expect("canonical tank paint");
            let morphs = if spec.id == crate::game::habitat::ANEMONE_HOST {
                ANEMONE_MORPHS.as_slice()
            } else {
                NO_MORPH.as_slice()
            };
            for variant in [0, 1] {
                for &morph in morphs {
                    let sprite =
                        crate::presentation::tank_life::tank_sprite_cells(spec.id, variant, morph);
                    assert!(
                        !sprite.is_empty(),
                        "{} variant {variant} morph {morph:?}",
                        spec.id
                    );
                    assert!(sprite.len() <= MAX_TANK_GLYPHS_PER_SLOT);

                    for layer in [InstanceLayer::Behind, InstanceLayer::Foreground] {
                        let mut fixture = SceneFixture::valid();
                        fixture.content.tank_slots[0].content = Some(
                            crate::presentation::companion_scene::scene::TankSemanticContent {
                                sprite_variant: variant,
                                morph,
                                color_srgb8: paint.color_srgb8,
                                bold: paint.bold,
                                glyphs: std::array::from_fn(|subslot| {
                                    sprite.get(subslot).map(|cell| {
                                        AuthoredGlyph::new(cell.glyph).expect("authored tank glyph")
                                    })
                                }),
                            },
                        );
                        fixture.frame.tank_slots[0] =
                            crate::presentation::companion_scene::scene::TankFrameSlot {
                                slot: 0,
                                visible: true,
                                origin_points: [0.0; 2],
                                cells: std::array::from_fn(|subslot| {
                                    if subslot < sprite.len() {
                                        crate::presentation::companion_scene::scene::TankCellFrame {
                                            visible: true,
                                            position_points: [subslot as f32, 0.0],
                                            layer,
                                            bounds_points: [subslot as f32, 0.0, 1.0, 1.0],
                                        }
                                    } else {
                                        crate::presentation::companion_scene::scene::TankCellFrame {
                                            visible: false,
                                            position_points: [0.0; 2],
                                            layer: InstanceLayer::Behind,
                                            bounds_points: [0.0; 4],
                                        }
                                    }
                                }),
                            };

                        let compiled = compile_fixture(&fixture);
                        let [red, green, blue] = paint.color_srgb8.map(i32::from);
                        let expected_paint =
                            [red | (green << 8) | (blue << 16), i32::from(paint.bold)];
                        for (subslot, source) in sprite.iter().enumerate() {
                            let content = compiled.content.tank_glyphs.as_slice()[subslot];
                            assert_eq!(content.glyph_scalar, u32::from(source.glyph));
                            assert_eq!(content.signed_data, expected_paint);
                            assert_eq!(content.flags, morph.map_or(NONE_U32, u32::from));
                            assert_eq!(content.variant, u32::from(variant));

                            let frame = compiled.frame.tank_cells.as_slice()[subslot];
                            assert_eq!(frame.flags, 3);
                            assert_eq!(
                                frame.variant,
                                instance_layer_tag(layer) | ((subslot as u32) << 16)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn tank_paint_delta_matches_fresh_compilation() {
        let mut changed = SceneFixture::valid();
        changed.content.tank_slots[1].content = Some(
            crate::presentation::companion_scene::scene::TankSemanticContent {
                sprite_variant: 1,
                morph: Some(2),
                color_srgb8: [232, 176, 208],
                bold: true,
                glyphs: [Some(AuthoredGlyph::new('◈').unwrap()); MAX_TANK_GLYPHS_PER_SLOT],
            },
        );
        let fresh = compile_fixture(&changed);

        let mut incremental = compile_fixture(&SceneFixture::valid());
        let (mut content, frame) = paired_deltas(
            &incremental,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.tank_slots.push(changed.content.tank_slots[1]);
        incremental.apply_deltas(&content, &frame).unwrap();

        assert_eq!(
            incremental.logical_content.tank_slots,
            fresh.logical_content.tank_slots
        );
        assert_eq!(incremental.content.tank_glyphs, fresh.content.tank_glyphs);
    }

    #[test]
    fn candidate_debug_redacts_exact_frame_globals() {
        fn fixture_with_private_frame(gauges: [f32; 4], dim: f32) -> SceneFixture {
            let mut fixture = SceneFixture::valid();
            fixture.frame.gauges = gauges;
            fixture.frame.dim_amount = dim;
            let dim_alias =
                crate::presentation::companion_scene::scene::CanonicalAlias::new("chrome.dim")
                    .unwrap();
            let dim_node = NodeId::from_alias(&dim_alias);
            let parent = fixture.template.nodes[0].id;
            fixture.template.nodes.push(
                crate::presentation::companion_scene::scene::NodeTemplate {
                    id: dim_node,
                    alias: dim_alias,
                    parent: Some(parent),
                    base_transform:
                        crate::presentation::companion_scene::scene::Transform3::IDENTITY,
                    local_bounds: Bounds3 { min: [0.0; 3], max: [360.0, 360.0, 0.0] },
                    depth_cue: crate::presentation::companion_scene::DepthCue::NEUTRAL,
                },
            );
            fixture.frame.nodes.push(
                crate::presentation::companion_scene::scene::NodeFrameState {
                    node: dim_node,
                    local_transform:
                        crate::presentation::companion_scene::scene::Transform3::IDENTITY,
                    visible: dim > 0.0,
                    opacity: dim,
                },
            );
            fixture
        }

        let first_gauges = [0.123_456_79, 0.234_567_9, 0.345_678_93, 0.456_789_02];
        let second_gauges = [0.176_543_2, 0.213_579_24, 0.398_765_44, 0.487_654_33];
        let first_dim = 0.567_890_1;
        let second_dim = 0.678_901_2;
        let first = compile_fixture(&fixture_with_private_frame(first_gauges, first_dim));
        let second = compile_fixture(&fixture_with_private_frame(second_gauges, second_dim));
        let first_debug = format!("{first:?}");
        let second_debug = format!("{second:?}");
        let first_node_debug = format!("{:?}", first.frame.nodes.as_slice());
        let second_node_debug = format!("{:?}", second.frame.nodes.as_slice());

        for private_value in first_gauges
            .into_iter()
            .chain(second_gauges)
            .chain([first_dim, second_dim])
            .map(|value| format!("{value:?}"))
        {
            assert!(
                !first_debug.contains(&private_value),
                "leaked {private_value}: {first_debug}"
            );
            assert!(
                !second_debug.contains(&private_value),
                "leaked {private_value}: {second_debug}"
            );
            assert!(!first_node_debug.contains(&private_value));
            assert!(!second_node_debug.contains(&private_value));
        }
        assert!(first_debug.contains("<redacted>"));
    }

    #[test]
    fn initial_mirrors_preserve_globals_fixed_slots_lights_and_empty_tail() {
        let mut fixture = SceneFixture::valid();
        fixture.content.palette[2] = [11, 22, 33];
        fixture.content.mood = MoodContentKind::Happy;
        fixture.content.weather = WeatherContentKind::Mixed;
        fixture.content.pet_art_slots[0].palette_role = PetPaletteRole::Eye;
        fixture.content.ambient_slots[2].kind = Some(AmbientContentKind::Mote);
        fixture.content.ambient_slots[2].glyph = Some(AuthoredGlyph::new('☁').unwrap());

        fixture.frame.gauges = [0.1, 0.2, 0.3, 0.4];
        fixture.frame.dim_amount = 0.35;
        fixture.frame.prop_slots[2].visible = true;
        fixture.frame.prop_slots[2].origin_points = [10.0, 20.0];
        fixture.frame.prop_slots[2].motion_offset_points = [1.0, 2.0];
        fixture.frame.prop_slots[2].opacity = 0.75;
        fixture.frame.prop_slots[2].footprint_points = [12.0, 14.0];
        fixture.frame.prop_slots[2].contact_shadow_strength = 0.6;
        fixture.frame.tank_slots[1].visible = true;
        fixture.frame.tank_slots[1].origin_points = [30.0, 40.0];
        fixture.frame.tank_slots[1].cells[3] =
            crate::presentation::companion_scene::scene::TankCellFrame {
                visible: true,
                position_points: [5.0, 6.0],
                layer: InstanceLayer::Foreground,
                bounds_points: [1.0, 2.0, 3.0, 4.0],
            };
        fixture.frame.ambient_slots[2].visible = true;
        fixture.frame.ambient_slots[2].position_points = [70.0, 80.0];
        fixture.frame.ambient_slots[2].opacity = 0.6;
        fixture
            .frame
            .lights
            .push(crate::presentation::companion_scene::scene::LightFrame {
                direction: [0.0, 1.0, 0.0],
                color_linear: [0.2, 0.3, 0.4],
                intensity: 2.0,
            });

        let compiled = compile_static_fixture(&fixture);
        let globals = compiled.content.globals.as_slice()[0];
        assert_eq!(globals.palette_rgba[2], [11, 22, 33, 255]);
        assert_eq!(globals.mood, 1);
        assert_eq!(globals.weather, 5);
        assert_eq!(
            compiled.content.pet_body.as_slice()[0].glyph_scalar,
            u32::from('^')
        );
        assert_eq!(compiled.content.pet_body.as_slice()[0].flags, 3);
        assert_eq!(
            compiled.content.ambient.as_slice()[2].glyph_scalar,
            u32::from('☁')
        );
        assert_eq!(compiled.content.ambient.as_slice()[2].flags, 1);
        assert_eq!(compiled.content.ambient.as_slice()[2].variant, 0);

        let frame_globals = compiled.frame.globals.as_slice()[0];
        assert_eq!(frame_globals.viewport_points, [360.0, 360.0]);
        assert_eq!(frame_globals.viewport_pixels, [0.0; 2]);
        assert_eq!(frame_globals.aperture, [0.0; 4]);
        assert_eq!(frame_globals.gauges, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(frame_globals.dim_amount, 0.35);
        assert_eq!(frame_globals.light_count, 1);
        assert_eq!(
            compiled.frame.props.as_slice()[2].values,
            [10.0, 20.0, 1.0, 2.0, 0.75, 12.0, 14.0, 0.6, 0.0, 0.0, 0.0, 0.0]
        );
        let tank = compiled.frame.tank_cells.as_slice()[MAX_TANK_GLYPHS_PER_SLOT + 3];
        assert_eq!(tank.slot, 1);
        assert_eq!(tank.variant, 2 | (3 << 16));
        assert_eq!(
            tank.values,
            [30.0, 40.0, 5.0, 6.0, 1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            compiled.frame.ambient.as_slice()[2].values[..3],
            [70.0, 80.0, 0.6]
        );
        assert_eq!(compiled.frame.lights.as_slice()[0].flags, 1);
        assert_eq!(compiled.frame.lights.as_slice()[1].variant, NONE_U32);
        assert_eq!(compiled.frame.nodes.as_slice()[2], NodeGpuValue::zeroed());
    }

    #[test]
    fn ambient_kind_tag_is_the_closed_forward_only_v2_abi() {
        assert_eq!(ambient_kind_tag(AmbientContentKind::Mote), 1);
    }

    #[test]
    fn attachment_tables_preserve_binding_source_and_local_transform() {
        let mut fixture = SceneFixture::valid();
        fixture.template.primitives[0].binding =
            PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(4));
        fixture.template.attachments[0].instance_binding =
            Some(AttachmentInstanceBinding::PropGlyphs(4));
        fixture.template.attachments[0].mode = AttachmentMode::SnapshotWorldOnSpawn;
        fixture.template.attachments[0].local =
            crate::presentation::companion_scene::scene::Transform3::translated([3.0, 5.0, 0.25]);

        let compiled = compile_static_fixture(&fixture);
        let attachment = compiled.attachments[0];
        assert_eq!(
            compiled
                .index
                .attachment_offset(fixture.template.attachments[0].id),
            Some(0)
        );
        assert_eq!(attachment.attachment_dense_index, 0);
        assert_eq!(attachment.owner_dense_index, 1);
        assert_eq!(attachment.local_transform[3], [3.0, 5.0, 0.25, 1.0]);
        assert_eq!(attachment.mode, 2);
        assert_eq!(attachment.instance_binding, 1);
        assert_eq!(attachment.instance_slot, 4);
        assert_eq!(attachment.source_primitive_dense_index, 0);
        assert_eq!(attachment.source_node_dense_index, 1);
    }

    #[test]
    fn every_immutable_template_family_changes_static_checksum_and_compiled_data() {
        let fixture = SceneFixture::valid();
        let baseline = compile_fixture(&fixture);

        let mut attachment = fixture.clone();
        attachment.template.attachments[0].local =
            crate::presentation::companion_scene::scene::Transform3::translated([1.0, 0.0, 0.0]);
        let attachment = compile_fixture(&attachment);
        assert_ne!(attachment.static_checksum, baseline.static_checksum);
        assert_ne!(attachment.attachments, baseline.attachments);

        let mut material = fixture.clone();
        let material_alias =
            crate::presentation::companion_scene::scene::CanonicalAlias::new("material.unused")
                .unwrap();
        material.template.materials.push(
            crate::presentation::companion_scene::scene::MaterialTemplate {
                id: crate::presentation::companion_scene::scene::MaterialId::from_alias(
                    &material_alias,
                ),
                alias: material_alias,
                kind: MaterialKind::AdditiveGlow,
            },
        );
        let material_a = compile_fixture(&material);
        material.template.materials.last_mut().unwrap().kind = MaterialKind::MultiplyShadow;
        let material_b = compile_fixture(&material);
        assert_ne!(material_a.static_checksum, material_b.static_checksum);
        assert_ne!(material_a.materials, material_b.materials);

        let mut resource = fixture.clone();
        let resource_alias =
            crate::presentation::companion_scene::scene::CanonicalAlias::new("resource.unused")
                .unwrap();
        resource.template.resources.push(
            crate::presentation::companion_scene::scene::ResourceTemplate {
                id: crate::presentation::companion_scene::scene::ResourceId::from_alias(
                    &resource_alias,
                ),
                alias: resource_alias,
                kind: ResourceKind::ColorAtlas,
            },
        );
        let resource_a = compile_fixture(&resource);
        resource.template.resources.last_mut().unwrap().kind = ResourceKind::AnalyticGeometry;
        let resource_b = compile_fixture(&resource);
        assert_ne!(resource_a.static_checksum, resource_b.static_checksum);
        assert_ne!(resource_a.resources, resource_b.resources);

        let mut depth_cue = fixture.clone();
        depth_cue.template.nodes[1].depth_cue = crate::presentation::companion_scene::DepthCue {
            scale: 0.9,
            y_offset_points_up: 3.0,
            opacity: 0.8,
            saturation: 0.7,
        };
        let depth_cue = compile_fixture(&depth_cue);
        assert_ne!(depth_cue.static_checksum, baseline.static_checksum);
        assert_eq!(depth_cue.nodes[1].depth_cue, [0.9, 3.0, 0.8, 0.7]);
        assert_eq!(
            depth_cue.frame.nodes.as_slice()[1].depth_cue,
            [0.9, 3.0, 0.8, 0.7]
        );

        let mut max_z = fixture.clone();
        max_z.template.primitives[0].local_geometry.max[2] = 0.75;
        let max_z = compile_fixture(&max_z);
        assert_ne!(max_z.static_checksum, baseline.static_checksum);
        assert_eq!(max_z.primitives[0].local_bounds_max[2], 0.75);
    }

    #[test]
    fn paired_delta_identity_revision_and_late_validation_failures_are_atomic() {
        let mut candidate = compile_fixture(&SceneFixture::valid());

        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.palette = Some([[9; 3]; 8]);
        let mut invalid_node = candidate.accepted.frame().frame().nodes[1];
        invalid_node.opacity = 2.0;
        frame.nodes.push(invalid_node);
        let before = candidate.clone();
        assert!(matches!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::Validation(_))
        ));
        assert_eq!(candidate, before);

        let (mut content, frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.generation_key.device = crate::presentation::companion_scene::DeviceEpoch(99);
        assert_eq!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::GenerationMismatch)
        );
        assert_eq!(candidate, before);

        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        frame.to = crate::presentation::companion_scene::AppliedRevisions::new(5, 7);
        assert_eq!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::PairMismatch)
        );

        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.from = crate::presentation::companion_scene::AppliedRevisions::new(3, 4);
        frame.from = content.from;
        assert_eq!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::StaleBase)
        );

        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(3, 4),
        );
        content.to = crate::presentation::companion_scene::AppliedRevisions::new(3, 4);
        frame.to = content.to;
        assert_eq!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::InvalidRevisionAdvance)
        );

        let (mut content, frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
        );
        let mut pet = candidate.logical_content.pet_art_slots[0];
        pet.palette_role = PetPaletteRole::Eye;
        content.pet_art_slots.push(pet);
        assert_eq!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::InvalidRevisionAdvance)
        );

        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
        );
        let mut missing = candidate.accepted.frame().frame().nodes[0];
        missing.node = NodeId(u32::MAX);
        frame.nodes.push(missing);
        assert!(matches!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::Validation(_))
        ));
        assert_eq!(candidate, before);
    }

    #[test]
    fn paired_room_and_analytic_deltas_update_v2_mirrors_and_invalid_frame_is_atomic() {
        let mut candidate = compile_fixture(&SceneFixture::valid());
        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        let mut content_slot = candidate.logical_content.room_glyph_slots[0];
        content_slot.glyph = Some(AuthoredGlyph::new('\u{25c7}').unwrap());
        content_slot.color_srgb8 = Some([10, 20, 30]);
        content.room_glyph_slots.push(content_slot);
        let mut analytic_content = candidate.logical_content.analytic_slots[0];
        analytic_content.value.as_mut().unwrap().paint = AnalyticPaint::ApertureDepth {
            core_srgb8: [31, 32, 33],
            rim_srgb8: [34, 35, 36],
            bed_srgb8: [37, 38, 39],
            fleck_srgb8: [40, 41, 42],
        };
        content.analytic_slots.push(analytic_content);
        let mut frame_slot = candidate.accepted.frame().frame().room_glyph_slots[0];
        frame_slot.visible = true;
        frame_slot.grid_cell = [1, 2];
        frame_slot.position_points = [12.0, 324.0];
        frame_slot.opacity = 0.75;
        frame.room_glyph_slots.push(frame_slot);
        let mut analytic_frame = candidate.accepted.frame().frame().analytic_slots[0];
        let AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points,
        } = analytic_frame.value.unwrap().geometry
        else {
            panic!("fixture slot zero is the room-background aperture");
        };
        analytic_frame.value.as_mut().unwrap().geometry = AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points: feather_points + 0.5,
        };
        frame.analytic_slots.push(analytic_frame);

        let dirty = candidate.apply_deltas(&content, &frame).unwrap();
        assert_eq!(
            dirty.room_content.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.room_frame.as_slice(),
            &[ByteSpan::slots::<FrameGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.content_analytics.as_slice(),
            &[ByteSpan::slots::<AnalyticContentGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.frame_analytics.as_slice(),
            &[ByteSpan::slots::<AnalyticFrameGpuValue>(0, 1)]
        );
        assert_eq!(
            candidate.content.room_glyphs.as_slice()[0],
            pack_room_content(content_slot)
        );
        assert_eq!(
            candidate.frame.room_glyphs.as_slice()[0],
            pack_room_frame(frame_slot)
        );
        assert_eq!(
            candidate.content.analytics.as_slice()[0],
            pack_analytic_content(analytic_content)
        );
        assert_eq!(
            candidate.frame.analytics.as_slice()[0],
            pack_analytic_frame(analytic_frame)
        );

        let before = candidate.clone();
        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 7),
        );
        let mut malformed = frame_slot;
        malformed.position_points = [13.0, 324.0];
        frame.room_glyph_slots.push(malformed);
        assert!(matches!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::Validation(_))
        ));
        assert_eq!(candidate, before);
    }

    #[test]
    fn paired_delta_emits_exact_family_spans_and_allows_packed_noop_revision_advance() {
        let mut fixture = SceneFixture::valid();
        fixture
            .frame
            .lights
            .push(crate::presentation::companion_scene::scene::LightFrame {
                direction: [0.0, 1.0, 0.0],
                color_linear: [0.2, 0.3, 0.4],
                intensity: 1.0,
            });
        let mut candidate = compile_fixture(&fixture);
        let before_frame_globals = candidate.frame.globals.as_slice()[0];
        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.palette = Some([[17; 3]; 8]);
        content.day_phase = Some(crate::presentation::companion_scene::CompanionDayPhase::Dusk);
        let mut pet = candidate.logical_content.pet_art_slots[0];
        pet.palette_role = PetPaletteRole::Eye;
        content.pet_art_slots.push(pet);
        content.prop_slots.push(
            crate::presentation::companion_scene::scene::PropContentSlot {
                slot: 3,
                content: Some(
                    crate::presentation::companion_scene::scene::PropSemanticContent {
                        sprite_phase: Some(2),
                        twinkle_active: Some(true),
                        lid_open: None,
                        bloom_active: None,
                        glyphs: std::array::from_fn(|subslot| {
                            crate::presentation::companion_scene::scene::PropGlyphContent {
                                glyph: (subslot == 0).then(|| AuthoredGlyph::new('◆').unwrap()),
                                local_cell: [0, 0],
                            }
                        }),
                    },
                ),
            },
        );
        let mut prop_paint = candidate.logical_content.prop_paint_slots[3];
        prop_paint.paints[0] = Some(
            crate::presentation::companion_scene::scene::GlyphPaintSource {
                color_srgb8: [40, 50, 60],
            },
        );
        content.prop_paint_slots.push(prop_paint);
        content.tank_slots.push(
            crate::presentation::companion_scene::scene::TankContentSlot {
                slot: 1,
                content: Some(
                    crate::presentation::companion_scene::scene::TankSemanticContent {
                        sprite_variant: 3,
                        morph: Some(1),
                        color_srgb8: [126, 238, 255],
                        bold: true,
                        glyphs: [Some(AuthoredGlyph::new('◈').unwrap()); MAX_TANK_GLYPHS_PER_SLOT],
                    },
                ),
            },
        );
        content.ambient_slots.push(
            crate::presentation::companion_scene::scene::AmbientContentSlot {
                slot: 2,
                kind: Some(AmbientContentKind::Mote),
                glyph: Some(AuthoredGlyph::new('☁').unwrap()),
            },
        );
        let mut ambient_paint = candidate.logical_content.ambient_paint_slots[2];
        ambient_paint.paint = Some(
            crate::presentation::companion_scene::scene::GlyphPaintSource {
                color_srgb8: [70, 80, 90],
            },
        );
        content.ambient_paint_slots.push(ambient_paint);

        let mut node = candidate.accepted.frame().frame().nodes[1];
        node.local_transform =
            crate::presentation::companion_scene::scene::Transform3::translated([2.0, 0.0, 0.0]);
        frame.nodes.push(node);
        frame.gauges = Some([0.1, 0.2, 0.3, 0.4]);
        frame.dim_amount = Some(0.25);
        frame
            .prop_slots
            .push(crate::presentation::companion_scene::scene::PropFrameSlot {
                slot: 3,
                visible: true,
                origin_points: [10.0, 20.0],
                motion_offset_points: [1.0, 2.0],
                opacity: 0.8,
                footprint_points: [12.0, 14.0],
                contact_shadow_strength: 0.6,
                cast_shadow_vector_points: [0.0; 2],
                cast_shadow_softness_points: 0.0,
                cast_shadow_strength: 0.0,
            });
        let mut tank = candidate.accepted.frame().frame().tank_slots[1];
        tank.visible = true;
        tank.origin_points = [30.0, 40.0];
        tank.cells = std::array::from_fn(|subslot| {
            crate::presentation::companion_scene::scene::TankCellFrame {
                visible: true,
                position_points: [subslot as f32, 4.0],
                layer: if subslot == 0 {
                    InstanceLayer::Foreground
                } else {
                    InstanceLayer::Behind
                },
                bounds_points: [subslot as f32, 4.0, 1.0, 1.0],
            }
        });
        frame.tank_slots.push(tank);
        frame.ambient_slots.push(
            crate::presentation::companion_scene::scene::AmbientFrameSlot {
                slot: 2,
                visible: true,
                position_points: [50.0, 60.0],
                opacity: 0.7,
            },
        );
        frame.lights.push((
            0,
            crate::presentation::companion_scene::scene::LightFrame {
                direction: [1.0, 0.0, 0.0],
                color_linear: [0.4, 0.3, 0.2],
                intensity: 2.0,
            },
        ));

        let dirty = candidate.apply_deltas(&content, &frame).unwrap();
        assert_eq!(candidate.last_node_resolves, 1);
        assert_eq!(
            dirty.content_globals.as_slice(),
            &[ByteSpan::slots::<ContentGlobalsGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.pet_body.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.pet_particles.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.prop_glyphs.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(27, 9)]
        );
        assert_eq!(
            dirty.tank_glyphs.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(8, 8)]
        );
        assert_eq!(
            dirty.content_ambient.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(2, 1)]
        );
        assert_eq!(
            dirty.frame_globals.as_slice(),
            &[ByteSpan::slots::<FrameGlobalsGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.nodes.as_slice(),
            &[ByteSpan::slots::<NodeGpuValue>(1, 1)]
        );
        assert_eq!(
            dirty.props.as_slice(),
            &[ByteSpan::slots::<FrameGpuValue>(3, 1)]
        );
        assert_eq!(
            dirty.tank_cells.as_slice(),
            &[ByteSpan::slots::<FrameGpuValue>(8, 8)]
        );
        assert_eq!(
            dirty.frame_ambient.as_slice(),
            &[ByteSpan::slots::<FrameGpuValue>(2, 1)]
        );
        assert_eq!(
            dirty.lights.as_slice(),
            &[ByteSpan::slots::<FrameGpuValue>(0, 1)]
        );
        let after_frame_globals = candidate.frame.globals.as_slice()[0];
        assert_eq!(
            after_frame_globals.viewport_pixels,
            before_frame_globals.viewport_pixels
        );
        assert_eq!(after_frame_globals.aperture, before_frame_globals.aperture);
        assert_eq!(
            after_frame_globals.activity_opacity,
            before_frame_globals.activity_opacity
        );
        assert_eq!(
            after_frame_globals.reduce_motion,
            before_frame_globals.reduce_motion
        );
        assert_eq!(after_frame_globals.light_count, 1);
        assert_eq!(
            candidate.logical_content.day_phase,
            crate::presentation::companion_scene::CompanionDayPhase::Dusk
        );
        assert_eq!(
            candidate.content.prop_glyphs.as_slice()[3 * MAX_PROP_GLYPHS_PER_SLOT].variant,
            packed_rgba([40, 50, 60, 255])
        );
        assert_eq!(
            candidate.content.ambient.as_slice()[2].variant,
            packed_rgba([70, 80, 90, 255])
        );

        let (mut noop_content, noop_frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(6, 7),
        );
        noop_content
            .pet_art_slots
            .push(candidate.logical_content.pet_art_slots[0]);
        let noop = candidate.apply_deltas(&noop_content, &noop_frame).unwrap();
        assert!(all_dirty_sets_are_empty(&noop));
        assert_eq!(candidate.source_revisions, noop.to);
    }

    #[test]
    fn scene_delta_prepare_is_read_only_and_commit_matches_atomic_wrapper() {
        let mut candidate = compile_fixture(&SceneFixture::valid());
        let mut expected = candidate.clone();
        let before = candidate.clone();
        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.palette = Some([[17; 3]; 8]);
        let mut node = candidate.accepted.frame().frame().nodes[0];
        node.local_transform =
            crate::presentation::companion_scene::scene::Transform3::translated([2.0, 3.0, 0.0]);
        frame.nodes.push(node);

        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();
        assert_eq!(candidate, before);

        let expected_dirty = expected.apply_deltas(&content, &frame).unwrap();
        let applied = candidate.commit_prepared(prepared);
        assert_eq!(applied.dirty, expected_dirty);
        assert_eq!(applied.generation_key, candidate.generation_key);
        assert_eq!(applied.static_checksum, candidate.static_checksum);
        assert_eq!(applied.to, content.to);
        assert_eq!(
            applied.prospective_logical_viewport_points,
            candidate.logical_viewport_points()
        );
        assert_eq!(candidate, expected);
    }

    #[test]
    fn scene_delta_prepare_reports_exact_prospective_camera_without_mutation() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let before = candidate.clone();
        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
        );
        let mut camera = candidate.accepted.frame().frame().camera;
        camera.far_z = -4.0;
        camera.near_z = 4.0;
        frame.camera = Some(camera);

        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();
        assert_eq!(
            prepared.prospective_logical_viewport_points(),
            [360.0, 360.0]
        );
        assert_eq!(candidate, before);
    }

    #[test]
    fn prepared_scene_delta_exposes_sealed_dirty_spans_and_exact_record_bytes() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.palette = Some([[17; 3]; 8]);
        let mut room_content = candidate.logical_content.room_glyph_slots[0];
        room_content.glyph = Some(AuthoredGlyph::new('\u{25c7}').unwrap());
        room_content.color_srgb8 = Some([10, 20, 30]);
        content.room_glyph_slots.push(room_content);
        let mut analytic_content = candidate.logical_content.analytic_slots[0];
        analytic_content.value.as_mut().unwrap().paint = AnalyticPaint::ApertureDepth {
            core_srgb8: [31, 32, 33],
            rim_srgb8: [34, 35, 36],
            bed_srgb8: [37, 38, 39],
            fleck_srgb8: [40, 41, 42],
        };
        content.analytic_slots.push(analytic_content);

        let mut node = candidate.accepted.frame().frame().nodes[0];
        node.local_transform =
            crate::presentation::companion_scene::scene::Transform3::translated([2.0, 0.0, 0.0]);
        frame.nodes.push(node);
        frame.gauges = Some([0.1, 0.2, 0.3, 0.4]);
        let mut room_frame = candidate.accepted.frame().frame().room_glyph_slots[0];
        room_frame.visible = true;
        room_frame.grid_cell = [1, 2];
        room_frame.position_points = [12.0, 324.0];
        room_frame.opacity = 0.75;
        frame.room_glyph_slots.push(room_frame);
        let mut analytic_frame = candidate.accepted.frame().frame().analytic_slots[0];
        let AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points,
        } = analytic_frame.value.unwrap().geometry
        else {
            panic!("fixture slot zero is the room-background aperture");
        };
        analytic_frame.value.as_mut().unwrap().geometry = AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points: feather_points + 0.5,
        };
        frame.analytic_slots.push(analytic_frame);

        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();
        let dirty = prepared.dirty_spans();
        assert_eq!(
            dirty.content_globals.as_slice(),
            &[ByteSpan::slots::<ContentGlobalsGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.room_content.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.content_analytics.as_slice(),
            &[ByteSpan::slots::<AnalyticContentGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.frame_globals.as_slice(),
            &[ByteSpan::slots::<FrameGlobalsGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.nodes.as_slice(),
            &[ByteSpan::slots::<NodeGpuValue>(0, 2)]
        );
        assert_eq!(
            dirty.room_frame.as_slice(),
            &[ByteSpan::slots::<FrameGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.frame_analytics.as_slice(),
            &[ByteSpan::slots::<AnalyticFrameGpuValue>(0, 1)]
        );
        assert_eq!(
            dirty.metrics(),
            SceneDirtyMetrics {
                content_ranges: 3,
                content_bytes: (std::mem::size_of::<ContentGlobalsGpuValue>()
                    + std::mem::size_of::<ContentGpuValue>()
                    + std::mem::size_of::<AnalyticContentGpuValue>())
                    as u64,
                frame_ranges: 4,
                frame_bytes: (2 * std::mem::size_of::<NodeGpuValue>()
                    + std::mem::size_of::<FrameGlobalsGpuValue>()
                    + std::mem::size_of::<FrameGpuValue>()
                    + std::mem::size_of::<AnalyticFrameGpuValue>())
                    as u64,
            }
        );

        let mut visited = Vec::new();
        prepared.visit_mirror_updates(|family, slot, bytes| {
            visited.push((family, slot, bytes.to_vec()));
        });
        assert_eq!(
            visited
                .iter()
                .map(|(family, slot, bytes)| (*family, *slot, bytes.len()))
                .collect::<Vec<_>>(),
            vec![
                (
                    PreparedMirrorFamily::Nodes,
                    0,
                    std::mem::size_of::<NodeGpuValue>()
                ),
                (
                    PreparedMirrorFamily::Nodes,
                    1,
                    std::mem::size_of::<NodeGpuValue>()
                ),
                (
                    PreparedMirrorFamily::ContentGlobals,
                    0,
                    std::mem::size_of::<ContentGlobalsGpuValue>(),
                ),
                (
                    PreparedMirrorFamily::RoomContent,
                    0,
                    std::mem::size_of::<ContentGpuValue>(),
                ),
                (
                    PreparedMirrorFamily::ContentAnalytics,
                    0,
                    std::mem::size_of::<AnalyticContentGpuValue>(),
                ),
                (
                    PreparedMirrorFamily::FrameGlobals,
                    0,
                    std::mem::size_of::<FrameGlobalsGpuValue>(),
                ),
                (
                    PreparedMirrorFamily::RoomFrame,
                    0,
                    std::mem::size_of::<FrameGpuValue>(),
                ),
                (
                    PreparedMirrorFamily::FrameAnalytics,
                    0,
                    std::mem::size_of::<AnalyticFrameGpuValue>(),
                ),
            ]
        );
        assert_eq!(
            visited[3].2,
            bytemuck::bytes_of(&pack_room_content(room_content))
        );
        assert_eq!(
            visited[4].2,
            bytemuck::bytes_of(&pack_analytic_content(analytic_content))
        );
        assert_eq!(
            visited[6].2,
            bytemuck::bytes_of(&pack_room_frame(room_frame))
        );
        assert_eq!(
            visited[7].2,
            bytemuck::bytes_of(&pack_analytic_frame(analytic_frame))
        );
    }

    #[test]
    fn prepared_noop_revision_advance_commits_empty_spans() {
        let mut candidate = compile_fixture(&SceneFixture::valid());
        let storage = candidate_storage_identity(&candidate);
        let (content, frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );

        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();
        let applied = candidate.commit_prepared(prepared);

        assert!(all_dirty_sets_are_empty(&applied.dirty));
        assert_eq!(candidate.source_revisions, content.to);
        assert_eq!(applied.to, content.to);
        assert_candidate_storage_identity(storage, &candidate);
    }

    #[test]
    fn prepared_scene_delta_owns_values_after_caller_mutation() {
        let mut candidate = compile_fixture(&SceneFixture::valid());
        let mut expected = candidate.clone();
        let (mut content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        content.palette = Some([[17; 3]; 8]);
        content.day_phase = Some(crate::presentation::companion_scene::CompanionDayPhase::Dusk);
        content
            .pet_art_slots
            .push(candidate.logical_content.pet_art_slots[0]);
        frame.gauges = Some([0.25, 0.5, 0.75, 1.0]);

        let expected_dirty = expected.apply_deltas(&content, &frame).unwrap();
        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();

        content.palette = Some([[99; 3]; 8]);
        content.day_phase = Some(crate::presentation::companion_scene::CompanionDayPhase::Night);
        content.pet_art_slots[0].slot = u16::MAX;
        content.to = crate::presentation::companion_scene::AppliedRevisions::new(500, 600);
        frame.gauges = Some([1.0; 4]);
        frame
            .prop_slots
            .push(crate::presentation::companion_scene::scene::PropFrameSlot {
                slot: u8::MAX,
                visible: true,
                origin_points: [1.0; 2],
                motion_offset_points: [1.0; 2],
                opacity: 1.0,
                footprint_points: [0.0; 2],
                contact_shadow_strength: 0.0,
                cast_shadow_vector_points: [0.0; 2],
                cast_shadow_softness_points: 0.0,
                cast_shadow_strength: 0.0,
            });
        frame.to = content.to;

        let applied = candidate.commit_prepared(prepared);
        assert_eq!(applied.dirty, expected_dirty);
        assert_eq!(candidate, expected);
    }

    #[test]
    fn prepared_scene_delta_survives_moving_candidate_and_caller_deltas() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let (content, frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();

        let moved_content = Box::new(content);
        let moved_frame = Box::new(frame);
        drop((moved_content, moved_frame));
        let mut candidate = Box::new(candidate);
        let applied = candidate.commit_prepared(prepared);

        assert_eq!(
            applied.to,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6)
        );
        assert_eq!(candidate.source_revisions, applied.to);
    }

    #[test]
    fn prepared_scene_delta_retains_source_identity_and_clones_get_fresh_identity() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let source_identity = std::sync::Arc::downgrade(&candidate.instance_identity.0);
        let (content, frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();
        let mut cloned_candidate = candidate.clone();
        let before = cloned_candidate.clone();

        assert!(!std::sync::Arc::ptr_eq(
            &candidate.instance_identity.0,
            &cloned_candidate.instance_identity.0,
        ));
        drop(candidate);
        assert!(source_identity.upgrade().is_some());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cloned_candidate.commit_prepared(prepared);
        }));

        assert!(result.is_err());
        assert!(source_identity.upgrade().is_none());
        assert_eq!(cloned_candidate, before);
    }

    #[test]
    fn prepared_scene_delta_cannot_commit_to_a_cloned_candidate() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let (content, frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(5, 6),
        );
        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();
        let mut cloned_candidate = candidate.clone();
        let before = cloned_candidate.clone();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cloned_candidate.commit_prepared(prepared);
        }));

        assert!(result.is_err());
        assert_eq!(cloned_candidate, before);
    }

    #[test]
    fn node_delta_recomputes_subtrees_in_topological_order_and_preserves_immutable_fields() {
        let mut fixture = SceneFixture::valid();
        let sibling_alias =
            crate::presentation::companion_scene::scene::CanonicalAlias::new("pet.sibling")
                .unwrap();
        let sibling_id = NodeId::from_alias(&sibling_alias);
        let root_id = fixture.template.nodes[0].id;
        let mut sibling = fixture.template.nodes[1].clone();
        sibling.id = sibling_id;
        sibling.alias = sibling_alias;
        sibling.parent = Some(root_id);
        sibling.base_transform =
            crate::presentation::companion_scene::scene::Transform3::translated([20.0, 0.0, 0.0]);
        fixture.template.nodes.push(sibling);
        fixture.frame.nodes.push(
            crate::presentation::companion_scene::scene::NodeFrameState {
                node: sibling_id,
                local_transform: crate::presentation::companion_scene::scene::Transform3::IDENTITY,
                visible: true,
                opacity: 1.0,
            },
        );
        let root = fixture.template.nodes.remove(0);
        fixture.template.nodes.push(root);
        let child_id = fixture.template.nodes[0].id;
        fixture.template.nodes[0].depth_cue.opacity = 0.5;

        let mut candidate = compile_fixture(&fixture);
        let mut compiled_child = candidate.frame.nodes.as_slice()[0];
        compiled_child.material_parameter_offset = 7;
        compiled_child.material_parameter_count = 3;
        candidate.frame.nodes.set_fixed(0, compiled_child);

        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
        );
        let mut child = candidate.accepted.frame().frame().nodes[0];
        assert_eq!(child.node, child_id);
        child.local_transform =
            crate::presentation::companion_scene::scene::Transform3::translated([3.0, 0.0, 0.0]);
        child.opacity = 0.8;
        child.visible = false;
        frame.nodes.push(child);
        let dirty = candidate.apply_deltas(&content, &frame).unwrap();
        assert_eq!(candidate.last_node_resolves, 1);
        assert_eq!(
            dirty.nodes.as_slice(),
            &[ByteSpan::slots::<NodeGpuValue>(0, 1)]
        );
        let child_gpu = candidate.frame.nodes.as_slice()[0];
        assert_eq!(child_gpu.world[3][0], 3.0);
        assert_eq!(child_gpu.opacity, 0.4);
        assert_eq!(child_gpu.visible, 0);
        assert_eq!(child_gpu.material_parameter_offset, 7);
        assert_eq!(child_gpu.material_parameter_count, 3);
        assert_eq!(candidate.frame.nodes.as_slice()[1].visible, 1);

        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 7),
        );
        let root_dense = usize::try_from(candidate.index.node_offset(root_id).unwrap()).unwrap();
        let mut root = candidate.accepted.frame().frame().nodes[root_dense];
        root.visible = false;
        root.opacity = 0.5;
        frame.nodes.push(root);
        let dirty = candidate.apply_deltas(&content, &frame).unwrap();
        assert_eq!(candidate.last_node_resolves, 3);
        assert_eq!(
            dirty.nodes.as_slice(),
            &[ByteSpan::slots::<NodeGpuValue>(0, 3)]
        );
        assert_eq!(candidate.frame.nodes.as_slice()[0].opacity, 0.2);
        assert_eq!(candidate.frame.nodes.as_slice()[1].opacity, 0.5);
        assert!(candidate.frame.nodes.as_slice()[..3]
            .iter()
            .all(|node| node.visible == 0));
    }

    #[test]
    fn permuted_slot_vectors_are_deterministic_and_duplicates_fail_atomically() {
        let candidate = compile_fixture(&SceneFixture::valid());
        let mut a = candidate.clone();
        let mut b = candidate.clone();
        let to = crate::presentation::companion_scene::AppliedRevisions::new(5, 5);
        let (mut content_a, frame_a) = paired_deltas(&a, to);
        let mut slot_0 = a.logical_content.pet_art_slots[0];
        slot_0.palette_role = PetPaletteRole::Eye;
        let mut slot_1 = a.logical_content.pet_art_slots[1];
        slot_1.glyph = Some(
            crate::presentation::companion_scene::scene::PetGlyph::for_species(
                '^',
                crate::pet::generation::Species::Fuzz,
            )
            .unwrap(),
        );
        slot_1.palette_role = PetPaletteRole::Accent;
        content_a.pet_art_slots.extend([slot_1, slot_0]);
        let (mut content_b, frame_b) = paired_deltas(&b, to);
        content_b.pet_art_slots.extend([slot_0, slot_1]);
        assert_eq!(
            a.apply_deltas(&content_a, &frame_a).unwrap(),
            b.apply_deltas(&content_b, &frame_b).unwrap()
        );
        assert_eq!(a, b);

        let mut duplicate_candidate = candidate;
        let before = duplicate_candidate.clone();
        let (mut duplicate, frame) = paired_deltas(&duplicate_candidate, to);
        duplicate.pet_art_slots.extend([slot_0, slot_0]);
        assert!(matches!(
            duplicate_candidate.apply_deltas(&duplicate, &frame),
            Err(MirrorDeltaError::Validation(_))
        ));
        assert_eq!(duplicate_candidate, before);
    }

    #[test]
    fn nonfinite_composed_world_is_rejected_atomically_even_for_unlit_nodes() {
        let mut fixture = SceneFixture::valid();
        fixture.template.nodes[1].base_transform.scale = [f32::MAX, 1.0, 1.0];
        let mut candidate = compile_fixture(&fixture);
        let before = candidate.clone();
        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
        );
        let mut root = candidate.accepted.frame().frame().nodes[0];
        root.local_transform.scale = [f32::MAX, 1.0, 1.0];
        frame.nodes.push(root);
        assert_eq!(
            candidate.apply_deltas(&content, &frame),
            Err(MirrorDeltaError::Compile(CompileError::InvalidTransform))
        );
        assert_eq!(candidate, before);

        let mut invalid_initial = fixture;
        invalid_initial.template.nodes[0].base_transform.scale = [f32::MAX, 1.0, 1.0];
        let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
            &invalid_initial.template,
            &invalid_initial.content,
            &invalid_initial.frame,
        )
        .unwrap();
        assert_eq!(
            compile_cpu_parts(
                crate::presentation::companion_scene::SceneGenerationKey {
                    device: crate::presentation::companion_scene::DeviceEpoch(1),
                    layout: crate::presentation::companion_scene::LayoutGeneration(2),
                    resources: crate::presentation::companion_scene::ResourceGeneration(3),
                },
                crate::presentation::companion_scene::AppliedRevisions::new(4, 5),
                &invalid_initial.template,
                &invalid_initial.content,
                &invalid_initial.frame,
                accepted,
            ),
            Err(CompileError::InvalidTransform)
        );
    }

    #[test]
    fn cpu_scene_lifetime_300_frames_matches_fresh_production_projection() {
        use std::sync::Arc;

        let fixture = lifetime_watch_fixture();
        let initial_snapshot = Arc::new(project_lifetime_snapshot(&fixture, 0));
        let generation_key = crate::presentation::companion_scene::SceneGenerationKey {
            device: crate::presentation::companion_scene::DeviceEpoch(7),
            layout: crate::presentation::companion_scene::LayoutGeneration(8),
            resources: crate::presentation::companion_scene::ResourceGeneration(9),
        };
        let initial_revisions = crate::presentation::companion_scene::AppliedRevisions::new(10, 20);
        let mut neutral =
            crate::presentation::companion_scene::scene::build_scene_generation_owned(
                Arc::clone(&initial_snapshot),
                generation_key,
                initial_revisions,
            )
            .unwrap();
        let mut candidate = compile_cpu_generation(&neutral).unwrap();
        let storage = candidate_storage_identity(&candidate);
        let static_checksum = candidate.static_checksum;
        let topology = candidate.topology.clone();
        let vertices = candidate.vertices.clone();
        let indices = candidate.indices.clone();
        let nodes = candidate.nodes.clone();
        let materials = candidate.materials.clone();
        let resources = candidate.resources.clone();
        let primitives = candidate.primitives.clone();
        let attachments = candidate.attachments.clone();
        let phases = candidate.phases.clone();
        let mut saw_nodes = false;
        let mut saw_globals = false;
        let mut saw_prop_content = false;
        let mut saw_prop_frame = false;
        let mut saw_tank_content = false;
        let mut saw_tank_frame = false;
        let mut saw_room_content = false;
        let mut saw_room_frame = false;
        let mut saw_prop_paint = false;
        let mut saw_content_globals = false;
        let mut saw_frame_analytics = false;

        for frame_index in 1..=300 {
            let target = Arc::new(project_lifetime_snapshot(&fixture, frame_index));
            let changes = crate::presentation::companion_scene::runtime::classify_snapshot_changes(
                neutral.source_snapshot(),
                &target,
            );
            assert!(!changes.requires_generation(), "frame {frame_index}");
            let from = neutral.source_revisions();
            let to = crate::presentation::companion_scene::AppliedRevisions::new(
                from.semantic.0
                    + u64::from(
                        changes.semantic()
                            != crate::presentation::companion_scene::runtime::SemanticChangeMask::NONE,
                    ),
                from.frame.0
                    + u64::from(
                        changes.frame()
                            != crate::presentation::companion_scene::runtime::FrameChangeMask::NONE,
                    ),
            );
            let projected = neutral
                .project_snapshot_changes(&target, changes, from, to)
                .unwrap();
            let prop_content_changed = !projected.content.prop_slots.is_empty();
            let prop_paint_changed = !projected.content.prop_paint_slots.is_empty();
            let prepared = candidate
                .prepare_deltas_with_private(
                    &projected.content,
                    &projected.frame,
                    PrivateSpatialFrame::from_snapshot(&target),
                )
                .unwrap();
            let dirty = candidate.commit_prepared(prepared).dirty;
            neutral
                .apply_compatible_snapshot(Arc::clone(&target), changes, from, to)
                .unwrap();
            let fresh_generation =
                crate::presentation::companion_scene::scene::build_scene_generation_owned(
                    Arc::clone(&target),
                    generation_key,
                    to,
                )
                .unwrap();
            let fresh = compile_cpu_generation(&fresh_generation).unwrap();

            assert_eq!(candidate.source_revisions, to);
            assert_eq!(neutral.source_revisions(), to);
            assert_eq!(candidate.logical_content, *neutral.content());
            assert_eq!(candidate.accepted.frame().frame(), neutral.frame());
            assert_eq!(candidate.content, fresh.content);
            assert_eq!(candidate.frame, fresh.frame);
            assert_eq!(candidate.logical_content, fresh.logical_content);
            assert_eq!(candidate.accepted, fresh.accepted);
            assert_eq!(candidate.static_checksum, static_checksum);
            assert_eq!(candidate.static_checksum, fresh.static_checksum);
            assert_eq!(candidate.topology, topology);
            assert_eq!(candidate.vertices, vertices);
            assert_eq!(candidate.indices, indices);
            assert_eq!(candidate.nodes, nodes);
            assert_eq!(candidate.materials, materials);
            assert_eq!(candidate.resources, resources);
            assert_eq!(candidate.primitives, primitives);
            assert_eq!(candidate.attachments, attachments);
            assert_eq!(candidate.phases, phases);
            assert_candidate_storage_identity(storage, &candidate);

            if changes.frame().contains(
                crate::presentation::companion_scene::runtime::FrameChangeMask::PET_TRANSFORM,
            ) {
                assert!(!dirty.nodes.as_slice().is_empty());
                saw_nodes = true;
            }
            if changes
                .frame()
                .contains(crate::presentation::companion_scene::runtime::FrameChangeMask::GAUGES)
                || changes
                    .frame()
                    .contains(crate::presentation::companion_scene::runtime::FrameChangeMask::DIM)
            {
                assert!(!dirty.frame_globals.as_slice().is_empty());
                saw_globals = true;
            }
            if changes.semantic().contains(
                crate::presentation::companion_scene::runtime::SemanticChangeMask::MOOD_WEATHER,
            ) {
                assert!(!dirty.content_globals.as_slice().is_empty());
                saw_content_globals = true;
            }
            if prop_content_changed && !dirty.prop_glyphs.as_slice().is_empty() {
                saw_prop_content = true;
            }
            if prop_paint_changed && !dirty.prop_glyphs.as_slice().is_empty() {
                saw_prop_paint = true;
            }
            if changes
                .semantic()
                .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::TANK)
                && !dirty.tank_glyphs.as_slice().is_empty()
            {
                saw_tank_content = true;
            }
            if changes.frame().contains(
                crate::presentation::companion_scene::runtime::FrameChangeMask::PROP_TRANSFORMS,
            ) {
                assert!(!dirty.props.as_slice().is_empty());
                saw_prop_frame = true;
            }
            if changes.frame().contains(
                crate::presentation::companion_scene::runtime::FrameChangeMask::TANK_INSTANCES,
            ) {
                assert!(!dirty.tank_cells.as_slice().is_empty());
                saw_tank_frame = true;
            }
            if changes.semantic().contains(
                crate::presentation::companion_scene::runtime::SemanticChangeMask::ROOM_GLYPHS,
            ) {
                assert!(!dirty.room_content.as_slice().is_empty());
                saw_room_content = true;
            }
            if changes.frame().contains(
                crate::presentation::companion_scene::runtime::FrameChangeMask::ROOM_GLYPHS,
            ) {
                assert!(!dirty.room_frame.as_slice().is_empty());
                saw_room_frame = true;
            }
            if !dirty.frame_analytics.as_slice().is_empty() {
                saw_frame_analytics = true;
            }
        }
        assert!(saw_nodes);
        assert!(saw_globals);
        assert!(saw_prop_content);
        assert!(saw_prop_frame);
        assert!(saw_tank_content);
        assert!(saw_tank_frame);
        assert!(saw_room_content);
        assert!(saw_room_frame);
        assert!(saw_prop_paint);
        assert!(saw_content_globals);
        assert!(saw_frame_analytics);
    }

    #[test]
    fn same_generation_camera_depth_delta_matches_fresh_compile_and_preserves_host_globals() {
        let fixture = SceneFixture::valid();
        let mut candidate = compile_fixture(&fixture);
        let before = candidate.frame.globals.as_slice()[0];
        let camera = crate::presentation::companion_scene::scene::OrthographicCamera::new(
            360.0, 360.0, -3.0, 3.0,
        )
        .unwrap();
        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
        );
        frame.camera = Some(camera);
        let prepared = candidate.prepare_deltas(&content, &frame).unwrap();
        assert!(prepared.blended_depth_dirty());
        let dirty = candidate.commit_prepared(prepared).dirty;
        assert_eq!(
            dirty.frame_globals.as_slice(),
            &[ByteSpan::slots::<FrameGlobalsGpuValue>(0, 1)]
        );

        let mut updated = fixture.clone();
        updated.frame.camera = camera;
        let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
            &updated.template,
            &updated.content,
            &updated.frame,
        )
        .unwrap();
        let fresh = compile_cpu_parts(
            candidate.generation_key,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
            &updated.template,
            &updated.content,
            &updated.frame,
            accepted,
        )
        .unwrap();
        assert_eq!(candidate.frame, fresh.frame);
        assert_eq!(candidate.accepted, fresh.accepted);
        let after = candidate.frame.globals.as_slice()[0];
        assert_eq!(
            after.projection,
            camera.projection_matrix().unwrap().columns
        );
        assert_eq!(after.viewport_points, before.viewport_points);
        assert_eq!(after.view, before.view);
        assert_eq!(after.viewport_pixels, before.viewport_pixels);
        assert_eq!(after.aperture, before.aperture);
        assert_eq!(after.activity_opacity, before.activity_opacity);
        assert_eq!(after.reduce_motion, before.reduce_motion);
    }

    #[test]
    fn prepared_blend_order_ignores_non_depth_motion_and_detects_blended_node_crossing() {
        let mut fixture = SceneFixture::valid();
        fixture.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        fixture.template.primitives[0].depth = DepthBehavior::WorldReadOnly;
        let prepare_motion = |translation: [f32; 3]| {
            let candidate = compile_static_fixture(&fixture);
            let blended_node = candidate.blended_draw_templates.as_slice()[0].node_index;
            let node_id = fixture.template.nodes[usize::from(blended_node)].id;
            let mut state = *candidate
                .accepted
                .frame()
                .frame()
                .nodes
                .iter()
                .find(|state| state.node == node_id)
                .expect("blended template node has frame state");
            state.local_transform.translation = translation;
            let (content, mut frame) = paired_deltas(
                &candidate,
                crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
            );
            frame.nodes.push(state);
            candidate.prepare_deltas(&content, &frame).unwrap()
        };

        assert!(!prepare_motion([8.0, 0.0, 0.0]).blended_depth_dirty());
        assert!(prepare_motion([0.0, 0.0, -0.75]).blended_depth_dirty());
    }

    #[test]
    fn compiler_rejects_static_and_blended_capacity_overflow() {
        let mut static_overflow = SceneFixture::valid();
        static_overflow.template.primitives =
            vec![static_overflow.template.primitives[0].clone(); MAX_STATIC_PRIMITIVES + 1];
        assert_eq!(
            compile_cpu_parts(
                crate::presentation::companion_scene::SceneGenerationKey {
                    device: crate::presentation::companion_scene::DeviceEpoch(1),
                    layout: crate::presentation::companion_scene::LayoutGeneration(2),
                    resources: crate::presentation::companion_scene::ResourceGeneration(3),
                },
                crate::presentation::companion_scene::AppliedRevisions::new(4, 5),
                &static_overflow.template,
                &static_overflow.content,
                &static_overflow.frame,
                crate::presentation::companion_scene::validate::validate_full_generation(
                    &SceneFixture::valid().template,
                    &SceneFixture::valid().content,
                    &SceneFixture::valid().frame,
                )
                .unwrap(),
            ),
            Err(CompileError::CapacityExceeded)
        );

        let mut blended_overflow = SceneFixture::valid();
        let mut blended = blended_overflow.template.primitives[0].clone();
        blended.blend = WorldBlend::PremultipliedAlpha;
        blended.depth = DepthBehavior::WorldReadOnly;
        blended_overflow.template.primitives = vec![blended; MAX_BLENDED_DRAWS + 1];
        assert_eq!(
            compile_cpu_parts(
                crate::presentation::companion_scene::SceneGenerationKey {
                    device: crate::presentation::companion_scene::DeviceEpoch(1),
                    layout: crate::presentation::companion_scene::LayoutGeneration(2),
                    resources: crate::presentation::companion_scene::ResourceGeneration(3),
                },
                crate::presentation::companion_scene::AppliedRevisions::new(4, 5),
                &blended_overflow.template,
                &blended_overflow.content,
                &blended_overflow.frame,
                crate::presentation::companion_scene::validate::validate_full_generation(
                    &SceneFixture::valid().template,
                    &SceneFixture::valid().content,
                    &SceneFixture::valid().frame,
                )
                .unwrap(),
            ),
            Err(CompileError::CapacityExceeded)
        );
    }
}
