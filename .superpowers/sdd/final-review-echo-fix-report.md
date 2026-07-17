# Final Review Echo Fix Report

## Scope

- Corrected the AppKit HUD echo scheduler to treat `[+0.60, -0.60]` as one
  two-dimensional Y-up offset.
- Each prepared statistics line now produces exactly one rear echo draw at
  `x + 0.60, y - 0.60`; primary line drawing is unchanged.
- Added one behavior-level regression test over the production draw scheduler.

## TDD and Verification

- RED: the focused regression failed because the single-call draw scheduler did
  not yet exist; the old renderer directly emitted two vertical draws.
- GREEN: `cargo test --lib companion::app::tests::appkit_hud_echo_draws_once_per_line_at_the_rear_offset -- --exact`
  passed (1 test).
- Focused AppKit suite: `cargo test --lib companion::app::tests` passed (39 tests).
- Formatting: `cargo fmt --check` passed.

## Scope Check

- No retained-renderer, prop-shadow validation, preview, capture, or acceptance
  paths changed.
