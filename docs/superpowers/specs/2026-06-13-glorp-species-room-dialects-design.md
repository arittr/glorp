# Glorp Species Room Dialects - design

- Date: 2026-06-13
- Status: approved direction by Drew; ready for implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-05-13-watch-visual-polish-design.md`
  - `docs/superpowers/specs/2026-05-14-habitat-props-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-alive-room-design.md`

## Problem

Glorp's pet species are meant to feel like different companions, not just
different labels. A live Glitch pet currently feels too close to a Crystal pet
once the surrounding room is visible.

The current implementation explains why:

- Pet silhouettes and particles differ by species.
- The ambient sky/floor layer has species glyph palettes.
- The larger room and tank layers are mostly species-blind.
- Earned props are intentionally catalog-colored and shared across species.
- Preview Lab compares all species as pet art, but does not compare Glitch and
  Crystal in equivalent rooms.

That means the biggest visual surface in watch mode can overwhelm species
identity. Two pets with similar earned props, day phase, and work weather can
look like different creatures living in almost the same tank.

## Direction

Adopt this rule:

**Props define what the room has earned. Species defines the room's visual
dialect.**

The same earned room should still tell the same history, but the pet's species
changes the physics and texture of that history. A lamp, shard, planter, or
cloud remains recognizable as the earned object. The surrounding marks, floor
rhythm, idle texture, and selected high-visibility prop variants speak the
pet's species language.

## Goals

- Make Glitch and Crystal rooms visibly distinct under identical props, day
  phase, stage, and work weather.
- Preserve the Alive Room principle that earned props are persistent identity.
- Define named room dialects for every species without adding persisted state.
- Add strict Glitch/Crystal acceptance first, plus review metadata for the full
  six-species dialect matrix.
- Keep watch mode calm and readable during normal work.
- Add Preview Lab coverage that would have caught this similarity earlier.

## Non-goals

- No six fully separate room systems.
- No new dependency.
- No new persisted pet state.
- No fake activity, transcript content, model names, command text, file paths,
  quotas, ETAs, or productivity pressure.
- No full rewrite of all species art in this pass.
- No change to the prop unlock economy.

## Current Evidence

The code already has the hooks needed for a small fix:

- `Species` is stable domain state.
- The pet panel already receives species when rendering ambient glyphs.
- `habitat_props_for` and `trophy_sprite` receive species, but the trophy sprite
  path currently ignores it.
- `RoomLifeProfile` does not include species, so `room_glyphs_for` cannot apply
  a species dialect to the Alive Room layer.
- Preview Lab has a pet-species matrix, but watch/props fixtures do not include
  a Glitch room equivalent to the existing Crystal-heavy room fixtures.

The visual overlap has two causes:

- Mature Glitch and Crystal pet art both lean on dense block glyphs as body
  material.
- The shared room/prop layers cover more area than the species ambient layer.

## Design

### Species Dialect

Add a compact derived species dialect for room rendering. It can be an enum or
small value object keyed from `Species`; no new stored data is needed.

The dialect controls:

- ambient glyph families beyond the existing sky/floor palettes
- room glyph zone bias and floor rhythm
- a small color/tint bias for room texture, not for prop identity
- selected high-visibility prop shape variants in the trophy and accent paths
- species-specific preview metadata

The dialect must not replace room biome. The biome still comes from earned
props. The dialect modifies how the biome is expressed.

### Glitch Dialect

Glitch should read as a broken interface, not a faceted object.

Visual language:

- asymmetric lanes and sheared rows
- horizontal scanlines
- sparse block corruption
- cursor blips and packet trails
- bracket and terminal fragments
- broken floor cells

Useful glyph families:

- `0`, `1`, `#`, `:`, `;`, `+`, `=`
- `[`, `]`, `_`, `-`, `|`, `/`, `\`
- `▌`, `▐`, `░`, `▒`, `▪`

Avoid filling the mature Glitch body with the same dense block material that
Crystal uses as facet shading. Blocks should feel like corruption passing
through the creature or room, not mineral mass.

### Crystal Dialect

Crystal should read as clean mineral geometry and refracted light.

Visual language:

- calm symmetry
- vertical facet clusters
- diamond and shard marks
- prism sparkles
- geode floor texture
- constellation-like upper air

Useful glyph families:

- `◇`, `◆`, `◊`, `✦`, `✧`, `*`, `·`
- `/`, `\`, `|`, `^`, `v`
- sparse `░`, `▒` only as facet shade, not noise

Crystal can keep more vertical structure than Glitch. Its room should feel
polished and geological rather than broken or terminal-like.

### Other Species

This pass should define all six dialects at the API level. The strongest
visual tuning and strict visual acceptance focus on Glitch and Crystal first.
The other species may start with stable defaults, but they must still be named
and reviewable in Preview Lab metadata.

Initial dialect sketches:

| Species | Dialect |
|---|---|
| Fuzz | soft tufts, crumbs, small warm dust, rounded floor texture |
| Blob | puddles, bubbles, soft waves, low blobby floor rhythm |
| Ghost | wisps, gaps, quiet vertical fades, sparse haunted air |
| Glitch | scanlines, cursor fragments, packet trails, broken lanes |
| Crystal | facets, shards, prism specks, geode floor |
| Mech | rails, rivets, vents, small mechanical ticks |

If implementation time is tight, add explicit Glitch and Crystal behavior first
and map the other species to conservative existing behavior with named stable
defaults and `dialect_status: "default"`.

### Room Integration

Extend the room profile so species dialect is a first-class semantic choice.
This keeps the Alive Room profile as the single compact description of room
identity instead of threading an extra species flag through renderer calls.

The simplest shape is:

```rust
pub enum RoomDialectStatus {
    Tuned,
    Default,
}

