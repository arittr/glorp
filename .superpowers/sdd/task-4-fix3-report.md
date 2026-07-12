# Task 4 Fix 3 Report: Surface Rebind and Recovery Retry

Status: DONE

Baseline: `146307dd7e2361c58c44e3f3592a46c489fb2e0d`

## Outcome

This repair separates routine successful surface rebinding from fatal surface
recovery and makes rejected recovery candidates explicitly retryable.

### Operational surface rebind

- `acknowledge_operational_surface_rebound` is valid only while recovery is
  Operational. It acknowledges a host rebind that has already succeeded.
- The method advances only `SurfaceEpoch` with checked arithmetic. Scene key,
  layout generation, resource generation, semantic/frame revisions, request ID,
  worker state, and accepted scene state remain unchanged.
- The active version is relabeled to the successful new surface, so a capture
  lease reports the exact current binding.
- Pending desired surface identity is updated without discarding work. Ready
  remains Ready. Activating remains structurally tracked but its old-surface
  attempt becomes commit-ineligible; late clean present, surface-lost, and
  surface-validation outcomes cannot commit or trigger fallback and the
  candidate retries against the new surface.
- Fatal `SurfaceSuccessor` recovery remains a separate typed path that issues a
  resource generation request.

### Retryable recovery

- Candidate rejection during Recovering transitions to `AwaitingRetry`, which
  retains the recovery requirement and the already verified successor device
  and surface tuple.
- Rejection destroys the candidate but emits no automatic replacement work.
  Activation remains unavailable until the host explicitly calls
  `retry_recovery`.
- `retry_recovery` accepts only an exact current successor tuple in a running
  lifecycle. It advances resource generation and request ID once, emits one
  fresh one-shot generation request, and does not advance device or surface
  epochs again.
- Both surface and device recovery retries can clean-present to Operational.
  A stale tuple or shutdown lifecycle rejects before changing counters, pending
  identity, worker state, or recovery state.

## TDD evidence

Initial RED compilation proved the operational rebind method and AwaitingRetry
authority were absent. A later focused RED showed AwaitingRetry incorrectly
reported `NoReadyCandidate`; it now produces the typed SurfaceUnavailable
activation result until explicit retry.

New direct coverage:

- `operational_surface_rebind_relabels_only_surface_binding`
- `operational_surface_rebind_preserves_ready_candidate_for_new_surface`
- `operational_rebind_keeps_old_activation_tracked_but_commit_ineligible`
- `rejected_surface_recovery_retries_on_same_verified_successor`
- `rejected_device_recovery_retries_without_advancing_device_again`
- `stale_or_shutdown_recovery_retry_rejects_without_work`

## Verification

- Focused runtime: 40 passed.
- Default companion scene: 92 passed.
- Retained companion scene: 92 passed.
- Default and retained boundary suites: 10 passed each.
- Renderer boundary suites: 7 passed.
- Round/privacy regression suites: 23 passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt --check`: clean.
- `git diff --check`: clean.

## Files

- `src/presentation/companion_scene/runtime.rs`
- `.superpowers/sdd/task-4-fix2-report.md`
- `.superpowers/sdd/task-4-fix3-report.md`

## Independent review

The fresh read-only review verified operational surface rebind behavior,
Ready/Activating preservation, surface/device fatal scoping, exact AwaitingRetry
authority, transactional stale/shutdown rejection, and all previously closed
one-shot/publication/rebase/cancellation/hidden/shutdown contracts.

Final verdict: **PASS**, with no Critical or Important findings.
