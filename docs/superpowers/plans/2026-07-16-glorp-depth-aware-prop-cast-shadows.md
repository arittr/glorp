# Glorp Depth-Aware Prop Cast Shadows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give only large, grounded, visually solid habitat props soft directional receiving-surface shadows, while preserving existing contact shadows and keeping small, suspended, translucent, and emissive props visually quiet.

**Architecture:** Author a closed shadow profile on each habitat prop and resolve it through one renderer-neutral geometry function using a fixed visual key-light projection. The companion scene carries validated fixed-size cast parameters beside each prop frame; retained evaluates contact plus capsule coverage in its existing `PropShadows` analytic, while Smooth/AppKit consumes the same resolved geometry through a private max-union alpha field painted once with multiply blending. Pixel and Classic ignore the new Smooth-only field.

**Tech Stack:** Rust, serde scene contracts, fixed retained GPU mirrors, WGSL analytic distance fields, AppKit/objc2 bitmap compositing, Rust unit/integration/native Metal tests, macOS companion xtask.

## Global Constraints

- The fixed visual key-light projection is normalized Y-up `[+0.19611613, -0.98058068]`, corresponding to authored direction `[+0.20, -1.00]`; no runtime scene light is added.
- Shadow profiles are exactly `None`, `ContactOnly`, or `Elevated { visual_height_cells, softness_cells }`.
- A cast shadow requires an `Elevated` profile, visibility, grounding, opacity above zero, width at least one cell, and height at least two cells.
- Projected length is `min(visual_height_cells * 0.55, 4.0)` cell heights.
- Cast softness is `clamp(softness_cells * cell_height + length_points * 0.12, 0.20 * cell_height, 1.25 * cell_height)`.
- Cast strength is `max(contact_strength, 0.22) * 0.65 * (1.0 - 0.08 * length_cells / 4.0)`; final opacity is applied once by each renderer.
- Existing contact strength remains for all profiles; an eligible or size-suppressed `Elevated` prop has a contact floor of `0.22`.
- Contact and cast coverage union by maximum within one prop and across props; overlapping shadows never repeatedly darken.
- Retained clips the field to the existing `PropShadows` receiving rect; Smooth clips it to the round tank receiving surface, so the four-cell length clamp never escapes the tank.
- Shadows use the existing biome-derived multiply tint and remain receiving-surface content behind props, pet, statistics, gauges, and chrome.
- Initial `Elevated` props are treasure chest `(2.25, 0.45)`, reeds `(3.50, 0.65)`, bonsai `(3.25, 0.70)`, and heavy-session planter `(3.00, 0.65)`.
- Initial `ContactOnly` props are pebble, shell, moss tuft, wilt-recovery sprout, and return sprout.
- Spark, friendly cloud, shard, orbit, lantern, hanging vine, geode, constellation, aurora, moon, Codex signal lamp, and first-ensemble-day use `None`.
- Pixel and Classic output remains unchanged; only Smooth/AppKit and retained render cast shadows.
- No general shadow maps, new lights, extra scene primitives, multiview renderer, head tracking, quilt generation, or display calibration is added.
- No preview/capture acceptance gate is added, and no Linear issue is created or updated for Glorp.

---

### Task 1: Authored Profiles and Shared Shadow Resolver

**Files:**
- Modify: `src/game/habitat.rs`
- Modify: `src/presentation/props.rs`
- Test: `src/game/habitat.rs`
- Test: `src/presentation/props.rs`

**Interfaces:**
- Produces: `HabitatPropShadowProfile::{None, ContactOnly, Elevated { visual_height_cells, softness_cells }}`
- Produces: `HabitatPropSpec::shadow_profile`
- Produces: `PROP_CAST_SHADOW_DIRECTION_Y_UP: [f32; 2]`
- Produces: `PropShadowResolveInput`, `ResolvedPropShadow`, and `ResolvedPropCastShadow`
- Produces: `resolve_prop_shadow(input: PropShadowResolveInput) -> Result<ResolvedPropShadow, PropShadowResolveError>`
- Produces: `prop_shadow_union_coverage(point_y_up: [f32; 2], shadows: &[ResolvedPropShadow]) -> f32`

- [ ] **Step 1: Add failing catalog and resolver tests**

Add exact catalog tests in `src/game/habitat.rs`:

```rust
#[test]
fn prop_shadow_profiles_are_explicit_for_every_catalog_entry() {
    assert_eq!(catalog_prop_by_str(TOKEN_PEBBLE_25K).unwrap().shadow_profile,
        HabitatPropShadowProfile::ContactOnly);
    assert_eq!(catalog_prop_by_str(TOKEN_TREASURE_CHEST_2M).unwrap().shadow_profile,
        HabitatPropShadowProfile::Elevated { visual_height_cells: 2.25, softness_cells: 0.45 });
    assert_eq!(catalog_prop_by_str(TOKEN_REEDS_5M).unwrap().shadow_profile,
        HabitatPropShadowProfile::Elevated { visual_height_cells: 3.50, softness_cells: 0.65 });
    assert_eq!(catalog_prop_by_str(TOKEN_BONSAI_100M).unwrap().shadow_profile,
        HabitatPropShadowProfile::Elevated { visual_height_cells: 3.25, softness_cells: 0.70 });
    assert_eq!(catalog_prop_by_str(HEAVY_SESSION_PLANTER).unwrap().shadow_profile,
        HabitatPropShadowProfile::Elevated { visual_height_cells: 3.00, softness_cells: 0.65 });
    assert_eq!(catalog_prop_by_str(TOKEN_LANTERN_10M).unwrap().shadow_profile,
        HabitatPropShadowProfile::None);
    assert_eq!(HABITAT_PROP_CATALOG.len(), 21);
}
```

Add resolver tests in `src/presentation/props.rs`:

```rust
#[test]
fn elevated_prop_resolves_fixed_direction_length_softness_and_strength() {
    let resolved = resolve_prop_shadow(PropShadowResolveInput {
        profile: HabitatPropShadowProfile::Elevated {
            visual_height_cells: 3.0,
            softness_cells: 0.65,
        },
        visible: true,
        grounded: true,
        opacity: 1.0,
        footprint_points: [18.0, 36.0],
        cell_extent_points: [18.0, 18.0],
        contact_strength: 0.24,
        origin_y_up_points: [100.0, 80.0],
    }).unwrap();
    assert_eq!(resolved.contact_strength, 0.24);
    let cast = resolved.cast.unwrap();
    let length = (cast.vector_y_up_points[0].powi(2) + cast.vector_y_up_points[1].powi(2)).sqrt();
    assert!((length - 29.7).abs() < 0.001);
    assert!(cast.vector_y_up_points[0] > 0.0);
    assert!(cast.vector_y_up_points[1] < 0.0);
    assert!((cast.softness_points - 15.264).abs() < 0.001);
    assert!((cast.strength - 0.150_852).abs() < 0.001);
}

#[test]
fn cast_shadow_suppresses_but_contact_survives_invalid_runtime_eligibility() {
    let base = elevated_input_fixture();
    for input in [
        PropShadowResolveInput { visible: false, ..base },
        PropShadowResolveInput { grounded: false, ..base },
        PropShadowResolveInput { opacity: 0.0, ..base },
        PropShadowResolveInput { footprint_points: [17.9, 36.0], ..base },
        PropShadowResolveInput { footprint_points: [18.0, 35.9], ..base },
    ] {
        let resolved = resolve_prop_shadow(input).unwrap();
        assert!(resolved.cast.is_none());
        assert!(resolved.contact_strength >= 0.22 || !input.grounded || !input.visible);
    }
}

#[test]
fn shadow_union_uses_maximum_coverage() {
    let shadow = resolve_prop_shadow(elevated_input_fixture()).unwrap();
    let single = prop_shadow_union_coverage([112.0, 70.0], &[shadow]);
    let double = prop_shadow_union_coverage([112.0, 70.0], &[shadow, shadow]);
    assert_eq!(single, double);
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```bash
cargo test --lib game::habitat::tests::prop_shadow_profiles_are_explicit_for_every_catalog_entry
cargo test --lib presentation::props::tests::elevated_prop_resolves_fixed_direction_length_softness_and_strength
```

Expected: FAIL because the authored profile and resolver types do not exist.

- [ ] **Step 3: Add the authored profile and exact initial catalog mapping**

Add to `src/game/habitat.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HabitatPropShadowProfile {
    None,
    ContactOnly,
    Elevated {
        visual_height_cells: f32,
        softness_cells: f32,
    },
}
```

Add `pub shadow_profile: HabitatPropShadowProfile` to `HabitatPropSpec`. Populate all 21 catalog records exactly according to Global Constraints; do not infer the profile from `kind`, `zone`, color, or glyph count.

- [ ] **Step 4: Implement the shared validated geometry resolver**

Add to `src/presentation/props.rs`:

```rust
pub(crate) const PROP_CAST_SHADOW_DIRECTION_Y_UP: [f32; 2] =
    [0.196_116_13, -0.980_580_7];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PropShadowResolveInput {
    pub(crate) profile: HabitatPropShadowProfile,
    pub(crate) visible: bool,
    pub(crate) grounded: bool,
    pub(crate) opacity: f32,
    pub(crate) footprint_points: [f32; 2],
    pub(crate) cell_extent_points: [f32; 2],
    pub(crate) contact_strength: f32,
    pub(crate) origin_y_up_points: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPropCastShadow {
    pub(crate) vector_y_up_points: [f32; 2],
    pub(crate) softness_points: f32,
    pub(crate) strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPropShadow {
    pub(crate) origin_y_up_points: [f32; 2],
    pub(crate) footprint_points: [f32; 2],
    pub(crate) opacity: f32,
    pub(crate) contact_strength: f32,
    pub(crate) cast: Option<ResolvedPropCastShadow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropShadowResolveError {
    NonFinite,
    NegativeGeometry,
    InvalidAuthoredProfile,
}
```

Validate every float before calculation. Preserve the supplied contact strength for `None` and `ContactOnly`. For `Elevated`, reject non-positive authored height/softness; set the grounded/visible contact floor to `0.22`; suppress only the cast portion when runtime eligibility fails. For eligible casts, apply the exact length, softness, direction, and strength equations from Global Constraints.

Implement `prop_shadow_coverage_at` with an ellipse for contact and a capsule from the grounded footprint center to `center + vector`. Implement `prop_shadow_union_coverage` as a fold with `f32::max`; never sum or source-over the individual coverage values.

- [ ] **Step 5: Run shared tests**

Run:

```bash
cargo test --lib game::habitat::tests
cargo test --lib presentation::props::tests
```

Expected: PASS, including the exact catalog, minimum footprint, fixed direction, height scaling, suppression, and max-union tests.

- [ ] **Step 6: Commit authored profiles and resolver**

```bash
git add src/game/habitat.rs src/presentation/props.rs
git commit -m "feat(companion): author prop shadow profiles"
```

---

### Task 2: Scene Snapshot, Validation, and Fixed Retained Frame ABI

**Files:**
- Modify: `src/presentation/companion_scene/mod.rs`
- Modify: `src/presentation/companion_scene/input.rs`
- Modify: `src/presentation/companion_scene/composition.rs`
- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/scene/checksum.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/presentation/companion_scene/runtime.rs`
- Modify: `src/companion/retained/compiler.rs`
- Test: same files' existing unit-test modules

**Interfaces:**
- Consumes: `HabitatPropShadowProfile` and `resolve_prop_shadow`
- Produces: `PropTopologySnapshot::shadow_profile`
- Produces: `PropFrameSnapshot::{cast_shadow_vector_points, cast_shadow_softness_points, cast_shadow_strength}`
- Produces: identical fields on `PropFrameSlot`
- Produces: 64-byte `FrameGpuValue` with `values: [f32; 12]`; prop cast values occupy indices `8..=11`
- Preserves: fixed prop slot count and one frame record per prop slot

- [ ] **Step 1: Add failing projection, validation, checksum, delta, and ABI tests**

Add focused tests that assert:

```rust
assert_eq!(projected.topology.visible_props[chest].shadow_profile,
    HabitatPropShadowProfile::Elevated { visual_height_cells: 2.25, softness_cells: 0.45 });
assert!(projected.frame.prop_instances[chest].cast_shadow_strength > 0.0);
assert_eq!(projected.frame.prop_instances[lantern].cast_shadow_strength, 0.0);
assert_ne!(checksum(&with_cast), checksum(&without_cast));
assert!(validate_snapshot(&nan_cast_vector).is_err());
assert!(classify_snapshot_changes(&before, &cast_changed).frame_changed());
assert_eq!(std::mem::size_of::<FrameGpuValue>(), 64);
```

Add a packer test with one `PropFrameSlot` and assert values `8`, `9`, `10`, and `11` are vector X, vector Y, softness, and strength. Existing values `0..=7` must remain byte-for-byte unchanged.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```bash
cargo test --features retained-renderer --lib presentation::companion_scene::input::tests::prop_projection_carries_authored_shadow_profile
cargo test --features retained-renderer --lib companion::retained::compiler::tests::prop_frame_packs_cast_shadow_tail
```

Expected: FAIL because the topology/frame fields and 12-float GPU ABI do not exist.

- [ ] **Step 3: Project profile and resolved frame geometry once**

Add `shadow_profile` to `PropTopologySnapshot` and copy `spec.shadow_profile` in `project_props`. Add these zero-safe fields to both frame types:

```rust
pub cast_shadow_vector_points: [f32; 2],
pub cast_shadow_softness_points: f32,
pub cast_shadow_strength: f32,
```

In `project_prop_frames`, call `resolve_prop_shadow` with the catalog-authored profile, placement visibility/grounding, current opacity, footprint, glyph-cell extent, existing contact strength, and the final origin plus motion offset converted to Y-up points. Copy resolved contact and cast values into `PropFrameSnapshot`; when `cast` is `None`, write `[0.0; 2]`, `0.0`, `0.0`.

`scene/compiler.rs` copies those values into `PropFrameSlot` without re-deriving eligibility. Update composition fixtures, runtime fixtures, scene defaults, and delta builders with explicit zero cast values.

- [ ] **Step 4: Validate and checksum every new semantic value**

In `validate_prop_frame_slot`, require all new values finite, softness/strength non-negative, strength in `[0, 1]`, and this zero-shape invariant:

```rust
let has_cast = slot.cast_shadow_strength > 0.0;
if has_cast
    != (slot.cast_shadow_vector_points != [0.0; 2]
        && slot.cast_shadow_softness_points > 0.0)
{
    return Err(SceneValidationError::InvalidFrameValue);
}
```

Reject cast strength for invisible or zero-opacity frames in `validate_prop_frame_slot`. In the enclosing frame validator, compare footprint against `glyph_cell_extent_points` and reject cast strength below the one-cell by two-cell minimum. Hash profile discriminant/height/softness in topology checksum and all four resolved cast values in frame checksum. Ensure delta classification includes the profile as generation topology and resolved cast values as frame changes.

- [ ] **Step 5: Extend the fixed frame record without changing prop slot count**

Change both Rust and WGSL `FrameGpuValue.values` from 8 to 12 floats. Append four zeros in every non-prop packer. Pack props exactly as:

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
    value.cast_shadow_vector_points[0],
    value.cast_shadow_vector_points[1],
    value.cast_shadow_softness_points,
    value.cast_shadow_strength,
],
```

Keep `PROP_FRAME_GPU_STRIDE == 1`, `PROP_FRAME_GPU_COUNT == MAX_VISIBLE_PROPS`, and all family counts unchanged. Update expected packed byte offsets/min binding sizes from the compiler-owned `CpuMirrorShape`; do not hard-code a second shadow buffer or a second record per slot.

- [ ] **Step 6: Run scene and ABI tests**

Run:

```bash
cargo test --features retained-renderer --lib presentation::companion_scene
cargo test --features retained-renderer --lib companion::retained::compiler::tests
```

Expected: PASS. Existing scene generation, delta, privacy, fixed-capacity, and mirror-layout tests remain green with the new 64-byte frame record.

- [ ] **Step 7: Commit scene and ABI propagation**

```bash
git add src/presentation/companion_scene/mod.rs src/presentation/companion_scene/input.rs src/presentation/companion_scene/composition.rs src/presentation/companion_scene/scene.rs src/presentation/companion_scene/scene/compiler.rs src/presentation/companion_scene/scene/checksum.rs src/presentation/companion_scene/validate.rs src/presentation/companion_scene/runtime.rs src/companion/retained/compiler.rs
git commit -m "feat(companion): project prop cast shadow geometry"
```

---

### Task 3: Retained Contact-Plus-Cast Analytic

**Files:**
- Modify: `src/companion/retained/scene.wgsl`
- Modify: `src/companion/retained/render.rs`
- Test: `src/companion/retained/render.rs`

**Interfaces:**
- Consumes: prop frame values `0..=11`
- Produces: one `fs_prop_shadows` evaluation combining existing contact ellipse and directional capsule
- Preserves: existing `PropShadows` semantic, primitive, draw count, receiving-surface Z, biome tint, and multiply pipeline

- [ ] **Step 1: Add failing shader-contract and native rendering tests**

Add source-contract tests that parse the WGSL constant/field contract and assert indices `8..11` are read by `fs_prop_shadows`, `PROP_FRAME_GPU_STRIDE` still matches Rust, and the function uses `max` rather than additive accumulation.

Add a native fixture with one elevated chest and assert:

```rust
assert!(shadow_alpha(sample_down_right_of_chest) > shadow_alpha(sample_up_left_of_chest));
assert!(shadow_extent(tall_profile) > shadow_extent(short_profile));
assert_eq!(overlap_alpha(two_identical_props), overlap_alpha(one_prop));
assert_eq!(cast_alpha(invisible_prop), 0);
assert_eq!(cast_alpha(ungrounded_prop), 0);
assert_eq!(cast_alpha(small_prop), 0);
```

- [ ] **Step 2: Run focused retained tests and verify failure**

Run:

```bash
cargo test --features retained-renderer --lib companion::retained::render::tests::prop_shadow_shader_reads_cast_tail_and_uses_max_union
```

Expected: FAIL because `fs_prop_shadows` still evaluates only the contact ellipse.

- [ ] **Step 3: Evaluate contact and capsule coverage in one analytic draw**

In `fs_prop_shadows`, retain the existing contact ellipse and add:

```wgsl
let cast_vector = vec2<f32>(frame.values[8], frame.values[9]);
let cast_softness = frame.values[10];
let cast_strength = frame.values[11];
let start = vec2<f32>(
    origin.x + footprint.x * 0.5,
    origin.y - max(footprint.y - cell_extent.y, 0.0) + cell_extent.y * 0.15,
);
let segment_length_squared = dot(cast_vector, cast_vector);
var cast_coverage = 0.0;
if (segment_length_squared > 0.0 && cast_softness > 0.0 && cast_strength > 0.0) {
    let relative = input.point_position - start;
    let along = clamp(dot(relative, cast_vector) / segment_length_squared, 0.0, 1.0);
    let distance = length(relative - cast_vector * along);
    let core_radius = max(footprint.x * 0.25, cell_extent.x * 0.5);
    cast_coverage = (1.0 - smoothstep(core_radius, core_radius + cast_softness, distance))
        * clamp(cast_strength, 0.0, 1.0)
        * clamp(frame.values[4], 0.0, 1.0);
}
let slot_coverage = max(contact_coverage, cast_coverage);
union_coverage = max(union_coverage, slot_coverage);
```

Keep the existing biome-derived multiply output after the loop. Do not create a shadow texture, new primitive, new light, or extra draw.

- [ ] **Step 4: Run retained shader and native tests**

Run:

```bash
cargo test --features retained-renderer --lib companion::retained::render::tests
cargo test --test round_scene
```

Expected: PASS, including direction, height scaling, softness, suppression, one-draw inventory, and exact max-union behavior.

- [ ] **Step 5: Commit retained cast rendering**

```bash
git add src/companion/retained/scene.wgsl src/companion/retained/render.rs
git commit -m "feat(companion): render retained prop cast shadows"
```

---

### Task 4: Smooth/AppKit Max-Union Shadow Field

**Files:**
- Modify: `src/presentation/smooth.rs`
- Modify: `src/tui/panels/pet/layered.rs`
- Modify: `src/round/smooth.rs`
- Modify: `src/companion/app.rs`
- Test: `src/presentation/smooth.rs`
- Test: `src/tui/panels/pet/layered.rs`
- Test: `tests/smooth_companion.rs`
- Test: `src/companion/app.rs`

**Interfaces:**
- Consumes: catalog profiles and shared `resolve_prop_shadow`
- Produces: `PresentationPropShadowSource` metadata on `LayeredPetScene`
- Produces: `SmoothLayerRole::PropShadows`
- Produces: `SmoothLayerItem::PropShadowField(SmoothPropShadowField)` containing resolved per-prop geometry in Y-up points
- Produces: one AppKit alpha bitmap whose per-pixel coverage is the maximum across all prop shadows
- Produces: `nsimage_from_straight_rgba(rgba: Vec<u8>, bounds: NSRect, backing_scale: f64) -> Option<Retained<NSImage>>`
- Preserves: Classic flattening because it continues to emit only `SmoothLayerItem::LocalCell`

- [ ] **Step 1: Add failing metadata, Smooth-plan, and max-union tests**

Add tests that assert:

```rust
let layered = full_habitat_layered_fixture();
assert!(layered.prop_shadow_sources.iter().any(|source| source.profile
    == HabitatPropShadowProfile::Elevated { visual_height_cells: 2.25, softness_cells: 0.45 }));

let smooth = full_habitat_smooth_fixture();
let shadow_layer = smooth.layer_by_role(SmoothLayerRole::PropShadows).unwrap();
assert_eq!(shadow_layer.motion_binding, SmoothLayerMotionBinding::Fixed);
assert_eq!(shadow_layer.blend, SmoothBlendMode::Multiply);
assert!(shadow_layer.z < smooth.layer_by_role(SmoothLayerRole::PropsBehind).unwrap().z);
assert!(smooth.flatten_classic_cells().cells.iter().all(|cell| cell.bg != Some(PROP_SHADOW_TINT)));

let once = appkit_prop_shadow_alpha(&[shadow_fixture()]);
let twice = appkit_prop_shadow_alpha(&[shadow_fixture(), shadow_fixture()]);
assert_eq!(once, twice);
```

- [ ] **Step 2: Run focused Smooth tests and verify failure**

Run:

```bash
cargo test --test smooth_companion smooth_scene_has_receiving_surface_prop_shadow_field
cargo test --lib companion::app::tests::appkit_prop_shadow_field_uses_max_union
```

Expected: FAIL because layered metadata, the Smooth-only shadow role/item, and AppKit field painter do not exist.

- [ ] **Step 3: Preserve placement identity as renderer-neutral shadow metadata**

Add to `src/presentation/props.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PresentationPropShadowSource {
    pub(crate) profile: HabitatPropShadowProfile,
    pub(crate) bounds_cells: [f32; 4],
    pub(crate) grounded: bool,
    pub(crate) opacity: f32,
    pub(crate) pet_layer: HabitatPetLayer,
}
```

Add `prop_shadow_sources: Vec<PresentationPropShadowSource>` to `LayeredPetScene`. In `tui/panels/pet/layered.rs`, call `habitat_prop_placements_for` once, flatten its cells for existing prop layers, and map each placement to shadow metadata using its catalog profile, bounds, floor-zone grounding, opacity `1.0`, and pet layer. Update existing `LayeredPetScene` fixtures with an empty metadata vector. Classic flattening must continue to inspect only `layers`.

- [ ] **Step 4: Add a Smooth-only fixed receiving-surface field after parallax is resolved**

Add `SmoothLayerRole::PropShadows` with `SmoothLayerMotionBinding::Fixed` and a closed item:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SmoothPropShadowField {
    pub shadows: Vec<ResolvedPropShadow>,
    pub tint: SmoothRgba8,
}

pub enum SmoothLayerItem {
    LocalCell(SmoothLocalCell),
    Shape(SmoothShape),
    PropShadowField(SmoothPropShadowField),
    Raster(SmoothRasterRef),
}
```

In `round/smooth.rs`, retain `prop_shadow_sources` before consuming layered layers. After normal prop parallax translations are resolved, select the translation from `PropsBehind` or `PropsForeground` for each source, convert the source's Y-down cell bounds to Y-up points, call `resolve_prop_shadow`, and collect nonzero contact/cast results. Add exactly one `PropShadows` layer at `z = 1`, after the tank bed and before ambient/props, with `Fixed`, `Multiply`, the same round receiving-surface clip as the tank bed, and one `PropShadowField` item. Do not add this layer to `LayeredPetScene`, so Classic never sees it.

Update Smooth validation and exhaustive role/item matches. `smooth_depth_bucket` routes `PropShadows` to `WorldBeforeStatistics`.

- [ ] **Step 5: Rasterize maximum coverage once and multiply the existing tint once**

In `appkit_blit_smooth_plan`, handle `PropShadowField` with:

```rust
fn appkit_draw_prop_shadow_field(
    field: &SmoothPropShadowField,
    bounds: NSRect,
    backing_scale: f64,
) -> Option<Retained<NSImage>> {
    let mut rgba = vec![0_u8; physical_rgba_len(bounds, backing_scale)?];
    for pixel in physical_pixels(bounds, backing_scale) {
        let point_y_up = pixel_center_y_up_points(pixel, bounds, backing_scale);
        let coverage = prop_shadow_union_coverage(point_y_up, &field.shadows);
        let alpha = (coverage.clamp(0.0, 1.0) * f32::from(field.tint.a)).round() as u8;
        rgba[pixel.byte_offset..pixel.byte_offset + 4].copy_from_slice(&[
            field.tint.r,
            field.tint.g,
            field.tint.b,
            alpha,
        ]);
    }
    nsimage_from_straight_rgba(rgba, bounds, backing_scale)
}
```

Draw that single image with the layer's existing `Multiply` compositing operation. Because every pixel alpha comes from `prop_shadow_union_coverage`, duplicate and overlapping props cannot accumulate darkness. Use the same biome-derived tint already authored for retained `PropShadows`; do not invent per-prop colors.

- [ ] **Step 6: Run Smooth/AppKit tests**

Run:

```bash
cargo test --lib presentation::smooth::tests
cargo test --lib tui::panels::pet::layered::tests
cargo test --lib companion::app::tests
cargo test --test smooth_companion
```

Expected: PASS. Smooth contains one fixed receiving-surface field; elevated props project; small/None props do not; duplicate overlap is identical; Classic flatten checksum remains unchanged for the same semantic scene.

- [ ] **Step 7: Commit Smooth/AppKit shadows**

```bash
git add src/presentation/smooth.rs src/presentation/props.rs src/tui/panels/pet/layered.rs src/round/smooth.rs src/companion/app.rs tests/smooth_companion.rs
git commit -m "feat(companion): render Smooth prop cast shadows"
```

---

### Task 5: Focused Integration Verification and Companion Rebuild

**Files:**
- Modify only if a focused test exposes a defect in files already listed above.

**Interfaces:**
- Consumes: completed shared, retained, and Smooth/AppKit prop-shadow implementations
- Produces: one rebuilt optimized `target/macos/Glorp.app` running the authored shadows

- [ ] **Step 1: Run formatting and focused behavior suites**

Run:

```bash
cargo fmt --check
cargo test --lib game::habitat::tests
cargo test --lib presentation::props::tests
cargo test --features retained-renderer --lib presentation::companion_scene
cargo test --features retained-renderer --lib companion::retained::compiler::tests
cargo test --features retained-renderer --lib companion::retained::render::tests
cargo test --lib companion::app::tests
cargo test --test smooth_companion
cargo test --test round_scene
```

Expected: all commands PASS. Do not add Preview Lab, paired-capture, or acceptance-matrix work to this slice.

- [ ] **Step 2: Inspect scope, ABI, and fallback invariants**

Run:

```bash
git diff --check HEAD~4..HEAD
git diff --stat HEAD~4..HEAD
rg -n "PROP_FRAME_GPU_STRIDE|values: array<f32, 12>|PropShadows" src/companion/retained src/presentation
```

Expected: no whitespace errors; stride remains one; Rust and WGSL both use 12 float frame values; the existing `PropShadows` semantic remains the sole retained shadow primitive.

- [ ] **Step 3: Rebuild and relaunch the optimized companion**

Run:

```bash
cargo xtask companion fresh
```

Expected: the optimized macOS app bundle builds, any running companion quits, and the fresh `target/macos/Glorp.app` opens.

- [ ] **Step 4: Commit only if verification required a scoped correction**

If Step 1 exposed and Step 2 confirmed a correction inside this plan's files:

```bash
git add src/game/habitat.rs src/presentation/props.rs src/presentation/smooth.rs src/presentation/companion_scene src/tui/panels/pet/layered.rs src/round/smooth.rs src/companion/app.rs src/companion/retained/compiler.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl tests/smooth_companion.rs
git commit -m "fix(companion): close prop shadow integration gaps"
```

If no correction was needed, do not create an empty commit.
