# Retained Floor Silhouette Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the retained renderer's generic radial floor ellipse with a
recognizable, horizontally facing, perspective-squashed projection of the
exact pet-body glyph mask.

**Architecture:** Preserve `FloorProjection` semantic slot 2, its node,
ordering, depth-derived destination rectangle, lifecycle opacity, and multiply
blend. Change only its typed shape/paint and route it through a second pet-mask
glyph draw. Reuse the existing 130 pet-body content records and glyph atlas;
add no pass, texture, buffer, or resize-managed resource. Keep Smooth and
Classic renderers unchanged.

**Tech Stack:** Rust scene contracts and compiler, WGPU render pipeline, WGSL,
native macOS offscreen readback, Cargo tests, Preview Lab.

## Global Constraints

- Follow the approved design in
  `docs/superpowers/specs/2026-07-15-glorp-retained-floor-silhouette-design.md`.
- Keep `floor_projection_metrics()` unchanged; this is a coverage change, not
  a depth-motion change.
- Use only `PetArtFilter::Body`; particles must not cast the floor silhouette.
- The floor projection must use its own node transform so it never inherits
  body bob or breath.
- Preserve slot 2's authored order, world depth, multiply blend, and effective
  opacity.
- Keep the existing numeric analytic-shape/checksum tag `3` for slot 2 while
  changing its typed meaning and payload.
- Fail closed on mismatched semantic, shape, mask, facing, instance source, or
  pipeline.
- Do not touch Smooth radial parity or Classic contact-shadow behavior.

---

## Task 1: Lock the silhouette behavior with a native GPU regression

**Files:**

- Modify: `src/companion/retained/render.rs`

- [ ] Add a focused macOS-only readback test named
  `floor_projection_tracks_asymmetric_pet_mask_and_facing` next to the existing
  rear-wall tint readback.

Build a deterministic retained snapshot whose pet-body mask is visibly
asymmetric inside the 13x10 lattice. Render two otherwise identical scenes:

1. room plus the production floor projection; and
2. room alone, with slot 2's node hidden.

Read slot 2's accepted `AnalyticFrame.rect_points` and divide it into 13x10
destination cells. Sample small inset ROIs for:

- an occupied cell on the asymmetric side;
- its empty horizontal mirror cell; and
- one occupied cell above/below it to prove the ten source rows are flattened
  into the floor rectangle rather than uniformly fitted.

The assertions must be based on paired pixel deltas, not absolute colors:

```rust
let occupied_delta = mean_linear_luma_drop(&shadowed, &room_only, occupied_roi);
let empty_delta = mean_linear_luma_drop(&shadowed, &room_only, empty_mirror_roi);
assert!(occupied_delta > 0.01, "occupied pet-mask ink must darken the bed");
assert!(
    empty_delta < occupied_delta * 0.20,
    "empty cells inside the old ellipse must remain effectively unchanged"
);
```

Create the same fixture with the facing sign reversed and assert that the two
horizontal ROI deltas swap while the floor rectangle center remains unchanged.
Also assert that the observed changed-pixel Y range stays within slot 2's floor
rectangle (allow one physical pixel for atlas antialiasing).

- [ ] Run the focused test and confirm RED:

```bash
cargo test --all-features floor_projection_tracks_asymmetric_pet_mask_and_facing -- --nocapture
```

Expected failure: the occupied and empty mirrored ROIs receive the same radial
darkening, or reversing facing does not mirror coverage. A compile error or a
fixture that cannot reach the production retained draw is not an acceptable
RED; fix the test harness until it fails on the current ellipse behavior.

- [ ] Keep the proven-RED test uncommitted until Task 3 is green. This work is
  happening directly on `main`; do not create an intentionally broken commit.

---

## Task 2: Type the floor projection as a pet-mask silhouette

**Files:**

- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/presentation/companion_scene/scene/checksum.rs`
- Test: existing unit tests in those modules

- [ ] Replace the radial slot-2 contract with explicit types:

```rust
pub enum AnalyticShape {
    // existing shapes ...
    PetFloorProjection,
}

pub enum AnalyticGeometry {
    // existing geometry ...
    PetFloorProjection {
        mask: AnalyticMaskSource,
        facing: i8,
    },
}

