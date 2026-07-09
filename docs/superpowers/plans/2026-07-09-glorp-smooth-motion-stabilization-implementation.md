# Glorp Smooth Motion Stabilization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stabilize Smooth companion motion so the existing Classic Glorp companion art, props, tank life, HUD, and gauges repaint at smooth cadence without flashing or snapping between integer tank cells.

**Architecture:** Move pet placement into a shared round-scene resolver that returns both exact Classic snapped placement and continuous Smooth placement. Keep Classic flattening on snapped anchors while native Smooth rendering consumes fractional residuals, then split the macOS fast redraw clock from the slower Classic semantic art clock. Extend Preview Lab and native review capture so checksum and anchor evidence would catch the reported flashing and jumping.

**Tech Stack:** Rust, ratatui `Rect`, existing `WatchViewModel`, `SceneDrawList`, `SmoothCompanionScenePlan`, Preview Lab JSON artifacts, serde/serde_json, AppKit/objc2 companion host on macOS, existing `cargo xtask companion fresh` workflow.

## Global Constraints

- Implement `docs/superpowers/specs/2026-07-09-glorp-smooth-motion-stabilization-design.md`.
- Keep the smooth companion visually anchored to the current Classic Glorp companion.
- Decouple the fast native paint loop from slower Classic semantic/art updates.
- Preserve smooth repaint cadence for continuous transforms, bob, and future effects.
- Move the pet, aura, contact shadow, and pet-attached cues from a continuous anchor instead of a snapped `u16` anchor in smooth mode.
- Leave Classic and Pixel renderers behaviorally unchanged.
- No default flip to smooth mode.
- No replacement of generated Classic pet art.
- No new Pixel full-frame companion.
- No new 3D engine, physics engine, or authored asset pipeline.
- No broad visual redesign of gauges, HUD, tank props, or pet body art.
- No rewrite of `glorp watch`.
- No Linux windowing implementation in this slice.
- `drawRect` / `draw_scene(...)` stays render-only: it must not drain live updates, advance semantic art ticks, mutate `animation_frame`, or depend on AppKit redraw coalescing for state progress.
- The semantic art clock must use monotonic `Instant` timing, advance at most one Classic art tick per UI timer callback, and drop missed intervals after stalls, resizes, wakes, or resumes.
- Classic snap parity must reproduce today's `companion_drift_position(...)` contract: truncate each motion term toward zero with Rust's `as i32`, add terms to the integer base, then clamp to the grid.
- `ChestBubble` is prop-attached, not pet-attached, and remains snapped with the treasure chest in this slice.
- New JSON evidence artifacts must include privacy claims and must not expose source names, exact token strings, project names, file paths, prompts, responses, raw diagnostics, or unprojected pet seed values.

---

## Scope

This plan fixes the live smooth renderer bugs Drew observed: too-fast flashing and cell-jump motion. It does not add the richer hero polish, rim light, parallax, squash/stretch, or feed pulse language yet. Those polish passes should start only after this plan proves stable timing and continuous placement.

Expected end state:

- `glorp companion --renderer smooth` still shows Classic Glorp art and the existing tank world.
- Smooth mode redraws at the fast AppKit cadence while Classic semantic pet art changes no faster than the 250 ms cadence.
- Smooth pet tank motion is sub-cell continuous; bob remains continuous and is no longer the only changing motion field.
- Preview Lab smooth artifacts expose `base_anchor`, `bob_offset`, `final_anchor`, `classic_snap_anchor`, `pet_visual_checksum`, and `semantic_art_tick_index`.
- Native `render-log.json` proves `paint_frame_count > semantic_art_tick_count`, checksum stability within a semantic tick, privacy claims, and changing anchor samples.

## File Map

| Path | Responsibility |
| --- | --- |
| `src/round/scene.rs` | Shared Classic/Smooth companion pet placement; exact Classic snap contract; round layout helpers. |
| `src/round/smooth.rs` | Build `SmoothCompanionScenePlan` from shared placement; attach fractional residuals to pet-attached layers; expose fractional pet bounds/center for AppKit aura. |
| `src/presentation/smooth.rs` | Smooth plan structs, pet placement metadata, Classic flatten compatibility, privacy-safe pet visual checksum helper. |
| `tests/smooth_companion.rs` | Cross-module smooth parity and placement tests. |
| `src/dev_preview/smooth.rs` | Deterministic smooth motion strip that advances both elapsed paint time and `now`. |
| `src/dev_preview/contract.rs` | Smooth motion artifact fields for anchors, checksums, semantic tick indices, deltas, and privacy. |
| `tests/dev_preview.rs` | Preview artifact evidence and privacy tests. |
| `src/companion/smooth_timing.rs` | Testable smooth semantic clock for macOS companion state. |
| `src/companion/mod.rs` | Export `smooth_timing` inside the macOS companion module. |
| `src/companion/app.rs` | Apply two-clock smooth behavior; keep `draw_scene` render-only; pass motion/checksum samples into review capture; draw Smooth aura from fractional pet center. |
| `src/companion/review_capture.rs` | Native review log schema for paint frames, semantic ticks, checksum samples, anchor samples, and privacy claims. |

## Core Interfaces

Implement these concrete interfaces. Keep names unless an implementer finds a compile-time conflict with an existing symbol.

```rust
// src/round/scene.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothPetAnchor {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionPetPlacement {
    pub fractional_top_left: SmoothPetAnchor,
    pub classic_snap_top_left: (u16, u16),
    pub classic_rect: ratatui::layout::Rect,
}

pub fn companion_pet_placement(
    vm: &crate::tui::view_model::WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> CompanionPetPlacement;
```

```rust
// src/presentation/smooth.rs
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SmoothCompanionPet {
    pub bounds: SmoothBounds,
    pub fractional_bounds: SmoothBounds,
    pub base_anchor: SmoothPoint,
    pub bob_offset: SmoothPoint,
    pub final_anchor: SmoothPoint,
    pub classic_snap_anchor: SmoothPoint,
}

pub fn pet_visual_checksum(
    pet_art: &[String],
    pet_spans: &[crate::pet::render::StyledSegment],
) -> u64;
```

```rust
// src/companion/smooth_timing.rs
#[derive(Debug, Clone)]
pub struct SmoothSemanticClock {
    interval: std::time::Duration,
    next_due: std::time::Instant,
    tick_index: u64,
}

impl SmoothSemanticClock {
    pub fn new(started_at: std::time::Instant, interval: std::time::Duration) -> Self;
    pub fn consume_due_tick(&mut self, now: std::time::Instant) -> Option<u64>;
    pub fn tick_index(&self) -> u64;
}
```

```rust
// src/companion/review_capture.rs
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SmoothReviewFrameSample {
    pub bob_y: f32,
    pub semantic_art_tick_index: u64,
    pub pet_visual_checksum: u64,
    pub base_anchor: SmoothReviewPoint,
    pub bob_offset: SmoothReviewPoint,
    pub final_anchor: SmoothReviewPoint,
    pub classic_snap_anchor: SmoothReviewPoint,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SmoothReviewPoint {
    pub x: f32,
    pub y: f32,
}
```

