# Phase 4 — Liveliness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax.

**Goal:** Delete the dead/divergent breath fields so `species_breath_rhythm_decis` is the single breath source of truth, and make glitch corruption a loud, intentional, deterministic effect via a new `PaletteRoleName::Corruption` role that wins z-order over the underlying Eye/Mouth span at a corrupted cell — bounded in rate/footprint, never touching the eye-center, calm (no flashing).

**Architecture:** `render_pet` (`src/pet/render.rs`) builds 8 art rows + role-tagged `StyledSegment` spans, then `apply_glitch_corruption` mutates a few body/edge/face cells per tick for the Glitch species before the 13×10 particle frame wraps them. Corruption is a new palette role with a contrasting acid/phosphor color threaded through `palette.rs::role_color`, `ResolvedPalette`, `colors.rs`, and the presentation role-name table. Corruption is applied by **rewriting the span at the corrupted cell** (split the underlying span and insert a `Corruption` span) so the colored consumer (`art_lines.rs::build_owned_spans_for_line`, which walks spans left-to-right by `start` keeping the first non-overlapped segment) renders corruption on top — z-order is made explicit at the span level, not left to last-write-wins.

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

## Resolved decisions (binding for this phase)

- **Breath amplitude:** keep the binary 0/1 bob + per-species *period* only. NO structural multi-row change. This phase only **deletes** the dead `AnimationProfile.breath_period`/`breath_hold` fields; `animator.rs::species_breath_rhythm_decis` stays the single breath source of truth. Do NOT wire the dead fields. Do NOT add a multi-row amplitude knob.
- **Corruption z-order:** corruption wins over the underlying Eye/Mouth span at a corrupted cell, but **never** at the eye-center. Implemented by rewriting (splitting) the underlying span and inserting a `Corruption` span — explicit, not last-write-wins.
- **Corruption footprint/rate:** bounded. At most a small fixed number of corrupted cells per active tick; corruption only fires on a periodic gate (calm, not every frame); the face is touched only briefly and never the eye-center.
- **Heavier glyphs:** corruption may use `▒▓`-weight glyphs from `GLITCH_NOISE`. The existing `glitch_particles_stay_punctuation_sized` test asserts the *particle gutter* glyphs stay light — corruption is a body-cell effect, not a particle, so that test is unaffected and is NOT rewritten. (Verified: `apply_glitch_corruption` and `particles_for_species` are separate; the test only inspects `particles_for_species`.)

---

## Cross-phase interfaces

### Consumes from Phase 3 (palette/role plumbing — must already exist when this phase runs)

Phase 3 retunes hues/chroma and adds the mood→eye-color path. This phase adds one more role on top of that plumbing. The concrete surfaces this phase mutates already exist today and are stable across Phase 3:

```rust
// src/pet/render.rs
pub enum PaletteRoleName { Body, Eye, Mouth, Accent, Pattern, Particle }   // <- this phase ADDS Corruption

// src/pet/palette.rs — Phase 3 already added the `particle` field (role_color
// reads palette.particle, NOT palette.accent). This phase ADDS `corruption`.
pub struct ResolvedPalette { body, eye, mouth, accent, pattern, particle: Rgb }
pub fn role_color(role: PaletteRoleName, palette: &ResolvedPalette) -> Rgb;
pub fn default_theme_palette() -> ResolvedPalette;
pub fn resolve_pet_palette(species: Species, traits: &VisibleTraits) -> ResolvedPalette;

// src/tui/panels/pet/colors.rs
pub(crate) fn pet_role_style(role: PaletteRoleName, palette: &ResolvedPalette) -> Style;
pub(super) fn brighten_pet_role(base: &SemanticStyles, role: Option<PaletteRoleName>, multiplier: f32) -> SemanticStyles;
```

> NOTE for the implementer: if Phase 3 has already added `corruption`/`Corruption`, re-check before duplicating; this plan assumes the variant does NOT yet exist. If a `corruption` field/arm already exists, treat the corresponding task as a no-op-verify and move to the next.

### Produces for later phases / reconciler

```rust
// src/pet/render.rs — new role variant (Phase 5 contact-shadow precedence does NOT depend on it, but the enum is public surface)
pub enum PaletteRoleName { Body, Eye, Mouth, Accent, Pattern, Particle, Corruption }

// src/pet/palette.rs — new resolved field + arm (particle came from Phase 3)
pub struct ResolvedPalette { pub body, pub eye, pub mouth, pub accent, pub pattern, pub particle, pub corruption: Rgb }
// role_color gains: Corruption => palette.corruption

// src/pet/render.rs — corruption signature CHANGES from &mut [StyledSegment] to &mut Vec<StyledSegment>
// (it now splits/inserts spans, not just edits glyphs in place).
fn apply_glitch_corruption(lines: &mut [String], spans: &mut Vec<StyledSegment>, tick: u64);

// AnimationProfile loses two fields:
pub struct AnimationProfile { pub blink_average: u8, pub blink_jitter: u8 }
```

No later phase consumes the corruption span-rewrite internals. The only externally visible change is the new `Corruption` enum variant, which Phase 5 and the reconciler see when matching `PaletteRoleName` exhaustively.

---

## File Structure

