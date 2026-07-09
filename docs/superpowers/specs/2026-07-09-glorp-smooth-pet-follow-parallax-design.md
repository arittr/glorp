# Glorp Smooth Pet-Follow Parallax - design

- Date: 2026-07-09
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-companion-renderer-v2-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-smooth-motion-stabilization-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-companion-draw-boundary-hardening-design.md`

## Calibration

The smooth companion now carries the current Classic Glorp scene through typed
layers, repaints at a native cadence, preserves the slower semantic art clock,
and moves the pet from a continuous anchor. Live review shows that the motion is
stable enough to add the first depth behavior.

This slice follows the same delivery shape as the stabilization work:

1. strengthen one reusable renderer boundary; and
2. prove the boundary with one visible feature.

The reusable boundary is a portable layer-motion contract. The visible feature
is subtle pet-follow parallax across the existing tank layers. This is not a
general animation engine and not a broad polish pass.

## Implementation Entry Gate

Parallax implementation starts only after the companion draw-boundary hardening
slice is complete on the target branch. The implementation plan must verify all
of these conditions before changing parallax code:

- `RoundView::drawRect:` paints `last_good_frame` through the guarded prepared-
  frame path;
- `drawRect` does not build round layout, Classic draw lists, smooth plans, HUD
  text, or review samples;
- `ui_tick()` owns frame preparation and last-good-frame replacement; and
- production smooth preparation uses `try_build_round_smooth_scene_plan(...)`.

Parallax is implemented only in the prepared smooth plan. Do not add a second
parallax implementation to the legacy `draw_scene(...)` path while boundary
hardening is in flight. If any entry condition is false, finish or merge that
prerequisite first.

## Problem

`SmoothCompanionLayer` already preserves role, anchor, transform, bounds, z
order, clip, opacity, blend mode, and local items. The smooth scene builder
currently applies continuous motion only to the pet-attached layers. Other room,
prop, and tank-life layers stay at their Classic snapped positions.

That proves motion but not depth. The companion still reads as a moving pet on a
flat field because every non-pet layer occupies the same motion plane.

Adding ad hoc translations in AppKit would produce the visible effect quickly,
but it would make the native host own scene semantics and would be difficult to
review deterministically. The depth assignment and transform resolution must
remain in cfg-free Rust so Preview Lab and future renderers can consume the same
plan.

## Goals

1. Add an explicit, renderer-neutral motion binding to each smooth layer.
2. Derive a stable parallax focus from the pet's existing continuous wander.
3. Resolve bounded per-layer transforms in Rust before AppKit paints.
4. Make the tank visibly read as several depth planes while keeping Glorp's
   current movement unchanged.
5. Keep gauges, HUD, aperture chrome, and status overlays fixed.
6. Preserve exact Classic flatten parity and existing privacy guarantees.
7. Extend deterministic and native review evidence so the effect cannot regress
   into invisible, excessive, or discontinuous motion.

## Non-goals

- No scale, rotation, squash/stretch, velocity lean, or spring physics.
- No autonomous camera drift.
- No cursor, hover, click, drag, or window-motion input.
- No feed, activity, prop-reaction, or scene-moment animation.
- No new pet, prop, tank-life, gauge, HUD, or room art.
- No change to the pet's wander curve, speed, facing, bob, or semantic art clock.
- No default renderer change.
- No Classic or Pixel renderer behavior change.
- No Linux windowing implementation.
- No general-purpose animation dependency.

## Product Behavior

The tank follows Glorp gently as Glorp roams. Layers move in the same direction
as the pet at progressively different fractions:

- far room texture moves least;
- ambient marks move slightly more;
- background props and tank life move more than ambient marks;
- the pet remains on its current stable path and receives no added parallax;
- foreground props and tank life move most; and
- companion chrome remains fixed.

The result should feel like a restrained camera follow inside the porthole, not
like the habitat is sliding around independently. The effect must be visible in
normal awake mode at the standard 960 px companion size, but it must not make
props feel detached from the room.

Horizontal motion is stronger than vertical motion. Lifecycle attenuation uses
one deterministic precedence rule: asleep (`0.25`) wins over `calm_mode`
(`0.5`), which wins over normal awake mode (`1.0`). The pet's existing
motion-energy scaling already reduces focus displacement when the pet is idle
or asleep; lifecycle attenuation is an additional presentation limit, not a
replacement for that existing behavior.

## Architecture

### Motion binding

Add an explicit motion binding to `SmoothCompanionLayer`:

```rust
pub enum SmoothLayerMotionBinding {
    Fixed,
    PetAttached,
    Parallax(SmoothDepthPlane),
}