## Task 1: Shared Companion Pet Placement

**Files:**
- Modify: `src/round/scene.rs`
- Test: `src/round/scene.rs`
- Test: `tests/smooth_companion.rs`

**Interfaces:**
- Produces: `SmoothPetAnchor`, `CompanionPetPlacement`, `companion_pet_placement(...)`.
- Consumes later: `src/round/smooth.rs` uses `CompanionPetPlacement` for fractional Smooth anchors and exact Classic parity.

- [ ] **Step 1: Add failing placement contract tests in `src/round/scene.rs`**

Add these tests inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn companion_pet_placement_matches_existing_classic_rect() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let motion = companion_roam_motion();
    let samples = [
        datetime!(2026-07-08 18:00:00 UTC),
        datetime!(2026-07-08 18:00:00.250 UTC),
        datetime!(2026-07-08 18:00:00.500 UTC),
        datetime!(2026-07-08 18:00:00.750 UTC),
    ];

    for now in samples {
        let placement = companion_pet_placement(&vm, now, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &motion);
        let scene = build_round_scene_draw_list(&vm, now, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &motion);

        assert_eq!(
            placement.classic_rect, scene.pet_rect,
            "shared placement must reproduce current Classic pet rect at {now}"
        );
        assert_eq!(
            placement.classic_snap_top_left,
            (scene.pet_rect.x, scene.pet_rect.y)
        );
    }
}

#[test]
fn companion_pet_placement_uses_classic_piecewise_truncation_contract() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.breath_offset_y = 1;
    let motion = CompanionMotion {
        wander_half: 8,
        drift_x_frac: 0.9,
        drift_y_frac: 0.6,
        drift_period_secs: 22,
        upward_bias: 0.5,
        wander: true,
    };
    let placement = companion_pet_placement_from_offsets_for_test(
        &vm,
        GOLDEN_GRID_COLS,
        GOLDEN_GRID_ROWS,
        &motion,
        -0.25,
        -0.25,
    );

    let cx = GOLDEN_GRID_COLS / 2;
    let cy = GOLDEN_GRID_ROWS / 2;
    let half_w = PET_W / 2;
    let half_h = PET_H / 2;
    let safe_x = cx.saturating_sub(half_w) as f32;
    let safe_y = cy.saturating_sub(half_h) as f32;
    let x_radius = safe_x * motion.drift_x_frac;
    let y_radius = safe_y * motion.drift_y_frac;
    let bias = motion.upward_bias * safe_y;
    let classic_x = (cx as i32 - half_w as i32 + (-0.25 * x_radius) as i32)
        .clamp(0, GOLDEN_GRID_COLS.saturating_sub(PET_W) as i32) as u16;
    let classic_drift_y = (cy as i32
        - half_h as i32
        - bias as i32
        + (-0.25 * y_radius) as i32)
        .clamp(0, GOLDEN_GRID_ROWS.saturating_sub(PET_H) as i32) as u16;
    let classic_y = (classic_drift_y + u16::from(vm.breath_offset_y))
        .min(GOLDEN_GRID_ROWS.saturating_sub(PET_H));

    assert_eq!(placement.classic_snap_top_left, (classic_x, classic_y));
    assert_ne!(
        placement.classic_snap_top_left.1 as i32,
        (placement.fractional_top_left.y.floor() as i32),
        "this fixture must prove Classic snap is not composite floor"
    );
}

#[test]
fn companion_pet_placement_exposes_fractional_top_left_for_roam_motion() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let motion = companion_roam_motion();
    let placement = companion_pet_placement(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GOLDEN_GRID_COLS,
        GOLDEN_GRID_ROWS,
        &motion,
    );

    assert!(
        placement.fractional_top_left.x.fract().abs() > f32::EPSILON
            || placement.fractional_top_left.y.fract().abs() > f32::EPSILON,
        "roam motion should preserve a fractional anchor for Smooth renderers"
    );
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test --test smooth_companion
cargo test round::scene::tests::companion_pet_placement -- --nocapture
```

Expected: compile fails because `companion_pet_placement` and `companion_pet_placement_from_offsets_for_test` do not exist.

- [ ] **Step 3: Implement shared placement in `src/round/scene.rs`**

Add the public structs near `CompanionMotion`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothPetAnchor {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionPetPlacement {
    pub fractional_top_left: SmoothPetAnchor,
    pub classic_snap_top_left: (u16, u16),
    pub classic_rect: Rect,
}
```

Replace the existing `companion_drift(...)` helper with placement helpers that preserve the exact Classic contract:

```rust
fn companion_motion_offsets(
    now: time::OffsetDateTime,
    motion: &CompanionMotion,
    energy: f32,
) -> (f32, f32) {
    if motion.wander {
        let (wx, wy) = companion_wander_offsets(now, motion.drift_period_secs);
        (wx * energy, wy * energy)
    } else {
        companion_drift_offsets(now, motion.drift_period_secs)
    }
}

fn companion_pet_placement_from_offsets(
    vm: &WatchViewModel,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    fx: f32,
    fy: f32,
) -> CompanionPetPlacement {
    let cx = grid_cols / 2;
    let cy = grid_rows / 2;
    let half_w = PET_W / 2;
    let half_h = PET_H / 2;
    let safe_x = cx.saturating_sub(half_w) as f32;
    let safe_y = cy.saturating_sub(half_h) as f32;
    let x_radius = safe_x * motion.drift_x_frac;
    let y_radius = safe_y * motion.drift_y_frac;
    let bias = motion.upward_bias * safe_y;
    let max_x = grid_cols.saturating_sub(PET_W);
    let max_y = grid_rows.saturating_sub(PET_H);

    let base_x = cx as i32 - half_w as i32;
    let base_y = cy as i32 - half_h as i32;
    let offset_x = fx * x_radius;
    let offset_y = fy * y_radius;

    let classic_x = (base_x + offset_x as i32).clamp(0, max_x as i32) as u16;
    let classic_drift_y =
        (base_y - bias as i32 + offset_y as i32).clamp(0, max_y as i32) as u16;
    let classic_y = (classic_drift_y + u16::from(vm.breath_offset_y)).min(max_y);

    let fractional_drift_x = (base_x as f32 + offset_x).clamp(0.0, max_x as f32);
    let fractional_drift_y = (base_y as f32 - bias + offset_y).clamp(0.0, max_y as f32);
    let fractional_y = (fractional_drift_y + f32::from(vm.breath_offset_y)).min(max_y as f32);

    CompanionPetPlacement {
        fractional_top_left: SmoothPetAnchor {
            x: fractional_drift_x,
            y: fractional_y,
        },
        classic_snap_top_left: (classic_x, classic_y),
        classic_rect: Rect::new(classic_x, classic_y, PET_W, PET_H),
    }
}

pub fn companion_pet_placement(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> CompanionPetPlacement {
    let energy = companion_motion_energy(vm);
    let (fx, fy) = companion_motion_offsets(now, motion, energy);
    companion_pet_placement_from_offsets(vm, grid_cols, grid_rows, motion, fx, fy)
}

#[cfg(test)]
fn companion_pet_placement_from_offsets_for_test(
    vm: &WatchViewModel,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    fx: f32,
    fy: f32,
) -> CompanionPetPlacement {
    companion_pet_placement_from_offsets(vm, grid_cols, grid_rows, motion, fx, fy)
}
```

