# Glorp Retained Habitat Composition Parity Design

**Date:** 2026-07-15
**Status:** Approved
**Surface:** macOS round companion using the direct retained scene runtime

## Goal

Make the retained companion habitat read as one composed 2.5D scene rather than
a collection of flat glyphs. Preserve Glorp's existing prop inventory, authored
sprites, tank inhabitants, animation meanings, pet art, HUD, and perimeter
gauges while restoring the placement, substrate texture, depth separation, and
grounding cues that did not survive the direct-runtime cutover.

This is a composition-parity pass on the existing retained architecture. It is
not a renderer rewrite.

## Problem And Root Cause

The direct runtime carries the important semantic data:

- the same visible trophy and accent selection;
- the canonical prop animation states;
- the complete prop and tank glyph repertoires;
- authored background, behind-pet, and foreground depth buckets;
- canonical tank routes and layer crossings; and
- a retained scene with real world Z, blend ordering, analytic shapes, and
  persistent GPU resources.

The visual result is still incomplete because four composition decisions were
replaced or omitted during cutover:

1. **Prop placement was simplified.** The direct projection maps each authored
   zone to one normalized anchor and offsets it by `stable_order % 3`. It does
   not know sprite footprints, occupied rectangles, the circular safe aperture,
   the center HUD stack, or perimeter gauge lanes. Multiple props therefore
   stack in one zone or land underneath chrome.
2. **The textured tank bed was flattened.** Smooth used a biome-tinted curved
   bed with deterministic flecks. The direct room shader uses one smooth bed
   fade and loses the substrate texture and much of the horizon cue.
3. **World depth is structural but visually weak.** Props and tank inhabitants
   retain Z ordering, but they lost the old depth-plane parallax. Non-pet nodes
   use neutral depth cues, and grounded props have no contact shadows.
4. **Tank routing does not consume the whole composition.** It respects the
   aperture, bottom reserve, and foreground pet-face reserve, but not prop
   footprints, the center HUD safe area, or the perimeter gauge annulus.

The previous direct-runtime qualification deliberately did not claim full-cast
human visual approval. This pass closes that missing product gate rather than
reopening the retained renderer's lifecycle or performance architecture.

## Product Outcome

At normal companion sizes:

- every visible multi-cell prop reads as one object;
- no two visible prop footprints overlap;
- no prop or foreground tank inhabitant is hidden under the HUD or perimeter
  gauges;
- floor props sit on the substrate with a quiet contact shadow;
- background, behind-pet, and foreground content separate through restrained
  parallax and tonal depth cues;
- the lower tank has a biome-specific curved gradient and stable texture;
- resize recomputes a deterministic composition without transient stacking;
- semantic animation changes glyphs inside frozen placement footprints; and
- HUD, gauges, pet identity, privacy, and existing motion remain unchanged.

## Locked Decisions

1. Keep `CompanionSceneSnapshot` as the direct route's sole world-scene
   authority. The renderer does not consume TUI draw cells or a Smooth scene
   plan.
2. Move canonical prop sprite selection and maximum footprints into a shared
   presentation module. TUI and direct projection consume that same art source;
   neither keeps a parallel catalog match.
3. Resolve placement in logical glyph-cell space, then project the accepted
   anchors into logical points. Backing scale never affects composition.
4. Use deterministic authored-zone candidate slots plus exclusions. Do not add
   a physics solver, unconstrained packing algorithm, or per-frame repacking.
5. Freeze each prop's placement against its maximum authored footprint across
   sprite phases, twinkle states, chest poses, bloom states, and species
   dialects. A semantic animation cannot move neighboring props.
6. Props remain independent of the wandering pet. The pet may occlude
   background/behind props and pass beneath foreground props; props do not chase
   the pet.
7. Reserve a conservative, value-independent center HUD region and the inner
   boundary of the real perimeter gauge layout. The composition model never
   receives private HUD strings or exact usage values.
8. Prefer hiding an unplaceable lowest-priority accent over stacking it. Trophy
   order stays deterministic, and hidden slots remain represented in fixed scene
   capacity with `visible = false`.
9. Preserve canonical tank cast, sprite, route, cadence, and behind/foreground
   layer semantics. Only safe geometry and subtle depth-plane offsets change.
10. Restore the bed inside the existing room analytic. Add deterministic dither
    and sparse substrate flecks; do not add a texture asset or animated noise.
11. Add one scene-native analytic prop-shadow field backed by the existing fixed
    prop frame slots. Do not issue one draw call or allocate one resource per
    prop.
12. Keep all new motion bounded, deterministic, monotonic-time driven, and
    disabled or neutralized by the existing reduce-motion/lifecycle policies.

## Non-Goals

- No PBR, shadow maps, dynamic lights, bloom, depth of field, or post-processing
  stack.
- No meshes, sprite-sheet assets, imported textures, or new prop artwork.
- No general ECS, constraint engine, or arbitrary scene-graph authoring API.
- No change to inventory unlocks, trophy/accent selection, daily tank cast, or
  persistence formats.