pub enum AnalyticPaint {
    // existing paints ...
    FloorShadowMultiplySilhouette {
        color_srgba8: [u8; 4],
    },
}
```

`AnalyticSemantic::FloorProjection::shape()` must return
`PetFloorProjection`. Keep its shape/checksum domain value at `3`; do not leave
an unused radial slot-2 compatibility path.

- [ ] In `project_analytic_frame_slots_for_geometry()`, continue using the
current `floor_projection_metrics()` result to construct `rect_points`, but
emit:

```rust
AnalyticFrame {
    semantic: AnalyticSemantic::FloorProjection,
    shape: AnalyticShape::PetFloorProjection,
    rect_points: floor_rect,
    geometry: AnalyticGeometry::PetFloorProjection {
        mask: AnalyticMaskSource::PetBody,
        facing: snapshot.frame.facing,
    },
}
```

Use the actual projected snapshot field/type for facing and normalize it to
exactly `-1` or `1` at the existing projection boundary if needed.

- [ ] Replace the radial inner/outer paint with one existing biome bed-shadow
color `[r, g, b, 235]`. Keep the floor node opacity equal to
`metrics.alpha / 235.0`, so the shader's packed paint alpha multiplied by the
node opacity still equals the current effective `metrics.alpha / 255.0`.

- [ ] Tighten validation:

  - slot 2 accepts only `PetFloorProjection` geometry;
  - `mask` must be `AnalyticMaskSource::PetBody`;
  - `facing` must be exactly `-1` or `1`;
  - slot 2 accepts only `FloorShadowMultiplySilhouette` with nonzero alpha;
  - all mismatched semantic/shape/paint cases remain rejected.

- [ ] Update deterministic checksum encoding. Reuse tag `3`, then encode the
mask tag and signed facing byte for geometry, and one packed RGBA value for
paint. Update exact checksum fixtures rather than weakening them.

- [ ] Update scene/compiler/validator tests to assert the new typed contract
while preserving existing coverage for far/neutral/near depth, active/calm/
asleep lifecycle opacity, authored order, no-bob anchoring, fixed slot count,
and fail-closed mismatches.

- [ ] Run the contract layer tests:

```bash
cargo test --all-features companion_scene::scene
cargo test --all-features companion_scene::validate
cargo test --all-features companion_scene::scene::compiler
```

Expected: all pass. The native pixel regression remains RED until the retained
GPU path is routed in Task 3.

- [ ] Keep the typed scene-contract changes uncommitted until the renderer is
  green in Task 3. Do not leave slot 2 typed for a glyph path while production
  still routes it through the radial pipeline.

---

## Task 3: Route slot 2 through the multiply glyph-mask pipeline

**Files:**

- Modify: `src/companion/retained/compiler.rs`
- Modify: `src/companion/retained/render.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Test: existing unit/native tests in `src/companion/retained/render.rs`

- [ ] Pack the new contract without changing fixed capacities:

  - `analytic_shape_tag(PetFloorProjection)` returns `3`;
  - slot 2 content payload 0 is the packed silhouette RGBA;
  - slot 2 frame payload stores mask tag and signed facing;
  - slot 2's `aux_content_base` points at the existing pet-body content base,
    just like wall slot 1;
  - no auxiliary node is required for slot 2 because its primary floor node is
    the transform authority.

- [ ] Extend retained draw typing:

```rust
enum InstanceSource {
    // existing sources ...
    FloorShadowGlyphMask,
}

enum ScenePipelineClass {
    // existing classes ...
    WorldMultiplyGlyphMask,
}
```

Route only analytic binding 2 to 130 instances of
`FloorShadowGlyphMask`. Keep analytic binding 8 (`PropShadows`) on
`WorldMultiplyAnalytic`; keep wall binding 1 on `WorldSourceOverGlyphMask`.
`WorldMultiplyGlyphMask` uses `vs_world_glyph`, the new floor fragment entry,
multiply blending, world read-only depth, and the existing glyph atlas/bind
groups.

- [ ] Update the production pipeline matrix and exact counts:

  - `PrimitiveSource::Analytic` drops from 8 to 7;
  - `WorldMultiplyAnalytic` drops from 2 to 1 (binding 8 only);
  - `WorldMultiplyGlyphMask` has exactly 1 draw (binding 2);
  - `FloorShadowGlyphMask` has exactly 130 instances;
  - `WallShadowGlyphMask` remains exactly 130 instances.

