# Glorp Style Pass — Coordinated Visual Identity

## Problem

The pet species read as too similar to each other, and only the Crystal
species feels genuinely good. The environment types read as too similar. The
macOS companion looks the same across nearly every state. We want a coordinated
style pass that gives each species a distinct, characterful identity and makes
each environment feel like a place — across the watch TUI, the macOS menubar
popover, and the macOS companion, as one visual language.

### Verified root causes

These were verified against the code during a three-reviewer design review.

- **Color is off, and there is no engine to "turn on."** The per-pet palette
  `palette_roles` (`src/pet/render.rs:161`) has zero production callers (only a
  test). More importantly, **no OKLCH/HSL→RGB conversion exists anywhere in
  `src/`** — `PaletteRole` is a struct of lightness/chroma/hue floats with no
  output stage. Every pet renders through the fixed `semantic_styles()` theme.
  So "wire color" is really "build a color resolver from scratch."
- **There are four independent pet role→color sites, not one.** Watch
  (`tui/panels/pet.rs:1658`), the **macOS menubar popover**
  (`menubar/render.rs:67`, its own `COLOR_*` constants), companion-live
  (`companion/app.rs:450`, already colors per-cell via the dead theme), and
  companion-preview (`round/preview.rs:108`, hardcodes cream and ignores spans).
  Color must converge through one resolver or surfaces will visibly disagree.
- **The "review in color" tool is partly broken** for exactly what we need to
  review. `dev_preview/export.rs:444` silently drops named/indexed colors
  (`is_hex_color` filter), so the non-truecolor fallback is reviewed blind; and
  `dev_preview/pets.rs:37` discards `color_capability` and renders the dead theme,
  so the pet matrix won't use the new palette until rewired.
