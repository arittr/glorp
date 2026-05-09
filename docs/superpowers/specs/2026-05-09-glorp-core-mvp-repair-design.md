# Glorp Core MVP Repair Design

Date: 2026-05-09

## Overview

This spec repairs the existing Rust Glorp MVP so the live product matches the
approved story contract and the Tokenpet terminal design direction. The scope is
the core pet loop and `glorp watch` experience: usage ingestion, effective-token
math, calibration, runtime pet updates, diagnostics, and terminal-native
rendering.

Packaging and release work are intentionally out of scope. Story 010 remains a
separate follow-up. This pass should make the currently implemented stories
truthful in the running product before distribution is revisited.

## Problem Statement

The current codebase has a green test suite and a broad implementation of the
stories, but the live product still feels unfinished and sometimes tells
contradictory truths. The screenshot shows the core symptom: usage events exist
in the log, but the current activity bucket can still be `0`; the pet surface is
present but sparse; helper diagnostics are visible only as terse event lines;
and renderer animation/color support exists without being fully reflected in
watch mode.

The fix should not be a separate styling pass layered on top of questionable
data. It should be a vertical story-truth repair: a newly observed real usage
delta should flow coherently through provider diffing, configured effective
tokens, pet state, storage, view model, and terminal rendering.

## Goals

- Make `glorp watch` accurately reflect newly observed usage across today
  totals, current bucket, source breakdown, recent log, vitals, XP, mood, and
  evolution state.
- Apply configured effective-token weights to real provider ingestion, not only
  isolated formula tests.
- Make calibration and catch-up behavior preserve the intended 6-8 active week
  companion arc for both heavy and light users.
- Keep partial helper failures source-local. If one source feeds and another is
  missing, Glorp is alive and fed while clearly explaining the missing source.
- Make the watch TUI pet-first, animated, colored, compact-aware, and faithful
  to the Tokenpet terminal style using only Unicode/ASCII and terminal colors.
- Preserve privacy and the "real usage is the only food" product boundary.
- Tighten tests so story completeness is proven by behavior, not only by
  presence of modules or style anchors.

## Non-Goals

- Do not implement npm release, cross-platform packaging, or published install
  verification in this spec.
- Do not add spritesheets, pixel-art image assets, generated bitmap assets, or a
  browser/web visual system.
- Do not add fake/manual feeding, `p` petting, treats, ship mechanics, death,
  revive, graveyard, litter picker, tweak panel, stage override, or species
  override.
- Do not add a daemon or a native Claude/Codex log parser.
- Do not turn Glorp into a token dashboard, prompt/session browser, cost
  optimizer, or billing source of truth.

## Core Contract

The repaired core should make one story-level promise:

> A newly observed provider delta is one coherent pet event.

That event should be idempotently detected, weighted according to current
configuration, applied once to pet state, stored without raw transcript content,
and rendered as the same truth in watch mode. The TUI should not show a feed log
without corresponding activity, or a globally blocked state when one provider is
healthy.

The implementation should introduce explicit event-time fields to make this
truth durable and testable. The important product distinction is:

- `period_start`: the coarse source period from `ccusage` or `@ccusage/codex`.
- `observed_at`: the time Glorp observed a new delta and treated it as food.
- `bucket_at`: the start of the 10-minute display/metabolism bucket derived from
  `observed_at` or from a deliberate catch-up smear.

Daily `ccusage` rows can keep their original source period for history and
privacy-preserving aggregation, but the current watch bucket should be based on
when Glorp observed new food, not on midnight from a date-only daily row.

The watch product surface uses pet-food time, not source-period time:

- Watch `today`, current bucket, source breakdown, and recent feed log use
  `bucket_at`/`observed_at`.
- Historical source aggregation and parser reconciliation can keep using
  `period_start` and `period_date`.
- Log rows render chronologically with the newest visible event nearest the
  bottom of the log area.

Persistence should migrate old local SQLite databases best-effort. Existing
usage rows that lack `observed_at` or `bucket_at` should receive conservative
values derived from `period_start`; they should not be replayed as new food.

The write boundary matters. Provider polling must not permanently advance
food/cursor state in a way that can lose pet progress if saving pet state fails.
The default implementation shape should be a durable, idempotent usage-delta
ledger:

- persist the provider delta as an unapplied ledger row;
- apply unapplied ledger rows to pet state;
- save pet state;
- mark rows applied and advance/reconcile cursor state only after the pet-state
  save succeeds.

An alternate transaction boundary is acceptable only if the pet state and
provider cursor state are actually in the same durable unit, or if the
implementation has an explicit recovery path. Do not treat a SQLite write plus a
separate JSON file save as atomic by convention.

The plan should choose the smallest approach that fits the existing Rust/SQLite
structure, but it must prove that a simulated state-save failure does not cause
food to disappear forever.

## Usage Provider And Effective Tokens

`CcusageCommandProvider` remains the MVP provider. It should continue shelling
out to `ccusage` and `@ccusage/codex`; Glorp should not parse raw local logs in
this pass.

Provider diffing should be stable across helper version changes. Parser and
provider versions should remain stored as metadata, but unchanged totals should
not be counted as new food merely because a helper version changed. Cursor keys
should identify the logical provider record being diffed: provider surface,
source command/surface, source period, and model when present. Version metadata
can be updated independently.

Effective-token calculation should use the loaded app config. If
`cache_read_weight` is set in `config.toml`, provider ingestion, status, watch,
and runtime pet updates should all use that value. Stored `effective_tokens`
are event-time values computed with the config in effect when the delta is
observed; changing config later does not silently rewrite old pet food history.
Cost remains display-only and must not affect food, XP, mood, or evolution.

Provider diagnostics should be structured and source-specific. The watch view
model should expose source health rows shaped around this information, not only
a flattened helper status string:

```text
source name
ready | diagnostic | blocked
today/bucket effective tokens
optional diagnostic code and actionable message
```

Required diagnostic behavior:

- `claude-code` healthy + `codex` missing should produce Claude deltas plus a
  Codex diagnostic.
- All providers missing should produce a blocked-but-alive setup state.
- Helper failures, invalid JSON, missing node, and cursor corruption should be
  sanitized and safe to paste into an issue.

## Calibration, Runtime, And Catch-Up

Historical usage during `glorp init` calibrates the user baseline but must not
feed, evolve, or grant XP to the newly created pet. Historical events used for
calibration should not inflate pet lifetime food counters. If the storage layer
keeps long-term source usage counters, they must be clearly separate from pet
lifetime counters.

Calibration should group historical usage by active day before deriving the
baseline. Multiple provider/model rows on the same day should contribute to one
active day total, not distort the median or active-day count as separate days.
Grouping happens before recent-day limiting and median/percentile calculation.

Catch-up usage should be reconciled coarsely and smeared into display/metabolism
buckets. Opening Glorp after ordinary time away should not turn a normal active
day into one giant damped bucket that barely advances the pet. The smear does
not need to reconstruct exact 10-minute history; it only needs to preserve the
intended companion-scale arc and make the live TUI honest about newly observed
food.

Initial numeric acceptance for catch-up:

- A single newly observed delta up to one calibrated active day should be split
  across at least 6 and at most 12 ten-minute buckets.
- Each smeared bucket should receive no more than 25% of the calibrated active
  day baseline before burst dampening.
- One calibrated active day delivered as catch-up should produce 0.75-1.25
  calibrated XP units after smearing.
- A 49-active-day simulation at the calibrated daily baseline, delivered as
  daily catch-up rather than minute-by-minute watch polls, should reach stage
  S6 with total XP in the 49-55 range.
- Catch-up smearing must not create duplicate stage-transition events.

If a source delta is smeared into multiple pet-food buckets, the provider delta
remains one cursor/reconciliation unit. The pet-food buckets may be separate
ledger rows or structured child rows, but they must reference the original
provider delta and must not duplicate source totals.

Evolution transitions should still be recorded exactly once. When a transition
is newly observed by watch mode, the terminal should show a simple live
evolution moment and then settle into the new stage art. A transition is
"newly observed" by watch when it appears in the latest successfully applied pet
state and has not yet been acknowledged by the running watch app. The moment can
be transient in memory for MVP; it does not need a new persistent
acknowledgement field unless that is the smallest reliable implementation.

## Watch TUI Behavior

`glorp watch` is the primary product surface. It should stay terminal-native and
pet-first.

Required behavior:

- The app redraw loop should advance animation frames independently of usage
  polling. Each redraw should derive or increment an animation tick used by
  `render_pet`; provider polling must not be required for pet-art changes.
