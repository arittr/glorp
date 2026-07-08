# Glorp Ambient Tank Life - design

- Date: 2026-07-07
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-05-14-habitat-props-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-daily-aliveness-design.md`
  - `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`
  - `docs/superpowers/specs/2026-06-24-glorp-companion-tank-redesign-design.md`
  - `docs/superpowers/specs/2026-07-07-glorp-companion-perimeter-gauges-design.md`

## Problem

The companion tank now has a strong visual identity: a dark round aquarium, the
pet as the hero, HUD text in the lower gap, and thick perimeter gauges. It also
has second-to-second motion from pet drift and HUD updates. What it lacks is
slow, day-scale change inside the tank.

The current habitat prop system helps the space accumulate visible history, but
it is mostly object-focused and token/milestone driven. Drew wants the fun layer
to feel more like a tiny ecosystem: fish, shrimp, anemones, and other residents
that swim, hide, creep, and pass in front of or behind the pet. The important
product distinction is:

- Tokens grow and animate the pet.
- Calendar age fills the tank.
- Activity changes how awake the tank feels, not which inhabitants are earned.

## Direction

Add **Ambient Tank Life**: a time-earned inhabitant pool plus a deterministic
daily visible cast.

The pet's calendar age unlocks an expanding catalog of tiny tank inhabitants.
Each local day, the renderer selects a stable random subset from the unlocked
pool. The selected inhabitants move through explicit habitat routes instead of
hovering in place. Some lanes render behind the pet and props, and some pass in
front of them. HUD and perimeter gauges stay readable above tank life.

This gives the tank a different "today" without adding streaks, check-ins,
token-based rewards, or a customization UI.

## Product Rules

1. **Age earns inhabitants.** Unlocks come from pet age, measured from
   `PetState.created_at`. Lifetime tokens, today tokens, and rate momentum do
   not unlock inhabitants.
2. **The collection persists; the cast rotates.** Earned inhabitants remain in
   state. The visible cast is derived from pet seed, local date, unlocked ids,
   and render surface.
3. **Daily means stable for the day.** A local-day cast does not re-randomize
   per frame or per app restart. Tomorrow may look different.
4. **Routes over effects.** Inhabitants are residents with movement paths:
   cross-tank, floor, glass/rim, substrate, and local host orbit. They are not
   sparks generated on props.
5. **Depth is part of the feature.** Every inhabitant declares a natural layer
   behavior: background, mid, foreground, or route-dependent. Route-dependent
   inhabitants can pass behind the pet in one segment and in front in another.
6. **Activity livens, but does not earn.** Live activity, burst level, or day
   weather may increase speed, pause frequency, color intensity, or awake time.
   It never changes the earned pool.
7. **The pet remains the hero.** Cast size is capped, glyphs stay tiny, and the
   HUD stays readable.

## Catalog V1

The first catalog intentionally uses a small set of distinct silhouettes and
movement grammars. More inhabitants can be added later without changing the
data model.

| id | Unlock age | Glyph family | Route | Natural layer |
|---|---:|---|---|---|
| `needlefish` | day 3 | `‹·` | cross-tank swim | route-dependent |
| `glass_shrimp` | day 5 | `,〃` / `,≈` | floor hops and pauses | foreground floor |
| `glass_snail` | day 7 | `◔` | glass-wall creep | foreground edge |
| `burrower` | day 10 | `▴` | substrate peek/hide | foreground floor |
| `rim_skimmer` | day 14 | `◜` | perimeter loop | route-dependent |
| `sand_ray` | day 21 | `▱` | bottom glide | foreground floor |
| `schoollet` | day 28 | `‹ ‹` cluster | grouped cross-tank pass | route-dependent |
| `anemone_host` | day 35 | anchor + `›·` fish | local orbit around anchor | anchor behind, fish route-dependent |

The exact days are v1 defaults. They are deliberately front-loaded enough to make
a young tank feel alive, then slow into weekly beats. Existing pets receive all
inhabitants whose age threshold they already satisfy on the next reconciliation.

### Anemone Host Morphs

`anemone_host` is one unlock, not four separate residents. Each day that it is
visible, the renderer picks one anchor morph:

| morph | Anchor glyph | Role |
|---|---|---|
| `flower` | `✺` over `╰╯` | pretty reef day |
| `comb` | `╵╷╵` over `╰┬╯` | clearest tentacle read |
| `crown` | `⌁⌁` over `╰╮/╱╲` | most creature-like |
| `dot_colony` | `⁙⁙` over `╰╯` | coral colony read |

The host fish uses the same local route for all morphs. Keeping the behavior
constant makes the morphs read as one family while still giving daily variety.

## Daily Cast Selection

The visible cast is deterministic and bounded.

Inputs:

- pet seed
- local date
- render surface id, for example `watch`, `round`, or `preview`
- unlocked inhabitant ids
- target habitat size

Rules:

1. If no inhabitants are unlocked, render none.
2. If one or two are unlocked, render all of them.
3. From day 7 through day 20, target 2 or 3 visible inhabitants.
4. From day 21 through day 59, target 3 or 4 visible inhabitants.
5. From day 60 onward, target 4 or 5 visible inhabitants.
6. Never render more than 5 moving inhabitant slots on one surface.
7. `anemone_host` counts as one slot even though it draws an anchor plus a host
   fish.
8. If a selected inhabitant cannot fit safely on a surface, skip it and try the
   next deterministic candidate.

The selection should use a small pure helper, for example:

```rust
pub fn visible_inhabitant_ids(
    unlocked: &[EarnedTankInhabitantView],
    pet_seed: &str,
    local_date: time::Date,
    surface: TankLifeSurface,
    habitat_size: Rect,
) -> Vec<TankInhabitantId>
```

No current-cast field is persisted. Recomputing the cast from the same inputs
must return the same result.

## State Model

Extend `HabitatState` with time-earned inhabitant facts:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HabitatState {
    pub earned_props: Vec<EarnedHabitatProp>,
    pub reconciled_lifetime_tokens_at: Option<f64>,
    #[serde(default)]
    pub earned_inhabitants: Vec<EarnedTankInhabitant>,
    #[serde(default)]
    pub reconciled_inhabitant_age_days_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarnedTankInhabitant {
    pub id: TankInhabitantId,
    pub earned_at: OffsetDateTime,
    pub source: TankInhabitantSource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TankInhabitantId(String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TankInhabitantSource {
    PetAgeDays { days: i64 },
}
```

