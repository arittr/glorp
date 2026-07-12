# Glorp Companion 2.5D Scene Runtime — design

**Date:** 2026-07-11
**Status:** Conversation-approved; awaiting written-spec review
**Scope:** The native macOS round companion and its wgpu renderer only

## Decision

Replace the retained renderer's per-frame `SmoothCompanionScenePlan` translation
with a direct, companion-specific 2.5D scene runtime. The runtime owns stable scene
nodes, real world depth, camera state, typed primitives and materials, bounded
dynamic content, GPU batch compilation, and renderer-native lighting.

The new runtime is forward-looking. It does not preserve Classic flattening,
terminal cell layout contracts, Smooth scene types, or a permanent dual-renderer
abstraction. Existing Smooth rendering remains unchanged as temporary visual
evidence and host-level fallback during migration, but it does not shape the new
scene contracts. After the new retained path is accepted, the current
Smooth-plan-to-GPU translator is deleted.

The first renderer-native proof feature is a lit shallow-card treasure-chest prop.
Its stable world node and attachment point establish the path for later
depth-aware bubbles, particles, and prop interactions.

## Calibration

The retained cutover delivered the difficult platform and operational foundation:

- an AppKit-hosted wgpu/Metal surface;
- transactional activation and explicit fallback;
- bounded glyph atlas compilation and upload;
- persistent instance-buffer allocation;
- canonical GPU readback and paired native captures;
- privacy-safe evidence and fault reporting;
- accepted visual parity at the shipping companion size.

The current retained frame is nevertheless still a fully resolved immediate scene:

```text
WatchViewModel
  -> rebuild TUI-derived layered scene
  -> build SmoothCompanionScenePlan
  -> sort layers
  -> expand every cell and shape into screen-space GpuPrimitive values
  -> upload the complete primitive array
  -> issue one draw per primitive at clip-space Z = 0
```

GPU objects are retained, but scene identity, static content, transforms, depth,
and batches are not. Ordinary motion reconstructs the room and uploads all
primitive data even when only a pet transform or parallax value changed.

## Product Goal

Make the companion materially easier to evolve into a more dynamic and expressive
Glorp habitat. New depth planes, props, bubbles, particles, localized lights,
camera treatments, and selective shallow geometry should be additions to stable
scene data—not new companion-specific painter branches.

The engine remains Glorp-shaped:

- one small round habitat;
- shallow hierarchy;
- bounded content and effects;
- generated glyph/sprite identity;
- typed materials and primitives;
- deterministic fixtures and captures;
- no general game-engine surface.

## Goals

1. Make the companion's retained scene independent of TUI, Ratatui, Classic, and
   Smooth renderer types.
2. Separate scene topology, semantic content, and per-frame presentation state.
3. Give scene elements stable identities, parent/child transforms, real world Z,
   and reusable attachment points.
4. Move camera and node transforms onto the GPU.
5. Compile immutable geometry and resources once per scene generation.
6. Update only bounded content slots and frame-state buffers during normal motion.
7. Batch compatible primitives rather than issuing one draw per item.
8. Support explicit opaque, translucent, effect, foreground, and screen-space
   phases with correct depth behavior.
9. Use conventional linear-light rendering on an sRGB output path so future
   lighting is technically sound.
10. Prove the architecture with a lit shallow-card treasure chest and leave a
    direct attachment path for later bubbles.
11. Preserve existing native host, capture, privacy, recovery, and packaging work.
12. Make scene lifetime and GPU work observable through production-derived tests
    and review artifacts.

## Non-Goals

- No changes to terminal watch, TUI, menubar, or Pixel rendering.
- No general ECS, physics engine, scene editor, scripting runtime, or plugin API.
- No general model loader or arbitrary user-authored shaders.
- No physically based material system, shadow maps, bloom, depth of field, or
  post-processing stack.
- No skeletal animation or general animation graph.
- No requirement for pixel equality with Smooth.
- No new Smooth adapter, Classic flattening path, or permanent old/new scene
  compatibility layer.
