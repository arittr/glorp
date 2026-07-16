# Glorp Shallow Tank Depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recalibrate Glorp's companion depth cues so the pet still changes size and drives ordered parallax, but the scene reads as a one-to-two-foot-deep tank rather than a room-sized volume.

**Architecture:** Keep the existing normalized depth, motion waveform, lifecycle attenuation, and renderer paths. Pin the approved spatial tuning in renderer-neutral companion effects, let the existing Smooth and direct scene projections consume those values, and preserve all current validation and fallback behavior.

**Tech Stack:** Rust, existing Smooth and retained/direct companion scene contracts, Cargo unit/integration tests, Preview Lab deterministic artifacts.

## Global Constraints

- Normalized pet depth remains `[-1.0, 1.0]`; neutral remains exactly `0.0` and renders at `1.0x`.
- Pet scale is exactly `0.97x` far, `1.0x` neutral, and `1.035x` near.
- Maximum vertical perspective is `0.10` cells; far pet atmospheric opacity is `0.93`.
- Parallax multipliers are exactly `0.006`, `0.010`, `0.014`, and `0.022` for far, mid, behind-pet, and foreground.
- Parallax caps are exactly `0.25` cell horizontally and `0.15` cell vertically.
- Wall-shadow detachment is exactly `0.45-1.2` cells; existing shadow strength remains unchanged.
- Floor-projection band is `0.18-0.32` of bed height, horizontal radii are `0.085-0.11` of viewport width, vertical radii are `0.022-0.032` of viewport height, and alpha remains `165-235`.
- Normal, calm, asleep, and Reduce Motion behavior remain full, half, quarter, and neutral/zero respectively.
- Do not change wander geometry, depth timing, activity energy, scene Z ordering, clipping, lighting, renderer selection, pet art, props, HUD, or gauges.
- Do not add configuration, dependencies, compatibility shims, or new runtime branches.
- Smooth and direct companion-scene paths must share canonical parallax tuning.

---

## File Structure

| File | Responsibility in this change |
|---|---|
| `src/presentation/companion_effects.rs` | Canonical renderer-neutral parallax constants and wall/floor depth-effect geometry. |
| `src/round/depth.rs` | Piecewise pet scale, vertical perspective, and atmospheric projection. |
| `src/round/parallax.rs` | Smooth plane mapping, lifecycle attenuation, caps, and chrome-overlap safety. |
| `src/presentation/companion_scene/mod.rs` | Direct-scene authored depth plane to canonical parallax multiplier mapping. |
| `src/presentation/companion_scene/input.rs` | Direct-scene point projection with canonical per-axis cell caps. |
| `src/presentation/companion_scene/runtime.rs` | Regression fixtures that independently reconstruct direct-scene parallax. |
| `tests/smooth_companion.rs` | Cross-path depth, shadow, floor projection, lifecycle, and Smooth integration coverage. |

No file split or new module is needed.

---

### Task 1: Pin Renderer-Neutral Shallow Shadow and Floor Geometry

**Files:**
- Modify: `src/presentation/companion_effects.rs:6-105`
- Modify: `src/presentation/companion_effects.rs:363-452`
- Modify: `tests/smooth_companion.rs:955-1018`
- Modify: `tests/smooth_companion.rs:1396-1445`
- Verify: `tests/smooth_companion.rs:1524-1600`

**Interfaces:**
- Consumes: existing `effective_depth`, `wall_shadow_depth_cue`, and `floor_projection_metrics` contracts.
- Produces: `PARALLAX_FAR_MULTIPLIER`, `PARALLAX_MID_MULTIPLIER`, `PARALLAX_BEHIND_MULTIPLIER`, `PARALLAX_FOREGROUND_MULTIPLIER`, `PARALLAX_MAX_X_CELLS`, and `PARALLAX_MAX_Y_CELLS` as `pub(crate) const f32`; approved wall/floor endpoint geometry through the existing functions.

- [ ] **Step 1: Add exact failing effect-contract tests**

Add these tests at the start of `src/presentation/companion_effects.rs`'s existing `tests` module:

