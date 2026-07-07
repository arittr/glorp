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

This spec is a narrow, explicit exception to the original V1 round companion
rule that avoided exact counts and rates in the round view. Drew approved this
exception for the native macOS companion HUD only. The guardrails remain:

- No source labels, project names, file paths, prompts, responses, costs, quotas,
  ETAs, streaks, or productivity scoring.
- Exact telemetry stays in the HUD/text layer, not in `RoundSceneModel`.
- Text remains neutral grey/white; pace direction is not encoded in companion
  text color.
- If the surface becomes too small, asleep, calm, or private-display behavior is
  later added, the companion may dim or drop the `daily` and `pace` lanes before
  it shrinks the pet/tank below legible size.

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

The companion text stack keeps the large total white and all comparison/rate
lines in the current neutral grey treatment. This supersedes any earlier
companion-rate color ambiguity in the rate-momentum spec: the amber perimeter
lane is the only companion pace color.

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
- Lane ends use round caps. The AppKit painter should set round line caps
  explicitly for track and fill paths. The pace lane may use a brighter
  cap/comet treatment to make recent activity feel alive.

The gauge should stay close enough to read as a thick band, not three distant
decorative outlines. It should still keep a small amount of separation between
lanes so the three colors do not blur together.

### Lane Geometry

The gauge is painted inside the existing circular aperture clip. "Outside lane"
means closest to the aperture rim, not outside the aperture.

Add a pure lane-bundle helper in cfg-free round code, for example:

```rust
pub struct GaugeLane {
    pub ring: GrowthRing,
    pub stroke_width: f64,
    pub cap: LineCap,
}

pub struct PerimeterGaugeLayout {
    pub xp: GaugeLane,
    pub daily: GaugeLane,
    pub pace: GaugeLane,
}
```

The helper receives the aperture center/radius and bottom gap degrees, then
derives all three lane center radii and stroke widths. The contract is:

```text
outer_inset_px = max(3.0, aperture_radius * 0.012)
xp_width       = clamp(aperture_radius * 0.050, 6.0, 16.0)
daily_width    = clamp(aperture_radius * 0.040, 5.0, 13.0)
pace_width     = clamp(aperture_radius * 0.034, 4.0, 11.0)
lane_gap       = clamp(aperture_radius * 0.010, 1.5, 4.0)

xp.radius    = aperture_radius - outer_inset_px - xp_width / 2
daily.radius = xp.radius - xp_width / 2 - lane_gap - daily_width / 2
pace.radius  = daily.radius - daily_width / 2 - lane_gap - pace_width / 2
```

Implementation may tune these constants during device review, but it must keep
the same invariants: every stroke's outer edge stays inside the aperture, every
lane uses the same bottom-gap arc, and the pace lane's inner edge leaves the
tank/pet safe area unobstructed. Tests should assert those invariants for the
normal 360px companion window and smaller supported sizes.

## Daily Progress

Daily progress must be based on provider-day snapshot totals, matching the
visible companion total and the Tokenmaxxing accounting day.

Add an explicit top-level `WatchViewModel.daily_comparison` HUD field rather
than deriving from vector positions inside the companion renderer:

```rust
pub struct DailyComparison {
    pub today_provider_day: time::Date,
    pub yesterday_provider_day: time::Date,
    pub today_tokens: f64,
    pub yesterday_tokens: Option<f64>,
    pub today_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub yesterday_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub today_observed_at: Option<time::OffsetDateTime>,
    pub yesterday_observed_at: Option<time::OffsetDateTime>,
    pub unavailable_reason: Option<String>,
    pub fraction_of_yesterday: Option<f64>,
}
```

Build this from `UsageStore::snapshot_totals_for_provider_day(today)` and
`UsageStore::snapshot_totals_for_provider_day(yesterday)`. `today` is the
current Tokenmaxxing provider day; `yesterday` is the previous provider day.

`fraction_of_yesterday` is `Some(today / yesterday)` only when:

- both snapshot states are `SnapshotState::Current`;
- both snapshot values exist;
- both totals are finite and non-negative;
- yesterday's total is greater than zero.

Otherwise `fraction_of_yesterday` is `None` and `unavailable_reason` records the
first useful reason, such as `today-stale`, `yesterday-missing`,
`yesterday-zero`, or `non-finite-total`.

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

Use a named soft cap and pure helper:

```text
PACE_SOFT_CAP_10M_TOKENS = 50_000_000
pace_fraction = 1.0 - exp(-current_10m_tokens / PACE_SOFT_CAP_10M_TOKENS)
```

`50_000_000` is a design constant for the companion gauge, not a provider limit
or a private animation constant. It roughly matches the "big active burst" scale
already used in preview examples while preserving the semantic contract:
"recent activity pulse strength," not "percentage of max possible speed."
Implementation may tune this constant only with updated preview artifacts and
tests.

The helper clamps negative, NaN, and infinite inputs to safe values before
applying the formula. Test anchors:

```text
0 tokens       -> 0.0
soft cap       -> about 0.63
2x soft cap    -> about 0.86
very large     -> <= 1.0
bad/non-finite -> 0.0
```

Idle pace renders as an empty amber track. Active pace renders as a short-to-long
amber fill with an optional bright cap.

## Text Formatting

The companion text stack is fixed-height:

```text
{today_total}
{daily_percent}
{pace}/10m
```

Formatting rules:

- `today_total`: existing `format_tokens(vm.today_effective_tokens)` treatment.
- `daily_percent`: nearest whole percent plus ` yday`, for example `94% yday`.
- unavailable daily comparison: `--% yday`.
- over-100 daily comparison: show the actual rounded percent until it exceeds
  `999%`, then show `999%+ yday`.
- `pace`: existing compact token formatting for the current 10-minute pulse.

The AppKit HUD should measure all three lines together and shrink the stack as a
unit to fit inside `stat_gap_box`. It must not let the percent line widen the
bottom gap or overlap the ring.

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

Put reusable geometry, cap-style metadata, text formatting, and normalization
helpers in cfg-free `src/round/hud.rs` or an adjacent `src/round/` module so
they are covered by Linux CI. Keep AppKit painting in `src/companion/app.rs`.

Preview Lab currently renders the shared round scene and halo, but not the
native AppKit HUD/ring layer. This feature must add a renderer-neutral Preview
Lab HUD artifact before implementation is considered complete. The artifact may
be JSON rather than pixel-perfect AppKit drawing, but it must include lane
radii, stroke widths, gap degrees, fill fractions, cap style, colors, and text
strings. The preview scenarios must cover normal, missing yesterday, stale
snapshot, zero yesterday, over-100% daily, idle pace, and burst pace cases while
preserving the existing round privacy contract.

## Error Handling

- Missing or zero yesterday: no daily fill; neutral `--% yday` text.
- Snapshot stale, missing, or blocked: daily comparison renders no fill and
  `--% yday` if either provider-day total is not trustworthy.
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
- Pace normalization is monotonic, clamps bad inputs, and matches the soft-cap
  anchors above.
- All three lane geometries share the same bottom gap and stay inside the
  circular aperture.
- XP is visually primary by geometry constants: outside lane and widest stroke.
- Lane cap style is round for tracks and fills.
- Companion text stack renders total, `% yday`, and `/10m`, without `/hr`.
- Preview Lab emits deterministic HUD artifacts for the lane bundle and text
  stack scenarios.
- Exact telemetry stays out of `RoundSceneModel`.

Useful local checks after implementation:

```bash
cargo test --test watch_integration
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
cargo test --lib round::hud
cargo xtask companion fresh
```
