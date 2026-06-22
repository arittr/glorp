# Glorp Visual Overhaul — Master Execution README

Status: GO for autonomous overnight execution (with one hard pre-flight gate — see
"Phase 2 art is a required input"). Date: 2026-06-21.

This README is the orchestration contract for the five-phase glorp visual overhaul.
It governs how the phase plans in this directory are run unattended, in what order,
and what proves each phase done.

Phase plans (run in this order):

1. `2026-06-21-glorp-overhaul-phase1-foundation.md`
2. `2026-06-21-glorp-overhaul-phase2-species-art.md`
3. `2026-06-21-glorp-overhaul-phase3-color-eyes-mood.md`
4. `2026-06-21-glorp-overhaul-phase4-liveliness.md`
5. `2026-06-21-glorp-overhaul-phase5-habitat-grounding.md`

Design source of truth: `../specs/2026-06-21-glorp-visual-overhaul-design.md`.
The plans reference it as "the contract / CONTRACT §x.y"; there is no separate
contract file — the spec *is* the contract.

---

## Mandated phase order (1 → 5) and why

The phases are sequenced by a hard data dependency, not preference. Each phase
*consumes* a named interface that the previous phase *produces*. Running out of
order means consuming a symbol that does not exist yet.

- **Phase 1 (Foundation) first** because it builds the entire art-resolution API
  every later phase calls: `stage_base_template`, `apply_interior_texture`,
  `stage_template_lines`, the redefined `morph_count`, the `#[cfg(test)]` invariant
  helpers (`rendered_occupied_cells`, `assert_in_stage_band`,
  `ambiguous_wide_width_warnings`, `assert_s6_fills_art_rows_no_sparkle`), and the
  `GutterContent` / `gutter_content_for` / `feet_row` / `feet_columns` surface. It
  rewires the *existing* art into the new per-stage map so the roster keeps
  rendering. It draws no new art and changes no color.
- **Phase 2 (Per-species art)** consumes Phase 1's stage-template API and replaces
  the stub bodies with the 42 validated silhouettes, standardizes the mood-face
  vocabulary (`mood_face`), fills `apply_interior_texture`, and turns the band /
  monotonicity invariants on over the *real* art (they cannot pass on Phase 1's
  placeholder art — that is by design, see the Phase 1 "reality check" table).
- **Phase 3 (Color & eyes-mood)** consumes Phase 2's silhouettes + mood faces. It
  adds the `particle` field to `ResolvedPalette`, retunes hue/chroma, and adds the
  mood→eye-color path (`eye_color_for_mood`, `apply_mood_eye_color`). Its
  flat-color figure-ground check (Task 10) and ≥3:1 contrast floor (Task 5) assume
  Phase 2's bodies carry a `▒▓` core — they fail loudly if Phase 2 art is too
  sparse, surfacing a Phase 2 art bug rather than papering over it.
- **Phase 4 (Liveliness)** consumes Phase 3's palette/role plumbing. It adds the
  `Corruption` palette role on top of the same `role_color` / `colors.rs` /
  presentation surfaces Phase 3 touched, and deletes the dead breath fields.
- **Phase 5 (Habitat grounding)** consumes Phase 1's `feet_row` / `feet_columns` /
  `GutterContent` precedence to anchor the pet to the floor and paint a
  feet-restricted contact shadow. It depends on Phase 1, not on 2/3/4, but runs
  last so grounding is validated against the final art + color.

Phases are independently shippable: the roster renders between every phase.

---

## Phase 2 art is a required input (the one hard pre-flight gate)

Phase 2's 42 base silhouette constants (`FUZZ_S0..FUZZ_S6`, `BLOB_S0..`, etc.) are a
**named input**, produced by the separate art-generation pass described in the
spec's "Art production approach": parallel subagents draw candidates under the
grammar + cell bands; an audit pass machine-validates every grid against the Phase 1
invariants (11×8, width-1 incl. `ambiguous=wide`, the stage cell band, rendered-size
monotonicity); survivors are embedded as Rust `Template` constants in `art.rs`.

