use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::generation::Species;
use crate::pet::render::StyledSegment;
use crate::storage::state::HabitatPropId;
use crate::tui::day::DayPhase;
use crate::tui::identity::SourceDiversity;
use crate::tui::life::WorkWeather;
use crate::tui::room::{RoomBiome, RoomDialectKey};
use crate::tui::view_model::WatchViewModel;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct RoundSceneModel {
    pub pet: RoundPetModel,
    pub room: RoundRoomModel,
    pub halo: RoundHaloModel,
    pub lifecycle: RoundLifecycleModel,
    pub moments: Vec<RoundSceneMoment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundPetModel {
    pub seed: String,
    pub species: Species,
    pub stage: Stage,
    pub mood: Mood,
    pub art_lines: Vec<String>,
    pub art_spans: Vec<StyledSegment>,
    pub asleep: bool,
    pub breath_offset_y: u8,
    pub facing: i8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundRoomModel {
    pub biome: RoomBiome,
    pub dialect: RoomDialectKey,
    pub work_weather: WorkWeather,
    pub day_phase: DayPhase,
    pub prop_landmarks: Vec<HabitatPropId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundHaloModel {
    pub activity_pulse: RoundActivityPulse,
    pub source_diversity: RoundSourceDiversity,
    pub helper_health: RoundHelperHealth,
    pub vitals: RoundVitals,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundVitals {
    pub fed: RoundVitalBucket,
    pub happiness: RoundVitalBucket,
    pub energy: RoundVitalBucket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundVitalBucket {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundSourceDiversity {
    Quiet,
    Single,
    Dual,
    Ensemble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundHelperHealth {
    Ok,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundActivityPulse {
    Quiet,
    Recent { age_ms: u16 },
}

impl RoundActivityPulse {
    pub fn is_quiet(self) -> bool {
        matches!(self, RoundActivityPulse::Quiet)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundLifecycleModel {
    // Lifecycle intent mirrors pet render state for V1; keep separate fields
    // so future lifecycle transitions (calm, dawn-wake, etc.) can diverge.
    pub asleep: bool,
    pub calm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundSceneMoment {
    pub kind: RoundSceneMomentKind,
    pub trigger_id: String,
    pub anchor: RoundMomentAnchor,
    pub duration_ms: u16,
    pub replay_policy: RoundReplayPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundSceneMomentKind {
    FeedSweep,
    PropResonance,
    DawnWake,
    DreamGlimmer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundMomentAnchor {
    Halo,
    Pet,
    Room,
    Prop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundReplayPolicy {
    OncePerTrigger,
}

pub fn derive_round_scene_model(vm: &WatchViewModel, now: OffsetDateTime) -> RoundSceneModel {
    let room_profile = crate::presentation::companion_scene::input::derive_room_profile(vm, now);
    let pulse = derive_activity_pulse(vm, now);
    let moments = derive_moments(vm, &pulse);
    RoundSceneModel {
        pet: RoundPetModel {
            seed: vm.pet_render.seed.clone(),
            species: vm.pet_render.generated_species,
            stage: vm.pet_render.stage,
            mood: vm.pet_render.mood,
            art_lines: vm.pet_art.clone(),
            art_spans: vm.pet_spans.clone(),
            asleep: vm.day_context.asleep,
            breath_offset_y: vm.breath_offset_y,
            facing: vm.facing,
        },
        room: RoundRoomModel {
            biome: room_profile.biome,
            dialect: room_profile.species_dialect.key,
            work_weather: vm.life_profile.work_weather,
            day_phase: vm.day_context.day_phase,
            prop_landmarks: room_profile.identity_prop_ids.into_iter().take(2).collect(),
        },
        halo: RoundHaloModel {
            activity_pulse: pulse,
            source_diversity: match vm.activity_identity.source_diversity {
                SourceDiversity::Quiet => RoundSourceDiversity::Quiet,
                SourceDiversity::SingleLane => RoundSourceDiversity::Single,
                SourceDiversity::DualLane => RoundSourceDiversity::Dual,
                SourceDiversity::Ensemble => RoundSourceDiversity::Ensemble,
            },
            helper_health: match crate::presentation::companion_scene::input::derive_helper_health(
                vm,
            ) {
                crate::presentation::companion_scene::input::SemanticHelperHealth::Ok => {
                    RoundHelperHealth::Ok
                }
                crate::presentation::companion_scene::input::SemanticHelperHealth::Trouble => {
                    RoundHelperHealth::Trouble
                }
            },
            vitals: RoundVitals {
                fed: vital_bucket(vm.fed),
                happiness: vital_bucket(vm.happiness),
                energy: vital_bucket(vm.energy),
            },
        },
        lifecycle: RoundLifecycleModel {
            asleep: vm.day_context.asleep,
            calm: vm.life_profile.calm_mode || vm.day_context.asleep,
        },
        moments,
    }
}

fn derive_activity_pulse(vm: &WatchViewModel, now: OffsetDateTime) -> RoundActivityPulse {
    match crate::presentation::companion_scene::input::derive_activity_pulse(vm, now) {
        crate::presentation::companion_scene::input::SemanticActivityPulse::Quiet => {
            RoundActivityPulse::Quiet
        }
        crate::presentation::companion_scene::input::SemanticActivityPulse::Recent { age_ms } => {
            RoundActivityPulse::Recent { age_ms }
        }
    }
}

fn derive_moments(vm: &WatchViewModel, pulse: &RoundActivityPulse) -> Vec<RoundSceneMoment> {
    let mut moments = Vec::new();
    if !pulse.is_quiet() {
        moments.push(RoundSceneMoment {
            kind: RoundSceneMomentKind::FeedSweep,
            trigger_id: "feed-pulse".to_string(),
            anchor: RoundMomentAnchor::Halo,
            duration_ms: 1200,
            replay_policy: RoundReplayPolicy::OncePerTrigger,
        });
    }
    if vm.day_context.asleep {
        moments.push(RoundSceneMoment {
            kind: RoundSceneMomentKind::DreamGlimmer,
            trigger_id: "asleep".to_string(),
            anchor: RoundMomentAnchor::Room,
            duration_ms: 1600,
            replay_policy: RoundReplayPolicy::OncePerTrigger,
        });
    }
    moments
}

fn vital_bucket(value: f64) -> RoundVitalBucket {
    match crate::presentation::companion_scene::input::derive_vital_bucket(value) {
        crate::presentation::companion_scene::input::SemanticVitalBucket::Low => {
            RoundVitalBucket::Low
        }
        crate::presentation::companion_scene::input::SemanticVitalBucket::Medium => {
            RoundVitalBucket::Medium
        }
        crate::presentation::companion_scene::input::SemanticVitalBucket::High => {
            RoundVitalBucket::High
        }
    }
}
