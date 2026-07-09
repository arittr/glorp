# Glorp Companion Draw Boundary Hardening - design

- Date: 2026-07-09
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md`
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-companion-renderer-v2-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-smooth-motion-stabilization-design.md`

## Calibration

The companion crashes are coming from a Rust panic reaching AppKit's Objective-C
callback frame. The crash reports show `panic_cannot_unwind` under
`glorp::companion::app::RoundView::draw_rect`, followed by AppKit and
QuartzCore display flushing. That shape does not prove that native drawing or
native timers are the wrong product layer. It proves that Rust work inside
Objective-C callbacks is allowed to panic, and that `drawRect` currently does
too much work for such a sharp boundary.

The first slice should keep the current product direction: Rust owns the Glorp
scene, AppKit owns the macOS window and paint host. The repair is to make the
boundary boring. Build the next frame as Rust data before AppKit asks to draw,
store the last good frame, and make `drawRect` a small guarded painter.

## Problem

`src/companion/app.rs` currently invokes Rust directly from two Objective-C
selectors:

```text
Controller::uiTick:  -> ui_tick()
RoundView::drawRect: -> draw_scene(self, self.bounds())
```

Both callbacks can cross into panic-capable Rust code without a local unwind
guard. `ui_tick()` and `draw_scene(...)` both use `MainThreadMarker::new()
.expect(...)`, and `draw_scene(...)` builds most of the companion frame on
demand during AppKit drawing.

For non-Pixel rendering, `draw_scene(...)` currently clones live state, derives
the aperture, computes round layout, builds draw commands, measures grid
metrics, derives the smooth scene plan or Classic draw list, computes review
samples, draws the aura, blits the tank, draws gauges, draws halo and trouble
commands, derives HUD text, and records the review frame. Smooth mode also
calls `build_round_smooth_scene_plan(...)`, which has `expect(...)` calls for a
required `PetBody` layer.

Any one of those assumptions can abort the whole companion if it panics inside
`drawRect`. Repaint coalescing can then turn one bad state into repeated
callback failures.

## Goals

1. Prevent Rust unwinds from escaping Objective-C callbacks.
2. Move heavy, panic-prone frame construction out of `drawRect`.
3. Keep rendering output visually equivalent for Classic, Smooth, and Pixel.
4. Preserve the last good frame when a new frame cannot be prepared.
5. Convert smooth planner invariant panics into recoverable frame-preparation
   errors.
6. Cache AppKit font/grid metrics by view size so measurement work is not
   repeated inside every draw.
7. Keep the first implementation slice small enough to review and ship before
   any full bitmap-renderer decision.

## Non-goals

- No rewrite of the macOS companion as a pure Rust bitmap renderer in this
  slice.
- No replacement of AppKit, `NSView`, `NSTimer`, or the current macOS bundle
  path.
- No default renderer change.
- No visual redesign of the companion, gauges, HUD, tank, or smooth renderer.
- No Linux windowing implementation.
- No new animation engine dependency.

## Design

### Callback guard

Add one small boundary helper for Objective-C callback entry points:

```rust
fn run_objc_callback(label: &'static str, f: impl FnOnce()) {
    if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        record_callback_panic(label, payload);
    }
}
```

`Controller::uiTick:` and `RoundView::drawRect:` should call this helper around
their Rust bodies. The guard must never re-panic. It should write a concise
stderr line and record the latest callback failure in `AppState` when state is
available. Review capture may report that a callback panic occurred, but the
companion should continue running.

`record_callback_panic(...)` must use non-panicking state access, such as
`try_borrow_mut()`, because the caught panic may have happened while `APP_STATE`
was already borrowed. If state cannot be borrowed, stderr logging is enough for
that callback.

The guard is not the fix by itself. It is the seatbelt that keeps AppKit alive
while frame preparation falls back to the last known good state.

### Prepared frame

Introduce a Rust-owned prepared frame snapshot stored in `AppState`:

```rust
struct PreparedCompanionFrame {
    bounds_px: PreparedBounds,
    aperture: RoundAperture,
    background: RoundColor,
    dim_overlay: bool,
    renderer: PreparedRendererFrame,
    gauges: PreparedGaugeFrame,
    hud: CompanionHudText,
    hud_font_size: f64,
    overlay_commands: Vec<RoundDrawCommand>,
    review_sample: Option<SmoothReviewFrameSample>,
}

enum PreparedRendererFrame {
    Pixel {
        frame: PixelFrame,
    },
    Classic {
        metrics: CompanionGridMetrics,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        draw_list: crate::presentation::SceneDrawList,
    },
    Smooth {
        metrics: CompanionGridMetrics,
        plan: SmoothCompanionScenePlan,
    },
}
```

Exact type names can shift during implementation, but the direction is fixed:
`drawRect` reads a prepared frame and paints it. It must not call
`layout_round_scene(...)`, `build_draw_commands(...)`,
`build_round_smooth_scene_plan(...)`, `build_round_scene_draw_list(...)`, or
`live_hud_text(...)`.

### Data flow

On startup, after `AppState` is installed, prepare the first frame from the
initial view bounds and store it as `last_good_frame`.

