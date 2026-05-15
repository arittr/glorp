# Habitat Props - Design

Date: 2026-05-14
Status: Approved design, written for review.
Source: Habitat-props brainstorm on 2026-05-14.

## Problem

Glorp's pet changes through the seven evolution stages, but the time between
stage changes can feel flat. The watch habitat already has species-flavored
ambient texture: sky glyphs, a floor row, and stage-scaled density. That
texture helps the pet scene feel less empty, but it does not create discrete
objects the user can recognize or remember.

The pet's living space should accumulate a small amount of visible history
between evolutions. The habitat should feel more inhabited after real use, not
only after stage thresholds.

## Goal

Add a habitat prop system that unlocks small visual objects from real pet
milestones. Props are drawn in the pet scene, move subtly, and make the habitat
feel lived in while keeping the pet as the hero.

The first version ships a small, focused catalog:

- Lifetime-token ladder accents: one-cell glyphs that unlock at increasing
  lifetime effective-token thresholds.
- Permanent trophy props: small multi-cell sprites for a few rarer milestones.
- Rotating visible accents: only a bounded subset of earned ladder accents is
  shown at once, selected deterministically from the earned collection and
  wall-clock time.

Everything earned is stored. The watch display chooses a safe, readable subset.

## Non-Goals

- No new evolution stages.
- No changes to pet art templates in `src/pet/art.rs`.
- No full achievements, inventory, shelf, customization, or prop-management UI.
- No manual feed, fake milestones, demo-only controls, or user-triggered props.
- No persisted placement coordinates in the first version.
- No new watch key bindings or commands.
- No notification/toast surface for unlocks in the first version.
- No backwards historical reconstruction for event-only trophies. Existing pets
  can reconcile token-ladder props from `lifetime_effective_tokens`; trophies
  that depend on observed transitions start earning after this feature ships.

## Decisions

1. **Props are not tied to evolution.** Evolution stays the slow primary arc.
   Props add smaller beats between stages.

2. **Hybrid fidelity.** Trophy props are tiny multi-cell sprites, such as a
   plant, lamp, or crystal cluster. Ladder props are one-cell accents, such as
   pebbles, shells, sparks, or small light marks.

3. **Trophies plus rotating accents.** The habitat keeps up to three permanent
   trophy sprites visible. Ladder accents rotate through the remaining space so
   the scene continues changing after early unlocks.

4. **Persist earned facts, not layout noise.** State records which props were
   earned and when. Placement, accent rotation, and idle motion are derived at
   view/render time from the pet seed, catalog, scene geometry, earned props,
   and clock.

5. **Subtle motion is part of the feature.** Props should flicker, sway, twinkle,
   or bob gently. A static prop layer would solve only the first minute of the
   problem and then become new wallpaper.

6. **The pet always wins the draw order.** Habitat texture draws first, props
   draw second, and pet art/effects draw last. If the pet wanders across a prop,
   the pet appears in front.

## Unlock Catalog V1

The first catalog is intentionally small. More props can be added later without
changing the architecture.

### Ladder Accent Props

Ladder props unlock from `PetState.lifetime_effective_tokens`. These thresholds
are absolute effective-token facts, not calibrated evolution progress.

| Prop id | Threshold | Form | Display role |
|---|---:|---|---|
| `token_pebble_25k` | 25,000 | one-cell accent | early visible change |
| `token_shell_100k` | 100,000 | one-cell accent | early visible change |
| `token_spark_500k` | 500,000 | one-cell accent | mid early milestone |
| `token_shard_1m` | 1,000,000 | one-cell accent | first million mark |
| `token_orbit_5m` | 5,000,000 | one-cell accent | mature habitat mark |
| `token_lantern_10m` | 10,000,000 | one-cell accent | long-life mark |

If one poll crosses multiple thresholds, all crossed props are earned in order.
If an old state file already has enough lifetime tokens when this feature is
introduced, the runtime reconciles missing ladder props once on the next apply.

### Trophy Props

Trophy props unlock from runtime events observed during `apply_unapplied_usage`.

| Prop id | Trigger | Form | Display priority |
|---|---|---|---:|
| `codex_signal_lamp` | first applied usage row from provider `codex` | multi-cell sprite | 70 |
| `heavy_session_planter` | first apply pass whose new effective tokens are at least 50,000 and at least 0.5x calibration baseline | multi-cell sprite | 80 |
| `wilt_recovery_sprout` | mood moves from `Wilted` to any non-wilted mood after food is applied | multi-cell sprite | 90 |

