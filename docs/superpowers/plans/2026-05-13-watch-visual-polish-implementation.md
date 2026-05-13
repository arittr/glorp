# Watch Visual Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the watch visual polish pass from `docs/superpowers/specs/2026-05-13-watch-visual-polish-design.md` as one cohesive PR: pet column grows to 52, frame max grows to 124, ambient habitat scenery fills the pet panel, frame chrome reflects stage, chrome row drops the duplicate species, preview fixture loses two flavor lines.

**Architecture:** Touches five existing files — `src/tui/component/watch_screen.rs` (three layout constants), `src/tui/layout.rs` (chrome title + per-stage border fill), `src/tui/panels/pet.rs` (habitat painter, replacing the PR1 stub), `src/dev_preview/watch.rs` (drop fixture flavor), and committed `tests/snapshots/dev_preview__*.snap` (regenerated). No new modules. The existing `PetScene::compute_layout` already exposes `habitat: Rect` and `exclusions: Vec<Rect>`, which the painter consumes.

**Tech Stack:** Rust, ratatui (with `symbols::border` for custom border characters), insta (snapshot testing), `time::OffsetDateTime` (already plumbed through `WatchClock`).

**Spec deviation noted up front:** The spec shows S6's frame fill as alternating `✦✧`. Ratatui's `Block::border_set` takes one char per cell, so the implementation uses single-char `✦` for S6. The "S6 feels mythic" intent is preserved. Switching to alternating later requires custom top/bottom row rendering and is out of scope.

---

## Task 0: Create WIP branch

**Files:**
- None — branch operation only.

- [ ] **Step 1: Verify clean working tree and create branch**

```bash
git status
git checkout -b watch-visual-polish
```
Expected: `nothing to commit, working tree clean` then `Switched to a new branch 'watch-visual-polish'`.

---

## Task 1: Drop the duplicate species in the chrome row

**Files:**
- Modify: `src/tui/layout.rs:75-99` (the `frame_title` function)
- Test: `src/tui/layout.rs` (existing test module at the bottom of the file)

Today's title format renders the species twice at certain stages (`Mochi the fuzz · fuzz · 18d · content`). Spec decision #4: drop `the {species}` from the inline string; the standalone `· {species} ·` token stays.

- [ ] **Step 1: Find the existing title-format test**

Run: `grep -n "frame_title\|the fuzz\|the {}" src/tui/layout.rs`
Expected: one or more matches; in particular the inline `format!(" glorp · {display_name} the {} · ", vm.species)` at line 91.

If no test asserts the current title format, skip to Step 3 and add a new test in Step 4.

- [ ] **Step 2: Update or add the failing test**

Add to the test module at the bottom of `src/tui/layout.rs`:

```rust
#[test]
fn frame_title_does_not_repeat_species() {
    use crate::tui::view_model::WatchViewModel;
    let vm = WatchViewModel::fixture();
    let spans = super::frame_title(&vm);
    let rendered: String = spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>();

    // The species token appears once (as a standalone field), not as part of "the {species}".
    assert!(
        !rendered.contains("the fuzz"),
        "frame title should not contain the inline 'the {{species}}' phrasing; got: {rendered:?}"
    );
    assert!(rendered.contains(" fuzz "), "species token should still appear as a standalone field");
}
```

Run: `cargo test --lib frame_title_does_not_repeat_species -- --nocapture`
Expected: FAIL — "the fuzz" is in the rendered title.

- [ ] **Step 3: Drop "the {species}" from the format string**

In `src/tui/layout.rs:91`, change:

```rust
Span::styled(
    format!(" glorp · {display_name} the {} · ", vm.species),
    styles.label,
),
```

to:

```rust
Span::styled(
    format!(" glorp · {display_name} · {} · ", vm.species),
    styles.label,
),
```

- [ ] **Step 4: Re-run the new test**

Run: `cargo test --lib frame_title_does_not_repeat_species -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run the layout test module**

Run: `cargo test --lib --package glorp tui::layout`
Expected: all unit tests pass.

- [ ] **Step 6: Regenerate dev-preview snapshots affected by the chrome change**

Run: `INSTA_UPDATE=auto cargo test --test dev_preview`
Expected: `dev_preview__watch_wide_normal_frame.snap`, `dev_preview__watch_tall_wide_frame.snap`, and `dev_preview__watch_compact_normal_frame.snap` update with the new chrome row.

Verify the diff is chrome-only:

```bash
git diff tests/snapshots/dev_preview__watch_wide_normal_frame.snap
```
Expected: only the top border row changes; "the fuzz" disappears. No layout/panel changes.

- [ ] **Step 7: Run the full test suite to confirm green**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/tui/layout.rs tests/snapshots/dev_preview__watch_wide_normal_frame.snap tests/snapshots/dev_preview__watch_tall_wide_frame.snap tests/snapshots/dev_preview__watch_compact_normal_frame.snap
git commit -m "fix(tui): drop duplicate species from frame chrome

Title was 'glorp · Mochi the fuzz · fuzz · ...' — the inline
'the {species}' rendered the same word as the standalone species
token at S4 of the fuzz species. Drop the inline form."
```

