# Glorp Spatial Cues and Natural Motion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ground the statistics plane with a private rear-wall projection, preserve the existing authored prop shadows, replace the broad mood aura with a faint pet-local rim, and replace frenetic oscillator wander with calm deterministic destination-and-dwell locomotion.

**Architecture:** Keep the semantic scene and existing depth resolver authoritative. A new pure locomotion module emits normalized X/Y/Z intent into the existing round motion projection; lifecycle easing wraps that sample without mutable state. Visual grounding remains renderer-owned: AppKit builds private CPU masks from the already-private HUD and pet-body coverage, while retained rendering uses fixed-size private R8 coverage targets and full-frame composites. The scene-v2 aura slot remains reserved but visually empty, and no general light, material, or shadow-map system is activated.

**Tech Stack:** Rust, `time`, AppKit through `objc2-app-kit`, wgpu/WGSL, Glorp Preview Lab, Cargo unit/integration tests, and the repo-local `cargo xtask companion fresh` runner.

**Source Spec:** `docs/superpowers/specs/2026-07-16-glorp-spatial-cues-and-natural-motion-design.md` at commit `b7629049ffb9f582209a1730884e9e3ca3bbd797`.

## Global Constraints

- Implement test-first. Every behavior task begins with the named failing test, records the expected failure, then adds the smallest production change that makes it pass.
- Do not add or update a Linear issue for this repository.
- Do not activate `LightFrame`, `MaterialKind::LitShallowCard`, shadow maps, normals, moving lights, HDR lighting, or any other general dynamic-lighting path.
- Keep `CompanionDepthComposition`, the existing full-depth placement resolver, and authored prop `None` / `ContactOnly` / `Elevated` profiles authoritative.
- Keep statistics content, typography, layout, primary Z `+0.72`, rear echo, gauges, and privacy boundaries unchanged.
- Keep exact HUD strings, packed HUD glyphs, atlas indices, statistics coverage, and value-shaped shadows out of scene snapshots, checksums, manifests, debug output, and external redacted captures.
- A missing private AppKit or retained mask is a frame-preparation/render failure. Do not introduce a global-opacity, serialized-text, or readable duplicate fallback.
- Pixel and Classic must lose the broad aura. Classic may use the approved no-rim fallback because its flattened draw list does not isolate pet-body coverage. Pixel may render a narrow rim because it owns a private pixel mask.
- The base rim must be controlled by one constant. If visual review shows a fuzzy duplicate silhouette, set that constant to disabled; do not restore the radial aura.
- Preserve unrelated working-tree changes. Before every commit, inspect `git status --short` and the staged diff.

---

### Task 1: Define Backend-Neutral Spatial Cue Contracts

**Files:**

- Modify: `src/presentation/companion_effects.rs`
- Verify: `src/presentation/props.rs`

**Interfaces:**

- Consumes: `PROP_CAST_SHADOW_DIRECTION_Y_UP`, biome-derived shadow colors, existing activity pulse opacity, physical cell extent
- Produces: `StatisticsRearShadowStyle`, `PetRimStyle`, shared mask-kernel helpers, and the single rim enable switch

- [ ] **Step 1: Add failing shared-style tests**

Add tests to `src/presentation/companion_effects.rs`:

```rust
#[test]
fn statistics_shadow_uses_prop_key_light_and_stays_soft_and_faint() {
    let style = statistics_rear_shadow_style([7.0, 14.0]).unwrap();
    assert!(style.offset_y_up_points[0] > 0.0);
    assert!(style.offset_y_up_points[1] < 0.0);
    assert!(style.softness_points >= 7.0);
    assert!(style.opacity > 0.0 && style.opacity <= 0.12);
    assert_parallel(
        style.offset_y_up_points,
        crate::presentation::props::PROP_CAST_SHADOW_DIRECTION_Y_UP,
    );
}

#[test]
fn pet_rim_is_narrow_constant_backed_and_activity_only_changes_alpha() {
    let idle = pet_rim_style(0.0, false);
    let active = pet_rim_style(1.0, false);
    let reduced = pet_rim_style(1.0, true);
    assert!(idle.enabled);
    assert!(idle.radius_points > 0.0 && idle.radius_points <= 1.5);
    assert!(idle.alpha > 0.0 && idle.alpha <= 0.12);
    assert_eq!(active.radius_points, idle.radius_points);
    assert!(active.alpha > idle.alpha);
    assert_eq!(reduced.alpha, idle.alpha);
}

#[test]
fn mask_dilation_returns_only_exterior_coverage() {
    let source = alpha_fixture_with_one_opaque_center_pixel();
    let rim = exterior_dilated_alpha(&source, 3, 3, 1).unwrap();
    assert_eq!(rim[center_index(3, 3)], 0);
    assert!(rim[neighbor_index(3, 3)] > 0);
}

#[test]
fn invalid_rim_inputs_disable_the_rim_without_restoring_an_aura() {
    let invalid = pet_rim_style(f32::NAN, false);
    assert!(!invalid.enabled);
    assert_eq!(invalid.alpha, 0.0);
}
```

- [ ] **Step 2: Run the tests and verify failure**

Run:

```bash
cargo test --lib presentation::companion_effects::tests
```

Expected: FAIL because the rear-shadow style, pet-rim style, and exterior-mask helpers do not exist.

- [ ] **Step 3: Add closed shared style types and constants**

Add to `src/presentation/companion_effects.rs`:

```rust
pub(crate) const PET_RIM_ENABLED: bool = true;
pub(crate) const PET_RIM_RADIUS_POINTS: f32 = 1.25;
pub(crate) const PET_RIM_IDLE_ALPHA: f32 = 0.09;
pub(crate) const PET_RIM_ACTIVITY_ALPHA_BONUS: f32 = 0.07;
const STATISTICS_REAR_SHADOW_LENGTH_CELLS: f32 = 0.90;
const STATISTICS_REAR_SHADOW_SOFTNESS_CELLS: f32 = 0.75;
const STATISTICS_REAR_SHADOW_OPACITY: f32 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StatisticsRearShadowStyle {
    pub(crate) offset_y_up_points: [f32; 2],
    pub(crate) softness_points: f32,
    pub(crate) opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PetRimStyle {
    pub(crate) enabled: bool,
    pub(crate) radius_points: f32,
    pub(crate) alpha: f32,
}

pub(crate) fn statistics_rear_shadow_style(
    cell_extent_points: [f32; 2],
) -> Option<StatisticsRearShadowStyle>;

pub(crate) fn pet_rim_style(
    activity_opacity: f32,
    reduce_motion: bool,
) -> PetRimStyle;

pub(crate) fn exterior_dilated_alpha(
    source: &[u8],
    width: u32,
    height: u32,
    radius_pixels: u32,
) -> Option<Vec<u8>>;
```

