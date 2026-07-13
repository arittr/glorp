//! Pure-CPU compilation for fixed retained companion scene generations.

use bytemuck::{Pod, Zeroable};

use super::buffers::{ByteSpan, DirtySpanSet, FixedPodMirror, MirrorError};
use crate::presentation::companion_scene::scene::{
    is_world_blended, AmbientContentKind, AttachmentInstanceBinding, AttachmentMode, Bounds3,
    ContentDelta, DepthBehavior, FrameDelta, InstanceGroupBinding, InstanceLayer, MaterialKind,
    MoodContentKind, NodeId, PetPaletteRole, PrimitiveKind, PrimitiveSpace, ResourceKind,
    SceneContent, SceneFrame, SceneGenerationData, SceneTemplate, WeatherContentKind, WorldBlend,
    MAX_AMBIENT_INSTANCES, MAX_ATTACHMENTS, MAX_BLENDED_DRAWS, MAX_HUD_GLYPH_SLOTS, MAX_LIGHTS,
    MAX_PET_ART_SLOTS, MAX_PROP_GLYPHS_PER_SLOT, MAX_ROUND_TANK_INHABITANTS, MAX_SCENE_NODES,
    MAX_STATIC_PRIMITIVES, MAX_TANK_GLYPHS_PER_SLOT, MAX_VISIBLE_PROPS,
};

const NONE_U32: u32 = u32::MAX;
const PROP_GLYPH_CAPACITY: usize = MAX_VISIBLE_PROPS * MAX_PROP_GLYPHS_PER_SLOT;
const TANK_GLYPH_CAPACITY: usize = MAX_ROUND_TANK_INHABITANTS * MAX_TANK_GLYPHS_PER_SLOT;

/// Compiler-owned record sizes and fixed capacities consumed by GPU packing.
/// The renderer derives packed offsets from this shape instead of duplicating
/// the CPU mirror ABI as independent literals.
pub(super) struct CpuMirrorShape;

