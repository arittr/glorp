# Glorp Purposeful Swim Cadence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the round companion travel visibly and continuously while choosing a new heading only about once every five seconds.

**Architecture:** Keep deterministic route/facing ownership in `round::locomotion`, restore five-second route legs, and select substantial valid strides without tiny interpolation fallbacks. In `round::motion`, measure the final aperture-safe path in physical points and map each leg through a deterministic C1 scalar screen-speed profile so visible pace remains gradual across waypoints without changing cadence or targets.

**Tech Stack:** Rust, `time`, Glorp round motion/depth/placement modules, Preview Lab, Cargo test/Clippy/formatting.

## Global Constraints

- Awake route legs last exactly five seconds with zero dwell.
- Meaningful facing decisions happen at most once per route leg and no more than 24 times in any 120-second window; five-second legs allow at most 24 boundary decisions.
- Axis reversals remain at most 20 per axis in every 120-second window.
- Complete production-projected legs cover 42 through 75 points of rendered arc.
- Every sliding two-second awake window covers 16 through 48 rendered points.
- No 250 ms awake sample advances more than 6 rendered points.
- Awake speed is at least 7.5 points per second, and consecutive 250 ms sampled speeds differ by at most 3.0 points per second (12 points per second squared).
- Visible-path inverse lookup uses at least 20 samples per five-second leg.
- Distances use final centers from the `36x18`, `360x360` production viewport: 10 points per X cell and 20 points per Y cell.
- Preserve deterministic identity-plus-wall-time routing, no-direct-reversal behavior, depth `±0.70`, zero dwell, sleep settling, wake blending, and Reduce Motion behavior.
- Do not change lighting, shadows, props, HUD, activity energy, renderer selection, or persisted state.
- Preserve the approved scope: do not tune `locomotion::swim_progress`, alter sleep/wake/Reduce Motion bypasses, add a renderer or artifact system, or broaden route generation for the facing-change cap.

---

### Task 1: Restore independent five-second heading cadence

**Files:**
- Modify: `src/round/locomotion.rs:3-118`
- Test: `src/round/locomotion.rs:639-1000`

**Interfaces:**
- Consumes: existing `sample_companion_locomotion(...)` and `sample_companion_locomotion_at_segment_phase(...)`.
- Produces: `LOCOMOTION_SEGMENT_SECS = 5` while retaining explicit phase sampling for visible-path pacing.

- [ ] **Step 1: Restore the cadence contracts in tests before changing the constant**

Change the current one-second and 100-reversal expectations while keeping the
axis-reversal ceiling separate from the facing-change cap:

```rust
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
            assert!(changes <= 24, "seed {seed}, start {start}: {changes} changes");
        }
    }
    assert!(saw_legitimate_twenty_one_change_window);
}
```

- [ ] **Step 2: Run the cadence tests and confirm the one-second experiment fails**

Run:

```bash
cargo test --lib round::locomotion::tests::each_segment_is_a_five_second_continuous_swim -- --exact
cargo test --lib round::locomotion::tests::every_phase_aligned_two_minute_window_has_at_most_twenty_reversals_per_axis -- --exact
cargo test --lib round::locomotion::tests::every_sliding_two_minute_window_has_at_most_twenty_four_facing_changes -- --exact
```

Expected: the first test fails because the current constant is `1`; the reversal test fails because the current route can make decisions every second. The facing test is a broad representative negative/positive sliding-window guard: it proves the known legitimate 21-change window remains valid and rejects only values above 24; do not change route generation to satisfy it.

- [ ] **Step 3: Restore the five-second production cadence without removing explicit phase sampling**

Set:

```rust
pub(crate) const LOCOMOTION_SEGMENT_SECS: i64 = 5;
```

Keep `sample_companion_locomotion_at_segment_phase(...)`. `sample_companion_locomotion(...)` must continue deriving `segment_index` and `segment_phase` from wall time, then delegate to the explicit-phase helper.

- [ ] **Step 4: Verify cadence, restart stability, continuity, and turn guards**

Run:

```bash
cargo test --lib round::locomotion::tests
```

