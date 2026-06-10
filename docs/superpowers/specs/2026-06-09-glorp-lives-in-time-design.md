# Glorp Lives In Time — design

- Date: 2026-06-09
- Status: direction approved by Drew; revised after two adversarial review
  rounds (3-lens red-team, then 3 staff-SWE hole-poke); spec pending review
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

- **Real data only.** The applied-usage ledger, the calibration baseline, and
  derived activity rhythm are real observed signals. The clock and calendar
  are real too, but with a locked boundary (2026-06-04 design, reaffirmed in
  Branch 2): **wall-clock/calendar time may vary visual texture** (ambient
  reshuffle, wander targets, sky family) **but never personality content** —
  which speech lines, idle quirks, or behaviors the pet shows during genuine
  idle must key off a real attribute, never the clock alone.
- **Honest about its own data.** `bucket_at` is poll-anchored: the smear
  places a delta's newest bucket at the current 10-minute bucket and trails
  back at most ~110 minutes (`src/game/runtime.rs:41-48`,
  `src/game/catchup.rs:10`). It approximates work time well for running
  frontends and degrades for poll-sparse users. This spec's derivations are
  designed around that reality and never claim more precision than it has.
- **Tamagotchi spirit.** Day texture reads as a lived-in room, not a progress
  meter. No streaks, quotas, ETAs, or countdowns. The next stage stays a
  surprise. The pet's lines express its own state, never the user's absence.
  The sanctioned vitals signal (hungry/sad/wilted) always outranks flavor.
- **Calm magical over energetic flashing.** Night is calmer than day, never
  busier. No whole-scene single-frame swaps: boundaries get deterministic
  ramps (below).
- **Privacy-local.** The new context layer is numeric/enum-only.
- **Deterministic and provable.** Pure functions of (clock, local-day mapper,
  ledger aggregates). Clock and mapper are injected, so Preview Lab and unit
  tests pin every scene. Behaviors are derivable at view-model build time —
  restart-idempotent and convergent across frontends within one poll.
- **Port, don't invent.** No new authored art templates; no invented trait
  mechanisms. The renderer stays content-agnostic.
- **The pet always wins.** Time-of-day ambience supports the creature; it
  never crowds it.

## DayContext: the new derived layer

A sibling of `PetLifeProfile`: a derived presentation contract built **once
per poll** in `build_watch_view_model_at` from its `(now, local-day mapper)`
parameters and carried on `WatchViewModel` — including precomputed **UTC
boundary instants** (phase edges, sleep-window edge, rollover). Per-frame
logic may only compare `clock.now_utc()` against those instants;
`RenderContext` gains no offset and no per-frame local conversion is
permitted (Preview Lab pins the clock but cannot pin a render-loop
`current_local_offset()` call).

Fields (all derived; no new persisted state):

| Field | Type | Source |
|---|---|---|
| `day_phase` | `Dawn \| Day \| Dusk \| Night` | activity-rhythm histogram over the applied ledger (below); clock defaults until mature |
| `date_seed` | `u64` | hash(local date of the most recent Dawn entry, pet seed) — visual texture only; rolling at dawn (not midnight) so the sky never swaps mid-night |
| `today_ratio` | `f32` | today's applied effective tokens (local-day axis) / `CalibrationBaseline.daily_effective_tokens`; always defined (the baseline is non-optional, default 100k) |
| `yesterday` | `Option<DaySummary>` | `None` **iff the ledger has no coverage of that local day** (pre-hatch / pre-retention); an observed idle day is `Some { ratio: 0.0, dominant_shape: None }` |
| `climate` | `Option<weather class>` | modal class over the 7 complete local days before today (below); `None` when no day classifies |
| `is_weekend` | `bool` | local weekday |
| `weekend_share` | `f32` | weekend share of applied effective tokens over the rhythm window (ledger-derived; `RhythmProfile.weekend_activity_weight` is frozen-at-init and binary — metabolism keeps it, this spec does not use it) |
| `season` | `Spring \| Summer \| Autumn \| Winter` | local month, fixed northern mapping (named constant). Palette/glyph drift only — **never named** in any UI text, speech, or dream |
| `asleep` | `bool` | see sleep semantics |

