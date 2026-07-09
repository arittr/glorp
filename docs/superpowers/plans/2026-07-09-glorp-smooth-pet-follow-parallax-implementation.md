# Glorp Smooth Pet-Follow Parallax Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the existing Classic Glorp tank subtle, smooth pet-follow depth while preserving the stabilized pet path, Classic flatten parity, fixed companion chrome, and the prepared-frame safety boundary.

**Architecture:** Add renderer-neutral motion bindings and per-layer parallax evidence to the smooth scene plan. Derive one focus vector from the pet's continuous wander anchor, resolve bounded plane translations in a cfg-free Rust module with occupied-cell chrome safety, compose those translations into prepared smooth plans, and let AppKit paint moving bindings at fractional coordinates. Preview Lab and native review capture consume the same typed plan fields and prove continuity, plane ordering, parity, and privacy.

**Tech Stack:** Rust, ratatui geometry, existing `SmoothCompanionScenePlan`, serde/serde_json Preview Lab artifacts, AppKit/objc2 on macOS, existing companion review capture, Cargo test tooling.

## Global Constraints

- Implement `docs/superpowers/specs/2026-07-09-glorp-smooth-pet-follow-parallax-design.md` exactly.
- Start only after the prepared-frame entry gate in Task 0 passes.
- Keep the existing Classic pet art, tank props, tank life, HUD, gauges, and semantic art cadence.
- Use only the pet's existing continuous wander displacement as the parallax driver.
- Do not change pet wander, facing, bob, blink, breath, posture, or semantic art timing.
- Keep `DepthRings`, `StatusHalo`, `TroubleIndicator`, `DimOverlay`, HUD reservations, gauges, and aperture chrome fixed.
- Keep `PetBody`, `ContactShadow`, `PerformanceCue`, and `MoodAura` free of added parallax.
- Resolve all motion in cfg-free Rust before AppKit paints the prepared frame.
- Use occupied `SmoothLayerItem::LocalCell` geometry for Behind and Foreground chrome safety. Do not use aggregate `local_bounds` as collision geometry.
- Treat Shape or Raster items in Behind or Foreground planes as invalid until they expose explicit occupied bounds.
- Preserve exact Classic flatten checksums by continuing to ignore smooth transforms in the compatibility flatten path.
- Keep all error categories static and privacy-safe. Never pass NaN or infinity to AppKit.
- No scale, rotation, squash/stretch, velocity lean, spring physics, pointer input, event reaction, new art, default renderer change, or Linux windowing work.
- Use focused tests for this slice. Do not run the repository's full test suite unless a focused failure shows wider coverage is necessary.
- Stage only the explicit files listed in each task.

---

## Expected End State

Running `glorp companion --renderer smooth` shows the current Classic Glorp companion and current tank composition. As Glorp follows its existing continuous path, room texture, ambient marks, background objects, and foreground objects move in the same direction at small, progressively stronger fractions. Glorp and attached effects keep their current stabilized motion. Gauges, HUD, status overlays, trouble indicators, and porthole chrome stay fixed.

At the standard `960x960` review size, Foreground motion is visibly stronger than Far motion but never exceeds `0.5` columns or `0.25` rows. Calm mode uses half strength. Asleep mode uses quarter strength and wins when both asleep and calm are true. Preview and native evidence expose the focus, lifecycle scale, binding, depth plane, exact resolved parallax delta, and maximum adjacent-frame delta without reconstructing those values from pixels.

## File Map

| Path | Responsibility |
| --- | --- |
| `src/presentation/smooth.rs` | Portable motion-binding/depth types, per-layer parallax evidence, pet focus evidence, scene lifecycle evidence, and per-plane summaries. |
| `src/tui/panels/pet/layered.rs` | Assign every Classic-derived layer its explicit renderer-neutral motion binding. |
| `src/round/scene.rs` | Expose the neutral continuous pet motion origin beside current and Classic-snapped anchors. |
| `src/round/parallax.rs` | Pure parallax tuning, lifecycle precedence, finite validation, occupied-cell safety, and translation resolution. |
| `src/round/mod.rs` | Export the pure parallax module. |
| `src/round/smooth.rs` | Derive focus, compose pet-attached motion, resolve layer parallax, and map invalid geometry to the fallible plan error. |
| `tests/smooth_companion.rs` | Cross-module binding, parity, focus, composition, lifecycle, and continuity coverage. |
| `src/companion/app.rs` | Map plan errors, choose coordinate precision from motion binding, and record typed prepared-frame evidence. |
| `tests/companion_draw_boundary.rs` | Source-contract guard proving draw callbacks consume prepared frames only. |
| `src/dev_preview/contract.rs` | Additive smooth plan and motion artifact fields. |
| `src/dev_preview/smooth.rs` | Deterministic parallax strip and aggregate adjacent-delta evidence. |
| `tests/dev_preview.rs` | Sidecar schema, depth-plane, continuity, snap-boundary, parity, and privacy assertions. |
| `src/companion/review_capture.rs` | Native focus/per-plane samples and maximum adjacent parallax deltas. |

## Core Interfaces

Implement these concrete interfaces. Keep the names and field meanings stable unless the current compiler reveals a direct name collision.

```rust
// src/presentation/smooth.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothDepthPlane {
    Far,
    Mid,
    Behind,
    Foreground,
}

impl SmoothDepthPlane {
    pub const fn as_str(self) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothLayerMotionBinding {
    Fixed,
    PetAttached,
    Parallax(SmoothDepthPlane),
}

impl SmoothLayerMotionBinding {
    pub const fn as_str(self) -> &'static str;
    pub const fn depth_plane(self) -> Option<SmoothDepthPlane>;
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SmoothParallaxPlaneTranslations {
    pub far: SmoothPoint,
    pub mid: SmoothPoint,
    pub behind: SmoothPoint,
    pub foreground: SmoothPoint,
}
```

```rust
// New fields on existing presentation structs.
pub struct SmoothCompanionLayer {
    pub id: SmoothLayerId,
    pub role: SmoothLayerRole,
    pub motion_binding: SmoothLayerMotionBinding,
    pub z: i16,
    pub local_bounds: SmoothBounds,
    pub anchor: SmoothPoint,
    pub transform_origin: SmoothPoint,
    pub transform: SmoothTransform,
    pub parallax_translation: SmoothPoint,
    pub opacity: f32,
    pub clip: SmoothClip,
    pub blend: SmoothBlendMode,
    pub items: Vec<SmoothLayerItem>,
    pub privacy: SmoothCompanionPrivacyClaims,
}

pub struct SmoothCompanionScenePlan {
    pub viewport: CompanionViewport,
    pub layers: Vec<SmoothCompanionLayer>,
    pub pet: SmoothCompanionPet,
    pub parallax_lifecycle_scale: f32,
    pub chrome: CompanionChromeReservation,
    pub privacy: SmoothCompanionPrivacyClaims,
    pub(crate) classic_flatten_compat: SmoothClassicFlattenCompat,
}

pub struct SmoothCompanionPet {
    pub bounds: SmoothBounds,
    pub fractional_bounds: SmoothBounds,
    pub base_anchor: SmoothPoint,
    pub bob_offset: SmoothPoint,
    pub final_anchor: SmoothPoint,
    pub classic_snap_anchor: SmoothPoint,
    pub parallax_focus_offset: SmoothPoint,
}
```

```rust
// src/round/parallax.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallaxResolveError {
    NonFiniteGeometry,
    UnsupportedObjectGeometry,
}

pub const fn parallax_lifecycle_scale(asleep: bool, calm: bool) -> f32;

pub fn resolve_layer_parallax(
    focus: SmoothPoint,
    lifecycle_scale: f32,
    layer: &SmoothCompanionLayer,
    viewport: CompanionViewport,
    chrome: &CompanionChromeReservation,
) -> Result<SmoothPoint, ParallaxResolveError>;
```

```rust
// src/round/scene.rs
pub struct CompanionPetPlacement {
    pub fractional_motion_top_left: SmoothPetAnchor,
    pub fractional_motion_origin_top_left: SmoothPetAnchor,
    pub fractional_top_left: SmoothPetAnchor,
    pub classic_snap_top_left: (u16, u16),
    pub classic_rect: ratatui::layout::Rect,
}
```

## Task 0: Verify the Prepared-Frame Entry Gate

**Files:**
- Inspect: `src/companion/app.rs`
- Inspect: `src/round/smooth.rs`
- Create: `tests/companion_draw_boundary.rs`

**Interfaces:**
- Consumes: the committed prepared-frame boundary from the draw-boundary hardening slice.
- Produces: a go/no-go decision before any parallax edit.

- [ ] **Step 1: Verify the worktree and entry-gate call graph**

Run:

```bash
git status --short --branch
rg -n "fn draw_scene|fn paint_prepared_frame|fn prepare_companion_frame|try_build_round_smooth_scene_plan|last_good_frame" src/companion/app.rs src/round/smooth.rs
```

Expected:

- The worktree contains no unrelated uncommitted source edits. If it does, stop and preserve them before implementation.
- `draw_scene()` reads `last_good_frame` and calls `paint_prepared_frame()`.
- `prepare_companion_frame()` is called from the UI-tick path, not from `draw_scene()`.
- Smooth production preparation calls `try_build_round_smooth_scene_plan()`.

