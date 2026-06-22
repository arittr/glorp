# Phase 2 — Per-Species Base Art + Algorithmic Variety Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax.

**Goal:** Wire the 42 validated per-(species, stage) base silhouettes into Phase 1's stage-template map, standardize a per-species mood-face vocabulary, add deterministic per-seed interior-texture variation, and extend the preview lab to show texture variants and the full mood set.

**Architecture:** Phase 1 already built the per-stage template-map API (`stage_base_template` / `apply_interior_texture` / `stage_template_lines`), redefined `morph_count` to mean interior-texture-variant count, and added the invariant test helpers (`rendered_occupied_cells`, `assert_in_stage_band`, `ambiguous_wide_width_warnings`, `assert_s6_fills_art_rows_no_sparkle`). Phase 2 (a) replaces each species' Phase-1-stub base bodies with the validated final silhouettes and asserts they pass the Phase-1 invariants, (b) standardizes the mood-glyph vocabulary in `expression_for` (`src/pet/render.rs`) via per-species mood-glyph sets, (c) fills in `apply_interior_texture` so it deterministically varies non-structural interior cells (`▒`/`▓`) and accent placement per `seed` while preserving the closed outline / width-1 / cell band, and (d) extends `src/dev_preview/pets.rs` to render representative interior-texture variants and the full mood set, bumping the manifest schema from v3 to v4.

**Tech Stack:** Rust, ratatui, SQLite; tests via cargo + assert_cmd.

## Global Constraints

- Templates are exactly 11 display columns × 8 lines; the `Template` type alias is `[&'static str; 8]` (`src/pet/art.rs`).
- Every art glyph (and every `{slot}` filler) is width-1 under unicode-width's default (ambiguous=narrow); enforced by `every_template_line_is_eleven_display_columns` (`src/pet/art.rs`).
- Eye/accent glyphs must be East-Asian-Width Neutral or Narrow, never Ambiguous; `◇◆◈●○` are Ambiguous (kept only per the Crystal decision — non-blocking lint).
- Growth cell bands (occupied non-space cells across the 8 art rows, fixed reference state): S0:1-4 · S1:5-10 · S2:11-20 · S3:21-34 · S4:35-50 · S5:51-66 · S6:67-88 — disjoint, strictly increasing, S4<S5<S6.
- S6 fills all 8 art rows; the sparkle no longer overwrites art rows 0/7 (asserted as a separate structural check, not in the size count).
- Color is truecolor-first, two tiers only: `ColorCapability::{Truecolor, Flat}` (`src/tui/style.rs`); honor `NO_COLOR`/`TERM=dumb`; under Flat pets render monochrome carried by silhouette; sub-truecolor is ratatui's automatic downgrade, not engineered here.
- Tamagotchi spirit: calm over flashy, night calmer than day, nurturing companion not optimizer; no death — floor state is `Mood::Wilted`.
- Only real signals drive content (growth/mood/biome/props/scene-moments trace to observed token usage + clock); the immature-pet zero-feast invariant is preserved (`flat_and_immature_pets_render_zero_motes`).
- The renderer stays content-agnostic: species/stage character lives in `art.rs` templates + palette, never in renderer special-casing.
- `cargo clippy --all-targets --all-features -- -D warnings` must stay clean; test-only helper fns must be `#[cfg(test)]`.
- Test output must be pristine; intentional error output must be captured and asserted.
- Test isolation: integration tests use `tempfile::tempdir()` + `GLORP_CONFIG_DIR`; when testing helper failures, pin BOTH `GLORP_CCUSAGE_BIN` and `GLORP_CCUSAGE_CODEX_BIN`.
- Commit frequently (do not ask first); WIP branch off `main`; never `git add -A` without a prior `git status`.
- Identity data is never touched: no `state.json` schema change; `seed`/`accepted_name`/`xp`/vitals/stage/calibration/seen-transitions untouched. A one-time visual reset is accepted.
- Do NOT call `apply_usage_poll` from production code (`#[doc(hidden)]` test wrapper).

---

## Preconditions (Phase 1 must be merged before starting)

This plan **consumes** Phase 1's public surface. Before Task 1, verify these exist in `src/pet/art.rs` (re-check file:line; Phase 1 introduced them):

```rust
type Template = [&'static str; 8];

// One hand-drawn base silhouette per (species, stage). 42 total.
pub(crate) fn stage_base_template(species: Species, stage: Stage) -> &'static Template;

// Deterministic per-seed interior-texture variation applied on top of the base.
pub(crate) fn apply_interior_texture(
    base: &Template,
    species: Species,
    stage: Stage,
    seed: u64,
) -> [String; 8];

// Public render entry replacing template_lines(...).
pub(crate) fn stage_template_lines(species: Species, stage: Stage, seed: u64) -> [String; 8];

// Interior-texture-variant count (>= 1), NOT a silhouette-pool size.
pub fn morph_count(species: Species, stage: Stage) -> usize;

// #[cfg(test)] invariant helpers:
#[cfg(test)] fn rendered_occupied_cells(species: Species, stage: Stage) -> usize;
#[cfg(test)] fn assert_in_stage_band(species: Species, stage: Stage);
#[cfg(test)] fn ambiguous_wide_width_warnings(species: Species, stage: Stage) -> Vec<char>;
#[cfg(test)] fn assert_s6_fills_art_rows_no_sparkle(species: Species);
```

And in `src/pet/render.rs`, `render_pet` calls `stage_template_lines(pet.species, stage, <seed>)` rather than the deleted `template_lines(...)`.

If any of these are missing, STOP — Phase 1 is incomplete; do not invent the surface.

**Verify before Task 1:**

```
- [ ] Run `cargo build` — expect clean compile (Phase 1 merged).
- [ ] Run `cargo test --lib pet::art` — expect Phase 1 invariant tests present and passing.
- [ ] `grep -n "fn stage_base_template" src/pet/art.rs` returns a hit.
- [ ] `grep -n "fn apply_interior_texture" src/pet/art.rs` returns a hit.
- [ ] `grep -n "fn rendered_occupied_cells" src/pet/art.rs` returns a hit.
- [ ] Create the WIP branch: `git checkout -b phase2-species-art`
```

---

## The validated art constants (named input — the art-pipeline deliverable)

