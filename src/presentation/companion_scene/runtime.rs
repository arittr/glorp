use super::validate::AcceptedSceneState;
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
    pub(crate) const HUD: Self = Self(1 << 5);
    pub(crate) const MOOD_WEATHER: Self = Self(1 << 6);

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

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

fn prop_topology_changed(
    previous: &super::PropTopologySnapshot,
    newest: &super::PropTopologySnapshot,
) -> bool {
    previous.catalog_id != newest.catalog_id
        || previous.stable_order != newest.stable_order
        || previous.zone != newest.zone
        || previous.authored_depth != newest.authored_depth
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
    if previous.sprite_variant != newest.sprite_variant
        || previous.anemone_morph != newest.anemone_morph
        || previous.cells.len() != newest.cells.len()
        || previous
            .cells
            .iter()
            .zip(&newest.cells)
            .any(|(left, right)| left.glyph != right.glyph)
    {
        changes.semantic.insert(SemanticChangeMask::TANK);
    }
    if previous.visible != newest.visible
        || previous.origin_col != newest.origin_col
        || previous.origin_row != newest.origin_row
        || previous.side != newest.side
        || previous.layer != newest.layer
        || previous.visible_rows != newest.visible_rows
        || previous.bounds != newest.bounds
        || previous.cells.len() != newest.cells.len()
        || previous
            .cells
            .iter()
            .zip(&newest.cells)
            .any(|(left, right)| {
                left.col != right.col || left.row != right.row || left.layer != right.layer
            })
    {
        changes.frame.insert(FrameChangeMask::TANK_INSTANCES);
    }
    // cadence_ms and calm are producer inputs already reflected in the resolved
    // cells/placement. They are not independent render state.
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
        || previous.content.pet_roles != newest.content.pet_roles
    {
        changes.semantic.insert(SemanticChangeMask::PET_ART);
    }
    if previous.content.room_weather != newest.content.room_weather {
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
            || left.chest_lid_open != right.chest_lid_open
        {
            changes.semantic.insert(SemanticChangeMask::PROP);
        }
        if left.motion_phase != right.motion_phase {
            changes.frame.insert(FrameChangeMask::PROP_TRANSFORMS);
        }
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

    if previous.content.activity_pulse_age_ms.is_some()
        != newest.content.activity_pulse_age_ms.is_some()
    {
        changes.semantic.insert(SemanticChangeMask::AMBIENT);
        changes.frame.insert(FrameChangeMask::AMBIENT_INSTANCES);
    } else if previous.content.activity_pulse_age_ms != newest.content.activity_pulse_age_ms {
        changes.frame.insert(FrameChangeMask::AMBIENT_INSTANCES);
    }

    if previous.frame.pet_anchor_points != newest.frame.pet_anchor_points
        || previous.frame.pet_depth != newest.frame.pet_depth
        || previous.frame.facing != newest.frame.facing
        || previous.frame.breath_offset_y_cells != newest.frame.breath_offset_y_cells
        || previous.frame.bob_offset_y_cells != newest.frame.bob_offset_y_cells
    {
        changes.frame.insert(FrameChangeMask::PET_TRANSFORM);
    }
    if previous.frame.asleep != newest.frame.asleep
        || previous.frame.helper_trouble != newest.frame.helper_trouble
    {
        changes.frame.insert(FrameChangeMask::STATUS_VISIBILITY);
    }
    if previous.frame.gauges != newest.frame.gauges {
        changes.frame.insert(FrameChangeMask::GAUGES);
    }
    if previous.frame.dim_amount != newest.frame.dim_amount {
        changes.frame.insert(FrameChangeMask::DIM);
    }
    if previous.frame.hud_lines != newest.frame.hud_lines {
        changes.semantic.insert(SemanticChangeMask::HUD);
    }
    // elapsed_ms is an input clock. Only derived fields above allocate a
    // revision, so a cadence tick with identical output is unchanged.

    changes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotRejection {
    SchemaVersion,
    RendererSchemaVersion,
    Privacy,
    NonFinite,
    InvalidValue,
    InconsistentIdentity,
    FixedCapacity,
}

fn validate_snapshot(snapshot: &CompanionSceneSnapshot) -> Result<(), SnapshotRejection> {
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
    if !layout.width_points.is_finite()
        || !layout.height_points.is_finite()
        || !snapshot
            .frame
            .pet_anchor_points
            .iter()
            .all(|value| value.is_finite())
        || !snapshot.frame.pet_depth.is_finite()
        || !snapshot.frame.bob_offset_y_cells.is_finite()
        || !snapshot.frame.dim_amount.is_finite()
    {
        return Err(SnapshotRejection::NonFinite);
    }
    if layout.width_points <= 0.0
        || layout.height_points <= 0.0
        || !matches!(snapshot.frame.facing, -1 | 1)
        || !(0.0..=1.0).contains(&snapshot.frame.dim_amount)
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
        if topology.catalog_id != content.catalog_id
            || topology.stable_order != content.stable_order
            || topology.route != content.route
            || usize::from(topology.stable_order) != index
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
    CounterOverflow(CounterKind),
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReconcileDecision {
    Unchanged {
        snapshot: Arc<CompanionSceneSnapshot>,
    },
    Changed {
        snapshot: Arc<CompanionSceneSnapshot>,
        changes: SnapshotChangeSet,
        layout: LayoutGeneration,
        applied: AppliedRevisions,
    },
}

impl ReconcileDecision {
    pub(crate) fn changes(&self) -> SnapshotChangeSet {
        match self {
            Self::Unchanged { .. } => SnapshotChangeSet::NONE,
            Self::Changed { changes, .. } => *changes,
        }
    }
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

    pub(crate) fn reconcile(
        &mut self,
        snapshot: Arc<CompanionSceneSnapshot>,
    ) -> Result<ReconcileDecision, RuntimeError> {
        validate_snapshot(&snapshot)?;
        let changes = classify_snapshot_changes(&self.snapshot, &snapshot);
        self.reconcile_validated(snapshot, changes)
    }

    fn reconcile_validated(
        &mut self,
        snapshot: Arc<CompanionSceneSnapshot>,
        changes: SnapshotChangeSet,
    ) -> Result<ReconcileDecision, RuntimeError> {
        let next_layout = if !changes.layout.is_empty() {
            LayoutGeneration(increment(
                self.layout_generation.0,
                CounterKind::LayoutGeneration,
            )?)
        } else {
            self.layout_generation
        };
        let next_semantic = if changes.has_semantic() {
            SemanticRevision(increment(
                self.semantic_revision.0,
                CounterKind::SemanticRevision,
            )?)
        } else {
            self.semantic_revision
        };
        let next_frame = if changes.has_frame() {
            FrameRevision(increment(
                self.frame_revision.0,
                CounterKind::FrameRevision,
            )?)
        } else {
            self.frame_revision
        };

        self.snapshot = Arc::clone(&snapshot);
        self.layout_generation = next_layout;
        self.semantic_revision = next_semantic;
        self.frame_revision = next_frame;
        if changes == SnapshotChangeSet::NONE {
            Ok(ReconcileDecision::Unchanged { snapshot })
        } else {
            Ok(ReconcileDecision::Changed {
                snapshot,
                changes,
                layout: next_layout,
                applied: AppliedRevisions {
                    semantic: next_semantic,
                    frame: next_frame,
                },
            })
        }
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RequestId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GenerationRequest {
    request_id: RequestId,
    key: SceneGenerationKey,
    surface: SurfaceEpoch,
    source: AppliedRevisions,
    snapshot: Arc<CompanionSceneSnapshot>,
}

impl GenerationRequest {
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

    fn version(&self, surface: SurfaceEpoch) -> SceneVersion {
        SceneVersion {
            generation: self.key,
            surface,
            applied: self.source,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AcceptedGenerationCandidate {
    request_id: RequestId,
    key: SceneGenerationKey,
    applied: AppliedRevisions,
    accepted: AcceptedSceneState,
}

impl AcceptedGenerationCandidate {
    pub(crate) fn new(
        request_id: RequestId,
        key: SceneGenerationKey,
        applied: AppliedRevisions,
        accepted: AcceptedSceneState,
    ) -> Self {
        Self { request_id, key, applied, accepted }
    }
}

#[derive(Debug)]
struct ActiveGeneration {
    version: SceneVersion,
    accepted: AcceptedSceneState,
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
    request: GenerationRequest,
    desired_surface: SurfaceEpoch,
    desired_source: AppliedRevisions,
    desired_snapshot: Arc<CompanionSceneSnapshot>,
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SnapshotSubmission {
    Unchanged,
    Updated(SnapshotChangeSet),
    GenerationRequested(GenerationRequest),
    HiddenCoalesced,
}

impl SnapshotSubmission {
    pub(crate) fn request(&self) -> Option<GenerationRequest> {
        match self {
            Self::GenerationRequested(request) => Some(request.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateCompletion {
    Ready,
    DroppedStale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerTransition {
    Started(RequestId),
    Idle,
    DroppedStale,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationStartError {
    NoReadyCandidate,
    CandidateNeedsRebase,
    Hidden,
    SurfaceUnavailable,
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
}

#[derive(Debug)]
pub(crate) struct CaptureLease<'a> {
    active: &'a ActiveGeneration,
}

impl CaptureLease<'_> {
    pub(crate) const fn version(&self) -> SceneVersion {
        self.active.version
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
    host_fallback_pending: bool,
    device_valid: bool,
    surface_valid: bool,
    cancelled_build_count: u64,
    request_count: u64,
    reconcile_count: u64,
    max_workers_observed: u8,
}

impl CompanionSceneRuntimeState {
    pub(crate) fn with_active(
        snapshot: Arc<CompanionSceneSnapshot>,
        accepted: AcceptedSceneState,
    ) -> Result<Self, RuntimeError> {
        let reconciler = CompanionSceneReconciler::new(snapshot)?;
        let generation = SceneGenerationKey {
            device: DeviceEpoch(1),
            layout: LayoutGeneration(1),
            resources: ResourceGeneration(1),
        };
        Ok(Self {
            active: Some(ActiveGeneration {
                version: SceneVersion {
                    generation,
                    surface: SurfaceEpoch(1),
                    applied: AppliedRevisions::new(1, 1),
                },
                accepted,
            }),
            pending: None,
            worker: WorkerState::Idle,
            visibility: RuntimeVisibility::Visible,
            hidden_latest: None,
            reconciler,
            device_epoch: DeviceEpoch(1),
            surface_epoch: SurfaceEpoch(1),
            resource_generation: ResourceGeneration(1),
            next_request_id: RequestId(1),
            next_activation_attempt_id: ActivationAttemptId(1),
            host_fallback_pending: false,
            device_valid: true,
            surface_valid: true,
            cancelled_build_count: 0,
            request_count: 0,
            reconcile_count: 0,
            max_workers_observed: 0,
        })
    }

    pub(crate) fn snapshot(&self) -> &Arc<CompanionSceneSnapshot> {
        self.reconciler.snapshot()
    }

    pub(crate) fn active_version(&self) -> Option<SceneVersion> {
        self.active.as_ref().map(|active| active.version)
    }

    pub(crate) fn pending_request(&self) -> Option<&GenerationRequest> {
        self.pending.as_ref().map(|pending| &pending.request)
    }

    pub(crate) fn pending_desired_source(&self) -> Option<AppliedRevisions> {
        self.pending.as_ref().map(|pending| pending.desired_source)
    }

    pub(crate) fn pending_desired_snapshot(&self) -> Option<&Arc<CompanionSceneSnapshot>> {
        self.pending
            .as_ref()
            .map(|pending| &pending.desired_snapshot)
    }

    fn preflight_generation_counters(&self) -> Result<(), RuntimeError> {
        increment(self.resource_generation.0, CounterKind::ResourceGeneration)?;
        increment(self.next_request_id.0, CounterKind::RequestId)?;
        Ok(())
    }

    pub(crate) fn submit_snapshot(
        &mut self,
        snapshot: Arc<CompanionSceneSnapshot>,
    ) -> Result<SnapshotSubmission, RuntimeError> {
        validate_snapshot(&snapshot)?;
        if self.visibility == RuntimeVisibility::Hidden {
            self.hidden_latest = Some(snapshot);
            return Ok(SnapshotSubmission::HiddenCoalesced);
        }
        let preview = classify_snapshot_changes(self.reconciler.snapshot(), &snapshot);
        if preview.requires_generation() {
            self.preflight_generation_counters()?;
        }
        let decision = self.reconciler.reconcile(snapshot)?;
        self.reconcile_count += 1;
        let changes = decision.changes();
        if changes == SnapshotChangeSet::NONE {
            return Ok(SnapshotSubmission::Unchanged);
        }
        if changes.requires_generation() {
            return self.queue_generation();
        }

        let applied = self.reconciler.applied_revisions();
        if let Some(active) = &mut self.active {
            active.version.applied = applied;
        }
        if let Some(pending) = &mut self.pending {
            pending.desired_source = applied;
            pending.desired_snapshot = Arc::clone(self.reconciler.snapshot());
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
        Ok(SnapshotSubmission::Updated(changes))
    }

    fn queue_generation(&mut self) -> Result<SnapshotSubmission, RuntimeError> {
        let resources = ResourceGeneration(increment(
            self.resource_generation.0,
            CounterKind::ResourceGeneration,
        )?);
        let request_id = self.next_request_id;
        let next_request_id = RequestId(increment(request_id.0, CounterKind::RequestId)?);
        let request = GenerationRequest {
            request_id,
            key: SceneGenerationKey {
                device: self.device_epoch,
                layout: self.reconciler.layout_generation(),
                resources,
            },
            surface: self.surface_epoch,
            source: self.reconciler.applied_revisions(),
            snapshot: Arc::clone(self.reconciler.snapshot()),
        };

        let mut superseded_activation = None;
        if let Some(old) = self.pending.take() {
            match old.phase {
                PendingPhase::Activating { candidate, attempt, .. }
                | PendingPhase::SupersedingActivation { candidate, attempt } => {
                    superseded_activation = Some((candidate, attempt));
                    self.worker = WorkerState::Idle;
                }
                _ => match self.worker {
                    WorkerState::Running(id) if id == old.request.request_id => {
                        self.worker = WorkerState::Cancelling(id);
                        self.cancelled_build_count += 1;
                    }
                    WorkerState::Cancelling(_) => {}
                    _ => self.worker = WorkerState::Idle,
                },
            }
        }

        let phase = if let Some((candidate, attempt)) = superseded_activation {
            PendingPhase::SupersedingActivation { candidate, attempt }
        } else if self.worker == WorkerState::Idle {
            self.worker = WorkerState::Running(request_id);
            self.max_workers_observed = self.max_workers_observed.max(1);
            PendingPhase::Preparing
        } else {
            PendingPhase::Queued
        };
        self.pending = Some(PendingGeneration {
            desired_surface: request.surface,
            desired_source: request.source,
            desired_snapshot: Arc::clone(&request.snapshot),
            request: request.clone(),
            phase,
        });
        self.resource_generation = resources;
        self.next_request_id = next_request_id;
        self.request_count += 1;
        Ok(SnapshotSubmission::GenerationRequested(request))
    }

    pub(crate) fn acknowledge_worker_cancelled(&mut self, id: RequestId) -> WorkerTransition {
        if self.worker != WorkerState::Cancelling(id) {
            return WorkerTransition::DroppedStale;
        }
        if let Some(pending) = &mut self.pending {
            if matches!(pending.phase, PendingPhase::Queued) {
                pending.phase = PendingPhase::Preparing;
                self.worker = WorkerState::Running(pending.request.request_id);
                self.max_workers_observed = self.max_workers_observed.max(1);
                return WorkerTransition::Started(pending.request.request_id);
            }
        }
        self.worker = WorkerState::Idle;
        WorkerTransition::Idle
    }

    pub(crate) fn complete_cpu_candidate(
        &mut self,
        candidate: AcceptedGenerationCandidate,
    ) -> CandidateCompletion {
        if self.worker == WorkerState::Cancelling(candidate.request_id) {
            self.acknowledge_worker_cancelled(candidate.request_id);
            return CandidateCompletion::DroppedStale;
        }
        let Some(pending) = &mut self.pending else {
            return CandidateCompletion::DroppedStale;
        };
        if self.worker != WorkerState::Running(candidate.request_id)
            || pending.request.request_id != candidate.request_id
            || pending.request.key != candidate.key
            || pending.request.source != candidate.applied
            || !matches!(pending.phase, PendingPhase::Preparing)
        {
            return CandidateCompletion::DroppedStale;
        }
        pending.phase = PendingPhase::Ready(candidate);
        self.worker = WorkerState::Idle;
        CandidateCompletion::Ready
    }

    pub(crate) fn replace_ready_candidate(
        &mut self,
        candidate: AcceptedGenerationCandidate,
    ) -> CandidateCompletion {
        let Some(pending) = &mut self.pending else {
            return CandidateCompletion::DroppedStale;
        };
        if pending.request.request_id != candidate.request_id
            || pending.request.key != candidate.key
            || pending.desired_source != candidate.applied
            || !matches!(pending.phase, PendingPhase::Ready(_))
        {
            return CandidateCompletion::DroppedStale;
        }
        pending.phase = PendingPhase::Ready(candidate);
        CandidateCompletion::Ready
    }

    pub(crate) fn begin_activation(&mut self) -> Result<ActivationAttempt, ActivationStartError> {
        if self.visibility == RuntimeVisibility::Hidden {
            return Err(ActivationStartError::Hidden);
        }
        if !self.device_valid || !self.surface_valid {
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
        if candidate.applied != pending.desired_source {
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
            request_id: pending.request.request_id,
            key: pending.request.key,
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
    ) -> ActivationTransition {
        let Some(pending) = &mut self.pending else {
            return ActivationTransition::DroppedStale;
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
                return ActivationTransition::DroppedStale;
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
            return ActivationTransition::DroppedStale;
        }

        if let ActivationAttemptOutcome::Fatal(failure) = outcome {
            let stale_surface_failure = matches!(
                failure,
                EpochFailure::SurfaceLost | EpochFailure::SurfaceValidation
            ) && attempt.surface != self.surface_epoch;
            let stale_device_failure = matches!(
                failure,
                EpochFailure::DeviceLost
                    | EpochFailure::Internal
                    | EpochFailure::OutOfMemory
                    | EpochFailure::UncertainPostSubmit
                    | EpochFailure::ImmediateGpuError
                    | EpochFailure::DelayedGpuError
            ) && attempt.key.device != self.device_epoch;
            if stale_surface_failure || stale_device_failure {
                if superseding {
                    if self.visibility == RuntimeVisibility::Visible {
                        self.worker = WorkerState::Running(pending.request.request_id);
                        self.max_workers_observed = self.max_workers_observed.max(1);
                        pending.phase = PendingPhase::Preparing;
                    } else {
                        self.worker = WorkerState::Idle;
                        pending.phase = PendingPhase::Queued;
                    }
                } else {
                    pending.phase = PendingPhase::Ready(candidate);
                }
                return ActivationTransition::DroppedStale;
            }
            let invalidates_device = matches!(
                failure,
                EpochFailure::DeviceLost
                    | EpochFailure::Internal
                    | EpochFailure::OutOfMemory
                    | EpochFailure::UncertainPostSubmit
                    | EpochFailure::ImmediateGpuError
                    | EpochFailure::DelayedGpuError
            );
            if invalidates_device {
                self.device_valid = false;
                self.active = None;
            }
            if matches!(
                failure,
                EpochFailure::SurfaceLost | EpochFailure::SurfaceValidation
            ) {
                self.surface_valid = false;
            }
            self.pending = None;
            self.host_fallback_pending = true;
            return ActivationTransition::HostFallbackPending;
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
            if self.visibility == RuntimeVisibility::Visible {
                self.worker = WorkerState::Running(pending.request.request_id);
                self.max_workers_observed = self.max_workers_observed.max(1);
                pending.phase = PendingPhase::Preparing;
            } else {
                self.worker = WorkerState::Idle;
                pending.phase = PendingPhase::Queued;
            }
            return transition;
        }

        match outcome {
            ActivationAttemptOutcome::Deferred(_) => {
                pending.phase = PendingPhase::Ready(candidate);
                ActivationTransition::RetryLater
            }
            ActivationAttemptOutcome::CandidateRejected(_) => {
                self.pending = None;
                ActivationTransition::CandidateDestroyedRetainingActive
            }
            ActivationAttemptOutcome::PresentedClean { surface }
                if commit_eligible
                    && surface == self.surface_epoch
                    && attempt.surface == pending.desired_surface
                    && attempt.applied == pending.desired_source
                    && candidate.request_id == pending.request.request_id
                    && candidate.key == pending.request.key
                    && candidate.applied == pending.desired_source =>
            {
                self.active = Some(ActiveGeneration {
                    version: SceneVersion {
                        generation: candidate.key,
                        surface,
                        applied: candidate.applied,
                    },
                    accepted: candidate.accepted,
                });
                self.pending = None;
                self.host_fallback_pending = false;
                ActivationTransition::Committed
            }
            ActivationAttemptOutcome::PresentedClean { .. } => {
                pending.phase = PendingPhase::Ready(candidate);
                ActivationTransition::DroppedStale
            }
            ActivationAttemptOutcome::Fatal(_) => unreachable!("handled above"),
        }
    }

    pub(crate) fn advance_surface_epoch(&mut self) -> Result<SurfaceEpoch, RuntimeError> {
        let next = SurfaceEpoch(increment(self.surface_epoch.0, CounterKind::SurfaceEpoch)?);
        self.surface_epoch = next;
        self.surface_valid = true;
        if let Some(active) = &mut self.active {
            active.version.surface = next;
        }
        if let Some(pending) = &mut self.pending {
            pending.desired_surface = next;
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
        Ok(next)
    }

    pub(crate) fn advance_device_epoch(&mut self) -> Result<DeviceEpoch, RuntimeError> {
        let next = DeviceEpoch(increment(self.device_epoch.0, CounterKind::DeviceEpoch)?);
        self.preflight_generation_counters()?;
        if let WorkerState::Running(id) = self.worker {
            self.worker = WorkerState::Cancelling(id);
            self.cancelled_build_count += 1;
        }
        self.device_epoch = next;
        self.device_valid = true;
        self.surface_valid = false;
        self.active = None;
        self.pending = None;
        self.host_fallback_pending = true;
        self.queue_generation()?;
        Ok(next)
    }

    pub(crate) fn capture_lease(&self) -> Result<CaptureLease<'_>, CaptureDefer> {
        if self.pending.as_ref().is_some_and(|pending| {
            matches!(
                pending.phase,
                PendingPhase::Activating { .. } | PendingPhase::SupersedingActivation { .. }
            )
        }) {
            return Err(CaptureDefer::ActivationInProgress);
        }
        if !self.device_valid || !self.surface_valid {
            return Err(CaptureDefer::NoActiveGeneration);
        }
        self.active
            .as_ref()
            .map(|active| CaptureLease { active })
            .ok_or(CaptureDefer::NoActiveGeneration)
    }

    pub(crate) fn set_hidden(&mut self) {
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
    }

    pub(crate) fn set_visible(&mut self) -> Result<SnapshotSubmission, RuntimeError> {
        self.visibility = RuntimeVisibility::Visible;
        let submission = match self.hidden_latest.take() {
            Some(snapshot) => self.submit_snapshot(snapshot),
            None => Ok(SnapshotSubmission::Unchanged),
        }?;
        if self.worker == WorkerState::Idle {
            if let Some(pending) = &mut self.pending {
                if matches!(pending.phase, PendingPhase::Queued) {
                    pending.phase = PendingPhase::Preparing;
                    self.worker = WorkerState::Running(pending.request.request_id);
                    self.max_workers_observed = self.max_workers_observed.max(1);
                }
            }
        }
        Ok(submission)
    }

    pub(crate) const fn host_fallback_pending(&self) -> bool {
        self.host_fallback_pending
    }

    pub(crate) fn observe_delayed_gpu_error(
        &mut self,
        device: DeviceEpoch,
    ) -> ActivationTransition {
        if device != self.device_epoch {
            return ActivationTransition::DroppedStale;
        }
        self.device_valid = false;
        self.active = None;
        self.pending = None;
        if let WorkerState::Running(id) = self.worker {
            self.worker = WorkerState::Cancelling(id);
            self.cancelled_build_count += 1;
        }
        self.host_fallback_pending = true;
        ActivationTransition::HostFallbackPending
    }

    pub(crate) fn shutdown(&mut self) {
        self.pending = None;
        self.hidden_latest = None;
        self.worker = match self.worker {
            WorkerState::Running(id) => WorkerState::Cancelling(id),
            state => state,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::Species;
    use crate::presentation::companion_scene::scene::SceneFixture;
    use crate::presentation::companion_scene::validate::validate_full_generation;
    use crate::presentation::companion_scene::{
        AuthoredDepthSnapshot, CompanionLogicalLayout, CompanionSceneSnapshot, ContentSnapshot,
        FrameSnapshot, GaugeLevelSnapshot, PaletteSnapshot, PetLatticeSnapshot,
        PetRoleSpanSnapshot, PetTopologySnapshot, PropAnimationKindSnapshot, PropAnimationSnapshot,
        PropTopologySnapshot, PropZoneSnapshot, RoomTopologySnapshot, TankAnimationSnapshot,
        TankBoundsSnapshot, TankCellSnapshot, TankLayerSnapshot, TankRouteSnapshot,
        TankSideSnapshot, TankTopologySnapshot, TopologySnapshot,
        COMPANION_RENDERER_SCHEMA_VERSION, COMPANION_SCENE_SCHEMA_VERSION, PET_LATTICE_HEIGHT,
        PET_LATTICE_SLOTS, PET_LATTICE_WIDTH,
    };
    use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};
    use std::sync::Arc;

    fn snapshot() -> Arc<CompanionSceneSnapshot> {
        Arc::new(CompanionSceneSnapshot {
            schema_version: COMPANION_SCENE_SCHEMA_VERSION,
            privacy: PrivacyProjection::for_surface(PresentationSurface::RoundCompanion),
            topology: TopologySnapshot {
                layout: CompanionLogicalLayout::round(360.0, 360.0),
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
                    catalog_id: "treasure_chest",
                    stable_order: 0,
                    zone: PropZoneSnapshot::FloorRight,
                    authored_depth: AuthoredDepthSnapshot::Foreground,
                }],
                visible_tank_inhabitants: vec![TankTopologySnapshot {
                    catalog_id: "bytefish",
                    stable_order: 0,
                    route: TankRouteSnapshot::CrossTankSwimmer,
                    authored_depth: AuthoredDepthSnapshot::BehindPet,
                }],
                renderer_schema: COMPANION_RENDERER_SCHEMA_VERSION,
            },
            content: ContentSnapshot {
                mood: Mood::Happy,
                room_weather: "clear",
                pet_lines: vec!["             ".to_owned(); usize::from(PET_LATTICE_HEIGHT)],
                pet_roles: vec![PetRoleSpanSnapshot {
                    line_index: 0,
                    start_char: 0,
                    end_char: 1,
                    role: "body",
                }],
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
                    catalog_id: "treasure_chest",
                    stable_order: 0,
                    kind: PropAnimationKindSnapshot::Animated,
                    sprite_phase: Some(0),
                    twinkle_active: Some(false),
                    motion_phase: Some(0),
                    chest_lid_open: Some(false),
                }],
                tank_animation_states: vec![TankAnimationSnapshot {
                    catalog_id: "bytefish",
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
                    cadence_ms: 400,
                    calm: false,
                    cells: vec![TankCellSnapshot {
                        col: 4,
                        row: 5,
                        glyph: '<',
                        layer: TankLayerSnapshot::Behind,
                    }],
                    bounds: Some(TankBoundsSnapshot { x: 4, y: 5, width: 1, height: 1 }),
                }],
                activity_pulse_age_ms: Some(100),
            },
            frame: FrameSnapshot {
                elapsed_ms: 1_000,
                pet_anchor_points: [120.0, 140.0],
                pet_depth: 0.5,
                facing: 1,
                breath_offset_y_cells: 0,
                bob_offset_y_cells: 0.25,
                asleep: false,
                helper_trouble: false,
                gauges: [GaugeLevelSnapshot::Medium; 4],
                dim_amount: 0.0,
                hud_lines: ["today".to_owned(), "daily".to_owned(), "pace".to_owned()],
            },
        })
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
        let semantic_frame = semantic.union(frame);

        assert_class!(layout_width, generation, |s| s
            .topology
            .layout
            .width_points += 1.0);
        assert_class!(layout_height, generation, |s| s
            .topology
            .layout
            .height_points += 1.0);
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
        assert_class!(pet_roles, semantic, |s| s.content.pet_roles[0].role = "eye");
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
        assert_class!(prop_motion, frame, |s| s.content.prop_animation_states[0]
            .motion_phase = Some(1));
        assert_class!(prop_lid, semantic, |s| s.content.prop_animation_states[0]
            .chest_lid_open = Some(true));
        assert_class!(tank_visible, frame, |s| s.content.tank_animation_states
            [0]
        .visible = false);
        assert_class!(tank_origin_col, frame, |s| s
            .content
            .tank_animation_states[0]
            .origin_col += 1);
        assert_class!(tank_origin_row, frame, |s| s
            .content
            .tank_animation_states[0]
            .origin_row += 1);
        assert_class!(tank_side, frame, |s| s.content.tank_animation_states[0]
            .side =
            Some(TankSideSnapshot::Right));
        assert_class!(tank_layer, frame, |s| s.content.tank_animation_states[0]
            .layer =
            TankLayerSnapshot::Foreground);
        assert_class!(tank_variant, semantic, |s| s
            .content
            .tank_animation_states[0]
            .sprite_variant = 1);
        assert_class!(tank_visible_rows, frame, |s| s
            .content
            .tank_animation_states[0]
            .visible_rows += 1);
        assert_class!(tank_morph, semantic, |s| s.content.tank_animation_states
            [0]
        .anemone_morph = Some(1));
        assert_class!(tank_cell_position, frame, |s| s
            .content
            .tank_animation_states[0]
            .cells[0]
            .col += 1);
        assert_class!(tank_cell_glyph, semantic, |s| s
            .content
            .tank_animation_states[0]
            .cells[0]
            .glyph = '>');
        assert_class!(tank_cell_layer, frame, |s| s
            .content
            .tank_animation_states[0]
            .cells[0]
            .layer =
            TankLayerSnapshot::Foreground);
        assert_class!(tank_bounds, frame, |s| s.content.tank_animation_states
            [0]
        .bounds
        .as_mut()
        .unwrap()
        .x += 1);
        assert_class!(activity_age, frame, |s| s.content.activity_pulse_age_ms =
            Some(101));
        assert_class!(activity_activation, semantic_frame, |s| s
            .content
            .activity_pulse_age_ms =
            None);

        assert_class!(pet_anchor, frame, |s| s.frame.pet_anchor_points[0] += 1.0);
        assert_class!(pet_depth, frame, |s| s.frame.pet_depth += 0.1);
        assert_class!(facing, frame, |s| s.frame.facing = -1);
        assert_class!(breath, frame, |s| s.frame.breath_offset_y_cells = 1);
        assert_class!(bob, frame, |s| s.frame.bob_offset_y_cells += 0.1);
        assert_class!(asleep, frame, |s| s.frame.asleep = true);
        assert_class!(helper, frame, |s| s.frame.helper_trouble = true);
        assert_class!(gauges, frame, |s| s.frame.gauges[0] =
            GaugeLevelSnapshot::High);
        assert_class!(dim, frame, |s| s.frame.dim_amount = 0.35);
        assert_class!(hud, semantic, |s| s.frame.hud_lines[0] = "new".to_owned());

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
    fn fixed_named_masks_are_task_five_extensible() {
        let mood = classify(|s| s.content.mood = Mood::Content);
        assert!(mood.semantic().contains(SemanticChangeMask::MOOD_WEATHER));
        assert!(!mood.semantic().contains(SemanticChangeMask::PET_ART));

        let pet_motion = classify(|s| s.frame.pet_depth += 0.1);
        assert!(pet_motion.frame().contains(FrameChangeMask::PET_TRANSFORM));
        let status = classify(|s| s.frame.helper_trouble = true);
        assert!(status.frame().contains(FrameChangeMask::STATUS_VISIBILITY));

        assert!(SemanticChangeMask::AMBIENT.is_named());
        assert!(FrameChangeMask::CAMERA.is_named());
        assert!(FrameChangeMask::LIGHTS.is_named());
        assert!(ResourceChangeMask::AMBIENT_AUTHORED.is_named());
    }

    #[test]
    fn mixed_change_advances_each_relevant_counter_once() {
        let initial = snapshot();
        let mut reconciler = CompanionSceneReconciler::new(Arc::clone(&initial)).unwrap();
        let mut mixed = (*initial).clone();
        mixed.topology.layout.width_points += 1.0;
        mixed.content.palette.eye[0] += 1;
        mixed.frame.pet_depth += 0.1;
        let decision = reconciler.reconcile(Arc::new(mixed)).unwrap();
        assert!(decision.changes().requires_generation());
        assert_eq!(reconciler.layout_generation(), LayoutGeneration(2));
        assert_eq!(reconciler.applied_revisions(), AppliedRevisions::new(2, 2));
    }

    #[test]
    fn invalid_snapshots_are_transactionally_rejected() {
        type SnapshotMutation = Box<dyn Fn(&mut CompanionSceneSnapshot)>;

        let initial = snapshot();
        let invalids: Vec<SnapshotMutation> = vec![
            Box::new(|s| s.schema_version += 1),
            Box::new(|s| s.topology.renderer_schema += 1),
            Box::new(|s| s.privacy.surface = PresentationSurface::WatchTui),
            Box::new(|s| s.privacy.source_names_visible = true),
            Box::new(|s| s.frame.pet_depth = f32::NAN),
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
            let mut reconciler = CompanionSceneReconciler::new(Arc::clone(&initial)).unwrap();
            let mut invalid = (*initial).clone();
            mutate(&mut invalid);
            assert!(reconciler.reconcile(Arc::new(invalid)).is_err());
            assert!(Arc::ptr_eq(reconciler.snapshot(), &initial));
            assert_eq!(reconciler.layout_generation(), LayoutGeneration(1));
            assert_eq!(reconciler.applied_revisions(), AppliedRevisions::new(1, 1));
        }
    }

    #[test]
    fn counter_overflow_is_typed_and_fail_closed() {
        let initial = snapshot();
        let mut reconciler = CompanionSceneReconciler::new(Arc::clone(&initial)).unwrap();
        reconciler.layout_generation = LayoutGeneration(u64::MAX);
        let mut changed = (*initial).clone();
        changed.topology.layout.width_points += 1.0;
        assert_eq!(
            reconciler.reconcile(Arc::new(changed)),
            Err(RuntimeError::CounterOverflow(CounterKind::LayoutGeneration))
        );
        assert!(Arc::ptr_eq(reconciler.snapshot(), &initial));

        let accepted = accepted_state();
        let mut runtime = CompanionSceneRuntimeState::with_active(initial, accepted).unwrap();
        runtime.next_request_id = RequestId(u64::MAX);
        let mut changed = (*snapshot()).clone();
        changed.topology.pet.stage = Stage::S4;
        assert_eq!(
            runtime.submit_snapshot(Arc::new(changed)),
            Err(RuntimeError::CounterOverflow(CounterKind::RequestId))
        );
        assert!(runtime.pending.is_none());
    }

    fn accepted_state() -> crate::presentation::companion_scene::validate::AcceptedSceneState {
        let fixture = SceneFixture::valid();
        validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap()
    }

    fn runtime() -> CompanionSceneRuntimeState {
        CompanionSceneRuntimeState::with_active(snapshot(), accepted_state()).unwrap()
    }

    fn topology_update(
        base: &Arc<CompanionSceneSnapshot>,
        stage: Stage,
    ) -> Arc<CompanionSceneSnapshot> {
        let mut next = (**base).clone();
        next.topology.pet.stage = stage;
        Arc::new(next)
    }

    #[test]
    fn rapid_topology_storm_has_one_worker_and_only_newest_request() {
        let mut runtime = runtime();
        let first = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        assert_eq!(runtime.worker, WorkerState::Running(first.request_id));
        let second = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S5))
            .unwrap()
            .request()
            .unwrap();
        assert_eq!(runtime.worker, WorkerState::Cancelling(first.request_id));
        assert_eq!(
            runtime.pending_request().unwrap().request_id,
            second.request_id
        );
        assert_ne!(first.request_id, second.request_id);
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&first)),
            CandidateCompletion::DroppedStale
        );
        assert_eq!(runtime.worker, WorkerState::Running(second.request_id),);
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&first)),
            CandidateCompletion::DroppedStale
        );
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&second)),
            CandidateCompletion::Ready
        );
        assert_eq!(runtime.cancelled_build_count, 1);
        assert_eq!(runtime.max_workers_observed, 1);
    }

    #[test]
    fn cancellation_ack_then_late_old_completion_also_starts_only_newest() {
        let mut runtime = runtime();
        let first = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        let second = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S5))
            .unwrap()
            .request()
            .unwrap();
        assert_eq!(
            runtime.acknowledge_worker_cancelled(first.request_id),
            WorkerTransition::Started(second.request_id)
        );
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&first)),
            CandidateCompletion::DroppedStale
        );
        assert_eq!(runtime.worker, WorkerState::Running(second.request_id));
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&second)),
            CandidateCompletion::Ready
        );
    }

    fn candidate(request: &GenerationRequest) -> AcceptedGenerationCandidate {
        candidate_at(request, request.source)
    }

    fn candidate_at(
        request: &GenerationRequest,
        applied: AppliedRevisions,
    ) -> AcceptedGenerationCandidate {
        AcceptedGenerationCandidate::new(request.request_id, request.key, applied, accepted_state())
    }

    #[test]
    fn compatible_updates_rebase_pending_and_require_exact_candidate_proof() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        let mut semantic = (**runtime.snapshot()).clone();
        semantic.content.mood = Mood::Content;
        runtime.submit_snapshot(Arc::new(semantic)).unwrap();
        assert_eq!(
            runtime.pending_request().unwrap().source,
            request.source,
            "the worker request remains immutable"
        );
        assert_eq!(
            runtime.pending_desired_source().unwrap().semantic,
            SemanticRevision(2)
        );
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&request)),
            CandidateCompletion::Ready
        );
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );
        let rebased = candidate_at(&request, runtime.pending_desired_source().unwrap());
        assert_eq!(
            runtime.replace_ready_candidate(rebased),
            CandidateCompletion::Ready
        );

        let mut frame = (**runtime.snapshot()).clone();
        frame.frame.pet_depth += 0.1;
        runtime.submit_snapshot(Arc::new(frame)).unwrap();
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );
        let rebased = candidate_at(&request, runtime.pending_desired_source().unwrap());
        assert_eq!(
            runtime.replace_ready_candidate(rebased),
            CandidateCompletion::Ready
        );
        assert!(runtime.begin_activation().is_ok());
    }

    #[test]
    fn compatible_update_during_activation_defers_old_attempt_for_exact_rebase() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let old_attempt = runtime.begin_activation().unwrap();

        let mut newest = (**runtime.snapshot()).clone();
        newest.frame.pet_depth += 0.1;
        runtime.submit_snapshot(Arc::new(newest)).unwrap();
        assert_eq!(
            runtime.finish_activation(
                old_attempt,
                ActivationAttemptOutcome::PresentedClean { surface: old_attempt.surface },
            ),
            ActivationTransition::DroppedStale
        );
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );
        let rebased = candidate_at(&request, runtime.pending_desired_source().unwrap());
        runtime.replace_ready_candidate(rebased);
        let newest_attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.finish_activation(
                newest_attempt,
                ActivationAttemptOutcome::PresentedClean { surface: newest_attempt.surface },
            ),
            ActivationTransition::Committed
        );
    }

    #[test]
    fn stale_in_flight_activation_still_honors_rejection_and_fatal_outcomes() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let attempt = runtime.begin_activation().unwrap();
        let mut newest = (**runtime.snapshot()).clone();
        newest.frame.pet_depth += 0.1;
        runtime.submit_snapshot(Arc::new(newest)).unwrap();
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::Fatal(EpochFailure::ImmediateGpuError),
            ),
            ActivationTransition::HostFallbackPending
        );
        assert_eq!(runtime.active_version(), None);

        let mut rejected_runtime =
            CompanionSceneRuntimeState::with_active(snapshot(), accepted_state()).unwrap();
        let request = rejected_runtime
            .submit_snapshot(topology_update(rejected_runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        rejected_runtime.complete_cpu_candidate(candidate(&request));
        let attempt = rejected_runtime.begin_activation().unwrap();
        rejected_runtime.set_hidden();
        assert_eq!(
            rejected_runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Resource),
            ),
            ActivationTransition::CandidateDestroyedRetainingActive
        );
        assert!(rejected_runtime.pending.is_none());
        assert!(rejected_runtime.active_version().is_some());
    }

    #[test]
    fn topology_supersession_keeps_old_attempt_failures_actionable() {
        let mut runtime = runtime();
        let first = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&first));
        let attempt = runtime.begin_activation().unwrap();
        let second = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S5))
            .unwrap()
            .request()
            .unwrap();
        assert_ne!(first.request_id, second.request_id);
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::Fatal(EpochFailure::DeviceLost),
            ),
            ActivationTransition::HostFallbackPending
        );
        assert_eq!(runtime.active_version(), None);
        assert!(runtime.pending.is_none());
    }

    #[test]
    fn topology_supersession_defers_capture_until_old_attempt_finishes() {
        let mut runtime = runtime();
        let first = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&first));
        let attempt = runtime.begin_activation().unwrap();
        runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S5))
            .unwrap();
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::ActivationInProgress
        );
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
            ),
            ActivationTransition::DroppedStale
        );
    }

    #[test]
    fn old_surface_failure_cannot_invalidate_new_surface_epoch() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let old_attempt = runtime.begin_activation().unwrap();
        assert_eq!(runtime.advance_surface_epoch().unwrap(), SurfaceEpoch(2));
        assert_eq!(
            runtime.finish_activation(
                old_attempt,
                ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
            ),
            ActivationTransition::DroppedStale
        );
        assert!(!runtime.host_fallback_pending());
        assert!(runtime.capture_lease().is_ok());
        assert!(runtime.begin_activation().is_ok());
    }

    #[test]
    fn resource_only_change_requests_generation_without_advancing_layout() {
        let initial = snapshot();
        let mut reconciler = CompanionSceneReconciler::new(Arc::clone(&initial)).unwrap();
        let changes = SnapshotChangeSet {
            resources: ResourceChangeMask::MATERIAL_CONTRACT,
            ..SnapshotChangeSet::NONE
        };
        let decision = reconciler
            .reconcile_validated(Arc::clone(&initial), changes)
            .unwrap();
        assert!(decision.changes().requires_generation());
        assert_eq!(reconciler.layout_generation(), LayoutGeneration(1));
    }

    #[test]
    fn surface_rebind_does_not_allocate_scene_generation_and_device_change_stales_work() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        assert_eq!(runtime.advance_surface_epoch().unwrap(), SurfaceEpoch(2));
        let after = runtime.active_version().unwrap();
        assert_eq!(before.generation, after.generation);
        assert_eq!(after.surface, SurfaceEpoch(2));

        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        assert_eq!(runtime.advance_device_epoch().unwrap(), DeviceEpoch(2));
        let replacement = runtime.pending_request().unwrap().clone();
        assert_eq!(replacement.key.device, DeviceEpoch(2));
        assert_ne!(replacement.request_id, request.request_id);
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&request)),
            CandidateCompletion::DroppedStale
        );
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&replacement)),
            CandidateCompletion::Ready
        );
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::SurfaceUnavailable)
        );
        runtime.advance_surface_epoch().unwrap();
        assert!(runtime.begin_activation().is_ok());
        assert!(runtime.host_fallback_pending());
    }

    #[test]
    fn activation_is_exact_typed_and_retains_active_until_clean_present() {
        let mut runtime = runtime();
        let previous = runtime.active_version().unwrap();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&request)),
            CandidateCompletion::Ready
        );

        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(runtime.active_version(), Some(previous));
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::Deferred(AcquireDeferral::Timeout)
            ),
            ActivationTransition::RetryLater
        );
        assert_eq!(runtime.active_version(), Some(previous));

        let attempt = runtime.begin_activation().unwrap();
        let wrong = ActivationAttempt {
            attempt_id: ActivationAttemptId(attempt.attempt_id.0 + 1),
            ..attempt
        };
        assert_eq!(
            runtime.finish_activation(
                wrong,
                ActivationAttemptOutcome::PresentedClean { surface: attempt.surface }
            ),
            ActivationTransition::DroppedStale
        );
        assert_eq!(runtime.active_version(), Some(previous));
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::Fatal(EpochFailure::ImmediateGpuError)
            ),
            ActivationTransition::HostFallbackPending
        );
        assert_ne!(
            runtime.active_version(),
            Some(request.version(SurfaceEpoch(1)))
        );
    }

    #[test]
    fn every_typed_deferral_and_candidate_failure_retains_active() {
        for deferral in [
            AcquireDeferral::OutdatedReconfigured,
            AcquireDeferral::Timeout,
            AcquireDeferral::Occluded,
        ] {
            let mut runtime = runtime();
            let previous = runtime.active_version().unwrap();
            let request = runtime
                .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
                .unwrap()
                .request()
                .unwrap();
            runtime.complete_cpu_candidate(candidate(&request));
            let attempt = runtime.begin_activation().unwrap();
            assert_eq!(
                runtime.finish_activation(attempt, ActivationAttemptOutcome::Deferred(deferral)),
                ActivationTransition::RetryLater
            );
            assert_eq!(runtime.active_version(), Some(previous));
        }

        for failure in [
            CandidateFailure::Validation,
            CandidateFailure::Resource,
            CandidateFailure::PreSubmitEncode,
        ] {
            let mut runtime = runtime();
            let previous = runtime.active_version().unwrap();
            let request = runtime
                .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
                .unwrap()
                .request()
                .unwrap();
            runtime.complete_cpu_candidate(candidate(&request));
            let attempt = runtime.begin_activation().unwrap();
            assert_eq!(
                runtime.finish_activation(
                    attempt,
                    ActivationAttemptOutcome::CandidateRejected(failure)
                ),
                ActivationTransition::CandidateDestroyedRetainingActive
            );
            assert_eq!(runtime.active_version(), Some(previous));
        }
    }

    #[test]
    fn every_epoch_failure_falls_back_and_wrong_exact_identity_cannot_commit() {
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
            let mut runtime = runtime();
            let request = runtime
                .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
                .unwrap()
                .request()
                .unwrap();
            runtime.complete_cpu_candidate(candidate(&request));
            let attempt = runtime.begin_activation().unwrap();
            assert_eq!(
                runtime.finish_activation(attempt, ActivationAttemptOutcome::Fatal(failure)),
                ActivationTransition::HostFallbackPending
            );
            assert!(runtime.host_fallback_pending());
        }

        let mut runtime = runtime();
        let previous = runtime.active_version().unwrap();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        let mut wrong_candidate = candidate(&request);
        wrong_candidate.key.resources.0 += 1;
        assert_eq!(
            runtime.complete_cpu_candidate(wrong_candidate),
            CandidateCompletion::DroppedStale
        );
        runtime.complete_cpu_candidate(candidate(&request));
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean {
                    surface: SurfaceEpoch(attempt.surface.0 + 1),
                },
            ),
            ActivationTransition::DroppedStale
        );
        assert_eq!(runtime.active_version(), Some(previous));
    }

    #[test]
    fn every_owned_counter_starts_at_one_and_overflows_without_mutation() {
        let initial = snapshot();
        let accepted = accepted_state();
        let mut runtime = CompanionSceneRuntimeState::with_active(initial, accepted).unwrap();
        let version = runtime.active_version().unwrap();
        assert_eq!(version.generation.device, DeviceEpoch(1));
        assert_eq!(version.surface, SurfaceEpoch(1));
        assert_eq!(version.generation.layout, LayoutGeneration(1));
        assert_eq!(version.generation.resources, ResourceGeneration(1));
        assert_eq!(version.applied, AppliedRevisions::new(1, 1));
        assert_eq!(runtime.next_request_id, RequestId(1));
        assert_eq!(runtime.next_activation_attempt_id, ActivationAttemptId(1));

        runtime.resource_generation = ResourceGeneration(u64::MAX);
        let mut changed = (**runtime.snapshot()).clone();
        changed.topology.pet.stage = Stage::S4;
        assert_eq!(
            runtime.submit_snapshot(Arc::new(changed)),
            Err(RuntimeError::CounterOverflow(
                CounterKind::ResourceGeneration
            ))
        );
        assert!(runtime.pending.is_none());

        runtime.surface_epoch = SurfaceEpoch(u64::MAX);
        assert_eq!(
            runtime.advance_surface_epoch(),
            Err(RuntimeError::CounterOverflow(CounterKind::SurfaceEpoch))
        );
        runtime.device_epoch = DeviceEpoch(u64::MAX);
        assert_eq!(
            runtime.advance_device_epoch(),
            Err(RuntimeError::CounterOverflow(CounterKind::DeviceEpoch))
        );

        let mut reconciler = CompanionSceneReconciler::new(snapshot()).unwrap();
        reconciler.semantic_revision = SemanticRevision(u64::MAX);
        let mut semantic = (**reconciler.snapshot()).clone();
        semantic.content.mood = Mood::Content;
        assert_eq!(
            reconciler.reconcile(Arc::new(semantic)),
            Err(RuntimeError::CounterOverflow(CounterKind::SemanticRevision))
        );
        let mut reconciler = CompanionSceneReconciler::new(snapshot()).unwrap();
        reconciler.frame_revision = FrameRevision(u64::MAX);
        let mut frame = (**reconciler.snapshot()).clone();
        frame.frame.pet_depth += 0.1;
        assert_eq!(
            reconciler.reconcile(Arc::new(frame)),
            Err(RuntimeError::CounterOverflow(CounterKind::FrameRevision))
        );

        let mut activation_runtime =
            CompanionSceneRuntimeState::with_active(snapshot(), accepted_state()).unwrap();
        let request = activation_runtime
            .submit_snapshot(topology_update(activation_runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        activation_runtime.complete_cpu_candidate(candidate(&request));
        activation_runtime.next_activation_attempt_id = ActivationAttemptId(u64::MAX);
        assert_eq!(
            activation_runtime.begin_activation(),
            Err(ActivationStartError::CounterOverflow(
                CounterKind::ActivationAttemptId
            ))
        );
    }

    #[test]
    fn candidate_rejection_retains_previous_and_clean_present_commits_exact_candidate() {
        let mut runtime = runtime();
        let previous = runtime.active_version().unwrap();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::CandidateRejected(CandidateFailure::PreSubmitEncode)
            ),
            ActivationTransition::CandidateDestroyedRetainingActive
        );
        assert_eq!(runtime.active_version(), Some(previous));

        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S5))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean { surface: attempt.surface }
            ),
            ActivationTransition::Committed
        );
        assert_eq!(
            runtime.active_version(),
            Some(request.version(attempt.surface))
        );
    }

    #[test]
    fn accepted_candidate_owns_validation_proof_after_source_drop() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        let accepted = {
            let fixture = SceneFixture::valid();
            validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap()
        };
        let checksum = accepted.template().template().generation_checksum;
        let candidate = AcceptedGenerationCandidate::new(
            request.request_id,
            request.key,
            request.source,
            accepted,
        );
        assert_eq!(
            candidate.accepted.template().template().generation_checksum,
            checksum
        );
    }

    #[test]
    fn delayed_post_commit_error_falls_back_without_resurrecting_previous() {
        let mut runtime = runtime();
        let previous = runtime.active_version().unwrap();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
            ),
            ActivationTransition::Committed
        );
        let committed = runtime.active_version().unwrap();
        assert_ne!(committed, previous);
        assert_eq!(
            runtime.observe_delayed_gpu_error(committed.generation.device),
            ActivationTransition::HostFallbackPending
        );
        assert!(runtime.host_fallback_pending());
        assert_eq!(runtime.active_version(), None);
        assert_ne!(runtime.active_version(), Some(previous));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::NoActiveGeneration
        );
    }

    #[test]
    fn delayed_error_from_retired_device_cannot_invalidate_new_device() {
        let mut runtime = runtime();
        let retired = runtime.active_version().unwrap().generation.device;
        runtime.advance_device_epoch().unwrap();
        runtime.advance_surface_epoch().unwrap();
        let replacement = runtime.pending_request().unwrap().clone();
        runtime.complete_cpu_candidate(candidate(&replacement));
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        let current = runtime.active_version().unwrap();
        assert_ne!(current.generation.device, retired);
        assert_eq!(
            runtime.observe_delayed_gpu_error(retired),
            ActivationTransition::DroppedStale
        );
        assert_eq!(runtime.active_version(), Some(current));
        assert!(!runtime.host_fallback_pending());
    }

    #[test]
    fn delayed_current_device_error_cannot_be_resurrected_by_in_flight_activation() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.observe_delayed_gpu_error(attempt.key.device),
            ActivationTransition::HostFallbackPending
        );
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
            ),
            ActivationTransition::DroppedStale
        );
        assert_eq!(runtime.active_version(), None);
        assert!(runtime.host_fallback_pending());
    }

    #[test]
    fn delayed_current_device_error_cancels_preparing_worker() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        assert_eq!(runtime.worker, WorkerState::Running(request.request_id));
        assert_eq!(
            runtime.observe_delayed_gpu_error(request.key.device),
            ActivationTransition::HostFallbackPending
        );
        assert!(runtime.pending.is_none());
        assert_eq!(runtime.worker, WorkerState::Cancelling(request.request_id));
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&request)),
            CandidateCompletion::DroppedStale
        );
        assert_eq!(runtime.worker, WorkerState::Idle);
        assert_eq!(runtime.active_version(), None);
    }

    #[test]
    fn capture_lease_binds_exact_active_version_and_defers_while_activating() {
        let mut runtime = runtime();
        let leased_version = {
            let lease = runtime.capture_lease().unwrap();
            lease.version()
        };
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        let attempt = runtime.begin_activation().unwrap();
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::ActivationInProgress
        );
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_ne!(runtime.active_version().unwrap(), leased_version);
    }

    #[test]
    fn hidden_snapshots_coalesce_without_work_and_reveal_reconciles_once() {
        let mut runtime = runtime();
        runtime.set_hidden();
        for step in 0..10 {
            let mut next = (**runtime.snapshot()).clone();
            next.frame.pet_depth = 0.5 + step as f32 * 0.01;
            if step == 9 {
                next.topology.pet.stage = Stage::S4;
            }
            assert_eq!(
                runtime.submit_snapshot(Arc::new(next)).unwrap(),
                SnapshotSubmission::HiddenCoalesced
            );
        }
        assert_eq!(runtime.reconcile_count, 0);
        assert!(runtime.pending.is_none());
        let reveal = runtime.set_visible().unwrap();
        assert!(reveal.request().is_some());
        assert_eq!(runtime.reconcile_count, 1);
        assert_eq!(runtime.request_count, 1);
    }

    #[test]
    fn hidden_state_defers_ready_or_in_flight_activation() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.complete_cpu_candidate(candidate(&request));
        runtime.set_hidden();
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::Hidden)
        );
        runtime.set_visible().unwrap();
        let attempt = runtime.begin_activation().unwrap();
        runtime.set_hidden();
        assert_eq!(
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
            ),
            ActivationTransition::DroppedStale
        );
        assert!(runtime.active_version().is_some());
    }

    #[test]
    fn shutdown_cancels_the_only_worker_and_late_completion_drops() {
        let mut runtime = runtime();
        let request = runtime
            .submit_snapshot(topology_update(runtime.snapshot(), Stage::S4))
            .unwrap()
            .request()
            .unwrap();
        runtime.shutdown();
        assert_eq!(runtime.worker, WorkerState::Cancelling(request.request_id));
        assert_eq!(
            runtime.complete_cpu_candidate(candidate(&request)),
            CandidateCompletion::DroppedStale
        );
    }

    #[test]
    fn fixed_capacity_overflow_never_requests_a_generation() {
        let mut runtime = runtime();
        let mut invalid = (**runtime.snapshot()).clone();
        let prop = invalid.topology.visible_props[0].clone();
        invalid.topology.visible_props.resize(
            crate::presentation::companion_scene::MAX_VISIBLE_PROPS + 1,
            prop,
        );
        assert!(runtime.submit_snapshot(Arc::new(invalid)).is_err());
        assert!(runtime.pending.is_none());
        assert_eq!(runtime.request_count, 0);
    }

    #[test]
    fn ordinary_reconciliation_shares_snapshots_and_uses_fixed_masks() {
        assert_eq!(std::mem::size_of::<SnapshotChangeSet>(), 32);
        let initial = snapshot();
        let mut reconciler = CompanionSceneReconciler::new(Arc::clone(&initial)).unwrap();
        let mut frame = (*initial).clone();
        frame.frame.pet_depth += 0.1;
        let next = Arc::new(frame);
        reconciler.reconcile(Arc::clone(&next)).unwrap();
        assert!(Arc::ptr_eq(reconciler.snapshot(), &next));
        assert_eq!(Arc::strong_count(&next), 2);
    }
}
