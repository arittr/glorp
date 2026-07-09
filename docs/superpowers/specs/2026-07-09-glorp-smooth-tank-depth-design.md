# Glorp Smooth Tank Bed And Depth Motion Design

**Date:** 2026-07-09
**Status:** Approved
**Surface:** macOS round companion using the Smooth renderer

## Goal

Give the round companion a readable sense of depth on its small circular display
without replacing Glorp's existing pet art, props, HUD, gauges, or free-swimming
motion. The visible feature is a curved tank bed plus forward/backward pet motion:
Glorp becomes smaller when farther away and larger when nearer.

This slice also establishes the renderer fundamentals needed to express those
effects honestly: typed shape primitives and working layer-scale transforms in
the platform-neutral Smooth scene plan.

## Problem And Root Cause

The first floor experiments reused terminal-cell primitives:

- Background-colored cells became large rectangular blocks because one logical
  row is roughly 40 physical pixels tall on the companion.
- Glyph-only dither became an unrelated footer because the pet, props, and tank
  life are authored as a free-floating aquarium with no shared horizon.
- The bottom three rows overlap the round surface's HUD reservation and are
  heavily cropped by the circular aperture.
- `SmoothLayerItem::Shape` and `SmoothLayerItem::Raster` are currently descriptive
  references; the AppKit Smooth backend renders only local cells.
- `SmoothTransform.scale` exists in the scene contract but is not applied by the
  AppKit backend.

The issue is therefore not a missing texture pattern. A convincing floor needs
sub-cell geometry, a curved relationship to the aperture, and one depth value
that drives pet scale and projection together.

## Locked Decisions

1. Use the **curved tank bed** direction from visual brainstorming.
2. Preserve the free-swimming aquarium composition; do not convert Glorp into a
   floor-bound walking pet.
3. Use the approved medium depth range: **0.88x far to 1.12x near**.
4. Derive pet scale, vertical perspective, and floor projection from one
   deterministic Z value.
5. Extend the platform-neutral Smooth scene contract. Do not special-case scene
   meaning in AppKit.
6. AppKit is the first rendering backend. A Linux companion is not part of this
   slice, but the scene plan must remain backend-neutral.
7. Keep Classic rendering unchanged. New tank-bed and Z effects are Smooth-only.
8. Keep `literal_floor_allowed: false` for round tank-life routing. The tank bed
   is a presentation surface, not permission for inhabitants to adopt literal
   floor routes.
9. Remove the failed glyph-based `FloorTexture` implementation rather than
   preserving compatibility for unshipped work.

## Non-Goals

- Rewriting the entire companion into the Pixel renderer
- Adding a Linux windowing backend
- Perspective-scaling HUD text, perimeter gauges, props, or tank inhabitants
- Dynamic prop occlusion or depth sorting based on Z
- Physically accurate lighting, blur, or ray-traced shadows
- Changing Classic companion output
- Changing pet identity, cast art, or habitat-prop art
- Adding settings or controls for depth amplitude in this slice

## Architecture

### 1. Typed Smooth Shape Primitives

Replace the name-only shape reference with typed, serializable geometry that can
be validated and rendered by any backend:

```rust
pub struct SmoothRgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub enum SmoothShapeGeometry {
    Ellipse { bounds: SmoothBounds },
}

pub struct SmoothShape {
    pub geometry: SmoothShapeGeometry,
    pub fill: SmoothRgba8,
}
```

`SmoothLayerItem::Shape` carries `SmoothShape`. Ellipses are sufficient for the
bed, its rim/value bands, sparse texture flecks, and the projected shadow. More
shape kinds are deferred until a real use case requires them.

Shape coordinates are local to their layer, just like local cells. Layer anchor,
transform origin, translation, scale, opacity, blend mode, and clip all apply to
both item kinds. Rotation remains in the model but is not used by this slice; a
Smooth plan validator must reject nonzero rotation until a backend contract for
it is implemented.

The AppKit backend renders ellipse shapes with `NSBezierPath` inside the existing
round-aperture clip. Preview artifacts serialize the same typed geometry. No
terminal glyph or cell-background fallback is allowed for a missing shape
backend.

### 2. Curved Tank Bed

Replace the unshipped `FloorTexture` role with `TankBed`.

The bed is fixed to the aperture and paints after the room wash/glyphs but before
ambient life, props, shadows, and the pet. Its geometry is derived from normalized
aperture dimensions rather than terminal rows:

- The visible bed occupies approximately the lower 24% of the aperture.
- A broad ellipse centered below the aperture creates a curved top edge.
- Two or three inset translucent ellipses establish value and depth without a
  gradient dependency.
- A bounded deterministic set of small ellipses provides quiet plum/teal texture.
- The circular aperture clip trims the bed naturally at the glass edge.

Texture placement is seeded only by biome identity and viewport dimensions. It
does not change with time, pet motion, activity, or redraw cadence.

The bed does not move with parallax or pet motion. HUD and gauge rendering remain
above the Smooth scene, exactly as today.

### 3. Deterministic Z Motion

Extend companion roam with a third deterministic target channel. Z uses the same
epoch and smooth interpolation as X/Y but a separately salted target so it is
not correlated with horizontal or vertical direction.

The normalized depth value is in `[-1.0, 1.0]`:

- `-1.0` is far.
- `0.0` is neutral.
- `1.0` is near.

Map depth to scale with:

```text
depth01 = (z + 1) / 2
pet_scale = 0.88 + depth01 * 0.24
```

Depth also produces a small vertical perspective offset: far moves slightly up
toward the bed horizon; near moves slightly down toward the viewer. The offset is
bounded to less than one logical cell across the full range so it cannot compete
with existing Y roam or idle bob.

