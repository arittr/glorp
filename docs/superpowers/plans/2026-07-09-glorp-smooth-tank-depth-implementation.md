# Glorp Smooth Tank Bed And Depth Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the failed glyph floor with a curved, typed-geometry tank bed and make the existing Glorp pet move smoothly forward and backward from `0.88x` to `1.12x` while preserving Classic art, props, tank life, HUD, and gauges.

**Architecture:** Extend the platform-neutral Smooth scene plan with typed ellipse shapes and validated uniform transforms. Add a deterministic Z channel beside the existing X/Y roam, resolve one depth sample into pet scale, perspective Y, and a floor projection, and render those contracts in AppKit. Keep the new bed and depth effects Smooth-only, expose the same geometry through Preview Lab, and add deterministic native far/neutral/near review captures.

**Tech Stack:** Rust, existing Smooth/Classic scene plans, ratatui geometry, serde/serde_json Preview Lab artifacts, AppKit/objc2 on macOS, existing native review capture and `cargo xtask companion fresh` workflows.

## Global Constraints

- Implement `docs/superpowers/specs/2026-07-09-glorp-smooth-tank-depth-design.md` as approved.
- Preserve current Glorp pet art, cast identity, habitat props, tank life, HUD, gauges, and free-swimming composition.
- Keep Classic rendering and Classic snapped placement behavior unchanged.
- Keep `literal_floor_allowed: false`; the tank bed is presentation geometry, not a route substrate.
- Remove the unshipped `FloorTexture` role and glyph-dither implementation. Do not preserve a compatibility alias.
- Keep all scene meaning in platform-neutral Rust types. AppKit receives geometry and transforms; it does not infer tank semantics.
- Support ellipse shapes only in this slice. Do not add polygons, images, blur, physics, dynamic occlusion, or Linux windowing.
- **Amended during implementation:** gradients are now in scope. Faking a gradient
  by stacking constant-alpha shapes bands visibly at companion size, which is what
  the tank bed and the tank's own depth falloff both did. Shapes carry a typed
  `SmoothFill` of `Solid` or `RadialGradient`, and the native tank background uses
  `NSGradient` directly. Polygons, images, blur, and physics remain out of scope.
- **Amended during implementation:** roam clearance is reserved against the 11x8
  creature art, not the 13x10 particle frame, and additionally reserves the full
  `SMOOTH_PERSPECTIVE_Y_MAX` excursion. Reserving the frame is incompatible with a
  `1.12x` near scale on the shipping 36x18 grid: the band inverts. The frame's
  ambient gutter may graze the HUD reserve, which is sound because the native HUD
  draws above the scene.
- Support finite, positive, uniform scale and zero rotation. Reject unsupported transforms before the native draw callback.
- Reserve X/Y roam clearance against the maximum `1.12x` scale, not only the current frame.
- Apply the composed pet transform to `PetBody`, `WallShadow`, `PerformanceCue`, and the actual mood-aura draw path.
- Keep `FloorProjection` below props and tank life, anchored to the bed, and independent of idle bob.
- Use deterministic biome/viewport seeds for bed flecks and a separately salted deterministic Z target channel.
- New Preview/native artifacts must remain sanitized: no source names, prompt text, project paths, raw diagnostics, or pet seed material.
- Stage only the files named by each task after inspecting `git diff --check` and `git status --short`.

## Expected End State

Running the Smooth companion shows the same Glorp and tank composition, now over a curved lower tank bed that reads at small size. Glorp continues its smooth X/Y roam while also moving forward and backward: nearer frames are larger and slightly lower, farther frames are smaller and slightly higher. A typed ellipse projection tracks the same depth on the bed and always remains behind props and tank inhabitants. Classic remains unchanged.

Preview Lab records typed bed/projection ellipses, depth, scale, perspective offset, transformed bounds, and maximum-scale clearance. Native review mode can pin far, neutral, and near depth for deterministic captures at `360x360` and `720x720`, and `render-log.json` proves distinct pet extents, nonblank shape draws, bounded adjacent scale changes, and a crash-free multi-second run.

## File Map

