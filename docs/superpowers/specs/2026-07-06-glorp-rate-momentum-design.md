# Glorp rate momentum - design

- Date: 2026-07-06
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-06-19-glorp-tokenmaxxing-token-contract-design.md`
  - `docs/superpowers/specs/2026-06-24-glorp-companion-tank-redesign-design.md`
  - `docs/superpowers/specs/2026-06-15-glorp-presentation-architecture-design.md`

## Problem

Glorp's token count has become a rich product surface: it has source rows,
recent buckets, seven-day history, and consistent Tokenmaxxing semantics. The
rate display is still thin by comparison. In the TUI it is a small inline
`/hr` segment attached to the progress bar; in the companion it is one small
`/hr` subline below the large token total.

Drew wants rate to explain momentum at a glance: green when activity is
increasing, red when it is decreasing, and a neutral state when activity is
flat. The rate should feel as intentional as the token count, while preserving
the companion's calm round display.

## Direction

Make rate a first-class `RateMomentum` model derived from canonical
Tokenmaxxing totals.

The product surface has two windows:

- `pulse`: current 10 minutes compared with the previous 10 minutes
- `hour`: current 60 minutes compared with the previous 60 minutes

The watch TUI shows both windows with explicit direction. The companion shows
the same two current values, but uses a color-only overall state so the round
HUD stays compact.

## Goals

- Show short-term and hourly momentum, not just a single rate-per-hour number.
- Use green/up, red/down, and neutral states in the TUI.
- Use color-only up/down/neutral treatment in the companion; no arrow, labels,
  or prose captions in the ring gap.
- Keep all values based on canonical Tokenmaxxing totals, matching the existing
  watch token contract.
- Keep the model shared so TUI, companion, preview, and tests read the same
  derived momentum state.
- Preserve the companion's current visual hierarchy: large token total first,
  compact rate stack underneath.

## Non-goals

- No cost, ETA, productivity scoring, streaks, or quota language.
- No new persisted state. Momentum is derived from the usage ledger at view
  model build time.
- No provider-specific momentum labels in the companion.
- No change to token ingestion, calibration, XP, evolution, or source identity.
- No companion labels such as `pulse`, `hour`, `warming`, or `cooling` in the
  final ring gap.

## Momentum Model

Add a renderer-neutral momentum model under `ProgressView`:

```rust
pub struct RateMomentum {
    pub pulse: RateWindow,
    pub hour: RateWindow,
    pub companion_direction: RateDirection,
}

pub struct RateWindow {
    pub current_tokens: f64,
    pub previous_tokens: f64,
    pub direction: RateDirection,
}

pub enum RateDirection {
    Up,
    Down,
    Neutral,
}
```

`current_tokens` values are the visible values:

- `pulse.current_tokens`: canonical tokens in `[now - 10m, now)`
- `hour.current_tokens`: canonical tokens in `[now - 60m, now)`

`previous_tokens` values are same-width comparison windows:

- `pulse.previous_tokens`: canonical tokens in `[now - 20m, now - 10m)`
- `hour.previous_tokens`: canonical tokens in `[now - 120m, now - 60m)`

Direction is derived by comparing current and previous:

```text
threshold = max(1,000 tokens, previous_tokens * 0.10)

current_tokens > previous_tokens + threshold  -> Up
current_tokens < previous_tokens - threshold  -> Down
otherwise                                     -> Neutral
```

This gives real movement without twitching on rounding noise. A transition from
zero to less than 1,000 tokens is neutral; a transition from zero to at least
1,000 tokens is up.

The companion needs one color for the whole rate block. Its overall direction
is:

```text
if pulse.direction is Up or Down:
    companion_direction = pulse.direction
else:
    companion_direction = hour.direction
