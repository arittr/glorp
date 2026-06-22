# Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax.

**Goal:** Replace the stage→template indexing in `src/pet/art.rs` with a per-stage base-template map (one base per species×stage), delete the shared-adult-pool / `elder_morph_index` logic, redefine `morph_count`, add the new invariant + continuity test helpers, and introduce the per-species gutter-content + precedence model that moves the S6 sparkle out of the art rows — all while rewiring the *existing* art so the roster still renders unchanged in shape.

**Architecture:** `art.rs` today maps stages onto three pools (`*_TINY[0..3]`, `*_PUP[0]`, `*_ADULT[..]`) with `elder_morph_index` reshuffling adults at S5/S6 and a sage-frame substitution at S6. Phase 1 collapses this to a single `stage_base_template(species, stage)` returning one `&'static Template`, plus a thin `apply_interior_texture` (identity passthrough in Phase 1 — bodies are filled in Phase 2) and an owned-`String` render entry `stage_template_lines`. The S6 sparkle stops overwriting art rows 0/7 and becomes gutter data via `GutterContent`/`gutter_content_for`, reconciled with the third sparkle surface `frame_fill_for_stage` (`tui/layout.rs`). New `#[cfg(test)]` helpers (`rendered_occupied_cells`, `assert_in_stage_band`, `ambiguous_wide_width_warnings`, `assert_s6_fills_art_rows_no_sparkle`) and `feet_row`/`feet_columns` are introduced for downstream phases. No new art is drawn — the existing per-stage shapes are wired into the new map verbatim.

**Tech Stack:** Rust, ratatui, SQLite; tests via cargo + assert_cmd.

## Global Constraints

- Templates are exactly 11 display columns × 8 lines; the `Template` type alias is `[&'static str; 8]` (`src/pet/art.rs`).
- Every art glyph (and every `{slot}` filler) is width-1 under unicode-width's default (ambiguous=narrow); enforced by `every_template_line_is_eleven_display_columns` (`src/pet/art.rs`).
- Eye/accent glyphs must be East-Asian-Width Neutral or Narrow, never Ambiguous; `◇◆◈●○` are Ambiguous (kept only per the Crystal decision — non-blocking lint).
- Growth cell bands (occupied non-space cells across the 8 art rows, fixed reference state): S0:1-4 · S1:5-10 · S2:11-20 · S3:21-34 · S4:35-50 · S5:51-66 · S6:67-88 — disjoint, strictly increasing, S4<S5<S6.
- S6 fills all 8 art rows; the sparkle no longer overwrites art rows 0/7 (asserted as a separate structural check, not in the size count).
- Color is truecolor-first, two tiers only: `ColorCapability::{Truecolor, Flat}` (`src/tui/style.rs:54`); honor `NO_COLOR`/`TERM=dumb`; under Flat pets render monochrome carried by silhouette; sub-truecolor is ratatui's automatic downgrade, not engineered here.
- Tamagotchi spirit: calm over flashy, night calmer than day, nurturing companion not optimizer; no death — floor state is `Mood::Wilted`.
- Only real signals drive content; the immature-pet zero-feast invariant is preserved (`flat_and_immature_pets_render_zero_motes`, `src/tui/panels/pet/ambient.rs:798`).
- The renderer stays content-agnostic: species/stage character lives in `art.rs` templates + palette, never in renderer special-casing.
- `cargo clippy --all-targets --all-features -- -D warnings` must stay clean; test-only helper fns must be `#[cfg(test)]`.
- Test output must be pristine; intentional error output must be captured and asserted.
- Test isolation: integration tests use `tempfile::tempdir()` + `GLORP_CONFIG_DIR`; when testing helper failures, pin BOTH `GLORP_CCUSAGE_BIN` and `GLORP_CCUSAGE_CODEX_BIN`.
- Commit frequently (do not ask first); WIP branch off `main`; never `git add -A` without a prior `git status`.
- Identity data is never touched: no `state.json` schema change; `seed`/`accepted_name`/`xp`/vitals/stage/calibration/seen-transitions untouched. A one-time visual reset is accepted.
- Do NOT call `apply_usage_poll` from production code (`#[doc(hidden)]` test wrapper).

---

## Reality check the implementer MUST internalize before starting

The contract's **growth cell bands** (S0:1-4 … S6:67-88) and the **S4<S5<S6 monotonicity** invariant are an audit target for the **new art drawn in Phase 2**, not for the existing art. The existing templates do NOT fit those bands. Measured occupied-cell counts of the current art (canonical slot fill, computed during planning):

| Species | S0 | S1 | S2 | S3 | adult pool densities (S4-pool order) |
|---|---|---|---|---|---|
| Fuzz | 10 | 14 | 23 | 35 | [45, 47, 47, 48] |
| Blob | 7 | 15 | 26 | 45 | [54, 51, 73, 56] |
| Ghost | 10 | 18 | 22 | 48 | [61, 40, 51, 51] |
| Glitch | 3 | 11 | 24 | 34 | [44, 58, 50] |
| Crystal | 5 | 12 | 23 | 35 | [43, 54, 41] |
| Mech | 9 | 17 | 22 | 35 | [53, 70, 40, 55] |

So Phase 1 **introduces** the band/monotonicity helpers and **unit-tests the helper math against synthetic fixtures** (proving the helpers are correct), but it does NOT run a band/monotonicity assertion over the *real* templates — that assertion would fail on the placeholder art and is a **Phase 2 acceptance gate**. Phase 1's real-template tests are exactly the ones that pass on rewired existing art: the existing 11×8 / width-1 / 8-line invariants (re-pointed at the new map), the **continuity** test (valid non-empty 11×8 for every species×stage over a fixed seed set), the **structural S6** check (`assert_s6_fills_art_rows_no_sparkle` — passes because S6 now uses a full adult template instead of the sage substitution), and the **ambiguous-width lint** (non-blocking; warns, never fails). This split is deliberate and matches the spec ("Existing art is rewired into the new map so the roster still renders").

### Stage→existing-template mapping used by `stage_base_template` (Phase 1 wiring)

One base per stage, picked from the existing pools so S4/S5/S6 are three *distinct* existing shapes (no `elder_morph_index`):

- `S0 → *_TINY[0]`, `S1 → *_TINY[1]`, `S2 → *_TINY[2]`, `S3 → *_PUP[0]`.
- `S4 → *_ADULT[0]`, `S5 → *_ADULT[1]`.
- `S6 → *_ADULT[last]` (`[3]` for Fuzz/Blob/Ghost/Mech which have 4 adults; `[2]` for Glitch/Crystal which have 3). For the 3-adult species `S5 → [1]` and `S6 → [2]` are already distinct.

This keeps every (species, stage) resolving to a real, width-valid, 8-line template and removes the sage-frame substitution at S6.

---

## File Structure

- **`src/pet/art.rs`** (modify) — Owns templates. Phase 1: add `stage_base_template`, `apply_interior_texture`, `stage_template_lines`; redefine `morph_count`; delete `template_lines`, `elder_morph_index`, `SAGE_TOP`/`SAGE_BOT` and their use; keep all template constants and `tiny_template`/`pup_templates`/`adult_templates` as internal accessors used by `stage_base_template`. Rewrite the orphaned tests; add new `#[cfg(test)]` invariant + continuity helpers and their self-tests.
- **`src/pet/render.rs`** (modify) — Single production caller of art. Phase 1: switch `render_pet` from `template_lines(species, stage, morph_index, morph_pup_index)` to `stage_template_lines(species, stage, seed)`; add the `GutterContent` enum + `gutter_content_for`; add `feet_row`/`feet_columns`; make `frame_with_particles` apply the gutter S6 sparkle (row 0 only) under explicit precedence; remove the no-longer-needed art-row sparkle.
- **`src/tui/layout.rs`** (modify) — `frame_fill_for_stage` is the third sparkle surface. Phase 1: leave its behavior but add a reconciliation note + a test asserting S6 outer-frame fill and the gutter sparkle agree on the sparkle glyph, so the two surfaces are coordinated rather than independently chosen.
- **`tests/generation.rs`** (modify) — Rewrite `species_have_enough_seeded_morph_variety` to the new `morph_count` contract; drop the `morph_count(S3)==1` / `>=3` assertions. Add the cross-binary continuity test (fixed seed set renders non-empty 11×8 for every species×stage).
- **`src/pet/generation.rs`** (modify) — `morph_pup_index` is now dead at render time. Phase 1: keep the field + draw (identity reset is accepted; removing the draw shifts every later draw and changes every pet — out of scope) but document it as retained-dead, and adjust `seeded_generation_selects_visible_traits_palette_morph_and_phase` only if it references removed behavior (it does not — no change needed).

