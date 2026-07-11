# Glorp Retained Rust Renderer And 2.5D Scene Runtime - design

- Date: 2026-07-10
- Status: proposed direction; written for architecture review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-06-15-glorp-presentation-architecture-design.md`
  - `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-companion-renderer-v2-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-smooth-motion-stabilization-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-companion-draw-boundary-hardening-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-smooth-pet-follow-parallax-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-smooth-tank-depth-design.md`

## Calibration

The Smooth renderer proved the product direction: Glorp can keep its generated
cell-art identity while gaining fractional motion, parallax, typed shapes, depth,
scale, gradients, projections, and a richer tank composition.

It also exposed the current architecture's ceiling.

The companion is still an immediate-mode CPU renderer. At every visible animation
step it rebuilds most of the layered scene, creates native drawing objects, draws
each glyph and shape through AppKit/CoreText, invalidates the full round view, and
asks Core Animation to upload and composite a new backing store. One focused local
profiling pass reported a median reduction from approximately 22.7% to 16.5% after
caching and frame-boundary work, but that result is directional evidence, not the
Phase 0 baseline. Phase 0 must check in the runner, raw samples, binary identity,
poll timestamps, and environment metadata before any number is used as an
acceptance comparison. The remaining sampled cost appears structural rather than
another missing cache.

The end goal is therefore not to rewrite AppKit calls in Rust one-for-one. It is to
make Rust own a retained scene runtime and a batched renderer. AppKit remains the
thin macOS application and window host. The leading backend candidate is a Rust GPU
renderer using Metal through `wgpu`, while deterministic Rust scene contracts and
review artifacts remain the source of truth. Phase 0 must compare candidates and
prove the surface bridge, package cost, capture path, and operational behavior
before selecting a production backend.

This is also the rendering foundation for future 2.5D and selective 3D work. New
depth planes, camera motion, lights, materials, particles, mesh-backed props, and
interaction should become additions to a stable scene runtime rather than more
special cases in `src/companion/app.rs`.

## Problem

The current Smooth flow is approximately:

```text
WatchViewModel + time
  -> layered Classic/TUI scene construction
  -> SmoothCompanionScenePlan
       content, transforms, depth, parallax, chrome reservations
  -> PreparedCompanionFrame
  -> AppKit drawRect
       per-layer sorting/clip/blend
       per-cell NSString/CoreText drawing
       per-shape NSBezierPath/NSGradient drawing
       full NSView backing-store update
       Core Animation commit
```

Several kinds of state are mixed together:

1. **Semantic state** changes on polls or discrete product events: pet identity,
   art, mood, room profile, earned props, tank-life cast, helper health, HUD text,
   and lifecycle state.
2. **Layout state** changes on viewport size, backing scale, font metrics, aperture,
   and chrome reservations.
3. **Resource state** changes when glyphs, textures, meshes, materials, or shaders
   must be created or replaced.
4. **Dynamic content state** changes at a semantic-art or authored-effect cadence:
   blink/expression glyph substitutions, Glitch corruption, species particles,
   ambient/mote/activity glyph populations, chest bubbles, and other topology or
   instance-data changes that are not just transforms.
5. **Animation state** changes every paint sample: pet transform, depth, bob,
   parallax, aura pulse, shadow projection, continuous particles, and gauge
   interpolation.

Today a change in categories 4 or 5 causes work from categories 1 through 3 to be
repeated.
The native adapter also owns too much rendering behavior, which makes each new
2.5D effect increase callback complexity and CPU cost.

The current `SmoothCompanionScenePlan` is a useful semantic seam, but it is still a
fully resolved per-frame plan. Its layer content and transforms live in the same
objects. It does not identify immutable GPU resources, distinguish template
rebuilds from uniform updates, or provide stable node handles for retained updates.

## Goals

1. Reduce the visible companion's steady-state process CPU to low single digits on
   supported Apple Silicon hardware without reducing Glorp's visual identity or
   disabling ambient motion.
2. Keep AppKit responsible for macOS lifecycle, menus, windowing, visibility,
   fullscreen, and input routing only. Rust owns scene derivation, resources,
   rendering, capture, and frame pacing policy.
3. Split retained semantic/layout content from small per-frame animation updates.
4. Render glyphs, sprites, shapes, gradients, and simple meshes in batches rather
   than one native draw call per item.
5. Establish a deliberate 2.5D scene model with stable transforms, camera, depth,
   materials, clipping, blend modes, and hit-test metadata.
6. Leave a narrow path to true 3D additions without turning Glorp into a general
   game engine.
7. Preserve Classic visual identity, existing privacy projections, deterministic
   Preview Lab evidence, bounded native review capture, and last-good-frame safety.
8. Preserve the current watch TUI and menubar adapters. This renderer is initially
   for the native round companion, not a forced universal surface backend.
9. Make renderer work measurable: template rebuilds, resource uploads, draw calls,
   frame CPU time, GPU time where available, missed frames, atlas misses, and
   device recovery are observable.
10. Land through reversible slices with the current Smooth AppKit path available
    until retained-renderer parity and performance gates pass.

## Non-Goals

- No replacement of AppKit windowing or the macOS application lifecycle.
- No rewrite of `glorp watch` into a GPU surface.
- No general-purpose ECS, physics engine, skeletal-animation system, scene editor,
  or arbitrary game-engine plugin architecture.
- No physically based renderer, ray tracing, volumetric simulation, or general
  post-processing graph in the first program.
- No public authored-asset format or mandatory glTF pipeline in the first program.
- No requirement that GPU pixel output be bit-identical across vendors or driver
  versions.
- No permanent second product state model beside `WatchViewModel` and
  `PresentationScene`.
- No inference of semantic roles from glyphs, colors, texture contents, or draw
  order after scene construction.
- No fallback that silently drops props, tank life, HUD, gauges, privacy rules, or
  Glorp identity to keep the new backend running.
- No removal of the Smooth AppKit renderer until the new backend has passed native
  visual, performance, recovery, and review gates.

## Locked Direction

1. **Retained Rust renderer, native host.** AppKit hosts the window and a drawable
   surface. It does not remain the cell/shape painter.
2. **`wgpu` is the leading shipping candidate, not a pre-approved dependency.** On
   macOS it uses Metal. Raw `wgpu` currently best matches instanced glyphs/sprites,
   depth-tested planes, camera transforms, lit 2.5D elements, and future mesh
   primitives. Phase 0 must compare it against bounded cheaper prototypes under the
   same fixtures and measurement protocol. Production dependency lock-in occurs
   only after the surface, capture, performance, energy, package, build, and release
   gates pass. Failure reopens the backend choice rather than forcing integration.
3. **No big-bang rewrite.** The existing Smooth contracts are evolved into
   retained template/frame contracts and both renderers run side by side during
   migration.
4. **Static, dynamic-content, and frame state are different types.** Per-frame
   motion cannot require cloning or rebuilding static layer items; glyph or
   instance substitutions cannot require rebuilding unrelated scene structure.
5. **Stable IDs, not vector positions, are the update contract.** Nodes and
   resources receive deterministic identifiers valid for one template generation.
6. **No native text work in the hot frame path.** Glyphs are rasterized into a
   bounded atlas outside frame encoding and drawn as instanced quads.
7. **One scene, multiple evidence paths.** The shipping GPU backend, headless
   semantic artifacts, and any reference raster/readback path consume the same
   retained scene and frame contracts.
8. **The round aperture is renderer-owned.** The GPU backend applies the porthole
   mask, clipping, dim overlay, and scene compositing. AppKit must not clip or
   repaint scene content around the GPU surface.
9. **Event-driven invalidation.** Semantic/layout/resource changes rebuild or
   upload only the affected retained data. Animation ticks update transforms and
   uniforms only.
10. **The existing Smooth AppKit path is a migration fallback, not the end-state
    software fallback.** Device initialization failure or unrecoverable device loss
    may temporarily return to Smooth while the new backend is hidden or staged.

## Product Model

The companion remains a round porthole into Glorp's habitat.

The retained renderer must preserve the current hierarchy:

- Glorp is the hero and remains recognizable as the generated pet.
- Props and tank life prove history and habitation.
- The tank, bed, wall shadow, projection, aura, and parallax establish depth.
- HUD and perimeter gauges remain secondary glanceable chrome.
- Motion communicates life and work reaction, not decorative busyness.

The new runtime should make the following additions straightforward without
rewriting the painter:

- more depth planes and authored Z placement;
- camera push/pull or subtle orbit within strict motion budgets;
- sprite extrusion or layered-card thickness;
- localized lights and rim lighting;
- material changes for glass, water, glow, shadow, and biome surfaces;
- GPU particles for bubbles, motes, feed bursts, and species effects;
- mesh-backed habitat props where sprites are insufficient;
- hit testing and pointer reactions through stable semantic targets;
- selective post effects such as a bounded water warp or bloom-like glow.

These are capabilities, not automatic visual requirements. Every feature still
needs a separate product design and Preview Lab review.

## Architecture

### Overview

```text
WatchViewModel + now + surface privacy
                |
                v
