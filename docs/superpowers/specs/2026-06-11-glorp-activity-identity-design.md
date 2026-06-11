# Glorp Activity Identity - design

- Date: 2026-06-11
- Status: direction approved by Drew; spec pending review
- Builds on:
  - `docs/superpowers/specs/2026-06-04-glorp-liveliness-design.md`
  - `docs/superpowers/specs/2026-06-05-glorp-liveliness-branch2-design.md`
  - `docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-alive-room-design.md`
  - `docs/superpowers/stories/story-001-usage-provider-ccusage.md`

## Problem

Glorp currently behaves like a Claude Code plus Codex pet even though the
underlying accounting layer has moved toward a broader coding-agent usage
model. The npm wrapper bundles `ccusage` and `@ccusage/codex`, the Rust
provider polls exactly two surfaces (`claude-code` and `codex`), and the
watch UI has Claude/Codex-specific source rows. This misses token usage from
other `ccusage`-reported sources and makes the product identity narrower than
the user's actual AI coding activity.

There is also a safety trap: `ccusage` 20.x changed bare `ccusage daily` into
an all-agent aggregate. Glorp correctly responded by scoping the Claude path to
`ccusage claude daily` so all-agent rows would not be mislabeled as Claude and
retro-fed into the pet. That guard must not be removed casually. The new design
must ingest all sources without corrupting source identity, cursor identity, or
pet personality.

The product goal is not a badge cabinet for harness brands. Drew's direction is
that Glorp should use the user's activity to map to a unique pet. Harnesses are
evidence; activity patterns are personality.

## Direction

Adopt an **Activity Identity** model:

- Unified `ccusage` sources feed the local usage ledger.
- Named source labels remain visible for trust, debugging, and accounting.
- Pet and room behavior derive from activity traits, not per-harness switches.
- Recent traits affect live presentation.
- Long-term traits can unlock durable habitat flavor or milestones.
- The ledger remains the source of truth.

The core rule:

> Harnesses feed the ledger. Activity patterns shape the pet.

## Goals

- Ingest token usage reported by modern `ccusage` for all supported local
  coding-agent sources, not just Claude and Codex.
- Preserve source identity so rows from different sources never collide or get
  mislabeled.
- Avoid retro-feeding historical totals when a new source first appears.
- Make all active sources visible in Today/status/feed/doctor surfaces.
- Replace Claude/Codex-only personality assumptions with derived activity
  traits: source diversity, rhythm, token shape, relative intensity, and
  recovery.
- Let recent activity traits shape presentation without inventing fake work.
- Let long-term activity traits unlock a small number of durable habitat or
  identity milestones.
- Keep privacy boundaries unchanged: no prompts, responses, tool payloads,
  project names, file paths, or transcript content.

## Non-goals

- No bespoke personality map for every named harness.
- No native parsing of Kimi/Gemini/OpenCode/Copilot/etc. logs outside
  `ccusage`.
- No large pet-state or identity schema rewrite in the first pass.
- No quotas, streaks, productivity score, leaderboard, or work-pressure UI.
- No claim that Glorp's cost display is billing-authoritative.
- No menubar redesign beyond consuming shared view-model state.
- No full visual polish pass for every new activity trait; this spec defines
  the contracts and enough rendering behavior to prove the concept.

## Product Principles

### Sources Are Evidence

The source label answers "where did this food come from?" It belongs in
diagnostics, source breakdowns, feed entries, and cursor/debug metadata.

The pet's identity answers "what kind of working life is this pet growing up
inside?" That should be based on real activity patterns, not tool branding.

### Activity Is Personality

The pet can become more unique because of:

- whether the user uses one source or many
- whether work arrives as bursts or steady flow
- whether token shape is cache-heavy, output-heavy, reasoning-heavy, or
  balanced
- whether today's activity is quiet, normal, heavy, or huge relative to the
  user's own baseline
- whether the user is returning after idle time or sustaining a rhythm

