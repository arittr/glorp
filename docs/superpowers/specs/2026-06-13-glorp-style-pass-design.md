# Glorp Style Pass — Coordinated Visual Identity

## Problem

The pet species read as too similar to each other, and only the Crystal
species feels genuinely good. The environment types read as too similar. The
macOS companion looks the same across nearly every state. We want a coordinated
style pass that gives each species a distinct, characterful identity and makes
each environment feel like a place — across the watch TUI *and* the macOS
companion, as one visual language.

### Verified root causes

- **The per-pet color engine is built but dead.** `palette_roles`
  (`src/pet/render.rs:161`) has zero callers; `seed_hue` / `saturation_percent`
  are consumed only inside that dead function. Every pet renders through the
  fixed `semantic_styles()` theme via `pet_role_style`, so every species has the
  same cream body and the **same bold-green eyes** — the single worst
  homogenizer. Color was disconnected during the terminal port.
- **The previews hide color.** `PreviewCell` already captures per-cell `fg`/`bg`
  including truecolor `Rgb` (`src/dev_preview/frame.rs`), and `index.html`
  renders it. The scouts saw "everything is monochrome" only because they read
  the `.txt` frames, which strip color. **Review surface is the HTML, not the
  `.txt`.**
- **The soft trio share a silhouette.** Fuzz, Blob, and Ghost are fundamentally
  the same `░▒`-shaded vertical lozenge. Color alone cannot separate them; that
  is genuine art surgery.
- **The biome paints ~16 faint dots over a void** and never touches the two
  highest-bandwidth surfaces: the floor band (keyed to *species*, the wrong
  axis) and the background (which does not exist). Four of six species share one
  "Default" dialect symbol table.
- **The room painter is duplicated three ways.** The model layer is shared
  (`derive_round_scene_model` → `derive_room_life_profile`; pet from
  `vm.pet_art`/`vm.pet_spans`), but room *painting* is reimplemented in the watch
  (`tui/room.rs::room_glyphs_for`, rich), the companion preview
  (`round/preview.rs`, a dumb `(x+y) % 5` lattice that ignores pet color spans),
  and the companion live path (`companion/render.rs` + `app.rs`, where
  `RoomGlyph` is an explicit no-op).

## Goals

- A coherent identity for each species across five visual channels.
- Environments that read as distinct places, on the watch and the companion.
- A single shared room-glyph generator so environment work lands on every
  surface at once and the companion stops drifting.

## Non-goals

- Rewriting the Crystal species (it works — polish only).
- Full companion weather motifs (phase 2).
- Any change to cost, XP, food, mood, or stage logic. Visual only.
- Porting back to `pet.jsx` (see Convention note).

## Principles

- **Pigment vs. light.** Species hue is the creature's intrinsic pigment; biome
  and day-phase are the ambient light of the place. They are orthogonal and
  harmonized through a shared palette engine, so they never fight.
- **Value-led and muted.** The `░▒▓█` value structure stays the form-giver (it
  is what makes Crystal read). Hue rides on top as a tint, not a repaint.
  Restrained body chroma; reserve the vivid end for eyes, accents, particles.
  Keeps the coffee/cream aesthetic and stays legible on weaker terminals.
- **Only real signals drive content.** Species, seed, earned biome, day-phase,
  and weather are all real observed signals — no random flavor.
- **Tamagotchi spirit.** Nurturing companion framing; no min-max, ETA, or
  countdown language anywhere in the visuals.
- **Smallest reasonable change.** Surgery where the design is broken (soft
  trio); polish where it works (Crystal/Mech/Glitch).

## Color model

Three layers, each with a distinct job:

1. **Species base hue** (dominant): each species owns a region of the wheel —
   Fuzz amber, Blob teal, Ghost pale violet, Glitch acid-green/magenta, Crystal
   ice-blue, Mech steel. This is what fixes "they look alike." Crystal and Mech
   sit near each other in hue but split hard on chroma and silhouette.
2. **Per-pet jitter**: reuse `seed_hue` / `saturation_percent` as a *small delta*
   off the species base, not an absolute. Your buddy is unmistakably a Fuzz, but
   a slightly-its-own Fuzz.