Update `build_round_pet_layout(...)` to call `companion_pet_placement(...)`:

```rust
let placement = companion_pet_placement(vm.as_ref(), now, grid_cols, grid_rows, motion);
let new_pet_art = placement.classic_rect;
layout.pet_art = new_pet_art;
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test round::scene::tests::companion_pet_placement -- --nocapture
cargo test --test smooth_companion
cargo test --test round_scene
```

Expected: placement tests pass, smooth parity still passes, round scene tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/round/scene.rs tests/smooth_companion.rs
git commit -m "feat(smooth): share companion pet placement"
```

## Task 2: Smooth Plan Fractional Anchors Without Breaking Classic Flattening

**Files:**
- Modify: `src/presentation/smooth.rs`
- Modify: `src/round/scene.rs`
- Modify: `src/round/smooth.rs`
- Test: `src/presentation/smooth.rs`
- Test: `tests/smooth_companion.rs`

**Interfaces:**
- Consumes: `CompanionPetPlacement` from Task 1.
- Produces: richer `SmoothCompanionPet` metadata and smooth residual transforms for AppKit and Preview Lab.

- [ ] **Step 1: Add failing tests for Classic flatten and pet-attached residuals**

In `src/presentation/smooth.rs`, replace `flatten_classic_cells_projects_local_cells_through_anchor_and_translation` with a test that proves `SmoothCompanionScenePlan::flatten_classic_cells()` ignores smooth transforms while `LayeredPetScene` still projects its own local transforms:

```rust
#[test]
fn smooth_scene_plan_classic_flatten_ignores_smooth_layer_transform() {
    let layer = SmoothCompanionLayer {
        id: SmoothLayerId("pet-body".to_string()),
        role: SmoothLayerRole::PetBody,
        z: 0,
        local_bounds: SmoothBounds {
            min: SmoothPoint { x: 0.0, y: 0.0 },
            max: SmoothPoint { x: 8.0, y: 8.0 },
        },
        anchor: SmoothPoint { x: 10.0, y: 20.0 },
        transform_origin: SmoothPoint { x: 0.0, y: 0.0 },
        transform: SmoothTransform {
            translation: SmoothPoint { x: 0.75, y: 0.33 },
            scale: SmoothPoint { x: 1.0, y: 1.0 },
            rotation_degrees: 0.0,
        },
        opacity: 1.0,
        clip: SmoothClip::None,
        blend: SmoothBlendMode::Normal,
        items: vec![local_item(cell(1, 4, "X", Some(rgb(1, 2, 3)), None, false))],
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
    };
    let plan = SmoothCompanionScenePlan {
        viewport: CompanionViewport::default(),
        layers: vec![layer],
        pet: SmoothCompanionPet::default(),
        chrome: CompanionChromeReservation::default(),
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
        classic_flatten_compat: SmoothClassicFlattenCompat::None,
    };

    assert_eq!(
        plan.flatten_classic_cells(),
        SceneDrawList {
            cells: vec![DrawCell {
                row: 21,
                col: 14,
                glyph: Some("X".to_string()),
                fg: Some(rgb(1, 2, 3)),
                bg: None,
                bold: false,
            }],
        }
    );
}
```

In `tests/smooth_companion.rs`, add:

```rust
#[test]
fn smooth_round_plan_records_fractional_pet_anchors_without_breaking_flatten_parity() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);

    let classic = build_round_scene_draw_list(&vm, now, GRID_COLS, GRID_ROWS, &motion);
    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        now,
        GRID_COLS,
        GRID_ROWS,
        &motion,
        250,
    );

    assert_eq!(smooth.flatten_classic_cells(), classic.draw_list);
    assert_eq!(smooth.pet.bounds.min.x, smooth.pet.classic_snap_anchor.x);
    assert_eq!(smooth.pet.bounds.min.y, smooth.pet.classic_snap_anchor.y);
    assert!(
        (smooth.pet.base_anchor.x - smooth.pet.classic_snap_anchor.x).abs() > f32::EPSILON
            || (smooth.pet.base_anchor.y - smooth.pet.classic_snap_anchor.y).abs() > f32::EPSILON,
        "smooth plan should preserve fractional residual separate from Classic snap"
    );
    assert_ne!(smooth.pet.final_anchor, smooth.pet.base_anchor);
}

#[test]
fn smooth_round_plan_moves_pet_attached_layers_but_keeps_chest_bubble_snapped() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        250,
    );
    let pet_body = plan.layer_by_role(SmoothLayerRole::PetBody).unwrap();
    let contact_shadow = plan.layer_by_role(SmoothLayerRole::ContactShadow).unwrap();
    let performance_cue = plan.layer_by_role(SmoothLayerRole::PerformanceCue).unwrap();
    let chest_bubble = plan.layer_by_role(SmoothLayerRole::ChestBubble).unwrap();

    assert!(pet_body.transform.translation.x.abs() > f32::EPSILON);
    assert_eq!(contact_shadow.transform.translation.x, pet_body.transform.translation.x);
    assert_eq!(performance_cue.transform.translation.x, pet_body.transform.translation.x);
    assert_eq!(chest_bubble.transform.translation.x, 0.0);
    assert_eq!(chest_bubble.transform.translation.y, 0.0);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test presentation::smooth::tests::smooth_scene_plan_classic_flatten_ignores_smooth_layer_transform
cargo test --test smooth_companion smooth_round_plan_records_fractional_pet_anchors_without_breaking_flatten_parity -- --nocapture
cargo test --test smooth_companion smooth_round_plan_moves_pet_attached_layers_but_keeps_chest_bubble_snapped -- --nocapture
```

Expected: compile fails because `SmoothCompanionPet` lacks the new fields and `SmoothCompanionScenePlan::flatten_classic_cells()` still applies transforms.

- [ ] **Step 3: Update smooth plan data and flattening**

In `src/presentation/smooth.rs`, update `SmoothCompanionPet`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SmoothCompanionPet {
    pub bounds: SmoothBounds,
    pub fractional_bounds: SmoothBounds,
    pub base_anchor: SmoothPoint,
    pub bob_offset: SmoothPoint,
    pub final_anchor: SmoothPoint,
    pub classic_snap_anchor: SmoothPoint,
}
```

