use super::scene::*;
use super::DepthCue;
use super::COMPANION_RENDERER_SCHEMA_VERSION;
use crate::presentation::privacy::PresentationSurface;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneValidationError {
    SchemaVersionMismatch,
    RendererSchemaVersionMismatch,
    FixedCapacityMismatch,
    NodeCapacityExceeded,
    StaticPrimitiveCapacityExceeded,
    PetArtCapacityExceeded,
    RoomGlyphCapacityExceeded,
    PropCapacityExceeded,
    TankCapacityExceeded,
    AmbientCapacityExceeded,
    BlendedDrawCapacityExceeded,
    LightCapacityExceeded,
    AttachmentCapacityExceeded,
    DuplicateNodeId,
    NodeIdCollision,
    DuplicateMaterialId,
    MaterialIdCollision,
    DuplicateResourceId,
    ResourceIdCollision,
    DuplicateAttachmentId,
    AttachmentIdCollision,
    InvalidAttachmentBinding,
    AliasIdMismatch,
    DanglingNodeReference,
    DanglingMaterialReference,
    DanglingResourceReference,
    HierarchyCycle,
    NonFiniteTransform,
    ZeroQuaternion,
    NonFiniteBounds,
    InvalidBounds,
    NonFiniteDepthCue,
    InvalidDepthCue,
    InvalidCamera,
    InvalidGlyphGrid,
    MaterialDepthIncompatible,
    PrimitiveResourceIncompatible,
    LitCardScaleIncompatible,
    PrivacyViolation,
    PetArtSlotOutOfBounds,
    RoomGlyphSlotOutOfBounds,
    PropSlotOutOfBounds,
    TankSlotOutOfBounds,
    AmbientSlotOutOfBounds,
    NodeSlotOutOfBounds,
    LightSlotOutOfBounds,
    DuplicateSlot,
    FixedSlotCountMismatch,
    InvalidPrimitiveBinding,
    DuplicateAuthoredOrder,
    NonCanonicalEmptySlot,
    MissingNodeFrameState,
    NonFiniteFrameValue,
    InvalidFrameValue,
    InvalidContentValue,
    AcceptedStateMismatch,
}

#[derive(Debug, Clone, PartialEq)]
struct LitPathNode {
    id: NodeId,
    dense_index: usize,
    base_transform: Transform3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedSceneTemplate {
    template: SceneTemplate,
    node_dense_indices: HashMap<NodeId, usize>,
    lit_paths: Vec<Vec<LitPathNode>>,
    node_lit_paths: Vec<Vec<usize>>,
    identity: Arc<()>,
}

impl AcceptedSceneTemplate {
    pub fn template(&self) -> &SceneTemplate {
        &self.template
    }
}

#[derive(Debug)]
pub struct AcceptedSceneFrame {
    frame: SceneFrame,
    template_identity: Arc<()>,
    instance_identity: Arc<()>,
    epoch: u64,
}

impl Clone for AcceptedSceneFrame {
    fn clone(&self) -> Self {
        Self {
            frame: self.frame.clone(),
            template_identity: Arc::clone(&self.template_identity),
            instance_identity: Arc::new(()),
            epoch: self.epoch,
        }
    }
}

impl PartialEq for AcceptedSceneFrame {
    fn eq(&self, other: &Self) -> bool {
        self.frame == other.frame && self.template_identity == other.template_identity
    }
}

impl AcceptedSceneFrame {
    pub fn frame(&self) -> &SceneFrame {
        &self.frame
    }

    pub fn into_frame(self) -> SceneFrame {
        self.frame
    }

    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedSceneState {
    template: AcceptedSceneTemplate,
    frame: AcceptedSceneFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDeltaValidation {
    /// Number of changed node records validated from the delta.
    pub node_slots_checked: usize,
    /// Number of changed node records committed in place.
    pub node_slots_applied: usize,
    /// Number of affected-path indices consumed from fixed scratch.
    pub lit_path_indices_visited: usize,
    /// Number of affected lit paths whose candidate transform was validated.
    pub lit_paths_checked: usize,
}

/// Sealed fixed-capacity proof that a frame delta passed every validation step.
/// The token owns exact overlays so commit performs no lookup or allocation.
pub(crate) struct PreparedAcceptedFrameDelta {
    template_identity: usize,
    frame_identity: Arc<()>,
    epoch: u64,
    camera: Option<OrthographicCamera>,
    nodes: [Option<NodeFrameState>; MAX_SCENE_NODES],
    room_glyph_slots: [Option<RoomGlyphFrameSlot>; MAX_ROOM_GLYPH_SLOTS],
    prop_slots: [Option<PropFrameSlot>; MAX_VISIBLE_PROPS],
    tank_slots: [Option<TankFrameSlot>; MAX_ROUND_TANK_INHABITANTS],
    ambient_slots: [Option<AmbientFrameSlot>; MAX_AMBIENT_INSTANCES],
    analytic_slots: [Option<AnalyticFrameSlot>; MAX_ANALYTIC_PARAMS],
    gauges: Option<[f32; 4]>,
    dim_amount: Option<f32>,
    lights: [Option<LightFrame>; MAX_LIGHTS],
    audit: FrameDeltaValidation,
}

impl AcceptedSceneState {
    pub fn template(&self) -> &AcceptedSceneTemplate {
        &self.template
    }

    pub fn frame(&self) -> &AcceptedSceneFrame {
        &self.frame
    }

    pub fn into_parts(self) -> (AcceptedSceneTemplate, AcceptedSceneFrame) {
        (self.template, self.frame)
    }

    #[allow(dead_code)] // Task 8 publishes same-generation projections through this proof.
    pub(crate) fn apply_frame_delta(
        &mut self,
        delta: &FrameDelta,
    ) -> Result<FrameDeltaValidation, SceneValidationError> {
        let prepared = self.prepare_frame_delta(delta)?;
        Ok(self.commit_prepared_frame_delta(prepared))
    }

    pub(crate) fn prepare_frame_delta(
        &self,
        delta: &FrameDelta,
    ) -> Result<PreparedAcceptedFrameDelta, SceneValidationError> {
        prepare_accepted_frame_delta(delta, &self.template, &self.frame)
    }

    pub(crate) fn commit_prepared_frame_delta(
        &mut self,
        prepared: PreparedAcceptedFrameDelta,
    ) -> FrameDeltaValidation {
        commit_accepted_frame_delta(&mut self.frame, prepared)
    }
}

pub fn validate_full_generation(
    template: &SceneTemplate,
    content: &SceneContent,
    frame: &SceneFrame,
) -> Result<AcceptedSceneState, SceneValidationError> {
    let accepted = validate_template(template)?;
    validate_content(content)?;
    let accepted_frame = validate_frame(frame, &accepted)?;
    validate_content_frame_canonical(content, accepted_frame.frame())?;
    for attachment in &template.attachments {
        if let Some(AttachmentInstanceBinding::PropGlyphs(slot)) = attachment.instance_binding {
            if content.prop_slots[usize::from(slot)].content.is_none() {
                return Err(SceneValidationError::InvalidAttachmentBinding);
            }
        }
        resolve_attachment_world(template, accepted_frame.frame(), attachment)
            .map_err(|_| SceneValidationError::InvalidAttachmentBinding)?;
    }
    Ok(AcceptedSceneState {
        template: accepted,
        frame: accepted_frame,
    })
}

pub fn validate_template(
    template: &SceneTemplate,
) -> Result<AcceptedSceneTemplate, SceneValidationError> {
    validate_versions(template.schema_version, template.renderer_schema_version)?;
    validate_capacity_counts(template)?;
    validate_fixed_capacities(template.capacities)?;
    validate_glyph_grid(template.glyph_grid)?;
    validate_nodes(template)?;
    validate_materials(template)?;
    validate_resources(template)?;
    validate_static_atlas_recipes(&template.static_atlas_recipes)?;
    validate_analytic_templates(&template.analytic_templates)?;
    validate_attachments(template)?;
    validate_hierarchy(template)?;
    validate_primitives(template)?;
    validate_companion_instance_sources(template)?;
    let node_dense_indices = template
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect::<HashMap<_, _>>();
    let lit_paths = collect_lit_card_paths(template, &node_dense_indices)?;
    validate_lit_card_world_transforms(&lit_paths, |_| None)?;
    let mut node_lit_paths = vec![Vec::new(); template.nodes.len()];
    for (path_index, path) in lit_paths.iter().enumerate() {
        for node in path {
            node_lit_paths[node.dense_index].push(path_index);
        }
    }
    validate_privacy(template)?;
    Ok(AcceptedSceneTemplate {
        template: template.clone(),
        node_dense_indices,
        lit_paths,
        node_lit_paths,
        identity: Arc::new(()),
    })
}

pub fn validate_content(content: &SceneContent) -> Result<(), SceneValidationError> {
    validate_versions(content.schema_version, content.renderer_schema_version)?;
    if content.pet_art_slots.len() > MAX_PET_ART_SLOTS {
        return Err(SceneValidationError::PetArtCapacityExceeded);
    }
    if content.room_glyph_slots.len() > MAX_ROOM_GLYPH_SLOTS {
        return Err(SceneValidationError::RoomGlyphCapacityExceeded);
    }
    if content.prop_slots.len() > MAX_VISIBLE_PROPS {
        return Err(SceneValidationError::PropCapacityExceeded);
    }
    if content.tank_slots.len() > MAX_ROUND_TANK_INHABITANTS {
        return Err(SceneValidationError::TankCapacityExceeded);
    }
    if content.ambient_slots.len() > MAX_AMBIENT_INSTANCES {
        return Err(SceneValidationError::AmbientCapacityExceeded);
    }
    if content.pet_art_slots.len() != MAX_PET_ART_SLOTS
        || content.room_glyph_slots.len() != MAX_ROOM_GLYPH_SLOTS
        || content.prop_slots.len() != MAX_VISIBLE_PROPS
        || content.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || content.ambient_slots.len() != MAX_AMBIENT_INSTANCES
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    validate_content_slots(
        &content.pet_art_slots,
        &content.room_glyph_slots,
        &content.prop_slots,
        &content.tank_slots,
        &content.ambient_slots,
    )?;
    validate_analytic_content_slots(&content.analytic_slots, true)?;
    validate_paint_slots(
        &content.prop_slots,
        &content.prop_paint_slots,
        &content.ambient_slots,
        &content.ambient_paint_slots,
        true,
    )?;
    if content
        .pet_art_slots
        .iter()
        .enumerate()
        .any(|(index, slot)| usize::from(slot.slot) != index)
        || content
            .room_glyph_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || content
            .prop_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || content
            .tank_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || content
            .ambient_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || content
            .prop_paint_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || content
            .ambient_paint_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    Ok(())
}

pub fn validate_frame(
    frame: &SceneFrame,
    accepted_template: &AcceptedSceneTemplate,
) -> Result<AcceptedSceneFrame, SceneValidationError> {
    validate_frame_against_template(frame, accepted_template)?;
    let by_id = frame
        .nodes
        .iter()
        .map(|node| (node.node, *node))
        .collect::<HashMap<_, _>>();
    let mut canonical = frame.clone();
    canonical.nodes = accepted_template
        .template
        .nodes
        .iter()
        .map(|node| by_id[&node.id])
        .collect();
    canonical.room_glyph_slots.sort_by_key(|slot| slot.slot);
    canonical.prop_slots.sort_by_key(|slot| slot.slot);
    canonical.tank_slots.sort_by_key(|slot| slot.slot);
    canonical.ambient_slots.sort_by_key(|slot| slot.slot);
    canonical.analytic_slots.sort_by_key(|slot| slot.id);
    Ok(AcceptedSceneFrame {
        frame: canonical,
        template_identity: Arc::clone(&accepted_template.identity),
        instance_identity: Arc::new(()),
        epoch: 0,
    })
}

fn validate_frame_against_template(
    frame: &SceneFrame,
    accepted_template: &AcceptedSceneTemplate,
) -> Result<(), SceneValidationError> {
    validate_versions(frame.schema_version, frame.renderer_schema_version)?;
    validate_camera(frame.camera)?;
    validate_room_frame_grid(
        frame.room_glyph_slots.iter().copied(),
        accepted_template.template.glyph_grid,
        frame.camera,
    )?;
    if frame.nodes.len() > MAX_SCENE_NODES {
        return Err(SceneValidationError::NodeCapacityExceeded);
    }
    if frame.lights.len() > MAX_LIGHTS {
        return Err(SceneValidationError::LightCapacityExceeded);
    }
    if frame.prop_slots.len() != MAX_VISIBLE_PROPS
        || frame.room_glyph_slots.len() != MAX_ROOM_GLYPH_SLOTS
        || frame.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || frame.ambient_slots.len() != MAX_AMBIENT_INSTANCES
        || frame.analytic_slots.len() != MAX_ANALYTIC_PARAMS
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    validate_instance_frame_slots(frame)?;
    validate_analytic_frame_slots(&frame.analytic_slots, true, frame.camera)?;
    if frame
        .prop_slots
        .iter()
        .enumerate()
        .any(|(index, slot)| usize::from(slot.slot) != index)
        || frame
            .room_glyph_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || frame
            .tank_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || frame
            .ambient_slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    let mut seen = HashSet::new();
    for node in &frame.nodes {
        if !accepted_template
            .node_dense_indices
            .contains_key(&node.node)
        {
            return Err(SceneValidationError::DanglingNodeReference);
        }
        if !seen.insert(node.node) {
            return Err(SceneValidationError::DuplicateSlot);
        }
        validate_transform(node.local_transform)?;
        validate_unit_interval(node.opacity)?;
    }
    if seen.len() != accepted_template.node_dense_indices.len() {
        return Err(SceneValidationError::MissingNodeFrameState);
    }
    let frame_transforms = frame
        .nodes
        .iter()
        .map(|node| (node.node, node.local_transform))
        .collect::<HashMap<_, _>>();
    validate_lit_card_world_transforms(&accepted_template.lit_paths, |id| {
        frame_transforms.get(&id).copied()
    })?;
    validate_instance_frame_slots(frame)?;
    validate_frame_scalars(frame.gauges, frame.dim_amount, &frame.lights)
}

/// Validates only bounded mutable content slots. It deliberately does not run
/// generation identity, hierarchy, material, resource, or privacy validation.
pub fn validate_content_delta(delta: &ContentDelta) -> Result<(), SceneValidationError> {
    validate_versions(delta.schema_version, delta.renderer_schema_version)?;
    validate_content_slots(
        &delta.pet_art_slots,
        &delta.room_glyph_slots,
        &delta.prop_slots,
        &delta.tank_slots,
        &delta.ambient_slots,
    )?;
    validate_analytic_content_slots(&delta.analytic_slots, false)?;
    if delta.prop_slots.len() != delta.prop_paint_slots.len()
        || delta.prop_slots.iter().any(|content| {
            !delta
                .prop_paint_slots
                .iter()
                .any(|paint| paint.slot == content.slot)
        })
        || delta.ambient_slots.len() != delta.ambient_paint_slots.len()
        || delta.ambient_slots.iter().any(|content| {
            !delta
                .ambient_paint_slots
                .iter()
                .any(|paint| paint.slot == content.slot)
        })
    {
        return Err(SceneValidationError::NonCanonicalEmptySlot);
    }
    validate_paint_slots(
        &delta.prop_slots,
        &delta.prop_paint_slots,
        &delta.ambient_slots,
        &delta.ambient_paint_slots,
        false,
    )
}

#[allow(dead_code)] // Task 8 calls this before committing paired content/frame deltas.
pub(crate) fn validate_content_frame_delta(
    current_content: &SceneContent,
    current_frame: &SceneFrame,
    content_delta: &ContentDelta,
    frame_delta: &FrameDelta,
) -> Result<(), SceneValidationError> {
    if content_delta.generation_key != frame_delta.generation_key
        || content_delta.from != frame_delta.from
        || content_delta.to != frame_delta.to
    {
        return Err(SceneValidationError::AcceptedStateMismatch);
    }
    validate_content_delta(content_delta)?;
    let zero2 = |value: [f32; 2]| value.into_iter().all(|component| component.to_bits() == 0);
    let zero4 = |value: [f32; 4]| value.into_iter().all(|component| component.to_bits() == 0);
    for slot in 0..MAX_ROOM_GLYPH_SLOTS {
        let content = content_delta
            .room_glyph_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_content.room_glyph_slots[slot]);
        let frame = frame_delta
            .room_glyph_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_frame.room_glyph_slots[slot]);
        if content.glyph.is_some() != frame.visible
            || (content.glyph.is_some() && frame.opacity <= 0.0)
            || (content.glyph.is_none()
                && (frame.grid_cell != [0; 2]
                    || !zero2(frame.position_points)
                    || frame.opacity.to_bits() != 0))
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    for slot in 0..MAX_VISIBLE_PROPS {
        let content = content_delta
            .prop_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_content.prop_slots[slot]);
        let frame = frame_delta
            .prop_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_frame.prop_slots[slot]);
        if content.content.is_none()
            && (frame.visible
                || !zero2(frame.origin_points)
                || !zero2(frame.motion_offset_points)
                || frame.opacity.to_bits() != 0)
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    for slot in 0..MAX_ROUND_TANK_INHABITANTS {
        let content = content_delta
            .tank_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_content.tank_slots[slot]);
        let frame = frame_delta
            .tank_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_frame.tank_slots[slot]);
        if content.content.is_none() && (frame.visible || !zero2(frame.origin_points)) {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
        let glyphs = content
            .content
            .map(|content| content.glyphs)
            .unwrap_or([None; MAX_TANK_GLYPHS_PER_SLOT]);
        for (glyph, cell) in glyphs.into_iter().zip(frame.cells) {
            if glyph.is_none()
                && (cell.visible
                    || !zero2(cell.position_points)
                    || !zero4(cell.bounds_points)
                    || cell.layer != InstanceLayer::Behind)
            {
                return Err(SceneValidationError::NonCanonicalEmptySlot);
            }
        }
    }
    for slot in 0..MAX_AMBIENT_INSTANCES {
        let content = content_delta
            .ambient_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_content.ambient_slots[slot]);
        let frame = frame_delta
            .ambient_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_frame.ambient_slots[slot]);
        if content.kind.is_none()
            && (frame.visible || !zero2(frame.position_points) || frame.opacity.to_bits() != 0)
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    validate_delta_paint_overlays(current_content, content_delta)?;
    for slot in 0..MAX_ANALYTIC_PARAMS {
        let content = content_delta
            .analytic_slots
            .iter()
            .find(|changed| usize::from(changed.id.0) == slot)
            .unwrap_or(&current_content.analytic_slots[slot]);
        let frame = frame_delta
            .analytic_slots
            .iter()
            .find(|changed| usize::from(changed.id.0) == slot)
            .unwrap_or(&current_frame.analytic_slots[slot]);
        if content.value.map(|value| (value.semantic, value.shape))
            != frame.value.map(|value| (value.semantic, value.shape))
        {
            return Err(SceneValidationError::InvalidFrameValue);
        }
    }
    Ok(())
}