---

## Task 2: Remove preview-fixture narrative lines

**Files:**
- Modify: `src/dev_preview/watch.rs:88-91`

The dev-preview fixture seeds two narrative flavor lines into `state.recent_events`. They render as `--:--  Mochi inspected a fresh diff` in the watch feed and look like null-timestamped real events. Drew's note: only real provider data should reach the user.

- [ ] **Step 1: Locate the lines**

Run: `grep -n "inspected a fresh diff\|warm token cache" src/dev_preview/watch.rs`
Expected: hits at lines 89 and 90.

- [ ] **Step 2: Remove the narrative entries**

In `src/dev_preview/watch.rs`, change:

```rust
state.recent_events = vec![
    "Mochi inspected a fresh diff".to_string(),
    "Mochi found a warm token cache".to_string(),
];
```

to:

```rust
state.recent_events = Vec::new();
```

The real provider events the fixture inserts via `seed_usage_store` populate the feed without these flavor lines.

- [ ] **Step 3: Regenerate dev-preview snapshots and inspect the diff**

Run: `INSTA_UPDATE=auto cargo test --test dev_preview`
Expected: `tests/snapshots/dev_preview__*.snap` files are updated.

Verify the diff:

```bash
git diff tests/snapshots/dev_preview__watch_wide_normal_frame.snap
```
Expected: the two `--:--  Mochi inspected...` / `--:--  Mochi found...` rows are gone from the feed panel. No other changes.

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/dev_preview/watch.rs tests/snapshots/dev_preview__watch_wide_normal_frame.snap tests/snapshots/dev_preview__watch_tall_wide_frame.snap tests/snapshots/dev_preview__watch_compact_normal_frame.snap
git commit -m "chore(dev-preview): drop narrative flavor from watch fixture

The two '--:-- Mochi inspected/found' lines were preview-only flavor.
Feed shows only real provider events now; fixture relies on the
seeded usage store."
```

---

## Task 3: Layout constant updates

**Files:**
- Modify: `src/tui/component/watch_screen.rs:18-42`
- Test: same file's test module

Three constants change: `WIDE_LEFT_COL 40 → 52`, `MAX_FRAME_WIDTH 110 → 124`, `COMPACT_THRESHOLD 104 → 118`.

- [ ] **Step 1: Add the COMPACT_THRESHOLD boundary test**

Add to the test module in `src/tui/component/watch_screen.rs`:

```rust
#[test]
fn compact_threshold_boundary() {
    use ratatui::layout::Rect;
    use crate::tui::component::LayoutMode;
    let vm = crate::tui::view_model::WatchViewModel::fixture();

    let just_below = layout_watch(Rect::new(0, 0, 117, 30), &vm);
    assert_eq!(just_below.mode, LayoutMode::Compact, "117 cols should be compact");

    let at_threshold = layout_watch(Rect::new(0, 0, 118, 30), &vm);
    assert_eq!(at_threshold.mode, LayoutMode::Wide, "118 cols should be wide");
}
```

Run: `cargo test --lib compact_threshold_boundary -- --nocapture`
Expected: FAIL — under current constants, 117 is already wide (current threshold is 104).

- [ ] **Step 2: Update the three constants**

In `src/tui/component/watch_screen.rs`, change:

```rust
pub const COMPACT_THRESHOLD: usize = 104;
pub const WIDE_LEFT_COL: u16 = 40;
pub const MAX_FRAME_WIDTH: u16 = 110;
```

to:

```rust
pub const COMPACT_THRESHOLD: usize = 118;
pub const WIDE_LEFT_COL: u16 = 52;
pub const MAX_FRAME_WIDTH: u16 = 124;
```

- [ ] **Step 3: Re-run the boundary test**

Run: `cargo test --lib compact_threshold_boundary -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Run the entire test suite**

Run: `cargo test`
Expected: all unit + integration tests pass; the dev-preview snapshot tests will fail because the rendered frames now use 124-wide layout.

- [ ] **Step 5: Regenerate dev-preview snapshots and inspect**

Run: `INSTA_UPDATE=auto cargo test --test dev_preview`
Then:

