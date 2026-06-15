use crate::pet::render::StyledSegment;
use crate::round::draw::{RoundDrawCommand, RoundDrawKind};
use crate::round::layout::{RoundAnchor, RoundAnchorKind, RoundMotionBudget, RoundSceneLayout};
use crate::round::model::RoundSceneModel;
use crate::tui::component::PreviewLayout;
use crate::tui::room::{biome_symbols, derive_room_life_profile, RoomSpeciesDialect};
use crate::tui::view_model::WatchViewModel;
use serde::Serialize;
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub const CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewFrameContract {
    pub scene: Option<PreviewSceneArtifact>,
    pub round_layout: Option<PreviewRoundLayoutArtifact>,
    pub round_commands: Option<PreviewRoundCommandsArtifact>,
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
            pet: PreviewPetArtifact::from_watch(vm),
            room: PreviewRoomArtifact {
                primary_biome: format!("{:?}", room_profile.biome.primary),
                secondary_biome: room_profile.biome.secondary.map(|tag| format!("{tag:?}")),
                species_dialect: dialect.key.as_str().to_string(),
                dialect_status: Some(dialect.status.as_str().to_string()),
                work_weather: format!("{:?}", vm.life_profile.work_weather),
                day_phase: format!("{:?}", vm.day_context.day_phase),
                prop_landmarks: room_profile
                    .identity_prop_ids
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
                seed: scene.pet.seed.clone(),
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
                prop_landmarks: scene
                    .room
                    .prop_landmarks
                    .iter()
                    .map(|id| id.as_str().to_string())
                    .collect(),
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
    fn from_watch(vm: &WatchViewModel) -> Self {
        Self {
            seed: vm.pet_render.seed.clone(),
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
        .map(|(id, target)| {
            (
                id.to_string(),
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

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundLayoutArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub fixture_id: String,
    pub aperture: PreviewRoundApertureArtifact,
    pub safe_inner_radius: f32,
    pub detail_level: String,
    pub pet_anchor: PreviewRoundAnchorArtifact,
    pub prop_anchors: Vec<PreviewRoundAnchorArtifact>,
    pub halo_anchors: Vec<PreviewRoundAnchorArtifact>,
    pub motion_budget: PreviewRoundMotionBudgetArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundApertureArtifact {
    pub width: u16,
    pub height: u16,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundAnchorArtifact {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoundMotionBudgetArtifact {
    pub pet_breath: bool,
    pub pet_blink: bool,
    pub activity_sweep: bool,
    pub prop_resonance: bool,
}

impl PreviewRoundLayoutArtifact {
    pub fn from_layout(frame_id: &str, layout: &RoundSceneLayout) -> Self {
        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture_id: frame_id.to_string(),
            aperture: PreviewRoundApertureArtifact {
                width: layout.aperture.width,
                height: layout.aperture.height,
                center_x: layout.aperture.center_x,
                center_y: layout.aperture.center_y,
                radius: layout.aperture.radius,
            },
            safe_inner_radius: layout.safe_inner_radius,
            detail_level: format!("{:?}", layout.detail_level).to_lowercase(),
            pet_anchor: round_anchor_artifact(&layout.pet_anchor),
            prop_anchors: layout
                .prop_anchors
                .iter()
                .map(round_anchor_artifact)
                .collect(),
            halo_anchors: layout
                .halo_anchors
                .iter()
                .map(round_anchor_artifact)
                .collect(),
            motion_budget: round_motion_budget_artifact(layout.motion_budget),
        }
    }
}

fn round_anchor_artifact(anchor: &RoundAnchor) -> PreviewRoundAnchorArtifact {
    PreviewRoundAnchorArtifact {
        kind: match anchor.kind {
            RoundAnchorKind::Pet => "pet",
            RoundAnchorKind::Prop => "prop",
            RoundAnchorKind::ActivityPulse => "activity-pulse",
            RoundAnchorKind::SourceDiversity => "source-diversity",
            RoundAnchorKind::Vital => "vital",
            RoundAnchorKind::HelperTrouble => "helper-trouble",
        }
        .to_string(),
        x: anchor.x,
        y: anchor.y,
        radius: anchor.radius,
    }
}

fn round_motion_budget_artifact(budget: RoundMotionBudget) -> PreviewRoundMotionBudgetArtifact {
    PreviewRoundMotionBudgetArtifact {
        pet_breath: budget.pet_breath,
        pet_blink: budget.pet_blink,
        activity_sweep: budget.activity_sweep,
        prop_resonance: budget.prop_resonance,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundCommandsArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub fixture_id: String,
    pub privacy_projection: PreviewPrivacyProjection,
    pub command_counts: BTreeMap<String, usize>,
    pub room: PreviewRoundRoomCommandSummary,
    pub pet: PreviewRoundPetCommandSummary,
    pub commands: Vec<PreviewRoundCommandArtifact>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoundRoomCommandSummary {
    pub glyph_vocabulary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewRoundPetCommandSummary {
    pub text: String,
    pub span_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewRoundCommandArtifact {
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub label: Option<String>,
    pub text_len: usize,
    pub span_count: usize,
    pub color_rgba: [f32; 4],
}

impl PreviewRoundCommandsArtifact {
    pub fn from_commands(
        frame_id: &str,
        scene: &RoundSceneModel,
        commands: &[RoundDrawCommand],
    ) -> Self {
        let mut command_counts = BTreeMap::new();
        for command in commands {
            *command_counts
                .entry(round_draw_kind_name(command.kind).to_string())
                .or_insert(0) += 1;
        }
        let pet = commands
            .iter()
            .find(|command| command.kind == RoundDrawKind::PetGlyph)
            .map(|command| PreviewRoundPetCommandSummary {
                text: command.text.clone().unwrap_or_default(),
                span_count: command.spans.len(),
            })
            .unwrap_or_else(|| PreviewRoundPetCommandSummary {
                text: String::new(),
                span_count: 0,
            });
        let room_dialect = RoomSpeciesDialect::for_species(scene.pet.species);
        let glyph_vocabulary = biome_symbols(scene.room.biome.primary, room_dialect)
            .iter()
            .map(|ch| ch.to_string())
            .collect();

        Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            fixture_id: frame_id.to_string(),
            privacy_projection: PreviewPrivacyProjection::sanitized("round-preview"),
            command_counts,
            room: PreviewRoundRoomCommandSummary { glyph_vocabulary },
            pet,
            commands: commands.iter().map(round_command_artifact).collect(),
        }
    }
}

fn round_command_artifact(command: &RoundDrawCommand) -> PreviewRoundCommandArtifact {
    PreviewRoundCommandArtifact {
        kind: round_draw_kind_name(command.kind).to_string(),
        x: command.x,
        y: command.y,
        radius: command.radius,
        label: command.label.map(|ch| ch.to_string()),
        text_len: command.text.as_ref().map(|text| text.len()).unwrap_or(0),
        span_count: command.spans.len(),
        color_rgba: [
            command.color.0,
            command.color.1,
            command.color.2,
            command.color.3,
        ],
    }
}

fn round_draw_kind_name(kind: RoundDrawKind) -> &'static str {
    match kind {
        RoundDrawKind::Background => "background",
        RoundDrawKind::RoomGlyph => "room-glyph",
        RoundDrawKind::PropGlyph => "prop-glyph",
        RoundDrawKind::PetGlyph => "pet-glyph",
        RoundDrawKind::Halo => "halo",
        RoundDrawKind::Trouble => "trouble",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::draw::RoundColor;
    use crate::round::layout::{
        RoundAnchor, RoundAnchorKind, RoundAperture, RoundDetailLevel, RoundMotionBudget,
        RoundSceneLayout,
    };
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
        assert_eq!(artifact.targets["watch.pet"].role, "Pet");
        assert_eq!(artifact.targets["watch.pet"].layer, "component");
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
        assert_eq!(artifact.pet.seed, vm.pet_render.seed);
        assert!(artifact.pet.asleep);
        assert_eq!(artifact.room.species_dialect, "fuzz");
        assert_eq!(artifact.room.dialect_status, None);
        assert_eq!(artifact.targets, BTreeMap::new());
    }

    #[test]
    fn round_layout_contract_names_anchors_and_motion_budget() {
        let layout = RoundSceneLayout {
            aperture: RoundAperture::new(52, 40),
            safe_inner_radius: 12.5,
            detail_level: RoundDetailLevel::Full,
            pet_anchor: RoundAnchor {
                kind: RoundAnchorKind::Pet,
                x: 25.0,
                y: 20.0,
                radius: 8.0,
            },
            prop_anchors: vec![RoundAnchor {
                kind: RoundAnchorKind::Prop,
                x: 12.0,
                y: 24.0,
                radius: 2.0,
            }],
            halo_anchors: vec![RoundAnchor {
                kind: RoundAnchorKind::ActivityPulse,
                x: 25.0,
                y: 0.0,
                radius: 1.0,
            }],
            motion_budget: RoundMotionBudget {
                pet_breath: true,
                pet_blink: true,
                activity_sweep: false,
                prop_resonance: true,
            },
        };

        let artifact = PreviewRoundLayoutArtifact::from_layout("round-layout", &layout);

        assert_eq!(artifact.schema_version, CONTRACT_SCHEMA_VERSION);
        assert_eq!(artifact.frame_id, "round-layout");
        assert_eq!(artifact.fixture_id, "round-layout");
        assert_eq!(artifact.aperture.width, 52);
        assert_eq!(artifact.aperture.height, 40);
        assert_eq!(artifact.detail_level, "full");
        assert_eq!(artifact.pet_anchor.kind, "pet");
        assert_eq!(artifact.prop_anchors[0].kind, "prop");
        assert_eq!(artifact.halo_anchors[0].kind, "activity-pulse");
        assert!(artifact.motion_budget.pet_breath);
        assert!(!artifact.motion_budget.activity_sweep);
    }

    #[test]
    fn round_command_contract_summarizes_draw_commands() {
        let now = datetime!(2026-06-13 18:00 UTC);
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, now);
        let commands = vec![
            RoundDrawCommand {
                kind: RoundDrawKind::Background,
                x: 26.0,
                y: 20.0,
                radius: 18.0,
                label: None,
                text: None,
                spans: Vec::new(),
                color: RoundColor(0.1, 0.2, 0.3, 1.0),
            },
            RoundDrawCommand {
                kind: RoundDrawKind::RoomGlyph,
                x: 10.0,
                y: 12.0,
                radius: 2.0,
                label: Some('~'),
                text: None,
                spans: Vec::new(),
                color: RoundColor(0.4, 0.5, 0.6, 0.55),
            },
            RoundDrawCommand {
                kind: RoundDrawKind::PetGlyph,
                x: 26.0,
                y: 22.0,
                radius: 9.0,
                label: None,
                text: Some("AB\n C".to_string()),
                spans: vm.pet_spans.clone(),
                color: RoundColor(0.9, 0.8, 0.7, 1.0),
            },
        ];

        let artifact =
            PreviewRoundCommandsArtifact::from_commands("round-commands", &scene, &commands);

        assert_eq!(artifact.schema_version, CONTRACT_SCHEMA_VERSION);
        assert_eq!(artifact.frame_id, "round-commands");
        assert_eq!(artifact.fixture_id, "round-commands");
        assert_eq!(artifact.privacy_projection.surface, "round-preview");
        assert_eq!(artifact.command_counts["background"], 1);
        assert_eq!(artifact.command_counts["room-glyph"], 1);
        assert_eq!(artifact.command_counts["pet-glyph"], 1);
        assert_eq!(artifact.pet.text, "AB\n C");
        assert_eq!(artifact.pet.span_count, vm.pet_spans.len());
        assert!(!artifact.room.glyph_vocabulary.is_empty());
        assert_eq!(artifact.commands[1].kind, "room-glyph");
        assert_eq!(artifact.commands[1].label.as_deref(), Some("~"));
        assert_eq!(artifact.commands[1].text_len, 0);
        assert_eq!(artifact.commands[2].text_len, 5);
        assert_eq!(artifact.commands[2].span_count, vm.pet_spans.len());
        assert_eq!(artifact.commands[2].color_rgba, [0.9, 0.8, 0.7, 1.0]);
    }
}
