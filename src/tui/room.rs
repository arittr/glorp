use crate::game::habitat::{
    catalog_prop_by_str, CODEX_SIGNAL_LAMP, HEAVY_SESSION_PLANTER, TOKEN_FRIENDLY_CLOUD_750K,
    TOKEN_HANGING_VINE_25M, TOKEN_LANTERN_10M, TOKEN_MOSS_TUFT_250K, TOKEN_ORBIT_5M,
    TOKEN_PEBBLE_25K, TOKEN_SHARD_1M, TOKEN_SHELL_100K, TOKEN_SPARK_500K, TOKEN_TREASURE_CHEST_2M,
    WILT_RECOVERY_SPROUT,
};
use crate::pet::generation::Species;
use crate::storage::state::{EarnedHabitatProp, HabitatPropId};
use crate::tui::day::{in_morning_after_window, resonant_prop_for_day, DayContext, DayPhase};
use crate::tui::life::WorkWeather;
use crate::tui::view_model::{EarnedHabitatPropView, WatchViewModel};
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};

use crate::tui::style::ColorCapability;

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
    pub species_dialect: RoomSpeciesDialect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomDialectKey {
    Fuzz,
    Blob,
    Ghost,
    Glitch,
    Crystal,
    Mech,
}

impl RoomDialectKey {
    pub const fn as_str(self) -> &'static str {
        match self {
            RoomDialectKey::Fuzz => "fuzz",
            RoomDialectKey::Blob => "blob",
            RoomDialectKey::Ghost => "ghost",
            RoomDialectKey::Glitch => "glitch",
            RoomDialectKey::Crystal => "crystal",
            RoomDialectKey::Mech => "mech",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomDialectStatus {
    Tuned,
    Default,
}

impl RoomDialectStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            RoomDialectStatus::Tuned => "tuned",
            RoomDialectStatus::Default => "default",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomSpeciesDialect {
    pub species: Species,
    pub key: RoomDialectKey,
    pub status: RoomDialectStatus,
}

impl RoomSpeciesDialect {
    pub const fn for_species(species: Species) -> Self {
        let key = match species {
            Species::Fuzz => RoomDialectKey::Fuzz,
            Species::Blob => RoomDialectKey::Blob,
            Species::Ghost => RoomDialectKey::Ghost,
            Species::Glitch => RoomDialectKey::Glitch,
            Species::Crystal => RoomDialectKey::Crystal,
            Species::Mech => RoomDialectKey::Mech,
        };
        let status = match species {
            Species::Glitch | Species::Crystal => RoomDialectStatus::Tuned,
            Species::Fuzz | Species::Blob | Species::Ghost | Species::Mech => {
                RoomDialectStatus::Default
            }
        };
        Self {
            species,
            key,
            status,
        }
    }
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
    // TODO: reserved for future replay-gating logic
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

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Derive a pure room life profile from the current view model and wall clock.
pub fn derive_room_life_profile(vm: &WatchViewModel, now: OffsetDateTime) -> RoomLifeProfile {
    let resonant = resonant_prop_from_vm(vm);
    let biome = derive_biome(&vm.habitat.earned_props, resonant.as_ref(), now);
    let room_weather = vm.life_profile.work_weather.into();
    let pet_performance = pet_performance_for(vm, now);
    let visible_prop_ids = visible_identity_ids(&vm.habitat.earned_props);
    let resonant_emitter = select_emitter(vm, resonant.as_ref(), &visible_prop_ids);
    let scene_moments = scene_moments_for(vm, now, resonant_emitter.as_ref(), pet_performance);
    let species_dialect = RoomSpeciesDialect::for_species(vm.pet_render.generated_species);

    RoomLifeProfile {
        biome,
        room_weather,
        resonant_emitter,
        pet_performance,
        scene_moments,
        identity_prop_ids: visible_prop_ids,
        species_dialect,
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

/// Select the pet performance state used for room scene moments and cues.
///
/// Precedence (highest first):
/// 1. AsleepDreaming (sleep overrides everything)
/// 2. CatchUpWake (morning-after wake-resume window)
/// 3. SourceBurstPerk (recent feed pulse + high burst)
/// 4. HeavyDayCozy (high today_ratio + dusk/evening + tiredness)
/// 5. TiredAwake (high tiredness, no heavy-day signal)
/// 6. RestedAwake (night or quiet default)
pub fn pet_performance_from_day_context(day: &DayContext) -> PetPerformance {
    if day.asleep {
        return PetPerformance::AsleepDreaming;
    }
    if day.wake_resume.is_some() {
        return PetPerformance::CatchUpWake;
    }
    if is_heavy_day_cozy(day) {
        return PetPerformance::HeavyDayCozy;
    }
    if day.tiredness > 0.6 {
        return PetPerformance::TiredAwake;
    }
    if matches!(day.day_phase, DayPhase::Night) {
        return PetPerformance::RestedAwake;
    }
    if day.is_weekend && day.weekend_share <= crate::tui::day::WEEKEND_QUIET_SHARE {
        return PetPerformance::HeavyDayCozy;
    }
    PetPerformance::RestedAwake
}

fn pet_performance_for(vm: &WatchViewModel, now: OffsetDateTime) -> PetPerformance {
    if is_source_burst_perk(vm, now) {
        return PetPerformance::SourceBurstPerk;
    }
    pet_performance_from_day_context(&vm.day_context)
}

fn is_source_burst_perk(vm: &WatchViewModel, now: OffsetDateTime) -> bool {
    vm.life_profile.burst_level > 0.8
        && vm
            .last_feed_pulse_at
            .is_some_and(|pulse| now - pulse <= Duration::seconds(8))
}

fn is_heavy_day_cozy(day: &DayContext) -> bool {
    day.today_ratio > 1.0 && day.tiredness > 0.6 && matches!(day.day_phase, DayPhase::Dusk)
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

/// One placed resting-room glyph produced from biome, weather, and emitter layers.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomGlyph {
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
    pub zone: RoomZone,
}

/// Spatial zone inside the habitat area where a glyph belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoomZone {
    Floor,
    UpperAir,
    LeftAnchor,
    RightAnchor,
    PetAdjacent,
}

/// Build the full set of resting-room glyphs for the current profile, capping
/// at the motion budget and filtering out anything that falls inside an
/// exclusion rect (e.g. pet silhouette or speech).
///
/// Layers are composed in biome → weather → emitter order; later layers
/// overwrite earlier layers at the same cell, and the total count is capped by
/// `motion_budget`.
pub fn room_glyphs_for(
    profile: &RoomLifeProfile,
    area: Rect,
    exclusions: &[Rect],
    now: OffsetDateTime,
    color_capability: ColorCapability,
    day_phase: DayPhase,
) -> Vec<RoomGlyph> {
    let mut cells: std::collections::HashMap<(u16, u16), RoomGlyph> =
        std::collections::HashMap::new();
    for glyph in biome_glyphs(profile, area, now, color_capability) {
        cells.insert((glyph.row, glyph.col), glyph);
    }
    for glyph in weather_glyphs(profile, area, now, color_capability) {
        cells.insert((glyph.row, glyph.col), glyph);
    }
    for glyph in emitter_glyphs(profile, area, now, color_capability) {
        cells.insert((glyph.row, glyph.col), glyph);
    }
    let mut glyphs: Vec<RoomGlyph> = cells.into_values().collect();
    glyphs.sort_by_key(|g| (g.row, g.col));
    let budget = (motion_budget(area) as f64 * phase_density_scale(day_phase)).round() as usize;
    let flat = matches!(color_capability, ColorCapability::Flat);
    glyphs
        .into_iter()
        .filter(|glyph| !rects_contain(exclusions, glyph.col, glyph.row))
        .take(budget)
        .map(|mut glyph| {
            if !flat {
                if let Some(fg) = glyph.style.fg {
                    glyph.style = glyph.style.fg(phase_warmth_tint(fg, day_phase));
                }
            }
            glyph
        })
        .collect()
}

/// Interior ambient-glyph density scale by phase. Night sparse, day full.
/// Mirrors the sky's `phase_count_scale`. Texture only.
fn phase_density_scale(phase: DayPhase) -> f64 {
    match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.7,
        DayPhase::Dusk => 0.85,
        DayPhase::Night => 0.5,
    }
}

/// Warmth/contrast bias applied to a room glyph color by phase: dawn cooler,
/// day neutral, dusk warmer, night dim. Texture only — never personality.
fn phase_warmth_tint(color: Color, phase: DayPhase) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    match phase {
        DayPhase::Day => color,
        DayPhase::Dawn => Color::Rgb(r.saturating_sub(6), g, b.saturating_add(8)),
        DayPhase::Dusk => Color::Rgb(
            r.saturating_add(12),
            g.saturating_add(2),
            b.saturating_sub(8),
        ),
        DayPhase::Night => Color::Rgb(
            r.saturating_sub(14),
            g.saturating_sub(10),
            b.saturating_sub(2),
        ),
    }
}

/// Maximum number of moving/resting glyphs allowed for a habitat area of the
/// given size class. Preview Lab and live watch use the same thresholds.
pub fn motion_budget(area: Rect) -> usize {
    if area.width <= 72 || area.height <= 24 {
        8
    } else if area.width >= 180 || area.height >= 50 {
        28
    } else {
        16
    }
}

pub(crate) fn rects_contain(rects: &[Rect], col: u16, row: u16) -> bool {
    rects.iter().any(|r| {
        r.x <= col
            && col < r.x.saturating_add(r.width)
            && r.y <= row
            && row < r.y.saturating_add(r.height)
    })
}

fn biome_symbols(tag: RoomBiomeTag, dialect: RoomSpeciesDialect) -> &'static [char] {
    match dialect.key {
        RoomDialectKey::Glitch => match tag {
            RoomBiomeTag::Starter => &[':', '.', '_'],
            RoomBiomeTag::Botanical => &[';', '`', ',', '_'],
            RoomBiomeTag::Technical => &[':', ';', '+', '='],
            RoomBiomeTag::Celestial => &['.', ':', '+', '*'],
            RoomBiomeTag::Artifact => &['#', ':', '[', ']'],
            RoomBiomeTag::Cozy => &['_', '-', ':', '.'],
        },
        RoomDialectKey::Crystal => match tag {
            RoomBiomeTag::Starter => &['.', '·', '◇'],
            RoomBiomeTag::Botanical => &['\'', '·', '◇', ','],
            RoomBiomeTag::Technical => &['◇', '+', '·', ':'],
            RoomBiomeTag::Celestial => &['✦', '✧', '·', '*'],
            RoomBiomeTag::Artifact => &['◇', '◆', '·', '°'],
            RoomBiomeTag::Cozy => &['·', '◇', '⌞', '⌟'],
        },
        RoomDialectKey::Fuzz
        | RoomDialectKey::Blob
        | RoomDialectKey::Ghost
        | RoomDialectKey::Mech => match tag {
            RoomBiomeTag::Starter => &['.', '·'],
            RoomBiomeTag::Botanical => &['"', '\'', '`', ','],
            RoomBiomeTag::Technical => &[':', ';', '+', '='],
            RoomBiomeTag::Celestial => &['*', '·', '˚', '.'],
            RoomBiomeTag::Artifact => &['.', 'o', '◇', '°'],
            RoomBiomeTag::Cozy => &['~', '·', '⌞', '⌟'],
        },
    }
}

type ZoneAllocations = (Vec<(RoomZone, usize)>, Vec<(RoomZone, usize)>);

fn dialect_zone_counts(dialect: RoomSpeciesDialect) -> Vec<(RoomZone, usize)> {
    match dialect.key {
        RoomDialectKey::Glitch => vec![(RoomZone::RightAnchor, 2), (RoomZone::Floor, 1)],
        RoomDialectKey::Crystal => vec![(RoomZone::UpperAir, 2), (RoomZone::Floor, 1)],
        RoomDialectKey::Fuzz
        | RoomDialectKey::Blob
        | RoomDialectKey::Ghost
        | RoomDialectKey::Mech => Vec::new(),
    }
}

fn zone_counts_for_biome(biome: RoomBiome) -> ZoneAllocations {
    let primary = match biome.primary {
        RoomBiomeTag::Starter => vec![(RoomZone::Floor, 2), (RoomZone::UpperAir, 1)],
        RoomBiomeTag::Botanical => vec![
            (RoomZone::Floor, 4),
            (RoomZone::UpperAir, 2),
            (RoomZone::LeftAnchor, 1),
            (RoomZone::RightAnchor, 1),
            (RoomZone::PetAdjacent, 1),
        ],
        RoomBiomeTag::Technical => vec![
            (RoomZone::RightAnchor, 4),
            (RoomZone::UpperAir, 2),
            (RoomZone::Floor, 1),
            (RoomZone::LeftAnchor, 1),
            (RoomZone::PetAdjacent, 1),
        ],
        RoomBiomeTag::Celestial => vec![
            (RoomZone::UpperAir, 4),
            (RoomZone::LeftAnchor, 1),
            (RoomZone::RightAnchor, 1),
            (RoomZone::Floor, 1),
            (RoomZone::PetAdjacent, 1),
        ],
        RoomBiomeTag::Artifact => vec![
            (RoomZone::Floor, 3),
            (RoomZone::LeftAnchor, 1),
            (RoomZone::RightAnchor, 1),
            (RoomZone::UpperAir, 1),
            (RoomZone::PetAdjacent, 1),
        ],
        RoomBiomeTag::Cozy => vec![
            (RoomZone::PetAdjacent, 3),
            (RoomZone::LeftAnchor, 1),
            (RoomZone::RightAnchor, 1),
            (RoomZone::Floor, 1),
            (RoomZone::UpperAir, 1),
        ],
    };

    let secondary = biome.secondary.map(|tag| match tag {
        RoomBiomeTag::Starter => vec![(RoomZone::Floor, 1)],
        RoomBiomeTag::Botanical => vec![(RoomZone::Floor, 2)],
        RoomBiomeTag::Technical => vec![(RoomZone::RightAnchor, 2)],
        RoomBiomeTag::Celestial => vec![(RoomZone::UpperAir, 2)],
        RoomBiomeTag::Artifact => vec![(RoomZone::Floor, 2)],
        RoomBiomeTag::Cozy => vec![(RoomZone::PetAdjacent, 2)],
    });

    (primary, secondary.unwrap_or_default())
}

fn biome_seed(biome: &RoomBiome, minute_floor: u64) -> u64 {
    let primary = tag_seed(biome.primary);
    let secondary = biome.secondary.map(tag_seed).unwrap_or(255);
    primary
        .wrapping_shl(16)
        .wrapping_add(secondary)
        .wrapping_add(minute_floor.wrapping_mul(0xA0F6_5E0E_8B99_4B17))
}

fn tag_seed(tag: RoomBiomeTag) -> u64 {
    match tag {
        RoomBiomeTag::Starter => 0,
        RoomBiomeTag::Botanical => 1,
        RoomBiomeTag::Technical => 2,
        RoomBiomeTag::Celestial => 3,
        RoomBiomeTag::Artifact => 4,
        RoomBiomeTag::Cozy => 5,
    }
}

fn zone_rect(area: Rect, zone: RoomZone) -> Rect {
    let third_w = area.width / 3;
    let third_h = area.height / 3;
    match zone {
        RoomZone::UpperAir => Rect::new(area.x, area.y, area.width, third_h),
        RoomZone::Floor => Rect::new(
            area.x,
            area.y.saturating_add(2 * third_h),
            area.width,
            area.height.saturating_sub(2 * third_h),
        ),
        RoomZone::LeftAnchor => Rect::new(area.x, area.y.saturating_add(third_h), third_w, third_h),
        RoomZone::RightAnchor => Rect::new(
            area.x.saturating_add(2 * third_w),
            area.y.saturating_add(third_h),
            area.width.saturating_sub(2 * third_w),
            third_h,
        ),
        RoomZone::PetAdjacent => Rect::new(
            area.x.saturating_add(third_w),
            area.y.saturating_add(third_h),
            third_w,
            third_h,
        ),
    }
}

fn place_zone_glyphs<R: Rng>(
    area: Rect,
    zone: RoomZone,
    count: usize,
    symbols: &[char],
    style: Style,
    rng: &mut R,
) -> Vec<RoomGlyph> {
    let rect = zone_rect(area, zone);
    if rect.width == 0 || rect.height == 0 || symbols.is_empty() || count == 0 {
        return Vec::new();
    }
    let mut placed = std::collections::HashSet::new();
    let mut glyphs = Vec::with_capacity(count);
    for _ in 0..count {
        for _attempt in 0..24 {
            let col = rect.x + rng.gen_range(0..rect.width);
            let row = rect.y + rng.gen_range(0..rect.height);
            if placed.insert((col, row)) {
                let glyph = *symbols.choose(rng).unwrap_or(&symbols[0]);
                glyphs.push(RoomGlyph {
                    row,
                    col,
                    glyph,
                    style,
                    zone,
                });
                break;
            }
        }
    }
    glyphs
}

fn biome_style(tag: RoomBiomeTag, color_capability: ColorCapability) -> Style {
    let base = Style::default();
    if matches!(color_capability, ColorCapability::Flat) {
        return base.fg(crate::tui::style::tokenpet_palette().faint.rgb);
    }
    let color = match tag {
        RoomBiomeTag::Starter => Color::Rgb(0x88, 0x88, 0x88),
        RoomBiomeTag::Botanical => Color::Rgb(0x7d, 0xb6, 0x69),
        RoomBiomeTag::Technical => Color::Rgb(0x86, 0xd9, 0xef),
        RoomBiomeTag::Celestial => Color::Rgb(0xff, 0xee, 0xaa),
        RoomBiomeTag::Artifact => Color::Rgb(0xd4, 0xa5, 0x5a),
        RoomBiomeTag::Cozy => Color::Rgb(0xdf, 0xa0, 0x7e),
    };
    base.fg(color)
}

fn biome_glyphs(
    profile: &RoomLifeProfile,
    area: Rect,
    now: OffsetDateTime,
    color_capability: ColorCapability,
) -> Vec<RoomGlyph> {
    let minute_floor = (now.unix_timestamp() / 60) as u64;
    let seed = biome_seed(&profile.biome, minute_floor);
    let mut rng = Pcg32::seed_from_u64(seed);
    let (primary_counts, secondary_counts) = zone_counts_for_biome(profile.biome);
    let style = biome_style(profile.biome.primary, color_capability);
    let symbols = biome_symbols(profile.biome.primary, profile.species_dialect);
    let mut glyphs = Vec::new();
    for (zone, count) in primary_counts {
        glyphs.extend(place_zone_glyphs(
            area, zone, count, symbols, style, &mut rng,
        ));
    }
    if let Some(secondary) = profile.biome.secondary {
        let sec_style = biome_style(secondary, color_capability);
        let sec_symbols = biome_symbols(secondary, profile.species_dialect);
        for (zone, count) in secondary_counts {
            glyphs.extend(place_zone_glyphs(
                area,
                zone,
                count,
                sec_symbols,
                sec_style,
                &mut rng,
            ));
        }
    }
    let dialect_counts = dialect_zone_counts(profile.species_dialect);
    for (zone, count) in dialect_counts {
        glyphs.extend(place_zone_glyphs(
            area, zone, count, symbols, style, &mut rng,
        ));
    }
    glyphs
}

fn weather_symbols(weather: RoomWeatherLayer) -> &'static [char] {
    match weather {
        RoomWeatherLayer::Clear => &[],
        RoomWeatherLayer::CacheMist => &['~', '·', ',', '`'],
        RoomWeatherLayer::OutputSparks => &['*', '+', '·', '˚'],
        RoomWeatherLayer::ReasoningPulse => &['○', '◌', '·', ':'],
        RoomWeatherLayer::Mixed => &[], // Mixed delegates to two sub-weather passes.
    }
}