```bash
git diff tests/snapshots/dev_preview__watch_wide_normal_frame.snap
git diff tests/snapshots/dev_preview__watch_tall_wide_frame.snap
git diff tests/snapshots/dev_preview__watch_compact_normal_frame.snap
```
Expected:
- `watch_wide_normal_frame` — frame extends from 110 to 120 wide (terminal is 120, capped to `min(120, 124)`), pet column is 52 wide, right column unchanged.
- `watch_tall_wide_frame` — frame extends from 110 to 124 wide (terminal is 180, capped to 124), pet column 52, vertical layout unchanged.
- `watch_compact_normal_frame` — no change beyond the prior chrome edit (compact threshold 117 stays compact at 72 cols).

If anything else changes (e.g., panel ordering, vitals/feed swap), STOP and diagnose — it means a layout invariant broke.

- [ ] **Step 6: Run all tests again to confirm green**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/tui/component/watch_screen.rs tests/snapshots/dev_preview__watch_wide_normal_frame.snap tests/snapshots/dev_preview__watch_tall_wide_frame.snap
git commit -m "feat(tui): grow pet column and frame for wide layouts

WIDE_LEFT_COL 40→52, MAX_FRAME_WIDTH 110→124, COMPACT_THRESHOLD
104→118. Pet column gains 12 cells (~30% wider); frame cap grows
to fit. Compact threshold rises in lockstep so wide-mode minimum
matches the new column + gutter + right-min math."
```

---

## Task 4: Frame fill per stage

**Files:**
- Modify: `src/tui/layout.rs` (add helper near `frame_title`, change `Block::bordered()` site at line 42-47)

Outer frame's top and bottom edges' fill character varies by stage tier: S0–S1 `┄`, S2–S3 `─`, S4–S5 `━`, S6 `✦`.

- [ ] **Step 1: Write the failing test for the helper**

Add to the test module of `src/tui/layout.rs`:

```rust
#[test]
fn frame_fill_for_stage_returns_expected_char() {
    use crate::game::evolution::Stage;
    assert_eq!(super::frame_fill_for_stage(Stage::S0), "┄");
    assert_eq!(super::frame_fill_for_stage(Stage::S1), "┄");
    assert_eq!(super::frame_fill_for_stage(Stage::S2), "─");
    assert_eq!(super::frame_fill_for_stage(Stage::S3), "─");
    assert_eq!(super::frame_fill_for_stage(Stage::S4), "━");
    assert_eq!(super::frame_fill_for_stage(Stage::S5), "━");
    assert_eq!(super::frame_fill_for_stage(Stage::S6), "✦");
}
```

Run: `cargo test --lib frame_fill_for_stage_returns_expected_char -- --nocapture`
Expected: FAIL — `frame_fill_for_stage` is not defined.

- [ ] **Step 2: Implement the helper**

Add to `src/tui/layout.rs`, just above `frame_title`:

```rust
/// Returns the horizontal border fill character for the outer frame, picked
/// per stage tier. S0–S1 use a dotted line, S2–S3 the default rounded fill,
/// S4–S5 a heavy line, S6 a sparkle. See the watch-visual-polish design.
pub(crate) fn frame_fill_for_stage(stage: crate::game::evolution::Stage) -> &'static str {
    use crate::game::evolution::Stage;
    match stage {
        Stage::S0 | Stage::S1 => "┄",
        Stage::S2 | Stage::S3 => "─",
        Stage::S4 | Stage::S5 => "━",
        Stage::S6 => "✦",
    }
}
```

- [ ] **Step 3: Re-run the helper test**

Run: `cargo test --lib frame_fill_for_stage_returns_expected_char -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Wire the helper to the outer Block**

In `src/tui/layout.rs`, find the existing block construction (around line 42):

```rust
let outer = Block::bordered()
    .border_type(BorderType::Rounded)
    .title(Line::from(frame_title(vm)))
    .title_bottom(Line::from(frame_footer()))
    .border_style(Style::default().fg(p.accent.rgb))
    .style(styles.body);
```

Replace with a `border_set` that picks the horizontal fill per stage while keeping the rounded corners and standard verticals:

```rust
use ratatui::symbols::border;

let fill = frame_fill_for_stage(vm.pet_render.stage);
let mut border_set = border::ROUNDED;
border_set.horizontal_top = fill;
border_set.horizontal_bottom = fill;

let outer = Block::default()
    .borders(ratatui::widgets::Borders::ALL)
    .border_set(border_set)
    .title(Line::from(frame_title(vm)))
    .title_bottom(Line::from(frame_footer()))
    .border_style(Style::default().fg(p.accent.rgb))
    .style(styles.body);
```

Add the `use ratatui::symbols::border;` import near the top of the file, alongside the existing ratatui imports. (If `border` is already imported elsewhere in the file, skip.)

- [ ] **Step 5: Add an integration-level snapshot assertion for the new behavior**

The dev-preview fixture's pet is at Stage::S4 (per `src/dev_preview/watch.rs:77`). After this task, the regenerated wide-normal snapshot should contain `━` characters in the top and bottom border rows. Verify by running:

```bash
INSTA_UPDATE=auto cargo test --test dev_preview
grep "━" tests/snapshots/dev_preview__watch_wide_normal_frame.snap | head -3
```
Expected: the top/bottom border rows contain `━` between the rounded corners and the title text.

- [ ] **Step 6: Run the full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/tui/layout.rs tests/snapshots/dev_preview__watch_wide_normal_frame.snap tests/snapshots/dev_preview__watch_tall_wide_frame.snap
git commit -m "feat(tui): vary frame border fill by stage

Outer frame's horizontal fill character now picks per stage tier:
S0/S1 ┄ (dotted), S2/S3 ─ (default), S4/S5 ━ (heavy), S6 ✦ (sparkle).
The whole room subtly changes as the pet grows."
```

---

## Task 5: Habitat painter signature refactor (no behavior change)

**Files:**
- Modify: `src/tui/panels/pet.rs:52-90` (signature, call site, existing tests)

Change the painter's signature to take `stage` and a slice of exclusions, without changing what it returns yet. Painter still returns empty. This is a pure refactor that sets the interface for Task 6.

- [ ] **Step 1: Change the painter signature and the call site**

In `src/tui/panels/pet.rs`, change the painter from:

```rust
pub fn ambient_glyphs_for(
    _species: Species,
    _panel: Rect,
    _pet_inner_rect: Rect,
    _now: time::OffsetDateTime,
) -> Vec<AmbientGlyph> {
    Vec::new()
}
```

to:

```rust
pub fn ambient_glyphs_for(
    _species: Species,
    _stage: Stage,
    _habitat: Rect,
    _exclusions: &[Rect],
    _now: time::OffsetDateTime,
) -> Vec<AmbientGlyph> {
    Vec::new()
}
```

(Add `use crate::game::evolution::Stage;` at the top of the file if not already imported. The grep earlier in this session showed `use crate::game::evolution::Stage;` at line 1 of `view_model.rs` but check for `panels/pet.rs` specifically.)

In the call site at `src/tui/panels/pet.rs:79`, change:

```rust
let glyphs = ambient_glyphs_for(species, scene.habitat, scene.pet_art, now);
```

to:

```rust
let stage = vm.pet_render.stage;
let glyphs = ambient_glyphs_for(species, stage, scene.habitat, &scene.exclusions, now);
```

- [ ] **Step 2: Update the existing painter test for the new signature**

The test `ambient_glyphs_for_returns_empty_in_pr1_stub` (around line 541 per earlier grep) calls the old signature. Update its body to pass the new arguments:

```rust
#[test]
fn ambient_glyphs_for_returns_empty_until_implemented() {
    use crate::game::evolution::Stage;
    let panel_rect = Rect::new(0, 0, 26, 12);
    let pet_inner = Rect::new(7, 2, 11, 8);
    let now = time::OffsetDateTime::UNIX_EPOCH;
    let glyphs = ambient_glyphs_for(
        Species::Fuzz,
        Stage::S4,
        panel_rect,
        &[pet_inner],
        now,
    );
    assert!(glyphs.is_empty(), "painter still stubbed; will fill in Task 6");
}
```

- [ ] **Step 3: Run the painter tests**

Run: `cargo test --lib --package glorp tui::panels::pet`
Expected: all painter tests pass — the rename test still asserts emptiness, and the other tests in the file (e.g., the exclusion helper test) are unaffected.

- [ ] **Step 4: Run all tests to confirm no regressions**

Run: `cargo test`
Expected: all pass. Snapshots do not change because the painter still returns empty.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs
git commit -m "refactor(tui): widen ambient painter signature

Take stage and an exclusions slice instead of just the pet rect.
Painter still returns empty; behavior change in the next commit."
```

---

## Task 6: Habitat painter per-species palettes and seeded RNG

**Files:**
- Modify: `src/tui/panels/pet.rs` — fill in the painter, add palette data, add tests
- Modify: `Cargo.toml` if `rand` is not already a dependency (add `rand = "0.8"` and `rand_pcg = "0.3"` for a deterministic seedable PRNG)

This is the meaty task. It introduces actual habitat rendering.

- [ ] **Step 1: Check if `rand` is already a dependency**

Run: `grep -E '^rand|^rand_pcg' Cargo.toml`
Expected: zero or more matches. If no `rand_pcg`, add to `Cargo.toml` under `[dependencies]`:

```toml
rand = { version = "0.8", default-features = false, features = ["std_rng"] }
rand_pcg = "0.3"
```

Run: `cargo build`
Expected: dependencies resolve.

- [ ] **Step 2: Write the deterministic-output test**

Add to the test module of `src/tui/panels/pet.rs`:

```rust
#[test]
fn ambient_glyphs_are_deterministic_per_minute() {
    use crate::game::evolution::Stage;
    let habitat = Rect::new(0, 0, 52, 20);
    let pet_inner = Rect::new(20, 6, 13, 10);
    let exclusions = vec![pet_inner];

    let t0 = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let t_same_minute = t0 + time::Duration::seconds(15);
    let t_next_minute = t0 + time::Duration::minutes(1);

    let a = ambient_glyphs_for(Species::Fuzz, Stage::S4, habitat, &exclusions, t0);
    let b = ambient_glyphs_for(Species::Fuzz, Stage::S4, habitat, &exclusions, t_same_minute);
    let c = ambient_glyphs_for(Species::Fuzz, Stage::S4, habitat, &exclusions, t_next_minute);

    assert_eq!(a, b, "same minute should yield identical glyphs");
    assert_ne!(a, c, "next minute should yield different glyphs");
}
```

Run: `cargo test --lib ambient_glyphs_are_deterministic_per_minute -- --nocapture`
Expected: FAIL — both empty vectors are equal, so `assert_ne!` fails ("next minute should yield different glyphs").

- [ ] **Step 3: Write the exclusion-overlap test**

Add to the same test module:

```rust
#[test]
fn ambient_glyphs_never_overlap_exclusions() {
    use crate::game::evolution::Stage;
    let habitat = Rect::new(0, 0, 52, 20);
    let pet_inner = Rect::new(20, 6, 13, 10);
    let exclusions = vec![pet_inner];
    let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

    for species in [Species::Fuzz, Species::Blob, Species::Ghost, Species::Glitch, Species::Crystal, Species::Mech] {
        for stage in [Stage::S0, Stage::S2, Stage::S4, Stage::S6] {
            let glyphs = ambient_glyphs_for(species, stage, habitat, &exclusions, now);
            for g in &glyphs {
                let in_exclusion = g.col >= pet_inner.x
                    && g.col < pet_inner.x + pet_inner.width
                    && g.row >= pet_inner.y
                    && g.row < pet_inner.y + pet_inner.height;
                assert!(
                    !in_exclusion,
                    "species {species:?} stage {stage:?} glyph at ({},{}) is inside exclusion {pet_inner:?}",
                    g.col, g.row
                );
            }
        }
    }
}
```

Run: `cargo test --lib ambient_glyphs_never_overlap_exclusions -- --nocapture`
Expected: PASS only because painter returns empty (vacuously true). Will become a meaningful assertion once Step 5 implements glyph generation.

- [ ] **Step 4: Write the in-bounds test**

```rust
#[test]
fn ambient_glyphs_within_habitat_bounds() {
    use crate::game::evolution::Stage;
    let habitat = Rect::new(5, 10, 52, 20);
    let pet_inner = Rect::new(25, 16, 13, 10);
    let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let glyphs = ambient_glyphs_for(Species::Crystal, Stage::S5, habitat, &[pet_inner], now);
    for g in glyphs {
        assert!(g.col >= habitat.x && g.col < habitat.x + habitat.width, "col {} outside habitat", g.col);
        assert!(g.row >= habitat.y && g.row < habitat.y + habitat.height, "row {} outside habitat", g.row);
    }
}
```

Run: `cargo test --lib ambient_glyphs_within_habitat_bounds -- --nocapture`
Expected: PASS (vacuously).

- [ ] **Step 5: Implement palette tables and the painter body**

In `src/tui/panels/pet.rs`, replace the painter stub with the real implementation. Add near the painter, but **above** it for readability:

```rust
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;

/// Per-species sky-glyph palette.
fn sky_palette_for(species: Species) -> &'static [char] {
    match species {
        Species::Fuzz => &['·', ',', '\'', '*'],
        Species::Blob => &['°', 'o', '.', '·'],
        Species::Ghost => &['~', '\'', ',', '*'],
        Species::Glitch => &['▒', '▓', '░', '▪'],
        Species::Crystal => &['✦', '✧', '·', '◆'],
        Species::Mech => &['~', '°', '·', '●'],
    }
}

/// Per-species floor-glyph palette (each cell of the floor row is drawn from this).
fn floor_palette_for(species: Species) -> &'static [char] {
    match species {
        Species::Fuzz => &['·', ',', '.', ' ', ' '],
        Species::Blob => &['~', '.', ',', ' '],
        Species::Ghost => &['\'', ' ', ' ', ' '],
        Species::Glitch => &['▒', '░', '▓', ' '],
        Species::Crystal => &['·', '.', ' ', ' ', ' '],
        Species::Mech => &['─', '·', '.', ' '],
    }
}

/// Sky-glyph count by stage tier.
fn stage_base_count(stage: Stage) -> usize {
    use crate::game::evolution::Stage::*;
    match stage {
        S0 | S1 => 4,
        S2 | S3 => 6,
        S4 | S5 => 8,
        S6 => 10,
    }
}
```