The **42 base silhouette template constants** are produced by a separate art-generation pass (the draw-and-validate pipeline described in the spec's "Art production approach"): parallel subagents draw candidates under the grammar + cell bands; an audit pass machine-validates each grid against the Phase-1 invariants; survivors are embedded as Rust `Template` constants.

For this plan they are a **named input** delivered as a Rust source fragment per species, of the form:

```rust
// Provided by the art pipeline. Each is a [&'static str; 8] landing in its stage band.
const FUZZ_S0: Template = [ /* 8 lines, 11 cols, validated */ ];
const FUZZ_S1: Template = [ /* ... */ ];
// ... FUZZ_S2 .. FUZZ_S6 (7 constants per species)
```

Each species task's job is **wiring + asserting**, not drawing: drop the 7 provided constants into `art.rs`, point `stage_base_template`'s match arm for that species at them, and assert the Phase-1 machine invariants pass on the rendered output. The acceptance gate for a species task is **the machine invariants pass on its templates** — no subjective art review here (that happens in the preview lab, Task 9, and Phase-level review).

**If the art-pipeline fragment for a species is not yet available** when its task runs: the task is blocked. Do NOT hand-draw a substitute under time pressure — note the block and proceed to the mood-face / interior-texture / preview tasks (Tasks 7–9), which do not depend on the final bodies (they exercise whatever bodies are wired, including Phase 1's stubs).

---

## File Structure

| File | Create/Modify | Responsibility |
|---|---|---|
| `src/pet/art.rs` | Modify | Embed the 42 validated base `Template` constants; point `stage_base_template` arms at them; fill in `apply_interior_texture` body; add per-species art-band assertion tests. |
| `src/pet/render.rs` | Modify | Replace shared hardcoded mood glyphs in `expression_for` with per-species mood-glyph sets via a new `mood_face(species, mood)` helper + `closed_blink_eyes` reuse. |
| `src/pet/generation.rs` | Modify | (Only if a per-species resting eye/mouth vocab change is needed) keep `visible_traits` eye/mouth pools aligned with the standardized resting face. No new persisted field. |
| `src/dev_preview/pets.rs` | Modify | Render representative interior-texture variants per adult stage + the full mood set; new preview frames. |
| `src/dev_preview/export.rs` | Modify | Bump `SCHEMA_VERSION` 3 → 4; update the `schema_version` assertions. |
| `tests/generation.rs` | Modify | Rewrite `species_have_enough_seeded_morph_variety` to the new `morph_count` contract (drop `== 1` at S3 and the `>= 3` adult assertions). |

No new files. No `state.json` schema change. No new persisted `VisibleTraits` field — the interior-texture `seed` reuses the already-drawn `traits.seed_hue` (cast to `u64`), per CONTRACT §2.1.

---

## Interfaces

**Consumes (from Phase 1, exact signatures — do not redefine):**

```rust
pub(crate) fn stage_base_template(species: Species, stage: Stage) -> &'static Template;
pub(crate) fn apply_interior_texture(base: &Template, species: Species, stage: Stage, seed: u64) -> [String; 8];
pub(crate) fn stage_template_lines(species: Species, stage: Stage, seed: u64) -> [String; 8];
pub fn morph_count(species: Species, stage: Stage) -> usize;
#[cfg(test)] fn rendered_occupied_cells(species: Species, stage: Stage) -> usize;
#[cfg(test)] fn assert_in_stage_band(species: Species, stage: Stage);
#[cfg(test)] fn ambiguous_wide_width_warnings(species: Species, stage: Stage) -> Vec<char>;
#[cfg(test)] fn assert_s6_fills_art_rows_no_sparkle(species: Species);
type Template = [&'static str; 8]; // src/pet/art.rs
```

From existing code (stable, this repo today):

```rust
// src/pet/render.rs
pub fn closed_blink_eyes(species: Species) -> &'static str;
fn expression_for(pet: &GeneratedPet, mood: Mood, blinking: bool, frame: AnimationFrame) -> Expression;
struct Expression { eyes: String, mouth: String }
// src/game/metabolism.rs
pub enum Mood { Happy, Ecstatic, Content, Hungry, Sad, Sleepy, Wilted }
// src/pet/generation.rs
pub struct VisibleTraits { pub eyes: String, pub mouth: String, /* ... */ pub seed_hue: u16, /* ... */ }
```

**Produces (Phase 3 and later consume / verify against):**

```rust
// src/pet/render.rs — the standardized per-species mood-face vocabulary.
// Returns the (eyes-3-cell, mouth-1-cell) glyphs for a resting/expressive face.
// Phase 3 measures resting-eye contrast against this resting face.
fn mood_face(species: Species, mood: Mood) -> Expression;

// src/pet/art.rs — apply_interior_texture is FILLED IN (was a Phase-1 identity/stub).
// Post-condition guaranteed by Phase 2: for every (species, stage, seed) the
// rendered occupied-cell count stays inside the same stage band as the base, the
// outline is preserved, every line is width-1/11-cols, and on S0..S2 the texture
// is constant-occupancy (pinned). Phase 5 feet detection relies on the closed
// outline this preserves.

// src/dev_preview — manifest SCHEMA_VERSION is 4 (was 3); the pets scenario now
// emits texture-variant + mood-set frames.
```

---

## Task 1 — Rewrite the morph-variety test to the new `morph_count` contract

Phase 1 redefined `morph_count` to the interior-texture-variant count. The checked-in `tests/generation.rs::species_have_enough_seeded_morph_variety` still asserts the OLD semantics (`morph_count(species, S3) == 1` and `>= 3` at S4/S6). Phase 1 may have left it failing or stubbed; Phase 2 owns the final rewrite because Phase 2 defines the variant counts.

**Files:**
- Modify: `tests/generation.rs`
- Test path: `tests/generation.rs` (the test IS the deliverable)

**Interfaces:**
- Consumes: `pub fn morph_count(species: Species, stage: Stage) -> usize;`

**Steps:**

- [ ] Read the current test at `tests/generation.rs:182-197` (`species_have_enough_seeded_morph_variety`).
- [ ] Run it to see current state: `cargo test --test generation species_have_enough_seeded_morph_variety` — record whether it passes/fails/compiles (Phase 1 may have left it broken).
- [ ] Replace the whole test function with the new-contract version:

```rust
#[test]
fn morph_count_reports_interior_texture_variants_not_silhouette_pools() {
    // New contract (Phase 1): morph_count is the number of deterministic
    // interior-texture variants a (species, stage) can render — NOT a hand-drawn
    // silhouette-pool size. It is >= 1 for every stage, and pinned to 1 on the
    // small stages (S0..S2) where texture is constant-occupancy.
    for species in Species::all() {
        for stage in [Stage::S0, Stage::S1, Stage::S2] {
            assert_eq!(
                morph_count(species, stage),
                1,
                "{species:?} {stage:?}: small stages pin interior texture to 1 variant"
            );
        }
        for stage in [Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
            assert!(
                morph_count(species, stage) >= 1,
                "{species:?} {stage:?}: every stage renders at least one variant"
            );
        }
    }
}
```

- [ ] Run: `cargo test --test generation morph_count_reports_interior_texture_variants_not_silhouette_pools` — expect PASS (Phase 1's `morph_count` returns 1 for S0..S2 and `>= 1` elsewhere).
- [ ] Run the full file to catch collateral: `cargo test --test generation` — expect PASS for everything except possibly `species_have_enough_seeded_morph_variety` no longer existing (the rename removed it; confirm no other reference to that old name: `grep -rn "species_have_enough_seeded_morph_variety" tests/ src/` returns nothing).
- [ ] Commit:

```bash
git add tests/generation.rs
git commit -m "test: align morph_count test with interior-texture-variant contract

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2 — Wire the validated **Fuzz** S0–S6 base silhouettes into the stage map

Replace Fuzz's Phase-1 stub base bodies with the validated Hearthfloof cast and assert the Phase-1 machine invariants.

**Files:**
- Modify: `src/pet/art.rs` (Fuzz base constants + `stage_base_template` Fuzz arms)
- Test path: `src/pet/art.rs` `#[cfg(test)] mod tests` (new `fuzz_base_art_passes_phase1_invariants`)

**Interfaces:**
- Consumes: `stage_base_template`, `rendered_occupied_cells`, `assert_in_stage_band`, `ambiguous_wide_width_warnings`, `assert_s6_fills_art_rows_no_sparkle`, `every_template_line_is_eleven_display_columns` (Phase 1), `Template`.
- Produces: final Fuzz silhouettes behind `stage_base_template(Species::Fuzz, _)`.

**Steps:**

- [ ] Confirm the art-pipeline fragment for Fuzz is available (7 constants `FUZZ_S0..FUZZ_S6`, each a validated `Template`). If not, mark this task blocked and skip to Task 7.
- [ ] Write the failing band test FIRST (it will fail against the Phase-1 stub bodies until the real constants are wired):

```rust
#[test]
fn fuzz_base_art_passes_phase1_invariants() {
    let species = Species::Fuzz;
    for stage in ALL_STAGES {
        // width-1 / 11-col on the rendered base (canonical fillers).
        // Reuses the Phase-1 helper that renders with mood=Content, resting eyes.
        assert_in_stage_band(species, stage); // band membership + monotonicity
    }
    assert_s6_fills_art_rows_no_sparkle(species);
    // Non-blocking ambiguous-wide lint: Fuzz has no Crystal-style ◆◈, so it must
    // be clean (Fuzz keeps `o o`/`w` resting face, block-mass body ▒▓█).
    for stage in ALL_STAGES {
        let warnings = ambiguous_wide_width_warnings(species, stage);
        assert!(
            warnings.is_empty(),
            "Fuzz {stage:?} unexpectedly uses ambiguous-wide glyphs: {warnings:?}"
        );
    }
}
```

- [ ] Run: `cargo test --lib pet::art::tests::fuzz_base_art_passes_phase1_invariants` — expect FAIL (`assert_in_stage_band` panics: a stub stage lands outside its band, or S4/S5/S6 not strictly increasing).
- [ ] Paste the 7 validated Fuzz constants into `art.rs` (in the `// ── Fuzz ──` region), replacing any Phase-1 stub constants for Fuzz:

```rust
// ── Fuzz ── Hearthfloof: ear-cones + mitten-feet + heart-locket, block-mass
// edges (▒▓█). Validated by the art pipeline against the Phase-1 invariants.
const FUZZ_S0: Template = [/* 8 validated lines */];
const FUZZ_S1: Template = [/* ... */];
const FUZZ_S2: Template = [/* ... */];
const FUZZ_S3: Template = [/* ... */];
const FUZZ_S4: Template = [/* ... */];
const FUZZ_S5: Template = [/* ... */];
const FUZZ_S6: Template = [/* ... */]; // fills all 8 rows, no sparkle substitution
```

- [ ] Point `stage_base_template`'s Fuzz arms at the new constants (match on `(Species::Fuzz, Stage::S0) => &FUZZ_S0`, … `S6 => &FUZZ_S6`). If Phase 1 used a per-species helper (e.g. `fuzz_base(stage)`), update that helper instead — match Phase 1's structure exactly; read it before editing.
- [ ] Run: `cargo test --lib pet::art::tests::fuzz_base_art_passes_phase1_invariants` — expect PASS.
- [ ] Run the Phase-1 global invariants to catch width/height regressions: `cargo test --lib pet::art` — expect PASS (`every_template_line_is_eleven_display_columns`, `every_template_is_eight_lines` cover Fuzz now).
- [ ] Run the render smoke (no-blank-pet continuity): `cargo test --test generation render_is_stable_for_same_seed_state_and_tick` — expect PASS.
- [ ] Commit:

```bash
git add src/pet/art.rs
git commit -m "feat: wire validated Fuzz base silhouettes into stage map

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3 — Wire the validated **Blob** S0–S6 base silhouettes

Same wiring+assert pattern. Blob is the figure-ground stress case (soft body must still read solid with color off): the validated body has a `▒▓` core inside the `( )` outline.

**Files:**
- Modify: `src/pet/art.rs` (Blob constants + `stage_base_template` Blob arms)
- Test path: `src/pet/art.rs` tests (`blob_base_art_passes_phase1_invariants`)

**Interfaces:**
- Consumes: Phase-1 helpers as in Task 2.
- Produces: final Blob silhouettes behind `stage_base_template(Species::Blob, _)`.

**Steps:**

- [ ] Confirm the Blob art fragment (`BLOB_S0..BLOB_S6`) is available; else mark blocked, skip to Task 7.
- [ ] Write the failing test (note the extra flat-color figure-ground check Blob owns):

```rust
#[test]
fn blob_base_art_passes_phase1_invariants() {
    let species = Species::Blob;
    for stage in ALL_STAGES {
        assert_in_stage_band(species, stage);
    }
    assert_s6_fills_art_rows_no_sparkle(species);

    // Flat-color figure-ground: at every stage that has a body (S2+), the base
    // must contain at least one ▒ or ▓ core cell so it reads as solid with color
    // off — a ░-only body in a ░ dot field is the failure this guards.
    for stage in [Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
        let base = stage_base_template(species, stage);
        let has_dense_core = base
            .iter()
            .any(|line| line.contains('\u{2592}') || line.contains('\u{2593}'));
        assert!(
            has_dense_core,
            "Blob {stage:?} base must carry a ▒/▓ core for flat-color figure-ground"
        );
    }
}
```

- [ ] Run: `cargo test --lib pet::art::tests::blob_base_art_passes_phase1_invariants` — expect FAIL.
- [ ] Paste the 7 validated Blob constants; point `stage_base_template` Blob arms at them (mirror Phase 1's structure).
- [ ] Run: `cargo test --lib pet::art::tests::blob_base_art_passes_phase1_invariants` — expect PASS.
- [ ] Run: `cargo test --lib pet::art` — expect PASS.
- [ ] Commit:

```bash
git add src/pet/art.rs
git commit -m "feat: wire validated Blob base silhouettes with flat-color core

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4 — Wire the validated **Ghost** S0–S6 base silhouettes

Ghost grows via density + width; soft body that fades at edges but still passes figure-ground.

**Files:**
- Modify: `src/pet/art.rs` (Ghost constants + arms)
- Test path: `src/pet/art.rs` tests (`ghost_base_art_passes_phase1_invariants`)

**Interfaces:**
- Consumes: Phase-1 helpers.
- Produces: final Ghost silhouettes behind `stage_base_template(Species::Ghost, _)`.

**Steps:**

- [ ] Confirm the Ghost fragment (`GHOST_S0..GHOST_S6`) is available; else blocked, skip to Task 7.
- [ ] Write the failing test:

```rust
#[test]
fn ghost_base_art_passes_phase1_invariants() {
    let species = Species::Ghost;
    for stage in ALL_STAGES {
        assert_in_stage_band(species, stage);
    }
    assert_s6_fills_art_rows_no_sparkle(species);
    // Ghost is soft but must still read solid with color off: dense core at S2+.
    for stage in [Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
        let base = stage_base_template(species, stage);
        let has_dense_core = base
            .iter()
            .any(|line| line.contains('\u{2592}') || line.contains('\u{2593}'));
        assert!(has_dense_core, "Ghost {stage:?} base needs a ▒/▓ core");
    }
}
```

- [ ] Run: `cargo test --lib pet::art::tests::ghost_base_art_passes_phase1_invariants` — expect FAIL.
- [ ] Paste the 7 validated Ghost constants; point `stage_base_template` Ghost arms at them.
- [ ] Run: `cargo test --lib pet::art::tests::ghost_base_art_passes_phase1_invariants` — expect PASS.
- [ ] Run: `cargo test --lib pet::art` — expect PASS.
- [ ] Commit:

```bash
git add src/pet/art.rs
git commit -m "feat: wire validated Ghost base silhouettes into stage map

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5 — Wire the validated **Glitch** S0–S6 base silhouettes (the hero)

Glitch is the hero fix: closed packet-frame silhouette (not open `▌▐` walls), lens eyes `◉` at rest, torn-data base. S5 must drop into the **S5** band (not S6), S6 owns edge-to-edge. This task only asserts the silhouette/structure; the loud corruption animation is Phase 4.

**Files:**
- Modify: `src/pet/art.rs` (Glitch constants + arms)
- Test path: `src/pet/art.rs` tests (`glitch_base_art_passes_phase1_invariants`, `glitch_resting_face_is_alive`)

**Interfaces:**
- Consumes: Phase-1 helpers; `closed_blink_eyes` (unchanged).
- Produces: final Glitch silhouettes behind `stage_base_template(Species::Glitch, _)`.

**Steps:**

- [ ] Confirm the Glitch fragment (`GLITCH_S0..GLITCH_S6`) is available; else blocked, skip to Task 7.
- [ ] Note for the reviewer: the old `art.rs` tests `elder_morph_skips_singleton_for_glitch`, `elder_morph_skips_singleton_for_carved_species`, and `glitch_daemon_silhouette_is_visibly_denser_than_glitch_form` referenced `elder_morph_index`/`adult_templates`, which Phase 1 **deleted**. Confirm Phase 1 already removed those three tests (`grep -n "elder_morph_skips_singleton\|glitch_daemon_silhouette_is_visibly_denser" src/pet/art.rs` returns nothing). If any survive, they will not compile — STOP and confirm Phase 1 completion; do not delete tests yourself without flagging (per the no-silent-delete rule, this was Phase 1's documented rewrite).
- [ ] Write the failing band test:

```rust
#[test]
fn glitch_base_art_passes_phase1_invariants() {
    let species = Species::Glitch;
    for stage in ALL_STAGES {
        assert_in_stage_band(species, stage);
    }
    assert_s6_fills_art_rows_no_sparkle(species);
}
```

- [ ] Write the failing living-face test (the hero requirement: no `x x` corpse eyes at rest; the resting eye is the daemon lens). This renders the real pet, so it depends on Task 6's mood face being correct — but the Content resting face comes from `pet.traits.eyes`, which for Glitch is set in `visible_traits`. Assert the rendered S4 Content face does not contain corpse eyes and the body is non-empty:

```rust
#[test]
fn glitch_resting_face_is_alive() {
    use crate::pet::generation::generate_pet;
    use crate::game::metabolism::Mood;
    use crate::pet::render::{render_pet, AnimationFrame};
    let pet = generate_pet("glitch-face-seed")
        .with_species(Species::Glitch);
    let art = render_pet(
        &pet,
        Stage::S4,
        Mood::Content,
        AnimationFrame { tick: 1, ..AnimationFrame::default() },
    )
    .lines
    .join("\n");
    assert!(
        !art.contains("x x"),
        "Glitch resting face must be alive, never corpse eyes, got:\n{art}"
    );
    assert!(
        art.chars().filter(|c| !c.is_whitespace()).count() > 30,
        "Glitch S4 must render a dense closed packet, got:\n{art}"
    );
}
```

- [ ] Run both: `cargo test --lib pet::art::tests::glitch_base_art_passes_phase1_invariants pet::art::tests::glitch_resting_face_is_alive` — expect FAIL on the band test (stub bodies). The living-face test may already pass if the Glitch resting eye vocab is fixed in Task 6; if it fails because `visible_traits` can still draw `"x x"` for Glitch, that is fixed in Task 6 — re-run after Task 6.
- [ ] Paste the 7 validated Glitch constants; point `stage_base_template` Glitch arms at them.
- [ ] Run: `cargo test --lib pet::art::tests::glitch_base_art_passes_phase1_invariants` — expect PASS.
- [ ] Run: `cargo test --lib pet::art` — expect PASS.
- [ ] Commit:

```bash
git add src/pet/art.rs
git commit -m "feat: wire validated Glitch packet-daemon base silhouettes

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6 — Standardize the per-species mood-face vocabulary in `expression_for`

The resting/happy/tired/wilted faces are currently shared hardcoded glyphs in `expression_for` (`render.rs:258-313`), except Content which uses `pet.traits.eyes`/`mouth`. Standardize the mood vocabulary **per species** so each species reads as itself across the mood set, while keeping the 3-cell `{eyes}` / 1-cell `{mouth}` slot widths. This also fixes the Glitch corpse-eye problem (drop `"x x"` from the Glitch resting pool).

**Files:**
- Modify: `src/pet/render.rs` (new `mood_face`; rewire `expression_for`)
- Modify: `src/pet/generation.rs` (Glitch resting `eyes` pool: replace `"x x"` with daemon lens; keep all other pools)
- Test path: `src/pet/render.rs` `#[cfg(test)] mod tests` (`mood_faces_are_species_specific_and_width_correct`, `glitch_resting_eyes_pool_has_no_corpse_eyes`)

**Interfaces:**
- Consumes: `Mood`, `Species`, `closed_blink_eyes(species)`, `Expression`.
- Produces: `fn mood_face(species: Species, mood: Mood) -> Expression;` (Phase 3 measures resting-eye contrast against `mood_face(species, Mood::Content)`'s eyes).

**Design decisions (firm, no further choices):**
- Slot widths are **invariant**: eyes = exactly 3 display columns, mouth = exactly 1. Every glyph here is width-1 under ambiguous=narrow (the Phase-1 width test covers templates, but mood glyphs are substituted at render time — Task 6 adds a direct width assertion on `mood_face` output).
- `Mood::Content` keeps reading from `pet.traits.eyes`/`mouth` (the per-seed resting identity) — `mood_face` is **not** consulted for Content; `expression_for` keeps its existing Content arm. `mood_face` covers the six **non-Content** moods so they stop being one shared glyph set across all species.
- The resting/Content eye for Glitch is its lens; we remove `"x x"` from the Glitch `visible_traits.eyes` pool so no seed can roll corpse eyes.
- `Wilted` is glyph-only (Global Constraint: wilting never changes silhouette). `mood_face(_, Wilted)` returns desaturated-reading glyphs (e.g. `,_,` / `_`) but **never** shrinks the art (the art comes from `stage_template_lines`, untouched here).

**Steps:**

- [ ] Read `expression_for` (`render.rs:258-313`) and `visible_traits` Glitch eye pool (`generation.rs:150`: `Species::Glitch => pick(rng, &["0 0", "x x", "# #", "1 1"])`).
- [ ] Write the failing per-species width/identity test in `render.rs` tests:

```rust
#[test]
fn mood_faces_are_species_specific_and_width_correct() {
    use unicode_width::UnicodeWidthStr;
    let non_content = [
        Mood::Happy, Mood::Ecstatic, Mood::Hungry,
        Mood::Sad, Mood::Sleepy, Mood::Wilted,
    ];
    for species in Species::all() {
        for mood in non_content {
            let face = mood_face(species, mood);
            assert_eq!(
                UnicodeWidthStr::width(face.eyes.as_str()),
                3,
                "{species:?} {mood:?} eyes must be 3 cols, got {:?}",
                face.eyes
            );
            assert_eq!(
                UnicodeWidthStr::width(face.mouth.as_str()),
                1,
                "{species:?} {mood:?} mouth must be 1 col, got {:?}",
                face.mouth
            );
        }
    }
    // Species differentiation: at least one species' happy eyes differs from
    // another's (the vocabulary is not one shared set).
    assert_ne!(
        mood_face(Species::Glitch, Mood::Happy).eyes,
        mood_face(Species::Fuzz, Mood::Happy).eyes,
        "mood vocabulary must vary by species"
    );
}
```

- [ ] Run: `cargo test --lib pet::render::tests::mood_faces_are_species_specific_and_width_correct` — expect FAIL to compile (`mood_face` does not exist).
- [ ] Add `mood_face` to `render.rs` (above `expression_for`). Each arm returns an `Expression`; glyphs are width-1, slot widths exact. Glitch/Mech/Crystal get species-flavored faces; soft species (Fuzz/Blob/Ghost) keep the warm set:

```rust
/// Per-species mood-face vocabulary for the six non-Content moods. Content reads
/// from the per-seed `traits.eyes`/`mouth` (handled in `expression_for`). All
/// glyphs are width-1 under ambiguous=narrow; eyes occupy exactly 3 columns,
/// mouth exactly 1. Phase 3 measures resting-eye contrast against the Content
/// face; this covers the expressive moods so they read per-species rather than
/// as one shared set.
fn mood_face(species: Species, mood: Mood) -> Expression {
    // Shared warm set for the soft creatures (Fuzz/Blob/Ghost) — already the
    // current behavior, lifted here so it is one source of truth.
    let warm = |eyes: &str, mouth: &str| Expression {
        eyes: eyes.to_string(),
        mouth: mouth.to_string(),
    };
    match species {
        Species::Fuzz | Species::Blob | Species::Ghost => match mood {
            Mood::Happy => warm("^.^", "\u{03c9}"),     // ^.^ / ω
            Mood::Ecstatic => warm("*o*", "\u{25bd}"),  // *o* / ▽
            Mood::Hungry => warm("u.u", "o"),
            Mood::Sad => warm("T.T", "\u{fe35}"),       // ﹵ -> the existing ︵ form
            Mood::Sleepy => warm("-.-", "-"),
            Mood::Wilted => warm(",_,", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Glitch => match mood {
            // Daemon lens face: alive, never corpse. ◉ is EAW-Neutral (width-1).
            Mood::Happy => warm(">\u{25c9}<", "\u{02c4}"),   // >◉< / ˄
            Mood::Ecstatic => warm("\u{25c9}o\u{25c9}", "\u{25bd}"),
            Mood::Hungry => warm("o\u{25c9}o", "o"),
            Mood::Sad => warm("v\u{25c9}v", "\u{fe35}"),
            Mood::Sleepy => warm("-.-", "_"),
            Mood::Wilted => warm("x_x", "_"),              // wilted may dim the lens
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Crystal => match mood {
            // Facet eyes; ◇ is ambiguous-narrow (kept per the Crystal decision).
            Mood::Happy => warm("\u{25c7}^\u{25c7}", "v"),
            Mood::Ecstatic => warm("*\u{25c7}*", "\u{25bd}"),
            Mood::Hungry => warm("\u{25c7}.\u{25c7}", "o"),
            Mood::Sad => warm("\u{25c7}_\u{25c7}", "\u{fe35}"),
            Mood::Sleepy => warm("-.-", "_"),
            Mood::Wilted => warm(",_,", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Mech => match mood {
            // Optic-sensor face: square/bracket eyes read mechanical.
            Mood::Happy => warm("^=^", "v"),
            Mood::Ecstatic => warm("*o*", "\u{25bd}"),
            Mood::Hungry => warm("o=o", "o"),
            Mood::Sad => warm("v=v", "\u{fe35}"),
            Mood::Sleepy => warm("=.=", "_"),
            Mood::Wilted => warm("x_x", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
    }
}
```

- [ ] Rewire `expression_for` to call `mood_face` for the non-Content arms while keeping Content reading the per-seed trait, and keeping the existing blink / soft-eyes / work-accent overrides exactly as they are (those run AFTER the base mood face, unchanged):

```rust
fn expression_for(
    pet: &GeneratedPet,
    mood: Mood,
    blinking: bool,
    frame: AnimationFrame,
) -> Expression {
    if blinking {
        return Expression {
            eyes: closed_blink_eyes(pet.species).to_string(),
            mouth: pet.traits.mouth.clone(),
        };
    }

    let mut expr = match mood {
        Mood::Content => Expression {
            eyes: pet.traits.eyes.clone(),
            mouth: pet.traits.mouth.clone(),
        },
        other => mood_face(pet.species, other),
    };
    if frame.soft_eyes && matches!(mood, Mood::Content | Mood::Happy) {
        expr.eyes = "\u{02d8}.\u{02d8}".to_string(); // ˘.˘ relaxed, heavy-lidded
    }
    if matches!(mood, Mood::Happy | Mood::Content) {
        match frame.work_accent {
            WorkAccent::None => {}
            WorkAccent::Alert => expr.eyes = "^o^".to_string(),
            WorkAccent::Focused => expr.eyes = ">.<".to_string(),
            WorkAccent::Dreamy => expr.eyes = "u.u".to_string(),
        }
    }
    expr
}
```

- [ ] Run: `cargo test --lib pet::render::tests::mood_faces_are_species_specific_and_width_correct` — expect PASS.
- [ ] Run the existing render tests that assert specific glyphs: `cargo test --lib pet::render` — expect PASS. NOTE: `ecstatic_renders_the_star_eyes_and_blocks_blink` and `ecstatic_keeps_star_eyes_when_work_accent_is_dreamy` assert `*o*` for the **default** species (the seed `generate_pet("ecstatic-seed")` may resolve to any species). If they now fail because the seed resolves to Glitch (`◉o◉`) or Mech (`*o*` — fine), update those two tests to pin the species with `.with_species(...)` to the one whose ecstatic eyes the test asserts, OR assert the species-correct ecstatic eyes. Read the failing assertion and fix it to match the standardized vocabulary — do not weaken it.
- [ ] Now fix the Glitch resting eye pool in `generation.rs` so no seed rolls corpse eyes. Write the failing test first in `tests/generation.rs`:

```rust
#[test]
fn glitch_resting_eyes_pool_has_no_corpse_eyes() {
    use glorp::pet::generation::Species;
    // Probe many seeds; the Glitch resting (Content) eyes must never be "x x".
    for n in 0..500 {
        let pet = generate_pet(&format!("glitch-pool-{n}")).with_species(Species::Glitch);
        assert_ne!(
            pet.traits.eyes, "x x",
            "Glitch resting eyes must never be corpse eyes"
        );
    }
}
```

- [ ] Run: `cargo test --test generation glitch_resting_eyes_pool_has_no_corpse_eyes` — expect FAIL (the pool still contains `"x x"`).
- [ ] Edit `visible_traits` Glitch eyes pool (`generation.rs:150`), replacing `"x x"` with the daemon lens `"\u{25c9} \u{25c9}"` (◉ ◉, width-1 each, 3 cols total):

```rust
        Species::Glitch => pick(rng, &["0 0", "\u{25c9} \u{25c9}", "# #", "1 1"]),
```

- [ ] Run: `cargo test --test generation glitch_resting_eyes_pool_has_no_corpse_eyes` — expect PASS.
- [ ] Re-run the Glitch living-face test from Task 5: `cargo test --lib pet::art::tests::glitch_resting_face_is_alive` — expect PASS now.
- [ ] Run the width invariant across the new resting pool: `cargo test --lib pet::art` and `cargo test --test generation` — expect PASS (the new `◉ ◉` is 3 cols; `every_template_line_is_eleven_display_columns` renders templates with the canonical filler, not the pool, so add a direct check that the pool eyes are width-3 if not already covered — the `mood_faces...` test covers mood faces, the resting pool needs its own assertion):

```rust
#[test]
fn species_resting_eye_pools_are_three_columns() {
    use unicode_width::UnicodeWidthStr;
    // The Content (resting) eyes come from the per-seed pool, substituted into a
    // 3-col {eyes} slot; every pool entry must be exactly 3 display columns.
    for n in 0..500 {
        for species in Species::all() {
            let pet = generate_pet(&format!("eye-width-{n}")).with_species(species);
            assert_eq!(
                UnicodeWidthStr::width(pet.traits.eyes.as_str()),
                3,
                "{species:?} resting eyes {:?} must be 3 cols",
                pet.traits.eyes
            );
        }
    }
}
```

- [ ] Run: `cargo test --test generation species_resting_eye_pools_are_three_columns` — expect PASS.
- [ ] Run clippy: `cargo clippy --all-targets --all-features -- -D warnings` — expect clean.
- [ ] Commit:

```bash
git add src/pet/render.rs src/pet/generation.rs tests/generation.rs
git commit -m "feat: standardize per-species mood-face vocabulary

Glitch loses corpse eyes; resting lens face is the daemon signature.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7 — Wire the validated **Crystal** S0–S6 base silhouettes (with eye-fill aging)

Crystal's signature is `◇`→`◆`→`◈` eyes filling with age. `◇◆◈` are ambiguous-width; the CONTRACT keeps them under the documented ambiguous=narrow assumption, so the `ambiguous_wide_width_warnings` lint is **expected to warn** for Crystal (non-blocking) — assert it warns rather than fails.

**Files:**
- Modify: `src/pet/art.rs` (Crystal constants + arms)
- Test path: `src/pet/art.rs` tests (`crystal_base_art_passes_phase1_invariants`, `crystal_eye_fill_ages_with_stage`)

**Interfaces:**
- Consumes: Phase-1 helpers; `ambiguous_wide_width_warnings`.
- Produces: final Crystal silhouettes behind `stage_base_template(Species::Crystal, _)`.

**Steps:**

- [ ] Confirm the Crystal fragment (`CRYSTAL_S0..CRYSTAL_S6`) is available; else blocked, skip to Task 8.
- [ ] Write the failing band test (and the eye-fill aging structural check). The eye-fill glyph lives in the resting face, but Crystal's caged-core glyph is in the body template; assert the body's core glyph escalates `◇`(young) → `◆`(mid) → `◈`(elder):

```rust
#[test]
fn crystal_base_art_passes_phase1_invariants() {
    let species = Species::Crystal;
    for stage in ALL_STAGES {
        assert_in_stage_band(species, stage);
    }
    assert_s6_fills_art_rows_no_sparkle(species);
    // Crystal deliberately keeps ambiguous-wide ◆◈; the lint WARNS (non-blocking)
    // and we assert it surfaces them rather than silently dropping the signature.
    let mut any_warning = false;
    for stage in [Stage::S4, Stage::S5, Stage::S6] {
        if !ambiguous_wide_width_warnings(species, stage).is_empty() {
            any_warning = true;
        }
    }
    assert!(
        any_warning,
        "Crystal elder stages keep the ◆/◈ signature; the ambiguous lint should warn"
    );
}

#[test]
fn crystal_eye_fill_ages_with_stage() {
    // The caged-core glyph fills with age: a young facet (◇) at S2, a filled
    // diamond (◆) by S4, the multi-facet (◈) by S6. Structural, not subjective.
    let young = stage_base_template(Species::Crystal, Stage::S2).join("");
    let elder = stage_base_template(Species::Crystal, Stage::S6).join("");
    assert!(
        elder.contains('\u{25c8}') || elder.contains('\u{25c6}'),
        "Crystal S6 must show a filled facet core (◆/◈), got:\n{elder}"
    );
    // young may use the hollow ◇ — assert it is NOT already the elder ◈.
    assert!(
        !young.contains('\u{25c8}'),
        "Crystal S2 must not pre-show the elder ◈ facet, got:\n{young}"
    );
}
```

NOTE: `Template` is `[&'static str; 8]`; `.join("")` works on an array slice — if the compiler rejects `.join` on a fixed array, use `base.iter().copied().collect::<String>()`. Adjust to whatever Phase 1's `Template` supports; read `stage_base_template`'s return type first.

- [ ] Run: `cargo test --lib pet::art::tests::crystal_base_art_passes_phase1_invariants pet::art::tests::crystal_eye_fill_ages_with_stage` — expect FAIL (stub bodies).
- [ ] Paste the 7 validated Crystal constants; point `stage_base_template` Crystal arms at them. Ensure the core glyph escalates `◇`/`◆`/`◈` across stages per the eye-fill design.
- [ ] Run: `cargo test --lib pet::art::tests::crystal_base_art_passes_phase1_invariants pet::art::tests::crystal_eye_fill_ages_with_stage` — expect PASS.
- [ ] Run: `cargo test --lib pet::art` — expect PASS (the blocking width test runs ambiguous=narrow, where `◇◆◈` are width-1, so it stays green).
- [ ] Commit:

```bash
git add src/pet/art.rs
git commit -m "feat: wire validated Crystal base silhouettes with aging eye-fill

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8 — Wire the validated **Mech** S0–S6 base silhouettes (the legibility benchmark)

Mech is the box-draw war-frame: head/torso/legs bolt onto the chassis as it ages. It is the legibility benchmark — every stage must read unambiguously.

**Files:**
- Modify: `src/pet/art.rs` (Mech constants + arms)
- Test path: `src/pet/art.rs` tests (`mech_base_art_passes_phase1_invariants`)

**Interfaces:**
- Consumes: Phase-1 helpers.
- Produces: final Mech silhouettes behind `stage_base_template(Species::Mech, _)`.

**Steps:**

- [ ] Confirm the Mech fragment (`MECH_S0..MECH_S6`) is available; else blocked, skip to Task 9.
- [ ] Write the failing band test:

```rust
#[test]
fn mech_base_art_passes_phase1_invariants() {
    let species = Species::Mech;
    for stage in ALL_STAGES {
        assert_in_stage_band(species, stage);
    }
    assert_s6_fills_art_rows_no_sparkle(species);
    // Mech keeps its own chassis rows at S6 (gutter_content_for == None per the
    // Phase-1/CONTRACT Mech-S6 decision); assert S6 fills all 8 rows itself.
    let s6 = stage_base_template(species, Stage::S6);
    let nonblank_rows = s6
        .iter()
        .filter(|line| line.chars().any(|c| !c.is_whitespace()))
        .count();
    assert_eq!(nonblank_rows, 8, "Mech S6 must fill all 8 art rows itself");
}
```

- [ ] Run: `cargo test --lib pet::art::tests::mech_base_art_passes_phase1_invariants` — expect FAIL.
- [ ] Paste the 7 validated Mech constants; point `stage_base_template` Mech arms at them.
- [ ] Run: `cargo test --lib pet::art::tests::mech_base_art_passes_phase1_invariants` — expect PASS.
- [ ] Run the **all-species continuity** check (every species×stage renders non-empty 11×8): `cargo test --lib pet::art && cargo test --test generation` — expect PASS.
- [ ] Run the full suite to confirm all 6 species wired clean: `cargo test` — expect PASS.
- [ ] Commit:

```bash
git add src/pet/art.rs
git commit -m "feat: wire validated Mech base silhouettes into stage map

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9 — Fill in `apply_interior_texture` (deterministic per-seed variation)

Phase 1 introduced `apply_interior_texture` as an identity/stub (returns the base unchanged) so the roster rendered. Phase 2 fills its body so the same (species, stage) renders **distinct interior texture** per seed — varying which non-structural interior cells render `▒` vs `▓` and where a single accent cell lands — while preserving the closed outline, width-1, and the stage band. On S0–S2 it is pinned to constant-occupancy (no change in occupied-cell count), so the narrow small bands cannot be crossed.

**Files:**
- Modify: `src/pet/art.rs` (`apply_interior_texture` body + a small deterministic helper)
- Test path: `src/pet/art.rs` tests (`interior_texture_varies_per_seed`, `interior_texture_preserves_band_and_width`, `interior_texture_is_pinned_on_small_stages`)

**Interfaces:**
- Consumes: `stage_base_template`, `apply_interior_texture` (signature only — we write the body), Phase-1 band/width helpers, `Template`.
- Produces: the filled `apply_interior_texture`. Post-conditions (Phase 5 relies on the preserved closed outline for feet detection):
  - For all (species, stage, seed): rendered output is 8 lines, each width-1/11-cols.
  - Occupied-cell count stays in the **same band** as the base (constant-occupancy on S0–S2; on S3+ texture swaps glyph identity, not occupancy).
  - The outline cells (non-`░▒▓` structural glyphs, and the leftmost/rightmost non-space cell of each row) are **never** changed.

**Design (firm):** "Interior cell" = a cell whose glyph is one of the fill-shade set `{░ ▒ ▓}` AND is not the leftmost or rightmost non-space cell of its row (those bound the outline). The texture step:
1. Walks each line, identifies interior fill cells.
2. For each interior fill cell, deterministically maps `(seed, row, col)` to one of `{▒, ▓}` (never `░` → that would thin the body below figure-ground; never a space → that would change occupancy/break the outline).
3. Places **one** accent-tier cell (`▓`) at a seed-chosen interior position if the species/stage has an interior accent budget — but only by **upgrading an existing `▒` interior cell to `▓`**, never by occupying a space (occupancy invariant).
4. On S0–S2: skip steps 2–3 entirely (return the base lines as owned `String`s) — texture is pinned.

This guarantees occupancy is constant (we only swap `▒`↔`▓` among already-occupied interior cells) so no band is crossed, and the outline (first/last non-space per row + all non-fill glyphs) is untouched.

**Steps:**

- [ ] Read Phase 1's stub `apply_interior_texture` to match its exact return type and signature (`-> [String; 8]`).
- [ ] Write the failing per-seed variation test:

```rust
#[test]
fn interior_texture_varies_per_seed() {
    // On adult stages, two different seeds produce different interior texture.
    for species in Species::all() {
        let base = stage_base_template(species, Stage::S5);
        let a = apply_interior_texture(base, species, Stage::S5, 11);
        let b = apply_interior_texture(base, species, Stage::S5, 91037);
        // Some species/stage may have too few interior fill cells to vary; require
        // that at least one species varies (the mechanism works), and that no call
        // panics. Stronger per-species variation is exercised in the preview lab.
        if a != b {
            return;
        }
    }
    panic!("interior texture did not vary for any species at S5");
}
```

- [ ] Run: `cargo test --lib pet::art::tests::interior_texture_varies_per_seed` — expect FAIL (stub returns base unchanged for all seeds → all equal → panic).
- [ ] Write the band/width-preservation test:

```rust
#[test]
fn interior_texture_preserves_band_and_width() {
    use unicode_width::UnicodeWidthStr;
    for species in Species::all() {
        for stage in ALL_STAGES {
            let base = stage_base_template(species, stage);
            let base_occupied: usize = base
                .iter()
                .map(|l| l.chars().filter(|c| !c.is_whitespace()).count())
                .sum();
            for seed in [0u64, 7, 42, 9999, 123_456_789] {
                let textured = apply_interior_texture(base, species, stage, seed);
                assert_eq!(textured.len(), 8, "{species:?} {stage:?}: must stay 8 lines");
                let mut occupied = 0;
                for (row, line) in textured.iter().enumerate() {
                    assert_eq!(
                        UnicodeWidthStr::width(line.as_str()),
                        11,
                        "{species:?} {stage:?} seed={seed} row={row}: width must stay 11, got {:?}",
                        line
                    );
                    occupied += line.chars().filter(|c| !c.is_whitespace()).count();
                }
                assert_eq!(
                    occupied, base_occupied,
                    "{species:?} {stage:?} seed={seed}: occupancy must not change (band-safe)"
                );
            }
        }
    }
}
```

- [ ] Run: `cargo test --lib pet::art::tests::interior_texture_preserves_band_and_width` — expect PASS already (the stub identity trivially preserves everything). This test guards the post-conditions once we implement; keep it.
- [ ] Write the small-stage pin test:

```rust
#[test]
fn interior_texture_is_pinned_on_small_stages() {
    // S0..S2 must be byte-identical to the base across seeds (constant-occupancy,
    // the narrow bands cannot tolerate any swing).
    for species in Species::all() {
        for stage in [Stage::S0, Stage::S1, Stage::S2] {
            let base = stage_base_template(species, stage);
            let base_lines: Vec<String> = base.iter().map(|l| l.to_string()).collect();
            for seed in [0u64, 5, 500, 5_000_000] {
                let textured = apply_interior_texture(base, species, stage, seed);
                assert_eq!(
                    textured.to_vec(),
                    base_lines,
                    "{species:?} {stage:?} seed={seed}: small stages must be pinned"
                );
            }
        }
    }
}
```

- [ ] Run: `cargo test --lib pet::art::tests::interior_texture_is_pinned_on_small_stages` — expect PASS (stub identity passes; locks the pin).
- [ ] Now implement the body. Replace the stub `apply_interior_texture` with the real swap logic + a deterministic hash helper:

```rust
/// Deterministic interior-texture variation. Swaps interior fill cells between
/// ▒ and ▓ per `seed`, preserving the closed outline, width-1, and occupancy
/// (so the stage cell band is never crossed). Pinned (no-op) on S0..S2.
pub(crate) fn apply_interior_texture(
    base: &Template,
    _species: Species,
    stage: Stage,
    seed: u64,
) -> [String; 8] {
    let pinned = matches!(stage, Stage::S0 | Stage::S1 | Stage::S2);
    let mut out: [String; 8] = Default::default();
    for (row, line) in base.iter().enumerate() {
        if pinned {
            out[row] = (*line).to_string();
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        // Find the outline bounds (first/last non-space cell) to protect them.
        let first = chars.iter().position(|c| !c.is_whitespace());
        let last = chars.iter().rposition(|c| !c.is_whitespace());
        let mut rebuilt = String::with_capacity(line.len());
        for (col, &ch) in chars.iter().enumerate() {
            let is_edge = Some(col) == first || Some(col) == last;
            let is_interior_fill = !is_edge
                && matches!(ch, '\u{2592}' | '\u{2593}'); // ▒ or ▓
            if is_interior_fill {
                // Hash (seed, row, col) -> pick ▒ or ▓. Never ░, never space.
                let h = mix_seed(seed, row as u64, col as u64);
                rebuilt.push(if h & 1 == 0 { '\u{2592}' } else { '\u{2593}' });
            } else {
                rebuilt.push(ch);
            }
        }
        out[row] = rebuilt;
    }
    out
}

/// Small deterministic mixer for interior-texture draws. Not crypto; just a
/// stable per-(seed,row,col) bit source independent of std hashing iteration.
fn mix_seed(seed: u64, row: u64, col: u64) -> u64 {
    let mut x = seed
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(row.wrapping_mul(0x0000_0100_0000_01b3))
        .wrapping_add(col.wrapping_mul(0xff51_afd7_ed55_8ccd));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 29;
    x
}
```

NOTE on `_species`: the swap logic is species-agnostic (it operates on the `▒▓` interior glyphs any species template uses). The parameter is kept for the Phase-1 signature contract; the leading underscore silences the unused-variable lint. If clippy still flags it, keep the underscore — do NOT change the signature (Phase 1 owns it and `stage_template_lines` calls it positionally).

- [ ] Run: `cargo test --lib pet::art::tests::interior_texture_varies_per_seed` — expect PASS (adult stages with ≥2 interior `▒▓` cells now vary).
- [ ] Run: `cargo test --lib pet::art::tests::interior_texture_preserves_band_and_width pet::art::tests::interior_texture_is_pinned_on_small_stages` — expect PASS (occupancy unchanged; S0–S2 byte-identical).
- [ ] Run all `art.rs` invariants + the render path that calls `stage_template_lines`: `cargo test --lib pet::art && cargo test --test generation render_is_stable_for_same_seed_state_and_tick different_same_species_seeds_have_visible_variation` — expect PASS. (`different_same_species_seeds_have_visible_variation` now also benefits from interior texture, not just morph indexing.)
- [ ] Run clippy: `cargo clippy --all-targets --all-features -- -D warnings` — expect clean.
- [ ] Commit:

```bash
git add src/pet/art.rs
git commit -m "feat: deterministic per-seed interior-texture variation

Swaps interior fill cells ▒/▓ by seed, preserving outline, width, and band;
pinned on S0-S2 so the narrow bands stay constant-occupancy.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10 — Extend the preview lab: texture variants + full mood set; bump manifest schema to v4

`src/dev_preview/pets.rs` renders one fixed seed per species (one cell per species×stage) plus a Glitch live-state strip. It cannot backstop per-pet variety or the mood set. Add (a) representative interior-texture variants per adult stage (3 distinct seeds side-by-side at S4/S5/S6 per species) and (b) the full mood set for a representative pet. Bump the manifest `SCHEMA_VERSION` 3 → 4 because the pets scenario's frame inventory changes.

**Files:**
- Modify: `src/dev_preview/pets.rs` (new frames `render_texture_variants`, `render_mood_set`)
- Modify: `src/dev_preview/export.rs` (`SCHEMA_VERSION` 3 → 4; assertion 3 → 4)
- Test path: `src/dev_preview/pets.rs` tests + `src/dev_preview/export.rs` schema test

**Interfaces:**
- Consumes: `generate_pet`, `with_species`, `render_pet`, `resolve_pet_palette`, `pet_role_spans_for_line`, `AnimationFrame`, `Mood`, `stage_template_lines` (indirectly, via `render_pet`).
- Produces: new preview frame ids `"pet-texture-variants"`, `"pet-mood-set"`; manifest `SCHEMA_VERSION == 4`.

**Steps:**

- [ ] Read `src/dev_preview/pets.rs` `pet_frames` (returns `Vec<PreviewFrame>`) and the helper `render_pet_cell` / `frame_from_buffer` to match the construction pattern.
- [ ] Write the failing test for the new frame ids in `pets.rs` tests:

```rust
#[test]
fn pets_preview_includes_texture_variant_and_mood_set_frames() {
    let ctx = PreviewRenderContext::deterministic();
    let frames = pet_frames(&ctx).unwrap();
    let ids: Vec<&str> = frames.iter().map(|f| f.id.as_str()).collect();
    assert!(
        ids.contains(&"pet-texture-variants"),
        "pets preview must show per-seed interior-texture variants; got {ids:?}"
    );
    assert!(
        ids.contains(&"pet-mood-set"),
        "pets preview must show the full mood set; got {ids:?}"
    );
}
```

- [ ] Run: `cargo test --features dev-preview --lib dev_preview::pets::tests::pets_preview_includes_texture_variant_and_mood_set_frames` — expect FAIL (frames absent). (If the lib builds preview behind a feature, use `--features dev-preview`; confirm the feature name from `Cargo.toml` — `grep -n "dev-preview" Cargo.toml`.)
- [ ] Add a texture-variants frame. Three distinct seeds per (species, adult stage), rendered side by side so a reviewer sees the interior-texture variation:

```rust
const TEXTURE_VARIANT_SEEDS: [&str; 3] =
    ["glorp-tex-a", "glorp-tex-b", "glorp-tex-c"];
const ADULT_STAGES: [Stage; 3] = [Stage::S4, Stage::S5, Stage::S6];

fn render_texture_variants(_ctx: &PreviewRenderContext) -> PreviewFrame {
    let styles = semantic_styles();
    // Layout: one row band per (species, adult stage); 3 variant columns.
    let row_height: u16 = 11;
    let band_count = (Species::all().len() * ADULT_STAGES.len()) as u16;
    let height = HEADER_HEIGHT + band_count * row_height;
    let col_width = FRAME_WIDTH / TEXTURE_VARIANT_SEEDS.len() as u16;
    let mut buffer = Buffer::empty(Rect::new(0, 0, FRAME_WIDTH, height));

    let mut band = 0u16;
    for species in Species::all() {
        for stage in ADULT_STAGES {
            for (col, seed) in TEXTURE_VARIANT_SEEDS.iter().enumerate() {
                let area = Rect::new(
                    col as u16 * col_width,
                    HEADER_HEIGHT + band * row_height,
                    col_width,
                    row_height,
                );
                render_seeded_pet_cell(area, &mut buffer, species, stage, seed, &styles);
            }
            band += 1;
        }
    }
    frame_from_buffer(
        "pet-texture-variants",
        "Pet Interior-Texture Variants",
        &buffer,
    )
}

fn render_seeded_pet_cell(
    area: Rect,
    buffer: &mut Buffer,
    species: Species,
    stage: Stage,
    seed: &str,
    styles: &crate::tui::style::SemanticStyles,
) {
    let pet = generate_pet(seed).with_species(species);
    let palette = crate::pet::palette::resolve_pet_palette(species, &pet.traits);
    let rendered = render_pet(
        &pet,
        stage,
        Mood::Content,
        AnimationFrame { tick: 0, ..AnimationFrame::default() },
    );
    let mut lines = vec![Line::styled(
        format!("{} s{} {}", species.as_str(), stage_index(stage), seed),
        styles.label,
    )];
    for (line_index, art_line) in rendered.lines.iter().enumerate() {
        let spans = pet_role_spans_for_line(
            art_line, line_index, &rendered.spans, styles, &palette, None,
        );
        lines.push(Line::from(spans));
    }
    Paragraph::new(lines).render(area, buffer);
}
```

NOTE: confirm `semantic_styles()` returns a type named `SemanticStyles` (or whatever `styles`'s type is) — read the import; adjust the `styles: &...` param type to the real type. If `AnimationFrame::default()` is not in scope here, import `crate::pet::render::WorkAccent` and construct the frame fully as the existing `render_pet_cell` does.

- [ ] Add a mood-set frame. The full mood set for one representative pet per species (or one fixed species across all moods — pick a representative pet per species so Phase 3 can review eye-color-by-mood per species):

```rust
const MOOD_SET: [(Mood, &str); 7] = [
    (Mood::Content, "content"),
    (Mood::Happy, "happy"),
    (Mood::Ecstatic, "ecstatic"),
    (Mood::Hungry, "hungry"),
    (Mood::Sad, "sad"),
    (Mood::Sleepy, "sleepy"),
    (Mood::Wilted, "wilted"),
];

fn render_mood_set(_ctx: &PreviewRenderContext) -> PreviewFrame {
    let styles = semantic_styles();
    let row_height: u16 = 11;
    let col_width = FRAME_WIDTH / MOOD_SET.len() as u16;
    let height = HEADER_HEIGHT + Species::all().len() as u16 * row_height;
    let mut buffer = Buffer::empty(Rect::new(0, 0, FRAME_WIDTH, height));

    for (mood_col, (_, label)) in MOOD_SET.iter().enumerate() {
        let area = Rect::new(
            mood_col as u16 * col_width, 0, col_width, HEADER_HEIGHT,
        );
        Paragraph::new(Line::styled(*label, styles.section_header))
            .render(area, &mut buffer);
    }

    for (row, species) in Species::all().into_iter().enumerate() {
        let pet = generate_pet(&format!("glorp-mood-{}", species.as_str()))
            .with_species(species);
        let palette = crate::pet::palette::resolve_pet_palette(species, &pet.traits);
        for (mood_col, (mood, _)) in MOOD_SET.iter().enumerate() {
            let area = Rect::new(
                mood_col as u16 * col_width,
                HEADER_HEIGHT + row as u16 * row_height,
                col_width,
                row_height,
            );
            let rendered = render_pet(
                &pet,
                Stage::S4,
                *mood,
                AnimationFrame { tick: 1, ..AnimationFrame::default() },
            );
            let mut lines = vec![Line::styled(species.as_str(), styles.label)];
            for (line_index, art_line) in rendered.lines.iter().enumerate() {
                let spans = pet_role_spans_for_line(
                    art_line, line_index, &rendered.spans, &styles, &palette, None,
                );
                lines.push(Line::from(spans));
            }
            Paragraph::new(lines).render(area, &mut buffer);
        }
    }
    frame_from_buffer("pet-mood-set", "Pet Mood Set", &buffer)
}
```

- [ ] Add both frames to `pet_frames`'s returned `Vec`:

```rust
pub fn pet_frames(ctx: &PreviewRenderContext) -> Result<Vec<PreviewFrame>> {
    Ok(vec![
        render_pet_matrix(ctx, "pet-species-stage", "Pet Species Stage", ColorCapability::Truecolor),
        render_pet_matrix(ctx, "pet-species-stage-flat", "Pet Species Stage (Flat)", ColorCapability::Flat),
        render_texture_variants(ctx),
        render_mood_set(ctx),
        render_glitch_live_states(ctx),
    ])
}
```

- [ ] Run: `cargo test --features dev-preview --lib dev_preview::pets::tests::pets_preview_includes_texture_variant_and_mood_set_frames` — expect PASS.
- [ ] Bump the manifest schema. Write the failing assertion update first in `export.rs` tests (`manifest_has_versioned_producer_and_artifact_types` at `export.rs:942` asserts `json["schema_version"] == 3`):

```rust
        assert_eq!(json["schema_version"], 4);
```

- [ ] Run: `cargo test --features dev-preview --lib dev_preview::export` — expect FAIL (`SCHEMA_VERSION` still 3).
- [ ] Bump the constant at `export.rs:12`:

```rust
pub const SCHEMA_VERSION: u32 = 4;
```

- [ ] Check for any OTHER schema_version=3 assertion: `grep -rn "schema_version\"\], 3\|schema_version, 3\|\"schema_version\": 3" src/`. The `scenarios.rs` uses the `SCHEMA_VERSION` constant (auto-follows). Update any remaining hardcoded `3` test expectations to `4`. Leave `output.rs:159` (`schema_version:1`) and `contract.rs` (`CONTRACT_SCHEMA_VERSION`) alone — different schemas.
- [ ] Run: `cargo test --features dev-preview --lib dev_preview` — expect PASS.
- [ ] Run the dev-preview integration test: `cargo test --features dev-preview --test dev_preview` — expect PASS (re-check the manifest schema assertion there too; `grep -rn "schema_version" tests/dev_preview.rs` and bump any `3` → `4`).
- [ ] Run clippy with the feature: `cargo clippy --all-targets --all-features -- -D warnings` — expect clean.
- [ ] Generate a real preview bundle and eyeball it (manual review gate for the art):

```bash
cargo run --features dev-preview -- dev-preview --scenario pets --out target/glorp-preview
open target/glorp-preview/index.html
```

  Confirm by eye: texture variants differ within a species×stage; mood set reads per-species; Glitch reads intentional; growth sorts S0→S6.
- [ ] Commit:

```bash
git add src/dev_preview/pets.rs src/dev_preview/export.rs tests/dev_preview.rs
git commit -m "feat: preview-lab texture variants + mood set; bump manifest schema v4

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 11 — Phase 2 acceptance: full-suite + clippy gate + roster render

Confirm the whole phase is shippable: all invariants pass, no crash / no blank pet, clippy clean, roster renders.

**Files:**
- No new code. Verification + a single roster-render continuity test if not already present.

**Interfaces:** none new.

**Steps:**

- [ ] Confirm the all-species×stage no-blank-pet continuity test exists (CONTRACT/spec require it). If `art.rs` or `tests/generation.rs` lacks one, add it to `tests/generation.rs`:

```rust
#[test]
fn every_species_stage_renders_a_nonempty_pet() {
    use glorp::pet::generation::Species;
    let stages = [
        Stage::S0, Stage::S1, Stage::S2, Stage::S3,
        Stage::S4, Stage::S5, Stage::S6,
    ];
    for seed in ["alpha", "mochi-7f3a", "ori-shard", "bolt-42"] {
        for species in Species::all() {
            let pet = generate_pet(seed).with_species(species);
            for stage in stages {
                let rendered = render_pet(&pet, stage, Mood::Content, frame(0));
                assert_eq!(rendered.lines.len(), 10, "{species:?} {stage:?}: 13x10 frame");
                assert!(
                    rendered.lines.iter().any(|l| l.chars().any(|c| !c.is_whitespace())),
                    "{species:?} {stage:?}: pet must not render blank"
                );
            }
        }
    }
}
```

  NOTE: `render_pet` wraps art in the 13×10 frame, so `lines.len()` is 10, not 8. Confirm against `FRAME_HEIGHT` in `render.rs` (currently 10) before asserting; if Phase 1 changed the frame height, match the new value.
- [ ] Run: `cargo test --test generation every_species_stage_renders_a_nonempty_pet` — expect PASS.
- [ ] Run the FULL suite: `cargo test` — expect all PASS.
- [ ] Run the FULL suite with the preview feature: `cargo test --features dev-preview` — expect all PASS.
- [ ] Run the CI clippy gate: `cargo clippy --all-targets --all-features -- -D warnings` — expect clean (zero warnings).
- [ ] Run fmt check: `cargo fmt --check` — expect clean (run `cargo fmt` and re-commit if not).
- [ ] Verify the roster renders live (sanity, not blocking): `GLORP_CONFIG_DIR=/tmp/glorp-phase2 cargo run -- init --yes --seed test --name buddy && GLORP_CONFIG_DIR=/tmp/glorp-phase2 cargo run -- status` — expect a rendered pet, no panic.
- [ ] Commit any continuity test / fmt fixes:

```bash
git add tests/generation.rs
git commit -m "test: assert every species-stage renders a non-empty pet

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

- [ ] Phase 2 complete. The branch `phase2-species-art` is ready for review/merge per the finishing-a-development-branch flow.

---

## Notes for the reconciler

- **Seed source for interior texture:** Phase 2 feeds `apply_interior_texture`/`stage_template_lines` the already-drawn `traits.seed_hue` (cast `u64`), per CONTRACT §2.1's default. No new `VisibleTraits` field, no `state.json` change. If Phase 1 instead added a dedicated `interior_texture_seed` draw, Phase 2 uses that field instead — read `render_pet`'s call site and match whatever seed Phase 1 wired; do not add a second seed.
- **`mood_face` is the Phase 3 contrast anchor.** Phase 3 measures resting-eye ≥3:1 luminance contrast against the species body; the *resting* (Content) eye still comes from `traits.eyes`, and the expressive moods now come from `mood_face`. Phase 3 should measure against `mood_face(species, Mood::Content)` semantics OR the `traits.eyes` resting glyph — flag which to the architect; Phase 2 left Content on the per-seed trait deliberately (it is the per-pet identity).
- **Ecstatic glyph tests:** Task 6 may require touching `pet::render::tests::ecstatic_renders_the_star_eyes_and_blocks_blink` and `_when_work_accent_is_dreamy` to pin the species, because the standardized vocabulary makes ecstatic eyes species-specific (Glitch = `◉o◉`, not `*o*`). These were not in the CONTRACT's rewrite list but are direct consequences of the mood-face standardization — the rewrite is mechanical (pin species or assert the species-correct glyph), never weakening.
