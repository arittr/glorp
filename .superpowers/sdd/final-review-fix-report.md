# Final Whole-Branch Review Fix Report

status: DONE

## Files Changed

- `src/presentation/pixel/art_reference.rs`
- `src/dev_preview/pixel.rs`
- `tests/pixel_art_reference.rs`
- `tests/dev_preview.rs`
- `docs/superpowers/measurements/2026-07-08-glorp-pixel-cast-identity-tank-composition-review.md`

## Findings Addressed

1. `pixel-tank-composition` now maps protected regions into the 96x96 Pixel
   preview coordinate space using `PixelPetScene` geometry and the same
   center/origin/scale math used by `render_pixel_frame`.
2. Composition evidence explicitly records prop and tank-life context as
   deferred/unavailable for the current Pixel runtime instead of implying a live
   comparison happened.
3. Foot-contact promotion now preserves existing `Corruption`, `Pattern`, and
   `Accent` visible roles after signature promotion.
4. Preview Lab now asserts the six cast `.pixel.json` frame payloads are
   distinct.
5. The measurement note records the revised composition evidence semantics while
   keeping manual visual review pending and Pixel opt-in.

## Red Test Summary

- `cargo test foot_contact_promotion_preserves_existing_species_and_accent_roles -- --nocapture`
  - Failed before implementation as expected: `Corruption` was promoted to
    `FootContact`.
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_composition_artifact_has_own_manifest_slot -- --exact --nocapture`
  - Failed before implementation as expected: unavailable prop comparison was
    not explicit.
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_cast_identity_frames_are_distinct -- --exact --nocapture`
  - Passed before implementation because the current six cast frames were
    already distinct. The new test still adds the missing gate and would fail if
    the six fixtures regressed to the same generic pixel payload.

## Green Verification Summary

- `cargo test --test pixel_art_reference -- --nocapture`
  - Passed: 14 passed, 0 failed.
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_composition_artifact_has_own_manifest_slot -- --exact --nocapture`
  - Passed: 1 passed, 0 failed.
- `cargo test promotion -- --nocapture`
  - Passed: 2 focused promotion tests passed, 0 failed.
- `cargo test --features dev-preview --test dev_preview dev_preview_pixel_cast_identity_frames_are_distinct -- --exact --nocapture`
  - Passed: 1 passed, 0 failed.
- `cargo test --features dev-preview --test dev_preview -- --nocapture`
  - Passed: 70 passed, 0 failed.
- `cargo fmt --check`
  - Passed.
- `git diff --check`
  - Passed.

## Concerns

- Manual visual review remains pending, and this fix does not claim Pixel
  default readiness.
