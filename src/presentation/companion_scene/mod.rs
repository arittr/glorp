pub mod contract;
pub(crate) mod input;
#[allow(dead_code)]
pub(crate) mod runtime;
pub mod scene;
pub mod validate;

pub use runtime::{
    AppliedRevisions, DeviceEpoch, FrameRevision, LayoutGeneration, ResourceGeneration,
    SceneGenerationKey, SceneVersion, SemanticRevision, SurfaceEpoch,
};
pub use scene::{MAX_ROUND_TANK_INHABITANTS as MAX_VISIBLE_TANK_INHABITANTS, MAX_VISIBLE_PROPS};

use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::generation::Species;
use crate::presentation::privacy::PrivacyProjection;

pub const COMPANION_SCENE_SCHEMA_VERSION: u16 = 2;
pub const COMPANION_RENDERER_SCHEMA_VERSION: u16 = 2;
pub const PET_LATTICE_WIDTH: u16 = 13;
pub const PET_LATTICE_HEIGHT: u16 = 10;
pub const PET_LATTICE_SLOTS: u16 = scene::MAX_PET_ART_SLOTS as u16;
const _: [(); scene::MAX_PET_ART_SLOTS] =
    [(); PET_LATTICE_WIDTH as usize * PET_LATTICE_HEIGHT as usize];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionSceneProjectionError {
    PetArtTooTall {
        row_count: usize,
    },
    PetArtTooWide {
        line_index: usize,
        char_count: usize,
    },
    DisallowedPetGlyph {
        line_index: usize,
        char_index: usize,
    },
    InvalidPetRoleSpan {
        span_index: usize,
        line_index: usize,
        start_char: usize,
        end_char: usize,
        source_char_count: usize,
    },
    RoomGlyphCapacity {
        count: usize,
    },
    InvalidRoomGlyphColor,
    InvalidProjectionGrid,
    InvalidProjectionLayout,
}

impl std::fmt::Display for CompanionSceneProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PetArtTooTall { row_count } => {
                write!(f, "pet art has {row_count} rows; maximum is {PET_LATTICE_HEIGHT}")
            }
            Self::PetArtTooWide { line_index, char_count } => write!(
                f,
                "pet art row {line_index} has {char_count} scalar cells; maximum is {PET_LATTICE_WIDTH}"
            ),
            Self::DisallowedPetGlyph { line_index, char_index } => write!(
                f,
                "pet art row {line_index} cell {char_index} is outside the declared repertoire"
            ),
            Self::InvalidPetRoleSpan {
                span_index,
                line_index,
                ..
            } => write!(
                f,
                "pet role span {span_index} is invalid for source row {line_index}"
            ),
            Self::RoomGlyphCapacity { count } => {
                write!(f, "room emitted {count} glyphs; maximum is {}", scene::MAX_ROOM_GLYPH_SLOTS)
            }
            Self::InvalidRoomGlyphColor => {
                write!(f, "room emitted a non-RGB color in truecolor projection")
            }
            Self::InvalidProjectionGrid => write!(f, "companion projection grid is empty"),
            Self::InvalidProjectionLayout => {
                write!(f, "companion projection layout must be finite and positive")
            }
        }
    }
}

impl std::error::Error for CompanionSceneProjectionError {}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionLogicalLayout {
    pub width_points: f32,
    pub height_points: f32,
}

