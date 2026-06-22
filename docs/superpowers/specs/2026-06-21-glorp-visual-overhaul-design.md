# Glorp Visual Overhaul — Design

Status: approved for planning · Date: 2026-06-21

## Goal

Make glorp's pet, evolution arc, and habitat feel alive and characterful. Two
concrete complaints drive this work:

1. **The glitch pet reads as a broken render**, not an intentional creature.
2. **The whole thing feels boring** — flat color, a static "tank" that is a void
   of confetti, and evolution stages that barely differ.

This is a single cohesive overhaul of the pet art, the growth/evolution arc, the
color system, the liveliness, and the habitat — not a series of isolated tweaks.

## Non-goals / scope guardrails

- **Evolution ceremony stays modest.** Polish the existing overlay (art + timing);
  do not build a new full-screen cinematic beat.
- **Color is truecolor-first.** Best-effort 256/16, basic `NO_COLOR`. Legibility
  must survive monochrome (carried by silhouette + glyph, never color alone), but
  we do not invest in full cross-depth parity.
- **Tamagotchi spirit is preserved.** Calm over flashy; night calmer than day;
  nurturing companion, not an optimizer; no ETAs/countdowns/min-max framing. There
  is no death — the floor state is `wilted` (drooped, desaturated).
- **Only real signals drive content.** Growth, mood, biome, props, and scene
  moments all trace to real observed token usage and the clock. Flavor is allowed
  only when a real signal selects which flavor shows.
- **The renderer stays content-agnostic.** Species/stage character lives in the
  templates (`art.rs`) and the palette, not in renderer special-casing.

## The shared art grammar

Every species obeys one structural grammar, which is what makes six bespoke
concepts read as a single designed cast:

