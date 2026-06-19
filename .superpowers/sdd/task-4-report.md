# Task 4 Report: Provider Contract Cutover And Cursor Seeding

## Implementation Summary

- Added persistent `token_contract_state` storage with `UsageStore::is_token_contract_active` and `UsageStore::mark_token_contract_active`.
- Added `glorp::usage::cutover::ensure_tokenmaxxing_contract_active`, which:
  - returns without polling when `TOKENMAXXING_TOTAL_V1` is already active;
  - snapshots canonical `agentsview` usage before normal polling;
  - refreshes calibration/rhythm from snapshot daily totals;
  - advances the exact snapshot cursors;
  - marks the Tokenmaxxing contract active only when the snapshot has no provider diagnostics.
- Wired `init`, `status`, and `watch` to use `AgentsviewCommandProvider::from_environment()`.
- Updated status/init fixtures to pin `GLORP_AGENTSVIEW_BIN` or explicitly hide it from `PATH`.

## TDD Evidence

RED:

```bash
cargo test --test runtime_integration tokenmaxxing_cutover_seeds_agentsview_cursors_without_feeding_existing_pet
```

Result: failed as expected with `cannot find cutover in usage` and missing `UsageStore::is_token_contract_active`.

Additional RED for missing helper activation:

```bash
cargo test --test runtime_integration tokenmaxxing_cutover_missing_agentsview_does_not_activate_contract
```

Result: failed as expected because the initial helper marked the contract active despite provider diagnostics.

GREEN:

```bash
cargo test --test runtime_integration tokenmaxxing_cutover
```

Result: 2 passed, 0 failed.

## Verification

Fresh final verification:

```bash
cargo test --test runtime_integration tokenmaxxing_cutover_seeds_agentsview_cursors_without_feeding_existing_pet
```

Result: 1 passed, 0 failed.

```bash
cargo test --test runtime_integration tokenmaxxing_cutover_missing_agentsview_does_not_activate_contract
```

Result: 1 passed, 0 failed.

```bash
cargo test --test doctor_status status_surfaces_first_contact_without_claiming_blocked
```

Result: 1 passed, 0 failed.

```bash
cargo test --test cli_smoke init_
```

Result: 5 passed, 0 failed.

```bash
cargo test --lib commands::watch::tests
```

Result: 18 passed, 0 failed.

```bash
cargo test --test doctor_status
```

Result: 13 passed, 0 failed.

```bash
cargo fmt --check
```

Result: passed.

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Result: passed.

## Changed Files

- `src/usage/cutover.rs`
- `src/usage/mod.rs`
- `src/storage/usage_store.rs`
- `src/commands/init.rs`
- `src/commands/status.rs`
- `src/commands/watch.rs`
- `tests/runtime_integration.rs`
- `tests/doctor_status.rs`
- `tests/cli_smoke.rs`
- `.superpowers/sdd/task-4-report.md`

## Remaining Risks

- There is no direct existing integration test for `watch::poll_usage_and_apply` with a real `GLORP_AGENTSVIEW_BIN`; the focused watch module tests passed, and the shared cutover/status paths cover the provider behavior.
- Task 5 runtime math remains intentionally out of scope. Existing feed/application logic still consumes the `effective_tokens` fields supplied by the provider, which now mirror Tokenmaxxing totals for agentsview deltas.