/// Validates changed fields and affected lit paths over an accepted current
/// frame, then transactionally applies only those changed slots.
/// It deliberately does not re-run full template validation or rescan primitives.
pub fn validate_frame_delta(
    delta: &FrameDelta,
    accepted_template: &AcceptedSceneTemplate,
    current_frame: &mut AcceptedSceneFrame,
) -> Result<FrameDeltaValidation, SceneValidationError> {
    let prepared = prepare_accepted_frame_delta(delta, accepted_template, current_frame)?;
    Ok(commit_accepted_frame_delta(current_frame, prepared))
}

fn prepare_accepted_frame_delta(
    delta: &FrameDelta,
    accepted_template: &AcceptedSceneTemplate,
    current_frame: &AcceptedSceneFrame,
) -> Result<PreparedAcceptedFrameDelta, SceneValidationError> {
    if !Arc::ptr_eq(
        &accepted_template.identity,
        &current_frame.template_identity,
    ) {
        return Err(SceneValidationError::AcceptedStateMismatch);
    }
    validate_versions(delta.schema_version, delta.renderer_schema_version)?;
    if delta.nodes.len() > MAX_SCENE_NODES {
        return Err(SceneValidationError::NodeCapacityExceeded);
    }
    if delta.lights.len() > MAX_LIGHTS {
        return Err(SceneValidationError::LightCapacityExceeded);
    }
    validate_changed_instance_frame_slots(delta)?;
    if let Some(camera) = delta.camera {
        validate_camera(camera)?;
    }
    let effective_camera = delta.camera.unwrap_or(current_frame.frame.camera);
    validate_analytic_frame_slots(&delta.analytic_slots, false, effective_camera)?;
    validate_room_frame_grid(
        (0..MAX_ROOM_GLYPH_SLOTS).map(|slot| {
            delta
                .room_glyph_slots
                .iter()
                .find(|changed| usize::from(changed.slot) == slot)
                .copied()
                .unwrap_or(current_frame.frame.room_glyph_slots[slot])
        }),
        accepted_template.template.glyph_grid,
        effective_camera,
    )?;
    if let Some(gauges) = delta.gauges {
        for gauge in gauges {
            validate_unit_interval(gauge)?;
        }
    }
    if let Some(dim_amount) = delta.dim_amount {
        validate_unit_interval(dim_amount)?;
    }
    let mut node_overlay = [None; MAX_SCENE_NODES];
    let mut changed_dense_indices = [None; MAX_SCENE_NODES];
    let mut affected_path_seen = [false; MAX_STATIC_PRIMITIVES];
    let mut affected_path_indices = [0; MAX_STATIC_PRIMITIVES];
    let mut affected_path_count = 0;
    for (change_index, node) in delta.nodes.iter().enumerate() {
        let dense_index = *accepted_template
            .node_dense_indices
            .get(&node.node)
            .ok_or(SceneValidationError::NodeSlotOutOfBounds)?;
        if node_overlay[dense_index].replace(*node).is_some() {
            return Err(SceneValidationError::DuplicateSlot);
        }
        changed_dense_indices[change_index] = Some(dense_index);
        validate_transform(node.local_transform)?;
        validate_unit_interval(node.opacity)?;
        for path_index in &accepted_template.node_lit_paths[dense_index] {
            if !affected_path_seen[*path_index] {
                affected_path_seen[*path_index] = true;
                affected_path_indices[affected_path_count] = *path_index;
                affected_path_count += 1;
            }
        }
    }

    let mut light_overlay = [None; MAX_LIGHTS];
    for (slot, light) in &delta.lights {
        let slot = usize::from(*slot);
        if slot >= current_frame.frame.lights.len() {
            return Err(SceneValidationError::LightSlotOutOfBounds);
        }
        if light_overlay[slot].replace(*light).is_some() {
            return Err(SceneValidationError::DuplicateSlot);
        }
        validate_light(*light)?;
    }

    let mut lit_paths_checked = 0;
    for path_index in affected_path_indices[..affected_path_count].iter().copied() {
        validate_lit_card_path_overlay(
            &accepted_template.lit_paths[path_index],
            &current_frame.frame,
            &node_overlay,
        )?;
        lit_paths_checked += 1;
    }

    if changed_dense_indices[..delta.nodes.len()]
        .iter()
        .any(Option::is_none)
    {
        return Err(SceneValidationError::AcceptedStateMismatch);
    }

    let mut room_glyph_slots = [None; MAX_ROOM_GLYPH_SLOTS];
    for changed in &delta.room_glyph_slots {
        room_glyph_slots[usize::from(changed.slot)] = Some(*changed);
    }
    let mut prop_slots = [None; MAX_VISIBLE_PROPS];
    for changed in &delta.prop_slots {
        prop_slots[usize::from(changed.slot)] = Some(*changed);
    }
    let mut tank_slots = [None; MAX_ROUND_TANK_INHABITANTS];
    for changed in &delta.tank_slots {
        tank_slots[usize::from(changed.slot)] = Some(*changed);
    }
    let mut ambient_slots = [None; MAX_AMBIENT_INSTANCES];
    for changed in &delta.ambient_slots {
        ambient_slots[usize::from(changed.slot)] = Some(*changed);
    }
    let mut analytic_slots = [None; MAX_ANALYTIC_PARAMS];
    for changed in &delta.analytic_slots {
        analytic_slots[usize::from(changed.id.0)] = Some(*changed);
    }
    let audit = FrameDeltaValidation {
        node_slots_checked: delta.nodes.len(),
        node_slots_applied: delta.nodes.len(),
        lit_path_indices_visited: affected_path_count,
        lit_paths_checked,
    };
    Ok(PreparedAcceptedFrameDelta {
        template_identity: Arc::as_ptr(&accepted_template.identity).addr(),
        frame_identity: Arc::clone(&current_frame.instance_identity),
        epoch: current_frame.epoch,
        camera: delta.camera,
        nodes: node_overlay,
        room_glyph_slots,
        prop_slots,
        tank_slots,
        ambient_slots,
        analytic_slots,
        gauges: delta.gauges,
        dim_amount: delta.dim_amount,
        lights: light_overlay,
        audit,
    })
}

fn commit_accepted_frame_delta(
    current_frame: &mut AcceptedSceneFrame,
    prepared: PreparedAcceptedFrameDelta,
) -> FrameDeltaValidation {
    assert_eq!(
        prepared.template_identity,
        Arc::as_ptr(&current_frame.template_identity).addr(),
        "prepared frame delta belongs to another accepted template"
    );
    assert!(
        Arc::ptr_eq(&prepared.frame_identity, &current_frame.instance_identity),
        "prepared frame delta belongs to another accepted frame"
    );
    assert_eq!(
        prepared.epoch, current_frame.epoch,
        "prepared frame delta is stale for this accepted frame"
    );
    if let Some(camera) = prepared.camera {
        current_frame.frame.camera = camera;
    }
    for (slot, changed) in prepared.nodes.into_iter().enumerate() {
        if let Some(changed) = changed {
            current_frame.frame.nodes[slot] = changed;
        }
    }
    if let Some(gauges) = prepared.gauges {
        current_frame.frame.gauges = gauges;
    }
    if let Some(dim_amount) = prepared.dim_amount {
        current_frame.frame.dim_amount = dim_amount;
    }
    for (slot, changed) in prepared.lights.into_iter().enumerate() {
        if let Some(changed) = changed {
            current_frame.frame.lights[slot] = changed;
        }
    }
    for (slot, changed) in prepared.prop_slots.into_iter().enumerate() {
        if let Some(changed) = changed {
            current_frame.frame.prop_slots[slot] = changed;
        }
    }
    for (slot, changed) in prepared.room_glyph_slots.into_iter().enumerate() {
        if let Some(changed) = changed {
            current_frame.frame.room_glyph_slots[slot] = changed;
        }
    }
    for (slot, changed) in prepared.tank_slots.into_iter().enumerate() {
        if let Some(changed) = changed {
            current_frame.frame.tank_slots[slot] = changed;
        }
    }
    for (slot, changed) in prepared.ambient_slots.into_iter().enumerate() {
        if let Some(changed) = changed {
            current_frame.frame.ambient_slots[slot] = changed;
        }
    }
    for (slot, changed) in prepared.analytic_slots.into_iter().enumerate() {
        if let Some(changed) = changed {
            current_frame.frame.analytic_slots[slot] = changed;
        }
    }
    current_frame.epoch = current_frame.epoch.wrapping_add(1);
    prepared.audit
}

fn validate_versions(schema: u16, renderer: u16) -> Result<(), SceneValidationError> {
    if schema != SCENE_CONTRACT_SCHEMA_VERSION {
        return Err(SceneValidationError::SchemaVersionMismatch);
    }
    if renderer != COMPANION_RENDERER_SCHEMA_VERSION {
        return Err(SceneValidationError::RendererSchemaVersionMismatch);
    }
    Ok(())
}

fn validate_glyph_grid(grid: super::CompanionGlyphGrid) -> Result<(), SceneValidationError> {
    if grid.columns == 0
        || grid.rows == 0
        || grid.y_up_origin_points != [0.0, 0.0]
        || !grid
            .y_up_origin_points
            .iter()
            .chain(&grid.cell_extent_points)
            .all(|value| value.is_finite())
        || grid.cell_extent_points.iter().any(|value| *value <= 0.0)
        || grid.scale != super::LogicalGlyphScale::OneCell
        || grid.anchor != super::LogicalGlyphAnchor::CellBottomLeft
    {
        return Err(SceneValidationError::InvalidGlyphGrid);
    }
    Ok(())
}

fn validate_room_frame_grid(
    slots: impl Iterator<Item = RoomGlyphFrameSlot>,
    grid: super::CompanionGlyphGrid,
    camera: OrthographicCamera,
) -> Result<(), SceneValidationError> {
    let canonical_extent = [
        camera.width_points / f32::from(grid.columns),
        camera.height_points / f32::from(grid.rows),
    ];
    if grid.cell_extent_points != canonical_extent {
        return Err(SceneValidationError::InvalidGlyphGrid);
    }
    for slot in slots.filter(|slot| slot.visible) {
        if slot.grid_cell[0] >= grid.columns || slot.grid_cell[1] >= grid.rows {
            return Err(SceneValidationError::InvalidGlyphGrid);
        }
        let expected = [
            grid.y_up_origin_points[0] + f32::from(slot.grid_cell[0]) * grid.cell_extent_points[0],
            grid.y_up_origin_points[1] + camera.height_points
                - (f32::from(slot.grid_cell[1]) + 1.0) * grid.cell_extent_points[1],
        ];
        if slot.position_points != expected {
            return Err(SceneValidationError::InvalidGlyphGrid);
        }
    }
    Ok(())
}

fn validate_capacity_counts(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    if template.nodes.len() > template.capacities.max_nodes
        || template.nodes.len() > MAX_SCENE_NODES
    {
        return Err(SceneValidationError::NodeCapacityExceeded);
    }
    if template.primitives.len() > template.capacities.max_static_primitives
        || template.primitives.len() > MAX_STATIC_PRIMITIVES
    {
        return Err(SceneValidationError::StaticPrimitiveCapacityExceeded);
    }
    let materials = template
        .materials
        .iter()
        .map(|material| (material.id, material.kind))
        .collect::<HashMap<_, _>>();
    let blended_draws = template
        .primitives
        .iter()
        .filter(|primitive| {
            is_world_blended(primitive.blend, materials.get(&primitive.material).copied())
        })
        .count();
    if blended_draws > template.capacities.max_blended_draws || blended_draws > MAX_BLENDED_DRAWS {
        return Err(SceneValidationError::BlendedDrawCapacityExceeded);
    }
    if template.attachments.len() > template.capacities.max_attachments
        || template.attachments.len() > MAX_ATTACHMENTS
    {
        return Err(SceneValidationError::AttachmentCapacityExceeded);
    }
    Ok(())
}

fn validate_fixed_capacities(capacities: SceneCapacities) -> Result<(), SceneValidationError> {
    if capacities != SceneCapacities::FIXED_V2 {
        return Err(SceneValidationError::FixedCapacityMismatch);
    }
    Ok(())
}

fn validate_nodes(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    let mut ids = HashMap::<NodeId, &CanonicalAlias>::new();
    for node in &template.nodes {
        if NodeId::from_alias(&node.alias) != node.id {
            return Err(SceneValidationError::AliasIdMismatch);
        }
        if let Some(previous) = ids.insert(node.id, &node.alias) {
            return Err(if previous == &node.alias {
                SceneValidationError::DuplicateNodeId
            } else {
                SceneValidationError::NodeIdCollision
            });
        }
        validate_transform(node.base_transform)?;
        validate_bounds(node.local_bounds)?;
        validate_depth_cue(node.depth_cue)?;
    }
    Ok(())
}

fn validate_materials(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    let mut ids = HashMap::<MaterialId, &CanonicalAlias>::new();
    for material in &template.materials {
        if MaterialId::from_alias(&material.alias) != material.id {
            return Err(SceneValidationError::AliasIdMismatch);
        }
        if let Some(previous) = ids.insert(material.id, &material.alias) {
            return Err(if previous == &material.alias {
                SceneValidationError::DuplicateMaterialId
            } else {
                SceneValidationError::MaterialIdCollision
            });
        }
    }
    Ok(())
}

fn validate_resources(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    let mut ids = HashMap::<ResourceId, &CanonicalAlias>::new();
    for resource in &template.resources {
        if ResourceId::from_alias(&resource.alias) != resource.id {
            return Err(SceneValidationError::AliasIdMismatch);
        }
        if let Some(previous) = ids.insert(resource.id, &resource.alias) {
            return Err(if previous == &resource.alias {
                SceneValidationError::DuplicateResourceId
            } else {
                SceneValidationError::ResourceIdCollision
            });
        }
    }
    Ok(())
}

fn validate_static_atlas_recipes(
    slots: &[StaticAtlasRecipeSlot],
) -> Result<(), SceneValidationError> {
    if slots.len() != MAX_STATIC_ATLAS_RECIPES
        || slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.id.0) != index)
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    for slot in slots {
        if let Some(recipe) = slot.recipe {
            validate_bounds(recipe.local_bounds)?;
        }
    }
    Ok(())
}

