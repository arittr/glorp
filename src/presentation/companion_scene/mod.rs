pub(crate) mod input;

use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::generation::Species;
use crate::presentation::privacy::PrivacyProjection;

pub const COMPANION_SCENE_SCHEMA_VERSION: u16 = 1;
pub const COMPANION_RENDERER_SCHEMA_VERSION: u16 = 1;
pub const MAX_VISIBLE_PROPS: usize = 10;
pub const MAX_VISIBLE_TANK_INHABITANTS: usize = 2;
pub const PET_LATTICE_WIDTH: u16 = 13;
pub const PET_LATTICE_HEIGHT: u16 = 10;
pub const PET_LATTICE_SLOTS: u16 = PET_LATTICE_WIDTH * PET_LATTICE_HEIGHT;

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
        }
    }
}

impl std::error::Error for CompanionSceneProjectionError {}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionLogicalLayout {
    pub width_points: f32,
    pub height_points: f32,
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
    pub palette: PaletteSnapshot,
    pub prop_animation_states: Vec<PropAnimationSnapshot>,
    pub tank_animation_states: Vec<TankAnimationSnapshot>,
    pub activity_pulse_age_ms: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PropAnimationSnapshot {
    pub catalog_id: &'static str,
    pub stable_order: u8,
    pub kind: PropAnimationKindSnapshot,
    pub sprite_phase: Option<u8>,
    pub twinkle_active: Option<bool>,
    pub motion_phase: Option<u8>,
    pub chest_lid_open: Option<bool>,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TankAnimationSnapshot {
    pub catalog_id: &'static str,
    pub stable_order: u8,
    pub route: TankRouteSnapshot,
    pub visible: bool,
    pub origin_col: u16,
    pub origin_row: u16,
    pub side: Option<TankSideSnapshot>,
    pub layer: TankLayerSnapshot,
    pub sprite_variant: u8,
    pub visible_rows: u8,
    pub anemone_morph: Option<u8>,
    pub cadence_ms: u16,
    pub calm: bool,
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
    pub breath_offset_y_cells: u8,
    pub bob_offset_y_cells: f32,
    pub asleep: bool,
    pub helper_trouble: bool,
    pub gauges: [GaugeLevelSnapshot; 4],
    pub dim_amount: f32,
    pub hud_lines: [String; 3],
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

#[cfg(test)]
mod tests {
    use super::{
        CompanionLogicalLayout, CompanionProjectionClock, CompanionSceneProjectionError,
        CompanionSceneProjectionInput, CompanionSceneSnapshot, PropAnimationSnapshot,
        PET_LATTICE_HEIGHT, PET_LATTICE_WIDTH,
    };
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::{generate_pet, Species};
    use crate::pet::render::{render_pet, AnimationFrame, PaletteRoleName, StyledSegment};
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    fn fixture_with_real_pet_art() -> WatchViewModel {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        let rendered = render_pet(
            &generate_pet("companion-scene-real-art").with_species(Species::Fuzz),
            Stage::S3,
            Mood::Content,
            AnimationFrame::default(),
        );
        vm.pet_render.generated_species = Species::Fuzz;
        vm.pet_render.stage = Stage::S3;
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    fn project_snapshot(
        vm: &WatchViewModel,
        wall_time: time::OffsetDateTime,
        layout: CompanionLogicalLayout,
    ) -> Result<CompanionSceneSnapshot, CompanionSceneProjectionError> {
        CompanionSceneSnapshot::project_with_input(
            vm,
            CompanionSceneProjectionInput::round(
                CompanionProjectionClock::new(wall_time, 0),
                layout,
                44,
                18,
                crate::round::scene::current_round_motion_clearance(18),
            ),
        )
    }

    fn prop_animation_state<'a>(
        snapshot: &'a CompanionSceneSnapshot,
        catalog_id: &str,
    ) -> &'a PropAnimationSnapshot {
        snapshot
            .content
            .prop_animation_states
            .iter()
            .find(|state| state.catalog_id == catalog_id)
            .expect("visible prop state")
    }

    #[test]
    fn projection_is_one_privacy_aware_snapshot() {
        let vm = fixture_with_real_pet_art();
        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("project canonical generated pet art");

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(
            snapshot.topology.pet.species,
            vm.pet_render.generated_species
        );
        assert_eq!(snapshot.topology.pet.stage, vm.pet_render.stage);
        assert_eq!(snapshot.topology.pet.lattice.slot_count, 130);
        assert!(snapshot.topology.visible_props.len() <= 10);
        assert!(snapshot.topology.visible_tank_inhabitants.len() <= 2);
        assert!(!snapshot.privacy.source_names_visible);
        assert!(!snapshot.privacy.file_paths_visible);
        assert_eq!(
            snapshot.content.pet_lines.len(),
            usize::from(PET_LATTICE_HEIGHT)
        );
        assert!(snapshot
            .content
            .pet_lines
            .iter()
            .all(|line| line.chars().count() == usize::from(PET_LATTICE_WIDTH)));
    }

    #[test]
    fn projection_serialization_excludes_every_private_input_sentinel() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_render.seed = "sentinel-private-seed-7ea431".to_string();
        vm.pet_name = "sentinel-private-pet-name-9b15".to_string();
        vm.source_breakdown[0].name = "sentinel-private-source-name-6d72".to_string();
        vm.source_breakdown[0].display_name = "sentinel-private-display-name-5c20".to_string();
        vm.source_health[0].diagnostic_message =
            Some("sentinel-private-diagnostic-a03f".to_string());
        vm.helper_status = "sentinel-private-helper-status-284c".to_string();
        vm.errors = vec!["sentinel-private-error-b884 /private/path/scene.json".to_string()];
        vm.current_speech = Some("sentinel-private-speech-token-d37a".to_string());
        vm.today_snapshot_reason = Some("sk-sentinel-private-auth-token-e19c".to_string());
        vm.today_effective_tokens = 987_654_321.25;
        vm.pet_art[0] = "private/path".to_string();
        vm.habitat
            .earned_props
            .push(crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("sentinel-private-unknown-prop-725f"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Accent,
                display_priority: i16::MAX,
                source: crate::storage::state::HabitatPropSource::ProviderFirstUse {
                    provider_surface: "sentinel-private-provider-source-42e6".to_string(),
                },
            });
        vm.habitat
            .earned_inhabitants
            .push(crate::tui::view_model::EarnedTankInhabitantView {
                id: crate::storage::state::TankInhabitantId::new(
                    "sentinel-private-unknown-inhabitant-f792",
                ),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                unlock_age_days: 0,
                kind: crate::game::habitat::TankInhabitantKind::Swimmer,
                source: crate::storage::state::TankInhabitantSource::PetAgeThreshold {
                    threshold_days: 0,
                },
            });

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        );
        let error_message = snapshot
            .as_ref()
            .expect_err("private pet text must fail closed")
            .to_string();
        assert!(!error_message.contains("private/path"));
        assert_eq!(
            snapshot,
            Err(CompanionSceneProjectionError::DisallowedPetGlyph { line_index: 0, char_index: 0 })
        );

        vm.pet_art = fixture_with_real_pet_art().pet_art;
        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("project sanitized canonical pet art");
        let json = serde_json::to_string(&snapshot).expect("serialize scene snapshot");

        for sentinel in [
            "sentinel-private-seed-7ea431",
            "sentinel-private-pet-name-9b15",
            "sentinel-private-source-name-6d72",
            "sentinel-private-display-name-5c20",
            "sentinel-private-diagnostic-a03f",
            "sentinel-private-helper-status-284c",
            "sentinel-private-error-b884",
            "/private/path/scene.json",
            "sentinel-private-speech-token-d37a",
            "sk-sentinel-private-auth-token-e19c",
            "sentinel-private-unknown-prop-725f",
            "sentinel-private-provider-source-42e6",
            "sentinel-private-unknown-inhabitant-f792",
            "987654321.25",
        ] {
            assert!(
                !json.contains(sentinel),
                "snapshot leaked {sentinel}: {json}"
            );
        }
        assert!(
            !json.contains("\"seed\""),
            "snapshot exposed a seed field: {json}"
        );
    }

