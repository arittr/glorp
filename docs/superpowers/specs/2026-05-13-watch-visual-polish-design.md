# Watch Visual Polish — Design

Date: 2026-05-13
Status: Approved after brainstorm with Drew on 2026-05-13.
Source: Dev-preview review on 2026-05-13.

## Problem

The watch view's data pipeline, component system, and pet art are all in shape, but the watch *looks* like a data table with a pet pasted into a corner. The pet panel is 40×~19 (760 cells) but the pet art occupies 13×10 (130 cells) — 17% of its own panel. The rest is empty space. The 180×50 tall-wide preview is mostly whitespace. The frame chrome renders the species twice. Two preview-fixture flavor lines (`Mochi inspected a fresh diff`, `Mochi found a warm token cache`) look like null-timestamped real events. Stage progression beyond S6 has no visual presence — every stage feels identical except for the art itself.

The architecture is ready for the fix: `PetScene::compute_layout` already exposes `habitat: Rect` and `exclusions: Vec<Rect>`, and `ambient_glyphs_for()` in `src/tui/panels/pet.rs` exists as a PR1 stub that returns empty. The 2026-05-10 watch visual redesign called out this stub as "PR2 fills it". This spec is PR2 plus a small layout rebalance and a stage-aware frame chrome.

## Goal

Make the pet visibly the hero of the watch view. The pet panel should read as a small inhabited world. The frame should subtly evolve as the pet grows up. The change is one cohesive polish pass — one PR — that does not add new mechanics, new data, or new commands.

## Non-goals

- No new game mechanics, no new view-model fields, no new commands.
- No changes to pet art templates, animation profiles, or species generation.
- No frontend animation framework upgrades. The existing animator tick is fine.
- No theme system, no user-configurable layout, no per-pet customization.
- No notification surface, no cross-day evolution overlay redesign.
- Compact mode (`<118` cols) keeps its current vertical stack — no outer frame, no habitat.

## Decisions

Six decisions locked during the brainstorm:

1. **Habitat is an ambient scene.** Per-species sky glyphs + a floor row, painted behind the pet art, scaled in density by stage.
2. **Pet column grows.** `WIDE_LEFT_COL: 40 → 52`. `MAX_FRAME_WIDTH: 110 → 124`. `COMPACT_THRESHOLD: 104 → 118`. Right column width unchanged.
3. **Stage is visible in the frame chrome.** The outer frame fill character varies by stage tier: S0–S1 dotted, S2–S3 default, S4–S5 heavy, S6 alternating sparkle. Density of habitat glyphs also scales with stage.
4. **Chrome row drops the duplicate species.** `glorp · {name} · {species} · {age} · {mood}`. The "the {species}" inline tag is removed.
5. **Feed shows only real events.** The two preview-fixture narrative lines are removed; the feed renders provider events only.
6. **Other panels' rules stay uniform.** Only the outer frame chrome reflects stage. Today / progress / vitals / bio / feed all keep plain `─` rules.

## Architecture

The polish pass touches five existing files. No new modules.

### `src/tui/component/watch_screen.rs`

Three constant changes:

```rust
pub const WIDE_LEFT_COL: u16 = 52;        // was 40
pub const MAX_FRAME_WIDTH: u16 = 124;     // was 110
pub const COMPACT_THRESHOLD: usize = 118; // was 104
```

`bounded_frame_rect` already caps to `terminal_area.width.min(MAX_FRAME_WIDTH)`. Terminals between 118 and 124 cols stay in wide mode but render a frame narrower than 124 — linear shrink, no jump. `WIDE_LEFT_COL` is one constant; the wide layout always uses it (no per-width branching).

### `src/tui/panels/pet.rs`

The `ambient_glyphs_for()` stub becomes the real painter. New signature:

```rust
pub fn ambient_glyphs_for(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    clock: &WatchClock,
) -> Vec<AmbientGlyph>;
```

