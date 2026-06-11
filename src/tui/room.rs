use crate::game::habitat::{
    CODEX_SIGNAL_LAMP, HEAVY_SESSION_PLANTER, TOKEN_FRIENDLY_CLOUD_750K, TOKEN_HANGING_VINE_25M,
    TOKEN_LANTERN_10M, TOKEN_MOSS_TUFT_250K, TOKEN_ORBIT_5M, TOKEN_PEBBLE_25K, TOKEN_SHARD_1M,
    TOKEN_SHELL_100K, TOKEN_SPARK_500K, TOKEN_TREASURE_CHEST_2M, WILT_RECOVERY_SPROUT,
};
use crate::storage::state::{EarnedHabitatProp, HabitatPropId};
use crate::tui::day::{in_morning_after_window, resonant_prop_for_day, DayPhase};
use crate::tui::life::WorkWeather;
use crate::tui::view_model::{EarnedHabitatPropView, WatchViewModel};
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};

const BASE_EARNED_PROP_WEIGHT: f32 = 1.0;
const RECENT_EARNED_BONUS: f32 = 0.4;
const RESONANT_BONUS: f32 = 1.2;
const SECONDARY_THRESHOLD: f32 = 0.6;

/// Pure, clock-independent description of the watch room's current character.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomLifeProfile {
    pub biome: RoomBiome,
    pub room_weather: RoomWeatherLayer,
    pub resonant_emitter: Option<PropEmitter>,
    pub pet_performance: PetPerformance,
    pub scene_moments: Vec<SceneMoment>,
    pub identity_prop_ids: Vec<HabitatPropId>,
}

/// Broad aesthetic category contributed by earned habitat props.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RoomBiomeTag {
    Starter,
    Botanical,
    Technical,
    Celestial,
    Artifact,
    Cozy,
}

/// Primary and optional secondary biome for the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomBiome {
    pub primary: RoomBiomeTag,
    pub secondary: Option<RoomBiomeTag>,
}

/// Ambient weather layer rendered behind or through the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomWeatherLayer {
    Clear,
    CacheMist,
    OutputSparks,
    ReasoningPulse,
    Mixed,
}

impl From<WorkWeather> for RoomWeatherLayer {
    fn from(weather: WorkWeather) -> Self {
        match weather {
            WorkWeather::Clear => RoomWeatherLayer::Clear,
            WorkWeather::CacheMist => RoomWeatherLayer::CacheMist,
            WorkWeather::OutputSparks => RoomWeatherLayer::OutputSparks,
            WorkWeather::ReasoningPulse => RoomWeatherLayer::ReasoningPulse,
            WorkWeather::Mixed => RoomWeatherLayer::Mixed,
        }
    }
}

/// Active visual emitter anchored to a specific earned prop.
#[derive(Debug, Clone, PartialEq)]
pub struct PropEmitter {
    pub prop_id: HabitatPropId,
    pub behavior: PropEmitterBehavior,
    pub intensity: f32,
}

/// Visual behavior style for a prop-backed room emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropEmitterBehavior {
    LeafDrift,
    TechnicalPing,
    HopefulSprout,
    WarmHalo,
    CloudDrift,
    OrbitArc,
    ArtifactGlint,
}

/// Pet performance state used to select room scene moments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetPerformance {
    RestedAwake,
    TiredAwake,
    HeavyDayCozy,
    AsleepDreaming,
    CatchUpWake,
    SourceBurstPerk,
}

/// One-shot Tachyonfx-style scene effect triggered by room life conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneMoment {
    pub key: SceneMomentKey,
    pub trigger_id: SceneTriggerId,
    pub target_id: &'static str,
    pub duration_ms: u16,
    pub max_replay_age_ms: u32,
}

/// Stable identifier for a class of scene effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SceneMomentKey {
    FeedSweep,
    PropResonanceRipple,
    DawnWakeWipe,
    HeavySessionShimmer,
    DreamGlimmer,
}

/// Stable, comparable identifier for a specific scene moment trigger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneTriggerId(String);

