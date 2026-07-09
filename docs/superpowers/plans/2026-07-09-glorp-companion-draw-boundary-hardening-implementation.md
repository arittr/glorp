# Glorp Companion Draw Boundary Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop Rust panics from aborting the macOS companion through Objective-C callbacks, and move frame construction out of `drawRect` into a last-good-frame preparation path.

**Architecture:** `ui_tick` owns state advancement and frame preparation. `drawRect` becomes a guarded AppKit painter that reads `AppState::last_good_frame`. Smooth planner invariant failures become recoverable preparation errors, and review capture records boundary health without leaking user data.

**Tech Stack:** Rust, objc2/AppKit, existing Glorp companion renderer, existing `ReviewCapture` JSON artifacts, Cargo tests.

## Global Constraints

- Do not introduce a new animation engine, windowing framework, or bitmap-only renderer in this slice.
- Do not change the default renderer.
- Preserve Classic, Smooth, and Pixel visual output except for eliminating crash/fallback behavior.
- Do not let Rust unwind across `Controller::uiTick:` or `RoundView::drawRect:`.
- Keep review-capture error strings static or sanitized; never log source names, file paths, prompts, responses, raw diagnostics, exact token strings, project names, or pet seeds in `render-log.json`.
- Do not use `git add -A`; stage exact files only after inspecting `git status`.

---

## File Structure

- Modify `src/companion/review_capture.rs`
  - Add privacy-safe boundary health counters and render-log fields.
  - Add test helpers and JSON privacy coverage for the new strings.
- Modify `src/round/smooth.rs`
  - Add a fallible smooth scene planner used by production frame preparation.
  - Keep the existing infallible wrapper only for current tests and compatibility.
- Modify `tests/smooth_companion.rs`
  - Exercise the fallible planner alongside existing smooth parity tests.
- Modify `src/companion/app.rs`
  - Add guarded Objective-C callback entry points.
  - Add frame-preparation data structures and last-good-frame state.
  - Split `draw_scene` into `prepare_companion_frame` and `paint_prepared_frame`.
  - Cache grid metrics by normalized view size.
- Keep this first implementation in `src/companion/app.rs`; defer file splits until after the crash fix is verified.

## Task 1: Review Capture Boundary Health

**Files:**
- Modify: `src/companion/review_capture.rs`

**Interfaces:**
- Produces:
  - `ReviewCapture::record_callback_panic(&mut self, label: &'static str)`
  - `ReviewCapture::record_frame_preparation_error(&mut self, category: &'static str)`
  - `ReviewCapture::record_last_good_frame_reused(&mut self)`
  - New render-log fields:
    - `callback_panic_count: u64`
    - `last_callback_panic_label: Option<&'static str>`
    - `frame_preparation_error_count: u64`
    - `last_frame_preparation_error: Option<&'static str>`
    - `last_good_frame_reused_count: u64`
- Consumes:
  - Existing `ReviewCapture::render_log_json_for_test()`
  - Existing sanitized string test in `review_capture.rs`

- [ ] **Step 1: Write the failing telemetry test**

Add this test near `smooth_review_capture_records_requested_evidence_and_privacy`:

```rust
#[test]
fn review_capture_records_boundary_health_without_private_strings() {
    let mut capture = ReviewCapture::from_options(
        CompanionRendererMode::Smooth,
        &CompanionReviewOptions {
            duration_ms: Some(2000),
            state: Some(CompanionReviewState::Normal),
            ..CompanionReviewOptions::default()
        },
    )
    .unwrap()
    .expect("duration should create review capture session");

    capture.record_callback_panic("drawRect");
    capture.record_callback_panic("uiTick");
    capture.record_frame_preparation_error("smooth-missing-pet-body");
    capture.record_last_good_frame_reused();
    capture.record_last_good_frame_reused();

    let json = capture.render_log_json_for_test().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    assert_eq!(value["callback_panic_count"], 2);
    assert_eq!(value["last_callback_panic_label"], "uiTick");
    assert_eq!(value["frame_preparation_error_count"], 1);
    assert_eq!(value["last_frame_preparation_error"], "smooth-missing-pet-body");
    assert_eq!(value["last_good_frame_reused_count"], 2);
    assert_render_log_json_values_are_sanitized(&value, "$");
}
```

Extend `RENDER_LOG_ALLOWED_STRING_VALUES` with the exact allowed static strings:

```rust
"drawRect",
"uiTick",
"smooth-missing-pet-body",
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test --lib companion::review_capture::tests::review_capture_records_boundary_health_without_private_strings -- --nocapture
```

Expected: fail to compile because the three `ReviewCapture::record_*` methods and render-log fields do not exist.

- [ ] **Step 3: Implement boundary health fields and methods**

Add fields to `ReviewCapture`:

```rust
callback_panic_count: u64,
last_callback_panic_label: Option<&'static str>,
frame_preparation_error_count: u64,
last_frame_preparation_error: Option<&'static str>,
last_good_frame_reused_count: u64,
```

Initialize them in `ReviewCapture::from_options(...)`:

```rust
callback_panic_count: 0,
last_callback_panic_label: None,
frame_preparation_error_count: 0,
last_frame_preparation_error: None,
last_good_frame_reused_count: 0,
```

Add public methods in the `impl ReviewCapture` block:

```rust
pub fn record_callback_panic(&mut self, label: &'static str) {
    self.panic = true;
    self.callback_panic_count = self.callback_panic_count.saturating_add(1);
    self.last_callback_panic_label = Some(label);
}

pub fn record_frame_preparation_error(&mut self, category: &'static str) {
    self.frame_preparation_error_count = self.frame_preparation_error_count.saturating_add(1);
    self.last_frame_preparation_error = Some(category);
}

pub fn record_last_good_frame_reused(&mut self) {
    self.last_good_frame_reused_count = self.last_good_frame_reused_count.saturating_add(1);
}
```

Add fields to `RenderLog<'a>`:

```rust
callback_panic_count: u64,
last_callback_panic_label: Option<&'static str>,
frame_preparation_error_count: u64,
last_frame_preparation_error: Option<&'static str>,
last_good_frame_reused_count: u64,
```

Populate them in `ReviewCapture::render_log(...)`:

```rust
callback_panic_count: self.callback_panic_count,
last_callback_panic_label: self.last_callback_panic_label,
frame_preparation_error_count: self.frame_preparation_error_count,
last_frame_preparation_error: self.last_frame_preparation_error,
last_good_frame_reused_count: self.last_good_frame_reused_count,
```

- [ ] **Step 4: Run review-capture tests**

Run:

```bash
cargo test --lib companion::review_capture::tests -- --nocapture
```

Expected: all `companion::review_capture::tests::*` pass.

- [ ] **Step 5: Commit**

```bash
git status --short
git add src/companion/review_capture.rs
git commit -m "feat(companion): record boundary health in review capture"
```

## Task 2: Fallible Smooth Scene Planning

**Files:**
- Modify: `src/round/smooth.rs`
- Modify: `tests/smooth_companion.rs`

**Interfaces:**
- Produces:
  - `pub fn try_build_round_smooth_scene_plan(...) -> Result<SmoothCompanionScenePlan>`
  - `pub enum SmoothScenePlanError`
- Consumes:
  - Existing `build_round_smooth_scene_plan(...)` callers
  - `crate::error::{GlorpError, Result}` for app-level conversion

- [ ] **Step 1: Write the fallible planner test**

Add this test to `tests/smooth_companion.rs`:

```rust
#[test]
fn fallible_smooth_round_plan_matches_existing_infallible_plan() {
    let vm = parity_fixture();
    let motion = CompanionMotion::default();

    let infallible = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, NOW, GRID_COLS, GRID_ROWS, &motion, 250,
    );
    let fallible = glorp::round::smooth::try_build_round_smooth_scene_plan(
        &vm, NOW, GRID_COLS, GRID_ROWS, &motion, 250,
    )
    .expect("fixture should include pet body layer");

    assert_eq!(fallible, infallible);
}
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test --test smooth_companion fallible_smooth_round_plan_matches_existing_infallible_plan -- --nocapture
```

Expected: fail to compile because `try_build_round_smooth_scene_plan` does not exist.

- [ ] **Step 3: Add the fallible planner**

In `src/round/smooth.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothScenePlanError {
    MissingPetBody,
}

impl std::fmt::Display for SmoothScenePlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmoothScenePlanError::MissingPetBody => f.write_str("smooth scene missing pet body"),
        }
    }
}

impl std::error::Error for SmoothScenePlanError {}
```

Rename the current body of `build_round_smooth_scene_plan(...)` to:

```rust
pub fn try_build_round_smooth_scene_plan(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    elapsed_ms: u64,
) -> std::result::Result<SmoothCompanionScenePlan, SmoothScenePlanError> {
    // existing body, changed to return Ok(plan)
}
```

Replace the first pet-body lookup with:

```rust
let pet_body_classic_anchor = layered
    .layers
    .iter()
    .find(|layer| layer.role == SmoothLayerRole::PetBody)
    .map(|layer| layer.anchor)
    .ok_or(SmoothScenePlanError::MissingPetBody)?;
```

Replace the second pet-body lookup with:

```rust
let pet_body = layers
    .iter()
    .find(|layer| layer.role == SmoothLayerRole::PetBody)
    .ok_or(SmoothScenePlanError::MissingPetBody)?;
```

Return the final plan with `Ok(SmoothCompanionScenePlan { ... })`.

Re-add the compatibility wrapper:

```rust
pub fn build_round_smooth_scene_plan(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    elapsed_ms: u64,
) -> SmoothCompanionScenePlan {
    try_build_round_smooth_scene_plan(vm, now, grid_cols, grid_rows, motion, elapsed_ms)
        .expect("round smooth scene should include a pet body layer")
}
```

- [ ] **Step 4: Run smooth companion tests**

Run:

```bash
cargo test --test smooth_companion -- --nocapture
```

Expected: all tests in `tests/smooth_companion.rs` pass.

- [ ] **Step 5: Commit**

```bash
git status --short
git add src/round/smooth.rs tests/smooth_companion.rs
git commit -m "fix(smooth): expose fallible round scene planning"
```

## Task 3: Prepared Frame Types and Bounds Validation

**Files:**
- Modify: `src/companion/app.rs`

**Interfaces:**
- Produces:
  - `PreparedCompanionFrame`
  - `PreparedRendererFrame`
  - `PreparedGaugeFrame`
  - `PreparedBounds`
  - `CompanionMetricCache`
  - `CompanionFramePreparationError`
  - `prepare_bounds(bounds: NSRect) -> std::result::Result<PreparedBounds, CompanionFramePreparationError>`
- Consumes:
  - Existing `CompanionGridMetrics`
  - Existing `companion_grid_metrics(...)`

- [ ] **Step 1: Write bounds-validation tests**

Add tests to `src/companion/app.rs` inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn prepared_bounds_rejects_zero_negative_non_finite_and_oversized_values() {
    for size in [
        NSSize::new(0.0, 360.0),
        NSSize::new(360.0, 0.0),
        NSSize::new(-1.0, 360.0),
        NSSize::new(360.0, -1.0),
        NSSize::new(f64::NAN, 360.0),
        NSSize::new(360.0, f64::INFINITY),
        NSSize::new(f64::from(u16::MAX) + 1.0, 360.0),
        NSSize::new(360.0, f64::from(u16::MAX) + 1.0),
    ] {
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), size);
        assert_eq!(
            prepare_bounds(bounds).unwrap_err(),
            CompanionFramePreparationError::InvalidBounds
        );
    }
}

#[test]
fn prepared_bounds_accepts_normal_companion_size() {
    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 360.0));
    let prepared = prepare_bounds(bounds).unwrap();

    assert_eq!(prepared.width_px, 360);
    assert_eq!(prepared.height_px, 360);
    assert_eq!(prepared.width_f64, 360.0);
    assert_eq!(prepared.height_f64, 360.0);
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
cargo test --lib companion::app::tests::prepared_bounds_ -- --nocapture
```

Expected: fail to compile because `prepare_bounds` and related types do not exist.

- [ ] **Step 3: Add prepared-frame support types**

Add near `AppState`:

```rust
#[derive(Debug, Clone)]
struct PreparedCompanionFrame {
    bounds: PreparedBounds,
    aperture: RoundAperture,
    background: RoundColor,
    dim_overlay: bool,
    renderer: PreparedRendererFrame,
    gauges: PreparedGaugeFrame,
    hud: CompanionHudText,
    hud_font_size: f64,
    overlay_commands: Vec<crate::companion::render::RoundDrawCommand>,
    review_sample: Option<crate::companion::review_capture::SmoothReviewFrameSample>,
}

