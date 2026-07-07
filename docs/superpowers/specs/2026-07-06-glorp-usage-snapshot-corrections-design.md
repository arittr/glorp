# Glorp Usage Snapshot Corrections - design

- Date: 2026-07-06
- Status: proposed, revised after adversarial review
- Builds on:
  - `docs/superpowers/specs/2026-06-19-glorp-tokenmaxxing-token-contract-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-activity-identity-design.md`

## Problem

Glorp currently treats provider daily totals as monotonic counters. That is not
true for local provider reports such as `ccusage`.

On 2026-07-06, Claude local logs showed the failure mode clearly:

- naive summing of every July 6 Claude `usage` object was about `1.30B` tokens
- deduping by `message.id` was about `531M` tokens
- current `ccusage claude daily` output was about `531M` tokens
- Glorp had already fed and displayed about `1.06B` visible tokens for the day

The real tokens did not disappear. The provider report changed because it is a
mutable snapshot over local Claude JSONL files, including subagent, sidechain,
streaming, retry, and deduplication behavior. Glorp assumed ledger semantics and
persisted already-seen totals as pet food.

The first draft fixed only the "same row went down" case. That is not enough:
provider rows can disappear entirely, collector identities can shift, and a
temporary bad snapshot can later rebound. The fix must handle complete snapshot
runs, not only row-local upserts.

## Direction

Adopt a three-surface local accounting model:

- **Snapshot truth:** the latest complete provider snapshot for each provider
  day and accounting source. This drives visible day/source accounting.
- **Feed ledger:** append-only positive deltas that have already fed the pet.
  This drives XP, lifetime tokens, props, vitals, recent feed events, and
  narrative history.
- **Feed high-water baselines:** monotonic provider cursor baselines used only
  to decide whether future provider totals represent not-yet-fed food.

When provider totals decrease, Glorp updates visible snapshot truth, records a
correction, and feeds nothing. It does not move the feed high-water baseline
down. Future provider increases first catch up to what the pet already ate; only
tokens above the feed high-water baseline become new food.

That preserves the product decision from Option A:

- visible accounting becomes honest
- pet progress is not rolled back
- already-fed overage is not double-fed on a rebound

## Goals

- Make watch/status token totals match current provider truth after provider
  corrections.
- Prevent provider decreases and disappeared rows from permanently inflating
  visible daily totals.
- Keep pet progression stable and non-punitive: no negative food, no XP
  subtraction, no unevolving, and no prop removal.
- Avoid double-feeding a rebound after a transient or corrected decrease.
- Preserve the Tokenmaxxing total-token contract:

  ```text
  total_tokens = uncached_input
               + output
               + cache_creation
               + cache_read
  ```

- Keep source identity explicit and privacy-preserving.
- Make diagnostics clear enough that a future reset or repair can be explained
  from the local database without re-parsing all raw logs.

## Non-goals

- No direct dependency on the Tokenmaxxing API.
- No network requirement for normal usage ingestion.
- No negative usage events in the pet feed ledger.
- No retroactive XP, lifetime-token, prop, stage, or vital rollback.
- No native Claude transcript parser in this pass.
- No broad redesign of watch, companion, preview, or TUI layout.
- No snapshot observation history for sub-day provider-rate windows in this
  pass.

## Core Invariants

1. A successful helper invocation is a complete snapshot for its scoped provider
   day. Missing rows from that scope must be marked inactive or superseded.
2. Visible day/source queries use only active rows from the latest complete
   snapshot run.
3. Cursor identity and visible accounting identity are separate. Feed cursors
   stay collector-specific; visible totals group by canonical accounting source,
   provider day, and token contract.
4. Feed high-water baselines never move down during normal correction handling.
5. Missing or blocked snapshots are not zero-token truth. They are a degraded
   state and must be labeled or explicitly fall back to accepted-food numbers.
6. Corrections are not provider failures. They should be visible as
   non-blocking accounting notices.
7. Snapshot and correction storage must not persist prompts, responses, file
   paths, project names, raw transcript content, or unsanitized raw source ids.

## Product Contract

Visible accounting and pet progression intentionally diverge after provider
corrections.

Visible accounting answers:

> What does the provider currently say happened for this accounting source and
> Tokenmaxxing day?

Pet progression answers:

> What positive usage deltas did Glorp already accept as food?

This means a corrected day can show fewer visible tokens than the pet has
already eaten from that day. That is acceptable. The user-facing principle is:
Glorp should make the numbers honest without punishing the pet for a collector
correction.

### Surface Dictionary