- [ ] **Step 2: Add an executable source-contract test for the boundary**

Create `tests/companion_draw_boundary.rs`:

```rust
const APP_SOURCE: &str = include_str!("../src/companion/app.rs");

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source marker {start}"));
    let tail = &source[start_index..];
    let end_offset = tail
        .find(end)
        .unwrap_or_else(|| panic!("missing source marker {end}"));
    &tail[..end_offset]
}

#[test]
fn draw_scene_consumes_only_the_last_prepared_frame() {
    let body = source_between(
        APP_SOURCE,
        "\nfn draw_scene(",
        "\nfn paint_prepared_frame(",
    );
    assert!(body.contains("state.last_good_frame.clone()"));
    assert!(body.contains("paint_prepared_frame(view, bounds, &frame)"));
    for forbidden in [
        "prepare_companion_frame(",
        "prepare_current_frame_from_state(",
        "build_round_scene_draw_list(",
        "try_build_round_smooth_scene_plan(",
        "companion_hud_text(",
        "SmoothReviewFrameSample {",
    ] {
        assert!(
            !body.contains(forbidden),
            "draw_scene must not call {forbidden}"
        );
    }
}

#[test]
fn ui_tick_owns_preparation_and_smooth_uses_the_fallible_planner() {
    let tick = source_between(
        APP_SOURCE,
        "\nfn ui_tick()",
        "\nfn prepare_current_frame_from_state()",
    );
    let prepare_current = source_between(
        APP_SOURCE,
        "\nfn prepare_current_frame_from_state()",
        "\nfn record_frame_preparation_error(",
    );
    let prepare_frame = source_between(
        APP_SOURCE,
        "\nfn prepare_companion_frame(",
        "\nstruct AppState",
    );

    assert!(tick.contains("prepare_current_frame_from_state()"));
    assert!(prepare_current.contains("prepare_companion_frame("));
    assert!(prepare_current.contains("state.last_good_frame = Some(frame)"));
    assert!(prepare_frame.contains("try_build_round_smooth_scene_plan("));
}
```

- [ ] **Step 3: Run and commit the entry-gate test**

Run:

```bash
cargo test --test companion_draw_boundary
```

Expected: both tests pass against the committed draw-boundary implementation. If this test fails, stop and repair the prerequisite before adding parallax.

```bash
git add tests/companion_draw_boundary.rs
git diff --cached --check
git commit -m "test(companion): lock prepared-frame draw boundary"
```

## Task 1: Add Portable Motion Bindings and Plan Evidence

**Files:**
- Modify: `src/presentation/smooth.rs`
- Modify: `src/tui/panels/pet/layered.rs`
- Modify: `src/round/smooth.rs`
- Test: `src/presentation/smooth.rs`
- Test: `src/tui/panels/pet/layered.rs`

**Interfaces:**
- Produces: `SmoothDepthPlane`, `SmoothLayerMotionBinding`, explicit role mapping, `parallax_translation`, `parallax_focus_offset`, lifecycle scale, and per-plane summary.
- Consumes later: the pure resolver, Preview Lab, AppKit, and native review capture read these typed values.

- [ ] **Step 1: Add failing role-mapping tests**

Add to `src/presentation/smooth.rs` tests:

```rust
#[test]
fn current_smooth_roles_have_the_approved_motion_bindings() {
    use SmoothDepthPlane::{Behind, Far, Foreground, Mid};
    use SmoothLayerMotionBinding::{Fixed, Parallax, PetAttached};
    use SmoothLayerRole::*;

    let cases = [
        (DepthRings, Fixed),
        (BiomeWash, Parallax(Far)),
        (RoomGlyphs, Parallax(Far)),
        (Ambient, Parallax(Mid)),
        (Motes, Parallax(Mid)),
        (ActivityGlyphs, Parallax(Mid)),
        (PropsBehind, Parallax(Behind)),
        (TankLifeBehind, Parallax(Behind)),
        (ChestBubble, Parallax(Behind)),
        (ContactShadow, PetAttached),
        (PetBody, PetAttached),
        (PerformanceCue, PetAttached),
        (PropsForeground, Parallax(Foreground)),
        (TankLifeForeground, Parallax(Foreground)),
        (StatusHalo, Fixed),
        (TroubleIndicator, Fixed),
        (MoodAura, PetAttached),
        (DimOverlay, Fixed),
    ];

    for (role, expected) in cases {
        assert_eq!(role.motion_binding(), expected, "unexpected binding for {role:?}");
    }
}

#[test]
fn motion_binding_exposes_privacy_safe_contract_names() {
    use SmoothDepthPlane::*;
    use SmoothLayerMotionBinding::*;

    assert_eq!(Fixed.as_str(), "fixed");
    assert_eq!(PetAttached.as_str(), "pet-attached");
    assert_eq!(Parallax(Far).as_str(), "parallax");
    assert_eq!(Parallax(Far).depth_plane(), Some(Far));
    assert_eq!(Parallax(Mid).depth_plane(), Some(Mid));
    assert_eq!(Parallax(Behind).depth_plane(), Some(Behind));
    assert_eq!(Parallax(Foreground).depth_plane(), Some(Foreground));
    assert_eq!(Fixed.depth_plane(), None);
    assert_eq!(PetAttached.depth_plane(), None);
}
```

Run:

```bash
cargo test --lib presentation::smooth::tests::current_smooth_roles_have_the_approved_motion_bindings -- --exact
```

Expected: FAIL because the binding types and `motion_binding()` do not exist.

- [ ] **Step 2: Add the portable types and conservative role mapping**

Add the Core Interfaces types to `src/presentation/smooth.rs`, plus:

```rust
impl SmoothDepthPlane {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Far => "far",
            Self::Mid => "mid",
            Self::Behind => "behind",
            Self::Foreground => "foreground",
        }
    }
}

impl SmoothLayerMotionBinding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::PetAttached => "pet-attached",
            Self::Parallax(_) => "parallax",
        }
    }

    pub const fn depth_plane(self) -> Option<SmoothDepthPlane> {
        match self {
            Self::Parallax(plane) => Some(plane),
            Self::Fixed | Self::PetAttached => None,
        }
    }
}

impl SmoothLayerRole {
    pub const fn motion_binding(self) -> SmoothLayerMotionBinding {
        use SmoothDepthPlane::{Behind, Far, Foreground, Mid};
        use SmoothLayerMotionBinding::{Fixed, Parallax, PetAttached};

        match self {
            Self::BiomeWash | Self::RoomGlyphs => Parallax(Far),
            Self::Ambient | Self::Motes | Self::ActivityGlyphs => Parallax(Mid),
            Self::PropsBehind | Self::TankLifeBehind | Self::ChestBubble => Parallax(Behind),
            Self::ContactShadow | Self::PetBody | Self::PerformanceCue | Self::MoodAura => {
                PetAttached
            }
            Self::PropsForeground | Self::TankLifeForeground => Parallax(Foreground),
            _ => Fixed,
        }
    }
}
```

The wildcard is intentional: a future role remains fixed until it is explicitly added to a moving group and covered by the mapping test.

- [ ] **Step 3: Add zero-valued evidence fields to every constructor**

Add `motion_binding` and `parallax_translation` to `SmoothCompanionLayer`. In both `layer_from_draw_cells_with_anchor()` and `reservation_layer()`, initialize them with:

```rust
motion_binding: role.motion_binding(),
parallax_translation: SmoothPoint { x: 0.0, y: 0.0 },
```

Update every direct `SmoothCompanionLayer` test fixture in `src/presentation/smooth.rs` the same way. Add `parallax_focus_offset: SmoothPoint::default()` and `parallax_lifecycle_scale: 1.0` to existing scene and pet fixtures.

Do not change any transform in this task.

- [ ] **Step 4: Add a per-plane summary that reads resolved evidence only**

Add to `SmoothCompanionScenePlan`:

```rust
pub fn parallax_translations_by_plane(&self) -> SmoothParallaxPlaneTranslations {
    fn strongest_axis(current: f32, candidate: f32) -> f32 {
        if candidate.abs() > current.abs() { candidate } else { current }
    }

    fn strongest_point(current: SmoothPoint, candidate: SmoothPoint) -> SmoothPoint {
        SmoothPoint {
            x: strongest_axis(current.x, candidate.x),
            y: strongest_axis(current.y, candidate.y),
        }
    }

    let mut result = SmoothParallaxPlaneTranslations::default();
    for layer in &self.layers {
        match layer.motion_binding.depth_plane() {
            Some(SmoothDepthPlane::Far) => {
                result.far = strongest_point(result.far, layer.parallax_translation);
            }
            Some(SmoothDepthPlane::Mid) => {
                result.mid = strongest_point(result.mid, layer.parallax_translation);
            }
            Some(SmoothDepthPlane::Behind) => {
                result.behind = strongest_point(result.behind, layer.parallax_translation);
            }
            Some(SmoothDepthPlane::Foreground) => {
                result.foreground = strongest_point(result.foreground, layer.parallax_translation);
            }
            None => {}
        }
    }
    result
}
```

