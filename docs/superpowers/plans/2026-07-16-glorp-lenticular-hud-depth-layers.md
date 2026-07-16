# Glorp Lenticular HUD Depth Layers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put the central statistics on a real mid-tank depth plane that the near pet can cross, while rendering the three perimeter gauge lanes as a staggered world-space bezel in front of the pet.

**Architecture:** Add one renderer-neutral depth-composition contract beside the existing pet depth sample. Smooth/AppKit consumes a precomputed semantic pass plan; the direct retained scene puts the sealed HUD marker and three gauge primitives into its transparent world order, splitting the world render around the private HUD draw. Pixel and Classic keep their flat fallback.

**Tech Stack:** Rust, AppKit, wgpu, WGSL, serde preview artifacts, Rust integration/unit tests, macOS companion xtask.

## Global Constraints

- Statistics Z is exactly `+0.72`; equality keeps the statistics in front of the pet.
- Gauge Z is exactly pace `+1.55`, daily `+1.65`, and XP `+1.75`.
- Pet reach, placement, scale, typography, HUD layout, gauge geometry, gauge values, and gauge colors do not change.
- The crossing group is exactly pet body, particles/performance cue, and mood aura.
- Wall shadow remains on the rear receiving surface; floor projection remains on the bed.
- Status/trouble remain screen chrome; dimming remains last.
- Smooth/AppKit and direct retained are the depth-aware acceptance paths.
- Pixel and Classic remain functional flat fallbacks.
- Live HUD values stay sealed, fixed-capacity, and excluded from scene snapshots, checksums, artifacts, and diagnostics.
- No multiview renderer, head tracking, quilt generation, stereo synthesis, or display calibration is added.
- No Linear issue is created or updated for Glorp.

---

### Task 1: Shared Depth Composition and Gauge-Lane Contract

**Files:**
- Modify: `src/round/depth.rs`
- Modify: `src/round/hud.rs`
- Modify: `src/presentation/smooth.rs`
- Modify: `src/round/smooth.rs`
- Test: `src/round/depth.rs`
- Test: `src/round/hud.rs`
- Test: `tests/smooth_companion.rs`

**Interfaces:**
- Produces: `COMPANION_STATISTICS_Z: f32`
- Produces: `COMPANION_GAUGE_PACE_Z: f32`, `COMPANION_GAUGE_DAILY_Z: f32`, `COMPANION_GAUGE_XP_Z: f32`
- Produces: `CompanionGaugeLane::{Pace, Daily, Xp}`
- Produces: `PetStatisticsOrder::{BehindStatistics, InFrontOfStatistics}`
- Produces: `CompanionGaugeDepthPlanes`
- Produces: `CompanionDepthComposition::resolve(effective_pet_z: f32) -> Result<Self, CompanionDepthCompositionError>`
- Produces: `SmoothCompanionPet::effective_depth: f32`
- Produces: `PreparedGaugeArc::lane: CompanionGaugeLane`
- Consumes: existing `SmoothDepthSample::effective_z`

- [ ] **Step 1: Add failing pure depth-contract tests**

Add tests that assert the exact planes, strict ordering, boundary behavior, and invalid-input behavior:

```rust
#[test]
fn companion_depth_planes_are_exact_and_strictly_ordered() {
    assert_eq!(COMPANION_STATISTICS_Z, 0.72);
    assert_eq!(COMPANION_GAUGE_PACE_Z, 1.55);
    assert_eq!(COMPANION_GAUGE_DAILY_Z, 1.65);
    assert_eq!(COMPANION_GAUGE_XP_Z, 1.75);
    assert!(COMPANION_STATISTICS_Z < 1.0);
    assert!(1.0 < COMPANION_GAUGE_PACE_Z);
    assert!(COMPANION_GAUGE_PACE_Z < COMPANION_GAUGE_DAILY_Z);
    assert!(COMPANION_GAUGE_DAILY_Z < COMPANION_GAUGE_XP_Z);
    assert!(COMPANION_GAUGE_XP_Z < 2.0);
}

#[test]
fn statistics_crossing_is_strictly_after_the_plane() {
    let boundary = CompanionDepthComposition::resolve(COMPANION_STATISTICS_Z).unwrap();
    assert_eq!(boundary.pet_statistics_order, PetStatisticsOrder::BehindStatistics);

    let just_crossed = CompanionDepthComposition::resolve(f32::from_bits(
        COMPANION_STATISTICS_Z.to_bits() + 1,
    ))
    .unwrap();
    assert_eq!(
        just_crossed.pet_statistics_order,
        PetStatisticsOrder::InFrontOfStatistics
    );
}

#[test]
fn companion_depth_composition_rejects_invalid_effective_depth() {
    for depth in [f32::NAN, f32::INFINITY, -1.01, 1.01] {
        assert!(CompanionDepthComposition::resolve(depth).is_err());
    }
}
```