impl SceneTriggerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Derive a pure room life profile from the current view model and wall clock.
pub fn derive_room_life_profile(vm: &WatchViewModel, now: OffsetDateTime) -> RoomLifeProfile {
    let resonant = resonant_prop_from_vm(vm);
    let biome = derive_biome(&vm.habitat.earned_props, resonant.as_ref(), now);
    let room_weather = vm.life_profile.work_weather.into();
    let pet_performance = pet_performance_for(vm);
    let visible_prop_ids = visible_identity_ids(&vm.habitat.earned_props);
    let resonant_emitter = select_emitter(vm, resonant.as_ref(), &visible_prop_ids);
    let scene_moments = scene_moments_for(vm, now, resonant_emitter.as_ref(), pet_performance);

    RoomLifeProfile {
        biome,
        room_weather,
        resonant_emitter,
        pet_performance,
        scene_moments,
        identity_prop_ids: visible_prop_ids,
    }
}

fn resonant_prop_from_vm(vm: &WatchViewModel) -> Option<HabitatPropId> {
    let earned: Vec<EarnedHabitatProp> = vm
        .habitat
        .earned_props
        .iter()
        .map(|view| EarnedHabitatProp {
            id: view.id.clone(),
            earned_at: view.earned_at,
            source: view.source.clone(),
        })
        .collect();
    resonant_prop_for_day(&vm.day_context, &earned)
}

fn derive_biome(
    earned: &[EarnedHabitatPropView],
    resonant: Option<&HabitatPropId>,
    now: OffsetDateTime,
) -> RoomBiome {
    let mut weights: HashMap<RoomBiomeTag, f32> = HashMap::new();
    let recent_cutoff = now - Duration::hours(24);

    for prop in earned {
        let base = prop.display_priority as f32 / 100.0 + BASE_EARNED_PROP_WEIGHT;
        let recent = if prop.earned_at >= recent_cutoff {
            RECENT_EARNED_BONUS
        } else {
            0.0
        };
        let resonant_bonus = if resonant == Some(&prop.id) {
            RESONANT_BONUS
        } else {
            0.0
        };
        for &tag in tags_for_prop(prop.id.as_str()) {
            *weights.entry(tag).or_insert(0.0) += base + recent + resonant_bonus;
        }
    }

    if weights.is_empty() {
        return RoomBiome {
            primary: RoomBiomeTag::Starter,
            secondary: None,
        };
    }

    let mut entries: Vec<(RoomBiomeTag, f32)> = weights.into_iter().collect();
    entries.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let primary = entries[0].0;
    let primary_weight = entries[0].1;
    let secondary = entries
        .iter()
        .skip(1)
        .find(|(_, w)| *w >= SECONDARY_THRESHOLD * primary_weight)
        .map(|(tag, _)| *tag);

    RoomBiome { primary, secondary }
}

fn pet_performance_for(vm: &WatchViewModel) -> PetPerformance {
    if vm.day_context.asleep {
        return PetPerformance::AsleepDreaming;
    }
    if vm.day_context.wake_resume.is_some() {
        return PetPerformance::CatchUpWake;
    }
    if vm.life_profile.burst_level > 0.9 {
        return PetPerformance::SourceBurstPerk;
    }
    if vm.day_context.tiredness > 0.6 {
        return PetPerformance::TiredAwake;
    }
    if matches!(vm.day_context.day_phase, DayPhase::Night) {
        return PetPerformance::RestedAwake;
    }
    if vm.day_context.is_weekend
        && vm.day_context.weekend_share <= crate::tui::day::WEEKEND_QUIET_SHARE
    {
        return PetPerformance::HeavyDayCozy;
    }
    PetPerformance::RestedAwake
}

fn visible_identity_ids(earned: &[EarnedHabitatPropView]) -> Vec<HabitatPropId> {
    let mut sorted: Vec<&EarnedHabitatPropView> = earned.iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.display_priority));
    sorted.iter().map(|p| p.id.clone()).collect()
}

fn select_emitter(
    vm: &WatchViewModel,
    resonant: Option<&HabitatPropId>,
    visible_ids: &[HabitatPropId],
) -> Option<PropEmitter> {
    if let Some(reaction) = vm
        .life_profile
        .prop_reactions
        .iter()
        .find(|r| visible_ids.contains(&r.prop_id))
    {
        return Some(PropEmitter {
            prop_id: reaction.prop_id.clone(),
            behavior: emitter_behavior_for_prop(reaction.prop_id.as_str()),
            intensity: reaction.intensity.clamp(0.0, 1.0),
        });
    }

    if let Some(id) = resonant {
        if visible_ids.contains(id) {
            return Some(PropEmitter {
                prop_id: id.clone(),
                behavior: emitter_behavior_for_prop(id.as_str()),
                intensity: 0.6,
            });
        }
    }

    None
}

