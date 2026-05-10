# Glorp Watch Visual Redesign

Date: 2026-05-10

## Overview

Rewrite the visual layer of `glorp watch` to fix five concrete pain points with the current TUI: fake terminal chrome inside an already-real terminal, weak layout hierarchy, prototype-derived stats panel that violates the parent spec by showing commits/PRs/diffs, flat-color bars, and a palette that lands flatter than intended.

The redesign keeps every existing system that works: pet animation, blink, mood expressions, per-species particles, glitch corruption, the ratatui app loop, the `UsageProvider` interface, calibration, evolution, decay, and the seeded pet generation. What changes is layout, framing, bar rendering, the contents of the today/feed/helpers panels, and the sparkline. Pet art templates are explicitly carried over unchanged from this redesign so the chrome rework can land independently of the larger pet-art initiative.

The reference visual is a 78-column outer frame in the existing accent color, with a pet-left / data-right two-column body. Sections inside the frame use horizontal `─ label ─` rules instead of nested boxes. Bars switch from solid fill to a 5-stop dark→bright gradient.

## Goals

- Drop the fake terminal-inside-a-terminal chrome (traffic dots, fake `name@claude:~ -- 80x30` title bar, fake `drew@claude:~$ glorp watch` prompt line).
- Replace the flat layout with a single accent outer frame and a clear pet-left / data-right split inside it.
- Bring the watch view into compliance with `2026-05-08-glorp-design.md`: today panel becomes token-only with per-source breakdown and the current 10-minute bucket; commits, PRs, diffs, and lines-shipped never appear.
- Add a real 7-day token sparkline (currently absent) using the existing usage history.
- Add a helpers status row reflecting `SourceStatus` so blocked-state visibility is one glance away.
- Switch bar rendering from solid color to a 5-stop gradient ramp anchored to the existing `good` and `accent` palette colors.
- Land all of the above as one cohesive PR. Pet art templates are not touched in this PR.

## Non-Goals

- Do not redesign pet art templates. The 8×11 slot system, mood expressions, blink behavior, particle overlays, and glitch corruption stay exactly as they are. Pet art quality is the subject of a separate follow-up initiative.
- Do not change game mechanics, calibration, evolution, decay, or the `UsageProvider` contract.
- Do not introduce themes, treats, achievements, graveyard, death overlays, litter pickers, command bar, or onboarding chrome — all explicitly excluded by the parent spec.
- Do not redesign `init`, `status`, `doctor`, `rename`, or `reset` rendering. Their output stays as-is for this PR.
- Do not change persistence, storage schema, or usage-ingestion code paths beyond adding read-only query methods needed for the new panels.
- Do not preserve any visual back-compat with the current frame. Every snapshot test that asserts on the old chrome is expected to churn.

## Architecture

The redesign is concentrated in `src/tui/`. The pet, game, usage, and storage modules are unchanged in behavior; storage may gain narrow read-only query methods for new view-model fields (no schema change, no new tables).

`src/tui/style.rs` adds a `BarRamp` type and two ramp constants (one anchored to `good`, one anchored to `accent`). `SemanticStyles` exposes the ramps. The existing solid-fill bar styles are removed so there is one bar-rendering API, not two.

`src/tui/layout.rs` is the bulk of the change. The `render_chrome()` function and the fake prompt line are removed. New helpers render the outer frame top, sides, and bottom. `render_wide()` is rewritten to lay out the inner grid inside the framed body. `render_compact()` keeps a vertical stack but drops the outer frame entirely below 80 columns. The pet panel keeps the pet art and vitals bars but drops the duplicated meta block (name/species/stage/mood/age) — those move into the frame title at the top. New section renderers cover today, sparkline, feed, and helpers. The `─ label ─` section header gains an explicit width parameter so each rule fills its column exactly.

`src/tui/view_model.rs` gains the fields the new panels need: today total, today delta vs yesterday, today per-source breakdown, current 10-minute bucket delta, and a 7-day token history. View-model construction reads these from the existing `usage` rollups; no new ingestion paths.

`src/tui/app.rs` only needs whatever wiring the new view-model fields require. The event loop and key handling are unchanged.

## Layout

The reference width is 78 columns. The frame is exactly 78 cols wide; every body row ends at the same column.

Inside the 76-column body (`┃` + 76 + `┃`), the wide-mode grid is:

```
pad_left(2) + pet_col(26) + gap(2) + data_col(43) + pad_right(3) = 76
```

The pet column holds, top to bottom: a blank breath row, environmental flourish row(s), the pet art (currently 8×11, with room for ~9×24 once the art initiative ships), an optional ground row, a blank, a `─ vitals ─` rule, and four gradient bars (fed, happy, energy, xp). The data column holds: `─ today ─` with four data rows, blank, `─ 7-day ─` with the sparkline, blank, `─ feed ─` with up to three event entries, blank, `─ helpers ─` with one status row.

