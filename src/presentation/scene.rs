use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};
use crate::presentation::room::PresentationRoom;
use crate::presentation::target::SurfaceTargetId;
use crate::tui::room::derive_room_life_profile;
use crate::tui::view_model::{SourceStatus, WatchViewModel};
use std::collections::BTreeMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationScene {
    pub privacy: PrivacyProjection,
    pub pet: PresentationPetSnapshot,
    pub room: PresentationRoom,
    pub activity: PresentationActivitySnapshot,
    pub targets: BTreeMap<SurfaceTargetId, PresentationTargetAnchor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationPetSnapshot {
    pub seed: String,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub art_lines: Vec<String>,
    pub span_count: usize,
    pub facing: i8,
    pub breath_offset_y: u8,
    pub asleep: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationActivitySnapshot {
    pub source_diversity: String,
    pub helper_health: PresentationHelperHealth,
    pub recent_activity: bool,
    pub fed_bucket: PresentationVitalBucket,
    pub happiness_bucket: PresentationVitalBucket,
    pub energy_bucket: PresentationVitalBucket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationHelperHealth {
    Ok,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationVitalBucket {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationTargetAnchor {
    pub layer: PresentationLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationLayer {
    Room,
    Prop,
    Pet,
    Halo,
    Overlay,
}

impl PresentationScene {
    pub fn from_watch_view_model(
        vm: &WatchViewModel,
        now: OffsetDateTime,
        surface: PresentationSurface,
    ) -> Self {
        let privacy = PrivacyProjection::for_surface(surface);
        let room_profile = derive_room_life_profile(vm, now);
        let redacts_runtime_ids = redacts_runtime_ids(privacy);
        let mut room = PresentationRoom::from_profile(&room_profile);
        if redacts_runtime_ids {
            room.prop_landmarks.clear();
        }

        Self {
            privacy,
            pet: PresentationPetSnapshot {
                seed: if redacts_runtime_ids {
                    "redacted".to_string()
                } else {
                    vm.pet_render.seed.clone()
                },
                species: vm.pet_render.generated_species.as_str().to_string(),
                stage: format!("{:?}", vm.pet_render.stage).to_lowercase(),
                mood: format!("{:?}", vm.pet_render.mood).to_lowercase(),
                art_lines: vm.pet_art.clone(),
                span_count: vm.pet_spans.len(),
                facing: vm.facing,
                breath_offset_y: vm.breath_offset_y,
                asleep: vm.day_context.asleep,
            },
            room,
            activity: PresentationActivitySnapshot {
                source_diversity: format!("{:?}", vm.activity_identity.source_diversity),
                helper_health: if vm
                    .source_health
                    .iter()
                    .any(|health| health.status == SourceStatus::Diagnostic)
                {
                    PresentationHelperHealth::Trouble
                } else {
                    PresentationHelperHealth::Ok
                },
                recent_activity: vm.last_feed_pulse_at.is_some(),
                fed_bucket: vital_bucket(vm.fed),
                happiness_bucket: vital_bucket(vm.happiness),
                energy_bucket: vital_bucket(vm.energy),
            },
            targets: BTreeMap::from([
                (
                    SurfaceTargetId::new("room.effect"),
                    PresentationTargetAnchor { layer: PresentationLayer::Room },
                ),
                (
                    SurfaceTargetId::new("pet.art"),
                    PresentationTargetAnchor { layer: PresentationLayer::Pet },
                ),
            ]),
        }
    }
}

fn redacts_runtime_ids(privacy: PrivacyProjection) -> bool {
    !privacy.source_names_visible && !privacy.exact_counts_visible
}

fn vital_bucket(value: f64) -> PresentationVitalBucket {
    if value < 0.34 {
        PresentationVitalBucket::Low
    } else if value < 0.67 {
        PresentationVitalBucket::Medium
    } else {
        PresentationVitalBucket::High
    }
}