Then replace the painter body:

```rust
pub fn ambient_glyphs_for(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
) -> Vec<AmbientGlyph> {
    if habitat.width == 0 || habitat.height == 0 {
        return Vec::new();
    }

    // Seed: (species, stage, minute-floor). Same minute → identical positions.
    let species_seed = species as u64;
    let stage_seed = stage as u64;
    let minute_floor = (now.unix_timestamp() / 60) as u64;
    let seed = species_seed
        .wrapping_mul(0x9E37_79B1_7F4A_7C15)
        .wrapping_add(stage_seed.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(minute_floor.wrapping_mul(0x94D0_49BB_1331_11EB));
    let mut rng = Pcg32::seed_from_u64(seed);

    let sky = sky_palette_for(species);
    let floor = floor_palette_for(species);

    // Sky color: muted version of the species' role color (use the palette's
    // pre-existing dim color; species role is wired up elsewhere).
    let p = crate::tui::style::tokenpet_palette();
    let sky_color = p.dim.rgb;
    let floor_color = p.dim.rgb;

    let mut glyphs = Vec::new();

    let count = stage_base_count(stage); // area scaling lands in Task 7.

    for _ in 0..count {
        // Reject-sample up to N times to find a free cell.
        for _attempt in 0..16 {
            let col = habitat.x + rng.gen_range(0..habitat.width);
            let row = habitat.y + rng.gen_range(0..habitat.height.saturating_sub(1)); // leave bottom row for floor
            let candidate = AmbientGlyph {
                row,
                col,
                glyph: *sky.choose(&mut rng).unwrap_or(&' '),
                color: sky_color,
            };
            if !overlaps_any(&candidate, exclusions) {
                glyphs.push(candidate);
                break;
            }
        }
    }

    // Floor row: anchored to the bottom of habitat.
    let floor_row = habitat.y + habitat.height.saturating_sub(1);
    for dx in 0..habitat.width {
        let col = habitat.x + dx;
        let candidate = AmbientGlyph {
            row: floor_row,
            col,
            glyph: *floor.choose(&mut rng).unwrap_or(&' '),
            color: floor_color,
        };
        if !overlaps_any(&candidate, exclusions) {
            glyphs.push(candidate);
        }
    }

    glyphs
}

fn overlaps_any(g: &AmbientGlyph, exclusions: &[Rect]) -> bool {
    exclusions.iter().any(|r| {
        g.col >= r.x
            && g.col < r.x.saturating_add(r.width)
            && g.row >= r.y
            && g.row < r.y.saturating_add(r.height)
    })
}
```

Also note: in the spec we promised a 1-cell *respect ring* around `pet_art`. The painter receives `exclusions: &[Rect]` and treats them verbatim; inflating the ring is the caller's responsibility. Update the call site in `src/tui/panels/pet.rs:79` (from Task 5) to inflate the pet exclusion by 1 cell before passing:

```rust
let stage = vm.pet_render.stage;
let inflated_pet = inflate_rect(scene.pet_art, 1);
let inflated_exclusions: Vec<Rect> = scene.exclusions
    .iter()
    .map(|&r| if r == scene.pet_art { inflated_pet } else { r })
    .collect();
let glyphs = ambient_glyphs_for(species, stage, scene.habitat, &inflated_exclusions, now);
```

And add the helper above the call site:

```rust
fn inflate_rect(r: Rect, by: u16) -> Rect {
    let x = r.x.saturating_sub(by);
    let y = r.y.saturating_sub(by);
    let width = r.width.saturating_add(2 * by);
    let height = r.height.saturating_add(2 * by);
    Rect::new(x, y, width, height)
}
```

Also drop the now-stale stub test name and replace with the painter-active test:

```rust
#[test]
fn ambient_glyphs_present_with_floor_row() {
    use crate::game::evolution::Stage;
    let habitat = Rect::new(0, 0, 52, 20);
    let pet_inner = Rect::new(20, 6, 13, 10);
    let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let glyphs = ambient_glyphs_for(Species::Fuzz, Stage::S4, habitat, &[pet_inner], now);
    // 8 sky glyphs (S4) + 52-cell floor minus the exclusion overlap (none, since pet is mid-panel).
    assert!(glyphs.len() >= 8 + 30, "expected ≥ stage_base + most of the floor row, got {}", glyphs.len());
}
```

Remove `ambient_glyphs_for_returns_empty_until_implemented` from Task 5 — it's superseded by this and the determinism/overlap tests.

- [ ] **Step 6: Run all painter tests**

Run: `cargo test --lib --package glorp tui::panels::pet`
Expected: all painter tests pass — determinism, overlap, in-bounds, presence.

- [ ] **Step 7: Run all tests; expect snapshot regen**

