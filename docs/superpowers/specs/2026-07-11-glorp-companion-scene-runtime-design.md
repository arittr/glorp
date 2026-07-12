# Glorp Companion 2.5D Scene Runtime — design

**Date:** 2026-07-11
**Status:** Adversarial architecture/rendering review passed; ready for
implementation planning
**Scope:** The native macOS round companion and its wgpu renderer only

## Decision

Replace the retained renderer's per-frame `SmoothCompanionScenePlan` translation
with a direct, companion-specific 2.5D scene runtime. One immutable
`CompanionSceneSnapshot` is the authoritative semantic input for each update. A
stateful `CompanionSceneReconciler` compares snapshots, classifies invalidation,
owns revisions and generation transitions, and produces bounded topology,
content, and frame work for the renderer.

The runtime owns stable scene nodes, genuine world depth, camera state, a small
closed set of primitives and materials, fixed-capacity dynamic content, compiled
GPU batches, and renderer-native lighting. Generation activation is atomic:
template, resources, initial content, and initial frame state become active
together or the last good generation remains visible.

The new runtime is forward-looking. It does not preserve Classic flattening,
terminal-cell layout contracts, Smooth scene types, or a permanent dual-renderer
abstraction. Smooth remains temporary rollout safety and visual evidence while the
new path is shadowed and canaried. That scaffolding does not shape the scene
contracts, and the Smooth-plan-to-GPU translator is deleted only after the canary
hold passes.

The first renderer-native feature after cutover is a lit shallow-card
treasure-chest prop. It proves real depth, local transforms, lighting, generated
geometry, and attachment semantics before production bubbles are added.

## Calibration

The retained cutover already delivered the difficult platform and operational
foundation:

- an AppKit-hosted wgpu/Metal surface;
- an Objective-C unwind guard, transactional layer activation, and explicit
  fallback;
- hidden-window suspension and bounded presentation dispositions;
- asynchronous GPU error delivery;
- compile-before-replace resources and a full glyph repertoire;
- persistent instance rings and GPU-native capture/readback;
- privacy-safe evidence and fault reporting;
- accepted visual parity at the shipping companion size.

Those properties are constraints, not optional cleanup. The scene-runtime work
must preserve them.

The current retained frame is nevertheless a fully resolved immediate scene:

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

The current capture path is also Smooth-plan-specific, and the immediate fallback
path expects a Smooth frame in the same host tick. Both must be migrated
deliberately; replacing the live draw call alone would break accepted evidence and
failure behavior.

## Product Goal

Make the companion materially easier to evolve into a dynamic Glorp habitat. New
depth planes, props, bubbles, particles, localized lights, camera treatments, and
selective shallow geometry should be additions to stable scene data rather than
new companion-specific painter branches.

The engine remains Glorp-shaped:

- one small round habitat;
- a shallow hierarchy;
- bounded content and effects;
- generated glyph/sprite identity;
- typed materials and primitives;
- deterministic fixtures and captures;
- no general game-engine API.

## Goals

1. Make the companion's retained scene independent of TUI, Ratatui, Classic, and
   Smooth renderer types.
2. Establish one renderer-neutral semantic snapshot and one reconciler as the
   authority for invalidation, revisions, and generations.
3. Separate topology/resources, semantic content, and per-frame presentation
   state without cloning the complete scene on ordinary ticks.
4. Give meaningful elements stable identities, parent/child transforms, real
   world Z, explicit depth cues, and reusable attachments.
5. Compile immutable geometry, resources, opaque/chrome batches, fixed blended
   draw records, and capacities once per resource generation.
6. Update only bounded CPU mirrors and GPU byte ranges during ordinary content or
   motion changes.
7. Batch compatible opaque content globally and transparent content without
   violating back-to-front order.
8. Use a conventional linear-light, premultiplied-alpha render contract on an
   explicitly selected SDR sRGB output path.
9. Keep generation work off the AppKit presentation hot path and make queueing,
   cancellation, activation, and fallback deterministic.
10. Prove the architecture with a lit shallow-card treasure chest and a precise
    attachment path for later depth-aware bubbles.
11. Preserve existing host, capture, privacy, recovery, accessibility, input, and
    packaging behavior.
12. Freeze a numeric baseline and make scene lifetime, GPU work, latency, memory,
    and fault behavior observable.

## Non-Goals

- No changes to terminal watch, TUI, menubar, or Pixel rendering.
- No general ECS, physics engine, scene editor, scripting runtime, or plugin API.
- No public resource registry, arbitrary hit-testing framework, or generic mesh
  escape hatch.
- No arbitrary user-authored shaders, shader graphs, PBR material system, shadow
  maps, bloom, depth of field, or post-processing stack.
- No skeletal animation or general animation graph.
- No requirement for pixel equality with Smooth.
- No new Smooth adapter, Classic flattening path, or permanent old/new scene
  compatibility layer.
- No retirement of host-level Smooth fallback in this project. Removing Smooth
  entirely remains a separate operational decision.
- No production bubble implementation in this project. The contracts must make
  bubbles the natural next feature.

## Architectural Spine

### One authoritative snapshot

```text
privacy-projected companion domain state + monotonic time + logical layout
                               |
                               v
                  CompanionSceneSnapshot::project
                    immutable, renderer-neutral
                               |
                               v
                  CompanionSceneReconciler
              owns IDs, revisions, generations, invalidation
                     /          |           \
                    v           v            v
              topology work  content work  frame work
                    |           |            |
                    +-----------+------------+
                                v
                     CompanionSceneRuntime
                active generation + pending generation
                                |
                                v
                      retained wgpu renderer
```

`CompanionSceneSnapshot` is the only authoritative input. The runtime must not
independently consume both `WatchViewModel` and `RoundSceneModel`; doing so would
create two clocks and ambiguous semantic ownership. Projection may call shared
pure domain helpers, but it returns one immutable value for one logical update.

