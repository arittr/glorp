# Glorp Purposeful Swim Cadence Design

**Date:** 2026-07-17
**Status:** Approved for implementation

## Problem

The round companion's locomotion currently couples two different behaviors:
travel speed and decision frequency. Shortening a route segment makes the pet
cross space faster, but it also gives the pet a new destination and facing
decision more often. The one-second experiment therefore made movement visible
by allowing a turn every second and loosening the two-minute reversal guard from
20 to 100. That reads as frenetic rather than animal-like.

The earlier five-second route cadence had the right broad character—longer,
purposeful swims—but some production-projected legs covered only two cells over
five seconds. Circular-aperture clipping and depth taper could concentrate that
small movement into part of the leg, making the pet appear nearly stationary.

The current visible-path pacing also linearizes physical arc within each leg and
restarts at a potentially different rate at every waypoint. That rate reset is
the acceleration problem: it produces abrupt visible speed changes even when
the route cadence and target selection are correct. It is not a reason to tune
`locomotion::swim_progress`.

## Goal

Restore infrequent, purposeful heading choices while keeping continuous awake
movement visibly active. The pet should commit to a broad heading or arc for
roughly four to eight seconds, travel at a readable speed throughout that swim,
and avoid abrupt reversals or rapid screen-crossing darts.

## Non-Goals

- Do not model species-specific animal biomechanics.
- Do not add a user-facing movement-speed setting.
- Do not change sleep, wake, Reduce Motion, activity-energy, depth, lighting,
  shadow, prop, or HUD behavior.
- Do not introduce random mutable route state; locomotion remains deterministic
  from identity and wall time.
- Do not add burst-and-rest behavior in this pass.

## Approved Behavior

- Awake route legs last five seconds with zero dwell.
- A meaningful heading or facing decision occurs at most once per route leg.
- Route legs remain gently curved and reject unforced direct reversals.
- The pet moves continuously during an awake leg; circular-aperture clipping
  must not flatten part of the leg into an apparent pause.
- The pet travels farther per decision rather than making decisions more often.
- Sleep settling and wake blending retain their existing timing and geometry.

## Architecture

### Route cadence and destination selection

`src/round/locomotion.rs` remains the owner of deterministic waypoints, facing,
and turn constraints. Restore the five-second segment cadence and the existing
20-axis-reversal ceiling over two minutes.

Candidate selection must favor substantial destination stride. It must not fall
back to an interpolation point that violates the same minimum-distance contract
used for ordinary candidates. If the preferred smooth-turn candidate is
unavailable, choose another deterministic valid candidate; do not silently
emit a tiny nominal swim.

### Visible-path pacing and acceleration

`src/round/motion.rs` remains the boundary that knows both the locomotion sample
and the round production projection. `pace_locomotion_for_visible_path` samples
the final aperture-safe center along the selected leg, measures arc in physical
points, and uses the existing inverse lookup with at least 20 visible-path
samples per five-second leg. It applies a deterministic C1 scalar screen-speed
profile, reading only the previous/current/next physical visible arc totals.

For current arc `L`, duration `T = 5`, `V = L/T`,
`V0 = min(Vprev, V)`, `V1 = min(V, Vnext)`, segment phase `t`,
`h(t) = (1 - cos(pi*t))/2`, and `g(t) = sin(pi*t)^2`:

```
v(t) = V0 + (V1 - V0) h(t) + (2V - V0 - V1) g(t)
p(t) = [V0*t + (V1 - V0)*(t/2 - sin(pi*t)/(2pi))
        + (2V - V0 - V1)*(t/2 - sin(2pi*t)/(4pi))] / V
```

Use `target_length = L * p(segment_phase)` in that inverse lookup. This shares
nonzero boundary speed with adjacent legs and has zero scalar acceleration at
boundaries, avoiding visible waypoint stop-starts while remaining monotone.

This pacing changes only progress within one five-second leg. It does not select
new targets, change facing, or increase decision frequency. Sleeping, waking,
and Reduce Motion paths bypass it and keep their established lifecycle logic.

### Measurement space

Behavior contracts use the final pet center after
`resolve_round_depth_placement` at the production `36x18`, `360x360` viewport.
Distances are measured in physical points (`10` points per X cell and `20`
points per Y cell), not raw normalized coordinates or cell-space Euclidean
distance.

## Motion Contract

For deterministic awake route corpora:

- Each complete five-second leg has `42..=75` points of rendered arc length.
- Every sliding two-second awake window, including across segment boundaries,
  covers `16..=48` points of rendered arc length.
- No 250 ms sample advances more than `6` points; awake speed is at least `7.5`
  points per second; consecutive 250 ms sampled speeds differ by at most `3.0`
  points per second (at most `12` points per second squared).
- Facing changes occur no more than once per five-second leg and no more than
  `24` route boundaries in any 120-second window. Axis reversals retain their
  separate `20` per-axis ceiling. A legitimate 21-change corpus window is
  accepted, so the facing cap must not broaden route backtracking.
- Unforced direct reversals remain rejected by the existing vector-dot-product
  rule.

These are production-geometry guardrails, not a promise that every leg has
identical velocity. Gentle acceleration, braking, and curvature remain visible.

## Failure Behavior

Invalid or non-finite route candidates remain rejected. Candidate exhaustion
must resolve to a deterministic candidate that satisfies the route-distance
contract; it must not create a tiny interpolation fallback. Existing
last-good-frame and invalid-scene handling remain unchanged.

## Verification

Implementation verification must prove:

1. Five-second cadence and the original reversal ceiling are restored.
2. Route candidates and all fallbacks satisfy normalized distance, depth-step,
   and direct-reversal contracts.
3. Final production-projected motion satisfies the physical-point speed,
   per-leg distance, 250 ms lurch, and gradual-acceleration bounds above across
   a shared identity and segment corpus, including segment boundaries.
4. Facing changes only at five-second route boundaries and stay within the
   separate 24-change cap; axis reversals retain their 20-change ceiling.
5. Sleep settling, wake blending, depth placement, round scene, and Reduce
   Motion tests retain their current behavior.
6. Preview Lab's purposeful-locomotion strip shows long continuous swims with
   infrequent turns, gradual visible speed changes, and no waypoint stop-start
   or apparent pause at front or rear depth.
7. The optimized companion is launched for Drew's visual acceptance after
   automated checks pass.

## Done Criteria

- The pet visibly travels throughout an awake swim.
- Meaningful turns occur roughly every four to eight seconds, not every second.
- Movement reads as a pet exploring a tank: steady and purposeful, neither
  glacial nor frenetic.
- Speed and heading cadence are independently tested so future tuning cannot
  trade one failure mode for the other.
