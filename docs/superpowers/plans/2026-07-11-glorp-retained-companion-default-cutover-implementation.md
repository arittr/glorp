# Glorp Retained Companion Default Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish retained visual parity, make wgpu/Metal the reversible Apple-Silicon companion default, and preserve Smooth as explicit and automatic fallback.

**Architecture:** Keep the existing WatchViewModel -> SmoothCompanionScenePlan derivation, but add explicit renderer policy/runtime state, transactional Metal activation, immutable paired review frames, ordered GPU readback, persistent bounded resources, and truthful fallback evidence. The persistent future scene graph remains deferred; this plan strengthens the current translator and host seams so that later work can replace frame preparation without replacing capture, policy, or recovery.

**Tech Stack:** Rust 2021, objc2/AppKit/Core Animation, wgpu 30/Metal/WGSL, serde/serde_json, sha2, png, Cargo features, Node 24 packaging scripts, GitHub Actions, Rust xtask.

**Spec:** docs/superpowers/specs/2026-07-11-glorp-retained-companion-default-cutover-design.md

## Global Constraints

- Do not create a branch without Drew's explicit approval.
- Preserve all unrelated dirty-worktree changes; stage only the files named by each task.
- Smooth remains compiled on macOS, explicitly selectable, and the automatic technical fallback.
- Intel release Auto remains Smooth; Intel release binaries do not compile Retained.
- Do not import crate::renderer_spike DTOs into production modules. Port small proven algorithms behind production-owned types.
- Do not add the persistent scene graph, new 2.5D effects, lighting, particles, materials, meshes, or camera behavior.
- Stored captures redact live HUD values by default.
- Sensitive captures require explicit opt-in and must stay below target/glorp-review-sensitive after canonical path and symlink validation.
- Routine commands target completion within two minutes. Stop and narrow a command that exceeds that bound.
- Do not run CPU, energy, memory, startup, package-size, build-time, release-publish, or renderer-qualification matrices automatically.
- Gate 5 cannot change Auto until the final capture/native gate is complete and Drew explicitly approves the evidence.

---

## Entry State

The working tree already contains the uncommitted retained pilot:

- Cargo.toml retained-renderer feature wiring;
- companion command/mode/AppKit host changes;
- src/companion/retained.rs and retained.wgsl;
- the visual-parity pilot spec and earlier implementation plan.

Commit 8ee0f8d contains the controlling cutover design. The first implementation task preserves and verifies the dirty prototype rather than recreating or discarding it.

## File Responsibility Map

- src/commands/companion_mode.rs: renderer request, effective renderer, target/capability resolution, runtime renderer state.
- src/companion/app.rs: macOS lifecycle, prepared live frame ownership, main-thread fallback transition, AppKit paint acknowledgement.
- src/companion/retained.rs: retained host orchestration, transactional activation, frame encode/present entrypoint.
- src/companion/retained/presentation.rs: frame milestones, terminal dispositions, sanitized GPU error mailbox.
- src/companion/retained/capture.rs: canonical GPU intermediate/readback, row normalization, frame-correlated RGBA output.
- src/companion/retained/resources.rs: glyph/resource manifests, atlas entries, persistent buffers, counters, atomic generations.
- src/companion/retained/parity.rs: production-owned color, blend, dither, HUD, gauge, and edge-coverage math.
- src/companion/retained.wgsl: premultiplied-linear scene rendering and physical-pixel analytic coverage.
- src/companion/paired_review.rs: immutable PairedReviewFrame, capture path policy, pair manifest, Smooth/Retained coordinator.
- src/companion/review_capture.rs: bounded capture session and existing Smooth evidence, adapted to renderer runtime state.
- xtask/src/lib.rs: bounded review-pair, artifact validation, staged-app smoke, and rollback-rehearsal commands.
- scripts/build-macos-app-shared.mjs: explicit feature selection for locally built Apple-Silicon companion bundles.
- scripts/test/macos-app-packaging.test.mjs: release workflow and bundle-feature assertions.
- .github/workflows/ci.yml and publish.yml: native retained checks and exact per-target feature matrix.
- tests/retained_renderer_boundary.rs: source-boundary assertions preventing spike DTO imports and AppKit caching for retained capture.

---

### Task 1: Preserve and commit the current retained pilot baseline

**Files:**
- Modify: src/companion/retained.rs:1248
- Modify: docs/superpowers/plans/2026-07-11-glorp-retained-companion-visual-parity-pilot-implementation.md:1
- Commit existing in-scope changes: Cargo.toml
- Commit existing in-scope changes: src/commands/companion.rs
- Commit existing in-scope changes: src/commands/companion_mode.rs
- Commit existing in-scope changes: src/companion/app.rs
- Commit existing in-scope changes: src/companion/mod.rs
- Commit existing in-scope changes: src/companion/retained.rs
- Commit existing in-scope changes: src/companion/retained.wgsl
- Commit existing in-scope changes: docs/superpowers/specs/2026-07-11-glorp-retained-companion-visual-parity-pilot.md
- Commit existing in-scope changes: docs/superpowers/plans/2026-07-11-glorp-retained-companion-visual-parity-pilot-implementation.md

**Interfaces:**
- Consumes: the current dirty prototype exactly as inspected before this plan.
- Produces: a clean committed retained pilot baseline; no behavioral change beyond the clippy fix.

- [ ] **Step 1: Mark the earlier pilot plan as historical**

Insert below its title:

~~~markdown
> Historical pilot plan. Execution is superseded by
> docs/superpowers/plans/2026-07-11-glorp-retained-companion-default-cutover-implementation.md.
> Keep this document as the record of the initial parity slice.
~~~

- [ ] **Step 2: Fix the known clippy failure without changing behavior**

Replace the final return in rasterize_glyph with:

~~~rust
        Ok(GlyphAtlasEntry {
            uv,
            ink_origin,
            ink_size,
            advance: size.width as f32,
            line_height: size.height as f32,
        })
~~~

- [ ] **Step 3: Run the focused baseline checks**

Run:

~~~bash
cargo fmt --check
cargo test --features retained-renderer companion::retained
cargo clippy --lib --features retained-renderer -- -D warnings
cargo test --test round_scene
git diff --check
~~~

Expected: seven retained unit tests pass, round_scene passes, and fmt/clippy/diff checks exit 0.

- [ ] **Step 4: Inspect and stage only the retained pilot**

Run:

~~~bash
git status --short
git add Cargo.toml src/commands/companion.rs src/commands/companion_mode.rs src/companion/app.rs src/companion/mod.rs src/companion/retained.rs src/companion/retained.wgsl docs/superpowers/specs/2026-07-11-glorp-retained-companion-visual-parity-pilot.md docs/superpowers/plans/2026-07-11-glorp-retained-companion-visual-parity-pilot-implementation.md
git diff --cached --check
git diff --cached --stat
~~~

Expected: the staged set is exactly the nine paths above.

- [ ] **Step 5: Commit**

~~~bash
git commit -m "feat(companion): add retained Metal pilot"
~~~

---

### Task 2: Separate renderer request, effective renderer, and runtime policy