This method must read `parallax_translation`, not total `transform.translation`, because pet-anchor correction and bob are separate motion.

- [ ] **Step 5: Verify constructors and Classic behavior**

Run:

```bash
cargo test --lib presentation::smooth::tests
cargo test --lib tui::panels::pet::layered::tests
cargo test --test smooth_companion smooth_round_plan_flattens_to_classic_round_scene_for_fixed_fixture -- --exact
```

Expected: PASS. The flatten checksum remains unchanged because the new fields are evidence-only and all parallax values are zero.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/presentation/smooth.rs src/tui/panels/pet/layered.rs src/round/smooth.rs
git diff --cached --check
git commit -m "feat(companion): add smooth layer motion bindings"
```

## Task 2: Expose a Neutral Continuous Pet Motion Origin

**Files:**
- Modify: `src/round/scene.rs`
- Test: `src/round/scene.rs`

**Interfaces:**
- Produces: `CompanionPetPlacement::fractional_motion_origin_top_left`.
- Consumes later: `src/round/smooth.rs` derives focus as current continuous anchor minus this neutral origin.

- [ ] **Step 1: Add failing origin and breath-isolation tests**

Add to `src/round/scene.rs` tests:

```rust
#[test]
fn neutral_motion_origin_uses_the_same_bias_and_clamps_as_current_motion() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let motion = companion_roam_motion();
    let placement = companion_pet_placement_from_offsets_for_test(
        &vm,
        GOLDEN_GRID_COLS,
        GOLDEN_GRID_ROWS,
        &motion,
        0.0,
        0.0,
    );

    assert_eq!(
        placement.fractional_motion_top_left,
        placement.fractional_motion_origin_top_left
    );
    assert!(placement.fractional_motion_origin_top_left.x >= 0.0);
    assert!(placement.fractional_motion_origin_top_left.y >= 0.0);
    assert!(
        placement.fractional_motion_origin_top_left.x
            <= f32::from(GOLDEN_GRID_COLS.saturating_sub(PET_W))
    );
    assert!(
        placement.fractional_motion_origin_top_left.y
            <= f32::from(GOLDEN_GRID_ROWS.saturating_sub(PET_H))
    );
}

#[test]
fn classic_breath_changes_posture_but_not_continuous_motion_origin() {
    let motion = companion_roam_motion();
    let mut still = WatchViewModel::fixture_with_habitat_props();
    let mut breathed = still.clone();
    still.breath_offset_y = 0;
    breathed.breath_offset_y = 1;

    let still_placement = companion_pet_placement_from_offsets_for_test(
        &still,
        GOLDEN_GRID_COLS,
        GOLDEN_GRID_ROWS,
        &motion,
        0.4,
        -0.3,
    );
    let breathed_placement = companion_pet_placement_from_offsets_for_test(
        &breathed,
        GOLDEN_GRID_COLS,
        GOLDEN_GRID_ROWS,
        &motion,
        0.4,
        -0.3,
    );

    assert_eq!(
        still_placement.fractional_motion_top_left,
        breathed_placement.fractional_motion_top_left
    );
    assert_eq!(
        still_placement.fractional_motion_origin_top_left,
        breathed_placement.fractional_motion_origin_top_left
    );
    assert_ne!(still_placement.fractional_top_left, breathed_placement.fractional_top_left);
    assert_ne!(still_placement.classic_rect, breathed_placement.classic_rect);
}
```

Run:

```bash
cargo test --lib round::scene::tests::neutral_motion_origin_uses_the_same_bias_and_clamps_as_current_motion -- --exact
```

Expected: FAIL because the origin field does not exist.

- [ ] **Step 2: Calculate current and neutral anchors from the same geometry**

In `companion_pet_placement_from_offsets()`, after `base_x`, `base_y`, `bias`, `max_x`, and `max_y` are known, calculate:

```rust
let fractional_motion_origin_top_left = SmoothPetAnchor {
    x: (base_x as f32).clamp(0.0, max_x as f32),
    y: (base_y as f32 - bias).clamp(0.0, max_y as f32),
};
```

Return it in `CompanionPetPlacement`. Do not include `breath_offset_y`, smooth bob, posture, or Classic truncation in this value.

- [ ] **Step 3: Re-run placement and parity tests**

```bash
cargo test --lib round::scene::tests::neutral_motion_origin_uses_the_same_bias_and_clamps_as_current_motion -- --exact
cargo test --lib round::scene::tests::classic_breath_changes_posture_but_not_continuous_motion_origin -- --exact
cargo test --lib round::scene::tests::companion_pet_placement_matches_existing_classic_rect -- --exact
cargo test --test smooth_companion smooth_round_plan_flattens_to_classic_round_scene_for_fixed_fixture -- --exact
```

Expected: PASS. The new field is additive; Classic placement remains byte-for-byte equivalent at the draw-list level.

- [ ] **Step 4: Commit Task 2**

```bash
git add src/round/scene.rs
git diff --cached --check
git commit -m "feat(companion): expose neutral pet motion origin"
```

## Task 3: Build the Pure Bounded Parallax Resolver

**Files:**
- Create: `src/round/parallax.rs`
- Modify: `src/round/mod.rs`
- Test: `src/round/parallax.rs`

**Interfaces:**
- Consumes: focus, lifecycle scale, motion binding, occupied local cells, viewport, and chrome reservations.
- Produces: finite bounded `SmoothPoint` deltas or a typed resolver error.

- [ ] **Step 1: Add the resolver module and failing scalar tests**

Export `pub mod parallax;` from `src/round/mod.rs`. In the new module define these private constants:

```rust
const FAR_MULTIPLIER: f32 = 0.01;
const MID_MULTIPLIER: f32 = 0.02;
const BEHIND_MULTIPLIER: f32 = 0.03;
const FOREGROUND_MULTIPLIER: f32 = 0.045;
const VERTICAL_AXIS_SCALE: f32 = 0.75;
const MAX_PARALLAX_X: f32 = 0.5;
const MAX_PARALLAX_Y: f32 = 0.25;
const SAFETY_SCALES: [f32; 5] = [1.0, 0.75, 0.5, 0.25, 0.0];
const OVERLAP_EPSILON: f32 = 0.000_01;
```

Add tests covering:

```rust
#[test]
fn lifecycle_precedence_is_asleep_then_calm_then_normal() {
    assert_eq!(parallax_lifecycle_scale(false, false), 1.0);
    assert_eq!(parallax_lifecycle_scale(false, true), 0.5);
    assert_eq!(parallax_lifecycle_scale(true, false), 0.25);
    assert_eq!(parallax_lifecycle_scale(true, true), 0.25);
}

#[test]
fn raw_plane_magnitudes_are_ordered_and_directionally_symmetric() {
    let positive = SmoothPoint { x: 4.0, y: 3.0 };
    let negative = SmoothPoint { x: -4.0, y: -3.0 };
    let far = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Far).unwrap();
    let mid = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Mid).unwrap();
    let behind = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Behind).unwrap();
    let foreground = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Foreground).unwrap();
    let negative_foreground =
        raw_plane_delta(negative, 1.0, SmoothDepthPlane::Foreground).unwrap();

    assert!(far.x.abs() < mid.x.abs());
    assert!(mid.x.abs() < behind.x.abs());
    assert!(behind.x.abs() < foreground.x.abs());
    assert!(far.y.abs() < mid.y.abs());
    assert!(mid.y.abs() < behind.y.abs());
    assert!(behind.y.abs() < foreground.y.abs());
    assert_eq!(negative_foreground.x, -foreground.x);
    assert_eq!(negative_foreground.y, -foreground.y);
}

#[test]
fn raw_plane_delta_caps_axes_independently() {
    let delta = raw_plane_delta(
        SmoothPoint { x: 10_000.0, y: -10_000.0 },
        1.0,
        SmoothDepthPlane::Foreground,
    )
    .unwrap();

    assert_eq!(delta.x, MAX_PARALLAX_X);
    assert_eq!(delta.y, -MAX_PARALLAX_Y);
}

#[test]
fn zero_focus_and_lifecycle_attenuation_are_exact() {
    for plane in [
        SmoothDepthPlane::Far,
        SmoothDepthPlane::Mid,
        SmoothDepthPlane::Behind,
        SmoothDepthPlane::Foreground,
    ] {
        assert_eq!(
            raw_plane_delta(SmoothPoint::default(), 1.0, plane).unwrap(),
            SmoothPoint::default()
        );
    }

    let focus = SmoothPoint { x: 4.0, y: -3.0 };
    let normal = raw_plane_delta(focus, 1.0, SmoothDepthPlane::Mid).unwrap();
    let calm = raw_plane_delta(focus, 0.5, SmoothDepthPlane::Mid).unwrap();
    let asleep = raw_plane_delta(focus, 0.25, SmoothDepthPlane::Mid).unwrap();
    assert_eq!(calm.x, normal.x * 0.5);
    assert_eq!(calm.y, normal.y * 0.5);
    assert_eq!(asleep.x, normal.x * 0.25);
    assert_eq!(asleep.y, normal.y * 0.25);
}
```

Run:

```bash
cargo test --lib round::parallax::tests -- --nocapture
```

Expected: FAIL until the module implementation exists.

- [ ] **Step 2: Implement finite raw translation and fixed bindings**

Implement:

```rust
pub const fn parallax_lifecycle_scale(asleep: bool, calm: bool) -> f32 {
    if asleep { 0.25 } else if calm { 0.5 } else { 1.0 }
}

