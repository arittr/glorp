# Glorp Rate Momentum Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Glorp rate a first-class momentum surface across the watch TUI and macOS companion.

**Architecture:** Add a shared `RateMomentum` model to `WatchViewModel`, derive it from normalized canonical Tokenmaxxing windows in the watch view-model builder, render detailed glyph-bearing rows in the TUI `today` panel, and render a compact neutral-grey pulse-first stack in the companion HUD. Keep storage queries in the existing `UsageStore` path and avoid companion-only data reads.

**Tech Stack:** Rust 2021, rusqlite-backed `UsageStore`, ratatui, AppKit text drawing in `src/companion/app.rs`, Preview Lab dev-preview feature.

## Global Constraints

- Momentum is derived at view-model build time; no new persisted state.
- Canonical token windows must use `UsageStore::canonical_total_tokens_between`.
- Query bounds must be normalized to whole seconds before deriving all half-open windows.
- `WatchViewModel` owns `rate_momentum` as a top-level field, not under `ProgressView`.
- TUI renders pulse first as `/10m`, hour second as `/hr`, with direction glyphs.
- Companion renders pulse first and hour second, slash-aligned, no labels, no captions, no arrows.
- Companion keeps the large token total white and renders the two-line rate block in neutral grey.
- Progress panel no longer renders the inline rate segment.
- Use TDD: write or update failing tests before production changes for each task.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `docs/superpowers/specs/2026-07-06-glorp-rate-momentum-design.md` | Modify | Capture review fixes: top-level field, precision-normalized windows, TUI height, companion legibility caveat |
| `src/tui/view_model.rs` | Modify | Define `RateMomentum`, `RateWindow`, `RateDirection`; add `WatchViewModel.rate_momentum` and fixtures |
| `src/commands/watch.rs` | Modify | Derive normalized 10m/60m current and previous windows from canonical totals |
| `tests/watch_integration.rs` | Modify | Cover canonical filtering, boundary precision, and direction derivation |
| `src/tui/panels/today.rs` | Modify | Render detailed momentum rows and update intrinsic height |
| `src/tui/component/widgets.rs` | Modify | Remove progress-bar inline rate display |
| `src/tui/panels/progress.rs` | Modify | Stop passing `rate_per_hour` into `ProgressBar`; update tests |
| `tests/tui_render.rs` or `src/tui/panels/today.rs` tests | Modify | Cover `/10m`, `/hr`, glyphs, and progress no-duplicate behavior |
| `src/companion/app.rs` | Modify | Render neutral slash-aligned two-line companion rate stack |
| `src/round/hud.rs` | Modify | Add cfg-free color mapping for `RateDirection` to `RoundColor` |
| `tests/round_scene.rs` / `src/round/hud.rs` tests | Modify | Cover companion up/down/neutral colors and no-arrow text contract where possible |
| `src/dev_preview/watch.rs` and/or `src/dev_preview/round.rs` | Modify | Add deterministic momentum fixtures for review |

---

## Task 1: Shared RateMomentum Model And Window Derivation

**Files:**
- Modify: `src/tui/view_model.rs`
- Modify: `src/commands/watch.rs`
- Modify: `tests/watch_integration.rs`
- Modify: `docs/superpowers/specs/2026-07-06-glorp-rate-momentum-design.md`

**Interfaces:**
- Produces: `WatchViewModel.rate_momentum: RateMomentum`
- Produces: `RateMomentum { pulse: RateWindow, hour: RateWindow, companion_direction: RateDirection }`
- Produces: `RateDirection::{Up, Down, Neutral}`

- [ ] **Step 1: Write failing integration tests**

Append tests in `tests/watch_integration.rs`:

```rust
#[test]
fn rate_momentum_uses_canonical_windows_and_directions() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = datetime!(2026-06-19 18:00:00 UTC);

    for (at, tokens) in [
        (now - Duration::minutes(5), 12_000.0),
        (now - Duration::minutes(15), 2_000.0),
        (now - Duration::minutes(30), 20_000.0),
        (now - Duration::minutes(90), 80_000.0),
    ] {
        store
            .insert_event(&NormalizedUsageEvent {
                provider_surface: "codex".to_string(),
                observed_at: at,
                bucket_at: at,
                total_tokens: tokens,
                effective_tokens: 1.0,
                token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
                ..NormalizedUsageEvent::for_test_at(at, 1.0)
            })
            .unwrap();
    }
    store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "legacy".to_string(),
            observed_at: now - Duration::minutes(4),
            bucket_at: now - Duration::minutes(4),
            total_tokens: 999_999.0,
            effective_tokens: 999_999.0,
            token_contract: glorp::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
            ..NormalizedUsageEvent::for_test_at(now - Duration::minutes(4), 999_999.0)
        })
        .unwrap();

    let vm = build_watch_view_model_for_test_at(&mech_state(), &db_path, now).unwrap();

    assert_eq!(vm.rate_momentum.pulse.current_tokens, 12_000.0);
    assert_eq!(vm.rate_momentum.pulse.previous_tokens, 2_000.0);
    assert_eq!(vm.rate_momentum.pulse.direction, glorp::tui::view_model::RateDirection::Up);
    assert_eq!(vm.rate_momentum.hour.current_tokens, 34_000.0);
    assert_eq!(vm.rate_momentum.hour.previous_tokens, 80_000.0);
    assert_eq!(vm.rate_momentum.hour.direction, glorp::tui::view_model::RateDirection::Down);
    assert_eq!(vm.rate_momentum.companion_direction, glorp::tui::view_model::RateDirection::Up);
}

#[test]
fn rate_momentum_normalizes_fractional_now_before_window_queries() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("usage.sqlite");
    let mut store = UsageStore::open(&db_path).unwrap();
    let now = datetime!(2026-06-19 18:00:00.5 UTC);
    let event_at = datetime!(2026-06-19 17:59:59 UTC);

    store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "codex".to_string(),
            observed_at: event_at,
            bucket_at: event_at,
            total_tokens: 1_500.0,
            effective_tokens: 1.0,
            token_contract: glorp::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
            ..NormalizedUsageEvent::for_test_at(event_at, 1.0)
        })
        .unwrap();

    let vm = build_watch_view_model_for_test_at(&mech_state(), &db_path, now).unwrap();

    assert_eq!(vm.rate_momentum.pulse.current_tokens, 1_500.0);
    assert_eq!(vm.rate_momentum.pulse.direction, glorp::tui::view_model::RateDirection::Up);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test watch_integration rate_momentum_uses_canonical_windows_and_directions
cargo test --test watch_integration rate_momentum_normalizes_fractional_now_before_window_queries
```

Expected: compile failure because `rate_momentum` and `RateDirection` do not exist.

- [ ] **Step 3: Implement model and derivation**