fn weather_seed(weather: RoomWeatherLayer, minute_floor: u64) -> u64 {
    let discriminant: u64 = match weather {
        RoomWeatherLayer::Clear => 0,
        RoomWeatherLayer::CacheMist => 1,
        RoomWeatherLayer::OutputSparks => 2,
        RoomWeatherLayer::ReasoningPulse => 3,
        RoomWeatherLayer::Mixed => 4,
    };
    discriminant
        .wrapping_mul(0xC7CE_5D9F_E3F7_29C9)
        .wrapping_add(minute_floor.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

fn weather_style(weather: RoomWeatherLayer, color_capability: ColorCapability) -> Style {
    let base = Style::default();
    if matches!(color_capability, ColorCapability::Flat) {
        return base.fg(crate::tui::style::tokenpet_palette().faint.rgb);
    }
    let color = match weather {
        RoomWeatherLayer::Clear => {
            unreachable!(
                "weather_glyphs returns early for Clear, so weather_style is never called with it"
            )
        }
        RoomWeatherLayer::CacheMist => Color::Rgb(0x9d, 0xb6, 0x9d),
        RoomWeatherLayer::OutputSparks => Color::Rgb(0xff, 0xd7, 0x66),
        RoomWeatherLayer::ReasoningPulse => Color::Rgb(0xc9, 0x8f, 0xde),
        RoomWeatherLayer::Mixed => Color::Rgb(0xbb, 0xbb, 0xbb),
    };
    base.fg(color)
}

fn weather_glyphs(
    profile: &RoomLifeProfile,
    area: Rect,
    now: OffsetDateTime,
    color_capability: ColorCapability,
) -> Vec<RoomGlyph> {
    let minute_floor = (now.unix_timestamp() / 60) as u64;
    match profile.room_weather {
        RoomWeatherLayer::Clear => Vec::new(),
        RoomWeatherLayer::Mixed => {
            // Combine mist (floor) and sparks (upper) spatially, but keep the
            // total count within the same budget as a single weather layer.
            let mut glyphs = Vec::new();
            let mist_seed = weather_seed(RoomWeatherLayer::CacheMist, minute_floor);
            let mut mist_rng = Pcg32::seed_from_u64(mist_seed);
            let mist_style = weather_style(RoomWeatherLayer::CacheMist, color_capability);
            let mist_symbols = weather_symbols(RoomWeatherLayer::CacheMist);
            glyphs.extend(place_zone_glyphs(
                area,
                RoomZone::Floor,
                2,
                mist_symbols,
                mist_style,
                &mut mist_rng,
            ));
            let spark_seed = weather_seed(RoomWeatherLayer::OutputSparks, minute_floor);
            let mut spark_rng = Pcg32::seed_from_u64(spark_seed);
            let spark_style = weather_style(RoomWeatherLayer::OutputSparks, color_capability);
            let spark_symbols = weather_symbols(RoomWeatherLayer::OutputSparks);
            glyphs.extend(place_zone_glyphs(
                area,
                RoomZone::UpperAir,
                2,
                spark_symbols,
                spark_style,
                &mut spark_rng,
            ));
            glyphs
        }
        _ => {
            let seed = weather_seed(profile.room_weather, minute_floor);
            let mut rng = Pcg32::seed_from_u64(seed);
            let style = weather_style(profile.room_weather, color_capability);
            let symbols = weather_symbols(profile.room_weather);
            let (zones, counts): (&[RoomZone], &[usize]) = match profile.room_weather {
                RoomWeatherLayer::CacheMist => (
                    &[RoomZone::Floor, RoomZone::LeftAnchor, RoomZone::PetAdjacent],
                    &[4, 1, 1],
                ),
                RoomWeatherLayer::OutputSparks => (
                    &[
                        RoomZone::UpperAir,
                        RoomZone::RightAnchor,
                        RoomZone::PetAdjacent,
                    ],
                    &[4, 2, 1],
                ),
                RoomWeatherLayer::ReasoningPulse => {
                    (&[RoomZone::PetAdjacent, RoomZone::UpperAir], &[3, 2])
                }
                _ => return Vec::new(),
            };
            let mut glyphs = Vec::new();
            for (&zone, &count) in zones.iter().zip(counts.iter()) {
                glyphs.extend(place_zone_glyphs(
                    area, zone, count, symbols, style, &mut rng,
                ));
            }
            glyphs
        }
    }
}

fn emitter_symbols(behavior: PropEmitterBehavior) -> &'static [char] {
    match behavior {
        PropEmitterBehavior::LeafDrift => &['"', '\'', '`', ','],
        PropEmitterBehavior::TechnicalPing => &[':', ';', '+', '='],
        PropEmitterBehavior::HopefulSprout => &['"', '·', '`', ','],
        PropEmitterBehavior::WarmHalo => &['○', '◌', '·', '°'],
        PropEmitterBehavior::CloudDrift => &['~', '·', ',', '`'],
        PropEmitterBehavior::OrbitArc => &['*', '·', '+', '˚'],
        PropEmitterBehavior::ArtifactGlint => &['.', 'o', '◇', '°'],
    }
}

fn emitter_zone_for_prop(prop_id: &str) -> RoomZone {
    use crate::game::habitat::HabitatPropZone;
    let Some(spec) = catalog_prop_by_str(prop_id) else {
        return RoomZone::PetAdjacent;
    };
    match spec.zone {
        HabitatPropZone::FloorLeft => RoomZone::LeftAnchor,
        HabitatPropZone::FloorRight => RoomZone::RightAnchor,
        HabitatPropZone::FloorMid => RoomZone::Floor,
        HabitatPropZone::WallLeft | HabitatPropZone::AirLeft => RoomZone::LeftAnchor,
        HabitatPropZone::WallRight | HabitatPropZone::AirRight => RoomZone::RightAnchor,
        HabitatPropZone::Ceiling => RoomZone::UpperAir,
        HabitatPropZone::AirMid => RoomZone::PetAdjacent,
    }
}

fn emitter_seed(prop_id: &str, minute_floor: u64) -> u64 {
    let id_hash = prop_id.bytes().fold(0x811c_9dc5u64, |h, b| {
        h.wrapping_mul(0x0100_0193) ^ b as u64
    });
    id_hash.wrapping_add(minute_floor.wrapping_mul(0xD6E8_FD9A_934D_CC17))
}

fn emitter_style(behavior: PropEmitterBehavior, color_capability: ColorCapability) -> Style {
    let base = Style::default();
    if matches!(color_capability, ColorCapability::Flat) {
        return base.fg(crate::tui::style::tokenpet_palette().faint.rgb);
    }
    let color = match behavior {
        PropEmitterBehavior::LeafDrift | PropEmitterBehavior::HopefulSprout => {
            Color::Rgb(0x7d, 0xb6, 0x69)
        }
        PropEmitterBehavior::TechnicalPing | PropEmitterBehavior::OrbitArc => {
            Color::Rgb(0x86, 0xd9, 0xef)
        }
        PropEmitterBehavior::WarmHalo => Color::Rgb(0xff, 0xcc, 0x66),
        PropEmitterBehavior::CloudDrift => Color::Rgb(0xbf, 0xd7, 0xea),
        PropEmitterBehavior::ArtifactGlint => Color::Rgb(0xd4, 0xa5, 0x5a),
    };
    base.fg(color)
}

fn emitter_glyphs(
    profile: &RoomLifeProfile,
    area: Rect,
    now: OffsetDateTime,
    color_capability: ColorCapability,
) -> Vec<RoomGlyph> {
    let Some(emitter) = profile.resonant_emitter.as_ref() else {
        return Vec::new();
    };
    let minute_floor = (now.unix_timestamp() / 60) as u64;
    let seed = emitter_seed(emitter.prop_id.as_str(), minute_floor);
    let mut rng = Pcg32::seed_from_u64(seed);
    let zone = emitter_zone_for_prop(emitter.prop_id.as_str());
    let count = (emitter.intensity * 3.0).round().clamp(1.0, 3.0) as usize;
    let symbols = emitter_symbols(emitter.behavior);
    let style = emitter_style(emitter.behavior, color_capability);
    place_zone_glyphs(area, zone, count, symbols, style, &mut rng)
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
    use crate::tui::style::{tokenpet_palette, ColorCapability};
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

    #[test]
    fn pet_performance_tired_and_heavy_day_are_distinct() {
        let mut vm = vm_with_props(vec![earned(HEAVY_SESSION_PLANTER, 80)]);
        vm.day_context = DayContext {
            mature: true,
            tiredness: 0.8,
            today_ratio: 1.4,
            day_phase: DayPhase::Dusk,
            ..vm.day_context
        };

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 19:00 UTC));

        assert_eq!(profile.pet_performance, PetPerformance::HeavyDayCozy);
    }

    #[test]
    fn source_burst_perk_requires_live_burst() {
        let mut vm = vm_with_props(vec![earned(CODEX_SIGNAL_LAMP, 70)]);
        vm.life_profile.burst_level = 0.9;
        vm.last_feed_pulse_at = Some(datetime!(2026-06-11 10:00 UTC));

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00:02 UTC));

        assert_eq!(profile.pet_performance, PetPerformance::SourceBurstPerk);
    }

    #[test]
    fn source_burst_perk_ignores_stale_pulse() {
        let mut vm = vm_with_props(vec![earned(CODEX_SIGNAL_LAMP, 70)]);
        vm.life_profile.burst_level = 0.9;
        vm.last_feed_pulse_at = Some(datetime!(2026-06-11 10:00 UTC));

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00:09 UTC));

        assert_ne!(
            profile.pet_performance,
            PetPerformance::SourceBurstPerk,
            "a 9-second-old pulse should not read as a live burst"
        );
    }

    #[test]
    fn room_motion_budget_matches_preview_size_classes() {
        assert_eq!(motion_budget(Rect::new(0, 0, 72, 24)), 8);
        assert_eq!(motion_budget(Rect::new(0, 0, 120, 32)), 16);
        assert_eq!(motion_budget(Rect::new(0, 0, 180, 50)), 28);
    }

    fn phase_test_profile() -> RoomLifeProfile {
        RoomLifeProfile {
            biome: RoomBiome {
                primary: RoomBiomeTag::Botanical,
                secondary: Some(RoomBiomeTag::Cozy),
            },
            room_weather: RoomWeatherLayer::Clear,
            resonant_emitter: None,
            pet_performance: PetPerformance::RestedAwake,
            scene_moments: Vec::new(),
            identity_prop_ids: Vec::new(),
            species_dialect: RoomSpeciesDialect::for_species(Species::Crystal),
        }
    }

    #[test]
    fn room_glyphs_are_sparser_at_night_than_day() {
        let profile = phase_test_profile();
        let area = Rect::new(0, 0, 120, 32);
        let now = datetime!(2026-06-11 10:00 UTC);
        let day = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
        );
        let night = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Night,
        );
        assert!(
            night.len() < day.len(),
            "night ({}) should have fewer ambient glyphs than day ({})",
            night.len(),
            day.len()
        );
    }

    #[test]
    fn room_glyphs_warm_at_dusk_in_color_mode() {
        let profile = phase_test_profile();
        let area = Rect::new(0, 0, 120, 32);
        let now = datetime!(2026-06-11 10:00 UTC);
        let day = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
        );
        let dusk = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Dusk,
        );
        // At least one co-located glyph should be warmer (redder) at dusk.
        let warmer = day.iter().any(|d| {
            dusk.iter().any(|k| {
                k.row == d.row
                    && k.col == d.col
                    && matches!((k.style.fg, d.style.fg),
                        (Some(Color::Rgb(kr, _, _)), Some(Color::Rgb(dr, _, _))) if kr > dr)
            })
        });
        assert!(warmer, "dusk should warm at least one room glyph vs day");
    }

    #[test]
    fn room_glyphs_still_emit_in_flat_mode_at_night() {
        let profile = phase_test_profile();
        let area = Rect::new(0, 0, 120, 32);
        let now = datetime!(2026-06-11 10:00 UTC);
        let glyphs = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Flat,
            DayPhase::Night,
        );
        assert!(
            !glyphs.is_empty(),
            "flat night should still emit room glyphs"
        );
        let faint = tokenpet_palette().faint.rgb;
        for g in &glyphs {
            assert_eq!(
                g.style.fg,
                Some(faint),
                "flat mode keeps the faint color (no warmth)"
            );
        }
    }

    #[test]
    fn room_glyphs_are_deterministic_for_identical_inputs() {
        let profile = RoomLifeProfile {
            biome: RoomBiome {
                primary: RoomBiomeTag::Botanical,
                secondary: Some(RoomBiomeTag::Cozy),
            },
            room_weather: RoomWeatherLayer::Clear,
            resonant_emitter: None,
            pet_performance: PetPerformance::RestedAwake,
            scene_moments: Vec::new(),
            identity_prop_ids: Vec::new(),
            species_dialect: RoomSpeciesDialect::for_species(Species::Crystal),
        };
        let area = Rect::new(0, 0, 120, 32);
        let now = datetime!(2026-06-11 10:00 UTC);

        let a = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
        );
        let b = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
        );

        assert_eq!(
            a, b,
            "identical inputs should produce identical room glyphs"
        );
    }

    #[test]
    fn room_glyphs_use_symbol_families_in_flat_mode() {
        let profile = RoomLifeProfile {
            biome: RoomBiome {
                primary: RoomBiomeTag::Technical,
                secondary: None,
            },
            room_weather: RoomWeatherLayer::OutputSparks,
            resonant_emitter: None,
            pet_performance: PetPerformance::RestedAwake,
            scene_moments: Vec::new(),
            identity_prop_ids: Vec::new(),
            species_dialect: RoomSpeciesDialect::for_species(Species::Crystal),
        };
        let area = Rect::new(0, 0, 120, 32);
        let now = datetime!(2026-06-11 10:00 UTC);

        let glyphs = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Flat,
            DayPhase::Day,
        );

        assert!(
            !glyphs.is_empty(),
            "flat mode should still emit room glyphs"
        );
        let expected = tokenpet_palette().faint.rgb;
        for g in &glyphs {
            assert_eq!(
                g.style.fg,
                Some(expected),
                "flat-mode glyph should use the faint palette color"
            );
        }
    }

    #[test]
    fn mixed_weather_glyphs_span_families_or_zones() {
        let profile = RoomLifeProfile {
            biome: RoomBiome {
                primary: RoomBiomeTag::Botanical,
                secondary: None,
            },
            room_weather: RoomWeatherLayer::Mixed,
            resonant_emitter: None,
            pet_performance: PetPerformance::RestedAwake,
            scene_moments: Vec::new(),
            identity_prop_ids: Vec::new(),
            species_dialect: RoomSpeciesDialect::for_species(Species::Crystal),
        };
        let area = Rect::new(0, 0, 120, 32);
        let now = datetime!(2026-06-11 10:00 UTC);

        let glyphs = room_glyphs_for(
            &profile,
            area,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
        );
        let symbols: std::collections::HashSet<char> = glyphs.iter().map(|g| g.glyph).collect();
        let zones: std::collections::HashSet<RoomZone> = glyphs.iter().map(|g| g.zone).collect();
        let has_mist = symbols.iter().any(|c| ['~', ',', '`'].contains(c));
        let has_spark = symbols.iter().any(|c| ['*', '+', '˚'].contains(c));

        assert!(
            (has_mist && has_spark) || zones.len() >= 2,
            "mixed weather should produce both families or span multiple zones; symbols={symbols:?} zones={zones:?}"
        );
    }

    #[test]
    fn emitter_intensity_yields_one_to_three_glyphs() {
        for (intensity, label) in [(0.5, "half"), (1.0, "full")] {
            let profile = RoomLifeProfile {
                biome: RoomBiome {
                    primary: RoomBiomeTag::Technical,
                    secondary: None,
                },
                room_weather: RoomWeatherLayer::Clear,
                resonant_emitter: Some(PropEmitter {
                    prop_id: HabitatPropId::from(CODEX_SIGNAL_LAMP),
                    behavior: PropEmitterBehavior::TechnicalPing,
                    intensity,
                }),
                pet_performance: PetPerformance::RestedAwake,
                scene_moments: Vec::new(),
                identity_prop_ids: vec![HabitatPropId::from(CODEX_SIGNAL_LAMP)],
                species_dialect: RoomSpeciesDialect::for_species(Species::Crystal),
            };
            let area = Rect::new(0, 0, 120, 32);
            let now = datetime!(2026-06-11 10:00 UTC);

            let glyphs = emitter_glyphs(&profile, area, now, ColorCapability::Truecolor);

            assert!(
                (1..=3).contains(&glyphs.len()),
                "emitter intensity {label} should produce 1-3 glyphs; got {}",
                glyphs.len()
            );
        }
    }

    #[test]
    fn room_profile_derives_species_dialect_from_pet_species() {
        let mut vm = WatchViewModel::fixture();
        vm.pet_render.generated_species = Species::Glitch;

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

        assert_eq!(profile.species_dialect.species, Species::Glitch);
        assert_eq!(profile.species_dialect.key, RoomDialectKey::Glitch);
        assert_eq!(profile.species_dialect.status, RoomDialectStatus::Tuned);
    }

    #[test]
    fn room_glyphs_change_symbols_by_species_dialect_in_flat_mode() {
        let mut glitch = phase_test_profile();
        glitch.species_dialect = RoomSpeciesDialect::for_species(Species::Glitch);
        glitch.biome = RoomBiome {
            primary: RoomBiomeTag::Technical,
            secondary: Some(RoomBiomeTag::Artifact),
        };

        let mut crystal = glitch.clone();
        crystal.species_dialect = RoomSpeciesDialect::for_species(Species::Crystal);

        let area = Rect::new(0, 0, 120, 32);
        let now = datetime!(2026-06-11 10:00 UTC);
        let glitch_glyphs = room_glyphs_for(
            &glitch,
            area,
            &[],
            now,
            ColorCapability::Flat,
            DayPhase::Day,
        );
        let crystal_glyphs = room_glyphs_for(
            &crystal,
            area,
            &[],
            now,
            ColorCapability::Flat,
            DayPhase::Day,
        );
        let changed = glitch_glyphs
            .iter()
            .zip(&crystal_glyphs)
            .filter(|(left, right)| left.glyph != right.glyph)
            .count();
        let changed_zones = glitch_glyphs
            .iter()
            .zip(&crystal_glyphs)
            .filter(|(left, right)| left.glyph != right.glyph)
            .map(|(left, _)| left.zone)
            .collect::<std::collections::BTreeSet<_>>();

        assert!(
            changed >= 6,
            "expected at least 6 dialect symbol changes; got {changed}"
        );
        assert!(
            changed_zones.len() >= 2,
            "expected dialect changes in at least two zones; got {changed_zones:?}"
        );
    }

    #[test]
    fn phase_density_scale_is_sparsest_at_night_fullest_at_day() {
        assert_eq!(phase_density_scale(DayPhase::Day), 1.0);
        assert!(phase_density_scale(DayPhase::Night) < phase_density_scale(DayPhase::Dusk));
        assert!(phase_density_scale(DayPhase::Dawn) < phase_density_scale(DayPhase::Day));
        assert!(phase_density_scale(DayPhase::Night) < phase_density_scale(DayPhase::Dawn));
    }

    #[test]
    fn phase_warmth_tint_warms_at_dusk_and_cools_at_dawn() {
        let base = Color::Rgb(120, 120, 120);
        let Color::Rgb(dawn_r, _, dawn_b) = phase_warmth_tint(base, DayPhase::Dawn) else {
            panic!("expected rgb");
        };
        let Color::Rgb(dusk_r, _, dusk_b) = phase_warmth_tint(base, DayPhase::Dusk) else {
            panic!("expected rgb");
        };
        // Dusk is warmer (more red, less blue) than dawn.
        assert!(dusk_r > dawn_r, "dusk should be redder than dawn");
        assert!(dusk_b < dawn_b, "dusk should be less blue than dawn");
        // Day is identity.
        assert_eq!(phase_warmth_tint(base, DayPhase::Day), base);
    }
}
