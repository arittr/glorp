use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use time::{OffsetDateTime, Weekday};

use crate::error::GlorpError;

const ACTIVE_HOUR_WEIGHT: f64 = 1.0;
const OVERNIGHT_WEIGHT: f64 = 0.35;
const DEFAULT_WEEKEND_WEIGHT: f64 = 0.45;
const LEARNED_WEEKEND_WEIGHT: f64 = 0.75;
const INACTIVE_HOUR_WEIGHT: f64 = 0.20;
const MIN_ACTIVE_TIMESTAMPS_FOR_RHYTHM: usize = 5;
const WEEKEND_HEAVY_SHARE: f64 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vitals {
    pub fed: f64,
    pub happiness: f64,
    pub energy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mood {
    Happy,
    Ecstatic,
    Content,
    Hungry,
    Sad,
    Sleepy,
    Wilted,
}

impl Mood {
    /// Matches the serde wire format (`#[serde(rename_all = "lowercase")]`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Mood::Happy => "happy",
            Mood::Ecstatic => "ecstatic",
            Mood::Content => "content",
            Mood::Hungry => "hungry",
            Mood::Sad => "sad",
            Mood::Sleepy => "sleepy",
            Mood::Wilted => "wilted",
        }
    }
}

impl fmt::Display for Mood {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Mood {
    type Err = GlorpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "happy" => Ok(Mood::Happy),
            "ecstatic" => Ok(Mood::Ecstatic),
            "content" => Ok(Mood::Content),
            "hungry" => Ok(Mood::Hungry),
            "sad" => Ok(Mood::Sad),
            "sleepy" => Ok(Mood::Sleepy),
            "wilted" => Ok(Mood::Wilted),
            other => Err(GlorpError::Message(format!("unknown mood {other:?}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RhythmProfile {
    pub active_hours: [bool; 24],
    pub weekend_activity_weight: f64,
}

impl Default for RhythmProfile {
    fn default() -> Self {
        let mut active_hours = [false; 24];
        for active_hour in active_hours.iter_mut().take(18).skip(9) {
            *active_hour = true;
        }
        Self {
            active_hours,
            weekend_activity_weight: DEFAULT_WEEKEND_WEIGHT,
        }
    }
}

impl RhythmProfile {
    pub fn from_history(history: &[super::calibration::DailyUsage]) -> Self {
        let active_days = history
            .iter()
            .filter(|day| day.effective_tokens > 0.0)
            .collect::<Vec<_>>();

        if active_days.len() < MIN_ACTIVE_TIMESTAMPS_FOR_RHYTHM {
            return Self::default();
        }

        let weekend_count = active_days
            .iter()
            .filter(|day| is_weekend(day.day.weekday()))
            .count();
        let weekend_activity_weight = weekend_activity_weight(weekend_count, active_days.len());
        let active_timestamps = active_days
            .iter()
            .filter_map(|day| day.activity_timestamp)
            .collect::<Vec<_>>();
        let active_hours = learned_active_hours(&active_timestamps);

        Self {
            active_hours,
            weekend_activity_weight,
        }
    }

    pub fn from_active_hours(timestamps: &[OffsetDateTime]) -> Self {
        if timestamps.len() < MIN_ACTIVE_TIMESTAMPS_FOR_RHYTHM {
            return Self::default();
        }

        let mut weekend_count = 0_usize;
        for timestamp in timestamps {
            if is_weekend(timestamp.weekday()) {
                weekend_count += 1;
            }
        }

        Self {
            active_hours: learned_active_hours(timestamps),
            weekend_activity_weight: weekend_activity_weight(weekend_count, timestamps.len()),
        }
    }

    fn is_active_hour(&self, hour: u8) -> bool {
        self.active_hours[hour as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MetabolismResult {
    pub vitals: Vitals,
    pub mood: Mood,
}

pub fn apply_food(vitals: Vitals, effective_tokens: f64, baseline_daily: f64) -> MetabolismResult {
    let relative = (effective_tokens / baseline_daily.max(1.0)).max(0.0);
    let fed_gain = (relative * 45.0).min(30.0);
    let happiness_gain = (relative * 30.0).min(20.0);
    let energy_gain = (relative * 25.0).min(18.0);
    let vitals = Vitals {
        fed: clamp_vital(vitals.fed + fed_gain),
        happiness: clamp_vital(vitals.happiness + happiness_gain),
        energy: clamp_vital(vitals.energy + energy_gain),
    };

    MetabolismResult {
        vitals,
        mood: mood_for(vitals),
    }
}

pub fn apply_decay(
    vitals: Vitals,
    last_seen: OffsetDateTime,
    now: OffsetDateTime,
    rhythm: RhythmProfile,
) -> MetabolismResult {
    if now <= last_seen {
        return MetabolismResult {
            vitals,
            mood: mood_for(vitals),
        };
    }

    let weighted_hours = weighted_elapsed_hours(last_seen, now, &rhythm);
    let vitals = Vitals {
        fed: clamp_vital(vitals.fed - weighted_hours * 2.2),
        happiness: clamp_vital(vitals.happiness - weighted_hours * 1.25),
        energy: clamp_vital(vitals.energy - weighted_hours * 1.65),
    };

    MetabolismResult {
        vitals,
        mood: mood_for(vitals),
    }
}

fn weighted_elapsed_hours(
    last_seen: OffsetDateTime,
    now: OffsetDateTime,
    rhythm: &RhythmProfile,
) -> f64 {
    let total_seconds = (now - last_seen).whole_seconds().max(0) as f64;
    let whole_hours = (total_seconds / 3600.0).floor() as i64;
    let mut weighted = 0.0;
    let mut cursor = last_seen;

    for _ in 0..whole_hours {
        weighted += decay_weight(cursor, rhythm);
        cursor += time::Duration::hours(1);
    }

    let remaining_hours = (now - cursor).whole_seconds().max(0) as f64 / 3600.0;
    weighted + remaining_hours * decay_weight(cursor, rhythm)
}

fn decay_weight(timestamp: OffsetDateTime, rhythm: &RhythmProfile) -> f64 {
    let hour = timestamp.hour();
    if !(6..22).contains(&hour) {
        return OVERNIGHT_WEIGHT;
    }

    if is_weekend(timestamp.weekday()) {
        return rhythm.weekend_activity_weight;
    }

    if !rhythm.is_active_hour(hour) {
        return INACTIVE_HOUR_WEIGHT;
    }

    ACTIVE_HOUR_WEIGHT
}

pub fn mood_for_vitals(vitals: Vitals) -> Mood {
    mood_for(vitals)
}

fn mood_for(vitals: Vitals) -> Mood {
    if vitals.fed < 12.0 && vitals.happiness < 20.0 {
        Mood::Wilted
    } else if vitals.fed < 25.0 {
        Mood::Hungry
    } else if vitals.happiness < 35.0 {
        Mood::Sad
    } else if vitals.energy < 20.0 {
        Mood::Sleepy
    } else if vitals.fed >= 90.0 && vitals.happiness >= 90.0 && vitals.energy >= 70.0 {
        Mood::Ecstatic
    } else if vitals.fed >= 75.0 && vitals.happiness >= 75.0 && vitals.energy >= 55.0 {
        Mood::Happy
    } else {
        Mood::Content
    }
}

fn clamp_vital(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn is_weekend(weekday: Weekday) -> bool {
    matches!(weekday, Weekday::Saturday | Weekday::Sunday)
}

fn learned_active_hours(timestamps: &[OffsetDateTime]) -> [bool; 24] {
    if timestamps.len() < MIN_ACTIVE_TIMESTAMPS_FOR_RHYTHM {
        return RhythmProfile::default().active_hours;
    }

    let mut counts = [0_u16; 24];
    for timestamp in timestamps {
        counts[timestamp.hour() as usize] += 1;
    }

    let mut active_hours = [false; 24];
    for (hour, count) in counts.iter().enumerate() {
        active_hours[hour] = *count > 0;
    }
    active_hours
}

fn weekend_activity_weight(weekend_count: usize, active_count: usize) -> f64 {
    let weekend_share = weekend_count as f64 / active_count as f64;
    if weekend_share >= WEEKEND_HEAVY_SHARE {
        LEARNED_WEEKEND_WEIGHT
    } else {
        DEFAULT_WEEKEND_WEIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_vitals_are_ecstatic() {
        let peak = Vitals {
            fed: 95.0,
            happiness: 95.0,
            energy: 80.0,
        };
        assert_eq!(mood_for_vitals(peak), Mood::Ecstatic);
    }

    #[test]
    fn mood_threshold_keeps_near_peak_happy() {
        let happy = Vitals {
            fed: 80.0,
            happiness: 80.0,
            energy: 60.0,
        };
        assert_eq!(mood_for_vitals(happy), Mood::Happy);
    }

    #[test]
    fn ecstatic_round_trips_through_str() {
        assert_eq!("ecstatic".parse::<Mood>().unwrap(), Mood::Ecstatic);
        assert_eq!(Mood::Ecstatic.as_str(), "ecstatic");
    }
}
