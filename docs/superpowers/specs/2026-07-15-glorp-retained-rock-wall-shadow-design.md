# Glorp Retained Rock Wall and Dark Shadow Design

## Summary

The retained companion tank currently lets its rear wall approach black at dark
day phases. Because that wall loses most visible contrast on darker displays,
the rear pet silhouette is authored as a translucent violet lift rather than a
dark shadow. The result reads as an infinite void with a light projection.

Raise the tank wall into a dark, readable biome-tinted slate range, add quiet
logical-coordinate rock strata and mineral grain, and recolor the existing rear
silhouette so it darkens the wall. The tank remains a dark environment; the
change restores physical surface cues rather than turning it into a bright
room.

## Goals

- Make empty wall areas read as a physical tank or rock surface instead of
  featureless black.
- Keep the wall visibly dark across every biome and day phase.
- Add medium-scale stone strata with restrained mineral grain that remains
  stable across animation, resize, fullscreen, and backing-scale changes.
- Make the existing rear pet silhouette darker than the wall instead of
  lifting it.
- Preserve the current depth-dependent shadow offset and strength behavior.
- Keep the work inside the existing analytic room and wall-shadow draws.

## Non-Goals

- A bright aquarium, cave mural, photographic rock, or high-contrast patterned
  backdrop.
- Changes to the ground texture, pet floor projection, mood aura, status halo,
  props, gauges, HUD, or pet art.
- A texture asset, extra render pass, new bind group, buffer, uniform, pipeline,
  resize-owned resource, mesh, camera change, or scene ABI change.
- New user settings or biome configuration.

## Current Behavior

`tank_background_paint_srgb8` derives the rear wall from the phase-dimmed biome
background and a fixed depth tint. At dusk and night, the darkest wall values
can approach display black. `WALL_SHADOW_SRGB8` is therefore a light violet,
and the retained wall-shadow glyph uses premultiplied source-over blending to
lift the wall. Native readback coverage explicitly asserts this lift.

The retained room already owns a single analytic aperture fragment. Its ground
texture uses deterministic absolute logical coordinates and is verified at 1x
and 2x. That is the correct implementation seam for wall surface variation.

## Approved Visual Treatment

### Dark but readable wall palette

Apply a tank-local ambient toe lift after phase dimming and before the existing
core/rim depth mix. Do not brighten the shared application background palette.
The lift preserves the biome hue and day-phase ordering while preventing the
tank wall from collapsing into black.

The target envelope is:

- Night and dusk remain the darkest phases, with ordinary rear-wall channels
  generally in the low `20` to high `30` sRGB8 range.
- Dawn and day remain brighter than night, with ordinary rear-wall channels
  generally in the high `20` to low `50` range.
- Local rock highlights remain below the ground, pet, props, gauges, and HUD.
- No supported biome or phase produces a flat black wall or a gray, washed-out
  wall.

These are visual envelopes, not a requirement that every channel fit the range;
biome hue and the violet depth component may place an individual channel just
outside it. Relative luminance and hierarchy are authoritative.

### Approved real-display wall calibration

The first optimized implementation proved the ground, strata, and mineral
structure on the normal display, but its rear wall still collapsed toward black
in the live companion. Raise only the tank-local ambient lift from
`[0.025, 0.028, 0.040]` to `[0.050, 0.055, 0.070]`.
This real-display calibration supersedes the initial numeric envelope above.

This is a palette calibration, not a shader redesign:

- Keep the wall rock field, strata, mineral grain, horizon gate, ground mix,
  and every texture amplitude and scale unchanged.
- Keep the retained shadow tint, authored opacity, source-over path, and Smooth
  multiply factor unchanged. The brighter wall should increase the visible
  separation from the already-dark retained shadow.
- Keep the shared application background and all ground, prop, pet, gauge, and
  HUD paint unchanged.
- Keep night and dusk darker than dawn and day. Ordinary night wall channels
  should land roughly in the low `20` to mid `40` range; ordinary day wall
  channels should land roughly in the high `20` to mid `50` range.
- The wall must remain visibly darker and quieter than the textured ground and
  foreground content. It should read as dark slate rock, not a lit backdrop.

The implementation changes only `TANK_WALL_AMBIENT_LIFT_SRGB` and the palette
behavior expectations that directly encode its approved output. Retained native
wall-variance, wall-below-ground, dark-shadow, ABI, and resource-preservation
tests remain unchanged and must stay green.

### Medium rock strata and mineral grain

Add a wall-only logical texture to the existing room fragment. It has three
quiet components:

1. A broad two-dimensional stone field that breaks up large empty regions.
2. Soft, horizontally biased strata whose position is perturbed by the broad
   field so they do not read as regular stripes.
