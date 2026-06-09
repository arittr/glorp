# Glorp Lives In Time — design

- Date: 2026-06-09
- Status: direction approved by Drew; revised after 3-lens red-team review; spec pending review
- Builds on: `2026-06-04-glorp-liveliness-design.md` (Branches 1–2 shipped),
  `2026-06-05-glorp-liveliness-branch2-design.md` (PetLifeProfile)

## Problem

Every aliveness channel shipped through liveliness Branch 2 is one of two
things: **live-work reactive** (varies only while tokens flow — activity
glyphs, bursts, accents, weather, prop reactions) or a **short-period
deterministic loop** (blink, 7s twinkle, 22s shimmer, 30s wander targets,
per-minute ambient reshuffle). The scene has high motion but no novelty at
longer timescales. An hour of glorp is statistically identical to any other
hour; this Tuesday looks like last Tuesday. The pet reacts to the present but
does not live in time — it has no morning, no evening, no "today," no
yesterday, no seasons of work.

Goal: the scene and the creature change meaningfully hour to hour, day to
day, and week to week — with every difference traceable to something real.

## Principles

Inherited from the liveliness designs, extended for time:

- **Real data only.** The applied-usage ledger, the learned activity rhythm,
  and the calibration baseline are real observed signals. The clock and
  calendar are real too, but with a locked boundary (2026-06-04 design,
  reaffirmed in Branch 2): **wall-clock/calendar time may vary visual
  texture** (ambient reshuffle, wander targets, sky family) **but never
  personality content** — which speech lines, idle quirks, or behaviors the
  pet shows during genuine idle must key off a real attribute (mood, energy,
  yesterday's work, time-since-real-activity), never the clock alone.
- **Tamagotchi spirit.** Day texture reads as a lived-in room, not a progress
  meter. No streaks, quotas, ETAs, or countdowns. The next stage stays a
  surprise. The pet's lines express its own state, never the user's absence.
- **Calm magical over energetic flashing.** Night is calmer than day, never
  busier. New channels live inside existing glyph budgets and intensity caps.
- **Privacy-local.** The new context layer is numeric/enum-only, like
  `PetLifeProfile`.
- **Deterministic and provable.** Pure functions of (clock, date, rhythm,
  ledger aggregates). The clock is an input (`WatchClock`), so Preview Lab
  fixtures can prove every distinct scene. Behaviors are derivable at
  view-model build time — restart-idempotent and consistent across frontends.
- **Port, don't invent.** No new authored art templates; no invented trait
  mechanisms. The renderer stays content-agnostic.
- **The pet always wins.** Time-of-day ambience supports the creature; it
  never crowds it.

## DayContext: the new derived layer

A sibling of `PetLifeProfile`: a derived presentation contract built in
`build_watch_view_model` (per poll; cheap fields re-derived per frame where
needed), carried on `WatchViewModel`, consumed by the pet panel, animator,
speech/activity selectors, and the menubar. Both frontends read the same
ledger and derive the same context, so they converge within one poll cycle
(≤10s skew); that is the parity contract.

Fields (all derived; **nothing new is persisted; no schema changes**):

| Field | Type | Source |
|---|---|---|
| `day_phase` | `Dawn \| Day \| Dusk \| Night` | learned activity rhythm over the applied ledger (below), clock defaults until mature |
| `date_seed` | `u64` | hash(local date, pet seed) — visual texture only |
| `today_ratio` | `f32` | today's applied effective tokens (canonical local-day axis) / `CalibrationBaseline.daily_effective_tokens`; always defined (the baseline is non-optional, default 100k); ratio-driven channels are gated by ledger maturity (below) |
| `yesterday` | `Option<DaySummary>` | `{ ratio: f32, dominant_shape: Option<weather class> }` from applied `usage_events` grouped on the local-day axis; `dominant_shape` is `None` when per-row token-shape detail is absent |
| `climate` | `Option<weather class>` | token components summed over the 7 complete local days before today, classified once with the existing `classify_work_weather` thresholds; `None` without shape detail; today is excluded |
| `is_weekend` | `bool` | local weekday; consumed together with `RhythmProfile.weekend_activity_weight` (normative, see T3) |
| `season` | `Spring \| Summer \| Autumn \| Winter` | local month, fixed northern mapping (named constant). Palette/glyph drift only — the season is **never named** in any UI text, speech line, or dream (the mapping is wrong for half the planet; texture is fine, claims are not) |
| `asleep` | `bool` | see sleep semantics |

### Learned activity rhythm (day_phase derivation)

`RhythmProfile.active_hours` is **not** usable here: ccusage history records
are date-only strings parsed to midnight UTC (`src/usage/normalize.rs:107`,
`src/usage/ccusage.rs:765-768`), so the learned mask marks hour 0 and nothing
else, and nothing updates the profile after `glorp init`. Day phase instead
derives from the ledger, which has real local hours and keeps learning:

- Build a local-hour histogram of applied effective tokens (`bucket_at`,
  `applied_at IS NOT NULL`) over the trailing `RHYTHM_WINDOW_DAYS` (default
  30).
- **Night** = the longest contiguous circular run of hours whose share of
  total volume is below `RHYTHM_QUIET_SHARE` (default 1% each). If that run
  is shorter than `MIN_NIGHT_RUN_HOURS` (default 5 — covers all-hours users
  and non-contiguous masks), fall back to clock defaults.
- **Dawn / Dusk** = fixed `PHASE_SHOULDER_HOURS` (default 2) carved from the
  night window's trailing/leading edges; **Day** = the remainder.
- **Maturity gate**: personalization applies only when the window contains at
  least `MIN_ACTIVE_DAYS` (default 5) distinct active local days; otherwise
  clock defaults (dawn 07–09, day 09–18, dusk 18–22, night 22–07 local).
  This same gate governs `today_ratio`-driven channels, since an immature
  ledger usually also means the calibration baseline is the unmeasured 100k
  default (it can be 10–100x off in a pet's first week — channels scaled by
  it must stay quiet rather than render a fabricated feast or famine).

All boundaries and thresholds are named constants for the established
live-tuning pass.

### Canonical local-day axis (prerequisite, absorbed into T1)

All "today / yesterday / trailing-7-day" math stands on one read-time
grouping of applied rows' `bucket_at` into **local calendar dates** (calendar
arithmetic, not 24h offsets, so DST days group correctly). Note
`daily_aggregates` is *not* a source for any DayContext window: compaction
only writes rows older than the 90-day retention cutoff
(`src/game/runtime.rs:20`), and its UTC-derived `period_date` rows cannot be
regrouped — the table stays as-is, permanently outside every DayContext
window. Absorbed scope from the queued panels branch: the grouping helper
plus routing the existing today/7-day readers (`today_effective_tokens` as
used by watch and `glorp status`, `seven_day_token_history`) through it, so
"today" agrees across every surface near midnight. The panels branch keeps
its features (hourly sparkline, pips, pace) — only the axis moves here.

