# Watch Layout Refresh — Design

**Status**: Design approved (revision 3), ready for implementation plan.
**Source**: Visual brainstorm session (2026-05-12), Hybrid 2 selected. Revised after architecture / code-quality / product review. Revision 3 adds per-stat color palette, vertical-packing policy (Fill on pet + feed, anchored vitals/bio), and a tighter feed cap; success criterion restated accordingly.

## Problem

The watch view feels sparse on tall terminals. Small species (crystal in particular) leave the left column mostly empty around the pet. The right column ends well before the bottom of the frame. The helpers row duplicates information already present in the today block. XP is buried as the smallest row in vitals.

## Goal

Reshape the watch view so every region carries content the viewer cares about, surfacing the pet's stage progress as first-class data and filling the pet column with species-flavored ambient motion.

## Success criterion

The **left column has no dead bands at any terminal height**: the pet panel uses `Constraint::Fill(1)` so habitat ambient (PR2) absorbs vertical slack, while vitals + bio anchor to the column bottom. The **right column packs to the top** (today → progress → feed); feed is intentionally bounded at 6 events, so on tall terminals there is trailing space below feed. This asymmetry is deliberate — the pet's environment carries more visual weight than the event log, and feed is a recency view, not a scrollback.

Reference checkpoint: at 180×50, the left column reads as full canvas; the right column ends ≤16 rows below the bottom chrome and that trailing space is accepted.

We are **not** optimizing for ultrawide (>200 cols) terminals beyond the existing layout policy (right column flexes 50–70 then becomes outer padding). That edge is accepted, not solved.

## Out of scope

- No new key bindings, no new commands, no new persisted state.
- No changes to the feed text generation, activity entries, or speech bubbles.
- No new species, no new stage thresholds.
- The dropped helpers panel is not replaced by a separate diagnostics overlay; broken sources surface only via an inline marker on their `today` row.
- Color-blind / 8-color terminal palette tuning is deferred — the per-stat palette below is dark-background truecolor; ratatui's color downgrade handles low-capability terminals via the existing role styles.
- No `this week` summary panel (best day, active days, stages gained) in this revision. Considered during revision 3 brainstorm and deferred — if the trailing right-column space proves distracting in practice, it becomes a follow-up change rather than expanding this spec.

## Final layout

```
╭ glorp · luxopal the crystal · shard · 0d · content ─────────────────────╮
│                                                                          │
│   habitat ambient (Pass 1, panel-rect-wide, ~3% density, slow drift)     │
│                                                                          │
│              ╱╲                  today ───────────────────               │
│             ╱ ⋄ ╲                  tokens          1,633,930             │
│             ╲⋄⋄⋄╱                  claude          425,549  26%          │
│              ╲╱                    codex   ⚠     1,208,381  74%          │
│                                    last 10m       +109.8k  this 10m     │
│                                    . . . . . ▮ .          ← 7-day       │
│                                                                          │
│   habitat continues...           progress ────────────────                │
│                                    shard ➜ fractal                       │
│                                    ▰▰▰▰▰▱▱▱▱▱▱▱▱▱▱  33%  ↑ 109k/hr       │
│                                                                          │
│  vitals ───────────────          feed ────────────────────                │
│  fed    ▰▰▰▰▰░░ 74                 04:06  codex added 61.9k              │
│  happy  ▰▰▰▰░░░ 72                 04:06  codex added 56.1k              │
│  energy ▰▰▰▰░░░ 72                 04:07  claude-code added 17.3k        │
│                                    04:07  codex added 8.6k               │
│  bio ──────────────────            04:07  luxopal evolved into s0→s1     │
│  hatched  may 11 04:00             --:--  gained 61.9k effective tokens  │
│  age      0d 4h                                                          │
│                                                                          │
╰ q quit · r refresh · m mouse · ? help ──────────────────────────────────╯
```

Key changes from current:

- Right column reads: **today → progress → feed**. `SparkPanel` is dropped (7-day strip moves inline at the bottom of `today`). `HelpersPanel` is dropped (broken sources show an inline `⚠` on their `today` row).
- Left column reads: **pet + habitat → vitals → bio**. `PetPanel.render` becomes a two-pass paint (habitat first, then pet art on top). `VitalsPanel` drops its `xp` row (progress now lives in the new `progress` panel). New `BioCardPanel` below vitals, using the same `Borders::TOP` + section-title convention as every other panel (NOT a boxed card; flat section is what fits the existing visual language).

