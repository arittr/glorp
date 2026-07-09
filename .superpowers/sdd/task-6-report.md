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

- No functional concerns from the final checks.
- The report cannot contain its own final commit SHA without changing the commit hash; final response records the actual commit SHA.
