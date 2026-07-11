# Glorp Retained Companion Default Cutover - design

- Date: 2026-07-11
- Status: approved design direction
- Scope: complete retained visual parity and make wgpu/Metal the reversible
  Apple-Silicon companion default
- Visual oracle: the live Smooth AppKit companion

## Context

Glorp has a real macOS retained-renderer prototype behind the non-default
`retained-renderer` feature. It attaches a `CAMetalLayer`, presents through
`wgpu`/Metal, consumes the existing `SmoothCompanionScenePlan`, renders generated
pet and habitat content, owns a native-rasterized glyph atlas, draws companion
chrome, and can fall back to Smooth.

The prototype proves the production host and rendering path, but it is not ready
to become the default. The remaining work is concentrated in five areas:

1. Metal output cannot yet be captured truthfully by the existing AppKit review
   screenshot path.
2. Glyph metrics, resource generations, and cache lifetime need complete
   contracts and evidence.
3. Tank, HUD, gauge, blend, clipping, and edge treatment need final parity work.
4. Presentation outcomes and runtime GPU errors need to drive explicit recovery
   and truthful fallback reporting.
5. Development and release builds need an architecture-aware renderer policy and
   a reversible Apple-Silicon default flip.

The current Smooth renderer remains the source of truth for Glorp's established
art. This migration changes the renderer, not the generated pet, room, activity,
or companion product model.

## Decision

Complete the current contract-first retained pipeline, make wgpu/Metal the
`auto` renderer on Apple Silicon, keep Smooth as both an explicit renderer and
the automatic runtime fallback, and keep Smooth as the `auto` renderer on Intel
Macs.

The cutover is deliberately reversible. No Smooth implementation code is
removed, deprecated, or rewritten as part of this design. Retiring Smooth
requires a separate approved design.

The migration does not build the full future retained scene graph or resource
compiler. It introduces only the boundaries required for truthful evidence,
resource lifetime, presentation state, recovery, and default selection. Those
boundaries must allow the later persistent scene graph to replace the current
Smooth-plan translator without another host rewrite.

## Goals

1. Preserve the recognizable generated pet art and established companion
   composition through the retained backend.
2. Capture the actual Metal output and compare the same live prepared frame
   through Smooth and Retained.
3. Eliminate native glyph rasterization and static atlas upload from ordinary
   frames.
4. Distinguish requested renderer, effective renderer, presentation success,
   skips, recovery, and fallback in review evidence.
5. Recover from retained initialization and unrecoverable runtime failures by
   repainting through Smooth without losing live companion state.
6. Make Retained the `auto` renderer on Apple Silicon while preserving explicit
   Smooth and Retained controls.
7. Keep Intel Macs on Smooth until a separately approved native Intel
   qualification changes that policy.
8. Keep existing TUI, Classic, Pixel, Smooth, packaging, and release behavior
   outside the selected macOS companion path intact.

## Non-goals

- New 2.5D effects, lighting, particles, materials, meshes, or camera behavior.
- A persistent retained scene graph, stable scene-node hierarchy, or general
  renderer resource compiler.
- Removing or retiring the Smooth AppKit renderer.
- A remote feature flag or user-facing renderer preference.
- A second software renderer, retry state machine, or permanent last-good-frame
  subsystem.
- Pixel equality between AppKit and Metal.
- Automatic performance, energy, memory, startup, package-size, or build-time
  qualification.
- Changing Intel, non-macOS, TUI, or terminal renderer defaults.

Performance and energy measurement are explicitly deferred and non-blocking.
They may be run later through a separate ad-hoc request, but this design neither
schedules them nor makes the default flip wait for them.

## Product invariants

The retained companion preserves the existing visual hierarchy:

1. Glorp is the hero and remains recognizable as the same generated pet.
2. Pet silhouette, palette, foreground and background cell ink, eyes, mouth,
   expression, species traits, and life stage remain intact.
3. Props, tank life, activity marks, and room details remain legible evidence of
   habitation and history.
4. Bed, wall shadow, projection, tank falloff, aura, clipping, blend order,
   parallax, and depth scaling preserve the established composition.
5. HUD and perimeter gauges remain centered, glanceable, and secondary.
6. Smooth remains selectable and can replace Retained in the same process after
   a retained failure.