PresentationScene / RoundSceneModel
                |
                v
RetainedSceneTemplateBuilder -------------------------+
  semantic content, layout, stable node/resource IDs  |
                |                                      |
                v                                      |
RetainedSceneTemplate                                  |
  immutable until semantic/layout generation changes  |
                |                                      |
                +-------------------+                  |
                                    |                  |
Semantic-art clock + live effects   |                  |
                |                   |                  |
                v                   |                  |
DynamicSceneState / ContentDelta    |                  |
  bounded glyph/sprite slots and    |                  |
  instance-group substitutions      |                  |
                |                   |                  |
Monotonic frame clock               |                  |
                |                   |                  |
                v                   v                  |
RetainedFrameState / FrameDelta     ResourceCompiler --+
  camera, transforms, opacity,      glyph/texture/mesh/material atlases
  uniforms, visibility, gauges             |
                |                          |
                +------------+-------------+
                             v
                    RetainedRenderer
                      selected retained backend
                      wgpu/Metal leading candidate
                      offscreen/readback review path
                             |
                             v
                  AppKit drawable/window host
```

### Layering with existing presentation architecture

The existing `PresentationScene` remains the privacy-aware product snapshot, but
the native round companion does not yet consume it directly. `RoundSceneModel`
and Smooth scene construction remain the current round-surface projections during
migration. The retained template initially adapts the proven Smooth plan; a later
bounded convergence step may derive both Smooth and retained plans from one round
projection. The migration must not create a second independently-derived product
scene that can disagree with `PresentationScene` privacy or current round output.

The retained renderer adds a lower presentation/runtime tier rather than another
product model:

```text
WatchViewModel
  -> PresentationScene               product semantics and privacy
  -> round/smooth projection          companion composition and placement
  -> RetainedSceneTemplate            renderer-ready stable content/resources
  -> DynamicSceneState                bounded content/instance substitutions
  -> RetainedFrameState               per-paint transforms and uniforms
  -> backend commands                 GPU or review output
```

`SceneDrawList` remains a compatibility artifact for terminal/Classic parity. It
is not the retained renderer primitive.

### Proposed modules

Exact names may change during implementation planning, but ownership should be
approximately:

```text
src/presentation/retained/
  mod.rs
  ids.rs
  template.rs
  frame.rs
  node.rs
  primitive.rs
  material.rs
  camera.rs
  resource.rs
  validate.rs
  checksum.rs

src/renderer/
  mod.rs
  metrics.rs
  error.rs
  software_reference.rs   # optional narrow visual oracle, not assumed shipping
  wgpu/
    mod.rs
    device.rs
    surface.rs
    atlas.rs
    pipelines.rs
    buffers.rs
    capture.rs
    recovery.rs

src/companion/
  app.rs                  # AppKit lifecycle and host only
  retained_host.rs        # surface attachment, resize, visibility, input bridge
```

Platform-neutral retained contracts must not contain AppKit, Metal, `wgpu`, or
window-handle types. Backend resources live under `src/renderer`.

## Core Data Contracts

### Generations and stable identifiers

```rust
pub struct SceneGeneration(pub u64);
pub struct NodeId(pub u32);
pub struct ResourceId(pub u32);
pub struct MaterialId(pub u32);
pub struct AtlasEntryId(pub u32);
```

IDs are stable within one `SceneGeneration`. A new template generation may reuse
numeric storage but must not accept stale deltas. Applying a `FrameDelta` with the
wrong generation is a recoverable renderer error and keeps the last good frame.

IDs are assigned deterministically from semantic role plus stable local identity,
not allocation order alone. Preview artifacts may expose sanitized string aliases
such as `pet.body`, `tank.bed`, or `prop.token_treasure_chest_2m.0`; runtime numeric
IDs remain compact.

### `RetainedSceneTemplate`

```rust
pub struct RetainedSceneTemplate {
    pub schema_version: u16,
    pub generation: SceneGeneration,
    pub viewport: RetainedViewport,
    pub camera: CameraTemplate,
    pub nodes: Vec<RetainedNode>,
    pub resources: ResourceManifest,
    pub materials: Vec<MaterialTemplate>,
    pub draw_phases: Vec<DrawPhase>,
    pub chrome: CompanionChromeReservation,
    pub privacy: PrivacyProjection,
    pub checksum: SceneTemplateChecksum,
}
```

The template owns content that should not change on an ordinary animation tick:

- node hierarchy and stable IDs;
- local geometry, glyph identity, sprite identity, and mesh identity;
- base transforms and bounds;
- material/pipeline selection;
- draw phase and stable transparency order;
- clip/mask definitions;
- hit-test targets;
- chrome reservations and privacy claims;
- resource requirements.

A template is rebuilt when semantic composition or layout changes, including:

- pet identity, stage, art, or palette role mapping;
- earned prop or tank-life composition changes;
- room/biome/dialect composition changes;
- helper/lifecycle changes that add or remove nodes;
- viewport dimensions or backing scale changes;
- font/atlas policy changes;
- renderer schema or shader/resource version changes.

Time passing alone does not rebuild the template. Time may update bounded dynamic
content: current pet expression cells, corruption cells, particle instances,
ambient/mote/activity instances, and chest-bubble instances. Those updates use
generation-checked slots or buffers declared by the template and must not rebuild
unrelated node hierarchy, materials, resources, or draw phases.

### `DynamicSceneState`

```rust
pub struct DynamicSceneState {
    pub generation: SceneGeneration,
    pub semantic_tick: u64,
    pub node_content: Vec<NodeContentDelta>,
    pub instance_groups: Vec<InstanceGroupDelta>,
    pub dirty: ContentDirtySet,
}

