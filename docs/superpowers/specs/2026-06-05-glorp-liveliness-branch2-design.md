# Glorp liveliness Branch 2 - live pet scene

- **Date:** 2026-06-05
- **Status:** approved design, patched after staff-SWE review, ready for implementation planning
- **Linear:** PRI-2072
- **Scope:** make high-stage Glorp pets visibly respond to real work sessions through a derived live-scene profile

## Problem

Branch 1 fixed the correctness issues from the original liveliness design: stage
progress is stage-relative, recent activity speech no longer keys off
`observed_at`, stale progress fields are gone, and the menubar no longer reveals
the next stage.

The remaining product problem is still the one Drew feels in daily use: high
level pets look effectively the same all day. Existing motion is mostly driven
by `(species, stage, seed, wall-clock)`. Breath, wander, shimmer, twinkle,
ambient glyphs, and habitat prop phases make the scene move, but they do not
know whether the user is idle, warming up, or in a heavy coding sprint. At high
stages, the large body templates are intentionally stable, so wall-clock-only
motion reads as sameness.

## Goals

- Make the live pet scene respond to real work intensity during the current
  session.
- Preserve Glorp's calm, nurturing tone. Hot sessions should feel alive and
  magical, not like a flashing dashboard.
- Support source accents, token-shape weather, reactive earned props,
  activity-aware speech, and activity-aware feed lines in the same branch.
- Ship through independently reviewable sub-slices so the data contract lands
  before renderer behavior depends on it.
- Keep Branch 3 panel/day aggregate work out of this slice.
- Keep all new life cues local, derived, numeric, and presentation-only.
- Keep `render_pet` content-agnostic.
- Extend Preview Lab so idle, warm, hot, and cooling high-stage states can be
  reviewed deterministically.

## Non-goals

- No new persisted pet-state schema for animation tails or live-scene state.
- No transcript, prompt, response, tool-call, or source-file content storage.
- No fake feed controls or demo-only activity in the real app.
- No ETAs, countdowns, streak pressure, or grind-oriented UI.
- No Branch 3 panel work: intraday sparkline, best-day highlight, intra-stage
  pips, canonical day-axis query, and day aggregate fixes remain separate.
- No new low-level pet template rewrite. Species silhouettes can stay stable.

## Design summary

Add a small derived `PetLifeProfile` layer between the watch/runtime data and
presentation code.

`PetLifeProfile` is built from real numeric signals:

- an explicit deduped `AppliedUsageSignal`, not
  `RuntimeUpdate.recent_effective_tokens` alone
- recent applied source mix, such as Claude vs Codex contribution, when
  available
- token-shape mix, such as cache-heavy, output-heavy, or reasoning-heavy, only
  when the provider/runtime path actually carries bucket detail
- mood, energy, stage, species
- earned habitat props
- idle duration and recent activity

The profile becomes the single presentation contract for "how alive should the
scene feel right now?" It is copied onto `WatchViewModel` so every frontend and
Preview Lab fixture can consume the same derived state.

## Proposed profile shape

Exact type names can evolve during implementation, but the design should keep
one compact profile object instead of threading ad hoc signal fields through
each renderer.

```rust
pub struct PetLifeProfile {
    pub activity_level: f32,
    pub burst_level: f32,
    pub source_accent: Option<SourceAccent>,
    pub work_weather: WorkWeather,
    pub prop_reactions: Vec<PropReaction>,
    pub idle: IdleLifeState,
    pub calm_mode: bool,
}

pub enum SourceAccent {
    Claude,
    Codex,
    Balanced,
}

pub enum WorkWeather {
    Clear,
    CacheMist,
    OutputSparks,
    ReasoningPulse,
    Mixed,
}

pub struct PropReaction {
    pub prop_id: HabitatPropId,
    pub intensity: f32,
    pub kind: PropReactionKind,
}

pub enum PropReactionKind {
    Glow,
    Bloom,
    Pulse,
    Orbit,
}

pub struct IdleLifeState {
    pub idle_minutes: u32,
    pub is_recently_active: bool,
}
```

All floating-point fields use finite clamped ranges. `activity_level` should be
able to distinguish idle, warm, hot, and very hot rather than peg at one value.
`burst_level` represents the leading edge of the most recent live applied
signal and can decay faster than ambient activity.

## Applied usage signal contract

The first implementation slice must add and test an explicit live signal
contract. The current `RuntimeUpdate` is not sufficient: it carries only a
recent applied effective-token sum plus applied IDs, and that sum is derived
after catchup smearing. The profile builder must not treat it as raw live pace.

