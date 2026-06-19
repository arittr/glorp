# Glorp Tokenmaxxing Token Contract - design

- Date: 2026-06-19
- Status: direction approved by Drew; staff review revisions applied
- Linear: PRI-2283
- Builds on:
  - `docs/superpowers/specs/2026-05-09-glorp-core-mvp-repair-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-activity-identity-design.md`
  - `docs/superpowers/stories/story-001-usage-provider-ccusage.md`

## Problem

Glorp currently has two token truths:

- visible status and pet progression use stored `effective_tokens`
- `effective_tokens` discounts cache reads through `cache_read_weight`, which
  defaults to `0.03`

That made sense for an early "pet food" normalization model, but it no longer
matches the external product truth Drew cares about. Tokenmaxxing and
`tkmx-client` count cached input fully:

```text
total_tokens = input_tokens
             + output_tokens
             + cache_creation_tokens
             + cache_read_tokens
```

The mismatch was visible on 2026-06-18:

- Tokenmaxxing reported `715,380,912` total tokens for Drew.
- Glorp's current weighted effective total for the same broad day was around
  `103M`.
- Glorp's full-cache ledger total was higher than Tokenmaxxing because Glorp's
  Codex path still uses `ccusage-codex`, while Tokenmaxxing uses `agentsview`.

Drew's decision is that Glorp should progress on cached tokens too. The
existing per-user calibration should absorb heavy and light usage differences.

## Direction

Adopt the Tokenmaxxing token contract as Glorp's canonical usage unit.

From this point forward, when Glorp says "tokens" in status, watch, source
breakdowns, pet feeding, XP, calibration, day summaries, and activity traits,
it means Tokenmaxxing-style `total_tokens`.

Weighted effective-token math should stop being the product's default
progression unit. If retained at all, it should be renamed or isolated as a
legacy/internal diagnostic so it cannot quietly keep driving the pet.

## Goals

- Make Glorp's visible token totals match Tokenmaxxing semantics.
- Make pet food, XP, and evolution progression use full cached-token totals.
- Keep the existing calibration model as the normalization mechanism for users
  with very different token volumes.
- Prefer `agentsview` for Claude and Codex collection so local Glorp rows agree
  with `tkmx-client` and Tokenmaxxing.
- Preserve source identity and privacy boundaries from Activity Identity.
- Keep old local state stable: migration must not surprise-feed historical
  data or reset the user's pet.
- Make names and tests clear enough that future code does not confuse
  Tokenmaxxing totals with weighted effective tokens.

## Non-goals

- No direct dependency on the Tokenmaxxing API for normal local Glorp behavior.
- No network requirement for Glorp usage ingestion.
- No leaderboard, ranking, cost dashboard, or Tokenmaxxing profile UI inside
  Glorp.
- No replay of historical usage as new pet food during migration.
- No broad redesign of watch, menubar, companion, or preview rendering.
- No native raw transcript parser in this pass.

## Product Contract

The repaired contract is:

> One newly observed local usage delta produces one pet-food delta measured in
> Tokenmaxxing total tokens.

The token amount for each normalized provider/model row is:

```text
total_tokens = uncached_input
             + output
             + cache_creation
             + cache_read
```

For OpenAI/Codex-shaped rows where the source reports total input plus cached
input, normalization first splits:

```text
uncached_input = input_tokens - cached_input_tokens
cache_read = cached_input_tokens
```

Then it applies the same full-cache total formula above. Reasoning output is
preserved as shape metadata when available, but it is not added on top of
Tokenmaxxing `total_tokens` unless Tokenmaxxing changes its contract.

The important user-facing wording is:

- `tokens`: Tokenmaxxing total tokens
- `source`: where the tokens came from
- `cost`: display metadata only

Avoid new visible labels like "effective tokens" unless they are explicitly
marked as legacy or diagnostic. The pet is fed by tokens, not by a discounted
surrogate.

Canonical Tokenmaxxing-aligned source labels should match the external row
labels where possible:

- `claude`
- `codex`

Internal provider and cursor keys may retain more specific names during
migration, such as `claude-code`, but visible Tokenmaxxing-compatible stats and
comparison fixtures should use the external labels. If an internal key differs
from the external source label, tests must cover both identities so cursor
stability and user-facing parity do not drift apart.

## Collector Contract

