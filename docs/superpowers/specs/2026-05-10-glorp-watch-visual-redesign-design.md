# Glorp Watch Visual Redesign

Date: 2026-05-10

## Overview

Rewrite the visual layer of `glorp watch` to fix five concrete pain points with the current TUI: fake terminal chrome inside an already-real terminal, weak layout hierarchy, prototype-derived stats panel that violates the parent product spec by showing commits/PRs/diffs, flat-color bars, and a palette that lands flatter than intended.

The redesign keeps every existing system that works: pet animation, blink, mood expressions, per-species particles, glitch corruption, the ratatui app loop, the `UsageProvider` interface, calibration, evolution, decay, and the seeded pet generation. What changes is layout, framing, bar rendering, the contents of the today/feed/helpers panels, and the addition of a sparkline. Pet art templates are explicitly carried over unchanged from this redesign so the chrome rework can land independently of the larger pet-art initiative.

The reference visual is a 78-column outer frame in the existing accent color, with a pet-left / data-right two-column body. Sections inside the frame use horizontal `─ label ─` rules instead of nested boxes. Bars switch from solid fill to a 5-stop dark→bright gradient, with documented degradation paths for non-truecolor terminals.

## Depends On

This redesign reads, but does not change, the data model in `WatchViewModel`. It depends on the data correctness and source-health work tracked in `2026-05-09-glorp-core-mvp-repair-design.md` Plan 1 (Data Truth Pipeline). Specifically, the new today panel and helpers row read `current_bucket_effective_tokens`, `source_breakdown`, and `source_health.diagnostic_code` / `diagnostic_message` — these fields exist today but their content is being firmed up by the repair work. This redesign is the visual side of repair Plan 2 (Watch Presentation) and is gated on Plan 1 landing first.

## Goals

- Drop the fake terminal-inside-a-terminal chrome (traffic dots, fake `name@claude:~ -- 80x30` title bar, fake `drew@claude:~$ glorp watch` prompt line).
- Replace the flat layout with a single accent outer frame and a clear pet-left / data-right split inside it.
- Bring the watch view into compliance with the parent product spec: today panel becomes token-only with per-source breakdown and the current 10-minute window; commits, PRs, diffs, and lines-shipped never appear.
- Add a visible 7-day token sparkline using the existing `recent_daily_effective_tokens` view-model field.
- Add a helpers status row that surfaces `source_health.status` so blocked/degraded state is one glance away.
- Switch bar rendering from solid color to a 5-stop gradient ramp anchored to the existing `good` and `accent` palette colors, with terminal-capability degradation.
- Land all of the above as one cohesive PR. Pet art templates are not touched in this PR.

## Non-Goals

- Pet art templates and the `pet/` module — the 8×11 slot system, mood expressions, blink, particles, and glitch corruption all carry over unchanged. The pet-art rewrite is a separate follow-up initiative.
- Game mechanics, calibration, evolution, decay, ingestion, persistence — out of scope; the parent product spec already excludes everything else (themes, treats, achievements, graveyard, death, litter, command bar, onboarding) and that exclusion stands.
- Non-watch commands (`init`, `status`, `doctor`, `rename`, `reset`) — their CLI output stays as-is for this PR.
- Visual back-compat with the current frame.

## Architecture

The redesign is concentrated in `src/tui/`. The `pet/`, `game/`, `usage/`, and `storage/` modules are unchanged in behavior; one read-only query method may be added to `UsageStore` for the sparkline (see Sparkline section).

`WatchViewModel` already carries every field this redesign reads. No new fields are added. The construction site at `src/commands/watch.rs` is unchanged except for one read-path widening in the sparkline source (replacing the capped `recent_events(500)` walk with a date-bounded daily query).

`src/tui/style.rs` adds a `BarRamp { stops: [Color; 5] }` type and two ramp constants (one anchored to `good`, one anchored to `accent`). `BarRamp` values are passed to renderers by value; `SemanticStyles` is not extended with parallel "ramp style" fields.

`src/tui/layout.rs` is the bulk of the change. The `render_chrome()` function and the fake prompt line are deleted entirely. New helpers render the outer frame top, sides, and bottom. `render_wide()` is rewritten to lay out the inner grid inside the framed body. `render_compact()` is rewritten as a vertical stack with no outer frame. The pet panel keeps the pet art and vitals bars but drops the duplicated meta block (name/species/stage/mood/age) — those move into the frame title at the top.