This mirrors the existing habitat-prop pattern: state stores durable earned
facts only. Placement, daily cast selection, route phase, layer, and motion are
derived at view/render time.

Unknown ids from future versions or hand-edited state files are retained in
state but skipped by the view model.

## Runtime Data Flow

Add a small pure unlock detector, likely near `src/game/habitat.rs`, rather than
embedding this in a renderer.

Runtime order:

1. Load pet state.
2. Compute pet age in whole local days from `PetState.created_at` to the current
   local date.
3. Reconcile missing inhabitants whose age threshold is now satisfied.
4. Append new `EarnedTankInhabitant` records in catalog order.
5. Save state as part of the normal runtime save path.

The detector should be idempotent. Running it multiple times on the same day
does not duplicate records.

Existing pets are not reset. On first run after this feature ships, an older pet
earns the catalog entries it already qualifies for.

## Watch View Model

Extend `WatchViewModel.habitat` with catalog-backed inhabitant view data:

```rust
pub struct HabitatView {
    pub earned_props: Vec<EarnedHabitatPropView>,
    pub earned_inhabitants: Vec<EarnedTankInhabitantView>,
}

pub struct EarnedTankInhabitantView {
    pub id: TankInhabitantId,
    pub earned_at: OffsetDateTime,
    pub unlock_age_days: i64,
    pub kind: TankInhabitantKind,
}

pub enum TankInhabitantKind {
    Swimmer,
    Floor,
    Glass,
    Rim,
    Substrate,
    HostCombo,
}
```

The view model does not choose the daily cast or coordinates. Those choices
depend on local date, render surface, habitat geometry, and animation phase.

## Rendering Model

Introduce a renderer parallel to habitat props, for example
`src/tui/component/tank_life.rs`.

It receives:

- `HabitatView`
- `PetSceneLayout`
- pet seed
- local date
- animation clock
- activity/liveliness profile
- surface id
- color capability

It returns layered cells or placements:

```rust
pub struct TankLifeCell {
    pub inhabitant_id: TankInhabitantId,
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
    pub pet_layer: HabitatPetLayer,
}

pub struct TankLifePlacement {
    pub inhabitant_id: TankInhabitantId,
    pub cells: Vec<TankLifeCell>,
    pub bounds: Rect,
    pub pet_layer: HabitatPetLayer,
}
```

The render order uses the existing `HabitatPetLayer` concept:

1. background ambient texture
2. background / behind props
3. background / behind inhabitants
4. pet
5. foreground props
6. foreground inhabitants
7. HUD and perimeter gauges

For the round companion, the scene draw list already separates the shared pet
scene from the native HUD/perimeter. Tank life belongs inside the shared scene
draw list so the watch, preview lab, menubar popover, and companion can all use
the same catalog and motion logic. Surface-specific filtering can reduce count
or simplify motion, but not invent a separate catalog.

## Route Grammar

Each inhabitant declares a route family:

- **Cross-tank swimmer:** moves horizontally through a shallow arc, can pass
  behind the pet for part of the route and foreground for another part.
- **Floor resident:** moves along the lower tank, hopping, pausing, or gliding.
  It should read as a resident interacting with the floor, not as a floating
  mark.
- **Glass resident:** moves along an edge/wall route and can tuck under the
  perimeter region in round surfaces.
- **Rim resident:** loops near the outer tank and occasionally crosses the
  foreground rim.
- **Substrate resident:** appears from the bottom, pauses, then disappears.
- **Host combo:** draws an anchored anemone plus a small host fish orbiting
  locally around it.

Route phase is a pure function of `(inhabitant id, pet seed, local date,
animation time)`. It can include per-day speed/phase offsets, but not random
runtime state.

## Motion And Activity

Base motion is slow and ambient. Activity may modulate it:

- idle/quiet: more pauses, lower opacity for background lanes, slower loops
- active output: slightly faster cross-tank routes and brighter foreground
  passes
- reasoning/cache/mixed weather: may bias color intensity or pause rhythm, but
  should not create new glyph effects

This modulation must remain subtle. A busy day should wake the ecosystem; it
should not turn the tank into a particle system.

## Surface Behavior

### Watch TUI

The watch gets the full shared renderer when space allows. Small watch layouts
should reduce cast size before shrinking or obscuring pet art. Low-color/flat
mode should preserve silhouette and route differences even if color is disabled.

### Round Companion

The companion uses the same daily cast and route logic but may cap the visible
cast at the lower end of the daily target range to preserve the HUD and perimeter
gauges. Inhabitants must stay inside the round aperture. Foreground passes may
cross the tank interior, but they should avoid the bottom stat stack's reserved
area so the HUD does not sit on top of busy motion.

### Preview Lab

Preview Lab needs deterministic review frames for:

- age progression: empty, first inhabitant, early ecosystem, full pool
- daily cast rotation: three fixed local dates for the same mature pet
- depth lanes: behind-pet, foreground, and route-dependent passes
- Anemone Host morphs: Flower, Comb, Crown, Dot
- compact/round surfaces

The preview manifest should list selected daily cast ids and anemone morphs so
reviewers can tell whether a visual difference is intentional.

## Error Handling And Fallback

Ambient Tank Life is presentational over validated state. It should fail soft:

- Unknown inhabitant ids are retained in state and skipped in rendering.
- Invalid or future `earned_at` values do not crash the renderer; catalog age
  thresholds remain the source of truth.
- If local-date conversion is unavailable, use UTC date and keep the cast
  deterministic.
- If a route cannot fit in the current habitat rect, skip that inhabitant for
  the surface.
- If the terminal cannot render a glyph as single-width, the catalog entry must
  have a single-width fallback glyph before shipping.
- If color is unavailable, shape and placement still differentiate inhabitants.

## Testing

### Unit Tests

- Unlock reconciliation is idempotent and based only on age days.
- Existing older pets earn all age-qualified inhabitants on first reconcile.
- Future/unknown inhabitant ids survive state round-trip but are omitted from
  catalog-backed view data.
- Daily cast selection is stable for the same `(seed, date, surface, unlocked)`
  and changes for at least some adjacent dates once the pool is large enough.
- Cast size respects the age/cap rules and never exceeds five slots.
- `anemone_host` counts as one slot and selects exactly one of Flower, Comb,
  Crown, or Dot for a given day.
- Route helpers keep cells inside the habitat rect for representative watch and
  round companion sizes.
- Route-dependent layer segments include both behind and foreground phases for
  the inhabitants that promise depth.
- Low-color fallback paths keep unique glyph silhouettes.

### Integration / Preview Tests

- `cargo test --features dev-preview --test dev_preview`
- `cargo test --features dev-preview dev_preview::habitat_props` or the new
  equivalent tank-life preview test
- `cargo test --test round_scene`
- A `dev-preview --scenario props` or new `--scenario tank-life` bundle showing
  age progression, daily cast dates, and anemone morphs.

### Visual Review

Before implementation is considered done, inspect preview artifacts for:

- no overlap with pet art or HUD regions that reads as broken
- visible front/behind behavior
- each inhabitant silhouette is distinguishable at actual companion scale
- Anemone Host morphs read as one family, not four unrelated props
- the tank changes across dates without feeling crowded

## Non-Goals

- No manual aquarium editor.
- No streaks, check-ins, login rewards, or daily task mechanics.
- No token-based inhabitant unlocks.
- No achievements UI, collection shelf, or notifications.
- No new pet species or evolution stages.
- No full physics simulation or collision system.
- No persistent per-day cast history.
- No separate companion-only inhabitant catalog.
- No emoji-width glyphs without a proven monospace fallback.