- No retirement of existing host-level fallback in this project. Removing Smooth
  entirely remains a separate operational decision after the new path is proven.
- No dynamic bubble implementation in this project. The scene and attachment
  contracts must make it the natural next feature.

## Architecture

### Data flow

```text
WatchViewModel + RoundSceneModel
                |
                v
CompanionSceneInput projection
  companion semantics and generated art, no renderer objects
                |
                v
CompanionSceneBuilder
                |
                +---------------------------+
                |                           |
         topology/layout change       semantic content change
                |                           |
                v                           v
CompanionSceneTemplate              CompanionSceneContent
  stable hierarchy                   bounded glyph/sprite slots
  local geometry                     prop and tank-life state
  world-space bases                  particle instance groups
  materials/resources                declared dirty ranges
  draw phases/capacities                    |
                |                           |
                +-------------+-------------+
                              |
                       presentation frame
                              |
                              v
                    CompanionFrameState
                      camera and globals
                      node transforms
                      opacity/visibility
                      gauges and lights
                              |
                              v
                    WgpuSceneCompiler/Renderer
                      immutable GPU batches
                      bounded dynamic buffers
                      small frame-state writes
                              |
                              v
                       RetainedHost surface
```

`CompanionSceneInput` is the only boundary allowed to understand current product
state shape. The builder receives companion semantics and generated art rather
than TUI render output. The new template, content, and frame contracts contain no
`ratatui`, `DrawCell`, `SmoothCompanionLayer`, AppKit, Metal, wgpu, or raw window
types.

### Proposed modules

```text
src/presentation/companion_scene/
  mod.rs
  input.rs
  ids.rs
  geometry.rs
  transform.rs
  node.rs
  primitive.rs
  material.rs
  light.rs
  camera.rs
  template.rs
  content.rs
  frame.rs
  builder.rs
  validate.rs
  checksum.rs

src/companion/retained/
  host.rs                 # CAMetalLayer and surface lifecycle
  compiler.rs             # scene generation -> GPU generation
  resources.rs            # atlas/texture/material resources
  buffers.rs              # static, content, and frame buffers
  batches.rs              # phase and pipeline batch compilation
  pipelines.rs            # typed pipeline families
  render.rs               # encode phases and batches
  capture.rs              # existing canonical GPU readback
  presentation.rs         # existing progress/disposition/fallback boundary
  parity.rs               # retained color/math helpers, narrowed as migration ends
```

Exact file splits may be adjusted to keep modules focused. Platform-neutral scene
contracts must remain outside the macOS-only retained host.

## Core Contracts

### Stable IDs and generations

```rust
pub struct SceneGeneration(pub u64);
pub struct NodeId(pub u32);
pub struct PrimitiveId(pub u32);
pub struct ResourceId(pub u32);
pub struct MaterialId(pub u16);
pub struct AttachmentId(pub u32);
pub struct InstanceGroupId(pub u16);
```

IDs are deterministic within a generation and derived from semantic identity,
such as `pet.body`, `world.prop.token_treasure_chest_2m`, or
`world.prop.token_treasure_chest_2m.bubble_origin`. Allocation order alone is not
an identity contract.

Every content and frame update names its target generation. A stale update is a
recoverable error and cannot mutate the active scene.

### `CompanionSceneTemplate`

```rust
pub struct CompanionSceneTemplate {
    pub schema_version: u16,
    pub generation: SceneGeneration,
    pub viewport: SceneViewport,
    pub camera: CameraTemplate,
    pub nodes: Vec<SceneNode>,
    pub primitives: Vec<PrimitiveTemplate>,
    pub materials: Vec<MaterialTemplate>,
    pub lights: Vec<LightTemplate>,
    pub resources: ResourceManifest,
    pub draw_phases: Vec<DrawPhaseTemplate>,
    pub instance_groups: Vec<InstanceGroupTemplate>,
    pub attachments: Vec<AttachmentTemplate>,
    pub privacy: CompanionPrivacyClaims,
    pub checksum: SceneTemplateChecksum,
}
```

