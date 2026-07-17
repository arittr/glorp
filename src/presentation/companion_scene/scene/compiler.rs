use super::checksum::{checksum_content, checksum_frame, checksum_template};
use super::*;

impl SceneGenerationData {
    #[cfg(test)]
    pub(crate) fn delta_capacities(&self) -> [usize; 15] {
        [
            self.delta_scratch.content.pet_art_slots.capacity(),
            self.delta_scratch.content.room_glyph_slots.capacity(),
            self.delta_scratch.content.prop_slots.capacity(),
            self.delta_scratch.content.tank_slots.capacity(),
            self.delta_scratch.content.ambient_slots.capacity(),
            self.delta_scratch.content.prop_paint_slots.capacity(),
            self.delta_scratch.content.ambient_paint_slots.capacity(),
            self.delta_scratch.content.analytic_slots.capacity(),
            self.delta_scratch.frame.nodes.capacity(),
            self.delta_scratch.frame.room_glyph_slots.capacity(),
            self.delta_scratch.frame.prop_slots.capacity(),
            self.delta_scratch.frame.tank_slots.capacity(),
            self.delta_scratch.frame.ambient_slots.capacity(),
            self.delta_scratch.frame.analytic_slots.capacity(),
            self.delta_scratch.frame.lights.capacity(),
        ]
    }

    #[cfg(test)]
    pub(crate) fn delta_storage_pointers(&self) -> [usize; 15] {
        [
            self.delta_scratch.content.pet_art_slots.as_ptr() as usize,
            self.delta_scratch.content.room_glyph_slots.as_ptr() as usize,
            self.delta_scratch.content.prop_slots.as_ptr() as usize,
            self.delta_scratch.content.tank_slots.as_ptr() as usize,
            self.delta_scratch.content.ambient_slots.as_ptr() as usize,
            self.delta_scratch.content.prop_paint_slots.as_ptr() as usize,
            self.delta_scratch.content.ambient_paint_slots.as_ptr() as usize,
            self.delta_scratch.content.analytic_slots.as_ptr() as usize,
            self.delta_scratch.frame.nodes.as_ptr() as usize,
            self.delta_scratch.frame.room_glyph_slots.as_ptr() as usize,
            self.delta_scratch.frame.prop_slots.as_ptr() as usize,
            self.delta_scratch.frame.tank_slots.as_ptr() as usize,
            self.delta_scratch.frame.ambient_slots.as_ptr() as usize,
            self.delta_scratch.frame.analytic_slots.as_ptr() as usize,
            self.delta_scratch.frame.lights.as_ptr() as usize,
        ]
    }

    #[allow(dead_code)] // Task 4/5 seam consumed by the fixed-mirror compiler in Task 8.
    pub(crate) fn project_snapshot_changes(
        &mut self,
        snapshot: &Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
        changes: crate::presentation::companion_scene::runtime::SnapshotChangeSet,
        from: crate::presentation::companion_scene::AppliedRevisions,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> Result<&SceneDeltaScratch, SceneGenerationError> {
        validate_builder_snapshot(snapshot)?;
        let content = &mut self.delta_scratch.content;
        content.generation_key = self.generation_key;
        content.from = from;
        content.to = to;
        content.palette = None;
        content.mood = None;
        content.weather = None;
        content.day_phase = None;
        content.pet_art_slots.clear();
        content.room_glyph_slots.clear();
        content.prop_slots.clear();
        content.tank_slots.clear();
        content.ambient_slots.clear();
        content.prop_paint_slots.clear();
        content.ambient_paint_slots.clear();
        content.analytic_slots.clear();
        let frame = &mut self.delta_scratch.frame;
        frame.generation_key = self.generation_key;
        frame.from = from;
        frame.to = to;
        frame.camera = None;
        frame.nodes.clear();
        frame.room_glyph_slots.clear();
        frame.prop_slots.clear();
        frame.tank_slots.clear();
        frame.ambient_slots.clear();
        frame.analytic_slots.clear();
        frame.gauges = None;
        frame.dim_amount = None;
        frame.lights.clear();

        let semantic = changes.semantic();
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::PALETTE)
        {
            let palette = snapshot.content.palette;
            content.palette = Some([
                palette.body,
                palette.body_glow,
                palette.eye,
                palette.mouth,
                palette.accent,
                palette.pattern,
                palette.particle,
                palette.corruption,
            ]);
        }
        if semantic.contains(
            crate::presentation::companion_scene::runtime::SemanticChangeMask::MOOD_WEATHER,
        ) {
            content.mood = Some(match snapshot.content.mood {
                crate::game::metabolism::Mood::Happy => MoodContentKind::Happy,
                crate::game::metabolism::Mood::Ecstatic => MoodContentKind::Ecstatic,
                crate::game::metabolism::Mood::Content => MoodContentKind::Content,
                crate::game::metabolism::Mood::Hungry => MoodContentKind::Hungry,
                crate::game::metabolism::Mood::Sad => MoodContentKind::Sad,
                crate::game::metabolism::Mood::Sleepy => MoodContentKind::Sleepy,
                crate::game::metabolism::Mood::Wilted => MoodContentKind::Wilted,
            });
            content.weather = Some(weather_content(snapshot.content.room_weather)?);
            content.day_phase = Some(snapshot.content.day_phase);
        }
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::PALETTE)
            || semantic.contains(
                crate::presentation::companion_scene::runtime::SemanticChangeMask::MOOD_WEATHER,
            )
            || semantic
                .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::TANK)
        {
            project_analytic_content_for_snapshot(snapshot, &mut content.analytic_slots);
        }
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::PET_ART)
        {
            project_pet_delta(snapshot, &mut content.pet_art_slots)?;
        }
        if semantic.contains(
            crate::presentation::companion_scene::runtime::SemanticChangeMask::ROOM_GLYPHS,
        ) {
            project_room_content_delta(snapshot, &mut content.room_glyph_slots)?;
        }
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::PROP)
        {
            project_prop_delta(snapshot, &mut content.prop_slots)?;
            project_prop_paint_delta(snapshot, &mut content.prop_paint_slots)?;
        }
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::TANK)
        {
            project_tank_delta(snapshot, &mut content.tank_slots)?;
        }
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::AMBIENT)
        {
            for source in &snapshot.content.ambient_semantics {
                content.ambient_slots.push(AmbientContentSlot {
                    slot: source.slot,
                    kind: source.kind.map(|kind| match kind {
                        crate::presentation::companion_scene::AmbientSemanticKindSnapshot::Mote => {
                            AmbientContentKind::Mote
                        }
                    }),
                    glyph: source
                        .glyph
                        .map(AuthoredGlyph::new)
                        .transpose()
                        .map_err(|_| SceneGenerationError::InvalidGlyph)?,
                });
            }
            project_ambient_paint_delta(snapshot, &mut content.ambient_paint_slots);
        }
        let frame_mask = changes.frame();
        if frame_mask
            .contains(crate::presentation::companion_scene::runtime::FrameChangeMask::CAMERA)
        {
            frame.camera = Some(
                OrthographicCamera::new(
                    snapshot.topology.layout.width_points,
                    snapshot.topology.layout.height_points,
                    -2.0,
                    2.0,
                )
                .map_err(|_| SceneGenerationError::NonFinite)?,
            );
        }
        if frame_mask
            .contains(crate::presentation::companion_scene::runtime::FrameChangeMask::PET_TRANSFORM)
        {
            let pet_id = self
                .template
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "pet")
                .map(|node| node.id)
                .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
            let node = self
                .frame
                .nodes
                .iter()
                .find(|node| node.node == pet_id)
                .copied()
                .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
            frame.nodes.push(NodeFrameState {
                local_transform: pet_transform(snapshot),
                ..node
            });
            if snapshot.frame.pet_depth != self.source_snapshot.frame.pet_depth
                || snapshot.frame.pet_depth_cue != self.source_snapshot.frame.pet_depth_cue
            {
                project_depth_effect_node_deltas(
                    snapshot,
                    &self.template,
                    &self.frame,
                    &mut frame.nodes,
                )?;
            }
            if snapshot.frame.pet_depth_cue != self.source_snapshot.frame.pet_depth_cue
                && !frame_mask.contains(
                    crate::presentation::companion_scene::runtime::FrameChangeMask::STATUS_VISIBILITY,
                )
            {
                project_pet_attached_node_deltas(
                    snapshot,
                    &self.template,
                    &self.frame,
                    &mut frame.nodes,
                )?;
            }
        }
        if frame_mask
            .contains(crate::presentation::companion_scene::runtime::FrameChangeMask::ROOM_GLYPHS)
        {
            project_room_frame_delta(snapshot, &mut frame.room_glyph_slots);
        }
        if frame_mask.contains(
            crate::presentation::companion_scene::runtime::FrameChangeMask::PROP_TRANSFORMS,
        ) {
            project_prop_frame_delta(snapshot, &mut frame.prop_slots);
        }
        if frame_mask.contains(
            crate::presentation::companion_scene::runtime::FrameChangeMask::TANK_INSTANCES,
        ) {
            project_tank_frame_delta(snapshot, &mut frame.tank_slots)?;
        }
        if frame_mask.contains(
            crate::presentation::companion_scene::runtime::FrameChangeMask::AMBIENT_INSTANCES,
        ) {
            for source in &snapshot.frame.ambient_instances {
                let occupied = snapshot.content.ambient_semantics[usize::from(source.slot)]
                    .kind
                    .is_some();
                frame.ambient_slots.push(AmbientFrameSlot {
                    slot: source.slot,
                    visible: occupied && source.visible,
                    position_points: if occupied {
                        [
                            source.position_points[0],
                            snapshot.topology.layout.height_points - source.position_points[1],
                        ]
                    } else {
                        [0.0; 2]
                    },
                    opacity: if occupied { source.opacity } else { 0.0 },
                });
            }
        }
        if frame_mask.contains(
            crate::presentation::companion_scene::runtime::FrameChangeMask::STATUS_VISIBILITY,
        ) {
            let (status_visible, status_opacity) =
                super::super::canonical_activity_status(snapshot);
            for name in ["pet.body", "pet.particles", "chrome.status"] {
                let node_id = self
                    .template
                    .nodes
                    .iter()
                    .find(|node| node.alias.as_str() == name)
                    .map(|node| node.id)
                    .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
                let mut node = self
                    .frame
                    .nodes
                    .iter()
                    .find(|node| node.node == node_id)
                    .copied()
                    .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
                match name {
                    "pet.body" => {
                        node.opacity = pet_body_opacity(snapshot);
                    }
                    "pet.particles" => {
                        node.visible = !snapshot.frame.asleep;
                        node.opacity = snapshot.frame.pet_depth_cue.opacity;
                    }
                    "chrome.status" => {
                        node.visible = status_visible;
                        node.opacity = status_opacity;
                    }
                    _ => unreachable!("closed status node set"),
                }
                frame.nodes.push(node);
            }
        }
        if frame_mask.contains(
            crate::presentation::companion_scene::runtime::FrameChangeMask::TROUBLE_VISIBILITY,
        ) {
            let node_id = self
                .template
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "chrome.trouble")
                .map(|node| node.id)
                .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
            let mut node = self
                .frame
                .nodes
                .iter()
                .find(|node| node.node == node_id)
                .copied()
                .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
            node.visible = snapshot.frame.helper_trouble;
            node.opacity = if snapshot.frame.helper_trouble {
                1.0
            } else {
                0.0
            };
            frame.nodes.push(node);
        }
        if frame_mask
            .contains(crate::presentation::companion_scene::runtime::FrameChangeMask::GAUGES)
        {
            frame.gauges = Some(snapshot.frame.gauge_fractions);
        }
        if frame_mask.contains(crate::presentation::companion_scene::runtime::FrameChangeMask::DIM)
        {
            frame.dim_amount = Some(snapshot.frame.dim_amount);
            let dim_id = self
                .template
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "chrome.dim")
                .map(|node| node.id)
                .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
            let mut node = self
                .frame
                .nodes
                .iter()
                .find(|node| node.node == dim_id)
                .copied()
                .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
            node.visible = snapshot.frame.dim_amount > 0.0;
            node.opacity = snapshot.frame.dim_amount;
            frame.nodes.push(node);
        }
        if frame_mask != crate::presentation::companion_scene::runtime::FrameChangeMask::NONE {
            project_analytic_frame_slots(snapshot, &mut frame.analytic_slots)?;
        }
        crate::presentation::companion_scene::validate::validate_content_delta(content)?;
        Ok(&self.delta_scratch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneDeltaApplyError {
    StaleBase,
    IdentityMismatch,
    GenerationRequired,
    Projection(SceneGenerationError),
    Validation(crate::presentation::companion_scene::validate::SceneValidationError),
}

impl SceneGenerationData {
    #[allow(dead_code)] // Task 8 consumes this revision-bound same-generation commit seam.
    pub(crate) fn apply_compatible_snapshot(
        &mut self,
        snapshot: Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
        changes: crate::presentation::companion_scene::runtime::SnapshotChangeSet,
        from: crate::presentation::companion_scene::AppliedRevisions,
        to: crate::presentation::companion_scene::AppliedRevisions,
    ) -> Result<(), SceneDeltaApplyError> {
        if from != self.source_revisions {
            return Err(SceneDeltaApplyError::StaleBase);
        }
        if changes.requires_generation() {
            return Err(SceneDeltaApplyError::GenerationRequired);
        }
        let actual = crate::presentation::companion_scene::runtime::classify_snapshot_changes(
            &self.source_snapshot,
            &snapshot,
        );
        if actual != changes {
            return Err(SceneDeltaApplyError::IdentityMismatch);
        }
        let semantic_changed = changes.semantic()
            != crate::presentation::companion_scene::runtime::SemanticChangeMask::NONE;
        let frame_changed =
            changes.frame() != crate::presentation::companion_scene::runtime::FrameChangeMask::NONE;
        if to.semantic.0 < from.semantic.0
            || to.frame.0 < from.frame.0
            || (semantic_changed && to.semantic == from.semantic)
            || (frame_changed && to.frame == from.frame)
        {
            return Err(SceneDeltaApplyError::IdentityMismatch);
        }
        self.project_snapshot_changes(&snapshot, changes, from, to)
            .map_err(SceneDeltaApplyError::Projection)?;
        crate::presentation::companion_scene::validate::validate_content_delta(
            &self.delta_scratch.content,
        )
        .map_err(SceneDeltaApplyError::Validation)?;
        crate::presentation::companion_scene::validate::validate_content_frame_delta(
            &self.content,
            &self.frame,
            &self.delta_scratch.content,
            &self.delta_scratch.frame,
        )
        .map_err(SceneDeltaApplyError::Validation)?;
        self.accepted
            .apply_frame_delta(&self.delta_scratch.frame)
            .map_err(SceneDeltaApplyError::Validation)?;
        apply_content_delta(&mut self.content, &self.delta_scratch.content);
        apply_frame_delta_unchecked(&mut self.frame, &self.delta_scratch.frame);
        self.content_checksum =
            checksum_content(&self.content).map_err(SceneDeltaApplyError::Projection)?;
        self.frame_checksum = checksum_frame(&self.template, &self.frame)
            .map_err(SceneDeltaApplyError::Projection)?;
        self.source_revisions = to;
        self.source_snapshot = snapshot;
        Ok(())
    }
}

#[allow(dead_code)]
fn project_pet_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<PetArtSlot>,
) -> Result<(), SceneGenerationError> {
    let mut roles = [PetPaletteRole::Body; MAX_PET_ART_SLOTS];
    let mut occupied = [false; MAX_PET_ART_SLOTS];
    for span in &snapshot.content.pet_roles {
        let role = pet_role(span.role).ok_or(SceneGenerationError::InvalidPetRole)?;
        for column in span.start_char..span.end_char {
            let index = usize::from(
                span.line_index * crate::presentation::companion_scene::PET_LATTICE_WIDTH + column,
            );
            if index >= MAX_PET_ART_SLOTS || occupied[index] {
                return Err(SceneGenerationError::OverlappingPetRole);
            }
            occupied[index] = true;
            roles[index] = role;
        }
    }
    for (row, line) in snapshot.content.pet_lines.iter().enumerate() {
        for (column, glyph) in line.chars().enumerate() {
            let index =
                row * crate::presentation::companion_scene::PET_LATTICE_WIDTH as usize + column;
            let glyph = if glyph == ' ' {
                None
            } else {
                Some(
                    PetGlyph::for_species(glyph, snapshot.topology.pet.species)
                        .map_err(|_| SceneGenerationError::InvalidGlyph)?,
                )
            };
            output.push(PetArtSlot {
                slot: index as u16,
                glyph,
                palette_role: if glyph.is_some() {
                    roles[index]
                } else {
                    PetPaletteRole::Body
                },
            });
        }
    }
    Ok(())
}

