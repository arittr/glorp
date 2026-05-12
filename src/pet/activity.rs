//! Pet activity feed generator.
//!
//! Derives short "thought" or "action" lines from observable state — token
//! spikes, stage transitions, idle periods, current mood — and produces them
//! as timestamped EventView entries that get merged into the watch feed.
//!
//! Everything is deterministic on (pet seed, current state, time). No new
//! persistence; if you re-build the view model with the same inputs you get
//! the same activities.

use time::OffsetDateTime;

use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::generation::Species;
use crate::storage::usage_store::NormalizedUsageEvent;
use crate::tui::style::LogKind;
use crate::tui::view_model::EventView;

/// Minimum token delta within a 10-minute window to count as a "munching"
/// activity. Anything below this is too small to trigger a thought line.
const MUNCH_TOKEN_THRESHOLD: f64 = 100_000.0;

/// If no usage events landed in the past this many minutes, emit one idle
/// thought line so the feed doesn't go silent during quiet stretches.
const IDLE_THOUGHT_MINUTES: i64 = 20;

/// Generate up to 3 pet activity entries from current state. Returns
/// EventView rows ready to be merged with usage and diagnostic events in
/// the feed log. Caller is responsible for the merge + sort by timestamp.
pub fn derive_pet_activities(
    pet_name: &str,
    species: Species,
    mood: Mood,
    usage_events: &[NormalizedUsageEvent],
    seen_stage_transitions: &[Stage],
    now: OffsetDateTime,
) -> Vec<EventView> {
    let mut out = Vec::new();

    // 1. Most recent stage transition (if any) → evolution activity.
    if let Some(last) = seen_stage_transitions.last() {
        out.push(EventView {
            timestamp: format_hhmm(now),
            kind: LogKind::PetActivity,
            text: format!("{pet_name} evolved into {last}"),
        });
    }

    // 2. Token spike in the last hour → munching activity.
    if let Some(spike_total) = recent_munch_spike(usage_events, now) {
        let pretty = format_tokens_short(spike_total);
        out.push(EventView {
            timestamp: format_hhmm(now),
            kind: LogKind::PetActivity,
            text: format!("{pet_name} munched {pretty} tokens"),
        });
    }

    // 3. Idle thought — if nothing happened in IDLE_THOUGHT_MINUTES, the
    //    pet "fills the silence" with a species-flavored idle line.
    if is_idle(usage_events, now) {
        let idle_text = idle_thought(pet_name, species, mood, now);
        out.push(EventView {
            timestamp: format_hhmm(now),
            kind: LogKind::PetActivity,
            text: idle_text,
        });
    }

    out
}

fn format_hhmm(t: OffsetDateTime) -> String {
    format!("{:02}:{:02}", t.hour(), t.minute())
}

/// Sum effective tokens from the last hour. Returns Some(total) if total >=
/// MUNCH_TOKEN_THRESHOLD, None otherwise.
fn recent_munch_spike(usage_events: &[NormalizedUsageEvent], now: OffsetDateTime) -> Option<f64> {
    let cutoff = now - time::Duration::hours(1);
    let total: f64 = usage_events
        .iter()
        .filter(|e| e.observed_at >= cutoff)
        .map(|e| e.effective_tokens)
        .sum();
    if total >= MUNCH_TOKEN_THRESHOLD {
        Some(total)
    } else {
        None
    }
}

fn is_idle(usage_events: &[NormalizedUsageEvent], now: OffsetDateTime) -> bool {
    let cutoff = now - time::Duration::minutes(IDLE_THOUGHT_MINUTES);
    !usage_events.iter().any(|e| e.observed_at >= cutoff)
}

