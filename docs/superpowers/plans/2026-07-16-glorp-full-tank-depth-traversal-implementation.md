# Glorp Full-Tank Depth Traversal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the companion pet visibly traverse the complete rear-to-front tank envelope while retaining shallow depth cues, aperture safety, renderer parity, and a front-glass HUD.

**Architecture:** Keep `round::motion` as the deterministic source of raw planar and Z samples, then add one pure `round::placement` resolver that combines effective depth, tapered local Y wander, shallow perspective, and aperture-safe bounds. Smooth and retained/direct scene preparation consume that same result; the HUD gains an explicit front-glass semantic without adding a middle-plane compositor in this slice.

**Tech Stack:** Rust, AppKit Smooth renderer, retained wgpu companion scene, existing round scene contracts, Cargo unit/integration tests, Preview Lab deterministic artifacts.

## Global Constraints

- Effective pet depth remains finite and normalized to `[-1.0, 1.0]`.
- Rear, neutral, and front pet-center targets are approximately `27%`, `50%`, and `73%` of aperture height.
- Far, neutral, and near scale remain exactly `0.97x`, `1.0x`, and `1.035x`; maximum shallow perspective remains `0.10` cell.
- Local Y wander is multiplied by `1 - abs(effective_z)` and is exactly zero at both depth endpoints.
- Existing deterministic X roam remains, but its visible center may clamp inward as required to keep maximum-scale pet corners inside the circular aperture.
- HUD-reserved rows continue to constrain habitat content; they do not constrain pet depth placement.
- Awake active and awake calm pets use full depth; asleep pets use quarter depth; Reduce Motion resolves neutral depth and zero animated parallax.
- Object parallax remains driven by planar displacement and its independent lifecycle attenuation.
- The HUD plane is `FrontGlass` and renders after pet-attached content in both non-Classic paths.
- Classic `classic_top_left_cells`, pet rect, and draw-list behavior remain unchanged.
- Invalid placement geometry fails through existing scene-generation or last-good-frame paths.
- Do not add a user setting, saved state, compatibility shim, dependency, pet-art change, HUD redesign, or middle-plane compositor.

---

## File Structure

| File | Responsibility in this change |
|---|---|
| `src/round/placement.rs` | New pure rear/neutral/front mapping, local-Y taper, aperture clamp, transformed bounds, and validation. |
| `src/round/mod.rs` | Export the new round placement module. |
| `src/round/motion.rs` | Preserve raw planar displacement on the motion projection without changing Classic placement. |
| `src/round/scene.rs` | Carry the raw motion projection into fallible Smooth preparation while preserving Classic/raw planar placement. |
| `src/round/smooth.rs` | Apply the shared placement to every pet-attached Smooth layer and publish its clearance bounds. |
| `src/presentation/smooth.rs` | Keep Smooth plan comments and pet-placement evidence aligned with the new aperture/HUD contract. |
| `src/presentation/companion_effects.rs` | Preserve the already-started awake-depth lifecycle correction. |
| `src/round/parallax.rs` | Existing independent calm/asleep parallax lifecycle contract; verification only. |
| `src/presentation/companion_scene/input.rs` | Use shared placement for initial and tick-projected retained/direct anchors. |
| `src/presentation/companion_scene/runtime.rs` | Validate depth identity and aperture-safe final pet bounds. |
| `src/presentation/companion_scene/scene/compiler.rs` | Interpret the HUD plane as front-glass screen chrome after world/pet content. |
| `src/round/hud.rs` | Define the small renderer-neutral HUD depth-plane semantic. |
| `src/companion/app.rs` | Carry and exhaustively interpret the HUD plane in prepared AppKit frames. |
| `src/dev_preview/smooth.rs` | Add deterministic production-grid far, neutral, and front review frames. |
| `src/dev_preview/scenarios.rs` | Include the endpoint frames in both `smooth` and `round` Preview Lab selections. |
| `tests/smooth_companion.rs` | Endpoint, aperture, lifecycle, Classic, Smooth, and cross-renderer regressions. |
| `tests/companion_draw_boundary.rs` | Prove AppKit paints front-glass HUD after the prepared renderer payload. |
| `tests/dev_preview.rs` | Prove the three endpoint artifacts are exported with meaningful vertical separation. |

The new placement file is intentional: `motion.rs` remains about sampling and Classic-compatible motion, while `placement.rs` owns the physically coupled tank geometry.

---

### Task 1: Finish the Awake Depth and Independent Parallax Checkpoint

**Current-worktree note:** This task is already implemented but unstaged in the five files below. Preserve those edits, verify them, and commit them as the first implementation checkpoint. Do not recreate or discard them.

**Files:**
- Modify: `src/presentation/companion_effects.rs:21-31`
- Modify: `src/presentation/companion_scene/input.rs:240-255`
- Modify: `src/presentation/companion_scene/input.rs:414-429`
- Modify: `src/presentation/companion_scene/input.rs:3218-3262`
- Modify: `src/presentation/companion_scene/runtime.rs:4915-5020`
- Modify: `src/round/motion.rs:268-278`
- Modify: `tests/smooth_companion.rs:348-385`
- Modify: `tests/smooth_companion.rs:1400-1423`
- Modify: `tests/smooth_companion.rs:1808-1825`
- Modify: `tests/smooth_companion.rs:1290-1305`

**Interfaces:**
- Consumes: `depth_lifecycle_scale(asleep: bool, calm: bool) -> f32` and `parallax_lifecycle_scale(asleep: bool, calm: bool) -> f32`.
- Produces: full effective depth for every awake pet, quarter depth for sleep, and independently softened calm parallax.

- [ ] **Step 1: Inspect the existing scoped diff**

Run:

```bash
git diff -- src/presentation/companion_effects.rs src/presentation/companion_scene/input.rs src/presentation/companion_scene/runtime.rs src/round/motion.rs tests/smooth_companion.rs
```

Expected: only the awake depth lifecycle, direct-scene parallax lifecycle, snapshot-identity tests, motion commentary, and associated regressions differ from `HEAD`.

- [ ] **Step 2: Verify the targeted lifecycle tests**

Run:

```bash
cargo test --test smooth_companion smooth_depth_resolver_maps_bounds_lifecycle_and_rejects_invalid_inputs -- --exact
cargo test --test smooth_companion awake_calm_depth_reaches_the_same_tank_endpoints_as_active -- --exact
cargo test --features retained-renderer presentation::companion_scene::input::tests::tank_animation_states_are_bounded_identity_stable_and_calm_aware -- --exact
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::resolved_depth_and_sleep_are_canonical_snapshot_identity -- --exact
```

Expected: all selected tests pass; calm awake depth equals active depth, while calm parallax still uses `parallax_lifecycle_scale`.

- [ ] **Step 3: Confirm the implemented split is exact**

The lifecycle functions must read:

```rust
pub(crate) const fn depth_lifecycle_scale(asleep: bool, _calm: bool) -> f32 {
    if asleep { 0.25 } else { 1.0 }
}
```

```rust
let parallax = DepthParallaxContext {
    motion,
    glyph_grid,
    lifecycle_motion_scale: crate::round::parallax::parallax_lifecycle_scale(asleep, calm),
    reduce_motion: options.reduce_motion,
};
```

The same `parallax_lifecycle_scale` call must be present in presentation-tick reprojection.

Update the stale fixture comment above `normal_lifecycle_fixture` to match the new lifecycle contract:

```rust
/// Awake pets use the full depth envelope; sleep is the only lifecycle depth
/// attenuation. These fixtures keep endpoint expectations explicit.
```

- [ ] **Step 4: Commit Task 1**

