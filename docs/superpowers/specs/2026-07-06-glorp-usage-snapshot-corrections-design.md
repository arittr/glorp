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

1. A successful helper invocation is a complete snapshot for its requested
   provider-day scopes. Missing rows from each complete scope must be marked
   inactive or superseded, including the case where a requested day returns zero
   rows.
2. Visible day/source queries use only active rows from the canonical collector
   assignment and latest complete snapshot run for the requested scope. Missing
   or blocked snapshots are typed states, not numeric zeroes.
3. Cursor identity and visible accounting identity are separate. Feed cursors
   stay collector-specific; visible totals group by canonical accounting source,
   provider day, and token contract; feed eligibility is guarded by a canonical
   high-water baseline so collector cutovers cannot double-feed old overage.
4. Feed high-water baselines never move down during normal correction handling.
5. Missing or blocked snapshots are not zero-token truth. They are a degraded
   state and must be labeled or explicitly fall back to accepted-food numbers.
6. Corrections are not provider failures. They should be visible as
   non-blocking accounting notices.
7. For a given accounting source, token contract, and provider day, at most one
   collector can be canonical-visible.
8. Snapshot, diagnostic, and correction storage must not persist prompts,
   responses, file paths, project names, raw transcript content, or unsanitized
   raw source ids.

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

| Surface | Field | Backing source | Axis | Contract | Missing snapshot behavior |
| --- | --- | --- | --- | --- | --- |
| watch | today total | snapshot | Tokenmaxxing provider day | current provider truth | show snapshot pending |
| watch | source breakdown | snapshot | Tokenmaxxing provider day | current provider truth by canonical accounting source | show snapshot pending |
| watch | source health today | snapshot | Tokenmaxxing provider day | current provider truth | show pending, not blocked |
| watch | source health recent bucket | feed ledger | wall-clock 10m window | recent accepted food |
| watch | rate momentum | feed ledger | wall-clock 10m/1h windows | recent accepted food |
| watch | current bucket | feed ledger | wall-clock 10m window | recent accepted food |
| watch | 7-day token history | snapshot | Tokenmaxxing provider days | current provider truth per day when snapshots exist | render missing days as unavailable, not zero |
| watch | recent feed events | feed ledger | applied event time | what the pet ate |
| status | today | snapshot | Tokenmaxxing provider day | label as provider today/current snapshot | print snapshot pending |
| status | recent | feed ledger | last poll/apply | label as accepted recent food |
| status | lifetime | pet state/feed ledger | lifetime | label as pet lifetime food/progress |
| doctor | provider-day source totals | snapshot | Tokenmaxxing provider day | current provider truth | print snapshot pending/block reason |
| doctor | rolling recent totals | feed ledger unless snapshot history exists | wall-clock windows | accepted food, not provider truth |
| companion | today total | shared watch view model snapshot | Tokenmaxxing provider day | current provider truth | show pending state, not zero |
| companion | rate values | shared watch view model feed ledger | wall-clock 10m/1h windows | recent accepted food |
| Activity Identity | source diversity and relative intensity | snapshot | current Tokenmaxxing provider day | presentation traits from current truth | use neutral/unknown traits |
| Activity Identity | token shape, rhythm, recovery, live reactions | feed ledger | applied event/bucket time | what recently fed the pet |

Compact surfaces do not need verbose explanatory text, but internal field names
and tests must preserve this split. Text surfaces such as status and doctor must
avoid a single unlabeled line that implies provider today, recent food, and pet
lifetime are the same accounting system.

If snapshot data is missing, visible surfaces must not silently render `0` as
provider truth. Snapshot-backed surfaces use the missing-snapshot behavior above.
Accepted-food surfaces keep rendering because they do not claim provider truth.

## Storage Model

Add provider snapshot storage to `UsageStore`.

### Snapshot Batches

Each successful helper invocation creates one `provider_snapshot_batches` row:

- batch id
- collector scope id, a stable internal key such as `claude-code:local-usage`
  or `codex:local-usage`
- collector surface, for example `ccusage:claude-code`, `ccusage-codex`, or
  `agentsview:claude`
- command, for diagnostics only
- token contract
- requested provider days covered by the invocation
- provider version
- parser version
- observed time
- completion status

An early helper exit, timeout, missing helper, invalid JSON result, or malformed
top-level response does not create a complete batch and must not tombstone prior
active snapshot rows. It must still write a provider diagnostic for the
requested scope, and that diagnostic counts as the latest blocked attempt for
`SnapshotResult` state precedence.

### Snapshot Runs

