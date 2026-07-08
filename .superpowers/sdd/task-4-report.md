# Task 4 Report: Cast Identity Fixtures, Matrix Grouping, And Composition Evidence

status: DONE

## Files Changed

- `src/dev_preview/pixel.rs`
- `src/dev_preview/scenarios.rs`
- `tests/dev_preview.rs`
- `.superpowers/sdd/task-4-report.md`

No `src/dev_preview/export.rs` update was needed.

## RED Evidence

Added the new cast-fixture tests first in `tests/dev_preview.rs`, then ran focused red checks.

The brief's combined command was not valid for `cargo test` because Cargo accepts one test-name filter before `--`, so I verified RED with three exact runs instead:

- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_cast_identity_writes_six_real_frame_artifacts -- --exact --nocapture`
  - Failed as expected: `missing scenario pixel-fuzz-s3-locket`
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_cast_matrix_references_real_cast_frames -- --exact --nocapture`
  - Failed as expected: `missing scenario pixel-cast-identity-matrix`
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_hero_cues_have_expected_coverage -- --exact --nocapture`
  - Failed as expected: `No such file or directory` for `frames/pixel-fuzz-s3-locket.pixel-art.json`

After the first implementation pass, the cue-coverage test still failed:

- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_hero_cues_have_expected_coverage -- --exact --nocapture`
  - Failed as expected for the narrower remaining issue: `pixel-glitch-s4-repair missing expected repair_mark`

The first full-suite run also exposed one sanitization regression:

- `cargo test --features dev-preview --test dev_preview -- --nocapture`
  - Failed at `dev_preview_pixel_artifacts_do_not_expose_raw_seed_or_private_fields`
  - Cause: the new matrix/composition scenarios emitted non-empty `review_prompts`, which violated the existing exported-artifact privacy contract.

## GREEN Evidence

Focused green checks after implementation:

- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_cast_identity_writes_six_real_frame_artifacts -- --exact --nocapture`
  - Passed: 1 passed, 0 failed
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_cast_matrix_references_real_cast_frames -- --exact --nocapture`
  - Passed: 1 passed, 0 failed
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_hero_cues_have_expected_coverage -- --exact --nocapture`
  - Passed: 1 passed, 0 failed

Formatting and diff hygiene:

- `cargo fmt --check`
  - Passed after running `cargo fmt`
- `git diff --check`
  - Passed

Required full verification:

- `cargo test --features dev-preview --test dev_preview -- --nocapture`
  - Passed: 69 passed, 0 failed

## Change Summary

- Added the six real cast identity Pixel fixtures:
  - `pixel-fuzz-s3-locket`
  - `pixel-blob-s3-body`
  - `pixel-ghost-s3-wisp`
  - `pixel-glitch-s4-repair`
  - `pixel-crystal-s5-facets`
  - `pixel-mech-s5-hardbody`
- Added `pixel-cast-identity-matrix` as a grouping frame only, with `inputs.cast_frame_ids` pointing at the six real cast frames and no `files.pixel` entry.
- Added `pixel-tank-composition` with pixel, pixel-art, pixel-fit, and pixel-composition artifacts sourced from the sanitized `PixelPetArtReference`.
- Switched `render_pixel_bundle(...)` summary lines to owned `Vec<String>` so the cast and composition fixtures can build their summary text without borrowed string arrays.
- Removed `#[allow(dead_code)]` from `pixel_composition_sidecar(...)` because Task 4 now uses it.
- Extended the exact manifest ID and artifact-file assertions in `tests/dev_preview.rs` and `src/dev_preview/scenarios.rs`.

## Self-review

- Kept `pixel-species-matrix` as the existing rendered Pixel frame and added `pixel-cast-identity-matrix` as a non-rendering grouping shell, so the matrix does not pretend to be a frame artifact.
- Kept Pixel opt-in behavior unchanged: all new fixtures live only under `PreviewSelection::Pixel` / `pixel_bundles(...)`.
- Preserved the existing IDs `pixel-fuzz-s3-content-idle`, `pixel-glitch-s4-feed-pulse`, and `pixel-species-matrix`.
- Verified the composition artifact is read-only evidence only: no runtime prop placement, tank-life mutation, sprite-sheet work, or renderer behavior changes were introduced.
- Fixed the privacy regression by leaving the new pixel scenarios' `review_prompts` empty, which keeps exported preview artifacts free of prompt text.

## Concerns

- `pixel-glitch-s4-repair` is pinned to the same deterministic art-request timestamp used by the cue-coverage reference tests so the promoted `repair_mark` cue is present reliably. That is intentionally narrow and local to this preview fixture, but it does mean this one cast fixture depends on a more specific clock than the rest of the pixel preview set.