Phase 2 correctly treats these as an input, not something it invents: each species
task says "If the art-pipeline fragment for a species is not yet available... the
task is blocked. Do NOT hand-draw a substitute under time pressure." The acceptance
gate for a species wiring task is purely machine-checkable — the Phase 1 invariants
pass on the rendered output — so no subjective art judgment is needed at wiring time.

**Consequence for overnight autonomy:** as of this audit the art-generation
deliverable does not yet exist on disk (no file embeds the 42 constants). If Phase 2
runs without them, its six species-wiring tasks (Tasks 2–5, 7, 8) BLOCK, and only the
art-independent tasks complete (Task 1 morph_count test, Task 6 mood faces, Task 9
interior texture, Task 10 preview lab, Task 11 acceptance) — leaving the roster on
Phase 1's rewired *existing* art, not the new cast.

**Therefore the art-generation pass MUST be run and its constants embedded in
`art.rs` before the Phase 2 overnight run starts.** Two acceptable orderings:

- Run the art-generation pass as "Phase 2, Task 0" earlier in the evening, commit the
  validated constants, then start the Phase 2 plan. (Preferred.)
- Or hold Phase 2 until the art exists and run Phases 3–5 only against Phase 1's
  rewired roster (Phases 3–5 do not depend on the *new* bodies to compile — they
  exercise whatever bodies are wired — but Phase 3 Task 10 / Task 5 acceptance bars
  assume the new Blob/Ghost cores and will flag the old art as a gap).

The art constants are validated by the SAME Phase 1 invariants the wiring tasks
re-assert, so "validated by the art pipeline" and "passes the wiring task's gate" are
the same bar.

---

## How to run it overnight

Use the `superpowers:subagent-driven-development` skill, **one phase at a time**,
feeding it that phase's plan file. Each phase's tasks are red→green→refactor→commit
and self-contained; the skill dispatches a subagent per task and reviews between
tasks.

For each phase, in order:

1. Confirm the previous phase merged/committed clean (its exit checklist passed).
2. Invoke `superpowers:subagent-driven-development` with the phase plan file as the
   plan to execute.
3. At the phase's acceptance task (the last task in each plan), confirm the gate
   below is green before moving on.

Do NOT batch phases into one subagent run — the phase boundary is where the
interface contract is verified and where a clean commit/branch checkpoint lives.

Each phase plan creates its own WIP branch off the integration branch:
`glorp-overhaul-phase1-foundation`, `phase2-species-art`, `phase3-color-eyes-mood`,
`phase4-liveliness`, `phase5-habitat-grounding`. (The plans branch off `main` in
their literal commands; since the integration branch is `glorp-visual-overhaul`,
branch each phase off `glorp-visual-overhaul` instead and merge back to it — keep
the phase-branch names.) Commit frequently per task; no PR unless Drew asks.

---

## Per-phase acceptance gate

A phase is done only when its gate is green. Every phase additionally requires
`cargo fmt --check` clean and `cargo clippy --all-targets --all-features -- -D
warnings` clean with pristine test output.

- **Phase 1:** `template_lines` / `elder_morph_index` / `SAGE_TOP` / `SAGE_BOT` are
  gone (`grep` returns nothing); `stage_base_template` / `apply_interior_texture`
  (identity) / `stage_template_lines` / redefined `morph_count` are the only
  art-resolution path `render_pet` uses; the `#[cfg(test)]` helpers exist and
  self-test; `GutterContent` / `gutter_content_for` (Mech-S6 = `None`) / `feet_row` /
  `feet_columns` exist; the S6 sparkle is in gutter row 0 with species-particle
  precedence and is reconciled with `frame_fill_for_stage` (same `✦` glyph); the
  fixed-seed continuity test renders a valid non-empty 11×8 (framed 13×10) for every
  species×stage; no `state.json` schema change. Band/monotonicity over real art is
  NOT asserted here (deferred to Phase 2).
