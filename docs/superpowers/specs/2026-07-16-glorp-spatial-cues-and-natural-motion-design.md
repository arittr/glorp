# Glorp Spatial Cues and Natural Motion Design

**Date:** 2026-07-16

**Status:** Approved direction, written for implementation planning

**Builds on:**

- `2026-07-16-glorp-full-tank-depth-traversal-design.md`
- `2026-07-16-glorp-lenticular-hud-depth-layers-design.md`
- `2026-07-16-glorp-soft-depth-interaction-and-cast-shadows-design.md`

## Summary

The round companion will improve spatial readability without activating a
general dynamic-lighting system. Statistics remain translucent glass glyphs at
their existing tank plane and gain a faint shadow projected onto the rear wall.
The existing authored prop-shadow profiles remain authoritative.

The broad pet mood aura is replaced by a narrow, low-opacity mood-tinted edge
glow immediately outside the pet silhouette, preserving a quiet mood cue
without painting a screen-facing disc behind the creature.

The current mixed-frequency X/Y/Z oscillator is replaced by deterministic,
purposeful 3D drift. The pet selects bounded destinations, rests, turns while
nearly stationary, then glides along one coherent path. Full tank depth remains
reachable, but the pet no longer reverses continually or traverses the whole
tank every few seconds.

## Problem

The latest companion has correct depth ordering and continuous rendering, but
two presentation systems still fight the physical-tank illusion.

First, the central statistics plane is difficult to locate in space. Its
current rear emission echo is offset by only a fraction of a point and reads as
text softness rather than separation from the rear wall. Props now cast
appropriate receiving-surface shadows, which makes the ungrounded statistics
more conspicuous.

Second, pet motion is mathematically smooth but behaviorally frenetic. The
current companion motion combines:

- independent mixed-frequency X and Y oscillators on a 22-second base period;
- an independent mixed-frequency Z oscillator;
- a separate two-second vertical bob; and
- immediate facing changes derived from horizontal velocity.

In a two-minute sample, each axis reverses roughly 13-15 times. Full-depth
placement now maps Z across approximately 46 percent of the aperture height, so
the previously subtle depth oscillator produces prominent rear-to-front trips
on a roughly eight-second half-cycle. There are no destinations, rests, or
intentional turns. The renderer is faithfully drawing a continuous but
animal-like-in-name-only trajectory.

The broad mood aura compounds both problems. It was introduced to replace
three vital ticks with an ambient mood color, but mood is already conveyed by
the pet's eyes, expression, posture, and trouble state. The large radial field
now reads as another flat plane and softens shadows and statistics crossings.

## Goals

- Make the statistics plane visibly suspended between the rear wall and front
  glass.
- Preserve statistics typography, values, layout, privacy, and Z `+0.72`.
- Preserve existing appropriate prop contact and directional cast shadows.
- Remove the large persistent mood aura from every round companion path.
- Retain mood as a faint edge-local cue rather than a broad field.
- Replace continuous oscillator wander with calm, purposeful 3D locomotion.
- Keep full rear-to-front depth reachable without making it routine or rapid.
- Keep motion deterministic, continuous, restart-stable, and shared by Smooth
  and retained renderers.
- Preserve Reduce Motion, sleep settling, aperture safety, and renderer parity.

## Non-goals

- No general dynamic-lighting system, shadow maps, material normals, moving
  lights, HDR lighting, or mesh lighting.
- No new multiview, quilt, head-tracking, or lenticular calibration work.
- No pet pathfinding around props or statistics.
- No physics simulation, collision system, flocking, or procedural behavior AI.
- No species-specific locomotion personalities in this slice.
- No change to statistics content, gauges, prop art, pet art, or tank geometry.
- No activity-driven animation-period changes.

## Statistics Rear-Wall Shadow

### Material model

Statistics remain translucent glass glyphs with a restrained emissive primary
and the existing rear echo. They also interrupt the authored key light enough
to cast a faint, blurred projection on the rear wall. This is a spatial cue,
not a solid drop shadow and not a second readable copy of the values.

### Projection

The shadow uses the existing fixed visual key-light convention already shared
by prop cast shadows. Its displacement and softness derive from the distance
between the statistics plane and the rear receiving plane. The statistics plane
is fixed, so the projection is stable and does not move with the pet.

The projection:

- is clipped to the circular aperture and rear-wall receiving region;
- renders behind props, pet, statistics, and gauges;
- uses multiply blending with the existing biome-derived shadow family;
- stays faint enough that individual values are not legible in the blur; and
- never lands on the pet, foreground props, gauges, or front-glass chrome.

### Privacy and renderer ownership

The shadow reuses renderer-private statistics glyph coverage. No HUD text,
atlas index, coverage pixels, value-shaped primitive, or shadow copy enters the
scene contract, checksums, Preview Lab scene artifacts, logs, diagnostics, or
external redacted captures. Internal review capture may include the live
projection only when it already includes the sealed HUD. Missing private
coverage fails frame preparation rather than falling back to serialized or
globally blurred text.