Split flattening so `SmoothCompanionScenePlan` ignores smooth transforms for Classic parity:

```rust
impl SmoothCompanionScenePlan {
    pub fn flatten_classic_cells(&self) -> SceneDrawList {
        let mut draw_list = flatten_layers_to_draw_list(&self.layers, FlattenTransformMode::Ignore);
        match self.classic_flatten_compat {
            SmoothClassicFlattenCompat::None => {}
            SmoothClassicFlattenCompat::UniformPortholeRecolor { grid_rows } => {
                crate::round::scene::apply_uniform_porthole_recolor(&mut draw_list, grid_rows);
            }
        }
        draw_list
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlattenTransformMode {
    Apply,
    Ignore,
}

fn flatten_layers_to_draw_list(
    layers: &[SmoothCompanionLayer],
    transform_mode: FlattenTransformMode,
) -> SceneDrawList {
    let mut ordered_layers: Vec<(usize, &SmoothCompanionLayer)> =
        layers.iter().enumerate().collect();
    ordered_layers.sort_by_key(|(index, layer)| (layer.z, *index));

    let mut cells = Vec::new();
    for (_, layer) in ordered_layers {
        let translation = match transform_mode {
            FlattenTransformMode::Apply => layer.transform.translation,
            FlattenTransformMode::Ignore => SmoothPoint { x: 0.0, y: 0.0 },
        };
        for item in &layer.items {
            if let SmoothLayerItem::LocalCell(cell) = item {
                cells.push(DrawCell {
                    row: classic_cell_axis(layer.anchor.y + translation.y + f32::from(cell.row)),
                    col: classic_cell_axis(layer.anchor.x + translation.x + f32::from(cell.col)),
                    glyph: cell.glyph.clone(),
                    fg: cell.fg,
                    bg: cell.bg,
                    bold: cell.bold,
                });
            }
        }
    }

    SceneDrawList { cells }
}
```

Update `LayeredPetScene::flatten_classic_cells()` to call `FlattenTransformMode::Apply`.

- [ ] **Step 4: Update `src/round/scene.rs` layout helper to expose placement**

Add a sibling helper:

```rust
pub(crate) fn build_round_pet_layout_with_placement<'a>(
    vm: &'a WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> (
    Cow<'a, WatchViewModel>,
    crate::tui::component::PetSceneLayout,
    CompanionPetPlacement,
) {
    let area = Rect::new(0, 0, grid_cols, grid_rows);
    let energy = companion_motion_energy(vm);
    let wander_width = PET_W + 2 * motion.wander_half;
    let (wx, fc) = crate::tui::wander::resolve_wander_offset(vm, now, wander_width);
    let facing = if motion.wander {
        companion_wander_facing(now, motion.drift_period_secs, energy, vm.facing)
    } else {
        fc
    };
    let vm: Cow<WatchViewModel> = if wx != vm.wander_offset_x || facing != vm.facing {
        Cow::Owned({
            let mut v = vm.clone();
            v.wander_offset_x = wx;
            v.facing = facing;
            v
        })
    } else {
        Cow::Borrowed(vm)
    };

    let ctx = RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(now));
    let mut layout = PetScene::compute_layout(area, vm.as_ref(), &ctx);
    let old_pet_art = layout.pet_art;
    let placement = companion_pet_placement(vm.as_ref(), now, grid_cols, grid_rows, motion);
    layout.pet_art = placement.classic_rect;
    for excl in &mut layout.exclusions {
        if *excl == old_pet_art {
            *excl = placement.classic_rect;
            break;
        }
    }

    (vm, layout, placement)
}
```

Then make existing `build_round_pet_layout(...)` wrap it and return `placement.classic_rect`.

- [ ] **Step 5: Apply fractional residuals in `src/round/smooth.rs`**

Import `build_round_pet_layout_with_placement`. In `build_round_smooth_scene_plan(...)`, replace:

```rust
let (vm, layout, pet_rect) = build_round_pet_layout(vm, now, grid_cols, grid_rows, motion);
```

with:

```rust
let (vm, layout, placement) =
    build_round_pet_layout_with_placement(vm, now, grid_cols, grid_rows, motion);
let pet_rect = placement.classic_rect;
```

Compute motion fields:

```rust
let residual = SmoothPoint {
    x: placement.fractional_top_left.x - f32::from(placement.classic_snap_top_left.0),
    y: placement.fractional_top_left.y - f32::from(placement.classic_snap_top_left.1),
};
let bob_offset = SmoothPoint {
    x: 0.0,
    y: smooth_pet_bob(elapsed_ms),
};
let base_anchor = SmoothPoint {
    x: placement.fractional_top_left.x,
    y: placement.fractional_top_left.y,
};
let final_anchor = SmoothPoint {
    x: base_anchor.x + bob_offset.x,
    y: base_anchor.y + bob_offset.y,
};
let classic_snap_anchor = SmoothPoint {
    x: f32::from(placement.classic_snap_top_left.0),
    y: f32::from(placement.classic_snap_top_left.1),
};
```

Apply transforms:

```rust
for mut layer in layered.layers {
    if matches!(
        layer.role,
        SmoothLayerRole::PetBody | SmoothLayerRole::ContactShadow | SmoothLayerRole::PerformanceCue
    ) {
        layer.transform.translation.x += residual.x;
        layer.transform.translation.y += residual.y;
    }
    if layer.role == SmoothLayerRole::PetBody {
        layer.transform_origin = SmoothPoint {
            x: (layer.local_bounds.max.x - layer.local_bounds.min.x) / 2.0,
            y: (layer.local_bounds.max.y - layer.local_bounds.min.y) / 2.0,
        };
        layer.transform.translation.y += bob_offset.y;
    }
    layers.push(layer);
}
```

Set pet metadata:

```rust
pet: SmoothCompanionPet {
    bounds: pet_bounds,
    fractional_bounds: SmoothBounds {
        min: final_anchor,
        max: SmoothPoint {
            x: final_anchor.x + f32::from(pet_rect.width),
            y: final_anchor.y + f32::from(pet_rect.height),
        },
    },
    base_anchor,
    bob_offset,
    final_anchor,
    classic_snap_anchor,
},
```

For `MoodAura`, use `final_anchor` to compute its center:

```rust
let fractional_pet_bounds = SmoothBounds {
    min: final_anchor,
    max: SmoothPoint {
        x: final_anchor.x + f32::from(pet_rect.width),
        y: final_anchor.y + f32::from(pet_rect.height),
    },
};
let fractional_pet_center = SmoothPoint {
    x: fractional_pet_bounds.min.x + (fractional_pet_bounds.max.x - fractional_pet_bounds.min.x) / 2.0,
    y: fractional_pet_bounds.min.y + (fractional_pet_bounds.max.y - fractional_pet_bounds.min.y) / 2.0,
};
```

