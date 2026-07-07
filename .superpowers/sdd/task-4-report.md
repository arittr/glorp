# Task 4 Report: Provider Snapshot-First Polling

status: DONE_WITH_CONCERNS

## Files Changed

- `src/usage/provider.rs`
- `src/usage/ccusage.rs`
- `src/usage/agentsview.rs`
- `tests/usage_provider.rs`
- `tests/runtime_integration.rs`
- `tests/fixtures/helpers/ccusage-drop-day.mjs`
- `tests/fixtures/helpers/ccusage-extra-day.mjs`
- `tests/fixtures/helpers/ccusage-malformed-row.mjs`
- `tests/fixtures/helpers/ccusage-model-remap.mjs`
- `tests/fixtures/helpers/agentsview-drop-day.mjs`

`tests/runtime_integration.rs` was the only adjacent file touched, to add the new `UsageProvider::refresh_snapshots_only` method to a test-only trait implementer.

## Red Test Summary

Ran the four required red commands after adding fixtures/tests:

- `cargo test --test usage_provider provider_writes_snapshot_before_emitting_feed_deltas`
- `cargo test --test usage_provider unexpected_extra_provider_day_does_not_write_snapshot_or_feed`
- `cargo test --test usage_provider disappeared_requested_provider_day_writes_current_zero_without_negative_food`
- `cargo test --test usage_provider malformed_requested_row_blocks_snapshot_and_does_not_feed_valid_looking_rows`

All four failed before implementation with the same compile gate:

- `E0599`: `CcusageCommandProvider::new_with_now_for_test` did not exist.
- `E0599`: `refresh_snapshots_only` did not exist on `CcusageCommandProvider`.

## Green Verification Summary

- `cargo test --test usage_provider`
  - Passed: 34 tests, 0 failed.
- `cargo fmt --check`
  - Passed after running `cargo fmt`.
- Extra adjacent verification: `cargo test --test runtime_integration tokenmaxxing_cutover_seeds_agentsview_cursors_without_feeding_existing_pet`
  - Passed: 1 test, 0 failed.
- Commit pre-commit hook:
  - `fmt` passed.
  - `clippy` passed.

## Commit

- `c3397cb feat: write provider snapshots before feeding`

## Concerns

- `UsageStore::write_provider_snapshot_batch` currently upserts provider cursors for snapshot rows. If providers call it before feed planning, known-source deltas are suppressed because the planner sees the just-written cursor as the baseline. The provider implementation therefore computes the feed plan from parsed requested-day rows before writing the snapshot batch, then writes the snapshot before returning the `UsagePollResult`. This preserves the externally visible behavior that a poll result is not emitted until the requested-day snapshot is durable, but it is not the literal storage-call order sketched in the task brief.

---

# Task 4 Sequencing Fix Report

status: DONE

## Files Changed

- `src/storage/usage_store.rs`
- `src/usage/ccusage.rs`
- `src/usage/agentsview.rs`
- `tests/usage_provider.rs`
- `tests/usage_snapshots.rs`

## Red Test Summary

- `cargo test --test usage_snapshots snapshot_batch_does_not_advance_row_provider_cursor`
  - Failed before the fix as expected: snapshot write advanced the row cursor, with `left: Some("raw:531")` and `right: None`.
- `cargo test --test usage_provider snapshot_only_refresh_does_not_seed_cursor_before_feed_poll`
  - Failed before the fix as expected: snapshot-only refresh seeded the cursor, so the follow-up known-source poll emitted `0` tokens and failed `result.total_tokens > 0.0`.

## Green Verification Summary

- `cargo test --test usage_snapshots snapshot_batch_does_not_advance_row_provider_cursor`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider snapshot_only_refresh_does_not_seed_cursor_before_feed_poll`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider`
  - Passed: 35 passed, 0 failed.
- `cargo test --test usage_snapshots`
  - Passed: 9 passed, 0 failed.
- `cargo fmt --check`
  - Passed after running `cargo fmt`.
- Commit pre-commit hook:
  - `fmt` passed.
  - `clippy` passed.