Expected: all locomotion tests pass, including exact endpoints, five-second cadence, boundary continuity, the 20-axis-reversal ceiling, and the separate 24-facing-change cap.

- [ ] **Step 5: Commit the cadence restoration**

```bash
git add src/round/locomotion.rs
git commit -m "fix(companion): restore purposeful heading cadence"
```

### Task 2: Select substantial deterministic route strides without invalid fallback legs

**Files:**
- Modify: `src/round/locomotion.rs:126-260`
- Test: `src/round/locomotion.rs:759-960`

**Interfaces:**
- Consumes: `candidate_point(...)`, `is_visible_planar_step(...)`, `reverses_too_directly(...)`, and the current 64 deterministic candidate ordering.
- Produces: `candidate_preference(distance: f32) -> f32` and route selection that prefers a `0.60` normalized stride while preserving hard `0.35..=0.85` validity.

- [ ] **Step 1: Add a failing shared-corpus fallback regression**

Add this test helper and regression inside `locomotion::tests`:

```rust
fn assert_route_corpus_is_valid(prefix: &str) {
    for seed in 0..16 {
        let identity = stable_companion_identity(&format!("{prefix}-{seed}"));
        for block in -16..=16 {
            let route = route_block(identity, block);
            assert!(route_is_valid(&route), "{prefix} seed {seed}, block {block}: {route:#?}");
        }
    }
}

#[test]
fn every_production_motion_identity_uses_only_valid_route_legs() {
    for prefix in [
        "bounded-route",
        "visible-production-route",
        "visible-production-speed",
    ] {
        assert_route_corpus_is_valid(prefix);
    }
}
```

Also add a focused selection test that asserts the chosen candidate is the valid candidate closest to `0.60` normalized distance, not merely the first hash-rotated candidate:

```rust
#[test]
fn route_prefers_a_substantial_valid_stride() {
    let successor = NormalizedLocomotionPoint { x: 0.60, y: 0.0, z: 0.0 };
    let short = NormalizedLocomotionPoint { x: 0.24, y: 0.0, z: 0.0 };
    let preferred = NormalizedLocomotionPoint { x: 0.0, y: 0.0, z: 0.0 };

    let selected = prefer_route_candidate(None, short, successor);
    let selected = prefer_route_candidate(selected, preferred, successor);

    assert_eq!(selected.unwrap().0, preferred);
}
```

- [ ] **Step 2: Run the new route tests before changing candidate selection**

Run:

```bash
cargo test --lib round::locomotion::tests::every_production_motion_identity_uses_only_valid_route_legs -- --exact
cargo test --lib round::locomotion::tests::route_prefers_a_substantial_valid_stride -- --exact
```

Expected: the preference test fails because current selection accepts the first valid rotated candidate; the corpus test protects candidate exhaustion and interpolation fallback behavior.

- [ ] **Step 3: Add deterministic stride preference**

Add:

```rust
const PREFERRED_PLANAR_TARGET_DISTANCE: f32 = 0.60;

fn candidate_preference(distance: f32) -> f32 {
    (distance - PREFERRED_PLANAR_TARGET_DISTANCE).abs()
}

fn prefer_route_candidate(
    selected: Option<(NormalizedLocomotionPoint, f32)>,
    candidate: NormalizedLocomotionPoint,
    successor: NormalizedLocomotionPoint,
) -> Option<(NormalizedLocomotionPoint, f32)> {
    let score = candidate_preference(planar_length(planar_vector(candidate, successor)));
    match selected {
        Some((_, best_score)) if best_score <= score => selected,
        _ => Some((candidate, score)),
    }
}
```

In the main `route_block` candidate loop, retain all existing validity and reversal filters, but inspect every eligible candidate and keep the candidate with the lowest `candidate_preference(planar_length(outgoing_step))`. Break exact score ties by the existing rotated attempt order.

Use the same preference rule in the relaxed second pass and in `final_route_target(...)`. The relaxed pass may relax an unavailable smooth-turn preference, but it must still require `is_visible_planar_step(...)` for every adjacent leg.