pub enum NodeContentDelta {
    GlyphSlots {
        node: NodeId,
        first_slot: u32,
        glyphs: Vec<GlyphInstanceState>,
    },
    SpriteSlots {
        node: NodeId,
        first_slot: u32,
        sprites: Vec<SpriteInstanceState>,
    },
}
```

The exact storage may become fixed-capacity arrays, sparse writes, or ring buffers.
The contract is more important than the container:

- the template declares capacity and resource compatibility;
- ordinary blink/twinkle/corruption changes update existing slots;
- ambient and bubble effects update bounded instance groups;
- exceeding declared capacity is a recoverable generation-change request, not an
  unchecked allocation in the frame encoder;
- content deltas are deterministic for fixed semantic tick and fixture state;
- applying a stale-generation delta preserves the last good content.

### `RetainedFrameState`

```rust
pub struct RetainedFrameState {
    pub generation: SceneGeneration,
    pub frame_index: u64,
    pub elapsed_ms: u64,
    pub camera: CameraFrameState,
    pub nodes: Vec<NodeFrameState>,
    pub globals: FrameGlobals,
    pub dirty: FrameDirtySet,
}

pub struct NodeFrameState {
    pub node: NodeId,
    pub transform: Transform3,
    pub opacity: f32,
    pub visible: bool,
    pub material_params: MaterialParams,
}
```

The frame state carries only per-paint values:

- pet X/Y/Z motion, scale, bob, facing, and future rotations;
- parallax plane transforms;
- wall-shadow and bed-projection changes;
- aura/feed/helper pulse values;
- continuous particle simulation state or emitter uniforms where the effect is
  not represented by a `DynamicSceneState` instance-group update;
- camera motion;
- gauge fractions and dim amount;
- per-node visibility or material parameters.

The implementation may encode this as dense arrays, sparse deltas, or both. The
observable contract is that unchanged template content is not cloned, revalidated,
re-sorted, or re-uploaded every frame, and dynamic-content changes do not force a
full template rebuild.

### Coordinate system

The presentation-facing world uses familiar companion units:

- X increases right.
- Y increases down, matching existing cell and round layout coordinates.
- Z increases toward the viewer.
- The neutral pet plane is Z = 0.
- Existing normalized depth `[-1, 1]` maps into a documented world-Z range.

The backend owns conversion into GPU clip coordinates. Keeping Y-down in the
presentation contract minimizes migration errors and preserves current typed
geometry. Shader matrices may use their native convention internally.

### `Transform3`

```rust
pub struct Transform3 {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
}
```

The first shipping slices use translation, uniform XY scale, and identity rotation
for current parity. The type supports future yaw/pitch/roll and thickness without
changing every node contract. Validation rejects non-finite values, zero/negative
scale where unsupported, and non-normalizable rotations.

Parent/child transforms are supported, but the first migration should keep a
shallow hierarchy: scene root, depth-plane groups, pet group, chrome group, and
leaf nodes. A deep general scene graph is not required.

## Rendering Primitives

### Glyph instances

Current pet, prop, room, tank-life, and ambient identity is largely glyph-based.
The renderer treats glyphs as atlas-backed instanced quads:

```rust
pub struct GlyphPrimitive {
    pub atlas_entry: AtlasEntryId,
    pub local_position: [f32; 2],
    pub size: [f32; 2],
    pub color: Rgba8,
    pub background: Option<Rgba8>,
    pub style: GlyphStyle,
}
```

Requirements:

- no per-frame text shaping or native font lookup;
- template/resource compilation preflights the bounded glyph repertoire required
  by the pet species/stage, room dialect, props, tank life, HUD character set, and
  declared dynamic effects—not merely glyphs visible in the first frame;
- atlas misses are resolved before the template becomes active;
- atlas pages are bounded and observable;
- missing glyphs use an explicit visible fallback and record a diagnostic;
- identical glyph/style/color combinations batch together;
- pet depth scale changes quad transforms, not glyph rasterization every frame.

HUD text may use glyph runs rather than one instance per semantic cell, but it
must use the same atlas/resource system and remain outside native text drawing.

### Sprites and rasters

Sprite primitives cover future pixel-art treatments, authored prop textures,
noise tiles, and pre-rendered effects. Resources are immutable texture regions
with stable sampling and color-space metadata.

The current `SmoothRasterRef` evolves from a descriptive name into a validated
`ResourceId` or atlas region. A missing raster backend is an error, not a silent
shape or cell fallback.

### Shapes

The first GPU shape pipeline supports the current typed needs:

- ellipse;
- rectangle and rounded rectangle;
- ring/arc;
- linear and radial gradients;
- optional signed-distance softness for shadow/aura edges.

Shapes should be represented as instanced analytic/SDF primitives where practical,
not freshly tessellated every frame. Static complex paths, if later required, are
compiled to meshes outside the frame path.

### Meshes and 2.5D cards

The retained contract includes a narrow mesh primitive:

```rust
pub struct MeshPrimitive {
    pub mesh: ResourceId,
    pub material: MaterialId,
    pub bounds: Bounds3,
}
```

The first program needs only generated quads/planes and optional shallow extruded
cards. General model loading is deferred. This primitive is the escape hatch for
future coral, rocks, glass rims, or pet treatments that cannot be expressed as
sprites or analytic shapes.

### Materials

Initial material families:

1. unlit glyph/sprite;
2. unlit solid/gradient shape;
3. multiply shadow;
4. additive/screen glow;
5. simple lit sprite/mesh with ambient plus bounded key/rim light;
6. screen-space chrome.

Materials are typed and validated. Arbitrary user shaders and a general shader
node graph are non-goals.

### Draw phases and transparency

The renderer uses a small explicit phase list rather than one global sort:

1. tank background and fixed far geometry;
2. opaque/alpha-tested world geometry with depth testing;
3. stable-order translucent world layers;
4. pet/shadow/projection effects;
5. foreground world layers;
6. aura, status, and trouble overlays;
7. screen-space HUD and gauges;
8. final round mask/dim/composite as required by the chosen pass structure.

Opaque geometry may use the depth buffer. Transparent content keeps deterministic
phase plus stable node order; order-independent transparency is not required.

## Camera And 2.5D Model

The initial camera is an orthographic 2.5D camera that reproduces current Smooth
output. Existing Z-derived scale and perspective-Y behavior remains the parity
oracle.

The runtime supports two projection modes:

```rust
pub enum CameraProjection {
    Orthographic2_5d { world_bounds: Bounds2 },
    Perspective { vertical_fov_radians: f32, near: f32, far: f32 },
}
```

Perspective mode is an enabled capability, not part of the first parity slice.
Any switch to a perspective camera requires a separate visual spec proving HUD,
protected regions, pet identity, and motion comfort.

Camera motion must be bounded independently from pet motion. No camera shake,
continuous orbit, or depth-of-field effect is introduced merely because the
runtime can express it.

## Resource System

### Resource compilation

`ResourceCompiler` turns a `ResourceManifest` into backend resources before a
new template becomes active:

- glyph atlas pages;
- sprite/texture atlas pages;
- static vertex/index buffers;
- material and pipeline keys;
- samplers;
- mask textures or generated geometry.

Compilation may run off the main thread when the underlying APIs permit it. The
active renderer continues presenting the previous generation until compilation
succeeds. A failed generation never replaces the last good scene.

### Glyph font source

The hot-path decision is locked: no system font resolution per frame.

The final source font remains an explicit Phase 0 decision, but the decision
contract is now fixed:

- **Preferred final policy:** bundle a redistributable monospace font whose license
  permits source inclusion, binary redistribution, subsetting if used, generated
  atlas redistribution, and modification if required. Check the license text and
  attribution into the release artifact; do not rely on an undocumented host font.
- **Transitional policy:** native CoreText rasterization may populate atlas entries
  during migration, but never during frame encoding. Its cache key includes the
  resolved font PostScript name/version, backing scale, glyph, weight, and raster
  policy so review evidence is reproducible on the recorded host.
- **Fallback chain:** chosen bundled font, then a bundled replacement-glyph entry.
  Production rendering does not silently fall back to arbitrary system fonts. A
  missing required glyph is a resource-preflight failure that preserves the last
  good generation.
- **Coverage manifest:** derive the required scalar set from current pet art,
  species/dialect marks, props, tank life, HUD/chrome, Preview Lab fixtures, and the
  replacement character. Store that sanitized set or its deterministic hash in the
  resource artifact. New content cannot ship without extending preflight fixtures.
- **Unicode contract:** atlas keys are Unicode scalar sequences, not `char`-sized
  assumptions or UTF-16 code units. Phase 0 must prototype at least one non-BMP
  scalar and one multi-scalar sequence even if today's shipping repertoire is BMP,
  proving decoding, lookup, rasterization, capture, and replacement behavior.
- **Metrics contract:** cell advance, baseline, ascent/descent, weight, and pixel
  snapping are versioned resource data. The full species/stage/state matrix must be
  compared at required sizes before a font change is accepted.

Phase 0 records font file bytes and release-size delta separately from generated
atlas memory. Font licensing, glyph coverage, non-BMP behavior, metrics parity, and
release attribution must pass before the final atlas pipeline is approved.

### Cache bounds

All caches have explicit limits and metrics. No cache clears wholesale in the
middle of a frame. Growth beyond a limit schedules generation rebuild/eviction at
a safe boundary or fails with a recoverable diagnostic.

## Backend Choice

### Option A: Rust software framebuffer as shipping backend

Pros:

- simpler headless determinism;
- removes per-glyph native draw calls;
- straightforward one-bitmap AppKit submission;
- useful as a prototype and reference renderer.

Cons:

- still redraws and uploads pixels on the CPU;
- richer 2.5D lighting, particles, mesh transforms, and camera effects remain CPU
  work;
- likely improves the current floor without reaching the full end goal.

Decision before Phase 0: required bounded comparison prototype and potentially a
narrow reference path. It is not presumed to be the shipping architecture, but it
must be measured rather than dismissed.

### Option B: GPU 2D vector renderer as the core

Pros:

- strong path, gradient, and text-oriented 2D rendering;
- can reduce custom shape implementation.

Cons:

- Glorp's target includes atlas glyphs, sprites, depth planes, depth testing,
  camera transforms, lit cards/meshes, and selective 3D;
- a 2D scene API would become another layer to escape for those features.

Decision: do not make it the core. A vector renderer may later assist with complex
path rasterization if a concrete need justifies it.

### Option C: Direct retained `wgpu` renderer

Pros:

- Metal-backed on macOS while remaining portable;
- natural fit for instancing, atlases, transforms, depth, particles, and meshes;
- offscreen textures and readback support native review capture;
- one retained model can grow from current 2.5D into selective 3D.

Cons:

- larger dependency and compile footprint;
- device/surface recovery and shader validation become Glorp responsibilities;
- deterministic cross-device pixel tests require tolerances and semantic sidecars;
- more initial engineering than a one-bitmap software renderer.

Decision before Phase 0: leading target candidate.

### Chosen route: staged hybrid

Use the retained contracts first, then prove them through exact semantic/resource
artifacts plus a narrow software or offscreen visual reference path, then ship the
`wgpu` backend if the Phase 0 spike passes. The hybrid is a migration strategy, not
two permanent equal renderers. A full CPU renderer is not required merely to claim
determinism: semantic contracts are the exact oracle, while pixel evidence may
come from a deliberately limited reference rasterizer or GPU readback.

### Phase 0 backend go/no-go gate

Before implementation planning locks `wgpu` into production dependencies, a
narrowly-scoped benchmark suite must render the same representative 360x360 and
720x720 fixtures through these candidates where technically applicable:

1. targeted batching/cached AppKit improvements using the existing draw-command
   seams;
2. retained AppKit/Core Animation layers with static content reused and only
   dynamic transforms/content updated;
3. a persistent Rust software bitmap plus glyph atlas with one native submission;
4. retained `wgpu`/Metal instancing through the proposed AppKit surface bridge.

These prototypes are disposable except for reusable benchmark/contracts code. They
must use the same visual fixture, frame cadence, backing scales, warmup, poll
exclusion, capture checks, and metrics schema. The suite records process CPU,
render-thread CPU, frame misses, renderer submissions, energy evidence, RSS,
renderer-attributable memory, clean/incremental build time, executable/app/package
size, and visual-contract failures. It must also prove all of the following:

- a `wgpu` surface can be hosted by the existing AppKit window without replacing
  application/menu/fullscreen lifecycle code;
- a bounded process can render, resize, read back one offscreen frame, and exit
  cleanly through the existing review harness shape;
- device/surface errors can be converted into static recoverable categories
  without unwinding through Objective-C;
- release binary size, clean build time, and incremental build time are recorded;
- the release process can exclude unused non-Metal backends on macOS without
  breaking tests or packaging;
- a simple batched glyph/quad frame demonstrates materially fewer CPU samples in
  AppKit/CoreText than the current Smooth painter.

`wgpu` receives production approval only if it passes the absolute budgets in this
spec and is the only candidate that meets the retained 2.5D requirements without a
materially cheaper candidate also meeting all current-scope performance, energy,
visual, reliability, and delivery gates. If two candidates pass, prefer the one
with lower lifecycle, package, and maintenance cost unless the first approved
renderer-native 2.5D feature demonstrates a concrete capability gap.

If the spike fails one of these gates, the retained contracts remain valid and the
backend decision returns to review. The project must not contort AppKit lifecycle
or accept an unmeasured package/build regression solely to preserve the proposed
library choice.

### Build, feature, and runtime gating

The retained contracts and headless artifact DTOs are platform-neutral and compile
on every CI/release target. Native surface and GPU implementation modules are macOS
target dependencies and do not make Linux or Windows builds link AppKit, Metal, or
unused GPU backends.

During Phases 0-4, the backend is gated twice:

- **compile-time:** a non-default `retained-renderer` Cargo feature enables the
  candidate backend and its dependencies on macOS. `dev-preview` remains independent.
- **runtime:** hidden `--renderer retained` selection or an explicitly-owned review
  harness activates it. Normal release behavior remains Smooth until Phase 5.

The repository's existing contracts are mandatory:

- `cargo clippy --all-targets --all-features -- -D warnings` must compile the
  retained feature;
- `cargo check --locked --no-default-features --all-targets` must remain green;
- publish builds continue using `--release --locked --no-default-features` for
  `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`;
- the hidden Preview Lab command remains excluded from published binaries.

Before Phase 5, release planning must make an explicit feature decision. The
recommended end state is a release-default, macOS-target retained backend that does
not enable `dev-preview`; if Cargo feature topology cannot express that without
shipping unwanted code on other targets, the build scripts must pass an explicit
release feature rather than overloading the current default feature set.

## Frame Pacing And Invalidation

The renderer is not required to repaint continuously at one fixed rate.

Proposed modes:

- **Occluded/minimized:** zero rendering work; semantic state may update and the
  next visible frame catches up from monotonic time.
- **Static review or no active motion/content change:** render on invalidation only.
- **Ambient motion:** target 15 FPS.
- **Active pulse/interaction:** target up to 30 FPS.
- **Deterministic capture:** explicit fixture-defined cadence independent of wall
  clock and visibility.

Frame pacing should use display-aware scheduling where practical. An ordinary
frame must not rebuild the template, rasterize glyphs, allocate per-node objects,
regenerate dynamic glyph populations, or block on usage-provider work.

## macOS Host Boundary

AppKit responsibilities:

- application activation and menu lifecycle;
- window creation, movement, resize, fullscreen, and close/minimize behavior;
- attaching the renderer's drawable surface to the content view;
- forwarding logical size, backing scale, visibility, and input events;
- scheduling or waking the renderer according to the shared frame-pacing policy;
- preserving Objective-C callback unwind guards.

Rust renderer responsibilities:

- device/surface configuration;
- scene/template compilation;
- all glyph, shape, sprite, mesh, gradient, mask, HUD, gauge, and dim rendering;
- resource lifetime and recovery;
- offscreen capture/readback;
- frame metrics and diagnostics;
- last-good frame/scene behavior.

`drawRect` should no longer traverse scene items. Ideally the GPU-backed view does
not use `drawRect` as the primary animation path at all; it presents the current
surface texture from the renderer's scheduled frame callback.

### Input, accessibility, and semantic overlay

The drawable is not allowed to become an opaque accessibility dead zone. AppKit
continues to own responder-chain behavior, keyboard focus, pointer event delivery,
and macOS accessibility objects. The retained template supplies sanitized semantic
targets and bounds; the host projects them into native view coordinates.

For the current noninteractive companion, the minimum contract is:

- the window and habitat expose stable roles and localized names without exposing
  source names, project names, token strings, paths, or raw diagnostics;
- HUD values that are meaningful to sighted users have equivalent sanitized value
  descriptions, with update announcements rate-limited to discrete semantic
  changes rather than every animation frame;
- decorative pet cells, particles, shadows, and props are not exposed as hundreds
  of native accessibility children;
- keyboard focus and existing menu/fullscreen/close behavior remain native;
- pointer hit testing, if enabled later, resolves through stable `HitTargetId`
  metadata and the same transform/camera snapshot that produced the presented
  frame; stale-generation hits are ignored;
- hiding, minimizing, fallback, resize, and device recovery do not leave stale
  accessible children or an invisible focused target.

Phase 0 must include a native prototype/audit proving that a Metal-backed child
surface can coexist with AppKit focus, keyboard commands, pointer coordinate
conversion, and a small accessibility overlay. Interaction behavior beyond this
bridge still requires a separate product design.

## Error Handling And Recovery

### Template and frame validation

Reject before activation:

- stale generation references;
- duplicate stable IDs;
- missing resources/materials;
- non-finite bounds, transforms, colors, camera values, or material parameters;
- unsupported blend/depth combinations;
- invalid hierarchy cycles;
- out-of-range atlas references;
- privacy claims inconsistent with the target surface.

### GPU initialization failure

During staged rollout:

1. record a static sanitized diagnostic category;
2. keep or start the existing Smooth AppKit backend;
3. expose the fallback in review/performance logs;
4. do not repeatedly retry every frame.

After Smooth retirement, the permanent fallback contract must be separately
approved. At minimum it must show the last good captured frame or a privacy-safe
static Glorp fallback rather than a blank/crashing window.

### Device loss and surface errors

- Recover outdated/lost surface configurations through bounded reconfiguration.
- Attempt device recreation at most once per loss episode.
- Keep the last successfully rendered/captured frame visible where the host allows.
- If recovery fails, transition to the staged fallback and record the reason.
- Never panic through an Objective-C callback or `wgpu` error callback.

The native fault harness must inject at least: initialization failure, outdated and
lost surface acquisition, zero-size/minimized configuration, resize during capture,
and one simulated device-loss episode. Every case has a maximum retry count, a
terminal state, and an automatic review-run exit; no test may depend on physically
removing a display or waiting indefinitely for a driver event.

### Resource compilation failure

A failed new generation does not replace the active generation. Diagnostics use
static categories plus sanitized resource IDs; they must not include private
runtime data or arbitrary shader/source text in external artifacts.

## Privacy

The retained template receives the same `PrivacyProjection` as the round
companion. It must not contain source names, exact token strings, project names,
file paths, prompts, responses, raw diagnostics, or an unprojected pet seed.

GPU buffers, texture labels, node labels, capture sidecars, and renderer metrics
are all part of the privacy boundary. Debug labels use stable sanitized IDs.

Readback images and renderer artifacts follow Preview Lab ownership and redaction
rules. No raw buffer dump is written outside an explicitly-owned review directory.

Automated scans reject known privacy field names and fixture secrets in JSON,
labels, filenames, and logs. Binary GPU buffers are not treated as publishable
diagnostics. A fault path must emit only an allowlisted static category plus
sanitized stable IDs; backend validation messages and shader source stay in local
development stderr unless explicitly scrubbed for an owned artifact.

## Preview Lab And Review Contract

Preview Lab remains the semantic regression harness. The new renderer adds
additive artifacts rather than replacing existing text/cell/smooth evidence during
migration.

Proposed artifacts:

- `frames/<id>.retained-template.json`
  - schema/generation;
  - sanitized node IDs and roles;
  - parent relationships;
  - primitive/material/resource kinds;
  - local bounds, base transforms, draw phases, clip/depth/blend policy;
  - resource counts and template checksum;
  - chrome reservations and privacy claims.
- `frames/<id>.retained-frame.json`
  - generation/frame index/elapsed time;
  - camera values;
  - dynamic node transforms, visibility, opacity, and material parameters;
  - dirty/update counts and frame checksum.
- `frames/<id>.retained-resources.json`
  - atlas page counts and dimensions;
  - glyph/sprite/mesh/material counts;
  - sanitized resource hashes;
  - estimated CPU/GPU bytes.
- `frames/<id>.retained-readback.png`
  - optional offscreen GPU or reference-renderer image for visual review.
- `frames/<id>.retained-metrics.json`
  - template builds, uploads, draw calls, encoded instances, CPU frame time, GPU
    timing when supported, atlas misses, and fallback/recovery state.

Manifest entries get first-class retained artifact types and paths. Existing
Smooth plan/parity/motion artifacts remain until the new contracts supersede them
through an explicit schema migration.

### Determinism rules

- Template and frame semantic artifacts are exact deterministic outputs for fixed
  input, viewport, clocks, and renderer schema.
- Resource hashes are deterministic for bundled resources and generated geometry.
- GPU readback images use tolerance-based or perceptual comparison, not universal
  byte-for-byte equality.
- General CI can validate semantic artifacts without a GPU.
- macOS native CI/review validates Metal initialization, nonblank readback,
  dimensions, selected pixel-region properties, frame counts, and recovery logs.

## Performance And Energy Budgets

Budgets apply to an optimized release build on the Phase 0 benchmark machine. The
implementation plan must record machine model, CPU/GPU family, memory, macOS
version, display refresh rate, backing scale, power source, and whether the window
is frontmost. Additional supported Apple Silicon machines are compatibility
evidence; they do not silently redefine the primary budget.

Absolute gates below apply to the pinned primary machine. Each candidate and later
release qualification also reports deltas against the checked-in Smooth baseline
from the same machine/run configuration. A backend cannot pass solely by exploiting
a newer/faster review machine. On additional supported Apple Silicon systems, it
must show no material regression versus Smooth in process CPU, frame misses, energy
evidence, or memory even when the primary absolute percentages are not portable.

For process CPU budgets, the standard run is five minutes after a 30-second
warmup. Sample the companion once per second. Report all samples and separately
report an **inter-poll set** that excludes samples from five seconds before through
ten seconds after each observed usage-poll start. The median and p95 requirements
below apply to that inter-poll set. Poll-inclusive mean, median, p95, and maximum
are still reported but are not renderer acceptance gates. CPU percentages use the
macOS convention where one fully-used core is 100%.

### Visible steady state at 360x360 logical points

- process CPU median: **<= 5%** outside usage-poll windows;
- process CPU p95: **<= 8%** outside usage-poll windows;
- render-thread CPU encode/update time p95: **<= 2 ms/frame**;
- missed-frame rate: **< 1%** over a five-minute ambient run;
- template rebuilds during unchanged ambient motion: **0**;
- glyph atlas misses after template activation: **0**;
- steady-state resource uploads after warmup: transforms/uniforms only;
- normal draw-call target: **<= 12 per frame** at current feature scope.

The full primary run is repeated at backing scale 1 where available and backing
scale 2. The scale-2 run is the shipping gate on Retina hardware; scale 1 is a
diagnostic/compatibility result and must not conceal excess physical-pixel work.

### Visible steady state at 720x720 logical points

- process CPU median: **<= 8%** outside usage-poll windows;
- render-thread CPU encode/update time p95: **<= 3 ms/frame**;
- no frame-preparation panic, device loss, unbounded resource growth, or fallback.

### Hidden/minimized/occluded

- over a 60-second fully occluded interval after a 10-second settling period,
  renderer frame count and GPU submission count do not increase;
- whole-process CPU median is no more than 1 percentage point above the same
  process with renderer scheduling disabled in the benchmark harness;
- no GPU submissions while fully occluded, except explicit bounded capture runs;
- semantic updates may occur without rendering and must present correctly on reveal.

### Memory and startup

- renderer-attributable steady GPU resources: target **<= 64 MiB** at 720x720;
- renderer-attributable steady CPU caches: target **<= 32 MiB**;
- no unbounded atlas, pipeline, node, or capture cache;
- warm start to first valid frame: target **<= 500 ms**, measured from completion
  of state load to the renderer's first successful present, p95 over 20 launches
  on the benchmark machine.

Memory accounting separates physical footprint/RSS, renderer-attributable CPU
allocations, IOSurface/graphics allocations, and estimated GPU resources. The
64 MiB target is not a substitute for a whole-process leak or footprint check.

### Energy, build, and distribution cost

- Phase 0 records a repeatable macOS energy trace or counter set for each candidate
  over the same five-minute ambient run. Until a stable absolute unit is validated,
  the gate is **no worse than Smooth** and the selected candidate must be no worse
  than the best passing candidate by more than **10%** in the chosen normalized
  energy measure.
- Production dependency approval requires checked-in before/after measurements for
  clean release build time, incremental rebuild time after a renderer source edit,
  stripped executable bytes, `Glorp.app` archive bytes for both Darwin targets, and
  npm platform package packed bytes.
- Initial hard rollback limits are: more than **15 MiB** added to either stripped
  Darwin executable, more than **20 MiB** added to either compressed companion app
  artifact, more than **20%** added to clean release build time, or more than
  **25%** added to the measured renderer-edit incremental build. Phase 0 may replace
  these provisional limits only through an explicit reviewed decision with the raw
  artifacts attached; silence is not approval.
- Size reports separate the candidate backend, bundled font/license, shaders, and
  other resources. Debug symbols, `target/` caches, Preview Lab output, and cargo
  registry/build caches are excluded; the actual publish inputs are included.

Poll/provider spikes are measured separately. The render thread must not perform
SQLite queries or wait for helper processes. Poll-inclusive runs still record
whole-process CPU so renderer improvements cannot hide regressions elsewhere.

## Observability

A bounded `RendererMetrics` snapshot should expose:

```rust
pub struct RendererMetrics {
    pub backend: RendererBackend,
    pub active_generation: SceneGeneration,
    pub template_build_count: u64,
    pub template_build_micros: u64,
    pub frame_count: u64,
    pub frame_cpu_micros_p50: u64,
    pub frame_cpu_micros_p95: u64,
    pub gpu_micros_p95: Option<u64>,
    pub draw_call_count: u32,
    pub instance_count: u32,
    pub upload_bytes: u64,
    pub atlas_page_count: u16,
    pub atlas_miss_count: u64,
    pub missed_frame_count: u64,
    pub surface_reconfigure_count: u64,
    pub device_recovery_count: u64,
    pub fallback_count: u64,
    pub last_error_category: Option<&'static str>,
}
```

Metrics are available to review capture and optional development logging. They are
not continuously persisted to user state.

Benchmark result artifacts additionally record binary hash/path/mtime, git commit,
Cargo feature set, backend dependency versions/features, executable/app/package
bytes, build timings, process footprint, energy method/result, backing scale, and
the complete sample series. Summary tables without their raw owned artifacts are
not accepted as Phase 0 evidence.

## Migration Program

### Phase 0: contract and benchmark freeze

Purpose: protect current output and establish repeatable performance evidence.

- Add a checked-in benchmark protocol for visible inter-poll, poll-inclusive,
  occluded, resize, and review-capture runs.
- Record current Smooth semantic artifacts and native screenshots at 260, 360,
  480, 720, and 960 sizes where practical.
- Add retained artifact schema types without a shipping backend.
- Implement the four-way candidate comparison: targeted AppKit batching, retained
  AppKit/Core Animation, persistent software bitmap/atlas, and `wgpu`/Metal.
- Prototype the AppKit surface, resize/backing-scale, bounded readback, focus/input,
  accessibility overlay, Objective-C unwind boundary, and injected-fault path.
- Resolve font source/licensing/coverage, including non-BMP and multi-scalar tests.
- Record the exact compile/runtime feature policy and all build/package-size costs.

Exit gates:

- repeatable baseline scripts and result format;
- explicit benchmark hardware;
- no raw user data in benchmark/review artifacts;
- raw baseline/candidate samples and proof artifacts are checked in or attached to
  an owned review record;
- one candidate satisfies the backend selection rule, or backend selection is
  explicitly reopened;
- approved font, feature, dependency, attribution, size, and rollback decisions;
- all five publish-target `--no-default-features` builds remain green;
- surface/capture/accessibility/fault prototypes exit automatically and pass privacy
  scans.

Rollback trigger: any candidate that requires replacing AppKit lifecycle, cannot
produce bounded capture, cannot preserve focus/accessibility semantics, violates a
hard size/build limit, or cannot recover without unwinding is rejected before
production integration.

### Phase 1: split template from frame state

Purpose: fix the architecture before changing the painter.

- Introduce `RetainedSceneTemplate`, `DynamicSceneState`, `RetainedFrameState`, IDs,
  validation, and checksums.
- Derive them from the current Smooth/round scene without changing visible output.
- Make current AppKit Smooth painting consume the template plus frame state through
  a compatibility adapter.
- Prove ambient ticks do not reconstruct static node content.

Exit gates:

- Classic flatten parity remains exact where applicable;
- deterministic retained template/frame artifacts exist;
- unchanged ambient motion causes zero template rebuilds;
- current native Smooth screenshots remain accepted.

Rollback trigger: if the compatibility adapter requires parallel semantic scene
derivation or ordinary time progression rebuilds unrelated template structure, stop
and repair the product-to-render projection before backend work.

### Phase 2: resource compiler and reference evidence path

Purpose: validate batchable resources and native-independent glyph preparation
before live surface integration.

- Implement glyph atlas compilation and bounded resource manifests.
- Model current time-varying glyph content explicitly: blink/expression, Glitch
  corruption, particle gutters, ambient/motes/activity glyphs, and chest bubbles
  update through declared dynamic slots or instance groups rather than template
  reconstruction.
- Add visual reference support sufficient for offscreen parity review. It may be a
  deliberately narrow software rasterizer or the Phase 0 offscreen GPU path.
- Exact determinism comes from retained semantic/resource artifacts; the reference
  image path must consume the same contracts but need not become a second complete
  shipping renderer.
- Do not yet replace the shipping AppKit painter.

Exit gates:

- all required glyphs/resources preflight before activation;
- zero atlas misses during fixture strips;
- semantic art/effect strips show bounded content-buffer updates with zero
  unrelated template rebuilds;
- retained readback visibly preserves Glorp identity, props, tank life, HUD, and
  gauges;
- resource caches stay within approved bounds.

Rollback trigger: atlas/resource preflight that depends on per-frame native text,
unbounded cache growth, or undeclared topology changes blocks Phase 3.

### Phase 3: hidden selected companion backend

Purpose: attach the Phase 0-approved retained backend to the native window. The
remaining bullets assume `wgpu`/Metal wins; substitute the approved candidate's
equivalent surface and batching mechanisms otherwise.

- Add hidden `--renderer retained` mode.
- Render static parity through glyph/sprite/shape pipelines.
- Implement resize, backing-scale changes, aperture mask, readback capture, and
  last-good generation behavior.
- Keep the existing Smooth AppKit backend unchanged and selectable.

Exit gates:

- bounded native captures pass at required sizes/states;
- no AppKit per-cell or per-shape drawing occurs in retained mode;
- fallback and recovery state is visible in render logs;
- visual review accepts static parity;
- keyboard/menu/fullscreen behavior and the accessibility audit pass;
- retained feature builds in all-features CI while no-default release builds remain
  valid across the five publish targets;
- executable/app/package and build-time deltas remain within approved Phase 0 limits.

Rollback trigger: repeated blank frames, lifecycle regressions, privacy leakage,
unbounded capture, or failure to preserve Smooth as a selectable fallback returns
the backend to hidden prototype status.

### Phase 4: retained motion and depth

Purpose: move current Smooth animation onto frame-state and GPU updates.

- Pet X/Y/Z, bob, parallax, wall shadow, floor projection, aura, dim, particles,
  gauges, blink/expression changes, Glitch corruption, ambient effects, and chest
  bubbles update without unrelated template rebuild.
- Introduce adaptive 15/30 FPS pacing and occlusion suspension.
- Tune batching and uploads against the performance budgets.

Exit gates:

- motion strips preserve deterministic semantic values;
- native motion reads as current Glorp Smooth motion or an explicitly approved
  improvement;
- 360x360 and 720x720 performance budgets pass;
- five-minute stability run has no device recovery or resource growth;
- energy evidence passes the selected-candidate gate at Retina backing scale.

Rollback trigger: if transforms-only ambient motion still rebuilds static content,
misses the primary CPU/energy budgets, or regresses the best cheaper passing
candidate materially, do not proceed to the default flip.

### Phase 5: default flip and AppKit painter retirement

Purpose: make retained rendering the shipping companion path.

- Flip the default only after visual, privacy, performance, recovery, packaging,
  and review gates pass.
- Keep Smooth AppKit behind an explicit temporary fallback for one release unless
  release planning approves immediate retirement.
- Remove per-cell AppKit painting, native glyph caches, and obsolete Smooth
  prepared-frame structures after fallback retirement.

Exit gates:

- release bundle uses an optimized profile;
- npm/package smoke tests include the new backend;
- no fallback occurs in the release qualification matrix;
- living docs and Preview Lab contracts identify retained as the primary backend.

Release qualification matrix:

- debug and optimized release companion bundles on supported Apple Silicon;
- `retained-renderer` on/off and `dev-preview` on/off where valid;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo check --locked --no-default-features --all-targets`;
- all five publish target release builds, with native retained execution/capture on
  Apple Silicon and at least launch/capture or an explicitly documented hardware
  exception for Darwin x86_64;