3. Sparse, low-opacity mineral grain that gives the wall a tank-rock surface
   without competing with foreground marks.

All terms use absolute logical tank coordinates and dedicated salts. They do
not use physical pixels, backing scale, normalized viewport coordinates, frame
time, or random state. The texture is stable between frames and has the same
logical structure at 1x and 2x.

The wall texture is gated above the curved bed horizon and fades through the
horizon feather. Ground substrate texture remains authoritative below that
transition. Wall variation should remain lower contrast than the ground's
granular texture: broad changes are visible in empty wall regions, while strata
and grain become secondary behind the pet, aura, props, and HUD.

### Dark rear silhouette

Keep one renderer-neutral wall-shadow semantic with compositor-specific color
encodings. The retained source-over path changes from the current light violet
to a very dark neutral-violet. The smooth path keeps its existing restrained
multiply factor; reusing the near-black retained tint as a multiply factor would
black out the scene. Keep both existing blend paths, the glyph-mask draw,
authored opacity, depth-dependent detach offset, and depth-dependent strength.

The dark source-over tint must lower wall luminance wherever the shadow has
coverage. It must remain translucent enough to retain the wall texture and pet
silhouette detail rather than becoming a flat black sticker. The darkest wall
and strongest shadow combination must still preserve visible separation on the
Napster display.

The shared companion-effects module owns both encodings so retained and the
manually selected smooth renderer retain the same semantic direction: wall
shadows darken rather than glow. The retained native readback is the pixel-level
acceptance path.

The mood aura and status halo are independent light effects and do not change.

## Architecture and Data Flow

1. The companion scene compiler continues to request biome- and phase-aware
   `ApertureDepth` paint from `tank_background_paint_srgb8`.
2. That helper applies the tank-local ambient lift and returns the same packed
   core/rim fields. No paint or payload field is added.
3. `fs_room_aperture` consumes the packed colors, computes wall texture in
   absolute logical coordinates, fades it out at the bed horizon, then applies
   the existing bed mix and ground texture.
4. The existing retained `WallShadow` analytic glyph draw consumes the darker
   source-over tint. The smooth renderer continues to consume its existing
   multiply factor. Neither compositor changes pipeline or blend mode.
5. All existing room, shadow, depth, ordering, validation, checksum, and resize
   ownership boundaries remain unchanged.

## Approaches Considered

### Approved: palette lift plus analytic rock texture

This keeps the new surface cues in the existing room pass and changes only
paint values plus fragment math. It preserves resize behavior and makes the
dark shadow possible without new renderer resources.

### Rejected: multiply-blended wall shadow

Multiply blending would produce a physically familiar shadow but would require
pipeline/blend-contract changes for a result available through the existing
dark source-over tint. That added surface area is not justified.

### Rejected: image or procedural texture resource

A texture asset or separately generated resource provides art-directable rock
detail but adds scaling, packaging, binding, and resource-lifecycle work. The
desired quiet strata and grain do not need it.

## Verification

### Pure palette and texture behavior

- Every supported biome and phase preserves biome identity and phase ordering.
- Tank wall paint is lifted from the current near-black values but remains
  below ground and foreground luminance.
- Wall texture samples are deterministic at identical logical coordinates.
- Wall texture samples are invariant to backing scale.
- Broad field, strata, and mineral terms are present above the horizon and zero
  below the bed transition.

### Retained native readback

- Repeated wall-only renders are byte-stable at 1x and 2x.
- Equal logical wall ROIs retain correlated structure after backing-scale
  normalization.
- An empty upper-wall ROI has visible low-frequency structure without excessive
  high-frequency variance.
- The bed ROI remains governed by the existing substrate texture.
- Shadowed wall pixels have lower linear luminance than the matching unshadowed
  pixels and retain the authored dark-violet bias.
- The shadow remains visibly translucent rather than replacing the wall with a
  solid shape.
- The smooth wall shadow keeps its current bounded multiply factor and remains
  neither invisible nor black.

### Preservation

- Existing room paint packing, analytic slot identities, scene ABI, checksums,
  validation, draw order, and GPU resource accounting remain unchanged.
- Existing floor projection, prop shadow, ground texture, aura, halo, HUD, and
  full-cast composition coverage remains green.
- Preview Lab round fixtures remain deterministic.

### Human QA

Use the optimized retained companion on the normal and Napster displays:

1. Empty tank regions read as dark rock, not a black void.
2. Strata and mineral grain are visible but do not compete with the pet or HUD.
3. The rear silhouette is darker than the wall and still preserves texture.
4. Day/night changes retain their ordering without making night unreadable.
5. Resize, fullscreen, display movement, and animation do not shift or shimmer
   the wall texture.