## Commit

- `f4fa884bb2d9e085d4a5948e29a98d41216c6fed fix: keep snapshot writes from advancing feed cursors`

## Concerns

- None. The previous sequencing concern is resolved: provider snapshot writes no longer advance data cursors, `ccusage` and `agentsview` now write requested-day snapshots before feed planning, snapshot-only refresh stays snapshot-only, helper-version metadata cursor writes are unchanged, and legacy cursor migration still happens before feed planning.

---

# Task 4 Review Fix Report

status: DONE

## Files Changed

- `src/usage/ccusage.rs`
- `src/usage/agentsview.rs`
- `tests/usage_provider.rs`
- `tests/fixtures/helpers/ccusage-mixed-malformed-row.mjs`
- `tests/fixtures/helpers/ccusage-unified-aggregate-requested.mjs`

## Red Test Summary

- `cargo test --test usage_provider unusable_unified_rows_fall_back_without_writing_zero_snapshot -- --exact`
  - Failed before the fix as expected: unified aggregate/unidentified rows were treated as a completed zero-row snapshot and legacy fallback did not run, failing `result.total_tokens > 0.0`.
- `cargo test --test usage_provider snapshot_only_refresh_does_not_migrate_legacy_cursor_before_feed_poll -- --exact`
  - Failed before the fix as expected: snapshot-only refresh migrated the requested-day legacy cursor, with the new cursor value present instead of `None`.
- `cargo test --test usage_provider mixed_malformed_and_valid_requested_rows_block_without_feeding -- --exact`
  - Failed before the fix as expected: a valid-looking requested row fed `100.0` tokens despite a malformed required row in the same requested response.

## Green Verification Summary

- `cargo test --test usage_provider unusable_unified_rows_fall_back_without_writing_zero_snapshot -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider snapshot_only_refresh_does_not_migrate_legacy_cursor_before_feed_poll -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider mixed_malformed_and_valid_requested_rows_block_without_feeding -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider`
  - Passed: 38 passed, 0 failed.
- `cargo fmt --check`
  - Passed.
- Commit pre-commit hook:
  - `fmt` passed.
  - `clippy` passed.

## Commit

- `2ae209717ef46334e4a4279da1a23f29d7f551bb fix: block unusable provider snapshots`

## Concerns

- None.

# Task 4 Third Review Fix Report

status: DONE

## Files Changed

- `src/usage/ccusage.rs`
- `src/usage/agentsview.rs`
- `tests/usage_provider.rs`
- `tests/fixtures/helpers/ccusage-malformed-period-sensitive.mjs`

## Red Test Summary

- `cargo test --test usage_provider ccusage_scoped_fallback_writes_one_complete_snapshot_for_sibling_sources -- --exact`
  - Failed before the fix as expected: visible snapshot sources were `{"codex"}`, proving the later scoped ccusage batch erased `claude-code`.
- `cargo test --test usage_provider agentsview_poll_writes_one_complete_snapshot_for_sibling_sources -- --exact`
  - Failed before the fix as expected: visible snapshot sources were `{"codex"}`, proving the later agentsview batch erased `claude`.
- `cargo test --test usage_provider malformed_ccusage_period_diagnostic_omits_raw_period_and_model -- --exact`
  - Failed before the fix as expected: persisted diagnostics contained `/Users/drew/private` from the malformed helper period field.

## Green Verification Summary

- `cargo test --test usage_provider ccusage_scoped_fallback_writes_one_complete_snapshot_for_sibling_sources -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider agentsview_poll_writes_one_complete_snapshot_for_sibling_sources -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider malformed_ccusage_period_diagnostic_omits_raw_period_and_model -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider legacy_cursor_with_parser_version_migrates_without_double_feeding -- --exact`
  - Passed: 1 passed, 0 failed after preserving per-helper migration command/version in combined ccusage batches.
- `cargo test --test usage_provider snapshot_only_refresh_does_not_migrate_legacy_cursor_before_feed_poll -- --exact`
  - Passed: 1 passed, 0 failed after preserving per-helper migration command/version in combined ccusage batches.