- **`src/pet/render.rs`** (modify) — delete `breath_period`/`breath_hold` from `AnimationProfile` + `species_animation_profile`; add `Corruption` to `PaletteRoleName`; rewrite `apply_glitch_corruption` to be a bounded, deterministic, span-splitting effect that never hits the eye-center; add `corruption_cells_for_tick` (deterministic cell selector) + a face/eye-center guard.
- **`src/pet/palette.rs`** (modify) — add `corruption: Rgb` to `ResolvedPalette`; add `Corruption =>` arm to `role_color`; populate `corruption` in `default_theme_palette` and `resolve_pet_palette`; extend `default_theme_matches_current_fixed_colors`; extend the `traits_with_hue` helper is untouched (it builds `VisibleTraits`, not `ResolvedPalette`).
- **`src/tui/panels/pet/colors.rs`** (modify) — add `Corruption` arm to `brighten_pet_role` (folds into accent, like `Particle`).
- **`src/presentation/pet.rs`** (modify) — add `Corruption => "corruption"` arm to `role_name`.
- **Verify-only (wildcard arms, no change needed):** `src/menubar/render.rs::role_color_base` (delegates to `role_color`), `src/round/preview.rs::flat_role_name` (has `_` arm). The implementer confirms these compile after the variant is added; they are NOT edited.

Test paths: all unit tests live inline in the modified `src/` files (`#[cfg(test)] mod tests`).

---

## Task 1 — Delete the dead/divergent breath fields

**Files:**
- Modify: `src/pet/render.rs`
- Test: inline `#[cfg(test)] mod tests` in `src/pet/render.rs`; existing `src/pet/animator.rs` breath tests are the regression backstop (run, do not edit).

**Interfaces:**
- Produces: `pub struct AnimationProfile { pub blink_average: u8, pub blink_jitter: u8 }` (loses `breath_period`, `breath_hold`).
- Consumes: nothing new. `species_breath_rhythm_decis` (`animator.rs:510`) remains the single breath source of truth (unchanged).

**Steps:**

- [ ] Confirm `breath_period`/`breath_hold` have no readers. Run:
  ```bash
  grep -rn "breath_period\|breath_hold" src/
  ```
  Expected: only matches in `src/pet/render.rs` (the struct def at `:104-105`, and the six `species_animation_profile` literals at `:176-209`). If any OTHER file reads these fields, STOP and report — the plan assumed dead fields.

- [ ] Write a guard test that pins the slimmed struct and that breath still varies per species via the animator. Add to `src/pet/render.rs` `mod tests`:
  ```rust
  #[test]
  fn animation_profile_has_no_breath_fields() {
      // Construction must compile with ONLY blink fields — proves the dead
      // divergent breath table is gone and animator.rs owns breath alone.
      let p = AnimationProfile {
          blink_average: 30,
          blink_jitter: 10,
      };
      assert_eq!(p.blink_average, 30);
      assert_eq!(p.blink_jitter, 10);
  }
  ```

- [ ] Run it; expect a COMPILE FAILURE (the struct still has `breath_period`/`breath_hold`, so the literal is missing fields):
  ```bash
  cargo test --lib pet::render::tests::animation_profile_has_no_breath_fields 2>&1 | tail -20
  ```
  Expected: `error[E0063]: missing fields \`breath_period\` and \`breath_hold\` in initializer of \`AnimationProfile\``.

- [ ] Remove the two fields from the struct definition. In `src/pet/render.rs`, change:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct AnimationProfile {
      pub breath_period: u8,
      pub breath_hold: u8,
      pub blink_average: u8,
      pub blink_jitter: u8,
  }
  ```
  to:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct AnimationProfile {
      pub blink_average: u8,
      pub blink_jitter: u8,
  }
  ```

- [ ] Remove the `breath_period`/`breath_hold` lines from all six arms of `species_animation_profile` (`src/pet/render.rs:174-212`). After the edit each arm reads e.g.:
  ```rust
      Species::Fuzz => AnimationProfile {
          blink_average: 32,
          blink_jitter: 12,
      },
  ```
  Repeat for Blob (`blink_average: 40, blink_jitter: 14`), Ghost (`50, 18`), Glitch (`24, 8`), Crystal (`60, 22`), Mech (`22, 6`). Keep the exact blink values already present — only the two breath lines are removed from each arm.

- [ ] Run the new test; expect PASS:
  ```bash
  cargo test --lib pet::render::tests::animation_profile_has_no_breath_fields 2>&1 | tail -5
  ```
  Expected: `test result: ok. 1 passed`.

- [ ] Run the animator breath regression tests to prove the single source of truth still works:
  ```bash
  cargo test --lib pet::animator::tests::breath 2>&1 | tail -10
  ```
  Expected: `breath_offset_returns_zero_or_one`, `breath_rhythm_differs_per_species`, `breath_periods_match_pet_jsx_ordering` all pass.

- [ ] Full render + clippy check on the touched file:
  ```bash
  cargo test --lib pet::render 2>&1 | tail -5
  cargo clippy --lib -- -D warnings 2>&1 | tail -5
  ```
  Expected: tests ok; clippy clean.

- [ ] Commit:
  ```bash
  git status
  git add src/pet/render.rs
  git commit -m "$(cat <<'EOF'
refactor: delete dead AnimationProfile breath fields

species_breath_rhythm_decis in animator.rs is the single breath source
of truth; the divergent breath_period/breath_hold table was dead.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
  ```

---

## Task 2 — Add the `Corruption` palette role variant + color field

