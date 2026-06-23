# Render Seam — Plan 05: SceneDrawList + render(viewport); Watch Becomes a Blitter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a resolved, backend-agnostic `SceneDrawList` (ordered `DrawCell`s) and migrate every watch pet-panel render pass to produce cells into it, so `PetPanel::render` ends as a thin "build scene → `render_pet_to_draw_list(WATCH_STYLE, area)` → blit" adapter. Zero visible change (dev-preview goldens byte-stable). This is the seam companion (Plan 06) blits into.

**Architecture:** `SceneDrawList { cells: Vec<DrawCell> }` is an **ordered** list; blitting cells in order reproduces the current pass-by-pass overdraw (later passes overwrite earlier). `DrawCell { row, col, glyph: Option<String>, fg: Option<Color>, bg: Option<Color>, add_modifier: Modifier }` — `bg`-only cells (biome wash, contact shadow), sparse fg-glyph cells (pet, decoration glyphs — `bg: None` lets the habitat bg show through), and full cells (speech bubble). The blitter applies each cell to the ratatui `Buffer` WITHOUT resetting it (preserves bg from earlier cells), exactly mirroring today's `set_char` + `set_style`. Each pass is migrated incrementally — converted from "paint to `buf`" to "return `Vec<DrawCell>`, blit at the same z-position" — keeping every step byte-stable; the final task concatenates all passes into one list produced by a single reusable `render_pet_to_draw_list`.

**Tech Stack:** Rust; ratatui `Buffer`/`Cell`/`Color`/`Modifier`; the existing pass helpers in `src/tui/panels/pet/` + `src/tui/room.rs` + `src/tui/component/habitat_props.rs`; `presentation::{PetSceneModel, EffectState}`.

This is **Plan 05** of the render-seam re-arch — spec `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`, **Track 3** (the `SceneDrawList`/`render(viewport)` half; companion adapter is Plan 06). The pass ORDER in `PetPanel::render` today (the z-order to preserve) is: biome-wash → room-glyphs → ambient → motes → activity → props(Background/Behind) → contact-shadow → pet-art+speech → performance-cue → props(Foreground).

## Global Constraints

- **`src/pet/render.rs` and `src/pet/art.rs` are FROZEN.**
- **`src/presentation/` must NOT import `tui::component::TargetPath`.** `SceneDrawList`/`DrawCell` live in `presentation` and are backend-agnostic (no ratatui in the TYPE — store `fg`/`bg` as `crate::pet::palette::Rgb` (or an `Option<Rgb>`), NOT ratatui `Color`; the watch blitter converts `Rgb`→`Color::Rgb` at blit time, so AppKit can consume the same list in Plan 06).
- **dev-preview goldens BYTE-STABLE** at every task — the pet's on-screen cells (symbol/fg/bg/modifiers) must not change. Goldens are the oracle.
- **Ordered preservation:** each migrated pass must blit at the SAME point in the pass sequence it painted at, so z-order/overdraw is identical.
- **Per-task gate (full suite, EXIT-STATUS based, never `| tail`):** `cargo test` (0 FAILED) AND `cargo test --features dev-preview --test dev_preview` AND `cargo clippy --all-targets --all-features -- -D warnings` AND `cargo fmt --check`.
- **Commit per task.**

## File Structure

- **Create** `src/presentation/draw_list.rs` — `SceneDrawList`, `DrawCell` (backend-agnostic; `Rgb` colors).
- **Modify** `src/presentation/mod.rs` — `pub mod draw_list;` + re-exports.
- **Create** `src/tui/panels/pet/blit.rs` — `blit_draw_list(buf: &mut Buffer, list: &SceneDrawList)` (the watch ratatui adapter; `Rgb`→`Color::Rgb`).
- **Modify** the pass helpers (`grounding.rs`, `ambient.rs`, `room.rs`, `props.rs`, `performance.rs`, `art_lines.rs`, `habitat_props.rs`) — add `*_cells(...) -> Vec<DrawCell>` variants (or change the existing fns to return cells) alongside/replacing the paint fns.
- **Modify** `src/tui/panels/pet.rs` (`PetPanel::render`, `render_pet_inside`) — replace each paint with build-cells + blit; finally collapse to one `render_pet_to_draw_list` + one blit.
- **Create (Task 6)** `src/tui/panels/pet/draw.rs` (or in `pet.rs`) — `pub(crate) fn render_pet_to_draw_list(scene_model, vm, area, now, color_capability) -> SceneDrawList` (the reusable producer companion calls in Plan 06).