| Path | Responsibility |
| --- | --- |
| `src/presentation/smooth.rs` | Typed colors/shapes, role replacement, transform validation, transformed bounds, and depth evidence on the portable scene plan. |
| `src/round/tank_bed.rs` | Pure normalized bed geometry, deterministic flecks, and depth-derived floor projection geometry. |
| `src/round/depth.rs` | Pure Z interpolation, lifecycle attenuation, scale mapping, perspective mapping, and validation. |
| `src/round/mod.rs` | Export the new focused round modules. |
| `src/round/scene.rs` | Deterministic Z channel and maximum-scale-safe Smooth placement while retaining exact Classic placement. |
| `src/round/smooth.rs` | Compose depth transforms, insert `TankBed`, replace the floor projection cells with an ellipse, and build the fallible plan. |
| `src/companion/app.rs` | Render typed ellipses and uniform scale in AppKit; derive the mood aura from transformed pet bounds. |
| `src/companion/review_capture.rs` | Record depth, scale, perspective, shape draw counts, pet extents, and adjacent-frame scale evidence. |
| `src/commands/companion_mode.rs` | Hidden deterministic native review-depth enum and option. |
| `src/commands/companion.rs` | Forward the hidden review-depth option to the native companion process. |
| `src/cli.rs` | Parse hidden `--review-depth far|neutral|near` arguments for companion review runs. |
| `src/lib.rs` | Thread the review-depth option through command dispatch. |
| `src/dev_preview/contract.rs` | Serialize typed shape artifacts and pet depth/scale/perspective/clearance evidence. |
| `src/dev_preview/smooth.rs` | Generate deterministic far/neutral/near frames and continuous depth strips. |
| `tests/smooth_companion.rs` | Cross-module shape, role, Z, transform, projection, clearance, and Classic-isolation coverage. |
| `tests/dev_preview.rs` | Preview schema, deterministic shape, depth, privacy, and parity assertions. |

## Core Interfaces

Implement these concrete interfaces. Names may change only to resolve a compiler-proven collision; preserve field meanings and invariants.

```rust
// src/presentation/smooth.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SmoothRgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub enum SmoothShapeGeometry {
    Ellipse { bounds: SmoothBounds },
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct SmoothShape {
    pub geometry: SmoothShapeGeometry,
    pub fill: SmoothRgba8,
}

pub enum SmoothLayerItem {
    LocalCell(SmoothLocalCell),
    Shape(SmoothShape),
    Raster(SmoothRasterRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothGeometryError {
    NonFiniteGeometry,
    InvalidBounds,
    NonPositiveScale,
    NonUniformScale,
    UnsupportedRotation,
}

pub fn validate_smooth_layer(layer: &SmoothCompanionLayer) -> Result<(), SmoothGeometryError>;
pub fn transformed_smooth_bounds(layer: &SmoothCompanionLayer) -> Result<SmoothBounds, SmoothGeometryError>;
```

Add `serde::Serialize` to the existing `SmoothPoint` and `SmoothBounds` derives
so typed shape serialization does not create a duplicate artifact-only bounds
model inside the renderer contract.

```rust
// src/round/depth.rs
pub const SMOOTH_PET_FAR_SCALE: f32 = 0.88;
pub const SMOOTH_PET_NEAR_SCALE: f32 = 1.12;
pub const SMOOTH_PERSPECTIVE_Y_MAX: f32 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothDepthSample {
    pub raw_z: f32,
    pub effective_z: f32,
    pub scale: f32,
    pub perspective_y: f32,
}

pub const fn depth_lifecycle_scale(asleep: bool, calm: bool) -> f32;
pub fn resolve_smooth_depth(raw_z: f32, lifecycle_scale: f32) -> Result<SmoothDepthSample, SmoothDepthError>;
```

```rust
// src/round/tank_bed.rs
#[derive(Debug, Clone, PartialEq)]
pub struct SmoothTankBedGeometry {
    pub shapes: Vec<SmoothShape>,
    pub horizon_y: f32,
    pub near_edge_y: f32,
}

pub fn smooth_tank_bed_geometry(
    viewport: CompanionViewport,
    biome: crate::tui::room::RoomBiome,
) -> Option<SmoothTankBedGeometry>;

pub fn smooth_floor_projection_shape(
    viewport: CompanionViewport,
    bed: &SmoothTankBedGeometry,
    pet_center_x: f32,
    depth: SmoothDepthSample,
) -> Option<SmoothShape>;
```

```rust
// New fields on existing scene evidence.
pub struct SmoothCompanionPet {
    // Existing fields remain.
    pub depth: f32,
    pub scale: f32,
    pub perspective_offset: SmoothPoint,
    pub transformed_bounds: SmoothBounds,
    pub max_scale_clearance: SmoothBounds,
}
```

```rust
// src/commands/companion_mode.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompanionReviewDepth {
    Far,
    Neutral,
    Near,
}

impl CompanionReviewDepth {
    pub const fn normalized(self) -> f32 {
        match self {
            Self::Far => -1.0,
            Self::Neutral => 0.0,
            Self::Near => 1.0,
        }
    }
}
```

## Task 0: Reconcile And Checkpoint The Dirty Baseline