**Files:**
- Modify: `src/pet/render.rs` (enum), `src/pet/palette.rs` (field + role_color + populate), `src/tui/panels/pet/colors.rs` (brighten arm), `src/presentation/pet.rs` (role_name arm)
- Test: inline tests in `src/pet/palette.rs`

**Interfaces:**
- Produces: `PaletteRoleName::Corruption`; `ResolvedPalette { ..., pub corruption: Rgb }`; `role_color(Corruption, p) -> p.corruption`.
- Consumes: Phase 3's `ResolvedPalette` shape, `role_color`, `default_theme_palette`, `resolve_pet_palette` (above).

**Steps:**

- [ ] Write the failing test pinning the new role + a contrasting color. Add to `src/pet/palette.rs` `mod tests`:
  ```rust
  #[test]
  fn corruption_role_resolves_to_a_contrasting_acid_color() {
      use crate::pet::generation::Species;
      use crate::pet::render::PaletteRoleName::Corruption;
      let p = resolve_pet_palette(Species::Glitch, &traits_with_hue(50));
      let c = role_color(Corruption, &p);
      // Acid/phosphor: green dominant, distinct from the body so corruption
      // never melts into its background (Appendix B failure mode #3).
      assert!(c.g > c.r && c.g > c.b, "corruption not acid-green: {c:?}");
      assert_ne!(c, p.body, "corruption must contrast the body");
  }

  #[test]
  fn default_theme_has_a_corruption_color() {
      use crate::pet::render::PaletteRoleName::Corruption;
      let p = default_theme_palette();
      let c = role_color(Corruption, &p);
      assert!(c.g > c.r && c.g > c.b, "default corruption not acid-green: {c:?}");
  }
  ```

- [ ] Run; expect COMPILE FAILURE (`Corruption` does not exist):
  ```bash
  cargo test --lib pet::palette::tests::corruption_role 2>&1 | tail -20
  ```
  Expected: `error[E0599]: no variant or associated item named \`Corruption\` found for enum \`PaletteRoleName\``.

- [ ] Add the variant. In `src/pet/render.rs` change:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PaletteRoleName {
      Body,
      Eye,
      Mouth,
      Accent,
      Pattern,
      Particle,
  }
  ```
  to add `Corruption,` as the last variant:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PaletteRoleName {
      Body,
      Eye,
      Mouth,
      Accent,
      Pattern,
      Particle,
      Corruption,
  }
  ```

- [ ] Add the `corruption` field to `ResolvedPalette`. In `src/pet/palette.rs` change:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct ResolvedPalette {
      pub body: Rgb,
      pub eye: Rgb,
      pub mouth: Rgb,
      pub accent: Rgb,
      pub pattern: Rgb,
  }
  ```
  to add `pub corruption: Rgb,` after `pattern`.

- [ ] Add the `role_color` arm. In `src/pet/palette.rs::role_color`, add before the closing brace of the match (alongside the existing `Particle => palette.accent,`):
  ```rust
          PaletteRoleName::Corruption => palette.corruption,
  ```

- [ ] Populate `default_theme_palette`. In `src/pet/palette.rs::default_theme_palette`, add after `pattern: Rgb::new(0x50, 0x4c, 0x49),`:
  ```rust
          corruption: Rgb::new(0x78, 0xff, 0xb4),
  ```
  (Acid phosphor — `g` dominant, distinct from the cream body.)

- [ ] Populate `resolve_pet_palette`. In `src/pet/palette.rs::resolve_pet_palette`, add to the returned struct after `pattern: role(0.64, 0.20, h + 210.0),`:
  ```rust
          // Corruption is species-independent acid/phosphor, fixed-hue so it
          // always contrasts the body and reads as a deliberate data glitch,
          // not a tint of the creature. High chroma green at high lightness.
          corruption: oklch_to_rgb(0.85, 0.22, 145.0),
  ```
  > Rationale: `role(...)` would tie corruption to the per-pet body hue and could collide with the Glitch body (acid green). A fixed independent hue/lightness guarantees the ≥-distinct-from-body contract for every seed. The `g`-dominant assertion holds because hue 145 is green.

- [ ] Extend `default_theme_matches_current_fixed_colors`. In `src/pet/palette.rs::default_theme_matches_current_fixed_colors`, add after the `Particle` assertion:
  ```rust
          assert_eq!(role_color(Corruption, &p), Rgb::new(0x78, 0xff, 0xb4));
  ```

- [ ] Add the `brighten_pet_role` arm. In `src/tui/panels/pet/colors.rs::brighten_pet_role`, add after the `Particle` arm (corruption folds into accent like Particle does):
  ```rust
          PaletteRoleName::Corruption => s.pet_accent = brighten_style(s.pet_accent, multiplier),
  ```

- [ ] Add the `role_name` arm. In `src/presentation/pet.rs::role_name`, add after `PaletteRoleName::Particle => "particle",`:
  ```rust
          PaletteRoleName::Corruption => "corruption",
  ```

- [ ] Run the new tests; expect PASS:
  ```bash
  cargo test --lib pet::palette::tests 2>&1 | tail -10
  ```
  Expected: `corruption_role_resolves_to_a_contrasting_acid_color`, `default_theme_has_a_corruption_color`, `default_theme_matches_current_fixed_colors` all pass.

- [ ] Confirm the whole crate still compiles (catches any exhaustive match that needs the new variant — menubar/round use wildcards and should be fine, but build proves it):
  ```bash
  cargo build 2>&1 | tail -20
  ```
  Expected: clean build, no E0004 (non-exhaustive match) errors. If E0004 appears for a file not in this plan's "verify-only" list, add the `Corruption` arm minimally and note it.

- [ ] Clippy:
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```
  Expected: clean.