## Layout

The reference width is 78 columns. The frame is exactly 78 cols wide; every body row ends at the same column.

Inside the 76-column body (`┃` + 76 + `┃`), the wide-mode grid is:

```
pad_left(2) + pet_col(26) + gap(2) + data_col(43) + pad_right(3) = 76
```

Each body row is rendered as a single full-width `Line` containing the leading `┃`, the inner content padded to exactly 76 cells, and the trailing `┃`. The renderer does not rely on `Paragraph` to pad short lines — it pads explicitly so the side `┃` columns connect cleanly top to bottom regardless of the inner content's natural width.

The pet column holds, top to bottom: a blank breath row, environmental flourish row(s), the pet art (currently 8×11), an optional ground row, a blank, a `─ vitals ─` rule, and four gradient bars (fed, happy, energy, xp). Until the bigger pet-art templates land, the 26-column panel is filled with deliberate environmental composition (sparse `·` flourishes above, a `,,,,,,,,,,,,,,,` ground line below) so the column does not read as empty space around the small pet.

The data column holds: `─ today ─` with four data rows, blank, `─ 7-day ─` with the sparkline, blank, `─ feed ─` with up to three event entries, blank, `─ helpers ─` with one status row.

The frame top row is `┏━ glorp · <name> the <species> · <age> · <mood> ━…━┓`, with the `━` fill computed so the total width matches the rendered frame width. Pet name length is bounded at the rename UX site (already a small string), so explicit overflow handling in the renderer is unnecessary.

The frame bottom row is `┗━ q quit · r refresh · ? help ━…━┛`. Mood, name, species, and age move from the old vitals meta block into the title — the meta block in the pet panel goes away.

### Stretch policy at widths above 78 cols

Pet column, gap, and outer-frame paddings stay fixed at 26 / 2 / 2 / 3. Surplus horizontal space is absorbed by the data column. The frame is not capped at 78; it stretches to the available terminal width, with `━` fills extending in the top and bottom rows. There is no centering — the frame anchors to the full terminal width.

### Compact mode

Compact mode triggers below 80 columns. There is no `COMPACT_WIDTH` constant in the current codebase — this redesign introduces compact mode. The threshold of 80 is the wide-mode frame minimum (78 cols of frame + 1 col safety margin on each side).

In compact mode the outer frame is dropped entirely. Sections stack vertically using the same `─ label ─` rules: pet, vitals, today, 7-day, feed, helpers. The footer (`q quit · r refresh · ? help`) renders as a plain unframed line at the bottom of the layout.

### Per-section heights

Wide mode body needs roughly 18 content rows plus 2 frame rows. The renderer assigns row counts by priority. Each section has a min, preferred, and max:

- pet panel: min 10, preferred 12, max 14 rows (pet art + vitals)
- today: min 5, preferred 5, max 5 rows (fixed)
- 7-day: min 2, preferred 2, max 2 rows (rule + sparkline)
- feed: min 2, preferred 4, max 8 rows (rule + N entries)
- helpers: min 2, preferred 2, max 2 rows

When extra vertical room is available, feed grows first (more entries shown). When the terminal is too short for the wide-mode minimum (about 20 rows), the renderer falls back to compact mode regardless of width. When even compact mode does not fit, drop priority is helpers → 7-day → feed entries → vitals labels.

## Bars

Bars are 12 cells wide. Empty cells are `░` rendered in `faint`. Filled cells are `█` colored from a 5-stop ramp.

Two ramps are defined as `BarRamp` constants in `style.rs`:

- Green ramp (`fed`, `energy`): `#3d6948 → #5a8462 → #82bc83 → #a8d690 → #d2eea2`
- Amber ramp (`happy`, `xp`): `#6e4516 → #b87a2c → #f0a646 → #ffc66e → #ffe0a8`

The middle stop in each ramp is the existing `good` / `accent` color from `tokenpet_palette()`. The ramps extend the existing palette rather than replace it. The two ramps map to the two semantic groups intentionally: green for vitals (fed, energy), amber for engagement signals (happy, xp).

`BarRamp` is passed to `bar_line()` by value. `SemanticStyles` is not extended with `bar_ramp_*` fields — the ramps are data, not styles, and are owned by the bar-rendering call sites.