**Files:**
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/presentation/smooth.rs`
- Modify: `src/round/smooth.rs`
- Modify: `tests/dev_preview.rs`
- Modify: `tests/smooth_companion.rs`

- [ ] **Step 1: Inspect and classify every uncommitted hunk**

Run:

```bash
git status --short
git diff -- src/dev_preview/contract.rs src/presentation/smooth.rs src/round/smooth.rs tests/dev_preview.rs tests/smooth_companion.rs
```

Preserve the approved Smooth-only corrections:

- `FloorProjection` translates one row upward relative to the failed first placement.
- `FloorProjection` uses `z = 1`, below every prop and tank-life layer.
- Preview parity can report `smooth-extension` when intentional Smooth-only content changes the flattened checksum.
- Tests no longer require new Smooth-only visual content to flatten exactly to Classic.

Delete the failed experiment:

- `SmoothLayerRole::FloorTexture`
- `smooth_floor_texture_layer(...)`
- substrate constants, glyph cells, role bindings, required-role entries, and related assertions

- [ ] **Step 2: Write the baseline regression tests first**

In `tests/smooth_companion.rs`, add/retain assertions equivalent to:

```rust
let projection = plan.layers.iter().find(|layer| layer.role == SmoothLayerRole::FloorProjection).unwrap();
let first_prop_z = plan.layers.iter().filter(|layer| matches!(layer.role, SmoothLayerRole::PropsBehind | SmoothLayerRole::PropsFront | SmoothLayerRole::TankLifeBehind | SmoothLayerRole::TankLifeFront)).map(|layer| layer.z).min().unwrap();
assert!(projection.z < first_prop_z);
assert_eq!(projection.transform.translation.y, -1.0);
assert!(!plan.layers.iter().any(|layer| layer.role.as_str() == "floor-texture"));
```

In `tests/dev_preview.rs`, restore the canonical role count to 19 while retaining `smooth-extension` parity semantics.

- [ ] **Step 3: Run the focused baseline tests**

```bash
cargo test --test smooth_companion floor_projection -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_smooth_sidecars_are_sanitized_and_report_parity -- --nocapture
```

Expected: both pass; no `floor-texture` role remains.

- [ ] **Step 4: Commit only the reconciled baseline**

```bash
git diff --check
git status --short
git add src/dev_preview/contract.rs src/presentation/smooth.rs src/round/smooth.rs tests/dev_preview.rs tests/smooth_companion.rs
git commit -m "fix(companion): keep floor projection below props"
```

## Task 1: Add Typed Smooth Ellipses And Transform Validation

**Files:**
- Modify: `src/presentation/smooth.rs`
- Modify: `tests/smooth_companion.rs`

- [ ] **Step 1: Add failing portable-contract tests**

Add tests named `smooth_shape_is_typed_and_serializable`,
`smooth_geometry_rejects_nonfinite_nonpositive_nonuniform_and_rotated_layers`,
and `transformed_bounds_scale_around_the_declared_origin`. The first must construct
an ellipse with bounds `(1.0, 2.0)..(4.0, 6.0)` and RGBA `(90, 61, 99, 128)`,
serialize it with `serde_json`, and assert the typed geometry and all four color
channels survive. The second must construct one otherwise-valid layer per error
variant and assert the exact `SmoothGeometryError`. The third must assert a
`2x2` local box scaled to `1.12x` around its center becomes `2.24x2.24` while its
center remains unchanged.

The transformed-bounds expectation must use:

```text
pivot = anchor + transform_origin
world = pivot + (anchor + local_point - pivot) * scale + translation
```

Run and confirm RED:

```bash
cargo test --test smooth_companion smooth_shape_is_typed_and_serializable -- --nocapture
cargo test --test smooth_companion smooth_geometry_rejects -- --nocapture
cargo test --test smooth_companion transformed_bounds_scale -- --nocapture
```

- [ ] **Step 2: Replace the name-only shape reference with typed geometry**

In `src/presentation/smooth.rs`:

- Add `SmoothRgba8`, `SmoothShapeGeometry::Ellipse`, and `SmoothShape`.
- Change `SmoothLayerItem::Shape(SmoothShapeRef)` to `Shape(SmoothShape)`.
- Remove `SmoothShapeRef` if no remaining caller uses it.
- Derive `PartialEq`, not `Eq`, on enums/structs that now contain `f32` geometry.
- Keep `Raster` descriptive and unrendered; this slice does not invent a raster backend.

- [ ] **Step 3: Implement one validator used by all consumers**

`validate_smooth_layer(...)` must check layer bounds, anchor, transform origin, translation, opacity, scale, shape bounds, and clip bounds are finite. It must reject inverted bounds, opacity outside `[0, 1]`, scale `<= 0`, non-uniform scale beyond `f32::EPSILON * 8.0`, and `rotation_degrees.abs() > f32::EPSILON`.

`transformed_smooth_bounds(...)` must call the validator and transform all four bounds corners around the layer pivot.

- [ ] **Step 4: Run focused tests and commit**

```bash
cargo test --test smooth_companion smooth_shape -- --nocapture
cargo test --test smooth_companion smooth_geometry -- --nocapture
cargo test --test smooth_companion transformed_bounds -- --nocapture
cargo test presentation::smooth --lib
git diff --check
git add src/presentation/smooth.rs tests/smooth_companion.rs
git commit -m "feat(companion): add typed smooth ellipse geometry"
```

## Task 2: Build The Curved Tank Bed As Pure Geometry

**Files:**
- Create: `src/round/tank_bed.rs`
- Modify: `src/round/mod.rs`
- Modify: `src/presentation/smooth.rs`
- Modify: `src/round/smooth.rs`
- Modify: `tests/smooth_companion.rs`

- [ ] **Step 1: Add failing geometry and layer-order tests**

Cover these exact contracts:

- `SmoothLayerRole::TankBed.as_str() == "tank-bed"` and its binding is `Fixed`.
- `smooth_tank_bed_geometry(...)` returns 2-3 broad ellipse bands plus 8-14 small ellipse flecks for a normal viewport.
- The bed horizon begins near `0.76 * viewport.grid_rows` and the visible bed occupies roughly the lower 24%.
- Every shape is finite, positive-area, and clipped by the companion aperture.
- Repeated calls with the same `RoomBiome` and viewport are exactly equal.
- Changing time or pet placement cannot change bed geometry.
- Degenerate viewports return `None`, not a panic.
- The `TankBed` layer sorts after `RoomGlyphs` and before `Ambient`/props/tank life.
- No tank-bed item is a `LocalCell`.

Run and confirm RED:

```bash
cargo test --test smooth_companion tank_bed -- --nocapture
```

- [ ] **Step 2: Implement normalized ellipse bands**

Use viewport-local coordinates. The broad base ellipse may extend below the aperture so clipping creates the curved horizon:

```rust
let width = f32::from(viewport.grid_cols);
let height = f32::from(viewport.grid_rows);
let horizon_y = height * 0.76;
let base = SmoothBounds {
    min: SmoothPoint { x: -width * 0.08, y: horizon_y },
    max: SmoothPoint { x: width * 1.08, y: height * 1.34 },
};
```

Add two inset ellipses with restrained plum/teal alpha variation. Generate
bounded flecks with a local integer hash over only `RoomBiome.primary`,
`RoomBiome.secondary`, `viewport.grid_cols`, and `viewport.grid_rows`; do not
use the private pet seed, current time, or `thread_rng`.

- [ ] **Step 3: Insert the fixed TankBed layer**

In `src/presentation/smooth.rs`, replace the deleted experimental role with `TankBed` and bind it to `Fixed`.

In `src/round/smooth.rs`, insert one `SmoothCompanionLayer` after `RoomGlyphs` with:

```rust
role: SmoothLayerRole::TankBed,
motion_binding: SmoothLayerMotionBinding::Fixed,
z: 1,
clip: SmoothClip::Circle {
    center: SmoothPoint {
        x: f32::from(viewport.grid_cols) / 2.0,
        y: f32::from(viewport.grid_rows) / 2.0,
    },
    radius: f32::from(viewport.grid_cols.min(viewport.grid_rows)) / 2.0,
},
blend: SmoothBlendMode::Normal,
items: bed.shapes.into_iter().map(SmoothLayerItem::Shape).collect(),
```

Preserve stable ordering for equal Z by inserting after room layers and before ambient layers.

- [ ] **Step 4: Run focused tests and commit**

```bash
cargo test --test smooth_companion tank_bed -- --nocapture
cargo test --test round_scene -- --nocapture
git diff --check
git add src/round/tank_bed.rs src/round/mod.rs src/presentation/smooth.rs src/round/smooth.rs tests/smooth_companion.rs
git commit -m "feat(companion): add curved smooth tank bed"
```

## Task 3: Add Deterministic Z Motion And Max-Scale-Safe Placement

**Files:**
- Create: `src/round/depth.rs`
- Modify: `src/round/mod.rs`
- Modify: `src/round/scene.rs`
- Modify: `tests/smooth_companion.rs`

- [ ] **Step 1: Add failing depth-unit tests**

Test:

```rust
assert_eq!(resolve_smooth_depth(-1.0, 1.0).unwrap().scale, 0.88);
assert_eq!(resolve_smooth_depth(0.0, 1.0).unwrap().scale, 1.0);
assert_eq!(resolve_smooth_depth(1.0, 1.0).unwrap().scale, 1.12);
assert_eq!(depth_lifecycle_scale(false, false), 1.0);
assert_eq!(depth_lifecycle_scale(false, true), 0.5);
assert_eq!(depth_lifecycle_scale(true, false), 0.25);
assert_eq!(depth_lifecycle_scale(true, true), 0.25);
```

Also prove nonfinite Z, lifecycle outside `[0, 1]`, and output outside finite bounds are rejected.

- [ ] **Step 2: Add failing motion and clearance tests**

In `tests/smooth_companion.rs`, sample at least two full drift periods in 50 ms increments and assert:

- identical inputs produce identical Z;
- Z remains in `[-1, 1]`;
- adjacent raw-Z delta is bounded and no waypoint jump appears;
- Z is not identical to X or Y across the sample set;
- Classic `classic_rect` is byte-for-byte equal before/after adding Z;
- maximum-scale Smooth bounds remain inside aperture/gauge/HUD protected regions.

Run and confirm RED:

```bash
cargo test --test smooth_companion smooth_depth -- --nocapture
cargo test --test smooth_companion maximum_scale -- --nocapture
```

- [ ] **Step 3: Implement the pure depth resolver**

In `src/round/depth.rs`:

```rust
let effective_z = raw_z.clamp(-1.0, 1.0) * lifecycle_scale;
let depth01 = (effective_z + 1.0) * 0.5;
let scale = SMOOTH_PET_FAR_SCALE + depth01 * (SMOOTH_PET_NEAR_SCALE - SMOOTH_PET_FAR_SCALE);
let perspective_y = effective_z * SMOOTH_PERSPECTIVE_Y_MAX;
```

Far is negative/up; near is positive/down. Keep `perspective_y.abs() < 1.0` logical cell.

- [ ] **Step 4: Extend the existing deterministic roam with a separately salted Z channel**

In `src/round/scene.rs`:

- Extend the internal motion offset result to carry `z: f32`.
- Use the same waypoint epoch/interpolation as X/Y and a distinct salt constant for Z targets.
- For sinusoidal wander, use a phase/frequency not shared by X or Y.
- Add `raw_depth: f32` to `CompanionPetPlacement` as Smooth evidence.
- Shrink only the fractional Smooth roam envelope using `SMOOTH_PET_NEAR_SCALE`; leave Classic integer clamp and `classic_rect` unchanged.

The safe fractional half extent is:

```text
scaled_half = unscaled_half * 1.12
safe_center_min = protected_min + scaled_half
safe_center_max = protected_max - scaled_half
```

Return a neutral depth for degenerate motion geometry rather than poisoning the frame.

- [ ] **Step 5: Run focused tests and commit**

```bash
cargo test --test smooth_companion smooth_depth -- --nocapture
cargo test --test smooth_companion maximum_scale -- --nocapture
cargo test round::scene --lib
git diff --check
git add src/round/depth.rs src/round/mod.rs src/round/scene.rs tests/smooth_companion.rs
git commit -m "feat(companion): add deterministic smooth depth motion"
```

## Task 4: Compose Pet Depth And Replace The Projection With An Ellipse

**Files:**
- Modify: `src/presentation/smooth.rs`
- Modify: `src/round/tank_bed.rs`
- Modify: `src/round/smooth.rs`
- Modify: `tests/smooth_companion.rs`

- [ ] **Step 1: Add failing scene-plan tests**

At explicit far, neutral, and near raw depth, assert:

- pet scale is `0.88`, `1.0`, `1.12` in normal lifecycle;
- perspective Y is negative, zero, positive;
- `PetBody`, `WallShadow`, and `PerformanceCue` share identical scale and depth translation;
- `PetBody` alone retains idle bob; `FloorProjection` does not inherit bob;
- `pet.transformed_bounds` equals `transformed_smooth_bounds(PetBody)`;
- the actual aura center/extent metadata follows transformed pet bounds;
- `FloorProjection` contains exactly one `SmoothLayerItem::Shape(Ellipse { .. })` and no background cells;
- far projection is smaller and fainter/closer to the horizon than near projection;
- projection Z is below all props/tank life;
- the plan validator rejects any invalid composed transform.

Expose a crate-visible test constructor or build options rather than forging plan internals:

```rust
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SmoothSceneBuildOptions {
    pub depth_override: Option<f32>,
}