1. **Closed silhouette** — the body is a sealed shape you could flood-fill in one
   pass. No accidental half-open walls (the original glitch's core defect).
2. **Figure-ground** — the body is visibly denser (`░▒▓█` fill) than the sparse
   dotty habitat, so it reads as a solid creature with color off.
3. **Growth reads as bigger** — across S0→S6 the creature visibly grows within the
   fixed 11×8 canvas: a tiny hatchling in negative space → an edge-to-edge,
   densest elder. See "Growth system".
4. **Living face** — the resting expression is alive (never `x x` corpse eyes). A
   3-cell eye region (`{eyes}` slot) + 1-cell mouth (`{mouth}` slot) stays
   expressive at rest.
5. **Recognizable species** — one memorable signature survives at every size
   (ears+locket, tendril-curtain, crown+hem, torn packet, prism apex, head/torso/
   legs).

## The cast

Six species, each pushed to a bold bespoke concept. Names below are working
codenames for the concepts; the in-game per-stage stage labels (`SPECIES_ARCS` in
`docs/tokenpet/project/pet.jsx`: fluff/fuzzling/kit/…) are unchanged.

| Species | Concept | Signature | Notes |
|---|---|---|---|
| Fuzz | **Hearthfloof** — dense plush loaf-cat | ear-cones + mitten-feet + chest heart-locket | edges are block-mass (`▓▒█`), not thin line-art; hatchling has eyes |
| Blob | **Deep-Light Medusa** — translucent bioluminescent jelly | trailing tendril curtain + glowing organ-core | the only soft/see-through silhouette; grows downward via tendrils |
| Ghost | **The Pall** — billowing shroud | box-curl crown + scalloped shedding `\_/` hem | growth via density (`░▒`→`█`) + width, not just footprint |
| Glitch | **Packet Daemon** — self-assembling data process | closed packet-frame + lens eyes + torn data-bleeding base | the locked hero; idle animation = face + base reshuffle |
| Crystal | **The Caged Lumen** — dark prism caging a core | prism apex + facet tiers; **eyes fill `◇`→`◆`→`◈` with age** | growth-in-the-face, the standout idea of the set |
| Mech | **Bulwark** — box-draw war-frame | unmistakable head/torso/legs that bolt on chassis | the legibility benchmark; clearest per-stage upgrade story |

Reference silhouettes (validated 11×8, width-1) are in Appendix A. They are the
base morph (morph 0) per stage and the concrete target for the build; additional
morphs are drawn during implementation.

## Growth system

The root cause of "S4 and S6 look the same" is the current stage→template map:
S0/S1/S2 → `tiny[0/1/2]`, S3 → `pup[…]`, **S4/S5/S6 → the same `adult[elder_morph_index]`
pool**. S4/S5/S6 are morph variants of one body, not escalating sizes.

The fix:

- **Each stage gets its own dedicated template(s).** Seven distinct, escalating
  forms per species. Each stage adds a size beat **and** a new structural feature
  over the previous one.
- **Retire `elder_morph_index`.** It exists only to fake an evolved S5/S6 out of the
  shared adult pool; with per-stage art it becomes dead code to delete.
- **Morphs are kept** (per-pet silhouette variety across seeds). Each stage holds a
  small morph pool: **1–2 morphs at the tiny early stages (S0–S2), 2–3 at the adult
  stages (S4–S6)** where the pet spends most of its life.
- **Size banding keeps growth monotonic regardless of rolled morph.** Each stage
  has a target occupied-cell band; every morph of that stage lands in its band; the
  bands strictly increase across stages. So any pet visibly grows no matter which
  morphs it draws.

### Acceptance bar for growth

A stranger shown a species' S0→S6 must be able to sort them by age on sight, and
**S4 / S5 / S6 must be unmistakably different rendered sizes.** This is enforced by
a new invariant test (see "Rendering architecture").

## Rendering architecture changes

These are the structural changes (discussed and approved per the architecture-
decision norm). File references are current as of this design; verify against the
code at implementation time.

1. **Per-stage template constants + new stage→template map** (`src/pet/art.rs`).
   Replace the tiny/pup/adult-pool indexing with a per-stage pool. Delete
   `elder_morph_index`.

2. **Move the S6 sparkle frame into the particle gutter** (`src/pet/art.rs` S6
   substitution + `src/pet/render.rs` particle frame). Today the renderer overwrites
   authored rows 0 and 7 at S6 with `SAGE_TOP`/`SAGE_BOT`, discarding whatever the
   artist drew there — which silently shrinks several S6 forms below their S5. After
   the change, all 8 art rows belong to the creature, and the sparkle lives in the
   13×10 frame's gutter. Result: every species' signature row survives at its
   pinnacle, and S6 can own the full canvas.

3. **New invariant test: rendered-size monotonicity.** Alongside the existing 11×8 /
   width-1 / 8-line invariants in `art.rs`, add a test that the **rendered** occupied
   size is non-decreasing across S0→S6 for every species and every morph, and that
   S4 < S5 < S6 strictly. "Rendered" accounts for any gutter/frame substitution.

4. **Mood-glyph vocabulary is standardized.** Each species gets a small mood-eye set
   (resting / happy / tired / wilted) as width-1 glyphs, wired through the existing
   `{eyes}`/`{mouth}` slot + role-span path. Resting eyes may be species-specific
   geometric glyphs (e.g. `◉` daemon, `◆` prism) — this is a deliberate expansion of
   the eye trait vocab in `src/pet/generation.rs`, not free; all glyphs are width-1.

## Color & palette system

Today every species is near-grey (body chroma ~0.10) with eyes hard-pinned green
(`EYE_HUE`), so the roster looks samey and mood carries no steady color. Changes in
`src/pet/palette.rs` (+ `colors.rs`, `animator.rs`):

- **Per-species identity palette.** Give each species a permanent body/accent/
  particle hue. Spend the vivid `species_feed` palette (peach / mint / lavender /
  acid / ice / amber) — which today only flashes for ~400ms during feeding — as the
  steady body-hue spine.

  | Species | Body | Accent | Particle | Signature move |
  |---|---|---|---|---|
  | Fuzz · Hearthfloof | peach | rose-amber | warm dust | heart-locket pulses |
  | Blob · Medusa | mint | ice-cyan | cyan motes | organ-core glows brighter with age |
  | Ghost · Pall | lavender | ice | pale wisps | cool pallor; lantern eyes |
  | Glitch · Packet Daemon | acid/phosphor | cyan | acid static | terminal-green static; lens scanline |
  | Crystal · Caged Lumen | ice | violet | white sparkle | cold shell, warming violet core |
  | Mech · Bulwark | amber/brass | red reactor | ember flecks | reactor-core glow at the chest |

- **Particles get their own species hue** (today `Particle => palette.accent`,
  undifferentiated). The halo/sparkle/mist differentiates species at a glance.
- **Raise body chroma** off near-grey so the species hue actually registers.
- **Eyes encode mood, not species.** Un-pin `EYE_HUE`. Eye color shifts with the
  pet's feeling: **green at rest (brand anchor) → warm/gold excited → cool blue
  tired → desaturated wilted.** Species identity rides on silhouette + body color;
  mood rides on the eyes (color) plus the mood-glyph (shape).
- **Truecolor-first degrade** (`src/tui/style.rs`): honor `NO_COLOR`; degrade
  truecolor → 256 → 16 best-effort; never let information depend on a color tier
  (the silhouette carries it).

## Liveliness / animation

The pet interior is essentially static today; the only motion is a blink and a
once-per-37-ticks glitch swap. Changes in `src/pet/render.rs` / `animator.rs`:

- **Wire up per-species breathing.** `species_animation_profile` already authors
  `breath_period`/`breath_hold` per species but they are read nowhere — breath is a
  single identical whole-pet bob. Drive the bob's amplitude/period from the profile
  so Crystal's slow held breath differs from the Packet Daemon's fast shallow one.
- **Make glitch corruption the loudest effect, not the quietest.** Today it fires
  one Body-only cell every 37 ticks from the same glyph family the body is built
  from, and is forbidden from touching the face. Give it a contrasting role-color,
  let it touch the face (the reshuffling expression is the signature idle), and
  raise its rate/footprint so it reads as intentional corruption.
- Keep it calm: animation is texture and gentle motion, never flashing; night is
  calmer than day.

## The tank / habitat

Today the habitat is a uniform confetti of dots over a void — same color sky and
ground, no floor, no horizon, no depth; the pet floats lost in it.

**Direction: grounded habitat (phased).** In `src/tui/panels/pet/ambient.rs`,
`src/tui/room.rs`, `src/tui/day.rs`, `src/tui/panels/pet.rs`:

- **A real ground line the pet stands on** — anchor the pet's feet to the ground
  instead of centering it in negative space. This single change kills the "floating
  in a void" read and is the highest-payoff, lowest-risk move.
- **A contact shadow under the pet** (borrowed from the diorama exploration).
- **A two-tone sky/ground value wash** so air and ground are visually separated.
- **Hold** the full terrarium glass frame + multi-row perspective floor until
  validated at real terminal width — at narrow widths the side rails crowd the
  ~13-wide pet and read busy, the opposite of the calm we want.

Stays calm: static structure, not motion-spam; night strictly sparser than day;
props/biome/weather continue to come from real signals.

Secondary habitat improvements to fold in where cheap (from the diagnosis):
front-load some room character earlier (today the interesting biome/props are gated
behind 750k–25M lifetime tokens, so a typical pet sits in a grey Starter dot-field),
and activate the two scene effects (`HeavySessionShimmer`, `DreamGlimmer`) that
exist in the enum + preview but are never emitted in the watch.

## Art production approach

The bold per-species + per-stage + morph-pool art is a large but bounded surface
(~6 species × 7 stages × 1–3 morphs, plus mood faces). Produce it with the same
draw-and-validate pipeline used during this design: parallel subagents draw
candidate grids under the grammar + invariants, an audit pass machine-validates
every grid (11×8, width-1, escalation band, rendered-size monotonicity), and the
results are reviewed in the preview lab.

**Every change is reviewable in the preview lab.** `glorp dev-preview --scenario all
--out target/glorp-preview` produces deterministic, seeded frames that never touch
real pet state. The acceptance bar from the existing alive-room spec applies: a
reviewer can identify biome, weather, emitter, and pet performance from the cropped
room alone, in flat color.

## Testing & acceptance

- **Invariants (`art.rs` tests):** every template 11 display-columns × 8 lines,
  width-1 glyphs only; plus the new rendered-size monotonicity test (S0≤…≤S6,
  S4<S5<S6) across every species and morph.
- **Growth acceptance:** S4/S5/S6 are unmistakably different rendered sizes for every
  species.
- **Glitch acceptance:** resting face is alive (no `x x`); the body is a closed
  silhouette; corruption reads as intentional (contrasting color, touches the face);
  the creature does not melt into the ambient field (figure-ground holds with color
  off).
- **Preview-lab review:** the full `--scenario all` bundle is the visual regression
  surface; species/stage matrices, the glitch live-states, and the grounded rooms
  are the key frames.
- **Pristine test output** and a clean `cargo clippy --all-targets --all-features -D
  warnings` gate, per project convention (test-only helpers stay `#[cfg(test)]`).

## Open items to confirm during planning

- **Exact morph counts per stage** (proposal: 1–2 early / 2–3 adult).
- **Whether the Mech S6 keeps the sparkle** at all, or a machine-appropriate frame
  (it reads more "machine" than "sage").
- **Phasing/sequencing** of the build (grammar + invariants → per-species arcs →
  palette → liveliness → tank) is decided in the implementation plan.

## File map (verify at implementation time)

- `src/pet/art.rs` — templates, per-stage map, S6 frame, invariants
- `src/pet/render.rs` — particle frame, role spans, glitch corruption, breathing
- `src/pet/palette.rs` — per-species palette, eye-hue/mood, body chroma
- `src/pet/generation.rs` — eye/mouth trait vocab, mood-glyph sets
- `src/pet/animator.rs` — per-species breath profile wiring
- `src/pet/colors.rs` — live mutation chain
- `src/tui/panels/pet.rs`, `src/tui/panels/pet/{ambient,colors,art_lines}.rs` — pet + habitat passes
- `src/tui/room.rs`, `src/tui/day.rs` — room / time-of-day
- `src/game/habitat.rs`, `src/tui/component/habitat_props.rs` — props
- `src/tui/style.rs` — color capability / degrade
- `src/tui/component/watch_screen.rs` — watch layout

---

## Appendix A — validated reference silhouettes

Base morph per stage, validated 11×8 / width-1. The starting target for the build;
refine in the preview lab. (Glitch S5/S6 to be redrawn so S6 owns the full canvas —
see note.)

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
                   \_/\_        \_/\_/\      ░▒▒▒▒▒▒▒░    ░▒▒▒▒▒░    (sparkle → gutter)
                                             \_/\_/\_/   \_/\_/\_/\
```

### Glitch — Packet Daemon  (S5/S6 to be redrawn: pull S5 inset so S6 owns edge-to-edge)
```
S0       S1        S2         S3            S4          S5          S6
                              ▄▄▄▄▄▄▄        ▄▄▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄▄▄▄▄  ▛▀▀▀▀▀▀▀▀▀▜
                   ▄▄▄▄▄      ▌◉   ◉▐        ▌ ◉   ◉ ▐  ▌▒◉  ░  ◉▒▐  ▌▓◉▒░ ░▒◉▓▐
 ▄▖     ▄▄▄        ▌◉ ◉▐      ▌  ▀  ▐        ▌  ▀▀▀  ▐  ▌░▄▄▄▄▄▄▄░▐  ▌▒░█▀▀▀█░▒▐
 ◉▌     ▌◉▐        ▌ ▀ ▐      ▌░▄▄▄░▐        ▌ ░▓▓▓░ ▐  ▌▒░ █▀█ ░▒▐  ▌▓▒░▓▓▓░▒▓▐
 ▀      ▙▄▟        ▌▄▄▄▐      ▙▄▄▄▄▄▟        ▌  ░▒░  ▐  ▌▓░ ▓▓▓ ░▓▐  ▙▟▙▟▙▟▙▟▙▟▟
                  ▙▟▙▟▟        ▝▟▙ ▟▙▘       ▙▄▄▄▄▄▄▄▟  ▌░▒ ▒▒▒ ▒░▐  ▝▟▙▟▙▟▙▟▙▟▘
                   ▘ ▖ ▝                    ▝▟▙▟ ▟▙▟▘  ▙▄▄▄▄▄▄▄▄▄▟  (sparkle → gutter)
                                             ▘ ▝ ▘ ▝   ▝▟▙▟▙ ▙▟▙▟▘
```

### Crystal — The Caged Lumen
```
S0       S1        S2         S3            S4          S5          S6
                                 /\            /\          /\       /\ /\ /\ /\
                   /\           /◆◆\         /◆◆\        /◈◈\      /▓██▓██▓██\
        /\        /◇◇\         /▒▓▓▒\       /▒▓▓▒\      /▒▓▓▒\     /▒██◈█◈██▒\
 ·     /◇◇\       /▒▓▓▒\       \▒▿▓▒/       /▒▓██▓▒\    /▒▓██▓▒\    \▒███▾███▒/
 ◇◇    \▒▒/       \▒▿▓▒/       /▓██▓\       \▒▓▾█▓▒/    \▒▓▾█▓▒/    \▓███████▓/
  ▿     \/         \▓▓/        \▓▓▓/         \▒▓▓▒/    /\ \▓█▓/ /\  \▒▓█▓█▓█▓▒/
                   \/           \▼/           \▓▓/     ▓▓ \▓▓/  ▓▓  (sparkle → gutter)
                                               \/      \/   ▼   \/
```

### Mech — Bulwark
```
S0       S1        S2         S3            S4          S5          S6
                                ╷ ╷            ╷╷╷       ╲╷╷╷╱     █▌┌─────┐▐█
                   ╷           ┌───┐          ┌───┐      ┌─────┐   █▌│·◉ ◉·│▐█
        ┌───┐     ┌───┐        │◉ ◉│         │◉ ◉│      │·◉ ◉·│    █▌│ ╴═╶ │▐█
        │◉ ◉│     │◉ ◉│        │ ═ │        ┌┴─═─┴┐     │ ╴═╶ │    ██▙▓◆◈◆▓▟██
 ▄      │ ═ │     │ ═ │       ┌┴───┴┐       ║▌▓███▓▐║   ▟█▌▓███▓▐█▙  ██▌▒███▒▐██
▐◉▌     └┬─┬┘     └┬─┬┘       │▒▓▓▓▒│       ║▌▓▒▒▒▓▐║   ▝█▌▓▒◈▒▓▐█▘  ██▙└┬─┬┘▟██
 ▀       ╨ ╨       ╨ ╨        │▒▒▒▒▒│       ╜└┬───┬┘╙    ║▌└┬─┬┘▐║   (sparkle → gutter)
                              ╜   ╙          ██  ██       ▟█▙ ▟█▙
```

### Blob — Deep-Light Medusa  (chosen)
```
S4            S5            S6
   .---.        ·˚ ✦ ˚·      ✦ ˚ · ˚ ✦
  /░░░░░\       /░░░░░\     .░▒▓███▓▒░.
 (░░░░░░░)     (░░░░░░░)    (▓░◉▒ ▒◉░▓)
 (░◉░ ░◉░)    (░░◉░ ░◉░░)   (▓▒░░~░░▒▓)
 (░░░~░░░)    (░░░░~░░░░)   (◆●◆◉◆◉◆●◆)
 (░░●◆●░░)    (░░●◆◉◆●░░)   \░▒░▒░▒░▒░/
  \░░░░░/      \░▒░▒░▒/     |┊|╎|┊|╎|┊|
   ╎|┊|╎        |┊|╎|┊|      '╵'╵'╵'╵'
```

## Appendix B — the glitch fix, stated plainly

The original glitch failed because: (1) its resting face used `x x` corpse eyes;
(2) its half-block `▌▐` walls never closed into a silhouette; (3) its corruption
used the same glyph alphabet as the ambient field, so it melted into its own
background; and (4) the actual corruption animation was the quietest effect in the
file and forbidden from touching the face. The Packet Daemon concept fixes all
four: a closed packet-frame (silhouette), living lens eyes (`◉`), a contrasting-
colored corruption that touches the face (intentional, loud), and a signature torn-
data base that is unique to the creature (figure-ground).
