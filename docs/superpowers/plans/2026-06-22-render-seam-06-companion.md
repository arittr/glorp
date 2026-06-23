# Render Seam — Plan 06: Companion Renders the Shared Scene (the visible win)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the macOS `companion` render the SAME scene the watch does — build a round-viewport `SceneDrawList` via the shared `render_pet_to_draw_list` and blit it to AppKit (with the circular clip), so the companion inherits grounding/ambient/props/effects/pet that it can't see today. Keep the round halo. The carried debts (`compact`, `ColorCapability`) resolve by parameterization (companion passes its own round `area` + `Truecolor`).

**Architecture:** `render_pet_to_draw_list(scene_model, vm, &PetSceneLayout, now, ctx)` is viewport-agnostic and self-contained (computes `compact` + reacted `life` from the area; resolves nothing watch-specific). So the companion: picks a round cell grid `(cols, rows)` from the view size, builds `area = Rect(0,0,cols,rows)`, pre-resolves wander via `resolve_wander_offset(vm, now, cols)`, builds the `PetSceneLayout` + `PetSceneModel`, calls `render_pet_to_draw_list` with `ColorCapability::Truecolor` → a `SceneDrawList`, and blits it to AppKit (glyph cells as `NSAttributedString drawAtPoint`, bg cells as filled rects, all under the existing aperture clip). The halo beads stay (drawn after, from the existing `build_draw_commands` output). The old `derive_round_scene_model`→`build_draw_commands` path and the dev-preview round-commands goldens are KEPT (untouched) this plan — retiring them is a follow-up.

**Tech Stack:** Rust + objc2 AppKit (`NSAttributedString`, `NSBezierPath`, `NSFont`); `presentation::{SceneDrawList, DrawCell, PetSceneModel}`; `tui::panels::pet::draw::render_pet_to_draw_list`; `tui::wander::resolve_wander_offset`; `tui::component::pet_scene::PetScene::compute_layout`; `tui::render_context::RenderContext`.

This is **Plan 06** — spec Track 3 (companion adapter). **VISUAL CHANGE with NO byte-stable oracle for the live AppKit look** — it is implemented + tested + held on the branch for Drew's visual review (run the companion, eyeball on the round display), NOT auto-merged.

## Global Constraints

