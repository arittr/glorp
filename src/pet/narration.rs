//! Character-driven feed entries narrated from real pet events.
//!
//! All functions are pure — no side effects. Variant selection uses the same
//! deterministic `(now.unix_timestamp() / 30) % variants.len()` pattern as
//! `src/pet/speech.rs` so a burst of similar events doesn't all use the same word.

use time::OffsetDateTime;

use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::storage::state::Vitals;

/// Token volume bucket for a single poll cycle.
pub enum PollBucket {
    Feast,
    Munch,
    Nibble,
    Sip,
}

/// Map effective token count to a bucket. Returns `None` for zero tokens.
pub fn poll_bucket(effective_tokens: f64) -> Option<PollBucket> {
    if effective_tokens >= 50_000.0 {
        Some(PollBucket::Feast)
    } else if effective_tokens >= 10_000.0 {
        Some(PollBucket::Munch)
    } else if effective_tokens >= 1_000.0 {
        Some(PollBucket::Nibble)
    } else if effective_tokens > 0.0 {
        Some(PollBucket::Sip)
    } else {
        None
    }
}

/// Produce a narrated eating line for a poll cycle.
pub fn poll_phrase(name: &str, bucket: PollBucket, now: OffsetDateTime) -> String {
    let variants: &[&str] = match bucket {
        PollBucket::Feast => &["{name} feasted", "{name} devoured a token cache"],
        PollBucket::Munch => &["{name} munched", "{name} gobbled"],
        PollBucket::Nibble => &["{name} nibbled", "{name} snacked"],
        PollBucket::Sip => &["{name} sipped", "{name} tasted"],
    };
    let idx = pick_idx(now, variants.len());
    variants[idx].replace("{name}", name)
}

/// Produce an evolution line for a stage transition.
pub fn stage_phrase(name: &str, new_stage: Stage) -> String {
    format!("{name} evolved into {}", new_stage.as_str())
}

/// Produce a mood-change narration line.
pub fn mood_phrase(name: &str, to: Mood, now: OffsetDateTime) -> String {
    let variants: &[&str] = match to {
        Mood::Happy => &["{name} brightened", "{name} looks great"],
        Mood::Content => &["{name} settled", "{name} relaxed"],
        Mood::Sleepy => &["{name} grew drowsy", "{name} yawned"],
        Mood::Hungry => &["{name} grew hungry", "{name} got peckish"],
        Mood::Sad => &["{name} slumped", "{name} looks down"],
        Mood::Wilted => &["{name} faded", "{name} dimmed"],
    };
    let idx = pick_idx(now, variants.len());
    variants[idx].replace("{name}", name)
}

/// Vital threshold crossing type.
pub enum VitalCrossing {
    Fed,
    Energy,
}

/// Detect if a vital just crossed a low threshold going downward.
///
/// Fires only on the downward crossing (previous above threshold, current below).
pub fn vital_crossing(previous: Vitals, current: Vitals) -> Option<VitalCrossing> {
    const FED_LOW: f64 = 30.0;
    const ENERGY_LOW: f64 = 30.0;

    if previous.fed >= FED_LOW && current.fed < FED_LOW {
        return Some(VitalCrossing::Fed);
    }
    if previous.energy >= ENERGY_LOW && current.energy < ENERGY_LOW {
        return Some(VitalCrossing::Energy);
    }
    None
}

/// Produce a vital threshold narration line.
pub fn vital_phrase(name: &str, crossing: VitalCrossing) -> String {
    match crossing {
        VitalCrossing::Fed => format!("{name} needs food"),
        VitalCrossing::Energy => format!("{name} looks tired"),
    }
}

/// Produce an idle drift narration line.
pub fn idle_phrase(name: &str, now: OffsetDateTime) -> String {
    let variants: &[&str] = &["{name} drifted off", "{name} dreams"];
    let idx = pick_idx(now, variants.len());
    variants[idx].replace("{name}", name)
}

