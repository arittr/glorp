# Glorp Smooth Companion Renderer v2 Slice 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first hidden Smooth Companion renderer slice so the current Classic Glorp companion still looks like itself, but is rendered through semantic layers that can move with continuous, fractional motion.

**Architecture:** Add a backend-neutral `SmoothCompanionScenePlan` above the current Classic companion scene passes. The existing Classic draw-list path remains the default and `DrawCell` remains the compatibility output. The new hidden `--renderer smooth` path consumes the same Classic pet/world layers and applies one visible fractional `PetBody` motion proof in AppKit review mode. Preview Lab owns deterministic parity and motion artifacts before live review is trusted.

**Tech Stack:** Rust, ratatui buffers, serde/serde_json, clap, Preview Lab, existing `WatchViewModel`, `SceneDrawList`, `round::scene`, AppKit/objc2 for macOS companion hosting, existing `cargo xtask companion fresh` workflow.

## Global Constraints

- Implement Slice 1 from `docs/superpowers/specs/2026-07-08-glorp-smooth-companion-renderer-v2-design.md`.
- Preserve Classic companion identity: current generated pet art, habitat props, tank life, ambient glyphs, mood aura, porthole depth, HUD, and perimeter gauges.
- Add `--renderer smooth`; do not reuse `--renderer pixel`.
- Keep Classic as the default renderer and leave the current Pixel renderer intact.
- Do not change `glorp watch` terminal rendering.
- Do not add a full 3D engine, camera system, physics engine, Linux windowing path, or authored asset pipeline in this slice.
- Do not infer layer roles from glyphs, colors, or already-flattened cells.
- Keep smooth artifacts privacy-safe: no source names, exact token strings, project names, file paths, prompts, responses, raw diagnostics, or unprojected pet seed values.
- The first live result must visibly demonstrate smooth capability through a fractional `PetBody` bob/drift/breath transform.
- Use TDD task by task: write focused failing coverage, see the failure, implement the smallest pass, rerun focused checks, then commit.

---

## Scope

This plan implements only Slice 1 plus the minimum native review harness needed to inspect it. Slice 3 depth polish and Slice 4 pixel/3D-ish body treatment stay out of scope.

The expected end state after this plan:

- `cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview` emits Classic baseline, smooth parity, plan, parity, and motion artifacts.
- `cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/smooth-360-active` opens the smooth renderer, animates the Classic pet layer with visible sub-cell motion, captures review evidence, and exits without manual quit.
- The smooth renderer still reads as the current Classic Glorp companion, not the earlier abstract Pixel mode.

## File Map

| Path | Responsibility |
| --- | --- |
| `src/presentation/smooth.rs` | Smooth scene-plan types, layer roles, transforms, flattening compatibility, motion math, privacy claims. |
| `src/presentation/mod.rs` | Export `presentation::smooth`. |
| `src/tui/panels/pet/layered.rs` | Layer-aware builder for the existing Classic pet scene passes. |
| `src/tui/panels/pet.rs` | Expose the layered pet-scene module. |
| `src/tui/panels/pet/draw.rs` | Keep existing draw-list API; route it through layered flattening after parity passes. |
| `src/round/scene.rs` | Extract shared round layout and uniform porthole recolor helpers for Classic and Smooth. |
| `src/round/smooth.rs` | Build `SmoothCompanionScenePlan` for round companion fixtures. |
| `src/round/mod.rs` | Export `round::smooth`. |
| `src/dev_preview/smooth.rs` | Build smooth Preview Lab frames and strips. |
| `src/dev_preview/mod.rs` | Export `dev_preview::smooth`. |
| `src/dev_preview/contract.rs` | Add smooth plan, parity, and motion contract structs. |
| `src/dev_preview/export.rs` | Add smooth manifest file fields, artifact types, JSON writers, and review links. |
| `src/dev_preview/scenarios.rs` | Wire `PreviewSelection::Smooth` and include smooth artifacts in `All`. |
| `src/commands/dev_preview.rs` | Map CLI smooth scenario to preview selection. |
| `src/cli.rs` | Add hidden `dev-preview --scenario smooth` and hidden companion review flags. |
| `src/commands/companion_mode.rs` | Add `Smooth` renderer mode plus review-state/duration/capture options. |
| `src/commands/companion.rs` | Forward smooth renderer and review options through `open --args`. |
| `src/commands/companion_app.rs` | Pass review options into the native app entrypoint. |
| `src/lib.rs` | Dispatch expanded companion review options from parsed CLI commands. |
| `src/companion/app.rs` | Store smooth motion/review state and render smooth layers in AppKit. |
| `src/companion/review_capture.rs` | Write bounded review logs and screenshots on macOS. |
| `src/companion/mod.rs` | Export capture module and updated app run signature. |
| `tests/smooth_companion.rs` | Smooth plan parity, role, privacy, and motion tests. |
| `tests/dev_preview.rs` | Smooth Preview Lab manifest/artifact/privacy tests. |
| `tests/cli_smoke.rs` | Hidden smooth renderer and review flag parsing tests. |