These are local, numeric, ledger-derived signals. They are better product
inputs than a harness-name switch.

### Visible Accounting, Poetic Behavior

The UI should be honest and literal about sources. The pet room can be poetic
about patterns. A feed line may say "gemini added 12k effective tokens"; the
room reaction should say "ensemble day" or "bursty output storm", not "Gemini
sparkles."

### Conservative Durability

Long-term traits may create durable unlocks, but the first pass should avoid a
new opaque temperament system. Prefer milestones that can be recomputed or
validated from the ledger, with only the existing habitat unlock state storing
the fact that the milestone has been awarded.

## Existing System Notes

- `src/usage/ccusage.rs` defines `CLAUDE_SURFACE` and `CODEX_SURFACE`, discovers
  two helper binaries, and polls two helper paths.
- `src/usage/ccusage.rs` scopes `ccusage >= 20` to `ccusage claude daily` on the
  Claude surface to avoid ingesting all-agent rows as Claude.
- `src/usage/normalize.rs` branches on `provider_surface == "codex"` and parses
  every other row as Claude-shaped.
- `src/usage/provider.rs` already carries `provider_surface`, `model`, and raw
  token totals through `UsageDelta`.
- `src/storage/usage_store.rs` stores `provider_surface`, `command`,
  `source_surface`, `model`, token buckets, cost metadata, parser version, and
  cursor metadata as generic values.
- `src/tui/panels/today.rs` hard-codes `claude-code` and `codex` rows.
- `src/game/runtime.rs` and `src/tui/life.rs` derive source accent only from
  Claude/Codex effective-token shares.
- `src/game/habitat.rs` has an existing named exception: the Codex signal lamp
  unlocks on first Codex usage.

The storage and runtime data plane is closer to generic than the ingestion and
presentation layers. The implementation should preserve the generic parts and
retire the hard-coded ingestion/UI assumptions.

## Source Identity

Add an explicit source identity concept at the provider boundary. Exact names
can change during implementation, but the boundary must exist.

```rust
pub struct SourceIdentity {
    pub provider_surface: String,
    pub display_name: String,
    pub raw_agent: Option<String>,
    pub source_family: SourceFamily,
}

pub enum SourceFamily {
    KnownCodingAgent,
    UnknownCodingAgent,
}
```

`provider_surface` is the stable storage key: lowercase, normalized, short,
and safe to display after truncation. Examples: `claude`, `codex`, `gemini`,
`kimi`, `opencode`, `copilot`, `unknown`.

`display_name` is the user-facing label. It may preserve capitalization or a
friendly spelling, but it must not be used as cursor identity.

`raw_agent` preserves the raw `ccusage` source/agent value when the JSON
provides one, but only if it is already a source label. It must never store
paths, project names, prompts, or transcript content.

`source_family` is intentionally coarse. It exists for validation and fallback,
not for pet behavior. Pet behavior consumes activity traits.

## ccusage Ingestion

Modern Glorp should prefer unified `ccusage daily --json --offline --order asc`
when the installed `ccusage` version and output shape support source-aware
rows.

Focused commands remain compatibility tools:

- `ccusage claude daily --json --offline --order asc`
- `ccusage codex daily --json --offline --order asc`, if available
- `ccusage-codex daily --json --offline`, as a legacy fallback

The old two-helper model should become fallback behavior, not the product's
primary model. The npm wrapper can continue exposing bundled helper paths while
the implementation migrates dependencies, but the Rust provider must be able to
represent unified output as multiple source identities.

### Row Identity Rules

- Never assign all rows from `ccusage daily` to `claude-code`.
- Each normalized record gets `provider_surface` from row source metadata when
  present.
- If a row has `agent: "all"` plus per-model or per-source breakdowns, Glorp
  must use the breakdown identity when available and must not create one giant
  `all` source that hides source-specific cursors.
