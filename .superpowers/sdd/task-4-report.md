# Task 4 report: Make retained host activation transactional

## What I implemented

Split the one-shot `RetainedHost::new(view)` (which attached the layer to the
view *before* the fallible wgpu work) into a two-phase, transactional flow:

- **`PreparedRetainedHost::prepare(view: &NSView, mailbox: GpuErrorMailbox)`** —
  does ALL fallible GPU work against a **detached** `CAMetalLayer`: create the
  layer, set its drawable size, build the wgpu instance/surface (from the layer
  pointer)/adapter/device, wire `on_uncaptured_error` to the passed-in mailbox's
  sender, `get_default_config`, `surface.configure`, the atlas bind-group layout,
  and `create_pipelines`. It never calls `setWantsLayer`/`setLayer`. The passed-in
  mailbox is stored on the inner host (`gpu_errors`), so the Task 3 drain path is
  unchanged. Any failure here leaves the view completely untouched.
- **`PreparedRetainedHost::activate(self, view: &NSView) -> Result<ActiveRetainedHost, _>`** —
  the ONLY code that calls `view.setWantsLayer(true)` / `view.setLayer(Some(&layer))`.
  It installs the layer under a `LayerActivationGuard` (RAII). On success it
  commits the guard and returns `ActiveRetainedHost`. If a fallible post-attach
  step is ever added and fails, the dropped-uncommitted guard restores the view's
  prior AppKit layer state before the error propagates.

Supporting types (all in `retained.rs`):

- **`LayerActivationState`** — models the attach lifecycle (`attached`,
  `appkit_restored`) with `default()`, `mark_attached()`, `preflight_failed()`,
  `attached()`, `appkit_restored()`. The production guard uses it as its
  arm/rollback source of truth; `preflight_failed`/`appkit_restored` carry
  `#[allow(dead_code)]` (they model the never-attached invariant the Step-1 test
  pins and are read only by that test — matching the existing
  `FrameProgress::observed` convention in `presentation.rs`).
- **`LayerActivationGuard<'a>`** — RAII guard over an `ActivationRollback<'a>`
  (production `View(&NSView)`; test-only `TestFlag(Rc<Cell<bool>>)` gated
  `#[cfg(test)]`). Drop-before-commit rolls back: production calls
  `ActiveRetainedHost::restore_appkit(view)`; test clears the cell. `commit(self)`
  disarms. `for_test` is `#[cfg(test)]`.
- **`ActiveRetainedHost`** — owns the built, attached inner `RetainedHost` and
  derefs to it (via `Deref`/`DerefMut`), so `render` and `drain_gpu_error` read
  through transparently. `restore_appkit` (idempotent) moved here from
  `RetainedHost`.

`RetainedHost` stays the single GPU-holding struct; `PreparedRetainedHost` and
`ActiveRetainedHost` each wrap one `host: RetainedHost` — no duplicated GPU fields.

## app.rs startup reorder (brief Step 4)

`build_window` now runs before the retained block, and the retained
prepare→activate block runs before `review_capture`/`redacts_live_hud`.
Sequence: `build_window` (get view) → `GpuErrorMailbox::new()` →
`PreparedRetainedHost::prepare(view, mailbox).and_then(|p| p.activate(view))` →
on Ok store `ActiveRetainedHost`; on prepare-OR-activate Err write the boundary
diagnostic + `renderer_runtime.fallback_to_smooth(category)` + `None` → THEN build
`review_capture` from the now-final `renderer_runtime.effective()`. This ensures a
failed activation that flips effective to Smooth is what the review capture reads,
instead of the pre-fallback Retained value.

`AppState.retained_host` is now `Option<ActiveRetainedHost>`; the render call
site (`state.retained_host.as_mut()?.render(...)` + `drain_gpu_error()`) works
unchanged through `Deref`/`DerefMut`. `fallback_from_retained` now calls
`ActiveRetainedHost::restore_appkit`.