Add `forced_visible_candidate(...)` for candidate exhaustion. It enumerates the same four radii and 16 compass directions around the known successor, rotated by the existing identity hash, then retains only candidates that are valid from the optional predecessor and to the successor. Because the smallest radius is `0.36`, at least one horizontal direction toward the route center remains within the fixed depth-dependent X bounds. The helper returns the candidate closest to the preferred `0.60` stride. Cover both `Some(route[0])` for slot 1 and `None` for other slots in unit tests. Remove `unwrap_or(interpolation)` completely; use `forced_visible_candidate(...).expect("fixed route geometry must provide a visible candidate")` so a violated geometry invariant cannot silently become a tiny nominal swim.

```rust
fn forced_visible_candidate(
    predecessor: Option<NormalizedLocomotionPoint>,
    successor: NormalizedLocomotionPoint,
    z: f32,
    rotation: usize,
) -> Option<NormalizedLocomotionPoint> {
    let mut selected = None;
    for attempt in 0..ROUTE_CANDIDATE_COUNT {
        let candidate = candidate_point(
            successor,
            (rotation + attempt) % ROUTE_CANDIDATE_COUNT,
            z,
        );
        if !is_visible_planar_step(candidate, successor)
            || predecessor.is_some_and(|point| !is_visible_planar_step(point, candidate))
        {
            continue;
        }
        selected = prefer_route_candidate(selected, candidate, successor);
    }
    selected.map(|(candidate, _)| candidate)
}
```

The production loops call the same helper after their validity and reversal filters:

```rust
selected = prefer_route_candidate(selected, candidate, route[slot + 1]);
```

- [ ] **Step 4: Verify route validity, distance bounds, deterministic continuity, and reversal behavior**

Run:

```bash
cargo test --lib round::locomotion::tests
```

Expected: all route corpora satisfy `0.35..=0.85`, cross-block continuity remains exact, and no unforced direct-reversal test regresses.

- [ ] **Step 5: Commit substantial route stride selection**

```bash
git add src/round/locomotion.rs
git commit -m "fix(companion): choose substantial swim destinations"
```

### Task 3: Pace visible arcs with gradual C1 screen-speed changes

**Files:**
- Modify: `src/round/motion.rs:1-460`
- Test: `src/round/motion.rs:535-740`

**Interfaces:**
- Consumes: `sample_companion_locomotion_at_segment_phase(...)`, `resolve_smooth_depth(...)`, and `resolve_round_depth_placement(...)`.
- Produces: `visible_distance_points(...) -> f32`, `screen_arc_profile(...) -> f32`, and `pace_locomotion_for_visible_path(...)`, which redistribute progress only inside the current five-second leg.

- [ ] **Step 1: Add the shared-corpus production speed and waypoint regressions**

Add test helpers:

```rust
const PRODUCTION_SAMPLE_MS: i64 = 250;
const VISIBLE_PATH_SAMPLES_PER_LEG: usize = 20;
const MIN_LEG_ARC_POINTS: f32 = 42.0;
const MAX_LEG_ARC_POINTS: f32 = 75.0;
const MIN_TWO_SECOND_ARC_POINTS: f32 = 16.0;
const MAX_TWO_SECOND_ARC_POINTS: f32 = 48.0;
const MAX_QUARTER_SECOND_STEP_POINTS: f32 = 6.0;
const MIN_AWAKE_SPEED_POINTS_PER_SECOND: f32 = 7.5;
const MAX_SPEED_DELTA_PER_SAMPLE: f32 = 3.0;

fn visible_distance_points(
    from: MotionPoint,
    to: MotionPoint,
    viewport: RoundCompanionMotionViewport,
) -> f32 {
    let point_scale_x = viewport.width_points / f32::from(viewport.grid_columns);
    let point_scale_y = viewport.height_points / f32::from(viewport.grid_rows);
    ((to.x - from.x) * point_scale_x).hypot((to.y - from.y) * point_scale_y)
}
```