fn plane_multiplier(plane: SmoothDepthPlane) -> f32 {
    match plane {
        SmoothDepthPlane::Far => FAR_MULTIPLIER,
        SmoothDepthPlane::Mid => MID_MULTIPLIER,
        SmoothDepthPlane::Behind => BEHIND_MULTIPLIER,
        SmoothDepthPlane::Foreground => FOREGROUND_MULTIPLIER,
    }
}

fn raw_plane_delta(
    focus: SmoothPoint,
    lifecycle_scale: f32,
    plane: SmoothDepthPlane,
) -> Result<SmoothPoint, ParallaxResolveError> {
    if !focus.x.is_finite() || !focus.y.is_finite() || !lifecycle_scale.is_finite() {
        return Err(ParallaxResolveError::NonFiniteGeometry);
    }
    let multiplier = plane_multiplier(plane);
    Ok(SmoothPoint {
        x: (focus.x * multiplier * lifecycle_scale)
            .clamp(-MAX_PARALLAX_X, MAX_PARALLAX_X),
        y: (focus.y * multiplier * VERTICAL_AXIS_SCALE * lifecycle_scale)
            .clamp(-MAX_PARALLAX_Y, MAX_PARALLAX_Y),
    })
}
```

`resolve_layer_parallax()` must return zero for `Fixed` and `PetAttached` after finite geometry validation. It must return the raw delta directly for Far and Mid.

- [ ] **Step 3: Add failing occupied-cell safety tests**

Use a local `local_cell_layer()` test helper that creates identity-transform layers with explicit local cells. Add tests proving:

```rust
fn bounds(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> SmoothBounds {
    SmoothBounds {
        min: SmoothPoint { x: min_x, y: min_y },
        max: SmoothPoint { x: max_x, y: max_y },
    }
}

fn local_cell_layer(
    motion_binding: SmoothLayerMotionBinding,
    global_bounds: SmoothBounds,
    cells: &[(u16, u16)],
) -> SmoothCompanionLayer {
    let anchor = global_bounds.min;
    SmoothCompanionLayer {
        id: SmoothLayerId("parallax-test-layer".to_string()),
        role: SmoothLayerRole::PropsBehind,
        motion_binding,
        z: 0,
        local_bounds: SmoothBounds {
            min: SmoothPoint::default(),
            max: SmoothPoint {
                x: global_bounds.max.x - global_bounds.min.x,
                y: global_bounds.max.y - global_bounds.min.y,
            },
        },
        anchor,
        transform_origin: SmoothPoint::default(),
        transform: SmoothTransform {
            translation: SmoothPoint::default(),
            scale: SmoothPoint { x: 1.0, y: 1.0 },
            rotation_degrees: 0.0,
        },
        parallax_translation: SmoothPoint::default(),
        opacity: 1.0,
        clip: SmoothClip::None,
        blend: SmoothBlendMode::Normal,
        items: cells
            .iter()
            .map(|&(row, col)| {
                SmoothLayerItem::LocalCell(SmoothLocalCell {
                    row,
                    col,
                    glyph: Some("x".to_string()),
                    fg: None,
                    bg: None,
                    bold: false,
                })
            })
            .collect(),
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
    }
}
```

```rust
#[test]
fn sparse_object_cells_ignore_aggregate_bounds_false_positives() {
    let layer = local_cell_layer(
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind),
        SmoothBounds {
            min: SmoothPoint { x: 0.0, y: 0.0 },
            max: SmoothPoint { x: 12.0, y: 1.0 },
        },
        &[(0, 0), (0, 11)],
    );
    let chrome = CompanionChromeReservation {
        hud_bounds: vec![bounds(5.0, 0.0, 7.0, 1.0)],
        gauge_bounds: Vec::new(),
    };

    let delta = resolve_layer_parallax(
        SmoothPoint { x: 4.0, y: 0.0 },
        1.0,
        &layer,
        CompanionViewport { grid_cols: 20, grid_rows: 10 },
        &chrome,
    )
    .unwrap();

    assert!(delta.x > 0.0, "empty aggregate-bounds space must not suppress motion");
}

#[test]
fn object_safety_prevents_new_overlap_and_does_not_worsen_existing_overlap() {
    let clear_layer = local_cell_layer(
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground),
        bounds(0.0, 0.0, 1.0, 1.0),
        &[(0, 0)],
    );
    let chrome = CompanionChromeReservation {
        hud_bounds: vec![bounds(1.01, 0.0, 2.0, 1.0)],
        gauge_bounds: Vec::new(),
    };
    let clear_delta = resolve_layer_parallax(
        SmoothPoint { x: 20.0, y: 0.0 },
        1.0,
        &clear_layer,
        CompanionViewport { grid_cols: 20, grid_rows: 10 },
        &chrome,
    )
    .unwrap();
    assert_eq!(clear_delta, SmoothPoint::default());

    let overlapping_layer = local_cell_layer(
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground),
        bounds(0.5, 0.0, 1.5, 1.0),
        &[(0, 0)],
    );
    let existing_overlap_delta = resolve_layer_parallax(
        SmoothPoint { x: 20.0, y: 0.0 },
        1.0,
        &overlapping_layer,
        CompanionViewport { grid_cols: 20, grid_rows: 10 },
        &chrome,
    )
    .unwrap();
    assert_eq!(existing_overlap_delta, SmoothPoint::default());
}

#[test]
fn object_planes_reject_shape_and_raster_without_occupied_bounds() {
    for item in [
        SmoothLayerItem::Shape(SmoothShapeRef { name: "shape".to_string() }),
        SmoothLayerItem::Raster(SmoothRasterRef { name: "raster".to_string() }),
    ] {
        let mut layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind),
            bounds(0.0, 0.0, 1.0, 1.0),
            &[],
        );
        layer.items.push(item);
        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint { x: 1.0, y: 1.0 },
                1.0,
                &layer,
                CompanionViewport { grid_cols: 20, grid_rows: 10 },
                &CompanionChromeReservation::default(),
            ),
            Err(ParallaxResolveError::UnsupportedObjectGeometry)
        );
    }
}

#[test]
fn fixed_and_pet_attached_bindings_are_zero_and_non_finite_focus_is_rejected() {
    let viewport = CompanionViewport { grid_cols: 20, grid_rows: 10 };
    let chrome = CompanionChromeReservation::default();
    for binding in [
        SmoothLayerMotionBinding::Fixed,
        SmoothLayerMotionBinding::PetAttached,
    ] {
        let layer = local_cell_layer(binding, bounds(0.0, 0.0, 1.0, 1.0), &[(0, 0)]);
        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint { x: 10.0, y: -10.0 },
                1.0,
                &layer,
                viewport,
                &chrome,
            )
            .unwrap(),
            SmoothPoint::default()
        );
    }

    let moving = local_cell_layer(
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Far),
        bounds(0.0, 0.0, 1.0, 1.0),
        &[(0, 0)],
    );
    assert_eq!(
        resolve_layer_parallax(
            SmoothPoint { x: f32::NAN, y: 0.0 },
            1.0,
            &moving,
            viewport,
            &chrome,
        ),
        Err(ParallaxResolveError::NonFiniteGeometry)
    );
}
```

The helper must set `layer.anchor` to the supplied bounds minimum and localize test cells relative to that anchor, matching production layer construction.

- [ ] **Step 4: Implement exact occupied-cell overlap checks**

For Behind and Foreground:

1. Reject non-identity scale or non-zero rotation as unsupported object geometry for this slice.
2. Reject Shape or Raster items.
3. Build each baseline one-cell rectangle from `anchor + transform.translation + local cell position`.
4. Compare every occupied cell against every HUD and gauge reservation.
5. A baseline intersection area of zero requires a candidate area of zero.
6. A positive baseline intersection permits only candidate area less than or equal to baseline plus `OVERLAP_EPSILON`.
7. Try `SAFETY_SCALES` in order and return the first safe scaled delta.

Use these exact geometry helpers:

```rust
fn translated_cell_bounds(layer: &SmoothCompanionLayer, row: u16, col: u16, delta: SmoothPoint) -> SmoothBounds {
    let min = SmoothPoint {
        x: layer.anchor.x + layer.transform.translation.x + f32::from(col) + delta.x,
        y: layer.anchor.y + layer.transform.translation.y + f32::from(row) + delta.y,
    };
    SmoothBounds {
        min,
        max: SmoothPoint { x: min.x + 1.0, y: min.y + 1.0 },
    }
}

fn intersection_area(left: SmoothBounds, right: SmoothBounds) -> f32 {
    let width = (left.max.x.min(right.max.x) - left.min.x.max(right.min.x)).max(0.0);
    let height = (left.max.y.min(right.max.y) - left.min.y.max(right.min.y)).max(0.0);
    width * height
}