The snapshot contains semantic values, stable authored aliases, generated-art
tokens, and logical companion dimensions. It contains no `ratatui`, `DrawCell`,
`SmoothCompanionLayer`, AppKit, Metal, wgpu, raw window, or GPU-resource type.

### Renderer-neutral seam extraction

Current shared meaning must be extracted before the new builder is trusted:

- pet state, expression, stage, palette roles, and generated-art inventory;
- habitat room/biome selection and prop/tank-life inventory;
- activity, helper, lifecycle, gauge, and dim semantics;
- privacy projection and deterministic fixture inputs.

These become small pure domain modules consumed by the existing TUI path and by
`CompanionSceneSnapshot::project`. The new path may share semantic algorithms and
authored inventories, but never consumes TUI draw output or Smooth scene types.
This prevents silent semantic forks without creating a renderer compatibility
layer.

### Stateful reconciliation

The reconciler retains the last accepted snapshot and classifies each new one:

```rust
pub enum ReconcileResult {
    Unchanged,
    Frame(FrameDelta),
    Content(ContentDelta, Option<FrameDelta>),
    NewGeneration(GenerationRequest),
}
```

Only the reconciler may allocate semantic revisions or request a layout/resource
generation. Producers cannot invent revisions, dirty GPU ranges, or generation
numbers. Renderer-side dense indices and byte ranges are compiler details.

For a newly built generation, the runtime re-runs reconciliation against the most
recent snapshot before activation. A worker result derived from an older snapshot
is never allowed to rewind current content or frame state.

### Compact module ownership

The initial split should stay small enough to understand:

```text
src/presentation/companion_scene/
  mod.rs            # public semantic snapshot and runtime boundary
  input.rs          # domain projection and authored inventories
  scene.rs          # nodes, transforms, materials, capacities, attachments
  runtime.rs        # reconciler, revisions, invalidation, activation state
  contract.rs       # fixed render enums, neutral capture snapshot, evidence
  validate.rs       # pure full-generation and delta validation

src/companion/retained/
  host.rs           # CAMetalLayer, surface, fallback, UI-thread boundary
  compiler.rs       # generation work and dense compiler maps
  resources.rs      # atlases, textures, depth attachment, pipelines
  buffers.rs        # persistent CPU mirrors and GPU buffers
  render.rs         # phase order, batch compilation, encoding
  capture.rs        # GPU capture request, readback, renderer-specific evidence
  presentation.rs   # progress, dispositions, activation acknowledgement
```

Split further only when a concrete module becomes difficult to own. Platform-
neutral contracts remain outside the macOS retained host.

## Revisions, Epochs, and Activation

### Distinct counters

The runtime keeps unrelated invalidation domains separate:

```rust
pub struct DeviceEpoch(pub u64);
pub struct SurfaceEpoch(pub u64);
pub struct LayoutGeneration(pub u64);
pub struct ResourceGeneration(pub u64);
pub struct SemanticRevision(pub u64);
pub struct FrameRevision(pub u64);

pub struct GenerationKey {
    pub device: DeviceEpoch,
    pub surface: SurfaceEpoch,
    pub layout: LayoutGeneration,
    pub resources: ResourceGeneration,
}

pub struct AppliedRevisions {
    pub semantic: SemanticRevision,
    pub frame: FrameRevision,
}

pub struct SceneVersion {
    pub generation: GenerationKey,
    pub applied: AppliedRevisions,
}
```

- `DeviceEpoch` changes after device loss/recreation.
- `SurfaceEpoch` changes when the configured surface contract changes.
- `LayoutGeneration` changes when logical companion geometry or topology changes.
- `ResourceGeneration` identifies an atomically compiled template/resource set.
- `SemanticRevision` advances for accepted content meaning.
- `FrameRevision` advances for accepted presentation state.

The host alone advances device and surface epochs. The reconciler alone advances
layout, semantic, and frame values and requests a resource rebuild. The runtime
alone allocates the next `ResourceGeneration`; a compiler result never chooses its
own identity. `GenerationKey` is immutable for one compiled GPU unit.
`AppliedRevisions` changes as bounded ordinary deltas are accepted without
activating a generation. `SceneVersion` identifies the exact state used by a
presentation, capture, or metrics snapshot.

Backing-scale changes do not automatically imply scene-topology changes. Logical
layout is expressed in companion points. Surface extent, scale, format, sample
count, and device epoch independently govern attachments and surface resources.

Every asynchronous request/result carries its immutable `GenerationKey` and the
source `AppliedRevisions` from which it was derived. Comparisons are monotonic and
stale results are rejected; they are never applied speculatively. Before
activation, the reconciler reapplies newer compatible semantic/frame deltas to the
candidate mirrors and records the resulting applied revisions.

Invalidation is exhaustive:

| Change | Owner and effect |
|---|---|
| Device recreation/loss | host advances `DeviceEpoch`; every GPU generation is invalid |
| Surface format, color space, alpha mode, sample count, or physical extent | host advances `SurfaceEpoch`; recreate only surface-dependent targets, and rebind a candidate before activation |
| Logical dimensions, hierarchy, art lattice, cast topology, or capacity | reconciler advances `LayoutGeneration` and requests a `ResourceGeneration` |
| Atlas repertoire/scale, material schema, shader, or pipeline schema | reconciler requests a `ResourceGeneration`; layout may remain unchanged |
| Semantic slot value | reconciler advances `SemanticRevision` |
| Camera, transform, visibility, opacity, light, gauge, or dim value | reconciler advances `FrameRevision` |

A field-classification test covers every projected snapshot field. Backing scale
can change raster resources without changing logical layout.

### Generation activation state machine

