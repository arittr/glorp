# Glorp Companion 2.5D Scene Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Replace the companion retained renderer's per-frame Smooth-plan translation with a direct, persistent, depth-aware wgpu scene runtime, then prove it with a lit treasure chest and a stable attachment contract for later bubbles.

**Architecture:** Project one immutable, privacy-aware CompanionSceneSnapshot from companion domain state, reconcile it into topology/content/frame lifetimes, compile fixed-capacity CPU and GPU generations, and atomically activate a candidate only after its first surface present. Keep all wgpu ownership on the AppKit render owner, render world content in linear light through a premultiplied intermediate and explicit PostMultiplied surface pass, and retain Smooth only as a cold host fallback plus deletion-bound rollout scaffolding.

**Tech Stack:** Rust 2021, wgpu =30.0.0, WGSL, Metal/CAMetalLayer through objc2, AppKit/CoreText glyph rasterization, bytemuck POD buffers, serde/serde_json evidence, png readback, Rust xtask, Preview Lab.

**Spec:** docs/superpowers/specs/2026-07-11-glorp-companion-scene-runtime-design.md

## Global Constraints

- Work on the current checkout; do not create a branch without Drew's explicit approval.
- Do not create, update, or consult Linear for this repository.
- Preserve unrelated user changes and stage only paths named by each task.
- Scope is the native macOS round companion and retained wgpu renderer only; terminal watch, TUI, menubar, Classic, and Pixel behavior must not change.
- CompanionSceneSnapshot is the only semantic authority for the new retained path; no new path may consume DrawCell, SceneDrawList, SmoothCompanionScenePlan, SmoothCompanionLayer, or Ratatui geometry.
- Smooth is temporary rollout evidence and the cold host fallback, not a scene backend or compatibility contract.
- V1 has no ECS, physics, scripting, public resource registry, custom shaders, generic mesh loader, PBR, post-processing stack, perspective camera, or production bubbles.
- Workers produce immutable CPU data only. Every wgpu device call, GPU creation, upload, write, destruction, encode, submit, and activation remains on the AppKit render owner.
- Scene world space is right-handed: X right, Y up, Z toward the camera. Snapshot layout is X right/Y down and converts once during projection.
- Orthographic depth maps greatest near_z to 0.0 and least far_z to 1.0; use Depth24Plus, clear 1.0, compare LessEqual.
- Require Bgra8UnormSrgb, SurfaceColorSpace::Srgb, and CompositeAlphaMode::PostMultiplied; otherwise the scene runtime is unavailable and the host uses Smooth fallback.
- World/chrome render into a persistent sRGB intermediate carrying encoded premultiplied-linear RGB; the final surface pass unpremultiplies in linear light and writes straight RGB for PostMultiplied composition.
- Source-over, multiply, and additive world items share one back-to-front stream; batch only adjacent compatible records.
- Fixed V1 limits are: 128 nodes, 768 static primitives, 130 pet art slots, 10 visible props, 2 round tank inhabitants, 64 ambient instances, 256 blended draw records, 2 lights, and 32 attachments. Task 1 must prove the full fixture inventory fits; increasing a limit requires measured evidence and a spec amendment.
- The initial visible cadence remains UI_TICK_INTERVAL_SECS = 0.25 (4 Hz). Cadence changes are outside this plan.
- After warmup, ordinary frames create no persistent GPU objects, grow no fixed storage, run no full validation, and perform no atlas or pipeline compilation.
- Preserve the Objective-C unwind guard, hidden suspension, transactional layer activation, async GPU mailbox, fallback pending-to-painted acknowledgement, full glyph repertoire, GPU-native capture, privacy scanning, input/accessibility behavior, and packaging matrix.
- Do not flip the scene runtime into Auto, delete the translator, or land the lit chest before the explicit gates in Tasks 16–18.
- Routine focused commands should finish within two minutes. Five-minute and multi-hour soaks are explicit gate steps.

---

## File Responsibility Map

- src/presentation/companion_scene/mod.rs: renderer-neutral snapshot/runtime boundary.
- src/presentation/companion_scene/input.rs: privacy-aware WatchViewModel projection and shared semantic inventories.
- src/presentation/companion_scene/scene.rs: fixed IDs, transforms, nodes, capacities, template/content/frame, materials, and attachments.
- src/presentation/companion_scene/runtime.rs: revisions, reconciliation, coalescing, and pure activation state.
- src/presentation/companion_scene/contract.rs: serialized scene/capture/evidence DTOs.
- src/presentation/companion_scene/validate.rs: full-generation and bounded-delta validation.
- src/presentation/companion_scene/chest.rs: ChestCardGeometryV1, added only in Task 18.
- src/companion/retained.rs: facade and temporary legacy translator until Task 17.
- src/companion/retained/host.rs: CAMetalLayer, adapter/device/surface, surface epochs, resize.
- src/companion/retained/compiler.rs: CPU compilation, dense indices, immutable geometry, batch records.
- src/companion/retained/buffers.rs: persistent CPU mirrors, GPU buffers, dirty spans, no-growth counters.
- src/companion/retained/render.rs: GPU materialization, targets, pipelines, encode/present, activation.
- src/companion/retained/scene.wgsl: scene shaders and final PostMultiplied surface pass.
- src/companion/retained/resources.rs: AppKit rasterization, straight-alpha/coverage atlases, padding/dilation.
- src/companion/retained/capture.rs: GPU capture/readback and renderer-specific evidence only.
- src/companion/retained/presentation.rs: milestones, dispositions, sanitized failures, GPU mailbox.
- src/companion/retained/metrics.rs: counters, bounded histograms, high-water marks, baseline comparison.
- src/companion/app.rs: AppKit tick, snapshot capture, scene orchestration, rollout routing, cold fallback.
- src/companion/paired_review.rs: temporary Smooth/scene evidence coordinator.
- src/commands/companion_mode.rs: deletion-bound rollout policy and fault injection.
- src/dev_preview/{contract,round,scenarios,export}.rs: scene fixtures and typed artifacts.
- xtask/src/lib.rs: baseline, review, fault soak, lifetime, and canary commands.
- tests/companion_scene_boundary.rs: source-layer and ownership bans.
- tests/retained_scene.rs: retained scene integration and lifetime tests.
- docs/superpowers/measurements/2026-07-11-glorp-companion-scene-baseline.md: frozen Stage 0 baseline and gates.

## Program Stop Gates

1. Stop after Task 1 if current safety or baseline instrumentation cannot remain deterministic without changing live behavior.
2. Stop after Task 6 if a full neutral companion fixture cannot be built without TUI/Smooth types.
3. After Task 6, close the evidence-amended raster-worker prerequisite below. Tasks 2-6 may proceed under the bounded `stage0-appkit-raster-v1` disposition, but Task 7 and later work are blocked until main-thread raster calls are zero, generation-service UI work is <=4000 us max, and the native worker/parity gates pass.
4. Stop after Task 13 if native depth, alpha, color, capture, or atomic activation evidence is wrong.
5. Stop after Task 15 for any frozen performance breach, leak, hidden work, or fault failure.
6. Task 16 requires Drew's explicit approval before changing Auto.
7. Task 17 requires the completed canary hold before deleting translation.
8. Task 18 begins only after one retained scene-generation path remains.

---

### Task 1: Freeze safety, inventory, metrics, and numeric gates

**Files:**
- Create: src/companion/retained/metrics.rs
- Modify: src/companion/retained.rs
- Modify: src/companion/retained/presentation.rs
- Modify: src/companion/app.rs
- Modify: src/companion/paired_review.rs
- Modify: src/commands/companion_mode.rs
- Modify: src/cli.rs
- Modify: xtask/src/lib.rs
- Create: docs/superpowers/measurements/2026-07-11-glorp-companion-scene-baseline.md
- Test: src/companion/retained/metrics.rs
- Test: tests/retained_renderer_boundary.rs

**Interfaces:**
- Consumes: current ui_tick, FrameProgress, RetainedResourceCounters, paired review, full Preview Lab fixtures.
- Produces: CompanionRuntimeMetricsSnapshot, CompanionCapacityInventory, hidden --review-runtime-metrics-out PATH, and cargo xtask companion scene-baseline.

- [ ] **Step 1: Write failing bounded-histogram tests**

~~~rust
#[test]
fn fixed_samples_overwrite_oldest_and_report_percentiles() {
    let mut samples = FixedSamples::<4>::default();
    for value in [10, 20, 30, 40, 50] {
        samples.push(value);
    }
    assert_eq!(samples.sorted_values(), vec![20, 30, 40, 50]);
    assert_eq!(samples.percentile(50), Some(30));
    assert_eq!(samples.percentile(95), Some(50));
    assert_eq!(samples.percentile(99), Some(50));
}

#[test]
fn snapshot_carries_epochs_counters_and_high_water_marks() {
    let mut metrics = CompanionRuntimeMetrics::default();
    metrics.record_ui_tick_us(1_500);
    metrics.record_encode_us(800);
    metrics.record_persistent_gpu_create(3);
    metrics.observe_nodes(72);
    let snapshot = metrics.snapshot(RuntimeIdentity::baseline());
    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.ui_tick_us.p95, Some(1_500));
    assert_eq!(snapshot.encode_us.p99, Some(800));
    assert_eq!(snapshot.persistent_gpu_objects_created, 3);
    assert_eq!(snapshot.node_high_water, 72);
}
~~~

- [ ] **Step 2: Run the tests and verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::metrics
~~~

Expected: compile failure because metrics.rs and its types do not exist.

- [ ] **Step 3: Implement allocation-free metric storage**

Use METRIC_SAMPLE_CAPACITY = 4_096 and fixed [u32; N] rings for ui_tick_us, prepare_us, encode_us, queue_wait_us, compile_us, and activation_us. CompanionRuntimeMetrics uses saturating u64 counters for generations, coalesces, cancellations, stale rejections, uploads, writes, draws, persistent object creates/destroys, hidden ticks, skips, fallbacks, and captures. Track node/primitive/blended-draw/CPU-byte/GPU-byte high-water marks. Snapshot serialization may allocate; ordinary recording may not.

