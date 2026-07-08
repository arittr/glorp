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

1. **Age earns inhabitants.** Unlocks come from calendar age, measured from
   `PetState.created_at` through the same local-day mapping used for provider
   days. Lifetime tokens, today tokens, and rate momentum do not unlock
   inhabitants.
2. **The collection persists; the cast rotates.** Earned inhabitants remain in
   state. A surface-independent canonical daily cast is derived from pet seed,
   local date, and unlocked ids. Each surface then projects that canonical cast
   into a safe rendered subset.
3. **Daily means stable for the day.** A local-day cast does not re-randomize
   per frame or per app restart. Tomorrow may look different.
4. **Routes over effects.** Inhabitants are residents with movement paths:
   cross-tank, lower-lane, glass/rim, lower-edge, and local host orbit. They
   are not sparks generated on props.
5. **Depth is part of the feature.** Every inhabitant declares a natural layer
   behavior: background, mid, foreground, or route-dependent. Route-dependent
   inhabitants can pass behind the pet in one segment and in front in another.
6. **Activity livens, but does not earn.** Live activity, burst level, or day
   weather may increase speed, brightness, pause cadence, and tiny route timing.
   It never changes the earned pool, canonical cast, rendered count, or whether
   a selected inhabitant appears that day. Quiet days still show the day's cast.
7. **The pet remains the hero.** Cast size is capped, glyphs stay tiny, the pet
   face is protected, and the HUD stays readable.
8. **No new round-companion floor.** Lower-lane inhabitants do not add a literal
   floor or substrate to the round companion. On round surfaces they use lower
   arcs, depth lanes, and edge peeks inside the existing free-float porthole.

## Catalog V1

The first catalog intentionally uses a small set of distinct silhouettes and
movement grammars. More inhabitants can be added later without changing the
data model.

| id | Unlock age | Glyph family | Route | Natural layer |
|---|---:|---|---|---|
| `glass_shrimp` | day 1 | `,~` / `,≈` | lower-lane hops and pauses | foreground lower lane |
| `needlefish` | day 3 | `‹·` | cross-tank swim | route-dependent |
| `glass_snail` | day 7 | `◔` | glass-wall creep | foreground edge |
| `burrower` | day 10 | `▴` | lower-edge peek/hide | foreground lower lane |
| `rim_skimmer` | day 14 | `◜` | perimeter loop | route-dependent |
| `sand_ray` | day 21 | `▱` | lower-lane glide | foreground lower lane |
| `schoollet` | day 28 | `‹ ‹` cluster | grouped cross-tank pass | route-dependent |
| `anemone_host` | day 35 | anchor + `›·` fish | local orbit around anchor | anchor behind, fish route-dependent |

The exact days are v1 defaults. Age 0 is intentionally a pet-only starter tank;
the first resident appears after one local-day boundary. Existing pets receive
all inhabitants whose age threshold they already satisfy on the next
reconciliation.

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

Daily selection has two stages: a canonical cast that defines "today's tank,"
then a surface projection that makes that cast safe for a specific renderer.

### Canonical Cast

The canonical cast is deterministic and surface-independent.

Inputs:

- pet seed
- local date
- unlocked inhabitant ids

Rules:

1. If no inhabitants are unlocked, render none.
2. If one or two are unlocked, render all of them.
3. From day 7 through day 20, target 2 or 3 visible inhabitants.
4. From day 21 through day 59, target 3 or 4 visible inhabitants.
5. From day 60 onward, target 4 or 5 visible inhabitants.
6. Never include more than 5 moving inhabitant slots in the canonical cast.
7. `anemone_host` counts as one slot even though it draws an anchor plus a host
   fish.
8. The canonical cast may include inhabitants that a small surface later skips.

The selection should use a small pure helper, for example:

```rust
pub fn canonical_daily_cast(
    unlocked: &[EarnedTankInhabitantView],
    pet_seed: &str,
    local_date: time::Date,
) -> Vec<TankInhabitantId>
```

No current-cast field is persisted. Recomputing the canonical cast from the same
inputs must return the same result.

### Surface Projection

