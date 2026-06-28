use time::OffsetDateTime;

use crate::{
    game::metabolism::Mood,
    storage::{
        state::{EarnedHabitatProp, HabitatPropSource, PetState},
        usage_store::UsageLedgerRow,
    },
    tui::identity::{ActivityIdentityProfile, RecoveryPattern, ENSEMBLE_MEMBER_MIN_SHARE},
};

pub use crate::storage::state::HabitatPropId;

const HEAVY_SESSION_MIN_TOKENS: f64 = 50_000.0;
const HEAVY_SESSION_BASELINE_FRACTION: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HabitatPropKind {
    Trophy,
    Accent,
}

/// Determines whether a prop renders before or after the pet, and whether it
/// avoids the pet's silhouette + halo when placing.
///
/// - `Background`: rendered before pet, avoids silhouette+halo. The vast
///   majority of props — they stay clear of the pet entirely.
/// - `Behind`: rendered before pet, no silhouette exclusion. The pet's
///   non-space glyphs paint over the prop where they overlap, so the prop
///   appears to sit *behind* the pet (parts visible in the diamond's
///   negative space).
/// - `Foreground`: rendered after pet, paints over the pet's silhouette.
///   The prop appears *in front of* the pet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HabitatPetLayer {
    Background,
    Behind,
    Foreground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HabitatPropZone {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HabitatPropSpec {
    pub id: &'static str,
    pub kind: HabitatPropKind,
    pub zone: HabitatPropZone,
    pub display_priority: i16,
    pub lifetime_threshold: Option<f64>,
    pub pet_layer: HabitatPetLayer,
    /// Natural rendering color for this prop's glyphs. Picked to read as
    /// the real-world thing (mossy green for plants, amber for chests,
    /// warm orange for lanterns) rather than tinted to match the pet —
    /// the habitat is a place, the pet is a creature in it.
    pub color: (u8, u8, u8),
}

pub const TOKEN_PEBBLE_25K: &str = "token_pebble_25k";
pub const TOKEN_SHELL_100K: &str = "token_shell_100k";
pub const TOKEN_MOSS_TUFT_250K: &str = "token_moss_tuft_250k";
pub const TOKEN_SPARK_500K: &str = "token_spark_500k";
pub const TOKEN_FRIENDLY_CLOUD_750K: &str = "token_friendly_cloud_750k";
pub const TOKEN_SHARD_1M: &str = "token_shard_1m";
pub const TOKEN_TREASURE_CHEST_2M: &str = "token_treasure_chest_2m";
pub const TOKEN_ORBIT_5M: &str = "token_orbit_5m";
pub const TOKEN_LANTERN_10M: &str = "token_lantern_10m";
pub const TOKEN_HANGING_VINE_25M: &str = "token_hanging_vine_25m";
pub const CODEX_SIGNAL_LAMP: &str = "codex_signal_lamp";
pub const HEAVY_SESSION_PLANTER: &str = "heavy_session_planter";
pub const WILT_RECOVERY_SPROUT: &str = "wilt_recovery_sprout";
pub const FIRST_ENSEMBLE_DAY: &str = "first_ensemble_day";
pub const RETURN_SPROUT: &str = "return_sprout";
// Prestige ladder beyond 25M — grand, animated habitat pieces for long-running
// pets. Ranked above the event props, but below the plant anchors (vine/moss/
// planter): the plants always hold the top trophy slots, and the grandest earned
// prestige pieces fill the remaining slots.
pub const TOKEN_GEODE_50M: &str = "token_geode_50m";
pub const TOKEN_BONSAI_100M: &str = "token_bonsai_100m";
pub const TOKEN_CONSTELLATION_250M: &str = "token_constellation_250m";
pub const TOKEN_AURORA_500M: &str = "token_aurora_500m";
pub const TOKEN_MOON_1B: &str = "token_moon_1b";

pub const HABITAT_PROP_CATALOG: &[HabitatPropSpec] = &[
    HabitatPropSpec {
        id: TOKEN_PEBBLE_25K,
        kind: HabitatPropKind::Accent,
        zone: HabitatPropZone::FloorLeft,
        display_priority: 10,
        // Front-loaded: the first pebble arrives at 10k lifetime tokens so a
        // young pet's habitat shows honest early character. The const name
        // keeps its 25k label for call-site stability; the value is the source
        // of truth. Maturity gate and later thresholds are unchanged.
        lifetime_threshold: Some(10_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0xa8, 0xa4, 0x9c), // weathered stone
    },
    HabitatPropSpec {
        id: TOKEN_SHELL_100K,
        kind: HabitatPropKind::Accent,
        zone: HabitatPropZone::FloorRight,
        display_priority: 20,
        lifetime_threshold: Some(100_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0xe8, 0xc8, 0xa8), // pearly cream
    },
    HabitatPropSpec {
        id: TOKEN_MOSS_TUFT_250K,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::FloorMid,
        display_priority: 150,
        lifetime_threshold: Some(250_000.0),
        pet_layer: HabitatPetLayer::Foreground,
        color: (0x6f, 0xb0, 0x60), // moss green
    },
    HabitatPropSpec {
        id: TOKEN_SPARK_500K,
        kind: HabitatPropKind::Accent,
        zone: HabitatPropZone::AirLeft,
        display_priority: 30,
        lifetime_threshold: Some(500_000.0),
        pet_layer: HabitatPetLayer::Background,
        color: (0xff, 0xee, 0x9c), // soft spark yellow
    },
    HabitatPropSpec {
        id: TOKEN_FRIENDLY_CLOUD_750K,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::AirMid,
        display_priority: 45,
        lifetime_threshold: Some(750_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0x9c, 0xac, 0xc0), // overcast blue-grey
    },
    HabitatPropSpec {
        id: TOKEN_SHARD_1M,
        kind: HabitatPropKind::Accent,
        zone: HabitatPropZone::WallRight,
        display_priority: 40,
        lifetime_threshold: Some(1_000_000.0),
        pet_layer: HabitatPetLayer::Background,
        color: (0x82, 0xcc, 0xd8), // crystal teal
    },
    HabitatPropSpec {
        id: TOKEN_TREASURE_CHEST_2M,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::FloorMid,
        display_priority: 55,
        lifetime_threshold: Some(2_000_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0xd8, 0xa4, 0x4c), // antique gold
    },
    HabitatPropSpec {
        id: TOKEN_ORBIT_5M,
        kind: HabitatPropKind::Accent,
        zone: HabitatPropZone::AirRight,
        display_priority: 50,
        lifetime_threshold: Some(5_000_000.0),
        pet_layer: HabitatPetLayer::Background,
        color: (0xb0, 0x9c, 0xe8), // pale violet
    },
    HabitatPropSpec {
        id: TOKEN_LANTERN_10M,
        kind: HabitatPropKind::Accent,
        zone: HabitatPropZone::Ceiling,
        display_priority: 60,
        lifetime_threshold: Some(10_000_000.0),
        pet_layer: HabitatPetLayer::Background,
        color: (0xf0, 0xb4, 0x60), // lantern glow
    },
    HabitatPropSpec {
        id: TOKEN_HANGING_VINE_25M,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::Ceiling,
        display_priority: 152, // vine — green plants anchor the tank
        lifetime_threshold: Some(25_000_000.0),
        pet_layer: HabitatPetLayer::Foreground,
        color: (0x7a, 0xb8, 0x80), // leafy green
    },
    HabitatPropSpec {
        id: TOKEN_GEODE_50M,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::WallLeft,
        display_priority: 100,
        lifetime_threshold: Some(50_000_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0x9c, 0x5c, 0xd0), // amethyst geode
    },
    HabitatPropSpec {
        id: TOKEN_BONSAI_100M,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::FloorRight,
        display_priority: 110,
        lifetime_threshold: Some(100_000_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0xe8, 0x9c, 0xc0), // cherry blossom
    },
    HabitatPropSpec {
        id: TOKEN_CONSTELLATION_250M,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::AirMid,
        display_priority: 120,
        lifetime_threshold: Some(250_000_000.0),
        pet_layer: HabitatPetLayer::Background,
        color: (0xf0, 0xe4, 0x9c), // starlight gold
    },
    HabitatPropSpec {
        id: TOKEN_AURORA_500M,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::Ceiling,
        display_priority: 130,
        lifetime_threshold: Some(500_000_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0xc0, 0x88, 0xe0), // aurora violet
    },
    HabitatPropSpec {
        id: TOKEN_MOON_1B,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::AirRight,
        display_priority: 140,
        lifetime_threshold: Some(1_000_000_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0x88, 0x94, 0xb0), // dusk moon silver
    },
    HabitatPropSpec {
        id: CODEX_SIGNAL_LAMP,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::AirRight,
        display_priority: 70,
        lifetime_threshold: None,
        pet_layer: HabitatPetLayer::Background,
        color: (0xd8, 0x6c, 0x5c), // signal-lamp red
    },
    HabitatPropSpec {
        id: HEAVY_SESSION_PLANTER,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::FloorRight,
        display_priority: 148,
        lifetime_threshold: None,
        pet_layer: HabitatPetLayer::Foreground,
        color: (0x6f, 0xb0, 0x60), // potted foliage
    },
    HabitatPropSpec {
        id: WILT_RECOVERY_SPROUT,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::FloorLeft,
        display_priority: 90,
        lifetime_threshold: None,
        pet_layer: HabitatPetLayer::Behind,
        color: (0x88, 0xc8, 0x78), // tender sprout
    },
    HabitatPropSpec {
        id: FIRST_ENSEMBLE_DAY,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::AirMid,
        display_priority: 75,
        lifetime_threshold: None,
        pet_layer: HabitatPetLayer::Background,
        color: (0xf0, 0xc4, 0x6a),
    },
    HabitatPropSpec {
        id: RETURN_SPROUT,
        kind: HabitatPropKind::Trophy,
        zone: HabitatPropZone::FloorLeft,
        display_priority: 85,
        lifetime_threshold: None,
        pet_layer: HabitatPetLayer::Behind,
        color: (0x88, 0xc8, 0x78),
    },
];

pub fn catalog_prop(id: &HabitatPropId) -> Option<&'static HabitatPropSpec> {
    catalog_prop_by_str(id.as_str())
}