- If a row has only aggregate all-source totals and no recoverable per-source
  identity, the row must be ignored with a structured diagnostic. It is not safe
  to feed aggregate all-source history as one source because it can collide with
  focused rows and destroy source diversity signals.
- Unknown source labels with enough token bucket detail are valid. They feed
  neutrally and participate in diversity traits.

### Token Bucket Rules

Normalization should choose bucket semantics from row shape, not from a
hard-coded `provider_surface == "codex"` branch.

Supported shapes:

- Claude-style rows: `inputTokens`, `outputTokens`, `cacheCreationTokens`,
  `cacheReadTokens`, optional `reasoningOutputTokens`.
- Codex/OpenAI-style rows: `inputTokens`, `outputTokens`,
  `cachedInputTokens`, optional `cacheCreationTokens`, optional
  `reasoningOutputTokens`; uncached input is `inputTokens - cachedInputTokens`.
- Future rows that contain an unambiguous equivalent set of fields can be
  normalized after an explicit fixture and test.

Malformed rows become diagnostics. They must not panic, silently feed zero, or
store raw payload fragments.

### Cursor Identity

Cursor identity must include the normalized source identity. The current
`ProviderCursorKey` shape already has `provider_surface`, `command`,
`source_surface`, `period_start`, and `model`; the implementation must ensure
that `provider_surface` is the row's source, not the helper invocation surface.

Minimum cursor key identity:

- `provider_surface`
- `command`
- `source_surface`
- `period_start`
- `model`

If unified `ccusage` exposes an additional stable source/harness key that can
distinguish rows with the same source label and model, the implementation may
add it to the cursor key. Do not add raw paths or project/session identifiers.

Cursor metadata (`provider_version`, `parser_version`) remains the helper
version and parser version. Data cursors still advance only after staged ledger
rows are applied and pet state is saved.

## First Contact and History Safety

Newly discovered sources must not retro-feed historical totals into the pet.
This is the same product rule as init calibration: historical usage calibrates
the baseline but does not grant initial XP.

For a source with no existing data cursor:

1. Normalize valid rows.
2. Write source cursors for existing totals.
3. Include rows in calibration/history reads where appropriate.
4. Emit no feeding deltas for those historical totals.
5. Record a source-first-contact diagnostic or event only if the UI needs to
   explain why a source appeared without food.

After first contact, positive deltas since the stored cursor feed normally.

This rule must apply both to a brand-new Glorp install and to an existing Glorp
user upgrading into unified `ccusage` support. It prevents "I upgraded Glorp
and my pet ate six months of Gemini history" behavior.

The existing discontinuity guard remains a second line of defense for unusually
large deltas after cursors exist. It should not be the primary first-contact
mechanism.

## Activity Identity Profile

Add a derived profile beside `PetLifeProfile` and `DayContext`. Exact type
names can change, but the profile should be compact, value-like, and built from
ledger aggregates.

```rust
pub struct ActivityIdentityProfile {
    pub source_diversity: SourceDiversity,
    pub rhythm: WorkRhythm,
    pub token_shape: TokenShapePersonality,
    pub relative_intensity: RelativeIntensity,
    pub recovery: RecoveryPattern,
    pub long_term_milestones: Vec<ActivityMilestone>,
}
```

The profile should be computed once per view-model build or poll update, not
per frame.

### Source Diversity

Derived from effective-token share by normalized source over a local-day or
rolling window.

Initial classes:

- `SingleLane`: one source contributes at least 85% of effective tokens.
- `DualLane`: two sources each contribute at least 20%, and together at least
  80%.
- `Ensemble`: at least three sources each contribute at least 10%.
- `Quiet`: not enough effective tokens to classify.

Source diversity drives "solo focus" versus "ensemble" room/pet flavor, not
named harness effects.

### Work Rhythm

Derived from bucketed applied usage over the same local-day axis used by
`DayContext`.

Initial classes:

- `Steady`: activity appears in several buckets with no single bucket
  dominating.
