use super::scene::*;
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
    PropCapacityExceeded,
    TankCapacityExceeded,
    AmbientCapacityExceeded,
    HudCapacityExceeded,
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
    MaterialDepthIncompatible,
    PrimitiveResourceIncompatible,
    LitCardScaleIncompatible,
    PrivacyViolation,
    PetArtSlotOutOfBounds,
    PropSlotOutOfBounds,
    TankSlotOutOfBounds,
    AmbientSlotOutOfBounds,
    HudSlotOutOfBounds,
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

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedSceneFrame {
    frame: SceneFrame,
    template_identity: Arc<()>,
}

impl AcceptedSceneFrame {
    pub fn frame(&self) -> &SceneFrame {
        &self.frame
    }

    pub fn into_frame(self) -> SceneFrame {
        self.frame
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
        validate_frame_delta(delta, &self.template, &mut self.frame)
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
    validate_nodes(template)?;
    validate_materials(template)?;
    validate_resources(template)?;
    validate_attachments(template)?;
    validate_hierarchy(template)?;
    validate_primitives(template)?;
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
    if content.prop_slots.len() > MAX_VISIBLE_PROPS {
        return Err(SceneValidationError::PropCapacityExceeded);
    }
    if content.tank_slots.len() > MAX_ROUND_TANK_INHABITANTS {
        return Err(SceneValidationError::TankCapacityExceeded);
    }
    if content.ambient_slots.len() > MAX_AMBIENT_INSTANCES {
        return Err(SceneValidationError::AmbientCapacityExceeded);
    }
    if content.hud_slots.len() > MAX_HUD_GLYPH_SLOTS {
        return Err(SceneValidationError::HudCapacityExceeded);
    }
    if content.pet_art_slots.len() != MAX_PET_ART_SLOTS
        || content.prop_slots.len() != MAX_VISIBLE_PROPS
        || content.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || content.ambient_slots.len() != MAX_AMBIENT_INSTANCES
        || content.hud_slots.len() != MAX_HUD_GLYPH_SLOTS
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    validate_content_slots(
        &content.pet_art_slots,
        &content.prop_slots,
        &content.tank_slots,
        &content.ambient_slots,
        &content.hud_slots,
    )?;
    if content
        .pet_art_slots
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
            .hud_slots
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
    canonical.prop_slots.sort_by_key(|slot| slot.slot);
    canonical.tank_slots.sort_by_key(|slot| slot.slot);
    canonical.ambient_slots.sort_by_key(|slot| slot.slot);
    canonical.hud_slots.sort_by_key(|slot| slot.slot);
    Ok(AcceptedSceneFrame {
        frame: canonical,
        template_identity: Arc::clone(&accepted_template.identity),
    })
}

fn validate_frame_against_template(
    frame: &SceneFrame,
    accepted_template: &AcceptedSceneTemplate,
) -> Result<(), SceneValidationError> {
    validate_versions(frame.schema_version, frame.renderer_schema_version)?;
    validate_camera(frame.camera)?;
    if frame.nodes.len() > MAX_SCENE_NODES {
        return Err(SceneValidationError::NodeCapacityExceeded);
    }
    if frame.lights.len() > MAX_LIGHTS {
        return Err(SceneValidationError::LightCapacityExceeded);
    }
    if frame.prop_slots.len() != MAX_VISIBLE_PROPS
        || frame.tank_slots.len() != MAX_ROUND_TANK_INHABITANTS
        || frame.ambient_slots.len() != MAX_AMBIENT_INSTANCES
        || frame.hud_slots.len() != MAX_HUD_GLYPH_SLOTS
    {
        return Err(SceneValidationError::FixedSlotCountMismatch);
    }
    validate_instance_frame_slots(frame)?;
    if frame
        .prop_slots
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
        || frame
            .hud_slots
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
        &delta.prop_slots,
        &delta.tank_slots,
        &delta.ambient_slots,
        &delta.hud_slots,
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
    for slot in 0..MAX_HUD_GLYPH_SLOTS {
        let content = content_delta
            .hud_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_content.hud_slots[slot]);
        let frame = frame_delta
            .hud_slots
            .iter()
            .find(|changed| usize::from(changed.slot) == slot)
            .unwrap_or(&current_frame.hud_slots[slot]);
        if content.glyph.is_none()
            && (frame.visible || !zero2(frame.position_points) || frame.opacity.to_bits() != 0)
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
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

    if let Some(camera) = delta.camera {
        current_frame.frame.camera = camera;
    }
    let mut node_slots_applied = 0;
    for (change_index, changed) in delta.nodes.iter().enumerate() {
        if let Some(dense_index) = changed_dense_indices[change_index] {
            current_frame.frame.nodes[dense_index] = *changed;
            node_slots_applied += 1;
        }
    }
    if let Some(gauges) = delta.gauges {
        current_frame.frame.gauges = gauges;
    }
    if let Some(dim_amount) = delta.dim_amount {
        current_frame.frame.dim_amount = dim_amount;
    }
    for (slot, _) in &delta.lights {
        let slot = usize::from(*slot);
        if let Some(changed) = light_overlay[slot] {
            current_frame.frame.lights[slot] = changed;
        }
    }
    for changed in &delta.prop_slots {
        current_frame.frame.prop_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.tank_slots {
        current_frame.frame.tank_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.ambient_slots {
        current_frame.frame.ambient_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.hud_slots {
        current_frame.frame.hud_slots[usize::from(changed.slot)] = *changed;
    }
    Ok(FrameDeltaValidation {
        node_slots_checked: delta.nodes.len(),
        node_slots_applied,
        lit_path_indices_visited: affected_path_count,
        lit_paths_checked,
    })
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
    if capacities != SceneCapacities::FIXED_V1 {
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
        let binding_matches = match (primitive.kind, primitive.instance_group) {
            (PrimitiveKind::InstanceQuad, Some(InstanceGroupBinding::PropGlyphs(slot))) => {
                usize::from(slot) < MAX_VISIBLE_PROPS
            }
            (PrimitiveKind::InstanceQuad, Some(InstanceGroupBinding::TankCells { slot, .. })) => {
                usize::from(slot) < MAX_ROUND_TANK_INHABITANTS
            }
            (
                PrimitiveKind::InstanceQuad,
                Some(
                    InstanceGroupBinding::PetBody
                    | InstanceGroupBinding::PetParticles
                    | InstanceGroupBinding::Ambient
                    | InstanceGroupBinding::Hud,
                ),
            ) => true,
            (PrimitiveKind::InstanceQuad, None) => false,
            (_, None) => true,
            (_, Some(_)) => false,
        };
        let space_matches = match material {
            MaterialKind::ScreenChrome => primitive.space == PrimitiveSpace::Screen,
            _ => primitive.space == PrimitiveSpace::World,
        };
        if !binding_matches || !space_matches {
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
    props: &[PropContentSlot],
    tanks: &[TankContentSlot],
    ambient: &[AmbientContentSlot],
    hud: &[HudContentSlot],
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
    )?;
    validate_unique_slots(
        hud.iter().map(|slot| usize::from(slot.slot)),
        MAX_HUD_GLYPH_SLOTS,
        SceneValidationError::HudSlotOutOfBounds,
    )
}

fn validate_instance_frame_slots(frame: &SceneFrame) -> Result<(), SceneValidationError> {
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
    validate_unique_slots(
        frame.hud_slots.iter().map(|slot| usize::from(slot.slot)),
        MAX_HUD_GLYPH_SLOTS,
        SceneValidationError::HudSlotOutOfBounds,
    )?;
    for slot in &frame.prop_slots {
        validate_points(slot.origin_points)?;
        validate_points(slot.motion_offset_points)?;
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
    for slot in &frame.hud_slots {
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
    for (content, frame) in content.prop_slots.iter().zip(&frame.prop_slots) {
        if content.content.is_none()
            && (frame.visible
                || !zero2(frame.origin_points)
                || !zero2(frame.motion_offset_points)
                || frame.opacity.to_bits() != 0)
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
    for (content, frame) in content.hud_slots.iter().zip(&frame.hud_slots) {
        if content.glyph.is_none()
            && (frame.visible || !zero2(frame.position_points) || frame.opacity.to_bits() != 0)
        {
            return Err(SceneValidationError::NonCanonicalEmptySlot);
        }
    }
    Ok(())
}

fn validate_changed_instance_frame_slots(delta: &FrameDelta) -> Result<(), SceneValidationError> {
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
    validate_unique_slots(
        delta.hud_slots.iter().map(|slot| usize::from(slot.slot)),
        MAX_HUD_GLYPH_SLOTS,
        SceneValidationError::HudSlotOutOfBounds,
    )?;
    for slot in &delta.prop_slots {
        validate_points(slot.origin_points)?;
        validate_points(slot.motion_offset_points)?;
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
    for slot in &delta.hud_slots {
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

    #[test]
    fn full_validation_rejects_non_finite_cues_and_zero_quaternions() {
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
                glyphs: [None; MAX_TANK_GLYPHS_PER_SLOT],
            }),
        });
        delta.ambient_slots.push(AmbientContentSlot {
            slot: 0,
            kind: Some(AmbientContentKind::ActivityPulse),
            glyph: Some(AuthoredGlyph::new('✦').unwrap()),
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
