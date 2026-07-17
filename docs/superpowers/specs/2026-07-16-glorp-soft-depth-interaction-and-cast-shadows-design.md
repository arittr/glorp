# Glorp Soft Depth Interaction and Cast Shadows Design

**Date:** 2026-07-16

**Status:** Approved direction, written for implementation planning

**Builds on:** `2026-07-16-glorp-lenticular-hud-depth-layers-design.md`

## Summary

The statistics will read as a translucent emissive display suspended at the
existing `+0.72` tank plane, not as solid lettering. The pet will keep the
existing behind/in-front depth contract, but the visible overlap will ease
through a short interaction band immediately before the crossing instead of
changing as one hard planar cut.

Large, elevated, visually solid habitat props will gain directional cast
shadows on their receiving surface. Small, flat, translucent, or emissive
elements will not. Existing contact shadows remain the grounding cue for props
that do not qualify for a cast shadow.

## Problem

The current depth order is geometrically correct, but the pet and statistics
are each effectively flat planes. When the pet crosses the statistics at
`+0.72`, their whole-object order changes at once. That reads as two layers
swapping rather than a creature moving through a luminous volume.

Adding an ordinary drop shadow to the statistics would imply solid acrylic
lettering that the pet somehow passes through. The better material model is a
translucent display volume with a soft local interaction at the overlap.

Props have the opposite problem. The retained scene already authors grounded
contact ellipses, but tall solid props do not project their height into a cast
shadow. On the lenticular display that omission makes substantial props feel
flat even when their Z placement is correct.

## Goals

- Remove the visible pop when the pet crosses the statistics plane.
- Keep statistics fixed at Z `+0.72` with unchanged text, layout, formatting,
  color, and sealed-value privacy.
- Preserve the pet's current motion, scale, endpoint geometry, and exact
  behind/front depth decision.
- Change only pixels where pet-front coverage overlaps statistics glyph
  coverage; statistics must not pulse or fade globally.
- Give qualifying solid props a soft directional cast shadow whose length and
  offset communicate height.
- Keep contact shadows for grounded props and receiving-surface shadows behind
  the pet/statistics/gauge composition.
- Match the Smooth/AppKit and retained depth-aware renderers.

## Non-goals

- Do not make the statistics solid or collision-bearing.
- Do not add a traditional statistics drop shadow.
- Do not change the statistics plane, gauge planes, pet traversal, or gauge
  styling.
- Do not add dynamic collision avoidance between the pet and HUD.
- Do not make every mote, particle, decal, plant tip, or emissive mark cast a
  shadow.
- Do not add general-purpose real-time shadow mapping, new lights, multiview
  synthesis, head tracking, quilt output, or display calibration.
- Do not change Pixel or Classic beyond preserving their functional flat
  fallback.

## Statistics Material and Interaction

### Material model

The statistics are a thin translucent emissive volume. Their primary glyphs
retain the current color and typography. A faint, low-opacity rear echo is
placed a small fixed distance behind the glyph plane to communicate thickness
on the lenticular display. The echo is a blurred/tinted emission echo, not a
multiply shadow, and never moves with the pet.

The echo must remain subordinate to the text: it cannot create a second
readable copy, increase the text footprint enough to collide with the gauges,
or expose live HUD values through a new artifact or scene snapshot.

### Soft interaction band

The geometric statistics plane remains `+0.72`. A one-sided visual interaction
band occupies effective pet Z `[+0.64, +0.72]`:

- at or behind `+0.64`, the statistics fully cover overlapping pet ink;
- between `+0.64` and `+0.72`, the pet-front group is progressively revealed
  through only the covered statistics glyph pixels using smoothstep easing;
- at `+0.72`, the overlap visually matches the fully-front result;
- above `+0.72`, the existing front-of-statistics order applies unchanged.

This band is deterministic and depends only on lifecycle-adjusted effective
pet Z. It adds no temporal state or hysteresis. Reverse travel follows the same
curve.

The crossing group remains exactly pet body, particles/performance cue, and
mood aura. Wall shadow and floor projection never enter the interaction mask.

### Overlap-only compositing

The soft reveal is clipped to the intersection of:

1. private statistics glyph coverage; and
2. the pet-front group's rendered coverage.

Outside that intersection, the frame is byte-identical to the existing depth
composition. In particular, the statistics do not globally dim when the pet is
near but elsewhere in the tank.

The overlap mask is renderer-private, fixed-capacity, and ephemeral. It must
not serialize glyph values, strings, atlas indices, or coverage pixels into
scene contracts, checksums, preview artifacts, logs, or diagnostics.

## Prop Shadow Qualification

### Authored shadow profile

Shadow eligibility is authored with prop presentation topology rather than
guessed only from a runtime bounding box:

```text
None          translucent, emissive, suspended, or visually weightless
ContactOnly   grounded but too small/flat to project meaningful height
Elevated      grounded, solid, and tall enough for a directional cast shadow
```