pub enum SmoothDepthPlane {
    Far,
    Mid,
    Behind,
    Foreground,
}
```

Each layer must declare its motion behavior before the AppKit adapter receives
the plan. AppKit must not infer motion from glyphs, colors, z values, or layer
ids.

The initial mapping is:

| Motion binding | Layer roles |
|---|---|
| `Fixed` | `DepthRings`, `StatusHalo`, `TroubleIndicator`, `DimOverlay` |
| `PetAttached` | `ContactShadow`, `PetBody`, `PerformanceCue`, `MoodAura` |
| `Parallax(Far)` | `BiomeWash`, `RoomGlyphs` |
| `Parallax(Mid)` | `Ambient`, `Motes`, `ActivityGlyphs` |
| `Parallax(Behind)` | `PropsBehind`, `TankLifeBehind`, `ChestBubble` |
| `Parallax(Foreground)` | `PropsForeground`, `TankLifeForeground` |

The treasure chest is catalogued as a background prop, so its separate bubble
layer uses the Behind plane. A future prop-attached overlay that can occupy more
than one plane must carry an explicit plane from prop placement; this slice does
not add a generalized attachment graph.

`DepthRings` remains fixed because the current role reserves native porthole
geometry and has no local scene items. It may participate in a future shape-
backed depth treatment, but this slice does not move native chrome separately
from the prepared scene plan.

New or unknown roles default to `Fixed`. Adding a moving role requires an
explicit binding and coverage.

### Parallax focus

`CompanionPetPlacement` already exposes the pet's continuous motion anchor. Add
the neutral continuous motion origin produced by the same geometry calculation
with zero wander offsets and the existing upward bias:

```rust
pub struct CompanionPetPlacement {
    pub fractional_motion_top_left: SmoothPetAnchor,
    pub fractional_motion_origin_top_left: SmoothPetAnchor,
    pub fractional_top_left: SmoothPetAnchor,
    pub classic_snap_top_left: (u16, u16),
    pub classic_rect: ratatui::layout::Rect,
}
```

The focus offset is:

```text
fractional_motion_top_left - fractional_motion_origin_top_left
```

This excludes discrete Classic breath/posture shifts and the smooth pet bob. It
uses only the stable continuous tank wander that was fixed in the previous
slice. At the neutral wander point the focus offset is exactly zero even though
the pet's neutral composition remains upward-biased.

Add `parallax_focus_offset: SmoothPoint` to `SmoothCompanionPet` so Preview Lab
and native review capture can record it without reconstructing motion from
rendered cells.

### Component ownership

| Component | Responsibility |
|---|---|
| `src/presentation/smooth.rs` | Motion-binding and depth-plane types; plan evidence fields |
| `src/round/scene.rs` | Continuous current and neutral pet motion anchors |
| `src/round/parallax.rs` | Pure focus-to-layer translation resolver and safety clamps |
| `src/round/smooth.rs` | Bind roles, compose resolved deltas, and return fallible plans |
| `src/dev_preview/*` | Deterministic plan and strip evidence |
| `src/companion/review_capture.rs` | Native prepared-frame parallax evidence |
| `src/companion/app.rs` | Paint the already-resolved prepared plan |

### Adapter precision contract

The current AppKit smooth blitter chooses fractional drawing from a hard-coded
set of pet roles and rounds every other role to integer cells. That behavior
must become motion-binding-driven:

- `PetAttached` and `Parallax(_)` layers use `fractional_cell_to_point(...)`;
- `Fixed` layers retain their existing snapped behavior; and
- AppKit does not inspect `SmoothLayerRole` to decide coordinate precision.

This is part of the fundamental boundary, not optional polish. With parallax
capped below half a cell, routing a parallax layer through `cell_to_point(...)`
would erase most movement and turn the remainder into one-cell jumps.

### Pure resolver

Add one cfg-free pure resolver in `src/round/parallax.rs`. Its inputs are:

- continuous focus offset in grid cells;
- layer motion binding;
- lifecycle attenuation (`normal`, `calm`, or `asleep`);
- layer bounds and occupied local items;
- viewport and chrome reservations; and
- named tuning constants.

Its output is a finite, bounded `SmoothPoint` translation delta. The scene plan
builder composes that delta with the layer's existing translation. Pet anchor
correction and bob remain unchanged.

The initial tuning profile uses these named constants:

| Plane | Focus multiplier |
|---|---:|
| Far | `0.01` |
| Mid | `0.02` |
| Behind | `0.03` |
| Foreground | `0.045` |

These values are private tuning constants, not public API. Resolve horizontal
translation as `focus.x * multiplier` and vertical translation as
`focus.y * multiplier * 0.75`. Regardless of the multiplier, resolved
translation is capped at `0.5` grid cells horizontally and `0.25` grid cells
vertically.

Every parallax plane moves in the same direction as the focus offset. Before
layer-specific safety attenuation, plane ordering must remain monotonic by
magnitude:

```text
abs(Far) < abs(Mid) < abs(Behind) < abs(Foreground)
```

`Fixed` and `PetAttached` resolve to zero additional parallax translation.

### Geometry safety

Aggregate `local_bounds` are not sufficient collision geometry for sparse
ambient fields or layers containing several distant props. They are only the
outer envelope and may cross a reservation even when no rendered item does.

Far and Mid planes are broad decorative fields. They use aperture clipping and
the global displacement caps, and may continue behind fixed gauge/HUD chrome as
they do in the Classic composition.

Behind and Foreground planes contain discrete props and tank life. For these
planes, derive occupied geometry from actual `SmoothLayerItem::LocalCell` items,
treating each cell as a one-cell rectangle at `anchor + local position`. For
each occupied cell:

- a cell clear of a chrome reservation before motion must remain clear after
  motion; and
- a cell already intersecting chrome may not increase its intersection area.

Evaluate the proposed layer delta at deterministic safety scales
`[1.0, 0.75, 0.5, 0.25, 0.0]` and choose the first safe scale. This preserves one
translation for the whole layer while avoiding bounding-box false positives.
The plane-ordering requirement applies to raw translations before this local
safety attenuation.

All current moving object layers contain local cells. A future Shape or Raster
item may not receive a Behind or Foreground binding until it exposes explicit
occupied bounds; otherwise planning returns
`SmoothScenePlanError::InvalidParallaxGeometry`.

Circular aperture clipping remains intentional: a foreground glyph may be
cropped at the porthole rim just as Classic scene content is today. Parallax may
not move coordinates outside finite viewport math or bypass the aperture clip.

### Data flow

```text
WatchViewModel + now + CompanionMotion
  -> CompanionPetPlacement
       continuous motion anchor
       neutral continuous motion origin
  -> SmoothCompanionScenePlan
       typed layer motion bindings
       pet-follow focus offset
       resolved bounded layer translations
  -> PreparedCompanionFrame
  -> AppKit paints the prepared transforms
```

The resolver has no host-owned temporal state and no independent clock. A fixed
view model, timestamp, viewport, and motion config always produce the same
focus and layer transforms.

Classic flattening continues to ignore smooth-plan transforms through the
existing compatibility path. The same fixed inputs must therefore retain the
current Classic draw-list checksum.

## Error Handling

All resolver inputs and outputs must be finite. Invalid focus or geometry
returns `SmoothScenePlanError::InvalidParallaxGeometry`; it is not a panic and
not a NaN passed to AppKit.

After the implementation entry gate passes, production frame preparation uses
the fallible smooth planner. A parallax planning error follows the existing
prepared-frame contract: record the categorized preparation failure and retain
the last good frame. Error categories remain static and privacy-safe.

Unclassified future layer roles resolve to `Fixed` through the role-binding
helper. Missing pet body keeps the existing `MissingPetBody` error. Keep the
infallible compatibility wrapper for existing tests and deterministic Preview
Lab fixtures; production continues using the fallible path.

## Review Evidence

Extend smooth plan and motion artifacts with:

- scene-level `parallax_focus_offset`;
- lifecycle attenuation value;
- per-layer motion binding;
- per-layer depth plane when applicable;
- per-layer resolved parallax translation; and
- maximum adjacent-frame parallax delta by plane.

Preview artifacts must use typed values from the scene plan. They must not
recompute depth from glyphs, colors, z order, or rendered coordinates.

The deterministic smooth strip must:

1. include at least one frame with a non-zero focus offset;
2. show at least Far, Mid, Behind, and Foreground translations;
3. prove the expected plane-magnitude ordering;
4. cross at least one Classic snap boundary while parallax remains continuous;
5. keep every adjacent parallax delta below its named bound; and
6. show zero added translation for fixed and pet-attached layers.

Native review capture should record the same focus and per-plane translations
for the prepared smooth frame. The existing privacy scanner must cover every
new textual field. Numeric offsets, layer roles, and enum names are allowed;
source names, exact token strings, project names, paths, prompts, responses,
raw diagnostics, and unprojected pet seeds remain forbidden.

## Testing

### Unit coverage

- Neutral focus resolves to zero for every plane.
- Positive and negative focus values are directionally symmetric.
- Plane magnitudes remain strictly ordered for non-zero focus before a shared
  cap is reached.
- Horizontal and vertical caps are enforced independently.
- Calm and asleep attenuation use the specified factors.
- Asleep attenuation takes precedence when asleep and calm are both true.
- Fixed and pet-attached bindings add no parallax.
- Non-finite input returns a categorized error.
- Occupied-cell safety avoids aggregate-bounds false positives.
- Behind and Foreground safety prevents new chrome overlap and does not worsen
  existing overlap.
- Unknown future roles default to fixed behavior.

### Integration coverage

- Smooth scene plans assign every current role an explicit binding.
- Chest bubble and background props share the Behind plane.
- Pet body, contact shadow, performance cue, and mood aura remain one subject
  group and preserve their existing continuous anchor relationship.
- Classic flatten checksums remain exact with non-zero parallax transforms.
- AppKit draws a `0.1`-cell Parallax translation at a fractional pixel position
  instead of rounding it to the Classic cell.
- The implementation entry-gate test proves `drawRect` consumes only prepared
  frames before parallax work begins.
- Prepared-frame errors retain the last good frame through the existing
  boundary-health path.
- Preview Lab exports the new focus and layer-motion evidence.
- Privacy scans cover the extended artifacts.

### Live review

At the standard 960 px companion size:

- parallax is visible without staring for individual glyph changes;
- the pet's path, facing, bob, blink, and art cadence remain stable;
- foreground props and tank life move more than the room texture;
- no layer jumps when Classic snapped anchors change;
- props still feel attached to the habitat;
- HUD, gauges, status, trouble, and aperture chrome do not drift; and
- calm and asleep states are visibly quieter.

## Acceptance Criteria

- Running `glorp companion --renderer smooth` shows the existing Classic Glorp
  companion with subtle pet-follow tank depth.
- The effect has exactly one driver: the pet's existing continuous wander.
- At least four distinct scene depth planes are visible in plan evidence.
- Glorp and all pet-attached layers retain the stabilized motion from the prior
  slice without added parallax.
- Foreground displacement never exceeds `0.5` columns or `0.25` rows.
- Discrete Behind and Foreground items create no new chrome-reservation
  intersection and do not worsen an existing one.
- Classic flatten parity, smooth cadence checks, and privacy checks continue to
  pass.
- Production uses the fallible prepared-frame path and never panics because of
  parallax input.
- No scale, rotation, spring, pointer, event-reaction, or new-art scope enters
  the implementation.

## Expected Review Outcome

After this slice, opening the smooth companion should show the same Glorp and
the same tank, but the habitat should have a quiet sense of depth as Glorp moves.
The implementation should also leave a clean layer-motion boundary for later
visible features such as squash/stretch, velocity lean, rim light, and event
reactions without implementing any of them now.
