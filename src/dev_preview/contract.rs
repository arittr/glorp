use crate::pet::render::StyledSegment;
use crate::presentation::privacy::PresentationSurface;
use crate::presentation::scene::PresentationScene;
use crate::round::model::RoundSceneModel;
use crate::tui::component::PreviewLayout;
use crate::tui::room::{biome_symbols, derive_room_life_profile, RoomSpeciesDialect};
use crate::tui::view_model::WatchViewModel;
use serde::Serialize;
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub const CONTRACT_SCHEMA_VERSION: u32 = 1;
pub const HUD_CONTRACT_SCHEMA_VERSION: u32 = 2;
pub const TANK_LIFE_CONTRACT_SCHEMA_VERSION: u32 = 1;
const REDACTED_RUNTIME_ID: &str = "redacted";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewFrameContract {
    pub pixel: Option<crate::dev_preview::export::PreviewPixelFrameArtifact>,
    pub pixel_art: Option<crate::dev_preview::export::PreviewPixelArtArtifact>,
    pub pixel_fit: Option<crate::dev_preview::export::PreviewPixelFitArtifact>,
    pub scene: Option<PreviewSceneArtifact>,
    pub hud: Option<PreviewHudArtifact>,
    pub tank_life: Option<PreviewTankLifeArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewSceneArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub fixture: PreviewFixtureArtifact,
    pub privacy_projection: PreviewPrivacyProjection,
    pub pet: PreviewPetArtifact,
    pub room: PreviewRoomArtifact,
    pub activity: PreviewActivityArtifact,
    pub targets: BTreeMap<String, PreviewTargetArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewFixtureArtifact {
    pub id: String,
    pub source: String,
    pub fixed_now_unix: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPrivacyProjection {
    pub surface: String,
    pub source_names_visible: bool,
    pub exact_counts_visible: bool,
    pub diagnostic_text_visible: bool,
    pub feed_rows_visible: bool,
    pub file_paths_visible: bool,
    pub project_names_visible: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPetArtifact {
    pub seed: String,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub asleep: bool,
    pub art_text: String,
    pub span_count: usize,
    pub roles: Vec<String>,
    pub facing: i8,
    pub breath_offset_y: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoomArtifact {
    pub primary_biome: String,
    pub secondary_biome: Option<String>,
    pub species_dialect: String,
    pub dialect_status: Option<String>,
    pub work_weather: String,
    pub day_phase: String,
    pub prop_landmarks: Vec<String>,
    pub glyph_vocabulary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewActivityArtifact {
    pub source_diversity: String,
    pub helper_health: String,
    pub recent_activity: String,
    pub vitals: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTargetArtifact {
    pub role: String,
    pub layer: String,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewHudArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub gap_deg: f64,
    pub aperture_radius: f64,
    pub lanes: BTreeMap<String, PreviewHudLaneArtifact>,
    pub text: PreviewHudTextArtifact,
    pub privacy_projection: PreviewPrivacyProjection,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewHudLaneArtifact {
    pub radius: f64,
    pub stroke_width: f64,
    pub track_start_deg: f64,
    pub track_sweep_deg: f64,
    pub fill_fraction: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overfill_fraction: Option<f64>,
    pub cap: String,
    pub track_color: PreviewHudColorArtifact,
    pub fill_color: PreviewHudColorArtifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overfill_color: Option<PreviewHudColorArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewHudTextArtifact {
    pub today_total: String,
    pub daily_percent: String,
    pub pace: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PreviewHudColorArtifact {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewTankLifeArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub local_date: String,
    pub calendar_age_days: i64,
    pub target_surface: String,
    pub canonical_ids: Vec<String>,
    pub rendered_ids: Vec<String>,
    pub skipped: Vec<PreviewTankLifeSkipArtifact>,
    pub anemone_morph: Option<String>,
    pub placements: Vec<PreviewTankLifePlacementArtifact>,
    pub layer_segments: Vec<PreviewTankLifeLayerArtifact>,
    pub collision_status: PreviewTankLifeCollisionArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifeSkipArtifact {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifePlacementArtifact {
    pub id: String,
    pub route_family: String,
    pub bounds: PreviewTargetArtifact,
    pub cell_count: usize,
    pub cells: Vec<PreviewTankLifeCellArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifeCellArtifact {
    pub row: u16,
    pub col: u16,
    pub glyph: String,
    pub pet_layer: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifeLayerArtifact {
    pub id: String,
    pub pet_layer: String,
    pub cell_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTankLifeCollisionArtifact {
    pub reserved_region_clear: bool,
    pub aperture_clear: bool,
    pub protected_pet_face_clear: bool,
}

impl PreviewSceneArtifact {
    pub fn from_watch_view_model(
        frame_id: &str,
        vm: &WatchViewModel,
        now: OffsetDateTime,
        layout: Option<&PreviewLayout>,
    ) -> Self {
        let presentation_scene = PresentationScene::from_watch_view_model(
            vm,
            now,
            PresentationSurface::PreviewLabArtifact,
        );
        let room_profile = derive_room_life_profile(vm, now);
        let dialect = &room_profile.species_dialect;
        let room_dialect = RoomSpeciesDialect::for_species(vm.pet_render.generated_species);
        let glyph_vocabulary = biome_symbols(room_profile.biome.primary, room_dialect)
            .iter()
            .map(|ch| ch.to_string())
            .collect();

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture: PreviewFixtureArtifact {
                id: frame_id.to_string(),
                source: "watch-view-model".to_string(),
                fixed_now_unix: now.unix_timestamp(),
            },
            privacy_projection: PreviewPrivacyProjection::sanitized("preview-lab-scene"),
            pet: PreviewPetArtifact::from_watch(vm, &presentation_scene.pet.seed),
            room: PreviewRoomArtifact {
                primary_biome: format!("{:?}", room_profile.biome.primary),
                secondary_biome: room_profile.biome.secondary.map(|tag| format!("{tag:?}")),
                species_dialect: dialect.key.as_str().to_string(),
                dialect_status: Some(dialect.status.as_str().to_string()),
                work_weather: format!("{:?}", vm.life_profile.work_weather),
                day_phase: format!("{:?}", vm.day_context.day_phase),
                prop_landmarks: presentation_scene
                    .room
                    .prop_landmarks
                    .iter()
                    .map(|id| id.as_str().to_string())
                    .collect(),
                glyph_vocabulary,
            },
            activity: PreviewActivityArtifact::from_watch(vm),
            targets: layout.map(targets_from_preview_layout).unwrap_or_default(),
        }
    }

    pub fn from_round_scene(frame_id: &str, scene: &RoundSceneModel, now: OffsetDateTime) -> Self {
        let room_dialect = RoomSpeciesDialect::for_species(scene.pet.species);
        let glyph_vocabulary = biome_symbols(scene.room.biome.primary, room_dialect)
            .iter()
            .map(|ch| ch.to_string())
            .collect();

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture: PreviewFixtureArtifact {
                id: frame_id.to_string(),
                source: "round-scene-model".to_string(),
                fixed_now_unix: now.unix_timestamp(),
            },
            privacy_projection: PreviewPrivacyProjection::sanitized("round-preview"),
            pet: PreviewPetArtifact {
                seed: REDACTED_RUNTIME_ID.to_string(),
                species: scene.pet.species.as_str().to_string(),
                stage: format!("{:?}", scene.pet.stage).to_lowercase(),
                mood: format!("{:?}", scene.pet.mood).to_lowercase(),
                asleep: scene.pet.asleep,
                art_text: scene.pet.art_lines.join("\n"),
                span_count: scene.pet.art_spans.len(),
                roles: role_names(&scene.pet.art_spans),
                facing: scene.pet.facing,
                breath_offset_y: scene.pet.breath_offset_y,
            },
            room: PreviewRoomArtifact {
                primary_biome: format!("{:?}", scene.room.biome.primary),
                secondary_biome: scene.room.biome.secondary.map(|tag| format!("{tag:?}")),
                species_dialect: scene.room.dialect.as_str().to_string(),
                dialect_status: None,
                work_weather: format!("{:?}", scene.room.work_weather),
                day_phase: format!("{:?}", scene.room.day_phase),
                prop_landmarks: Vec::new(),
                glyph_vocabulary,
            },
            activity: PreviewActivityArtifact::from_round(scene),
            targets: BTreeMap::new(),
        }
    }
}

impl PreviewHudArtifact {
    pub fn from_companion_view_model(
        frame_id: &str,
        vm: &WatchViewModel,
        aperture: crate::round::layout::RoundAperture,
    ) -> Self {
        let gap_deg = crate::round::hud::COMPANION_GAUGE_GAP_DEG;
        let layout = crate::round::hud::perimeter_gauge_layout(
            aperture.center_x as f64,
            aperture.center_y as f64,
            aperture.radius as f64,
            gap_deg,
        );
        let colors = crate::round::hud::perimeter_gauge_colors();
        let xp_fraction = if vm.progress.is_max_stage {
            1.0
        } else {
            vm.progress.fraction as f64
        };
        let daily_ratio = vm.daily_comparison.fraction_of_yesterday;
        let daily_fraction = crate::round::hud::daily_fraction_for_gauge(daily_ratio);
        let daily_overfill_fraction = crate::round::hud::daily_overage_marker_fraction(daily_ratio);
        let pace_fraction =
            crate::round::hud::companion_pace_fraction(vm.rate_momentum.pulse.current_tokens);
        let text = crate::round::hud::companion_hud_text(
            vm.today_effective_tokens,
            daily_ratio,
            vm.rate_momentum.pulse.current_tokens,
        );

        Self {
            schema_version: HUD_CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            gap_deg,
            aperture_radius: aperture.radius as f64,
            lanes: BTreeMap::from([
                (
                    "xp".to_string(),
                    PreviewHudLaneArtifact::from_lane(&layout.xp, &colors.xp, xp_fraction),
                ),
                (
                    "daily".to_string(),
                    PreviewHudLaneArtifact::from_lane(&layout.daily, &colors.daily, daily_fraction)
                        .with_overfill(
                            daily_overfill_fraction,
                            crate::round::hud::daily_overage_color(),
                        ),
                ),
                (
                    "pace".to_string(),
                    PreviewHudLaneArtifact::from_lane(&layout.pace, &colors.pace, pace_fraction),
                ),
            ]),
            text: PreviewHudTextArtifact {
                today_total: text.today_total,
                daily_percent: text.daily_percent,
                pace: text.pace,
            },
            privacy_projection: PreviewPrivacyProjection {
                surface: "companion-hud".to_string(),
                source_names_visible: false,
                exact_counts_visible: true,
                diagnostic_text_visible: false,
                feed_rows_visible: false,
                file_paths_visible: false,
                project_names_visible: false,
            },
        }
    }
}

impl PreviewHudLaneArtifact {
    fn from_lane(
        lane: &crate::round::hud::GaugeLane,
        colors: &crate::round::hud::GaugeLaneColors,
        fill_fraction: f64,
    ) -> Self {
        Self {
            radius: lane.ring.radius,
            stroke_width: lane.stroke_width,
            track_start_deg: lane.ring.track_start_deg,
            track_sweep_deg: lane.ring.track_sweep_deg,
            fill_fraction: fill_fraction.clamp(0.0, 1.0),
            overfill_fraction: None,
            cap: match lane.cap {
                crate::round::hud::LineCap::Butt => "butt".to_string(),
                crate::round::hud::LineCap::Round => "round".to_string(),
            },
            track_color: PreviewHudColorArtifact::from_round_color(colors.track),
            fill_color: PreviewHudColorArtifact::from_round_color(colors.fill),
            overfill_color: None,
        }
    }

    fn with_overfill(
        mut self,
        overfill_fraction: f64,
        overfill_color: crate::round::draw::RoundColor,
    ) -> Self {
        let overfill_fraction = overfill_fraction.clamp(0.0, 1.0);
        if overfill_fraction > 0.0 {
            self.overfill_fraction = Some(overfill_fraction);
            self.overfill_color = Some(PreviewHudColorArtifact::from_round_color(overfill_color));
        }
        self
    }
}

impl PreviewHudColorArtifact {
    fn from_round_color(color: crate::round::draw::RoundColor) -> Self {
        Self {
            r: color.0 as f64,
            g: color.1 as f64,
            b: color.2 as f64,
            a: color.3 as f64,
        }
    }
}

impl PreviewPrivacyProjection {
    pub fn sanitized(surface: &str) -> Self {
        Self {
            surface: surface.to_string(),
            source_names_visible: false,
            exact_counts_visible: false,
            diagnostic_text_visible: false,
            feed_rows_visible: false,
            file_paths_visible: false,
            project_names_visible: false,
        }
    }
}

impl PreviewPetArtifact {
    fn from_watch(vm: &WatchViewModel, seed: &str) -> Self {
        Self {
            seed: seed.to_string(),
            species: vm.pet_render.generated_species.as_str().to_string(),
            stage: format!("{:?}", vm.pet_render.stage).to_lowercase(),
            mood: format!("{:?}", vm.pet_render.mood).to_lowercase(),
            asleep: vm.day_context.asleep,
            art_text: vm.pet_art.join("\n"),
            span_count: vm.pet_spans.len(),
            roles: role_names(&vm.pet_spans),
            facing: vm.facing,
            breath_offset_y: vm.breath_offset_y,
        }
    }
}

impl PreviewActivityArtifact {
    fn from_watch(vm: &WatchViewModel) -> Self {
        let helper_health = if vm.source_health.iter().any(|health| {
            matches!(
                health.status,
                crate::tui::view_model::SourceStatus::Diagnostic
            )
        }) {
            "trouble"
        } else {
            "ok"
        };

        Self {
            source_diversity: format!("{:?}", vm.activity_identity.source_diversity),
            helper_health: helper_health.to_string(),
            recent_activity: vm
                .last_feed_pulse_at
                .map(|_| "recent")
                .unwrap_or("quiet")
                .to_string(),
            vitals: BTreeMap::from([
                ("fed".to_string(), vital_bucket(vm.fed).to_string()),
                (
                    "happiness".to_string(),
                    vital_bucket(vm.happiness).to_string(),
                ),
                ("energy".to_string(), vital_bucket(vm.energy).to_string()),
            ]),
        }
    }

    fn from_round(scene: &RoundSceneModel) -> Self {
        Self {
            source_diversity: format!("{:?}", scene.halo.source_diversity),
            helper_health: format!("{:?}", scene.halo.helper_health).to_lowercase(),
            recent_activity: format!("{:?}", scene.halo.activity_pulse).to_lowercase(),
            vitals: BTreeMap::from([
                (
                    "fed".to_string(),
                    format!("{:?}", scene.halo.vitals.fed).to_lowercase(),
                ),
                (
                    "happiness".to_string(),
                    format!("{:?}", scene.halo.vitals.happiness).to_lowercase(),
                ),
                (
                    "energy".to_string(),
                    format!("{:?}", scene.halo.vitals.energy).to_lowercase(),
                ),
            ]),
        }
    }
}

fn targets_from_preview_layout(layout: &PreviewLayout) -> BTreeMap<String, PreviewTargetArtifact> {
    layout
        .targets
        .iter()
        .enumerate()
        .map(|(index, (_id, target))| {
            (
                format!("target-{index:02}"),
                PreviewTargetArtifact {
                    role: target.role.clone(),
                    layer: target.layer.clone(),
                    x: target.x,
                    y: target.y,
                    width: target.width,
                    height: target.height,
                },
            )
        })
        .collect()
}

fn role_names(spans: &[StyledSegment]) -> Vec<String> {
    let mut roles = spans
        .iter()
        .map(|span| format!("{:?}", span.role).to_lowercase())
        .collect::<Vec<_>>();
    roles.sort();
    roles.dedup();
    roles
}

fn vital_bucket(value: f64) -> &'static str {
    if value < 0.34 {
        "low"
    } else if value < 0.67 {
        "medium"
    } else {
        "high"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::model::derive_round_scene_model;
    use crate::tui::component::{PreviewLayout, PreviewRect, PreviewTarget};
    use crate::tui::view_model::{SourceStatus, WatchViewModel};
    use std::collections::BTreeMap;
    use time::macros::datetime;

    #[test]
    fn watch_scene_contract_sanitizes_inputs_and_buckets_normalized_vitals() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.fed = 0.33;
        vm.happiness = 0.34;
        vm.energy = 0.67;
        vm.source_health[0].status = SourceStatus::Diagnostic;
        let layout = PreviewLayout {
            schema_version: 2,
            frame_id: "watch-contract".to_string(),
            mode: "wide".to_string(),
            frame: PreviewRect { x: 0, y: 0, width: 120, height: 32 },
            content: PreviewRect { x: 1, y: 1, width: 118, height: 30 },
            components: BTreeMap::new(),
            targets: BTreeMap::from([(
                "watch.pet".to_string(),
                PreviewTarget {
                    x: 10,
                    y: 5,
                    width: 20,
                    height: 8,
                    owner: "watch".to_string(),
                    role: "Pet".to_string(),
                    clip: PreviewRect { x: 10, y: 5, width: 20, height: 8 },
                    z: 2,
                    layer: "component".to_string(),
                    cell_count: Some(12),
                },
            )]),
            decisions: vec![],
        };

        let artifact = PreviewSceneArtifact::from_watch_view_model(
            "watch-contract",
            &vm,
            datetime!(2026-06-13 18:00 UTC),
            Some(&layout),
        );

        assert_eq!(artifact.schema_version, CONTRACT_SCHEMA_VERSION);
        assert_eq!(artifact.fixture.source, "watch-view-model");
        assert_eq!(artifact.fixture.fixed_now_unix, 1_781_373_600);
        assert_eq!(artifact.privacy_projection.surface, "preview-lab-scene");
        assert!(!artifact.privacy_projection.source_names_visible);
        assert_eq!(artifact.activity.helper_health, "trouble");
        assert_eq!(artifact.activity.vitals["fed"], "low");
        assert_eq!(artifact.activity.vitals["happiness"], "medium");
        assert_eq!(artifact.activity.vitals["energy"], "high");
        let target = artifact
            .targets
            .values()
            .find(|target| target.role == "Pet" && target.layer == "component")
            .expect("expected neutral pet target");
        assert_eq!(target.width, 20);
        assert_eq!(artifact.pet.seed, REDACTED_RUNTIME_ID);
        assert!(artifact.room.prop_landmarks.is_empty());
        assert!(!artifact.room.glyph_vocabulary.is_empty());
    }

    #[test]
    fn round_scene_contract_uses_round_model_without_preview_layout_targets() {
        let now = datetime!(2026-06-13 18:00 UTC);
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.day_context.asleep = true;
        let scene = derive_round_scene_model(&vm, now);

        let artifact = PreviewSceneArtifact::from_round_scene("round-contract", &scene, now);

        assert_eq!(artifact.schema_version, CONTRACT_SCHEMA_VERSION);
        assert_eq!(artifact.fixture.source, "round-scene-model");
        assert_eq!(artifact.privacy_projection.surface, "round-preview");
        assert_eq!(artifact.pet.seed, REDACTED_RUNTIME_ID);
        assert!(artifact.pet.asleep);
        assert_eq!(artifact.room.species_dialect, "fuzz");
        assert_eq!(artifact.room.dialect_status, None);
        assert!(artifact.room.prop_landmarks.is_empty());
        assert_eq!(artifact.targets, BTreeMap::new());
    }
}