```

Pulse wins because the companion is a live surface. The watch TUI still exposes
both per-window directions, so an hour-down but pulse-up recovery is not hidden.

## Watch TUI

The richer rate treatment belongs in the `today` panel, not the two-row
`progress` panel. The `today` panel already owns token accounting, source
breakdown, recent bucket, and seven-day history. Rate momentum is part of that
accounting story.

The watch TUI should render a compact momentum block after source rows and
before the seven-day sparkline:

```text
tokens       848,800,000
claude       567,000,000    67%
codex        281,800,000    33%
rate          ↑ 31.8M/10m
              ↓ 190.8M/hr
7-day       ...
```

Exact spacing can follow existing `MetricRow` conventions, but the visual
contract is:

- `pulse` appears first.
- `hour` appears second.
- Values use compact token units and suffixes: `/10m`, `/hr`.
- Each row has its own direction glyph in the TUI: up, down, or neutral.
- Color-capable terminals color the direction glyph and value green, red, or
  neutral/subtle.
- Non-color or limited-color terminals keep the glyphs as the non-color cue.

Once this block exists, the progress bar stops rendering the inline rate
segment. Progress remains focused on stage progress; rate momentum lives in the
`today` panel and the companion HUD.

## Companion

The companion keeps the current bottom-gap structure:

```text
large token total
compact rate stack
```

The approved companion readout is:

```text
848.8M
31.8M/10m
190.8M/hr
```

The final companion contract:

- Pulse first.
- Hour second.
- No arrow.
- No row labels.
- No captions.
- Align the two lines at the slash so `/10m` and `/hr` act as labels.
- Keep the large token total white.
- Color the whole two-line rate block by `companion_direction`:
  - up: green
  - down: red
  - neutral: current cool/subtle rate color

The companion should fit the block inside `stat_gap_box` the same way the
current `today` number and `/hr` line are fit. The implementation must measure
both rate lines and shrink or clamp them together so neither line clips the
growth ring gap.

If pulse and hour disagree, the companion colors by pulse direction. This keeps
the companion alive and glanceable. The TUI remains the detailed place to see
the disagreement.

## Data Flow

Use the existing usage store query boundary. `build_watch_view_model_at` already
opens `UsageStore` and computes:

- today totals
- last-10-minute totals
- current `rate_per_hour`

Implementation should extend that same build path with previous-window queries
using canonical Tokenmaxxing totals:

```text
UsageStore canonical token windows
  -> RateMomentum
  -> WatchViewModel
  -> TodayPanel
  -> companion draw_hud
  -> dev-preview fixtures
```

Do not add a separate companion-only query path. The companion receives a
`WatchViewModel` and should render from the shared momentum model.

## Testing And Preview

Unit and integration coverage should prove:

- Rate windows use canonical Tokenmaxxing totals and ignore legacy weighted
  rows, matching the existing `rate_per_hour` behavior.
- Current and previous 10-minute windows produce up, down, and neutral states.
- Current and previous 60-minute windows produce up, down, and neutral states.
- Companion direction prioritizes pulse when pulse is non-neutral and falls
  back to hour when pulse is neutral.
- The watch TUI renders both `/10m` and `/hr` rows and includes direction
  glyphs.
- The companion HUD formats pulse first, hour second, with slash-aligned units
  and no arrow.
- Idle/zero activity renders a neutral state without `-0` or misleading red.

Preview Lab should include deterministic fixtures for:

- watch momentum up/down/neutral
- companion rate up/down/neutral
- conflicting pulse/hour state, to confirm companion color and TUI detail match
  the design

Useful local checks after implementation:

```bash
cargo test --test watch_integration
cargo test --test tui_render
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

## Acceptance Criteria

- The TUI `today` panel makes rate momentum feel comparable in richness to the
  token count.
- The companion bottom-gap readout remains calmer than the TUI and does not
  become a miniature dashboard.
- A user can distinguish increasing, decreasing, and neutral activity without
  reading a paragraph of UI text.
- Both surfaces derive from one shared model, so Preview Lab and tests can
  review the behavior deterministically.