The frame top row is `┏━ glorp · <name> the <species> · <age> · <mood> ━…━┓`, with the `━` fill computed so the total width is exactly 78. The frame bottom row is `┗━ q quit · r refresh · ? help ━…━┛`. Mood, name, species, and age move from the old vitals meta block into the title — the meta block in the pet panel goes away.

Compact mode triggers below 80 columns. The constant currently called `COMPACT_WIDTH` moves from 72 to 80. In compact mode the outer frame is dropped (frame chrome at narrow widths fights the data for space). Sections stack vertically using the same `─ label ─` rules: pet, vitals, today, 7-day, feed, helpers. Bars and ramps work the same in both modes.

The wide mode requires roughly 18 content rows plus two frame rows. If terminal height is below that, fall back to compact mode (the existing height-based degradation in the pet panel applies as today).

## Bars

Bars are 12 cells wide (current is 20). The narrower bar fits comfortably in the 26-column pet panel and matches the verified mockup. Empty cells are `░` rendered in `faint`. Filled cells are `█` colored from a 5-stop ramp.

Two ramps are defined:

- Green ramp (`fed`, `energy`): `#3d6948 → #5a8462 → #82bc83 → #a8d690 → #d2eea2`
- Amber ramp (`happy`, `xp`): `#6e4516 → #b87a2c → #f0a646 → #ffc66e → #ffe0a8`

The middle stop in each ramp is the existing `good` / `accent` color from `tokenpet_palette()`. The ramps extend the palette rather than replace it.

For a bar with `N` filled cells, cell `i` uses ramp index `round(i * 4 / max(N-1, 1))`. A single-cell fill uses the middle stop. A bar at 0% renders 12 faint `░` characters. A bar at 100% renders 12 ramp-graded `█` characters from `r0` to `r4`.

The bar line format is `  <label>  <bar(12)>  <value>` with a 6-character left-aligned label so `fed`, `happy`, `energy`, and `xp` all share a column. The value is the integer percent, no `%` suffix (values are 0–100 by definition).

## Today Panel

The today panel is rebuilt token-only. Commits, PRs, diff lines, and any other shipping signals never appear. The four rows are:

```
─ today ───────────────────────────────────
  tokens   412,847       ↑ 22.9%
  claude   287,140       70%
  codex    125,707       30%
  bucket   +8,420        this 10m
```

- `tokens` is total effective tokens for today across all sources. The right-side annotation is the delta vs yesterday: `↑ N%` in `good`, `↓ N%` in `bad`, `—` in `dim` when yesterday is unknown or zero.
- `claude` is the claude-code surface effective-token total today, paired with its share of today's total as a percent. If the source is unhealthy, the value renders as `—` and the corresponding helpers-row glyph carries the failure.
- `codex` mirrors `claude` for the codex surface. Sources beyond claude and codex collapse into `other` if any appear; this is graceful, not a crash.
- `bucket` is the effective-token delta in the current ~10-minute bucket from the metabolism layer described in the parent spec. The right-side label is `this 10m`. With no activity yet in the current bucket, render `+0` in `dim`.

The view-model additions for this panel are: `today_total: u64`, `today_delta_vs_yesterday: Option<f32>`, `today_by_source: Vec<(SourceName, u64, f32)>`, `current_bucket_tokens: i64`, `current_bucket_label: String`. All of these read from existing `usage` rollups and storage queries; storage may gain read-only methods (`today_by_source()`, `current_bucket_delta()`) but not schema changes.

Source surface names from `UsageProvider` records (`claude-code`, `codex`) are mapped to the display names `claude` and `codex` in the renderer.

## Sparkline

The sparkline lives in its own ruled section under today:

```
─ 7-day ───────────────────────────────────
       ▁   ▂   ▃   ▁   ▄   ▅   █
```

Seven cells, oldest on the left, today on the right. Heights are chosen by relative magnitude within the 7-day window using the 8-level glyph set `▁ ▂ ▃ ▄ ▅ ▆ ▇ █`. Days with zero tokens render `·` in `faint` to keep the column visible. When fewer than 7 days of history exist, left-pad with `·` in `faint`.

Each cell is colored by age using the amber ramp: oldest cell at `a0`, today at `a4`, with the intermediate days walking up the ramp. This carries the same visual idea as the bars (dim past, bright present) and reuses the existing palette.

The view-model addition is `seven_day_history: [Option<u64>; 7]`. The reading logic queries existing usage rollups; if the storage layer does not already expose a 7-day-by-day token sum, add a read-only `seven_day_token_history()` method.

## Feed

The feed section keeps the existing `EventView` data — token deltas, evolution events, diagnostics — and just respaces them visually:

```
─ feed ────────────────────────────────────
  14:21  +52k tokens   claude
  14:18  evolution     pup → adult
  14:02  +18k tokens   codex
```

Three most recent events maximum. Time in `faint`. Token deltas render as `+Nk tokens` or `+Nm tokens` in `good`, with the source name in `dim`. Evolution events render the literal word `evolution` in `accent` with the target stage in `dim`. Diagnostic and helper failures render the short message in `bad`.

