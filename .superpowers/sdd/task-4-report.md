# Task 4 Report: Preview Lab Smooth Scenario

status: DONE

## Files Changed

- `src/cli.rs`
- `src/commands/dev_preview.rs`
- `src/dev_preview/contract.rs`
- `src/dev_preview/export.rs`
- `src/dev_preview/mod.rs`
- `src/dev_preview/pixel.rs`
- `src/dev_preview/scenarios.rs`
- `src/dev_preview/smooth.rs`
- `src/dev_preview/strips.rs`
- `tests/dev_preview.rs`
- `.superpowers/sdd/task-4-report.md`

`src/dev_preview/pixel.rs` and `src/dev_preview/strips.rs` only changed to populate the new `PreviewStripFrameFiles::smooth_motion` field with `None` for existing strip families.

## RED Evidence

Added the smooth-preview tests first in `tests/dev_preview.rs`, then ran the required red command:

- `cargo test --features dev-preview --test dev_preview dev_preview_smooth`
  - Failed as expected with:
    - `invalid value 'smooth' for '--scenario <SCENARIO>'`
    - current possible values were only `all, watch, pets, props, animation, round, tank-life, pixel`

That established the first missing seam cleanly at the CLI/scenario plumbing layer before any production changes.

## GREEN Evidence

Required focused test:

- `cargo test --features dev-preview --test dev_preview dev_preview_smooth`
  - Passed: `3 passed; 0 failed`

Required bundle generation:

- `cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview`
  - Passed and wrote the smooth preview bundle to `target/glorp-preview`

Required formatting check:

- `cargo fmt --check`
  - Failed once on rustfmt wrapping
  - Ran `cargo fmt`
  - Re-ran `cargo fmt --check`
  - Passed

Diff hygiene:

- `git diff --check`
  - Passed

## Generated Artifact Notes

Generated smooth frame artifacts:

- `target/glorp-preview/frames/round-smooth-classic-baseline.txt`
- `target/glorp-preview/frames/round-smooth-classic-baseline.cells.json`
- `target/glorp-preview/frames/round-smooth-classic-parity.txt`
- `target/glorp-preview/frames/round-smooth-classic-parity.cells.json`
- `target/glorp-preview/frames/round-smooth-classic-parity.smooth-plan.json`
- `target/glorp-preview/frames/round-smooth-classic-parity.smooth-parity.json`

Generated smooth strip artifacts:

- `target/glorp-preview/strips/round-smooth-motion/frame-000.txt`
- `target/glorp-preview/strips/round-smooth-motion/frame-000.cells.json`
- `target/glorp-preview/strips/round-smooth-motion/frame-000.smooth-motion.json`
- `target/glorp-preview/strips/round-smooth-motion/frame-001..005.{txt,cells.json,smooth-motion.json}`

Bundle behavior verified by tests:

- `manifest.json` now records `kind: "smooth"` scenarios and a `kind: "smooth-motion"` strip
- parity scenario files expose `smooth_plan` and `smooth_parity`
- strip frame files expose `smooth_motion`
- artifact inventory includes `smooth-plan`, `smooth-parity`, and `smooth-motion`
- `review.md` links the smooth parity and motion sidecars

## Change Summary

- Added hidden `dev-preview --scenario smooth` CLI support and `PreviewSelection::Smooth`
- Added a new `src/dev_preview/smooth.rs` scenario builder that:
  - renders `round-smooth-classic-baseline` from `build_round_scene_draw_list(...)`
  - renders `round-smooth-classic-parity` from `build_round_smooth_scene_plan(...).flatten_classic_cells()`
  - emits deterministic `round-smooth-motion` strip frames with per-frame `.smooth-motion.json` sidecars
- Added smooth preview contracts in `src/dev_preview/contract.rs`:
  - `PreviewSmoothPlanArtifact`
  - `PreviewSmoothLayerArtifact`
  - `PreviewSmoothParityArtifact`
  - `PreviewSmoothMotionArtifact`
- Added manifest/export support for smooth sidecars and artifact inventory in:
  - `src/dev_preview/export.rs`
  - `src/dev_preview/scenarios.rs`
- Wired smooth bundles into both `Smooth` and `All` preview selection paths
- Extended `tests/dev_preview.rs` with smooth scenario, parity, privacy, motion, and `all`-bundle coverage

## Self-review

- Kept live TUI/watch behavior untouched; all work is scoped under `src/dev_preview` plus CLI routing for the hidden preview scenario
- Reused the existing round and smooth scene seams instead of duplicating render logic:
  - Classic baseline comes from the existing draw-list path
  - parity comes from the smooth plan's Classic flattening compatibility path
