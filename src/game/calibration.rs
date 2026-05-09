use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

const DEFAULT_DAILY_EFFECTIVE_TOKENS: f64 = 100_000.0;
const MIN_ACTIVE_DAYS_FOR_MEDIAN: usize = 5;
const RECENT_ACTIVE_DAY_LIMIT: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CalibrationBaseline {
    pub daily_effective_tokens: f64,
}

impl Default for CalibrationBaseline {
    fn default() -> Self {
        Self {
            daily_effective_tokens: DEFAULT_DAILY_EFFECTIVE_TOKENS,
        }
    }
}

impl CalibrationBaseline {
    pub fn from_history(history: &[DailyUsage]) -> Self {
        let mut active_days = history
            .iter()
            .copied()
            .filter(|day| day.effective_tokens > 0.0)
            .collect::<Vec<_>>();

        if active_days.len() < MIN_ACTIVE_DAYS_FOR_MEDIAN {
            return Self::default();
        }

        active_days.sort_by_key(|day| day.day);
        let recent_start = active_days.len().saturating_sub(RECENT_ACTIVE_DAY_LIMIT);
        let mut recent_values = active_days[recent_start..]
            .iter()
            .map(|day| day.effective_tokens)
            .collect::<Vec<_>>();
        recent_values.sort_by(f64::total_cmp);

        Self {
            daily_effective_tokens: median(&recent_values).max(1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DailyUsage {
    pub day: Date,
    pub effective_tokens: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_timestamp: Option<OffsetDateTime>,
}

impl DailyUsage {
    pub fn new(day: Date, effective_tokens: f64) -> Self {
        Self {
            day,
            effective_tokens,
            activity_timestamp: None,
        }
    }

    pub fn with_activity_timestamp(
        activity_timestamp: OffsetDateTime,
        effective_tokens: f64,
    ) -> Self {
        Self {
            day: activity_timestamp.date(),
            effective_tokens,
            activity_timestamp: Some(activity_timestamp),
        }
    }
}

fn median(values: &[f64]) -> f64 {
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}
