# Glorp liveliness Branch 2 - live pet scene

- **Date:** 2026-06-05
- **Status:** approved design, ready for implementation planning
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

- raw applied usage delta from `RuntimeUpdate.recent_effective_tokens`
- recent source mix, such as Claude vs Codex contribution
- token-shape mix, such as cache-heavy, output-heavy, or reasoning-heavy
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
`burst_level` represents the leading edge of the most recent raw delta and can
decay faster than ambient activity.

## Data flow

The runtime path should carry raw applied usage into the watch app instead of
discarding it:

1. `poll_usage_and_apply` loads state, stages usage buckets, applies unapplied
   usage, and returns both updated `PetState` and the `RuntimeUpdate`.
2. `WatchApp` keeps an in-memory `LifeSignalState` across polls.
3. `LifeSignalState` updates EMA, burst, source mix, and token-shape summaries
   from the latest raw poll result.
4. `build_watch_view_model_at` or a nearby builder composes `PetLifeProfile`
   from `PetState`, usage-store summaries, and `LifeSignalState`.
5. `WatchViewModel` carries `life_profile`.
6. Render paths consume `life_profile` without reading raw usage stores.

The smeared `bucket_at` ledger remains the source of truth for historical
aggregates and panels. The live intensity signal uses raw applied poll deltas.
This avoids the failure mode from the earlier design review, where smeared rows
under-read bursts and saturated too easily.

## Live intensity

The first implementation task must prototype the normalization against a real
ccusage ledger before visual consumers are wired.

Requirements:

- raw poll delta maps to an instantaneous pace
- EMA smooths across the roughly 10 second poll interval
- idle decays gradually
- warm, hot, and very hot remain distinguishable
- first-week pets work even if calibration defaults are off by 10x to 100x
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

`burst_level` should come from the same raw poll delta, scaled relative to the
normalization reference. It replaces the old fixed `today_effective_tokens`
delta trigger for token-pop behavior.

The visual relationship:

- burst is the leading edge of a real usage event
- ambient activity is the decay tail
- one poll delta should read as one coherent reaction, not multiple unrelated
  effects
- midnight rollover cannot create a negative or phantom burst

`PetAnimator` should continue to handle short transition effects. Continuous
ambient liveliness should not keep the app in the fast 60 fps tick forever.

## Work-shape weather

Branch 2 should include token-shape weather because the data is already numeric
and privacy-preserving. Weather is presentation-only and should degrade to
`Clear` when token-shape detail is missing.

Initial mapping:

- cache-heavy sessions: soft mist, orbiting particles, or slow haze
- output-heavy sessions: brighter sparks, brief edge flashes, or sharper flecks
- reasoning-heavy sessions: deeper shimmer, slower pulses, or denser core glow
- mixed sessions: combine subtly, with a cap so the scene does not become noisy

Weather classification should use simple proportions and thresholds over recent
raw or recent ledger token buckets. It should not inspect prompt or response
content.

## Source accents

Branch 2 should also include lightweight source accents:

- Claude-dominant: one accent hue or prop family
- Codex-dominant: another accent hue or prop family
- balanced: dual accent

The source accent should be visible through the pet scene and earned props, not
through new panel rows. For example, a Codex signal lamp can glow during Codex
activity, and balanced activity can make two accents appear together.

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

The feed should continue to preserve token-added rows. Pet activity lines should
remain sparse and deterministic.

## Menubar

The menubar builds from the shared watch view model, so it should receive the
same `PetLifeProfile`. It does not need every visual effect from the TUI, but it
should use the same profile for any pet rerendering, brightness, or small accent
behavior it supports.

The menubar must continue to avoid revealing the next stage label.

## Preview Lab

Preview Lab must prove this feature, not merely the static layout.

Add deterministic profile fixtures such as:

- `watch-liveliness-s6-idle-dawn`
- `watch-liveliness-s6-warm-midday`
- `watch-liveliness-s6-hot-midday`
- `watch-liveliness-s6-cooling-evening`
- `watch-liveliness-compact-s6-hot`

The review contract:

- frames must differ visually beyond timestamps and text labels
- `.cells.json` should show changed glyphs/styles for activity, weather, or prop
  reactions
- compact frames must not overlap or crowd the pet scene
- the manifest should list profile inputs: activity level, burst level, source
  accent, weather, stage, species, and prop reactions

Use real render paths. Preview-only helpers may construct deterministic profiles,
but should not fork the rendering behavior.

## Testing

Use test-driven implementation for each seam.

Unit tests:

- `PetLifeProfile` construction handles idle, warm, hot, and cooling states
- live intensity EMA decays and does not saturate too early
- normalization handles zero, NaN, infinity, and tiny baselines
- burst mapping handles positive deltas and local-midnight rollover
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

Visual verification:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

## Implementation sequence

1. Add profile types and fixture defaults with no visual behavior change.
2. Change watch polling to return raw runtime updates and maintain
   `LifeSignalState`.
3. Prototype and test activity normalization against a real local ledger.
4. Build `PetLifeProfile` and put it on `WatchViewModel`.
5. Add Preview Lab idle/warm/hot/cooling fixtures before tuning visuals.
6. Add pet-panel aura, particle, brightness, and burst consumers.
7. Add source accents and work-shape weather.
8. Add prop reactions through `habitat_props_for`.
9. Repoint speech/feed selection to profile-derived signals.
10. Thread profile through menubar pet rerendering where applicable.
11. Regenerate snapshots, run focused tests, run preview, tune conservatively.

## Risks and guardrails

- **Signal saturation:** mitigate by measuring against a real ledger before
  wiring visuals.
- **Visual noise:** cap weather and prop reaction intensity; calm magical beats
  energetic flashing.
- **Renderer coupling:** keep `render_pet` unchanged and profile consumers in
  panel/component layers.
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
