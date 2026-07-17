# Final Review Validation Fix

## Scope

Closed the final-review finding that runtime snapshots and retained full/delta
frames accepted incomplete or forged Elevated prop cast-shadow lanes.

## Root cause and repair

- The validators checked ranges, profile eligibility, and only part of the
  vector/softness/strength presence relationship. A nonzero vector with zero
  softness and strength therefore passed, and complete finite placeholders did
  not have to match the authored projection.
- Added one shared canonicality predicate around `resolve_prop_shadow`. An
  absent cast must be entirely zero. A present cast is resolved with the fixed
  authored profile, current visibility/opacity/footprint/cell extent/contact
  strength, `grounded = true`, and an arbitrary finite origin; vector,
  softness, strength, and resolved contact strength must match exactly.
- Cast absence remains valid because grounding is intentionally not serialized.
- Applied the same predicate in `validate_snapshot` and
  `validate_prop_frame_slot`, which covers retained full frames and deltas.

## Focused behavior coverage

- Runtime snapshot: rejects a partial tail plus forged direction, softness, and
  strength; keeps a canonical Elevated cast valid.
- Retained validation: rejects the same four mutations through both full and
  delta paths and proves failed deltas are atomic.
- Existing authored-profile acceptance fixtures now use canonical resolver
  output instead of placeholder cast values.

## Verification

- RED: both new tests initially failed because the partial tail returned `Ok`.
- `cargo test --lib presentation::companion_scene::runtime::tests -- --nocapture`
  — 79 passed.
- `cargo test --lib presentation::companion_scene::validate::tests -- --nocapture`
  — 42 passed.
- `cargo test --lib cast_lanes -- --nocapture` — 3 passed.
- `cargo fmt --check` — passed.
- Focused `git diff --check` — passed.

Existing default-feature warnings were unchanged and outside this fix.
