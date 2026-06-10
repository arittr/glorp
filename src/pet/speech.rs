//! Occasional speech-bubble lines from the pet.
//!
//! Speech is purely a function of (mood, recent token activity, current
//! wall-clock time) — deterministic, no persistence. The bubble shows for
//! ~5 seconds every ~30 seconds so it feels alive without being constant.

use time::OffsetDateTime;

use crate::game::metabolism::Mood;

/// How long (in seconds) the speech bubble stays visible within each cycle.
const SPEECH_VISIBLE_SECS: i64 = 5;

/// Total cycle length. Speech shows for the first SPEECH_VISIBLE_SECS of
/// each cycle, then hides for the rest.
const SPEECH_CYCLE_SECS: i64 = 30;

/// Effective tokens in the recent activity window above which speech defaults
/// to feeding reactions ("yum!" etc.), regardless of mood. Branch 2 will
/// re-point this onto the normalized live-activity signal.
const MUNCH_SPEECH_THRESHOLD: f64 = 30_000.0;

/// Compute the pet's current speech line, if any. Returns Some(text) for
/// the first ~5s of each 30s cycle (deterministic on `now`), None otherwise.
/// Text choice prioritizes feeding reactions when tokens have been pouring
/// in, then falls back to mood-flavored idle lines.
pub fn current_pet_speech(
    mood: Mood,
    recent_activity_tokens: f64,
    now: OffsetDateTime,
) -> Option<String> {
    let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
    if cycle_pos >= SPEECH_VISIBLE_SECS {
        return None;
    }

    if recent_activity_tokens >= MUNCH_SPEECH_THRESHOLD {
        return Some(pick_munch_phrase(now));
    }

    Some(mood_phrase(mood, now))
}

/// Show the sleep bubble only on every Nth 30s speech cycle — night is calm.
const SLEEP_SPEECH_CYCLE_N: i64 = 3;
const SLEEP_SPEECH_PHRASES: &[&str] = &["zzz...", "...zzz", "z z z"];

const DREAM_WINDOW_MINUTES: i64 = 10;

const DREAM_MIST_PHRASES: &[&str] = &["*dreams of drifting mist*", "*soft fog rolls past*"];
const DREAM_SPARKS_PHRASES: &[&str] = &["*dreams of tiny sparks*", "*sparks flicker by*"];
const DREAM_PULSE_PHRASES: &[&str] = &["*dreams in slow pulses*", "*a gentle pulse hums*"];
const DREAM_MIXED_PHRASES: &[&str] = &["*dreams of swirling colors*", "*a busy little dream*"];

const MORNING_IDLE_RATIO: f32 = 0.1;
const MORNING_MELLOW_PHRASES: &[&str] = &[
    "*stretches* still full...",
    "what a feast that was",
    "slow and cozy this morning",
];
const MORNING_RESTED_PHRASES: &[&str] = &[
    "*stretches* feeling rested!",
    "bright-eyed this morning",
    "good morning!",
];
const MORNING_FRESH_PHRASES: &[&str] = &["morning!", "*happy wiggle* a new day", "fresh and ready"];

pub fn current_pet_speech_for_scene(
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    day: &crate::tui::day::DayContext,
    now: OffsetDateTime,
) -> Option<String> {
    if day.asleep {
        let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
        let cycle_index = now.unix_timestamp().div_euclid(SPEECH_CYCLE_SECS);
        if cycle_pos >= SPEECH_VISIBLE_SECS || cycle_index.rem_euclid(SLEEP_SPEECH_CYCLE_N) != 0 {
            return None;
        }
        if in_dream_window(day.date_seed, now) {
            if let Some(line) = day
                .yesterday
                .and_then(|y| y.dominant_shape)
                .and_then(|shape| dream_phrase(shape, now))
            {
                return Some(line);
            }
        }
        let idx = cycle_index
            .div_euclid(SLEEP_SPEECH_CYCLE_N)
            .rem_euclid(SLEEP_SPEECH_PHRASES.len() as i64) as usize;
        return Some(SLEEP_SPEECH_PHRASES[idx].to_string());
    }

    let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
    if cycle_pos >= SPEECH_VISIBLE_SECS {
        return None;
    }
    if profile.burst_level >= 0.35 || profile.activity_level >= 1.25 {
        return Some(pick_munch_phrase(now));
    }
    if matches!(mood, Mood::Hungry | Mood::Sad | Mood::Wilted) {
        return Some(mood_phrase(mood, now));
    }
    if day.mature && crate::tui::day::in_morning_after_window(day, now) {
        if let Some(yesterday) = day.yesterday {
            return Some(morning_after_phrase(yesterday, now));
        }
    }
    Some(mood_phrase(mood, now))
}