## Files changed (and why)

- `src/companion/retained.rs` — the split, the guard/state/active types.
- `src/companion/app.rs` — field type, startup reorder, fallback restore call.
- `src/companion/retained/presentation.rs` — **justified extra file**: widened
  `GpuErrorMailbox` struct + `new()` from `pub(super)` to `pub(crate)`. The brief's
  `prepare(view, mailbox)` signature requires app.rs to construct the mailbox, and
  `retained.rs` now re-exports `GpuErrorMailbox` via its existing `pub(crate) use`.
  `sender`/`drain` stay `pub(super)`.

## TDD evidence

RED — added the two verbatim Step-1 tests to `retained.rs` `mod tests`, then:

```
$ cargo test --features retained-renderer companion::retained::tests::failed_preflight
error[E0432]: unresolved imports `super::LayerActivationGuard`, `super::LayerActivationState`
    --> src/companion/retained.rs:1488:9
     | no `LayerActivationState` in `companion::retained`
     | no `LayerActivationGuard` in `companion::retained`
error: could not compile `glorp` (lib test) due to 1 previous error
```

This is the expected failure (Step 2: "compile failure for missing activation
types") — the tests reference types that do not exist yet.

GREEN — after implementing the types + split:

```
$ cargo test --features retained-renderer --lib companion::retained
test companion::retained::tests::activation_guard_restores_uncommitted_attachment ... ok
test companion::retained::tests::failed_preflight_never_marks_layer_attached ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 930 filtered out
```

The guard test exercises real Drop-before-commit rollback (the cell flips
true→false); the state test exercises the real `LayerActivationState` transition.
Neither uses live GPU or an NSView.

## Verification results

Feature-ON:
- `cargo test --features retained-renderer --lib companion::retained` → 12 passed.
- `cargo test --features retained-renderer --lib companion::app` → 42 passed.
- `cargo test --features retained-renderer --lib` → 942 passed, 0 failed.
- `cargo clippy --lib --features retained-renderer -- -D warnings` → clean.
- `cargo clippy --all-targets --all-features -- -D warnings` (pre-commit gate) → clean.

Feature-OFF (default):
- `cargo build` → clean.
- `cargo clippy --lib -- -D warnings` → clean.

Formatting: `cargo fmt --check` → clean.

Transactional-invariant audit: the only `setWantsLayer(true)`/`setLayer(Some)`
callsite in `retained.rs` is inside `activate`; `prepare`'s single view-layer
touch is `setDrawableSize` on the detached layer object (not the view's layer);
`restore_appkit` remains the `setLayer(None)`/`setWantsLayer(false)` restore.

## Concerns

- **Surface validity on a detached layer.** The whole approach rests on the
  design decision that a `CAMetalLayer` created with `CAMetalLayer::new()` renders
  fine while detached from its view, and that installing it later (via `setLayer`)
  only makes it visible without invalidating the wgpu surface (the surface is
  created from the layer *pointer*, which does not change when the same layer
  object is attached to the view). I could not exercise this on live GPU in this
  environment (the guard/state tests are pure). `resize_if_needed` reconfigures the
  surface on the first `render`, providing a safety net if attachment perturbs the
  drawable size. This should be confirmed by a real companion launch.
- **`activate` currently cannot return `Err`.** Because `prepare` performs all
  fallible GPU work, `activate` has no fallible step after the attach today, so it
  always returns `Ok`. The `Result` return type and the RAII guard are the
  future-proofing the brief asked for: they make adding a fallible post-attach step
  transactional by construction. Clippy is clean on this.
- **`LayerActivationState::preflight_failed`/`appkit_restored`** are exercised only
  by the Step-1 test today (`#[allow(dead_code)]`, matching the codebase's
  `FrameProgress::observed` precedent). They model the by-construction invariant
  that a failed prepare never touches the view; production maintains that invariant
  by simply not calling `activate` when `prepare` fails.
