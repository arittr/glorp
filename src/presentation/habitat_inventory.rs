use crate::game::habitat::HabitatPropKind;
use crate::storage::state::TankInhabitantId;
use crate::tui::view_model::{EarnedTankInhabitantView, HabitatView};

const MAX_TROPHIES: usize = 6;
const MAX_ACCENTS: usize = 4;
const ACCENT_ROTATION_SECS: i64 = 600;

pub(crate) fn visible_trophy_ids(habitat: &HabitatView) -> Vec<&str> {
    let mut props = habitat
        .earned_props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Trophy)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| {
        b.display_priority
            .cmp(&a.display_priority)
            .then_with(|| a.earned_at.cmp(&b.earned_at))
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });
    props
        .into_iter()
        .take(MAX_TROPHIES)
        .map(|prop| prop.id.as_str())
        .collect()
}

pub(crate) fn visible_accent_ids(habitat: &HabitatView, now: time::OffsetDateTime) -> Vec<&str> {
    let props = sorted_accent_ids(habitat);
    if props.len() <= MAX_ACCENTS {
        return props;
    }
    let start =
        (now.unix_timestamp() / ACCENT_ROTATION_SECS).rem_euclid(props.len() as i64) as usize;
    (0..MAX_ACCENTS)
        .map(|offset| props[(start + offset) % props.len()])
        .collect()
}

pub(crate) fn sorted_accent_ids(habitat: &HabitatView) -> Vec<&str> {
    let mut props = habitat
        .earned_props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Accent)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| {
        a.earned_at
            .cmp(&b.earned_at)
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });
    props.into_iter().map(|prop| prop.id.as_str()).collect()
}

pub fn canonical_daily_cast(
    unlocked: &[EarnedTankInhabitantView],
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

fn stable_hash(input: &str) -> u64 {
    const OFFSET: u64 = 1_469_598_103_934_665_603;
    const PRIME: u64 = 1_099_511_628_211;
    input.bytes().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
        hash
    })
}