The bio section is intentionally minimal — just `hatched` and `age`. Earlier drafts had lifetime/best-day/active-days, but on day 0 four of five rows would read zero or `—`, and `lifetime` overlaps `today`'s `tokens`. Two rows are enough to anchor the bottom-left without becoming filler.

## Architecture

The watch pipeline stays: `Frame → outer chrome → inner Rect → wide/compact layout → panels`. No new abstractions, no rewrites.

### Module / file changes

```
src/tui/view_model.rs         + ProgressView, + BioView; existing fields unchanged
src/tui/panels/mod.rs         + bio_card, + progress, + bars; - spark, - helpers
src/tui/panels/pet.rs         add habitat backdrop pass before existing render;
                              switch preferred_constraint to Fill(1)
src/tui/panels/today.rs       add 7-day inline footer, add ⚠ marker on source rows;
                              source labels use new claude_color() / codex_color() roles
src/tui/panels/vitals.rs      drop xp row; each remaining row uses its stat color role
                              (fed_color, happy_color, energy_color) for label+bar+value
src/tui/panels/feed.rs        drop MAX_EVENT_ROWS from 8 to 6 (cap is hard, no Fill);
                              event source labels use claude_color() / codex_color()
src/tui/panels/progress.rs    NEW; xp bar uses xp_color() for label+fill+percent
src/tui/panels/bio_card.rs    NEW
src/tui/panels/bars.rs        NEW (shared bar/spark/format helpers — see "Shared bars module")
src/tui/style.rs              + fed_color/happy_color/energy_color/xp_color/
                              claude_color/codex_color semantic role functions
src/tui/layout.rs             remove SparkPanel + HelpersPanel from render_wide/compact;
                              reorder right column to today → progress → feed;
                              switch to Fill/Length constraint sequences per "Vertical
                              packing" below; update pet_panel_rect() to account for
                              BioCardPanel below vitals and to return the 13×10 pet
                              sub-rect (NOT the full panel rect) so tachyonfx scopes
                              correctly even when pet panel is tall
src/commands/watch.rs         compute ProgressView, BioView in build_watch_view_model_at
src/storage/usage_store.rs    + best_day_effective_tokens(), + events_within(duration);
                              fix seven_day_token_history to be aggregate-aware
src/tui/app.rs                no change to render loop; pet_panel_rect() change is invisible
src/tui/panels/spark.rs       REMOVED (logic absorbed into bars + today footer)
src/tui/panels/helpers.rs     REMOVED (signal absorbed into today)
```

## View model

Two new sub-structs in `tui/view_model.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressView {
    pub stage_label: String,        // "shard"
    pub next_stage_label: String,   // "fractal" — or "—" at S6
    pub fraction: f32,              // 0.0..=1.0; saturates at 1.0
    pub xp_in_stage: f64,           // state.xp - stage_start_xp(state.stage)
    pub xp_to_next: f64,            // next_stage_xp_target(state.stage)
    pub rate_per_hour: f64,         // 6h-half-life EMA, effective tokens / hour
    pub is_max_stage: bool,         // true at S6; ProgressPanel renders "stage maxed" instead of a bar
}

#[derive(Debug, Clone, PartialEq)]
pub struct BioView {
    pub hatched_label: String,      // "may 11 04:00" — pre-formatted at vm build, local TZ
    pub age_label: String,          // "0d 4h" sub-day, "12d" otherwise
}
```

`WatchViewModel` gains `pub progress: ProgressView` and `pub bio: BioView`. Existing `xp_current` / `xp_target` are **kept** in `WatchViewModel` — `ProgressView` derives `xp_in_stage`, `xp_to_next`, and `fraction` from them at build time, so stage thresholds stay defined in exactly one place (`next_stage_xp_target` in `commands/watch.rs`).

### XP unit clarification (correctness)