`DaySummary.dominant_shape` = sum the day's stored shape components
**effective-weighted** (cache_read × `cache_read_weight`), then classify with
the existing `classify_work_weather` thresholds — with a pre-check: all-zero
components with nonzero effective tokens → `None` (the classifier itself
returns `Clear` for empty shapes and must not be used to encode absence).

**Climate** classifies each of the 7 days that way and takes the modal class
(ties → `Clear`); days without shape detail are excluded; no day classifies →
`None`. `Clear` and `None` both render nothing. Effective-weighting exists
because raw cache-read tokens dominate agentic workloads — a raw 7-day sum
would pin nearly every user at CacheMist forever (the live weather channel
keeps its raw per-delta shares; day-scale classification is a different
regime). A Preview Lab fixture must prove at least two distinct climates
arise from realistic weekly mixes.

**Maturity gate** (one gate, two conditions): personalization and all
baseline-ratio-scaled channels activate only when the rhythm window contains
≥ `MIN_ACTIVE_DAYS` (default 5) distinct active local days **and** ≥
`MIN_DISTINCT_ACTIVE_HOURS` (default 3) distinct active hours. An "active
day/hour" = one with ≥1 applied row with effective tokens > 0 (deliberately
no volume threshold — the baseline may be unmeasured at this point). The gate
governs: rhythm personalization, `weekend_share` scaling, and **every channel
scaled by a baseline ratio** — `today_ratio` motes and tiredness,
`yesterday.ratio` morning-after flavor, and ratio-qualified prop resonance —
because the default 100k baseline can be 10–100x off in a pet's first week
and must not render a fabricated feast or famine.

### Local-day mapper (the timezone seam)

All local-date math goes through one injectable mapper: `FixedOffset(off)`
for tests and Preview Lab (dev_preview currently injects UTC,
`src/dev_preview/watch.rs:157`), `System` in production. `System` resolves
the UTC offset **once per calendar-day boundary in the window** (~30
`localtime_r` calls for the rhythm window, not one per row), so DST days
group correctly without a per-row libc call. Production resolves the mapper
per view-model build via `current_local_offset()`; the documented fallback
on resolution failure is UTC (named here so it is a decision, not an
accident). Rows with `bucket_at` in the future (clock set backwards) count
as recent activity for the sleep predicate (fail-awake) and are excluded
from day windows.

### Activity rhythm (day_phase derivation)

`RhythmProfile.active_hours` is **not** usable: ccusage history records are
date-only strings parsed to midnight UTC (`src/usage/normalize.rs:107`,
`src/usage/ccusage.rs:765-768`), so the learned mask marks hour 0 only, and
nothing updates the profile after `glorp init`. Day phase derives from the
ledger instead:

- Local-hour histogram of applied effective tokens (`bucket_at`,
  `applied_at IS NOT NULL`) over the trailing `RHYTHM_WINDOW_DAYS` (30).
- **Night** = the longest contiguous circular run of hours each below
  `RHYTHM_QUIET_SHARE` (1%) of total volume, **clamped to
  `MAX_NIGHT_RUN_HOURS` (12), centered on the quiet run's midpoint** — a
  4h/day user must not get a 20-hour night and a vanishing Day (Day is
  always ≥ 24 − MAX − 2×shoulders). Quiet run shorter than
  `MIN_NIGHT_RUN_HOURS` (5), or equal-length tie runs, or an immature
  ledger → clock defaults (dawn 07–09, day 09–18, dusk 18–22, night 22–07).
- **Dawn / Dusk** = `PHASE_SHOULDER_HOURS` (2) carved from the night
  window's trailing/leading edges; **Day** = the remainder.
- Honesty note: for status-only users the histogram concentrates at
  glorp-invocation hours; the MAX clamp bounds the damage and such users
  rarely watch the scene. Recorded as accepted degradation.