- `Bursty`: one or two buckets dominate the recent window.
- `Sporadic`: activity exists, but separated by long gaps.
- `Returning`: current activity follows an idle gap longer than the recovery
  threshold.
- `Quiet`: no meaningful recent activity.

Work rhythm can influence pet movement, animation tempo, and room ambience.

### Token Shape Personality

Derived from stored token buckets, using effective weighting where needed so
raw cache-read volume does not dominate every classification.

Initial classes:

- `CacheHeavy`
- `OutputHeavy`
- `ReasoningHeavy`
- `Balanced`
- `UnknownShape`

`UnknownShape` is valid for older rows or helpers without bucket detail. It
feeds the pet but does not fabricate a weather/personality signal.

### Relative Intensity

Derived from today's applied effective tokens divided by
`CalibrationBaseline.daily_effective_tokens`.

Initial classes:

- `Quiet`: below 25% baseline
- `Normal`: 25% to 125%
- `Heavy`: 125% to 300%
- `Huge`: above 300%

The exact thresholds can be constants, but they must be baseline-relative, not
absolute-token thresholds.

### Recovery Pattern

Derived from recent applied rows and prior idle duration.

Initial classes:

- `Sustained`: recent activity continues an existing active period.
- `Returned`: recent activity follows a meaningful idle gap.
- `Fading`: no recent activity, but the pet has recent historical activity.
- `Dormant`: no recent activity and no meaningful recent history.

Recovery should remain gentle. It can influence pet posture and habitat flavor;
it must not shame the user for absence.

## Presentation Behavior

Recent activity traits affect live presentation:

- `SourceDiversity` can choose solo, dual, or ensemble accent density.
- `WorkRhythm` can tune movement cadence and ambient texture.
- `TokenShapePersonality` can feed the existing weather/room vocabulary.
- `RelativeIntensity` can scale activity level within existing liveliness
  limits.
- `RecoveryPattern` can drive gentle return or rest states.

These traits should flow into existing presentation layers rather than becoming
ad hoc render flags. The likely destination is a new profile carried on
`WatchViewModel`, consumed by room/pet panels and optionally by menubar render.

The first implementation only needs enough rendering to prove the profile is
used. It does not need to author a complete visual language for every trait
combination. Prefer clear, testable changes:

- dynamic source rows
- source-stable feed coloring
- an ensemble/solo accent distinction
- one or two habitat reactions or milestones

## Durable Milestones

Long-term traits can unlock durable habitat or identity flavor. These should
use existing habitat unlock machinery where possible.

Initial milestone candidates:

- `first_ensemble_day`: at least three sources each contribute at least 10% of
  a local day's effective tokens.
- `steady_week`: at least five active local days in seven, with activity spread
  across multiple buckets.
- `cache_craft`: repeated cache-heavy days over a rolling window.
- `return_sprout`: meaningful activity after a multi-day dormant period.

Milestones must be derived from the ledger and personal baseline. They must not
be named after harnesses. The existing `codex_signal_lamp` may remain as a
legacy named exception, but new milestones should be activity-pattern based.

If the implementation needs persisted unlocks, store only the awarded prop or
milestone event through existing habitat state patterns. Do not persist a
mutable personality scorecard.

## UI Surfaces

### Today Panel

Replace hard-coded Claude/Codex source rows with dynamic rows.

Expected behavior:

- Always show total effective tokens.
- Show top sources by today's effective tokens, stable sorted by descending
  tokens then source name.
- Use an `other` row when there are more active sources than fit.
- Preserve diagnostic markers for blocked or diagnostic sources.
- Keep text width bounded; source names must truncate or compact safely.

The panel height can stay fixed in the first pass if it uses top-N plus
`other`. A later visual polish pass can choose more responsive layout.

### Feed

Usage feed entries should keep source labels literal:

```text
gemini added 12k effective tokens
opencode added 8k effective tokens
```