- initialization failure, resize/backing-scale changes, minimize/occlude/reveal,
  surface faults, device-loss simulation, capture, fallback, and clean exit.

Rollback trigger: any release-matrix fallback, crash, blank frame, inaccessible
surface, privacy failure, or hard package/build regression keeps Smooth as default.

### Phase 6: first renderer-native 2.5D feature

Purpose: prove the new architecture makes visual evolution easier rather than only
cheaper.

Choose one separately-specified feature, such as:

- bounded camera push/pull plus glass-rim parallax;
- a lit shallow-card habitat prop;
- GPU bubble/mote particles with true depth;
- a localized rim light tied to feed activity.

The feature must primarily add template resources and frame parameters, not create
a new companion-specific paint path.

## Testing And Verification

### Pure contract tests

- deterministic stable ID assignment;
- stale-generation delta rejection;
- deterministic dynamic-content slot assignment and bounded-capacity behavior;
- template/frame validation and finite transform checks;
- hierarchy cycle rejection;
- draw-phase and stable transparency ordering;
- privacy projection completeness;
- resource manifest completeness;
- template checksum stability;
- unchanged semantic input does not rebuild static content;
- blink, corruption, ambient, and bubble samples update only their declared
  dynamic content groups;
- fixed time samples produce deterministic frame states.