- No pixel equality with Smooth or the watch TUI.
- No redesign of the HUD, gauges, pet motion, mood aura, or accessibility tree.
- No removal of Smooth fallback or retained rollout/lifecycle work.
- No live-state-dependent layout decisions.

## Architecture

### 1. Canonical Prop Art And Frozen Footprints

`src/presentation/props.rs` becomes the shared presentation owner for prop art:

```rust
pub(crate) struct PresentationPropVisualState {
    pub(crate) species: Species,
    pub(crate) sprite_phase: Option<u8>,
    pub(crate) twinkle_active: Option<bool>,
    pub(crate) chest_lid_open: Option<bool>,
    pub(crate) bloom_active: Option<bool>,
}

pub(crate) struct PresentationPropSpriteCell {
    pub(crate) dx: i8,
    pub(crate) dy: i8,
    pub(crate) glyph: char,
}

pub(crate) struct PresentationPropFootprint {
    pub(crate) min_dx: i8,
    pub(crate) max_dx: i8,
    pub(crate) min_dy: i8,
    pub(crate) max_dy: i8,
}

pub(crate) fn presentation_prop_sprite(
    catalog_id: &str,
    state: PresentationPropVisualState,
) -> Option<Vec<PresentationPropSpriteCell>>;

pub(crate) fn presentation_prop_max_footprint(
    catalog_id: &str,
) -> Option<PresentationPropFootprint>;
```

The existing TUI trophy renderer and direct scene compiler adapt these cells to
their own paint records. Color, reaction glow, and boldness remain surface paint
concerns. Glyph identity and local coordinates have one owner.

The maximum footprint is the union of every valid authored state for one
catalog ID. Tests enumerate every species and semantic phase to prove the
resolved sprite always fits.

### 2. Companion Composition Model

Add `src/presentation/companion_scene/composition.rs`. It owns pure,
platform-neutral placement and safe geometry:

```rust
pub(crate) struct CompanionCompositionInput<'a> {
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) width_points: f32,
    pub(crate) height_points: f32,
    pub(crate) bottom_reserved_rows: u16,
    pub(crate) props: &'a [PropTopologySnapshot],
}

pub(crate) struct CompanionPropPlacement {
    pub(crate) slot: u8,
    pub(crate) visible: bool,
    pub(crate) anchor_cell: [i16; 2],
    pub(crate) bounds_cells: [i16; 4],
    pub(crate) footprint_cells: [u16; 2],
    pub(crate) grounded: bool,
}

pub(crate) struct CompanionComposition {
    pub(crate) prop_placements: Vec<CompanionPropPlacement>,
    pub(crate) hud_reserve_cells: [i16; 4],
    pub(crate) gauge_inner_radius_cells: [f32; 2],
    pub(crate) tank_reserved_regions: Vec<TankRouteRect>,
    pub(crate) tank_foreground_reserved_regions: Vec<TankRouteRect>,
}

pub(crate) fn resolve_companion_composition(
    input: CompanionCompositionInput<'_>,
) -> CompanionComposition;
```

Candidate order comes from authored `HabitatPropZone` and mirrors the proven
left/mid/right, wall, air, and ceiling alternatives. Every candidate is tested
against:

- the glyph grid;
- the ellipse inside the innermost perimeter gauge lane with half a cell of
  padding;
- the center HUD reserve;
- the existing bottom reserve; and
- accepted prop bounds expanded by a one-cell trophy gutter.

The HUD reserve is a fixed centered composition region, not measured private
text: 58% of the logical width, from 58% through 90% of logical height. The
gauge safe radius is derived from `perimeter_gauge_layout`, including the
innermost lane's half stroke and half a glyph cell of breathing room.

The solver is greedy and deterministic in the already-canonical visible order.
It tries every authored-zone candidate before hiding a slot. It never moves an
accepted prop to respond to pet motion or animation state.

### 3. Snapshot And Fixed-Slot Projection

Extend `PropFrameSnapshot` and scene `PropFrameSlot` with:

```rust
pub visible: bool,
pub footprint_points: [f32; 2],
pub contact_shadow_strength: f32,
```

`project_prop_frame_states` receives one frozen `CompanionComposition`. It
converts cell anchors to points and combines authored sway/hover/two-pose motion
with a bounded depth-plane parallax offset. Hidden props retain content slots
but project `visible = false` and zero shadow strength.

The existing fixed GPU prop frame ABI has three unused float lanes. Pack width,
height, and shadow strength into those lanes; do not grow per-frame storage.

### 4. Depth Treatment

Authored depth buckets remain the sorting authority. Add restrained visual cues:

| Authored depth | Parallax multiplier | Opacity | Saturation |
|---|---:|---:|---:|
| Background | 0.010 | 0.82 | 0.78 |
| Behind pet | 0.030 | 0.94 | 0.90 |
| Foreground | 0.045 | 1.00 | 1.05 |