fn prop_target_id(id: &str) -> &'static str {
    crate::tui::component::habitat_props::prop_effect_target_path(id)
        .map(|path| path.as_str())
        .unwrap_or("watch.prop.effect")
}

fn scene_moments_for(
    vm: &WatchViewModel,
    now: OffsetDateTime,
    emitter: Option<&PropEmitter>,
    performance: PetPerformance,
) -> Vec<SceneMoment> {
    let mut moments = Vec::new();
    if vm.life_profile.burst_level > 0.0
        && !vm.day_context.asleep
        && vm
            .last_feed_pulse_at
            .is_some_and(|pulse| now - pulse <= Duration::seconds(8))
    {
        moments.push(SceneMoment {
            key: SceneMomentKey::FeedSweep,
            trigger_id: SceneTriggerId::new(format!(
                "feed:{}",
                vm.last_feed_pulse_at.unwrap().unix_timestamp()
            )),
            target_id: "watch.pet.effect",
            duration_ms: 500,
            max_replay_age_ms: 8_000,
        });
    }
    if let Some(emitter) = emitter {
        moments.push(SceneMoment {
            key: SceneMomentKey::PropResonanceRipple,
            trigger_id: SceneTriggerId::new(format!(
                "prop:{}:{}",
                emitter.prop_id.as_str(),
                vm.day_context.date_seed
            )),
            target_id: prop_target_id(emitter.prop_id.as_str()),
            duration_ms: 700,
            max_replay_age_ms: 3_600_000,
        });
    }
    if in_morning_after_window(&vm.day_context, now)
        && matches!(performance, PetPerformance::CatchUpWake)
    {
        moments.push(SceneMoment {
            key: SceneMomentKey::DawnWakeWipe,
            trigger_id: SceneTriggerId::new(format!("wake:{}", vm.day_context.date_seed)),
            target_id: "watch.room.effect",
            duration_ms: 900,
            max_replay_age_ms: 3_600_000,
        });
    }
    moments
}

fn tags_for_prop(id: &str) -> &'static [RoomBiomeTag] {
    match id {
        TOKEN_MOSS_TUFT_250K
        | TOKEN_HANGING_VINE_25M
        | HEAVY_SESSION_PLANTER
        | WILT_RECOVERY_SPROUT => &[RoomBiomeTag::Botanical, RoomBiomeTag::Cozy],
        CODEX_SIGNAL_LAMP => &[RoomBiomeTag::Technical],
        TOKEN_ORBIT_5M => &[RoomBiomeTag::Technical, RoomBiomeTag::Celestial],
        TOKEN_SPARK_500K | TOKEN_FRIENDLY_CLOUD_750K | TOKEN_LANTERN_10M => {
            &[RoomBiomeTag::Celestial, RoomBiomeTag::Cozy]
        }
        TOKEN_PEBBLE_25K | TOKEN_SHELL_100K | TOKEN_SHARD_1M | TOKEN_TREASURE_CHEST_2M => {
            &[RoomBiomeTag::Artifact]
        }
        _ => &[],
    }
}