- `cargo test --test usage_provider`
  - Passed: 46 passed, 0 failed.
- `cargo fmt --check`
  - Passed.

## Commit

- Commit hash: final hash recorded in handoff after commit creation.
- Commit message: `fix: preserve scoped provider snapshot siblings`

## Concerns

- None.

# Task 4 Second Review Fix Report

status: DONE

## Files Changed

- `src/usage/ccusage.rs`
- `src/usage/agentsview.rs`
- `tests/usage_provider.rs`
- `tests/fixtures/helpers/ccusage-malformed-period-only.mjs`
- `tests/fixtures/helpers/ccusage-malformed-period-with-valid-sibling.mjs`
- `tests/fixtures/helpers/ccusage-unified-malformed-required-requested.mjs`
- `tests/fixtures/helpers/agentsview-malformed-period-with-valid-sibling.mjs`

## Red Test Summary

- `cargo test --test usage_provider malformed -- --nocapture`
  - Failed before the fix as expected:
    - `malformed_ccusage_period_blocks_zero_snapshot`: snapshot was `Current`, expected `Blocked`.
    - `malformed_ccusage_period_blocks_valid_sibling_rows`: valid sibling fed `100.0` tokens, expected `0.0`.
    - `malformed_agentsview_period_blocks_valid_sibling_rows`: valid siblings fed `200.0` tokens, expected `0.0`.
    - `malformed_unified_ccusage_blocks_scoped_fallback`: scoped fallback fed `84500.0` tokens, expected `0.0`.
- `cargo test --test usage_provider ccusage_poll_does_not_migrate_legacy_cursor_before_durable_snapshot_write -- --exact --nocapture`
  - Failed before the fix as expected: a forced snapshot write failure still left the migrated data cursor present, with `Some("{\"uncached_input\":1000,\"output\":1500,\"cache_creation\":300,\"cache_read\":50000,\"reasoning_output\":0}")` instead of `None`.

## Green Verification Summary

- `cargo test --test usage_provider malformed -- --nocapture`
  - Passed: 7 passed, 0 failed.