Parallax uses the pet's current point-space displacement from its motion origin,
is bounded to half a glyph cell per axis, and is attenuated by the existing
normal/calm/asleep lifecycle scale. Reduce motion resolves every non-semantic
parallax offset to zero.

Background/behind/foreground node depth cues carry opacity and saturation with
`scale = 1.0`; props do not resize around the global origin. The same parallax
multipliers apply to tank cells according to their resolved layer.

### 5. Prop Contact Shadows

Add `AnalyticSemantic::PropShadows` at analytic slot 8 and
`AnalyticShape::PropShadowField`. It renders after the bed and pet floor
projection but before behind-pet props and tank life.

One full-room analytic fragment evaluates the ten fixed prop frame slots. It
draws a soft multiply ellipse only when:

- the slot is visible;
- the prop is in a floor zone; and
- `contact_shadow_strength > 0`.

The ellipse is centered under the resolved footprint, uses 75% of footprint
width with a minimum one-cell radius, and uses 0.30 cell height. Behind-pet floor
props use strength `0.24`; foreground floor props use `0.34`; background and
non-floor props use zero. Sleep/dim opacity still applies through the ordinary
node/frame opacity path.

The shadow color comes from the biome's existing `bed_shadow_srgb8` helper and
uses the existing multiply blend pipeline. No shadow map, blur texture, or
per-prop draw is introduced.

### 6. Textured Retained Bed

Extend `AnalyticPaint::ApertureDepth` with `bed_srgb8` and
`fleck_srgb8`. `fs_room_aperture` keeps the current radial core-to-rim falloff,
then adds:

- the existing deterministic per-pixel dither hash;
- a curved lower-bed mask beginning near 76% of logical height;
- a lifted biome bed color; and
- sparse deterministic flecks whose density and strength increase toward the
  near edge.

Hash inputs use logical point coordinates plus backing scale only to stabilize
physical-pixel grain. Time, semantic revision, frame revision, pet position, and
usage values are forbidden inputs. The same layout and biome therefore produce
stable texture across redraws.

### 7. Tank Safe Routing

The canonical tank resolver remains unchanged in meaning. Before route
resolution, direct projection augments `TankRouteGeometry` from the companion
composition:

- all layers avoid the bottom reserve and HUD reserve;
- all cells remain inside the gauge-safe aperture;
- foreground cells avoid accepted foreground prop footprints and the pet-face
  reserve; and
- behind cells may travel behind foreground obstacles but still avoid chrome.

If final clipping removes every cell, the inhabitant is honestly hidden for
that route sample. The resolver does not teleport it into an unrelated route.

## Invalidation And Runtime Behavior

- Layout/grid/visible-cast changes recompute composition and may require a new
  layout generation under existing rules.
- Semantic sprite-phase, twinkle, lid, bloom, mood, usage, and clock changes do
  not recompute composition.
- Presentation ticks update only motion offsets, tank interpolation, opacity,
  shadow strength, and existing frame values.
- Backing-scale changes rebuild raster resources but preserve logical
  composition.
- Resize resolves one new deterministic composition for the new grid; no stale
  old-size placement may be presented to the new surface.

## Verification Contract

### Deterministic layout

For full-cast fixtures at 260, 360, 480, and 720 logical points, plus 480x360 and
360x480:

- every visible prop footprint is finite and inside the gauge-safe aperture;
- visible prop bounds are pairwise disjoint;
- visible props do not intersect HUD or bottom reserves;
- repeated projection is byte-for-byte stable;
- changing only sprite phase does not change placement; and
- smaller surfaces hide accents deterministically instead of stacking.

### Art and depth

- Every canonical prop sprite state fits its maximum footprint.
- TUI and direct adapters emit the same glyph/local-cell pairs for every catalog
  state.
- Background, behind, and foreground props preserve authored Z ordering.
- Normal motion produces bounded depth-plane parallax; reduce motion produces
  none.
- Only visible floor props contribute contact-shadow coverage.

### GPU output

- The lower bed ROI has stable nonzero texture variance beyond the smooth
  gradient reference.
- The upper room ROI does not receive substrate flecks.
- Contact-shadow pixels darken the bed through multiply blending without
  changing prop glyph color.
- Full-cast prop and tank ROIs are nonblank at 1x and 2x.
- HUD/gauge pixels remain unchanged by the habitat pass.

### Manual review

Launch the optimized direct runtime and review normal, full-cast, resized,
non-square, external-display, and fullscreen states. Approval requires readable
individual props, a textured substrate, quiet shadows, no chrome collisions,
and no new motion jitter or fallback.

## Delivery Slices

1. Canonical prop art and frozen footprints.
2. Shared composition placement and safe regions.
3. Tank integration and depth-plane motion.
4. Retained bed texture.
5. Scene-native prop contact shadows.
6. Deterministic/native visual gates and manual approval.

Each slice is independently testable and commit-worthy. None changes the
retained host, surface lifecycle, renderer selection, or fallback policy.
