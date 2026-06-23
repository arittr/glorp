# Render Seam — Plan 08: Menubar Popover Renders the Shared Scene

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the macOS menubar popover render the SAME full habitat scene as the watch + companion, by building a menubar-viewport `SceneDrawList` via the shared `render_pet_to_draw_list` and converting it to an `NSAttributedString` (the popover's text surface). The pet gains its full habitat in the popover; the stats block below stays as-is.

**Architecture:** The menubar popover (`src/menubar/render.rs`) is an `NSTextView`-backed attributed-string surface (`POPOVER_COLUMNS=36` × `POPOVER_ROWS=22` at 13pt mono). Today `render_pet_block` builds attributed runs directly from `vm.pet_art`/`vm.pet_spans` (pet-only). This plan: (1) a **`SceneDrawList` → `NSAttributedString` blitter** that rasterizes the ordered cells into a dense grid and emits row-major styled runs (fg = foreground color, bg = background color, bold = font weight); (2) flip `render_pet_block` to build a menubar-viewport scene via `build_round_scene_draw_list(vm, now, cols, rows)` (the pure producer the companion already uses) and blit it. The stats block (`render_stats_block`) is unchanged. The status-bar button title (`· PetName ·`) is unchanged and out of scope.

**Tech Stack:** Rust + objc2 AppKit (`NSAttributedString`, `NSMutableAttributedString`, `NSColor`, `NSFont` weights); `presentation::{SceneDrawList, DrawCell}`; `round::scene::build_round_scene_draw_list`; `tui::panels::pet::draw::render_pet_to_draw_list`.

This is **Plan 08** — spec Track 5 (menubar adapter). Per Drew's decision the menubar popover now shows the FULL habitat (a deliberate upgrade from the spec's original `Detail::Minimal`). **VISUAL CHANGE with no pixel oracle** — implemented + tested + HELD on the branch for Drew's visual review, NOT auto-merged.

## Global Constraints

- **Watch + companion must NOT regress:** `render_pet_to_draw_list`, `build_round_scene_draw_list`, `PetSceneModel`, and all shared/round code stay byte-stable (watch 46 dev-preview goldens + the `round_draw_list` snapshots unchanged). The menubar is the only behavior change. Reuse the shared producer — do NOT modify it.
- **`src/pet/render.rs`/`art.rs` FROZEN; `src/presentation/` agnostic** (no ratatui/AppKit in the types).
- **Stats block + status-bar title unchanged.** Only the pet region of the popover changes (pet-only → full scene). `render_stats_block` and `status_item_title` are untouched.
- **Privacy:** the menubar is a privileged local surface (source names/exact counts allowed in the stats block, as today) — unchanged. The scene cells themselves carry no source names (abstract pet/habitat), so no new privacy surface.
- **Per-task gate (full suite, EXIT STATUS not `| tail`):** `cargo build` + `cargo test` (0 FAILED) + `cargo test --features dev-preview --test dev_preview` (46 + round snapshots unchanged) + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo fmt --check`. (These verify compile + no watch/companion regression + the new content lock. They do NOT verify the popover pixels — that's Drew's visual review.)
- Commit per task. **DO NOT MERGE** — hold for visual review.

## File Structure

- **Modify** `src/menubar/render.rs` — add the `SceneDrawList` → `NSAttributedString` blitter; flip `render_pet_block` to build + blit the menubar-viewport scene.
- **Reuse (do not modify)** `src/round/scene.rs::build_round_scene_draw_list` (pure, surface-agnostic despite the name; the "round" name is a wart to rename in the Plan 10 cleanup — note, don't rename here).

---

### Task 1: `SceneDrawList` → `NSAttributedString` blitter

**Files:** Modify `src/menubar/render.rs`.

**Interfaces:** `fn scene_draw_list_to_attributed(list: &SceneDrawList, cols: u16, rows: u16, font: &NSFont, bold_font: &NSFont) -> Retained<NSMutableAttributedString>` (exact AppKit types per the file's existing helpers — match `render_pet_block`'s current attributed-string construction).

- [ ] **Step 1: Rasterize the draw list into a dense grid (TDD on the pure part).** Write `fn rasterize(list: &SceneDrawList, cols: u16, rows: u16) -> Vec<Vec<RasterCell>>` where `RasterCell { glyph: char, fg: Option<Rgb>, bg: Option<Rgb>, bold: bool }`, initialized to space/none, then apply each `DrawCell` in list order (later overwrites earlier — same z-order as `blit_draw_list`): set `glyph`/`fg`/`bold` for glyph cells, set `bg` for bg cells, leaving other fields intact (a glyph cell with `bg: None` keeps the bg a prior biome-wash cell set — same sparse semantics as the watch blitter). Cells out of `[0,cols)×[0,rows)` are skipped. Unit-test it: a bg-only cell then a glyph cell at the same coord → the rasterized cell has the glyph + fg AND the prior bg; out-of-bounds skipped.
- [ ] **Step 2: Grid → attributed string (row-major runs).** For each row, coalesce consecutive cells with identical (fg, bg, bold) into one run; append `NSAttributedString` with `NSForegroundColorAttributeName` (fg → `NSColor`, default to the popover's text color if `None`), `NSBackgroundColorAttributeName` (bg → `NSColor`, omitted if `None`), and `NSFontAttributeName` (bold_font if `bold`, else font). Join rows with `\n`. Reuse the file's existing `NSColor`/font construction helpers (match `render_pet_block`). Out-of-band: keep the run-coalescing simple and correct over clever.
- [ ] **Step 3: Deterministic content lock.** Add a test that locks the attributed-string STRUCTURE (or the rasterized grid) for a fixed `SceneDrawList` (build one via `build_round_scene_draw_list(WatchViewModel::fixture(), fixed_now, small cols, rows)`) — assert the grid's glyph rows + a sample of fg/bg, OR snapshot the rasterized grid. This locks the cells→grid conversion deterministically (the `NSAttributedString` object itself isn't easily snapshotted; lock the `rasterize` output). Existing goldens unchanged.
- [ ] **Step 4: Gate.** Full gate (EXIT STATUS). The blitter is not yet called by `render_pet_block` — if clippy flags it dead, `#[allow(dead_code)]` + comment "wired in Task 2". Commit: `feat(menubar): SceneDrawList → NSAttributedString blitter + rasterize`.