    #[test]
    fn privacy_projection_redacts_formatted_live_hud_telemetry_everywhere() {
        let mut vm = fixture_with_real_pet_art();
        vm.today_effective_tokens = 842_000_000.0;
        vm.daily_comparison.fraction_of_yesterday = Some(0.94);
        vm.rate_momentum.pulse.current_tokens = 31_000_000.0;
        let live = crate::round::hud::companion_hud_text(
            vm.today_effective_tokens,
            vm.daily_comparison.fraction_of_yesterday,
            vm.rate_momentum.pulse.current_tokens,
        );

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("privacy-aware projection");
        let json = serde_json::to_string(&snapshot).expect("serialize privacy projection");
        let debug = format!("{snapshot:?}");
        let mut invalid = vm.clone();
        invalid.pet_art[0] = "private/path".to_string();
        let error = project_snapshot(
            &invalid,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect_err("invalid private art fails closed");
        let error_text = format!("{error} {error:?}");

        assert!(!snapshot.privacy.exact_counts_visible);
        assert!(!snapshot.privacy.source_names_visible);
        assert_eq!(snapshot.frame.hud_lines, ["review", "privacy", "redacted"]);
        for private in [live.today_total, live.daily_percent, live.pace] {
            assert!(!json.contains(&private), "JSON leaked {private}: {json}");
            assert!(!debug.contains(&private), "Debug leaked {private}: {debug}");
            assert!(
                !error_text.contains(&private),
                "public error text leaked {private}: {error_text}"
            );
        }
    }

    #[test]
    fn privacy_projection_quantizes_live_gauges_instead_of_serializing_exact_ratios() {
        let mut vm = fixture_with_real_pet_art();
        vm.progress.fraction = 0.432_109;
        vm.daily_comparison.fraction_of_yesterday = Some(0.943_217);
        vm.rate_momentum.pulse.current_tokens = 31_234_567.0;
        let exact_daily =
            crate::round::hud::daily_fraction_for_gauge(vm.daily_comparison.fraction_of_yesterday)
                as f32;
        let exact_pace =
            crate::round::hud::companion_pace_fraction(vm.rate_momentum.pulse.current_tokens)
                as f32;

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("privacy-aware gauges");
        let json = serde_json::to_string(&snapshot).expect("serialize privacy-aware gauges");

        for exact in [vm.progress.fraction, exact_daily, exact_pace] {
            let exact = serde_json::to_string(&exact).unwrap();
            assert!(
                !json.contains(&exact),
                "serialized exact live gauge {exact}: {json}"
            );
        }
    }

    #[test]
    fn seed_changes_do_not_change_serialized_pet_placement() {
        let mut first = fixture_with_real_pet_art();
        first.pet_render.seed = "sentinel-seed-alpha".to_string();
        let mut second = first.clone();
        second.pet_render.seed = "sentinel-seed-beta".to_string();
        let layout = CompanionLogicalLayout::round(360.0, 360.0);
        let now = datetime!(2026-07-11 12:00 UTC);

        let first = project_snapshot(&first, now, layout).expect("first projection");
        let second = project_snapshot(&second, now, layout).expect("second projection");

        assert_eq!(
            first.frame.pet_anchor_points,
            second.frame.pet_anchor_points
        );
        assert_eq!(first.frame.pet_depth, second.frame.pet_depth);
        let json = serde_json::to_string(&(first, second)).expect("serialize snapshots");
        assert!(!json.contains("sentinel-seed-alpha"));
        assert!(!json.contains("sentinel-seed-beta"));
    }

    #[test]
    fn topology_order_and_fixed_limits_are_deterministic() {
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(
            120,
            time::macros::date!(2026 - 07 - 11),
        );
        let canonical = fixture_with_real_pet_art();
        vm.pet_art = canonical.pet_art;
        vm.pet_spans = canonical.pet_spans;
        vm.habitat.earned_props = crate::game::habitat::HABITAT_PROP_CATALOG
            .iter()
            .map(|spec| crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new(spec.id),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: spec.kind,
                display_priority: spec.display_priority,
                source: crate::storage::state::HabitatPropSource::LifetimeTokens { threshold: 1.0 },
            })
            .collect();
        let now = datetime!(2026-07-11 12:00 UTC);
        let layout = CompanionLogicalLayout::round(360.0, 360.0);

        let first = project_snapshot(&vm, now, layout).expect("first projection");
        let second = project_snapshot(&vm, now, layout).expect("second projection");

        assert_eq!(first, second);
        assert_eq!(first.topology.visible_props.len(), 10);
        assert_eq!(first.topology.visible_tank_inhabitants.len(), 2);
        assert!(first
            .topology
            .visible_props
            .iter()
            .enumerate()
            .all(|(index, prop)| usize::from(prop.stable_order) == index));
        assert!(first
            .topology
            .visible_tank_inhabitants
            .iter()
            .enumerate()
            .all(|(index, inhabitant)| usize::from(inhabitant.stable_order) == index));
    }