```rust
fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1.0e-5,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn shallow_tank_parallax_contract_is_exact_and_ordered() {
    assert_eq!(PARALLAX_FAR_MULTIPLIER, 0.006);
    assert_eq!(PARALLAX_MID_MULTIPLIER, 0.010);
    assert_eq!(PARALLAX_BEHIND_MULTIPLIER, 0.014);
    assert_eq!(PARALLAX_FOREGROUND_MULTIPLIER, 0.022);
    assert_eq!(PARALLAX_MAX_X_CELLS, 0.25);
    assert_eq!(PARALLAX_MAX_Y_CELLS, 0.15);
    assert!(PARALLAX_FAR_MULTIPLIER < PARALLAX_MID_MULTIPLIER);
    assert!(PARALLAX_MID_MULTIPLIER < PARALLAX_BEHIND_MULTIPLIER);
    assert!(PARALLAX_BEHIND_MULTIPLIER < PARALLAX_FOREGROUND_MULTIPLIER);
}

#[test]
fn shallow_tank_wall_and_floor_geometry_matches_approved_endpoints() {
    let far_wall = wall_shadow_depth_cue(-1.0);
    let near_wall = wall_shadow_depth_cue(1.0);
    assert_close(far_wall.detach_cells, 0.45);
    assert_close(near_wall.detach_cells, 1.2);
    assert_close(far_wall.strength, 1.0);
    assert_close(near_wall.strength, 0.6);

    let far_floor = floor_projection_metrics(100.0, 100.0, 20.0, 80.0, 50.0, -1.0)
        .expect("valid far floor projection");
    let near_floor = floor_projection_metrics(100.0, 100.0, 20.0, 80.0, 50.0, 1.0)
        .expect("valid near floor projection");
    assert_close(far_floor.center_y, 30.8);
    assert_close(near_floor.center_y, 39.2);
    assert_close(far_floor.radius_x, 8.5);
    assert_close(near_floor.radius_x, 11.0);
    assert_close(far_floor.radius_y, 2.2);
    assert_close(near_floor.radius_y, 3.2);
    assert_eq!(far_floor.alpha, 165);
    assert_eq!(near_floor.alpha, 235);
}
```

- [ ] **Step 2: Run the new tests and verify RED**

Run:

```bash
cargo test --lib presentation::companion_effects::tests::shallow_tank_ -- --nocapture
```

Expected: compilation fails because the six canonical parallax constants do not exist, or the endpoint assertions fail against the current deep geometry.

- [ ] **Step 3: Add the canonical parallax constants and approved effect geometry**

At the top of `src/presentation/companion_effects.rs`, after `WallShadowDepthCue`, add:

```rust
pub(crate) const PARALLAX_FAR_MULTIPLIER: f32 = 0.006;
pub(crate) const PARALLAX_MID_MULTIPLIER: f32 = 0.010;
pub(crate) const PARALLAX_BEHIND_MULTIPLIER: f32 = 0.014;
pub(crate) const PARALLAX_FOREGROUND_MULTIPLIER: f32 = 0.022;
pub(crate) const PARALLAX_MAX_X_CELLS: f32 = 0.25;
pub(crate) const PARALLAX_MAX_Y_CELLS: f32 = 0.15;
```

Replace the spatial effect constants with:

```rust
const WALL_SHADOW_DETACH_FAR: f32 = 0.45;
const WALL_SHADOW_DETACH_NEAR: f32 = 1.2;
const WALL_SHADOW_STRENGTH_FAR: f32 = 1.0;
const WALL_SHADOW_STRENGTH_NEAR: f32 = 0.6;

const PROJECTION_ALPHA_FAR: f32 = 165.0;
const PROJECTION_ALPHA_NEAR: f32 = 235.0;
const PROJECTION_BAND_FAR: f32 = 0.18;
const PROJECTION_BAND_NEAR: f32 = 0.32;
const PROJECTION_RADIUS_X_FAR: f32 = 0.085;
const PROJECTION_RADIUS_X_NEAR: f32 = 0.11;
const PROJECTION_RADIUS_Y_FAR: f32 = 0.022;
const PROJECTION_RADIUS_Y_NEAR: f32 = 0.032;
```

Use the named radius constants in `floor_projection_metrics`:

```rust
let radius_x = lerp(
    PROJECTION_RADIUS_X_FAR * width,
    PROJECTION_RADIUS_X_NEAR * width,
    depth01,
);
let radius_y = lerp(
    PROJECTION_RADIUS_Y_FAR * height,
    PROJECTION_RADIUS_Y_NEAR * height,
    depth01,
);
```