The existing lifecycle scale attenuates depth around neutral:

- Normal/active: full 0.88x to 1.12x range
- Calm: half depth excursion
- Asleep: quarter depth excursion

This preserves life while preventing sleep from reading as rhythmic pumping.

### 4. Composed Pet Transform

The scene plan calculates one composed pet transform from:

1. X/Y fractional roam
2. Z-derived perspective translation
3. Z-derived uniform scale
4. Existing idle bob

The shared pivot is the pet body's visual center. The same translation and scale
are applied to pet-attached content:

- `PetBody`
- `WallShadow`
- `PerformanceCue`
- the actual mood-aura draw path

The floor projection does not inherit pet scale directly. It receives its own
shape geometry derived from Z.

AppKit must apply the layer transform to glyph positions and glyph size as one
group so the pet's cells do not separate during scaling. The implementation may
use a graphics-context affine transform or equivalent explicit coordinate math,
but the observable contract is one coherent layer transform around the declared
origin.

### 5. Bed Projection

Keep the `FloorProjection` role but replace its background-cell trapezoid with a
typed ellipse shape.

The projection:

- Tracks the pet's horizontal center.
- Sits on the curved bed surface, not on the wall or HUD plane.
- Moves toward the bed horizon and becomes smaller/fainter when Z is far.
- Moves toward the near bed edge and becomes broader/darker when Z is near.
- Remains below all prop and tank-life layers so it never clips in front of an
  object.
- Does not inherit idle bob.

This is an art-directed depth cue rather than a physically simulated shadow.
Its purpose is to connect pet depth to the bed at a glance.

### 6. Bounds And Protected Regions

Pet placement must reserve for the maximum 1.12x scale, not only the current
frame's scale. X/Y roam bounds therefore shrink slightly so the largest pet frame
cannot touch the aperture rim, perimeter gauges, or bottom HUD reserve.

The final fractional bounds and mood-aura bounds are calculated after scale and
perspective translation. All geometry must remain finite and nonnegative.

Props and tank inhabitants retain their existing fixed depth planes. Dynamic
occlusion based on pet Z is deferred.

## Data Flow

```text
WatchViewModel + time + CompanionMotion
        |
        v
deterministic XYZ roam targets
        |
        v
pet placement + depth value + max-scale-safe bounds
        |
        v
Smooth scene plan
  - typed TankBed shapes
  - composed pet transforms
  - typed FloorProjection shape
        |
        +--> AppKit backend (first implementation)
        |
        +--> Preview Lab typed artifacts
        |
        `--> future Linux/native backend
```

## Error Handling

- Reject non-finite anchors, bounds, translations, or scales while building the
  fallible Smooth scene plan.
- Reject nonpositive scale values.
- Omit bed and projection shapes for degenerate apertures that cannot contain
  valid ellipses; retain the pet and HUD without crashing.
- Reject unsupported nonzero rotation during Smooth plan validation, before the
  draw callback receives the frame.
- Do not fall back to cell backgrounds or glyph texture when shape rendering is
  unavailable.
- The new geometry must not introduce panics in the native draw callback.

## Preview And Review Contract

Preview Lab adds typed evidence for:

- `TankBed` shape count, bounds, fill colors, clip, and Z order
- `FloorProjection` ellipse geometry at far, neutral, and near depth
- pet depth value, composed scale, perspective translation, and final bounds
- maximum-scale protected-region clearance
- absence of `FloorTexture` cell backgrounds and glyph dither

Deterministic native review captures cover at least 360x360 and 720x720 logical
review sizes. The visual review must inspect far, neutral, and near frames rather
than relying on one animation screenshot.

## Acceptance Criteria

### Visual

- The floor reads as a curved tank bed, not a footer, HUD bar, or terminal row.
- No full-width cell rectangles or glyph-rain substrate remain.
- The bed remains visible at the small companion size without competing with the
  pet, HUD, gauges, or props.
- Glorp clearly moves forward/backward at the approved 0.88x to 1.12x range.
- Scale changes read as depth, not inflation: perspective Y and projection change
  in the same direction and on the same easing curve.
- The projected shadow never paints in front of props or tank inhabitants.
- The pet remains fully inside all protected regions at maximum scale.
- Calm and asleep states retain quieter depth motion.

### Structural

- The scene plan contains typed shape geometry with no AppKit types.
- AppKit renders shape items and coherent layer scale transforms.
- Preview artifacts expose the same geometry and transforms.
- Classic output and code paths are unchanged.
- Round tank-life routing still reports `literal_floor_allowed: false`.
- No compatibility layer preserves the failed unshipped `FloorTexture` role.

### Verification

- Pure tests prove deterministic and continuous Z interpolation.
- Scale never leaves `[0.88, 1.12]` in normal mode.
- Adjacent animation samples have bounded translation and scale deltas.
- Maximum-scale pet bounds avoid aperture, gauge, and HUD reserves.
- Bed and projection shapes have valid finite geometry and approved Z order.
- Nonzero layer scale changes native output while fixed HUD/gauge geometry stays
  unchanged.
- Preview sidecars remain sanitized and deterministic.
- Native capture checks prove nonblank bed pixels and distinct far/near pet
  extents at both review sizes.
- A multi-second Smooth companion smoke run completes without a crash.

## Scope Boundary With Earlier Specs

The June round-companion design described a free-float tank with no floor, and
the ambient tank-life plan prohibited a literal substrate. This design narrows
that rule:

- The pet remains free-swimming.
- Tank-life route selection still has no literal floor.
- The Smooth external companion gains a curved presentation bed as a depth cue.
- Classic and shared terminal surfaces retain their existing floor semantics.

This is an intentional Smooth-only evolution, not a retroactive change to every
surface.