    #[test]
    fn short_pet_art_is_padded_to_the_fixed_lattice() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_art = vec!["✦".to_string(), "".to_string()];
        vm.pet_spans = vec![StyledSegment {
            line: 0,
            start: 0,
            end: 1,
            role: PaletteRoleName::Particle,
        }];

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("short declared art is padded");

        assert_eq!(snapshot.content.pet_lines.len(), 10);
        assert_eq!(snapshot.content.pet_lines[0].chars().count(), 13);
        assert!(snapshot.content.pet_lines[0].starts_with('✦'));
        assert!(snapshot.content.pet_lines[2..]
            .iter()
            .all(|line| line == "             "));
    }

    #[test]
    fn oversized_pet_art_fails_closed() {
        let mut too_many_rows = fixture_with_real_pet_art();
        too_many_rows.pet_art = vec!["".to_string(); 11];
        assert_eq!(
            project_snapshot(
                &too_many_rows,
                datetime!(2026-07-11 12:00 UTC),
                CompanionLogicalLayout::round(360.0, 360.0),
            ),
            Err(CompanionSceneProjectionError::PetArtTooTall { row_count: 11 })
        );

        let mut too_many_columns = fixture_with_real_pet_art();
        too_many_columns.pet_art = vec!["✦".repeat(14)];
        assert_eq!(
            project_snapshot(
                &too_many_columns,
                datetime!(2026-07-11 12:00 UTC),
                CompanionLogicalLayout::round(360.0, 360.0),
            ),
            Err(CompanionSceneProjectionError::PetArtTooWide { line_index: 0, char_count: 14 })
        );
    }

    #[test]
    fn invalid_pet_role_spans_fail_closed_against_unpadded_rows() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_art = vec!["✦".to_string()];
        vm.pet_spans = vec![StyledSegment {
            line: 0,
            start: 0,
            end: 2,
            role: PaletteRoleName::Particle,
        }];

        assert_eq!(
            project_snapshot(
                &vm,
                datetime!(2026-07-11 12:00 UTC),
                CompanionLogicalLayout::round(360.0, 360.0),
            ),
            Err(CompanionSceneProjectionError::InvalidPetRoleSpan {
                span_index: 0,
                line_index: 0,
                start_char: 0,
                end_char: 2,
                source_char_count: 1,
            })
        );
    }

    #[test]
    fn multibyte_pet_role_spans_use_unicode_scalar_indices() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_art = vec!["✦✦".to_string()];
        vm.pet_spans = vec![StyledSegment {
            line: 0,
            start: 1,
            end: 2,
            role: PaletteRoleName::Eye,
        }];

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("multibyte scalar span");

        assert_eq!(snapshot.content.pet_roles[0].line_index, 0);
        assert_eq!(snapshot.content.pet_roles[0].start_char, 1);
        assert_eq!(snapshot.content.pet_roles[0].end_char, 2);
    }

    #[test]
    fn frame_motion_matches_the_shared_round_projection_and_monotonic_clock() {
        let mut vm = fixture_with_real_pet_art();
        vm.progress.rate_per_hour = 50_000_000.0;
        vm.breath_offset_y = 1;
        let wall_time = datetime!(2026-07-11 12:00 UTC);
        let elapsed_ms = 250;
        let layout = CompanionLogicalLayout::round(360.0, 360.0);
        let input = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(wall_time, elapsed_ms),
            layout,
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );

        let snapshot = CompanionSceneSnapshot::project_with_input(&vm, input)
            .expect("project shared round motion");
        let shared = crate::round::motion::project_round_companion_motion(
            super::input::companion_motion_input(
                &vm,
                wall_time,
                &crate::round::motion::companion_roam_motion(),
            ),
            wall_time,
            elapsed_ms,
            input.motion_viewport(),
            &crate::round::motion::companion_roam_motion(),
        );

        assert_eq!(snapshot.frame.elapsed_ms, elapsed_ms);
        assert_eq!(
            snapshot.frame.pet_anchor_points,
            shared.motion_top_left_points
        );
        assert_eq!(snapshot.frame.pet_depth, shared.normalized_depth);
        assert_eq!(snapshot.frame.facing, shared.facing);
        assert_eq!(
            snapshot.frame.breath_offset_y_cells,
            shared.breath_offset_y_cells
        );
        assert_eq!(snapshot.frame.bob_offset_y_cells, shared.bob_offset_y_cells);
        assert_ne!(snapshot.frame.bob_offset_y_cells, 0.0);
        assert_ne!(snapshot.frame.pet_depth, 0.0);
    }

    #[test]
    fn prop_animation_states_use_authored_cadences_and_explicit_static_state() {
        let mut vm = fixture_with_real_pet_art();
        vm.habitat
            .earned_props
            .push(crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new(
                    crate::game::habitat::TOKEN_SPARK_500K,
                ),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Accent,
                display_priority: 30,
                source: crate::storage::state::HabitatPropSource::LifetimeTokens {
                    threshold: 500_000.0,
                },
            });
        vm.habitat
            .earned_props
            .push(crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new(
                    crate::game::habitat::FIRST_ENSEMBLE_DAY,
                ),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Trophy,
                display_priority: 65,
                source: crate::storage::state::HabitatPropSource::ActivityMilestone {
                    milestone: "ensemble".to_string(),
                },
            });
        let at = |seconds| {
            project_snapshot(
                &vm,
                time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds),
                CompanionLogicalLayout::round(360.0, 360.0),
            )
            .expect("project prop animation state")
        };
        let initial = at(0);
        let sprite_changed = at(4);
        let twinkle_changed = at(2);
        let motion_changed = at(10);

        assert_ne!(
            prop_animation_state(&initial, crate::game::habitat::CODEX_SIGNAL_LAMP).sprite_phase,
            prop_animation_state(&sprite_changed, crate::game::habitat::CODEX_SIGNAL_LAMP)
                .sprite_phase
        );
        assert_ne!(
            prop_animation_state(&initial, crate::game::habitat::TOKEN_SPARK_500K).twinkle_active,
            prop_animation_state(&twinkle_changed, crate::game::habitat::TOKEN_SPARK_500K)
                .twinkle_active
        );
        assert_ne!(
            prop_animation_state(&initial, crate::game::habitat::TOKEN_PEBBLE_25K).motion_phase,
            prop_animation_state(&motion_changed, crate::game::habitat::TOKEN_PEBBLE_25K)
                .motion_phase
        );
        assert!(
            prop_animation_state(&initial, crate::game::habitat::FIRST_ENSEMBLE_DAY).is_static()
        );
        let static_json = serde_json::to_string(prop_animation_state(
            &initial,
            crate::game::habitat::FIRST_ENSEMBLE_DAY,
        ))
        .unwrap();
        assert!(static_json.contains("\"kind\":\"static\""));
        vm.habitat.earned_props.reverse();
        let reordered = project_snapshot(
            &vm,
            time::OffsetDateTime::UNIX_EPOCH,
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("reordered prop projection");
        assert_eq!(
            initial.content.prop_animation_states,
            reordered.content.prop_animation_states
        );
    }

    #[test]
    fn tank_animation_states_are_bounded_identity_stable_and_calm_aware() {
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(
            120,
            time::macros::date!(2026 - 07 - 11),
        );
        let canonical = fixture_with_real_pet_art();
        vm.pet_art = canonical.pet_art;
        vm.pet_spans = canonical.pet_spans;
        let now = datetime!(2026-07-11 12:00 UTC);
        let normal = project_snapshot(&vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("normal tank projection");
        let mut asleep_vm = vm.clone();
        asleep_vm.day_context.asleep = true;
        let asleep = project_snapshot(&asleep_vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("asleep tank projection");
        vm.life_profile.calm_mode = true;
        let calm = project_snapshot(&vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("calm tank projection");

        assert_eq!(normal.content.tank_animation_states.len(), 2);
        for state in &normal.content.tank_animation_states {
            assert!(state.sprite_variant <= 1);
            assert_eq!(state.cadence_ms, 4_000);
        }
        assert!(calm
            .content
            .tank_animation_states
            .iter()
            .all(|state| state.calm && state.cadence_ms == 8_000));
        assert!(asleep
            .content
            .tank_animation_states
            .iter()
            .all(|state| state.calm && state.cadence_ms == 8_000));

        vm.habitat.earned_inhabitants.reverse();
        let reordered = project_snapshot(&vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("reordered tank projection");
        assert_eq!(
            calm.content.tank_animation_states,
            reordered.content.tank_animation_states
        );
    }

    #[test]
    fn tank_state_serializes_visible_route_outcomes_not_seed_tokens() {
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(
            120,
            time::macros::date!(2026 - 07 - 11),
        );
        let canonical = fixture_with_real_pet_art();
        vm.pet_art = canonical.pet_art;
        vm.pet_spans = canonical.pet_spans;

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("tank route projection");
        let json = serde_json::to_string(&snapshot.content.tank_animation_states)
            .expect("serialize tank outcomes");

        assert!(
            !json.contains("route_phase"),
            "serialized a seed-derived phase: {json}"
        );
        assert!(!json.contains("token"), "serialized a token: {json}");
        assert!(!json.contains("hash"), "serialized a hash: {json}");
        assert!(
            json.contains("origin_col"),
            "missing visible route column: {json}"
        );
        assert!(
            json.contains("origin_row"),
            "missing visible route row: {json}"
        );
        assert!(
            json.contains("sprite_variant"),
            "missing bounded sprite variant: {json}"
        );
    }

    #[test]
    fn shared_tank_route_resolver_fails_closed_and_bounds_catalog_outcomes() {
        let geometry = crate::presentation::tank_life::TankRouteGeometry::round(44, 18, 5);
        for spec in crate::game::habitat::TANK_INHABITANT_CATALOG {
            let outcome = crate::presentation::tank_life::resolve_tank_route(
                crate::presentation::tank_life::TankRouteInput {
                    catalog_id: spec.id,
                    pet_seed: "private-route-seed",
                    local_date: time::macros::date!(2026 - 07 - 11),
                    now: datetime!(2026-07-11 12:00 UTC),
                    calm: false,
                    geometry: &geometry,
                },
            )
            .expect("known catalog route");
            assert!(outcome.sprite_variant <= 1);
            assert!(outcome.origin_col < 44);
            assert!(outcome.origin_row < 18);
        }
        assert!(crate::presentation::tank_life::resolve_tank_route(
            crate::presentation::tank_life::TankRouteInput {
                catalog_id: "private-unknown-inhabitant",
                pet_seed: "private-route-seed",
                local_date: time::macros::date!(2026 - 07 - 11),
                now: datetime!(2026-07-11 12:00 UTC),
                calm: false,
                geometry: &geometry,
            },
        )
        .is_none());
    }

    #[test]
    fn weather_changes_content_without_changing_topology() {
        let clear = fixture_with_real_pet_art();
        let mut sparks = clear.clone();
        sparks.life_profile.work_weather = crate::tui::life::WorkWeather::OutputSparks;
        let now = datetime!(2026-07-11 12:00 UTC);
        let clear = project_snapshot(&clear, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("clear projection");
        let sparks = project_snapshot(&sparks, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("sparks projection");

        assert_eq!(clear.topology, sparks.topology);
        assert_ne!(clear.content.room_weather, sparks.content.room_weather);
        assert_eq!(
            serde_json::to_string(&clear.topology).unwrap(),
            serde_json::to_string(&sparks.topology).unwrap()
        );
    }

    #[test]
    fn frame_depth_parity_covers_far_neutral_and_near_fixtures() {
        let vm = fixture_with_real_pet_art();
        let now = datetime!(2026-07-11 12:00 UTC);
        for depth in [-1.0, 0.0, 1.0] {
            let input = CompanionSceneProjectionInput::round(
                CompanionProjectionClock::new(now, 500),
                CompanionLogicalLayout::round(360.0, 360.0),
                44,
                18,
                crate::round::scene::current_round_motion_clearance(18),
            )
            .with_depth_override(depth);
            let snapshot = CompanionSceneSnapshot::project_with_input(&vm, input)
                .expect("depth fixture projection");
            let shared = crate::round::motion::project_round_companion_motion_with_options(
                super::input::companion_motion_input(
                    &vm,
                    now,
                    &crate::round::motion::companion_roam_motion(),
                ),
                now,
                500,
                input.motion_viewport(),
                &crate::round::motion::companion_roam_motion(),
                crate::round::motion::RoundMotionProjectionOptions { depth_override: Some(depth) },
            );
            assert_eq!(snapshot.frame.pet_depth, depth);
            assert_eq!(snapshot.frame.pet_depth, shared.normalized_depth);
            assert_eq!(
                snapshot.frame.pet_anchor_points,
                shared.motion_top_left_points
            );
        }
    }

    #[test]
    fn frame_motion_parity_covers_active_and_asleep_calm_lifecycle() {
        let mut active = fixture_with_real_pet_art();
        active.progress.rate_per_hour = 50_000_000.0;
        let mut resting = active.clone();
        resting.day_context.asleep = true;
        resting.life_profile.calm_mode = true;
        let now = datetime!(2026-07-11 12:00 UTC);
        let input = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(now, 750),
            CompanionLogicalLayout::round(360.0, 360.0),
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );
        let active_snapshot = CompanionSceneSnapshot::project_with_input(&active, input)
            .expect("active motion projection");
        let resting_snapshot = CompanionSceneSnapshot::project_with_input(&resting, input)
            .expect("resting motion projection");
        let active_shared = crate::round::motion::project_round_companion_motion(
            super::input::companion_motion_input(
                &active,
                now,
                &crate::round::motion::companion_roam_motion(),
            ),
            now,
            750,
            input.motion_viewport(),
            &crate::round::motion::companion_roam_motion(),
        );
        let resting_shared = crate::round::motion::project_round_companion_motion(
            super::input::companion_motion_input(
                &resting,
                now,
                &crate::round::motion::companion_roam_motion(),
            ),
            now,
            750,
            input.motion_viewport(),
            &crate::round::motion::companion_roam_motion(),
        );

        assert_eq!(
            active_snapshot.frame.pet_anchor_points,
            active_shared.motion_top_left_points
        );
        assert_eq!(
            resting_snapshot.frame.pet_anchor_points,
            resting_shared.motion_top_left_points
        );
        assert_eq!(
            resting_snapshot.frame.bob_offset_y_cells,
            active_snapshot.frame.bob_offset_y_cells
        );
        assert!(resting_snapshot.frame.asleep);
        assert!(resting_snapshot
            .content
            .tank_animation_states
            .iter()
            .all(|state| state.calm));
        assert!(resting_shared.normalized_depth.abs() < active_shared.normalized_depth.abs());
    }
}