`agentsview` should become the required provider for Tokenmaxxing-compatible
Claude and Codex accounting. This follows `tkmx-client` v1.3.0's collector and
avoids the `ccusage-codex` drift observed on Drew's 2026-06-18 Codex `gpt-5.5`
totals.

The provider commands are:

```bash
agentsview usage daily --json --breakdown --agent claude --since YYYY-MM-DD --timezone America/Los_Angeles
agentsview usage daily --json --breakdown --agent codex --since YYYY-MM-DD --timezone America/Los_Angeles --no-sync
```

The explicit timezone flag is an intentional Glorp choice to align with the
Tokenmaxxing rendered profile's Los Angeles day axis. Current `tkmx-client`
source does not pass this flag, so implementation notes and tests must not
claim byte-for-byte command parity with current `tkmx-client`; the shared
contract is the normalized `totalTokens` semantics and Tokenmaxxing profile
day axis.

The first command may run without `--no-sync` so one sync pass refreshes the
local agentsview index. The Codex command can use `--no-sync` in the same poll
cycle to avoid redundant sync work.

The implementation should keep a small timeout and return structured
diagnostics for:

- missing `agentsview`
- non-zero exit
- invalid JSON
- missing `daily`
- malformed model breakdown rows
- timeout

`ccusage` may remain as a legacy diagnostics fallback during the transition,
but it must not silently feed canonical pet progression or claim
Tokenmaxxing-compatible totals. If `agentsview` is missing, default Glorp
behavior should be "Tokenmaxxing provider blocked" with actionable doctor
output, not automatic progression from incompatible `ccusage-codex` rows. Any
explicit future legacy mode must label its output as legacy/provider-derived and
exclude those rows from canonical Tokenmaxxing totals.

## Provider Cutover And Cursors

Switching an existing Glorp install from `ccusage`/`ccusage-codex` to
`agentsview` changes command names, provider versions, date formats, and cursor
keys. Existing `claude-code` or `codex` cursors are not sufficient to prove
that `agentsview` history has already been accounted for.

The migration must include an explicit provider-contract cutover:

1. Run an `agentsview` snapshot for the Tokenmaxxing day window.
2. Normalize all returned rows into the new Tokenmaxxing row shape.
3. Recompute and save calibration and rhythm from Tokenmaxxing totals.
4. Seed the exact `agentsview` cursor keys and values for every current
   provider/model/date row.
5. Mark the Tokenmaxxing provider contract as active.
6. Only then allow normal polling to emit feedable deltas.

This cutover must run even when a surface already has old `ccusage` cursors.
The first `agentsview` run for an existing pet should not change XP, lifetime
food, stage, vitals, or recent feed events. Only a later positive delta beyond
the seeded `agentsview` cursor may feed the pet.

Provider cursor keys should include enough identity to distinguish:

- provider contract, for example `tokenmaxxing_total_v1`
- external source label, for example `claude` or `codex`
- command/source surface, for example `agentsview daily`
- Tokenmaxxing accounting date
- model name when present

## Day Axis

Tokenmaxxing's rendered profile uses an America/Los_Angeles daily chart axis,
and Drew's captured 2026-06-18 public API totals matched local `agentsview`
output when queried with `--timezone America/Los_Angeles` at review time.

Glorp should make the provider day axis explicit instead of relying on the
environment. The first implementation should use `America/Los_Angeles` for
Tokenmaxxing compatibility, with the timezone represented as configuration or
a narrow provider constant so it can be changed later without touching parsing
logic.

Watch/status "today" token totals should use the explicit Tokenmaxxing day
axis in this pass. The existing local-day mapper may remain for ambiance,
sleep, and life-context presentation, but token accounting should not split
from Tokenmaxxing-compatible dates.

Date-only `agentsview` periods must be interpreted as dates on the
Tokenmaxxing accounting axis, not as UTC-midnight instants. A provider date
`2026-06-18` means the America/Los_Angeles day that starts at local midnight on
2026-06-18. Stored rows may keep a date string and/or a UTC boundary instant,
but queries must preserve the Los Angeles accounting date. Tests must include
non-Los-Angeles `TZ` runs and midnight/DST boundary fixtures so status/watch do
not accidentally return to `LocalDayMapper::System` for token accounting.

## Storage And Naming

The current storage schema has columns named `effective_tokens`. That name is
now misleading for the desired product contract. The more serious risk is not
the name itself, but mixing old weighted rows with new Tokenmaxxing rows in the
same aggregate.

Implementation should choose the smallest safe migration that makes intent
clear. Acceptable shapes:

1. Add `total_tokens` columns alongside existing `effective_tokens`, then move
   new reads to `total_tokens`.
2. Rename in Rust types first while leaving the SQLite column as a legacy
   backing field for one release, with comments and tests preventing confusion.
3. Perform a full SQLite migration if the implementation can keep old databases
   safe and simple.

The chosen plan must satisfy these rules:

- Every row that participates in canonical token accounting has an explicit
  token contract such as `tokenmaxxing_total_v1`.
- Legacy weighted rows remain identifiable as a separate contract such as
  `weighted_effective_v1`, or are excluded by an explicit cutover epoch.
- New provider deltas store Tokenmaxxing totals as the pet-food amount.
- Calibration history uses Tokenmaxxing totals.
- Watch/status Today and source breakdowns use Tokenmaxxing totals.
- Cost remains separate.
- Reasoning output remains shape metadata, not an additive total-token field.
- Existing historical rows are not replayed as new food.

All canonical watch/status/calibration/lifetime queries must filter to the
Tokenmaxxing contract once the cutover is active. Old weighted rows may remain
available for diagnostics or legacy history, but they must not be summed into
new canonical totals.

Pet-state fields such as `lifetime_effective_tokens` should be renamed or
clearly migrated in the implementation plan. If a direct rename is too risky in
one pass, the compatibility layer must be explicit and temporary.

## Calibration And Progression

Pet progression should continue to be user-relative:

- A heavy Tokenmaxxing user and a light Tokenmaxxing user should evolve on a
  similar wall-clock arc when each is active at their own normal pace.
- Cache-heavy work should count as real work because Tokenmaxxing counts it and
  Drew wants pet progression to reflect it.
- The baseline should be recalculated from historical Tokenmaxxing totals, not
  from the old discounted effective values.

The migration order matters because the discontinuity guard currently depends
on persisted calibration. The implementation must save a full-cache
Tokenmaxxing baseline before enabling normal `agentsview` polling. Otherwise a
normal Tokenmaxxing-scale day could be compared against an old discounted
baseline and refused as implausible.

The existing catch-up smearing, discontinuity guard, activity intensity, and
stage thresholds should be reviewed under the new units. The likely
implementation should keep the same conceptual rules but update fixtures and
threshold expectations around full-cache totals.

The discontinuity guard deserves special attention. Full cached-token days can
be much larger than old effective-token days, so guard thresholds must compare
against a full-cache calibrated baseline. Do not simply keep an old absolute
threshold if it creates false refusals for normal Tokenmaxxing-scale activity.

## Migration

Migration should be boring and conservative:

- Load existing pet state without resetting the pet.
- Preserve stage, XP, vitals, name, species, recent events, and habitat state.
- Recompute or refresh calibration from `agentsview` Tokenmaxxing history when
  available.
- Seed exact provider-contract cursors from current `agentsview` totals so old
  history does not feed as new activity.
- Do not subtract old overcounted `ccusage-codex` rows from pet XP during this
  pass unless Drew explicitly asks for a historical correction.

If the existing usage database has old weighted rows, they may remain as legacy
history. New code should not let them corrupt new Tokenmaxxing totals. A
one-time diagnostic explaining that the token contract changed is acceptable if
it helps trust.

Required migration fixture:

- existing pet state with nonzero XP, lifetime, stage, and recent events
- old weighted `usage_events`
- old `ccusage`/`ccusage-codex` provider cursors
- current `agentsview` Tokenmaxxing rows

The first run after migration must save Tokenmaxxing calibration, seed
`agentsview` cursors, and leave XP/lifetime/stage unchanged. A second run with a
positive `agentsview` delta beyond the seeded cursor must feed exactly that new
delta.

## Packaging And Resolution

Packaged Glorp must not advertise Tokenmaxxing-compatible behavior unless it can
resolve `agentsview`.

Resolution order:

1. `GLORP_AGENTSVIEW_BIN`
2. `agentsview` on `PATH`
3. packaged/bundled helper path, if the release includes one

The first implementation may choose a hard external dependency instead of
bundling `agentsview`, but then README and npm README must say so plainly and
`glorp doctor` must report the install command or path override. The npm
wrapper should pass `GLORP_AGENTSVIEW_BIN` when it can resolve a packaged
helper, matching the existing `GLORP_CCUSAGE_BIN` pattern.

Doctor/status should capture and display:

- `agentsview --version`
- whether the provider is Tokenmaxxing-compatible
- whether Glorp is blocked because `agentsview` is missing
- whether any legacy `ccusage` fallback data is present

Release checks must include packaged-install smoke coverage where the shell
`PATH` does not happen to contain the developer's local `agentsview`.

## Privacy

Privacy boundaries for Glorp's own storage do not change.

Glorp may store:

- dates and bucket timestamps
- provider/source labels
- model names
- numeric token buckets
- cost metadata when provided
- parser/provider versions
- sanitized diagnostics

Glorp must not store:

- prompts
- responses
- tool-call payloads
- file paths
- project names
- transcript content

`agentsview` is a local indexer and may create or update its own local database
when Glorp invokes a syncing usage query. The implementation must document this
external side effect and keep it outside Glorp's own usage database contract.
By default, Glorp should let `agentsview` use its normal data directory rather
than placing a full-text session index under Glorp's config directory. If a
future implementation uses an isolated `AGENT_VIEWER_DATA_DIR`, the privacy
section and doctor output must say exactly where that index lives and why.

Glorp should consume only the daily numeric usage JSON needed for this feature.
Persisted diagnostics must sanitize `agentsview` stderr and must not store
session paths or transcript snippets.

## Testing

Required coverage:

- Token math counts cache reads fully.
- Codex/OpenAI-shaped rows split cached input into `uncached_input` and
  `cache_read`, then compute full Tokenmaxxing `total_tokens`.
- Reasoning output is preserved but not added to total tokens.
- Agentsview daily JSON with Claude and Codex model breakdowns normalizes into
  the same row shape as `tkmx-client`: every missing counter defaults to zero,
  per-model totals are computed once as input + output + cache creation + cache
  read, costs remain metadata, and source labels are preserved.
- Checked-in raw `agentsview usage daily --json --breakdown` fixtures cover
  Claude, Codex, per-model rows, omitted zero fields, costs, source labels, and
  malformed rows.
- A captured Tokenmaxxing API/server regression fixture for Drew's 2026-06-18
  totals:
  - Claude: `46,011,892`
  - Codex: `669,369,020`
  - Total: `715,380,912`
- A separate live-local-collector fixture asserts Glorp matches current local
  `agentsview` semantics even when the public Tokenmaxxing profile has not been
  refreshed to the same totals yet.
- Status/watch source totals use Tokenmaxxing totals rather than discounted
  values.
- Calibration derives the baseline from full-cache totals.
- A first-contact or migration path seeds cursors without granting historical
  XP.
- A provider total decrease does not double-count or silently corrupt future
  deltas.
- Status/watch tests run with a non-Los-Angeles `TZ` and assert token
  accounting still uses the Tokenmaxxing Los Angeles day axis.
- Tests assert the exact `agentsview ... --timezone America/Los_Angeles` argv.
- Packaging tests verify `GLORP_AGENTSVIEW_BIN`, `PATH`, and missing-helper
  diagnostics.
- Privacy tests verify Glorp persists only numeric usage rows and sanitized
  diagnostics from `agentsview`.
- README, npm README, and story docs are updated or explicitly marked legacy
  anywhere they mention `effective tokens`, `cache_read_weight`, or
  `ccusage-codex` as the normal provider path.

Useful checks after implementation:

```bash
cargo test usage_provider
cargo test runtime_integration
cargo test watch_integration
cargo test --test acceptance_matrix
cargo run -- status
```

If Preview Lab output changes because Today/source text changes, also run:

```bash
cargo run -- dev-preview --scenario watch --out target/glorp-preview
cargo test --features dev-preview --test dev_preview
```

## Implementation Decisions For Planning

- `agentsview` is required for canonical Tokenmaxxing-compatible totals.
  `ccusage` may remain as a legacy diagnostic provider for one release, but its
  rows must not feed canonical progression or visible Tokenmaxxing totals.
- `cache_read_weight` is deprecated for pet progression immediately. It may
  remain accepted in `config.toml` for compatibility, but new progression and
  visible stats must ignore it.
- Watch/status "today" token totals should move to the Tokenmaxxing day axis in
  the same implementation pass. Local-day mapping can remain for ambiance,
  sleep, and life-context presentation if those systems need it.
- The first implementation should avoid a broad all-at-once rename. Add a
  canonical `total_tokens` path for provider deltas, storage reads, calibration,
  and UI totals, then leave any deeper `effective_tokens` field cleanup as a
  follow-up if it would create unrelated churn.
