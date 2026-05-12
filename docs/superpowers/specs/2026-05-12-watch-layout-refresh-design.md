# Watch Layout Refresh — Design

**Status**: Design approved, ready for implementation plan.
**Source**: Visual brainstorm session (2026-05-12), Hybrid 2 selected.

## Problem

The watch view feels sparse on tall terminals. Small species (crystal in particular) leave the left column mostly empty around the pet. The right column ends well before the bottom of the frame. The helpers row duplicates information already present in the today block. XP is buried as the smallest row in vitals.

## Goal

Reshape the watch view so every region carries content the viewer cares about, surfacing pet "history" (lifetime stats, hatched-on, best day) and stage progress as first-class data, while filling the pet column with species-flavored ambient motion.

## Out of scope

- No new key bindings, no new commands, no new persisted state.
- No changes to the feed text generation, activity entries, or speech bubbles.
- No new species, no new stage thresholds.
- The dropped helpers panel is not replaced by a separate diagnostics overlay; broken sources surface only via an inline marker on their `today` row.

## Final layout

```
╭ glorp · luxopal the crystal · shard · 0d · content ─────────────────────╮
│                                                                          │
│   habitat ambient (Pass 1, panel-rect-wide, ~3% density, slow drift)     │
│                                                                          │
│              ╱╲                  today ───────────────────               │
│             ╱ ⋄ ╲                  tokens          1,633,930             │
│             ╲⋄⋄⋄╱                  claude          425,549  26%          │
│              ╲╱                    codex         1,208,381  74%   ⚠      │
│                                    last 10m       +109.8k  this 10m     │
│                                    . . . . . ▮ .          ← 7-day       │
│                                                                          │
│   habitat continues...           progress ────────────────                │
│                                    shard ➜ fractal                       │
│                                    ▰▰▰▰▰▱▱▱▱▱▱▱▱▱▱  33%                  │
│                                    1.6M / 5.0M     ↑ 109k/hr             │
│                                                                          │
│  vitals ───────────────          feed ────────────────────                │
│  fed    ▰▰▰▰▰░░ 74                 04:06  codex added 61.9k              │
│  happy  ▰▰▰▰░░░ 72                 04:06  codex added 56.1k              │
│  energy ▰▰▰▰░░░ 72                 04:07  claude-code added 17.3k        │
│                                    04:07  codex added 8.6k               │
│  ┌─ luxopal ───────────────┐       04:07  luxopal evolved into s0→s1     │
│  │ lifetime  1.6M tokens   │       --:--  gained 61.9k effective tokens  │
│  │ best day  1.6M today    │       --:--  gained 56.1k effective tokens  │
│  │ hatched   may 11 04:00  │                                             │
│  │ active    1 day         │                                             │
│  │ age       0d 4h         │                                             │
│  └─────────────────────────┘                                             │
│                                                                          │
╰ q quit · r refresh · m mouse · ? help ──────────────────────────────────╯
```

Key changes from current:

- Right column reads: **today → progress → feed**. `SparkPanel` is dropped (7-day strip moves inline at the bottom of `today`). `HelpersPanel` is dropped (broken sources show an inline `⚠` on their `today` row).
- Left column reads: **pet + habitat → vitals → bio card**. `PetPanel.render` becomes a two-pass paint (habitat first, then pet art on top). `VitalsPanel` drops its `xp` row (XP now lives in `progress`). New `BioCardPanel` below vitals.

## Architecture

The watch pipeline stays: `Frame → outer chrome → inner Rect → wide/compact layout → panels`. No new abstractions, no rewrites.

### Module / file changes

```
src/tui/view_model.rs         + ProgressView, + BioView; existing fields unchanged
src/tui/panels/mod.rs         + bio_card, + progress; - removed exports
src/tui/panels/pet.rs         add habitat backdrop pass before existing render
src/tui/panels/today.rs       add 7-day inline footer, add ⚠ marker on source rows
src/tui/panels/vitals.rs      drop xp row
src/tui/panels/progress.rs    NEW
src/tui/panels/bio_card.rs    NEW
src/tui/layout.rs             remove SparkPanel + HelpersPanel from render_wide/compact;
                              reorder right column to today → progress → feed
src/commands/watch.rs         compute ProgressView, BioView in build_watch_view_model_at
src/storage/usage_store.rs    + best_day_effective_tokens(), + active_days_count()
src/tui/panels/spark.rs       REMOVED (logic absorbed into today)
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
    pub tokens_in_stage: f64,       // xp earned in current stage, displayed as tokens
    pub tokens_to_next: f64,        // tokens needed to reach next stage
    pub rate_per_hour: f64,         // 6h-half-life EMA over last ~48h of events
}

#[derive(Debug, Clone, PartialEq)]
pub struct BioView {
    pub lifetime_tokens: f64,
    pub best_day_tokens: f64,
    pub hatched_at: OffsetDateTime,
    pub active_days: u32,
    pub age_label: String,          // "0d 4h" sub-day, "12d" otherwise
}
```