```text
Active(Gn, Rn)
  -> Preparing(Gn+1, source = Rm, newest_request)
  -> Ready(Gn+1, applied = newest_compatible_R, resources + template + mirrors)
  -> Activating(previous = Gn, candidate = Gn+1)
  -> Active(Gn+1, newest_compatible_R)

Preparing/Ready/Activating
  -> FailedRetaining(Gn, sanitized_reason)
  -> Active(Gn, current_R)

No usable active generation or unrecoverable device/surface failure
  -> HostFallbackPending
  -> HostFallbackPainted
```

Activation swaps the compiled template, complete resources, persistent buffer
set, initial content mirror, initial frame mirror, and capture metadata as one
candidate unit. `Activating` retains the previous unit while it acquires, encodes,
submits, calls `SurfaceTexture::present`, and drains the immediately available GPU
error mailbox for the candidate's first frame. The exact commit milestone is
`SurfacePresentCalled` with an empty immediate mailbox; only then does the runtime
publish the candidate as `Active` and release the previous unit. No mixed-key
frame is encodable.

An `Outdated`, `Timeout`, or `Occluded` acquire leaves the candidate ready for a
later visible tick. A pre-submit validation/resource failure destroys the
candidate and retains the previous unit when its device/surface epochs remain
usable. `SurfaceLost`, surface validation failure, device loss, or OOM follows the
host fallback policy because neither candidate nor previous GPU resources are
trusted. A delayed asynchronous validation/device error discovered after the
commit invalidates the whole `DeviceEpoch` and enters fallback; it does not try to
restore the previous generation on the same device.

Only one pending generation request is retained. Newer requests coalesce and
cancel or supersede older worker work at safe checkpoints. A completed stale
result is dropped. The last good active generation remains renderable throughout
preparation.

## Scene Contracts

### Stable semantic identity, dense compiler identity

The public scene contract exposes stable semantic `NodeId` and `AttachmentId`
values. They are deterministic within a layout generation and derived from
authored meaning such as `pet.body`, `world.prop.treasure_chest`, and
`world.prop.treasure_chest.bubble_origin`.

Primitive, material, resource, and instance indices are private dense compiler
indices. Allocation order is not semantic identity and is not serialized as one.
The compiler owns maps from stable nodes and typed slots to dense buffer offsets.

### Fixed contract families

The first version has closed enums:

```rust
pub enum PrimitiveKind {
    AtlasQuad,
    AnalyticShape,
    ShallowCard,
    InstanceQuad,
}

pub enum MaterialKind {
    UnlitGlyphSprite,
    UnlitAnalytic,
    LitShallowCard,
    MultiplyShadow,
    AdditiveGlow,
    ScreenChrome,
}

pub enum WorldBlend {
    Opaque,
    AlphaCutout,
    PremultipliedAlpha,
    Multiply,
    Additive,
}

pub enum AttachmentMode {
    Follow,
    SnapshotWorldOnSpawn,
}
```

There is no public “custom phase,” “custom resource,” “custom shader,” or generic
mesh hook. New enum cases require a concrete companion feature, validation,
pipeline ownership, and evidence.

### Template, content, and frame lifetimes

The immutable generation template owns:

- stable hierarchy, base local transforms, pivots, and local bounds;
- typed primitive definitions and immutable geometry;
- typed material/resource references and draw policies;
- fixed art lattices and dynamic group capacities;
- attachment definitions;
- privacy claims and a generation checksum.

The persistent semantic-content mirror owns bounded substitutions:

- pet expression/blink/twinkle/corruption slots;
- generated-art atlas slots and palette-role values;
- fixed prop/tank-life state slots;
- particle, mote, activity, and future bubble instance groups.

The persistent frame mirror owns camera/globals, local node transforms, parent
opacity/visibility, gauges, dim amount, and lights. Visibility has one owner: the
frame mirror. Content cannot independently hide the same node.

Reconciler output names changed semantic slots or node fields, not caller-guessed
byte spans. The compiler applies those changes to fixed-layout POD CPU mirrors,
coalesces adjacent changed bytes, and emits bounded queue writes. Ordinary
updates do not clone the complete mirror, sort the complete scene, or validate the
complete template.

### Fixed capacities

Each pet species/stage has a fixed generated-art lattice and palette-role layout.
Smaller poses leave slots empty rather than changing topology. The generation
also fixes maxima for:

- room props by authored depth bucket;
- tank inhabitants and decorations;
- ambient motes and activity particles;
- each future bubble emitter;
- lit cards, lights, attachments, and screen-chrome elements.

The implementation plan must derive numeric limits from the current full Preview
Lab fixture inventory, add explicit headroom, and freeze the values in a versioned
contract. Overflow requests a new resource generation or rejects an invalid
fixture; no ordinary tick grows a `Vec`, atlas, CPU mirror, or GPU buffer.

## Coordinates, Camera, and Transforms

### Coordinate contract

- Snapshot/layout X/Y are logical companion points, independent of backing scale,
  with X right and Y down.
- Scene world space is right-handed: X increases right, Y increases up, and Z
  increases toward the camera. Projection converts snapshot Y-down to world Y-up
  exactly once.
- The neutral pet plane is world Z `0.0`.
- World Z remains shallow and is mapped into the camera's documented near/far
  interval.
- WebGPU clip-space depth is `[0, 1]`.

The first camera is orthographic. Let `near_z` be the greatest permitted authored
world Z and `far_z` the least. The locked depth mapping is
`clip_z = (near_z - world_z) / (near_z - far_z)`: nearest maps to `0.0`, farthest
maps to `1.0`, the depth clear is `1.0`, and comparison is `LessEqual`. Fixtures
lock the near, neutral, and far matrix outputs numerically.

World Z determines occlusion only; it must not
implicitly change scale, Y position, opacity, saturation, haze, or light. Those
artistic effects are explicit `DepthCue` parameters derived from authored depth:

```rust
pub struct DepthCue {
    pub scale: f32,
    pub y_offset_points_up: f32,
    pub opacity: f32,
    pub saturation: f32,
}
```

This separation prevents a future camera change from silently changing Glorp's
art direction. Perspective remains a separate future design.