- `src/pet/render.rs`/`art.rs` FROZEN; `src/presentation/` no `tui::component::TargetPath`/no ratatui in `draw_list`.
- **Watch must NOT regress:** `render_pet_to_draw_list` and all shared code stay byte-stable for the watch (existing 46 dev-preview goldens unchanged). The companion is the only behavior change.
- **Keep the existing round path + goldens:** do NOT change `derive_round_scene_model`, `build_draw_commands`, `RoundDrawCommand`, or `dev_preview/round.rs` / `round-commands.json` goldens. (`build_draw_commands` stays — used for the halo + dev-preview.)
- **Per-task gate (full suite, EXIT STATUS not `| tail`):** `cargo test` (0 FAILED) + `cargo test --features dev-preview --test dev_preview` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo fmt --check`. (These verify compile + watch byte-stability + the new content golden. They do NOT verify the live AppKit pixels — that's Drew's visual review.)
- Commit per task. **DO NOT MERGE** — hold for visual review.

## File Structure

- **Modify** `src/companion/app.rs` — add `appkit_blit_draw_list(...)`; add `build_round_scene_draw_list(...)`; flip `draw_scene` to blit the scene + keep halo overlay.
- **Maybe add** a small `companion`-side helper module if `app.rs` grows too large (note as a concern; don't pre-split).
- **Modify** `src/dev_preview/` (scenarios + a round-scene producer) — add ONE additive frame serializing a round-viewport `SceneDrawList` (content coverage). Do NOT touch the existing round-commands path.

---

### Task 1: `appkit_blit_draw_list` — AppKit blitter for a `SceneDrawList`

A faithful generalization of the proven `draw_pet_art_block` (`app.rs:418`) to blit ANY `SceneDrawList`.

**Interfaces:** `fn appkit_blit_draw_list(list: &SceneDrawList, grid_cols: u16, grid_rows: u16, cell_w: f64, cell_h: f64, origin_x: f64, origin_y: f64)` — assumes the caller has already installed the aperture clip (as `draw_scene` does today).

- [ ] **Step 1:** Implement per-cell draw, mirroring `draw_pet_art_block`'s cell→pixel math (`app.rs:418-460`): for each `DrawCell` (cells are already z-ordered — draw in list order):
  - pixel: `px = origin_x + cell.col as f64 * cell_w`, `py = origin_y - (cell.row + 1) as f64 * cell_h` (AppKit Y-up, same as `draw_pet_art_block`).
  - if `cell.bg.is_some()`: fill `NSBezierPath::bezierPathWithRect(NSRect { (px, py), (cell_w, cell_h) })` with `ns_color(bg)`.
  - if `cell.glyph.is_some()`: `attributed_pet_glyph(glyph, font_size, &fg_round_color)` (reuse `app.rs:534`), bold weight if `cell.bold`, `.drawAtPoint(NSPoint(px, py))`.
  - `font_size` is derived from `cell_h` (the caller passes the measured cell size; `font_size` is the size used to measure `cell_w`/`cell_h`). Thread `font_size` in as a param too.
- [ ] **Step 2:** Unit-test the pure parts you CAN test without a live view: e.g. a `cell_to_point(col, row, cell_w, cell_h, origin) -> (f64,f64)` helper with assertions (the AppKit draw calls themselves can't be unit-tested — note that). Keep the AppKit calls in a thin wrapper.
- [ ] **Step 3:** Gate (compile + cargo test + clippy + fmt). Commit: `feat(companion): appkit_blit_draw_list — blit a SceneDrawList to AppKit`.

---

### Task 2: Round-viewport scene production + additive content golden

**Interfaces:** `fn build_round_scene_draw_list(vm: &WatchViewModel, now, view_w_pt: f64, view_h_pt: f64) -> RoundSceneRender` where `RoundSceneRender { list: SceneDrawList, grid_cols, grid_rows, cell_w, cell_h, font_size, origin_x, origin_y }`.

- [ ] **Step 1:** Pick the cell grid: measure `"M"` at a chosen `font_size` (start with the same fit logic as `draw_pet_art_block` but for a grid, e.g. target ~14–18 rows tall so the pet (10 rows) is a sensible fraction); `grid_cols = floor(view_w / cell_w)`, `grid_rows = floor(view_h / cell_h)`. **DOCUMENT the chosen default grid + font in a comment as TUNABLE (Drew's aesthetic call).** A reasonable starting point given the grounding: aim for the pet (PET_W=13) to be ~⅓ of the width, so ~36–45 cols. Center the grid: `origin_x = (view_w - grid_cols*cell_w)/2`, `origin_y` placing the grid centered.
- [ ] **Step 2:** Build the scene: `area = Rect::new(0,0,grid_cols,grid_rows)`; `(wx,fc) = resolve_wander_offset(vm, now, grid_cols)` → patch a `Cow`/clone vm; `ctx = RenderContext::new(ColorCapability::Truecolor)` (live clock, or pass `now`); `layout = PetScene::compute_layout(area, &vm, &ctx)`; `model = PetSceneModel::build(&vm, now, Truecolor)`; `list = render_pet_to_draw_list(&model, &vm, &layout, now, &ctx)`. Return the `RoundSceneRender`.
- [ ] **Step 3 (additive golden — content coverage):** Add ONE dev-preview frame that serializes a round-viewport `SceneDrawList` for a fixed fixture + fixed clock (deterministic) — e.g. a `*.round-scene.json` artifact listing the cells (row/col/glyph/fg/bg/bold). This golden-covers the SCENE CONTENT the companion will blit (NOT the AppKit pixels). Register it in `scenarios.rs` + assert in `tests/dev_preview.rs`. Additive — existing goldens unchanged.
- [ ] **Step 4:** Gate (incl. the new golden frame deterministic + existing 46 byte-stable). Commit: `feat(companion): build_round_scene_draw_list + round-scene content golden`.

---

### Task 3: Flip `draw_scene` to the shared scene (VISUAL — held for review)

- [ ] **Step 1:** In `draw_scene` (`app.rs:313`): keep the aperture clip install. Replace the `build_draw_commands` iteration that draws Background + RoomGlyph + PropGlyph + PetGlyph with: `let r = build_round_scene_draw_list(vm, now, bounds.w, bounds.h); appkit_blit_draw_list(&r.list, ...);`. KEEP the halo: still call `derive_round_scene_model` + `layout_round_scene` + `build_draw_commands`, but draw ONLY the `Halo`/`Trouble` commands after the scene blit (filter the command list by kind). Keep the dim overlay last.
- [ ] **Step 2:** Confirm it compiles (`cargo build`) and the full gate passes (watch byte-stable, no test regression). The AppKit visual is NOT test-verifiable.
- [ ] **Step 3:** Commit: `feat(companion): render the shared SceneDrawList in the round view (VISUAL — review before merge)`.
- [ ] **Step 4 (handoff, not a code step):** The controller writes a visual-review handoff: how to run the companion, the tunable choices made (cell grid, font size, origin), and what to look for (pet readable? habitat too busy cropped to the circle? halo placement? colors?).

---

## Self-Review

**Spec coverage (Track 3, companion):** companion builds + blits the shared `SceneDrawList` (Tasks 1-2-3); halo kept; debts resolve via parameterization (round area + Truecolor); old round path + goldens untouched (deferred retirement); additive content golden added (Task 2). ✓

**The verification gap (explicit):** Tasks 1-2 are fully verifiable (compile, cargo test, the content golden). Task 3's live AppKit look is NOT auto-verifiable — held for Drew's visual review. The content golden verifies WHAT cells the companion blits; only the pixel rendering is unverified.

**Risks:** (1) AppKit objc2 draw code can compile + pass tests but render wrong — Task 3 flagged visual-unverified. (2) The cell-grid/font choice is an aesthetic guess — documented as tunable. (3) Full habitat cropped to a small circle may look cluttered vs the current minimal porthole — that's the core thing for Drew to judge; the old path is one revert away (branch unmerged). (4) `app.rs` may grow large — note as a concern, don't pre-split.

**Out of scope (follow-up plan):** retiring `derive_round_scene_model`/`build_draw_commands`/the dev-preview round-commands contract; migrating dev-preview's round path to `render_pet_to_draw_list`; the rectangular screen-window surface; menubar; the `EffectState`/`ColorCapability` architectural cleanup (functionally resolved via parameterization; the `SurfaceStyle`-purity refactor remains optional).
