# Glorp Retained Renderer Fail-Visible Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove every automatic retained-to-Smooth transition while preserving explicit Smooth launches and the retained renderer's bounded internal recovery.

**Architecture:** Renderer selection remains immutable in `RendererRuntimeState`; a separate one-way terminal-failure value records retained health. Startup failures return immediately, while runtime failures leave the retained host installed and stop later retained work so its last presented frame remains visible. Dev-only bounded review runs exit nonzero after recording truthful retained evidence rather than hanging or manufacturing a Smooth paint.

**Tech Stack:** Rust, AppKit/Objective-C bindings, wgpu/Metal retained host, Cargo integration tests, Rust `xtask` fault-soak harness.

## Global Constraints

- Smooth remains available through explicit `--renderer smooth` only.
- Retained surface/device recovery remains unchanged until it escalates to the host boundary.
- A terminal failure is idempotent, retains the first sanitized `RetainedFailureCategory`, and never tears down the retained host.
- Shipping runtime failures freeze until relaunch; dev-only bounded review processes exit nonzero after writing available evidence.
- Existing paired/direct capture schema fields for fallback remain present and report zero/none.

---

### Task 1: Make renderer health independent from renderer selection

**Files:**
- Modify: `src/commands/companion_mode.rs:640-715,1185-1270`

**Interfaces:**
- Consumes: `RetainedFailureCategory` and `FrameDisposition::Failed` from `crate::companion::retained`.
- Produces: `RendererRuntimeState::record_terminal_failure(category) -> bool`, `terminal_failure() -> Option<RetainedFailureCategory>`, and `disposition() -> FrameDisposition`.

- [ ] **Step 1: Replace the acknowledged-fallback unit test with a failing sticky-failure test**

```rust
#[test]
fn terminal_failure_preserves_retained_selection_and_first_category() {
    use crate::companion::retained::{FrameDisposition, RetainedFailureCategory};

    let mut state = RendererRuntimeState::fixture_retained();
    assert!(state.record_terminal_failure(RetainedFailureCategory::DeviceUnavailable));
    assert!(!state.record_terminal_failure(RetainedFailureCategory::DeviceValidation));
    assert_eq!(state.effective(), EffectiveCompanionRenderer::Retained);
    assert_eq!(state.terminal_failure(), Some(RetainedFailureCategory::DeviceUnavailable));
    assert_eq!(state.disposition(), FrameDisposition::Failed(RetainedFailureCategory::DeviceUnavailable));
    assert_eq!(state.transition_count(), 0);
    assert_eq!(state.last_fallback_reason(), None);
}
```

Delete `fallback_preserves_requested_renderer` because changing the effective renderer is no longer valid behavior.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test --all-features commands::companion_mode::tests::terminal_failure_preserves_retained_selection_and_first_category -- --exact`

Expected: compilation fails because `record_terminal_failure` and `terminal_failure` do not exist.

- [ ] **Step 3: Replace mutable fallback state with sticky retained health**

Use this state and behavior in `RendererRuntimeState`:

```rust
pub struct RendererRuntimeState {
    requested: CompanionRendererRequest,
    effective: EffectiveCompanionRenderer,
    #[cfg(all(target_os = "macos", feature = "retained-renderer"))]
    terminal_failure: Option<RetainedFailureCategory>,
}

pub fn transition_count(&self) -> u64 { 0 }
pub fn last_fallback_reason(&self) -> Option<&'static str> { None }

#[cfg(all(target_os = "macos", feature = "retained-renderer"))]
pub(crate) fn record_terminal_failure(&mut self, category: RetainedFailureCategory) -> bool {
    if self.terminal_failure.is_some() {
        false
    } else {
        self.terminal_failure = Some(category);
        true
    }
}

#[cfg(all(target_os = "macos", feature = "retained-renderer"))]
pub(crate) fn terminal_failure(&self) -> Option<RetainedFailureCategory> {
    self.terminal_failure
}

#[cfg(all(target_os = "macos", feature = "retained-renderer"))]
pub(crate) fn disposition(&self) -> FrameDisposition {
    self.terminal_failure
        .map(FrameDisposition::Failed)
        .unwrap_or(FrameDisposition::SurfacePresentCalled)
}
```

Initialize `terminal_failure` to `None`. Delete `fallback_to_smooth`, `request_fallback`, and `acknowledge_smooth_paint`; no code may mutate `effective` after construction.

- [ ] **Step 4: Run the focused renderer-state tests and verify GREEN**

Run: `cargo test --all-features commands::companion_mode::tests -- --nocapture`

Expected: all companion-mode tests pass; no test expects Smooth after retained failure.

- [ ] **Step 5: Commit Task 1**

```bash
git add src/commands/companion_mode.rs
git commit -m "refactor(companion): make retained failure sticky"
```

### Task 2: Fail retained startup and freeze terminal runtime failures

**Files:**
- Modify: `src/companion/app.rs:663-720,786-801,1295-1510,1710-1960,2320-2385,2525-2630,2870-2940,3205-3240,4445-4840`
- Modify: `tests/retained_scene.rs:55-85`
- Modify: `tests/retained_renderer_boundary.rs:465-490,860-900`

**Interfaces:**
- Consumes: Task 1's sticky terminal-failure API.
- Produces: `retained_startup_failure(category) -> GlorpError`, `freeze_retained(error)`, and an event-loop guard that suppresses all post-failure scene updates/presents.

- [ ] **Step 1: Add failing tests for startup errors and the terminal-work guard**

Add pure helpers and tests with these contracts:

```rust
#[test]
fn retained_startup_error_keeps_the_sanitized_category() {
    let error = retained_startup_failure(RetainedFailureCategory::DeviceUnavailable);
    assert_eq!(error.to_string(), "retained renderer initialization failed (retained-device-unavailable)");
}

