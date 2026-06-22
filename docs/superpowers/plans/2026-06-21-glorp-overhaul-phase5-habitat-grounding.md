# Phase 5 — Habitat Grounding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax.

**Goal:** Ground the pet on the floor with a feet-anchored position and a feet-restricted contact shadow, sharpen the biome sky/ground value separation, front-load honest early character, and activate the dormant `HeavySessionShimmer` scene moment on its real heavy-session signal.

**Architecture:** All work is render-time and signal-derivation only; no `state.json` schema change, no new persisted data. The pet panel (`src/tui/panels/pet.rs`) computes the pet's vertical anchor from its silhouette feet (lowest non-blank art row) instead of vertical centering, and paints a 1-row contact shadow on the buffer beneath the feet columns, composited after the floor/ambient passes and respecting the Phase 1 gutter-precedence rule (species identity particles outrank the shadow). `biome_wash_color` gains a paired floor-darkening so sky reads lighter than ground. Honest front-loading lowers one early prop threshold (`TOKEN_PEBBLE_25K`) and never touches the maturity gate. The heavy-session shimmer is emitted from `scene_moments_for` (`src/tui/room.rs`) keyed to a freshly-earned `HEAVY_SESSION_PLANTER` prop; the effect is already wired in `effect_for_moment` (`src/pet/animator.rs`).

**Tech Stack:** Rust, ratatui, SQLite; tests via cargo + assert_cmd.

## Global Constraints

- Templates are exactly 11 display columns × 8 lines; the `Template` type alias is `[&'static str; 8]` (`src/pet/art.rs`).
- Every art glyph (and every `{slot}` filler) is width-1 under unicode-width's default (ambiguous=narrow); enforced by `every_template_line_is_eleven_display_columns` (`src/pet/art.rs`).
- Eye/accent glyphs must be East-Asian-Width Neutral or Narrow, never Ambiguous; `◇◆◈●○` are Ambiguous (kept only per the Crystal decision — non-blocking lint).
- Growth cell bands (occupied non-space cells across the 8 art rows, fixed reference state): S0:1-4 · S1:5-10 · S2:11-20 · S3:21-34 · S4:35-50 · S5:51-66 · S6:67-88 — disjoint, strictly increasing, S4<S5<S6.
- S6 fills all 8 art rows; the sparkle no longer overwrites art rows 0/7 (asserted as a separate structural check, not in the size count).
- Color is truecolor-first, two tiers only: `ColorCapability::{Truecolor, Flat}` (`src/tui/style.rs:54`); honor `NO_COLOR`/`TERM=dumb`; under Flat pets render monochrome carried by silhouette; sub-truecolor is ratatui's automatic downgrade, not engineered here.
- Tamagotchi spirit: calm over flashy, night calmer than day, nurturing companion not optimizer; no death — floor state is `Mood::Wilted`.
- Only real signals drive content (growth/mood/biome/props/scene-moments trace to observed token usage + clock); the immature-pet zero-feast invariant is preserved (`flat_and_immature_pets_render_zero_motes`, `src/tui/panels/pet/ambient.rs:798`).
- The renderer stays content-agnostic: species/stage character lives in `art.rs` templates + palette, never in renderer special-casing.
- `cargo clippy --all-targets --all-features -- -D warnings` must stay clean; test-only helper fns must be `#[cfg(test)]`.
- Test output must be pristine; intentional error output must be captured and asserted.
- Test isolation: integration tests use `tempfile::tempdir()` + `GLORP_CONFIG_DIR`; when testing helper failures, pin BOTH `GLORP_CCUSAGE_BIN` and `GLORP_CCUSAGE_CODEX_BIN`.
- Commit frequently (do not ask first); WIP branch off `main`; never `git add -A` without a prior `git status`.
- Identity data is never touched: no `state.json` schema change; `seed`/`accepted_name`/`xp`/vitals/stage/calibration/seen-transitions untouched. A one-time visual reset is accepted.
- Do NOT call `apply_usage_poll` from production code (`#[doc(hidden)]` test wrapper).

---

## File Structure

- `src/pet/panels/pet.rs` (actually `src/tui/panels/pet.rs`) — **Modify.** `pet_inner_rect_in_panel` anchors the pet feet-relative instead of vertically centered; a new `render_contact_shadow` pass paints a 1-row shadow under the feet columns; `PetPanel::render` calls it after the ambient/floor passes and before/around the pet, respecting gutter precedence.
- `src/tui/panels/pet/ambient.rs` — **Modify.** `biome_wash_color` extended with a paired `biome_floor_wash_color` so sky reads lighter than ground; the pet panel paints the floor band with the darker wash. Adds the contact-shadow color helper.
- `src/game/habitat.rs` — **Modify.** Lower `TOKEN_PEBBLE_25K` lifetime threshold for honest early front-loading (maturity gate and all other thresholds untouched).
- `src/tui/room.rs` — **Modify.** `scene_moments_for` emits a `HeavySessionShimmer` `SceneMoment` when the `HEAVY_SESSION_PLANTER` prop was freshly earned (a real, replay-safe signal).
- `src/dev_preview/watch.rs` — already serializes `scene_moments` (line ~1284); no change needed, used for review only.

### Phase 1 interfaces this phase CONSUMES (copy exactly; do not re-invent)

From `src/pet/render.rs` (Phase 1, §2.4 of the contract):

```rust
// Per-species gutter identity for the 13×10 frame's gutter rows (0 and 9) and side columns.
pub enum GutterContent { Sparkle, MachineFrame, None }
fn gutter_content_for(species: Species, stage: Stage) -> GutterContent;

// Lowest non-blank art row of the rendered 8 rows = the silhouette's "feet".
pub(crate) fn feet_row(art_lines: &[String]) -> Option<usize>;
pub(crate) fn feet_columns(art_lines: &[String]) -> Vec<usize>;
```

**Precedence (highest wins) when an S6 sparkle, a species identity particle, and a contact shadow target the same cell: species identity particle > S6 sparkle > contact shadow.** The contact shadow is restricted to `feet_columns` on the gutter row beneath the feet, leaving side-column gutter identity cells (Crystal facets, Mech LED) untouched. The S6 sparkle uses row 0 only.

> **Phase-ordering note for the implementer:** This plan is executed AFTER Phase 1. If `feet_row`/`feet_columns` are not yet present in `src/pet/render.rs` when you start (e.g. Phase 1 slipped), STOP and confirm with the lead before adding them here — they are Phase 1's deliverable. Task 1 below includes a guard step that checks they exist.

### This phase PRODUCES (later phases / reconciler rely on these)