7. A renderer or capture failure never silently drops required pet, habitat,
   HUD, gauge, or privacy content.

## Architecture

### Data path

The migration keeps the existing semantic and scene derivation:

```text
Live WatchViewModel
  -> existing round and Smooth semantic preparation
  -> SmoothCompanionScenePlan plus prepared chrome
  -> retained resource and frame preparation
  -> wgpu/Metal presentation
```

The retained backend adapts the current prepared scene. It does not import the
renderer-spike DTOs or create a second product-state derivation.

### Required boundaries

The current prototype gains four narrow contracts.

#### `RendererPolicy`

Renderer selection supports three requested modes:

- `auto`
- `smooth`
- `retained`

`auto` resolves by architecture:

- Apple Silicon macOS: Retained
- Intel macOS: Smooth
- Existing non-macOS behavior: unchanged

Review evidence records both `requested_renderer` and `effective_renderer`.
Explicit Retained selection does not suppress safety fallback; evidence makes any
fallback visible instead of pretending that Retained presented the frame.

#### `RetainedResources`

The retained host owns persistent resource generations, including:

- glyph atlas pixels and entries;
- atlas texture, view, sampler, and bind group;
- the deterministic generation key;
- compilation and upload counters;
- the last successfully activated resource generation.

The contract is intentionally narrower than a general resource compiler. It
exists to make the current retained renderer correct and observable.

#### `PresentationOutcome`

Every retained presentation attempt produces a typed outcome that distinguishes:

- frame prepared;
- submitted and presented;
- skipped because of timeout, occlusion, minimization, or zero size;
- surface reconfigured;
- recovered;
- fallen back to Smooth;
- failed before presentation.

Review capture and diagnostics consume this outcome. A prepared frame is not
reported as presented merely because no synchronous Rust error was returned.

#### `ReviewCaptureTarget`

Retained capture reads the actual Metal output through an offscreen-compatible
texture or copy/readback path. AppKit view caching is not an acceptable retained
capture implementation because it does not include `CAMetalLayer` contents.

The capture boundary freezes one prepared live frame and sends that identical
semantic/layout/chrome state through both Smooth and Retained. Wall-clock motion,
polling, and semantic art advancement do not change between the paired captures.

### Future scene-graph compatibility

The later retained scene graph must be able to replace retained frame preparation
without replacing renderer policy, the macOS host, capture/readback,
presentation outcomes, or fallback handling.

This design does not invent stable scene nodes, materials, meshes, or future
effect abstractions. The persistent scene graph is the next renderer project and
should be designed alongside the first renderer-native visual feature that needs
it.

## Visual parity

### Live visual oracle

Parity review uses the actual current companion state:

- the user's generated pet;
- the current room and biome;
- earned props and tank life;
- current activity state;
- the HUD values currently displayed by the product;
- the current gauge fractions and dim state.

The review tool does not replace that content with a synthetic fixture. It freezes
the live prepared frame so both backends receive the same input.

Local capture artifacts may contain the real HUD totals exactly as shown. They
must be written below a gitignored local output directory and must never be
committed, uploaded, or attached automatically.

### Review sizes and states

Routine pixel iteration uses one automatically exiting Smooth/Retained pair at
360 logical points.

The final one-shot review covers:

- 260 logical points;
- 360 logical points;
- 480 logical points;
- 720 logical points;
- normal composition;
- dimmed composition.

The matrix is run once after focused parity work, not after every change.

### Acceptance standard

Pixel equality is not required across AppKit and Metal. Visual acceptance
requires:

- the same immediately recognizable pet identity;
- equivalent pet and habitat anchors, bounds, and relative scale;
- preserved glyph structure, baseline, bearing, and authored cell spacing;
- equivalent layer order and clipping;
- equivalent HUD run bounds, centering, and vertical stacking;
- equivalent gauge start, end, width, cap, and overage geometry;
- equivalent tank core, midpoint, rim, and dither intent;
- no missing, stretched, fragmented, blurred-away, or arbitrarily aligned art;
- no required content silently omitted to keep Retained active;
- direct side-by-side human approval by Drew.

Focused geometry and resource assertions support human review; they do not
replace it.

## Glyph and resource contract

### Atlas entry

Each glyph/style entry records enough data to reproduce Smooth placement:

- normalized visible-ink UV rectangle;
- ink width and height;
- horizontal and vertical bearing from the Smooth draw origin;
- baseline, ascent, descent, and line height;
- advance;
- source raster size and style identity;
- safe padding excluded from visible UVs;
- the font/raster policy version used by the generation key.

Whitespace has an advance and no visible quad. Missing or unsupported required
glyphs fail resource preflight rather than disappearing.

### Resource generation

The generation key covers the bounded repertoire and raster policy required by
the live prepared companion, including pet art, room marks, props, tank life,
activity marks, and the permitted HUD character set.

An unchanged generation reuses atlas metadata and GPU resources. Ordinary motion
and presentation frames may update instance or uniform data but perform no
native font lookup, native rasterization, atlas texture creation, or atlas upload.

A changed valid generation compiles completely before activation. If the active
frame cannot be represented safely, Retained falls back to Smooth with a bounded
diagnostic. Compilation and upload counters make cache behavior testable.

### Sampling and placement

Background ink continues to fill the authored cell. Glyph ink uses its measured
quad and is not stretched to fill that cell. Pet depth and motion transform the
completed placement; they do not rerasterize glyphs per frame.

Filtering is selected from bounded visual evidence. Sampling cannot be used to
blur away metric or placement errors.

## Shared parity geometry

Backend-neutral preparation is introduced only where independent implementations
have already drifted or where exact geometry is part of acceptance.

### Tank falloff

Smooth and Retained consume shared interpolation parameters and a deterministic
dither definition, or tests prove the shader implementation is equivalent at the
core, midpoint, and rim within one output level.

### HUD

Prepared HUD runs expose measured advances, ink bounds, line heights, total
bounds, and final line origins. Both backends consume the same placement result
where practical. Native text measurement and lookup stay outside ordinary
retained frames.

### Gauges

Smooth and Retained consume the same lane, start/end angle, stroke width, cap,
fill, and overage-marker geometry. Retained rendering may be analytic or
tessellated, but zero, partial, full, and overage states must match the shared
contract.

### Shapes, blends, and clipping

Required shape kinds, gradients, blend modes, and clip modes are either
implemented faithfully or cause explicit fallback. Antialiasing and edge
treatment must preserve the Smooth visual intent at the final review sizes.

## Runtime recovery

Recovery is intentionally bounded.

### Non-fatal presentation conditions

- Outdated or suboptimal surfaces may be reconfigured.
- Timeout, occlusion, minimization, and zero-size conditions skip presentation.
- Skipped frames are not counted as presented frames.

### Fallback conditions

The retained host transitions to Smooth after:

- retained initialization failure;
- unrecoverable surface loss;
- device loss;
- validation, internal, or out-of-memory device errors;
- required resource compilation failure;
- unsupported required content.

The transition:

1. records a sanitized failure category;
2. detaches the Metal layer;
3. preserves the current live view model and prepared state;
4. selects Smooth as the effective renderer;
5. requests an AppKit repaint;
6. reports the fallback in review and development evidence.

There is no general retry state machine. A future recovery redesign requires its
own evidence and scope.

## Build and release policy

Normal Apple-Silicon companion development and release bundles compile the
retained backend so `auto` can select it. Smooth remains compiled on macOS.

Intel builds keep Smooth as the `auto` renderer. Intel retained execution is not
required by this cutover and does not block it. Existing non-macOS and
`--no-default-features` contracts remain green unless an architecture-specific
release change is explicitly required and reviewed.

Development commands must make the selected build behavior explicit and must not
silently replace a retained-capable app bundle with a binary that rejects the
Retained renderer.

Rollback paths are:

- immediate local selection with `--renderer smooth`;
- an architecture policy change returning Apple-Silicon `auto` to Smooth;
- automatic in-process fallback after retained failure.

## Evidence contract

Each paired review records:

- requested renderer;
- effective renderer;
- logical width and height;
- physical width and height;
- backing scale;
- live prepared-state checksum;
- capture path;
- presentation outcome;
- resource generation identifier;
- atlas build and upload counters;
- fallback count and sanitized reason;
- panic and frame-preparation error state.

A retained artifact is accepted only when it is nonblank and records a presented
Metal frame before readback. A Smooth fallback image cannot satisfy a retained
capture merely because the requested renderer was Retained.

## Verification policy

### Routine checks

Routine work uses the cheapest sufficient evidence:

1. pure tests for changed metrics, geometry, policy, or resource behavior;
2. narrow retained-feature compile or test;
3. formatting, clippy, and diff checks;
4. one automatically exiting live 360 capture pair only when pixels change.

Routine checks do not run repeated native loops, unattended windows, renderer
qualification matrices, or broad performance suites.

### Focused tests

Required focused coverage includes:

- ordinary, narrow, descender, bold, whitespace, and multi-scalar glyph entries;
- slot-edge UV normalization and padding exclusion;
- unit, fractional-motion, and depth-scaled glyph placement;
- background-only cells;
- unchanged and changed atlas generations;
- actual atlas build/upload counters across identical frames;
- HUD line measurement, centering, and stacking;
- gauge zero, partial, full, and overage geometry;
- tank core, midpoint, rim, and deterministic dither samples;
- required blend and clip modes;
- architecture-specific `auto` policy and explicit overrides;
- requested versus effective renderer reporting;
- blank retained capture rejection.

### Native recovery tests

Bounded fault injection covers:

- initialization failure;
- surface loss;
- device validation or loss;
- atlas/resource failure;
- unsupported required content.

Each test asserts the presentation outcome, effective renderer, fallback reason,
successful Smooth repaint, and clean automatic exit.

A separate retained-readback failure test asserts a failed capture with truthful
metadata and a clean automatic exit. Readback failure alone does not change the
live renderer or force a Smooth fallback; a failed presentation follows the
runtime fallback policy above.

### Final one-shot gate

Before the default flip, run once:

- focused retained unit and integration tests;
- formatting and clippy;
- relevant existing companion, round-scene, packaging, and feature-boundary
  checks;
- the four-size normal/dimmed Smooth/Retained capture set;
- bounded resize, backing-scale, minimize/restore, occlusion, input/window, and
  fallback smoke checks;
- direct visual review and approval.

The final gate explicitly does not run CPU, energy, memory, startup,
package-size, or build-time qualification.

## Cutover acceptance

Retained becomes the Apple-Silicon `auto` renderer only when:

1. Drew approves the final side-by-side visual set.
2. Metal capture is nonblank, reads actual Metal output, and reports truthful
   presentation metadata.
3. The live pet, habitat, HUD, gauges, dimming, and layer composition preserve the
   Smooth identity and hierarchy.
4. Ordinary frames perform no native glyph rasterization or static atlas upload.
5. Required primitives never disappear silently.
6. Initialization and runtime retained failures repaint successfully through
   Smooth and report the effective renderer and reason.
7. `auto`, explicit Smooth, and explicit Retained behave according to policy.
8. Intel `auto` remains Smooth.
9. Formatting, clippy, focused retained tests, and relevant existing regressions
   pass.
10. No Smooth code is removed or made unavailable.

## Delivery sequence

### Gate 1: truthful evidence

- Implement retained Metal readback.
- Freeze one live prepared frame across both backends.
- Record complete dimensions, renderer, resource, presentation, and fallback
  metadata.
- Reject blank and fallback-mislabeled retained captures.

### Gate 2: pet and resource parity

- Complete glyph metric and placement contracts.
- Complete atlas generation and lifetime behavior.
- Add cache counters and focused tests.
- Obtain a representative 360 parity pair with recognizable pet art.

### Gate 3: composition parity

- Close tank falloff, HUD, gauge, blend, clip, gradient, and edge differences.
- Cover normal and dimmed composition.
- Preserve required content or fall back explicitly.

### Gate 4: operational cutover

- Connect runtime device errors to presentation outcomes.
- Complete bounded fallback tests.
- Add architecture-aware `auto` policy and build integration.
- Keep explicit Smooth and Retained controls.

### Gate 5: reversible default flip

- Run the one-shot visual and native gate.
- Obtain Drew's visual approval.
- Make Apple-Silicon `auto` resolve to Retained.
- Re-run the bounded regression gate with the new default.

Each gate leaves Smooth usable. The default does not change before Gate 5.

## Deferred follow-on

The next renderer project is the persistent retained scene graph and resource
compiler. It should use the stable host, capture, presentation, resource-
generation, and fallback seams delivered here. Its design should be grounded in
the first renderer-native 2.5D feature and may use a separately requested
post-cutover profile as evidence.

That project is not a prerequisite for this cutover.