For a bar with `N` filled cells where `N >= 2`, cell `i` uses ramp index `round(i * 4 / (N - 1))`. A single-cell fill (`N == 1`) uses ramp index 0 (darkest stop) to communicate "barely filled." A bar at 0% renders 12 faint `░` characters and never indexes into the ramp. A bar at 100% renders 12 ramp-graded `█` characters from `r0` to `r4`.

The bar line format is `  <label>  <bar(12)>  <value>` with a 6-character left-aligned label so `fed`, `happy`, `energy`, and `xp` all share a column. The value is the integer percent, no `%` suffix (values are 0–100 by definition).

### Color degradation

The bar ramp design assumes truecolor (24-bit RGB) terminal output. ratatui 0.29 emits `Color::Rgb` verbatim, but several ramp stops collapse to the same xterm-256 cube cell when the terminal silently quantizes (default macOS Terminal.app, tmux without `terminal-overrides=":Tc"`, Linux console).

Capability detection runs once at startup, reading `COLORTERM` and `TERM`:

- `COLORTERM=truecolor` or `=24bit`: render the RGB ramp as specified.
- otherwise (256-color): use a hand-picked `Color::Indexed` ramp where adjacent stops differ by at least 2 cube steps. The 256-color ramp is defined alongside the RGB ramp in `style.rs` and is selected by the same `BarRamp` based on detected capability.
- 16-color or no color (`TERM=dumb`, `NO_COLOR` env set): render filled cells as solid `good` / `accent` (no gradient). Empty cells stay `░` in `faint`. The bar still communicates fill level; the gradient is the casualty.

The 7-day sparkline (also gradient-colored) follows the same capability fallback chain.

## Today Panel

The today panel is rebuilt token-only. Commits, PRs, diff lines, and any other shipping signals never appear. The four rows are:

```
─ today ───────────────────────────────────
  tokens   412,847
  claude   287,140       70%
  codex    125,707       30%
  last 10m +8,420
```

- `tokens` is `WatchViewModel.today_effective_tokens` formatted with thousand separators. No delta-vs-yesterday annotation — a date-bounded yesterday total is not currently available from `UsageStore`, and adding that query is out of scope. If the data layer later exposes a `total_for_date(date)` query, a delta annotation can be added without changing the layout.
- `claude` and `codex` are the corresponding entries in `WatchViewModel.source_breakdown`, with the share-of-today percent computed at render time. If a source is absent from `source_breakdown` today, render `—` in `dim` for the value and percent.
- `last 10m` is `WatchViewModel.current_bucket_effective_tokens`, which is computed today as a trailing 10-minute sum (events in the last 600 seconds). The label "last 10m" reflects the trailing-window semantics. If `bucket_effective_tokens` later moves to bucket-aligned semantics under the repair-spec data work, the label changes to `this 10m` — that is a label change in one constant, not a layout change.

If a third source surface ever appears in `source_breakdown`, it is logged via `glorp doctor` but not rendered in the today panel for this PR. The today panel has four fixed rows; named sources beyond claude and codex are out of scope.

The panel reads from existing view-model fields. No new view-model fields are added. The renderer maps surface names from `UsageProvider` records (`claude-code`, `codex`) to the display names `claude` and `codex`.

## Sparkline

The sparkline lives in its own ruled section under today:

```
─ 7-day ───────────────────────────────────
       ▁   ▂   ▃   ▁   ▄   ▅   █
```

Seven cells, oldest on the left, today on the right. Heights are chosen by relative magnitude within the 7-day window using the 8-level glyph set `▁ ▂ ▃ ▄ ▅ ▆ ▇ █`. Days with zero tokens render `·` in `faint` to keep the column visible. When fewer than 7 days of history exist, left-pad with `·` in `faint`.

Each cell is colored by age using the **green** ramp from the bar palette: oldest cell at `g0`, today at `g4`, with the intermediate days walking up the ramp. Green is chosen over amber because token volume maps semantically to the "good" channel (more tokens = pet feeds), matching the green bars (fed, energy). The amber ramp stays paired with engagement metrics.

### Data source

`WatchViewModel.recent_daily_effective_tokens` is currently computed by walking `usage_store.recent_events(500)` and bucketing by date. The 500-event cap silently undercounts history for heavy users (≥ ~170 events/day). This redesign requires a date-bounded query directly against `daily_aggregates`.

`UsageStore` gains one new read-only method: `seven_day_token_history(now: OffsetDateTime) -> Vec<Option<f64>>` that returns seven values keyed by the seven calendar dates ending at `now`'s local date. None for days with no recorded usage. The construction in `commands/watch.rs` switches to this method. The `recent_events(500)` walk for the sparkline goes away. No schema change.

