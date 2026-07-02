# Glorp Glitch Persistent Corruption - Design

- Date: 2026-07-02
- Status: direction approved by Drew; revised after staff review
- Scope: Glitch species polish only

## Goal

Make Glitch as cute and memorable as the recently improved Blob pass, without
turning it into generic visual noise.

Blob improved because its species concept became visible across multiple layers:
the elder form became a story beat, the face got small idle life, feed reactions
read as delight, and the habitat reinforced the soft creature identity. Glitch
already has a strong identity as a Packet Daemon, but it still often reads as a
cool terminal artifact instead of a lovable pet.

The new north star:

> Glitch is a mischievous little packet-being that sometimes breaks in cute ways,
> then patches itself. Its species feature is persistent corruption memory:
> small, bounded visual repair artifacts that linger after activity, feeding, or
> rare idle glitch-outs.

## Decisions

1. **Personality:** Mischievous Helper plus Buggy Little Friend.
   Glitch should feel clever, playful, and slightly unstable, but never broken or
   dead.
2. **Signature feature:** Persistent corruption memory.
   Glitch occasionally glitches out, then carries tiny self-repair marks for the
   current day.
3. **Persistence model for v1:** hybrid A/B.
   Day-local patch memory is the core feature; session-local glitching is the
   animated layer.
4. **No permanent pet-memory in v1.**
   Rare permanent scars could be delightful later, but v1 must not change pet
   identity or require a `state.json` migration.
5. **Elder charm is blocking.**
   S5/S6 must keep a readable living face, or a protected equivalent expression
   island, before this feature is considered shippable.

## Non-goals

- No new persisted pet identity fields.
- No `state.json` schema migration.
- No provider- or harness-branded Glitch behavior.
- No reads from `source_accent`, `source_diversity`, provider names, display
  names, or provider-first-use props when deriving Glitch corruption.
- No high-frequency flicker that makes the pet unreadable.
- No render-time stage-up trigger in v1. Stage-up glitching needs an explicit
  event input before it can be implemented correctly.
- No change to prop IDs, unlock semantics, calibration, ledger storage, or
  activity identity derivation.
- No broad species refactor. This is the Glitch pass.

## Product Behavior

### Day-local patch memory

Glitch gets zero to three tiny repaired-corruption marks for the current local
day. Selection is deterministic and storage-free:

```text
(pet seed, DayContext.date_seed, Species::Glitch, stage) -> ordered safe cells
```

The ordered cell list must not depend on live activity, feed reactions, calm
mode, work weather, provider/source identity, or tick timing. This prevents
restart drift and prevents already-visible marks from moving as the day changes.

The number of visible marks is a discrete day tier:

```text
GlitchPatchTier::Pristine -> 0 marks
GlitchPatchTier::Quiet    -> first 1 safe cell
GlitchPatchTier::Active   -> first 2 safe cells
GlitchPatchTier::Heavy    -> first 3 safe cells
```

`GlitchPatchTier` may be derived from stable ledger-backed day shape such as
`DayContext.today_ratio`. If a surface cannot provide that value, it should
fall back to `Quiet` rather than reading live `PetLifeProfile.activity_level`.
Small stages with fewer safe cells may render fewer marks.

Marks survive app restart because the same pet seed, day seed, stage, and day
tier recompute the same prefix of the same ordered cell list. They reset at the
next local dawn because `date_seed` already rolls at dawn. If activity grows
during the day, new marks reveal later cells from the ordered list; existing
marks do not relocate.

Patch marks should read as self-repair, not damage. Good vocabulary:

- one-cell checksum ticks,
- tiny `+` / `=` repair glyphs,
- bracket clamps on explicitly safe interior cells,
- cursor welds,
- softened packet repair dots.

Bad vocabulary:

- corpse eyes,
- wounds or scars,
- large scrambled regions,
- broken outline,
- anything that makes the terminal itself look corrupted.

All runtime patch glyphs must be display-width 1 under `unicode_width`.

### Session-local glitching

