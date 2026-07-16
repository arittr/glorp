# Glorp Lenticular HUD Depth Layers Design

**Date:** 2026-07-16

**Status:** Approved direction, written for implementation planning

**Builds on:** `2026-07-16-glorp-full-tank-depth-traversal-design.md`

## Summary

The round companion will stop treating the central statistics and perimeter
gauges as one flat front-glass overlay. The three central statistics lines will
sit on a fixed plane inside the tank. The pet will render behind that plane for
most of its rear-to-front travel and cross in front of it only during the last
part of its approach to the glass. The three gauge lanes will become a shallow,
staggered bezel stack in front of every reachable pet position.

This composition is designed for the lenticular display: depth is represented
as renderer-neutral scene Z and occlusion semantics, not merely as a coincidental
call order. The existing single-view AppKit path will reproduce the same
occlusion with ordered passes. Adding multiview synthesis or hardware-specific
lenticular output is outside this slice.

## Problem

The full-tank traversal work lets the pet reach the near glass, but the current
HUD contract still has only `FrontGlass`. In the retained scene, gauges, status,
trouble, statistics, and dimming are all screen chrome with no depth. In AppKit,
the full scene is painted first and all gauges and statistics are painted over
it. That makes the pet's new forward range geometrically correct but visually
flatter than the tank scene.

The perimeter lanes already suggest depth because their radii and stroke widths
form nested rings. Treating those rings as a stepped bezel and letting the pet
cross the statistics plane turns that suggestion into a coherent scene without
moving or redesigning the existing UI.

## Goals

- Put the central token total, yesterday percentage, and ten-minute pace on one
  fixed plane inside the tank.
- Let the pet-front group (body, particles/performance cue, and mood aura)
  occlude those glyphs only when the pet is in the final portion of its forward
  travel.
- Preserve the current pet travel envelope, HUD position, typography, gauge
  geometry, gauge colors, and gauge values.
- Give the XP, daily, and pace lanes distinct ordered Z planes that read as a
  shallow outer-to-inner bezel.
- Keep all gauge lanes in front of the pet and foreground habitat content so
  they remain legible and continue to frame the aperture.
- Make Smooth/AppKit and the retained renderer consume the same semantic depth
  contract and produce the same occlusion decision.
- Keep projected wall and floor shadows on their receiving surfaces; neither
  shadow may cross the statistics plane with the pet.

## Non-goals

- Do not move the statistics stack or change its type scale, line spacing,
  content, colors, formatting, or bottom-gap fit rules.
- Do not change the pet's rear, neutral, or front placement or its depth motion.
- Do not add a background plate, blur, outline, or readability treatment behind
  the statistics.
- Do not redesign the gauge arcs or change their progress/rollover behavior.
- Do not add head tracking, multiview rendering, quilt generation, stereo camera
  synthesis, or display-specific calibration.
- Do not retrofit depth into the Pixel or Classic fallback renderers. They
  retain their existing flat front-glass statistics behavior; Smooth/AppKit and
  retained are the depth-aware acceptance paths.
- Do not refactor unrelated scene phases or renderer infrastructure.

## Depth Contract

### Central statistics plane

The statistics plane is fixed at effective scene depth `+0.72`, where `-1.0` is
the rear wall, `0.0` is neutral, and `+1.0` is the pet's nearest reachable
center plane.

This value is intentional. On the production 18-row round grid, the current
placement mapping puts the neutral pet center at row `9.0` and the front center
at row `13.14`. Effective depth `+0.72` maps to approximately row `11.98`, a
little over one row behind the front center. The pet therefore spends most of
its movement behind the statistics and crosses them only near the glass.

The crossing rule uses the lifecycle-adjusted effective depth, not raw motion
depth:

- `pet_effective_z <= +0.72`: the pet-front group is behind the statistics;
- `pet_effective_z > +0.72`: the pet-front group is in front of the statistics.

Equality deliberately leaves the statistics in front. This gives the crossing
a single deterministic boundary without a temporal hysteresis state.

The statistics remain a screen-aligned billboard: their projected X/Y layout is
unchanged, but their scene transform carries the fixed Z plane. They do not
scale with the pet and do not inherit pet motion.

### Gauge bezel planes

The gauge lanes use fixed world-space Z planes inside the retained camera's
existing `[-2.0, +2.0]` range:

| Lane | Visual position | Scene Z |
| --- | --- | ---: |
| Pace (orange, inner) | deepest bezel step | `+1.55` |
| Daily (green, middle) | middle bezel step | `+1.65` |
| XP (violet, outer) | frontmost bezel step | `+1.75` |

All three values are in front of the pet's maximum `+1.0` depth and the current
foreground habitat planes, while retaining at least `0.25` of clip-space margin
before the `+2.0` near plane. Track, fill, daily rollover, and overage marks for
one lane share that lane's Z. Their existing authored order remains the tie
breaker within the lane.

The outer lane is frontmost because the physical reading is a bezel that rises
toward the aperture edge. The offsets are deliberately shallow: they should
produce separation without making the rings look detached from one another.

### Other chrome and effects

- Status and trouble indicators remain front-glass screen chrome.
- The sleep/calm dim treatment remains the final full-frame operation.
- Pet body, pet particles/performance cues, and mood aura use the pet's effective
  composite depth and cross the statistics together.
- Wall shadow remains on the rear receiving surface.
- Floor projection remains on the tank bed.
- Foreground props and tank inhabitants keep their existing depth relative to
  the pet and statistics.

The resulting back-to-front composition is:

```text
rear room and receiving-surface shadows
behind habitat content
pet-front group when pet_effective_z <= 0.72
central statistics at z = 0.72
pet-front group when pet_effective_z > 0.72
foreground habitat content
pace gauge at z = 1.55
daily gauge at z = 1.65
XP gauge at z = 1.75
status and trouble chrome
dim treatment
```

## Architecture

### Shared prepared composition

The round scene domain will expose a small renderer-neutral composition result
containing:

- the lifecycle-adjusted pet effective Z;
- the fixed statistics plane;
- the three fixed gauge planes; and
- the derived pet-versus-statistics ordering.

This result is prepared with the frame. Renderers consume it; they do not
recalculate the `0.72` threshold or infer depth from the pet's screen Y, scale,
or raw motion sample.

The existing `CompanionHudDepthPlane` seam will be split by responsibility. A
statistics plane describes the three central text lines. Gauge lane planes
describe the bezel. The term “HUD plane” will no longer imply that text and all
perimeter chrome must share one compositing phase.

```text
raw pet depth + lifecycle
            |
            v
    effective pet depth -------> existing placement and depth cues
            |
            v
 shared depth composition <----- fixed stats and gauge planes
       |                 |
       v                 v
 AppKit pass plan   retained world/chrome phases
```

### Retained renderer

The statistics move from the sealed screen-chrome schedule into a dedicated
world-space HUD draw that remains backed by the existing fixed-capacity HUD
atlas and staging buffer. It uses source-over blending, reads world depth, does
not write depth, and participates in the existing transparent back-to-front
order at Z `+0.72`. This is a new scheduled use of the sealed HUD resource, not
a generic arbitrary-text primitive.

The pet node's compositing Z uses effective depth so renderer ordering matches
the lifecycle-adjusted placement, scale, atmosphere, and shadow cues. Pet body
and attached particles inherit that node. Mood aura receives the same composite
depth even though its absolute point geometry remains outside the pet transform.

The single `chrome.gauges` primitive becomes three lane primitives/nodes. Each
lane keeps the existing analytic gauge geometry and paint data but uses its
fixed world-space Z. The retained transparent sorter then supplies stable
back-to-front ordering for the statistics, pet, foreground content, and gauges.
Status, trouble, and dim remain in the no-depth chrome schedule.

This change must preserve the retained renderer's fixed capacities,
transactional generation/delta behavior, deterministic semantic IDs, and the
sealed HUD privacy contract.

### Smooth/AppKit renderer

AppKit cannot supply lenticular disparity, but it must reproduce the same
occlusion. The prepared Smooth draw order will be partitioned into semantic
passes around the statistics plane:

1. rear/behind layers and receiving-surface shadows;
2. the pet-attached group when the pet is behind the statistics;
3. the statistics;
4. the pet-attached group when the pet is in front of the statistics;
5. foreground habitat layers;
6. the three gauge lanes from deepest to frontmost;
7. status/trouble and dimming.

