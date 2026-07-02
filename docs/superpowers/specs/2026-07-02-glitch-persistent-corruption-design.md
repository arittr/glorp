# Glorp Glitch Persistent Corruption - Design

- Date: 2026-07-02
- Status: direction approved by Drew; spec pending review
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
> small, bounded visual artifacts that linger after activity, feeding, or rare
> idle glitch-outs.

## Decisions

1. **Personality:** Mischievous Helper plus Buggy Little Friend.
   Glitch should feel clever, playful, and slightly unstable, but never broken or
   dead.
2. **Signature feature:** Persistent corruption memory.
   Glitch occasionally glitches out, then carries tiny self-repair marks for a
   while.
3. **Persistence model for v1:** hybrid A/B.
   Day-local patch memory is the core feature; session-local glitching is the
   animated layer.
4. **No permanent pet-memory in v1.**
   Rare permanent scars could be delightful later, but v1 must not change pet
   identity or require a `state.json` migration.

## Non-goals

- No new persisted pet identity fields.
- No `state.json` schema migration.
- No provider- or harness-branded Glitch behavior.
- No high-frequency flicker that makes the pet unreadable.
- No change to prop IDs, unlock semantics, calibration, ledger storage, or
  activity identity derivation.
- No broad species refactor. This is the Glitch pass.

## Product Behavior

### Day-local patch memory

Glitch gets one to three tiny repaired-corruption marks for the current local
day. They are deterministic from existing data:

- pet seed,
- `DayContext.date_seed`,
- stage,
- today's activity intensity or activity tier.

The marks survive app restart because they are recomputed from the dawn-rolled
day seed. They reset at the next local dawn because `date_seed` already rolls at
dawn. This keeps the feature feeling persistent without adding new storage.

Patch marks should read as self-repair, not damage. Good vocabulary:

- small stitch-like cells,
- one-cell repaired seams,
- a tiny patched packet edge,
- a leftover symbol in the body interior.

Bad vocabulary:

- corpse eyes,
- large scrambled regions,
- broken outline,
- anything that makes the terminal look corrupted.

### Session-local glitching

During live bursts, feed reactions, stage-up moments, or rare idle windows,
Glitch can briefly glitch more dramatically. The burst settles back into the
day-local patch marks.

Session-local glitching should be short, legible, and bounded. It can include:

- brief character corruption,
- a cursor-tail blip near the body,
- a "patching itself" mouth/eye beat,
- a one-cell packet fragment that fades.

It should not persist across restart unless it resolves into the deterministic
day-local patch memory.

### Elder charm

S5/S6 are the biggest current risk. S4 still has a clean mouth slot and can look
alive; S5/S6 bake mouth decoration and can drift toward "terminal boss block."

The Glitch pass should restore companionable elder charm by giving daemon/kernel
forms a readable living face or an equivalent cute repair beat. This can be done
by either:

- reintroducing clean face slots for S5/S6 if the art constraints allow it, or
- adding a stable, protected expression region plus tiny patch marks that do not
  disturb the silhouette.

The goal is not to make elder Glitch smaller or softer. It can still be powerful;
it just needs to remain a pet.

## Architecture

### Existing seams to use

- `src/pet/art.rs`
  Owns Glitch stage templates, stage labels, and art invariants.
- `src/pet/render.rs`
  Owns render-time expression, blink cadence, bounded corruption, particles, and
  role spans.
- `src/tui/day.rs`
  Provides `DayContext.date_seed`, a stable dawn-rolled day seed derived from
  local date and pet seed.
- `src/tui/life.rs`
  Provides `PetLifeProfile.activity_level`, `burst_level`, `work_weather`, and
  `calm_mode`.
- `src/commands/watch.rs`
  Re-renders the pet per tick and passes `AnimationFrame`.
- `src/dev_preview/pets.rs`
  Provides the Glitch live-state fixture and species-stage contact sheets.

### Render contract