---

### Task 1: `SceneDrawList`/`DrawCell` model + watch blitter (proven on biome wash)

Establish the model + blitter, and migrate the simplest pass (biome wash, bg-only, no RNG/now) to prove byte-stability.

**Files:** Create `src/presentation/draw_list.rs`, `src/tui/panels/pet/blit.rs`; modify `src/presentation/mod.rs`, `src/tui/panels/pet.rs`, `src/tui/panels/pet/grounding.rs`.

**Interfaces:**
- Produces: `pub struct DrawCell { pub row: u16, pub col: u16, pub glyph: Option<String>, pub fg: Option<Rgb>, pub bg: Option<Rgb>, pub add_modifier: CellModifier }`; `pub struct SceneDrawList { pub cells: Vec<DrawCell> }` with `push`/`extend`; `pub(crate) fn blit_draw_list(buf: &mut Buffer, list: &SceneDrawList)`.
- `CellModifier`: a small backend-agnostic modifier (e.g. `bitflags`/`enum` with `BOLD`); the blitter maps it to ratatui `Modifier`. (Only BOLD is used today — keep it minimal.)

- [ ] **Step 1: Define the types + blitter with unit tests (TDD).** Test `blit_draw_list` against a hand-built `SceneDrawList`: assert a bg-only cell sets only bg (symbol/fg untouched), a sparse glyph cell sets symbol+fg and LEAVES bg intact (pre-seed the buffer cell's bg, blit a glyph cell, assert bg survived — this is the load-bearing "sparse" behavior), and a BOLD cell adds the modifier. RED→GREEN. `blit_draw_list` must mirror exactly: `if let Some(g)=glyph { cell.set_symbol(g) }; if let Some(fg) { cell.set_fg(Color::Rgb(..)) }; if let Some(bg) { cell.set_bg(Color::Rgb(..)) }; cell.modifier.insert(mods)` — NO `cell.reset()`.
- [ ] **Step 2: Migrate biome wash.** Add `grounding::biome_wash_cells(habitat, biome) -> Vec<DrawCell>` returning bg-only cells (every habitat cell with `bg = biome_wash_color`/`biome_floor_wash_color`, `glyph: None`, `fg: None`) — same color math as `paint_biome_wash`. In `PetPanel::render`, replace the `paint_biome_wash` call with `blit_draw_list(buf, &SceneDrawList{cells: biome_wash_cells(...)})` at the SAME position. Keep `paint_biome_wash` if other callers exist; otherwise remove it.
- [ ] **Step 3: Goldens byte-stable + full gate.** `cargo test --features dev-preview --test dev_preview` byte-identical; full gate (exit-status). Commit: `feat: SceneDrawList/DrawCell + watch blitter; migrate biome wash`.

---

### Task 2: Pet body + speech → DrawCells (the entangled one)

Convert the Stage 1–7 pet-body pipeline (`render_pet_inside` + `art_lines.rs`) to produce `Vec<DrawCell>` and blit, byte-stable. This is the hard task — the goldens are the oracle for the twinkle/cursor/mirror/BOLD-eye/multi-width details.

**Files:** Modify `src/tui/panels/pet/art_lines.rs`, `src/tui/panels/pet.rs`.

**Interfaces:** `pub(super) fn pet_body_cells(pet_rect: Rect, lines: &[Line], ...) -> Vec<DrawCell>` and a speech variant; consumed by `render_pet_inside`.

- [ ] **Step 1: Extract a cell-producing variant of `render_pet_lines_sparse`.** Today (`art_lines.rs:16-42`) it walks `lines: &[Line]`, and for each non-space char does `cell.set_char(ch); cell.set_style(span.style)` at `(area.x + col, area.y + row)`. Write `pet_body_cells(area, lines) -> Vec<DrawCell>` that instead PUSHES a `DrawCell { row: area.y+row, col: area.x+col, glyph: Some(ch), fg: span.style.fg→Rgb, bg: None, add_modifier: BOLD if span.style has BOLD else empty }` for each non-space char (skip spaces, advance col by 1 — identical sparse logic). This reuses the existing `build_pet_lines` output (mirror, cursor-eye, twinkle, role styling all already baked into the `Line` spans), so the ONLY change is "push a cell" vs "write to buf". That keeps the entangled parts untouched.
- [ ] **Step 2: Speech bubble cells.** `render_speech_bubble` (`art_lines.rs:46-63`) uses `Paragraph` (writes ALL cells incl. spaces, with bg). Produce its cells explicitly: the padded `« text »` line → `DrawCell`s for every column in `scene.speech` (glyph = the char incl. spaces, fg = `droop_styles.pet_accent` fg, bg from the Paragraph's style if any). Verify against the speech-frame golden.
- [ ] **Step 3: Blit at position.** In `render_pet_inside`, replace `render_pet_lines_sparse(buf, pet_rect, &lines)` and the speech `Paragraph::render` with `blit_draw_list` of the produced cells, at the same sequence positions.
- [ ] **Step 4: Goldens byte-stable** (this is where twinkle/cursor/mirror/BOLD must match exactly — the existing `pet_renderer_roles_reach_tui_cells` test at `tui_render.rs:595` and the dev-preview pet frames are the oracles). Full gate. Commit: `feat: pet body + speech render to DrawCells`.

---

### Task 3: Grounding bg passes → DrawCells (contact shadow)

**Files:** Modify `src/tui/panels/pet/grounding.rs`, `pet.rs`.

- [ ] Convert `paint_contact_shadow`/`contact_shadow_cells` (`grounding.rs:31,89`) to return `Vec<DrawCell>` (bg-only cells at the feet-column positions, `bg = contact_shadow_color`, mirrored by facing, habitat-clipped — same as today). Blit at the same position (after props-behind, before pet). Byte-stable goldens. Commit: `feat: contact shadow renders to DrawCells`.

---

### Task 4: Glyph-collection decoration passes → DrawCells (uniform)

These passes already return `Vec<…Glyph>` with absolute `(col,row)` + color; converting is uniform: map each glyph to `DrawCell { row, col, glyph: Some(g.glyph), fg: Some(g.color→Rgb), bg: None, add_modifier: empty }`.

**Files:** Modify `room.rs` (`room_glyphs_for`), `ambient.rs` (`ambient_glyphs_for_phase`, `mote_glyphs_for`, `activity_glyphs_for`), `pet.rs`.

- [ ] For each of: room glyphs, ambient sky glyphs, motes, activity glyphs — add a `*_cells(...) -> Vec<DrawCell>` that maps the existing `Vec<Glyph>` (with its exclusion filtering + phase tint already applied) into `DrawCell`s, and replace the paint loop in `PetPanel::render` with `blit_draw_list` at the same position. (The RNG/now-seeding/exclusion logic is UNCHANGED — only the final write becomes a cell push.) Byte-stable goldens after EACH (commit per pass or per the group, your call — but verify goldens between). Commit: `feat: room/ambient/mote/activity glyphs render to DrawCells`.

---

### Task 5: Props + performance cues → DrawCells

**Files:** Modify `props.rs` (`render_prop_layers`/`render_prop_layer`), `performance.rs` (`apply_pet_performance_cues`), `pet.rs`.

- [ ] Convert prop-layer rendering: `habitat_props_for` already yields a `Vec<HabitatPropCell>` with resolved positions; map the per-layer filtered cells (with their reaction-glow styles) into `DrawCell`s. Blit Background/Behind before the pet, Foreground after — same z-order.
- [ ] Convert `apply_pet_performance_cues`: the single cue cell (`glyph` at `(pet_art.x+width/2, pet_art.y±1)`) → one `DrawCell`. Blit at its position (after the pet).
- [ ] Byte-stable goldens (note: the no-prop fixture means prop reactions aren't golden-covered — add a small `dev-preview` fixture or unit assertion WITH an earned prop to cover prop-cell + reaction rendering, per the Plan-04 lesson). Full gate. Commit: `feat: props + performance cues render to DrawCells`.

---

### Task 6: Unify into `render_pet_to_draw_list`; flip `PetPanel` to a pure blitter

**Files:** Create `src/tui/panels/pet/draw.rs` (or a section of `pet.rs`); modify `pet.rs` to its final adapter form.

- [ ] **Step 1:** Define `pub(crate) fn render_pet_to_draw_list(scene_model: &PetSceneModel, vm: &WatchViewModel, area: Rect, now, color_capability) -> SceneDrawList` that calls each pass's `*_cells` producer IN Z-ORDER and `extend`s one `SceneDrawList` (biome-wash → room → ambient → motes → activity → props-bg/behind → contact-shadow → pet+speech → performance → props-fg). This is the reusable producer companion calls in Plan 06.
- [ ] **Step 2:** Reduce `PetPanel::render` to: build `PetSceneModel`, compute layout, `let list = render_pet_to_draw_list(...)`, `blit_draw_list(buf, &list)` — ONE blit. Remove the now-dead per-pass paint calls and any orphaned `paint_*` fns (clippy dead-code gate enforces). Keep the `PetSceneLayout`/`compute_layout` geometry (positions still come from it).
- [ ] **Step 3:** Goldens byte-stable (the one-blit result must equal the per-pass-blit result, which equalled the original). Full gate. Confirm `pet.rs` line count dropped substantially (the structural-cap test `tests/pet_panel_structure.rs` should pass with room to spare). Commit: `refactor: PetPanel renders via one SceneDrawList blit; retire per-pass paint`.

---

## Self-Review

**Spec coverage (Track 3, the draw-list half):**
- `SceneDrawList`/`DrawCell` backend-agnostic (Rgb, no ratatui in the type): Task 1. ✓
- `render_pet_to_draw_list` reusable producer (companion consumes it Plan 06): Task 6. ✓
- Watch is a blitter; per-pass paint retired: Task 6. ✓
- Byte-stable at every task (ordered list = z-order; sparse glyph cells preserve bg): the blit contract (Task 1) + goldens. ✓
- Constraints: `render_pet`/`art.rs` frozen; `presentation/` no `tui::component::TargetPath` (and no ratatui in the type); gate via exit status. ✓
- Plan-04 prop-fixture gap addressed in Task 5 (prop reactions need golden coverage). ✓

**Placeholder scan:** The model + blit contract + pet-body conversion (Tasks 1–2) are fully specified. Tasks 3–5 are the SAME uniform transform (existing `Vec<Glyph>`/paint → `Vec<DrawCell>` + blit) applied per pass — concrete per-file, not re-deriving verbatim code that mirrors Task 1's established pattern. Each task's byte-stability is goldens-verified.

**Risk notes:** Task 2 (pet body) is the byte-stability crux — twinkle/cursor/mirror/BOLD/multi-width. Mitigation: reuse the existing `build_pet_lines` `Line` output unchanged and only change the final write (buf → cell), so the entangled logic is untouched. The `glyph: Option<String>` (not `char`) guards future wide glyphs. If any task's goldens drift, reconcile — never re-bake.

**Carried debts to retire here (per spec Track 3 notes):** Plan-02 `EffectState::from_vm` `ColorCapability` leak and Plan-04 `compact`/reacted-`life` resolve naturally as the passes move into `render(viewport)` where the per-surface viewport (and `SurfaceStyle`) are in scope; thread `compact` from the real `area`, never hardcode.

**Out of scope (Plan 06+):** the companion AppKit blitter, circular clip, halo overlay, privacy; reconciling the `tui::component::pet_scene::PetScene` (geometry) vs `presentation::PetSceneModel` name; the screen-window/menubar/dev-preview-unification surfaces.
