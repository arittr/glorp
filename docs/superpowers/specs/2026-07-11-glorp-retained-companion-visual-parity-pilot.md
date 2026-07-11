# Glorp Retained Companion Visual-Parity Pilot - specification

- Date: 2026-07-11
- Status: approved pilot direction
- Scope: hidden, non-default macOS retained renderer
- Visual oracle: current Smooth AppKit companion

## Context

Glorp now has a real production-path `wgpu`/Metal companion prototype behind the
non-default `retained-renderer` feature. It consumes the existing
`SmoothCompanionScenePlan`, attaches a `CAMetalLayer`, renders glyph and shape
instances, draws perimeter gauges and HUD text, and falls back to Smooth after a
retained-renderer failure.

The prototype proves that live pet state can reach the GPU pipeline, but it does
not yet preserve the established companion art. The pet, habitat, gauges, and HUD
are present while their rasterization and layout differ materially from Smooth.
The pilot therefore treats visual parity as the next product gate. It does not add
new 2.5D effects or attempt final renderer qualification.

## Goal

Make the hidden retained companion reproduce the recognizable generated-pet art
and established round-companion composition closely enough for direct human
comparison with Smooth, while retaining the production GPU host and avoiding
native text work in the per-frame hot path.

## Product invariants

The retained output must preserve the current hierarchy:

1. Glorp is the hero and remains recognizable as the same generated pet.
2. Pet glyph structure, palette, silhouette, eyes, mouth, and species/stage traits
   remain intact.
3. Props and tank life remain legible evidence of habitation and history.
4. Bed, wall shadow, projection, tank falloff, aura, clipping, blend order, and
   parallax preserve the established depth composition.
5. HUD and perimeter gauges remain secondary, centered, and glanceable.
6. Smooth remains the default and explicit fallback throughout this pilot.

## Current parity defects

### Glyph atlas and placement

The prototype rasterizes every glyph into a fixed 64x64 slot, applies a fixed
sample inset, and stretches that sampled rectangle across the destination cell.
That discards the font's actual advance, baseline, bearing, and ink bounds. Linear
sampling further softens or spreads the established cell art.

The retained resource compiler must instead record per glyph/style:

- atlas pixel rectangle;
- ink width and height;
- horizontal and vertical bearing relative to the Smooth cell origin;
- baseline/ascent/descent information needed to reproduce Smooth placement;
- source raster font size and style;
- safe texture padding independent of visible ink;
- normalized UV rectangle containing the ink, not an arbitrary fixed crop.

A glyph quad is positioned from those metrics inside the transformed cell. It is
not stretched to fill the whole cell. Pet depth scaling scales the completed quad
and its placement; it does not rerasterize the glyph each animation frame.

### Resource lifetime

The current prototype rebuilds, creates, and uploads the glyph atlas on every
paint. The pilot must cache atlas pixels, texture, sampler, view, and bind group by
a deterministic resource key. Ordinary animation frames may update instance data
but must not repeat native glyph rasterization or atlas upload. A semantic/layout
change may compile a replacement atlas before activation.

### Tank background

Smooth uses a deterministic dithered radial falloff. The retained shader currently
uses an undithered gradient. The retained path must eventually implement the same
core/rim interpolation and deterministic output-level noise, either in a cached
texture or a pixel-equivalent shader.

### HUD

Smooth measures attributed lines using actual font metrics. The retained path
currently estimates width from character count. HUD layout must use precomputed
run metrics produced with the same font policy as the atlas. Per-frame rendering
uses glyph-run instances and no native text measurement.

### Gauges

Smooth uses exact arc geometry, stroke width, and configured cap styles. The
retained path currently approximates arcs with overlapping ellipse instances.
The pilot must share backend-neutral tessellated or analytic arc data so retained
and Smooth consume the same start/end angles, overfill marker, width, and caps.

### Primitive coverage

The retained translator must never silently reinterpret an unsupported primitive.
Raster items are either implemented faithfully or trigger an explicit fallback
with a bounded diagnostic. The pilot does not drop pet, prop, tank-life, HUD, or
privacy content to keep retained rendering active.

## Architecture boundary

The pilot adapts the proven Smooth/round projection. It does not import
`renderer_spike` DTOs and does not create a second product-state derivation.

```text
WatchViewModel
  -> existing round/Smooth preparation
  -> SmoothCompanionScenePlan + chrome data
  -> retained resource compilation (semantic/layout generation)
  -> retained frame instances (animation/content changes)
  -> wgpu backend
```

The current single-file prototype may be split only where the first parity slice
needs a clear, testable boundary. A broad retained scene graph rewrite is outside
this pilot.

## Review contract

### Required fixtures

Use deterministic, privacy-safe fixtures covering at least:

- one representative generated pet with eyes, mouth, foreground and background
  cell ink;
- one bold glyph;
- one fractional-motion/depth-scaled pet frame;
- habitat props and tank life;
- all three HUD lines;
- perimeter gauge track, fill, and overage marker;
- normal and dimmed composition.

### Review sizes

The eventual pilot parity review covers 260, 360, 480, and 720 logical pixels.
Routine implementation iterations must not regenerate the full matrix. Use one
representative 360 capture while developing, then run the four-size capture set
once at the final parity checkpoint.

### Evidence

Each accepted capture records renderer mode, logical/physical size, backing scale,
fixture identity, and image path. A side-by-side review must make it possible to
compare the same state in Smooth and Retained.

Pixel equality is not required across AppKit and Metal. Acceptance is based on
preserved art identity plus focused geometry assertions:

- pet anchor and bounds;
- glyph baseline/bearing within declared tolerance;
- layer order and clipping;
- HUD run bounds and centering;
- gauge start/end/width/cap geometry;
- aperture and dim-overlay coverage.

## Bounded verification policy

This pilot explicitly avoids repeating the renderer-decision program's long
measurement matrices.

Routine change verification is limited to:

1. focused pure/unit tests for atlas metrics, UVs, quad placement, resource keys,
   and geometry;
2. `cargo check` or the narrowest relevant test target with
   `--features retained-renderer`;
3. one bounded, automatically exiting 360 review capture when visual evidence is
   needed;
4. formatting and diff checks.

Prohibited during routine pilot work unless the user separately approves it:

- five-minute renderer repetitions;
- the prior 12-run matched performance matrix;
- privileged energy collection;
- broad release/package qualification;
- unattended capture loops or manual windows without a bounded exit;
- rerunning all four review sizes after every change.

A routine verification command should target completion within two minutes on the
current development machine. If a command exceeds that bound, stop it, preserve
its output, and replace it with a narrower check. Final parity review may take
longer only through a small, declared one-shot capture set; it must not become a
performance soak.

## Pilot slices

1. Glyph metrics, placement, sampling, and atlas lifetime.
2. Deterministic Smooth/Retained capture seam and representative 360 comparison.
3. Dithered tank-background parity.
4. Shared HUD run metrics and placement.
5. Shared exact gauge geometry.
6. Four-size final visual review, focused fallback checks, and pilot disposition.

## Acceptance

The pilot succeeds when:

- the same generated pet is immediately recognizable in Smooth and Retained;
- glyphs no longer appear stretched, fragmented, or arbitrarily aligned;
- ordinary animation frames perform no glyph rasterization or atlas recreation;
- tank, HUD, and gauges preserve the established composition;
- unsupported content fails explicitly to Smooth rather than disappearing;
- focused tests and the bounded capture contract pass;
- Smooth remains default and no new visual feature is introduced.