The existing `state.xp` and `next_stage_xp_target` are in **stage-progress units**, not tokens. S0→S1 target is `0.04`, S5→S6 target is `60.0`. Roughly 1 XP ≈ "one calibrated daily-effective-tokens budget" (per `game/evolution.rs`), with sqrt-squashing above 0.25 so the unit is non-linear vs raw tokens beyond mid-stages.

`ProgressView` therefore renders **just the percentage and the rate** — no raw XP or token counts in the bar. The percentage reads correctly at every stage; the rate (in tokens/hour) is what users actually want to track. Field naming reflects this: `xp_in_stage` / `xp_to_next`, not `tokens_in_stage` / `tokens_to_next`. The struct still exposes the raw XP for tests and any future tooling, but the panel doesn't print them. We deliberately do not display "tokens to next stage" — that conversion would be misleading without exposing the calibration baseline, which is not in scope.

### Source health marker

`SourceHealthView` (already exists) carries `status: SourceStatus` (`Ready | Diagnostic | Blocked`). `TodayPanel` renders the `⚠` marker when `status != Ready` — **NOT** when `diagnostic_code.is_some()`. The distinction matters: a silently-uninstalled helper has no `diagnostic_code` but `status == Blocked`, and that case must surface the marker (this was the helpers-panel regression flagged in review).

Existing fixtures (`WatchViewModel::fixture()`, `fixture_with_events()`) populate sensible defaults for both new structs so snapshot tests don't break.

## Data flow

All new work happens in `commands/watch.rs::build_watch_view_model_at(state, usage_db, now)`:

```
usage_store.events_within(Duration::hours(48), now)   NEW — replaces recent_events(500)
                                                      for EMA computation
    │
    ├─ progress_rate_ema(events, now)     → rate_per_hour
state.xp / next_stage_xp_target(stage)
    │
    ├→ ProgressView { stage_label, next_stage_label, fraction,
                      xp_in_stage, xp_to_next, rate_per_hour, is_max_stage }

state.created_at
    │
    ├→ BioView { hatched_label, age_label }
```

### `events_within` — replaces the 500-row LIMIT for EMA

```rust
pub fn events_within(
    &self,
    duration: Duration,
    now: OffsetDateTime,
) -> Result<Vec<NormalizedUsageEvent>>
```

`SELECT … FROM usage_events WHERE observed_at >= ?1 ORDER BY observed_at DESC`. Returns every event in the window, not capped at 500 rows. Smear inserts 6–12 buckets per delta × 2 providers polling every ~10s, which can exceed 500 rows within hours — the old `recent_events(500)` would silently truncate the EMA tail. The existing `recent_events(500)` stays for the feed-rendering use case where 500 is plenty and is also display-bounded.

### EMA rate

```rust
fn progress_rate_ema(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    const TAU_HOURS: f64 = 6.0 / std::f64::consts::LN_2;   // half-life = τ·ln2 = 6h
    let weighted: f64 = events
        .iter()
        .map(|e| {
            let dt_h = (now - e.observed_at).as_seconds_f64() / 3600.0;
            e.effective_tokens * (-dt_h / TAU_HOURS).exp()
        })
        .sum();
    weighted / TAU_HOURS    // tokens/hour
}
```

Properties: monotonic increase during active use, smooth decay during idle, no persisted state.

**Burst behavior — known and accepted.** A single 600k-token delta arriving over a short window reads as `~600k / τ ≈ 69k/hr` at peak, decaying thereafter. This is intrinsic EMA behavior, not a bug: the rate communicates "you're delivering tokens at a τ-window-averaged pace." Smear-bucket clustering (6–12 buckets of `delta/N` each, all within minutes) has negligible effect — the sum of the weighted buckets approximates the sum of the original delta because all weights are near 1 during the burst. No bucket-count weighting needed; documented behavior.

### `best_day_effective_tokens` — fixed SQL

The earlier draft of "max across union" was wrong: it would pick the larger of either source per day, not the daily sum. A day with rows in both `usage_events` (still-recent, uncompacted) and `daily_aggregates` (after compaction overlap) would undercount.

Correct shape:

```sql
SELECT MAX(daily_total) FROM (
    SELECT period_date, SUM(effective_tokens) AS daily_total
    FROM (
        SELECT period_date, effective_tokens FROM usage_events
        UNION ALL
        SELECT period_date, effective_tokens FROM daily_aggregates
    )
    GROUP BY period_date
)
```

