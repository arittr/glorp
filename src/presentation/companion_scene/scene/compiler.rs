use super::checksum::{checksum_content, checksum_frame, checksum_template};
use super::*;

impl SceneGenerationData {
    #[cfg(test)]
    pub(super) fn delta_capacities(&self) -> [usize; 13] {
        [
            self.delta_scratch.content.pet_art_slots.capacity(),
            self.delta_scratch.content.room_glyph_slots.capacity(),
            self.delta_scratch.content.prop_slots.capacity(),
            self.delta_scratch.content.tank_slots.capacity(),
            self.delta_scratch.content.ambient_slots.capacity(),
            self.delta_scratch.content.hud_slots.capacity(),
            self.delta_scratch.frame.nodes.capacity(),
            self.delta_scratch.frame.room_glyph_slots.capacity(),
            self.delta_scratch.frame.prop_slots.capacity(),
            self.delta_scratch.frame.tank_slots.capacity(),
            self.delta_scratch.frame.ambient_slots.capacity(),
            self.delta_scratch.frame.hud_slots.capacity(),
            self.delta_scratch.frame.lights.capacity(),
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
        content.pet_art_slots.clear();
        content.room_glyph_slots.clear();
        content.prop_slots.clear();
        content.tank_slots.clear();
        content.ambient_slots.clear();
        content.hud_slots.clear();
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
        frame.hud_slots.clear();
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
        }
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::TANK)
        {
            project_tank_delta(snapshot, &mut content.tank_slots)?;
        }
        if semantic
            .contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::AMBIENT)
            || semantic.contains(
                crate::presentation::companion_scene::runtime::SemanticChangeMask::MOOD_WEATHER,
            )
        {
            for source in &snapshot.content.ambient_semantics {
                content.ambient_slots.push(AmbientContentSlot {
                    slot: source.slot,
                    kind: source.kind.map(|kind| match kind {
                        crate::presentation::companion_scene::AmbientSemanticKindSnapshot::Weather => AmbientContentKind::Weather,
                        crate::presentation::companion_scene::AmbientSemanticKindSnapshot::ActivityPulse => {
                            AmbientContentKind::ActivityPulse
                        }
                        crate::presentation::companion_scene::AmbientSemanticKindSnapshot::Mote => AmbientContentKind::Mote,
                    }),
                    glyph: source
                        .glyph
                        .map(AuthoredGlyph::new)
                        .transpose()
                        .map_err(|_| SceneGenerationError::InvalidGlyph)?,
                });
            }
        }
        if semantic.contains(crate::presentation::companion_scene::runtime::SemanticChangeMask::HUD)
        {
            for source in &snapshot.content.hud_glyphs {
                content.hud_slots.push(HudContentSlot {
                    slot: source.slot,
                    glyph: source
                        .glyph
                        .map(AuthoredGlyph::new)
                        .transpose()
                        .map_err(|_| SceneGenerationError::InvalidGlyph)?,
                });
            }
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
                super::super::canonical_activity_pulse_state(snapshot);
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
                    "pet.body" => node.opacity = if snapshot.frame.asleep { 0.65 } else { 1.0 },
                    "pet.particles" => node.visible = !snapshot.frame.asleep,
                    "chrome.status" => {
                        node.visible = status_visible;
                        node.opacity = status_opacity;
                    }
                    _ => unreachable!("closed status node set"),
                }
                frame.nodes.push(node);
            }
            for source in &snapshot.frame.hud_instances {
                let occupied = snapshot.content.hud_glyphs[usize::from(source.slot)]
                    .glyph
                    .is_some();
                frame.hud_slots.push(HudFrameSlot {
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
            frame.gauges = Some(snapshot.frame.gauges.map(gauge_value));
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
        let mut glyphs = [None; MAX_TANK_GLYPHS_PER_SLOT];
        for (index, cell) in semantic.cells.iter().enumerate() {
            if index >= MAX_TANK_GLYPHS_PER_SLOT {
                return Err(SceneGenerationError::FixedCapacity);
            }
            glyphs[index] = Some(
                AuthoredGlyph::new(cell.glyph).map_err(|_| SceneGenerationError::InvalidGlyph)?,
            );
        }
        output.push(TankContentSlot {
            slot: topology.stable_order,
            content: Some(TankSemanticContent {
                sprite_variant: semantic.sprite_variant,
                morph: semantic.anemone_morph,
                glyphs,
            }),
        });
    }
    Ok(())
}

#[allow(dead_code)]
fn pet_transform(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
) -> Transform3 {
    let y = snapshot.frame.pet_anchor_points[1]
        + snapshot.frame.breath_offset_y_points
        + snapshot.frame.bob_offset_y_points;
    let cell = snapshot.topology.glyph_grid.cell_extent_points;
    let pet_extent = [
        f32::from(crate::presentation::companion_scene::PET_LATTICE_WIDTH) * cell[0],
        f32::from(crate::presentation::companion_scene::PET_LATTICE_HEIGHT) * cell[1],
    ];
    let mut transform = Transform3::translated([
        snapshot.frame.pet_anchor_points[0],
        snapshot.topology.layout.height_points - y - pet_extent[1],
        snapshot.frame.pet_depth,
    ]);
    transform.scale[0] = f32::from(snapshot.frame.facing);
    transform.pivot = [pet_extent[0] * 0.5, pet_extent[1] * 0.5, 0.0];
    transform
}

#[allow(dead_code)]
fn project_prop_frame_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<PropFrameSlot>,
) {
    for source in &snapshot.content.prop_animation_states {
        output.push(PropFrameSlot {
            slot: source.stable_order,
            visible: true,
            origin_points: [
                source.origin_points[0],
                snapshot.topology.layout.height_points - source.origin_points[1],
            ],
            motion_offset_points: if source
                .motion_phase
                .is_some_and(|phase| !phase.is_multiple_of(2))
            {
                [0.0, 1.0]
            } else {
                [0.0; 2]
            },
            opacity: if snapshot.frame.asleep { 0.72 } else { 1.0 },
        });
    }
}