Use `fractional_pet_bounds` and `fractional_pet_center` for the `MoodAura` reservation layer.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test presentation::smooth
cargo test --test smooth_companion
cargo test --test round_scene
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/presentation/smooth.rs src/round/scene.rs src/round/smooth.rs tests/smooth_companion.rs
git commit -m "feat(smooth): preserve fractional companion anchors"
```

## Task 3: Deterministic Preview Evidence for Anchors and Checksums

**Files:**
- Modify: `src/presentation/smooth.rs`
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/smooth.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Consumes: `SmoothCompanionPet` metadata from Task 2.
- Produces: preview JSON fields that prove bob, base anchor, final anchor, Classic snap, semantic tick index, and pet visual checksum behavior.

- [ ] **Step 1: Add failing Preview Lab assertions**

In `tests/dev_preview.rs`, update `dev_preview_smooth_motion_sidecars_show_fractional_progression_and_all_bundle_includes_them` to read the new fields:

```rust
let mut pet_visual_checksums = BTreeSet::new();
let mut base_anchors = Vec::new();
let mut final_anchors = Vec::new();
let mut classic_snap_anchors = BTreeSet::new();
let mut bob_offsets = BTreeSet::new();
for frame in frames {
    let path = frame["files"]["smooth_motion"].as_str().unwrap();
    let artifact = run.read_json(path);
    assert_eq!(artifact["schema_version"], 1);
    assert_eq!(artifact["strip_id"], SMOOTH_MOTION_ID);
    assert!(artifact["now_unix_ms"].as_i64().is_some());
    assert!(artifact["semantic_art_tick_index"].as_u64().is_some());
    assert!(artifact["pet_visual_checksum"].as_u64().is_some());
    assert_eq!(artifact["privacy"]["source_names_visible"], false);
    assert_eq!(artifact["privacy"]["exact_token_strings_visible"], false);

    let base = &artifact["pet_motion"]["base_anchor"];
    let final_anchor = &artifact["pet_motion"]["final_anchor"];
    let snap = &artifact["pet_motion"]["classic_snap_anchor"];
    let bob = &artifact["pet_motion"]["bob_offset"];

    base_anchors.push((base["x"].as_f64().unwrap(), base["y"].as_f64().unwrap()));
    final_anchors.push((
        final_anchor["x"].as_f64().unwrap(),
        final_anchor["y"].as_f64().unwrap(),
    ));
    classic_snap_anchors.insert(format!(
        "{:.1}:{:.1}",
        snap["x"].as_f64().unwrap(),
        snap["y"].as_f64().unwrap()
    ));
    bob_offsets.insert(format!(
        "{:.4}:{:.4}",
        bob["x"].as_f64().unwrap(),
        bob["y"].as_f64().unwrap()
    ));
    pet_visual_checksums.insert(artifact["pet_visual_checksum"].as_u64().unwrap());
}

assert!(base_anchors.windows(2).any(|pair| pair[0] != pair[1]));
assert!(classic_snap_anchors.len() >= 2);
assert!(bob_offsets.len() >= 5);
assert_eq!(
    pet_visual_checksums.len(),
    1,
    "Preview strip should prove paint motion changes without semantic art flashing"
);
for pair in final_anchors.windows(2) {
    let dx = (pair[1].0 - pair[0].0).abs();
    let dy = (pair[1].1 - pair[0].1).abs();
    assert!(dx < 1.0, "adjacent smooth x delta should stay sub-cell: {dx}");
    assert!(dy < 1.0, "adjacent smooth y delta should stay sub-cell: {dy}");
}
```

- [ ] **Step 2: Run Preview Lab test to verify failure**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_smooth_motion_sidecars_show_fractional_progression_and_all_bundle_includes_them -- --nocapture
```

Expected: fails because the JSON still contains only `anchor_x`, `anchor_y`, and `bob_y`, and smooth strips do not advance deterministic `now`.

- [ ] **Step 3: Add pet visual checksum helper**

In `src/presentation/smooth.rs`, add a checksum helper near `classic_flatten_checksum(...)`:

```rust
pub fn pet_visual_checksum(
    pet_art: &[String],
    pet_spans: &[crate::pet::render::StyledSegment],
) -> u64 {
    let mut hash = FNV_OFFSET;
    hash = hash_bytes(hash, b"pet-visual");
    for line in pet_art {
        hash = hash_bytes(hash, line.as_bytes());
        hash = hash_u8(hash, 0xff);
    }
    for span in pet_spans {
        hash = hash_u64(hash, span.line as u64);
        hash = hash_u64(hash, span.start as u64);
        hash = hash_u64(hash, span.end as u64);
        hash = hash_bytes(hash, palette_role_name(span.role).as_bytes());
    }
    hash
}

fn palette_role_name(role: crate::pet::render::PaletteRoleName) -> &'static str {
    match role {
        crate::pet::render::PaletteRoleName::Body => "body",
        crate::pet::render::PaletteRoleName::BodyGlow => "body-glow",
        crate::pet::render::PaletteRoleName::Eye => "eye",
        crate::pet::render::PaletteRoleName::Mouth => "mouth",
        crate::pet::render::PaletteRoleName::Accent => "accent",
        crate::pet::render::PaletteRoleName::Pattern => "pattern",
        crate::pet::render::PaletteRoleName::Particle => "particle",
        crate::pet::render::PaletteRoleName::Corruption => "corruption",
    }
}
```

Add a unit test:

```rust
#[test]
fn pet_visual_checksum_tracks_art_and_spans() {
    let pet_art = vec!["abc".to_string()];
    let spans = vec![crate::pet::render::StyledSegment {
        line: 0,
        start: 0,
        end: 1,
        role: crate::pet::render::PaletteRoleName::Eye,
    }];

    let checksum = pet_visual_checksum(&pet_art, &spans);
    assert_eq!(checksum, pet_visual_checksum(&pet_art, &spans));

    let mut changed_art = pet_art.clone();
    changed_art[0] = "abd".to_string();
    assert_ne!(checksum, pet_visual_checksum(&changed_art, &spans));

    let mut changed_spans = spans.clone();
    changed_spans[0].role = crate::pet::render::PaletteRoleName::Mouth;
    assert_ne!(checksum, pet_visual_checksum(&pet_art, &changed_spans));
}
```

- [ ] **Step 4: Expand smooth motion artifact schema**