`UNION ALL` (not `UNION`) so duplicate `(period_date, effective_tokens)` pairs aren't deduped — they may legitimately appear when a row sits in `usage_events` and is also rolled up into `daily_aggregates` during the compaction window. The outer `GROUP BY period_date` sums them all per day. Returns 0.0 for a freshly hatched pet.

`best_day_effective_tokens` is **not consumed by the bio card** in this revision (bio cut to `hatched · age`), but is added to `UsageStore` for completeness and tested.

### `seven_day_token_history` — same compaction blind spot

`seven_day_token_history` (`usage_store.rs:734`) currently queries only `usage_events`. Beyond compaction (90-day retention by default) the older days vanish even though they exist as `daily_aggregates`. The fix is to use the same aggregate-aware union pattern as `best_day_effective_tokens`. This change ships in the same PR — the new 7-day inline strip in `today` depends on the existing field and we shouldn't lock in the bug.

### `events_within` is also used for the existing feed

After this change, `today`'s `last_10m` / `this_10m` computation also uses `events_within(60min)` instead of consuming the 500-row stream. Today's `current_bucket_effective_tokens` keeps working because all its events fit comfortably under 500 — but standardizing on a time-windowed query removes a class of "silently truncates under load" bugs.

## Habitat rendering

Lives in `src/tui/panels/pet.rs`, not `pet/render.rs`. The panel composes; the renderer stays focused on the creature itself.

`PetPanel.render` becomes two passes within its own rect:

**Pass 1 — Ambient backdrop.** A new helper

```rust
fn ambient_glyphs_for(
    species: Species,
    panel: Rect,
    pet_inner_rect: Rect,   // the 13×10 the pet occupies — excluded from glyph placement
    now: OffsetDateTime,    // wall-clock; see drift period note
) -> Vec<AmbientGlyph>
```

returns 12–18 deterministic positions across the panel rect. Each glyph carries:

```rust
struct AmbientGlyph {
    row: u16,
    col: u16,
    glyph: char,
    role: PaletteRoleName,   // Particle, Accent, or Pattern — flows through themed palette
}
```

Positions are seeded by `(species_hash, drift_phase)`, where `drift_phase = (now.unix_timestamp() / DRIFT_SECS as i64)`. **`DRIFT_SECS = 8` — anchored to wall-clock seconds, NOT animation_frame ticks.** Same slots for ~8s, then shift ±1 row/col on the next phase. This avoids the strobe-during-fast-tick problem flagged in review: tachyonfx bursts run at 60fps but the habitat continues to drift at wall-clock rate.

The pet's `pet_inner_rect` (the 13×10 sub-rect occupied by the pet art, after wander/breath offsets) is excluded so the pet stays a clean silhouette over the ambient field.

**Pass 2 — Pet art.** Unchanged. Existing `render_pet`-rendered art sits at its wander/breath-offset position on top of the backdrop. The pet's own 13×10 particle frame stays — that's its personal halo.

### tachyonfx layering — explicit policy

`pet_panel_rect()` (called from the watch loop at `tui/app.rs:188-193` to scope tachyonfx overlay) must return the **13×10 pet sub-rect**, NOT the full `PetPanel` rect. Otherwise mood-fade / stage-up / feed-pulse effects would sweep across the habitat too — habitat glyphs would desaturate during mood changes, glitch noise would pulse with feed events, etc.

The helper's existing calculation `let pet_h = match PetPanel.preferred_constraint(vm) { Constraint::Length(n) => n, _ => 5 };` returns the wrong height now: it's the full panel height including habitat. Fix: return the inner pet rect explicitly, accounting for the wander/breath offset:

```rust
pub fn pet_panel_rect(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    // ... existing column-content-h calc, NOW including BioCardPanel's preferred height
    // ... existing inner panel rect calc
    // Return the 13×10 sub-rect within the panel that holds the pet art,
    // offset by vm.wander_offset_x and vm.breath_offset_y so effects track
    // the pet's actual on-screen position frame-by-frame.
}
```