fn overlap_is_safe(before: SmoothBounds, after: SmoothBounds, reservation: SmoothBounds) -> bool {
    let baseline = intersection_area(before, reservation);
    let candidate = intersection_area(after, reservation);
    if baseline <= OVERLAP_EPSILON {
        candidate <= OVERLAP_EPSILON
    } else {
        candidate <= baseline + OVERLAP_EPSILON
    }
}
```

Validate the viewport is non-zero and every point used by the resolver is finite. Aperture cropping remains the renderer's clip responsibility; do not force occupied cells inside a rectangular viewport.

- [ ] **Step 5: Run resolver coverage**

```bash
cargo test --lib round::parallax::tests -- --nocapture
cargo test --lib presentation::smooth::tests
```

Expected: PASS for zero focus, symmetry, strict raw ordering before caps, independent caps, lifecycle precedence, fixed/pet zero movement, non-finite rejection, sparse-cell safety, no-new-overlap safety, no-worse-overlap safety, and unsupported object geometry.

- [ ] **Step 6: Commit Task 3**

```bash
git add src/round/parallax.rs src/round/mod.rs
git diff --cached --check
git commit -m "feat(companion): add bounded parallax resolver"
```

## Task 4: Compose Pet-Follow Parallax Into the Prepared Smooth Plan

**Files:**
- Modify: `src/round/smooth.rs`
- Modify: `tests/smooth_companion.rs`
- Test: `tests/smooth_companion.rs`

**Interfaces:**
- Consumes: neutral pet origin, typed layer bindings, lifecycle state, chrome reservations, and the pure resolver.
- Produces: a fallible prepared plan with resolved layer transforms and typed evidence.

- [ ] **Step 1: Add failing focus, lifecycle, and binding integration tests**

Extend the imports in `tests/smooth_companion.rs` to include `SmoothDepthPlane` and `SmoothLayerMotionBinding`. Add:

```rust
#[test]
fn smooth_plan_focus_is_continuous_wander_minus_neutral_origin() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);
    let placement = glorp::round::scene::companion_pet_placement(
        &vm, now, GRID_COLS, GRID_ROWS, &motion,
    );
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 500,
    );

    assert_eq!(
        plan.pet.parallax_focus_offset,
        glorp::presentation::smooth::SmoothPoint {
            x: placement.fractional_motion_top_left.x
                - placement.fractional_motion_origin_top_left.x,
            y: placement.fractional_motion_top_left.y
                - placement.fractional_motion_origin_top_left.y,
        }
    );
}

#[test]
fn smooth_plan_assigns_every_current_role_its_approved_binding() {
    use SmoothDepthPlane::{Behind, Far, Foreground, Mid};
    use SmoothLayerMotionBinding::{Fixed, Parallax, PetAttached};
    use SmoothLayerRole::*;

    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &glorp::round::scene::companion_roam_motion(),
        0,
    );
    let expected = [
        (DepthRings, Fixed),
        (BiomeWash, Parallax(Far)),
        (RoomGlyphs, Parallax(Far)),
        (Ambient, Parallax(Mid)),
        (Motes, Parallax(Mid)),
        (ActivityGlyphs, Parallax(Mid)),
        (PropsBehind, Parallax(Behind)),
        (TankLifeBehind, Parallax(Behind)),
        (ChestBubble, Parallax(Behind)),
        (ContactShadow, PetAttached),
        (PetBody, PetAttached),
        (PerformanceCue, PetAttached),
        (PropsForeground, Parallax(Foreground)),
        (TankLifeForeground, Parallax(Foreground)),
        (StatusHalo, Fixed),
        (TroubleIndicator, Fixed),
        (MoodAura, PetAttached),
        (DimOverlay, Fixed),
    ];

    for (role, binding) in expected {
        let layer = plan.layer_by_role(role).expect("current role should exist");
        assert_eq!(layer.motion_binding, binding, "unexpected binding for {role:?}");
    }
}

#[test]
fn smooth_plan_lifecycle_scale_uses_asleep_precedence() {
    let motion = glorp::round::scene::companion_roam_motion();
    let mut normal = parity_fixture();
    normal.day_context.asleep = false;
    normal.life_profile.calm_mode = false;
    let mut calm = normal.clone();
    calm.life_profile.calm_mode = true;
    let mut asleep_and_calm = calm.clone();
    asleep_and_calm.day_context.asleep = true;

    let normal_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &normal, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let calm_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &calm, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let asleep_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &asleep_and_calm, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );

    assert_eq!(normal_plan.parallax_lifecycle_scale, 1.0);
    assert_eq!(calm_plan.parallax_lifecycle_scale, 0.5);
    assert_eq!(asleep_plan.parallax_lifecycle_scale, 0.25);
}
```

Run:

```bash
cargo test --test smooth_companion smooth_plan_focus_is_continuous_wander_minus_neutral_origin -- --exact
cargo test --test smooth_companion smooth_plan_assigns_every_current_role_its_approved_binding -- --exact
cargo test --test smooth_companion smooth_plan_lifecycle_scale_uses_asleep_precedence -- --exact
```

Expected: FAIL because the builder does not populate focus, lifecycle scale, or resolved motion.

- [ ] **Step 2: Add the categorized plan error**

Extend `SmoothScenePlanError`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothScenePlanError {
    MissingPetBody,
    InvalidParallaxGeometry,
}
```

Its display text is `smooth scene has invalid parallax geometry`. Map every `ParallaxResolveError` to `InvalidParallaxGeometry`; do not expose resolver details to the native error log.

- [ ] **Step 3: Derive focus and lifecycle before resolving layers**

In `try_build_round_smooth_scene_plan()`, derive:

```rust
let parallax_focus_offset = SmoothPoint {
    x: placement.fractional_motion_top_left.x
        - placement.fractional_motion_origin_top_left.x,
    y: placement.fractional_motion_top_left.y
        - placement.fractional_motion_origin_top_left.y,
};
let round_scene = derive_round_scene_model(vm, now);
let parallax_lifecycle_scale = crate::round::parallax::parallax_lifecycle_scale(
    round_scene.lifecycle.asleep,
    round_scene.lifecycle.calm,
);
```

Construct `CompanionChromeReservation` before the final layer-resolution pass. Keep the existing HUD reservation source and `gauge_bounds()` call unchanged.

- [ ] **Step 4: Generalize existing pet-anchor composition by binding**

Replace the hard-coded three-role condition in the Classic-derived layer loop with:

```rust
if layer.motion_binding == SmoothLayerMotionBinding::PetAttached {
    layer.transform.translation.x += pet_anchor_delta.x;
    layer.transform.translation.y += pet_anchor_delta.y;
}
```

At this point the layered scene contains `ContactShadow`, `PetBody`, and `PerformanceCue` as pet-attached layers. `MoodAura` is added later from already-fractional pet bounds, so it receives the same binding but no second anchor correction.

Keep pet bob restricted to `PetBody`.

- [ ] **Step 5: Resolve and compose parallax after all layers exist**

After adding status, trouble, aura, and dim reservation layers, run:

```rust
for layer in &mut layers {
    let parallax_translation = crate::round::parallax::resolve_layer_parallax(
        parallax_focus_offset,
        parallax_lifecycle_scale,
        layer,
        viewport,
        &chrome,
    )
    .map_err(|_| SmoothScenePlanError::InvalidParallaxGeometry)?;
    layer.parallax_translation = parallax_translation;
    layer.transform.translation.x += parallax_translation.x;
    layer.transform.translation.y += parallax_translation.y;
}
```

Store `parallax_focus_offset` on `SmoothCompanionPet` and `parallax_lifecycle_scale` on `SmoothCompanionScenePlan`.

Pet metadata may be calculated before or after this pass because all pet-attached layers resolve zero parallax. Keep the existing `base_anchor`, `final_anchor`, fractional bounds, and aura center assertions passing.

- [ ] **Step 6: Add composition and parity tests**

Add:

```rust
#[test]
fn smooth_plan_composes_nonzero_parallax_without_moving_fixed_or_pet_layers() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        500,
    );

    assert_ne!(plan.pet.parallax_focus_offset, glorp::presentation::smooth::SmoothPoint::default());
    assert!(plan.layers.iter().any(|layer| {
        matches!(layer.motion_binding, SmoothLayerMotionBinding::Parallax(_))
            && layer.parallax_translation != glorp::presentation::smooth::SmoothPoint::default()
    }));
    for layer in &plan.layers {
        if matches!(
            layer.motion_binding,
            SmoothLayerMotionBinding::Fixed | SmoothLayerMotionBinding::PetAttached
        ) {
            assert_eq!(
                layer.parallax_translation,
                glorp::presentation::smooth::SmoothPoint::default(),
                "fixed and pet-attached layers must not receive parallax: {:?}",
                layer.role
            );
        }
    }
}

#[test]
fn nonzero_parallax_preserves_exact_classic_flatten_parity() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);
    let classic = build_round_scene_draw_list(&vm, now, GRID_COLS, GRID_ROWS, &motion);
    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 500,
    );

    assert!(smooth.layers.iter().any(|layer| {
        layer.parallax_translation != glorp::presentation::smooth::SmoothPoint::default()
    }));
    assert_eq!(smooth.flatten_classic_cells(), classic.draw_list);
}

#[test]
fn classic_breath_does_not_change_parallax_focus() {
    let motion = glorp::round::scene::companion_roam_motion();
    let mut still = parity_fixture();
    let mut breathed = still.clone();
    still.breath_offset_y = 0;
    breathed.breath_offset_y = 1;

    let still_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &still, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let breathed_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &breathed, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );

    assert_eq!(
        still_plan.pet.parallax_focus_offset,
        breathed_plan.pet.parallax_focus_offset
    );
}
```

