# Glorp Lives In Time — design

- Date: 2026-06-09
- Status: direction approved by Drew; spec pending review
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

- **Real data only.** The local clock, the calendar, the learned rhythm
  profile, and the applied-usage ledger are all real observed state. Every
  time-driven variation derives from one of them. Flavor (speech, dream
  wording, idle quirks) is allowed only where a real signal selects it.
- **Tamagotchi spirit.** Day texture reads as a lived-in room, not a progress
  meter. No streaks, quotas, ETAs, or countdowns anywhere.
- **Calm magical over energetic flashing.** Night is calmer than day, never
  busier. New channels respect existing intensity caps and compact budgets.
- **Privacy-local.** The new context layer is numeric/enum-only, like
  `PetLifeProfile`.
- **Deterministic and provable.** Pure functions of (clock, date, rhythm,
  aggregates). The clock is an input (`WatchClock`), so Preview Lab fixtures
  can prove every distinct scene.
- **Port, don't invent.** No new authored art templates. The renderer stays
  content-agnostic.
- **The pet always wins.** Time-of-day ambience supports the creature; it
  never crowds it.

## DayContext: the new derived layer

A sibling of `PetLifeProfile`: a derived presentation contract built once per
poll (and cheaply re-derived per frame where needed), carried on
`WatchViewModel`, consumed by the pet panel, animator, speech/activity
selectors, and the menubar — both frontends stay at parity through the shared
view model, exactly like the live profile does today.

Fields (all derived; **nothing new is persisted**):

| Field | Type | Source |
|---|---|---|
| `day_phase` | `Dawn \| Day \| Dusk \| Night` | `RhythmProfile.active_hours[24]` with clock-default boundaries until the profile has signal |
| `date_seed` | `u64` | hash(local date, pet seed) |
| `today_ratio` | `f32` | today's applied effective tokens (local-day axis) / `CalibrationBaseline.daily_effective_tokens`; `0.0` without a baseline |
| `yesterday` | `Option<DaySummary>` | yesterday's daily aggregate: ratio vs baseline + dominant token-shape class |
| `climate` | weather-class enum | dominant token shape over the trailing 7 daily aggregates |
| `is_weekend` | `bool` | local weekday (+ `weekend_activity_weight` available for tuning) |
| `season` | `Spring \| Summer \| Autumn \| Winter` | local month, fixed northern mapping (named constant) |
| `stage_tier` | `Early \| Mid \| Late` | qualitative cut of xp-in-stage / xp-to-next |
| `asleep` | `bool` | `day_phase == Night` && live profile idle; see sleep semantics |

Derivation notes:

- **Day phase from rhythm.** Night is the contiguous low-weight window of
  `active_hours`; dawn/dusk are the entry/exit shoulders. Until the rhythm
  profile has enough active days, fall back to clock defaults (dawn 07–09,
  day 09–18, dusk 18–22, night 22–07 local). Boundaries are named constants
  for the established live-tuning pass.
- **`date_seed`** changes at local midnight and differs between pets with
  different seeds — same pet, same day, same character.
- **`yesterday` / `climate`** reuse the existing token-shape classifier
  (`classify_work_weather`) at day granularity over `daily_aggregates`.
  Weather is hours; climate is the week.
- **`stage_tier`** is display-only and qualitative. It must never render as a
  number, pip count, or remaining-amount.

### Sleep semantics

**Asleep is a presentation state, not a mood.** The mood enum stays the
vitals contract, untouched. Sleep is: night phase, and the live profile shows
genuine idleness (`is_recently_active == false`). Any **Live-freshness
burst** wakes the pet immediately — the normal feed-reaction chain fires (the
pet blinks awake, eats, watches you work) — and sleep re-arms only after
sustained idle. The hysteresis window lives in-memory on `WatchApp` /
menubar `AppState` (the established WatchApp-owned-state pattern, surviving
wholesale vm replacement). Backfill, cold start, and diagnostics-only polls
cannot wake the pet, by the existing freshness gates.

### Local-day axis (prerequisite, absorbed into T1)

All "today / yesterday / 7-day" math stands on one canonical local-date
grouping of `bucket_at`. This absorbs **only** the `period_date`
local-vs-UTC fix already described in the queued panels branch (Branch 3 of
the 2026-06-04 design); the panel features themselves (hourly sparkline,
pips, pace) stay queued and untouched. Local-day rollover uses calendar-date
arithmetic, not 24h offsets, so DST days group correctly.

## Branch T1 — time foundation + day/night + sleep

The flagship slice that proves the contract end-to-end (as Branch 2 did for
`PetLifeProfile`):

- `DayContext` layer, local-day axis fix, derivation unit tests.
- **Habitat day/night cycle**: sky glyph family and palette warmth keyed to
  `day_phase` — dawn warm and low, day bright, dusk amber, night starfield
  with a dimmed palette. Flat color capability degrades to glyph-only
  variation (no palette shifts), per the existing Flat rules.