## Core Interfaces

Implement these concrete boundaries. Small field additions are fine when tests require them, but do not rename the domain concepts.

```rust
// src/presentation/smooth.rs
pub struct SmoothCompanionScenePlan {
    pub viewport: SmoothViewport,
    pub layers: Vec<SmoothCompanionLayer>,
    pub pet: SmoothCompanionPet,
    pub chrome: CompanionChromeReservation,
    pub privacy: SmoothCompanionPrivacyClaims,
}

pub struct SmoothCompanionLayer {
    pub id: SmoothLayerId,
    pub role: SmoothLayerRole,
    pub z: i16,
    pub local_bounds: SmoothBounds,
    pub anchor: SmoothPoint,
    pub transform_origin: SmoothPoint,
    pub transform: SmoothTransform,
    pub opacity: f32,
    pub clip: SmoothClip,
    pub blend: SmoothBlendMode,
    pub items: Vec<SmoothLayerItem>,
}

pub enum SmoothLayerRole {
    DepthRings,
    BiomeWash,
    RoomGlyphs,
    Ambient,
    Motes,
    ActivityGlyphs,
    PropsBehind,
    TankLifeBehind,
    ChestBubble,
    ContactShadow,
    PetBody,
    PerformanceCue,
    PropsForeground,
    TankLifeForeground,
    StatusHalo,
    TroubleIndicator,
    MoodAura,
    DimOverlay,
}

impl SmoothCompanionScenePlan {
    pub fn flatten_classic_cells(&self) -> crate::presentation::SceneDrawList;
    pub fn layer(&self, role: SmoothLayerRole) -> Option<&SmoothCompanionLayer>;
    pub fn classic_flatten_checksum(&self) -> String;
}

pub fn smooth_pet_bob(elapsed_ms: u64) -> f32;
```

```rust
// src/tui/panels/pet/layered.rs
pub(crate) fn render_layered_pet_scene_with_tank_geometry(
    model: &crate::presentation::PetSceneModel,
    vm: &crate::tui::view_model::WatchViewModel,
    layout: &crate::tui::component::PetSceneLayout,
    now: time::OffsetDateTime,
    ctx: &crate::tui::render_context::RenderContext,
    tank_geometry: &crate::tui::component::TankLifeSurfaceGeometry,
) -> crate::presentation::smooth::LayeredPetScene;
```

```rust
// src/round/smooth.rs
pub fn build_round_smooth_scene_plan(
    vm: &crate::tui::view_model::WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &crate::round::scene::CompanionMotion,
    elapsed_ms: u64,
) -> crate::presentation::smooth::SmoothCompanionScenePlan;
```

