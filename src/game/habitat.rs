use time::OffsetDateTime;

use crate::{
    game::metabolism::Mood,
    storage::{
        state::{EarnedHabitatProp, HabitatPropSource, PetState},
        usage_store::UsageLedgerRow,
    },
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

pub const HABITAT_PROP_CATALOG: &[HabitatPropSpec] = &[
    HabitatPropSpec {
        id: TOKEN_PEBBLE_25K,
        kind: HabitatPropKind::Accent,
        zone: HabitatPropZone::FloorLeft,
        display_priority: 10,
        lifetime_threshold: Some(25_000.0),
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
        display_priority: 25,
        lifetime_threshold: Some(250_000.0),
        pet_layer: HabitatPetLayer::Behind,
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
        color: (0xe8, 0xed, 0xf2), // soft cloud white
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
        display_priority: 65,
        lifetime_threshold: Some(25_000_000.0),
        pet_layer: HabitatPetLayer::Behind,
        color: (0x7a, 0xb8, 0x80), // leafy green
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
        display_priority: 80,
        lifetime_threshold: None,
        pet_layer: HabitatPetLayer::Behind,
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

pub fn unlock_habitat_props(
    state: &mut PetState,
    rows: &[UsageLedgerRow],
    recent_effective_tokens: f64,
    initial_mood: Mood,
    new_mood: Mood,
    now: OffsetDateTime,
) -> Vec<HabitatPropId> {
    let mut unlocked = Vec::new();
    unlock_lifetime_ladder(state, now, &mut unlocked);
    unlock_first_codex(state, rows, now, &mut unlocked);
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
    fn cloud_and_vine_are_behind_pet_layer() {
        assert_eq!(
            catalog_prop_by_str(TOKEN_FRIENDLY_CLOUD_750K)
                .unwrap()
                .pet_layer,
            HabitatPetLayer::Behind
        );
        assert_eq!(
            catalog_prop_by_str(TOKEN_HANGING_VINE_25M)
                .unwrap()
                .pet_layer,
            HabitatPetLayer::Behind
        );
    }

    #[test]
    fn floor_props_are_behind_pet_layer() {
        // Floor-row props share the pet's walking row. They should render
        // behind the pet so the pet visually occludes them when it wanders
        // past, rather than being shoved aside by the silhouette halo.
        for id in [
            TOKEN_PEBBLE_25K,
            TOKEN_SHELL_100K,
            TOKEN_MOSS_TUFT_250K,
            TOKEN_TREASURE_CHEST_2M,
            HEAVY_SESSION_PLANTER,
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
}