```bash
git add src/presentation/companion_effects.rs src/presentation/companion_scene/input.rs src/presentation/companion_scene/runtime.rs src/round/motion.rs tests/smooth_companion.rs
git diff --cached --check
git commit -m "fix(companion): let awake pets traverse full depth"
```

---

### Task 2: Add the Pure Full-Tank Placement Resolver

**Files:**
- Create: `src/round/placement.rs`
- Modify: `src/round/mod.rs:1-12`
- Modify: `src/round/motion.rs:74-90`
- Modify: `src/round/motion.rs:173-230`
- Modify: `src/presentation/companion_scene/input.rs:1490-1515`
- Modify: `src/presentation/companion_scene/input.rs:2495-2565`

**Interfaces:**
- Consumes: `RoundCompanionMotionProjection`, `SmoothDepthSample`, and `RoundCompanionMotionViewport`.
- Produces: `resolve_round_depth_placement(motion, depth, viewport) -> Result<RoundDepthPlacement, RoundDepthPlacementError>`.
- Produces: `RoundDepthPlacement { anchor_top_left_cells, anchor_top_left_points, final_center_cells, max_scale_bounds_cells, tapered_local_y_cells }`.
- Produces: `RoundCompanionMotionProjection::planar_offset_cells: MotionPoint` for unclipped local motion evidence.

- [ ] **Step 1: Preserve raw planar displacement in the motion projection**

Add this field to `RoundCompanionMotionProjection` in `src/round/motion.rs`:

```rust
/// Energy-scaled planar displacement before the non-Classic placement resolver.
/// Classic and parallax keep their existing clamped fields; tank depth uses this
/// value so the HUD reservation cannot erase local Y motion.
pub planar_offset_cells: MotionPoint,
```

Populate it in `project_round_companion_motion_from_offsets` beside the existing motion fields:

```rust
planar_offset_cells: MotionPoint { x: offset_x, y: offset_y },
```

Add `planar_offset_cells: MotionPoint { x: 0.0, y: 0.0 }` to the three explicit `RoundCompanionMotionProjection` fixtures in `src/presentation/companion_scene/input.rs`; use each fixture's authored displacement where the fixture tests parallax:

```rust
planar_offset_cells: crate::round::motion::MotionPoint { x: 10.0, y: -10.0 },
```

- [ ] **Step 2: Write the failing placement tests**

Create `src/round/placement.rs` with the public contract, errors, and tests first:

```rust
use crate::presentation::smooth::{SmoothBounds, SmoothPoint};
use crate::round::depth::SmoothDepthSample;
use crate::round::motion::{
    MotionPoint, RoundCompanionMotionProjection, RoundCompanionMotionViewport,
};

pub const ROUND_REAR_CENTER_Y_FRACTION: f32 = 0.27;
pub const ROUND_NEUTRAL_CENTER_Y_FRACTION: f32 = 0.50;
pub const ROUND_FRONT_CENTER_Y_FRACTION: f32 = 0.73;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundDepthPlacement {
    pub anchor_top_left_cells: MotionPoint,
    pub anchor_top_left_points: [f32; 2],
    pub final_center_cells: MotionPoint,
    pub max_scale_bounds_cells: SmoothBounds,
    pub tapered_local_y_cells: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundDepthPlacementError {
    EmptyViewport,
    NonFiniteInput,
    PetDoesNotFit,
    InvalidOutput,
}

impl std::fmt::Display for RoundDepthPlacementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyViewport => formatter.write_str("round depth placement viewport is empty"),
            Self::NonFiniteInput => formatter.write_str("round depth placement input is non-finite"),
            Self::PetDoesNotFit => formatter.write_str("maximum-scale pet does not fit in aperture"),
            Self::InvalidOutput => formatter.write_str("round depth placement output is invalid"),
        }
    }
}

impl std::error::Error for RoundDepthPlacementError {}

pub fn resolve_round_depth_placement(
    motion: RoundCompanionMotionProjection,
    depth: SmoothDepthSample,
    viewport: RoundCompanionMotionViewport,
) -> Result<RoundDepthPlacement, RoundDepthPlacementError> {
    resolve_round_depth_placement_impl(motion, depth, viewport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::depth::resolve_smooth_depth;
    use crate::round::motion::CompanionMotionClearance;

    fn viewport(bottom_reserved_rows: u16) -> RoundCompanionMotionViewport {
        RoundCompanionMotionViewport {
            grid_columns: 44,
            grid_rows: 18,
            width_points: 440.0,
            height_points: 360.0,
            clearance: CompanionMotionClearance {
                near_scale: crate::round::depth::SMOOTH_PET_NEAR_SCALE,
                perspective_y_max: crate::round::depth::SMOOTH_PERSPECTIVE_Y_MAX,
                bottom_reserved_rows,
            },
        }
    }

    fn motion(planar_x: f32, planar_y: f32, depth: f32) -> RoundCompanionMotionProjection {
        RoundCompanionMotionProjection {
            motion_top_left_cells: MotionPoint {
                x: 15.5 + planar_x,
                y: 4.0 + planar_y,
            },
            motion_origin_top_left_cells: MotionPoint { x: 15.5, y: 4.0 },
            motion_top_left_points: [155.0 + planar_x * 10.0, 80.0 + planar_y * 20.0],
            planar_offset_cells: MotionPoint { x: planar_x, y: planar_y },
            classic_top_left_cells: [15, 4],
            normalized_depth: depth,
            facing: 1,
            wander_offset_x: 0,
            breath_offset_y_cells: 0,
            bob_offset_y_cells: 0.0,
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1.0e-4, "expected {expected}, got {actual}");
    }

    #[test]
    fn endpoints_map_to_rear_neutral_and_front_and_zero_local_y() {
        for (raw_z, expected_fraction) in [
            (-1.0, ROUND_REAR_CENTER_Y_FRACTION),
            (0.0, ROUND_NEUTRAL_CENTER_Y_FRACTION),
            (1.0, ROUND_FRONT_CENTER_Y_FRACTION),
        ] {
            let depth = resolve_smooth_depth(raw_z, 1.0).unwrap();
            let placement = resolve_round_depth_placement(
                motion(0.0, if raw_z == 0.0 { 0.0 } else { 2.0 }, raw_z),
                depth,
                viewport(5),
            )
            .unwrap();
            assert_close(placement.final_center_cells.y / 18.0, expected_fraction);
            if raw_z.abs() == 1.0 {
                assert_eq!(placement.tapered_local_y_cells, 0.0);
            }
        }
    }

    #[test]
    fn local_y_tapers_continuously_and_hud_reservation_is_not_an_input() {
        for raw_z in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            let depth = resolve_smooth_depth(raw_z, 1.0).unwrap();
            let placement = resolve_round_depth_placement(
                motion(0.0, 2.0, raw_z),
                depth,
                viewport(5),
            )
            .unwrap();
            assert_close(
                placement.tapered_local_y_cells,
                2.0 * (1.0 - raw_z.abs()),
            );
        }

        let depth = resolve_smooth_depth(0.5, 1.0).unwrap();
        let reserved = resolve_round_depth_placement(motion(0.0, 2.0, 0.5), depth, viewport(5))
            .unwrap();
        let open = resolve_round_depth_placement(motion(0.0, 2.0, 0.5), depth, viewport(0))
            .unwrap();
        assert_close(reserved.tapered_local_y_cells, 1.0);
        assert_eq!(reserved, open);
    }

    #[test]
    fn maximum_scale_corners_stay_inside_the_elliptical_cell_aperture() {
        let neutral_depth = resolve_smooth_depth(0.0, 1.0).unwrap();
        let neutral = resolve_round_depth_placement(
            motion(2.0, 0.0, 0.0),
            neutral_depth,
            viewport(5),
        )
        .unwrap();
        assert_close(neutral.final_center_cells.x, 24.0);

        let depth = resolve_smooth_depth(1.0, 1.0).unwrap();
        let placement = resolve_round_depth_placement(motion(20.0, 4.0, 1.0), depth, viewport(5))
            .unwrap();
        assert!(bounds_inside_round_aperture(
            placement.max_scale_bounds_cells,
            viewport(5)
        ));
        assert_close(placement.final_center_cells.y / 18.0, 0.73);
    }

    #[test]
    fn small_but_valid_apertures_compress_depth_symmetrically() {
        let mut small = viewport(0);
        small.grid_columns = 20;
        small.grid_rows = 12;
        small.width_points = 200.0;
        small.height_points = 240.0;
        let far = resolve_round_depth_placement(
            motion(0.0, 0.0, -1.0),
            resolve_smooth_depth(-1.0, 1.0).unwrap(),
            small,
        )
        .unwrap();
        let near = resolve_round_depth_placement(
            motion(0.0, 0.0, 1.0),
            resolve_smooth_depth(1.0, 1.0).unwrap(),
            small,
        )
        .unwrap();
        assert_close(far.final_center_cells.y + near.final_center_cells.y, 12.0);
        assert!(far.final_center_cells.y > 12.0 * ROUND_REAR_CENTER_Y_FRACTION);
        assert!(near.final_center_cells.y < 12.0 * ROUND_FRONT_CENTER_Y_FRACTION);
    }

    #[test]
    fn perspective_is_preaccounted_in_the_anchor_and_invalid_geometry_fails() {
        let depth = resolve_smooth_depth(1.0, 1.0).unwrap();
        let placement = resolve_round_depth_placement(motion(0.0, 0.0, 1.0), depth, viewport(5))
            .unwrap();
        let rendered_center_y = placement.anchor_top_left_cells.y
            + crate::pet::render::FRAME_HEIGHT as f32 / 2.0
            + depth.perspective_y;
        assert_close(rendered_center_y, placement.final_center_cells.y);

        let mut nonfinite = viewport(5);
        nonfinite.width_points = f32::NAN;
        assert_eq!(
            resolve_round_depth_placement(motion(0.0, 0.0, 0.0), depth, nonfinite),
            Err(RoundDepthPlacementError::NonFiniteInput)
        );
        let mut empty = viewport(5);
        empty.grid_columns = 0;
        assert_eq!(
            resolve_round_depth_placement(motion(0.0, 0.0, 0.0), depth, empty),
            Err(RoundDepthPlacementError::EmptyViewport)
        );
        let mut too_small = viewport(0);
        too_small.grid_columns = 4;
        too_small.grid_rows = 4;
        assert_eq!(
            resolve_round_depth_placement(motion(0.0, 0.0, 0.0), depth, too_small),
            Err(RoundDepthPlacementError::PetDoesNotFit)
        );
    }
}
```