- Manual refresh should poll immediately and should reset or debounce the next
  interval poll so users do not get a back-to-back helper call.
- Refresh/poll failures should update helper diagnostics without destroying the
  last known pet state.
- `?` should behave consistently with help copy: either toggle the help overlay
  or the copy should explicitly say `Esc` closes it. The preferred behavior is
  `?` toggles and `Esc` closes.
- Compact layout should request compact pet rendering instead of squeezing wide
  art into a small frame. The TUI must not cache only non-compact art; it should
  either render from pet identity/state on demand or carry explicit wide and
  compact render variants.
- Recent log ordering should show the newest relevant feed and diagnostic
  events.
- Mixed helper states should remain visible in the main surface. Actionable
  diagnostics should not disappear just because another source is healthy.
  `claude-code ready` plus `codex missing_helper` should be visible as two
  source-health rows without making the whole pet globally blocked.

The current key surface remains:

- `q`: quit.
- `r`: refresh usage and pet state.
- `?`: help.
- `Esc`: close overlay.

No `p` petting key is included in this spec.

## Terminal Design Requirements

The visual target is "alive terminal companion," not a dashboard and not a web
port. The Tokenpet mockup remains the style source, adapted to Ratatui and
Unicode/ASCII.

Required visual direction:

- Warm black background, dark surface chrome, parchment foreground, faint rules,
  amber accent, moss-green positive state, and coral diagnostic/error state.
- Pet-forward composition. In wide mode, the left panel should lead with the pet
  and stage presence, then identity metadata immediately under it, then vitals;
  it should not feel like a stats card with a small mascot attached.
- Dense but readable stat rows with labels, 20-cell bars, and values or
  percentages where space allows.
- Seeded pet color roles should reach the TUI. Body, eyes, mouth, accent, and
  pattern roles can be approximated with terminal colors, but should not be
  discarded into one foreground color. Existing `StyledSegment` roles from the
  renderer should become Ratatui spans or equivalent role-aware styled output.
- Source rows and event rows should use rails and semantic color to distinguish
  healthy usage, diagnostics, evolution, and normal messages.
- The right panel should make hierarchy clear: today/current bucket, 7-day
  activity, source health, then recent log.
- The footer should feel like a restrained terminal hint strip with key labels,
  not an unrelated status line.
- Compact mode should preserve required story content without overlap:
  pet/name/stage/mood, vitals, today/bucket, source health, and recent errors.
- The TUI should avoid prototype-only web features such as glows, blur, cards,
  buttons, sprite animations, and fake controls unless they translate naturally
  into terminal redraws.

## Story Coverage

This spec repairs and tightens the parts of Stories 001-009 that affect the
vertical watch truth path. Status, doctor, init, reset, and rename should change
only where they share provider diagnostics, storage correctness, or MVP
correctness with that path.

- Story 001: Provider deltas, diagnostics, helper discovery, and cursor diffing
  should remain real and idempotent, including helper version changes.
- Story 002: Persistence should distinguish source history/calibration data
  from pet lifetime counters while preserving privacy.
- Story 003: Init/reset/rename behavior remains in scope only where core polish
  is clearly broken, such as rejecting empty rename values. No new pet picker.
- Story 004: Configured effective-token math must apply to real provider
  ingestion.
- Story 005: Calibration should be active-day based, catch-up should be smeared,
  and watch should surface new evolution moments.
- Story 006: Mood and decay remain recoverable and real-usage-driven. No
  petting, fake feeding, or death mechanics.
- Story 007: Watch mode should render truthful live activity, key behavior,
  helper states, compact layout, and terminal cleanup boundaries.
- Story 008: The renderer should be live in watch mode, including animation,
  mood expression, compact rendering, color roles, and evolution moments.
- Story 009: Status, doctor, watch, and errors should stay friendly, structured,
  source-local, and safe to paste.

Story 010 is explicitly excluded from implementation scope, though the build
report may be corrected if it currently overclaims core or packaging completion.

## Implementation Planning Boundaries

This spec should become two implementation plans rather than one large batch:

1. **Data Truth Pipeline:** provider diffing, config weights, event-time storage,
   write boundary, calibration grouping, catch-up smearing, mixed diagnostics,
   and build-report correction.