- Kept the smooth sidecars intentionally narrow:
  - roles
  - z order
  - local bounds
  - transforms
  - item counts
  - chrome reservations
  - checksums
  - abstract state buckets
  - privacy claims
- Verified the parity contract is exact for the fixed fixture by asserting matching Classic and smooth flatten checksums
- Verified motion metadata changes across at least five distinct fractional bob/anchor values without changing the underlying Classic cell source seam

## Concerns

- The motion strip currently proves fractional `PetBody` bob through metadata and repeated parity frames, but because Slice 1 intentionally preserves Classic flattened cells, the text/cells strip itself can look nearly static between adjacent frames. That is expected and is why the `.smooth-motion.json` sidecars are the primary review contract for this task.

## Fix Follow-up: review-blocked Task 4 hardening

### Scope

Reviewer-blocked fixes applied only in:

- `src/dev_preview/contract.rs`
- `tests/dev_preview.rs`
- `.superpowers/sdd/task-4-report.md`

No companion/AppKit code changed.

### RED Evidence

Added failing coverage first, then ran the required RED command:

- `cargo test --features dev-preview --test dev_preview dev_preview_smooth`
  - Failed as expected in `dev_preview_smooth_privacy_scan_covers_motion_sidecars`
  - Failure showed the privacy scan only covered:
    - `frames/round-smooth-classic-parity.smooth-plan.json`
    - `frames/round-smooth-classic-parity.smooth-parity.json`
  - And missed every motion sidecar:
    - `strips/round-smooth-motion/frame-000..005.smooth-motion.json`

Added a direct unit test in `src/dev_preview/contract.rs` for missing required-role detection:

- `smooth_parity_artifact_flags_missing_required_roles`
  - Removes `ambient` and `pet-body` from a generated plan
  - Asserts fixed Slice 1 `required_roles`
  - Asserts `missing_roles == ["ambient", "pet-body"]`
  - Asserts `exact_match == false`
  - Asserts `review_status == "missing-required-roles"`

This unit test would fail on the previous implementation because parity derived
`required_roles` from the surviving plan and never reported any missing roles.

### GREEN Evidence

Direct unit coverage after the contract fix:

- `cargo test --features dev-preview smooth_parity_artifact_flags_missing_required_roles`
  - Passed

Required focused integration test after the test + contract fixes:

- `cargo test --features dev-preview --test dev_preview dev_preview_smooth`
  - Passed: `4 passed; 0 failed`

Required bundle generation:

- `cargo run --features dev-preview -- dev-preview --scenario smooth --out target/glorp-preview`
  - Passed and rewrote the smooth preview bundle in `target/glorp-preview`

Required formatting check:

- `cargo fmt --check`
  - Failed once on rustfmt wrapping
  - Ran `cargo fmt`
  - Re-ran `cargo fmt --check`
  - Passed

### Fix Summary

- `PreviewSmoothParityArtifact` now compares the exported plan against a fixed
  Slice 1 required-role list from the spec instead of deriving the list from
  whatever roles happen to survive in the plan.
- `missing_roles` is now computed from absent plan roles.
- `exact_match` now fails closed when required roles are missing, even if the
  provided checksum matches the reduced plan.
- `review_status` now reports `missing-required-roles` before checksum status.
- Smooth privacy scanning in `tests/dev_preview.rs` now walks every smooth
  sidecar type:
  - `.smooth-plan.json`
  - `.smooth-parity.json`
  - `.smooth-motion.json`
- Smooth dev-preview assertions now verify the live parity artifact exports the
  full 18-role Slice 1 required-role set and an empty `missing_roles` list.
- `smooth_abstract_state()` now buckets:
  - species into `soft-body` / `spectral` / `synthetic`
  - stage into `early` / `grown` / `veteran`
  - mood into `settled` / `resting` / `needs-care`

### Generated Artifact Notes

- Verified the regenerated smooth bundle still includes:
  - `frames/round-smooth-classic-parity.smooth-plan.json`
  - `frames/round-smooth-classic-parity.smooth-parity.json`
  - `strips/round-smooth-motion/frame-000..005.smooth-motion.json`
- The privacy scan now explicitly covers all six motion sidecars instead of
  only the parity pair.

### Concerns

- None beyond the existing Slice 1 note above about fractional motion being
  primarily visible in `.smooth-motion.json` sidecars rather than large cell
  changes in every strip frame.
