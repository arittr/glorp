# Glorp Retained Habitat Grounding Design

**Date:** 2026-07-15
**Status:** Approved
**Surface:** Round companion habitat, with retained rendering as the visual target

## Goal

Finish the habitat's environmental depth cues without adding renderer
architecture. Ground vegetation must remain fully visible and distributed across
the floor depth, foreground vines must visibly attach to the tank ceiling, and
the substrate must read as a quiet granular surface instead of a flat gradient.

## Problems And Root Causes

### Ground vegetation crowds the aperture edge

Grounded props first try horizontal candidates beside the HUD. When those
positions compete, the current fallback offsets expand left props farther left
and right props farther right. Grounded safety is checked against the full
aperture rather than the visible area inside the gauge, so a mathematically valid
prop can still be covered or visually clipped by the circular rim.

### Foreground vines do not meet the ceiling

Ceiling props choose among fixed grid rows and require their entire rectangular
footprint to fit inside the gauge-safe ellipse. The highest accepted row is
therefore not the visible ceiling at the prop's horizontal position. The vine's
stem ends in open water and reads as floating.

### The substrate texture is not perceptible

The retained room shader currently adds physical-pixel dithering and isolated
single-pixel flecks. Native tests can measure the resulting variance, but the
pattern is too fine, sparse, and low-contrast to survive normal viewing distance
or the darker external display.

### Vegetation occupies only the near floor

Moss and reeds are both authored as foreground props, so the depth-lane solver
places both at the near floor contact. This leaves no plant landmark on the rear
substrate and weakens the 2.5D read.

## Decision

Keep the current composition solver, analytic room pass, prop catalog, and
retained GPU resources. Refine their contracts in four focused ways.

### Inset grounded horizontal lanes

Grounded candidates use the point-space circular aperture projected into the
logical cell grid, with one logical cell of visual inset. This matters on tall
or landscape surfaces, where the circle does not occupy both full grid axes.
They do not use the gauge's inner ellipse because the gauge intentionally opens
across the bottom substrate. Candidate fallbacks move inward from the HUD side
lanes rather than expanding toward the circular edge.

- Left-side fallback positions progress toward the center.
- Right-side fallback positions progress toward the center.
- Middle-zone props may use either side of the floor HUD, but every candidate
  keeps the same inset.
- Existing rear, middle, and near lane offsets remain authoritative. On the
  canonical square 18-row grid their exclusive contacts remain `15`, `16`, and
  `17`; non-square surfaces derive equivalent contacts from the actual circular
  floor so a prop remains grounded instead of following the rectangular window
  bottom.
- Existing HUD exclusion, prop gutters, collision checks, and deterministic
  placement remain authoritative.
- If no inset candidate is safe, the lower-priority prop is hidden instead of
  being clipped or moved to another depth lane.

The visible glyph footprint, not merely its anchor, must remain inside the
inset. Moss and reeds are acceptance fixtures across square, tall, and landscape
surfaces. Resize keeps each prop in the same named depth lane while recomputing
the lane's physical contact from the current circular aperture.

### Reeds become the rear vegetation anchor

The existing reeds prop changes from foreground to background authored depth.
It is not duplicated and no second catalog reward is introduced. The shared
authored depth keeps the reeds behind the pet and assigns them to the rear floor
contact, while moss remains foreground on the near contact.

This intentionally changes the reeds' depth presentation anywhere that consumes
the shared habitat prop contract. Its identity, unlock threshold, art,
animation, color, zone, and display priority do not change.

### Ceiling attachment follows the circular contour

Foreground ceiling props resolve a contact point from the circular aperture at
their accepted horizontal position rather than choosing a nominal top row. The
vine's topmost occupied stem cell meets that contour and may pass beneath the
gauge, so the gauge masks the attachment naturally. The remaining occupied vine
cells stay inside the aperture and outside the HUD.

Safety uses occupied prop cells for this attachment check rather than rejecting
the vine because empty corners of its rectangular footprint cross the curved
boundary. Background ceiling props retain their recessed placement and do not
gain the foreground attachment behavior.

The contact is recomputed from logical surface geometry after resize,
fullscreen, or display-scale changes; it does not depend on the previous frame
or window size.

### Logical-scale granular substrate

The existing room-aperture fragment shader continues to own the bed. Replace
the imperceptible physical-pixel-only pattern with deterministic texture in
logical tank coordinates:

- broad, low-amplitude tonal variation breaks up the smooth gradient;
- small clustered pebble/grain marks create readable local texture;
- sparse larger flecks provide occasional landmarks without resembling props;
- biome-derived bed and fleck colors preserve the current habitat palette; and
- texture contrast ramps in below the curved horizon and remains absent from
  the upper room.

The pattern is stable across frames and backing scales. Resize may reveal more
or less of the logical field, but an unchanged logical point keeps the same
texture value. The texture does not use time, animation phase, pet state, or
scene revision.

The result should be visible at normal companion size on the external display
while remaining quieter than the HUD, pet, props, and floor projection.

## Renderer And Resource Constraints

- No new render pass, texture asset, GPU buffer, pipeline, or resize-owned
  resource.
- No perspective camera or mesh floor.
- No dynamic per-frame prop packing.
- The retained room remains one analytic aperture draw.
- Existing prop nodes, Z ordering, contact shadows, saturation, and parallax
  remain in use.
- The pet floor silhouette remains unchanged.

## Verification

Add focused failing coverage before implementation:

1. Moss and reeds keep at least a one-cell visible margin inside the circular
   floor aperture on supported square, tall, and landscape surfaces while the
   gauge's open bottom remains usable.
2. Same-lane competition tries inward candidates and hides on exhaustion rather
   than expanding outward.
3. Moss resolves to the near contact and reeds resolve to the rear contact,
   with both grounded and the reeds behind the pet.
4. A foreground vine's topmost occupied stem cell contacts the circular ceiling
   at its chosen horizontal position across supported surfaces.
5. Background ceiling props remain recessed and non-ceiling zones retain their
   existing attachment behavior.
6. CPU texture samples are deterministic in logical coordinates and invariant
   to backing scale.
7. Native retained readback shows structured lower-bed contrast at a scale
   larger than isolated pixels, while the upper room remains free of substrate
   marks.
8. Existing composition, retained-scene, round-scene, Preview Lab, formatting,
   clippy, and all-feature library checks pass.

Human QA in the optimized companion must confirm that the props remain inset
during resize/fullscreen, the vine visually meets the top boundary, rear reeds
read behind near moss, and the ground texture is visible without becoming
noisy.

## Non-Goals

- New prop art, duplicated reeds, or additional habitat rewards.
- Literal soil photography, moss carpeting, sprite textures, or procedural
  terrain simulation.
- Changes to inventory selection or reward progression.
- Perspective scaling, pivot-aware prop scaling, or a broader scene-graph
  refactor.
- Changes to the floor silhouette or generic prop contact-shadow design.
