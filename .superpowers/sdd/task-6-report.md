# Task 6 Report: Smooth AppKit Motion And Review Capture

## Files Changed

- `Cargo.toml`
- `src/cli.rs`
- `src/commands/companion.rs`
- `src/commands/companion_mode.rs`
- `src/companion/app.rs`
- `src/companion/mod.rs`
- `src/companion/review_capture.rs`
- `src/lib.rs`
- `tests/cli_smoke.rs`

## RED Evidence

Command:

```bash
cargo test --test cli_smoke companion_review -- --nocapture
```

Result: failed as expected before production changes. All three new focused smoke tests failed because clap rejected the missing hidden review flags:

- `--review-state` was an unexpected argument for `companion`
- `--review-state` was an unexpected argument for `companion-app`
- `--review-duration-ms` was an unexpected argument when using legacy `--review-active-pulse`

Summary from RED run: `0 passed; 3 failed; 20 filtered out`.

## GREEN Evidence

Command:

```bash
cargo test --test cli_smoke companion_review -- --nocapture
```

Result: passed after implementation. Summary: `3 passed; 0 failed; 20 filtered out`.

Command:

```bash
cargo build
```

Result: passed after implementation. Summary: `Finished dev profile`.

Command:

```bash
cargo fmt --check
```

Result: passed after running `cargo fmt`.

Additional focused unit checks:

```bash
cargo test --lib review_state -- --nocapture
cargo test --lib legacy_active_pulse -- --nocapture
```

Result: review-state precedence and legacy active-pulse mapping tests passed.

## Native Capture Evidence

Command:

```bash
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/smooth-360-active
```

Result: exited 0.

Artifacts written:

- `target/glorp-review/smooth-360-active/screenshot.png`
- `target/glorp-review/smooth-360-active/render-log.json`

Fresh `render-log.json` facts:

- `renderer`: `smooth`
- `review_state`: `active-pulse`
- `requested_size`: `360x360`
- `frame_count`: `59`
- `elapsed_duration_ms`: `2036`
- `smooth_bob_samples`: `59` samples, `59` unique values
- first/last bob samples changed from `0.029` to `-0.0679`
- `panic`: `false`

Fresh screenshot fact:

- `screenshot.png`: PNG image data, `720 x 720`, RGBA

## Self-Review Notes

- Classic remains the default renderer.
- Pixel remains on its separate renderer path.
- Smooth still uses `build_round_smooth_scene_plan(...)`.
- Smooth AppKit rendering now walks smooth layers in z order; non-`PetBody` local cells render at rounded integer cell positions through the same AppKit cell drawing helper, while `PetBody` local cells use fractional `transform.translation.x/y` AppKit coordinates.
- HUD, gauges, halo/trouble overlays, mood aura, dim overlay, and aperture clipping remain in the existing AppKit composition.
- Review logs contain renderer, review state, requested size, frame count, elapsed duration, bob samples, and panic flag only. They do not include source names, prompts, diagnostics, raw file paths, or user data.
- Native capture initially found a real re-entrant AppKit bug: screenshot capture held a mutable `APP_STATE` borrow while `displayIfNeeded()` re-entered `drawRect`. The fix takes the capture session out of state before invoking screenshot/log writes.

## Commit SHA(s)

- Final commit SHA is reported in the final response after commit creation.

## Concerns

## Final Review Fix

### Files Changed

- `src/companion/app.rs`
- `.superpowers/sdd/task-6-report.md`

### RED Evidence

Command:

```bash
cargo test --lib review_state -- --nocapture
```

Result before the fix: failed for the targeted drift behavior after post-poll updates. Summary: `2 passed; 3 failed; 831 filtered out`.

Representative failures:

- `post_poll_review_state_active_pulse_reapplies_after_live_update`: `left: None`, `right: Some(2026-07-08 12:00:00.0 +00:00:00)`
- `post_poll_review_state_asleep_calm_reapplies_after_live_update`: `assertion failed: vm.day_context.asleep`
- `post_poll_review_state_helper_trouble_reapplies_after_live_update`: `left: Ready`, `right: Diagnostic`

### GREEN Evidence

Focused check:

```bash
cargo test --lib review_state -- --nocapture
```

Result after the fix: passed. Summary: `5 passed; 0 failed; 831 filtered out`.

Required checks:

```bash
cargo build
cargo fmt --check
```

Results:

- `cargo build`: passed (`Finished dev profile`)
- `cargo fmt --check`: passed

Native review checks:

```bash
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/smooth-360-active
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 12000 --review-capture-dir target/glorp-review/smooth-360-active-12s
```

Results:

- Required 2s capture exited 0 with `review_state: active-pulse`, `frame_count: 59`, `elapsed_duration_ms: 2050`, `panic: false`
- Extra 12s capture exited 0 across the 10s poll boundary with `review_state: active-pulse`, `frame_count: 359`, `elapsed_duration_ms: 12037`, `panic: false`

### Commit SHA

- Placeholder: `PENDING_COMMIT_SHA`

### Concerns

- The focused regression tests directly cover the bug path, but the native capture logs do not expose a fixture-specific boolean for post-poll pulse persistence, so runtime verification here is exit-status plus review-state metadata rather than a stronger artifact assertion.

- No functional concerns from the final checks.
- The report cannot contain its own final commit SHA without changing the commit hash; final response records the actual commit SHA.

## Fix Follow-up

## Files Changed

- `src/companion/review_capture.rs`
- `src/companion/app.rs`
- `src/commands/companion.rs`
- `.superpowers/sdd/task-6-report.md`

## RED Evidence

Command:

```bash
cargo test --lib review_open_command_forwards_state_duration_and_capture_dir -- --nocapture
```

Result before implementation: failed to compile as expected because the new review-capture tests referenced missing `ReviewCapture::writes_artifacts` and `ReviewCapture::redacts_live_hud` methods.

Representative error:

```text
error[E0599]: no method named `writes_artifacts` found for struct `review_capture::ReviewCapture`
error[E0599]: no method named `redacts_live_hud` found for struct `review_capture::ReviewCapture`
```

Additional native RED during verification:

```bash
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/smooth-360-active
```

Visual screenshot inspection initially showed live HUD token strings (`1.9B`, `79% yday`, `1.8M/10m`). Root cause: `finish_review_capture_if_due` takes the capture session out of `APP_STATE` before `displayIfNeeded()`, so the forced screenshot redraw no longer saw capture mode.

## GREEN Evidence

Focused checks:

```bash
cargo test --lib review_capture -- --nocapture
cargo test --lib review_open_command_forwards_state_duration_and_capture_dir -- --nocapture
```

Result: passed. `review_capture` ran 3 tests covering duration-only sessions, artifact redaction mode, and redacted HUD text; command forwarding ran 1 test covering `--review-state`, `--review-duration-ms`, and `--review-capture-dir`.

Required checks:

```bash
cargo test --test cli_smoke companion_review -- --nocapture
cargo build
cargo fmt --check
```

Results:

- `cli_smoke companion_review`: `3 passed; 0 failed; 20 filtered out`
- `cargo build`: passed
- `cargo fmt --check`: passed after running `cargo fmt`

Duration-only native check:

```bash
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 500
```

Result: exited 0 on its own without `--review-capture-dir`.

## Native Capture Evidence

Command:

```bash
cargo run -- companion-app --renderer smooth --review-size 360x360 --review-state active-pulse --review-duration-ms 2000 --review-capture-dir target/glorp-review/smooth-360-active
```

Result: exited 0.

Artifact facts:

- `target/glorp-review/smooth-360-active/screenshot.png` exists, `720 x 720`
- `target/glorp-review/smooth-360-active/render-log.json` exists
- `frame_count`: `59`
- `elapsed_duration_ms`: `2043`
- `smooth_bob_samples`: `59` samples, `59` unique values
- first/last bob samples changed from `0.028` to `-0.0679`
- `panic`: `false`

Privacy verification:

- Final screenshot visual inspection shows deterministic redacted HUD text: `review`, `privacy`, `redacted`.
- It no longer shows live HUD token strings from the first failed artifact (`1.9B`, `79% yday`, `1.8M/10m`).
- `render-log.json` remains limited to renderer, review state, requested size, frame count, elapsed duration, bob samples, and panic flag.

## Fix Details

- Duration-only review sessions now create a `ReviewCapture` without artifact output, allowing the app to terminate after `review_duration_ms` even when no capture directory is supplied.
- Artifact-producing review sessions now request live HUD redaction.
- The app stores redaction as persistent app state so the final screenshot redraw remains redacted even after the capture session is temporarily taken out of state for safe AppKit screenshot writing.
- Normal live companion HUD behavior is unchanged when not writing review artifacts.

## Commit SHA(s)

- Pending until commit creation. Final response records the actual commit SHA because adding the SHA to this file before commit would change the SHA.

## Concerns

- No functional concerns from the final checks.
