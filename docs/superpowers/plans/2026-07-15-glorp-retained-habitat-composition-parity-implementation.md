# Glorp Retained Habitat Composition Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the retained companion's complete prop composition, substrate texture, depth separation, contact shadows, and tank clearance without changing renderer lifecycle, canonical habitat state, HUD, gauges, or fallback policy.

**Architecture:** Keep `CompanionSceneSnapshot` as the direct route's sole scene authority. Consolidate prop glyph art and maximum footprints in presentation code, resolve one deterministic cell-space `CompanionComposition`, project that frozen composition into fixed retained scene slots, and add the bed/shadow treatment through the existing analytic pipeline and fixed GPU frame ABI.

**Tech Stack:** Rust 2021, the existing Glorp presentation/round modules, retained scene contracts, WGSL, wgpu native offscreen tests, Cargo integration tests, and Preview Lab.

## Global Constraints

- The approved contract is [the retained habitat composition parity design](../specs/2026-07-15-glorp-retained-habitat-composition-parity-design.md). If implementation evidence contradicts a locked decision, stop and amend the design with Drew before changing architecture.
- Do not add a second scene authority, a general packing engine, per-prop GPU resources, texture assets, or a new animation clock.
- Preserve canonical prop inventory/order, tank cast/routes/cadence/layers, pet art, HUD, gauges, privacy boundaries, renderer selection, resize/fullscreen lifecycle, and Smooth fallback.
- Composition is deterministic in logical glyph-cell space. Backing scale and semantic animation state cannot change accepted prop anchors.
- Keep fixed capacities: ten prop slots and the existing analytic capacity. Hide an unplaceable accent by setting its existing slot `visible = false`.
- Use the existing monotonic presentation clock and reduce-motion/lifecycle policies. Never introduce wall-clock animation or frame-to-frame accumulation.
- Before every commit, inspect `git status --short`, run the named focused tests, and run `git diff --check`. Stage only the files listed for that task.

---

## Task 1: Make Prop Art And Maximum Footprints Canonical

**Files:**

- Modify: `src/presentation/props.rs`
- Modify: `src/tui/component/habitat_props.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Verify unchanged integration surface: `tests/presentation_props.rs`

### Contract

`src/presentation/props.rs` becomes the only catalog-to-glyph mapping. The TUI and direct retained compiler remain paint/layout adapters; they must not retain independent `match catalog_id` sprite definitions.

Add these crate-visible types and functions:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropVisualState {
    pub(crate) species: Species,
    pub(crate) sprite_phase: Option<u8>,
    pub(crate) twinkle_active: Option<bool>,
    pub(crate) chest_lid_open: Option<bool>,
    pub(crate) bloom_active: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropSpriteCell {
    pub(crate) dx: i8,
    pub(crate) dy: i8,
    pub(crate) glyph: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropFootprint {
    pub(crate) min_dx: i8,
    pub(crate) max_dx: i8,
    pub(crate) min_dy: i8,
    pub(crate) max_dy: i8,
}

pub(crate) fn presentation_prop_sprite(
    catalog_id: &str,
    state: PresentationPropVisualState,
) -> Option<Vec<PresentationPropSpriteCell>>;

pub(crate) fn presentation_prop_max_footprint(
    catalog_id: &str,
) -> Option<PresentationPropFootprint>;
```

### Test-first steps