fn in_dream_window(date_seed: u64, now: OffsetDateTime) -> bool {
    let hour = u64::from(now.hour());
    let mixed =
        (date_seed ^ hour.wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_mul(0x0000_0100_0000_01B3);
    let offset = (mixed % (60 - DREAM_WINDOW_MINUTES) as u64) as i64;
    let minute = i64::from(now.minute());
    minute >= offset && minute < offset + DREAM_WINDOW_MINUTES
}

fn dream_phrase(shape: crate::tui::life::WorkWeather, now: OffsetDateTime) -> Option<String> {
    let phrases: &[&str] = match shape {
        crate::tui::life::WorkWeather::CacheMist => DREAM_MIST_PHRASES,
        crate::tui::life::WorkWeather::OutputSparks => DREAM_SPARKS_PHRASES,
        crate::tui::life::WorkWeather::ReasoningPulse => DREAM_PULSE_PHRASES,
        crate::tui::life::WorkWeather::Mixed => DREAM_MIXED_PHRASES,
        crate::tui::life::WorkWeather::Clear => return None,
    };
    let idx = (now.unix_timestamp() / SPEECH_CYCLE_SECS).rem_euclid(phrases.len() as i64) as usize;
    Some(phrases[idx].to_string())
}

fn morning_after_phrase(yesterday: crate::tui::day::DaySummary, now: OffsetDateTime) -> String {
    let phrases: &[&str] = if yesterday.ratio >= crate::tui::day::FEAST_DAY_RATIO {
        MORNING_MELLOW_PHRASES
    } else if yesterday.ratio <= MORNING_IDLE_RATIO {
        MORNING_RESTED_PHRASES
    } else {
        MORNING_FRESH_PHRASES
    };
    let idx = (now.unix_timestamp() / SPEECH_CYCLE_SECS).rem_euclid(phrases.len() as i64) as usize;
    phrases[idx].to_string()
}

fn pick_munch_phrase(now: OffsetDateTime) -> String {
    const PHRASES: &[&str] = &["yum!", "more!", "tasty!", "delicious", "*chomp*"];
    let idx = (now.unix_timestamp() / SPEECH_CYCLE_SECS).rem_euclid(PHRASES.len() as i64) as usize;
    PHRASES[idx].to_string()
}

/// How long after a 'p' press the petting bubble stays visible.
pub const PETTING_BUBBLE_VISIBLE: std::time::Duration = std::time::Duration::from_secs(4);

/// Pick a reaction phrase when the user pets the pet. Selection rotates
/// deterministically off `now` so repeated petting cycles through the pool.
pub fn pick_petting_phrase(now: OffsetDateTime) -> String {
    const PHRASES: &[&str] = &[
        "*purrs*",
        "hi!",
        "*nuzzles*",
        "more pets?",
        "hehe",
        "*wiggles*",
        "thanks!",
    ];
    let idx = (now.unix_timestamp()).rem_euclid(PHRASES.len() as i64) as usize;
    PHRASES[idx].to_string()
}

/// Reaction pool when the user pets a SLEEPING pet: it stirs but stays
/// asleep — petting is affection, not food, so it never wakes the pet.
pub(crate) const SLEEP_PETTING_PHRASES: &[&str] =
    &["*snore*", "*stirs*", "...zzz", "*curls up tighter*"];

pub fn pick_sleep_petting_phrase(now: OffsetDateTime) -> String {
    let idx = (now.unix_timestamp()).rem_euclid(SLEEP_PETTING_PHRASES.len() as i64) as usize;
    SLEEP_PETTING_PHRASES[idx].to_string()
}

fn mood_phrase(mood: Mood, now: OffsetDateTime) -> String {
    let phrases: &[&str] = match mood {
        Mood::Happy => &["great job!", "feeling fantastic", "all good!", "happy days"],
        Mood::Content => &["hmm", "thinking deeply", "just chilling", "all is well"],
        Mood::Sleepy => &["zzz...", "so tired", "*yawn*", "5 more minutes"],
        Mood::Sad => &["...", "missing you", "kinda down", "*sigh*"],
        Mood::Hungry => &["feed me?", "tokens?", "hungry...", "where's the food"],
        Mood::Wilted => &["...", "running low", "need energy", "fading"],
    };
    let idx = (now.unix_timestamp() / SPEECH_CYCLE_SECS).rem_euclid(phrases.len() as i64) as usize;
    phrases[idx].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn speech_visible_in_first_few_seconds_of_cycle() {
        // Cycle boundary: a unix_timestamp % 30 == 0 should be the start of
        // the visible window.
        let visible = datetime!(2026-05-11 12:00 UTC); // unix_ts % 30 == 0 here
        assert!(current_pet_speech(Mood::Happy, 0.0, visible).is_some());

        // 4 seconds in is still visible (< SPEECH_VISIBLE_SECS = 5).
        let still_visible = visible + time::Duration::seconds(4);
        assert!(current_pet_speech(Mood::Happy, 0.0, still_visible).is_some());
    }

    #[test]
    fn speech_hidden_outside_visible_window() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let hidden = visible + time::Duration::seconds(10);
        assert!(current_pet_speech(Mood::Happy, 0.0, hidden).is_none());
    }

    #[test]
    fn munch_speech_fires_on_high_token_rate() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let speech = current_pet_speech(Mood::Content, 50_000.0, visible).unwrap();
        let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(munch_phrases.contains(&speech.as_str()));
    }

    #[test]
    fn speech_uses_profile_burst_for_munch_reaction() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile {
            burst_level: 1.0,
            ..Default::default()
        };

        let speech = current_pet_speech_for_scene(
            Mood::Content,
            &profile,
            &crate::tui::day::DayContext::default(),
            visible,
        )
        .unwrap();

        let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(munch_phrases.contains(&speech.as_str()));
    }

    #[test]
    fn speech_does_not_fake_munch_when_profile_is_idle() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile::default();

        let speech = current_pet_speech_for_scene(
            Mood::Content,
            &profile,
            &crate::tui::day::DayContext::default(),
            visible,
        )
        .unwrap();

        let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(!munch_phrases.contains(&speech.as_str()));
    }

    #[test]
    fn mood_phrase_changes_with_mood() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let happy = current_pet_speech(Mood::Happy, 0.0, visible);
        let sleepy = current_pet_speech(Mood::Sleepy, 0.0, visible);
        assert_ne!(happy, sleepy);
    }

    #[test]
    fn sleep_petting_phrases_come_from_the_sleep_pool() {
        let visible = datetime!(2026-06-09 23:30 UTC);
        for seconds in 0..10 {
            let at = visible + time::Duration::seconds(seconds);
            let phrase = pick_sleep_petting_phrase(at);
            assert!(
                SLEEP_PETTING_PHRASES.contains(&phrase.as_str()),
                "got {phrase}"
            );
        }
    }

    #[test]
    fn asleep_speech_is_a_sparse_zzz_cadence_and_suppresses_munch_and_mood_lines() {
        use crate::tui::day::DayContext;
        let cycle0 = OffsetDateTime::from_unix_timestamp(90 * (1_700_000_000 / 90)).unwrap();
        let hot_profile = crate::tui::life::PetLifeProfile {
            burst_level: 1.0,
            ..Default::default()
        };
        let asleep = DayContext {
            asleep: true,
            ..Default::default()
        };
        let line = current_pet_speech_for_scene(Mood::Hungry, &hot_profile, &asleep, cycle0);
        assert!(
            matches!(line.as_deref(), Some(l) if SLEEP_SPEECH_PHRASES.contains(&l)),
            "asleep at an eligible cycle: zzz, never munch or 'feed me?' — got {line:?}"
        );
        let cycle1 = cycle0 + time::Duration::seconds(30);
        assert_eq!(
            current_pet_speech_for_scene(Mood::Hungry, &hot_profile, &asleep, cycle1),
            None
        );
        let awake = current_pet_speech_for_scene(
            Mood::Hungry,
            &hot_profile,
            &DayContext::default(),
            cycle0,
        );
        let munch = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(
            matches!(awake.as_deref(), Some(l) if munch.contains(&l)),
            "got {awake:?}"
        );
    }

    fn dawn_day(
        yesterday: Option<crate::tui::day::DaySummary>,
        mature: bool,
    ) -> crate::tui::day::DayContext {
        crate::tui::day::DayContext {
            day_phase: crate::tui::day::DayPhase::Dawn,
            mature,
            yesterday,
            ..Default::default()
        }
    }

    #[test]
    fn hungry_at_dawn_after_an_idle_yesterday_shows_the_vitals_line_not_a_greeting() {
        use crate::tui::day::DaySummary;
        let visible = datetime!(2026-05-11 12:00 UTC);
        let day = dawn_day(
            Some(DaySummary {
                ratio: 0.0,
                dominant_shape: None,
            }),
            true,
        );
        let line = current_pet_speech_for_scene(
            Mood::Hungry,
            &crate::tui::life::PetLifeProfile::default(),
            &day,
            visible,
        )
        .unwrap();
        let hungry = ["feed me?", "tokens?", "hungry...", "where's the food"];
        assert!(
            hungry.contains(&line.as_str()),
            "needy vitals outrank morning flavor, got {line}"
        );
    }

    #[test]
    fn morning_flavor_fires_for_observed_idle_yesterday_but_not_missing_coverage() {
        use crate::tui::day::DaySummary;
        let visible = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile::default();
        let content = ["hmm", "thinking deeply", "just chilling", "all is well"];

        let observed_idle = dawn_day(
            Some(DaySummary {
                ratio: 0.0,
                dominant_shape: None,
            }),
            true,
        );
        let line =
            current_pet_speech_for_scene(Mood::Content, &profile, &observed_idle, visible).unwrap();
        assert!(
            MORNING_RESTED_PHRASES.contains(&line.as_str()),
            "Some(0.0) selects the rested flavor, got {line}"
        );

        let feast = dawn_day(
            Some(DaySummary {
                ratio: 2.0,
                dominant_shape: None,
            }),
            true,
        );
        let line = current_pet_speech_for_scene(Mood::Content, &profile, &feast, visible).unwrap();
        assert!(
            MORNING_MELLOW_PHRASES.contains(&line.as_str()),
            "a feast yesterday reads mellow, got {line}"
        );

        let no_coverage = dawn_day(None, true);
        let line =
            current_pet_speech_for_scene(Mood::Content, &profile, &no_coverage, visible).unwrap();
        assert!(
            content.contains(&line.as_str()),
            "None must fall through to the mood line, got {line}"
        );

        let immature = dawn_day(
            Some(DaySummary {
                ratio: 0.0,
                dominant_shape: None,
            }),
            false,
        );
        let line =
            current_pet_speech_for_scene(Mood::Content, &profile, &immature, visible).unwrap();
        assert!(
            content.contains(&line.as_str()),
            "immature must fall through to the mood line, got {line}"
        );
    }

    #[test]
    fn dream_windows_are_deterministic_and_need_yesterdays_shape_detail() {
        use crate::tui::day::{DayContext, DaySummary};
        use crate::tui::life::WorkWeather;
        let base = datetime!(2026-05-11 12:00 UTC);
        let sparks_day = DayContext {
            asleep: true,
            date_seed: 7,
            yesterday: Some(DaySummary {
                ratio: 1.2,
                dominant_shape: Some(WorkWeather::OutputSparks),
            }),
            ..Default::default()
        };
        let profile = crate::tui::life::PetLifeProfile::default();
        let scan = |day: &DayContext| -> Vec<bool> {
            (0..40_i64)
                .map(|k| {
                    let at = base + time::Duration::seconds(k * 90);
                    match current_pet_speech_for_scene(Mood::Content, &profile, day, at) {
                        Some(line) => {
                            assert!(
                                SLEEP_SPEECH_PHRASES.contains(&line.as_str())
                                    || DREAM_SPARKS_PHRASES.contains(&line.as_str()),
                                "asleep lines are zzz or this family's dreams only, got {line}"
                            );
                            DREAM_SPARKS_PHRASES.contains(&line.as_str())
                        }
                        None => panic!("every probe sits on an eligible visible slot"),
                    }
                })
                .collect()
        };
        let pass1 = scan(&sparks_day);
        let pass2 = scan(&sparks_day);
        assert_eq!(pass1, pass2, "dream windows must be restart-deterministic");
        let dream_probes = pass1.iter().filter(|&&d| d).count();
        assert!(
            (5..=8).contains(&dream_probes),
            "one ~10-minute window sampled every 90s, got {dream_probes}"
        );
        let first = pass1.iter().position(|&d| d).unwrap();
        let last = pass1.iter().rposition(|&d| d).unwrap();
        assert!(
            pass1[first..=last].iter().all(|&d| d),
            "dream probes must form one contiguous window"
        );

        for yesterday in [
            None,
            Some(DaySummary {
                ratio: 0.7,
                dominant_shape: None,
            }),
            Some(DaySummary {
                ratio: 0.7,
                dominant_shape: Some(WorkWeather::Clear),
            }),
        ] {
            let day = DayContext {
                yesterday,
                ..sparks_day
            };
            let any_dream = scan(&day).into_iter().any(|d| d);
            assert!(
                !any_dream,
                "no signal must mean zzz only, got a dream for {yesterday:?}"
            );
        }
    }
}
