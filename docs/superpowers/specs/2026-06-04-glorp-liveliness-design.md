# Glorp liveliness — design

- **Date:** 2026-06-04 (revised after staff-SWE design review)
- **Status:** approved design, pre-implementation
- **Scope:** make the pet feel alive and responsive to real-time token use, especially at high stages

## Problem

The pet's always-on visuals encode only `(species, wall-clock now)`. Breath, wander, blink, shimmer, twinkle, and particles are pure functions of the clock (`src/pet/animator.rs`), so the pet looks identical during a 1M-token sprint and at 3am while idle. The only usage-driven channel is XP/stage, the slowest and least visible — and at high stages it barely moves (S5→S6 ≈ 46 days at calibrated pace). The result reads as static everywhere and doubly static at high stages.

Two real bugs compound this:

1. **`recent_tokens_per_min`** (`src/commands/watch.rs:250-257`) filters by `observed_at` over 60s. Every smeared ledger row from one poll shares `observed_at = now`, so it sums an entire delta as "per minute," then snaps to 0 — it flickers the munch speech with each poll instead of tracking real activity.
2. **The progress bar** (`src/commands/watch.rs:185-191`) computes `fraction = state.xp / next_threshold` using *absolute* XP, not stage-relative. A pet entering S4 (xp=4.0, next=14.0) already shows ~29% and crawls. The `ProgressView` struct already documents these fields as stage-relative (`src/tui/view_model.rs:126-129`); only the population is wrong.

## What the design review changed

A three-lens staff-SWE review (correctness, architecture, product) red-teamed the first draft. The load-bearing finding: **the original `activity_level` definition did not survive the data model.** It read the *smeared* ledger (every poll delta is back-dated across 6–12 ten-minute buckets, capped per bucket at `daily×0.25`), and normalized against the daily *average* — so it under-read bursts, its capture fraction varied with burst size, and it saturated at the ceiling during any sustained session (no usable mid-range). The whole layered-feel premise needs the signal to *have* a middle.

The corrected approach, adopted here:

- **One canonical live-intensity signal** derived from the **raw applied poll delta** (`RuntimeUpdate.recent_effective_tokens`, `src/game/runtime.rs:25-26` — computed every poll but currently *discarded*: `poll_usage_and_apply` uses only `applied_event_ids` and returns `Ok(Some(state))`, `src/commands/watch.rs:344-350`), **EMA-smoothed across polls**, with a **graduated/saturating** normalization curve. The smeared `bucket_at` ledger is used **only** for historical aggregates (today total, hourly sparkline, 7-day, best-day).
- **Validate the normalization curve against a real ccusage ledger before wiring consumers.** The curve is derived from data, not deferred to live tuning.
- **Three sequential branches**, bug-fixes first, each independently revertible, so a regression in the contested signal cannot block verified fixes.

Approved calls from the review:

- **Keep the breath 0/1 hop** (pet.jsx is the declared source of truth — "port, don't invent"). Liveliness from cadence/amplitude, no multi-step ease.
- **Drop the menubar's `next_stage_label` reveal** so both frontends keep the next stage a surprise.

## Principles

- **Real data only** — flavored text (thoughts/speech) is acceptable only as a presentation layer where a *real* signal decides which line shows. During genuine idle, selection must key off a real attribute (mood, energy, time-since-real-activity), not the wall clock.
- **Tamagotchi spirit** — a creature you nurture, not a meter you optimize. No ETAs, countdowns, or grind mechanics. Show magnitude for context, not time-to-goal.
- **Renderer stays content-agnostic** — gameplay signals do not enter `render_pet`'s signature.

## Goals

- The pet responds, moment-to-moment, to live token usage — at every stage.
- High stages feel materially more alive than low stages and show day-to-day progress.
- The watch screen changes shape as you work.
- Fix the two real bugs and the stale doc comments/tests.

## The canonical live-intensity signal (foundation)

This is the prerequisite for everything in Branch 2 and the thoughts rework.