### Renderer tests

- glyph atlas packing, bounds, misses, and bounded growth;
- required Unicode repertoire preflight, replacement behavior, a non-BMP scalar,
  and a multi-scalar atlas key;
- sprite and shape instance encoding;
- material/pipeline key stability;
- round aperture and nested clip behavior;
- depth-test and transparency phase behavior;
- resize and backing-scale reconfiguration;
- offscreen readback dimensions and nonblank content;
- device/surface error classification and bounded recovery;
- last-good generation stays active after a failed replacement.

### Cross-renderer parity

During migration, retained and Smooth fixtures must agree on:

- pet identity/art source and semantic role counts;
- required layer/node presence;
- prop and tank-life identities and depth groups;
- pet anchor/depth/scale/parallax values;
- HUD/gauge values and chrome reservations;
- privacy claims;
- accepted visual composition at native review sizes.

Exact antialiased pixels are not the parity contract.

### Native smoke

Bounded direct `companion-app` runs cover:

- normal;
- active pulse;
- asleep/calm;
- helper trouble;
- far/neutral/near depth;
- 260, 360, 480, 720, and at least one large resize size;
- minimize/occlude/reveal;
- repeated live resize;
- simulated recoverable surface error;
- simulated initialization and device-loss failure with bounded retry/fallback;
- keyboard focus, menu commands, pointer-coordinate projection, and accessibility
  overlay audit;