pub fn catalog_prop_by_str(id: &str) -> Option<&'static HabitatPropSpec> {
    HABITAT_PROP_CATALOG.iter().find(|prop| prop.id == id)
}

pub fn ladder_props() -> impl Iterator<Item = &'static HabitatPropSpec> {
    HABITAT_PROP_CATALOG
        .iter()
        .filter(|prop| prop.lifetime_threshold.is_some())
}

#[allow(clippy::too_many_arguments)]
pub fn unlock_habitat_props(
    state: &mut PetState,
    rows: &[UsageLedgerRow],
    recent_effective_tokens: f64,
    initial_mood: Mood,
    new_mood: Mood,
    now: OffsetDateTime,
    activity_profile: &ActivityIdentityProfile,
    current_batch_source_totals: &[(String, f64)],
) -> Vec<HabitatPropId> {
    let mut unlocked = Vec::new();
    unlock_lifetime_ladder(state, now, &mut unlocked);
    // Skip row-dependent unlocks when there is no current batch to evaluate.
    if !rows.is_empty() {
        unlock_first_codex(state, rows, now, &mut unlocked);
        unlock_first_ensemble_day(state, current_batch_source_totals, now, &mut unlocked);
    }
    unlock_return_sprout(state, activity_profile, now, &mut unlocked);
    unlock_heavy_session(state, recent_effective_tokens, now, &mut unlocked);
    unlock_wilt_recovery(state, initial_mood, new_mood, now, &mut unlocked);
    unlocked
}

