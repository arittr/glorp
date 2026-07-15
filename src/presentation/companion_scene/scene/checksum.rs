use super::compiler::SceneGenerationError;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CanonicalEncodingError;

pub(super) fn canonical_f32_bits(value: f32) -> Result<u32, CanonicalEncodingError> {
    if !value.is_finite() {
        return Err(CanonicalEncodingError);
    }
    Ok(if value == 0.0 { 0 } else { value.to_bits() })
}

struct Fnv1a64(u64);

impl Fnv1a64 {
    fn domain(domain: &[u8]) -> Self {
        let mut hash = Self(0xcbf2_9ce4_8422_2325);
        hash.bytes(domain);
        hash
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }
    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }
    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
    fn i8(&mut self, value: i8) {
        self.u8(value as u8);
    }
    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }
    fn f32(&mut self, value: f32) -> Result<(), CanonicalEncodingError> {
        self.u32(canonical_f32_bits(value)?);
        Ok(())
    }
    fn string(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes(value.as_bytes());
    }
    fn glyph(&mut self, value: char) {
        self.u32(value as u32);
    }
    fn finish(self) -> u64 {
        self.0
    }
}

fn primitive_kind_tag(value: PrimitiveKind) -> u8 {
    match value {
        PrimitiveKind::AtlasQuad => 1,
        PrimitiveKind::AnalyticShape => 2,
        PrimitiveKind::ShallowCard => 3,
        PrimitiveKind::InstanceQuad => 4,
    }
}

fn material_kind_tag(value: MaterialKind) -> u8 {
    match value {
        MaterialKind::UnlitGlyphSprite => 1,
        MaterialKind::UnlitAnalytic => 2,
        MaterialKind::LitShallowCard => 3,
        MaterialKind::MultiplyShadow => 4,
        MaterialKind::AdditiveGlow => 5,
        MaterialKind::ScreenChrome => 6,
    }
}

fn resource_kind_tag(value: ResourceKind) -> u8 {
    match value {
        ResourceKind::GlyphAtlas => 1,
        ResourceKind::ColorAtlas => 2,
        ResourceKind::AnalyticGeometry => 3,
        ResourceKind::ShallowCardGeometry => 4,
    }
}

fn world_blend_tag(value: WorldBlend) -> u8 {
    match value {
        WorldBlend::Opaque => 1,
        WorldBlend::AlphaCutout => 2,
        WorldBlend::PremultipliedAlpha => 3,
        WorldBlend::Multiply => 4,
        WorldBlend::Additive => 5,
    }
}

fn depth_behavior_tag(value: DepthBehavior) -> u8 {
    match value {
        DepthBehavior::WorldWrite => 1,
        DepthBehavior::WorldReadOnly => 2,
        DepthBehavior::ScreenNoDepth => 3,
    }
}

fn primitive_space_tag(value: PrimitiveSpace) -> u8 {
    match value {
        PrimitiveSpace::World => 1,
        PrimitiveSpace::Screen => 2,
    }
}

fn instance_layer_tag(value: InstanceLayer) -> u8 {
    match value {
        InstanceLayer::Behind => 1,
        InstanceLayer::Foreground => 2,
    }
}

fn attachment_mode_tag(value: AttachmentMode) -> u8 {
    match value {
        AttachmentMode::Follow => 1,
        AttachmentMode::SnapshotWorldOnSpawn => 2,
    }
}

