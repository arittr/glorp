# Task 1 Report: Add Token Contract Primitives And Storage Columns

## What I implemented

- Added `glorp::usage::token_contract` with `TOKENMAXXING_TOTAL_V1 = "tokenmaxxing_total_v1"` and `WEIGHTED_EFFECTIVE_V1 = "weighted_effective_v1"`.
- Added `RawTokenTotals::total_tokens()` as uncached input + output + cache creation + cache read, excluding reasoning output.
- Added `total_tokens` and `token_contract` to `UsagePollResult`, `UsageDelta`, and `NormalizedUsageEvent`.
- Kept current ccusage/provider behavior legacy-compatible by setting provider `total_tokens` equal to `effective_tokens` and `token_contract` to `weighted_effective_v1`.
- Added `total_tokens` and `token_contract` storage columns, migration/backfill, INSERT/SELECT row mappings, and canonical Tokenmaxxing total query methods.
- Added the brief-specified math and storage contract tests.
- Updated existing provider-result test helpers that directly construct `UsagePollResult`/`UsageDelta` so the expanded structs compile.

## What I tested and exact test results

- `cargo test --test game_rules tokenmaxxing_total_counts_cache_reads_fully_and_excludes_reasoning`
  - Result: `1 passed; 0 failed; 26 filtered out`
- `cargo test --test game_rules legacy_cache_read_weight_does_not_define_canonical_total_tokens`
  - Result: `1 passed; 0 failed; 26 filtered out`
- `cargo test --test storage_privacy canonical_total_queries_exclude_legacy_weighted_rows`
  - Result: `1 passed; 0 failed; 10 filtered out`
- `cargo test --test usage_provider`
  - Result: `19 passed; 0 failed`
- `cargo test --tests --no-run`
  - Result: compiled all integration test executables successfully.
- `cargo test --test runtime_integration`
  - Result: `25 passed; 0 failed`
- `cargo test --test activity_identity_cursors`
  - Result: `2 passed; 0 failed`
- `cargo test runtime::tests`
  - Result: `7 passed; 0 failed; 599 filtered out`
- `cargo test watch::tests`
  - Result: `20 passed; 0 failed; 586 filtered out`
- `cargo fmt --check`
  - Result: exit 0, no output.

## TDD Evidence

### RED command and failing output

`cargo test --test game_rules tokenmaxxing_total_counts_cache_reads_fully_and_excludes_reasoning`

```text
error[E0599]: no method named `total_tokens` found for struct `RawTokenTotals` in the current scope
  --> tests/game_rules.rs:60:23
...
error: could not compile `glorp` (test "game_rules") due to 2 previous errors
```

`cargo test --test game_rules legacy_cache_read_weight_does_not_define_canonical_total_tokens`

```text
error[E0599]: no method named `total_tokens` found for struct `RawTokenTotals` in the current scope
  --> tests/game_rules.rs:60:23
...
error: could not compile `glorp` (test "game_rules") due to 2 previous errors
```

`cargo test --test storage_privacy canonical_total_queries_exclude_legacy_weighted_rows`

```text
error[E0433]: cannot find `token_contract` in `usage`
error[E0560]: struct `NormalizedUsageEvent` has no field named `token_contract`
error[E0560]: struct `NormalizedUsageEvent` has no field named `total_tokens`
error[E0599]: no method named `canonical_total_tokens_between` found for struct `UsageStore`
error[E0599]: no method named `canonical_total_tokens_by_source_between` found for struct `UsageStore`
error: could not compile `glorp` (test "storage_privacy") due to 8 previous errors
```

### GREEN command and passing output

`cargo test --test game_rules tokenmaxxing_total_counts_cache_reads_fully_and_excludes_reasoning`

```text
running 1 test
test tokenmaxxing_total_counts_cache_reads_fully_and_excludes_reasoning ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 26 filtered out
```

`cargo test --test game_rules legacy_cache_read_weight_does_not_define_canonical_total_tokens`

```text
running 1 test
test legacy_cache_read_weight_does_not_define_canonical_total_tokens ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 26 filtered out
```

`cargo test --test storage_privacy canonical_total_queries_exclude_legacy_weighted_rows`

```text
running 1 test
test canonical_total_queries_exclude_legacy_weighted_rows ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out
```

`cargo test --test usage_provider`

```text
running 19 tests
...
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Files changed

- `src/usage/token_contract.rs`
- `src/usage/mod.rs`
- `src/usage/provider.rs`
- `src/usage/normalize.rs`
- `src/usage/ccusage.rs`
- `src/storage/usage_store.rs`
- `src/game/runtime.rs`
- `src/commands/watch.rs`
- `tests/game_rules.rs`
- `tests/storage_privacy.rs`
- `tests/runtime_integration.rs`
- `tests/activity_identity_cursors.rs`

## Self-review findings

- SQL INSERT/SELECT column order was reviewed against row index mappings after the storage changes.
- `ccusage` remains weighted-effective for both `effective_tokens` and `total_tokens`, matching the Task 1 constraint to avoid switching canonical providers early.
- Canonical storage query methods filter on `tokenmaxxing_total_v1` only, so legacy weighted rows are excluded.
- Existing effective-token APIs were left unchanged; later Tokenmaxxing consumption is not implemented here.

## Concerns

- The brief's file list did not include `src/game/runtime.rs`, `src/commands/watch.rs`, `tests/runtime_integration.rs`, or `tests/activity_identity_cursors.rs`, but those files directly construct the expanded provider structs. I made the smallest compatibility edits there so the repo compiles and legacy behavior stays unchanged.
- One exploratory verification command was mistyped as `cargo test runtime::tests watch::tests`, which Cargo rejected. I reran the intended filters separately as `cargo test runtime::tests` and `cargo test watch::tests`, both passing.
