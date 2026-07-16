# Glorp Shallow Tank Depth Design

**Date:** 2026-07-16
**Status:** Approved for implementation planning

## Problem

Glorp's round companion currently reads as a room-sized volume rather than a
one-to-two-foot-deep virtual tank. At full depth excursion, the pet scales from
`0.92x` at the rear to `1.12x` at the glass. The near pose is therefore about
21.7% larger than the far pose. Object parallax, atmospheric fading, vertical
perspective, and shadow travel reinforce the same exaggerated distance.

The existing implementation describes this as a shallow tank, but the visual
result is not shallow. The pet appears to travel very far away from the viewer.

## Goal

Keep visible front-to-back pet motion and layered parallax while making the
space read as a shallow physical tank. The approved target is the **balanced
shallow** profile: clearly dimensional, but restrained enough that the pet
remains present at the rear wall.

## Non-Goals

- Do not remove depth animation or parallax.
- Do not change the pet's wander path, depth waveform, timing, or activity
  energy model.
- Do not add user-facing depth settings.
- Do not change scene Z ordering, clipping, lighting architecture, or renderer
  selection.
- Do not redesign the tank, pet art, props, HUD, gauges, or Reduce Motion
  behavior.

## Approved Visual Contract

### Pet depth mapping

The normalized pet depth remains `[-1.0, 1.0]`, with `0.0` as the exact neutral
plane. Only the visual projection of that value changes.

| Cue | Current | Approved |
|---|---:|---:|
| Far pet scale | `0.92x` | `0.97x` |
| Neutral pet scale | `1.0x` | `1.0x` |
| Near pet scale | `1.12x` | `1.035x` |
| Near/far apparent-size difference | 21.7% | 6.7% |
| Maximum vertical perspective | `0.30` cells | `0.10` cells |
| Far atmospheric opacity | `0.82` | `0.93` |
| Neutral/near atmospheric opacity | `1.0` | `1.0` |

The piecewise mapping around neutral remains intact. Classic parity therefore
continues to hold at `z = 0.0`, and the front and rear excursions remain
slightly asymmetric.

### Layer parallax

Parallax remains driven by the pet's continuous X/Y displacement, not by its Z
value. Plane ordering and motion direction remain unchanged.

| Plane | Current multiplier | Approved multiplier |
|---|---:|---:|
| Far/background | `0.010` | `0.006` |
| Mid, where represented | `0.020` | `0.010` |
| Behind pet | `0.030` | `0.014` |
| Foreground | `0.045` | `0.022` |

The maximum resolved translation becomes `0.25` cell horizontally and `0.15`
cell vertically. The Smooth safety resolver may still reduce an individual
object below those maxima to protect chrome and gauge reservations.

The direct companion-scene path has three authored planes—background,
behind-pet, and foreground. The Smooth compatibility path also has a mid plane.
The shared planes must use identical canonical multipliers in both paths.

### Wall and floor shadows

The wall shadow remains the primary separation cue, but its detachment range
shrinks from `0.35-2.4` cells to `0.45-1.2` cells. Existing far/near strength
values remain unchanged so the shadow stays legible against the dark tank.

The floor projection keeps its existing alpha range while using a narrower
geometric excursion:

| Floor projection cue | Current | Approved |
|---|---:|---:|
| Bed-position band | `0.10-0.45` | `0.18-0.32` |
| Horizontal radius / viewport width | `0.07-0.13` | `0.085-0.11` |
| Vertical radius / viewport height | `0.016-0.040` | `0.022-0.032` |

This keeps the contact shadow readable without letting its position or size
imply a much deeper floor than the pet scale does.

## Architecture

This is a tuning correction, not a new depth system.

- `src/round/depth.rs` remains the canonical pet scale, perspective, and
  atmospheric projection used by both Smooth and direct companion-scene input.
- `src/presentation/companion_effects.rs` remains the renderer-neutral home for
  wall and floor shadow geometry. It will also own the canonical parallax
  multipliers and per-axis cell caps so renderer paths cannot drift.
- `src/round/parallax.rs` keeps its safety and chrome-overlap resolver but reads
  the canonical multipliers and caps.
- `src/presentation/companion_scene/mod.rs` and
  `src/presentation/companion_scene/input.rs` keep their authored-depth and
  point-projection responsibilities but read the same canonical parallax
  tuning.

No new scene types, configuration surfaces, compatibility shims, or runtime
branches are required.

## Lifecycle and Accessibility

Existing lifecycle attenuation is preserved:

- Normal/active: full approved excursion.
- Calm: half effective depth and parallax motion.
- Asleep: quarter effective depth and parallax motion.
- Reduce Motion: neutral pet depth and zero object parallax.

The activity-energy model remains unchanged, so ordinary idle movement still
uses only a fraction of the approved maximum range.

## Validation and Failure Behavior

Existing finite-value, normalized-depth, scale-bound, opacity-bound, and
geometry validation remains fail-closed. Bounds and exact-value assertions must
be updated to the approved values. Invalid geometry must continue to use the
established last-good-frame or generation-rejection behavior; tuning changes
must not create a new fallback path.

## Verification

Implementation verification must prove:

1. Far, neutral, and near depth samples resolve exactly to `0.97`, `1.0`, and
   `1.035` in the normal lifecycle.
2. Calm and asleep samples retain half and quarter attenuation.
3. Parallax multipliers remain strictly monotonic by plane and never exceed
   `0.25` horizontal or `0.15` vertical cells.
4. Smooth and direct companion-scene paths use identical values for shared
   planes.
5. Reduce Motion produces neutral depth and zero parallax.
6. Wall-shadow detachment and floor-projection geometry stay within the
   approved ranges at both endpoints.
7. Existing renderer, scene-runtime, and deterministic preview tests pass.
8. Deterministic round and animation Preview Lab artifacts show a present pet
   at the rear wall, visible but restrained size change, and ordered parallax.

Visual verification should use headless deterministic captures first. It must
not fullscreen or steal focus during automated review.

## Done Criteria

- The pet remains visibly closer at the glass and farther at the rear wall.
- The full near/far apparent-size difference is approximately 6.7%, not 21.7%.
- Background, behind-pet, and foreground layers retain ordered parallax without
  any plane moving more than the approved cell caps.
- Shadows agree with the shallower scale/parallax story.
- Smooth and retained/direct scene paths cannot diverge in shared parallax
  tuning.
- All targeted tests, renderer regression tests, and deterministic preview
  checks pass cleanly.