- [ ] **Step 7: Run focused smooth-plan coverage**

```bash
cargo test --test smooth_companion
cargo test --lib round::parallax::tests
cargo test --lib round::scene::tests::companion_pet_placement_matches_existing_classic_rect -- --exact
```

Expected: PASS. In particular, the existing pet anchor, facing, breath/posture isolation, adjacent motion, and Classic parity tests remain green.

- [ ] **Step 8: Commit Task 4**

```bash
git add src/round/smooth.rs tests/smooth_companion.rs
git diff --cached --check
git commit -m "feat(companion): compose pet-follow parallax"
```

## Task 5: Paint Moving Bindings Fractionally and Preserve Failure Handling

**Files:**
- Modify: `src/companion/app.rs`
- Modify: `src/companion/review_capture.rs`
- Test: `src/companion/app.rs`
- Test: `src/companion/review_capture.rs`

**Interfaces:**
- Consumes: prepared plan `motion_binding` and `SmoothScenePlanError`.
- Produces: binding-driven coordinate precision and privacy-safe last-good-frame reuse for invalid parallax geometry.

- [ ] **Step 1: Add failing binding-precision and error-category tests**

Inside the macOS `src/companion/app.rs` tests add:

```rust
#[test]
fn moving_bindings_use_fractional_appkit_coordinates() {
    use crate::presentation::smooth::{SmoothDepthPlane, SmoothLayerMotionBinding};

    assert!(!motion_binding_uses_fractional_coordinates(
        SmoothLayerMotionBinding::Fixed
    ));
    assert!(motion_binding_uses_fractional_coordinates(
        SmoothLayerMotionBinding::PetAttached
    ));
    assert!(motion_binding_uses_fractional_coordinates(
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Far)
    ));
    assert!(motion_binding_uses_fractional_coordinates(
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground)
    ));

    let fractional = fractional_cell_to_point(10.1, 4.0, 30.0, 60.0, 0.0, 960.0);
    let snapped = cell_to_point(
        appkit_cell_axis(10.1),
        appkit_cell_axis(4.0),
        30.0,
        60.0,
        0.0,
        960.0,
    );
    assert!((fractional.0 - 303.0).abs() < 0.000_000_001);
    assert_eq!(snapped.0, 300.0);
}

#[test]
fn parallax_geometry_failure_has_a_distinct_static_category() {
    assert_eq!(
        CompanionFramePreparationError::SmoothInvalidParallaxGeometry.category(),
        "smooth-invalid-parallax-geometry"
    );
    assert!(should_record_frame_preparation_error(
        Some(CompanionFramePreparationError::SmoothMissingPetBody),
        CompanionFramePreparationError::SmoothInvalidParallaxGeometry,
    ));
}
```

Run:

```bash
cargo test --lib companion::app::tests::moving_bindings_use_fractional_appkit_coordinates -- --exact
cargo test --lib companion::app::tests::parallax_geometry_failure_has_a_distinct_static_category -- --exact
```

Expected: FAIL because the helper and error variant do not exist.

- [ ] **Step 2: Replace role inference with binding-driven precision**

Add:

```rust
fn motion_binding_uses_fractional_coordinates(binding: SmoothLayerMotionBinding) -> bool {
    matches!(
        binding,
        SmoothLayerMotionBinding::PetAttached | SmoothLayerMotionBinding::Parallax(_)
    )
}
```

In `appkit_blit_smooth_plan()`, replace the `SmoothLayerRole` match with:

```rust
let fractional = motion_binding_uses_fractional_coordinates(layer.motion_binding);
```

Keep the existing fractional and snapped coordinate functions. AppKit must not inspect role, z, id, glyph, or color to decide precision.

- [ ] **Step 3: Map invalid parallax planning to last-good-frame reuse**

Add `SmoothInvalidParallaxGeometry` to `CompanionFramePreparationError` and return the static category `smooth-invalid-parallax-geometry`.

Replace the catch-all smooth planner mapping with an explicit match:

```rust
.map_err(|err| match err {
    SmoothScenePlanError::MissingPetBody => {
        CompanionFramePreparationError::SmoothMissingPetBody
    }
    SmoothScenePlanError::InvalidParallaxGeometry => {
        CompanionFramePreparationError::SmoothInvalidParallaxGeometry
    }
})?;
```

Add `smooth-invalid-parallax-geometry` to `RENDER_LOG_ALLOWED_STRING_VALUES` in `src/companion/review_capture.rs`. Do not serialize the resolver's internal error or any layer data in the error string.

- [ ] **Step 4: Verify prepared-frame behavior and native tests**

```bash
cargo test --lib companion::app::tests::moving_bindings_use_fractional_appkit_coordinates -- --exact
cargo test --lib companion::app::tests::parallax_geometry_failure_has_a_distinct_static_category -- --exact
cargo test --lib companion::app::tests::repeated_frame_preparation_errors_are_throttled_per_category -- --exact
cargo test --lib companion::review_capture::tests::review_capture_records_boundary_health_without_private_strings -- --exact
```

Expected: PASS. Existing prepared-frame tests still prove that a failed new frame does not replace `last_good_frame`.

- [ ] **Step 5: Commit Task 5**

```bash
git add src/companion/app.rs src/companion/review_capture.rs
git diff --cached --check
git commit -m "fix(companion): preserve fractional parallax drawing"
```

## Task 6: Extend Preview Lab With Deterministic Parallax Evidence

**Files:**
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/smooth.rs`
- Modify: `tests/dev_preview.rs`
- Test: `src/dev_preview/contract.rs`
- Test: `src/dev_preview/smooth.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Consumes: typed focus, lifecycle, binding, depth plane, per-layer parallax delta, and per-plane plan summaries.
- Produces: additive sidecar fields and deterministic strip-level maximum adjacent deltas.

- [ ] **Step 1: Add additive artifact fields without changing schema version**

Keep `CONTRACT_SCHEMA_VERSION` at `1`; these fields are additive and existing consumers already tolerate extra JSON members.

Add:

```rust
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PreviewSmoothParallaxPlanesArtifact {
    pub far: PreviewSmoothPointArtifact,
    pub mid: PreviewSmoothPointArtifact,
    pub behind: PreviewSmoothPointArtifact,
    pub foreground: PreviewSmoothPointArtifact,
}
```

Add these fields to both `PreviewSmoothPlanArtifact` and `PreviewSmoothMotionArtifact`:

```rust
pub parallax_focus_offset: PreviewSmoothPointArtifact,
pub parallax_lifecycle_scale: f32,
pub parallax_planes: PreviewSmoothParallaxPlanesArtifact,
```

Add only to `PreviewSmoothMotionArtifact`:

```rust
pub max_adjacent_parallax_delta_by_plane: PreviewSmoothParallaxPlanesArtifact,
```

Add to `PreviewSmoothLayerArtifact` and `PreviewSmoothMotionLayerArtifact`:

```rust
pub motion_binding: String,
pub depth_plane: Option<String>,
pub parallax_translation: PreviewSmoothPointArtifact,
```

Populate them only from typed plan fields:

```rust
motion_binding: layer.motion_binding.as_str().to_string(),
depth_plane: layer
    .motion_binding
    .depth_plane()
    .map(|plane| plane.as_str().to_string()),
parallax_translation: PreviewSmoothPointArtifact::from_point(
    layer.parallax_translation,
),
```

Do not infer a plane from role, z, item content, or total transform.

- [ ] **Step 2: Make the motion artifact constructor accept strip aggregate evidence**

Change the constructor signature to:

```rust
pub fn from_scene_plan(
    strip_id: &str,
    frame_index: u16,
    elapsed_ms: u64,
    now: time::OffsetDateTime,
    semantic_art_tick_index: u64,
    vm: &WatchViewModel,
    plan: &SmoothCompanionScenePlan,
    max_adjacent_parallax_delta_by_plane: SmoothParallaxPlaneTranslations,
) -> Self
```

Implement:

```rust
impl From<SmoothParallaxPlaneTranslations> for PreviewSmoothParallaxPlanesArtifact {
    fn from(planes: SmoothParallaxPlaneTranslations) -> Self {
        Self {
            far: PreviewSmoothPointArtifact::from_point(planes.far),
            mid: PreviewSmoothPointArtifact::from_point(planes.mid),
            behind: PreviewSmoothPointArtifact::from_point(planes.behind),
            foreground: PreviewSmoothPointArtifact::from_point(planes.foreground),
        }
    }
}
```