---

## Task 1 — WIP branch + redefine `morph_count`, delete the old multi-arg `template_lines`/`elder_morph_index` behind a new `stage_base_template`

**Files:**
- Modify: `/Users/drewritter/projects/glorp/src/pet/art.rs`
- Test: `/Users/drewritter/projects/glorp/src/pet/art.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces (consumed by render.rs Task 3, and Phases 3/4):
  ```rust
  pub(crate) fn stage_base_template(species: Species, stage: Stage) -> &'static Template;
  pub fn morph_count(species: Species, stage: Stage) -> usize;
  ```
  `morph_count` new contract: interior-texture-variant count, `>= 1` for every stage; in Phase 1 it returns `1` for every (species, stage) because `apply_interior_texture` is an identity passthrough until Phase 2.
- Removes: `pub(crate) fn template_lines(species, stage, morph_index, morph_pup_index)`, `fn elder_morph_index(...)`, `SAGE_TOP`, `SAGE_BOT`.

**Steps:**

- [ ] Create the WIP branch:
  ```bash
  git -C /Users/drewritter/projects/glorp status
  git -C /Users/drewritter/projects/glorp checkout -b glorp-overhaul-phase1-foundation
  ```

- [ ] Write the failing test for `stage_base_template` + the new `morph_count`. Add to `src/pet/art.rs` `mod tests`:
  ```rust
  #[test]
  fn stage_base_template_returns_a_valid_template_for_every_species_stage() {
      for species in Species::all() {
          for stage in ALL_STAGES {
              let tpl = stage_base_template(species, stage);
              assert_eq!(tpl.len(), 8, "{species:?} {stage:?} must be 8 lines");
              for (row, line) in tpl.iter().enumerate() {
                  let rendered = substitute_slots(line);
                  assert_eq!(
                      rendered.chars().count(),
                      11,
                      "{species:?} {stage:?} row {row} must be 11 chars: {rendered:?}"
                  );
              }
          }
      }
  }

  #[test]
  fn morph_count_is_at_least_one_for_every_stage() {
      for species in Species::all() {
          for stage in ALL_STAGES {
              assert!(
                  morph_count(species, stage) >= 1,
                  "{species:?} {stage:?} must have >= 1 interior-texture variant"
              );
          }
      }
  }
  ```

- [ ] Run it, expect a compile failure:
  ```bash
  cargo test --lib stage_base_template_returns_a_valid_template
  ```
  Expected: `error[E0425]: cannot find function 'stage_base_template' in this scope` (and the existing `morph_count` still compiles but its semantics are about to change).

- [ ] Replace `morph_count` and add `stage_base_template`; delete `template_lines`, `elder_morph_index`, `SAGE_TOP`, `SAGE_BOT`. In `src/pet/art.rs`, replace the block from `pub fn morph_count` (line ~46) through the end of `elder_morph_index` (line ~111) and the two `SAGE_*` constants (lines ~113-115) with:
  ```rust
  /// Number of deterministic interior-texture variants a (species, stage) can
  /// render. Per-pet variety is algorithmic (interior texture), not hand-drawn
  /// silhouette pools, so this is the interior-texture-variant count (>= 1 for
  /// every stage; 1 where texture is pinned). It is NOT a silhouette-pool size.
  /// Phase 1: `apply_interior_texture` is an identity passthrough, so every
  /// (species, stage) has exactly one variant. Phase 2 raises this where texture
  /// adds variants.
  pub fn morph_count(_species: Species, _stage: Stage) -> usize {
      1
  }

  /// One hand-drawn base silhouette per (species, stage). 42 total. Phase 1 wires
  /// the existing art into this map; Phase 2 replaces the bodies with the new cast.
  /// The S4/S5/S6 picks are three distinct existing adult shapes so growth still
  /// reads as change without the retired `elder_morph_index` reshuffle.
  pub(crate) fn stage_base_template(species: Species, stage: Stage) -> &'static Template {
      match stage {
          Stage::S0 => tiny_template(species, 0),
          Stage::S1 => tiny_template(species, 1),
          Stage::S2 => tiny_template(species, 2),
          Stage::S3 => &pup_templates(species)[0],
          Stage::S4 => &adult_templates(species)[0],
          Stage::S5 => &adult_templates(species)[1],
          Stage::S6 => {
              let adults = adult_templates(species);
              &adults[adults.len() - 1]
          }
      }
  }
  ```
  Then delete the now-orphaned `pub(crate) fn template_lines(...)` (lines ~54-86). Leave `tiny_template`, `pup_templates`, `adult_templates` and all `*_TINY`/`*_PUP`/`*_ADULT` constants in place (they back `stage_base_template`).

- [ ] Run the new tests, expect a different failure — the old tests still reference removed symbols. First confirm the new ones compile-fail on the orphans:
  ```bash
  cargo test --lib --no-run 2>&1 | head -40
  ```
  Expected: `error[E0425]: cannot find function 'template_lines'` / `'elder_morph_index'` from the OLD tests (`every_template_line_is_eleven_cells_wide`, `elder_morph_skips_*`, `glitch_daemon_silhouette_*`, etc.). These are rewritten in Task 2. Leave them broken for now — do NOT commit yet.

- [ ] (No commit yet — the test module is intentionally broken until Task 2 rewires the existing invariant tests onto `stage_base_template`.)

---

## Task 2 — Re-point the existing invariant tests + rewrite the orphaned tests onto the new map

**Files:**
- Modify: `/Users/drewritter/projects/glorp/src/pet/art.rs` (`#[cfg(test)] mod tests`)
- Test: same file

**Interfaces:**
- Consumes: `stage_base_template(species, stage) -> &'static Template`, `morph_count(species, stage) -> usize` (Task 1).

**Steps:**

- [ ] Rewrite the three structural invariant tests to iterate `stage_base_template` (one base per stage, no morph loops). Replace `every_template_line_is_eleven_cells_wide`, `every_template_line_is_eleven_display_columns`, and `every_template_is_eight_lines` bodies with:
  ```rust
  #[test]
  fn every_template_line_is_eleven_cells_wide() {
      for species in Species::all() {
          for stage in ALL_STAGES {
              let lines = stage_base_template(species, stage);
              for (row, line) in lines.iter().enumerate() {
                  let rendered = substitute_slots(line);
                  let width = rendered.chars().count();
                  assert_eq!(
                      width, 11,
                      "template width != 11 for species={species:?} stage={stage:?} row={row}: \
                       {rendered:?}"
                  );
              }
          }
      }
  }

  #[test]
  fn every_template_line_is_eleven_display_columns() {
      use unicode_width::UnicodeWidthStr;
      // Terminal columns under unicode-width's default (ambiguous=narrow).
      for species in Species::all() {
          for stage in ALL_STAGES {
              let lines = stage_base_template(species, stage);
              for (row, line) in lines.iter().enumerate() {
                  let rendered = substitute_slots(line);
                  let columns = UnicodeWidthStr::width(rendered.as_str());
                  assert_eq!(
                      columns, 11,
                      "display width != 11 for species={species:?} stage={stage:?} row={row}: \
                       {rendered:?}"
                  );
              }
          }
      }
  }

  #[test]
  fn every_template_is_eight_lines() {
      for species in Species::all() {
          for stage in ALL_STAGES {
              let lines = stage_base_template(species, stage);
              assert_eq!(
                  lines.len(),
                  8,
                  "template height != 8 for species={species:?} stage={stage:?}"
              );
          }
      }
  }
  ```