- [ ] Commit:
  ```bash
  git status
  git add src/pet/render.rs src/pet/palette.rs src/tui/panels/pet/colors.rs src/presentation/pet.rs
  git commit -m "$(cat <<'EOF'
feat: add Corruption palette role wired through role_color/palette/colors

A contrasting acid/phosphor role so glitch corruption never melts into
its background. Threaded through ResolvedPalette, role_color,
default_theme_palette, resolve_pet_palette, brighten_pet_role, and the
presentation role-name table.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
  ```

---

## Task 3 — Deterministic bounded corruption-cell selector + eye-center guard

This task adds the pure helpers that decide *which* cells corrupt on a given tick, with all the bounds (rate, footprint, never-eye-center) baked into testable functions — **before** rewiring `apply_glitch_corruption`. This keeps the bounds independently testable.

**Files:**
- Modify: `src/pet/render.rs`
- Test: inline tests in `src/pet/render.rs`

**Interfaces:**
- Produces (private to render.rs):
  ```rust
  /// Up to CORRUPTION_MAX_CELLS deterministic (row, col) cells to corrupt this
  /// tick. Empty when the corruption gate is closed (calm — corruption only
  /// fires periodically, never every frame). `art_lines` is the pre-frame 8 art
  /// rows (so callers select within the creature, not the gutter).
  fn corruption_cells_for_tick(art_lines: &[String], tick: u64) -> Vec<(usize, usize)>;

  /// True if (row, col) is the center cell of an Eye span on that row. The
  /// living-face rule: corruption may briefly touch the face but NEVER the
  /// eye-center.
  fn is_eye_center(spans: &[StyledSegment], row: usize, col: usize) -> bool;

  const CORRUPTION_GATE_TICKS: u64 = 13;   // fires roughly every 13 ticks (calm)
  const CORRUPTION_MAX_CELLS: usize = 3;   // bounded footprint per active tick
  ```
- Consumes: nothing new.

**Steps:**

- [ ] Add the constants and the eye-center helper test first. Add to `src/pet/render.rs` `mod tests`:
  ```rust
  #[test]
  fn is_eye_center_is_true_only_at_the_middle_of_an_eye_span() {
      // Eye span on row 2 covering cols 3..6 ("o o"): center is col 4.
      let spans = vec![StyledSegment {
          line: 2,
          start: 3,
          end: 6,
          role: PaletteRoleName::Eye,
      }];
      assert!(is_eye_center(&spans, 2, 4), "col 4 is the eye-center");
      assert!(!is_eye_center(&spans, 2, 3), "col 3 is an eye edge, not center");
      assert!(!is_eye_center(&spans, 2, 5), "col 5 is an eye edge, not center");
      assert!(!is_eye_center(&spans, 1, 4), "wrong row");
      assert!(!is_eye_center(&spans, 2, 7), "outside the span");
  }
  ```

- [ ] Run; expect COMPILE FAILURE (`is_eye_center` undefined):
  ```bash
  cargo test --lib pet::render::tests::is_eye_center_is_true_only 2>&1 | tail -10
  ```
  Expected: `error[E0425]: cannot find function \`is_eye_center\``.

- [ ] Implement the constants and `is_eye_center`. In `src/pet/render.rs`, add near `GLITCH_NOISE` (just below it):
  ```rust
  /// Corruption fires on a periodic gate so it reads as a calm, intentional
  /// data-glitch rather than a constant flicker.
  const CORRUPTION_GATE_TICKS: u64 = 13;
  /// Bounded footprint: at most this many cells corrupt on an active tick.
  const CORRUPTION_MAX_CELLS: usize = 3;
  ```
  And add the helper near `apply_glitch_corruption`:
  ```rust
  /// The living-face rule: corruption may briefly touch the face but NEVER the
  /// eye-center. Returns true when (row, col) is the middle cell of an Eye span
  /// on that row. For even-width eye spans, both middle cells are protected.
  fn is_eye_center(spans: &[StyledSegment], row: usize, col: usize) -> bool {
      spans.iter().any(|span| {
          if span.line != row || span.role != PaletteRoleName::Eye {
              return false;
          }
          let width = span.end.saturating_sub(span.start);
          if width == 0 {
              return false;
          }
          let mid_lo = span.start + (width - 1) / 2;
          let mid_hi = span.start + width / 2;
          col >= mid_lo && col <= mid_hi
      })
  }
  ```

- [ ] Run; expect PASS:
  ```bash
  cargo test --lib pet::render::tests::is_eye_center_is_true_only 2>&1 | tail -5
  ```

