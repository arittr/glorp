# Glorp Visual Overhaul — Design

Status: approved for planning (post staff-review revision) · Date: 2026-06-21

> Revised after a three-reviewer staff-SWE adversarial pass. Code claims in this
> revision are verified against source; file:line references are current as of
> 2026-06-21 and must still be re-checked at implementation time.

## Goal

Make glorp's pet, evolution arc, and habitat feel alive and characterful. Two
concrete complaints drive this:

1. **The glitch pet reads as a broken render**, not an intentional creature.
2. **The whole thing feels boring** — flat-looking color, a static "tank," and
   evolution stages that barely differ (S4/S5/S6 look the same).

This is one design, delivered in **five sequenced, independently-shippable phases**
(see "Delivery phases"). It is not a single monolithic change.

## Non-goals / scope guardrails

- **Evolution ceremony stays modest.** Polish the existing overlay (art + timing);
  no new full-screen cinematic beat.
- **Color is truecolor-first, two tiers only.** `ColorCapability` has exactly
  `Truecolor` and `Flat` (`src/tui/style.rs`). Colored pets render in truecolor and
  fall back to monochrome-by-silhouette under `Flat`/`NO_COLOR`. Sub-truecolor
  (256/16) is delegated to ratatui's automatic downgrade and **not engineered
  here.** Legibility must survive monochrome (carried by silhouette + glyph).
- **Tamagotchi spirit.** Calm over flashy; night calmer than day; nurturing
  companion, not an optimizer. No death — the floor state is `wilted`.
- **Only real signals drive content.** Growth, mood, biome, props, and scene
  moments all trace to real observed token usage and the clock. No fabricated
  richness (the immature-pet "zero-feast" invariant is preserved — see Habitat).
- **The renderer stays content-agnostic.** Species/stage character lives in the
  templates (`art.rs`) and the palette, not in renderer special-casing.

## Migration / continuity

Pet appearance is a pure function of `(seed, species)`, recomputed every render
(`commands/watch.rs` `generate_pet(&state.pet.seed).with_species(species)`); the
derived `morph_index` / `palette_index` / `seed_hue` are **not persisted**
(`storage/state.rs` `PetIdentity` holds only `seed`, `generated_species`,
`accepted_name`). They are drawn in a fixed RNG order in `visible_traits`
(`generation.rs`): `palette_index` → `morph_index` → `morph_pup_index` → `seed_hue`.

**Decision (Drew): a one-time visual reset is accepted.** We are an early project
with few live pets, so we do **not** invest in append-only draw-order discipline to
keep existing pets pixel-identical. On upgrade a pet may change body texture/hue and
(post stage→template rework) its silhouette, once.

Hard constraints that still hold:

- **Identity data is never touched.** `seed`, `accepted_name`, `xp`, vitals, stage,
  calibration baseline, and seen transitions are untouched. This is a rendering
  change, not a state migration. No `state.json` schema change.
- **No crash / no blank pet.** Every persisted `(seed, species, stage)` must resolve
  to a valid template after the template-map rework. A test renders a fixed seed set
  across all species × stages and asserts a non-empty 11×8 result.

## The shared art grammar

Every species obeys one structural grammar — what makes six bespoke concepts read
as a single designed cast:

1. **Closed silhouette** — a sealed shape you could flood-fill in one pass.
2. **Figure-ground** — the body is visibly denser (`▒▓█`) than the sparse dotty
   habitat, so it reads as a solid creature **with color off.** This applies to
   every species **including Blob** (see Blob note); a `░`-only body in a `░` dot
   field is the failure this rule exists to kill.
3. **Growth reads as bigger** — across S0→S6 the creature grows within the fixed
   11×8 canvas per the concrete cell bands below.
4. **Living face** — the resting expression is alive (never `x x` corpse eyes): a
   3-cell `{eyes}` region + 1-cell `{mouth}` region, expressive at rest.
5. **Recognizable species** — one signature survives at every size.

### Growth cell bands (concrete, the audit target)

Occupied-cell count of the **8 art rows** per stage, measured per the invariant
below. Bands are disjoint and strictly increasing:

| Stage | S0 | S1 | S2 | S3 | S4 | S5 | S6 |
|---|---|---|---|---|---|---|---|
| occupied cells | 1–4 | 5–10 | 11–20 | 21–34 | 35–50 | 51–66 | 67–88 |