`statistics_rear_shadow_style` validates finite positive cell extents, multiplies `PROP_CAST_SHADOW_DIRECTION_Y_UP` by `STATISTICS_REAR_SHADOW_LENGTH_CELLS * cell_extent_points[1]`, and derives softness from the vertical cell extent. `pet_rim_style` disables the rim for any non-finite input, otherwise clamps activity opacity, leaves radius fixed, and suppresses only the activity bonus under Reduce Motion. `exterior_dilated_alpha` computes `max(neighborhood_alpha) - source_alpha` with saturating subtraction, so the pet paints over the interior and only the exterior rim survives.

Keep `PROP_CAST_SHADOW_DIRECTION_Y_UP` in `src/presentation/props.rs` as the sole authored key-light direction. Do not duplicate its numeric vector in the new helper.

- [ ] **Step 4: Run shared tests**

Run:

```bash
cargo test --lib presentation::companion_effects::tests
cargo test --lib presentation::props::tests
```

Expected: PASS. Invalid/non-finite extents return `None`; the key-light vector is shared; the rim is exterior-only and constant-backed.

- [ ] **Step 5: Commit the shared cue contracts**

```bash
git status --short
git add src/presentation/companion_effects.rs
git diff --cached --check
git commit -m "feat(companion): define authored spatial cue styles"
```

---

### Task 2: Build the Deterministic Destination-and-Dwell Locomotion Core

**Files:**

- Create: `src/round/locomotion.rs`
- Modify: `src/round/mod.rs`

**Interfaces:**

- Consumes: stable pet seed, wall-clock instant, current fallback facing
- Produces: one bounded normalized `CompanionLocomotionSample` with coherent X/Y/Z phase and segment metadata

- [ ] **Step 1: Add failing locomotion contract tests**

Create `src/round/locomotion.rs` with the test module first. Cover these exact behaviors:

```rust
#[test]
fn same_identity_and_instant_are_restart_stable();

#[test]
fn each_segment_dwells_between_eight_and_eighteen_seconds();

#[test]
fn dwell_is_stationary_and_glide_meets_both_endpoints_exactly();

#[test]
fn minimum_jerk_has_zero_velocity_and_acceleration_at_endpoints();

#[test]
fn quadratic_xy_bend_is_at_most_twelve_percent_and_z_is_unbent();

#[test]
fn route_stays_normalized_and_each_move_is_bounded();

#[test]
fn route_avoids_identical_targets_and_unforced_direct_reversals();

#[test]
fn a_sixteen_segment_window_reaches_rear_and_front_depth();

#[test]
fn facing_changes_only_at_the_stationary_segment_start();

#[test]
fn two_minute_sample_has_visible_dwell_and_at_most_two_reversals_per_axis();

#[test]
fn segment_lookup_is_continuous_at_exact_minute_and_unix_zero_boundaries();
```

The route-window test samples at least two stable identities and asserts a rear target `<= -0.95` and a front target `>= 0.95` within sixteen consecutive segments. The move-bound test uses the constants exported by the module rather than duplicating thresholds in the test.

- [ ] **Step 2: Run the locomotion tests and verify failure**

Run:

```bash
cargo test --lib round::locomotion::tests
```

Expected: FAIL because the module and sampler are not implemented.

- [ ] **Step 3: Implement fixed-size deterministic route blocks**

Export from `src/round/mod.rs`:

```rust
pub mod locomotion;
```

Implement these closed types in `src/round/locomotion.rs`:

```rust
pub(crate) const LOCOMOTION_SEGMENT_SECS: i64 = 60;
pub(crate) const LOCOMOTION_BLOCK_SEGMENTS: usize = 8;
pub(crate) const LOCOMOTION_DWELL_MIN_SECS: i64 = 8;
pub(crate) const LOCOMOTION_DWELL_MAX_SECS: i64 = 18;
pub(crate) const LOCOMOTION_MAX_PLANAR_STEP: f32 = 0.62;
pub(crate) const LOCOMOTION_MAX_DEPTH_STEP: f32 = 0.58;
const LOCOMOTION_CONTROL_OFFSET_FRACTION: f32 = 0.12;
const LOCOMOTION_FACING_DEADZONE: f32 = 0.06;

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

pub(crate) fn stable_companion_identity(seed: &str) -> u64;

pub(crate) fn sample_companion_locomotion(
    identity: u64,
    now: time::OffsetDateTime,
    fallback_facing: i8,
) -> CompanionLocomotionSample;
```

Use a fixed `[NormalizedLocomotionPoint; 9]` route block for eight one-minute segments. The block starts at deterministic boundary anchor `A(block_index)` and ends at `A(block_index + 1)`, so adjacent blocks share an exact endpoint without recursion or persisted state. Define boundary anchors as X in `[-0.55, +0.55]` and Y in `[-0.45, +0.45]` from independent identity/block hash lanes, with Z `-1.0` for even blocks and `+1.0` for odd blocks.

For interior slots `1..=7`, build eight XY candidates in stable hash-rotated order around the linear anchor-to-anchor interpolation point. Candidate radii alternate `0.18` and `0.28`; candidate angles are the eight 45-degree compass increments. Clamp candidates to normalized bounds, reject planar distance below `0.12`, reject a step above `LOCOMOTION_MAX_PLANAR_STEP`, and reject a reversal whose normalized dot product with the prior step is below `-0.75` when another candidate remains. Choose the first valid candidate. Use the clamped interpolation point only as the exhausted-candidate fallback.