The tests intentionally call an undefined `resolve_round_depth_placement_impl` and `bounds_inside_round_aperture` during the RED step.

- [ ] **Step 3: Run the new tests and verify RED**

Run:

```bash
cargo test --lib round::placement::tests -- --nocapture
```

Expected: compilation fails because the placement implementation helpers are not defined.

- [ ] **Step 4: Implement the resolver**

Add this implementation above the test module in `src/round/placement.rs`:

```rust
fn resolve_round_depth_placement_impl(
    motion: RoundCompanionMotionProjection,
    depth: SmoothDepthSample,
    viewport: RoundCompanionMotionViewport,
) -> Result<RoundDepthPlacement, RoundDepthPlacementError> {
    if viewport.grid_columns == 0 || viewport.grid_rows == 0 {
        return Err(RoundDepthPlacementError::EmptyViewport);
    }
    let scalar_inputs = [
        viewport.width_points,
        viewport.height_points,
        motion.motion_top_left_cells.x,
        motion.motion_top_left_cells.y,
        motion.planar_offset_cells.x,
        motion.planar_offset_cells.y,
        depth.raw_z,
        depth.effective_z,
        depth.scale,
        depth.perspective_y,
        depth.atmosphere,
    ];
    if scalar_inputs.into_iter().any(|value| !value.is_finite())
        || viewport.width_points <= 0.0
        || viewport.height_points <= 0.0
        || !(-1.0..=1.0).contains(&depth.effective_z)
    {
        return Err(RoundDepthPlacementError::NonFiniteInput);
    }

    let aperture_center = SmoothPoint {
        x: f32::from(viewport.grid_columns) / 2.0,
        y: f32::from(viewport.grid_rows) / 2.0,
    };
    let aperture_radii = aperture_center;
    let half_ink = SmoothPoint {
        x: crate::pet::render::ART_WIDTH as f32 / 2.0
            * crate::round::depth::SMOOTH_PET_NEAR_SCALE,
        y: crate::pet::render::ART_HEIGHT as f32 / 2.0
            * crate::round::depth::SMOOTH_PET_NEAR_SCALE,
    };
    if half_ink.x >= aperture_radii.x || half_ink.y >= aperture_radii.y {
        return Err(RoundDepthPlacementError::PetDoesNotFit);
    }

    let taper = 1.0 - depth.effective_z.abs();
    let tapered_local_y_cells = motion.planar_offset_cells.y * taper;
    let target_fraction = if depth.effective_z >= 0.0 {
        ROUND_NEUTRAL_CENTER_Y_FRACTION
            + depth.effective_z
                * (ROUND_FRONT_CENTER_Y_FRACTION - ROUND_NEUTRAL_CENTER_Y_FRACTION)
    } else {
        ROUND_NEUTRAL_CENTER_Y_FRACTION
            + depth.effective_z
                * (ROUND_NEUTRAL_CENTER_Y_FRACTION - ROUND_REAR_CENTER_Y_FRACTION)
    };
    let requested_y = f32::from(viewport.grid_rows) * target_fraction + tapered_local_y_cells;

    let centered_x_ratio = half_ink.x / aperture_radii.x;
    let max_center_dy = aperture_radii.y * (1.0 - centered_x_ratio.powi(2)).sqrt()
        - half_ink.y;
    if !max_center_dy.is_finite() || max_center_dy < 0.0 {
        return Err(RoundDepthPlacementError::PetDoesNotFit);
    }
    let center_y = requested_y.clamp(
        aperture_center.y - max_center_dy,
        aperture_center.y + max_center_dy,
    );

    let vertical_corner_ratio =
        ((center_y - aperture_center.y).abs() + half_ink.y) / aperture_radii.y;
    let max_center_dx = aperture_radii.x
        * (1.0 - vertical_corner_ratio.clamp(0.0, 1.0).powi(2)).sqrt()
        - half_ink.x;
    if !max_center_dx.is_finite() || max_center_dx < 0.0 {
        return Err(RoundDepthPlacementError::PetDoesNotFit);
    }
    let requested_x = motion.motion_top_left_cells.x
        + crate::pet::render::FRAME_WIDTH as f32 / 2.0;
    let center_x = requested_x.clamp(
        aperture_center.x - max_center_dx,
        aperture_center.x + max_center_dx,
    );
    let final_center_cells = MotionPoint { x: center_x, y: center_y };
    let max_scale_bounds_cells = SmoothBounds {
        min: SmoothPoint {
            x: center_x - half_ink.x,
            y: center_y - half_ink.y,
        },
        max: SmoothPoint {
            x: center_x + half_ink.x,
            y: center_y + half_ink.y,
        },
    };
    let anchor_top_left_cells = MotionPoint {
        x: center_x - crate::pet::render::FRAME_WIDTH as f32 / 2.0,
        y: center_y
            - crate::pet::render::FRAME_HEIGHT as f32 / 2.0
            - depth.perspective_y,
    };
    let anchor_top_left_points = [
        anchor_top_left_cells.x * viewport.width_points / f32::from(viewport.grid_columns),
        anchor_top_left_cells.y * viewport.height_points / f32::from(viewport.grid_rows),
    ];
    let placement = RoundDepthPlacement {
        anchor_top_left_cells,
        anchor_top_left_points,
        final_center_cells,
        max_scale_bounds_cells,
        tapered_local_y_cells,
    };
    if !bounds_inside_round_aperture(max_scale_bounds_cells, viewport)
        || [
            placement.anchor_top_left_cells.x,
            placement.anchor_top_left_cells.y,
            placement.anchor_top_left_points[0],
            placement.anchor_top_left_points[1],
            placement.final_center_cells.x,
            placement.final_center_cells.y,
            placement.tapered_local_y_cells,
        ]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(RoundDepthPlacementError::InvalidOutput);
    }
    Ok(placement)
}

pub(crate) fn bounds_inside_round_aperture(
    bounds: SmoothBounds,
    viewport: RoundCompanionMotionViewport,
) -> bool {
    if viewport.grid_columns == 0 || viewport.grid_rows == 0 {
        return false;
    }
    let center = SmoothPoint {
        x: f32::from(viewport.grid_columns) / 2.0,
        y: f32::from(viewport.grid_rows) / 2.0,
    };
    let radii = center;
    if [
        bounds.min.x,
        bounds.min.y,
        bounds.max.x,
        bounds.max.y,
        radii.x,
        radii.y,
    ]
    .into_iter()
    .any(|value| !value.is_finite())
        || radii.x <= 0.0
        || radii.y <= 0.0
        || bounds.min.x > bounds.max.x
        || bounds.min.y > bounds.max.y
    {
        return false;
    }
    [
        bounds.min,
        SmoothPoint { x: bounds.max.x, y: bounds.min.y },
        SmoothPoint { x: bounds.min.x, y: bounds.max.y },
        bounds.max,
    ]
    .into_iter()
    .all(|corner| {
        let x = (corner.x - center.x) / radii.x;
        let y = (corner.y - center.y) / radii.y;
        x * x + y * y <= 1.0 + 1.0e-4
    })
}

#[doc(hidden)]
pub fn bounds_inside_round_aperture_for_test(
    bounds: SmoothBounds,
    viewport: RoundCompanionMotionViewport,
) -> bool {
    bounds_inside_round_aperture(bounds, viewport)
}
```