fn validate_analytic_templates(slots: &[AnalyticTemplateSlot]) -> Result<(), SceneValidationError> {
    let unit_bounds = Bounds3 { min: [0.0; 3], max: [1.0, 1.0, 0.0] };
    if slots.len() != MAX_ANALYTIC_PARAMS {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    for (index, slot) in slots.iter().enumerate() {
        if usize::from(slot.id.0) != index {
            return Err(SceneValidationError::DuplicateSlot);
        }
        match (index, slot.value) {
            (0..=7, Some(value))
                if value.semantic == AnalyticSemantic::ALL[index]
                    && value.semantic.id() == slot.id
                    && value.shape == value.semantic.shape()
                    && value.normalized_local_bounds == unit_bounds =>
            {
                validate_bounds(value.normalized_local_bounds)?;
            }
            (8.., None) => {}
            _ => return Err(SceneValidationError::NonCanonicalEmptySlot),
        }
    }
    Ok(())
}

fn validate_attachments(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    let node_ids = template
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<HashSet<_>>();
    let mut ids = HashMap::<AttachmentId, &CanonicalAlias>::new();
    for attachment in &template.attachments {
        if AttachmentId::from_alias(&attachment.alias) != attachment.id {
            return Err(SceneValidationError::AliasIdMismatch);
        }
        if let Some(previous) = ids.insert(attachment.id, &attachment.alias) {
            return Err(if previous == &attachment.alias {
                SceneValidationError::DuplicateAttachmentId
            } else {
                SceneValidationError::AttachmentIdCollision
            });
        }
        if !node_ids.contains(&attachment.owner) {
            return Err(SceneValidationError::DanglingNodeReference);
        }
        validate_transform(attachment.local)?;
        if let Some(AttachmentInstanceBinding::PropGlyphs(slot)) = attachment.instance_binding {
            if usize::from(slot) >= MAX_VISIBLE_PROPS {
                return Err(SceneValidationError::InvalidAttachmentBinding);
            }
            let mut sources = template
                .primitives
                .iter()
                .filter(|primitive| {
                    primitive.binding
                        == PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(slot))
                })
                .map(|primitive| primitive.node);
            let source = sources
                .next()
                .ok_or(SceneValidationError::InvalidAttachmentBinding)?;
            if sources.next().is_some() {
                return Err(SceneValidationError::InvalidAttachmentBinding);
            }
            let mut current = Some(attachment.owner);
            let mut visited = 0;
            let mut found = false;
            while let Some(id) = current {
                if id == source {
                    found = true;
                    break;
                }
                visited += 1;
                if visited > MAX_SCENE_NODES {
                    return Err(SceneValidationError::HierarchyCycle);
                }
                current = template
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .and_then(|node| node.parent);
            }
            if !found {
                return Err(SceneValidationError::InvalidAttachmentBinding);
            }
        }
    }
    Ok(())
}

fn validate_hierarchy(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    let parents = template
        .nodes
        .iter()
        .map(|node| (node.id, node.parent))
        .collect::<HashMap<_, _>>();
    for node in &template.nodes {
        if node
            .parent
            .is_some_and(|parent| !parents.contains_key(&parent))
        {
            return Err(SceneValidationError::DanglingNodeReference);
        }
        let mut current = Some(node.id);
        let mut path = HashSet::new();
        while let Some(id) = current {
            if !path.insert(id) {
                return Err(SceneValidationError::HierarchyCycle);
            }
            current = *parents
                .get(&id)
                .ok_or(SceneValidationError::DanglingNodeReference)?;
        }
    }
    Ok(())
}

fn validate_primitives(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    let node_ids = template
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<HashSet<_>>();
    let materials = template
        .materials
        .iter()
        .map(|material| (material.id, material.kind))
        .collect::<HashMap<_, _>>();
    let resources = template
        .resources
        .iter()
        .map(|resource| (resource.id, resource.kind))
        .collect::<HashMap<_, _>>();
    let mut authored_orders = HashSet::new();
    let mut instance_bindings = Vec::new();
    for primitive in &template.primitives {
        if !node_ids.contains(&primitive.node) {
            return Err(SceneValidationError::DanglingNodeReference);
        }
        let material = materials
            .get(&primitive.material)
            .copied()
            .ok_or(SceneValidationError::DanglingMaterialReference)?;
        let resource = primitive
            .resource
            .map(|id| {
                resources
                    .get(&id)
                    .copied()
                    .ok_or(SceneValidationError::DanglingResourceReference)
            })
            .transpose()?;
        if !material_primitive_compatible(material, primitive.kind)
            || !material_depth_compatible(material, primitive)
        {
            return Err(SceneValidationError::MaterialDepthIncompatible);
        }
        if !primitive_resource_compatible(primitive.kind, resource) {
            return Err(SceneValidationError::PrimitiveResourceIncompatible);
        }
        validate_bounds(primitive.local_geometry)?;
        if !authored_orders.insert(primitive.authored_order) {
            return Err(SceneValidationError::DuplicateAuthoredOrder);
        }
        let binding_matches = match (primitive.kind, primitive.binding) {
            (PrimitiveKind::InstanceQuad, PrimitiveBinding::Instances(binding)) => {
                let slot_in_bounds = match binding {
                    InstanceGroupBinding::PropGlyphs(slot) => usize::from(slot) < MAX_VISIBLE_PROPS,
                    InstanceGroupBinding::TankCells { slot, .. } => {
                        usize::from(slot) < MAX_ROUND_TANK_INHABITANTS
                    }
                    _ => true,
                };
                slot_in_bounds && instance_binding_semantics_match(binding, material, primitive)
            }
            (PrimitiveKind::InstanceQuad, PrimitiveBinding::ShallowCard) => false,
            (PrimitiveKind::AnalyticShape, PrimitiveBinding::Analytic(id)) => {
                usize::from(id.0) < MAX_ANALYTIC_PARAMS
                    && template.analytic_templates[usize::from(id.0)]
                        .value
                        .is_some_and(|value| {
                            analytic_binding_semantics_match(value.semantic, material, primitive)
                        })
            }
            (PrimitiveKind::AtlasQuad, PrimitiveBinding::StaticAtlas(id)) => {
                usize::from(id.0) < MAX_STATIC_ATLAS_RECIPES
                    && template.static_atlas_recipes[usize::from(id.0)]
                        .recipe
                        .is_some()
            }
            (PrimitiveKind::ShallowCard, PrimitiveBinding::ShallowCard) => true,
            (_, _) => false,
        };
        let space_matches = match material {
            MaterialKind::ScreenChrome => primitive.space == PrimitiveSpace::Screen,
            _ => primitive.space == PrimitiveSpace::World,
        };
        if !binding_matches || !space_matches {
            return Err(SceneValidationError::InvalidPrimitiveBinding);
        }
        if let PrimitiveBinding::Instances(binding) = primitive.binding {
            if instance_bindings.contains(&binding) {
                return Err(SceneValidationError::InvalidPrimitiveBinding);
            }
            instance_bindings.push(binding);
        }
    }
    Ok(())
}

fn instance_binding_semantics_match(
    binding: InstanceGroupBinding,
    material: MaterialKind,
    primitive: &PrimitiveTemplate,
) -> bool {
    let (expected_material, expected_blend, expected_depth, expected_space) = match binding {
        InstanceGroupBinding::PetArt(PetArtFilter::Particles) | InstanceGroupBinding::Ambient => (
            MaterialKind::AdditiveGlow,
            WorldBlend::Additive,
            DepthBehavior::WorldReadOnly,
            PrimitiveSpace::World,
        ),
        InstanceGroupBinding::Hud => (
            MaterialKind::ScreenChrome,
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::ScreenNoDepth,
            PrimitiveSpace::Screen,
        ),
        InstanceGroupBinding::RoomGlyphs
        | InstanceGroupBinding::PetArt(PetArtFilter::Body)
        | InstanceGroupBinding::PropGlyphs(_)
        | InstanceGroupBinding::TankCells { .. } => (
            MaterialKind::UnlitGlyphSprite,
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::WorldReadOnly,
            PrimitiveSpace::World,
        ),
    };
    material == expected_material
        && primitive.blend == expected_blend
        && primitive.depth == expected_depth
        && primitive.space == expected_space
}

fn analytic_binding_semantics_match(
    semantic: AnalyticSemantic,
    material: MaterialKind,
    primitive: &PrimitiveTemplate,
) -> bool {
    let (expected_material, expected_blend, expected_depth, expected_space) = match semantic {
        AnalyticSemantic::RoomBackground => (
            MaterialKind::UnlitAnalytic,
            WorldBlend::Opaque,
            DepthBehavior::WorldWrite,
            PrimitiveSpace::World,
        ),
        AnalyticSemantic::WallShadow | AnalyticSemantic::FloorProjection => (
            MaterialKind::MultiplyShadow,
            WorldBlend::Multiply,
            DepthBehavior::WorldReadOnly,
            PrimitiveSpace::World,
        ),
        AnalyticSemantic::MoodAura => (
            MaterialKind::UnlitAnalytic,
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::WorldReadOnly,
            PrimitiveSpace::World,
        ),
        AnalyticSemantic::StatusHalo
        | AnalyticSemantic::Gauges
        | AnalyticSemantic::Trouble
        | AnalyticSemantic::Dim => (
            MaterialKind::ScreenChrome,
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::ScreenNoDepth,
            PrimitiveSpace::Screen,
        ),
    };
    material == expected_material
        && primitive.blend == expected_blend
        && primitive.depth == expected_depth
        && primitive.space == expected_space
}

fn validate_companion_instance_sources(
    template: &SceneTemplate,
) -> Result<(), SceneValidationError> {
    if !template
        .nodes
        .iter()
        .any(|node| node.alias.as_str() == "world.room.glyphs")
    {
        return Ok(());
    }
    let expected = [
        (
            "world.room.glyphs",
            PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs),
        ),
        (
            "pet.body",
            PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body)),
        ),
        (
            "pet.particles",
            PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Particles)),
        ),
    ];
    let mut resources = [None; 3];
    for (expected_index, (alias, binding)) in expected.into_iter().enumerate() {
        let node = template
            .nodes
            .iter()
            .find(|node| node.alias.as_str() == alias)
            .ok_or(SceneValidationError::InvalidPrimitiveBinding)?;
        let mut matches = template
            .primitives
            .iter()
            .filter(|primitive| primitive.binding == binding);
        let primitive = matches
            .next()
            .ok_or(SceneValidationError::InvalidPrimitiveBinding)?;
        if matches.next().is_some() || primitive.node != node.id {
            return Err(SceneValidationError::InvalidPrimitiveBinding);
        }
        resources[expected_index] = primitive.resource;
    }
    if resources[1].is_none() || resources[1] != resources[2] {
        return Err(SceneValidationError::InvalidPrimitiveBinding);
    }
    if template
        .static_atlas_recipes
        .iter()
        .any(|slot| slot.recipe.is_some())
    {
        return Err(SceneValidationError::InvalidPrimitiveBinding);
    }
    let analytic_owners = [
        ("world.room.background", AnalyticSemantic::RoomBackground),
        ("pet.shadow.wall", AnalyticSemantic::WallShadow),
        ("pet.projection.floor", AnalyticSemantic::FloorProjection),
        ("chrome.status", AnalyticSemantic::StatusHalo),
        ("pet.aura.mood", AnalyticSemantic::MoodAura),
        ("chrome.gauges", AnalyticSemantic::Gauges),
        ("chrome.trouble", AnalyticSemantic::Trouble),
        ("chrome.dim", AnalyticSemantic::Dim),
    ];
    for (alias, semantic) in analytic_owners {
        let node = template
            .nodes
            .iter()
            .find(|node| node.alias.as_str() == alias)
            .ok_or(SceneValidationError::InvalidPrimitiveBinding)?;
        let binding = PrimitiveBinding::Analytic(semantic.id());
        let mut matches = template
            .primitives
            .iter()
            .filter(|primitive| primitive.binding == binding);
        let primitive = matches
            .next()
            .ok_or(SceneValidationError::InvalidPrimitiveBinding)?;
        if matches.next().is_some() || primitive.node != node.id {
            return Err(SceneValidationError::InvalidPrimitiveBinding);
        }
    }
    Ok(())
}

fn material_primitive_compatible(material: MaterialKind, primitive: PrimitiveKind) -> bool {
    match material {
        MaterialKind::UnlitGlyphSprite => {
            matches!(
                primitive,
                PrimitiveKind::AtlasQuad | PrimitiveKind::InstanceQuad
            )
        }
        MaterialKind::UnlitAnalytic => primitive == PrimitiveKind::AnalyticShape,
        MaterialKind::LitShallowCard => primitive == PrimitiveKind::ShallowCard,
        MaterialKind::MultiplyShadow | MaterialKind::AdditiveGlow | MaterialKind::ScreenChrome => {
            primitive != PrimitiveKind::ShallowCard
        }
    }
}

fn material_depth_compatible(material: MaterialKind, primitive: &PrimitiveTemplate) -> bool {
    let depth_matches_blend = match primitive.blend {
        WorldBlend::Opaque | WorldBlend::AlphaCutout => {
            primitive.depth == DepthBehavior::WorldWrite
        }
        WorldBlend::PremultipliedAlpha | WorldBlend::Multiply | WorldBlend::Additive => {
            primitive.depth == DepthBehavior::WorldReadOnly
        }
    };
    match material {
        MaterialKind::ScreenChrome => {
            primitive.blend == WorldBlend::PremultipliedAlpha
                && primitive.depth == DepthBehavior::ScreenNoDepth
        }
        MaterialKind::LitShallowCard => {
            primitive.kind == PrimitiveKind::ShallowCard
                && matches!(
                    primitive.blend,
                    WorldBlend::Opaque | WorldBlend::AlphaCutout
                )
                && depth_matches_blend
        }
        MaterialKind::MultiplyShadow => {
            primitive.blend == WorldBlend::Multiply && depth_matches_blend
        }
        MaterialKind::AdditiveGlow => {
            primitive.blend == WorldBlend::Additive && depth_matches_blend
        }
        MaterialKind::UnlitGlyphSprite | MaterialKind::UnlitAnalytic => depth_matches_blend,
    }
}

fn primitive_resource_compatible(kind: PrimitiveKind, resource: Option<ResourceKind>) -> bool {
    matches!(
        (kind, resource),
        (
            PrimitiveKind::AtlasQuad,
            Some(ResourceKind::GlyphAtlas | ResourceKind::ColorAtlas)
        ) | (
            PrimitiveKind::AnalyticShape,
            Some(ResourceKind::AnalyticGeometry)
        ) | (
            PrimitiveKind::ShallowCard,
            Some(ResourceKind::ShallowCardGeometry)
        ) | (
            PrimitiveKind::InstanceQuad,
            Some(ResourceKind::GlyphAtlas | ResourceKind::ColorAtlas)
        )
    )
}

fn collect_lit_card_paths(
    template: &SceneTemplate,
    node_dense_indices: &HashMap<NodeId, usize>,
) -> Result<Vec<Vec<LitPathNode>>, SceneValidationError> {
    let nodes = template
        .nodes
        .iter()
        .map(|node| (node.id, node))
        .collect::<HashMap<_, _>>();
    let materials = template
        .materials
        .iter()
        .map(|material| (material.id, material.kind))
        .collect::<HashMap<_, _>>();
    let mut lit_nodes = HashSet::new();
    let mut paths = Vec::new();
    for primitive in &template.primitives {
        if materials.get(&primitive.material) != Some(&MaterialKind::LitShallowCard)
            || !lit_nodes.insert(primitive.node)
        {
            continue;
        }
        let mut current = Some(primitive.node);
        let mut path = Vec::new();
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(SceneValidationError::HierarchyCycle);
            }
            let node = nodes
                .get(&id)
                .ok_or(SceneValidationError::DanglingNodeReference)?;
            path.push(LitPathNode {
                id,
                dense_index: *node_dense_indices
                    .get(&id)
                    .ok_or(SceneValidationError::DanglingNodeReference)?,
                base_transform: node.base_transform,
            });
            current = node.parent;
        }
        path.reverse();
        paths.push(path);
    }
    Ok(paths)
}

fn validate_lit_card_world_transforms(
    paths: &[Vec<LitPathNode>],
    dynamic_transform: impl Fn(NodeId) -> Option<Transform3>,
) -> Result<(), SceneValidationError> {
    for path in paths {
        let mut world = Mat4::IDENTITY;
        for node in path {
            world = world
                * node
                    .base_transform
                    .matrix()
                    .map_err(transform_validation_error)?;
            if let Some(dynamic) = dynamic_transform(node.id) {
                world = world * dynamic.matrix().map_err(transform_validation_error)?;
            }
        }
        validate_lit_card_world_linear(world)?;
    }
    Ok(())
}