Before the helper result is written, the caller must know the requested provider
days for that invocation. Normal polling requests the current Tokenmaxxing day.
Repair and history refreshes request an explicit day list. A helper result can
only replace snapshots for requested days; unexpected extra days are diagnostic
noise unless the caller explicitly requested them.

Each complete batch creates one `provider_snapshot_runs` row per requested
provider day, including zero-row runs for requested days that produce no
normalized records:

- run id
- batch id
- replacement scope id
- collector scope id
- collector surface, for example `ccusage:claude-code`, `ccusage-codex`, or
  `agentsview:claude`
- command, for diagnostics only
- token contract
- provider day
- provider version
- parser version
- observed time
- completion status

A complete replacement scope is one run: replacement scope id, token contract,
and provider day. The replacement scope id is normalized code-owned state, not
raw argv. If a helper invocation covers multiple requested provider days, the
batch contains multiple runs and each provider day is replaced independently
inside the same transaction.

Row-level required-field failures are fail-closed. If a record for a requested
day cannot be normalized because required fields are missing or malformed, the
affected run is blocked and prior rows for that run's replacement scope remain
active. If the provider day cannot be determined, the whole batch is blocked.
Blocked runs write diagnostics but do not supersede prior active rows.

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

- replacement scope id
- collector scope id
- collector surface
- command, for diagnostics only
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

1. create the complete batch and its provider-day runs
2. validate every requested run before replacing rows
3. for each complete run, mark previous active rows for the run's complete
   replacement scope as superseded
4. insert the new active rows, if any
5. compare prior active rows with new active rows by source-day aggregate and
   canonical accounting identity, then record corrections for lower totals and
   disappeared rows

Display queries must read active rows only from the canonical collector
assignment for the requested accounting source, token contract, and provider
day. This fixes both disappeared-row and disappeared-day cases: if a model/source
row was present in the old run and absent in the new complete run, or a requested
day now has a complete zero-row run, it no longer contributes to visible totals.

Correction comparison has two levels.

Source-day aggregate comparison uses:

- token contract
- accounting source
- provider day

This is the user-facing correction level. Doctor/status should report a downward
source correction only when the source-day aggregate total decreases.

Row comparison uses canonical accounting identity:

- token contract
- accounting source
- provider day
- model when present

Collector identity fields such as cursor key hash, source surface, or command
are diagnostic context. A collector identity shift that leaves canonical
identity and totals unchanged must not create a false downward correction or new
feed.

Model-label churn with unchanged source-day aggregate should be recorded as an
identity-remap diagnostic if useful, not as a user-facing `row_removed`
correction. Row-level removals are actionable correction details only when they
contribute to a net source-day decrease.

### Feed High-Water Baselines

Existing provider cursors should become feed high-water baselines, not visible
truth baselines.

Feed state has three related baselines:

- collector feed cursors keyed by collector-specific cursor identity, kept for
  provider/runtime continuity
- canonical source-day high-waters keyed by token contract, accounting source,
  and provider day, used as the aggregate guard against source/model identity
  churn
- canonical row high-waters keyed by token contract, accounting source, provider
  day, and model when present, used for exact bucket attribution when row shape
  is stable

Feed eligibility uses the greater of the collector cursor total, canonical row
high-water total, and canonical source-day aggregate high-water for the same
accounting source/day. A new collector must seed its collector cursor to at
least the canonical source-day high-water before it can emit feed. That means a
corrected replacement collector cannot re-feed tokens that an older collector
already overfed, even if model labels appear, disappear, or shift.

Before emitting any model-scoped feed delta, compute the current complete
snapshot aggregate for the accounting source/day and compare it with the
source-day aggregate high-water. If the source-day aggregate has not exceeded
that high-water, no rows for that source/day can feed. If it has exceeded the
high-water, the total feed emitted across all rows in that source/day is capped
at the aggregate excess. After pet state save, advance both the affected row
high-water and the source-day aggregate high-water.

The aggregate cap is a safety guard, not an allocation oracle. If exact
candidate row deltas exceed the aggregate excess, or if the excess cannot be
attributed to stable row identity without guessing, emit the excess as a
source-day total-only delta with `confidence = corrected-total-only`.

Feed high-water baselines:

- advance only after feed ledger rows are durably applied and pet state is saved
- never move their total-token high-water value down during normal correction
  handling
- store total-token high-water values at both row and source-day aggregate levels
- store the latest raw buckets for diagnostics when present
- store an exact bucket high-water when bucket attribution is trustworthy
- store a bucket-baseline confidence: `exact` or `corrected-total-only`
- store unshaped total-only food since the last exact bucket baseline