3. **Biome + day-phase ambient**: a tint / warmth / lightness shift applied on
   top of both pet and room. A Crystal at dusk in a celestial room is cooler and
   dimmer than the same Crystal at midday; its pigment is constant, the lighting
   changes.

Wiring: route pet roles through the (now-live) per-pet palette instead of the
fixed `semantic_styles()` theme for pet roles, then apply the biome/day-phase
ambient on top. Provide a named/indexed fallback palette for non-truecolor
terminals. Review every change in color via `index.html`.

## Per-species identity (five channels each)

Each species gets: **Silhouette · Shading · Hue · Eyes · Particles.**

- **Crystal — benchmark; polish only, no silhouette change.** Diamond/kite
  silhouette (keep). Directional facet light `▓█▒` — *this is the shading model
  we propagate to the others.* Cool ice-blue. Gem-glint eyes `< >` / `◇ ◇`.
  5-sparkle border ring (the gold standard).
- **Fuzz — surgery: commit to the cat.** Triangular haunched cat-sit; ears
  always; **tail from S3, not S5** (today S3/S4 read as "blob with ears" — the
  tail arrives too late). Shading: vertical `░▒` column alternation as fur
  grain. Warm amber. Soft `o o`/`^ ^` eyes; keep the `='w'=` whisker mouth.
  Particles: drifting fur motes on the sides (today: one rare tail-flick).
- **Blob — surgery: make it melt.** Asymmetric gooey body: off-center cap,
  drip-tongues of *different* lengths left vs. right, a sag on one side. Morphs
  deform instead of garnishing the same capsule. Keep round `( )` walls (Blob
  owns "round"). Shading: light `░` cap → dark `▒▓` belly + a `°` specular dot.
  Teal/aqua. Big round `o o`/`O O` eyes. Drip/bubble trail particles.
- **Ghost — surgery: cloth, not a capsule.** Drop the rectangular `|█...█|`
  pipe-walls (they read like the Mech/Glitch frame). Billowing sheet: tapering
  top, scalloped `‿‿`/`⌇` hem, fade-to-nothing bottom. Shading: vertical fade
  `█→▒→░→ ` (incorporeal). Pale desaturated violet. **Hollow/empty socket eyes.**
  Particles: an always-visible drifting `~` wisp (a ghost should never look
  static).