- [ ] Write the cell-selector tests (gate, bounds, determinism, non-space). Add to `src/pet/render.rs` `mod tests`:
  ```rust
  #[test]
  fn corruption_gate_is_closed_most_ticks() {
      let lines: Vec<String> = vec!["###########".to_string(); 8];
      let active = (0..130_u64)
          .filter(|&t| !corruption_cells_for_tick(&lines, t).is_empty())
          .count();
      // Calm: corruption is the quiet exception, not the rule.
      assert!(active > 0, "corruption must fire sometimes");
      assert!(
          active <= 130 / CORRUPTION_GATE_TICKS as usize + 1,
          "corruption fired {active}/130 ticks — too noisy"
      );
  }

  #[test]
  fn corruption_footprint_is_bounded() {
      let lines: Vec<String> = vec!["###########".to_string(); 8];
      for t in 0..200_u64 {
          let cells = corruption_cells_for_tick(&lines, t);
          assert!(
              cells.len() <= CORRUPTION_MAX_CELLS,
              "tick {t} produced {} cells, over the cap",
              cells.len()
          );
      }
  }

  #[test]
  fn corruption_cells_are_deterministic() {
      let lines: Vec<String> = vec!["#### ## ####".chars().take(11).collect(); 8];
      let a = corruption_cells_for_tick(&lines, 26);
      let b = corruption_cells_for_tick(&lines, 26);
      assert_eq!(a, b, "same tick must give same cells");
  }

  #[test]
  fn corruption_cells_skip_blank_cells_and_stay_in_bounds() {
      // Sparse art: only a few non-space cells. Selector must never pick a
      // space cell or an out-of-grid cell.
      let lines: Vec<String> = vec![
          "   ###     ".to_string(),
          "           ".to_string(),
          "  #     #  ".to_string(),
          "           ".to_string(),
          "           ".to_string(),
          "           ".to_string(),
          "           ".to_string(),
          "           ".to_string(),
      ];
      for t in 0..300_u64 {
          for (row, col) in corruption_cells_for_tick(&lines, t) {
              assert!(row < lines.len(), "row {row} out of bounds at tick {t}");
              let ch = lines[row].chars().nth(col).unwrap();
              assert_ne!(ch, ' ', "tick {t} picked a blank cell ({row},{col})");
          }
      }
  }

  #[test]
  fn corruption_cells_empty_for_all_blank_art() {
      let lines: Vec<String> = vec!["           ".to_string(); 8];
      for t in 0..40_u64 {
          assert!(
              corruption_cells_for_tick(&lines, t).is_empty(),
              "blank art must never corrupt (tick {t})"
          );
      }
  }
  ```

- [ ] Run; expect COMPILE FAILURE (`corruption_cells_for_tick` undefined):
  ```bash
  cargo test --lib pet::render::tests::corruption_ 2>&1 | tail -10
  ```
  Expected: `error[E0425]: cannot find function \`corruption_cells_for_tick\``.

- [ ] Implement `corruption_cells_for_tick`. Add to `src/pet/render.rs` near `apply_glitch_corruption`:
  ```rust
  /// Deterministic, bounded set of non-blank art cells to corrupt this tick.
  /// Returns empty when the periodic gate is closed (calm). Selection walks the
  /// non-blank cells of the 8 art rows and picks up to CORRUPTION_MAX_CELLS via
  /// a tick-seeded stride, so the footprint reshuffles across base/edge/face
  /// cells over time without ever exceeding the cap.
  fn corruption_cells_for_tick(art_lines: &[String], tick: u64) -> Vec<(usize, usize)> {
      if !tick.is_multiple_of(CORRUPTION_GATE_TICKS) {
          return Vec::new();
      }
      // Collect every non-blank cell of the art rows.
      let mut candidates: Vec<(usize, usize)> = Vec::new();
      for (row, line) in art_lines.iter().enumerate().take(8) {
          for (col, ch) in line.chars().enumerate().take(ART_WIDTH) {
              if ch != ' ' {
                  candidates.push((row, col));
              }
          }
      }
      if candidates.is_empty() {
          return Vec::new();
      }
      let count = CORRUPTION_MAX_CELLS.min(candidates.len());
      // Tick-seeded start + odd stride so successive active ticks land on
      // different cells (reshuffle) while staying deterministic per tick.
      let n = candidates.len();
      let gate = tick / CORRUPTION_GATE_TICKS;
      let start = (gate.wrapping_mul(7) as usize) % n;
      let stride = ((gate.wrapping_mul(11) as usize) % n) | 1; // always odd, >=1
      let mut out = Vec::with_capacity(count);
      let mut idx = start;
      for _ in 0..count {
          out.push(candidates[idx % n]);
          idx += stride;
      }
      out.sort_unstable();
      out.dedup();
      out
  }
  ```

- [ ] Run all corruption-selector tests; expect PASS:
  ```bash
  cargo test --lib pet::render::tests::corruption_ 2>&1 | tail -10
  ```
  Expected: all five pass.

- [ ] Clippy on the file:
  ```bash
  cargo clippy --lib -- -D warnings 2>&1 | tail -5
  ```
  Expected: clean. (If clippy flags `corruption_cells_for_tick` or `is_eye_center` as dead code because nothing calls them yet, that is expected — they get wired in Task 4 in the SAME branch. To keep this commit clippy-clean, proceed directly to Task 4 before running the `--all-targets` gate, OR temporarily mark them `#[allow(dead_code)]` and remove the allow in Task 4. Prefer wiring in Task 4 within the same session; commit this task only after confirming `cargo clippy --lib` passes — the lib check does not flag unused-but-pub-crate fns the same way `--all-targets` does. If `cargo clippy --lib` DOES flag them, add `#[allow(dead_code)]` to both fns now and delete it in Task 4.)

- [ ] Commit:
  ```bash
  git status
  git add src/pet/render.rs
  git commit -m "$(cat <<'EOF'
feat: deterministic bounded corruption-cell selector + eye-center guard

corruption_cells_for_tick gates corruption to a calm cadence, caps the
footprint, reshuffles across cells, and only picks non-blank art cells.
is_eye_center protects the living-face eye-center.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
  ```