- [ ] Add `canonical_prop_states_fit_their_frozen_footprints` to the unit tests in `src/presentation/props.rs`. Enumerate every prop catalog ID, every `Species::all()` entry, both boolean values for twinkle/lid/bloom, and every sprite phase accepted by that catalog. Assert every returned `(dx, dy)` lies inside `presentation_prop_max_footprint` and every catalog entry has a nonempty sprite.
- [ ] Add `tui_sprite_adapter_preserves_canonical_local_cells` to `src/tui/component/habitat_props.rs` and `retained_sprite_adapter_preserves_canonical_local_cells` to `src/presentation/companion_scene/scene/compiler.rs`. For each catalog/state, compare the adapter's `(dx, dy, glyph)` values with `presentation_prop_sprite` before paint or absolute placement is applied.
- [ ] Run `cargo test presentation::props::tests::canonical_prop_states_fit_their_frozen_footprints -- --exact`. Expected result: compilation fails because the shared types/functions do not exist.
- [ ] Move the existing `SpriteCell` data and catalog match from `src/tui/component/habitat_props.rs` into `src/presentation/props.rs` mechanically. Preserve every glyph, local coordinate, species dialect, phase, twinkle, lid, and bloom branch.
- [ ] Implement maximum footprints as the union of every authored state, using one private state enumerator shared by the footprint tests and implementation. Do not calculate the footprint from the current frame.
- [ ] Adapt `src/tui/component/habitat_props.rs` to call `presentation_prop_sprite` and translate local cells into `HabitatPropCell`. Keep paint, boldness, reaction effects, collision handling, and target IDs in the TUI module.
- [ ] Replace the parallel direct `prop_glyphs` catalog match in `src/presentation/companion_scene/scene/compiler.rs` with the shared sprite call. Keep direct paint and scene content construction in the compiler.
- [ ] Run `cargo test --test presentation_props` and the focused compiler unit tests with `cargo test --features retained-renderer presentation::companion_scene::scene::compiler::tests`. Expected result: all pass.
- [ ] Run `rg -n "fn (trophy_sprite|prop_glyphs)|match catalog_id" src/tui/component/habitat_props.rs src/presentation/companion_scene/scene/compiler.rs`. Inspect every match; none may be a second prop sprite catalog.
- [ ] Run `cargo fmt --check` and `git diff --check`.
- [ ] Commit only these files:

```bash
git add src/presentation/props.rs \
  src/tui/component/habitat_props.rs \
  src/presentation/companion_scene/scene/compiler.rs
git commit -m "refactor(presentation): share canonical habitat prop art"
```

---

## Task 2: Resolve One Safe, Deterministic Companion Composition

**Files:**

- Create: `src/presentation/companion_scene/composition.rs`
- Modify: `src/presentation/companion_scene/mod.rs`
- Modify: `src/presentation/companion_scene/input.rs`
- Modify: `tests/retained_scene.rs`

### Contract

Add the pure cell-space composition API below. `resolve_companion_composition` consumes topology plus layout only; it must not accept current prop animation state, pet position, private HUD strings, backing scale, or time.

```rust
pub(crate) struct CompanionCompositionInput<'a> {
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) width_points: f32,
    pub(crate) height_points: f32,
    pub(crate) bottom_reserved_rows: u16,
    pub(crate) props: &'a [PropTopologySnapshot],
}

pub(crate) struct CompanionPropPlacement {
    pub(crate) slot: u8,
    pub(crate) visible: bool,
    pub(crate) anchor_cell: [i16; 2],
    pub(crate) bounds_cells: [i16; 4],
    pub(crate) footprint_cells: [u16; 2],
    pub(crate) grounded: bool,
}

pub(crate) struct CompanionComposition {
    pub(crate) prop_placements: Vec<CompanionPropPlacement>,
    pub(crate) hud_reserve_cells: [i16; 4],
    pub(crate) gauge_inner_radius_cells: [f32; 2],
    pub(crate) tank_reserved_regions: Vec<TankRouteRect>,
    pub(crate) tank_foreground_reserved_regions: Vec<TankRouteRect>,
}

pub(crate) fn resolve_companion_composition(
    input: CompanionCompositionInput<'_>,
) -> CompanionComposition;
```

Use half-open cell bounds `[min_col, min_row, max_col, max_row]` everywhere. Convert to `TankRouteRect` only after bounds are accepted, so collision and route semantics cannot disagree at the edge.

### Test-first steps