- `cargo test --test usage_provider ccusage_poll_does_not_migrate_legacy_cursor_before_durable_snapshot_write -- --exact --nocapture`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider`
  - Passed: 43 passed, 0 failed.
- `cargo fmt --check`
  - Passed after running `cargo fmt`.
- Commit pre-commit hook:
  - `fmt` passed.
  - `clippy` passed.

## Commit

- `b0367fc6b10af2a1cc166544e6bbba66e389a9f0 fix: block malformed provider identity rows`

## Concerns

- None.

# Task 4 Fourth Review Fix Report

status: DONE

## Files Changed

- `src/storage/usage_store.rs`
- `src/usage/agentsview.rs`
- `src/usage/ccusage.rs`
- `src/usage/identity.rs`
- `src/usage/snapshot.rs`
- `tests/usage_provider.rs`
- `tests/usage_snapshots.rs`
- `tests/fixtures/helpers/ccusage-malformed-raw-agent.mjs`

## Red Test Summary

- `cargo test --test usage_provider malformed_ccusage_raw_agent_diagnostics_do_not_persist_raw_source_content -- --exact`
  - Failed before the fix as expected: persisted diagnostics still contained `project-secret` from the raw helper `agent` value.
- `cargo test --test usage_provider claude_only_scoped_refresh_preserves_uncovered_codex_snapshot_truth -- --exact`
  - Failed before the fix as expected: codex was absent after a claude-only scoped refresh, with only `claude-code` remaining in source totals.

## Green Verification Summary

- `cargo test --test usage_provider malformed_ccusage_raw_agent_diagnostics_do_not_persist_raw_source_content -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider claude_only_scoped_refresh_preserves_uncovered_codex_snapshot_truth -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider`
  - Passed: 48 passed, 0 failed.
- `cargo test --test usage_snapshots`
  - Passed: 9 passed, 0 failed.
- `cargo fmt --check`
  - Passed.

## Commit

- Commit hash: recorded in final handoff after commit creation.
- Commit message: `fix: sanitize provider diagnostics and snapshot coverage`

## Concerns

- None.

---

# Task 4 Unsupported Token Shape Fix Report

status: DONE

## Files Changed

- `src/usage/ccusage.rs`
- `tests/usage_provider.rs`
- `tests/fixtures/helpers/ccusage-unsupported-token-shape-with-valid-sibling.mjs`
- `tests/fixtures/ccusage-unified-multi.json`
- `tests/fixtures/ccusage-unified-multi-next.json`

## Red Test Summary

- `cargo test --test usage_provider unsupported_token_shape_requested_row_blocks_valid_sibling_rows -- --exact`
  - Failed before the fix as expected: the valid sibling row fed `100.0` tokens, failing `left: 100.0` vs `right: 0.0`.

## Green Verification Summary

- `cargo test --test usage_provider unsupported_token_shape_requested_row_blocks_valid_sibling_rows -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider`
  - Passed: 49 passed, 0 failed.
- `cargo fmt --check`
  - Passed.

## Commit

- Commit hash: recorded in final handoff after commit creation.
- Commit message: `fix: block unsupported ccusage token shape`

## Concerns

- None.

---

# Task 4 Provider-Day Scope Fix Report

status: DONE

## Files Changed

- `src/usage/agentsview.rs`
- `src/usage/ccusage.rs`
- `tests/usage_provider.rs`
- `tests/fixtures/helpers/agentsview-unrequested-malformed-row.mjs`
- `tests/fixtures/helpers/ccusage-unified-aggregate-unrequested.mjs`
- `tests/fixtures/helpers/ccusage-unrequested-malformed-row.mjs`
- `tests/fixtures/helpers/ccusage-unrequested-unsupported-shape-with-valid-requested.mjs`

## Red Test Summary

- `cargo test --test usage_provider agentsview_scoped_refresh_preserves_uncovered_snapshot_truth -- --exact`
  - Failed before the fix as expected: the seeded `gemini` snapshot disappeared after AgentsView wrote a claude/codex combined snapshot.
- `cargo test --test usage_provider unrequested_malformed_ccusage_row_writes_requested_zero_snapshot -- --exact`
  - Failed before the fix as expected: no `unexpected_provider_day` diagnostic was emitted because the malformed unrequested row blocked before provider-day filtering.
- `cargo test --test usage_provider unrequested_unsupported_ccusage_row_does_not_block_requested_valid_row -- --exact`
  - Failed before the fix as expected: requested-day valid tokens were blocked by an unsupported-shape row from an unrequested day.
- `cargo test --test usage_provider unrequested_unidentified_unified_row_does_not_force_scoped_fallback -- --exact`
  - Failed before the fix as expected: an unrequested aggregate unified row forced scoped fallback and fed `100.0` tokens.
- `cargo test --test usage_provider unrequested_malformed_agentsview_row_writes_requested_zero_snapshot -- --exact`
  - Failed before the fix as expected: no `unexpected_provider_day` diagnostic was emitted because the malformed unrequested row blocked before provider-day filtering.

## Green Verification Summary

- `cargo test --test usage_provider agentsview_scoped_refresh_preserves_uncovered_snapshot_truth -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider unrequested_malformed_ccusage_row_writes_requested_zero_snapshot -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider unrequested_unsupported_ccusage_row_does_not_block_requested_valid_row -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider unrequested_unidentified_unified_row_does_not_force_scoped_fallback -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider unrequested_malformed_agentsview_row_writes_requested_zero_snapshot -- --exact`
  - Passed: 1 passed, 0 failed.
- `cargo test --test usage_provider`
  - Passed: 54 passed, 0 failed.
- `cargo test --test usage_snapshots`
  - Passed: 9 passed, 0 failed.
- `cargo fmt --check`
  - Passed.

## Commit

- Commit hash: recorded in final handoff after commit creation.
- Commit message: `fix: scope provider snapshots to requested days`

## Concerns

- None.
