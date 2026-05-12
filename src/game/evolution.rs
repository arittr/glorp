use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::calibration::CalibrationBaseline;
use crate::error::GlorpError;

const STAGE_THRESHOLDS: [f64; 7] = [0.0, 0.04, 0.25, 1.0, 4.0, 14.0, 60.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    S0,
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
}

impl Stage {
    /// Stable ordinal: `S0` -> 0, `S6` -> 6. Used by anything that needs a
    /// stage-indexed array (stage labels per species, threshold lookups).
    pub const fn index(self) -> usize {
        match self {
            Stage::S0 => 0,
            Stage::S1 => 1,
            Stage::S2 => 2,
            Stage::S3 => 3,
            Stage::S4 => 4,
            Stage::S5 => 5,
            Stage::S6 => 6,
        }
    }

    /// Inverse of `index`: returns `None` for any out-of-range value so
    /// callers must handle the "unknown stage" case explicitly.
    pub const fn from_index(index: usize) -> Option<Stage> {
        match index {
            0 => Some(Stage::S0),
            1 => Some(Stage::S1),
            2 => Some(Stage::S2),
            3 => Some(Stage::S3),
            4 => Some(Stage::S4),
            5 => Some(Stage::S5),
            6 => Some(Stage::S6),
            _ => None,
        }
    }

    /// Matches the serde wire format (`#[serde(rename_all = "lowercase")]`):
    /// `"s0"`..`"s6"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Stage::S0 => "s0",
            Stage::S1 => "s1",
            Stage::S2 => "s2",
            Stage::S3 => "s3",
            Stage::S4 => "s4",
            Stage::S5 => "s5",
            Stage::S6 => "s6",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Stage {
    type Err = GlorpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "s0" => Ok(Stage::S0),
            "s1" => Ok(Stage::S1),
            "s2" => Ok(Stage::S2),
            "s3" => Ok(Stage::S3),
            "s4" => Ok(Stage::S4),
            "s5" => Ok(Stage::S5),
            "s6" => Ok(Stage::S6),
            other => Err(GlorpError::Message(format!("unknown stage {other:?}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageTransition {
    pub from: Stage,
    pub to: Stage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XpDeltaResult {
    pub xp: f64,
    pub stage_transitions: Vec<StageTransition>,
    pub mood_food_benefit: f64,
}

pub fn stage_for_xp(xp: f64) -> Stage {
    Stage::from_index(stage_index_for_xp(xp)).unwrap_or(Stage::S6)
}

pub fn apply_xp_delta(
    current_xp: f64,
    delta_effective: f64,
    baseline: CalibrationBaseline,
) -> XpDeltaResult {
    let xp_gain = calibrated_xp_units(delta_effective, baseline);
    let xp = (current_xp + xp_gain).max(current_xp);
    let before = stage_index_for_xp(current_xp);
    let after = stage_index_for_xp(xp);
    let mut stage_transitions = Vec::new();

    for stage_index in (before + 1)..=after {
        stage_transitions.push(StageTransition {
            from: Stage::from_index(stage_index - 1).unwrap_or(Stage::S6),
            to: Stage::from_index(stage_index).unwrap_or(Stage::S6),
        });
    }

    XpDeltaResult {
        xp,
        stage_transitions,
        mood_food_benefit: mood_food_benefit(delta_effective, baseline),
    }
}

pub fn calibrated_xp_units(delta_effective: f64, baseline: CalibrationBaseline) -> f64 {
    let daily = baseline.daily_effective_tokens.max(1.0);
    let relative = (delta_effective / daily).max(0.0);
    let direct = relative.min(0.25);
    let excess = (relative - 0.25).max(0.0);
    direct + excess.sqrt() * 0.05
}

fn mood_food_benefit(delta_effective: f64, baseline: CalibrationBaseline) -> f64 {
    let daily = baseline.daily_effective_tokens.max(1.0);
    let relative = (delta_effective / daily).max(0.0);
    (relative * 35.0).min(25.0)
}

fn stage_index_for_xp(xp: f64) -> usize {
    let xp = xp.max(0.0);
    STAGE_THRESHOLDS
        .iter()
        .rposition(|threshold| xp >= *threshold)
        .unwrap_or(0)
}
