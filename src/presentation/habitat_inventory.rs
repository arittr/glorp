use crate::game::habitat::HabitatPropKind;
use crate::storage::state::TankInhabitantId;

const MAX_TROPHIES: usize = 6;
const MAX_ACCENTS: usize = 4;
const ACCENT_ROTATION_SECS: i64 = 600;

#[derive(Debug, Clone, Copy)]
pub(crate) struct HabitatPropRecord<'a> {
    pub(crate) id: &'a str,
    pub(crate) earned_at: time::OffsetDateTime,
    pub(crate) kind: HabitatPropKind,
    pub(crate) display_priority: i16,
}

pub(crate) fn visible_trophy_ids<'a>(props: &[HabitatPropRecord<'a>]) -> Vec<&'a str> {
    let mut props = props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Trophy)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| {
        b.display_priority
            .cmp(&a.display_priority)
            .then_with(|| a.earned_at.cmp(&b.earned_at))
            .then_with(|| a.id.cmp(b.id))
    });
    props
        .into_iter()
        .take(MAX_TROPHIES)
        .map(|prop| prop.id)
        .collect()
}

pub(crate) fn visible_accent_ids<'a>(
    props: &[HabitatPropRecord<'a>],
    now: time::OffsetDateTime,
) -> Vec<&'a str> {
    let props = sorted_accent_ids(props);
    if props.len() <= MAX_ACCENTS {
        return props;
    }
    let start =
        (now.unix_timestamp() / ACCENT_ROTATION_SECS).rem_euclid(props.len() as i64) as usize;
    (0..MAX_ACCENTS)
        .map(|offset| props[(start + offset) % props.len()])
        .collect()
}

pub(crate) fn sorted_accent_ids<'a>(props: &[HabitatPropRecord<'a>]) -> Vec<&'a str> {
    let mut props = props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Accent)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| a.earned_at.cmp(&b.earned_at).then_with(|| a.id.cmp(b.id)));
    props.into_iter().map(|prop| prop.id).collect()
}

pub fn canonical_daily_cast(
    unlocked: &[TankInhabitantId],
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> Vec<TankInhabitantId> {
    let mut known = unlocked
        .iter()
        .filter(|id| crate::game::habitat::tank_inhabitant_spec(id).is_some())
        .collect::<Vec<_>>();
    known.sort();
    let target = canonical_target_count(known.len(), pet_seed, local_date, calendar_age_days);
    if target == 0 {
        return Vec::new();
    }
    let mut scored = known
        .into_iter()
        .map(|id| {
            let score = stable_hash(&format!(
                "tank-life-cast-v1|{pet_seed}|{local_date}|{}",
                id.as_str()
            ));
            (score, id.clone())
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
