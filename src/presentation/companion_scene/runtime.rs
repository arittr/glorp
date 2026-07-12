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
    pub(crate) const BACKING_SCALE_ATLAS: Self = Self(1 << 6);
    pub(crate) const SURFACE_RECOVERY: Self = Self(1 << 7);

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RequestId(pub u64);

#[derive(Debug, PartialEq)]
pub(crate) struct GenerationRequest {
    request_id: RequestId,
    key: SceneGenerationKey,
    surface: SurfaceEpoch,
    source: AppliedRevisions,
    snapshot: Arc<CompanionSceneSnapshot>,
}

impl GenerationRequest {
    fn duplicate_for_start(&self) -> Self {
        Self {
            request_id: self.request_id,
            key: self.key,
            surface: self.surface,
            source: self.source,
            snapshot: Arc::clone(&self.snapshot),
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

    pub(crate) fn accept(&self, accepted: AcceptedSceneState) -> AcceptedGenerationCandidate {
        AcceptedGenerationCandidate {
            request_id: self.request_id,
            key: self.key,
            applied: self.source,
            accepted,
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
    Shutdown,
    DroppedStale,
}

#[derive(Debug, PartialEq)]
pub(crate) struct RuntimeEffects {
    disposition: RuntimeDisposition,
    cancel_worker: Option<RequestId>,
    start_worker: Option<GenerationRequest>,
    drop_candidate: Option<RequestId>,
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

    pub(crate) const fn cancel_worker(&self) -> Option<RequestId> {
        self.cancel_worker
    }

    pub(crate) fn take_start_worker(&mut self) -> Option<GenerationRequest> {
        self.start_worker.take()
    }

    pub(crate) const fn drop_candidate(&self) -> Option<RequestId> {
        self.drop_candidate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceInvalidation {
    BackingScaleAtlas,
    SurfaceRecovery,
    MaterialContract,
}

impl ResourceInvalidation {
    const fn mask(self) -> ResourceChangeMask {
        match self {
            Self::BackingScaleAtlas => ResourceChangeMask::BACKING_SCALE_ATLAS,
            Self::SurfaceRecovery => ResourceChangeMask::SURFACE_RECOVERY,
            Self::MaterialContract => ResourceChangeMask::MATERIAL_CONTRACT,
        }
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
pub(crate) enum RecoveryState {
    Operational,
    FallbackPending {
        device: DeviceEpoch,
        surface: SurfaceEpoch,
    },
    Recovering {
        device: DeviceEpoch,
        surface: SurfaceEpoch,
        request: RequestId,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CandidateRebaseError<E> {
    DroppedStale,
    Projection(E),
    StaleFrameProof,
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
            recovery: RecoveryState::Operational,
            lifecycle: RuntimeLifecycle::Running,
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
        self.ensure_running()?;
        if self.visibility == RuntimeVisibility::Hidden {
            return Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::InvalidValue,
            ));
        }
        validate_snapshot(&snapshot)?;
        let changes = classify_snapshot_changes(self.reconciler.snapshot(), &snapshot);
        self.prepare_with_changes(snapshot, changes, false)
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
            expected_pending_request: self.pending_request().map(GenerationRequest::request_id),
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
                == self.pending_request().map(GenerationRequest::request_id)
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
            };
            return Ok(self.queue_request(request));
        }

        if prepared.changes != SnapshotChangeSet::NONE {
            if let Some(active) = &mut self.active {
                active.version.applied = prepared.applied;
            }
            if let Some(pending) = &mut self.pending {
                pending.desired_source = prepared.applied;
                pending.desired_snapshot = Arc::clone(&prepared.snapshot);
                if let PendingPhase::Activating { commit_eligible, .. } = &mut pending.phase {
                    *commit_eligible = false;
                }
            }
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
        pending.phase = PendingPhase::Preparing;
        self.worker = WorkerState::Running(pending.request.request_id);
        effects.start_worker = Some(pending.request.duplicate_for_start());
        effects.disposition = RuntimeDisposition::GenerationStarted(pending.request.request_id);
    }

    fn queue_request(&mut self, request: GenerationRequest) -> RuntimeEffects {
        let mut effects =
            RuntimeEffects::new(RuntimeDisposition::GenerationQueued(request.request_id));
        let mut superseded_activation = None;
        if let Some(old) = self.pending.take() {
            match old.phase {
                PendingPhase::Activating { candidate, attempt, .. }
                | PendingPhase::SupersedingActivation { candidate, attempt } => {
                    superseded_activation = Some((candidate, attempt));
                }
                PendingPhase::Ready(candidate) => {
                    effects.drop_candidate = Some(candidate.request_id);
                }
                _ => {
                    if self.worker == WorkerState::Running(old.request.request_id) {
                        self.worker = WorkerState::Cancelling(old.request.request_id);
                        effects.cancel_worker = Some(old.request.request_id);
                    }
                }
            }
        }
        let phase = if let Some((candidate, attempt)) = superseded_activation {
            PendingPhase::SupersedingActivation { candidate, attempt }
        } else {
            PendingPhase::Queued
        };
        if let RecoveryState::Recovering { .. } = self.recovery {
            self.recovery = RecoveryState::Recovering {
                device: request.key.device,
                surface: request.surface,
                request: request.request_id,
            };
        }
        self.pending = Some(PendingGeneration {
            desired_surface: request.surface,
            desired_source: request.source,
            desired_snapshot: Arc::clone(&request.snapshot),
            accepted_snapshot: Arc::clone(&request.snapshot),
            request,
            phase,
        });
        self.start_pending_worker(&mut effects);
        effects
    }

    pub(crate) fn invalidate_resources(
        &mut self,
        invalidation: ResourceInvalidation,
    ) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        let prepared = self.prepare_with_changes(
            Arc::clone(self.reconciler.snapshot()),
            SnapshotChangeSet {
                resources: invalidation.mask(),
                ..SnapshotChangeSet::NONE
            },
            false,
        )?;
        let effects = self
            .commit_prepared(prepared)
            .map_err(|_| RuntimeError::SnapshotRejected(SnapshotRejection::InvalidValue))?;
        if matches!(invalidation, ResourceInvalidation::SurfaceRecovery) {
            if let Some(request) = self.pending_request() {
                self.recovery = RecoveryState::Recovering {
                    device: self.device_epoch,
                    surface: self.surface_epoch,
                    request: request.request_id,
                };
            }
        }
        Ok(effects)
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

    pub(crate) fn complete_candidate(
        &mut self,
        candidate: AcceptedGenerationCandidate,
    ) -> RuntimeEffects {
        let mut effects = RuntimeEffects::new(RuntimeDisposition::DroppedStale);
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            effects.drop_candidate = Some(candidate.request_id);
            return effects;
        }
        if self.worker == WorkerState::Cancelling(candidate.request_id) {
            self.worker = WorkerState::Idle;
            effects.drop_candidate = Some(candidate.request_id);
            self.start_pending_worker(&mut effects);
            return effects;
        }
        let Some(pending) = &mut self.pending else {
            effects.drop_candidate = Some(candidate.request_id);
            return effects;
        };
        if self.worker != WorkerState::Running(candidate.request_id)
            || pending.request.request_id != candidate.request_id
            || pending.request.key != candidate.key
            || pending.request.source != candidate.applied
            || !matches!(pending.phase, PendingPhase::Preparing)
        {
            effects.drop_candidate = Some(candidate.request_id);
            return effects;
        }
        let request_id = candidate.request_id;
        pending.phase = PendingPhase::Ready(candidate);
        self.worker = WorkerState::Idle;
        effects.disposition = RuntimeDisposition::CandidateReady(request_id);
        effects
    }

    pub(crate) fn rebase_ready_candidate<E>(
        &mut self,
        project: impl FnOnce(
            &Arc<CompanionSceneSnapshot>,
            SnapshotChangeSet,
            &mut AcceptedSceneState,
        ) -> Result<(), E>,
    ) -> Result<(), CandidateRebaseError<E>> {
        let Some(pending) = &mut self.pending else {
            return Err(CandidateRebaseError::DroppedStale);
        };
        let PendingPhase::Ready(candidate) = &pending.phase else {
            return Err(CandidateRebaseError::DroppedStale);
        };
        let changes =
            classify_snapshot_changes(&pending.accepted_snapshot, &pending.desired_snapshot);
        let mut accepted = candidate.accepted.clone();
        project(&pending.desired_snapshot, changes, &mut accepted)
            .map_err(CandidateRebaseError::Projection)?;
        if changes.has_frame() && accepted == candidate.accepted {
            return Err(CandidateRebaseError::StaleFrameProof);
        }
        pending.accepted_snapshot = Arc::clone(&pending.desired_snapshot);
        pending.phase = PendingPhase::Ready(AcceptedGenerationCandidate {
            request_id: pending.request.request_id,
            key: pending.request.key,
            applied: pending.desired_source,
            accepted,
        });
        Ok(())
    }

    pub(crate) fn begin_activation(&mut self) -> Result<ActivationAttempt, ActivationStartError> {
        if self.lifecycle == RuntimeLifecycle::Shutdown {
            return Err(ActivationStartError::Shutdown);
        }
        if self.visibility == RuntimeVisibility::Hidden {
            return Err(ActivationStartError::Hidden);
        }
        if matches!(self.recovery, RecoveryState::FallbackPending { .. }) {
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
                    effects.drop_candidate = Some(candidate.request_id);
                    pending.phase = PendingPhase::Queued;
                } else {
                    pending.phase = PendingPhase::Ready(candidate);
                }
                self.start_pending_worker(&mut effects);
                return effects;
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
                self.active = None;
            }
            self.pending = None;
            self.recovery = RecoveryState::FallbackPending {
                device: self.device_epoch,
                surface: self.surface_epoch,
            };
            effects.disposition =
                RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending);
            effects.drop_candidate = Some(candidate.request_id);
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
            effects.drop_candidate = Some(candidate.request_id);
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
                self.pending = None;
                effects.drop_candidate = Some(dropped);
                if matches!(self.recovery, RecoveryState::Recovering { .. }) {
                    self.recovery = RecoveryState::FallbackPending {
                        device: self.device_epoch,
                        surface: self.surface_epoch,
                    };
                }
                ActivationTransition::CandidateDestroyedRetainingActive
            }
            ActivationAttemptOutcome::PresentedClean { surface }
                if commit_eligible
                    && surface == self.surface_epoch
                    && attempt.surface == pending.desired_surface
                    && attempt.applied == pending.desired_source
                    && candidate.request_id == pending.request.request_id
                    && candidate.key == pending.request.key
                    && candidate.applied == pending.desired_source
                    && match self.recovery {
                        RecoveryState::Operational => true,
                        RecoveryState::Recovering {
                            device,
                            surface: recovery_surface,
                            request,
                        } => {
                            device == candidate.key.device
                                && recovery_surface == surface
                                && request == candidate.request_id
                        }
                        RecoveryState::FallbackPending { .. } => false,
                    } =>
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
        let next = SurfaceEpoch(increment(self.surface_epoch.0, CounterKind::SurfaceEpoch)?);
        increment(self.resource_generation.0, CounterKind::ResourceGeneration)?;
        increment(self.next_request_id.0, CounterKind::RequestId)?;
        self.surface_epoch = next;
        let effects = self.invalidate_resources(ResourceInvalidation::SurfaceRecovery)?;
        if let Some(pending) = &mut self.pending {
            pending.desired_surface = next;
            if let PendingPhase::Activating { commit_eligible, .. } = &mut pending.phase {
                *commit_eligible = false;
            }
        }
        Ok(effects)
    }

    pub(crate) fn acknowledge_device_recreated(&mut self) -> Result<RuntimeEffects, RuntimeError> {
        self.ensure_running()?;
        let next = DeviceEpoch(increment(self.device_epoch.0, CounterKind::DeviceEpoch)?);
        increment(self.resource_generation.0, CounterKind::ResourceGeneration)?;
        increment(self.next_request_id.0, CounterKind::RequestId)?;
        self.device_epoch = next;
        self.active = None;
        let effects = self.invalidate_resources(ResourceInvalidation::SurfaceRecovery)?;
        if let Some(request) = self.pending_request() {
            self.recovery = RecoveryState::Recovering {
                device: next,
                surface: self.surface_epoch,
                request: request.request_id,
            };
        }
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
        self.ensure_running()?;
        if self.visibility != RuntimeVisibility::Hidden {
            return Err(RuntimeError::SnapshotRejected(
                SnapshotRejection::InvalidValue,
            ));
        }
        let update = if let Some(prepared) = self.prepare_hidden_latest()? {
            prepared
        } else {
            self.prepare_with_changes(
                Arc::clone(self.reconciler.snapshot()),
                SnapshotChangeSet::NONE,
                false,
            )?
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
                    Some(candidate.request_id)
                }
                _ => None,
            };
        }
        self.pending = None;
        if let WorkerState::Running(id) = self.worker {
            self.worker = WorkerState::Cancelling(id);
            effects.cancel_worker = Some(id);
        }
        self.recovery = RecoveryState::FallbackPending { device, surface: self.surface_epoch };
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
            effects.cancel_worker = Some(id);
        }
        if let Some(pending) = &self.pending {
            effects.drop_candidate = match &pending.phase {
                PendingPhase::Ready(candidate)
                | PendingPhase::Activating { candidate, .. }
                | PendingPhase::SupersedingActivation { candidate, .. } => {
                    Some(candidate.request_id)
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

    fn accepted_state() -> crate::presentation::companion_scene::validate::AcceptedSceneState {
        let fixture = SceneFixture::valid();
        validate_full_generation(&fixture.template, &fixture.content, &fixture.frame).unwrap()
    }

    fn changed_accepted_frame() -> crate::presentation::companion_scene::validate::AcceptedSceneState
    {
        let mut fixture = SceneFixture::valid();
        fixture.frame.dim_amount = 0.25;
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

    #[derive(Default)]
    struct FakeWorkerDispatcher {
        live: Option<RequestId>,
        cancelling: Option<RequestId>,
        starts: Vec<RequestId>,
    }

    impl FakeWorkerDispatcher {
        fn apply(&mut self, effects: &mut RuntimeEffects) -> Option<GenerationRequest> {
            if let Some(cancel) = effects.cancel_worker() {
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

        let mut alternate =
            CompanionSceneRuntimeState::with_active(snapshot(), accepted_state()).unwrap();
        let first_snapshot = topology_update(alternate.snapshot(), Stage::S4);
        let mut first_effects = commit_snapshot(&mut alternate, first_snapshot);
        let mut dispatcher = FakeWorkerDispatcher::default();
        let first = dispatcher.apply(&mut first_effects).unwrap();
        let second_snapshot = topology_update(alternate.snapshot(), Stage::S5);
        let mut queued = commit_snapshot(&mut alternate, second_snapshot);
        dispatcher.apply(&mut queued);
        dispatcher.complete(first.request_id());
        let mut completion = alternate.complete_candidate(first.accept(accepted_state()));
        dispatcher.apply(&mut completion);
        assert_eq!(dispatcher.starts.len(), 2);
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
        latest.frame.pet_depth += 0.1;
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
    fn accepted_candidate_metadata_is_bound_by_runtime_authority() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        let candidate = request.accept(accepted_state());
        assert_eq!(candidate.request_id, request.request_id());
        assert_eq!(candidate.key, request.key());
        assert_eq!(candidate.applied, request.source());
        runtime.complete_candidate(candidate);

        let mut semantic = (**runtime.snapshot()).clone();
        semantic.content.mood = Mood::Content;
        commit_snapshot(&mut runtime, Arc::new(semantic));
        runtime
            .rebase_ready_candidate(|_, _, _| Ok::<_, ()>(()))
            .unwrap();
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
        runtime.complete_candidate(request.accept(accepted_state()));
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );
        runtime
            .rebase_ready_candidate(|_, _, accepted| {
                *accepted = accepted_state();
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(runtime.begin_activation().is_ok());
    }

    #[test]
    fn frame_rebase_cannot_relabel_unchanged_accepted_proof() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        runtime.complete_candidate(request.accept(accepted_state()));
        let mut frame = (**runtime.snapshot()).clone();
        frame.frame.pet_depth += 0.1;
        commit_snapshot(&mut runtime, Arc::new(frame));

        assert_eq!(
            runtime.rebase_ready_candidate(|_, changes, _| {
                assert!(changes.has_frame());
                Ok::<_, ()>(())
            }),
            Err(CandidateRebaseError::StaleFrameProof)
        );
        assert_eq!(
            runtime.begin_activation(),
            Err(ActivationStartError::CandidateNeedsRebase)
        );
        runtime
            .rebase_ready_candidate(|_, _, accepted| {
                *accepted = changed_accepted_frame();
                Ok::<_, ()>(())
            })
            .unwrap();
        assert!(runtime.begin_activation().is_ok());
    }

    #[test]
    fn repeated_rebase_diffs_from_last_accepted_projection() {
        let mut runtime = runtime();
        let next = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, next));
        runtime.complete_candidate(request.accept(accepted_state()));
        let original_depth = runtime.snapshot().frame.pet_depth;

        let mut forward = (**runtime.snapshot()).clone();
        forward.frame.pet_depth += 0.1;
        commit_snapshot(&mut runtime, Arc::new(forward));
        runtime
            .rebase_ready_candidate(|_, changes, accepted| {
                assert!(changes.has_frame());
                *accepted = changed_accepted_frame();
                Ok::<_, ()>(())
            })
            .unwrap();

        let mut backward = (**runtime.snapshot()).clone();
        backward.frame.pet_depth = original_depth;
        commit_snapshot(&mut runtime, Arc::new(backward));
        assert_eq!(
            runtime.rebase_ready_candidate(|_, changes, _| {
                assert!(changes.has_frame());
                Ok::<_, ()>(())
            }),
            Err(CandidateRebaseError::StaleFrameProof)
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
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        let request = take_start(runtime.acknowledge_surface_rebound().unwrap());
        assert_eq!(request.surface(), SurfaceEpoch(before.surface.0 + 1));
        assert_eq!(request.key().layout, before.generation.layout);
        assert_eq!(
            request.key().resources,
            ResourceGeneration(before.generation.resources.0 + 1)
        );
        assert_eq!(runtime.active_version(), Some(before));
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::RecoveryInProgress
        );
    }

    #[test]
    fn prepared_snapshot_shares_arc_and_change_masks_remain_fixed_size() {
        assert_eq!(std::mem::size_of::<SnapshotChangeSet>(), 32);
        let mut runtime = runtime();
        let mut frame = (**runtime.snapshot()).clone();
        frame.frame.pet_depth += 0.1;
        let next = Arc::new(frame);
        let prepared = runtime.prepare_snapshot(Arc::clone(&next)).unwrap();
        assert!(Arc::ptr_eq(prepared.snapshot(), &next));
        runtime.commit_prepared(prepared).unwrap();
        assert!(Arc::ptr_eq(runtime.snapshot(), &next));
        assert_eq!(Arc::strong_count(&next), 2);
    }

    #[test]
    fn layout_and_resource_generations_advance_only_for_their_own_lifetimes() {
        let mut runtime = runtime();
        let before = runtime.active_version().unwrap();
        let mut layout = (**runtime.snapshot()).clone();
        layout.topology.layout.width_points += 1.0;
        let request = take_start(commit_snapshot(&mut runtime, Arc::new(layout)));
        assert_eq!(
            request.key().layout,
            LayoutGeneration(before.generation.layout.0 + 1)
        );
        assert_eq!(request.key().resources, before.generation.resources);

        let mut mixed = (**runtime.snapshot()).clone();
        mixed.topology.layout.height_points += 1.0;
        let mixed = Arc::new(mixed);
        let mut changes = classify_snapshot_changes(runtime.snapshot(), &mixed);
        changes.resources = ResourceChangeMask::MATERIAL_CONTRACT;
        let prepared = runtime.prepare_with_changes(mixed, changes, false).unwrap();
        let effects = runtime.commit_prepared(prepared).unwrap();
        let request = runtime.pending_request().unwrap();
        assert_eq!(
            request.key().layout,
            LayoutGeneration(before.generation.layout.0 + 2)
        );
        assert_eq!(
            request.key().resources,
            ResourceGeneration(before.generation.resources.0 + 1)
        );
        assert!(effects.start_worker.is_none());
        assert!(effects.cancel_worker().is_some());
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
            let runtime =
                CompanionSceneRuntimeState::with_active(Arc::clone(&base), accepted_state())
                    .unwrap();
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
        frame.frame.pet_depth += 0.1;
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
        assert_eq!(
            surface_runtime.acknowledge_surface_rebound(),
            Err(RuntimeError::CounterOverflow(CounterKind::SurfaceEpoch))
        );
        let mut device_runtime = runtime();
        device_runtime.device_epoch = DeviceEpoch(u64::MAX);
        assert_eq!(
            device_runtime.acknowledge_device_recreated(),
            Err(RuntimeError::CounterOverflow(CounterKind::DeviceEpoch))
        );

        let mut activation_runtime = runtime();
        let topology = topology_update(activation_runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut activation_runtime, topology));
        activation_runtime.complete_candidate(request.accept(accepted_state()));
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
        runtime.complete_candidate(request.accept(accepted_state()));
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
        runtime.complete_candidate(request.accept(accepted_state()));
        let attempt = runtime.begin_activation().unwrap();
        runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(runtime.recovery, RecoveryState::Operational);
        assert!(runtime.capture_lease().is_ok());
    }

    #[test]
    fn superseding_activation_drops_old_candidate_and_starts_exact_new_request() {
        let mut runtime = runtime();
        let first_snapshot = topology_update(runtime.snapshot(), Stage::S4);
        let first = take_start(commit_snapshot(&mut runtime, first_snapshot));
        runtime.complete_candidate(first.accept(accepted_state()));
        let attempt = runtime.begin_activation().unwrap();

        let second_snapshot = topology_update(runtime.snapshot(), Stage::S5);
        let second_effects = commit_snapshot(&mut runtime, second_snapshot);
        let second_id = runtime.pending_request().unwrap().request_id();
        assert!(second_effects.start_worker.is_none());
        assert_eq!(
            runtime.capture_lease().unwrap_err(),
            CaptureDefer::ActivationInProgress
        );

        let mut completion = runtime.finish_activation(
            attempt,
            ActivationAttemptOutcome::PresentedClean { surface: attempt.surface },
        );
        assert_eq!(completion.drop_candidate(), Some(first.request_id()));
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
        GenerationRequest,
        ActivationAttempt,
    ) {
        let mut runtime = runtime();
        let topology = topology_update(runtime.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut runtime, topology));
        runtime.complete_candidate(request.accept(accepted_state()));
        let attempt = runtime.begin_activation().unwrap();
        (runtime, request, attempt)
    }

    #[test]
    fn superseding_activation_late_rejection_and_current_fatal_remain_actionable() {
        let (mut rejected, first, attempt) = runtime_with_activation();
        let next = topology_update(rejected.snapshot(), Stage::S5);
        commit_snapshot(&mut rejected, next);
        let replacement = rejected.pending_request().unwrap().request_id();
        let mut effects = rejected.finish_activation(
            attempt,
            ActivationAttemptOutcome::CandidateRejected(CandidateFailure::Resource),
        );
        assert_eq!(effects.drop_candidate(), Some(first.request_id()));
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
        let effects = fatal.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::DeviceLost),
        );
        assert_eq!(effects.drop_candidate(), Some(first.request_id()));
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
        let recovery = surface.acknowledge_surface_rebound().unwrap();
        let replacement = surface.pending_request().unwrap().request_id();
        assert!(recovery.start_worker.is_none());
        let mut late = surface.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
        );
        assert_eq!(late.drop_candidate(), Some(old_surface.request_id()));
        assert_eq!(
            late.take_start_worker().map(|request| request.request_id()),
            Some(replacement)
        );
        assert!(matches!(surface.recovery, RecoveryState::Recovering { .. }));
        assert!(surface.active_version().is_some());

