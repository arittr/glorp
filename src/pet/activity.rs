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

use crate::format::format_tokens;
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
    local_offset: time::UtcOffset,
) -> Vec<EventView> {
    let mut out = Vec::new();

    // 1. Most recent stage transition (if any) → evolution activity.
    if let Some(last) = seen_stage_transitions.last() {
        out.push(EventView {
            timestamp: format_hhmm_local(now, local_offset),
            kind: LogKind::PetActivity,
            text: format!("{pet_name} evolved into {last}"),
        });
    }

    // 2. Token spike in the last hour → munching activity.
    if let Some(spike_total) = recent_munch_spike(usage_events, now) {
        let pretty = format_tokens(spike_total);
        out.push(EventView {
            timestamp: format_hhmm_local(now, local_offset),
            kind: LogKind::PetActivity,
            text: format!("{pet_name} munched {pretty} tokens"),
        });
    }

    // 3. Idle thought — if nothing happened in IDLE_THOUGHT_MINUTES, the
    //    pet "fills the silence" with a species-flavored idle line.
    if is_idle(usage_events, now) {
        let idle_text = idle_thought(pet_name, species, mood, now);
        out.push(EventView {
            timestamp: format_hhmm_local(now, local_offset),
            kind: LogKind::PetActivity,
            text: idle_text,
        });
    }

    out
}

/// Generate at most one sparse activity line from the live presentation
/// profile. Usage-event rows stay owned by `derive_pet_activities`; this only
/// adds a profile-flavored reaction when the live scene is hot enough.
pub fn derive_profile_pet_activities(
    pet_name: &str,
    species: Species,
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    now: OffsetDateTime,
    local_offset: time::UtcOffset,
) -> Vec<EventView> {
    if profile.burst_level < 0.35 && profile.activity_level < 1.25 {
        return Vec::new();
    }

    let verb = match (profile.work_weather, species, mood) {
        (crate::tui::life::WorkWeather::CacheMist, _, _) => "is glowing through cached light",
        (crate::tui::life::WorkWeather::OutputSparks, _, _) => "sparked at the edges",
        (crate::tui::life::WorkWeather::ReasoningPulse, _, _) => "pulsed thoughtfully",
        (_, Species::Crystal, _) => "rang softly with work",
        (_, _, Mood::Sleepy) => "perked up",
        _ => "brightened",
    };

    vec![EventView {
        timestamp: format_hhmm_local(now, local_offset),
        kind: LogKind::PetActivity,
        text: format!("{pet_name} {verb}"),
    }]
}

/// Format an instant as a local-clock `hh:mm` label. All EventView timestamp
/// formatting goes through this; callers thread the offset (vm build:
/// `mapper.offset_at(now)`; install paths: `LocalDayMapper::System`).
pub fn format_hhmm_local(now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let local = now.to_offset(offset);
    format!("{:02}:{:02}", local.hour(), local.minute())
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
        let acts = derive_pet_activities(
            "vex-jit",
            Species::Glitch,
            Mood::Happy,
            &events,
            &[],
            now,
            time::UtcOffset::UTC,
        );
        assert!(acts.iter().any(|e| e.text.contains("munched")));
    }

    #[test]
    fn munch_spike_does_not_fire_below_threshold() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let events = vec![usage_event(now - time::Duration::minutes(5), 5_000.0)];
        let acts = derive_pet_activities(
            "vex",
            Species::Blob,
            Mood::Happy,
            &events,
            &[],
            now,
            time::UtcOffset::UTC,
        );
        assert!(!acts.iter().any(|e| e.text.contains("munched")));
    }

    #[test]
    fn idle_thought_appears_when_no_recent_activity() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let acts = derive_pet_activities(
            "vex",
            Species::Mech,
            Mood::Happy,
            &[],
            &[],
            now,
            time::UtcOffset::UTC,
        );
        assert!(acts.iter().any(|e| e.text.contains("vex")));
        assert!(acts.iter().any(|e| e.kind == LogKind::PetActivity));
    }

    #[test]
    fn idle_thought_uses_mood_override_when_sleepy() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let acts = derive_pet_activities(
            "vex",
            Species::Blob,
            Mood::Sleepy,
            &[],
            &[],
            now,
            time::UtcOffset::UTC,
        );
        assert!(acts.iter().any(|e| e.text.contains("dozing")));
    }

    #[test]
    fn profile_activity_adds_sparse_live_line_for_hot_profile() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile {
            activity_level: 1.5,
            burst_level: 0.8,
            ..Default::default()
        };
        let acts = derive_profile_pet_activities(
            "luxopal",
            Species::Crystal,
            Mood::Happy,
            &profile,
            now,
            time::UtcOffset::UTC,
        );

        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].kind, LogKind::PetActivity);
        assert!(acts[0].text.contains("luxopal"));
    }

    #[test]
    fn profile_activity_stays_silent_for_quiet_recent_profile() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile::default();
        let acts = derive_profile_pet_activities(
            "luxopal",
            Species::Crystal,
            Mood::Happy,
            &profile,
            now,
            time::UtcOffset::UTC,
        );

        assert!(acts.is_empty());
    }

    #[test]
    fn idle_thought_is_species_flavored() {
        let now = datetime!(2026-05-11 12:00 UTC);
        let mech_acts = derive_pet_activities(
            "m",
            Species::Mech,
            Mood::Happy,
            &[],
            &[],
            now,
            time::UtcOffset::UTC,
        );
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
        let acts = derive_pet_activities(
            "vex",
            Species::Glitch,
            Mood::Happy,
            &[],
            &transitions,
            now,
            time::UtcOffset::UTC,
        );
        assert!(acts.iter().any(|e| e.text.contains("evolved into s4")));
    }

    #[test]
    fn format_hhmm_local_renders_the_offset_clock_not_utc() {
        let now = datetime!(2026-06-09 06:00 UTC); // 23:00 the previous evening at UTC-7
        let offset = time::UtcOffset::from_hms(-7, 0, 0).unwrap();
        assert_eq!(format_hhmm_local(now, offset), "23:00");
        assert_eq!(format_hhmm_local(now, time::UtcOffset::UTC), "06:00");
    }

    #[test]
    fn activity_timestamps_thread_the_local_offset() {
        let now = datetime!(2026-06-09 03:10 UTC);
        let offset = time::UtcOffset::from_hms(-8, 0, 0).unwrap(); // 19:10 local
        let acts = derive_pet_activities("vex", Species::Mech, Mood::Happy, &[], &[], now, offset);
        assert!(!acts.is_empty());
        assert!(
            acts.iter().all(|e| e.timestamp == "19:10"),
            "expected local 19:10 stamps: {acts:?}"
        );
    }
}