`column_content_h` in `pet_panel_rect` must include `BioCardPanel`'s preferred height (`2` content rows + `1` `Borders::TOP` row carrying the title = `3`, plus `COLUMN_GAP` above). Without this fix the helper underestimates by ~4 rows and tachyonfx effects draw over vitals.

### Per-species glyph sets

| Species | Glyphs |
|---|---|
| Fuzz    | `· . , \`` (soft scatter) |
| Blob    | `o ° º`   (bubbles) |
| Ghost   | `~ ・ ⋮`  (mist) |
| Glitch  | `▒ ░ ▓ ▤` (static noise) |
| Crystal | `✦ ✧ ◇ ⋄ ·` (facet sparkles) |
| Mech    | `· + ╴ ╵ ╶` (grid ticks) |

Density target: ~3% of panel cells fill on any given drift phase. On a 40×14 panel rect that's ~17 glyphs.

## Helpers collapse

`HelpersPanel` is removed from the layout. Source health flows through the existing `SourceHealthView`:

`TodayPanel` renders an inline `⚠` marker on each source row when `status != Ready` (i.e. `Diagnostic` or `Blocked`). The marker sits **between the source label and its numeric value**:

```
codex          1,208,381   74%        (healthy)
codex  ⚠       1,208,381   74%        (blocked or has active diagnostic)
```

The `⚠` glyph uses the existing `diagnostic` log kind style (red/orange). Column alignment is preserved by reserving a 3-cell gutter after every source label whether or not a marker is rendered. The 1-hour staleness window from `STALE_DIAGNOSTIC_CUTOFF` is intentionally *not* applied to the `Blocked` state — a uninstalled helper stays marked indefinitely, not just for an hour.

## 7-day inline strip

`SparkPanel` is removed. `TodayPanel` gains a footer row rendering the existing `recent_daily_effective_tokens: Vec<f64>` as the same height-quantized blocks the spark panel used. Layout:

```
last 10m       +109.8k  this 10m
. . . . . ▮ .          ← 7-day
```

Implementation reuses the shared bar/spark helpers from the new `bars` module (see next section), so the visual is byte-identical to the dropped spark panel.

## Shared bars module

`src/tui/panels/bars.rs` is **new** and absorbs:

- `build_spark_lines` (currently in `spark.rs`)
- `bar_spans` (currently duplicated across `vitals.rs` and `today.rs`)
- `format_tokens_full` / `format_tokens_short` (currently duplicated; existing standing "T7 will deduplicate" TODOs)

This refactor was already pending across the existing panels. We absorb it here rather than letting the duplicates rot. All panels touched by this change point at the shared helpers.

## Vertical packing

Layout uses asymmetric anchoring per column to eliminate left-column dead bands while keeping feed bounded.

```rust
// Wide mode — left column
let left_constraints = [
    Constraint::Fill(1),                 // pet panel — habitat fills slack
    Constraint::Length(VITALS_HEIGHT),   // 4 rows (header + fed/happy/energy)
    Constraint::Length(COLUMN_GAP),      // 1 row
    Constraint::Length(BIO_HEIGHT),      // 3 rows (header + hatched + age)
];

// Wide mode — right column
let right_constraints = [
    Constraint::Length(TODAY_HEIGHT),    // 6 rows (header + 5 content)
    Constraint::Length(COLUMN_GAP),
    Constraint::Length(PROGRESS_HEIGHT), // 3 rows (header + stage row + xp bar)
    Constraint::Length(COLUMN_GAP),
    Constraint::Length(FEED_HEIGHT_MAX), // 7 rows (header + 6 events)
];
```

`MAX_EVENT_ROWS` in `feed.rs` drops from `8` to **`6`**. The cap is hard, not a `Fill` — feed never grows beyond 7 rows total. Anything beyond is intentionally off-screen for ambient use. The underlying event store is unchanged; users wanting the full tail can query it via `glorp status`.

**Compact mode** (single column) uses the same per-panel constraints, stacked vertically. Pet panel's `Fill(1)` absorbs vertical slack so habitat fills the slack rather than dead space. Same `MAX_EVENT_ROWS = 6` for feed.

**`pet_panel_rect()` implication.** Because the pet panel can now be tall (e.g. 25+ rows on a 50-row terminal when bio + vitals are only ~9 rows), the helper that scopes tachyonfx effects must return the 13×10 pet sub-rect, not the panel rect — already covered in "tachyonfx layering — explicit policy", but it becomes load-bearing here.