fn unlock_lifetime_ladder(
    state: &mut PetState,
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    let lifetime = state.lifetime_effective_tokens.max(0.0);
    for prop in ladder_props() {
        let threshold = prop.lifetime_threshold.unwrap_or(f64::INFINITY);
        if lifetime >= threshold {
            record_prop(
                state,
                HabitatPropId::new(prop.id),
                HabitatPropSource::LifetimeTokens { threshold },
                now,
                unlocked,
            );
        }
    }
    state.habitat.reconciled_lifetime_tokens_at = Some(lifetime);
}

fn unlock_first_codex(
    state: &mut PetState,
    rows: &[UsageLedgerRow],
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    if rows
        .iter()
        .any(|row| row.event.provider_surface == "codex" && row.event.effective_tokens > 0.0)
    {
        record_prop(
            state,
            HabitatPropId::new(CODEX_SIGNAL_LAMP),
            HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
            now,
            unlocked,
        );
    }
}

/// Unlocks `first_ensemble_day` when a single applied batch contains at least
/// three significant sources. "Significant" means the source contributes at
/// least `ENSEMBLE_MEMBER_MIN_SHARE` of the batch total. This intentionally
/// checks the current batch rather than the whole local day so that
/// first-contact seeded history does not count toward the milestone.
fn unlock_first_ensemble_day(
    state: &mut PetState,
    current_batch_source_totals: &[(String, f64)],
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    let total: f64 = current_batch_source_totals.iter().map(|(_, v)| *v).sum();
    if total <= 0.0 || !total.is_finite() {
        return;
    }
    let significant = current_batch_source_totals
        .iter()
        .filter(|(_, v)| *v / total >= ENSEMBLE_MEMBER_MIN_SHARE)
        .count();
    if significant >= 3 {
        record_prop(
            state,
            HabitatPropId::new(FIRST_ENSEMBLE_DAY),
            HabitatPropSource::ActivityMilestone {
                milestone: FIRST_ENSEMBLE_DAY.to_string(),
            },
            now,
            unlocked,
        );
    }
}