~~~rust
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(crate) struct RuntimeIdentity {
    pub device_epoch: u64,
    pub surface_epoch: u64,
    pub layout_generation: u64,
    pub resource_generation: u64,
    pub semantic_revision: u64,
    pub frame_revision: u64,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(crate) struct Percentiles {
    pub p50: Option<u32>,
    pub p95: Option<u32>,
    pub p99: Option<u32>,
}
~~~

- [ ] **Step 4: Instrument current safety seams without reordering them**

Record durations/counters around ui_tick, prepare_current_frame_from_state, prepare_gpu_frame, queue writes, encode/submit, resize, capture, fallback, and the hidden early return. Extend the source-boundary test so draw_scene still only consumes last_good_frame and never records scene metrics.

- [ ] **Step 5: Add deterministic capacity inventory**

~~~rust
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(crate) struct CompanionCapacityInventory {
    pub max_nodes: u32,
    pub max_static_primitives: u32,
    pub max_pet_slots: u32,
    pub max_visible_props: u32,
    pub max_round_tank_inhabitants: u32,
    pub max_ambient_instances: u32,
    pub max_blended_draws: u32,
    pub max_lights: u32,
    pub max_attachments: u32,
}
~~~

Collect maxima across every species/stage, normal/active/asleep/helper-trouble/dim, full prop catalog, full tank cast, and far/neutral/near. Assert every observed value fits the Global Constraints limits.

- [ ] **Step 6: Add the baseline command and generated report**

cargo xtask companion scene-baseline --duration-ms 120000 --out PATH must build release retained-renderer, run a redacted 360x360 fixture for 120 seconds, discard the first 20 visible ticks, read the JSON snapshot, and generate the measurement report with hardware/OS/build identity and these frozen gates:

~~~text
ui tick p95 <= 1422us
ui tick p99 <= 2070us
encode p95 <= 282us
main-thread raster calls = 0
generation-service UI work max <= 4000us
GPU materialize/upload/publish max <= 16000us
activation render-owner slice <= 16000us
metrics overhead <= 2% of baseline ui-tick p95
hidden steady state after one transition tick = zero prepare/write/acquire/encode/submit
ordinary post-warmup persistent GPU creations = 0
ordinary post-warmup static upload bytes = 0
RSS and accounted GPU bytes after 4500 virtual frames <= warmup high-water + 1%
~~~

The original target was an AppKit raster slice <=4000 us. Native phase evidence
later proved that one non-preemptible fallback text setup call can exceed 8 ms,
so the post-Task-6 amendment closes the same UI-safety intent with zero
main-thread raster calls and <=4000 us max generation-service UI work. Worker
raster time is reported separately and cannot satisfy or fail the UI gate.

- [ ] **Step 7: Run Stage 0 verification**

~~~bash
cargo fmt --check
cargo test --features retained-renderer companion::retained::metrics
cargo test --test companion_draw_boundary
cargo test --test retained_renderer_boundary
cargo test -p xtask
cargo xtask companion scene-baseline --duration-ms 120000 --out docs/superpowers/measurements/2026-07-11-glorp-companion-scene-baseline.md
git diff --check
~~~

Expected: all focused checks pass; the report has no pending/unknown/blank metrics; inventory fits fixed limits; fallback/capture behavior is unchanged.

- [ ] **Step 8: Commit**

~~~bash
git add src/companion/retained/metrics.rs src/companion/retained.rs src/companion/retained/presentation.rs src/companion/app.rs src/companion/paired_review.rs src/commands/companion_mode.rs src/cli.rs xtask/src/lib.rs tests/retained_renderer_boundary.rs docs/superpowers/measurements/2026-07-11-glorp-companion-scene-baseline.md
git commit -m "test(companion): freeze scene runtime baseline"
~~~

---

### Task 2: Project one renderer-neutral companion snapshot

**Files:**
- Create: src/presentation/companion_scene/mod.rs
- Create: src/presentation/companion_scene/input.rs
- Modify: src/presentation/mod.rs
- Modify: src/presentation/privacy.rs
- Modify: src/presentation/scene.rs
- Modify: src/presentation/pet_scene.rs
- Modify: src/round/model.rs
- Create: tests/companion_scene_boundary.rs

**Interfaces:**
- Consumes: WatchViewModel, room/habitat catalogs, pet art/spans, HUD/gauge helpers, RoundCompanion privacy.
- Produces: CompanionLogicalLayout and CompanionSceneSnapshot with nested topology/content/frame snapshots.

- [ ] **Step 1: Write failing projection and boundary tests**

~~~rust
#[test]
fn projection_is_one_privacy_aware_snapshot() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let snapshot = CompanionSceneSnapshot::project(
        &vm,
        time::macros::datetime!(2026-07-11 12:00 UTC),
        CompanionLogicalLayout::round(360.0, 360.0),
    );
    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.topology.pet.species, vm.pet_render.generated_species);
    assert!(snapshot.topology.visible_props.len() <= 10);
    assert!(snapshot.topology.visible_tank_inhabitants.len() <= 2);
    assert!(!snapshot.privacy.source_names_visible);
    assert!(!snapshot.privacy.file_paths_visible);
}

#[test]
fn projection_serialization_contains_no_raw_seed_or_source_name() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.seed = "very-secret-seed".to_string();
    let snapshot = CompanionSceneSnapshot::project(
        &vm,
        time::macros::datetime!(2026-07-11 12:00 UTC),
        CompanionLogicalLayout::round(360.0, 360.0),
    );
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(!json.contains("very-secret-seed"));
    assert!(!json.contains("claude"));
    assert!(!json.contains("codex"));
}
~~~

The boundary test recursively scans src/presentation/companion_scene and rejects ratatui, DrawCell, SceneDrawList, SmoothCompanion, wgpu, objc2, NSView, and CAMetalLayer.

- [ ] **Step 2: Verify failure**

~~~bash
cargo test presentation::companion_scene
cargo test --test companion_scene_boundary
~~~

Expected: compile failure because the module does not exist.