Do not change `wall_shadow_depth_cue` interpolation, floor alpha interpolation, invalid-input handling, or color constants.

- [ ] **Step 4: Update Smooth integration expectations for wall detachment**

In `tests/smooth_companion.rs`, update the integration formula and endpoint assertions:

```rust
let expected_detach_extra = 0.45 + (1.2 - 0.45) * depth01 - plan.pet.scale;
```

```rust
assert!((far_offset - 0.45).abs() < 1e-4, "got {far_offset}");
assert!((near_offset - 1.2).abs() < 1e-4, "got {near_offset}");
```

Keep the existing diagonal-detachment, strength, multiply-blend, alpha-ordering, bed-boundary, and invalid-geometry assertions unchanged.

- [ ] **Step 5: Run effect and Smooth shadow/floor tests and verify GREEN**

Run:

```bash
cargo test --lib presentation::companion_effects::tests::shallow_tank_ -- --nocapture
cargo test --test smooth_companion wall_shadow_detachment_and_strength_encode_wall_distance -- --exact
cargo test --test smooth_companion floor_projection_is_one_bed_anchored_ellipse_that_tracks_depth -- --exact
cargo test --test smooth_companion smooth_round_plan_floor_projection_stays_below_props_and_moves_pet_attached_layers -- --exact
```

Expected: all selected tests pass with the approved endpoints and unchanged shadow-strength/alpha behavior.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/presentation/companion_effects.rs tests/smooth_companion.rs
git commit -m "fix(companion): shallow tank shadow geometry"
```

---

### Task 2: Compress Pet Front-to-Back Projection

**Files:**
- Modify: `tests/smooth_companion.rs:347-384`
- Modify: `src/round/depth.rs:1-11`
- Verify: `tests/smooth_companion.rs:1284-1394`
- Verify: `tests/smooth_companion.rs:1781-1826`
- Verify: `src/presentation/companion_scene/input.rs:3669-3712`

**Interfaces:**
- Consumes: unchanged `resolve_smooth_depth(raw_z: f32, lifecycle_scale: f32) -> Result<SmoothDepthSample, SmoothDepthError>`.
- Produces: exact far/neutral/near scale `0.97/1.0/1.035`, maximum perspective `0.10`, and far atmosphere `0.93` for both Smooth and direct companion-scene input.

- [ ] **Step 1: Pin the approved pet-depth constants in the integration test**

At the start of `smooth_depth_resolver_maps_bounds_lifecycle_and_rejects_invalid_inputs` in `tests/smooth_companion.rs`, add:

```rust
assert_eq!(SMOOTH_PET_FAR_SCALE, 0.97);
assert_eq!(SMOOTH_PET_NEAR_SCALE, 1.035);
assert_eq!(SMOOTH_PERSPECTIVE_Y_MAX, 0.10);
assert_eq!(SMOOTH_FAR_ATMOSPHERE, 0.93);
```

Replace the hard-coded near-scale assertion with:

```rust
assert_eq!(
    resolve_smooth_depth(1.0, 1.0).unwrap().scale,
    SMOOTH_PET_NEAR_SCALE
);
```

Keep all finite-input, lifecycle, bound, and monotonic assertions.

- [ ] **Step 2: Run the depth test and verify RED**

Run:

```bash
cargo test --test smooth_companion smooth_depth_resolver_maps_bounds_lifecycle_and_rejects_invalid_inputs -- --exact
```

Expected: FAIL because the current constants are `0.92`, `1.12`, `0.30`, and `0.82`.

- [ ] **Step 3: Implement the approved depth constants**

Replace the opening constants and stale commentary in `src/round/depth.rs` with:

```rust
/// The pet keeps a visible front-to-back excursion without reading as though it
/// crosses a room-sized volume. Shadow separation carries the remaining Z cue.
pub const SMOOTH_PET_FAR_SCALE: f32 = 0.97;
pub const SMOOTH_PET_NEAR_SCALE: f32 = 1.035;
pub const SMOOTH_PERSPECTIVE_Y_MAX: f32 = 0.10;

