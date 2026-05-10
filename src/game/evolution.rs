use serde::{Deserialize, Serialize};

use super::calibration::CalibrationBaseline;

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
    match stage_index_for_xp(xp) {
        0 => Stage::S0,
        1 => Stage::S1,
        2 => Stage::S2,
        3 => Stage::S3,
        4 => Stage::S4,
        5 => Stage::S5,
        _ => Stage::S6,
    }
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
            from: stage_from_index(stage_index - 1),
            to: stage_from_index(stage_index),
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

fn stage_from_index(index: usize) -> Stage {
    match index {
        0 => Stage::S0,
        1 => Stage::S1,
        2 => Stage::S2,
        3 => Stage::S3,
        4 => Stage::S4,
        5 => Stage::S5,
        _ => Stage::S6,
    }
}