Run: `cargo test`
Expected: dev-preview snapshot tests fail (habitat now renders into the pet panel).

- [ ] **Step 8: Regenerate snapshots, inspect, and verify visually**

```bash
INSTA_UPDATE=auto cargo test --test dev_preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```
Expected by eye: the pet panel in `watch-wide-normal.txt` and `watch-tall-wide.txt` now shows ambient `·` `,` `'` `*` glyphs above the pet and a `·,.` floor row below; pet art is untouched. No glyphs sit inside the 1-cell ring around the pet outline.

If glyphs feel crowded, try raising the divisor in the area-scale formula (Task 7) — for now, just verify the stage_base count looks right (8 sky + floor row).

- [ ] **Step 9: Run the full suite green**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add src/tui/panels/pet.rs Cargo.toml Cargo.lock tests/snapshots/dev_preview__watch_wide_normal_frame.snap tests/snapshots/dev_preview__watch_tall_wide_frame.snap
git commit -m "feat(tui): fill in habitat painter per species and stage

Painter now generates sky glyphs and a floor row keyed to species, with
positions seeded by (species, stage, minute_floor) so output is stable
within a minute and drifts across minutes. Pet outline gets a 1-cell
respect ring at the call site. Area-scaled density lands in next commit."
```

---

## Task 7: Area-scaled density

**Files:**
- Modify: `src/tui/panels/pet.rs` (the `ambient_glyphs_for` body — the line `let count = stage_base_count(stage);` becomes the area-scaled formula)

Spec formula: `count = stage_base + max(0, (habitat_cells - 200) / 60)`.

- [ ] **Step 1: Write the area-scaling test**

Add to the test module of `src/tui/panels/pet.rs`:

```rust
#[test]
fn ambient_glyph_count_scales_with_habitat_area() {
    use crate::game::evolution::Stage;
    let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

    // Normal-wide pet panel: ~52 × 14 = 728 cells, well above the 200 threshold.
    let normal = Rect::new(0, 0, 52, 14);
    let normal_glyphs = ambient_glyphs_for(
        Species::Fuzz, Stage::S4, normal, &[], now,
    );

    // Tall-wide pet panel: ~52 × 35 = 1820 cells.
    let tall = Rect::new(0, 0, 52, 35);
    let tall_glyphs = ambient_glyphs_for(
        Species::Fuzz, Stage::S4, tall, &[], now,
    );

    let normal_sky_count = normal_glyphs.iter().filter(|g| g.row < normal.height - 1).count();
    let tall_sky_count = tall_glyphs.iter().filter(|g| g.row < tall.height - 1).count();

    // Normal: 8 (S4 base) + (728 - 200) / 60 = 8 + 8 = 16.
    // Tall: 8 + (1820 - 200) / 60 = 8 + 27 = 35.
    assert!(
        tall_sky_count > normal_sky_count + 10,
        "tall habitat should produce noticeably more sky glyphs; normal={normal_sky_count} tall={tall_sky_count}"
    );
}
```

Run: `cargo test --lib ambient_glyph_count_scales_with_habitat_area -- --nocapture`
Expected: FAIL — current count is just `stage_base_count(stage)`, so normal == tall == 8.

- [ ] **Step 2: Implement the area-scaling**

In `src/tui/panels/pet.rs`, change:

```rust
let count = stage_base_count(stage); // area scaling lands in Task 7.
```

to:

```rust
let habitat_cells = (habitat.width as usize) * (habitat.height as usize);
let area_term = habitat_cells.saturating_sub(200) / 60;
let count = stage_base_count(stage) + area_term;
```

- [ ] **Step 3: Re-run the test**

Run: `cargo test --lib ambient_glyph_count_scales_with_habitat_area -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Run all painter tests; verify exclusion test still holds**

Run: `cargo test --lib --package glorp tui::panels::pet`
Expected: all painter tests still pass — more glyphs, but exclusion logic is unchanged.

- [ ] **Step 5: Regenerate dev-preview snapshots and inspect**

Run: `INSTA_UPDATE=auto cargo test --test dev_preview`
Then verify by eye:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```
Expected: `watch-tall-wide` now has visibly denser habitat — many more sky glyphs filling the vertical slack. `watch-wide-normal` gets a modest density bump (~16 glyphs total instead of 8).

- [ ] **Step 6: Commit**

```bash
git add src/tui/panels/pet.rs tests/snapshots/dev_preview__watch_wide_normal_frame.snap tests/snapshots/dev_preview__watch_tall_wide_frame.snap
git commit -m "feat(tui): scale habitat density with panel area