During live bursts, feed reactions, or rare tick-derived idle windows, Glitch can
briefly glitch more dramatically. The burst settles back into the day-local patch
marks.

Allowed v1 session inputs:

- `PetLifeProfile.burst_level`, after quantizing into an `Eq`-friendly enum,
- `AnimationFrame.feed_reaction`,
- `PetLifeProfile.calm_mode`,
- `PetLifeProfile.work_weather`, only as transient flavor,
- `AnimationFrame.tick`.

Session-local glitching should be short, legible, and bounded. It can include:

- brief character corruption,
- a cursor-tail blip near the body,
- a "patching itself" mouth/eye beat,
- a one-cell packet fragment that fades.

It should not persist across restart. Feed uses the same burst-to-repair path in
v1; it does not introduce separate permanent feed marks.

`calm_mode` suppresses dramatic transient glitching, but it can keep the calm
day-local patch marks.

### Elder charm

S5/S6 are the biggest current risk. S4 still has a clean mouth slot and can look
alive; S5/S6 bake mouth decoration and can drift toward "terminal boss block."

The implementation must choose one of these two approaches before merging:

1. Re-slot S5/S6 with explicit expression cells that preserve the current kernel
   silhouette.
2. Keep the baked elder art, but define a protected expression island that patch
   marks and transient corruption may never disturb.

Acceptance criteria:

- S5/S6 show a living expression in truecolor and flat-color previews.
- S5/S6 patch marks never overlap the eye span, baked expression island, or
  outline.
- The round S6 preview shows at least one declared safe patch cell inside the
  circular aperture when the tier permits one.
- The elder form can still feel powerful, but it must remain a pet.

Suggested protected elder islands, in raw 11-column pet-art coordinates before
the 13x10 frame is added:

- S5: row 1 eye span; rows 2-3, cols 3-7.
- S6: row 1 eye span; rows 2-3, cols 3-7.

These ranges can be adjusted during implementation only if the preview contract
and tests are updated with the replacement protected cells.

## Architecture

### Existing integration points

- `src/pet/art.rs`
  Owns Glitch stage templates, stage labels, and art invariants.
- `src/pet/render.rs`
  Owns render-time expression, blink cadence, bounded corruption, particles, and
  role spans.
- `src/tui/day.rs`
  Provides `DayContext.date_seed` and `DayContext.today_ratio`.
- `src/tui/life.rs`
  Provides `PetLifeProfile.burst_level`, `work_weather`, and `calm_mode` for
  transient behavior only.
- `src/commands/watch.rs`
  Builds the watch view model, then re-renders the pet after day/activity data
  is available.
- `src/tui/panels/pet/art_lines.rs`
  Consumes sorted, non-overlapping role spans.
- `src/dev_preview/pets.rs` and `src/dev_preview/scenarios.rs`
  Own Glitch preview frames and manifest inputs.

### Render contract

Add an optional Glitch presentation input to `AnimationFrame`, or add an adjacent
render-time struct if that keeps `AnimationFrame` simpler. Do not put raw `f32`
values into `AnimationFrame`; it currently derives `Eq`.

