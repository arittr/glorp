# Glorp Retained Companion Default Cutover - design

- Date: 2026-07-11
- Status: approved design direction; amended after adversarial staff review
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

The cutover is deliberately source-reversible and automatically falls back for
technical renderer failures. A bad-but-successfully-presenting release requires
a follow-up Smooth-default release; there is no remote kill switch or same-version
npm rollback. No Smooth implementation code is removed, deprecated, or rewritten
as part of this design. Retiring Smooth requires a separate approved design.

The migration does not build the full future retained scene graph or resource
compiler. It introduces only the boundaries required for truthful evidence,
resource lifetime, presentation state, recovery, and default selection. Those
boundaries must allow the later persistent scene graph to replace the current
Smooth-plan translator without another host rewrite.

## Supersession of the July 10 renderer design

This document is the controlling cutover decision where it conflicts with
`2026-07-10-glorp-retained-rust-renderer-design.md`.

It supersedes that document on these points:

- direct `wgpu`/Metal is selected rather than remaining a candidate;
- the persistent retained scene graph is not a prerequisite for the default flip;
- CPU, energy, memory, startup, package-size, and build-time measurements are
  deferred and non-blocking;
- Smooth is preserved after the default flip rather than retired by this phase;
- Intel remains Smooth by default and is not part of native retained
  qualification for this cutover.

The earlier document remains binding for privacy, native host ownership,
accessibility and input preservation, Unicode correctness, resource boundedness,
sanitized diagnostics, and recovery unless this document states a more specific
contract. Its numerical performance, energy, memory, build, and package budgets
are not inherited as cutover gates.

## Goals

1. Preserve the recognizable generated pet art and established companion
   composition through the retained backend.
2. Capture the actual Metal output and compare the same live prepared frame
   through Smooth and Retained.
3. Eliminate native glyph rasterization and static atlas upload from ordinary
   frames.
4. Distinguish requested renderer, effective renderer, observable frame
   milestones, terminal disposition, recovery, and fallback in review evidence.
5. Recover from retained initialization and unrecoverable runtime failures by
   repainting through Smooth without losing live companion state.
6. Make Retained the `auto` renderer on Apple Silicon while preserving explicit
   Classic, Pixel, Smooth, and Retained controls according to compiled
   capabilities.
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

The current prototype gains the following narrow contracts and activation rule.

#### Renderer request, policy, and runtime state

Requested and effective renderer identity are separate types:

```text
RendererRequest  = Auto | Classic | Pixel | Smooth | Retained
EffectiveRenderer = Classic | Pixel | Smooth | Retained
```

`RendererPolicy::resolve(request, target_arch, retained_compiled)` is the only
place that resolves `Auto` or rejects an unavailable explicit renderer:

- Apple Silicon macOS with Retained compiled: `Auto -> Retained`;
- Intel macOS release builds: `Auto -> Smooth`, and explicit Retained is rejected
  because the release binary does not compile that backend;
- Classic, Pixel, and Smooth remain explicit modes with their existing behavior;
- existing non-macOS behavior is unchanged.

One `RendererRuntimeState` owns the immutable request plus the current effective
renderer, transition count, last sanitized fallback reason, and latest frame
disposition. Fallback changes only the effective renderer. Review capture and
diagnostics read this state rather than retaining a second renderer label.

#### `RetainedResources`

The retained host owns persistent resource generations, including:

- glyph atlas pixels and entries;
- atlas texture, view, sampler, and bind group;
- capacity-bounded instance/uniform buffers and their write ranges;
- pipelines and other immutable GPU objects;
- the deterministic generation key;
- compilation, creation, upload, miss, and write counters;
- the last successfully activated resource generation.

The contract is intentionally narrower than a general resource compiler. It
exists to make the current retained renderer correct and observable.

#### Transactional retained activation

Retained activation is two-phase. Host, adapter, device, surface configuration,
pipelines, and required initial resources are constructed and preflighted before
the view permanently adopts the `CAMetalLayer`. An activation guard restores the
original AppKit layer state on every partial failure. Only a successfully
preflighted host is committed to the view.

This is Gate 1 infrastructure. Initialization failure must leave a paintable
Smooth view even when no `RetainedHost` value is returned.

#### Frame progress and terminal disposition

Frame progress is an ordered set of observable API milestones:

- prepared;
- encoded;
- submitted;
- surface `present` called;
- GPU work completed;
- readback completed.

The renderer never claims compositor scanout, which `wgpu` does not expose.
Each attempt also has exactly one terminal disposition:

- `SurfacePresentCalled`;
- `Captured`;
- `Skipped(reason)` for timeout, occlusion, minimization, or zero size;
- `Failed(category)`;
- `FallbackPending(category)`;
- `FallbackPainted(category)`.

Surface reconfiguration and recovery attempts are transition metadata, not
competing terminal outcomes. Review evidence increments only milestones that were
actually observed. Requesting an AppKit repaint is not `FallbackPainted`; the
next successful Smooth draw callback acknowledges that disposition.

Uncaptured `wgpu` errors enter a thread-safe sanitized mailbox. The main thread
drains it at a defined device-poll or UI-tick boundary before recording a
successful terminal disposition and performs any fallback there.

#### Paired review frame and capture coordinator

`PairedReviewFrame` is an immutable snapshot containing:

- the exact Smooth scene plan and draw order;
- grid metrics, aperture, and every prepared chrome value;
- logical and physical viewport dimensions and backing scale;
- semantic tick and the single sampled monotonic elapsed value;
- resource generation identifier;
- a canonical checksum of all preceding fields.

After freezing, neither backend reads a clock, polls usage, advances semantic art,
or rederives scene state. `PairedCaptureCoordinator` sends the same object to an
offscreen Smooth target and a retained capture target.

Retained draws once into a physical-size sRGB intermediate texture with
`RENDER_ATTACHMENT | COPY_SRC`. One ordered submission copies or blits that
result to the surface and copies it to a readback buffer. The readback contract
specifies 256-byte row alignment, row unpadding, BGRA-to-RGBA normalization when
required, vertical orientation, premultiplied-alpha normalization, PNG sRGB
metadata, `map_async` plus device polling, and frame/generation correlation.
AppKit view caching is not an acceptable retained capture implementation because
it does not include `CAMetalLayer` contents.

A capture gate consumes a mandatory terminal manifest and exits nonzero when the
manifest is absent or reports blank output, write failure, map failure, mismatched
frame identity, fallback-mislabeled output, or any incomplete required milestone.
Logging an error and exiting zero is not an accepted capture failure path.

### Future scene-graph compatibility

The later retained scene graph must be able to replace retained frame preparation
without replacing renderer policy, the macOS host, capture/readback,
frame progress/dispositions, or fallback handling.

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

The human parity review freezes that live prepared frame so both backends receive
the same input. Separate privacy-safe sentinel fixtures remain required for
automated readback plumbing, channel/orientation probes, and fault injection;
they do not replace live state as the visual oracle.

Stored artifacts redact live HUD values by default. Exact displayed HUD values
may be stored only with explicit `--review-capture-live-values` opt-in. Sensitive
captures are restricted to the canonical repo-owned
`target/glorp-review-sensitive/` subtree. The writer resolves and verifies the
destination, rejects symlinks and path traversal, and refuses paths outside that
ignored subtree. Sensitive artifacts are never committed, uploaded, attached, or
copied automatically.

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
- the resolved PostScript font name and version;
- backing scale, weight, and raster/antialiasing policy;
- the font/raster policy version used by the generation key;
- an entry kind: `CoverageMask` or `PremultipliedColorRgba`.

Whitespace has an advance and no visible quad. Missing or unsupported required
glyphs fail resource preflight rather than disappearing. Coverage masks receive
the authored foreground color. Color-RGBA glyphs preserve their rasterized RGB
and bypass foreground tinting.

Atlas keys are Unicode scalar sequences, not `char`-sized fragments. Prepared
glyph runs reference atlas IDs for those complete sequences, including composed
marks, replacement glyphs, and color emoji.

### Transitional font-source decision

For the first retained-default release while Smooth remains available, native
AppKit rasterization using the exact resolved Smooth font policy is an explicit
transitional exception. It occurs only during bounded resource compilation, never
in ordinary frames. The exception is revisited before Smooth retirement or as
part of the persistent scene-graph/resource-compiler project. A follow-up font
decision is required before a second retained-default release. Arbitrary fallback
fonts are not permitted.

### Resource generation

