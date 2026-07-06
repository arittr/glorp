# Glorp Usage Snapshot Corrections - design

- Date: 2026-07-06
- Status: proposed
- Builds on:
  - `docs/superpowers/specs/2026-06-19-glorp-tokenmaxxing-token-contract-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-activity-identity-design.md`

## Problem

Glorp currently treats provider daily totals as monotonic counters. That is not
true for `ccusage`.

On 2026-07-06, Claude local logs showed the failure mode clearly:

- naive summing of every July 6 Claude `usage` object was about `1.30B` tokens
- deduping by `message.id` was about `531M` tokens
- current `ccusage claude daily` output was about `531M` tokens
- Glorp had already fed and displayed about `1.06B` visible tokens for the day

The real tokens did not disappear. The provider report changed because it is a
mutable snapshot over local Claude JSONL files, including subagent, sidechain,
streaming, retry, and deduplication behavior. Glorp assumed ledger semantics and
persisted already-seen totals as pet food.

## Direction

Adopt a two-surface local accounting model:

- **Snapshot truth:** the latest provider total for each source, day, model, and
  cursor identity. This drives visible accounting surfaces.
- **Feed ledger:** append-only positive deltas that have already fed the pet.
  This drives XP, lifetime tokens, props, vitals, recent feed events, and
  narrative history.

When provider totals increase, Glorp may feed the positive delta. When provider
totals decrease, Glorp updates snapshot truth and cursor baselines, records a
diagnostic, and feeds nothing. Existing pet progress is not subtracted.

## Goals

- Make watch/status token totals match current provider truth after provider
  corrections.
- Prevent a provider decrease from permanently inflating visible daily totals.
- Keep pet progression stable and non-punitive: no negative food, no XP
  subtraction, no unevolving, and no prop removal.
- Keep future feeding correct after a decrease by resetting the baseline to the
  corrected provider total.
- Preserve the Tokenmaxxing total-token contract:

  ```text
  total_tokens = uncached_input
               + output
               + cache_creation
               + cache_read
  ```

- Keep source identity explicit and privacy-preserving.
- Make diagnostics clear enough that a future reset can be explained from the
  local database without re-parsing all raw logs.

## Non-goals

- No direct dependency on the Tokenmaxxing API.
- No network requirement for normal usage ingestion.
- No negative usage events in the pet feed ledger.
- No retroactive XP, lifetime-token, prop, stage, or vital rollback.
- No native Claude transcript parser in this pass.
- No broad redesign of watch, companion, preview, or TUI layout.

## Product Contract

Visible accounting and pet progression intentionally diverge after provider
corrections.

Visible accounting answers:

> What does the provider currently say happened for this source and
> Tokenmaxxing day?

Pet progression answers:

> What positive usage deltas did Glorp already accept as food?

This means a corrected day can show fewer visible tokens than the pet has
already eaten from that day. That is acceptable. The user-facing principle is:
Glorp should make the numbers honest without punishing the pet for a collector
correction.

The visible day/source surfaces that must use snapshot truth are:

- watch today total
- watch source breakdown
- watch source health today and recent-bucket token values
- status today total
- status source breakdown
- doctor recent canonical source totals
- companion today total

Short live-rate surfaces should remain feed-ledger-backed in the first pass and
be labeled internally as recent accepted food:

- watch rate momentum windows
- companion rate values

The pet surfaces that remain ledger-backed are:

- XP and stage
- lifetime food/token counters
- vitals
- habitat unlocks
- recent feed events
- narrative entries
- applied usage pulse effects

Activity Identity should use snapshot truth for current-day source diversity
and relative intensity, because those are presentation traits. It may continue
using feed-ledger shape data for short-lived live reactions where the intended
input is "what just fed the pet."

## Storage Model

Add a provider snapshot table to `UsageStore`.

Each row represents the current provider total for one logical accounting row.
The logical identity is the same identity already used for provider cursors:

- provider surface
- token contract
- command
- source surface
- provider period
- model
- raw source id when present

The stored values are:

- raw token buckets
- `total_tokens`
- cost metadata when available
- confidence
- provider version
- parser version
- first observed time
- last observed time
- last decrease time
- decrease count

The table must upsert on the logical identity. Upserts can move totals up or
down. Decreases are not errors in the snapshot table; they are normal provider
corrections with diagnostics.

Snapshot rows should use the provider's accounting day as their day key. For
Tokenmaxxing-compatible rows this remains the America/Los_Angeles day from the
provider period, not the smear bucket time used by pet feeding.

## Polling And Feeding Flow

For each normalized provider record:

1. Build the provider cursor key.
2. Read the previous cursor value, if any.
3. Upsert the current raw totals into the snapshot table.
4. If there is no previous cursor, seed the cursor and feed nothing.
5. If current totals are greater than or equal to previous totals, emit a
   positive `UsageDelta` for the bucket-wise difference.
6. If any current bucket is less than the previous bucket:
   - record a `cursor_total_decreased` diagnostic with previous total, current
     total, and delta amount
   - advance the provider cursor to the current raw totals
   - feed nothing

The current `cursor_total_decreased` behavior already advances the cursor and
feeds nothing. The important change is that visible totals must also be updated
to the lower snapshot value.

## Display Queries

Add snapshot-backed canonical token queries to `UsageStore`:

- total tokens between provider-day bounds
- total tokens by source between provider-day bounds
- recent/rate window totals when the requested window can be answered from
  snapshot observations

For day totals, snapshot rows should be grouped by provider period/day.

For short rate windows, Glorp needs one of two strategies:

1. Continue using feed ledger rows for sub-day rate windows, accepting that the
   rate is "recent accepted food" rather than current provider truth.
2. Add observation history for snapshots so rate windows can answer "provider
   total increased during this wall-clock window."

The first implementation should choose strategy 1 for short windows. It is
smaller, preserves existing animation behavior, and avoids storing a full
snapshot history. The visible day/source totals are the urgent correctness
problem.

Rate labels should be documented as recent accepted tokens until a later
snapshot-history pass changes that contract.

## Existing Data Repair

No destructive repair is required for pet state.

After display surfaces switch to snapshot day totals, inflated visible totals
will fall back to current provider truth on the next successful poll. Existing
feed-ledger rows remain in `usage_events` because they record what the pet
already ate.

For installs that already have provider cursors but no snapshot rows, migration
should not infer snapshots from `usage_events`. It should wait for the next
provider poll and populate snapshots from current provider output.

## Diagnostics

`cursor_total_decreased` diagnostics should include enough structured context
to explain corrections:

- provider surface
- cursor key
- previous total
- current total
- decrease amount
- previous raw token buckets
- current raw token buckets

The human-readable message should stay concise, for example:

```text
claude-code corrected 490M tokens downward for 2026-07-06 model totals
```

Doctor/status can surface recent corrections as non-blocking diagnostics. A
correction means the provider changed its current snapshot; it does not mean the
provider is broken.

## Testing

Add focused tests for:

- a provider decrease updates snapshot totals and advances the cursor without
  emitting a feed delta
- watch today totals read the corrected snapshot total instead of old applied
  feed rows
- existing pet XP/lifetime remains unchanged after a decrease
- source breakdown follows snapshot totals after a decrease
- the first poll on an existing cursorless snapshot seeds baseline without
  feeding
- diagnostics include previous and current totals
- legacy applied rows with `tokenmaxxing_total_v1` do not inflate snapshot
  display totals after a lower provider snapshot is observed

Existing tests that assert pet progression from positive deltas should remain
ledger-backed.

## Open Follow-Ups

- Migrate canonical Claude collection from `ccusage` to `agentsview` per the
  Tokenmaxxing contract spec. Snapshot correction handling is still useful even
  after that migration because local provider reports can remain mutable.
- Rename transitional `effective_tokens` fields in presentation models once the
  snapshot/ledger split has settled.
- Consider snapshot observation history for true provider-rate windows if
  recent accepted-food rate is not good enough.