---

### Task 2: Flip `render_pet_block` to the shared scene (VISUAL — held for review)

**Files:** Modify `src/menubar/render.rs`.

- [ ] **Step 1: Choose the menubar scene grid (TUNABLE — document it).** The popover is `POPOVER_COLUMNS=36` wide. Pick the pet-region height (e.g. `MENU_SCENE_ROWS ≈ 14`, leaving rows for the stats block within `POPOVER_ROWS=22`; if the scene + stats exceed 22, either grow the popover height or trim scene rows). Use `cols = POPOVER_COLUMNS`. **Document the chosen `MENU_SCENE_ROWS` as a tunable aesthetic default (Drew's call), and note that 36×14 trips `compact=true` (so activity budget + Orbit→Glow are trimmed) — same parameterization as the companion.**
- [ ] **Step 2: Build + blit the scene.** In `render_pet_block`, replace the direct `vm.pet_art`/`vm.pet_spans` attributed-run loop with: `let list = crate::round::scene::build_round_scene_draw_list(vm, now, POPOVER_COLUMNS, MENU_SCENE_ROWS); let attr = scene_draw_list_to_attributed(&list, POPOVER_COLUMNS, MENU_SCENE_ROWS, &font, &bold_font);` and append that to the popover's attributed string in the pet region (where the old pet art went). Keep the trailing newline / spacing that separates pet from stats. The `now` should be the same instant the rest of the render uses (thread it from the caller; if `render_pet_block` doesn't currently have `now`, pass it in).
- [ ] **Step 3: Remove the old pet-art attributed-run code** that's now superseded (the bespoke `vm.pet_art`/`vm.pet_spans` walk in `render_pet_block`, and `menubar_resolve` IF it's now only used there — check; if `render_stats_block` or the color path still needs it, keep it). Remove any now-dead imports. Clippy must stay clean.
- [ ] **Step 4: Gate + hold.** `cargo build`; full gate (watch 46 + round snapshots byte-identical — confirms shared code untouched; no test regression). The popover pixels are NOT test-verifiable. Commit: `feat(menubar): popover renders the shared habitat scene (VISUAL — review before merge)`.
- [ ] **Step 5 (handoff, not a code step):** controller writes the visual-review handoff: how to run (`cargo run -- menubar`, click the status item), the tunable choices (`MENU_SCENE_ROWS`, grid), and what to look at (pet scale in 36-wide, habitat density at `compact`, scene/stats spacing, colors via `MENU_STYLE` source-accent vs the new full scene — note: the scene's pet colors now come through `render_pet_to_draw_list`/`PetSceneModel`, which may differ from the old `MENU_STYLE` source-accent path; flag for Drew whether the menubar should keep source-accent tinting).

---

## Self-Review

**Spec coverage (Track 5):** the menubar popover renders through the shared `SceneDrawList` seam (Tasks 1-2); stats block + status title unchanged; full habitat per Drew's decision (supersedes the spec's original `Detail::Minimal`). ✓

**The verification gap (explicit):** Task 1 (blitter + rasterize) is fully verifiable (compile, unit tests, content lock). Task 2's popover pixels are NOT auto-verifiable — held for Drew's visual review. The scene CONTENT is deterministic (locked via `build_round_scene_draw_list` + the rasterize lock); only the `NSAttributedString` rendering in the live popover is unverified.

**Open question for Drew's review (Step 5):** the old menubar pet used `MENU_STYLE` (source-accent: recolor accent/particle to the work-identity color). The shared `render_pet_to_draw_list` resolves pet colors through `PetSceneModel`/the watch path (which does NOT apply `MENU_STYLE`'s source-accent). So the menubar pet's colors may change. If Drew wants source-accent kept, that needs the `SurfaceStyle` threaded into the seam (deferred design) — flag it; for v1 the full-scene colors are what `render_pet_to_draw_list` produces.

**Risks:** (1) AppKit attributed-string construction can compile + pass tests but render wrong — Task 2 held for visual review. (2) The scene/stats row budget in a 22-row popover — documented tunable; may need the popover to grow. (3) `compact=true` at 36×14 trims the scene (same as companion). (4) Color path change (the `MENU_STYLE` source-accent question above).

**Out of scope (later):** rename `build_round_scene_draw_list` → surface-neutral (Plan 10 cleanup); threading `SurfaceStyle` into the seam (if source-accent must be kept); the status-bar button token-meter (the `app.rs:231` "v2 Little-Snitch-style meter" TODO); screen-window (Plan 09); dev-preview unification (Plan 10).