The world depth attachment is `Depth24Plus`, cleared to `1.0`, with `LessEqual`
comparison. Every world pipeline has mandatory depth state. Opaque and cutout
draws write depth; translucent, multiply, and additive draws test depth without
writing. Screen chrome does neither. The depth attachment is recreated only when
surface extent, sample count, depth format, or device epoch changes—not on
ordinary layout, semantic, or frame revisions.

### Transform contract

Transforms use column vectors and right-handed local/world coordinates. Snapshot
Y-down is converted during projection before any local transform is built.
Quaternions are right-handed active rotations stored `[x, y, z, w]`, normalized
during full-generation validation, and composed as:

```text
local = translate(position)
      * translate(pivot)
      * rotate(quaternion)
      * scale(scale)
      * translate(-pivot)

world(child) = world(parent) * local(child)
clip = projection * view * world * local_vertex
```

Parent visibility is ANDed with child visibility. Parent opacity multiplies child
opacity. A negative or non-uniform effective scale anywhere in a
`LitShallowCard` node's ancestor chain is invalid in the first version; the
compiler therefore uses the composed rotation for card normals without requiring
a general inverse-transpose path.

The CPU resolves world matrices only for dirty shallow subtrees and writes those
matrices to the persistent frame mirror. The vertex shader applies world and
camera matrices to vertices. This keeps hierarchy reconciliation deterministic
without baking screen-space vertices on the CPU.

Attachments are named local transforms owned by a node. `Follow` resolves against
the current owner world matrix every frame. `SnapshotWorldOnSpawn` copies the
resolved world transform into a spawned instance, which then evolves
independently. Future chest bubbles can therefore follow the lid while attached
or detach into world motion without bespoke coordinate code.

## Rendering Contract

### Render phases and depth

Semantic categories such as “pet” and “foreground prop” do not override world Z.
The fixed phases are pipeline/depth/blend phases:

| Phase | Blend | Depth test | Depth write | Ordering |
|---|---|---:|---:|---|
| World opaque/cutout | replace or cutout | yes | yes | batch-compatible |
| World ordered blend | source-over, multiply, or additive | yes | no | one back-to-front stream |
| Screen chrome | source-over | no | no | authored stable order |

Pet, room, props, tank life, projections, and world effects participate according
to material behavior, not a painter-category phase. The aperture/final composite
remains a host/output concern after world and chrome rendering.

### Transparency and batching

Opaque/cutout items may be globally regrouped by compatible pipeline, material,
resource page, and depth policy. Every world item using source-over, multiply, or
additive blending enters one camera-depth-ordered stream because those blend modes
do not commute with one another. Items sort back-to-front by current camera-space
depth and a stable semantic tie-breaker. The renderer may merge only adjacent
compatible runs; it cannot pull compatible items across an intervening item.

Blended draw records and sort scratch have fixed capacities. The compiler emits
immutable record templates; when a blended node/instance or camera depth changes,
the CPU updates keys and stable-sorts the active records in the reusable scratch
arena. Frames with no blended-depth changes reuse the previous order. This bounded
sort is the deliberate exception to immutable opaque batch order and performs no
heap or GPU-object allocation.

Dynamic blended instance groups choose one declared policy:

- CPU-sort active instances back-to-front into a fixed mirror; or
- use authored non-crossing depth buckets whose relative order cannot change.

The first implementation does not claim order-independent transparency. Future
bubbles default to the shared CPU-ordered stream because they may cross other
blended world content in depth. A separate unsorted additive pass is permitted
only for an explicitly screen-local effect that makes no cross-world-Z ordering
claim.

### Linear-light and alpha contract

The renderer queries adapter/surface capabilities and requires
`Bgra8UnormSrgb`, `SurfaceColorSpace::Srgb`, and
`CompositeAlphaMode::PostMultiplied` for the transparent SDR companion surface.
Pinned wgpu 30 Metal exposes `Opaque` and `PostMultiplied`, not
`PreMultiplied`. If the required combination is unavailable, the new retained
path is unavailable and host fallback remains active. The chosen format, color
space, and alpha mode are recorded in metrics and capture metadata; no suffix
stripping is used to preserve gamma-space parity.

The contract is:

1. Authored scalar sRGB colors are decoded to linear exactly once.
2. Coverage-only glyph data uses `R8Unorm` and modulates a linear color.
3. Color atlas data uses straight-alpha `Rgba8UnormSrgb` with linear filtering,
   per-entry padding, and edge-dilated RGB in zero-alpha gutter texels.
4. Fragment shaders operate in linear light and emit premultiplied-linear RGB.
5. Source-over blending uses `One`, `OneMinusSrcAlpha`.
6. World and chrome render into a persistent transparent sRGB intermediate whose
   stored RGB represents encoded premultiplied-linear output.
7. A final surface pass samples/decodes that intermediate, unpremultiplies in
   linear light with a zero-alpha guard, and emits straight-linear RGB plus alpha
   to the `PostMultiplied` sRGB surface.

AppKit-produced glyph/color pixels are premultiplied sRGB. Before upload to a
straight-alpha color atlas they are unpremultiplied in sRGB with a zero-alpha
guard; sampling then performs the sRGB-to-linear decode and the shader
premultiplies in linear light. Coverage-only glyphs bypass color unpremultiplying.

Canonical transparent capture reads the persistent premultiplied intermediate
before the final surface conversion. Normalization decodes captured RGB from
sRGB, unpremultiplies in linear light with a zero-alpha guard, then re-encodes
straight RGB to sRGB for the PNG contract. Tests lock transparent-edge, blend,
surface-pass, and readback samples so the capture is not “visually close” but
mathematically defined.

### Persistent resources, mirrors, and draws

Generation preparation has two explicit products. The dedicated serial CPU
worker builds a candidate:

1. validates the complete template and fixed capacities;
2. resolves resource manifests, performs pure atlas packing, and rasterizes the
   packed glyph repertoire into worker-owned offscreen bitmap contexts;
3. builds immutable vertices/indices, private dense node/slot maps, and fixed
   blended-draw record templates/scratch;
4. compiles opaque/chrome batch ranges and initial blended stream order;
5. allocates fixed-layout content/frame CPU mirrors;
6. prepares initial content/frame mirrors from the newest snapshot;
7. returns one immutable CPU candidate.

The raster worker creates, uses, and destroys every Cocoa object on that worker
thread and returns only owned Rust pixels, entries, identities, and timings.
No `NSFont`, `NSGraphicsContext`, `NSBitmapImageRep`, raw bitmap pointer, wgpu
object, or live `AppState` reference crosses the mailbox boundary. The render
owner then materializes pipelines, textures, bind groups, fixed GPU buffers,
depth/intermediate targets, and initial uploads into a GPU candidate. Nothing is
published to the active slot until the activation state machine commits it.

This is an evidence-driven amendment to the original bounded-AppKit-lane design.
Native phase timing proved that fallback text setup/measurement can consume more
than 8 ms in one non-preemptible AppKit call, so a 4 ms UI-thread slice cannot be
made truthful by changing glyph-boundary scheduling. Apple permits offscreen
bitmap drawing on a secondary thread when that thread owns its graphics context,
flushes explicitly, and drains its own autorelease pools. The worker restores the
thread-local graphics context through an RAII guard on success, failure, or Rust
unwind. Objective-C message exceptions are converted at the `objc2` boundary and
feed the same typed terminal worker-failure path instead of unwinding through
Rust. The worker boundary preserves current raster output while removing AppKit
raster calls from the UI thread.

After warmup, an ordinary visible tick performs no atlas growth, pipeline
creation, shader compilation, full-template validation, heap-capacity growth, or
persistent GPU-object creation. “Zero GPU objects per frame” means zero persistent
textures, buffers, bind groups, pipelines, samplers, or depth attachments; a
frame-scoped command encoder, render pass, and acquired surface view are expected.

The UI thread applies bounded mirror deltas, coalesces dirty byte ranges, updates
the fixed blended stream only when a blended depth key changed, issues bounded
queue writes, encodes compiled batch ranges, submits, and records a disposition.
Static geometry and opaque/chrome batch order are not uploaded or re-sorted during
ordinary motion.

## First Renderer-Native Feature: Lit Treasure Chest

The proof prop preserves the existing 3×2 authored treasure-chest glyph sprite and
warm palette rather than replacing it with a generic box. Version 1 pairs the
open/closed sprite texture with one closed, convex local outline in chest-cell
units, listed counter-clockwise when viewed from `+Z`:

```text
(-1.50, -1.00), (1.50, -1.00), (1.50, 0.65),
(1.25,  1.00), (-1.25, 1.00), (-1.50, 0.65)
```

The compiler constructs `ChestCardGeometryV1` deterministically:

- UVs map the outline's 3×2 bounding box onto the current generated sprite;
- the front face is the outline inset by `0.08` cell units at Z `0.0`, triangulated
  as a convex fan, with local normal `+Z`;
- a bevel ring connects that inset face to the authored outline at Z `-0.08`;
- side quads connect the outer outline to a reversed back outline at Z `-0.28`;
- edge-offset intersection defines inset vertices with a `2.0×` miter cap;
- front winding is counter-clockwise from `+Z`, the back is reversed, side winding
  faces outward, and back-face culling is enabled;
- front albedo combines the current open/closed glyph raster with the authored
  amber base; bevel/side/back albedo use named darker palette roles;
- flat per-face normals make the depth legible; there is no alpha-mask extrusion;
- a fixed local yaw of `+12°` and pitch of `-6°` expose top/right depth under the
  orthographic camera;
- the full ancestor chain has positive uniform scale, as required by the first
  normal contract;
- ambient, clamped Lambert diffuse, and a bounded view/key rim term operate in
  linear light;
- `bubble_origin` is local `[0.0, 1.25, 0.0]`, above the lid.

Preview Lab includes far, neutral, near, unlit-control, lit-key-left,
lit-key-right, and attachment-transform fixtures. The pet, HUD, and gauges remain
unlit during this proof so review isolates the new depth/material behavior.

This feature follows the base scene cutover. It does not add production bubbles.

## Threading, Pacing, and Ownership

### Thread affinity

| Work | Owner |
|---|---|
| Capture privacy-projected state and create snapshot | AppKit/UI thread |
| Reconcile current snapshot and coalesce pending request | AppKit/UI thread, bounded |
| Full template validation and CPU compilation | dedicated serial worker |
| Pure atlas packing and activation candidate assembly | dedicated serial worker |
| Offscreen AppKit-dependent glyph rasterization | dedicated serial worker; worker-local Cocoa objects and autorelease pools |
| GPU resource/pipeline creation, upload, and destruction | AppKit/render owner |
| Atomic activation and persistent GPU writes | AppKit/render owner |
| Acquire, encode, submit, present, acknowledge | AppKit/render owner |
| Capture request scheduling and result publication | host/render owner |

No atlas construction, pipeline creation, full-template validation, unbounded
allocation, or blocking worker wait occurs in the ordinary presentation tick.
The UI owns one running request plus one replaceable latest pending request and
polls bounded mailboxes without waiting while the last good generation remains
visible. The raster worker checks cancellation between glyphs and returns only a
complete immutable CPU candidate. Workers produce immutable CPU descriptors,
pixels, geometry, draw records, and initial buffer bytes only. In V1, every wgpu
device call, resource or pipeline creation, upload, destruction, and activation
occurs on the render owner.
Parallel GPU materialization is a separate future optimization that requires
profiling evidence and a new ownership decision.