---

## Task 4 — Rewire `apply_glitch_corruption` to a loud, span-rewriting, living-face-safe effect

Replace the old body-only single-cell glyph swap with the bounded multi-cell effect that (a) corrupts base/edge/face cells, (b) NEVER the eye-center, (c) rewrites the underlying span so corruption WINS z-order over Eye/Mouth at the corrupted cell, (d) recolors via the `Corruption` role.

**Files:**
- Modify: `src/pet/render.rs`
- Test: inline tests in `src/pet/render.rs`

**Interfaces:**
- Produces: `fn apply_glitch_corruption(lines: &mut [String], spans: &mut Vec<StyledSegment>, tick: u64)` (signature CHANGED: `spans` is now `&mut Vec<StyledSegment>` so it can split/insert).
- Consumes: `corruption_cells_for_tick`, `is_eye_center` (Task 3); `GLITCH_NOISE`; `PaletteRoleName::Corruption` (Task 2).

**Steps:**

- [ ] Write the failing behavior tests. Add to `src/pet/render.rs` `mod tests`:
  ```rust
  use crate::pet::generation::Species;

  /// At a tick where the corruption gate is open, a Glitch pet shows at least
  /// one Corruption-role span over a cell that was previously Eye or Mouth or
  /// Body — proving corruption is loud (recolored) and z-wins, not a silent
  /// in-place glyph edit.
  #[test]
  fn glitch_corruption_emits_corruption_role_spans_on_active_tick() {
      let pet = generate_pet("corrupt-seed").with_species(Species::Glitch);
      // CORRUPTION_GATE_TICKS == 13; tick 13 opens the gate.
      let frame = AnimationFrame {
          tick: 13,
          ..AnimationFrame::default()
      };
      let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
      let has_corruption = rendered
          .spans
          .iter()
          .any(|s| s.role == PaletteRoleName::Corruption);
      assert!(
          has_corruption,
          "active corruption tick must emit Corruption-role spans"
      );
  }

  #[test]
  fn glitch_corruption_never_recolors_the_eye_center() {
      let pet = generate_pet("corrupt-eye-seed").with_species(Species::Glitch);
      // Scan many gate-open ticks; the eye-center must never become Corruption.
      for gate in 1..400_u64 {
          let tick = gate * CORRUPTION_GATE_TICKS;
          let frame = AnimationFrame {
              tick,
              ..AnimationFrame::default()
          };
          // Re-derive the pre-frame spans the same way render_pet does, then
          // run the public render and check no Corruption span lands on an
          // eye-center cell (in the pre-frame coordinate space).
          let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
          // Find the Eye spans (post-frame coords: render adds +1 to line/col).
          let eye_spans: Vec<_> = rendered
              .spans
              .iter()
              .filter(|s| s.role == PaletteRoleName::Eye)
              .cloned()
              .collect();
          for c in rendered.spans.iter().filter(|s| s.role == PaletteRoleName::Corruption) {
              for e in &eye_spans {
                  if c.line == e.line {
                      let width = e.end.saturating_sub(e.start);
                      let mid_lo = e.start + (width.saturating_sub(1)) / 2;
                      let mid_hi = e.start + width / 2;
                      // Corruption span is width-1; its start is the cell.
                      let col = c.start;
                      assert!(
                          !(col >= mid_lo && col <= mid_hi),
                          "tick {tick}: corruption hit the eye-center at line {} col {col}",
                          c.line
                      );
                  }
              }
          }
      }
  }

  #[test]
  fn non_glitch_species_never_corrupt() {
      for species in [
          Species::Fuzz,
          Species::Blob,
          Species::Ghost,
          Species::Crystal,
          Species::Mech,
      ] {
          let pet = generate_pet("no-corrupt").with_species(species);
          let frame = AnimationFrame {
              tick: 13,
              ..AnimationFrame::default()
          };
          let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
          assert!(
              !rendered
                  .spans
                  .iter()
                  .any(|s| s.role == PaletteRoleName::Corruption),
              "{species:?} must never corrupt"
          );
      }
  }

  #[test]
  fn glitch_corruption_quiet_off_gate_tick() {
      let pet = generate_pet("corrupt-seed").with_species(Species::Glitch);
      // Tick 1 is not a multiple of CORRUPTION_GATE_TICKS (13): gate closed.
      let frame = AnimationFrame {
          tick: 1,
          ..AnimationFrame::default()
      };
      let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
      assert!(
          !rendered
              .spans
              .iter()
              .any(|s| s.role == PaletteRoleName::Corruption),
          "off-gate tick must stay calm (no corruption)"
      );
  }
  ```
  > NOTE: `generate_pet` and `Species` are already imported in the test module (`generate_pet` at the top of `mod tests`, `Species` used in existing tests via local `use`). Add `use crate::pet::generation::Species;` at the top of `mod tests` only if not already present — check first; `glitch_particles_stay_punctuation_sized` already references `Species::Glitch` via the module-level `use super::*;` + `generation::Species` path, so a local `use` may already exist. If a duplicate-import error occurs, drop the added `use`.