Each renderer projects the canonical cast into a safe rendered subset. Projection
may filter, cap, or simplify members, but it may not re-randomize the cast.

Inputs:

- canonical daily cast
- target surface, for example `Watch`, `Round`, or `Menubar`
- target habitat size
- surface geometry, including any aperture mask and reserved regions

Projection returns both rendered ids and skip reasons:

```rust
pub struct RenderedTankLifeCast {
    pub canonical_ids: Vec<TankInhabitantId>,
    pub rendered_ids: Vec<TankInhabitantId>,
    pub skipped: Vec<TankLifeSkip>,
}

pub struct TankLifeSkip {
    pub id: TankInhabitantId,
    pub reason: TankLifeSkipReason,
}
```

Surface budgets:

- Watch TUI: up to the canonical cap when the habitat rect is large enough.
- Round companion: default max 2 moving inhabitant slots. It may render 3 only
  when preview/device review proves the pet face, bottom HUD, and perimeter
  gauges remain readable.
- Menubar popover: follow the round budget unless a later spec approves a
  separate density.

`anemone_host` still counts as one catalog slot, but projection must account for
its visual footprint: anchor plus host fish. If it cannot fit safely, the
surface should skip it and record the reason rather than crowding the tank.

## State Model

Extend `HabitatState` with time-earned inhabitant facts:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HabitatState {
    pub earned_props: Vec<EarnedHabitatProp>,
    pub reconciled_lifetime_tokens_at: Option<f64>,
    #[serde(default)]
    pub earned_inhabitants: Vec<EarnedTankInhabitant>,
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
    PetAgeThreshold { threshold_days: i64 },
}
```

This mirrors the existing habitat-prop pattern: state stores durable earned
facts only. Placement, daily cast selection, route phase, layer, and motion are
derived at view/render time.

`PetAgeThreshold.threshold_days` records the catalog threshold that caused the
unlock. It is not the pet's age at reconciliation time. For backfilled older
pets, `earned_at` is the reconciliation timestamp and `threshold_days` preserves
which age milestone each inhabitant represents.

Unknown ids from future versions or hand-edited state files are retained in
state but skipped by the view model. This promise applies to unknown ids inside
otherwise-loadable state; the first version does not need a custom
forward-compatible deserializer for unknown future `TankInhabitantSource`
variants.

This should be a no-bump serde-default migration: existing schema-version-1
state files load because `earned_inhabitants` has a default. The implementation
must include a legacy JSON fixture test without the new field.

## Runtime Data Flow

Add a small pure unlock detector, likely near `src/game/habitat.rs`, rather than
embedding this in a renderer.

Age is calendar-day based, not elapsed 24-hour duration based. Use one helper
for all inhabitant unlocks and preview fixtures:

```rust
pub fn calendar_age_days(
    created_at: OffsetDateTime,
    now: OffsetDateTime,
    local_day_mapper: &LocalDayMapper,
) -> i64
```

The helper maps both timestamps to local dates with the same local-day mapper
used for provider-day behavior, subtracts dates, and clamps negative values to
zero. It must not reuse the existing elapsed-duration `age_days` display field.
If local-date conversion is unavailable, use UTC date as an explicit fallback.

Runtime order:

1. Load pet state.
2. Compute calendar age days from `PetState.created_at` to `now`.
3. Reconcile missing inhabitants whose age threshold is now satisfied.
4. Append new `EarnedTankInhabitant` records in catalog order.
5. If reconciliation added records, save state immediately.
6. Continue into the normal usage-provider poll/apply path.

The detector should be idempotent. Running it multiple times on the same day
does not duplicate records.

Existing pets are not reset. On first run after this feature ships, an older pet
earns the catalog entries it already qualifies for.

This reconciliation must run after state load and before provider success is
required in the watch, companion, and menubar entrypoints. A provider failure
must not prevent an old pet from receiving age-qualified inhabitants. The
renderer and view-model conversion remain read-only.

Do not use a `reconciled_age_days_at` skip guard. Reconciliation always scans
the catalog, compares ids against `earned_inhabitants`, and appends only missing
qualified ids. That lets future catalog additions backfill correctly for old
pets.

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
    LowerLane,
    Glass,
    Rim,
    LowerEdge,
    HostCombo,
}
```