## Per-stat color palette

Each vital + xp gets a distinct semantic color role. The same color paints the label, the bar fill, and the trailing value; empty bar cells stay in the existing `empty_bar` muted gray so the colored fill pops.

New semantic palette entries in `tui/style.rs`:

```rust
pub fn fed_color()    -> Color { Color::Rgb(0xe8, 0xc4, 0x74) }   // amber
pub fn happy_color()  -> Color { Color::Rgb(0xe8, 0xa3, 0xc2) }   // pink
pub fn energy_color() -> Color { Color::Rgb(0x7f, 0xc8, 0xd6) }   // cyan
pub fn xp_color()     -> Color { Color::Rgb(0xef, 0x8e, 0x6c) }   // coral

pub fn claude_color() -> Color { Color::Rgb(0xb3, 0x9d, 0xf0) }   // violet
pub fn codex_color()  -> Color { Color::Rgb(0x8f, 0xcf, 0x90) }   // green
```

Application:

| Panel       | Where color applies                                                              |
|-------------|----------------------------------------------------------------------------------|
| VitalsPanel | Each row: label, filled bar segments, numeric value all use that stat's color    |
| ProgressPanel | xp bar: label, filled segments, percent all use `xp_color()`. Stage names (`fuzz ➜ archfuzz`) and rate (`↑ 109k/hr`) stay neutral |
| TodayPanel  | Source name only (`claude`, `codex`); values, percentages, `⚠` keep existing roles |
| FeedPanel   | Event source labels (`claude-code`, `codex`); timestamps and event text stay neutral |

Coral (`xp_color`) is deliberately distinct from amber (`fed_color`) — earlier draft used gold for both and they collided. Sources stay aligned across today and feed so the eye reads "who's contributing right now" and "who contributed recently" with the same color.

Color-blind accessibility is deferred (see "Out of scope").

## Error handling

- `events_within(48h)` returning empty → EMA rate `0.0`. `ProgressPanel` hides the `↑ Nk/hr` segment so the row reads `▰▰▰▱▱▱▱▱▱▱▱▱▱▱  33%` alone.
- S6 pets (`is_max_stage == true`): `ProgressPanel` renders a single line "**stage:** aurora · max evolved" with the species's max-stage label and a sage glyph (`✦`); no bar, no rate. Not a permanently-full bar.
- `best_day_effective_tokens` / `seven_day_token_history` on a freshly hatched pet → 0 / `[0.0; 7]`. Today's 7-day strip renders 7 zero-height dots.
- `BioView::age_label` at age 0 (instant after hatch): `"0d 0h"`. Sub-day formatting kicks in for any pet < 24h old.
- Source-health marker is purely additive — `status == Ready` renders the 3-cell gutter without a glyph.

## Testing

### Unit tests

- `progress_rate_ema` —
  - empty events → 0.0
  - single event at `now` → tokens / τ_hours within tolerance
  - two events 6h apart → second weighted ~half as much as first
  - 50k events spread over 48h → finite, no overflow (regression guard for the truncation bug)
- `BioView::age_label` formatting at 0h, 1h, 23h, 24h, 25h, 7d, 90d, 365d.
- `best_day_effective_tokens` SQL —
  - rows only in `usage_events`
  - rows only in `daily_aggregates`
  - rows in BOTH for the same `period_date` (compaction overlap) → sums correctly, not max-only
  - empty case
- `events_within(duration)` SQL — boundary at exactly `duration` ago, near-boundary inclusion.
- `seven_day_token_history` — old days that exist only in `daily_aggregates` are surfaced, no longer drop off after compaction.
- `ambient_glyphs_for` — same `drift_phase` returns same positions across calls; positions never overlap the pet inner rect; per-species glyph sets are non-empty.
- `pet_panel_rect` — returns the inner 13×10 pet sub-rect, offset by wander/breath; total column height calc includes `BioCardPanel`.
- `FeedPanel` — given a `Vec<EventView>` of length 12, `preferred_constraint` reports `Length(7)` (header + 6 events); rendering produces at most 6 event rows regardless of input length.
- Color role functions in `style.rs` — return the expected `Color::Rgb(...)` tuples (regression guard against accidental palette drift).