/// Deterministic variant picker. Slots rotate in 30-second windows so rapid
/// successive events at different timestamps naturally cycle through the pool.
fn pick_idx(now: OffsetDateTime, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((now.unix_timestamp() / 30).rem_euclid(len as i64)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    // ── poll_bucket ──────────────────────────────────────────────────────────

    #[test]
    fn poll_bucket_zero_returns_none() {
        assert!(poll_bucket(0.0).is_none());
    }

    #[test]
    fn poll_bucket_below_1k_is_sip() {
        assert!(matches!(poll_bucket(999.9), Some(PollBucket::Sip)));
        assert!(matches!(poll_bucket(1.0), Some(PollBucket::Sip)));
    }

    #[test]
    fn poll_bucket_at_1k_is_nibble() {
        assert!(matches!(poll_bucket(1_000.0), Some(PollBucket::Nibble)));
    }

    #[test]
    fn poll_bucket_below_10k_is_nibble() {
        assert!(matches!(poll_bucket(9_999.9), Some(PollBucket::Nibble)));
    }

    #[test]
    fn poll_bucket_at_10k_is_munch() {
        assert!(matches!(poll_bucket(10_000.0), Some(PollBucket::Munch)));
    }

    #[test]
    fn poll_bucket_below_50k_is_munch() {
        assert!(matches!(poll_bucket(49_999.9), Some(PollBucket::Munch)));
    }

    #[test]
    fn poll_bucket_at_50k_is_feast() {
        assert!(matches!(poll_bucket(50_000.0), Some(PollBucket::Feast)));
    }

    #[test]
    fn poll_bucket_above_50k_is_feast() {
        assert!(matches!(poll_bucket(100_000.0), Some(PollBucket::Feast)));
    }

    // ── poll_phrase ──────────────────────────────────────────────────────────

    #[test]
    fn poll_phrase_is_deterministic_for_same_inputs() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let a = poll_phrase("buddy", PollBucket::Munch, now);
        let b = poll_phrase("buddy", PollBucket::Munch, now);
        assert_eq!(a, b);
    }

    #[test]
    fn poll_phrase_contains_pet_name() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let phrase = poll_phrase("kira", PollBucket::Feast, now);
        assert!(
            phrase.contains("kira"),
            "phrase should contain name: {phrase}"
        );
    }

    #[test]
    fn poll_phrase_cycles_variants_as_now_changes() {
        // Advance time by 30-second increments until both feast variants appear.
        let base = datetime!(2026-05-11 12:00 UTC);
        let feast_variants = ["feasted", "devoured"];
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for tick in 0..4i64 {
            let now = base + time::Duration::seconds(tick * 30);
            let phrase = poll_phrase("buddy", PollBucket::Feast, now);
            for v in &feast_variants {
                if phrase.contains(v) {
                    seen.insert(v);
                }
            }
        }
        assert_eq!(
            seen.len(),
            2,
            "both feast variants should appear across ticks"
        );
    }

    // ── stage_phrase ─────────────────────────────────────────────────────────

    #[test]
    fn stage_phrase_includes_stage_label() {
        let phrase = stage_phrase("buddy", Stage::S4);
        assert!(phrase.contains("s4"), "phrase: {phrase}");
        assert!(phrase.contains("buddy"), "phrase: {phrase}");
    }

    // ── mood_phrase ──────────────────────────────────────────────────────────

    #[test]
    fn mood_phrase_happy_contains_name() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let phrase = mood_phrase("kira", Mood::Happy, now);
        assert!(phrase.contains("kira"));
    }

    #[test]
    fn mood_phrase_each_variant_is_representable() {
        let base = datetime!(2026-05-11 12:00 UTC);
        for mood in [
            Mood::Happy,
            Mood::Content,
            Mood::Sleepy,
            Mood::Hungry,
            Mood::Sad,
            Mood::Wilted,
        ] {
            for tick in 0..2i64 {
                let now = base + time::Duration::seconds(tick * 30);
                let phrase = mood_phrase("test", mood, now);
                assert!(
                    !phrase.is_empty(),
                    "empty phrase for {mood:?} at tick {tick}"
                );
                assert!(
                    phrase.contains("test"),
                    "phrase should contain name for {mood:?}: {phrase}"
                );
            }
        }
    }

    // ── vital_crossing ───────────────────────────────────────────────────────

    fn vitals(fed: f64, energy: f64) -> Vitals {
        Vitals {
            fed,
            happiness: 70.0,
            energy,
        }
    }

    #[test]
    fn vital_crossing_fed_detected_crossing_downward() {
        let prev = vitals(35.0, 70.0);
        let curr = vitals(25.0, 70.0);
        assert!(matches!(
            vital_crossing(prev, curr),
            Some(VitalCrossing::Fed)
        ));
    }

    #[test]
    fn vital_crossing_fed_not_triggered_on_upward() {
        let prev = vitals(25.0, 70.0);
        let curr = vitals(35.0, 70.0);
        assert!(vital_crossing(prev, curr).is_none());
    }

    #[test]
    fn vital_crossing_fed_not_triggered_when_both_below() {
        let prev = vitals(20.0, 70.0);
        let curr = vitals(15.0, 70.0);
        assert!(vital_crossing(prev, curr).is_none());
    }

    #[test]
    fn vital_crossing_fed_not_triggered_when_both_above() {
        let prev = vitals(50.0, 70.0);
        let curr = vitals(40.0, 70.0);
        assert!(vital_crossing(prev, curr).is_none());
    }

    #[test]
    fn vital_crossing_energy_detected_crossing_downward() {
        let prev = vitals(70.0, 35.0);
        let curr = vitals(70.0, 25.0);
        assert!(matches!(
            vital_crossing(prev, curr),
            Some(VitalCrossing::Energy)
        ));
    }

    #[test]
    fn vital_crossing_energy_not_triggered_on_upward() {
        let prev = vitals(70.0, 25.0);
        let curr = vitals(70.0, 35.0);
        assert!(vital_crossing(prev, curr).is_none());
    }

    // ── idle_phrase ──────────────────────────────────────────────────────────

    #[test]
    fn idle_phrase_contains_name() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let phrase = idle_phrase("kit", now);
        assert!(phrase.contains("kit"), "phrase: {phrase}");
    }

    #[test]
    fn idle_phrase_cycles_variants() {
        let base = datetime!(2026-05-11 12:00 UTC);
        let mut phrases: std::collections::HashSet<String> = std::collections::HashSet::new();
        for tick in 0..4i64 {
            let now = base + time::Duration::seconds(tick * 30);
            phrases.insert(idle_phrase("buddy", now));
        }
        assert_eq!(phrases.len(), 2, "both idle variants should appear");
    }
}