Stage 0 freezes numeric p95/p99 UI-thread budgets and requires zero main-thread
raster calls. Generation-service enqueue, nonblocking poll, and state transition
work has a 4 ms maximum. Worker raster time is reported separately and is never
relabelled as a passing UI slice. Activation itself performs only the pre-measured
bounded materialization/swap, initial writes, and first-frame encode.

The initial visible cadence may remain 15 FPS. Hidden, minimized, or occluded
windows suspend presentation and semantic animation work. New snapshots may
coalesce while hidden, but no backlog of historical frames is replayed. Reveal
reconciles the newest state once.

### Host boundary

The retained host continues to own the CAMetalLayer, surface/device lifecycle,
resize integration, AppKit unwind boundary, capture/readback, progress and
terminal dispositions, fallback, accessibility, input, and restoration. It does
not derive product semantics or walk scene primitives.

Surface resize updates the configured extent and dependent surface resources.
Backing-scale changes affect point-to-pixel conversion and resource selection,
not semantic topology unless the logical companion layout itself changes.

## Capture and Fallback Transition

Introduce a renderer-neutral `CompanionCaptureSnapshot` in
`presentation::companion_scene::contract` before live cutover. It contains the
selected `SceneVersion`, logical state alias, privacy claims, surface/color
metadata, canonical readback request, and scene metrics. Retained `capture.rs`
owns only GPU request/readback and renderer-specific evidence. During the
temporary shadow period, both Smooth and scene-runtime evidence are derived from
the same projected semantic snapshot.

Migration order matters:

1. Move capture request semantics and privacy metadata out of
   `SmoothCompanionScenePlan`.
2. Preserve GPU-native readback and the canonical transparent normalization.
3. Let Smooth and the new renderer populate renderer-specific evidence beneath
   the neutral snapshot during shadow review.
4. Switch live capture acknowledgement to the active scene generation.
5. Delete Smooth-plan capture translation only after the live canary hold.

If the retained scene cannot present, the host cold-builds the current Smooth
fallback frame from the latest domain state. Fallback is acknowledged only after
that frame is actually painted, preserving the existing pending-to-painted
contract. The scene runtime never emits Smooth types and does not keep a mirrored
Smooth plan warm every tick.

### Failure actions

| Failure | Action |
|---|---|
| Invalid/superseded pending generation | drop it, retain active generation |
| Capacity overflow | request/coalesce new generation; retain active generation |
| Candidate validation/resource failure before acquire | destroy candidate, retain active generation |
| Acquire `Outdated` | reconfigure once and skip this tick; retry candidate on a later tick, with no loop in one tick |
| Acquire `Timeout` or `Occluded` | skip this tick; retain ready candidate and active generation |
| Candidate encode failure before submit | destroy candidate; retain previous generation if its epochs remain usable |
| Stale content/frame update | reject and count; never mutate mirrors |
| `SurfaceLost` or surface validation failure | no retry loop; immediately enter host fallback pending, matching current policy |
| Device validation/loss, OOM, or no usable active generation | invalidate `DeviceEpoch`; immediately enter host fallback pending |
| Delayed GPU error after activation commit | invalidate `DeviceEpoch`; enter fallback rather than restore prior GPU resources |
| Fallback paint succeeds | acknowledge fallback painted |
| Capture during generation swap | bind request to one active generation or defer |

Diagnostics and GPU labels are sanitized; no project names, prompts, file paths,
raw pet seeds, or unprojected user data are exposed.

## Observability and Baseline

Before implementation measurements are inspected, record a versioned baseline
protocol and numeric pass/regression gates for the current production companion.
The protocol fixes build profile, device/OS, window sizes/scales, fixtures,
warmup, sample duration, capture settings, and aggregation method. Thresholds may
be revised only with a written reason, not moved after an unfavorable result.

Each metrics snapshot includes schema version and:

- device, surface, layout, resource-generation, semantic, and frame counters;
- monotonic counts for requests, coalesces, cancellations, stale rejections,
  prepares, activation attempts/successes/failures, fallbacks, and captures;
- node, primitive, material, light, attachment, capacity, active-instance, batch,
  and draw high-water marks;
- static upload count/bytes and content/frame write count/ranges/bytes;
- persistent GPU-object creation/destruction and depth-attachment reuse;
- atlas/resource misses and unexpected growth;
- queue wait, snapshot/reconcile, compile, activation, mirror-write, encode, and
  submit duration histograms;
- p50/p95/p99 UI-thread and encode latency;
- CPU-mirror, GPU-resource, and process-memory high-water marks;
- hidden-window wake/work counts and frame dispositions.

Counters are bounded and privacy-safe. Evidence aliases use fixture names, never
raw user-derived text.

## Preview and Evidence Contract

Preview Lab adds first-class, versioned artifacts:

```text
frames/<id>.companion-snapshot.json
frames/<id>.companion-generation.json
frames/<id>.companion-content.json
frames/<id>.companion-frame.json
frames/<id>.companion-batches.json
frames/<id>.companion-resources.json
frames/<id>.companion-metrics.json
frames/<id>.companion-readback.png
```

Required deterministic fixtures include:

- normal, active, asleep, helper-trouble, and dim states;
- all pet species/stages and maximum generated-art lattice occupancy;
- far, neutral, and near authored depth cues;
- representative props and tank life in every world depth range;
- opaque/cutout occlusion and crossing translucent order;
- transparent atlas edges and capture normalization;
- chest controls, light directions, and attachment world transforms;
- resize and backing-scale changes independently and together;
- stale/out-of-order updates, superseded generation work, capacity edges, and
  build/resource/activation/capture failure states.

Smooth captures are migration evidence, not part of the final scene artifact
schema or a post-approval parity requirement.

## Verification Strategy

### Pure contracts and reconciliation

- deterministic projection, IDs, checksums, and private dense mapping;
- hierarchy, pivot, quaternion, transform, parent visibility/opacity, and
  attachment composition;
- orthographic world-Z projection and separate `DepthCue` behavior;
- invalidation classification for frame, content, layout, resource, and surface
  changes;
