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

/// Compute speech from the live presentation profile. Uses the same visibility
/// cycle as token-based speech while deriving feeding reactions from normalized
/// live activity instead of raw recent token totals.
pub fn current_pet_speech_for_profile(
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    now: OffsetDateTime,
) -> Option<String> {
    let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
    if cycle_pos >= SPEECH_VISIBLE_SECS {
        return None;
    }

    if profile.burst_level >= 0.35 || profile.activity_level >= 1.25 {
        return Some(pick_munch_phrase(now));
    }

    Some(mood_phrase(mood, now))
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

        let speech = current_pet_speech_for_profile(Mood::Content, &profile, visible).unwrap();

        let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(munch_phrases.contains(&speech.as_str()));
    }

    #[test]
    fn speech_does_not_fake_munch_when_profile_is_idle() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile::default();

        let speech = current_pet_speech_for_profile(Mood::Content, &profile, visible).unwrap();

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
}
