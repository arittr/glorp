# Glorp Full-Tank Depth Traversal Design

**Date:** 2026-07-16
**Status:** Awaiting design approval

## Context

Glorp already produces a normalized pet depth sample across `[-1.0, 1.0]`, and
the calm awake lifecycle now permits that sample to reach both endpoints. The
rendered pet still does not appear to reach the front of the tank. Its vertical
position is dominated by planar roam geometry: a `0.6` Y-drift fraction, a
`0.5` upward bias, and a five-row bottom reservation for the HUD. At production
companion sizes, those constraints keep the pet center at approximately the
vertical midpoint even when depth reaches `1.0`.

The shallow-tank projection cannot correct that placement. It changes the pet
scale from `0.97x` to `1.035x` and contributes at most `0.10` cell of vertical
perspective. The result is a depth value that reaches the glass numerically but
a scene that still reads as though the pet stops halfway forward.

The companion should instead behave like a virtual pet moving inside a
physical tank on the desk: depth must determine where the pet appears between
the rear and front of the tank, while small planar motion keeps the movement
organic.

## Goal

Make the shared depth sample the canonical source of the pet's rear-to-front
screen-space placement in both non-Classic companion render paths. At full rear
and front depth, the pet must visibly reach the corresponding safe limits of
the circular tank aperture. The current shallow scale, atmosphere, shadow, and
parallax treatment remains restrained, and every depth-driven pet cue must use
the same effective depth sample.

For this first slice, the stats remain a front-glass overlay. The pet can travel
behind them at the front of the tank without the HUD reducing its movement
range.

## Non-Goals

- Do not redesign the pet, tank, props, gauges, or HUD typography.
- Do not change the depth waveform, depth period, X roam, activity-energy
  model, or species animation.
- Do not increase the shallow scale excursion merely to make forward movement
  more obvious.
- Do not move the HUD into the middle of the tank in this slice.
- Do not split the pet into far and near render passes in this slice.
- Do not change Classic renderer placement or parity behavior.
- Do not add a user-facing setting, compatibility mode, dependency, or saved
  state.

## Approved Visual Contract

### Depth-driven vertical placement

The normalized effective depth maps monotonically across the pet's usable
rear-to-front vertical envelope:

| Effective depth | Visual plane | Target pet-center position |
|---:|---|---:|
| `-1.0` | Rear | approximately 27% of aperture height |
| `0.0` | Neutral | approximately 50% of aperture height |
| `1.0` | Front glass | approximately 73% of aperture height |

These percentages define the intended composition, not fixed pixel or cell
coordinates. The shared placement resolver derives the actual endpoints from
the circular aperture and the pet's maximum transformed bounds; renderers do
not derive them independently. The entire maximum-scale pet must remain inside
the aperture at both endpoints.

The mapping must be continuous and centered: `0.0` resolves to the neutral
plane, equal depth steps produce equal vertical travel before safety clamping,
and the rear and front endpoints are both attainable. A small viewport may
compress the envelope symmetrically to preserve the pet, but it must not
silently restore the current front-half cap.

### Local planar wander

The existing planar Y motion becomes a bounded deviation around the canonical
depth path rather than the source of the path. Its maximum allowed contribution
tapers with depth magnitude, using `1 - abs(effective_z)` or an equivalent
continuous envelope:

- At neutral depth, the pet retains the approved small Y wander.
- As the pet approaches either endpoint, the deviation decreases.
- At `-1.0` and `1.0`, the deviation is zero, so planar wander cannot prevent
  the pet from reaching the rear or front limit.

X roam remains unchanged apart from the existing aperture safety clamp. The
combined X/Y placement is resolved against maximum transformed pet bounds, not
the current HUD reservation.

### Depth cues

The balanced-shallow cue profile remains in force:

- far, neutral, and near pet scale remain `0.97x`, `1.0x`, and `1.035x`;
- maximum vertical perspective remains `0.10` cell;
- atmospheric attenuation, wall shadow, floor projection, aura, and authored
  scene layers continue to use the same effective depth sample;
- object parallax remains coupled to planar displacement and uses its current
  shallow caps.

The existing perspective offset is included when the final safe center and
bounds are resolved. It must not be added after endpoint clamping. No renderer
may independently infer or add a second depth position from scale, raw depth,
perspective, or planar Y motion.

### HUD depth plane

The HUD receives an explicit semantic depth plane in the prepared scene
contract. For this slice that plane is `FrontGlass`:

1. tank and authored scene layers render;
2. the pet and all pet-attached effects render at the resolved depth placement;
3. the HUD renders last as a front-glass overlay.

The HUD may overlap the pet when the pet is at the front. That overlap is
intentional and must not reserve movement space or push the pet upward.
Background props and other habitat content may continue to respect the lower
content reservation; only pet traversal is removed from the HUD reservation.

The semantic plane is required even though both current renderers draw the HUD
last. It records the compositing decision without baking it into motion code.
A later middle-plane experiment can change the HUD plane and split pet
compositing into far and near passes while reusing the same depth placement.
That future compositor change is localized but is not treated as a constant or
configuration-only change.

## Architecture

