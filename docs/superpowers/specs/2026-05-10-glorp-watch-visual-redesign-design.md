# Glorp Watch Visual Redesign

Date: 2026-05-10

## Overview

Rewrite the visual layer of `glorp watch` to fix five concrete pain points with the current TUI: fake terminal chrome inside an already-real terminal, weak layout hierarchy, prototype-derived stats panel that violates the parent product spec by showing commits/PRs/diffs, flat-color bars, and a palette that lands flatter than intended.

The redesign keeps every existing system that works: pet animation, blink, mood expressions, per-species particles, glitch corruption, the ratatui app loop, the `UsageProvider` interface, calibration, evolution, decay, and the seeded pet generation. What changes is layout, framing, bar rendering, the contents of the today/feed/helpers panels, and the addition of a sparkline. Pet art templates are explicitly carried over unchanged from this redesign so the chrome rework can land independently of the larger pet-art initiative.

The reference visual is a 78-column outer frame in the existing accent color, with a pet-left / data-right two-column body. Sections inside the frame use horizontal `─ label ─` rules instead of nested boxes. Bars switch from solid fill to a 5-stop dark→bright gradient, with documented degradation paths for non-truecolor terminals.

## Relationship to other work

This redesign reads, but does not change, the data model in `WatchViewModel`. The fields it depends on (`today_effective_tokens`, `source_breakdown`, `source_health.diagnostic_*`, `current_bucket_effective_tokens`, `recent_daily_effective_tokens`) all exist and produce sensible values today; this PR is **not blocked** on the repair work in `2026-05-09-glorp-core-mvp-repair-design.md`.

The repair spec's Plan 1 (Data Truth Pipeline) improves the *correctness* of those fields under failure modes (write-boundary atomicity, calibration grouping, smearing). When Plan 1 lands, this redesign keeps working without changes — the existing field shapes are stable. If this redesign ships first, users see the new chrome immediately and benefit from any subsequent data-correctness improvements automatically.

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

The body-row builder takes a `Vec<Span>` of inner content (not a pre-built `Line`), measures the visible width, pads with blank `Span`s to 76 cells, and prepends/appends the framing `┃` `Span`s. Section-rule rows (`─ vitals ─…`, `─ today ─…`, etc.) go through the same builder: `section_line()` is refactored to return its inner spans as `Vec<Span>` rather than a finished `Line`, and the body-row builder wraps them. Rules are never rendered in isolation, and the body-row builder is the single source of truth for `┃` placement.

The pet column holds, top to bottom: a blank breath row, environmental flourish row(s), the pet art (currently 11×8 — 11 wide, 8 tall), an optional ground row, a blank, a `─ vitals ─` rule, and four gradient bars (fed, happy, energy, xp). Until the bigger pet-art templates land, the 26-column panel is filled with deliberate environmental composition (sparse `·` flourishes above, a `,,,,,,,,,,,,,,,` ground line below) so the column does not read as empty space around the small pet.

The current pet art is 11 columns wide × 8 rows tall (per `src/pet/art.rs` templates). It is centered horizontally inside the 26-column panel: 7 columns of left-pad, 11 columns of art, 8 columns of right-pad (the extra column on the right is a deliberate, minor asymmetry — splitting evenly is impossible with even/odd width parity). The pad cells are blank (no fill), keeping the art floating without visual noise. When larger pet-art templates ship, the same 26-column panel hosts a wider canvas with smaller centered margins.

The data column holds: `─ today ─` with four data rows, blank, `─ 7-day ─` with the sparkline, blank, `─ feed ─` with up to three event entries, blank, `─ helpers ─` with one status row.

The frame top row is `┏━ glorp · <name> the <species> · <age> · <mood> ━…━┓`, with the `━` fill computed so the total width matches the rendered frame width. The renderer truncates the pet name to 16 characters with a trailing `…` if the title would overflow available frame width. There is no length cap at the rename UX today, so the truncation lives in the renderer rather than relying on an absent invariant.

The frame bottom row is `┗━ q quit · r refresh · ? help ━…━┛`. Mood, name, species, and age move from the old vitals meta block into the title — the meta block in the pet panel goes away.

### Stretch policy at widths above 78 cols

Pet column, gap, and outer-frame paddings stay fixed at 26 / 2 / 2 / 3. Surplus horizontal space is absorbed by the data column. The frame is not capped at 78; it stretches to the available terminal width, with `━` fills extending in the top and bottom rows. There is no centering — the frame anchors to the full terminal width.

### Compact mode

Compact mode triggers below 80 columns; the constant is `COMPACT_THRESHOLD = 80`. The threshold is chosen as the wide-mode frame minimum (78 cols of frame + 1 col safety margin on each side).