fn emitter_behavior_for_prop(id: &str) -> PropEmitterBehavior {
    match id {
        HEAVY_SESSION_PLANTER => PropEmitterBehavior::LeafDrift,
        CODEX_SIGNAL_LAMP => PropEmitterBehavior::TechnicalPing,
        WILT_RECOVERY_SPROUT => PropEmitterBehavior::HopefulSprout,
        TOKEN_LANTERN_10M => PropEmitterBehavior::WarmHalo,
        TOKEN_FRIENDLY_CLOUD_750K => PropEmitterBehavior::CloudDrift,
        TOKEN_ORBIT_5M => PropEmitterBehavior::OrbitArc,
        _ => PropEmitterBehavior::ArtifactGlint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::{
        CODEX_SIGNAL_LAMP, HEAVY_SESSION_PLANTER, TOKEN_LANTERN_10M, TOKEN_MOSS_TUFT_250K,
        TOKEN_ORBIT_5M, TOKEN_SHELL_100K,
    };
    use crate::storage::state::{HabitatPropId, HabitatPropSource};
    use crate::tui::day::{DayContext, DayPhase};
    use crate::tui::life::{
        IdleLifeState, PetLifeProfile, PropReaction, PropReactionKind, WorkWeather,
    };
    use crate::tui::view_model::{EarnedHabitatPropView, HabitatView, WatchViewModel};
    use time::macros::datetime;

    fn earned(id: &str, priority: i16) -> EarnedHabitatPropView {
        EarnedHabitatPropView {
            id: HabitatPropId::new(id),
            earned_at: datetime!(2026-06-10 12:00 UTC),
            kind: crate::game::habitat::catalog_prop_by_str(id).unwrap().kind,
            display_priority: priority,
            source: HabitatPropSource::LifetimeTokens { threshold: 1.0 },
        }
    }

    fn vm_with_props(props: Vec<EarnedHabitatPropView>) -> WatchViewModel {
        let mut vm = WatchViewModel::fixture();
        vm.habitat = HabitatView {
            earned_props: props,
        };
        vm.day_context = DayContext {
            day_phase: DayPhase::Day,
            mature: true,
            ..DayContext::default()
        };
        vm
    }

    #[test]
    fn biome_uses_all_earned_props_not_visible_rotation_only() {
        let vm = vm_with_props(vec![
            earned(HEAVY_SESSION_PLANTER, 80),
            earned(TOKEN_MOSS_TUFT_250K, 25),
            earned(CODEX_SIGNAL_LAMP, 70),
            earned(TOKEN_ORBIT_5M, 50),
            earned(TOKEN_SHELL_100K, 20),
        ]);

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

        // Both CODEX_SIGNAL_LAMP and TOKEN_ORBIT_5M contribute Technical weight,
        // so Technical edges out Botanical as the primary biome here.
        assert_eq!(profile.biome.primary, RoomBiomeTag::Technical);
        assert!(profile.biome.secondary.is_some());
        assert!(
            profile
                .identity_prop_ids
                .contains(&HabitatPropId::from(HEAVY_SESSION_PLANTER)),
            "high-weight earned props should anchor the room identity"
        );
    }

    #[test]
    fn starter_room_has_no_emitter_or_scene_moments() {
        let vm = vm_with_props(Vec::new());

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

        assert_eq!(profile.biome.primary, RoomBiomeTag::Starter);
        assert_eq!(profile.resonant_emitter, None);
        assert!(profile.scene_moments.is_empty());
    }

    #[test]
    fn live_prop_reaction_selects_visible_emitter() {
        let mut vm = vm_with_props(vec![
            earned(CODEX_SIGNAL_LAMP, 70),
            earned(TOKEN_LANTERN_10M, 60),
        ]);
        vm.life_profile = PetLifeProfile {
            activity_level: 1.2,
            burst_level: 0.9,
            work_weather: WorkWeather::OutputSparks,
            prop_reactions: vec![PropReaction {
                prop_id: HabitatPropId::from(CODEX_SIGNAL_LAMP),
                intensity: 0.8,
                kind: PropReactionKind::Glow,
            }],
            idle: IdleLifeState {
                idle_minutes: 0,
                is_recently_active: true,
            },
            ..PetLifeProfile::default()
        };
        vm.last_feed_pulse_at = Some(datetime!(2026-06-11 09:59:55 UTC));

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

        assert_eq!(
            profile
                .resonant_emitter
                .as_ref()
                .map(|emitter| emitter.prop_id.clone()),
            Some(HabitatPropId::from(CODEX_SIGNAL_LAMP))
        );
        assert!(profile
            .scene_moments
            .iter()
            .any(|moment| moment.key == SceneMomentKey::FeedSweep));
    }

    #[test]
    fn pet_performance_sleep_beats_live_burst() {
        let mut vm = vm_with_props(vec![earned(TOKEN_LANTERN_10M, 60)]);
        vm.day_context = DayContext {
            asleep: true,
            mature: true,
            ..vm.day_context
        };
        vm.life_profile.burst_level = 1.0;

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 03:00 UTC));

        assert_eq!(profile.pet_performance, PetPerformance::AsleepDreaming);
        assert!(
            !profile
                .scene_moments
                .iter()
                .any(|moment| moment.key == SceneMomentKey::FeedSweep),
            "sleeping room should not fake a live feed burst"
        );
    }
}