```rust
// src/dev_preview/smooth.rs
pub fn smooth_frames(
    ctx: &crate::dev_preview::scenarios::PreviewRenderContext,
    scratch_dir: &std::path::Path,
) -> crate::error::Result<Vec<crate::dev_preview::scenarios::PreviewScenarioBundle>>;

pub fn smooth_strips(
    ctx: &crate::dev_preview::scenarios::PreviewRenderContext,
) -> Vec<crate::dev_preview::strips::PreviewStripBundle>;
```

## Task 1: Smooth Scene Plan Core

**Files:** `src/presentation/smooth.rs`, `src/presentation/mod.rs`

- [ ] Add failing unit tests in `src/presentation/smooth.rs` for:
  - z-stable flattening from local cells into `SceneDrawList`
  - preserving insertion order within the same z layer
  - `SmoothLayerRole::as_str()` kebab-case names for every required role
  - `smooth_pet_bob(0)`, `smooth_pet_bob(250)`, and `smooth_pet_bob(500)` are deterministic, fractional, nonzero at at least one sampled frame, and bounded under one cell
  - `SmoothCompanionPrivacyClaims::external_companion()` has every private field set to `false`
- [ ] Verify failure:
  ```bash
  cargo test presentation::smooth
  ```
- [ ] Implement `src/presentation/smooth.rs` with:
  - `SmoothPoint`, `SmoothBounds`, `SmoothTransform`, `SmoothClip`, `SmoothBlendMode`
  - `SmoothLayerId`, `SmoothLayerRole`, `SmoothLayerItem`, `SmoothLocalCell`, optional simple shape/raster refs for future extension
  - `LayeredPetScene { layers: Vec<SmoothCompanionLayer> }`
  - `flatten_classic_cells()` that orders by `(z, original_layer_index)` and writes `DrawCell`s in layer item order
  - `classic_flatten_checksum()` using a deterministic local hash over row, col, glyph, fg, bg, and bold
  - `smooth_pet_bob(elapsed_ms)` as a small sine wave with amplitude below `0.5` cells
- [ ] Export the module from `src/presentation/mod.rs`.
- [ ] Rerun:
  ```bash
  cargo test presentation::smooth
  ```
- [ ] Commit:
  ```bash
  git add src/presentation/mod.rs src/presentation/smooth.rs
  git commit -m "feat(smooth): add companion scene plan core"
  ```

## Task 2: Preserve Classic Pet Passes As Layers

**Files:** `src/tui/panels/pet/layered.rs`, `src/tui/panels/pet.rs`, `src/tui/panels/pet/draw.rs`

- [ ] Add failing tests for `render_layered_pet_scene_with_tank_geometry` before routing the old draw function through it:
  - a prop-rich active fixture contains `BiomeWash`, `RoomGlyphs`, `Ambient`, `Motes`, `ActivityGlyphs`, `PropsBehind`, `TankLifeBehind`, `ContactShadow`, `PetBody`, `PerformanceCue`, `PropsForeground`, and `TankLifeForeground`
  - a treasure-chest-earned fixture contains `ChestBubble`
  - `layered.flatten_classic_cells()` exactly equals the current `render_pet_to_draw_list_with_tank_geometry(...)` output for fixed `WatchViewModel`, `now`, layout, context, and tank geometry
- [ ] Verify failure:
  ```bash
  cargo test tui::panels::pet::layered
  ```
- [ ] Add `src/tui/panels/pet/layered.rs` and split the existing pass sequence in `draw.rs` into role-bearing layer pushes. The required pass order is:
  ```text
  biome wash
  room glyphs
  ambient glyphs
  motes
  activity glyphs
  props behind
  tank life behind
  chest bubble
  contact shadow
  pet body
  performance cue
  props foreground
  tank life foreground
  ```
- [ ] Use local coordinates inside each layer. Each layer anchor must be the top-left point that makes flattening reproduce the current absolute draw cells.
- [ ] Keep the existing public `render_pet_to_draw_list_with_tank_geometry(...)` unchanged until the parity test passes.
- [ ] After parity passes, route `render_pet_to_draw_list_with_tank_geometry(...)` through `render_layered_pet_scene_with_tank_geometry(...).flatten_classic_cells()`.
- [ ] Rerun:
  ```bash
  cargo test tui::panels::pet::layered
  cargo test --test round_draw_list
  ```