- **Night calm**: night sets `calm_mode = true` — the first real setter for
  the already-wired quiet rendering path. A user-facing calm config flag
  remains deferred, as Branch 2 decided.
- **Sleep**: closed eyes (existing species blink glyphs, held), breath slowed
  and deepened (amplitude/period scaling in the animator), wander suppressed,
  speech replaced by a sparse `zzz` cadence. The menubar pet sleeps too, for
  free, via the shared view model.
- **Wake-on-burst** per the sleep semantics above.

## Branch T2 — the pet's day

- **Day accumulation**: floor motes whose density tracks `today_ratio`,
  capped well below ambient-budget limits; cleared at local-day rollover
  ("the room is tidied overnight"). It reads as lived-in texture: no
  numbers, no bar-like fill direction, no completion framing.
- **Honest energy arc (presentation-only)**: the existing low-energy droop
  shader gains a time dimension — droop intensity additionally weighted by
  `today_ratio` and hours since the day's first activity, so the pet visibly
  tires late in a heavy day and looks fresh in your morning. The energy
  vital itself is not touched.
- **Morning-after**: during the first dawn/day-phase polls of a day,
  idle-line and greeting selection is flavored by `yesterday.ratio` — mellow
  and content after a feast day, eager after an idle one. Selection only;
  no fabricated events.
- **Dreams**: while asleep, sparse dream bubbles in the speech channel.
  The dream family (misty / sparking / pulsing) is selected by
  `yesterday.dominant_shape`; appearance windows are deterministic from
  `date_seed` + hour. A real signal picks the flavor; the clock picks the
  moment.

## Branch T3 — day character + slow change

- **Today's character**: `date_seed` picks the sky glyph family variant, the
  idle-quirk subset the pet favors, and the day's speech vocabulary subset.
  Same catalogs, daily rotation — variation without invention.
- **Prop companion of the day**: `date_seed` picks one prop *from those
  actually earned*; the pet's wander targets bias toward it and it gets a
  gentle reaction styling. Grounded in real earned history.
- **Weekend texture**: lazier wander cadence and a softer palette when
  `is_weekend` — the rhythm profile already knows the user's weekends.
- **Climate rendering**: ambient glyph bias from the `climate` class — a
  cache-heavy week reads as lingering mist, a reasoning-heavy stretch as a
  pulsing sky.
- **Seasons**: a subtle hue/glyph drift by `season`. Deliberately the
  smallest item here.
- **Within-stage growth**: `stage_tier` selects among the pet's
  already-generated seed trait variants for `{pattern}`/`{accent}` slot
  rendering, so the body changes within a stage week to week. No new
  authored templates (port-don't-invent stands); no numeric progress
  display anywhere.

## Honesty and degradation rules

- Today/yesterday/climate reads come from the **applied ledger and
  `daily_aggregates` on the local-day axis** — never raw helper totals (the
  `observed_at`-vs-`bucket_at` lesson is load-bearing here).
- Freshness gating is unchanged and prerequisite: backfill and cold starts
  cannot wake the pet, fire dreams, or spike accumulation. Accumulation
  density derives from applied rows, which catch-up smearing already shapes.
- Rhythm cold start: clock-default phase boundaries until the profile has
  signal; a brand-new pet still gets day/night, just not personalized yet.
- Flat color: glyph-level variation only. Compact layouts keep existing
  budget caps; night reduces glyph counts, never adds.
- No new persisted state, no migrations. Sleep/wake hysteresis and
  morning-after bookkeeping are in-memory app state.

## Testing and proof

- Unit tests: phase boundaries from synthetic rhythm profiles (including
  cold start), `date_seed` determinism and midnight change, local-day
  rollover incl. DST days, asleep/wake hysteresis incl. freshness gates,
  `today_ratio` with and without a baseline.
- Preview Lab fixtures (deterministic via `WatchClock` + seeded ledger):
  `night-asleep`, `dawn-after-feast-day`, `heavy-day-evening`,
  `light-day-morning`, `weekend-midday`, `dream-night`; manifest inputs
  carry the `DayContext` so review is a contract, not a vibe.
- Whole-frame snapshots for wide/compact day and night scenes; the menubar
  BMP-invariant test extended to the sleeping pet.
- TDD per house rules for all implementation work.

## Non-goals

- No streak counters, daily quotas, ETAs, or countdowns.
- No new moods (asleep is a presentation state) and no new authored art
  templates.
- No new persisted state or migrations.
- No hemisphere/locale configuration (fixed month→season mapping as a named
  constant).
- No user-facing calm/scheduling config in this roadmap (still deferred).
- Panels-Branch-3 features stay out; only the local-day axis fix is
  absorbed.