In compact mode the outer frame is dropped entirely. Sections stack vertically using the same `─ label ─` rules: pet, vitals, today, 7-day, feed, helpers. The footer (`q quit · r refresh · ? help`) renders as a plain unframed line at the bottom of the layout.

Compact mode's preferred row budget is roughly 22 rows (pet 10 + vitals 4 + today 5 + 7-day 2 + feed 4 + helpers 2 + section gaps + footer). When terminal height is below that, the renderer computes which sections fit *before* building the ratatui constraints vector — sections are added to the layout in priority order until vertical room is exhausted. Drop priority (last to first): helpers, then 7-day, then feed entries down to 1, then today rows down to 2 (`tokens` and `last 10m` only), then vitals labels collapse to a single summary line. Below ~10 rows, render only the pet art and a one-line vitals summary in the format `fed N · happy N · energy N · xp N` (no bars, just numbers). At height 1, render only the vitals summary (pet art is dropped). The renderer must not panic at any height ≥ 1.

### Per-section heights

Wide mode body needs roughly 18 content rows plus 2 frame rows. Three sections are fixed; two flex:

- today: 5 rows (fixed)
- 7-day: 2 rows (fixed; rule + sparkline)
- helpers: 2 rows (fixed)
- pet panel: min 10, preferred 12, max 14 rows (pet art + vitals; flexes with art size)
- feed: min 2, preferred 4, max 8 rows (rule + 1–7 entries)

Extra vertical room is absorbed by feed first (more entries shown), then by pet panel (more breathing room). Section ordering uses `Constraint::Length` for fixed sections (today, 7-day, helpers), `Constraint::Max(14)` for the pet panel so it caps at its preferred max, and `Constraint::Min(2)` for feed so all residual flows there once pet has reached its cap. ratatui distributes proportionally between unbounded `Min` constraints; capping pet with `Max` is what produces feed-first absorption.

When the terminal is too short for the wide-mode minimum (≈20 rows), the renderer falls back to compact mode regardless of width. Compact's own height-degradation rules (above) take over.

## Bars

Bars are 12 cells wide. Empty cells are `░` rendered in `faint`. Filled cells are `█` colored from a 5-stop ramp.

Two ramps are defined as `BarRamp` constants in `style.rs`:

- Green ramp (`fed`, `energy`): `#3d6948 → #5a8462 → #82bc83 → #a8d690 → #d2eea2`
- Amber ramp (`happy`, `xp`): `#6e4516 → #b87a2c → #f0a646 → #ffc66e → #ffe0a8`

The middle stop in each ramp is the existing `good` / `accent` color from `tokenpet_palette()`. The ramps extend the existing palette rather than replace it. The two ramps map to the two semantic groups intentionally: green for vitals (fed, energy), amber for engagement signals (happy, xp).

`BarRamp` is passed to `bar_line()` by value. `SemanticStyles` is not extended with `bar_ramp_*` fields — the ramps are data, not styles, and are owned by the bar-rendering call sites.

For a bar with `N` filled cells where `N >= 2`, cell `i` uses ramp index `(round((i as f64) * 4.0 / ((N - 1) as f64)) as usize).min(4)` — the multiply and divide are in `f64` so the curve doesn't degrade to integer-division steps; the `.min(4)` is a defensive cap against floating drift. A single-cell fill (`N == 1`) uses ramp index 0 (darkest stop). A bar at 0% renders 12 faint `░` characters and never indexes into the ramp. A bar at 100% renders 12 ramp-graded `█` characters from `r0` to `r4`.

The bar line format is `  <label>  <bar(12)>  <value>` with a 6-character left-aligned label so `fed`, `happy`, `energy`, and `xp` all share a column. The value is the integer percent, no `%` suffix (values are 0–100 by definition).

### Color degradation

The bar ramp design assumes truecolor (24-bit RGB) terminal output. ratatui 0.29 emits `Color::Rgb` verbatim. On non-truecolor terminals (default macOS Terminal.app, tmux without `terminal-overrides=":Tc"`, Linux console, anything with `NO_COLOR` set), several ramp stops would collapse to the same xterm-256 cell — so the redesign uses a binary fallback:

- **Truecolor** (detected via `COLORTERM=truecolor` or `COLORTERM=24bit`): render the RGB ramp as specified.
- **Anything else** (no `COLORTERM`, `TERM=dumb`, or `NO_COLOR` set): render filled cells as the solid existing `good` / `accent` color (no gradient). Empty cells stay `░` in `faint`. The bar still communicates fill level; the gradient is the casualty.

