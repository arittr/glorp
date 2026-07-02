# Glorp Calibrated Evolution Curve - design

- Date: 2026-07-02
- Status: draft for review
- Builds on:
  - `docs/superpowers/specs/2026-05-08-glorp-design.md`
  - `docs/superpowers/specs/2026-06-19-glorp-tokenmaxxing-token-contract-design.md`

## Problem

Glorp promises that evolution is calibrated to the user's recent pace: a heavy
Tokenmaxxing user and a light Tokenmaxxing user should experience roughly the
same companion arc when they work at their own normal pace.

The current curve can violate that promise on first run. Drew's fresh pet reached
the first evolution in about five minutes, even though the product-facing arc
describes S0 -> S1 as about one active hour. The problem is not that the first
stage should be time-locked. The problem is that the calibration denominator can
be wrong for a newborn pet, so a cache-heavy first-run delta can look like much
more than one active-hour equivalent.

Two current behaviors make this possible:

- `CalibrationBaseline::from_history` falls back to `100_000` tokens/day when
  fewer than five active days are available. That is safe for no-history users
  but wrong for sparse high-volume users.
- Evolution thresholds are expressed in XP units, but early thresholds are small
  (`0.04`, `0.25`, `1.0`). Under a too-small baseline, a few minutes of
  Tokenmaxxing `total_tokens` can satisfy the one-hour stage.

## Goals

- Preserve the product promise: stages advance from calibrated recent usage, not
  elapsed wall-clock time.
- Make the early curve correspond to cumulative active-work equivalents:
  - S0 -> S1: one calibrated active hour
  - S0 -> S2: six calibrated active hours
  - S0 -> S3: one calibrated active day
- Use the Tokenmaxxing token contract consistently for both XP gain and baseline
  calibration.
- Make first run a calibration and cursor-seeding event, never a feeding event.
- Prevent a reinitialized newborn pet from eating stale unapplied rows from an
  old pet.
- Keep the implementation focused on calibration, XP, first-run safety, and
  tests. Do not redesign watch, companion, habitat, or Tokenmaxxing collection.

## Non-goals

- No wall-clock gate for evolution. A stage should not wait for 60 real minutes
  if the user has genuinely done one calibrated active-hour equivalent of work.
- No fake feeding, manual stage controls, or admin override UI.
- No change to the canonical Tokenmaxxing `total_tokens` contract.
- No historical XP replay or "catch up the pet to old usage" migration.
- No broad reset of existing pets' XP or stages unless Drew explicitly asks for a
  separate repair/migration pass.

## Calibration Contract

Glorp's evolution unit is "one active day at your typical recent pace." The
baseline must therefore use the same token unit as XP gain:

```text
Tokenmaxxing total_tokens = input
                         + output
                         + cache_creation
                         + cache_read
```

Calibration groups normalized usage rows by Tokenmaxxing accounting day before
computing the baseline. In plain terms, all Claude/Codex/model rows for one
Tokenmaxxing day are summed into one daily total, and the recent active-day
totals feed the baseline.

All baseline paths must use `TOKENMAXXING_TOTAL_V1` rows only. Legacy weighted
token rows, helper diagnostics, and non-finite totals do not participate in
calibration, XP gain, progress rates, or first-run snapshots.

The baseline rules are deterministic:

- Drop non-finite totals and totals less than or equal to zero.
- Group remaining rows by Tokenmaxxing accounting day.
- Sort active days by date and take the latest 30 active days.
- If the window has at least five active days, use the median active-day total.
- If the window has one to four active days, use the median observed active-day
  total as a provisional baseline.
- If the window has zero active days, use the default `100_000` baseline.
- On an existing pet, clamp each successful refresh candidate to
  `0.5x..2.0x` of the currently persisted baseline. A clean init has no previous
  baseline, so this refresh clamp does not apply to the first persisted value.
- Historical seeded rows may update calibration and rhythm, but never grant XP,
  lifetime tokens, vitals, habitat props, source activity effects, today-summary
  activity, or stage transitions.

The invariant is: a heavy user's first real delta is normalized against their
observed recent pace, not a tiny no-history default.

## Active-Hour Curve

Evolution remains usage-derived. No wall-clock gate is introduced.

The curve defines active-work equivalents from the recent baseline:

- `active_day_baseline`: rolling/provisional Tokenmaxxing total tokens for one
  active day.
- `active_hour_baseline`: `active_day_baseline / active_hours_per_day`.

For v1, `active_hours_per_day` is fixed at `8`. Do not derive a lower value from
usage history until threshold migration semantics are specified; otherwise six
active hours can accidentally collapse into one active day.

XP for the canonical apply window is linear up to one active day:

```text
B = active_day_baseline
H = 8
active_hour_baseline = B / H
xp_gain = total_tokens / B       when total_tokens <= B
```

Diminishing returns may apply only above one active-day baseline in the same
canonical apply window. Smearing, bucketing, or presentation ledgers must not
change lifecycle XP; they can affect display or animation only.

Stage thresholds are cumulative XP thresholds expressed in active-work units:

- S1: `1 / H` XP, reached by `B / H` tokens.
- S2: `6 / H` XP, reached by `6 * B / H` tokens.
- S3: `1.0` XP, reached by `B` tokens.
- Later stages remain day-scale and preserve the broad 6-8 active-week arc.

Acceptance fixtures should use these formulas directly. Starting from S0:

- `B / H` tokens reaches S1.
- `(B / H) * (5.0 / 60.0)` tokens remains S0.
- `6 * B / H` tokens reaches S2.
- `B` tokens reaches S3.

This is not a wall-clock gate. If the user genuinely produces `B / H` tokens in
five real minutes, S1 is allowed. The bug is when five minutes of ordinary recent
pace reaches S1 because the baseline or cursor seeding is wrong.