Export the module in `src/round/mod.rs`:

```rust
pub mod placement;
```

- [ ] **Step 5: Run motion and placement tests and verify GREEN**

Run:

```bash
cargo test --lib round::placement::tests -- --nocapture
cargo test --test smooth_companion smooth_depth_motion_is_deterministic_continuous_bounded_and_separately_salted -- --exact
cargo test --lib round::scene::tests -- --nocapture
```

Expected: placement tests pass; raw motion remains deterministic; Classic projection remains unchanged.

- [ ] **Step 6: Commit Task 2**

```bash
git add src/round/placement.rs src/round/mod.rs src/round/motion.rs src/presentation/companion_scene/input.rs
git diff --cached --check
git commit -m "feat(companion): resolve full-tank depth placement"
```

---

### Task 3: Apply Shared Placement to the Smooth Scene

**Files:**
- Modify: `src/round/scene.rs:40-205`
- Modify: `src/round/smooth.rs:30-175`
- Modify: `src/round/smooth.rs:195-430`
- Modify: `src/round/smooth.rs:485-510`
- Modify: `src/presentation/smooth.rs:600-645`
- Modify: `src/companion/app.rs:500-540`
- Modify: `tests/smooth_companion.rs:480-545`
- Modify: `tests/smooth_companion.rs:1350-1425`
- Modify: `tests/smooth_companion.rs:1655-1705`

**Interfaces:**
- Consumes: Task 2's `resolve_round_depth_placement` and `RoundDepthPlacement`.
- Produces: Smooth pet `base_anchor` from `anchor_top_left_cells`, transformed pet bounds centered on `final_center_cells` plus idle bob, and `max_scale_clearance` from the shared result.
- Preserves: `CompanionPetPlacement`'s raw planar fields, `CompanionPetPlacement::classic_rect`, and `RoundCompanionMotionProjection::classic_top_left_cells`.

- [ ] **Step 1: Write the failing Smooth endpoint regression**

Add to `tests/smooth_companion.rs` after `depth_transform_maps_far_neutral_and_near_onto_scale_and_perspective`:

```rust
fn depth_center_without_bob(
    plan: &glorp::presentation::smooth::SmoothCompanionScenePlan,
) -> f32 {
    center_y(plan.pet.transformed_bounds) - plan.pet.bob_offset.y
}

#[test]
fn depth_transform_reaches_the_full_rear_and_front_visual_envelope() {
    let vm = normal_lifecycle_fixture();
    let far = plan_at_depth(&vm, 0, -1.0);
    let neutral = plan_at_depth(&vm, 0, 0.0);
    let near = plan_at_depth(&vm, 0, 1.0);

    let fractions = [
        depth_center_without_bob(&far) / f32::from(GRID_ROWS),
        depth_center_without_bob(&neutral) / f32::from(GRID_ROWS),
        depth_center_without_bob(&near) / f32::from(GRID_ROWS),
    ];
    for (actual, expected) in fractions.into_iter().zip([0.27, 0.50, 0.73]) {
        assert!((actual - expected).abs() < 0.015, "expected {expected}, got {actual}");
    }
    assert!(depth_center_without_bob(&near) > f32::from(GRID_ROWS) / 2.0);
}
```

Replace the HUD-clearance assertion in `composed_plan_publishes_max_scale_clearance_inside_the_protected_regions` with corner checks against the ellipse:

```rust
let viewport = glorp::round::motion::RoundCompanionMotionViewport {
    grid_columns: GRID_COLS,
    grid_rows: GRID_ROWS,
    width_points: f32::from(GRID_COLS),
    height_points: f32::from(GRID_ROWS),
    clearance: glorp::round::scene::current_round_motion_clearance(GRID_ROWS),
};
assert!(glorp::round::placement::bounds_inside_round_aperture_for_test(
    clearance,
    viewport
));
```

Use the Task 2 `bounds_inside_round_aperture_for_test` wrapper so the integration test exercises the canonical aperture predicate.

- [ ] **Step 2: Run the endpoint regression and verify RED**

Run:

```bash
cargo test --test smooth_companion depth_transform_reaches_the_full_rear_and_front_visual_envelope -- --exact
```

Expected: FAIL because far and near differ by only the shallow `0.10`-cell perspective and the near center remains around the old upper-half roam envelope.

- [ ] **Step 3: Carry the raw motion projection through round scene preparation**

Add this crate-private field to `CompanionPetPlacement` in `src/round/scene.rs`:

```rust
pub(crate) motion_projection: RoundCompanionMotionProjection,
```

Store it in the existing `CompanionPetPlacement` initializer:

```rust
motion_projection: projection,
```

Keep `fractional_motion_top_left`, `fractional_motion_origin_top_left`, breath, and Classic calculations unchanged. They remain raw planar/Classic evidence for parallax and parity. The fallible Smooth builder in Step 4 is the first place that resolves depth placement, so invalid apertures return `SmoothScenePlanError` instead of panicking in shared Classic layout code.

- [ ] **Step 4: Resolve the Smooth override and use one placement everywhere**

Add an error variant in `src/round/smooth.rs`:

```rust
InvalidDepthPlacement(crate::round::placement::RoundDepthPlacementError),
```

Render it in `Display`:

```rust
SmoothScenePlanError::InvalidDepthPlacement(error) => {
    write!(f, "smooth scene depth placement: {error}")
}
```