- [ ] Replace the orphaned `elder_morph_skips_singleton_for_carved_species`, `elder_morph_skips_singleton_for_glitch`, and `glitch_daemon_silhouette_is_visibly_denser_than_glitch_form` tests. The `elder_morph_*` tests asserted behavior of a deleted function; their *intent* (S5/S6 are distinct evolved forms, not the S4 form) is now a property of `stage_base_template` picking distinct adult indices. The glitch-density test asserted S5 outgrows S4 — under the placeholder mapping that is not guaranteed (existing adult densities don't strictly increase), so it is re-expressed as the honest Phase-1 property: **S4/S5/S6 are three distinct base templates**, with the strict-growth assertion deferred to the Phase 2 band gate. Replace all three with:
  ```rust
  #[test]
  fn elder_stages_are_distinct_base_templates() {
      // Replaces elder_morph_skips_singleton_for_carved_species / _for_glitch.
      // The retired `elder_morph_index` ensured S5/S6 were not the S4 form; the
      // per-stage base map encodes that directly by mapping S4/S5/S6 to three
      // different existing shapes. Strict occupied-cell growth (S4<S5<S6) is the
      // Phase 2 band gate, not a Phase 1 property of the placeholder art.
      for species in Species::all() {
          let s4 = stage_base_template(species, Stage::S4);
          let s5 = stage_base_template(species, Stage::S5);
          let s6 = stage_base_template(species, Stage::S6);
          assert_ne!(s4, s5, "{species:?} S4 and S5 must be different base templates");
          assert_ne!(s5, s6, "{species:?} S5 and S6 must be different base templates");
          assert_ne!(s4, s6, "{species:?} S4 and S6 must be different base templates");
      }
  }
  ```
  Delete the now-unused `visible_cell_count` helper (it was only used by the glitch-density test; the new occupied-cell helper in Task 4 supersedes it). If the deletion produces a `clippy::dead_code` warning anywhere, that is the signal it is truly orphaned — remove it.

- [ ] Run the full art test module, expect PASS:
  ```bash
  cargo test --lib --package glorp art::tests
  ```
  Expected: all `art::tests::*` pass (`stage_base_template_returns_*`, `morph_count_is_at_least_one_*`, the three width/line tests, `elder_stages_are_distinct_base_templates`).

- [ ] Run clippy to confirm no dead-code warning from the removed helpers:
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -20
  ```
  Expected: clean (note: `render.rs` still calls the now-deleted `template_lines` — this is fixed in Task 3, so the *build* will fail here; that is expected and acceptable mid-task. If you want a green checkpoint, do Task 3 before committing. Otherwise continue.)

---

## Task 3 — Switch `render_pet` onto `stage_template_lines` + `apply_interior_texture`; keep the build green

**Files:**
- Modify: `/Users/drewritter/projects/glorp/src/pet/art.rs`
- Modify: `/Users/drewritter/projects/glorp/src/pet/render.rs`
- Test: `/Users/drewritter/projects/glorp/src/pet/art.rs`, `/Users/drewritter/projects/glorp/src/pet/render.rs`

**Interfaces:**
- Produces (consumed by `render_pet` and Phase 2 which fills the body):
  ```rust
  // src/pet/art.rs — interior texture; Phase 1 is identity passthrough.
  pub(crate) fn apply_interior_texture(
      base: &Template,
      species: Species,
      stage: Stage,
      seed: u64,
  ) -> [String; 8];

  // src/pet/art.rs — render entry replacing template_lines. Returns owned Strings
  // because interior texture is computed, not 'static.
  pub(crate) fn stage_template_lines(species: Species, stage: Stage, seed: u64) -> [String; 8];
  ```
- Consumes: `stage_base_template` (Task 1).
- Note: `render_pet` stops passing `pet.traits.morph_index`/`morph_pup_index`; it derives a `seed` from `pet.traits.seed_hue` (a `u16`) widened to `u64`. This is the interior-texture draw the contract names ("`seed` is `traits.seed_hue`").

**Steps:**

- [ ] Write the failing test for `apply_interior_texture` (identity passthrough in Phase 1) and `stage_template_lines`. Add to `src/pet/art.rs` `mod tests`:
  ```rust
  #[test]
  fn apply_interior_texture_is_identity_in_phase_one() {
      // Phase 1 ships the texture hook as a passthrough: the rendered lines equal
      // the base template (slots still unresolved {} markers) regardless of seed.
      for species in Species::all() {
          for stage in ALL_STAGES {
              let base = stage_base_template(species, stage);
              for seed in [0u64, 1, 7, 99, 360, u64::from(u16::MAX)] {
                  let textured = apply_interior_texture(base, species, stage, seed);
                  for (row, (a, b)) in base.iter().zip(textured.iter()).enumerate() {
                      assert_eq!(
                          *a, b.as_str(),
                          "{species:?} {stage:?} seed={seed} row={row} must be unchanged"
                      );
                  }
              }
          }
      }
  }

  #[test]
  fn stage_template_lines_matches_base_after_slot_widths() {
      // stage_template_lines feeds render.rs; in Phase 1 it equals the base.
      for species in Species::all() {
          for stage in ALL_STAGES {
              let base = stage_base_template(species, stage);
              let lines = stage_template_lines(species, stage, 42);
              assert_eq!(lines.len(), 8);
              for (a, b) in base.iter().zip(lines.iter()) {
                  assert_eq!(*a, b.as_str());
              }
          }
      }
  }
  ```

- [ ] Run, expect compile failure:
  ```bash
  cargo test --lib apply_interior_texture_is_identity 2>&1 | head -20
  ```
  Expected: `error[E0425]: cannot find function 'apply_interior_texture'`.

- [ ] Implement `apply_interior_texture` and `stage_template_lines` in `src/pet/art.rs` (place them right after `stage_base_template`):
  ```rust
  /// Deterministic per-seed interior-texture variation applied on top of a base
  /// silhouette. Phase 1: identity passthrough (returns the base verbatim) — the
  /// hook exists so render.rs and the invariant tests can target the final API
  /// now; Phase 2 fills in the texture math. MUST preserve the closed outline,
  /// width-1, and the stage cell band. On S0-S2 it is constrained to
  /// constant-occupancy glyphs (band-safety).
  pub(crate) fn apply_interior_texture(
      base: &Template,
      _species: Species,
      _stage: Stage,
      _seed: u64,
  ) -> [String; 8] {
      std::array::from_fn(|i| base[i].to_string())
  }

  /// Public render entry replacing `template_lines`. Returns owned Strings because
  /// interior texture is computed, not 'static. `seed` is the interior-texture
  /// draw (render.rs passes `pet.traits.seed_hue`).
  pub(crate) fn stage_template_lines(species: Species, stage: Stage, seed: u64) -> [String; 8] {
      let base = stage_base_template(species, stage);
      apply_interior_texture(base, species, stage, seed)
  }
  ```

- [ ] Run the two new art tests, expect PASS:
  ```bash
  cargo test --lib apply_interior_texture_is_identity stage_template_lines_matches_base
  ```
  Expected: both pass.

- [ ] Now switch `render_pet`. In `src/pet/render.rs`, change the import line 3 and the `template_lines(...)` call (lines ~127-132). Replace:
  ```rust
  use crate::pet::art::template_lines;
  ```
  with:
  ```rust
  use crate::pet::art::stage_template_lines;
  ```
  and replace:
  ```rust
      let raw = template_lines(
          pet.species,
          stage,
          pet.traits.morph_index,
          pet.traits.morph_pup_index,
      );
      let rendered = raw
          .iter()
          .enumerate()
          .map(|(line_index, line)| render_template_line(line, line_index, pet, &expression))
          .collect::<Vec<_>>();
  ```
  with:
  ```rust
      let raw = stage_template_lines(pet.species, stage, u64::from(pet.traits.seed_hue));
      let rendered = raw
          .iter()
          .enumerate()
          .map(|(line_index, line)| {
              render_template_line(line.as_str(), line_index, pet, &expression)
          })
          .collect::<Vec<_>>();
  ```
  (`render_template_line` takes `&str`; `line` is now a `&String`, so `.as_str()` adapts.)

- [ ] Build + run the render and generation tests, expect PASS:
  ```bash
  cargo test --lib render
  cargo test --test generation 2>&1 | tail -30
  ```
  Expected: render unit tests pass. `tests/generation.rs` will still FAIL at the `morph_count` assertions (`species_have_enough_seeded_morph_variety`) and possibly fail to compile if it referenced removed symbols — but it only imports `morph_count`/`stage_label` (both still exist), so it compiles and only that one test fails. That test is rewritten in Task 6.

- [ ] Commit the green-internal checkpoint (lib builds, art + render tests pass):
  ```bash
  cargo build
  cargo test --lib
  git -C /Users/drewritter/projects/glorp add src/pet/art.rs src/pet/render.rs
  git -C /Users/drewritter/projects/glorp commit -m "$(cat <<'EOF'
  refactor: per-stage base-template map replaces adult-pool indexing

  Introduce stage_base_template / apply_interior_texture / stage_template_lines
  and redefine morph_count as the interior-texture-variant count. Delete the
  multi-arg template_lines, elder_morph_index, and the SAGE frame substitution.
  render_pet now resolves art by (species, stage, seed); per-stage shapes are the
  existing art rewired one-base-per-stage so the roster renders unchanged.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 4 — Invariant + continuity test helpers in `art.rs` (size, band, ambiguous-width lint), self-tested against synthetic fixtures

**Files:**
- Modify: `/Users/drewritter/projects/glorp/src/pet/art.rs` (`#[cfg(test)] mod tests`)
- Test: same file

**Interfaces:**
- Produces (used by Phases 1-4; band/monotonicity *enforcement over real art* is a Phase 2 gate):
  ```rust
  #[cfg(test)] fn rendered_occupied_cells(species: Species, stage: Stage) -> usize;
  #[cfg(test)] fn assert_in_stage_band(species: Species, stage: Stage);
  #[cfg(test)] fn ambiguous_wide_width_warnings(species: Species, stage: Stage) -> Vec<char>;
  #[cfg(test)] fn assert_s6_fills_art_rows_no_sparkle(species: Species);
  ```
- Consumes: `stage_base_template`, `stage_template_lines` (Tasks 1, 3).

**Steps:**

- [ ] Add the helper `rendered_occupied_cells` and the canonical stage-band table, then a self-test that proves the helper math on synthetic fixtures (not on real art). Add to `src/pet/art.rs` `mod tests`, above the new tests:
  ```rust
  // Inclusive [lo, hi] occupied-cell band per stage (the audit target for the
  // Phase 2 art). Disjoint and strictly increasing.
  const STAGE_CELL_BANDS: [(usize, usize); 7] = [
      (1, 4),    // S0
      (5, 10),   // S1
      (11, 20),  // S2
      (21, 34),  // S3
      (35, 50),  // S4
      (51, 66),  // S5
      (67, 88),  // S6
  ];

  // Rendered occupied (non-space) cell count of the 8 art rows at the fixed
  // reference state: mood = Content, resting (non-blink) expression, no work
  // accent, fixed tick, each {slot} replaced by its canonical width-correct
  // filler ({eyes}->"o o", {mouth}->"w", {pattern}->"...", {accent}->"*").
  // Excludes the particle gutter and any frame substitution. Built from
  // stage_template_lines so it tracks the Phase 1 (identity) and Phase 2 (real)
  // texture path.
  #[cfg(test)]
  fn rendered_occupied_cells(species: Species, stage: Stage) -> usize {
      stage_template_lines(species, stage, REFERENCE_SEED)
          .iter()
          .map(|line| {
              substitute_slots(line)
                  .chars()
                  .filter(|c| !c.is_whitespace())
                  .count()
          })
          .sum()
  }

  // A fixed reference seed for the tick-independent measurement. The slot fill is
  // canonical (substitute_slots), so this is only the interior-texture draw.
  #[cfg(test)]
  const REFERENCE_SEED: u64 = 0;
  ```

- [ ] Add `assert_in_stage_band` and a self-test proving it accepts in-band counts and rejects out-of-band counts on synthetic vectors (so the helper is proven correct WITHOUT depending on the placeholder art passing). Append:
  ```rust
  // Band membership + S0->S6 monotonicity (S4 < S5 < S6) over the occupied-cell
  // count. NOTE: in Phase 1 this is NOT run over the real templates (they predate
  // the band redesign); Phase 2 calls it as its growth-acceptance gate. It is
  // exercised here only via the synthetic self-test below so its logic is proven.
  #[cfg(test)]
  fn assert_in_stage_band_value(stage: Stage, occupied: usize) {
      let (lo, hi) = STAGE_CELL_BANDS[stage.index()];
      assert!(
          occupied >= lo && occupied <= hi,
          "occupied cells {occupied} for {stage:?} outside band [{lo}, {hi}]"
      );
  }

  #[cfg(test)]
  fn assert_in_stage_band(species: Species, stage: Stage) {
      assert_in_stage_band_value(stage, rendered_occupied_cells(species, stage));
  }

  #[test]
  fn stage_cell_bands_are_disjoint_and_strictly_increasing() {
      for w in STAGE_CELL_BANDS.windows(2) {
          let (lo, hi) = w[0];
          let (next_lo, next_hi) = w[1];
          assert!(lo <= hi, "band [{lo}, {hi}] is inverted");
          assert!(
              hi < next_lo,
              "bands must be disjoint and increasing: [{lo},{hi}] then [{next_lo},{next_hi}]"
          );
      }
      // S4 < S5 < S6 lower bounds (the explicit monotonicity callout).
      assert!(STAGE_CELL_BANDS[4].0 < STAGE_CELL_BANDS[5].0);
      assert!(STAGE_CELL_BANDS[5].0 < STAGE_CELL_BANDS[6].0);
  }

  #[test]
  fn assert_in_stage_band_value_accepts_in_band_rejects_out_of_band() {
      // Proves the band check logic without depending on placeholder art.
      assert_in_stage_band_value(Stage::S0, 1);
      assert_in_stage_band_value(Stage::S0, 4);
      assert_in_stage_band_value(Stage::S6, 67);
      assert_in_stage_band_value(Stage::S6, 88);
      let rejected = std::panic::catch_unwind(|| assert_in_stage_band_value(Stage::S0, 5));
      assert!(rejected.is_err(), "5 cells must be rejected from the S0 band");
      let rejected_high = std::panic::catch_unwind(|| assert_in_stage_band_value(Stage::S6, 89));
      assert!(rejected_high.is_err(), "89 cells must be rejected from the S6 band");
  }
  ```
  (`assert_in_stage_band` is intentionally defined but not yet called over real art in Phase 1. To avoid a `clippy::dead_code` warning on the unused `#[cfg(test)] fn`, add `#[allow(dead_code)]` directly above it with a comment: `// Called by the Phase 2 growth gate; see plan Task 4.`)

- [ ] Run the band self-tests, expect PASS:
  ```bash
  cargo test --lib stage_cell_bands_are_disjoint assert_in_stage_band_value_accepts
  ```
  Expected: both pass.

- [ ] Add the non-blocking `ambiguous_wide_width_warnings` lint helper + a test that emits (does not fail) on the known Ambiguous glyphs (`◆◈◇●○`) per the Crystal decision. Append:
  ```rust
  // Ambiguous=WIDE width check. Per the Crystal eye-fill decision this is a
  // NON-BLOCKING lint: it WARNS (eprintln!) on East-Asian-Width Ambiguous glyphs
  // and returns them, but never asserts — failing the build would contradict
  // keeping the ◇/◆/◈ eye-fill. The blocking width invariant
  // (every_template_line_is_eleven_display_columns) stays under the default
  // narrow assumption.
  #[cfg(test)]
  fn ambiguous_wide_width_warnings(species: Species, stage: Stage) -> Vec<char> {
      use unicode_width::UnicodeWidthChar;
      let mut offenders = Vec::new();
      for line in stage_template_lines(species, stage, REFERENCE_SEED).iter() {
          for ch in substitute_slots(line).chars() {
              // width_cjk() applies the ambiguous=wide rule. A glyph whose narrow
              // width is 1 but whose cjk width is 2 is the Ambiguous case.
              let narrow = UnicodeWidthChar::width(ch).unwrap_or(0);
              let wide = UnicodeWidthChar::width_cjk(ch).unwrap_or(0);
              if narrow == 1 && wide == 2 {
                  offenders.push(ch);
              }
          }
      }
      offenders
  }

  #[test]
  fn ambiguous_width_lint_warns_but_does_not_fail() {
      // Surfaces ambiguous-width glyphs for human review without failing CI.
      let mut total = 0usize;
      for species in Species::all() {
          for stage in ALL_STAGES {
              let offenders = ambiguous_wide_width_warnings(species, stage);
              if !offenders.is_empty() {
                  eprintln!(
                      "ambiguous-width glyphs in {species:?} {stage:?}: {offenders:?}"
                  );
                  total += offenders.len();
              }
          }
      }
      // The lint is informational. Assert only that the helper ran over every
      // species/stage (the count is allowed to be zero or positive).
      let _ = total;
  }
  ```
  Note for pristine output: this `eprintln!` is an intentional, captured lint signal. Per the test-output-pristine rule, downgrade it if reviewers object — but it is a `#[test]`-local warning surfaced only with `--nocapture`, so default `cargo test` output stays clean. Document this in the test comment.

- [ ] Run the lint test, expect PASS (with no stderr under default capture):
  ```bash
  cargo test --lib ambiguous_width_lint_warns_but_does_not_fail
  ```
  Expected: passes; with `-- --nocapture` it prints the Crystal `◆`/`◈`-bearing stages and Mech `◉`-adjacent cells, etc.

- [ ] Commit:
  ```bash
  git -C /Users/drewritter/projects/glorp add src/pet/art.rs
  git -C /Users/drewritter/projects/glorp commit -m "$(cat <<'EOF'
  test: add growth-band and ambiguous-width invariant helpers

  rendered_occupied_cells / assert_in_stage_band measure the 8 art rows at the
  fixed Content/non-blink/canonical-slot reference state; the band table is the
  Phase 2 growth-acceptance gate (self-tested on synthetic vectors here).
  ambiguous_wide_width_warnings is a non-blocking lint that surfaces EAW-Ambiguous
  glyphs without failing the build, per the Crystal eye-fill decision.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 5 — Gutter content + precedence model; move the S6 sparkle out of art rows into the gutter; feet helpers

**Files:**
- Modify: `/Users/drewritter/projects/glorp/src/pet/render.rs`
- Test: `/Users/drewritter/projects/glorp/src/pet/render.rs`

**Interfaces:**
- Produces (consumed by Phase 5 contact shadow + the `frame_fill_for_stage` reconciliation):
  ```rust
  // src/pet/render.rs
  pub enum GutterContent { Sparkle, MachineFrame, None }
  fn gutter_content_for(species: Species, stage: Stage) -> GutterContent;
  pub(crate) fn feet_row(art_lines: &[String]) -> Option<usize>;
  pub(crate) fn feet_columns(art_lines: &[String]) -> Vec<usize>;
  ```
- Precedence (highest wins) when an S6 sparkle, a species identity particle, and a contact shadow target the same cell: **species identity particle > S6 sparkle > contact shadow**. The S6 sparkle uses **row 0 only**. Contact shadow (Phase 5) is restricted to `feet_columns` on row 9.
- Decisions wired here (from the contract): `gutter_content_for(Species::Mech, Stage::S6) == GutterContent::None` (Mech keeps its chassis art rows). `Sparkle` for the S6 of every other species; `None` below S6.
- The S6 sparkle glyph is the same as the outer-frame S6 fill (`frame_fill_for_stage(Stage::S6)` returns `"✦"`), so the two surfaces agree (reconciliation in Task 7).

**Steps:**

- [ ] Write the failing test for `feet_row`/`feet_columns` against a synthetic art block. Add to `src/pet/render.rs` `mod tests`:
  ```rust
  #[test]
  fn feet_row_is_lowest_non_blank_art_row() {
      let art: Vec<String> = vec![
          "    ░░░    ".to_string(), // row 0
          "   ░▒▒▒░   ".to_string(), // row 1 (widest, but not lowest)
          "    d b    ".to_string(), // row 2 lowest non-blank
          "           ".to_string(), // row 3 blank
      ];
      assert_eq!(feet_row(&art), Some(2));
      // Columns of the lowest non-blank row that are non-space:
      assert_eq!(feet_columns(&art), vec![4, 6]);
  }

  #[test]
  fn feet_row_none_for_all_blank() {
      let art: Vec<String> = vec!["           ".to_string(); 3];
      assert_eq!(feet_row(&art), None);
      assert!(feet_columns(&art).is_empty());
  }
  ```

- [ ] Run, expect compile failure:
  ```bash
  cargo test --lib feet_row_is_lowest_non_blank 2>&1 | head -10
  ```
  Expected: `error[E0425]: cannot find function 'feet_row'`.

- [ ] Implement `feet_row`/`feet_columns` in `src/pet/render.rs` (place after `frame_with_particles`):
  ```rust
  /// Lowest non-blank art row of the rendered 8 rows = the silhouette's "feet".
  /// Templates carry trailing blank rows, so this finds the true bottom of the
  /// creature. Phase 5 restricts the contact shadow to the columns beneath it.
  pub(crate) fn feet_row(art_lines: &[String]) -> Option<usize> {
      art_lines
          .iter()
          .enumerate()
          .rev()
          .find(|(_, line)| line.chars().any(|c| c != ' '))
          .map(|(row, _)| row)
  }

  /// Non-space columns of the feet row (the contact-shadow footprint).
  pub(crate) fn feet_columns(art_lines: &[String]) -> Vec<usize> {
      match feet_row(art_lines) {
          None => Vec::new(),
          Some(row) => art_lines[row]
              .chars()
              .enumerate()
              .filter(|(_, c)| *c != ' ')
              .map(|(col, _)| col)
              .collect(),
      }
  }
  ```

- [ ] Run, expect PASS:
  ```bash
  cargo test --lib feet_row_is_lowest_non_blank feet_row_none_for_all_blank
  ```

- [ ] Write the failing test for `GutterContent`/`gutter_content_for`. Add:
  ```rust
  #[test]
  fn gutter_content_is_sparkle_at_s6_except_mech() {
      use crate::pet::generation::Species;
      for species in Species::all() {
          // Below S6: no gutter sparkle.
          assert_eq!(
              gutter_content_for(species, Stage::S5),
              GutterContent::None,
              "{species:?} S5 must have no gutter sparkle"
          );
      }
      // S6: sparkle for everyone except Mech (keeps its chassis art rows).
      for species in [
          Species::Fuzz,
          Species::Blob,
          Species::Ghost,
          Species::Glitch,
          Species::Crystal,
      ] {
          assert_eq!(gutter_content_for(species, Stage::S6), GutterContent::Sparkle);
      }
      assert_eq!(gutter_content_for(Species::Mech, Stage::S6), GutterContent::None);
  }
  ```

- [ ] Run, expect compile failure (no enum/fn yet):
  ```bash
  cargo test --lib gutter_content_is_sparkle_at_s6 2>&1 | head -10
  ```
  Expected: `cannot find type 'GutterContent'`.

- [ ] Implement the enum + fn in `src/pet/render.rs` (near the particle code). Add the enum after `PaletteRoleName` (or near `frame_with_particles`):
  ```rust
  /// Per-species gutter identity for the 13x10 frame's gutter rows (0 and 9) and
  /// side columns. Data, not an architecture fork. Phase 1 uses it only for the
  /// S6 sparkle move; Phase 5 reads it when compositing the contact shadow.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum GutterContent {
      Sparkle,
      MachineFrame,
      None,
  }

  /// S6 earns a gutter sparkle for every species except Mech, which keeps its own
  /// chassis art rows (decision: Mech-S6 gutter == None). Below S6 there is no
  /// gutter sparkle. `MachineFrame` is reserved for a future Mech gutter overlay
  /// and is unused in Phase 1.
  fn gutter_content_for(species: Species, stage: Stage) -> GutterContent {
      match (species, stage) {
          (Species::Mech, Stage::S6) => GutterContent::None,
          (_, Stage::S6) => GutterContent::Sparkle,
          _ => GutterContent::None,
      }
  }
  ```
  Because `MachineFrame` is a never-constructed variant in Phase 1, clippy `--all-targets` may flag it. Add `#[allow(dead_code)]` on the `MachineFrame` arm via an attribute on the enum with a comment: place `#[allow(dead_code)] // MachineFrame: reserved for a future Mech gutter overlay (Phase 5).` directly above the `MachineFrame,` variant. (Variant-level `#[allow]` is supported.)