#[derive(Debug, Clone)]
enum PreparedRendererFrame {
    Pixel { frame: PixelFrame },
    Classic {
        metrics: CompanionGridMetrics,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        draw_list: crate::presentation::SceneDrawList,
    },
    Smooth {
        metrics: CompanionGridMetrics,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        plan: SmoothCompanionScenePlan,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedGaugeFrame {
    xp_fraction: f64,
    daily_fraction: f64,
    daily_overage_fraction: f64,
    pace_fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedBounds {
    width_px: u16,
    height_px: u16,
    width_f64: f64,
    height_f64: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompanionMetricKey {
    width_px: u16,
    height_px: u16,
}

#[derive(Debug, Clone, Copy, Default)]
struct CompanionMetricCache {
    last: Option<(CompanionMetricKey, CompanionGridMetrics)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompanionFramePreparationError {
    InvalidBounds,
    MissingGridMetrics,
    SmoothMissingPetBody,
}

impl CompanionFramePreparationError {
    fn category(self) -> &'static str {
        match self {
            CompanionFramePreparationError::InvalidBounds => "invalid-bounds",
            CompanionFramePreparationError::MissingGridMetrics => "missing-grid-metrics",
            CompanionFramePreparationError::SmoothMissingPetBody => "smooth-missing-pet-body",
        }
    }
}
```

Add `RoundDrawCommand` to the existing companion render import:

```rust
use crate::companion::render::{build_draw_commands, RoundColor, RoundDrawCommand, RoundDrawKind};
```

- [ ] **Step 4: Add bounds and metric-cache helpers**

Add:

```rust
fn prepare_bounds(bounds: NSRect) -> std::result::Result<PreparedBounds, CompanionFramePreparationError> {
    let width = bounds.size.width;
    let height = bounds.size.height;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > f64::from(u16::MAX)
        || height > f64::from(u16::MAX)
    {
        return Err(CompanionFramePreparationError::InvalidBounds);
    }

    Ok(PreparedBounds {
        width_px: width.round() as u16,
        height_px: height.round() as u16,
        width_f64: width,
        height_f64: height,
    })
}

impl CompanionMetricCache {
    fn metrics_for(
        &mut self,
        bounds: PreparedBounds,
    ) -> std::result::Result<CompanionGridMetrics, CompanionFramePreparationError> {
        let key = CompanionMetricKey {
            width_px: bounds.width_px,
            height_px: bounds.height_px,
        };
        if let Some((cached_key, metrics)) = self.last {
            if cached_key == key {
                return Ok(metrics);
            }
        }
        let metrics = companion_grid_metrics(bounds.width_f64, bounds.height_f64)
            .ok_or(CompanionFramePreparationError::MissingGridMetrics)?;
        self.last = Some((key, metrics));
        Ok(metrics)
    }
}
```

- [ ] **Step 5: Run bounds tests**

Run:

```bash
cargo test --lib companion::app::tests::prepared_bounds_ -- --nocapture
```

Expected: the new bounds tests pass.

- [ ] **Step 6: Commit**

```bash
git status --short
git add src/companion/app.rs
git commit -m "feat(companion): define prepared frame primitives"
```

## Task 4: Prepare Frames During UI Tick

**Files:**
- Modify: `src/companion/app.rs`

**Interfaces:**
- Consumes:
  - `try_build_round_smooth_scene_plan(...)` from Task 2
  - `PreparedCompanionFrame` types from Task 3
  - Review-capture methods from Task 1
- Produces:
  - `AppState::last_good_frame`
  - `AppState::metric_cache`
  - `AppState::last_frame_preparation_error`
  - `prepare_companion_frame(...)`
  - `prepare_current_frame_from_state(...)`
  - `record_frame_preparation_error(...)`

- [ ] **Step 1: Write focused frame-preparation tests**

Add tests to `src/companion/app.rs`:

```rust
#[test]
fn prepared_gauge_frame_matches_current_vm_values() {
    let mut vm = WatchViewModel::fixture();
    vm.progress.is_max_stage = false;
    vm.progress.fraction = 0.42;
    vm.daily_comparison.fraction_of_yesterday = Some(1.25);
    vm.rate_momentum.pulse.current_tokens = 31_000_000.0;

    let gauges = prepare_gauge_frame(&vm);

    assert_eq!(gauges.xp_fraction, 0.42);
    assert_eq!(gauges.daily_fraction, daily_fraction_for_gauge(Some(1.25)));
    assert_eq!(
        gauges.daily_overage_fraction,
        daily_overage_marker_fraction(Some(1.25))
    );
    assert_eq!(gauges.pace_fraction, companion_pace_fraction(31_000_000.0));
}

#[test]
fn prepared_hud_text_uses_review_redaction_when_requested() {
    let mut vm = WatchViewModel::fixture();
    vm.today_effective_tokens = 842_000_000.0;
    vm.daily_comparison.fraction_of_yesterday = Some(0.94);
    vm.rate_momentum.pulse.current_tokens = 31_000_000.0;

    assert_eq!(prepare_hud_frame(&vm, true), review_capture_hud_text());
    assert_eq!(prepare_hud_frame(&vm, false), live_hud_text(&vm));
}
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run:

```bash
cargo test --lib companion::app::tests::prepared_gauge_frame_matches_current_vm_values companion::app::tests::prepared_hud_text_uses_review_redaction_when_requested -- --nocapture
```

Expected: fail to compile because `prepare_gauge_frame` and `prepare_hud_frame` do not exist.

- [ ] **Step 3: Add AppState fields**

Extend `AppState`:

```rust
metric_cache: CompanionMetricCache,
last_good_frame: Option<PreparedCompanionFrame>,
last_frame_preparation_error: Option<CompanionFramePreparationError>,
callback_panic_count: u64,
last_callback_panic_label: Option<&'static str>,
```

Initialize them in `run(...)`:

```rust
metric_cache: CompanionMetricCache::default(),
last_good_frame: None,
last_frame_preparation_error: None,
callback_panic_count: 0,
last_callback_panic_label: None,
```

- [ ] **Step 4: Add frame component preparation helpers**

Add:

```rust
fn prepare_gauge_frame(vm: &WatchViewModel) -> PreparedGaugeFrame {
    PreparedGaugeFrame {
        xp_fraction: if vm.progress.is_max_stage { 1.0 } else { vm.progress.fraction as f64 },
        daily_fraction: daily_fraction_for_gauge(vm.daily_comparison.fraction_of_yesterday),
        daily_overage_fraction: daily_overage_marker_fraction(
            vm.daily_comparison.fraction_of_yesterday,
        ),
        pace_fraction: companion_pace_fraction(vm.rate_momentum.pulse.current_tokens),
    }
}

fn prepare_hud_frame(vm: &WatchViewModel, redacts_live_hud: bool) -> CompanionHudText {
    if redacts_live_hud {
        review_capture_hud_text()
    } else {
        live_hud_text(vm)
    }
}
```

- [ ] **Step 5: Add full frame preparation**

Add:

```rust
fn prepare_companion_frame(
    vm: &WatchViewModel,
    scene: &RoundSceneModel,
    renderer_mode: CompanionRendererMode,
    pixel_frame: Option<&PixelFrame>,
    smooth_started_at: Option<Instant>,
    smooth_semantic_art_tick_index: u64,
    redacts_live_hud: bool,
    bounds: NSRect,
    metric_cache: &mut CompanionMetricCache,
) -> std::result::Result<PreparedCompanionFrame, CompanionFramePreparationError> {
    let prepared_bounds = prepare_bounds(bounds)?;
    let aperture = RoundAperture::new(prepared_bounds.width_px, prepared_bounds.height_px);
    let layout = layout_round_scene(
        scene,
        aperture,
        RoundRenderCapabilities::preview_truecolor(),
    );
    let overlay_commands = build_draw_commands(scene, &layout);
    let background = overlay_commands
        .iter()
        .find(|command| command.kind == RoundDrawKind::Background)
        .map(|command| command.color)
        .unwrap_or(RoundColor(0.05, 0.06, 0.10, 1.0));
    let dim_overlay = scene.lifecycle.asleep || scene.lifecycle.calm;
    let gauges = prepare_gauge_frame(vm);
    let hud = prepare_hud_frame(vm, redacts_live_hud);

    let renderer = if renderer_mode.is_pixel() {
        PreparedRendererFrame::Pixel {
            frame: pixel_frame
                .cloned()
                .unwrap_or_else(|| PixelFrame::transparent(PixelViewport::companion_default())),
        }
    } else {
        let metrics = metric_cache.metrics_for(prepared_bounds)?;
        if renderer_mode.is_smooth() {
            let elapsed_ms = smooth_started_at
                .map(|started_at| started_at.elapsed().as_millis())
                .unwrap_or(0)
                .min(u128::from(u64::MAX)) as u64;
            let plan = crate::round::smooth::try_build_round_smooth_scene_plan(
                vm,
                time::OffsetDateTime::now_utc(),
                metrics.grid_cols,
                metrics.grid_rows,
                &companion_motion(),
                elapsed_ms,
            )
            .map_err(|_| CompanionFramePreparationError::SmoothMissingPetBody)?;
            let pet_center_col = f64::from(
                plan.pet.fractional_bounds.min.x
                    + (plan.pet.fractional_bounds.max.x - plan.pet.fractional_bounds.min.x)
                        / 2.0,
            );
            let pet_center_row = f64::from(
                plan.pet.fractional_bounds.min.y
                    + (plan.pet.fractional_bounds.max.y - plan.pet.fractional_bounds.min.y)
                        / 2.0,
            );
            let pet_width_cells =
                f64::from(plan.pet.fractional_bounds.max.x - plan.pet.fractional_bounds.min.x);
            PreparedRendererFrame::Smooth {
                metrics,
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                plan,
            }
        } else {
            let companion_scene = crate::round::scene::build_round_scene_draw_list(
                vm,
                time::OffsetDateTime::now_utc(),
                metrics.grid_cols,
                metrics.grid_rows,
                &companion_motion(),
            );
            PreparedRendererFrame::Classic {
                metrics,
                pet_center_col: f64::from(
                    companion_scene.pet_rect.x + companion_scene.pet_rect.width / 2,
                ),
                pet_center_row: f64::from(
                    companion_scene.pet_rect.y + companion_scene.pet_rect.height / 2,
                ),
                pet_width_cells: f64::from(companion_scene.pet_rect.width),
                draw_list: companion_scene.draw_list,
            }
        }
    };

    let review_sample = match &renderer {
        PreparedRendererFrame::Smooth { plan, .. } => Some(
            crate::companion::review_capture::SmoothReviewFrameSample {
                bob_y: plan.pet.bob_offset.y,
                semantic_art_tick_index: smooth_semantic_art_tick_index,
                pet_visual_checksum: crate::presentation::smooth::pet_visual_checksum(
                    &vm.pet_art,
                    &vm.pet_spans,
                ),
                base_anchor:
                    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                        plan.pet.base_anchor,
                    ),
                bob_offset:
                    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                        plan.pet.bob_offset,
                    ),
                final_anchor:
                    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                        plan.pet.final_anchor,
                    ),
                classic_snap_anchor:
                    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                        plan.pet.classic_snap_anchor,
                    ),
            },
        ),
        _ => None,
    };

    Ok(PreparedCompanionFrame {
        bounds: prepared_bounds,
        aperture,
        background,
        dim_overlay,
        renderer,
        gauges,
        hud,
        hud_font_size: match &renderer {
            PreparedRendererFrame::Classic { metrics, .. }
            | PreparedRendererFrame::Smooth { metrics, .. } => metrics.font_size,
            PreparedRendererFrame::Pixel { .. } => 8.5,
        },
        overlay_commands,
        review_sample,
    })
}
```

- [ ] **Step 6: Wire preparation into `ui_tick`**

Add:

```rust
fn prepare_current_frame_from_state() {
    let result = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        let bounds = state.view.bounds();
        let prepared = prepare_companion_frame(
            &state.vm,
            &state.scene,
            state.renderer_mode,
            state.pixel_frame.as_ref(),
            state.smooth_started_at,
            state.smooth_semantic_art_tick_index,
            state.redacts_live_hud,
            bounds,
            &mut state.metric_cache,
        );
        match prepared {
            Ok(frame) => {
                state.last_good_frame = Some(frame);
                state.last_frame_preparation_error = None;
            }
            Err(err) => {
                state.last_frame_preparation_error = Some(err);
                if let Some(capture) = state.review_capture.as_mut() {
                    capture.record_frame_preparation_error(err.category());
                    if state.last_good_frame.is_some() {
                        capture.record_last_good_frame_reused();
                    }
                }
                eprintln!("glorp companion frame preparation failed: {}", err.category());
            }
        }
        Some(())
    });
    let _ = result;
}
```

Change `ui_tick()` order to:

```rust
fn ui_tick() {
    let _mtm = MainThreadMarker::new().expect("companion ui_tick on non-main thread");
    drain_poll_results();
    animate_pet();
    prepare_current_frame_from_state();
    finish_review_capture_if_due();
}
```

After installing `AppState` in `run(...)`, call `prepare_current_frame_from_state();` before scheduling the timer.

- [ ] **Step 7: Run app tests**

Run:

```bash
cargo test --lib companion::app::tests -- --nocapture
```

Expected: all `companion::app::tests::*` pass.

- [ ] **Step 8: Commit**

```bash
git status --short
git add src/companion/app.rs
git commit -m "feat(companion): prepare native frames before draw"
```

## Task 5: Guard Objective-C Callbacks and Paint Prepared Frames

**Files:**
- Modify: `src/companion/app.rs`

**Interfaces:**
- Consumes:
  - `PreparedCompanionFrame` from Task 3
  - `AppState::last_good_frame` from Task 4
  - Review-capture telemetry methods from Task 1
- Produces:
  - `run_objc_callback(...)`
  - `record_callback_panic(...)`
  - `paint_prepared_frame(...)`
  - `paint_fallback_background(...)`

- [ ] **Step 1: Write callback-guard unit test**

Add to `src/companion/app.rs` tests:

```rust
#[test]
fn objc_callback_guard_catches_unwind() {
    let did_run = std::cell::Cell::new(false);

    run_objc_callback("drawRect", || {
        did_run.set(true);
        panic!("injected callback panic");
    });

    assert!(did_run.get());
}
```

The default panic hook may still print the injected panic in test output. Do not install a global panic hook for this test.

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cargo test --lib companion::app::tests::objc_callback_guard_catches_unwind -- --nocapture
```

Expected: fail to compile because `run_objc_callback` does not exist.

- [ ] **Step 3: Add the callback guard**

Add:

```rust
fn run_objc_callback(label: &'static str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        record_callback_panic(label);
    }
}

fn record_callback_panic(label: &'static str) {
    eprintln!("glorp companion caught panic in Objective-C callback: {label}");
    APP_STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            if let Some(state) = state.as_mut() {
                state.callback_panic_count = state.callback_panic_count.saturating_add(1);
                state.last_callback_panic_label = Some(label);
                if let Some(capture) = state.review_capture.as_mut() {
                    capture.record_callback_panic(label);
                }
            }
        }
    });
}
```

Change Objective-C selectors:

```rust
#[method(uiTick:)]
fn ui_tick(&self, _sender: Option<&AnyObject>) {
    run_objc_callback("uiTick", ui_tick);
}
```

```rust
#[method(drawRect:)]
fn draw_rect(&self, _rect: NSRect) {
    run_objc_callback("drawRect", || draw_scene(self, self.bounds()));
}
```

- [ ] **Step 4: Replace `draw_scene` with prepared-frame painting**

Change `draw_scene(...)` to read the prepared frame:

```rust
fn draw_scene(view: &RoundView, bounds: NSRect) {
    let Some(_mtm) = MainThreadMarker::new() else {
        eprintln!("glorp companion draw_scene called off main thread");
        return;
    };
    let frame = APP_STATE.with(|cell| cell.borrow().as_ref().and_then(|s| s.last_good_frame.clone()));
    match frame {
        Some(frame) => {
            paint_prepared_frame(view, bounds, &frame);
            record_review_frame(view, frame.review_sample);
        }
        None => paint_fallback_background(bounds),
    }
}
```

Move the current AppKit drawing body into `paint_prepared_frame(...)`, replacing local computed values with fields from `PreparedCompanionFrame`.

Required substitutions:

```rust
let aperture = frame.aperture;
let bg_color = frame.background;
let dim_overlay = frame.dim_overlay;
let commands = &frame.overlay_commands;
let hud_text = &frame.hud;
let hud_font_size = frame.hud_font_size;
```

For renderer-specific tank drawing:

```rust
match &frame.renderer {
    PreparedRendererFrame::Pixel { frame: pixel_frame } => {
        crate::companion::pixel::draw_pixel_frame(pixel_frame, bounds, aperture, &frame.hud);
    }
    PreparedRendererFrame::Smooth { metrics, plan, .. } => {
        draw_mood_aura(frame, metrics);
        appkit_blit_smooth_plan(
            plan,
            metrics.font_size,
            metrics.cell_w,
            metrics.cell_h,
            metrics.origin_x,
            metrics.origin_y,
        );
    }
    PreparedRendererFrame::Classic {
        metrics,
        draw_list,
        ..
    } => {
        draw_mood_aura(frame, metrics);
        appkit_blit_draw_list(
            draw_list,
            metrics.font_size,
            metrics.cell_w,
            metrics.cell_h,
            metrics.origin_x,
            metrics.origin_y,
        );
    }
}
```

Implement `draw_mood_aura(frame, metrics)` by moving the existing aura loop and reading `pet_center_col`, `pet_center_row`, and `pet_width_cells` from either `PreparedRendererFrame::Classic` or `PreparedRendererFrame::Smooth`.

Add:

```rust
fn paint_fallback_background(bounds: NSRect) {
    let width = bounds.size.width.max(1.0);
    let height = bounds.size.height.max(1.0);
    let radius = width.min(height) / 2.0;
    let cx = width / 2.0;
    let cy = height / 2.0;
    unsafe {
        let bg_path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(cx - radius, cy - radius),
            NSSize::new(radius * 2.0, radius * 2.0),
        ));
        ns_color(&RoundColor(0.05, 0.06, 0.10, 1.0)).setFill();
        bg_path.fill();
    }
}
```

- [ ] **Step 5: Remove draw-time frame construction**

Confirm this search returns no matches inside `draw_scene` or `paint_prepared_frame`:

```bash
rg -n "layout_round_scene|build_draw_commands|build_round_smooth_scene_plan|try_build_round_smooth_scene_plan|build_round_scene_draw_list|live_hud_text" src/companion/app.rs
```

Expected: matches may remain in `prepare_companion_frame`, tests, imports, or helpers, but not in draw/paint functions.

- [ ] **Step 6: Run app tests**

Run:

```bash
cargo test --lib companion::app::tests -- --nocapture
```

Expected: all app tests pass. The injected panic test may print panic-hook output while still passing.

- [ ] **Step 7: Commit**

```bash
git status --short
git add src/companion/app.rs
git commit -m "fix(companion): guard native callbacks and paint prepared frames"
```

## Task 6: Full Verification and Native Smoke

**Files:**
- Modify only if tests reveal an in-scope bug:
  - `src/companion/app.rs`
  - `src/companion/review_capture.rs`
  - `src/round/smooth.rs`
  - `tests/smooth_companion.rs`

**Interfaces:**
- Consumes all prior tasks.
- Produces verified implementation evidence.

- [ ] **Step 1: Run focused Rust verification**

Run:

```bash
cargo test --test smooth_companion -- --nocapture
cargo test --lib companion::app::tests -- --nocapture
cargo test --lib companion::review_capture::tests -- --nocapture
cargo test --test round_scene -- --nocapture
cargo test --test cli_smoke companion_ -- --nocapture
```

Expected: all pass.

- [ ] **Step 2: Build the macOS companion bundle**

Run:

```bash
cargo xtask companion fresh
```

Expected: the bundle builds and launches. If it leaves the app running, quit it before native review runs.

- [ ] **Step 3: Run bounded native review checks**

Run:

```bash
RUST_BACKTRACE=full target/macos/Glorp.app/Contents/MacOS/glorp-companion companion-app --renderer classic --review-duration-ms 2500
RUST_BACKTRACE=full target/macos/Glorp.app/Contents/MacOS/glorp-companion companion-app --renderer smooth --review-duration-ms 2500
RUST_BACKTRACE=full target/macos/Glorp.app/Contents/MacOS/glorp-companion companion-app --renderer smooth --review-size 960x960 --review-duration-ms 18000
```

Expected: each command exits 0, no `panic_cannot_unwind`, no `abort() called`, and no new `.ips` crash report for `glorp-companion` appears in `~/Library/Logs/DiagnosticReports` during the run window.

- [ ] **Step 4: Inspect native review capture when artifacts are useful**

If a capture directory is used during debugging, inspect `render-log.json`:

```bash
jq '{callback_panic_count, frame_preparation_error_count, last_good_frame_reused_count, panic}' /path/to/render-log.json
```

Expected for clean smoke: all counts are `0` and `panic` is `false`.

- [ ] **Step 5: Final status**

Run:

```bash
git status --short --branch
git log --oneline -6
```

Expected: only intentional commits from this plan are present, with no uncommitted changes.
