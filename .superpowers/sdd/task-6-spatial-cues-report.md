# Task 6 Report: Retained Private Statistics Shadow and Pet Rim

## Status: complete

Task 6 adds renderer-private R8 coverage for the statistics HUD and pet body,
then uses those masks for the retained statistics rear projection and narrow
pet rim. The public scene snapshot, artifact, checksum, and debug contracts
remain unchanged.

## Final review corrections

- The independently constructed redacted capture retains its non-sensitive rim
  crossing but omits the value-shaped statistics rear projection.
- Pet composite selection is preflighted before allocation or submission and
  rejects absent, duplicate, and wrong-pipeline body/particle plans.
- Ready CPU and materialized GPU candidates rebase their private spatial frame
  from the current runtime snapshot.
- An active frame whose public scene revisions are unchanged now also enters
  the delta transaction with the fresh private spatial frame. This covers a
  reduce-motion-only update, uploads the private frame globals before the
  spatial-cue uniform is staged, and applies to both surface and lifetime
  offscreen rendering.

## Regression coverage

`active_reduce_motion_only_update_refreshes_private_gpu_frame_without_advancing_scene_version`
starts from a live activity rim (`activity_opacity = 0.73`), commits only
`reduce_motion = true`, and proves that:

- the public `SceneVersion` and serialized scene artifacts are unchanged;
- the active CPU mirror has the fresh private values; and
- the committed GPU frame mirror has the same fresh values after the render
  transaction.

The test was red before the active equal-version path was fixed: its private
transaction assertion failed because that path returned no dirty transaction.

## Verification

- RED: `cargo test --features retained-renderer --lib
  active_reduce_motion_only_update_refreshes_private_gpu_frame_without_advancing_scene_version`
  failed at the expected private-transaction assertion before the fix.
- GREEN: the same command passed after the fix.
- `cargo test --features retained-renderer --lib companion::retained::tests`
  — 42 passed.
- `cargo test --features retained-renderer --lib companion::retained::compiler::tests`
  — 35 passed.
- `cargo test --features retained-renderer --lib companion::paired_review::tests`
  — 22 passed.
- `cargo test --features retained-renderer --lib companion::retained::render::tests`
  — 97 passed; one existing unrelated failure is recorded below.

## Residuals

The report intentionally does not claim a new public artifact or paired-review
field. Private coverage, projection pixels, exact HUD data, and private frame
values remain outside those public contracts.

The retained renderer group has one unchanged, pre-rendering fixture failure:
`prop_cast_shadow_projects_down_right_scales_unions_and_suppresses_noise`
returns `SnapshotRejected(InconsistentIdentity)` while constructing its fixture.
This change does not modify that test or its fixture path.