On every `ui_tick()`:

1. drain live poll updates;
2. advance Pixel, Smooth, or Classic animation state using the current logic;
3. read `view.bounds()` on the main thread;
4. prepare a new frame snapshot from the current `vm`, `scene`, renderer mode,
   pixel frame, smooth timing state, review redaction state, and bounds;
5. if preparation succeeds, replace `last_good_frame`;
6. if preparation fails, store the error and keep `last_good_frame`;
7. call `setNeedsDisplay(true)`;
8. finish review capture if due.

`drawRect` becomes:

1. enter the Objective-C callback guard;
2. read `last_good_frame`;
3. paint it with AppKit primitives;
4. if no frame exists yet, paint only the circular fallback background;
5. record the frame's review sample after a successful paint.

This keeps AppKit painting in AppKit, but it removes Rust scene planning from
the display callback.

### Metric cache

`companion_grid_metrics(width, height)` may still use AppKit font measurement,
because the companion is a native macOS surface. It should move into frame
preparation and be cached by a normalized size key:

```rust
struct CompanionMetricCache {
    last: Option<(CompanionMetricKey, CompanionGridMetrics)>,
}

struct CompanionMetricKey {
    width_px: u16,
    height_px: u16,
}
```

The key should reject non-finite, zero, negative, and oversized dimensions
before they reach `RoundAperture::new(width as u16, height as u16)`. If metrics
cannot be computed, preparation returns a recoverable error and the companion
keeps the last good frame.

### Smooth planner errors

`build_round_smooth_scene_plan(...)` should stop panicking when a required layer
is missing. Replace the `expect("round smooth scene should include a pet body
layer")` assumptions with a result type:

```rust
pub fn try_build_round_smooth_scene_plan(...) -> Result<SmoothCompanionScenePlan>
```

The existing `build_round_smooth_scene_plan(...)` name can either become the
fallible function or remain as a test-only compatibility wrapper. Production
frame preparation must use the fallible path. Missing `PetBody` is a frame
preparation error, not a process abort.

### Painting

Split `draw_scene(...)` into two roles:

- `prepare_companion_frame(...)` builds pure Rust data plus cached metrics.
- `paint_prepared_frame(view, bounds, frame)` performs AppKit drawing.

The paint function may create `NSBezierPath`, `NSColor`, `NSFont`, and
`NSMutableAttributedString` objects. It should not mutate companion state other
than review-capture frame recording after paint returns.

Pixel mode keeps drawing the pixel frame through the existing Pixel AppKit
helper. Classic mode keeps using the existing draw-list blitter. Smooth mode
keeps using `appkit_blit_smooth_plan(...)`.

## Error Handling

Frame preparation returns `Result<PreparedCompanionFrame>`. Recoverable errors
include invalid bounds, unavailable grid metrics, missing smooth `PetBody`, and
other local invariant failures. They are logged once per error class or
throttled so a bad frame does not flood stderr at 30 FPS.

If preparation fails and `last_good_frame` exists, the app paints the last good
frame. If preparation fails before any good frame exists, the app paints a
simple clipped circular background using the default dark round color.

Objective-C callback panics are recorded separately from recoverable
preparation errors. A caught panic means something still violated a Rust
invariant, and review capture should surface that fact in `render-log.json`.

## Review Evidence

Extend review capture with boundary-health metadata:

- `callback_panic_count`;
- `last_callback_panic_label`;
- `frame_preparation_error_count`;
- `last_frame_preparation_error`;
- `last_good_frame_reused_count`.

The evidence should remain privacy-safe. Error strings must be static or
sanitized categories, not raw source names, file paths, prompts, token strings,
diagnostics, or user project data.

## Acceptance Criteria

- `Controller::uiTick:` and `RoundView::drawRect:` both catch Rust panics before
  returning to Objective-C.
- `drawRect` does not build round layout, smooth plans, Classic scene draw
  lists, live HUD text, or review samples.
- The app stores and paints a last good prepared frame.
- Invalid or unusual bounds do not panic.
- Missing smooth `PetBody` becomes a preparation error.
- Classic, Smooth, and Pixel visual paths continue to use their current drawing
  primitives.
- Native review capture reports whether callback panics, preparation errors, or
  last-good-frame reuse occurred.
- No new external animation or rendering dependency is introduced.

## Verification Commands

Focused Rust checks:

```bash
cargo test --test smooth_companion
cargo test --test round_scene
cargo test --test cli_smoke companion_ -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_smooth
```

Native smoke checks:

```bash
RUST_BACKTRACE=full target/macos/Glorp.app/Contents/MacOS/glorp-companion companion-app --renderer classic --review-duration-ms 2500
RUST_BACKTRACE=full target/macos/Glorp.app/Contents/MacOS/glorp-companion companion-app --renderer smooth --review-duration-ms 2500
RUST_BACKTRACE=full target/macos/Glorp.app/Contents/MacOS/glorp-companion companion-app --renderer smooth --review-size 960x960 --review-duration-ms 18000
```

The implementation plan may adjust exact command names if repository entry
points change, but it must keep both unit-level Rust checks and native bounded
companion runs.