### Sleep semantics

**Asleep is a presentation state, not a mood.** The mood enum stays the
vitals contract, untouched.

- `asleep` = `day_phase == Night` **and** zero applied effective tokens with
  `bucket_at` within the last `SLEEP_IDLE_MINUTES` (default 20), computed at
  view-model build time from the ledger.
- This single predicate gives symmetric onset and re-arm with no app-owned
  hysteresis state: the pet falls asleep after 20 quiet night minutes, and
  after a wake it sleeps again only after the same window. Restarts are
  idempotent; both frontends converge on the same answer from the same
  ledger within one poll.
- `bucket_at` recency is the honest axis (the Branch 1 lesson, commit
  136aa99): catch-up smears place old work at old bucket times, so backfill
  and cold starts do not wake the pet. A burst of genuinely recent work
  writes current-bucket rows and wakes it — the normal feed-reaction chain
  fires (it blinks awake, eats, watches you work). Freshness gating still
  owns whether burst *animations* fire, unchanged.
- Rhythm cold start: for a night-owl's first days, clock defaults mislabel
  their prime hours as night. Wake-on-burst plus the idle window is the
  intended mitigation — one gentle wake per session, never repeated churn.
  The hysteresis unit tests must cover exactly this scenario.

## Branch T1 — time foundation + day/night + sleep

The flagship slice that proves the contract end-to-end (as Branch 2 did for
`PetLifeProfile`):

- `DayContext` layer, learned-rhythm derivation, canonical local-day axis +
  reader routing, derivation unit tests.
- **Habitat day/night cycle**: per-phase sky glyph family and palette warmth
  — dawn warm and low, day bright, dusk amber, night a sparse starfield with
  a dimmed palette. Sky glyphs draw from the **existing ambient allocation**
  (they re-skin it, never add to it), with a per-phase budget where night ≤
  day. Flat color tier degrades to glyph-only variation (no palette shifts).