The contract can be named differently during implementation, but it should
carry this information:

```rust
pub struct AppliedUsageSignal {
    pub applied_effective_tokens: f64,
    pub raw_effective_tokens: Option<f64>,
    pub source_mix: Option<AppliedSourceMix>,
    pub token_shape: Option<TokenShapeDelta>,
    pub observed_at: DateTime<Utc>,
    pub elapsed_since_successful_poll: Duration,
    pub freshness: UsageSignalFreshness,
}

pub struct AppliedSourceMix {
    pub claude_effective_tokens: f64,
    pub codex_effective_tokens: f64,
}

pub struct TokenShapeDelta {
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_creation_tokens: f64,
    pub cache_read_tokens: f64,
    pub reasoning_output_tokens: f64,
}

pub enum UsageSignalFreshness {
    Live,
    ColdStart,
    Backfill,
    DiagnosticsOnly,
}
```

Rules:

- The signal is built only from deltas that were actually applied or reflected
  after dedupe. Consuming provider deltas directly can double-fire visuals when
  cursor save and ledger insertion disagree.
- `applied_effective_tokens` is the amount that changed pet state.
  `raw_effective_tokens` is the provider delta before catchup distribution when
  that value is known. Unknown raw provider pace is `None`; do not fake `0.0`
  or copy the applied value just to satisfy the type.
- `elapsed_since_successful_poll` is measured from the real poll cadence. Do
  not assume a 10 second interval when computing pace.
- First poll, app restart, delayed helper output, diagnostics-only output, and
  large backfills must set a non-`Live` freshness and suppress burst effects.
- `TokenShapeDelta` requires extending the provider/runtime path. If the
  current provider cannot supply bucket fields, `token_shape` is `None` and
  `work_weather` degrades to `Clear`. Missing token-shape detail is not a
  freshness state and must not suppress otherwise valid live activity or burst.
- Missing source detail is represented by `source_mix: None`; it disables source
  accents but does not suppress activity or burst.
- Freshness classification happens before or alongside catchup distribution.
  Do not infer `Backfill` from `bucket_count > 1`, old `bucket_at`, or a daily
  provider `period_start`, because normal live deltas can smear into multiple
  buckets. Use poll/session facts instead: whether a prior cursor existed,
  whether this is the first poll in the app session, elapsed time since the last
  successful poll, raw delta size when known, diagnostics presence, and whether
  rows actually changed pet state versus were already reflected.
- Session-only freshness facts can be finalized in `LifeSignalState::observe`
  or the poll owner before burst/profile updates. `poll_usage_and_apply` does
  not need to know every app-session fact by itself.
- If implementation starts populating token bucket columns in smeared ledger
  rows, bucket values must be apportioned across smear buckets. Do not duplicate
  the raw token totals into every bucket; daily aggregates sum those columns.
- The contract is numeric/enum-only. It must not expose model names, command
  strings, cursor keys, provider delta IDs, file paths, prompts, responses, or
  provider diagnostic payloads.

## Data flow

The runtime path should carry a deduped applied usage signal into the watch app
instead of discarding it:

1. `poll_usage_and_apply` loads state, stages usage buckets, applies unapplied
   usage, and returns updated `PetState`, existing runtime update fields, and
   `AppliedUsageSignal`.
2. `WatchUsagePoller` returns a poll envelope such as `{ vm, pet_state,
   applied_signal }`, not only `WatchViewModel`.
3. `WatchApp` owns an in-memory `LifeSignalState` across polls and observes the
   applied signal after each completed poll.
4. `WatchApp` stamps the resulting `PetLifeProfile` onto the new
   `WatchViewModel` before swapping it into the UI.
5. Menubar follows the same ownership rule in its own app loop: its worker
   returns the poll envelope, and the menubar loop owns its own
   `LifeSignalState`.
6. Render paths consume `life_profile` without reading raw usage stores.

The smeared `bucket_at` ledger remains the source of truth for historical
aggregates and panels. The live intensity signal uses the explicit applied
signal, which can distinguish raw provider pace, applied pet-state change, and
freshness. This avoids both failure modes from the reviews: smeared rows
under-reading bursts, and raw provider deltas double-firing after dedupe.

## Live intensity

The first implementation task after the signal contract must prototype the
normalization against a real ccusage ledger before visual consumers are wired.
Commit a short measurement note or test fixture explaining the chosen constants
and why idle, warm, hot, and very hot do not all collapse into the same range.

Requirements:

- live applied signal maps to an instantaneous pace
- EMA smooths across the actual poll interval, nominally around 10 seconds
- actual elapsed poll time is used when it differs from the nominal interval
- idle decays gradually
- warm, hot, and very hot remain distinguishable
- first-week pets work even if calibration defaults are off by 10x to 100x
- cold-start and backfill signals do not create burst
- non-finite values clamp to safe defaults
- zero divisors are floored with `.max(1.0)` or equivalent

Candidate normalization:

- keep an in-memory rolling reference pace from recent poll rates
- map current pace against that reference through a graduated saturating curve
- clamp display-facing `activity_level` to a finite range, for example
  `0.0..=2.0`

The exact constants should be named, tested, and tuned after measurement. The
design requirement is useful middle range, not a particular formula.

## Burst response

`burst_level` should come from the same applied signal, scaled relative to the
normalization reference. It replaces the old fixed `today_effective_tokens`
delta trigger for token-pop behavior.

The visual relationship:

- burst is the leading edge of a real usage event
- ambient activity is the decay tail
- one poll delta should read as one coherent reaction, not multiple unrelated
  effects
- midnight rollover cannot create a negative or phantom burst
- cold-start, backfill, and diagnostics-only signals do not trigger burst
- missing token-shape or source detail alone does not suppress burst

`PetAnimator` should continue to handle short transition effects. Continuous
ambient liveliness should not keep the app in the fast 60 fps tick forever.

## Work-shape weather

Branch 2 should include token-shape weather only after token-shape detail is
available in the applied signal. Weather is presentation-only and should degrade
to `Clear` when token-shape detail is missing.

Initial mapping:

- cache-heavy sessions: soft mist, orbiting particles, or slow haze
- output-heavy sessions: brighter sparks, brief edge flashes, or sharper flecks
- reasoning-heavy sessions: deeper shimmer, slower pulses, or denser core glow
- mixed sessions: combine subtly, with a cap so the scene does not become noisy

Weather classification should use simple proportions and thresholds over
applied/deduped token buckets. It should not inspect prompt or response
content. Reasoning-heavy weather, if present, must use raw bucket proportions;
effective-token deltas alone are not enough because reasoning output is not
part of effective-token weighting today.

## Source accents

Branch 2 should also include lightweight source accents:

- Claude-dominant: one accent hue or prop family
- Codex-dominant: another accent hue or prop family
- balanced: dual accent

The source accent should be visible through the pet scene and earned props, not
through new panel rows. For example, a Codex signal lamp can glow during Codex
activity, and balanced activity can make two accents appear together.

Source accents must be computed from applied/deduped signal data, not raw
provider deltas alone. If the signal freshness is not `Live`, the accent should
either hold its previous decaying state or degrade to no accent; it should not
fire a new burst.

If source mix is missing, the profile uses no source accent.

## Reactive earned props

Earned habitat props should become instruments in the live scene. This gives
high-level pets more day-to-day variation without inventing new rewards.

Initial reactions:

- Codex signal lamp glows with Codex activity
- heavy-session planter blooms or brightens after large bursts
- lifetime-token props gain richer glow at higher stages under activity
- wilt-recovery or recovery-themed props brighten during recovery states

Prop reactions should be computed as `PropReaction` entries in the profile and
consumed by `habitat_props_for`. The prop catalog remains the source of prop
identity and base rendering. Reactions only adjust glyph, color, brightness, or
small phase behavior.

Constraints:

- reactions target earned props only
- reactions target visible props only in compact layouts
- reactions may not promote hidden earned props into compact frames
- `Orbit` may add cells only in wide Truecolor layouts with explicit cell caps
- Flat/reduced-motion modes clamp reaction intensity before rendering

## Pet panel behavior

`PetPanel` consumes `PetLifeProfile` for all continuous scene expression.

Effects:

- aura or particle density scales by `activity_level` and stage
- body brightness gets a gentle lift from `activity_level`
- shimmer/twinkle can be enriched by weather and burst
- token pop scales by `burst_level`
- low energy droop still matters and composes with activity brightness

Composition order:

1. base semantic pet styles
2. low-energy droop
3. activity brightness lift
4. shimmer, burst, or weather-specific role highlight
5. final RGB clamp

Flat or low-color modes should suppress or simplify ambient glyphs, weather, and
brightness effects. A calm/reduced-motion configuration can clamp
`activity_level`, `burst_level`, and prop reaction intensity before rendering.
Branch 2 does not need to add a new persisted reduced-motion preference.
Preview Lab calm-mode fixtures should set `PetLifeProfile.calm_mode = true`
directly. A future app-level reduced-motion setting can thread through
`RenderContext`, but that is not required for this slice.

