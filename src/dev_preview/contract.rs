use crate::pet::render::StyledSegment;
use crate::presentation::privacy::PresentationSurface;
use crate::presentation::scene::PresentationScene;
use crate::presentation::smooth::{
    CompanionChromeReservation, CompanionViewport, SmoothBounds, SmoothCompanionLayer,
    SmoothCompanionPrivacyClaims, SmoothCompanionScenePlan, SmoothLayerRole,
    SmoothParallaxPlaneTranslations, SmoothPoint, SmoothTransform,
};
use crate::round::model::RoundSceneModel;
use crate::tui::component::PreviewLayout;
use crate::tui::room::{biome_symbols, derive_room_life_profile, RoomSpeciesDialect};
use crate::tui::view_model::WatchViewModel;
use serde::Serialize;
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub const CONTRACT_SCHEMA_VERSION: u32 = 1;
pub const HUD_CONTRACT_SCHEMA_VERSION: u32 = 3;
pub const TANK_LIFE_CONTRACT_SCHEMA_VERSION: u32 = 1;
const REDACTED_RUNTIME_ID: &str = "redacted";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewFrameContract {
    pub pixel: Option<crate::dev_preview::export::PreviewPixelFrameArtifact>,
    pub pixel_art: Option<crate::dev_preview::export::PreviewPixelArtArtifact>,
    pub pixel_composition: Option<crate::dev_preview::export::PreviewPixelCompositionArtifact>,
    pub pixel_fit: Option<crate::dev_preview::export::PreviewPixelFitArtifact>,
    pub smooth_plan: Option<PreviewSmoothPlanArtifact>,
    pub smooth_parity: Option<PreviewSmoothParityArtifact>,
    pub smooth_motion: Option<PreviewSmoothMotionArtifact>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rollover_layers: Vec<PreviewHudRolloverLayerArtifact>,
    pub cap: String,
    pub track_color: PreviewHudColorArtifact,
    pub fill_color: PreviewHudColorArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewHudRolloverLayerArtifact {
    pub rollover: u32,
    pub fraction: f64,
    pub color: PreviewHudColorArtifact,
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

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothPlanArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub viewport: PreviewSmoothViewportArtifact,
    pub flatten_checksum: u64,
    pub parallax_focus_offset: PreviewSmoothPointArtifact,
    pub parallax_lifecycle_scale: f32,
    pub parallax_planes: PreviewSmoothParallaxPlanesArtifact,
    pub layers: Vec<PreviewSmoothLayerArtifact>,
    pub chrome: PreviewSmoothChromeArtifact,
    pub abstract_state: BTreeMap<String, String>,
    pub privacy: PreviewSmoothPrivacyArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothParityArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub fixture_id: String,
    pub classic_checksum: u64,
    pub smooth_flatten_checksum: u64,
    pub exact_match: bool,
    pub required_roles: Vec<String>,
    pub missing_roles: Vec<String>,
    pub review_status: String,
    pub abstract_state: BTreeMap<String, String>,
    pub privacy: PreviewSmoothPrivacyArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothMotionArtifact {
    pub schema_version: u32,
    pub strip_id: String,
    pub frame_index: u16,
    pub elapsed_ms: u64,
    pub now_unix_ms: i128,
    pub semantic_art_tick_index: u64,
    pub pet_visual_checksum: u64,
    pub pet_motion: PreviewSmoothPetMotionArtifact,
    pub parallax_focus_offset: PreviewSmoothPointArtifact,
    pub parallax_lifecycle_scale: f32,
    pub parallax_planes: PreviewSmoothParallaxPlanesArtifact,
    pub max_adjacent_parallax_delta_by_plane: PreviewSmoothParallaxPlanesArtifact,
    pub layer_transforms: Vec<PreviewSmoothMotionLayerArtifact>,
    pub chrome: PreviewSmoothChromeArtifact,
    pub abstract_state: BTreeMap<String, String>,
    pub privacy: PreviewSmoothPrivacyArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothViewportArtifact {
    pub grid_cols: u16,
    pub grid_rows: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothLayerArtifact {
    pub role: String,
    pub motion_binding: String,
    pub depth_plane: Option<String>,
    pub z: i16,
    pub local_bounds: PreviewSmoothBoundsArtifact,
    pub anchor: PreviewSmoothPointArtifact,
    pub transform_origin: PreviewSmoothPointArtifact,
    pub transform: PreviewSmoothTransformArtifact,
    pub parallax_translation: PreviewSmoothPointArtifact,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothMotionLayerArtifact {
    pub role: String,
    pub motion_binding: String,
    pub depth_plane: Option<String>,
    pub z: i16,
    pub local_bounds: PreviewSmoothBoundsArtifact,
    pub translation: PreviewSmoothPointArtifact,
    pub scale: PreviewSmoothPointArtifact,
    pub rotation_degrees: f32,
    pub opacity: f32,
    pub parallax_translation: PreviewSmoothPointArtifact,
    pub item_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PreviewSmoothParallaxPlanesArtifact {
    pub far: PreviewSmoothPointArtifact,
    pub mid: PreviewSmoothPointArtifact,
    pub behind: PreviewSmoothPointArtifact,
    pub foreground: PreviewSmoothPointArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothPetMotionArtifact {
    pub base_anchor: PreviewSmoothPointArtifact,
    pub bob_offset: PreviewSmoothPointArtifact,
    pub final_anchor: PreviewSmoothPointArtifact,
    pub classic_snap_anchor: PreviewSmoothPointArtifact,
    pub scale_x: f32,
    pub scale_y: f32,
    pub opacity: f32,
    pub pulse: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothChromeArtifact {
    pub hud_bounds: Vec<PreviewSmoothBoundsArtifact>,
    pub gauge_bounds: Vec<PreviewSmoothBoundsArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothPrivacyArtifact {
    pub source_names_visible: bool,
    pub exact_token_strings_visible: bool,
    pub project_names_visible: bool,
    pub file_paths_visible: bool,
    pub prompt_text_visible: bool,
    pub response_text_visible: bool,
    pub raw_diagnostics_visible: bool,
    pub unprojected_pet_seed_visible: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PreviewSmoothBoundsArtifact {
    pub min: PreviewSmoothPointArtifact,
    pub max: PreviewSmoothPointArtifact,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PreviewSmoothPointArtifact {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PreviewSmoothTransformArtifact {
    pub translation: PreviewSmoothPointArtifact,
    pub scale: PreviewSmoothPointArtifact,
    pub rotation_degrees: f32,
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
        let daily_rollover_layers =
            crate::round::hud::daily_rollover_layers(daily_overfill_fraction);
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
                        .with_rollovers(&daily_rollover_layers),
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
            rollover_layers: Vec::new(),
            cap: match lane.cap {
                crate::round::hud::LineCap::Butt => "butt".to_string(),
                crate::round::hud::LineCap::Round => "round".to_string(),
            },
            track_color: PreviewHudColorArtifact::from_round_color(colors.track),
            fill_color: PreviewHudColorArtifact::from_round_color(colors.fill),
        }
    }

    fn with_rollovers(mut self, layers: &[crate::round::hud::DailyRolloverLayer]) -> Self {
        self.rollover_layers = layers
            .iter()
            .map(|layer| PreviewHudRolloverLayerArtifact {
                rollover: layer.rollover,
                fraction: layer.fraction,
                color: PreviewHudColorArtifact::from_round_color(layer.color),
            })
            .collect();
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

impl PreviewSmoothPlanArtifact {
    pub fn from_scene_plan(
        frame_id: &str,
        vm: &WatchViewModel,
        plan: &SmoothCompanionScenePlan,
    ) -> Self {
        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            viewport: PreviewSmoothViewportArtifact::from_viewport(plan.viewport),
            flatten_checksum: plan.classic_flatten_checksum(),
            parallax_focus_offset: PreviewSmoothPointArtifact::from_point(
                plan.pet.parallax_focus_offset,
            ),
            parallax_lifecycle_scale: plan.parallax_lifecycle_scale,
            parallax_planes: plan.parallax_translations_by_plane().into(),
            layers: plan
                .layers
                .iter()
                .map(PreviewSmoothLayerArtifact::from_layer)
                .collect(),
            chrome: PreviewSmoothChromeArtifact::from_chrome(&plan.chrome),
            abstract_state: smooth_abstract_state(vm),
            privacy: PreviewSmoothPrivacyArtifact::from_claims(&plan.privacy),
        }
    }
}

impl PreviewSmoothParityArtifact {
    pub fn from_scene_plan(
        frame_id: &str,
        fixture_id: &str,
        vm: &WatchViewModel,
        classic_checksum: u64,
        plan: &SmoothCompanionScenePlan,
    ) -> Self {
        let smooth_flatten_checksum = plan.classic_flatten_checksum();
        let required_roles = smooth_required_role_names();
        let missing_roles = smooth_missing_role_names(plan);
        let exact_match = classic_checksum == smooth_flatten_checksum && missing_roles.is_empty();
        let review_status = if !missing_roles.is_empty() {
            "missing-required-roles".to_string()
        } else if exact_match {
            "exact-match".to_string()
        } else {
            "smooth-extension".to_string()
        };

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture_id: fixture_id.to_string(),
            classic_checksum,
            smooth_flatten_checksum,
            exact_match,
            required_roles,
            missing_roles,
            review_status,
            abstract_state: smooth_abstract_state(vm),
            privacy: PreviewSmoothPrivacyArtifact::from_claims(&plan.privacy),
        }
    }
}

impl PreviewSmoothMotionArtifact {
    #[allow(clippy::too_many_arguments)]
    pub fn from_scene_plan(
        strip_id: &str,
        frame_index: u16,
        elapsed_ms: u64,
        now: time::OffsetDateTime,
        semantic_art_tick_index: u64,
        vm: &WatchViewModel,
        plan: &SmoothCompanionScenePlan,
        max_adjacent_parallax_delta_by_plane: SmoothParallaxPlaneTranslations,
    ) -> Self {
        let pet_layer = plan
            .layer_by_role(SmoothLayerRole::PetBody)
            .expect("smooth plan should include a pet-body layer for motion proof");
        let pulse = if vm.last_feed_pulse_at.is_some() {
            "recent-feed"
        } else if vm.day_context.asleep {
            "asleep"
        } else {
            "steady"
        };

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            strip_id: strip_id.to_string(),
            frame_index,
            elapsed_ms,
            now_unix_ms: i128::from(now.unix_timestamp()) * 1_000 + i128::from(now.millisecond()),
            semantic_art_tick_index,
            pet_visual_checksum: crate::presentation::smooth::pet_visual_checksum(
                &vm.pet_art,
                &vm.pet_spans,
            ),
            pet_motion: PreviewSmoothPetMotionArtifact {
                base_anchor: PreviewSmoothPointArtifact::from_point(plan.pet.base_anchor),
                bob_offset: PreviewSmoothPointArtifact::from_point(plan.pet.bob_offset),
                final_anchor: PreviewSmoothPointArtifact::from_point(plan.pet.final_anchor),
                classic_snap_anchor: PreviewSmoothPointArtifact::from_point(
                    plan.pet.classic_snap_anchor,
                ),
                scale_x: pet_layer.transform.scale.x,
                scale_y: pet_layer.transform.scale.y,
                opacity: pet_layer.opacity,
                pulse: pulse.to_string(),
            },
            parallax_focus_offset: PreviewSmoothPointArtifact::from_point(
                plan.pet.parallax_focus_offset,
            ),
            parallax_lifecycle_scale: plan.parallax_lifecycle_scale,
            parallax_planes: plan.parallax_translations_by_plane().into(),
            max_adjacent_parallax_delta_by_plane: max_adjacent_parallax_delta_by_plane.into(),
            layer_transforms: plan
                .layers
                .iter()
                .map(PreviewSmoothMotionLayerArtifact::from_layer)
                .collect(),
            chrome: PreviewSmoothChromeArtifact::from_chrome(&plan.chrome),
            abstract_state: smooth_abstract_state(vm),
            privacy: PreviewSmoothPrivacyArtifact::from_claims(&plan.privacy),
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

impl PreviewSmoothViewportArtifact {
    fn from_viewport(viewport: CompanionViewport) -> Self {
        Self {
            grid_cols: viewport.grid_cols,
            grid_rows: viewport.grid_rows,
        }
    }
}

impl PreviewSmoothLayerArtifact {
    fn from_layer(layer: &SmoothCompanionLayer) -> Self {
        Self {
            role: layer.role.as_str().to_string(),
            motion_binding: layer.motion_binding.as_str().to_string(),
            depth_plane: layer
                .motion_binding
                .depth_plane()
                .map(|plane| plane.as_str().to_string()),
            z: layer.z,
            local_bounds: PreviewSmoothBoundsArtifact::from_bounds(layer.local_bounds),
            anchor: PreviewSmoothPointArtifact::from_point(layer.anchor),
            transform_origin: PreviewSmoothPointArtifact::from_point(layer.transform_origin),
            transform: PreviewSmoothTransformArtifact::from_transform(layer.transform),
            parallax_translation: PreviewSmoothPointArtifact::from_point(
                layer.parallax_translation,
            ),
            item_count: smooth_item_count(layer),
        }
    }
}

impl PreviewSmoothMotionLayerArtifact {
    fn from_layer(layer: &SmoothCompanionLayer) -> Self {
        Self {
            role: layer.role.as_str().to_string(),
            motion_binding: layer.motion_binding.as_str().to_string(),
            depth_plane: layer
                .motion_binding
                .depth_plane()
                .map(|plane| plane.as_str().to_string()),
            z: layer.z,
            local_bounds: PreviewSmoothBoundsArtifact::from_bounds(layer.local_bounds),
            translation: PreviewSmoothPointArtifact::from_point(layer.transform.translation),
            scale: PreviewSmoothPointArtifact::from_point(layer.transform.scale),
            rotation_degrees: layer.transform.rotation_degrees,
            opacity: layer.opacity,
            parallax_translation: PreviewSmoothPointArtifact::from_point(
                layer.parallax_translation,
            ),
            item_count: smooth_item_count(layer),
        }
    }
}

impl From<SmoothParallaxPlaneTranslations> for PreviewSmoothParallaxPlanesArtifact {
    fn from(planes: SmoothParallaxPlaneTranslations) -> Self {
        Self {
            far: PreviewSmoothPointArtifact::from_point(planes.far),
            mid: PreviewSmoothPointArtifact::from_point(planes.mid),
            behind: PreviewSmoothPointArtifact::from_point(planes.behind),
            foreground: PreviewSmoothPointArtifact::from_point(planes.foreground),
        }
    }
}

impl PreviewSmoothChromeArtifact {
    fn from_chrome(chrome: &CompanionChromeReservation) -> Self {
        Self {
            hud_bounds: chrome
                .hud_bounds
                .iter()
                .copied()
                .map(PreviewSmoothBoundsArtifact::from_bounds)
                .collect(),
            gauge_bounds: chrome
                .gauge_bounds
                .iter()
                .copied()
                .map(PreviewSmoothBoundsArtifact::from_bounds)
                .collect(),
        }
    }
}

impl PreviewSmoothPrivacyArtifact {
    fn from_claims(claims: &SmoothCompanionPrivacyClaims) -> Self {
        Self {
            source_names_visible: claims.source_names_visible,
            exact_token_strings_visible: claims.exact_token_strings_visible,
            project_names_visible: claims.project_names_visible,
            file_paths_visible: claims.file_paths_visible,
            prompt_text_visible: claims.prompt_text_visible,
            response_text_visible: claims.response_text_visible,
            raw_diagnostics_visible: claims.raw_diagnostics_visible,
            unprojected_pet_seed_visible: claims.unprojected_pet_seed_visible,
        }
    }
}

impl PreviewSmoothBoundsArtifact {
    fn from_bounds(bounds: SmoothBounds) -> Self {
        Self {
            min: PreviewSmoothPointArtifact::from_point(bounds.min),
            max: PreviewSmoothPointArtifact::from_point(bounds.max),
        }
    }
}

impl PreviewSmoothPointArtifact {
    fn from_point(point: SmoothPoint) -> Self {
        Self { x: point.x, y: point.y }
    }
}

impl PreviewSmoothTransformArtifact {
    fn from_transform(transform: SmoothTransform) -> Self {
        Self {
            translation: PreviewSmoothPointArtifact::from_point(transform.translation),
            scale: PreviewSmoothPointArtifact::from_point(transform.scale),
            rotation_degrees: transform.rotation_degrees,
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
        let recent_activity = match scene.halo.activity_pulse {
            crate::round::model::RoundActivityPulse::Quiet => "quiet",
            crate::round::model::RoundActivityPulse::Recent { .. } => "recent",
        };
        Self {
            source_diversity: format!("{:?}", scene.halo.source_diversity),
            helper_health: format!("{:?}", scene.halo.helper_health).to_lowercase(),
            recent_activity: recent_activity.to_string(),
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

fn smooth_item_count(layer: &SmoothCompanionLayer) -> usize {
    layer
        .items
        .iter()
        .filter(|item| {
            matches!(
                item,
                crate::presentation::smooth::SmoothLayerItem::LocalCell(_)
            )
        })
        .count()
}

const SMOOTH_SLICE1_REQUIRED_ROLES: [SmoothLayerRole; 19] = [
    SmoothLayerRole::DepthRings,
    SmoothLayerRole::BiomeWash,
    SmoothLayerRole::RoomGlyphs,
    SmoothLayerRole::Ambient,
    SmoothLayerRole::Motes,
    SmoothLayerRole::ActivityGlyphs,
    SmoothLayerRole::PropsBehind,
    SmoothLayerRole::TankLifeBehind,
    SmoothLayerRole::ChestBubble,
    SmoothLayerRole::WallShadow,
    SmoothLayerRole::FloorProjection,
    SmoothLayerRole::PetBody,
    SmoothLayerRole::PerformanceCue,
    SmoothLayerRole::PropsForeground,
    SmoothLayerRole::TankLifeForeground,
    SmoothLayerRole::StatusHalo,
    SmoothLayerRole::TroubleIndicator,
    SmoothLayerRole::MoodAura,
    SmoothLayerRole::DimOverlay,
];

fn smooth_required_role_names() -> Vec<String> {
    SMOOTH_SLICE1_REQUIRED_ROLES
        .into_iter()
        .map(|role| role.as_str().to_string())
        .collect()
}

fn smooth_missing_role_names(plan: &SmoothCompanionScenePlan) -> Vec<String> {
    SMOOTH_SLICE1_REQUIRED_ROLES
        .into_iter()
        .filter(|role| plan.layer_by_role(*role).is_none())
        .map(|role| role.as_str().to_string())
        .collect()
}

fn smooth_abstract_state(vm: &WatchViewModel) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "species".to_string(),
            smooth_species_bucket(vm.pet_render.generated_species).to_string(),
        ),
        (
            "stage".to_string(),
            smooth_stage_bucket(vm.pet_render.stage).to_string(),
        ),
        (
            "mood".to_string(),
            smooth_mood_bucket(vm.pet_render.mood).to_string(),
        ),
        (
            "day_state".to_string(),
            if vm.day_context.asleep {
                "asleep"
            } else {
                "awake"
            }
            .to_string(),
        ),
        (
            "activity_bucket".to_string(),
            if vm.last_feed_pulse_at.is_some() {
                "recent-feed"
            } else {
                "quiet"
            }
            .to_string(),
        ),
        (
            "helper_health".to_string(),
            if vm.source_health.iter().any(|health| {
                matches!(
                    health.status,
                    crate::tui::view_model::SourceStatus::Diagnostic
                )
            }) {
                "trouble"
            } else {
                "ok"
            }
            .to_string(),
        ),
    ])
}

fn smooth_species_bucket(species: crate::pet::generation::Species) -> &'static str {
    match species {
        crate::pet::generation::Species::Fuzz | crate::pet::generation::Species::Blob => {
            "soft-body"
        }
        crate::pet::generation::Species::Ghost | crate::pet::generation::Species::Crystal => {
            "spectral"
        }
        crate::pet::generation::Species::Glitch | crate::pet::generation::Species::Mech => {
            "synthetic"
        }
    }
}

fn smooth_stage_bucket(stage: crate::game::evolution::Stage) -> &'static str {
    match stage {
        crate::game::evolution::Stage::S0
        | crate::game::evolution::Stage::S1
        | crate::game::evolution::Stage::S2 => "early",
        crate::game::evolution::Stage::S3 | crate::game::evolution::Stage::S4 => "grown",
        crate::game::evolution::Stage::S5 | crate::game::evolution::Stage::S6 => "veteran",
    }
}

fn smooth_mood_bucket(mood: crate::game::metabolism::Mood) -> &'static str {
    match mood {
        crate::game::metabolism::Mood::Happy
        | crate::game::metabolism::Mood::Ecstatic
        | crate::game::metabolism::Mood::Content => "settled",
        crate::game::metabolism::Mood::Sleepy => "resting",
        crate::game::metabolism::Mood::Hungry
        | crate::game::metabolism::Mood::Sad
        | crate::game::metabolism::Mood::Wilted => "needs-care",
    }
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
    use crate::presentation::smooth::SmoothLayerRole;
    use crate::round::model::derive_round_scene_model;
    use crate::round::scene::CompanionMotion;
    use crate::round::smooth::build_round_smooth_scene_plan;
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

    #[test]
    fn round_scene_contract_redacts_exact_recent_activity_age() {
        let now = datetime!(2026-06-13 18:00 UTC);
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.last_feed_pulse_at = Some(now - time::Duration::milliseconds(617));
        let scene = derive_round_scene_model(&vm, now);

        let artifact =
            PreviewSceneArtifact::from_round_scene("round-activity-privacy", &scene, now);
        let json = serde_json::to_string(&artifact).expect("serialize round preview artifact");

        assert_eq!(artifact.activity.recent_activity, "recent");
        assert!(!json.contains("age_ms"));
        assert!(!json.contains("617"));
    }

    #[test]
    fn smooth_parity_artifact_flags_missing_required_roles() {
        let now = datetime!(2026-07-08 18:00 UTC);
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = CompanionMotion::default();
        let mut plan = build_round_smooth_scene_plan(&vm, now, 52, 52, &motion, 0);
        plan.layers.retain(|layer| {
            !matches!(
                layer.role,
                SmoothLayerRole::Ambient | SmoothLayerRole::PetBody
            )
        });
        let checksum = plan.classic_flatten_checksum();

        let artifact = PreviewSmoothParityArtifact::from_scene_plan(
            "smooth-parity-missing",
            "fixture",
            &vm,
            checksum,
            &plan,
        );

        assert_eq!(
            artifact.required_roles,
            vec![
                "depth-rings".to_string(),
                "biome-wash".to_string(),
                "room-glyphs".to_string(),
                "ambient".to_string(),
                "motes".to_string(),
                "activity-glyphs".to_string(),
                "props-behind".to_string(),
                "tank-life-behind".to_string(),
                "chest-bubble".to_string(),
                "wall-shadow".to_string(),
                "floor-projection".to_string(),
                "pet-body".to_string(),
                "performance-cue".to_string(),
                "props-foreground".to_string(),
                "tank-life-foreground".to_string(),
                "status-halo".to_string(),
                "trouble-indicator".to_string(),
                "mood-aura".to_string(),
                "dim-overlay".to_string(),
            ]
        );
        assert_eq!(
            artifact.missing_roles,
            vec!["ambient".to_string(), "pet-body".to_string()]
        );
        assert!(!artifact.exact_match);
        assert_eq!(artifact.review_status, "missing-required-roles");
    }

    #[test]
    #[should_panic(expected = "smooth plan should include a pet-body layer for motion proof")]
    fn smooth_motion_artifact_requires_pet_body_layer() {
        let now = datetime!(2026-07-08 18:00 UTC);
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = CompanionMotion::default();
        let mut plan = build_round_smooth_scene_plan(&vm, now, 52, 52, &motion, 0);
        plan.layers
            .retain(|layer| !matches!(layer.role, SmoothLayerRole::PetBody));

        let _artifact = PreviewSmoothMotionArtifact::from_scene_plan(
            "round-smooth-motion",
            0,
            0,
            now,
            0,
            &vm,
            &plan,
            SmoothParallaxPlaneTranslations::default(),
        );
    }
}