A middle 256-color tier was considered and rejected: it requires hand-tuning a separate ramp, capability tests for a tier most users on tmux-without-`Tc` would tolerate flat anyway, and it does not appreciably improve the experience over solid fill.

Capability detection runs once at startup. A new `color_capability: ColorCapability` field is added to `WatchAppConfig` (snapshotted from environment at construction). `render_watch_frame()`'s signature gains a `capability: ColorCapability` parameter (or takes `&WatchAppConfig`); the value is threaded down to bar/sparkline rendering. It is **not** stored as a module-static `OnceCell` — that would make unit tests pollute each other when they want to exercise different capability variants. Capability does not change per-frame and does not need to react to `SIGWINCH`.

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
- `claude` and `codex` are the corresponding entries in `WatchViewModel.source_breakdown`, with the share-of-today percent computed at render time. The renderer carries a hardcoded ordered list of expected surfaces — `[("claude-code", "claude"), ("codex", "codex")]` — defined as a constant in `layout.rs`. The today panel always renders one row per expected surface; if a surface is absent from `source_breakdown`, render `—` in `dim` for the value and percent. Both this list and the same mapping are reused by the helpers row.
- `last 10m` is `WatchViewModel.current_bucket_effective_tokens`, which is computed today as a trailing 10-minute sum (events in the last 600 seconds). The label "last 10m" reflects the trailing-window semantics. This redesign owns the trailing-vs-aligned choice; it is not deferred to other work.

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

`WatchViewModel.recent_daily_effective_tokens` is currently computed by walking `usage_store.recent_events(500)` and bucketing by date. The 500-event cap silently undercounts history for heavy users (≥ ~170 events/day). This redesign replaces the walk with a direct aggregation query.