Use this fixed Z waypoint pattern from rear to front: `[-1.0, -1.0, -1.0, -0.33, -0.33, 0.33, 0.33, 1.0, 1.0]`; reverse it for front to rear. Thus five of eight segments are planar at constant depth and three make bounded depth progress. Validate every generated block in debug/tests and return a neutral route only for impossible non-finite input. This keeps lookup constant-time while guaranteeing full-depth reachability in a bounded window.

For segment `N`, derive dwell seconds from a stable hash in inclusive range `8..=18`. During dwell, return `T(N)`. During glide, evaluate:

```rust
fn minimum_jerk(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (10.0 + t * (-15.0 + 6.0 * t))
}
```

Compute signed segment and block indices with `div_euclid` / `rem_euclid`; do not cast Unix seconds to `u64`, so fixtures before the epoch and the exact zero boundary remain continuous. Use the same eased phase for X, Y, and Z. Apply a deterministic perpendicular quadratic Bezier control offset to X/Y only; clamp its magnitude to `0.12 * planar_segment_length`. Select facing from the next meaningful X delta at the start of the segment and hold it for dwell plus glide, preserving the existing convention: positive destination X delta maps to facing `-1`, and negative delta maps to `+1`. If the X delta is within the deadzone, scan the fixed route block backward for the last meaningful X direction and use `fallback_facing` only when the route contains none.

- [ ] **Step 4: Run locomotion tests**

Run:

```bash
cargo test --lib round::locomotion::tests
```

Expected: PASS. The route is deterministic, bounded, continuous, endpoint-smooth, and reaches both depth endpoints without independent axis oscillators.

- [ ] **Step 5: Commit the pure locomotion core**

```bash
git status --short
git add src/round/locomotion.rs src/round/mod.rs
git diff --cached --check
git commit -m "feat(companion): add purposeful drift locomotion"
```

---

### Task 3: Integrate Locomotion with Round Projection, Sleep, Wake, and Reduce Motion

**Files:**

- Modify: `src/round/motion.rs`
- Modify: `src/round/scene.rs`
- Modify: `src/round/placement.rs`
- Modify: `src/presentation/companion_scene/input.rs`
- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/runtime.rs`
- Modify: `tests/round_scene.rs`

**Interfaces:**

- Consumes: `CompanionLocomotionSample`, pet seed, `DayContext.sleep_onset_utc`, `DayContext.wake_resume`, existing projection envelope and depth override
- Produces: the existing `RoundCompanionMotionProjection` populated by calm normalized locomotion; no caller-facing renderer split

- [ ] **Step 1: Replace oscillator expectations with failing projection tests**

In `src/round/motion.rs`, remove tests that assert activity-scaled energy, mixed-frequency reversal, or two-second bob behavior. Add:

```rust
#[test]
fn activity_and_calm_do_not_change_awake_locomotion_geometry();

#[test]
fn projection_uses_one_locomotion_sample_for_planar_and_depth_intent();

#[test]
fn sleeping_motion_settles_to_neutral_then_holds();

#[test]
fn waking_motion_eases_from_neutral_to_the_live_path();

#[test]
fn facing_flips_during_dwell_and_never_mid_glide();

#[test]
fn companion_bob_is_zero_for_all_elapsed_times();

#[test]
fn depth_override_changes_only_depth_for_review_fixtures();
```

In `tests/round_scene.rs`, add a surface-parity test that builds both the round draw-list path and companion-scene projection from the same view model/instant and asserts equal normalized depth, facing, and fractional motion origin.

- [ ] **Step 2: Run focused motion tests and verify failure**

Run:

```bash
cargo test --lib round::motion::tests
cargo test --test round_scene purposeful_motion
```

Expected: FAIL because `CompanionMotionInput` still carries activity/calm energy inputs, the oscillator functions are live, and bob remains nonzero.

- [ ] **Step 3: Replace motion input with stable identity and flattened lifecycle instants**

Change `CompanionMotionInput` to remain `Copy` and serialization-private:

```rust
#[derive(Clone, Copy, PartialEq)]
pub struct CompanionMotionInput {
    pub identity: u64,
    pub asleep: bool,
    pub sleep_onset_utc: Option<time::OffsetDateTime>,
    pub wake_from_eval_utc: Option<time::OffsetDateTime>,
    pub woke_at_utc: Option<time::OffsetDateTime>,
    pub current_facing: i8,
    pub resolved_wander_offset_x: i16,
    pub resolved_wander_facing: i8,
    pub breath_offset_y_cells: u8,
}
```

Both builders—`src/round/scene.rs::companion_motion_input` and `src/presentation/companion_scene/input.rs::companion_motion_input`—must use `stable_companion_identity(&vm.pet_render.seed)` and flatten the optional `WakeResume` pair. Remove `calm` and `rate_per_hour` from all explicit fixtures in `src/round/placement.rs`, `src/presentation/companion_scene/scene.rs`, and `src/presentation/companion_scene/runtime.rs`.

Keep `resolved_wander_*` only for `CompanionMotion { wander: false }` compatibility and neutral fallback. Do not feed those watch-wander offsets into the new round locomotion path.

Add `#[serde(skip)] pub(crate) reduce_motion: bool` to the private tail of `FrameSnapshot`. Set it from `CompanionPresentationOptions` on initial projection and every re-projection. It is renderer input for suppressing the rim activity bonus in Task 6; keep it out of scene serialization, checksums, artifacts, and redacted debug fields.

- [ ] **Step 4: Project lifecycle-wrapped locomotion through the existing envelope**

In `project_round_companion_motion_with_options`:

1. Sample live locomotion from identity + wall time.
2. If asleep with an onset, sample the active path at the onset and minimum-jerk blend that point to `[0, 0, 0]` over `crate::pet::animator::WANDER_SETTLE_SECS`; hold neutral afterward.
3. If awake with a complete wake pair, first reconstruct the stable held pose at wake: sample the active path at `wake_from_eval_utc`, then apply the same sleep-settle function for elapsed time `woke_at_utc - wake_from_eval_utc`. Minimum-jerk blend from that reconstructed held pose to the current live sample over the settle duration beginning at `woke_at_utc`. Incomplete or inverted pairs fail closed to the ordinary live sample in release and assert in tests.
4. Pass the resulting X/Y into `project_round_companion_motion_from_offsets` and the resulting Z through the existing normalized depth and optional review override.
5. Set `bob_offset_y_cells` to `0.0` unconditionally. Preserve authored breath through `breath_offset_y_cells`.