| Surface | Field | Backing source | Axis | Contract |
| --- | --- | --- | --- | --- |
| watch | today total | snapshot | Tokenmaxxing provider day | current provider truth |
| watch | source breakdown | snapshot | Tokenmaxxing provider day | current provider truth by canonical accounting source |
| watch | source health today | snapshot | Tokenmaxxing provider day | current provider truth |
| watch | source health recent bucket | feed ledger | wall-clock 10m window | recent accepted food |
| watch | rate momentum | feed ledger | wall-clock 10m/1h windows | recent accepted food |
| watch | current bucket | feed ledger | wall-clock 10m window | recent accepted food |
| watch | 7-day token history | snapshot | Tokenmaxxing provider days | current provider truth per day when snapshots exist |
| watch | recent feed events | feed ledger | applied event time | what the pet ate |
| status | today | snapshot | Tokenmaxxing provider day | label as provider today/current snapshot |
| status | recent | feed ledger | last poll/apply | label as accepted recent food |
| status | lifetime | pet state/feed ledger | lifetime | label as pet lifetime food/progress |
| doctor | provider-day source totals | snapshot | Tokenmaxxing provider day | current provider truth |
| doctor | rolling recent totals | feed ledger unless snapshot history exists | wall-clock windows | accepted food, not provider truth |
| companion | today total | shared watch view model snapshot | Tokenmaxxing provider day | current provider truth |
| companion | rate values | shared watch view model feed ledger | wall-clock 10m/1h windows | recent accepted food |
| Activity Identity | source diversity and relative intensity | snapshot | current Tokenmaxxing provider day | presentation traits from current truth |
| Activity Identity | token shape, rhythm, recovery, live reactions | feed ledger | applied event/bucket time | what recently fed the pet |

Compact surfaces do not need verbose explanatory text, but internal field names
and tests must preserve this split. Text surfaces such as status and doctor must
avoid a single unlabeled line that implies provider today, recent food, and pet
lifetime are the same accounting system.

If snapshot data is missing, visible surfaces must not silently render `0` as
provider truth. They may either show a snapshot-pending state or show an
accepted-food fallback with an explicit degraded label.

## Storage Model

Add provider snapshot storage to `UsageStore`.

### Snapshot Runs

Each successful helper invocation creates a `provider_snapshot_runs` row for a
complete scope:

- run id
- collector surface, for example `ccusage:claude-code`, `ccusage-codex`, or
  `agentsview:claude`
- command
- token contract
- provider day
- provider version
- parser version
- observed time
- completion status

A complete scope is the collector surface, command, token contract, and provider
day returned by one successful helper invocation. If a helper returns multiple
provider days, each provider day is replaced independently inside the same
transaction.

An early helper exit, timeout, missing helper, invalid JSON result, or malformed
top-level response does not create a complete run and must not tombstone prior
active snapshot rows.

### Snapshot Rows

Each normalized provider record in a complete run becomes a
`provider_snapshot_rows` row.

The row has both canonical accounting identity and collector identity.

Canonical accounting identity:

- token contract
- accounting source, for example `claude-code`, `codex`, or another sanitized
  source label
- provider day
- model when present

Collector identity:

- collector surface
- command
- source surface
- provider period
- raw source id hash when present
- cursor key hash

Stored values:

- raw token buckets
- `total_tokens`
- cost metadata when available
- confidence
- provider version
- parser version
- run id
- active/superseded status
- first observed time
- last observed time

The snapshot write transaction must:

1. create the complete run
2. mark previous active rows for the same complete scope and provider day as
   superseded
3. insert the new active rows
4. compare prior active rows with new active rows and record corrections for
   lower totals and disappeared rows

Display queries must read active rows only. This is what fixes the disappeared
row case: if a model/source row was present in the old run and absent in the new
complete run, it no longer contributes to visible totals.

### Feed High-Water Baselines

Existing provider cursors should become feed high-water baselines, not visible
truth baselines.

Feed high-water baselines:

- are keyed by collector-specific cursor identity
- advance only after feed ledger rows are durably applied and pet state is saved
- never move down during normal correction handling
- store raw bucket baselines plus a total-token high-water value

The no-previous-feed-cursor case remains runtime-owned first contact. Providers
may emit the initial positive delta as they do today; runtime stages it as
non-feedable seeded history, records `source_first_contact`, and advances the
high-water cursor after state save. Do not bypass that path in the provider.

### Corrections

Add a `provider_corrections` table. Provider diagnostics may link to correction
rows, but correction details should not be crammed into human-readable strings.

Each correction record should include:

- correction kind: `row_decreased`, `row_removed`, `mixed_bucket_correction`,
  or `snapshot_unavailable`
- accounting source
- provider day
- model when present
- previous total
- current total, or zero/absent for removed rows
- decrease amount
- previous raw token buckets
- current raw token buckets when present
- collector surface
- cursor key hash
- run id
- recorded time

Doctor/status should render a concise safe message such as:

```text
claude-code corrected 490M tokens downward for 2026-07-06; visible totals updated, pet progress kept
```

The stored context must be sufficient for debugging without storing raw prompts,
responses, file paths, project names, or raw source ids.

## Polling And Feeding Flow

For each provider helper invocation:

1. Invoke the helper.
2. If invocation fails or the top-level response is invalid, persist a normal
   diagnostic and leave prior snapshots active.
3. Normalize all records.
4. Parse provider dates on the Tokenmaxxing America/Los_Angeles day axis. A
   date-only provider period such as `2026-07-06` means the LA accounting day
   that starts at local midnight on that date, not UTC midnight.