/// The far plane keeps most of its ink in the deliberately shallow tank; the
/// neutral and near planes remain fully present.
pub const SMOOTH_FAR_ATMOSPHERE: f32 = 0.93;
```

Do not change the piecewise neutral mapping, `effective_depth`, validation, or lifecycle scale.

- [ ] **Step 4: Run cross-path depth tests and verify GREEN**

Run:

```bash
cargo test --lib round::depth::tests -- --nocapture
cargo test --test smooth_companion smooth_depth_resolver_maps_bounds_lifecycle_and_rejects_invalid_inputs -- --exact
cargo test --test smooth_companion depth_transform_maps_far_neutral_and_near_onto_scale_and_perspective -- --exact
cargo test --test smooth_companion smooth_depth_resolves_atmospheric_attenuation_from_the_same_sample -- --exact
cargo test --features retained-renderer presentation::companion_scene::input::tests::frame_depth_parity_covers_far_neutral_and_near_fixtures -- --exact
```

Expected: all selected tests pass; neutral remains exactly `1.0`, and direct scene input matches the same resolver.

- [ ] **Step 5: Commit Task 2**

```bash
git add src/round/depth.rs tests/smooth_companion.rs
git commit -m "fix(companion): compress pet depth excursion"
```

---

### Task 3: Share and Enforce Shallow Parallax Across Renderer Paths

**Files:**
- Modify: `src/round/parallax.rs:1-63`
- Modify: `src/round/parallax.rs:294-418`
- Modify: `src/round/parallax.rs:689-736`
- Modify: `src/presentation/companion_scene/mod.rs:352-382`
- Modify: `src/presentation/companion_scene/input.rs:804-833`
- Modify: `src/presentation/companion_scene/input.rs:2484-2559`
- Modify: `src/presentation/companion_scene/runtime.rs:3064-3095`
- Modify: `src/presentation/companion_scene/runtime.rs:4278-4416`

**Interfaces:**
- Consumes: the six canonical `PARALLAX_*` constants created in Task 1.
- Produces: Smooth and direct scene paths with identical shared-plane multipliers, `0.25/0.15` cell caps, unchanged lifecycle attenuation, unchanged Smooth chrome safety, and unchanged Reduce Motion zeroing.

- [ ] **Step 1: Update Smooth resolver tests to require canonical tuning**

In `src/round/parallax.rs`'s tests, add:

```rust
#[test]
fn plane_mapping_uses_the_canonical_shallow_tank_contract() {
    use crate::presentation::companion_effects::{
        PARALLAX_BEHIND_MULTIPLIER, PARALLAX_FAR_MULTIPLIER,
        PARALLAX_FOREGROUND_MULTIPLIER, PARALLAX_MID_MULTIPLIER,
    };

    assert_eq!(plane_multiplier(SmoothDepthPlane::Far), PARALLAX_FAR_MULTIPLIER);
    assert_eq!(plane_multiplier(SmoothDepthPlane::Mid), PARALLAX_MID_MULTIPLIER);
    assert_eq!(
        plane_multiplier(SmoothDepthPlane::Behind),
        PARALLAX_BEHIND_MULTIPLIER
    );
    assert_eq!(
        plane_multiplier(SmoothDepthPlane::Foreground),
        PARALLAX_FOREGROUND_MULTIPLIER
    );
}
```

Change the cap assertion in `raw_plane_delta_caps_axes_independently` to canonical constants:

```rust
assert_eq!(
    delta.x,
    crate::presentation::companion_effects::PARALLAX_MAX_X_CELLS
);
assert_eq!(
    delta.y,
    -crate::presentation::companion_effects::PARALLAX_MAX_Y_CELLS
);
```

Update the two exact safety expectations affected by the smaller multipliers/cap:

```rust
SmoothPoint { x: 0.056, y: 0.0 }
```

for a four-cell Behind displacement, and:

```rust
SmoothPoint { x: -0.25, y: 0.0 }
```

for the large negative Foreground displacement.

- [ ] **Step 2: Update direct-scene tests to require approved multipliers and axis caps**

Rename `depth_parallax_is_bounded_to_half_a_cell` in `src/presentation/companion_scene/input.rs` to `depth_parallax_uses_canonical_multipliers_and_axis_caps` and replace its expected values with:

```rust
[0.48, -1.2]  // Background: 10 cells * 0.006 * [8, 20]
[1.12, -2.8]  // Behind:    10 cells * 0.014 * [8, 20]
[1.76, -3.0]  // Foreground X is raw; Y reaches the 0.15-cell cap
[2.0, -3.0]   // Large displacement reaches 0.25/0.15-cell axis caps
```

In `src/presentation/companion_scene/runtime.rs`, change `expected_depth_parallax` to clamp each axis using:

```rust
let cap_cells = [
    crate::presentation::companion_effects::PARALLAX_MAX_X_CELLS,
    crate::presentation::companion_effects::PARALLAX_MAX_Y_CELLS,
];
std::array::from_fn(|axis| {
    let cap = grid.cell_extent_points[axis] * cap_cells[axis];
    (displacement[axis] * multiplier * grid.cell_extent_points[axis]).clamp(-cap, cap)
})
```

Replace the literal test inputs with the authored plane contract:

```rust
let depth_parallax = expected_depth_parallax(
    &previous,
    clock,
    AuthoredDepthSnapshot::Foreground.parallax_multiplier(),
);
```

```rust
let expected_parallax = expected_depth_parallax(
    &previous,
    clock,
    AuthoredDepthSnapshot::BehindPet.parallax_multiplier(),
);
```

- [ ] **Step 3: Run both resolver test groups and verify RED**

Run:

```bash
cargo test --lib round::parallax::tests -- --nocapture
cargo test --features retained-renderer presentation::companion_scene::input::tests::depth_parallax_uses_canonical_multipliers_and_axis_caps -- --exact
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::two_pose_semantic_rebase_keeps_exactly_one_foreground_parallax_offset -- --exact
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::tank_semantic_rebase_keeps_parallax_out_of_anchors_and_reduce_motion -- --exact
```

Expected: plane/cap assertions fail against the current duplicated `0.010/0.020/0.030/0.045` and `0.5/0.25` or half-cell contracts.

- [ ] **Step 4: Wire Smooth to the canonical constants**

Remove the six local multiplier/cap constants from `src/round/parallax.rs`. Import the canonical contract:

```rust
use crate::presentation::companion_effects::{
    PARALLAX_BEHIND_MULTIPLIER, PARALLAX_FAR_MULTIPLIER,
    PARALLAX_FOREGROUND_MULTIPLIER, PARALLAX_MAX_X_CELLS,
    PARALLAX_MAX_Y_CELLS, PARALLAX_MID_MULTIPLIER,
};
```

Map planes with those constants:

```rust
match plane {
    SmoothDepthPlane::Far => PARALLAX_FAR_MULTIPLIER,
    SmoothDepthPlane::Mid => PARALLAX_MID_MULTIPLIER,
    SmoothDepthPlane::Behind => PARALLAX_BEHIND_MULTIPLIER,
    SmoothDepthPlane::Foreground => PARALLAX_FOREGROUND_MULTIPLIER,
}
```

Clamp the two axes with `PARALLAX_MAX_X_CELLS` and `PARALLAX_MAX_Y_CELLS`. Keep `VERTICAL_AXIS_SCALE`, lifecycle validation, safety scales, and chrome-overlap resolution unchanged.

- [ ] **Step 5: Wire direct scene depth planes and caps to the same contract**

Replace `AuthoredDepthSnapshot::parallax_multiplier`'s literals in `src/presentation/companion_scene/mod.rs` with:

```rust
match self {
    Self::Background => crate::presentation::companion_effects::PARALLAX_FAR_MULTIPLIER,
    Self::BehindPet => crate::presentation::companion_effects::PARALLAX_BEHIND_MULTIPLIER,
    Self::Foreground => crate::presentation::companion_effects::PARALLAX_FOREGROUND_MULTIPLIER,
}
```

In `bounded_depth_parallax_points`, replace the half-cell clamp with axis-specific caps:

```rust
let cap_cells = [
    crate::presentation::companion_effects::PARALLAX_MAX_X_CELLS,
    crate::presentation::companion_effects::PARALLAX_MAX_Y_CELLS,
];
std::array::from_fn(|axis| {
    let cap = glyph_grid.cell_extent_points[axis] * cap_cells[axis];
    (displacement_cells[axis] * multiplier * glyph_grid.cell_extent_points[axis])
        .clamp(-cap, cap)
})
```

Keep Reduce Motion's early zero return and lifecycle clamping unchanged.

- [ ] **Step 6: Run parallax tests and verify GREEN**

Run:

```bash
cargo test --lib round::parallax::tests -- --nocapture
cargo test --features retained-renderer presentation::companion_scene::input::tests::depth_parallax_uses_canonical_multipliers_and_axis_caps -- --exact
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::two_pose_semantic_rebase_keeps_exactly_one_foreground_parallax_offset -- --exact
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::tank_semantic_rebase_keeps_parallax_out_of_anchors_and_reduce_motion -- --exact
```

Expected: all selected tests pass, shared planes match, each axis obeys its approved cap, and Reduce Motion remains zero.

- [ ] **Step 7: Commit Task 3**

```bash
git add src/round/parallax.rs src/presentation/companion_scene/mod.rs src/presentation/companion_scene/input.rs src/presentation/companion_scene/runtime.rs
git commit -m "fix(companion): unify shallow parallax tuning"
```

---

### Task 4: Run Proportional Regression and Deterministic Visual Verification

**Files:**
- Verify: all files modified in Tasks 1-3
- Generate ignored artifacts: `target/glorp-preview-shallow-depth-round/`
- Generate ignored artifacts: `target/glorp-preview-shallow-depth-animation/`

**Interfaces:**
- Consumes: completed shallow-depth implementation.
- Produces: test, lint, manifest, and deterministic visual evidence for the approved contract; no tracked source changes.

- [ ] **Step 1: Format and inspect the complete patch**

Run:

```bash
cargo fmt --all
git diff --check
git status --short
git diff --stat
```

Expected: formatting succeeds, `git diff --check` is clean, and only the intended source/test files are modified beyond already committed task commits.

- [ ] **Step 2: Run the focused depth and parallax suites**

Run:

```bash
cargo test --lib round::depth::tests -- --nocapture
cargo test --lib round::parallax::tests -- --nocapture
cargo test --features retained-renderer presentation::companion_effects::tests::shallow_tank_ -- --nocapture
cargo test --features retained-renderer presentation::companion_scene::input::tests
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::two_pose_semantic_rebase_keeps_exactly_one_foreground_parallax_offset -- --exact
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::tank_semantic_rebase_keeps_parallax_out_of_anchors_and_reduce_motion -- --exact
cargo test --test smooth_companion
```

Expected: all selected suites pass with clean output.

- [ ] **Step 3: Run renderer and Preview Lab regression tests**

Run:

```bash
cargo test --test round_scene
cargo test --features retained-renderer --test retained_scene
cargo test --features dev-preview --test dev_preview
```

Expected: all integration suites pass.

- [ ] **Step 4: Generate deterministic round and animation artifacts**

Run:

```bash
cargo run -- dev-preview --scenario round --out target/glorp-preview-shallow-depth-round
cargo run -- dev-preview --scenario animation --out target/glorp-preview-shallow-depth-animation
```

Expected: both commands succeed without reading real pet state. Each output contains `.glorp-preview`, `index.html`, `review.md`, and `manifest.json`; the animation output also contains scene-moment strips.

- [ ] **Step 5: Inspect manifest contracts and depth evidence without opening fullscreen UI**

Run:

```bash
jq '{schema_version, scenario_count: (.scenarios | length), strip_count: (.strips | length)}' target/glorp-preview-shallow-depth-round/manifest.json
jq '{schema_version, scenario_count: (.scenarios | length), strip_count: (.strips | length)}' target/glorp-preview-shallow-depth-animation/manifest.json
rg -n 'pet_depth|pet_depth_cue|parallax|0\.97|1\.035|0\.93' target/glorp-preview-shallow-depth-round target/glorp-preview-shallow-depth-animation
```

Expected: both manifests use schema version 3; the round bundle contains round scenarios, the animation bundle contains scene-moment strips, and typed artifacts expose the approved depth values where those fields are serialized. Do not invoke `open`, fullscreen, or focus-taking UI automation.

- [ ] **Step 6: Run final static checks**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
git status --short --branch
```

Expected: formatting, Clippy, and diff checks pass. The worktree contains no uncommitted source changes; only ignored Preview Lab artifacts may remain.

- [ ] **Step 7: Inspect the task commits and final branch diff**

Run:

```bash
git log --oneline --decorate -5
git diff main...HEAD --stat
git diff main...HEAD -- src/presentation/companion_effects.rs src/round/depth.rs src/round/parallax.rs src/presentation/companion_scene/mod.rs src/presentation/companion_scene/input.rs src/presentation/companion_scene/runtime.rs tests/smooth_companion.rs
```

Expected: the branch contains the plan plus three intentional implementation commits, with no unrelated source changes introduced by this work.
