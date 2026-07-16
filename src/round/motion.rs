const PET_WIDTH_CELLS: u16 = 13;
const PET_HEIGHT_CELLS: u16 = 10;
const PET_INK_WIDTH_CELLS: u16 = 11;
const PET_INK_HEIGHT_CELLS: u16 = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionMotion {
    pub wander_half: u16,
    pub drift_x_frac: f32,
    pub drift_y_frac: f32,
    pub drift_period_secs: u64,
    pub upward_bias: f32,
    pub wander: bool,
}

impl Default for CompanionMotion {
    fn default() -> Self {
        Self {
            wander_half: 8,
            drift_x_frac: 0.45,
            drift_y_frac: 0.30,
            drift_period_secs: 20,
            upward_bias: 0.0,
            wander: false,
        }
    }
}

pub fn companion_roam_motion() -> CompanionMotion {
    CompanionMotion {
        wander_half: 8,
        drift_x_frac: 0.92,
        drift_y_frac: 0.6,
        drift_period_secs: 22,
        upward_bias: 0.5,
        wander: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundCompanionMotionViewport {
    pub grid_columns: u16,
    pub grid_rows: u16,
    pub width_points: f32,
    pub height_points: f32,
    pub clearance: CompanionMotionClearance,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionMotionClearance {
    pub near_scale: f32,
    pub perspective_y_max: f32,
    pub bottom_reserved_rows: u16,
}

#[derive(Clone, Copy, PartialEq)]
pub struct CompanionMotionInput {
    pub asleep: bool,
    pub calm: bool,
    pub rate_per_hour: f64,
    pub current_facing: i8,
    pub resolved_wander_offset_x: i16,
    pub resolved_wander_facing: i8,
    pub breath_offset_y_cells: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundCompanionMotionProjection {
    pub motion_top_left_cells: MotionPoint,
    pub motion_origin_top_left_cells: MotionPoint,
    pub motion_top_left_points: [f32; 2],
    pub classic_top_left_cells: [u16; 2],
    pub normalized_depth: f32,
    pub facing: i8,
    pub wander_offset_x: i16,
    pub breath_offset_y_cells: u8,
    pub bob_offset_y_cells: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RoundMotionProjectionOptions {
    pub depth_override: Option<f32>,
}

pub fn project_round_companion_motion(
    input: CompanionMotionInput,
    wall_time: time::OffsetDateTime,
    elapsed_ms: u64,
    viewport: RoundCompanionMotionViewport,
    motion: &CompanionMotion,
) -> RoundCompanionMotionProjection {
    project_round_companion_motion_with_options(
        input,
        wall_time,
        elapsed_ms,
        viewport,
        motion,
        RoundMotionProjectionOptions::default(),
    )
}

pub fn project_round_companion_motion_with_options(
    input: CompanionMotionInput,
    wall_time: time::OffsetDateTime,
    elapsed_ms: u64,
    viewport: RoundCompanionMotionViewport,
    motion: &CompanionMotion,
    options: RoundMotionProjectionOptions,
) -> RoundCompanionMotionProjection {
    let energy = companion_motion_energy(input);
    let facing = if motion.wander {
        companion_wander_facing(
            wall_time,
            motion.drift_period_secs,
            energy,
            input.current_facing,
        )
    } else {
        input.resolved_wander_facing
    };
    let (fx, fy, fz) = companion_motion_offsets(wall_time, motion, energy);
    project_round_companion_motion_from_offsets(
        input,
        elapsed_ms,
        viewport,
        motion,
        fx,
        fy,
        normalized_depth(options.depth_override.unwrap_or(fz)),
        facing,
        input.resolved_wander_offset_x,
    )
}

/// Projects the authored neutral pose through the same envelope, bias, scale,
/// and clamping rules as moving presentation. Used by Reduce Motion so the
/// accessibility path cannot drift from production geometry.
pub(crate) fn project_round_companion_motion_neutral(
    input: CompanionMotionInput,
    viewport: RoundCompanionMotionViewport,
    motion: &CompanionMotion,
    depth_override: Option<f32>,
) -> RoundCompanionMotionProjection {
    let mut projection = project_round_companion_motion_from_offsets(
        input,
        0,
        viewport,
        motion,
        0.0,
        0.0,
        normalized_depth(depth_override.unwrap_or(0.0)),
        input.resolved_wander_facing,
        input.resolved_wander_offset_x,
    );
    projection.bob_offset_y_cells = 0.0;
    projection
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn project_round_companion_motion_from_offsets(
    input: CompanionMotionInput,
    elapsed_ms: u64,
    viewport: RoundCompanionMotionViewport,
    motion: &CompanionMotion,
    fx: f32,
    fy: f32,
    normalized_depth: f32,
    facing: i8,
    wander_offset_x: i16,
) -> RoundCompanionMotionProjection {
    let grid_columns = viewport.grid_columns;
    let grid_rows = viewport.grid_rows;
    let center_x = grid_columns / 2;
    let center_y = grid_rows / 2;
    let half_width = PET_WIDTH_CELLS / 2;
    let half_height = PET_HEIGHT_CELLS / 2;
    let safe_x = center_x.saturating_sub(half_width) as f32;
    let safe_y = center_y.saturating_sub(half_height) as f32;
    let x_radius = safe_x * motion.drift_x_frac;
    let y_radius = safe_y * motion.drift_y_frac;
    let bias = motion.upward_bias * safe_y;
    let max_x = grid_columns.saturating_sub(PET_WIDTH_CELLS);
    let max_y = grid_rows.saturating_sub(PET_HEIGHT_CELLS);
    let base_x = center_x as i32 - half_width as i32;
    let base_y = center_y as i32 - half_height as i32;
    let offset_x = fx * x_radius;
    let offset_y = fy * y_radius;

    let classic_x = (base_x + offset_x as i32).clamp(0, max_x as i32) as u16;
    let classic_drift_y = (base_y - bias as i32 + offset_y as i32).clamp(0, max_y as i32) as u16;
    let classic_y = (classic_drift_y + u16::from(input.breath_offset_y_cells)).min(max_y);

    let envelope = companion_roam_envelope(viewport);
    let motion_origin = MotionPoint {
        x: clamp_within(base_x as f32, envelope.min_x, envelope.max_x),
        y: clamp_within(base_y as f32 - bias, envelope.min_y, envelope.max_y),
    };
    let motion_top_left = MotionPoint {
        x: clamp_within(motion_origin.x + offset_x, envelope.min_x, envelope.max_x),
        y: clamp_within(motion_origin.y + offset_y, envelope.min_y, envelope.max_y),
    };
    let point_scale_x = if grid_columns == 0 {
        0.0
    } else {
        viewport.width_points / f32::from(grid_columns)
    };
    let point_scale_y = if grid_rows == 0 {
        0.0
    } else {
        viewport.height_points / f32::from(grid_rows)
    };

    RoundCompanionMotionProjection {
        motion_top_left_cells: motion_top_left,
        motion_origin_top_left_cells: motion_origin,
        motion_top_left_points: [
            motion_top_left.x * point_scale_x,
            motion_top_left.y * point_scale_y,
        ],
        classic_top_left_cells: [classic_x, classic_y],
        normalized_depth,
        facing: normalize_facing(facing),
        wander_offset_x,
        breath_offset_y_cells: input.breath_offset_y_cells,
        bob_offset_y_cells: round_companion_bob(elapsed_ms),
    }
}

pub fn round_companion_bob(elapsed_ms: u64) -> f32 {
    const AMPLITUDE: f32 = 0.33;
    const PERIOD_MS: f32 = 2_000.0;
    let phase = (elapsed_ms as f32 / PERIOD_MS) * std::f32::consts::TAU;
    phase.sin() * AMPLITUDE
}

pub(crate) fn companion_drift_position(
    motion: &CompanionMotion,
    grid_columns: u16,
    grid_rows: u16,
    fx: f32,
    fy: f32,
) -> (u16, u16) {
    let center_x = grid_columns / 2;
    let center_y = grid_rows / 2;
    let half_width = PET_WIDTH_CELLS / 2;
    let half_height = PET_HEIGHT_CELLS / 2;
    let safe_x = center_x.saturating_sub(half_width) as f32;
    let safe_y = center_y.saturating_sub(half_height) as f32;
    let x_radius = safe_x * motion.drift_x_frac;
    let y_radius = safe_y * motion.drift_y_frac;
    let bias = motion.upward_bias * safe_y;
    let art_x = center_x as i32 - half_width as i32 + (fx * x_radius) as i32;
    let art_y = center_y as i32 - half_height as i32 - bias as i32 + (fy * y_radius) as i32;
    (
        art_x.clamp(0, grid_columns.saturating_sub(PET_WIDTH_CELLS) as i32) as u16,
        art_y.clamp(0, grid_rows.saturating_sub(PET_HEIGHT_CELLS) as i32) as u16,
    )
}

pub(crate) fn companion_motion_offsets(
    now: time::OffsetDateTime,
    motion: &CompanionMotion,
    energy: f32,
) -> (f32, f32, f32) {
    if motion.wander {
        let (x, y) = companion_wander_offsets(now, motion.drift_period_secs);
        let z = companion_wander_depth(now, motion.drift_period_secs);
        // Activity controls lateral liveliness; lifecycle projection attenuates
        // depth separately so an awake pet can traverse the full shallow tank.
        (x * energy, y * energy, z)
    } else {
        companion_drift_offsets(now, motion.drift_period_secs)
    }
}

pub(crate) fn companion_wander_offsets(now: time::OffsetDateTime, period_secs: u64) -> (f32, f32) {
    use std::f64::consts::TAU;
    let t = (now.unix_timestamp() as f64 + now.nanosecond() as f64 / 1_000_000_000.0)
        / period_secs.max(1) as f64;
    let x = 0.72 * (TAU * t).cos() + 0.28 * (TAU * t * 1.93 + 0.6).sin();
    let y = 0.72 * (TAU * t * 1.21 + 0.3).sin() + 0.28 * (TAU * t * 2.41 + 1.5).cos();
    (x as f32, y as f32)
}

pub(crate) fn companion_wander_facing(
    now: time::OffsetDateTime,
    period_secs: u64,
    energy: f32,
    current: i8,
) -> i8 {
    const WINDOW_SECS: i64 = 1;
    const DEADZONE: f32 = 0.04;
    let (x_now, _) = companion_wander_offsets(now, period_secs);
    let (x_previous, _) =
        companion_wander_offsets(now - time::Duration::seconds(WINDOW_SECS), period_secs);
    let visible_dx = (x_now - x_previous) * energy;
    if visible_dx > DEADZONE {
        -1
    } else if visible_dx < -DEADZONE {
        1
    } else {
        normalize_facing(current)
    }
}

pub(crate) fn companion_motion_energy(input: CompanionMotionInput) -> f32 {
    const IDLE_FLOOR: f32 = 0.25;
    const RESTING_ENERGY: f32 = 0.12;
    const RATE_FULL: f64 = 50_000_000.0;
    if input.asleep || input.calm {
        return RESTING_ENERGY;
    }
    (IDLE_FLOOR + (input.rate_per_hour.max(0.0) / RATE_FULL) as f32).clamp(IDLE_FLOOR, 1.0)
}

fn companion_drift_offsets(now: time::OffsetDateTime, period_secs: u64) -> (f32, f32, f32) {
    let unix = now.unix_timestamp() as u64;
    let period = period_secs.max(1);
    let epoch = unix / period;
    let phase = (unix % period) as f32 / period as f32;
    let target = |value: u64| {
        let x = value
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(0x6c62_272e_07bb_0142);
        let y = x
            .wrapping_mul(0x517c_c1b7_2722_0a95)
            .wrapping_add(0xbf87_8c2f_a7a4_c6a5);
        let z = y
            .wrapping_mul(0x2545_f491_4f6c_dd1d)
            .wrapping_add(0x1405_7b7e_f767_814f);
        (
            ((x >> 32) as i32 as f32) / i32::MAX as f32,
            ((y >> 32) as i32 as f32) / i32::MAX as f32,
            ((z >> 32) as i32 as f32) / i32::MAX as f32,
        )
    };
    let previous = target(epoch.saturating_sub(1));
    let next = target(epoch);
    let t = phase * phase * (3.0 - 2.0 * phase);
    (
        previous.0 + (next.0 - previous.0) * t,
        previous.1 + (next.1 - previous.1) * t,
        previous.2 + (next.2 - previous.2) * t,
    )
}

fn companion_wander_depth(now: time::OffsetDateTime, period_secs: u64) -> f32 {
    use std::f64::consts::TAU;
    let t = (now.unix_timestamp() as f64 + now.nanosecond() as f64 / 1_000_000_000.0)
        / period_secs.max(1) as f64;
    (0.70 * (TAU * t * 1.37 + 0.9).sin() + 0.30 * (TAU * t * 0.61 + 2.0).cos()) as f32
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CompanionRoamEnvelope {
    pub(crate) min_x: f32,
    pub(crate) max_x: f32,
    pub(crate) min_y: f32,
    pub(crate) max_y: f32,
}

pub(crate) fn companion_roam_envelope(
    viewport: RoundCompanionMotionViewport,
) -> CompanionRoamEnvelope {
    let grid_columns = viewport.grid_columns;
    let grid_rows = viewport.grid_rows;
    let frame_half_width = f32::from(PET_WIDTH_CELLS) / 2.0;
    let frame_half_height = f32::from(PET_HEIGHT_CELLS) / 2.0;
    let scaled_ink_half_width =
        f32::from(PET_INK_WIDTH_CELLS) / 2.0 * viewport.clearance.near_scale;
    let scaled_ink_half_height =
        f32::from(PET_INK_HEIGHT_CELLS) / 2.0 * viewport.clearance.near_scale;
    let protected_bottom =
        f32::from(grid_rows.saturating_sub(viewport.clearance.bottom_reserved_rows));
    CompanionRoamEnvelope {
        min_x: scaled_ink_half_width - frame_half_width,
        max_x: f32::from(grid_columns) - scaled_ink_half_width - frame_half_width,
        min_y: scaled_ink_half_height - frame_half_height + viewport.clearance.perspective_y_max,
        max_y: protected_bottom
            - scaled_ink_half_height
            - frame_half_height
            - viewport.clearance.perspective_y_max,
    }
}

fn clamp_within(value: f32, min: f32, max: f32) -> f32 {
    if min > max {
        (min + max) * 0.5
    } else {
        value.clamp(min, max)
    }
}

fn normalized_depth(raw: f32) -> f32 {
    if raw.is_finite() {
        raw.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn normalize_facing(facing: i8) -> i8 {
    if facing < 0 {
        -1
    } else {
        1
    }
}