**Performance mandate**: all DayContext window math (histogram, today,
yesterday, climate, sleep recency) is **SQL-side aggregation** — one pass,
GROUP BY local day/hour — computed once per poll and carried on the vm.
Never row-fetch-and-bucket in Rust (the real ledger is ~130k rows/28 days),
never per frame. The `no new persisted state` constraint means no new
*semantic* state; adding `CREATE INDEX IF NOT EXISTS
idx_usage_events_bucket_at` is an access-path addition under the existing
idempotent-migration pattern and is explicitly allowed.

### Canonical local-day axis (prerequisite, absorbed into T1)

One read-time grouping of **applied** rows' `bucket_at` into local calendar
dates (via the mapper) feeds DayContext **and** the routed readers. The
actual readers, named precisely:

- `glorp status` today: `today_effective_tokens`
  (`src/storage/usage_store.rs:773-784`) — currently UTC `period_date`,
  includes unapplied rows; reroute.
- watch today panel: `token_totals_by_source_between`
  (`src/commands/watch.rs:89-91`) — already local-midnight on `bucket_at`;
  the routed helper must preserve its **per-source grouping**
  (source_breakdown / source_health depend on it).
- `seven_day_token_history` (`usage_store.rs:819-863`) — fixes its existing
  local-date-vs-UTC-`period_date` mismatch; its `daily_aggregates` UNION
  drops (compaction cutoff is 90 days, so aggregates cannot occur in a
  7-day window), which **supersedes** the pinned test
  `seven_day_token_history_includes_compacted_days` — explicitly, not
  silently.