Add a gauge test that checks the returned list is grouped `Pace`, `Daily`, `Xp`, while each lane retains track-before-fill and the daily rollover entries remain between daily track/fill and XP.

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
cargo test --lib round::depth::tests::companion_depth_planes_are_exact_and_strictly_ordered
cargo test --lib round::hud::tests::prepared_gauge_arcs_follow_deep_to_front_bezel_order
```

Expected: FAIL because the shared composition types/constants and lane identity do not exist.

- [ ] **Step 3: Implement the shared contract**

Add this contract to `src/round/depth.rs`:

```rust
pub const COMPANION_STATISTICS_Z: f32 = 0.72;
pub const COMPANION_GAUGE_PACE_Z: f32 = 1.55;
pub const COMPANION_GAUGE_DAILY_Z: f32 = 1.65;
pub const COMPANION_GAUGE_XP_Z: f32 = 1.75;
pub const COMPANION_PET_MAX_Z: f32 = 1.0;
pub const COMPANION_CAMERA_NEAR_Z: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompanionGaugeLane {
    Pace,
    Daily,
    Xp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PetStatisticsOrder {
    BehindStatistics,
    InFrontOfStatistics,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionGaugeDepthPlanes {
    pub pace: f32,
    pub daily: f32,
    pub xp: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionDepthComposition {
    pub pet_effective_z: f32,
    pub statistics_z: f32,
    pub gauges: CompanionGaugeDepthPlanes,
    pub pet_statistics_order: PetStatisticsOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionDepthCompositionError {
    InvalidEffectiveDepth,
    InvalidPlaneOrder,
}

impl CompanionGaugeLane {
    pub const fn scene_z(self) -> f32 {
        match self {
            Self::Pace => COMPANION_GAUGE_PACE_Z,
            Self::Daily => COMPANION_GAUGE_DAILY_Z,
            Self::Xp => COMPANION_GAUGE_XP_Z,
        }
    }
}

impl CompanionDepthComposition {
    pub fn resolve(
        pet_effective_z: f32,
    ) -> Result<Self, CompanionDepthCompositionError> {
        if !pet_effective_z.is_finite() || !(-1.0..=1.0).contains(&pet_effective_z) {
            return Err(CompanionDepthCompositionError::InvalidEffectiveDepth);
        }
        if !(COMPANION_STATISTICS_Z < COMPANION_PET_MAX_Z
            && COMPANION_PET_MAX_Z < COMPANION_GAUGE_PACE_Z
            && COMPANION_GAUGE_PACE_Z < COMPANION_GAUGE_DAILY_Z
            && COMPANION_GAUGE_DAILY_Z < COMPANION_GAUGE_XP_Z
            && COMPANION_GAUGE_XP_Z < COMPANION_CAMERA_NEAR_Z)
        {
            return Err(CompanionDepthCompositionError::InvalidPlaneOrder);
        }
        Ok(Self {
            pet_effective_z,
            statistics_z: COMPANION_STATISTICS_Z,
            gauges: CompanionGaugeDepthPlanes {
                pace: COMPANION_GAUGE_PACE_Z,
                daily: COMPANION_GAUGE_DAILY_Z,
                xp: COMPANION_GAUGE_XP_Z,
            },
            pet_statistics_order: if pet_effective_z > COMPANION_STATISTICS_Z {
                PetStatisticsOrder::InFrontOfStatistics
            } else {
                PetStatisticsOrder::BehindStatistics
            },
        })
    }
}
```

In `src/round/hud.rs`, add `lane: CompanionGaugeLane` to `PreparedGaugeArc`, pass the lane through `push_lane_arcs`/`push_daily_rollover_arc`, and emit lanes in this exact order:

```rust
push_lane_arcs(&mut arcs, CompanionGaugeLane::Pace, &layout.pace, &colors.pace, fractions.pace);
push_lane_arcs(
    &mut arcs,
    CompanionGaugeLane::Daily,
    &layout.daily,
    &colors.daily,
    fractions.daily,
);
for layer in daily_rollover_layers(fractions.daily_overage) {
    push_daily_rollover_arc(
        &mut arcs,
        CompanionGaugeLane::Daily,
        &layout.daily,
        layer.rollover,
        layer.fraction,
    );
}
push_lane_arcs(&mut arcs, CompanionGaugeLane::Xp, &layout.xp, &colors.xp, fractions.xp);
```

Keep `CompanionHudDepthPlane::FrontGlass` temporarily so production retained code compiles until Task 4 removes it.

Add `effective_depth: f32` beside raw `depth` in `SmoothCompanionPet`, default it to `0.0`, and populate it from `depth.effective_z` in `src/round/smooth.rs`. Do not change the meaning of `SmoothCompanionPet::depth`.

- [ ] **Step 4: Run shared and Smooth tests**

Run:

```bash
cargo test --lib round::depth::tests
cargo test --lib round::hud::tests
cargo test --test smooth_companion
```

Expected: PASS. Existing HUD formatting, gauge geometry/color, placement, shadow, and Smooth depth tests remain green.

- [ ] **Step 5: Commit**

```bash
git add src/round/depth.rs src/round/hud.rs src/presentation/smooth.rs src/round/smooth.rs tests/smooth_companion.rs
git commit -m "feat(companion): define lenticular HUD depth planes"
```

---

### Task 2: Precomputed Smooth/AppKit Depth Passes

**Files:**
- Modify: `src/companion/app.rs`
- Modify: `src/companion/paired_review.rs`
- Test: `src/companion/app.rs`
- Test: `tests/companion_draw_boundary.rs`
- Test: `src/companion/paired_review.rs`

**Interfaces:**
- Consumes: `CompanionDepthComposition`, `PetStatisticsOrder`, and `PreparedGaugeArc::lane`
- Consumes: `SmoothCompanionPet::effective_depth`
- Produces: `PreparedSmoothDepthPasses { world_before_statistics, pet_front, foreground }`
- Produces: a prepared Smooth identity that includes the depth composition and pass lists

- [ ] **Step 1: Add failing pure pass-planner tests**

Build a small plan fixture with one layer for each relevant `SmoothLayerRole` and assert:

```rust
assert_eq!(passes.pet_front_roles(), [
    SmoothLayerRole::PetBody,
    SmoothLayerRole::PerformanceCue,
]);
assert!(passes.world_roles().contains(&SmoothLayerRole::WallShadow));
assert!(passes.world_roles().contains(&SmoothLayerRole::FloorProjection));
assert_eq!(passes.foreground_roles(), [
    SmoothLayerRole::PropsForeground,
    SmoothLayerRole::TankLifeForeground,
]);
```

For depths `-1.0`, `0.0`, `0.72`, `f32::from_bits(0.72_f32.to_bits() + 1)`, and `1.0`, assert the pet pass is before statistics for the first three and after statistics for the last two. Assert mood aura follows that same branch even though it is painted externally.

Replace `tests/companion_draw_boundary.rs::appkit_front_glass_hud_follows_renderer_gauges_and_status_before_dim` with a depth-aware boundary test that prohibits these operations inside `paint_prepared_frame`:

```rust
for forbidden in [
    "prepared_perimeter_gauge_arcs(",
    ".sort_by",
    ".sort_by_key",
    "COMPANION_STATISTICS_Z",
] {
    assert!(!paint_body.contains(forbidden), "paint callback derived {forbidden}");
}
```

- [ ] **Step 2: Run the pass tests and verify failure**

Run:

```bash
cargo test --lib companion::app::tests::smooth_appkit_pass_plan -- --nocapture
cargo test --test companion_draw_boundary
```

Expected: FAIL because Smooth still stores one flat `draw_order` and prepares gauge arcs inside the native callback.

- [ ] **Step 3: Implement semantic pass preparation**

Add these private types in `src/companion/app.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PreparedSmoothDepthPasses {
    world_before_statistics: Vec<usize>,
    pet_front: Vec<usize>,
    foreground: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmoothDepthBucket {
    WorldBeforeStatistics,
    PetFront,
    Foreground,
    ScreenReservation,
}

fn smooth_depth_bucket(role: SmoothLayerRole) -> SmoothDepthBucket {
    match role {
        SmoothLayerRole::PetBody | SmoothLayerRole::PerformanceCue => {
            SmoothDepthBucket::PetFront
        }
        SmoothLayerRole::PropsForeground | SmoothLayerRole::TankLifeForeground => {
            SmoothDepthBucket::Foreground
        }
        SmoothLayerRole::StatusHalo
        | SmoothLayerRole::TroubleIndicator
        | SmoothLayerRole::MoodAura
        | SmoothLayerRole::DimOverlay => SmoothDepthBucket::ScreenReservation,
        SmoothLayerRole::DepthRings
        | SmoothLayerRole::BiomeWash
        | SmoothLayerRole::RoomGlyphs
        | SmoothLayerRole::TankBed
        | SmoothLayerRole::Ambient
        | SmoothLayerRole::Motes
        | SmoothLayerRole::ActivityGlyphs
        | SmoothLayerRole::PropsBehind
        | SmoothLayerRole::TankLifeBehind
        | SmoothLayerRole::ChestBubble
        | SmoothLayerRole::WallShadow
        | SmoothLayerRole::FloorProjection => SmoothDepthBucket::WorldBeforeStatistics,
    }
}
```

Prepare the three index vectors from the existing stable `smooth_layer_draw_order`; do not sort a second time. Store `CompanionDepthComposition` and `PreparedSmoothDepthPasses` in `PreparedRendererFrame::Smooth`.

Move `prepared_perimeter_gauge_arcs` into `prepare_companion_frame_at` and store the resulting `Vec<PreparedGaugeArc>` on `PreparedCompanionFrame`. The paint callback only iterates it.

Paint Smooth in this exact order:

```text
world_before_statistics
if BehindStatistics: mood aura, then pet_front layers
central statistics
if InFrontOfStatistics: mood aura, then pet_front layers
foreground layers
pace, daily, XP prepared gauge arcs
status/trouble
dim
```

Reuse `appkit_blit_smooth_plan(plan, indices, metrics, aperture)` for each slice. Do not classify `WallShadow` by `PetAttached`; role classification above is authoritative.

Pixel and Classic keep their current flat sequence and draw statistics after their scene/chrome path.

Extend the paired Smooth identity/checksum to record effective pet Z, statistics order, and the three pass lists instead of only the old flat order.

- [ ] **Step 4: Run AppKit and paired-identity tests**

Run:

```bash
cargo test --lib companion::app::tests
cargo test --test companion_draw_boundary
cargo test --features retained-renderer --lib companion::paired_review::tests
cargo test --features retained-renderer --test retained_renderer_boundary
```

Expected: PASS. Source-boundary tests confirm preparation owns allocation/sorting and the callback only consumes prepared passes.

- [ ] **Step 5: Commit**

```bash
git add src/companion/app.rs src/companion/paired_review.rs tests/companion_draw_boundary.rs
git commit -m "feat(companion): composite Smooth stats by tank depth"
```

---

### Task 3: Three World-Space Retained Gauge Lanes

**Files:**
- Modify: `src/presentation/companion_scene/scene.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/scene/checksum.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/companion/retained/compiler.rs`
- Modify: `src/companion/retained/render.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Test: the inline test modules in each file above
- Test: `tests/round_scene.rs`

**Interfaces:**
- Consumes: `CompanionGaugeLane::scene_z()` and the three gauge-plane constants
- Produces: `AnalyticSemantic::{GaugePace, GaugeDaily, GaugeXp}` at IDs `5`, `9`, and `10`
- Produces: exactly three `WorldReadOnly`/`World` gauge primitives
- Preserves: the existing closed `AnalyticPaint::PerimeterGaugeSet` and `AnalyticGeometry::PerimeterGaugeSet` payload ABI

- [ ] **Step 1: Add failing scene and renderer contract tests**

Add tests that require:

```rust
assert_eq!(AnalyticSemantic::GaugePace.id(), AnalyticParamId(5));
assert_eq!(AnalyticSemantic::GaugeDaily.id(), AnalyticParamId(9));
assert_eq!(AnalyticSemantic::GaugeXp.id(), AnalyticParamId(10));
```

For a production scene, assert one primitive each for `GaugePace`, `GaugeDaily`, and `GaugeXp`; each is `PrimitiveSpace::World`, `DepthBehavior::WorldReadOnly`, and source-over blended. Assert node Z values `1.55`, `1.65`, `1.75` and no `chrome.gauges` primitive.

Add a shader/packer test where each of the three semantics changes only its own annulus. The daily semantic alone must retain overage/rollover behavior.

- [ ] **Step 2: Run focused retained tests and verify failure**

Run:

```bash
cargo test --features retained-renderer --lib retained_scene_v2_has_typed_sources_and_fixed_semantic_tables
cargo test --features retained-renderer --lib analytic_bindings_require_their_exact_semantic_render_state
cargo test --features retained-renderer --lib gauge_shader_layers_completed_and_current_daily_rollovers
```

Expected: FAIL because production still has one screen-space `Gauges` semantic/primitive.

- [ ] **Step 3: Split gauge semantics without expanding the analytic ABI**

Change `AnalyticSemantic` so existing non-gauge IDs remain stable:

```rust
pub enum AnalyticSemantic {
    RoomBackground, // 0
    WallShadow,     // 1
    FloorProjection,// 2
    StatusHalo,     // 3
    MoodAura,       // 4
    GaugePace,      // 5
    Trouble,        // 6
    Dim,            // 7
    PropShadows,    // 8
    GaugeDaily,     // 9
    GaugeXp,        // 10
}
```

All three gauge semantics keep `AnalyticShape::PerimeterGaugeSet`, the same `PerimeterGaugeSet` paint payload, and the same `PerimeterGaugeSet` geometry payload. Add:

```rust
impl AnalyticSemantic {
    pub const fn gauge_lane(self) -> Option<crate::round::depth::CompanionGaugeLane> {
        match self {
            Self::GaugePace => Some(crate::round::depth::CompanionGaugeLane::Pace),
            Self::GaugeDaily => Some(crate::round::depth::CompanionGaugeLane::Daily),
            Self::GaugeXp => Some(crate::round::depth::CompanionGaugeLane::Xp),
            Self::RoomBackground
            | Self::WallShadow
            | Self::FloorProjection
            | Self::StatusHalo
            | Self::MoodAura
            | Self::Trouble
            | Self::Dim
            | Self::PropShadows => None,
        }
    }
}
```

Project the identical closed paint and geometry into all three analytic slots. In WGSL, select exactly one lane from the existing set based on semantic ID; only semantic `9` executes daily overage/rollover logic.

Replace `chrome.gauges` with nodes `world.gauge.pace`, `world.gauge.daily`, and `world.gauge.xp`, using Z `1.55`, `1.65`, and `1.75`. Each primitive is unlit analytic, premultiplied source-over, `WorldReadOnly`, and `World`.

Update exhaustive validation/checksum/packing tags without changing `MAX_ANALYTIC_PARAMS = 16` or `MAX_BLENDED_DRAWS = 256`.

At this intermediate task boundary, the production inventory is `22` primitives: `1` opaque, `17` world-blended, and `4` chrome (status, trouble, sealed screen HUD, dim).

- [ ] **Step 4: Run gauge, scene, checksum, and capacity tests**

Run:

```bash
cargo test --features retained-renderer --lib production_projection_closes_all_eleven_analytic_roles_with_y_up_geometry
cargo test --features retained-renderer --lib analytic_packers_preserve_all_eleven_closed_roles_exactly
cargo test --features retained-renderer --lib gauge_frame_flat_pack_matches_wgsl_vec4_reconstruction
cargo test --features retained-renderer --lib gauge_shader_layers_completed_and_current_daily_rollovers
cargo test --features retained-renderer --lib every_immutable_template_family_changes_static_checksum_and_compiled_data
cargo test --features retained-renderer --lib fixed_family_capacities_are_exact
cargo test --test round_scene
```

Expected: PASS after renaming the affected test functions and assertions to the new 11-role inventory. No lane is triple-rendered.

- [ ] **Step 5: Commit**

```bash
git add src/presentation/companion_scene/scene.rs src/presentation/companion_scene/scene/compiler.rs src/presentation/companion_scene/scene/checksum.rs src/presentation/companion_scene/validate.rs src/companion/retained/compiler.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl tests/round_scene.rs
git commit -m "feat(companion): stagger retained gauge depth planes"
```

---

### Task 4: Sorted World-Space Sealed Statistics in Retained

**Files:**
- Modify: `src/round/hud.rs`
- Modify: `src/presentation/companion_scene/scene/compiler.rs`
- Modify: `src/presentation/companion_scene/runtime.rs`
- Modify: `src/presentation/companion_scene/validate.rs`
- Modify: `src/companion/retained/compiler.rs`
- Modify: `src/companion/retained/render.rs`
- Modify: `src/companion/retained/hud.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Test: inline tests in all files above
- Test: `tests/round_scene.rs`

**Interfaces:**
- Consumes: `COMPANION_STATISTICS_Z` and `CompanionDepthComposition`
- Produces: one `WorldReadOnly`/`World` sealed HUD marker in the transparent sort
- Produces: `ScenePipelineClass::WorldHud`
- Produces: a three-pass world encoder: transparent prefix, sealed HUD, transparent suffix
- Preserves: separate nominal live/redacted HUD types, 26 fixed records, and 832-byte buffers

- [ ] **Step 1: Add failing retained HUD/order tests**

Add tests that assert:

- production has one HUD primitive in the world-blended phase and no screen HUD copy;
- chrome contains only status, trouble, and dim;
- pet body, particles, and mood aura use lifecycle-adjusted effective Z;
- wall/floor shadow nodes remain on their receiver planes;
- at pet Z `0.72`, authored-order tie breaking draws HUD after the pet;
- at the next `f32`, the pet sorts after HUD;
- pending blend-order changes commit/discard transactionally;
- changing `asleep` marks `PET_TRANSFORM` dirty even when raw depth is unchanged;
- HUD pipelines read depth and never write it.

- [ ] **Step 2: Run retained HUD/order tests and verify failure**

Run:

```bash
cargo test --features retained-renderer --lib draw_plan_preserves_world_phases_and_seals_the_canonical_chrome_schedule
cargo test --features retained-renderer --lib packed_blended_order_crossing_is_transactional_across_discard_and_retry
cargo test --features retained-renderer --lib dedicated_hud_shader_contract_matches_fixed_rust_abi_and_private_group
cargo test --features retained-renderer --lib pet_depth_projection_covers_active_calm_and_asleep_lifecycles
```

Expected: FAIL because HUD is still a no-depth chrome hook and pet node Z still uses raw depth.

- [ ] **Step 3: Move the sealed HUD marker into world transparency**

Remove `CompanionHudDepthPlane` and `COMPANION_HUD_DEPTH_PLANE` from `src/round/hud.rs`.

In the scene compiler:

- parent the statistics node under `scene.root` at Z `COMPANION_STATISTICS_Z`;
- use `Instances(InstanceGroupBinding::Hud)`, source-over blending, `WorldReadOnly`, and `World`;
- set pet transform Z to `resolved_effective_depth(snapshot)`;
- set mood-aura node Z to the same effective depth without applying the pet XY transform twice;
- leave wall shadow and floor projection node depth unchanged;
- classify an asleep-state change as `PET_TRANSFORM` dirty.

Replace the unused HUD record padding while preserving 32-byte ABI:

```rust
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct HudGlyphGpuValue {
    rect_points: [f32; 4],
    glyph_entry_index: u32,
    role: u32,
    visible: u32,
    scene_z: f32,
}
```

Populate `scene_z` from the shared composition during preparation. In `vs_hud`, project the screen-aligned point as world geometry:

```wgsl
let world = vec4<f32>(point_position, instance.scene_z, 1.0);
output.position = frame_buffer.globals.projection * frame_buffer.globals.view * world;
```

Add `ScenePipelineClass::WorldHud` with source-over blending, `depth_write_enabled: Some(false)`, and the existing sealed HUD bind-group layout.

The transparent order must include one typed HUD marker. Split rendering at that marker:

```text
pass 1: clear color/depth; opaque draws; transparent draws before HUD; store depth
pass 2: load color/depth; atomically stage and draw sealed HUD; store depth
pass 3: load color/depth; transparent draws after HUD
chrome: load color; status, trouble, dim
```

Route activation, lifetime, offscreen, direct-surface, and capture render paths through the same split helper. Keep exact HUD staging and drawing in one private call; never expose raw HUD records or bind groups.

Final production inventory is `22` primitives: `1` opaque, `18` world-blended including HUD, and `3` chrome.

- [ ] **Step 4: Run retained order, privacy, delta, and native pixel tests**

Run:

```bash
cargo test --features retained-renderer --lib draw_plan_preserves_world_phases_and_seals_the_canonical_chrome_schedule
cargo test --features retained-renderer --lib draw_plan_fails_closed_on_missing_duplicate_misplaced_or_untyped_draws
cargo test --features retained-renderer --lib pipeline_contracts_lock_entrypoints_blend_and_depth_behavior
cargo test --features retained-renderer --lib blend_modes_share_camera_depth_order_with_a_stable_semantic_tie_breaker
cargo test --features retained-renderer --lib packed_blended_order_crossing_is_transactional_across_discard_and_retry
cargo test --features retained-renderer --lib native_hud_hooks_reuse_caller_belt_render_redaction_and_keep_zero_slots_blank
cargo test --features retained-renderer --lib native_production_scene_renders_complete_fuzz_s3_inventory_with_redacted_hud
cargo test --features retained-renderer --lib native_delta_render_matches_fresh_upload_shadows_and_pixels
cargo test --test round_scene
```

Expected: PASS. Native change-mask tests prove boundary/just-crossed occlusion, gauge annuli remain visible, and wall/floor shadows never join the crossing group.

- [ ] **Step 5: Commit**

```bash
git add src/round/hud.rs src/presentation/companion_scene/scene/compiler.rs src/presentation/companion_scene/runtime.rs src/presentation/companion_scene/validate.rs src/companion/retained/compiler.rs src/companion/retained/render.rs src/companion/retained/hud.rs src/companion/retained/scene.wgsl tests/round_scene.rs
git commit -m "feat(companion): let near pets cross retained stats"
```

---

### Task 5: Deterministic Five-Depth Preview Contract

**Files:**
- Modify: `src/dev_preview/smooth.rs`
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Consumes: `CompanionDepthComposition`
- Produces: `round-smooth-depth-boundary` at `0.72`
- Produces: `round-smooth-depth-just-crossed` at `f32::from_bits(0.72_f32.to_bits() + 1)`
- Produces: typed `depth_composition` in each Smooth plan sidecar

- [ ] **Step 1: Add failing Preview Lab tests**

Extend the depth fixture table to five entries and assert exact semantic evidence:

```rust
const DEPTH_FIXTURES: [(&str, &str, f32); 5] = [
    ("round-smooth-depth-far", "far", -1.0),
    ("round-smooth-depth-neutral", "neutral", 0.0),
    ("round-smooth-depth-boundary", "boundary", 0.72),
    (
        "round-smooth-depth-just-crossed",
        "just-crossed",
        f32::from_bits(0.72_f32.to_bits() + 1),
    ),
    ("round-smooth-depth-front", "front", 1.0),
];
```

Assert boundary serializes `behind-statistics`, just-crossed serializes `in-front-of-statistics`, gauges serialize `1.55/1.65/1.75`, crossing roles exclude wall/floor, and two exports are byte-identical.

- [ ] **Step 2: Run the preview test and verify failure**

Run:

```bash
cargo test --features dev-preview --test dev_preview round_preview_exports_lenticular_depth -- --nocapture
```

Expected: FAIL because only far/neutral/front and the old Smooth sidecar exist.

- [ ] **Step 3: Add the fixtures and typed sidecar**

Add both scenario IDs to `src/dev_preview/smooth.rs` using the existing arbitrary `depth_override`. Add a serializable depth-composition projection containing:

```rust
pub struct PreviewDepthComposition {
    pub pet_effective_z: f32,
    pub statistics_z: f32,
    pub pet_statistics_order: PetStatisticsOrder,
    pub gauge_pace_z: f32,
    pub gauge_daily_z: f32,
    pub gauge_xp_z: f32,
    pub crossing_roles: Vec<String>,
    pub receiving_surface_roles: Vec<String>,
}
```

Populate crossing roles with `pet-body`, `performance-cue`, `mood-aura`; populate receiving surfaces with `wall-shadow`, `floor-projection`. Keep `.txt`/`.cells.json` explicitly semantic-only; they do not claim native occlusion proof.

- [ ] **Step 4: Run preview tests and export the deterministic bundle**

Run:

```bash
cargo test --features dev-preview --test dev_preview round_preview_exports_lenticular_depth -- --nocapture
cargo test --features dev-preview dev_preview::scenarios
cargo run --features dev-preview -- dev-preview --scenario round --out target/glorp-preview-hud-depth
```

Expected: PASS and the bundle contains five depth fixtures plus typed sidecars.

- [ ] **Step 5: Commit**

```bash
git add src/dev_preview/smooth.rs src/dev_preview/contract.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "test(companion): prove HUD depth crossing fixtures"
```

---

### Task 6: Truthful Smooth/Direct-Retained Native Review Pair

**Files:**
- Modify: `src/commands/companion_mode.rs`
- Modify: `src/cli.rs`
- Modify: `src/companion/app.rs`
- Modify: `src/companion/paired_review.rs`
- Modify: `src/companion/direct_capture.rs`
- Modify: `xtask/src/lib.rs`
- Test: inline tests in those files

**Interfaces:**
- Produces: native review depths `far`, `neutral`, `boundary`, `just-crossed`, `near`
- Produces: `cargo xtask companion review-pair --depth <value>`
- Produces: one manifest whose retained half comes from the live direct retained scene, not the legacy Smooth-plan translator
- Preserves: redaction defaults and sensitive-output root separation

- [ ] **Step 1: Add failing CLI, xtask, and manifest-validator tests**

Extend `CompanionReviewDepth`:

```rust
pub enum CompanionReviewDepth {
    Far,
    Neutral,
    Boundary,
    JustCrossed,
    Near,
}
```

Assert `normalized()` returns `-1.0`, `0.0`, `0.72`, the next `f32` after `0.72`, and `1.0`. Add xtask parser tests for all five values and forwarding of `--review-depth`.

Add manifest rejection tests for:

- retained route is `legacy` or fallback;
- Smooth and retained frozen-frame identities differ;
- depth-composition records differ;
- logical/physical extents differ;
- either capture checksum is missing.

- [ ] **Step 2: Run harness tests and verify failure**

Run:

```bash
cargo test --lib commands::companion_mode::tests::review_depth
cargo test -p xtask review_pair
cargo test --features retained-renderer --lib companion::paired_review::tests
```

Expected: FAIL because the enum and xtask expose only three depths and current direct routing writes `scene-manifest.json`, not a truthful pair.

- [ ] **Step 3: Implement one-frozen-frame direct pairing**

Add `--depth` to `parse_companion_review_pair` and forward it as `--review-depth`.

For review captures only, pin the horizontal motion origin so pet ink overlaps the central statistics. Ordinary launches retain normal motion.

When the retained route is direct/live, freeze one accepted semantic frame and:

1. render its Smooth prepared frame through `render_prepared_frame_to_rgba`;
2. capture the currently presented direct retained scene through `capture_presented_scene`;
3. require identical frozen semantic identity, depth composition, logical size, physical size, and backing scale;
4. write both PNGs and one pair manifest recording effective Z, statistics Z, all gauge Z values, ordering, route, and checksums;
5. reject fallback or legacy-translator retained output.

Keep the existing nominal redacted/live HUD separation. Exact live strings and glyph records do not enter the manifest.

- [ ] **Step 4: Run harness tests and five native pairs**

Run:

```bash
cargo test --lib commands::companion_mode::tests::review_depth
cargo test -p xtask review_pair
cargo test --features retained-renderer --lib companion::paired_review::tests

for depth in far neutral boundary just-crossed near; do
  cargo xtask companion review-pair \
    --size 360 \
    --state normal \
    --depth "$depth" \
    --out "target/glorp-review/hud-depth-360-$depth"
done
```

Expected: five valid pair manifests with direct-retained route, matching depth contracts, and nonblank PNGs. Single-view PNGs prove occlusion, not lenticular disparity.

- [ ] **Step 5: Commit**

```bash
git add src/commands/companion_mode.rs src/cli.rs src/companion/app.rs src/companion/paired_review.rs src/companion/direct_capture.rs xtask/src/lib.rs
git commit -m "test(companion): capture direct HUD depth pairs"
```

---

### Task 7: Final Verification and Companion Rebuild

**Files:**
- Verify only; no source changes expected

**Interfaces:**
- Consumes: all prior tasks
- Produces: a clean worktree, passing relevant suites, and a freshly rebuilt companion app

- [ ] **Step 1: Run formatting and focused suites**

```bash
cargo fmt --all -- --check
cargo test --lib round::depth::tests
cargo test --lib round::hud::tests
cargo test --test smooth_companion
cargo test --test companion_draw_boundary
cargo test --features retained-renderer --test round_scene
cargo test --features dev-preview --test dev_preview round_preview_exports_lenticular_depth -- --nocapture
```

Expected: PASS with no ignored failures.

- [ ] **Step 2: Run retained and full project verification**

```bash
cargo test --features retained-renderer companion::retained::compiler --lib
cargo test --features retained-renderer companion::retained::render --lib
cargo test --features retained-renderer companion::paired_review --lib
cargo test --features dev-preview --test dev_preview
cargo test --test round_scene
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS and clean output. If a broad pre-existing failure appears, record its exact command/output and prove the touched focused suites remain green; do not hide it.

- [ ] **Step 3: Inspect final history and diff**

```bash
git status --short --branch
git diff --check HEAD~6..HEAD
git log --oneline -8
```

Expected: no tracked or untracked implementation residue; six intentional feature/test commits follow the approved design and plan commits.

- [ ] **Step 4: Rebuild and relaunch the optimized companion**

```bash
cargo xtask companion fresh
```

Expected: optimized `target/macos/Glorp.app` is rebuilt, the old companion exits, and the fresh bundle opens.

- [ ] **Step 5: Live hardware review**

On the lenticular display, confirm:

- the pet is behind statistics at rear, neutral, and the exact boundary;
- the near pet crosses in front of statistics;
- bottom and rear shadows remain attached to their receiving surfaces;
- the pet never covers any gauge lane;
- the bezel reads deepest-to-front as pace, daily, XP.

This is the only acceptance step that judges lenticular disparity. The automated suite proves scene Z, ordering, occlusion, and renderer agreement.
