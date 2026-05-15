use time::OffsetDateTime;

use crate::{
    game::metabolism::Mood,
    storage::{
        state::{EarnedHabitatProp, HabitatPropId, HabitatPropSource, PetState},
        usage_store::UsageLedgerRow,
    },
};

const HEAVY_SESSION_MIN_TOKENS: f64 = 50_000.0;
const HEAVY_SESSION_BASELINE_FRACTION: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HabitatPropKind {
    Trophy,
    Accent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HabitatPropSpec {
    pub id: &'static str,
    pub kind: HabitatPropKind,
    pub display_priority: i16,
    pub lifetime_threshold: Option<f64>,
}

pub const TOKEN_PEBBLE_25K: &str = "token_pebble_25k";
pub const TOKEN_SHELL_100K: &str = "token_shell_100k";
pub const TOKEN_SPARK_500K: &str = "token_spark_500k";
pub const TOKEN_SHARD_1M: &str = "token_shard_1m";
pub const TOKEN_ORBIT_5M: &str = "token_orbit_5m";
pub const TOKEN_LANTERN_10M: &str = "token_lantern_10m";
pub const CODEX_SIGNAL_LAMP: &str = "codex_signal_lamp";
pub const HEAVY_SESSION_PLANTER: &str = "heavy_session_planter";
pub const WILT_RECOVERY_SPROUT: &str = "wilt_recovery_sprout";

pub const HABITAT_PROP_CATALOG: &[HabitatPropSpec] = &[
    HabitatPropSpec {
        id: TOKEN_PEBBLE_25K,
        kind: HabitatPropKind::Accent,
        display_priority: 10,
        lifetime_threshold: Some(25_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_SHELL_100K,
        kind: HabitatPropKind::Accent,
        display_priority: 20,
        lifetime_threshold: Some(100_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_SPARK_500K,
        kind: HabitatPropKind::Accent,
        display_priority: 30,
        lifetime_threshold: Some(500_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_SHARD_1M,
        kind: HabitatPropKind::Accent,
        display_priority: 40,
        lifetime_threshold: Some(1_000_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_ORBIT_5M,
        kind: HabitatPropKind::Accent,
        display_priority: 50,
        lifetime_threshold: Some(5_000_000.0),
    },
    HabitatPropSpec {
        id: TOKEN_LANTERN_10M,
        kind: HabitatPropKind::Accent,
        display_priority: 60,
        lifetime_threshold: Some(10_000_000.0),
    },
    HabitatPropSpec {
        id: CODEX_SIGNAL_LAMP,
        kind: HabitatPropKind::Trophy,
        display_priority: 70,
        lifetime_threshold: None,
    },
    HabitatPropSpec {
        id: HEAVY_SESSION_PLANTER,
        kind: HabitatPropKind::Trophy,
        display_priority: 80,
        lifetime_threshold: None,
    },
    HabitatPropSpec {
        id: WILT_RECOVERY_SPROUT,
        kind: HabitatPropKind::Trophy,
        display_priority: 90,
        lifetime_threshold: None,
    },
];

pub fn catalog_prop(id: &HabitatPropId) -> Option<&'static HabitatPropSpec> {
    HABITAT_PROP_CATALOG
        .iter()
        .find(|prop| prop.id == id.as_str())
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