        let (mut device, old_device, attempt) = runtime_with_activation();
        device.acknowledge_device_recreated().unwrap();
        let replacement = device.pending_request().unwrap().request_id();
        let mut late = device.finish_activation(
            attempt,
            ActivationAttemptOutcome::Fatal(EpochFailure::DeviceLost),
        );
        assert_eq!(late.drop_candidate(), Some(old_device.request_id()));
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
        runtime.set_hidden();
        let device = runtime.acknowledge_device_recreated().unwrap();
        assert!(device.start_worker.is_none());
        assert_eq!(runtime.device_epoch, DeviceEpoch(2));
        assert!(matches!(runtime.recovery, RecoveryState::Recovering { .. }));

        let resource = runtime
            .invalidate_resources(ResourceInvalidation::BackingScaleAtlas)
            .unwrap();
        assert!(resource.start_worker.is_none());
        assert_eq!(resource.cancel_worker(), None);
        let expected = runtime.pending_request().unwrap().request_id();
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
            runtime.complete_candidate(initial.accept(accepted_state()));
            let attempt = runtime.begin_activation().unwrap();
            runtime.finish_activation(
                attempt,
                ActivationAttemptOutcome::Fatal(EpochFailure::SurfaceLost),
            );
            let recovery = take_start(runtime.acknowledge_surface_rebound().unwrap());
            let newest_snapshot = topology_update(runtime.snapshot(), Stage::S5);
            let superseding = commit_snapshot(&mut runtime, newest_snapshot);
            assert_eq!(superseding.cancel_worker(), Some(recovery.request_id()));
            let newest = runtime.pending_request().unwrap().request_id();
            assert!(matches!(
                runtime.recovery,
                RecoveryState::Recovering { request, .. } if request == newest
            ));