- review capture and clean automatic exit.

Each run records backend, frame count, template builds, draw calls, misses,
recovery/fallback state, panic state, and preparation/render errors.

### Performance protocol

The benchmark runner records:

- exact commit and release profile;
- machine model and OS version;
- CPU/GPU family, memory, backing scale, refresh rate, power source, and frontmost
  state;
- renderer/backend and viewport;
- visible inter-poll CPU distribution;
- poll-inclusive CPU distribution;
- observed poll start timestamps and the exact excluded sample windows;
- RSS and renderer resource estimates;
- physical footprint, IOSurface/graphics allocation evidence, and energy result;
- clean/incremental build timings and executable/app/npm packed sizes;
- template/frame metrics;
- an eight-to-ten-second native stack sample when a budget fails;
- process cleanup status.

A performance claim is not accepted from Activity Monitor observation alone.

## Acceptance Criteria

### Structural

- AppKit contains no semantic scene derivation and no per-cell/per-shape companion
  painter in retained mode.
- Static node content and resources live in `RetainedSceneTemplate`.
- Bounded glyph/sprite/instance substitutions live in `DynamicSceneState` or
  generation-checked content deltas.
- Per-paint motion lives in `RetainedFrameState` or generation-checked frame deltas.
- Ambient frames do not clone or rebuild static scene content.
- Glyph, sprite, shape, and mesh work is batched through bounded pipelines.
- Platform-neutral contracts contain no AppKit, Metal, `wgpu`, or raw window types.
- The renderer can express current Smooth depth, projection, parallax, blend,
  gradient, clip, and chrome behavior.
