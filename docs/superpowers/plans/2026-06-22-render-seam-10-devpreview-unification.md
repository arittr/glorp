# Render Seam — Plan 10: Dev-Preview Unification + Retire the Dead Round Path

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the dev-preview ROUND preview render through the shared seam (`build_round_scene_draw_list` + `presentation::rasterize`) so it reflects what the live companion actually shows, then retire the now-dead old round path (`build_draw_commands`' pet/room/prop emission, the `PetGlyph`/`RoomGlyph`/`PropGlyph` kinds, the preview painters, and the `round-commands`/`round-layout` preview artifacts).

**Architecture:** Since Plan 06 the companion renders via `build_round_scene_draw_list` (the full scene), but the dev-preview round preview still renders via the OLD `derive_round_scene_model` → `build_draw_commands` → `paint_commands` path — so the design-review preview is stale (it shows a render the companion abandoned). This plan migrates the preview to the seam: `build_round_scene_draw_list(vm, now, w, h)` → `rasterize` → `PreviewCell` grid (aperture-masked), with the halo/trouble beads still overlaid from `build_draw_commands`' `Halo`/`Trouble` commands (preserving the `round-helper-trouble`/`round-active-pulse` review frames). Then it deletes the now-dead pet/room/prop command emission + painters + artifacts. `build_draw_commands` keeps emitting `Background`/`Halo`/`Trouble` (the companion + preview-overlay still need them); `derive_round_scene_model`/`RoundSceneModel`/`layout_round_scene` stay (the companion uses them).

**Tech Stack:** Rust; `src/round/{preview,draw,model}.rs`; `src/dev_preview/{round,contract,export,scenarios}.rs`; `src/presentation/rasterize.rs`; `tests/dev_preview.rs`.

This is **Plan 10** — spec Track 7 (dev-preview unification + dead-scaffolding cleanup). It is the final cleanup of the old round path. **Unlike Plans 05-08 this INTENTIONALLY changes dev-preview output** (the 7 round frames' render + the removal of two artifact types + a manifest schema bump) — those changes are deliberate, not regressions.

## Global Constraints

- **The COMPANION must not change.** `src/companion/app.rs` and `build_round_scene_draw_list` stay byte-stable; the companion still calls `derive_round_scene_model` + `layout_round_scene` + `build_draw_commands` for `Background`/`Halo`/`Trouble` + the dim overlay. The watch (46 dev-preview goldens) + the `round_draw_list` insta snapshots stay BYTE-IDENTICAL.
- **`build_draw_commands` keeps `Background`/`Halo`/`Trouble`.** Only `PetGlyph`/`RoomGlyph`/`PropGlyph` emission is removed.
- **`derive_round_scene_model`/`RoundSceneModel`/`layout_round_scene` stay** (companion + preview-halo-overlay depend on them). The now-unread `RoundPetModel.palette` field may stay (harmless) — do NOT churn the struct.
- **`src/pet/render.rs`/`art.rs` FROZEN; `presentation/` agnostic.**
- **Per-task gate (full suite, EXIT STATUS not `| tail`):** `cargo test` (0 FAILED) + `cargo test --features dev-preview --test dev_preview` + `cargo clippy --all-targets --all-features -- -D warnings` (the dead-code completeness oracle for Task 3) + `cargo fmt --check`. Watch goldens + `round_draw_list` snapshots byte-identical every task.
- Commit per task. **Byte-stable plan — safe to auto-merge** (the dev-preview round-frame render change is the only output change, verified via the dev-preview structural tests; no live product surface changes).

## File Structure

- **Modify** `src/round/preview.rs` — `render_round_preview_frame_from_vm`: render pet+habitat via the seam; keep halo/trouble overlay; drop the pet/room/prop painters.
- **Modify** `src/dev_preview/{contract,export,scenarios}.rs` — retire the `round-commands` + `round-layout` artifacts + plumbing; bump schema.
- **Modify** `src/round/draw.rs`, `src/round/model.rs` (minimal) — delete dead pet/room/prop command emission + `RoundDrawKind` variants.
- **Modify** `tests/dev_preview.rs` — delete/adapt the affected artifact + render tests.

---

### Task 1: Render the round preview via the seam (keep halo/trouble overlay)

**Files:** Modify `src/round/preview.rs`; adapt the render-content tests in `tests/dev_preview.rs`.

- [ ] **Step 1: Swap the pet+habitat render.** In `render_round_preview_frame_from_vm` (`preview.rs:10-50`), replace the pet/room/prop portion: build `let list = crate::round::scene::build_round_scene_draw_list(vm, now, width, height);` then `let grid = crate::presentation::rasterize(&list, width, height);`. Convert `grid` (`Vec<Vec<RasterCell>>`) → the `Vec<PreviewCell>` slice, applying the aperture mask EXACTLY as `blank_cells` does today (`preview.rs:52-71`): for each `(col,row)`, `outside_aperture = !aperture.contains(col as f32, row as f32)`; masked cells get `symbol=" "`, `fg=None`, `bg=None`; in-aperture cells copy `glyph`/`fg`/`bg` from the `RasterCell`. Then `mark_continuations` as today.
- [ ] **Step 2: Keep the halo/trouble overlay.** Still call `derive_round_scene_model` + `layout_round_scene` + `build_draw_commands`, but paint ONLY the `Halo`/`Trouble` commands into the cells via the existing `paint_labeled_command` (so `round-helper-trouble`/`round-active-pulse` still show their beads). Drop the `Background`/`PetGlyph`/`RoomGlyph`/`PropGlyph` painting from the preview (Background is now the rasterized biome-wash bg; pet/room/prop come from the seam). Remove `paint_pet_art_command` + the pet/room/prop arms of `paint_commands` if they become unused (Task 3 owns the command-side deletion; here just stop calling them — if that orphans them, either `#[allow(dead_code)]` + "removed in Task 3" or, cleaner, delete the now-unused preview-side painters here and leave the `build_draw_commands` emission for Task 3).
- [ ] **Step 3: Adapt the render-content tests.** `dev_preview_round_aperture_corners_are_masked` (`tests/dev_preview.rs:1810`) must still pass (masking preserved). `dev_preview_round_glitch_and_crystal_differ_by_symbols` (`:1837`) still tests cells.json — re-validate it passes with seam-rendered glyphs (the dialects still differ in the seam render; if the test pins exact symbols that changed, update to assert the dialects DIFFER, not specific glyphs). Round frames have NO committed insta/byte snapshots — the structural tests are the gate.
- [ ] **Step 4: Gate.** Full gate; watch 46 + `round_draw_list` snapshots byte-identical; the 7 round frames now render the seam scene (verify via the structural tests). Commit: `feat(dev-preview): round preview renders the shared scene (was stale old-path render)`.

---

### Task 2: Retire the `round-commands` + `round-layout` preview artifacts + bump schema

**Files:** Modify `src/dev_preview/{contract,export,scenarios}.rs`, `src/round/preview.rs`; `tests/dev_preview.rs`; `CLAUDE.md`.

- [ ] **Step 1: Stop attaching the artifacts.** In `preview.rs` remove `frame.contract.round_commands = ...` (`:44-48`) and `frame.contract.round_layout = ...` (`:41-43`). The preview no longer needs `layout_round_scene` for an artifact (it still calls it for the halo overlay in Task 1 — keep that call).
- [ ] **Step 2: Delete the artifact types + plumbing.** Remove `PreviewRoundCommandsArtifact` + `from_commands` + `round_draw_kind_name` (`contract.rs:416-519`); `PreviewFrameContract.round_commands` (`contract.rs:21`); `PreviewRoundLayoutArtifact` (if now unused — check); `PreviewScenarioFiles.round_commands`/`round_layout` (`export.rs:95`); `ArtifactType::RoundCommands`/`RoundLayout` (`export.rs:157`); `round_commands_path`/`round_layout_path` (`scenarios.rs:1709-1710`); the write/serialize paths (`scenarios.rs:171-174, 617-621, 707-713`); the HTML linker (`export.rs:466-469`) + Markdown linker (`export.rs:313-316`).
- [ ] **Step 3: Bump the manifest schema.** `SCHEMA_VERSION` 4 → 5 (`export.rs:12`); update the asserts (`tests/dev_preview.rs:294, 703, 1645, 1471`). Update `CLAUDE.md`'s stale "manifest schema version is 3" → "5".
- [ ] **Step 4: Delete/adapt the artifact tests.** DELETE `dev_preview_round_writes_layout_and_command_artifacts` (`:1675`) and `dev_preview_round_artifacts_match_scene_semantics` (`:1732`). CHANGE `dev_preview_review_surfaces_link_typed_artifacts` (`:1774`) — remove the `round-commands`/`round-layout` link assertions. Keep `{id}.scene.json` (unchanged — `PreviewSceneArtifact::from_round_scene` stays).
- [ ] **Step 5: Gate.** Full gate; the manifest no longer emits `round-commands`/`round-layout`; `.scene.json` unchanged; watch goldens byte-identical. Commit: `refactor(dev-preview): retire round-commands/round-layout artifacts; bump manifest schema to 5`.

---

### Task 3: Delete the dead `build_draw_commands` pet/room/prop emission + `RoundDrawKind` variants

**Files:** Modify `src/round/draw.rs` (+ `model.rs` only if a derivation is now provably dead — prefer leaving `RoundSceneModel` intact per constraints).

- [ ] **Step 1: Slim `build_draw_commands`** (`draw.rs:68-118`): delete the `push_room_glyph_commands` call (`:82`), the `push_pet_art_command` call (`:83`), and the `prop_anchors` → `PropGlyph` loop (`:84-95`). Keep the `Background` push (`:72-81`) and the `halo_anchors` → `Halo`/`Trouble` loop (`:96-118`). Delete the now-orphaned `push_room_glyph_commands` (`:125-187`) and `push_pet_art_command` (`:189-209`) fns, and any preview painter still orphaned (`paint_pet_art_command`, the pet/room arms of `paint_commands`, `command_color`'s dead arms).
- [ ] **Step 2: Remove the dead `RoundDrawKind` variants** `PetGlyph`/`RoomGlyph`/`PropGlyph` (now no producers/consumers). Fix any `match` that becomes non-exhaustive (the preview `paint_commands` + `contract` matches were handled in Tasks 1-2; the companion never matched them).
- [ ] **Step 3: Delete the dead unit tests** in `draw.rs` (`:212-312`) that assert `PetGlyph`/`RoomGlyph` emission.
- [ ] **Step 4: Gate.** `cargo clippy --all-targets --all-features -- -D warnings` is the completeness oracle — passes only if removal is complete and nothing live was cut. Watch 46 + `round_draw_list` snapshots byte-identical; companion unaffected. Commit: `refactor(round): delete dead pet/room/prop draw-command emission + RoundDrawKind variants`.

---

## Self-Review

**Spec coverage (Track 7):** dev-preview round preview unified onto the seam (Task 1); dead scaffolding deleted (Tasks 2-3); only `Background`/`Halo`/`Trouble` survive in `build_draw_commands`. ✓ Resolves the "two parallel scene derivations" risk flagged since Plan 06.

**Intentional changes (NOT regressions):** the 7 round frames render the seam scene (Task 1); `round-commands`/`round-layout` artifacts removed + schema 4→5 (Task 2); `PetGlyph`/`RoomGlyph`/`PropGlyph` deleted (Task 3). The dev-preview structural tests (aperture mask, dialect-differ) are the gate — round frames have no committed byte/insta snapshots.

**Byte-stable invariants (verified every task):** the companion, the watch (46 goldens), and the `round_draw_list` snapshots are untouched. The companion's `build_draw_commands` usage (Background/Halo/Trouble) is preserved.

**Placeholder scan:** the DELETE/KEEP/CHANGE surface is exact (from grounding, with file:line). Task 3's clippy gate enforces completeness.

**Risks:** (1) Task 1's seam-render of the round preview is a content change — the structural tests (not byte snapshots) must be re-validated, and `dev_preview_round_glitch_and_crystal_differ_by_symbols` may need its assertion generalized (differ, not exact glyphs). (2) Non-exhaustive `match` after removing `RoundDrawKind` variants — Task 3 fixes each. (3) The halo overlay must still paint into the seam-rasterized grid (Task 1) — verify `round-helper-trouble` still shows its bead. (4) Schema bump may surprise external manifest consumers — bumping to 5 is the signal.

**Out of scope:** the screen-window surface (Plan 09, skipped — addable later); pruning the now-dead `RoundPetModel.palette` field (harmless, leave it); the `SurfaceStyle`-into-seam refactor (only needed if the menubar wants source-accent back). With this plan the render seam re-arch is complete: one `render_pet_to_draw_list` → `SceneDrawList` producer feeds watch (ratatui blit), companion (AppKit blit), menubar (NSAttributedString), and dev-preview (rasterize) — no parallel render paths remain.
