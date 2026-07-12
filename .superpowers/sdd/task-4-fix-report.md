# Task 4 Fix Report: Runtime Transition Contracts

Status: DONE

Baseline: `2bc49edc8ed3aaa1a70906de5b69a7fada41f52f`

## Outcome

The Task 4 runtime now exposes one explicit transition/effect contract and one
transactional snapshot publication boundary.

- Non-cloneable `RuntimeEffects` owns the exact worker start request and identifies worker
  cancellation and candidate destruction. `WorkerState::Running` is assigned in
  one helper only, at the same transition that returns a one-shot
  `take_start_worker` action. `GenerationRequest` is also non-cloneable.
- `PreparedSnapshotUpdate` is non-cloneable and binds the proposed snapshot,
  complete additive masks, checked next revisions/generations, the exact base
  `Arc`, all owned counters, epochs, visibility, worker/pending identity, and
  recovery state. Dropping a prepared token publishes nothing. A stale token
  cannot commit.
- Generation candidates can only be created from the immutable
  `GenerationRequest::accept` authority or the runtime-owned exact rebase path.
- Resource invalidation is a closed Glorp-specific enum covering backing-scale
  atlas, surface recovery, and material contract changes. Pure layout changes do
  not advance resource generation.
- Capture is gated by typed operational/recovery/shutdown state. Surface or
  device epoch movement does not make a frame capturable; only exact clean
  presentation of the recovery request returns the runtime to operational.
- Hidden reveal is a two-phase `prepare_reveal`/`commit_reveal` transaction.
  Task 5 projects through the prepared update before consuming commit. A
  projection, validation, or counter failure keeps the snapshot and leaves the
  runtime hidden; successful commit emits at most one start.
- Shutdown is irreversible and all late worker, candidate, activation, device,
  surface, resource, reveal, and capture paths fail closed.

The obsolete mutation-first reconciler API, caller-authored candidate
constructor, boolean fallback flag, generic submission/worker transition enums,
and diagnostic counters were removed.

## Review-finding closure

1. Worker launch ownership: fixed `RuntimeEffects` plus fake dispatcher tests
   prove every Running transition has one owned start and at most one live
   worker across both cancellation completion orders and activation
   supersession.
2. Projection-before-publication: prepare validates, classifies, and preflights
   without mutation; commit checks the exact base then publishes atomically.
3. Candidate authority: request-bound acceptance and a transactional projection
   closure over the runtime-owned exact desired snapshot are the only paths.
   Unchanged stale frame proof cannot acquire newer revision metadata.
4. Production resource invalidation: fixed variants flow through the normal
   prepare/commit/queue path.
5. Recovery state: exact device/surface/request authority follows a sufficient
   superseding request. Capture remains unavailable until that exact successor
   presents cleanly.
6. Hidden/shutdown lifecycle: hidden work queues without starting, reveal
   coalesces once, failures retain latest, and shutdown cannot restart.
7. Diagnostics: mutable reconciliation/worker counters were removed; the test
   dispatcher derives its assertions from effects.

## Coverage matrix

| Contract | Direct focused coverage |
| --- | --- |
| Complete additive Task 5 mask classification | `every_snapshot_leaf_is_classified_by_render_lifetime`; `fixed_named_masks_are_task_five_extensible` |
| Prepare/drop/commit and exact stale token | `prepared_updates_publish_only_after_exact_commit`; `prepared_token_is_invalidated_by_runtime_boundary_changes` |
| Invalid/capacity/non-finite rejection and counter overflow | `rejected_and_overflowed_preparation_leave_published_state_untouched`; `every_invalid_snapshot_family_is_transactionally_rejected`; `every_owned_counter_overflow_is_typed_and_non_mutating` |
| Layout/resource/surface generation independence | `layout_and_resource_generations_advance_only_for_their_own_lifetimes`; `production_resource_invalidation_preserves_layout_and_emits_work`; `surface_recovery_advances_surface_and_resources_without_layout_relabel` |
| Shared snapshots and fixed masks | `prepared_snapshot_shares_arc_and_change_masks_remain_fixed_size` |
| Exact candidate authority and compatible rebase | `accepted_candidate_metadata_is_bound_by_runtime_authority`; `compatible_updates_require_runtime_owned_exact_rebase`; `frame_rebase_cannot_relabel_unchanged_accepted_proof` |
| Worker ownership, topology storm, both cancellation orders | `running_worker_transitions_emit_exactly_one_owned_start_action` |
| SupersedingActivation late present/reject/fatal | `superseding_activation_drops_old_candidate_and_starts_exact_new_request`; `superseding_activation_late_rejection_and_current_fatal_remain_actionable` |
| Old-surface/device fatal scope | `retired_surface_and_device_failures_cannot_poison_recovery` |
| Exact activation/deferral/rejection/fatal guards | `activation_guards_retain_active_until_exact_clean_present`; `every_epoch_failure_enters_typed_fallback` |
| Typed recovery and capture lease | `fallback_and_recovery_gate_capture_until_exact_present`; `recovery_authority_follows_newest_request_in_both_cancellation_orders`; `capture_lease_binds_exact_active_and_defers_during_activation` |
| Hidden resource/device queues and reveal storm | `device_recreation_and_resource_work_stay_queued_while_hidden`; `hidden_snapshot_storm_commits_only_latest_and_emits_one_reveal_start`; `hiding_ready_or_in_flight_activation_never_commits_while_hidden` |
| Delayed error scope/no resurrection | `delayed_errors_are_device_scoped_and_cannot_resurrect_state` |
| Rebase base follows accepted projection | `repeated_rebase_diffs_from_last_accepted_projection` |
| Reveal projection failure/absence binding and shutdown terminality | `dropped_reveal_projection_retains_hidden_snapshot_and_publishes_nothing`; `reveal_token_binds_absence_of_hidden_snapshot`; `hidden_reveal_and_shutdown_are_fail_closed` |

## TDD evidence

Initial RED was a focused contract test compile failure for the missing
`RuntimeEffects`, prepared-token API, resource invalidation variants, typed
recovery state, and terminal lifecycle APIs. A later RED pass reproduced missing
candidate-drop, stale recovery-authority, hidden-absence, and repeated-rebase
failures. The repaired focused suite is now 31 passing tests. New tests were added before or alongside each repaired
transition and exercised failures before the final green gate.

## Verification

- `cargo test --lib presentation::companion_scene::runtime`: 31 passed.
- `cargo test --lib presentation::companion_scene`: 83 passed.
- `cargo test --lib --features retained-renderer presentation::companion_scene`: 83 passed.
- `cargo test --test companion_scene_boundary`: 10 passed.
- `cargo test --features retained-renderer --test companion_scene_boundary`: 10 passed.
- renderer boundary tests: 7 passed.
- round/privacy regression tests: 23 passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: clean.
- `cargo fmt --check`: clean at the last formatting gate.
- `git diff --check`: clean at the last diff gate.

## Files

- `src/presentation/companion_scene/runtime.rs`
- `.superpowers/sdd/task-4-fix-report.md`

## Independent review

Three fresh read-only review passes were run against the evolving repair. The
first two found seven Important transition-contract issues in total; each was
reproduced and fixed. The final frozen-tree release review verified all seven
closures and found no remaining Critical or Important issue.

Final verdict: **PASS**.