`WatchViewModel` gains `pub progress: ProgressView` and `pub bio: BioView`. Existing `xp_current` / `xp_target` are kept — `ProgressView` derives `tokens_*` and `fraction` from them at build time, so stage thresholds stay defined in exactly one place (`next_stage_xp_target`).

`SourceHealthView` (already exists) carries `diagnostic_code: Option<String>` which `TodayPanel` uses to render the `⚠` marker. No new type needed for the helpers-collapse.

Existing fixtures (`WatchViewModel::fixture()`, `fixture_with_events()`) populate sensible defaults for both new structs so snapshot tests don't break.

## Data flow

All new work happens in `commands/watch.rs::build_watch_view_model_at(state, usage_db, now)`:

```
usage_store.recent_events(500)            (existing)
    │
    ├─ progress_rate_ema(events, now)     → rate_per_hour
state.xp / next_stage_xp_target(stage)
    │
    ├→ ProgressView { stage_label, next_stage_label, fraction,
                      tokens_in_stage, tokens_to_next, rate_per_hour }

usage_store.lifetime_effective_tokens()   (existing)
usage_store.best_day_effective_tokens()   NEW
usage_store.active_days_count()           NEW
state.created_at
    │
    ├→ BioView { lifetime_tokens, best_day_tokens, hatched_at,
                 active_days, age_label }
```

### EMA rate

```rust
fn progress_rate_ema(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    const TAU_HOURS: f64 = 6.0 / std::f64::consts::LN_2;   // 6h half-life
    let cutoff = now - Duration::hours(48);
    let weighted: f64 = events
        .iter()
        .filter(|e| e.observed_at >= cutoff)
        .map(|e| {
            let dt_h = (now - e.observed_at).as_seconds_f64() / 3600.0;
            e.effective_tokens * (-dt_h / TAU_HOURS).exp()
        })
        .sum();
    weighted / TAU_HOURS
}
```

Properties: monotonic increase during active use, smooth decay during idle, no persisted state.

### New `UsageStore` methods

```rust
pub fn best_day_effective_tokens(&self) -> Result<f64> {
    // UNION: today's running sum from usage_events + each historical day from daily_aggregates
    // Returns max across the union, or 0.0 if both are empty.
}

pub fn active_days_count(&self) -> Result<u32> {
    // SELECT COUNT(*) FROM (
    //   SELECT DISTINCT period_date FROM usage_events
    //   UNION
    //   SELECT DISTINCT period_date FROM daily_aggregates
    // )
}
```

Both use SQL the same shape as existing aggregate queries in `usage_store.rs`. No new tables, no schema migration.

## Habitat rendering

Lives in `src/tui/panels/pet.rs`, not `pet/render.rs`. The panel composes; the renderer stays focused on the creature itself.

`PetPanel.render` becomes two passes within its own rect:

**Pass 1 — Ambient backdrop.** A new helper

```rust
fn ambient_glyphs_for(species: Species, panel: Rect, tick: u64) -> Vec<AmbientGlyph>
```

returns 12-18 deterministic positions across the panel rect. Each glyph carries:

```rust
struct AmbientGlyph {
    row: u16,
    col: u16,
    glyph: char,
    role: PaletteRoleName,   // Particle, Accent, or Pattern — flows through themed palette
}
```

Positions are seeded by `(species_hash, tick / DRIFT_PERIOD)`. Same slots for several seconds, then shift ±1 row/col on the next period. `DRIFT_PERIOD ≈ 32` (8s at 250ms idle tick). Half-life-per-slot ~8s — viewer sees motion every few seconds without it being twitchy.

The pet's centered 13×10 sub-rect is excluded so the pet stays a clean silhouette over the ambient field.

**Pass 2 — Pet art.** Unchanged. Existing `render_pet`-rendered art sits at its wander/breath-offset position on top of the backdrop. The pet's own 13×10 particle frame stays — that's its personal halo.

### Per-species glyph sets

| Species | Glyphs |
|---|---|
| Fuzz    | `· . , \`` (soft scatter) |
| Blob    | `o ° º`   (bubbles) |
| Ghost   | `~ ・ ⋮`  (mist) |
| Glitch  | `▒ ░ ▓ ▤` (static noise) |
| Crystal | `✦ ✧ ◇ ⋄ ·` (facet sparkles) |
| Mech    | `· + ╴ ╵ ╶` (grid ticks) |