Delete `companion_motion_energy`, `companion_wander_offsets`, `companion_wander_depth`, `companion_wander_facing`, and the public sinusoidal `round_companion_bob` implementation once no production caller remains. Keep the legacy non-wander `companion_drift_offsets` path only if tests prove another surface still consumes it.

Do not alter `resolve_round_depth_placement`, `CompanionDepthComposition`, parallax caps, or the Reduce Motion neutral projector.

- [ ] **Step 5: Run projection and scene tests**

Run:

```bash
cargo test --lib round::locomotion::tests
cargo test --lib round::motion::tests
cargo test --lib round::placement::tests
cargo test --lib presentation::companion_scene::input::tests
cargo test --test round_scene
```

Expected: PASS. Activity changes do not teleport or rescale paths; sleep/wake are deterministic and continuous; Reduce Motion remains static; both scene paths consume the same motion contract.

- [ ] **Step 6: Commit locomotion integration**

```bash
git status --short
git add src/round/motion.rs src/round/scene.rs src/round/placement.rs src/presentation/companion_scene/input.rs src/presentation/companion_scene/scene.rs src/presentation/companion_scene/runtime.rs tests/round_scene.rs
git diff --cached --check
git commit -m "feat(companion): integrate calm lifecycle locomotion"
```

---

### Task 4: Retire the Broad Aura from Scene Contracts and Flat Renderers

**Files:**

- Modify: `src/presentation/smooth.rs`
- Modify: `src/presentation/companion_effects.rs`
- Modify: `src/round/smooth.rs`
- Modify: `src/round/hud.rs`
- Modify: `src/dev_preview/contract.rs`
- Modify: `tests/smooth_companion.rs`
- Modify: `src/presentation/pixel/animator.rs`
- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/scene/checksum.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/companion/app.rs`
- Modify: `src/companion/paired_review.rs`
- Modify: `src/companion/retained/compiler.rs`
- Modify: `src/companion/retained/render.rs`
- Modify: `src/companion/retained/capture.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Modify: `src/companion/retained.rs`

**Interfaces:**

- Consumes: the existing aura role, scene analytic slot 4, Classic/AppKit aura painter, Pixel ellipse painter, retained aura primitive
- Produces: no broad aura pixels on any path; analytic slot 4 reserved and empty; mood remains available through content globals/pet model

- [ ] **Step 1: Add failing no-aura contract tests**

Add/replace tests so they assert:

```rust
#[test]
fn smooth_plan_has_no_mood_aura_layer_or_required_role();

#[test]
fn scene_v2_reserves_analytic_slot_four_without_a_primitive_or_paint();

#[test]
fn retained_draw_plan_has_no_mood_aura_source();

#[test]
fn classic_paint_schedule_never_calls_a_radial_pet_effect();

#[test]
fn pixel_idle_frame_has_no_large_ellipse_outside_the_body_shadow_and_rim_band();
```

Update `tests/smooth_companion.rs` to expect one fewer screen-reservation layer and remove depth-transform assertions tied to `MoodAura`. Add a `tests/round_scene.rs` assertion that `pet.aura.mood` is absent from primitive aliases and serialized JSON.

- [ ] **Step 2: Run the no-aura tests and verify failure**

Run:

```bash
cargo test --test smooth_companion mood_aura
cargo test --features retained-renderer --lib companion::retained::render::tests::retained_draw_plan_has_no_mood_aura_source
cargo test --lib presentation::pixel::animator::tests
cargo test --test round_scene aura
```

Expected: FAIL because Smooth still reserves `MoodAura`, AppKit/Classic still call `draw_mood_aura`, Pixel still paints three broad ellipses, and scene-v2 still emits `pet.aura.mood`.

- [ ] **Step 3: Remove broad aura work while keeping the ABI hole explicit**

- Delete `SmoothLayerRole::MoodAura`, its required-role entry, reservation layer, contract export, and all matching tests.
- Delete `SmoothPetFrontPaintStep::MoodAura`, `draw_mood_aura`, `MOOD_AURA_RING_ALPHA_U8`, and `mood_aura_radius`. Rename `round::hud::mood_aura_color` to `mood_rim_color`, and rename the prepared/review/capture fields and accessors to `pet_rim_color` / `review_pet_rim_color`; this is a tint, not a radial primitive. Update `paired_review.rs` and `retained/capture.rs` so no capture identity field is still named as an aura.
- Remove Pixel `draw_aura` and its call before the body. Do not add the replacement rim until Task 5.
- In the current scene-v2 compiler, retain `AnalyticSemantic::MoodAura.id() == 4` only as a reserved identifier. Do not emit `pet.aura.mood`, `AnalyticShape::PetAura`, `AnalyticPaint::MoodAuraRings`, frame geometry, or a blended draw for it. The slot value is `None`, primitive count drops by one, and checksum/validation tables record the reserved hole explicitly.
- Remove `fs_mood_rings`, its `fs_analytic` case, `fs_hud_interaction_aura`, `HudInteractionSource::MoodAura`, and the aura interaction pipeline. The interaction plan becomes exactly `[PetBody, PetParticles]`.
- Delete legacy `push_mood_aura` in `src/companion/retained.rs` and update `RetainedChrome` naming to `pet_rim_color` for Task 5 compatibility.

Mood remains in `ContentGlobalsGpuValue.mood` and in the round pet model. The next tasks derive the narrow rim from that mood plus renderer-private body coverage; do not smuggle a value-shaped aura primitive back into scene data.

- [ ] **Step 4: Run semantic and flat-renderer tests**

Run:

```bash
cargo test --lib presentation::smooth::tests
cargo test --lib presentation::pixel::animator::tests
cargo test --features retained-renderer --lib presentation::companion_scene
cargo test --features retained-renderer --lib companion::retained::compiler::tests
cargo test --features retained-renderer --lib companion::retained::render::tests
cargo test --test smooth_companion
cargo test --test round_scene
```

