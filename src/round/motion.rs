const PET_WIDTH_CELLS: u16 = 13;
const PET_HEIGHT_CELLS: u16 = 10;
const PET_INK_WIDTH_CELLS: u16 = 11;
const PET_INK_HEIGHT_CELLS: u16 = 8;

use crate::round::locomotion::{
    sample_companion_locomotion, CompanionLocomotionSample, NormalizedLocomotionPoint,
};

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
    pub identity: u64,
    pub asleep: bool,
    pub sleep_onset_utc: Option<time::OffsetDateTime>,
    pub wake_from_eval_utc: Option<time::OffsetDateTime>,
    pub woke_at_utc: Option<time::OffsetDateTime>,
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
    /// Unclipped planar displacement before the non-Classic placement resolver.
    /// Classic and parallax keep their existing clamped fields; tank depth uses this
    /// value so the HUD reservation cannot erase local Y motion.
    pub planar_offset_cells: MotionPoint,
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
    let (fx, fy, fz, facing) = if motion.wander {
        let sample = lifecycle_locomotion_sample(input, wall_time);
        (
            sample.point.x,
            sample.point.y,
            sample.point.z,
            sample.facing,
        )
    } else {
        let (fx, fy, fz) = companion_drift_offsets(wall_time, motion.drift_period_secs);
        (fx, fy, fz, input.resolved_wander_facing)
    };
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
    _elapsed_ms: u64,
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
        planar_offset_cells: MotionPoint { x: offset_x, y: offset_y },
        classic_top_left_cells: [classic_x, classic_y],
        normalized_depth,
        facing: normalize_facing(facing),
        wander_offset_x,
        breath_offset_y_cells: input.breath_offset_y_cells,
        bob_offset_y_cells: 0.0,
    }
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