5. In one `UsageStore` transaction, write a complete snapshot run, replace active
   rows for that scope/day, and persist correction records.
6. Only after snapshot storage succeeds, evaluate feed deltas against the
   feed high-water cursor.

Feed delta rules for each normalized record:

- If there is no feed cursor, preserve runtime-owned first contact behavior.
- If the current total is less than or equal to the feed high-water total, emit
  no feed delta and do not lower the feed cursor.
- If the current total is greater than the feed high-water total and all raw
  buckets are greater than or equal to the cursor buckets, emit the bucket-wise
  positive delta.
- If the current total is greater than the feed high-water total but one or more
  raw buckets decreased, emit only the net-positive `total_tokens` above the
  high-water total as low-confidence shape data, record a
  `mixed_bucket_correction`, and avoid pretending the bucket mix is precise.

Cursor advancement remains delayed until after pet state save and feed ledger
apply, as today. Snapshot writes must not cause pet XP or lifetime changes by
themselves.

## Display Query APIs

Do not retarget existing ledger window APIs globally. Add explicit snapshot APIs
and keep accepted-food APIs explicit.

Snapshot APIs:

- `snapshot_total_tokens_for_provider_day(day)`
- `snapshot_total_tokens_by_source_for_provider_day(day)`
- `snapshot_token_history_for_provider_days(days)`
- `snapshot_state_for_provider_day(day)` returning current, stale, missing, or
  blocked

Feed-ledger APIs:

- accepted tokens between wall-clock bucket bounds
- accepted tokens by source between wall-clock bucket bounds
- accepted bucket sums for rate momentum
- accepted token shape for live reactions
- recent feed events
- lifetime pet food/progress

The implementation should rename or wrap ambiguous methods such as
`canonical_total_tokens_between` so call sites cannot accidentally use provider
day snapshots for live rates or accepted-food windows.

## Existing Data And Repair

No destructive repair is required for pet state.

Migration creates empty snapshot tables and leaves `usage_events`,
provider cursors, lifetime counters, XP, vitals, props, and narrative history
unchanged. It must not infer provider snapshots from `usage_events`; the feed
ledger records what the pet ate, not current provider truth.

After upgrade, the next successful provider poll populates snapshots from
current provider output. Until then, snapshot-backed surfaces are degraded:

- show snapshot pending/unavailable, or
- show accepted-food fallback with an explicit label

Implement a non-destructive repair path, either
`glorp doctor --refresh-usage-snapshots` or `glorp usage repair`, that:

- repolls providers
- replaces scoped snapshots
- leaves pet state and feed ledger untouched
- prints before/after visible provider totals
- reports any blocked provider scopes

Partial snapshot runs must not become active. If a process dies mid-write,
transaction boundaries should leave either the old complete snapshot or the new
complete snapshot, never a half-replaced day.

## Diagnostics And Health

Corrections are non-blocking accounting notices.

`cursor_total_decreased` and row-removal corrections should not mark a source as
provider-blocked by themselves. Status/watch/doctor should distinguish:

- provider blocked: missing helper, timeout, invalid JSON, malformed required
  fields
- provider corrected: lower or missing rows in a complete snapshot
- snapshot pending: no complete snapshot exists yet for the requested day

Fresh corrections should be surfaced on text diagnostics so the user can
understand why visible totals dropped while pet progress stayed high.

## Provider Coverage

Snapshot storage is provider-agnostic. Any provider that can emit normalized
Tokenmaxxing-compatible records must write snapshots, including both the current
`ccusage` path and `AgentsviewCommandProvider`.

The later canonical migration from `ccusage` to `agentsview` still matters, but
snapshot correction handling must not be hard-coded to one collector. During a
collector cutover, only one canonical collector should be active for a given
accounting source/token contract/day, or old collector rows must be superseded
so visible queries do not double-count old and new collectors.

## Testing

Add focused tests for:

- lower provider totals update snapshot totals without emitting negative food
- disappeared model/source rows stop contributing to visible totals after the
  next complete scoped snapshot run
- feed high-water cursors do not move down on decreases
- a rebound below the old high-water does not double-feed the pet
- a net-positive mixed-bucket correction feeds only the new total above
  high-water and marks token shape low-confidence
- no-cursor first contact still uses runtime-owned non-feedable seeding
- watch today totals read corrected snapshot totals instead of old applied feed
  rows
- watch source breakdown follows snapshot totals after a decrease
- watch source-health recent bucket and rate windows remain accepted-food-backed
- status labels provider today, accepted recent food, and pet lifetime food as
  separate concepts
- doctor surfaces fresh corrections as non-blocking notices
- existing pet XP/lifetime/vitals/props remain unchanged after a decrease
- existing cursor/no snapshot migration returns snapshot pending or labeled
  accepted-food fallback, not zero provider truth
- LA provider-day parsing around UTC/LA midnight boundaries
- structured diagnostics include previous/current totals and bucket details
  without raw source ids
- agentsview and ccusage paths both write provider snapshots
- 7-day visible history uses snapshot days or explicitly degrades when snapshots
  are missing
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