The pet panel's render loop already iterates returned glyphs. The loop's filter call becomes `ambient_glyph_is_inside_area(&g, habitat) && !exclusions.iter().any(|r| rect_contains_glyph(r, &g))`. The existing `pet_art` exclusion gets a 1-cell respect ring (rect inflated by 1 on all sides) so habitat glyphs do not crowd the pet outline.

### `src/tui/layout.rs`

Two changes:

1. The frame title format string drops `"the {species}"`. New format: `glorp · {name} · {species} · {age} · {mood}`. The species token already follows `{name}` separately; the inline `the {species}` was redundant.
2. The frame top and bottom edges' fill character is selected per stage via a new helper `frame_fill_for_stage(stage: Stage) -> &'static str`. See "Stage frame chrome" below for the table.

### `src/dev_preview/watch.rs`

Remove these two lines from the preview fixture's `state.recent_events`:

```rust
"Mochi inspected a fresh diff".to_string(),
"Mochi found a warm token cache".to_string(),
```

The preview feed renders only the real provider events the fixture already inserts (claude-code 12.5k, codex 4.2k, claude-code 8.8k).

### `tests/snapshots/`

The committed snapshots that change in this PR:

- `dev_preview__watch_wide_normal_frame.snap` — frame width 110 → 124, habitat content, chrome row.
- `dev_preview__watch_tall_wide_frame.snap` — frame width and habitat content.
- `dev_preview__watch_compact_normal_frame.snap` — chrome row only (no habitat, no outer frame, no layout change).
- Component-layout JSON artifacts under the dev-preview output bundle (`frames/*.layout.json`).

`tests/snapshots/dev_preview__pet_species_stage.snap` does not change — it renders pet art only, no habitat.

## Habitat painter

### Per-species palettes

| Species | Sky glyphs | Floor pattern |
|---|---|---|
| Fuzz | `·` `,` `'` `*` | `·,.,. ·.,,. .,, ·  · . . , .,.·` |
| Blob | `°` `o` `.` `·` | `~.~,~.~ ~  ~.~,~ ~.~ ~,~ ~.~` |
| Ghost | `~` `'` `,` `*` | `' ' '   '   '   '   '` |
| Glitch | `▒` `▓` `░` `▪` | `▒░▒  ░▓ ░░ ▒░ ░ ▒░ ▒░ ░░ ▒` |
| Crystal | `✦` `✧` `·` `◆` | `·.·   ·  .   ·.  · ·  .` |
| Mech | `~` `°` `·` `~●` | `─·─ ─ ─.─ ─ ─·─` |

All glyphs are strictly single-column. No emoji-width characters. Specific glyph picks may shift slightly during implementation; the species feel does not.

### Stage drives density

| Stage | Sky-glyph count |
|---|---|
| S0–S1 | 4 |
| S2–S3 | 6 |
| S4–S5 | 8 |
| S6 | 10 |

The floor row is always present, anchored to the bottom of the `habitat` rect. S6's existing top and bottom sparkle frame in `src/pet/render.rs` is unchanged — habitat layers underneath it.

### Deterministic drift

Glyph positions are generated by a small seeded RNG (e.g., `rand::SeedableRng` from a `u64` hash). The seed is `hash(species, stage, minute_floor(clock))`. Within a minute, positions are stable; across minutes, they roll over. Live watch gets slow "weather" without frame-rate jitter. Preview lab's fixed clock lands on a known minute, so artifacts remain byte-stable across runs.

### Exclusions

The painter takes `exclusions: &[Rect]` and rejects any candidate position that falls within any exclusion. Callers pass:

- The pet art rect, inflated by 1 on each side (the respect ring).
- The speech bubble rect when speech is active.

`PetScene::compute_layout` already maintains `exclusions: Vec<Rect>`; the painter consumes it directly.

### Color

A new `StyleRole::HabitatAmbient` maps to a muted variant (~35% lightness) of the species' role color in `src/tui/style.rs`. The floor row uses a slightly stronger variant. Stage does not affect color.

Color capability:

- `Truecolor` and `EightBit` → full habitat with role colors.
- `Basic` → habitat with a single dim foreground color.
- `Monochrome` → painter returns empty. Dim-without-color is just noise.

## Stage frame chrome

The outer rounded frame's top and bottom edges' fill character varies by stage:

| Stage | Fill | Top edge example |
|---|---|---|
| S0–S1 | `┄` | `╭ glorp · Mochi · fuzz · 1d · content ┄┄┄┄┄┄┄┄┄┄┄┄┄╮` |
| S2–S3 | `─` | `╭ glorp · Mochi · fuzz · 12d · content ──────────────╮` |
| S4–S5 | `━` | `╭ glorp · Mochi · fuzz · 18d · content ━━━━━━━━━━━━━╮` |
| S6 | `✦✧` | `╭ glorp · Mochi · fuzz · 90d · content ✦✧✦✧✦✧✦✧✦✧✦✧✦╮` |

Both top and bottom edges use the same character. S6's alternation always starts with `✦` and pads to even length; if width is odd, the leading char repeats once.

The corner glyphs (`╭`, `╮`, `╰`, `╯`) do not change. The title and footer text spans do not change beyond decision #4 (chrome row format).

Compact mode (`<118` cols) has no outer frame, so this stage chrome does not apply to compact.

## Testing

### New unit tests in `src/tui/panels/pet.rs`

The existing `ambient_glyphs_for_returns_empty_in_pr1_stub` test is replaced by:

1. `ambient_glyphs_never_overlap_exclusions` — for each species × stage, returned glyphs are disjoint from `pet_art` and `speech` rects.
2. `ambient_glyphs_are_deterministic_per_minute` — same `(species, stage, minute_floor(clock))` produces identical output across calls.
3. `ambient_glyph_count_scales_with_stage` — S0/S1 = 4, S2/S3 = 6, S4/S5 = 8, S6 = 10.
4. `ambient_glyphs_empty_on_monochrome` — `ColorCapability::Basic` returns colored glyphs, `Monochrome` returns empty.
5. `ambient_glyphs_within_habitat_bounds` — every glyph's (x, y) lies inside `habitat`.

### Layout tests in `src/tui/component/watch_screen.rs`

- Update `COMPACT_THRESHOLD` boundary tests to 118 (117 → compact, 118 → wide).
- Existing pet-panel-allocates-fill tests stay; the rect numbers change but the assertions are structural.

### Chrome tests in `src/tui/layout.rs`

- New test: `frame_fill_for_stage` returns the expected character per stage tier (S0 → `┄`, S3 → `─`, S5 → `━`, S6 → `✦✧`).
- Update the existing title-format test for the dropped `"the {species}"` inline tag.

### Snapshot regeneration

The five committed snapshots listed under `tests/snapshots/` are regenerated as part of the PR. The visual diff lives in the PR description.

## Risks

1. **Habitat noise vs readability.** Glyphs near the pet outline can crowd it. Mitigated by the 1-cell respect ring around the pet-art exclusion. If review still finds it noisy, the sky-glyph counts can drop by 2 per tier without revisiting the painter shape.

2. **Cell width assumptions.** The painter assumes 1 cell per glyph slot. All palette entries are strictly single-column ASCII or single-width Unicode. No `✨`-class emoji. Adding multi-cell glyphs later requires painter changes; not in scope.

3. **Terminal width 110–123.** Today, every terminal ≥ 106 cols gets a 110-wide frame. After this change, 118–123 stays in wide mode but renders a frame less than 124 wide with the new 52-wide pet column. The right column shrinks below its current width on those terminals. Not a regression — wide mode was already gated on roughly this range — but worth a line in the PR description.

4. **Snapshot churn looks scary in diff.** Five snapshot files plus a layout-JSON artifact change in one PR. Mitigated by the brainstorm-decided "one sweeping PR" approach: the diff is large but cohesive, and reviewers can read the PR description's visual diff before reading the snapshots.

## Sequencing

One PR. The brainstorm explored a 2-PR split (layout + habitat) and a 3-PR split (layout, habitat, stage chrome). Drew chose one sweeping PR — the changes reinforce each other visually, and concentrating snapshot churn in one commit makes the visual diff easier to review than three rounds of partial churn.