- **The soft trio still share a lozenge — but Fuzz already escaped it.** Fuzz is
  already a chunky cat with `/\_/\` ears and tail morphs (`art.rs:148`). The
  genuine lozenge-sharers in shipping `art.rs` are **Blob** (round `░▒` capsule)
  and **Ghost** (`|█...█|` pipe-walls that read like the Mech/Glitch frame).
- **The grid invariant is enforced by code-point count, not display width**
  (`art.rs:743` `chars().count()`), and **height is not asserted at all**. The
  art uses East-Asian-ambiguous glyphs (`° — █ ▒ ▓ ◇ ┌ ·` …) that can render
  double-wide and shear the 11-cell row on some terminals. Latent today; the
  silhouette surgery widens the exposure.
- **The environment step is over-claimed.** Night already exists
  (`room.rs:540` `phase_density_scale` Night=0.5, `phase_warmth_tint` Night,
  `pet.rs:293` `sky_palette_for_phase` Night starfield) — it is too *subtle*, not
  absent. Floor glyph *symbols* are already biome×dialect (`room.rs:592`); the
  species-keyed element is the solid floor *row* in `floor_palette_for`
  (`pet.rs:90`), which lives in a **different painter** (`ambient_glyphs_for_phase`)
  than the room generator we plan to extract.
- **The room painter is duplicated.** The model layer is shared
  (`derive_round_scene_model` → `derive_room_life_profile`; pet from
  `vm.pet_art`/`vm.pet_spans`), but room *painting* is reimplemented in the watch
  (`tui/room.rs::room_glyphs_for`, rich), the companion preview
  (`round/preview.rs`, a `(x+y) % 5` lattice), and the companion live path
  (`companion/app.rs:411`, where `RoomGlyph` is an explicit no-op).

## Goals

- A coherent, **vibrant** color identity for each species and each pet, rendered
  consistently across all four pet-color surfaces through one resolver.
- A distinct, characterful silhouette for each species.
- Environments that read as distinct places (gated — see Sequencing).
- A color-review pipeline that actually shows what ships, including the fallback.

## Non-goals

- Rewriting the Crystal species (it works — polish only).
- Removing the green eyes. They are a deliberate, liked signature and stay.
- A "muted / restrained" aesthetic. Pets should be colorful.
- Full companion weather motifs (deferred).
- Any change to cost, XP, food, mood, or stage logic. Visual only.

## Principles

- **Many colors, per pet, leaning by species.** Every pet is vibrantly
  multi-colored (the multi-role palette gives body/mouth/accent/pattern/particle
  their own hues) and unique, but each species tilts toward a palette family so a
  Fuzz and a Blob also read different by color, and two Fuzzes read related.
- **Green eyes are the constant signature.** The eye role is pinned green across
  every species; per-pet/species color varies the other roles around it.
- **Pigment vs. light.** Species/pet color is the creature's pigment; biome and
  day-phase are the ambient light of the place, applied as a tint on top of both
  pet and room so a scene reads as one lighting environment.
- **Shading gives form; color rides on it.** The `░▒▓█` value structure stays the
  form-giver (it is what makes Crystal read as solid), but it now carries real,
  unsuppressed color — value for shape, hue for identity.
- **Only real signals drive content.** Species, seed, earned biome, day-phase,
  weather — all real. No random flavor.
- **Smallest reasonable change.** Surgery where the design is broken (Blob,
  Ghost); polish where it works (Crystal, Mech, Glitch, Fuzz).

## Color model

### What gets built (not wired)

There is no color-space conversion in the tree, so step one builds one:
`OKLCH → oklab → linear sRGB → gamut-map → gamma → Color::Rgb`, with a
hand-authored named/indexed fallback for non-truecolor terminals. **Gamut
mapping reduces chroma at fixed lightness/hue** until in-gamut — never a
per-channel clamp, which would shift hue exactly on the vivid eyes/accents that
carry identity. Compose **in OKLCH** (apply species lean, per-pet jitter, and
ambient as L/C/H deltas) and convert to sRGB **once** at the end; do not tint an
already-sRGB color (that double-applies gamma and muddies hue).

### The three layers

1. **Species lean** — each species biases the palette toward a hue family.
2. **Per-pet jitter** — `seed_hue` / `saturation_percent` perturb within that
   family, so each pet is its own. The eye role is exempt (stays green).
3. **Biome + day-phase ambient** — an L/C/H tint applied on top of both pet and
   room, so the scene shares one light.

### Where it is computed and consumed

`ResolvedPalette` is computed **once** per frame at `build_watch_view_model`
(`commands/watch.rs`) / `derive_round_scene_model` (`round/model.rs`) and carried
on the VM/scene next to `pet_spans`, then memoized per `(pet, ambient)` so it is
not recomputed per cell per frame. A single
`role_color(role, &ResolvedPalette) -> Color` is the only role→color function;
all four sites route through it.

### Step 0: converge the four sites first

Before per-pet color, extract `role_color` and route the watch panel, the
menubar popover, the companion-live painter, and the companion-preview painter
through it, producing **byte-identical** output (still the current theme). This
turns turning-on-color into a one-place data change instead of a four-site
surgery, and is independently testable.

## Per-species identity (five channels each)

Each species gets: **Silhouette · Shading · Hue family · Eyes · Particles.**
Eyes stay green for all; the hue family colors the rest.

- **Crystal — benchmark; polish only.** Diamond/kite silhouette (keep).
  Directional facet light `▓█▒` — the shading model we propagate. Cool ice-blue
  family. Gem-glint eyes `< >`. 5-sparkle border ring (the gold standard).
- **Blob — surgery: make it melt.** Asymmetric gooey body: off-center cap,
  drip-tongues of *different* lengths left vs. right, a sag on one side. Morphs
  deform instead of garnishing the same capsule. Keep round `( )` walls. Shading:
  light `░` cap → dark `▒▓` belly + a `°` specular dot. Teal/aqua family. Big
  round eyes. Drip/bubble trail particles.
- **Ghost — surgery: cloth, not a capsule.** Drop the rectangular `|█...█|`
  pipe-walls. Billowing sheet: tapering top, scalloped `‿‿`/`⌇` hem,
  fade-to-nothing bottom (still space-padded to 11). Shading: vertical fade
  `█→▒→░→ `. Pale violet family. Particles: an always-visible drifting `~` wisp.
- **Fuzz — light: it is already a cat.** Keep the silhouette; just bring the
  **tail forward to S3** (it currently appears only at S5+) and add vertical
  `░▒` column shading for fur grain. Warm amber family. Keep the `='w'=` whisker
  mouth. Drifting fur-mote particles.
- **Mech — polish + articulate.** Keep the box-drawing chassis, but show
  shoulders/legs **even at S3**. Beveled plating `█▒░` with chrome `═`. Steel/
  gunmetal family + one warm `◉` indicator. Optical sensor eyes `[ ]`. Steam/
  exhaust particles + the blinking `●○` LED.
- **Glitch — polish: structural brokenness.** Make misalignment the silhouette;
  the adult m0 torn-edge frame is the standard, and the S3 pup currently throws
  it away for a clean box — fix that. Torn `▌▐` half-cells + `▒▓` gradient + the
  existing live corruption. Acid-green/magenta family (allowed to clash). Digital
  eyes `0 0`. Scan-line + `▒░▓` noise particles.

## Environment model (Tranche 2 — gated)

Re-grounded against the code. Four moves:

1. **Strengthen the existing night.** Night infra exists but is too subtle. Push
   density lower, cool the palette more, and let the upper-air zone read as a real
   starfield — extend `phase_density_scale` / `phase_warmth_tint` /
   `sky_palette_for_phase`, do not re-implement.
2. **Biome-key the floor row.** Re-key `floor_palette_for` (`pet.rs:90`) from
   species to biome. Note: this is a **separate painter** from the room
   generator, so it does **not** reach the companion for free — either fold the
   floor into the shared generator as a Floor-zone concept, or scope the floor
   re-key watch-only and document it.
3. **Add a subtle per-biome background wash.** Genuinely new. Cells carry a single
   `bg`, and pet glyphs set only `fg`, so the wash must be a **base-layer bg pass**
   written before the glyph `fg` passes. Keep it whisper-quiet.
4. **Split the four "Default" dialects** (Fuzz/Blob/Ghost/Mech share one symbol
   table today). Each gets its own family.

Weather-as-motif (mist drift band, upward sparks, reasoning rings) is deferred.

## Shared room-glyph generator (Tranche 2 — gated)

The reuse seam: glyph **selection** (which symbols, colors, density, day-phase
tint, per biome+dialect+weather) is canvas-independent; **placement** (rectangular
zone rects vs. circular aperture scatter) is canvas-specific. `RoomZone` is
already the right shared vocabulary.

- The producer emits **position-free** `RoomGlyphSpec { zone: RoomZone,
  glyph: char, style: Style }` plus a budget — never row/col. The watch places
  via `zone_rect`; the aperture maps `RoomZone` → polar band and reject-samples
  inside `aperture.contains()`.
- Consumers: watch (existing), companion preview (`round/preview.rs` — also make
  it honor pet spans, which it ignores today), companion live
  (`companion/app.rs` — implement the `RoomGlyph` arm; the pet itself already
  honors spans).
- **Extract in two commits:** (1) a private selection fn behind the existing
  placement, asserted byte-identical; (2) then sink-agnostic placement.
  `room_glyphs_for` couples selection/placement/dedup/budget/exclusion/tint in
  one pass (`room.rs:500-536`), so the golden test must pin a fixed `now` and the
  full glyph vector across all six dialects, and the refactor must preserve the
  `Pcg32` seed stream and zone iteration order exactly.

## Tooling: review in color

Fixed as part of Tranche 1, before any color content lands, or color is reviewed
blind:

- Map `Color::Indexed` and named ANSI to representative hex in the HTML exporter
  (or resolve to hex at frame build), so `export.rs` stops dropping the fallback.
- Route `dev_preview/pets.rs` through the live per-pet palette honoring
  `color_capability`.
- Add a **preview scenario matrix** crossing representative species × biome ×
  day-phase, plus a flat-mode scenario, so the species-lean and ambient layers
  are actually visible.

## Constraints and invariants

- Art templates are **11 visible columns × 8 lines**. Add a **display-width**
  invariant test (`unicode-width`, asserting 11 columns per filled line) and a
  **`height == 8`** assertion, covering all species/stage/morph and the real
  runtime slot glyphs (not ASCII substitutes), before any silhouette surgery.
  Adopt a policy: art uses width-1 glyphs only (forbid East-Asian-ambiguous), or
  document a narrow-ambiguous assumption.
- Slot widths: eyes 3, mouth 1, pattern 3, accent 1. The green eye signature and
  closed-blink/mood eye overrides must stay 3 visible chars.
- Particles overwrite grid cells, so they live in the 13×10 border ring plus a
  few sacrificial interior cells.
- The background wash is a base-layer `bg` pass; define its order (wash → room
  glyphs → pet art).
- The renderer stays content-agnostic: species/stage variation is template and
  palette data, not renderer changes.

## Build order (ship-and-gate)

Designed coordinated; built and shipped in two validated tranches.

### Tranche 1 — Pets (ship, then look together)

0. **pet.jsx convention** — first commit: declare `art.rs` canonical for
   templates, update `CLAUDE.md`.
1. **Color resolver convergence** — build the OKLCH→sRGB resolver + Flat
   fallback; extract `role_color` and route all four sites through it
   byte-identical; fix the color-review pipeline + add the preview matrix.
2. **Turn on color** — species lean + per-pet jitter, computed once on the
   VM/scene, eyes pinned green.
3. **Pet identity** — display-width/height invariant tests first; then directional
   shading + particle signatures on all six; silhouette surgery on Blob and Ghost;
   Fuzz tail-from-S3 + fur grain.

**Gate:** review T1 in color via `index.html`. Decide whether T2 is warranted.

### Tranche 2 — Environments + companion (gated)

4. **Shared room-glyph generator** — extract selection from placement, two
   commits, byte-identical golden.
5. **Environment content** — strengthen night, biome-key the floor row, subtle
   background wash, dialect split.
6. **Companion** — implement live `RoomGlyph`, colorize the companion-preview pet
   by span, biome background, day-phase darkening.

Deferred: weather-as-motif.

## Definition of done

- A reviewer can name all six species blind from a single watch screenshot.
- Two random seeds of the same species read as the same species (species lean
  holds) while looking individually distinct (per-pet variety holds).
- The same pet renders consistently across the watch, the menubar popover, and
  the companion.
- The fallback (non-truecolor) palette keeps the six species distinguishable.

## Testing strategy

- **Grid:** display-width (11 columns) + `height == 8` invariants across all
  species/stage/morph and real slot glyphs.
- **Color:** resolver produces in-gamut sRGB; gamut mapping preserves requested
  hue within tolerance for high-chroma roles; palette is deterministic by
  `(species, seed, ambient)`; `role_color` convergence is byte-identical at
  step 0; Flat fallback keeps species distinguishable.
- **Room generator:** golden test pinning a fixed `now` and the full glyph vector
  across all six dialects, asserting byte-identical watch output across the
  extraction.
- **Companion:** `RoomGlyph` commands are emitted; the companion-preview pet
  colors by role (eye vs. body `fg` differ); frames differ across
  asleep/active/biome states (the companion has no byte-identical baseline, so
  this positive test is its safety net).

## Risks and mitigations

- **Color is the highest-uncertainty step, not the cheapest** (resolver + gamut +
  gamma + fallback + four-site convergence). Mitigate with step 0 (byte-identical
  convergence) and prototype-then-review the resolver across the preview matrix
  before locking hues.
- **EAW-ambiguous glyphs can shear the grid.** Mitigate with the display-width
  test and the width-1-only policy before surgery.
- **Naive gamut clamp shifts identity hues.** Mitigate with chroma-reduction
  gamut mapping + a hue-preservation test.
- **Building on an over-claimed environment diagnosis risks regressing working
  code.** Mitigate by re-grounding T2 against current code with before/after
  `index.html` comparisons; the generator extraction is a real architecture
  change (rect/zone vs. aperture), gated behind T1.
- **`tachyonfx` mood-drift (`hsl_shift`, runtime) was tuned against the cream
  body** and may misbehave against vibrant species hues. Re-check and rescale
  per species once color is live.

## Convention note — decided

`CLAUDE.md` says `pet.jsx` is the source of truth and "port from there, don't
invent." Verified that `art.rs` has already diverged: filled-block art vs.
pet.jsx's thin line-art, different slot syntax, more adult morphs, and `pet.jsx`
is read at neither build nor runtime. **Decision:** `art.rs` is canonical for
templates and silhouettes; do not port back. `pet.jsx` remains the reference only
for `SPECIES_ARCS` labels, `SPECIES_ANIM` profiles, and `EYES_BY_MOOD` overrides.
Update `CLAUDE.md` accordingly as the first commit.
