# Glorp Alive Room — design

- Date: 2026-06-11
- Status: approved direction by Drew; spec pending review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-06-04-glorp-liveliness-design.md`
  - `docs/superpowers/specs/2026-06-05-glorp-liveliness-branch2-design.md`
  - `docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md`
  - `docs/superpowers/specs/2026-05-12-glorp-preview-lab-animation-strips-design.md`

## Problem

The T2+T3 "lives in time" branch added meaningful state: day phase, tiredness,
morning-after flavor, weather/climate, weekend softening, prop resonance, and
preview fixtures. But the rendered preview still reads as nearly the same room
across scenarios. The differences are real in the data contract and subtle in
the cells: small glyph swaps, light tint changes, and sparse ambient texture.

That is not enough for Glorp's product goal. Drew wants the pet room to be
cute, alive, and personal. A user should be able to glance at the room and feel
that this is *their* pet in *their* room on *this* kind of day.

## Direction

Adopt an **Alive Room** visual language:

- Props define the room's persistent identity.
- Day phase and work weather animate inside that identity.
- The pet performs the emotional state with a small, readable pose vocabulary.
- `tachyonfx` punctuates meaningful moments instead of carrying the whole look.
- Preview Lab proves both resting states and motion strips.

The design target is: **the room should look different at rest, then feel alive
in motion.**

## Goals

- Make preview states visibly distinct from the pet room alone, without reading
  the manifest or right-side panels.
- Make earned props important. Props should give each room and pet a unique
  vibe, not just render as static trophies.
- Use `tachyonfx` beyond the current hatch/stage/mood/feed effects, especially
  for room and prop reactions.
- Keep the watch screen readable during daily use. More alive does not mean
  constant noise.
- Keep new behavior derived from existing local data: earned props,
  `DayContext`, `PetLifeProfile`, species/stage, and the current view model.
- Extend Preview Lab so animated effects can be reviewed deterministically.

## Non-goals

- No new persisted pet-state schema in this pass.
- No new dependency; use the existing Ratatui + `tachyonfx` stack.
- No transcript, prompt, response, model-name, command, file-path, or provider
  payload content in visual contracts.
- No ETAs, streaks, quotas, or productivity pressure.
- No full pet-template rewrite. Add a small performance vocabulary; do not
  redesign all species art.
- No menubar redesign. Menubar should keep consuming shared view-model state,
  but this pass focuses on the watch room.

## Design Principles

### Cute, Alive, Readable

The room may be playful and magical, but it should stay calm enough to leave
open while working. Panels remain utilitarian and readable. The pet room is
where liveliness belongs.

### Props Are Identity

Props are the persistent personality layer. Day/weather/pet state can modify
the room, but they should not erase what the room has become through earned
objects.

### State Changes Need Silhouette

The previous pass failed because it mostly changed color or isolated glyphs.
This pass should change the room's silhouette: where texture lives, which zones
are active, how props participate, where the pet sits emotionally, and which
motion is visible.

### Motion Is Punctuation

`tachyonfx` should make meaningful events feel alive: a feed sweep, dawn wipe,
prop resonance ripple, or heavy-session shimmer. It should not be the only way
to tell states apart, because motion disappears in static screenshots and can
become tiring in live watch mode.

### Real Signals, No Fake Activity

The system may present real signals poetically, but it must not invent work or
emotional context. If source/weather/shape detail is absent, the room degrades
to a simpler biome instead of fabricating specificity.

## System Overview

Add one derived presentation layer:

```rust
pub struct RoomLifeProfile {
    pub biome: RoomBiome,
    pub room_weather: RoomWeatherLayer,
    pub resonant_emitter: Option<PropEmitter>,
    pub pet_performance: PetPerformance,
    pub scene_moments: Vec<SceneMoment>,
}
```

Exact type names can change during implementation, but the boundary should
remain: build a compact, derived room profile once per view-model build or poll
update, then render from that profile. Avoid threading many independent ad hoc
flags through the panel.

Inputs:

- `HabitatView.earned_props`
- `DayContext`
- `PetLifeProfile`
- species, stage, mood, energy, and existing pet render data
- current render clock for deterministic phase/motion

Outputs:

- resting biome glyph families, colors, densities, and zones
- one local prop emitter, usually the current resonant prop
- pet posture/performance hints
- short effect moments for `tachyonfx`

No semantic state is persisted. If any transient animation memory is needed, it
lives in the watch app/animator, like the current `PetAnimator`.

## Prop Biomes

Each catalog prop contributes one or more biome tags. The strongest one or two
tags form the room's base identity.

Initial tag model:

| Tag | Props |
|---|---|
| `Botanical` | moss tuft, hanging vine, heavy-session planter, wilt-recovery sprout |
| `Technical` | Codex signal lamp, orbit, shard |
| `Celestial` | spark, friendly cloud, orbit, lantern |
| `Artifact` | shell, shard, treasure chest, pebble |
| `Cozy` | lantern, moss, shell, planter |

Composition rules:

- Visible earned props contribute tag weight using display priority, recency,
  and whether the prop is currently resonant.
- Initial weight formula, expressed as named constants in code:
  - base visible prop weight: `1.0`
  - display priority weight: `display_priority / 100.0`
  - recent earned bonus: `0.4` when earned within the last 7 local days
  - resonant bonus: `1.2` for the selected resonant prop
- The highest-weight tag is the primary biome. The second tag flavors the room
  when it is at least `60%` of the primary tag's weight. Below that threshold,
  the room renders as a single-tag biome.
- The renderer should expose named biome outputs, not raw weights:
  `Botanical`, `Technical`, `Celestial`, `Artifact`, `Cozy`, and common blends
  such as `BotanicalTechnical` or `CelestialArtifact`.
- If no prop identity is strong enough, use a simple starter room.

This gives durable identity without making every visible prop emit noise at
once.

## Prop Emitters

In addition to the global biome, one prop gets local behavior.

Emitter selection:

1. Current `DayContext` resonant prop, if present and visible.
2. A live `PetLifeProfile` prop reaction, if present and visible.
3. The highest-priority visible prop whose biome tag matches the current room
   weather.
4. None, for starter/flat/low-signal states.

Emitter examples:

| Prop | Resting behavior | Moment behavior |
|---|---|---|
| Heavy-session planter | leaf drift, soft growth around floor-right | bloom/ripple toward pet after a heavy day |
| Codex signal lamp | small technical ping/glow, right-side attention anchor | directional sweep or ping on Codex-heavy activity |
| Wilt-recovery sprout | tiny hopeful growth, low floor texture | soft green shimmer after recovery |
| Lantern | warm halo, calmer night/cozy rooms | dawn/wake light wipe |
| Cloud | drifting soft glyphs in upper room | mist or clearing sweep |
| Orbit | slow arc glyphs in air-right | pulse/ring on burst |
| Shell/shard/chest | glints, artifact floor texture | short sparkle/coalesce when resonant |

Emitter output must include target rect/cells so both renderer overlays and
`tachyonfx` moments can act on the same prop.

## Weather And Day Overlays

Existing work weather and day phase should become bolder and spatially
distinct:

- `CacheMist`: low, rolling floor/ground mist; denser near botanical/artifact
  rooms; calmer and lower at night.
- `OutputSparks`: high and mid-air sparks; stronger near celestial/technical
  rooms; avoids hiding the pet.
- `ReasoningPulse`: slow pulse lines or rings; strongest near technical and
  artifact rooms.
- `Mixed`: layered but restrained combination of two channels.
- `Clear`: room biome remains visible; no fake weather.

Day phase changes composition, not just color:

- Dawn: light enters from one side; waking room wipe is eligible.
- Day: broader ambient field and clearer prop colors.
- Dusk: lower contrast, warmer edges, tired/performance cues more eligible.
- Night: fewer glyphs, stronger cozy/lantern/cloud behavior, sleep/dream cues.
- Weekend softening: less frantic motion and softer colors unless live activity
  is currently present.

The pet room should remain readable in flat/low-color mode by changing glyph
families and placement, not relying only on RGB differences.

## Pet Performance

Add a small performance vocabulary that can be applied across existing species
templates without rewriting every pet:

- Rested/awake: current baseline.
- Tired-but-awake: slower breath, fewer blinks, slight lower posture, softer
  eyes where species art supports it.
- Heavy-day cozy: pet feels satisfied and a little worn out; not sad.
- Dreaming/asleep: closed/dream eyes, sparse dream bubble, quieter room.
- Catch-up wake: gentle wake/eat reaction after nighttime backfill.
- Source burst/perk: brief alertness or glance when live work arrives.

Implementation should prefer small overlays and eye/posture substitutions over
large new templates. If a species cannot support a particular eye shape, it
uses motion/speech instead.

## Tachyonfx Moments

Current `PetAnimator` already owns a `tachyonfx::EffectManager` and applies
effects over the pet effect rectangle. This pass should expand the concept from
"pet-only transition effects" to "scene moments."

Add named effect targets:

- `watch.pet.effect` — existing pet art/effect target.
- `watch.room.effect` — full habitat room interior, excluding panels.
- `watch.prop.<prop-id>.effect` — bounds/cells for visible prop emitters.

The exact path shape can change if the component layout has a better naming
pattern, but the manifest/layout artifacts must expose stable target ids.

Moment examples:

- Feed sweep: current feed pulse continues, but can also sweep through a
  matching prop target when source/weather details justify it.
- Prop resonance ripple: a short ripple from resonant prop toward pet.
- Dawn/wake wipe: a gentle side-to-side light wipe over the room.
- Heavy-session shimmer: room-level shimmer that settles into the biome.
- Dream glimmer: very sparse, low-frequency night effect.

Rules:

- Moments are short, targeted, and meaningful.
- No effect should loop forever through `tachyonfx`; continuous identity stays
  in the renderer.
- Live burst effects remain freshness-gated so cold starts/backfills do not
  masquerade as real-time work.
- The watch loop may use the existing fast tick while effects are active, but
  resting biome motion must not pin the UI at 60fps forever.

Architecturally, use one scene-level animator owned by `WatchApp`. The simplest
implementation is to extend/rename the current `PetAnimator` into a
`SceneAnimator` that still handles existing hatch/stage/mood/feed effects but
can process multiple named target rects. Do not add a parallel effect manager;
one owner should arbitrate effect keys, active-effect timing, and the fast-tick
decision.

## Preview Lab Proof

The Preview Lab is the review contract for this work. Static screenshots alone
are not enough.

Extend `dev-preview` with:

- still frames for representative room biomes:
  - starter room
  - botanical cozy room
  - technical signal room
  - celestial/artifact room
  - mixed advanced room
- still frames for state overlays:
  - cache-mist heavy evening
  - output-sparks active day
  - quiet weekend midday
  - night asleep/dream
  - dawn wake
- animation strips for scene moments:
  - prop resonance ripple
  - feed sweep through prop + pet
  - dawn/wake room wipe
  - heavy-session shimmer

Reuse the existing animation-strip design:

- strips live under `strips/<strip-id>/`
- strip metadata is first-class in `manifest.json`
- HTML playback starts paused with frame-by-frame controls
- `review.md` includes prompts explaining what each strip proves

The success bar: a reviewer should be able to identify the scenario family by
looking at the room area alone.

## Testing

Use test-first implementation for each behavior slice.

Unit tests:

- biome tag weighting chooses stable primary/secondary tags from earned props
- starter/no-prop room degrades cleanly
- resonant prop selection prefers real resonance and visible props
- weather overlays produce different glyph families/placement for each weather
- weekend/night/calm gates reduce motion without deleting identity
- flat/low-color mode still changes glyph placement/family
- pet performance selection respects sleep/tiredness/freshness precedence
- scene moment selection is freshness-gated and does not fire for cold starts

Render/preview tests:

- affected static preview snapshots update deliberately
- new still frames exist and have truthful manifest inputs
- strip artifacts are written with fixed dimensions and manifest entries
- layout artifacts expose room/prop/pet effect targets
- animation-strip HTML references local assets only

Integration tests:

- `WatchApp` uses the fast tick only while finite scene effects are active
- prop effect targets line up with actual visible prop bounds
- live feed effects can target pet plus matching prop without panics
- no real user config/state is read during Preview Lab generation

Manual visual checks:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

Focused checks should include any new scenario selectors introduced for room
biomes or animation strips.

## Rollout Shape

This can be implemented as one coherent branch, but it should still be sliced
internally:

1. Add `RoomLifeProfile` and biome derivation with tests.
2. Render resting biome + bolder weather/day overlays.
3. Add prop emitters and prop effect targets.
4. Add pet performance hints.
5. Add scene/tachyonfx moments.
6. Extend Preview Lab stills and animation strips.
7. Tune against the generated contact sheet and live `cargo run -- watch`.

Each slice should leave the preview usable. Avoid landing a large invisible data
contract without visual proof.

## Success Criteria

- At least five preview still states are visually distinguishable from the room
  area alone.
- At least three props have clearly different local emitter behaviors.
- At least three scene moments have deterministic animation strips.
- A heavy/cache-mist day, output-sparks live day, quiet weekend, night dream,
  and dawn wake all read differently.
- The pet remains the focal point; effects do not obscure panels or pet art.
- The implementation adds no persisted semantic state and no new dependency.