- [ ] Commit:
  ```bash
  git add src/tui/panels/pet.rs src/tui/panels/pet/draw.rs src/tui/panels/pet/layered.rs
  git commit -m "feat(smooth): preserve classic pet scene layers"
  ```

## Task 3: Round Smooth Plan And Classic Parity

**Files:** `src/round/scene.rs`, `src/round/smooth.rs`, `src/round/mod.rs`, `tests/smooth_companion.rs`

- [ ] Add failing integration tests in `tests/smooth_companion.rs` for:
  - `build_round_smooth_scene_plan(...).flatten_classic_cells()` exactly equals `build_round_scene_draw_list(...).draw_list` for a fixed parity fixture
  - required roles include depth/chrome roles plus all Classic pet-scene roles
  - `PetBody` has a fractional `transform.translate_y` when `elapsed_ms` is in the moving part of the bob cycle
  - `PetBody` still uses Classic cell art items, not Pixel frame/raster items
  - `SmoothCompanionPrivacyClaims` stay external-safe
- [ ] Verify failure:
  ```bash
  cargo test --test smooth_companion
  ```
- [ ] In `src/round/scene.rs`, extract existing layout/postprocess logic into shared helpers:
  - `build_round_pet_layout(vm, now, grid_cols, grid_rows, motion) -> (Cow<WatchViewModel>, PetSceneLayout, Rect)`
  - `apply_uniform_porthole_recolor(draw_list: &mut SceneDrawList, grid_rows: u16)`
  - Keep `build_round_scene_draw_list(...)` behavior byte-identical by calling those helpers.
- [ ] Add `src/round/smooth.rs`:
  - call the shared layout helper and `render_layered_pet_scene_with_tank_geometry(...)`
  - add smooth-only layers for `DepthRings`, `StatusHalo`, `TroubleIndicator`, `MoodAura`, and `DimOverlay` as semantic layer records with privacy-safe cells or zero-item reservations when the current Classic path renders them outside the draw list
  - apply the same uniform porthole recolor during `flatten_classic_cells()` compatibility
  - set the `PetBody` layer transform to include `smooth_pet_bob(elapsed_ms)`
  - update pet/chrome bounds from the drifted layout and reserve the bottom HUD/gauge regions
- [ ] Export `round::smooth`.
- [ ] Rerun:
  ```bash
  cargo test --test smooth_companion
  cargo test --test round_scene
  cargo test --test round_draw_list
  ```
- [ ] Commit:
  ```bash
  git add src/round/scene.rs src/round/smooth.rs src/round/mod.rs tests/smooth_companion.rs
  git commit -m "feat(smooth): build round companion scene plan"
  ```

## Task 4: Preview Lab Smooth Scenario

**Files:** `src/dev_preview/smooth.rs`, `src/dev_preview/mod.rs`, `src/dev_preview/contract.rs`, `src/dev_preview/export.rs`, `src/dev_preview/scenarios.rs`, `src/commands/dev_preview.rs`, `src/cli.rs`, `tests/dev_preview.rs`

- [ ] Add failing tests in `tests/dev_preview.rs` for:
  - `dev-preview --scenario smooth` succeeds
  - `manifest.json` includes `PreviewScenarioKind::Smooth` and `PreviewStripKind::SmoothMotion`
  - smooth parity frame files include `.smooth-plan.json` and `.smooth-parity.json`
  - smooth strip frame files include `.smooth-motion.json`
  - `review.md` links the smooth plan, parity, and motion artifacts
  - smooth sidecars pass the same privacy token scan used by round/pixel artifacts
  - parity artifact reports exact checksum match for the fixed Classic fixture
  - motion artifact records at least five distinct fractional bob/drift values across strip frames
