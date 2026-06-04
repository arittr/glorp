# Glorp liveliness — design

- **Date:** 2026-06-04
- **Status:** approved design, pre-implementation
- **Scope:** make the pet feel alive and responsive to real-time token use, especially at high stages

## Problem

The pet's always-on visuals encode only `(species, wall-clock now)`. Breath, wander, blink, shimmer, twinkle, and particles are pure functions of the clock (`src/pet/animator.rs`), so the pet looks identical during a 1M-token sprint and at 3am while idle. The only usage-driven channel is XP/stage, which is the slowest and least visible — and at high stages it barely moves (S5→S6 ≈ 46 days at calibrated pace). The result reads as static everywhere and doubly static at high stages.

Two real bugs compound this:

1. **`recent_tokens_per_min`** (`src/commands/watch.rs:250-257`) filters by `observed_at` over 60s. Every smeared ledger row from one poll shares `observed_at = now`, so it sums an entire delta as "per minute," then snaps to 0 — it flickers the munch speech on/off with each poll instead of tracking real activity.
2. **The progress bar** (`src/commands/watch.rs:185-191`) computes `fraction = state.xp / next_threshold` using *absolute* XP, not stage-relative. A pet entering S4 (xp=4.0, next=14.0) already shows ~29% and crawls to 100%. The `ProgressView` struct already documents these fields as stage-relative (`src/tui/view_model.rs:126-129`); only the population is wrong. A test (`src/commands/watch.rs`, the `0.607` assertion) currently locks in the buggy behavior.

A third, smaller inaccuracy: `rate_per_hour` is a plain 1-hour token sum (`src/commands/watch.rs:178-183`) but its doc comment claims a "6h-half-life EMA" (`src/tui/view_model.rs:130`).

## Principle

Add one real signal — live token activity — and let it drive multiple visual channels, **layered** so that:

- **Sustained** activity reads as ambient liveliness (the pet is awake and with you).
- **Bursts** read as punchy, proportional reactions (it noticed *that*).

Everything is real ledger data, deterministic given fixed inputs, and every response strength is a **named constant tuned live** in `cargo run -- watch`. We are not picking a single global intensity; we wire the architecture and dial each channel against real token flow afterward.

This stays inside two standing rules:

- **Real data only** — no fabricated content. Flavored text (thoughts/speech) is acceptable only as a *presentation layer over a real signal*: real activity decides *which* line shows.
- **Tamagotchi spirit** — a creature you nurture, not a meter you optimize. No ETAs, countdowns, or grind mechanics. Show magnitude for context, not time-to-goal.

## Goals

- The pet visibly responds, moment-to-moment, to live token usage — at every stage.
- High stages feel materially more alive than low stages and show day-to-day progress.
- The watch screen changes shape as you work.
- Fix the two real bugs (`recent_tokens_per_min`, progress fraction) and the stale doc comments.

## Non-goals / out of scope (clean follow-ons)

Pip narration + persistence, daily streak counter, habitat ladder past 25M lifetime tokens, new mood variants. Rationale recorded in discussion: narration adds persistence + spam for marginal gain; streaks are a grind mechanic against the spirit; the habitat gap only bites 25M+ lifetime users and is covered enough by stage-scaled richness + pips; a new mood is redundant with the non-persistent active-brightness state below.

## Architecture

One new signal feeds two layers and several consumers.

```
ledger (usage_events, bucket_at) ──┬─► activity_level (smoothed, ~30m window, baseline-normalized)
                                   │        └─► ambient channels: breath, particles, body brightness
                                   │        └─► thoughts/speech selection
                                   │        └─► (× stage tier) particle richness
                                   │
poll-over-poll today delta ────────┴─► burst layer: proportional feed-pulse + token-pop
```

`activity_level` is computed once per poll in `build_watch_view_model_at` and carried on `WatchViewModel`. The burst signal is the `today_effective_tokens` jump the animator already tracks (`src/pet/animator.rs:154-156`) — un-smeared, immediate.