#[doc(hidden)]
pub fn try_build_round_smooth_scene_plan_with_options(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    elapsed_ms: u64,
    options: SmoothSceneBuildOptions,
) -> Result<SmoothCompanionScenePlan, SmoothScenePlanError>;
```

Run and confirm RED:

```bash
cargo test --test smooth_companion depth_transform -- --nocapture
cargo test --test smooth_companion floor_projection_ellipse -- --nocapture
```

- [ ] **Step 2: Resolve one depth sample per frame**

In `src/round/smooth.rs`, derive `round_scene` first, select
`options.depth_override.unwrap_or(placement.raw_depth)`, then call:

```rust
resolve_smooth_depth(
    raw_depth,
    depth_lifecycle_scale(
        round_scene.lifecycle.asleep,
        round_scene.lifecycle.calm,
    ),
)
```

Map any depth or layer validation failure into explicit `SmoothScenePlanError` variants; do not collapse them into `InvalidParallaxGeometry`.

- [ ] **Step 3: Compose attached transforms around the visual center**

Use one shared pivot and uniform scale for `PetBody`, `WallShadow`, and `PerformanceCue`. Apply perspective translation to all attached layers and idle bob only where the existing bob contract permits it. Validate every layer before returning the plan.

Update `SmoothCompanionPet` with depth, scale, perspective offset, transformed bounds, and the maximum-scale clearance envelope. Derive the prepared aura center/extent from transformed bounds rather than from unscaled `pet_width_cells`.

- [ ] **Step 4: Build the floor projection from the bed and depth sample**

In `src/round/tank_bed.rs`, interpolate projection center Y, radii, and alpha from effective Z:

```text
t = (effective_z + 1) / 2
center_y = lerp(horizon_y + 0.10 * bed_height, near_edge_y - 0.10 * bed_height, t)
radius_x = lerp(0.055 * viewport_width, 0.105 * viewport_width, t)
radius_y = lerp(0.012 * viewport_height, 0.030 * viewport_height, t)
alpha = round(lerp(46, 92, t))
```

Clamp the ellipse horizontally inside the aperture. Return `None` for degenerate geometry. In `src/round/smooth.rs`, replace the Classic-derived projection cells only in the Smooth plan while preserving Classic flatten compatibility data.

- [ ] **Step 5: Run focused tests and commit**

```bash
cargo test --test smooth_companion depth_transform -- --nocapture
cargo test --test smooth_companion floor_projection -- --nocapture
cargo test --test smooth_companion classic -- --nocapture
cargo test --test round_scene -- --nocapture
git diff --check
git add src/presentation/smooth.rs src/round/tank_bed.rs src/round/smooth.rs tests/smooth_companion.rs
git commit -m "feat(companion): compose pet depth and bed projection"
```

## Task 5: Render Shapes And Uniform Scale In AppKit

**Files:**
- Modify: `src/companion/app.rs`
- Modify: `tests/smooth_companion.rs`

- [ ] **Step 1: Extract and test pure AppKit-coordinate helpers**

Keep the math in a macOS-independent module section callable from unit tests:

```rust
fn smooth_layer_point(layer: &SmoothCompanionLayer, local: SmoothPoint) -> Result<SmoothPoint, SmoothGeometryError>;
fn smooth_shape_rect(metrics: &CompanionGridMetrics, layer: &SmoothCompanionLayer, bounds: SmoothBounds) -> Result<CGRect, SmoothGeometryError>;
```

Test identity, `0.88x`, `1.12x`, translation, pivot stability, transformed ellipse width/height, and rejection of invalid geometry. Confirm RED before implementation:

```bash
cargo test companion::app::tests::smooth_layer_point --lib -- --nocapture
cargo test companion::app::tests::smooth_shape_rect --lib -- --nocapture
```

- [ ] **Step 2: Apply coherent uniform transforms to local cells**

In `appkit_blit_smooth_plan(...)`:

- Call `validate_smooth_layer(layer)` before drawing.
- Transform each cell origin around the layer pivot.
- Multiply cell width, cell height, and font size by the validated uniform scale.
- Multiply foreground/background alpha by `layer.opacity`.
- Preserve glyph grouping; do not independently round transformed cells to logical grid coordinates.

- [ ] **Step 3: Render typed ellipse shapes**

For each `SmoothLayerItem::Shape(SmoothShape { geometry: Ellipse { bounds }, fill })`:

```rust
let rect = smooth_shape_rect(metrics, layer, bounds)?;
let path = NSBezierPath::bezierPathWithOvalInRect(rect);
rgba_to_nscolor(fill, layer.opacity).setFill();
path.fill();
```

Save/restore `NSGraphicsContext` state per layer. Map existing blend modes exactly:

```rust
Normal   => NSCompositingOperation::SourceOver,
Multiply => NSCompositingOperation::Multiply,
Screen   => NSCompositingOperation::Screen,
Add      => NSCompositingOperation::PlusLighter,
Replace  => NSCompositingOperation::Copy,
```

Intersect layer clips with the existing circular aperture clip. Continue to skip `Raster` explicitly; do not silently reinterpret it as a shape.

- [ ] **Step 4: Make native preparation fallible before draw**

Validate the complete plan in `prepare_companion_frame(...)`, map errors to a static privacy-safe `CompanionFramePreparationError`, and keep `draw_scene(...)` render-only. A malformed scale/shape must retain the last good frame rather than reach AppKit or panic.

- [ ] **Step 5: Derive mood aura from transformed pet bounds**

Replace the unscaled `pet_center_col`, `pet_center_row`, and `pet_width_cells` preparation with the transformed bounds center/width. Verify the aura follows far/near scale and perspective while HUD/gauge geometry stays fixed.

- [ ] **Step 6: Run focused tests and commit**

```bash
cargo test companion::app::tests::smooth_ --lib -- --nocapture
cargo test --test companion_draw_boundary -- --nocapture
git diff --check
git add src/companion/app.rs tests/smooth_companion.rs
git commit -m "feat(companion): render smooth shapes and layer scale"
```

## Task 6: Expose Typed Bed And Depth Evidence In Preview Lab

**Files:**
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/smooth.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Add failing artifact-schema tests**

Require additive fields:

```rust
pub struct PreviewSmoothShapeArtifact {
    pub kind: String,
    pub bounds: PreviewSmoothBoundsArtifact,
    pub fill: PreviewSmoothRgbaArtifact,
}