- [ ] Verify failure:
  ```bash
  cargo test --features dev-preview --test dev_preview dev_preview_smooth
  ```
- [ ] Add enum variants:
  - `PreviewScenarioArg::Smooth` in `src/cli.rs`
  - `PreviewSelection::Smooth` in `src/dev_preview/scenarios.rs`
  - `PreviewScenarioKind::Smooth` in `src/dev_preview/export.rs`
  - `PreviewStripKind::SmoothMotion` in `src/dev_preview/export.rs`
  - `ArtifactType::{SmoothPlan, SmoothParity, SmoothMotion}` in `src/dev_preview/export.rs`
- [ ] Add smooth file fields:
  - `PreviewScenarioFiles::{smooth_plan, smooth_parity}`
  - `PreviewStripFrameFiles::smooth_motion`
- [ ] Add contract structs in `src/dev_preview/contract.rs`:
  - `PreviewSmoothPlanArtifact`
  - `PreviewSmoothLayerArtifact`
  - `PreviewSmoothParityArtifact`
  - `PreviewSmoothMotionArtifact`
  Each struct must serialize only role names, z order, local bounds, transforms, item counts, chrome reservations, checksums, abstract state buckets, and privacy claims.
- [ ] Add writer support in `src/dev_preview/export.rs` for the three smooth sidecars and include them in artifact inventory and review markdown.
- [ ] Implement `src/dev_preview/smooth.rs`:
  - baseline frame id: `round-smooth-classic-baseline`
  - parity frame id: `round-smooth-classic-parity`
  - strip id: `round-smooth-motion`
  - add `scene_draw_list_to_preview_frame(id, title, width, height, draw_list)` as a private helper in `src/dev_preview/smooth.rs`
  - render baseline from `build_round_scene_draw_list(...)`
  - render parity from `build_round_smooth_scene_plan(...).flatten_classic_cells()`
  - attach smooth plan/parity contracts to parity frame
  - create at least five motion strip frames at deterministic elapsed times and attach smooth-motion contracts to every strip frame
- [ ] Wire smooth generation into `generate_preview_bundle` for `Smooth` and `All`.
- [ ] Rerun:
  ```bash
  cargo test --features dev-preview --test dev_preview dev_preview_smooth
  cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview
  ```
- [ ] Commit:
  ```bash
  git add src/dev_preview src/commands/dev_preview.rs src/cli.rs tests/dev_preview.rs
  git commit -m "feat(smooth): add preview lab parity artifacts"
  ```

## Task 5: Hidden Smooth Renderer Mode

**Files:** `src/commands/companion_mode.rs`, `src/commands/companion.rs`, `src/commands/companion_app.rs`, `src/companion/app.rs`, `tests/cli_smoke.rs`

- [ ] Add failing CLI smoke tests:
  - `glorp companion --renderer smooth` parses before the macOS availability gate on non-macOS
  - `glorp companion-app --renderer smooth` parses before the macOS availability gate on non-macOS
  - `glorp companion --help` and `glorp companion-app --help` still hide `--renderer`, `classic`, `pixel`, and `smooth`
  - unknown renderer values still fail
- [ ] Verify failure:
  ```bash
  cargo test --test cli_smoke companion_ -- --nocapture
  ```
- [ ] Add `CompanionRendererMode::Smooth` with `as_str() == "smooth"` and `is_smooth()`.
- [ ] Update `commands::companion::build_open_command(...)` so `Smooth` gets `open -n ... --args --renderer smooth`, matching Pixel's fresh-window behavior.
- [ ] Update `commands::companion_app::run(...)` and `companion::run(...)` signatures to pass review options through unchanged.
- [ ] In `src/companion/app.rs`:
  - keep Classic path unchanged for default mode
  - keep Pixel path unchanged for Pixel mode
  - for Smooth mode, build the Classic semantic `WatchViewModel`, call `build_round_smooth_scene_plan(...)`, and render the flattened Classic-compatible cells through the existing AppKit cell blitter first
  - add a smooth timer cadence close to 30 FPS for Smooth mode
  - store `smooth_elapsed_ms`/start instant in `AppState`