- The renderer has a documented camera, world-Z, material, and mesh escape hatch
  sufficient for separately-designed 2.5D/3D additions.

### Visual and product

- Glorp remains recognizable as the existing generated pet.
- Props, tank life, room dialect, ambient marks, bed, shadow, projection, aura,
  HUD, and gauges remain present and correctly ordered.
- The companion remains a calm round habitat, not a generic 3D demo.
- Existing asleep/calm and activity behavior remains legible.
- Any default flip receives explicit side-by-side native visual approval.

### Performance

- The approved benchmark machine passes the 360x360 and 720x720 budgets.
- Occluded/minimized rendering stops.
- No steady-state atlas misses or static-resource uploads occur after warmup.
- Poll/provider work does not run on or block the render thread.
- A five-minute visible run has bounded memory and no device recovery.

### Reliability and privacy

- No Rust panic crosses Objective-C callbacks or renderer error callbacks.
- Failed templates and resources preserve the last good generation.
- Surface/device errors follow bounded recovery and explicit fallback rules.
- Review artifacts and GPU labels pass privacy scans.
- Bounded capture exits automatically even when the window is not frontmost.
- Required font coverage and license/attribution are release artifacts; missing
  glyphs do not silently resolve through a host font.
- AppKit accessibility/focus semantics survive retained rendering and fallback.