The no-previous-feed-cursor case remains runtime-owned first contact only when
the accounting source itself has no prior feed history or source registration.
It is not day-level. A new provider day for a known source starts with row and
source-day high-waters at zero and uses normal feed rules, so the first poll of a
new day can still feed the pet.

Source registration means persisted feed/contact state, such as
`source_first_contact`, a prior feed cursor or high-water, or an explicit cutover
seed. Configured collector assignments, health-row enumeration, and
snapshot-only repair rows do not count as source registration.

On true source first contact, providers may emit the initial positive delta as
they do today; runtime stages it as non-feedable seeded history, records
`source_first_contact`, and advances collector, row, and source-day high-waters
after state save. Do not bypass that path in the provider. If first contact
lacks complete raw buckets, the baseline is `corrected-total-only`, exact bucket
high-water is unset, and future feed remains total-only until a non-feeding
exact resync establishes complete buckets. If a collector cursor is missing but
canonical source-day high-water exists, seed the collector cursor from the
canonical high-water and apply normal feed rules; this is a collector cutover,
not first contact.

### Corrections And Diagnostics

Add a `provider_corrections` table. Corrections are for complete provider
snapshots that changed current truth. Provider failures, blocked runs, and
missing snapshots belong in provider diagnostics instead.

Each provider correction record should include:

- correction kind: `row_decreased` or `row_removed`
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
- batch id
- run id
- recorded time

Provider and feed-baseline diagnostics should include:

- diagnostic kind: `helper_unavailable`, `helper_timeout`, `invalid_json`,
  `malformed_required_fields`, `unexpected_provider_day`,
  `snapshot_unavailable`, `run_blocked`, `identity_remap`, or
  `mixed_bucket_correction`
- collector scope id
- replacement scope id when known
- requested provider days
- provider day when known
- reason code
- blocked batch id or run id when one exists
- recorded time

For `mixed_bucket_correction`, diagnostics also record:

- effective high-water total
- current total
- unshaped feed amount
- previous exact raw buckets when present
- current raw buckets when present
- bucket-baseline confidence before and after the feed

Doctor/status should render a concise safe message such as:

```text
claude-code corrected 490M tokens downward for 2026-07-06; visible totals updated, pet progress kept
```

The stored context must be sufficient for debugging without storing raw prompts,
responses, file paths, project names, or raw source ids.

## Polling And Feeding Flow

For each provider helper invocation:

1. Determine the requested provider-day set before invoking the helper.
2. Invoke the helper.
3. If invocation fails or the top-level response is invalid, persist a normal
   diagnostic and leave prior snapshots active.
4. Normalize all records. Required-field failures block the affected requested
   run before any tombstoning. If the affected provider day is unknown, block the
   entire batch.
5. Parse provider dates on the Tokenmaxxing America/Los_Angeles day axis. A
   date-only provider period such as `2026-07-06` means the LA accounting day
   that starts at local midnight on that date, not UTC midnight.
6. In one `UsageStore` transaction, write the complete snapshot batch and its
   provider-day runs, including complete zero-row runs for requested days with no
   records, replace active rows for each complete run's scope/day, and persist
   correction records.
7. Only after snapshot storage succeeds, evaluate feed deltas against the
   effective feed high-water for normalized records that belong to complete,
   unblocked, requested provider-day runs. Records from unexpected extra days or
   blocked runs are diagnostics only and must not feed.

Feed evaluation is source-day-first. Group normalized records by accounting
source and provider day, compute all candidate row deltas for the group, and
emit feed only after checking the source-day aggregate guard.

Feed delta rules for each complete source-day group:

- If there is no collector cursor, no canonical row or source-day high-water for
  the feed key, and the accounting source has no prior feed history or source
  registration, preserve runtime-owned first contact behavior.
- If the accounting source is known but the provider day has no same-day row or
  source-day high-water yet, initialize same-day high-waters at zero and use
  normal feed rules.
- If there is no collector cursor but a canonical row or source-day high-water
  exists, seed the collector cursor from the canonical high-water before
  evaluating feed.
- Compute the source-day aggregate excess. If the current complete source-day
  aggregate is less than or equal to the source-day aggregate high-water, no row
  for that source/day can feed.
- For each row, compute the effective row high-water as the max of the collector
  cursor total and canonical row high-water total.
