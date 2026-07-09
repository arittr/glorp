# Glorp Smooth Motion Stabilization - design

- Date: 2026-07-09
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-companion-renderer-v2-design.md`
  - `docs/superpowers/plans/2026-07-08-glorp-smooth-companion-renderer-v2-slice-1-implementation.md`

## Calibration

The smooth companion is not meant to become a new abstract pet. It is the
current Classic Glorp companion, with the existing generated pet art, habitat
props, tank life, ambient marks, gauges, HUD, and round tank composition,
rendered through a motion-capable pipeline.

The first smooth renderer slice proved that the Classic scene can be carried as
semantic layers and repainted at a higher cadence, but live review exposed two
timing problems:

1. The old Classic pet art animator is being advanced on every smooth repaint.
   Smooth mode repaints around 30 FPS, while Classic pet art was authored around
   the 250 ms UI tick. As a result, glitch/blink/art-frame changes run much too
   fast and read as flashing.
2. The larger tank wander still resolves to integer grid anchors. The bob layer
   has fractional motion, but the pet anchor, aura, and surrounding cues can
   still jump cell-by-cell.

This follow-up stabilizes time before adding richer depth, parallax, rim light,
squash/stretch, or feed effects.

## Goals

1. Keep the smooth companion visually anchored to the current Classic Glorp
   companion.
2. Decouple the fast native paint loop from slower Classic semantic/art updates.
3. Preserve smooth repaint cadence for continuous transforms, bob, and future
   effects.
4. Move the pet, aura, contact shadow, and pet-attached cues from a continuous
   anchor instead of a snapped `u16` anchor in smooth mode.
5. Add review evidence that catches fast art-frame flashing and snapped anchor
   motion before live AppKit review.
6. Leave Classic and Pixel renderers behaviorally unchanged.

## Non-goals

- No default flip to smooth mode.
- No replacement of generated Classic pet art.
- No new Pixel full-frame companion.
- No new 3D engine, physics engine, or authored asset pipeline.
- No broad visual redesign of gauges, HUD, tank props, or pet body art.
- No rewrite of `glorp watch`.
- No Linux windowing implementation in this slice.

## Design

Smooth mode needs two clocks:

- **Paint clock:** the native view may redraw around 30 FPS so transforms can
  advance smoothly.
- **Semantic art clock:** Classic pet art, blink/glitch frame counters, and
  other tick-authored art updates advance at the existing Classic cadence
  (`UI_TICK_INTERVAL_SECS`, currently 250 ms).

`src/companion/app.rs` should keep smooth mode on the faster AppKit timer, but
`animate_pet()` must not call `advance_companion_animation(...)` on every smooth
paint. Instead, the smooth app state should track the next time a Classic art
tick is due. On each paint tick:

1. drain live usage updates as today;
2. if the renderer is Pixel, keep the existing Pixel frame path;
3. if the renderer is Smooth, advance Classic pet art only when the semantic
   art cadence has elapsed;
4. rebuild or redraw the smooth scene every paint tick so continuous transforms
   remain live;
5. if the renderer is Classic, keep the existing 250 ms behavior.

Smooth mode also needs a continuous anchor layer. The current
`round::scene::companion_drift_position(...)` maps fractional wander offsets
into integer `u16` cell coordinates. That remains correct for Classic flattening,
but the smooth renderer should preserve a fractional target anchor before
rounding. The smooth path should expose a pure, testable anchor resolver that
returns:

```rust
pub struct SmoothPetAnchor {
    pub x: f32,
    pub y: f32,
}
```

The resolver should reuse `CompanionMotion`, live motion energy, current time,
grid dimensions, `PET_W`, `PET_H`, breathing offset, clamping, and upward bias
from the Classic companion motion path. Its output should match the Classic
integer anchor when rounded or floored according to the existing flattening
contract, but smooth layers consume the fractional value directly.

In `build_round_smooth_scene_plan(...)`, the pet-body layer should keep using
Classic layer items, but its anchor/translation should be adjusted so AppKit
draws the pet from the fractional smooth anchor. Layers that are visually
attached to the pet should use the same smooth anchor treatment:

- `PetBody`
- `ContactShadow`
- `MoodAura`
- `PerformanceCue`
- `ChestBubble` when present

Other tank layers remain snapped in this slice so habitat props and tank life
stay visually stable while the pet motion is corrected.

## Review Evidence

The current review capture records frame count and bob samples. That was not
enough to catch this bug. The stabilization slice should extend deterministic
and native review evidence with:

- semantic art tick count or pet-art checksum samples;
- per-frame smooth pet anchor samples;
- maximum adjacent-frame anchor delta in smooth Preview Lab strips;
- explicit proof that smooth paint frames outnumber semantic art ticks during a
  bounded capture.

The evidence should remain privacy-safe. It may record checksums, counters,
elapsed milliseconds, layer roles, and anchor coordinates; it must not record
source names, exact token strings, project names, file paths, prompts,
responses, raw diagnostics, or unprojected pet seed values.

## Acceptance Criteria

- Running `glorp companion --renderer smooth` still shows the Classic Glorp pet,
  props, tank life, aura, gauges, HUD, and porthole composition.
- In smooth mode, the pet no longer flashes from over-fast blink/glitch/art
  updates.
- In smooth mode, the pet's tank movement uses continuous sub-cell motion rather
  than visibly jumping between integer cells.
- Smooth bob remains visibly continuous.
- Classic renderer timing and output are unchanged.
- Pixel renderer timing and output are unchanged.
- Preview Lab smooth motion artifacts include anchor and cadence evidence.
- Native smooth review capture proves a high paint-frame count with a lower
  semantic art-tick count.
- Existing smooth parity/privacy tests continue to pass.

## Verification Commands

Focused checks:

```bash
cargo test --test smooth_companion
cargo test --features dev-preview --test dev_preview dev_preview_smooth
cargo test --test round_scene
cargo test --test round_draw_list
cargo test --test cli_smoke companion_ -- --nocapture
```

Preview and native review:

```bash
cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 12000 --review-capture-dir target/glorp-review/smooth-stabilized-active
```

Manual review should confirm that the pet reads as the current Classic Glorp,
that the visible motion is smooth rather than snapped, and that blink/glitch
changes occur at a readable Classic cadence instead of flashing.