- [ ] Add table-driven unit tests in `composition.rs` for 260, 360, 480, and 720 point squares plus 480x360 and 360x480. Use the real full-prop fixture topology and the logical grid produced by scene projection.
- [ ] Name and cover these invariants: `full_cast_props_are_disjoint_and_inside_safe_aperture`, `props_avoid_hud_and_bottom_reserves`, `composition_is_byte_stable`, `sprite_phase_does_not_move_prop_anchors`, and `small_surfaces_hide_accents_in_stable_order`.
- [ ] In `tests/retained_scene.rs`, add `retained_snapshot_keeps_fixed_prop_slots_when_composition_hides_an_accent`. Assert the prop topology/content count remains fixed while the small-layout frame marks the same lowest-priority slot invisible on repeated projections.
- [ ] Run `cargo test --features retained-renderer presentation::companion_scene::composition::tests`. Expected result: compilation fails because the module and resolver do not exist.
- [ ] Register `pub(crate) mod composition;` in `src/presentation/companion_scene/mod.rs`.
- [ ] Port the proven authored-zone candidate anchors from `src/tui/component/habitat_props.rs` into a private, deterministic `candidate_anchors(zone, columns, rows)` function. Include all left/mid/right, wall, air, and ceiling alternatives in stable order; do not call the TUI placement function.
- [ ] Derive the center HUD reserve as a centered rectangle covering 58% of logical width and rows from 58% through 90% of logical height. Round outward so glyphs cannot leak into the reserve.
- [ ] Call `crate::presentation::companion_effects::perimeter_gauge_layout` and compute the inner safe ellipse from the innermost lane radius minus half its stroke and half a glyph cell. Express the result in column/row radii.
- [ ] Resolve props greedily in existing canonical visible order. For each candidate, translate `presentation_prop_max_footprint` to half-open bounds and reject grid, ellipse, HUD, bottom, and accepted-bounds-plus-one-cell-gutter collisions. When all candidates fail, append the fixed slot with `visible = false` instead of moving another prop.
- [ ] Derive `grounded` from floor zones only. Populate `tank_reserved_regions` with HUD and bottom rectangles. Populate `tank_foreground_reserved_regions` with accepted foreground prop rectangles expanded by the one-cell gutter.
- [ ] In `src/presentation/companion_scene/input.rs`, compute one `CompanionComposition` at the start of initial and incremental projection. Remove `resolved_prop_origin`; pass composition forward without yet changing tank/depth effects beyond safe placement.
- [ ] Run `cargo test --features retained-renderer presentation::companion_scene::composition::tests`, `cargo test --features retained-renderer --test retained_scene`, and `cargo test --test round_scene`. Expected result: all pass.
- [ ] Run `cargo fmt --check` and `git diff --check`.
- [ ] Commit only these files:

```bash
git add src/presentation/companion_scene/composition.rs \
  src/presentation/companion_scene/mod.rs \
  src/presentation/companion_scene/input.rs \
  tests/retained_scene.rs
git commit -m "feat(companion): resolve safe habitat composition"
```

---

## Task 3: Project Frozen Placement, Tank Clearance, And Depth Cues

**Files:**