Smooth/AppKit and retained use the same projection constants and ordering.
Pixel and Classic may omit the rear-wall projection if their flat fallback
cannot preserve the privacy and receiving-surface contract.

## Prop Shadows

The current explicit `None`, `ContactOnly`, and `Elevated` prop profiles remain
the source of truth.

- Grounded small or flat props retain contact shadows.
- Grounded solid elevated props retain directional cast shadows.
- Suspended, wall-mounted, translucent, emissive, or visually weightless props
  retain no receiving-surface shadow.
- Overlapping prop shadows continue to union coverage rather than repeatedly
  darkening the same pixels.

This bundle does not introduce runtime shadow eligibility inference or make
every prop cast a shadow. It preserves the current authored behavior and
adds regression coverage where the statistics and pet changes affect ordering.

## Pet Aura Replacement

### Remove the broad field

The persistent radial `MoodAura` field is removed from Smooth/AppKit, retained,
Classic round-companion composition, and Pixel's always-on base treatment. It
no longer participates in statistics crossing, depth placement, scene
painting, review captures, or broad background compositing.

If fixed scene ABI slots require the semantic to remain reserved for the
current schema, the slot is present but invisible and carries no live paint.
This is an ABI detail, not a compatibility visual.

### Faint silhouette glow

The replacement is a narrow edge glow derived from pet-body coverage only:

- hue comes from the existing mood palette;
- coverage extends only a small physical distance outside the visible body;
- opacity is low and constant during ordinary idle presentation;
- the pet body paints over the inner portion, leaving only the exterior rim;
- particles, wall shadow, floor projection, and props do not expand the mask;
- the rim uses the pet's effective Z and crosses statistics with the pet body;
  and
- the rim does not cast a shadow or illuminate nearby surfaces.

This is authored emissive trim, not dynamic lighting. A brief feed/activity
event raises rim intensity through the existing bounded pulse envelope. No
persistent activity signal changes its size or animation frequency.

The implementation must make the rim easy to disable as one constant-backed
presentation choice. If deterministic visual review shows fuzzy glyph cells,
a second readable silhouette, or reduced depth clarity, the accepted fallback
is no glow at all—not restoration of the radial aura.

## Purposeful 3D Drift

### Why not retune the oscillator

Making the current frequencies slower would reduce speed but preserve the
underlying problem: independent axes continuously pull the pet through turns
with no destination or rest. The motion generator therefore changes model
rather than constants.

### Deterministic segments

Motion is a deterministic sequence of 60-second segments derived from stable
pet identity and segment index. Segment `N` has:

1. the held 3D destination `T(N)`;
2. a stationary dwell interval at `T(N)`;
3. one bounded next destination `T(N+1)`; and
4. one glide interval from `T(N)` to `T(N+1)`.

At the boundary, segment `N+1` begins by dwelling at the exact endpoint
`T(N+1)`. No runtime history or persisted locomotion state is needed to recover
the current segment after relaunch.

Each segment lasts exactly 60 seconds, keeping segment lookup constant-time and
restart-stable. Dwell duration is identity-seeded between 8 and 18 seconds; the
remaining 42-52 seconds is the glide.

The glide uses the quintic minimum-jerk curve
`6t^5 - 15t^4 + 10t^3`, so position, velocity, and acceleration meet cleanly at
both endpoints. X, Y, and Z use the same segment phase. A deterministic
quadratic control offset bends nonzero X/Y paths by at most 12 percent of
segment length to avoid robotic straight-line travel; it does not alter Z,
introduce a second turn, or give any axis an independent oscillator.

### Destination policy

Destination selection is deterministic and bounded by the existing aperture
and maximum pet silhouette. It must:

- limit the distance of one move so the pet does not ricochet across all three
  axes at once;
- favor local X/Y exploration at the current depth;
- choose a meaningful Z excursion less often than a planar move;
- permit the full rear and front endpoints over a bounded multi-segment review
  window;
- avoid choosing effectively identical consecutive destinations; and
- avoid a direct reversal along the just-completed path unless no other valid
  destination exists.

The full-depth placement resolver remains canonical. The locomotion generator
produces normalized X/Y/Z intent; it does not duplicate aperture projection,
statistics ordering, parallax, shadow, or scale calculations.

### Facing and posture

The pet chooses facing from the next segment's meaningful horizontal travel.
When a turn is needed, facing changes at the start of the dwell, while
translational speed is zero, and then remains fixed through the glide. Tiny X
deltas retain the previous facing. There is no mid-glide sign sampling or
turnaround flicker.

The independent two-second companion bob is removed. Existing pet-art breath
and posture remain. If a separate water-float displacement is retained, it must
be slow, below one tenth of a cell, and subordinate to the single locomotion
path rather than a competing visible rhythm.

### Activity, calm, sleep, and accessibility