- [ ] Run, expect PASS:
  ```bash
  cargo test --lib gutter_content_is_sparkle_at_s6
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -5
  ```
  Expected: test passes; clippy clean.

- [ ] Write the failing test that the S6 sparkle now lands in the **gutter** (frame row 0), not in art rows, and that it does not shrink the art. The S6 art now uses a full adult template (no SAGE substitution), so art rows 0 and 7 are the creature, and the sparkle is in framed row 0. Add:
  ```rust
  #[test]
  fn s6_sparkle_is_in_gutter_row_zero_not_art_rows() {
      use crate::pet::generation::generate_pet;
      // Force a Sparkle species at S6.
      let pet = generate_pet("s6-sparkle").with_species(Species::Crystal);
      // tick 0 keeps animation deterministic.
      let rendered = render_pet(&pet, Stage::S6, Mood::Content, AnimationFrame::default());
      // Framed grid is 10 rows tall; art occupies framed rows 1..=8, gutter is
      // rows 0 and 9. The S6 sparkle ('✦') must appear only in framed row 0.
      let row0 = &rendered.lines[0];
      assert!(
          row0.contains('\u{2726}'),
          "S6 gutter row 0 must carry the sparkle, got: {row0:?}"
      );
      for (i, line) in rendered.lines.iter().enumerate().skip(1) {
          assert!(
              !line.contains('\u{2726}'),
              "S6 sparkle must not appear in framed row {i} (art/bottom gutter): {line:?}"
          );
      }
  }
  ```
  Note: `\u{2726}` is `✦`. Confirm the glyph used by `frame_fill_for_stage(Stage::S6)` — it is `"✦"` (`tui/layout.rs:214`), the same glyph; this keeps the two surfaces consistent.