- Modify: `src/presentation/companion_scene/mod.rs`
- Modify: `src/presentation/companion_scene/input.rs`
- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/scene/checksum.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/presentation/tank_life.rs`
- Modify: `src/companion/retained/compiler.rs`
- Modify: `tests/retained_scene.rs`
- Modify: `tests/round_scene.rs`
- Modify: `tests/smooth_companion.rs`

### Contract

Extend both `PropFrameSnapshot` and retained `PropFrameSlot`:

```rust
pub visible: bool,
pub footprint_points: [f32; 2],
pub contact_shadow_strength: f32,
```

Pack the new values into the existing unused lanes without changing `FrameGpuValue`:

```rust
values: [
    value.origin_points[0],
    value.origin_points[1],
    value.motion_offset_points[0],
    value.motion_offset_points[1],
    value.opacity,
    value.footprint_points[0],
    value.footprint_points[1],
    value.contact_shadow_strength,
]
```

Depth values are fixed:

| Depth | Parallax | Opacity | Saturation |
|---|---:|---:|---:|
| Background | 0.010 | 0.82 | 0.78 |
| Behind pet | 0.030 | 0.94 | 0.90 |
| Foreground | 0.045 | 1.00 | 1.05 |

All node depth cues use `scale = 1.0`. Parallax is a frame offset derived from pet displacement, never a transform around the room origin.

### Test-first steps

- [ ] Add `prop_frame_pack_uses_existing_width_height_shadow_lanes` beside `pack_prop_frame` tests in `src/companion/retained/compiler.rs`. Assert the packed record remains the same byte size and lanes 5, 6, and 7 contain width, height, and strength.
- [ ] Add input projection tests named `prop_placement_is_frozen_across_semantic_animation`, `depth_parallax_is_bounded_to_half_a_cell`, and `reduce_motion_zeroes_prop_and_tank_parallax`. Exercise nonzero pet displacement from `motion_origin_top_left_cells` and all three depth buckets.
- [ ] Add scene compiler/validator tests proving prop and tank nodes receive the exact opacity/saturation table, retain authored Z order, and keep scale 1.0.
- [ ] Extend the tank tests in `tests/round_scene.rs` with `tank_routes_avoid_composition_chrome_and_foreground_props`. Assert every resolved cell is inside the gauge-safe aperture; all layers avoid HUD/bottom; foreground cells also avoid pet-face and foreground-prop rectangles.
- [ ] Run `cargo test --features retained-renderer prop_frame_pack_uses_existing_width_height_shadow_lanes`. Expected result: compilation fails because frame structs lack the new fields.
- [ ] Add the fields to `PropFrameSnapshot` and `PropFrameSlot`, then update all constructors/fixtures with honest values. Hidden props keep their stable slot/content but use `visible = false`, zero opacity contribution, and zero shadow strength.
- [ ] Change `project_prop_frame_states` to accept `&CompanionComposition`. Convert accepted cell anchors/footprints to logical points using `CompanionGlyphGrid`; never recompute from the current sprite cells.
- [ ] Compute pet displacement in logical points from current minus origin motion cells. Multiply by the depth table, attenuate with the existing lifecycle motion scale, clamp each axis to half a cell, and return zero under reduce motion. Add this offset to existing sway/hover/two-pose motion rather than replacing semantic motion.
- [ ] Apply the same bounded parallax helper to each tank cell based on resolved `TankRouteLayer`.
- [ ] In scene compilation, assign the exact opacity/saturation table to prop and tank `NodeTemplate.depth_cue` values with `scale = 1.0` and `y_offset_points_up = 0.0`.
- [ ] Augment `TankRouteGeometry` from `CompanionComposition`: use its gauge-safe aperture, append HUD/bottom to `reserved_regions`, append accepted foreground prop bounds plus `pet_face_reserved_region` to `foreground_reserved_regions`, and preserve `literal_floor_allowed = false`.
- [ ] Update scene checksum and validation for the new prop-frame fields: values must be finite, footprint dimensions nonnegative, hidden slots must have zero shadow, and strength must be in `[0.0, 1.0]`.
- [ ] Update every changed-frame comparison/delta path so changes to visibility, footprint, or shadow strength produce a prop frame update without forcing topology/content rebuilds.
- [ ] Run:

```bash
cargo test --features retained-renderer prop_frame_pack_uses_existing_width_height_shadow_lanes
cargo test --features retained-renderer presentation::companion_scene::input::tests
cargo test --features retained-renderer presentation::companion_scene::scene::tests
cargo test --features retained-renderer --test retained_scene
cargo test --test round_scene
cargo test --test smooth_companion
```

Expected result: all pass, including existing tank cadence/cast and Smooth parity tests.

- [ ] Run `cargo fmt --check` and `git diff --check`.
- [ ] Commit only these files:

```bash
git add src/presentation/companion_scene/mod.rs \
  src/presentation/companion_scene/input.rs \
  src/presentation/companion_scene/scene.rs \
  src/presentation/companion_scene/scene/compiler.rs \
  src/presentation/companion_scene/scene/checksum.rs \
  src/presentation/companion_scene/validate.rs \
  src/presentation/tank_life.rs \
  src/companion/retained/compiler.rs \
  tests/retained_scene.rs \
  tests/round_scene.rs \
  tests/smooth_companion.rs