- Both store readers take `(now, mapper)` parameters so midnight-boundary
  tests are unit-level (no env seam can pin the real binary's clock).

Recorded trade-offs of the single axis: daily attribution is **eat-time,
not work-time** — work done Friday evening with glorp closed lands on
Saturday's history when it is polled. The pet's day is when it ate; all
surfaces agree with each other and with the motes. Large backfills are
visible only as they apply (`apply_unapplied_usage` caps 500 rows/run,
`src/game/runtime.rs:116`) — a test must cover a >500-row staged backlog
converging over successive polls.

**Retention fix (absorbed)**: `compact_before` deletes by `period_start <
cutoff` only (`usage_store.rs:275,289`) — a long-gap resume (provider days
>90d old, `bucket_at` = now) would have its rows applied and then deleted by
the *next* poll's compaction while still inside every DayContext window.
The compaction predicate gains `AND bucket_at < cutoff` (query change, not
schema), with a named test.

The 2026-06-04 design's Branch 3 keeps best-day / intraday series / panel
features and **consumes** this helper; a supersession note is added there so
the axis work is not double-assigned.

### Sleep semantics

**Asleep is a presentation state, not a mood.** The mood enum, the mood
passed to `render_pet`, and the vitals panel are untouched — sleeping must
not masquerade as `Mood::Sleepy` (that would fire a spurious MoodFade on
every onset/wake via the animator's mood diff, `src/pet/animator.rs:137-141`,
and lie on the vitals panel).

- `asleep` = `day_phase == Night` **and** zero applied effective tokens with
  `bucket_at` within `SLEEP_IDLE_MINUTES` (20) **and** the ledger contains at
  least one applied row ever (**a newborn that has never eaten stays awake
  watching for its first meal** — a pet hatched at 11pm must not be
  unconscious the moment the hatch animation ends).
- One predicate, computed at vm build from the ledger: symmetric onset and
  re-arm, no app-owned hysteresis, restart-idempotent, frontends converge
  within one poll.
- **Catch-up wake is accepted and bounded** (decided after review): the
  smear anchors every delta — including backfill — at the current bucket, so
  *any* newly applied tokens wake the pet, and the ledger cannot
  distinguish backfill from live work after staging. Opening the app at
  night to deliver accumulated food wakes the pet **once**: it stirs, eats,
  and re-sleeps after `SLEEP_IDLE_MINUTES` (self-limiting — no further rows
  arrive). The wake is gentle by construction because burst *animations*
  (feed sweep, token pop) remain freshness-gated and Backfill/ColdStart
  cannot fire them; only eyes and speech change. A genuinely live night
  burst wakes with the full feed-reaction chain, simultaneously on one poll
  frame (accepted). Tests must drive the real `stage_usage_poll_deltas`
  path with a cold-start delta — not fixture rows with fabricated old
  `bucket_at` values the production smear never writes.
- Rhythm cold start: clock defaults can mislabel a night-owl's prime hours
  as night for their first days; wake-on-tokens plus the idle window means
  one gentle wake per session, never churn. Covered by a named test.

**While asleep** (interaction rules — all named so implementers don't
diverge):

- Eyes: held closed via a new `hold_eyes_closed` field on `AnimationFrame`
  (the existing closed-blink glyphs; today blink is a single-tick pulse with
  no hold seam, `src/pet/render.rs:254`), threaded through
  `rerender_pet_for_view_model`. The mood-substitution shortcut is
  forbidden (above).
- **Cursor-tracked eyes are disabled** (`cursor_norm_x = None` while
  asleep) — closed eyes must not pop open and follow the mouse.
- **Petting** (`p`) selects from a sleep-flavored reaction pool (`*snore*`,
  `*stirs*`, `...zzz`), does not open the eyes, and does not wake the pet
  (the predicate is ledger-derived; petting is not work). The transient
  vitals nudge still applies.
- Breath: slowed period and widened inhale via a sleep-rhythm parameter on
  `compute_breath_offset` (`src/pet/animator.rs:274-297`) — the phase is
  **anchored at the derived sleep-onset instant** (newest qualifying
  `bucket_at` + `SLEEP_IDLE_MINUTES`, or night start, whichever is later)
  so the period change is continuous, not a pop. `render.rs`'s
  `AnimationProfile.breath_*` fields are vestigial for breathing and out of
  scope — do not scale them. A deeper 2-row offset only if the pet panel
  and menubar block verifiably have vertical headroom.
- Wander: position and facing are evaluated at the sleep-onset instant and
  **held** (no center-snap, no 30s mirror-flips with shut eyes); onset and
  wake ease between held and live positions over `WANDER_SETTLE_SECS`
  (named constant) keyed to the derived boundary instants — pure functions,
  preview-pinnable.
- Speech: the selector emits dream text during a dream window, `zzz`
  otherwise, surfacing every `SLEEP_SPEECH_CYCLE_N`th (3rd) 30s cycle; mood
  idle lines and munch phrases are suppressed.
- Feed-surface consistency: idle activity thoughts draw from a
  sleep-flavored set while asleep; the persisted idle-narration vocabulary
  is partitioned — sleep-claiming variants ("drifted off", "dreams") are
  eligible **only** while asleep, neutral variants ("is quiet", "settled
  in") otherwise (absorbed scope: touches existing narration pools, which
  today claim sleep at any hour after 30 idle minutes).
- **Milestone effects are exempt**: hatch, stage-up morph, and the evolution
  overlay ignore `calm_mode` and render the pet awake (eyes open) for their
  duration; sleep re-derives normally afterward.
- Prop-resonance styling pauses while asleep (calm already scales it; full
  pause avoids a glowing shrine over a sleeping pet).

## Branch T1 — time foundation + day/night + sleep

- `DayContext` layer, local-day mapper, activity rhythm, canonical axis +
  reader routing + retention fix, derivation unit tests.
- **Habitat day/night cycle**: per-phase sky glyph family and palette warmth
  — dawn warm and low, day bright, dusk amber, night a sparse starfield with
  a dimmed palette. Sky glyphs **re-skin the existing ambient allocation**
  (never add to it), per-phase budget with night ≤ day. **Flat tier
  decision**: Flat renders zero ambient glyphs today
  (`src/tui/panels/pet.rs:280-282`) and keeps zero — day/night in Flat is
  conveyed only through pet timing cues (blink/breath); the
  glyph-only-degradation rule applies to the pet, not the sky.
- **Night calm**: `calm_mode = night && asleep` — full quiet only while
  actually sleeping; midnight work keeps the entire live-reactive channel.
  Setter ordering: applied after `LifeSignalState::observe` **and before
  any profile consumer in the same install path** (speech selection,
  activity derivation, prop reactions — `src/tui/app.rs:495-516`,
  `src/menubar/app.rs:386-397`; observe hardcodes `calm_mode: false`,
  `src/tui/life.rs:236`). Both frontends ship a default profile until the
  first poll (~10s un-calm at startup — within the accepted skew); the
  calm test covers first-poll establishment. User-facing calm config stays
  deferred.
- **Sleep** and **wake** per the semantics above. Menubar scope, stated
  honestly: the popover shows sleep eyes and the dimmed palette via the
  same context; breath/wander/zzz cues are watch-TUI-only (the popover has
  no positioning or speech surface, `src/menubar/app.rs:423-450`).

### Boundary behavior (no single-frame scene swaps)

- Phase palettes interpolate across the shoulder edges over
  `PHASE_BLEND_MINUTES` (30) — a dawn crossing is a slow warm-up, not a
  flip.
- Motes fade out over `MOTE_TIDY_FADE_MINUTES` (30) after local-day
  rollover instead of vanishing mid-grind at 00:00.
- `date_seed` (sky family) rolls at the Dawn entry, not midnight.
- Tiredness (T2) uses a trailing window, so nothing snaps at midnight.
- All ramps are pure functions of the boundary instants already carried on
  the vm. Dawn-crossing and midnight-mid-session get Preview Lab fixtures.

## Amendment (2026-06-10): T2 + T3 ship as one combined branch

Decided with Drew after T1 shipped in v0.6.1: Branches T2 and T3 are
implemented as a single branch (`feat/lives-in-time-t2t3`) — none of the
remaining features is individually large now that the `DayContext` contract
exists. The combined branch also absorbs two items born from the
2026-06-10 incident (ccusage 20.x silently became an all-agents aggregator
and its first successful poll fed the pet a 212M-effective-token bolus of
non-claude history):

- **Usage discontinuity guard.** Before staging, if a single poll's summed
  delta exceeds `DISCONTINUITY_GUARD_RATIO` (default 3.0) ×
  `CalibrationBaseline.daily_effective_tokens` × `max(1,
  whole_days_since_last_successful_poll + 1)`, the poll is a discontinuity:
  **advance the provider cursors without staging any ledger rows** (the
  calibration-path precedent — totals are marked seen, the pet does not
  eat) and persist a `usage_discontinuity` diagnostic carrying the
  magnitude. The elapsed-days factor keeps honest vacation catch-ups
  feeding (a real week away passes at ~8× headroom) while a same-cadence
  poll claiming 10× a typical day is refused. This changes feeding
  semantics by design: a delta that implausible is a helper or cursor
  discontinuity, not work, and refusing it is the honest reading. Checked
  against the live incident: elapsed ~10s → threshold ≈ 59M ≪ the observed
  212M → the guard would have fired.
- **Local feed timestamps.** Feed/event `hh:mm` labels currently render
  the UTC clock (last night's 23:00 PDT feeds displayed as "06:00").
  Display-only fix: format via the mapper's local offset. No stored data
  changes.

## Branch T2 — the pet's day

- **Day accumulation**: floor motes whose density tracks `today_ratio` with
  soft saturation — asymptotic, sub-countable, placement jittered by
  `date_seed`, capped at `MOTE_BUDGET_SHARE` (≤ half) of the existing
  ambient allocation — no visually distinct "full room" exists to learn.
  Cleared at rollover with the tidy fade. Maturity-gated. No numbers, no
  fill direction, no completion framing.
- **Evening tiredness (its own vocabulary, not droop)**: the low-energy
  droop shader keeps its single meaning — the energy *vital* is low. Real
  fatigue gets timing-level cues (blink cadence slows, breath period
  lengthens), scaled by `tiredness = f(active_hours, volume_ratio)` where
  `active_hours` = count of 10-minute buckets containing applied tokens in
  the trailing `FATIGUE_WINDOW_HOURS` (16) ÷ 6, and `volume_ratio` = that
  window's effective tokens / baseline. Accumulated *active time*, not
  elapsed span — a heavy morning followed by a six-hour lid-closed rest
  must not render near-max tiredness at 4pm, and a trailing window means no
  midnight snap. Zero on light days via the volume term. Timing cues
  survive Flat. The energy vital and its bar are untouched. Maturity-gated.
- **Morning-after**: defined by clock — while `day_phase == Dawn` and for
  the first hour of `Day`, idle-line/greeting *selection* is flavored by
  `yesterday.ratio` (mellow after a feast day; the idle-day flavor fires
  for `Some` with ratio ≈ 0, and `None` — no ledger coverage — selects no
  flavor at all). Pure function of (clock, DayContext): restart- and
  frontend-idempotent. Maturity-gated (it is a baseline-ratio channel).
  **Authoring guardrail**: morning lines express the pet's own state
  (rested, eager, content) and never reference the user's absence, name
  yesterday's lowness, or imply owed make-up work.
- **Dreams**: only when `yesterday` is `Some` **with** `dominant_shape`
  detail — the dream family (misty / sparking / pulsing) is selected by
  that real signal; the clock only picks the moment (deterministic windows
  from `date_seed` + hour). No signal → no dreams, `zzz` only.
- **Speech precedence stack** (binding, top wins):
  1. petting override (awake or sleep-flavored pool)
  2. while asleep: dream window, else `zzz`
  3. live-burst munch
  4. needy mood line (Hungry / Sad / Wilted — the sanctioned vitals signal
     always outranks flavor; hungry-at-dawn-after-an-idle-yesterday shows
     "feed me?", not an eager greeting — two channels both keyed to the
     user working less must never stack into nagging)
  5. morning-after greeting flavor
  6. default mood line

## Branch T3 — day character + slow change

- **Today's sky character**: `date_seed` picks the sky glyph family variant
  for the day (rolls at dawn). Visual texture only — explicitly **not**
  speech vocabulary or idle-quirk rotation (locked rule: the calendar is
  the wall clock). Personality variation comes from the real signals this
  spec derives (yesterday, climate, mood).
- **Prop resonance**: the pet keeps company with the earned prop whose
  unlock story matches yesterday's real character — the heavy-session
  planter after a feast day, the codex signal lamp after a codex-heavy day,
  the recovery sprout after a wilt-recovery — matched from
  `yesterday`/`climate` against each prop's `HabitatPropSource` provenance;
  `date_seed` only tie-breaks among equally qualified props. Wander bias +
  gentle reaction styling within existing caps; paused while asleep. **No
  qualifying signal → no companion.** Ratio-qualified matches are
  maturity-gated.
- **Weekend texture**: softer palette and lazier wander cadence when
  `is_weekend`, scaled by the ledger-derived `weekend_share`: full
  softening at share ≤ `WEEKEND_QUIET_SHARE` (0.10), fading to zero at ≥
  `WEEKEND_ACTIVE_SHARE` (0.30) — a weekend-active user gets no sleepy
  Saturday room. Live-activity channels always win over softening.
  Maturity-gated.
- **Climate rendering**: ambient glyph bias from the `climate` class
  (modal, effective-weighted, per the contract section). `None`/`Clear`
  render nothing.
- **Seasons**: a subtle hue/glyph drift by `season`. Deliberately the
  smallest item; drift only, never named.

### Considered and cut (recorded so it stays cut)

- **Within-stage body growth**: cut after round 1. The "already-generated
  trait variants" the first draft cited do not exist (generation picks one
  pattern and one accent per seed, `src/pet/generation.rs:145-209`);
  pet.jsx has no such mechanism (port-don't-invent); and a learnable
  Early→Mid→Late body is a progress meter that telegraphs evolution
  proximity. Revisit only as its own explicitly-decided design.
- **Date-seeded speech vocabulary / idle-quirk subsets**: cut — violates
  the locked wall-clock idle-personality rule.
- **"Backfill cannot wake the pet"**: cut after round 2 — the claim was
  false against the smear's bucket anchoring (`src/game/runtime.rs:41-48`),
  and no ledger-only predicate can deliver it. Replaced by the accepted
  bounded catch-up wake (sleep semantics), which keeps the predicate pure,
  restart-idempotent, and honest about what the data can support.

## Honesty and degradation rules

- DayContext reads come from **applied rows only** (`applied_at IS NOT
  NULL`) on the canonical local-day axis — never raw helper totals, never
  staged rows.
- Freshness gating still owns burst *animations*: Backfill/ColdStart cannot
  fire the feed sweep or token pop — which is exactly what keeps the
  accepted catch-up wake gentle.
- The maturity gate (one definition, contract section) governs rhythm
  personalization and every baseline-ratio channel; immature pets get
  clock-default phases, no motes, no tiredness scaling, no morning-after
  flavor, no ratio-qualified resonance — day/night still works, just not
  personalized.
- Flat color: zero ambient glyphs (unchanged contract); day/night reaches
  Flat users through pet timing cues only. Compact keeps existing budget
  caps; night reduces effective glyph counts, never adds.
- Future-dated rows fail-awake and are excluded from day windows; offset
  resolution failure falls back to UTC (named decision).
- No new persisted *state*, no schema changes; the `bucket_at` index and
  the compaction-predicate fix are access-path/query changes under the
  existing idempotent-migration pattern.

## Testing and proof

- T1 — rhythm & axis: histogram → boundaries (typical 9–18, night-owl,
  split-shift, all-hours, **short-active-day / Day-floor clamp**, tie runs,
  immature → defaults), maturity boundary (MIN_ACTIVE_DAYS −1 vs exact),
  mapper DST grouping (scripted offset table), local-day rollover, future
  `bucket_at` fail-awake, routed readers agree across midnight (status /
  watch / 7-day, per-source grouping preserved), superseded
  compacted-days test replaced, **compaction-vs-bucket_at retention test**
  (period_start >90d + bucket_at now), >500-row staged backlog
  convergence.
- T1 — sleep: onset window and re-arm symmetry, **cold-start catch-up
  through the real `stage_usage_poll_deltas` path** (one bounded wake,
  burst animations stay suppressed), live night burst wakes, night-owl
  cold-start one-wake, **hatch-at-night newborn stays awake**, petting
  while asleep (sleep pool, no wake, eyes stay closed), cursor eyes
  disabled, wander/facing held + eased boundaries, breath phase continuity
  at onset, calm survives a poll cycle in both frontends incl. first-poll
  establishment, milestone-effect exemption (evolution while asleep).
- T2: mote soft-saturation + budget cap + tidy fade, maturity gating,
  tiredness from active-bucket counting (lid-closed scenario; zero on
  light days), morning-after window idempotence + `None` vs `Some(0.0)`
  semantics, dream window determinism + no-signal → no dreams, sleep
  speech cadence and suppression, **speech precedence: hungry-at-dawn
  beats morning-after**.
- T3: prop-resonance qualification (earned-only, signal-matched, tie-break
  determinism, none-qualified → none, paused asleep), weekend_share
  mapping boundaries, climate modal/effective-weighted (mixed
  shape-detail week, ≥2 distinct climates from realistic mixes,
  shape-less → `None`).
- Preview Lab fixtures: `night-asleep`, `night-wake-catchup`,
  `dream-night`, `dawn-crossing`, `dawn-after-feast-day`,
  `heavy-day-evening`, `light-day-morning`, `midnight-mid-session`,
  `hatch-at-night`, `weekend-midday`, `climate-cache-week`,
  `prop-resonance-planter`; manifest inputs carry the `DayContext`.
- Whole-frame snapshots for wide/compact day and night scenes; menubar
  BMP-invariant test extended to the sleeping pet.
- TDD per house rules for all implementation work.

## Non-goals

- No streak counters, daily quotas, ETAs, or countdowns.
- No new moods (asleep is a presentation state) and no new authored art
  templates or invented trait mechanisms.
- No wall-clock/calendar-driven personality content (visual texture only).
- No new persisted semantic state; no schema changes (index + query fixes
  allowed as stated).
- No hemisphere/locale configuration; the season is texture, never a claim.
- No user-facing calm/scheduling config in this roadmap (still deferred).
- Panels-Branch-3 features stay out; this spec absorbs the canonical axis,
  reader routing, and the compaction retention fix — Branch 3 consumes the
  helper for best-day/intraday work (supersession note added there).