/// Picks an idle thought line. Mood overrides species when the mood is
/// strong (sleepy / sad / wilted); otherwise the species' personality
/// shines through.
fn idle_thought(pet_name: &str, species: Species, mood: Mood, now: OffsetDateTime) -> String {
    // Mood-driven overrides first.
    match mood {
        Mood::Sleepy => return format!("{pet_name} is dozing"),
        Mood::Wilted => return format!("{pet_name} is low on energy"),
        Mood::Sad => return format!("{pet_name} is moping a little"),
        _ => {}
    }

    let catalog: &[&str] = match species {
        Species::Mech => &[
            "is oiling its gears",
            "is recalibrating sensors",
            "is scanning for updates",
            "is optimizing routines",
            "is humming subroutines",
        ],
        Species::Glitch => &[
            "is recalibrating frequencies",
            "is defragging memory",
            "is parsing tokens",
            "is dreaming in binary",
            "is patching itself",
        ],
        Species::Blob => &[
            "is jiggling happily",
            "is absorbing nutrients",
            "is considering its shape",
            "is perfectly content",
            "is wiggling softly",
        ],
        Species::Fuzz => &[
            "is grooming",
            "is purring softly",
            "is snuggling into the buffer",
            "is pawing at the screen",
            "is curled up",
        ],
        Species::Ghost => &[
            "is drifting",
            "is contemplating",
            "is watching silently",
            "is whispering",
            "is fading in and out",
        ],
        Species::Crystal => &[
            "is refracting light",
            "is resonating",
            "is growing slowly",
            "is faceting",
            "is humming a clear tone",
        ],
    };

    let idx = (now.unix_timestamp() / 60).rem_euclid(catalog.len() as i64) as usize;
    format!("{pet_name} {}", catalog[idx])
}

fn format_tokens_short(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn usage_event(observed_at: OffsetDateTime, tokens: f64) -> NormalizedUsageEvent {
        NormalizedUsageEvent {
            observed_at,
            effective_tokens: tokens,
            ..NormalizedUsageEvent::for_test_at(observed_at, tokens)
        }
    }

    #[test]
    fn munch_spike_fires_when_total_exceeds_threshold() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let events = vec![
            usage_event(now - time::Duration::minutes(5), 60_000.0),
            usage_event(now - time::Duration::minutes(15), 50_000.0),
        ];
        let acts =
            derive_pet_activities("vex-jit", Species::Glitch, Mood::Happy, &events, &[], now);
        assert!(acts.iter().any(|e| e.text.contains("munched")));
    }

    #[test]
    fn munch_spike_does_not_fire_below_threshold() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let events = vec![usage_event(now - time::Duration::minutes(5), 5_000.0)];
        let acts = derive_pet_activities("vex", Species::Blob, Mood::Happy, &events, &[], now);
        assert!(!acts.iter().any(|e| e.text.contains("munched")));
    }

    #[test]
    fn idle_thought_appears_when_no_recent_activity() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let acts = derive_pet_activities("vex", Species::Mech, Mood::Happy, &[], &[], now);
        assert!(acts.iter().any(|e| e.text.contains("vex")));
        assert!(acts.iter().any(|e| e.kind == LogKind::PetActivity));
    }

    #[test]
    fn idle_thought_uses_mood_override_when_sleepy() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let acts = derive_pet_activities("vex", Species::Blob, Mood::Sleepy, &[], &[], now);
        assert!(acts.iter().any(|e| e.text.contains("dozing")));
    }

    #[test]
    fn idle_thought_is_species_flavored() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let mech_acts = derive_pet_activities("m", Species::Mech, Mood::Happy, &[], &[], now);
        let mech_text = mech_acts.iter().find(|e| !e.text.contains("evolved"));
        // Mech catalog includes mechanical/technical verbs.
        let has_mech = mech_text.is_some_and(|e| {
            e.text.contains("gears")
                || e.text.contains("sensor")
                || e.text.contains("subroutine")
                || e.text.contains("routine")
                || e.text.contains("scan")
        });
        assert!(
            has_mech,
            "expected a mech-flavored idle thought: {mech_text:?}"
        );
    }

    #[test]
    fn stage_transition_emits_evolution_activity() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let transitions = vec![Stage::S4];
        let acts =
            derive_pet_activities("vex", Species::Glitch, Mood::Happy, &[], &transitions, now);
        assert!(acts.iter().any(|e| e.text.contains("evolved into s4")));
    }

    #[test]
    fn format_tokens_short_renders_pretty_units() {
        assert_eq!(format_tokens_short(750.0), "750");
        // 1250 / 1000 = 1.25, Rust's default rounding rounds-half-to-even here.
        assert_eq!(format_tokens_short(1_250.0), "1.2k");
        assert_eq!(format_tokens_short(123_456.0), "123.5k");
        assert_eq!(format_tokens_short(2_345_678.0), "2.3M");
    }
}