The template owns data that does not change on ordinary animation frames:

- node hierarchy and stable IDs;
- local bounds and base transforms;
- primitive geometry and resource references;
- material family and immutable parameters;
- draw phase, depth policy, and stable transparency order;
- clip/mask definitions;
- attachment points and optional hit bounds;
- bounded dynamic group capacities;
- resource requirements and privacy claims.

Template rebuild triggers are explicit:

- viewport or backing-scale policy change;
- pet identity, stage, generated-art topology, or palette-role topology change;
- room/biome composition change;
- earned-prop or tank-life cast topology change;
- helper/lifecycle topology change that adds or removes nodes;
- material, shader, atlas, or renderer schema change.

Time passing, pet movement, bob, parallax, light pulses, gauges, or opacity changes
do not rebuild the template.

### `CompanionSceneContent`

```rust
pub struct CompanionSceneContent {
    pub generation: SceneGeneration,
    pub semantic_tick: u64,
    pub node_slots: Vec<NodeContentDelta>,
    pub instance_groups: Vec<InstanceGroupDelta>,
    pub dirty: ContentDirtySet,
}
```

Content updates cover bounded semantic substitutions:

- pet blink, expression, twinkle, and corruption glyphs;
- current generated-art cell or sprite slots;
- ambient, mote, activity, and bubble instances;
- bounded prop and tank-life state changes that do not alter topology;
- visibility or glyph substitutions declared by the template.

The template declares capacity and compatible resources. Exceeding capacity
requests a new template generation; the frame encoder never grows a buffer or
discovers a resource.

### `CompanionFrameState`

```rust
pub struct CompanionFrameState {
    pub generation: SceneGeneration,
    pub frame_index: u64,
    pub elapsed_ms: u64,
    pub camera: CameraFrameState,
    pub nodes: Vec<NodeFrameState>,
    pub lights: Vec<LightFrameState>,
    pub chrome: ChromeFrameState,
    pub globals: FrameGlobals,
    pub dirty: FrameDirtySet,
}

pub struct NodeFrameState {
    pub node: NodeId,
    pub transform: Transform3,
    pub opacity: f32,
    pub visible: bool,
    pub material_params: MaterialFrameParams,
}
```

Frame state covers pet X/Y/Z, bob, facing, parallax group transforms, shadow and
projection parameters, aura/feed/helper pulses, camera motion, light parameters,
gauge fractions, dim amount, and per-node visibility.

An ordinary motion frame updates only dirty frame ranges. Unchanged static
geometry, content slots, resource tables, and batch order are not cloned, sorted,
validated, or uploaded.

## Scene Model

### Coordinate system

- X increases right.
- Y increases down at the presentation boundary.
- Z increases toward the viewer.
- The pet's neutral plane is Z = 0.
- The builder maps existing normalized pet depth `[-1, 1]` into a documented,
  shallow world-Z interval.
- The renderer owns Y-down world conversion to GPU clip coordinates.

The initial camera is orthographic. Existing visual depth cues may be reproduced
through a bounded orthographic Z response while world Z becomes authoritative.
Perspective is a future camera mode and requires a separate visual design.

### Transforms and hierarchy

```rust
pub struct Transform3 {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
}
```

The first hierarchy is deliberately shallow:

```text
scene.root
|- world.far
|- world.behind
|- pet
|- world.foreground
`- chrome.screen
```

Leaf nodes represent meaningful independently moving or lightable elements:
individual props, tank inhabitants, grouped ambient emitters, the pet body,
projection, shadow, aura, and status elements. Cells that always move, sort, and
light together remain one node. Per-element depth means each meaningful element
can own Z; it does not require an object per glyph cell.

Attachments are named local transforms owned by a node. The treasure chest
declares a `bubble_origin` attachment. Future bubble emitters resolve that
attachment through the node's current world transform and inherit prop motion and
depth without bespoke coordinate code.

## Primitives, Materials, and Lights

### Primitive families

The first renderer supports:

- atlas-backed glyph quads;
- atlas-backed sprite quads;
- analytic rectangle, rounded rectangle, ellipse, ring, arc, and gradient shapes;
- shallow cards generated from a front quad and optional side faces;
- bounded instance groups for particles and repeated decorations.

General mesh loading is deferred. Static complex geometry may later compile into
a mesh resource without changing node, material, transform, or draw-phase
contracts.

### Material families

```text
UnlitGlyphSprite
UnlitAnalyticShape
LitShallowCard
MultiplyShadow
AdditiveGlow
ScreenChrome
```

Materials are typed. Each family has a bounded parameter structure and a known
pipeline. Arbitrary shader graphs are not supported.

`LitShallowCard` starts with linear-space base color, flat generated normals,
ambient contribution, diffuse key light, and a bounded rim term. It is not PBR;
there are no metallic workflows, image-based lighting, shadow maps, or runtime
shader selection.

### Lights

The first scene supports:

- one global ambient term;
- one directional or bounded local key light;
- one optional rim contribution carried by the lit material.

Light counts and parameter buffers are fixed by the template. Later activity-driven
lighting changes frame parameters, not scene structure or shader topology.

## Rendering Model

### Color convention

The new scene uses conventional linear-light shading and blending with sRGB-authored
colors and sRGB output conversion. It does not inherit the current retained
renderer's gamma-space Smooth-parity convention.

The first implementation should use an sRGB-capable surface/target path where
supported and verify the exact wgpu/Metal conversion behavior in capture tests.
Lighting, interpolation, and blend equations operate in linear space. Screen-space
chrome is unlit but follows the same defined input/output conversion.

This will require visual retuning. That is accepted as forward renderer work, not
a compatibility defect. Any offscreen linear or HDR intermediate is deferred until
a concrete effect requires it; the first lit card does not justify a permanent
extra pass by itself.

### Draw phases

1. Tank background and fixed far geometry.
2. Opaque and alpha-tested world geometry with depth test and depth writes.
3. Translucent world geometry in stable back-to-front order, depth-tested without
   depth writes.
4. Pet, projection, shadow, and world effects.
5. Foreground props, tank life, and particles.
6. Unlit screen-space HUD, gauges, status, trouble, and dim composition.
7. Aperture mask/final composite as required by the chosen surface path.

Opaque geometry may use a depth buffer. Translucent content never relies on a
global depth sort or order-independent transparency. Phase, depth group, and
stable node order remain explicit.

### GPU batches and buffers

At generation activation, the compiler:

1. validates the complete template and resource manifest;
2. resolves atlas, texture, and material resources;
3. builds immutable primitive and index/instance buffers;
4. groups compatible draws by phase, pipeline, material, resource page, blend,
   and depth policy;
5. allocates bounded content and frame buffers;
6. activates the generation only after compilation succeeds.

Static data is uploaded once. Content buffers receive declared dirty-range writes.
Frame buffers carry camera, global, node-transform, light, and chrome data. Viewport
and aperture data use globals rather than being duplicated in every primitive.

The render encoder issues one draw per compiled compatible batch, not one draw per
primitive. Ordinary frames create no GPU objects and do not change batch order.

## First Renderer-Native Feature: Lit Treasure Chest

The existing treasure-chest habitat prop becomes a generated shallow card:

- one stable prop node in `world.behind` or `world.foreground`, selected from its
  authored placement relative to the pet;
- front and shallow side geometry with flat normals;
- `LitShallowCard` material using the existing prop palette as base color;
- ambient plus bounded key/rim lighting;
- explicit world Z and depth policy;
- a named `bubble_origin` attachment above the lid;
- deterministic Preview Lab far, neutral, near, unlit-control, and lit frames.

The first feature does not add production bubbles. It proves that later bubbles
can attach to the prop, inherit its transform and depth, occupy a bounded instance
group, and render through the existing translucent phase.

The pet, HUD, and gauges remain unlit during this proof so review can isolate the
new material and depth behavior.

## Frame Pacing and Invalidation

The runtime distinguishes three clocks:

- topology/layout changes rebuild a scene generation;
- semantic animation ticks update bounded content;
- visible presentation frames update transforms and globals.

The current 15 FPS visible companion cadence may remain initially. Frame pacing
can later become adaptive without changing scene contracts. Hidden, minimized, or
occluded windows continue to suspend presentation work. On reveal, one current
frame is derived; hidden frames are not replayed.

Scene construction, resource compilation, and full validation cannot occur inside
the render-pass encoder. Expensive generation work may be prepared before
activation while the last good generation remains visible.

## Host and Ownership Boundaries

The existing retained host continues to own:

- CAMetalLayer and wgpu surface lifecycle;
- device, queue, and surface configuration;
- resize and backing-scale integration;
- frame progress and terminal dispositions;
- capture/readback;
- error mailbox, fallback, and AppKit restoration;
- accessibility/input host integration.

The host does not derive product semantics or walk scene primitives. It accepts an
active compiled generation plus content/frame updates and asks the renderer to
encode them.

The scene builder and contracts do not know whether Smooth fallback exists. Smooth
remains an external host policy during migration, not a scene backend the runtime
must support.

## Error Handling

### Build and validation failures

The builder rejects:

- duplicate or missing stable IDs;
- invalid parent relationships or hierarchy cycles;
- non-finite transforms, bounds, camera, light, or material values;
- missing resources, materials, attachments, or draw phases;
- unsupported primitive/material/depth combinations;
- dynamic capacities that exceed configured limits;
- privacy claims inconsistent with the companion surface.

A failed template never replaces the last good generation.

### Update failures

Stale-generation content or frame updates are rejected with sanitized diagnostics.
Out-of-range slot writes are rejected. Exceeding a declared capacity requests a
new generation; it never reallocates within frame encoding.

Invalid optional effects may be disabled only if the template explicitly declares
that degraded mode. Missing required pet, room, prop, depth, or chrome content is a
generation failure, not a silent omission.

### GPU and surface failures

Existing retained host presentation and fallback rules remain authoritative.
Device or surface failure tears down or restores the host through the current
bounded recovery path. The new runtime adds no Objective-C unwinding and exposes
no raw user data in GPU labels or diagnostics.

## Observability

Production metrics and artifacts must expose:

- active scene and resource generation;
- template build and activation counts;
- template node, primitive, material, light, attachment, and instance capacities;
- static upload count and bytes;
- content write count, ranges, and bytes;
- frame write count, ranges, and bytes;
- batch and draw counts by phase;
- depth attachment creation and reuse;
- atlas/resource misses;
- scene build, compile, encode, and submit CPU durations;
- frame dispositions and fallback category.

Metrics are sanitized and bounded. Stable semantic aliases in artifacts must not
include project names, prompts, file paths, or unprojected pet seeds.

## Preview and Evidence Contract

Preview Lab and native review should add first-class artifacts:

```text
frames/<id>.companion-template.json
frames/<id>.companion-content.json
frames/<id>.companion-frame.json
frames/<id>.companion-batches.json
frames/<id>.companion-resources.json
frames/<id>.companion-metrics.json
frames/<id>.companion-readback.png
```

Required deterministic fixtures include:

- normal, active, asleep, helper-trouble, and dim states;
- far, neutral, and near pet/world depth;
- representative props and tank life in every depth group;
- opaque and translucent overlap ordering;
- treasure-chest unlit control and lit result;
- chest attachment world transform;
- resize/backing-scale changes;
- stale-generation and resource/build failure fixtures.

The existing native capture and privacy scanner are reused. Smooth captures may be
consulted during migration but are not part of the new scene artifact schema or an
acceptance requirement after visual approval.

## Testing Strategy

### Pure contract tests

- deterministic stable IDs and checksums;
- hierarchy and transform composition;
- world-Z and camera projection;
- generation mismatch rejection;
- template rebuild classification;
- content and frame dirty-range classification;
- capacity and resource validation;
- attachment world-transform resolution;
- privacy-safe serialization.

### Production-derived lifetime tests

Build a real companion fixture through `CompanionSceneBuilder`, warm the actual
compiler, then run at least 300 ambient presentation frames. Assert:

- zero template rebuilds;
- zero static uploads;
- zero atlas/resource rebuilds or misses;
- zero GPU object creation;
- unchanged static batch checksums;
- only expected content/frame ranges written;
- bounded, stable batch and draw counts.

Synthetic primitive-only resource tests may remain, but they do not satisfy this
production lifetime gate.

### Renderer tests

- camera and node transforms reach nonzero clip/world depth;
- depth-tested opaque occlusion;
- stable translucent ordering with depth writes disabled;
- correct phase and batch compilation;
- sRGB input/output and linear-light blend samples;
- glyph/sprite/shape/card encoding;
- lit-card normal, ambient, key, and rim response;
- aperture, clip, resize, and backing-scale behavior;
- capture ordering and readback normalization.

### Native review and stability

- native captures at required sizes and states;
- lit chest review at far, neutral, and near depth;
- five-minute visible stability run with bounded resources and no fallback;
- hidden/minimized/occluded suspension and clean reveal;
- injected initialization, surface, device, stale-generation, and capture faults;
- keyboard, fullscreen, input, accessibility, privacy, and packaging regression
  checks.

### Profiling

Before and after cutover, record production companion measurements for:

- CPU scene preparation and render encoding;
- upload bytes per ordinary frame;
- batch and draw counts;
- visible frame pacing;
- hidden-window work;
- memory/resource stability.

The measurements are evidence and tuning input. The implementation plan should set
absolute regression gates from the measured current branch rather than inventing
untested thresholds in this design.

## Migration Program

### Stage 1: contracts and direct builder

- Add platform-neutral companion scene contracts, validation, checksums, and
  deterministic artifacts.
- Define `CompanionSceneInput` and build directly from companion domain state.
- Reuse pure domain algorithms where useful, but do not consume TUI draw output or
  produce Smooth scene types.
- Keep the live retained renderer unchanged during this stage.

Exit gate: a complete deterministic companion template/content/frame fixture
exists with no Ratatui, `DrawCell`, or Smooth dependencies.

### Stage 2: wgpu scene compiler

- Split the existing retained module into host, compiler, resources, buffers,
  batches, pipelines, and render ownership.
- Compile glyph, sprite, analytic shape, and card primitives.
- Add camera/frame globals, node transforms, content slots, and a reused depth
  attachment.
- Produce offscreen and native evidence through the existing capture path.

Exit gate: the full unlit companion renders from the new scene contracts with
bounded batches and no old-plan translation.

### Stage 3: live lifecycle cutover

- Replace per-frame scene reconstruction with topology, semantic-content, and
  frame-state invalidation.
- Drive the active compiled generation from `AppState` without walking primitives
  in the frame loop.
- Prove the production-derived 300-frame lifetime test and visible stability run.

Exit gate: ordinary motion updates only bounded frame/content buffers and the live
retained path contains no `SmoothCompanionScenePlan`.

### Stage 4: remove the transitional translator

- Delete the current `prepare_gpu_frame` Smooth-plan translator and obsolete
  retained-only parity helpers.
- Narrow `app.rs` to scene input/state orchestration and host presentation.
- Keep existing Smooth renderer code only for current host fallback policy.

Exit gate: there is one retained companion scene generation path.

### Stage 5: real depth and composition tuning

- Tune world-Z placement and orthographic camera response.
- Close opaque depth and transparent stable-order fixtures.
- Retune colors for the linear-light renderer.
- Receive native visual approval across the required matrix.

Exit gate: the existing companion identity is preserved while depth is genuine
scene data rather than CPU painter simulation.

### Stage 6: lit treasure-chest proof

- Add shallow-card geometry, material, light parameters, and attachment point.
- Add deterministic and native review fixtures.
- Verify the feature primarily adds scene resources and parameters, not a bespoke
  render path.

Exit gate: the lit chest is accepted and a later bubble emitter can attach through
the stable scene contract.

## Acceptance Criteria

### Structural

- The retained live path does not construct or consume `SmoothCompanionScenePlan`,
  `SmoothCompanionLayer`, `SceneDrawList`, `DrawCell`, or Ratatui geometry.
- Platform-neutral companion scene contracts contain no AppKit, Metal, wgpu, or
  raw window types.
- Static topology/resources, semantic content, and per-frame state have separate
  generation-checked lifetimes.
- Meaningful world elements have stable IDs, XYZ transforms, and depth policy.
- The scene exposes stable attachments suitable for later bubbles and effects.
- Ordinary frames do not rebuild or re-sort static scene content.
- The renderer uses compiled compatible batches rather than one draw per
  primitive.
- World-space rendering uses camera transforms and a reused depth attachment.
- Screen-space chrome remains isolated from world lighting and depth.

### Forward capability

- Props and future particles can occupy real world depth.
- Typed material/light parameters can change per frame without scene rebuild.
- A new shallow card or bounded instance group can be added without modifying the
  host lifecycle or inventing a custom painter.
- The treasure chest renders as a lit shallow card and exposes a verified
  `bubble_origin` world transform.
- The design leaves room for future sprites, generated meshes, perspective camera,
  and additional bounded effects without requiring those features now.

### Performance and reliability

- A production-derived 300-frame ambient run has zero template builds, static
  uploads, resource misses, or GPU object creation after warmup.
- Static batch checksums remain stable across ordinary motion.
- Visible and hidden behavior is measured against the pre-cutover branch and does
  not introduce an unexplained material regression.
- A five-minute visible run has bounded resource counts and no fallback.
- Failed generations and stale updates preserve the last good scene.
- Existing surface/device recovery, capture, privacy, input, accessibility, and
  package checks remain green.

### Evidence

- Template, content, frame, batch, resource, metric, and readback artifacts are
  deterministic and first-class Preview Lab contract entries.
- Native captures cover required lifecycle, depth, overlap, resize, and lit-prop
  states.
- Production metrics expose template churn, upload bytes, batch/draw count, and
  scene/encode timing.
- The final review record explicitly confirms deletion of the transitional
  Smooth-plan translator.

## Risks and Mitigations

### Risk: accidental generic-engine expansion

Mitigation: keep the hierarchy shallow, primitive/material families typed, light
counts bounded, and new capabilities justified by a concrete companion feature.

### Risk: direct scene projection drifts from product semantics

Mitigation: build `CompanionSceneInput` from the same privacy-projected companion
state, preserve deterministic semantic fixtures, and require native review before
deleting the translator. Do not maintain two independently evolving retained scene
paths after cutover.

### Risk: transparent content behaves poorly with real depth

Mitigation: depth-write only opaque/alpha-tested geometry. Keep translucent content
in explicit phases with stable back-to-front order and depth testing without
writes.

### Risk: linear-light migration changes Glorp's look

Mitigation: treat retuning as a deliberate renderer evolution, lock authored color
and blend samples, and review representative native states. Do not reintroduce
gamma-space lighting for parity.

### Risk: scene lifetime split is nominal but the CPU still rebuilds everything

Mitigation: production-derived churn counters and the 300-frame lifetime test are
hard acceptance gates. Synthetic buffer-reuse tests alone are insufficient.

### Risk: one large renderer module becomes a new maintenance bottleneck

Mitigation: separate host, compiler, resources, buffers, batches, pipelines, and
render encoding behind small contracts while preserving the existing host and
capture behavior.

## Implementation Planning Boundary

The implementation plan should cover all six migration stages in dependency order,
with explicit tests and review gates for each stage. It should not mix in production
bubbles, general mesh loading, perspective camera behavior, PBR materials, or
retirement of host-level Smooth fallback.

The plan must identify exact source files, failing-first tests, Preview Lab/native
evidence commands, resource and performance counters, translator-deletion checks,
and commit boundaries.