- If a row's current total is less than or equal to its effective row
  high-water, emit no row feed delta and do not lower any high-water. If complete
  raw buckets are present and match or exceed the latest exact bucket high-water,
  this no-feed poll may establish an exact resync for that row: set exact bucket
  high-water to the current buckets, clear unshaped total-only debt, and return
  confidence to `exact`.
- Build exact row candidates only for rows whose current total is greater than
  the effective row high-water, whose confidence is `exact`, whose raw buckets
  are complete, and whose raw buckets are greater than or equal to the exact
  bucket high-water.
- Emit bucket-wise exact row deltas only when exact candidate deltas sum exactly
  to the source-day aggregate excess and attribution is stable. If candidate
  deltas exceed the aggregate excess, fall short of it, or require choosing among
  remapped rows, emit one source-day total-only delta for the aggregate excess.
- After exact row deltas and pet state save, advance affected row high-waters and
  the source-day aggregate high-water to the current complete snapshot values.
- If any row that would otherwise feed has missing raw buckets, confidence
  `corrected-total-only`, or a raw bucket below the exact bucket high-water,
  emit one source-day total-only delta for the aggregate excess. The delta uses
  `confidence = corrected-total-only`, carries `token_totals = None`, must not
  contribute to token-shape/personality buckets, and records a
  `mixed_bucket_correction` feed-baseline diagnostic.
- After a total-only feed delta and pet state save, advance total high-waters to
  the current total and source-day aggregate high-water to the current aggregate,
  store latest raw buckets for diagnostics when present, keep the previous exact
  bucket high-water unchanged, increment unshaped total-only food, and keep
  `bucket_baseline_confidence = corrected-total-only`.

Cursor advancement remains delayed until after pet state save and feed ledger
apply, as today. Snapshot writes must not cause pet XP or lifetime changes by
themselves.

## Display Query APIs

Do not retarget existing ledger window APIs globally. Add explicit snapshot APIs
and keep accepted-food APIs explicit.

Snapshot APIs:

- `snapshot_totals_for_provider_day(day) -> SnapshotResult<DayTotals>`
- `snapshot_totals_by_source_for_provider_day(day) -> SnapshotResult<SourceTotals>`
- `snapshot_token_history_for_provider_days(days) -> Vec<SnapshotResult<DayTotals>>`
- `snapshot_health_for_provider_day(day) -> Vec<SourceSnapshotHealth>`

`SnapshotResult<T>` must carry a state and optional value:

- state: `current`, `stale`, `missing`, or `blocked`
- value: present only for `current` or `stale`
- provider day
- observed time when present
- block or pending reason when present

Numeric snapshot totals are unavailable unless `value` is present. UI and status
callers must branch on the typed state; missing or blocked snapshots cannot be
converted to `0` provider truth by helper defaults.

State precedence considers both snapshot runs and top-level failure diagnostics
for the requested scope:

- `current`: the latest attempted run for the requested scope is complete and
  selected by the canonical collector assignment
- `stale`: the latest attempt is blocked, but an older complete canonical
  snapshot value exists; render the old value only with a stale/blocked label
- `blocked`: the latest attempt is blocked and no older complete canonical value
  exists
- `missing`: no complete or blocked attempt exists yet for the requested scope

`pending` is display copy for `missing` with reason `not_polled`; it is not a
separate storage state.

A complete zero-row run is different from a missing snapshot. It returns
`state = current` with a numeric zero value because the requested provider day
was successfully covered and the provider reported no rows for that scope.

`SourceSnapshotHealth` must include snapshot state separately from recent
accepted-food state. A source can be snapshot-pending while it still has recent
accepted food, and the UI should not collapse that into `Ready`.

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
current provider output. Until then, snapshot-backed surfaces follow the
surface dictionary's missing-snapshot behavior. Accepted-food surfaces continue
rendering because they do not claim provider truth.

Implement a non-destructive repair path, either
`glorp doctor --refresh-usage-snapshots` or `glorp usage repair`, that:

- repolls providers
- requests an explicit provider-day set and replaces only those scoped snapshots
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

Source-health rows have two independent statuses:

- snapshot state for current provider truth
- recent accepted-food state for rate/reaction windows

Health rows should be enumerated from configured active collector assignments
plus sources seen in recent feed or snapshot history. A configured source with no
snapshot rows yet must still produce a health row with snapshot state `missing`
or `pending`, rather than disappearing from the UI.

Snapshot state takes precedence for provider-truth fields. Recent accepted food
can keep rate and liveliness indicators active, but it must not hide a
`missing`, `blocked`, or `pending` snapshot state for today's provider total.

Fresh corrections should be surfaced on text diagnostics so the user can
understand why visible totals dropped while pet progress stayed high.