Color should come from a stable palette function keyed by normalized source
name, with named colors preserved for existing Claude/Codex if desired. Unknown
sources get deterministic colors, not all the same fallback.

### Status and Doctor

Status should report aggregate health and recent usage without assuming two
sources. Doctor should list helper versions, source diagnostics, and recent
source health generically.

Missing a specific optional source is not an error. A source can only be
diagnostic after it appears in `ccusage` output or a helper reports a parsing
problem for it.

### Menubar

Menubar consumes shared view-model state. This spec does not require a menubar
redesign.

## Privacy and Security

Glorp stores only normalized numeric and coarse identity metadata:

- source label
- display label
- model when present
- date/period
- token buckets
- effective tokens
- display-only cost metadata
- helper/parser version
- derived activity traits

Glorp must not store:

- prompts
- responses
- tool payloads
- transcript rows
- file paths
- project names
- session identifiers
- raw helper JSON payloads

Diagnostics must stay sanitized. Error messages should name the source and the
missing/invalid field category, not echo raw helper output.

## Compatibility With ccusage Drift

`ccusage` is an external parser layer and can drift as harnesses change. Glorp
should trust `ccusage` for source discovery and parsing rather than chasing
each vendor's log format.

When `ccusage` misses a source because its upstream parser lags a vendor
migration, Glorp should surface the helper reality: no source rows were
reported, or a source row was malformed. It should not add a Glorp-native parser
for that vendor in this pass.

Version probing should continue to be sanitized and bounded. Helper subprocess
timeouts remain required.

## Testing Requirements

Provider tests:

- unified multi-source fixture emits deltas for every source identity
- same day plus same model across two sources creates distinct cursor keys
- first contact with a new source seeds cursors without feeding historical
  totals
- repeated poll after cursor advance emits zero new deltas
- valid unknown source with known token fields feeds neutrally
- malformed source row emits a structured diagnostic and no delta
- `date` and `period` fields both parse
- legacy `ccusage-codex` fallback still works while supported

Runtime/storage tests:

- arbitrary source deltas stage, apply, mark cursors, and update lifetime
  counters
- first-contact source rows do not unlock new activity milestones
- unknown source does not unlock `codex_signal_lamp`
- source diversity trait classifies single-lane, dual-lane, ensemble, and quiet
  windows
- token shape trait handles cache-heavy, output-heavy, reasoning-heavy,
  balanced, and unknown rows

UI/render tests:

- Today panel renders more than two sources via top-N plus `other`
- long source names do not overflow their row
- feed coloring is deterministic for arbitrary source names
- status/doctor output does not assume Claude/Codex-only helper health

Preview Lab tests:

- at least one watch scenario includes three active sources and an ensemble
  activity profile
- at least one scenario includes an unknown source that feeds neutrally
- generated preview manifest records the source mix and activity profile intent
  without raw payloads

## Rollout Plan

Implementation should land in task-sized commits:

1. Provider fixtures and source identity normalization.
2. Source-safe cursor and first-contact behavior.
3. Dynamic source UI rows and deterministic source palette.
4. Activity identity profile derivation.
5. Presentation hooks and minimal milestone unlocks.
6. Docs, preview fixtures, and release/package wiring.

The implementation plan should decide whether to keep `@ccusage/codex` as a
packaged dependency for one release as a fallback. Removing it is allowed only
after tests prove modern `ccusage` covers Codex rows well enough for installed
users.

## Open Decisions For Implementation Planning

- Whether to add an explicit field to `ProviderCursorKey` for raw source id, or
  fold all stable identity into `provider_surface`.
- Whether `SourceIdentity` should live in `src/usage/provider.rs` or a new
  focused module under `src/usage/`.
- How many dynamic source rows the Today panel can fit before using `other`.
- Which one or two durable activity milestones should ship first.

These decisions are implementation-shaping details, not product ambiguities.
The product direction is fixed: all `ccusage` sources feed the ledger, and
activity patterns shape the pet.