`GlyphRepertoireManifest` covers every glyph sequence the active companion can
emit, not merely the current frame. It is derived from generated pet identity and
life stage, every declared semantic animation slot, species and room dialect,
owned props and tank life, activity and particle effects, Glitch corruption,
chest bubbles, replacement behavior, and the complete permitted HUD character
set.

The resource generation key combines that manifest hash with resolved font
identity/version, raster point size, backing scale, weight/style set,
antialiasing policy, atlas packing version, and shader resource version.

An unchanged generation reuses atlas metadata and GPU resources. Ordinary motion
and presentation frames may update instance or uniform data but perform no
native font lookup, native rasterization, atlas texture creation, or atlas upload.

A changed valid generation compiles completely before activation. If the active
frame cannot be represented safely, Retained falls back to Smooth with a bounded
diagnostic. Compilation and upload counters make cache behavior testable.

Deterministic animation strips exercise every declared species/stage/state and
changing HUD digits. After generation activation, those strips must produce zero
atlas builds, atlas uploads, and atlas misses while glyph instances continue to
change.

### Sampling and placement

Background ink continues to fill the authored cell. Glyph ink uses its measured
quad and is not stretched to fill that cell. Pet depth and motion transform the
completed placement; they do not rerasterize glyphs per frame.

Filtering is selected from bounded visual evidence. Sampling cannot be used to
blur away metric or placement errors.

### Persistent GPU allocation boundary

Deferring the full scene graph does not permit per-frame GPU resource creation.
The retained host owns a capacity-bounded instance-buffer ring or equivalent
persistent buffers, updated through bounded writes. Unchanged static primitive
ranges remain stable across ordinary motion.

Counters cover buffer, texture, sampler, bind-group, pipeline, and atlas creation
and upload. After warmup, a deterministic 300-frame varied ambient strip creates
no buffers, textures, samplers, bind groups, pipelines, or static uploads; only
bounded instance and uniform writes may increase.

## Shared parity geometry

Backend-neutral preparation is introduced only where independent implementations
have already drifted or where exact geometry is part of acceptance.

### Color, alpha, and blend convention

Authored colors and canonical PNG output are sRGB. Shader working RGB and
compositing use linear light on an sRGB render target. GPU primitives and color
atlas entries use premultiplied linear RGBA; coverage-mask glyphs are converted to
that convention before blending.

The five Smooth blend modes have one retained definition:

- Normal: source-over;
- Multiply: separable multiply composed source-over;
- Screen: separable screen composed source-over;
- Add: saturating plus-lighter;
- Replace: source copy.

Opaque and translucent swatches for every mode are compared with Smooth using
declared per-channel and alpha tolerances. Unresolved alpha convention or blend
equations block the default flip.

### Tank falloff

Smooth and Retained consume shared interpolation parameters and a deterministic
dither definition, or tests prove the shader implementation is equivalent at the
core, midpoint, and rim within one output level.

Tank interpolation and output-level dithering remain in sRGB output space to
match the established Smooth bitmap. Other retained shape gradients interpolate
in linear light and must match Smooth endpoint and midpoint samples within the
declared visual tolerance.

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
implemented faithfully or cause explicit fallback. Ellipse, arc, aperture, and
rect/ellipse clip edges use physical-pixel analytic coverage based on derivatives
rather than hard discard. Nested clip coverage composes with primitive coverage;
glyph edges use measured atlas coverage. An equivalent technique requires
explicit side-by-side approval rather than silently changing the policy.

Backing-scale changes derive physical dimensions from the view's backing
conversion, activate a compatible raster/resource generation, and revalidate
coverage at scales 1 and 2.

## Runtime recovery

Recovery is intentionally bounded.

### Non-fatal presentation conditions

- Outdated or suboptimal surfaces may be reconfigured.
- Timeout, occlusion, minimization, and zero-size conditions skip presentation.
- Skipped frames never record surface-present-called, GPU-completed, or readback
  milestones.

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
6. records `FallbackPainted` only after the successful Smooth draw callback;
7. reports the fallback in review and development evidence.

There is no general retry state machine. A future recovery redesign requires its
own evidence and scope.

## Build and release policy

The feature and artifact matrix is explicit:

| Surface | Required build behavior | Runtime policy |
| --- | --- | --- |
| Apple-Silicon local development | `cargo build --features retained-renderer` | `Auto -> Retained` after Gate 5 |
| `cargo xtask companion fresh` on Apple Silicon | builder passes `--features retained-renderer` | `Auto -> Retained` after Gate 5 |
| arm64 npm/release binary | `cargo build --release --locked --no-default-features --features retained-renderer --target aarch64-apple-darwin` | `Auto -> Retained` |
| Intel npm/release binary | `cargo build --release --locked --no-default-features --target x86_64-apple-darwin` | `Auto -> Smooth`; explicit Retained unavailable |
| non-macOS release targets | existing `--no-default-features` commands | unchanged |
| macOS retained CI | arm64 `cargo clippy --all-targets --features retained-renderer -- -D warnings` plus focused retained tests | compile and test Retained natively |
| portable CI | existing all-target/no-default checks | unchanged |

Before upload, the staged arm64 `Glorp.app` launches once with `auto` and once
with explicit Retained. Both bounded runs must report effective Retained from the
bundled binary. The Intel staged app reports effective Smooth for `auto` and
rejects explicit Retained clearly.

Development commands must not silently replace a retained-capable app bundle
with a binary that rejects the selected renderer. The app builder records its
compiled renderer capabilities in review metadata and validates them against the
requested launch mode.

Rollback paths are:

- immediate local selection with `--renderer smooth`;
- automatic in-process fallback after retained failure.

For a visual, privacy, input, or driver-specific regression that still presents
successfully, operator rollback is a follow-up release:

1. change only the Apple-Silicon `Auto` policy back to Smooth;
2. build and run the staged Smooth-default arm64 bundle;
3. verify explicit Retained remains available for diagnosis;
4. publish a new version through the normal npm workflow.

Gate 5 includes a no-publish rehearsal of that policy-only rollback artifact.
Immutable npm versions are not overwritten, so this is not an immediate or
same-version kill switch. The release notes and operational handoff must say so
plainly. The rehearsal records elapsed time from the policy edit to a verified
staged artifact; that measured duration is the local rollback-build expectation,
while registry publication latency remains external and unguaranteed.

## Evidence contract

Each paired review records:

- requested renderer;
- effective renderer;
- compiled renderer capabilities;
- logical width and height;
- physical width and height;
- backing scale;
- paired-review-frame checksum and frame/generation identifiers;
- capture path;
- every observed frame milestone and the single terminal disposition;
- resource generation identifier;
- atlas build, upload, and miss counters;
- buffer, texture, sampler, bind-group, pipeline, static-upload, instance-write,
  and uniform-write counters;
- fallback count and sanitized reason;
- sensitive-capture opt-in state without recording live values in the manifest;
- panic and frame-preparation error state.

A retained artifact is accepted only when it is nonblank and records matching
encoded, submitted, surface-present-called, GPU-completed, and readback-completed
milestones for the same frame and resource generation. This proves ordered GPU
work and readback, not compositor scanout. A Smooth fallback image cannot satisfy
a retained capture merely because the requested renderer was Retained.

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

- the complete qualified glyph manifest at all four review sizes and backing
  scales 1 and 2;
- ordinary, narrow, descender, bold, whitespace, replacement, composed-mark,
  multi-scalar, and color-emoji glyph entries, including `�`, `ö`, and `🫧`;
- slot-edge UV normalization and padding exclusion;
- unit, fractional-motion, and depth-scaled glyph placement;
- background-only cells;
- unchanged and changed atlas generations;
- full deterministic species/stage/state strips with zero post-activation atlas
  builds, uploads, or misses;
- 300 varied ambient frames with zero post-warmup GPU resource creation or static
  upload;
- HUD line measurement, centering, and stacking;
- gauge zero, partial, full, and overage geometry;
- tank core, midpoint, rim, and deterministic dither samples;
- opaque/translucent swatches for every blend mode plus gradient samples;
- physical-pixel aperture, nested clip, ellipse, arc, glyph-edge, and gauge-cap
  coverage at backing scales 1 and 2;
- architecture-specific `auto` policy and explicit overrides;
- requested versus effective renderer reporting;
- asymmetric colored-corner probes for vertical orientation and BGRA/RGBA
  normalization;
- odd-width row padding, consecutive sentinel frames, and stale-readback
  rejection;
- blank/map/write/readback failure causing the gate command itself to fail;
- default-redacted capture, explicit sensitive opt-in, and rejection of symlink,
  traversal, and out-of-root sensitive paths.

### Native recovery tests

Bounded fault injection covers:

- initialization failure;
- surface loss;
- device validation or loss;
- atlas/resource failure;
- unsupported required content.

Each test asserts the terminal disposition, effective renderer, fallback reason,
zero false retained surface-present-called/readback counts, successful
acknowledged Smooth paint, and clean automatic exit. Asynchronous validation,
internal, out-of-memory, and device-loss injections cross the GPU-error mailbox and are
drained before success is recorded.

A separate retained-readback failure test asserts a failed capture with truthful
metadata and a bounded nonzero exit. Readback failure alone does not change the
live renderer or force a Smooth fallback; a failed presentation follows the
runtime fallback policy above.

### Final one-shot gate

Before the default flip, run once:

- focused retained unit and integration tests;
- formatting and clippy;
- relevant existing companion, round-scene, packaging, and feature-boundary
  checks;
- the exact architecture/build matrix and staged arm64/Intel app smokes;
- the four-size normal/dimmed Smooth/Retained capture set;
- bounded resize, backing-scale, minimize/restore, occlusion, input/window, and
  fallback smoke checks;
- accessibility and focus checks inherited from the July 10 host contract;
- a no-publish Smooth-default rollback-artifact rehearsal;
- direct visual review and approval.

The final gate explicitly does not run CPU, energy, memory, startup,
package-size, or build-time qualification.

## Cutover acceptance

Retained becomes the Apple-Silicon `auto` renderer only when:

1. Drew approves the final side-by-side visual set.
2. Transactional activation proves every partial initialization failure leaves a
   paintable Smooth view.
3. Metal capture is nonblank, reads the exact frozen frame through ordered GPU
   completion/readback, and reports truthful metadata without claiming scanout.
4. The live pet, habitat, HUD, gauges, dimming, and layer composition preserve the
   Smooth identity and hierarchy.
5. The complete dynamic glyph repertoire, including color and multi-scalar
   entries, is preflighted for the active companion.
6. Ordinary frames perform no native glyph rasterization, GPU resource creation,
   or static upload after warmup.
7. Required primitives never disappear silently, and color/blend/AA contracts
   pass their physical-pixel comparisons.
8. Initialization and runtime retained failures repaint successfully through
   Smooth and report the effective renderer and reason.
9. `auto`, explicit Classic, Pixel, Smooth, and Retained behave according to the
   compiled capability and architecture policy.
10. The staged arm64 release artifact contains Retained and reports it effective;
    Intel `auto` remains Smooth and explicit Retained is unavailable.
11. The sensitive-capture boundary and machine-verifiable capture failure path
    pass.
12. The Smooth-default follow-up-release rollback artifact is rehearsed.
13. Formatting, clippy, focused retained tests, and relevant existing regressions
   pass.
14. No Smooth code is removed or made unavailable.

## Delivery sequence

### Gate 1: truthful evidence

- Split renderer request from effective runtime state.
- Make retained construction and layer activation transactional.
- Define terminal frame dispositions and main-thread GPU-error delivery.
- Define and freeze the canonical `PairedReviewFrame`.
- Implement retained Metal readback.
- Implement the paired coordinator, canonical PNG normalization, and
  machine-verifiable terminal manifest.
- Record complete dimensions, capability, renderer, resource, milestone, privacy,
  and fallback metadata.
- Reject blank and fallback-mislabeled retained captures.

### Gate 2: pet and resource parity

- Complete glyph metric and placement contracts.
- Approve the transitional font source and color/multi-scalar glyph behavior.
- Build the full dynamic repertoire manifest and resource-generation key.
- Complete atlas lifetime plus persistent buffer/resource behavior.
- Add all resource creation/upload/miss counters and deterministic strips.
- Obtain a representative 360 parity pair with recognizable pet art.

### Gate 3: composition parity

- Close tank falloff, HUD, gauge, color, alpha, blend, clip, gradient, and
  physical-pixel antialiasing differences.
- Cover normal and dimmed composition.
- Preserve required content or fall back explicitly.

### Gate 4: operational cutover

- Connect runtime device errors to the main-thread mailbox and terminal
  dispositions.
- Complete bounded fallback tests.
- Implement the exact architecture-aware build, CI, packaging, and staged-app
  matrix.
- Keep explicit Classic, Pixel, Smooth, and Retained controls according to the
  compiled-capability policy.
- Rehearse the policy-only Smooth-default follow-up-release artifact.

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