`UsageStore` gains one new read-only method: `seven_day_token_history(now_utc_date: time::Date) -> Vec<f64>` that returns exactly seven values, one per calendar date in the window `[now_utc_date - 6, now_utc_date]`, oldest first. Days with no recorded usage produce `0.0` (matching the existing field's shape; the sparkline already renders zero as `·` in `faint`).

The query groups `usage_events` by the existing `period_date` column, which is populated at insert time as `period_start.date().to_string()` — pure UTC, matching `today_effective_tokens()`'s `WHERE period_date = ?1` pattern. The query reads from `usage_events`, not `daily_aggregates`: `daily_aggregates` is only populated by `compact_before(cutoff)`, which has no caller in the repo today, so recent days live in `usage_events`. Pure-UTC bucketing is a deliberate choice — the entire codebase records and reads dates in UTC; introducing local-tz handling here would require the `time` crate's `local-offset` feature and additional soundness work, all of which is out of scope. The caller passes `OffsetDateTime::now_utc().date()`.

The `recent_events(500)` walk for the sparkline goes away. No schema change.

## Feed

The feed section keeps the existing `EventView` data — token deltas, evolution events, diagnostics — and just respaces them visually:

```
─ feed ────────────────────────────────────
  14:21  +52k tokens   claude
  14:18  evolution     pup → adult
  14:02  +18k tokens   codex
```

Up to three most recent events, displayed in the order produced by the watch loop's `build_recent_events()` (state events, then usage events, then diagnostics). `EventView.timestamp` is a `String` formatted as `"HH:MM"` with no date component, so reliable chronological sorting in the renderer is not possible — keeping the existing constructor ordering avoids a fragile time-string sort that would break across midnight. If chronological ordering becomes important later, the upstream constructor is the place to fix it. Time in `faint`. Token deltas render as `+Nk tokens` or `+Nm tokens` in `good`, with the source name in `dim`. Evolution events render the literal word `evolution` in `accent` with the target stage in `dim`. Diagnostic and helper failures render the short message in `bad`.

Token formatting condenses to `Nk` and `Nm` with one decimal place when magnitude is below 10 of the unit.

## Helpers Status

The helpers section is one ruled row showing `source_health.status` for each provider helper, using the same display-name list as the today panel (`claude-code → claude`, `codex → codex`):

```
─ helpers ─────────────────────────────────
  claude  ✓     codex  ✓
```

Status glyphs follow the existing `SourceStatus` enum directly:

- `Ready` → `✓` in `good`
- `Diagnostic` → `~` in `accent` (parsed but a non-fatal diagnostic is recorded)
- `Blocked` → `✗` in `bad`

The row is a maximum of two helpers wide (claude-code and codex). If a third surface appears, it is dropped from the helpers row for this PR — visibility goes through `glorp doctor` instead. There is no inline remediation hint line; users who see `✗` are directed to `glorp doctor` via the existing footer hint copy where applicable.

In compact mode, the helpers row is the last block in the vertical stack.

## Empty / cold-start state

`glorp watch` invoked before `glorp init` returns the existing CLI error path; the new framed layout is not involved.

Post-init with no usage data yet (or a user genuinely at zero today) renders identically: `tokens` row shows `0`; `claude` and `codex` rows show `—`; `last 10m` shows `+0` in `dim`; sparkline shows seven `·` cells in `faint`; helpers row shows whatever `source_health` carries (`—` if the slice is empty, ✓/~/✗ once helpers have been queried). The renderer does not try to distinguish "first poll hasn't happened" from "real zero" — the view-model carries no flag for that and the visual is the same either way.

Other failure modes (helper blocked, source degraded, partial outage) are handled by the rendering rules already specified in Today Panel and Helpers Status — they don't need separate empty-state treatment.

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
- Introduce `COMPACT_THRESHOLD = 80` and a wide/compact branch in `render_watch_frame()`.

`src/tui/view_model.rs`: no changes. The fields needed already exist.

`src/tui/app.rs`: add `color_capability: ColorCapability` to `WatchAppConfig`, populated at construction from `style::detect_color_capability()`. Update `render_watch_frame()`'s signature to accept the capability (passed through from the app loop). Other event-loop and key-handling logic is unchanged.

`src/commands/watch.rs`: change one call site so the sparkline reads from the new `seven_day_token_history()` query rather than walking `recent_events(500)`.

`src/storage/usage_store.rs`: add `seven_day_token_history(now_local_date: time::Date) -> Result<Vec<f64>>`. Read-only, queries `usage_events` grouped by `period_start::date`. Returns exactly seven values, oldest first; days with no events produce `0.0`.

## Error Handling

Helper failures are visible in two places: the source row in the today panel renders `—` instead of a number when its source is unhealthy, and the helpers row glyph reflects the underlying `SourceStatus`. The watch loop already preserves the last good view-model on poll failure; the renderer treats the model as authoritative and does not blank panels on transient failure.

The frame renders even when the body is degraded — a blocked helper does not break the layout, it just shows up in the helpers row and turns affected today rows into `—` placeholders.

Width below the compact threshold (80 cols) falls back cleanly to the un-framed vertical stack. Height below the wide-mode minimum (~20 rows) also falls back to compact, which itself drops sections by priority when even compact does not fit.

## Unicode and Terminal Compatibility

The frame uses box-drawing characters (`┏━┓┗┛┃─`) which are East-Asian-Ambiguous in UAX#11. Terminals running under a CJK locale or with a CJK-wide font may render these at width 2, breaking the column-alignment invariant. This is a known trade-off; an ASCII fallback frame is out of scope for this PR. Most users are not on a CJK locale and the frame is consistent for them.

The bar uses `█` and `░` (block elements). `░` shows visible inter-cell gaps in some Windows fonts but is still readable. The 8-level sparkline glyph set `▁▂▃▄▅▆▇█` shows occasional baseline gaps for `▁`/`▂` on older macOS Terminal.app at small sizes; readability is preserved.

Title segments and right-aligned annotations (e.g. `~`, `↑` if added later, `…`, `·`) are all BMP characters with reliable width-1 in modern monospace fonts. Title-width math uses `chars().count()`; if `unicode-width` becomes necessary later (for example, to support emoji in pet names), it can be added without spec changes.

## Testing Strategy

Existing TUI tests in this repo are hand-written buffer-text assertions, not snapshot files. Affected tests (counted from `tests/tui_render.rs` directly):

- `tests/tui_render.rs` — roughly 30 literal-string assertions destined to break: `"glorp --"`, `"─ vitals"`, `"sources"`, `"log"`, `"stats"`, `"bucket"`, `"╎"`, `"┄"`, `"✦ ✧ ✦"`, `"▏"`, the chrome traffic dot `●`, separate `name`/`species`/`stage`/`mood` label rows, and `"fed"`/`"happy"`/`"energy"`/`"xp"` literal positions. Three positional ordering tests (`wide_layout_keeps_pet_and_stats_top_stacked_*`, `wide_layout_leads_with_pet_before_vitals_metadata`, `pet_art_and_vitals_metadata_have_no_blank_rows_between_them`) encode the obsolete pet/vitals/stats stacking — these tests are **deleted**, not rewritten, because the new layout has no `stats` section. Other tests get surgical rewrites against new section names (`vitals`, `today`, `feed`, `helpers`, `7-day`).
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
- Repair spec (related, not blocking): `docs/superpowers/specs/2026-05-09-glorp-core-mvp-repair-design.md`
- Original handoff: `docs/tokenpet/README.md`
- Verified mockup: `.superpowers/brainstorm/31764-1778396947/content/hybrid-v4.html`