Expected: PASS. There is no radial aura draw or scene primitive; slot 4 remains an explicit empty ABI reservation; the two-source HUD crossing plan still validates.

- [ ] **Step 5: Commit aura retirement**

```bash
git status --short
git add src/presentation/smooth.rs src/presentation/companion_effects.rs src/round/smooth.rs src/round/hud.rs src/dev_preview/contract.rs tests/smooth_companion.rs src/presentation/pixel/animator.rs src/presentation/companion_scene src/companion/app.rs src/companion/paired_review.rs src/companion/retained/compiler.rs src/companion/retained/render.rs src/companion/retained/capture.rs src/companion/retained/scene.wgsl src/companion/retained.rs tests/round_scene.rs
git diff --cached --check
git commit -m "refactor(companion): retire the broad pet aura"
```

---

### Task 5: Add AppKit Statistics Projection and Pet-Local Rim

**Files:**

- Modify: `src/companion/app.rs`
- Modify: `src/presentation/pixel/animator.rs`

**Interfaces:**

- Consumes: private AppKit HUD primary coverage, Smooth `PetBody` layer only, shared shadow/rim styles, biome shadow tint, `RoundActivityPulse`
- Produces: `PreparedAppKitHudVolume` with private rear projection, `PreparedAppKitPetRim`, Pixel exterior rim, and unchanged public scene/review contracts

- [ ] **Step 1: Add failing private-mask and painter-order tests**

Add AppKit tests in `src/companion/app.rs`:

```rust
#[test]
fn prepared_appkit_hud_volume_debug_redacts_shadow_and_live_text();

#[test]
fn appkit_statistics_shadow_is_after_tank_background_and_before_world();

#[test]
fn appkit_statistics_shadow_is_displaced_soft_and_not_readable_at_full_strength();

#[test]
fn appkit_pet_rim_uses_only_pet_body_alpha();

#[test]
fn appkit_pet_front_paints_rim_before_body_and_crosses_statistics_as_one_group();

#[test]
fn appkit_missing_private_coverage_fails_frame_preparation();
```

Add Pixel tests that compare changed pixels against the rendered body mask and assert every rim pixel is within the configured dilation radius, the center/body interior is not recolored, and activity changes alpha but not extent.

- [ ] **Step 2: Run AppKit/Pixel tests and verify failure**

Run:

```bash
cargo test --lib companion::app::tests::appkit_statistics_shadow
cargo test --lib companion::app::tests::appkit_pet_rim
cargo test --lib presentation::pixel::animator::tests
```

Expected: FAIL because neither prepared rear projection nor body-only rim exists after Task 4.

- [ ] **Step 3: Extend private prepared AppKit resources**

Keep exact text and masks inside redacted types:

```rust
#[derive(Clone)]
struct PreparedAppKitHudVolume {
    lines: [PreparedAppKitHudLine; 3],
    primary_coverage: Retained<NSImage>,
    rear_wall_projection: Option<Retained<NSImage>>,
    bitmap_target: PreparedAppKitBitmapTarget,
}

#[derive(Clone)]
struct PreparedAppKitPetRim {
    image: Retained<NSImage>,
    bounds: NSRect,
}

impl std::fmt::Debug for PreparedAppKitPetRim {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PreparedAppKitPetRim(<private>)")
    }
}
```

During `prepare_appkit_hud_volume`, read the alpha bytes from the existing off-screen primary coverage representation, apply a separable normalized blur whose radius is derived from `StatisticsRearShadowStyle.softness_points * backing_scale`, shift by the authored key-light offset, clip to the aperture, tint with `bed_shadow_srgb8(scene.room.biome)`, and write one straight-RGBA private image. Cap alpha by the shared style opacity. When `redacts_live_hud` is true, keep `rear_wall_projection` as `None` by capture policy while still preparing primary coverage for soft pet/statistics interaction. Add `appkit_redacted_review_omits_statistics_projection` and verify with a live test fixture that the blurred output cannot reconstruct the original binary coverage by threshold equality.

Add `pet_body: Vec<usize>` to `PreparedSmoothDepthPasses`. It must contain exactly the layer with `SmoothLayerRole::PetBody`; `PerformanceCue`, particles, wall shadow, floor projection, props, and HUD reservations are excluded. When `PetRimStyle.enabled`, render that layer into a private bitmap, take its alpha, call `exterior_dilated_alpha`, tint with `pet_rim_color`, and store `appkit_pet_rim: Some(Box<PreparedAppKitPetRim>)`; failure to isolate exactly one body layer or allocate the bitmap returns a typed `CompanionFramePreparationError`. When the style is disabled, store `None` and do not allocate body coverage.

- [ ] **Step 4: Insert effects at physical receiving surfaces**

Add `SmoothAppKitPaintStep::StatisticsRearShadow` as the first Smooth schedule step. The round tank background remains painted before the schedule, so the order is:

```text
tank background
statistics rear-wall projection
world props/shadows/ambient
depth-ordered echo, pet group, and primary statistics
foreground and chrome
```

Change `paint_smooth_pet_front` to draw `PreparedAppKitPetRim` immediately before the `pet_front` layers. Because `render_masked_pet_front_image` calls the same helper, the rim and body cross statistics together without adding rim coverage to the HUD mask.

Derive rim alpha from the existing two-second `RoundActivityPulse` envelope. Add an explicit `reduce_motion: bool` argument to `prepare_companion_frame`, `prepare_companion_frame_at`, and their current callers so `pet_rim_style` removes only the activity bonus. Update the existing deterministic frame-preparation fixtures with `false` unless they are the Reduce Motion case. Do not read the ambient accessibility setting again inside an off-screen painter, and do not change locomotion timing or destination selection.

- [ ] **Step 5: Add Pixel's narrow exterior rim**

In `render_pixel_frame`, render body/reference occupancy into a temporary alpha mask, call the same exterior-only dilation contract at the Pixel logical scale, tint through the renamed `crate::round::hud::mood_rim_color(input.mood)`, composite the rim, then paint shadow/body/face over it. Do not restore the three large ellipses or substitute the generic accent palette. Keep extent fixed between idle and activity.