- [ ] Run; expect FAILURE. The new effect isn't wired, so `glitch_corruption_emits_corruption_role_spans_on_active_tick` fails (no Corruption spans), and the signature still takes `&mut [StyledSegment]`:
  ```bash
  cargo test --lib pet::render::tests::glitch_corruption 2>&1 | tail -20
  ```
  Expected: assertion failure `active corruption tick must emit Corruption-role spans` (and/or related).

- [ ] Rewrite `apply_glitch_corruption`. Replace the entire current function body (`src/pet/render.rs:430-461`) with:
  ```rust
  /// Glitch corruption: a bounded, deterministic, loud data-glitch effect.
  ///
  /// On a periodic gate (calm — not every frame) it corrupts up to
  /// CORRUPTION_MAX_CELLS non-blank art cells. Each corrupted cell:
  ///   * swaps its glyph to a contrasting GLITCH_NOISE glyph (may be ▒▓ weight),
  ///   * is re-tagged as the Corruption role by SPLITTING the underlying span,
  ///     so corruption wins z-order over Eye/Mouth/Body at that cell.
  /// The eye-center is never corrupted (living-face rule).
  fn apply_glitch_corruption(lines: &mut [String], spans: &mut Vec<StyledSegment>, tick: u64) {
      let cells = corruption_cells_for_tick(lines, tick);
      if cells.is_empty() {
          return;
      }
      for (i, (row, col)) in cells.into_iter().enumerate() {
          if is_eye_center(spans, row, col) {
              continue; // living-face: never the eye-center
          }
          // Pick a noise glyph deterministically per (tick, cell index).
          let noise = GLITCH_NOISE[((tick as usize).wrapping_mul(3).wrapping_add(i)) % GLITCH_NOISE.len()];
          replace_char_in_line(&mut lines[row], col, noise);
          retag_cell_as_corruption(spans, row, col);
      }
  }

  /// Re-tag the single cell (row, col) as the Corruption role. If an existing
  /// span covers the cell, it is split around the cell so the surrounding cells
  /// keep their original role and the cell itself becomes a width-1 Corruption
  /// span. If no span covers the cell, a standalone Corruption span is added.
  fn retag_cell_as_corruption(spans: &mut Vec<StyledSegment>, row: usize, col: usize) {
      let mut split: Vec<StyledSegment> = Vec::new();
      let mut covered = false;
      for span in spans.iter_mut() {
          if span.line != row || col < span.start || col >= span.end {
              continue;
          }
          covered = true;
          let original_end = span.end;
          let original_role = span.role;
          // Left fragment: keep original role for [start, col).
          // Shrink this span to the left fragment (possibly empty -> filtered later).
          span.end = col;
          // Right fragment: keep original role for (col, end).
          if col + 1 < original_end {
              split.push(StyledSegment {
                  line: row,
                  start: col + 1,
                  end: original_end,
                  role: original_role,
              });
          }
          break;
      }
      // The corrupted cell itself.
      split.push(StyledSegment {
          line: row,
          start: col,
          end: col + 1,
          role: PaletteRoleName::Corruption,
      });
      if !covered {
          // No span covered the cell (a body-gap cell). Just add the corruption
          // span; the consumer renders uncovered cells as body, and this span
          // overrides that single cell.
      }
      // Drop any now-empty left fragments produced by the shrink.
      spans.retain(|s| s.start < s.end);
      spans.extend(split);
  }
  ```
  > Implementation note on z-order: `art_lines.rs::build_owned_spans_for_line` sorts spans by `start` and walks with a `cursor`, keeping the first non-overlapped segment and skipping any whose `end <= cursor`. By making the Corruption span exactly width-1 at `col` and shrinking/splitting the underlying span so it no longer covers `col`, the corruption span is the unique covering span at that cell — it always renders. This is the explicit z-order the contract requires (corruption > Eye/Mouth at a corrupted cell).

- [ ] The call site at `src/pet/render.rs:149` already passes `&mut spans` where `spans: Vec<StyledSegment>`; `&mut Vec<...>` coerces. No call-site change is needed, but confirm the line reads:
  ```rust
      if pet.species == Species::Glitch {
          apply_glitch_corruption(&mut lines, &mut spans, frame.tick);
      }
  ```

- [ ] Run the new behavior tests; expect PASS:
  ```bash
  cargo test --lib pet::render::tests::glitch_corruption 2>&1 | tail -15
  cargo test --lib pet::render::tests::non_glitch_species_never_corrupt 2>&1 | tail -5
  ```
  Expected: all pass.

- [ ] Run the FULL render test module to catch regressions (blink, soft-eyes, ecstatic, particle-size test):
  ```bash
  cargo test --lib pet::render 2>&1 | tail -15
  ```
  Expected: all pass, including `glitch_particles_stay_punctuation_sized` (untouched — it inspects `particles_for_species`, not corruption).

- [ ] If Task 3 added `#[allow(dead_code)]` to `corruption_cells_for_tick`/`is_eye_center`, remove it now (they have callers). Then run the full gate:
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```
  Expected: clean.

- [ ] Run the broader pet + preview suites to be sure nothing downstream broke on the new role/spans:
  ```bash
  cargo test --lib pet 2>&1 | tail -10
  cargo test --features dev-preview --test dev_preview 2>&1 | tail -15
  ```
  Expected: all pass. (Glitch preview fixtures in `dev_preview/pets.rs` render with `tick` values from their own fixtures; a Corruption span only appears on gate ticks and is colored via the resolved palette — no panic, valid 11×8.)

- [ ] Commit:
  ```bash
  git status
  git add src/pet/render.rs
  git commit -m "$(cat <<'EOF'