The plan artifact uses `plan.parallax_translations_by_plane()`. The motion artifact uses that same current-frame summary plus the aggregate argument.

- [ ] **Step 3: Precompute strip plans and maximum adjacent deltas**

In `src/dev_preview/smooth.rs`, introduce:

```rust
struct SmoothMotionSample {
    index: usize,
    elapsed_ms: u64,
    now: time::OffsetDateTime,
    semantic_art_tick_index: u64,
    plan: crate::presentation::smooth::SmoothCompanionScenePlan,
}
```

Build all `MOTION_FRAME_COUNT` samples first. Calculate component-wise absolute adjacent deltas from `plan.parallax_translations_by_plane()` and retain the maximum for each axis and plane. Pass that aggregate to every motion sidecar so each sidecar is self-contained.

Use exact helper semantics:

```rust
fn point_delta(
    previous: SmoothPoint,
    current: SmoothPoint,
) -> SmoothPoint {
    SmoothPoint {
        x: (current.x - previous.x).abs(),
        y: (current.y - previous.y).abs(),
    }
}

fn max_point(left: SmoothPoint, right: SmoothPoint) -> SmoothPoint {
    SmoothPoint { x: left.x.max(right.x), y: left.y.max(right.y) }
}

fn plane_delta(
    previous: SmoothParallaxPlaneTranslations,
    current: SmoothParallaxPlaneTranslations,
) -> SmoothParallaxPlaneTranslations {
    SmoothParallaxPlaneTranslations {
        far: point_delta(previous.far, current.far),
        mid: point_delta(previous.mid, current.mid),
        behind: point_delta(previous.behind, current.behind),
        foreground: point_delta(previous.foreground, current.foreground),
    }
}

fn max_planes(
    left: SmoothParallaxPlaneTranslations,
    right: SmoothParallaxPlaneTranslations,
) -> SmoothParallaxPlaneTranslations {
    SmoothParallaxPlaneTranslations {
        far: max_point(left.far, right.far),
        mid: max_point(left.mid, right.mid),
        behind: max_point(left.behind, right.behind),
        foreground: max_point(left.foreground, right.foreground),
    }
}

fn max_adjacent_parallax_delta(
    samples: &[SmoothMotionSample],
) -> SmoothParallaxPlaneTranslations {
    samples.windows(2).fold(
        SmoothParallaxPlaneTranslations::default(),
        |maximum, pair| {
            let previous = pair[0].plan.parallax_translations_by_plane();
            let current = pair[1].plan.parallax_translations_by_plane();
            max_planes(maximum, plane_delta(previous, current))
        },
    )
}
```

Call `max_adjacent_parallax_delta(&samples)` once before exporting frames. Do not compare total layer transforms.

- [ ] **Step 4: Strengthen the reviewed motion-window contract**

Replace `smooth_motion_window_crosses_snapped_anchor()` with a predicate that requires all of these from the same sample window:

- at least two distinct Classic snap anchors;
- no adjacent pet final-anchor delta at or above one cell;
- at least one non-zero parallax focus;
- non-zero Far, Mid, Behind, and Foreground plane summaries in at least one frame;
- fixed and pet-attached layer `parallax_translation` values remain zero;
- at least one frame has strict resolved absolute ordering Far < Mid < Behind < Foreground on a non-zero axis;
- aggregate adjacent deltas do not exceed `0.15` cells horizontally or `0.10` cells vertically.

Keep `REVIEWED_MOTION_START_UNIX_MS` only if it satisfies the stronger predicate. Otherwise search deterministic 160 ms offsets within one 22-second wander period and return the first passing start. If no window passes, fall back to `ctx.fixed_now` and let the existing pinned-start unit test fail clearly.

- [ ] **Step 5: Add sidecar assertions for exact typed evidence**

Extend `dev_preview_smooth_motion_sidecars_show_fractional_progression_and_all_bundle_includes_them` to collect each sidecar and assert:

```rust
assert_eq!(artifact["schema_version"], 1);
assert_eq!(artifact["parallax_lifecycle_scale"], 1.0);
assert!(artifact["parallax_focus_offset"]["x"].is_number());
assert!(artifact["parallax_focus_offset"]["y"].is_number());
assert!(artifact["parallax_planes"]["far"]["x"].is_number());
assert!(artifact["parallax_planes"]["mid"]["x"].is_number());
assert!(artifact["parallax_planes"]["behind"]["x"].is_number());
assert!(artifact["parallax_planes"]["foreground"]["x"].is_number());
assert!(artifact["max_adjacent_parallax_delta_by_plane"]["foreground"]["x"]
    .as_f64()
    .unwrap()
    <= 0.15);
assert!(artifact["max_adjacent_parallax_delta_by_plane"]["foreground"]["y"]
    .as_f64()
    .unwrap()
    <= 0.10);
```

For every `layer_transforms` row assert:

- `motion_binding` is one of `fixed`, `pet-attached`, or `parallax`;
- `depth_plane` is null for fixed/pet-attached and one of `far`, `mid`, `behind`, `foreground` for parallax;
- `parallax_translation` is numeric;
- fixed and pet-attached values are exactly zero.

Across the strip assert non-zero focus, all four non-zero planes, strict raw ordering on at least one frame, at least two Classic snap anchors, and bounded adjacent deltas.

- [ ] **Step 6: Extend the privacy scan**

The existing Preview Lab privacy walker already scans every `*.smooth-plan.json` and `*.smooth-motion.json` file and rejects forbidden tokens without an allowlist. Keep that behavior. Add assertions that the enum strings `fixed`, `pet-attached`, `parallax`, `far`, `mid`, `behind`, and `foreground` appear only in the new binding/plane fields, while source names, exact token strings, project names, paths, prompts, responses, diagnostics, and unprojected pet seeds remain forbidden.

- [ ] **Step 7: Run deterministic preview checks and inspect the bundle**

```bash
cargo test --features dev-preview --lib dev_preview::smooth::tests -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_smooth -- --nocapture
cargo run -- dev-preview --scenario smooth --out target/glorp-preview-parallax
```

Expected:

- Tests pass.
- `target/glorp-preview-parallax/manifest.json` lists the smooth strip.
- Motion sidecars contain non-zero focus and all four depth planes.
- Fixed and pet-attached parallax fields are zero.
- Classic parity remains exact.
- The privacy report passes.

Open the generated review index:

```bash
open target/glorp-preview-parallax/index.html
```

- [ ] **Step 8: Commit Task 6**

```bash
git add src/dev_preview/contract.rs src/dev_preview/smooth.rs tests/dev_preview.rs
git diff --cached --check
git commit -m "test(dev-preview): capture smooth parallax evidence"
```

## Task 7: Capture Native Prepared-Frame Parallax Evidence

**Files:**
- Modify: `src/companion/review_capture.rs`
- Modify: `src/companion/app.rs`
- Test: `src/companion/review_capture.rs`

**Interfaces:**
- Consumes: `SmoothCompanionPet::parallax_focus_offset`, scene lifecycle scale, and `parallax_translations_by_plane()` from the prepared frame.
- Produces: privacy-safe native frame samples and maximum adjacent parallax deltas by plane.

- [ ] **Step 1: Add review-plane and frame-sample types**

Add:

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct SmoothReviewParallaxPlanes {
    pub far: SmoothReviewPoint,
    pub mid: SmoothReviewPoint,
    pub behind: SmoothReviewPoint,
    pub foreground: SmoothReviewPoint,
}