S6 must fill all 8 art rows (the sparkle no longer steals rows — see Rendering #2).
The upper bands (S5–S6) are reached as **dense, filled masses**, not hollow outlines —
grammar rule #1's "flood-fill in one pass" describes the closed boundary, not a hollow
interior.

## The cast

Working codenames for the concepts; the in-game per-stage labels (`SPECIES_ARCS` in
`docs/tokenpet/project/pet.jsx`) are unchanged.

| Species | Concept | Signature | Notes |
|---|---|---|---|
| Fuzz | **Hearthfloof** — dense plush loaf-cat | ear-cones + mitten-feet + heart-locket | block-mass edges (`▒▓█`), not line-art; hatchling has eyes |
| Blob | **Deep-Light Medusa** — bioluminescent jelly | trailing tendril curtain + glowing core | soft concept, but must still pass flat-color figure-ground (closed `( )` outline + a `▒▓` core, not a `░`-only body) |
| Ghost | **The Pall** — billowing shroud | box-curl crown + scalloped `\_/` hem | growth via density + width |
| Glitch | **Packet Daemon** — self-assembling data process | closed packet-frame + lens eyes + torn data base | the hero; idle = face/base reshuffle |
| Crystal | **The Caged Lumen** — prism caging a core | apex + facet tiers; **eyes fill `◇`→`◆`→`◈` with age** | see the ambiguous-width constraint on `◆/◈` |
| Mech | **Bulwark** — box-draw war-frame | head/torso/legs that bolt on chassis | the legibility benchmark |

## Growth & per-pet variety (algorithmic)

The root cause of "S4/S5/S6 look the same" is the current stage→template map:
S0/S1/S2 → `tiny[0/1/2]`, S3 → `pup[…]`, **S4 → `adult[0]` (ignores morph_index),
S5/S6 → `adult[elder_morph_index]`** — so S4/S5/S6 are variants of one adult pool.

The fix:

- **One hand-drawn base silhouette per species per stage.** 42 base templates
  (6 × 7), each a distinct escalating form landing in its stage's cell band. Each
  stage adds a size beat **and** a structural feature over the previous.
- **Retire `elder_morph_index`** and the shared-adult-pool indexing.
- **Per-pet individuality is algorithmic, not hand-drawn morph pools.** (This
  supersedes the earlier "hand-draw several morph shapes per stage" decision.)
  Variety comes from:
  - **Color** — per-species body hue (retuned, chroma raised) + per-seed hue jitter
    (the existing ±18° in `resolve_pet_palette`).
  - **Slots** — `{eyes}`/`{mouth}` (mood) and `{pattern}`/`{accent}` (per-seed from
    species pools), the existing substitution path.
  - **Interior texture (new)** — a deterministic per-seed variation of the body's
    **non-structural interior cells** (which interior cells render `▒` vs `▓`, accent
    placement), constrained to preserve the closed outline, width-1, and the stage's
    cell band. This is the "more algorithmic, less hand-drawing" direction.
  - **Outline shape is shared** per species/stage. Algorithmic *silhouette* variety
    is **out of scope** — it cannot preserve the closed-silhouette/width/band
    invariants reliably. If wanted later, it is a separate research spike.

### Acceptance bar for growth

A reviewer can sort a species' S0→S6 base templates by age on sight, and S4 < S5 <
S6 in occupied cells. Enforced by the monotonicity invariant (Rendering #3).

## Rendering architecture changes

Verified against source; re-check at implementation time.

1. **Per-stage template constants + new stage→template map** (`src/pet/art.rs`).
   Replace tiny/pup/adult-pool indexing with one base template per stage. Delete
   `elder_morph_index` (`art.rs`). Redefine `morph_count` semantics (it has **zero
   production callers** — only tests): per-pet variety is now algorithmic, so
   `morph_count` either goes away or returns the interior-texture-variant count; the
   tests that assert `morph_count >= 3` are rewritten (see Testing). When S3 collapses
   to a single base template, resolve the `morph_pup_index` / `next_usize(4)` pup draw
   (`generation.rs`) — remove it or mark it retained-dead — and update the
   `visible_traits` draw-order note above; harmless under the accepted reset.

2. **Move the S6 sparkle into the particle gutter, with an explicit per-species
   gutter-precedence rule.** Today the renderer overwrites authored art rows 0 and 7
   at S6 with `SAGE_TOP`/`SAGE_BOT` (`art.rs`), shrinking several S6 forms below
   their S5. After the change all 8 art rows belong to the creature. **But the 13×10
   frame's gutter rows (0 and 9) are already contended** and there is a *third*
   sparkle surface the original spec missed:
   - species particles already paint gutter rows (`render.rs` `particles_for_species`
     — e.g. Crystal's whole identity is gutter cells; Mech's LED; Ghost/Blob/Fuzz
     motes),
   - `frame_fill_for_stage` renders an outer-frame `✦` at S6 (`tui/layout.rs`),
   - the new contact shadow wants the bottom row (Habitat), and `PET_H` is a fixed 10
     (`pet.rs`).

   Required: define a **per-species gutter content model** (sparkle / machine-frame /
   none) so the Mech-S6 choice is data, not an architecture fork; define
   **precedence** when an S6 sparkle, a species particle, and a contact
   shadow target the same cell. **Species identity particles outrank the contact
   shadow** (grammar rule #5 — the signature must survive every size): the contact
   shadow is restricted to the columns **directly beneath the silhouette's feet** (the
   lowest non-blank art row), leaving side-column gutter identity cells (e.g. Crystal's
   facets, Mech's LED) untouched; the S6 sparkle uses **row 0 only** to avoid the
   row-9 shadow. Reconcile with `frame_fill_for_stage` so there are not three
   uncoordinated sparkle treatments. `frame_with_particles` is last-write-wins today; precedence must be
   explicit.

3. **New invariant: rendered-size monotonicity (precise, tick-independent).**
   Count **non-space cells in the 8 art rows only** (exclude the particle gutter and
   any frame substitution), with each `{slot}` replaced by a fixed canonical
   width-correct filler, at a **fixed reference state: mood = Content, resting
   (non-blink) expression, no work accent, fixed tick.** Assert each species' base
   templates land in their stage cell band and are strictly increasing S0→S6
   (S4 < S5 < S6). The "sparkle no longer steals art rows" is asserted **separately**
   as a structural check, not folded into the size count. (Per-pet interior texture
   varies glyph *identity* within structural cells, not occupancy, so it does not
   move a template across a band. The variable-occupancy `{pattern}` slot is the
   exception — its per-seed pool can swing 0–3 cells, enough to cross the narrow
   small-stage bands — so on **S0–S2** the `{pattern}`/interior-texture pool is
   constrained to constant-occupancy glyphs, and the band check is additionally run
   across the real per-seed slot values for those stages, not only the canonical
   base.)

4. **Ambiguous-width glyph invariant.** Eye/accent glyphs must be East-Asian-Width
   **Neutral or Narrow**, not Ambiguous. `◉` (U+25C9) is Neutral (safe); `◇◆◈●○` are
   Ambiguous and render width-2 on `ambiguous=wide` terminals, shattering the grid.
   Either the existing 11×8 width test is **also run under `ambiguous=wide`**
   (rejecting Ambiguous glyphs), or Ambiguous glyphs are banned from templates. The
   Crystal eye-fill `◇`→`◆`→`◈` must comply: if kept, the project's
   `ambiguous=narrow` assumption is documented and the wide-mode test is added; else
   the eye-fill uses Neutral/Narrow glyphs.

5. **Mood-glyph vocabulary is standardized** across species (resting / happy / tired
   / wilted) as width-1 glyphs in the existing `{eyes}`(3) / `{mouth}`(1) slots, wired
   through `expression_for` (`render.rs`). Resting eyes may be species-specific (e.g.
   `◉` daemon) within the 3-cell slot. **The 3-cell `{eyes}` slot is kept**; any
   reference art with wider/split eyes (Blob/Mech/Crystal in Appendix A) is redrawn
   to fit the slot rather than widening it.

## Color & palette system

Corrected diagnosis: species **already have per-species base hues** —
`species_base_hue` (`palette.rs`: Fuzz 70 / Blob 195 / Ghost 300 / Glitch 135 /
Crystal 230 / Mech 250), applied live via `resolve_pet_palette`. They read near-grey
**because body chroma is pinned low (~0.10, `palette.rs`)**, not because hue is
missing. Changes:

- **Raise body chroma** off ~0.10 so the species hue registers; **retune the base
  hues** toward the identity palette below. Keep OKLCH `resolve_pet_palette` and the
  per-seed hue jitter. Re-validate `seed_pet_palette` / `palette_from_styles`
  (`tui/panels/pet/colors.rs`, per-channel `saturating_add`) at higher chroma so a
  high channel doesn't blow out.

  | Species | Body | Accent | Particle | Signature move |
  |---|---|---|---|---|
  | Fuzz · Hearthfloof | peach | rose-amber | warm dust | heart-locket pulses |
  | Blob · Medusa | mint | ice-cyan | cyan motes | core glows brighter with age |
  | Ghost · Pall | lavender | ice | pale wisps | cool pallor; lantern eyes |
  | Glitch · Packet Daemon | acid/phosphor | cyan | acid static | green static; lens scanline |
  | Crystal · Caged Lumen | ice | violet | white sparkle | cold shell, warming core |
  | Mech · Bulwark | amber/brass | red reactor | ember flecks | reactor-core glow |

  These are the `species_feed` hue family (`animator.rs`). Decision: **re-point
  `species_base_hue` to these hues** (keep OKLCH resolution + seed jitter); do **not**
  adopt the flat `species_feed_color` RGBs directly (they have no jitter and would
  kill `per_pet_variety_within_species`).

- **Particles get their own species hue** (today `Particle => palette.accent`,
  undifferentiated).

- **Eyes encode mood, not species — with a real data path.** `resolve_pet_palette`
  is currently mood-blind and the palette is rebuilt only on the ~10s worker poll
  (`commands/watch.rs`), so eye *color* cannot ride mood there without lagging the
  eye *glyph* (which updates on the animation tick via `expression_for`). Required:
  a `mood → eye hue/lightness` mapping applied **at animation-tick cadence**. The
  watch surface already has live mood at its per-tick render site
  (`rerender_pet_for_view_model`, `vm.pet_render.mood` in scope), so the step is added
  there — no new threading; the menubar / round companion / dev_preview surfaces are
  confirmed per-surface during Phase 3 (some may lack live mood), applying the step or
  a post-step in `tui/panels/pet/colors.rs`. Mapping: **green at rest → warm/gold
  excited → cool blue tired → desaturated wilted.** Resolve the dead `palette_roles`
  path (`render.rs`, zero callers) — adopt it as the mechanism or delete it. Eye hue
  is pinned in **two** places (`palette.rs` `EYE_HUE`; `render.rs` `palette_roles`);
  both change. `eyes_are_green_for_every_species` becomes "green **at rest**."

- **Resting-eye contrast floor.** The resting eye must hold **≥3:1 luminance contrast
  against the species body color.** The brand green (`good`, `style.rs`) measured
  against the new bodies is ~1.4–1.8:1 (effectively invisible) on Blob/Fuzz/Glitch.
  Species whose body collides with the green anchor get a per-species resting-eye
  lightness shift or a dark eye outline. Specify the mechanism before Phase 3.

- **Wilting is glyph + desaturation only.** The `wilted` floor state is expressed by
  eye/mouth glyph + a desaturated palette — **not** a new per-stage droop silhouette
  (no per-stage × wilted art explosion). Wilting never reduces a pet's rendered
  occupied size below its stage band, so it never reads as de-evolution.

- **Flat / NO_COLOR.** Honor `NO_COLOR`. Under `Flat`, pets render monochrome and
  legibility is carried by silhouette; pass RGB to ratatui's downgrade otherwise. Add
  a **flat-color figure-ground acceptance check** for the soft-bodied species
  (Blob/Ghost) whose interiors are sparse.

## Liveliness / animation

Corrected: breath **is already per-species** via `compute_breath_offset_with_rhythm`
→ `species_breath_rhythm_decis` (`animator.rs`; test `breath_periods_match_pet_jsx_ordering`).
The `AnimationProfile.breath_period`/`breath_hold` fields (`render.rs`) are dead **and
divergent** (a second table that disagrees with the live rhythm). Changes:

- **Delete the dead `AnimationProfile.breath_period`/`breath_hold` fields** so there
  is one breath source of truth (`animator.rs`).
- **Breath amplitude is the real gap.** The bob is a binary 0/1 row offset — there is
  no amplitude knob without a structural change. **Decide in Phase 4** whether a
  multi-row per-species amplitude is worth that change; do not wire the dead fields.
- **Make glitch corruption a loud, intentional effect.** Today `apply_glitch_corruption`
  (`render.rs`) is body-only (`in_body` guard), pre-framing, from the body's own glyph
  alphabet, one cell / 37 ticks. Required: a new `PaletteRoleName::Corruption` variant
  threaded through `role_color` / the palette / `style.rs` degrade, with **z-order
  winning over the underlying Eye/Mouth span** at a corrupted cell. Bounds: bounded
  rate/footprint; reshuffles base/edge cells and **only briefly** touches the face,
  **never the eye-center** (the living-face rule holds); respects "calm, never
  flashing"; respects `glitch_particles_stay_punctuation_sized` (`render.rs`) — heavier
  glyphs must not trip that assertion (rewrite it deliberately if the new corruption
  needs `▒▓`-weight glyphs).

## The tank / habitat

Corrected starting state: the habitat is **not** an empty void. Code already has a
patchy floor row (`ambient.rs`, test `ambient_glyphs_present_with_floor_row`),
per-biome floor palettes (`biome_floor_palette`), a per-biome background wash
(`biome_wash_color`), and night-sparser dimming. The genuinely-missing,
highest-payoff piece: **the pet is vertically centered, not grounded** —
`pet.rs` `let cy = area.y + area.height.saturating_sub(PET_H) / 2;`.

Direction (Phase 5):

- **Anchor the pet to the floor.** Change the `pet.rs` anchor from centered to
  floor-relative. Define "feet" as the **lowest non-blank art row** of the template
  (templates carry trailing blank rows). This single change kills the "floating in a
  void" read.
- **Contact shadow under the pet**, composited against the existing floor row and the
  silhouette halo (`pet_silhouette_halo_rects`); resolve row-9 contention per the
  gutter-precedence rule (Rendering #2).
- **Sky/ground value separation** — extend the existing `biome_wash_color` for a
  clearer two-tone, rather than introducing a parallel system.
- **Hold** the full terrarium glass frame + perspective floor until validated at the
  real (narrow) pet-column width.

Honest-signal habitat improvements (Phase 5, constrained):

- **Front-load some early character without fabricating a feast.** `mote_glyphs_for`
  returns empty for immature pets (`ambient.rs`, test
  `flat_and_immature_pets_render_zero_motes`) and props gate on real
  `lifetime_effective_tokens` (`habitat.rs`). Permitted: lower a *specific* early prop
  threshold (e.g. the 25k pebble) or add honest Starter-biome **texture variety**. Not
  permitted: lowering the maturity gate or pre-granting props. The immature-pet
  zero-feast invariant is preserved.
- **Activate `HeavySessionShimmer`** on a named real trigger — the heavy-session
  unlock (`recent_effective_tokens >= threshold`, `habitat.rs`), emitted from
  `scene_moments_for` (`room.rs`). **Drop `DreamGlimmer`** from scope — it has no real
  signal, and inventing one violates the only-real-signals rule.

## Delivery phases

Each phase is independently shippable, reviewable, and committable, with the existing
roster still rendering between phases.

1. **Foundation** — per-stage template map + invariants (rendered-size monotonicity,
   ambiguous-width), delete `elder_morph_index`, redefine `morph_count`, the
   gutter-precedence model + S6-sparkle-to-gutter. Existing art is rewired into the
   new map so the roster still renders. *Acceptance:* all invariants pass; no
   unintended visual regression.
2. **Per-species base art** — the 42 S0→S6 base silhouettes + standardized mood faces
   + the algorithmic interior-texture variation. *Acceptance:* growth monotonicity,
   figure-ground (incl. flat-color), glitch reads as intentional, preview-lab review.
3. **Color & eyes-mood** — raise chroma, retune hues, particle hues, the
   mood→eye-color data path, the contrast floor. *Acceptance:* per-species identity
   legible, mood reads in the eye, contrast ≥3:1, flat-color figure-ground holds.
4. **Liveliness** — delete dead breath fields; decide/implement breath amplitude; the
   loud glitch corruption (new role + z-order). *Acceptance:* calm/no-flash;
   corruption reads intentional; living-face preserved.
5. **Habitat grounding** — floor anchor, contact shadow, biome wash extension, honest
   early front-loading, `HeavySessionShimmer`. *Acceptance:* pet grounded; calm;
   real-signals + zero-feast preserved.

## Art production approach

Real surface: **42 base templates** + standardized mood faces (`expression_for`
covers 7 moods + blink + soft-eyes + 3 work-accents — currently shared hardcoded
glyphs; standardizing per-species is the larger sub-task) + the algorithmic
interior-texture variation. This is materially smaller than hand-drawn morph pools.

Produce art with the draw-and-validate pipeline used during design: parallel
subagents draw candidates under the grammar + cell bands + invariants; an audit pass
machine-validates every grid (11×8, width-1 incl. `ambiguous=wide`, cell band,
rendered-size monotonicity); results reviewed in the preview lab.

**Preview-lab requirement:** the pets scenario currently renders only one fixed seed
per species (`dev_preview/pets.rs`), so it cannot backstop per-pet variety. Extend it
to render representative interior-texture variants per adult stage and the full mood
set. This increments the manifest schema (currently v3).

## Testing & acceptance

- **Invariants (`art.rs`):** 11×8 / width-1 (incl. a run under `ambiguous=wide`),
  8-line; rendered-size monotonicity per the precise definition (Rendering #3);
  "S6 fills all 8 rows / sparkle not in art" structural check.
- **Tests to rewrite (never silently delete):** `species_have_enough_seeded_morph_variety`
  (`generation.rs`), `elder_morph_skips_singleton_for_carved_species` and `_for_glitch`
  (`art.rs`), `glitch_daemon_silhouette_is_visibly_denser_than_glitch_form` (`art.rs`),
  `morph_count >= 3` assertions plus the `morph_count(species, S3) == 1` assertion (`tests/generation.rs`), `eyes_are_green_for_every_species`
  → "green at rest" (`palette.rs`). State the new `morph_count` contract.
- **Continuity:** a fixed seed set renders a valid non-empty 11×8 for every
  species × stage after the map rework (no crash / no blank pet).
- **Color:** resting-eye ≥3:1 contrast vs body per species; flat-color figure-ground
  check for Blob/Ghost.
- **Glitch:** resting face alive (no `x x`); closed silhouette; corruption reads
  intentional (contrasting role, z-over Eye/Mouth) and never blanks the eye-center.
- **Preview-lab** is the visual regression surface (extended per above).
- **Pristine test output** and a clean `cargo clippy --all-targets --all-features -D
  warnings` gate; test-only helpers stay `#[cfg(test)]`.

## Open items (decide during planning)

- **Breath amplitude** (Phase 4): multi-row per-species amplitude (structural change)
  vs leave the 0/1 bob and rely on per-species *period* only.
- **Mech S6 gutter content**: sparkle vs machine-frame vs none — now per-species data,
  decided at art time.
- **Crystal eye-fill glyphs**: keep `◇◆◈` (document `ambiguous=narrow` + add the
  wide-mode test) vs swap to Neutral/Narrow glyphs.

## File map (verify at implementation time)

- `src/pet/art.rs` — templates, per-stage map, S6 frame, invariants; delete `elder_morph_index`
- `src/pet/render.rs` — particle frame, role spans, `expression_for`, glitch corruption; delete dead `AnimationProfile` breath fields + dead `palette_roles` (or adopt)
- `src/pet/palette.rs` — `species_base_hue` retune, body chroma, `EYE_HUE` un-pin, mood→eye color
- `src/pet/generation.rs` — eye/mouth trait vocab, mood-glyph sets, `visible_traits` draw order
- `src/pet/animator.rs` — `species_breath_rhythm_decis` (breath site), `species_feed_color` hues
- `src/tui/panels/pet/colors.rs` — live mutation chain (NOTE: this is the real path; there is **no** `src/pet/colors.rs`)
- `src/tui/panels/pet.rs` — pet anchor (`cy`), contact shadow, `PET_H`
- `src/tui/panels/pet/ambient.rs` — floor row, motes, biome floor palettes
- `src/tui/room.rs` — `scene_moments_for`, biome wash
- `src/tui/day.rs` — time-of-day
- `src/game/habitat.rs` — prop thresholds, heavy-session unlock
- `src/tui/layout.rs` — `frame_fill_for_stage` (the third S6 sparkle surface)
- `src/tui/style.rs` — `ColorCapability` (Truecolor/Flat), degrade
- `src/commands/watch.rs`, `src/tui/view_model.rs` — palette build, mood threading
- `src/dev_preview/pets.rs` — preview-lab pets scenario (extend for variants)

---

## Appendix A — candidate reference silhouettes

**Candidate**, not validated-as-templates: drawn 7-stages side-by-side for review.
Before authoring, each is re-extracted into a single-column 11×8 grid and run through
`every_template_line_is_eleven_display_columns` / `_is_eight_lines`, the
`ambiguous=wide` width check, **and the rendered-size cell-band / monotonicity
invariant** (Rendering #3). **Candidate cell counts here are illustrative, not
normative**: on redraw each base template must be re-measured to land in its stage
band (e.g. S5 51–66, S6 67–88) with S4<S5<S6 — in particular the Glitch S5 redraw must
drop into the **S5** band (not the S6 band), and Crystal S5 must clear the 51 floor.
Known redraws: Glitch S5/S6 (pull S5 inset so S6 owns
edge-to-edge); any S6 reserving a `(sparkle → gutter)` row must instead fill all 8
rows; Blob/Mech/Crystal eyes must fit the 3-cell `{eyes}` slot; the Blob body needs a
`▒▓` core for flat-color figure-ground.

### Fuzz — Hearthfloof
```
S0       S1        S2         S3            S4          S5           S6
                                 ▲   ▲        ▟▙   ▟▙    ▟█▙   ▟█▙   ▟██▙   ▟██▙
                   ▲   ▲        ▓▒▒▒▒▓        ▓▒▒▒▒▒▓    ▓▓▒▒▒▒▒▓▓   ▓██▒▒▒▒▒██▓
                  ▓▒▒▒▒▓        ▒o o▒▒       ▓▒o o▒▒▓    ▓▒o o▒▒▒▓   ▓▒█o o█▒▒▒▓
        ▲         ▒o o▒         ▒ w ▒▒       ▓▒ w ▒▒▓    ▓▒ w ▒▒▒▓   ▓▒█ w █▒▒▒▓
       ▒▒▒        ▒ w ▒         ▓▒◇▒▒▓       ▓▒▒◆▒▒▒▓    ▓▒▒◆◆▒▒▒▒▓   ▓▒▒◈◈◈▒▒▒▒▓
 ▒▒   ▒oo▒        ▓▒◌▒▓         ▓▒▒▒▒▓       ▓▒▒▒▒▒▒▓    ▓▒▒◆◆▒▒▒▒▓   ▓▒▒◈◈◈▒▒▒▒▓
▒oo▒  ▒▒▒▒          ▝ ▘          ▙▒ ▒▟        ▙▒▒▒▒▟      ▓▒▒▒▒▒▒▓    ▓██▒▒▒▒▒██▓
 ▀▀    ▀▀                        ▘   ▘        ▘     ▘     ▙▒▘ ▝▒▟    ▝▙█▒▘ ▝█▟▘
```

### Ghost — The Pall
```
S0       S1        S2         S3            S4          S5          S6
                                  ╭─╮          ╭───╮      ╭╯───╰╮    ░▒▓▓▓▓▓▓▓▒░
                    ╭─╮          ╭╯ ╰╮        ░▒▒▒▒▒░    ░▒▓▓▓▓▓▒░   ▒▓███████▓▒
        ▄▄▄        ░▒▒▒░        ░▒▒▒▒▒░       ░▒▒▒▒▒░    ░▒▓o o▓▒░   ▓███o o███▓
 ▗▄▖   ░▒▒▒░       ░o o░        ░▒o o▒░       ░▒o o▒░    ░▒▓ o ▓▒░   ▓███ o ███▓
 ◦ ◦   ░o o░       ░ o ░        ░▒ o ▒░       ░▒ o ▒░    ░▒▓▓▓▓▓▒░   ▒▓███████▓▒
        \_/        ░▒▒▒░        ░▒▒▒▒▒░       ░▒▒▒▒▒░     ░▒▓▓▓▒░    ░▒▓█▓█▓█▓▒░
                   \_/\_        \_/\_/\      ░▒▒▒▒▒▒▒░    ░▒▒▒▒▒░    (fill row 8)
                                             \_/\_/\_/   \_/\_/\_/\
```

### Glitch — Packet Daemon  (S5/S6 to be redrawn so S6 owns edge-to-edge)
```
S0       S1        S2         S3            S4          S5          S6
                              ▄▄▄▄▄▄▄        ▄▄▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄▄▄▄▄  ▛▀▀▀▀▀▀▀▀▀▜
                   ▄▄▄▄▄      ▌◉   ◉▐        ▌ ◉   ◉ ▐  ▌▒◉  ░  ◉▒▐  ▌▓◉▒░ ░▒◉▓▐
 ▄▖     ▄▄▄        ▌◉ ◉▐      ▌  ▀  ▐        ▌  ▀▀▀  ▐  ▌░▄▄▄▄▄▄▄░▐  ▌▒░█▀▀▀█░▒▐
 ◉▌     ▌◉▐        ▌ ▀ ▐      ▌░▄▄▄░▐        ▌ ░▓▓▓░ ▐  ▌▒░ █▀█ ░▒▐  ▌▓▒░▓▓▓░▒▓▐
 ▀      ▙▄▟        ▌▄▄▄▐      ▙▄▄▄▄▄▟        ▌  ░▒░  ▐  ▌▓░ ▓▓▓ ░▓▐  ▙▟▙▟▙▟▙▟▙▟▟
                  ▙▟▙▟▟        ▝▟▙ ▟▙▘       ▙▄▄▄▄▄▄▄▟  ▌░▒ ▒▒▒ ▒░▐  ▝▟▙▟▙▟▙▟▙▟▘
                   ▘ ▖ ▝                    ▝▟▙▟ ▟▙▟▘  ▙▄▄▄▄▄▄▄▄▄▟  (fill row 8)
                                             ▘ ▝ ▘ ▝   ▝▟▙▟▙ ▙▟▙▟▘
```

### Crystal — The Caged Lumen  (eye-fill glyphs subject to ambiguous-width check)
```
S0       S1        S2         S3            S4          S5          S6
                                 /\            /\          /\       /\ /\ /\ /\
                   /\           /◆◆\         /◆◆\        /◈◈\      /▓██▓██▓██\
        /\        /◇◇\         /▒▓▓▒\       /▒▓▓▒\      /▒▓▓▒\     /▒██◈█◈██▒\
 ·     /◇◇\       /▒▓▓▒\       \▒▿▓▒/       /▒▓██▓▒\    /▒▓██▓▒\    \▒███▾███▒/
 ◇◇    \▒▒/       \▒▿▓▒/       /▓██▓\       \▒▓▾█▓▒/    \▒▓▾█▓▒/    \▓███████▓/
  ▿     \/         \▓▓/        \▓▓▓/         \▒▓▓▒/    /\ \▓█▓/ /\  \▒▓█▓█▓█▓▒/
                   \/           \▼/           \▓▓/     ▓▓ \▓▓/  ▓▓  (fill row 8)
                                               \/      \/   ▼   \/
```

### Mech — Bulwark  (S4/S6 eyes redraw to 3-cell {eyes} slot)
```
S0       S1        S2         S3            S4          S5          S6
                                ╷ ╷            ╷╷╷       ╲╷╷╷╱     █▌┌─────┐▐█
                   ╷           ┌───┐          ┌───┐      ┌─────┐   █▌│ ◉ ◉ │▐█
        ┌───┐     ┌───┐        │◉ ◉│         │◉ ◉│      │ ◉ ◉ │    █▌│ ╴═╶ │▐█
        │◉ ◉│     │◉ ◉│        │ ═ │        ┌┴─═─┴┐     │ ╴═╶ │    ██▙▓◆◈◆▓▟██
 ▄      │ ═ │     │ ═ │       ┌┴───┴┐       ║▌▓███▓▐║   ▟█▌▓███▓▐█▙  ██▌▒███▒▐██
▐◉▌     └┬─┬┘     └┬─┬┘       │▒▓▓▓▒│       ║▌▓▒▒▒▓▐║   ▝█▌▓▒◈▒▓▐█▘  ██▙└┬─┬┘▟██
 ▀       ╨ ╨       ╨ ╨        │▒▒▒▒▒│       ╜└┬───┬┘╙    ║▌└┬─┬┘▐║   (fill row 8)
                              ╜   ╙          ██  ██       ▟█▙ ▟█▙
```

### Blob — Deep-Light Medusa  (needs a ▒▓ core for flat-color figure-ground; eyes to 3-cell slot)
```
S4            S5            S6
   .---.        ·˚ ✦ ˚·      ✦ ˚ · ˚ ✦
  /▒▒▒▒▒\       /▒▒▒▒▒\     .▒▓███▓▒.
 (▒▒◉ ◉▒▒)     (▒▒◉ ◉▒▒)    (▓▒◉ ◉▒▓)
 (▒▒▒~▒▒▒)    (▒▒▒▒~▒▒▒▒)   (▓▒▒~▒▒▓)
 (░▒▓◆▓▒░)    (░▒▓◆◉◆▓▒░)   (◆▓◉◆◉▓◆)
  \░▒░▒░/      \░▒░▒░/      \▒░▒░▒░/
   ╎|┊|╎        |┊|╎|┊|     |┊|╎|┊|╎
   '╵'╵'        '╵'╵'╵'      '╵'╵'╵
```

## Appendix B — the glitch fix, stated plainly

The original glitch failed because: (1) resting face used `x x` corpse eyes; (2) the
half-block `▌▐` walls never closed into a silhouette; (3) corruption used the same
glyph alphabet as the ambient field, so it melted into its background; and (4) the
corruption animation was the quietest effect in the file and forbidden from touching
the face. The Packet Daemon concept fixes all four: a closed packet-frame
(silhouette), living lens eyes (`◉`), a contrasting-colored corruption that briefly
touches the face but never the eye-center (intentional + loud + living-face-safe), and
a signature torn-data base unique to the creature (figure-ground).