Classic AppKit remains no-rim and no-statistics-shadow because its flattened draw list has no private body/HUD receiving-surface contract.

- [ ] **Step 6: Run AppKit, Pixel, and Smooth tests**

Run:

```bash
cargo test --lib companion::app::tests
cargo test --lib presentation::pixel::animator::tests
cargo test --test smooth_companion
cargo test --test companion_draw_boundary
```

Expected: PASS. The statistics projection is behind world content, the rim is exterior and body-only, activity changes only rim alpha, and all private debug output stays redacted.

- [ ] **Step 7: Commit AppKit and Pixel effects**

```bash
git status --short
git add src/companion/app.rs src/presentation/pixel/animator.rs
git diff --cached --check
git commit -m "feat(companion): ground AppKit HUD and rim the pet"
```

---

### Task 6: Add Retained Private Coverage Passes for Statistics Shadow and Pet Rim

**Files:**

- Modify: `src/companion/retained/hud.rs`
- Modify: `src/companion/retained/compiler.rs`
- Modify: `src/companion/retained/render.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Modify: `src/companion/retained/host.rs`
- Modify: `src/companion/retained/capture.rs`
- Modify: `src/companion/retained.rs`
- Modify: `src/companion/paired_review.rs`

**Interfaces:**

- Consumes: sealed HUD glyph records, validated `PetBody` planned draw, content-global mood, frame-global activity opacity, shared style constants
- Produces: private statistics and pet-body R8 targets, retained rear-shadow/rim composites, unchanged scene snapshot/checksum/artifacts, redacted-capture omission of the value-shaped shadow

- [ ] **Step 1: Add failing retained resource, privacy, and pixel tests**

Add retained tests for:

```rust
#[test]
fn pet_body_coverage_target_is_private_fixed_r8();

#[test]
fn hud_coverage_prepass_stages_once_and_writes_no_scene_color();

#[test]
fn statistics_shadow_composite_runs_after_room_background_before_world_blends();

#[test]
fn retained_rim_uses_pet_body_draw_only_and_precedes_the_body();

#[test]
fn retained_rim_crosses_statistics_with_body_and_excludes_wall_and_floor_shadows();

#[test]
fn external_redacted_capture_omits_statistics_shadow_but_keeps_redacted_hud();

#[test]
fn scene_json_checksum_and_debug_do_not_gain_private_coverage_or_shadow_fields();

#[test]
fn gpu_globals_pack_activity_opacity_and_rim_tint_without_changing_public_artifacts();

#[test]
fn invalid_private_spatial_cue_input_preserves_the_last_good_presented_frame();

#[test]
fn paired_review_uses_matching_spatial_cue_parameters_without_serializing_coverage();
```

Native pixel tests render a fixed body/HUD fixture and assert: rear projection pixels exist only when sensitive/live projection is used; projection lies on the key-light side of source coverage; rim pixels are within the configured physical radius; no rim appears around particle-only pixels; and duplicate effect composites do not darken or brighten the same pixel twice.

- [ ] **Step 2: Run retained tests and verify failure**

Run:

```bash
cargo test --features retained-renderer --lib companion::retained::hud::tests
cargo test --features retained-renderer --lib companion::retained::render::tests::pet_body_coverage_target_is_private_fixed_r8
cargo test --features retained-renderer --lib companion::retained::render::tests::statistics_shadow_composite_runs_after_room_background_before_world_blends
```

Expected: FAIL because HUD coverage is produced only during the echo pass, there is no pet-body coverage target, and neither full-frame composite exists.

- [ ] **Step 3: Split sealed HUD coverage from visible HUD drawing**

Replace the two-target HUD MRT dependency with one private coverage prepass plus visible echo/primary passes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HudDrawPhase {
    Coverage,
    Echo,
    Primary,
}
```

Add a coverage-only pipeline whose WGSL fragment returns one `R8Unorm` coverage value and no scene color. `Coverage` stages the fixed HUD records and interaction record exactly once, clears `statistics_coverage`, and draws only primary glyph geometry. `Echo` and `Primary` load the already-staged buffers and target only raw scene color. Preserve the atomic caller-owned encoder and resource-generation validation.

Expose no raw records or coverage handles. The live/sensitive and independently built redacted HUD projections both need coverage for statistics crossing, but only live/sensitive projection sets a private `statistics_rear_shadow_enabled` bit consumed by the renderer. External redacted capture must skip the rear-shadow composite even though it still draws redacted HUD glyphs.

- [ ] **Step 4: Add private pet-body coverage and closed draw selection**

Extend `SceneTargets` with:

```rust
pub(super) pet_body_coverage_texture: wgpu::Texture,
pub(super) pet_body_coverage_view: wgpu::TextureView,
pub(super) pet_body_coverage_bind_group: wgpu::BindGroup,
```

Use physical scene extent, `R8Unorm`, and `RENDER_ATTACHMENT | TEXTURE_BINDING`. Add a coverage pipeline that reuses `vs_scene` and the validated `PetBody` glyph draw but outputs atlas/body alpha only. Clear and encode it once per prepared scene frame. Do not include particles or any analytic draw.