git commit -m "feat(companion): restore habitat depth and tank clearance"
```

---

## Task 4: Restore The Textured Biome Bed In The Room Analytic

**Files:**

- Modify: `src/presentation/companion_effects.rs`
- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/scene/checksum.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/companion/retained/compiler.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Modify: `src/companion/retained/render.rs`

### Contract

Extend room paint without adding a texture resource:

```rust
AnalyticPaint::ApertureDepth {
    core_srgb8: [u8; 3],
    rim_srgb8: [u8; 3],
    bed_srgb8: [u8; 3],
    fleck_srgb8: [u8; 3],
}
```

`fs_room_aperture` must preserve the current radial room falloff, add a curved bed beginning near 76% of logical height, and use a deterministic logical-point/backing-scale hash for dither and sparse lower-bed flecks. No time, revisions, pet position, or usage value may enter the hash.

### Test-first steps

- [ ] In `src/presentation/companion_effects.rs`, add table tests proving every biome produces deterministic `bed_primary_srgb8`, `bed_shadow_srgb8`, and a new `bed_fleck_srgb8` distinct enough to be visible without exceeding the room palette.
- [ ] Add a CPU reference helper for the bed mask/hash sample and tests named `bed_texture_is_stable_for_same_logical_sample`, `bed_texture_changes_with_biome`, and `upper_room_samples_never_emit_flecks`. Use the same integer hash constants as `tank_dither_noise`.
- [ ] In `src/companion/retained/render.rs`, add shader-contract tests requiring the room analytic to consume packed bed/fleck colors and backing scale, and forbidding presentation time/revision frame lanes in `fs_room_aperture`.
- [ ] Add native offscreen tests `retained_bed_lower_roi_has_stable_texture_variance` and `retained_bed_upper_roi_has_no_substrate_flecks`. Render the same scene twice at 1x and compare bytes; calculate lower-ROI variance after subtracting a smooth vertical/radial trend, and assert the upper ROI stays at the smooth-room reference.
- [ ] Run `cargo test --features retained-renderer retained_bed_lower_roi_has_stable_texture_variance`. Expected result: the new test fails because the current shader produces only the smooth fade.
- [ ] Add `bed_fleck_srgb8` and the CPU bed-sample reference. Reuse the biome alias and palette helpers; do not duplicate biome colors in WGSL.
- [ ] Extend `AnalyticPaint::ApertureDepth`, scene checksum encoding, validation, compiler packing, and all fixtures. Pack core/rim/bed/fleck into existing analytic content payload lanes; do not change bind-group or uniform sizes.
- [ ] Update `fs_room_aperture` to:
  - compute physical hash coordinates from logical point times backing scale;
  - apply the existing deterministic dither range before output quantization;
  - form a curved horizon around 76% of logical height;
  - blend the lifted biome bed below the horizon; and
  - emit sparse flecks only inside the lower mask, with density/strength increasing toward the near edge.
- [ ] Keep the analytic premultiplied-alpha and saturation path unchanged after the new straight color is calculated.
- [ ] Run:

```bash
cargo test --features retained-renderer bed_texture_
cargo test --features retained-renderer retained_bed_
cargo test --features retained-renderer presentation::companion_scene::scene::tests
cargo test --features retained-renderer presentation::companion_scene::validate::tests
```

Expected result: all pass and repeated offscreen renders are byte-stable.

- [ ] Run `cargo fmt --check` and `git diff --check`.
- [ ] Commit only these files:

```bash
git add src/presentation/companion_effects.rs \
  src/presentation/companion_scene/scene.rs \
  src/presentation/companion_scene/scene/compiler.rs \
  src/presentation/companion_scene/scene/checksum.rs \
  src/presentation/companion_scene/validate.rs \
  src/companion/retained/compiler.rs \
  src/companion/retained/scene.wgsl \
  src/companion/retained/render.rs