The view model does not choose the daily cast or coordinates. Those choices
depend on local date, target surface, habitat geometry, and animation phase.

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
- target surface
- surface geometry
- color capability

Surface geometry is explicit. Round rendering cannot rely on AppKit HUD layout
that is computed later. The shared tank-life renderer needs enough geometry to
avoid reserved regions before cells are produced:

```rust
pub struct TankLifeSurfaceGeometry {
    pub surface: TankLifeSurface,
    pub habitat: Rect,
    pub aperture_mask: Option<RoundApertureMask>,
    pub reserved_regions: Vec<Rect>,
    pub max_moving_slots: usize,
    pub literal_floor_allowed: bool,
}
```

For the round companion, `literal_floor_allowed` is `false`, `aperture_mask` is
present, and `reserved_regions` include the bottom stat/HUD area and any
perimeter-gauge no-go band converted to cell space.

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
}
```

Layering authority lives on cells or route segments, not on the whole placement.
Route-dependent inhabitants may emit background cells at one timestamp and
foreground cells at another. If an implementation prefers uniform-layer
placements, it must split route output into separate background/foreground
placement batches.

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

Catalog glyphs and morphs are explicit cell sprites, not free-form strings.
Every rendered cell glyph, including fallback glyphs, must be a single Unicode
scalar with terminal display width 1. Multi-cell forms like Anemone Host morphs
are represented as `Vec<SpriteCell>` rows and columns so preview/raster code
never has to split a multi-character cell.

## Route Grammar

Each inhabitant declares a route family:

- **Cross-tank swimmer:** moves horizontally through a shallow arc, can pass
  behind the pet for part of the route and foreground for another part.
- **Lower-lane resident:** moves along the lower tank, hopping, pausing, or
  gliding. It should read as resident motion in the lower depth lane, not as a
  floating mark. On round companion surfaces this lane is an arc/depth cue, not
  a literal floor.
- **Glass resident:** moves along an edge/wall route and can tuck under the
  perimeter region in round surfaces.
- **Rim resident:** loops near the outer tank and occasionally crosses the
  foreground rim.
- **Lower-edge resident:** appears from the lower edge, pauses, then
  disappears. On round companion surfaces this is a peripheral peek, not a new
  substrate band.
- **Host combo:** draws an anchored anemone plus a small host fish orbiting
  locally around it.

Route phase is a pure function of `(inhabitant id, pet seed, local date,
animation time)`. It can include per-day speed/phase offsets, but not random
runtime state.

Foreground occlusion is deliberately limited. On the round companion,
foreground tank-life cells must never cover the pet's eyes or mouth, should not
cross the central face/body region, and must avoid HUD/stat reserved regions.
Foreground passes should be brief. When in doubt, the round projection should
prefer background/behind-pet depth unless Preview Lab proves the foreground pass
reads as depth rather than pet damage.

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

The companion starts from the same canonical daily cast and projects it to a
round-safe rendered subset. Default budget is 2 moving inhabitant slots; 3 is
allowed only after preview/device review proves the pet face, bottom HUD, and
perimeter gauges remain readable.

Inhabitants must stay inside the round aperture and outside reserved HUD/gauge
regions. Foreground passes may cross the tank interior, but they must avoid the
bottom stat stack and protected pet-face region. Lower-lane residents use lower
arcs and edge peeks; they do not add a literal floor or substrate band to the
free-float companion tank.

### Preview Lab

Preview Lab needs deterministic review frames for:

- age progression: empty, first inhabitant, early ecosystem, full pool
- daily cast rotation: three fixed local dates for the same mature pet
- canonical-vs-rendered cast projection for watch and round targets
- depth lanes: behind-pet, foreground, and route-dependent passes at named
  timestamps
- Anemone Host morphs: Flower, Comb, Crown, Dot
- compact/round surfaces

Preview Lab should pass real target surfaces (`Watch`, `Round`, `Menubar`) plus
fixture metadata. It should not invent a separate `Preview` surface id that can
diverge from shipped renderers.

The preview bundle must include a typed `tank_life` artifact for each relevant
frame or strip. It should list:

- local date and calendar age days
- target surface
- canonical cast ids
- rendered cast ids
- skipped ids with reasons
- anemone morph, when selected
- route family for each rendered inhabitant
- layer segment/cell summaries
- bounds and cell count
- reserved-region collision status

If visible HUD/gauge overlays are not added to round preview frames, the typed
artifact is required to prove the same no-go regions and collision checks that
the live companion uses.

## Error Handling And Fallback

Ambient Tank Life is presentational over validated state. It should fail soft:

- Unknown inhabitant ids are retained in state and skipped in rendering.
- Invalid or future `earned_at` values do not crash the renderer; catalog age
  thresholds remain the source of truth.
- If local-date conversion is unavailable, use UTC date and keep the cast
  deterministic.
- If a route cannot fit in the current habitat rect, skip that inhabitant for
  the surface and record a skip reason in projection/preview artifacts.
- If the terminal cannot render a glyph as single-width, the catalog entry must
  have a single-width fallback glyph before shipping. The preferred v1 catalog
  should avoid known wide glyphs rather than relying on fallbacks.
- If color is unavailable, shape and placement still differentiate inhabitants.

## Testing

### Unit Tests

- `calendar_age_days` uses local-date difference, clamps future `created_at` to
  zero, and has table fixtures for local midnight, UTC-vs-local mismatch, DST
  transition, and UTC fallback.
- Unlock reconciliation is idempotent, scans the catalog every time, and is
  based only on calendar age days.
- Existing older pets earn all age-qualified inhabitants on first reconcile,
  with `PetAgeThreshold.threshold_days` recorded for each backfilled id in
  catalog order.
- Reconciliation after state load persists age-qualified inhabitants even when
  usage-provider polling later fails.
- Old schema-version-1 state JSON without `earned_inhabitants` loads through
  serde defaults; future/unknown inhabitant ids survive state round-trip and
  are omitted from catalog-backed view data.
- Canonical daily cast selection has exact fixture outputs for a fixed seed,
  three local dates, and unlocked pools at ages 0, 1, 3, 7, 21, and 60.
- Surface projection has exact fixture outputs for watch and round targets,
  including rendered ids, skipped ids, skip reasons, target counts, and the
  Anemone Host morph when selected.
- Cast size respects the age/cap rules and never exceeds five canonical slots.
  Round projection defaults to at most two moving slots.
- `anemone_host` counts as one canonical slot, selects exactly one of Flower,
  Comb, Crown, or Dot for a given day, and projection accounts for its anchor
  plus host-fish footprint.
- Catalog validation asserts every rendered/fallback cell glyph has terminal
  display width 1 and each low-color fallback silhouette remains distinct.
- Route helpers keep cells inside the habitat rect for representative watch and
  round companion sizes, and round routes stay inside aperture/no-go geometry.
- Route-dependent layer segments include both behind and foreground phases at
  named timestamps for inhabitants that promise depth.
- Round occlusion tests assert no foreground tank-life cells enter the HUD/stat
  reserved region or protected pet-face region.

### Integration / Preview Tests

- `cargo test --features dev-preview --test dev_preview`
- `cargo test --features dev-preview dev_preview::habitat_props` or the new
  equivalent tank-life preview test
- `cargo test --test round_scene`
- A `dev-preview --scenario props` or new `--scenario tank-life` bundle showing
  age progression, daily cast dates, and anemone morphs.
- Preview manifest/contract tests assert `tank_life` artifact presence and
  fields: local date, canonical/rendered ids, skip reasons, morph, route family,
  layer segments, bounds, cell count, and reserved-region collision status.

### Visual Review

Before implementation is considered done, inspect preview artifacts for:

- no literal floor/substrate added to the round companion
- no overlap with pet face or HUD regions that reads as broken
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
- No preview-only inhabitant behavior that diverges from watch/round/menubar
  targets.
- No literal floor or substrate added to the round companion.
- No emoji-width glyphs without a proven monospace fallback.