## Feed

The feed section keeps the existing `EventView` data — token deltas, evolution events, diagnostics — and just respaces them visually:

```
─ feed ────────────────────────────────────
  14:21  +52k tokens   claude
  14:18  evolution     pup → adult
  14:02  +18k tokens   codex
```

Up to three most recent events. Time in `faint`. Token deltas render as `+Nk tokens` or `+Nm tokens` in `good`, with the source name in `dim`. Evolution events render the literal word `evolution` in `accent` with the target stage in `dim`. Diagnostic and helper failures render the short message in `bad`.

Token formatting condenses to `Nk` and `Nm` with one decimal place when magnitude is below 10 of the unit.

## Helpers Status

The helpers section is one ruled row showing `source_health.status` for each provider helper:

```
─ helpers ─────────────────────────────────
  ccusage  ✓     codex  ✓
```

Status glyphs follow the existing `SourceStatus` enum directly:

- `Ready` → `✓` in `good`
- `Diagnostic` → `~` in `accent` (parsed but a non-fatal diagnostic is recorded)
- `Blocked` → `✗` in `bad`

The row is a maximum of two helpers wide (claude-code and codex). If a third surface appears, it is dropped from the helpers row for this PR — visibility goes through `glorp doctor` instead. There is no inline remediation hint line; users who see `✗` are directed to `glorp doctor` via the existing footer hint copy where applicable.

In compact mode, the helpers row is the last block in the vertical stack.

## Empty States

The renderer must handle these cases without panic and without breaking the frame:

- **Pre-init.** `glorp watch` invoked before `glorp init` returns the existing CLI error. The new framed layout is not involved. No change.
- **Post-init, pre-first-poll.** `today_effective_tokens = 0`, `recent_daily_effective_tokens = []`, `source_breakdown = []`, `current_bucket_effective_tokens = 0`. Render: `tokens` row shows `0`; `claude` and `codex` rows show `—`; `last 10m` shows `+0` in `dim`; sparkline shows seven `·` cells in `faint`; helpers shows `—` in `dim` until first source-health observation.
- **All helpers blocked.** Today panel and sparkline show last-known values from view-model state (the watch loop preserves the last good model on poll failure). Helpers row shows `✗` glyphs. Frame chrome is unchanged.
- **Helper degraded for one source only.** That source's row in the today panel renders `—`; the helpers row glyph for that source is `~`; the other source renders normally.

## Code Organization

`src/tui/style.rs`:

- Add `BarRamp { stops: [Color; 5] }`, `BAR_RAMP_GOOD`, `BAR_RAMP_ACCENT` (truecolor stops) and matching 256-color and 16-color fallback definitions.
- Add color-capability detection (probably `color_capability() -> ColorCapability`) reading `COLORTERM`, `TERM`, `NO_COLOR`.
- Remove `filled_bar_good`, `filled_bar_accent` (replaced by `BarRamp`).
- Remove `chrome_title`, `prompt_user`, `prompt_path`, `prompt_sep` (no longer used after the chrome row and fake prompt are deleted).

`src/tui/layout.rs`:

- Delete `render_chrome()` and the fake prompt-line render.
- Add `render_frame_top()`, `render_frame_bottom()`, `render_frame_sides()`.
- Rewrite `render_wide()` to lay out the inner 26/2/43 grid; render each body row as a full-width `Line` with leading and trailing `┃`.
- Rewrite `render_compact()` to drop the frame and stack vertically.
- Simplify `render_pet_panel()` to render art + ground + vitals only (meta block deleted).
- Add `render_today_panel()`, `render_sparkline_row()`, `render_feed_panel()`, `render_helpers_panel()`.
- Rewrite `bar_line()` to take a `BarRamp` and emit per-cell `Span`s.
- Extend `section_line()` to take a target width.
- Introduce `COMPACT_WIDTH = 80` and a wide/compact branch in `render_watch_frame()`.

`src/tui/view_model.rs`: no changes. The fields needed already exist.

`src/tui/app.rs`: no changes beyond whatever wiring follows from the layout rewrite.

`src/commands/watch.rs`: change one call site so the sparkline reads from the new `seven_day_token_history()` query rather than walking `recent_events(500)`.

`src/storage/usage_store.rs`: add `seven_day_token_history(now: OffsetDateTime) -> Result<Vec<Option<f64>>>`. Read-only, queries the existing `daily_aggregates` rows by date.