### Review and tooling

- Preview Lab manifests expose retained template, frame, resource, readback, and
  metrics artifacts as first-class types.
- Semantic artifacts are deterministic for fixed fixtures.
- Native Metal captures cover the required matrix.
- Existing TUI, Classic parity, round, Smooth, Pixel, packaging, and release tests
  remain green until their paths are explicitly retired.
- Phase proof records contain raw benchmark samples, captures, fault logs, privacy
  scan results, build/package measurements, and the explicit go/no-go decision.

## Risks And Mitigations

### Risk: renderer rewrite drifts from Glorp identity

Mitigation: contract-first migration, retained-vs-Smooth artifacts, native
side-by-side review, and no default flip before explicit acceptance.

### Risk: `wgpu` adds complexity and binary/compile cost

Mitigation: treat `wgpu` as a candidate until the four-way Phase 0 comparison,
constrain features/backends for macOS, enforce hard package/build rollback limits,
and keep renderer modules narrow rather than introducing an engine framework.

### Risk: GPU output is difficult to test deterministically

Mitigation: exact semantic/template/frame/resource sidecars plus bounded
hardware-specific readback tests and tolerance-based visual comparisons.

### Risk: font change alters Glorp's silhouette

Mitigation: resolve license, attribution, fallback, Unicode coverage, and metrics
before backend integration; prototype non-BMP/multi-scalar lookup; pre-render the
full species/stage matrix; and treat glyph metrics as a product contract.

### Risk: GPU surface erases native accessibility and input behavior

Mitigation: keep responder, focus, pointer routing, and accessibility objects in
AppKit; project sanitized retained semantic targets into a bounded native overlay;
and make the Phase 0 surface prototype pass a native accessibility/input audit.

### Risk: primary-machine percentages overfit one Mac

Mitigation: preserve the pinned absolute gate for reproducibility, record raw
baseline/candidate evidence, add relative no-regression checks on other supported
Apple Silicon systems, and report backing scale and energy rather than CPU alone.

### Risk: scene graph becomes a premature general engine

Mitigation: shallow hierarchy, fixed primitive/material families, no ECS, no
plugin shaders, and new capabilities only when an approved feature needs them.

### Risk: transparent 2.5D layers produce sorting artifacts

Mitigation: explicit draw phases, stable order, depth testing only where valid,
and no promise of general order-independent transparency.

### Risk: device recovery creates blank or crashing windows

Mitigation: last-good generation, bounded surface/device recovery, explicit staged
fallback, and native error-injection tests.

### Risk: the migration maintains two renderers indefinitely

Mitigation: each phase has retirement gates; the software/reference path is narrow,
and Smooth fallback has an explicit release-bound removal decision.

## Open Decisions Before Implementation Planning

1. **Font asset:** which redistributable monospace font satisfies the locked
   license, attribution, coverage, metrics, non-BMP, visual, and size policy.
   Transitional native atlas rasterization is allowed only during migration.
2. **Visual reference path:** minimal software rasterizer versus `wgpu`
   offscreen/readback. Recommendation: keep exact correctness in semantic/resource
   artifacts and implement only enough pixel reference behavior for visual review;
   avoid a second full shipping renderer.
3. **Window surface integration:** exact AppKit view/layer and raw-window-handle
   bridge. This requires a focused macOS prototype before the Phase 3 plan.
4. **Dependency features:** exact `wgpu` version, enabled backends/features, shader
   source format, and whether it passes the provisional hard build/size limits or a
   separately reviewed replacement budget.
5. **GPU timing availability:** whether timestamp queries are reliable enough on
   the supported Metal matrix; CPU metrics remain mandatory regardless.
6. **Permanent failure fallback after Smooth retirement:** last-good image, narrow
   CPU fallback, or static fallback scene. This must be decided before Phase 5.
7. **Initial perspective support:** compile the perspective camera in Phase 3 or
   defer it until the first renderer-native 2.5D feature. Recommendation: define
   and test the contract early, do not expose it visually until separately approved.
8. **Interaction scope:** include hit-test IDs in the initial template, but defer
   pointer behaviors until a separate interaction design.
9. **Backend selection result:** which Phase 0 candidate wins the common benchmark.
   `wgpu` is the leading hypothesis, not an implementation prerequisite.
10. **Darwin x86_64 native qualification:** dedicated Intel hardware, translated
    launch evidence where meaningful, or a documented release exception with
    cross-compiled package inspection. Compilation alone does not prove Metal
    surface behavior.

## Recommended First Implementation Plan

Do not begin with shaders or window-surface integration.

The first plan should be **Retained Contracts, Candidate Bake-Off, And Benchmark
Freeze**:

1. add the benchmark protocol, raw-result schema, environment/binary identity, and
   build/package measurement tooling;
2. introduce generation-checked retained IDs, template, frame, node, primitive,
   dynamic-content, camera, resource, validation, checksum, and Preview Lab DTOs;
3. derive retained contracts from current Smooth fixtures;
4. add a compatibility adapter that can reproduce current Smooth plan values;
5. prove template reuse across animation samples;
6. prototype and measure targeted AppKit batching, retained AppKit/CALayer,
   persistent software bitmap/atlas, and `wgpu`/Metal against the same fixtures;
7. prove the native surface/capture/fault/accessibility/input bridge;
8. resolve font, feature, backend, dependency, release-size, and rollback decisions;
9. stop before changing default native drawing.

That plan creates the architectural leverage and safety net needed for every later
renderer and 2.5D slice.