## Provider Coverage

Snapshot storage is provider-agnostic. Any active collector that can emit
normalized Tokenmaxxing-compatible records must write snapshots. Before the
agentsview cutover, the current `ccusage` path may be the active collector. Once
`AgentsviewCommandProvider` is cut over for a canonical accounting source, it is
the only active collector for that source/token contract/day.

Store or derive a canonical collector assignment for each accounting source,
token contract, and provider day. Visible snapshot queries must join through
that assignment, so old active rows from a previous collector cannot double-count
with the new canonical collector. The assignment, not row insertion order,
defines which collector is canonical-visible.

The later canonical migration from `ccusage` to `agentsview` still matters, but
snapshot correction handling must not be hard-coded to one collector. During a
collector cutover, old `ccusage` snapshot rows for that accounting
source/token contract/day must either be superseded by a cutover run or excluded
from canonical-visible queries by the collector assignment. After cutover,
`ccusage` must not feed canonical pet progression or claim canonical
Tokenmaxxing totals for that source.

Cutover also seeds feed baselines: before a replacement collector can feed, its
collector cursor and the canonical high-water for the same accounting
source/token contract/provider day/model key must be at least the prior
canonical high-water or accepted total. This prevents corrected replacement
collectors from re-feeding old overage between the corrected total and the
previous overcounted high-water.

If model identity changes during cutover, the source-day aggregate high-water is
the guard. New or remapped model keys cannot feed while the source-day aggregate
remains below the prior accepted source-day total.

## Testing

Add focused tests for:

- lower provider totals update snapshot totals without emitting negative food
- disappeared model/source rows stop contributing to visible totals after the
  next complete scoped snapshot run
- a requested provider day with no returned rows writes a complete zero-row run
  and removes the old day from visible totals
- an unexpected extra provider day is diagnostic noise and does not replace
  snapshots, create zero-row runs, or feed
- row-level required-field failures block the affected run and leave prior
  active rows visible
- valid-looking records from blocked runs are excluded from feed evaluation
- a latest blocked run with prior good data returns `stale` with value and block
  reason, not `current`
- a missing helper or invalid JSON before any successful snapshot returns
  `blocked` with no value, not `missing/not_polled`
- feed high-water cursors do not move down on decreases
- a rebound below the old high-water does not double-feed the pet
- collector cutover seeds the replacement collector from canonical high-water so
  corrected lower totals cannot re-feed prior overage
- source-day aggregate high-water prevents double-feed when model labels are
  added, removed, or remapped
- source-day-first feed evaluation falls back to one total-only delta when exact
  row allocation would require guessing
- a net-positive mixed-bucket correction feeds only the new total above
  high-water as `corrected-total-only` with no token-shape contribution and
  records a feed-baseline diagnostic
- exact bucket-wise feeding does not resume after `corrected-total-only` until a
  non-feeding exact resync establishes a new exact bucket baseline
- no-cursor first contact still uses runtime-owned non-feedable seeding
- a new provider day for a known source feeds from zero instead of being treated
  as source first contact
- configured collectors, health rows, and snapshot-only repair rows do not count
  as source registration for first-contact seeding
- first contact with missing raw buckets seeds `corrected-total-only` and does
  not emit token-shape/personality buckets
- watch today totals read corrected snapshot totals instead of old applied feed
  rows
- watch source breakdown follows snapshot totals after a decrease
- watch source-health recent bucket and rate windows remain accepted-food-backed
- source health exposes snapshot state separately from recent accepted-food state
- status labels provider today, accepted recent food, and pet lifetime food as
  separate concepts
- doctor surfaces fresh corrections as non-blocking notices
- existing pet XP/lifetime/vitals/props remain unchanged after a decrease
- existing cursor/no snapshot migration returns the surface-specific
  missing-snapshot behavior for snapshot-backed surfaces, not zero provider truth
- snapshot result APIs return typed missing/blocked state with no numeric value,
  so absent snapshots cannot render as `0`
- `pending` display copy maps to typed `missing` with reason `not_polled`
- LA provider-day parsing around UTC/LA midnight boundaries
- structured diagnostics include previous/current totals and bucket details
  without raw source ids
- agentsview and ccusage paths both write provider snapshots
- agentsview cutover supersedes or excludes old ccusage canonical snapshots and
  leaves exactly one canonical-visible collector
- collector identity changes with stable canonical identity do not create false
  corrections or new feed
- model remap with unchanged source-day aggregate creates no downward
  user-facing source correction
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