2. **Watch Presentation And Interaction:** live renderer integration,
   role-colored pet spans, pet-first layout, compact render variants, help
   behavior, refresh debounce, source health rows, log ordering, and evolution
   moments.

The second plan depends on the first plan's event-time and source-health model.
Packaging remains outside both plans.

## Testing Strategy

Tests should be written against the story contracts, not merely against file
presence.

Provider and runtime tests:

- A cache-read-heavy fixture with `cache_read_weight` set in `config.toml`
  should produce real provider deltas using that configured weight.
- Repeated polls with unchanged totals should not double-feed, even if provider
  or parser version metadata changes.
- A provider poll followed by a simulated pet-state save failure should not
  permanently lose food after the next successful reconciliation.
- A `claude-code` healthy plus `codex` missing fixture should feed from Claude
  and report Codex as a source-local diagnostic.
- Historical calibration with multiple rows on the same date should group those
  rows into one active-day total.
- Init with historical fixtures should calibrate without granting pet XP,
  stage progress, food, or pet lifetime counters.
- Catch-up simulation over active baseline days should progress at the intended
  companion pace instead of being crushed as one burst.
- SQLite migration tests should prove older databases without `observed_at` and
  `bucket_at` remain readable and do not replay old rows as new food.

Watch/TUI tests:

- A real provider-style date-only daily row observed during watch should appear
  in today's total, current bucket, source breakdown, log, and pet state.
- A recent-events ordering test with more than five events should render the
  newest relevant lines.
- Animation ticks should change rendered pet art without provider polling.
- Wide and compact buffer tests should verify pet-first composition, seeded
  color roles, denser stat rows, source rails, actionable diagnostics, and help
  overlay behavior.
- Evolution transition tests should prove a newly recorded transition produces a
  renderable live watch moment.
- Terminal cleanup should retain existing guard coverage; a PTY-level test is
  welcome if practical but not required to complete this spec.

Verification for implementation completion should include at least:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Packaging commands are not completion gates for this spec.

## TDD Task Matrix

The implementation plans should include concrete failing tests for at least
these seams:

| Test area | Fixture or setup | Expected failing behavior today | Final assertion |
| --- | --- | --- | --- |
| Event-time storage | Date-only daily row observed at fixed `now` | Bucket reads `0` because `period_start` is midnight | Watch today/current bucket/source/log use `bucket_at` |
| Write boundary | Provider delta plus injected state-save failure | Cursor/event can advance while pet misses food | Next successful run applies the unapplied food once |
| Configured weights | Cache-read-heavy fixture and `cache_read_weight = 0.05` | Provider uses default `0.03` | Stored delta uses configured weight |
| Version-stable cursor | Same totals, changed helper version | New version can look like new food | No new delta; metadata updates safely |
| Calibration grouping | Same date, multiple model/source rows | Rows can count as separate active days | One active-day total enters baseline |
| Catch-up smear | Fixed baseline and one-day catch-up delta | Delta is damped as one giant bucket | 6-12 buckets and 0.75-1.25 calibrated XP units |
| Mixed diagnostics | Claude fixture healthy, Codex helper missing | Global helper state can read blocked or hide detail | Claude feeds; Codex diagnostic remains visible |
| Live animation | Watch redraws without poll | Art stays static | `render_pet` tick changes displayed art |
| Compact rendering | Narrow terminal | Wide art is squeezed/cropped accidentally | Compact render variant is used intentionally |
| Evolution moment | Stage transition during watch | `latest_evolution` is ignored | A transient terminal evolution moment renders once |

## Acceptance Criteria

- `cargo test` includes failing-before/failing-after coverage for the usage
  bucket, configured weight, calibration grouping, catch-up, mixed diagnostics,
  live animation, and TUI render contracts described above.
- In `glorp watch`, real observed usage no longer produces a feed log with a
  contradictory zero current bucket.
- The watch surface remains alive and accurate when one provider is feeding and
  another provider is missing.
- The pet visibly animates in watch mode without additional provider polls.
- The watch layout reads as a terminal companion: pet-first, colored, dense, and
  faithful to Tokenpet's restrained terminal style.
- No prototype-only controls or sprite/image assets are introduced.
- Story 010 packaging/release gaps are not solved in this pass and are not used
  as blockers for core MVP completion.