fn validate_lit_card_path_overlay(
    path: &[LitPathNode],
    current: &SceneFrame,
    overlay: &[Option<NodeFrameState>; MAX_SCENE_NODES],
) -> Result<(), SceneValidationError> {
    let mut world = Mat4::IDENTITY;
    for node in path {
        world = world
            * node
                .base_transform
                .matrix()
                .map_err(transform_validation_error)?;
        let dynamic = overlay[node.dense_index]
            .unwrap_or(current.nodes[node.dense_index])
            .local_transform;
        world = world * dynamic.matrix().map_err(transform_validation_error)?;
    }
    validate_lit_card_world_linear(world)
}

fn transform_validation_error(error: TransformError) -> SceneValidationError {
    match error {
        TransformError::NonFinite => SceneValidationError::NonFiniteTransform,
        TransformError::ZeroQuaternion => SceneValidationError::ZeroQuaternion,
    }
}

fn validate_lit_card_world_linear(matrix: Mat4) -> Result<(), SceneValidationError> {
    let columns =
        [0, 1, 2].map(|column| [0, 1, 2].map(|row| f64::from(matrix.columns[column][row])));
    let norms = columns.map(|column| column.iter().map(|value| value * value).sum::<f64>().sqrt());
    if norms
        .iter()
        .any(|norm| !norm.is_finite() || *norm < MIN_LIT_CARD_WORLD_SCALE)
    {
        return Err(SceneValidationError::LitCardScaleIncompatible);
    }
    let max_norm = norms.into_iter().fold(0.0_f64, f64::max);
    if norms
        .iter()
        .any(|norm| (norm - norms[0]).abs() > f64::from(LIT_CARD_SCALE_TOLERANCE) * max_norm)
    {
        return Err(SceneValidationError::LitCardScaleIncompatible);
    }
    for (left, right) in [(0, 1), (0, 2), (1, 2)] {
        let dot = columns[left]
            .iter()
            .zip(columns[right])
            .map(|(a, b)| a * b)
            .sum::<f64>();
        if dot.abs() > f64::from(LIT_CARD_SCALE_TOLERANCE) * norms[left] * norms[right] {
            return Err(SceneValidationError::LitCardScaleIncompatible);
        }
    }
    let determinant = columns[0][0]
        * (columns[1][1] * columns[2][2] - columns[1][2] * columns[2][1])
        - columns[1][0] * (columns[0][1] * columns[2][2] - columns[0][2] * columns[2][1])
        + columns[2][0] * (columns[0][1] * columns[1][2] - columns[0][2] * columns[1][1]);
    if !determinant.is_finite() || determinant <= 0.0 {
        return Err(SceneValidationError::LitCardScaleIncompatible);
    }
    Ok(())
}

fn validate_privacy(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    let privacy = template.privacy;
    let external_surface = matches!(
        privacy.surface,
        PresentationSurface::RoundCompanion
            | PresentationSurface::RoundPreviewLab
            | PresentationSurface::PreviewLabArtifact
    );
    let sanitized = !privacy.source_names_visible
        && !privacy.exact_counts_visible
        && !privacy.diagnostic_text_visible
        && !privacy.feed_rows_visible
        && !privacy.file_paths_visible
        && !privacy.project_names_visible;
    if !external_surface || !sanitized {
        return Err(SceneValidationError::PrivacyViolation);
    }
    Ok(())
}

fn validate_content_slots(
    pet: &[PetArtSlot],
    room: &[RoomGlyphContentSlot],
    props: &[PropContentSlot],
    tanks: &[TankContentSlot],
    ambient: &[AmbientContentSlot],
) -> Result<(), SceneValidationError> {
    if pet
        .iter()
        .any(|slot| slot.glyph.is_none() && slot.palette_role != PetPaletteRole::Body)
        || props.iter().any(|slot| {
            slot.content.is_some_and(|content| {
                content
                    .glyphs
                    .iter()
                    .any(|glyph| glyph.glyph.is_none() && glyph.local_cell != [0; 2])
            })
        })
        || room
            .iter()
            .any(|slot| slot.glyph.is_some() != slot.color_srgb8.is_some())
    {
        return Err(SceneValidationError::NonCanonicalEmptySlot);
    }
    if ambient
        .iter()
        .any(|slot| slot.kind.is_some() != slot.glyph.is_some())
    {
        return Err(SceneValidationError::InvalidContentValue);
    }
    validate_unique_slots(
        pet.iter().map(|slot| usize::from(slot.slot)),
        MAX_PET_ART_SLOTS,
        SceneValidationError::PetArtSlotOutOfBounds,
    )?;
    validate_unique_slots(
        room.iter().map(|slot| usize::from(slot.slot)),
        MAX_ROOM_GLYPH_SLOTS,
        SceneValidationError::RoomGlyphSlotOutOfBounds,
    )?;
    validate_unique_slots(
        props.iter().map(|slot| usize::from(slot.slot)),
        MAX_VISIBLE_PROPS,
        SceneValidationError::PropSlotOutOfBounds,
    )?;
    validate_unique_slots(
        tanks.iter().map(|slot| usize::from(slot.slot)),
        MAX_ROUND_TANK_INHABITANTS,
        SceneValidationError::TankSlotOutOfBounds,
    )?;
    validate_unique_slots(
        ambient.iter().map(|slot| usize::from(slot.slot)),
        MAX_AMBIENT_INSTANCES,
        SceneValidationError::AmbientSlotOutOfBounds,
    )
}

fn validate_analytic_content_slots(
    slots: &[AnalyticContentSlot],
    full_table: bool,
) -> Result<(), SceneValidationError> {
    if full_table && slots.len() != MAX_ANALYTIC_PARAMS {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    validate_unique_slots(
        slots.iter().map(|slot| usize::from(slot.id.0)),
        MAX_ANALYTIC_PARAMS,
        SceneValidationError::InvalidContentValue,
    )?;
    for slot in slots {
        let index = usize::from(slot.id.0);
        match (index, slot.value) {
            (0..=7, Some(value))
                if value.semantic == AnalyticSemantic::ALL[index]
                    && value.shape == value.semantic.shape() =>
            {
                validate_analytic_paint(value.semantic, value.paint)?;
            }
            (8.., None) => {}
            _ => return Err(SceneValidationError::InvalidContentValue),
        }
    }
    if full_table
        && slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.id.0) != index)
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    Ok(())
}