fn unlock_return_sprout(
    state: &mut PetState,
    activity_profile: &ActivityIdentityProfile,
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    if matches!(activity_profile.recovery, RecoveryPattern::Returned) {
        record_prop(
            state,
            HabitatPropId::new(RETURN_SPROUT),
            HabitatPropSource::ActivityMilestone {
                milestone: RETURN_SPROUT.to_string(),
            },
            now,
            unlocked,
        );
    }
}

fn unlock_heavy_session(
    state: &mut PetState,
    recent_effective_tokens: f64,
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    let baseline = state.calibration.daily_effective_tokens.max(1.0);
    let threshold = HEAVY_SESSION_MIN_TOKENS.max(baseline * HEAVY_SESSION_BASELINE_FRACTION);
    if recent_effective_tokens >= threshold {
        record_prop(
            state,
            HabitatPropId::new(HEAVY_SESSION_PLANTER),
            HabitatPropSource::HeavySession,
            now,
            unlocked,
        );
    }
}

fn unlock_wilt_recovery(
    state: &mut PetState,
    initial_mood: Mood,
    new_mood: Mood,
    now: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    if initial_mood == Mood::Wilted && new_mood != Mood::Wilted {
        record_prop(
            state,
            HabitatPropId::new(WILT_RECOVERY_SPROUT),
            HabitatPropSource::WiltRecovery,
            now,
            unlocked,
        );
    }
}

