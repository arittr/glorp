use super::scene::*;
use super::COMPANION_RENDERER_SCHEMA_VERSION;
use crate::presentation::privacy::PresentationSurface;
use std::collections::{HashMap, HashSet};

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
    NodeSlotOutOfBounds,
    LightSlotOutOfBounds,
    DuplicateSlot,
    MissingNodeFrameState,
    NonFiniteFrameValue,
    InvalidFrameValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedSceneTemplate(SceneTemplate);

impl AcceptedSceneTemplate {
    pub fn template(&self) -> &SceneTemplate {
        &self.0
    }
}

pub fn validate_full_generation(
    template: &SceneTemplate,
    content: &SceneContent,
    frame: &SceneFrame,
) -> Result<AcceptedSceneTemplate, SceneValidationError> {
    let accepted = validate_template(template)?;
    validate_content(content)?;
    validate_frame(frame, &accepted)?;
    Ok(accepted)
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
    validate_lit_card_scale_ancestry(template)?;
    validate_privacy(template)?;
    Ok(AcceptedSceneTemplate(template.clone()))
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
    validate_content_slots(
        &content.pet_art_slots,
        &content.prop_slots,
        &content.tank_slots,
        &content.ambient_slots,
    )
}

pub fn validate_frame(
    frame: &SceneFrame,
    accepted_template: &AcceptedSceneTemplate,
) -> Result<(), SceneValidationError> {
    let template = accepted_template.template();
    validate_versions(frame.schema_version, frame.renderer_schema_version)?;
    validate_camera(frame.camera)?;
    if frame.nodes.len() > MAX_SCENE_NODES {
        return Err(SceneValidationError::NodeCapacityExceeded);
    }
    if frame.lights.len() > MAX_LIGHTS {
        return Err(SceneValidationError::LightCapacityExceeded);
    }
    let node_ids = template
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for node in &frame.nodes {
        if !node_ids.contains(&node.node) {
            return Err(SceneValidationError::DanglingNodeReference);
        }
        if !seen.insert(node.node) {
            return Err(SceneValidationError::DuplicateSlot);
        }
        validate_transform(node.local_transform)?;
        validate_unit_interval(node.opacity)?;
    }
    if seen.len() != node_ids.len() {
        return Err(SceneValidationError::MissingNodeFrameState);
    }
    validate_lit_card_frame_scale_ancestry(frame, template)?;
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
    )
}