pub(crate) fn companion_drift_offsets(
    now: time::OffsetDateTime,
    period_secs: u64,
) -> (f32, f32, f32) {
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

fn lifecycle_locomotion_sample(
    input: CompanionMotionInput,
    wall_time: time::OffsetDateTime,
) -> CompanionLocomotionSample {
    let live = sample_companion_locomotion(input.identity, wall_time, input.current_facing);

    if input.asleep {
        return input.sleep_onset_utc.map_or(live, |onset| {
            let held = sample_companion_locomotion(input.identity, onset, input.current_facing);
            CompanionLocomotionSample {
                point: settle_toward_neutral(held.point, wall_time - onset),
                facing: held.facing,
                ..live
            }
        });
    }

    let (wake_from_eval_utc, woke_at_utc) = match (input.wake_from_eval_utc, input.woke_at_utc) {
        (None, None) => return live,
        (Some(from), Some(woke)) if from <= woke && woke <= wall_time => (from, woke),
        (Some(from), Some(woke)) => {
            debug_assert!(
                from <= woke && woke <= wall_time,
                "wake locomotion instants must be ordered and not in the future"
            );
            return live;
        }
        _ => {
            debug_assert!(
                false,
                "wake locomotion requires both wake_from_eval_utc and woke_at_utc"
            );
            return live;
        }
    };
    let wake_sample =
        sample_companion_locomotion(input.identity, wake_from_eval_utc, input.current_facing);
    let held_at_wake = settle_toward_neutral(wake_sample.point, woke_at_utc - wake_from_eval_utc);
    let point = blend_locomotion_points(
        held_at_wake,
        live.point,
        settle_fraction(wall_time - woke_at_utc),
    );
    CompanionLocomotionSample { point, ..live }
}

fn settle_toward_neutral(
    point: NormalizedLocomotionPoint,
    elapsed: time::Duration,
) -> NormalizedLocomotionPoint {
    blend_locomotion_points(
        point,
        NormalizedLocomotionPoint { x: 0.0, y: 0.0, z: 0.0 },
        settle_fraction(elapsed),
    )
}

fn blend_locomotion_points(
    from: NormalizedLocomotionPoint,
    to: NormalizedLocomotionPoint,
    fraction: f32,
) -> NormalizedLocomotionPoint {
    let fraction = minimum_jerk(fraction);
    NormalizedLocomotionPoint {
        x: from.x + (to.x - from.x) * fraction,
        y: from.y + (to.y - from.y) * fraction,
        z: from.z + (to.z - from.z) * fraction,
    }
}

fn settle_fraction(elapsed: time::Duration) -> f32 {
    let elapsed_ms = elapsed.whole_milliseconds().max(0) as f64;
    (elapsed_ms / (crate::pet::animator::WANDER_SETTLE_SECS as f64 * 1_000.0)) as f32
}

fn minimum_jerk(fraction: f32) -> f32 {
    let t = fraction.clamp(0.0, 1.0);
    t * t * t * (10.0 + t * (-15.0 + 6.0 * t))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::locomotion::{
        sample_companion_locomotion, stable_companion_identity, LocomotionPhase,
    };
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    fn motion_input(
        asleep: bool,
        sleep_onset_utc: Option<time::OffsetDateTime>,
        wake_from_eval_utc: Option<time::OffsetDateTime>,
        woke_at_utc: Option<time::OffsetDateTime>,
    ) -> CompanionMotionInput {
        CompanionMotionInput {
            identity: 0x5eed_5eed,
            asleep,
            sleep_onset_utc,
            wake_from_eval_utc,
            woke_at_utc,
            current_facing: 1,
            resolved_wander_offset_x: 0,
            resolved_wander_facing: 1,
            breath_offset_y_cells: 0,
        }
    }

    fn viewport() -> RoundCompanionMotionViewport {
        RoundCompanionMotionViewport {
            grid_columns: 44,
            grid_rows: 18,
            width_points: 440.0,
            height_points: 360.0,
            clearance: CompanionMotionClearance {
                near_scale: crate::round::depth::SMOOTH_PET_NEAR_SCALE,
                perspective_y_max: crate::round::depth::SMOOTH_PERSPECTIVE_Y_MAX,
                bottom_reserved_rows: 5,
            },
        }
    }

    fn production_viewport() -> RoundCompanionMotionViewport {
        RoundCompanionMotionViewport {
            grid_columns: 36,
            grid_rows: 18,
            width_points: 360.0,
            height_points: 360.0,
            clearance: crate::round::scene::current_round_motion_clearance(18),
        }
    }

    #[test]
    fn activity_and_calm_do_not_change_awake_locomotion_geometry() {
        let mut active = WatchViewModel::fixture_with_habitat_props();
        active.life_profile.calm_mode = false;
        active.progress.rate_per_hour = 80_000_000.0;
        let mut calm = active.clone();
        calm.life_profile.calm_mode = true;
        calm.progress.rate_per_hour = 0.0;
        let motion = companion_roam_motion();
        let now = datetime!(2026-07-17 12:34:56 UTC);
        let active = crate::round::scene::companion_pet_placement(&active, now, 44, 18, &motion);
        let calm = crate::round::scene::companion_pet_placement(&calm, now, 44, 18, &motion);

        assert_eq!(
            active.fractional_motion_top_left,
            calm.fractional_motion_top_left
        );
        assert_eq!(active.raw_depth, calm.raw_depth);
        assert_eq!(
            active.motion_projection.facing,
            calm.motion_projection.facing
        );
    }

    #[test]
    fn projection_uses_one_locomotion_sample_for_planar_and_depth_intent() {
        let now = datetime!(2026-07-17 12:34:56 UTC);
        let input = motion_input(false, None, None, None);
        let sample = sample_companion_locomotion(input.identity, now, input.current_facing);
        let projection =
            project_round_companion_motion(input, now, 0, viewport(), &companion_roam_motion());

        assert_eq!(projection.normalized_depth, sample.point.z);
        assert_eq!(projection.facing, sample.facing);
        assert_close(
            projection.planar_offset_cells.x,
            sample.point.x * (16.0 * 0.92),
        );
        assert_close(
            projection.planar_offset_cells.y,
            sample.point.y * (4.0 * 0.6),
        );
    }

    #[test]
    fn every_awake_route_leg_crosses_visible_production_aperture_distance() {
        let viewport = production_viewport();
        let motion = companion_roam_motion();
        let center_at = |identity, segment| {
            let now = time::OffsetDateTime::UNIX_EPOCH
                + time::Duration::seconds(
                    segment * crate::round::locomotion::LOCOMOTION_SEGMENT_SECS,
                );
            let mut input = motion_input(false, None, None, None);
            input.identity = identity;
            let projection = project_round_companion_motion(input, now, 0, viewport, &motion);
            let depth = crate::round::depth::resolve_smooth_depth(projection.normalized_depth, 1.0)
                .unwrap();
            crate::round::placement::resolve_round_depth_placement(projection, depth, viewport)
                .unwrap()
                .final_center_cells
        };

        for seed in 0..16 {
            let identity = stable_companion_identity(&format!("visible-production-route-{seed}"));
            for segment in -16..16 {
                let start = center_at(identity, segment);
                let end = center_at(identity, segment + 1);
                let distance = (end.x - start.x).hypot(end.y - start.y);
                assert!(
                    distance >= 2.0,
                    "seed {seed}, segment {segment} moved only {distance} cells: {start:?} to {end:?}"
                );
            }
        }
    }

    #[test]
    fn sleeping_motion_settles_to_neutral_then_holds() {
        let onset = datetime!(2026-07-17 12:34:56 UTC);
        let input = motion_input(true, Some(onset), None, None);
        let motion = companion_roam_motion();
        let at_onset = project_round_companion_motion(input, onset, 0, viewport(), &motion);
        let halfway = project_round_companion_motion(
            input,
            onset + time::Duration::seconds(4),
            0,
            viewport(),
            &motion,
        );
        let settled = project_round_companion_motion(
            input,
            onset + time::Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS),
            0,
            viewport(),
            &motion,
        );
        let held = project_round_companion_motion(
            input,
            onset + time::Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS + 60),
            0,
            viewport(),
            &motion,
        );

        assert_close(
            halfway.planar_offset_cells.x,
            at_onset.planar_offset_cells.x * 0.5,
        );
        assert_close(
            halfway.planar_offset_cells.y,
            at_onset.planar_offset_cells.y * 0.5,
        );
        assert_close(halfway.normalized_depth, at_onset.normalized_depth * 0.5);
        assert_eq!(settled.planar_offset_cells, MotionPoint { x: 0.0, y: 0.0 });
        assert_eq!(settled.normalized_depth, 0.0);
        assert_eq!(held.planar_offset_cells, settled.planar_offset_cells);
        assert_eq!(held.normalized_depth, settled.normalized_depth);
    }

    #[test]
    fn waking_motion_eases_from_neutral_to_the_live_path() {
        let wake_from_eval_utc = datetime!(2026-07-17 12:34:56 UTC);
        let woke_at_utc =
            wake_from_eval_utc + time::Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS);
        let waking = motion_input(false, None, Some(wake_from_eval_utc), Some(woke_at_utc));
        let live = motion_input(false, None, None, None);
        let motion = companion_roam_motion();
        let at_wake = project_round_companion_motion(waking, woke_at_utc, 0, viewport(), &motion);
        let halfway_at = woke_at_utc + time::Duration::seconds(4);
        let halfway = project_round_companion_motion(waking, halfway_at, 0, viewport(), &motion);
        let halfway_live = project_round_companion_motion(live, halfway_at, 0, viewport(), &motion);
        let settled_at =
            woke_at_utc + time::Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS);
        let settled = project_round_companion_motion(waking, settled_at, 0, viewport(), &motion);
        let settled_live = project_round_companion_motion(live, settled_at, 0, viewport(), &motion);

        assert_eq!(at_wake.planar_offset_cells, MotionPoint { x: 0.0, y: 0.0 });
        assert_eq!(at_wake.normalized_depth, 0.0);
        assert_close(
            halfway.planar_offset_cells.x,
            halfway_live.planar_offset_cells.x * 0.5,
        );
        assert_close(
            halfway.planar_offset_cells.y,
            halfway_live.planar_offset_cells.y * 0.5,
        );
        assert_close(
            halfway.normalized_depth,
            halfway_live.normalized_depth * 0.5,
        );
        assert_eq!(settled, settled_live);
    }

    #[test]
    fn facing_flips_during_dwell_and_never_mid_glide() {
        let identity = 0x5eed_5eed;
        let start = datetime!(2026-07-17 12:00:00 UTC);
        let mut previous = sample_companion_locomotion(identity, start, 1);
        let mut saw_flip = false;
        for second in 1..=(60 * 32) {
            let current =
                sample_companion_locomotion(identity, start + time::Duration::seconds(second), 1);
            if current.phase == LocomotionPhase::Glide && previous.phase == LocomotionPhase::Glide {
                assert_eq!(current.facing, previous.facing, "second={second}");
            }
            if current.facing != previous.facing {
                saw_flip = true;
                assert_eq!(current.phase, LocomotionPhase::Dwell, "second={second}");
                assert_eq!(previous.phase, LocomotionPhase::Glide, "second={second}");
            }
            previous = current;
        }
        assert!(
            saw_flip,
            "fixture must cover at least one route-facing change"
        );
    }

    #[test]
    fn companion_bob_is_zero_for_all_elapsed_times() {
        let motion = companion_roam_motion();
        let now = datetime!(2026-07-17 12:34:56 UTC);
        for elapsed_ms in [0, 250, 500, 1_000, 1_500, 2_000] {
            let projection = project_round_companion_motion(
                motion_input(false, None, None, None),
                now,
                elapsed_ms,
                viewport(),
                &motion,
            );
            assert_eq!(
                projection.bob_offset_y_cells, 0.0,
                "elapsed_ms={elapsed_ms}"
            );
        }
    }

    #[test]
    fn depth_override_changes_only_depth_for_review_fixtures() {
        let now = datetime!(2026-07-17 12:34:56 UTC);
        let input = motion_input(false, None, None, None);
        let live = project_round_companion_motion_with_options(
            input,
            now,
            500,
            viewport(),
            &companion_roam_motion(),
            RoundMotionProjectionOptions::default(),
        );
        let overridden = project_round_companion_motion_with_options(
            input,
            now,
            500,
            viewport(),
            &companion_roam_motion(),
            RoundMotionProjectionOptions { depth_override: Some(0.75) },
        );

        assert_eq!(overridden.motion_top_left_cells, live.motion_top_left_cells);
        assert_eq!(
            overridden.motion_origin_top_left_cells,
            live.motion_origin_top_left_cells
        );
        assert_eq!(
            overridden.motion_top_left_points,
            live.motion_top_left_points
        );
        assert_eq!(overridden.planar_offset_cells, live.planar_offset_cells);
        assert_eq!(
            overridden.classic_top_left_cells,
            live.classic_top_left_cells
        );
        assert_eq!(overridden.facing, live.facing);
        assert_eq!(overridden.wander_offset_x, live.wander_offset_x);
        assert_eq!(overridden.breath_offset_y_cells, live.breath_offset_y_cells);
        assert_eq!(overridden.bob_offset_y_cells, live.bob_offset_y_cells);
        assert_eq!(overridden.normalized_depth, 0.75);
    }

    #[test]
    #[should_panic(expected = "wake locomotion requires both")]
    fn incomplete_wake_data_asserts_in_tests() {
        let now = datetime!(2026-07-17 12:34:56 UTC);
        let input = motion_input(false, None, Some(now - time::Duration::seconds(1)), None);
        let _ = project_round_companion_motion(input, now, 0, viewport(), &companion_roam_motion());
    }

    #[test]
    #[should_panic(expected = "wake locomotion instants must be ordered")]
    fn inverted_wake_data_asserts_in_tests() {
        let now = datetime!(2026-07-17 12:34:56 UTC);
        let input = motion_input(
            false,
            None,
            Some(now),
            Some(now - time::Duration::seconds(1)),
        );
        let _ = project_round_companion_motion(input, now, 0, viewport(), &companion_roam_motion());
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1.0e-5,
            "expected {expected}, got {actual}"
        );
    }
}