The implementation introduces one renderer-neutral depth-placement result in
the round scene domain. It combines:

- the effective lifecycle-adjusted depth;
- aperture and maximum-scale pet bounds;
- the canonical rear, neutral, and front positions;
- the tapered local Y deviation; and
- the final safe pet center and bounds.

`src/round/motion.rs` remains responsible for deterministic depth and planar
motion samples. It must expose the local planar contribution without applying
the HUD-reserved vertical envelope to pet traversal.

The shared round depth/scene preparation layer resolves the canonical
depth-placement result. Both the Smooth plan builder and the direct companion
scene input consume that result. Renderer code applies the prepared placement;
it does not repeat the mapping. Classic continues to use its existing
top-left-cell placement path.

The prepared scene contract also carries a small HUD-plane enum whose first
supported value is `FrontGlass`. Both current render paths interpret
`FrontGlass` by drawing the HUD after all pet-attached layers. The enum is a
scene contract, not a user setting or a promise that arbitrary planes already
render correctly.

```text
raw depth + lifecycle              planar roam
          |                             |
          v                             v
     effective depth ------> tapered local Y
          |                             |
          +----------+------------------+
                     v
      aperture + maximum pet bounds
                     |
                     v
       shared depth-placement result
          |                       |
          v                       v
   Smooth scene plan       direct scene input
          |                       |
          +-----------+-----------+
                      v
       scene -> pet/effects -> FrontGlass HUD
```

## Lifecycle and Accessibility

- Awake active and awake calm pets use the full rear-to-front depth envelope.
- Asleep pets retain quarter-depth lifecycle attenuation and therefore stay
  close to neutral while resting.
- Reduce Motion resolves to neutral depth and suppresses animated parallax,
  preserving the existing accessibility behavior.
- Activity energy continues to influence planar roam only. It must not prevent
  an awake calm pet from eventually reaching either depth endpoint.

## Validation and Failure Behavior

The placement resolver rejects non-finite depth, motion, aperture, or bound
inputs. It validates that its final maximum-scale bounds are finite and inside
the aperture. Invalid prepared geometry follows the existing fail-closed scene
generation or last-good-frame behavior; it must not be clamped into a plausible
but incorrect frame.

Safety compression is allowed only when the aperture genuinely cannot contain
the target composition. It must be deterministic, symmetric around neutral,
and observable in tests. The HUD rectangle is not an input to pet depth
placement when its plane is `FrontGlass`.

## Verification

Implementation verification must prove:

1. A production-size companion at effective depth `1.0` places the pet center
   beyond the viewport midpoint and at the derived front-safe limit.
2. Effective depth `-1.0`, `0.0`, and `1.0` resolve to the derived rear,
   neutral, and front positions, with the expected approximately
   `27% / 50% / 73%` composition when the aperture permits it.
3. Maximum-scale pet bounds remain inside the circular aperture across the
   complete deterministic X/Y/Z cycle.
4. Local Y deviation is continuous, is largest at neutral depth, and is exactly
   zero at both depth endpoints.
5. Awake calm and active states can reach the same rear and front placement;
   asleep and Reduce Motion retain their lifecycle behavior.
6. Scale, atmosphere, wall shadow, floor projection, and aura consume the same
   effective depth used by placement. Object parallax remains driven by the
   canonical planar displacement and its independent lifecycle attenuation.
7. Smooth and direct companion-scene paths resolve matching pet centers and
   transformed bounds for identical inputs.
8. Classic placement is unchanged.
9. The prepared HUD plane is `FrontGlass`, both render paths composite it last,
   and HUD overlap does not alter pet placement.
10. Deterministic Preview Lab round and animation artifacts visibly show the
    pet at the rear, neutral, and front planes without clipping.
11. Targeted motion, round-scene, and companion tests pass, followed by the
    repository's formatting, clippy, and full test checks.

The endpoint regression must fail against the current implementation by
demonstrating that the near pet center remains capped around the midpoint.
Visual review uses deterministic captures before launching the live companion.

## Relationship to the Shallow-Tank Design

This design supersedes the earlier shallow-tank document where it says calm
depth is halved, ordinary idle uses only a fraction of maximum depth, the pet
wander path is unchanged, and the HUD reservation constrains pet clearance. It
preserves that document's balanced-shallow scale, atmosphere, parallax, wall
shadow, and floor-projection tuning.

The result is a shallow-looking tank with full usable front-to-back placement:
small apparent-size change communicates physical shallowness, while vertical
screen-space travel communicates where the pet is within that tank.

## Done Criteria

- An awake pet visibly travels from the rear safe limit through neutral to the
  front safe limit; the near endpoint is no longer capped at 50%.
- The tank still reads as physically shallow rather than a deep room.
- Pet placement is derived once and matches across Smooth and direct render
  paths.
- HUD overlap is a deliberate front-glass compositing effect, not a movement
  constraint.
- The maximum-scale pet remains inside the circular aperture at every tested
  position.
- Sleep, Reduce Motion, Classic rendering, and deterministic replay retain
  their stated behavior.
- A later middle-HUD experiment can reuse the completed motion and placement
  work and change only the prepared HUD plane plus renderer compositing.