In `src/dev_preview/contract.rs`, change `PreviewSmoothMotionArtifact` and `PreviewSmoothPetMotionArtifact`:

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothMotionArtifact {
    pub schema_version: u32,
    pub strip_id: String,
    pub frame_index: u16,
    pub elapsed_ms: u64,
    pub now_unix_ms: i128,
    pub semantic_art_tick_index: u64,
    pub pet_visual_checksum: u64,
    pub pet_motion: PreviewSmoothPetMotionArtifact,
    pub layer_transforms: Vec<PreviewSmoothMotionLayerArtifact>,
    pub chrome: PreviewSmoothChromeArtifact,
    pub abstract_state: BTreeMap<String, String>,
    pub privacy: PreviewSmoothPrivacyArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewSmoothPetMotionArtifact {
    pub base_anchor: PreviewSmoothPointArtifact,
    pub bob_offset: PreviewSmoothPointArtifact,
    pub final_anchor: PreviewSmoothPointArtifact,
    pub classic_snap_anchor: PreviewSmoothPointArtifact,
    pub scale_x: f32,
    pub scale_y: f32,
    pub opacity: f32,
    pub pulse: String,
}
```

Change `PreviewSmoothMotionArtifact::from_scene_plan(...)` signature:

```rust
pub fn from_scene_plan(
    strip_id: &str,
    frame_index: u16,
    elapsed_ms: u64,
    now: time::OffsetDateTime,
    semantic_art_tick_index: u64,
    vm: &WatchViewModel,
    plan: &SmoothCompanionScenePlan,
) -> Self
```

Inside it, set:

```rust
now_unix_ms: i128::from(now.unix_timestamp()) * 1_000
    + i128::from(now.millisecond()),
semantic_art_tick_index,
pet_visual_checksum: crate::presentation::smooth::pet_visual_checksum(
    &vm.pet_art,
    &vm.pet_spans,
),
pet_motion: PreviewSmoothPetMotionArtifact {
    base_anchor: PreviewSmoothPointArtifact::from_point(plan.pet.base_anchor),
    bob_offset: PreviewSmoothPointArtifact::from_point(plan.pet.bob_offset),
    final_anchor: PreviewSmoothPointArtifact::from_point(plan.pet.final_anchor),
    classic_snap_anchor: PreviewSmoothPointArtifact::from_point(plan.pet.classic_snap_anchor),
    scale_x: pet_layer.transform.scale.x,
    scale_y: pet_layer.transform.scale.y,
    opacity: pet_layer.opacity,
    pulse: pulse.to_string(),
},
```

- [ ] **Step 5: Advance deterministic `now` in smooth strips**

In `src/dev_preview/smooth.rs`, replace `MOTION_ELAPSED_MS` with frames that advance both elapsed time and `now`:

```rust
const MOTION_FRAME_DURATION_MS: u64 = 160;
const MOTION_FRAME_COUNT: usize = 12;
```

In `smooth_strips(...)`, build each frame with:

```rust
for index in 0..MOTION_FRAME_COUNT {
    let elapsed_ms = index as u64 * MOTION_FRAME_DURATION_MS;
    let frame_now = ctx.fixed_now + time::Duration::milliseconds(elapsed_ms as i64);
    let semantic_art_tick_index = elapsed_ms / 250;
    let plan = build_round_smooth_scene_plan(
        &vm,
        frame_now,
        GRID_COLS,
        GRID_ROWS,
        &motion,
        elapsed_ms,
    );
    ...
    frame.contract.smooth_motion = Some(PreviewSmoothMotionArtifact::from_scene_plan(
        SMOOTH_MOTION_ID,
        index as u16,
        elapsed_ms,
        frame_now,
        semantic_art_tick_index,
        &vm,
        &plan,
    ));
}
```

Set manifest inputs:

```rust
("frame_duration_ms".to_string(), json!(MOTION_FRAME_DURATION_MS)),
("frame_count".to_string(), json!(MOTION_FRAME_COUNT)),
("now_advances_with_elapsed".to_string(), json!(true)),
```

- [ ] **Step 6: Run Preview Lab focused checks**

Run:

```bash
cargo test presentation::smooth::tests::pet_visual_checksum_tracks_art_and_spans
cargo test --features dev-preview --test dev_preview dev_preview_smooth -- --nocapture
cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview
```

Expected: tests pass and `target/glorp-preview/strips/round-smooth-motion/frame-000.smooth-motion.json` contains the new fields.

- [ ] **Step 7: Commit**

```bash
git add src/presentation/smooth.rs src/dev_preview/contract.rs src/dev_preview/smooth.rs tests/dev_preview.rs
git commit -m "feat(smooth): prove anchor and checksum motion"
```

## Task 4: Native Smooth Cadence and Review Capture

**Files:**
- Create: `src/companion/smooth_timing.rs`
- Modify: `src/companion/mod.rs`
- Modify: `src/companion/app.rs`
- Modify: `src/companion/review_capture.rs`

**Interfaces:**
- Consumes: `SmoothCompanionPet`, `pet_visual_checksum(...)`, and Preview-style anchor fields.
- Produces: Smooth semantic clock and native render-log evidence.

- [ ] **Step 1: Add the smooth semantic clock with failing tests**

Create `src/companion/smooth_timing.rs`:

```rust
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SmoothSemanticClock {
    interval: Duration,
    next_due: Instant,
    tick_index: u64,
}

impl SmoothSemanticClock {
    pub fn new(started_at: Instant, interval: Duration) -> Self {
        Self {
            interval,
            next_due: started_at + interval,
            tick_index: 0,
        }
    }

    pub fn consume_due_tick(&mut self, now: Instant) -> Option<u64> {
        if now < self.next_due {
            return None;
        }
        self.tick_index = self.tick_index.saturating_add(1);
        self.next_due = now + self.interval;
        Some(self.tick_index)
    }

    pub fn tick_index(&self) -> u64 {
        self.tick_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_semantic_clock_waits_until_interval_elapses() {
        let start = Instant::now();
        let mut clock = SmoothSemanticClock::new(start, Duration::from_millis(250));

        assert_eq!(clock.consume_due_tick(start + Duration::from_millis(249)), None);
        assert_eq!(clock.consume_due_tick(start + Duration::from_millis(250)), Some(1));
        assert_eq!(clock.tick_index(), 1);
    }

    #[test]
    fn smooth_semantic_clock_drops_missed_intervals_instead_of_catching_up() {
        let start = Instant::now();
        let mut clock = SmoothSemanticClock::new(start, Duration::from_millis(250));

        assert_eq!(clock.consume_due_tick(start + Duration::from_secs(3)), Some(1));
        assert_eq!(clock.consume_due_tick(start + Duration::from_secs(3) + Duration::from_millis(1)), None);
        assert_eq!(clock.tick_index(), 1);
    }
}
```

Export it in `src/companion/mod.rs`:

```rust
pub mod smooth_timing;
```

Run:

```bash
cargo test companion::smooth_timing
```

Expected: passes after the file is exported.

- [ ] **Step 2: Add review capture schema tests**

In `src/companion/review_capture.rs`, add tests:

```rust
#[test]
fn smooth_review_capture_records_semantic_ticks_anchors_and_privacy() {
    let dir = tempfile::tempdir().unwrap();
    let mut capture = ReviewCapture::from_options(
        CompanionRendererMode::Smooth,
        &CompanionReviewOptions {
            duration_ms: Some(2000),
            capture_dir: Some(dir.path().join("capture")),
            ..CompanionReviewOptions::default()
        },
    )
    .unwrap()
    .expect("capture dir should create review capture session");

    capture.record_frame(Some(SmoothReviewFrameSample {
        bob_y: 0.1,
        semantic_art_tick_index: 0,
        pet_visual_checksum: 123,
        base_anchor: SmoothReviewPoint { x: 10.25, y: 12.5 },
        bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
        final_anchor: SmoothReviewPoint { x: 10.25, y: 12.6 },
        classic_snap_anchor: SmoothReviewPoint { x: 10.0, y: 12.0 },
    }));
    capture.record_frame(Some(SmoothReviewFrameSample {
        bob_y: 0.2,
        semantic_art_tick_index: 0,
        pet_visual_checksum: 123,
        base_anchor: SmoothReviewPoint { x: 10.30, y: 12.55 },
        bob_offset: SmoothReviewPoint { x: 0.0, y: 0.2 },
        final_anchor: SmoothReviewPoint { x: 10.30, y: 12.75 },
        classic_snap_anchor: SmoothReviewPoint { x: 10.0, y: 12.0 },
    }));
    let json = capture.render_log_json_for_test().unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(value["frame_count"], 2);
    assert_eq!(value["semantic_art_tick_count"], 1);
    assert_eq!(value["smooth_frame_samples"].as_array().unwrap().len(), 2);
    assert_eq!(value["smooth_frame_samples"][0]["pet_visual_checksum"], 123);
    assert_eq!(value["privacy"]["source_names_visible"], false);
    assert_eq!(value["privacy"]["exact_token_strings_visible"], false);
}

#[test]
fn smooth_review_capture_checksum_stability_detects_flashing() {
    let mut capture = ReviewCapture::from_options(
        CompanionRendererMode::Smooth,
        &CompanionReviewOptions {
            duration_ms: Some(2000),
            ..CompanionReviewOptions::default()
        },
    )
    .unwrap()
    .expect("duration should create review capture session");

    capture.record_frame(Some(SmoothReviewFrameSample {
        bob_y: 0.1,
        semantic_art_tick_index: 0,
        pet_visual_checksum: 123,
        base_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
        bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
        final_anchor: SmoothReviewPoint { x: 1.0, y: 1.1 },
        classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
    }));
    capture.record_frame(Some(SmoothReviewFrameSample {
        bob_y: 0.2,
        semantic_art_tick_index: 0,
        pet_visual_checksum: 456,
        base_anchor: SmoothReviewPoint { x: 1.1, y: 1.0 },
        bob_offset: SmoothReviewPoint { x: 0.0, y: 0.2 },
        final_anchor: SmoothReviewPoint { x: 1.1, y: 1.2 },
        classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
    }));

    assert!(!capture.pet_checksums_stable_within_semantic_ticks_for_test());
}
```

Run:

```bash
cargo test companion::review_capture::tests::smooth_review_capture -- --nocapture
```

Expected: compile fails until `SmoothReviewFrameSample`, `render_log_json_for_test`, privacy fields, and stability helper exist.

- [ ] **Step 3: Extend review capture data**

In `src/companion/review_capture.rs`, add `SmoothReviewFrameSample`, `SmoothReviewPoint`, and privacy log structs. Replace `smooth_bob_samples: Vec<f32>` with `smooth_frame_samples: Vec<SmoothReviewFrameSample>`.

Update `record_frame`:

```rust
pub fn record_frame(&mut self, smooth_sample: Option<SmoothReviewFrameSample>) {
    self.frame_count = self.frame_count.saturating_add(1);
    if let Some(sample) = smooth_sample {
        if self.smooth_frame_samples.len() < MAX_SMOOTH_FRAME_SAMPLES {
            self.smooth_frame_samples.push(round_smooth_sample(sample));
        }
    }
}
```

Add helpers:

```rust
pub fn render_log_json_for_test(&self) -> Result<String> {
    self.render_log_json()
}

fn render_log_json(&self) -> Result<String> {
    serde_json::to_string_pretty(&self.render_log()).map_err(Into::into)
}

fn semantic_art_tick_count(&self) -> u64 {
    self.smooth_frame_samples
        .iter()
        .map(|sample| sample.semantic_art_tick_index)
        .collect::<std::collections::BTreeSet<_>>()
        .len() as u64
}

fn pet_checksums_stable_within_semantic_ticks(&self) -> bool {
    let mut by_tick = std::collections::BTreeMap::<u64, u64>::new();
    for sample in &self.smooth_frame_samples {
        if let Some(existing) = by_tick.insert(sample.semantic_art_tick_index, sample.pet_visual_checksum) {
            if existing != sample.pet_visual_checksum {
                return false;
            }
        }
    }
    true
}
```

Use `SmoothCompanionPrivacyClaims::external_companion()` in the log privacy field.

- [ ] **Step 4: Wire two-clock Smooth behavior in `src/companion/app.rs`**

Add AppState fields:

```rust
smooth_semantic_clock: Option<crate::companion::smooth_timing::SmoothSemanticClock>,
smooth_semantic_art_tick_index: u64,
```

Initialize:

```rust
let smooth_started_at = renderer_mode.is_smooth().then(Instant::now);
let smooth_semantic_clock = smooth_started_at.map(|started_at| {
    crate::companion::smooth_timing::SmoothSemanticClock::new(
        started_at,
        Duration::from_secs_f64(UI_TICK_INTERVAL_SECS),
    )
});
```

Replace the non-pixel part of `animate_pet()` with renderer-specific logic:

```rust
let now = time::OffsetDateTime::now_utc();
if state.renderer_mode.is_smooth() {
    let due_tick = state
        .smooth_semantic_clock
        .as_mut()
        .and_then(|clock| clock.consume_due_tick(Instant::now()));
    if let Some(tick_index) = due_tick {
        let _ = advance_companion_animation(&mut state.vm, tick_index, now);
        state.animation_frame = tick_index;
        state.smooth_semantic_art_tick_index = tick_index;
        state.scene = derive_round_scene_model(&state.vm, now);
    }
    return Some(state.view.clone());
}

let next_frame = state.animation_frame.wrapping_add(1);
let _ = advance_companion_animation(&mut state.vm, next_frame, now);
state.animation_frame = next_frame;
state.scene = derive_round_scene_model(&state.vm, now);
Some(state.view.clone())
```

Keep Pixel behavior unchanged.

- [ ] **Step 5: Pass Smooth samples from AppKit draw to review capture**

Extend the `draw_scene(...)` snapshot tuple to include `smooth_semantic_art_tick_index`.

After building `plan` in Smooth mode, build a review sample:

```rust
let smooth_sample = SmoothReviewFrameSample {
    bob_y: plan.pet.bob_offset.y,
    semantic_art_tick_index,
    pet_visual_checksum: crate::presentation::smooth::pet_visual_checksum(
        &vm.pet_art,
        &vm.pet_spans,
    ),
    base_anchor: SmoothReviewPoint::from_smooth_point(plan.pet.base_anchor),
    bob_offset: SmoothReviewPoint::from_smooth_point(plan.pet.bob_offset),
    final_anchor: SmoothReviewPoint::from_smooth_point(plan.pet.final_anchor),
    classic_snap_anchor: SmoothReviewPoint::from_smooth_point(plan.pet.classic_snap_anchor),
};
```

Change `record_review_frame(view, smooth_bob_sample)` to `record_review_frame(view, smooth_sample)`.

Use the fractional pet center for aura:

```rust
let pet_center_col = f64::from(
    plan.pet.fractional_bounds.min.x
        + (plan.pet.fractional_bounds.max.x - plan.pet.fractional_bounds.min.x) / 2.0,
);
let pet_center_row = f64::from(
    plan.pet.fractional_bounds.min.y
        + (plan.pet.fractional_bounds.max.y - plan.pet.fractional_bounds.min.y) / 2.0,
);
```

- [ ] **Step 6: Draw pet-attached Smooth roles fractionally**

In `appkit_blit_smooth_plan(...)`, replace the role check:

```rust
let fractional = matches!(
    layer.role,
    SmoothLayerRole::PetBody | SmoothLayerRole::ContactShadow | SmoothLayerRole::PerformanceCue
);
let (px, py) = if fractional {
    fractional_cell_to_point(
        f64::from(col),
        f64::from(row),
        cell_w,
        cell_h,
        origin_x,
        origin_y,
    )
} else {
    cell_to_point(
        appkit_cell_axis(col),
        appkit_cell_axis(row),
        cell_w,
        cell_h,
        origin_x,
        origin_y,
    )
};
```

- [ ] **Step 7: Run focused native tests**

Run:

```bash
cargo test companion::smooth_timing
cargo test companion::review_capture::tests::smooth_review_capture -- --nocapture
cargo test --test cli_smoke companion_ -- --nocapture
cargo test --test smooth_companion
```

Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/companion/mod.rs src/companion/smooth_timing.rs src/companion/app.rs src/companion/review_capture.rs
git commit -m "fix(companion): stabilize smooth animation cadence"
```

## Task 5: End-to-End Verification and Native Review

**Files:**
- No source files expected.
- Generated artifacts under `target/glorp-preview` and `target/glorp-review` are verification outputs and must not be committed.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: final proof that Smooth mode is stable enough for Drew to spot-check.

- [ ] **Step 1: Run formatting and focused tests**

Run:

```bash
cargo fmt --check
cargo test --test smooth_companion
cargo test --test round_scene
cargo test --test round_draw_list
cargo test --test cli_smoke companion_ -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_smooth -- --nocapture
```

Expected: every command exits 0.

- [ ] **Step 2: Generate Preview Lab smooth bundle**

Run:

```bash
cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview
```

Expected: exits 0 and writes:

```text
target/glorp-preview/frames/round-smooth-classic-parity.smooth-plan.json
target/glorp-preview/frames/round-smooth-classic-parity.smooth-parity.json
target/glorp-preview/strips/round-smooth-motion/frame-000.smooth-motion.json
```

Inspect motion evidence:

```bash
jq '.pet_motion, .semantic_art_tick_index, .pet_visual_checksum, .privacy' \
  target/glorp-preview/strips/round-smooth-motion/frame-000.smooth-motion.json
```

Expected: output includes `base_anchor`, `bob_offset`, `final_anchor`, `classic_snap_anchor`, numeric checksum, numeric semantic tick index, and all privacy booleans set to `false`.

- [ ] **Step 3: Run native Smooth review capture**

Run:

```bash
rm -rf target/glorp-review/smooth-stabilized-active
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 12000 --review-capture-dir target/glorp-review/smooth-stabilized-active
```

Expected: exits 0 and writes `screenshot.png` plus `render-log.json`.

Inspect log:

```bash
jq '{frame_count, semantic_art_tick_count, first_sample: .smooth_frame_samples[0], privacy}' \
  target/glorp-review/smooth-stabilized-active/render-log.json
```

Expected:

- `frame_count` is greater than `semantic_art_tick_count`.
- `smooth_frame_samples[0].base_anchor`, `bob_offset`, `final_anchor`, and `classic_snap_anchor` exist.
- `smooth_frame_samples[0].pet_visual_checksum` is numeric.
- all privacy booleans are `false`.

- [ ] **Step 4: Scan artifacts for private strings and placeholders**

Run:

```bash
rg -n "T[B]D|T[O]DO|implement [l]ater|fill [i]n|approp[r]iate|handle edge [c]ases|Similar [t]o" docs/superpowers/plans/2026-07-09-glorp-smooth-motion-stabilization-implementation.md docs/superpowers/specs/2026-07-09-glorp-smooth-motion-stabilization-design.md
rg -n "Users/|/var/folders|prompt|response|diagnostic|raw_source|project" target/glorp-preview/frames target/glorp-preview/strips target/glorp-review/smooth-stabilized-active/render-log.json
```

Expected: first command exits 1 with no output; second command exits 1 with no output.

- [ ] **Step 5: Build and run the fresh companion for Drew**

Run:

```bash
cargo build
node scripts/build-macos-companion-app.mjs --profile debug
pkill -f "glorp-companion companion-app" || true
target/debug/glorp companion --renderer smooth --review-size 360x360 --review-state active-pulse
```

Expected: the Smooth companion opens. Visual spot-check should show the current Classic Glorp art/tank composition, with smooth bob and tank movement, without rapid flashing or cell-jump motion.

- [ ] **Step 6: Commit verification note if any source/doc cleanup was needed**

If Step 4 required doc/source cleanup, commit the cleanup:

```bash
git add docs/superpowers/plans/2026-07-09-glorp-smooth-motion-stabilization-implementation.md docs/superpowers/specs/2026-07-09-glorp-smooth-motion-stabilization-design.md
git commit -m "docs(smooth): verify motion stabilization plan"
```

If no tracked files changed, do not create an empty commit.

## Final Acceptance Checklist

- [ ] Smooth mode advances Classic pet art at 250 ms semantic cadence, not 30 FPS.
- [ ] Smooth mode drops missed semantic art intervals instead of catching up after stalls.
- [ ] `draw_scene(...)` is render-only.
- [ ] Shared placement reproduces Classic snapped rect exactly.
- [ ] Smooth plan records fractional base, bob, final, and Classic snap anchors.
- [ ] Classic flatten parity remains exact.
- [ ] `ChestBubble` remains snapped with props.
- [ ] `MoodAura` uses fractional pet center or a real smooth shape layer.
- [ ] Preview Lab artifacts prove checksum stability within semantic ticks.
- [ ] Preview Lab artifacts advance deterministic `now` and prove tank anchor motion independent of bob.
- [ ] Native review log proves paint frames outnumber semantic art ticks.
- [ ] Privacy scans cover smooth preview sidecars and native render log.
- [ ] Drew can run the fresh Smooth companion and visually confirm no rapid flashing or cell-jump motion.