- [ ] **Step 3: Implement the snapshot**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CompanionLogicalLayout {
    pub width_points: f32,
    pub height_points: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CompanionSceneSnapshot {
    pub schema_version: u16,
    pub privacy: crate::presentation::privacy::PrivacyProjection,
    pub topology: TopologySnapshot,
    pub content: ContentSnapshot,
    pub frame: FrameSnapshot,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TopologySnapshot {
    pub layout: CompanionLogicalLayout,
    pub pet: PetTopologySnapshot,
    pub room: RoomTopologySnapshot,
    pub visible_props: Vec<PropTopologySnapshot>,
    pub visible_tank_inhabitants: Vec<TankTopologySnapshot>,
    pub renderer_schema: u16,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ContentSnapshot {
    pub pet_lines: Vec<String>,
    pub pet_roles: Vec<PetRoleSpanSnapshot>,
    pub palette: PaletteSnapshot,
    pub prop_animation_phases: Vec<u8>,
    pub tank_animation_phases: Vec<u8>,
    pub activity_pulse_age_ms: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PetRoleSpanSnapshot {
    pub line: u16,
    pub start: u16,
    pub end: u16,
    pub role: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PaletteSnapshot {
    pub body: [u8; 3],
    pub body_glow: [u8; 3],
    pub eye: [u8; 3],
    pub mouth: [u8; 3],
    pub accent: [u8; 3],
    pub pattern: [u8; 3],
    pub particle: [u8; 3],
    pub corruption: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FrameSnapshot {
    pub elapsed_ms: u64,
    pub pet_xy_depth: [f32; 3],
    pub facing: i8,
    pub breath_offset_y: u8,
    pub asleep: bool,
    pub helper_trouble: bool,
    pub gauges: [f32; 4],
    pub dim_amount: f32,
    pub hud_lines: [String; 3],
}
~~~

Pet topology stores species/stage and fixed lattice identity, never the raw seed. Prop/tank topology stores catalog ID, zone/route, stable order, and authored depth category; the builder computes positions without serializing a seed-derived token.
Add serde::Serialize to PresentationSurface and PrivacyProjection. Convert
StyledSegment and ResolvedPalette into the snapshot DTOs above instead of adding
serialization policy to pet-rendering types.

- [ ] **Step 4: Consolidate shared semantic derivation**

Extract pure vital bucket, helper health, activity pulse, room profile, visible prop selection, and visible round tank selection helpers. PresentationScene, PetSceneModel, RoundSceneModel, and the snapshot call these helpers; none independently re-derives the same meaning.

- [ ] **Step 5: Run regressions**

~~~bash
cargo test presentation::companion_scene
cargo test --test companion_scene_boundary
cargo test --test presentation_scene
cargo test --test round_scene
cargo test --test smooth_companion
~~~

Expected: all pass and existing fixtures remain unchanged.

- [ ] **Step 6: Commit**

~~~bash
git add src/presentation/companion_scene/mod.rs src/presentation/companion_scene/input.rs src/presentation/mod.rs src/presentation/privacy.rs src/presentation/scene.rs src/presentation/pet_scene.rs src/round/model.rs tests/companion_scene_boundary.rs
git commit -m "feat(companion): add renderer-neutral scene snapshot"
~~~

---

### Task 3: Define fixed scene, transform, material, and validation contracts

**Files:**
- Create: src/presentation/companion_scene/scene.rs
- Create: src/presentation/companion_scene/contract.rs
- Create: src/presentation/companion_scene/validate.rs
- Modify: src/presentation/companion_scene/mod.rs

**Interfaces:**
- Consumes: CompanionSceneSnapshot and fixed limits.
- Produces: IDs, Transform3/Mat4/camera, SceneTemplate/SceneContent/SceneFrame, fixed enums, attachments, validation, artifact DTOs.

- [ ] **Step 1: Write failing transform, depth, ID, and capacity tests**

~~~rust
#[test]
fn y_down_projection_becomes_y_up_once() {
    let transform = Transform3::from_snapshot_xy_depth([10.0, 20.0, 0.5], 100.0);
    assert_eq!(transform.translation, [10.0, 80.0, 0.5]);
}

#[test]
fn orthographic_depth_maps_near_to_zero_and_far_to_one() {
    let camera = OrthographicCamera::new(360.0, 360.0, -2.0, 2.0).unwrap();
    assert_eq!(camera.clip_depth(2.0), 0.0);
    assert_eq!(camera.clip_depth(-2.0), 1.0);
    assert_eq!(camera.clip_depth(0.0), 0.5);
}

#[test]
fn duplicate_ids_and_capacity_overflow_are_rejected() {
    let mut template = SceneTemplate::fixture();
    template.nodes.push(template.nodes[0].clone());
    assert_eq!(validate_template(&template), Err(SceneValidationError::DuplicateNodeId));
    template = SceneTemplate::fixture();
    template.capacities.max_nodes = 1;
    assert_eq!(validate_template(&template), Err(SceneValidationError::NodeCapacityExceeded));
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test presentation::companion_scene::scene
cargo test presentation::companion_scene::validate
~~~

Expected: compile failure for missing contracts.

- [ ] **Step 3: Implement fixed limits, IDs, and enums**

~~~rust
pub const MAX_SCENE_NODES: usize = 128;
pub const MAX_STATIC_PRIMITIVES: usize = 768;
pub const MAX_PET_ART_SLOTS: usize = 130;
pub const MAX_VISIBLE_PROPS: usize = 10;
pub const MAX_ROUND_TANK_INHABITANTS: usize = 2;
pub const MAX_AMBIENT_INSTANCES: usize = 64;
pub const MAX_BLENDED_DRAWS: usize = 256;
pub const MAX_LIGHTS: usize = 2;
pub const MAX_ATTACHMENTS: usize = 32;

pub struct NodeId(pub u32);
pub struct AttachmentId(pub u32);
pub enum PrimitiveKind { AtlasQuad, AnalyticShape, ShallowCard, InstanceQuad }
pub enum MaterialKind { UnlitGlyphSprite, UnlitAnalytic, LitShallowCard, MultiplyShadow, AdditiveGlow, ScreenChrome }
pub enum WorldBlend { Opaque, AlphaCutout, PremultipliedAlpha, Multiply, Additive }
pub enum AttachmentMode { Follow, SnapshotWorldOnSpawn }

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Transform3 {
    pub translation: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct DepthCue {
    pub scale: f32,
    pub y_offset_points_up: f32,
    pub opacity: f32,
    pub saturation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AttachmentTemplate {
    pub id: AttachmentId,
    pub owner: NodeId,
    pub local: Transform3,
}
~~~

Derive IDs with stable FNV-1a over canonical ASCII aliases and reject collisions using the retained alias map during full validation. Dense indices are compiler-private.

- [ ] **Step 4: Implement exact transform/camera math**

Use column vectors, right-handed active quaternions [x,y,z,w], pivot composition, parent multiplication, and clip_z = (near_z - world_z) / (near_z - far_z). Reject non-finite inputs, zero quaternions, invalid near/far, and negative/non-uniform effective scale in any LitShallowCard ancestor.

- [ ] **Step 5: Implement lifetimes and validation**

SceneTemplate owns hierarchy, immutable primitives/materials/resources, capacities, attachments, and privacy. SceneContent owns fixed semantic slots. SceneFrame alone owns camera, transforms, visibility, opacity, gauges, dim, and lights. Full validation checks IDs/cycles/references/capacities/material-depth compatibility and privacy; delta validation checks only version and slot bounds.

- [ ] **Step 6: Run checks**

~~~bash
cargo test presentation::companion_scene::scene
cargo test presentation::companion_scene::validate
cargo test --test companion_scene_boundary
cargo clippy --lib --all-features -- -D warnings
~~~

Expected: exact math tests and stable validation errors pass.

- [ ] **Step 7: Commit**

~~~bash
git add src/presentation/companion_scene/mod.rs src/presentation/companion_scene/scene.rs src/presentation/companion_scene/contract.rs src/presentation/companion_scene/validate.rs
git commit -m "feat(companion): define fixed scene contracts"
~~~

---

### Task 4: Implement revisions, reconciliation, coalescing, and pure activation state

**Files:**
- Create: src/presentation/companion_scene/runtime.rs
- Modify: src/presentation/companion_scene/mod.rs

**Interfaces:**
- Consumes: nested snapshot lifetimes and validated contracts.
- Produces: epoch/revision types, GenerationKey, AppliedRevisions, SceneVersion, CompanionSceneReconciler, ReconcileResult, GenerationRequest, and CompanionSceneRuntimeState.

- [ ] **Step 1: Write failing ordering tests**

~~~rust
#[test]
fn frame_change_does_not_allocate_a_generation() {
    let initial = CompanionSceneSnapshot::fixture();
    let mut reconciler = CompanionSceneReconciler::new(initial.clone());
    let mut moved = initial;
    moved.frame.pet_xy_depth[0] += 1.0;
    assert!(matches!(reconciler.reconcile(moved), ReconcileResult::Frame(_)));
    assert_eq!(reconciler.layout_generation(), LayoutGeneration(1));
    assert_eq!(reconciler.semantic_revision(), SemanticRevision(1));
    assert_eq!(reconciler.frame_revision(), FrameRevision(2));
}

#[test]
fn newer_topology_supersedes_pending_and_stale_completion_drops() {
    let mut runtime = CompanionSceneRuntimeState::fixture_active();
    let first = runtime.request_fixture_generation(Stage::S3);
    let second = runtime.request_fixture_generation(Stage::S4);
    assert_eq!(runtime.pending_request_id(), Some(second.request_id));
    assert_eq!(runtime.complete_cpu_candidate(first.request_id), RuntimeTransition::DropStale);
    assert_eq!(runtime.cancelled_build_count(), 1);
}

#[test]
fn candidate_commits_only_after_present_and_empty_mailbox() {
    let mut runtime = CompanionSceneRuntimeState::fixture_active();
    let candidate = runtime.fixture_ready_candidate();
    assert_eq!(runtime.begin_activation(candidate), RuntimeTransition::Activating);
    assert!(runtime.active_generation_is_previous());
    assert_eq!(runtime.observe_first_present(false), RuntimeTransition::CommitCandidate);
    assert!(runtime.active_generation_is_candidate());
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test presentation::companion_scene::runtime
~~~

Expected: compile failure for missing runtime types.

- [ ] **Step 3: Implement singular identity ownership**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DeviceEpoch(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SurfaceEpoch(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct LayoutGeneration(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ResourceGeneration(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SemanticRevision(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct FrameRevision(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct GenerationKey {
    pub device: DeviceEpoch,
    pub surface: SurfaceEpoch,
    pub layout: LayoutGeneration,
    pub resources: ResourceGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AppliedRevisions {
    pub semantic: SemanticRevision,
    pub frame: FrameRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SceneVersion {
    pub generation: GenerationKey,
    pub applied: AppliedRevisions,
}
~~~

Host inputs advance device/surface. Reconciler advances layout/semantic/frame. Runtime alone allocates resource generations and request IDs.

- [ ] **Step 4: Implement exhaustive reconciliation**

Compare TopologySnapshot, ContentSnapshot, then FrameSnapshot. Emit NewGeneration, Content with optional frame delta, Frame, or Unchanged. Add one mutation-classification test per field.

- [ ] **Step 5: Implement one-active/one-pending state**

Represent Active, Preparing, Ready, Activating { previous, candidate }, FailedRetaining, and HostFallbackPending. Completions carry request ID, GenerationKey, and source revisions. Rebase compatible newest content/frame before Ready. Return explicit drop-stale, retry-later, destroy-candidate, commit, retain-previous, and fallback transitions.

- [ ] **Step 6: Cover lifecycle races**

Test two topology completions out of order; updates against old/pending/ready/active; candidate failure; Outdated/Timeout/Occluded retry; SurfaceLost/validation/device/OOM fallback; capture binding; hidden update/reveal; cancellation; shutdown; delayed post-commit GPU error.

- [ ] **Step 7: Run and commit**

~~~bash
cargo test presentation::companion_scene::runtime
cargo test presentation::companion_scene
cargo clippy --lib --all-features -- -D warnings
git add src/presentation/companion_scene/mod.rs src/presentation/companion_scene/runtime.rs
git commit -m "feat(companion): reconcile scene generations and revisions"
~~~

---

### Task 5: Build the complete unlit CPU scene from the snapshot

**Files:**
- Modify: src/presentation/companion_scene/scene.rs
- Modify: src/presentation/companion_scene/input.rs
- Modify: src/presentation/companion_scene/validate.rs
- Test: src/presentation/companion_scene/scene.rs

**Interfaces:**
- Consumes: CompanionSceneSnapshot and fixed contracts.
- Produces: build_scene_generation(snapshot, GenerationKey) -> SceneGenerationData and deterministic template/content/frame checksums.

- [ ] **Step 1: Write failing full-fixture tests**

~~~rust
#[test]
fn full_fixture_builds_stable_shallow_hierarchy_with_real_depth() {
    let snapshot = CompanionSceneSnapshot::fixture_full();
    let built = build_scene_generation(&snapshot, GenerationKey::fixture()).unwrap();
    assert_eq!(built.template.node("scene.root").unwrap().parent, None);
    assert!(built.template.node("pet.body").unwrap().base.translation[2].is_finite());
    assert!(built.template.node("chrome.screen").unwrap().screen_space);
    assert!(built.template.attachments.iter().any(|a| a.alias == "world.prop.treasure_chest.bubble_origin"));
    assert_eq!(built.template.checksum, build_scene_generation(&snapshot, GenerationKey::fixture()).unwrap().template.checksum);
}

#[test]
fn every_species_stage_uses_the_fixed_130_slot_lattice() {
    for snapshot in CompanionSceneSnapshot::fixture_species_stage_matrix() {
        let built = build_scene_generation(&snapshot, GenerationKey::fixture()).unwrap();
        assert_eq!(built.content.pet_art_slots.len(), MAX_PET_ART_SLOTS);
    }
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test presentation::companion_scene::scene::tests::full_fixture
~~~

Expected: compile failure for missing builder.

- [ ] **Step 3: Build fixed nodes and typed primitives**

Create scene.root, world.far, world.behind, pet, world.foreground, and chrome.screen. Add meaningful leaf nodes for room groups, each visible prop/tank inhabitant, pet body/shadow/projection/aura, ambient group, HUD/gauges/trouble/dim. Assign authored Z planes and explicit DepthCue; world Z never changes scale/Y/opacity implicitly.

- [ ] **Step 4: Build fixed semantic and frame slots**

Use exactly 130 pet slots with empty entries for unused cells, 10 prop slots, 2 tank slots, 64 ambient slots, 2 lights, and 32 attachment slots. Build current open/closed chest sprite as an unlit atlas-quad group in this task; ShallowCard remains unused until Task 18.

- [ ] **Step 5: Validate and serialize deterministic checksums**

Hash canonical template/content/frame artifacts, excluding private dense indices and runtime addresses. Every species/stage/state/full-habitat fixture must validate within fixed capacity.

- [ ] **Step 6: Run and commit**

~~~bash
cargo test presentation::companion_scene
cargo test --test companion_scene_boundary
cargo test --features dev-preview dev_preview::scenarios
git add src/presentation/companion_scene/input.rs src/presentation/companion_scene/scene.rs src/presentation/companion_scene/validate.rs
git commit -m "feat(companion): build the direct unlit scene"
~~~

---

### Task 6: Introduce the renderer-neutral capture snapshot

**Files:**
- Modify: src/presentation/companion_scene/contract.rs
- Modify: src/companion/paired_review.rs
- Modify: src/companion/review_capture.rs
- Modify: src/companion/retained/capture.rs
- Modify: src/companion/app.rs

**Interfaces:**
- Consumes: SceneVersion, privacy projection, surface metadata, scene metrics, existing GPU readback.
- Produces: CompanionCaptureSnapshot in the neutral contract and renderer-specific Smooth/scene evidence beneath it.

- [ ] **Step 1: Write failing neutral capture tests**

~~~rust
#[test]
fn capture_identity_binds_one_scene_version() {
    let capture = CompanionCaptureSnapshot::fixture();
    assert_eq!(capture.requested_version, capture.readback_version);
    assert_eq!(capture.privacy.surface, PresentationSurface::RoundCompanion);
}

#[test]
fn capture_during_swap_defers_instead_of_mixing_versions() {
    let active = SceneVersion::fixture(7, 11, 19);
    let pending = SceneVersion::fixture(8, 12, 20);
    assert_eq!(bind_capture_version(active, Some(pending), CaptureTiming::DuringActivation), CaptureBinding::Defer);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test companion::paired_review::tests::capture_identity
~~~

Expected: compile failure for missing neutral capture types.

- [ ] **Step 3: Implement neutral ownership**

~~~rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CompanionCaptureSnapshot {
    pub schema_version: u16,
    pub requested_version: SceneVersion,
    pub readback_version: SceneVersion,
    pub logical_state_alias: CompanionCaptureStateAlias,
    pub privacy: PrivacyProjection,
    pub surface: CaptureSurfaceArtifact,
    pub metrics: CompanionSceneMetricsArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CompanionCaptureStateAlias {
    Normal,
    Active,
    Asleep,
    HelperTrouble,
    Dim,
    Fault,
}

pub enum CaptureBinding {
    Active(SceneVersion),
    Defer,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptureSurfaceArtifact {
    pub logical_points: [u16; 2],
    pub physical_pixels: [u32; 2],
    pub backing_scale: f32,
    pub format: String,
    pub color_space: String,
    pub alpha_mode: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CompanionSceneMetricsArtifact {
    pub schema_version: u16,
    pub node_high_water: u32,
    pub primitive_high_water: u32,
    pub blended_draw_high_water: u32,
    pub persistent_gpu_objects_created: u64,
    pub static_upload_bytes: u64,
    pub content_write_bytes: u64,
    pub frame_write_bytes: u64,
}
~~~

This type stays under presentation::companion_scene. retained/capture.rs owns only GPU readback and renderer evidence.

- [ ] **Step 4: Adapt temporary paired review**

Derive both Smooth and scene evidence from the same CompanionSceneSnapshot. Keep the existing Smooth-plan capture path for rollout evidence, but remove Smooth types from capture request identity and privacy metadata. A capture requested during Activating binds the previous active version or defers; it never mixes generations.

- [ ] **Step 5: Preserve fallback acknowledgement**

On retained scene failure, cold-build Smooth from the latest domain state. Keep FallbackPending until drawRect actually paints and acknowledge_smooth_paint promotes it to FallbackPainted. Do not keep a Smooth plan warm on healthy scene ticks.

- [ ] **Step 6: Run and commit**

~~~bash
cargo test --features retained-renderer companion::paired_review
cargo test --features retained-renderer companion::retained::capture
cargo test --test retained_renderer_boundary
cargo xtask companion review-pair --size 360 --state normal --out target/glorp-review/neutral-capture
git add src/presentation/companion_scene/contract.rs src/companion/paired_review.rs src/companion/review_capture.rs src/companion/retained/capture.rs src/companion/app.rs
git commit -m "refactor(companion): neutralize capture identity"
~~~

---

### Post-Task-6 prerequisite: close the raster-worker UI-safety gate

This prerequisite executes after Task 6 and before Task 7. It is not deferred to
Task 12: Tasks 2-6 may proceed while the bounded `stage0-appkit-raster-v1`
disposition remains in effect, but Task 7 is blocked until this evidence-amended
section passes.

- [ ] Add failing tests proving one dedicated serial `NSThread` worker produces
      a byte-for-byte identical full atlas while every Cocoa object stays
      worker-local and only Rust pixels/entries cross bounded mailboxes. Require
      an unwind-safe current-graphics-context guard and an Objective-C exception
      boundary that terminates the worker without unwinding through Rust.
- [ ] Add one-running/one-latest-pending lifecycle coverage for cancellation
      between glyphs, hidden cancellation/reveal restart, stale rejection before
      GPU materialization, disconnect/panic failure, and shutdown.
- [ ] Replace production UI raster scheduling with nonblocking enqueue/poll/state
      transition work. Require zero main-thread raster calls, generation-service
      UI max <=4000 us, and render-owner-only GPU materialization/upload/publish
      max <=16000 us. Report worker time separately.
- [ ] Run the clean 120-second Task 1 baseline protocol and require
      the raster-worker UI-safety, parity, lifecycle, materialization, hidden,
      memory, and frozen UI/encode gates to pass. The old Stage-0 exception is
      not sufficient to unlock Task 7.
- [ ] Commit the implementation and refreshed measurement before starting Task 7.

---

### Task 7: Split retained host ownership without changing rendering

**Files:**
- Create: src/companion/retained/host.rs
- Create: src/companion/retained/buffers.rs
- Modify: src/companion/retained.rs
- Modify: src/companion/mod.rs
- Modify: tests/retained_renderer_boundary.rs

**Interfaces:**
- Consumes: current PreparedRetainedHost, ActiveRetainedHost, RetainedHost, layer rollback guard, resize, legacy PersistentFrameBuffers.
- Produces: the same public retained API with host/layer/surface and buffer ownership moved behind focused modules.

- [ ] **Step 1: Extend boundary tests before moving code**

Assert host.rs is the only retained module allowed to name CAMetalLayer, Surface, Adapter, Device, Queue, and MainThreadMarker. Assert buffers.rs contains no AppKit/objc2 types. Keep the renderer_spike and AppKit view-cache bans.

- [ ] **Step 2: Run boundary tests and verify failure**

~~~bash
cargo test --test retained_renderer_boundary
~~~

Expected: failure because host.rs and buffers.rs do not exist.

- [ ] **Step 3: Move host code mechanically**

Move PreparedRetainedHost, ActiveRetainedHost, RetainedHost, LayerActivationState, ActivationRollback, LayerActivationGuard, physical_dimension, resize_if_needed, surface configuration, GPU mailbox installation, and layer restoration into host.rs. Preserve function bodies and ordering in this task.

- [ ] **Step 4: Move legacy persistent buffers mechanically**

Move PersistentFrameBuffers and PersistentCaptureResources into buffers.rs. Keep GpuPrimitive and legacy prepare_gpu_frame in retained.rs until Task 17. Re-export only the narrow crate-private constructors/methods retained.rs currently needs.

- [ ] **Step 5: Verify no behavior drift**

~~~bash
cargo fmt --check
cargo test --features retained-renderer companion::retained
cargo test --test retained_renderer_boundary
cargo test --test companion_draw_boundary
cargo xtask companion review-pair --size 360 --state normal --out target/glorp-review/host-split
git diff --check
~~~

Expected: capture succeeds with the same frame checksum and no fallback; boundary tests pass.

- [ ] **Step 6: Commit**

~~~bash
git add src/companion/mod.rs src/companion/retained.rs src/companion/retained/host.rs src/companion/retained/buffers.rs tests/retained_renderer_boundary.rs
git commit -m "refactor(renderer): isolate retained host ownership"
~~~

---

### Task 8: Compile fixed CPU generations and persistent mirrors

**Files:**
- Create: src/companion/retained/compiler.rs
- Modify: src/companion/retained/buffers.rs
- Modify: src/companion/retained.rs
- Test: src/companion/retained/compiler.rs
- Test: src/companion/retained/buffers.rs

**Interfaces:**
- Consumes: SceneGenerationData, fixed capacities, GenerationKey, AppliedRevisions.
- Produces: CpuSceneCandidate, DenseSceneIndex, StaticVertex, StaticIndex, NodeGpuValue, ContentGpuValue, FrameGpuValue, DirtySpanSet, and apply_content_delta/apply_frame_delta.

- [ ] **Step 1: Write failing dense-map and dirty-span tests**

~~~rust
#[test]
fn compiler_maps_semantic_ids_to_dense_offsets_deterministically() {
    let generation = SceneGenerationData::fixture_full();
    let a = compile_cpu_generation(&generation).unwrap();
    let b = compile_cpu_generation(&generation).unwrap();
    assert_eq!(a.static_checksum, b.static_checksum);
    assert_eq!(a.index.node_offset(NodeId::from_alias("pet.body")), b.index.node_offset(NodeId::from_alias("pet.body")));
}

#[test]
fn adjacent_dirty_slots_coalesce_without_growth() {
    let mut mirror = PersistentContentMirror::fixture(8);
    let capacity = mirror.capacity();
    let spans = mirror.apply(&[
        ContentSlotDelta::fixture(2, 10),
        ContentSlotDelta::fixture(3, 11),
        ContentSlotDelta::fixture(6, 12),
    ]).unwrap();
    assert_eq!(spans.as_slice(), &[ByteSpan::slots(2, 2), ByteSpan::slots(6, 1)]);
    assert_eq!(mirror.capacity(), capacity);
}

#[test]
fn capacity_overflow_requests_generation_instead_of_reallocating() {
    let mut mirror = PersistentContentMirror::fixture(2);
    assert_eq!(mirror.apply(&[ContentSlotDelta::fixture(2, 1)]), Err(MirrorError::CapacityExceeded));
    assert_eq!(mirror.capacity(), 2);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::compiler
cargo test --features retained-renderer companion::retained::buffers
~~~

Expected: compile failure for missing compiler/mirror types.

- [ ] **Step 3: Define fixed POD layouts**

Use bytemuck Pod/Zeroable repr(C) structs. NodeGpuValue contains a 4x4 world matrix plus opacity/visibility/material parameter indices. Frame globals contain view/projection, viewport points/pixels, aperture, dim, and two lights. Static vertices carry local XYZ, UV, normal, primitive/material indices. Assert size/alignment in tests and keep WGSL layouts byte-identical.

- [ ] **Step 4: Compile immutable geometry and dense indices**

Validate before compile. Build immutable vertex/index arrays, semantic NodeId-to-dense index, typed content/frame slot maps, opaque batch templates, blended draw-record templates, and attachment owner/index maps. Dense indices never appear in neutral artifacts.

- [ ] **Step 5: Build fixed mirrors and dirty coalescing**

Allocate exact declared capacities once in CpuSceneCandidate. Delta application updates POD CPU mirrors, tracks changed semantic slots, and coalesces adjacent byte ranges into fixed [ByteSpan; 64] storage. Return CapacityExceeded rather than growing a Vec or span list.

- [ ] **Step 6: Add the 300-frame CPU lifetime smoke**

Build CompanionSceneSnapshot::fixture_full, reconcile/compile once, then drive 300 ordinary frames. Assert zero generation requests, unchanged static checksum, unchanged capacities, and only expected frame/content spans.

- [ ] **Step 7: Run and commit**

~~~bash
cargo test --features retained-renderer companion::retained::compiler
cargo test --features retained-renderer companion::retained::buffers
cargo test --features retained-renderer companion::retained::tests::cpu_scene_lifetime
cargo clippy --lib --features retained-renderer -- -D warnings
git add src/companion/retained.rs src/companion/retained/compiler.rs src/companion/retained/buffers.rs
git commit -m "feat(renderer): compile fixed companion scene mirrors"
~~~

---

### Task 9: Materialize the sRGB/depth/intermediate GPU generation

**Files:**
- Create: src/companion/retained/render.rs
- Create: src/companion/retained/scene.wgsl
- Modify: src/companion/retained/host.rs
- Modify: src/companion/retained/resources.rs
- Modify: src/companion/retained/buffers.rs
- Modify: src/companion/retained.rs
- Test: src/companion/retained/render.rs
- Test: src/companion/retained/parity.rs

**Interfaces:**
- Consumes: CpuSceneCandidate and render-owner Device/Queue/Surface.
- Produces: SceneSurfaceContract, GpuSceneCandidate, SceneTargets, ScenePipelines, materialize_gpu_candidate, and exact linear/alpha conversion helpers.

- [ ] **Step 1: Write failing surface/color/depth tests**

~~~rust
#[test]
fn metal_scene_surface_requires_srgb_and_postmultiplied() {
    let caps = SurfaceCapabilitiesFixture::metal_wgpu_30();
    let contract = SceneSurfaceContract::select(&caps).unwrap();
    assert_eq!(contract.format, wgpu::TextureFormat::Bgra8UnormSrgb);
    assert_eq!(contract.color_space, wgpu::SurfaceColorSpace::Srgb);
    assert_eq!(contract.alpha_mode, wgpu::CompositeAlphaMode::PostMultiplied);
}

#[test]
fn unsupported_alpha_contract_refuses_scene_runtime() {
    let caps = SurfaceCapabilitiesFixture::opaque_only();
    assert_eq!(SceneSurfaceContract::select(&caps), Err(SceneGpuError::UnsupportedSurfaceContract));
}

#[test]
fn final_pass_round_trips_premultiplied_linear_pixel() {
    let stored_srgb_premul = encode_premultiplied_linear([0.20, 0.10, 0.05, 0.25]);
    let straight = final_surface_straight_linear(stored_srgb_premul);
    assert_channels_close(straight, [0.80, 0.40, 0.20, 0.25], 0.0005);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::render
cargo test --features retained-renderer companion::retained::parity
~~~

Expected: compile failure for missing scene render contract.

- [ ] **Step 3: Select the exact surface contract**

Query format capabilities. Require Bgra8UnormSrgb + Srgb + PostMultiplied. Do not call remove_srgb_suffix. When unavailable, return UnsupportedSurfaceContract before installing the scene path. Keep the legacy surface configuration reachable only for rollout translation until Task 17.

- [ ] **Step 4: Correct atlas representations**

Coverage atlas entries use R8Unorm. Color atlas entries use straight-alpha Rgba8UnormSrgb. Convert AppKit premultiplied-sRGB pixels to straight sRGB with a zero-alpha guard, add per-entry padding, and edge-dilate RGB into alpha-zero gutters. Sampling decodes sRGB; shaders premultiply in linear light.

- [ ] **Step 5: Materialize on render owner only**

Create immutable geometry/index buffers, node/content/frame buffers, atlas textures/samplers/bind groups, Depth24Plus attachment, Bgra8UnormSrgb premultiplied intermediate, and pipelines. Key depth/intermediate resources by DeviceEpoch, SurfaceEpoch, extent, format, and sample count. No worker may receive Device or Queue.

- [ ] **Step 6: Implement scene.wgsl base and final pass**

The scene vertex shader applies world, view, and projection matrices and emits WebGPU clip Z. Opaque/cutout pipelines use LessEqual with depth writes. Blended pipelines use LessEqual without writes. Screen chrome has no depth. World/chrome output premultiplied-linear color to the sRGB intermediate. The final pass samples/decodes, divides RGB by alpha when alpha > 0, writes zero RGB for zero alpha, and emits straight-linear color to the PostMultiplied surface.

- [ ] **Step 7: Verify resource lifetime**

Materialize once, resize with unchanged extent, then resize to a new extent. Assert the first path creates no resources, the second recreates only surface-dependent targets, and logical/frame revisions recreate neither.

- [ ] **Step 8: Run and commit**

~~~bash
cargo test --features retained-renderer companion::retained::render
cargo test --features retained-renderer companion::retained::resources
cargo test --features retained-renderer companion::retained::parity
cargo test --test retained_renderer_boundary
cargo clippy --lib --features retained-renderer -- -D warnings
git add src/companion/retained.rs src/companion/retained/host.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl src/companion/retained/resources.rs src/companion/retained/buffers.rs src/companion/retained/parity.rs
git commit -m "feat(renderer): materialize linear depth scene resources"
~~~

---

### Task 10: Render the complete unlit companion offscreen

**Files:**
- Modify: src/companion/retained/render.rs
- Modify: src/companion/retained/scene.wgsl
- Modify: src/companion/retained/capture.rs
- Modify: src/companion/retained.rs
- Create: tests/retained_scene.rs

**Interfaces:**
- Consumes: GpuSceneCandidate, fixed mirrors, camera/depth contract, existing GPU readback.
- Produces: SceneRenderer::render_offscreen, SceneRenderRequest, ScenePresentOutcome, and canonical premultiplied-intermediate capture.

- [ ] **Step 1: Write failing offscreen tests**

~~~rust
#[test]
fn opaque_world_elements_occlude_by_z_across_semantic_categories() {
    let frame = render_fixture(SceneFixture::OpaqueCrossCategory).unwrap();
    assert_eq!(frame.pixel(180, 180), EXPECTED_NEAR_PROP_RGBA);
}

#[test]
fn screen_chrome_ignores_world_depth_and_lighting() {
    let frame = render_fixture(SceneFixture::ChromeOverNearWorld).unwrap();
    assert_eq!(frame.pixel(180, 330), EXPECTED_HUD_RGBA);
}

#[test]
fn depth_attachment_reuses_at_fixed_extent() {
    let mut renderer = OffscreenSceneRenderer::fixture();
    renderer.render(SceneFixture::Normal).unwrap();
    let first = renderer.metrics();
    renderer.render(SceneFixture::NearPet).unwrap();
    let second = renderer.metrics();
    assert_eq!(second.depth_attachment_creations - first.depth_attachment_creations, 0);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer --test retained_scene offscreen
~~~

Expected: compile failure for missing offscreen scene renderer.

- [ ] **Step 3: Encode fixed opaque and chrome batches**

Apply dirty CPU mirror spans through Queue::write_buffer, bind immutable geometry/resources, encode opaque/cutout, ordered blend placeholder, chrome, and final surface/capture pass. Offscreen mode targets the persistent intermediate and capture staging without acquiring a surface.

- [ ] **Step 4: Implement canonical capture normalization**

Read the premultiplied intermediate. For each pixel: sRGB-decode RGB, unpremultiply in linear light with a zero-alpha guard, then sRGB-encode straight RGB for PNG. Lock opaque, translucent-edge, zero-alpha, and high-alpha samples.

- [ ] **Step 5: Render the full matrix**

Cover normal, active, asleep, helper trouble, dim, all species/stages, full props/tank, far/neutral/near, resize 260/360/480/720, and 1x/2x backing scale. Assert nonblank output, finite metrics, valid depth, and privacy-safe artifacts.

- [ ] **Step 6: Run and commit**

~~~bash
cargo test --features retained-renderer --test retained_scene offscreen
cargo test --features retained-renderer companion::retained::capture
cargo test --features retained-renderer companion::retained::render
git add src/companion/retained.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl src/companion/retained/capture.rs tests/retained_scene.rs
git commit -m "feat(renderer): render the direct companion scene offscreen"
~~~

---

### Task 11: Implement one ordered blended-world stream

**Files:**
- Modify: src/companion/retained/compiler.rs
- Modify: src/companion/retained/buffers.rs
- Modify: src/companion/retained/render.rs
- Modify: src/companion/retained/scene.wgsl
- Modify: tests/retained_scene.rs

**Interfaces:**
- Consumes: immutable BlendedDrawTemplate records and current world/camera matrices.
- Produces: BlendedDrawKey, PersistentBlendOrder, update_blended_order, and contiguous compatible draw runs.

- [ ] **Step 1: Write failing blend-order tests**

~~~rust
#[test]
fn blend_modes_share_camera_depth_order() {
    let order = sort_blended_fixture([
        record("alpha-near", WorldBlend::PremultipliedAlpha, 0.8),
        record("multiply-mid", WorldBlend::Multiply, 0.0),
        record("additive-far", WorldBlend::Additive, -0.7),
    ]);
    assert_eq!(order.aliases(), ["additive-far", "multiply-mid", "alpha-near"]);
}

#[test]
fn batching_merges_only_adjacent_compatible_records() {
    let runs = compile_runs([
        record("a", WorldBlend::PremultipliedAlpha, -0.8),
        record("b", WorldBlend::Multiply, -0.2),
        record("c", WorldBlend::PremultipliedAlpha, 0.4),
    ]);
    assert_eq!(runs.len(), 3);
}

#[test]
fn crossing_dynamic_instances_reorder_without_allocation() {
    let mut order = PersistentBlendOrder::fixture(8);
    let capacity = order.capacity();
    order.update(&crossing_fixture(0.25)).unwrap();
    let before = order.aliases();
    order.update(&crossing_fixture(0.75)).unwrap();
    assert_ne!(order.aliases(), before);
    assert_eq!(order.capacity(), capacity);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer --test retained_scene blended
~~~

Expected: compile failure for missing blended stream.

- [ ] **Step 3: Implement fixed stable sorting**

Use a fixed [BlendedDrawRecord; 256] mirror and [u16; 256] sort indices. Compute camera-space depth from current world matrix plus stable semantic tie-breaker. Stable insertion sort is acceptable at this bound; perform no heap allocation. Reuse order when neither camera nor blended-node depth changed.

- [ ] **Step 4: Implement blend equations**

Premultiplied source-over uses color/alpha One, OneMinusSrcAlpha. Multiply uses color Dst, OneMinusSrcAlpha and alpha One, OneMinusSrcAlpha. Additive uses color One, One and alpha One, OneMinusSrcAlpha. All three remain in the shared depth order. Permit an unsorted additive pass only for explicitly screen-local material, not world content.

- [ ] **Step 5: Render crossing fixtures**

Render alpha/multiply/additive crossings, transparent sprite gutters, particle crossings, static/dynamic overlap, and camera movement. Compare locked pixel samples and ordered-record artifacts.

- [ ] **Step 6: Run and commit**

~~~bash
cargo test --features retained-renderer --test retained_scene blended
cargo test --features retained-renderer companion::retained::compiler
cargo test --features retained-renderer companion::retained::buffers
git add src/companion/retained/compiler.rs src/companion/retained/buffers.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl tests/retained_scene.rs
git commit -m "feat(renderer): order blended world content by depth"
~~~

---

### Task 12: Wire worker preparation and atomic GPU activation

**Files:**
- Modify: src/presentation/companion_scene/runtime.rs
- Modify: src/companion/retained/render.rs
- Modify: src/companion/retained/host.rs
- Modify: src/companion/retained/presentation.rs
- Modify: src/companion/app.rs
- Modify: tests/retained_scene.rs

**Interfaces:**
- Consumes: GenerationRequest, CPU compiler, the completed post-Task-6 raster-worker proof, render-owner materialization, FrameProgress/GpuErrorMailbox.
- Produces: SceneBuildWorker, CpuCandidateMailbox, ActiveSceneGeneration, ReadyGpuCandidate, and activate_candidate.

- [ ] **Step 1: Write failing atomic-lifecycle tests**

~~~rust
#[test]
fn stale_worker_result_is_destroyed_before_gpu_materialization() {
    let mut harness = SceneLifecycleHarness::active();
    let old = harness.request_generation(Stage::S3);
    let newest = harness.request_generation(Stage::S4);
    harness.complete_cpu(old);
    assert_eq!(harness.gpu_materialization_count(), 0);
    harness.complete_cpu(newest);
    assert_eq!(harness.gpu_materialization_count(), 1);
}

#[test]
fn failed_first_present_retains_previous_generation() {
    let mut harness = SceneLifecycleHarness::active();
    let previous = harness.active_checksum();
    harness.prepare_candidate();
    harness.inject(SceneFault::EncodeBeforeSubmit);
    assert_eq!(harness.activate(), ScenePresentOutcome::FailedRetainingActive);
    assert_eq!(harness.active_checksum(), previous);
}

#[test]
fn delayed_gpu_error_invalidates_device_epoch_and_falls_back() {
    let mut harness = SceneLifecycleHarness::active();
    harness.prepare_and_present_candidate();
    harness.inject(SceneFault::DelayedDeviceValidation);
    assert_eq!(harness.next_tick(), ScenePresentOutcome::FallbackPending);
    assert!(harness.device_epoch_invalidated());
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer --test retained_scene lifecycle
~~~

Expected: compile failure for missing lifecycle harness/runtime wiring.

- [ ] **Step 3: Implement one coalesced CPU worker**

The UI thread sends immutable GenerationRequest values through a one-active/one-pending owner. The worker checks cancellation between validation, geometry compile, atlas packing, worker-local offscreen rasterization, and mirror creation. It returns CpuSceneCandidate plus request ID/generation/source revisions. It never receives Device, Queue, Surface, Cocoa objects, or live AppState references; it creates and destroys any offscreen raster Cocoa objects locally.

- [ ] **Step 4: Integrate the evidence-gated raster worker**

Fold scene-generation raster work into the same dedicated serial
`SceneBuildWorker` from Step 3; do not create a second worker, nested mailbox, or
second coalescer. The worker
creates and destroys all offscreen Cocoa objects locally under autorelease pools
and returns only owned Rust atlas pixels, entries, identities, and timings. The
existing UI-owned one-running/one-latest-pending lifecycle checks cancellation
between glyphs, polls without blocking, and rejects stale results before any GPU
materialization. Verify the active generation remains visible
while work is pending, main-thread raster call count is exactly zero, generation
service UI work remains <=4000 us max, and Task 12 activation uploads completed
raster output only on the render owner.

- [ ] **Step 5: Materialize and activate atomically**

On render owner: reject stale CPU candidates, materialize complete GPU resources, rebase latest compatible deltas, retain previous during Activating, acquire/encode/submit/present candidate, drain immediate GPU mailbox, then commit at SurfacePresentCalled. Outdated reconfigures once and retries later; Timeout/Occluded defer; SurfaceLost/validation/device/OOM fallback; delayed device errors invalidate DeviceEpoch.

- [ ] **Step 6: Exercise full fault timing matrix**

Cover newer snapshot during prepare/ready/activate, resize/scale during preparation, capture before/during/after swap, hidden topology update, shutdown cancellation, candidate resource failure, encode failure, surface states, and delayed mailbox errors. Assert active checksum/resource counts/disposition after every transition.

- [ ] **Step 7: Run and commit**

~~~bash
cargo test --features retained-renderer --test retained_scene lifecycle
cargo test presentation::companion_scene::runtime
cargo test --features retained-renderer companion::retained::presentation
cargo clippy --all-targets --features retained-renderer -- -D warnings
git add src/presentation/companion_scene/runtime.rs src/companion/retained/render.rs src/companion/retained/host.rs src/companion/retained/presentation.rs src/companion/app.rs tests/retained_scene.rs
git commit -m "feat(renderer): activate scene generations atomically"
~~~

---

### Task 13: Export deterministic scene artifacts and receive native depth/color approval

**Files:**
- Modify: src/dev_preview/contract.rs
- Modify: src/dev_preview/export.rs
- Modify: src/dev_preview/round.rs
- Modify: src/dev_preview/scenarios.rs
- Modify: tests/dev_preview.rs
- Modify: tests/retained_scene.rs
- Modify: xtask/src/lib.rs
- Create: docs/superpowers/measurements/2026-07-11-glorp-companion-scene-native-review.md

**Interfaces:**
- Consumes: neutral snapshot, SceneGenerationData, CpuSceneCandidate, offscreen GPU renderer/capture.
- Produces: schema-versioned snapshot/generation/content/frame/batches/resources/metrics/readback artifacts and native visual review record.

- [ ] **Step 1: Write failing manifest/artifact tests**

~~~rust
#[test]
fn round_scene_runtime_fixture_lists_all_typed_artifacts() {
    let run = PreviewRun::new();
    run.run_success("round");
    let scenario = run.scenario("round-scene-runtime-normal");
    for suffix in [
        "companion-snapshot.json",
        "companion-generation.json",
        "companion-content.json",
        "companion-frame.json",
        "companion-batches.json",
        "companion-resources.json",
        "companion-metrics.json",
        "companion-readback.png",
    ] {
        assert!(scenario.artifacts.iter().any(|artifact| artifact.path.ends_with(suffix)), "missing {suffix}");
    }
}

#[test]
fn scene_artifacts_are_privacy_safe_and_use_schema_one() {
    let run = PreviewRun::new();
    run.run_success("round");
    for path in run.scene_artifact_paths() {
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("schema_version"));
        for forbidden in ["very-secret-seed", "/Users/", "prompt", "response", "transcript", "claude", "codex"] {
            assert!(!text.to_lowercase().contains(&forbidden.to_lowercase()));
        }
    }
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features "dev-preview retained-renderer" --test dev_preview round_scene_runtime
~~~

Expected: failure because scene artifacts/fixture are absent.

- [ ] **Step 3: Add artifact DTO conversion and paths**

Export snapshot, generation, content, frame, compiled batches, resources, metrics, and canonical readback with schema_version 1. Use stable semantic aliases, never dense indices or user-derived identifiers. Add all files to manifest artifact inventory and review.md links.

- [ ] **Step 4: Add the deterministic review matrix**

Add normal, active, asleep, helper-trouble, dim, every species/stage max lattice, full props/tank, far/neutral/near, opaque cross-category occlusion, alpha/multiply/additive crossing, transparent edges, resize and backing-scale independently/together, capture-before/during/after-swap, and every generation/fault fixture.

- [ ] **Step 5: Add native review command**

Extend xtask with cargo xtask companion scene-review --size 260|360|480|720 --scale 1|2 --state STATE --out DIR. It builds release retained-renderer + dev-preview, renders the new scene path, validates all typed artifacts/readback, and fails on fallback, blank pixels, version mismatch, privacy match, or resource growth.

- [ ] **Step 6: Run automated matrix**

~~~bash
cargo test --features "dev-preview retained-renderer" --test dev_preview
cargo test --features retained-renderer --test retained_scene
cargo run --features "dev-preview retained-renderer" -- dev-preview --scenario round --out target/glorp-preview-scene-runtime
cargo xtask companion scene-review --size 360 --scale 2 --state normal --out target/glorp-review/scene-native-360
~~~

Expected: all pass; target/glorp-preview-scene-runtime/index.html contains the complete review matrix.

- [ ] **Step 7: Perform and record native visual approval**

Review real 360x360 Metal output plus 260/480/720, far/neutral/near, blend crossings, transparent edges, dim, helper trouble, resize, 1x simulated capture, 2x native capture, and capture during swap. Record exact commands, commit, hardware/OS, artifact paths, accepted/rejected observations, and fixes in the measurement file. Stop for any incorrect depth, halo, HUD, aperture, alpha edge, color, or resize result.

- [ ] **Step 8: Commit**

~~~bash
git add src/dev_preview/contract.rs src/dev_preview/export.rs src/dev_preview/round.rs src/dev_preview/scenarios.rs tests/dev_preview.rs tests/retained_scene.rs xtask/src/lib.rs docs/superpowers/measurements/2026-07-11-glorp-companion-scene-native-review.md
git commit -m "test(renderer): qualify companion scene evidence"
~~~

---

### Task 14: Shadow production snapshots and add an explicit live opt-in

**Files:**
- Modify: src/commands/companion_mode.rs
- Modify: src/cli.rs
- Modify: src/commands/companion_app.rs
- Modify: src/companion/app.rs
- Modify: src/companion/retained.rs
- Modify: src/companion/retained/host.rs
- Modify: tests/cli_smoke.rs
- Modify: tests/companion_draw_boundary.rs
- Modify: tests/retained_scene.rs

**Interfaces:**
- Consumes: accepted offscreen renderer, atomic lifecycle, neutral snapshot/capture.
- Produces: deletion-bound SceneRuntimeRollout { Off, Shadow, Live }, hidden review flag, shadow metrics, and opt-in live presentation.

- [ ] **Step 1: Write failing rollout-policy tests**

~~~rust
#[test]
fn scene_runtime_rollout_defaults_to_shadow_for_explicit_retained_review_only() {
    assert_eq!(resolve_scene_rollout(false, false), SceneRuntimeRollout::Off);
    assert_eq!(resolve_scene_rollout(true, false), SceneRuntimeRollout::Shadow);
    assert_eq!(resolve_scene_rollout(true, true), SceneRuntimeRollout::Live);
}

#[test]
fn one_line_scene_rollback_disables_auto_live_path() {
    assert!(!AUTO_SCENE_RUNTIME_ON_APPLE_SILICON);
    assert_eq!(resolve_scene_rollout(true, AUTO_SCENE_RUNTIME_ON_APPLE_SILICON), SceneRuntimeRollout::Shadow);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer commands::companion_mode::tests::scene_runtime_rollout
~~~

Expected: compile failure for missing rollout policy.

- [ ] **Step 3: Add deletion-bound policy**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SceneRuntimeRollout { Off, Shadow, Live }

pub const AUTO_SCENE_RUNTIME_ON_APPLE_SILICON: bool = false;
~~~

Expose a hidden dev/review --retained-scene-runtime off|shadow|live option only when retained-renderer is compiled. This is not a new public renderer backend. Retained remains the effective renderer.

- [ ] **Step 4: Drive one snapshot from ui_tick**

On each visible retained tick, capture one immutable snapshot. Shadow reconciles/builds and records metrics but does not materialize or present every tick; it materializes only bounded review candidates. Live routes the active scene generation to present. Hidden state coalesces newest snapshot and does no prepare/write/acquire/encode/submit work; reveal reconciles once.

- [ ] **Step 5: Keep Smooth cold**

Healthy scene Live ticks do not call derive_round_scene_model, build a Smooth plan, or populate PreparedRendererFrame::Smooth. On technical fallback, build one current Smooth frame from latest domain state, restore AppKit, request fallback, force display, and acknowledge only after paint.

- [ ] **Step 6: Add source and runtime regressions**

Source scans assert live scene preparation contains no Smooth/TUI types and drawRect remains a prepared-frame/fallback-only boundary. Runtime tests assert Shadow never presents, Live presents SceneVersion, hidden steady state is zero work, and fallback cold-builds exactly once.

- [ ] **Step 7: Run shadow and opt-in live gates**

~~~bash
cargo test --features retained-renderer commands::companion_mode
cargo test --features retained-renderer --test retained_scene rollout
cargo test --test companion_draw_boundary
cargo test --test cli_smoke companion
cargo xtask companion scene-review --size 360 --scale 2 --state normal --out target/glorp-review/scene-opt-in
cargo xtask companion fresh
~~~

Launch the fresh bundle once with --retained-scene-runtime shadow and once with live. Expected: shadow shows the shipping renderer while recording bounded scene metrics; live shows the scene renderer with no fallback; normal Auto behavior is unchanged.

- [ ] **Step 8: Commit**

~~~bash
git add src/commands/companion_mode.rs src/cli.rs src/commands/companion_app.rs src/companion/app.rs src/companion/retained.rs src/companion/retained/host.rs tests/cli_smoke.rs tests/companion_draw_boundary.rs tests/retained_scene.rs
git commit -m "feat(companion): add scene runtime shadow and live modes"
~~~

---

### Task 15: Close lifetime, performance, resize, visibility, and fault gates

**Files:**
- Modify: src/companion/retained/metrics.rs
- Modify: src/companion/retained/render.rs
- Modify: src/companion/app.rs
- Modify: src/commands/companion_mode.rs
- Modify: xtask/src/lib.rs
- Modify: tests/retained_scene.rs
- Create: docs/superpowers/measurements/2026-07-11-glorp-companion-scene-runtime-gate.md

**Interfaces:**
- Consumes: frozen Task 1 gates, opt-in live renderer, fault injection, metrics.
- Produces: cargo xtask companion scene-lifetime, scene-fault-soak, and scene-native-smoke plus a complete gate report.

- [ ] **Step 1: Write the 300-frame and 4,500-frame tests**

~~~rust
#[test]
fn production_scene_has_no_post_warmup_churn_for_300_frames() {
    let result = run_virtual_scene(SceneRun::ordinary_frames(300));
    assert_eq!(result.template_builds_after_warmup, 0);
    assert_eq!(result.static_upload_bytes_after_warmup, 0);
    assert_eq!(result.persistent_gpu_creations_after_warmup, 0);
    assert_eq!(result.resource_misses, 0);
    assert_eq!(result.static_checksum_changes, 0);
}

#[test]
fn production_scene_survives_4500_virtual_frames_and_lifecycle_boundaries() {
    let result = run_virtual_scene(SceneRun::extended_with_poll_minute_content_and_topology());
    assert_eq!(result.frames, 4_500);
    assert_eq!(result.stale_mutations, 0);
    assert_eq!(result.capacity_growths, 0);
    assert!(result.rss_high_water <= result.warmup_rss_high_water * 101 / 100);
    assert!(result.gpu_bytes_high_water <= result.warmup_gpu_bytes_high_water * 101 / 100);
}
~~~

- [ ] **Step 2: Verify the long virtual tests fail before harness support**

~~~bash
cargo test --features retained-renderer --test retained_scene production_scene
~~~

Expected: compile failure for missing SceneRun harness.

- [ ] **Step 3: Add deterministic virtual-time runner**

Advance 4,500 frames at the current 4 Hz cadence, spanning 18 minutes 45 seconds
of virtual time. Include semantic animation changes, 30-second poll boundaries,
minute changes, content substitutions, one topology replacement, hidden/reveal,
resize/scale storms, and capture around activation. Collect versioned metrics and
compare exact counts/high-water values.

- [ ] **Step 4: Expand fault injection**

Add stale candidate, candidate resource creation, encode-before-submit, Outdated, Timeout, Occluded, SurfaceLost, surface validation, delayed device validation, OOM, capture during swap, map/readback/write failure, cancellation, and shutdown variants. Each maps to exactly one seam and static sanitized category.

- [ ] **Step 5: Add bounded xtask gates**

~~~text
cargo xtask companion scene-lifetime --frames 4500 --out target/glorp-scene-gates/lifetime
cargo xtask companion scene-fault-soak --out target/glorp-scene-gates/faults
cargo xtask companion scene-native-smoke --duration-ms 300000 --out target/glorp-scene-gates/native-five-minute
~~~

scene-native-smoke runs release live scene mode for five real minutes, checks every 10 seconds, records metrics, and fails on fallback, generation churn, persistent-object creation, capacity/resource growth, hidden work, or frozen baseline breach. All xtasks use hard timeout = requested duration + 60 seconds and clean stale output before launch.

- [ ] **Step 6: Run the automated gate**

~~~bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features retained-renderer
cargo test --features "dev-preview retained-renderer" --test dev_preview
cargo test --test round_scene
cargo test --test smooth_companion
cargo test --test companion_draw_boundary
cargo test --test retained_renderer_boundary
cargo test -p xtask
node --test scripts/test/macos-app-packaging.test.mjs
cargo xtask companion scene-lifetime --frames 4500 --out target/glorp-scene-gates/lifetime
cargo xtask companion scene-fault-soak --out target/glorp-scene-gates/faults
cargo xtask companion scene-native-smoke --duration-ms 300000 --out target/glorp-scene-gates/native-five-minute
git diff --check
~~~

Expected: zero failures; every Task 1 numeric gate passes; gate report records exact observed values and artifact paths.

- [ ] **Step 7: Perform accessibility/input/package confirmation**

Confirm keyboard quit/fullscreen, pointer/input behavior, Accessibility Inspector/VoiceOver semantics, hidden/occluded suspension, 260/360/480/720 window behavior, release bundle launch, capability output, and cold Smooth fallback on a native injected fault. Record results in the gate measurement.

- [ ] **Step 8: Commit**

~~~bash
git add src/companion/retained/metrics.rs src/companion/retained/render.rs src/companion/app.rs src/commands/companion_mode.rs xtask/src/lib.rs tests/retained_scene.rs docs/superpowers/measurements/2026-07-11-glorp-companion-scene-runtime-gate.md
git commit -m "test(renderer): close companion scene runtime gates"
~~~

---

### Task 16: Run the Auto canary, four-hour hold, and rollback rehearsal

**Files:**
- Modify: src/commands/companion_mode.rs
- Modify: scripts/test/macos-app-packaging.test.mjs
- Modify: .github/workflows/publish.yml
- Modify: docs/superpowers/measurements/2026-07-11-glorp-companion-scene-runtime-gate.md

**Interfaces:**
- Consumes: green Task 15 gate, native visual approval, one-line scene rollback constant.
- Produces: Auto scene runtime canary on Apple Silicon, proven rollback, four-hour release-candidate hold, explicit Drew approval record.

- [ ] **Step 1: Stop and present the complete gate to Drew**

Present native review, baseline comparison, 300/4,500-frame results, five-minute smoke, fault/resize/visibility results, resource/memory high-water marks, accessibility/input/package status, and rollback command. Do not change Auto without explicit approval.

- [ ] **Step 2: After approval, write the failing Auto policy test**

~~~rust
#[test]
fn apple_silicon_auto_retained_uses_scene_runtime_after_approval() {
    assert!(AUTO_SCENE_RUNTIME_ON_APPLE_SILICON);
    assert_eq!(resolve_scene_rollout(true, AUTO_SCENE_RUNTIME_ON_APPLE_SILICON), SceneRuntimeRollout::Live);
}
~~~

Run:

~~~bash
cargo test --features retained-renderer commands::companion_mode::tests::apple_silicon_auto_retained_uses_scene_runtime_after_approval
~~~

Expected before the flip: FAIL because the constant is false.

- [ ] **Step 3: Flip one line and preserve rollback**

~~~rust
pub const AUTO_SCENE_RUNTIME_ON_APPLE_SILICON: bool = true;
~~~

Rollback is exactly the same line set to false; it routes Retained to the legacy translator during canary. Smooth host fallback remains independent.

- [ ] **Step 4: Update package/capability assertions**

Apple-Silicon Auto must report retained compiled, effective retained, scene-runtime live. Intel remains Smooth-only without retained. Explicit Smooth remains selectable. Explicit Retained on Apple Silicon uses the scene runtime.

- [ ] **Step 5: Rehearse rollback**

Set the constant false, build/run capability tests and a fresh app, verify Auto uses the legacy retained translator while explicit live review remains reachable, then restore true and rerun. Record both commit-tree diffs and observed capability output; do not commit the false rehearsal state.

- [ ] **Step 6: Run the four-hour hold**

Run release Auto for 14,400,000 ms on native Apple Silicon with ordinary work, visibility transitions, one resize/scale sequence, captures, and injected fault only in a separate bounded run:

~~~text
cargo xtask companion scene-native-smoke --duration-ms 14400000 --auto --out target/glorp-scene-gates/auto-four-hour
~~~

Require zero unexplained fallback, zero post-warmup resource growth, no hidden work, no stale mutation, all baseline gates, successful captures, and stable memory high-water. Stop and revert the constant for any breach.

- [ ] **Step 7: Run final canary checks and commit**

~~~bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features retained-renderer commands::companion_mode
cargo test --features retained-renderer --test retained_scene rollout
node --test scripts/test/macos-app-packaging.test.mjs
cargo test -p xtask
git diff --check
git add src/commands/companion_mode.rs scripts/test/macos-app-packaging.test.mjs .github/workflows/publish.yml docs/superpowers/measurements/2026-07-11-glorp-companion-scene-runtime-gate.md
git commit -m "feat(companion): canary direct scene runtime by default"
~~~

---

### Task 17: Delete legacy Smooth-plan-to-GPU translation and rollout scaffolding

**Files:**
- Modify: src/companion/retained.rs
- Delete: src/companion/retained.wgsl
- Modify: src/companion/retained/parity.rs
- Modify: src/companion/retained/capture.rs
- Modify: src/companion/retained/resources.rs
- Modify: src/companion/app.rs
- Modify: src/commands/companion_mode.rs
- Modify: src/cli.rs
- Modify: tests/retained_renderer_boundary.rs
- Modify: tests/companion_draw_boundary.rs
- Modify: tests/retained_scene.rs
- Modify: xtask/src/lib.rs

**Interfaces:**
- Consumes: completed canary hold and accepted direct scene runtime.
- Produces: one retained scene-generation path; Smooth remains only explicit renderer and cold host fallback.

- [ ] **Step 1: Add deletion boundary tests**

~~~rust
#[test]
fn retained_scene_sources_do_not_reference_smooth_plan_or_draw_cells() {
    for path in retained_scene_source_files() {
        let text = read(&path);
        for forbidden in ["SmoothCompanionScenePlan", "SmoothCompanionLayer", "SceneDrawList", "DrawCell", "prepare_gpu_frame"] {
            assert!(!text.contains(forbidden), "{} contains {}", path.display(), forbidden);
        }
    }
}

#[test]
fn retained_no_longer_claims_it_uses_a_smooth_scene() {
    assert!(!EffectiveCompanionRenderer::Retained.uses_smooth_scene());
    assert!(EffectiveCompanionRenderer::Smooth.uses_smooth_scene());
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer --test retained_renderer_boundary
cargo test --features retained-renderer commands::companion_mode::tests::retained_no_longer_claims_it_uses_a_smooth_scene
~~~

Expected: failures identify legacy translator references and Retained Smooth-scene policy.

- [ ] **Step 3: Remove legacy GPU translation**

Delete GpuPrimitive, PreparedGpuFrame, legacy Pipelines, prepare_gpu_frame, push_tank_background, push_mood_aura, push_gauges, push_overlays, push_hud, layer/cell/shape screen-space conversion, legacy draw loop, and retained.wgsl. Remove obsolete gamma-parity helpers/tests; retain or relocate only linear conversion/canonical capture math used by scene.wgsl/resources/capture.

- [ ] **Step 4: Remove rollout shadow/legacy routing**

Delete SceneRuntimeRollout, hidden off/shadow/live flag, AUTO_SCENE_RUNTIME_ON_APPLE_SILICON, shadow comparison work, and legacy retained routing. Retained always uses the direct scene runtime. The existing AUTO_RETAINED_ON_APPLE_SILICON remains the one-line rollback to Smooth.

- [ ] **Step 5: Make Smooth cold and independent**

Effective Retained no longer calls prepare_smooth_view_model_for_tick, derive_round_scene_model, or builds PreparedRendererFrame::Smooth. Explicit Smooth still does. Technical fallback builds the current Smooth frame after failure and preserves pending-to-painted acknowledgement.

- [ ] **Step 6: Update capture and xtask**

Paired review launches explicit Smooth and explicit Retained scene runs from the same neutral snapshot; it no longer translates one Smooth plan into GPU primitives. Remove shadow-only manifest fields/commands. Keep GPU-native scene capture and privacy validation.

- [ ] **Step 7: Run translator-deletion gate**

~~~bash
rg -n "prepare_gpu_frame|SmoothCompanionScenePlan|SmoothCompanionLayer|SceneDrawList|DrawCell" src/companion/retained.rs src/companion/retained
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features retained-renderer
cargo test --features "dev-preview retained-renderer" --test dev_preview
cargo test --test companion_draw_boundary
cargo test --test retained_renderer_boundary
cargo test -p xtask
cargo xtask companion scene-review --size 360 --scale 2 --state normal --out target/glorp-review/post-translator
node --test scripts/test/macos-app-packaging.test.mjs
git diff --check
~~~

Expected: rg returns no forbidden reference in retained scene sources; all tests/capture/package checks pass; injected scene fault still paints Smooth fallback.

- [ ] **Step 8: Commit**

~~~bash
git add src/companion/retained.rs src/companion/retained/parity.rs src/companion/retained/capture.rs src/companion/retained/resources.rs src/companion/app.rs src/commands/companion_mode.rs src/cli.rs tests/retained_renderer_boundary.rs tests/companion_draw_boundary.rs tests/retained_scene.rs xtask/src/lib.rs
git rm src/companion/retained.wgsl
git commit -m "refactor(renderer): remove Smooth scene translation"
~~~

---

### Task 18: Add the lit ChestCardGeometryV1 proof and bubble attachment

**Files:**
- Create: src/presentation/companion_scene/chest.rs
- Modify: src/presentation/companion_scene/mod.rs
- Modify: src/presentation/companion_scene/scene.rs
- Modify: src/presentation/companion_scene/validate.rs
- Modify: src/companion/retained/compiler.rs
- Modify: src/companion/retained/render.rs
- Modify: src/companion/retained/scene.wgsl
- Modify: src/dev_preview/contract.rs
- Modify: src/dev_preview/round.rs
- Modify: src/dev_preview/scenarios.rs
- Modify: tests/dev_preview.rs
- Modify: tests/retained_scene.rs
- Create: docs/superpowers/measurements/2026-07-11-glorp-lit-chest-review.md

**Interfaces:**
- Consumes: ShallowCard primitive, LitShallowCard material, two lights, stable attachment system.
- Produces: ChestCardGeometryV1, lit chest node/material, bubble_origin attachment, deterministic/native evidence. It does not produce bubbles.

- [ ] **Step 1: Write failing geometry and attachment tests**

~~~rust
#[test]
fn chest_v1_has_locked_outline_depth_winding_and_bounds() {
    let mesh = ChestCardGeometryV1::build();
    assert_eq!(mesh.outer_outline, [
        [-1.50, -1.00], [1.50, -1.00], [1.50, 0.65],
        [1.25, 1.00], [-1.25, 1.00], [-1.50, 0.65],
    ]);
    assert_eq!(mesh.front_z, 0.0);
    assert_eq!(mesh.bevel_z, -0.08);
    assert_eq!(mesh.back_z, -0.28);
    assert!(mesh.front_triangles_are_ccw_from_positive_z());
    assert!(mesh.side_normals_point_outward());
}

#[test]
fn chest_attachment_follows_then_snapshots_world_transform() {
    let mut scene = SceneGenerationData::fixture_chest();
    let attachment_id = {
        let local = scene.attachment("world.prop.treasure_chest.bubble_origin").unwrap();
        assert_eq!(local.transform.translation, [0.0, 1.25, 0.0]);
        local.id
    };
    let followed = scene.resolve_attachment(attachment_id, AttachmentMode::Follow);
    let spawned = scene.spawn_attachment(attachment_id, AttachmentMode::SnapshotWorldOnSpawn);
    let snapped_world = spawned.world;
    scene.move_node("world.prop.treasure_chest", [1.0, 0.0, 0.5]);
    assert_ne!(scene.resolve_attachment(attachment_id, AttachmentMode::Follow), followed);
    assert_eq!(spawned.world, snapped_world);
}

#[test]
fn non_uniform_or_negative_ancestor_scale_rejects_lit_card() {
    let mut scene = SceneGenerationData::fixture_chest();
    scene.set_parent_scale("world.behind", [-1.0, 1.0, 1.0]);
    assert_eq!(validate_template(&scene.template), Err(SceneValidationError::LitCardRequiresPositiveUniformScale));
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test presentation::companion_scene::chest
cargo test --features retained-renderer --test retained_scene lit_chest
~~~

Expected: compile failure because chest module/geometry do not exist.

- [ ] **Step 3: Implement the exact authored mesh**

Use the locked six-point convex outline. Map its 3x2 bounds to the current open/closed chest glyph sprite. Inset front by 0.08 with edge-line intersections and 2.0x miter cap at Z 0.0; connect bevel to outer outline at Z -0.08; extrude sides to reversed back at Z -0.28. Front is CCW from +Z, back reversed, sides outward, back-face culling enabled. Use flat face normals, fixed yaw +12 degrees, pitch -6 degrees, and positive uniform ancestor scale.

- [ ] **Step 4: Implement LitShallowCard**

Front albedo combines current open/closed glyph raster with authored amber base. Bevel/side/back use named darker palette roles. Shader computes linear ambient + clamped Lambert key + bounded view/key rim. Use one ambient and one directional key in the fixed two-light buffer. Pet/HUD/gauges remain unlit.

- [ ] **Step 5: Add bubble_origin without bubbles**

Create AttachmentId from world.prop.treasure_chest.bubble_origin with local translation [0.0, 1.25, 0.0]. Export resolved Follow and SnapshotWorldOnSpawn matrices in evidence. Do not allocate or render a bubble instance group in this task.

- [ ] **Step 6: Add deterministic and native fixtures**

Add far, neutral, near, unlit control, lit-key-left, lit-key-right, open, closed, yaw/pitch, and attachment-transform fixtures. Lock geometry checksum, normals, light parameters, resolved attachment matrices, readback samples, and privacy.

- [ ] **Step 7: Run final automated and native gate**

~~~bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features retained-renderer presentation::companion_scene::chest
cargo test --features retained-renderer --test retained_scene lit_chest
cargo test --features "dev-preview retained-renderer" --test dev_preview lit_chest
cargo run --features "dev-preview retained-renderer" -- dev-preview --scenario round --out target/glorp-preview-lit-chest
cargo xtask companion scene-review --size 360 --scale 2 --state normal --out target/glorp-review/lit-chest
cargo test --features retained-renderer
cargo test -p xtask
node --test scripts/test/macos-app-packaging.test.mjs
git diff --check
~~~

Expected: all pass, native chest depth/light identity is accepted, attachment matrices are correct, no fallback/resource growth, and no production bubble exists.

- [ ] **Step 8: Record approval and commit**

Record exact commit, hardware/OS, commands, artifact paths, accepted light/depth views, attachment evidence, performance delta, and Drew's visual decision in the measurement file.

~~~bash
git add src/presentation/companion_scene/chest.rs src/presentation/companion_scene/mod.rs src/presentation/companion_scene/scene.rs src/presentation/companion_scene/validate.rs src/companion/retained/compiler.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl src/dev_preview/contract.rs src/dev_preview/round.rs src/dev_preview/scenarios.rs tests/dev_preview.rs tests/retained_scene.rs docs/superpowers/measurements/2026-07-11-glorp-lit-chest-review.md
git commit -m "feat(companion): add lit treasure chest scene proof"
~~~

---

## Final Completion Gate

Before claiming the program complete, verify:

~~~bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --locked --no-default-features --all-targets
cargo test -p xtask
node --test scripts/test/macos-app-packaging.test.mjs
cargo run --features "dev-preview retained-renderer" -- dev-preview --scenario all --out target/glorp-preview
cargo xtask companion scene-review --size 360 --scale 2 --state normal --out target/glorp-review/final-scene
git diff --check
~~~

The final review record must show one retained scene-generation path, no legacy translator, a proven cold Smooth fallback, accepted linear/depth/capture behavior, fixed post-warmup resources, green baseline/fault/soak gates, and an accepted lit chest with a stable bubble_origin attachment.