Live activity does not change the locomotion clock or continuously rescale the
current path. That avoids poll-aligned phase or position changes. Activity
continues to appear through existing transient pet cues and does not influence
locomotion destinations in this slice.

- Awake normal and calm pets share the same continuous geometry and can
  eventually reach the full depth envelope.
- Sleep transition settles from the active path into a held near-neutral pose;
  sleeping pets do not continue the destination schedule visibly.
- Sleep and wake reuse the day-context onset/resume instants already used by
  watch wandering, so both transitions evaluate from a stable locomotion
  instant rather than introducing mutable behavior state.
- Reduce Motion resolves to the existing neutral static pose with no rim pulse
  or parallax motion.

## Dynamic-Lighting Decision

General dynamic lighting remains deferred. The scene contract reserves up to
two light records, but production currently clears the light list, the retained
shader does not evaluate it, and lit shallow-card rendering is unsupported.
Most visible content is glyph or analytic coverage without material normals.

Activating the light records now would add runtime, shader, validation, and
cross-renderer policy while producing little more than the authored projection
already provides. Revisit dynamic lighting when at least one real consumer
ships, such as:

- a moving lantern or time-of-day key light;
- lit shallow-card or mesh-backed props;
- per-material normal or rim response; or
- multiple surfaces that must share runtime light direction and intensity.

The statistics and prop projections use one shared authored key-light constant
so that later migration to a real light record remains localized.

## Validation and Failure Behavior

- Statistics shadow inputs must be finite, bounded, private, and tied to the
  single sealed HUD resource.
- The shadow must fail closed if private glyph coverage or receiving-surface
  geometry is unavailable.
- Rim width and opacity must be finite and bounded; invalid values disable the
  rim rather than restoring the aura.
- Motion segment duration, dwell, endpoints, and curve inputs must be finite and
  within authored bounds.
- Consecutive segments must meet with matching position and zero endpoint
  velocity/acceleration.
- Destination projection continues to use existing aperture and maximum-scale
  validation; invalid geometry keeps the last good frame.
- Renderer-specific code consumes prepared motion and spatial-cue results. It
  does not independently choose destinations, thresholds, or shadow geometry.

## Verification

### Pure motion tests

- Segment boundaries are continuous in position, velocity, and acceleration.
- Dwell intervals have zero translation and stable facing.
- Facing changes only while stationary or for an explicit wake/settle boundary.
- No axis reverses during one glide unless the authored curved path explicitly
  requires it and remains within the turn bound.
- A two-minute normal sample contains materially fewer direction reversals than
  the current 13-15 per axis and includes at least one visible dwell.
- Full rear and front destinations are reachable over a bounded deterministic
  multi-segment window.
- Sleep, wake, calm, restart, clock-boundary, and Reduce Motion cases remain
  deterministic and continuous.

### Rendering tests

- Statistics projection appears only on the rear receiving surface and remains
  behind props, pet, HUD primary, and gauges.
- External redacted captures omit the projected blur; internal sealed-HUD
  review captures show it only as an unreadable spatial cue.
- Existing prop-shadow qualification and coverage union remain unchanged.
- No broad mood-aura coverage remains in any round companion renderer.
- The rim stays within its maximum physical width, uses body coverage
  only, and follows the pet's effective depth.
- Smooth/AppKit and retained agree on motion pose, facing, statistics projection,
  aura absence, and rim parameters.

### Visual review

Preview Lab adds or updates deterministic motion strips covering:

- dwell, departure, mid-glide, arrival, and turn-before-departure;
- a local planar move and a less-frequent depth excursion;
- statistics shadow with pet behind, interacting, and in front;
- prop shadows with statistics projection present;
- mood variants with no aura and with the candidate narrow rim; and
- sleep settle and wake resume.

Review rejects any result that reads as a screensaver orbit, repeated bobbing,
rapid tank-depth pumping, a duplicate statistics copy, fuzzy pet glyphs, or a
reintroduced broad glow field.

## Acceptance Criteria

1. Statistics visibly occupy a stable interior plane without becoming a solid
   panel or revealing private values through a duplicate copy.
2. Appropriate prop shadows remain present and ordered on receiving surfaces.
3. The broad persistent mood aura is absent from all round companion paths.
4. The pet has at most a faint body-local rim; disabling it does not remove any
   unique state or event information.
5. The pet visibly rests, turns while stopped, and follows coherent 3D glides.
6. Ordinary motion no longer produces rapid repeated reversals or eight-second
   rear-to-front pumping.
7. Full tank depth remains reachable over time and uses the existing canonical
   placement, scale, atmosphere, parallax, and shadow contracts.
8. Activity changes do not produce position or phase jumps.
9. Sleep, wake, Reduce Motion, aperture safety, privacy, and last-good-frame
   behavior remain correct.
10. Smooth/AppKit and retained present matching spatial cues and locomotion;
    flat fallbacks remain functional.