Keep selector tests fail-closed: substituting the wall source, ordinary pet
source, analytic source, wrong blend, wrong binding, or wrong instance range
must return `None`.

- [ ] Add an anisotropic glyph helper in WGSL; do not reuse the normal uniform
cell fit:

```wgsl
fn projected_metric_ink_offset(
    quad_corner: vec2<f32>,
    entry: GlyphAtlasGpuEntry,
    destination_cell_extent: vec2<f32>,
) -> vec2<f32> {
    let scale = destination_cell_extent / entry.metrics.xy;
    return entry.ink_origin_size.xy * scale
        + quad_corner * entry.ink_origin_size.zw * scale;
}
```

In `glyph_instance_placement()`, recognize analytic binding 2 as `is_floor`,
load its 130 records from `aux_content_base`, require body content, and map
each instance into the slot-2 rectangle:

```wgsl
let floor_cell = analytic.rect_points.zw / vec2<f32>(13.0, 10.0);
let source_col = input.instance_index % 13u;
let source_row = input.instance_index / 13u;
let facing = i32(round(analytic.payload[0].y));
let projected_col = select(12u - source_col, source_col, facing > 0);
let base = analytic.rect_points.xy + vec2<f32>(
    f32(projected_col) * floor_cell.x,
    f32(9u - source_row) * floor_cell.y,
);
let local_xy = base
    + projected_metric_ink_offset(input.local_position.xy, entry, floor_cell);
```

Use the actual packed payload lanes established in the Rust packer. Validate
the analytic ID, shape, mask tag, facing, frame bounds, and atlas entry before
placing any vertex. Transform `local_xy` with the primary floor node only.
This preserves slot 2's depth and opacity without body bob/breath.

- [ ] Add `fs_floor_shadow_glyph`. It must validate slot 2, sample glyph alpha
coverage using the same monochrome/color-glyph rules as the wall mask, unpack
the floor paint, and return premultiplied source for the existing multiply
blend. Leave `fs_wall_shadow_glyph` unchanged.

- [ ] Run focused retained tests, including the original RED test:

```bash
cargo test --all-features floor_projection_tracks_asymmetric_pet_mask_and_facing -- --nocapture
cargo test --all-features companion::retained::render
cargo test --all-features companion::retained::compiler
```

Expected: the asymmetric readback is GREEN; exact selector, capacity, ABI,
shader-entry, and upload-phase tests all pass.

- [ ] Commit the regression, scene contract, and retained GPU path together:

```bash
git add \
  src/presentation/companion_scene/scene.rs \
  src/presentation/companion_scene/scene/compiler.rs \
  src/presentation/companion_scene/validate.rs \
  src/presentation/companion_scene/scene/checksum.rs \
  src/companion/retained/compiler.rs \
  src/companion/retained/render.rs \
  src/companion/retained/scene.wgsl
git commit -m "feat(companion): project pet silhouette onto floor"
```

---

## Task 4: Verify the complete renderer and relaunch

**Files:**

- Verify only unless a focused defect is found.

- [ ] Run formatting and static checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

- [ ] Run focused preview/round contracts and the full library suite:

```bash
cargo test --features dev-preview --test dev_preview
cargo test --test round_scene
cargo test --all-features --lib
```

- [ ] Generate the retained round visual review bundle:

```bash
cargo run -- dev-preview --scenario round --out target/glorp-preview-floor-silhouette
```

Inspect the retained round full-cast output and confirm:

- the floor shadow is recognizably derived from the current pet;
- it is visibly flattened and bed-anchored;
- it does not inherit pet bob/breath;
- facing mirrors without recentering;
- props and tank life still draw above it;
- wall shadow, ground texture, vines, and aperture remain unchanged.

- [ ] Inspect the final diff and commit graph. Do not create an empty cleanup
commit. If verification exposes a scoped defect, add a regression, fix it, and
commit that fix separately.

- [ ] Rebuild and relaunch the optimized companion for Drew's manual review:

```bash
cargo xtask companion fresh
```

Do not automate fullscreen or window movement. Report the launched PID and
leave resize/fullscreen/display checks to manual review.