If `layout.rs` grows past about 25k after these changes, peeling each `render_*_panel()` into its own file under `src/tui/panels/` is reasonable as a follow-up. Not part of this PR.

## Error Handling

Helper failures are visible in two places: the source row in the today panel renders `—` instead of a number when its source is unhealthy, and the helpers row glyph reflects the underlying `SourceStatus`. The watch loop already preserves the last good view-model on poll failure; the renderer treats the model as authoritative and does not blank panels on transient failure.

The frame renders even when the body is degraded — a blocked helper does not break the layout, it just shows up in the helpers row and turns affected today rows into `—` placeholders.

Width below the compact threshold (80 cols) falls back cleanly to the un-framed vertical stack. Height below the wide-mode minimum (~20 rows) also falls back to compact, which itself drops sections by priority when even compact does not fit.

## Unicode and Terminal Compatibility

The frame uses box-drawing characters (`┏━┓┗┛┃─`) which are East-Asian-Ambiguous in UAX#11. Terminals running under a CJK locale or with a CJK-wide font may render these at width 2, breaking the column-alignment invariant. This is a known trade-off; an ASCII fallback frame is out of scope for this PR. Most users are not on a CJK locale and the frame is consistent for them.

The bar uses `█` and `░` (block elements). `░` shows visible inter-cell gaps in some Windows fonts but is still readable. The 8-level sparkline glyph set `▁▂▃▄▅▆▇█` shows occasional baseline gaps for `▁`/`▂` on older macOS Terminal.app at small sizes; readability is preserved.

Title segments and right-aligned annotations (e.g. `~`, `↑` if added later, `…`, `·`) are all BMP characters with reliable width-1 in modern monospace fonts. Title-width math uses `chars().count()`; if `unicode-width` becomes necessary later (for example, to support emoji in pet names), it can be added without spec changes.

## Testing Strategy

Existing TUI tests in this repo are hand-written buffer-text assertions, not snapshot files. Affected tests:

- `tests/tui_render.rs` — about 15 assertions on literal strings (`"glorp --"`, `"╎"`, `"┄"`, separate `name`/`species`/`stage`/`mood` label rows, `"sources"`, `"log"`, `"stats"`, the chrome traffic dot `●`). All of these need surgical rewrites for the new layout — they are part of this PR, not a follow-up.
- `tests/style_tokens.rs` — asserts `chrome_title`, `prompt_user`, `prompt_path` styles still exist on `SemanticStyles`. Those style fields are being deleted, so these assertions are deleted too.
- `tests/watch_integration.rs` — verify against the new layout; rewrite assertions that target removed structure.

New unit tests:

- `style.rs` — bar ramp index function: 0 / 1 / N=12 fill levels; `N==1` produces index 0; ramp index never overflows; capability detection returns expected variant for each `COLORTERM`/`TERM` combination.
- `layout.rs` — section header line generator produces exact target width regardless of label length; pad characters are `─`.
- `layout.rs` — wide-mode column math: pet column + gap + data column + pads sum to 76 at the reference 78-column width; surplus at wider widths lands in the data column.
- `view_model.rs` — happy/fed/energy/xp percent rounding; share-of-today computation handles zero `today_effective_tokens` without divide-by-zero.

Width fixtures: snapshot-style buffer comparisons use `TestBackend` at fixed widths 80×30 and 100×40, with truecolor capability forced for stability. The 60×16 compact case uses a dedicated test that asserts only on structure (no chrome present, sections stacked vertically).

Manual verification before merging: run `glorp watch` against real local usage at 80, 100, and 60 column widths, in (1) a truecolor terminal and (2) tmux without `terminal-overrides=":Tc"`. Verify frame characters connect cleanly top to bottom, gradient is visible in truecolor and degrades to indexed-256 readably in tmux, helpers row reflects actual `source_health` values, and the 80-column threshold flips cleanly without flicker.

This redesign does not test pet art quality. The 8×11 templates carry over unchanged.

## References

- Parent product spec: `docs/superpowers/specs/2026-05-08-glorp-design.md`
- Repair spec (this work depends on Plan 1): `docs/superpowers/specs/2026-05-09-glorp-core-mvp-repair-design.md`
- Original handoff: `docs/tokenpet/README.md`
- Verified mockup: `.superpowers/brainstorm/31764-1778396947/content/hybrid-v4.html`
