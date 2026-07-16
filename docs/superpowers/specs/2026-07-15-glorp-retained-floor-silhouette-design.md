# Retained Floor Silhouette Design

**Date:** 2026-07-15
**Status:** Approved for implementation
**Scope:** Retained macOS companion renderer only

## Problem

The retained companion currently represents the pet's floor shadow as a soft
radial ellipse. Its center, size, and opacity respond correctly to pet depth,
but its coverage is unrelated to the pet artwork. On the textured tank bed it
reads as a generic circular blur rather than a shadow cast by the visible pet.

The earlier Smooth tank-depth design deliberately chose an art-directed
ellipse. This design amends that choice for the retained renderer now that it
can reuse the exact pet glyph mask. Smooth and Classic remain unchanged.

## Decision

Render the retained floor projection from the exact pet-body glyph mask,
affinely flattened into the existing depth-derived floor rectangle.

The projection will:

- preserve the pet silhouette's asymmetric ears, body, and feet;
- preserve horizontal facing;
- exclude particle-role glyphs;
- remain anchored to the curved bed and track the pet's horizontal center;
- keep the existing far/near size, position, and opacity response;
- remain independent of idle bob and breath;
- multiply-darken the bed below props and tank inhabitants; and
- use existing glyph-atlas antialiasing without a blur pass.

This is a coverage change, not new depth behavior.

## Alternatives

### Tight foot/contact mask

A lower-body-only mask would be appropriate for a standing pet, but Glorp
floats above the bed. It would lose the recognizable cast shape and repeat an
older contact-shadow model that was removed when the pet moved higher in the
tank.

### Analytic capsules or a tuned ellipse

Analytic approximations are cheaper but still generic. They cannot preserve
species, stage, facing, or asymmetric artwork and therefore do not satisfy the
visual requirement.

### Render-to-mask blur pass

A separate mask texture and blur would provide soft penumbra control, but it
adds targets, passes, resource lifetime, and resize work. The glyph atlas
already provides sufficient edge antialiasing at companion sizes, so this is
unnecessary.

## Scene Contract

`FloorProjection` remains analytic semantic slot 2 and keeps its existing node,
authored order, world depth, and multiply blend. Its typed shape changes from a
radial ellipse to a projected pet silhouette.

The frame contract carries:

- the existing destination rectangle derived by `floor_projection_metrics`;
- the pet-body mask source;
- the horizontal facing sign; and
- no live bob or breath displacement.

The paint contract keeps the existing biome-derived multiply color and the
existing depth-derived effective opacity. Radial inner/outer falloff is removed
because coverage now comes from the pet mask.

## Retained GPU Path

The compiler routes floor semantic slot 2 to a new floor-shadow glyph-mask
source, alongside the existing rear-wall mask source. Both reuse the fixed 130
pet-body content records and the prepared pet atlas; no new GPU buffer or scene
resource is introduced.

The floor draw uses a multiply glyph-mask pipeline:

1. Map each occupied 13x10 pet cell into the destination floor rectangle.
2. Apply horizontal facing within that rectangle.
3. Scale glyph ink anisotropically so the complete silhouette is flattened,
   rather than shrinking every glyph uniformly to the rectangle height.
4. Sample exact monochrome or color-glyph alpha coverage from the pet atlas.
5. Multiply the bed by the authored floor-shadow paint at the existing
   depth/lifecycle opacity.

The destination rectangle remains in bed point space, so the shadow follows
depth travel but does not inherit the pet body's bob, breath, or vertical
transform.

## Ordering And Failure Behavior

The draw remains after the opaque room/background and before ambient marks,
props, tank inhabitants, the rear-wall shadow, and the pet body. Existing
aperture and depth behavior remain authoritative.

Invalid mask, geometry, facing, atlas, or pipeline combinations fail closed
through the existing scene validation and materialization errors. There is no
fallback to the radial ellipse.

## Verification

Add one controlled native GPU readback using an asymmetric pet mask. Render the
room plus floor shadow, then render the room alone, and assert that:

- occupied left/right mask regions darken in the expected asymmetric pattern;
- empty cells inside the former ellipse bounds remain unchanged;
- the shadow is vertically flattened into the bed rectangle;
- effective opacity remains bounded by the existing depth-derived contract;
- the draw uses multiply blending and remains before props; and
- changing facing mirrors the projected silhouette without moving its center.

Update scene/compiler/validator tests that currently require slot 2 to be a
`RadialEllipse`. Preserve existing far, neutral, near, active, calm, asleep,
no-bob, ordering, checksum, and fixed-capacity coverage.

Use the retained round full-cast preview for visual QA after the focused test is
green. Do not change Smooth radial-parity tests.

## Acceptance Criteria

- The bottom shadow visibly follows the current pet silhouette rather than a
  circle or oval.
- It stays centered and bed-anchored while the pet moves through depth.
- Facing mirrors the shadow; particles do not affect it.
- Resizing and fullscreen transitions require no new resources or passes.
- Smooth and Classic output is unchanged.
- Relevant focused tests, all-feature library tests, clippy, and formatting
  pass before commit.

## Non-Goals

- Physically simulated lighting or configurable light direction.
- Penumbra blur, render-to-texture masks, or multi-pass post-processing.
- Changes to prop contact shadows, rear-wall tint, tank-life shadows, Smooth,
  or Classic.