Replace the old interaction-only selector with a closed pet composite plan:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct PetCompositeDrawPlan {
    body: ScenePlannedDraw,
    particles: ScenePlannedDraw,
}
```

It must resolve exactly one body and one particle draw, reject duplicates/wrong pipelines, and continue to exclude wall shadow, floor projection, props, and the reserved analytic slot 4.

- [ ] **Step 5: Add full-frame statistics-shadow and rim composites**

Pack the renderer-only scalar inputs without duplicating Rust style constants in WGSL:

```rust
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpatialCueGpuValue {
    statistics_offset_points: [f32; 2],
    statistics_softness_points: f32,
    statistics_opacity: f32,
    rim_radius_pixels: f32,
    rim_idle_alpha: f32,
    rim_activity_alpha_bonus: f32,
    rim_enabled: u32,
}
```

Build this fixed private uniform from `statistics_rear_shadow_style` plus the exported rim constants for the current physical target. Replace the two `FrameGlobalsGpuValue` padding words with `activity_opacity: f32` and `reduce_motion: u32`, and add packed `pet_rim_srgba8: u32` to `ContentGlobalsGpuValue`, derived from the existing mood constants during CPU compilation. Update Rust/WGSL ABI size/offset tests together. None of these values enter serde output or public debug output.

Add two source-over/multiply pipelines that draw one fixed full-screen triangle without adding scene primitives:

1. `statistics_rear_shadow`: sample the statistics R8 texture along the shared key-light offset with a fixed symmetric blur kernel, multiply the biome shadow tint by capped opacity, and clip to the aperture. Encode after the opaque room background but before prop shadows and all other world draws.
2. `pet_rim`: sample the maximum pet-body coverage in a physical-radius neighborhood, subtract center coverage, tint from packed `content_globals_buffer.globals.pet_rim_srgba8`, and apply the private style buffer plus `select(frame_globals.activity_opacity, 0.0, frame_globals.reduce_motion != 0u)`. Encode immediately before the body at its depth position.

Split the current world prefix so the one opaque room background draw can be encoded, then the optional private statistics projection, then the remaining world prefix. Keep the existing persistent blended order for every semantic draw.

For statistics crossing, render the rim in the same three logical portions as the pet body:

- normal behind-statistics contribution before the primary HUD;
- statistics-mask-limited reveal overlay using `statistics_coverage * reveal_mix`; and
- ordinary in-front contribution when the pet is above the statistics plane.

The body paints over the center of the rim. Do not write the rim into pet-body or statistics coverage, do not give it depth writes, and do not let it illuminate/shadow other surfaces.

- [ ] **Step 6: Keep capture and failure behavior closed**

Update `capture.rs` and `host.rs` so:

- direct sensitive/live retained capture includes the same rear projection and rim as the presented scene;
- independently constructed external redacted capture omits the statistics projection but may include the non-sensitive pet rim;
- missing coverage targets, invalid private plan, non-finite style input, or generation mismatch fails the render transaction before queue submission;
- public capture metrics, scene artifacts, and debug records gain no coverage counts, shadow pixels, HUD identities, or value-derived geometry.

In `paired_review.rs`, compare only non-sensitive style facts—statistics offset/softness/opacity, rim enabled/radius/alpha, pet pose, and facing—between the frozen AppKit preparation and retained preparation. Do not add either coverage mask or value-shaped projection pixels to the paired identity JSON.

- [ ] **Step 7: Run retained tests**

Run:

```bash
cargo test --features retained-renderer --lib companion::retained::hud::tests
cargo test --features retained-renderer --lib companion::retained::compiler::tests
cargo test --features retained-renderer --lib companion::retained::render::tests
cargo test --test companion_scene_boundary
cargo test --test retained_renderer_boundary
cargo test --test round_scene
```

Expected: PASS. Metal pixel tests may self-skip only when no compatible adapter is available; CPU shader/resource/privacy/plan tests must execute. External redacted capture has no value-shaped rear projection.

- [ ] **Step 8: Commit retained effects**

```bash
git status --short
git add src/companion/retained/hud.rs src/companion/retained/compiler.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl src/companion/retained/host.rs src/companion/retained/capture.rs src/companion/retained.rs src/companion/paired_review.rs
git diff --cached --check
git commit -m "feat(companion): add private retained spatial cues"
```

---

### Task 7: Replace the Motion Preview with Purposeful Locomotion and Add Visual QA Fixtures

**Files:**

- Modify: `src/dev_preview/smooth.rs`
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/round.rs`
- Modify: `tests/dev_preview.rs`

**Interfaces:**

- Consumes: deterministic locomotion segment metadata, existing round far/neutral/front fixtures, Preview Lab manifest schema 3
- Produces: paused locomotion strip covering dwell/turn/glide/depth, round visual fixtures for rear shadow/rim/aura absence, typed non-sensitive review facts

- [ ] **Step 1: Add failing Preview Lab contract tests**

Add tests asserting the Preview Lab output contains:

- a `round-purposeful-locomotion` strip with samples at dwell start, dwell end, glide quartiles, glide end, and the next segment boundary;
- at least one planar segment and one depth-excursion segment;
- typed facts for `segment_index`, `phase`, `segment_phase`, normalized destination/depth bucket, and facing;
- no `bob_offset` review prompt or `mood-aura` required role;
- round frames for content/sad/sleepy idle rims, active rim, rim-disabled fallback, and stats rear-wall shadow ordering with the pet behind, interacting with, and in front of statistics;
- deterministic sleep-settle and wake-resume frames using the same onset/resume instants as production;
- no exact HUD string, HUD coverage, atlas index, value-shaped shadow mask, or raw pet seed in manifest/JSON artifacts.

- [ ] **Step 2: Run Preview Lab tests and verify failure**

Run:

```bash
cargo test --features dev-preview --test dev_preview purposeful_locomotion
cargo test --features dev-preview --test dev_preview spatial_cues
```

Expected: FAIL because the current `round-smooth-motion` strip samples twelve 160 ms bob frames and the new review fixtures do not exist.

- [ ] **Step 3: Publish non-sensitive locomotion review metadata**

Replace the 1.76-second bob strip with a deterministic paused strip whose timestamps intentionally straddle one dwell and one glide plus a later depth excursion. Rename the contract artifact from bob-oriented facts to locomotion facts. Derive all published values from normalized public buckets:

```rust
pub struct PreviewLocomotionArtifact {
    pub segment_index: i64,
    pub phase: String,
    pub segment_phase: f32,
    pub planar_bucket: String,
    pub depth_bucket: String,
    pub facing: i8,
}
```

Do not publish route hashes, raw identity, exact destination coordinates, or private day transition instants. Keep physical motion geometry authoritative in the existing Smooth plan artifact.

- [ ] **Step 4: Add deterministic spatial-cue review frames**

Add round fixtures with static redacted HUD values that make the projection visible but non-sensitive. Their review prompts must ask:

- whether the stats projection reads as a rear receiving-surface cue rather than a readable duplicate;
- whether prop shadows retain their authored profiles and remain in front of the stats projection;
- whether the rim stays immediately outside the body without becoming a second silhouette;
- whether no broad radial aura remains;
- whether feed/activity changes rim intensity without changing size;
- whether the dwell/turn/glide strip reads like an animal choosing a destination rather than an oscillator.
- whether sleep settles to the held near-neutral pose and wake resumes without a discontinuity.

