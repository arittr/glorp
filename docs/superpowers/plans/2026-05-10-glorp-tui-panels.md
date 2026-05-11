# Glorp TUI Layout Overhaul (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development for implementation tasks.

**Goal:** Replace `src/tui/composer.rs` (323 LOC) and the bulk of `src/tui/layout.rs` (~800 of 1133 LOC) with native ratatui `Layout` + `Flex` primitives organized behind a small `Panel` trait. Each panel lives in its own file under `src/tui/panels/` with a single clear responsibility.

**Source spec:** `docs/superpowers/specs/2026-05-10-glorp-frontend-overhaul-design.md` (Layout overhaul section).

**Architecture:** A `Panel` trait with `min_height`, `preferred_constraint`, and `render`. Six concrete panels (`PetPanel`, `VitalsPanel`, `TodayPanel`, `SparkPanel`, `FeedPanel`, `HelpersPanel`) each rendering into their allocated `Rect`. A top-level dispatcher picks wide-vs-compact mode and assembles a list of panels for the layout solver. Section dividers become `Block::default().borders(Borders::TOP).title(...)` instead of hand-painted `─── label ───` strings.

**Tech Stack:** Rust 2021, ratatui 0.29, crossterm 0.28. No new dependencies.

**Phase 2 of 6.** Independent of Phase 3 (animator), Phase 4 (tachyonfx), Phase 5 (mouse), Phase 6 (polish). Each gets its own plan.

---

## File map

**New:**
- `src/tui/panels/mod.rs` — declares Panel trait and re-exports concrete panels.
- `src/tui/panels/pet.rs` — `PetPanel`.
- `src/tui/panels/vitals.rs` — `VitalsPanel`.
- `src/tui/panels/today.rs` — `TodayPanel`.
- `src/tui/panels/spark.rs` — `SparkPanel`.
- `src/tui/panels/feed.rs` — `FeedPanel`.
- `src/tui/panels/helpers.rs` — `HelpersPanel`.

**Modified:**
- `src/tui/layout.rs` — bulk replaced with a thin dispatcher that builds a `Vec<Box<dyn Panel>>` and lays out via native ratatui `Layout`. Overlay functions (`render_help_overlay`, `render_evolution_overlay`, `render_hatch_overlay`) preserved. Drops from ~1133 LOC to ~300 LOC.
- `src/tui/mod.rs` — adds `pub mod panels;`.
- `src/tui/style.rs` — small additions for any new style accessors. No behavioral changes.

**Deleted:**
- `src/tui/composer.rs` — entirely removed. All callers migrate to native ratatui or the new panels.

---

## Tasks

### Task 1: Panel trait + module skeleton

**Files:** Create `src/tui/panels/mod.rs`. Modify `src/tui/mod.rs`.

- [ ] **Step 1: Create `src/tui/panels/mod.rs`**

```rust
//! Panel trait and concrete panel implementations.
//!
//! Each panel owns its rendering, its preferred sizing, and its honest
//! minimum height. The dispatcher in `tui::layout` builds a panel list per
//! frame and lays them out via ratatui `Layout`.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::view_model::WatchViewModel;

pub mod pet;
pub mod vitals;
pub mod today;
pub mod spark;
pub mod feed;
pub mod helpers;

pub use pet::PetPanel;
pub use vitals::VitalsPanel;
pub use today::TodayPanel;
pub use spark::SparkPanel;
pub use feed::FeedPanel;
pub use helpers::HelpersPanel;

pub trait Panel {
    /// Minimum height (in rows) this panel needs at the given width.
    fn min_height(&self, width: u16) -> u16;

    /// Preferred constraint for the layout solver.
    fn preferred_constraint(&self) -> Constraint;

    /// Render into the allocated rect.
    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel);
}
```

- [ ] **Step 2: Stub the six panel files**