impl SmoothReviewParallaxPlanes {
    pub fn from_smooth_planes(
        planes: crate::presentation::smooth::SmoothParallaxPlaneTranslations,
    ) -> Self {
        Self {
            far: SmoothReviewPoint::from_smooth_point(planes.far),
            mid: SmoothReviewPoint::from_smooth_point(planes.mid),
            behind: SmoothReviewPoint::from_smooth_point(planes.behind),
            foreground: SmoothReviewPoint::from_smooth_point(planes.foreground),
        }
    }
}
```

Extend `SmoothReviewFrameSample` with:

```rust
pub parallax_focus_offset: SmoothReviewPoint,
pub parallax_lifecycle_scale: f32,
pub parallax_planes: SmoothReviewParallaxPlanes,
```

Update every review test fixture with explicit values. Do not use string-keyed maps for planes; fixed fields keep the schema bounded and privacy review simple.

- [ ] **Step 2: Track maximum adjacent plane deltas**

Add `max_adjacent_parallax_delta_by_plane: SmoothReviewParallaxPlanes` to `ReviewCapture`, initialize it with `Default`, and serialize it on `RenderLog`.

When `last_smooth_sample` exists, update each plane with the existing `max_point_delta()` helper:

```rust
fn max_parallax_plane_delta(
    maximum: SmoothReviewParallaxPlanes,
    previous: SmoothReviewParallaxPlanes,
    current: SmoothReviewParallaxPlanes,
) -> SmoothReviewParallaxPlanes {
    SmoothReviewParallaxPlanes {
        far: max_point_delta(maximum.far, previous.far, current.far),
        mid: max_point_delta(maximum.mid, previous.mid, current.mid),
        behind: max_point_delta(maximum.behind, previous.behind, current.behind),
        foreground: max_point_delta(
            maximum.foreground,
            previous.foreground,
            current.foreground,
        ),
    }
}
```

Extend `round_smooth_sample()` so focus and all four planes use `round_review_point()`, and lifecycle scale uses `round_sample()`.

- [ ] **Step 3: Add failing render-log evidence tests**

In `smooth_review_capture_records_requested_evidence_and_privacy`, give the three samples increasing focus and plane values, then assert:

```rust
assert_eq!(value["smooth_frame_samples"][0]["parallax_lifecycle_scale"], 1.0);
assert_eq!(
    value["smooth_frame_samples"][0]["parallax_focus_offset"]["x"],
    0.25
);
assert_eq!(
    value["smooth_frame_samples"][0]["parallax_planes"]["far"]["x"],
    0.01
);
assert_eq!(
    value["smooth_frame_samples"][0]["parallax_planes"]["foreground"]["x"],
    0.045
);
assert!(
    value["max_adjacent_parallax_delta_by_plane"]["foreground"]["x"]
        .as_f64()
        .unwrap()
        > 0.0
);
assert_eq!(value["privacy"]["source_names_visible"], false);
assert_eq!(value["privacy"]["exact_token_strings_visible"], false);
```

Run:

```bash
cargo test --lib companion::review_capture::tests::smooth_review_capture_records_requested_evidence_and_privacy -- --exact
```

Expected: FAIL until the new fields are serialized and aggregated.

- [ ] **Step 4: Populate native samples only from the prepared plan**

In the `PreparedRendererFrame::Smooth { plan, .. }` review-sample branch in `prepare_companion_frame()`, add:

```rust
parallax_focus_offset:
    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
        plan.pet.parallax_focus_offset,
    ),
parallax_lifecycle_scale: plan.parallax_lifecycle_scale,
parallax_planes:
    crate::companion::review_capture::SmoothReviewParallaxPlanes::from_smooth_planes(
        plan.parallax_translations_by_plane(),
    ),
```

Do not rebuild focus from anchors and do not infer planes from roles inside AppKit.

- [ ] **Step 5: Verify native review schema and privacy**

```bash
cargo test --lib companion::review_capture::tests -- --nocapture
cargo test --lib companion::app::tests -- --nocapture
```

Expected: PASS. The native render-log string walker still accepts only known static values, and every numeric field remains privacy-safe.

- [ ] **Step 6: Commit Task 7**

```bash
git add src/companion/review_capture.rs src/companion/app.rs
git diff --cached --check
git commit -m "test(companion): capture native parallax evidence"
```

## Task 8: Focused Verification and Live Acceptance

**Files:**
- Verify: all files changed in Tasks 1 through 7
- Generate: `target/glorp-preview-parallax/`
- Generate: `target/glorp-review/smooth-parallax-active/`
- Generate: `target/glorp-review/smooth-parallax-asleep/`

**Interfaces:**
- Consumes: the completed portable resolver, prepared plan, AppKit adapter, and evidence paths.
- Produces: objective go/no-go evidence plus Drew's visual acceptance at `960x960`.

- [ ] **Step 1: Run formatting and focused automated checks**

```bash
cargo fmt --check
cargo test --lib round::parallax::tests -- --nocapture
cargo test --lib round::scene::tests::neutral_motion_origin_uses_the_same_bias_and_clamps_as_current_motion -- --exact
cargo test --test smooth_companion
cargo test --features dev-preview --lib dev_preview::smooth::tests -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_smooth -- --nocapture
cargo test --lib companion::app::tests -- --nocapture
cargo test --lib companion::review_capture::tests -- --nocapture
```

Expected: all commands pass with no ignored failure. Do not substitute a full-suite run for these targeted contracts.

- [ ] **Step 2: Generate and inspect deterministic Preview Lab evidence**

```bash
rm -rf target/glorp-preview-parallax
cargo run -- dev-preview --scenario smooth --out target/glorp-preview-parallax
open target/glorp-preview-parallax/index.html
```

Inspect the smooth strip and sidecars. Confirm:

- The Classic cell frame remains visually unchanged.
- At least one frame has non-zero focus.
- Far, Mid, Behind, and Foreground translations are present.
- Foreground is stronger than Far before safety attenuation.
- Fixed and pet-attached parallax values are zero.
- The strip crosses a Classic snap boundary without a corresponding parallax jump.
- The manifest and privacy report pass.

- [ ] **Step 3: Capture normal active native evidence at the standard size**

```bash
rm -rf target/glorp-review/smooth-parallax-active
cargo run -- companion-app --renderer smooth --review-size 960x960 --review-state active-pulse --review-duration-ms 12000 --review-capture-dir target/glorp-review/smooth-parallax-active
```

Inspect:

```bash
jq '{renderer, review_state, requested_size, paint_frame_count, semantic_art_tick_count, pet_checksums_stable_within_semantic_ticks, max_adjacent_base_anchor_delta, max_adjacent_final_anchor_delta, max_adjacent_parallax_delta_by_plane, panic, callback_panic_count, frame_preparation_error_count, last_good_frame_reused_count, privacy}' target/glorp-review/smooth-parallax-active/render-log.json
open target/glorp-review/smooth-parallax-active/screenshot.png
```

Expected objective evidence:

- `renderer` is `smooth`.
- requested size is `960x960`.
- paint frames outnumber semantic art ticks.
- pet checksums are stable within each semantic tick.
- Far, Mid, Behind, and Foreground have non-zero samples during the capture.
- maximum adjacent parallax deltas are at most `0.15` x and `0.10` y for every plane.
- `panic` is false.
- callback panic count is zero.
- frame preparation error count is zero.
- last-good-frame reuse count is zero in the healthy capture.
- every privacy claim is false.

Expected visual evidence:

- The tank follows Glorp gently in the same direction.
- Foreground objects move more than room texture.
- Glorp's path, facing, bob, blink, and art cadence look unchanged.
- Props remain attached to the habitat.
- Gauges, HUD, status, trouble, and aperture chrome do not drift.
- No movement snaps when the Classic anchor crosses a cell boundary.

- [ ] **Step 4: Capture asleep/calm attenuation**

```bash
rm -rf target/glorp-review/smooth-parallax-asleep
cargo run -- companion-app --renderer smooth --review-size 960x960 --review-state asleep-calm --review-duration-ms 12000 --review-capture-dir target/glorp-review/smooth-parallax-asleep
```

Inspect:

```bash
jq '{review_state, smooth_frame_samples, max_adjacent_parallax_delta_by_plane, panic, callback_panic_count, frame_preparation_error_count, privacy}' target/glorp-review/smooth-parallax-asleep/render-log.json
open target/glorp-review/smooth-parallax-asleep/screenshot.png
```

Expected: every sampled `parallax_lifecycle_scale` is `0.25`, the visual motion is quieter than normal, and the same crash/privacy gates pass.

- [ ] **Step 5: Build and launch the companion Drew will spot-check**

```bash
node scripts/build-macos-companion-app.mjs --profile debug
target/debug/glorp companion --renderer smooth --review-size 960x960
```

Leave the smooth companion running for Drew. Do not flip the default renderer.

- [ ] **Step 6: Audit scope and repository state**

```bash
git status --short --branch
git diff main@{upstream} --stat
git log --oneline -8
```

Confirm the implementation contains no new art, scale, rotation, spring, pointer input, event reactions, renderer-default change, or Linux windowing changes. Confirm generated `target/` artifacts are untracked/ignored and no source edit remains uncommitted.

## Spec Coverage Matrix

| Approved requirement | Plan coverage |
| --- | --- |
| Prepared-frame entry gate | Task 0 |
| Explicit renderer-neutral bindings | Task 1 |
| Conservative default-to-fixed role behavior | Task 1 |
| Neutral continuous motion origin | Task 2 |
| Focus excludes breath, posture, and bob | Tasks 2 and 4 |
| Plane multipliers `0.01`, `0.02`, `0.03`, `0.045` | Task 3 |
| Vertical scale `0.75` and caps `0.5`/`0.25` | Task 3 |
| Lifecycle precedence asleep > calm > normal | Tasks 3 and 4 |
| Occupied-cell object safety | Task 3 |
| Shape/Raster object-plane rejection | Task 3 |
| Fixed and pet-attached zero parallax | Tasks 3 and 4 |
| Classic flatten parity | Tasks 1 and 4 |
| Binding-driven AppKit fractional precision | Task 5 |
| Fallible planning and last-good-frame behavior | Task 5 |
| Typed Preview Lab evidence and privacy | Task 6 |
| Native prepared-frame evidence and privacy | Task 7 |
| Snap-boundary continuity and four visible planes | Tasks 6 and 8 |
| `960x960` active and asleep live review | Task 8 |
| No out-of-scope polish or platform work | Global Constraints and Task 8 |

## Completion Criteria

This slice is complete only when all focused tests pass, Preview Lab contains exact typed evidence for four depth planes, native review logs show bounded continuous movement with no preparation failures, Classic flatten parity remains exact, and Drew confirms that the live `960x960` companion reads as the same Glorp tank with subtle depth rather than a new visual system.