- [ ] Rerun:
  ```bash
  cargo test --test cli_smoke companion_ -- --nocapture
  cargo build
  ```
- [ ] Commit:
  ```bash
  git add src/commands/companion_mode.rs src/commands/companion.rs src/commands/companion_app.rs src/companion/app.rs tests/cli_smoke.rs
  git commit -m "feat(companion): add hidden smooth renderer mode"
  ```

## Task 6: Visible AppKit PetBody Motion And Review Capture

**Files:** `src/commands/companion_mode.rs`, `src/commands/companion.rs`, `src/commands/companion_app.rs`, `src/cli.rs`, `src/lib.rs`, `src/companion/app.rs`, `src/companion/review_capture.rs`, `src/companion/mod.rs`, `tests/cli_smoke.rs`

- [ ] Add failing CLI smoke tests for hidden review flags:
  - `--review-state normal`
  - `--review-state active-pulse`
  - `--review-state asleep-calm`
  - `--review-state helper-trouble`
  - `--review-duration-ms 2000`
  - `--review-capture-dir target/glorp-review/test`
  - existing `--review-active-pulse` still parses and maps to active pulse when `--review-state` is absent
- [ ] Verify failure:
  ```bash
  cargo test --test cli_smoke companion_review -- --nocapture
  ```
- [ ] Extend `CompanionReviewOptions`:
  - `state: Option<CompanionReviewState>`
  - `duration_ms: Option<u64>`
  - `capture_dir: Option<PathBuf>`
  - keep `initial_size` and `active_pulse`
- [ ] Add `CompanionReviewState` as a hidden clap value enum with values `normal`, `active-pulse`, `asleep-calm`, and `helper-trouble`.
- [ ] Forward review state, duration, and capture dir through `commands::companion::build_open_command(...)`.
- [ ] Update `Cli::companion_review_options(...)` and `src/lib.rs` command dispatch so `review_state`, `review_duration_ms`, and `review_capture_dir` are included for both `companion` and `companion-app`.
- [ ] Apply review state in AppKit VM construction:
  - `active-pulse` uses the existing live review signal path
  - `asleep-calm` forces a sleeping/calm fixture without leaking user data
  - `helper-trouble` forces trouble indicator state without leaking diagnostics
  - `normal` leaves the deterministic live path alone
- [ ] Render Smooth mode from layer data rather than only from fully flattened cells:
  - draw non-pet layers at integer cell positions using the existing AppKit cell logic
  - draw the `PetBody` layer with fractional `transform.translate_y` and its cell glyphs mapped to fractional AppKit coordinates
  - leave HUD and gauges in their existing AppKit composition
  - preserve porthole clipping
  - make the bob visible at 360x360 while keeping amplitude under one cell
- [ ] Add `src/companion/review_capture.rs`:
  - create the capture directory
  - write `render-log.json` with renderer, review state, requested size, frame count, elapsed duration, smooth bob samples, and panic flag
  - write `screenshot.png` of the companion window after at least five frames on macOS
  - request app termination after `review_duration_ms`
  - keep the module behind macOS cfg where AppKit capture APIs are required
- [ ] Before coding screenshot capture, inspect local objc2/AppKit/CoreGraphics bindings with:
  ```bash
  rg -n "CGWindowList|dataWithTIFF|bitmapImageRep|representationUsingType|PNG" ~/.cargo/registry/src src
  ```
  Use the available local API names found by that command; do not invent bindings.
- [ ] Rerun:
  ```bash
  cargo test --test cli_smoke companion_review -- --nocapture
  cargo build
  ```
- [ ] On macOS, run:
  ```bash
  cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/smooth-360-active
  ```
  Expected: exits 0, writes `screenshot.png`, writes `render-log.json`, records at least five frames, and records changing smooth bob samples.
