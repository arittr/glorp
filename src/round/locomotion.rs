//! Deterministic locomotion shared by live companion motion and Preview Lab.

pub(crate) const LOCOMOTION_SEGMENT_SECS: i64 = 16;
pub(crate) const LOCOMOTION_BLOCK_SEGMENTS: usize = 8;
pub(crate) const LOCOMOTION_DWELL_MIN_SECS: i64 = 2;
pub(crate) const LOCOMOTION_DWELL_MAX_SECS: i64 = 4;
pub(crate) const LOCOMOTION_MAX_PLANAR_STEP: f32 = 0.62;
pub(crate) const LOCOMOTION_MAX_DEPTH_STEP: f32 = 0.67;
const LOCOMOTION_CONTROL_OFFSET_FRACTION: f32 = 0.12;
const LOCOMOTION_FACING_DEADZONE: f32 = 0.06;
const LOCOMOTION_DEPTH_EXTREME: f32 = 0.70;
const LOCOMOTION_DEPTH_MID: f32 = 0.23;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const MIN_PLANAR_TARGET_DISTANCE: f32 = 0.12;
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
    let dwell = dwell_seconds(identity, segment_index);
    let dwell_nanos = dwell * NANOS_PER_SECOND;
    let start = route_target(identity, segment_index);
    let end = route_target(identity, segment_index + 1);
    let facing = facing_for_segment(identity, segment_index, fallback_facing);

    if elapsed_nanos < dwell_nanos {
        return CompanionLocomotionSample {
            point: start,
            facing,
            phase: LocomotionPhase::Dwell,
            segment_index,
            segment_phase: segment_phase as f32,
        };
    }

    let glide_nanos = segment_nanos - dwell_nanos;
    let glide_phase = (elapsed_nanos - dwell_nanos) as f64 / glide_nanos as f64;
    CompanionLocomotionSample {
        point: glide_point(
            identity,
            segment_index,
            start,
            end,
            minimum_jerk(glide_phase as f32),
        ),
        facing,
        phase: LocomotionPhase::Glide,
        segment_index,
        segment_phase: segment_phase as f32,
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
        let rotation = (hash_lane(identity, block_index, 0x100 + slot as u64) % 8) as usize;
        let mut selected = None;

        for attempt in 0..8 {
            let candidate_index = (rotation + attempt) % 8;
            let candidate = candidate_point(interpolation, candidate_index, z_waypoints[slot]);
            let outgoing_step = planar_vector(candidate, route[slot + 1]);
            let outgoing_length = planar_length(outgoing_step);
            if !(MIN_PLANAR_TARGET_DISTANCE..=LOCOMOTION_MAX_PLANAR_STEP).contains(&outgoing_length)
            {
                continue;
            }

            let has_alternative = attempt < 7;
            let successor_step = planar_vector(route[slot + 1], route[slot + 2]);
            if reverses_too_directly(outgoing_step, successor_step) && has_alternative {
                continue;
            }

            if slot == 1 {
                let entry_step = planar_vector(route[0], candidate);
                if !(MIN_PLANAR_TARGET_DISTANCE..=LOCOMOTION_MAX_PLANAR_STEP)
                    .contains(&planar_length(entry_step))
                {
                    continue;
                }

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

        let mut point = selected.unwrap_or(interpolation);
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
    let rotation = (hash_lane(identity, block_index, 0x100 + slot as u64) % 8) as usize;

    for attempt in 0..8 {
        let candidate = candidate_point(interpolation, (rotation + attempt) % 8, z);
        let end_step = planar_vector(candidate, end_anchor);
        if (MIN_PLANAR_TARGET_DISTANCE..=LOCOMOTION_MAX_PLANAR_STEP)
            .contains(&planar_length(end_step))
        {
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
        x: hash_range(identity, block_index, 0x01, -0.55, 0.55),
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
    const COMPASS: [(f32, f32); 8] = [
        (1.0, 0.0),
        (DIAGONAL, DIAGONAL),
        (0.0, 1.0),
        (-DIAGONAL, DIAGONAL),
        (-1.0, 0.0),
        (-DIAGONAL, -DIAGONAL),
        (0.0, -1.0),
        (DIAGONAL, -DIAGONAL),
    ];
    let radius = if index.is_multiple_of(2) { 0.18 } else { 0.28 };
    let (x, y) = COMPASS[index];
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

fn minimum_jerk(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (10.0 + t * (-15.0 + 6.0 * t))
}

#[cfg_attr(not(test), allow(dead_code))] // Used by the endpoint derivative contract test.
fn minimum_jerk_velocity(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    30.0 * t * t * (1.0 - t) * (1.0 - t)
}

#[cfg_attr(not(test), allow(dead_code))] // Used by the endpoint derivative contract test.
fn minimum_jerk_acceleration(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    60.0 * t - 180.0 * t * t + 120.0 * t * t * t
}

fn route_is_valid(route: &[NormalizedLocomotionPoint; ROUTE_POINTS]) -> bool {
    route.iter().all(|point| {
        is_finite_point(point)
            && (-1.0..=1.0).contains(&point.x)
            && (-1.0..=1.0).contains(&point.y)
            && (-1.0..=1.0).contains(&point.z)
    }) && route.windows(2).all(|pair| {
        let distance = planar_length(planar_vector(pair[0], pair[1]));
        distance <= LOCOMOTION_MAX_PLANAR_STEP
            && (pair[1].z - pair[0].z).abs() <= LOCOMOTION_MAX_DEPTH_STEP + f32::EPSILON
    })
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
    point.x = point.x.clamp(-1.0, 1.0);
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
    fn each_segment_has_a_brief_turn_and_a_visibly_purposeful_glide() {
        let identity = stable_companion_identity("dwell-duration");
        for segment in -24..24 {
            let dwell = dwell_seconds(identity, segment);
            assert!(
                (LOCOMOTION_DWELL_MIN_SECS..=LOCOMOTION_DWELL_MAX_SECS).contains(&dwell),
                "segment {segment} had dwell {dwell}"
            );

            let start = segment * LOCOMOTION_SEGMENT_SECS;
            assert_eq!(sample(identity, start).phase, LocomotionPhase::Dwell);
            assert_eq!(
                sample(identity, start + dwell - 1).phase,
                LocomotionPhase::Dwell
            );
            assert_eq!(
                sample(identity, start + dwell).phase,
                LocomotionPhase::Glide
            );
            let glide = LOCOMOTION_SEGMENT_SECS - dwell;
            assert!(
                (12..=14).contains(&glide),
                "segment {segment} had a {glide}-second glide"
            );
        }
    }

    #[test]
    fn dwell_is_stationary_and_glide_meets_both_endpoints_exactly() {
        let identity = stable_companion_identity("dwell-and-endpoints");
        let segment = 17;
        let start = segment * LOCOMOTION_SEGMENT_SECS;
        let target = route_target(identity, segment);
        let next = route_target(identity, segment + 1);
        let dwell = dwell_seconds(identity, segment);

        assert_same_point(sample(identity, start).point, target);
        assert_same_point(sample(identity, start + dwell - 1).point, target);
        assert_same_point(sample(identity, start + dwell).point, target);
        assert_same_point(
            sample(identity, start + LOCOMOTION_SEGMENT_SECS).point,
            next,
        );
    }

    #[test]
    fn minimum_jerk_has_zero_velocity_and_acceleration_at_endpoints() {
        for t in [0.0, 1.0] {
            assert_close(minimum_jerk_velocity(t), 0.0, "endpoint velocity");
            assert_close(minimum_jerk_acceleration(t), 0.0, "endpoint acceleration");
        }
        assert_close(minimum_jerk(0.0), 0.0, "start position");
        assert_close(minimum_jerk(1.0), 1.0, "end position");
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
            let eased = minimum_jerk(0.5);
            let sampled = sample_companion_locomotion(identity, halfway, 1);
            assert_close(
                sampled.point.z,
                start.z + (end.z - start.z) * eased,
                "z must interpolate without a bend",
            );
        }
    }

    #[test]
    fn route_stays_normalized_and_each_move_is_bounded() {
        let identity = stable_companion_identity("bounded-route");
        for block in -2..3 {
            let route = route_block(identity, block);
            for point in route {
                assert!((-1.0..=1.0).contains(&point.x));
                assert!((-1.0..=1.0).contains(&point.y));
                assert!((-1.0..=1.0).contains(&point.z));
            }
            for pair in route.windows(2) {
                assert!(
                    planar_distance(pair[0], pair[1]) <= LOCOMOTION_MAX_PLANAR_STEP + f32::EPSILON
                );
                assert!((pair[1].z - pair[0].z).abs() <= LOCOMOTION_MAX_DEPTH_STEP + f32::EPSILON);
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
        let rotation = (hash_lane(identity, block, 0x100 + slot as u64) % 8) as usize;
        (0..8).any(|attempt| {
            let candidate = candidate_point(
                interpolation,
                (rotation + attempt) % 8,
                depth_waypoints(block)[slot],
            );
            let step = planar_vector(candidate, route[slot + 1]);
            let distance = planar_length(step);
            if !(MIN_PLANAR_TARGET_DISTANCE..=LOCOMOTION_MAX_PLANAR_STEP).contains(&distance)
                || reverses_too_directly(step, planar_vector(route[slot + 1], route[slot + 2]))
            {
                return false;
            }

            if slot == 1 {
                let entry_step = planar_vector(start, candidate);
                let incoming_step = final_step_for_block(identity, block - 1);
                (MIN_PLANAR_TARGET_DISTANCE..=LOCOMOTION_MAX_PLANAR_STEP)
                    .contains(&planar_length(entry_step))
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
    fn facing_changes_only_at_the_stationary_segment_start() {
        let identity = stable_companion_identity("facing-boundary");
        for segment in -8..16 {
            let start = segment * LOCOMOTION_SEGMENT_SECS;
            let dwell = dwell_seconds(identity, segment);
            let facing = sample(identity, start).facing;
            assert_eq!(sample(identity, start + dwell - 1).facing, facing);
            assert_eq!(sample(identity, start + dwell).facing, facing);
            assert_eq!(
                sample(identity, start + LOCOMOTION_SEGMENT_SECS - 1).facing,
                facing
            );
        }
    }

    #[test]
    fn deadzone_facing_uses_the_next_meaningful_direction() {
        let mut covered_deadzone = false;

        for seed in 0..64 {
            let identity = stable_companion_identity(&format!("facing-forward-{seed}"));
            for segment in -16..16 {
                if facing_for_delta(
                    route_target(identity, segment + 1).x - route_target(identity, segment).x,
                )
                .is_some()
                {
                    continue;
                }

                let expected = ((segment + 1)..=(segment + LOCOMOTION_BLOCK_SEGMENTS as i64))
                    .find_map(|successor| {
                        facing_for_delta(
                            route_target(identity, successor + 1).x
                                - route_target(identity, successor).x,
                        )
                    });
                let Some(expected) = expected else {
                    continue;
                };

                covered_deadzone = true;
                assert_eq!(
                    sample_companion_locomotion(
                        identity,
                        instant(segment * LOCOMOTION_SEGMENT_SECS),
                        -1,
                    )
                    .facing,
                    expected,
                    "seed {seed}, segment {segment} must look forward from its deadzone"
                );
            }
        }

        assert!(covered_deadzone, "the fixtures must exercise a deadzone");
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
    fn every_phase_aligned_two_minute_window_has_at_most_eight_reversals_per_axis() {
        for seed in 0..16 {
            let identity = stable_companion_identity(&format!("two-minute-window-{seed}"));
            for minute in [-9, -1, 0, 7, 15] {
                for phase in [1, 17, 39, 59] {
                    let start = minute * LOCOMOTION_SEGMENT_SECS + phase;
                    let samples = (start..=start + 120)
                        .map(|second| sample(identity, second))
                        .collect::<Vec<_>>();
                    let x_reversals = axis_reversal_count(&samples, |point| point.x);
                    let y_reversals = axis_reversal_count(&samples, |point| point.y);
                    assert!(
                        x_reversals <= 8,
                        "seed {seed} at minute {minute}, phase {phase} had {x_reversals} X reversals"
                    );
                    assert!(
                        y_reversals <= 8,
                        "seed {seed} at minute {minute}, phase {phase} had {y_reversals} Y reversals"
                    );
                }
            }
        }
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