/// Validates only bounded mutable frame fields against an already accepted
/// template. It deliberately does not re-run full template validation.
pub fn validate_frame_delta(
    delta: &FrameDelta,
    accepted_template: &AcceptedSceneTemplate,
) -> Result<(), SceneValidationError> {
    let accepted_template = accepted_template.template();
    validate_versions(delta.schema_version, delta.renderer_schema_version)?;
    if delta.nodes.len() > MAX_SCENE_NODES {
        return Err(SceneValidationError::NodeCapacityExceeded);
    }
    if delta.lights.len() > MAX_LIGHTS {
        return Err(SceneValidationError::LightCapacityExceeded);
    }
    if let Some(camera) = delta.camera {
        validate_camera(camera)?;
    }
    let node_ids = accepted_template
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<HashSet<_>>();
    let mut changed_nodes = HashSet::new();
    for node in &delta.nodes {
        if !node_ids.contains(&node.node) {
            return Err(SceneValidationError::NodeSlotOutOfBounds);
        }
        if !changed_nodes.insert(node.node) {
            return Err(SceneValidationError::DuplicateSlot);
        }
        validate_transform(node.local_transform)?;
        validate_unit_interval(node.opacity)?;
    }
    validate_lit_card_delta_scale_ancestry(delta, accepted_template)?;
    if let Some(gauges) = delta.gauges {
        for gauge in gauges {
            validate_unit_interval(gauge)?;
        }
    }
    if let Some(dim_amount) = delta.dim_amount {
        validate_unit_interval(dim_amount)?;
    }
    let mut light_slots = HashSet::new();
    for (slot, light) in &delta.lights {
        if usize::from(*slot) >= MAX_LIGHTS {
            return Err(SceneValidationError::LightSlotOutOfBounds);
        }
        if !light_slots.insert(*slot) {
            return Err(SceneValidationError::DuplicateSlot);
        }
        validate_light(*light)?;
    }
    Ok(())
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
    let blended_draws = template
        .primitives
        .iter()
        .filter(|primitive| {
            matches!(
                primitive.blend,
                WorldBlend::PremultipliedAlpha | WorldBlend::Multiply | WorldBlend::Additive
            )
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

fn validate_lit_card_scale_ancestry(template: &SceneTemplate) -> Result<(), SceneValidationError> {
    validate_lit_card_world_transforms(template, |_| None)
}

fn validate_lit_card_frame_scale_ancestry(
    frame: &SceneFrame,
    template: &SceneTemplate,
) -> Result<(), SceneValidationError> {
    let frame_transforms = frame
        .nodes
        .iter()
        .map(|node| (node.node, node.local_transform))
        .collect::<HashMap<_, _>>();
    validate_lit_card_dynamic_transforms(template, |id| frame_transforms.get(&id).copied())
}

fn validate_lit_card_delta_scale_ancestry(
    delta: &FrameDelta,
    template: &SceneTemplate,
) -> Result<(), SceneValidationError> {
    let changed_transforms = delta
        .nodes
        .iter()
        .map(|node| (node.node, node.local_transform))
        .collect::<HashMap<_, _>>();
    validate_lit_card_dynamic_transforms(template, |id| changed_transforms.get(&id).copied())
}

fn validate_lit_card_dynamic_transforms(
    template: &SceneTemplate,
    dynamic_transform: impl Fn(NodeId) -> Option<Transform3>,
) -> Result<(), SceneValidationError> {
    validate_lit_card_world_transforms(template, dynamic_transform)
}

fn validate_lit_card_world_transforms(
    template: &SceneTemplate,
    dynamic_transform: impl Fn(NodeId) -> Option<Transform3>,
) -> Result<(), SceneValidationError> {
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
    for primitive in &template.primitives {
        if materials.get(&primitive.material) != Some(&MaterialKind::LitShallowCard) {
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
            path.push(*node);
            current = node.parent;
        }
        let mut world = Mat4::IDENTITY;
        for node in path.into_iter().rev() {
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
) -> Result<(), SceneValidationError> {
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
    )
}

fn validate_unique_slots(
    slots: impl Iterator<Item = usize>,
    capacity: usize,
    out_of_bounds: SceneValidationError,
) -> Result<(), SceneValidationError> {
    let mut seen = HashSet::new();
    for slot in slots {
        if slot >= capacity {
            return Err(out_of_bounds);
        }
        if !seen.insert(slot) {
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
    let direction_length_squared = light
        .direction
        .iter()
        .map(|value| value * value)
        .sum::<f32>();
    if direction_length_squared <= f32::EPSILON
        || light.color_linear.iter().any(|value| *value < 0.0)
        || light.intensity < 0.0
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
            validate_frame(&frame, &accepted),
            Err(SceneValidationError::DanglingNodeReference)
        );

        let mut frame = fixture.frame.clone();
        frame.nodes[0].local_transform.scale = [1.0, 2.0, 1.0];
        let lit_template = SceneFixture::valid_lit_card();
        let accepted_lit = validate_template(&lit_template).unwrap();
        assert_eq!(
            validate_frame(&frame, &accepted_lit),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );

        let mut frame = fixture.frame.clone();
        frame.dim_amount = f32::INFINITY;
        assert_eq!(
            validate_frame(&frame, &accepted),
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
        assert_eq!(validate_frame_delta(&frame_delta, &accepted), Ok(()));
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
        assert_eq!(
            validate_frame_delta(
                &frame_delta,
                &validate_template(&SceneFixture::valid_lit_card()).unwrap(),
            ),
            Err(SceneValidationError::LitCardScaleIncompatible)
        );
    }

    #[test]
    fn accepted_template_is_owned_and_reusable_for_deltas() {
        let accepted = {
            let template = SceneFixture::valid().template;
            validate_template(&template).unwrap()
        };
        assert_eq!(
            validate_frame_delta(&FrameDelta::empty(), &accepted),
            Ok(())
        );
        assert_eq!(accepted.template().nodes.len(), 2);
    }

    #[test]
    fn lit_card_frame_linear_transform_rejects_reflection_shear_and_tiny_scale() {
        let template = SceneFixture::valid_lit_card();
        let accepted = validate_template(&template).unwrap();
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
                validate_frame_delta(&delta, &accepted),
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
            validate_frame_delta(&shear, &accepted),
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
            kind: PropContentKind::ChestOpen,
        });
        delta.tank_slots.push(TankContentSlot {
            slot: 0,
            kind: TankContentKind::SpriteVariant1,
        });
        delta.ambient_slots.push(AmbientContentSlot {
            slot: 0,
            active: true,
            kind: AmbientContentKind::ActivityPulse,
        });
        assert_eq!(validate_content_delta(&delta), Ok(()));
        delta
            .prop_slots
            .push(PropContentSlot { slot: 0, kind: PropContentKind::Static });
        assert_eq!(
            validate_content_delta(&delta),
            Err(SceneValidationError::DuplicateSlot)
        );
    }
}
