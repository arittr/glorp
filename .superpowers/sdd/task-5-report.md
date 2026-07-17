# Task 5 Report: AppKit Statistics Projection and Pet-Local Rim

Status: DONE

## Outcome

Implemented the private, receiving-surface-only effects requested for Smooth
AppKit and Pixel without changing public scene, review, or capture contracts.

- Smooth AppKit now prepares a redacted rear-wall projection from the existing
  HUD primary-coverage alpha, with a normalized separable blur, authored
  key-light offset, circular-aperture clip, biome bed-shadow tint, and shared
  opacity cap. Capture/redacted preparation retains HUD interaction coverage
  but omits this projection.
- The projection is the first Smooth schedule step, after the tank background
  (which is painted outside the schedule) and before world content. The
  boundary test asserts this ordering for behind, interacting, and in-front
  pet depths.
- Smooth AppKit isolates exactly the `PetBody` layer into private coverage,
  derives an exterior-only rim with the shared style, and paints it immediately
  before the pet group both normally and while crossing statistics. The rim is
  not part of the HUD mask. Classic and retained frames do not allocate either
  AppKit resource.
- Activity uses the existing two-second pulse envelope. Reduce Motion removes
  only the activity alpha bonus while preserving rim extent.
- Pixel derives a body/reference occupancy mask, composites the same
  exterior-only narrow rim, then paints body, face, and accents over it. It
  does not restore the retired broad aura.

## TDD evidence

Initial RED command:

```bash
cargo test --lib companion::app::tests::appkit_statistics_shadow &&
cargo test --lib companion::app::tests::appkit_pet_rim &&
cargo test --lib presentation::pixel::animator::tests
```

It exited 101 with the expected missing Task 5 types, fields, helpers, and
schedule signatures: no `StatisticsRearShadow`, private HUD projection,
private pet rim, isolated `pet_body` coverage, or Pixel rim helpers existed.

## Verification

Fresh focused checks after the final boundary assertion:

- `cargo fmt --check`: passed.
- `frame_preparation_keeps_appkit_hud_smooth_only_and_carries_effective_reveal`:
  passed; it now verifies the rim is present only for Smooth AppKit and absent
  for retained frames.
- `appkit_statistics_shadow`: 2 passed.
- `appkit_pet_rim`: 1 passed.
- `presentation::pixel::animator::tests`: 3 passed.
- `smooth_appkit_prepared_schedule_crosses_statistics_at_effective_pet_depth`:
  passed, including the new first-step `StatisticsRearShadow` assertion.
- `git diff --check`: passed.

The focused test invocations report existing repository warnings in unrelated
modules; this change introduces no new warning failure.

## Known unrelated baseline failures

- `cargo test --test smooth_companion` retains four existing motion/depth
  fixture failures: zero idle-bob expectations, a classic placement mismatch,
  and an `InvalidDepthProjection` fixture rejection. They occur before or
  outside the Task 5 effect paths.
- The full `companion_draw_boundary` binary retains one pre-existing stale
  source-shape assertion expecting
  `try_build_round_smooth_scene_plan_with_options(`. The baseline code already
  uses `try_build_round_smooth_scene_plan_with_grid_points`; the Task 5
  schedule assertion itself passes. No unrelated boundary cleanup was made.

## Scope and self-review

- Kept all live text, source masks, and private generated images behind
  redacted debug types.
- Confirmed the body rim comes only from `SmoothLayerRole::PetBody`, excluding
  performance cues, particles, props, shadows, and HUD reservations.
- Corrected the existing Smooth pass-plan fixture accounting to reserve its
  three screen layers, not four; it is a test-fixture calculation repair, not
  a rendering behavior change.
- Limited the authorized boundary update to the twelve-step schedule and its
  explicit first-step assertion.

## Files

- `src/companion/app.rs`
- `src/presentation/pixel/animator.rs`
- `tests/companion_draw_boundary.rs`
- `.superpowers/sdd/task-5-report.md`