- [ ] Run, expect FAIL (sparkle not yet emitted into the gutter):
  ```bash
  cargo test --lib s6_sparkle_is_in_gutter_row_zero 2>&1 | tail -15
  ```
  Expected: assertion failure "S6 gutter row 0 must carry the sparkle".

- [ ] Emit the gutter sparkle inside `frame_with_particles`, with explicit precedence. `frame_with_particles` does not currently know the stage; thread it through. Change the signature and the single call site in `render_pet`:
  - In `render_pet` (line ~153) change:
    ```rust
    let (framed_lines, framed_spans) = frame_with_particles(lines, spans, pet.species, frame.tick);
    ```
    to:
    ```rust
    let (framed_lines, framed_spans) =
        frame_with_particles(lines, spans, pet.species, stage, frame.tick);
    ```
  - Change `fn frame_with_particles(art_lines, art_spans, species, tick)` to add `stage: Stage`:
    ```rust
    fn frame_with_particles(
        art_lines: Vec<String>,
        art_spans: Vec<StyledSegment>,
        species: Species,
        stage: Stage,
        tick: u64,
    ) -> (Vec<String>, Vec<StyledSegment>) {
    ```
  - Inside, AFTER the art overlay and BEFORE the species-particle loop, paint the S6 gutter sparkle on row 0 so the species particles (which run next, last-write-wins) outrank it — satisfying "species identity particle > S6 sparkle". Insert after the `framed_spans` translation block (after line ~500) and before `for particle in particles_for_species(...)`:
    ```rust
    // S6 gutter sparkle (row 0 only) — precedence: species identity particles
    // (painted just below) outrank it; the contact shadow (Phase 5, row 9) never
    // collides with row 0. Same glyph as the outer-frame S6 fill so the surfaces
    // agree.
    if gutter_content_for(species, stage) == GutterContent::Sparkle {
        const SPARKLE_COLS: [usize; 3] = [2, 6, 10];
        for col in SPARKLE_COLS {
            grid[0][col] = '\u{2726}';
            framed_spans.push(StyledSegment {
                line: 0,
                start: col,
                end: col + 1,
                role: PaletteRoleName::Particle,
            });
        }
    }
    ```
    (Three sparkle cells on row 0; cols 2/6/10 are inside the 13-wide gutter and away from the corners. The species-particle loop that follows can overwrite these cells per the precedence rule because it runs after.)