After `resolve_smooth_depth`, resolve placement with the exact same depth sample, including Preview Lab overrides:

```rust
let depth_placement = crate::round::placement::resolve_round_depth_placement(
    placement.motion_projection,
    depth,
    crate::round::motion::RoundCompanionMotionViewport {
        grid_columns: grid_cols,
        grid_rows,
        width_points: f32::from(grid_cols),
        height_points: f32::from(grid_rows),
        clearance: crate::round::scene::current_round_motion_clearance(grid_rows),
    },
)
.map_err(SmoothScenePlanError::InvalidDepthPlacement)?;
let smooth_base_anchor = SmoothPoint {
    x: depth_placement.anchor_top_left_cells.x,
    y: depth_placement.anchor_top_left_cells.y,
};
```

Keep `perspective_offset` on each pet-attached transform, because the resolver has pre-accounted for that offset in the anchor. Replace the old `max_scale_clearance_bounds(roam_center)` call with:

```rust
let max_scale_clearance = depth_placement.max_scale_bounds_cells;
```

Remove the obsolete `max_scale_clearance_bounds` function. Update its comments in `src/presentation/smooth.rs` so `max_scale_clearance` means maximum-scale ink at the final resolved center, inside the aperture and allowed to overlap front-glass HUD bounds.

Map the new preparation error in `src/companion/app.rs`:

```rust
SmoothScenePlanError::InvalidDepthPlacement(_) => {
    CompanionFramePreparationError::SmoothInvalidDepth
}
```

- [ ] **Step 5: Replace HUD-reserve assertions with the aperture contract**

In `maximum_scale_smooth_placement_preserves_classic_and_protected_clearance`, keep the raw placement only for its Classic equality assertion and inspect the built Smooth plan for actual depth placement. Initialize the observed range before the loop:

```rust
let mut lowest_center_y = f32::INFINITY;
let mut highest_center_y = f32::NEG_INFINITY;
```

Inside the loop, build the plan and update the range from its canonical clearance:

```rust
let plan = glorp::round::smooth::try_build_round_smooth_scene_plan(
    &vm,
    now,
    GRID_COLS,
    GRID_ROWS,
    &motion,
    (step * 50) as u64,
)
.expect("production viewport resolves depth placement");
let clearance = plan.pet.max_scale_clearance;
let rendered_center_y = center_y(clearance);
lowest_center_y = lowest_center_y.min(rendered_center_y);
highest_center_y = highest_center_y.max(rendered_center_y);
let viewport = glorp::round::motion::RoundCompanionMotionViewport {
    grid_columns: GRID_COLS,
    grid_rows: GRID_ROWS,
    width_points: f32::from(GRID_COLS),
    height_points: f32::from(GRID_ROWS),
    clearance: glorp::round::scene::current_round_motion_clearance(GRID_ROWS),
};
assert!(glorp::round::placement::bounds_inside_round_aperture_for_test(
    clearance,
    viewport,
));
```

After the loop, assert:

```rust
assert!(lowest_center_y < f32::from(GRID_ROWS) * 0.35);
assert!(highest_center_y > f32::from(GRID_ROWS) * 0.65);
```

In `composed_plan_publishes_max_scale_clearance_inside_the_protected_regions`, rename the test to `composed_plan_publishes_max_scale_clearance_inside_the_aperture`, remove `hud_start`, and assert the new clearance height:

```rust
let expected_h = f32::from(PET_INK_H) * SMOOTH_PET_NEAR_SCALE;
assert!((clearance.max.y - clearance.min.y - expected_h).abs() < 1e-3);
```

Add an explicit near-plane overlap assertion:

```rust
let near = plan_at_depth(&vm, 0, 1.0);
let hud_start = GRID_ROWS
    - glorp::round::scene::round_tank_life_geometry(GRID_COLS, GRID_ROWS).reserved_regions[0]
        .height;
assert!(near.pet.max_scale_clearance.max.y > f32::from(hud_start));
```

Extend `awake_calm_depth_reaches_the_same_tank_endpoints_as_active` so it checks actual placement as well as scale and perspective:

```rust
assert!(
    (depth_center_without_bob(&calm_plan) - depth_center_without_bob(&active_plan)).abs()
        < 1.0e-4
);
```

- [ ] **Step 6: Run Smooth, Classic, and geometry tests and verify GREEN**

Run:

```bash
cargo test --test smooth_companion depth_transform_reaches_the_full_rear_and_front_visual_envelope -- --exact
cargo test --test smooth_companion maximum_scale_smooth_placement_preserves_classic_and_protected_clearance -- --exact
cargo test --test smooth_companion composed_plan_publishes_max_scale_clearance_inside_the_aperture -- --exact
cargo test --test smooth_companion awake_calm_depth_reaches_the_same_tank_endpoints_as_active -- --exact
cargo test --test round_scene
```

Expected: all selected tests pass; near center is below the viewport midpoint, HUD overlap is allowed, aperture corners remain safe, and Classic stays unchanged.

- [ ] **Step 7: Commit Task 3**

```bash
git add src/round/scene.rs src/round/smooth.rs src/presentation/smooth.rs src/companion/app.rs tests/smooth_companion.rs
git diff --cached --check
git commit -m "feat(companion): drive smooth placement from tank depth"
```

---

### Task 4: Apply the Same Placement to Retained/Direct Scene Frames

**Files:**
- Modify: `src/presentation/companion_scene/input.rs:205-335`
- Modify: `src/presentation/companion_scene/input.rs:350-425`
- Modify: `src/presentation/companion_scene/input.rs:3710-3845`
- Modify: `src/presentation/companion_scene/runtime.rs:710-900`
- Modify: `src/presentation/companion_scene/runtime.rs:3020-3100`
- Modify: `tests/smooth_companion.rs:1300-1435`

**Interfaces:**
- Consumes: Task 2's resolver and Task 3's Smooth output.
- Produces: `FrameSnapshot::pet_anchor_points` from `RoundDepthPlacement::anchor_top_left_points` on semantic builds and presentation ticks.
- Preserves: `FrameSnapshot::pet_depth_cue.y_offset_points_up` as the shallow perspective cue consumed by `pet_transform`.

- [ ] **Step 1: Make direct endpoint parity fail on the current anchor**

In `frame_depth_parity_covers_far_neutral_and_near_fixtures`, resolve the expected placement after the existing `resolve_smooth_depth` call:

```rust
let expected_placement = crate::round::placement::resolve_round_depth_placement(
    shared,
    expected,
    input.motion_viewport(),
)
.unwrap();
assert_eq!(
    snapshot.frame.pet_anchor_points,
    expected_placement.anchor_top_left_points
);
```

Delete the old assertion that compares the snapshot anchor directly to `shared.motion_top_left_points`.

- [ ] **Step 2: Run direct parity and verify RED**

Run:

```bash
cargo test --features retained-renderer presentation::companion_scene::input::tests::frame_depth_parity_covers_far_neutral_and_near_fixtures -- --exact
```

Expected: FAIL because `FrameSnapshot::pet_anchor_points` still carries the HUD-limited raw motion anchor.

- [ ] **Step 3: Resolve placement in both direct projection paths**

In `CompanionSceneSnapshot::project_with_options`, immediately after `resolve_smooth_depth`, add:

```rust
let depth_placement = crate::round::placement::resolve_round_depth_placement(
    motion,
    depth,
    input.motion_viewport(),
)
.map_err(|_| CompanionSceneProjectionError::InvalidDepthProjection)?;
```

Set the initial frame anchor to:

```rust
pet_anchor_points: depth_placement.anchor_top_left_points,
```

In `project_presentation_frame`, resolve the same placement after depth and set:

```rust
frame.pet_anchor_points = depth_placement.anchor_top_left_points;
```

Keep `motion` itself in `DepthParallaxContext`; object parallax must not consume depth placement.

Extend `initial_semantic_projection_applies_reduce_motion_before_first_frame` with exact neutral-depth assertions:

```rust
assert_eq!(initial.frame.pet_depth, 0.0);
assert_eq!(initial.frame.pet_depth_cue, DepthCue::NEUTRAL);
let cell_h = initial.topology.glyph_grid.cell_extent_points[1];
let reduced_center_y = initial.frame.pet_anchor_points[1] / cell_h
    + f32::from(PET_LATTICE_HEIGHT) / 2.0;
assert!((reduced_center_y / f32::from(input.grid_rows) - 0.5).abs() < 1.0e-4);
```

- [ ] **Step 4: Validate final maximum-scale bounds at the runtime boundary**

Add this helper in `src/presentation/companion_scene/runtime.rs` near `validate_snapshot`:

```rust
fn snapshot_pet_clearance(
    snapshot: &CompanionSceneSnapshot,
) -> crate::presentation::smooth::SmoothBounds {
    let cell = snapshot.topology.glyph_grid.cell_extent_points;
    let anchor_cells = crate::presentation::smooth::SmoothPoint {
        x: snapshot.frame.pet_anchor_points[0] / cell[0],
        y: snapshot.frame.pet_anchor_points[1] / cell[1],
    };
    let final_center = crate::presentation::smooth::SmoothPoint {
        x: anchor_cells.x + f32::from(PET_LATTICE_WIDTH) / 2.0,
        y: anchor_cells.y + f32::from(PET_LATTICE_HEIGHT) / 2.0
            - snapshot.frame.pet_depth_cue.y_offset_points_up / cell[1],
    };
    let half_w = crate::pet::render::ART_WIDTH as f32 / 2.0
        * crate::round::depth::SMOOTH_PET_NEAR_SCALE;
    let half_h = crate::pet::render::ART_HEIGHT as f32 / 2.0
        * crate::round::depth::SMOOTH_PET_NEAR_SCALE;
    crate::presentation::smooth::SmoothBounds {
        min: crate::presentation::smooth::SmoothPoint {
            x: final_center.x - half_w,
            y: final_center.y - half_h,
        },
        max: crate::presentation::smooth::SmoothPoint {
            x: final_center.x + half_w,
            y: final_center.y + half_h,
        },
    }
}
```

After validating `pet_depth_cue`, construct the snapshot viewport and reject clearance outside the aperture:

```rust
let motion_viewport = crate::round::motion::RoundCompanionMotionViewport {
    grid_columns: snapshot.topology.glyph_grid.columns,
    grid_rows: snapshot.topology.glyph_grid.rows,
    width_points: snapshot.topology.layout.width_points,
    height_points: snapshot.topology.layout.height_points,
    clearance: crate::round::scene::current_round_motion_clearance(
        snapshot.topology.glyph_grid.rows,
    ),
};
if !crate::round::placement::bounds_inside_round_aperture(
    snapshot_pet_clearance(snapshot),
    motion_viewport,
) {
    return Err(SnapshotRejection::InvalidValue);
}
```

Add this runtime test:

```rust
#[test]
fn snapshot_rejects_pet_clearance_outside_aperture() {
    let mut invalid = snapshot();
    invalid.frame.pet_anchor_points[1] += invalid.topology.layout.height_points;
    assert_eq!(validate_snapshot(&invalid), Err(SnapshotRejection::InvalidValue));
}
```

Update `frame_motion_parity_covers_active_and_asleep_calm_lifecycle` to resolve the active and sleeping expected placements instead of comparing anchors with raw motion:

```rust
let active_depth = crate::round::depth::resolve_smooth_depth(
    active_shared.normalized_depth,
    crate::round::depth::depth_lifecycle_scale(false, false),
)
.unwrap();
let resting_depth = crate::round::depth::resolve_smooth_depth(
    resting_shared.normalized_depth,
    crate::round::depth::depth_lifecycle_scale(true, true),
)
.unwrap();
let active_expected = crate::round::placement::resolve_round_depth_placement(
    active_shared,
    active_depth,
    input.motion_viewport(),
)
.unwrap();
let resting_expected = crate::round::placement::resolve_round_depth_placement(
    resting_shared,
    resting_depth,
    input.motion_viewport(),
)
.unwrap();
assert_eq!(active_snapshot.frame.pet_anchor_points, active_expected.anchor_top_left_points);
assert_eq!(
    resting_snapshot.frame.pet_anchor_points,
    resting_expected.anchor_top_left_points
);
```

- [ ] **Step 5: Add explicit Smooth/direct center parity**

Add this integration test to `tests/smooth_companion.rs`:

```rust
#[test]
fn smooth_and_direct_scene_share_depth_driven_pet_centers() {
    use glorp::presentation::companion_scene::{
        CompanionLogicalLayout, CompanionProjectionClock, CompanionSceneProjectionInput,
        CompanionSceneSnapshot,
    };

    let vm = normal_lifecycle_fixture();
    for depth in [-1.0, 0.0, 1.0] {
        let smooth = plan_at_depth(&vm, 0, depth);
        let input = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(DEPTH_NOW, 0),
            CompanionLogicalLayout::round(440.0, 360.0),
            GRID_COLS,
            GRID_ROWS,
            glorp::round::scene::current_round_motion_clearance(GRID_ROWS),
        )
        .with_depth_override(depth);
        let direct = CompanionSceneSnapshot::project_with_input(&vm, input).unwrap();
        let cell_h = direct.topology.glyph_grid.cell_extent_points[1];
        let direct_center_y = direct.frame.pet_anchor_points[1] / cell_h
            + f32::from(glorp::presentation::companion_scene::PET_LATTICE_HEIGHT) / 2.0
            - direct.frame.pet_depth_cue.y_offset_points_up / cell_h;
        assert!((depth_center_without_bob(&smooth) - direct_center_y).abs() < 1.0e-4);
    }
}
```

- [ ] **Step 6: Run retained/direct parity and runtime validation**

Run:

```bash
cargo test --features retained-renderer presentation::companion_scene::input::tests::frame_depth_parity_covers_far_neutral_and_near_fixtures -- --exact
cargo test --features retained-renderer presentation::companion_scene::input::tests::frame_motion_parity_covers_active_and_asleep_calm_lifecycle -- --exact
cargo test --features retained-renderer presentation::companion_scene::runtime::tests::snapshot_rejects_pet_clearance_outside_aperture -- --exact
cargo test --test smooth_companion smooth_and_direct_scene_share_depth_driven_pet_centers -- --exact
cargo test --test retained_scene
```

Expected: all selected tests pass; both render paths share center geometry and invalid direct frames fail before rendering.

- [ ] **Step 7: Commit Task 4**

```bash
git add src/presentation/companion_scene/input.rs src/presentation/companion_scene/runtime.rs tests/smooth_companion.rs
git diff --cached --check
git commit -m "feat(companion): share depth placement with retained scene"
```

---

### Task 5: Make the Front-Glass HUD Plane Explicit

**Files:**
- Modify: `src/round/hud.rs:1-30`
- Modify: `src/companion/app.rs:135-165`
- Modify: `src/companion/app.rs:620-640`
- Modify: `src/companion/app.rs:3230-3320`
- Modify: `src/presentation/companion_scene/scene/compiler.rs:1440-1490`
- Modify: `src/presentation/companion_scene/scene/compiler.rs:1880-1980`
- Modify: `tests/companion_draw_boundary.rs:1-110`