pub struct PreviewSmoothPetMotionArtifact {
    // Existing fields remain.
    pub depth: f32,
    pub scale: f32,
    pub perspective_y: f32,
    pub transformed_bounds: PreviewSmoothBoundsArtifact,
    pub max_scale_clearance: PreviewSmoothBoundsArtifact,
}
```

Each layer artifact must carry `shapes: Vec<PreviewSmoothShapeArtifact>` in addition to item counts. Test canonical `TankBed` binding, typed ellipse bounds/colors, projection order, deterministic sidecars, and privacy scans.

Run and confirm RED:

```bash
cargo test --features dev-preview --test dev_preview smooth_shape -- --nocapture
cargo test --features dev-preview --test dev_preview smooth_depth -- --nocapture
```

- [ ] **Step 2: Serialize typed geometry directly from the scene plan**

Do not reconstruct ellipses from flattened cells or parse role names. Serialize `SmoothLayerItem::Shape` and `SmoothCompanionPet` fields directly. Keep schema additions additive and update the required canonical role count from 19 to 20 with `TankBed`, not `FloorTexture`.

- [ ] **Step 3: Add deterministic far/neutral/near fixtures and a continuity strip**

In `src/dev_preview/smooth.rs`, build three pinned-depth frames through `SmoothSceneBuildOptions` and one unpinned strip spanning enough time to cross at least one Z waypoint. Aggregate min/max scale and maximum adjacent scale/perspective deltas.

- [ ] **Step 4: Generate and inspect Preview Lab**

```bash
cargo run --features dev-preview -- dev-preview --scenario round --out target/glorp-preview
open target/glorp-preview/index.html
```

Inspect the Smooth far/neutral/near frames at small and large preview sizes. Confirm the bed is curved, projection remains behind props, pet depth reads without HUD/gauge movement, and there are no terminal-row rectangles or glyph dither.

- [ ] **Step 5: Run focused tests and commit**

```bash
cargo test --features dev-preview --test dev_preview smooth -- --nocapture
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
git diff --check
git add src/dev_preview/contract.rs src/dev_preview/smooth.rs tests/dev_preview.rs
git commit -m "feat(preview): expose smooth tank depth evidence"
```

## Task 7: Add Deterministic Native Depth Review And Capture Evidence

**Files:**
- Modify: `src/commands/companion_mode.rs`
- Modify: `src/commands/companion.rs`
- Modify: `src/cli.rs`
- Modify: `src/lib.rs`
- Modify: `src/companion/app.rs`
- Modify: `src/companion/review_capture.rs`

- [ ] **Step 1: Add failing CLI forwarding tests**

Add hidden `--review-depth <far|neutral|near>` to both `Companion` and
`CompanionApp`, matching the existing review arguments. It is not persisted.
Its presence makes `CompanionReviewOptions::has_review_launch_options()` true;
only Smooth scene preparation consumes it. Test parsing and forwarding all
three values and prove Classic/Pixel preparation remains unchanged.

Run and confirm RED:

```bash
cargo test cli::tests::companion_review_depth --lib -- --nocapture
```

- [ ] **Step 2: Thread the override to Smooth scene construction**

Add `depth: Option<CompanionReviewDepth>` to `CompanionReviewOptions`, forward it through `src/lib.rs` and `src/commands/companion.rs`, and pass `normalized()` into `SmoothSceneBuildOptions`. Normal companion runs always use `None`.

- [ ] **Step 3: Extend native review samples and aggregates**

Add fields to `SmoothReviewFrameSample` and its JSON representation:

```rust
pub depth: f32,
pub pet_scale: f32,
pub perspective_y: f32,
pub pet_extent_width: f32,
pub pet_extent_height: f32,
pub shape_draw_count: u32,
```

Add aggregate `min_pet_scale`, `max_pet_scale`, `max_adjacent_pet_scale_delta`, and `nonblank_shape_frame_count`. Keep all values finite and sanitized. Update every existing sample constructor in tests explicitly.

- [ ] **Step 4: Add aggregation and privacy tests**

Test that far/neutral/near samples produce ordered extents, shape frames are nonblank, adjacent scale deltas are bounded in unpinned animation, and rendered JSON contains none of the existing forbidden privacy keys/values.

Run and confirm GREEN:

```bash
cargo test companion::review_capture --lib -- --nocapture
```

- [ ] **Step 5: Commit the review tooling**

```bash
git diff --check
git add src/commands/companion_mode.rs src/commands/companion.rs src/cli.rs src/lib.rs src/companion/app.rs src/companion/review_capture.rs
git commit -m "feat(companion): add deterministic depth review captures"
```

## Task 8: Verify The Full Slice And Spot-Check The Native Companion

**Files:**
- Verify only unless a focused failure requires a scoped fix.

- [ ] **Step 1: Run format and the focused contract suites**

```bash
cargo fmt --check
cargo test --test smooth_companion
cargo test --test round_scene
cargo test --test companion_draw_boundary
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
```

Expected: all pass with no ignored geometry/privacy failures.

- [ ] **Step 2: Run static analysis on the touched surfaces**

```bash
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```

Expected: clean. If repository-wide clippy exposes unrelated pre-existing failures, record exact output and run the narrowest clippy target that proves the touched modules.

- [ ] **Step 3: Prove Classic isolation and routing invariants**

```bash
cargo test --test smooth_companion classic
cargo test --test round_scene literal_floor_allowed
rg -n "FloorTexture|floor-texture|SMOOTH_SUBSTRATE|smooth_floor_texture_layer" src tests
```

Expected: tests pass and `rg` returns no matches.

- [ ] **Step 4: Capture far/neutral/near native frames at both review sizes**

Run the direct hidden app command so each process exits after writing its
artifacts:

```bash
for size in 360x360 720x720; do
  for depth in far neutral near; do
    cargo run -- companion-app \
      --renderer smooth \
      --review-depth "$depth" \
      --review-size "$size" \
      --review-duration-ms 2000 \
      --review-capture-dir "target/glorp-review/depth-$size-$depth"
  done