fn validate_analytic_paint(
    semantic: AnalyticSemantic,
    paint: AnalyticPaint,
) -> Result<(), SceneValidationError> {
    let rgba = |color: [u8; 4]| color[3] > 0;
    let valid = match (semantic, paint) {
        (
            AnalyticSemantic::RoomBackground,
            AnalyticPaint::ApertureDepth { core_srgb8, rim_srgb8 },
        ) => {
            let _ = (core_srgb8, rim_srgb8);
            true
        }
        (
            AnalyticSemantic::WallShadow,
            AnalyticPaint::PetShadowMultiply { color_srgb8, opacity_u8 },
        ) => {
            let _ = color_srgb8;
            opacity_u8 > 0
        }
        (
            AnalyticSemantic::FloorProjection,
            AnalyticPaint::FloorShadowMultiplyRadial { inner_srgba8, outer_srgba8 },
        ) => rgba(inner_srgba8) && outer_srgba8[3] == 0,
        (
            AnalyticSemantic::StatusHalo,
            AnalyticPaint::StatusBeacon { active_srgba8, calm_srgba8 },
        ) => rgba(active_srgba8) && rgba(calm_srgba8),
        (
            AnalyticSemantic::MoodAura,
            AnalyticPaint::MoodAuraRings {
                color_srgb8,
                ring_count: 8,
                per_ring_alpha_u8,
            },
        ) => {
            let _ = color_srgb8;
            per_ring_alpha_u8 > 0
        }
        (
            AnalyticSemantic::Gauges,
            AnalyticPaint::PerimeterGaugeSet { xp, daily, pace, daily_overage_srgba8 },
        ) => {
            [xp, daily, pace]
                .into_iter()
                .all(|lane| rgba(lane.track_srgba8) && rgba(lane.fill_srgba8))
                && rgba(daily_overage_srgba8)
        }
        (AnalyticSemantic::Trouble, AnalyticPaint::TroubleBeacon { color_srgba8 }) => {
            rgba(color_srgba8)
        }
        (AnalyticSemantic::Dim, AnalyticPaint::DimOverlay { color_srgb8 }) => {
            let _ = color_srgb8;
            true
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(SceneValidationError::InvalidContentValue)
}

fn validate_paint_slots(
    props: &[PropContentSlot],
    prop_paints: &[PropGlyphPaintSlot],
    ambient: &[AmbientContentSlot],
    ambient_paints: &[AmbientGlyphPaintSlot],
    full_table: bool,
) -> Result<(), SceneValidationError> {
    if full_table
        && (prop_paints.len() != MAX_VISIBLE_PROPS || ambient_paints.len() != MAX_AMBIENT_INSTANCES)
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    validate_unique_slots(
        prop_paints.iter().map(|slot| usize::from(slot.slot)),
        MAX_VISIBLE_PROPS,
        SceneValidationError::PropSlotOutOfBounds,
    )?;
    validate_unique_slots(
        ambient_paints.iter().map(|slot| usize::from(slot.slot)),
        MAX_AMBIENT_INSTANCES,
        SceneValidationError::AmbientSlotOutOfBounds,
    )?;
    for paint in prop_paints {
        if let Some(content) = props
            .iter()
            .find(|content| content.slot == paint.slot)
            .and_then(|content| content.content)
        {
            if content
                .glyphs
                .into_iter()
                .zip(paint.paints)
                .any(|(glyph, paint)| glyph.glyph.is_some() != paint.is_some())
            {
                return Err(SceneValidationError::NonCanonicalEmptySlot);
            }
        } else if full_table && paint.paints.into_iter().any(|paint| paint.is_some()) {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    for paint in ambient_paints {
        if let Some(content) = ambient.iter().find(|content| content.slot == paint.slot) {
            if content.kind.is_some() != paint.paint.is_some() {
                return Err(SceneValidationError::NonCanonicalEmptySlot);
            }
        } else if full_table && paint.paint.is_some() {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    Ok(())
}

fn validate_delta_paint_overlays(
    current: &SceneContent,
    delta: &ContentDelta,
) -> Result<(), SceneValidationError> {
    for slot in 0..MAX_VISIBLE_PROPS {
        let content = delta
            .prop_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current.prop_slots[slot]);
        let paints = delta
            .prop_paint_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current.prop_paint_slots[slot]);
        let glyphs = content.content.map(|content| content.glyphs).unwrap_or(
            [PropGlyphContent { glyph: None, local_cell: [0; 2] }; MAX_PROP_GLYPHS_PER_SLOT],
        );
        if glyphs
            .into_iter()
            .zip(paints.paints)
            .any(|(glyph, paint)| glyph.glyph.is_some() != paint.is_some())
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    for slot in 0..MAX_AMBIENT_INSTANCES {
        let content = delta
            .ambient_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current.ambient_slots[slot]);
        let paint = delta
            .ambient_paint_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current.ambient_paint_slots[slot]);
        if content.kind.is_some() != paint.paint.is_some() {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    Ok(())
}

fn validate_analytic_frame_slots(
    slots: &[AnalyticFrameSlot],
    full_table: bool,
    camera: OrthographicCamera,
) -> Result<(), SceneValidationError> {
    if full_table && slots.len() != MAX_ANALYTIC_PARAMS {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    validate_unique_slots(
        slots.iter().map(|slot| usize::from(slot.id.0)),
        MAX_ANALYTIC_PARAMS,
        SceneValidationError::InvalidFrameValue,
    )?;
    for slot in slots {
        let index = usize::from(slot.id.0);
        match (index, slot.value) {
            (0..=7, Some(value))
                if value.semantic == AnalyticSemantic::ALL[index]
                    && value.shape == value.semantic.shape() =>
            {
                validate_analytic_frame(value, camera)?;
            }
            (8.., None) => {}
            _ => return Err(SceneValidationError::InvalidFrameValue),
        }
    }
    if full_table
        && slots
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.id.0) != index)
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    Ok(())
}

fn validate_analytic_frame(
    value: AnalyticFrame,
    camera: OrthographicCamera,
) -> Result<(), SceneValidationError> {
    if !value.rect_points.iter().all(|value| value.is_finite()) {
        return Err(SceneValidationError::NonFiniteFrameValue);
    }
    if value.rect_points[2] <= 0.0 || value.rect_points[3] <= 0.0 {
        return Err(SceneValidationError::InvalidFrameValue);
    }
    let spatial_limit = camera.width_points.max(camera.height_points) * 4.0 + 64.0;
    if value
        .rect_points
        .into_iter()
        .any(|component| component.abs() > spatial_limit)
    {
        return Err(SceneValidationError::InvalidFrameValue);
    }
    let finite2 = |values: [f32; 2]| values.into_iter().all(f32::is_finite);
    let positive2 =
        |values: [f32; 2]| finite2(values) && values.into_iter().all(|value| value > 0.0);
    let geometry_finite = match value.geometry {
        AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points,
        }
        | AnalyticGeometry::StatusBeacon {
            center_points,
            radius_points,
            thickness_points: feather_points,
            ..
        }
        | AnalyticGeometry::TroubleBeacon {
            center_points,
            radius_points,
            thickness_points: feather_points,
        } => finite2(center_points) && radius_points.is_finite() && feather_points.is_finite(),
        AnalyticGeometry::PetSilhouette { offset_points, softness_points, .. } => {
            finite2(offset_points) && softness_points.is_finite()
        }
        AnalyticGeometry::RadialEllipse {
            center_points,
            radii_points,
            softness_points,
        } => finite2(center_points) && finite2(radii_points) && softness_points.is_finite(),
        AnalyticGeometry::PetAura {
            center_points,
            max_radius_points,
            feather_points,
            ..
        } => finite2(center_points) && max_radius_points.is_finite() && feather_points.is_finite(),
        AnalyticGeometry::PerimeterGaugeSet { center_points, xp, daily, pace } => {
            finite2(center_points)
                && [xp, daily, pace].into_iter().all(|lane| {
                    lane.radius_points.is_finite()
                        && lane.stroke_width_points.is_finite()
                        && lane.track_start_degrees.is_finite()
                        && lane.track_sweep_degrees.is_finite()
                })
        }
        AnalyticGeometry::SurfaceOverlay => true,
    };
    if !geometry_finite {
        return Err(SceneValidationError::NonFiniteFrameValue);
    }
    let bounded2 = |values: [f32; 2]| {
        values
            .into_iter()
            .all(|component| component.abs() <= spatial_limit)
    };
    let geometry_bounded = match value.geometry {
        AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points,
        }
        | AnalyticGeometry::StatusBeacon {
            center_points,
            radius_points,
            thickness_points: feather_points,
            ..
        }
        | AnalyticGeometry::TroubleBeacon {
            center_points,
            radius_points,
            thickness_points: feather_points,
        } => {
            bounded2(center_points)
                && radius_points.abs() <= spatial_limit
                && feather_points.abs() <= spatial_limit
        }
        AnalyticGeometry::PetSilhouette { offset_points, softness_points, .. } => {
            bounded2(offset_points) && softness_points.abs() <= spatial_limit
        }
        AnalyticGeometry::RadialEllipse {
            center_points,
            radii_points,
            softness_points,
        } => {
            bounded2(center_points)
                && bounded2(radii_points)
                && softness_points.abs() <= spatial_limit
        }
        AnalyticGeometry::PetAura {
            center_points,
            max_radius_points,
            feather_points,
            ..
        } => {
            bounded2(center_points)
                && max_radius_points.abs() <= spatial_limit
                && feather_points.abs() <= spatial_limit
        }
        AnalyticGeometry::PerimeterGaugeSet { center_points, xp, daily, pace } => {
            bounded2(center_points)
                && [xp, daily, pace].into_iter().all(|lane| {
                    lane.radius_points.abs() <= spatial_limit
                        && lane.stroke_width_points.abs() <= spatial_limit
                        && lane.track_start_degrees.abs() <= 360.0
                        && lane.track_sweep_degrees.abs() <= 360.0
                })
        }
        AnalyticGeometry::SurfaceOverlay => true,
    };
    if !geometry_bounded {
        return Err(SceneValidationError::InvalidFrameValue);
    }
    let full_camera_rect = [0.0, 0.0, camera.width_points, camera.height_points];
    let centered_rect = |center: [f32; 2], radii: [f32; 2]| {
        [
            center[0] - radii[0],
            center[1] - radii[1],
            radii[0] * 2.0,
            radii[1] * 2.0,
        ]
    };
    let valid = match (value.semantic, value.geometry) {
        (
            AnalyticSemantic::RoomBackground,
            AnalyticGeometry::ApertureRadial {
                center_points,
                radius_points,
                feather_points,
            },
        ) => {
            finite2(center_points)
                && value.rect_points == full_camera_rect
                && center_points
                    == [
                        (camera.width_points - 1.0) * 0.5,
                        (camera.height_points - 1.0) * 0.5,
                    ]
                && radius_points.is_finite()
                && radius_points > 0.0
                && radius_points == camera.width_points.min(camera.height_points) * 0.5 - 1.0
                && feather_points.is_finite()
                && feather_points >= 0.0
        }
        (
            AnalyticSemantic::WallShadow,
            AnalyticGeometry::PetSilhouette {
                mask: AnalyticMaskSource::PetBody,
                offset_points,
                softness_points,
            },
        ) => {
            finite2(offset_points)
                && softness_points.is_finite()
                && softness_points >= 0.0
                && value.rect_points[2] > offset_points[0].abs() + softness_points * 2.0
                && value.rect_points[3] > offset_points[1].abs() + softness_points * 2.0
        }
        (
            AnalyticSemantic::FloorProjection,
            AnalyticGeometry::RadialEllipse {
                center_points,
                radii_points,
                softness_points,
            },
        ) => {
            finite2(center_points)
                && positive2(radii_points)
                && value.rect_points == centered_rect(center_points, radii_points)
                && softness_points.is_finite()
                && softness_points >= 0.0
        }
        (
            AnalyticSemantic::StatusHalo,
            AnalyticGeometry::StatusBeacon {
                center_points,
                radius_points,
                thickness_points,
                ..
            },
        )
        | (
            AnalyticSemantic::Trouble,
            AnalyticGeometry::TroubleBeacon {
                center_points,
                radius_points,
                thickness_points,
            },
        ) => {
            finite2(center_points)
                && radius_points.is_finite()
                && radius_points > 0.0
                && thickness_points.is_finite()
                && thickness_points > 0.0
                && value.rect_points
                    == centered_rect(center_points, [radius_points + thickness_points; 2])
        }
        (
            AnalyticSemantic::MoodAura,
            AnalyticGeometry::PetAura {
                center_points,
                max_radius_points,
                ring_count: 8,
                feather_points,
            },
        ) => {
            finite2(center_points)
                && max_radius_points.is_finite()
                && max_radius_points > 0.0
                && value.rect_points == centered_rect(center_points, [max_radius_points; 2])
                && feather_points.is_finite()
                && feather_points >= 0.0
        }
        (
            AnalyticSemantic::Gauges,
            AnalyticGeometry::PerimeterGaugeSet { center_points, xp, daily, pace },
        ) => {
            let expected_center = [
                value.rect_points[0] + (value.rect_points[2] - 1.0) * 0.5,
                value.rect_points[1] + (value.rect_points[3] - 1.0) * 0.5,
            ];
            let aperture_radius = value.rect_points[2].min(value.rect_points[3]) * 0.5 - 1.0;
            let expected = crate::presentation::companion_effects::perimeter_gauge_layout(
                f64::from(aperture_radius),
                crate::presentation::companion_effects::COMPANION_GAUGE_GAP_DEGREES,
            );
            let lane_matches = |actual: GaugeLaneGeometry,
                                expected: crate::presentation::companion_effects::GaugeLaneLayout| {
                actual.radius_points > 0.0
                    && actual.stroke_width_points > 0.0
                    && actual.stroke_width_points < actual.radius_points
                    && actual.radius_points == expected.radius as f32
                    && actual.stroke_width_points == expected.stroke_width as f32
                    && actual.track_start_degrees == expected.track_start_degrees as f32
                    && actual.track_sweep_degrees == expected.track_sweep_degrees as f32
                    && actual.cap == GaugeLineCap::Round
            };
            finite2(center_points)
                && value.rect_points == full_camera_rect
                && center_points == expected_center
                && aperture_radius > 0.0
                && lane_matches(xp, expected.xp)
                && lane_matches(daily, expected.daily)
                && lane_matches(pace, expected.pace)
        }
        (AnalyticSemantic::Dim, AnalyticGeometry::SurfaceOverlay) => {
            value.rect_points == full_camera_rect
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(SceneValidationError::InvalidFrameValue)
}

fn validate_instance_frame_slots(frame: &SceneFrame) -> Result<(), SceneValidationError> {
    validate_unique_slots(
        frame
            .room_glyph_slots
            .iter()
            .map(|slot| usize::from(slot.slot)),
        MAX_ROOM_GLYPH_SLOTS,
        SceneValidationError::RoomGlyphSlotOutOfBounds,
    )?;
    validate_unique_slots(
        frame.prop_slots.iter().map(|slot| usize::from(slot.slot)),
        MAX_VISIBLE_PROPS,
        SceneValidationError::PropSlotOutOfBounds,
    )?;
    validate_unique_slots(
        frame.tank_slots.iter().map(|slot| usize::from(slot.slot)),
        MAX_ROUND_TANK_INHABITANTS,
        SceneValidationError::TankSlotOutOfBounds,
    )?;
    validate_unique_slots(
        frame
            .ambient_slots
            .iter()
            .map(|slot| usize::from(slot.slot)),
        MAX_AMBIENT_INSTANCES,
        SceneValidationError::AmbientSlotOutOfBounds,
    )?;
    for slot in &frame.prop_slots {
        validate_prop_frame_slot(*slot)?;
    }
    for slot in &frame.room_glyph_slots {
        validate_points(slot.position_points)?;
        validate_unit_interval(slot.opacity)?;
    }
    for slot in &frame.tank_slots {
        validate_points(slot.origin_points)?;
        for cell in slot.cells {
            validate_points(cell.position_points)?;
            if !cell.bounds_points.iter().all(|value| value.is_finite()) {
                return Err(SceneValidationError::NonFiniteFrameValue);
            }
        }
    }
    for slot in &frame.ambient_slots {
        validate_points(slot.position_points)?;
        validate_unit_interval(slot.opacity)?;
    }
    Ok(())
}

fn validate_content_frame_canonical(
    content: &SceneContent,
    frame: &SceneFrame,
) -> Result<(), SceneValidationError> {
    let zero2 = |value: [f32; 2]| value.into_iter().all(|component| component.to_bits() == 0);
    let zero4 = |value: [f32; 4]| value.into_iter().all(|component| component.to_bits() == 0);
    for (content, frame) in content.room_glyph_slots.iter().zip(&frame.room_glyph_slots) {
        if content.glyph.is_some() != frame.visible
            || (content.glyph.is_some() && frame.opacity <= 0.0)
            || (content.glyph.is_none()
                && (frame.grid_cell != [0; 2]
                    || !zero2(frame.position_points)
                    || frame.opacity.to_bits() != 0))
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    for (content, frame) in content.prop_slots.iter().zip(&frame.prop_slots) {
        if content.content.is_none()
            && (frame.visible
                || !zero2(frame.origin_points)
                || !zero2(frame.motion_offset_points)
                || frame.opacity.to_bits() != 0
                || !zero2(frame.footprint_points)
                || frame.contact_shadow_strength.to_bits() != 0)
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    for (content, frame) in content.tank_slots.iter().zip(&frame.tank_slots) {
        if content.content.is_none() && (frame.visible || !zero2(frame.origin_points)) {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
        let glyphs = content
            .content
            .map(|content| content.glyphs)
            .unwrap_or([None; MAX_TANK_GLYPHS_PER_SLOT]);
        for (glyph, cell) in glyphs.into_iter().zip(frame.cells) {
            if glyph.is_none()
                && (cell.visible
                    || !zero2(cell.position_points)
                    || !zero4(cell.bounds_points)
                    || cell.layer != InstanceLayer::Behind)
            {
                return Err(SceneValidationError::NonCanonicalEmptySlot);
            }
        }
    }
    for (content, frame) in content.ambient_slots.iter().zip(&frame.ambient_slots) {
        if content.kind.is_none()
            && (frame.visible || !zero2(frame.position_points) || frame.opacity.to_bits() != 0)
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    for (content, frame) in content.analytic_slots.iter().zip(&frame.analytic_slots) {
        if content.value.map(|value| (value.semantic, value.shape))
            != frame.value.map(|value| (value.semantic, value.shape))
        {
            return Err(SceneValidationError::InvalidFrameValue);
        }
    }
    Ok(())
}

fn validate_changed_instance_frame_slots(delta: &FrameDelta) -> Result<(), SceneValidationError> {
    validate_unique_slots(
        delta
            .room_glyph_slots
            .iter()
            .map(|slot| usize::from(slot.slot)),
        MAX_ROOM_GLYPH_SLOTS,
        SceneValidationError::RoomGlyphSlotOutOfBounds,
    )?;
    validate_unique_slots(
        delta.prop_slots.iter().map(|slot| usize::from(slot.slot)),
        MAX_VISIBLE_PROPS,
        SceneValidationError::PropSlotOutOfBounds,
    )?;
    validate_unique_slots(
        delta.tank_slots.iter().map(|slot| usize::from(slot.slot)),
        MAX_ROUND_TANK_INHABITANTS,
        SceneValidationError::TankSlotOutOfBounds,
    )?;
    validate_unique_slots(
        delta
            .ambient_slots
            .iter()
            .map(|slot| usize::from(slot.slot)),
        MAX_AMBIENT_INSTANCES,
        SceneValidationError::AmbientSlotOutOfBounds,
    )?;
    for slot in &delta.prop_slots {
        validate_prop_frame_slot(*slot)?;
    }
    for slot in &delta.room_glyph_slots {
        validate_points(slot.position_points)?;
        validate_unit_interval(slot.opacity)?;
    }
    for slot in &delta.tank_slots {
        validate_points(slot.origin_points)?;
        for cell in slot.cells {
            validate_points(cell.position_points)?;
            if !cell.bounds_points.iter().all(|value| value.is_finite()) {
                return Err(SceneValidationError::NonFiniteFrameValue);
            }
        }
    }
    for slot in &delta.ambient_slots {
        validate_points(slot.position_points)?;
        validate_unit_interval(slot.opacity)?;
    }
    Ok(())
}

fn validate_points(points: [f32; 2]) -> Result<(), SceneValidationError> {
    points
        .iter()
        .all(|value| value.is_finite())
        .then_some(())
        .ok_or(SceneValidationError::NonFiniteFrameValue)
}

fn validate_prop_frame_slot(slot: PropFrameSlot) -> Result<(), SceneValidationError> {
    validate_points(slot.origin_points)?;
    validate_points(slot.motion_offset_points)?;
    validate_points(slot.footprint_points)?;
    validate_unit_interval(slot.opacity)?;
    if slot.footprint_points.into_iter().any(|extent| extent < 0.0)
        || !slot.contact_shadow_strength.is_finite()
        || !(0.0..=1.0).contains(&slot.contact_shadow_strength)
        || (!slot.visible && slot.contact_shadow_strength != 0.0)
    {
        return if slot.contact_shadow_strength.is_finite() {
            Err(SceneValidationError::InvalidFrameValue)
        } else {
            Err(SceneValidationError::NonFiniteFrameValue)
        };
    }
    Ok(())
}

fn validate_unique_slots(
    slots: impl Iterator<Item = usize>,
    capacity: usize,
    out_of_bounds: SceneValidationError,
) -> Result<(), SceneValidationError> {
    debug_assert!(capacity <= MAX_PET_ART_SLOTS);
    let mut seen = [false; MAX_PET_ART_SLOTS];
    for slot in slots {
        if slot >= capacity {
            return Err(out_of_bounds);
        }
        if std::mem::replace(&mut seen[slot], true) {
            return Err(SceneValidationError::DuplicateSlot);
        }
    }
    Ok(())
}

fn validate_transform(transform: Transform3) -> Result<(), SceneValidationError> {
    match transform.matrix() {
        Ok(matrix)
            if matrix
                .columns
                .iter()
                .flatten()
                .all(|value| value.is_finite()) =>
        {
            Ok(())
        }
        Ok(_) | Err(TransformError::NonFinite) => Err(SceneValidationError::NonFiniteTransform),
        Err(TransformError::ZeroQuaternion) => Err(SceneValidationError::ZeroQuaternion),
    }
}

fn validate_bounds(bounds: Bounds3) -> Result<(), SceneValidationError> {
    if !bounds
        .min
        .iter()
        .chain(bounds.max.iter())
        .all(|value| value.is_finite())
    {
        return Err(SceneValidationError::NonFiniteBounds);
    }
    if bounds
        .min
        .into_iter()
        .zip(bounds.max)
        .any(|(min, max)| min > max)
    {
        return Err(SceneValidationError::InvalidBounds);
    }
    Ok(())
}

fn validate_depth_cue(cue: DepthCue) -> Result<(), SceneValidationError> {
    if ![
        cue.scale,
        cue.y_offset_points_up,
        cue.opacity,
        cue.saturation,
    ]
    .iter()
    .all(|value| value.is_finite())
    {
        return Err(SceneValidationError::NonFiniteDepthCue);
    }
    if cue.scale <= 0.0 || !(0.0..=1.0).contains(&cue.opacity) || cue.saturation < 0.0 {
        return Err(SceneValidationError::InvalidDepthCue);
    }
    Ok(())
}

fn validate_camera(camera: OrthographicCamera) -> Result<(), SceneValidationError> {
    OrthographicCamera::new(
        camera.width_points,
        camera.height_points,
        camera.far_z,
        camera.near_z,
    )
    .and_then(|camera| camera.projection_matrix().map(|_| ()))
    .map_err(|_| SceneValidationError::InvalidCamera)
}

fn validate_frame_scalars(
    gauges: [f32; 4],
    dim_amount: f32,
    lights: &[LightFrame],
) -> Result<(), SceneValidationError> {
    for gauge in gauges {
        validate_unit_interval(gauge)?;
    }
    validate_unit_interval(dim_amount)?;
    for light in lights {
        validate_light(*light)?;
    }
    Ok(())
}

fn validate_light(light: LightFrame) -> Result<(), SceneValidationError> {
    if !light
        .direction
        .iter()
        .chain(light.color_linear.iter())
        .chain(std::iter::once(&light.intensity))
        .all(|value| value.is_finite())
    {
        return Err(SceneValidationError::NonFiniteFrameValue);
    }
    let direction_norm = light
        .direction
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum::<f64>()
        .sqrt();
    if !(MIN_LIGHT_DIRECTION_NORM..=MAX_LIGHT_DIRECTION_NORM).contains(&direction_norm)
        || light
            .color_linear
            .iter()
            .any(|value| !(0.0..=MAX_LIGHT_COLOR_LINEAR).contains(value))
        || !(0.0..=MAX_LIGHT_INTENSITY).contains(&light.intensity)
        || light.color_linear.iter().any(|value| {
            f64::from(*value) * f64::from(light.intensity) > MAX_LIGHT_COLOR_INTENSITY_PRODUCT
        })
    {
        return Err(SceneValidationError::InvalidFrameValue);
    }
    Ok(())
}

fn validate_unit_interval(value: f32) -> Result<(), SceneValidationError> {
    if !value.is_finite() {
        return Err(SceneValidationError::NonFiniteFrameValue);
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(SceneValidationError::InvalidFrameValue);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytic_and_glyph_paint_contracts_reject_mismatches_and_nonfinite_geometry() {
        let mut fixture = SceneFixture::valid();
        fixture.content.analytic_slots[0]
            .value
            .as_mut()
            .unwrap()
            .paint = AnalyticPaint::DimOverlay { color_srgb8: [0; 3] };
        assert_eq!(
            validate_content(&fixture.content),
            Err(SceneValidationError::InvalidContentValue)
        );

        let mut fixture = SceneFixture::valid();
        let room = fixture.frame.analytic_slots[0].value.as_mut().unwrap();
        room.geometry = AnalyticGeometry::ApertureRadial {
            center_points: [f32::NAN, 180.0],
            radius_points: 179.0,
            feather_points: 1.0,
        };
        assert_eq!(
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame)
                .map(|_| ()),
            Err(SceneValidationError::NonFiniteFrameValue)
        );

        let mut fixture = SceneFixture::valid();
        fixture.content.prop_slots[0].content = Some(PropSemanticContent {
            sprite_phase: None,
            twinkle_active: None,
            lid_open: None,
            bloom_active: None,
            glyphs: std::array::from_fn(|index| PropGlyphContent {
                glyph: (index == 0).then(|| AuthoredGlyph::new('◆').unwrap()),
                local_cell: [0; 2],
            }),
        });
        assert_eq!(
            validate_content(&fixture.content),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );
    }

    #[test]
    fn analytic_geometry_rejects_finite_extremes_in_full_and_atomic_delta_paths() {
        type Mutation = fn(&mut AnalyticFrame, f32);
        let mutations: &[(usize, Mutation)] = &[
            (0, |value, extreme| value.rect_points[0] = extreme),
            (0, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::ApertureRadial { center_points, .. } => {
                    center_points[0] = extreme
                }
                _ => unreachable!(),
            }),
            (0, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::ApertureRadial { radius_points, .. } => *radius_points = extreme,
                _ => unreachable!(),
            }),
            (0, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::ApertureRadial { feather_points, .. } => {
                    *feather_points = extreme
                }
                _ => unreachable!(),
            }),
            (1, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PetSilhouette { offset_points, .. } => offset_points[1] = extreme,
                _ => unreachable!(),
            }),
            (1, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PetSilhouette { softness_points, .. } => {
                    *softness_points = extreme
                }
                _ => unreachable!(),
            }),
            (2, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::RadialEllipse { center_points, .. } => center_points[1] = extreme,
                _ => unreachable!(),
            }),
            (2, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::RadialEllipse { radii_points, .. } => radii_points[0] = extreme,
                _ => unreachable!(),
            }),
            (2, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::RadialEllipse { softness_points, .. } => {
                    *softness_points = extreme
                }
                _ => unreachable!(),
            }),
            (3, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::StatusBeacon { center_points, .. } => center_points[0] = extreme,
                _ => unreachable!(),
            }),
            (3, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::StatusBeacon { radius_points, .. } => *radius_points = extreme,
                _ => unreachable!(),
            }),
            (3, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::StatusBeacon { thickness_points, .. } => {
                    *thickness_points = extreme
                }
                _ => unreachable!(),
            }),
            (4, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PetAura { center_points, .. } => center_points[1] = extreme,
                _ => unreachable!(),
            }),
            (4, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PetAura { max_radius_points, .. } => *max_radius_points = extreme,
                _ => unreachable!(),
            }),
            (4, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PetAura { feather_points, .. } => *feather_points = extreme,
                _ => unreachable!(),
            }),
            (5, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PerimeterGaugeSet { center_points, .. } => {
                    center_points[0] = extreme
                }
                _ => unreachable!(),
            }),
            (5, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PerimeterGaugeSet { xp, .. } => xp.radius_points = extreme,
                _ => unreachable!(),
            }),
            (5, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PerimeterGaugeSet { daily, .. } => {
                    daily.stroke_width_points = extreme
                }
                _ => unreachable!(),
            }),
            (5, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PerimeterGaugeSet { pace, .. } => {
                    pace.track_start_degrees = extreme
                }
                _ => unreachable!(),
            }),
            (5, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::PerimeterGaugeSet { pace, .. } => {
                    pace.track_sweep_degrees = extreme
                }
                _ => unreachable!(),
            }),
            (6, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::TroubleBeacon { center_points, .. } => center_points[0] = extreme,
                _ => unreachable!(),
            }),
            (6, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::TroubleBeacon { radius_points, .. } => *radius_points = extreme,
                _ => unreachable!(),
            }),
            (6, |value, extreme| match &mut value.geometry {
                AnalyticGeometry::TroubleBeacon { thickness_points, .. } => {
                    *thickness_points = extreme
                }
                _ => unreachable!(),
            }),
        ];

        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        for extreme in [f32::MAX, -f32::MAX] {
            for &(slot, mutate) in mutations {
                let mut frame = fixture.frame.clone();
                mutate(frame.analytic_slots[slot].value.as_mut().unwrap(), extreme);
                assert_eq!(
                    validate_full_generation(&fixture.template, &fixture.content, &frame)
                        .map(|_| ()),
                    Err(SceneValidationError::InvalidFrameValue),
                    "slot {slot} extreme {extreme}"
                );

                let mut current = validate_frame(&fixture.frame, &accepted).unwrap();
                let before = current.frame().clone();
                let mut delta = FrameDelta::empty();
                delta.analytic_slots.push(frame.analytic_slots[slot]);
                assert_eq!(
                    validate_frame_delta(&delta, &accepted, &mut current),
                    Err(SceneValidationError::InvalidFrameValue),
                    "delta slot {slot} extreme {extreme}"
                );
                assert_eq!(current.frame(), &before);
            }
        }
    }

    #[test]
    fn invalid_analytic_delta_is_atomic() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        let mut current = validate_frame(&fixture.frame, &accepted).unwrap();
        let before = current.frame().clone();
        let mut delta = FrameDelta::empty();
        let mut changed = fixture.frame.analytic_slots[5];
        let invalid_lane = GaugeLaneGeometry {
            radius_points: 170.0,
            stroke_width_points: 6.0,
            track_start_degrees: 305.0,
            track_sweep_degrees: 180.0,
            cap: GaugeLineCap::Round,
        };
        changed.value.as_mut().unwrap().geometry = AnalyticGeometry::PerimeterGaugeSet {
            center_points: [180.0; 2],
            xp: invalid_lane,
            daily: invalid_lane,
            pace: invalid_lane,
        };
        delta.analytic_slots.push(changed);
        assert_eq!(
            validate_frame_delta(&delta, &accepted, &mut current),
            Err(SceneValidationError::InvalidFrameValue)
        );
        assert_eq!(current.frame(), &before);
    }

    #[test]
    fn gauge_lanes_reject_collapsed_and_swapped_named_geometry() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        for swap in [false, true] {
            let mut changed = fixture.frame.analytic_slots[5];
            let value = changed.value.as_mut().unwrap();
            let AnalyticGeometry::PerimeterGaugeSet { xp, daily, pace, .. } = &mut value.geometry
            else {
                unreachable!()
            };
            if swap {
                std::mem::swap(xp, pace);
            } else {
                daily.radius_points = xp.radius_points;
            }

            let mut full = fixture.frame.clone();
            full.analytic_slots[5] = changed;
            assert_eq!(
                validate_full_generation(&fixture.template, &fixture.content, &full).map(|_| ()),
                Err(SceneValidationError::InvalidFrameValue)
            );

            let mut current = validate_frame(&fixture.frame, &accepted).unwrap();
            let before = current.frame().clone();
            let mut delta = FrameDelta::empty();
            delta.analytic_slots.push(changed);
            assert_eq!(
                validate_frame_delta(&delta, &accepted, &mut current),
                Err(SceneValidationError::InvalidFrameValue)
            );
            assert_eq!(current.frame(), &before);
        }

        let tiny_camera = OrthographicCamera::new(4.0, 4.0, -2.0, 2.0).unwrap();
        let tiny_layout = crate::presentation::companion_effects::perimeter_gauge_layout(
            1.0,
            crate::presentation::companion_effects::COMPANION_GAUGE_GAP_DEGREES,
        );
        let lane =
            |value: crate::presentation::companion_effects::GaugeLaneLayout| GaugeLaneGeometry {
                radius_points: value.radius as f32,
                stroke_width_points: value.stroke_width as f32,
                track_start_degrees: value.track_start_degrees as f32,
                track_sweep_degrees: value.track_sweep_degrees as f32,
                cap: GaugeLineCap::Round,
            };
        assert_eq!(
            validate_analytic_frame(
                AnalyticFrame {
                    semantic: AnalyticSemantic::Gauges,
                    shape: AnalyticShape::PerimeterGaugeSet,
                    rect_points: [0.0, 0.0, 4.0, 4.0],
                    geometry: AnalyticGeometry::PerimeterGaugeSet {
                        center_points: [1.5, 1.5],
                        xp: lane(tiny_layout.xp),
                        daily: lane(tiny_layout.daily),
                        pace: lane(tiny_layout.pace),
                    },
                },
                tiny_camera,
            ),
            Err(SceneValidationError::InvalidFrameValue)
        );
    }

    #[test]
    fn analytic_rectangles_must_match_their_geometry_in_full_and_delta_paths() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        for slot in [0_usize, 2, 3, 4, 5, 6, 7] {
            let mut changed = fixture.frame.analytic_slots[slot];
            changed.value.as_mut().unwrap().rect_points[2] *= 0.5;

            let mut full = fixture.frame.clone();
            full.analytic_slots[slot] = changed;
            assert_eq!(
                validate_full_generation(&fixture.template, &fixture.content, &full).map(|_| ()),
                Err(SceneValidationError::InvalidFrameValue),
                "full slot {slot}"
            );

            let mut current = validate_frame(&fixture.frame, &accepted).unwrap();
            let before = current.frame().clone();
            let mut delta = FrameDelta::empty();
            delta.analytic_slots.push(changed);
            assert_eq!(
                validate_frame_delta(&delta, &accepted, &mut current),
                Err(SceneValidationError::InvalidFrameValue),
                "delta slot {slot}"
            );
            assert_eq!(current.frame(), &before);
        }
    }

    #[test]
    fn duplicate_ids_and_capacity_overflow_are_rejected() {
        let mut template = SceneFixture::valid().template;
        template.nodes.push(template.nodes[0].clone());
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::DuplicateNodeId)
        );
        template = SceneFixture::valid().template;
        template.capacities.max_nodes = 1;
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::NodeCapacityExceeded)
        );
    }

    #[test]
    fn prop_frame_footprint_visibility_and_shadow_strength_are_validated() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();

        let mut frame = fixture.frame.clone();
        frame.prop_slots[0].footprint_points = [f32::NAN, 1.0];
        assert_eq!(
            validate_frame(&frame, &accepted).map(|_| ()),
            Err(SceneValidationError::NonFiniteFrameValue)
        );

        let mut frame = fixture.frame.clone();
        frame.prop_slots[0].footprint_points = [-1.0, 1.0];
        assert_eq!(
            validate_frame(&frame, &accepted).map(|_| ()),
            Err(SceneValidationError::InvalidFrameValue)
        );

        let mut frame = fixture.frame.clone();
        frame.prop_slots[0].contact_shadow_strength = f32::INFINITY;
        assert_eq!(
            validate_frame(&frame, &accepted).map(|_| ()),
            Err(SceneValidationError::NonFiniteFrameValue)
        );

        let mut frame = fixture.frame.clone();
        frame.prop_slots[0].contact_shadow_strength = 1.01;
        assert_eq!(
            validate_frame(&frame, &accepted).map(|_| ()),
            Err(SceneValidationError::InvalidFrameValue)
        );

        let mut frame = fixture.frame.clone();
        frame.prop_slots[0].visible = false;
        frame.prop_slots[0].contact_shadow_strength = 0.25;
        assert_eq!(
            validate_frame(&frame, &accepted).map(|_| ()),
            Err(SceneValidationError::InvalidFrameValue)
        );
    }

    #[test]
    fn duplicate_authored_order_and_noncanonical_empty_mirrors_are_rejected() {
        let mut fixture = SceneFixture::valid();
        let mut duplicate = fixture.template.primitives[0].clone();
        duplicate.node = fixture.template.nodes[0].id;
        fixture.template.primitives.push(duplicate);
        assert_eq!(
            validate_template(&fixture.template),
            Err(SceneValidationError::DuplicateAuthoredOrder)
        );

        let mut fixture = SceneFixture::valid();
        fixture.frame.prop_slots[0].visible = true;
        assert_eq!(
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );

        let mut fixture = SceneFixture::valid();
        fixture.content.pet_art_slots[1].palette_role = PetPaletteRole::Eye;
        assert_eq!(
            validate_content(&fixture.content),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );

        let mut bad_glyphs =
            [PropGlyphContent { glyph: None, local_cell: [0; 2] }; MAX_PROP_GLYPHS_PER_SLOT];
        bad_glyphs[0].local_cell = [1, 0];
        let bad_prop = PropContentSlot {
            slot: 0,
            content: Some(PropSemanticContent {
                sprite_phase: None,
                twinkle_active: None,
                lid_open: None,
                bloom_active: None,
                glyphs: bad_glyphs,
            }),
        };
        let mut fixture = SceneFixture::valid();
        fixture.content.prop_slots[0] = bad_prop;
        assert_eq!(
            validate_content(&fixture.content),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );

        let mut delta = ContentDelta::empty();
        delta.pet_art_slots.push(PetArtSlot {
            slot: 1,
            glyph: None,
            palette_role: PetPaletteRole::Eye,
        });
        assert_eq!(
            validate_content_delta(&delta),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );
        delta.pet_art_slots.clear();
        delta.prop_slots.push(bad_prop);
        assert_eq!(
            validate_content_delta(&delta),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );
    }

    #[test]
    fn aliases_retain_evidence_for_hash_collision_rejection() {
        let mut template = SceneFixture::valid().template;
        let first = CanonicalAlias::new("costarring").unwrap();
        let second = CanonicalAlias::new("liquid").unwrap();
        assert_eq!(NodeId::from_alias(&first), NodeId::from_alias(&second));
        template.nodes[0].alias = first;
        template.nodes[0].id = NodeId::from_alias(&template.nodes[0].alias);
        template.nodes[1].alias = second;
        template.nodes[1].id = NodeId::from_alias(&template.nodes[1].alias);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::NodeIdCollision)
        );
    }

    #[test]
    fn all_hashed_identity_families_reject_alias_collisions_and_mismatches() {
        let first = CanonicalAlias::new("costarring").unwrap();
        let second = CanonicalAlias::new("liquid").unwrap();

        let mut template = SceneFixture::valid().template;
        template.materials[0].alias = first.clone();
        template.materials[0].id = MaterialId::from_alias(&first);
        let mut collision = template.materials[0].clone();
        collision.alias = second.clone();
        collision.id = MaterialId::from_alias(&second);
        template.materials.push(collision);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::MaterialIdCollision)
        );

        let mut template = SceneFixture::valid().template;
        template.resources[0].alias = first.clone();
        template.resources[0].id = ResourceId::from_alias(&first);
        let mut collision = template.resources[0].clone();
        collision.alias = second.clone();
        collision.id = ResourceId::from_alias(&second);
        template.resources.push(collision);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::ResourceIdCollision)
        );

        let mut template = SceneFixture::valid().template;
        template.attachments[0].alias = first.clone();
        template.attachments[0].id = AttachmentId::from_alias(&first);
        let mut collision = template.attachments[0].clone();
        collision.alias = second.clone();
        collision.id = AttachmentId::from_alias(&second);
        template.attachments.push(collision);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::AttachmentIdCollision)
        );

        let mut template = SceneFixture::valid().template;
        template.nodes[0].id = NodeId(0);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::AliasIdMismatch)
        );
    }

    #[test]
    fn cycles_and_dangling_references_are_rejected() {
        let mut template = SceneFixture::valid().template;
        template.nodes[0].parent = Some(template.nodes[1].id);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::HierarchyCycle)
        );

        let mut template = SceneFixture::valid().template;
        template.attachments[0].owner = NodeId(17);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::DanglingNodeReference)
        );

        let mut template = SceneFixture::valid_lit_card();
        template.nodes[1].parent = Some(template.nodes[1].id);
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::HierarchyCycle)
        );

        let mut template = SceneFixture::valid_lit_card();
        let a_alias = CanonicalAlias::new("world.disconnected-a").unwrap();
        let b_alias = CanonicalAlias::new("world.disconnected-b").unwrap();
        let a = NodeId::from_alias(&a_alias);
        let b = NodeId::from_alias(&b_alias);
        template.nodes.push(NodeTemplate {
            id: a,
            alias: a_alias,
            parent: Some(b),
            base_transform: Transform3::IDENTITY,
            local_bounds: Bounds3 { min: [0.0; 3], max: [1.0; 3] },
            depth_cue: DepthCue::NEUTRAL,
        });
        template.nodes.push(NodeTemplate {
            id: b,
            alias: b_alias,
            parent: Some(a),
            base_transform: Transform3::IDENTITY,
            local_bounds: Bounds3 { min: [0.0; 3], max: [1.0; 3] },
            depth_cue: DepthCue::NEUTRAL,
        });
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::HierarchyCycle)
        );
    }

    #[test]
    fn invalid_material_depth_pairing_is_rejected() {
        let mut template = SceneFixture::valid().template;
        template.primitives[0].depth = DepthBehavior::ScreenNoDepth;
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::MaterialDepthIncompatible)
        );
    }

    fn instance_template(binding: InstanceGroupBinding) -> SceneTemplate {
        let mut template = SceneFixture::valid().template;
        let primitive = &mut template.primitives[0];
        primitive.kind = PrimitiveKind::InstanceQuad;
        primitive.binding = PrimitiveBinding::Instances(binding);
        primitive.space = PrimitiveSpace::World;
        primitive.depth = DepthBehavior::WorldReadOnly;
        match binding {
            InstanceGroupBinding::PetArt(PetArtFilter::Particles)
            | InstanceGroupBinding::Ambient => {
                template.materials[0].kind = MaterialKind::AdditiveGlow;
                primitive.blend = WorldBlend::Additive;
            }
            InstanceGroupBinding::Hud => {
                template.materials[0].kind = MaterialKind::ScreenChrome;
                primitive.blend = WorldBlend::PremultipliedAlpha;
                primitive.depth = DepthBehavior::ScreenNoDepth;
                primitive.space = PrimitiveSpace::Screen;
            }
            _ => {
                template.materials[0].kind = MaterialKind::UnlitGlyphSprite;
                primitive.blend = WorldBlend::PremultipliedAlpha;
            }
        }
        template
    }

    fn analytic_template(semantic: AnalyticSemantic) -> SceneTemplate {
        let mut template = SceneFixture::valid().template;
        let primitive = &mut template.primitives[0];
        primitive.kind = PrimitiveKind::AnalyticShape;
        primitive.binding = PrimitiveBinding::Analytic(semantic.id());
        template.resources[0].kind = ResourceKind::AnalyticGeometry;
        match semantic {
            AnalyticSemantic::RoomBackground => {
                template.materials[0].kind = MaterialKind::UnlitAnalytic;
                primitive.blend = WorldBlend::Opaque;
                primitive.depth = DepthBehavior::WorldWrite;
                primitive.space = PrimitiveSpace::World;
            }
            AnalyticSemantic::WallShadow | AnalyticSemantic::FloorProjection => {
                template.materials[0].kind = MaterialKind::MultiplyShadow;
                primitive.blend = WorldBlend::Multiply;
                primitive.depth = DepthBehavior::WorldReadOnly;
                primitive.space = PrimitiveSpace::World;
            }
            AnalyticSemantic::MoodAura => {
                template.materials[0].kind = MaterialKind::UnlitAnalytic;
                primitive.blend = WorldBlend::PremultipliedAlpha;
                primitive.depth = DepthBehavior::WorldReadOnly;
                primitive.space = PrimitiveSpace::World;
            }
            AnalyticSemantic::StatusHalo
            | AnalyticSemantic::Gauges
            | AnalyticSemantic::Trouble
            | AnalyticSemantic::Dim => {
                template.materials[0].kind = MaterialKind::ScreenChrome;
                primitive.blend = WorldBlend::PremultipliedAlpha;
                primitive.depth = DepthBehavior::ScreenNoDepth;
                primitive.space = PrimitiveSpace::Screen;
            }
        }
        template
    }

    #[test]
    fn instance_bindings_require_exact_semantics_and_unique_sources() {
        let bindings = [
            InstanceGroupBinding::RoomGlyphs,
            InstanceGroupBinding::PetArt(PetArtFilter::Body),
            InstanceGroupBinding::PetArt(PetArtFilter::Particles),
            InstanceGroupBinding::PropGlyphs(2),
            InstanceGroupBinding::TankCells { slot: 1, layer: InstanceLayer::Behind },
            InstanceGroupBinding::TankCells {
                slot: 1,
                layer: InstanceLayer::Foreground,
            },
            InstanceGroupBinding::Ambient,
            InstanceGroupBinding::Hud,
        ];

        for binding in bindings {
            let template = instance_template(binding);
            assert!(validate_template(&template).is_ok(), "{binding:?}");

            let mut duplicate = template.clone();
            let mut primitive = duplicate.primitives[0].clone();
            primitive.authored_order = 1;
            duplicate.primitives.push(primitive);
            assert_eq!(
                validate_template(&duplicate),
                Err(SceneValidationError::InvalidPrimitiveBinding),
                "{binding:?}"
            );

            let mut wrong_blend = template;
            wrong_blend.primitives[0].blend = match binding {
                InstanceGroupBinding::PetArt(PetArtFilter::Particles)
                | InstanceGroupBinding::Ambient => WorldBlend::PremultipliedAlpha,
                _ => WorldBlend::Additive,
            };
            assert!(validate_template(&wrong_blend).is_err(), "{binding:?}");
        }

        let mut distinct_props = instance_template(InstanceGroupBinding::PropGlyphs(0));
        let mut second_prop = distinct_props.primitives[0].clone();
        second_prop.binding = PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(1));
        second_prop.authored_order = 1;
        distinct_props.primitives.push(second_prop);
        assert!(validate_template(&distinct_props).is_ok());

        let mut distinct_tank_layers = instance_template(InstanceGroupBinding::TankCells {
            slot: 0,
            layer: InstanceLayer::Behind,
        });
        let mut foreground = distinct_tank_layers.primitives[0].clone();
        foreground.binding = PrimitiveBinding::Instances(InstanceGroupBinding::TankCells {
            slot: 0,
            layer: InstanceLayer::Foreground,
        });
        foreground.authored_order = 1;
        distinct_tank_layers.primitives.push(foreground);
        assert!(validate_template(&distinct_tank_layers).is_ok());
    }

    #[test]
    fn analytic_template_bounds_are_exactly_unit_normalized() {
        let mut template = SceneFixture::valid().template;
        template.analytic_templates[0]
            .value
            .as_mut()
            .unwrap()
            .normalized_local_bounds
            .max[0] = 2.0;
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );
    }

    #[test]
    fn analytic_bindings_require_their_exact_semantic_render_state() {
        for semantic in AnalyticSemantic::ALL {
            let template = analytic_template(semantic);
            assert!(validate_template(&template).is_ok(), "{semantic:?}");

            let alternate = match semantic {
                AnalyticSemantic::RoomBackground => AnalyticSemantic::WallShadow,
                AnalyticSemantic::WallShadow | AnalyticSemantic::FloorProjection => {
                    AnalyticSemantic::MoodAura
                }
                AnalyticSemantic::MoodAura
                | AnalyticSemantic::StatusHalo
                | AnalyticSemantic::Gauges
                | AnalyticSemantic::Trouble
                | AnalyticSemantic::Dim => AnalyticSemantic::WallShadow,
            };
            let alternate = analytic_template(alternate);
            let mut wrong = template;
            wrong.materials[0].kind = alternate.materials[0].kind;
            wrong.primitives[0].blend = alternate.primitives[0].blend;
            wrong.primitives[0].depth = alternate.primitives[0].depth;
            wrong.primitives[0].space = alternate.primitives[0].space;
            assert_eq!(
                validate_template(&wrong),
                Err(SceneValidationError::InvalidPrimitiveBinding),
                "{semantic:?}"
            );
        }
    }

    #[test]
    fn full_validation_rejects_non_finite_authored_cues_and_zero_quaternions() {
        let mut template = SceneFixture::valid().template;
        template.nodes[0].depth_cue.opacity = f32::NAN;
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::NonFiniteDepthCue)
        );

        let mut template = SceneFixture::valid().template;
        template.nodes[0].base_transform.rotation_xyzw = [0.0; 4];
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::ZeroQuaternion)
        );
    }

    #[test]
    fn lit_shallow_cards_reject_non_uniform_or_negative_nested_ancestor_scale() {
        let mut template = SceneFixture::valid_lit_card();
        let grandparent_alias = CanonicalAlias::new("world.root").unwrap();
        let grandparent = NodeId::from_alias(&grandparent_alias);
        template.nodes[0].parent = Some(grandparent);
        template.nodes.push(NodeTemplate {
            id: grandparent,
            alias: grandparent_alias,
            parent: None,
            base_transform: Transform3 {
                scale: [1.0, 1.000_1, 1.0],
                ..Transform3::IDENTITY
            },
            local_bounds: Bounds3 { min: [0.0; 3], max: [360.0, 360.0, 0.0] },
            depth_cue: DepthCue::NEUTRAL,
        });
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );

        let mut template = SceneFixture::valid_lit_card();
        template.nodes[0].base_transform.scale = [-1.0, -1.0, -1.0];
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );

        let mut template = SceneFixture::valid_lit_card();
        template.nodes[0].base_transform.scale = [1.0, 1.0 + 0.5 * LIT_CARD_SCALE_TOLERANCE, 1.0];
        assert!(validate_template(&template).is_ok());

        let mut template = SceneFixture::valid_lit_card();
        template.nodes[0].base_transform.rotation_xyzw = [0.0, 0.0, 0.382_683_43, 0.923_879_5];
        template.nodes[1].base_transform.scale = [2.0, 1.0, 1.0];
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );

        let mut template = SceneFixture::valid_lit_card();
        template.nodes[0].base_transform.scale = [1.0e-20; 3];
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );
    }

    #[test]
    fn external_scene_privacy_must_be_fully_sanitized() {
        let mut template = SceneFixture::valid().template;
        template.privacy.exact_counts_visible = true;
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::PrivacyViolation)
        );
    }

    #[test]
    fn full_content_and_frame_validation_reject_slot_and_reference_errors() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        let mut content = fixture.content.clone();
        content.pet_art_slots[0].slot = MAX_PET_ART_SLOTS as u16;
        assert_eq!(
            validate_content(&content),
            Err(SceneValidationError::PetArtSlotOutOfBounds)
        );

        let mut frame = fixture.frame.clone();
        frame.nodes[0].node = NodeId(42);
        assert_eq!(
            validate_frame(&frame, &accepted).map(|_| ()),
            Err(SceneValidationError::DanglingNodeReference)
        );

        let mut frame = fixture.frame.clone();
        frame.nodes[0].local_transform.scale = [1.0, 2.0, 1.0];
        let lit_template = SceneFixture::valid_lit_card();
        let accepted_lit = validate_template(&lit_template).unwrap();
        assert_eq!(
            validate_frame(&frame, &accepted_lit).map(|_| ()),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );

        let mut frame = fixture.frame.clone();
        frame.dim_amount = f32::INFINITY;
        assert_eq!(
            validate_frame(&frame, &accepted).map(|_| ()),
            Err(SceneValidationError::NonFiniteFrameValue)
        );
    }

    #[test]
    fn room_frame_grid_rejects_full_frame_geometry_and_camera_mismatch() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();

        let mut out_of_grid = fixture.frame.clone();
        out_of_grid.room_glyph_slots[0] = RoomGlyphFrameSlot {
            slot: 0,
            visible: true,
            grid_cell: [fixture.template.glyph_grid.columns, 0],
            position_points: [0.0, 348.0],
            opacity: 1.0,
        };
        assert_eq!(
            validate_frame(&out_of_grid, &accepted),
            Err(SceneValidationError::InvalidGlyphGrid)
        );

        let mut arbitrary_position = fixture.frame.clone();
        arbitrary_position.room_glyph_slots[0] = RoomGlyphFrameSlot {
            slot: 0,
            visible: true,
            grid_cell: [1, 1],
            position_points: [13.0, 336.0],
            opacity: 1.0,
        };
        assert_eq!(
            validate_frame(&arbitrary_position, &accepted),
            Err(SceneValidationError::InvalidGlyphGrid)
        );

        let mut mismatched_camera = fixture.frame;
        mismatched_camera.camera = OrthographicCamera::new(361.0, 360.0, -2.0, 2.0).unwrap();
        assert_eq!(
            validate_frame(&mismatched_camera, &accepted),
            Err(SceneValidationError::InvalidGlyphGrid)
        );
    }

    #[test]
    fn room_frame_grid_rejects_slot_and_camera_only_deltas() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        let current = validate_frame(&fixture.frame, &accepted).unwrap();

        for malformed in [
            RoomGlyphFrameSlot {
                slot: 0,
                visible: true,
                grid_cell: [fixture.template.glyph_grid.columns, 0],
                position_points: [0.0, 348.0],
                opacity: 1.0,
            },
            RoomGlyphFrameSlot {
                slot: 0,
                visible: true,
                grid_cell: [1, 1],
                position_points: [13.0, 336.0],
                opacity: 1.0,
            },
        ] {
            let mut delta = FrameDelta::empty();
            delta.room_glyph_slots.push(malformed);
            let mut target = current.clone();
            assert_eq!(
                validate_frame_delta(&delta, &accepted, &mut target),
                Err(SceneValidationError::InvalidGlyphGrid)
            );
            assert_eq!(target, current);
        }

        let mut camera_only = FrameDelta::empty();
        camera_only.camera = Some(OrthographicCamera::new(361.0, 360.0, -2.0, 2.0).unwrap());
        let mut target = current.clone();
        assert_eq!(
            validate_frame_delta(&camera_only, &accepted, &mut target),
            Err(SceneValidationError::InvalidGlyphGrid)
        );
        assert_eq!(target, current);
    }

    #[test]
    fn attachment_instance_binding_must_resolve_to_an_ancestor_instance_source() {
        let mut template = SceneFixture::valid().template;
        template.attachments[0].instance_binding = Some(AttachmentInstanceBinding::PropGlyphs(
            MAX_VISIBLE_PROPS as u8,
        ));
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::InvalidAttachmentBinding)
        );

        let mut template = SceneFixture::valid().template;
        template.primitives[0].binding =
            PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(0));
        template.attachments[0].instance_binding = Some(AttachmentInstanceBinding::PropGlyphs(0));
        template.attachments[0].owner = template.nodes[0].id;
        assert_eq!(
            validate_template(&template),
            Err(SceneValidationError::InvalidAttachmentBinding)
        );
    }

    #[test]
    fn attachment_instance_binding_rejects_canonical_empty_prop_slot() {
        let mut fixture = SceneFixture::valid();
        fixture.template.primitives[0].kind = PrimitiveKind::InstanceQuad;
        fixture.template.primitives[0].binding =
            PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(0));
        fixture.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        fixture.template.primitives[0].depth = DepthBehavior::WorldReadOnly;
        fixture.template.attachments[0].instance_binding =
            Some(AttachmentInstanceBinding::PropGlyphs(0));
        assert_eq!(
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame),
            Err(SceneValidationError::InvalidAttachmentBinding)
        );
    }

    #[test]
    fn deltas_enforce_versions_and_slot_bounds_without_running_full_template_validation() {
        let fixture = SceneFixture::valid();
        let mut invalid_template = fixture.template.clone();
        invalid_template
            .nodes
            .push(invalid_template.nodes[0].clone());

        let mut content_delta = ContentDelta::empty();
        assert_eq!(validate_content_delta(&content_delta), Ok(()));
        content_delta.schema_version += 1;
        assert_eq!(
            validate_content_delta(&content_delta),
            Err(SceneValidationError::SchemaVersionMismatch)
        );
        content_delta.schema_version -= 1;
        content_delta.pet_art_slots.push(PetArtSlot {
            slot: MAX_PET_ART_SLOTS as u16,
            glyph: None,
            palette_role: PetPaletteRole::Body,
        });
        assert_eq!(
            validate_content_delta(&content_delta),
            Err(SceneValidationError::PetArtSlotOutOfBounds)
        );

        let frame_delta = FrameDelta::empty();
        let accepted = validate_template(&fixture.template).unwrap();
        let mut accepted_frame = validate_frame(&fixture.frame, &accepted).unwrap();
        assert!(validate_frame_delta(&frame_delta, &accepted, &mut accepted_frame).is_ok());
        assert_eq!(
            validate_template(&invalid_template),
            Err(SceneValidationError::DuplicateNodeId)
        );

        let mut frame_delta = FrameDelta::empty();
        frame_delta.nodes.push(NodeFrameState {
            node: fixture.template.nodes[0].id,
            local_transform: Transform3 {
                scale: [1.0, 2.0, 1.0],
                ..Transform3::IDENTITY
            },
            visible: true,
            opacity: 1.0,
        });
        let lit_template = validate_template(&SceneFixture::valid_lit_card()).unwrap();
        let mut lit_frame = validate_frame(&SceneFixture::valid().frame, &lit_template).unwrap();
        assert_eq!(
            validate_frame_delta(&frame_delta, &lit_template, &mut lit_frame),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );
    }

    #[test]
    fn accepted_template_is_owned_and_reusable_for_deltas() {
        let accepted = {
            let template = SceneFixture::valid().template;
            validate_template(&template).unwrap()
        };
        let mut accepted_frame = validate_frame(&SceneFixture::valid().frame, &accepted).unwrap();
        assert!(validate_frame_delta(&FrameDelta::empty(), &accepted, &mut accepted_frame).is_ok());
        assert_eq!(accepted.template().nodes.len(), 2);

        let fixture = SceneFixture::valid();
        let state =
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap();
        let (template, mut frame) = state.into_parts();
        assert!(validate_frame_delta(&FrameDelta::empty(), &template, &mut frame).is_ok());
    }

    #[test]
    fn lit_card_frame_linear_transform_rejects_reflection_shear_and_tiny_scale() {
        let template = SceneFixture::valid_lit_card();
        let accepted = validate_template(&template).unwrap();
        let mut current = validate_frame(&SceneFixture::valid().frame, &accepted).unwrap();
        for scale in [[-1.0, 1.0, 1.0], [2.0, 1.0, 1.0], [1.0e-20; 3]] {
            let mut delta = FrameDelta::empty();
            delta.nodes.push(NodeFrameState {
                node: template.nodes[0].id,
                local_transform: Transform3 {
                    rotation_xyzw: [0.0, 0.0, 0.382_683_43, 0.923_879_5],
                    scale,
                    ..Transform3::IDENTITY
                },
                visible: true,
                opacity: 1.0,
            });
            assert_eq!(
                validate_frame_delta(&delta, &accepted, &mut current),
                Err(SceneValidationError::LitCardScaleIncompatible)
            );
        }

        let mut shear = FrameDelta::empty();
        shear.nodes.push(NodeFrameState {
            node: template.nodes[0].id,
            local_transform: Transform3 {
                scale: [2.0, 1.0, 1.581_138_8],
                ..Transform3::IDENTITY
            },
            visible: true,
            opacity: 1.0,
        });
        shear.nodes.push(NodeFrameState {
            node: template.nodes[1].id,
            local_transform: Transform3 {
                rotation_xyzw: [0.0, 0.0, 0.382_683_43, 0.923_879_5],
                ..Transform3::IDENTITY
            },
            visible: true,
            opacity: 1.0,
        });
        assert_eq!(
            validate_frame_delta(&shear, &accepted, &mut current),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );
    }

    #[test]
    fn content_delta_rejects_adversarial_slot_values() {
        let mut missing_paint = ContentDelta::empty();
        missing_paint.prop_slots.push(PropContentSlot {
            slot: 0,
            content: Some(PropSemanticContent {
                sprite_phase: Some(1),
                twinkle_active: None,
                lid_open: None,
                bloom_active: None,
                glyphs: std::array::from_fn(|index| PropGlyphContent {
                    glyph: (index == 0).then(|| AuthoredGlyph::new('*').unwrap()),
                    local_cell: [0; 2],
                }),
            }),
        });
        assert_eq!(
            validate_content_delta(&missing_paint),
            Err(SceneValidationError::NonCanonicalEmptySlot)
        );

        let mut delta = ContentDelta::empty();
        delta.pet_art_slots.push(PetArtSlot {
            slot: 0,
            glyph: Some(PetGlyph::for_species('^', crate::pet::generation::Species::Fuzz).unwrap()),
            palette_role: PetPaletteRole::Eye,
        });
        delta.prop_slots.push(PropContentSlot {
            slot: 0,
            content: Some(PropSemanticContent {
                sprite_phase: None,
                twinkle_active: None,
                lid_open: Some(true),
                bloom_active: None,
                glyphs: [PropGlyphContent { glyph: None, local_cell: [0; 2] };
                    MAX_PROP_GLYPHS_PER_SLOT],
            }),
        });
        delta.tank_slots.push(TankContentSlot {
            slot: 0,
            content: Some(TankSemanticContent {
                sprite_variant: 1,
                morph: None,
                color_srgb8: [126, 238, 255],
                bold: true,
                glyphs: [None; MAX_TANK_GLYPHS_PER_SLOT],
            }),
        });
        delta.ambient_slots.push(AmbientContentSlot {
            slot: 0,
            kind: Some(AmbientContentKind::Mote),
            glyph: Some(AuthoredGlyph::new('✦').unwrap()),
        });
        delta.prop_paint_slots.push(PropGlyphPaintSlot {
            slot: 0,
            paints: [None; MAX_PROP_GLYPHS_PER_SLOT],
        });
        delta.ambient_paint_slots.push(AmbientGlyphPaintSlot {
            slot: 0,
            paint: Some(GlyphPaintSource { color_srgb8: [1, 2, 3] }),
        });
        assert_eq!(validate_content_delta(&delta), Ok(()));
        delta
            .prop_slots
            .push(PropContentSlot { slot: 0, content: None });
        assert_eq!(
            validate_content_delta(&delta),
            Err(SceneValidationError::DuplicateSlot)
        );
    }

    #[test]
    fn frame_delta_merges_over_current_lit_world_transform() {
        let template = SceneFixture::valid_lit_card();
        let accepted = validate_template(&template).unwrap();
        let mut current = SceneFixture::valid().frame;
        current.nodes[0].local_transform.scale = [2.0, 1.0, 1.0];
        current.nodes[1].local_transform.scale = [0.5, 1.0, 1.0];
        let mut current = validate_frame(&current, &accepted).unwrap();
        let before = current.clone();

        let mut delta = FrameDelta::empty();
        delta.nodes.push(NodeFrameState {
            node: template.nodes[1].id,
            local_transform: Transform3::IDENTITY,
            visible: true,
            opacity: 1.0,
        });
        assert_eq!(
            validate_frame_delta(&delta, &accepted, &mut current),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );
        assert_eq!(current, before);
    }

    #[test]
    fn frame_delta_checks_only_changed_slots_and_affected_lit_paths() {
        let unlit_fixture = SceneFixture::valid();
        let unlit_template = validate_template(&unlit_fixture.template).unwrap();
        let mut unlit_frame = validate_frame(&unlit_fixture.frame, &unlit_template).unwrap();
        let mut delta = FrameDelta::empty();
        delta.nodes.push(NodeFrameState {
            node: unlit_fixture.template.nodes[0].id,
            local_transform: Transform3::translated([1.0, 0.0, 0.0]),
            visible: true,
            opacity: 1.0,
        });
        let audit = validate_frame_delta(&delta, &unlit_template, &mut unlit_frame).unwrap();
        assert_eq!(audit.node_slots_checked, 1);
        assert_eq!(audit.node_slots_applied, 1);
        assert_eq!(audit.lit_path_indices_visited, 0);
        assert_eq!(audit.lit_paths_checked, 0);

        let mut lit_source = SceneFixture::valid_lit_card();
        let sibling_alias = CanonicalAlias::new("pet.sibling").unwrap();
        let sibling = NodeId::from_alias(&sibling_alias);
        let mut sibling_node = lit_source.nodes[1].clone();
        sibling_node.id = sibling;
        sibling_node.alias = sibling_alias;
        lit_source.nodes.push(sibling_node);
        let mut sibling_primitive = lit_source.primitives[0].clone();
        sibling_primitive.node = sibling;
        sibling_primitive.authored_order = 1;
        lit_source.primitives.push(sibling_primitive);
        let lit_template = validate_template(&lit_source).unwrap();
        assert_eq!(lit_template.lit_paths.len(), 2);
        let mut lit_frame_source = SceneFixture::valid().frame;
        lit_frame_source.nodes.push(NodeFrameState {
            node: sibling,
            local_transform: Transform3::IDENTITY,
            visible: true,
            opacity: 1.0,
        });
        let mut lit_frame = validate_frame(&lit_frame_source, &lit_template).unwrap();
        delta.nodes[0].node = lit_source.nodes[1].id;
        let audit = validate_frame_delta(&delta, &lit_template, &mut lit_frame).unwrap();
        assert_eq!(audit.node_slots_checked, 1);
        assert_eq!(audit.node_slots_applied, 1);
        assert_eq!(audit.lit_path_indices_visited, 1);
        assert_eq!(audit.lit_paths_checked, 1);
    }

    #[test]
    fn repeated_frame_deltas_preserve_persistent_storage() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        let mut frame = validate_frame(&fixture.frame, &accepted).unwrap();
        let nodes_ptr = frame.frame().nodes.as_ptr();
        let nodes_capacity = frame.frame().nodes.capacity();
        let lights_ptr = frame.frame().lights.as_ptr();
        let lights_capacity = frame.frame().lights.capacity();
        for step in 0..300 {
            let mut delta = FrameDelta::empty();
            delta.nodes.push(NodeFrameState {
                node: fixture.template.nodes[0].id,
                local_transform: Transform3::translated([step as f32 * 0.01, 0.0, 0.0]),
                visible: true,
                opacity: 1.0,
            });
            validate_frame_delta(&delta, &accepted, &mut frame).unwrap();
        }
        assert_eq!(frame.frame().nodes.as_ptr(), nodes_ptr);
        assert_eq!(frame.frame().nodes.capacity(), nodes_capacity);
        assert_eq!(frame.frame().lights.as_ptr(), lights_ptr);
        assert_eq!(frame.frame().lights.capacity(), lights_capacity);
    }

    #[test]
    fn accepted_frame_is_bound_to_its_template_acceptance() {
        let fixture = SceneFixture::valid();
        let accepted_a = validate_template(&fixture.template).unwrap();
        let accepted_b = validate_template(&fixture.template).unwrap();
        let mut current_b = validate_frame(&fixture.frame, &accepted_b).unwrap();
        assert_eq!(
            validate_frame_delta(&FrameDelta::empty(), &accepted_a, &mut current_b),
            Err(SceneValidationError::AcceptedStateMismatch)
        );
    }

    #[test]
    fn accepted_frame_delta_prepare_is_read_only_and_commit_matches_wrapper() {
        let fixture = SceneFixture::valid();
        let mut prepared_state =
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap();
        let mut wrapped_state = prepared_state.clone();
        let before = prepared_state.clone();
        let mut delta = FrameDelta::empty();
        let mut camera = fixture.frame.camera;
        camera.far_z = -4.0;
        camera.near_z = 4.0;
        delta.camera = Some(camera);
        let mut node = fixture.frame.nodes[0];
        node.local_transform = Transform3::translated([3.0, 4.0, 0.0]);
        node.opacity = 0.75;
        delta.nodes.push(node);
        delta.gauges = Some([0.25, 0.5, 0.75, 1.0]);
        delta.dim_amount = Some(0.5);

        let prepared = prepared_state.prepare_frame_delta(&delta).unwrap();
        assert_eq!(prepared_state, before);

        let prepared_audit = prepared_state.commit_prepared_frame_delta(prepared);
        let wrapped_audit = wrapped_state.apply_frame_delta(&delta).unwrap();
        assert_eq!(prepared_audit, wrapped_audit);
        assert_eq!(prepared_state, wrapped_state);
    }

    #[test]
    fn prepared_frame_delta_survives_moving_state_and_caller_delta() {
        let fixture = SceneFixture::valid();
        let state =
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap();
        let mut delta = FrameDelta::empty();
        delta.gauges = Some([0.25, 0.5, 0.75, 1.0]);
        let prepared = state.prepare_frame_delta(&delta).unwrap();

        let moved_delta = Box::new(delta);
        drop(moved_delta);
        let mut moved_state = Box::new(state);
        moved_state.commit_prepared_frame_delta(prepared);

        assert_eq!(moved_state.frame().frame().gauges, [0.25, 0.5, 0.75, 1.0]);
    }

    #[test]
    fn prepared_frame_delta_retains_source_identity_and_clones_get_fresh_identity() {
        let fixture = SceneFixture::valid();
        let state =
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap();
        let source_identity = Arc::downgrade(&state.frame.instance_identity);
        let prepared = state.prepare_frame_delta(&FrameDelta::empty()).unwrap();
        let mut cloned_state = state.clone();
        let before = cloned_state.clone();

        assert!(!Arc::ptr_eq(
            &state.frame.instance_identity,
            &cloned_state.frame.instance_identity,
        ));
        drop(state);
        assert!(source_identity.upgrade().is_some());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cloned_state.commit_prepared_frame_delta(prepared);
        }));

        assert!(result.is_err());
        assert!(source_identity.upgrade().is_none());
        assert_eq!(cloned_state.frame().epoch(), 0);
        assert_eq!(cloned_state, before);
    }

    #[test]
    fn stale_prepared_frame_delta_is_rejected_before_a_second_commit() {
        let fixture = SceneFixture::valid();
        let mut state =
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap();
        let mut first_delta = FrameDelta::empty();
        let mut first_node = fixture.frame.nodes[0];
        first_node.local_transform = Transform3::translated([1.0, 0.0, 0.0]);
        first_delta.nodes.push(first_node);
        let first = state.prepare_frame_delta(&first_delta).unwrap();

        let mut newer_delta = FrameDelta::empty();
        let mut newer_node = fixture.frame.nodes[0];
        newer_node.local_transform = Transform3::translated([2.0, 0.0, 0.0]);
        newer_delta.nodes.push(newer_node);
        let newer = state.prepare_frame_delta(&newer_delta).unwrap();

        assert_eq!(state.frame().epoch(), 0);
        state.commit_prepared_frame_delta(newer);
        assert_eq!(state.frame().epoch(), 1);
        let after_first_commit = state.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state.commit_prepared_frame_delta(first);
        }));

        assert!(result.is_err());
        assert_eq!(state.frame().epoch(), 1);
        assert_eq!(state, after_first_commit);
    }

    #[test]
    fn prepared_frame_delta_cannot_commit_to_a_cloned_frame_instance() {
        let fixture = SceneFixture::valid();
        let state =
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap();
        let prepared = state.prepare_frame_delta(&FrameDelta::empty()).unwrap();
        let mut cloned_state = state.clone();
        let before = cloned_state.clone();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cloned_state.commit_prepared_frame_delta(prepared);
        }));

        assert!(result.is_err());
        assert_eq!(cloned_state.frame().epoch(), 0);
        assert_eq!(cloned_state, before);
    }

    #[test]
    fn frame_delta_validates_and_returns_every_merged_field() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        let mut current = validate_frame(&fixture.frame, &accepted).unwrap();

        let mut valid = FrameDelta::empty();
        valid.gauges = Some([0.25, 0.5, 0.75, 1.0]);
        valid.dim_amount = Some(0.5);
        validate_frame_delta(&valid, &accepted, &mut current).unwrap();
        assert_eq!(current.frame().gauges, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(current.frame().dim_amount, 0.5);

        let mut invalid_gauge = FrameDelta::empty();
        invalid_gauge.gauges = Some([f32::NAN, 0.0, 0.0, 0.0]);
        assert_eq!(
            validate_frame_delta(&invalid_gauge, &accepted, &mut current),
            Err(SceneValidationError::NonFiniteFrameValue)
        );

        let mut invalid_dim = FrameDelta::empty();
        invalid_dim.dim_amount = Some(1.5);
        assert_eq!(
            validate_frame_delta(&invalid_dim, &accepted, &mut current),
            Err(SceneValidationError::InvalidFrameValue)
        );

        let mut invalid_camera = FrameDelta::empty();
        invalid_camera.camera = Some(OrthographicCamera {
            width_points: f32::from_bits(1),
            height_points: 360.0,
            far_z: -2.0,
            near_z: 2.0,
        });
        assert_eq!(
            validate_frame_delta(&invalid_camera, &accepted, &mut current),
            Err(SceneValidationError::InvalidCamera)
        );

        let mut missing_light_slot = FrameDelta::empty();
        missing_light_slot.lights.push((
            0,
            LightFrame {
                direction: [1.0, 0.0, 0.0],
                color_linear: [1.0; 3],
                intensity: 1.0,
            },
        ));
        assert_eq!(
            validate_frame_delta(&missing_light_slot, &accepted, &mut current),
            Err(SceneValidationError::LightSlotOutOfBounds)
        );
    }

    #[test]
    fn lights_reject_finite_values_that_can_overflow_shader_math() {
        let fixture = SceneFixture::valid();
        let accepted = validate_template(&fixture.template).unwrap();
        let mut bounded = fixture.frame.clone();
        bounded.lights.push(LightFrame {
            direction: [MAX_LIGHT_DIRECTION_NORM as f32, 0.0, 0.0],
            color_linear: [MAX_LIGHT_COLOR_LINEAR, 0.0, 0.0],
            intensity: 16.0,
        });
        assert!(validate_frame(&bounded, &accepted).is_ok());

        for light in [
            LightFrame {
                direction: [f32::MAX, 0.0, 0.0],
                color_linear: [1.0; 3],
                intensity: 1.0,
            },
            LightFrame {
                direction: [1.0, 0.0, 0.0],
                color_linear: [f32::MAX; 3],
                intensity: 1.0,
            },
            LightFrame {
                direction: [1.0, 0.0, 0.0],
                color_linear: [1.0; 3],
                intensity: f32::MAX,
            },
        ] {
            let mut frame = fixture.frame.clone();
            frame.lights.push(light);
            assert_eq!(
                validate_frame(&frame, &accepted).map(|_| ()),
                Err(SceneValidationError::InvalidFrameValue)
            );
        }
    }
}