            let mut start = if acknowledge_first {
                runtime.acknowledge_worker_cancelled(recovery.request_id())
            } else {
                runtime.complete_candidate(recovery.accept(accepted_state()))
            };
            let newest_request = start.take_start_worker().unwrap();
            assert_eq!(newest_request.request_id(), newest);
            runtime.complete_candidate(newest_request.accept(accepted_state()));
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
            next.frame.pet_depth = 0.5 + step as f32 * 0.01;
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
        assert!(effects.cancel_worker().is_none());
    }

    #[test]
    fn dropped_reveal_projection_retains_hidden_snapshot_and_publishes_nothing() {
        let mut runtime = runtime();
        runtime.set_hidden();
        let before = runtime.active_version();
        let before_snapshot = Arc::clone(runtime.snapshot());
        let mut hidden = (*before_snapshot).clone();
        hidden.frame.pet_depth += 0.1;
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
        late.frame.pet_depth += 0.1;
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
        ready.complete_candidate(request.accept(accepted_state()));
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
        runtime.complete_candidate(request.accept(accepted_state()));
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
    fn delayed_errors_are_device_scoped_and_cannot_resurrect_state() {
        let mut preparing = runtime();
        let topology = topology_update(preparing.snapshot(), Stage::S4);
        let request = take_start(commit_snapshot(&mut preparing, topology));
        let effects = preparing.observe_delayed_gpu_error(request.key().device);
        assert_eq!(effects.cancel_worker(), Some(request.request_id()));
        assert!(preparing.active_version().is_none());
        let completion = preparing.complete_candidate(request.accept(accepted_state()));
        assert_eq!(completion.drop_candidate(), Some(request.request_id()));
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
        let replacement = take_start(retired.acknowledge_device_recreated().unwrap());
        retired.complete_candidate(replacement.accept(accepted_state()));
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
            runtime.invalidate_resources(ResourceInvalidation::SurfaceRecovery),
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