fn project_room_content_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<RoomGlyphContentSlot>,
) -> Result<(), SceneGenerationError> {
    for slot in 0..MAX_ROOM_GLYPH_SLOTS {
        let source = snapshot.content.room_glyphs.get(slot);
        output.push(RoomGlyphContentSlot {
            slot: slot as u8,
            glyph: source
                .map(|source| AuthoredGlyph::new(source.glyph))
                .transpose()
                .map_err(|_| SceneGenerationError::InvalidGlyph)?,
            color_srgb8: source.map(|source| source.color_srgb8),
        });
    }
    Ok(())
}

fn project_room_frame_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<RoomGlyphFrameSlot>,
) {
    for slot in 0..MAX_ROOM_GLYPH_SLOTS {
        let source = snapshot.frame.room_glyphs.get(slot);
        output.push(RoomGlyphFrameSlot {
            slot: slot as u8,
            visible: source.is_some_and(|source| source.visible),
            grid_cell: source.map_or([0; 2], |source| source.grid_cell),
            position_points: source.map_or([0.0; 2], |source| source.position_points),
            opacity: source.map_or(0.0, |source| source.opacity),
        });
    }
}

#[allow(dead_code)]
fn project_prop_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<PropContentSlot>,
) -> Result<(), SceneGenerationError> {
    for (topology, semantic) in snapshot
        .topology
        .visible_props
        .iter()
        .zip(&snapshot.content.prop_animation_states)
    {
        output.push(PropContentSlot {
            slot: topology.stable_order,
            content: Some(PropSemanticContent {
                sprite_phase: semantic.sprite_phase,
                twinkle_active: semantic.twinkle_active,
                lid_open: semantic.chest_lid_open,
                bloom_active: semantic.bloom_active,
                glyphs: prop_glyphs(
                    topology.catalog_id,
                    snapshot.topology.pet.species,
                    semantic.sprite_phase,
                    semantic.twinkle_active,
                    semantic.chest_lid_open,
                    semantic.bloom_active,
                )?,
            }),
        });
    }
    Ok(())
}

#[allow(dead_code)]
fn project_tank_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<TankContentSlot>,
) -> Result<(), SceneGenerationError> {
    for (topology, semantic) in snapshot
        .topology
        .visible_tank_inhabitants
        .iter()
        .zip(&snapshot.content.tank_animation_states)
    {
        output.push(TankContentSlot {
            slot: topology.stable_order,
            content: Some(tank_semantic_content(topology, semantic)?),
        });
    }
    Ok(())
}

fn tank_semantic_content(
    topology: &crate::presentation::companion_scene::TankTopologySnapshot,
    semantic: &crate::presentation::companion_scene::TankAnimationSnapshot,
) -> Result<TankSemanticContent, SceneGenerationError> {
    if topology.catalog_id != semantic.catalog_id
        || topology.stable_order != semantic.stable_order
        || topology.route != semantic.route
    {
        return Err(SceneGenerationError::UnknownAuthoredIdentity);
    }
    let mut glyphs = [None; MAX_TANK_GLYPHS_PER_SLOT];
    if semantic.cells.len() > MAX_TANK_GLYPHS_PER_SLOT {
        return Err(SceneGenerationError::FixedCapacity);
    }
    for (slot, cell) in semantic.cells.iter().enumerate() {
        glyphs[slot] =
            Some(AuthoredGlyph::new(cell.glyph).map_err(|_| SceneGenerationError::InvalidGlyph)?);
    }
    Ok(TankSemanticContent {
        sprite_variant: semantic.sprite_variant,
        morph: semantic.anemone_morph,
        color_srgb8: semantic.color_srgb8,
        bold: semantic.bold,
        glyphs,
    })
}

