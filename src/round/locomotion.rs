//! Deterministic locomotion shared by live companion motion and Preview Lab.

pub(crate) const LOCOMOTION_SEGMENT_SECS: i64 = 5;
pub(crate) const LOCOMOTION_BLOCK_SEGMENTS: usize = 8;
pub(crate) const LOCOMOTION_DWELL_MIN_SECS: i64 = 0;
pub(crate) const LOCOMOTION_DWELL_MAX_SECS: i64 = 0;
pub(crate) const LOCOMOTION_MAX_PLANAR_STEP: f32 = 0.85;
pub(crate) const LOCOMOTION_MAX_DEPTH_STEP: f32 = 0.67;
const LOCOMOTION_CONTROL_OFFSET_FRACTION: f32 = 0.12;
const SWIM_RAMP_FRACTION: f32 = 0.14;
const LOCOMOTION_FACING_DEADZONE: f32 = 0.06;
const LOCOMOTION_DEPTH_EXTREME: f32 = 0.70;
const LOCOMOTION_DEPTH_MID: f32 = 0.23;
const LOCOMOTION_EXTREME_X_LIMIT: f32 = 0.45;
const LOCOMOTION_MID_X_LIMIT: f32 = 0.62;
const LOCOMOTION_NEUTRAL_X_LIMIT: f32 = 0.78;
const MIN_HORIZONTAL_TARGET_DISTANCE: f32 = 0.20;
const ROUTE_DIRECTIONS: usize = 16;
const ROUTE_CANDIDATE_RADII: [f32; 4] = [0.36, 0.52, 0.68, 0.82];
const ROUTE_CANDIDATE_COUNT: usize = ROUTE_DIRECTIONS * ROUTE_CANDIDATE_RADII.len();

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const MIN_PLANAR_TARGET_DISTANCE: f32 = 0.35;
const ROUTE_POINTS: usize = LOCOMOTION_BLOCK_SEGMENTS + 1;
const NANOS_PER_SECOND: i64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NormalizedLocomotionPoint {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocomotionPhase {
    Dwell,
    Glide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CompanionLocomotionSample {
    pub(crate) point: NormalizedLocomotionPoint,
    pub(crate) facing: i8,
    pub(crate) phase: LocomotionPhase,
    pub(crate) segment_index: i64,
    pub(crate) segment_phase: f32,
}

/// Produces a stable identity without relying on Rust's process-randomized hashers.
pub(crate) fn stable_companion_identity(seed: &str) -> u64 {
    fnv1a(FNV_OFFSET_BASIS, seed.as_bytes())
}

/// Samples the complete route from immutable identity and wall time. There is
/// deliberately no route cache: day changes cannot leave stale route state
/// behind, and a restarted process lands on the same route immediately.
pub(crate) fn sample_companion_locomotion(
    identity: u64,
    now: time::OffsetDateTime,
    fallback_facing: i8,
) -> CompanionLocomotionSample {
    let seconds = now.unix_timestamp();
    let nanos = i64::from(now.nanosecond());
    let segment_index = seconds.div_euclid(LOCOMOTION_SEGMENT_SECS);
    let seconds_into_segment = seconds.rem_euclid(LOCOMOTION_SEGMENT_SECS);
    let elapsed_nanos = seconds_into_segment * NANOS_PER_SECOND + nanos;
    let segment_nanos = LOCOMOTION_SEGMENT_SECS * NANOS_PER_SECOND;
    let segment_phase = elapsed_nanos as f64 / segment_nanos as f64;
    debug_assert!(elapsed_nanos <= segment_nanos);
    sample_companion_locomotion_at_segment_phase(
        identity,
        segment_index,
        segment_phase as f32,
        fallback_facing,
    )
}

/// Samples one deterministic leg at an explicit elapsed phase. The round
/// renderer uses this to redistribute a swim over the visible, clipped path
/// without changing its route, waypoints, or facing intent.
pub(crate) fn sample_companion_locomotion_at_segment_phase(
    identity: u64,
    segment_index: i64,
    segment_phase: f32,
    fallback_facing: i8,
) -> CompanionLocomotionSample {
    let segment_phase = segment_phase.clamp(0.0, 1.0);
    let dwell = dwell_seconds(identity, segment_index);
    let dwell_phase = dwell as f32 / LOCOMOTION_SEGMENT_SECS as f32;
    let start = route_target(identity, segment_index);
    let end = route_target(identity, segment_index + 1);
    let facing = facing_for_segment(identity, segment_index, fallback_facing);

    if segment_phase < dwell_phase {
        return CompanionLocomotionSample {
            point: start,
            facing,
            phase: LocomotionPhase::Dwell,
            segment_index,
            segment_phase,
        };
    }

    let glide_phase = (segment_phase - dwell_phase) / (1.0 - dwell_phase);
    CompanionLocomotionSample {
        point: glide_point(
            identity,
            segment_index,
            start,
            end,
            swim_progress(glide_phase),
        ),
        facing,
        phase: LocomotionPhase::Glide,
        segment_index,
        segment_phase,
    }
}

fn route_target(identity: u64, segment_index: i64) -> NormalizedLocomotionPoint {
    let block_index = segment_index.div_euclid(LOCOMOTION_BLOCK_SEGMENTS as i64);
    let slot = segment_index.rem_euclid(LOCOMOTION_BLOCK_SEGMENTS as i64) as usize;
    route_block(identity, block_index)[slot]
}

fn route_block(identity: u64, block_index: i64) -> [NormalizedLocomotionPoint; ROUTE_POINTS] {
    let start_anchor = boundary_anchor(identity, block_index);
    let end_anchor = boundary_anchor(identity, block_index + 1);
    let mut route = [start_anchor; ROUTE_POINTS];
    let z_waypoints = depth_waypoints(block_index);
    route[0].z = z_waypoints[0];
    route[LOCOMOTION_BLOCK_SEGMENTS] = NormalizedLocomotionPoint {
        x: end_anchor.x,
        y: end_anchor.y,
        z: z_waypoints[LOCOMOTION_BLOCK_SEGMENTS],
    };
    // The tail depends only on this block's anchors, allowing the next block
    // to inspect the actual incoming step without recursive generation.
    route[LOCOMOTION_BLOCK_SEGMENTS - 1] = final_route_target(
        identity,
        block_index,
        start_anchor,
        end_anchor,
        z_waypoints[LOCOMOTION_BLOCK_SEGMENTS - 1],
    );

    // Work backward from the independently derived tail so each candidate can
    // reject a reversal against its already-known successor.
    for slot in (1..LOCOMOTION_BLOCK_SEGMENTS - 1).rev() {
        let t = slot as f32 / LOCOMOTION_BLOCK_SEGMENTS as f32;
        let interpolation = clamp_xy(NormalizedLocomotionPoint {
            x: start_anchor.x + (end_anchor.x - start_anchor.x) * t,
            y: start_anchor.y + (end_anchor.y - start_anchor.y) * t,
            z: z_waypoints[slot],
        });
        let rotation = (hash_lane(identity, block_index, 0x100 + slot as u64)
            % ROUTE_CANDIDATE_COUNT as u64) as usize;
        let mut selected = None;

        for attempt in 0..ROUTE_CANDIDATE_COUNT {
            let candidate_index = (rotation + attempt) % ROUTE_CANDIDATE_COUNT;
            let candidate = candidate_point(interpolation, candidate_index, z_waypoints[slot]);
            if !is_visible_planar_step(candidate, route[slot + 1]) {
                continue;
            }
            let outgoing_step = planar_vector(candidate, route[slot + 1]);

            let has_alternative = attempt + 1 < ROUTE_CANDIDATE_COUNT;
            let successor_step = planar_vector(route[slot + 1], route[slot + 2]);
            if reverses_too_directly(outgoing_step, successor_step) && has_alternative {
                continue;
            }

            if slot == 1 {
                if !is_visible_planar_step(route[0], candidate) {
                    continue;
                }
                let entry_step = planar_vector(route[0], candidate);

                let incoming_step = final_step_for_block(identity, block_index - 1);
                if (reverses_too_directly(incoming_step, entry_step)
                    || reverses_too_directly(entry_step, outgoing_step))
                    && has_alternative
                {
                    continue;
                }
            }

            selected = Some(candidate);
            break;
        }

        // If a smooth turn is geometrically unavailable, a brief dwell permits
        // a deliberate turn rather than silently collapsing to a tiny leg.
        let mut point = selected
            .or_else(|| {
                (0..ROUTE_CANDIDATE_COUNT)
                    .map(|attempt| {
                        candidate_point(
                            interpolation,
                            (rotation + attempt) % ROUTE_CANDIDATE_COUNT,
                            z_waypoints[slot],
                        )
                    })
                    .find(|candidate| {
                        is_visible_planar_step(*candidate, route[slot + 1])
                            && (slot != 1 || is_visible_planar_step(route[0], *candidate))
                    })
            })
            .unwrap_or(interpolation);
        point.z = z_waypoints[slot];
        route[slot] = point;
    }

    if route.iter().all(is_finite_point) {
        debug_assert!(route_is_valid(&route), "route invalid: {route:#?}");
        route
    } else {
        neutral_route()
    }
}

fn final_route_target(
    identity: u64,
    block_index: i64,
    start_anchor: NormalizedLocomotionPoint,
    end_anchor: NormalizedLocomotionPoint,
    z: f32,
) -> NormalizedLocomotionPoint {
    let slot = LOCOMOTION_BLOCK_SEGMENTS - 1;
    let t = slot as f32 / LOCOMOTION_BLOCK_SEGMENTS as f32;
    let interpolation = clamp_xy(NormalizedLocomotionPoint {
        x: start_anchor.x + (end_anchor.x - start_anchor.x) * t,
        y: start_anchor.y + (end_anchor.y - start_anchor.y) * t,
        z,
    });
    let rotation = (hash_lane(identity, block_index, 0x100 + slot as u64)
        % ROUTE_CANDIDATE_COUNT as u64) as usize;

    for attempt in 0..ROUTE_CANDIDATE_COUNT {
        let candidate = candidate_point(
            interpolation,
            (rotation + attempt) % ROUTE_CANDIDATE_COUNT,
            z,
        );
        if is_visible_planar_step(candidate, end_anchor) {
            return candidate;
        }
    }

    interpolation
}

fn final_step_for_block(identity: u64, block_index: i64) -> (f32, f32) {
    let start_anchor = boundary_anchor(identity, block_index);
    let end_anchor = boundary_anchor(identity, block_index + 1);
    let z = depth_waypoints(block_index)[LOCOMOTION_BLOCK_SEGMENTS - 1];
    let final_target = final_route_target(identity, block_index, start_anchor, end_anchor, z);
    planar_vector(final_target, end_anchor)
}

fn boundary_anchor(identity: u64, block_index: i64) -> NormalizedLocomotionPoint {
    NormalizedLocomotionPoint {
        x: hash_range(
            identity,
            block_index,
            0x01,
            -LOCOMOTION_EXTREME_X_LIMIT,
            LOCOMOTION_EXTREME_X_LIMIT,
        ),
        y: hash_range(identity, block_index, 0x02, -0.45, 0.45),
        z: if block_index.rem_euclid(2) == 0 {
            -LOCOMOTION_DEPTH_EXTREME
        } else {
            LOCOMOTION_DEPTH_EXTREME
        },
    }
}

fn depth_waypoints(block_index: i64) -> [f32; ROUTE_POINTS] {
    let rear_to_front = [
        -LOCOMOTION_DEPTH_EXTREME,
        -LOCOMOTION_DEPTH_EXTREME,
        -LOCOMOTION_DEPTH_EXTREME,
        -LOCOMOTION_DEPTH_MID,
        -LOCOMOTION_DEPTH_MID,
        LOCOMOTION_DEPTH_MID,
        LOCOMOTION_DEPTH_MID,
        LOCOMOTION_DEPTH_EXTREME,
        LOCOMOTION_DEPTH_EXTREME,
    ];
    if block_index.rem_euclid(2) == 0 {
        rear_to_front
    } else {
        let mut front_to_rear = rear_to_front;
        front_to_rear.reverse();
        front_to_rear
    }
}

fn candidate_point(
    center: NormalizedLocomotionPoint,
    index: usize,
    z: f32,
) -> NormalizedLocomotionPoint {
    const DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const SHALLOW: f32 = 0.923_879_5;
    const STEEP: f32 = 0.382_683_43;
    const COMPASS: [(f32, f32); ROUTE_DIRECTIONS] = [
        (1.0, 0.0),
        (SHALLOW, STEEP),
        (DIAGONAL, DIAGONAL),
        (STEEP, SHALLOW),
        (0.0, 1.0),
        (-STEEP, SHALLOW),
        (-DIAGONAL, DIAGONAL),
        (-SHALLOW, STEEP),
        (-1.0, 0.0),
        (-SHALLOW, -STEEP),
        (-DIAGONAL, -DIAGONAL),
        (-STEEP, -SHALLOW),
        (0.0, -1.0),
        (STEEP, -SHALLOW),
        (DIAGONAL, -DIAGONAL),
        (SHALLOW, -STEEP),
    ];
    let radius = ROUTE_CANDIDATE_RADII[index / ROUTE_DIRECTIONS];
    let (x, y) = COMPASS[index % ROUTE_DIRECTIONS];
    clamp_xy(NormalizedLocomotionPoint {
        x: center.x + x * radius,
        y: center.y + y * radius,
        z,
    })
}

fn dwell_seconds(identity: u64, segment_index: i64) -> i64 {
    let range = (LOCOMOTION_DWELL_MAX_SECS - LOCOMOTION_DWELL_MIN_SECS + 1) as u64;
    LOCOMOTION_DWELL_MIN_SECS + (hash_lane(identity, segment_index, 0x200) % range) as i64
}

fn facing_for_segment(identity: u64, segment_index: i64, fallback_facing: i8) -> i8 {
    let target = route_target(identity, segment_index);
    let next = route_target(identity, segment_index + 1);
    if let Some(facing) = facing_for_delta(next.x - target.x) {
        return facing;
    }

    for successor in (segment_index + 1)..=(segment_index + LOCOMOTION_BLOCK_SEGMENTS as i64) {
        let start = route_target(identity, successor);
        let end = route_target(identity, successor + 1);
        if let Some(facing) = facing_for_delta(end.x - start.x) {
            return facing;
        }
    }
    normalize_facing(fallback_facing)
}

fn facing_for_delta(delta_x: f32) -> Option<i8> {
    if delta_x > LOCOMOTION_FACING_DEADZONE {
        Some(-1)
    } else if delta_x < -LOCOMOTION_FACING_DEADZONE {
        Some(1)
    } else {
        None
    }
}

fn glide_point(
    identity: u64,
    segment_index: i64,
    start: NormalizedLocomotionPoint,
    end: NormalizedLocomotionPoint,
    t: f32,
) -> NormalizedLocomotionPoint {
    let control = glide_control_point(identity, segment_index, start, end);
    let t = t.clamp(0.0, 1.0);
    let inverse_t = 1.0 - t;
    NormalizedLocomotionPoint {
        x: inverse_t * inverse_t * start.x + 2.0 * inverse_t * t * control.x + t * t * end.x,
        y: inverse_t * inverse_t * start.y + 2.0 * inverse_t * t * control.y + t * t * end.y,
        z: start.z + (end.z - start.z) * t,
    }
}

fn glide_control_point(
    identity: u64,
    segment_index: i64,
    start: NormalizedLocomotionPoint,
    end: NormalizedLocomotionPoint,
) -> NormalizedLocomotionPoint {
    let midpoint = NormalizedLocomotionPoint {
        x: (start.x + end.x) * 0.5,
        y: (start.y + end.y) * 0.5,
        z: (start.z + end.z) * 0.5,
    };
    let (dx, dy) = planar_vector(start, end);
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return midpoint;
    }

    let normal = (-dy / length, dx / length);
    let requested_offset = LOCOMOTION_CONTROL_OFFSET_FRACTION
        * length
        * if hash_lane(identity, segment_index, 0x300) & 1 == 0 {
            -1.0
        } else {
            1.0
        };
    let limit = control_offset_limit(midpoint, normal, requested_offset)
        .min(control_axis_monotonic_limit(
            midpoint.x,
            normal.0,
            requested_offset,
            start.x,
            end.x,
        ))
        .min(control_axis_monotonic_limit(
            midpoint.y,
            normal.1,
            requested_offset,
            start.y,
            end.y,
        ));
    let offset = requested_offset * limit;
    NormalizedLocomotionPoint {
        x: midpoint.x + normal.0 * offset,
        y: midpoint.y + normal.1 * offset,
        z: midpoint.z,
    }
}

fn control_offset_limit(
    midpoint: NormalizedLocomotionPoint,
    normal: (f32, f32),
    offset: f32,
) -> f32 {
    let mut limit: f32 = 1.0;
    for (mid, component) in [
        (midpoint.x, normal.0 * offset),
        (midpoint.y, normal.1 * offset),
    ] {
        if component > 0.0 {
            limit = limit.min((1.0 - mid) / component);
        } else if component < 0.0 {
            limit = limit.min((-1.0 - mid) / component);
        }
    }
    limit.clamp(0.0, 1.0)
}

fn control_axis_monotonic_limit(
    midpoint: f32,
    normal: f32,
    offset: f32,
    start: f32,
    end: f32,
) -> f32 {
    let component = normal * offset;
    if component > 0.0 {
        ((start.max(end) - midpoint) / component).clamp(0.0, 1.0)
    } else if component < 0.0 {
        ((start.min(end) - midpoint) / component).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// Covers most of a leg at a steady pace, with only a short acceleration and
/// brake at each end. This avoids the long low-velocity tails of a full-leg
/// minimum-jerk curve while preserving a continuous turn at waypoints.
fn swim_progress(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let cruise_velocity = 1.0 / (1.0 - SWIM_RAMP_FRACTION);

    if t < SWIM_RAMP_FRACTION {
        return cruise_velocity
            * 0.5
            * (t - SWIM_RAMP_FRACTION / std::f32::consts::PI
                * (std::f32::consts::PI * t / SWIM_RAMP_FRACTION).sin());
    }
    if t <= 1.0 - SWIM_RAMP_FRACTION {
        return cruise_velocity * (0.5 * SWIM_RAMP_FRACTION + t - SWIM_RAMP_FRACTION);
    }

    let remaining = 1.0 - t;
    1.0 - cruise_velocity
        * 0.5
        * (remaining
            - SWIM_RAMP_FRACTION / std::f32::consts::PI
                * (std::f32::consts::PI * remaining / SWIM_RAMP_FRACTION).sin())
}

#[cfg_attr(not(test), allow(dead_code))] // Used by the swim-profile contract test.
fn swim_velocity(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let cruise_velocity = 1.0 / (1.0 - SWIM_RAMP_FRACTION);

    if t < SWIM_RAMP_FRACTION {
        return cruise_velocity
            * 0.5
            * (1.0 - (std::f32::consts::PI * t / SWIM_RAMP_FRACTION).cos());
    }
    if t <= 1.0 - SWIM_RAMP_FRACTION {
        return cruise_velocity;
    }
    cruise_velocity * 0.5 * (1.0 - (std::f32::consts::PI * (1.0 - t) / SWIM_RAMP_FRACTION).cos())
}

fn route_is_valid(route: &[NormalizedLocomotionPoint; ROUTE_POINTS]) -> bool {
    route.iter().all(|point| {
        is_finite_point(point)
            && (-1.0..=1.0).contains(&point.x)
            && (-1.0..=1.0).contains(&point.y)
            && (-1.0..=1.0).contains(&point.z)
    }) && route.windows(2).all(|pair| {
        is_visible_planar_step(pair[0], pair[1])
            && (pair[1].z - pair[0].z).abs() <= LOCOMOTION_MAX_DEPTH_STEP + f32::EPSILON
    })
}

fn is_visible_planar_step(from: NormalizedLocomotionPoint, to: NormalizedLocomotionPoint) -> bool {
    let step = planar_vector(from, to);
    (MIN_PLANAR_TARGET_DISTANCE..=LOCOMOTION_MAX_PLANAR_STEP).contains(&planar_length(step))
        && step.0.abs() >= MIN_HORIZONTAL_TARGET_DISTANCE
}

fn neutral_route() -> [NormalizedLocomotionPoint; ROUTE_POINTS] {
    [NormalizedLocomotionPoint { x: 0.0, y: 0.0, z: 0.0 }; ROUTE_POINTS]
}

fn reverses_too_directly(previous: (f32, f32), next: (f32, f32)) -> bool {
    let previous_length = planar_length(previous);
    let next_length = planar_length(next);
    previous_length > f32::EPSILON
        && next_length > f32::EPSILON
        && (previous.0 * next.0 + previous.1 * next.1) / (previous_length * next_length) < -0.75
}

fn planar_vector(start: NormalizedLocomotionPoint, end: NormalizedLocomotionPoint) -> (f32, f32) {
    (end.x - start.x, end.y - start.y)
}

fn planar_length(vector: (f32, f32)) -> f32 {
    vector.0.hypot(vector.1)
}

fn clamp_xy(mut point: NormalizedLocomotionPoint) -> NormalizedLocomotionPoint {
    let x_limit = if point.z.abs() >= LOCOMOTION_DEPTH_EXTREME {
        LOCOMOTION_EXTREME_X_LIMIT
    } else if point.z.abs() >= LOCOMOTION_DEPTH_MID {
        LOCOMOTION_MID_X_LIMIT
    } else {
        LOCOMOTION_NEUTRAL_X_LIMIT
    };
    point.x = point.x.clamp(-x_limit, x_limit);
    point.y = point.y.clamp(-1.0, 1.0);
    point
}

fn is_finite_point(point: &NormalizedLocomotionPoint) -> bool {
    point.x.is_finite() && point.y.is_finite() && point.z.is_finite()
}

fn hash_range(identity: u64, index: i64, lane: u64, min: f32, max: f32) -> f32 {
    let fraction = hash_lane(identity, index, lane) as f64 / u64::MAX as f64;
    min + (max - min) * fraction as f32
}

fn hash_lane(identity: u64, index: i64, lane: u64) -> u64 {
    let mut hash = fnv1a(FNV_OFFSET_BASIS, &identity.to_le_bytes());
    hash = fnv1a(hash, &index.to_le_bytes());
    fnv1a(hash, &lane.to_le_bytes())
}

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
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
    use time::{Duration, OffsetDateTime};

    const NANOSECOND: i64 = 1;

    fn instant(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds).expect("test instant must be in range")
    }

    fn sample(identity: u64, seconds: i64) -> CompanionLocomotionSample {
        sample_companion_locomotion(identity, instant(seconds), 1)
    }

    fn planar_distance(a: NormalizedLocomotionPoint, b: NormalizedLocomotionPoint) -> f32 {
        (a.x - b.x).hypot(a.y - b.y)
    }

    fn axis_reversal_count(
        samples: &[CompanionLocomotionSample],
        axis: fn(NormalizedLocomotionPoint) -> f32,
    ) -> usize {
        let directions = samples
            .windows(2)
            .filter_map(|pair| {
                let delta = axis(pair[1].point) - axis(pair[0].point);
                (delta.abs() > 0.000_01).then_some(delta.signum() as i8)
            })
            .collect::<Vec<_>>();
        directions
            .windows(2)
            .filter(|pair| pair[0] != pair[1])
            .count()
    }

    fn assert_close(actual: f32, expected: f32, label: &str) {
        assert!(
            (actual - expected).abs() < 0.000_1,
            "{label}: expected {expected}, got {actual}"
        );
    }

    fn assert_same_point(actual: NormalizedLocomotionPoint, expected: NormalizedLocomotionPoint) {
        assert_close(actual.x, expected.x, "x");
        assert_close(actual.y, expected.y, "y");
        assert_close(actual.z, expected.z, "z");
    }

    #[test]
    fn same_identity_and_instant_are_restart_stable() {
        let identity = stable_companion_identity("restart-stable-pet");
        let now = instant(1_725_000_017) + Duration::milliseconds(321);

        assert_eq!(
            sample_companion_locomotion(identity, now, -1),
            sample_companion_locomotion(identity, now, -1)
        );
        assert_ne!(
            stable_companion_identity("restart-stable-pet"),
            stable_companion_identity("another-pet")
        );

        let today = sample(identity, 1_725_000_000);
        let tomorrow = sample(identity, 1_725_086_400);
        assert_ne!(
            today.point, tomorrow.point,
            "the wall-clock date must affect the route"
        );
    }

    #[test]
    fn each_segment_is_a_five_second_continuous_swim() {
        let identity = stable_companion_identity("dwell-duration");
        for segment in -24..24 {
            let dwell = dwell_seconds(identity, segment);
            assert_eq!(dwell, 0);
            let start = segment * LOCOMOTION_SEGMENT_SECS;
            assert_eq!(sample(identity, start).phase, LocomotionPhase::Glide);
            assert_eq!(LOCOMOTION_SEGMENT_SECS - dwell, 5);
        }
    }

    #[test]
    fn continuous_swim_meets_both_route_endpoints_exactly() {
        let identity = stable_companion_identity("continuous-swim-endpoints");
        let segment = 17;
        let start = segment * LOCOMOTION_SEGMENT_SECS;
        let target = route_target(identity, segment);
        let next = route_target(identity, segment + 1);
        let dwell = dwell_seconds(identity, segment);

        assert_same_point(sample(identity, start).point, target);
        assert_eq!(dwell, 0);
        assert_same_point(
            sample(identity, start + LOCOMOTION_SEGMENT_SECS).point,
            next,
        );
    }

    #[test]
    fn awake_swim_makes_meaningful_progress_each_second() {
        let identity = stable_companion_identity("purposeful-swim-cadence");

        for segment in -24..24 {
            let start_second = segment * LOCOMOTION_SEGMENT_SECS;
            let start = route_target(identity, segment);
            let end = route_target(identity, segment + 1);
            let route_distance = planar_distance(start, end);

            for elapsed_second in 0..LOCOMOTION_SEGMENT_SECS {
                let before = sample(identity, start_second + elapsed_second).point;
                let after = sample(identity, start_second + elapsed_second + 1).point;
                let distance = planar_distance(before, after);
                assert!(
                    distance >= route_distance * 0.15,
                    "segment {segment}, second {elapsed_second} moved only {distance} across a {route_distance} route"
                );
            }
        }
    }

    #[test]
    fn swim_profile_accelerates_gently_to_a_brisker_cruise() {
        for t in [0.0, 1.0] {
            assert_close(swim_velocity(t), 0.0, "endpoint velocity");
        }
        assert_close(swim_progress(0.0), 0.0, "start position");
        assert_close(swim_progress(1.0), 1.0, "end position");
        let cruise_velocity = swim_velocity(0.5);
        assert!(
            swim_velocity(0.10) < cruise_velocity,
            "the pet should still be accelerating after half a second"
        );
        assert!(
            cruise_velocity >= 1.16,
            "the middle cruise should be at least 16% faster than the leg average"
        );
    }

    #[test]
    fn quadratic_xy_bend_is_at_most_twelve_percent_and_z_is_unbent() {
        let identity = stable_companion_identity("quadratic-bend");
        for segment in 0..16 {
            let start = route_target(identity, segment);
            let end = route_target(identity, segment + 1);
            let control = glide_control_point(identity, segment, start, end);
            let midpoint = NormalizedLocomotionPoint {
                x: (start.x + end.x) * 0.5,
                y: (start.y + end.y) * 0.5,
                z: (start.z + end.z) * 0.5,
            };
            let bend = planar_distance(control, midpoint);
            assert!(
                bend <= LOCOMOTION_CONTROL_OFFSET_FRACTION * planar_distance(start, end)
                    + f32::EPSILON
            );

            let dwell = dwell_seconds(identity, segment);
            let glide_seconds = LOCOMOTION_SEGMENT_SECS - dwell;
            let halfway = instant(segment * LOCOMOTION_SEGMENT_SECS + dwell)
                + Duration::nanoseconds(glide_seconds * NANOS_PER_SECOND / 2);
            let eased = swim_progress(0.5);
            let sampled = sample_companion_locomotion(identity, halfway, 1);
            assert_close(
                sampled.point.z,
                start.z + (end.z - start.z) * eased,
                "z must interpolate without a bend",
            );
        }
    }

    #[test]
    fn route_stays_normalized_and_each_move_is_substantial_but_bounded() {
        for seed in 0..16 {
            let identity = stable_companion_identity(&format!("bounded-route-{seed}"));
            for block in -16..=16 {
                let route = route_block(identity, block);
                for point in route {
                    assert!((-1.0..=1.0).contains(&point.x));
                    assert!((-1.0..=1.0).contains(&point.y));
                    assert!((-1.0..=1.0).contains(&point.z));
                }
                for pair in route.windows(2) {
                    let planar_distance = planar_distance(pair[0], pair[1]);
                    assert!(
                        (0.35..=0.85 + f32::EPSILON).contains(&planar_distance),
                        "seed {seed}, block {block}, route leg {pair:?} had planar distance {planar_distance}"
                    );
                    assert!(
                        (pair[1].z - pair[0].z).abs() <= LOCOMOTION_MAX_DEPTH_STEP + f32::EPSILON
                    );
                }
            }
        }
    }

    #[test]
    fn route_avoids_identical_targets_and_unforced_direct_reversals() {
        let identity = stable_companion_identity("no-reversals");
        let mut targets = Vec::new();
        for segment in -8..24 {
            targets.push(route_target(identity, segment));
        }

        for pair in targets.windows(2) {
            assert!(planar_distance(pair[0], pair[1]) > f32::EPSILON);
        }
        for block in -1..3 {
            assert_no_unforced_route_reversals(identity, block, "no-reversals");
        }
    }

    fn has_non_reversing_source_candidate(identity: u64, block: i64, slot: usize) -> bool {
        assert!(
            (1..LOCOMOTION_BLOCK_SEGMENTS - 1).contains(&slot),
            "candidate source slots stop before the anchor-derived tail"
        );
        let route = route_block(identity, block);
        let start = boundary_anchor(identity, block);
        let end = boundary_anchor(identity, block + 1);
        let t = slot as f32 / LOCOMOTION_BLOCK_SEGMENTS as f32;
        let interpolation = clamp_xy(NormalizedLocomotionPoint {
            x: start.x + (end.x - start.x) * t,
            y: start.y + (end.y - start.y) * t,
            z: depth_waypoints(block)[slot],
        });
        let rotation = (hash_lane(identity, block, 0x100 + slot as u64)
            % ROUTE_CANDIDATE_COUNT as u64) as usize;
        (0..ROUTE_CANDIDATE_COUNT).any(|attempt| {
            let candidate = candidate_point(
                interpolation,
                (rotation + attempt) % ROUTE_CANDIDATE_COUNT,
                depth_waypoints(block)[slot],
            );
            let step = planar_vector(candidate, route[slot + 1]);
            if !is_visible_planar_step(candidate, route[slot + 1])
                || reverses_too_directly(step, planar_vector(route[slot + 1], route[slot + 2]))
            {
                return false;
            }

            if slot == 1 {
                let entry_step = planar_vector(start, candidate);
                let incoming_step = final_step_for_block(identity, block - 1);
                is_visible_planar_step(start, candidate)
                    && !reverses_too_directly(incoming_step, entry_step)
                    && !reverses_too_directly(entry_step, step)
            } else {
                true
            }
        })
    }

    fn assert_no_unforced_route_reversals(identity: u64, block: i64, label: &str) {
        let route = route_block(identity, block);

        for slot in 1..LOCOMOTION_BLOCK_SEGMENTS - 1 {
            let step = planar_vector(route[slot], route[slot + 1]);
            let successor_step = planar_vector(route[slot + 1], route[slot + 2]);
            assert!(
                !reverses_too_directly(step, successor_step)
                    || !has_non_reversing_source_candidate(identity, block, slot),
                "unforced reversal from source slot {slot} in block {block} for {label}"
            );
        }

        let incoming_step = final_step_for_block(identity, block - 1);
        let entry_step = planar_vector(route[0], route[1]);
        assert!(
            !reverses_too_directly(incoming_step, entry_step)
                || !has_non_reversing_source_candidate(identity, block, 1),
            "unforced cross-block entry reversal in block {block} for {label}"
        );
    }

    #[test]
    fn every_route_slot_rejects_an_unforced_direct_reversal() {
        let identity = stable_companion_identity("enum-0");
        let block = -74;
        let route = route_block(identity, block);

        assert!(
            !reverses_too_directly(
                planar_vector(route[5], route[6]),
                planar_vector(route[6], route[7]),
            ),
            "the reported slot-six to slot-seven reversal must be removed"
        );
        assert_no_unforced_route_reversals(identity, block, "enum-0");
    }

    #[test]
    fn a_sixteen_segment_window_reaches_visible_rear_and_front_depth() {
        for seed in ["depth-window-a", "depth-window-b"] {
            let identity = stable_companion_identity(seed);
            for start in [-8, 0, 8] {
                let mut points =
                    (start..=start + 16).map(|segment| route_target(identity, segment));
                assert!(points
                    .clone()
                    .any(|point| (-0.75..=-0.65).contains(&point.z)));
                assert!(points.any(|point| (0.65..=0.75).contains(&point.z)));
            }
        }
    }

    #[test]
    fn facing_stays_fixed_between_waypoints() {
        let identity = stable_companion_identity("facing-boundary");
        for segment in -8..16 {
            let start = segment * LOCOMOTION_SEGMENT_SECS;
            let facing = sample(identity, start).facing;
            for elapsed_second in 1..LOCOMOTION_SEGMENT_SECS {
                assert_eq!(sample(identity, start + elapsed_second).facing, facing);
            }
        }
    }

    #[test]
    fn substantial_horizontal_legs_keep_facing_unambiguous() {
        for seed in 0..64 {
            let identity = stable_companion_identity(&format!("facing-forward-{seed}"));
            for segment in -16..16 {
                let horizontal_step =
                    route_target(identity, segment + 1).x - route_target(identity, segment).x;
                let expected = facing_for_delta(horizontal_step).expect("substantial route leg");
                assert_eq!(
                    sample_companion_locomotion(
                        identity,
                        instant(segment * LOCOMOTION_SEGMENT_SECS),
                        -1,
                    )
                    .facing,
                    expected,
                    "seed {seed}, segment {segment} must face its route direction"
                );
            }
        }
    }

    #[test]
    fn every_block_entry_rejects_an_unforced_cross_block_reversal() {
        for seed in 0..64 {
            let identity = stable_companion_identity(&format!("entry-reversal-{seed}"));
            for block in -8..8 {
                let segment = block * LOCOMOTION_BLOCK_SEGMENTS as i64 + 1;
                let incoming = planar_vector(
                    route_target(identity, segment - 2),
                    route_target(identity, segment - 1),
                );
                let entry_step = planar_vector(
                    route_target(identity, segment - 1),
                    route_target(identity, segment),
                );

                assert!(
                    !reverses_too_directly(incoming, entry_step)
                        || !has_non_reversing_source_candidate(identity, block, 1),
                    "unforced cross-block reversal for seed {seed}, block {block}"
                );
            }
        }
    }

    #[test]
    fn every_candidate_selected_route_turn_rejects_unforced_reversals() {
        for seed in 0..16 {
            let identity = stable_companion_identity(&format!("all-route-turns-{seed}"));
            for block in -16..=16 {
                assert_no_unforced_route_reversals(identity, block, "all-route-turns");
            }
        }
    }

    #[test]
    fn every_phase_aligned_two_minute_window_has_at_most_twenty_reversals_per_axis() {
        for seed in 0..16 {
            let identity = stable_companion_identity(&format!("two-minute-window-{seed}"));
            for minute in [-9, -1, 0, 7, 15] {
                for phase in [1, 17, 39, 59] {
                    let start = minute * LOCOMOTION_SEGMENT_SECS + phase;
                    let samples = (start..=start + 120)
                        .map(|second| sample(identity, second))
                        .collect::<Vec<_>>();
                    assert!(axis_reversal_count(&samples, |point| point.x) <= 20);
                    assert!(axis_reversal_count(&samples, |point| point.y) <= 20);
                }
            }
        }
    }

    #[test]
    fn every_sliding_two_minute_window_has_at_most_twenty_four_facing_changes() {
        let mut saw_legitimate_twenty_one_change_window = false;
        for seed in 0..16 {
            let identity = stable_companion_identity(&format!("facing-cadence-{seed}"));
            for start in -600i64..=600 {
                let samples = (start..=start + 120)
                    .map(|second| sample(identity, second))
                    .collect::<Vec<_>>();
                let changes = samples
                    .windows(2)
                    .filter(|pair| pair[0].facing != pair[1].facing)
                    .count();
                saw_legitimate_twenty_one_change_window |= changes == 21;
                assert!(
                    changes <= 24,
                    "seed {seed}, start {start}: {changes} changes"
                );
            }
        }
        assert!(saw_legitimate_twenty_one_change_window);
    }

    #[test]
    fn segment_lookup_is_continuous_at_exact_segment_and_unix_zero_boundaries() {
        let identity = stable_companion_identity("negative-time");
        for boundary in [
            0,
            LOCOMOTION_SEGMENT_SECS,
            -LOCOMOTION_SEGMENT_SECS,
            LOCOMOTION_SEGMENT_SECS * 2,
        ] {
            let before = sample_companion_locomotion(
                identity,
                instant(boundary) - Duration::nanoseconds(NANOSECOND),
                1,
            );
            let at = sample(identity, boundary);
            assert_same_point(before.point, at.point);
            assert_eq!(
                at.segment_index,
                boundary.div_euclid(LOCOMOTION_SEGMENT_SECS)
            );
            assert_close(at.segment_phase, 0.0, "segment boundary phase");
        }
    }
}