- [ ] Commit:
  ```bash
  git add src/commands/companion_mode.rs src/commands/companion.rs src/commands/companion_app.rs src/cli.rs src/lib.rs src/companion src/companion/mod.rs tests/cli_smoke.rs
  git commit -m "feat(companion): capture smooth renderer review evidence"
  ```

## Task 7: Final Verification And Review Evidence

**Files:** source files from previous tasks only.

- [ ] Run formatting:
  ```bash
  cargo fmt --check
  ```
- [ ] Run focused core tests:
  ```bash
  cargo test --test smooth_companion
  cargo test --test round_scene
  cargo test --test round_draw_list
  cargo test --test cli_smoke companion_ -- --nocapture
  ```
- [ ] Run Preview Lab tests:
  ```bash
  cargo test --features dev-preview --test dev_preview dev_preview_smooth
  ```
- [ ] Generate smooth Preview Lab bundle:
  ```bash
  cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview
  ```
- [ ] Inspect artifact presence:
  ```bash
  ls target/glorp-preview/frames/*smooth*
  ls target/glorp-preview/strips/round-smooth-motion
  ```
- [ ] Run native Smooth capture:
  ```bash
  cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/smooth-360-active
  ```
- [ ] Run native Classic comparison capture:
  ```bash
  cargo run -- companion-app --renderer classic --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/classic-360-active
  ```
- [ ] Check for fresh crash reports:
  ```bash
  ls -lt ~/Library/Logs/DiagnosticReports/glorp-companion*.ips 2>/dev/null | head -5
  ```
  Confirm no report timestamp is later than the review capture start.
- [ ] Run a placeholder and privacy scan over changed docs and smooth artifacts:
  ```bash
  PLAN_SCAN_PATTERN="$(printf '%s' 'TB''D|TO''DO|FIX''ME|PLACE''HOLDER|does not'' exist|similar'' to|implement'' later|fill'' in')"
  rg -n "$PLAN_SCAN_PATTERN" docs/superpowers/plans src tests
  rg -n "Users/|/var/folders|prompt|response|diagnostic|raw_source|project" target/glorp-preview/frames target/glorp-preview/strips
  ```
  Expected: no unresolved plan/source placeholders, and no private smooth artifact leaks.
- [ ] Commit any final narrow verification fixes:
  ```bash
  git status --short
  git add <changed-source-files>
  git commit -m "fix(companion): verify smooth renderer review path"
  ```
  Do not create an empty commit.

## Implementation Notes

- The first smooth AppKit rendering pass may still draw glyph cells. The important difference is that it draws the `PetBody` layer from semantic layer data with a fractional transform.
- The smooth plan must carry props and tank life as first-class required roles; missing props/tank life is a failed Slice 1 artifact.
- The earlier Pixel renderer remains useful as research and future body-style input, but it is not part of this implementation plan except to ensure `--renderer pixel` behavior is not broken.
- Keep HUD and perimeter gauges in the existing AppKit composition for this slice. The smooth plan records safe regions and privacy claims, not exact HUD/token values.
- Avoid adding new dependencies unless the implementation hits a hard local API blocker and Drew approves the addition.

## Acceptance Checklist

- [ ] Classic renderer remains the default and current Pixel renderer still works.
- [ ] `--renderer smooth` is hidden, parses, and launches a fresh native companion window.
- [ ] Smooth Preview Lab artifacts prove exact Classic flatten parity for fixed fixtures.
- [ ] Smooth artifacts expose required layer roles, chrome reservations, privacy claims, and motion metadata.
- [ ] Live Smooth mode visibly moves the Classic `PetBody` layer with sub-cell motion.
- [ ] Pet art, habitat props, tank life, ambient marks, mood aura, HUD, and perimeter gauges are present.
- [ ] Native review capture writes screenshot/log evidence and exits automatically.
- [ ] No smooth artifacts leak private user/project/source/prompt/diagnostic data.
- [ ] Drew can compare Classic and Smooth and still recognize the current Glorp companion.