The desired shape is discrete and deterministic:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlitchCorruptionFrame {
    pub day_seed: u64,
    pub patch_tier: GlitchPatchTier,
    pub burst_level: GlitchBurstLevel,
    pub calm_mode: bool,
    pub feed_reaction: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlitchPatchTier {
    Pristine,
    #[default]
    Quiet,
    Active,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlitchBurstLevel {
    #[default]
    None,
    Small,
    Strong,
}
```

The final type names can follow surrounding style. The important boundary is
that `render_pet` remains pure: given pet, stage, mood, animation frame, and
derived presentation inputs, it returns deterministic lines/spans. It must not
read the clock, ledger, provider metadata, or state file.

### Watch and round integration

`build_watch_view_model_at` currently renders the pet before all day/activity
context has been attached to the view model. The Glitch implementation should
derive `GlitchCorruptionFrame` after the view model has `day_context`,
ledger-backed day tier, life profile, feed reaction, and calm mode, then call the
existing `rerender_pet_for_view_model` path.

The round companion copies `vm.pet_art`, so the round renderer should receive the
already-rerendered Glitch art. Do not implement a separate round-only corruption
path.

### Patch selection

Patch marks should be selected by a small deterministic function:

```text
ordered_glitch_patch_cells(pet, stage, day_seed) -> Vec<Cell>
visible prefix length = patch_tier.max_marks()
```

Rules:

- Only Glitch receives patch marks.
- Patch positions are based on pet seed, day seed, species, and stage only.
- Patch count is based on `GlitchPatchTier`.
- Stage S0/S1 may render zero marks if no safe body cell exists.
- Patch marks target interior body cells or explicitly allowlisted edge cells.
- Patch marks never target eye spans, mouth spans, protected expression cells, or
  the face center.
- Patch marks never break the closed packet-frame silhouette.
- The ordered list must be stable when `GlitchPatchTier`, `burst_level`,
  `feed_reaction`, `calm_mode`, or `work_weather` changes.

### Safe-cell classifier

Do not reuse the current `apply_glitch_corruption` target filter directly for
day-local repair marks. It only protects the eye center, which is not strict
enough for persistent marks.

Add a shared safety helper with a shape like:

```rust
fn safe_glitch_patch_candidates(
    stage: Stage,
    lines: &[String],
    spans: &[StyledSegment],
) -> Vec<Cell>;

fn is_protected_glitch_face_cell(
    stage: Stage,
    row: usize,
    col: usize,
    spans: &[StyledSegment],
) -> bool;
```

The classifier must reject:

- every cell covered by an `Eye` span,
- every cell covered by a `Mouth` span,
- protected S5/S6 baked expression islands,
- outline/silhouette cells unless explicitly allowlisted,
- whitespace cells,
- glyphs whose replacement would change display width.

The classifier may accept:

- interior `Body`, `Pattern`, or `Accent` cells,
- a tiny stage-specific allowlist of safe packet-edge cells if preview proves the
  silhouette remains intact.

### Span and role behavior

Existing TUI rendering expects sorted, non-overlapping spans. Persistent repair
marks must mutate the glyph in `lines` and retag that exact cell by splitting the
original span into non-overlapping segments, following the pattern of
`retag_cell_as_corruption`.

Do not add overlapping overlay spans for repair marks.

Use role semantics to distinguish the two effects:

- transient glitch-out cells use `PaletteRoleName::Corruption`,
- day-local repaired marks use existing softer roles, preferably `Pattern` for
  integrated repairs and `Accent` for at most one brighter clamp.

Do not use the loud `Corruption` role for persistent repaired marks unless a
future visual pass adds a softer corruption palette role.

### Interaction with existing corruption

Glitch already has bounded per-tick corruption. That should stay, but the new
feature changes the meaning:

- transient corruption is the moment of glitching,
- patch marks are the memory after repair.

The existing corruption path can be refactored to share the safer protected-cell
logic, but the day-local patch path needs stricter candidate selection than the
current "protect eye center" rule.

### Habitat and room flavor

The first implementation should keep habitat changes light. Glitch already has a
tuned room dialect and punctuation particles. Add only enough environment memory
to support the feature:

- one or two optional packet-tail particles during live bursts,
- possibly a tiny ambient cursor fragment near the pet,
- no changes to prop catalog identity.

If the body patch marks work, the room can remain secondary.

## Preview And Review

Preview Lab is the control surface before live TUI review. The preview contract
must prove the mechanics, not only show nice frames.

Required preview updates:

- Add `pet-glitch-persistence-states`, or expand `pet-glitch-live-states`, with
  the same pet seed/day rendered as quiet, active, burst, feed-repair, patched
  rest, same-day restart, and next-dawn reset.
- Include S5/S6 truecolor and flat-color variants for elder charm review.
- Add watch frames for `watch-glitch-patched-quiet`,
  `watch-glitch-patched-active`, `watch-glitch-burst`, and
  `watch-glitch-calm-hot`.
- Include compact/flat watch variants if repair color is hard to distinguish.
- Add `round-glitch-patched-s6` and assert at least one declared patch cell is
  visible inside the circular aperture when the tier permits one.

Typed preview artifacts should record, per relevant frame:

- `date_seed`,
- `patch_tier`,
- `burst_level`,
- `calm_mode`,
- `feed_reaction`,
- `expected_patch_count`,
- `selected_patch_cells`,
- `protected_face_cells`,
- whether the frame is same-day restart or next-dawn reset.

Cells JSON remains useful visual evidence, but it is not the mechanics contract
by itself. Add a typed artifact or manifest input block that tests can read.

Useful commands:

```bash
cargo run -- dev-preview --scenario pets --out target/glorp-preview
cargo run -- dev-preview --scenario watch --out target/glorp-preview
cargo run -- dev-preview --scenario round --out target/glorp-preview
```

## Testing

Focused checks should cover:

- patch marks are deterministic for the same seed/date/stage/tier,
- patch position order does not change when only the tier changes,
- increasing the tier reveals a prefix instead of relocating previous marks,
- patch marks change when `date_seed` changes,
- patch marks are absent for non-Glitch species,
- patch marks never overlap eye spans, mouth spans, or protected elder islands,
- patch marks never target outline/silhouette cells unless allowlisted,
- patch glyphs are display-width 1,
- repair marks produce sorted, non-overlapping spans,
- repair marks do not use `PaletteRoleName::Corruption`,
- transient corruption remains bounded and may still use `Corruption`,
- `calm_mode` suppresses dramatic glitching,
- feed reactions use the transient repair beat and do not create separate
  permanent feed marks,
- S5/S6 Glitch still render a living face or equivalent companionable expression,
- preview manifest/artifact data records patch inputs, protected cells, and
  expected counts.

Likely existing tests to extend:

```bash
cargo test --lib pet::art::tests::glitch_base_art_passes_phase1_invariants
cargo test --lib pet::art::tests::glitch_resting_face_is_alive
cargo test --lib pet::render::tests::glitch_particles_stay_punctuation_sized
cargo test --lib pet::render::tests::glitch_corruption_emits_corruption_role_spans_on_active_tick
cargo test --lib pet::render::tests::glitch_corruption_never_recolors_the_eye_center
cargo test --features dev-preview --test dev_preview dev_preview_pets_writes_species_stage_matrix
cargo test --test round_scene
```

Add new tests for:

- `ordered_glitch_patch_cells` stability and tier-prefix behavior,
- `safe_glitch_patch_candidates` on S2-S6,
- S5/S6 protected expression islands,
- repair span splitting,
- preview artifact fields and expected counts.

## Delivery Shape

1. **Lock render contract and persistence boundary.**
   Add the discrete Glitch corruption frame/tier types and confirm no persisted
   state or provider/source metadata is involved.
2. **Add preview and manifest contracts.**
   Create the deterministic Glitch persistence frames and typed artifacts before
   tuning live behavior.
3. **Implement pure patch selection and safety tests.**
   Build ordered day-local patch cells, safe-cell classification, and protected
   elder islands.
4. **Integrate through watch rerendering.**
   Derive the presentation frame from `WatchViewModel` data and re-render through
   `rerender_pet_for_view_model`, so round previews inherit the same art.
5. **Add session-local bursts.**
   Quantize burst/feed/calm inputs and layer transient glitching separately from
   day-local repair marks.
6. **Tune S5/S6 elder charm.**
   Choose re-slotted expressions or protected elder islands, then verify
   truecolor, flat-color, and round previews.
7. **Run pet/watch/round preview review.**
   Use Preview Lab artifacts as the go/no-go gate before live TUI review.
8. **Optional room flourish.**
   Add packet-tail particles only if the body feature needs environmental
   reinforcement.

## Implementation Tuning Notes

- Quiet days should render one tiny mark when a safe cell exists. `Pristine` is
  reserved for no safe cells, non-Glitch species, and explicit test fixtures.
- Feed reactions should use the same transient burst-to-repair path in v1.
- Exact repair glyph choices are visual tuning, but they must stay width-1 and
  must not read as injury.