#[test]
fn terminal_retained_failure_stops_future_render_work() {
    let mut runtime = RendererRuntimeState::fixture_retained();
    assert!(retained_work_allowed(&runtime));
    runtime.record_terminal_failure(RetainedFailureCategory::DeviceUnavailable);
    assert!(!retained_work_allowed(&runtime));
    assert_eq!(runtime.effective(), EffectiveCompanionRenderer::Retained);
}
```

Update source-boundary tests to forbid `request_fallback`, `restore_appkit`, `new_round_view`, and `acknowledge_smooth_paint` in the terminal failure path, while requiring `record_terminal_failure` and retention of `retained_host`.

- [ ] **Step 2: Run the focused app/boundary tests and verify RED**

Run: `cargo test --all-features companion::app::tests::retained_startup_error_keeps_the_sanitized_category -- --exact`

Run: `cargo test --all-features companion::app::tests::terminal_retained_failure_stops_future_render_work -- --exact`

Run: `cargo test --all-features --test retained_scene --test retained_renderer_boundary`

Expected: focused helpers are missing and the boundary tests still find Smooth fallback construction.

- [ ] **Step 3: Make startup failures return instead of selecting Smooth**

Add:

```rust
#[cfg(feature = "retained-renderer")]
fn retained_startup_failure(category: RetainedFailureCategory) -> GlorpError {
    GlorpError::Message(format!(
        "retained renderer initialization failed ({})",
        category.category()
    ))
}
```

For both injected initialization failure and `PreparedRetainedHost` activation failure, emit the existing sanitized boundary diagnostic and `return Err(retained_startup_failure(category))`. Remove `needs_initial_smooth_frame`; a retained launch without a host is no longer a valid running state.

- [ ] **Step 4: Replace host teardown with an idempotent freeze**

Delete `ColdSmoothFallbackGate`, `prepare_cold_smooth_fallback_once`, and both `drive_smooth_fallback_paint` variants. Replace `fallback_from_retained` with:

```rust
#[cfg(feature = "retained-renderer")]
fn freeze_retained(error: RetainedFailureCategory) {
    let exit_review = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else { return false };
        if !state.renderer_runtime.record_terminal_failure(error) {
            return false;
        }
        write_boundary_diagnostic(format_args!(
            "glorp retained renderer frozen after terminal failure: {}\n",
            error.category()
        ));
        state.review_capture.is_some() || state.runtime_metrics_out.is_some()
    });
    if exit_review {
        finish_terminal_retained_review();
    }
}
```

`finish_terminal_retained_review` writes live retained metrics when requested and exits nonzero for the dev/test bounded review. It never removes `retained_host`, restores AppKit, replaces `RoundView`, prepares a Smooth model, or asks AppKit to paint Smooth.

Route live scene errors, legacy present errors, watchdog exhaustion, and resource-preparation failure-without-active to `freeze_retained`. Keep shadow-route diagnostics unchanged because shadow does not control presentation.

- [ ] **Step 5: Stop all work after the first terminal failure**

Add:

```rust
#[cfg(feature = "retained-renderer")]
fn retained_work_allowed(runtime: &RendererRuntimeState) -> bool {
    runtime.terminal_failure().is_none()
}
```

At the first visible-tick boundary, return before animation, projection, reconciliation, resource preparation, surface resize, or present when `retained_work_allowed` is false. Leave the retained host and CAMetalLayer installed. Remove Smooth acknowledgement logic from `record_review_frame`; explicit Smooth painting remains governed solely by an initially Smooth `effective()` value.

- [ ] **Step 6: Simplify runtime metrics to require the retained host**

Remove `terminal_runtime_metrics` and `select_terminal_runtime_metrics`. `write_runtime_metrics_if_requested` now obtains the snapshot directly from `state.retained_host` and errors with `retained runtime metrics requested without a live retained host` only when no host exists. Fallback counters remain zero because `record_fallback` is never called.

- [ ] **Step 7: Run app, boundary, retained scene, and round scene tests**

Run: `cargo test --all-features commands::companion_mode::tests`

Run: `cargo test --all-features companion::app::tests`

Run: `cargo test --all-features --test retained_scene --test retained_renderer_boundary --test round_scene`

Expected: all tests pass and source-boundary tests prove there is no automatic Smooth construction.

- [ ] **Step 8: Commit Task 2**

```bash
git add src/companion/app.rs tests/retained_scene.rs tests/retained_renderer_boundary.rs
git commit -m "fix(companion): freeze retained renderer on terminal failure"
```

### Task 3: Make fault-soak evidence prove fail-visible behavior

**Files:**
- Modify: `xtask/src/lib.rs:630-710,790-830,918-980,1180-1370,2240-2360,3960-3995,4235-4285`
- Modify: `docs/superpowers/measurements/2026-07-14-glorp-direct-runtime-qualification.md:155-170,250-260`

**Interfaces:**
- Consumes: Task 2's nonzero dev-review exit, exact terminal diagnostic, retained host, and zero fallback counters.
- Produces: updated `scene-fault-soak` validation that distinguishes startup failure, terminal runtime failure, and capture failure without requiring a Smooth screenshot.

- [ ] **Step 1: Change fault-plan tests to require nonzero terminal failures**

Set `expected_success: false` for initialization and all seven runtime fault cases. Keep capture faults false. Add assertions that runtime fault plans retain `--review-runtime-metrics-out`, while initialization does not.

Change fixture evidence keys from `acknowledged_fallback_paint` to:

```rust
"terminal_retained_failure_observed": !case.is_capture_failure(),
"smooth_fallback_absent": true,
```

- [ ] **Step 2: Run xtask tests and verify RED**

Run: `cargo test -p xtask fault_plans_cover_every_existing_typed_category_with_all_features`

Run: `cargo test -p xtask fault_summary_rejects_missing_mismatched_or_unsanitized_outcomes`

Expected: assertions fail because existing cases expect success and validated evidence requires an acknowledged Smooth paint.

- [ ] **Step 3: Rewrite fault evidence validation around retained truth**

For initialization failures, require: nonzero exit, the exact sanitized diagnostic, no runtime metrics, and no direct artifacts. Do not require `render-log.json` or `screenshot.png` because no renderer host was activated.

For runtime terminal failures, require: nonzero exit, exact category, runtime metrics present, and all of `fallback_count`, `fallback_pending_transitions`, and `fallback_painted_transitions` equal zero. Do not require an AppKit paint; the retained CAMetalLayer remains installed.

For capture failures, retain the existing capture-attempt/failure and artifact checks. All cases publish `smooth_fallback_absent: true`; only initialization/runtime cases publish `terminal_retained_failure_observed: true`.

- [ ] **Step 4: Update the qualification record**

Replace the eight old “acknowledged nonblank Smooth fallback” outcomes with “nonzero fail-visible retained outcome; exact sanitized category; no Smooth transition.” Mark the prior evidence as superseded by the 2026-07-15 sticky-renderer policy rather than deleting the historical table.

- [ ] **Step 5: Run xtask tests and the bounded native fault soak**

Run: `cargo test -p xtask`

Run: `cargo xtask companion scene-fault-soak --out target/glorp-scene-gates/fail-visible-faults`

Expected: all 11 cases match their exact sanitized category; eight startup/runtime cases exit nonzero with no fallback, and three capture cases remain failed retained captures.

- [ ] **Step 6: Commit Task 3**

```bash
git add xtask/src/lib.rs docs/superpowers/measurements/2026-07-14-glorp-direct-runtime-qualification.md
git commit -m "test(companion): prove retained failures never fall back"
```

### Task 4: Final verification and manual launch

**Files:**
- Verify only; no planned production changes.

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: release bundle showing the direct retained scene with explicit Smooth still manually selectable.

- [ ] **Step 1: Run formatting, lint, and focused renderer suites**

Run: `cargo fmt --check`

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Run: `cargo test --all-features --test retained_scene --test retained_renderer_boundary --test round_scene`

Expected: every command exits zero with no warnings from clippy.

- [ ] **Step 2: Prove automatic fallback calls are gone**

Run: `rg -n "request_fallback|fallback_to_smooth|acknowledge_smooth_paint|fallback_from_retained|prepare_cold_smooth_fallback_once|drive_smooth_fallback_paint" src/companion src/commands`

Expected: no production matches.

- [ ] **Step 3: Build and launch the normal optimized companion**

Run: `cargo xtask companion fresh`

Expected: the app launches with `effective-renderer=retained` and `effective-scene-route=direct`; no renderer override is needed.

- [ ] **Step 4: Verify explicit Smooth remains available**

Run: `target/release/glorp companion-app --renderer smooth --help >/dev/null`

Expected: exit zero, proving the explicit Smooth CLI request still parses without changing the running retained instance.

- [ ] **Step 5: Commit any verification-only documentation correction**

Only if Task 4 exposes a factual documentation mismatch, stage that exact file and commit it with `docs(companion): align retained failure verification`. Otherwise do not create an empty commit.
