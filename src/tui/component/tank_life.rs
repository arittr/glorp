use ratatui::layout::Rect;

use crate::storage::state::TankInhabitantId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankLifeSurface {
    Watch,
    Round,
    Menubar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundApertureMask {
    pub center_col: i16,
    pub center_row: i16,
    pub radius_cols: u16,
    pub radius_rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeSurfaceGeometry {
    pub surface: TankLifeSurface,
    pub habitat: Rect,
    pub aperture_mask: Option<RoundApertureMask>,
    pub reserved_regions: Vec<Rect>,
    pub max_moving_slots: usize,
    pub literal_floor_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedTankLifeCast {
    pub canonical_ids: Vec<TankInhabitantId>,
    pub rendered_ids: Vec<TankInhabitantId>,
    pub skipped: Vec<TankLifeSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeSkip {
    pub id: TankInhabitantId,
    pub reason: TankLifeSkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankLifeSkipReason {
    UnknownCatalogId,
    SurfaceBudget,
    HabitatTooSmall,
    ReservedRegionCollision,
    ApertureCollision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnemoneMorph {
    Flower,
    Comb,
    Crown,
    DotColony,
}

impl TankLifeSurfaceGeometry {
    #[cfg(test)]
    pub fn round_for_test(cols: u16, rows: u16, max_moving_slots: usize) -> Self {
        Self {
            surface: TankLifeSurface::Round,
            habitat: Rect::new(0, 0, cols, rows),
            aperture_mask: Some(RoundApertureMask {
                center_col: (cols / 2) as i16,
                center_row: (rows / 2) as i16,
                radius_cols: cols / 2,
                radius_rows: rows / 2,
            }),
            reserved_regions: vec![Rect::new(0, rows.saturating_sub(4), cols, 4)],
            max_moving_slots,
            literal_floor_allowed: false,
        }
    }
}

pub fn canonical_daily_cast(
    unlocked: &[crate::tui::view_model::EarnedTankInhabitantView],
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> Vec<TankInhabitantId> {
    let mut known = unlocked
        .iter()
        .filter(|earned| crate::game::habitat::tank_inhabitant_spec(&earned.id).is_some())
        .collect::<Vec<_>>();
    known.sort_by(|a, b| a.id.cmp(&b.id));

    let target = canonical_target_count(known.len(), pet_seed, local_date, calendar_age_days);
    if target == 0 {
        return Vec::new();
    }

    let mut scored = known
        .into_iter()
        .map(|earned| {
            let score = stable_hash(&format!(
                "tank-life-cast-v1|{pet_seed}|{local_date}|{}",
                earned.id.as_str()
            ));
            (score, earned.id.clone())
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, id)| (*score, id.clone()));
    scored
        .into_iter()
        .take(target.min(5))
        .map(|(_, id)| id)
        .collect()
}

pub fn canonical_target_count(
    unlocked_len: usize,
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> usize {
    if unlocked_len <= 2 {
        return unlocked_len;
    }
    let flip = (stable_hash(&format!(
        "tank-life-target-v1|{pet_seed}|{local_date}|{calendar_age_days}"
    )) & 1) as usize;
    let target = if calendar_age_days < 21 {
        2 + flip
    } else if calendar_age_days < 60 {
        3 + flip
    } else {
        4 + flip
    };
    target.min(unlocked_len).min(5)
}

pub fn anemone_morph_for_day(pet_seed: &str, local_date: time::Date) -> AnemoneMorph {
    match stable_hash(&format!("anemone-morph-v1|{pet_seed}|{local_date}")) % 4 {
        0 => AnemoneMorph::Flower,
        1 => AnemoneMorph::Comb,
        2 => AnemoneMorph::Crown,
        _ => AnemoneMorph::DotColony,
    }
}

pub fn project_tank_life_cast(
    canonical_ids: &[TankInhabitantId],
    geometry: &TankLifeSurfaceGeometry,
) -> RenderedTankLifeCast {
    let mut rendered_ids = Vec::new();
    let mut skipped = Vec::new();

    for id in canonical_ids {
        let Some(spec) = crate::game::habitat::tank_inhabitant_spec(id) else {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::UnknownCatalogId,
            });
            continue;
        };
        if rendered_ids.len() >= geometry.max_moving_slots {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::SurfaceBudget,
            });
            continue;
        }
        if !footprint_can_fit(spec.id, geometry) {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::HabitatTooSmall,
            });
            continue;
        }
        rendered_ids.push(id.clone());
    }

    RenderedTankLifeCast {
        canonical_ids: canonical_ids.to_vec(),
        rendered_ids,
        skipped,
    }
}

fn footprint_can_fit(id: &str, geometry: &TankLifeSurfaceGeometry) -> bool {
    let min = match id {
        crate::game::habitat::ANEMONE_HOST => (8, 6),
        crate::game::habitat::SCHOOLLET => (8, 3),
        _ => (4, 3),
    };
    geometry.habitat.width >= min.0 && geometry.habitat.height >= min.1
}

fn stable_hash(input: &str) -> u64 {
    const OFFSET: u64 = 1_469_598_103_934_665_603;
    const PRIME: u64 = 1_099_511_628_211;

    input.bytes().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
        hash
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::state::TankInhabitantId;
    use crate::tui::view_model::EarnedTankInhabitantView;
    use time::macros::date;

    const SEED: &str = "glorp-tank-life-fixture";

    fn earned(age_days: i64) -> Vec<EarnedTankInhabitantView> {
        crate::game::habitat::TANK_INHABITANT_CATALOG
            .iter()
            .filter(|spec| spec.unlock_age_days <= age_days)
            .map(|spec| EarnedTankInhabitantView {
                id: TankInhabitantId::new(spec.id),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                unlock_age_days: spec.unlock_age_days,
                kind: spec.kind,
                source: crate::storage::state::TankInhabitantSource::PetAgeThreshold {
                    threshold_days: spec.unlock_age_days,
                },
            })
            .collect()
    }

    fn ids(ids: &[TankInhabitantId]) -> Vec<&str> {
        ids.iter().map(|id| id.as_str()).collect()
    }

    #[test]
    fn canonical_cast_has_exact_age_and_date_fixtures() {
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(0),
                SEED,
                date!(2026 - 07 - 07),
                0
            )),
            Vec::<&str>::new()
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(1),
                SEED,
                date!(2026 - 07 - 07),
                1
            )),
            vec!["glass_shrimp"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(3),
                SEED,
                date!(2026 - 07 - 07),
                3
            )),
            vec!["glass_shrimp", "needlefish"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(7),
                SEED,
                date!(2026 - 07 - 07),
                7
            )),
            vec!["glass_snail", "glass_shrimp", "needlefish"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(21),
                SEED,
                date!(2026 - 07 - 07),
                21
            )),
            vec!["rim_skimmer", "burrower", "glass_snail", "sand_ray"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(60),
                SEED,
                date!(2026 - 07 - 07),
                60
            )),
            vec!["anemone_host", "rim_skimmer", "burrower", "glass_snail"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(60),
                SEED,
                date!(2026 - 07 - 08),
                60
            )),
            vec![
                "rim_skimmer",
                "glass_shrimp",
                "sand_ray",
                "burrower",
                "anemone_host"
            ]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(60),
                SEED,
                date!(2026 - 07 - 09),
                60
            )),
            vec!["sand_ray", "schoollet", "glass_snail", "glass_shrimp"]
        );
    }

    #[test]
    fn anemone_morph_has_exact_date_fixtures() {
        assert_eq!(
            anemone_morph_for_day(SEED, date!(2026 - 07 - 07)),
            AnemoneMorph::DotColony
        );
        assert_eq!(
            anemone_morph_for_day(SEED, date!(2026 - 07 - 08)),
            AnemoneMorph::Crown
        );
        assert_eq!(
            anemone_morph_for_day(SEED, date!(2026 - 07 - 09)),
            AnemoneMorph::Comb
        );
    }

    #[test]
    fn round_projection_caps_without_rerandomizing() {
        let canonical = canonical_daily_cast(&earned(60), SEED, date!(2026 - 07 - 08), 60);
        let geometry = TankLifeSurfaceGeometry::round_for_test(44, 18, 2);

        let projected = project_tank_life_cast(&canonical, &geometry);

        assert_eq!(
            ids(&projected.canonical_ids),
            vec![
                "rim_skimmer",
                "glass_shrimp",
                "sand_ray",
                "burrower",
                "anemone_host"
            ]
        );
        assert_eq!(
            ids(&projected.rendered_ids),
            vec!["rim_skimmer", "glass_shrimp"]
        );
        assert_eq!(projected.skipped.len(), 3);
        assert!(projected
            .skipped
            .iter()
            .all(|skip| skip.reason == TankLifeSkipReason::SurfaceBudget));
    }
}