**Files:**
- Modify: src/commands/companion_mode.rs
- Modify: src/cli.rs:47-78,182-221
- Modify: src/commands/companion.rs
- Modify: src/commands/companion_app.rs
- Modify: src/companion/app.rs:443-475,547-705
- Modify: tests/cli_smoke.rs:205-470

**Interfaces:**
- Consumes: CompanionReviewOptions and the existing Classic/Pixel/Smooth/Retained modes.
- Produces:
  - CompanionRendererRequest
  - EffectiveCompanionRenderer
  - CompanionRendererTarget
  - resolve_renderer(request, target, retained_compiled, auto_retained_enabled)
  - RendererRuntimeState

- [ ] **Step 1: Write failing pure policy tests**

Add tests in companion_mode.rs:

~~~rust
#[cfg(all(target_os = "macos", feature = "retained-renderer"))]
#[test]
fn auto_policy_is_architecture_and_capability_aware() {
    assert_eq!(
        resolve_renderer(
            CompanionRendererRequest::Auto,
            CompanionRendererTarget::AppleSiliconMac,
            true,
            false,
        ),
        Ok(EffectiveCompanionRenderer::Smooth),
    );
    assert_eq!(
        resolve_renderer(
            CompanionRendererRequest::Auto,
            CompanionRendererTarget::AppleSiliconMac,
            true,
            true,
        ),
        Ok(EffectiveCompanionRenderer::Retained),
    );
    assert_eq!(
        resolve_renderer(
            CompanionRendererRequest::Auto,
            CompanionRendererTarget::IntelMac,
            false,
            true,
        ),
        Ok(EffectiveCompanionRenderer::Smooth),
    );
}

#[cfg(all(target_os = "macos", feature = "retained-renderer"))]
#[test]
fn fallback_preserves_requested_renderer() {
    let mut state = RendererRuntimeState::new(
        CompanionRendererRequest::Retained,
        EffectiveCompanionRenderer::Retained,
    );
    state.fallback_to_smooth("retained-device-lost");
    assert_eq!(state.requested(), CompanionRendererRequest::Retained);
    assert_eq!(state.effective(), EffectiveCompanionRenderer::Smooth);
    assert_eq!(state.transition_count(), 1);
    assert_eq!(state.last_fallback_reason(), Some("retained-device-lost"));
}
~~~

- [ ] **Step 2: Run the policy tests to verify failure**

Run:

~~~bash
cargo test --features retained-renderer commands::companion_mode::tests::auto_policy_is_architecture_and_capability_aware
~~~

Expected: compile failure because the new types/functions do not exist.

- [ ] **Step 3: Implement the request/effective types and keep the cutover disabled**

Use this public shape in companion_mode.rs:

