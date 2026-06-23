# Render Seam — Plan 07: Retire the Dead Companion Round-Rendering Path

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Remove the old AppKit pet/room/prop rendering code in `src/companion/app.rs` that Plan 06 made unreachable (the companion now blits the shared `SceneDrawList` via `appkit_blit_draw_list`, and `draw_scene` already filters its command loop to only `Halo`/`Trouble`). Pure dead-code removal — zero companion behavior change.

**Architecture:** After Plan 06, `draw_scene` draws: circular clip → Background oval (drawn directly) → `appkit_blit_draw_list` (habitat + pet) → a loop over `build_draw_commands` filtered to `Halo`/`Trouble` → dim overlay. The filter means `draw_command`'s `Background`/`PetGlyph`/`PropGlyph`/`RoomGlyph` arms (and everything they call) are live-but-unreachable. This plan inlines the `Halo`/`Trouble` oval fill into the loop, deletes `draw_command`, and deletes the now-orphaned helpers. The removed code is unreachable, so removal is byte-stable by construction.

**Tech Stack:** Rust + objc2 AppKit. Single file: `src/companion/app.rs`.

This is **Plan 07** — spec Track 3 cleanup. It retires ONLY the companion-side dead code. `build_draw_commands`/`RoundDrawCommand`/`derive_round_scene_model`'s pet/room/prop derivation STAY — they still back the round halo AND the dev-preview `round-commands.json` goldens (`src/round/preview.rs` serializes the full command set). Retiring those is owned by the later dev-preview-unification plan (Plan 09), after dev-preview migrates to `render_pet_to_draw_list`.

## Global Constraints

- **`src/round/draw.rs` MUST NOT change.** `build_draw_commands` keeps emitting the full command set (incl. `PetGlyph`/`RoomGlyph`/`PropGlyph`) — dev-preview's `round-commands.json` goldens depend on it byte-for-byte. Same for `derive_round_scene_model`.
- **Byte-stable everywhere:** watch dev-preview goldens (46) AND the round-commands.json goldens unchanged. The companion's live render is unchanged (only unreachable code is removed + the `Halo`/`Trouble` oval is inlined identically).
- **`src/pet/render.rs`/`art.rs` FROZEN.**
- **Per-task gate (full suite, EXIT STATUS not `| tail`):** `cargo test` (0 FAILED) + `cargo test --features dev-preview --test dev_preview` (all goldens unchanged) + `cargo clippy --all-targets --all-features -- -D warnings` (this is the completeness oracle — it fails if any LIVE symbol was removed, and catches any newly-orphaned dead code) + `cargo fmt --check`.
- Commit per task.

## File Structure

- **Modify** `src/companion/app.rs` — inline `Halo`/`Trouble` oval into `draw_scene`'s loop; delete `draw_command` + the orphaned helpers, structs, import, and dead tests listed below.

---

### Task 1: Inline `Halo`/`Trouble`, delete the dead old round-rendering code

**Files:** Modify `src/companion/app.rs`.

The `Halo` and `Trouble` arms of `draw_command` are identical (both fill an oval at `(command.x - radius, command.y - radius)` size `(radius*2, radius*2)` with `ns_color(&command.color)`). Inline that into the existing filtered loop in `draw_scene`, then delete `draw_command` and its now-orphaned callees.

- [ ] **Step 1: Inline the oval fill into `draw_scene`'s `Halo`/`Trouble` loop.** Replace the `draw_command(command, &scene.pet.palette)` call inside the `commands.iter().filter(|c| matches!(c.kind, Halo | Trouble))` loop with the oval-fill body extracted verbatim from `draw_command`'s `Halo`/`Trouble` arm:
```rust
unsafe {
    let path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
        NSPoint::new(
            (command.x - command.radius) as f64,
            (command.y - command.radius) as f64,
        ),
        NSSize::new(
            (command.radius * 2.0) as f64,
            (command.radius * 2.0) as f64,
        ),
    ));
    ns_color(&command.color).setFill();
    path.fill();
}
```
(Copy the EXACT expressions from the current `Halo`/`Trouble` arm — do not paraphrase the coordinate/size math. The `&scene.pet.palette` argument is no longer needed.)

