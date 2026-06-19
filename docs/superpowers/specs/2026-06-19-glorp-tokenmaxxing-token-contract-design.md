# Glorp Tokenmaxxing Token Contract - design

- Date: 2026-06-19
- Status: direction approved by Drew; spec pending review
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

## Collector Contract

`agentsview` should become the preferred local provider for Claude and Codex.
This matches `tkmx-client` v1.3.0 and avoids the `ccusage-codex` drift observed
on Drew's 2026-06-18 Codex `gpt-5.5` totals.

The preferred commands are:

```bash
agentsview usage daily --json --breakdown --agent claude --since YYYY-MM-DD --timezone America/Los_Angeles
agentsview usage daily --json --breakdown --agent codex --since YYYY-MM-DD --timezone America/Los_Angeles --no-sync
```

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

`ccusage` may remain as fallback during the transition, but the status and
doctor surfaces must make the provider clear. A fallback path can be useful for
users without `agentsview`, but it is not Tokenmaxxing-compatible enough to be
the preferred source for Drew's stats.

## Day Axis

Tokenmaxxing's rendered profile uses an America/Los_Angeles daily chart axis,
and Drew's 2026-06-18 public API totals matched local `agentsview` output when
queried with `--timezone America/Los_Angeles`.

Glorp should make the provider day axis explicit instead of relying on the
environment. The first implementation should use `America/Los_Angeles` for
Tokenmaxxing compatibility, with the timezone represented as configuration or
a narrow provider constant so it can be changed later without touching parsing
logic.

Watch/status "today" token totals should use the explicit Tokenmaxxing day
axis in this pass. The existing local-day mapper may remain for ambiance,
sleep, and life-context presentation, but token accounting should not split
from Tokenmaxxing-compatible dates.

## Storage And Naming

The current storage schema has columns named `effective_tokens`. That name is
now misleading for the desired product contract.

Implementation should choose the smallest safe migration that makes intent
clear. Acceptable shapes:

1. Add `total_tokens` columns alongside existing `effective_tokens`, then move
   new reads to `total_tokens`.
2. Rename in Rust types first while leaving the SQLite column as a legacy
   backing field for one release, with comments and tests preventing confusion.
3. Perform a full SQLite migration if the implementation can keep old databases
   safe and simple.

The chosen plan must satisfy these rules:

- New provider deltas store Tokenmaxxing totals as the pet-food amount.
- Calibration history uses Tokenmaxxing totals.
- Watch/status Today and source breakdowns use Tokenmaxxing totals.
- Cost remains separate.
- Reasoning output remains shape metadata, not an additive total-token field.
- Existing historical rows are not replayed as new food.

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
- Seed provider cursors from current `agentsview` totals so old history does
  not feed as new activity.
- Do not subtract old overcounted `ccusage-codex` rows from pet XP during this
  pass unless Drew explicitly asks for a historical correction.

If the existing usage database has old weighted rows, they may remain as legacy
history. New code should not let them corrupt new Tokenmaxxing totals. A
one-time diagnostic explaining that the token contract changed is acceptable if
it helps trust.

## Privacy

Privacy boundaries do not change.

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

`agentsview` is a local indexer, but Glorp should still consume only the daily
usage JSON needed for this feature.

## Testing

Required coverage:

- Token math counts cache reads fully.
- Codex/OpenAI-shaped rows split cached input into `uncached_input` and
  `cache_read`, then compute full Tokenmaxxing `total_tokens`.
- Reasoning output is preserved but not added to total tokens.
- Agentsview daily JSON with Claude and Codex model breakdowns normalizes into
  the same totals as `tkmx-client`.
- A Drew regression fixture for 2026-06-18 totals:
  - Claude: `46,011,892`
  - Codex: `669,369,020`
  - Total: `715,380,912`
- Status/watch source totals use Tokenmaxxing totals rather than discounted
  values.
- Calibration derives the baseline from full-cache totals.
- A first-contact or migration path seeds cursors without granting historical
  XP.
- A provider total decrease does not double-count or silently corrupt future
  deltas.

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

- `agentsview` is the preferred provider and the only provider that can claim
  Tokenmaxxing-compatible totals. For one release, `ccusage` may remain a
  fallback, but fallback output must be labeled as legacy/provider-derived and
  must not be presented as Tokenmaxxing-compatible.
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
