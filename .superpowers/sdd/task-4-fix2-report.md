# Task 4 Fix 2 Report: One-Shot Recovery Authority

Status: DONE

Baseline: `9ecb4b03884fe332b36150c177303046b8835fc2`

## Outcome

The final Task 4 repair closes three authority and recovery contracts.

### One-shot external authority

- `GenerationRequest::accept` consumes the request, so one emitted worker
  request can create at most one candidate.
- A pending generation stores the authoritative request only while Queued.
  Entering Running moves that sole request into the one-shot Start effect; the
  runtime retains only `RequestIdentity` and desired-state data.
- Runtime callers can inspect pending `RequestIdentity` metadata but cannot
  borrow or duplicate a `GenerationRequest` from runtime state.
- Start, cancel, and candidate-drop actions are non-Clone/non-Copy values.
  `RuntimeEffects::take_*` consumes each action slot exactly once.
- Runtime-owned compatible rebase remains a separate transactional authority
  over the exact accepted-to-desired snapshot transition.

### Typed recovery requirements

- `RecoveryRequirement::SurfaceSuccessor` records the failed device and
  surface; `DeviceSuccessor` records the failed device.
- Surface recovery is no longer a public `ResourceInvalidation` variant.
  Only `acknowledge_surface_rebound` can issue it, and only for the exact current
  surface-successor requirement.
- Only `acknowledge_device_recreated` can issue device recovery, and its device
  epoch is checked to be strictly newer than the failed epoch.
- A wrong acknowledgement returns `RecoveryActionRejected` before changing any
  epoch, generation, request ID, active state, pending state, or effect.
- Recovering authority stays bound to the exact requirement, request, device,
  and surface. A sufficient exact clean presentation consumes it back to
  Operational; superseding requests carry the same requirement forward.

### Fatal scoping

- Surface-lost and surface-validation failures are actionable only when both
  the attempt device and surface equal the runtime's current epochs.
- An old-device attempt reporting either surface failure at the current surface
  is stale. Its superseded candidate is destroyed exactly once, the new worker
  start remains one-shot, and the current device recovery is unchanged.

## TDD evidence

The first RED compile showed missing one-shot cancel/drop extraction,
`RequestIdentity`, `RecoveryRequirement`, typed rejection, and exact pending
metadata APIs. Focused behavior RED then covered wrong recovery acknowledgement
and old-device/same-surface failures. The resulting focused suite is 34 passing
tests.

New direct coverage:

- `emitted_worker_and_cleanup_actions_are_one_shot`
- `recovery_requirement_allows_only_its_exact_acknowledgement`
- `surface_fatal_from_old_device_is_stale_even_at_current_surface`

Existing recovery-supersession coverage continues to exercise both worker
cancellation completion orders.

## Verification

- `cargo test --lib presentation::companion_scene::runtime`: 34 passed.
- `cargo test --lib presentation::companion_scene`: 86 passed.
- `cargo test --lib --features retained-renderer presentation::companion_scene`:
  86 passed.
- Default and retained companion-scene boundary suites: 10 passed each.
- Renderer boundary suites: 7 passed.
- Round/privacy regression suites: 23 passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt --check`: clean.
- `git diff --check`: clean.

## Files

- `src/presentation/companion_scene/runtime.rs`
- `.superpowers/sdd/task-4-fix2-report.md`

## Independent review

The fresh read-only review verified the one-shot authority model, typed recovery
requirements, stale surface-fatal cross-product, publication/rebase seams,
cancellation ordering, hidden/shutdown behavior, counters, and cleanup.

Final verdict: **PASS**, with no Critical or Important findings.