pub struct RoomSpeciesDialect {
    pub species: Species,
    pub key: RoomDialectKey,
    pub status: RoomDialectStatus,
}

pub struct RoomLifeProfile {
    pub biome: RoomBiome,
    pub room_weather: RoomWeatherLayer,
    pub resonant_emitter: Option<PropEmitter>,
    pub pet_performance: PetPerformance,
    pub scene_moments: Vec<SceneMoment>,
    pub identity_prop_ids: Vec<HabitatPropId>,
    pub species_dialect: RoomSpeciesDialect,
}
```

Exact names can change, but ownership should not: `derive_room_life_profile`
derives the dialect from `vm.pet_render.generated_species`, and
`room_glyphs_for` consumes the resulting profile. Species dialect must not
affect prop unlocks, prop weights, biome weighting, emitter selection, or scene
moment triggers.

The renderer applies the profile after biome selection:

1. Biome chooses the durable room identity from props.
2. Day phase and work weather choose the current state texture.
3. Species dialect changes glyph families, zone bias, and texture rhythm.
4. Prop emitters and scene moments remain tied to real earned props/events.

This keeps the conceptual split clear and avoids making species a hidden prop
weight.

### Ambient Integration

Keep the existing species sky/floor palettes, but strengthen them with dialect
profiles:

- Glitch gets horizontal scanline and broken-floor bias.
- Crystal gets shard clusters and cleaner upper-air sparkle.
- Dusk/night variants should not collapse all non-Glitch species into the same
  generic star/dot palette.

Ambient changes should stay sparse enough that pet art and props remain
readable.

### Prop Integration

Use species-specific prop variants sparingly and keep earned-prop identity
intact.

Hard invariants:

- no prop unlock, id, kind, display priority, source, target id, anchor zone, or
  base object class changes by species
- no catalog color changes by species
- no footprint class changes large enough to change the prop's room role
- a shard remains a shard, a lamp remains a lamp, an orbit remains an orbit

Candidate high-impact props:

- `codex_signal_lamp` is a Trophy prop and can use species-specific
  `trophy_sprite` variants. Glitch can shear it toward a terminal beacon;
  Crystal can keep a cleaner prism-lamp silhouette.
- `token_shard_1m`, `token_orbit_5m`, and `token_lantern_10m` are Accent props,
  so species handling belongs in the accent glyph/placement path, not
  `trophy_sprite`. Glitch can add dropped pixels, bracket breaks, or cursor
  flicker; Crystal can keep cleaner shard/orbit/lantern forms.

Catalog colors should remain recognizable. Species variants can change glyph
shape and nearby texture, but should not make earned objects unrecognizable.

### Pet Art Tuning

The room fix may not be enough if mature Glitch and Crystal bodies still share
too much material.

Tune mature Glitch art toward:

- asymmetry
- gaps and offsets
- terminal/cursor fragments
- fewer solid filled facets

Keep mature Crystal art as:

- centered
- faceted
- angular
- bright and clean

This can be a small template edit, not a full pet renderer rewrite.

## Preview Lab Contract

Add deterministic fixtures that compare species under identical room inputs.

Strict Glitch/Crystal pair:

- `watch-species-dialect-glitch`
- `watch-species-dialect-crystal`
- `watch-species-dialect-glitch-flat`
- `watch-species-dialect-crystal-flat`

All strict fixture variants should share:

- `comparison_group: "species-dialect-glitch-crystal"`
- `stage: S6`
- the exact same mood, energy, `DayContext`, `PetLifeProfile`, and activity
  profile
- the exact same earned prop ids, including `codex_signal_lamp`,
  `token_shard_1m`, `token_orbit_5m`, and `token_lantern_10m`
- `day_phase: Day`
- `work_weather: OutputSparks`
- `terminal_width: 120` and `terminal_height: 32`
- matching color capability within each pair: Truecolor for the primary pair,
  Flat for the flat pair

Full matrix:

- Add truecolor room frames for Fuzz, Blob, Ghost, Glitch, Crystal, and Mech
  under the same non-species inputs.
- The Glitch and Crystal frames carry `dialect_status: "tuned"`.
- Other species may carry `dialect_status: "default"` until tuned.

Manifest contract:

- Add a `scenario_metadata` branch for `watch-species-dialect-*` ids.
- Each scenario records `species`, `room_dialect`, `dialect_status`,
  `comparison_group`, shared input invariants, expected changed zones, and prop
  identity invariants.
- Each scenario exports `frames/<id>.room.txt`, `frames/<id>.layout.json`, and
  manifest `files.room_text`.
- Strict dialect scenarios also export `frames/<id>.room-masked.txt` and
  manifest `files.room_masked_text`. The masked crop replaces pet art, speech,
  and shared prop target cells with spaces so human review sees the same surface
  used for automated dialect comparison.
- Review prompts should ask reviewers to inspect paired `.room.txt` crops before
  full frames, then inspect the paired `.room-masked.txt` crops for the dialect
  acceptance surface. The review question is species distinctiveness under the
  same earned room, not prop unlock behavior.

Acceptance:

- With identical non-species inputs, compare `watch.room.effect` after masking
  `watch.pet.art`, speech, and the union of matching prop target rects across the
  comparison pair. Prop targets match by target id or prop id, not by exact rect,
  so small species-specific shape/bounds differences cannot be counted as room
  dialect differences.
- Glitch and Crystal must differ by non-color glyph symbols in at least two room
  zones, including one floor or anchor zone and one upper-air or pet-adjacent
  zone.
- The truecolor and flat Glitch/Crystal pairs must both pass symbol-based
  comparison; changed foreground colors alone do not count.
- In the truecolor pair, all listed shared earned props remain recognizable in
  both rooms. In the flat pair, any props rendered under flat color capability
  remain recognizable.
- Shared prop ids, target ids, and base prop silhouettes remain stable.
- Glitch does not read as a dark crystal; Crystal does not read as clean glitch.

## Testing

Focused checks:

```bash
cargo test --test dev_preview
cargo test dev_preview::scenarios
cargo test dev_preview::habitat_props
cargo test dev_preview::export
cargo test --lib room_glyphs
cargo test --test tui_render
```

Preview review:

```bash
cargo run -- dev-preview --scenario watch --out target/glorp-preview
open target/glorp-preview/index.html
```

If prop variants are changed, include a unit test proving species-specific
shape variants are available for the intended Trophy and Accent paths while
shared prop identity remains stable.

If room glyph generation changes, include tests that identical profiles with
Glitch and Crystal species produce different glyph families or zone allocations.

Preview tests should also assert:

- `watch-species-dialect-*` ids have non-empty metadata and review prompts
- the strict Glitch/Crystal pair shares every non-species input
- room comparisons mask pet art, speech, and shared prop targets
- all six species appear in the matrix with `room_dialect` and `dialect_status`

## Risks

- Too much dialect can weaken earned-prop identity. Keep props recognizable.
- Too little dialect leaves the original problem intact. The preview pair is the
  guardrail.
- Color-only differences will fail in flat/low-color terminals. Glyph and zone
  differences must carry the design.
- Glitch can become noisy. Use sparse corruption and readable scanlines instead
  of filling the whole room with static.
- Crystal can become generic sparkle. Use facet/geode structure, not only stars.
- Preview scenario ids and snapshots are intentionally guarded. Expected churn
  is limited to new fixture ids and deliberate species-dialect artifacts; review
  any snapshot changes with `cargo insta review`.
- Non-Glitch/Crystal outputs should remain unchanged unless explicitly tuned.

## Implementation Shape

1. Add Preview Lab metadata/tests for the strict Glitch/Crystal pair and the
   six-species matrix first.
2. Add species dialect data/helpers and `RoomLifeProfile.species_dialect`.
3. Apply Glitch and Crystal room/ambient dialects.
4. Add prop shape variants in the correct Trophy or Accent render paths while
   preserving prop identity invariants.
5. Tune mature Glitch pet art if masked room previews pass but full frames still
   show Crystal overlap.
6. Run focused tests and review the Preview Lab bundle.