- **Phase 2:** every species' base art passes the Phase 1 invariants over the REAL
  templates — band membership + S4<S5<S6 monotonicity (`assert_in_stage_band`),
  `assert_s6_fills_art_rows_no_sparkle`, width-1/11-col/8-line; Blob/Ghost carry a
  `▒▓` core (flat-color figure-ground); Glitch resting face is alive (no `x x`),
  closed packet silhouette; mood faces are species-specific and width-correct
  (`mood_face`); `apply_interior_texture` varies per seed on S3+ and is pinned
  byte-identical on S0–S2; the preview lab shows texture variants + the full mood set
  (manifest schema v4); full suite + clippy green.
- **Phase 3:** six mutually-distinct species body hues (peach/mint/lavender/acid/
  ice/amber) that read chromatic, not grey; particles resolve their own hue;
  resting-eye ≥3:1 luminance contrast vs body across a seed sweep, per species;
  `eye_color_for_mood` reads green-at-rest / warm-excited / blue-tired / grey-wilted
  and is applied per-tick in `rerender_pet_for_view_model`; `eyes_are_green_for_every_species`
  is rewritten to "green at rest"; the dead `palette_roles` path is removed;
  flat-color figure-ground holds for Blob/Ghost; full suite + clippy green.
- **Phase 4:** the dead `AnimationProfile.breath_period`/`breath_hold` fields are
  deleted (animator owns breath alone); the `Corruption` role is wired through
  `role_color` / `ResolvedPalette` / `colors.rs` / presentation; corruption fires on
  a calm gate (every 13 ticks) capped at 3 cells, emits `Corruption`-role spans that
  z-win over Eye/Mouth, never recolors the eye-center, and never fires for non-Glitch
  species; `glitch_particles_stay_punctuation_sized` still passes (untouched);
  full suite + clippy green.
- **Phase 5:** the pet anchors feet-relative (`pet_feet_anchor_y`) one row above the
  floor with no panic on degenerate areas; a feet-restricted, bg-only contact shadow
  sits under the pet and never touches side-column gutter identity; `biome_floor_wash_color`
  makes the floor read darker than the sky, both subtle and biome-distinct;
  `TOKEN_PEBBLE_25K` front-loads at 10k lifetime tokens with the maturity gate and
  `flat_and_immature_pets_render_zero_motes` untouched; `HeavySessionShimmer` fires
  once on a fresh `HabitatPropSource::HeavySession` planter (never stale, never
  lifetime-sourced, never asleep) and `DreamGlimmer` stays unemitted; full suite +
  clippy green; preview-lab grounding reviewed.

The preview lab (`cargo run -- dev-preview --scenario pets|watch`) is the visual
regression surface and the human review backstop; it is not an automated gate.

---

## Resolved open-item decisions (binding)

The spec's three "decide during planning" open items are resolved in the plans:

- **Breath amplitude (Phase 4):** keep the binary 0/1 bob + per-species *period*
  only. NO multi-row structural amplitude. Phase 4 only deletes the dead breath
  fields; `species_breath_rhythm_decis` stays the single breath source of truth.
- **Mech S6 gutter content (Phase 1):** `None` — Mech keeps its chassis art rows
  (`gutter_content_for(Mech, S6) == GutterContent::None`). Every other species' S6
  gets `Sparkle`; nothing below S6 gets a gutter sparkle.
- **Crystal eye-fill glyphs (Phase 1/2):** keep `◇`→`◆`→`◈`. The project's
  `ambiguous=narrow` assumption is documented and the wide-mode check
  (`ambiguous_wide_width_warnings`) is a NON-BLOCKING lint (warns, never fails) so
  the signature survives. The blocking width invariant runs under narrow.

Plus the seed-source decision: interior texture reuses the already-drawn
`traits.seed_hue` (cast `u64`); no new `VisibleTraits` field, no `state.json` change.