**Interfaces:**
- Produces: `CompanionHudDepthPlane::FrontGlass` and `COMPANION_HUD_DEPTH_PLANE` in `round::hud`.
- Consumes: the same semantic in AppKit frame preparation and retained scene-template compilation.
- Preserves: current HUD text, layout, privacy projection, dim ordering, and gauge rendering.

- [ ] **Step 1: Write failing semantic and ordering tests**

Add this contract to the top of `src/round/hud.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionHudDepthPlane {
    FrontGlass,
}

pub const COMPANION_HUD_DEPTH_PLANE: CompanionHudDepthPlane =
    CompanionHudDepthPlane::FrontGlass;
```

Add this compiler test:

```rust
#[test]
fn front_glass_hud_compiles_after_pet_as_screen_chrome() {
    let vm = crate::tui::view_model::WatchViewModel::fixture_with_habitat_props();
    let input = crate::presentation::companion_scene::CompanionSceneProjectionInput::round(
        crate::presentation::companion_scene::CompanionProjectionClock::new(
            time::OffsetDateTime::UNIX_EPOCH,
            0,
        ),
        crate::presentation::companion_scene::CompanionLogicalLayout::round(360.0, 360.0),
        44,
        18,
        crate::round::scene::current_round_motion_clearance(18),
    );
    let snapshot = crate::presentation::companion_scene::CompanionSceneSnapshot::project_with_input(
        &vm,
        input,
    )
    .unwrap();
    let template = build_template(&snapshot).unwrap();
    let hud = template
        .primitives
        .iter()
        .find(|primitive| {
            matches!(
                &primitive.binding,
                PrimitiveBinding::Instances(InstanceGroupBinding::Hud)
            )
        })
        .unwrap();
    let pet = template
        .primitives
        .iter()
        .find(|primitive| {
            matches!(
                &primitive.binding,
                PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body))
            )
        })
        .unwrap();

    assert_eq!(
        crate::round::hud::COMPANION_HUD_DEPTH_PLANE,
        crate::round::hud::CompanionHudDepthPlane::FrontGlass
    );
    assert!(hud.authored_order > pet.authored_order);
    assert_eq!(hud.depth, DepthBehavior::ScreenNoDepth);
    assert_eq!(hud.space, PrimitiveSpace::Screen);
}
```

Add a source-boundary test to `tests/companion_draw_boundary.rs`:

```rust
#[test]
fn appkit_front_glass_hud_is_painted_after_the_renderer_payload() {
    let source = std::fs::read_to_string("src/companion/app.rs").unwrap();
    let paint_start = source.find("fn paint_prepared_frame(").unwrap();
    let paint = &source[paint_start..];
    let renderer = paint.find("match &frame.renderer").unwrap();
    let hud_plane = paint.find("match frame.hud_plane").unwrap();
    let dim = paint.find("if dim_overlay").unwrap();
    assert!(renderer < hud_plane);
    assert!(hud_plane < dim);
}
```

- [ ] **Step 2: Run the ordering tests and verify RED**

Run:

```bash
cargo test --test companion_draw_boundary appkit_front_glass_hud_is_painted_after_the_renderer_payload -- --exact
```

Expected: FAIL because AppKit prepared frames do not yet carry `hud_plane`. The retained compiler test records already-correct front-glass ordering and runs after the shared-semantic refactor in Step 5.

- [ ] **Step 3: Carry and interpret the plane in AppKit preparation**

Add this field to `PreparedCompanionFrame`:

```rust
hud_plane: crate::round::hud::CompanionHudDepthPlane,
```

Set it in every `PreparedCompanionFrame` initializer:

```rust
hud_plane: crate::round::hud::COMPANION_HUD_DEPTH_PLANE,
```

Replace the direct `draw_hud` call in `paint_prepared_frame` with an exhaustive match at the same draw position:

```rust
match frame.hud_plane {
    crate::round::hud::CompanionHudDepthPlane::FrontGlass => {
        draw_hud(bounds, &aperture, hud_text, hud_font_size);
    }
}
```

Do not move the dim overlay; it continues to soften the HUD during rest.

- [ ] **Step 4: Interpret the same semantic in retained scene compilation**

Replace the unconditional `chrome.hud` push with:

```rust
match crate::round::hud::COMPANION_HUD_DEPTH_PLANE {
    crate::round::hud::CompanionHudDepthPlane::FrontGlass => push(
        "chrome.hud",
        PrimitiveKind::InstanceQuad,
        "material.screen-chrome",
        "resource.hud-glyph-atlas",
        WorldBlend::PremultipliedAlpha,
        DepthBehavior::ScreenNoDepth,
        PrimitiveBinding::Instances(InstanceGroupBinding::Hud),
        PrimitiveSpace::Screen,
    )?,
}
```

This is the only supported plane in this slice. A later middle-plane change adds a variant and compiler branch; it does not alter Task 2 placement.

- [ ] **Step 5: Run HUD, privacy, and compositor-order tests**

Run:

```bash
cargo test --features retained-renderer presentation::companion_scene::scene::compiler::tests::front_glass_hud_compiles_after_pet_as_screen_chrome -- --exact
cargo test --test companion_draw_boundary appkit_front_glass_hud_is_painted_after_the_renderer_payload -- --exact
cargo test --features retained-renderer companion::retained::hud::tests -- --nocapture
cargo test --features retained-renderer presentation::companion_scene::scene::checksum::tests -- --nocapture
```

Expected: all selected tests pass; exact HUD text remains sealed/redacted as before, and both paths represent the HUD as front glass.

- [ ] **Step 6: Commit Task 5**

```bash
git add src/round/hud.rs src/companion/app.rs src/presentation/companion_scene/scene/compiler.rs tests/companion_draw_boundary.rs
git diff --cached --check
git commit -m "feat(companion): mark HUD as front-glass chrome"
```

---

### Task 6: Add Deterministic Rear, Neutral, and Front Preview Frames

**Files:**
- Modify: `src/dev_preview/smooth.rs:1-125`
- Modify: `src/dev_preview/scenarios.rs:130-190`
- Modify: `tests/dev_preview.rs:2625-2820`

**Interfaces:**
- Consumes: `try_build_round_smooth_scene_plan_with_options` and `SmoothSceneBuildOptions`.
- Produces: Preview scenario IDs `round-smooth-depth-far`, `round-smooth-depth-neutral`, and `round-smooth-depth-front` on the production `44x18` grid.

- [ ] **Step 1: Add failing Preview Lab assertions**

Add this test to `tests/dev_preview.rs` beside the existing Smooth scenario tests:

```rust
#[test]
fn round_preview_exports_full_tank_depth_endpoints() {
    let run = PreviewRun::new();
    run.run_success("round");
    let manifest = run.manifest();
    let ids = [
        "round-smooth-depth-far",
        "round-smooth-depth-neutral",
        "round-smooth-depth-front",
    ];
    let mut translations = Vec::new();
    for id in ids {
        let entry = scenario(&manifest, id);
        assert_eq!(entry["dimensions"]["width"], 44);
        assert_eq!(entry["dimensions"]["height"], 18);
        let plan = run.read_json(entry["files"]["smooth_plan"].as_str().unwrap());
        let pet = plan["layers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|layer| layer["role"] == "pet-body")
            .unwrap();
        translations.push(pet["transform"]["translation"]["y"].as_f64().unwrap());
    }
    assert!(translations[0] < translations[1]);
    assert!(translations[1] < translations[2]);
    assert!(translations[2] - translations[0] > 7.0);
}
```

- [ ] **Step 2: Run the Preview Lab test and verify RED**

Run:

```bash
cargo test --features dev-preview --test dev_preview round_preview_exports_full_tank_depth_endpoints -- --exact
```

Expected: FAIL because the three scenario IDs do not exist.

- [ ] **Step 3: Export the three endpoint scenarios**