git commit -m "feat(renderer): restore textured retained tank bed"
```

---

## Task 5: Ground Floor Props With One Analytic Shadow Field

**Files:**

- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/scene/checksum.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/companion/retained/compiler.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Modify: `src/companion/retained/render.rs`
- Modify: `tests/retained_scene.rs`

### Contract

Add one fixed analytic, not one draw/resource per prop:

```rust
AnalyticSemantic::PropShadows // id 8
AnalyticShape::PropShadowField
AnalyticPaint::PropShadowMultiply { color_srgb8: [u8; 3] }
AnalyticGeometry::PropShadowField
```

The full-room fragment loops the ten fixed prop frame records. It draws only visible floor props with positive `contact_shadow_strength`. The ellipse is 75% of frozen footprint width with at least a one-cell radius and has 0.30-cell height. Behind strength is `0.24`, foreground strength is `0.34`, and background/non-floor/hidden strength is zero.

### Test-first steps

- [ ] Add scene contract tests asserting `AnalyticSemantic::PropShadows.id() == AnalyticParamId(8)`, shape/paint/geometry agree, and all analytic IDs remain unique and within `MAX_ANALYTIC_PARAMS`.
- [ ] Add compiler tests proving the prop-shadow draw is ordered after room/floor projection and before behind-pet prop/tank draws, and that it uses the existing multiply blend pipeline.
- [ ] Add validation tests rejecting non-full-room shadow geometry, a non-multiply shadow paint, out-of-range shadow strength, and a hidden prop with nonzero strength. Add the grounded/depth-to-strength cases to `src/presentation/companion_scene/input.rs`, where prop zone and authored depth are both available.
- [ ] Add a Rust/WGSL layout test that proves the shader's prop-frame base and stride match the compiler constants. Do not hard-code a second unexplained base inside WGSL.
- [ ] Add native render tests named `prop_shadow_field_darkens_bed_without_tinting_glyphs` and `hidden_or_non_floor_props_emit_no_contact_shadow`. Compare a controlled bed ROI with shadow strength zero and nonzero; assert multiply darkening under the footprint and identical prop glyph pixels.
- [ ] Run `cargo test --features retained-renderer prop_shadow_field_darkens_bed_without_tinting_glyphs`. Expected result: compilation fails because the shadow semantic/shape/paint do not exist.
- [ ] Add semantic id 8 to `AnalyticSemantic::ALL`, map it to `PropShadowField`, and extend analytic shape/paint/geometry, checksum, compiler, validator, and exhaustive matches.
- [ ] Insert a full-room prop-shadow analytic node/draw at the specified order. Use `bed_shadow_srgb8` for `PropShadowMultiply` and the standard multiply blend state already used by floor shadows.
- [ ] Set `contact_shadow_strength` during prop frame projection from accepted placement and authored depth: `0.24` for grounded behind props, `0.34` for grounded foreground props, zero otherwise. Preserve zero for hidden slots.
- [ ] Define compiler-owned `PROP_FRAME_GPU_BASE` and `PROP_FRAME_GPU_STRIDE` constants from the packed frame family order. Declare matching WGSL constants and make the Rust/WGSL contract test parse and compare them, then have WGSL iterate exactly ten records and decode visibility, origin, motion offset, footprint, and strength.
- [ ] In WGSL, union the soft ellipse coverage for eligible slots, clamp the resulting multiply coverage, and return premultiplied shadow color. Use 75% footprint width, minimum one cell radius, and 0.30 cell height.
- [ ] Run:

```bash
cargo test --features retained-renderer prop_shadow_
cargo test --features retained-renderer presentation::companion_scene::scene::tests
cargo test --features retained-renderer presentation::companion_scene::validate::tests
cargo test --features retained-renderer --test retained_scene
```

Expected result: all pass; the render test observes bed darkening without prop tint or extra per-prop draws.

- [ ] Run `cargo fmt --check` and `git diff --check`.
- [ ] Commit only these files:

```bash
git add src/presentation/companion_scene/scene.rs \
  src/presentation/companion_scene/scene/compiler.rs \
  src/presentation/companion_scene/scene/checksum.rs \
  src/presentation/companion_scene/validate.rs \
  src/companion/retained/compiler.rs \
  src/companion/retained/scene.wgsl \
  src/companion/retained/render.rs \
  tests/retained_scene.rs