feat: loud span-rewriting glitch corruption that spares the eye-center

apply_glitch_corruption now corrupts up to 3 cells on a calm gate, swaps
their glyphs to contrasting noise, and splits the underlying span so a
width-1 Corruption span wins z-order over Eye/Mouth/Body at that cell.
The eye-center is never corrupted (living-face rule). Deterministic per
tick; non-Glitch species never corrupt.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
  ```

---

## Task 5 — Style.rs Flat-degrade coverage for the Corruption role

The contract says the corruption role must be "covered under Flat" in the degrade path. The pet render path is truecolor-first: under `Flat` the watch surface does not seed the per-pet palette and ratatui downgrades RGB automatically; there is no per-role Flat branch in `style.rs` to extend (verified — `style.rs` has no per-`PaletteRoleName` match). The Flat coverage that DOES exist and CAN regress is the round-companion `flat_role_name` and any exhaustive-but-wildcard match. This task adds a guard test proving Corruption degrades cleanly (folds into the existing wildcard) rather than panicking or being mis-categorized.

**Files:**
- Modify: `src/round/preview.rs` (test only) OR `src/presentation/pet.rs` (test only) — pick the file that already imports `PaletteRoleName`. Use `src/round/preview.rs`.
- Test: inline test in `src/round/preview.rs`

**Interfaces:**
- Consumes: `flat_role_name(PaletteRoleName) -> &'static str` (`round/preview.rs:165`, has a `_` wildcard arm).
- Produces: nothing new (guard only).

**Steps:**

- [ ] Confirm `flat_role_name` has a wildcard arm so Corruption is handled:
  ```bash
  sed -n '165,171p' src/round/preview.rs
  ```
  Expected: the `_ => "white",` wildcard catches Corruption.

- [ ] Add a guard test pinning Corruption's Flat degrade to the neutral/white bucket (it must NOT be silently miscategorized as green/yellow). Add to `src/round/preview.rs` `mod tests` (find the existing `#[cfg(test)] mod tests` block; if none, add one at the end of the file mirroring the crate's style):
  ```rust
  #[test]
  fn corruption_role_degrades_to_neutral_under_flat() {
      use crate::pet::render::PaletteRoleName;
      // Under Flat the round companion carries the pet by silhouette; the
      // contrasting corruption color is gone, so corruption must read as a
      // neutral cell, not be mistaken for an eye (green) or accent (yellow).
      assert_eq!(flat_role_name(PaletteRoleName::Corruption), "white");
  }
  ```
  > If `src/round/preview.rs` has no `mod tests`, instead add this test to `src/presentation/pet.rs` `mod tests` keyed on `role_name(Corruption) == "corruption"` (already added in Task 2) AND additionally verify the round file compiles. Prefer the round file because it owns the Flat name path. Check first:
  ```bash
  grep -n "mod tests" src/round/preview.rs
  ```

- [ ] Run; expect PASS (the wildcard already returns `"white"`, so this is a pinning/regression test that should pass immediately — it guards future edits to `flat_role_name`):
  ```bash
  cargo test --lib round::preview 2>&1 | tail -10
  ```
  Expected: `corruption_role_degrades_to_neutral_under_flat` passes.
  > If `round::preview` tests are not compiled into the default `--lib` target (the round module may be feature-gated), discover the right invocation:
  ```bash
  cargo test corruption_role_degrades_to_neutral_under_flat 2>&1 | tail -15
  ```

- [ ] Full gate:
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
  ```
  Expected: clean.

- [ ] Commit:
  ```bash
  git status
  git add src/round/preview.rs
  git commit -m "$(cat <<'EOF'
test: pin Corruption role Flat degrade to neutral

Guards against future flat_role_name edits silently miscoloring
corruption as an eye or accent under NO_COLOR.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
  ```

---

## Task 6 — Phase acceptance: full suite + visual confirmation

**Files:** none (verification only).

**Steps:**

- [ ] Run the entire test suite. Expected: green.
  ```bash
  cargo test 2>&1 | tail -25
  ```

- [ ] Run the full clippy gate. Expected: clean.
  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```

- [ ] Regenerate the preview lab and eyeball the Glitch column (corruption should read as intentional acid-green specks that never blank the eye-center, calm cadence; the rest of the roster unchanged):
  ```bash
  cargo run --features dev-preview -- dev-preview --scenario pets --out target/glorp-preview-phase4
  ```
  Then open `target/glorp-preview-phase4/index.html` and confirm: (1) Glitch resting face is alive (no `x x`); (2) corruption glyphs are visibly a different color from the body, not melted in; (3) no eye-center is ever a corruption glyph; (4) non-Glitch species show no corruption.

- [ ] Acceptance checklist from the spec (Phase 4): calm/no-flash (gate at every 13 ticks, ≤3 cells — confirmed by `corruption_gate_is_closed_most_ticks` + `corruption_footprint_is_bounded`); corruption reads intentional (contrasting `Corruption` role + z-win — `glitch_corruption_emits_corruption_role_spans_on_active_tick`); living-face preserved (`glitch_corruption_never_recolors_the_eye_center`). Confirm each test name is present and passing in the run above.

- [ ] If on `main`, the WIP branch was created at the start; confirm the branch is ready and report the commit range. No merge/PR unless Drew asks.
  ```bash
  git log --oneline main..HEAD
  ```