In `src/tui/view_model.rs`, add public structs/enums:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDirection {
    Up,
    Down,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateWindow {
    pub current_tokens: f64,
    pub previous_tokens: f64,
    pub direction: RateDirection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateMomentum {
    pub pulse: RateWindow,
    pub hour: RateWindow,
    pub companion_direction: RateDirection,
}
```

Add `pub rate_momentum: RateMomentum` to `WatchViewModel` beside `current_bucket_effective_tokens`. Add fixture values:

```rust
rate_momentum: RateMomentum {
    pulse: RateWindow {
        current_tokens: 2_300.0,
        previous_tokens: 900.0,
        direction: RateDirection::Up,
    },
    hour: RateWindow {
        current_tokens: 109_000.0,
        previous_tokens: 140_000.0,
        direction: RateDirection::Down,
    },
    companion_direction: RateDirection::Up,
},
```

In `src/commands/watch.rs`, add helpers near `build_watch_view_model_at`:

```rust
fn normalized_window_end(now: OffsetDateTime) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(now.unix_timestamp()).unwrap_or(now)
}

fn rate_direction(current: f64, previous: f64) -> crate::tui::view_model::RateDirection {
    let threshold = 1_000.0_f64.max(previous.max(0.0) * 0.10);
    if current > previous + threshold {
        crate::tui::view_model::RateDirection::Up
    } else if current < previous - threshold {
        crate::tui::view_model::RateDirection::Down
    } else {
        crate::tui::view_model::RateDirection::Neutral
    }
}

fn build_rate_window(
    usage_store: &UsageStore,
    end: OffsetDateTime,
    width: Duration,
) -> crate::tui::view_model::RateWindow {
    let start = end - width;
    let previous_start = start - width;
    let current = usage_store
        .canonical_total_tokens_between(start, end)
        .unwrap_or(0.0);
    let previous = usage_store
        .canonical_total_tokens_between(previous_start, start)
        .unwrap_or(0.0);
    crate::tui::view_model::RateWindow {
        current_tokens: current,
        previous_tokens: previous,
        direction: rate_direction(current, previous),
    }
}

fn companion_direction(
    pulse: crate::tui::view_model::RateDirection,
    hour: crate::tui::view_model::RateDirection,
) -> crate::tui::view_model::RateDirection {
    match pulse {
        crate::tui::view_model::RateDirection::Up
        | crate::tui::view_model::RateDirection::Down => pulse,
        crate::tui::view_model::RateDirection::Neutral => hour,
    }
}
```

In `build_watch_view_model_at`, compute:

```rust
let rate_end = normalized_window_end(now);
let pulse_window = build_rate_window(&usage_store, rate_end, Duration::minutes(10));
let hour_window = build_rate_window(&usage_store, rate_end, Duration::hours(1));
let rate_momentum = RateMomentum {
    pulse: pulse_window,
    hour: hour_window,
    companion_direction: companion_direction(pulse_window.direction, hour_window.direction),
};
```

Set `current_bucket_effective_tokens: pulse_window.current_tokens`, `progress.rate_per_hour: hour_window.current_tokens`, and `rate_momentum`.

- [ ] **Step 4: Run tests and verify pass**

Run:

```bash
cargo test --test watch_integration rate_momentum_uses_canonical_windows_and_directions
cargo test --test watch_integration rate_momentum_normalizes_fractional_now_before_window_queries
cargo test --test watch_integration rate_per_hour_uses_only_canonical_tokenmaxxing_totals
```

Expected: all pass.

---

## Task 2: Watch TUI Momentum Rows And Progress De-duplication

**Files:**
- Modify: `src/tui/panels/today.rs`
- Modify: `src/tui/panels/progress.rs`
- Modify: `src/tui/component/widgets.rs`

**Interfaces:**
- Consumes: `WatchViewModel.rate_momentum`
- Produces: TUI `today` panel rows for `/10m` and `/hr`

- [ ] **Step 1: Write failing TUI tests**

In `src/tui/panels/today.rs` tests, add:

```rust
#[test]
fn today_panel_renders_rate_momentum_rows() {
    let vm = WatchViewModel::fixture();
    let s = render_to_string(70, 8, &vm);
    assert!(s.contains("rate"), "expected rate label");
    assert!(s.contains("/10m"), "expected pulse row");
    assert!(s.contains("/hr"), "expected hour row");
    assert!(s.contains("↑"), "expected up glyph");
    assert!(s.contains("↓"), "expected down glyph");
}
```

In `src/tui/panels/progress.rs`, update `progress_panel_idle_hides_rate_segment` or add:

```rust
#[test]
fn progress_panel_does_not_render_rate_segment() {
    let mut vm = WatchViewModel::fixture();
    vm.progress.rate_per_hour = 109_000.0;
    let s = render(&vm);
    assert!(!s.contains("/hr"));
    assert!(!s.contains("↑ 109.0k"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --lib tui::panels::today::tests::today_panel_renders_rate_momentum_rows
cargo test --lib tui::panels::progress::tests::progress_panel_does_not_render_rate_segment
```

Expected: today test fails because rows do not exist; progress test fails because current progress bar renders `/hr`.

- [ ] **Step 3: Implement TUI rows and height**

In `src/tui/panels/today.rs`:

- Import `RateDirection`.
- Increase preferred height by replacing the fixed content row counts with:

```rust
let source_rows = vm.source_breakdown.len().min(MAX_VISIBLE_SOURCE_ROWS);
let overflow_rows = usize::from(vm.source_breakdown.len() > MAX_VISIBLE_SOURCE_ROWS);
let content_rows = 1 + source_rows + overflow_rows + 2 + 1;
Constraint::Length((content_rows as u16).saturating_add(1))
```

- Replace the `last 10m` row with two momentum rows:

```rust
if area.height > next_row {
    rate_row(
        "rate",
        vm.rate_momentum.pulse.direction,
        vm.rate_momentum.pulse.current_tokens,
        "/10m",
    )
    .render(row_rect(area, next_row), buf, ctx);
    next_row += 1;
}
if area.height > next_row {
    rate_row(
        "",
        vm.rate_momentum.hour.direction,
        vm.rate_momentum.hour.current_tokens,
        "/hr",
    )
    .render(row_rect(area, next_row), buf, ctx);
    next_row += 1;
}
```

- Add helper functions:

```rust
fn rate_row(
    label: &'static str,
    direction: RateDirection,
    tokens: f64,
    suffix: &'static str,
) -> MetricRow<'static> {
    MetricRow::new(label, format!("{} {}/{}", direction_glyph(direction), crate::format::format_tokens(tokens), &suffix[1..]))
        .value_color(direction_color(direction))
}

fn direction_glyph(direction: RateDirection) -> &'static str {
    match direction {
        RateDirection::Up => "↑",
        RateDirection::Down => "↓",
        RateDirection::Neutral => "→",
    }
}

fn direction_color(direction: RateDirection) -> ratatui::style::Color {
    let p = tokenpet_palette();
    match direction {
        RateDirection::Up => p.good.rgb,
        RateDirection::Down => p.bad.rgb,
        RateDirection::Neutral => p.dim.rgb,
    }
}
```

In `src/tui/component/widgets.rs`, add `value_color: Option<Color>` to `MetricRow`, builder `value_color`, and use it for the value span:

```rust
let value_style = self
    .value_color
    .map(|color| Style::default().fg(color))
    .unwrap_or(styles.primary_text);
```

In `src/tui/panels/progress.rs`, remove `.rate_per_hour(vm.progress.rate_per_hour)` from `ProgressBar`.

- [ ] **Step 4: Run tests and verify pass**

Run:

```bash
cargo test --lib tui::panels::today::tests::today_panel_renders_rate_momentum_rows
cargo test --lib tui::panels::progress::tests::progress_panel_does_not_render_rate_segment
cargo test --lib tui::component::widgets::tests::progress_bar_does_not_render_rate_segment
```

Expected: all pass. Replace the existing `progress_bar_can_append_rate_segment`
widget test with `progress_bar_does_not_render_rate_segment`; that test should
construct `ProgressBar::new(0.25).gradient(GradientToken::Xp)` and assert the
rendered spans do not contain `/hr`.

---

## Task 3: Companion Color-Only Rate Stack

**Files:**
- Modify: `src/round/hud.rs`
- Modify: `src/companion/app.rs`

**Interfaces:**
- Consumes: `WatchViewModel.rate_momentum`
- Produces: companion rate stack text with pulse first and hour second

- [ ] **Step 1: Write failing color helper test**

In `src/round/hud.rs` tests:

```rust
#[test]
fn rate_direction_colors_are_distinct() {
    use crate::tui::view_model::RateDirection;

    assert_ne!(rate_direction_color(RateDirection::Up), rate_direction_color(RateDirection::Down));
    assert_ne!(
        rate_direction_color(RateDirection::Neutral),
        rate_direction_color(RateDirection::Up)
    );
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test --lib round::hud::tests::rate_direction_colors_are_distinct
```

Expected: compile failure because `rate_direction_color` does not exist.

- [ ] **Step 3: Implement helper and AppKit rendering**

In `src/round/hud.rs`:

```rust
pub fn rate_direction_color(direction: crate::tui::view_model::RateDirection) -> RoundColor {
    match direction {
        crate::tui::view_model::RateDirection::Up => RoundColor(0.45, 0.84, 0.51, 1.0),
        crate::tui::view_model::RateDirection::Down => RoundColor(0.95, 0.38, 0.36, 1.0),
        crate::tui::view_model::RateDirection::Neutral => RoundColor(0.62, 0.63, 0.77, 1.0),
    }
}
```

In `src/companion/app.rs::draw_hud`, replace the one-line `sub_text` block with:

```rust
let rate_color = companion_rate_stack_color(vm.rate_momentum.companion_direction);
let pulse_text = format!(
    "{}/10m",
    crate::format::format_tokens(vm.rate_momentum.pulse.current_tokens)
);
let hour_text = format!(
    "{}/hr",
    crate::format::format_tokens(vm.rate_momentum.hour.current_tokens)
);
let sub_size = font_size * 0.72;
let pulse = attributed_pet_glyph(&pulse_text, sub_size, &rate_color);
let hour = attributed_pet_glyph(&hour_text, sub_size, &rate_color);
let max_rate_width = pulse.size().width.max(hour.size().width);
let rate_x = gap.center_x - max_rate_width / 2.0;
let pulse_y = top - big_h * 0.86;
let hour_y = pulse_y - pulse.size().height * 0.82;
pulse.drawAtPoint(NSPoint::new(rate_x, pulse_y));
hour.drawAtPoint(NSPoint::new(rate_x, hour_y));
```

If the two-line stack exceeds `gap.max_width`, shrink `sub_size` in a loop exactly like the big number loop.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test --lib round::hud::tests::rate_direction_colors_are_distinct
cargo test --test round_scene
```

Expected: pass.

---

## Task 4: Preview Fixtures And Final Verification

**Files:**
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/round.rs`
- Modify: snapshots only if tests explicitly require updates

**Interfaces:**
- Produces deterministic preview coverage for momentum states.

- [ ] **Step 1: Add deterministic fixture values**

In `src/dev_preview/watch.rs` or the watch fixture builder used by scenario `watch`, set at least one fixture to:

```rust
vm.rate_momentum.pulse.current_tokens = 31_800_000.0;
vm.rate_momentum.pulse.previous_tokens = 4_200_000.0;
vm.rate_momentum.pulse.direction = RateDirection::Up;
vm.rate_momentum.hour.current_tokens = 190_800_000.0;
vm.rate_momentum.hour.previous_tokens = 240_000_000.0;
vm.rate_momentum.hour.direction = RateDirection::Down;
vm.rate_momentum.companion_direction = RateDirection::Up;
```

In `src/dev_preview/round.rs`, add up/down/neutral companion fixture variants or set existing round fixtures to cover the three `companion_direction` states.

- [ ] **Step 2: Run targeted tests**

Run:

```bash
cargo test --test watch_integration rate_momentum
cargo test --lib tui::panels::today::tests::today_panel_renders_rate_momentum_rows
cargo test --lib tui::panels::progress::tests
cargo test --lib round::hud::tests::rate_direction_colors_are_distinct
cargo test --test round_scene
```

Expected: pass.

- [ ] **Step 3: Run Preview Lab check**

Run:

```bash
cargo test --features dev-preview --test dev_preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Expected: both commands exit 0 and write `target/glorp-preview/index.html`.

- [ ] **Step 4: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: pass.

- [ ] **Step 5: Review diff**

Run:

```bash
git diff --stat
git diff -- docs/superpowers/specs/2026-07-06-glorp-rate-momentum-design.md docs/superpowers/plans/2026-07-06-glorp-rate-momentum-implementation.md src/tui/view_model.rs src/commands/watch.rs src/tui/panels/today.rs src/tui/panels/progress.rs src/tui/component/widgets.rs src/round/hud.rs src/companion/app.rs src/dev_preview/watch.rs src/dev_preview/round.rs tests/watch_integration.rs
```

Expected: diff only covers rate momentum design/implementation.