#[allow(dead_code)]
fn pet_transform(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> Transform3 {
    // The classic breath offset is a whole-cell 0/1 toggle. Retained motion
    // follows the continuous bob so breath phase changes cannot snap the pet.
    let y = snapshot.frame.pet_anchor_points[1] + snapshot.frame.bob_offset_y_points;
    let cell = snapshot.topology.glyph_grid.cell_extent_points;
    let pet_extent = [
        f32::from(crate::presentation::companion_scene::PET_LATTICE_WIDTH) * cell[0],
        f32::from(crate::presentation::companion_scene::PET_LATTICE_HEIGHT) * cell[1],
    ];
    let mut transform = Transform3::translated([
        snapshot.frame.pet_anchor_points[0],
        snapshot.topology.layout.height_points - y - pet_extent[1]
            + snapshot.frame.pet_depth_cue.y_offset_points_up,
        snapshot.frame.pet_depth,
    ]);
    transform.scale[0] = f32::from(snapshot.frame.facing) * snapshot.frame.pet_depth_cue.scale;
    transform.scale[1] = snapshot.frame.pet_depth_cue.scale;
    transform.pivot = [pet_extent[0] * 0.5, pet_extent[1] * 0.5, 0.0];
    transform
}

fn pet_body_opacity(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> f32 {
    let lifecycle_opacity = if snapshot.frame.asleep { 0.65 } else { 1.0 };
    lifecycle_opacity * snapshot.frame.pet_depth_cue.opacity
}

fn resolved_effective_depth(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> f32 {
    crate::presentation::companion_effects::effective_depth(
        snapshot.frame.pet_depth,
        crate::presentation::companion_effects::depth_lifecycle_scale(
            snapshot.frame.asleep,
            snapshot.frame.calm,
        ),
    )
}

fn floor_projection_opacity(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> f32 {
    let layout = snapshot.topology.layout;
    let metrics = crate::presentation::companion_effects::floor_projection_metrics(
        layout.width_points,
        layout.height_points,
        layout.height_points * 0.76,
        layout.height_points,
        snapshot.frame.pet_anchor_points[0],
        resolved_effective_depth(snapshot),
    )
    .expect("validated companion snapshot produces floor projection metrics");
    f32::from(metrics.alpha) / 235.0
}

fn project_pet_attached_node_deltas(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    template: &SceneTemplate,
    current_frame: &SceneFrame,
    output: &mut Vec<NodeFrameState>,
) -> Result<(), SceneGenerationError> {
    for name in ["pet.body", "pet.particles"] {
        let node_id = template
            .nodes
            .iter()
            .find(|node| node.alias.as_str() == name)
            .map(|node| node.id)
            .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
        let mut node = current_frame
            .nodes
            .iter()
            .find(|node| node.node == node_id)
            .copied()
            .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
        node.opacity = match name {
            "pet.body" => pet_body_opacity(snapshot),
            "pet.particles" => snapshot.frame.pet_depth_cue.opacity,
            _ => unreachable!("closed pet-attached node set"),
        };
        output.push(node);
    }
    Ok(())
}

fn project_depth_effect_node_deltas(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    template: &SceneTemplate,
    current_frame: &SceneFrame,
    output: &mut Vec<NodeFrameState>,
) -> Result<(), SceneGenerationError> {
    let wall_opacity = crate::presentation::companion_effects::wall_shadow_depth_cue(
        resolved_effective_depth(snapshot),
    )
    .strength;
    for (name, opacity) in [
        ("pet.shadow.wall", wall_opacity),
        ("pet.projection.floor", floor_projection_opacity(snapshot)),
    ] {
        let node_id = template
            .nodes
            .iter()
            .find(|node| node.alias.as_str() == name)
            .map(|node| node.id)
            .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
        let mut node = current_frame
            .nodes
            .iter()
            .find(|node| node.node == node_id)
            .copied()
            .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
        node.opacity = opacity;
        output.push(node);
    }
    Ok(())
}

#[allow(dead_code)]
fn project_prop_frame_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<PropFrameSlot>,
) {
    for source in &snapshot.frame.prop_instances {
        output.push(PropFrameSlot {
            slot: source.slot,
            visible: source.visible,
            origin_points: [
                source.origin_points[0],
                prop_origin_y_up(
                    snapshot.topology.layout.height_points,
                    snapshot.topology.glyph_grid.cell_extent_points[1],
                    source.origin_points[1],
                ),
            ],
            motion_offset_points: source.motion_offset_points,
            opacity: source.opacity,
            footprint_points: source.footprint_points,
            contact_shadow_strength: source.contact_shadow_strength,
        });
    }
}

fn prop_origin_y_up(height_points: f32, cell_height_points: f32, top_y_points: f32) -> f32 {
    height_points - top_y_points - cell_height_points
}

#[allow(dead_code)]
fn project_tank_frame_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<TankFrameSlot>,
) -> Result<(), SceneGenerationError> {
    for (semantic, source) in snapshot
        .content
        .tank_animation_states
        .iter()
        .zip(&snapshot.frame.tank_instances)
    {
        let mut cells = [TankCellFrame {
            visible: false,
            position_points: [0.0; 2],
            layer: InstanceLayer::Behind,
            bounds_points: [0.0; 4],
        }; MAX_TANK_GLYPHS_PER_SLOT];
        for (index, (cell, semantic_cell)) in source.cells.iter().zip(&semantic.cells).enumerate() {
            if index >= MAX_TANK_GLYPHS_PER_SLOT {
                return Err(SceneGenerationError::FixedCapacity);
            }
            let bounds = source.bounds_points.unwrap_or([
                cell.position_points[0],
                cell.position_points[1],
                0.0,
                0.0,
            ]);
            cells[index] = TankCellFrame {
                visible: source.visible,
                position_points: [
                    cell.position_points[0],
                    snapshot.topology.layout.height_points - cell.position_points[1],
                ],
                layer: match semantic_cell.layer {
                    crate::presentation::companion_scene::TankLayerSnapshot::Behind => {
                        InstanceLayer::Behind
                    }
                    _ => InstanceLayer::Foreground,
                },
                bounds_points: [
                    bounds[0],
                    snapshot.topology.layout.height_points - bounds[1] - bounds[3],
                    bounds[2],
                    bounds[3],
                ],
            };
        }
        output.push(TankFrameSlot {
            slot: source.slot,
            visible: source.visible,
            origin_points: [
                source.origin_points[0],
                snapshot.topology.layout.height_points - source.origin_points[1],
            ],
            cells,
        });
    }
    Ok(())
}

#[allow(dead_code)] // Used by the Task 8 same-generation commit seam above.
fn apply_content_delta(content: &mut SceneContent, delta: &ContentDelta) {
    if let Some(palette) = delta.palette {
        content.palette = palette;
    }
    if let Some(mood) = delta.mood {
        content.mood = mood;
    }
    if let Some(weather) = delta.weather {
        content.weather = weather;
    }
    if let Some(day_phase) = delta.day_phase {
        content.day_phase = day_phase;
    }
    for changed in &delta.pet_art_slots {
        content.pet_art_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.room_glyph_slots {
        content.room_glyph_slots[usize::from(changed.slot)] = *changed;
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
    for changed in &delta.prop_paint_slots {
        content.prop_paint_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.ambient_paint_slots {
        content.ambient_paint_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.analytic_slots {
        content.analytic_slots[usize::from(changed.id.0)] = *changed;
    }
}

#[allow(dead_code)] // Used only after the Task 3 validator accepts the exact delta.
fn apply_frame_delta_unchecked(frame: &mut SceneFrame, delta: &FrameDelta) {
    if let Some(camera) = delta.camera {
        frame.camera = camera;
    }
    for changed in &delta.nodes {
        if let Some(current) = frame
            .nodes
            .iter_mut()
            .find(|node| node.node == changed.node)
        {
            *current = *changed;
        }
    }
    for changed in &delta.room_glyph_slots {
        frame.room_glyph_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.prop_slots {
        frame.prop_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.tank_slots {
        frame.tank_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.ambient_slots {
        frame.ambient_slots[usize::from(changed.slot)] = *changed;
    }
    for changed in &delta.analytic_slots {
        frame.analytic_slots[usize::from(changed.id.0)] = *changed;
    }
    if let Some(gauges) = delta.gauges {
        frame.gauges = gauges;
    }
    if let Some(dim_amount) = delta.dim_amount {
        frame.dim_amount = dim_amount;
    }
    for (slot, light) in &delta.lights {
        frame.lights[usize::from(*slot)] = *light;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneGenerationError {
    SchemaVersion,
    RendererSchemaVersion,
    FixedCapacity,
    NonFinite,
    InvalidPetRole,
    OverlappingPetRole,
    InvalidGlyph,
    UnknownAuthoredIdentity,
    SnapshotRejected(crate::presentation::companion_scene::runtime::SnapshotRejection),
    Validation(crate::presentation::companion_scene::validate::SceneValidationError),
}

impl From<crate::presentation::companion_scene::validate::SceneValidationError>
    for SceneGenerationError
{
    fn from(value: crate::presentation::companion_scene::validate::SceneValidationError) -> Self {
        Self::Validation(value)
    }
}

pub fn build_scene_generation(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
) -> Result<SceneGenerationData, SceneGenerationError> {
    build_scene_generation_owned(
        Arc::new(snapshot.clone()),
        generation_key,
        crate::presentation::companion_scene::AppliedRevisions::new(0, 0),
    )
}

pub(crate) fn build_scene_generation_owned(
    snapshot: Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    source_revisions: crate::presentation::companion_scene::AppliedRevisions,
) -> Result<SceneGenerationData, SceneGenerationError> {
    build_scene_generation_sealed(snapshot, generation_key, source_revisions, Arc::new(()))
}

pub(crate) fn build_scene_generation_for_request(
    snapshot: Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    request_seal: Arc<()>,
) -> Result<SceneGenerationData, SceneGenerationError> {
    build_scene_generation_sealed(snapshot, generation_key, source_revisions, request_seal)
}

fn build_scene_generation_sealed(
    snapshot: Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    generation_key: crate::presentation::companion_scene::SceneGenerationKey,
    source_revisions: crate::presentation::companion_scene::AppliedRevisions,
    request_seal: Arc<()>,
) -> Result<SceneGenerationData, SceneGenerationError> {
    validate_builder_snapshot(&snapshot)?;
    let layout = snapshot.topology.layout;
    let mut template = build_template(&snapshot)?;
    let content = build_content(&snapshot)?;
    let frame = build_frame(&snapshot, &template)?;
    crate::presentation::companion_scene::validate::validate_full_generation(
        &template, &content, &frame,
    )?;
    template.generation_checksum = checksum_template(&template)?;
    // Acceptance owns a template clone, so create the published proof only
    // after assigning the validated template's intrinsic checksum.
    let accepted = crate::presentation::companion_scene::validate::validate_full_generation(
        &template, &content, &frame,
    )?;
    let content_checksum = checksum_content(&content)?;
    let frame_checksum = checksum_frame(&template, &frame)?;
    debug_assert!(layout.width_points.is_finite());
    Ok(SceneGenerationData {
        generation_key,
        source_revisions,
        source_snapshot: snapshot,
        request_seal,
        template,
        content,
        frame,
        content_checksum,
        frame_checksum,
        delta_scratch: SceneDeltaScratch::fixed_v2(),
        accepted,
    })
}

impl SceneGenerationData {
    #[allow(dead_code)] // Retained CPU candidates keep this bounded validation proof.
    pub(crate) fn accepted_state(&self) -> &super::super::validate::AcceptedSceneState {
        &self.accepted
    }

    pub const fn generation_key(&self) -> crate::presentation::companion_scene::SceneGenerationKey {
        self.generation_key
    }

    pub const fn source_revisions(&self) -> crate::presentation::companion_scene::AppliedRevisions {
        self.source_revisions
    }

    pub fn source_snapshot(
        &self,
    ) -> &Arc<crate::presentation::companion_scene::CompanionSceneSnapshot> {
        &self.source_snapshot
    }

    pub fn template(&self) -> &SceneTemplate {
        &self.template
    }

    pub fn content(&self) -> &SceneContent {
        &self.content
    }

    pub fn frame(&self) -> &SceneFrame {
        &self.frame
    }

    pub const fn content_checksum(&self) -> u64 {
        self.content_checksum
    }

    pub const fn frame_checksum(&self) -> u64 {
        self.frame_checksum
    }

    pub(crate) fn matches_request(
        &self,
        request_seal: &Arc<()>,
        generation_key: crate::presentation::companion_scene::SceneGenerationKey,
        source_revisions: crate::presentation::companion_scene::AppliedRevisions,
        snapshot: &Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    ) -> bool {
        Arc::ptr_eq(&self.request_seal, request_seal)
            && self.generation_key == generation_key
            && self.source_revisions == source_revisions
            && Arc::ptr_eq(&self.source_snapshot, snapshot)
    }
}

fn validate_builder_snapshot(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> Result<(), SceneGenerationError> {
    crate::presentation::companion_scene::runtime::validate_snapshot(snapshot)
        .map_err(SceneGenerationError::SnapshotRejected)?;
    if snapshot.schema_version
        != crate::presentation::companion_scene::COMPANION_SCENE_SCHEMA_VERSION
    {
        return Err(SceneGenerationError::SchemaVersion);
    }
    if snapshot.topology.renderer_schema
        != crate::presentation::companion_scene::COMPANION_RENDERER_SCHEMA_VERSION
    {
        return Err(SceneGenerationError::RendererSchemaVersion);
    }
    let layout = snapshot.topology.layout;
    if !layout.width_points.is_finite()
        || !layout.height_points.is_finite()
        || layout.width_points <= 0.0
        || layout.height_points <= 0.0
        || !snapshot.frame.pet_depth.is_finite()
        || !(-1.0..=1.0).contains(&snapshot.frame.pet_depth)
    {
        return Err(SceneGenerationError::NonFinite);
    }
    if snapshot.topology.pet.lattice.width
        != crate::presentation::companion_scene::PET_LATTICE_WIDTH
        || snapshot.topology.pet.lattice.height
            != crate::presentation::companion_scene::PET_LATTICE_HEIGHT
        || snapshot.topology.pet.lattice.slot_count as usize != MAX_PET_ART_SLOTS
        || snapshot.content.pet_lines.len()
            != crate::presentation::companion_scene::PET_LATTICE_HEIGHT as usize
        || snapshot.content.pet_lines.iter().any(|line| {
            line.chars().count() != crate::presentation::companion_scene::PET_LATTICE_WIDTH as usize
        })
        || snapshot.topology.visible_props.len() > MAX_VISIBLE_PROPS
        || snapshot.topology.visible_tank_inhabitants.len() > MAX_ROUND_TANK_INHABITANTS
        || snapshot.content.ambient_semantics.len() != MAX_AMBIENT_INSTANCES
        || snapshot.frame.ambient_instances.len() != MAX_AMBIENT_INSTANCES
    {
        return Err(SceneGenerationError::FixedCapacity);
    }
    Ok(())
}

pub(super) fn alias(value: impl Into<String>) -> Result<CanonicalAlias, SceneGenerationError> {
    CanonicalAlias::new(value).map_err(|_| SceneGenerationError::UnknownAuthoredIdentity)
}

fn build_template(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> Result<SceneTemplate, SceneGenerationError> {
    let layout = snapshot.topology.layout;
    let mut nodes = Vec::with_capacity(MAX_SCENE_NODES);
    let mut add_node = |name: String,
                        parent: Option<&str>,
                        z: f32,
                        bounds: Bounds3,
                        depth_cue: DepthCue|
     -> Result<NodeId, SceneGenerationError> {
        let node_alias = alias(name)?;
        let id = NodeId::from_alias(&node_alias);
        let parent = parent
            .map(|value| alias(value).map(|value| NodeId::from_alias(&value)))
            .transpose()?;
        nodes.push(NodeTemplate {
            id,
            alias: node_alias,
            parent,
            base_transform: Transform3::translated([0.0, 0.0, z]),
            local_bounds: bounds,
            depth_cue,
        });
        Ok(id)
    };
    let scene_bounds = Bounds3 {
        min: [0.0, 0.0, -2.0],
        max: [layout.width_points, layout.height_points, 2.0],
    };
    let unit_bounds = Bounds3 { min: [0.0; 3], max: [1.0, 1.0, 0.0] };
    for (name, parent, z) in [
        ("scene.root", None, 0.0),
        ("world.far", Some("scene.root"), 0.0),
        ("world.room.background", Some("world.far"), -1.90),
        ("world.room.glyphs", Some("world.far"), -1.75),
        ("pet.projection.floor", Some("world.far"), -1.70),
        ("world.prop.shadows", Some("world.far"), -1.69),
        ("world.ambient", Some("world.far"), -1.65),
        ("world.behind", Some("scene.root"), 0.0),
        ("world.props.behind", Some("world.behind"), 0.0),
        ("world.tank.behind", Some("world.behind"), 0.0),
        ("pet.shadow.wall", Some("world.behind"), -1.30),
        ("pet", Some("scene.root"), 0.0),
        // Aura geometry is already resolved into absolute world point-space.
        // Keeping it under the pet would apply the pet transform twice.
        ("pet.aura.mood", Some("scene.root"), 0.0),
        ("pet.body", Some("pet"), 0.0),
        ("pet.particles", Some("pet"), 0.0),
        ("world.foreground", Some("scene.root"), 0.0),
        ("world.props.foreground", Some("world.foreground"), 0.0),
        ("world.tank.foreground", Some("world.foreground"), 0.0),
        (
            "world.gauge.pace",
            Some("scene.root"),
            crate::round::depth::CompanionGaugeLane::Pace.scene_z(),
        ),
        (
            "world.gauge.daily",
            Some("scene.root"),
            crate::round::depth::CompanionGaugeLane::Daily.scene_z(),
        ),
        (
            "world.gauge.xp",
            Some("scene.root"),
            crate::round::depth::CompanionGaugeLane::Xp.scene_z(),
        ),
        ("chrome.screen", Some("scene.root"), 0.0),
        ("chrome.status", Some("chrome.screen"), 0.0),
        ("chrome.trouble", Some("chrome.screen"), 0.0),
        ("chrome.hud", Some("chrome.screen"), 0.0),
        ("chrome.dim", Some("chrome.screen"), 0.0),
    ] {
        add_node(name.to_owned(), parent, z, scene_bounds, DepthCue::NEUTRAL)?;
    }

    for prop in &snapshot.topology.visible_props {
        let prop_alias = format!("world.prop.{}", prop.catalog_id);
        let parent = match prop.authored_depth {
            crate::presentation::companion_scene::AuthoredDepthSnapshot::Foreground => {
                "world.props.foreground"
            }
            _ => "world.props.behind",
        };
        let z = match prop.authored_depth {
            crate::presentation::companion_scene::AuthoredDepthSnapshot::Foreground => {
                1.20 + f32::from(prop.stable_order) * 0.01
            }
            _ => -1.60 + f32::from(prop.stable_order) * 0.01,
        };
        add_node(
            prop_alias.clone(),
            Some(parent),
            z,
            unit_bounds,
            prop.authored_depth.depth_cue(),
        )?;
        if prop.catalog_id == crate::game::habitat::TOKEN_TREASURE_CHEST_2M {
            add_node(
                format!("{prop_alias}.body"),
                Some(&prop_alias),
                0.0,
                unit_bounds,
                DepthCue::NEUTRAL,
            )?;
            add_node(
                format!("{prop_alias}.lid"),
                Some(&prop_alias),
                0.0,
                unit_bounds,
                DepthCue::NEUTRAL,
            )?;
        }
    }
    for tank in &snapshot.topology.visible_tank_inhabitants {
        let tank_alias = format!("world.tank.{}", tank.catalog_id);
        add_node(
            format!("{tank_alias}.behind"),
            Some("world.tank.behind"),
            -1.45 + f32::from(tank.stable_order) * 0.01,
            unit_bounds,
            crate::presentation::companion_scene::AuthoredDepthSnapshot::BehindPet.depth_cue(),
        )?;
        add_node(
            format!("{tank_alias}.foreground"),
            Some("world.tank.foreground"),
            1.35 + f32::from(tank.stable_order) * 0.01,
            unit_bounds,
            crate::presentation::companion_scene::AuthoredDepthSnapshot::Foreground.depth_cue(),
        )?;
    }

    let material_specs = [
        ("material.unlit-glyph", MaterialKind::UnlitGlyphSprite),
        ("material.unlit-analytic", MaterialKind::UnlitAnalytic),
        ("material.multiply-shadow", MaterialKind::MultiplyShadow),
        ("material.additive-glow", MaterialKind::AdditiveGlow),
        ("material.screen-chrome", MaterialKind::ScreenChrome),
    ];
    let materials = material_specs
        .into_iter()
        .map(|(name, kind)| {
            let alias = alias(name)?;
            Ok(MaterialTemplate {
                id: MaterialId::from_alias(&alias),
                alias,
                kind,
            })
        })
        .collect::<Result<Vec<_>, SceneGenerationError>>()?;
    let room_resource = format!(
        "resource.room.{}.{}.{}.glyph-atlas",
        snapshot.topology.room.primary_biome,
        snapshot.topology.room.secondary_biome.unwrap_or("none"),
        snapshot.topology.room.species_dialect,
    );
    let resource_specs = [
        (
            format!(
                "resource.pet-{}-stage-{}",
                snapshot.topology.pet.species.as_str(),
                snapshot.topology.pet.stage.index()
            ),
            ResourceKind::GlyphAtlas,
        ),
        (room_resource.clone(), ResourceKind::GlyphAtlas),
        (
            "resource.prop-glyph-atlas".to_owned(),
            ResourceKind::GlyphAtlas,
        ),
        (
            "resource.tank-glyph-atlas".to_owned(),
            ResourceKind::GlyphAtlas,
        ),
        (
            "resource.hud-glyph-atlas".to_owned(),
            ResourceKind::GlyphAtlas,
        ),
        (
            "resource.analytic-geometry".to_owned(),
            ResourceKind::AnalyticGeometry,
        ),
    ];
    let resources = resource_specs
        .into_iter()
        .map(|(name, kind)| {
            let alias = alias(name)?;
            Ok(ResourceTemplate {
                id: ResourceId::from_alias(&alias),
                alias,
                kind,
            })
        })
        .collect::<Result<Vec<_>, SceneGenerationError>>()?;
    let material_id = |name: &str| -> Result<MaterialId, SceneGenerationError> {
        let value = alias(name)?;
        Ok(MaterialId::from_alias(&value))
    };
    let resource_id = |name: &str| -> Result<ResourceId, SceneGenerationError> {
        let value = alias(name)?;
        Ok(ResourceId::from_alias(&value))
    };
    let node_id = |name: &str| -> Result<NodeId, SceneGenerationError> {
        let value = alias(name)?;
        Ok(NodeId::from_alias(&value))
    };
    let mut primitives = Vec::new();
    let mut order = 0u16;
    let mut push = |node: &str,
                    kind: PrimitiveKind,
                    material: &str,
                    resource: &str,
                    blend: WorldBlend,
                    depth: DepthBehavior,
                    binding: PrimitiveBinding,
                    space: PrimitiveSpace|
     -> Result<(), SceneGenerationError> {
        primitives.push(PrimitiveTemplate {
            node: node_id(node)?,
            kind,
            material: material_id(material)?,
            resource: Some(resource_id(resource)?),
            blend,
            depth,
            binding,
            authored_order: order,
            local_geometry: unit_bounds,
            space,
        });
        order = order
            .checked_add(1)
            .ok_or(SceneGenerationError::FixedCapacity)?;
        Ok(())
    };
    push(
        "world.room.background",
        PrimitiveKind::AnalyticShape,
        "material.unlit-analytic",
        "resource.analytic-geometry",
        WorldBlend::Opaque,
        DepthBehavior::WorldWrite,
        PrimitiveBinding::Analytic(AnalyticSemantic::RoomBackground.id()),
        PrimitiveSpace::World,
    )?;
    push(
        "world.room.glyphs",
        PrimitiveKind::InstanceQuad,
        "material.unlit-glyph",
        &room_resource,
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs),
        PrimitiveSpace::World,
    )?;
    push(
        "pet.projection.floor",
        PrimitiveKind::AnalyticShape,
        "material.multiply-shadow",
        "resource.analytic-geometry",
        WorldBlend::Multiply,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Analytic(AnalyticSemantic::FloorProjection.id()),
        PrimitiveSpace::World,
    )?;
    push(
        "world.prop.shadows",
        PrimitiveKind::AnalyticShape,
        "material.multiply-shadow",
        "resource.analytic-geometry",
        WorldBlend::Multiply,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Analytic(AnalyticSemantic::PropShadows.id()),
        PrimitiveSpace::World,
    )?;
    push(
        "world.ambient",
        PrimitiveKind::InstanceQuad,
        "material.additive-glow",
        &room_resource,
        WorldBlend::Additive,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Instances(InstanceGroupBinding::Ambient),
        PrimitiveSpace::World,
    )?;
    for prop in snapshot.topology.visible_props.iter().filter(|prop| {
        prop.authored_depth
            != crate::presentation::companion_scene::AuthoredDepthSnapshot::Foreground
    }) {
        push(
            &format!("world.prop.{}", prop.catalog_id),
            PrimitiveKind::InstanceQuad,
            "material.unlit-glyph",
            "resource.prop-glyph-atlas",
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::WorldReadOnly,
            PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(prop.stable_order)),
            PrimitiveSpace::World,
        )?;
    }
    for tank in &snapshot.topology.visible_tank_inhabitants {
        push(
            &format!("world.tank.{}.behind", tank.catalog_id),
            PrimitiveKind::InstanceQuad,
            "material.unlit-glyph",
            "resource.tank-glyph-atlas",
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::WorldReadOnly,
            PrimitiveBinding::Instances(InstanceGroupBinding::TankCells {
                slot: tank.stable_order,
                layer: InstanceLayer::Behind,
            }),
            PrimitiveSpace::World,
        )?;
    }
    push(
        "pet.shadow.wall",
        PrimitiveKind::AnalyticShape,
        "material.unlit-analytic",
        "resource.analytic-geometry",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Analytic(AnalyticSemantic::WallShadow.id()),
        PrimitiveSpace::World,
    )?;
    push(
        "pet.aura.mood",
        PrimitiveKind::AnalyticShape,
        "material.unlit-analytic",
        "resource.analytic-geometry",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Analytic(AnalyticSemantic::MoodAura.id()),
        PrimitiveSpace::World,
    )?;
    let pet_resource = format!(
        "resource.pet-{}-stage-{}",
        snapshot.topology.pet.species.as_str(),
        snapshot.topology.pet.stage.index()
    );
    push(
        "pet.body",
        PrimitiveKind::InstanceQuad,
        "material.unlit-glyph",
        &pet_resource,
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body)),
        PrimitiveSpace::World,
    )?;
    push(
        "pet.particles",
        PrimitiveKind::InstanceQuad,
        "material.additive-glow",
        &pet_resource,
        WorldBlend::Additive,
        DepthBehavior::WorldReadOnly,
        PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Particles)),
        PrimitiveSpace::World,
    )?;
    for prop in snapshot.topology.visible_props.iter().filter(|prop| {
        prop.authored_depth
            == crate::presentation::companion_scene::AuthoredDepthSnapshot::Foreground
    }) {
        push(
            &format!("world.prop.{}", prop.catalog_id),
            PrimitiveKind::InstanceQuad,
            "material.unlit-glyph",
            "resource.prop-glyph-atlas",
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::WorldReadOnly,
            PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(prop.stable_order)),
            PrimitiveSpace::World,
        )?;
    }
    for tank in &snapshot.topology.visible_tank_inhabitants {
        push(
            &format!("world.tank.{}.foreground", tank.catalog_id),
            PrimitiveKind::InstanceQuad,
            "material.unlit-glyph",
            "resource.tank-glyph-atlas",
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::WorldReadOnly,
            PrimitiveBinding::Instances(InstanceGroupBinding::TankCells {
                slot: tank.stable_order,
                layer: InstanceLayer::Foreground,
            }),
            PrimitiveSpace::World,
        )?;
    }
    for (alias, semantic) in [
        ("world.gauge.pace", AnalyticSemantic::GaugePace),
        ("world.gauge.daily", AnalyticSemantic::GaugeDaily),
        ("world.gauge.xp", AnalyticSemantic::GaugeXp),
    ] {
        push(
            alias,
            PrimitiveKind::AnalyticShape,
            "material.unlit-analytic",
            "resource.analytic-geometry",
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::WorldReadOnly,
            PrimitiveBinding::Analytic(semantic.id()),
            PrimitiveSpace::World,
        )?;
    }
    push(
        "chrome.status",
        PrimitiveKind::AnalyticShape,
        "material.screen-chrome",
        "resource.analytic-geometry",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::ScreenNoDepth,
        PrimitiveBinding::Analytic(AnalyticSemantic::StatusHalo.id()),
        PrimitiveSpace::Screen,
    )?;
    push(
        "chrome.trouble",
        PrimitiveKind::AnalyticShape,
        "material.screen-chrome",
        "resource.analytic-geometry",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::ScreenNoDepth,
        PrimitiveBinding::Analytic(AnalyticSemantic::Trouble.id()),
        PrimitiveSpace::Screen,
    )?;
    match crate::round::hud::COMPANION_HUD_DEPTH_PLANE {
        crate::round::hud::CompanionHudDepthPlane::FrontGlass => push(
            "chrome.hud",
            PrimitiveKind::InstanceQuad,
            "material.screen-chrome",
            "resource.hud-glyph-atlas",
            WorldBlend::PremultipliedAlpha,
            DepthBehavior::ScreenNoDepth,
            PrimitiveBinding::Instances(InstanceGroupBinding::Hud),
            PrimitiveSpace::Screen,
        )?,
    }
    push(
        "chrome.dim",
        PrimitiveKind::AnalyticShape,
        "material.screen-chrome",
        "resource.analytic-geometry",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::ScreenNoDepth,
        PrimitiveBinding::Analytic(AnalyticSemantic::Dim.id()),
        PrimitiveSpace::Screen,
    )?;

    let mut attachments = Vec::new();
    if snapshot
        .topology
        .visible_props
        .iter()
        .any(|prop| prop.catalog_id == crate::game::habitat::TOKEN_TREASURE_CHEST_2M)
    {
        let attachment_alias = alias("world.prop.token_treasure_chest_2m.bubble-origin")?;
        let owner_alias = alias("world.prop.token_treasure_chest_2m.lid")?;
        attachments.push(AttachmentTemplate {
            id: AttachmentId::from_alias(&attachment_alias),
            alias: attachment_alias,
            owner: NodeId::from_alias(&owner_alias),
            local: Transform3::translated([0.0, 1.25, 0.0]),
            mode: AttachmentMode::Follow,
            instance_binding: Some(AttachmentInstanceBinding::PropGlyphs(
                snapshot
                    .topology
                    .visible_props
                    .iter()
                    .find(|prop| prop.catalog_id == crate::game::habitat::TOKEN_TREASURE_CHEST_2M)
                    .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?
                    .stable_order,
            )),
        });
    }
    Ok(SceneTemplate {
        schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
        renderer_schema_version:
            crate::presentation::companion_scene::COMPANION_RENDERER_SCHEMA_VERSION,
        capacities: SceneCapacities::FIXED_V2,
        glyph_grid: snapshot.topology.glyph_grid,
        nodes,
        primitives,
        materials,
        resources,
        attachments,
        static_atlas_recipes: empty_static_atlas_recipe_slots(),
        analytic_templates: build_analytic_templates(unit_bounds),
        privacy: snapshot.privacy,
        generation_checksum: 0,
    })
}

fn build_content(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> Result<SceneContent, SceneGenerationError> {
    let mut content = SceneContent::empty_v2();
    content.room_glyph_slots.clear();
    project_room_content_delta(snapshot, &mut content.room_glyph_slots)?;
    let palette = snapshot.content.palette;
    content.palette = [
        palette.body,
        palette.body_glow,
        palette.eye,
        palette.mouth,
        palette.accent,
        palette.pattern,
        palette.particle,
        palette.corruption,
    ];
    content.mood = mood_content(snapshot.content.mood);
    content.weather = weather_content(snapshot.content.room_weather)?;
    content.day_phase = snapshot.content.day_phase;
    project_analytic_content_for_snapshot(snapshot, &mut content.analytic_slots);
    let mut occupied_roles = [false; MAX_PET_ART_SLOTS];
    for span in &snapshot.content.pet_roles {
        if span.line_index >= crate::presentation::companion_scene::PET_LATTICE_HEIGHT
            || span.start_char >= span.end_char
            || span.end_char > crate::presentation::companion_scene::PET_LATTICE_WIDTH
        {
            return Err(SceneGenerationError::InvalidPetRole);
        }
        let role = pet_role(span.role).ok_or(SceneGenerationError::InvalidPetRole)?;
        for column in span.start_char..span.end_char {
            let index = usize::from(
                span.line_index * crate::presentation::companion_scene::PET_LATTICE_WIDTH + column,
            );
            if occupied_roles[index] {
                return Err(SceneGenerationError::OverlappingPetRole);
            }
            occupied_roles[index] = true;
            content.pet_art_slots[index].palette_role = role;
        }
    }
    for (row, line) in snapshot.content.pet_lines.iter().enumerate() {
        for (column, glyph) in line.chars().enumerate() {
            let index =
                row * crate::presentation::companion_scene::PET_LATTICE_WIDTH as usize + column;
            content.pet_art_slots[index].glyph = if glyph == ' ' {
                None
            } else {
                Some(
                    PetGlyph::for_species(glyph, snapshot.topology.pet.species)
                        .map_err(|_| SceneGenerationError::InvalidGlyph)?,
                )
            };
            if content.pet_art_slots[index].glyph.is_none() {
                content.pet_art_slots[index].palette_role = PetPaletteRole::Body;
            }
        }
    }
    for (topology, semantic) in snapshot
        .topology
        .visible_props
        .iter()
        .zip(&snapshot.content.prop_animation_states)
    {
        if topology.catalog_id != semantic.catalog_id
            || topology.stable_order != semantic.stable_order
        {
            return Err(SceneGenerationError::UnknownAuthoredIdentity);
        }
        let glyphs = prop_glyphs(
            topology.catalog_id,
            snapshot.topology.pet.species,
            semantic.sprite_phase,
            semantic.twinkle_active,
            semantic.chest_lid_open,
            semantic.bloom_active,
        )?;
        content.prop_slots[usize::from(topology.stable_order)].content =
            Some(PropSemanticContent {
                sprite_phase: semantic.sprite_phase,
                twinkle_active: semantic.twinkle_active,
                lid_open: semantic.chest_lid_open,
                bloom_active: semantic.bloom_active,
                glyphs,
            });
        content.prop_paint_slots[usize::from(topology.stable_order)] = prop_paint_slot(
            topology.catalog_id,
            topology.stable_order,
            semantic.bloom_active == Some(true),
            glyphs,
        )?;
    }
    for (topology, semantic) in snapshot
        .topology
        .visible_tank_inhabitants
        .iter()
        .zip(&snapshot.content.tank_animation_states)
    {
        content.tank_slots[usize::from(topology.stable_order)].content =
            Some(tank_semantic_content(topology, semantic)?);
    }
    for semantic in &snapshot.content.ambient_semantics {
        let slot = usize::from(semantic.slot);
        if slot >= MAX_AMBIENT_INSTANCES {
            return Err(SceneGenerationError::FixedCapacity);
        }
        if let Some(glyph) = semantic.glyph {
            AuthoredGlyph::new(glyph).map_err(|_| SceneGenerationError::InvalidGlyph)?;
        }
        content.ambient_slots[slot].kind = semantic.kind.map(|kind| match kind {
            crate::presentation::companion_scene::AmbientSemanticKindSnapshot::Mote => {
                AmbientContentKind::Mote
            }
        });
        content.ambient_slots[slot].glyph = semantic
            .glyph
            .map(AuthoredGlyph::new)
            .transpose()
            .map_err(|_| SceneGenerationError::InvalidGlyph)?;
        content.ambient_paint_slots[slot].paint = semantic
            .kind
            .map(|_| GlyphPaintSource { color_srgb8: AMBIENT_MOTE_COLOR_SRGB8 });
    }
    Ok(content)
}

pub(super) fn build_analytic_templates(bounds: Bounds3) -> Vec<AnalyticTemplateSlot> {
    let mut slots = empty_analytic_template_slots();
    for semantic in AnalyticSemantic::ALL {
        let id = semantic.id();
        slots[usize::from(id.0)].value = Some(AnalyticTemplate {
            semantic,
            shape: semantic.shape(),
            normalized_local_bounds: bounds,
        });
    }
    slots
}

fn analytic_paint(
    semantic: AnalyticSemantic,
    mood: MoodContentKind,
    day_phase: super::super::CompanionDayPhase,
    biome: &str,
) -> AnalyticPaint {
    let phase_scale = match day_phase {
        super::super::CompanionDayPhase::Dawn => 0.85,
        super::super::CompanionDayPhase::Day => 1.0,
        super::super::CompanionDayPhase::Dusk => 0.8,
        super::super::CompanionDayPhase::Night => 0.6,
    };
    match semantic {
        AnalyticSemantic::RoomBackground => {
            let (core, rim) = crate::presentation::companion_effects::tank_background_paint_srgb8(
                biome,
                phase_scale,
            );
            AnalyticPaint::ApertureDepth {
                core_srgb8: core,
                rim_srgb8: rim,
                bed_srgb8: crate::presentation::companion_effects::bed_primary_srgb8(biome),
                fleck_srgb8: crate::presentation::companion_effects::bed_fleck_srgb8(biome),
            }
        }
        AnalyticSemantic::WallShadow => AnalyticPaint::PetShadowTint {
            color_srgb8: crate::presentation::companion_effects::RETAINED_WALL_SHADOW_TINT_SRGB8,
            opacity_u8: crate::presentation::companion_effects::RETAINED_WALL_SHADOW_TINT_ALPHA_U8,
        },
        AnalyticSemantic::FloorProjection => AnalyticPaint::FloorShadowMultiplySilhouette {
            color_srgba8: {
                let color = crate::presentation::companion_effects::bed_shadow_srgb8(biome);
                [color[0], color[1], color[2], 235]
            },
        },
        AnalyticSemantic::StatusHalo => AnalyticPaint::StatusBeacon {
            active_srgba8: crate::presentation::companion_effects::srgba8(
                crate::presentation::companion_effects::STATUS_ACTIVE_SRGBA,
            ),
            calm_srgba8: crate::presentation::companion_effects::srgba8(
                crate::presentation::companion_effects::STATUS_CALM_SRGBA,
            ),
        },
        AnalyticSemantic::MoodAura => {
            let color = match mood {
                MoodContentKind::Content => {
                    crate::presentation::companion_effects::MOOD_CONTENT_SRGBA
                }
                MoodContentKind::Happy => crate::presentation::companion_effects::MOOD_HAPPY_SRGBA,
                MoodContentKind::Ecstatic => {
                    crate::presentation::companion_effects::MOOD_ECSTATIC_SRGBA
                }
                MoodContentKind::Hungry => {
                    crate::presentation::companion_effects::MOOD_HUNGRY_SRGBA
                }
                MoodContentKind::Sad => crate::presentation::companion_effects::MOOD_SAD_SRGBA,
                MoodContentKind::Sleepy => {
                    crate::presentation::companion_effects::MOOD_SLEEPY_SRGBA
                }
                MoodContentKind::Wilted => {
                    crate::presentation::companion_effects::MOOD_WILTED_SRGBA
                }
            };
            AnalyticPaint::MoodAuraRings {
                color_srgb8: crate::presentation::companion_effects::srgb8([
                    color[0], color[1], color[2],
                ]),
                ring_count: 8,
                per_ring_alpha_u8: crate::presentation::companion_effects::MOOD_AURA_RING_ALPHA_U8,
            }
        }
        AnalyticSemantic::Trouble => AnalyticPaint::TroubleBeacon {
            color_srgba8: crate::presentation::companion_effects::srgba8(
                crate::presentation::companion_effects::TROUBLE_SRGBA,
            ),
        },
        AnalyticSemantic::Dim => AnalyticPaint::DimOverlay { color_srgb8: [13, 15, 26] },
        AnalyticSemantic::PropShadows => AnalyticPaint::PropShadowMultiply {
            color_srgb8: crate::presentation::companion_effects::bed_shadow_srgb8(biome),
        },
        AnalyticSemantic::GaugePace | AnalyticSemantic::GaugeDaily | AnalyticSemantic::GaugeXp => {
            unreachable!("gauges use the closed gauge paint set")
        }
    }
}

#[cfg(test)]
pub(super) fn build_analytic_content(content: &SceneContent) -> Vec<AnalyticContentSlot> {
    let mut slots = Vec::with_capacity(MAX_ANALYTIC_PARAMS);
    project_analytic_content_with_biome(content.mood, content.day_phase, "starter", &mut slots);
    slots
}

fn project_analytic_content_for_snapshot(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<AnalyticContentSlot>,
) {
    project_analytic_content_with_biome(
        mood_content(snapshot.content.mood),
        snapshot.content.day_phase,
        snapshot.topology.room.primary_biome,
        output,
    );
}

fn project_analytic_content_with_biome(
    mood: MoodContentKind,
    day_phase: super::super::CompanionDayPhase,
    biome: &str,
    slots: &mut Vec<AnalyticContentSlot>,
) {
    slots.clear();
    slots.extend((0..MAX_ANALYTIC_PARAMS).map(|slot| AnalyticContentSlot {
        id: AnalyticParamId(slot as u8),
        value: None,
    }));
    for semantic in AnalyticSemantic::ALL {
        let paint = if semantic.gauge_lane().is_some() {
            AnalyticPaint::PerimeterGaugeSet {
                xp: GaugeLanePaint {
                    track_srgba8: crate::presentation::companion_effects::srgba8(
                        crate::presentation::companion_effects::GAUGE_XP_TRACK_SRGBA,
                    ),
                    fill_srgba8: crate::presentation::companion_effects::srgba8(
                        crate::presentation::companion_effects::GAUGE_XP_FILL_SRGBA,
                    ),
                },
                daily: GaugeLanePaint {
                    track_srgba8: crate::presentation::companion_effects::srgba8(
                        crate::presentation::companion_effects::GAUGE_DAILY_TRACK_SRGBA,
                    ),
                    fill_srgba8: crate::presentation::companion_effects::srgba8(
                        crate::presentation::companion_effects::GAUGE_DAILY_FILL_SRGBA,
                    ),
                },
                pace: GaugeLanePaint {
                    track_srgba8: crate::presentation::companion_effects::srgba8(
                        crate::presentation::companion_effects::GAUGE_PACE_TRACK_SRGBA,
                    ),
                    fill_srgba8: crate::presentation::companion_effects::srgba8(
                        crate::presentation::companion_effects::GAUGE_PACE_FILL_SRGBA,
                    ),
                },
                daily_overage_srgba8: crate::presentation::companion_effects::srgba8(
                    crate::presentation::companion_effects::GAUGE_DAILY_OVERAGE_SRGBA,
                ),
                daily_rollover_contract_unorm8:
                    crate::presentation::companion_effects::daily_rollover_contract_unorm8(),
            }
        } else {
            analytic_paint(semantic, mood, day_phase, biome)
        };
        let id = semantic.id();
        slots[usize::from(id.0)].value =
            Some(AnalyticContent { semantic, shape: semantic.shape(), paint });
    }
}

fn mood_content(mood: crate::game::metabolism::Mood) -> MoodContentKind {
    match mood {
        crate::game::metabolism::Mood::Happy => MoodContentKind::Happy,
        crate::game::metabolism::Mood::Ecstatic => MoodContentKind::Ecstatic,
        crate::game::metabolism::Mood::Content => MoodContentKind::Content,
        crate::game::metabolism::Mood::Hungry => MoodContentKind::Hungry,
        crate::game::metabolism::Mood::Sad => MoodContentKind::Sad,
        crate::game::metabolism::Mood::Sleepy => MoodContentKind::Sleepy,
        crate::game::metabolism::Mood::Wilted => MoodContentKind::Wilted,
    }
}

fn prop_paint_slot(
    catalog_id: &str,
    slot: u8,
    bloom_active: bool,
    glyphs: [PropGlyphContent; MAX_PROP_GLYPHS_PER_SLOT],
) -> Result<PropGlyphPaintSlot, SceneGenerationError> {
    let spec = crate::game::habitat::catalog_prop_by_str(catalog_id)
        .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
    let base = [spec.color.0, spec.color.1, spec.color.2];
    let blossom_override =
        bloom_active && crate::game::habitat::habitat_prop_supports_bloom(catalog_id);
    let paints = glyphs.map(|glyph| {
        glyph.glyph.map(|glyph| GlyphPaintSource {
            color_srgb8: if blossom_override && glyph.as_char() == '*' {
                [0xe8, 0x84, 0xbc]
            } else {
                base
            },
        })
    });
    Ok(PropGlyphPaintSlot { slot, paints })
}

fn project_prop_paint_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<PropGlyphPaintSlot>,
) -> Result<(), SceneGenerationError> {
    for (topology, semantic) in snapshot
        .topology
        .visible_props
        .iter()
        .zip(&snapshot.content.prop_animation_states)
    {
        let glyphs = prop_glyphs(
            topology.catalog_id,
            snapshot.topology.pet.species,
            semantic.sprite_phase,
            semantic.twinkle_active,
            semantic.chest_lid_open,
            semantic.bloom_active,
        )?;
        output.push(prop_paint_slot(
            topology.catalog_id,
            topology.stable_order,
            semantic.bloom_active == Some(true),
            glyphs,
        )?);
    }
    Ok(())
}

fn project_ambient_paint_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<AmbientGlyphPaintSlot>,
) {
    output.extend(snapshot.content.ambient_semantics.iter().map(|source| {
        AmbientGlyphPaintSlot {
            slot: source.slot,
            paint: source
                .kind
                .map(|_| GlyphPaintSource { color_srgb8: AMBIENT_MOTE_COLOR_SRGB8 }),
        }
    }));
}

#[derive(Clone, Copy)]
struct PetAnalyticGeometry {
    frame_center: [f32; 2],
    body_center: [f32; 2],
    body_radii: [f32; 2],
}

fn project_analytic_frame_slots_for_geometry(
    camera: OrthographicCamera,
    pet: PetAnalyticGeometry,
    cell_extent_points: [f32; 2],
    effective_z: f32,
    facing: i8,
    status_tone: StatusBeaconTone,
    slots: &mut Vec<AnalyticFrameSlot>,
) {
    slots.clear();
    slots.extend((0..MAX_ANALYTIC_PARAMS).map(|slot| AnalyticFrameSlot {
        id: AnalyticParamId(slot as u8),
        value: None,
    }));
    let center = [
        (camera.width_points - 1.0) * 0.5,
        (camera.height_points - 1.0) * 0.5,
    ];
    let radius = camera.width_points.min(camera.height_points) * 0.5 - 1.0;
    let room = [0.0, 0.0, camera.width_points, camera.height_points];
    let pet_rect = [
        pet.body_center[0] - pet.body_radii[0],
        pet.body_center[1] - pet.body_radii[1],
        pet.body_radii[0] * 2.0,
        pet.body_radii[1] * 2.0,
    ];
    let wall_cue = crate::presentation::companion_effects::wall_shadow_depth_cue(effective_z);
    let wall_offset = [
        wall_cue.detach_cells * cell_extent_points[0],
        -wall_cue.detach_cells * cell_extent_points[1],
    ];
    let wall_softness = (cell_extent_points[0].min(cell_extent_points[1]) * 0.35).max(1.0);
    let wall_rect = [
        pet_rect[0] + wall_offset[0].min(0.0) - wall_softness,
        pet_rect[1] + wall_offset[1].min(0.0) - wall_softness,
        pet_rect[2] + wall_offset[0].abs() + wall_softness * 2.0,
        pet_rect[3] + wall_offset[1].abs() + wall_softness * 2.0,
    ];
    let floor = crate::presentation::companion_effects::floor_projection_metrics(
        camera.width_points,
        camera.height_points,
        camera.height_points * 0.76,
        camera.height_points,
        pet.frame_center[0],
        effective_z,
    )
    .expect("validated companion layout and depth produce floor geometry");
    let floor_center = [floor.center_x, camera.height_points - floor.center_y];
    let floor_rect = [
        floor_center[0] - floor.radius_x,
        floor_center[1] - floor.radius_y,
        floor.radius_x * 2.0,
        floor.radius_y * 2.0,
    ];
    let aura_radius = crate::presentation::companion_effects::mood_aura_radius(f64::from(
        pet.body_radii[0] * 2.0,
    )) as f32;
    let gauge_layout = crate::presentation::companion_effects::perimeter_gauge_layout(
        f64::from(radius),
        crate::presentation::companion_effects::COMPANION_GAUGE_GAP_DEGREES,
    );
    let gauge_lane =
        |lane: crate::presentation::companion_effects::GaugeLaneLayout| GaugeLaneGeometry {
            radius_points: lane.radius as f32,
            stroke_width_points: lane.stroke_width as f32,
            track_start_degrees: lane.track_start_degrees as f32,
            track_sweep_degrees: lane.track_sweep_degrees as f32,
            cap: GaugeLineCap::Round,
        };
    let gauge_geometry = AnalyticGeometry::PerimeterGaugeSet {
        center_points: center,
        xp: gauge_lane(gauge_layout.xp),
        daily: gauge_lane(gauge_layout.daily),
        pace: gauge_lane(gauge_layout.pace),
    };
    let gauge_frame = |semantic| AnalyticFrame {
        semantic,
        shape: AnalyticShape::PerimeterGaugeSet,
        rect_points: room,
        geometry: gauge_geometry,
    };
    let values = [
        AnalyticFrame {
            semantic: AnalyticSemantic::RoomBackground,
            shape: AnalyticShape::ApertureRadial,
            rect_points: room,
            geometry: AnalyticGeometry::ApertureRadial {
                center_points: center,
                radius_points: radius,
                feather_points: 1.0,
            },
        },
        AnalyticFrame {
            semantic: AnalyticSemantic::WallShadow,
            shape: AnalyticShape::PetSilhouette,
            rect_points: wall_rect,
            geometry: AnalyticGeometry::PetSilhouette {
                mask: AnalyticMaskSource::PetBody,
                offset_points: wall_offset,
                softness_points: wall_softness,
            },
        },
        AnalyticFrame {
            semantic: AnalyticSemantic::FloorProjection,
            shape: AnalyticShape::PetFloorProjection,
            rect_points: floor_rect,
            geometry: AnalyticGeometry::PetFloorProjection {
                mask: AnalyticMaskSource::PetBody,
                facing,
            },
        },
        AnalyticFrame {
            semantic: AnalyticSemantic::StatusHalo,
            shape: AnalyticShape::StatusBeacon,
            rect_points: [center[0] - 2.0, center[1] + radius - 2.0, 4.0, 4.0],
            geometry: AnalyticGeometry::StatusBeacon {
                center_points: [center[0], center[1] + radius],
                radius_points: 1.0,
                thickness_points: 1.0,
                tone: status_tone,
            },
        },
        AnalyticFrame {
            semantic: AnalyticSemantic::MoodAura,
            shape: AnalyticShape::PetAura,
            rect_points: [
                pet.body_center[0] - aura_radius,
                pet.body_center[1] - aura_radius,
                aura_radius * 2.0,
                aura_radius * 2.0,
            ],
            geometry: AnalyticGeometry::PetAura {
                center_points: pet.body_center,
                max_radius_points: aura_radius,
                ring_count: 8,
                feather_points: 4.0,
            },
        },
        gauge_frame(AnalyticSemantic::GaugePace),
        AnalyticFrame {
            semantic: AnalyticSemantic::Trouble,
            shape: AnalyticShape::TroubleBeacon,
            rect_points: [
                center[0] - radius * 0.66 - 2.0,
                center[1] + radius * 0.66 - 2.0,
                4.0,
                4.0,
            ],
            geometry: AnalyticGeometry::TroubleBeacon {
                center_points: [center[0] - radius * 0.66, center[1] + radius * 0.66],
                radius_points: 1.0,
                thickness_points: 1.0,
            },
        },
        AnalyticFrame {
            semantic: AnalyticSemantic::Dim,
            shape: AnalyticShape::SurfaceOverlay,
            rect_points: room,
            geometry: AnalyticGeometry::SurfaceOverlay,
        },
        AnalyticFrame {
            semantic: AnalyticSemantic::PropShadows,
            shape: AnalyticShape::PropShadowField,
            rect_points: room,
            geometry: AnalyticGeometry::PropShadowField,
        },
        gauge_frame(AnalyticSemantic::GaugeDaily),
        gauge_frame(AnalyticSemantic::GaugeXp),
    ];
    for value in values {
        let id = value.semantic.id();
        slots[usize::from(id.0)].value = Some(value);
    }
}

#[cfg(test)]
pub(super) fn fixture_analytic_frame_slots(camera: OrthographicCamera) -> Vec<AnalyticFrameSlot> {
    let mut slots = Vec::with_capacity(MAX_ANALYTIC_PARAMS);
    project_analytic_frame_slots_for_geometry(
        camera,
        PetAnalyticGeometry {
            frame_center: [180.0, 180.0],
            body_center: [180.0, 180.0],
            body_radii: [39.0, 60.0],
        },
        [8.0, 20.0],
        0.0,
        1,
        StatusBeaconTone::Calm,
        &mut slots,
    );
    slots
}

fn project_analytic_frame_slots(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<AnalyticFrameSlot>,
) -> Result<(), SceneGenerationError> {
    let layout = snapshot.topology.layout;
    let camera = OrthographicCamera::new(layout.width_points, layout.height_points, -2.0, 2.0)
        .expect("validated companion layout produces an orthographic camera");
    let transform = pet_transform(snapshot);
    let cell = snapshot.topology.glyph_grid.cell_extent_points;
    let frame_center = [
        transform.translation[0] + transform.pivot[0],
        transform.translation[1] + transform.pivot[1],
    ];
    let (body_center, body_radii) = pet_body_world_geometry(snapshot, transform)?;
    let effective_depth = resolved_effective_depth(snapshot);
    project_analytic_frame_slots_for_geometry(
        camera,
        PetAnalyticGeometry { frame_center, body_center, body_radii },
        cell,
        effective_depth,
        if snapshot.frame.facing < 0 { -1 } else { 1 },
        if snapshot.frame.calm {
            StatusBeaconTone::Calm
        } else {
            StatusBeaconTone::Active
        },
        output,
    );
    Ok(())
}

pub(super) fn pet_body_world_geometry(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    transform: Transform3,
) -> Result<([f32; 2], [f32; 2]), SceneGenerationError> {
    let mut roles = [PetPaletteRole::Body; MAX_PET_ART_SLOTS];
    let mut assigned = [false; MAX_PET_ART_SLOTS];
    let mut min_declared_body_column = None::<usize>;
    let mut min_declared_body_row = None::<usize>;
    let mut max_declared_body_column = None::<usize>;
    let mut max_declared_body_row = None::<usize>;
    for span in &snapshot.content.pet_roles {
        let role = pet_role(span.role).ok_or(SceneGenerationError::InvalidPetRole)?;
        for column in span.start_char..span.end_char {
            let index = usize::from(
                span.line_index * crate::presentation::companion_scene::PET_LATTICE_WIDTH + column,
            );
            if index >= MAX_PET_ART_SLOTS || assigned[index] {
                return Err(SceneGenerationError::OverlappingPetRole);
            }
            assigned[index] = true;
            roles[index] = role;
            if PetArtFilter::Body.includes(role) {
                let column = usize::from(column);
                let row = usize::from(span.line_index);
                min_declared_body_column =
                    Some(min_declared_body_column.map_or(column, |current| current.min(column)));
                min_declared_body_row =
                    Some(min_declared_body_row.map_or(row, |current| current.min(row)));
                max_declared_body_column =
                    Some(max_declared_body_column.map_or(column, |current| current.max(column)));
                max_declared_body_row =
                    Some(max_declared_body_row.map_or(row, |current| current.max(row)));
            }
        }
    }

    let mut min_body_column = None::<usize>;
    let mut min_body_row = None::<usize>;
    let mut max_body_column = None::<usize>;
    let mut max_body_row = None::<usize>;
    for (row, line) in snapshot.content.pet_lines.iter().enumerate() {
        for (column, glyph) in line.chars().enumerate() {
            let index =
                row * usize::from(crate::presentation::companion_scene::PET_LATTICE_WIDTH) + column;
            if glyph != ' ' && PetArtFilter::Body.includes(roles[index]) {
                min_body_column =
                    Some(min_body_column.map_or(column, |current| current.min(column)));
                min_body_row = Some(min_body_row.map_or(row, |current| current.min(row)));
                max_body_column =
                    Some(max_body_column.map_or(column, |current| current.max(column)));
                max_body_row = Some(max_body_row.map_or(row, |current| current.max(row)));
            }
        }
    }
    let min_body_column = min_body_column
        .or(min_declared_body_column)
        .ok_or(SceneGenerationError::InvalidPetRole)?;
    let min_body_row = min_body_row
        .or(min_declared_body_row)
        .ok_or(SceneGenerationError::InvalidPetRole)?;
    let max_body_column = max_body_column
        .or(max_declared_body_column)
        .ok_or(SceneGenerationError::InvalidPetRole)?;
    let max_body_row = max_body_row
        .or(max_declared_body_row)
        .ok_or(SceneGenerationError::InvalidPetRole)?;
    let cell = snapshot.topology.glyph_grid.cell_extent_points;
    let lattice_height = f32::from(crate::presentation::companion_scene::PET_LATTICE_HEIGHT);
    let local_min = [
        min_body_column as f32 * cell[0],
        (lattice_height - (max_body_row + 1) as f32) * cell[1],
    ];
    let local_max = [
        (max_body_column + 1) as f32 * cell[0],
        (lattice_height - min_body_row as f32) * cell[1],
    ];
    let matrix = transform
        .matrix()
        .map_err(|_| SceneGenerationError::NonFinite)?;
    let corners = [
        [local_min[0], local_min[1], 0.0],
        [local_max[0], local_min[1], 0.0],
        [local_min[0], local_max[1], 0.0],
        [local_max[0], local_max[1], 0.0],
    ];
    let first = matrix.transform_point3(corners[0]);
    let mut min = [first[0], first[1]];
    let mut max = min;
    for corner in corners.into_iter().skip(1) {
        let point = matrix.transform_point3(corner);
        min[0] = min[0].min(point[0]);
        min[1] = min[1].min(point[1]);
        max[0] = max[0].max(point[0]);
        max[1] = max[1].max(point[1]);
    }
    let radii = [(max[0] - min[0]) * 0.5, (max[1] - min[1]) * 0.5];
    if !radii
        .into_iter()
        .all(|radius| radius.is_finite() && radius > 0.0)
    {
        return Err(SceneGenerationError::NonFinite);
    }
    Ok(([(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5], radii))
}

fn pet_role(value: &str) -> Option<PetPaletteRole> {
    Some(match value {
        "body" => PetPaletteRole::Body,
        "body-glow" => PetPaletteRole::BodyGlow,
        "eye" => PetPaletteRole::Eye,
        "mouth" => PetPaletteRole::Mouth,
        "accent" => PetPaletteRole::Accent,
        "pattern" => PetPaletteRole::Pattern,
        "particle" => PetPaletteRole::Particle,
        "corruption" => PetPaletteRole::Corruption,
        _ => return None,
    })
}

fn weather_content(value: &str) -> Result<WeatherContentKind, SceneGenerationError> {
    Ok(match value {
        "clear" => WeatherContentKind::Clear,
        "cache-mist" => WeatherContentKind::CacheMist,
        "output-sparks" => WeatherContentKind::OutputSparks,
        "reasoning-pulse" => WeatherContentKind::ReasoningPulse,
        "mixed" => WeatherContentKind::Mixed,
        _ => return Err(SceneGenerationError::UnknownAuthoredIdentity),
    })
}

pub(super) fn prop_glyphs(
    catalog_id: &str,
    species: crate::pet::generation::Species,
    sprite_phase: Option<u8>,
    twinkle: Option<bool>,
    lid_open: Option<bool>,
    bloom: Option<bool>,
) -> Result<[PropGlyphContent; MAX_PROP_GLYPHS_PER_SLOT], SceneGenerationError> {
    let sprite = crate::presentation::props::presentation_prop_sprite(
        catalog_id,
        crate::presentation::props::PresentationPropVisualState {
            species,
            sprite_phase,
            twinkle_active: twinkle,
            chest_lid_open: lid_open,
            bloom_active: bloom,
        },
    )
    .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
    let mut result =
        [PropGlyphContent { glyph: None, local_cell: [0; 2] }; MAX_PROP_GLYPHS_PER_SLOT];
    for (index, cell) in sprite.into_iter().enumerate() {
        if index >= MAX_PROP_GLYPHS_PER_SLOT {
            return Err(SceneGenerationError::FixedCapacity);
        }
        result[index] = PropGlyphContent {
            glyph: Some(
                AuthoredGlyph::new(cell.glyph).map_err(|_| SceneGenerationError::InvalidGlyph)?,
            ),
            local_cell: [cell.dx, cell.dy],
        };
    }
    Ok(result)
}

fn build_frame(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    template: &SceneTemplate,
) -> Result<SceneFrame, SceneGenerationError> {
    let layout = snapshot.topology.layout;
    let camera = OrthographicCamera::new(layout.width_points, layout.height_points, -2.0, 2.0)
        .map_err(|_| SceneGenerationError::NonFinite)?;
    let mut frame = SceneFrame::empty_v2(camera);
    frame.room_glyph_slots.clear();
    project_room_frame_delta(snapshot, &mut frame.room_glyph_slots);
    frame.nodes = template
        .nodes
        .iter()
        .map(|node| NodeFrameState {
            node: node.id,
            local_transform: Transform3::IDENTITY,
            visible: true,
            opacity: 1.0,
        })
        .collect();
    let set_node = |frame: &mut SceneFrame,
                    name: &str,
                    transform: Option<Transform3>,
                    visible: Option<bool>,
                    opacity: Option<f32>|
     -> Result<(), SceneGenerationError> {
        let node_alias = alias(name)?;
        let node = NodeId::from_alias(&node_alias);
        let state = frame
            .nodes
            .iter_mut()
            .find(|state| state.node == node)
            .ok_or(SceneGenerationError::UnknownAuthoredIdentity)?;
        if let Some(transform) = transform {
            state.local_transform = transform;
        }
        if let Some(visible) = visible {
            state.visible = visible;
        }
        if let Some(opacity) = opacity {
            state.opacity = opacity;
        }
        Ok(())
    };
    let pet_transform = pet_transform(snapshot);
    set_node(&mut frame, "pet", Some(pet_transform), None, None)?;
    set_node(
        &mut frame,
        "pet.body",
        None,
        None,
        Some(pet_body_opacity(snapshot)),
    )?;
    set_node(
        &mut frame,
        "pet.particles",
        None,
        Some(!snapshot.frame.asleep),
        Some(snapshot.frame.pet_depth_cue.opacity),
    )?;
    let effective_depth = resolved_effective_depth(snapshot);
    set_node(
        &mut frame,
        "pet.shadow.wall",
        None,
        None,
        Some(
            crate::presentation::companion_effects::wall_shadow_depth_cue(effective_depth).strength,
        ),
    )?;
    set_node(
        &mut frame,
        "pet.projection.floor",
        None,
        None,
        Some(floor_projection_opacity(snapshot)),
    )?;
    let (status_visible, status_opacity) = super::super::canonical_activity_status(snapshot);
    set_node(
        &mut frame,
        "chrome.status",
        None,
        Some(status_visible),
        Some(status_opacity),
    )?;
    set_node(
        &mut frame,
        "chrome.trouble",
        None,
        Some(snapshot.frame.helper_trouble),
        Some(if snapshot.frame.helper_trouble {
            1.0
        } else {
            0.0
        }),
    )?;
    set_node(
        &mut frame,
        "chrome.dim",
        None,
        Some(snapshot.frame.dim_amount > 0.0),
        Some(snapshot.frame.dim_amount),
    )?;

    for source in &snapshot.frame.prop_instances {
        let slot = usize::from(source.slot);
        let origin = [
            source.origin_points[0],
            prop_origin_y_up(
                layout.height_points,
                snapshot.topology.glyph_grid.cell_extent_points[1],
                source.origin_points[1],
            ),
        ];
        frame.prop_slots[slot] = PropFrameSlot {
            slot: source.slot,
            visible: source.visible,
            origin_points: origin,
            motion_offset_points: source.motion_offset_points,
            opacity: source.opacity,
            footprint_points: source.footprint_points,
            contact_shadow_strength: source.contact_shadow_strength,
        };
    }
    for (semantic, source) in snapshot
        .content
        .tank_animation_states
        .iter()
        .zip(&snapshot.frame.tank_instances)
    {
        let slot = usize::from(source.slot);
        let mut cells = [TankCellFrame {
            visible: false,
            position_points: [0.0; 2],
            layer: InstanceLayer::Behind,
            bounds_points: [0.0; 4],
        }; MAX_TANK_GLYPHS_PER_SLOT];
        for (index, (semantic_cell, cell)) in semantic.cells.iter().zip(&source.cells).enumerate() {
            let layer = match semantic_cell.layer {
                crate::presentation::companion_scene::TankLayerSnapshot::Behind => InstanceLayer::Behind,
                crate::presentation::companion_scene::TankLayerSnapshot::Foreground
                | crate::presentation::companion_scene::TankLayerSnapshot::BehindAnchorForegroundHost => InstanceLayer::Foreground,
            };
            let bounds = source.bounds_points.unwrap_or([
                cell.position_points[0],
                cell.position_points[1],
                0.0,
                0.0,
            ]);
            cells[index] = TankCellFrame {
                visible: source.visible,
                position_points: [
                    cell.position_points[0],
                    layout.height_points - cell.position_points[1],
                ],
                layer,
                bounds_points: [
                    bounds[0],
                    layout.height_points - bounds[1] - bounds[3],
                    bounds[2],
                    bounds[3],
                ],
            };
        }
        frame.tank_slots[slot] = TankFrameSlot {
            slot: source.slot,
            visible: source.visible,
            origin_points: [
                source.origin_points[0],
                layout.height_points - source.origin_points[1],
            ],
            cells,
        };
    }
    for source in &snapshot.frame.ambient_instances {
        let slot = usize::from(source.slot);
        let occupied = snapshot.content.ambient_semantics[slot].kind.is_some();
        frame.ambient_slots[slot] = AmbientFrameSlot {
            slot: source.slot,
            visible: occupied && source.visible,
            position_points: if occupied {
                [
                    source.position_points[0],
                    layout.height_points - source.position_points[1],
                ]
            } else {
                [0.0; 2]
            },
            opacity: if occupied { source.opacity } else { 0.0 },
        };
    }
    frame.gauges = snapshot.frame.gauge_fractions;
    frame.dim_amount = snapshot.frame.dim_amount;
    project_analytic_frame_slots(snapshot, &mut frame.analytic_slots)?;
    frame.lights.clear();
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::generation::Species;

    #[test]
    fn front_glass_hud_compiles_after_pet_as_screen_chrome() {
        let mut vm = crate::tui::view_model::WatchViewModel::fixture_with_habitat_props();
        let pet = crate::pet::generation::generate_pet("front-glass-hud-fixture")
            .with_species(Species::Fuzz);
        let rendered = crate::pet::render::render_pet(
            &pet,
            crate::game::evolution::Stage::S3,
            crate::game::metabolism::Mood::Content,
            crate::pet::render::AnimationFrame::default(),
        );
        vm.pet_render.seed = pet.seed;
        vm.pet_render.generated_species = Species::Fuzz;
        vm.pet_render.stage = crate::game::evolution::Stage::S3;
        vm.pet_render.mood = crate::game::metabolism::Mood::Content;
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        let input = crate::presentation::companion_scene::CompanionSceneProjectionInput::round(
            crate::presentation::companion_scene::CompanionProjectionClock::new(
                time::OffsetDateTime::UNIX_EPOCH,
                0,
            ),
            crate::presentation::companion_scene::CompanionLogicalLayout::round(360.0, 360.0),
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );
        let snapshot =
            crate::presentation::companion_scene::CompanionSceneSnapshot::project_with_input(
                &vm, input,
            )
            .unwrap();
        let template = build_template(&snapshot).unwrap();
        let hud = template
            .primitives
            .iter()
            .find(|primitive| {
                matches!(
                    &primitive.binding,
                    PrimitiveBinding::Instances(InstanceGroupBinding::Hud)
                )
            })
            .unwrap();
        let pet = template
            .primitives
            .iter()
            .find(|primitive| {
                matches!(
                    &primitive.binding,
                    PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body))
                )
            })
            .unwrap();

        assert_eq!(
            crate::round::hud::COMPANION_HUD_DEPTH_PLANE,
            crate::round::hud::CompanionHudDepthPlane::FrontGlass
        );
        assert!(hud.authored_order > pet.authored_order);
        assert_eq!(hud.depth, DepthBehavior::ScreenNoDepth);
        assert_eq!(hud.space, PrimitiveSpace::Screen);
    }

    #[test]
    fn retained_sprite_adapter_preserves_canonical_local_cells() {
        for spec in crate::game::habitat::HABITAT_PROP_CATALOG {
            let supports_sprite_phase = crate::game::habitat::habitat_prop_animation_state(
                spec.id,
                time::OffsetDateTime::UNIX_EPOCH,
            )
            .sprite_phase
            .is_some();
            let phases = if supports_sprite_phase {
                &[Some(0), Some(1)][..]
            } else {
                &[None][..]
            };
            for species in Species::all() {
                for &sprite_phase in phases {
                    for twinkle_active in [false, true] {
                        for chest_lid_open in [false, true] {
                            for bloom_active in [false, true] {
                                let state =
                                    crate::presentation::props::PresentationPropVisualState {
                                        species,
                                        sprite_phase,
                                        twinkle_active: Some(twinkle_active),
                                        chest_lid_open: Some(chest_lid_open),
                                        bloom_active: Some(bloom_active),
                                    };
                                let canonical =
                                    crate::presentation::props::presentation_prop_sprite(
                                        spec.id, state,
                                    )
                                    .unwrap();
                                let adapted = prop_glyphs(
                                    spec.id,
                                    species,
                                    sprite_phase,
                                    Some(twinkle_active),
                                    Some(chest_lid_open),
                                    Some(bloom_active),
                                )
                                .unwrap();
                                let canonical = canonical
                                    .iter()
                                    .map(|cell| (cell.dx, cell.dy, cell.glyph))
                                    .collect::<Vec<_>>();
                                let adapted = adapted
                                    .iter()
                                    .filter_map(|cell| {
                                        cell.glyph.map(|glyph| {
                                            (
                                                cell.local_cell[0],
                                                cell.local_cell[1],
                                                glyph.as_char(),
                                            )
                                        })
                                    })
                                    .collect::<Vec<_>>();
                                assert_eq!(adapted, canonical, "{} state {state:?}", spec.id);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_prop_identity_remains_fail_closed() {
        assert_eq!(
            prop_glyphs("unknown_stored_prop", Species::Fuzz, None, None, None, None),
            Err(SceneGenerationError::UnknownAuthoredIdentity)
        );
    }

    #[test]
    fn prop_cell_top_left_converts_to_retained_cell_bottom_left() {
        assert_eq!(prop_origin_y_up(360.0, 20.0, 320.0), 20.0);
        assert_eq!(prop_origin_y_up(360.0, 20.0, 300.0), 40.0);
    }
}