fn record_prop(
    state: &mut PetState,
    id: HabitatPropId,
    source: HabitatPropSource,
    earned_at: OffsetDateTime,
    unlocked: &mut Vec<HabitatPropId>,
) {
    if state.habitat.earned_props.iter().any(|prop| prop.id == id) {
        return;
    }

    state.habitat.earned_props.push(EarnedHabitatProp {
        id: id.clone(),
        earned_at,
        source,
    });
    unlocked.push(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moss_tuft_and_planter_render_in_green() {
        for id in [
            TOKEN_MOSS_TUFT_250K,
            HEAVY_SESSION_PLANTER,
            WILT_RECOVERY_SPROUT,
        ] {
            let (r, g, b) = catalog_prop_by_str(id).unwrap().color;
            assert!(
                g > r && g > b,
                "{id} should be green-dominant; got rgb({r}, {g}, {b})"
            );
        }
    }

    #[test]
    fn treasure_chest_renders_in_warm_amber() {
        let (r, g, b) = catalog_prop_by_str(TOKEN_TREASURE_CHEST_2M).unwrap().color;
        assert!(
            r > 180 && r > b && g > b,
            "treasure chest should be warm/amber; got rgb({r}, {g}, {b})"
        );
    }

    #[test]
    fn lantern_and_signal_lamp_render_in_warm_red_to_orange() {
        for id in [TOKEN_LANTERN_10M, CODEX_SIGNAL_LAMP] {
            let (r, g, b) = catalog_prop_by_str(id).unwrap().color;
            assert!(
                r > g && r > b,
                "{id} should read warm (r dominant); got rgb({r}, {g}, {b})"
            );
        }
    }

    #[test]
    fn props_have_distinct_colors() {
        // Catch the regression where every prop shared the species tint and
        // came through as the same shade. Distinct colors per prop is the
        // visible payoff of having per-prop styling.
        let colors: std::collections::HashSet<(u8, u8, u8)> =
            HABITAT_PROP_CATALOG.iter().map(|spec| spec.color).collect();
        // Allow some sharing (e.g. moss and planter both green) but not all
        // identical.
        assert!(
            colors.len() >= 8,
            "expected at least 8 distinct prop colors; got {}",
            colors.len()
        );
    }

    #[test]
    fn cloud_is_behind_pet_layer() {
        assert_eq!(
            catalog_prop_by_str(TOKEN_FRIENDLY_CLOUD_750K)
                .unwrap()
                .pet_layer,
            HabitatPetLayer::Behind
        );
    }

    #[test]
    fn flowering_plants_are_foreground_layer() {
        // The flowering plants render in FRONT of the pet at a FIXED anchor, so
        // the player can watch them grow and bloom. Foreground (unlike Background)
        // does not dodge the pet's silhouette, so the plants stay put instead of
        // chasing the free-floating pet around the tank.
        for id in [
            TOKEN_MOSS_TUFT_250K,
            TOKEN_HANGING_VINE_25M,
            HEAVY_SESSION_PLANTER,
        ] {
            assert_eq!(
                catalog_prop_by_str(id).unwrap().pet_layer,
                HabitatPetLayer::Foreground,
                "{id} is a visible plant; should be Foreground so it stays put + visible"
            );
        }
    }

    #[test]
    fn floor_props_are_behind_pet_layer() {
        // Non-plant floor-row props still render behind the pet so it occludes
        // them when it wanders past (the plants moved to Background — see above).
        for id in [
            TOKEN_PEBBLE_25K,
            TOKEN_SHELL_100K,
            TOKEN_TREASURE_CHEST_2M,
            WILT_RECOVERY_SPROUT,
        ] {
            assert_eq!(
                catalog_prop_by_str(id).unwrap().pet_layer,
                HabitatPetLayer::Behind,
                "{id} sits on the floor; should be Behind so pet occludes it"
            );
        }
    }

    #[test]
    fn non_floor_props_default_to_background_pet_layer() {
        for id in [
            TOKEN_SPARK_500K,
            TOKEN_SHARD_1M,
            TOKEN_ORBIT_5M,
            TOKEN_LANTERN_10M,
            CODEX_SIGNAL_LAMP,
        ] {
            assert_eq!(
                catalog_prop_by_str(id).unwrap().pet_layer,
                HabitatPetLayer::Background,
                "{id} should default to Background"
            );
        }
    }

    #[test]
    fn first_ensemble_day_unlocks_with_three_sources() {
        let mut state = PetState::new_for_test("seed", "miso");
        let today = vec![
            ("claude".to_string(), 4_000.0),
            ("codex".to_string(), 3_000.0),
            ("gemini".to_string(), 3_000.0),
        ];
        let mut unlocked = Vec::new();
        unlock_first_ensemble_day(
            &mut state,
            &today,
            OffsetDateTime::UNIX_EPOCH,
            &mut unlocked,
        );
        assert!(unlocked.iter().any(|id| id.as_str() == FIRST_ENSEMBLE_DAY));
    }

    #[test]
    fn first_ensemble_day_requires_three_significant_sources() {
        let mut state = PetState::new_for_test("seed", "miso");
        let today = vec![
            ("claude".to_string(), 9_000.0),
            ("codex".to_string(), 900.0),
            ("gemini".to_string(), 100.0),
        ];
        let mut unlocked = Vec::new();
        unlock_first_ensemble_day(
            &mut state,
            &today,
            OffsetDateTime::UNIX_EPOCH,
            &mut unlocked,
        );
        assert!(!unlocked.iter().any(|id| id.as_str() == FIRST_ENSEMBLE_DAY));
    }

    #[test]
    fn return_sprout_unlocks_on_returned_recovery() {
        let mut state = PetState::new_for_test("seed", "miso");
        let profile = ActivityIdentityProfile {
            recovery: RecoveryPattern::Returned,
            ..Default::default()
        };
        let mut unlocked = Vec::new();
        unlock_return_sprout(
            &mut state,
            &profile,
            OffsetDateTime::UNIX_EPOCH,
            &mut unlocked,
        );
        assert!(unlocked.iter().any(|id| id.as_str() == RETURN_SPROUT));
    }

    #[test]
    fn pebble_unlocks_earlier_for_honest_early_character() {
        let spec = catalog_prop_by_str(TOKEN_PEBBLE_25K).unwrap();
        assert_eq!(
            spec.lifetime_threshold,
            Some(10_000.0),
            "the first pebble front-loads early character at 10k lifetime tokens"
        );
        // The maturity gate (100k default baseline) is unrelated and untouched —
        // a pet at 10k lifetime tokens is still immature and renders zero motes.
    }
}