## Components

### 1. `activity_level` signal (foundation)

New `activity_level: f32` field on `WatchViewModel`, computed in `build_watch_view_model_at` (`src/commands/watch.rs`):

- Sum effective tokens over a short trailing window (`ACTIVITY_WINDOW`, default ~30 min) by `bucket_at`, reusing `token_totals_by_source_between`.
- Normalize to the user's own pace: `expected = state.calibration.daily_effective_tokens × (ACTIVITY_WINDOW / 24h)`; `activity_level = (window_tokens / expected).clamp(0.0, ACTIVITY_CEILING)`.
- `0.0` = idle, `~1.0` = your average daily pace, `> 1.0` = a session that's hot for you.
- Pure function of `(window_tokens, baseline)` → deterministic. Fixtures and dev-preview pin a fixed value so snapshots stay stable.

### 2. Bug fix: `recent_tokens_per_min` (foundation)

Rebase from `observed_at` to `bucket_at` over a short window so it returns a real, smoothed rate that decays naturally instead of flickering with the poll. Consumed by speech (§6) and available to the activity signal.

### 3. Ambient channels ← `activity_level`

Each channel gets an independent, named tunable gain. Functions in `src/pet/animator.rs` currently pure on `(species, now)` gain an `activity_level` argument:

- **Breath** — faster cadence as activity rises; also converted from the current 0/1 hop (`compute_breath_offset`) to a multi-step ease so it reads as breathing. Larger amplitude requires reserving an extra row in `src/tui/panels/pet.rs` so it doesn't clip.
- **Particles** — denser / faster ring as activity rises (`particles_for_species` / `frame_with_particles` in `src/pet/render.rs`).
- **Body brightness** — gentle lightness lift while active, render-time only (non-persistent). Mirror of the existing `low_energy_lightness_multiplier` droop. Being non-persistent, it sidesteps the "vitals pinned at 100 → mood never changes on the upswing" problem without touching saved state.

### 4. Stage-scaled richness ← stage tier

Particle density/cadence (and optionally breath amplitude / twinkle frequency) scale with a per-stage tier, combined **multiplicatively** with `activity_level`, so an S6 elder is materially more animated than an S1 byte. Add a `stage` parameter to `particles_for_species` / `frame_with_particles`. The 11-char art body is untouched — all added motion lives in the particle ring and the overlay system. Mirrors the precedent in `docs/tokenpet/project/pet.jsx` where stage gates aura richness.

### 5. Proportional bursts ← poll-delta magnitude

The feed pulse + token-pop currently fire one identical effect above a fixed `FEED_EVENT_TOKEN_THRESHOLD` of +250 tokens (`src/pet/animator.rs:53,154-169`). Make them scale by magnitude (delta relative to the daily baseline): a nibble → faint quick blip; a big spike → longer/brighter sweep + longer `TokenPop`. Lower or tier the threshold so light real activity still registers. Requires plumbing the daily baseline onto the view model.

### 6. Thoughts/speech as a representation of activity

Keep the hand-authored species/mood vocabulary in `src/pet/speech.rs` and `src/pet/activity.rs`. Change the **selection driver**:

- High `activity_level` → working/munching-themed lines.
- Idle → quiet / mood lines.
- The munch trigger rebases onto the fixed rate from §2.
- Remove the pure wall-clock idle rotation (`now.unix_timestamp() / 60` in `idle_thought`) that is untethered from activity.

Text becomes a presentation layer over the real signal — no fabricated activity.

### 7. Progress legibility + intra-stage pips

In `src/commands/watch.rs` and `src/tui/panels/progress.rs`:

- **Fix the stage-relative bug:** add a `stage_start_xp(stage)` helper (alongside `next_stage_xp_target`); `xp_in_stage = state.xp − stage_start_xp`; `xp_to_next = next_threshold − stage_start_xp`; fraction over the span. The bar restarts at 0% each stage and moves ~2× faster. Replace the test that asserts the buggy absolute fraction.
- **Magnitude readout** beside the bar, e.g. `2.4 / 10 XP-days`. **No ETA** (tamagotchi spirit).
- **Always show pace** — drop the `> 0` hide so `↑ N/hr` / `idle` is always present and ticks the moment work starts.
- **Intra-stage pips:** a small sub-level row from the stage-relative fraction — `floor(fraction × PIP_COUNT)` of `PIP_COUNT` pips. **Display-only**: no new persistence, no narration. Gives long S5/S6 spans visible day-to-day movement without spamming at the fast low stages.
- Fix the two stale doc comments (`xp_in_stage` contract; the bogus "6h EMA" on `rate_per_hour`).

### 8. Intraday hourly sparkline

The only sparkline today is a 7-day daily history (`src/tui/panels/today.rs`), so within a day the screen has no shape. Add a today-by-hour sparkline so a working session visibly builds a rising curve in real time.

- New `UsageStore` query that buckets `usage_events.bucket_at` by hour for the current local day (following the `token_totals_by_source_between` / `seven_day_token_history` pattern).
- New `Vec<f64>` field on `WatchViewModel`, populated in `build_watch_view_model_at`.
- Render via the existing spark-line widget in `TodayPanel`; add a row and bump the panel's height constraint. Gate the extra row behind available height, as the panel already does in compact mode.
- Real data, deterministic; fixtures pin it.

### 9. Best-day milestone

When the stage bar barely moves for weeks, give the user a number that does move and a gentle, on-spirit celebration.

- Surface personal best day via the existing, currently-unused `best_day_effective_tokens()` (`src/storage/usage_store.rs`).
- New field on `WatchViewModel`; rendered in/near `BioCardPanel`.
- A **non-persistent** "new best day" highlight when today's running total exceeds the best of all *prior* days — display-only, no persistence, deterministic on inputs. If `best_day_effective_tokens()` includes the current day, exclude today from the comparison so the highlight reflects beating a previous record rather than comparing today against itself.

## Data flow

Poll → `build_watch_view_model_at` computes `activity_level`, the fixed rate, the corrected progress (with pips), the intraday hourly series, and best-day → these ride on `WatchViewModel` → the animator and panels consume `activity_level` for ambient channels and the poll delta for proportional bursts → render. All inputs are real numeric ledger data. The worker replaces the whole view model each poll (~10s); animation interpolates between polls on the wall clock as today.

## Testing

Test-driven, per project convention. Unit coverage for:

- `activity_level` normalization (idle → 0, baseline pace → ~1, burst → clamps at ceiling).
- The rebased `recent_tokens_per_min` (bucket_at based; no flicker; decays).
- `stage_start_xp` + corrected stage-relative fraction (replacing the test that locks in the bug); each stage starts at 0% and reaches 1.0 at its next threshold.
- Pip computation (`floor(fraction × PIP_COUNT)`, bounds).
- Proportional burst mapping (small vs large delta → different effect parameters; threshold floor).
- Activity-driven speech/thought selection (active → working-themed; idle → quiet; munch trigger off the fixed rate).
- Intraday hourly query + best-day surfacing.

Animation channel functions get deterministic tests at fixed `activity_level` values. dev-preview fixtures pin `activity_level` and the new series so contact-sheet snapshots stay stable. Test output must stay pristine.

## Tuning

Every response strength is a named constant: breath/particle/brightness response curves, stage tiers, burst magnitude curve, `PIP_COUNT`, `ACTIVITY_WINDOW`, `ACTIVITY_CEILING`. Wire conservative ambient-leaning defaults, then dial them live in `cargo run -- watch` against real token flow. The feel is deliberately deferred to live tuning rather than fixed in this spec.

## Suggested implementation staging

The plan will sequence roughly: (a) foundation — `activity_level` + the two bug fixes; (b) pet animation — ambient channels, stage scaling, proportional bursts; (c) thoughts/speech rewiring; (d) progress legibility + pips; (e) panels — intraday sparkline + best-day. Each lands with its tests before the next.