### Snapshot tests

The following existing assertions in `tests/tui_render.rs` need explicit deletion or rewrite (not "fixture-update propagation"):

- Lines that assert `text.contains("helpers")` — at least 6 occurrences (~121, 168, 258, 438, 443, 842) — all removed.
- `compact_threshold_switches_modes` (~522-531) — the helper-string assertion is replaced with an assertion about the new compact panel ordering (today / progress / feed; no helpers).
- Any assertion about the `xp` row in vitals — moves to progress panel assertions.

New per-panel snapshots:

- `ProgressPanel` at S0 (sub-S1, fractional bar) — xp bar uses `xp_color()`
- `ProgressPanel` at S6 (max-evolved, no bar)
- `ProgressPanel` with rate = 0 (idle, no rate token)
- `BioCardPanel` at age 0h (sub-day formatting)
- `BioCardPanel` at age 12d (day-only formatting)
- `TodayPanel` with one source `Blocked` (marker rendered) and one `Ready` (no marker; gutter preserved); source labels use `claude_color()` / `codex_color()`
- `TodayPanel` 7-day footer with all-zero history
- `PetPanel` 2-pass render for crystal (sparkles present in panel rect, none overlap 13×10 pet sub-rect)
- `VitalsPanel` — assert each row's label / bar-fill / value span uses the matching stat color role (one assertion per stat)
- `FeedPanel` with 12 events in the vm — renders exactly 6 (cap honored); source label spans use the matching source color

New whole-frame snapshots:

- Wide mode at 120×32 with all panels in the new arrangement (fed pet, ProgressView populated) — assert left column has zero rows with no content between the pet panel rect and the bottom chrome; assert feed panel rect height ≤ 7.
- Wide mode at 180×50 — same assertions; verifies Fill policy scales (left column fills, feed stays bounded, trailing right-column space is acceptable).
- **Compact mode** with the same content (verifies the new panel ordering in compact still works; feed cap still 6).

### Integration tests

- `tests/watch_integration.rs` gains a case that seeds `daily_aggregates` for 8 days, hatches a pet at a fixed `created_at`, and asserts `vm.bio.age_label` and `vm.bio.hatched_label` from a deterministic `now`.
- A second case asserts the inline `⚠` marker by seeding a `provider_diagnostic` for codex and checking `vm.source_health[codex].status != Ready`.
- A third case asserts EMA monotonicity: insert N events over 6h, verify `rate_per_hour` is strictly greater than after the same setup minus the last event.

### Dev-preview QA

`glorp dev-preview` renders the new layout to `target/glorp-preview/index.html`. Manual visual QA happens in a browser — easier to verify habitat density, progress bar styling, the `⚠` placement, absence of dead bands at 180×50 in the left column, the bounded feed in the right column, and the per-stat color palette reading correctly against the dark frame background.

## Implementation order

Two PRs at execution time, in this order:

1. **Layout refactor + color + Fill** — view-model changes, new `BioCardPanel` / `ProgressPanel` / `bars` module, today gains 7-day footer + `⚠` marker, vitals drops xp, layout drops `SparkPanel` + `HelpersPanel`, removed files deleted, `UsageStore` query additions (`events_within`, `best_day_effective_tokens`, fixed `seven_day_token_history`), EMA helper, **per-stat color palette** added to `style.rs` and consumed by vitals/progress/today/feed, **Fill(1) constraint** on pet + bounded `Length` on feed (cap `MAX_EVENT_ROWS = 6`). **Includes a no-op `ambient_glyphs_for` stub returning an empty Vec, plus the 2-pass paint scaffolding in `PetPanel.render`.** Merging PR1 leaves the left-column pet zone visibly empty above and below the pet (Fill creates room that PR2 fills with habitat glyphs) — this is intentional and called out in the PR description so reviewers aren't surprised. PR1 is independently shippable.
2. **Habitat** — fills in `ambient_glyphs_for` per-species and the drift-phase logic, populating the empty pet-panel rows created by PR1's Fill constraint. Pure rendering, no view-model changes. Independently shippable on top of PR1.