Use the same `visible-production-*` identity and `-16..=16` segment corpus for
all three tests. Sample final centers every 250 ms, with windows that continue
across segment boundaries. The existing overall regression remains named
`every_awake_swim_stays_within_production_speed_bounds`; add the named
regressions `screen_speed_changes_gradually_across_production_waypoints` and
`screen_arc_profile_never_stops_at_an_awake_waypoint`.

The overall test asserts complete-leg, two-second, per-step, and awake-speed
bounds:

```rust
assert!((MIN_LEG_ARC_POINTS..=MAX_LEG_ARC_POINTS).contains(&full_leg_arc));
for window in all_awake_step_lengths.windows(8) {
    let two_second_arc: f32 = window.iter().sum();
    assert!((MIN_TWO_SECOND_ARC_POINTS..=MAX_TWO_SECOND_ARC_POINTS).contains(&two_second_arc));
}
assert!(step_lengths.iter().all(|distance| *distance <= MAX_QUARTER_SECOND_STEP_POINTS));
assert!(step_lengths.iter().all(|distance| {
    *distance / (PRODUCTION_SAMPLE_MS as f32 / 1000.0)
        >= MIN_AWAKE_SPEED_POINTS_PER_SECOND
}));
```

The gradual-speed regression derives a points-per-second value for every 250 ms
step and asserts every adjacent pair differs by at most `3.0`; it explicitly
includes the last sample of one segment and first sample of the next:

```rust
let speeds = all_awake_step_lengths.iter().map(|distance| {
    *distance / (PRODUCTION_SAMPLE_MS as f32 / 1000.0)
}).collect::<Vec<_>>();
assert!(speeds.windows(2).all(|pair| {
    (pair[1] - pair[0]).abs() <= MAX_SPEED_DELTA_PER_SAMPLE
}));
```

The no-stop regression asserts those same awake boundary-adjacent steps remain
strictly positive, so a waypoint cannot become a stop-start. Keep all existing
sleep, wake-blending, and Reduce Motion tests as their lifecycle proof.

- [ ] **Step 2: Run the new production contracts before implementing the C1 profile**

Run:

```bash
cargo test --lib round::motion::tests::every_awake_swim_stays_within_production_speed_bounds -- --exact --nocapture
cargo test --lib round::motion::tests::screen_speed_changes_gradually_across_production_waypoints -- --exact --nocapture
cargo test --lib round::motion::tests::screen_arc_profile_never_stops_at_an_awake_waypoint -- --exact --nocapture
```

Expected: FAIL on the speed-transition or waypoint-continuity assertion because
linear per-leg arc pacing restarts at a potentially different rate at each
waypoint. Do not tune `locomotion::swim_progress`; the fault is in visible-path
pacing.

- [ ] **Step 3: Implement physical-point inverse lookup with a C1 scalar speed profile**

Keep `pace_locomotion_for_visible_path(...)`, calculating physical visible arc
totals with `visible_distance_points(...)` and the production viewport's 2:1
cell aspect ratio. Sample each five-second leg at least
`VISIBLE_PATH_SAMPLES_PER_LEG` times before inverse lookup; do not introduce
coarse quarter-second speed steps.

It may read the previous, current, and next legs' complete physical visible arc
totals, but must return a phase only in the current segment. For current arc
`L`, `T = 5`, `V = L / T`, `V0 = min(Vprev, V)`, `V1 = min(V, Vnext)`, and
`t = sample.segment_phase`, implement this profile or an algebraically
equivalent monotone form:

```rust
let h = (1.0 - (std::f32::consts::PI * t).cos()) / 2.0;
let g = (std::f32::consts::PI * t).sin().powi(2);
let v = v0 + (v1 - v0) * h + (2.0 * v - v0 - v1) * g;
let p = (v0 * t
    + (v1 - v0) * (t / 2.0 - (std::f32::consts::PI * t).sin() / (2.0 * std::f32::consts::PI))
    + (2.0 * v - v0 - v1)
        * (t / 2.0 - (2.0 * std::f32::consts::PI * t).sin() / (4.0 * std::f32::consts::PI))) / v;
let target_length = current_arc_length * p;
```

