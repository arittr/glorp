# Glorp companion perimeter gauges - design

- Date: 2026-07-07
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md`
  - `docs/superpowers/specs/2026-06-24-glorp-companion-tank-redesign-design.md`
  - `docs/superpowers/specs/2026-07-06-glorp-rate-momentum-design.md`

## Problem

The macOS round companion currently has one outside growth ring for stage XP and
a compact text stack in the bottom gap. Rate momentum is already modeled and
shown as text, but the perimeter still answers only one visual question:
"how far through this stage is my Glorp?"

Drew wants the companion to show more glanceable live context, especially
current pace and the current day compared with yesterday, without turning the
round pet surface into a full dashboard.

## Direction

Add a close, thick, three-lane perimeter gauge around the existing tank.

The approved visual family is the "B2" mockup direction from visual
brainstorming:

- Thick, close lanes hugging the right-hand perimeter.
- XP remains the outside lane and is slightly more prominent than the others.
- The secondary lanes sit immediately inside the XP lane and are still thick
  enough to read on the round display.
- All lanes preserve the existing open-bottom gap for the text stack.
- The pet and tank remain the emotional center; the gauge is strong but does
  not cover the pet, props, or bottom stat area.

## Metrics

The three lanes are:

1. `xp`: stage progress, from `WatchViewModel.progress.fraction`.
2. `daily`: today's provider-day Tokenmaxxing total compared with yesterday's
   provider-day Tokenmaxxing total.
3. `pace`: current 10-minute Tokenmaxxing total, from
   `WatchViewModel.rate_momentum.pulse.current_tokens`.

The bottom text stack becomes:

```text
842M
94% yday
31M/10m
```

Drop the hourly rate from the companion text stack for this design. The watch
TUI can remain the detailed place for pulse plus hour momentum. The companion
should answer "today versus yesterday" and "is work happening right now?" at a
glance.

## Visual Contract

The perimeter gauge keeps the same bottom-gap direction as the existing XP ring:
fill starts at the lower-right edge of the bottom gap and advances
counter-clockwise around the right side of the rim, over the top, and toward the
left as it completes. In ordinary partially-filled states, this reads visually
as movement from right to left along the outside of the companion.

Lane treatment:

- `xp`: violet, outside lane, widest and brightest.
- `daily`: cyan, middle lane, slightly narrower than XP.
- `pace`: amber, inner lane, slightly narrower than daily.
- Track strokes are dim and present for all lanes, using the same open-bottom
  arc geometry so the lanes read as one intentional gauge.
- Lane ends use round caps. The pace lane may use a brighter cap/comet treatment
  to make recent activity feel alive.

The gauge should stay close enough to read as a thick band, not three distant
decorative outlines. It should still keep a small amount of separation between
lanes so the three colors do not blur together.

## Daily Progress

Daily progress must be based on provider-day snapshot totals, matching the
visible companion total and the Tokenmaxxing accounting day.

Add an explicit renderer-neutral model field rather than deriving from vector
positions inside the companion renderer:

```rust
pub struct DailyComparison {
    pub today_tokens: f64,
    pub yesterday_tokens: Option<f64>,
    pub yesterday_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub fraction_of_yesterday: Option<f64>,
}
```

`fraction_of_yesterday` is `None` when yesterday is missing, stale,
incomplete, or zero.
When present, the daily lane fill clamps to `0.0..=1.0`, but the text may show
values above 100%, such as `124% yday`, after today beats yesterday.

If yesterday is unavailable or untrusted, render the daily lane track only and
show a neutral `--% yday` line. Keeping the line preserves the companion text
stack's height and avoids implying a confident comparison.

## Pace Normalization

The pace lane must not pretend there is a precise hard speedometer maximum.
Normalize the current 10-minute total with a gentle saturating scale that makes
small real activity visible and prevents unusually large bursts from dominating
the whole companion.

The implementation plan should choose and test a small pure helper such as:

```text
pace_fraction = 1.0 - exp(-current_10m_tokens / pace_soft_cap)
```

The exact `pace_soft_cap` should be based on current fixture values and local
visual review, not on private animation constants such as `RATE_FULL`. The
semantic contract is "recent activity pulse strength," not "percentage of max
possible speed."

Idle pace renders as an empty amber track. Active pace renders as a short-to-long
amber fill with an optional bright cap.

## Architecture

Keep the exact telemetry in `WatchViewModel` and companion HUD rendering. Do not
move exact token totals, rates, or provider-day percentages into
`RoundSceneModel`; that model remains the privacy-safe semantic scene for round
preview and non-dashboard surfaces.

Recommended shape:

```text
UsageStore provider-day snapshots
  -> WatchViewModel.daily_comparison
  -> companion HUD text and perimeter lane fill

WatchViewModel.rate_momentum.pulse.current_tokens
  -> pace normalization helper
  -> companion perimeter lane fill

WatchViewModel.progress.fraction
  -> XP lane fill
```

Put reusable geometry and normalization helpers in cfg-free `src/round/hud.rs`
or an adjacent `src/round/` module so they are covered by Linux CI. Keep AppKit
painting in `src/companion/app.rs`.

Preview Lab currently renders the shared round scene and halo, but not the
native AppKit HUD/ring layer. Implementation should either extend preview with a
renderer-neutral HUD artifact or rely on pure helper tests plus macOS/device
visual verification. If preview support is added, it must preserve the existing
round privacy contract.

## Error Handling

- Missing or zero yesterday: no daily fill; neutral `--% yday` text.
- Snapshot stale or diagnostic: daily comparison renders no fill and `--% yday`
  if the underlying provider-day total is not trustworthy.
- Negative, NaN, or non-finite values: clamp to safe empty values before
  rendering.
- Max-stage XP: XP lane renders full, matching current behavior.
- Over-100% day: daily lane renders full; text may exceed 100%.

## Testing And Preview

Coverage should prove:

- Daily comparison uses provider-day Tokenmaxxing snapshot totals, not local
  applied-effective `DayContext.yesterday.ratio`.
- Missing, zero, stale, and over-100% yesterday cases render deterministic
  fractions/text.
- Pace normalization is monotonic, clamps bad inputs, and saturates smoothly.
- All three lane geometries share the same bottom gap and stay inside the
  circular aperture.
- XP is visually primary by geometry constants: outside lane and widest stroke.
- Companion text stack renders total, `% yday`, and `/10m`, without `/hr`.
- Exact telemetry stays out of `RoundSceneModel`.

Useful local checks after implementation:

```bash
cargo test --test watch_integration
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
cargo test --lib round::hud
cargo xtask companion fresh
```
