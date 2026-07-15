# Glorp Retained Prop Depth Lanes Design

**Date:** 2026-07-15
**Status:** Approved
**Surface:** macOS round companion using the direct retained scene runtime

## Goal

Use the retained renderer's existing 2.5D depth semantics to keep grounded props
from reading as one flat lineup. Preserve every prop's authored attachment zone:
floor props remain visibly planted on the substrate, ceiling props remain at the
top, wall props remain against their side, and air props remain in the interior.

This extends the existing habitat composition solver. It is not a renderer or
shader redesign.

## Root Cause

Retained prop nodes already have real world Z for occlusion plus authored
parallax, opacity, saturation, and contact-shadow strength. The orthographic
camera does not turn Z into screen-space position or scale, however, and every
floor candidate currently uses the same `End { offset: -1 }` contact row.
Consequently all grounded props read as if they occupy the near edge of the
floor regardless of their authored depth.

## Locked Design

### Attachment zones remain authoritative

`PropZoneSnapshot` continues to decide the physical surface available to a
prop. Depth placement may choose among candidates inside that surface, but may
not reclassify the prop or detach it:

- floor zones use a bottom contact row on the visible substrate;
- ceiling candidates retain a top contact row;
- wall candidates retain their left or right boundary relationship; and
- air candidates remain inside the aperture and outside floor/ceiling lanes.

The first implementation changes floor candidates only. Non-floor candidate
geometry remains unchanged, with regression coverage proving those attachment
contracts continue to hold.

### Three deterministic floor lanes

Grounded candidates gain rear, middle, and near contact lanes. On the canonical
18-row companion grid their exclusive bottom bounds are rows 15, 16, and 17,
respectively. Equivalent offsets are derived from the current row count so the
contract scales with other supported logical grids.

Every accepted floor footprint therefore ends on a defined floor contact row;
moving a prop toward the rear never makes it float. The active sprite remains
bottom-aligned within its frozen maximum footprint, and its contact shadow is
derived from the same projected origin and footprint.

### Stable lane assignment

Authored depth is authoritative for accepted floor contact, not a replacement for
draw ordering:

- `Background` is fixed to rear;
- `BehindPet` is fixed to rear or middle by a catalog-ID-stable discriminator;
  and
- `Foreground` is fixed to near.

The discriminator applies only to `BehindPet`. It must not use `stable_order`,
current visible inventory length, animation phase, time, or viewport size, so
earning, hiding, or reordering another prop cannot move an existing prop between
depth lanes.

Within the assigned lane, grounded props first try a bounded set of horizontal
contacts beside the floor HUD: left zones use offsets `0`, `-2`, and `-4` from
the HUD's left edge, right zones use `0`, `+2`, and `+4` from its right edge,
and middle zones interleave both sequences. The existing zone-specific
horizontal candidates follow. The current aperture, HUD, gauge, gutter, and
collision checks remain authoritative for every candidate. If every horizontal
candidate in that lane is unavailable, the solver hides the prop. It never
falls through to another depth lane or stacks footprints. This final-review
correction resolves packing versus resize stability in favor of stable
anchoring while keeping a grounded `BehindPet` prop available in the full cast.

### Existing Z and depth cues remain intact

The implementation does not change `AuthoredDepthSnapshot`, scene-node Z,
parent layers, parallax multipliers, opacity, saturation, or shadow strength.
Screen-space floor contact provides the missing perspective cue while existing
Z continues to own occlusion.

Do not change `DepthCue::scale` in this pass. The current shader scales absolute
glyph coordinates around the scene origin, so non-unit scaling would also slide
props. Pivot-aware perspective scaling is a separate follow-up only if lane
placement still looks too flat.

## Resize And Animation Invariants

- The same catalog prop keeps the same assigned lane across resize,
  fullscreen, display changes, animation phases, and semantic refreshes.
- Resize may change horizontal acceptance because the circular aperture changes,
  but may not detach a prop from its authored surface.
- A sprite phase with a shorter active footprint remains bottom-aligned to the
  accepted floor contact row.
- Contact shadows move with grounded props and remain absent for non-grounded
  props.
- Foreground tank routing reservations use the final accepted, lane-adjusted
  bounds.

## Verification

Add focused composition tests before implementation to prove:

1. a representative set of grounded props occupies more than one contact lane;
2. every grounded footprint ends on one of the valid floor contact rows;
3. authored background props remain rear, foreground props remain near, and only
   behind-pet props use the rear/middle catalog discriminator;
4. lane choice is catalog-stable across inventory ordering and supported surface
   sizes;
5. floor sprites remain bottom-aligned through active animation footprints;
6. ceiling, wall, and air fixtures retain their attachment-zone bounds;
7. same-lane competition hides a prop after horizontal exhaustion instead of
   changing its accepted contact lane;
8. the full cast keeps a grounded behind-pet prop visible alongside its existing
   foreground props without losing the one-cell gutter;
9. no prop overlaps the HUD, gauges, aperture, or another accepted footprint;
   and
10. foreground tank reservations follow the final moved prop bounds.

Then run the existing companion-scene, retained-scene, and Preview Lab prop
checks. Human review should confirm that the floor reads as rear/middle/near
without any prop appearing suspended.

## Non-Goals

- No perspective camera, mesh floor, shader pivot work, or per-prop scale.
- No dynamic layout balancing or per-frame repacking.
- No changes to inventory selection, prop art, tank routes, or non-retained
  renderers.
- No weakening of collision, HUD, aperture, or gauge exclusions to keep more
  props visible.