Use `target_length` in the existing inverse arc lookup, then re-sample only the
current `sample.segment_index` at that phase. Adjacent legs share nonzero
boundary speed (`V1`/`V0`), while derivatives of `h` and `g` are zero at the
boundaries, giving zero scalar acceleration there. The function must remain
monotone and must not change a target, facing, segment index, cadence, or route
generation.

The pacing function must:

```rust
if sample.phase != LocomotionPhase::Glide {
    return sample;
}
```

It must also retain the lifecycle bypass for asleep and wake-blended inputs. It may only re-sample phase inside `sample.segment_index`; it must not change the segment index, route target, facing, or wall-clock cadence.

- [ ] **Step 4: Run motion, placement, and round-scene behavior tests**

Run:

```bash
cargo test --lib round::motion::tests
cargo test --lib round::placement::tests
cargo test --test round_scene
```

Expected: all three production speed tests pass, including gradual boundary
speed and no awake waypoint stop; sleeping settles to neutral, waking blends
from neutral, depth placement remains aperture-safe, and round scene contracts
remain green.

- [ ] **Step 5: Commit physical screen-space pacing**

```bash
git add src/round/motion.rs
git commit -m "fix(companion): pace swims in visible screen space"
```

### Task 4: Verify Preview Lab behavior and launch the optimized companion

**Files:**
- Modify: `src/dev_preview/smooth.rs:190-540`
- Test: `tests/dev_preview.rs:3093-3590`

**Interfaces:**
- Consumes: the restored five-second locomotion cadence and C1 physical point-space pacing.
- Produces: the existing deterministic purposeful-locomotion artifacts, reviewed for one long swim before a waypoint turn without a speed reset.

- [ ] **Step 1: Strengthen the purposeful-locomotion fixture contract**

Keep the current public review timeline and privacy-safe artifacts; do not add a
renderer or artifact system. Add assertions that the waypoint, quarter, half,
three-quarter, and end samples remain in one segment, and that `turn-boundary`
is the next segment. Assert the facing change, when present, occurs only at
that boundary. The review acceptance must explicitly inspect gradual visible
speed changes through the boundary and confirm no waypoint stop-start.

```rust
assert_eq!(samples[0].locomotion.segment_index, samples[4].locomotion.segment_index);
assert_eq!(samples[5].locomotion.segment_index, samples[0].locomotion.segment_index + 1);
```

- [ ] **Step 2: Run focused Preview Lab tests**

Run:

```bash
cargo test --features dev-preview dev_preview::smooth::tests::purposeful_locomotion_fixture_covers_waypoint_turn_swim_and_depth
cargo test --features dev-preview --test dev_preview dev_preview_purposeful_locomotion
```

Expected: all purposeful-locomotion unit and export tests pass.

- [ ] **Step 3: Render deterministic animation artifacts**

Run:

```bash
cargo run --features dev-preview -- dev-preview --scenario animation --out target/glorp-preview-motion
```

Inspect `target/glorp-preview-motion/manifest.json` and the
`round-purposeful-locomotion` sidecars. Confirm one five-second segment supplies
the swim phases, the next segment supplies the turn boundary, screen speed
changes gradually through that boundary, and no awake waypoint reads as a stop
then restart.

- [ ] **Step 4: Run final focused verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib round::locomotion::tests
cargo test --lib round::motion::tests
cargo test --lib round::placement::tests
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview dev_preview_purposeful_locomotion
git diff --check
```

Expected: every command exits zero with no new warnings or formatting failures.

- [ ] **Step 5: Launch the optimized companion and verify the running process**

Run:

```bash
cargo xtask companion fresh
ps -axo pid,lstart,command | rg 'target/macos/Glorp\.app/.*/glorp-companion'
```

Expected: the release companion builds, opens from this worktree, and the process list shows the fresh `target/macos/Glorp.app` binary.

- [ ] **Step 6: Commit Preview Lab assertion updates**

Run:

```bash
git add src/dev_preview/smooth.rs tests/dev_preview.rs
git commit -m "test(companion): lock purposeful swim preview cadence"
```