Trophy triggers are event-based and only fire once. If a trigger depends on a
transition that happened before this feature existed, it is not backfilled.
That keeps the first version honest about what the runtime actually observed.

## State Model

Add habitat state to `PetState` with a serde default so existing state files
continue loading:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HabitatState {
    pub earned_props: Vec<EarnedHabitatProp>,
    pub reconciled_lifetime_tokens_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarnedHabitatProp {
    pub id: HabitatPropId,
    pub earned_at: OffsetDateTime,
    pub source: HabitatPropSource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HabitatPropId(String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HabitatPropSource {
    LifetimeTokens { threshold: f64 },
    ProviderFirstUse { provider_surface: String },
    HeavySession,
    WiltRecovery,
}
```

`HabitatPropId` is a typed newtype over a string, not a loose string used
throughout the code. All behavior goes through the static prop catalog. Unknown
ids from future or edited state files are retained in state but skipped by the
renderer.

`reconciled_lifetime_tokens_at` records that the state has already reconciled
ladder props for the current lifetime counter. It prevents repeated migration
work while still allowing new ladder thresholds to unlock when lifetime tokens
increase.

## Runtime Data Flow

Prop unlocking happens in `src/game/runtime.rs`, inside
`apply_unapplied_usage`, after usage rows have been applied and after mood
transitions are known.

Runtime order:

1. Reconcile saved stage with XP, as today.
2. Load unapplied usage rows, as today.
3. Apply effective-token deltas, as today.
4. Apply idle decay or food, as today.
5. Detect prop unlocks from the new state and the apply-pass inputs.
6. Emit narration for existing pet events, as today.
7. Save `last_usage_poll_at`, trim recent events, compact old usage, as today.

The unlock detector is a small pure module, for example
`src/game/habitat.rs`, with functions like:

```rust
pub fn unlock_habitat_props(
    state: &mut PetState,
    rows: &[UsageLedgerRow],
    recent_effective_tokens: f64,
    initial_mood: Mood,
    new_mood: Mood,
    now: OffsetDateTime,
) -> Vec<HabitatPropId>
```

The implementation can keep this exact boundary or split the detector into
smaller helpers, but it must use `UsageLedgerRow`-backed applied rows rather
than re-querying display aggregates. Runtime owns durable earned facts;
rendering never mutates `PetState`.

Duplicate prevention is by prop id. If the state already contains an id, the
detector does not add it again.

## Watch View Model

`WatchViewModel` gains a habitat field:

```rust
pub struct HabitatView {
    pub earned_props: Vec<EarnedHabitatPropView>,
}

pub struct EarnedHabitatPropView {
    pub id: HabitatPropId,
    pub earned_at: OffsetDateTime,
    pub kind: HabitatPropKind,
    pub display_priority: i16,
}

pub enum HabitatPropKind {
    Trophy,
    Accent,
}
```

`build_watch_view_model_at` converts stored earned props into catalog-backed
view data. Unknown ids are omitted from the view model. The view model does not
choose coordinates; coordinates depend on `PetSceneLayout`, panel size, speech
visibility, and the current render clock.

## Rendering Model

Rendering stays inside the existing pet scene path:

```text
PetScene::compute_layout(area, vm, ctx)
    -> habitat rect
    -> speech/pet exclusions

PetPanel::render
    pass 1: ambient_glyphs_for(...)
    pass 2: habitat_props_for(...)
    pass 3: render_pet_inside(...)
```

`habitat_props_for` takes the catalog-backed `HabitatView`, `PetSceneLayout`,
species, stage, seed, and clock. It returns positioned cells or sprite cells:

```rust
pub struct HabitatPropCell {
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
}
```

The placement algorithm is deterministic:

- Trophy sprites use stable anchor slots: left floor, right floor, rear wall.
- Displayed trophies are the highest-priority earned trophy props, capped at
  three. Ties prefer older earned props, so early major trophies remain visible.
- Accent glyphs use the remaining safe cells and a deterministic rotating
  window based on `minute_floor(now) / 10`.
- Accent placement excludes the current pet-art rect and speech bubble.
- Trophy anchors avoid the central wander lane when possible, but the pet still
  paints on top if it crosses them.

All prop catalog glyphs must be single-column. No emoji-width characters.

## Motion

Motion is deterministic and clock-driven. It must not require persisted state.

Motion styles:

- Plant/sprout: two-frame sway by swapping one or two glyphs every few seconds.
- Lamp: low-frequency flicker by changing brightness or the lit glyph.
- Crystal/spark: twinkle by alternating a highlight glyph.
- Pebble/shell/orbit accents: tiny bob or phase swap, no coordinate jump larger
  than one cell.

The prop renderer uses the same fixed `WatchClock` support as the rest of the
watch. Dev-preview output stays stable because preview runs with a fixed clock.

## Styling

Props use existing semantic palette roles where possible:

- Trophy sprites use the species role color at normal or slightly dim strength.
- Accent glyphs use the dim habitat color so they do not compete with pet art.
- Motion highlights can temporarily use the accent color, but only for one or
  two cells.

Color capability behavior follows the current `ColorCapability` enum:

- `Truecolor` renders full props with species/dim/accent styling.
- `Flat` omits ambient texture and ladder accents. Trophy props may render only
  when the glyph shape stays readable with default terminal foreground styling.

## Error Handling

- Unknown prop ids in state are ignored by the watch renderer and retained on
  save. A malformed prop entry still makes `state.json` invalid, matching the
  current state-store behavior.
- Unlock detection must tolerate missing provider surfaces. No provider trophy
  fires unless the applied usage row has the expected provider surface.
- Heavy-session detection uses the apply-pass token sum and the already-loaded
  calibration baseline. It returns no unlock when the threshold is not met.
- If there is not enough habitat space for all visible props, trophies win,
  then accents are reduced. The renderer never writes outside the habitat rect.
- Prop rendering is display-only. A render failure or skipped unknown id must
  not block usage polling or state saves.

## Preview Lab

Add prop coverage to the hidden preview lab:

- `watch-wide-normal` includes a fixture with two trophy props and several
  ladder accents.
- `watch-compact-normal` verifies the pet scene remains readable when habitat
  space is smaller.
- The manifest records the fixture prop ids so reviewers can tell which
  catalog entries should be visible.

The preview remains dev-only and does not read real user state.

## Testing

### Runtime Tests

- Crossing one lifetime threshold records exactly one ladder prop.
- Crossing multiple lifetime thresholds in one poll records all crossed props
  in threshold order.
- Re-running apply with no new threshold does not duplicate props.
- Existing `lifetime_effective_tokens` reconciles missing ladder props once.
- First Codex usage unlocks `codex_signal_lamp` once.
- A heavy apply pass unlocks `heavy_session_planter` once.
- Wilted-to-non-wilted recovery unlocks `wilt_recovery_sprout` once.
- Event-only trophies are not backfilled from old usage history.

### View-Model Tests

- Unknown stored prop ids are omitted from `HabitatView`.
- Earned props preserve `earned_at`, kind, and display priority from the
  catalog.
- Fixture view models include representative props for render tests.

### Render Tests

- Prop cells stay inside `PetSceneLayout.habitat`.
- Accent cells do not overlap the current pet-art rect or speech bubble.
- Pet art overwrites prop cells when they collide.
- Trophy selection caps at three and prefers higher display priority.
- Accent rotation is deterministic for a fixed clock and changes at the
  documented rotation interval.
- Motion is deterministic for a fixed clock.
- Every catalog glyph is single-column.

### Dev-Preview Tests

- `cargo test --test dev_preview --features dev-preview`
- `cargo test dev_preview::scenarios --features dev-preview`
- `cargo test dev_preview::export --features dev-preview`

The visual review must include the generated `watch-wide-normal` and
`watch-compact-normal` frames before implementation is considered done.

## Risks

1. **The prop catalog becomes too cute or too busy.** The first catalog is
   intentionally small and physical. Props should read as terminal objects, not
   stickers.

2. **State grows into an achievements system.** The state model stores only
   earned prop facts. No shelf, inventory, unlock UI, or achievements panel is
   introduced in this pass.

3. **Rotation recreates the same flatness later.** Trophy sprites stay stable,
   while ladder accents rotate and idle motion continues between unlocks. If
   this still feels flat, the next tuning knob is motion cadence, not more
   storage.

4. **Absolute token thresholds favor high-usage users.** This is accepted for
   ladder props because they represent absolute work done. Evolution remains
   calibrated for relative growth.

5. **Existing pets miss old event trophies.** Accepted for v1. Token-ladder
   props reconcile from durable counters; event trophies require observed
   transitions and start from the feature's install point.

## Sequencing

One implementation plan, likely split into small commits:

1. Add habitat state types and catalog.
2. Add runtime unlock detection and tests.
3. Add view-model habitat data.
4. Add prop renderer and motion.
5. Update Preview Lab fixtures and snapshots.
6. Run the relevant Rust checks and a real visual pass with dev-preview.

Do not start implementation until Drew reviews this written spec.
