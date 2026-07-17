use super::{
    CompanionSceneSnapshot, TankAnimationSnapshot, COMPANION_RENDERER_SCHEMA_VERSION,
    COMPANION_SCENE_SCHEMA_VERSION, PET_LATTICE_HEIGHT, PET_LATTICE_SLOTS, PET_LATTICE_WIDTH,
};
use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};
use std::sync::Arc;

macro_rules! counter_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}

counter_type!(DeviceEpoch);
counter_type!(SurfaceEpoch);
counter_type!(LayoutGeneration);
counter_type!(ResourceGeneration);
counter_type!(SemanticRevision);
counter_type!(FrameRevision);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct SceneGenerationKey {
    pub device: DeviceEpoch,
    pub layout: LayoutGeneration,
    pub resources: ResourceGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct AppliedRevisions {
    pub semantic: SemanticRevision,
    pub frame: FrameRevision,
}

impl AppliedRevisions {
    pub const fn new(semantic: u64, frame: u64) -> Self {
        Self {
            semantic: SemanticRevision(semantic),
            frame: FrameRevision(frame),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct SceneVersion {
    pub generation: SceneGenerationKey,
    pub surface: SurfaceEpoch,
    pub applied: AppliedRevisions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayoutChangeMask(u64);

impl LayoutChangeMask {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const LOGICAL_EXTENT: Self = Self(1 << 0);
    pub(crate) const PET_TOPOLOGY: Self = Self(1 << 1);
    pub(crate) const ROOM_TOPOLOGY: Self = Self(1 << 2);
    pub(crate) const PROP_CAST: Self = Self(1 << 3);
    pub(crate) const TANK_CAST: Self = Self(1 << 4);
    pub(crate) const GLYPH_GRID: Self = Self(1 << 5);

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceChangeMask(u64);

impl ResourceChangeMask {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const PET_ART: Self = Self(1 << 0);
    pub(crate) const ROOM_AUTHORED: Self = Self(1 << 1);
    pub(crate) const PROP_AUTHORED: Self = Self(1 << 2);
    pub(crate) const TANK_AUTHORED: Self = Self(1 << 3);
    // Reserved named families keep Task 5 additions additive rather than
    // changing the reconciliation result shape.
    pub(crate) const AMBIENT_AUTHORED: Self = Self(1 << 4);
    pub(crate) const MATERIAL_CONTRACT: Self = Self(1 << 5);
    pub(crate) const BACKING_SCALE_ATLAS: Self = Self(1 << 6);
    pub(crate) const SURFACE_RECOVERY: Self = Self(1 << 7);
    pub(crate) const DEVICE_RECOVERY: Self = Self(1 << 8);

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[cfg(test)]
    const fn is_named(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SemanticChangeMask(u64);

impl SemanticChangeMask {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const PET_ART: Self = Self(1 << 0);
    pub(crate) const PALETTE: Self = Self(1 << 1);
    pub(crate) const PROP: Self = Self(1 << 2);
    pub(crate) const TANK: Self = Self(1 << 3);
    pub(crate) const AMBIENT: Self = Self(1 << 4);
    pub(crate) const MOOD_WEATHER: Self = Self(1 << 6);
    pub(crate) const ROOM_GLYPHS: Self = Self(1 << 7);

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[cfg(test)]
    const fn is_named(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameChangeMask(u64);

impl FrameChangeMask {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const CAMERA: Self = Self(1 << 0);
    pub(crate) const PET_TRANSFORM: Self = Self(1 << 1);
    pub(crate) const PROP_TRANSFORMS: Self = Self(1 << 2);
    pub(crate) const TANK_INSTANCES: Self = Self(1 << 3);
    pub(crate) const AMBIENT_INSTANCES: Self = Self(1 << 4);
    pub(crate) const STATUS_VISIBILITY: Self = Self(1 << 5);
    pub(crate) const GAUGES: Self = Self(1 << 6);
    pub(crate) const DIM: Self = Self(1 << 7);
    pub(crate) const LIGHTS: Self = Self(1 << 8);
    pub(crate) const ROOM_GLYPHS: Self = Self(1 << 9);
    pub(crate) const TROUBLE_VISIBILITY: Self = Self(1 << 10);

    fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[cfg(test)]
    const fn is_named(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnapshotChangeSet {
    layout: LayoutChangeMask,
    resources: ResourceChangeMask,
    semantic: SemanticChangeMask,
    frame: FrameChangeMask,
}

impl SnapshotChangeSet {
    const NONE: Self = Self {
        layout: LayoutChangeMask::NONE,
        resources: ResourceChangeMask::NONE,
        semantic: SemanticChangeMask::NONE,
        frame: FrameChangeMask::NONE,
    };

    pub(crate) const fn requires_generation(self) -> bool {
        !self.layout.is_empty() || !self.resources.is_empty()
    }

    pub(crate) const fn layout(self) -> LayoutChangeMask {
        self.layout
    }

    pub(crate) const fn resources(self) -> ResourceChangeMask {
        self.resources
    }

    pub(crate) const fn semantic(self) -> SemanticChangeMask {
        self.semantic
    }

    pub(crate) const fn frame(self) -> FrameChangeMask {
        self.frame
    }

    const fn has_semantic(self) -> bool {
        !self.semantic.is_empty()
    }

    const fn has_frame(self) -> bool {
        !self.frame.is_empty()
    }

    #[cfg(test)]
    const fn families(self) -> ChangeFamilies {
        let mut bits = 0;
        if self.requires_generation() {
            bits |= ChangeFamilies::GENERATION.0;
        }
        if self.has_semantic() {
            bits |= ChangeFamilies::SEMANTIC.0;
        }
        if self.has_frame() {
            bits |= ChangeFamilies::FRAME.0;
        }
        ChangeFamilies(bits)
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChangeFamilies(u8);

#[cfg(test)]
impl ChangeFamilies {
    const NONE: Self = Self(0);
    const GENERATION: Self = Self(1 << 0);
    const SEMANTIC: Self = Self(1 << 1);
    const FRAME: Self = Self(1 << 2);
}

fn prop_topology_changed(
    previous: &super::PropTopologySnapshot,
    newest: &super::PropTopologySnapshot,
) -> bool {
    previous.catalog_id != newest.catalog_id
        || previous.stable_order != newest.stable_order
        || previous.zone != newest.zone
        || previous.authored_depth != newest.authored_depth
        || previous.shadow_profile != newest.shadow_profile
        || previous.presentation_motion != newest.presentation_motion
}

fn tank_topology_changed(
    previous: &super::TankTopologySnapshot,
    newest: &super::TankTopologySnapshot,
) -> bool {
    previous.catalog_id != newest.catalog_id
        || previous.stable_order != newest.stable_order
        || previous.route != newest.route
        || previous.authored_depth != newest.authored_depth
}

fn classify_tank_changes(
    previous: &TankAnimationSnapshot,
    newest: &TankAnimationSnapshot,
    changes: &mut SnapshotChangeSet,
) {
    if previous.visible != newest.visible
        || previous.sprite_variant != newest.sprite_variant
        || previous.anemone_morph != newest.anemone_morph
        || previous.color_srgb8 != newest.color_srgb8
        || previous.bold != newest.bold
        || previous.cells.len() != newest.cells.len()
        || previous
            .cells
            .iter()
            .zip(&newest.cells)
            .any(|(left, right)| left.glyph != right.glyph || left.layer != right.layer)
    {
        changes.semantic.insert(SemanticChangeMask::TANK);
    }
    // cadence_ms and calm are producer inputs already reflected in the resolved
    // cells/placement. They are not independent render state.
}

fn canonical_tank_frame_changed(
    previous: &super::TankFrameSnapshot,
    newest: &super::TankFrameSnapshot,
) -> bool {
    if previous.visible != newest.visible
        || previous.origin_points != newest.origin_points
        || previous.cells.len() != newest.cells.len()
    {
        return true;
    }
    previous
        .cells
        .iter()
        .zip(&newest.cells)
        .any(|(left, right)| {
            let left_bounds = previous.bounds_points.unwrap_or([
                left.position_points[0],
                left.position_points[1],
                0.0,
                0.0,
            ]);
            let right_bounds = newest.bounds_points.unwrap_or([
                right.position_points[0],
                right.position_points[1],
                0.0,
                0.0,
            ]);
            left.position_points != right.position_points || left_bounds != right_bounds
        })
}

fn visible_pet_roles_changed(
    previous: &CompanionSceneSnapshot,
    newest: &CompanionSceneSnapshot,
) -> bool {
    for (row, line) in previous.content.pet_lines.iter().enumerate() {
        for (column, glyph) in line.chars().enumerate() {
            if glyph == ' ' {
                continue;
            }
            let role_at = |snapshot: &CompanionSceneSnapshot| {
                snapshot
                    .content
                    .pet_roles
                    .iter()
                    .find(|span| {
                        usize::from(span.line_index) == row
                            && usize::from(span.start_char) <= column
                            && column < usize::from(span.end_char)
                    })
                    .map_or("body", |span| span.role)
            };
            if role_at(previous) != role_at(newest) {
                return true;
            }
        }
    }
    false
}

fn canonical_ambient_frames_changed(
    previous: &CompanionSceneSnapshot,
    newest: &CompanionSceneSnapshot,
) -> bool {
    previous
        .frame
        .ambient_instances
        .iter()
        .zip(&previous.content.ambient_semantics)
        .zip(
            newest
                .frame
                .ambient_instances
                .iter()
                .zip(&newest.content.ambient_semantics),
        )
        .any(|((left, left_content), (right, right_content))| {
            let left = if left_content.kind.is_some() {
                (left.visible, left.position_points, left.opacity)
            } else {
                (false, [0.0; 2], 0.0)
            };
            let right = if right_content.kind.is_some() {
                (right.visible, right.position_points, right.opacity)
            } else {
                (false, [0.0; 2], 0.0)
            };
            left != right
        })
}

pub(crate) fn classify_snapshot_changes(
    previous: &CompanionSceneSnapshot,
    newest: &CompanionSceneSnapshot,
) -> SnapshotChangeSet {
    let mut changes = SnapshotChangeSet::NONE;

    if previous.topology.layout.width_points != newest.topology.layout.width_points
        || previous.topology.layout.height_points != newest.topology.layout.height_points
    {
        changes.layout.insert(LayoutChangeMask::LOGICAL_EXTENT);
    }
    if previous.topology.glyph_grid != newest.topology.glyph_grid {
        changes.layout.insert(LayoutChangeMask::GLYPH_GRID);
    }
    if previous.topology.pet.species != newest.topology.pet.species
        || previous.topology.pet.stage != newest.topology.pet.stage
        || previous.topology.pet.lattice.identity != newest.topology.pet.lattice.identity
        || previous.topology.pet.lattice.width != newest.topology.pet.lattice.width
        || previous.topology.pet.lattice.height != newest.topology.pet.lattice.height
        || previous.topology.pet.lattice.slot_count != newest.topology.pet.lattice.slot_count
    {
        changes.layout.insert(LayoutChangeMask::PET_TOPOLOGY);
        changes.resources.insert(ResourceChangeMask::PET_ART);
    }
    if previous.topology.room.primary_biome != newest.topology.room.primary_biome
        || previous.topology.room.secondary_biome != newest.topology.room.secondary_biome
        || previous.topology.room.species_dialect != newest.topology.room.species_dialect
    {
        changes.layout.insert(LayoutChangeMask::ROOM_TOPOLOGY);
        changes.resources.insert(ResourceChangeMask::ROOM_AUTHORED);
    }
    if previous.topology.visible_props.len() != newest.topology.visible_props.len()
        || previous
            .topology
            .visible_props
            .iter()
            .zip(&newest.topology.visible_props)
            .any(|(left, right)| prop_topology_changed(left, right))
    {
        changes.layout.insert(LayoutChangeMask::PROP_CAST);
        changes.resources.insert(ResourceChangeMask::PROP_AUTHORED);
    }
    if previous.topology.visible_tank_inhabitants.len()
        != newest.topology.visible_tank_inhabitants.len()
        || previous
            .topology
            .visible_tank_inhabitants
            .iter()
            .zip(&newest.topology.visible_tank_inhabitants)
            .any(|(left, right)| tank_topology_changed(left, right))
    {
        changes.layout.insert(LayoutChangeMask::TANK_CAST);
        changes.resources.insert(ResourceChangeMask::TANK_AUTHORED);
    }

    if previous.content.pet_lines != newest.content.pet_lines
        || visible_pet_roles_changed(previous, newest)
    {
        changes.semantic.insert(SemanticChangeMask::PET_ART);
    }
    if previous.content.room_glyphs != newest.content.room_glyphs {
        changes.semantic.insert(SemanticChangeMask::ROOM_GLYPHS);
    }
    if previous.frame.room_glyphs != newest.frame.room_glyphs {
        changes.frame.insert(FrameChangeMask::ROOM_GLYPHS);
    }
    if previous.content.room_weather != newest.content.room_weather
        || previous.content.day_phase != newest.content.day_phase
    {
        changes.semantic.insert(SemanticChangeMask::MOOD_WEATHER);
    }
    if previous.content.mood != newest.content.mood {
        changes.semantic.insert(SemanticChangeMask::MOOD_WEATHER);
    }
    if previous.content.palette.body != newest.content.palette.body
        || previous.content.palette.body_glow != newest.content.palette.body_glow
        || previous.content.palette.eye != newest.content.palette.eye
        || previous.content.palette.mouth != newest.content.palette.mouth
        || previous.content.palette.accent != newest.content.palette.accent
        || previous.content.palette.pattern != newest.content.palette.pattern
        || previous.content.palette.particle != newest.content.palette.particle
        || previous.content.palette.corruption != newest.content.palette.corruption
    {
        changes.semantic.insert(SemanticChangeMask::PALETTE);
    }

    if previous.content.prop_animation_states.len() != newest.content.prop_animation_states.len() {
        changes.semantic.insert(SemanticChangeMask::PROP);
        changes.frame.insert(FrameChangeMask::PROP_TRANSFORMS);
    }
    for (left, right) in previous
        .content
        .prop_animation_states
        .iter()
        .zip(&newest.content.prop_animation_states)
    {
        if left.kind != right.kind
            || left.sprite_phase != right.sprite_phase
            || left.twinkle_active != right.twinkle_active
            || left.motion_phase != right.motion_phase
            || left.chest_lid_open != right.chest_lid_open
            || left.bloom_active != right.bloom_active
        {
            changes.semantic.insert(SemanticChangeMask::PROP);
        }
    }

    if previous.frame.prop_instances != newest.frame.prop_instances {
        changes.frame.insert(FrameChangeMask::PROP_TRANSFORMS);
    }

    if previous.content.tank_animation_states.len() != newest.content.tank_animation_states.len() {
        changes.semantic.insert(SemanticChangeMask::TANK);
        changes.frame.insert(FrameChangeMask::TANK_INSTANCES);
    }
    for (left, right) in previous
        .content
        .tank_animation_states
        .iter()
        .zip(&newest.content.tank_animation_states)
    {
        classify_tank_changes(left, right, &mut changes);
    }
    if previous.frame.tank_instances.len() != newest.frame.tank_instances.len()
        || previous
            .frame
            .tank_instances
            .iter()
            .zip(&newest.frame.tank_instances)
            .any(|(left, right)| canonical_tank_frame_changed(left, right))
    {
        changes.frame.insert(FrameChangeMask::TANK_INSTANCES);
    }

    if previous.content.ambient_semantics != newest.content.ambient_semantics {
        changes.semantic.insert(SemanticChangeMask::AMBIENT);
    }
    if canonical_ambient_frames_changed(previous, newest) {
        changes.frame.insert(FrameChangeMask::AMBIENT_INSTANCES);
    }
    if super::canonical_activity_status(previous) != super::canonical_activity_status(newest) {
        changes.frame.insert(FrameChangeMask::STATUS_VISIBILITY);
    }

    if previous.frame.pet_anchor_points[0] != newest.frame.pet_anchor_points[0]
        || previous.frame.pet_anchor_points[1] + previous.frame.bob_offset_y_points
            != newest.frame.pet_anchor_points[1] + newest.frame.bob_offset_y_points
        || previous.frame.pet_depth != newest.frame.pet_depth
        || previous.frame.pet_depth_cue != newest.frame.pet_depth_cue
        || previous.frame.calm != newest.frame.calm
        || previous.frame.facing != newest.frame.facing
    {
        changes.frame.insert(FrameChangeMask::PET_TRANSFORM);
    }
    if previous.frame.asleep != newest.frame.asleep {
        changes.frame.insert(FrameChangeMask::PET_TRANSFORM);
        changes.frame.insert(FrameChangeMask::STATUS_VISIBILITY);
        changes.frame.insert(FrameChangeMask::PROP_TRANSFORMS);
    }
    if previous.frame.helper_trouble != newest.frame.helper_trouble {
        changes.frame.insert(FrameChangeMask::TROUBLE_VISIBILITY);
    }
    if previous.frame.gauge_fractions != newest.frame.gauge_fractions
        || previous.frame.gauge_levels != newest.frame.gauge_levels
    {
        changes.frame.insert(FrameChangeMask::GAUGES);
    }
    if previous.frame.dim_amount != newest.frame.dim_amount
        || previous.frame.dimmed != newest.frame.dimmed
    {
        changes.frame.insert(FrameChangeMask::DIM);
    }
    // Raw clocks and grid helpers are producer inputs.
    // Only the canonical point-space/content/frame mirrors above allocate a
    // revision, so changing redundant metadata with identical output is a no-op.

    changes
}

fn rebase_semantic_transition_frames(
    previous: &CompanionSceneSnapshot,
    newest: &mut CompanionSceneSnapshot,
    semantic_revision: SemanticRevision,
) {
    for newest_frame in &mut newest.frame.prop_instances {
        let Some(topology) = newest
            .topology
            .visible_props
            .iter()
            .find(|prop| prop.stable_order == newest_frame.slot)
        else {
            continue;
        };
        let Some(previous_frame) = previous
            .frame
            .prop_instances
            .iter()
            .find(|frame| frame.slot == newest_frame.slot)
        else {
            continue;
        };
        let previous_semantic = previous
            .content
            .prop_animation_states
            .iter()
            .find(|state| state.stable_order == newest_frame.slot);
        let newest_semantic = newest
            .content
            .prop_animation_states
            .iter()
            .find(|state| state.stable_order == newest_frame.slot);
        let transitioned = previous_semantic
            .zip(newest_semantic)
            .is_some_and(|(left, right)| {
                left.motion_phase != right.motion_phase
                    || left.chest_lid_open != right.chest_lid_open
                    || left.sprite_phase != right.sprite_phase
                    || left.twinkle_active != right.twinkle_active
            });
        if transitioned {
            match topology.presentation_motion {
                super::PropPresentationMotion::TwoPoseEase { duration_ms, curve } => {
                    let source_pose = previous_frame.transition.map_or_else(
                        || {
                            super::input::prop_two_pose_target(
                                topology.catalog_id,
                                previous_semantic.and_then(|state| state.motion_phase),
                            )
                        },
                        |anchor| {
                            super::input::resolve_prop_transition(anchor, previous.frame.elapsed_ms)
                        },
                    );
                    let target_pose = super::input::prop_two_pose_target(
                        topology.catalog_id,
                        newest_semantic.and_then(|state| state.motion_phase),
                    );
                    let parallax = [
                        newest_frame.motion_offset_points[0] - target_pose[0],
                        newest_frame.motion_offset_points[1] - target_pose[1],
                    ];
                    newest_frame.motion_offset_points =
                        [source_pose[0] + parallax[0], source_pose[1] + parallax[1]];
                    newest_frame.transition = Some(super::PropTransitionAnchor {
                        source_pose,
                        target_pose,
                        source_opacity: newest_frame.opacity,
                        target_opacity: newest_frame.opacity,
                        semantic_revision,
                        started_at_monotonic_ms: newest.frame.elapsed_ms,
                        duration_ms,
                        curve,
                    });
                }
                super::PropPresentationMotion::TwinkleFade { attack_ms, release_ms } => {
                    let target_opacity = newest_frame.opacity;
                    newest_frame.opacity = previous_frame.opacity;
                    newest_frame.transition = Some(super::PropTransitionAnchor {
                        source_pose: [0.0; 2],
                        target_pose: [0.0; 2],
                        source_opacity: previous_frame.opacity,
                        target_opacity,
                        semantic_revision,
                        started_at_monotonic_ms: newest.frame.elapsed_ms,
                        duration_ms: if target_opacity >= previous_frame.opacity {
                            attack_ms
                        } else {
                            release_ms
                        },
                        curve: super::EaseCurve::SmoothStep,
                    });
                }
                super::PropPresentationMotion::Static
                | super::PropPresentationMotion::Sway { .. }
                | super::PropPresentationMotion::Hover { .. } => {}
            }
        } else {
            newest_frame.motion_offset_points = previous_frame.motion_offset_points;
            newest_frame.opacity = previous_frame.opacity;
            newest_frame.transition = previous_frame.transition;
        }
    }

    for newest_frame in &mut newest.frame.tank_instances {
        let Some(previous_frame) = previous
            .frame
            .tank_instances
            .iter()
            .find(|frame| frame.slot == newest_frame.slot)
        else {
            continue;
        };
        for (newest_cell, previous_cell) in newest_frame.cells.iter_mut().zip(&previous_frame.cells)
        {
            let target_position = newest_cell.target_position_points;
            let parallax = [
                newest_cell.position_points[0] - target_position[0],
                newest_cell.position_points[1] - target_position[1],
            ];
            let source_position = super::input::resolve_tank_transition_position(
                previous_frame,
                previous_cell,
                previous.frame.elapsed_ms,
            );
            newest_cell.source_position_points = source_position;
            newest_cell.position_points = [
                source_position[0] + parallax[0],
                source_position[1] + parallax[1],
            ];
        }
        newest_frame.semantic_revision = semantic_revision;
        newest_frame.started_at_monotonic_ms = newest.frame.elapsed_ms;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotRejection {
    SchemaVersion,
    RendererSchemaVersion,
    Privacy,
    NonFinite,
    InvalidValue,
    InconsistentIdentity,
    FixedCapacity,
}

fn snapshot_pet_clearance(
    snapshot: &CompanionSceneSnapshot,
) -> crate::presentation::smooth::SmoothBounds {
    let cell = snapshot.topology.glyph_grid.cell_extent_points;
    let final_center = crate::presentation::smooth::SmoothPoint {
        x: snapshot.frame.pet_anchor_points[0] / cell[0] + f32::from(PET_LATTICE_WIDTH) / 2.0,
        // `pet_transform` converts the top-left anchor into Y-up space after
        // adding downward bob, then applies the signed Y-up perspective cue.
        // Converted back to the resolver's Y-down cell space, the rendered
        // center therefore adds bob and subtracts the stored Y-up cue once.
        y: snapshot.frame.pet_anchor_points[1] / cell[1]
            + f32::from(PET_LATTICE_HEIGHT) / 2.0
            + snapshot.frame.bob_offset_y_points / cell[1]
            - snapshot.frame.pet_depth_cue.y_offset_points_up / cell[1],
    };
    let half_w =
        crate::pet::render::ART_WIDTH as f32 / 2.0 * crate::round::depth::SMOOTH_PET_NEAR_SCALE;
    let half_h =
        crate::pet::render::ART_HEIGHT as f32 / 2.0 * crate::round::depth::SMOOTH_PET_NEAR_SCALE;
    crate::presentation::smooth::SmoothBounds {
        min: crate::presentation::smooth::SmoothPoint {
            x: final_center.x - half_w,
            y: final_center.y - half_h,
        },
        max: crate::presentation::smooth::SmoothPoint {
            x: final_center.x + half_w,
            y: final_center.y + half_h,
        },
    }
}

pub(crate) fn validate_snapshot(
    snapshot: &CompanionSceneSnapshot,
) -> Result<(), SnapshotRejection> {
    if snapshot.schema_version != COMPANION_SCENE_SCHEMA_VERSION {
        return Err(SnapshotRejection::SchemaVersion);
    }
    if snapshot.topology.renderer_schema != COMPANION_RENDERER_SCHEMA_VERSION {
        return Err(SnapshotRejection::RendererSchemaVersion);
    }
    let expected = PrivacyProjection::for_surface(PresentationSurface::RoundCompanion);
    if snapshot.privacy != expected {
        return Err(SnapshotRejection::Privacy);
    }
    let layout = snapshot.topology.layout;
    let grid = snapshot.topology.glyph_grid;
    if !layout.width_points.is_finite()
        || !layout.height_points.is_finite()
        || !grid
            .y_up_origin_points
            .iter()
            .chain(&grid.cell_extent_points)
            .all(|value| value.is_finite())
        || !snapshot
            .frame
            .pet_anchor_points
            .iter()
            .all(|value| value.is_finite())
        || !snapshot.frame.pet_depth.is_finite()
        || !snapshot.frame.pet_depth_cue.scale.is_finite()
        || !snapshot.frame.pet_depth_cue.y_offset_points_up.is_finite()
        || !snapshot.frame.pet_depth_cue.opacity.is_finite()
        || !snapshot.frame.pet_depth_cue.saturation.is_finite()
        || !snapshot.frame.activity_opacity.is_finite()
        || !snapshot
            .frame
            .gauge_fractions
            .iter()
            .all(|value| value.is_finite())
        || !snapshot.frame.breath_offset_y_points.is_finite()
        || !snapshot.frame.bob_offset_y_points.is_finite()
        || !snapshot.frame.dim_amount.is_finite()
        || snapshot.frame.prop_instances.iter().any(|state| {
            !state.origin_points.iter().all(|value| value.is_finite())
                || !state
                    .motion_offset_points
                    .iter()
                    .all(|value| value.is_finite())
                || !state.opacity.is_finite()
                || !state.footprint_points.iter().all(|value| value.is_finite())
                || !state.contact_shadow_strength.is_finite()
                || !state
                    .cast_shadow_vector_points
                    .iter()
                    .all(|value| value.is_finite())
                || !state.cast_shadow_softness_points.is_finite()
                || !state.cast_shadow_strength.is_finite()
                || state.transition.is_some_and(|anchor| {
                    !anchor.source_pose.iter().all(|value| value.is_finite())
                        || !anchor.target_pose.iter().all(|value| value.is_finite())
                        || !anchor.source_opacity.is_finite()
                        || !anchor.target_opacity.is_finite()
                })
        })
        || snapshot.frame.tank_instances.iter().any(|state| {
            !state.origin_points.iter().all(|value| value.is_finite())
                || state.cells.iter().any(|cell| {
                    !cell
                        .source_position_points
                        .iter()
                        .all(|value| value.is_finite())
                        || !cell.position_points.iter().all(|value| value.is_finite())
                        || !cell
                            .target_position_points
                            .iter()
                            .all(|value| value.is_finite())
                })
                || state
                    .bounds_points
                    .is_some_and(|bounds| !bounds.iter().all(|value| value.is_finite()))
        })
        || snapshot.frame.ambient_instances.iter().any(|slot| {
            !slot.position_points.iter().all(|value| value.is_finite()) || !slot.opacity.is_finite()
        })
        || snapshot.frame.room_glyphs.iter().any(|slot| {
            !slot.position_points.iter().all(|value| value.is_finite()) || !slot.opacity.is_finite()
        })
    {
        return Err(SnapshotRejection::NonFinite);
    }
    if layout.width_points <= 0.0
        || layout.height_points <= 0.0
        || snapshot.topology.glyph_grid.columns == 0
        || snapshot.topology.glyph_grid.rows == 0
        || !matches!(snapshot.frame.facing, -1 | 1)
        || !(0.0..=1.0).contains(&snapshot.frame.dim_amount)
        || !(-1.0..=1.0).contains(&snapshot.frame.pet_depth)
        || snapshot.frame.pet_depth_cue.scale <= 0.0
        || !(0.0..=1.0).contains(&snapshot.frame.pet_depth_cue.opacity)
        || !(0.0..=1.0).contains(&snapshot.frame.pet_depth_cue.saturation)
        || !(0.0..=1.0).contains(&snapshot.frame.activity_opacity)
        || snapshot
            .frame
            .gauge_fractions
            .iter()
            .enumerate()
            .any(|(index, value)| *value < 0.0 || (index != 2 && *value > 1.0))
        || snapshot
            .frame
            .ambient_instances
            .iter()
            .any(|slot| !(0.0..=1.0).contains(&slot.opacity))
        || snapshot.frame.room_glyphs.iter().any(|slot| {
            !slot.visible || !(0.0..=1.0).contains(&slot.opacity) || slot.opacity == 0.0
        })
        || grid.cell_extent_points.iter().any(|value| *value <= 0.0)
    {
        return Err(SnapshotRejection::InvalidValue);
    }
    if snapshot.topology.pet.lattice.width != PET_LATTICE_WIDTH
        || snapshot.topology.pet.lattice.height != PET_LATTICE_HEIGHT
        || snapshot.topology.pet.lattice.slot_count != PET_LATTICE_SLOTS
        || snapshot.content.pet_lines.len() != usize::from(PET_LATTICE_HEIGHT)
        || snapshot
            .content
            .pet_lines
            .iter()
            .any(|line| line.chars().count() != usize::from(PET_LATTICE_WIDTH))
        || snapshot.content.ambient_semantics.len() != super::scene::MAX_AMBIENT_INSTANCES
        || snapshot.frame.ambient_instances.len() != super::scene::MAX_AMBIENT_INSTANCES
        || snapshot.content.room_glyphs.len() > super::scene::MAX_ROOM_GLYPH_SLOTS
        || snapshot.frame.room_glyphs.len() != snapshot.content.room_glyphs.len()
    {
        return Err(SnapshotRejection::FixedCapacity);
    }
    if snapshot.topology.visible_props.len() > super::MAX_VISIBLE_PROPS
        || snapshot.topology.visible_tank_inhabitants.len() > super::MAX_VISIBLE_TANK_INHABITANTS
    {
        return Err(SnapshotRejection::FixedCapacity);
    }
    if snapshot.content.prop_animation_states.len() != snapshot.topology.visible_props.len()
        || snapshot.content.tank_animation_states.len()
            != snapshot.topology.visible_tank_inhabitants.len()
        || snapshot.frame.prop_instances.len() != snapshot.topology.visible_props.len()
        || snapshot.frame.tank_instances.len() != snapshot.topology.visible_tank_inhabitants.len()
    {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    if snapshot.frame.gauge_levels
        != snapshot
            .frame
            .gauge_fractions
            .map(|value| super::GaugeLevelSnapshot::from_fraction(f64::from(value)))
    {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    if snapshot.frame.dimmed != (snapshot.frame.dim_amount > 0.0) {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    if !snapshot.frame.activity_recent && snapshot.frame.activity_opacity != 0.0 {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    if snapshot.frame.asleep && !snapshot.frame.calm {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    if snapshot.frame.asleep && snapshot.frame.activity_recent {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    let resolved_depth = crate::round::depth::resolve_smooth_depth(
        snapshot.frame.pet_depth,
        crate::round::depth::depth_lifecycle_scale(snapshot.frame.asleep, snapshot.frame.calm),
    )
    .map_err(|_| SnapshotRejection::InvalidValue)?;
    if snapshot.frame.pet_depth_cue
        != (super::DepthCue {
            scale: resolved_depth.scale,
            y_offset_points_up: -resolved_depth.perspective_y
                * snapshot.topology.glyph_grid.cell_extent_points[1],
            opacity: resolved_depth.atmosphere,
            saturation: 1.0,
        })
    {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    let motion_viewport = crate::round::motion::RoundCompanionMotionViewport {
        grid_columns: grid.columns,
        grid_rows: grid.rows,
        width_points: layout.width_points,
        height_points: layout.height_points,
        clearance: crate::round::scene::current_round_motion_clearance(grid.rows),
    };
    if !crate::round::placement::bounds_inside_round_aperture(
        snapshot_pet_clearance(snapshot),
        motion_viewport,
    ) {
        return Err(SnapshotRejection::InvalidValue);
    }
    if snapshot
        .content
        .ambient_semantics
        .iter()
        .enumerate()
        .any(|(index, slot)| usize::from(slot.slot) != index)
        || snapshot
            .frame
            .ambient_instances
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || snapshot
            .content
            .room_glyphs
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
        || snapshot
            .frame
            .room_glyphs
            .iter()
            .enumerate()
            .any(|(index, slot)| usize::from(slot.slot) != index)
    {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    let expected_extent = [
        layout.width_points / f32::from(grid.columns),
        layout.height_points / f32::from(grid.rows),
    ];
    if grid.y_up_origin_points != [0.0, 0.0]
        || grid.cell_extent_points != expected_extent
        || grid.scale != super::LogicalGlyphScale::OneCell
        || grid.anchor != super::LogicalGlyphAnchor::CellBottomLeft
        || snapshot.frame.room_glyphs.iter().any(|slot| {
            slot.grid_cell[0] >= grid.columns
                || slot.grid_cell[1] >= grid.rows
                || slot.position_points
                    != [
                        f32::from(slot.grid_cell[0]) * expected_extent[0],
                        layout.height_points
                            - (f32::from(slot.grid_cell[1]) + 1.0) * expected_extent[1],
                    ]
        })
    {
        return Err(SnapshotRejection::InconsistentIdentity);
    }
    for (index, (topology, content)) in snapshot
        .topology
        .visible_props
        .iter()
        .zip(&snapshot.content.prop_animation_states)
        .enumerate()
    {
        if topology.catalog_id != content.catalog_id
            || topology.stable_order != content.stable_order
            || usize::from(topology.stable_order) != index
            || usize::from(topology.stable_order) >= super::MAX_VISIBLE_PROPS
            || content.bloom_active.is_some()
                != crate::game::habitat::habitat_prop_supports_bloom(content.catalog_id)
        {
            return Err(SnapshotRejection::InconsistentIdentity);
        }
        let frame = &snapshot.frame.prop_instances[index];
        let authored = crate::game::habitat::catalog_prop_by_str(topology.catalog_id);
        let has_cast = frame.cast_shadow_strength > 0.0;
        let has_any_cast_value = frame.cast_shadow_vector_points != [0.0; 2]
            || frame.cast_shadow_softness_points != 0.0
            || frame.cast_shadow_strength != 0.0;
        if frame.slot != topology.stable_order
            || authored.is_none_or(|spec| spec.shadow_profile != topology.shadow_profile)
            || !(0.0..=1.0).contains(&frame.opacity)
            || frame
                .footprint_points
                .into_iter()
                .any(|extent| extent < 0.0)
            || !(0.0..=1.0).contains(&frame.contact_shadow_strength)
            || frame.cast_shadow_softness_points < 0.0
            || !(0.0..=1.0).contains(&frame.cast_shadow_strength)
            || has_cast
                != (frame.cast_shadow_vector_points != [0.0; 2]
                    && frame.cast_shadow_softness_points > 0.0)
            || (has_any_cast_value
                && !matches!(
                    topology.shadow_profile,
                    crate::game::habitat::HabitatPropShadowProfile::Elevated { .. }
                ))
            || (!frame.visible
                && (frame.contact_shadow_strength != 0.0 || frame.cast_shadow_strength != 0.0))
            || (frame.opacity == 0.0 && frame.cast_shadow_strength != 0.0)
            || (has_cast
                && (frame.footprint_points[0] < grid.cell_extent_points[0]
                    || frame.footprint_points[1] < grid.cell_extent_points[1] * 2.0))
            || frame.transition.is_some_and(|anchor| {
                anchor.duration_ms == 0
                    || !(0.0..=1.0).contains(&anchor.source_opacity)
                    || !(0.0..=1.0).contains(&anchor.target_opacity)
                    || !matches!(
                        topology.presentation_motion,
                        super::PropPresentationMotion::TwoPoseEase { .. }
                            | super::PropPresentationMotion::TwinkleFade { .. }
                    )
            })
        {
            return Err(SnapshotRejection::InconsistentIdentity);
        }
    }
    for (index, (topology, content)) in snapshot
        .topology
        .visible_tank_inhabitants
        .iter()
        .zip(&snapshot.content.tank_animation_states)
        .enumerate()
    {
        let expected_paint = crate::presentation::tank_life::tank_paint_for(topology.catalog_id);
        if topology.catalog_id != content.catalog_id
            || topology.stable_order != content.stable_order
            || topology.route != content.route
            || expected_paint.is_none_or(|paint| {
                paint.color_srgb8 != content.color_srgb8 || paint.bold != content.bold
            })
            || usize::from(topology.stable_order) != index
            || usize::from(topology.stable_order) >= super::MAX_VISIBLE_TANK_INHABITANTS
            || content.cells.len() > super::scene::MAX_TANK_GLYPHS_PER_SLOT
        {
            return Err(SnapshotRejection::InconsistentIdentity);
        }
        let frame = &snapshot.frame.tank_instances[index];
        if frame.slot != topology.stable_order
            || frame.visible != content.visible
            || frame.cells.len() != content.cells.len()
            || frame.duration_ms != content.cadence_ms
            || frame.duration_ms == 0
        {
            return Err(SnapshotRejection::InconsistentIdentity);
        }
    }
    for (index, left) in snapshot.topology.visible_props.iter().enumerate() {
        if snapshot.topology.visible_props[index + 1..]
            .iter()
            .any(|right| {
                right.catalog_id == left.catalog_id || right.stable_order == left.stable_order
            })
        {
            return Err(SnapshotRejection::InconsistentIdentity);
        }
    }
    for (index, left) in snapshot
        .topology
        .visible_tank_inhabitants
        .iter()
        .enumerate()
    {
        if snapshot.topology.visible_tank_inhabitants[index + 1..]
            .iter()
            .any(|right| {
                right.catalog_id == left.catalog_id || right.stable_order == left.stable_order
            })
        {
            return Err(SnapshotRejection::InconsistentIdentity);
        }
    }
    for role in &snapshot.content.pet_roles {
        if role.line_index >= PET_LATTICE_HEIGHT
            || role.start_char >= role.end_char
            || role.end_char > PET_LATTICE_WIDTH
        {
            return Err(SnapshotRejection::InvalidValue);
        }
    }
    for state in &snapshot.frame.tank_instances {
        if state
            .bounds_points
            .is_some_and(|bounds| bounds[2] < 0.0 || bounds[3] < 0.0)
        {
            return Err(SnapshotRejection::InvalidValue);
        }
    }
    for slot in &snapshot.content.ambient_semantics {
        if slot.kind.is_some() != slot.glyph.is_some() {
            return Err(SnapshotRejection::InvalidValue);
        }
    }
    for (semantic, frame) in snapshot
        .content
        .ambient_semantics
        .iter()
        .zip(&snapshot.frame.ambient_instances)
    {
        if semantic.kind.is_none()
            && semantic.glyph.is_none()
            && (frame.visible
                || frame
                    .position_points
                    .iter()
                    .any(|value| value.to_bits() != 0)
                || frame.opacity.to_bits() != 0)
        {
            return Err(SnapshotRejection::InconsistentIdentity);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterKind {
    DeviceEpoch,
    SurfaceEpoch,
    LayoutGeneration,
    ResourceGeneration,
    SemanticRevision,
    FrameRevision,
    RequestId,
    ActivationAttemptId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeError {
    SnapshotRejected(SnapshotRejection),
    StaleSemanticBase {
        expected: SemanticRevision,
        actual: SemanticRevision,
    },
    CounterOverflow(CounterKind),
    RecoveryActionRejected,
    Shutdown,
}

impl From<SnapshotRejection> for RuntimeError {
    fn from(value: SnapshotRejection) -> Self {
        Self::SnapshotRejected(value)
    }
}

fn increment(value: u64, counter: CounterKind) -> Result<u64, RuntimeError> {
    value
        .checked_add(1)
        .ok_or(RuntimeError::CounterOverflow(counter))
}

#[derive(Debug)]
pub(crate) struct CompanionSceneReconciler {
    snapshot: Arc<CompanionSceneSnapshot>,
    layout_generation: LayoutGeneration,
    semantic_revision: SemanticRevision,
    frame_revision: FrameRevision,
}

impl CompanionSceneReconciler {
    pub(crate) fn new(snapshot: Arc<CompanionSceneSnapshot>) -> Result<Self, RuntimeError> {
        validate_snapshot(&snapshot)?;
        Ok(Self {
            snapshot,
            layout_generation: LayoutGeneration(1),
            semantic_revision: SemanticRevision(1),
            frame_revision: FrameRevision(1),
        })
    }

    pub(crate) fn snapshot(&self) -> &Arc<CompanionSceneSnapshot> {
        &self.snapshot
    }

    pub(crate) const fn layout_generation(&self) -> LayoutGeneration {
        self.layout_generation
    }

    pub(crate) const fn applied_revisions(&self) -> AppliedRevisions {
        AppliedRevisions {
            semantic: self.semantic_revision,
            frame: self.frame_revision,
        }
    }

    fn commit_prepared(&mut self, prepared: &PreparedSnapshotUpdate) {
        self.snapshot = Arc::clone(&prepared.snapshot);
        self.layout_generation = prepared.layout_generation;
        self.semantic_revision = prepared.applied.semantic;
        self.frame_revision = prepared.applied.frame;
    }
}

#[derive(Debug)]
pub(crate) struct PreparedSnapshotUpdate {
    expected_snapshot: Arc<CompanionSceneSnapshot>,
    expected_layout: LayoutGeneration,
    expected_applied: AppliedRevisions,
    expected_resources: ResourceGeneration,
    expected_next_request: RequestId,
    expected_device: DeviceEpoch,
    expected_surface: SurfaceEpoch,
    expected_visibility: RuntimeVisibility,
    expected_worker: WorkerState,
    expected_pending_request: Option<RequestId>,
    expected_recovery: RecoveryState,
    expected_hidden_latest: Option<Arc<CompanionSceneSnapshot>>,
    snapshot: Arc<CompanionSceneSnapshot>,
    changes: SnapshotChangeSet,
    layout_generation: LayoutGeneration,
    applied: AppliedRevisions,
    generation: Option<PreparedGenerationRequest>,
    hidden_origin: bool,
}

#[derive(Debug)]
pub(crate) struct PreparedReveal {
    update: PreparedSnapshotUpdate,
}

impl PreparedReveal {
    pub(crate) const fn update(&self) -> &PreparedSnapshotUpdate {
        &self.update
    }
}

impl PreparedSnapshotUpdate {
    pub(crate) const fn changes(&self) -> SnapshotChangeSet {
        self.changes
    }

    pub(crate) fn snapshot(&self) -> &Arc<CompanionSceneSnapshot> {
        &self.snapshot
    }

    pub(crate) const fn semantic_revision(&self) -> SemanticRevision {
        self.applied.semantic
    }

    pub(crate) const fn frame_revision(&self) -> FrameRevision {
        self.applied.frame
    }
}

#[derive(Debug)]
pub(crate) struct PreparedFrameProjection {
    update: PreparedSnapshotUpdate,
}

impl PreparedFrameProjection {
    pub(crate) const fn frame_revision(&self) -> FrameRevision {
        self.update.applied.frame
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreparedGenerationRequest {
    request_id: RequestId,
    next_request_id: RequestId,
    resources: ResourceGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparedCommitError {
    StaleBase,
    Shutdown,
    Projection(super::scene::SceneDeltaApplyError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RequestId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestIdentity {
    request_id: RequestId,
    key: SceneGenerationKey,
    surface: SurfaceEpoch,
    source: AppliedRevisions,
}

impl RequestIdentity {
    pub(crate) const fn request_id(self) -> RequestId {
        self.request_id
    }

    pub(crate) const fn key(self) -> SceneGenerationKey {
        self.key
    }

    pub(crate) const fn surface(self) -> SurfaceEpoch {
        self.surface
    }

    pub(crate) const fn source(self) -> AppliedRevisions {
        self.source
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct GenerationRequest {
    request_id: RequestId,
    key: SceneGenerationKey,
    surface: SurfaceEpoch,
    source: AppliedRevisions,
    snapshot: Arc<CompanionSceneSnapshot>,
    seal: Arc<()>,
}

impl GenerationRequest {
    pub(crate) const fn identity(&self) -> RequestIdentity {
        RequestIdentity {
            request_id: self.request_id,
            key: self.key,
            surface: self.surface,
            source: self.source,
        }
    }

    pub(crate) const fn request_id(&self) -> RequestId {
        self.request_id
    }

    pub(crate) const fn key(&self) -> SceneGenerationKey {
        self.key
    }

    pub(crate) const fn surface(&self) -> SurfaceEpoch {
        self.surface
    }

    pub(crate) const fn source(&self) -> AppliedRevisions {
        self.source
    }

    pub(crate) fn snapshot(&self) -> &Arc<CompanionSceneSnapshot> {
        &self.snapshot
    }

    pub(crate) fn build_scene_generation(
        &self,
    ) -> Result<super::scene::SceneGenerationData, super::scene::SceneGenerationError> {
        super::scene::build_scene_generation_for_request(
            Arc::clone(&self.snapshot),
            self.key,
            self.source,
            Arc::clone(&self.seal),
        )
    }

    #[cfg(test)]
    pub(crate) fn accept(self) -> AcceptedGenerationCandidate {
        let built = self
            .build_scene_generation()
            .expect("valid test generation");
        self.accept_generation(built)
            .expect("matching test request")
    }

    pub(crate) fn accept_generation(
        self,
        built: super::scene::SceneGenerationData,
    ) -> Result<AcceptedGenerationCandidate, GenerationAcceptanceError> {
        if !built.matches_request(&self.seal, self.key, self.source, &self.snapshot) {
            return Err(GenerationAcceptanceError::IdentityMismatch);
        }
        Ok(AcceptedGenerationCandidate {
            request_id: self.request_id,
            key: self.key,
            applied: self.source,
            generation: built,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationAcceptanceError {
    IdentityMismatch,
}

#[derive(Debug, PartialEq)]
pub(crate) struct AcceptedGenerationCandidate {
    request_id: RequestId,
    key: SceneGenerationKey,
    applied: AppliedRevisions,
    generation: super::scene::SceneGenerationData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeDisposition {
    Unchanged,
    SnapshotCommitted(SnapshotChangeSet),
    GenerationQueued(RequestId),
    GenerationStarted(RequestId),
    CandidateReady(RequestId),
    CandidateDropped(RequestId),
    Activation(ActivationTransition),
    HiddenCoalesced,
    Revealed,
    SurfaceRebound(SurfaceEpoch),
    Shutdown,
    DroppedStale,
}

#[derive(Debug, PartialEq)]
pub(crate) struct RuntimeEffects {
    disposition: RuntimeDisposition,
    cancel_worker: Option<CancelWorker>,
    start_worker: Option<GenerationRequest>,
    drop_candidate: Option<DropCandidate>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CancelWorker {
    request_id: RequestId,
}

impl CancelWorker {
    pub(crate) const fn request_id(&self) -> RequestId {
        self.request_id
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DropCandidate {
    request_id: RequestId,
}

impl DropCandidate {
    pub(crate) const fn request_id(&self) -> RequestId {
        self.request_id
    }
}

impl RuntimeEffects {
    fn new(disposition: RuntimeDisposition) -> Self {
        Self {
            disposition,
            cancel_worker: None,
            start_worker: None,
            drop_candidate: None,
        }
    }

    pub(crate) const fn disposition(&self) -> RuntimeDisposition {
        self.disposition
    }

    pub(crate) fn take_cancel_worker(&mut self) -> Option<CancelWorker> {
        self.cancel_worker.take()
    }

    pub(crate) fn take_start_worker(&mut self) -> Option<GenerationRequest> {
        self.start_worker.take()
    }

    pub(crate) fn take_drop_candidate(&mut self) -> Option<DropCandidate> {
        self.drop_candidate.take()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceInvalidation {
    BackingScaleAtlas,
    MaterialContract,
}

impl ResourceInvalidation {
    const fn mask(self) -> ResourceChangeMask {
        match self {
            Self::BackingScaleAtlas => ResourceChangeMask::BACKING_SCALE_ATLAS,
            Self::MaterialContract => ResourceChangeMask::MATERIAL_CONTRACT,
        }
    }
}

#[derive(Debug)]
struct ActiveGeneration {
    version: SceneVersion,
    generation: super::scene::SceneGenerationData,
}

#[derive(Debug)]
enum PendingPhase {
    Queued,
    Preparing,
    Ready(AcceptedGenerationCandidate),
    Activating {
        candidate: AcceptedGenerationCandidate,
        attempt: ActivationAttempt,
        commit_eligible: bool,
    },
    SupersedingActivation {
        candidate: AcceptedGenerationCandidate,
        attempt: ActivationAttempt,
    },
}

#[derive(Debug)]
struct PendingGeneration {
    identity: RequestIdentity,
    worker_request: Option<GenerationRequest>,
    desired_surface: SurfaceEpoch,
    desired_source: AppliedRevisions,
    desired_snapshot: Arc<CompanionSceneSnapshot>,
    accepted_snapshot: Arc<CompanionSceneSnapshot>,
    phase: PendingPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerState {
    Idle,
    Running(RequestId),
    Cancelling(RequestId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeVisibility {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeLifecycle {
    Running,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryRequirement {
    SurfaceSuccessor {
        failed_device: DeviceEpoch,
        failed_surface: SurfaceEpoch,
    },
    DeviceSuccessor {
        failed_device: DeviceEpoch,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryState {
    Operational,
    FallbackPending(RecoveryRequirement),
    AwaitingRetry {
        requirement: RecoveryRequirement,
        device: DeviceEpoch,
        surface: SurfaceEpoch,
    },
    Recovering {
        requirement: RecoveryRequirement,
        device: DeviceEpoch,
        surface: SurfaceEpoch,
        request: RequestId,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CandidateRebaseError {
    DroppedStale,
    Projection(super::scene::SceneDeltaApplyError),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateRebase {
    identity: RequestIdentity,
    version: SceneVersion,
    content: super::scene::ContentDelta,
    frame: super::scene::FrameDelta,
}

impl CandidateRebase {
    pub(crate) const fn identity(&self) -> RequestIdentity {
        self.identity
    }

    pub(crate) const fn version(&self) -> SceneVersion {
        self.version
    }

    pub(crate) const fn content(&self) -> &super::scene::ContentDelta {
        &self.content
    }

    pub(crate) const fn frame(&self) -> &super::scene::FrameDelta {
        &self.frame
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ActivationAttemptId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActivationAttempt {
    attempt_id: ActivationAttemptId,
    request_id: RequestId,
    key: SceneGenerationKey,
    surface: SurfaceEpoch,
    applied: AppliedRevisions,
}

impl ActivationAttempt {
    pub(crate) const fn request_id(self) -> RequestId {
        self.request_id
    }

    pub(crate) const fn key(self) -> SceneGenerationKey {
        self.key
    }

    pub(crate) const fn surface(self) -> SurfaceEpoch {
        self.surface
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationStartError {
    NoReadyCandidate,
    CandidateNeedsRebase,
    Hidden,
    SurfaceUnavailable,
    Shutdown,
    CounterOverflow(CounterKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcquireDeferral {
    OutdatedReconfigured,
    Timeout,
    Occluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateFailure {
    Validation,
    Resource,
    PreSubmitEncode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EpochFailure {
    SurfaceLost,
    SurfaceValidation,
    DeviceLost,
    Internal,
    OutOfMemory,
    UncertainPostSubmit,
    ImmediateGpuError,
    DelayedGpuError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationAttemptOutcome {
    Deferred(AcquireDeferral),
    CandidateRejected(CandidateFailure),
    PresentedClean { surface: SurfaceEpoch },
    Fatal(EpochFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationTransition {
    RetryLater,
    CandidateDestroyedRetainingActive,
    Committed,
    HostFallbackPending,
    DroppedStale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureDefer {
    NoActiveGeneration,
    ActivationInProgress,
    RecoveryInProgress,
    Shutdown,
}

#[derive(Debug)]
pub(crate) struct CaptureLease<'a> {
    active: &'a ActiveGeneration,
}

impl CaptureLease<'_> {
    pub(crate) const fn version(&self) -> SceneVersion {
        self.active.version
    }

    pub(crate) fn template(&self) -> &super::scene::SceneTemplate {
        self.active.generation.template()
    }

    pub(crate) fn content(&self) -> &super::scene::SceneContent {
        self.active.generation.content()
    }

    pub(crate) fn frame(&self) -> &super::scene::SceneFrame {
        self.active.generation.frame()
    }

    /// The latest compatible content delta committed into the active logical
    /// generation. A retained host whose GPU mirror still carries `from` can
    /// stage this transaction directly and publish `to` only after submission.
    pub(crate) fn content_delta(&self) -> &super::scene::ContentDelta {
        &self.active.generation.delta_scratch.content
    }

    /// The frame half of [`content_delta`](Self::content_delta). The pair shares
    /// one generation and revision interval by construction.
    pub(crate) fn frame_delta(&self) -> &super::scene::FrameDelta {
        &self.active.generation.delta_scratch.frame
    }

    pub(crate) const fn content_checksum(&self) -> u64 {
        self.active.generation.content_checksum()
    }

    pub(crate) const fn frame_checksum(&self) -> u64 {
        self.active.generation.frame_checksum()
    }

    pub(crate) fn source_identity(&self) -> super::contract::CaptureSourceIdentity {
        super::contract::CaptureSourceIdentity::new(
            self.template().generation_checksum,
            self.content_checksum(),
            // A lease can only reference an active generation, and activation requires the
            // complete template/content/frame set to pass validation first.
            self.frame()
                .capture_source_checksum(self.template())
                .expect("an active companion scene frame is already validated"),
        )
    }

    pub(crate) fn logical_state_alias(&self) -> super::contract::CompanionCaptureStateAlias {
        let snapshot = self.source_snapshot();
        super::contract::CompanionCaptureStateAlias::resolve(
            snapshot.frame.helper_trouble,
            snapshot.frame.asleep,
            snapshot.frame.dimmed,
            super::canonical_activity_status(snapshot).0,
        )
    }

    pub(crate) fn source_snapshot(&self) -> &Arc<CompanionSceneSnapshot> {
        self.active.generation.source_snapshot()
    }
}

#[derive(Debug)]
pub(crate) struct CompanionSceneRuntimeState {
    active: Option<ActiveGeneration>,
    pending: Option<PendingGeneration>,
    worker: WorkerState,
    visibility: RuntimeVisibility,
    hidden_latest: Option<Arc<CompanionSceneSnapshot>>,
    reconciler: CompanionSceneReconciler,
    device_epoch: DeviceEpoch,
    surface_epoch: SurfaceEpoch,
    resource_generation: ResourceGeneration,
    next_request_id: RequestId,
    next_activation_attempt_id: ActivationAttemptId,
    recovery: RecoveryState,
    lifecycle: RuntimeLifecycle,
}

impl CompanionSceneRuntimeState {
    pub(crate) fn cold_start(snapshot: Arc<CompanionSceneSnapshot>) -> Result<Self, RuntimeError> {
        Self::cold_start_on_surface(snapshot, SurfaceEpoch(1))
    }

    pub(crate) fn cold_start_on_surface(
        snapshot: Arc<CompanionSceneSnapshot>,
        surface_epoch: SurfaceEpoch,
    ) -> Result<Self, RuntimeError> {
        let reconciler = CompanionSceneReconciler::new(snapshot)?;
        Ok(Self {
            active: None,
            pending: None,
            worker: WorkerState::Idle,
            visibility: RuntimeVisibility::Visible,
            hidden_latest: None,
            reconciler,
            device_epoch: DeviceEpoch(1),
            surface_epoch,
            resource_generation: ResourceGeneration(1),
            next_request_id: RequestId(1),
            next_activation_attempt_id: ActivationAttemptId(1),
            recovery: RecoveryState::Operational,
            lifecycle: RuntimeLifecycle::Running,
        })
    }

    pub(crate) fn with_active(snapshot: Arc<CompanionSceneSnapshot>) -> Result<Self, RuntimeError> {
        let mut runtime = Self::cold_start(Arc::clone(&snapshot))?;
        let generation = SceneGenerationKey {
            device: DeviceEpoch(1),
            layout: LayoutGeneration(1),
            resources: ResourceGeneration(1),
        };
        let compiled = super::scene::build_scene_generation_owned(
            Arc::clone(&snapshot),
            generation,
            AppliedRevisions::new(1, 1),
        )
        .map_err(|_| RuntimeError::SnapshotRejected(SnapshotRejection::InvalidValue))?;
        runtime.active = Some(ActiveGeneration {
            version: SceneVersion {
                generation,
                surface: SurfaceEpoch(1),
                applied: AppliedRevisions::new(1, 1),
            },
            generation: compiled,
        });
        Ok(runtime)
    }

    pub(crate) fn snapshot(&self) -> &Arc<CompanionSceneSnapshot> {
        self.reconciler.snapshot()
    }

    pub(crate) const fn applied_revisions(&self) -> AppliedRevisions {
        self.reconciler.applied_revisions()
    }

    pub(crate) fn active_version(&self) -> Option<SceneVersion> {
        self.active.as_ref().map(|active| active.version)
    }

    pub(crate) fn pending_request_identity(&self) -> Option<RequestIdentity> {
        self.pending.as_ref().map(|pending| pending.identity)
    }

    pub(crate) fn pending_desired_source(&self) -> Option<AppliedRevisions> {
        self.pending.as_ref().map(|pending| pending.desired_source)
    }

    pub(crate) fn pending_desired_snapshot(&self) -> Option<&Arc<CompanionSceneSnapshot>> {
        self.pending
            .as_ref()
            .map(|pending| &pending.desired_snapshot)
    }

    fn pending_identity_after_generation(&self) -> RequestIdentity {
        self.pending_request_identity()
            .expect("resource invalidation always queues one generation")
    }

    fn ensure_running(&self) -> Result<(), RuntimeError> {
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            Err(RuntimeError::Shutdown)
        } else {
            Ok(())
        }
    }

    pub(crate) fn prepare_snapshot(
        &self,
        snapshot: Arc<CompanionSceneSnapshot>,
    ) -> Result<PreparedSnapshotUpdate, RuntimeError> {
        self.prepare_snapshot_with_resource_invalidation(snapshot, None)
    }

    pub(crate) fn prepare_snapshot_with_resource_invalidation(
        &self,
        snapshot: Arc<CompanionSceneSnapshot>,
        invalidation: Option<ResourceInvalidation>,
    ) -> Result<PreparedSnapshotUpdate, RuntimeError> {
        self.ensure_running()?;
        if self.visibility == RuntimeVisibility::Hidden {
            return Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::InvalidValue,
            ));
        }
        validate_snapshot(&snapshot)?;
        let mut changes = classify_snapshot_changes(self.reconciler.snapshot(), &snapshot);
        if let Some(invalidation) = invalidation {
            changes.resources.insert(invalidation.mask());
        }
        let mut snapshot = snapshot;
        if changes.has_semantic() {
            let semantic = SemanticRevision(increment(
                self.reconciler.semantic_revision.0,
                CounterKind::SemanticRevision,
            )?);
            rebase_semantic_transition_frames(
                self.reconciler.snapshot(),
                Arc::make_mut(&mut snapshot),
                semantic,
            );
            changes = classify_snapshot_changes(self.reconciler.snapshot(), &snapshot);
            if let Some(invalidation) = invalidation {
                changes.resources.insert(invalidation.mask());
            }
        }
        self.prepare_with_changes(snapshot, changes, false)
    }

    pub(crate) fn prepare_frame_projection(
        &self,
        projection: super::CompanionFrameProjection,
    ) -> Result<PreparedFrameProjection, RuntimeError> {
        self.prepare_frame_projection_with_resource_invalidation(projection, None)
    }

    pub(crate) fn prepare_frame_projection_with_resource_invalidation(
        &self,
        projection: super::CompanionFrameProjection,
        invalidation: Option<ResourceInvalidation>,
    ) -> Result<PreparedFrameProjection, RuntimeError> {
        self.ensure_running()?;
        if self.visibility == RuntimeVisibility::Hidden {
            return Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::InvalidValue,
            ));
        }
        let actual = self.reconciler.applied_revisions().semantic;
        if projection.semantic_base != actual {
            return Err(RuntimeError::StaleSemanticBase {
                expected: projection.semantic_base,
                actual,
            });
        }
        let snapshot = Arc::new(CompanionSceneSnapshot {
            schema_version: self.reconciler.snapshot().schema_version,
            privacy: self.reconciler.snapshot().privacy,
            topology: self.reconciler.snapshot().topology.clone(),
            content: self.reconciler.snapshot().content.clone(),
            frame: projection.frame,
        });
        validate_snapshot(&snapshot)?;
        let mut changes = classify_snapshot_changes(self.reconciler.snapshot(), &snapshot);
        if changes.requires_generation() || changes.has_semantic() {
            return Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::InconsistentIdentity,
            ));
        }
        if let Some(invalidation) = invalidation {
            changes.resources.insert(invalidation.mask());
        }
        self.prepare_with_changes(snapshot, changes, false)
            .map(|update| PreparedFrameProjection { update })
    }

    pub(crate) fn commit_frame_projection(
        &mut self,
        prepared: PreparedFrameProjection,
    ) -> Result<RuntimeEffects, PreparedCommitError> {
        self.commit_prepared(prepared.update)
    }

    fn prepare_hidden_latest(&self) -> Result<Option<PreparedSnapshotUpdate>, RuntimeError> {
        self.ensure_running()?;
        let Some(snapshot) = &self.hidden_latest else {
            return Ok(None);
        };
        validate_snapshot(snapshot)?;
        let changes = classify_snapshot_changes(self.reconciler.snapshot(), snapshot);
        self.prepare_with_changes(Arc::clone(snapshot), changes, true)
            .map(Some)
    }

    fn prepare_with_changes(
        &self,
        snapshot: Arc<CompanionSceneSnapshot>,
        changes: SnapshotChangeSet,
        hidden_origin: bool,
    ) -> Result<PreparedSnapshotUpdate, RuntimeError> {
        let layout_generation = if !changes.layout.is_empty() {
            LayoutGeneration(increment(
                self.reconciler.layout_generation.0,
                CounterKind::LayoutGeneration,
            )?)
        } else {
            self.reconciler.layout_generation
        };
        let semantic = if changes.has_semantic() {
            SemanticRevision(increment(
                self.reconciler.semantic_revision.0,
                CounterKind::SemanticRevision,
            )?)
        } else {
            self.reconciler.semantic_revision
        };
        let frame = if changes.has_frame() {
            FrameRevision(increment(
                self.reconciler.frame_revision.0,
                CounterKind::FrameRevision,
            )?)
        } else {
            self.reconciler.frame_revision
        };
        let generation = if changes.requires_generation() {
            let resources = if changes.resources.is_empty() {
                self.resource_generation
            } else {
                ResourceGeneration(increment(
                    self.resource_generation.0,
                    CounterKind::ResourceGeneration,
                )?)
            };
            Some(PreparedGenerationRequest {
                request_id: self.next_request_id,
                next_request_id: RequestId(increment(
                    self.next_request_id.0,
                    CounterKind::RequestId,
                )?),
                resources,
            })
        } else {
            None
        };
        Ok(PreparedSnapshotUpdate {
            expected_snapshot: Arc::clone(self.reconciler.snapshot()),
            expected_layout: self.reconciler.layout_generation,
            expected_applied: self.reconciler.applied_revisions(),
            expected_resources: self.resource_generation,
            expected_next_request: self.next_request_id,
            expected_device: self.device_epoch,
            expected_surface: self.surface_epoch,
            expected_visibility: self.visibility,
            expected_worker: self.worker,
            expected_pending_request: self
                .pending_request_identity()
                .map(RequestIdentity::request_id),
            expected_recovery: self.recovery,
            expected_hidden_latest: self.hidden_latest.as_ref().map(Arc::clone),
            snapshot,
            changes,
            layout_generation,
            applied: AppliedRevisions { semantic, frame },
            generation,
            hidden_origin,
        })
    }

    pub(crate) fn commit_prepared(
        &mut self,
        prepared: PreparedSnapshotUpdate,
    ) -> Result<RuntimeEffects, PreparedCommitError> {
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return Err(PreparedCommitError::Shutdown);
        }
        let base_matches = Arc::ptr_eq(&prepared.expected_snapshot, self.reconciler.snapshot())
            && prepared.expected_layout == self.reconciler.layout_generation
            && prepared.expected_applied == self.reconciler.applied_revisions()
            && prepared.expected_resources == self.resource_generation
            && prepared.expected_next_request == self.next_request_id
            && prepared.expected_device == self.device_epoch
            && prepared.expected_surface == self.surface_epoch
            && prepared.expected_visibility == self.visibility
            && prepared.expected_worker == self.worker
            && prepared.expected_pending_request
                == self
                    .pending_request_identity()
                    .map(RequestIdentity::request_id)
            && prepared.expected_recovery == self.recovery;
        let hidden_matches = match (
            prepared.expected_hidden_latest.as_ref(),
            self.hidden_latest.as_ref(),
        ) {
            (None, None) => true,
            (Some(expected), Some(current)) => Arc::ptr_eq(expected, current),
            _ => false,
        };
        if !base_matches || !hidden_matches {
            return Err(PreparedCommitError::StaleBase);
        }

        if prepared.generation.is_none() {
            if let Some(active) = &mut self.active {
                let active_changes = classify_snapshot_changes(
                    active.generation.source_snapshot(),
                    &prepared.snapshot,
                );
                if !active_changes.requires_generation() {
                    active
                        .generation
                        .apply_compatible_snapshot(
                            Arc::clone(&prepared.snapshot),
                            active_changes,
                            active.generation.source_revisions(),
                            prepared.applied,
                        )
                        .map_err(PreparedCommitError::Projection)?;
                    active.version.applied = prepared.applied;
                }
            }
        }
        self.reconciler.commit_prepared(&prepared);
        if prepared.hidden_origin {
            self.hidden_latest = None;
        }
        if let Some(generation) = prepared.generation {
            self.resource_generation = generation.resources;
            self.next_request_id = generation.next_request_id;
            let request = GenerationRequest {
                request_id: generation.request_id,
                key: SceneGenerationKey {
                    device: self.device_epoch,
                    layout: prepared.layout_generation,
                    resources: generation.resources,
                },
                surface: self.surface_epoch,
                source: prepared.applied,
                snapshot: Arc::clone(&prepared.snapshot),
                seal: Arc::new(()),
            };
            return Ok(self.queue_request(request));
        }

        if let Some(pending) = &mut self.pending {
            pending.desired_source = prepared.applied;
            pending.desired_snapshot = Arc::clone(&prepared.snapshot);
            if let PendingPhase::Activating { candidate, commit_eligible, .. } = &mut pending.phase
            {
                if !Arc::ptr_eq(
                    candidate.generation.source_snapshot(),
                    &pending.desired_snapshot,
                ) {
                    *commit_eligible = false;
                }
            }
        }
        if prepared.changes != SnapshotChangeSet::NONE {
            Ok(RuntimeEffects::new(RuntimeDisposition::SnapshotCommitted(
                prepared.changes,
            )))
        } else {
            Ok(RuntimeEffects::new(RuntimeDisposition::Unchanged))
        }
    }

    fn start_pending_worker(&mut self, effects: &mut RuntimeEffects) {
        if self.lifecycle != RuntimeLifecycle::Running
            || self.visibility != RuntimeVisibility::Visible
            || self.worker != WorkerState::Idle
        {
            return;
        }
        let Some(pending) = &mut self.pending else {
            return;
        };
        if !matches!(pending.phase, PendingPhase::Queued) {
            return;
        }
        let Some(request) = pending.worker_request.take() else {
            return;
        };
        let identity = pending.identity;
        pending.phase = PendingPhase::Preparing;
        self.worker = WorkerState::Running(identity.request_id);
        effects.start_worker = Some(request);
        effects.disposition = RuntimeDisposition::GenerationStarted(identity.request_id);
    }

    fn queue_request(&mut self, request: GenerationRequest) -> RuntimeEffects {
        let identity = request.identity();
        let mut effects =
            RuntimeEffects::new(RuntimeDisposition::GenerationQueued(identity.request_id));
        let mut superseded_activation = None;
        if let Some(old) = self.pending.take() {
            match old.phase {
                PendingPhase::Activating { candidate, attempt, .. }
                | PendingPhase::SupersedingActivation { candidate, attempt } => {
                    superseded_activation = Some((candidate, attempt));
                }
                PendingPhase::Ready(candidate) => {
                    effects.drop_candidate =
                        Some(DropCandidate { request_id: candidate.request_id });
                }
                _ => {
                    if self.worker == WorkerState::Running(old.identity.request_id) {
                        self.worker = WorkerState::Cancelling(old.identity.request_id);
                        effects.cancel_worker =
                            Some(CancelWorker { request_id: old.identity.request_id });
                    }
                }
            }
        }
        let phase = if let Some((candidate, attempt)) = superseded_activation {
            PendingPhase::SupersedingActivation { candidate, attempt }
        } else {
            PendingPhase::Queued
        };
        if let RecoveryState::Recovering { requirement, .. } = self.recovery {
            self.recovery = RecoveryState::Recovering {
                requirement,
                device: identity.key.device,
                surface: identity.surface,
                request: identity.request_id,
            };
        }
        self.pending = Some(PendingGeneration {
            identity,
            desired_surface: identity.surface,
            desired_source: identity.source,
            desired_snapshot: Arc::clone(&request.snapshot),
            accepted_snapshot: Arc::clone(&request.snapshot),
            worker_request: Some(request),
            phase,
        });
        self.start_pending_worker(&mut effects);
        effects
    }

    pub(crate) fn invalidate_resources(
        &mut self,
        invalidation: ResourceInvalidation,
    ) -> Result<RuntimeEffects, RuntimeError> {
        self.invalidate_resource_mask(invalidation.mask())
    }

    fn invalidate_resource_mask(
        &mut self,
        mask: ResourceChangeMask,
    ) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        let prepared = self.prepare_with_changes(
            Arc::clone(self.reconciler.snapshot()),
            SnapshotChangeSet {
                resources: mask,
                ..SnapshotChangeSet::NONE
            },
            false,
        )?;
        self.commit_prepared(prepared)
            .map_err(|_| RuntimeError::SnapshotRejected(SnapshotRejection::InvalidValue))
    }

    pub(crate) fn acknowledge_worker_cancelled(&mut self, id: RequestId) -> RuntimeEffects {
        let mut effects = RuntimeEffects::new(RuntimeDisposition::DroppedStale);
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return effects;
        }
        if self.worker != WorkerState::Cancelling(id) {
            return effects;
        }
        self.worker = WorkerState::Idle;
        self.start_pending_worker(&mut effects);
        effects
    }

    pub(crate) fn reject_worker_candidate(&mut self, identity: RequestIdentity) -> RuntimeEffects {
        if self.worker == WorkerState::Cancelling(identity.request_id) {
            return self.acknowledge_worker_cancelled(identity.request_id);
        }
        let mut effects = RuntimeEffects::new(RuntimeDisposition::DroppedStale);
        if self.lifecycle == RuntimeLifecycle::Shutdown
            || self.worker != WorkerState::Running(identity.request_id)
            || self.pending.as_ref().is_none_or(|pending| {
                pending.identity != identity || !matches!(pending.phase, PendingPhase::Preparing)
            })
        {
            return effects;
        }
        self.worker = WorkerState::Idle;
        self.pending = None;
        effects.disposition = RuntimeDisposition::CandidateDropped(identity.request_id);
        effects
    }

    pub(crate) fn complete_candidate(
        &mut self,
        candidate: AcceptedGenerationCandidate,
    ) -> RuntimeEffects {
        let mut effects = RuntimeEffects::new(RuntimeDisposition::DroppedStale);
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            effects.drop_candidate = Some(DropCandidate { request_id: candidate.request_id });
            return effects;
        }
        if self.worker == WorkerState::Cancelling(candidate.request_id) {
            self.worker = WorkerState::Idle;
            effects.drop_candidate = Some(DropCandidate { request_id: candidate.request_id });
            self.start_pending_worker(&mut effects);
            return effects;
        }
        let Some(pending) = &mut self.pending else {
            effects.drop_candidate = Some(DropCandidate { request_id: candidate.request_id });
            return effects;
        };
        if self.worker != WorkerState::Running(candidate.request_id)
            || pending.identity.request_id != candidate.request_id
            || pending.identity.key != candidate.key
            || pending.identity.source != candidate.applied
            || !matches!(pending.phase, PendingPhase::Preparing)
        {
            effects.drop_candidate = Some(DropCandidate { request_id: candidate.request_id });
            return effects;
        }
        let request_id = candidate.request_id;
        pending.phase = PendingPhase::Ready(candidate);
        self.worker = WorkerState::Idle;
        effects.disposition = RuntimeDisposition::CandidateReady(request_id);
        effects
    }

    pub(crate) fn rebase_ready_candidate(
        &mut self,
    ) -> Result<CandidateRebase, CandidateRebaseError> {
        let Some(pending) = &mut self.pending else {
            return Err(CandidateRebaseError::DroppedStale);
        };
        let PendingPhase::Ready(candidate) = &mut pending.phase else {
            return Err(CandidateRebaseError::DroppedStale);
        };
        let changes =
            classify_snapshot_changes(&pending.accepted_snapshot, &pending.desired_snapshot);
        candidate
            .generation
            .apply_compatible_snapshot(
                Arc::clone(&pending.desired_snapshot),
                changes,
                candidate.applied,
                pending.desired_source,
            )
            .map_err(CandidateRebaseError::Projection)?;
        pending.accepted_snapshot = Arc::clone(&pending.desired_snapshot);
        candidate.applied = pending.desired_source;
        Ok(CandidateRebase {
            identity: pending.identity,
            version: SceneVersion {
                generation: candidate.key,
                surface: pending.desired_surface,
                applied: candidate.applied,
            },
            content: candidate.generation.delta_scratch.content.clone(),
            frame: candidate.generation.delta_scratch.frame.clone(),
        })
    }

    pub(crate) fn begin_activation(&mut self) -> Result<ActivationAttempt, ActivationStartError> {
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return Err(ActivationStartError::Shutdown);
        }
        if self.visibility == RuntimeVisibility::Hidden {
            return Err(ActivationStartError::Hidden);
        }
        if matches!(
            self.recovery,
            RecoveryState::FallbackPending(_) | RecoveryState::AwaitingRetry { .. }
        ) {
            return Err(ActivationStartError::SurfaceUnavailable);
        }
        let pending = self
            .pending
            .as_mut()
            .ok_or(ActivationStartError::NoReadyCandidate)?;
        let placeholder = PendingPhase::Queued;
        let phase = std::mem::replace(&mut pending.phase, placeholder);
        let PendingPhase::Ready(candidate) = phase else {
            pending.phase = phase;
            return Err(ActivationStartError::NoReadyCandidate);
        };
        if candidate.applied != pending.desired_source
            || !Arc::ptr_eq(
                candidate.generation.source_snapshot(),
                &pending.desired_snapshot,
            )
        {
            pending.phase = PendingPhase::Ready(candidate);
            return Err(ActivationStartError::CandidateNeedsRebase);
        }
        let attempt_id = self.next_activation_attempt_id;
        let Some(next_id) = attempt_id.0.checked_add(1) else {
            pending.phase = PendingPhase::Ready(candidate);
            return Err(ActivationStartError::CounterOverflow(
                CounterKind::ActivationAttemptId,
            ));
        };
        let attempt = ActivationAttempt {
            attempt_id,
            request_id: pending.identity.request_id,
            key: pending.identity.key,
            surface: pending.desired_surface,
            applied: pending.desired_source,
        };
        self.next_activation_attempt_id = ActivationAttemptId(next_id);
        pending.phase = PendingPhase::Activating {
            candidate,
            attempt,
            commit_eligible: true,
        };
        Ok(attempt)
    }

    pub(crate) fn finish_activation(
        &mut self,
        attempt: ActivationAttempt,
        outcome: ActivationAttemptOutcome,
    ) -> RuntimeEffects {
        let mut effects = RuntimeEffects::new(RuntimeDisposition::DroppedStale);
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return effects;
        }
        let Some(pending) = &mut self.pending else {
            return effects;
        };
        let placeholder = PendingPhase::Queued;
        let phase = std::mem::replace(&mut pending.phase, placeholder);
        let (candidate, current, commit_eligible, superseding) = match phase {
            PendingPhase::Activating { candidate, attempt, commit_eligible } => {
                (candidate, attempt, commit_eligible, false)
            }
            PendingPhase::SupersedingActivation { candidate, attempt } => {
                (candidate, attempt, false, true)
            }
            other => {
                pending.phase = other;
                return effects;
            }
        };
        if current != attempt
            || candidate.request_id != attempt.request_id
            || candidate.key != attempt.key
            || candidate.applied != attempt.applied
        {
            pending.phase = if superseding {
                PendingPhase::SupersedingActivation { candidate, attempt: current }
            } else {
                PendingPhase::Activating {
                    candidate,
                    attempt: current,
                    commit_eligible,
                }
            };
            return effects;
        }

        if let ActivationAttemptOutcome::Fatal(failure) = outcome {
            let surface_failure = matches!(
                failure,
                EpochFailure::SurfaceLost | EpochFailure::SurfaceValidation
            );
            let device_failure = matches!(
                failure,
                EpochFailure::DeviceLost
                    | EpochFailure::Internal
                    | EpochFailure::OutOfMemory
                    | EpochFailure::UncertainPostSubmit
                    | EpochFailure::ImmediateGpuError
                    | EpochFailure::DelayedGpuError
            );
            let stale_surface_failure = surface_failure
                && (attempt.key.device != self.device_epoch
                    || attempt.surface != self.surface_epoch);
            let stale_device_failure = device_failure && attempt.key.device != self.device_epoch;
            if stale_surface_failure || stale_device_failure {
                if superseding {
                    effects.drop_candidate =
                        Some(DropCandidate { request_id: candidate.request_id });
                    pending.phase = PendingPhase::Queued;
                } else {
                    pending.phase = PendingPhase::Ready(candidate);
                }
                self.start_pending_worker(&mut effects);
                return effects;
            }
            if device_failure {
                self.active = None;
            }
            self.pending = None;
            let requirement = if device_failure {
                RecoveryRequirement::DeviceSuccessor { failed_device: self.device_epoch }
            } else {
                RecoveryRequirement::SurfaceSuccessor {
                    failed_device: self.device_epoch,
                    failed_surface: self.surface_epoch,
                }
            };
            self.recovery = RecoveryState::FallbackPending(requirement);
            effects.disposition =
                RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending);
            effects.drop_candidate = Some(DropCandidate { request_id: candidate.request_id });
            return effects;
        }

        if superseding {
            let transition = match outcome {
                ActivationAttemptOutcome::CandidateRejected(_) => {
                    ActivationTransition::CandidateDestroyedRetainingActive
                }
                ActivationAttemptOutcome::Deferred(_)
                | ActivationAttemptOutcome::PresentedClean { .. } => {
                    ActivationTransition::DroppedStale
                }
                ActivationAttemptOutcome::Fatal(_) => unreachable!("handled above"),
            };
            effects.drop_candidate = Some(DropCandidate { request_id: candidate.request_id });
            pending.phase = PendingPhase::Queued;
            self.start_pending_worker(&mut effects);
            if effects.start_worker.is_none() {
                effects.disposition = RuntimeDisposition::Activation(transition);
            }
            return effects;
        }

        let transition = match outcome {
            ActivationAttemptOutcome::Deferred(_) => {
                pending.phase = PendingPhase::Ready(candidate);
                ActivationTransition::RetryLater
            }
            ActivationAttemptOutcome::CandidateRejected(_) => {
                let dropped = candidate.request_id;
                let retained_active = self.active.is_some();
                self.pending = None;
                effects.drop_candidate = Some(DropCandidate { request_id: dropped });
                if let RecoveryState::Recovering { requirement, device, surface, .. } =
                    self.recovery
                {
                    self.recovery = RecoveryState::AwaitingRetry { requirement, device, surface };
                }
                if retained_active {
                    ActivationTransition::CandidateDestroyedRetainingActive
                } else {
                    ActivationTransition::HostFallbackPending
                }
            }
            ActivationAttemptOutcome::PresentedClean { surface }
                if commit_eligible
                    && surface == self.surface_epoch
                    && attempt.surface == pending.desired_surface
                    && attempt.applied == pending.desired_source
                    && candidate.request_id == pending.identity.request_id
                    && candidate.key == pending.identity.key
                    && candidate.applied == pending.desired_source
                    && Arc::ptr_eq(
                        candidate.generation.source_snapshot(),
                        &pending.desired_snapshot,
                    )
                    && match self.recovery {
                        RecoveryState::Operational => true,
                        RecoveryState::Recovering {
                            requirement,
                            device,
                            surface: recovery_surface,
                            request,
                        } => {
                            let sufficient_successor = match requirement {
                                RecoveryRequirement::SurfaceSuccessor {
                                    failed_device,
                                    failed_surface,
                                } => {
                                    device == failed_device && recovery_surface.0 > failed_surface.0
                                }
                                RecoveryRequirement::DeviceSuccessor { failed_device } => {
                                    device.0 > failed_device.0
                                }
                            };
                            sufficient_successor
                                && device == candidate.key.device
                                && recovery_surface == surface
                                && request == candidate.request_id
                        }
                        RecoveryState::FallbackPending(_) => false,
                        RecoveryState::AwaitingRetry { .. } => false,
                    } =>
            {
                self.active = Some(ActiveGeneration {
                    version: SceneVersion {
                        generation: candidate.key,
                        surface,
                        applied: candidate.applied,
                    },
                    generation: candidate.generation,
                });
                self.pending = None;
                self.recovery = RecoveryState::Operational;
                ActivationTransition::Committed
            }
            ActivationAttemptOutcome::PresentedClean { .. } => {
                pending.phase = PendingPhase::Ready(candidate);
                ActivationTransition::DroppedStale
            }
            ActivationAttemptOutcome::Fatal(_) => unreachable!("handled above"),
        };
        effects.disposition = RuntimeDisposition::Activation(transition);
        effects
    }

    pub(crate) fn acknowledge_surface_rebound(&mut self) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        let requirement = match self.recovery {
            RecoveryState::FallbackPending(
                requirement @ RecoveryRequirement::SurfaceSuccessor {
                    failed_device,
                    failed_surface,
                },
            ) if failed_device == self.device_epoch && failed_surface == self.surface_epoch => {
                requirement
            }
            _ => return Err(RuntimeError::RecoveryActionRejected),
        };
        let next = SurfaceEpoch(increment(self.surface_epoch.0, CounterKind::SurfaceEpoch)?);
        increment(self.resource_generation.0, CounterKind::ResourceGeneration)?;
        increment(self.next_request_id.0, CounterKind::RequestId)?;
        self.surface_epoch = next;
        let effects = self.invalidate_resource_mask(ResourceChangeMask::SURFACE_RECOVERY)?;
        if let Some(pending) = &mut self.pending {
            pending.desired_surface = next;
            if let PendingPhase::Activating { commit_eligible, .. } = &mut pending.phase {
                *commit_eligible = false;
            }
        }
        let identity = self.pending_identity_after_generation();
        self.recovery = RecoveryState::Recovering {
            requirement,
            device: identity.key.device,
            surface: identity.surface,
            request: identity.request_id,
        };
        Ok(effects)
    }

    #[cfg(test)]
    pub(crate) fn acknowledge_operational_surface_rebound(
        &mut self,
    ) -> Result<RuntimeEffects, RuntimeError> {
        let next = SurfaceEpoch(increment(self.surface_epoch.0, CounterKind::SurfaceEpoch)?);
        self.acknowledge_operational_surface_rebound_to(next)
    }

    pub(crate) fn acknowledge_operational_surface_rebound_to(
        &mut self,
        next: SurfaceEpoch,
    ) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        if self.recovery != RecoveryState::Operational {
            return Err(RuntimeError::RecoveryActionRejected);
        }
        if next.0 != increment(self.surface_epoch.0, CounterKind::SurfaceEpoch)? {
            return Err(RuntimeError::RecoveryActionRejected);
        }
        self.surface_epoch = next;
        if let Some(active) = &mut self.active {
            active.version.surface = next;
        }
        if let Some(pending) = &mut self.pending {
            pending.desired_surface = next;
            if let PendingPhase::Activating { commit_eligible, .. } = &mut pending.phase {
                *commit_eligible = false;
            }
        }
        Ok(RuntimeEffects::new(RuntimeDisposition::SurfaceRebound(
            next,
        )))
    }

    pub(crate) fn retry_current_generation(&mut self) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        if self.visibility != RuntimeVisibility::Visible
            || self.recovery != RecoveryState::Operational
        {
            return Err(RuntimeError::RecoveryActionRejected);
        }
        let request_id = self.next_request_id;
        self.next_request_id =
            RequestId(increment(self.next_request_id.0, CounterKind::RequestId)?);
        let request = GenerationRequest {
            request_id,
            key: SceneGenerationKey {
                device: self.device_epoch,
                layout: self.reconciler.layout_generation,
                resources: self.resource_generation,
            },
            surface: self.surface_epoch,
            source: self.reconciler.applied_revisions(),
            snapshot: Arc::clone(self.reconciler.snapshot()),
            seal: Arc::new(()),
        };
        Ok(self.queue_request(request))
    }

    pub(crate) fn acknowledge_device_recreated(&mut self) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        let requirement = match self.recovery {
            RecoveryState::FallbackPending(
                requirement @ RecoveryRequirement::DeviceSuccessor { failed_device },
            ) if failed_device == self.device_epoch => requirement,
            _ => return Err(RuntimeError::RecoveryActionRejected),
        };
        let next = DeviceEpoch(increment(self.device_epoch.0, CounterKind::DeviceEpoch)?);
        increment(self.resource_generation.0, CounterKind::ResourceGeneration)?;
        increment(self.next_request_id.0, CounterKind::RequestId)?;
        self.device_epoch = next;
        self.active = None;
        let effects = self.invalidate_resource_mask(ResourceChangeMask::DEVICE_RECOVERY)?;
        let identity = self.pending_identity_after_generation();
        self.recovery = RecoveryState::Recovering {
            requirement,
            device: identity.key.device,
            surface: identity.surface,
            request: identity.request_id,
        };
        Ok(effects)
    }

    pub(crate) fn retry_recovery(&mut self) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        let (requirement, device, surface) = match self.recovery {
            RecoveryState::AwaitingRetry { requirement, device, surface }
                if device == self.device_epoch
                    && surface == self.surface_epoch
                    && match requirement {
                        RecoveryRequirement::SurfaceSuccessor { failed_device, failed_surface } => {
                            device == failed_device && surface.0 > failed_surface.0
                        }
                        RecoveryRequirement::DeviceSuccessor { failed_device } => {
                            device.0 > failed_device.0
                        }
                    } =>
            {
                (requirement, device, surface)
            }
            _ => return Err(RuntimeError::RecoveryActionRejected),
        };
        let mask = match requirement {
            RecoveryRequirement::SurfaceSuccessor { .. } => ResourceChangeMask::SURFACE_RECOVERY,
            RecoveryRequirement::DeviceSuccessor { .. } => ResourceChangeMask::DEVICE_RECOVERY,
        };
        let effects = self.invalidate_resource_mask(mask)?;
        let identity = self.pending_identity_after_generation();
        debug_assert_eq!(identity.key.device, device);
        debug_assert_eq!(identity.surface, surface);
        self.recovery = RecoveryState::Recovering {
            requirement,
            device,
            surface,
            request: identity.request_id,
        };
        Ok(effects)
    }

    pub(crate) fn capture_lease(&self) -> Result<CaptureLease<'_>, CaptureDefer> {
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return Err(CaptureDefer::Shutdown);
        }
        if self.pending.as_ref().is_some_and(|pending| {
            matches!(
                pending.phase,
                PendingPhase::Activating { .. } | PendingPhase::SupersedingActivation { .. }
            )
        }) {
            return Err(CaptureDefer::ActivationInProgress);
        }
        if self.recovery != RecoveryState::Operational {
            return Err(CaptureDefer::RecoveryInProgress);
        }
        self.active
            .as_ref()
            .map(|active| CaptureLease { active })
            .ok_or(CaptureDefer::NoActiveGeneration)
    }

    pub(crate) fn set_hidden(&mut self) -> RuntimeEffects {
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return RuntimeEffects::new(RuntimeDisposition::Shutdown);
        }
        self.visibility = RuntimeVisibility::Hidden;
        if let Some(pending) = &mut self.pending {
            let placeholder = PendingPhase::Queued;
            let phase = std::mem::replace(&mut pending.phase, placeholder);
            pending.phase = match phase {
                PendingPhase::Activating { candidate, attempt, .. } => PendingPhase::Activating {
                    candidate,
                    attempt,
                    commit_eligible: false,
                },
                other => other,
            };
        }
        RuntimeEffects::new(RuntimeDisposition::HiddenCoalesced)
    }

    pub(crate) fn coalesce_hidden_snapshot(
        &mut self,
        snapshot: Arc<CompanionSceneSnapshot>,
    ) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        validate_snapshot(&snapshot)?;
        if self.visibility != RuntimeVisibility::Hidden {
            return Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::InvalidValue,
            ));
        }
        self.hidden_latest = Some(snapshot);
        Ok(RuntimeEffects::new(RuntimeDisposition::HiddenCoalesced))
    }

    pub(crate) fn prepare_reveal(&self) -> Result<PreparedReveal, RuntimeError> {
        self.prepare_reveal_with_resource_invalidation(None)
    }

    pub(crate) fn prepare_reveal_with_resource_invalidation(
        &self,
        invalidation: Option<ResourceInvalidation>,
    ) -> Result<PreparedReveal, RuntimeError> {
        self.ensure_running()?;
        if self.visibility != RuntimeVisibility::Hidden {
            return Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::InvalidValue,
            ));
        }
        let update = if let Some(snapshot) = &self.hidden_latest {
            validate_snapshot(snapshot)?;
            let mut changes = classify_snapshot_changes(self.reconciler.snapshot(), snapshot);
            if let Some(invalidation) = invalidation {
                changes.resources.insert(invalidation.mask());
            }
            self.prepare_with_changes(Arc::clone(snapshot), changes, true)?
        } else {
            let mut changes = SnapshotChangeSet::NONE;
            if let Some(invalidation) = invalidation {
                changes.resources.insert(invalidation.mask());
            }
            self.prepare_with_changes(Arc::clone(self.reconciler.snapshot()), changes, false)?
        };
        Ok(PreparedReveal { update })
    }

    pub(crate) fn commit_reveal(
        &mut self,
        prepared: PreparedReveal,
    ) -> Result<RuntimeEffects, PreparedCommitError> {
        if self.visibility != RuntimeVisibility::Hidden {
            return Err(PreparedCommitError::StaleBase);
        }
        let mut effects = self.commit_prepared(prepared.update)?;
        self.visibility = RuntimeVisibility::Visible;
        self.start_pending_worker(&mut effects);
        if effects.start_worker.is_none()
            && effects.cancel_worker.is_none()
            && matches!(effects.disposition, RuntimeDisposition::Unchanged)
        {
            effects.disposition = RuntimeDisposition::Revealed;
        }
        Ok(effects)
    }

    pub(crate) fn observe_delayed_gpu_error(&mut self, device: DeviceEpoch) -> RuntimeEffects {
        let mut effects = RuntimeEffects::new(RuntimeDisposition::DroppedStale);
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return effects;
        }
        if device != self.device_epoch {
            return effects;
        }
        self.active = None;
        if let Some(pending) = &self.pending {
            effects.drop_candidate = match &pending.phase {
                PendingPhase::Ready(candidate)
                | PendingPhase::Activating { candidate, .. }
                | PendingPhase::SupersedingActivation { candidate, .. } => {
                    Some(DropCandidate { request_id: candidate.request_id })
                }
                _ => None,
            };
        }
        self.pending = None;
        if let WorkerState::Running(id) = self.worker {
            self.worker = WorkerState::Cancelling(id);
            effects.cancel_worker = Some(CancelWorker { request_id: id });
        }
        self.recovery = RecoveryState::FallbackPending(RecoveryRequirement::DeviceSuccessor {
            failed_device: device,
        });
        effects.disposition =
            RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending);
        effects
    }

    pub(crate) fn shutdown(&mut self) -> RuntimeEffects {
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return RuntimeEffects::new(RuntimeDisposition::Shutdown);
        }
        let mut effects = RuntimeEffects::new(RuntimeDisposition::Shutdown);
        self.lifecycle = RuntimeLifecycle::Shutdown;
        if let WorkerState::Running(id) = self.worker {
            self.worker = WorkerState::Cancelling(id);
            effects.cancel_worker = Some(CancelWorker { request_id: id });
        }
        if let Some(pending) = &self.pending {
            effects.drop_candidate = match &pending.phase {
                PendingPhase::Ready(candidate)
                | PendingPhase::Activating { candidate, .. }
                | PendingPhase::SupersedingActivation { candidate, .. } => {
                    Some(DropCandidate { request_id: candidate.request_id })
                }
                _ => None,
            };
        }
        self.pending = None;
        self.hidden_latest = None;
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::Species;
    use crate::presentation::companion_scene::{
        AmbientFrameSnapshot, AmbientSemanticKindSnapshot, AmbientSemanticSnapshot,
        AuthoredDepthSnapshot, CompanionDayPhase, CompanionGlyphGrid, CompanionLogicalLayout,
        CompanionProjectionClock, CompanionSceneSnapshot, ContentSnapshot, DepthCue, FrameSnapshot,
        GaugeLevelSnapshot, LogicalGlyphAnchor, LogicalGlyphScale, PaletteSnapshot,
        PetLatticeSnapshot, PetRoleSpanSnapshot, PetTopologySnapshot, PropAnimationKindSnapshot,
        PropAnimationSnapshot, PropFrameSnapshot, PropPresentationMotion, PropTopologySnapshot,
        PropZoneSnapshot, RoomGlyphContentSnapshot, RoomGlyphFrameSnapshot, RoomTopologySnapshot,
        TankAnimationSnapshot, TankCellFrameSnapshot, TankCellSnapshot, TankFrameSnapshot,
        TankLayerSnapshot, TankRouteSnapshot, TankSideSnapshot, TankTopologySnapshot,
        TopologySnapshot, COMPANION_RENDERER_SCHEMA_VERSION, COMPANION_SCENE_SCHEMA_VERSION,
        PET_LATTICE_HEIGHT, PET_LATTICE_SLOTS, PET_LATTICE_WIDTH,
    };
    use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};
    use std::sync::Arc;

    type SnapshotMutation = (&'static str, Box<dyn Fn(&mut CompanionSceneSnapshot)>);

    fn snapshot() -> Arc<CompanionSceneSnapshot> {
        let mut snapshot = CompanionSceneSnapshot {
            schema_version: COMPANION_SCENE_SCHEMA_VERSION,
            privacy: PrivacyProjection::for_surface(PresentationSurface::RoundCompanion),
            topology: TopologySnapshot {
                layout: CompanionLogicalLayout::round(360.0, 360.0),
                glyph_grid: CompanionGlyphGrid {
                    columns: 60,
                    rows: 30,
                    y_up_origin_points: [0.0, 0.0],
                    cell_extent_points: [6.0, 12.0],
                    scale: LogicalGlyphScale::OneCell,
                    anchor: LogicalGlyphAnchor::CellBottomLeft,
                },
                pet: PetTopologySnapshot {
                    species: Species::Fuzz,
                    stage: Stage::S3,
                    lattice: PetLatticeSnapshot {
                        identity: "pet-art-13x10-v1",
                        width: PET_LATTICE_WIDTH,
                        height: PET_LATTICE_HEIGHT,
                        slot_count: PET_LATTICE_SLOTS,
                    },
                },
                room: RoomTopologySnapshot {
                    primary_biome: "starter",
                    secondary_biome: None,
                    species_dialect: "fuzz",
                },
                visible_props: vec![PropTopologySnapshot {
                    catalog_id: crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
                    stable_order: 0,
                    zone: PropZoneSnapshot::FloorRight,
                    authored_depth: AuthoredDepthSnapshot::Foreground,
                    shadow_profile: crate::game::habitat::catalog_prop_by_str(
                        crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
                    )
                    .unwrap()
                    .shadow_profile,
                    presentation_motion: PropPresentationMotion::Static,
                }],
                visible_tank_inhabitants: vec![TankTopologySnapshot {
                    catalog_id: crate::game::habitat::NEEDLEFISH,
                    stable_order: 0,
                    route: TankRouteSnapshot::CrossTankSwimmer,
                    authored_depth: AuthoredDepthSnapshot::BehindPet,
                }],
                renderer_schema: COMPANION_RENDERER_SCHEMA_VERSION,
            }
            .into(),
            content: ContentSnapshot {
                mood: Mood::Happy,
                room_weather: "clear",
                day_phase: CompanionDayPhase::Day,
                pet_lines: vec!["             ".to_owned(); usize::from(PET_LATTICE_HEIGHT)],
                pet_roles: vec![PetRoleSpanSnapshot {
                    line_index: 0,
                    start_char: 0,
                    end_char: 1,
                    role: "body",
                }],
                room_glyphs: Vec::new(),
                palette: PaletteSnapshot {
                    body: [1, 2, 3],
                    body_glow: [4, 5, 6],
                    eye: [7, 8, 9],
                    mouth: [10, 11, 12],
                    accent: [13, 14, 15],
                    pattern: [16, 17, 18],
                    particle: [19, 20, 21],
                    corruption: [22, 23, 24],
                },
                prop_animation_states: vec![PropAnimationSnapshot {
                    catalog_id: crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
                    stable_order: 0,
                    kind: PropAnimationKindSnapshot::Animated,
                    sprite_phase: Some(0),
                    twinkle_active: Some(false),
                    motion_phase: Some(0),
                    chest_lid_open: Some(false),
                    bloom_active: None,
                }],
                tank_animation_states: vec![TankAnimationSnapshot {
                    catalog_id: crate::game::habitat::NEEDLEFISH,
                    stable_order: 0,
                    route: TankRouteSnapshot::CrossTankSwimmer,
                    visible: true,
                    origin_col: 4,
                    origin_row: 5,
                    side: Some(TankSideSnapshot::Left),
                    layer: TankLayerSnapshot::Behind,
                    sprite_variant: 0,
                    visible_rows: 1,
                    anemone_morph: None,
                    color_srgb8: [126, 238, 255],
                    bold: true,
                    cadence_ms: 400,
                    calm: false,
                    cells: vec![TankCellSnapshot {
                        col: 4,
                        row: 5,
                        glyph: '<',
                        layer: TankLayerSnapshot::Behind,
                    }],
                }],
                ambient_semantics: (0..64)
                    .map(|slot| AmbientSemanticSnapshot { slot, kind: None, glyph: None })
                    .collect(),
            }
            .into(),
            frame: FrameSnapshot {
                elapsed_ms: 1_000,
                pet_anchor_points: [120.0, 140.0],
                pet_depth: 0.5,
                pet_depth_cue: DepthCue {
                    scale: 1.06,
                    y_offset_points_up: -1.8,
                    opacity: 1.0,
                    saturation: 1.0,
                },
                facing: 1,
                breath_offset_y_points: 0.0,
                bob_offset_y_points: 5.0,
                asleep: false,
                calm: false,
                helper_trouble: false,
                activity_recent: false,
                activity_opacity: 0.0,
                gauge_levels: [GaugeLevelSnapshot::Medium; 4],
                gauge_fractions: [0.375; 4],
                dimmed: false,
                dim_amount: 0.0,
                room_glyphs: Vec::new(),
                prop_instances: vec![PropFrameSnapshot {
                    slot: 0,
                    visible: true,
                    origin_points: [120.0, 140.0],
                    motion_offset_points: [0.0; 2],
                    opacity: 1.0,
                    footprint_points: [0.0; 2],
                    contact_shadow_strength: 0.0,
                    cast_shadow_vector_points: [0.0; 2],
                    cast_shadow_softness_points: 0.0,
                    cast_shadow_strength: 0.0,
                    transition: None,
                }],
                tank_instances: vec![TankFrameSnapshot {
                    slot: 0,
                    visible: true,
                    origin_points: [40.0, 50.0],
                    cells: vec![TankCellFrameSnapshot {
                        source_position_points: [40.0, 50.0],
                        position_points: [40.0, 50.0],
                        target_position_points: [40.0, 50.0],
                    }],
                    bounds_points: Some([40.0, 50.0, 10.0, 10.0]),
                    semantic_revision: SemanticRevision(1),
                    started_at_monotonic_ms: 1_000,
                    duration_ms: 400,
                }],
                ambient_instances: (0..64)
                    .map(|slot| AmbientFrameSnapshot {
                        slot,
                        visible: false,
                        position_points: [0.0; 2],
                        opacity: 0.0,
                    })
                    .collect(),
                pet_motion_input: crate::round::motion::CompanionMotionInput {
                    asleep: false,
                    calm: false,
                    rate_per_hour: 0.0,
                    current_facing: 1,
                    resolved_wander_offset_x: 0,
                    resolved_wander_facing: 1,
                    breath_offset_y_cells: 0,
                },
                pet_depth_override: None,
            },
        };
        set_pet_depth(&mut snapshot, 0.5);
        Arc::new(snapshot)
    }

    fn set_pet_depth(snapshot: &mut CompanionSceneSnapshot, raw_depth: f32) {
        snapshot.frame.pet_depth = raw_depth;
        let resolved = crate::round::depth::resolve_smooth_depth(
            raw_depth,
            crate::round::depth::depth_lifecycle_scale(snapshot.frame.asleep, snapshot.frame.calm),
        )
        .unwrap();
        snapshot.frame.pet_depth_cue = DepthCue {
            scale: resolved.scale,
            y_offset_points_up: -resolved.perspective_y
                * snapshot.topology.glyph_grid.cell_extent_points[1],
            opacity: resolved.atmosphere,
            saturation: 1.0,
        };
    }

    fn offset_pet_depth(snapshot: &mut CompanionSceneSnapshot, delta: f32) {
        set_pet_depth(snapshot, snapshot.frame.pet_depth + delta);
    }

    fn parallax_regression_clock() -> CompanionProjectionClock {
        CompanionProjectionClock::new(time::macros::datetime!(2026-07-11 12:00 UTC), 2_731)
    }

    fn expected_depth_parallax(
        snapshot: &CompanionSceneSnapshot,
        clock: CompanionProjectionClock,
        multiplier: f32,
    ) -> [f32; 2] {
        let grid = snapshot.topology.glyph_grid;
        let motion = crate::round::motion::project_round_companion_motion_with_options(
            snapshot.frame.pet_motion_input,
            clock.wall_time,
            clock.elapsed_ms,
            crate::round::motion::RoundCompanionMotionViewport {
                grid_columns: grid.columns,
                grid_rows: grid.rows,
                width_points: snapshot.topology.layout.width_points,
                height_points: snapshot.topology.layout.height_points,
                clearance: crate::round::scene::current_round_motion_clearance(grid.rows),
            },
            &crate::round::motion::companion_roam_motion(),
            crate::round::motion::RoundMotionProjectionOptions {
                depth_override: snapshot.frame.pet_depth_override,
            },
        );
        let displacement = [
            motion.motion_top_left_cells.x - motion.motion_origin_top_left_cells.x,
            motion.motion_top_left_cells.y - motion.motion_origin_top_left_cells.y,
        ];
        let cap_cells = [
            crate::presentation::companion_effects::PARALLAX_MAX_X_CELLS,
            crate::presentation::companion_effects::PARALLAX_MAX_Y_CELLS,
        ];
        std::array::from_fn(|axis| {
            let cap = grid.cell_extent_points[axis] * cap_cells[axis];
            (displacement[axis] * multiplier * grid.cell_extent_points[axis]).clamp(-cap, cap)
        })
    }

    fn assert_points_close(actual: [f32; 2], expected: [f32; 2]) {
        for axis in 0..2 {
            assert!(
                (actual[axis] - expected[axis]).abs() <= 0.000_01,
                "axis {axis}: expected {}, got {}",
                expected[axis],
                actual[axis]
            );
        }
    }

    fn classify(mutator: impl FnOnce(&mut CompanionSceneSnapshot)) -> SnapshotChangeSet {
        let initial = snapshot();
        let mut changed = (*initial).clone();
        mutator(&mut changed);
        classify_snapshot_changes(&initial, &changed)
    }

    macro_rules! assert_class {
        ($name:ident, $expected:expr, $mutation:expr) => {{
            let changes = classify($mutation);
            assert_eq!(changes.families(), $expected, stringify!($name));
        }};
    }

    #[test]
    fn every_snapshot_leaf_is_classified_by_render_lifetime() {
        let generation = ChangeFamilies::GENERATION;
        let semantic = ChangeFamilies::SEMANTIC;
        let frame = ChangeFamilies::FRAME;

        assert_class!(layout_width, generation, |s| s
            .topology
            .layout
            .width_points += 1.0);
        assert_class!(layout_height, generation, |s| s
            .topology
            .layout
            .height_points += 1.0);
        assert_class!(grid_columns, generation, |s| s
            .topology
            .glyph_grid
            .columns += 1);
        assert_class!(grid_rows, generation, |s| s.topology.glyph_grid.rows += 1);
        assert_class!(grid_origin, generation, |s| s
            .topology
            .glyph_grid
            .y_up_origin_points[0] +=
            1.0);
        assert_class!(grid_extent, generation, |s| s
            .topology
            .glyph_grid
            .cell_extent_points[0] +=
            1.0);
        assert_class!(pet_species, generation, |s| s.topology.pet.species =
            Species::Blob);
        assert_class!(pet_stage, generation, |s| s.topology.pet.stage = Stage::S4);
        assert_class!(lattice_identity, generation, |s| s
            .topology
            .pet
            .lattice
            .identity = "other");
        assert_class!(lattice_width, generation, |s| s
            .topology
            .pet
            .lattice
            .width += 1);
        assert_class!(lattice_height, generation, |s| s
            .topology
            .pet
            .lattice
            .height += 1);
        assert_class!(lattice_slots, generation, |s| s
            .topology
            .pet
            .lattice
            .slot_count += 1);
        assert_class!(primary_biome, generation, |s| s
            .topology
            .room
            .primary_biome = "cozy");
        assert_class!(secondary_biome, generation, |s| s
            .topology
            .room
            .secondary_biome =
            Some("cozy"));
        assert_class!(species_dialect, generation, |s| s
            .topology
            .room
            .species_dialect =
            "blob");
        assert_class!(prop_identity, generation, |s| s.topology.visible_props[0]
            .catalog_id = "lamp");
        assert_class!(prop_order, generation, |s| s.topology.visible_props[0]
            .stable_order = 1);
        assert_class!(prop_zone, generation, |s| s.topology.visible_props[0]
            .zone =
            PropZoneSnapshot::FloorLeft);
        assert_class!(prop_depth, generation, |s| s.topology.visible_props[0]
            .authored_depth =
            AuthoredDepthSnapshot::Background);
        assert_class!(prop_shadow_profile, generation, |s| s
            .topology
            .visible_props[0]
            .shadow_profile =
            crate::game::habitat::HabitatPropShadowProfile::ContactOnly);
        assert_class!(tank_identity, generation, |s| s
            .topology
            .visible_tank_inhabitants[0]
            .catalog_id = "crab");
        assert_class!(tank_order, generation, |s| s
            .topology
            .visible_tank_inhabitants[0]
            .stable_order = 1);
        assert_class!(tank_route, generation, |s| s
            .topology
            .visible_tank_inhabitants[0]
            .route =
            TankRouteSnapshot::RimResident);
        assert_class!(tank_depth, generation, |s| s
            .topology
            .visible_tank_inhabitants[0]
            .authored_depth =
            AuthoredDepthSnapshot::Foreground);

        assert_class!(mood, semantic, |s| s.content.mood = Mood::Content);
        assert_class!(weather, semantic, |s| s.content.room_weather = "cache-mist");
        assert_class!(pet_glyphs, semantic, |s| s.content.pet_lines[0]
            .replace_range(0..1, "o"));
        assert_class!(room_glyph_content, semantic, |s| s
            .content
            .room_glyphs
            .push(RoomGlyphContentSnapshot {
                slot: 0,
                glyph: '✦',
                color_srgb8: [1, 2, 3]
            }));
        assert_class!(pet_roles, ChangeFamilies::NONE, |s| s.content.pet_roles
            [0]
        .role = "eye");
        assert_class!(palette, semantic, |s| s.content.palette.body[0] += 1);
        assert_class!(prop_kind, semantic, |s| s.content.prop_animation_states
            [0]
        .kind =
            PropAnimationKindSnapshot::Static);
        assert_class!(prop_sprite, semantic, |s| s.content.prop_animation_states
            [0]
        .sprite_phase = Some(1));
        assert_class!(prop_twinkle, semantic, |s| s
            .content
            .prop_animation_states[0]
            .twinkle_active = Some(true));
        assert_class!(prop_motion, semantic, |s| s.content.prop_animation_states
            [0]
        .motion_phase = Some(1));
        assert_class!(prop_lid, semantic, |s| s.content.prop_animation_states[0]
            .chest_lid_open = Some(true));
        assert_class!(prop_bloom, semantic, |s| s.content.prop_animation_states
            [0]
        .bloom_active = Some(true));
        assert_class!(prop_resolved_origin, frame, |s| s.frame.prop_instances
            [0]
        .origin_points[0] += 1.0);
        assert_class!(prop_cast_vector, frame, |s| s.frame.prop_instances[0]
            .cast_shadow_vector_points =
            [1.0, -2.0]);
        assert_class!(prop_cast_softness, frame, |s| s.frame.prop_instances[0]
            .cast_shadow_softness_points =
            1.0);
        assert_class!(prop_cast_strength, frame, |s| s.frame.prop_instances[0]
            .cast_shadow_strength =
            0.2);
        assert_class!(tank_visible, semantic, |s| s
            .content
            .tank_animation_states[0]
            .visible = false);
        assert_class!(tank_origin_col, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .origin_col +=
            1);
        assert_class!(tank_origin_row, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .origin_row +=
            1);
        assert_class!(tank_origin_points, frame, |s| s.frame.tank_instances[0]
            .origin_points[0] += 1.0);
        assert_class!(tank_side, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .side =
            Some(TankSideSnapshot::Right));
        assert_class!(tank_layer, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .layer =
            TankLayerSnapshot::Foreground);
        assert_class!(tank_variant, semantic, |s| s
            .content
            .tank_animation_states[0]
            .sprite_variant = 1);
        assert_class!(tank_visible_rows, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .visible_rows +=
            1);
        assert_class!(tank_morph, semantic, |s| s.content.tank_animation_states
            [0]
        .anemone_morph = Some(1));
        assert_class!(tank_cell_position, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .cells[0]
            .col += 1);
        assert_class!(tank_cell_glyph, semantic, |s| s
            .content
            .tank_animation_states[0]
            .cells[0]
            .glyph = '>');
        assert_class!(tank_cell_layer, semantic, |s| s
            .content
            .tank_animation_states[0]
            .cells[0]
            .layer =
            TankLayerSnapshot::Foreground);
        assert_class!(tank_cell_points, frame, |s| s.frame.tank_instances[0]
            .cells[0]
            .position_points[0] += 1.0);
        assert_class!(tank_bounds_points, frame, |s| s.frame.tank_instances[0]
            .bounds_points
            .as_mut()
            .unwrap()[0] += 1.0);
        assert_class!(ambient_identity, semantic, |s| s
            .content
            .ambient_semantics[0]
            .kind =
            Some(AmbientSemanticKindSnapshot::Mote));
        assert_class!(ambient_glyph, semantic, |s| s.content.ambient_semantics
            [0]
        .glyph = Some('·'));
        assert_class!(ambient_frame, ChangeFamilies::NONE, |s| s
            .frame
            .ambient_instances[0]
            .position_points[0] +=
            1.0);
        assert_class!(activity_status, frame, |s| {
            s.frame.activity_recent = true;
            s.frame.activity_opacity = 0.75;
        });

        assert_class!(pet_anchor, frame, |s| s.frame.pet_anchor_points[0] += 1.0);
        assert_class!(room_glyph_frame, frame, |s| s.frame.room_glyphs.push(
            RoomGlyphFrameSnapshot {
                slot: 0,
                visible: true,
                grid_cell: [1, 1],
                position_points: [6.0, 336.0],
                opacity: 1.0,
            }
        ));
        assert_class!(pet_depth, frame, |s| offset_pet_depth(s, 0.1));
        assert_class!(facing, frame, |s| s.frame.facing = -1);
        assert_class!(breath, ChangeFamilies::NONE, |s| s
            .frame
            .breath_offset_y_points =
            20.0);
        assert_class!(bob, frame, |s| s.frame.bob_offset_y_points += 0.1);
        assert_class!(asleep, frame, |s| s.frame.asleep = true);
        assert_class!(helper, frame, |s| s.frame.helper_trouble = true);
        assert_class!(gauge_levels, frame, |s| s.frame.gauge_levels[0] =
            GaugeLevelSnapshot::High);
        assert_class!(gauge_fractions, frame, |s| s.frame.gauge_fractions[0] =
            0.75);
        assert_class!(day_phase, semantic, |s| s.content.day_phase =
            CompanionDayPhase::Dusk);
        assert_class!(calm, frame, |s| s.frame.calm = true);
        assert_class!(depth_cue, frame, |s| s.frame.pet_depth_cue.scale = 1.01);
        assert_class!(dim, frame, |s| {
            s.frame.dimmed = true;
            s.frame.dim_amount = 0.35;
        });
        assert_class!(dim_derivative, frame, |s| s.frame.dimmed = true);
        assert_class!(elapsed_clock, ChangeFamilies::NONE, |s| s
            .frame
            .elapsed_ms +=
            1);
        assert_class!(tank_cadence, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .cadence_ms += 1);
        assert_class!(tank_calm, ChangeFamilies::NONE, |s| s
            .content
            .tank_animation_states[0]
            .calm = true);
    }

    #[test]
    fn redundant_raw_inputs_are_accepted_noops_with_identical_compiled_mirrors() {
        let mut cases: Vec<SnapshotMutation> = vec![
            (
                "role over empty pet cell",
                Box::new(|s| s.content.pet_roles[0].role = "eye"),
            ),
            (
                "tank origin col",
                Box::new(|s| s.content.tank_animation_states[0].origin_col += 1),
            ),
            (
                "tank origin row",
                Box::new(|s| s.content.tank_animation_states[0].origin_row += 1),
            ),
            (
                "tank side",
                Box::new(|s| {
                    s.content.tank_animation_states[0].side = Some(TankSideSnapshot::Right)
                }),
            ),
            (
                "tank top layer",
                Box::new(|s| {
                    s.content.tank_animation_states[0].layer = TankLayerSnapshot::Foreground
                }),
            ),
            (
                "tank visible rows",
                Box::new(|s| s.content.tank_animation_states[0].visible_rows += 1),
            ),
            (
                "tank cell col",
                Box::new(|s| s.content.tank_animation_states[0].cells[0].col += 1),
            ),
            (
                "tank cell row",
                Box::new(|s| s.content.tank_animation_states[0].cells[0].row += 1),
            ),
            (
                "retained breath offset",
                Box::new(|s| s.frame.breath_offset_y_points += 1.0),
            ),
        ];
        let base = snapshot();
        let baseline = super::super::scene::build_scene_generation(
            &base,
            SceneGenerationKey {
                device: DeviceEpoch(1),
                layout: LayoutGeneration(1),
                resources: ResourceGeneration(1),
            },
        )
        .unwrap();
        for (name, mutate) in cases.drain(..) {
            let mut changed = (*base).clone();
            mutate(&mut changed);
            validate_snapshot(&changed).unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_eq!(
                classify_snapshot_changes(&base, &changed),
                SnapshotChangeSet::NONE,
                "{name}"
            );
            let compiled =
                super::super::scene::build_scene_generation(&changed, baseline.generation_key())
                    .unwrap();
            assert_eq!(compiled.template(), baseline.template(), "{name}");
            assert_eq!(compiled.content(), baseline.content(), "{name}");
            assert_eq!(compiled.frame(), baseline.frame(), "{name}");
            assert_eq!(
                compiled.content_checksum(),
                baseline.content_checksum(),
                "{name}"
            );
            assert_eq!(
                compiled.frame_checksum(),
                baseline.frame_checksum(),
                "{name}"
            );
        }

        let mut empty_cells = (*base).clone();
        empty_cells.content.tank_animation_states[0].cells.clear();
        empty_cells.frame.tank_instances[0].cells.clear();
        empty_cells.frame.tank_instances[0].bounds_points = None;
        let empty_cells = Arc::new(empty_cells);
        let baseline =
            super::super::scene::build_scene_generation(&empty_cells, baseline.generation_key())
                .unwrap();
        let mut changed = (*empty_cells).clone();
        changed.frame.tank_instances[0].bounds_points = Some([1.0, 2.0, 3.0, 4.0]);
        let changed = Arc::new(changed);
        assert_eq!(
            classify_snapshot_changes(&empty_cells, &changed),
            SnapshotChangeSet::NONE,
            "bounds are not rendered without tank cells"
        );
        let compiled =
            super::super::scene::build_scene_generation(&changed, baseline.generation_key())
                .unwrap();
        assert_eq!(compiled.frame(), baseline.frame());
        assert_eq!(compiled.frame_checksum(), baseline.frame_checksum());
    }

    #[test]
    fn every_compatible_render_family_matches_a_fresh_compilation() {
        let cases: Vec<SnapshotMutation> = vec![
            ("mood", Box::new(|s| s.content.mood = Mood::Content)),
            (
                "weather",
                Box::new(|s| s.content.room_weather = "cache-mist"),
            ),
            ("palette", Box::new(|s| s.content.palette.body[0] ^= 1)),
            (
                "prop semantic",
                Box::new(|s| s.content.prop_animation_states[0].chest_lid_open = Some(true)),
            ),
            (
                "prop frame",
                Box::new(|s| s.content.prop_animation_states[0].motion_phase = Some(1)),
            ),
            (
                "tank semantic",
                Box::new(|s| s.content.tank_animation_states[0].sprite_variant = 1),
            ),
            (
                "tank glyph",
                Box::new(|s| s.content.tank_animation_states[0].cells[0].glyph = '>'),
            ),
            (
                "tank frame",
                Box::new(|s| s.frame.tank_instances[0].origin_points[0] += 1.0),
            ),
            (
                "tank cell frame",
                Box::new(|s| s.frame.tank_instances[0].cells[0].position_points[0] += 1.0),
            ),
            (
                "ambient",
                Box::new(|s| {
                    s.content.ambient_semantics[0] = AmbientSemanticSnapshot {
                        slot: 0,
                        kind: Some(AmbientSemanticKindSnapshot::Mote),
                        glyph: Some('·'),
                    };
                    s.frame.ambient_instances[0] = AmbientFrameSnapshot {
                        slot: 0,
                        visible: true,
                        position_points: [20.0, 30.0],
                        opacity: 0.75,
                    };
                }),
            ),
            (
                "activity status",
                Box::new(|s| {
                    s.frame.activity_recent = true;
                    s.frame.activity_opacity = 0.6;
                }),
            ),
            ("pet transform", Box::new(|s| offset_pet_depth(s, 0.1))),
            (
                "asleep",
                Box::new(|s| {
                    s.frame.asleep = true;
                    s.frame.calm = true;
                    set_pet_depth(s, s.frame.pet_depth);
                }),
            ),
            ("helper", Box::new(|s| s.frame.helper_trouble = true)),
            (
                "gauges",
                Box::new(|s| {
                    s.frame.gauge_levels[0] = GaugeLevelSnapshot::High;
                    s.frame.gauge_fractions[0] = 0.75;
                }),
            ),
            (
                "dim",
                Box::new(|s| {
                    s.frame.dimmed = true;
                    s.frame.dim_amount = 0.25;
                }),
            ),
        ];
        let base = snapshot();
        let key = SceneGenerationKey {
            device: DeviceEpoch(2),
            layout: LayoutGeneration(3),
            resources: ResourceGeneration(4),
        };
        for (name, mutate) in cases {
            let mut target = (*base).clone();
            mutate(&mut target);
            validate_snapshot(&target).unwrap_or_else(|error| panic!("{name}: {error:?}"));
            let target = Arc::new(target);
            let changes = classify_snapshot_changes(&base, &target);
            assert!(!changes.requires_generation(), "{name}");
            assert!(changes.has_semantic() || changes.has_frame(), "{name}");
            let from = AppliedRevisions::new(5, 7);
            let to = AppliedRevisions::new(
                from.semantic.0 + u64::from(changes.has_semantic()),
                from.frame.0 + u64::from(changes.has_frame()),
            );
            let mut projected =
                super::super::scene::build_scene_generation_owned(Arc::clone(&base), key, from)
                    .unwrap();
            projected
                .apply_compatible_snapshot(Arc::clone(&target), changes, from, to)
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            let fresh = super::super::scene::build_scene_generation_owned(target, key, to).unwrap();
            assert_eq!(projected.template(), fresh.template(), "{name}");
            assert_eq!(projected.content(), fresh.content(), "{name}");
            assert_eq!(projected.frame(), fresh.frame(), "{name}");
            assert_eq!(
                projected.content_checksum(),
                fresh.content_checksum(),
                "{name}"
            );
            assert_eq!(projected.frame_checksum(), fresh.frame_checksum(), "{name}");
        }
    }

    #[test]
    fn fixed_named_masks_are_task_five_extensible() {
        let mood = classify(|s| s.content.mood = Mood::Content);
        assert!(mood.semantic().contains(SemanticChangeMask::MOOD_WEATHER));
        assert!(!mood.semantic().contains(SemanticChangeMask::PET_ART));

        let pet_motion = classify(|s| offset_pet_depth(s, 0.1));
        assert!(pet_motion.frame().contains(FrameChangeMask::PET_TRANSFORM));
        let asleep = classify(|s| s.frame.asleep = !s.frame.asleep);
        assert!(asleep.frame().contains(FrameChangeMask::PET_TRANSFORM));
        let trouble = classify(|s| s.frame.helper_trouble = true);
        assert!(trouble
            .frame()
            .contains(FrameChangeMask::TROUBLE_VISIBILITY));
        assert!(!trouble.frame().contains(FrameChangeMask::STATUS_VISIBILITY));

        let status = classify(|s| {
            s.frame.activity_recent = true;
            s.frame.activity_opacity = 0.75;
        });
        assert!(status.frame().contains(FrameChangeMask::STATUS_VISIBILITY));

        assert!(SemanticChangeMask::AMBIENT.is_named());
        assert!(FrameChangeMask::CAMERA.is_named());
        assert!(FrameChangeMask::LIGHTS.is_named());
        assert!(ResourceChangeMask::AMBIENT_AUTHORED.is_named());
    }

    #[test]
    fn status_classification_uses_only_the_canonical_activity_tuple() {
        let mut active = (*snapshot()).clone();
        active.frame.activity_recent = true;
        active.frame.activity_opacity = 0.6;

        let mut moved = active.clone();
        moved.content.ambient_semantics.reverse();
        moved.frame.ambient_instances.rotate_left(3);
        let changes = classify_snapshot_changes(&active, &moved);
        assert!(!changes.frame().contains(FrameChangeMask::STATUS_VISIBILITY));
        assert_eq!(super::super::canonical_activity_status(&moved), (true, 0.6));

        let mut faded = active.clone();
        faded.frame.activity_opacity = 0.4;
        let changes = classify_snapshot_changes(&active, &faded);
        assert!(!changes.frame().contains(FrameChangeMask::AMBIENT_INSTANCES));
        assert!(changes.frame().contains(FrameChangeMask::STATUS_VISIBILITY));

        let mut hidden = active.clone();
        hidden.frame.activity_recent = false;
        hidden.frame.activity_opacity = 0.0;
        assert_eq!(
            super::super::canonical_activity_status(&hidden),
            (false, 0.0)
        );
        let mut ambient_active = hidden.clone();
        ambient_active.content.ambient_semantics[3] = AmbientSemanticSnapshot {
            slot: 3,
            kind: Some(AmbientSemanticKindSnapshot::Mote),
            glyph: Some('·'),
        };
        ambient_active.frame.ambient_instances[3] = AmbientFrameSnapshot {
            slot: 3,
            visible: true,
            position_points: [40.0, 50.0],
            opacity: 0.2,
        };
        let mut ambient_opacity_changed = ambient_active.clone();
        ambient_opacity_changed.frame.ambient_instances[3].opacity = 0.4;
        let changes = classify_snapshot_changes(&ambient_active, &ambient_opacity_changed);
        assert!(changes.frame().contains(FrameChangeMask::AMBIENT_INSTANCES));
        assert!(!changes.frame().contains(FrameChangeMask::STATUS_VISIBILITY));
    }

    #[test]
    fn generation_request_builds_from_exact_snapshot_key_and_revisions() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        let expected_key = request.key();
        let expected_source = request.source();
        let expected_snapshot = Arc::clone(request.snapshot());

        let built = request.build_scene_generation().unwrap();
        assert_eq!(built.generation_key(), expected_key);
        assert_eq!(built.source_revisions(), expected_source);
        assert!(Arc::ptr_eq(&expected_snapshot, runtime.snapshot()));
        assert!(Arc::ptr_eq(built.source_snapshot(), &expected_snapshot));
    }

    #[test]
    fn generation_request_rejects_another_requests_validated_output() {
        let mut first_runtime = runtime();
        let first_snapshot = topology_update(first_runtime.snapshot(), Stage::S4);
        let first = take_start(commit_snapshot(&mut first_runtime, first_snapshot));
        let first_built = first.build_scene_generation().unwrap();

        let mut other_runtime = runtime();
        let second_snapshot = topology_update(other_runtime.snapshot(), Stage::S5);
        let second = take_start(commit_snapshot(&mut other_runtime, second_snapshot));
        assert_eq!(
            second.accept_generation(first_built),
            Err(GenerationAcceptanceError::IdentityMismatch)
        );
    }

    #[test]
    fn generation_request_seal_rejects_same_value_and_same_arc_cross_request_builds() {
        let snapshot = snapshot();
        let key = SceneGenerationKey {
            device: DeviceEpoch(7),
            layout: LayoutGeneration(8),
            resources: ResourceGeneration(9),
        };
        let source = AppliedRevisions::new(3, 5);
        let first = GenerationRequest {
            request_id: RequestId(10),
            key,
            surface: SurfaceEpoch(1),
            source,
            snapshot: Arc::clone(&snapshot),
            seal: Arc::new(()),
        };
        let same_arc_other_request = GenerationRequest {
            request_id: RequestId(11),
            key,
            surface: SurfaceEpoch(1),
            source,
            snapshot: Arc::clone(&snapshot),
            seal: Arc::new(()),
        };
        assert_eq!(
            same_arc_other_request.accept_generation(first.build_scene_generation().unwrap()),
            Err(GenerationAcceptanceError::IdentityMismatch)
        );

        let equal_value_other_arc = GenerationRequest {
            request_id: RequestId(12),
            key,
            surface: SurfaceEpoch(1),
            source,
            snapshot: Arc::new((*snapshot).clone()),
            seal: Arc::new(()),
        };
        let origin = GenerationRequest {
            request_id: RequestId(13),
            key,
            surface: SurfaceEpoch(1),
            source,
            snapshot,
            seal: Arc::new(()),
        };
        assert_eq!(
            equal_value_other_arc.accept_generation(origin.build_scene_generation().unwrap()),
            Err(GenerationAcceptanceError::IdentityMismatch)
        );
    }

    #[test]
    fn real_candidate_and_active_lifecycle_rebases_complete_compiled_generation() {
        let mut runtime = runtime();
        let generated_snapshot = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(
            &mut runtime,
            Arc::clone(&generated_snapshot),
        ));
        let key = request.key();
        let built = request.build_scene_generation().unwrap();
        let candidate = request.accept_generation(built).unwrap();
        assert!(matches!(
            runtime.complete_candidate(candidate).disposition(),
            RuntimeDisposition::CandidateReady(_)
        ));

        let mut desired = (*generated_snapshot).clone();
        desired.content.palette.body[0] ^= 1;
        desired.content.prop_animation_states[0].motion_phase = Some(1);
        desired.frame.gauge_levels[0] = GaugeLevelSnapshot::High;
        desired.frame.gauge_fractions[0] = 0.75;
        let desired = Arc::new(desired);
        commit_snapshot(&mut runtime, Arc::clone(&desired));
        let rebased_desired = Arc::clone(runtime.snapshot());
        runtime.rebase_ready_candidate().unwrap();

        let expected = super::super::scene::build_scene_generation_owned(
            Arc::clone(&rebased_desired),
            key,
            runtime.pending_desired_source().unwrap(),
        )
        .unwrap();
        let PendingPhase::Ready(candidate) = &runtime.pending.as_ref().unwrap().phase else {
            panic!("candidate must remain ready after exact rebase");
        };
        assert_eq!(candidate.generation.template(), expected.template());
        assert_eq!(candidate.generation.content(), expected.content());
        assert_eq!(candidate.generation.frame(), expected.frame());
        assert_eq!(
            candidate.generation.content_checksum(),
            expected.content_checksum()
        );
        assert_eq!(
            candidate.generation.frame_checksum(),
            expected.frame_checksum()
        );
        assert!(Arc::ptr_eq(
            candidate.generation.source_snapshot(),
            &rebased_desired
        ));

        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: runtime.surface_epoch },
        );
        let lease = runtime.capture_lease().unwrap();
        assert_eq!(lease.template(), expected.template());
        assert_eq!(lease.content(), expected.content());
        assert_eq!(lease.frame(), expected.frame());
        assert_eq!(lease.content_checksum(), expected.content_checksum());
        assert_eq!(lease.frame_checksum(), expected.frame_checksum());

        let mut active_update = (*rebased_desired).clone();
        active_update.frame.dimmed = true;
        active_update.frame.dim_amount = 0.25;
        active_update.frame.prop_instances[0].origin_points[0] += 5.0;
        let active_update = Arc::new(active_update);
        commit_snapshot(&mut runtime, Arc::clone(&active_update));
        let expected = super::super::scene::build_scene_generation_owned(
            active_update,
            key,
            runtime.active_version().unwrap().applied,
        )
        .unwrap();
        let lease = runtime.capture_lease().unwrap();
        assert_eq!(lease.content(), expected.content());
        assert_eq!(lease.frame(), expected.frame());
        assert_eq!(lease.content_checksum(), expected.content_checksum());
        assert_eq!(lease.frame_checksum(), expected.frame_checksum());
    }

    #[test]
    fn ready_rebase_rejects_stale_candidate_revision_without_mutating_generation() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        let built = request.build_scene_generation().unwrap();
        runtime.complete_candidate(request.accept_generation(built).unwrap());

        let mut changed = (**runtime.snapshot()).clone();
        offset_pet_depth(&mut changed, 0.1);
        commit_snapshot(&mut runtime, Arc::new(changed));
        let pending = runtime.pending.as_mut().unwrap();
        let PendingPhase::Ready(candidate) = &mut pending.phase else {
            panic!("candidate ready");
        };
        let before = candidate.generation.clone();
        candidate.applied = AppliedRevisions::new(0, 0);
        assert_eq!(
            runtime.rebase_ready_candidate(),
            Err(CandidateRebaseError::Projection(
                super::super::scene::SceneDeltaApplyError::StaleBase
            ))
        );
        let PendingPhase::Ready(candidate) = &runtime.pending.as_ref().unwrap().phase else {
            panic!("candidate remains ready");
        };
        assert_eq!(candidate.generation, before);
    }

    #[test]
    fn active_projection_error_leaves_runtime_and_compiled_generation_unchanged() {
        let mut runtime = runtime();
        let before_snapshot = Arc::clone(runtime.snapshot());
        let before_version = runtime.active_version().unwrap();
        let before_generation = runtime.active.as_ref().unwrap().generation.clone();
        let mut target = (*before_snapshot).clone();
        offset_pet_depth(&mut target, 0.1);
        let mut prepared = runtime.prepare_snapshot(Arc::new(target)).unwrap();
        Arc::make_mut(&mut prepared.snapshot).frame.pet_depth = f32::NAN;
        assert!(matches!(
            runtime.commit_prepared(prepared),
            Err(PreparedCommitError::Projection(_))
        ));
        assert!(Arc::ptr_eq(runtime.snapshot(), &before_snapshot));
        assert_eq!(runtime.active_version(), Some(before_version));
        assert_eq!(
            runtime.active.as_ref().unwrap().generation,
            before_generation
        );
    }

    #[derive(Clone, Copy)]
    enum CanonicalNoopPhase {
        Preparing,
        Ready,
        Activating,
    }

    fn assert_canonical_noop_rebases_exact_arc(phase: CanonicalNoopPhase) {
        let mut runtime = runtime();
        let generated = topology_update(runtime.snapshot(), Stage::S4);
        let mut request = Some(take_start(commit_snapshot(&mut runtime, generated)));
        let mut activating = None;
        let expected = request.as_ref().unwrap().build_scene_generation().unwrap();
        let expected_content_checksum = expected.content_checksum();
        let expected_frame_checksum = expected.frame_checksum();
        if !matches!(phase, CanonicalNoopPhase::Preparing) {
            runtime
                .complete_candidate(request.take().unwrap().accept_generation(expected).unwrap());
            if matches!(phase, CanonicalNoopPhase::Activating) {
                activating = Some(runtime.begin_activation().unwrap());
            }
        }

        let before_revisions = runtime.pending_desired_source().unwrap();
        let mut raw_only = (**runtime.snapshot()).clone();
        raw_only.frame.elapsed_ms += 1;
        let raw_only = Arc::new(raw_only);
        assert_eq!(
            commit_snapshot(&mut runtime, Arc::clone(&raw_only)).disposition(),
            RuntimeDisposition::Unchanged
        );
        assert_eq!(runtime.pending_desired_source(), Some(before_revisions));
        assert!(Arc::ptr_eq(
            &runtime.pending.as_ref().unwrap().desired_snapshot,
            &raw_only
        ));

        if matches!(phase, CanonicalNoopPhase::Preparing) {
            let expected = request.as_ref().unwrap().build_scene_generation().unwrap();
            runtime
                .complete_candidate(request.take().unwrap().accept_generation(expected).unwrap());
        }
        if let Some(attempt) = activating {
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean { surface: runtime.surface_epoch },
            );
        }
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );
        runtime.rebase_ready_candidate().unwrap();
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: runtime.surface_epoch },
        );
        let capture = runtime.capture_lease().unwrap();
        assert!(Arc::ptr_eq(runtime.snapshot(), capture.source_snapshot()));
        assert_eq!(capture.version().applied, before_revisions);
        assert_eq!(capture.content_checksum(), expected_content_checksum);
        assert_eq!(capture.frame_checksum(), expected_frame_checksum);
    }

    #[test]
    fn canonical_noop_commits_rebase_exact_arc_while_preparing_ready_and_activating() {
        for phase in [
            CanonicalNoopPhase::Preparing,
            CanonicalNoopPhase::Ready,
            CanonicalNoopPhase::Activating,
        ] {
            assert_canonical_noop_rebases_exact_arc(phase);
        }
    }

    #[test]
    fn active_canonical_noop_retargets_exact_arc_without_changing_rendered_state() {
        let mut runtime = runtime();
        let before = runtime.capture_lease().unwrap();
        let before_version = before.version();
        let before_content = before.content().clone();
        let before_frame = before.frame().clone();
        let before_content_checksum = before.content_checksum();
        let before_frame_checksum = before.frame_checksum();

        let mut raw_only = (**runtime.snapshot()).clone();
        raw_only.frame.elapsed_ms += 1;
        let raw_only = Arc::new(raw_only);
        assert_eq!(
            commit_snapshot(&mut runtime, Arc::clone(&raw_only)).disposition(),
            RuntimeDisposition::Unchanged
        );

        let capture = runtime.capture_lease().unwrap();
        assert!(Arc::ptr_eq(runtime.snapshot(), &raw_only));
        assert!(Arc::ptr_eq(capture.source_snapshot(), &raw_only));
        assert_eq!(capture.version(), before_version);
        assert_eq!(capture.content(), &before_content);
        assert_eq!(capture.frame(), &before_frame);
        assert_eq!(capture.content_checksum(), before_content_checksum);
        assert_eq!(capture.frame_checksum(), before_frame_checksum);
    }

    fn assert_coalesced_rebase(phase: CanonicalNoopPhase, mutations: Vec<SnapshotMutation>) {
        let mut runtime = runtime();
        let generated = topology_update(runtime.snapshot(), Stage::S4);
        let mut request = Some(take_start(commit_snapshot(&mut runtime, generated)));
        if matches!(phase, CanonicalNoopPhase::Ready) {
            let built = request.as_ref().unwrap().build_scene_generation().unwrap();
            runtime.complete_candidate(request.take().unwrap().accept_generation(built).unwrap());
        }
        for (_, mutate) in mutations {
            let mut next = (**runtime.snapshot()).clone();
            mutate(&mut next);
            commit_snapshot(&mut runtime, Arc::new(next));
        }
        if matches!(phase, CanonicalNoopPhase::Preparing) {
            let built = request.as_ref().unwrap().build_scene_generation().unwrap();
            runtime.complete_candidate(request.take().unwrap().accept_generation(built).unwrap());
        }
        let target = Arc::clone(runtime.snapshot());
        let target_revisions = runtime.pending_desired_source().unwrap();
        runtime.rebase_ready_candidate().unwrap();
        let PendingPhase::Ready(candidate) = &runtime.pending.as_ref().unwrap().phase else {
            panic!("coalesced candidate ready");
        };
        assert_eq!(candidate.generation.source_revisions(), target_revisions);
        assert!(Arc::ptr_eq(candidate.generation.source_snapshot(), &target));
        let fresh = super::super::scene::build_scene_generation_owned(
            target,
            candidate.key,
            target_revisions,
        )
        .unwrap();
        assert_eq!(candidate.generation.content(), fresh.content());
        assert_eq!(candidate.generation.frame(), fresh.frame());
        assert_eq!(
            candidate.generation.content_checksum(),
            fresh.content_checksum()
        );
        assert_eq!(
            candidate.generation.frame_checksum(),
            fresh.frame_checksum()
        );

        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: runtime.surface_epoch },
        );
        let capture = runtime.capture_lease().unwrap();
        assert_eq!(capture.version().applied, target_revisions);
        assert!(Arc::ptr_eq(capture.source_snapshot(), runtime.snapshot()));
    }

    #[test]
    fn preparing_and_ready_candidates_coalesce_multi_revision_and_revert_updates() {
        for phase in [CanonicalNoopPhase::Preparing, CanonicalNoopPhase::Ready] {
            assert_coalesced_rebase(
                phase,
                vec![
                    ("semantic one", Box::new(|s| s.content.mood = Mood::Content)),
                    ("semantic two", Box::new(|s| s.content.palette.eye[0] ^= 1)),
                ],
            );
            assert_coalesced_rebase(
                phase,
                vec![
                    ("frame one", Box::new(|s| offset_pet_depth(s, 0.1))),
                    (
                        "frame two",
                        Box::new(|s| {
                            s.frame.dimmed = true;
                            s.frame.dim_amount = 0.25;
                        }),
                    ),
                ],
            );
            assert_coalesced_rebase(
                phase,
                vec![
                    (
                        "mixed one",
                        Box::new(|s| {
                            s.content.mood = Mood::Content;
                            offset_pet_depth(s, 0.1);
                        }),
                    ),
                    (
                        "mixed two",
                        Box::new(|s| {
                            s.content.palette.eye[0] ^= 1;
                            s.frame.dimmed = true;
                            s.frame.dim_amount = 0.25;
                        }),
                    ),
                ],
            );
            assert_coalesced_rebase(
                phase,
                vec![
                    ("forward", Box::new(|s| offset_pet_depth(s, 0.1))),
                    ("revert", Box::new(|s| offset_pet_depth(s, -0.1))),
                ],
            );
        }
    }

    fn runtime() -> CompanionSceneRuntimeState {
        CompanionSceneRuntimeState::with_active(snapshot()).unwrap()
    }

    fn topology_update(
        base: &Arc<CompanionSceneSnapshot>,
        stage: Stage,
    ) -> Arc<CompanionSceneSnapshot> {
        let mut next = (**base).clone();
        next.topology.pet.stage = stage;
        Arc::new(next)
    }

    #[derive(Default)]
    struct FakeWorkerDispatcher {
        live: Option<RequestId>,
        cancelling: Option<RequestId>,
        starts: Vec<RequestId>,
    }

    impl FakeWorkerDispatcher {
        fn apply(&mut self, effects: &mut RuntimeEffects) -> Option<GenerationRequest> {
            if let Some(cancel) = effects.take_cancel_worker() {
                let cancel = cancel.request_id();
                assert_eq!(self.live, Some(cancel));
                self.live = None;
                self.cancelling = Some(cancel);
            }
            let request = effects.take_start_worker();
            if let Some(request) = &request {
                assert!(self.live.is_none(), "a second worker was started");
                assert!(self.cancelling.is_none(), "start preceded cancellation ack");
                self.live = Some(request.request_id());
                self.starts.push(request.request_id());
            }
            request
        }

        fn acknowledge_cancel(&mut self, id: RequestId) {
            assert_eq!(self.cancelling.take(), Some(id));
        }

        fn complete(&mut self, id: RequestId) {
            if self.live == Some(id) {
                self.live = None;
            } else {
                assert_eq!(self.cancelling.take(), Some(id));
            }
        }
    }

    fn commit_snapshot(
        runtime: &mut CompanionSceneRuntimeState,
        snapshot: Arc<CompanionSceneSnapshot>,
    ) -> RuntimeEffects {
        let prepared = runtime.prepare_snapshot(snapshot).unwrap();
        runtime.commit_prepared(prepared).unwrap()
    }

    fn take_start(mut effects: RuntimeEffects) -> GenerationRequest {
        effects
            .take_start_worker()
            .expect("expected owned start action")
    }

    fn reveal(runtime: &mut CompanionSceneRuntimeState) -> RuntimeEffects {
        let prepared = runtime.prepare_reveal().unwrap();
        runtime.commit_reveal(prepared).unwrap()
    }

    #[test]
    fn running_worker_transitions_emit_exactly_one_owned_start_action() {
        let mut runtime = runtime();
        let mut dispatcher = FakeWorkerDispatcher::default();
        let first_snapshot = topology_update(runtime.snapshot(), Stage::S4);
        let mut first_effects = commit_snapshot(&mut runtime, first_snapshot);
        assert!(matches!(
            first_effects.disposition(),
            RuntimeDisposition::GenerationStarted(_)
        ));
        let first = dispatcher.apply(&mut first_effects).unwrap();
        assert!(first_effects.take_start_worker().is_none());

        let second_snapshot = topology_update(runtime.snapshot(), Stage::S5);
        let mut second_effects = commit_snapshot(&mut runtime, second_snapshot);
        assert!(matches!(
            second_effects.disposition(),
            RuntimeDisposition::GenerationQueued(_)
        ));
        assert!(second_effects.take_start_worker().is_none());
        dispatcher.apply(&mut second_effects);
        dispatcher.acknowledge_cancel(first.request_id());
        let mut ack = runtime.acknowledge_worker_cancelled(first.request_id());
        dispatcher.apply(&mut ack);
        assert_eq!(dispatcher.starts.len(), 2);
        assert_eq!(
            runtime.worker,
            WorkerState::Running(dispatcher.live.unwrap())
        );

        let mut alternate = CompanionSceneRuntimeState::with_active(snapshot()).unwrap();
        let first_snapshot = topology_update(alternate.snapshot(), Stage::S4);
        let mut first_effects = commit_snapshot(&mut alternate, first_snapshot);
        let mut dispatcher = FakeWorkerDispatcher::default();
        let first = dispatcher.apply(&mut first_effects).unwrap();
        let second_snapshot = topology_update(alternate.snapshot(), Stage::S5);
        let mut queued = commit_snapshot(&mut alternate, second_snapshot);
        dispatcher.apply(&mut queued);
        dispatcher.complete(first.request_id());
        let mut completion = alternate.complete_candidate(first.accept());
        dispatcher.apply(&mut completion);
        assert_eq!(dispatcher.starts.len(), 2);
    }

    #[test]
    fn emitted_worker_and_cleanup_actions_are_one_shot() {
        let mut runtime = runtime();
        let first_snapshot = topology_update(runtime.snapshot(), Stage::S4);
        let mut first_effects = commit_snapshot(&mut runtime, first_snapshot);
        let first = first_effects.take_start_worker().unwrap();
        assert!(first_effects.take_start_worker().is_none());

        let first_id = first.request_id();
        let first_candidate = first.accept();
        let second_snapshot = topology_update(runtime.snapshot(), Stage::S5);
        let mut second_effects = commit_snapshot(&mut runtime, second_snapshot);
        assert_eq!(
            second_effects
                .take_cancel_worker()
                .map(|action| action.request_id()),
            Some(first_id)
        );
        assert_eq!(second_effects.take_cancel_worker(), None);

        let mut completion = runtime.complete_candidate(first_candidate);
        let second = completion.take_start_worker().unwrap();
        assert!(completion.take_start_worker().is_none());
        runtime.complete_candidate(second.accept());
        let mut third_snapshot = (**runtime.snapshot()).clone();
        third_snapshot.topology.pet.stage = Stage::S6;
        let mut third = commit_snapshot(&mut runtime, Arc::new(third_snapshot));
        assert!(third.take_drop_candidate().is_some());
        assert_eq!(third.take_drop_candidate(), None);
    }

    #[test]
    fn prepared_updates_publish_only_after_exact_commit() {
        let mut runtime = runtime();
        let initial_version = runtime.active_version().unwrap();
        let initial_snapshot = Arc::clone(runtime.snapshot());
        let mut next = (*initial_snapshot).clone();
        next.content.mood = Mood::Content;
        let rejected = runtime.prepare_snapshot(Arc::new(next)).unwrap();
        assert_eq!(runtime.capture_lease().unwrap().version(), initial_version);
        drop(rejected);
        assert!(Arc::ptr_eq(runtime.snapshot(), &initial_snapshot));
        assert_eq!(runtime.active_version(), Some(initial_version));

        let mut first = (*initial_snapshot).clone();
        first.content.mood = Mood::Content;
        let stale = runtime.prepare_snapshot(Arc::new(first)).unwrap();
        let mut latest = (*initial_snapshot).clone();
        latest.content.palette.eye[0] += 1;
        offset_pet_depth(&mut latest, 0.1);
        let latest = runtime.prepare_snapshot(Arc::new(latest)).unwrap();
        assert!(latest
            .changes()
            .semantic()
            .contains(SemanticChangeMask::PALETTE));
        assert!(latest.frame_revision().0 > initial_version.applied.frame.0);
        runtime.commit_prepared(latest).unwrap();
        assert_eq!(
            runtime.commit_prepared(stale),
            Err(PreparedCommitError::StaleBase)
        );
    }

    #[test]
    fn two_pose_semantic_rebase_keeps_exactly_one_foreground_parallax_offset() {
        let clock = parallax_regression_clock();
        let mut previous = (*snapshot()).clone();
        previous.topology.visible_props[0].catalog_id = crate::game::habitat::TOKEN_PEBBLE_25K;
        previous.topology.visible_props[0].authored_depth = AuthoredDepthSnapshot::Foreground;
        previous.topology.visible_props[0].shadow_profile =
            crate::game::habitat::catalog_prop_by_str(crate::game::habitat::TOKEN_PEBBLE_25K)
                .unwrap()
                .shadow_profile;
        previous.topology.visible_props[0].presentation_motion = PropPresentationMotion::Static;
        previous.content.prop_animation_states[0].catalog_id =
            crate::game::habitat::TOKEN_PEBBLE_25K;
        previous.content.prop_animation_states[0].motion_phase = Some(0);
        previous.content.prop_animation_states[0].chest_lid_open = None;
        previous.frame.prop_instances[0].transition = None;
        previous.frame = previous
            .project_presentation_frame(
                SemanticRevision(0),
                clock,
                super::super::input::CompanionPresentationOptions::STANDARD,
            )
            .unwrap()
            .frame;
        previous.topology.visible_props[0].presentation_motion =
            PropPresentationMotion::TwoPoseEase {
                duration_ms: 900,
                curve: super::super::EaseCurve::SmoothStep,
            };
        let depth_parallax = expected_depth_parallax(
            &previous,
            clock,
            AuthoredDepthSnapshot::Foreground.parallax_multiplier(),
        );
        let expected_prop_parallax = [depth_parallax[0], -depth_parallax[1]];
        assert!(
            expected_prop_parallax != [0.0; 2],
            "fixture needs nonzero displacement"
        );
        assert_points_close(
            previous.frame.prop_instances[0].motion_offset_points,
            expected_prop_parallax,
        );
        let previous = Arc::new(previous);

        let mut newest = (*previous).clone();
        newest.content.prop_animation_states[0].motion_phase = Some(1);
        newest.frame.prop_instances[0].motion_offset_points =
            [expected_prop_parallax[0], 3.0 + expected_prop_parallax[1]];
        newest.frame.prop_instances[0].transition = None;
        let mut runtime = CompanionSceneRuntimeState::with_active(Arc::clone(&previous)).unwrap();
        commit_snapshot(&mut runtime, Arc::new(newest));

        let rebased = runtime.snapshot();
        let anchor = rebased.frame.prop_instances[0]
            .transition
            .expect("semantic change starts a two-pose transition");
        assert_eq!(anchor.source_pose, [0.0; 2]);
        assert_eq!(anchor.target_pose, [0.0, 3.0]);
        assert_points_close(
            rebased.frame.prop_instances[0].motion_offset_points,
            previous.frame.prop_instances[0].motion_offset_points,
        );

        let projected = rebased
            .project_presentation_frame(
                runtime.applied_revisions().semantic,
                clock,
                super::super::input::CompanionPresentationOptions::STANDARD,
            )
            .unwrap();
        assert_points_close(
            projected.frame.prop_instances[0].motion_offset_points,
            expected_prop_parallax,
        );
    }

    #[test]
    fn tank_semantic_rebase_keeps_parallax_out_of_anchors_and_reduce_motion() {
        let clock = parallax_regression_clock();
        let mut previous = (*snapshot()).clone();
        previous.frame = previous
            .project_presentation_frame(
                SemanticRevision(0),
                clock,
                super::super::input::CompanionPresentationOptions::STANDARD,
            )
            .unwrap()
            .frame;
        let expected_parallax = expected_depth_parallax(
            &previous,
            clock,
            AuthoredDepthSnapshot::BehindPet.parallax_multiplier(),
        );
        assert!(
            expected_parallax != [0.0; 2],
            "fixture needs nonzero displacement"
        );
        assert_points_close(
            previous.frame.tank_instances[0].cells[0].position_points,
            [40.0 + expected_parallax[0], 50.0 + expected_parallax[1]],
        );
        let previous = Arc::new(previous);

        let semantic_target = [48.0, 60.0];
        let mut newest = (*previous).clone();
        newest.content.tank_animation_states[0].cells[0].glyph = '>';
        newest.frame.tank_instances[0].cells[0] = TankCellFrameSnapshot {
            source_position_points: semantic_target,
            position_points: [
                semantic_target[0] + expected_parallax[0],
                semantic_target[1] + expected_parallax[1],
            ],
            target_position_points: semantic_target,
        };
        let mut runtime = CompanionSceneRuntimeState::with_active(Arc::clone(&previous)).unwrap();
        commit_snapshot(&mut runtime, Arc::new(newest));

        let rebased = runtime.snapshot();
        let cell = rebased.frame.tank_instances[0].cells[0];
        assert_eq!(cell.source_position_points, [40.0, 50.0]);
        assert_eq!(cell.target_position_points, semantic_target);
        assert_points_close(
            cell.position_points,
            previous.frame.tank_instances[0].cells[0].position_points,
        );

        let projected = rebased
            .project_presentation_frame(
                runtime.applied_revisions().semantic,
                clock,
                super::super::input::CompanionPresentationOptions::STANDARD,
            )
            .unwrap();
        assert_points_close(
            projected.frame.tank_instances[0].cells[0].position_points,
            [40.0 + expected_parallax[0], 50.0 + expected_parallax[1]],
        );

        let reduced = rebased
            .project_presentation_frame(
                runtime.applied_revisions().semantic,
                clock,
                super::super::input::CompanionPresentationOptions { reduce_motion: true },
            )
            .unwrap();
        assert_eq!(
            reduced.frame.tank_instances[0].cells[0].position_points,
            semantic_target
        );
    }

    #[test]
    fn accepted_candidate_metadata_is_bound_by_runtime_authority() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        let identity = request.identity();
        let candidate = request.accept();
        assert_eq!(candidate.request_id, identity.request_id());
        assert_eq!(candidate.key, identity.key());
        assert_eq!(candidate.applied, identity.source());
        runtime.complete_candidate(candidate);

        let mut semantic = (**runtime.snapshot()).clone();
        semantic.content.mood = Mood::Content;
        commit_snapshot(&mut runtime, Arc::new(semantic));
        runtime.rebase_ready_candidate().unwrap();
    }

    #[test]
    fn compatible_updates_require_runtime_owned_exact_rebase() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        let original_source = request.source();
        let mut semantic = (**runtime.snapshot()).clone();
        semantic.content.mood = Mood::Content;
        commit_snapshot(&mut runtime, Arc::new(semantic));
        assert_eq!(
            request.source(),
            original_source,
            "worker request is immutable"
        );
        assert_ne!(runtime.pending_desired_source(), Some(original_source));
        runtime.complete_candidate(request.accept());
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );
        runtime.rebase_ready_candidate().unwrap();
        assert!(runtime.begin_activation().is_ok());
    }

    #[test]
    fn frame_rebase_updates_the_complete_compiled_generation() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        runtime.complete_candidate(request.accept());
        let mut frame = (**runtime.snapshot()).clone();
        offset_pet_depth(&mut frame, 0.1);
        commit_snapshot(&mut runtime, Arc::new(frame));

        runtime.rebase_ready_candidate().unwrap();
        assert!(runtime.begin_activation().is_ok());
    }

    #[test]
    fn repeated_rebase_diffs_from_last_accepted_projection() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        runtime.complete_candidate(request.accept());
        let original_depth = runtime.snapshot().frame.pet_depth;

        let mut forward = (**runtime.snapshot()).clone();
        offset_pet_depth(&mut forward, 0.1);
        commit_snapshot(&mut runtime, Arc::new(forward));
        runtime.rebase_ready_candidate().unwrap();

        let mut backward = (**runtime.snapshot()).clone();
        set_pet_depth(&mut backward, original_depth);
        commit_snapshot(&mut runtime, Arc::new(backward));
        runtime.rebase_ready_candidate().unwrap();
        let PendingPhase::Ready(candidate) = &runtime.pending.as_ref().unwrap().phase else {
            panic!("candidate remains ready");
        };
        assert_eq!(
            candidate.generation.source_snapshot().frame.pet_depth,
            original_depth
        );
    }

    #[test]
    fn production_resource_invalidation_preserves_layout_and_emits_work() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        let effects = runtime
            .invalidate_resources(ResourceInvalidation::MaterialContract)
            .unwrap();
        let request = take_start(effects);
        assert_eq!(request.key().layout, before.generation.layout);
        assert_eq!(request.key().resources, ResourceGeneration(2));
        assert_eq!(runtime.active_version(), Some(before));
    }

    #[test]
    fn surface_recovery_advances_surface_and_resources_without_layout_relabel() {
        let (mut runtime, _, attempt) = runtime_with_activation();
        let before = runtime.active_version().unwrap();
        let layout = runtime.reconciler.layout_generation();
        let resources = runtime.resource_generation;
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
        );
        let request = take_start(runtime.acknowledge_surface_rebound().unwrap());
        assert_eq!(request.surface(), SurfaceEpoch(before.surface.0 + 1));
        assert_eq!(request.key().layout, layout);
        assert_eq!(request.key().resources, ResourceGeneration(resources.0 + 1));
        assert_eq!(runtime.active_version(), Some(before));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::RecoveryInProgress
        );
    }

    #[test]
    fn operational_surface_rebind_relabels_only_surface_binding() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        let counters = (
            runtime.reconciler.layout_generation(),
            runtime.resource_generation,
            runtime.reconciler.applied_revisions(),
            runtime.next_request_id,
        );
        let mut effects = runtime.acknowledge_operational_surface_rebound().unwrap();
        let after = runtime.active_version().unwrap();
        assert_eq!(after.surface, SurfaceEpoch(before.surface.0 + 1));
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.applied, before.applied);
        assert_eq!(runtime.capture_lease().unwrap().version(), after);
        assert_eq!(
            (
                runtime.reconciler.layout_generation(),
                runtime.resource_generation,
                runtime.reconciler.applied_revisions(),
                runtime.next_request_id,
            ),
            counters
        );
        assert!(effects.take_start_worker().is_none());
        assert!(effects.take_cancel_worker().is_none());
        assert!(effects.take_drop_candidate().is_none());
    }

    #[test]
    fn host_owned_surface_rebind_requires_the_exact_next_epoch() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        assert_eq!(
            runtime.acknowledge_operational_surface_rebound_to(before.surface),
            Err(RuntimeError::RecoveryActionRejected)
        );
        assert_eq!(
            runtime.acknowledge_operational_surface_rebound_to(SurfaceEpoch(before.surface.0 + 2)),
            Err(RuntimeError::RecoveryActionRejected)
        );
        runtime
            .acknowledge_operational_surface_rebound_to(SurfaceEpoch(before.surface.0 + 1))
            .unwrap();
        assert_eq!(
            runtime.active_version().unwrap().surface,
            SurfaceEpoch(before.surface.0 + 1)
        );
    }

    #[test]
    fn logical_resize_and_scale_invalidation_queue_one_coalesced_replacement() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        let mut resized = (**runtime.snapshot()).clone();
        resized.topology.layout.width_points += 40.0;
        resized.topology.glyph_grid.cell_extent_points[0] =
            resized.topology.layout.width_points / f32::from(resized.topology.glyph_grid.columns);
        let prepared = runtime
            .prepare_snapshot_with_resource_invalidation(
                Arc::new(resized),
                Some(ResourceInvalidation::BackingScaleAtlas),
            )
            .unwrap();
        assert!(prepared
            .changes()
            .resources()
            .contains(ResourceChangeMask::BACKING_SCALE_ATLAS));
        let request = take_start(runtime.commit_prepared(prepared).unwrap());
        assert_eq!(
            request.key().layout,
            LayoutGeneration(before.generation.layout.0 + 1)
        );
        assert_eq!(
            request.key().resources,
            ResourceGeneration(before.generation.resources.0 + 1)
        );
        assert_eq!(runtime.next_request_id.0, request.request_id().0 + 1);
    }

    #[test]
    fn bounded_retry_supersedes_inflight_replacement_and_shutdown_rejects_late_candidate() {
        let mut runtime = runtime();
        let replacement = topology_update(runtime.snapshot(), Stage::S4);
        let first = take_start(commit_snapshot(&mut runtime, replacement));
        let mut retry_effects = runtime.retry_current_generation().unwrap();
        assert_eq!(
            retry_effects.take_cancel_worker().unwrap().request_id(),
            first.request_id()
        );
        assert!(retry_effects.take_start_worker().is_none());
        let retry_identity = runtime.pending_request_identity().unwrap();
        assert_ne!(retry_identity.request_id(), first.request_id());
        runtime.shutdown();
        let mut late = runtime.complete_candidate(first.accept());
        assert_eq!(late.disposition(), RuntimeDisposition::DroppedStale);
        assert!(late.take_drop_candidate().is_some());
        assert_eq!(
            runtime.retry_current_generation(),
            Err(RuntimeError::Shutdown)
        );
    }

    #[test]
    fn operational_surface_rebind_preserves_ready_candidate_for_new_surface() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        let identity = request.identity();
        runtime.complete_candidate(request.accept());
        assert!(matches!(
            runtime.pending.as_ref().unwrap().phase,
            PendingPhase::Ready(_)
        ));

        let effects = runtime.acknowledge_operational_surface_rebound().unwrap();
        assert!(effects.start_worker.is_none());
        assert!(matches!(
            runtime.pending.as_ref().unwrap().phase,
            PendingPhase::Ready(_)
        ));
        assert_eq!(runtime.pending_request_identity(), Some(identity));
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(attempt.surface, SurfaceEpoch(2));
    }

    #[test]
    fn operational_rebind_keeps_old_activation_tracked_but_commit_ineligible() {
        for outcome in [
            ActivationAttemptOutcome::PresentedClean { surface: SurfaceEpoch(1) },
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceValidation),
        ] {
            let (mut runtime, _, old_attempt) = runtime_with_activation();
            runtime.acknowledge_operational_surface_rebound().unwrap();
            assert!(matches!(
                runtime.pending.as_ref().unwrap().phase,
                PendingPhase::Activating { commit_eligible: false, .. }
            ));
            let before = runtime.active_version();
            let effects = runtime.finish_activation(old_attempt, outcome);
            assert!(matches!(
                effects.disposition(),
                RuntimeDisposition::DroppedStale
                    | RuntimeDisposition::Activation(ActivationTransition::DroppedStale)
            ));
            assert_eq!(runtime.active_version(), before);
            assert!(matches!(
                runtime.pending.as_ref().unwrap().phase,
                PendingPhase::Ready(_)
            ));
            let retry = runtime.begin_activation().unwrap();
            assert_eq!(retry.surface, SurfaceEpoch(2));
        }
    }

    #[test]
    fn prepared_snapshot_shares_arc_and_change_masks_remain_fixed_size() {
        assert_eq!(std::mem::size_of::<SnapshotChangeSet>(), 32);
        let mut runtime = runtime();
        let mut frame = (**runtime.snapshot()).clone();
        offset_pet_depth(&mut frame, 0.1);
        let next = Arc::new(frame);
        let prepared = runtime.prepare_snapshot(Arc::clone(&next)).unwrap();
        assert!(Arc::ptr_eq(prepared.snapshot(), &next));
        runtime.commit_prepared(prepared).unwrap();
        assert!(Arc::ptr_eq(runtime.snapshot(), &next));
        assert!(Arc::ptr_eq(
            runtime.capture_lease().unwrap().source_snapshot(),
            &next
        ));
        assert_eq!(Arc::strong_count(&next), 3);
    }

    #[test]
    fn task8_frame_projection_reuses_semantics_rejects_stale_base_and_never_generates() {
        let mut runtime = runtime();
        let initial_version = runtime.active_version().unwrap();
        let initial_topology = runtime.snapshot().topology.clone();
        let initial_content = runtime.snapshot().content.clone();
        let projection = runtime
            .snapshot()
            .project_presentation_frame(
                runtime.applied_revisions().semantic,
                super::super::CompanionProjectionClock::new(
                    time::OffsetDateTime::UNIX_EPOCH,
                    1_033,
                ),
                super::super::input::CompanionPresentationOptions::STANDARD,
            )
            .unwrap();
        let prepared = runtime.prepare_frame_projection(projection).unwrap();
        let mut effects = runtime.commit_frame_projection(prepared).unwrap();
        assert!(effects.take_start_worker().is_none());
        assert_eq!(
            runtime.active_version().unwrap().generation,
            initial_version.generation
        );
        assert_eq!(
            runtime.applied_revisions().semantic,
            initial_version.applied.semantic
        );
        assert_eq!(
            runtime.applied_revisions().frame,
            FrameRevision(initial_version.applied.frame.0 + 1)
        );
        assert!(runtime.snapshot().topology.ptr_eq(&initial_topology));
        assert!(runtime.snapshot().content.ptr_eq(&initial_content));

        let stale = runtime
            .snapshot()
            .project_presentation_frame(
                SemanticRevision(initial_version.applied.semantic.0 + 1),
                super::super::CompanionProjectionClock::new(
                    time::OffsetDateTime::UNIX_EPOCH,
                    1_066,
                ),
                super::super::input::CompanionPresentationOptions::STANDARD,
            )
            .unwrap();
        let expected_stale = SemanticRevision(initial_version.applied.semantic.0 + 1);
        assert!(matches!(
            runtime.prepare_frame_projection(stale),
            Err(RuntimeError::StaleSemanticBase { expected, actual })
                if expected == expected_stale && actual == initial_version.applied.semantic
        ));
    }

    #[test]
    fn frame_projection_with_backing_scale_change_queues_atlas_replacement() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        let projection = runtime
            .snapshot()
            .project_presentation_frame(
                runtime.applied_revisions().semantic,
                super::super::CompanionProjectionClock::new(
                    time::OffsetDateTime::UNIX_EPOCH,
                    1_033,
                ),
                super::super::input::CompanionPresentationOptions::STANDARD,
            )
            .unwrap();

        let prepared = runtime
            .prepare_frame_projection_with_resource_invalidation(
                projection,
                Some(ResourceInvalidation::BackingScaleAtlas),
            )
            .unwrap();
        let mut effects = runtime.commit_frame_projection(prepared).unwrap();
        let request = effects.take_start_worker().unwrap();

        assert_eq!(request.key().layout, before.generation.layout);
        assert_eq!(
            request.key().resources,
            ResourceGeneration(before.generation.resources.0 + 1)
        );
        assert_eq!(request.surface(), before.surface);
    }

    #[test]
    fn layout_and_resource_generations_advance_only_for_their_own_lifetimes() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        let mut layout = (**runtime.snapshot()).clone();
        layout.topology.layout.width_points += 1.0;
        layout.topology.glyph_grid.cell_extent_points[0] =
            layout.topology.layout.width_points / f32::from(layout.topology.glyph_grid.columns);
        let request = take_start(commit_snapshot(&mut runtime, Arc::new(layout)));
        assert_eq!(
            request.key().layout,
            LayoutGeneration(before.generation.layout.0 + 1)
        );
        assert_eq!(request.key().resources, before.generation.resources);

        let mut mixed = (**runtime.snapshot()).clone();
        mixed.topology.layout.height_points += 1.0;
        mixed.topology.glyph_grid.cell_extent_points[1] =
            mixed.topology.layout.height_points / f32::from(mixed.topology.glyph_grid.rows);
        let mixed = Arc::new(mixed);
        let mut changes = classify_snapshot_changes(runtime.snapshot(), &mixed);
        changes.resources = ResourceChangeMask::MATERIAL_CONTRACT;
        let prepared = runtime.prepare_with_changes(mixed, changes, false).unwrap();
        let mut effects = runtime.commit_prepared(prepared).unwrap();
        let request = runtime.pending_request_identity().unwrap();
        assert_eq!(
            request.key().layout,
            LayoutGeneration(before.generation.layout.0 + 2)
        );
        assert_eq!(
            request.key().resources,
            ResourceGeneration(before.generation.resources.0 + 1)
        );
        assert!(effects.start_worker.is_none());
        assert!(effects.take_cancel_worker().is_some());
    }

    #[test]
    fn room_reshuffle_is_content_and_frame_work_not_generation_work() {
        let first = snapshot();
        let mut changed = (*first).clone();
        changed.content.room_glyphs.push(RoomGlyphContentSnapshot {
            slot: 0,
            glyph: '✦',
            color_srgb8: [12, 34, 56],
        });
        changed.frame.room_glyphs.push(RoomGlyphFrameSnapshot {
            slot: 0,
            visible: true,
            grid_cell: [3, 4],
            position_points: [18.0, 300.0],
            opacity: 1.0,
        });
        validate_snapshot(&changed).unwrap();
        let changes = classify_snapshot_changes(&first, &changed);
        assert!(!changes.requires_generation());
        assert!(changes.semantic().contains(SemanticChangeMask::ROOM_GLYPHS));
        assert!(changes.frame().contains(FrameChangeMask::ROOM_GLYPHS));
    }

    #[test]
    fn rejected_and_overflowed_preparation_leave_published_state_untouched() {
        let mut runtime = runtime();
        let base = Arc::clone(runtime.snapshot());
        let version = runtime.active_version();
        let request_id = runtime.next_request_id;

        let mut invalid = (*base).clone();
        invalid.topology.layout.width_points = f32::NAN;
        assert!(matches!(
            runtime.prepare_snapshot(Arc::new(invalid)),
            Err(RuntimeError::SnapshotRejected(_))
        ));
        assert!(Arc::ptr_eq(runtime.snapshot(), &base));
        assert_eq!(runtime.active_version(), version);
        assert_eq!(runtime.next_request_id, request_id);

        let mut over_capacity = (*base).clone();
        let prop = over_capacity.topology.visible_props[0].clone();
        over_capacity.topology.visible_props.resize(
            crate::presentation::companion_scene::MAX_VISIBLE_PROPS + 1,
            prop,
        );
        assert!(matches!(
            runtime.prepare_snapshot(Arc::new(over_capacity)),
            Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::FixedCapacity
            ))
        ));
        assert!(runtime.pending.is_none());

        runtime.reconciler.semantic_revision = SemanticRevision(u64::MAX);
        let mut semantic = (*base).clone();
        semantic.content.mood = Mood::Content;
        assert_eq!(
            runtime.prepare_snapshot(Arc::new(semantic)).unwrap_err(),
            RuntimeError::CounterOverflow(CounterKind::SemanticRevision)
        );
        assert!(Arc::ptr_eq(runtime.snapshot(), &base));
        assert_eq!(runtime.active_version(), version);
        assert_eq!(runtime.next_request_id, request_id);
    }

    #[test]
    fn every_invalid_snapshot_family_is_transactionally_rejected() {
        type SnapshotMutation = Box<dyn Fn(&mut CompanionSceneSnapshot)>;
        let base = snapshot();
        let invalids: Vec<SnapshotMutation> = vec![
            Box::new(|s| s.schema_version += 1),
            Box::new(|s| s.topology.renderer_schema += 1),
            Box::new(|s| s.privacy.surface = PresentationSurface::WatchTui),
            Box::new(|s| s.privacy.source_names_visible = true),
            Box::new(|s| s.frame.pet_depth = f32::NAN),
            Box::new(|s| s.frame.pet_depth_cue.scale += 0.01),
            Box::new(|s| s.frame.pet_depth_cue.y_offset_points_up += 0.01),
            Box::new(|s| s.frame.pet_depth_cue.opacity -= 0.01),
            Box::new(|s| s.frame.pet_depth_cue.saturation -= 0.01),
            Box::new(|s| s.frame.asleep = true),
            Box::new(|s| s.frame.dimmed = true),
            Box::new(|s| s.topology.glyph_grid.cell_extent_points[0] = 0.0),
            Box::new(|s| s.topology.glyph_grid.cell_extent_points[1] = f32::NAN),
            Box::new(|s| {
                s.content.room_glyphs.push(RoomGlyphContentSnapshot {
                    slot: 0,
                    glyph: '✦',
                    color_srgb8: [1, 2, 3],
                });
                s.frame.room_glyphs.push(RoomGlyphFrameSnapshot {
                    slot: 0,
                    visible: false,
                    grid_cell: [1, 1],
                    position_points: [6.0, 336.0],
                    opacity: 1.0,
                });
            }),
            Box::new(|s| {
                s.content.room_glyphs.push(RoomGlyphContentSnapshot {
                    slot: 0,
                    glyph: '✦',
                    color_srgb8: [1, 2, 3],
                });
                s.frame.room_glyphs.push(RoomGlyphFrameSnapshot {
                    slot: 0,
                    visible: true,
                    grid_cell: [1, 1],
                    position_points: [6.0, 336.0],
                    opacity: 0.0,
                });
            }),
            Box::new(|s| {
                s.content.room_glyphs.push(RoomGlyphContentSnapshot {
                    slot: 0,
                    glyph: '✦',
                    color_srgb8: [1, 2, 3],
                });
                s.frame.room_glyphs.push(RoomGlyphFrameSnapshot {
                    slot: 0,
                    visible: true,
                    grid_cell: [1, 1],
                    position_points: [f32::NAN, 336.0],
                    opacity: 1.0,
                });
            }),
            Box::new(|s| s.content.prop_animation_states[0].catalog_id = "wrong"),
            Box::new(|s| s.content.tank_animation_states[0].stable_order = 1),
            Box::new(|s| {
                let prop = s.topology.visible_props[0].clone();
                s.topology.visible_props.resize(
                    crate::presentation::companion_scene::MAX_VISIBLE_PROPS + 1,
                    prop,
                );
            }),
        ];
        for mutate in invalids {
            let runtime = CompanionSceneRuntimeState::with_active(Arc::clone(&base)).unwrap();
            let before = runtime.active_version();
            let mut invalid = (*base).clone();
            mutate(&mut invalid);
            assert!(runtime.prepare_snapshot(Arc::new(invalid)).is_err());
            assert!(Arc::ptr_eq(runtime.snapshot(), &base));
            assert_eq!(runtime.active_version(), before);
            assert!(runtime.pending.is_none());
            assert_eq!(runtime.next_request_id, RequestId(1));
        }
    }

    #[test]
    fn snapshot_cast_lanes_require_matching_elevated_authored_profile() {
        let cast_snapshot = |catalog_id, profile| {
            let mut candidate = (*snapshot()).clone();
            candidate.topology.visible_props[0].catalog_id = catalog_id;
            candidate.topology.visible_props[0].shadow_profile = profile;
            candidate.content.prop_animation_states[0].catalog_id = catalog_id;
            candidate.content.prop_animation_states[0].bloom_active = None;
            let frame = &mut candidate.frame.prop_instances[0];
            frame.visible = true;
            frame.opacity = 1.0;
            frame.footprint_points = [12.0, 24.0];
            frame.cast_shadow_vector_points = [2.0, -10.0];
            frame.cast_shadow_softness_points = 2.0;
            frame.cast_shadow_strength = 0.2;
            candidate
        };

        for catalog_id in [
            crate::game::habitat::TOKEN_LANTERN_10M,
            crate::game::habitat::TOKEN_PEBBLE_25K,
        ] {
            let profile = crate::game::habitat::catalog_prop_by_str(catalog_id)
                .unwrap()
                .shadow_profile;
            let forged = cast_snapshot(catalog_id, profile);
            assert_eq!(
                validate_snapshot(&forged),
                Err(SnapshotRejection::InconsistentIdentity),
                "{profile:?}",
            );
        }

        let catalog_id = crate::game::habitat::TOKEN_TREASURE_CHEST_2M;
        let profile = crate::game::habitat::catalog_prop_by_str(catalog_id)
            .unwrap()
            .shadow_profile;
        assert_eq!(
            validate_snapshot(&cast_snapshot(catalog_id, profile)),
            Ok(())
        );
    }

    #[test]
    fn resolved_depth_and_sleep_are_canonical_snapshot_identity() {
        let base = snapshot();
        assert_eq!(validate_snapshot(&base), Ok(()));

        let mut calm = (*base).clone();
        calm.frame.calm = true;
        assert_eq!(
            validate_snapshot(&calm),
            Ok(()),
            "calm is not part of awake depth identity"
        );

        let cases: Vec<SnapshotMutation> = vec![
            (
                "scale",
                Box::new(|snapshot| snapshot.frame.pet_depth_cue.scale += 0.01),
            ),
            (
                "perspective",
                Box::new(|snapshot| {
                    snapshot.frame.pet_depth_cue.y_offset_points_up += 0.01;
                }),
            ),
            (
                "atmosphere",
                Box::new(|snapshot| snapshot.frame.pet_depth_cue.opacity -= 0.01),
            ),
            (
                "depth-saturation",
                Box::new(|snapshot| snapshot.frame.pet_depth_cue.saturation -= 0.01),
            ),
            (
                "sleep lifecycle",
                Box::new(|snapshot| {
                    snapshot.frame.asleep = true;
                    snapshot.frame.calm = true;
                }),
            ),
        ];
        for (name, mutate) in cases {
            let mut invalid = (*base).clone();
            mutate(&mut invalid);
            assert_eq!(
                validate_snapshot(&invalid),
                Err(SnapshotRejection::InconsistentIdentity),
                "{name}"
            );
        }
    }

    #[test]
    fn snapshot_rejects_pet_clearance_outside_aperture() {
        let mut invalid = (*snapshot()).clone();
        assert_ne!(invalid.frame.bob_offset_y_points, 0.0);
        assert_ne!(invalid.frame.pet_depth_cue.y_offset_points_up, 0.0);

        // On this 360x360, 60x30 fixture, y=236.8 keeps the maximum-scale
        // corners inside if the five-point bob is omitted. The actual retained
        // transform adds that bob and the depth perspective, moving the lower
        // corners just outside the physical circle.
        invalid.frame.pet_anchor_points[1] = 236.8;
        let mut without_bob = invalid.clone();
        without_bob.frame.bob_offset_y_points = 0.0;
        assert_eq!(validate_snapshot(&without_bob), Ok(()));
        assert_eq!(
            validate_snapshot(&invalid),
            Err(SnapshotRejection::InvalidValue)
        );
    }

    #[test]
    fn tank_paint_is_canonical_snapshot_identity() {
        let base = snapshot();
        assert_eq!(validate_snapshot(&base), Ok(()));

        for mutate in [
            |snapshot: &mut CompanionSceneSnapshot| {
                snapshot.content.tank_animation_states[0].color_srgb8[0] ^= 1;
            },
            |snapshot: &mut CompanionSceneSnapshot| {
                snapshot.content.tank_animation_states[0].bold = false;
            },
        ] {
            let mut invalid = (*base).clone();
            mutate(&mut invalid);
            assert_eq!(
                validate_snapshot(&invalid),
                Err(SnapshotRejection::InconsistentIdentity)
            );
        }

        let mut unknown = (*base).clone();
        unknown.topology.visible_tank_inhabitants[0].catalog_id = "future-tank-inhabitant";
        unknown.content.tank_animation_states[0].catalog_id = "future-tank-inhabitant";
        assert_eq!(
            validate_snapshot(&unknown),
            Err(SnapshotRejection::InconsistentIdentity)
        );
    }

    #[test]
    fn tank_paint_change_classifies_as_content_only() {
        let base = snapshot();
        let mut changed = (*base).clone();
        changed.content.tank_animation_states[0].color_srgb8[0] ^= 1;
        changed.content.tank_animation_states[0].bold = false;

        let changes = classify_snapshot_changes(&base, &changed);
        assert_eq!(changes.layout(), LayoutChangeMask::NONE);
        assert_eq!(changes.resources(), ResourceChangeMask::NONE);
        assert!(changes.semantic().contains(SemanticChangeMask::TANK));
        assert_eq!(changes.frame(), FrameChangeMask::NONE);
    }

    #[test]
    fn activity_status_requires_a_private_bounded_coherent_fade() {
        let base = snapshot();

        let mut non_finite = (*base).clone();
        non_finite.frame.activity_opacity = f32::NAN;
        assert_eq!(
            validate_snapshot(&non_finite),
            Err(SnapshotRejection::NonFinite)
        );

        let mut out_of_range = (*base).clone();
        out_of_range.frame.activity_recent = true;
        out_of_range.frame.activity_opacity = 1.01;
        assert_eq!(
            validate_snapshot(&out_of_range),
            Err(SnapshotRejection::InvalidValue)
        );

        let mut inconsistent = (*base).clone();
        inconsistent.frame.activity_opacity = 0.25;
        assert_eq!(
            validate_snapshot(&inconsistent),
            Err(SnapshotRejection::InconsistentIdentity)
        );

        let mut recent = (*base).clone();
        recent.frame.activity_recent = true;
        recent.frame.activity_opacity = 0.25;
        assert_eq!(validate_snapshot(&recent), Ok(()));

        recent.frame.activity_opacity = 0.0;
        assert_eq!(validate_snapshot(&recent), Ok(()));
    }

    #[test]
    fn daily_rollovers_allow_unbounded_private_excess_only_on_the_overage_lane() {
        let base = snapshot();
        let mut rollover = (*base).clone();
        rollover.frame.gauge_fractions[2] = 1.62;
        rollover.frame.gauge_levels[2] = GaugeLevelSnapshot::Full;
        assert_eq!(validate_snapshot(&rollover), Ok(()));

        let mut invalid_base_lane = rollover;
        invalid_base_lane.frame.gauge_fractions[1] = 1.01;
        invalid_base_lane.frame.gauge_levels[1] = GaugeLevelSnapshot::Full;
        assert_eq!(
            validate_snapshot(&invalid_base_lane),
            Err(SnapshotRejection::InvalidValue)
        );
    }

    #[test]
    fn empty_ambient_semantic_rejects_a_visible_frame() {
        let mut invalid = (*snapshot()).clone();
        invalid.frame.ambient_instances[0].visible = true;
        assert_eq!(
            validate_snapshot(&invalid),
            Err(SnapshotRejection::InconsistentIdentity)
        );
    }

    #[test]
    fn empty_ambient_semantic_rejects_a_positioned_frame() {
        let mut invalid = (*snapshot()).clone();
        invalid.frame.ambient_instances[0].position_points = [1.0, 0.0];
        assert_eq!(
            validate_snapshot(&invalid),
            Err(SnapshotRejection::InconsistentIdentity)
        );
    }

    #[test]
    fn empty_ambient_semantic_rejects_a_nonzero_opacity_frame() {
        let mut invalid = (*snapshot()).clone();
        invalid.frame.ambient_instances[0].opacity = 0.1;
        assert_eq!(
            validate_snapshot(&invalid),
            Err(SnapshotRejection::InconsistentIdentity)
        );
    }

    #[test]
    fn empty_ambient_semantic_rejects_negative_zero_frame_state() {
        let base = snapshot();
        let mut invalid = (*base).clone();
        invalid.frame.ambient_instances[0].position_points[0] = -0.0;
        assert_eq!(
            validate_snapshot(&invalid),
            Err(SnapshotRejection::InconsistentIdentity)
        );

        let mut invalid = (*base).clone();
        invalid.frame.ambient_instances[0].opacity = -0.0;
        assert_eq!(
            validate_snapshot(&invalid),
            Err(SnapshotRejection::InconsistentIdentity)
        );
    }

    #[test]
    fn asleep_snapshot_rejects_recent_activity_even_at_zero_opacity() {
        let mut invalid = (*snapshot()).clone();
        invalid.frame.asleep = true;
        invalid.frame.calm = true;
        invalid.frame.activity_recent = true;
        invalid.frame.activity_opacity = 0.0;
        let depth = invalid.frame.pet_depth;
        set_pet_depth(&mut invalid, depth);
        assert_eq!(
            validate_snapshot(&invalid),
            Err(SnapshotRejection::InconsistentIdentity)
        );
    }

    #[test]
    fn every_owned_counter_overflow_is_typed_and_non_mutating() {
        let mut layout_runtime = runtime();
        let initial = layout_runtime.active_version().unwrap();
        assert_eq!(initial.generation.device, DeviceEpoch(1));
        assert_eq!(initial.surface, SurfaceEpoch(1));
        assert_eq!(initial.generation.layout, LayoutGeneration(1));
        assert_eq!(initial.generation.resources, ResourceGeneration(1));
        assert_eq!(initial.applied, AppliedRevisions::new(1, 1));
        assert_eq!(layout_runtime.next_request_id, RequestId(1));
        assert_eq!(
            layout_runtime.next_activation_attempt_id,
            ActivationAttemptId(1)
        );
        let base = Arc::clone(layout_runtime.snapshot());
        layout_runtime.reconciler.layout_generation = LayoutGeneration(u64::MAX);
        let mut layout = (*base).clone();
        layout.topology.layout.width_points += 1.0;
        layout.topology.glyph_grid.cell_extent_points[0] =
            layout.topology.layout.width_points / f32::from(layout.topology.glyph_grid.columns);
        assert_eq!(
            layout_runtime
                .prepare_snapshot(Arc::new(layout))
                .unwrap_err(),
            RuntimeError::CounterOverflow(CounterKind::LayoutGeneration)
        );
        assert!(Arc::ptr_eq(layout_runtime.snapshot(), &base));

        let mut frame_runtime = runtime();
        frame_runtime.reconciler.frame_revision = FrameRevision(u64::MAX);
        let mut frame = (**frame_runtime.snapshot()).clone();
        offset_pet_depth(&mut frame, 0.1);
        assert_eq!(
            frame_runtime.prepare_snapshot(Arc::new(frame)).unwrap_err(),
            RuntimeError::CounterOverflow(CounterKind::FrameRevision)
        );

        let mut request_runtime = runtime();
        request_runtime.next_request_id = RequestId(u64::MAX);
        let topology = topology_update(request_runtime.snapshot(), Stage::S4);
        assert_eq!(
            request_runtime.prepare_snapshot(topology).unwrap_err(),
            RuntimeError::CounterOverflow(CounterKind::RequestId)
        );
        assert!(request_runtime.pending.is_none());

        let mut surface_runtime = runtime();
        surface_runtime.surface_epoch = SurfaceEpoch(u64::MAX);
        surface_runtime.recovery =
            RecoveryState::FallbackPending(RecoveryRequirement::SurfaceSuccessor {
                failed_device: surface_runtime.device_epoch,
                failed_surface: surface_runtime.surface_epoch,
            });
        assert_eq!(
            surface_runtime.acknowledge_surface_rebound(),
            Err(RuntimeError::CounterOverflow(CounterKind::SurfaceEpoch))
        );
        let mut device_runtime = runtime();
        device_runtime.device_epoch = DeviceEpoch(u64::MAX);
        device_runtime.recovery =
            RecoveryState::FallbackPending(RecoveryRequirement::DeviceSuccessor {
                failed_device: device_runtime.device_epoch,
            });
        assert_eq!(
            device_runtime.acknowledge_device_recreated(),
            Err(RuntimeError::CounterOverflow(CounterKind::DeviceEpoch))
        );

        let mut activation_runtime = runtime();
        let topology = topology_update(activation_runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut activation_runtime, topology));
        activation_runtime.complete_candidate(request.accept());
        activation_runtime.next_activation_attempt_id = ActivationAttemptId(u64::MAX);
        assert_eq!(
            activation_runtime.begin_activation(),
            Err(ActivationStartError::CounterOverflow(
                CounterKind::ActivationAttemptId
            ))
        );
    }

    #[test]
    fn prepared_token_is_invalidated_by_runtime_boundary_changes() {
        let mut runtime = runtime();
        let mut semantic = (**runtime.snapshot()).clone();
        semantic.content.mood = Mood::Content;
        let prepared = runtime.prepare_snapshot(Arc::new(semantic)).unwrap();
        runtime.set_hidden();
        assert_eq!(
            runtime.commit_prepared(prepared),
            Err(PreparedCommitError::StaleBase)
        );
    }

    #[test]
    fn fallback_and_recovery_gate_capture_until_exact_present() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        runtime.complete_candidate(request.accept());
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
        );
        assert!(matches!(
            runtime.recovery,
            RecoveryState::FallbackPending { .. }
        ));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::RecoveryInProgress
        );

        let request = take_start(runtime.acknowledge_surface_rebound().unwrap());
        assert!(matches!(runtime.recovery, RecoveryState::Recovering { .. }));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::RecoveryInProgress
        );
        runtime.complete_candidate(request.accept());
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(runtime.recovery, RecoveryState::Operational);
        assert!(runtime.capture_lease().is_ok());
    }

    #[test]
    fn recovery_requirement_allows_only_its_exact_acknowledgement() {
        let (mut surface, _, attempt) = runtime_with_activation();
        surface.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
        );
        assert!(matches!(
            surface.recovery,
            RecoveryState::FallbackPending(RecoveryRequirement::SurfaceSuccessor {
                failed_device: DeviceEpoch(1),
                failed_surface: SurfaceEpoch(1),
            })
        ));
        let before = (
            surface.device_epoch,
            surface.surface_epoch,
            surface.resource_generation,
            surface.next_request_id,
            surface.active_version(),
            surface.pending_request_identity(),
            surface.worker,
            surface.recovery,
        );
        assert_eq!(
            surface.acknowledge_device_recreated(),
            Err(RuntimeError::RecoveryActionRejected)
        );
        assert_eq!(
            (
                surface.device_epoch,
                surface.surface_epoch,
                surface.resource_generation,
                surface.next_request_id,
                surface.active_version(),
                surface.pending_request_identity(),
                surface.worker,
                surface.recovery,
            ),
            before
        );

        let mut surface_effects = surface.acknowledge_surface_rebound().unwrap();
        let surface_request = surface_effects.take_start_worker().unwrap();
        assert_eq!(surface_request.key().device, DeviceEpoch(1));
        assert_eq!(surface_request.surface(), SurfaceEpoch(2));

        let (mut device, _, attempt) = runtime_with_activation();
        device.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::DeviceLost),
        );
        assert!(matches!(
            device.recovery,
            RecoveryState::FallbackPending(RecoveryRequirement::DeviceSuccessor {
                failed_device: DeviceEpoch(1),
            })
        ));
        let before = (
            device.device_epoch,
            device.surface_epoch,
            device.resource_generation,
            device.next_request_id,
            device.active_version(),
            device.pending_request_identity(),
            device.worker,
            device.recovery,
        );
        assert_eq!(
            device.acknowledge_surface_rebound(),
            Err(RuntimeError::RecoveryActionRejected)
        );
        assert_eq!(
            (
                device.device_epoch,
                device.surface_epoch,
                device.resource_generation,
                device.next_request_id,
                device.active_version(),
                device.pending_request_identity(),
                device.worker,
                device.recovery,
            ),
            before
        );
        let mut device_effects = device.acknowledge_device_recreated().unwrap();
        let device_request = device_effects.take_start_worker().unwrap();
        assert!(device_request.key().device.0 > DeviceEpoch(1).0);
    }

    fn awaiting_recovery_retry(
        failure: EpochFailure,
    ) -> (CompanionSceneRuntimeState, RequestIdentity) {
        let (mut runtime, _, attempt) = runtime_with_activation();
        runtime.finish_activation(attempt, ActivationAttemptOutcome::Fatal(failure));
        let recovery = if matches!(
            failure,
            EpochFailure::SurfaceLost | EpochFailure::SurfaceValidation
        ) {
            take_start(runtime.acknowledge_surface_rebound().unwrap())
        } else {
            take_start(runtime.acknowledge_device_recreated().unwrap())
        };
        let identity = recovery.identity();
        runtime.complete_candidate(recovery.accept());
        let attempt = runtime.begin_activation().unwrap();
        let mut rejection = runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Resource),
        );
        assert!(rejection.take_start_worker().is_none());
        assert!(rejection.take_drop_candidate().is_some());
        assert!(runtime.pending.is_none());
        assert_eq!(runtime.worker, WorkerState::Idle);
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::SurfaceUnavailable)
        );
        (runtime, identity)
    }

    #[test]
    fn rejected_surface_recovery_retries_on_same_verified_successor() {
        let (mut runtime, rejected) = awaiting_recovery_retry(EpochFailure::SurfaceLost);
        assert!(matches!(
            runtime.recovery,
            RecoveryState::AwaitingRetry {
                requirement: RecoveryRequirement::SurfaceSuccessor { .. },
                device: DeviceEpoch(1),
                surface: SurfaceEpoch(2),
            }
        ));
        let resource = runtime.resource_generation;
        let next_request = runtime.next_request_id;
        let mut effects = runtime.retry_recovery().unwrap();
        let retry = effects.take_start_worker().unwrap();
        assert_ne!(retry.request_id(), rejected.request_id());
        assert_eq!(retry.request_id(), next_request);
        assert_eq!(retry.key().device, rejected.key().device);
        assert_eq!(retry.surface(), rejected.surface());
        assert_eq!(retry.key().resources, ResourceGeneration(resource.0 + 1));
        runtime.complete_candidate(retry.accept());
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(runtime.recovery, RecoveryState::Operational);
    }

    #[test]
    fn rejected_device_recovery_retries_without_advancing_device_again() {
        let (mut runtime, rejected) = awaiting_recovery_retry(EpochFailure::DeviceLost);
        assert!(matches!(
            runtime.recovery,
            RecoveryState::AwaitingRetry {
                requirement: RecoveryRequirement::DeviceSuccessor { .. },
                device: DeviceEpoch(2),
                surface: SurfaceEpoch(1),
            }
        ));
        let device = runtime.device_epoch;
        let surface = runtime.surface_epoch;
        let resource = runtime.resource_generation;
        let next_request = runtime.next_request_id;
        let retry = take_start(runtime.retry_recovery().unwrap());
        assert_eq!(runtime.device_epoch, device);
        assert_eq!(runtime.surface_epoch, surface);
        assert_eq!(retry.request_id(), next_request);
        assert_ne!(retry.request_id(), rejected.request_id());
        assert_eq!(retry.key().device, device);
        assert_eq!(retry.surface(), surface);
        assert_eq!(retry.key().resources, ResourceGeneration(resource.0 + 1));
        runtime.complete_candidate(retry.accept());
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(runtime.recovery, RecoveryState::Operational);
    }

    #[test]
    fn stale_or_shutdown_recovery_retry_rejects_without_work() {
        let (mut stale, _) = awaiting_recovery_retry(EpochFailure::SurfaceLost);
        stale.surface_epoch = SurfaceEpoch(stale.surface_epoch.0 + 1);
        let before = (
            stale.resource_generation,
            stale.next_request_id,
            stale.pending_request_identity(),
            stale.worker,
            stale.recovery,
        );
        assert_eq!(
            stale.retry_recovery(),
            Err(RuntimeError::RecoveryActionRejected)
        );
        assert_eq!(
            (
                stale.resource_generation,
                stale.next_request_id,
                stale.pending_request_identity(),
                stale.worker,
                stale.recovery,
            ),
            before
        );

        let (mut shutdown, _) = awaiting_recovery_retry(EpochFailure::DeviceLost);
        shutdown.shutdown();
        assert_eq!(shutdown.retry_recovery(), Err(RuntimeError::Shutdown));
    }

    #[test]
    fn surface_fatal_from_old_device_is_stale_even_at_current_surface() {
        for failure in [EpochFailure::SurfaceLost, EpochFailure::SurfaceValidation] {
            let (mut runtime, old_request, attempt) = runtime_with_activation();
            runtime.recovery =
                RecoveryState::FallbackPending(RecoveryRequirement::DeviceSuccessor {
                    failed_device: runtime.device_epoch,
                });
            runtime.acknowledge_device_recreated().unwrap();
            let replacement = runtime.pending_request_identity().unwrap().request_id();
            assert_eq!(attempt.surface, runtime.surface_epoch);
            assert_ne!(attempt.key.device, runtime.device_epoch);

            let mut late =
                runtime.finish_activation(attempt, ActivationAttemptOutcome::Fatal(failure));
            assert_eq!(
                late.take_drop_candidate().map(|action| action.request_id()),
                Some(old_request.request_id())
            );
            assert_eq!(
                late.take_start_worker().map(|request| request.request_id()),
                Some(replacement)
            );
            assert!(matches!(
                runtime.recovery,
                RecoveryState::Recovering {
                    requirement: RecoveryRequirement::DeviceSuccessor { .. },
                    ..
                }
            ));
        }
    }

    #[test]
    fn superseding_activation_drops_old_candidate_and_starts_exact_new_request() {
        let mut runtime = runtime();
        let first_snapshot = topology_update(runtime.snapshot(), Stage::S4);
        let first = take_start(commit_snapshot(&mut runtime, first_snapshot));
        let first_id = first.request_id();
        runtime.complete_candidate(first.accept());
        let attempt = runtime.begin_activation().unwrap();

        let second_snapshot = topology_update(runtime.snapshot(), Stage::S5);
        let second_effects = commit_snapshot(&mut runtime, second_snapshot);
        let second_id = runtime.pending_request_identity().unwrap().request_id();
        assert!(second_effects.start_worker.is_none());
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::ActivationInProgress
        );

        let mut completion = runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(
            completion
                .take_drop_candidate()
                .map(|action| action.request_id()),
            Some(first_id)
        );
        assert_eq!(
            completion
                .take_start_worker()
                .map(|request| request.request_id()),
            Some(second_id)
        );
        assert_eq!(runtime.worker, WorkerState::Running(second_id));
    }

    fn runtime_with_activation() -> (
        CompanionSceneRuntimeState,
        RequestIdentity,
        ActivationAttempt,
    ) {
        let mut runtime = runtime();
        let topology = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, topology));
        let identity = request.identity();
        runtime.complete_candidate(request.accept());
        let attempt = runtime.begin_activation().unwrap();
        (runtime, identity, attempt)
    }

    #[test]
    fn superseding_activation_late_rejection_and_current_fatal_remain_actionable() {
        let (mut rejected, first, attempt) = runtime_with_activation();
        let next = topology_update(rejected.snapshot(), Stage::S5);
        commit_snapshot(&mut rejected, next);
        let replacement = rejected.pending_request_identity().unwrap().request_id();
        let mut effects = rejected.finish_activation(
            attempt,
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Resource),
        );
        assert_eq!(
            effects
                .take_drop_candidate()
                .map(|action| action.request_id()),
            Some(first.request_id())
        );
        assert_eq!(
            effects
                .take_start_worker()
                .map(|request| request.request_id()),
            Some(replacement)
        );
        assert!(rejected.active_version().is_some());

        let (mut fatal, first, attempt) = runtime_with_activation();
        let next = topology_update(fatal.snapshot(), Stage::S5);
        commit_snapshot(&mut fatal, next);
        let mut effects = fatal.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::DeviceLost),
        );
        assert_eq!(
            effects
                .take_drop_candidate()
                .map(|action| action.request_id()),
            Some(first.request_id())
        );
        assert!(effects.start_worker.is_none());
        assert!(fatal.active_version().is_none());
        assert!(matches!(
            fatal.recovery,
            RecoveryState::FallbackPending { .. }
        ));
    }

    #[test]
    fn retired_surface_and_device_failures_cannot_poison_recovery() {
        let (mut surface, old_surface, attempt) = runtime_with_activation();
        surface.recovery = RecoveryState::FallbackPending(RecoveryRequirement::SurfaceSuccessor {
            failed_device: surface.device_epoch,
            failed_surface: surface.surface_epoch,
        });
        let recovery = surface.acknowledge_surface_rebound().unwrap();
        let replacement = surface.pending_request_identity().unwrap().request_id();
        assert!(recovery.start_worker.is_none());
        let mut late = surface.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
        );
        assert_eq!(
            late.take_drop_candidate().map(|action| action.request_id()),
            Some(old_surface.request_id())
        );
        assert_eq!(
            late.take_start_worker().map(|request| request.request_id()),
            Some(replacement)
        );
        assert!(matches!(surface.recovery, RecoveryState::Recovering { .. }));
        assert!(surface.active_version().is_some());

        let (mut device, old_device, attempt) = runtime_with_activation();
        device.recovery = RecoveryState::FallbackPending(RecoveryRequirement::DeviceSuccessor {
            failed_device: device.device_epoch,
        });
        device.acknowledge_device_recreated().unwrap();
        let replacement = device.pending_request_identity().unwrap().request_id();
        let mut late = device.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::DeviceLost),
        );
        assert_eq!(
            late.take_drop_candidate().map(|action| action.request_id()),
            Some(old_device.request_id())
        );
        assert_eq!(
            late.take_start_worker().map(|request| request.request_id()),
            Some(replacement)
        );
        assert!(matches!(device.recovery, RecoveryState::Recovering { .. }));
    }

    #[test]
    fn activation_guards_retain_active_until_exact_clean_present() {
        for deferral in [
            AcquireDeferral::OutdatedReconfigured,
            AcquireDeferral::Timeout,
            AcquireDeferral::Occluded,
        ] {
            let (mut runtime, _, attempt) = runtime_with_activation();
            let previous = runtime.active_version();
            let effects =
                runtime.finish_activation(attempt, ActivationAttemptOutcome::Deferred(deferral));
            assert_eq!(
                effects.disposition(),
                RuntimeDisposition::Activation(ActivationTransition::RetryLater)
            );
            assert_eq!(runtime.active_version(), previous);
        }
        for failure in [
            CandidateFailure::Validation,
            CandidateFailure::Resource,
            CandidateFailure::PreSubmitEncode,
        ] {
            let (mut runtime, _, attempt) = runtime_with_activation();
            let previous = runtime.active_version();
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::CandidateRejected(failure),
            );
            assert_eq!(runtime.active_version(), previous);
        }

        let (mut runtime, request, attempt) = runtime_with_activation();
        let previous = runtime.active_version();
        let wrong = ActivationAttempt {
            attempt_id: ActivationAttemptId(attempt.attempt_id.0 + 1),
            ..attempt
        };
        runtime.finish_activation(
            wrong,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(runtime.active_version(), previous);
        let effects = runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean {
                surface: SurfaceEpoch(attempt.surface.0 + 1),
            },
        );
        assert_eq!(
            effects.disposition(),
            RuntimeDisposition::Activation(ActivationTransition::DroppedStale)
        );
        assert_ne!(runtime.active_version().unwrap().generation, request.key());
    }

    #[test]
    fn every_epoch_failure_enters_typed_fallback() {
        for failure in [
            EpochFailure::SurfaceLost,
            EpochFailure::SurfaceValidation,
            EpochFailure::DeviceLost,
            EpochFailure::Internal,
            EpochFailure::OutOfMemory,
            EpochFailure::UncertainPostSubmit,
            EpochFailure::ImmediateGpuError,
            EpochFailure::DelayedGpuError,
        ] {
            let (mut runtime, _, attempt) = runtime_with_activation();
            let effects =
                runtime.finish_activation(attempt, ActivationAttemptOutcome::Fatal(failure));
            assert_eq!(
                effects.disposition(),
                RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending)
            );
            assert!(matches!(
                runtime.recovery,
                RecoveryState::FallbackPending { .. }
            ));
            assert_eq!(
                runtime.capture_lease().unwrap_err(),
                CaptureDefer::RecoveryInProgress
            );
        }
    }

    #[test]
    fn device_recreation_and_resource_work_stay_queued_while_hidden() {
        let mut runtime = runtime();
        runtime.observe_delayed_gpu_error(runtime.device_epoch);
        runtime.set_hidden();
        let device = runtime.acknowledge_device_recreated().unwrap();
        assert!(device.start_worker.is_none());
        assert_eq!(runtime.device_epoch, DeviceEpoch(2));
        assert!(matches!(runtime.recovery, RecoveryState::Recovering { .. }));

        let mut resource = runtime
            .invalidate_resources(ResourceInvalidation::BackingScaleAtlas)
            .unwrap();
        assert!(resource.start_worker.is_none());
        assert_eq!(resource.take_cancel_worker(), None);
        let expected = runtime.pending_request_identity().unwrap().request_id();
        assert!(matches!(
            runtime.recovery,
            RecoveryState::Recovering { request, .. } if request == expected
        ));
        let mut reveal = reveal(&mut runtime);
        assert_eq!(
            reveal
                .take_start_worker()
                .map(|request| request.request_id()),
            Some(expected)
        );
    }

    #[test]
    fn recovery_authority_follows_newest_request_in_both_cancellation_orders() {
        for acknowledge_first in [true, false] {
            let mut runtime = runtime();
            let topology = topology_update(runtime.snapshot(), Stage::S4);
            let initial = take_start(commit_snapshot(&mut runtime, topology));
            runtime.complete_candidate(initial.accept());
            let attempt = runtime.begin_activation().unwrap();
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
            );
            let recovery = take_start(runtime.acknowledge_surface_rebound().unwrap());
            let newest_snapshot = topology_update(runtime.snapshot(), Stage::S5);
            let mut superseding = commit_snapshot(&mut runtime, newest_snapshot);
            assert_eq!(
                superseding
                    .take_cancel_worker()
                    .map(|action| action.request_id()),
                Some(recovery.request_id())
            );
            let newest = runtime.pending_request_identity().unwrap().request_id();
            assert!(matches!(
                runtime.recovery,
                RecoveryState::Recovering { request, .. } if request == newest
            ));

            let mut start = if acknowledge_first {
                runtime.acknowledge_worker_cancelled(recovery.request_id())
            } else {
                runtime.complete_candidate(recovery.accept())
            };
            let newest_request = start.take_start_worker().unwrap();
            assert_eq!(newest_request.request_id(), newest);
            runtime.complete_candidate(newest_request.accept());
            let attempt = runtime.begin_activation().unwrap();
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
            );
            assert_eq!(runtime.recovery, RecoveryState::Operational);
        }
    }

    #[test]
    fn hidden_snapshot_storm_commits_only_latest_and_emits_one_reveal_start() {
        let mut runtime = runtime();
        runtime.set_hidden();
        let mut latest = None;
        for step in 0..10 {
            let mut next = (**runtime.snapshot()).clone();
            set_pet_depth(&mut next, 0.5 + step as f32 * 0.01);
            if step == 9 {
                next.topology.pet.stage = Stage::S4;
            }
            let next = Arc::new(next);
            latest = Some(Arc::clone(&next));
            runtime.coalesce_hidden_snapshot(next).unwrap();
            assert!(runtime.pending.is_none());
        }
        let prepared = runtime.prepare_reveal().unwrap();
        assert!(Arc::ptr_eq(
            prepared.update().snapshot(),
            latest.as_ref().unwrap()
        ));
        let mut effects = runtime.commit_reveal(prepared).unwrap();
        assert!(Arc::ptr_eq(runtime.snapshot(), latest.as_ref().unwrap()));
        assert!(runtime.hidden_latest.is_none());
        assert!(effects.take_start_worker().is_some());
        assert!(effects.take_cancel_worker().is_none());
    }

    #[test]
    fn dropped_reveal_projection_retains_hidden_snapshot_and_publishes_nothing() {
        let mut runtime = runtime();
        runtime.set_hidden();
        let before = runtime.active_version();
        let before_snapshot = Arc::clone(runtime.snapshot());
        let mut hidden = (*before_snapshot).clone();
        offset_pet_depth(&mut hidden, 0.1);
        let hidden = Arc::new(hidden);
        runtime
            .coalesce_hidden_snapshot(Arc::clone(&hidden))
            .unwrap();

        let prepared = runtime.prepare_reveal().unwrap();
        assert!(Arc::ptr_eq(prepared.update().snapshot(), &hidden));
        drop(prepared); // Task 5 projection failed.
        assert!(Arc::ptr_eq(runtime.snapshot(), &before_snapshot));
        assert_eq!(runtime.active_version(), before);
        assert!(runtime.pending.is_none());
        assert!(runtime.hidden_latest.is_some());
        assert_eq!(runtime.visibility, RuntimeVisibility::Hidden);
    }

    #[test]
    fn reveal_token_binds_absence_of_hidden_snapshot() {
        let mut runtime = runtime();
        runtime.set_hidden();
        let prepared = runtime.prepare_reveal().unwrap();
        let mut late = (**runtime.snapshot()).clone();
        offset_pet_depth(&mut late, 0.1);
        let late = Arc::new(late);
        runtime.coalesce_hidden_snapshot(Arc::clone(&late)).unwrap();

        assert_eq!(
            runtime.commit_reveal(prepared),
            Err(PreparedCommitError::StaleBase)
        );
        assert_eq!(runtime.visibility, RuntimeVisibility::Hidden);
        assert!(runtime
            .hidden_latest
            .as_ref()
            .is_some_and(|hidden| Arc::ptr_eq(hidden, &late)));
    }

    #[test]
    fn hiding_ready_or_in_flight_activation_never_commits_while_hidden() {
        let mut ready = runtime();
        let topology = topology_update(ready.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut ready, topology));
        ready.complete_candidate(request.accept());
        ready.set_hidden();
        assert_eq!(ready.begin_activation(), Err(ActivationStartError::Hidden));

        let (mut activating, _, attempt) = runtime_with_activation();
        let previous = activating.active_version();
        activating.set_hidden();
        let effects = activating.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(
            effects.disposition(),
            RuntimeDisposition::Activation(ActivationTransition::DroppedStale)
        );
        assert_eq!(activating.active_version(), previous);
    }

    #[test]
    fn capture_lease_binds_exact_active_and_defers_during_activation() {
        let mut runtime = runtime();
        let leased = runtime.capture_lease().unwrap().version();
        let topology = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, topology));
        runtime.complete_candidate(request.accept());
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::ActivationInProgress
        );
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_ne!(runtime.capture_lease().unwrap().version(), leased);
    }

    #[test]
    fn capture_lease_derives_neutral_identity_and_locked_logical_state() {
        let runtime = runtime();
        let lease = runtime.capture_lease().unwrap();
        assert_eq!(
            lease.source_identity(),
            super::super::contract::CaptureSourceIdentity::new(
                lease.template().generation_checksum,
                lease.content_checksum(),
                lease
                    .frame()
                    .capture_source_checksum(lease.template())
                    .unwrap(),
            )
        );
        assert_eq!(
            lease.logical_state_alias(),
            super::super::contract::CompanionCaptureStateAlias::Normal
        );
    }

    #[test]
    fn capture_identity_quantizes_private_gauge_and_dim_detail() {
        fn identity_for(
            gauge_fraction: f32,
            dim_amount: f32,
        ) -> (
            super::super::contract::CaptureSourceIdentity,
            String,
            String,
            u64,
        ) {
            let mut source = (*snapshot()).clone();
            source.frame.gauge_fractions = [gauge_fraction; 4];
            source.frame.gauge_levels =
                [GaugeLevelSnapshot::from_fraction(f64::from(gauge_fraction)); 4];
            source.frame.dimmed = dim_amount > 0.0;
            source.frame.dim_amount = dim_amount;
            let runtime = CompanionSceneRuntimeState::with_active(Arc::new(source)).unwrap();
            let lease = runtime.capture_lease().unwrap();
            let identity = lease.source_identity();
            let artifacts = super::super::contract::SceneArtifacts::try_from_parts(
                lease.template(),
                lease.content(),
                lease.frame(),
            )
            .unwrap();
            (
                identity,
                serde_json::to_string(&identity).unwrap(),
                serde_json::to_string(&artifacts).unwrap(),
                lease.frame_checksum(),
            )
        }

        let (first, first_json, first_artifact_json, first_internal_checksum) =
            identity_for(0.31, 0.1);
        let (
            same_public_state,
            same_public_json,
            same_public_artifact_json,
            second_internal_checksum,
        ) = identity_for(0.49, 0.9);
        assert_ne!(
            first_internal_checksum, second_internal_checksum,
            "retained rendering must keep exact frame identity"
        );
        assert_eq!(first, same_public_state);
        assert_eq!(first_json, same_public_json);
        assert_eq!(first_artifact_json, same_public_artifact_json);

        let (different_gauge_band, _, different_gauge_artifact_json, _) = identity_for(0.51, 0.9);
        assert_ne!(first, different_gauge_band);
        assert_ne!(first_artifact_json, different_gauge_artifact_json);

        let (not_dimmed, _, not_dimmed_artifact_json, _) = identity_for(0.31, 0.0);
        assert_ne!(first, not_dimmed);
        assert_ne!(first_artifact_json, not_dimmed_artifact_json);
    }

    #[test]
    fn capture_identity_quantizes_private_activity_fade_but_preserves_live_frame() {
        fn capture_for_status(
            recent: bool,
            opacity: f32,
        ) -> (
            super::super::contract::CaptureSourceIdentity,
            u64,
            String,
            super::super::contract::CompanionCaptureStateAlias,
            f32,
        ) {
            let mut source = (*snapshot()).clone();
            source.frame.activity_recent = recent;
            source.frame.activity_opacity = opacity;
            let runtime = CompanionSceneRuntimeState::with_active(Arc::new(source)).unwrap();
            let lease = runtime.capture_lease().unwrap();
            let status_id = lease
                .template()
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "chrome.status")
                .expect("status node")
                .id;
            let live_opacity = lease
                .frame()
                .nodes
                .iter()
                .find(|node| node.node == status_id)
                .expect("status frame")
                .opacity;
            let artifacts = super::super::contract::SceneArtifacts::try_from_parts(
                lease.template(),
                lease.content(),
                lease.frame(),
            )
            .unwrap();
            (
                lease.source_identity(),
                lease.frame_checksum(),
                serde_json::to_string(&artifacts).unwrap(),
                lease.logical_state_alias(),
                live_opacity,
            )
        }

        let first = capture_for_status(true, 0.2);
        let second = capture_for_status(true, 0.8);
        assert_ne!(
            first.1, second.1,
            "live frame identity must preserve exact fade"
        );
        assert_eq!((first.4, second.4), (0.2, 0.8));
        assert_eq!(
            first.0, second.0,
            "capture checksum must quantize exact fade"
        );
        assert_eq!(first.2, second.2, "artifact JSON must quantize exact fade");
        assert_eq!(first.3, second.3);
        assert_eq!(
            first.3,
            super::super::contract::CompanionCaptureStateAlias::Active
        );

        let quiet = capture_for_status(false, 0.0);
        assert_ne!(first.0, quiet.0);
        assert_ne!(first.2, quiet.2);
        assert_ne!(first.3, quiet.3);
        assert_eq!(
            quiet.3,
            super::super::contract::CompanionCaptureStateAlias::Normal
        );
    }

    #[test]
    fn serialized_snapshot_exposes_only_boolean_dim_state() {
        fn json_for(dim_amount: f32) -> String {
            let mut source = (*snapshot()).clone();
            source.frame.dimmed = dim_amount > 0.0;
            source.frame.dim_amount = dim_amount;
            assert_eq!(validate_snapshot(&source), Ok(()));
            serde_json::to_string(&source).unwrap()
        }

        let first_positive = json_for(0.123_456_79);
        let second_positive = json_for(0.987_654_3);
        assert_eq!(first_positive, second_positive);
        assert!(!first_positive.contains("0.12345679"));
        assert!(!second_positive.contains("0.9876543"));

        let not_dimmed = json_for(0.0);
        assert_ne!(first_positive, not_dimmed);
    }

    #[test]
    fn capture_lease_state_matrix_never_mixes_active_and_candidate_versions() {
        let mut runtime = runtime();
        let active = runtime.capture_lease().unwrap().version();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        assert!(matches!(
            runtime.pending.as_ref().unwrap().phase,
            PendingPhase::Preparing
        ));
        assert_eq!(runtime.capture_lease().unwrap().version(), active);

        runtime.complete_candidate(request.accept());
        assert!(matches!(
            runtime.pending.as_ref().unwrap().phase,
            PendingPhase::Ready(_)
        ));
        assert_eq!(runtime.capture_lease().unwrap().version(), active);

        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::ActivationInProgress
        );
        let superseding = topology_update(runtime.snapshot(), Stage::S5);
        commit_snapshot(&mut runtime, superseding);
        assert!(matches!(
            runtime.pending.as_ref().unwrap().phase,
            PendingPhase::SupersedingActivation { .. }
        ));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::ActivationInProgress
        );

        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
        );
        assert!(matches!(
            runtime.recovery,
            RecoveryState::FallbackPending(_)
        ));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::RecoveryInProgress
        );

        let request = take_start(runtime.acknowledge_surface_rebound().unwrap());
        assert!(matches!(runtime.recovery, RecoveryState::Recovering { .. }));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::RecoveryInProgress
        );
        runtime.complete_candidate(request.accept());
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Validation),
        );
        assert!(matches!(
            runtime.recovery,
            RecoveryState::AwaitingRetry { .. }
        ));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::RecoveryInProgress
        );

        let mut no_active = CompanionSceneRuntimeState::with_active(snapshot()).unwrap();
        no_active.active = None;
        assert_eq!(
            no_active.capture_lease().unwrap_err(),
            CaptureDefer::NoActiveGeneration
        );
        no_active.shutdown();
        assert_eq!(
            no_active.capture_lease().unwrap_err(),
            CaptureDefer::Shutdown
        );
    }

    #[test]
    fn operational_rebind_releases_only_new_surface_version_to_capture() {
        let mut runtime = runtime();
        let before = runtime.capture_lease().unwrap().version();
        runtime.acknowledge_operational_surface_rebound().unwrap();
        let after = runtime.capture_lease().unwrap().version();
        assert_eq!(after.surface, SurfaceEpoch(before.surface.0 + 1));
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.applied, before.applied);
    }

    #[test]
    fn delayed_errors_are_device_scoped_and_cannot_resurrect_state() {
        let mut preparing = runtime();
        let topology = topology_update(preparing.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut preparing, topology));
        let identity = request.identity();
        let mut effects = preparing.observe_delayed_gpu_error(identity.key().device);
        assert_eq!(
            effects
                .take_cancel_worker()
                .map(|action| action.request_id()),
            Some(identity.request_id())
        );
        assert!(preparing.active_version().is_none());
        let mut completion = preparing.complete_candidate(request.accept());
        assert_eq!(
            completion
                .take_drop_candidate()
                .map(|action| action.request_id()),
            Some(identity.request_id())
        );
        assert_eq!(preparing.worker, WorkerState::Idle);

        let (mut activating, _, attempt) = runtime_with_activation();
        activating.observe_delayed_gpu_error(attempt.key.device);
        activating.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert!(activating.active_version().is_none());
        assert!(matches!(
            activating.recovery,
            RecoveryState::FallbackPending { .. }
        ));

        let mut retired = runtime();
        let old_device = retired.device_epoch;
        retired.observe_delayed_gpu_error(old_device);
        let replacement = take_start(retired.acknowledge_device_recreated().unwrap());
        retired.complete_candidate(replacement.accept());
        let attempt = retired.begin_activation().unwrap();
        retired.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        let current = retired.active_version();
        let stale = retired.observe_delayed_gpu_error(old_device);
        assert_eq!(stale.disposition(), RuntimeDisposition::DroppedStale);
        assert_eq!(retired.active_version(), current);
    }

    #[test]
    fn hidden_reveal_and_shutdown_are_fail_closed() {
        let mut runtime = runtime();
        runtime.set_hidden();
        let queued = runtime
            .invalidate_resources(ResourceInvalidation::BackingScaleAtlas)
            .unwrap();
        assert!(queued.start_worker.is_none());
        assert!(matches!(
            queued.disposition(),
            RuntimeDisposition::GenerationQueued(_)
        ));
        let mut reveal_effects = reveal(&mut runtime);
        assert!(reveal_effects.take_start_worker().is_some());

        runtime.set_hidden();
        let mut hidden = (**runtime.snapshot()).clone();
        hidden.topology.pet.stage = Stage::S5;
        runtime.coalesce_hidden_snapshot(Arc::new(hidden)).unwrap();
        runtime.resource_generation = ResourceGeneration(u64::MAX);
        assert!(matches!(
            runtime.prepare_reveal(),
            Err(RuntimeError::CounterOverflow(
                CounterKind::ResourceGeneration
            ))
        ));
        assert!(runtime.hidden_latest.is_some());
        assert_eq!(runtime.visibility, RuntimeVisibility::Hidden);

        let shutdown = runtime.shutdown();
        assert!(matches!(
            shutdown.disposition(),
            RuntimeDisposition::Shutdown
        ));
        assert_eq!(runtime.lifecycle, RuntimeLifecycle::Shutdown);
        assert_eq!(runtime.capture_lease().unwrap_err(), CaptureDefer::Shutdown);
        assert_eq!(
            runtime.invalidate_resources(ResourceInvalidation::BackingScaleAtlas),
            Err(RuntimeError::Shutdown)
        );
        assert_eq!(
            runtime.acknowledge_surface_rebound(),
            Err(RuntimeError::Shutdown)
        );
        assert_eq!(
            runtime.acknowledge_device_recreated(),
            Err(RuntimeError::Shutdown)
        );
        assert!(matches!(
            runtime.prepare_reveal(),
            Err(RuntimeError::Shutdown)
        ));
        let late = runtime.acknowledge_worker_cancelled(RequestId(1));
        assert!(late.start_worker.is_none());
    }
}
