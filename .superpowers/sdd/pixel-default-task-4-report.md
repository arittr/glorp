# Task 4 Report: Preview Lab Default-Readiness Artifacts

## Status

DONE

## Scope Completed

- Added failing Task 4 Preview Lab tests first, then verified the expected RED state.
- Exported typed Pixel sidecars for Preview Lab frame scenarios:
  - `*.pixel-art.json`
  - `*.pixel-fit.json`
- Added manifest `files.pixel_art` and `files.pixel_fit` for Pixel scenarios.
- Added artifact inventory entries and review/index links for the new sidecars.
- Corrected the Fuzz S3 human-review label from `archfuzz` to `pup`.
- Added side-by-side human-review fixture content by expanding the Pixel summary text frame with:
  - fit readiness lines for min/default/large geometries
  - canonical terminal reference art for visual comparison
- Kept raw terminal art and raw seed data out of machine-readable Pixel sidecars.
- Added privacy coverage for the new sidecars.

## Files Changed

- `src/dev_preview/assets/preview.css`
- `src/dev_preview/assets/preview.js`
- `src/dev_preview/contract.rs`
- `src/dev_preview/export.rs`
- `src/dev_preview/pixel.rs`
- `src/dev_preview/scenarios.rs`
- `src/round/pixel_fit.rs`
- `tests/dev_preview.rs`

## Verification

### Red phase

Ran:

```bash
cargo test --features dev-preview --test dev_preview pixel -- --nocapture
```

Observed expected failures:

- missing `files.pixel_art` / `files.pixel_fit`
- stale `stage s3 archfuzz` label

### Green verification

Ran:

```bash
cargo test --features dev-preview --test dev_preview pixel -- --nocapture
cargo test --features dev-preview dev_preview::scenarios -- --nocapture
cargo run --features dev-preview -- dev-preview --scenario pixel --out target/glorp-preview-pixel-readiness
```

Results:

- Pixel-focused Preview Lab tests passed: `9 passed, 0 failed`
- `dev_preview::scenarios` coverage passed
- Pixel preview bundle generated successfully at `target/glorp-preview-pixel-readiness`

### Bundle spot-checks

Verified:

- `manifest.json` includes `files.pixel_art` and `files.pixel_fit` for Pixel scenarios
- `frames/pixel-fuzz-s3-content-idle.pixel-art.json` exists and excludes raw terminal art / seed data
- `frames/pixel-fuzz-s3-content-idle.pixel-fit.json` exists and reports:
  - producer `round::pixel_fit::pixel_companion_fit`
  - `body_eye_mouth_pixels = 0`
  - `translucent_effect_pixels = 0`
- `frames/pixel-fuzz-s3-content-idle.txt` includes `stage s3 pup`

## Commit

- `367c339 feat(dev-preview): export pixel readiness artifacts`

## Self-Review Notes

- The typed fit sidecar uses production `pixel_companion_fit`.
- Human-readable fit readiness lines now use the same overlap math as the typed fit artifact instead of a placeholder heuristic.
- No Task 5 review-launch flags or Task 6 measurement docs were implemented.
- No new dependencies were added.

## Concerns

None.

## Task 4 Review Fixes

### Review Finding 1: fullscreen-equivalent readiness coverage

- Extended Pixel Preview Lab fit-readiness coverage with a `fullscreen` target at `900x900`.
- Kept the readiness evaluation on production `round::pixel_fit::pixel_companion_fit`.
- Added regression coverage proving the exported Pixel preview summary includes `fit fullscreen ready`.

### Review Finding 2: strip art-reference cache reuse

- Refactored Pixel strip rendering so one `PixelArtReferenceProvider` is created per strip and reused across the frame loop.
- Kept single-frame artifact rendering behavior unchanged by routing the shared-strip path through a new internal helper.
- Added an internal unit test in `src/dev_preview/pixel.rs` that exercises a strip-like idle sequence and asserts cached pose reuse keeps provider renders below the frame count.

### Red/Green Verification

Red:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_pixel_summary_includes_fullscreen_fit_readiness -- --nocapture
cargo test --features dev-preview pixel_strip_reference_provider_reuses_cached_pose_during_sequence -- --nocapture
```

Observed expected failures:

- missing `fit fullscreen ready` line in `frames/pixel-fuzz-s3-content-idle.txt`
- strip-like sequence rendered `48` references for `48` frames

Green:

```bash
cargo fmt
cargo test --features dev-preview --test dev_preview pixel -- --nocapture
cargo test --features dev-preview dev_preview::pixel::tests::pixel_strip_reference_provider_reuses_cached_pose_during_sequence -- --nocapture
```

Results:

- Pixel-focused Preview Lab tests passed: `10 passed, 0 failed`
- Internal cache-reuse regression test passed