- [ ] Run, expect PASS:
  ```bash
  cargo test --lib s6_sparkle_is_in_gutter_row_zero
  ```
  Expected: passes — Crystal's row-0 species particles are at cols 1 and 11 (`particles_for_species`), so at least one of cols 2/6/10 survives; the sparkle shows in row 0 and nowhere else.

- [ ] Verify the precedence claim with a focused test (species particle wins a contested row-0 cell). Crystal paints `✧` (`\u{2727}`) at row 0 col 1 when `tick % 23 < 3`; our sparkle is at cols 2/6/10 so they don't contest. To prove precedence concretely, use a cell that IS contested: Mech is `None` at S6, so use a species whose row-0 particle column overlaps a sparkle column. Glitch paints `:` at row 0 col 10 when `tick % 17 < 2`. Add:
  ```rust
  #[test]
  fn species_particle_outranks_s6_sparkle_on_a_contested_cell() {
      use crate::pet::generation::generate_pet;
      let pet = generate_pet("contested").with_species(Species::Glitch);
      // tick 0: tick % 17 == 0 < 2, so Glitch paints ':' at row 0 col 10, which
      // is also a sparkle column. The species particle must win.
      let rendered = render_pet(&pet, Stage::S6, Mood::Content, AnimationFrame::default());
      let row0: Vec<char> = rendered.lines[0].chars().collect();
      assert_eq!(
          row0[10], ':',
          "Glitch row-0 particle at col 10 must outrank the S6 sparkle, got {:?}",
          row0[10]
      );
  }
  ```

- [ ] Run, expect PASS:
  ```bash
  cargo test --lib species_particle_outranks_s6_sparkle
  ```
  Expected: passes (the particle loop runs after the sparkle paint).