The `rim-disabled` fixture uses `PET_RIM_ENABLED = false` through a test-only presentation option, not by mutating the production constant.

- [ ] **Step 5: Run Preview Lab tests and export artifacts**

Run:

```bash
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo run -- dev-preview --scenario round --out target/glorp-preview
cargo run -- dev-preview --scenario animation --out target/glorp-preview
```

Expected: PASS. `target/glorp-preview/index.html` contains the new round cue frames and purposeful locomotion strip; `manifest.json` remains schema version 3 and contains no private coverage/value material.

- [ ] **Step 6: Inspect the generated review bundle**

Open:

```bash
open target/glorp-preview/index.html
```

Review the named prompts. If the rim reads fuzzy or duplicates glyph edges, set `PET_RIM_ENABLED` to `false`, update expected screenshots/contracts, and keep the no-rim result. If the statistics projection is readable as a second copy, increase softness or reduce opacity without changing its order or privacy model.

- [ ] **Step 7: Commit Preview Lab coverage**

```bash
git status --short
git add src/dev_preview/smooth.rs src/dev_preview/contract.rs src/dev_preview/round.rs tests/dev_preview.rs
git diff --cached --check
git commit -m "test(companion): preview spatial cues and purposeful motion"
```

---

### Task 8: Focused Verification, Scope Audit, and Optimized Companion Rebuild

**Files:**

- Modify only if a focused verification step exposes a defect in files already listed above.

**Interfaces:**

- Consumes: all completed motion, AppKit, Pixel, retained, privacy, and Preview Lab work
- Produces: one verified optimized `target/macos/Glorp.app` running the approved bundle

- [ ] **Step 1: Run formatting and focused behavior suites**

Run:

```bash
cargo fmt --check
cargo test --lib round::locomotion::tests
cargo test --lib round::motion::tests
cargo test --lib round::placement::tests
cargo test --lib presentation::companion_effects::tests
cargo test --lib presentation::props::tests
cargo test --lib presentation::pixel::animator::tests
cargo test --lib companion::app::tests
cargo test --features retained-renderer --lib presentation::companion_scene
cargo test --features retained-renderer --lib companion::retained::hud::tests
cargo test --features retained-renderer --lib companion::retained::compiler::tests
cargo test --features retained-renderer --lib companion::retained::render::tests
cargo test --features dev-preview --test dev_preview
cargo test --test smooth_companion
cargo test --test round_scene
cargo test --test companion_scene_boundary
cargo test --test retained_renderer_boundary
```

Expected: all commands PASS. Native GPU tests may self-skip only for missing compatible hardware; all CPU contracts and privacy tests run.

- [ ] **Step 2: Run lint and inspect the full planned diff**

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
git diff --check HEAD~7..HEAD
git diff --stat HEAD~7..HEAD
rg -n "draw_mood_aura|push_mood_aura|fs_mood_rings|MoodAuraRings|round_companion_bob|companion_wander_depth|companion_motion_energy" src tests
rg -n "lights\.clear\(\)|LitShallowCard|MAX_LIGHTS" src/presentation/companion_scene src/companion/retained
```

Expected: clippy passes; no whitespace errors; the removed aura/oscillator search returns no production implementation matches; dynamic lighting remains reserved/unsupported and production lights remain empty.

- [ ] **Step 3: Audit privacy and authored-shadow invariants**

Run:

```bash
rg -n "statistics_coverage|pet_body_coverage|rear_wall_projection" src/dev_preview tests/fixtures
rg -n "HabitatPropShadowProfile::(None|ContactOnly|Elevated)" src/game/habitat.rs src/presentation/props.rs
rg -n "981\.7M|349\.4k/10m|private-route-seed" src/companion/retained src/dev_preview
```

Expected: private coverage names are absent from serialized fixture/artifact code except tests explicitly proving exclusion; all three authored prop profiles remain; sentinel live values/seeds do not appear in production output paths.

- [ ] **Step 4: Rebuild and relaunch the optimized companion**

Run:

```bash
cargo xtask companion fresh
```

Expected: the optimized macOS app bundle builds, any running companion quits, and the fresh `target/macos/Glorp.app` opens.

- [ ] **Step 5: Perform the final live acceptance pass**

Observe at least one full 60-second segment plus the following segment's dwell. Confirm:

- the pet rests, turns while stationary, then glides without mid-path facing flicker;
- there is no two-second bob or rapid full-depth oscillation;
- stats read as suspended glass due to a faint rear-wall projection, not a readable duplicate;
- prop contact/elevated/no-shadow choices remain appropriate;
- no broad aura remains;
- the rim is faint and body-local, or is disabled if the preview fallback was selected;
- feed/activity changes the rim briefly without changing the locomotion path;
- Reduce Motion remains static and sleep/wake transitions do not jump.

- [ ] **Step 6: Commit only if verification required a scoped correction**

If verification exposed a defect within this plan's files, inspect the correction and return to the owning task's explicit file list and commit procedure. First run:

```bash
git status --short
git diff --check
```

Then stage only the correction using the owning task's `git add` list, verify `git diff --cached --check`, and commit it as `fix(companion): close spatial cue verification gaps`.

If no correction was needed, do not create an empty commit.

## Done Criteria

- Statistics retain their existing content/layout/Z and gain one faint private rear-wall projection behind world content.
- Existing authored prop shadow profiles and max-union compositing remain unchanged.
- No broad mood aura is drawn or serialized on Smooth, retained, Classic, or Pixel paths.
- Smooth/AppKit, retained, and Pixel use body-only exterior rim coverage; Classic uses the accepted no-rim fallback.
- Round motion is deterministic destination/dwell/glide locomotion with common X/Y/Z phase, bounded curve, stable facing, full-depth reachability, and no independent bob.
- Activity does not change locomotion geometry; sleep/wake and Reduce Motion follow their stable lifecycle contracts.
- External redacted capture omits the value-shaped statistics projection; private coverage never enters scene or Preview Lab artifacts.
- Focused tests, clippy, Preview Lab export, and optimized companion rebuild all pass.