Create `src/tui/panels/pet.rs` (and `vitals.rs`, `today.rs`, `spark.rs`, `feed.rs`, `helpers.rs`) each with a placeholder implementation that compiles:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::panels::Panel;
use crate::tui::view_model::WatchViewModel;

pub struct PetPanel;

impl Panel for PetPanel {
    fn min_height(&self, _width: u16) -> u16 { 0 }
    fn preferred_constraint(&self) -> Constraint { Constraint::Min(0) }
    fn render(&self, _area: Rect, _buf: &mut Buffer, _vm: &WatchViewModel) {
        // TODO: implement in Task 2
    }
}
```

(Substitute `PetPanel` with the appropriate name for each file.)

- [ ] **Step 3: Register module**

Edit `src/tui/mod.rs`:

```rust
pub mod app;
pub mod composer;  // still exists; deleted in Task 8
pub mod layout;
pub mod panels;
pub mod style;
pub mod view_model;
```

- [ ] **Step 4: Build**

`cargo build --lib` — expect clean build, no behavioral change yet.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/ src/tui/mod.rs
git commit -m "feat(tui): Panel trait + panel module skeleton"
```

### Task 2: Implement `PetPanel`

**Files:** Modify `src/tui/panels/pet.rs`.

Read `src/tui/layout.rs` to find the current pet rendering code (look for references to `vm.pet_art`, `vm.pet_spans`, the per-cell role-to-color resolution via `palette_roles`, and the existing `species_animation_profile`/blink logic). Port that into `PetPanel::render`.

Pet rendering depends on `vm.pet_art: Vec<String>`, `vm.pet_spans: Vec<StyledSegment>`, and the resolved `PaletteRoles`. Use ratatui's `Paragraph` with `Line`/`Span` construction; each cell's color comes from `roles.role_for(seg.role)` resolved through the existing `tokenpet_palette` helper in `tui::style`.

Center the pet horizontally in the area; top-anchor vertically; honor the LEFT_COL = 40 width budget.

`min_height` should report the actual pet line count plus padding for the vitals bars. `preferred_constraint` = `Constraint::Length(pet_lines.len() as u16 + padding)`.

- [ ] **Step 1: Implement render**
- [ ] **Step 2: Build + smoke**
- [ ] **Step 3: Commit** with message `feat(tui): PetPanel`

### Task 3: Implement `VitalsPanel`

**Files:** Modify `src/tui/panels/vitals.rs`.

The current vitals are rendered alongside the pet in the left column. Extract that into `VitalsPanel::render`. Vitals are 4 gradient bars (fed, happy, energy, xp) — find the gradient bar rendering in current `layout.rs` and port it.

`min_height` ≈ 4 (one row per bar) + 1 (section divider) = 5.

- [ ] **Step 1-3:** same shape as Task 2. Commit: `feat(tui): VitalsPanel`

### Task 4: Implement `TodayPanel`

**Files:** Modify `src/tui/panels/today.rs`.

Today panel shows the 4 data rows: total tokens, per-source breakdown, current 10-min window, last-event. Find the existing rendering in `layout.rs` (search for `today` section). Port into `TodayPanel::render`.

`min_height` ≈ 4 rows + 1 divider = 5.

Commit: `feat(tui): TodayPanel`

### Task 5: Implement `SparkPanel`

**Files:** Modify `src/tui/panels/spark.rs`.

7-day sparkline using `vm.recent_daily_effective_tokens`. Port the sparkline rendering from `layout.rs`. ratatui has a built-in `Sparkline` widget — use it if it matches; otherwise port the existing braille-based one.

`min_height` ≈ 2 (label + sparkline row) + 1 divider = 3.

Commit: `feat(tui): SparkPanel`

### Task 6: Implement `FeedPanel` and `HelpersPanel`

**Files:** Modify `src/tui/panels/feed.rs`, `src/tui/panels/helpers.rs`.

FeedPanel shows up to N recent events from `vm.recent_events` (or similar). It's the panel with the most flexibility — `preferred_constraint` returns `Constraint::Min(3)` so it absorbs leftover height.