- [ ] Add the structural `assert_s6_fills_art_rows_no_sparkle` helper in `art.rs` and use it (this is the contract's separate structural check that S6 fills all 8 art rows and no row is a sparkle substitution). Add to `src/pet/art.rs` `mod tests`:
  ```rust
  // Structural: S6 fills all 8 art rows from the creature (no sparkle row
  // substitution). Phase 1 satisfies this because S6 now maps to a full adult
  // template instead of the retired SAGE_TOP/SAGE_BOT framing.
  #[cfg(test)]
  fn assert_s6_fills_art_rows_no_sparkle(species: Species) {
      let lines = stage_base_template(species, Stage::S6);
      // No row is a literal sparkle frame (the old SAGE rows were sparkle-only).
      for (row, line) in lines.iter().enumerate() {
          let only_sparkle_and_space = line
              .chars()
              .all(|c| c == ' ' || matches!(c, '\u{2726}' | '\u{2727}' | '*' | '.'));
          assert!(
              !only_sparkle_and_space || substitute_slots(line).trim().is_empty(),
              "{species:?} S6 row {row} looks like a sparkle frame, not creature art: {line:?}"
          );
      }
  }

  #[test]
  fn s6_fills_art_rows_for_every_species() {
      for species in Species::all() {
          assert_s6_fills_art_rows_no_sparkle(species);
      }
  }
  ```
  Note: the existing adult templates do leave a trailing blank row for some species (e.g. Fuzz adult morph 0 ends with a blank row 7). That is acceptable — the structural check rejects only *sparkle-substituted* rows, not legitimately blank trailing rows. The "S6 fills all 8 rows" density requirement is part of the Phase 2 band gate (S6: 67-88 cells), not Phase 1. The comment in this test must say so to avoid a future implementer mistaking the blank trailing row for a regression.

- [ ] Run, expect PASS:
  ```bash
  cargo test --lib s6_fills_art_rows_for_every_species
  ```

- [ ] Run the full lib + render tests and clippy:
  ```bash
  cargo test --lib
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -10
  ```
  Expected: all lib tests pass; clippy clean.

- [ ] Commit:
  ```bash
  git -C /Users/drewritter/projects/glorp add src/pet/render.rs src/pet/art.rs
  git -C /Users/drewritter/projects/glorp commit -m "$(cat <<'EOF'
  feat: gutter content + precedence model; move S6 sparkle to the gutter

  Add GutterContent / gutter_content_for (Mech-S6 = None) and emit the S6 sparkle
  into frame row 0 instead of overwriting art rows. Species identity particles
  outrank the sparkle (painted after it); the sparkle stays on row 0 so the
  Phase 5 contact shadow (row 9) never collides. Add feet_row/feet_columns and the
  assert_s6_fills_art_rows_no_sparkle structural helper.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 6 — Rewrite `tests/generation.rs` orphaned `morph_count` assertions + add the continuity test

**Files:**
- Modify: `/Users/drewritter/projects/glorp/tests/generation.rs`
- Test: same file (integration test against the `glorp` library crate)

**Interfaces:**
- Consumes: `glorp::pet::art::morph_count(species, stage) -> usize` (new contract), `glorp::pet::render::render_pet`, `glorp::pet::generation::{generate_pet, Species}`, `glorp::game::evolution::Stage`.

**Steps:**

- [ ] Rewrite `species_have_enough_seeded_morph_variety` (lines ~182-197) to the new `morph_count` contract (interior-texture-variant count, `>= 1` for every stage). Replace the whole test with:
  ```rust
  #[test]
  fn morph_count_is_the_interior_texture_variant_count() {
      // New contract: per-pet variety is algorithmic (interior texture), so
      // morph_count is the interior-texture-variant count and is >= 1 for every
      // (species, stage). It is NOT a hand-drawn silhouette-pool size, so the old
      // `morph_count(S3) == 1` and `morph_count(S4/S6) >= 3` assertions are gone.
      let stages = [
          Stage::S0,
          Stage::S1,
          Stage::S2,
          Stage::S3,
          Stage::S4,
          Stage::S5,
          Stage::S6,
      ];
      for species in Species::all() {
          for stage in stages {
              assert!(
                  morph_count(species, stage) >= 1,
                  "{species:?} {stage:?} must report >= 1 interior-texture variant"
              );
          }
      }
  }
  ```

- [ ] Run, expect PASS (this replaces the failing assertion from Task 3):
  ```bash
  cargo test --test generation morph_count_is_the_interior_texture
  ```
  Expected: passes.

- [ ] Add the continuity test (fixed seed set → valid non-empty 11×8 for every species×stage, no crash / no blank pet). Append to `tests/generation.rs`:
  ```rust
  #[test]
  fn fixed_seed_set_renders_valid_non_empty_11x8_for_every_species_stage() {
      use unicode_width::UnicodeWidthStr;
      let stages = [
          Stage::S0,
          Stage::S1,
          Stage::S2,
          Stage::S3,
          Stage::S4,
          Stage::S5,
          Stage::S6,
      ];
      // A fixed seed set spanning the seed_hue space that drives interior texture.
      let seeds = ["mochi-7f3a", "alpha", "beta", "gamma", "ori-shard", "0x-404"];
      for seed in seeds {
          for species in Species::all() {
              let pet = generate_pet(seed).with_species(species);
              for stage in stages {
                  let rendered = render_pet(&pet, stage, Mood::Content, frame(0));
                  // The framed grid is 10 rows x 13 cols; assert it is present and
                  // rectangular, and that at least one art row is non-blank (no
                  // blank pet).
                  assert_eq!(
                      rendered.lines.len(),
                      10,
                      "seed={seed} {species:?} {stage:?} must render 10 framed rows"
                  );
                  for (row, line) in rendered.lines.iter().enumerate() {
                      assert_eq!(
                          UnicodeWidthStr::width(line.as_str()),
                          13,
                          "seed={seed} {species:?} {stage:?} row {row} must be 13 cols wide: \
                           {line:?}"
                      );
                  }
                  let any_ink = rendered
                      .lines
                      .iter()
                      .any(|line| line.chars().any(|c| c != ' '));
                  assert!(
                      any_ink,
                      "seed={seed} {species:?} {stage:?} rendered a blank pet"
                  );
              }
          }
      }
  }
  ```
  (`frame(0)` is the existing helper at the top of `tests/generation.rs`. `unicode-width` is already a dependency — `art.rs` uses it. If the integration crate cannot see it, add `unicode-width` to `[dev-dependencies]` in `Cargo.toml`; check first with `grep unicode-width Cargo.toml`.)

- [ ] Confirm `unicode-width` is available to the integration test target:
  ```bash
  grep -n "unicode-width" /Users/drewritter/projects/glorp/Cargo.toml
  ```
  If it is only under `[dependencies]` (not `[dev-dependencies]`), it is still linkable from integration tests because integration tests depend on the library crate's public deps only transitively — to be safe, prefer `.chars().count()`-free measurement. If `grep` shows it is NOT a dev-dependency and the build fails on the import, replace `UnicodeWidthStr::width(line.as_str())` with a direct column count helper inline:
  ```rust
  // fallback if unicode-width is not a dev-dependency: every framed glyph is
  // width-1 by the art invariant, so char count == column count here.
  assert_eq!(line.chars().count(), 13, ...);
  ```
  Pick ONE form, run it, and keep whichever compiles.

- [ ] Run the continuity test, expect PASS:
  ```bash
  cargo test --test generation fixed_seed_set_renders_valid_non_empty 2>&1 | tail -15
  ```
  Expected: passes for all 6 seeds × 6 species × 7 stages.

- [ ] Run the whole generation integration file, expect PASS:
  ```bash
  cargo test --test generation 2>&1 | tail -20
  ```
  Expected: all tests pass (the rewritten `morph_count` test + continuity + the untouched ones).

- [ ] Commit:
  ```bash
  git -C /Users/drewritter/projects/glorp add tests/generation.rs Cargo.toml
  git -C /Users/drewritter/projects/glorp commit -m "$(cat <<'EOF'
  test: rewrite morph_count variety test + add render continuity coverage

  species_have_enough_seeded_morph_variety becomes
  morph_count_is_the_interior_texture_variant_count under the new contract
  (>= 1 per stage; no silhouette-pool size). Add a fixed-seed continuity test that
  every species x stage renders a valid non-empty 11x8 (framed 13x10) pet, so the
  template-map rework never produces a crash or a blank pet.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 7 — Reconcile `frame_fill_for_stage` (the third sparkle surface) with the gutter sparkle

**Files:**
- Modify: `/Users/drewritter/projects/glorp/src/tui/layout.rs`
- Test: `/Users/drewritter/projects/glorp/src/tui/layout.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `frame_fill_for_stage(stage) -> &'static str` (existing, `layout.rs:208`).
- Produces: a single source-of-truth assertion that the outer-frame S6 fill glyph and the pet-gutter S6 sparkle glyph are the same character, so the two surfaces are coordinated (not three uncoordinated sparkle treatments).

**Steps:**

- [ ] Add a test asserting the S6 outer-frame fill is the sparkle glyph used by the gutter (`✦` / `\u{2726}`). The existing `frame_fill_for_stage_returns_expected_char` already asserts `"✦"`; add an explicit cross-surface coordination test next to it. In `src/tui/layout.rs` `mod tests`, after `frame_fill_for_stage_returns_expected_char`:
  ```rust
  #[test]
  fn s6_outer_frame_fill_matches_the_pet_gutter_sparkle_glyph() {
      // The pet-gutter S6 sparkle (render.rs frame_with_particles) and the outer
      // frame fill must use the same glyph so the watch frame reads as one design,
      // not two independent sparkle treatments. Keep these in sync.
      const GUTTER_SPARKLE: &str = "\u{2726}";
      assert_eq!(
          super::frame_fill_for_stage(Stage::S6),
          GUTTER_SPARKLE,
          "S6 outer-frame fill must equal the pet-gutter sparkle glyph"
      );
  }
  ```

- [ ] Run, expect PASS (both are `✦` / `\u{2726}`):
  ```bash
  cargo test --lib s6_outer_frame_fill_matches_the_pet_gutter_sparkle
  ```
  Expected: passes. (If it fails, the glyphs diverged — fix the gutter sparkle constant in `render.rs` Task 5 to match `frame_fill_for_stage`, not the other way around, since the outer frame fill predates this work.)

- [ ] Add the reconciliation note to `frame_fill_for_stage`. Edit the doc comment above it (`layout.rs:205-207`) to append:
  ```rust
  /// Returns the horizontal border fill character for the outer frame, picked
  /// per stage tier. S0–S1 use a dotted line, S2–S3 the default rounded fill,
  /// S4–S5 a heavy line, S6 a sparkle. See the watch-visual-polish design.
  ///
  /// The S6 sparkle here is one of two coordinated sparkle surfaces: the pet's
  /// own gutter row 0 (render.rs `gutter_content_for` == `GutterContent::Sparkle`)
  /// uses the same `✦` glyph. Keep them in sync — see
  /// `s6_outer_frame_fill_matches_the_pet_gutter_sparkle_glyph`.
  ```

- [ ] Run the layout tests, expect PASS:
  ```bash
  cargo test --lib layout 2>&1 | tail -15
  ```
  Expected: all `layout::tests::*` pass.

- [ ] Commit:
  ```bash
  git -C /Users/drewritter/projects/glorp add src/tui/layout.rs
  git -C /Users/drewritter/projects/glorp commit -m "$(cat <<'EOF'
  test: coordinate the three S6 sparkle surfaces on one glyph

  Assert the outer-frame S6 fill equals the pet-gutter sparkle glyph so the watch
  frame reads as one design, and document the reconciliation on
  frame_fill_for_stage.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Task 8 — Document the retained-dead `morph_pup_index` / `morph_index` render decoupling and full-suite gate

**Files:**
- Modify: `/Users/drewritter/projects/glorp/src/pet/generation.rs` (comment only)
- Test: full suite

**Interfaces:**
- No new interface. `morph_index` / `morph_pup_index` remain on `VisibleTraits` (no `state.json` schema change — they are not persisted in `PetIdentity`, only recomputed) but are no longer consumed by `render_pet`. Removing the RNG draws would shift every later draw and rehue every pet beyond the accepted one-time reset's scope, so they are kept and documented as retained-dead.

**Steps:**

- [ ] Add a comment marking the draws as retained-dead at the render layer. Edit `src/pet/generation.rs` around the `morph_index` / `morph_pup_index` draws (lines ~193-194):
  ```rust
      let palette_index = rng.next_usize(8);
      // morph_index / morph_pup_index are retained-dead at the render layer: the
      // per-stage base-template map (pet/art.rs `stage_base_template`) replaced the
      // adult-pool / pup-pool indexing they fed. They are kept in the draw order so
      // existing seeds keep their downstream draws (seed_hue, saturation) stable;
      // removing them would rehue every pet beyond the accepted one-time reset.
      let morph_index = rng.next_usize(4);
      let morph_pup_index = rng.next_usize(4);
  ```

- [ ] Run the full test suite, expect PASS:
  ```bash
  cargo test 2>&1 | tail -30
  ```
  Expected: the whole suite passes (lib + all integration test files). If any preview-lab / watch test snapshots the S6 pet and was pinned to the old SAGE-framed art, it will fail — that is the expected one-time visual reset. Inspect the failure; if it is purely the S6 sparkle/art change, update the snapshot/fixture to the new rendering and note it in the commit. Do NOT delete the test.

- [ ] Run `cargo fmt --check` and the full clippy gate:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -15
  ```
  Expected: fmt clean (run `cargo fmt` if not), clippy clean.

- [ ] Confirm the roster still renders end-to-end via the dev-preview pets scenario (visual smoke; does not touch real state):
  ```bash
  cargo run -- dev-preview --scenario pets --out target/glorp-preview-phase1
  ls target/glorp-preview-phase1/frames | head
  ```
  Expected: the command succeeds and writes pet frames for every species×stage. (No assertion here — this is the human-reviewable smoke artifact; the automated guarantee is the continuity test in Task 6.)

- [ ] Commit:
  ```bash
  git -C /Users/drewritter/projects/glorp add src/pet/generation.rs
  git -C /Users/drewritter/projects/glorp commit -m "$(cat <<'EOF'
  docs: mark morph_index/morph_pup_index retained-dead at the render layer

  The per-stage base-template map replaced the pool indexing these draws fed; they
  stay in the RNG draw order to keep downstream per-seed draws stable under the
  accepted one-time visual reset.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Phase 1 exit checklist (the implementer must verify before declaring done)

- [ ] `cargo build` and `cargo test` both pass with pristine output.
- [ ] `cargo fmt --check` clean; `cargo clippy --all-targets --all-features -- -D warnings` clean.
- [ ] `template_lines`, `elder_morph_index`, `SAGE_TOP`, `SAGE_BOT` are gone; nothing references them (`grep -rn "template_lines\|elder_morph_index\|SAGE_TOP\|SAGE_BOT" src/ tests/` returns nothing).
- [ ] `stage_base_template`, `apply_interior_texture` (identity), `stage_template_lines`, redefined `morph_count` exist and are the only art-resolution path used by `render_pet`.
- [ ] The new `#[cfg(test)]` helpers exist: `rendered_occupied_cells`, `assert_in_stage_band` (self-tested; deferred over real art to Phase 2), `ambiguous_wide_width_warnings` (non-blocking), `assert_s6_fills_art_rows_no_sparkle` (passing).
- [ ] `GutterContent` / `gutter_content_for` (Mech-S6 == None), `feet_row`, `feet_columns` exist; the S6 sparkle is in gutter row 0 with species-particle precedence; reconciled with `frame_fill_for_stage`.
- [ ] The continuity test renders a valid non-empty 11×8 (framed 13×10) for the fixed seed set across all species×stages.
- [ ] No `state.json` schema change; identity data untouched.

## What Phase 1 deliberately does NOT do (hand-off to later phases)

- **No new art.** All 42 per-stage shapes are the existing art rewired; the bodies are redrawn in **Phase 2**, which also turns on `assert_in_stage_band` over the real templates as its growth-acceptance gate (the bands do not fit the placeholder art — see the reality-check table).
- **`apply_interior_texture` is an identity passthrough.** The real per-seed `▒`/`▓` texture math is **Phase 2**.
- **No palette / eye-color / chroma changes** (`PaletteRoleName::Corruption`, `eye_color_for_mood`, `species_base_hue` retune) — **Phases 3-4**.
- **No contact shadow / floor anchor.** `feet_row`/`feet_columns` are produced here but consumed in **Phase 5**.
- **No dev-preview schema bump.** The preview-lab variant/mood extension lands when **Phase 2/3** needs it.
