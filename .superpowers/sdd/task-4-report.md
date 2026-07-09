# Task 4 Report: Native Smooth Cadence and Review Capture

## What I implemented

- Added `SmoothSemanticClock` in `src/companion/smooth_timing.rs` with the required no-catch-up cadence semantics.
- Exported the new timing module from `src/companion/mod.rs`.
- Extended `src/companion/review_capture.rs` to record structured smooth frame samples instead of raw bob floats.
- Added review log fields for:
  - `semantic_art_tick_count`
  - `smooth_frame_samples`
  - privacy claims derived from `SmoothCompanionPrivacyClaims::external_companion()`
- Added checksum-stability helpers so review capture can detect art flashing within a semantic tick.
- Updated `src/companion/app.rs` so smooth mode uses two clocks:
  - a fast redraw clock via the existing 30fps UI timer
  - a semantic art clock via `SmoothSemanticClock`
- Kept `draw_scene(...)` render-only by snapshotting `smooth_semantic_art_tick_index` from app state and using it only during render capture sample creation.
- Updated smooth AppKit rendering so:
  - aura centers on `plan.pet.fractional_bounds`
  - `PetBody`, `ContactShadow`, and `PerformanceCue` render with fractional coordinates

## Tests run and results

- `cargo test companion::smooth_timing`
  - Passed: 2 tests
- `cargo test companion::review_capture::tests::smooth_review_capture -- --nocapture`
  - Passed: 2 tests
- `cargo test --test cli_smoke companion_ -- --nocapture`
  - Passed: 11 tests
- `cargo test --test smooth_companion`
  - Passed: 8 tests
- `cargo fmt --check`
  - Passed
- `git diff --check`
  - Passed

## TDD Evidence

### RED

Command:

```bash
cargo test companion::smooth_timing
cargo test companion::review_capture::tests::smooth_review_capture -- --nocapture
```

Relevant failing output:

```text
error[E0433]: cannot find type `SmoothSemanticClock` in this scope
error[E0422]: cannot find struct, variant or union type `SmoothReviewFrameSample` in this scope
error[E0599]: no method named `render_log_json_for_test` found for struct `review_capture::ReviewCapture`
error[E0599]: no method named `pet_checksums_stable_within_semantic_ticks_for_test` found for struct `review_capture::ReviewCapture`
```

Why this failure was expected:

- The tests were added first to define the new timing type, structured review sample schema, and test helpers before implementation existed.

### GREEN

Command:

```bash
cargo test companion::smooth_timing
cargo test companion::review_capture::tests::smooth_review_capture -- --nocapture
cargo test --test cli_smoke companion_ -- --nocapture
cargo test --test smooth_companion
```

Relevant passing output:

```text
test companion::smooth_timing::tests::smooth_semantic_clock_waits_until_interval_elapses ... ok
test companion::smooth_timing::tests::smooth_semantic_clock_drops_missed_intervals_instead_of_catching_up ... ok

test companion::review_capture::tests::smooth_review_capture_checksum_stability_detects_flashing ... ok
test companion::review_capture::tests::smooth_review_capture_records_semantic_ticks_anchors_and_privacy ... ok

test result: ok. 11 passed; 0 failed
test result: ok. 8 passed; 0 failed
```

## Files changed

- `src/companion/mod.rs`
- `src/companion/smooth_timing.rs`
- `src/companion/app.rs`
- `src/companion/review_capture.rs`
- `.superpowers/sdd/task-4-report.md`

## Self-review findings

- The semantic art clock now advances art state only on 250ms semantic boundaries and intentionally drops missed intervals instead of replaying them.
- Smooth redraw still happens on the fast UI timer, so fractional bob/drift motion can stay fluid even when art does not advance.
- `draw_scene(...)` stayed render-only; it reads a captured semantic tick index from app state rather than mutating animation state inside drawing.
- I did not add any new panics in `drawRect` or `draw_scene`.
- The AppKit smooth path now uses fractional placement for the pet-attached roles called out in the brief and uses the fractional pet center for aura placement.

## Issues or concerns

- None at the moment. The requested owned source files were sufficient for the implementation; the only additional file written was this task report.