`render_pet` remains content-agnostic and does not receive `PetLifeProfile`.
Profile effects live in `PetPanel`, `habitat_props_for`, speech/feed selection,
and menubar style mapping only. `rerender_pet_for_view_model` should continue
to rerender semantic pet art without live usage details.

Compact S6 guardrails:

- no new rows
- no cells outside the pet/habitat region
- no text dependency, because compact layouts may hide speech
- cap added live glyphs by profile, with a stricter cap for hot S6 compact
- body brightness or style changes are preferred over extra particles when
  space is tight

Effect ownership table:

| Effect | Data source | Owner layer | Flat behavior | Reduced-motion behavior | Compact cap | Menubar behavior |
| --- | --- | --- | --- | --- | --- | --- |
| Activity brightness | `PetLifeProfile.activity_level` | `PetPanel` style mapping | simplify to role color only | clamp level | no new cells | poll-bound accent only |
| Burst token pop | `PetLifeProfile.burst_level` derived from `Live` signal | `PetAnimator` trigger plus `PetPanel` rendering | disabled | disabled or one pulse | no extra rows | none |
| Weather particles | `PetLifeProfile.work_weather` | `PetPanel` ambient glyph layer | degrade to `Clear` | lower density | max added glyph count | none |
| Source accent | `PetLifeProfile.source_accent` | `PetPanel` and visible earned props | role color only | decay only | no hidden prop promotion | poll-bound color/accent |
| Prop reactions | `PropReaction` | `habitat_props_for` | disabled or static glow | clamp intensity | visible props only | none unless already rendered |
| Speech/feed flavor | `PetLifeProfile` | `speech.rs`, `activity.rs` | unchanged text | unchanged text | text may be hidden | no extra text |

## Speech and feed

Branch 2 should unify the two flavor systems around the profile:

- speech bubble in `speech.rs`
- pet activity feed in `activity.rs`

Activity-aware text is allowed only as presentation selected by real signals.

Rules:

- active lines key off `activity_level`, `burst_level`, source accent, or weather
- idle lines key off mood, energy, and idle duration
- no wall-clock-only idle personality rotation during genuine idle
- no fake activity lines when there was no activity
- source/weather text should stay understated
- line selection is deterministic from profile fields, pet identity, and bucket
  time; not from the current render frame
- sparse caps prevent repeated pet-flavor rows from crowding token-added rows

The feed should continue to preserve token-added rows. Pet activity lines should
remain sparse and deterministic.

## Menubar

The menubar receives the same `PetLifeProfile` fields, but it does not need
every visual effect from the TUI. Scope Branch 2 menubar work to poll-bound
brightness or accent behavior unless implementation adds an explicit rendered
block signature that changes when profile-only style changes should repaint.

Menubar constraints:

- preserve the BMP/char-length invariant
- avoid extra glyphs unless the rendered block diff is explicit and tested
- do not animate per tick solely from style changes
- continue to avoid revealing the next stage label

## Branch 2 sub-slices

Keep the live-scene branch cohesive, but split implementation and review into
bounded sub-slices:

1. signal/data contract: `AppliedUsageSignal`, provider token buckets when
   available, dedupe/backfill classification, and tests
2. `LifeSignalState`: normalization constants, real-ledger measurement note,
   EMA, burst suppression, and deterministic unit tests
3. Preview Lab proof: profile fixtures and targeted `.cells.json` assertions
4. pet-panel baseline: brightness, capped particles, and burst token-pop
5. source/weather/prop reactions: only after the data contract proves the
   required fields exist
6. speech/feed and menubar: profile-driven flavor and constrained menubar
   accenting

## Preview Lab

Preview Lab must prove this feature, not merely the static layout.

Add deterministic profile fixtures such as:

- `watch-liveliness-s6-idle-dawn`
- `watch-liveliness-s6-warm-midday`
- `watch-liveliness-s6-hot-midday`
- `watch-liveliness-s6-cooling-evening`
- `watch-liveliness-compact-s6-hot`
- `watch-liveliness-flat-s6-hot`
- `watch-liveliness-calm-mode-s6-hot`

The review contract:

- frames must differ visually beyond timestamps and text labels
- `.cells.json` should show changed glyphs/styles for activity, weather, or prop
  reactions
- targeted assertions should compare pet/habitat cells while excluding clock,
  feed, and other timestamp/text regions