- monotonic revisions and rejection of old generation/update results;
- pending request coalescing/cancellation and reconcile-again-before-activation;
- capacity, resource, privacy, and fixed enum validation;
- exact dirty-slot-to-byte-range coalescing.

### Lifecycle and fault matrix

Exercise every timing boundary:

- newer snapshot arrives during prepare, ready, and activation;
- semantic/frame revision advances while a new generation compiles;
- activation fails before and after candidate resource creation;
- resize/scale storms occur during generation work and capture;
- capture is requested immediately before, during, and after a swap;
- device/surface faults occur during acquire, write, encode, submit, and readback;
- capacity is exactly full and then exceeds by one;
- hidden state receives topology changes and reveals;
- worker cancellation and application shutdown race safely;
- fallback pending is not acknowledged until a Smooth frame actually paints.

Every failure asserts the active generation/mirror checksum, disposition, resource
counts, and next permitted transition.

### Renderer correctness

- world pipelines always have the specified depth state;
- opaque/cutout elements occlude by Z across semantic categories;
- source-over/multiply/additive world items share one back-to-front order and
  batch only contiguous compatible runs;
- CPU-sorted dynamic blended instances cross static and dynamic blended content
  correctly in depth;
- sRGB decode/output, premultiplied-linear blending, AppKit upload conversion,
  and transparent capture normalization match locked samples;
- camera/node transforms reach the shader and produce nonzero clip depth;
- depth attachment reuse/recreation matches the exact epoch/extent contract;
- aperture, resize, scale, and chrome isolation are correct;
- lit-card normal, diffuse, ambient, rim, yaw/pitch, and uniform-scale behavior are
  deterministic.

### Lifetime, soak, and performance

The 300-frame production-derived run remains as a quick smoke test, not the
endurance claim. After warmup, assert zero template builds, static uploads,
resource misses, buffer/atlas growth, and persistent GPU-object creation.

The acceptance sequence is:

1. 300 ordinary frames for fast local smoke.
2. 4,500 virtual-time frames covering presentation, semantic ticks, minute
   changes, content changes, and topology requests.
3. Five real minutes visible on native Metal with bounded resources and no
   fallback.
4. A resize/scale/occlusion/fault soak with deterministic injections.
5. A multi-hour release-candidate soak before deleting rollout scaffolding.

All runs compare versioned p50/p95/p99 latency, upload bytes, draw/batch counts,
hidden work, resource high-water marks, and memory against the frozen baseline.
No success claim may hide an unexplained material regression behind an average.

Existing companion, Preview Lab, capture, privacy, keyboard, fullscreen, input,
accessibility, packaging, formatting, lint, and test suites remain required.

## Migration Program

Temporary shadow/canary work below is rollout scaffolding, not reverse-
compatibility architecture.

### Stage 0: freeze safety and baseline

- Record the current host/fallback/capture invariants as executable tests.
- Freeze the numeric baseline protocol and gates.
- Freeze p95/p99 ordinary UI-thread budgets, require zero main-thread raster
  calls plus a maximum generation-service UI slice, and freeze activation and
  metrics-overhead gates before scene implementation proceeds.
- Inventory maximum production-derived nodes, art lattices, props, tank life,
  particles, resources, and current failure dispositions.

Exit: current safety contracts and capacity inputs are measured and versioned.

### Stage 1: extract neutral semantics

- Extract shared pure pet/habitat/activity/helper inventories and projection.
- Make the ownership of existing `PresentationScene`, `RoundSceneModel`, and
  `PetSceneModel` explicit: reuse pure semantic helpers or retire redundant
  projections so none remains a second companion-scene authority.
- Add immutable `CompanionSceneSnapshot` fixtures.
- Keep TUI and live retained rendering behavior unchanged.

Exit: the same renderer-neutral snapshot meaning can feed deterministic TUI and
new-scene fixtures without TUI draw output entering the new path.

### Stage 2: implement contracts and reconciler

- Add fixed scene enums, transforms, capacities, attachments, revisions, and
  evidence schemas.
- Implement failing-first invalidation, stale work, coalescing, and activation
  state-machine tests.
- Keep all work platform-neutral and off the live path.

Exit: pure tests cover every state transition and revision edge.

### Stage 3: offscreen compiler, lifecycle, and capture

- Compile the full unlit companion into fixed GPU resources and dense mirrors.
- Add depth, color/alpha, phase, batching, and transform contracts.
- Introduce renderer-neutral capture snapshots and canonical new-path readback.
- Exercise atomic activation and fault injection offscreen/native without live
  cutover.

Exit: the full companion renders without a Smooth plan, and lifetime/correctness
tests pass with deterministic evidence.

### Stage 4: native depth, resize, and color approval

- Close native occlusion, transparency, sRGB, transparent-edge, capture, resize,
  and scale fixtures.
- Retune authored colors deliberately for linear-light output.
- Receive visual approval across the required companion matrix.

Exit: depth and color behavior are accepted before production traffic is moved.

### Stage 5: shadow production snapshots

- Build/reconcile the new runtime from production snapshots without presenting it.
- Measure churn, queueing, cancellation, capacity, latency, memory, and faults.
- Compare new captures only in bounded review runs, not every production tick.

Exit: no unresolved capacity, semantic divergence, lifecycle, or baseline breach.

### Stage 6: opt-in live and automated soak

- Add a local opt-in retained-scene presentation mode.
- Run the complete automated suite, 4,500-frame lifetime test, native smoke, and
  resize/scale/fault soak.
- Preserve a one-line host routing rollback to Smooth.

Exit: opt-in live mode satisfies all correctness, safety, and performance gates.

### Stage 7: Auto canary and hold

- Make the new scene runtime the Auto choice for a bounded canary.
- Track generation failures, stale rejections, fallbacks, latency, resource
  growth, capture success, and privacy disposition.
