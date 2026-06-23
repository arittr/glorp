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
const REDACTED_RUNTIME_ID: &str = "redacted";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewFrameContract {
    pub scene: Option<PreviewSceneArtifact>,
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
            frame: PreviewRect {
                x: 0,
                y: 0,
                width: 120,
                height: 32,
            },
            content: PreviewRect {
                x: 1,
                y: 1,
                width: 118,
                height: 30,
            },
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
                    clip: PreviewRect {
                        x: 10,
                        y: 5,
                        width: 20,
                        height: 8,
                    },
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