fn pet_palette_role_tag(value: PetPaletteRole) -> u8 {
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

fn ambient_content_tag(value: AmbientContentKind) -> u8 {
    match value {
        AmbientContentKind::Mote => 1,
    }
}

fn mood_content_tag(value: MoodContentKind) -> u8 {
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

fn weather_content_tag(value: WeatherContentKind) -> u8 {
    match value {
        WeatherContentKind::Clear => 1,
        WeatherContentKind::CacheMist => 2,
        WeatherContentKind::OutputSparks => 3,
        WeatherContentKind::ReasoningPulse => 4,
        WeatherContentKind::Mixed => 5,
    }
}

fn presentation_surface_tag(value: crate::presentation::privacy::PresentationSurface) -> u8 {
    match value {
        crate::presentation::privacy::PresentationSurface::WatchTui => 1,
        crate::presentation::privacy::PresentationSurface::RoundCompanion => 2,
        crate::presentation::privacy::PresentationSurface::RoundPreviewLab => 3,
        crate::presentation::privacy::PresentationSurface::MenubarPopover => 4,
        crate::presentation::privacy::PresentationSurface::PreviewLabArtifact => 5,
    }
}

pub(super) fn checksum_template(template: &SceneTemplate) -> Result<u64, SceneGenerationError> {
    let mut hash = Fnv1a64::domain(b"glorp.scene-template.v2\0");
    hash.u16(template.schema_version);
    hash.u16(template.renderer_schema_version);
    for value in [
        template.capacities.max_nodes,
        template.capacities.max_static_primitives,
        template.capacities.max_pet_art_slots,
        template.capacities.max_room_glyph_slots,
        template.capacities.max_visible_props,
        template.capacities.max_round_tank_inhabitants,
        template.capacities.max_ambient_instances,
        template.capacities.max_blended_draws,
        template.capacities.max_lights,
        template.capacities.max_attachments,
        template.capacities.max_static_atlas_recipes,
        template.capacities.max_analytic_params,
    ] {
        hash.usize(value);
    }
    hash.u16(template.glyph_grid.columns);
    hash.u16(template.glyph_grid.rows);
    for value in template.glyph_grid.y_up_origin_points {
        hash.f32(value)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    for value in template.glyph_grid.cell_extent_points {
        hash.f32(value)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    hash.u8(match template.glyph_grid.scale {
        super::super::LogicalGlyphScale::OneCell => 1,
    });
    hash.u8(match template.glyph_grid.anchor {
        super::super::LogicalGlyphAnchor::CellBottomLeft => 1,
    });
    let mut nodes = template.nodes.iter().collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.alias.cmp(&right.alias));
    hash.usize(nodes.len());
    for node in nodes {
        hash.string(node.alias.as_str());
        hash.u32(node.id.0);
        match node.parent {
            Some(parent) => {
                hash.u8(1);
                hash.u32(parent.0);
            }
            None => hash.u8(0),
        }
        encode_transform(&mut hash, node.base_transform)?;
        encode_bounds(&mut hash, node.local_bounds)?;
        encode_depth_cue(&mut hash, node.depth_cue)?;
    }
    let mut materials = template.materials.iter().collect::<Vec<_>>();
    materials.sort_by(|left, right| left.alias.cmp(&right.alias));
    hash.usize(materials.len());
    for material in materials {
        hash.string(material.alias.as_str());
        hash.u32(material.id.0);
        hash.u8(material_kind_tag(material.kind));
    }
    let mut resources = template.resources.iter().collect::<Vec<_>>();
    resources.sort_by(|left, right| left.alias.cmp(&right.alias));
    hash.usize(resources.len());
    for resource in resources {
        hash.string(resource.alias.as_str());
        hash.u32(resource.id.0);
        hash.u8(resource_kind_tag(resource.kind));
    }
    let mut primitives = template.primitives.iter().collect::<Vec<_>>();
    primitives.sort_by_key(|primitive| primitive.authored_order);
    hash.usize(primitives.len());
    for primitive in primitives {
        hash.u32(primitive.node.0);
        hash.u8(primitive_kind_tag(primitive.kind));
        hash.u32(primitive.material.0);
        match primitive.resource {
            Some(resource) => {
                hash.u8(1);
                hash.u32(resource.0);
            }
            None => hash.u8(0),
        }
        hash.u8(world_blend_tag(primitive.blend));
        hash.u8(depth_behavior_tag(primitive.depth));
        hash.u16(primitive.authored_order);
        hash.u8(primitive_space_tag(primitive.space));
        encode_primitive_binding(&mut hash, primitive.binding);
        encode_bounds(&mut hash, primitive.local_geometry)?;
    }
    hash.usize(template.static_atlas_recipes.len());
    for slot in &template.static_atlas_recipes {
        hash.u8(slot.id.0);
        match slot.recipe {
            Some(recipe) => {
                hash.u8(1);
                hash.u8(static_atlas_semantic_tag(recipe.semantic));
                hash.u32(recipe.source.0);
                encode_bounds(&mut hash, recipe.local_bounds)?;
            }
            None => hash.u8(0),
        }
    }
    hash.usize(template.analytic_templates.len());
    for slot in &template.analytic_templates {
        hash.u8(slot.id.0);
        match slot.value {
            Some(value) => {
                hash.u8(1);
                hash.u8(analytic_semantic_tag(value.semantic));
                hash.u8(analytic_shape_tag(value.shape));
                encode_bounds(&mut hash, value.normalized_local_bounds)?;
            }
            None => hash.u8(0),
        }
    }
    let mut attachments = template.attachments.iter().collect::<Vec<_>>();
    attachments.sort_by(|left, right| left.alias.cmp(&right.alias));
    hash.usize(attachments.len());
    for attachment in attachments {
        hash.string(attachment.alias.as_str());
        hash.u32(attachment.id.0);
        hash.u32(attachment.owner.0);
        encode_transform(&mut hash, attachment.local)?;
        hash.u8(attachment_mode_tag(attachment.mode));
        match attachment.instance_binding {
            None => hash.u8(0),
            Some(AttachmentInstanceBinding::PropGlyphs(slot)) => {
                hash.u8(1);
                hash.u8(slot);
            }
        }
    }
    let privacy = template.privacy;
    hash.u8(presentation_surface_tag(privacy.surface));
    hash.bool(privacy.source_names_visible);
    hash.bool(privacy.exact_counts_visible);
    hash.bool(privacy.diagnostic_text_visible);
    hash.bool(privacy.feed_rows_visible);
    hash.bool(privacy.file_paths_visible);
    hash.bool(privacy.project_names_visible);
    Ok(hash.finish())
}

pub(super) fn checksum_content(content: &SceneContent) -> Result<u64, SceneGenerationError> {
    let mut hash = Fnv1a64::domain(b"glorp.scene-content.v2\0");
    hash.u16(content.schema_version);
    hash.u16(content.renderer_schema_version);
    for color in content.palette {
        for channel in color {
            hash.u8(channel);
        }
    }
    hash.u8(mood_content_tag(content.mood));
    hash.u8(weather_content_tag(content.weather));
    hash.u8(day_phase_tag(content.day_phase));
    for slot in &content.pet_art_slots {
        hash.u16(slot.slot);
        match slot.glyph {
            Some(glyph) => {
                hash.u8(1);
                hash.glyph(glyph.as_char());
            }
            None => hash.u8(0),
        }
        hash.u8(pet_palette_role_tag(slot.palette_role));
    }
    for slot in &content.room_glyph_slots {
        hash.u8(slot.slot);
        match (slot.glyph, slot.color_srgb8) {
            (Some(glyph), Some(color)) => {
                hash.u8(1);
                hash.glyph(glyph.as_char());
                for channel in color {
                    hash.u8(channel);
                }
            }
            _ => hash.u8(0),
        }
    }
    for slot in &content.prop_slots {
        hash.u8(slot.slot);
        match slot.content {
            Some(value) => {
                hash.u8(1);
                encode_prop_content(&mut hash, value);
            }
            None => hash.u8(0),
        }
    }
    for slot in &content.tank_slots {
        hash.u8(slot.slot);
        match slot.content {
            Some(value) => {
                hash.u8(1);
                encode_tank_content(&mut hash, value);
            }
            None => hash.u8(0),
        }
    }
    for slot in &content.ambient_slots {
        hash.u8(slot.slot);
        match slot.kind {
            Some(kind) => {
                hash.u8(1);
                hash.u8(ambient_content_tag(kind));
            }
            None => hash.u8(0),
        }
        match slot.glyph {
            Some(glyph) => {
                hash.u8(1);
                hash.glyph(glyph.as_char());
            }
            None => hash.u8(0),
        }
    }
    for slot in &content.prop_paint_slots {
        hash.u8(slot.slot);
        for paint in slot.paints {
            match paint {
                Some(paint) => {
                    hash.u8(1);
                    encode_rgb(&mut hash, paint.color_srgb8);
                }
                None => hash.u8(0),
            }
        }
    }
    for slot in &content.ambient_paint_slots {
        hash.u8(slot.slot);
        match slot.paint {
            Some(paint) => {
                hash.u8(1);
                encode_rgb(&mut hash, paint.color_srgb8);
            }
            None => hash.u8(0),
        }
    }
    for slot in &content.analytic_slots {
        hash.u8(slot.id.0);
        match slot.value {
            Some(value) => {
                hash.u8(1);
                hash.u8(analytic_semantic_tag(value.semantic));
                hash.u8(analytic_shape_tag(value.shape));
                encode_analytic_paint(&mut hash, value.paint);
            }
            None => hash.u8(0),
        }
    }
    Ok(hash.finish())
}

pub(crate) fn checksum_content_for_capture(
    content: &SceneContent,
) -> Result<u64, SceneGenerationError> {
    checksum_content(content)
}

pub(super) fn checksum_frame(
    template: &SceneTemplate,
    frame: &SceneFrame,
) -> Result<u64, SceneGenerationError> {
    checksum_frame_with_projection(template, frame, FrameChecksumProjection::InternalExact)
}

impl SceneFrame {
    pub(crate) fn capture_source_checksum(
        &self,
        template: &SceneTemplate,
    ) -> Result<u64, SceneGenerationError> {
        checksum_frame_with_projection(template, self, FrameChecksumProjection::CapturePrivacy)
    }
}

#[derive(Clone, Copy)]
enum FrameChecksumProjection {
    InternalExact,
    CapturePrivacy,
}

fn checksum_frame_with_projection(
    template: &SceneTemplate,
    frame: &SceneFrame,
    projection: FrameChecksumProjection,
) -> Result<u64, SceneGenerationError> {
    let mut hash = Fnv1a64::domain(match projection {
        FrameChecksumProjection::InternalExact => b"glorp.scene-frame.v2\0",
        FrameChecksumProjection::CapturePrivacy => b"glorp.capture-frame.v1\0",
    });
    let capture_privacy = matches!(projection, FrameChecksumProjection::CapturePrivacy)
        .then(|| CaptureFramePrivacyProjection::from_frame(frame));
    hash.u16(frame.schema_version);
    hash.u16(frame.renderer_schema_version);
    hash.f32(frame.camera.width_points)
        .map_err(|_| SceneGenerationError::NonFinite)?;
    hash.f32(frame.camera.height_points)
        .map_err(|_| SceneGenerationError::NonFinite)?;
    hash.f32(frame.camera.far_z)
        .map_err(|_| SceneGenerationError::NonFinite)?;
    hash.f32(frame.camera.near_z)
        .map_err(|_| SceneGenerationError::NonFinite)?;
    let mut previous_alias: Option<&CanonicalAlias> = None;
    for _ in 0..template.nodes.len() {
        let template_node = template
            .nodes
            .iter()
            .filter(|node| previous_alias.is_none_or(|previous| node.alias > *previous))
            .min_by(|left, right| left.alias.cmp(&right.alias))
            .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
        let node = frame
            .nodes
            .iter()
            .find(|node| node.node == template_node.id)
            .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
        hash.u32(node.node.0);
        encode_transform(&mut hash, node.local_transform)?;
        let (visible, opacity) = capture_privacy.map_or((node.visible, node.opacity), |privacy| {
            privacy.node_state(template_node.alias.as_str(), node.visible, node.opacity)
        });
        hash.bool(visible);
        hash.f32(opacity)
            .map_err(|_| SceneGenerationError::NonFinite)?;
        previous_alias = Some(&template_node.alias);
    }
    for slot in &frame.prop_slots {
        hash.u8(slot.slot);
        hash.bool(slot.visible);
        encode_points(&mut hash, slot.origin_points)?;
        encode_points(&mut hash, slot.motion_offset_points)?;
        hash.f32(slot.opacity)
            .map_err(|_| SceneGenerationError::NonFinite)?;
        encode_points(&mut hash, slot.footprint_points)?;
        hash.f32(slot.contact_shadow_strength)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    for slot in &frame.room_glyph_slots {
        hash.u8(slot.slot);
        hash.bool(slot.visible);
        hash.u16(slot.grid_cell[0]);
        hash.u16(slot.grid_cell[1]);
        encode_points(&mut hash, slot.position_points)?;
        hash.f32(slot.opacity)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    for slot in &frame.tank_slots {
        hash.u8(slot.slot);
        hash.bool(slot.visible);
        encode_points(&mut hash, slot.origin_points)?;
        for cell in slot.cells {
            hash.bool(cell.visible);
            encode_points(&mut hash, cell.position_points)?;
            hash.u8(instance_layer_tag(cell.layer));
            for value in cell.bounds_points {
                hash.f32(value)
                    .map_err(|_| SceneGenerationError::NonFinite)?;
            }
        }
    }
    for slot in &frame.ambient_slots {
        hash.u8(slot.slot);
        hash.bool(slot.visible);
        encode_points(&mut hash, slot.position_points)?;
        hash.f32(slot.opacity)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    for slot in &frame.analytic_slots {
        hash.u8(slot.id.0);
        match slot.value {
            Some(value) => {
                hash.u8(1);
                hash.u8(analytic_semantic_tag(value.semantic));
                hash.u8(analytic_shape_tag(value.shape));
                for component in value.rect_points {
                    hash.f32(component)
                        .map_err(|_| SceneGenerationError::NonFinite)?;
                }
                encode_analytic_geometry(&mut hash, value.geometry)?;
            }
            None => hash.u8(0),
        }
    }
    match capture_privacy {
        Some(privacy) => {
            for gauge in privacy.gauges() {
                hash.u8(gauge_level_tag(gauge));
            }
        }
        None => {
            for gauge in frame.gauges {
                hash.f32(gauge)
                    .map_err(|_| SceneGenerationError::NonFinite)?;
            }
        }
    }
    match capture_privacy {
        Some(privacy) => hash.bool(privacy.dimmed()),
        None => hash
            .f32(frame.dim_amount)
            .map_err(|_| SceneGenerationError::NonFinite)?,
    }
    hash.usize(frame.lights.len());
    for light in &frame.lights {
        for value in light.direction {
            hash.f32(value)
                .map_err(|_| SceneGenerationError::NonFinite)?;
        }
        for value in light.color_linear {
            hash.f32(value)
                .map_err(|_| SceneGenerationError::NonFinite)?;
        }
        hash.f32(light.intensity)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    Ok(hash.finish())
}

fn gauge_level_tag(level: super::super::GaugeLevelSnapshot) -> u8 {
    match level {
        super::super::GaugeLevelSnapshot::Empty => 0,
        super::super::GaugeLevelSnapshot::Low => 1,
        super::super::GaugeLevelSnapshot::Medium => 2,
        super::super::GaugeLevelSnapshot::High => 3,
        super::super::GaugeLevelSnapshot::Full => 4,
    }
}

fn day_phase_tag(phase: super::super::CompanionDayPhase) -> u8 {
    match phase {
        super::super::CompanionDayPhase::Dawn => 0,
        super::super::CompanionDayPhase::Day => 1,
        super::super::CompanionDayPhase::Dusk => 2,
        super::super::CompanionDayPhase::Night => 3,
    }
}

fn encode_transform(hash: &mut Fnv1a64, transform: Transform3) -> Result<(), SceneGenerationError> {
    for value in transform
        .translation
        .into_iter()
        .chain(transform.rotation_xyzw)
        .chain(transform.scale)
        .chain(transform.pivot)
    {
        hash.f32(value)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    Ok(())
}
fn encode_bounds(hash: &mut Fnv1a64, bounds: Bounds3) -> Result<(), SceneGenerationError> {
    for value in bounds.min.into_iter().chain(bounds.max) {
        hash.f32(value)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    Ok(())
}
fn encode_depth_cue(hash: &mut Fnv1a64, cue: DepthCue) -> Result<(), SceneGenerationError> {
    for value in [
        cue.scale,
        cue.y_offset_points_up,
        cue.opacity,
        cue.saturation,
    ] {
        hash.f32(value)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    Ok(())
}
fn encode_points(hash: &mut Fnv1a64, points: [f32; 2]) -> Result<(), SceneGenerationError> {
    for value in points {
        hash.f32(value)
            .map_err(|_| SceneGenerationError::NonFinite)?;
    }
    Ok(())
}
fn encode_rgb(hash: &mut Fnv1a64, color: [u8; 3]) {
    for channel in color {
        hash.u8(channel);
    }
}
fn encode_rgba(hash: &mut Fnv1a64, color: [u8; 4]) {
    for channel in color {
        hash.u8(channel);
    }
}
fn encode_analytic_paint(hash: &mut Fnv1a64, paint: AnalyticPaint) {
    match paint {
        AnalyticPaint::ApertureDepth { core_srgb8, rim_srgb8 } => {
            hash.u8(1);
            encode_rgb(hash, core_srgb8);
            encode_rgb(hash, rim_srgb8);
        }
        AnalyticPaint::PetShadowMultiply { color_srgb8, opacity_u8 } => {
            hash.u8(2);
            encode_rgb(hash, color_srgb8);
            hash.u8(opacity_u8);
        }
        AnalyticPaint::FloorShadowMultiplyRadial { inner_srgba8, outer_srgba8 } => {
            hash.u8(3);
            encode_rgba(hash, inner_srgba8);
            encode_rgba(hash, outer_srgba8);
        }
        AnalyticPaint::StatusBeacon { active_srgba8, calm_srgba8 } => {
            hash.u8(4);
            encode_rgba(hash, active_srgba8);
            encode_rgba(hash, calm_srgba8);
        }
        AnalyticPaint::MoodAuraRings {
            color_srgb8,
            ring_count,
            per_ring_alpha_u8,
        } => {
            hash.u8(5);
            encode_rgb(hash, color_srgb8);
            hash.u8(ring_count);
            hash.u8(per_ring_alpha_u8);
        }
        AnalyticPaint::PerimeterGaugeSet { xp, daily, pace, daily_overage_srgba8 } => {
            hash.u8(6);
            for lane in [xp, daily, pace] {
                encode_rgba(hash, lane.track_srgba8);
                encode_rgba(hash, lane.fill_srgba8);
            }
            encode_rgba(hash, daily_overage_srgba8);
        }
        AnalyticPaint::TroubleBeacon { color_srgba8 } => {
            hash.u8(7);
            encode_rgba(hash, color_srgba8);
        }
        AnalyticPaint::DimOverlay { color_srgb8 } => {
            hash.u8(8);
            encode_rgb(hash, color_srgb8);
        }
    }
}
fn encode_analytic_geometry(
    hash: &mut Fnv1a64,
    geometry: AnalyticGeometry,
) -> Result<(), SceneGenerationError> {
    match geometry {
        AnalyticGeometry::ApertureRadial {
            center_points,
            radius_points,
            feather_points,
        } => {
            hash.u8(1);
            encode_points(hash, center_points)?;
            hash.f32(radius_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
            hash.f32(feather_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
        }
        AnalyticGeometry::PetSilhouette {
            mask: AnalyticMaskSource::PetBody,
            offset_points,
            softness_points,
        } => {
            hash.u8(2);
            hash.u8(1);
            encode_points(hash, offset_points)?;
            hash.f32(softness_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
        }
        AnalyticGeometry::RadialEllipse {
            center_points,
            radii_points,
            softness_points,
        } => {
            hash.u8(3);
            encode_points(hash, center_points)?;
            encode_points(hash, radii_points)?;
            hash.f32(softness_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
        }
        AnalyticGeometry::StatusBeacon {
            center_points,
            radius_points,
            thickness_points,
            tone,
        } => {
            hash.u8(4);
            encode_points(hash, center_points)?;
            hash.f32(radius_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
            hash.f32(thickness_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
            hash.u8(match tone {
                StatusBeaconTone::Active => 1,
                StatusBeaconTone::Calm => 2,
            });
        }
        AnalyticGeometry::PetAura {
            center_points,
            max_radius_points,
            ring_count,
            feather_points,
        } => {
            hash.u8(5);
            encode_points(hash, center_points)?;
            hash.f32(max_radius_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
            hash.u8(ring_count);
            hash.f32(feather_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
        }
        AnalyticGeometry::PerimeterGaugeSet { center_points, xp, daily, pace } => {
            hash.u8(6);
            encode_points(hash, center_points)?;
            for lane in [xp, daily, pace] {
                hash.f32(lane.radius_points)
                    .map_err(|_| SceneGenerationError::NonFinite)?;
                hash.f32(lane.stroke_width_points)
                    .map_err(|_| SceneGenerationError::NonFinite)?;
                hash.f32(lane.track_start_degrees)
                    .map_err(|_| SceneGenerationError::NonFinite)?;
                hash.f32(lane.track_sweep_degrees)
                    .map_err(|_| SceneGenerationError::NonFinite)?;
                hash.u8(match lane.cap {
                    GaugeLineCap::Round => 1,
                });
            }
        }
        AnalyticGeometry::TroubleBeacon {
            center_points,
            radius_points,
            thickness_points,
        } => {
            hash.u8(7);
            encode_points(hash, center_points)?;
            hash.f32(radius_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
            hash.f32(thickness_points)
                .map_err(|_| SceneGenerationError::NonFinite)?;
        }
        AnalyticGeometry::SurfaceOverlay => hash.u8(8),
    }
    Ok(())
}
fn static_atlas_semantic_tag(value: StaticAtlasSemantic) -> u8 {
    match value {
        StaticAtlasSemantic::DecorativeSprite => 1,
    }
}
fn analytic_semantic_tag(value: AnalyticSemantic) -> u8 {
    value.id().0 + 1
}
fn analytic_shape_tag(value: AnalyticShape) -> u8 {
    match value {
        AnalyticShape::ApertureRadial => 1,
        AnalyticShape::PetSilhouette => 2,
        AnalyticShape::RadialEllipse => 3,
        AnalyticShape::StatusBeacon => 4,
        AnalyticShape::PetAura => 5,
        AnalyticShape::PerimeterGaugeSet => 6,
        AnalyticShape::TroubleBeacon => 7,
        AnalyticShape::SurfaceOverlay => 8,
    }
}
fn encode_primitive_binding(hash: &mut Fnv1a64, binding: PrimitiveBinding) {
    match binding {
        PrimitiveBinding::ShallowCard => hash.u8(0),
        PrimitiveBinding::Analytic(id) => {
            hash.u8(1);
            hash.u8(id.0);
        }
        PrimitiveBinding::StaticAtlas(id) => {
            hash.u8(2);
            hash.u8(id.0);
        }
        PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs) => hash.u8(3),
        PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body)) => hash.u8(4),
        PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Particles)) => {
            hash.u8(5)
        }
        PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(slot)) => {
            hash.u8(6);
            hash.u8(slot);
        }
        PrimitiveBinding::Instances(InstanceGroupBinding::TankCells { slot, layer }) => {
            hash.u8(7);
            hash.u8(slot);
            hash.u8(instance_layer_tag(layer));
        }
        PrimitiveBinding::Instances(InstanceGroupBinding::Ambient) => hash.u8(8),
        PrimitiveBinding::Instances(InstanceGroupBinding::Hud) => hash.u8(9),
    }
}
fn encode_prop_content(hash: &mut Fnv1a64, value: PropSemanticContent) {
    encode_option_u8(hash, value.sprite_phase);
    encode_option_bool(hash, value.twinkle_active);
    encode_option_bool(hash, value.lid_open);
    encode_option_bool(hash, value.bloom_active);
    for glyph in value.glyphs {
        match glyph.glyph {
            Some(value) => {
                hash.u8(1);
                hash.glyph(value.as_char());
            }
            None => hash.u8(0),
        }
        hash.i8(glyph.local_cell[0]);
        hash.i8(glyph.local_cell[1]);
    }
}
fn encode_tank_content(hash: &mut Fnv1a64, value: TankSemanticContent) {
    hash.u8(value.sprite_variant);
    encode_option_u8(hash, value.morph);
    for channel in value.color_srgb8 {
        hash.u8(channel);
    }
    hash.u8(u8::from(value.bold));
    for glyph in value.glyphs {
        match glyph {
            Some(value) => {
                hash.u8(1);
                hash.glyph(value.as_char());
            }
            None => hash.u8(0),
        }
    }
}
fn encode_option_u8(hash: &mut Fnv1a64, value: Option<u8>) {
    match value {
        Some(value) => {
            hash.u8(1);
            hash.u8(value);
        }
        None => hash.u8(0),
    }
}
fn encode_option_bool(hash: &mut Fnv1a64, value: Option<bool>) {
    match value {
        Some(value) => {
            hash.u8(1);
            hash.bool(value);
        }
        None => hash.u8(0),
    }
}
