use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::calibration::CalibrationBaseline;
use crate::error::GlorpError;

pub const ACTIVE_HOURS_PER_DAY: f64 = 8.0;
pub const STAGE_THRESHOLDS: [f64; 7] = [
    0.0,
    1.0 / ACTIVE_HOURS_PER_DAY,
    6.0 / ACTIVE_HOURS_PER_DAY,
    1.0,
    4.0,
    14.0,
    60.0,
];

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
    let direct = relative.min(1.0);
    let excess = (relative - 1.0).max(0.0);
    direct + excess.sqrt() * 0.05
}

pub fn stage_start_xp(stage: Stage) -> f64 {
    STAGE_THRESHOLDS[stage.index()]
}

pub fn next_stage_xp_target(stage: Stage) -> f64 {
    let next = (stage.index() + 1).min(Stage::S6.index());
    STAGE_THRESHOLDS[next]
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // These properties pin the contract of `apply_xp_delta`:
    //
    // 1. `xp_never_decreases_for_non_negative_delta` — monotonicity. A delta of
    //    zero or more must never reduce the pet's XP. The function explicitly
    //    uses `.max(current_xp)` as a safety floor; this property is the
    //    regression guard for that floor.
    //
    // 2. `stage_transitions_are_strictly_increasing` — when the function emits
    //    multiple transitions (because a single XP delta crossed several stage
    //    thresholds at once) they must be consecutive and strictly ascending
    //    in stage index, with each `to.index() == from.index() + 1`. The runtime
    //    feeds these to the animation queue assuming this ordering.
    //
    // 3. `final_stage_matches_start_plus_transitions` — the number of emitted
    //    transitions equals `end_stage_index - start_stage_index`, and the
    //    final stage reached (computed from `result.xp`) equals the `to` of
    //    the last transition (or the start stage when no transitions fired).
    //    Together these pin the relationship between XP-derived stage and the
    //    transition stream consumers see.
    proptest! {
        #[test]
        fn xp_never_decreases_for_non_negative_delta(
            current_xp in 0.0..100.0f64,
            delta in 0.0..1e10f64,
            daily in 1.0..1e10f64,
        ) {
            let baseline = CalibrationBaseline { daily_effective_tokens: daily };
            let result = apply_xp_delta(current_xp, delta, baseline);
            prop_assert!(result.xp >= current_xp,
                "xp {} regressed to {} for delta {}", current_xp, result.xp, delta);
        }

        #[test]
        fn stage_transitions_are_strictly_increasing(
            current_xp in 0.0..100.0f64,
            delta in 0.0..1e10f64,
            daily in 1.0..1e10f64,
        ) {
            let baseline = CalibrationBaseline { daily_effective_tokens: daily };
            let result = apply_xp_delta(current_xp, delta, baseline);
            for window in result.stage_transitions.windows(2) {
                let a = window[0];
                let b = window[1];
                prop_assert_eq!(b.from.index(), a.to.index(),
                    "transitions not consecutive: {:?} -> {:?}", a, b);
                prop_assert!(b.to.index() > a.to.index(),
                    "transition stage index not increasing");
            }
            for t in &result.stage_transitions {
                prop_assert_eq!(t.to.index(), t.from.index() + 1,
                    "single transition skipped stages: {:?}", t);
            }
        }

        #[test]
        fn final_stage_matches_start_plus_transitions(
            current_xp in 0.0..100.0f64,
            delta in 0.0..1e10f64,
            daily in 1.0..1e10f64,
        ) {
            let baseline = CalibrationBaseline { daily_effective_tokens: daily };
            let start_stage = stage_for_xp(current_xp);
            let result = apply_xp_delta(current_xp, delta, baseline);
            let end_stage = stage_for_xp(result.xp);

            let expected_count = end_stage.index() - start_stage.index();
            prop_assert_eq!(result.stage_transitions.len(), expected_count,
                "expected {} transitions, got {}",
                expected_count, result.stage_transitions.len());

            if let Some(last) = result.stage_transitions.last() {
                prop_assert_eq!(last.to.index(), end_stage.index(),
                    "last transition's `to` does not match final stage");
                prop_assert_eq!(result.stage_transitions[0].from.index(), start_stage.index(),
                    "first transition's `from` does not match start stage");
            } else {
                prop_assert_eq!(end_stage.index(), start_stage.index(),
                    "no transitions but stage changed");
            }
        }
    }
}