- **Night calm**: `calm_mode = night && asleep` — full quiet only while the
  pet actually sleeps. A user actively working at midnight keeps the entire
  live-reactive channel (Branch 2's flagship) — night never silences real
  work. This is the first real `calm_mode` setter; it must be applied after
  `LifeSignalState::observe` in both frontends (observe currently hardcodes
  `calm_mode: false`, `src/tui/life.rs:236`, and the vm is replaced
  wholesale each poll), with a unit test asserting calm survives a poll
  cycle. A user-facing calm config flag remains deferred, as Branch 2
  decided.
- **Sleep**: closed eyes (existing species blink glyphs, held), breath period
  lengthened and inhale window widened (both existing knobs in
  `compute_breath_offset`; a deeper 2-row offset only if the pet panel and
  menubar block verifiably have the vertical headroom), wander suppressed,
  speech showing a sparse `zzz` cadence (cadence rules under Dreams, T2).
  The menubar pet sleeps via the same derived context, within the one-poll
  parity contract.
- **Wake-on-burst** per the sleep semantics above.

## Branch T2 — the pet's day

- **Day accumulation**: floor motes whose density tracks `today_ratio` with
  **soft saturation** — asymptotic, sub-countable, placement jittered by
  `date_seed`, capped at `MOTE_BUDGET_SHARE` (default ≤ half) of the
  existing ambient allocation — so no visually distinct "full room" state
  exists to learn (a legible ceiling would be a quota readout by another
  name). Cleared at local-day rollover ("the room is tidied overnight").
  Gated by ledger maturity. No numbers, no fill direction, no completion
  framing.
- **Evening tiredness (its own vocabulary, not droop)**: the low-energy droop
  shader keeps its single meaning — the energy *vital* is low. Real day
  fatigue gets distinct, timing-level cues: blink cadence slows and breath
  period lengthens, scaled by `tiredness = f(today_ratio, hours since the
  day's first applied activity)` (named-constant curve; zero on light days
  via the `today_ratio` term, so dusk alone never fakes exhaustion). Timing
  cues survive the Flat color tier untouched. The energy vital and its bar
  are not touched.
- **Morning-after**: defined by clock, not bookkeeping — while `day_phase ==
  Dawn` and for the first hour of `Day`, idle-line/greeting *selection* is
  flavored by `yesterday.ratio` (mellow and content after a feast day, eager
  after an idle one). A pure function of (clock, DayContext): restart- and
  frontend-idempotent, nothing persisted. **Authoring guardrail**: morning
  lines express the pet's own state (rested, eager, content) and never
  reference the user's absence, name yesterday's lowness, or imply owed
  make-up work — glorp is not a guilt machine.
- **Dreams**: dream bubbles render **only when `yesterday` exists with
  `dominant_shape` detail** — the dream family (misty / sparking / pulsing)
  is selected by that real signal; the clock only picks the moment
  (deterministic windows from `date_seed` + hour). No signal → no dreams;
  sleep shows only the `zzz` cadence. While asleep the speech selector emits
  dream text during a dream window and `zzz` otherwise, surfacing on every
  `SLEEP_SPEECH_CYCLE_N`th (default 3rd) of the existing 30s speech cycles;
  mood idle lines and munch phrases are suppressed while asleep (munch
  cannot fire anyway — waking precedes feeding).

## Branch T3 — day character + slow change

- **Today's sky character**: `date_seed` picks the sky glyph family variant
  for the day. Visual texture only — the same accepted class as the
  per-minute ambient reshuffle. Explicitly **not** speech vocabulary or
  idle-quirk rotation: the locked real-data rule (2026-06-04 design)
  prohibits wall-clock-only idle personality rotation, and the calendar is
  the wall clock. Day-to-day *personality* variation comes from the real
  signals this spec already derives (yesterday, climate, mood).
- **Prop resonance**: the pet keeps company with the earned prop whose
  unlock story matches yesterday's real character — the heavy-session
  planter after a feast day, the codex signal lamp after a codex-heavy day,
  the recovery sprout after a wilt-recovery — selected from
  `yesterday`/`climate` against each earned prop's `HabitatPropSource`
  provenance, with `date_seed` only tie-breaking among equally qualified
  props. Wander targets bias toward it; it gets gentle reaction styling
  within existing intensity caps. **No qualifying signal → no companion**
  (the date hash alone must never manufacture a preference — that would be
  the animation-channel cousin of the rejected ambient-chatter lines).
- **Weekend texture**: softer palette and lazier wander cadence when
  `is_weekend` — **scaled down to zero as
  `RhythmProfile.weekend_activity_weight` shows the user is weekend-active**
  (a sleepy room over a hot Saturday session would contradict observed
  reality), and live-activity channels always win over weekend softening.
- **Climate rendering**: ambient glyph bias from the `climate` class — a
  cache-heavy week reads as lingering mist, a reasoning-heavy stretch as a
  pulsing sky. `None` (no shape detail) renders nothing.
- **Seasons**: a subtle hue/glyph drift by `season`. Deliberately the
  smallest item; drift only, never named (see field table).

### Considered and cut (recorded so it stays cut)

- **Within-stage body growth** (stage-tier `{pattern}`/`{accent}` variation):
  cut after red-team review. The "already-generated trait variants" the
  first draft cited do not exist (generation picks exactly one pattern and
  accent per seed, `src/pet/generation.rs:145-209`); pet.jsx has no such
  mechanism, so it would breach port-don't-invent; and a learnable
  Early→Mid→Late body is a progress meter worn on the creature that softly
  telegraphs evolution proximity, eroding the next-stage-surprise principle.
  Revisit only as its own explicitly-decided design.
- **Date-seeded speech vocabulary / idle-quirk subsets**: cut — violates the
  locked wall-clock idle-personality rule (see T3 first bullet).

## Honesty and degradation rules

- Today/yesterday/climate reads come from **applied rows only**
  (`applied_at IS NOT NULL`) on the canonical local-day axis — never raw
  helper totals, never staged rows (the `observed_at`-vs-`bucket_at` lesson
  is load-bearing here).
- Freshness gating is unchanged and prerequisite: backfill and cold starts
  cannot fire burst animations; the sleep predicate's `bucket_at` recency
  keeps them from waking the pet; smeared catch-ups shape accumulation the
  same way they shape feeding.
- Ledger maturity (≥ `MIN_ACTIVE_DAYS` distinct active local days in the
  rhythm window) gates both rhythm personalization and `today_ratio`-driven
  channels; immature pets get clock-default phases, no motes, no tiredness
  scaling — day/night still works, just not personalized.
- Flat color: glyph- and timing-level variation only. Compact layouts keep
  existing budget caps; night reduces effective glyph counts, never adds.
- No new persisted state, no migrations, no schema changes. Everything is
  derivable from (clock, ledger, existing PetState fields) at view-model
  build time.

## Testing and proof

- T1 unit tests: rhythm histogram → phase boundaries (typical 9–18 worker,
  night-owl, split-shift/non-contiguous, all-hours, immature ledger →
  defaults), `date_seed` determinism and midnight change, local-day rollover
  incl. DST days, asleep predicate (onset window, re-arm symmetry, backfilled
  old buckets don't wake, recent buckets do, night-owl cold-start one-wake
  scenario), calm_mode survives a poll cycle in both frontends, axis routing
  (status/watch/7-day agree across a midnight boundary).
- T2 unit tests: mote density soft-saturation and budget cap, maturity
  gating, tiredness curve (zero on light days; composes with droop without
  redefining it), morning-after window pure-function idempotence, dream
  window determinism and the no-signal → no-dreams rule, sleep speech
  cadence and suppression.
- T3 unit tests: prop-resonance qualification (earned-only, signal-matched,
  tie-break determinism, none-qualified → none), weekend scaling by
  `weekend_activity_weight`, climate aggregation (sum-then-classify,
  today excluded, shape-less → `None`).
- Preview Lab fixtures (deterministic via `WatchClock` + seeded ledger):
  `night-asleep`, `dream-night`, `dawn-after-feast-day`,
  `heavy-day-evening`, `light-day-morning`, `weekend-midday`,
  `climate-cache-week`, `prop-resonance-planter`; manifest inputs carry the
  `DayContext` so review is a contract, not a vibe.
- Whole-frame snapshots for wide/compact day and night scenes; the menubar
  BMP-invariant test extended to the sleeping pet.
- TDD per house rules for all implementation work.

## Non-goals

- No streak counters, daily quotas, ETAs, or countdowns.
- No new moods (asleep is a presentation state) and no new authored art
  templates or invented trait mechanisms (within-stage growth: cut, above).
- No wall-clock/calendar-driven personality content (visual texture only).
- No new persisted state, migrations, or schema changes.
- No hemisphere/locale configuration; the season is texture, never a claim.
- No user-facing calm/scheduling config in this roadmap (still deferred).
- Panels-Branch-3 features stay out; only the canonical local-day axis (and
  routing its existing readers) is absorbed.