- Complete the multi-hour release-candidate soak and written canary hold.

Exit: the canary record has no unresolved blocker and rollback remains proven.

### Stage 8: delete transitional translation

- Delete `prepare_gpu_frame` and the Smooth-plan-to-GPU translator.
- Delete obsolete parity helpers and shadow comparison wiring.
- Keep Smooth only as the existing cold host fallback renderer.

Exit: exactly one retained companion scene-generation path remains.

### Stage 9: lit treasure-chest proof

- Add the authored shallow-card geometry, fixed material/light parameters, and
  attachment.
- Run deterministic, native, color, depth, and transform evidence.
- Verify the feature adds typed scene data and compiled resources rather than a
  bespoke painter or host branch.

Exit: the lit chest is accepted and a later bubble emitter can use
`bubble_origin` with either attachment mode.

Production bubbles follow as a separate design/implementation after scene
generation is fixed.

## Acceptance Criteria

### Structural

- One immutable snapshot is the semantic authority for each reconciliation.
- One stateful reconciler owns invalidation, revisions, generations, and stale
  result rejection.
- `GenerationKey` is immutable for a compiled GPU unit, `AppliedRevisions` advances
  for compatible ordinary updates, and `SceneVersion` identifies each
  presentation, capture, and metric sample.
- Activation retains the previous unit and atomically commits template, resources,
  initial mirrors, and capture metadata only at the specified first-present
  milestone.
- The retained live path does not construct or consume
  `SmoothCompanionScenePlan`, `SmoothCompanionLayer`, `SceneDrawList`, `DrawCell`,
  or Ratatui geometry after Stage 8.
- Platform-neutral scene contracts contain no AppKit, Metal, wgpu, or window
  types.
- Stable semantic IDs are separate from private dense compiler indices.
- Fixed capacities and closed primitive/material/blend/attachment enums are
  explicit and versioned.
- Ordinary frames do not rebuild, clone, fully validate, or globally re-sort the
  scene.

### Rendering and forward capability

- World elements occlude using real depth across semantic categories.
- Orthographic Z affects occlusion only; artistic depth treatment uses explicit
  `DepthCue` values.
- Opaque batching and transparent contiguous-run batching preserve blend order.
- Linear-light, alpha, atlas upload, surface, and capture behavior obey one tested
  contract.
- Screen chrome remains isolated from world lighting and depth.
- The lit chest preserves authored identity, responds to light, exposes correct
  geometry under orthographic view, and publishes a verified `bubble_origin`.
- A future fixed-capacity bubble group can attach, detach, sort, and render
  without changing host lifecycle or inventing a painter branch.

### Performance and reliability

- After warmup, ordinary frames create no persistent GPU objects, grow no fixed
  mirrors/resources, perform no full validation, and write only expected ranges.
- The 300-frame smoke, 4,500-frame virtual run, five-minute native run,
  resize/scale/fault soak, and multi-hour release-candidate soak pass.
- Versioned latency, uploads, batches/draws, hidden work, resources, and memory
  satisfy the frozen numeric gates.
- Out-of-order work, failed candidates, and stale updates preserve the last good
  active generation and mirror checksum.
- Capture binds to one generation and fallback is acknowledged only after paint.
- Existing surface/device recovery, AppKit unwind, privacy, input, accessibility,
  and packaging checks remain green.

### Evidence

- Snapshot, generation, content, frame, batch, resource, metric, and readback
  artifacts are deterministic Preview Lab contract entries.
- Native captures cover lifecycle, depth, transparency, color, resize/scale,
  capture-swap, and lit-prop states.
- Metrics expose all epochs/revisions, queueing/cancellation, churn, dirty writes,
  persistent objects, high-water marks, and latency percentiles.
- The final review record confirms the canary hold and deletion of transitional
  Smooth-plan translation.

## Risks and Mitigations

### Accidental generic-engine expansion

Keep fixed enums, hard capacities, shallow hierarchy, private compiler indices,
and feature-justified capabilities. Do not expose generic registries or escape
hatches in anticipation of unknown needs.

### Semantic drift during direct projection

Extract shared pure meaning first, derive both temporary evidence paths from the
same immutable snapshot, and shadow production snapshots before cutover. Remove
the translator only after canary evidence, without retaining two long-term scene
implementations.

### Nominal lifetimes that still rebuild everything

Use persistent fixed-layout mirrors, compiler-owned dense maps, dirty-slot deltas,
object/allocation counters, and production-derived lifetime tests. Synthetic
buffer-reuse tests alone do not satisfy the gate.

### Incorrect transparency under real depth

Use depth writes only for opaque/cutout content, sort all cross-world blended
items back-to-front in one fixed-capacity stream, merge only contiguous compatible
runs, and require explicit dynamic-instance ordering policy.

### Linear-light changes Glorp's look

Define conversion and alpha math end-to-end, lock sample tests, retune authored
colors deliberately, and obtain native visual approval. Do not reintroduce
gamma-space lighting for parity.

### Main-thread stalls and stale worker work

Keep one coalesced pending request, cancel at worker checkpoints, reconcile again
against the newest snapshot, bound activation, and measure queue/UI latency at
p95/p99 rather than averages alone.

### Capture or fallback regressions at cutover

Migrate capture semantics before routing live frames, bind evidence to an active
generation, preserve GPU-native readback, cold-build Smooth only on fallback, and
retain pending-to-painted acknowledgement.

## Implementation Planning Boundary

The implementation plan should cover Stages 0–9 in dependency order, with exact
source files, failing-first tests, Preview Lab/native evidence commands, numeric
capacity derivation, frozen performance gates, fault injections, commit
boundaries, and rollback checkpoints.

It must not mix in production bubbles, generic mesh loading, perspective-camera
behavior, PBR materials, or retirement of host-level Smooth fallback. Temporary
shadow/canary work must be visibly scoped as deletion-bound rollout scaffolding.