done
```

For each capture, inspect the PNG and `render-log.json`. Confirm:

- bed pixels are nonblank and curved;
- near pet extents exceed neutral, which exceed far;
- perspective and projection move in the same depth direction;
- projection remains below props/tank life;
- HUD and gauge coordinates are identical across pinned depth;
- no clipping enters the aperture, gauge, or HUD reserves.

- [ ] **Step 5: Run a live multi-second Smooth smoke**

```bash
cargo xtask companion fresh
```

Let the companion run for at least 30 seconds while watching one full near/far transition. Confirm motion is continuous, direction-facing behavior remains correct, no flashing occurs, and the app does not crash. Inspect the native review log for bounded adjacent scale deltas.

- [ ] **Step 6: Final repository audit**

```bash
git status --short
git log --oneline -10
git diff --check
rg -n "TODO|FIXME" docs/superpowers/plans/2026-07-09-glorp-smooth-tank-depth-implementation.md src/round/depth.rs src/round/tank_bed.rs src/round/smooth.rs src/presentation/smooth.rs src/companion/app.rs
rg -n "FloorTexture|floor-texture|SMOOTH_SUBSTRATE|smooth_floor_texture_layer" src tests
```

Expected: only intentional files are changed, no unfinished implementation
markers remain, and the failed floor implementation is absent from source and
tests.

## Completion Checklist

- [ ] The curved bed reads at `360x360` without becoming a footer or HUD bar.
- [ ] Glorp scales continuously from `0.88x` far to `1.12x` near in normal lifecycle.
- [ ] Calm and asleep depth excursions are attenuated to half and quarter.
- [ ] Scale, perspective Y, and floor projection all derive from one Z sample.
- [ ] Pet body, wall shadow, performance cue, and mood aura share the composed transform.
- [ ] Projection remains below every prop/tank-life layer and does not inherit bob.
- [ ] Max-scale-safe bounds protect aperture, gauges, and HUD.
- [ ] AppKit renders typed ellipses and coherent uniform cell scaling.
- [ ] Preview and native evidence expose the same typed geometry and depth contracts.
- [ ] Classic output and `literal_floor_allowed: false` remain unchanged.
- [ ] `FloorTexture` and its glyph/background fallback are gone.
- [ ] Focused tests, Preview Lab, native captures, and 30-second smoke all pass.