Extend `AnimationFrame` with an optional Glitch corruption presentation input,
or add an adjacent render-time struct if that stays cleaner:

```rust
pub struct GlitchCorruptionFrame {
    pub day_seed: u64,
    pub activity_level: f32,
    pub burst_level: f32,
    pub calm_mode: bool,
}
```

The final shape should follow surrounding style. The important boundary is that
`render_pet` remains pure: given pet, stage, mood, animation frame, and derived
presentation inputs, it returns deterministic lines/spans. It must not read the
clock, ledger, or state file.

### Patch selection

Patch marks should be selected by a small deterministic function:

```text
(pet seed, date_seed, species, stage, activity tier) -> patch cells
```

Rules:

- Only Glitch receives patch marks.
- Patch marks target interior body cells or explicitly safe edge cells.
- Never target eye spans, mouth spans, or the protected face center.
- Never break the closed packet-frame silhouette.
- Cap to one mark on quiet days, two on active days, three on heavy days.
- `calm_mode` suppresses live glitch bursts but can keep calm day-local marks.

### Interaction with existing corruption

Glitch already has bounded per-tick corruption. That should stay, but the new
feature changes the meaning:

- transient corruption is the moment of glitching,
- patch marks are the memory after repair.

The existing "protect the eye center" rule remains non-negotiable. Any new
patching code should reuse the same safety concept, ideally through shared helper
logic instead of duplicating coordinate exclusions.

### Habitat and room flavor

The first implementation should keep habitat changes light. Glitch already has a
tuned room dialect and punctuation particles. Add only enough environment memory
to support the feature:

- one or two optional packet-tail particles during live bursts,
- possibly a tiny ambient cursor fragment near the pet,
- no changes to prop catalog identity.

If the body patch marks work, the room can remain secondary.

## Preview And Review

Preview Lab should be the control surface before live TUI review.

Required preview updates:

- `pet-glitch-live-states` should include a quiet day, active day, burst, and
  post-burst patched state.
- `pet-species-stage` should make S5/S6 Glitch's elder charm reviewable.
- Watch species dialect frames should be regenerated for Glitch vs Crystal
  figure-ground comparison if the room particles change.
- Round companion preview should include a Glitch dialect frame after any face or
  patch-mark change, because the circular aperture can hide tiny details.

Useful commands:

```bash
cargo run -- dev-preview --scenario pets --out target/glorp-preview
cargo run -- dev-preview --scenario watch --out target/glorp-preview
cargo run -- dev-preview --scenario round --out target/glorp-preview
```

## Testing

Focused checks should cover:

- patch marks are deterministic for the same seed/date/stage/activity tier,
- patch marks change when `date_seed` changes,
- patch marks are absent for non-Glitch species,
- patch marks never overlap eye or mouth spans,
- patch marks never break art width or occupied-cell invariants,
- live burst corruption remains bounded,
- `calm_mode` suppresses dramatic glitching,
- S5/S6 Glitch still render a living face or equivalent companionable expression.

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

## Delivery Shape

1. **Preview fixtures first.**
   Add or adjust Glitch preview states so the work can be judged visually before
   changing live behavior.
2. **Patch-mark derivation.**
   Implement deterministic day-local patch cells and safety tests.
3. **Render integration.**
   Apply patch marks to Glitch art with role spans and protected face logic.
4. **Burst-to-patch behavior.**
   Layer session-local live glitching from `PetLifeProfile`/`AnimationFrame`.
5. **Elder charm pass.**
   Tune S5/S6 face/patch readability in the pet matrix and round preview.
6. **Optional room flourish.**
   Add packet-tail particles only if the body feature needs environmental
   reinforcement.

## Open Questions

- Should quiet days always get exactly one tiny mark, or should completely idle
  days be pristine?
- Should feed reactions create their own distinct patch vocabulary, or just
  trigger the same burst-to-patch path?
- Can S5/S6 regain clean face slots without weakening the current kernel
  silhouette?

These do not block the design. They should be resolved during implementation
against Preview Lab artifacts.