#[allow(dead_code)]
fn project_tank_frame_delta(
    snapshot: &crate::presentation::companion_scene::CompanionSceneSnapshot,
    output: &mut Vec<TankFrameSlot>,
) -> Result<(), SceneGenerationError> {
    for source in &snapshot.content.tank_animation_states {
        let mut cells = [TankCellFrame {
            visible: false,
            position_points: [0.0; 2],
            layer: InstanceLayer::Behind,
            bounds_points: [0.0; 4],
        }; MAX_TANK_GLYPHS_PER_SLOT];
        for (index, cell) in source.cells.iter().enumerate() {
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
                layer: match cell.layer {
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
            slot: source.stable_order,
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

#[allow(dead_code)]
fn gauge_value(gauge: crate::presentation::companion_scene::GaugeLevelSnapshot) -> f32 {
    match gauge {
        crate::presentation::companion_scene::GaugeLevelSnapshot::Empty => 0.0,
        crate::presentation::companion_scene::GaugeLevelSnapshot::Low => 0.125,
        crate::presentation::companion_scene::GaugeLevelSnapshot::Medium => 0.375,
        crate::presentation::companion_scene::GaugeLevelSnapshot::High => 0.75,
        crate::presentation::companion_scene::GaugeLevelSnapshot::Full => 1.0,
    }
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
    for changed in &delta.hud_slots {
        content.hud_slots[usize::from(changed.slot)] = *changed;
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
    for changed in &delta.hud_slots {
        frame.hud_slots[usize::from(changed.slot)] = *changed;
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
        || snapshot.content.hud_glyphs.len() != MAX_HUD_GLYPH_SLOTS
        || snapshot.frame.hud_instances.len() != MAX_HUD_GLYPH_SLOTS
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
                        bounds: Bounds3|
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
            depth_cue: DepthCue::NEUTRAL,
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
        ("world.ambient", Some("world.far"), -1.65),
        ("world.behind", Some("scene.root"), 0.0),
        ("world.props.behind", Some("world.behind"), 0.0),
        ("world.tank.behind", Some("world.behind"), 0.0),
        ("pet.shadow.wall", Some("world.behind"), -1.30),
        ("pet", Some("scene.root"), 0.0),
        ("pet.aura.mood", Some("pet"), 0.0),
        ("pet.body", Some("pet"), 0.0),
        ("pet.particles", Some("pet"), 0.0),
        ("world.foreground", Some("scene.root"), 0.0),
        ("world.props.foreground", Some("world.foreground"), 0.0),
        ("world.tank.foreground", Some("world.foreground"), 0.0),
        ("chrome.screen", Some("scene.root"), 0.0),
        ("chrome.gauges", Some("chrome.screen"), 0.0),
        ("chrome.status", Some("chrome.screen"), 0.0),
        ("chrome.trouble", Some("chrome.screen"), 0.0),
        ("chrome.hud", Some("chrome.screen"), 0.0),
        ("chrome.dim", Some("chrome.screen"), 0.0),
    ] {
        add_node(name.to_owned(), parent, z, scene_bounds)?;
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
        add_node(prop_alias.clone(), Some(parent), z, unit_bounds)?;
        if prop.catalog_id == crate::game::habitat::TOKEN_TREASURE_CHEST_2M {
            add_node(
                format!("{prop_alias}.body"),
                Some(&prop_alias),
                0.0,
                unit_bounds,
            )?;
            add_node(
                format!("{prop_alias}.lid"),
                Some(&prop_alias),
                0.0,
                unit_bounds,
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
        )?;
        add_node(
            format!("{tank_alias}.foreground"),
            Some("world.tank.foreground"),
            1.35 + f32::from(tank.stable_order) * 0.01,
            unit_bounds,
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
        "material.multiply-shadow",
        "resource.analytic-geometry",
        WorldBlend::Multiply,
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
    push(
        "chrome.gauges",
        PrimitiveKind::AnalyticShape,
        "material.screen-chrome",
        "resource.analytic-geometry",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::ScreenNoDepth,
        PrimitiveBinding::Analytic(AnalyticSemantic::Gauges.id()),
        PrimitiveSpace::Screen,
    )?;
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
    push(
        "chrome.hud",
        PrimitiveKind::InstanceQuad,
        "material.screen-chrome",
        "resource.hud-glyph-atlas",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::ScreenNoDepth,
        PrimitiveBinding::Instances(InstanceGroupBinding::Hud),
        PrimitiveSpace::Screen,
    )?;
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
    content.mood = match snapshot.content.mood {
        crate::game::metabolism::Mood::Happy => MoodContentKind::Happy,
        crate::game::metabolism::Mood::Ecstatic => MoodContentKind::Ecstatic,
        crate::game::metabolism::Mood::Content => MoodContentKind::Content,
        crate::game::metabolism::Mood::Hungry => MoodContentKind::Hungry,
        crate::game::metabolism::Mood::Sad => MoodContentKind::Sad,
        crate::game::metabolism::Mood::Sleepy => MoodContentKind::Sleepy,
        crate::game::metabolism::Mood::Wilted => MoodContentKind::Wilted,
    };
    content.weather = weather_content(snapshot.content.room_weather)?;
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
    }
    for (topology, semantic) in snapshot
        .topology
        .visible_tank_inhabitants
        .iter()
        .zip(&snapshot.content.tank_animation_states)
    {
        if topology.catalog_id != semantic.catalog_id
            || topology.stable_order != semantic.stable_order
        {
            return Err(SceneGenerationError::UnknownAuthoredIdentity);
        }
        let mut glyphs = [None; MAX_TANK_GLYPHS_PER_SLOT];
        if semantic.cells.len() > MAX_TANK_GLYPHS_PER_SLOT {
            return Err(SceneGenerationError::FixedCapacity);
        }
        for (slot, cell) in semantic.cells.iter().enumerate() {
            glyphs[slot] = Some(
                AuthoredGlyph::new(cell.glyph).map_err(|_| SceneGenerationError::InvalidGlyph)?,
            );
        }
        content.tank_slots[usize::from(topology.stable_order)].content =
            Some(TankSemanticContent {
                sprite_variant: semantic.sprite_variant,
                morph: semantic.anemone_morph,
                glyphs,
            });
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
            crate::presentation::companion_scene::AmbientSemanticKindSnapshot::Weather => {
                AmbientContentKind::Weather
            }
            crate::presentation::companion_scene::AmbientSemanticKindSnapshot::ActivityPulse => {
                AmbientContentKind::ActivityPulse
            }
            crate::presentation::companion_scene::AmbientSemanticKindSnapshot::Mote => {
                AmbientContentKind::Mote
            }
        });
        content.ambient_slots[slot].glyph = semantic
            .glyph
            .map(AuthoredGlyph::new)
            .transpose()
            .map_err(|_| SceneGenerationError::InvalidGlyph)?;
    }
    for hud in &snapshot.content.hud_glyphs {
        let slot = usize::from(hud.slot);
        if slot >= MAX_HUD_GLYPH_SLOTS {
            return Err(SceneGenerationError::FixedCapacity);
        }
        content.hud_slots[slot].glyph = hud
            .glyph
            .map(AuthoredGlyph::new)
            .transpose()
            .map_err(|_| SceneGenerationError::InvalidGlyph)?;
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
    let phase = sprite_phase.unwrap_or(0).is_multiple_of(2);
    let mut result =
        [PropGlyphContent { glyph: None, local_cell: [0; 2] }; MAX_PROP_GLYPHS_PER_SLOT];
    let mut count = 0;
    {
        let mut add = |x: i8, y: i8, glyph: char| -> Result<(), SceneGenerationError> {
            if count >= MAX_PROP_GLYPHS_PER_SLOT {
                return Err(SceneGenerationError::FixedCapacity);
            }
            result[count] = PropGlyphContent {
                glyph: Some(
                    AuthoredGlyph::new(glyph).map_err(|_| SceneGenerationError::InvalidGlyph)?,
                ),
                local_cell: [x, y],
            };
            count += 1;
            Ok(())
        };
        macro_rules! cells {
        ($(($x:expr, $y:expr, $glyph:expr)),* $(,)?) => {{
            $(add($x, $y, $glyph)?;)*
        }};
    }
        match catalog_id {
            crate::game::habitat::TOKEN_PEBBLE_25K => cells![(0, 0, '▲')],
            crate::game::habitat::TOKEN_SHELL_100K => cells![(0, 0, '◌')],
            crate::game::habitat::TOKEN_SPARK_500K => {
                cells![(0, 0, if twinkle == Some(true) { '✦' } else { '·' })]
            }
            crate::game::habitat::TOKEN_SHARD_1M => cells![(
                0,
                0,
                if species == crate::pet::generation::Species::Glitch {
                    '#'
                } else {
                    '◆'
                }
            )],
            crate::game::habitat::TOKEN_ORBIT_5M => cells![(
                0,
                0,
                if species == crate::pet::generation::Species::Glitch {
                    ']'
                } else {
                    '°'
                }
            )],
            crate::game::habitat::TOKEN_LANTERN_10M => {
                cells![(0, 0, if twinkle == Some(true) { '☼' } else { '○' })]
            }
            crate::game::habitat::TOKEN_MOSS_TUFT_250K => {
                if bloom == Some(true) {
                    cells![
                        (0, 0, '*'),
                        (2, 0, '*'),
                        (0, 1, '▂'),
                        (1, 1, if phase { '▃' } else { '▂' }),
                        (2, 1, '▂')
                    ]
                } else {
                    cells![
                        (0, 0, '▂'),
                        (1, 0, if phase { '▃' } else { '▂' }),
                        (2, 0, '▂')
                    ]
                }
            }
            crate::game::habitat::TOKEN_FRIENDLY_CLOUD_750K => cells![
                (1, 0, '☁'),
                (0, 1, if phase { '◦' } else { '˙' }),
                (1, 1, '◡'),
                (2, 1, if phase { '◦' } else { '˙' })
            ],
            crate::game::habitat::TOKEN_TREASURE_CHEST_2M => {
                if lid_open == Some(true) {
                    cells![
                        (0, 0, '╲'),
                        (1, 0, '✦'),
                        (2, 0, '╱'),
                        (0, 1, '▣'),
                        (1, 1, '◆'),
                        (2, 1, '▣')
                    ]
                } else {
                    cells![
                        (0, 0, '╭'),
                        (1, 0, '─'),
                        (2, 0, '╮'),
                        (0, 1, '▣'),
                        (1, 1, '◆'),
                        (2, 1, '▣')
                    ]
                }
            }
            crate::game::habitat::TOKEN_HANGING_VINE_25M => cells![
                (1, 0, if bloom == Some(true) { '*' } else { '╽' }),
                (1, 1, '┃'),
                (0, 2, if phase { '╱' } else { '╲' }),
                (1, 2, '┃'),
                (2, 2, if phase { '╲' } else { '╱' })
            ],
            crate::game::habitat::TOKEN_REEDS_5M => cells![
                (0, 0, if bloom == Some(true) { '*' } else { '╷' }),
                (1, 0, '│'),
                (2, 0, if phase { '╵' } else { '╷' }),
                (0, 1, '│'),
                (1, 1, '┃'),
                (2, 1, '│')
            ],
            crate::game::habitat::TOKEN_GEODE_50M => cells![
                (0, 0, if phase { '◆' } else { '◇' }),
                (1, 0, if phase { '◇' } else { '◆' }),
                (2, 0, if phase { '◆' } else { '◇' }),
                (0, 1, '◇'),
                (1, 1, '◈'),
                (2, 1, '◇'),
                (0, 2, '◣'),
                (1, 2, '▼'),
                (2, 2, '◢')
            ],
            crate::game::habitat::TOKEN_BONSAI_100M => cells![
                (0, 0, '*'),
                (1, 0, '▓'),
                (2, 0, '*'),
                (0, 1, '╲'),
                (1, 1, '┃'),
                (2, 1, '╱'),
                (0, 2, '▂'),
                (1, 2, '▃'),
                (2, 2, '▂')
            ],
            crate::game::habitat::TOKEN_CONSTELLATION_250M => cells![
                (0, 0, if phase { '✦' } else { '·' }),
                (1, 0, if phase { '·' } else { '✦' }),
                (2, 0, if phase { '✦' } else { '·' }),
                (0, 1, '·'),
                (1, 1, '*'),
                (2, 1, '·'),
                (0, 2, '✦'),
                (1, 2, '·'),
                (2, 2, '✦')
            ],
            crate::game::habitat::TOKEN_AURORA_500M => cells![
                (0, 0, if phase { '✦' } else { '·' }),
                (2, 0, if phase { '·' } else { '✦' }),
                (4, 0, if phase { '✦' } else { '·' }),
                (0, 1, '╿'),
                (2, 1, '╿'),
                (4, 1, '╿'),
                (0, 2, '┊'),
                (2, 2, '┊'),
                (4, 2, '┊')
            ],
            crate::game::habitat::TOKEN_MOON_1B => cells![
                (1, 0, '·'),
                (0, 1, '·'),
                (1, 1, '◑'),
                (2, 1, '·'),
                (1, 2, '·'),
                (3, 1, if phase { '✦' } else { '·' })
            ],
            crate::game::habitat::CODEX_SIGNAL_LAMP => cells![
                (0, 0, '╷'),
                (0, 1, if phase { '◉' } else { '○' }),
                (0, 2, '╵')
            ],
            crate::game::habitat::HEAVY_SESSION_PLANTER => cells![
                (1, 0, if bloom == Some(true) { '*' } else { 'ѱ' }),
                (0, 1, '╲'),
                (1, 1, '┃'),
                (2, 1, '╱'),
                (1, 2, '◌')
            ],
            crate::game::habitat::WILT_RECOVERY_SPROUT | crate::game::habitat::RETURN_SPROUT => {
                cells![(1, 0, '╿'), (0, 1, '╲'), (1, 1, '┃'), (2, 1, '╱')]
            }
            crate::game::habitat::FIRST_ENSEMBLE_DAY => {
                cells![(0, 0, '✦'), (1, 0, '◈'), (2, 0, '✦')]
            }
            _ => return Err(SceneGenerationError::UnknownAuthoredIdentity),
        }
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
        Some(if snapshot.frame.asleep { 0.65 } else { 1.0 }),
    )?;
    set_node(
        &mut frame,
        "pet.particles",
        None,
        Some(!snapshot.frame.asleep),
        None,
    )?;
    let (status_visible, status_opacity) = super::super::canonical_activity_pulse_state(snapshot);
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

    for semantic in &snapshot.content.prop_animation_states {
        let slot = usize::from(semantic.stable_order);
        let origin = [
            semantic.origin_points[0],
            layout.height_points - semantic.origin_points[1],
        ];
        let motion = match semantic.motion_phase {
            Some(phase) if !phase.is_multiple_of(2) => [0.0, 1.0],
            Some(_) => [0.0, 0.0],
            None => [0.0, 0.0],
        };
        frame.prop_slots[slot] = PropFrameSlot {
            slot: semantic.stable_order,
            visible: true,
            origin_points: origin,
            motion_offset_points: motion,
            opacity: if snapshot.frame.asleep { 0.72 } else { 1.0 },
        };
    }
    for semantic in &snapshot.content.tank_animation_states {
        let slot = usize::from(semantic.stable_order);
        let mut cells = [TankCellFrame {
            visible: false,
            position_points: [0.0; 2],
            layer: InstanceLayer::Behind,
            bounds_points: [0.0; 4],
        }; MAX_TANK_GLYPHS_PER_SLOT];
        for (index, cell) in semantic.cells.iter().enumerate() {
            let layer = match cell.layer {
                crate::presentation::companion_scene::TankLayerSnapshot::Behind => InstanceLayer::Behind,
                crate::presentation::companion_scene::TankLayerSnapshot::Foreground
                | crate::presentation::companion_scene::TankLayerSnapshot::BehindAnchorForegroundHost => InstanceLayer::Foreground,
            };
            let bounds = semantic.bounds_points.unwrap_or([
                cell.position_points[0],
                cell.position_points[1],
                0.0,
                0.0,
            ]);
            cells[index] = TankCellFrame {
                visible: semantic.visible,
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
            slot: semantic.stable_order,
            visible: semantic.visible,
            origin_points: [
                semantic.origin_points[0],
                layout.height_points - semantic.origin_points[1],
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
    for source in &snapshot.frame.hud_instances {
        let slot = usize::from(source.slot);
        let occupied = snapshot.content.hud_glyphs[slot].glyph.is_some();
        frame.hud_slots[slot] = HudFrameSlot {
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
    frame.gauges = snapshot.frame.gauges.map(gauge_value);
    frame.dim_amount = snapshot.frame.dim_amount;
    frame.lights.clear();
    Ok(frame)
}
