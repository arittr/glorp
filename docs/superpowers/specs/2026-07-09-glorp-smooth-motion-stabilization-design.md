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
   the 250 ms UI tick. As a result, `AnimationFrame.tick` consumers such as
   blink, glitch, idle gestures, particles, and art-frame changes run much too
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
timer callback. Instead, the smooth app state should track the next time a
Classic art tick is due. On each UI timer tick:

1. drain live usage updates as today;
2. if the renderer is Pixel, keep the existing Pixel frame path;
3. if the renderer is Smooth, advance Classic pet art only when the semantic
   art cadence has elapsed;
4. request a redraw on each fast UI timer tick so continuous transforms remain
   live;
5. if the renderer is Classic, keep the existing 250 ms behavior.

The semantic art clock must use monotonic `Instant` timing, advance at most one
Classic art tick per UI timer callback, and drop missed intervals after a
run-loop stall, resize, wake, or resume. It must never loop through missed
250 ms intervals to "catch up", because catch-up loops would compress Classic
art changes back into visible flashing.

`drawRect` / `draw_scene(...)` stays render-only. It may read the latest
snapshot and compute continuous elapsed-time transforms for the frame it is
drawing, but it must not drain live updates, advance semantic art ticks, mutate
`animation_frame`, or depend on AppKit's redraw coalescing for state progress.

Smooth mode also needs a continuous anchor layer. The current
`round::scene::companion_drift_position(...)` maps fractional wander offsets
into integer `u16` cell coordinates. That remains correct for Classic flattening,
but the smooth renderer should preserve a fractional target anchor before
snapping. The placement resolver belongs in the shared, cfg-free round layout
seam beside the current Classic drift code, not as Smooth-only logic hidden in
`build_round_smooth_scene_plan(...)`.

The shared resolver should return both the continuous placement and the exact
Classic snapped placement:

```rust
pub struct CompanionPetPlacement {
    pub fractional_top_left: SmoothPetAnchor,
    pub classic_snap_top_left: (u16, u16),
    pub classic_rect: ratatui::layout::Rect,
}

pub struct SmoothPetAnchor {
    pub x: f32,
    pub y: f32,
}
```

The resolver should reuse `CompanionMotion`, live motion energy, current time,
grid dimensions, `PET_W`, `PET_H`, breathing offset, clamping, and upward bias
from the Classic companion motion path.

Classic parity is exact, not approximate. `classic_snap_top_left` must reproduce
today's `companion_drift_position(...)` contract: truncate each motion term
toward zero with Rust's `as i32` behavior, add those terms to the integer base,
then clamp to the grid. It must not be derived by applying `floor`, `round`, or
another single rounding operation to the composite fractional anchor, because
that can diverge from Classic by whole cells for negative offsets or biased Y
motion.

Classic flattening continues to use `classic_rect` / snapped anchors. Smooth
native renderers consume `fractional_top_left` or the fractional residual from
`classic_snap_top_left` so parity tests can stay exact while native AppKit draws
sub-cell motion.

In `build_round_smooth_scene_plan(...)`, the pet-body layer should keep using
Classic layer items, but its anchor/translation should be adjusted so AppKit
draws the pet from the fractional smooth anchor. Layers that are visually
attached to the pet should use the same smooth anchor treatment:

- `PetBody`
- `ContactShadow`
- `PerformanceCue`

`MoodAura` must use the same fractional pet center as the smooth pet body, but
the implementation can satisfy that either by adding fractional pet center/bounds
to `SmoothCompanionPet` and consuming them in AppKit, or by turning the aura into
an actual shape layer that the smooth renderer honors.

Other tank layers remain snapped in this slice so habitat props and tank life
stay visually stable while the pet motion is corrected. `ChestBubble` is
prop-attached, not pet-attached, and remains snapped with the treasure chest in
this slice.

## Review Evidence

The current review capture records frame count and bob samples. That was not
enough to catch this bug. The stabilization slice should extend deterministic
and native review evidence with:

- `paint_frame_count`;
- `semantic_art_tick_count`;
- per-frame `semantic_art_tick_index`;
- per-frame pet visual checksums covering both `pet_art` and `pet_spans`;
- per-frame `base_anchor`, `bob_offset`, `final_anchor`, and
  `classic_snap_anchor`;
- maximum adjacent-frame deltas for `base_anchor` and `final_anchor`;
- explicit proof that smooth paint frames outnumber semantic art ticks during a
  bounded capture.

The flashing proof must be mechanical: multiple paint frames inside the same
250 ms semantic bucket must share the same pet visual checksum, and pet visual
checksums may change only on semantic art ticks.

The jumping proof must separate tank motion from bob motion. Bob alone cannot
make the evidence pass. Preview Lab smooth motion strips must advance both
paint elapsed time and deterministic `now` values, and the fixture sequence must
be chosen so `classic_snap_anchor` changes at least once while adjacent
`base_anchor` / `final_anchor` deltas remain bounded. At 30 FPS review cadence,
adjacent smooth anchor deltas should stay below one cell unless the pet identity,
viewport, or review state changes. Pet-attached roles must report the same
continuous anchor source.

The evidence should remain privacy-safe. It may record checksums, counters,
elapsed milliseconds, layer roles, and anchor coordinates; it must not record
source names, exact token strings, project names, file paths, prompts,
responses, raw diagnostics, or unprojected pet seed values.

Every new JSON evidence artifact, including native `render-log.json`, must
include explicit privacy claims and must be covered by an automated scan of all
textual fields. Native screenshots are allowed only through the existing
redacted review-capture HUD path; screenshot privacy remains a manual visual
review item because pixel data is not meaningfully covered by the JSON scanner.

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
- Pet visual checksums are stable across multiple paint frames within one
  semantic art tick and change only on semantic art tick boundaries.
- Preview Lab smooth strips advance deterministic `now` as well as elapsed
  paint time, prove that bob is not the only changing anchor component, and
  prove that Classic snap parity remains exact.
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