Token formatting in the feed condenses to `Nk` and `Nm` to keep entries on one line; precision is one decimal place when the magnitude is below 10 of the unit.

## Helpers Status

The helpers section is one ruled row showing live `SourceStatus` for each provider helper:

```
─ helpers ─────────────────────────────────
  ccusage  ✓     codex  ✓
```

Status glyphs:

- `✓` in `good` — healthy.
- `!` in `accent` — degraded (parsed but stale, or partial output).
- `✗` in `bad` — blocked, missing, or returning unparsable output.

When any helper is `✗`, a second body line below the status row carries a short remediation hint in `bad`, for example `! ccusage not found · run glorp doctor`. Hints come from the existing `SourceStatus` data; they are not invented at render time.

In compact mode, the helpers row is the last block in the vertical stack.

## Code Organization

- `src/tui/style.rs` — add `BarRamp { stops: [Color; 5] }`, `BAR_RAMP_GOOD`, `BAR_RAMP_ACCENT`. Add `bar_ramp_good` and `bar_ramp_accent` to `SemanticStyles`. Remove `filled_bar_good` and `filled_bar_accent` once `bar_line()` is migrated.
- `src/tui/layout.rs` — delete `render_chrome()` and the fake prompt-line render. Add `render_frame_top()`, `render_frame_bottom()`, `render_frame_sides()`. Rewrite `render_wide()` to lay out the inner 26/2/43 grid. Rewrite `render_compact()` to drop the frame and stack vertically. Simplify `render_pet_panel()` to render art + ground + vitals only (meta block deleted). Add `render_today_panel()`, `render_sparkline_row()`, `render_feed_panel()`, `render_helpers_panel()`. Rewrite `bar_line()` to take a `BarRamp` and emit per-cell spans. Extend `section_line()` to take a target width. Move `COMPACT_WIDTH` from 72 to 80.
- `src/tui/view_model.rs` — add `today_total`, `today_delta_vs_yesterday`, `today_by_source`, `current_bucket_tokens`, `current_bucket_label`, `seven_day_history`. Wire view-model construction to existing usage rollups.
- `src/tui/app.rs` — minimal wiring for new view-model fields. Event loop and key handling unchanged.
- `src/storage/` — read-only query method additions if existing methods do not already cover today-by-source, current-bucket-delta, and 7-day token history. No migrations, no schema changes.

If `layout.rs` grows past about 25k after these changes, peel each `render_*_panel()` into its own file under `src/tui/panels/` as a follow-up. Not required for this PR.

## Error Handling

Helper failures are visible in two places: the source row in the today panel renders `—` instead of a number when its source is unhealthy, and the helpers row glyph reflects the underlying `SourceStatus`. Repeated provider failures keep the last known pet state and last known view-model values rather than blanking the panels.

The frame renders even when the body is degraded — a blocked helper does not break the layout, it just shows up in the helpers row and turns affected today rows into `—` placeholders.

Width below the compact threshold (80 cols) falls back cleanly to the un-framed vertical stack. Height below the wide-mode minimum also falls back to compact. The transition is the existing `area.width < COMPACT_WIDTH` branch in `render_watch_frame()`; only the constant changes.

## Testing Strategy

The existing TUI snapshot tests will all churn — every watch frame renders differently. Regenerate baselines once the new layout is verified visually and commit the new snapshots in the same PR.

New unit tests:

- `style.rs` — bar ramp index function: 0 / 1 / N=12 fill levels, single-cell fill uses the middle stop, empty bar produces 12 faint cells, fill index never overflows the 5-element ramp.
- `layout.rs` — section header line generator produces exact target width regardless of label length; pad characters are `─`.
- `layout.rs` — wide-mode column math: pet column + gap + data column + pads sum to 76 at the reference 78-column width.
- `view_model.rs` — `today_by_source` shares sum to ~100% within rounding; zero-yesterday produces `None` delta rather than divide-by-zero; missing days in `seven_day_history` are `None`, not zero.

Visual smoke tests update to assert that frame characters appear at the expected columns for representative widths and that the helpers row reflects each `SourceStatus` variant.

Manual verification before merging: run `glorp watch` against real local usage at 80, 100, and 60 column widths. Verify that frame characters connect cleanly top to bottom, that bars render the gradient on a truecolor terminal and degrade readably to 256-color, that helpers reflects actual ccusage and codex availability, and that the 80-column compact threshold flips cleanly without flicker.

This design does not test pet art quality. The 8×11 templates carry over unchanged; their replacement is the subject of a follow-up initiative.

## References

- Parent spec: `docs/superpowers/specs/2026-05-08-glorp-design.md`
- Repair spec: `docs/superpowers/specs/2026-05-09-glorp-core-mvp-repair-design.md`
- Original handoff: `docs/tokenpet/README.md`
- Verified mockup: `.superpowers/brainstorm/31764-1778396947/content/hybrid-v4.html`