- **Mech — polish + articulate.** Keep the box-drawing chassis, but show
  shoulders/legs **even at S3** (today the S3 pup is a plain box, nearly
  identical to Glitch's). Shading: beveled plating `█▒░` with chrome `═`
  highlights. Steel/gunmetal (low chroma) + one warm `◉` indicator. Optical
  sensor eyes `[ ]` / `= =`. Particles: steam/exhaust column + the existing
  blinking `●○` LED.
- **Glitch — polish: structural brokenness.** Make misalignment *the
  silhouette*, not an occasional 1-cell swap. The adult m0 frame
  (`▌▀▀▀ ▐` offset half-cells, torn edge) is its best look; the **S3 pup
  currently throws that away for a clean box — fix that.** Shading: torn `▌▐`
  half-cells + `▒▓` gradient + the existing live corruption (unique — keep).
  Acid-green/magenta (the one species allowed to clash). Digital eyes
  `0 0` / `x x` / `# #`. Particles: scan-line + `▒░▓` noise (keep).

## Environment model

Biome stays orthogonal to species (earned from props), with palettes designed to
sit in the same family via the shared engine. Four moves:

1. **Floor band → biome-keyed** (today species-keyed — the wrong axis, and it is
   the single most legible environment element). Botanical = moss/grass,
   Artifact = pebbles `o ◦ °`, Technical = traces `─ ┄ ╌`, Celestial = horizon
   glow, Cozy = soft, Starter = sparse.
2. **Subtle background wash** — a very low-contrast per-biome fill behind the
   habitat so the room reads as a place even in a screenshot. Kept whisper-quiet
   so the pet and panels stay dominant.
3. **Split the four "Default" dialects** (Fuzz/Blob/Ghost/Mech), which share one
   symbol table today. Each gets its own family (Ghost misty/sparse, Mech
   gridded box-drawing, Blob rounded `o ° ∘`).
4. **Real night.** Drop density hard, cool the palette, turn the upper-air zone
   into a true starfield. The same biome becomes two environments across the day.

Weather-as-motif (mist = floor drift band, sparks = upward flecks above the pet,
reasoning = concentric rings near the pet) is **phase 2** — the symbols and
colors already exist.

## Shared room-glyph generator

The reuse seam: glyph **selection** (which symbols, which colors, how dense, what
day-phase tint, per biome + dialect + weather) is canvas-independent; **placement**
(rectangular zone rects vs. circular aperture scatter) is canvas-specific.

- Extract selection into a sink-agnostic producer in `tui/room.rs` that yields
  positioned-or-weighted styled glyphs independent of the output sink.
- Consumers, each doing its own placement: the watch buffer painter (existing),
  the companion preview (`round/preview.rs` — also make it **honor pet color
  spans**, which it currently ignores), and the companion live path
  (`companion/render.rs` + `app.rs` — **implement the `RoomGlyph` arm** and honor
  pet spans; the text-drawing primitives `draw_label` / `draw_pet_art_block`
  already exist).
- Refactor **test-first**: a golden test asserts the watch room output is
  byte-identical for unchanged inputs before and after the extraction.

## Constraints and invariants

- Art templates are exactly **11 visible chars wide, 8 lines** (enforced by
  `every_template_line_is_eleven_cells_wide` after slot fill). Eyes slot stays 3
  visible chars; mouth 1; pattern 3; accent 1.
- Particles overwrite grid cells, so they must live in the 13×10 border ring plus
  a few sacrificial interior cells.
- Provide non-truecolor fallback colors (named/indexed) everywhere color is
  introduced.
- The renderer stays content-agnostic: adding species/stage variation is template
  and palette data, not renderer changes.

## Build order (designed coordinated, shipped in increments)

1. **Color engine** — wire `palette_roles`; species base hue + per-pet jitter +
   biome/day-phase ambient; fallback palette. Cheapest, biggest win, unlocks the
   rest. Review via `index.html`.
2. **Pet identity** — directional shading + species eye geometry + particle
   signatures on all six; silhouette surgery on the soft trio (Fuzz/Blob/Ghost).
3. **Shared room-glyph generator** — extract selection from placement,
   test-first; watch output byte-identical for unchanged inputs.
4. **Environment content** — biome-keyed floor band + subtle background wash +
   dialect split + real night, built on the shared generator so it lands on both
   the watch and the companion at once.
5. **Companion** — implement live `RoomGlyph`, honor pet spans in both companion
   painters, biome background tint, day-phase darkening.
6. **Phase 2 (deferred)** — weather-as-motif on the shared generator.

## Testing strategy

- Keep the existing art width/height invariant tests.
- Color: determinism by species + seed; ambient tint application; fallback
  palette produces valid non-truecolor output.
- Preview snapshots (`cells.json`) for representative species × stage × biome,
  reviewed **in color** via `index.html`.
- Shared generator: golden test asserting byte-identical watch room output for
  unchanged inputs across the refactor.
- Companion: `RoomGlyph` commands are emitted; both companion painters honor pet
  spans; frames differ across asleep/active/biome states (no longer
  byte-identical).

## Risks and mitigations

- **Touching `room.rs` (the watch works well today).** Mitigate with the
  byte-identical golden test before changing behavior.
- **Over-saturation breaking the muted aesthetic.** Hold the value-led
  discipline; keep body chroma restrained; pop only on details.
- **Color on weak terminals.** Named/indexed fallback palette.
- **Soft-trio silhouette surgery is real craft.** Iterate visually via
  `index.html`; the diamond/cloth/melt targets are concrete.

## Convention note (needs a decision during review)

`CLAUDE.md` states `docs/tokenpet/project/pet.jsx` is the source of truth for
templates and says to "port from there, don't invent." The shipping `art.rs` has
**already diverged** into a filled-block redesign that `pet.jsx` (thin line-art)
does not contain, and this style pass invents new silhouettes not present in
`pet.jsx`. We should pick one and update `CLAUDE.md` accordingly: either treat
`art.rs` as the de-facto source of truth for the filled art (recommended — do not
port back), or mirror the new templates back into `pet.jsx`.