An `Elevated` profile carries authored visual height and softness. Runtime
projection still suppresses the cast shadow when the prop is invisible,
ungrounded, fully transparent, or projects below the minimum useful footprint.
The minimum projected size is one cell wide and two cells high; smaller output
falls back to contact-only so tiny shadows do not create noise.

### Cast-shadow geometry

Qualifying prop shadows project from the prop's grounded footprint away from
the existing scene light direction. Shadow length is proportional to authored
height and light elevation, with a conservative clamp so it remains inside the
tank bed/receiving surface. Softness increases with projection distance and
opacity decreases with distance.

The cast shadow uses multiply blending with the existing biome-derived shadow
tint. Multiple prop shadows union by maximum coverage rather than repeatedly
darkening overlaps. The original contact ellipse remains underneath as the
short-range grounding core.

Cast shadows are receiving-surface content. They do not inherit the prop's Z,
do not cross the statistics or gauges, and do not shadow the pet or HUD in this
slice.

## Architecture

### Shared statistics interaction contract

The round depth domain adds a renderer-neutral prepared result containing:

- effective pet Z;
- interaction start Z `+0.64`;
- statistics plane Z `+0.72`;
- smoothstep reveal mix in `[0, 1]`; and
- the existing pet-versus-statistics order.

Render callbacks consume this result and do not derive thresholds from screen
Y, scale, raw depth, or wall/floor shadow state.

### Smooth/AppKit

AppKit keeps the existing prepared pass schedule. During the interaction band,
it reuses the prepared pet-front group through a private statistics coverage
clip with the prepared reveal mix. The main statistics draw and all text layout
remain unchanged. The coverage mask and reveal inputs are prepared outside the
native paint callback; the callback performs no sorting or semantic inference.

The rear emission echo is painted with the statistics pass. Prop cast shadows
are added to the existing receiving-surface shadow pass before props and pet.

### Retained renderer

The retained renderer keeps the sealed statistics world marker and its private
HUD resource. Its HUD preparation produces a private fixed-size glyph coverage
mask/stencil alongside the existing fixed-capacity records. During the
interaction band, the retained world encoder draws the prepared pet-front group
through that coverage clip after the sealed HUD pass using the reveal mix.
Above the plane, the normal sorted front order remains authoritative.

The interaction draw is not a second scene primitive or a second statistics
record. It is a private compositor operation tied to the single sealed HUD
marker, so scene inventory, semantic IDs, checksums, and HUD privacy remain
stable.

The existing `PropShadows` analytic gains the authored elevated-shadow inputs
and evaluates contact plus directional coverage in one receiving-surface draw.
No general shadow texture, extra light, or unbounded draw list is introduced.

## Validation and Failure Behavior

- Interaction thresholds must be finite and satisfy
  `PET_MIN_Z < +0.64 < +0.72 < PET_MAX_Z`.
- Reveal mix must be finite and within `[0, 1]`.
- Missing or invalid private statistics coverage fails frame preparation; the
  renderer does not fall back to globally fading the statistics.
- A prop cannot emit a cast shadow unless it is visible, grounded, solid,
  sufficiently large, and has an `Elevated` profile.
- Invalid authored height, softness, opacity, or projected geometry fails scene
  validation instead of being silently repaired in the shader.
- Pixel and Classic ignore the interaction/cast-shadow additions and keep their
  current fallback behavior.

## Testing

- Pure depth tests cover reveal mix at `+0.64`, interior samples, `+0.72`, just
  above the plane, asleep effective depth, and reverse travel equivalence.
- AppKit schedule tests prove only the pet-front group enters the statistics
  clip and wall/floor shadows never do.
- Retained native tests compare frames immediately around both band boundaries
  and prove the interaction is continuous at overlapping glyph pixels.
- Non-overlapping statistics pixels and all pixels outside the statistics mask
  remain unchanged across the interaction band.
- HUD privacy, fixed-capacity, redaction, and single-marker tests remain green.
- Prop tests cover `None`, `ContactOnly`, and `Elevated`, including projected
  size suppression, invisible/ungrounded suppression, light-direction offset,
  height-scaled length, soft falloff, and overlap union behavior.
- Smooth/AppKit and retained snapshots agree on reveal mix and qualifying prop
  shadow geometry.

## Acceptance Criteria

1. The pet/statistics overlap no longer pops when effective Z crosses `+0.72`.
2. Only overlapping pet/statistics pixels change inside the interaction band;
   the statistics never fade globally.
3. Pet body, performance cue/particles, and mood aura transition together.
4. Wall and floor shadows remain on their receiving surfaces.
5. Statistics layout, content, plane, privacy, and single-draw contract remain
   unchanged.
6. The rear emission echo reads as depth but not as a second text copy or solid
   drop shadow.
7. Large/tall solid props cast soft directional receiving-surface shadows;
   small, flat, translucent, or emissive props do not.
8. Contact shadows remain present and directional shadows do not repeatedly
   over-darken where they overlap.
9. Gauge planes, status/trouble chrome, dimming, and pet traversal are unchanged.
10. Smooth/AppKit and retained produce the same visual transition and prop
    shadow qualification; Pixel and Classic remain functional fallbacks.