- **Seam:** make `poll_usage_and_apply` return the raw applied delta (`RuntimeUpdate` or `recent_effective_tokens`) alongside `PetState`, and plumb it to `build_watch_view_model_at`.
- **`activity_level: f32` on `WatchViewModel`:** an EMA, across the ~10s polls, of the raw per-poll delta normalized to an instantaneous pace, passed through a saturating-but-graduated curve (e.g. `x/(x+k)` or log) so 0.3 / 0.6 / 1.0 / 2.0 are visually distinct.
- **Normalization reference — decided by the prototyping step**, under two hard constraints: (a) it must have usable dynamic range during real sessions, and (b) it must not be broken during a pet's first ~5 days, when the calibration baseline is the default 100k and may be 10–100× off (the baseline is set once at init and never re-calibrates, `src/game/calibration.rs`). Candidate references to evaluate against a real ledger: a self-normalizing rolling EMA of the user's own recent window-rates (auto-calibrating, no init dependency) vs. the calibration baseline pace once real. Floor any divisor with `.max(1.0)` and guard the clamp against NaN.
- **Continuity:** `activity_level` is a per-poll scalar but animation ticks per-frame. It must drive **amplitude/density**, never an animation *period* — modulating a `now % period` expression causes a visible phase jump every poll (the artifact `compute_wander_position_x`'s smoothstep was built to avoid). The EMA gives a continuous, decaying value the panel re-derives each frame.
- **Override seam:** carry `activity_level` as a settable vm field (mirroring `wander_offset_x`) so dev-preview and unit tests pin it deterministically.
- **First task of Branch 2 is the prototype/measurement**, not code that depends on the final curve.

## Branch 1 — Correctness fixes (ships first, safe, independently revertible)

- **Stage-relative progress fraction:** add a `stage_start_xp(stage)` helper (alongside `next_stage_xp_target`); `xp_in_stage = state.xp − stage_start_xp`; `xp_to_next = next_threshold − stage_start_xp`; fraction over the span. Bar restarts at 0% each stage. Thresholds: `[0, 0.04, 0.25, 1.0, 4.0, 14.0, 60.0]`.
- **All real consumers of the fraction (the review's audit):**
  - Regenerate the golden snapshot `tests/snapshots/dev_preview__watch_wide_normal_frame.snap` (renders `61` → `45`; bar-fill glyphs change too).
  - Fix the `WatchViewModel` fixture (`src/tui/view_model.rs:235-243`): `fraction 0.61 → 0.45`, `xp_in_stage 8.5 → 4.5`, `xp_to_next 14.0 → 10.0`.
  - Replace the watch.rs progress test (it asserts `fraction ∈ (0.5,0.7)` at xp=8.5/S4; the corrected 0.45 fails the lower bound).
  - Update the **macOS menubar** percent (`src/menubar/render.rs:155`) to the stage-relative value, and **drop the `next_stage_label` reveal** there.
  - Reconcile or delete the vestigial absolute `xp_current` / `xp_target` (`src/commands/watch.rs:147-148`; the fixture's `42000` / `100000` are not XP-scale and unused by any panel).
- **`recent_tokens_per_min` flicker fix:** rebase off `observed_at` to a `bucket_at` window with an **explicit length ≥ ~20 min (two 10-min buckets)** so it is a real smoothed rate, not a 10-min step function, and reconcile the munch threshold so it can actually fire given the per-bucket cap (`daily×0.25` = 25k at default vs the current 30k/min). Branch 2 later re-points munch onto the canonical signal; this keeps it correct in the interim.
- **Doc/test honesty:** fix the `rate_per_hour` "6h-half-life EMA" doc comment (`src/tui/view_model.rs:130`; it is a plain 1h `bucket_at` sum, `src/commands/watch.rs:178-183`) and rename the misnamed `ema_rate_grows_with_more_recent_events` test. Do **not** touch the `xp_in_stage` doc comment — it already states the correct post-fix contract.

## Branch 2 — Live-activity signal + pet animation (the risky core)

Opens with the prototype/measurement step above; nothing downstream is built until the curve is validated.

- **Ambient channels** (amplitude/density only) driven by `activity_level`, each a named tunable gain:
  - **Breath** — faster cadence as activity rises (keep the 0/1 hop; no type change).
  - **Particles** — denser/faster ring. **Computed in the panel layer** via a new animator fn (e.g. `particle_intensity(stage, activity_level, tick)`) consumed where shimmer/twinkle/token-pop already overlay (`src/tui/panels/pet.rs`). **Not** threaded through `render_pet`. The panel reads `activity_level` from `self.vm` so per-frame density and tick-driven cadence stay consistent between polls.
  - **Body brightness** — gentle render-time lift, a mirror of `low_energy_lightness_multiplier`. **Specify the compose order and clamp** with the existing droop and shimmer multipliers so stacked lightness ops can't overflow into mush; decide whether to gate the lift when energy is low.
- **Stage-scaled richness:** thread `stage` into the particle path (`render_pet` already carries `stage`), combined multiplicatively with `activity_level`. Design the per-stage curve fresh and cite the real in-repo precedent, `stage_base_count` (`src/tui/panels/pet.rs:91-98`) — *not* pet.jsx, whose ParticleLayer is a binary on/off gate, not graduated richness.
- **Proportional burst:** drive from the **raw applied delta** (now plumbed onto the vm), scaled by magnitude relative to the signal's pace; replaces the fixed +250 `today_effective_tokens`-diff trigger. **Define the burst↔ambient relationship for one delta** (leading edge vs decay tail) so a single event reads as one coherent reaction, and **specify/test the local-midnight reset** (the old `today_effective_tokens` diff goes negative at the day boundary).
- **Thoughts as a representation of activity** — unify the **two** flavor systems onto the canonical signal: `speech.rs` (bubble) *and* `activity.rs` (feed: `recent_munch_spike` + `is_idle`, both currently `observed_at`-based and broken by the smear). Drive idle-line selection off real attributes (mood/energy/idle-duration), not the wall clock. Do not claim "no fabricated activity" — the honest framing is "real signal selects the presentation."
- **Menubar:** thread `activity_level` through the shared vm + `rerender_pet_for_view_model` path so the second frontend stays consistent (it builds the same `WatchViewModel`).
- **Accessibility / cost:** define a calm / reduced-motion behavior (a config flag clamping activity-driven channels), the Flat / low-color fallback for the new brightness + particle channels (ambient glyphs already no-op on Flat at `src/tui/panels/pet.rs:149-151`), and confirm constant ambient motion doesn't pin the fast tick (`has_active_effects`, `src/pet/animator.rs:188-189`).
- **Determinism:** add idle/baseline/hot dev-preview seeds (via the `activity_level` override seam) so the contact sheet exercises the range, and regenerate every affected snapshot — watch-wide, watch-compact, and (if particles gain a stage multiplier) the 6×7 pet-species-stage matrix + `tests/generation.rs` + the `.cells.json` frames. Pin a fixed stage tier in the pets matrix.

## Branch 3 — Panels & aggregates

- **One canonical day axis** (local-day on `bucket_at`): add a day-grouping query on that basis and route best-day, the intraday series, and today's total through it. Fix the **pre-existing** 7-day-history bug that compares a local date against UTC `period_date` (`src/storage/usage_store.rs`). Add a `best_prior_day(local_today)` query — `best_day_effective_tokens()` is arg-less and cannot exclude today.
  - **Superseded in part (2026-06-09):** the day-grouping helper, the today/7-day reader routing, and the 7-day local-vs-UTC fix moved to `2026-06-09-glorp-lives-in-time-design.md` (Branch T1), which also absorbs a compaction-retention fix on the same axis. This branch consumes that helper for best-day / `best_prior_day` / the intraday series; do not re-implement the axis here.
- **Intraday hourly sparkline** (today-by-hour, local) in `TodayPanel`, via the existing spark widget. Define empty/early-day and midnight-rollover behavior.
- **Best-day milestone:** display + a non-persistent "new best day" highlight when today's running total beats the best prior day, all on the canonical axis. Define the midnight rollover.
- **Intra-stage pips:** a display-only sub-level row from the stage-relative fraction (`floor(fraction × PIP_COUNT)`). No persistence, no narration.
- **Always show pace:** locate the **actual** rate-hide site(s) and tests (the review's `widgets.rs:284/485` citation is unverified — the file may be elsewhere) before changing the shared widget contract. Prefer omitting pace when there is none over a permanent "idle" label (the latter flirts with meter-not-creature framing).
- **Vertical budget pass:** at the minimum supported height (72×24, where compact mode already hides bio/speech), total the added rows (pips, sparkline, best-day) against the `Fill(1)` pet they steal from, and gate each explicitly. Prefer inlining (magnitude on the bar) over new rows.

## Testing

Test-driven, per project convention; pristine output. Unit coverage for: the canonical `activity_level` (idle → 0, sustained session → graduated mid-range, not pegged; EMA decay; NaN/zero-baseline guard); the rebased `recent_tokens_per_min` (gradual decay, munch *can* fire under a realistic delta); `stage_start_xp` + corrected fraction (0% at stage entry, 1.0 at next threshold; the new fixture/snapshot values); proportional burst mapping incl. the midnight reset; activity-driven thought/speech selection for both subsystems; the canonical day-axis query, `best_prior_day`, and the intraday/best-day rollover; pip computation. Animation channel fns get deterministic tests at fixed `activity_level`. Enumerate and regenerate all affected snapshots in the same branch that changes each frame.

## Tuning

Every response strength is a named constant. After the curve is validated against a real ledger and the channels are wired with conservative defaults, dial them live in `cargo run -- watch`.

## Out of scope (clean follow-ons)

Pip narration + persistence, daily streak counter, habitat ladder past 25M lifetime tokens, new mood variants, periodic baseline re-calibration (touches saved-state semantics — separate decision).