Sky-glyph count = stage_base + max(0, (cells - 200) / 60). Tall-wide
gets ~27 extra glyphs on top of the S4 base of 8; normal-wide gets ~8
extra. Stops tall-wide reading as a small pet in a big empty box."
```

---

## Task 8: Color-capability fallback

**Files:**
- Modify: `src/tui/panels/pet.rs` (extend the painter to consult `RenderContext::color_capability`)

The spec calls for Monochrome → empty, Basic → single dim color, Truecolor/EightBit → full habitat.

Today the painter doesn't take a `RenderContext`. Pass one in.

- [ ] **Step 1: Extend the painter signature with `color_capability`**

In `src/tui/panels/pet.rs`, change:

```rust
pub fn ambient_glyphs_for(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
) -> Vec<AmbientGlyph>
```

to:

```rust
pub fn ambient_glyphs_for(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: crate::tui::style::ColorCapability,
) -> Vec<AmbientGlyph>
```

Update the call site:

```rust
let glyphs = ambient_glyphs_for(
    species,
    stage,
    scene.habitat,
    &inflated_exclusions,
    now,
    ctx.color_capability,
);
```

(Confirm the field name on `RenderContext` via `grep -n "color_capability\|ColorCapability" src/tui/render_context.rs`. If the field is named differently, use the actual name.)

Inside the painter, at the top:

```rust
if matches!(color_capability, crate::tui::style::ColorCapability::Monochrome) {
    return Vec::new();
}
```

- [ ] **Step 2: Write the Monochrome test**

Add to the test module:

```rust
#[test]
fn ambient_glyphs_empty_on_monochrome() {
    use crate::game::evolution::Stage;
    use crate::tui::style::ColorCapability;
    let habitat = Rect::new(0, 0, 52, 20);
    let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let glyphs = ambient_glyphs_for(
        Species::Fuzz, Stage::S4, habitat, &[], now, ColorCapability::Monochrome,
    );
    assert!(glyphs.is_empty(), "Monochrome should disable habitat (dim-without-color is just noise)");
}
```

Update earlier painter tests (determinism, overlap, in-bounds, presence, area-scaling) to pass `ColorCapability::Truecolor` as the final argument.

- [ ] **Step 3: Run all painter tests**

Run: `cargo test --lib --package glorp tui::panels::pet`
Expected: all pass — Monochrome returns empty, others return the same density they did in Task 7.

- [ ] **Step 4: Run the full suite**

Run: `cargo test`
Expected: pass. Dev-preview uses Truecolor in its render context (per `tests/dev_preview.rs`), so snapshots do not change.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs
git commit -m "feat(tui): disable habitat on monochrome terminals

Dim-glyph-without-color is just visual noise. Painter returns empty
when ColorCapability is Monochrome; Basic and above keep habitat."
```

---

## Task 9: Visual review and final sweep

**Files:**
- No source changes. Visual verification + commit hygiene.

- [ ] **Step 1: Full preview review**

Run:
```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```
Inspect each scenario:

- `watch-wide-normal` (120×32): chrome row reads `glorp · Mochi · fuzz · 18d · content` (no duplicate species). Frame border shows `━` (S4). Pet column is 52 wide with ambient `·,',*` glyphs above the pet and a `·,.` floor below. No glyphs touching the pet outline.
- `watch-tall-wide` (180×50): same as above but with visibly more sky glyphs (~35 total). Frame is 124 wide.
- `watch-compact-normal` (72×24): no frame, no habitat. Chrome-like top line is gone in compact (already true). Feed shows only real provider events, no narrative lines.
- `pet-species-stage`: unchanged (no habitat in this matrix view).

If anything looks off, STOP and diagnose. Likely causes: respect ring too tight (increase from 1 to 2 cells), too many glyphs (raise the area divisor 60 → 80), wrong floor pattern (tune the palette).

- [ ] **Step 2: Run the full test and lint sweep**

Run:
```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
Expected: all pass.

- [ ] **Step 3: Skim the git log**

```bash
git log --oneline main..HEAD
```
Expected: 8 commits, one per task (Task 0 is just the branch; Tasks 1–8 are the substantive commits). If any commits are missing or have stale "WIP" messages, fix them now.

- [ ] **Step 4: Done**

The branch is ready for `gh pr create`. The PR description should include before/after screenshots of the dev-preview HTML output and a callout that snapshots regenerated as part of the polish.

---

## Out of scope (deferred)

These came up during planning but are not in this PR:

- **S6 alternating `✦✧` border fill.** Requires custom top/bottom row rendering instead of `border_set`. Tracked as a follow-up; spec example shows `✦✧` but implementation lands single-char `✦`.
- **Preview lab scenario sizing.** Today's `watch-wide-normal` is 120 cols; the new max frame is 124. Reviewers won't see the full 124-wide layout in the default scenario. A follow-up task can bump the preview-wide-normal size to 128 or add a `watch-roomy-wide` scenario. Out of scope for this PR per Drew's "one sweeping" decision.