- compact frames must not overlap or crowd the pet scene
- compact changed cells stay inside the pet/habitat region
- wide glyph continuations are not introduced
- the manifest should list profile inputs: activity level, burst level, source
  accent, weather, stage, species, prop reactions, color capability, calm mode,
  and freshness

Use real render paths. Preview-only helpers may construct deterministic profiles,
but should not fork the rendering behavior.

## Testing

Use test-driven implementation for each seam.

Unit tests:

- `AppliedUsageSignal` is derived from applied/deduped usage, not raw provider
  deltas that were ignored
- signal freshness suppresses cold-start, backfill, diagnostics-only, and
  delayed-helper bursts
- missing token-shape or source detail does not suppress otherwise live burst
- token bucket propagation either preserves real bucket detail or explicitly
  yields missing detail
- populated smeared ledger token buckets do not overcount daily aggregates
- `PetLifeProfile` construction handles idle, warm, hot, and cooling states
- live intensity EMA decays and does not saturate too early
- normalization handles zero, NaN, infinity, and tiny baselines
- burst mapping handles positive deltas, local-midnight rollover, and non-live
  freshness
- source accent classifier handles Claude, Codex, balanced, and missing data
- work-weather classifier handles cache-heavy, output-heavy, reasoning-heavy,
  mixed, and missing detail
- prop reactions only target earned props
- profile-to-particle and profile-to-brightness functions are deterministic
- speech/feed lines are selected from real profile signals

Preview and integration tests:

- `cargo test --test dev_preview`
- `cargo test dev_preview::scenarios`
- `cargo test dev_preview::export`
- any focused watch integration tests touched by the view-model changes
- regenerate affected snapshots in the same branch that changes rendered frames
- targeted `.cells.json` assertions for idle vs hot S6, compact S6 hot, Flat S6
  hot, and calm-mode S6 hot

Visual verification:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

## Implementation sequence

1. Add `AppliedUsageSignal` and token/source detail propagation or explicit
   absent-detail behavior, with no visual behavior change.
2. Change watch and menubar polling to return poll envelopes containing
   `applied_signal` plus the existing view model data.
3. Add `LifeSignalState::observe(signal, now)` with injected time, named
   constants, burst suppression, and deterministic tests.
4. Prototype and test activity normalization against a real local ledger; save
   the measurement note or fixture in the branch.
5. Add profile types and fixture defaults with no visual behavior change.
6. Build `PetLifeProfile`, stamp it onto `WatchViewModel`, and keep
   `render_pet` unchanged.
7. Add Preview Lab idle/warm/hot/cooling, compact, Flat, and calm-mode fixture
   scaffolding plus failing/targeted assertions before tuning visuals.
8. Add pet-panel aura, particle, brightness, and burst consumers with compact
   caps, then satisfy the Preview Lab visual-diff assertions.
9. Add source accents and work-shape weather only where the applied signal has
   enough detail.
10. Add prop reactions through `habitat_props_for`.
11. Repoint speech/feed selection to profile-derived signals.
12. Thread constrained profile behavior through menubar rendering where
    applicable.
13. Regenerate snapshots, run focused tests, run preview, tune conservatively.

## Risks and guardrails

- **Signal saturation:** mitigate by measuring against a real ledger before
  wiring visuals.
- **Fake bursts:** classify cold start, diagnostics-only periods, delayed
  helper output, and backfill before updating burst state.
- **Data contract drift:** weather/source effects degrade when applied/deduped
  signal detail is missing.
- **Visual noise:** cap weather and prop reaction intensity; calm magical beats
  energetic flashing.
- **Renderer coupling:** keep `render_pet` unchanged and profile consumers in
  panel/component layers.
- **Compact crowding:** prove compact S6 hot with targeted Preview Lab cell
  assertions before accepting the visual slice.
- **Aggregate overcount:** apportion token-shape columns across smear buckets if
  those columns start being populated.
- **Fast-tick cost:** continuous ambient effects must not force permanent 60 fps.
- **Privacy drift:** profile fields stay numeric and derived from existing local
  usage metadata only.
- **Preview divergence:** preview constructs deterministic profiles but uses the
  same render paths as the live TUI.

## Open follow-ons

- Branch 3 panel/day aggregate work: local-day axis, intraday sparkline,
  best-prior-day highlight, intra-stage pips, and always-show-pace decisions.
- Additional high-level habitat unlocks beyond the current catalog.
- Periodic calibration rework. That touches saved-state semantics and should
  stay separate.