The pet-attached group is body, particles/performance cue, and mood aura. Wall
shadow and floor projection are explicitly excluded. Gauge tracks and fills are
still generated by `prepared_perimeter_gauge_arcs`; the AppKit painter groups
those arcs by lane and paints the lanes in shared bezel order.

The pass partition is prepared outside the native paint callback. The callback
must not allocate, sort, or derive depth decisions during repaint.

### Pixel and Classic fallback behavior

Pixel and Classic do not currently carry the semantic layer information needed
to place the statistics between world groups. They retain the existing flat
front-glass behavior. This is an explicit fallback limitation, not a second
interpretation of the lenticular contract. No depth-aware acceptance claim may
be made from those modes.

## Validation and Failure Behavior

Depth planes are constants in the round scene contract and are validated as a
strict finite ordering:

```text
STATISTICS_Z < PET_MAX_Z < PACE_Z < DAILY_Z < XP_Z < CAMERA_NEAR_Z
```

The statistics plane itself must remain inside `[-1.0, +1.0]` so it can be
compared directly with effective pet depth. Invalid or non-finite composition
data fails frame preparation through the existing typed preparation error path;
render callbacks do not repair, clamp, or guess.

Retained scene validation must reject a statistics primitive that is still
screen chrome, a gauge lane with no world depth, a broken lane ordering, or a
HUD schedule containing both world and screen copies. Exactly one statistics
draw is allowed per frame.

## Testing

### Pure contract tests

- The statistics plane is exactly `+0.72` and gauge planes are exactly `+1.55`,
  `+1.65`, and `+1.75` in pace/daily/XP order.
- Plane ordering is finite and satisfies all camera/pet constraints.
- A pet at effective Z `+0.72` remains behind the statistics; a pet just above
  it is in front.
- Calm and asleep samples compare using effective, not raw, depth.
- Wall shadow and floor projection never enter the pet-attached crossing group.
- Gauge arc generation retains identical geometry, values, colors, and
  within-lane order.

### Renderer contract tests

- Smooth/AppKit prepared passes put pet body, particles, and aura on the correct
  side of the statistics at far, neutral, boundary, near-crossing, and front
  samples.
- Retained scene templates expose one world statistics draw, three world gauge
  lane draws, and only status/trouble/dim in the relevant screen-chrome phases.
- Retained blended ordering changes transactionally when the pet crosses
  `+0.72` and remains stable when depth does not change.
- Retained HUD atlas capacity, repertoire, privacy redaction, and staging tests
  continue to pass with the world-space HUD schedule.
- Paired Smooth/retained captures agree on which pixels occlude the statistics
  at representative behind and front samples.

### Visual acceptance

Deterministic review captures will include at least:

- rear (`-1.0`), neutral (`0.0`), boundary (`+0.72`), just-crossed, and front
  (`+1.0`) pet depths;
- one frame where pet ink visibly overlaps each of the three statistics lines;
- a capture proving the near pet covers statistics glyphs but never covers any
  gauge lane;
- a capture proving wall and floor shadows stay on their surfaces during the
  crossing; and
- a gauge-focused frame showing the inner-to-outer pace/daily/XP depth order.

The visual target is a pet swimming through a tank on the desk: the statistics
feel suspended within the water, the pet passes them near the glass, and the
gauges read as the display's stepped bezel rather than floating scene content.

## Acceptance Criteria

1. The pet's existing rear-to-front path and endpoint geometry are unchanged.
2. At effective depth `<= +0.72`, the pet-front group is behind the central
   statistics.
3. At effective depth `> +0.72`, pet body, particles/performance cue, and mood
   aura are in front of the central statistics.
4. Wall shadow and floor projection never move in front of the statistics with
   the pet.
5. Central statistics retain their current layout and render exactly once.
6. Pace, daily, and XP gauge lanes use Z `+1.55`, `+1.65`, and `+1.75`, remain in
   front of all pet positions, and preserve existing arc behavior.
7. Status/trouble remain front-glass chrome and dimming remains last.
8. Smooth/AppKit and retained captures agree on pet/statistics occlusion at the
   specified depth fixtures.
9. Retained scene validation, fixed capacities, privacy claims, and transactional
   depth sorting remain intact.
10. Pixel and Classic remain functional with their documented flat fallback;
    neither blocks depth-aware Smooth/retained acceptance.