impl CpuMirrorShape {
    pub(super) const NODE_RECORD_BYTES: usize = std::mem::size_of::<NodeGpuValue>();
    pub(super) const NODE_COUNT: usize = MAX_SCENE_NODES;
    pub(super) const CONTENT_RECORD_BYTES: [usize; 6] = [
        std::mem::size_of::<ContentGlobalsGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
        std::mem::size_of::<ContentGpuValue>(),
    ];
    pub(super) const CONTENT_COUNTS: [usize; 6] = [
        1,
        MAX_PET_ART_SLOTS,
        PROP_GLYPH_CAPACITY,
        TANK_GLYPH_CAPACITY,
        MAX_AMBIENT_INSTANCES,
        MAX_HUD_GLYPH_SLOTS,
    ];
    pub(super) const FRAME_RECORD_BYTES: [usize; 6] = [
        std::mem::size_of::<FrameGlobalsGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
        std::mem::size_of::<FrameGpuValue>(),
    ];
    pub(super) const FRAME_COUNTS: [usize; 6] = [
        1,
        MAX_VISIBLE_PROPS,
        TANK_GLYPH_CAPACITY,
        MAX_AMBIENT_INSTANCES,
        MAX_HUD_GLYPH_SLOTS,
        MAX_LIGHTS,
    ];
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
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing per-node upload ABI shared with the Task 9 shader layout.
pub(super) struct NodeGpuValue {
    world: [[f32; 4]; 4],
    opacity: f32,
    visible: u32,
    material_parameter_offset: u32,
    material_parameter_count: u32,
    depth_cue: [f32; 4],
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
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing fixed frame-slot upload ABI.
pub(super) struct FrameGpuValue {
    kind: u32,
    slot: u32,
    flags: u32,
    variant: u32,
    values: [f32; 8],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing content-global upload ABI.
struct ContentGlobalsGpuValue {
    palette_rgba: [[u32; 4]; 8],
    mood: u32,
    weather: u32,
    _padding: [u32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
/// GPU-facing frame-global upload ABI.
struct FrameGlobalsGpuValue {
    view: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    viewport_points: [f32; 2],
    viewport_pixels: [f32; 2],
    aperture: [f32; 4],
    gauges: [f32; 4],
    dim_amount: f32,
    light_count: u32,
    // Storage-buffer tail padding only; it carries no scene or host semantics.
    _padding: [u32; 2],
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
    pet: FixedPodMirror<ContentGpuValue, MAX_PET_ART_SLOTS>,
    prop_glyphs: FixedPodMirror<ContentGpuValue, PROP_GLYPH_CAPACITY>,
    tank_glyphs: FixedPodMirror<ContentGpuValue, TANK_GLYPH_CAPACITY>,
    ambient: FixedPodMirror<ContentGpuValue, MAX_AMBIENT_INSTANCES>,
    hud: FixedPodMirror<ContentGpuValue, MAX_HUD_GLYPH_SLOTS>,
}

impl ContentMirrors {
    fn zeroed() -> Self {
        Self {
            globals: FixedPodMirror::zeroed(),
            pet: FixedPodMirror::zeroed(),
            prop_glyphs: FixedPodMirror::zeroed(),
            tank_glyphs: FixedPodMirror::zeroed(),
            ambient: FixedPodMirror::zeroed(),
            hud: FixedPodMirror::zeroed(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct FrameMirrors {
    globals: FixedPodMirror<FrameGlobalsGpuValue, 1>,
    nodes: FixedPodMirror<NodeGpuValue, MAX_SCENE_NODES>,
    props: FixedPodMirror<FrameGpuValue, MAX_VISIBLE_PROPS>,
    tank_cells: FixedPodMirror<FrameGpuValue, TANK_GLYPH_CAPACITY>,
    ambient: FixedPodMirror<FrameGpuValue, MAX_AMBIENT_INSTANCES>,
    hud: FixedPodMirror<FrameGpuValue, MAX_HUD_GLYPH_SLOTS>,
    lights: FixedPodMirror<FrameGpuValue, MAX_LIGHTS>,
}

impl FrameMirrors {
    fn zeroed() -> Self {
        Self {
            globals: FixedPodMirror::zeroed(),
            nodes: FixedPodMirror::zeroed(),
            props: FixedPodMirror::zeroed(),
            tank_cells: FixedPodMirror::zeroed(),
            ambient: FixedPodMirror::zeroed(),
            hud: FixedPodMirror::zeroed(),
            lights: FixedPodMirror::zeroed(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PhaseLists {
    opaque_cutout: Vec<u32>,
    world_blended_unsorted: Vec<u32>,
    chrome_authored: Vec<u32>,
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
    pub(super) pet: DirtySpanSet,
    pub(super) prop_glyphs: DirtySpanSet,
    pub(super) tank_glyphs: DirtySpanSet,
    pub(super) content_ambient: DirtySpanSet,
    pub(super) content_hud: DirtySpanSet,
    pub(super) frame_globals: DirtySpanSet,
    pub(super) nodes: DirtySpanSet,
    pub(super) props: DirtySpanSet,
    pub(super) tank_cells: DirtySpanSet,
    pub(super) frame_ambient: DirtySpanSet,
    pub(super) frame_hud: DirtySpanSet,
    pub(super) lights: DirtySpanSet,
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
            pet: DirtySpanSet::default(),
            prop_glyphs: DirtySpanSet::default(),
            tank_glyphs: DirtySpanSet::default(),
            content_ambient: DirtySpanSet::default(),
            content_hud: DirtySpanSet::default(),
            frame_globals: DirtySpanSet::default(),
            nodes: DirtySpanSet::default(),
            props: DirtySpanSet::default(),
            tank_cells: DirtySpanSet::default(),
            frame_ambient: DirtySpanSet::default(),
            frame_hud: DirtySpanSet::default(),
            lights: DirtySpanSet::default(),
        }
    }
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

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CpuSceneCandidate {
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
    pub(super) content: ContentMirrors,
    pub(super) frame: FrameMirrors,
    logical_content: SceneContent,
    accepted: crate::presentation::companion_scene::validate::AcceptedSceneState,
    topology: FixedNodeTopology,
    #[cfg(test)]
    last_node_resolves: usize,
}

struct PreparedMirrorDelta {
    dirty: SceneDirtySpans,
    content_globals: Option<ContentGlobalsGpuValue>,
    pet: [Option<ContentGpuValue>; MAX_PET_ART_SLOTS],
    prop_glyphs: [Option<ContentGpuValue>; PROP_GLYPH_CAPACITY],
    tank_glyphs: [Option<ContentGpuValue>; TANK_GLYPH_CAPACITY],
    content_ambient: [Option<ContentGpuValue>; MAX_AMBIENT_INSTANCES],
    content_hud: [Option<ContentGpuValue>; MAX_HUD_GLYPH_SLOTS],
    frame_globals: Option<FrameGlobalsGpuValue>,
    nodes: [Option<NodeGpuValue>; MAX_SCENE_NODES],
    props: [Option<FrameGpuValue>; MAX_VISIBLE_PROPS],
    tank_cells: [Option<FrameGpuValue>; TANK_GLYPH_CAPACITY],
    frame_ambient: [Option<FrameGpuValue>; MAX_AMBIENT_INSTANCES],
    frame_hud: [Option<FrameGpuValue>; MAX_HUD_GLYPH_SLOTS],
    lights: [Option<FrameGpuValue>; MAX_LIGHTS],
    #[cfg(test)]
    node_resolves: usize,
}

impl PreparedMirrorDelta {
    fn empty(
        from: crate::presentation::companion_scene::AppliedRevisions,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> Self {
        Self {
            dirty: SceneDirtySpans::empty(from, to),
            content_globals: None,
            pet: [None; MAX_PET_ART_SLOTS],
            prop_glyphs: [None; PROP_GLYPH_CAPACITY],
            tank_glyphs: [None; TANK_GLYPH_CAPACITY],
            content_ambient: [None; MAX_AMBIENT_INSTANCES],
            content_hud: [None; MAX_HUD_GLYPH_SLOTS],
            frame_globals: None,
            nodes: [None; MAX_SCENE_NODES],
            props: [None; MAX_VISIBLE_PROPS],
            tank_cells: [None; TANK_GLYPH_CAPACITY],
            frame_ambient: [None; MAX_AMBIENT_INSTANCES],
            frame_hud: [None; MAX_HUD_GLYPH_SLOTS],
            lights: [None; MAX_LIGHTS],
            #[cfg(test)]
            node_resolves: 0,
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
    #[allow(dead_code)] // Task 9 routes reconciler deltas through this paired transaction.
    pub(super) fn apply_deltas(
        &mut self,
        content_delta: &ContentDelta,
        frame_delta: &FrameDelta,
    ) -> Result<SceneDirtySpans, MirrorDeltaError> {
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
        let prepared = self.prepare_mirror_delta(content_delta, frame_delta)?;

        // This is the final fallible operation. The neutral validator performs
        // every check before mutating AcceptedSceneState, so an error here still
        // leaves the complete candidate byte-for-byte unchanged.
        self.accepted
            .apply_frame_delta(frame_delta)
            .map_err(MirrorDeltaError::Validation)?;

        apply_content_delta_in_place(&mut self.logical_content, content_delta);
        let dirty = prepared.dirty;
        #[cfg(test)]
        let node_resolves = prepared.node_resolves;
        self.commit_prepared(prepared);
        #[cfg(test)]
        {
            self.last_node_resolves = node_resolves;
        }
        self.source_revisions = content_delta.to;
        Ok(dirty)
    }
}

fn content_delta_has_values(delta: &ContentDelta) -> bool {
    delta.palette.is_some()
        || delta.mood.is_some()
        || delta.weather.is_some()
        || !delta.pet_art_slots.is_empty()
        || !delta.prop_slots.is_empty()
        || !delta.tank_slots.is_empty()
        || !delta.ambient_slots.is_empty()
        || !delta.hud_slots.is_empty()
}

fn frame_delta_has_values(delta: &FrameDelta) -> bool {
    delta.camera.is_some()
        || !delta.nodes.is_empty()
        || !delta.prop_slots.is_empty()
        || !delta.tank_slots.is_empty()
        || !delta.ambient_slots.is_empty()
        || !delta.hud_slots.is_empty()
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
                &self.content.pet,
                &mut prepared.pet,
                &mut prepared.dirty.pet,
                usize::from(changed.slot),
                pack_pet_content(*changed),
            )?;
        }
        for changed in &content_delta.prop_slots {
            let slot = usize::from(changed.slot);
            if slot >= MAX_VISIBLE_PROPS {
                return Err(MirrorDeltaError::Mirror(MirrorError::CapacityExceeded));
            }
            for subslot in 0..MAX_PROP_GLYPHS_PER_SLOT {
                let flat = slot * MAX_PROP_GLYPHS_PER_SLOT + subslot;
                record_update(
                    &self.content.prop_glyphs,
                    &mut prepared.prop_glyphs,
                    &mut prepared.dirty.prop_glyphs,
                    flat,
                    pack_prop_content(*changed, subslot),
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
            record_update(
                &self.content.ambient,
                &mut prepared.content_ambient,
                &mut prepared.dirty.content_ambient,
                usize::from(changed.slot),
                pack_ambient_content(*changed),
            )?;
        }
        for changed in &content_delta.hud_slots {
            record_update(
                &self.content.hud,
                &mut prepared.content_hud,
                &mut prepared.dirty.content_hud,
                usize::from(changed.slot),
                pack_hud_content(*changed),
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
        for changed in &frame_delta.hud_slots {
            record_update(
                &self.frame.hud,
                &mut prepared.frame_hud,
                &mut prepared.dirty.frame_hud,
                usize::from(changed.slot),
                pack_hud_frame(*changed),
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

    fn commit_prepared(&mut self, prepared: PreparedMirrorDelta) {
        if let Some(value) = prepared.content_globals {
            self.content.globals.set_fixed(0, value);
        }
        commit_updates(&mut self.content.pet, prepared.pet);
        commit_updates(&mut self.content.prop_glyphs, prepared.prop_glyphs);
        commit_updates(&mut self.content.tank_glyphs, prepared.tank_glyphs);
        commit_updates(&mut self.content.ambient, prepared.content_ambient);
        commit_updates(&mut self.content.hud, prepared.content_hud);
        if let Some(value) = prepared.frame_globals {
            self.frame.globals.set_fixed(0, value);
        }
        commit_updates(&mut self.frame.nodes, prepared.nodes);
        commit_updates(&mut self.frame.props, prepared.props);
        commit_updates(&mut self.frame.tank_cells, prepared.tank_cells);
        commit_updates(&mut self.frame.ambient, prepared.frame_ambient);
        commit_updates(&mut self.frame.hud, prepared.frame_hud);
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
) -> ContentGpuValue {
    ContentGpuValue {
        kind: 1,
        glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
        slot: u32::from(value.slot),
        subslot: 0,
        signed_data: [0; 2],
        flags: palette_role_tag(value.palette_role),
        variant: 0,
    }
}

fn pack_prop_content(
    outer: crate::presentation::companion_scene::scene::PropContentSlot,
    subslot: usize,
) -> ContentGpuValue {
    let (glyph_scalar, local_cell, flags, variant) = match outer.content {
        Some(value) => {
            let glyph = value.glyphs[subslot];
            (
                option_glyph(glyph.glyph.map(|glyph| glyph.as_char())),
                glyph.local_cell.map(i32::from),
                option_bool(value.twinkle_active)
                    | (option_bool(value.lid_open) << 2)
                    | (option_bool(value.bloom_active) << 4),
                value.sprite_phase.map_or(NONE_U32, u32::from),
            )
        }
        None => (NONE_U32, [0; 2], 0, NONE_U32),
    };
    ContentGpuValue {
        kind: 2,
        glyph_scalar,
        slot: u32::from(outer.slot),
        subslot: u32::try_from(subslot).expect("fixed prop subslot fits u32"),
        signed_data: local_cell,
        flags,
        variant,
    }
}

fn pack_tank_content(
    outer: crate::presentation::companion_scene::scene::TankContentSlot,
    subslot: usize,
) -> ContentGpuValue {
    let (glyph_scalar, flags, variant) = match outer.content {
        Some(value) => (
            option_glyph(value.glyphs[subslot].map(|glyph| glyph.as_char())),
            value.morph.map_or(NONE_U32, u32::from),
            u32::from(value.sprite_variant),
        ),
        None => (NONE_U32, NONE_U32, NONE_U32),
    };
    ContentGpuValue {
        kind: 3,
        glyph_scalar,
        slot: u32::from(outer.slot),
        subslot: u32::try_from(subslot).expect("fixed tank subslot fits u32"),
        signed_data: [0; 2],
        flags,
        variant,
    }
}

fn pack_ambient_content(
    value: crate::presentation::companion_scene::scene::AmbientContentSlot,
) -> ContentGpuValue {
    ContentGpuValue {
        kind: 4,
        glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
        slot: u32::from(value.slot),
        subslot: 0,
        signed_data: [0; 2],
        flags: 0,
        variant: value.kind.map_or(NONE_U32, ambient_kind_tag),
    }
}

fn pack_hud_content(
    value: crate::presentation::companion_scene::scene::HudContentSlot,
) -> ContentGpuValue {
    ContentGpuValue {
        kind: 5,
        glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
        slot: u32::from(value.slot),
        subslot: 0,
        signed_data: [0; 2],
        flags: 0,
        variant: 0,
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
            0.0,
            0.0,
            0.0,
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
        ],
    }
}

fn pack_hud_frame(
    value: crate::presentation::companion_scene::scene::HudFrameSlot,
) -> FrameGpuValue {
    FrameGpuValue {
        kind: 4,
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
            ],
        },
        None => FrameGpuValue {
            kind: 5,
            slot: u32::try_from(slot).expect("fixed light slot fits u32"),
            flags: 0,
            variant: NONE_U32,
            values: [0.0; 8],
        },
    }
}

fn apply_content_delta_in_place(content: &mut SceneContent, delta: &ContentDelta) {
    if let Some(palette) = delta.palette {
        content.palette = palette;
    }
    if let Some(mood) = delta.mood {
        content.mood = mood;
    }
    if let Some(weather) = delta.weather {
        content.weather = weather;
    }
    for changed in &delta.pet_art_slots {
        content.pet_art_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.prop_slots {
        content.prop_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.tank_slots {
        content.tank_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.ambient_slots {
        content.ambient_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.hud_slots {
        content.hud_slots[usize::from(changed.slot)] = *changed;
    }
}

#[allow(dead_code)] // Called when retained-scene materialization lands in the next checkpoint.
pub(super) fn compile_cpu_generation(
    generation: &SceneGenerationData,
) -> Result<CpuSceneCandidate, CompileError> {
    compile_cpu_parts(
        generation.generation_key(),
        generation.source_revisions(),
        generation.template(),
        generation.content(),
        generation.frame(),
        generation.accepted_state().clone(),
    )
}

fn compile_cpu_parts(
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    template: &SceneTemplate,
    content: &SceneContent,
    frame: &SceneFrame,
    accepted: crate::presentation::companion_scene::validate::AcceptedSceneState,
) -> Result<CpuSceneCandidate, CompileError> {
    let logical_content = content.clone();
    if template.nodes.len() > MAX_SCENE_NODES
        || template.primitives.len() > MAX_STATIC_PRIMITIVES
        || content.pet_art_slots.len() != MAX_PET_ART_SLOTS
        || content.prop_slots.len() != MAX_VISIBLE_PROPS
        || content.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || content.ambient_slots.len() != MAX_AMBIENT_INSTANCES
        || content.hud_slots.len() != MAX_HUD_GLYPH_SLOTS
        || frame.nodes.len() > MAX_SCENE_NODES
        || frame.prop_slots.len() != MAX_VISIBLE_PROPS
        || frame.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || frame.ambient_slots.len() != MAX_AMBIENT_INSTANCES
        || frame.hud_slots.len() != MAX_HUD_GLYPH_SLOTS
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
        let (instance_group, instance_slot) = instance_group_tags(primitive.instance_group);
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

    let attachments = compile_attachment_descriptors(template, &index)?;

    let content = compile_content_mirrors(content)?;
    let frame = compile_frame_mirrors(template, frame, &index)?;
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
        content,
        frame,
        logical_content,
        accepted,
        topology,
        #[cfg(test)]
        last_node_resolves: 0,
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
                                    primitive.instance_group
                                        == Some(InstanceGroupBinding::PropGlyphs(slot))
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

fn compile_content_mirrors(content: &SceneContent) -> Result<ContentMirrors, CompileError> {
    let mut result = ContentMirrors::zeroed();
    set_all(
        &mut result.globals,
        [ContentGlobalsGpuValue {
            palette_rgba: content
                .palette
                .map(|rgb| [u32::from(rgb[0]), u32::from(rgb[1]), u32::from(rgb[2]), 255]),
            mood: mood_tag(content.mood),
            weather: weather_tag(content.weather),
            _padding: [0; 2],
        }],
    );
    set_all(
        &mut result.pet,
        std::array::from_fn(|slot| {
            let value = content.pet_art_slots[slot];
            ContentGpuValue {
                kind: 1,
                glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
                slot: u32::from(value.slot),
                subslot: 0,
                signed_data: [0; 2],
                flags: palette_role_tag(value.palette_role),
                variant: 0,
            }
        }),
    );
    set_all(
        &mut result.prop_glyphs,
        std::array::from_fn(|flat| {
            let slot = flat / MAX_PROP_GLYPHS_PER_SLOT;
            let subslot = flat % MAX_PROP_GLYPHS_PER_SLOT;
            let outer = content.prop_slots[slot];
            let (glyph_scalar, local_cell, flags, variant) = match outer.content {
                Some(value) => {
                    let glyph = value.glyphs[subslot];
                    (
                        option_glyph(glyph.glyph.map(|glyph| glyph.as_char())),
                        glyph.local_cell.map(i32::from),
                        option_bool(value.twinkle_active)
                            | (option_bool(value.lid_open) << 2)
                            | (option_bool(value.bloom_active) << 4),
                        value.sprite_phase.map_or(NONE_U32, u32::from),
                    )
                }
                None => (NONE_U32, [0; 2], 0, NONE_U32),
            };
            ContentGpuValue {
                kind: 2,
                glyph_scalar,
                slot: u32::from(outer.slot),
                subslot: u32::try_from(subslot).expect("fixed prop subslot fits u32"),
                signed_data: local_cell,
                flags,
                variant,
            }
        }),
    );
    set_all(
        &mut result.tank_glyphs,
        std::array::from_fn(|flat| {
            let slot = flat / MAX_TANK_GLYPHS_PER_SLOT;
            let subslot = flat % MAX_TANK_GLYPHS_PER_SLOT;
            let outer = content.tank_slots[slot];
            let (glyph_scalar, flags, variant) = match outer.content {
                Some(value) => (
                    option_glyph(value.glyphs[subslot].map(|glyph| glyph.as_char())),
                    value.morph.map_or(NONE_U32, u32::from),
                    u32::from(value.sprite_variant),
                ),
                None => (NONE_U32, NONE_U32, NONE_U32),
            };
            ContentGpuValue {
                kind: 3,
                glyph_scalar,
                slot: u32::from(outer.slot),
                subslot: u32::try_from(subslot).expect("fixed tank subslot fits u32"),
                signed_data: [0; 2],
                flags,
                variant,
            }
        }),
    );
    set_all(
        &mut result.ambient,
        std::array::from_fn(|slot| {
            let value = content.ambient_slots[slot];
            ContentGpuValue {
                kind: 4,
                glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
                slot: u32::from(value.slot),
                subslot: 0,
                signed_data: [0; 2],
                flags: 0,
                variant: value.kind.map_or(NONE_U32, ambient_kind_tag),
            }
        }),
    );
    set_all(
        &mut result.hud,
        std::array::from_fn(|slot| {
            let value = content.hud_slots[slot];
            ContentGpuValue {
                kind: 5,
                glyph_scalar: option_glyph(value.glyph.map(|glyph| glyph.as_char())),
                slot: u32::from(value.slot),
                subslot: 0,
                signed_data: [0; 2],
                flags: 0,
                variant: 0,
            }
        }),
    );
    Ok(result)
}

fn compile_frame_mirrors(
    template: &SceneTemplate,
    frame: &SceneFrame,
    index: &DenseSceneIndex,
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
            _padding: [0; 2],
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
        &mut result.props,
        std::array::from_fn(|slot| {
            let value = frame.prop_slots[slot];
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
                    0.0,
                    0.0,
                    0.0,
                ],
            }
        }),
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
                ],
            }
        }),
    );
    set_all(
        &mut result.hud,
        std::array::from_fn(|slot| {
            let value = frame.hud_slots[slot];
            FrameGpuValue {
                kind: 4,
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
                ],
            }
        }),
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
                ],
            },
            None => FrameGpuValue {
                kind: 5,
                slot: u32::try_from(slot).expect("fixed light slot fits u32"),
                flags: 0,
                variant: NONE_U32,
                values: [0.0; 8],
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

fn instance_group_tags(value: Option<InstanceGroupBinding>) -> (u32, u32) {
    match value {
        None => (0, NONE_U32),
        Some(InstanceGroupBinding::PetBody) => (1, 0),
        Some(InstanceGroupBinding::PetParticles) => (2, 0),
        Some(InstanceGroupBinding::PropGlyphs(slot)) => (3, u32::from(slot)),
        Some(InstanceGroupBinding::TankCells { slot, layer }) => {
            (4 + instance_layer_tag(layer), u32::from(slot))
        }
        Some(InstanceGroupBinding::Ambient) => (7, 0),
        Some(InstanceGroupBinding::Hud) => (8, 0),
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
        AmbientContentKind::ActivityPulse => 2,
        AmbientContentKind::PetParticle => 3,
        AmbientContentKind::Weather => 4,
        AmbientContentKind::MoodAura => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::companion_scene::scene::{AuthoredGlyph, MaterialKind, SceneFixture};

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
            &dirty.pet,
            &dirty.prop_glyphs,
            &dirty.tank_glyphs,
            &dirty.content_ambient,
            &dirty.content_hud,
            &dirty.frame_globals,
            &dirty.nodes,
            &dirty.props,
            &dirty.tank_cells,
            &dirty.frame_ambient,
            &dirty.frame_hud,
            &dirty.lights,
        ]
        .into_iter()
        .all(|spans| spans.as_slice().is_empty())
    }

    #[derive(Clone, Copy)]
    struct CandidateStorageIdentity {
        static_vecs: [(usize, usize); 19],
        fixed_mirrors: [(usize, usize); 13],
        logical_vecs: [(usize, usize); 11],
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
            ],
            fixed_mirrors: [
                (
                    candidate.content.globals.as_slice().as_ptr() as usize,
                    candidate.content.globals.capacity(),
                ),
                (
                    candidate.content.pet.as_slice().as_ptr() as usize,
                    candidate.content.pet.capacity(),
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
                    candidate.content.hud.as_slice().as_ptr() as usize,
                    candidate.content.hud.capacity(),
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
                    candidate.frame.hud.as_slice().as_ptr() as usize,
                    candidate.frame.hud.capacity(),
                ),
                (
                    candidate.frame.lights.as_slice().as_ptr() as usize,
                    candidate.frame.lights.capacity(),
                ),
            ],
            logical_vecs: [
                vec_identity(&candidate.logical_content.pet_art_slots),
                vec_identity(&candidate.logical_content.prop_slots),
                vec_identity(&candidate.logical_content.tank_slots),
                vec_identity(&candidate.logical_content.ambient_slots),
                vec_identity(&candidate.logical_content.hud_slots),
                vec_identity(&accepted_frame.nodes),
                vec_identity(&accepted_frame.prop_slots),
                vec_identity(&accepted_frame.tank_slots),
                vec_identity(&accepted_frame.ambient_slots),
                vec_identity(&accepted_frame.hud_slots),
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

    fn lifetime_watch_fixture() -> crate::tui::view_model::WatchViewModel {
        let day = time::macros::date!(2026 - 07 - 11);
        let mut vm =
            crate::tui::view_model::WatchViewModel::fixture_with_tank_inhabitants_for_age(120, day);
        vm.habitat.earned_props =
            crate::tui::view_model::WatchViewModel::fixture_with_habitat_props()
                .habitat
                .earned_props;
        let rendered = crate::pet::render::render_pet(
            &crate::pet::generation::generate_pet("retained-cpu-lifetime")
                .with_species(crate::pet::generation::Species::Fuzz),
            crate::game::evolution::Stage::S3,
            crate::game::metabolism::Mood::Content,
            crate::pet::render::AnimationFrame::default(),
        );
        vm.pet_render.generated_species = crate::pet::generation::Species::Fuzz;
        vm.pet_render.stage = crate::game::evolution::Stage::S3;
        vm.pet_render.mood = crate::game::metabolism::Mood::Content;
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    fn project_lifetime_snapshot(
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
        let wall_time = time::macros::datetime!(2026-07-11 12:00 UTC);
        let mut snapshot =
            crate::presentation::companion_scene::CompanionSceneSnapshot::project_with_input(
                &vm,
                crate::presentation::companion_scene::CompanionSceneProjectionInput::round(
                    crate::presentation::companion_scene::CompanionProjectionClock::new(
                        wall_time,
                        u64::try_from(frame_index).unwrap() * 33,
                    ),
                    crate::presentation::companion_scene::CompanionLogicalLayout::round(
                        360.0, 360.0,
                    ),
                    44,
                    18,
                    crate::round::scene::current_round_motion_clearance(18),
                ),
            )
            .unwrap();
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
        assert_eq!(std::mem::size_of::<FrameGpuValue>(), 48);
        assert_eq!(std::mem::align_of::<FrameGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(FrameGpuValue, values), 16);

        assert_eq!(std::mem::size_of::<ContentGlobalsGpuValue>(), 144);
        assert_eq!(std::mem::align_of::<ContentGlobalsGpuValue>(), 4);
        assert_eq!(std::mem::offset_of!(ContentGlobalsGpuValue, mood), 128);
        assert_eq!(std::mem::offset_of!(ContentGlobalsGpuValue, weather), 132);
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
    }

    #[test]
    fn fixed_family_capacities_are_exact() {
        let content = ContentMirrors::zeroed();
        assert_eq!(content.pet.capacity(), 130);
        assert_eq!(content.prop_glyphs.capacity(), 90);
        assert_eq!(content.tank_glyphs.capacity(), 16);
        assert_eq!(content.ambient.capacity(), 64);
        assert_eq!(content.hud.capacity(), 24);
        let frame = FrameMirrors::zeroed();
        assert_eq!(frame.nodes.capacity(), 128);
        assert_eq!(frame.props.capacity(), 10);
        assert_eq!(frame.tank_cells.capacity(), 16);
        assert_eq!(frame.ambient.capacity(), 64);
        assert_eq!(frame.hud.capacity(), 24);
        assert_eq!(frame.lights.capacity(), 2);
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
                glyphs: std::array::from_fn(|index| {
                    (index == 7).then(|| AuthoredGlyph::new('◈').unwrap())
                }),
            },
        );
        let compiled = compile_static_fixture(&fixture);
        let prop = compiled.content.prop_glyphs.as_slice()[3 * 9 + 4];
        assert_eq!(prop.glyph_scalar, u32::from('◆'));
        assert_eq!(prop.signed_data, [0, 2]);
        assert_eq!(prop.flags, 2 | (1 << 2));
        assert_eq!(prop.variant, 7);
        let tank = compiled.content.tank_glyphs.as_slice()[8 + 7];
        assert_eq!(tank.glyph_scalar, u32::from('◈'));
        assert_eq!(tank.flags, 6);
        assert_eq!(tank.variant, 9);
        assert_eq!(
            compiled.content.prop_glyphs.as_slice()[0].glyph_scalar,
            NONE_U32
        );
        assert_eq!(compiled.content.tank_glyphs.as_slice()[0].variant, NONE_U32);
    }

    #[test]
    fn initial_mirrors_preserve_globals_fixed_slots_lights_and_empty_tail() {
        let mut fixture = SceneFixture::valid();
        fixture.content.palette[2] = [11, 22, 33];
        fixture.content.mood = MoodContentKind::Happy;
        fixture.content.weather = WeatherContentKind::Mixed;
        fixture.content.pet_art_slots[0].palette_role = PetPaletteRole::Eye;
        fixture.content.ambient_slots[2].kind = Some(AmbientContentKind::Weather);
        fixture.content.ambient_slots[2].glyph = Some(AuthoredGlyph::new('☁').unwrap());
        fixture.content.hud_slots[1].glyph = Some(AuthoredGlyph::new('7').unwrap());

        fixture.frame.gauges = [0.1, 0.2, 0.3, 0.4];
        fixture.frame.dim_amount = 0.35;
        fixture.frame.prop_slots[2].visible = true;
        fixture.frame.prop_slots[2].origin_points = [10.0, 20.0];
        fixture.frame.prop_slots[2].motion_offset_points = [1.0, 2.0];
        fixture.frame.prop_slots[2].opacity = 0.75;
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
        fixture.frame.hud_slots[1].visible = true;
        fixture.frame.hud_slots[1].position_points = [90.0, 100.0];
        fixture.frame.hud_slots[1].opacity = 0.8;
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
            compiled.content.pet.as_slice()[0].glyph_scalar,
            u32::from('^')
        );
        assert_eq!(compiled.content.pet.as_slice()[0].flags, 3);
        assert_eq!(
            compiled.content.ambient.as_slice()[2].glyph_scalar,
            u32::from('☁')
        );
        assert_eq!(compiled.content.ambient.as_slice()[2].variant, 4);
        assert_eq!(
            compiled.content.hud.as_slice()[1].glyph_scalar,
            u32::from('7')
        );

        let frame_globals = compiled.frame.globals.as_slice()[0];
        assert_eq!(frame_globals.gauges, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(frame_globals.dim_amount, 0.35);
        assert_eq!(frame_globals.light_count, 1);
        assert_eq!(
            compiled.frame.props.as_slice()[2].values[..5],
            [10.0, 20.0, 1.0, 2.0, 0.75]
        );
        let tank = compiled.frame.tank_cells.as_slice()[MAX_TANK_GLYPHS_PER_SLOT + 3];
        assert_eq!(tank.slot, 1);
        assert_eq!(tank.variant, 2 | (3 << 16));
        assert_eq!(tank.values, [30.0, 40.0, 5.0, 6.0, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            compiled.frame.ambient.as_slice()[2].values[..3],
            [70.0, 80.0, 0.6]
        );
        assert_eq!(
            compiled.frame.hud.as_slice()[1].values[..3],
            [90.0, 100.0, 0.8]
        );
        assert_eq!(compiled.frame.lights.as_slice()[0].flags, 1);
        assert_eq!(compiled.frame.lights.as_slice()[1].variant, NONE_U32);
        assert_eq!(compiled.frame.nodes.as_slice()[2], NodeGpuValue::zeroed());
    }

    #[test]
    fn attachment_tables_preserve_binding_source_and_local_transform() {
        let mut fixture = SceneFixture::valid();
        fixture.template.primitives[0].instance_group = Some(InstanceGroupBinding::PropGlyphs(4));
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
        depth_cue.template.nodes[1].depth_cue =
            crate::presentation::companion_scene::scene::DepthCue {
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
        content.tank_slots.push(
            crate::presentation::companion_scene::scene::TankContentSlot {
                slot: 1,
                content: Some(
                    crate::presentation::companion_scene::scene::TankSemanticContent {
                        sprite_variant: 3,
                        morph: Some(1),
                        glyphs: [Some(AuthoredGlyph::new('◈').unwrap()); MAX_TANK_GLYPHS_PER_SLOT],
                    },
                ),
            },
        );
        content.ambient_slots.push(
            crate::presentation::companion_scene::scene::AmbientContentSlot {
                slot: 2,
                kind: Some(AmbientContentKind::Weather),
                glyph: Some(AuthoredGlyph::new('☁').unwrap()),
            },
        );
        content.hud_slots.push(
            crate::presentation::companion_scene::scene::HudContentSlot {
                slot: 1,
                glyph: Some(AuthoredGlyph::new('7').unwrap()),
            },
        );

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
        frame
            .hud_slots
            .push(crate::presentation::companion_scene::scene::HudFrameSlot {
                slot: 1,
                visible: true,
                position_points: [70.0, 80.0],
                opacity: 0.9,
            });
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
            dirty.pet.as_slice(),
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
            dirty.content_hud.as_slice(),
            &[ByteSpan::slots::<ContentGpuValue>(1, 1)]
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
            dirty.frame_hud.as_slice(),
            &[ByteSpan::slots::<FrameGpuValue>(1, 1)]
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
        assert_eq!(after_frame_globals._padding, before_frame_globals._padding);
        assert_eq!(after_frame_globals.light_count, 1);

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
            let dirty = candidate
                .apply_deltas(&projected.content, &projected.frame)
                .unwrap();
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
            if changes
                .semantic()
                .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::PROP)
            {
                assert!(!dirty.prop_glyphs.as_slice().is_empty());
                saw_prop_content = true;
            }
            if changes
                .semantic()
                .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::TANK)
            {
                assert!(!dirty.tank_glyphs.as_slice().is_empty());
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
        }
        assert!(saw_nodes);
        assert!(saw_globals);
        assert!(saw_prop_content);
        assert!(saw_prop_frame);
        assert!(saw_tank_content);
        assert!(saw_tank_frame);
    }

    #[test]
    fn same_generation_camera_delta_matches_fresh_compile_and_preserves_host_globals() {
        let fixture = SceneFixture::valid();
        let mut candidate = compile_fixture(&fixture);
        let before = candidate.frame.globals.as_slice()[0];
        let camera = crate::presentation::companion_scene::scene::OrthographicCamera::new(
            420.0, 380.0, -2.0, 2.0,
        )
        .unwrap();
        let (content, mut frame) = paired_deltas(
            &candidate,
            crate::presentation::companion_scene::AppliedRevisions::new(4, 6),
        );
        frame.camera = Some(camera);
        let dirty = candidate.apply_deltas(&content, &frame).unwrap();
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
        assert_eq!(after.viewport_points, [420.0, 380.0]);
        assert_eq!(after.view, before.view);
        assert_eq!(after.viewport_pixels, before.viewport_pixels);
        assert_eq!(after.aperture, before.aperture);
        assert_eq!(after._padding, before._padding);
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