~~~rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum CompanionRendererRequest {
    #[default]
    Auto,
    Classic,
    Pixel,
    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    Retained,
    Smooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveCompanionRenderer {
    Classic,
    Pixel,
    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    Retained,
    Smooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionRendererTarget {
    AppleSiliconMac,
    IntelMac,
    Other,
}

pub const AUTO_RETAINED_ON_APPLE_SILICON: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererRuntimeState {
    requested: CompanionRendererRequest,
    effective: EffectiveCompanionRenderer,
    transition_count: u64,
    last_fallback_reason: Option<&'static str>,
}
~~~

Implement resolve_renderer as the sole Auto resolver. Return a static sanitized RendererUnavailable error when explicit Retained is unavailable. Move is_pixel, uses_smooth_scene, and as_str behavior to the effective enum. Add requested/effective accessors and fallback_to_smooth exactly as exercised by the tests.

- [ ] **Step 4: Thread request/effective state through CLI, commands, and AppState**

Change both hidden CLI renderer defaults to CompanionRendererRequest::Auto. Resolve once in companion::app::run and store RendererRuntimeState in AppState. Never overwrite the request during fallback. Update command launch arguments so Auto is omitted and explicit selections pass their request string.

- [ ] **Step 5: Add CLI regression tests**

Cover hidden Auto default, retained availability under the feature, and preservation of Classic/Pixel/Smooth. On a feature-off build, explicit retained must be rejected by clap.

- [ ] **Step 6: Run checks**

~~~bash
cargo test commands::companion_mode
cargo test cli::tests
cargo test --test cli_smoke companion
cargo test --features retained-renderer commands::companion_mode
cargo clippy --lib --features retained-renderer -- -D warnings
~~~

Expected: all selected tests pass; Auto still resolves to Smooth because the cutover constant remains false.

- [ ] **Step 7: Commit**

~~~bash
git add src/commands/companion_mode.rs src/cli.rs src/commands/companion.rs src/commands/companion_app.rs src/companion/app.rs tests/cli_smoke.rs
git commit -m "refactor(companion): separate renderer request from runtime"
~~~

---

### Task 3: Add observable frame progress and a main-thread GPU error mailbox

**Files:**
- Create: src/companion/retained/presentation.rs
- Modify: src/companion/retained.rs
- Modify: src/companion/app.rs

**Interfaces:**
- Consumes: RendererRuntimeState from Task 2.
- Produces:
  - FrameMilestone
  - FrameDisposition
  - FrameProgress
  - RetainedFailureCategory
  - GpuErrorMailbox

- [ ] **Step 1: Write failing transition tests**

~~~rust
#[test]
fn skipped_frame_cannot_claim_surface_or_readback() {
    let mut progress = FrameProgress::new(7, 3);
    progress.mark(FrameMilestone::Prepared).unwrap();
    progress.finish(FrameDisposition::Skipped(SkipReason::Occluded)).unwrap();
    assert!(!progress.observed(FrameMilestone::SurfacePresentCalled));
    assert!(!progress.observed(FrameMilestone::ReadbackCompleted));
}

#[test]
fn milestones_are_monotonic_and_terminal_is_single_assignment() {
    let mut progress = FrameProgress::new(8, 4);
    progress.mark(FrameMilestone::Prepared).unwrap();
    assert!(progress.mark(FrameMilestone::Submitted).is_err());
    progress.mark(FrameMilestone::Encoded).unwrap();
    progress.mark(FrameMilestone::Submitted).unwrap();
    progress.finish(FrameDisposition::SurfacePresentCalled).unwrap();
    assert!(progress.finish(FrameDisposition::Failed(
        RetainedFailureCategory::SurfaceLost,
    )).is_err());
}
~~~

- [ ] **Step 2: Run to verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::presentation
~~~

Expected: compile failure because the module/types do not exist.

- [ ] **Step 3: Implement production-owned progress types**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FrameMilestone {
    Prepared,
    Encoded,
    Submitted,
    SurfacePresentCalled,
    GpuCompleted,
    ReadbackCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameDisposition {
    SurfacePresentCalled,
    Captured,
    Skipped(SkipReason),
    Failed(RetainedFailureCategory),
    FallbackPending(RetainedFailureCategory),
    FallbackPainted(RetainedFailureCategory),
}
~~~

FrameProgress stores frame_id, resource_generation, a BTreeSet of milestones, and one optional disposition. mark requires the preceding milestone; finish rejects a second terminal disposition.

Implement GpuErrorMailbox with std::sync::mpsc::Sender/Receiver. The wgpu callback owns only a cloned sender and emits static categories. AppState drains the receiver on the main thread before recording success.

- [ ] **Step 4: Replace Result<(), RetainedFailure> render reporting**

RetainedHost::render returns FrameProgress. Map Outdated/Suboptimal reconfiguration, Timeout, Occluded, Lost, and Validation to distinct progress/disposition values. Do not call review_capture.record_frame from a generic Ok branch.

- [ ] **Step 5: Run checks**

~~~bash
cargo test --features retained-renderer companion::retained::presentation
cargo test --features retained-renderer companion::retained
cargo clippy --lib --features retained-renderer -- -D warnings
~~~

Expected: transition/mailbox tests and existing retained tests pass.

- [ ] **Step 6: Commit**

~~~bash
git add src/companion/retained/presentation.rs src/companion/retained.rs src/companion/app.rs
git commit -m "feat(renderer): report retained frame dispositions"
~~~

---

### Task 4: Make retained host activation transactional

**Files:**
- Modify: src/companion/retained.rs:176-266
- Modify: src/companion/app.rs:600-660
- Test: src/companion/retained.rs unit tests

**Interfaces:**
- Consumes: GpuErrorMailbox and RetainedFailureCategory.
- Produces:
  - PreparedRetainedHost::prepare
  - PreparedRetainedHost::activate
  - ActiveRetainedHost
  - LayerActivationGuard

- [ ] **Step 1: Write failing activation-state tests**

~~~rust
#[test]
fn failed_preflight_never_marks_layer_attached() {
    let mut state = LayerActivationState::default();
    state.preflight_failed();
    assert!(!state.attached());
    assert!(state.appkit_restored());
}

#[test]
fn activation_guard_restores_uncommitted_attachment() {
    let state = std::rc::Rc::new(std::cell::Cell::new(true));
    {
        let _guard = LayerActivationGuard::for_test(state.clone());
    }
    assert!(!state.get());
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::tests::failed_preflight
~~~

Expected: compile failure for missing activation types.

- [ ] **Step 3: Split prepare from activate**

Create the CAMetalLayer, wgpu instance/surface, adapter, device, configuration, pipelines, mailbox, and initial size without installing the layer on NSView. PreparedRetainedHost::activate is the only method that calls setWantsLayer(true) and setLayer.

Use this ownership shape:

~~~rust
pub(super) struct PreparedRetainedHost {
    host: RetainedHost,
}

impl PreparedRetainedHost {
    pub(super) fn prepare(
        view: &NSView,
        mailbox: GpuErrorMailbox,
    ) -> Result<Self, RetainedFailureCategory>;

    pub(super) fn activate(
        self,
        view: &NSView,
    ) -> Result<ActiveRetainedHost, RetainedFailureCategory>;
}
~~~

LayerActivationGuard restores the prior AppKit layer state on Drop until commit is called. ActiveRetainedHost::restore_appkit remains idempotent.

- [ ] **Step 4: Update app startup fallback**

Resolve the request first, prepare the host, activate only on success, and update effective renderer to Smooth on failure. Review capture reads requested/effective from RendererRuntimeState after activation.

- [ ] **Step 5: Run checks**

~~~bash
cargo test --features retained-renderer companion::retained
cargo test --features retained-renderer companion::app
cargo clippy --lib --features retained-renderer -- -D warnings
~~~

Expected: activation tests pass; injected/preflight failure leaves effective Smooth.

- [ ] **Step 6: Commit**

~~~bash
git add src/companion/retained.rs src/companion/app.rs
git commit -m "fix(renderer): activate Metal host transactionally"
~~~

---

### Task 5: Freeze canonical paired review frames and enforce capture-path privacy

**Files:**
- Create: src/companion/paired_review.rs
- Modify: src/companion/mod.rs
- Modify: src/companion/app.rs:88-138,350-440
- Modify: src/commands/companion_mode.rs:4-60
- Modify: src/cli.rs:47-78,145-180
- Modify: Cargo.toml retained-renderer feature

**Interfaces:**
- Consumes: PreparedCompanionFrame and RendererRuntimeState.
- Produces:
  - PairedReviewFrame
  - PairedReviewIdentity
  - CapturePrivacy
  - validate_review_output
  - canonical_frame_checksum

- [ ] **Step 1: Add failing checksum and path tests**

~~~rust
#[test]
fn frozen_frame_checksum_changes_with_chrome_or_geometry() {
    let frame = PairedReviewFrame::fixture();
    let mut changed = frame.clone();
    changed.identity.gauges.pace_fraction = 0.75;
    assert_ne!(frame.checksum, changed.recompute_checksum());
}

#[test]
fn sensitive_capture_rejects_escape_and_symlink() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path();
    std::fs::create_dir_all(repo.join("target/glorp-review-sensitive")).unwrap();
    assert!(validate_review_output(
        repo,
        std::path::Path::new("target/glorp-review-sensitive/pair"),
        CapturePrivacy::SensitiveLiveValues,
    ).is_ok());
    assert!(validate_review_output(
        repo,
        std::path::Path::new("target/glorp-review-sensitive/../../private"),
        CapturePrivacy::SensitiveLiveValues,
    ).is_err());
    std::os::unix::fs::symlink(
        repo.join("target/glorp-review-sensitive"),
        repo.join("target/review-link"),
    ).unwrap();
    assert!(validate_review_output(
        repo,
        std::path::Path::new("target/review-link/pair"),
        CapturePrivacy::SensitiveLiveValues,
    ).is_err());
    assert!(validate_review_output(
        repo,
        repo.join("target/glorp-review-sensitive/pair").as_path(),
        CapturePrivacy::SensitiveLiveValues,
    ).is_err());
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::paired_review
~~~

Expected: compile failure for missing module/types.

- [ ] **Step 3: Add production checksum dependencies**

Add dep:sha2 and dep:png to retained-renderer. Cargo.lock should not need new package versions because both are already used by renderer-spike; verify the lock diff rather than assuming.

- [ ] **Step 4: Implement immutable identity**

PairedReviewFrame owns a cloned PreparedCompanionFrame plus a serializable PairedReviewIdentity containing plan checksum, draw order, metrics, aperture, background/aura, dim state, gauges, HUD, overlays, logical/physical dimensions, backing scale, semantic tick, sampled elapsed milliseconds, frame ID, and resource generation. canonical_frame_checksum serializes only that identity and returns lowercase SHA-256.

Expose the minimum PreparedCompanionFrame accessors required by paired_review; do not expose AppState or live ViewModel state.

- [ ] **Step 5: Implement privacy policy**

Add review_capture_live_values: bool to CompanionReviewOptions and hidden --review-capture-live-values to both companion commands. Default is false. Sensitive mode requires a canonical path below target/glorp-review-sensitive and rejects symlinks, parent components, absolute paths, and out-of-root destinations. Default redacted captures use target/glorp-review.

- [ ] **Step 6: Run checks**

~~~bash
cargo test --features retained-renderer companion::paired_review
cargo test commands::companion_mode
cargo test cli::tests
cargo test --test storage_privacy
~~~

Expected: checksum sensitivity, default redaction, opt-in, and path-rejection tests pass.

- [ ] **Step 7: Commit**

~~~bash
git add Cargo.toml Cargo.lock src/companion/paired_review.rs src/companion/mod.rs src/companion/app.rs src/commands/companion_mode.rs src/cli.rs
git commit -m "feat(renderer): freeze privacy-safe paired review frames"
~~~

---

### Task 6: Port canonical GPU readback behind production-owned types

**Files:**
- Create: src/companion/retained/capture.rs
- Modify: src/companion/retained.rs
- Create: tests/retained_renderer_boundary.rs

**Interfaces:**
- Consumes: PairedReviewFrame frame/generation IDs and ActiveRetainedHost device/queue.
- Produces:
  - CanonicalRgbaFrame
  - ReadbackMetadata
  - normalize_readback_rows
  - RetainedCaptureTarget::capture

- [ ] **Step 1: Write failing pure readback-normalization tests**

~~~rust
#[test]
fn bgra_rows_are_unpadded_swizzled_and_top_left() {
    let source = vec![
        3, 2, 1, 255, 6, 5, 4, 255, 0, 0, 0, 0,
        9, 8, 7, 255, 12, 11, 10, 255, 0, 0, 0, 0,
    ];
    let rgba = normalize_readback_rows(&source, 2, 2, 12, PixelOrder::Bgra).unwrap();
    assert_eq!(
        rgba,
        vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
    );
}

#[test]
fn row_normalization_rejects_short_mapped_buffer() {
    assert!(normalize_readback_rows(&[0; 7], 2, 1, 8, PixelOrder::Rgba).is_err());
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::capture
~~~

Expected: compile failure for missing capture module.

- [ ] **Step 3: Implement canonical CPU normalization**

~~~rust
pub(super) struct CanonicalRgbaFrame {
    pub(super) frame_id: u64,
    pub(super) resource_generation: u64,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba: Vec<u8>,
}

pub(super) fn aligned_bytes_per_row(width: u32) -> u32 {
    let raw = width.saturating_mul(4);
    raw.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        .saturating_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
}
~~~

normalize_readback_rows copies only width * 4 bytes from each aligned row, swaps B/R for BGRA, preserves top-left row order, and unpremultiplies RGB only when the declared source convention requires it.

- [ ] **Step 4: Port the proven wgpu readback sequence**

Adapt src/renderer_spike/wgpu.rs:812-950 into production-owned capture types:

1. use the retained host's persistent physical-size sRGB intermediate;
2. copy the intermediate to a MAP_READ staging buffer with 256-byte alignment;
3. submit once and retain the submission index;
4. map_async, device.poll with the submission index and a five-second bounded timeout;
5. receive the callback result;
6. normalize rows/channels/alpha;
7. return CanonicalRgbaFrame with matching frame/generation IDs.

Do not import any renderer_spike module.

- [ ] **Step 5: Add boundary assertions**

tests/retained_renderer_boundary.rs reads production source files and fails if they contain renderer_spike::, bitmapImageRepForCachingDisplayInRect inside retained capture, or cacheDisplayInRect_toBitmapImageRep inside retained capture.

- [ ] **Step 6: Run checks**

~~~bash
cargo test --features retained-renderer companion::retained::capture
cargo test --test retained_renderer_boundary
cargo clippy --lib --features retained-renderer -- -D warnings
~~~

Expected: normalization and boundary tests pass.

- [ ] **Step 7: Commit**

~~~bash
git add src/companion/retained/capture.rs src/companion/retained.rs tests/retained_renderer_boundary.rs
git commit -m "feat(renderer): add canonical Metal readback"
~~~

---

### Task 7: Produce paired Smooth/Retained artifacts with a machine-verifiable manifest

**Files:**
- Modify: src/companion/paired_review.rs
- Modify: src/companion/review_capture.rs
- Modify: src/companion/app.rs
- Modify: src/companion/retained.rs
- Modify: src/cli.rs
- Modify: xtask/src/lib.rs

**Interfaces:**
- Consumes: PairedReviewFrame and CanonicalRgbaFrame.
- Produces:
  - PairedCaptureCoordinator
  - PairManifest schema version 1
  - cargo xtask companion review-pair
  - validate_review_pair

- [ ] **Step 1: Write failing manifest-validation tests**

~~~rust
#[test]
fn retained_pair_requires_matching_gpu_and_readback_milestones() {
    let mut manifest = PairManifest::fixture();
    manifest.retained.milestones.retain(|m| m != "readback-completed");
    assert_eq!(
        validate_review_pair(&manifest).unwrap_err(),
        "retained capture missing readback-completed",
    );
}

#[test]
fn smooth_fallback_cannot_satisfy_retained_capture() {
    let mut manifest = PairManifest::fixture();
    manifest.retained.effective_renderer = "smooth".into();
    assert!(validate_review_pair(&manifest).is_err());
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::paired_review::tests::retained_pair
~~~

Expected: compile failure for missing manifest/coordinator.

- [ ] **Step 3: Implement paired rendering**

PairedCaptureCoordinator receives one PairedReviewFrame. It installs an AppKit bitmap graphics context and calls the existing paint_prepared_frame for Smooth, then calls ActiveRetainedHost::capture for Retained. Neither path may read AppState clocks or rebuild the frame.

Write:

~~~text
pair-manifest.json
smooth.png
retained.png
~~~

The manifest records requested/effective renderer, compiled capabilities, frame checksum, logical/physical size, backing scale, frame/generation IDs, observed milestones, terminal disposition, resource counters, fallback state, privacy mode, and relative paths. It never records exact HUD strings.

- [ ] **Step 4: Make capture failure machine-visible**

ReviewCapture stores a terminal Result. The direct companion-app review process returns a GlorpError after NSApplication exits when pair generation fails. The xtask validator also rejects a missing/non-success manifest, so open returning success cannot hide an app-side capture failure.

- [ ] **Step 5: Add bounded xtask command**

Add:

~~~text
cargo xtask companion review-pair --size 360 --state normal --out target/glorp-review/pair
cargo xtask companion review-pair --size 360 --out target/glorp-review/live-pair
cargo xtask companion review-pair --size 360 --live-values --out target/glorp-review-sensitive/live-pair
~~~

The command builds with retained-renderer, runs companion-app directly with a duration and hard timeout, then validates pair-manifest.json. With no --state, it freezes the actual current companion pet, room, props, tank life, activity, gauges, dim state, and prepared HUD. --state selects only a privacy-safe deterministic sentinel fixture for plumbing and fault tests; it is never the human parity oracle. --dimmed freezes the same live state with the prepared dim-composition flag forced on for the final matrix. --live-values maps to the hidden --review-capture-live-values process flag and does not change which live frame is frozen.

- [ ] **Step 6: Run checks and one redacted sentinel pair**

~~~bash
cargo test --features retained-renderer companion::paired_review
cargo test -p xtask
cargo xtask companion review-pair --size 360 --state normal --out target/glorp-review/sentinel-360
~~~

Expected: command exits 0; both PNGs are nonblank; each physical dimension equals its logical dimension multiplied by the recorded backing scale (720x720 only when the scale is 2); manifest frame/generation IDs match; privacy mode is redacted.

- [ ] **Step 7: Commit**

~~~bash
git add src/companion/paired_review.rs src/companion/review_capture.rs src/companion/app.rs src/companion/retained.rs src/cli.rs xtask/src/lib.rs
git commit -m "feat(renderer): capture matched Smooth and Metal frames"
~~~

---

### Task 8: Implement complete glyph metrics, scalar-sequence keys, and color atlas entries

**Files:**
- Create: src/companion/retained/resources.rs
- Modify: src/companion/retained.rs:1111-1302
- Modify: src/companion/retained.wgsl

**Interfaces:**
- Consumes: SmoothLocalCell glyph strings and ASCII CompanionHudText.
- Produces:
  - GlyphSequence
  - GlyphKey
  - GlyphEntryKind
  - GlyphAtlasEntry
  - ResolvedFontPolicy
  - rasterize_glyph_entry

- [ ] **Step 1: Write failing entry-contract tests**

~~~rust
#[test]
fn scalar_sequence_is_one_atlas_key() {
    let key = GlyphKey::new("ö", false);
    assert_eq!(key.sequence.as_str(), "ö");
}

#[test]
fn color_entry_bypasses_foreground_tint() {
    let entry = GlyphAtlasEntry::fixture(GlyphEntryKind::PremultipliedColorRgba);
    assert_eq!(entry.fragment_mode(), FragmentGlyphMode::NativeColor);
}

#[test]
fn whitespace_keeps_advance_without_visible_uv() {
    let entry = GlyphAtlasEntry::whitespace(24.0, 52.0);
    assert_eq!(entry.advance, 24.0);
    assert_eq!(entry.visible_uv, None);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::resources::glyph
~~~

Expected: compile failure because resources module/types are missing.

- [ ] **Step 3: Implement the metric and policy types**

GlyphAtlasEntry contains visible UV, ink size, horizontal/vertical bearing, baseline, ascent, descent, line height, advance, raster size, safe padding, font policy ID, and entry kind.

ResolvedFontPolicy contains the resolved NSFont PostScript name, a stable hash of its canonicalized fontDescriptor attributes, point size, backing scale, weight, antialiasing policy, atlas packing version, and shader resource version. Use the exact NSFont returned by the Smooth font policy; do not invent a fallback font.

- [ ] **Step 4: Preserve full color pixels**

Rasterize into RGBA. Classify an entry as PremultipliedColorRgba only when nontransparent pixels contain native chroma; otherwise store coverage alpha and white RGB. Convert native color pixels to the declared premultiplied representation. The WGSL fragment path samples alpha for masks and full RGBA for native color.

- [ ] **Step 5: Keep HUD tokenization explicit**

HUD's permitted character set is ASCII. Tokenize it as one-scalar GlyphSequence values only after validating text.is_ascii(). Pet/room/prop strings remain complete authored scalar sequences and are never split with chars().

- [ ] **Step 6: Add native metric coverage**

Cover ordinary, narrow, descender, bold, whitespace, replacement, composed mark, and bubble emoji entries at backing scales 1 and 2. Compare physical ink bounds/baselines to attributed Smooth measurement with declared one-pixel geometry tolerance.

- [ ] **Step 7: Run checks**

~~~bash
cargo test --features retained-renderer companion::retained::resources
cargo clippy --lib --features retained-renderer -- -D warnings
~~~

Expected: metric, scalar-sequence, color-entry, and native parity tests pass.

- [ ] **Step 8: Commit**

~~~bash
git add src/companion/retained/resources.rs src/companion/retained.rs src/companion/retained.wgsl
git commit -m "feat(renderer): preserve complete glyph atlas semantics"
~~~

---

### Task 9: Preflight the full dynamic glyph repertoire and activate resources atomically

**Files:**
- Modify: src/companion/retained/resources.rs
- Modify: src/companion/app.rs
- Modify: src/round/smooth.rs
- Test: src/companion/retained/resources.rs unit tests

**Interfaces:**
- Consumes: active pet identity/stage, room dialect, owned props/tank life, declared animations/effects, and HUD charset.
- Produces:
  - GlyphRepertoireManifest
  - ResourceGenerationKey
  - CompiledRetainedResources
  - RetainedResourceCounters

- [ ] **Step 1: Write failing repertoire tests**

~~~rust
#[test]
fn manifest_contains_dynamic_and_chrome_repertoire() {
    let manifest = GlyphRepertoireManifest::for_fixture_pet();
    for required in ["-", ".", "0", "9", "�", "ö", "🫧"] {
        assert!(manifest.contains_sequence(required), "missing {required}");
    }
}

#[test]
fn full_animation_strip_has_no_post_activation_atlas_churn() {
    let manifest = GlyphRepertoireManifest::for_fixture_pet();
    let mut cache = TestResourceCache::activate(manifest);
    for frame in deterministic_full_strip() {
        cache.prepare(&frame).unwrap();
    }
    assert_eq!(cache.counters().atlas_builds_after_activation, 0);
    assert_eq!(cache.counters().atlas_uploads_after_activation, 0);
    assert_eq!(cache.counters().atlas_misses, 0);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::resources::tests::manifest
~~~

Expected: compile failure for missing manifest/cache helpers.

- [ ] **Step 3: Build manifest from declared content, not current pixels**

Add a backend-neutral repertoire collector beside Smooth scene derivation. Include every pet animation slot, species/stage dialect, room dialect, owned prop/tank-life glyph, activity/particle/performance cue, Glitch replacement, chest bubble, and HUD character. Return a sorted/deduplicated Vec<GlyphKey>.

- [ ] **Step 4: Implement deterministic generation keys**

Hash the sorted manifest plus ResolvedFontPolicy, backing scale, packing version, and shader resource version. Compile complete atlas metadata/pixels/GPU objects into CompiledRetainedResources before replacing the active generation. On failure, retain the previous generation and report explicit fallback if it cannot render the frame.

- [ ] **Step 5: Add deterministic strips**

Reuse existing pet animation/render fixtures to cover every species, stage, normal/asleep/helper trouble/activity state, blink/gesture, Glitch corruption, particles, and changing HUD digits. Execute each qualified repertoire at logical sizes 260, 360, 480, and 720 and backing scales 1 and 2:

~~~rust
for logical_size in [260, 360, 480, 720] {
    for backing_scale in [1.0, 2.0] {
        let manifest = GlyphRepertoireManifest::for_fixture_pet_at(
            logical_size,
            backing_scale,
        );
        let mut cache = TestResourceCache::activate(manifest);
        for frame in deterministic_full_strip_at(logical_size, backing_scale) {
            cache.prepare(&frame).unwrap();
        }
        assert_eq!(cache.counters().atlas_builds_after_activation, 0);
        assert_eq!(cache.counters().atlas_uploads_after_activation, 0);
        assert_eq!(cache.counters().atlas_misses, 0);
    }
}
~~~

- [ ] **Step 6: Run checks**

~~~bash
cargo test --features retained-renderer companion::retained::resources
cargo test --test smooth_companion
~~~

Expected: full strips pass with zero post-activation atlas builds/uploads/misses.

- [ ] **Step 7: Commit**

~~~bash
git add src/companion/retained/resources.rs src/companion/app.rs src/round/smooth.rs
git commit -m "feat(renderer): preflight the dynamic glyph repertoire"
~~~

---

### Task 10: Replace per-frame GPU allocation with persistent bounded buffers

**Files:**
- Modify: src/companion/retained/resources.rs
- Modify: src/companion/retained.rs:268-355,538-697

**Interfaces:**
- Consumes: prepared GPU primitives and RetainedResourceCounters.
- Produces:
  - PersistentFrameBuffers
  - ensure_instance_capacity
  - write_frame_instances

- [ ] **Step 1: Write failing resource-counter test**

~~~rust
#[test]
fn ambient_strip_allocates_no_gpu_objects_after_warmup() {
    let mut host = TestRetainedResources::warm();
    let before = host.counters();
    for frame in deterministic_ambient_frames(300) {
        host.prepare_frame(&frame).unwrap();
    }
    let delta = host.counters() - before;
    assert_eq!(delta.buffer_creations, 0);
    assert_eq!(delta.texture_creations, 0);
    assert_eq!(delta.sampler_creations, 0);
    assert_eq!(delta.bind_group_creations, 0);
    assert_eq!(delta.pipeline_creations, 0);
    assert_eq!(delta.static_uploads, 0);
    assert!(delta.instance_writes > 0);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer ambient_strip_allocates_no_gpu_objects_after_warmup
~~~

Expected: test fails because the current render path creates a new primitive buffer.

- [ ] **Step 3: Implement persistent capacity-bounded buffers**

PersistentFrameBuffers owns a small ring of VERTEX | COPY_DST instance buffers. Grow only on a declared semantic/layout generation change, never ordinary motion. Normal frames call queue.write_buffer for the used prefix and draw only the current instance count.

The retained host also owns persistent intermediate/readback resources keyed by physical size. Resize/backing-scale change may replace them once and increments counters.

- [ ] **Step 4: Remove create_buffer_init from ordinary render**

Delete the per-frame primitive-buffer creation in RetainedHost::render. Increment instance_writes and bytes only after queue.write_buffer succeeds. Static uploads remain tied to resource generation activation.

- [ ] **Step 5: Run checks**

~~~bash
cargo test --features retained-renderer ambient_strip_allocates_no_gpu_objects_after_warmup
cargo test --features retained-renderer companion::retained
cargo clippy --lib --features retained-renderer -- -D warnings
~~~

Expected: 300-frame structural test passes; no ordinary-frame GPU object creation.

- [ ] **Step 6: Commit**

~~~bash
git add src/companion/retained/resources.rs src/companion/retained.rs
git commit -m "perf(renderer): retain bounded GPU frame resources"
~~~

---

### Task 11: Normalize color, alpha, blend, and capture output semantics

**Files:**
- Create: src/companion/retained/parity.rs
- Modify: src/companion/retained.rs:463-534,1399-1423
- Modify: src/companion/retained.wgsl
- Modify: src/companion/retained/capture.rs

**Interfaces:**
- Consumes: SmoothBlendMode, SmoothRgba8, RoundColor, atlas entry kinds.
- Produces:
  - premultiply_linear_srgb
  - BlendContract
  - canonical_png_rgba

- [ ] **Step 1: Write failing color/blend tests**

~~~rust
#[test]
fn premultiplied_linear_contract_is_explicit() {
    let color = premultiply_linear_srgb([0.5, 0.25, 0.0, 0.5]);
    assert!((color[0] - 0.107_020_57).abs() < 1e-6);
    assert!((color[1] - 0.025_438).abs() < 1e-5);
    assert_eq!(color[3], 0.5);
}

#[test]
fn blend_contract_covers_every_smooth_mode() {
    for mode in [
        SmoothBlendMode::Normal,
        SmoothBlendMode::Multiply,
        SmoothBlendMode::Screen,
        SmoothBlendMode::Add,
        SmoothBlendMode::Replace,
    ] {
        assert!(BlendContract::for_mode(mode).is_some());
    }
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::parity::tests
~~~

Expected: compile failure for missing module/contracts.

- [ ] **Step 3: Implement one GPU convention**

Authored input is straight sRGB. Convert to premultiplied linear RGBA before GPU upload. Configure pipeline equations for source-over, separable multiply, separable screen, saturating plus-lighter, and source copy. Native color atlas pixels follow the same convention; coverage masks multiply authored color and coverage.

- [ ] **Step 4: Add swatch comparator tests**

Create small opaque/translucent reference swatches through the Smooth/AppKit offscreen target and Retained target. Assert declared per-channel/alpha tolerances in canonical RGBA output. Include gradient endpoints/midpoints.

- [ ] **Step 5: Canonicalize PNG output**

Before writing PNG, convert premultiplied linear readback to straight RGBA8 sRGB, preserve top-left orientation, and emit sRGB metadata. Tank falloff remains its documented output-space dither exception.

- [ ] **Step 6: Run checks**

~~~bash
cargo test --features retained-renderer companion::retained::parity
cargo test --features retained-renderer companion::retained::capture
cargo clippy --lib --features retained-renderer -- -D warnings
~~~

Expected: blend swatches and canonical PNG tests pass.

- [ ] **Step 7: Commit**

~~~bash
git add src/companion/retained/parity.rs src/companion/retained.rs src/companion/retained.wgsl src/companion/retained/capture.rs
git commit -m "fix(renderer): normalize retained color and blending"
~~~

---

### Task 12: Finish physical-pixel antialiasing and shared composition geometry

**Files:**
- Modify: src/companion/retained/parity.rs
- Modify: src/companion/retained.rs:710-1110
- Modify: src/companion/retained.wgsl:70-156
- Modify: src/companion/app.rs:1454-1615,2355-2490
- Modify: src/round/hud.rs

**Interfaces:**
- Consumes: shared tank/HUD/gauge geometry and BlendContract.
- Produces:
  - analytic_coverage
  - PreparedHudLayout
  - shared gauge/tank parity samples

- [ ] **Step 1: Write failing geometry/coverage tests**

Cover zero/partial/full/overage gauges, HUD centered run bounds, tank core/mid/rim one-output-level samples, and scale-1/scale-2 aperture/ellipse/arc/cap coverage continuity.

~~~rust
#[test]
fn analytic_edge_coverage_is_continuous_across_one_physical_pixel() {
    let samples = [-0.75, -0.25, 0.25, 0.75]
        .map(|distance| analytic_coverage(distance, 1.0));
    assert!(samples.windows(2).all(|pair| pair[0] >= pair[1]));
    assert!(samples[0] > samples[3]);
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer companion::retained::parity
~~~

Expected: new coverage/shared-layout tests fail against hard-discard/current duplicated math.

- [ ] **Step 3: Share prepared composition data**

Extract backend-neutral HUD run origins/bounds, gauge lane/overage geometry, and tank interpolation/dither samples. Smooth and Retained consume the same prepared values; native drawing remains backend-specific.

- [ ] **Step 4: Replace hard edge discards**

Use WGSL fwidth/smoothstep analytic coverage for aperture, ellipses, arcs, round caps, and ellipse clips. Multiply nested clip and primitive coverage. Preserve glyph atlas coverage rather than hard thresholding.

- [ ] **Step 5: Run checks**

~~~bash
cargo test --features retained-renderer companion::retained::parity
cargo test --features retained-renderer companion::app
cargo test --test round_scene
cargo test --test smooth_companion
~~~

Expected: parity math and existing Smooth/round regressions pass.

- [ ] **Step 6: Commit**

~~~bash
git add src/companion/retained/parity.rs src/companion/retained.rs src/companion/retained.wgsl src/companion/app.rs src/round/hud.rs
git commit -m "fix(renderer): match Smooth composition edges"
~~~

---

### Task 13: Complete bounded fault injection and acknowledged Smooth fallback

**Files:**
- Modify: src/companion/retained/presentation.rs
- Modify: src/companion/retained.rs
- Modify: src/companion/app.rs:481-505,946-1025,1173-1207
- Modify: src/companion/review_capture.rs
- Modify: src/cli.rs

**Interfaces:**
- Consumes: FrameProgress, GpuErrorMailbox, RendererRuntimeState.
- Produces:
  - RetainedFaultInjection
  - request_fallback
  - acknowledge_smooth_paint

- [ ] **Step 1: Write failing state-transition tests**

~~~rust
#[test]
fn initialization_failure_is_labeled_and_smooth_paint_is_acknowledged() {
    let mut state = RendererRuntimeState::fixture_retained();
    state.request_fallback(RetainedFailureCategory::DeviceUnavailable);
    assert_eq!(state.disposition(), FrameDisposition::FallbackPending(
        RetainedFailureCategory::DeviceUnavailable,
    ));
    state.acknowledge_smooth_paint();
    assert_eq!(state.disposition(), FrameDisposition::FallbackPainted(
        RetainedFailureCategory::DeviceUnavailable,
    ));
}
~~~

- [ ] **Step 2: Verify failure**

~~~bash
cargo test --features retained-renderer fallback_is_labeled_and_smooth_paint
~~~

Expected: compile/test failure until pending/painted acknowledgement exists.

- [ ] **Step 3: Add hidden bounded fault options**

Support initialization, surface loss, validation, internal, out-of-memory, device loss, resource failure, unsupported raster, map failure, blank capture, and write failure. Keep categories static and privacy-safe. Fault controls are hidden and compiled only with retained-renderer plus dev-preview/test support.

- [ ] **Step 4: Perform fallback only on the main thread**

The wgpu callback writes to GpuErrorMailbox. ui_tick drains it, takes the ActiveRetainedHost, restores AppKit, changes effective renderer to Smooth, requests display, and records FallbackPending. draw_scene records FallbackPainted after a successful Smooth paint.

- [ ] **Step 5: Make capture failures nonzero without forcing runtime fallback**

Readback/map/write/blank failure marks the pair manifest failed and returns a process error after bounded app termination. It does not change effective renderer unless the presentation itself failed.

- [ ] **Step 6: Run bounded injections**

Run focused unit tests plus one direct automatically exiting native injection for initialization failure and one for readback failure. Expected: init fault exits after acknowledged Smooth paint; readback fault exits nonzero with effective Retained and failed capture status.

- [ ] **Step 7: Commit**

~~~bash
git add src/companion/retained/presentation.rs src/companion/retained.rs src/companion/app.rs src/companion/review_capture.rs src/cli.rs
git commit -m "fix(renderer): make retained fallback observable"
~~~

---

### Task 14: Implement the exact development, CI, packaging, and release feature matrix

**Files:**
- Modify: scripts/build-macos-app-shared.mjs
- Modify: scripts/build-macos-companion-app.mjs
- Modify: scripts/test/macos-app-packaging.test.mjs
- Modify: xtask/src/lib.rs
- Modify: .github/workflows/ci.yml
- Modify: .github/workflows/publish.yml

**Interfaces:**
- Consumes: renderer capability metadata and review-pair validator.
- Produces:
  - arm64 release command with retained-renderer
  - Intel Smooth-only release command
  - retained-capable companion fresh
  - staged-app capability and policy smoke steps

- [ ] **Step 1: Write failing packaging assertions**

Add Node tests that require publish.yml to contain:

~~~text
darwin-arm64: --no-default-features --features retained-renderer
darwin-x64: --no-default-features with no retained-renderer
~~~

Add xtask tests expecting companion_fresh_steps on Apple Silicon to pass --features retained-renderer to the builder.
Add staged-smoke assertions that the arm64 artifact reports Retained in compiled capabilities and can run explicit Retained, while Auto is checked against the artifact's reported policy value. At this task, the source policy constant is still false, so Auto remains Smooth until Task 17. The Intel artifact must report Retained unavailable and Auto Smooth.

- [ ] **Step 2: Verify failure**

~~~bash
node --test scripts/test/macos-app-packaging.test.mjs
cargo test -p xtask companion_fresh
~~~

Expected: failures because all current release targets use the same feature command.

- [ ] **Step 3: Add explicit builder features**

buildMacosApp accepts features: string[]. When building locally it appends --features followed by the comma-joined list. build-macos-companion-app passes retained-renderer on Apple Silicon and no extra feature on Intel. An externally supplied binary is never rebuilt and records capabilities from a bounded companion-app metadata command.

- [ ] **Step 4: Update CI and publish matrix**

CI adds a native macOS retained clippy/test step. Publish assigns per-target cargo feature arguments, keeps non-macOS no-default builds unchanged, bundles the exact built binary, and runs bounded staged-app smokes before artifact upload. Encode these exact macOS Cargo commands in the target matrix:

~~~bash
cargo build --release --locked --no-default-features --features retained-renderer --target aarch64-apple-darwin
cargo build --release --locked --no-default-features --target x86_64-apple-darwin
~~~

The arm64 smoke runs the staged binary once with Auto and once with explicit Retained. The Intel smoke runs Auto and verifies that explicit Retained is rejected. The smoke validator reads requested renderer, effective renderer, compiled capabilities, and Auto policy from the binary's metadata; the final arm64 Auto -> Retained assertion is activated by the policy flip in Task 17, while explicit Retained proves the shipped capability before that flip.

- [ ] **Step 5: Validate workflow and builders**

~~~bash
node --test scripts/test/macos-app-packaging.test.mjs
cargo test -p xtask
cargo fmt --check
git diff --check
~~~

Expected: packaging tests prove per-target commands and staged artifact ordering; arm64 proves retained capability and explicit Retained without changing the still-disabled Auto policy.

- [ ] **Step 6: Commit**

~~~bash
git add scripts/build-macos-app-shared.mjs scripts/build-macos-companion-app.mjs scripts/test/macos-app-packaging.test.mjs xtask/src/lib.rs .github/workflows/ci.yml .github/workflows/publish.yml
git commit -m "build(renderer): ship retained backend on Apple Silicon"
~~~

---

### Task 15: Close the representative live 360 parity gate

**Files:**
- Create: docs/superpowers/measurements/2026-07-11-glorp-retained-360-parity.md

**Interfaces:**
- Consumes: cargo xtask companion review-pair and the live visual oracle.
- Produces: accepted 360 Smooth/Retained pair and a privacy-safe review record.

- [ ] **Step 1: Generate one redacted live pair**

~~~bash
cargo xtask companion review-pair --size 360 --out target/glorp-review/live-360
~~~

Expected: validated nonblank pair, matching checksum/frame/generation IDs, no fallback, no post-activation atlas churn.

- [ ] **Step 2: Inspect the pair**

Compare pet identity, glyph baseline/bearing, props, tank life, bed, shadow, projection, tank falloff, aura, HUD, gauges, blend order, clipping, and edge coverage. Do not compare private strings in the measurement document.

- [ ] **Step 3: Handle rejection with a focused loop**

If Drew rejects a concrete difference, stop this gate and return to the owning Task 8-12. Add one failing geometry/resource/shader test naming that difference, verify failure, apply the smallest fix there, rerun its focused tests, and regenerate only the 360 pair. Resume Task 15 only after the focused fix is committed. Do not start the four-size matrix until Drew accepts 360.

- [ ] **Step 4: Record accepted evidence**

The measurement document records commit, commands, artifact-relative paths, frame checksum, sizes/backing scale, resource counters, fallback state, and Drew's approval. It contains no exact HUD text or live values.

- [ ] **Step 5: Commit**

~~~bash
git add docs/superpowers/measurements/2026-07-11-glorp-retained-360-parity.md
git commit -m "docs(renderer): accept retained 360 parity"
~~~

---

### Task 16: Run the final one-shot matrix and rehearse Smooth-default rollback

**Files:**
- Create: docs/superpowers/measurements/2026-07-11-glorp-retained-cutover-gate.md

**Interfaces:**
- Consumes: accepted 360 pair, bounded fault controls, staged app builders.
- Produces: final visual/native gate record and no-publish rollback artifact evidence.

- [ ] **Step 1: Run focused automated gates**

~~~bash
cargo fmt --check
cargo clippy --all-targets --features retained-renderer -- -D warnings
cargo test --features retained-renderer
cargo test --test round_scene
cargo test --test smooth_companion
cargo test --test retained_renderer_boundary
node --test scripts/test/macos-app-packaging.test.mjs
cargo test -p xtask
git diff --check
~~~

Expected: all commands exit 0. This is not a CPU/energy/package/build-time qualification run.

- [ ] **Step 2: Run the one-shot capture matrix**

Generate paired normal and dimmed captures from the frozen current companion state. Each command validates its pair manifest automatically:

~~~bash
cargo xtask companion review-pair --size 260 --out target/glorp-review/final-260-normal
cargo xtask companion review-pair --size 260 --dimmed --out target/glorp-review/final-260-dimmed
cargo xtask companion review-pair --size 360 --out target/glorp-review/final-360-normal
cargo xtask companion review-pair --size 360 --dimmed --out target/glorp-review/final-360-dimmed
cargo xtask companion review-pair --size 480 --out target/glorp-review/final-480-normal
cargo xtask companion review-pair --size 480 --dimmed --out target/glorp-review/final-480-dimmed
cargo xtask companion review-pair --size 720 --out target/glorp-review/final-720-normal
cargo xtask companion review-pair --size 720 --dimmed --out target/glorp-review/final-720-dimmed
~~~

Do not repeat accepted sizes without a concrete defect.

- [ ] **Step 3: Run bounded native smokes**

Exercise resize, backing-scale change, minimize/restore, occlusion, input, focus/accessibility, initialization failure, surface/device failure, unsupported content, capture failure, explicit Smooth, and explicit Retained. Every app exits automatically.

- [ ] **Step 4: Rehearse rollback without publishing**

Build a temporary policy-only variant with Apple-Silicon Auto resolving to Smooth, stage Glorp.app, run Auto and explicit Retained smokes, and record elapsed edit-to-verified-artifact time. Do not tag, publish, or overwrite an npm version.

- [ ] **Step 5: Request Drew's final approval**

Present the eight visual pairs and the privacy-safe manifest summaries. Hard stop until Drew explicitly approves the default flip.

- [ ] **Step 6: Record and commit the gate**

~~~bash
git add docs/superpowers/measurements/2026-07-11-glorp-retained-cutover-gate.md
git commit -m "docs(renderer): record retained cutover gate"
~~~

The document records approval or rejection. If rejected, do not execute Task 17.
If any validation command fails, stop Task 16 and return to the task that owns the failing implementation; do not patch code as part of this gate-record task.

---

### Task 17: Flip Apple-Silicon Auto to Retained and verify staged artifacts

**Files:**
- Modify: src/commands/companion_mode.rs AUTO_RETAINED_ON_APPLE_SILICON
- Modify: src/commands/companion_mode.rs tests
- Modify: src/cli.rs tests
- Modify: docs/superpowers/measurements/2026-07-11-glorp-retained-cutover-gate.md

**Interfaces:**
- Consumes: explicit approval recorded by Task 16.
- Produces: Apple-Silicon Auto -> Retained; Intel Auto -> Smooth; one-line source rollback.

- [ ] **Step 1: Confirm the approval gate**

Read the Task 16 document and obtain Drew's explicit approval in the active session. If either is absent, stop.

- [ ] **Step 2: Change the single policy constant**

~~~rust
pub const AUTO_RETAINED_ON_APPLE_SILICON: bool = true;
~~~

Do not delete Smooth or change Intel resolution.

- [ ] **Step 3: Update policy expectations**

The Apple-Silicon capable Auto test now expects Retained. Intel Auto still expects Smooth. Explicit Classic/Pixel/Smooth/Retained tests remain unchanged.

- [ ] **Step 4: Re-run the bounded regression gate**

~~~bash
cargo fmt --check
cargo clippy --all-targets --features retained-renderer -- -D warnings
cargo test --features retained-renderer commands::companion_mode
cargo test --features retained-renderer --test cli_smoke
cargo test --features retained-renderer companion::retained
cargo test --test round_scene
cargo test --test smooth_companion
node --test scripts/test/macos-app-packaging.test.mjs
cargo test -p xtask
git diff --check
~~~

Expected: all commands exit 0; no automatic performance/energy/package matrix runs.

- [ ] **Step 5: Build and smoke the staged arm64 app**

Build the exact release artifact command from the spec, package Glorp.app, run bounded Auto and explicit Retained smokes, and validate effective Retained in both manifests. The validator introduced in Task 14 now observes the flipped policy from binary metadata; no workflow edit is required. Validate the Intel artifact's capability manifest reports Auto Smooth and Retained unavailable.

- [ ] **Step 6: Commit the reversible flip**

~~~bash
git add src/commands/companion_mode.rs src/cli.rs docs/superpowers/measurements/2026-07-11-glorp-retained-cutover-gate.md
git commit -m "feat(companion): default to Metal on Apple Silicon"
~~~

- [ ] **Step 7: Report rollback command and remaining deferred work**

Report the one-line policy rollback, staged-artifact evidence, Smooth availability, and the deferred persistent scene-graph/font follow-on. Do not publish unless Drew separately requests the release procedure.