- [ ] **Step 2: Delete `draw_command`** entirely (the whole `fn draw_command` incl. all six match arms).

- [ ] **Step 3: Delete the now-orphaned helpers and types** (each was reachable ONLY from `draw_command`'s deleted arms — verified by grounding):
  - `fn draw_pet_art_block`
  - `fn pet_art_grid`
  - `fn role_for_pet_cell`
  - `fn pet_role_color`
  - `fn draw_label`
  - `fn line_display_width`
  - `fn char_display_width`
  - `struct PetArtGrid`
  - `struct PetArtCell`
  - the `use crate::pet::render::{PaletteRoleName, StyledSegment};` import (both names go away with the structs)

- [ ] **Step 4: Delete the now-dead tests** in the `mod tests` of `app.rs` (each tests a deleted fn):
  - `pet_art_grid_preserves_terminal_columns`
  - `pet_art_grid_maps_terminal_span_roles_to_cells`
  - `companion_role_color_matches_resolver_round_style`
  - `companion_pet_role_color_matches_resolved_palette`

- [ ] **Step 5: Verify nothing live was removed.** `cargo build` compiles. Confirm these LIVE symbols remain and still compile: `attributed_pet_glyph` (used by `companion_grid_metrics` + `appkit_blit_draw_list`), `rgb_color` (used by `appkit_blit_draw_list`), `ns_color` (used by `draw_scene` + the inlined oval), `companion_grid_metrics`, `cell_to_point`, `appkit_blit_draw_list`, `CompanionGridMetrics`, and the `use ...{build_draw_commands, RoundColor, RoundDrawCommand, RoundDrawKind}` import (all four still used). The `cell_to_point_*`, `companion_menu_spec_*`, `companion_animation_*` tests stay (they test live code).

- [ ] **Step 6: Full gate (EXIT STATUS).** `cargo test` 0 FAILED; `cargo test --features dev-preview --test dev_preview` — watch (46) AND round-commands.json goldens BYTE-IDENTICAL (proves `round/draw.rs` untouched); `cargo clippy --all-targets --all-features -- -D warnings` (no new dead-code warnings → removal complete; no compile error → nothing live removed); `cargo fmt --check`.

- [ ] **Step 7: Commit.** `refactor(companion): retire dead old round pet/room/prop rendering (superseded by SceneDrawList blit)`.

---

## Self-Review

**Spec coverage:** retires the companion's dead old round-rendering path (the review's "two parallel derivations" flag, companion half) — Task 1. The dev-preview-coupled half (`build_draw_commands`/`derive_round_scene_model` pet/room/prop) is explicitly deferred to Plan 09. ✓

**Byte-stability argument:** the removed code is provably unreachable (the `draw_scene` loop filters to `Halo`/`Trouble` before the only `draw_command` call site), and the inlined `Halo`/`Trouble` oval is copied verbatim from `draw_command`'s arms — so the companion's pixels are unchanged. The watch + round-commands goldens are untouched because `round/draw.rs` is untouched. `clippy --all-targets` is the completeness oracle.

**Placeholder scan:** the removal list is exact (from grounding, with file:line); the inline body is the verbatim oval-fill. No placeholders.

**Risk:** Low — pure dead-code removal in one file. The only failure mode is removing a symbol that's still live (caught immediately by `cargo build`) or missing one (caught by clippy dead-code). One task; the task review serves as the whole-branch review.

**Out of scope (Plan 09 — dev-preview unification):** retiring `build_draw_commands`'s pet/room/prop emission, `derive_round_scene_model`'s pet/room/prop derivation, and migrating dev-preview's round preview to `render_pet_to_draw_list` (which changes the round-commands.json goldens). Also out of scope: menubar (Plan 07-orig→renumber), screen-window.