---

## Pre-flight checklist (run before the overnight session)

- [ ] On the integration branch `glorp-visual-overhaul` (confirmed current).
- [ ] Working tree clean except the intended overhaul docs (no stray tracked-file
      edits). `git status --short` and `git diff --stat` should be empty of `src/`
      changes.
- [ ] Baseline green — substantiated at audit time:
      `cargo build` clean; `cargo clippy --all-targets --all-features -- -D warnings`
      clean; `cargo test --lib` = 610 passed / 0 failed. Re-run `cargo test`
      (full incl. integration) once more immediately before starting and confirm 0
      failures.
- [ ] `cargo fmt --check` clean.
- [ ] **Phase 2 art constants embedded** (the hard gate above): the 42 validated
      `Template` constants are in `art.rs` and pass the Phase 1 invariants, OR the
      art-generation pass is scheduled as Phase 2 Task 0 before Phase 2 starts. If
      neither, Phase 2's species-wiring tasks will block.
- [ ] Each phase branches off `glorp-visual-overhaul` (not a stale `main`) and merges
      back to it; phase-branch names per plan.
- [ ] Helper binaries resolvable for any integration tests that shell out
      (`GLORP_CCUSAGE_BIN` / `GLORP_CCUSAGE_CODEX_BIN` pinned where a test needs
      determinism) — these phases are render-only and mostly avoid the provider path,
      but the full `cargo test` gate exercises it.

---

## Known coordination points (not blockers)

- **`morph_count` test is rewritten twice (Phase 1 Task 6, then Phase 2 Task 1).**
  Intentional layering: Phase 1 relaxes it to `>= 1` to keep its own suite green;
  Phase 2 supersedes with the stricter `== 1` on S0–S2 contract. Safe under
  sequential execution; do not "fix" the apparent duplication.
- **`elder_morph_*` / `glitch_daemon_*` tests are deleted in Phase 1; Phase 2 Task 5
  re-greps to confirm.** If Phase 1 left them, Phase 2 STOPs rather than deleting
  tests itself (no-silent-delete rule).
- **`ResolvedPalette` grows across phases:** Phase 3 adds `particle`, Phase 4 adds
  `corruption`. Struct-literal field order is irrelevant in Rust, so the
  "add after `pattern`" inserts compose cleanly into `body,eye,mouth,accent,pattern,
  particle,corruption`. Phase 4's consumed/produced interface blocks were corrected
  in this audit to show the `particle` field Phase 3 contributes.
- **`mood_face` is named as a Phase 3 input by Phase 2 but Phase 3 measures contrast
  against the resolved palette eye, not `mood_face`.** Phase 2 keeps the resting
  (Content) eye on the per-seed `traits.eyes`; `mood_face` covers only the six
  expressive moods. The contrast floor and "green at rest" assertions key on
  `resolve_pet_palette(...).eye`, which is the correct anchor. No code dependency on
  `mood_face` from Phase 3.

---

## Residual must-note (cannot be auto-fixed pre-art)

- **S0–S2 `{pattern}`-slot per-seed band safety (spec Rendering #3).** The spec wants
  the band check additionally run across the real per-seed `{pattern}`/interior
  values on S0–S2 (the `{pattern}` pool can swing occupancy 0–3 cells, enough to
  cross the narrow small bands). The plans pin the *interior-texture* path to
  constant-occupancy on S0–S2 (`interior_texture_is_pinned_on_small_stages`), but no
  task explicitly asserts the per-seed `{pattern}` slot pool is constant-occupancy on
  S0–S2. This is a Phase 2 art-validation concern that can only be verified once the
  real templates + slot pools exist; add a per-seed S0–S2 band assertion to Phase 2
  Task 9 (or a new tiny test) when the art lands. Low risk (width-1 is already
  guaranteed; small-stage templates may not use `{pattern}` at all), but it is the
  one spec sub-clause not pinned by an explicit test.