- `src/tui/panels/pet.rs`:
  - `pub(crate) fn pet_feet_anchor_y(area: Rect, art_lines: &[String], pet_h: u16) -> u16` — the feet-relative top-y of the pet rect (replaces the centered `cy`).
  - `fn contact_shadow_cells(pet_rect: Rect, art_lines: &[String], mirror: bool, habitat: Rect) -> Vec<(u16, u16)>` — absolute `(col,row)` cells of the shadow, already clipped to `habitat` and restricted to feet columns on the row beneath the feet.
- `src/tui/panels/pet/ambient.rs`:
  - `pub(super) fn biome_floor_wash_color(tag: RoomBiomeTag) -> ratatui::style::Color` — the darker ground companion of `biome_wash_color`.
  - `pub(super) fn contact_shadow_color(floor_wash: Color) -> Color` — a slightly darker tint of the floor wash, used by the shadow pass.
- `src/tui/room.rs`: no new public symbol; `scene_moments_for` simply gains a `HeavySessionShimmer` arm.

---

## Task 1 — Feet-relative anchor helper

Replace the vertical-centering `cy` with a feet-relative anchor so the pet's lowest non-blank art row sits near the habitat floor instead of floating in the middle.

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Test: `src/tui/panels/pet.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes (Phase 1, `src/pet/render.rs`): `pub(crate) fn feet_row(art_lines: &[String]) -> Option<usize>;`
- Produces: `pub(crate) fn pet_feet_anchor_y(area: Rect, art_lines: &[String], pet_h: u16) -> u16;`

Behavior contract: the pet rect's bottom-most rendered art row (frame row `feet_row + 1`, since `frame_with_particles` inserts the 11×8 art at frame rows 1..=8) should land one row above the habitat's floor row (`area.y + area.height - 1`), leaving the floor row visible beneath the feet for the contact shadow. When `area` is shorter than `pet_h`, the anchor clamps to `area.y` (degenerate, no panic — matching the existing `.max(...)` clamp discipline).

- [ ] **Step 1: Confirm the Phase 1 feet helper exists.** Run:
  ```bash
  grep -n "pub(crate) fn feet_row" src/pet/render.rs
  ```
  Expect a single match. If empty, STOP (see phase-ordering note above).

- [ ] **Step 2: Write the failing test.** Add to the `tests` module in `src/tui/panels/pet.rs`:
  ```rust
  #[test]
  fn pet_feet_anchor_drops_feet_one_row_above_floor() {
      // 13×10 frame: art occupies frame rows 1..=8. A pet whose lowest non-blank
      // art row is art-row 5 (frame row 6) should anchor so frame row 6 lands at
      // habitat_floor_row - 1, i.e. the feet sit just above the floor band.
      let area = Rect::new(0, 0, 13, 24);
      // 10 lines, last non-blank art line at index 5; indices 6,7 blank; plus the
      // two particle-gutter frame rows are NOT part of art_lines here — art_lines
      // are the 8 art rows the renderer passes (vm.pet_art is the framed 10 rows;
      // feet_row operates on the framed lines).
      let art_lines: Vec<String> = vec![
          "             ".to_string(), // 0 gutter
          "    ▟██▙     ".to_string(), // 1
          "   ▓██████   ".to_string(), // 2
          "   ▒o o▒     ".to_string(), // 3
          "   ▒ w ▒     ".to_string(), // 4
          "   ▙▒▒▟      ".to_string(), // 5 feet (lowest non-blank)
          "             ".to_string(), // 6
          "             ".to_string(), // 7
          "             ".to_string(), // 8
          "             ".to_string(), // 9 gutter
      ];
      let y = pet_feet_anchor_y(area, &art_lines, PET_H);
      // feet at framed row 5; floor row = 23; we want framed row 5 -> row 22.
      // So pet_rect.y = 22 - 5 = 17.
      assert_eq!(y, 17, "feet should land one row above the floor");
  }

  #[test]
  fn pet_feet_anchor_clamps_when_area_shorter_than_pet() {
      let area = Rect::new(0, 5, 13, 4); // shorter than PET_H=10
      let art_lines: Vec<String> = (0..10).map(|_| "      X      ".to_string()).collect();
      let y = pet_feet_anchor_y(area, &art_lines, PET_H);
      assert_eq!(y, area.y, "degenerate area clamps to origin, no underflow");
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::tests::pet_feet_anchor 2>&1 | tail -20
  ```
  Expect FAIL: `cannot find function `pet_feet_anchor_y` in this scope`.

- [ ] **Step 3: Implement `pet_feet_anchor_y`.** Add above `pet_inner_rect_in_panel` in `src/tui/panels/pet.rs`:
  ```rust
  /// Top-y of the pet's 13×10 rect so the silhouette's lowest non-blank row
  /// (its "feet") lands one row above the habitat floor row, instead of being
  /// vertically centered. `art_lines` are the framed 10 rows (`vm.pet_art`);
  /// `feet_row` returns the lowest non-blank framed row. Clamps to `area.y`
  /// when the area is too short for the pet (degenerate, no panic).
  pub(crate) fn pet_feet_anchor_y(area: Rect, art_lines: &[String], pet_h: u16) -> u16 {
      let floor_row = area.y + area.height.saturating_sub(1);
      // Reserve one row for the floor band beneath the feet.
      let feet_target_row = floor_row.saturating_sub(1);
      let feet = crate::pet::render::feet_row(art_lines).unwrap_or((pet_h as usize).saturating_sub(1));
      let anchor = feet_target_row.saturating_sub(feet as u16);
      anchor.max(area.y)
  }
  ```
  Run the same test command. Expect PASS.

- [ ] **Step 4: Wire the anchor into `pet_inner_rect_in_panel`.** In `src/tui/panels/pet.rs`, the helper currently has no access to art lines. Change its signature to take `art_lines` and replace the centered `cy`:
  ```rust
  pub(crate) fn pet_inner_rect_in_panel(area: Rect, vm: &WatchViewModel) -> Rect {
      let cx = area.x + area.width.saturating_sub(PET_W) / 2;
      let cy = pet_feet_anchor_y(area, &vm.pet_art, PET_H);
      // When `area` is smaller than the pet, the upper clamp bound would fall
      // below `area.x` / `area.y`, which makes `i32::clamp` panic. `.max(...)`
      // ensures min ≤ max so the rect collapses to `area`'s origin instead.
      let max_x = (area.x + area.width).saturating_sub(PET_W).max(area.x);
      let max_y = (area.y + area.height).saturating_sub(PET_H).max(area.y);
      let wander_x = vm.wander_offset_x as i32;
      let x = (cx as i32 + wander_x).clamp(area.x as i32, max_x as i32) as u16;
      let y = (cy as i32 + vm.breath_offset_y as i32).clamp(area.y as i32, max_y as i32) as u16;
      Rect::new(x, y, PET_W, PET_H)
  }
  ```
  (`vm.pet_art` is already in scope via the `WatchViewModel` param; no signature change to callers.)

- [ ] **Step 5: Fix the now-stale centering test.** The existing `pet_panel_renders_pet_centered_in_tall_rect` asserts the pet renders in a tall rect — its assertion (`s.contains('o') || ...`) still holds since the pet still renders, just lower. Rename it for honesty and keep its assertion:
  ```rust
  #[test]
  fn pet_panel_renders_pet_grounded_in_tall_rect() {
  ```
  (only the fn name changes; the body asserts visible content, which a grounded pet still produces). Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet 2>&1 | tail -25
  ```
  Expect PASS (all pet panel tests green).

- [ ] **Step 6: Add a grounding regression test at narrow width.** The whole point is the pet no longer floats. Add:
  ```rust
  #[test]
  fn pet_sits_in_the_lower_half_at_narrow_column_width() {
      // 40-wide is the real pet column. In a tall area the lowest pet glyph row
      // must be in the lower half — proof the pet is grounded, not centered.
      let vm = vm_with_real_pet();
      let panel = PetPanel;
      let ctx = test_context();
      let backend = TestBackend::new(40, 24);
      let mut terminal = Terminal::new(backend).unwrap();
      terminal
          .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
          .unwrap();
      let buf = terminal.backend().buffer();
      let mut lowest_pet_row = 0u16;
      for y in 0..24u16 {
          for x in 0..40u16 {
              let sym = buf[(x, y)].symbol();
              // pet art uses block + ascii glyphs; floor uses dot texture.
              if matches!(sym.chars().next(), Some(c) if "▟▙█▓▒owO".contains(c)) {
                  lowest_pet_row = lowest_pet_row.max(y);
              }
          }
      }
      assert!(
          lowest_pet_row >= 12,
          "grounded pet's lowest glyph should be in the lower half (row >= 12), got {lowest_pet_row}"
      );
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::tests::pet_sits_in_the_lower_half 2>&1 | tail -15
  ```
  Expect PASS. If the fixture pet's glyph alphabet differs, broaden the match set to the real fixture glyphs (read `vm_with_real_pet` — it renders `generate_pet("pet-panel-test-seed")` at `Stage::S2`; confirm the glyphs by inspecting the failure output, do not guess).

- [ ] **Step 7: Full suite + clippy gate, then commit.**
  ```bash
  cargo test -p glorp --lib 2>&1 | tail -15
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```
  Expect both clean. Commit:
  ```bash
  git add src/tui/panels/pet.rs
  git commit -m "feat: anchor pet to the floor by its silhouette feet

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 2 — Sky/ground value separation via paired biome wash

Extend the existing single `biome_wash_color` into a sky/ground pair so the floor band reads visibly darker than the sky — a clearer two-tone, not a parallel system. The pet panel paints the floor band (the lower `FLOOR_BAND_ROWS` rows of the habitat) with the darker wash.

**Files:**
- Modify: `src/tui/panels/pet/ambient.rs`
- Modify: `src/tui/panels/pet.rs` (the wash pass in `PetPanel::render`)
- Test: `src/tui/panels/pet/ambient.rs` (inline tests) + `src/tui/panels/pet.rs` (existing `biome_wash_is_subtle_and_biome_distinct`)

**Interfaces:**
- Consumes: `pub(super) fn biome_wash_color(tag: RoomBiomeTag) -> Color;` (existing, `ambient.rs:63`).
- Produces: `pub(super) fn biome_floor_wash_color(tag: RoomBiomeTag) -> Color;`

- [ ] **Step 1: Write the failing test.** Add to the `tests` module in `src/tui/panels/pet/ambient.rs`:
  ```rust
  #[test]
  fn floor_wash_is_darker_than_sky_wash_per_biome() {
      use crate::tui::room::RoomBiomeTag;
      use ratatui::style::Color;
      for tag in [
          RoomBiomeTag::Starter,
          RoomBiomeTag::Botanical,
          RoomBiomeTag::Technical,
          RoomBiomeTag::Celestial,
          RoomBiomeTag::Artifact,
          RoomBiomeTag::Cozy,
      ] {
          let sky = biome_wash_color(tag);
          let floor = biome_floor_wash_color(tag);
          let (Color::Rgb(sr, sg, sb), Color::Rgb(fr, fg, fb)) = (sky, floor) else {
              panic!("washes must be rgb");
          };
          let sky_lum = sr as u32 + sg as u32 + sb as u32;
          let floor_lum = fr as u32 + fg as u32 + fb as u32;
          assert!(
              floor_lum < sky_lum,
              "{tag:?}: floor wash {floor_lum} must be darker than sky wash {sky_lum}"
          );
          // Stay subtle: floor within 36 of sky per channel (a touch deeper, not black).
          assert!((sr as i16 - fr as i16).abs() <= 36);
          assert!((sg as i16 - fg as i16).abs() <= 36);
          assert!((sb as i16 - fb as i16).abs() <= 36);
      }
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::ambient::tests::floor_wash 2>&1 | tail -15
  ```
  Expect FAIL: `cannot find function `biome_floor_wash_color``.

- [ ] **Step 2: Implement `biome_floor_wash_color`.** Add directly below `biome_wash_color` in `src/tui/panels/pet/ambient.rs`:
  ```rust
  /// Ground companion to [`biome_wash_color`]: the same biome nudge, then a
  /// uniform darkening so the floor band reads as ground value, lighter sky
  /// above it. Kept subtle (a small fixed subtraction) so it deepens without
  /// turning into a hard band.
  pub(super) fn biome_floor_wash_color(tag: crate::tui::room::RoomBiomeTag) -> ratatui::style::Color {
      use ratatui::style::Color;
      const FLOOR_DARKEN: i16 = 14;
      let Color::Rgb(r, g, b) = biome_wash_color(tag) else {
          return biome_wash_color(tag);
      };
      let darken = |v: u8| (v as i16 - FLOOR_DARKEN).clamp(0, 255) as u8;
      Color::Rgb(darken(r), darken(g), darken(b))
  }
  ```
  Run the same test command. Expect PASS.

- [ ] **Step 3: Add the contact-shadow color helper (used by Task 3).** Add below `biome_floor_wash_color`:
  ```rust
  /// The contact shadow under the pet's feet: the floor wash deepened a bit
  /// further so the pet reads as resting ON the ground, never a hard black
  /// blob. Calm, never high-contrast (Tamagotchi spirit).
  pub(super) fn contact_shadow_color(floor_wash: ratatui::style::Color) -> ratatui::style::Color {
      use ratatui::style::Color;
      const SHADOW_DARKEN: i16 = 16;
      let Color::Rgb(r, g, b) = floor_wash else {
          return floor_wash;
      };
      let darken = |v: u8| (v as i16 - SHADOW_DARKEN).clamp(0, 255) as u8;
      Color::Rgb(darken(r), darken(g), darken(b))
  }
  ```
  Add a test:
  ```rust
  #[test]
  fn contact_shadow_is_darker_than_its_floor_wash_and_stays_rgb() {
      use crate::tui::room::RoomBiomeTag;
      use ratatui::style::Color;
      let floor = biome_floor_wash_color(RoomBiomeTag::Botanical);
      let shadow = contact_shadow_color(floor);
      let (Color::Rgb(fr, fg, fb), Color::Rgb(sr, sg, sb)) = (floor, shadow) else {
          panic!("rgb");
      };
      assert!((sr as u32 + sg as u32 + sb as u32) < (fr as u32 + fg as u32 + fb as u32));
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::ambient::tests::contact_shadow 2>&1 | tail -15
  ```
  Expect PASS.

- [ ] **Step 4: Export the new helpers to the pet panel.** In `src/tui/panels/pet.rs`, the panel imports ambient helpers at the top (the `use ambient::{...}` block around line 28). Add `biome_floor_wash_color` and `contact_shadow_color` to that import list:
  ```rust
  use ambient::{
      activity_glyphs_for, ambient_glyph_is_inside_area, biome_floor_wash_color, biome_wash_color,
      contact_shadow_color, effective_weekend_softening, mote_glyphs_for, weekend_soften_color,
  };
  ```

- [ ] **Step 5: Paint the floor band darker in the wash pass.** In `src/tui/panels/pet.rs` `PetPanel::render`, the wash block currently fills the whole habitat with one `wash` color (the `{ let wash = biome_wash_color(...); for wy ... }` block around line 245). Split it so the lower `FLOOR_BAND_ROWS` rows get the floor wash. First add a const near the other consts at the top of the file (after `const MOTE_GLYPHS`):
  ```rust
  /// The lower N habitat rows painted with the deeper floor wash so the ground
  /// reads as a value distinct from the lighter sky above it.
  const FLOOR_BAND_ROWS: u16 = 3;
  ```
  Then replace the wash loop body:
  ```rust
  {
      let sky_wash = biome_wash_color(room_profile.biome.primary);
      let floor_wash = biome_floor_wash_color(room_profile.biome.primary);
      let floor_band_top = scene
          .habitat
          .y
          .saturating_add(scene.habitat.height.saturating_sub(FLOOR_BAND_ROWS));
      for wy in scene.habitat.y..scene.habitat.y.saturating_add(scene.habitat.height) {
          let wash = if wy >= floor_band_top { floor_wash } else { sky_wash };
          for wx in scene.habitat.x..scene.habitat.x.saturating_add(scene.habitat.width) {
              buf[(wx, wy)].set_style(ratatui::style::Style::default().bg(wash));
          }
      }
  }
  ```

- [ ] **Step 6: Update the existing wash subtlety test.** The `biome_wash_is_subtle_and_biome_distinct` test in `src/tui/panels/pet.rs` still passes (it tests `biome_wash_color`, unchanged). Run the full pet-panel and ambient suites:
  ```bash
  cargo test -p glorp --lib tui::panels::pet 2>&1 | tail -20
  ```
  Expect PASS.

- [ ] **Step 7: Commit.**
  ```bash
  git add src/tui/panels/pet/ambient.rs src/tui/panels/pet.rs
  git commit -m "feat: deepen the biome floor wash for sky/ground value separation

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 3 — Contact shadow under the feet (gutter-precedence aware)

Paint a calm 1-row contact shadow on the row directly beneath the pet's feet, restricted to the columns under the feet (per the Phase 1 precedence rule: species identity particles outrank the shadow; the shadow stays in `feet_columns` so side-column gutter identity — Crystal facets, Mech LED — is untouched). Composited after the floor/ambient passes, before the pet art, so the pet still paints over its own feet.

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Test: `src/tui/panels/pet.rs` (inline tests)

**Interfaces:**
- Consumes (Phase 1, `src/pet/render.rs`):
  - `pub(crate) fn feet_columns(art_lines: &[String]) -> Vec<usize>;`
  - `pub(crate) fn feet_row(art_lines: &[String]) -> Option<usize>;`
- Consumes (Task 2): `pub(super) fn contact_shadow_color(Color) -> Color;`, `pub(super) fn biome_floor_wash_color(RoomBiomeTag) -> Color;`
- Produces: `fn contact_shadow_cells(pet_rect: Rect, art_lines: &[String], mirror: bool, habitat: Rect) -> Vec<(u16, u16)>;`

Behavior: the shadow row is `pet_rect.y + (feet_row + 1)` (one row below the lowest art glyph row), clipped to `habitat`. The shadow columns are `pet_rect.x + col` for each `col` in `feet_columns(art_lines)`, mirrored when `mirror` is true (same column-flip math as `pet_silhouette_halo_rects`). The shadow is a background tint only (`set_style(bg)`), never a glyph — it must not overwrite floor texture glyphs' chars, only deepen the cell behind them.

- [ ] **Step 1: Write the failing test for cell computation.** Add to the `tests` module in `src/tui/panels/pet.rs`:
  ```rust
  #[test]
  fn contact_shadow_lands_one_row_below_feet_under_feet_columns() {
      // Framed art: feet glyphs at framed row 5, columns 4 and 6.
      let art_lines: Vec<String> = vec![
          "             ".to_string(), // 0
          "             ".to_string(), // 1
          "             ".to_string(), // 2
          "             ".to_string(), // 3
          "             ".to_string(), // 4
          "    X X      ".to_string(), // 5 feet at cols 4 and 6
          "             ".to_string(), // 6
          "             ".to_string(), // 7
          "             ".to_string(), // 8
          "             ".to_string(), // 9
      ];
      let pet_rect = Rect::new(10, 20, 13, 10);
      let habitat = Rect::new(0, 0, 60, 40);
      let cells = contact_shadow_cells(pet_rect, &art_lines, false, habitat);
      // feet_row = 5 -> shadow row = pet_rect.y + 6 = 26.
      // feet cols 4,6 -> absolute 14,16.
      let set: std::collections::HashSet<(u16, u16)> = cells.into_iter().collect();
      assert!(set.contains(&(14, 26)), "shadow under left foot");
      assert!(set.contains(&(16, 26)), "shadow under right foot");
      assert!(!set.contains(&(15, 26)), "gap between feet is not shadowed");
      assert_eq!(set.len(), 2, "shadow is exactly the feet columns, no halo");
  }

  #[test]
  fn contact_shadow_is_clipped_to_habitat() {
      let art_lines: Vec<String> =
          (0..10).map(|i| if i == 7 { "XXXXXXXXXXXXX".to_string() } else { "             ".to_string() }).collect();
      let pet_rect = Rect::new(0, 0, 13, 10);
      // Habitat only 5 rows tall: shadow row would be below it -> empty.
      let habitat = Rect::new(0, 0, 13, 5);
      let cells = contact_shadow_cells(pet_rect, &art_lines, false, habitat);
      assert!(cells.is_empty(), "shadow below the habitat floor is clipped away");
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::tests::contact_shadow 2>&1 | tail -15
  ```
  Expect FAIL: `cannot find function `contact_shadow_cells``.

- [ ] **Step 2: Implement `contact_shadow_cells`.** Add to `src/tui/panels/pet.rs` (near `pet_feet_anchor_y`):
  ```rust
  /// Absolute `(col, row)` cells of the contact shadow: the columns directly
  /// under the silhouette's feet, on the row one below the lowest art glyph,
  /// clipped to `habitat`. `mirror` flips columns the same way the pet art is
  /// mirrored when facing left. Restricted to feet columns so side-column
  /// gutter identity (Crystal facets, Mech LED) is never overwritten
  /// (gutter-precedence rule, Phase 1 §2.4).
  fn contact_shadow_cells(
      pet_rect: Rect,
      art_lines: &[String],
      mirror: bool,
      habitat: Rect,
  ) -> Vec<(u16, u16)> {
      let Some(feet) = crate::pet::render::feet_row(art_lines) else {
          return Vec::new();
      };
      let shadow_row = pet_rect.y + (feet as u16) + 1;
      // Clip: must be inside the habitat (and at/below the feet, never above).
      if shadow_row < habitat.y || shadow_row >= habitat.y.saturating_add(habitat.height) {
          return Vec::new();
      }
      let line_width = art_lines
          .get(feet)
          .map(|l| l.chars().count())
          .unwrap_or(0);
      crate::pet::render::feet_columns(art_lines)
          .into_iter()
          .filter_map(|col| {
              let col_in_frame = if mirror {
                  line_width.saturating_sub(1).saturating_sub(col)
              } else {
                  col
              };
              let abs_col = pet_rect.x + col_in_frame as u16;
              if abs_col < habitat.x || abs_col >= habitat.x.saturating_add(habitat.width) {
                  return None;
              }
              Some((abs_col, shadow_row))
          })
          .collect()
  }
  ```
  Run the same test command. Expect PASS.

  > Note: `feet_columns` (Phase 1) returns the occupied columns of the feet row. If Phase 1 defines `feet_columns` over *all* art rows rather than just the lowest row, the `line_width` mirror anchor here must use the feet row's width — confirm the Phase 1 doc comment. The contract says "the columns directly beneath this row," so feet_columns is the lowest-row columns; this implementation assumes that. If it differs, STOP and reconcile with the lead before adapting.

- [ ] **Step 3: Write the failing render-integration test.** The shadow must actually deepen the buffer bg beneath the pet without overwriting glyph chars. Add:
  ```rust
  #[test]
  fn contact_shadow_deepens_bg_under_feet_without_replacing_glyphs() {
      let vm = vm_with_real_pet();
      let panel = PetPanel;
      let ctx = test_context();
      let backend = TestBackend::new(40, 24);
      let mut terminal = Terminal::new(backend).unwrap();
      terminal
          .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
          .unwrap();
      let buf = terminal.backend().buffer();
      // Find the pet rect, derive its feet/shadow cells, and assert at least one
      // shadow cell carries a non-default bg (the shadow tint) and is not blanked
      // out as a glyph (the shadow is bg-only).
      let pet_rect = pet_inner_rect_in_panel(f_area(), &vm);
      let cells = contact_shadow_cells(pet_rect, &vm.pet_art, vm.facing == -1, f_area());
      // At least one shadow cell exists for a grounded S2 pet in a 24-tall area.
      assert!(!cells.is_empty(), "a grounded pet has a contact shadow");
      let mut deepened = 0usize;
      for (x, y) in &cells {
          if *x < 40 && *y < 24 {
              if let Some(ratatui::style::Color::Rgb(..)) = buf[(*x, *y)].style().bg {
                  deepened += 1;
              }
          }
      }
      assert!(deepened > 0, "shadow cells must carry a deepened bg tint");
  }

  fn f_area() -> Rect {
      Rect::new(0, 0, 40, 24)
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::tests::contact_shadow_deepens 2>&1 | tail -20
  ```
  Expect FAIL (shadow not yet painted; `deepened` is 0, or the bg is the floor wash not the shadow tint).

- [ ] **Step 4: Add the contact-shadow pass to `PetPanel::render`.** In `src/tui/panels/pet.rs`, after the wash block (Task 2) and after the ambient/mote/floor glyph passes but BEFORE `render_pet_inside` (so the pet still paints over its own feet), insert:
  ```rust
  // Contact shadow: a calm bg deepening directly under the pet's feet so it
  // reads as resting ON the floor. Restricted to feet columns (gutter
  // precedence: species identity side cells are never touched). Bg-only — it
  // never replaces a floor-texture glyph, just deepens the cell behind it.
  {
      let mirror = vm.facing == -1;
      let floor_wash = biome_floor_wash_color(room_profile.biome.primary);
      let shadow = contact_shadow_color(floor_wash);
      for (sx, sy) in contact_shadow_cells(scene.pet_art, &vm.pet_art, mirror, scene.habitat) {
          let cell = &mut buf[(sx, sy)];
          let mut style = cell.style();
          style.bg = Some(shadow);
          cell.set_style(style);
      }
  }
  ```
  Place this immediately before the `render_pet_inside(...)` call (around line 363). Note `scene.pet_art` is the absolute pet rect; `room_profile` is already in scope from the room pass.

- [ ] **Step 5: Run the integration test.**
  ```bash
  cargo test -p glorp --lib tui::panels::pet::tests::contact_shadow_deepens 2>&1 | tail -20
  ```
  Expect PASS. If the shadow row collides with the floor glyph row and you need the shadow to sit ON the floor row rather than above it, that is correct — the shadow is the cell directly under the feet, which is the floor band. Confirm via the test, not by guessing.

- [ ] **Step 6: Guard the precedence rule with a test.** The shadow must NOT touch side-column gutter identity. Since the shadow is feet-columns-only by construction, assert that a wide-footed pet's shadow never extends past the feet span:
  ```rust
  #[test]
  fn contact_shadow_never_exceeds_the_feet_span() {
      let art_lines: Vec<String> = vec![
          "             ".to_string(),
          "             ".to_string(),
          "             ".to_string(),
          "             ".to_string(),
          "             ".to_string(),
          "             ".to_string(),
          "  ▙▒▒▒▒▒▟    ".to_string(), // feet span cols 2..=8
          "             ".to_string(),
          "             ".to_string(),
          "             ".to_string(),
      ];
      let pet_rect = Rect::new(5, 5, 13, 10);
      let habitat = Rect::new(0, 0, 60, 40);
      let cells = contact_shadow_cells(pet_rect, &art_lines, false, habitat);
      let cols: std::collections::HashSet<u16> = cells.iter().map(|(c, _)| *c).collect();
      // No shadow column outside the feet glyph span (abs cols 7..=13 for cols 2..=8).
      for c in &cols {
          assert!(*c >= 7 && *c <= 13, "shadow col {c} escaped the feet span");
      }
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::tests::contact_shadow_never_exceeds 2>&1 | tail -10
  ```
  Expect PASS.

- [ ] **Step 7: Full suite + clippy, commit.**
  ```bash
  cargo test -p glorp --lib 2>&1 | tail -15
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```
  Expect clean. Commit:
  ```bash
  git add src/tui/panels/pet.rs
  git commit -m "feat: paint a feet-restricted contact shadow under the pet

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 4 — Honest early front-loading (lower the 25k pebble threshold)

Front-load a touch of early character without fabricating a feast: lower the `TOKEN_PEBBLE_25K` prop's lifetime threshold so the very first earned prop arrives sooner. The maturity gate and every other threshold stay exactly where they are; `flat_and_immature_pets_render_zero_motes` is untouched (motes gate on `day.mature`, not on props).

**Files:**
- Modify: `src/game/habitat.rs`
- Test: `src/game/habitat.rs` (inline tests) + any existing threshold test

**Interfaces:**
- Consumes: nothing new.
- Produces: no signature change — a constant value change with test coverage.

Decision: lower `TOKEN_PEBBLE_25K` from `25_000.0` to `10_000.0`. This is the earliest prop on the lifetime ladder; the rename of the const (`TOKEN_PEBBLE_25K`) stays for stability (renaming it touches many call sites and the spec permits "lower a specific early prop threshold," not a rename). Document the divergence between the name and the value in a comment.

- [ ] **Step 1: Find the existing threshold assertions.** Run:
  ```bash
  grep -rn "25_000\|TOKEN_PEBBLE_25K\|pebble" src/ tests/ | grep -v "target/" | head -30
  ```
  Note every test that asserts the 25k value so they can be updated honestly (never silently).

- [ ] **Step 2: Write the failing test.** Add to the `tests` module in `src/game/habitat.rs`:
  ```rust
  #[test]
  fn pebble_unlocks_earlier_for_honest_early_character() {
      let spec = catalog_prop_by_str(TOKEN_PEBBLE_25K).unwrap();
      assert_eq!(
          spec.lifetime_threshold,
          Some(10_000.0),
          "the first pebble front-loads early character at 10k lifetime tokens"
      );
      // The maturity gate (100k default baseline) is unrelated and untouched —
      // a pet at 10k lifetime tokens is still immature and renders zero motes.
  }
  ```
  (If `catalog_prop_by_str` is not the accessor, use the real one — `grep -n "fn catalog_prop_by_str\|fn catalog_prop\b" src/game/habitat.rs` and match the signature.)
  Run:
  ```bash
  cargo test -p glorp --lib game::habitat::tests::pebble_unlocks_earlier 2>&1 | tail -15
  ```
  Expect FAIL: `left: Some(25000.0) right: Some(10000.0)`.

- [ ] **Step 3: Lower the threshold.** In `src/game/habitat.rs`, the `TOKEN_PEBBLE_25K` catalog entry (around line 87-94), change:
  ```rust
      HabitatPropSpec {
          id: TOKEN_PEBBLE_25K,
          kind: HabitatPropKind::Accent,
          zone: HabitatPropZone::FloorLeft,
          display_priority: 10,
          // Front-loaded: the first pebble arrives at 10k lifetime tokens so a
          // young pet's habitat shows honest early character. The const name
          // keeps its 25k label for call-site stability; the value is the source
          // of truth. Maturity gate and later thresholds are unchanged.
          lifetime_threshold: Some(10_000.0),
          pet_layer: HabitatPetLayer::Behind,
          color: (0xa8, 0xa4, 0x9c), // weathered stone
      },
  ```
  Run the same test command. Expect PASS.

- [ ] **Step 4: Update any stale threshold tests honestly.** For each test found in Step 1 that asserted `25_000.0` for this prop, update the expected value to `10_000.0` with a comment noting the front-loading intent. Do NOT delete any test. Run:
  ```bash
  cargo test -p glorp --lib game::habitat 2>&1 | tail -20
  ```
  Expect PASS.

- [ ] **Step 5: Confirm the zero-feast invariant is intact.**
  ```bash
  cargo test -p glorp --lib flat_and_immature_pets_render_zero_motes 2>&1 | tail -10
  ```
  Expect PASS (motes gate on `day.mature`, not props — this change cannot affect it, the test proves it).

- [ ] **Step 6: Commit.**
  ```bash
  git add src/game/habitat.rs
  git commit -m "feat: front-load the first pebble prop at 10k lifetime tokens

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 5 — Activate `HeavySessionShimmer` on the real heavy-session signal

`HeavySessionShimmer` is a defined `SceneMomentKey` with a wired effect (`effect_for_moment`, `src/pet/animator.rs:815`) but is never emitted. Emit it from `scene_moments_for` (`src/tui/room.rs`) on a real, replay-safe signal: a `HEAVY_SESSION_PLANTER` prop earned via `HabitatPropSource::HeavySession` within a recent freshness window of `now`. This mirrors how `FeedSweep` fires off a fresh `last_feed_pulse_at`. `DreamGlimmer` stays dropped (no real signal).

**Files:**
- Modify: `src/tui/room.rs`
- Test: `src/tui/room.rs` (inline tests)

**Interfaces:**
- Consumes: the `vm.habitat.earned_props` view (`EarnedHabitatPropView { id, earned_at, source, .. }`), `HabitatPropSource::HeavySession`, `HEAVY_SESSION_PLANTER` (`src/game/habitat.rs:80`).
- Produces: no new public symbol; a new arm in `scene_moments_for`.

Rationale for the signal: the heavy-session unlock (`recent_effective_tokens >= threshold`, `unlock_heavy_session`, `habitat.rs:355`) records the `HEAVY_SESSION_PLANTER` prop with `HabitatPropSource::HeavySession` at poll time. The freshly-earned prop IS the real heavy-session signal surfaced to the render layer. Keying the shimmer to `earned_at` freshness makes it: (a) traceable to real tokens, (b) replay-safe (the `trigger_id` encodes `earned_at`, so the one-shot `seen_triggers` guard in `update_scene_moments` fires it exactly once), and (c) calm (a single ~700ms hsl shimmer, not a flash).

Freshness window: `HEAVY_SESSION_SHIMMER_FRESH = Duration::minutes(20)`. The planter earns once per pet lifetime in practice (a `record_prop` no-op on re-earn), so the window only matters the first time it's earned; choosing 20 minutes ensures the shimmer plays on the next render after the heavy session that earned it, even across a poll boundary, without re-firing later (the trigger_id is stable per `earned_at`, so the guard prevents replay regardless of window).

- [ ] **Step 1: Write the failing test.** Add to the `tests` module in `src/tui/room.rs`. First a helper that builds a freshly-earned HeavySession prop view, then the assertions:
  ```rust
  fn earned_heavy_session(earned_at: time::OffsetDateTime) -> EarnedHabitatPropView {
      EarnedHabitatPropView {
          id: HabitatPropId::new(HEAVY_SESSION_PLANTER),
          earned_at,
          kind: crate::game::habitat::catalog_prop_by_str(HEAVY_SESSION_PLANTER)
              .unwrap()
              .kind,
          display_priority: 80,
          source: HabitatPropSource::HeavySession,
      }
  }

  #[test]
  fn fresh_heavy_session_planter_emits_a_shimmer() {
      let now = datetime!(2026-06-11 10:00 UTC);
      let mut vm = vm_with_props(vec![earned_heavy_session(now - Duration::minutes(5))]);
      vm.day_context.asleep = false;
      let profile = derive_room_life_profile(&vm, now);
      assert!(
          profile
              .scene_moments
              .iter()
              .any(|m| m.key == SceneMomentKey::HeavySessionShimmer),
          "a heavy session earned 5 minutes ago should shimmer the room"
      );
  }

  #[test]
  fn stale_heavy_session_planter_does_not_shimmer() {
      let now = datetime!(2026-06-11 10:00 UTC);
      let vm = vm_with_props(vec![earned_heavy_session(now - Duration::hours(6))]);
      let profile = derive_room_life_profile(&vm, now);
      assert!(
          !profile
              .scene_moments
              .iter()
              .any(|m| m.key == SceneMomentKey::HeavySessionShimmer),
          "a long-past heavy session must not keep shimmering"
      );
  }

  #[test]
  fn lifetime_planter_without_heavy_session_source_does_not_shimmer() {
      // A planter earned via the lifetime ladder (not a heavy session) must not
      // trigger the heavy-session shimmer — the signal is source-specific.
      let now = datetime!(2026-06-11 10:00 UTC);
      let mut prop = earned_heavy_session(now - Duration::minutes(2));
      prop.source = HabitatPropSource::LifetimeTokens { threshold: 1.0 };
      let vm = vm_with_props(vec![prop]);
      let profile = derive_room_life_profile(&vm, now);
      assert!(
          !profile
              .scene_moments
              .iter()
              .any(|m| m.key == SceneMomentKey::HeavySessionShimmer),
          "only a HeavySession-sourced planter shimmers"
      );
  }

  #[test]
  fn sleeping_room_does_not_shimmer_a_heavy_session() {
      let now = datetime!(2026-06-11 03:00 UTC);
      let mut vm = vm_with_props(vec![earned_heavy_session(now - Duration::minutes(5))]);
      vm.day_context.asleep = true;
      let profile = derive_room_life_profile(&vm, now);
      assert!(
          !profile
              .scene_moments
              .iter()
              .any(|m| m.key == SceneMomentKey::HeavySessionShimmer),
          "a sleeping room stays calm — no shimmer"
      );
  }
  ```
  Confirm imports at the top of the `tests` module include `HabitatPropSource` and `HEAVY_SESSION_PLANTER` and `Duration` (grep the module head; the existing `earned` helper already uses `HabitatPropSource`, and `HEAVY_SESSION_PLANTER` is used elsewhere in these tests). Run:
  ```bash
  cargo test -p glorp --lib tui::room::tests::fresh_heavy_session 2>&1 | tail -20
  ```
  Expect FAIL (no shimmer emitted yet).

- [ ] **Step 2: Add the freshness constant + the shimmer arm.** In `src/tui/room.rs`, add near the other module consts (top of file, after the existing `const`s):
  ```rust
  /// How recently the heavy-session planter must have been earned for the room
  /// to shimmer once in celebration. Keyed to `earned_at` so the one-shot
  /// trigger guard fires it exactly once, never on replay.
  const HEAVY_SESSION_SHIMMER_FRESH: Duration = Duration::minutes(20);
  ```
  Then in `scene_moments_for`, after the `DawnWakeWipe` block and before `moments` is returned (around line 439), add:
  ```rust
  if !vm.day_context.asleep {
      if let Some(planter) = vm.habitat.earned_props.iter().find(|p| {
          p.id.as_str() == HEAVY_SESSION_PLANTER
              && matches!(p.source, HabitatPropSource::HeavySession)
              && now - p.earned_at >= Duration::ZERO
              && now - p.earned_at <= HEAVY_SESSION_SHIMMER_FRESH
      }) {
          moments.push(SceneMoment {
              key: SceneMomentKey::HeavySessionShimmer,
              trigger_id: SceneTriggerId::new(format!(
                  "heavy:{}",
                  planter.earned_at.unix_timestamp()
              )),
              target_id: "watch.room.effect",
              duration_ms: 700,
              max_replay_age_ms: 3_600_000,
          });
      }
  }
  ```
  Confirm `HabitatPropSource` and `HEAVY_SESSION_PLANTER` are in scope in `room.rs` (grep: `grep -n "use.*HabitatPropSource\|HEAVY_SESSION_PLANTER\|use.*Duration" src/tui/room.rs` — `HEAVY_SESSION_PLANTER` is already referenced in `tags_for_prop`/`emitter_behavior_for_prop`, and `Duration` is used throughout; add a `use` for `HabitatPropSource` only if the grep shows it missing).
  Run the same test command. Expect PASS.

- [ ] **Step 3: Run all four new tests.**
  ```bash
  cargo test -p glorp --lib tui::room::tests:: 2>&1 | tail -25
  ```
  Expect all green, including the pre-existing `starter_room_has_no_emitter_or_scene_moments` (a starter room has no planter, so no shimmer).

- [ ] **Step 4: Confirm the effect engine renders it (integration sanity).** The effect is already wired in `effect_for_moment` (`animator.rs:815`) and routed by `update_scene_moments` for `target_id == "watch.room.effect"`. Add a focused animator test proving a `HeavySessionShimmer` moment enqueues an active effect:
  ```rust
  #[test]
  fn heavy_session_shimmer_enqueues_a_room_effect() {
      let mut animator = super::SceneEffectAnimator::default();
      let moment = crate::tui::room::SceneMoment {
          key: crate::tui::room::SceneMomentKey::HeavySessionShimmer,
          trigger_id: crate::tui::room::SceneTriggerId::new("heavy:123"),
          target_id: "watch.room.effect",
          duration_ms: 700,
          max_replay_age_ms: 3_600_000,
      };
      animator.update_scene_moments(std::slice::from_ref(&moment), &dummy_targets());
      assert!(animator.has_active_effects(), "shimmer must produce a live effect");
  }
  ```
  (Match the real animator type/constructor and the `has_active_effects`/active-count accessor used by the neighboring `scene_moment_*` tests — read `src/pet/animator.rs` lines ~1498-1535 and copy their exact helpers `dummy_targets()` and the active-effects assertion they use. If they assert via a different accessor, use that one; do NOT invent `has_active_effects` if it isn't there.)
  Run:
  ```bash
  cargo test -p glorp --lib pet::animator::tests::heavy_session_shimmer 2>&1 | tail -15
  ```
  Expect PASS.

- [ ] **Step 5: Full suite + clippy, commit.**
  ```bash
  cargo test -p glorp --lib 2>&1 | tail -15
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```
  Expect clean. Commit:
  ```bash
  git add src/tui/room.rs src/pet/animator.rs
  git commit -m "feat: shimmer the room on a fresh heavy-session unlock

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 6 — Phase acceptance: calm + narrow-width validation + full gate

Prove the phase's acceptance bar end-to-end: the pet is grounded, the scene stays calm, real-signal + zero-feast invariants hold, and everything renders at the real narrow pet-column width.

**Files:**
- Test: `src/tui/panels/pet.rs` (one acceptance test) + run the full suite.

- [ ] **Step 1: Add the calm-at-narrow-width acceptance test.** Add to the `tests` module in `src/tui/panels/pet.rs`:
  ```rust
  #[test]
  fn grounded_scene_stays_calm_at_narrow_column_width() {
      // The real pet column is ~40 wide. Render a full panel and assert the
      // scene is grounded (pet low) and calm (no excessive bright churn): the
      // contact shadow + floor wash are bg-only, so the count of non-blank
      // GLYPH cells stays bounded and the pet still reads.
      let vm = vm_with_real_pet();
      let panel = PetPanel;
      let ctx = test_context();
      let backend = TestBackend::new(40, 18);
      let mut terminal = Terminal::new(backend).unwrap();
      terminal
          .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
          .unwrap();
      let buf = terminal.backend().buffer();
      let glyph_cells: usize = (0..18u16)
          .flat_map(|y| (0..40u16).map(move |x| (x, y)))
          .filter(|&(x, y)| buf[(x, y)].symbol() != " ")
          .count();
      assert!(glyph_cells > 5, "pet + floor must render visible content");
      // Calm ceiling: a 40×18 = 720-cell panel should not be glyph-saturated.
      assert!(
          glyph_cells < 720 / 2,
          "scene must stay calm — fewer than half the cells carry glyphs; got {glyph_cells}"
      );
  }
  ```
  Run:
  ```bash
  cargo test -p glorp --lib tui::panels::pet::tests::grounded_scene_stays_calm 2>&1 | tail -15
  ```
  Expect PASS. (If the calm ceiling is too tight for the real fixture, read the actual count from the failure and set the bound from the observed value plus a small margin — do not loosen blindly; the point is to catch a future regression that floods the scene.)

- [ ] **Step 2: Run the full workspace test suite.**
  ```bash
  cargo test 2>&1 | tail -25
  ```
  Expect all green. In particular confirm these named invariants still pass:
  ```bash
  cargo test flat_and_immature_pets_render_zero_motes 2>&1 | tail -5
  cargo test starter_room_has_no_emitter_or_scene_moments 2>&1 | tail -5
  cargo test biome_wash_is_subtle_and_biome_distinct 2>&1 | tail -5
  ```

- [ ] **Step 3: Clippy gate + fmt.**
  ```bash
  cargo fmt --check 2>&1 | tail -5
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```
  Expect both clean. If `fmt --check` reports diffs, run `cargo fmt` and re-verify.

- [ ] **Step 4: Regenerate the preview lab and eyeball the grounding (manual review backstop).**
  ```bash
  cargo run -- dev-preview --scenario watch --out target/glorp-preview
  ```
  Open `target/glorp-preview/index.html` and confirm: the pet sits on the floor (not centered), the floor band reads darker than the sky, and a heavy-session fixture (if present in the watch scenario) shows the shimmer in its `scene_moments` list. This is a review checkpoint, not an automated assertion — note observations for the reviewer.

- [ ] **Step 5: Final commit for the phase.**
  ```bash
  git add src/tui/panels/pet.rs
  git commit -m "test: lock the grounded calm scene at narrow pet-column width

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Done criteria for Phase 5

- Pet anchors feet-relative; `pet_feet_anchor_y` lands the lowest art row one row above the floor; no panic on degenerate areas.
- A feet-restricted, bg-only contact shadow sits under the pet, never touching side-column gutter identity cells (precedence preserved).
- `biome_floor_wash_color` makes the floor band read darker than the sky; both stay subtle and biome-distinct.
- `TOKEN_PEBBLE_25K` front-loads at 10k lifetime tokens; the maturity gate and `flat_and_immature_pets_render_zero_motes` are untouched.
- `HeavySessionShimmer` fires once on a fresh `HabitatPropSource::HeavySession` planter, never on a stale or lifetime-sourced one, never while asleep; `DreamGlimmer` stays unemitted.
- Full `cargo test` green; `cargo clippy --all-targets --all-features -- -D warnings` clean; `cargo fmt --check` clean; preview lab reviewed.