/// Exact projection from the snapshot's top-left cell lattice into the
/// renderer-neutral, Y-up logical point space used by the retained scene.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionGlyphGrid {
    pub columns: u16,
    pub rows: u16,
    pub y_up_origin_points: [f32; 2],
    pub cell_extent_points: [f32; 2],
    pub scale: LogicalGlyphScale,
    pub anchor: LogicalGlyphAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogicalGlyphScale {
    OneCell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogicalGlyphAnchor {
    CellBottomLeft,
}

impl CompanionLogicalLayout {
    pub const fn round(width_points: f32, height_points: f32) -> Self {
        Self { width_points, height_points }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionProjectionClock {
    pub wall_time: time::OffsetDateTime,
    pub elapsed_ms: u64,
}

impl CompanionProjectionClock {
    pub const fn new(wall_time: time::OffsetDateTime, elapsed_ms: u64) -> Self {
        Self { wall_time, elapsed_ms }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionSceneProjectionInput {
    pub clock: CompanionProjectionClock,
    pub layout: CompanionLogicalLayout,
    pub grid_columns: u16,
    pub grid_rows: u16,
    pub depth_override: Option<f32>,
    pub motion_clearance: crate::round::motion::CompanionMotionClearance,
}

impl CompanionSceneProjectionInput {
    pub const fn round(
        clock: CompanionProjectionClock,
        layout: CompanionLogicalLayout,
        grid_columns: u16,
        grid_rows: u16,
        motion_clearance: crate::round::motion::CompanionMotionClearance,
    ) -> Self {
        Self {
            clock,
            layout,
            grid_columns,
            grid_rows,
            depth_override: None,
            motion_clearance,
        }
    }

    #[doc(hidden)]
    pub const fn with_depth_override(mut self, depth: f32) -> Self {
        self.depth_override = Some(depth);
        self
    }

    pub const fn motion_viewport(self) -> crate::round::motion::RoundCompanionMotionViewport {
        crate::round::motion::RoundCompanionMotionViewport {
            grid_columns: self.grid_columns,
            grid_rows: self.grid_rows,
            width_points: self.layout.width_points,
            height_points: self.layout.height_points,
            clearance: self.motion_clearance,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CompanionSceneSnapshot {
    pub schema_version: u16,
    pub privacy: PrivacyProjection,
    pub topology: TopologySnapshot,
    pub content: ContentSnapshot,
    pub frame: FrameSnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TopologySnapshot {
    pub layout: CompanionLogicalLayout,
    pub glyph_grid: CompanionGlyphGrid,
    pub pet: PetTopologySnapshot,
    pub room: RoomTopologySnapshot,
    pub visible_props: Vec<PropTopologySnapshot>,
    pub visible_tank_inhabitants: Vec<TankTopologySnapshot>,
    pub renderer_schema: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PetTopologySnapshot {
    pub species: Species,
    pub stage: Stage,
    pub lattice: PetLatticeSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PetLatticeSnapshot {
    pub identity: &'static str,
    pub width: u16,
    pub height: u16,
    pub slot_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RoomTopologySnapshot {
    pub primary_biome: &'static str,
    pub secondary_biome: Option<&'static str>,
    pub species_dialect: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PropTopologySnapshot {
    pub catalog_id: &'static str,
    pub stable_order: u8,
    pub zone: PropZoneSnapshot,
    pub authored_depth: AuthoredDepthSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TankTopologySnapshot {
    pub catalog_id: &'static str,
    pub stable_order: u8,
    pub route: TankRouteSnapshot,
    pub authored_depth: AuthoredDepthSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PropZoneSnapshot {
    FloorLeft,
    FloorMid,
    FloorRight,
    WallLeft,
    WallRight,
    AirLeft,
    AirMid,
    AirRight,
    Ceiling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TankRouteSnapshot {
    CrossTankSwimmer,
    LowerLaneResident,
    GlassResident,
    RimResident,
    LowerEdgeResident,
    HostCombo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthoredDepthSnapshot {
    Background,
    BehindPet,
    Foreground,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ContentSnapshot {
    pub mood: Mood,
    pub room_weather: &'static str,
    pub pet_lines: Vec<String>,
    pub pet_roles: Vec<PetRoleSpanSnapshot>,
    pub room_glyphs: Vec<RoomGlyphContentSnapshot>,
    pub palette: PaletteSnapshot,
    pub prop_animation_states: Vec<PropAnimationSnapshot>,
    pub tank_animation_states: Vec<TankAnimationSnapshot>,
    pub ambient_semantics: Vec<AmbientSemanticSnapshot>,
    pub hud_glyphs: Vec<HudGlyphSnapshot>,
    pub activity_pulse_age_ms: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PropAnimationSnapshot {
    pub catalog_id: &'static str,
    pub stable_order: u8,
    pub kind: PropAnimationKindSnapshot,
    pub sprite_phase: Option<u8>,
    pub twinkle_active: Option<bool>,
    pub motion_phase: Option<u8>,
    pub chest_lid_open: Option<bool>,
    pub bloom_active: Option<bool>,
    pub origin_points: [f32; 2],
}

impl PropAnimationSnapshot {
    pub const fn is_static(&self) -> bool {
        matches!(self.kind, PropAnimationKindSnapshot::Static)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PropAnimationKindSnapshot {
    Static,
    Animated,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TankAnimationSnapshot {
    pub catalog_id: &'static str,
    pub stable_order: u8,
    pub route: TankRouteSnapshot,
    pub visible: bool,
    pub origin_col: u16,
    pub origin_row: u16,
    pub origin_points: [f32; 2],
    pub side: Option<TankSideSnapshot>,
    pub layer: TankLayerSnapshot,
    pub sprite_variant: u8,
    pub visible_rows: u8,
    pub anemone_morph: Option<u8>,
    pub cadence_ms: u16,
    pub calm: bool,
    pub cells: Vec<TankCellSnapshot>,
    pub bounds: Option<TankBoundsSnapshot>,
    pub bounds_points: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct TankCellSnapshot {
    pub col: u16,
    pub row: u16,
    pub glyph: char,
    pub layer: TankLayerSnapshot,
    pub position_points: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AmbientSemanticKindSnapshot {
    Weather,
    ActivityPulse,
    Mote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RoomGlyphContentSnapshot {
    pub slot: u8,
    pub glyph: char,
    pub color_srgb8: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AmbientSemanticSnapshot {
    pub slot: u8,
    pub kind: Option<AmbientSemanticKindSnapshot>,
    pub glyph: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct HudGlyphSnapshot {
    pub slot: u8,
    pub glyph: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TankBoundsSnapshot {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TankSideSnapshot {
    Left,
    Right,
    Rear,
    Front,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TankLayerSnapshot {
    Behind,
    Foreground,
    BehindAnchorForegroundHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PetRoleSpanSnapshot {
    pub line_index: u16,
    pub start_char: u16,
    pub end_char: u16,
    pub role: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PaletteSnapshot {
    pub body: [u8; 3],
    pub body_glow: [u8; 3],
    pub eye: [u8; 3],
    pub mouth: [u8; 3],
    pub accent: [u8; 3],
    pub pattern: [u8; 3],
    pub particle: [u8; 3],
    pub corruption: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FrameSnapshot {
    pub elapsed_ms: u64,
    pub pet_anchor_points: [f32; 2],
    pub pet_depth: f32,
    pub facing: i8,
    pub breath_offset_y_points: f32,
    pub bob_offset_y_points: f32,
    pub asleep: bool,
    pub helper_trouble: bool,
    pub gauges: [GaugeLevelSnapshot; 4],
    pub dim_amount: f32,
    pub hud_lines: [String; 3],
    pub room_glyphs: Vec<RoomGlyphFrameSnapshot>,
    pub ambient_instances: Vec<AmbientFrameSnapshot>,
    pub hud_instances: Vec<HudFrameSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct RoomGlyphFrameSnapshot {
    pub slot: u8,
    pub visible: bool,
    pub grid_cell: [u16; 2],
    pub position_points: [f32; 2],
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AmbientFrameSnapshot {
    pub slot: u8,
    pub visible: bool,
    pub position_points: [f32; 2],
    pub opacity: f32,
}

pub(crate) fn canonical_activity_pulse_state(snapshot: &CompanionSceneSnapshot) -> (bool, f32) {
    let Some(semantic) = snapshot
        .content
        .ambient_semantics
        .iter()
        .find(|semantic| semantic.kind == Some(AmbientSemanticKindSnapshot::ActivityPulse))
    else {
        return (false, 0.0);
    };
    snapshot
        .frame
        .ambient_instances
        .iter()
        .find(|frame| frame.slot == semantic.slot)
        .filter(|frame| frame.visible)
        .map_or((false, 0.0), |frame| (true, frame.opacity))
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct HudFrameSnapshot {
    pub slot: u8,
    pub visible: bool,
    pub position_points: [f32; 2],
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GaugeLevelSnapshot {
    Empty,
    Low,
    Medium,
    High,
    Full,
}

impl GaugeLevelSnapshot {
    pub(crate) fn from_fraction(value: f64) -> Self {
        if value == 0.0 {
            Self::Empty
        } else if value < 0.25 {
            Self::Low
        } else if value < 0.5 {
            Self::Medium
        } else if value < 1.0 {
            Self::High
        } else {
            Self::Full
        }
    }
}