HelpersPanel shows source health status (`vm.source_health.status`). Single row + divider.

Both panels in one task to reduce subagent overhead since they're small. Commit: `feat(tui): FeedPanel + HelpersPanel`

### Task 7: New dispatcher in `layout.rs`

**Files:** Modify `src/tui/layout.rs`.

Replace the existing `render_watch_frame_with_capability` body with a new dispatcher:

```rust
pub fn render_watch_frame_with_capability(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    capability: ColorCapability,
) {
    let _ = capability; // capability is consumed inside panels via styles
    let outer = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(frame_title(vm))
        .title_bottom(frame_footer())
        .style(styles().outer_frame);
    let inner = outer.inner(frame.area());
    frame.render_widget(outer, frame.area());

    let mode = if inner.width >= COMPACT_THRESHOLD { Mode::Wide } else { Mode::Compact };
    let panels = build_panel_layout(mode, vm);
    layout_and_render(inner, &panels, frame.buffer_mut(), vm);
}
```

Define `Mode`, `build_panel_layout`, `layout_and_render`. The panel layout matches the existing wide/compact behavior:

- **Wide mode (width ≥ 104):** `Layout::horizontal([Length(40), Length(4), Min(0)])` — pet column on the left (PetPanel + VitalsPanel), gutter, main column with vertical Layout of [TodayPanel, SparkPanel, FeedPanel, HelpersPanel].
- **Compact mode (width < 104):** single vertical Layout stacking all panels with `Flex::Start` and `spacing(1)`.

Constants like `COMPACT_THRESHOLD = 104` survive. The hand-padded blank rows, body_height backfill, and char-precision span math all go away.

- [ ] **Step 1: Implement** — most of layout.rs body gets deleted; the new dispatcher is ~80-120 LOC.
- [ ] **Step 2: Build + run tests** — many existing tests will fail because they assert on the old composer output. Update or delete the ones that test internal layout details. Keep tests that assert on full-frame output via `TestBackend`.
- [ ] **Step 3: Commit** `feat(tui): native Layout dispatcher`

### Task 8: Delete `composer.rs`

**Files:** Delete `src/tui/composer.rs`. Modify `src/tui/mod.rs`.

- [ ] **Step 1:** `grep -rn "tui::composer\|use crate::tui::composer" src/ tests/` — confirm all callers migrated.
- [ ] **Step 2:** Edit `src/tui/mod.rs` — remove `pub mod composer;`.
- [ ] **Step 3:** `rm src/tui/composer.rs`.
- [ ] **Step 4:** `cargo build --all-targets` clean.
- [ ] **Step 5:** Commit `feat(tui): delete composer.rs`.

### Task 9: Final regression sweep

- [ ] `cargo test` — all pass.
- [ ] `cargo clippy --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --all -- --check` — clean.
- [ ] `cargo run --bin glorp -- watch` — manual smoke test (Drew verifies UI).
- [ ] Commit any cleanup, push.

---

## Out of scope

- Mood/blink/breathing animation — Phase 3 (PetAnimator).
- Tachyonfx transitions — Phase 4.
- Mouse-tracked eyes + `m` toggle key — Phase 5.
- `terminal-light` adaptive palette — Phase 6.
- Light-palette OkLCH anchors — Phase 6.

## Risks

1. **Existing snapshot tests assert on the old composer output.** Expect ~30% test churn. Plan: update assertions against new buffer output rather than character widths.
2. **The "fake terminal" chrome from `2026-05-10-glorp-watch-visual-redesign-design.md` was already removed.** Confirm current state matches the rounded outer-frame spec — the dispatcher in Task 7 should preserve it.
3. **Inter-panel gaps.** `Layout::spacing(1)` adds 1 row between panels. If the existing layout uses different gap heights per section, the result may look subtly different; check the gallery render after Task 7.