Density target: ~3% of panel cells fill on any given tick. On a 40×14 panel rect that's ~17 glyphs.

The drift period reuses the existing `animation_frame` counter, so fast-tick bursts during effects naturally accelerate the habitat too.

## Helpers collapse

`HelpersPanel` is removed from the layout. Source health flows through the existing `SourceHealthView` instead:

`TodayPanel` renders an inline marker on each source row. The marker shows when that source has a recent diagnostic (existing `diagnostic_code: Option<String>` field, which already ages out after 1h per the existing `STALE_DIAGNOSTIC_CUTOFF`). The marker sits **between the source label and its numeric value**:

```
codex          1,208,381   74%        (healthy)
codex  ⚠       1,208,381   74%        (active diagnostic)
```

The `⚠` glyph uses the existing `diagnostic` log kind style (red/orange). Hover/tooltip is not part of this design — the meaning is signaled by color and position alone. Column alignment is preserved by reserving a 3-cell gutter after the source label whether or not a marker is rendered.

When all sources have diagnostics (totally broken state), users still see numbers (likely zeros) plus markers; the `errors` field on `WatchViewModel` continues to drive any modal or future error treatment.

## 7-day inline strip

`SparkPanel` is removed. `TodayPanel` gains a footer row rendering the existing `recent_daily_effective_tokens: Vec<f64>` as the same height-quantized blocks the spark panel used. Layout:

```
last 10m       +109.8k  this 10m
. . . . . ▮ .          ← 7-day
```

Implementation reuses `SparkPanel`'s `format_bar` helper (extracted to a shared module or `pub(super)`-promoted), so the visual is identical. Save ~3 rows of panel chrome on the right column.

## Error handling

- `best_day_effective_tokens()` / `active_days_count()` on a freshly hatched pet → `0.0` / `0`. Bio card renders `best day  —` and `active days  0` for the empty case.
- EMA rate with zero events in last 48h → `0.0`. ProgressPanel hides the `↑ N/hr` token from the third row of the panel rather than printing `↑ 0/hr`; the `tokens_in / tokens_to_next` portion still renders.
- S6 pets: `next_stage_label = "—"`, `tokens_to_next = 0.0`, `fraction = 1.0`. Bar renders full + a locked-stage glyph; rate still shows.
- Source-health marker is purely additive — `diagnostic_code.is_none()` renders nothing extra. No new failure modes.

## Testing

### Unit tests

- `progress_rate_ema` — empty events → 0; single recent event → tokens / τ_hours within tolerance; two events 6h apart → second weighted half as much.
- `BioView::age_label` formatting at 0h, 1h, 23h, 24h, 7d, 90d.
- `best_day_effective_tokens` SQL — rows only in `usage_events`, rows only in `daily_aggregates`, rows in both, empty case.
- `active_days_count` SQL — equivalent shape.
- `ambient_glyphs_for` — same `tick / DRIFT_PERIOD` returns same positions; positions never overlap the pet sub-rect.

### Snapshot tests

- Existing `tests/tui_render.rs` snapshots adopt `ProgressView` / `BioView` defaults on the fixture. Existing assertions remain valid because xp display moves but title-bar / vitals fed/happy/energy don't.
- New per-panel snapshot tests using the existing render-buffer harness for `ProgressPanel` and `BioCardPanel`.
- One new wide-mode whole-frame snapshot covering all panels in the new arrangement.

### Integration tests

- `tests/watch_integration.rs` gains a case that seeds 2 weeks of `daily_aggregates`, hatches a pet at a fixed `created_at`, and asserts `vm.bio.lifetime_tokens`, `vm.bio.best_day_tokens`, `vm.bio.active_days`, `vm.bio.age_label` from a deterministic `now`.
- A second case asserts the inline `⚠` marker by seeding a `provider_diagnostic` and checking `vm.source_health[codex].diagnostic_code.is_some()`.

### Dev-preview QA

`glorp dev-preview` renders the new layout to `target/glorp-preview/index.html`. Manual visual QA happens in a browser, not a tall terminal — easier to verify habitat density, progress bar styling, bio card framing.

## Implementation order

Two PRs at execution time, in this order:

1. **Layout refactor** — view-model changes, new `BioCardPanel` / `ProgressPanel`, today gains 7-day footer + `⚠` marker, vitals drops xp, layout drops `SparkPanel` + `HelpersPanel`, removed files deleted. Plus `UsageStore` query additions and EMA helper.
2. **Habitat** — `ambient_glyphs_for` + `PetPanel.render` 2-pass paint. Pure rendering, no view-model changes.

Both should land on `main` within a day of each other; the design is one piece.