git commit -m "feat(renderer): ground retained habitat props"
```

---

## Task 6: Qualify The Complete Composition And Prepare Manual Review

**Files:**

- Modify: `src/companion/retained/render.rs`
- Modify: `tests/retained_scene.rs`
- Modify: `tests/retained_renderer_boundary.rs`
- Modify: `tests/round_scene.rs`
- Modify: `tests/smooth_companion.rs`
- Modify if fixture coverage is missing: `src/dev_preview/round.rs`
- Generated, not committed: `target/glorp-preview-retained-composition/`

### Test-first steps

- [ ] Add one table-driven `retained_full_cast_composition_matrix` covering 260, 360, 480, and 720 squares plus 480x360 and 360x480. For every size assert: finite/in-aperture footprints, pairwise disjoint visible bounds, HUD/bottom clearance, deterministic repeated projection, fixed placement across semantic phase, and deterministic accent hiding.
- [ ] Add `retained_full_cast_rois_are_nonblank_at_one_and_two_x` in native render tests. Use `WatchViewModel::fixture_with_tank_inhabitants_for_age`, merge its tank cast with `fixture_with_habitat_props`, and assert distinct nonblank ROIs for bed, props, and tank at both backing scales.
- [ ] Add `habitat_pass_does_not_change_hud_or_gauge_pixels`. Render controlled scenes with habitat effects toggled through test-only fixture construction while leaving HUD/gauge inputs identical; compare masks around all three lanes and the center HUD glyphs.
- [ ] Extend `tests/retained_renderer_boundary.rs` to assert no new presentation/composition code owns a wgpu device, surface, event loop, renderer choice, fallback, or resize/fullscreen policy.
- [ ] Run the new matrix and native ROI tests before the final fixture/test wiring. Expected result: at least one test fails until every full-cast fixture and ROI is connected.
- [ ] If the existing round Preview Lab full-cast fixture does not include both maximum props and the mature tank cast, add one deterministic `round-retained-composition-full-cast` fixture in `src/dev_preview/round.rs`. Do not read user state and do not add a new preview framework.
- [ ] Generate deterministic review artifacts without launching a focused window:

```bash
cargo run -- dev-preview --scenario round \
  --out target/glorp-preview-retained-composition
```

Expected result: the manifest and frames include the full-cast round fixture, with stable output on a second generation.

- [ ] Run the focused qualification suite:

```bash
cargo test --test presentation_props
cargo test --features retained-renderer --test retained_scene
cargo test --features retained-renderer --test retained_renderer_boundary
cargo test --test round_scene
cargo test --test smooth_companion
cargo test --features retained-renderer retained_full_cast_
cargo test --features retained-renderer prop_shadow_
cargo test --features retained-renderer retained_bed_
```

- [ ] Run repository-wide verification:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --features retained-renderer
git diff --check
```

Expected result: every command exits zero with no ignored warning or failing test.

- [ ] Inspect `git status --short`. Commit only test/fixture changes; never stage `target/`:

```bash
git add src/companion/retained/render.rs \
  tests/retained_scene.rs \
  tests/retained_renderer_boundary.rs \
  tests/round_scene.rs \
  tests/smooth_companion.rs
git add src/dev_preview/round.rs  # only if the full-cast fixture was required
git commit -m "test(renderer): qualify retained habitat composition"
```

### Manual approval handoff

- [ ] Build and relaunch the optimized companion only when Drew is ready to inspect it:

```bash
cargo xtask companion fresh
```

- [ ] Confirm the launched app reports the retained renderer and live retained scene runtime. Do not automate focus, fullscreen, or display movement.
- [ ] Ask Drew to review: normal animation; maximum props/tank cast; several resize sizes; square and non-square windows; external display; fullscreen; and return from fullscreen.
- [ ] Record the outcome for each acceptance criterion: readable individual props, textured substrate, quiet contact shadows, no HUD/gauge collisions, no new jitter, no fallback, and no crash.
- [ ] If manual review finds a defect, reproduce it with the smallest deterministic fixture/test before patching. Do not weaken the acceptance criteria or alter renderer lifecycle as part of this plan.

---

## Final Definition Of Done

- All six commits exist in order and `git status --short` is empty.
- Canonical prop art has one owner and every authored state fits a frozen footprint.
- Full-cast composition is deterministic, disjoint, inside the gauge-safe aperture, and clear of HUD/bottom reserves at the required size matrix.
- Prop/tank depth cues and parallax match the fixed table and reduce motion eliminates non-semantic parallax.
- The retained bed has stable biome texture; only visible grounded props cast quiet multiply shadows.
- Native 1x/2x visual gates, boundary tests, full Cargo tests, clippy, formatting, and release build pass.
- Drew completes the manual resize/display/fullscreen review with no new jitter, fallback, or crash.