Backfill and catchup are not part of the live lifecycle curve. Historical,
late-discovered, or cursor-repair usage may be refused, clipped, or seeded
without feeding. Clipped excess must not grant XP, lifetime tokens, vitals,
habitat props, or stage transitions unless a separate migration spec explicitly
allows it.

## First-Run Safety

A clean `glorp init` must:

1. Read historical provider totals.
2. Build active-day and active-hour calibration from that history.
3. Seed the exact provider cursor keys that the next poll will use.
4. Save the newborn pet at S0 with zero XP, zero lifetime tokens, and no usage
   feed effects from history.

First-contact and history seeding are evaluated per
`(provider_surface, cursor_key)`, not merely per provider surface. The snapshot
path and the later poll path must use one shared cursor-key builder. For each
serialized `ProviderCursorKey`, the contract includes:

- `provider_surface`
- `token_contract`
- `command`
- `source_surface`
- raw `period_start`
- `model`
- `raw_source_id`

The cursor partition is `source_identity.provider_surface`, not a helper surface
such as `unified`.

The first `status` or `watch` after init must not feed unchanged history. If a
provider emits rows under a cursor shape that init did not seed, Glorp should
treat those rows as first-contact history and seed them without feeding. That
path is diagnostic/debuggable, but primary Claude/Codex fixtures for a clean init
must prove the immediate next poll emits zero deltas, zero unapplied rows, no XP
or lifetime change, no stage change, and no `source_first_contact` diagnostic.

Rows written only to seed calibration/history must be distinguishable from
feedable usage, either with an explicit origin such as `calibration_seed` or an
explicit `feedable = false` field. Pet effects, source activity, today summaries,
and lifecycle queries must exclude those rows unless a query is intentionally
documented as display-only historical context.

Confirmed reinitialization uses reset-style cache replacement:

1. Remove the existing usage cache and any unapplied rows from the previous pet.
2. Read a fresh provider snapshot.
3. Rebuild calibration from that snapshot.
4. Seed all current provider cursors from that snapshot as non-feedable history.
5. Save the newborn pet.

If cache replacement or cursor seeding fails, confirmed reinit must abort before
saving the newborn pet. Deleting stale rows while leaving stale cursor state
behind is forbidden.

The invariant is: no historical row, cursor-shape drift, or stale unapplied event
can become newborn XP.

## Error Handling And Diagnostics

Calibration failures should fail conservative but visible:

- If helpers are unavailable, keep the no-history default and surface the normal
  provider diagnostic. Do not invent a baseline from missing data.
- If history is partially malformed, advance safe cursors where possible to avoid
  bolus behavior, but do not feed malformed rows.
- If a malformed row cannot produce a safe cursor, record a diagnostic or
  tombstone for the raw provider segment. The first later valid key for the same
  provider/day/model/source segment must still be seeded as non-feedable history,
  not treated as new work merely because it became parseable later.
- If a clean first run falls into first-contact seeding for primary sources,
  record a diagnostic that names the provider surface and explains that history
  was seeded without feeding.
- Discontinuity/refusal diagnostics remain non-blocking for source health: they
  indicate a refused poll, not a broken provider.

## Acceptance Tests

The implementation should land with tests that prove the product contract:

- Baseline grouping drops non-finite and non-positive totals, groups by
  Tokenmaxxing accounting day, sorts by date, and uses the latest 30 active
  days.
- With `n >= 5`, the baseline is the median of the latest active-day totals.
- With `1 <= n < 5`, the provisional baseline is the median of the observed
  active-day totals, not the `100_000` default.
- With `n == 0`, the baseline is `100_000`.
- Baseline refresh on an existing pet clamps each candidate to `0.5x..2.0x` of
  the persisted baseline.
- Every baseline, active-hour, XP, progress-rate, first-run snapshot, and
  lifecycle test uses `TOKENMAXXING_TOTAL_V1`.
- With `B = active_day_baseline` and `H = 8`, `B / H` tokens from S0 reaches S1.
- With `B = active_day_baseline` and `H = 8`, `(B / H) * (5.0 / 60.0)` tokens
  from S0 remains S0.
- With `B = active_day_baseline` and `H = 8`, `6 * B / H` tokens from S0 reaches
  S2.
- With `B = active_day_baseline` and `H = 8`, `B` tokens from S0 reaches S3.
- Aggregating one canonical apply window and splitting that same window into
  smear/display buckets produce identical lifecycle XP.
- Above one active-day baseline in a single canonical apply window, any
  diminishing-return formula is covered by explicit tests.
- Clean `init -> status` with unchanged primary Claude/Codex fixture history
  emits zero deltas, zero unapplied rows, no XP change, no lifetime change, no
  stage change, and no `source_first_contact` diagnostic.
- Init snapshot and poll paths serialize byte-identical `ProviderCursorKey`
  values for the same primary Claude/Codex fixture rows.
- Seeded historical rows are explicitly non-feedable and are excluded from
  lifecycle, pet-effect, source-activity, and today-summary activity queries.
- Confirmed `init --yes` reset-style cache replacement cannot apply pre-existing
  unapplied rows or stale cursors to the new pet.
- Backfill/catchup/clipped historical rows cannot grant XP, lifetime tokens,
  vitals, habitat props, or stage transitions.
- Existing no-history users still get a usable default curve.

## User-Facing Documentation

README wording should stay close to the current product promise, but it should
make the calibration unit clearer:

- Glorp compares new Tokenmaxxing `total_tokens` against your recent active-day
  baseline.
- Early stages are active-hour equivalents, not real-time locks.
- Historical usage calibrates a newborn pet but does not feed it.

The docs should avoid implying that a real clock blocks evolution. The pet grows
from work; calibration decides how much work counts as one active hour or day for
that user.
