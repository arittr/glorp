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
    pub room_weather: &'static str,
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
    pub pet_lines: Vec<String>,
    pub pet_roles: Vec<PetRoleSpanSnapshot>,
    pub palette: PaletteSnapshot,
    pub prop_animation_phases: Vec<u8>,
    pub tank_animation_phases: Vec<u8>,
    pub activity_pulse_age_ms: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PetRoleSpanSnapshot {
    pub line: u16,
    pub start: u16,
    pub end: u16,
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
    pub pet_xy_depth: [f32; 3],
    pub facing: i8,
    pub breath_offset_y: u8,
    pub asleep: bool,
    pub helper_trouble: bool,
    pub gauges: [f32; 4],
    pub dim_amount: f32,
    pub hud_lines: [String; 3],
}

#[cfg(test)]
mod tests {
    use super::{CompanionLogicalLayout, CompanionSceneSnapshot};
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    #[test]
    fn projection_is_one_privacy_aware_snapshot() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let snapshot = CompanionSceneSnapshot::project(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        );

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
    }

    #[test]
    fn projection_serialization_excludes_every_private_input_sentinel() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
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

        let snapshot = CompanionSceneSnapshot::project(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        );
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
    fn seed_changes_do_not_change_serialized_pet_placement() {
        let mut first = WatchViewModel::fixture();
        first.pet_render.seed = "sentinel-seed-alpha".to_string();
        let mut second = first.clone();
        second.pet_render.seed = "sentinel-seed-beta".to_string();
        let layout = CompanionLogicalLayout::round(360.0, 360.0);
        let now = datetime!(2026-07-11 12:00 UTC);

        let first = CompanionSceneSnapshot::project(&first, now, layout);
        let second = CompanionSceneSnapshot::project(&second, now, layout);

        assert_eq!(first.frame.pet_xy_depth, second.frame.pet_xy_depth);
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

        let first = CompanionSceneSnapshot::project(&vm, now, layout);
        let second = CompanionSceneSnapshot::project(&vm, now, layout);

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
}