In `src/dev_preview/smooth.rs`, add:

```rust
const DEPTH_GRID_COLS: u16 = 44;
const DEPTH_GRID_ROWS: u16 = 18;
pub const SMOOTH_DEPTH_FAR_ID: &str = "round-smooth-depth-far";
pub const SMOOTH_DEPTH_NEUTRAL_ID: &str = "round-smooth-depth-neutral";
pub const SMOOTH_DEPTH_FRONT_ID: &str = "round-smooth-depth-front";

pub fn smooth_depth_bundles(ctx: &PreviewRenderContext) -> Vec<PreviewScenarioBundle> {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let review_motion = CompanionMotion {
        wander_half: 8,
        drift_x_frac: 0.0,
        drift_y_frac: 0.0,
        drift_period_secs: 22,
        upward_bias: 0.0,
        wander: true,
    };
    [
        (SMOOTH_DEPTH_FAR_ID, "Smooth Depth Far", -1.0f32, "far"),
        (SMOOTH_DEPTH_NEUTRAL_ID, "Smooth Depth Neutral", 0.0f32, "neutral"),
        (SMOOTH_DEPTH_FRONT_ID, "Smooth Depth Front", 1.0f32, "front"),
    ]
    .into_iter()
    .map(|(id, title, depth, plane)| {
        let plan = crate::round::smooth::try_build_round_smooth_scene_plan_with_options(
            &vm,
            ctx.fixed_now,
            DEPTH_GRID_COLS,
            DEPTH_GRID_ROWS,
            &review_motion,
            0,
            crate::round::smooth::SmoothSceneBuildOptions {
                depth_override: Some(depth),
            },
        )
        .expect("depth endpoint preview should build");
        let mut frame = scene_draw_list_to_preview_frame(
            id,
            title,
            DEPTH_GRID_COLS,
            DEPTH_GRID_ROWS,
            &plan.flatten_classic_cells(),
        );
        frame.contract.smooth_plan = Some(PreviewSmoothPlanArtifact::from_scene_plan(
            id, &vm, &plan,
        ));
        PreviewScenarioBundle::from_parts(
            frame,
            PreviewScenarioKind::Smooth,
            "Review the pet's depth-driven vertical placement in the physical tank.",
            BTreeMap::from([
                ("fixture".to_string(), Value::String("full-tank-depth".to_string())),
                ("depth".to_string(), json!(depth)),
                ("plane".to_string(), Value::String(plane.to_string())),
            ]),
            Some(smooth_round_metadata(DEPTH_GRID_COLS, DEPTH_GRID_ROWS)),
            vec![
                format!("Confirm the pet reads at the {plane} plane without aperture clipping."),
                "Confirm shallow scale remains restrained while vertical placement carries depth."
                    .to_string(),
            ],
        )
    })
    .collect()
}
```

Change the end of `smooth_bundles` to append the depth fixtures:

```rust
let mut bundles = vec![baseline_bundle, parity_bundle];
bundles.extend(smooth_depth_bundles(ctx));
bundles
```

Name the existing two `PreviewScenarioBundle::from_parts` values `baseline_bundle` and `parity_bundle` before constructing the vector; do not duplicate their contents.

Update the `PreviewSelection::Round` branch in `src/dev_preview/scenarios.rs`:

```rust
PreviewSelection::Round => {
    bundles.extend(crate::dev_preview::round::round_bundles(&ctx));
    bundles.extend(crate::dev_preview::smooth::smooth_depth_bundles(&ctx));
}
```

- [ ] **Step 4: Run Preview Lab tests and export artifacts**

Run:

```bash
cargo test --features dev-preview --test dev_preview round_preview_exports_full_tank_depth_endpoints -- --exact
cargo test --features dev-preview --test dev_preview
cargo run -- dev-preview --scenario round --out target/glorp-preview-full-depth
```

Expected: tests pass; `target/glorp-preview-full-depth/manifest.json` lists all three IDs and each frame has `.txt`, `.cells.json`, and Smooth typed contract artifacts.

- [ ] **Step 5: Perform deterministic visual review**

Open:

```bash
open target/glorp-preview-full-depth/index.html
```

Review in order:

1. Far pet center is near 27% height and fully inside the aperture.
2. Neutral pet center is near 50% height.
3. Front pet center is near 73% height and visibly enters the lower HUD-content band.
4. Pet size changes only from `0.97x` to `1.035x`.
5. No prop, gauge, shadow, aura, or pet-attached layer detaches from the shared center.

Preview cell frames do not rasterize native HUD glyphs. Task 5 proves front-glass ordering from the prepared contracts, and Task 7 verifies the actual pet/HUD overlap in the rebuilt native companion.

- [ ] **Step 6: Commit Task 6**

```bash
git add src/dev_preview/smooth.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git diff --cached --check
git commit -m "test(companion): preview full tank depth endpoints"
```

---

### Task 7: Run Final Verification and Rebuild the Companion

**Files:**
- Verify: all files changed in Tasks 1-6
- Verify: `docs/superpowers/specs/2026-07-16-glorp-full-tank-depth-traversal-design.md`

**Interfaces:**
- Consumes: completed implementation commits from Tasks 1-6.
- Produces: clean formatting, lint, unit/integration, deterministic preview, and live optimized companion evidence.

- [ ] **Step 1: Run formatting and lint checks**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: both commands exit `0` with no warnings.

- [ ] **Step 2: Run the focused companion suites**

```bash
cargo test --test smooth_companion
cargo test --test round_scene
cargo test --test retained_scene
cargo test --test companion_draw_boundary
cargo test --features retained-renderer presentation::companion_scene -- --nocapture
cargo test --features dev-preview --test dev_preview
```

Expected: all focused suites pass.

- [ ] **Step 3: Run the full repository suite**

```bash
cargo test --all-features
```

Expected: all library, binary, integration, and documentation tests pass.

- [ ] **Step 4: Inspect the final diff and commit boundaries**

```bash
git status --short --branch
git log --oneline --decorate -7
git diff HEAD~6..HEAD --check
git diff HEAD~6..HEAD --stat
```

Expected: no unstaged implementation edits remain; six scoped implementation commits follow the approved design/plan docs; generated Preview Lab output remains untracked or ignored.

- [ ] **Step 5: Capture pinned native depth frames in both render paths**

```bash
for renderer in smooth retained; do
  for depth in far neutral near; do
    cargo run --features retained-renderer -- companion-app \
      --renderer "$renderer" \
      --review-depth "$depth" \
      --review-size 360x360 \
      --review-duration-ms 2000 \
      --review-capture-dir "target/glorp-review/full-depth-$renderer-$depth"
  done
done
```

Expected: all six bounded runs exit successfully and write a PNG plus `render-log.json`. Compare far, neutral, and near PNGs for each renderer; the pet center must progress rear-to-front, the near pet must overlap beneath the stats, and no pet ink may leave the circular aperture.

- [ ] **Step 6: Rebuild and launch the optimized companion**

```bash
cargo xtask companion fresh
```

Expected: the optimized `target/macos/Glorp.app` is rebuilt, any prior companion process exits, and the fresh app launches.

- [ ] **Step 7: Confirm the live acceptance contract**

Use the six pinned captures from Step 5 and the freshly launched companion to confirm:

- rear placement visibly reaches the upper/rear tank plane;
- front placement visibly reaches below the midpoint into the stats region;
- stats remain legible because they paint over the pet;
- scale remains shallow and the pet never clips the circular aperture;
- sleep and Reduce Motion remain restrained;
- Classic mode remains visually unchanged.

If the live scene disagrees with deterministic artifacts, stop and capture the exact renderer mode, depth override, grid dimensions, and screenshot before changing constants.
