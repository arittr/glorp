# Glorp Watch Visual Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the fake-terminal chrome and prototype-derived data panels in `glorp watch` with a single accent outer frame, gradient bars, a token-only today panel with per-source breakdown and sparkline, and a helpers status row.

**Architecture:** Visual-layer rewrite concentrated in `src/tui/`. The `WatchViewModel` is unchanged — every field the new layout reads already exists. One read-only storage method (`seven_day_token_history`) is added to replace a capped event-walk in the sparkline source. Bars and sparkline use a 5-stop gradient ramp on truecolor terminals and degrade to solid fill elsewhere. Compact mode (new — currently absent) triggers below 80 cols and stacks vertically with no frame.

**Tech Stack:** Rust 2021, ratatui 0.29, crossterm 0.28, rusqlite (bundled), `time` crate. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-05-10-glorp-watch-visual-redesign-design.md`

---

## File Map

Files modified:
- `src/tui/style.rs` — add `BarRamp`, `ColorCapability`, ramp constants and detection; remove `chrome_title`, `prompt_user`, `prompt_path`, `prompt_sep`, `filled_bar_good`, `filled_bar_accent`.
- `src/tui/layout.rs` — biggest change. Delete `render_chrome` and the fake prompt line. Add `body_row`, `render_frame_top`, `render_frame_bottom`, `render_today_panel`, `render_sparkline_row`, `render_feed_panel`, `render_helpers_panel`. Rewrite `render_wide`, `render_compact`, `render_pet_panel`. Refactor `section_line` to return `Vec<Span>`. Rewrite `bar_line` to take `BarRamp` + `ColorCapability`. Introduce `COMPACT_THRESHOLD = 80`. Add hardcoded `EXPECTED_SOURCES` constant.
- `src/tui/app.rs` — add `color_capability` field to `WatchAppConfig`; thread it into `render_watch_frame`.
- `src/storage/usage_store.rs` — add `seven_day_token_history(now_utc_date)`.
- `src/commands/watch.rs` — switch sparkline source from `recent_events(500)` walk to `seven_day_token_history`.
- `tests/tui_render.rs` — surgical rewrites for new layout; delete three obsolete positional ordering tests.
- `tests/style_tokens.rs` — delete chrome/prompt style assertions.
- `tests/watch_integration.rs` — verify against new layout structure.

Files unchanged: `src/tui/view_model.rs`, `src/pet/`, `src/game/`, `src/usage/`.

---

## Phase A — Foundation (additive, doesn't change rendering)

### Task 1: Add `ColorCapability` type and detection in `style.rs`

**Files:**
- Modify: `src/tui/style.rs`
- Test: `src/tui/style.rs` (inline `#[cfg(test)]` module)

- [ ] **Step 1: Add a failing test for `ColorCapability::detect_from`**

Append to `src/tui/style.rs`:

```rust
#[cfg(test)]
mod color_capability_tests {
    use super::*;

    fn lookup(map: &[(&str, &str)], key: &str) -> Option<String> {
        map.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_string())
    }

    #[test]
    fn truecolor_when_colorterm_truecolor() {
        let env = [("COLORTERM", "truecolor")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Truecolor);
    }

    #[test]
    fn truecolor_when_colorterm_24bit() {
        let env = [("COLORTERM", "24bit")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Truecolor);
    }

    #[test]
    fn flat_when_no_color_set() {
        let env = [("COLORTERM", "truecolor"), ("NO_COLOR", "1")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Flat);
    }

    #[test]
    fn flat_when_term_dumb() {
        let env = [("TERM", "dumb")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Flat);
    }

    #[test]
    fn flat_when_no_relevant_env() {
        let env: [(&str, &str); 0] = [];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Flat);
    }
}
```

- [ ] **Step 2: Run the test, expect failure**

Run: `cargo test --lib --no-run` then `cargo test color_capability_tests -- --nocapture`
Expected: FAIL with `cannot find type ColorCapability` etc.

- [ ] **Step 3: Add the type and detection function**

Insert near the top of `src/tui/style.rs`, before `tokenpet_palette`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    Truecolor,
    Flat,
}

impl ColorCapability {
    pub fn detect() -> Self {
        Self::detect_from(|k| std::env::var(k).ok())
    }

    pub fn detect_from<F>(read: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        if read("NO_COLOR").is_some() {
            return ColorCapability::Flat;
        }
        if matches!(read("TERM").as_deref(), Some("dumb")) {
            return ColorCapability::Flat;
        }
        match read("COLORTERM").as_deref() {
            Some("truecolor") | Some("24bit") => ColorCapability::Truecolor,
            _ => ColorCapability::Flat,
        }
    }
}
```

- [ ] **Step 4: Run tests, expect pass**

Run: `cargo test color_capability_tests`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/style.rs
git commit -m "feat(tui): add ColorCapability type with env-var detection"
```

---

### Task 2: Add `BarRamp` type and ramp constants

**Files:**
- Modify: `src/tui/style.rs`

- [ ] **Step 1: Write a failing test for the ramp index function**

Append to `src/tui/style.rs`:

```rust
#[cfg(test)]
mod bar_ramp_tests {
    use super::*;

    #[test]
    fn ramp_index_zero_fill_never_called() {
        // Sanity: callers should never call ramp_index for N=0.
        // We only test the function for N >= 1.
    }

    #[test]
    fn ramp_index_single_cell_returns_zero() {
        assert_eq!(ramp_index(0, 1), 0);
    }

    #[test]
    fn ramp_index_full_bar_spans_entire_ramp() {
        let total = 12;
        assert_eq!(ramp_index(0, total), 0);
        assert_eq!(ramp_index(total - 1, total), 4);
    }

    #[test]
    fn ramp_index_clamps_to_four() {
        // Pathological input: i > N-1 should never come from the renderer,
        // but `.min(4)` is the guard.
        assert!(ramp_index(20, 12) <= 4);
    }

    #[test]
    fn green_ramp_middle_stop_matches_palette_good() {
        let ramp = BAR_RAMP_GOOD;
        let palette_good = tokenpet_palette().good.rgb;
        assert_eq!(ramp.stops[2], palette_good);
    }

    #[test]
    fn amber_ramp_middle_stop_matches_palette_accent() {
        let ramp = BAR_RAMP_ACCENT;
        let palette_accent = tokenpet_palette().accent.rgb;
        assert_eq!(ramp.stops[2], palette_accent);
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test bar_ramp_tests`
Expected: FAIL with undefined `ramp_index`, `BarRamp`, `BAR_RAMP_GOOD`, `BAR_RAMP_ACCENT`.

- [ ] **Step 3: Add `BarRamp` and ramps to `style.rs`**

Append to `src/tui/style.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct BarRamp {
    pub stops: [Color; 5],
}

pub const BAR_RAMP_GOOD: BarRamp = BarRamp {
    stops: [
        Color::Rgb(0x3d, 0x69, 0x48),
        Color::Rgb(0x5a, 0x84, 0x62),
        Color::Rgb(0x82, 0xbc, 0x83),
        Color::Rgb(0xa8, 0xd6, 0x90),
        Color::Rgb(0xd2, 0xee, 0xa2),
    ],
};

pub const BAR_RAMP_ACCENT: BarRamp = BarRamp {
    stops: [
        Color::Rgb(0x6e, 0x45, 0x16),
        Color::Rgb(0xb8, 0x7a, 0x2c),
        Color::Rgb(0xf0, 0xa6, 0x46),
        Color::Rgb(0xff, 0xc6, 0x6e),
        Color::Rgb(0xff, 0xe0, 0xa8),
    ],
};

pub fn ramp_index(i: usize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 0;
    }
    let raw = ((i as f64) * 4.0 / ((n - 1) as f64)).round() as usize;
    raw.min(4)
}
```

- [ ] **Step 4: Run tests, expect pass**

Run: `cargo test bar_ramp_tests`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/style.rs
git commit -m "feat(tui): add BarRamp type, green/amber ramps, ramp_index helper"
```

---

### Task 3: Add `seven_day_token_history` storage method

**Files:**
- Modify: `src/storage/usage_store.rs`

- [ ] **Step 1: Write a failing test**

Append to the existing `#[cfg(test)] mod tests { ... }` block in `src/storage/usage_store.rs` (or the module that holds the existing storage tests — locate by `grep -n "today_effective_tokens" src/storage/usage_store.rs` and add near other date-bounded tests):

```rust
#[test]
fn seven_day_token_history_returns_seven_oldest_first() {
    let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
    let today = OffsetDateTime::now_utc();
    // Insert events on day 0 (six days ago) and day 6 (today).
    let day0 = today - time::Duration::days(6);
    let day6 = today;
    store
        .insert_event(&NormalizedUsageEvent::for_test_at(day0, 1000.0))
        .unwrap();
    store
        .insert_event(&NormalizedUsageEvent::for_test_at(day6, 5000.0))
        .unwrap();

    let history = store.seven_day_token_history(today.date()).unwrap();
    assert_eq!(history.len(), 7);
    assert_eq!(history[0], 1000.0);
    assert_eq!(history[6], 5000.0);
    // The middle days have no events.
    for v in &history[1..6] {
        assert_eq!(*v, 0.0);
    }
}

#[test]
fn seven_day_token_history_sums_multiple_events_per_day() {
    let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
    let today = OffsetDateTime::now_utc();
    store
        .insert_event(&NormalizedUsageEvent::for_test_at(today, 1500.0))
        .unwrap();
    store
        .insert_event(&NormalizedUsageEvent::for_test_at(today, 2500.0))
        .unwrap();

    let history = store.seven_day_token_history(today.date()).unwrap();
    assert_eq!(history[6], 4000.0);
}

#[test]
fn seven_day_token_history_zero_for_empty_store() {
    let store = UsageStore::open(":memory:".as_ref()).unwrap();
    let today = OffsetDateTime::now_utc();
    let history = store.seven_day_token_history(today.date()).unwrap();
    assert_eq!(history, vec![0.0; 7]);
}
```

If `for_test_at` doesn't exist or has a different signature, check `src/storage/usage_store.rs:64` for the test helper signature and adapt.

- [ ] **Step 2: Run, expect failure**

Run: `cargo test seven_day_token_history`
Expected: FAIL with `no method named seven_day_token_history`.

- [ ] **Step 3: Implement the method**

Add to the `impl UsageStore` block in `src/storage/usage_store.rs`, near `today_effective_tokens`:

```rust
pub fn seven_day_token_history(
    &self,
    now_utc_date: time::Date,
) -> crate::error::Result<Vec<f64>> {
    let mut out = vec![0.0_f64; 7];
    for (i, slot) in out.iter_mut().enumerate() {
        let offset_days = 6 - i as i64;
        let day = now_utc_date - time::Duration::days(offset_days);
        let day_str = day.to_string();
        let value: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(effective_tokens), 0.0)
             FROM usage_events
             WHERE period_date = ?1",
            params![day_str],
            |row| row.get(0),
        )?;
        *slot = value;
    }
    Ok(out)
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test seven_day_token_history`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/storage/usage_store.rs
git commit -m "feat(storage): add seven_day_token_history(now_utc_date)"
```

---

### Task 4: Wire `commands/watch.rs` sparkline source to the new method

**Files:**
- Modify: `src/commands/watch.rs:105` and the `recent_daily_effective_tokens` helper at line 291

- [ ] **Step 1: Read the current sparkline source code**

Run: `grep -n "recent_daily_effective_tokens\|recent_usage = usage_store" src/commands/watch.rs`

- [ ] **Step 2: Replace the call site to use the new method**

Locate where `recent_daily_effective_tokens(&recent_usage, now)` is computed (around line 105). Replace with:

```rust
recent_daily_effective_tokens: usage_store
    .seven_day_token_history(now.date())
    .unwrap_or_else(|_| vec![0.0; 7]),
```

Then delete the old `fn recent_daily_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> Vec<f64>` helper (around line 291).

- [ ] **Step 3: Run watch-integration tests; check no regressions**

Run: `cargo test --test watch_integration`
Expected: pass (the field shape is unchanged).

- [ ] **Step 4: Commit**

```bash
git add src/commands/watch.rs
git commit -m "feat(watch): switch sparkline to seven_day_token_history query"
```

---

## Phase B — Layout primitives (additive)

### Task 5: Refactor `section_line` to return `Vec<Span>`

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Locate `section_line`**

Run: `grep -n "fn section_line" src/tui/layout.rs`

- [ ] **Step 2: Refactor to return `Vec<Span<'a>>` instead of `Line<'a>`**

Replace the existing `section_line` function with:

```rust
fn section_line<'a>(label: &'a str, target_width: usize, styles: &'a SemanticStyles) -> Vec<Span<'a>> {
    let label_text = format!(" {label} ");
    let label_visible = label_text.chars().count();
    let dash_total = target_width.saturating_sub(label_visible + 1); // +1 for leading dash
    let leading = "─";
    let trailing_count = dash_total;
    let trailing: String = std::iter::repeat('─').take(trailing_count).collect();
    vec![
        Span::styled(leading, styles.section_header),
        Span::styled(label_text, styles.label),
        Span::styled(trailing, styles.section_header),
    ]
}
```

- [ ] **Step 3: Update call sites**

Run: `grep -n "section_line" src/tui/layout.rs` to find all callers. Each caller used to push the resulting `Line` into a `Vec<Line>` directly; now they need to wrap the spans into a `Line::from(...)` themselves. Wrap each call site's result, e.g.:

```rust
let spans = section_line("vitals", area.width as usize, styles);
lines.push(Line::from(spans));
```

- [ ] **Step 4: `cargo check` to confirm no breakage**

Run: `cargo check`
Expected: clean.

- [ ] **Step 5: Run TUI tests**

Run: `cargo test --test tui_render` — many will fail due to other layout differences not yet made; for now just verify nothing panics during compilation.
Run: `cargo build`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src/tui/layout.rs
git commit -m "refactor(tui): section_line returns Vec<Span> for body-row composition"
```

---

### Task 6: Add `body_row` builder helper

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Write a failing unit test**

Add at the bottom of `src/tui/layout.rs`:

```rust
#[cfg(test)]
mod body_row_tests {
    use super::*;

    #[test]
    fn body_row_pads_short_content_to_inner_width() {
        let styles = semantic_styles();
        let inner: Vec<Span> = vec![Span::raw("hi")];
        let line = body_row(inner, 10, &styles);
        // Visible width: ┃ + 10 + ┃ = 12.
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 12);
        // First and last spans must be ┃.
        assert_eq!(line.spans.first().unwrap().content.as_ref(), "┃");
        assert_eq!(line.spans.last().unwrap().content.as_ref(), "┃");
    }

    #[test]
    fn body_row_truncates_overflowing_content() {
        let styles = semantic_styles();
        let inner: Vec<Span> = vec![Span::raw("xxxxxxxxxxxxxxxx")]; // 16 chars
        let line = body_row(inner, 10, &styles);
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 12);
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test body_row_tests`
Expected: FAIL — `body_row` undefined.

- [ ] **Step 3: Implement `body_row`**

Add to `src/tui/layout.rs` (near other helpers):

```rust
fn body_row<'a>(inner: Vec<Span<'a>>, inner_width: usize, styles: &'a SemanticStyles) -> Line<'a> {
    let visible: usize = inner.iter().map(|s| s.content.chars().count()).sum();
    let frame_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(inner.len() + 3);
    spans.push(Span::styled("┃", frame_style));
    if visible <= inner_width {
        spans.extend(inner);
        let pad = inner_width - visible;
        if pad > 0 {
            spans.push(Span::styled(" ".repeat(pad), styles.body));
        }
    } else {
        // Truncate cell-by-cell to fit; last visible char may be cut on a
        // multi-char span boundary. Scan spans and accumulate up to inner_width.
        let mut remaining = inner_width;
        for span in inner {
            let span_len = span.content.chars().count();
            if span_len <= remaining {
                spans.push(span);
                remaining -= span_len;
            } else {
                let truncated: String = span.content.chars().take(remaining).collect();
                spans.push(Span::styled(truncated, span.style));
                remaining = 0;
                break;
            }
        }
    }
    spans.push(Span::styled("┃", frame_style));
    Line::from(spans)
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test body_row_tests`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): add body_row builder for framed lines"
```

---

### Task 7: Rewrite `bar_line` to take `BarRamp` + `ColorCapability`

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

Append to `src/tui/layout.rs` (in the test module from previous tasks):

```rust
#[cfg(test)]
mod bar_line_tests {
    use super::*;
    use crate::tui::style::{ramp_index, BAR_RAMP_GOOD, ColorCapability};

    #[test]
    fn bar_line_zero_fill_renders_twelve_faint() {
        let styles = semantic_styles();
        let spans = bar_line_spans("fed", 0.0, BAR_RAMP_GOOD, ColorCapability::Truecolor, &styles);
        // 6-char label + 2 sp + 12 cells + 2 sp + value -> count fill cells.
        let bar_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let fill_count = bar_text.chars().filter(|c| *c == '░').count();
        assert_eq!(fill_count, 12);
        let solid_count = bar_text.chars().filter(|c| *c == '█').count();
        assert_eq!(solid_count, 0);
    }

    #[test]
    fn bar_line_full_fill_renders_twelve_solid() {
        let styles = semantic_styles();
        let spans = bar_line_spans("fed", 1.0, BAR_RAMP_GOOD, ColorCapability::Truecolor, &styles);
        let bar_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let solid_count = bar_text.chars().filter(|c| *c == '█').count();
        assert_eq!(solid_count, 12);
    }

    #[test]
    fn bar_line_flat_capability_uses_solid_color() {
        let styles = semantic_styles();
        let spans = bar_line_spans("fed", 0.5, BAR_RAMP_GOOD, ColorCapability::Flat, &styles);
        // All filled spans should share a single style (no gradient).
        let filled: Vec<_> = spans
            .iter()
            .filter(|s| s.content.contains('█'))
            .map(|s| s.style)
            .collect();
        let first = filled.first().copied().unwrap();
        for s in &filled {
            assert_eq!(*s, first);
        }
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test bar_line_tests`
Expected: FAIL — `bar_line_spans` undefined.

- [ ] **Step 3: Replace `bar_line` with `bar_line_spans`**

Locate the existing `bar_line` in `src/tui/layout.rs` (`grep -n "fn bar_line" src/tui/layout.rs`). Replace it (and remove the now-unused fill style constants in `style.rs`'s `SemanticStyles` if they're only used here — that cleanup is in Task 22) with:

```rust
fn bar_line_spans<'a>(
    label: &'a str,
    fill_fraction: f64,
    ramp: BarRamp,
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    const BAR_CELLS: usize = 12;
    let clamped = fill_fraction.clamp(0.0, 1.0);
    let n_filled = (clamped * BAR_CELLS as f64).round() as usize;
    let n_filled = n_filled.min(BAR_CELLS);
    let n_empty = BAR_CELLS - n_filled;
    let value_pct = (clamped * 100.0).round() as u32;

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(BAR_CELLS + 6);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{label:<6}"), styles.label));
    spans.push(Span::raw(" "));
    for i in 0..n_filled {
        let style = match capability {
            ColorCapability::Truecolor => {
                let idx = ramp_index(i, n_filled);
                Style::default().fg(ramp.stops[idx])
            }
            ColorCapability::Flat => {
                // Solid middle stop, no gradient.
                Style::default().fg(ramp.stops[2])
            }
        };
        spans.push(Span::styled("█", style));
    }
    if n_empty > 0 {
        spans.push(Span::styled("░".repeat(n_empty), styles.empty_bar));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{value_pct}"), styles.primary_text));
    spans
}
```

Make sure `BarRamp`, `BAR_RAMP_GOOD`, `BAR_RAMP_ACCENT`, `ramp_index`, `ColorCapability` are imported via `use crate::tui::style::{...}` at the top of `layout.rs`.

- [ ] **Step 4: Run, expect pass**

Run: `cargo test bar_line_tests`
Expected: 3 passed.

- [ ] **Step 5: Update existing call sites of `bar_line`**

Run: `grep -n "bar_line" src/tui/layout.rs` and replace each call with `bar_line_spans(label, fraction, ramp, capability, styles)` wrapped in `Line::from(...)`. Use `BAR_RAMP_GOOD` for `fed` and `energy`; `BAR_RAMP_ACCENT` for `happy` and `xp`. Capability for now: pass `ColorCapability::Truecolor` as a temporary literal — Task 18 wires the real value through.

- [ ] **Step 6: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): bar_line_spans takes BarRamp and ColorCapability"
```

---

## Phase C — Frame and panels (additive, not yet wired)

### Task 8: `render_frame_top` helper

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[cfg(test)]
mod frame_top_tests {
    use super::*;

    #[test]
    fn frame_top_pads_to_target_width() {
        let styles = semantic_styles();
        let line = render_frame_top_line(78, "mochi", "fuzz", "12d 4h", "content", &styles);
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 78);
    }

    #[test]
    fn frame_top_truncates_long_pet_name() {
        let styles = semantic_styles();
        let very_long = "thisnameiswaytoolongforthetitle";
        let line = render_frame_top_line(78, very_long, "fuzz", "12d 4h", "content", &styles);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("…"));
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 78);
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test frame_top_tests`
Expected: FAIL.

- [ ] **Step 3: Implement**

Add to `src/tui/layout.rs`:

```rust
const NAME_MAX: usize = 16;

fn render_frame_top_line<'a>(
    width: usize,
    pet_name: &'a str,
    species: &'a str,
    age: &'a str,
    mood: &'a str,
    styles: &'a SemanticStyles,
) -> Line<'a> {
    let frame_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let mood_style = Style::default().fg(tokenpet_palette().good.rgb);
    let display_name: String = if pet_name.chars().count() > NAME_MAX {
        let truncated: String = pet_name.chars().take(NAME_MAX - 1).collect();
        format!("{truncated}…")
    } else {
        pet_name.to_string()
    };
    // Title text: "glorp · <name> the <species> · <age> · <mood>"
    let title_text = format!("glorp · {display_name} the {species} · {age} · {mood}");
    let title_visible = title_text.chars().count();
    // Layout: ┏ ━ ' ' title ' ' (━ × N) ┓
    // 1 + 1 + 1 + title + 1 + N + 1 = width  ⇒ N = width - 5 - title
    let n_fill = width.saturating_sub(5 + title_visible);
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::styled("┏━ ", frame_style));
    // Style the visible title pieces. We render the title as one styled
    // chunk with mood highlighted in good color separately to keep the
    // visual interest. For simplicity, render plain and color the mood:
    let prefix_end = title_text.len() - mood.len();
    let prefix = &title_text[..prefix_end];
    spans.push(Span::styled(prefix.to_string(), styles.label));
    spans.push(Span::styled(mood.to_string(), mood_style));
    spans.push(Span::styled(" ".to_string(), styles.label));
    spans.push(Span::styled("━".repeat(n_fill), frame_style));
    spans.push(Span::styled("┓", frame_style));
    Line::from(spans)
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test frame_top_tests`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_frame_top_line with name truncation"
```

---

### Task 9: `render_frame_bottom` helper

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[cfg(test)]
mod frame_bottom_tests {
    use super::*;

    #[test]
    fn frame_bottom_pads_to_target_width() {
        let styles = semantic_styles();
        let line = render_frame_bottom_line(78, &styles);
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 78);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.starts_with("┗━"));
        assert!(text.ends_with("┛"));
        assert!(text.contains("q quit"));
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test frame_bottom_tests`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
fn render_frame_bottom_line<'a>(width: usize, styles: &'a SemanticStyles) -> Line<'a> {
    let frame_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let footer_text = "q quit · r refresh · ? help";
    let footer_visible = footer_text.chars().count();
    let n_fill = width.saturating_sub(5 + footer_visible);
    let spans = vec![
        Span::styled("┗━ ", frame_style),
        Span::styled(footer_text.to_string(), styles.label),
        Span::styled(" ".to_string(), styles.label),
        Span::styled("━".repeat(n_fill), frame_style),
        Span::styled("┛", frame_style),
    ];
    Line::from(spans)
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test frame_bottom_tests`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_frame_bottom_line"
```

---

### Task 10: `render_today_panel` returning `Vec<Line>`

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add the `EXPECTED_SOURCES` constant near the top of `layout.rs`**

```rust
/// Expected source surfaces and their display names.
/// Order is the render order for the today panel and helpers row.
const EXPECTED_SOURCES: &[(&str, &str)] = &[
    ("claude-code", "claude"),
    ("codex", "codex"),
];
```

- [ ] **Step 2: Add a failing test**

```rust
#[cfg(test)]
mod today_panel_tests {
    use super::*;
    use crate::tui::view_model::{SourceUsageView, WatchViewModel};

    #[test]
    fn today_panel_has_four_rows_plus_rule() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_today_panel_lines(43, &vm, &styles);
        // 1 rule + 4 data rows.
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn today_panel_renders_dash_for_absent_source() {
        let styles = semantic_styles();
        let mut vm = WatchViewModel::fixture();
        // Remove codex from breakdown.
        vm.source_breakdown.retain(|s| s.name != "codex");
        let lines = render_today_panel_lines(43, &vm, &styles);
        let text: String = lines[3]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains("codex"));
        assert!(text.contains("—"));
    }
}
```

- [ ] **Step 3: Run, expect failure**

Run: `cargo test today_panel_tests`
Expected: FAIL.

- [ ] **Step 4: Implement**

```rust
fn render_today_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out: Vec<Line<'a>> = Vec::new();
    let header = section_line("today", width, styles);
    out.push(Line::from(header));
    // tokens row
    out.push(today_row("tokens", &format_tokens_full(vm.today_effective_tokens), None, styles));
    // per-source rows
    let total = vm.today_effective_tokens.max(0.0);
    for (surface, display) in EXPECTED_SOURCES {
        let value_opt = vm
            .source_breakdown
            .iter()
            .find(|s| s.name == *surface)
            .map(|s| s.effective_tokens);
        let (value_str, share) = match value_opt {
            Some(v) => {
                let pct = if total > 0.0 { (v / total) * 100.0 } else { 0.0 };
                (format_tokens_full(v), Some(format!("{}%", pct.round() as u32)))
            }
            None => ("—".to_string(), Some("—".to_string())),
        };
        out.push(today_row(display, &value_str, share, styles));
    }
    // bucket row
    let bucket_str = format_signed_tokens_short(vm.current_bucket_effective_tokens);
    out.push(today_row("last 10m", &bucket_str, Some("this 10m".to_string()), styles));
    out
}

fn today_row<'a>(
    label: &'a str,
    value: &str,
    annotation: Option<String>,
    styles: &'a SemanticStyles,
) -> Line<'a> {
    let value_owned = value.to_string();
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{label:<8}"), styles.label));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(value_owned, styles.primary_text));
    if let Some(ann) = annotation {
        spans.push(Span::raw("       "));
        spans.push(Span::styled(ann, styles.label));
    }
    Line::from(spans)
}

fn format_tokens_full(n: f64) -> String {
    let n = n.round() as i64;
    if n.abs() >= 1_000 {
        let mut s = String::new();
        let neg = n < 0;
        let mut abs = n.unsigned_abs() as i64;
        let mut groups: Vec<String> = Vec::new();
        while abs >= 1000 {
            groups.push(format!("{:03}", abs % 1000));
            abs /= 1000;
        }
        groups.push(abs.to_string());
        groups.reverse();
        if neg {
            s.push('-');
        }
        s.push_str(&groups.join(","));
        s
    } else {
        n.to_string()
    }
}

fn format_signed_tokens_short(n: f64) -> String {
    let abs = n.abs();
    let unit = if abs >= 1_000_000.0 {
        format!("{:.1}m", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}k", abs / 1_000.0)
    } else {
        format!("{}", abs.round() as i64)
    };
    if n < 0.0 {
        format!("-{unit}")
    } else {
        format!("+{unit}")
    }
}
```

- [ ] **Step 5: Run, expect pass**

Run: `cargo test today_panel_tests`
Expected: 2 passed.

- [ ] **Step 6: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_today_panel_lines with EXPECTED_SOURCES list"
```

---

### Task 11: `render_sparkline_row`

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[cfg(test)]
mod sparkline_tests {
    use super::*;
    use crate::tui::style::ColorCapability;

    #[test]
    fn sparkline_row_returns_two_lines() {
        let styles = semantic_styles();
        let history = vec![100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0];
        let lines = render_sparkline_lines(43, &history, ColorCapability::Truecolor, &styles);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn sparkline_zeroes_render_dot() {
        let styles = semantic_styles();
        let history = vec![0.0; 7];
        let lines = render_sparkline_lines(43, &history, ColorCapability::Truecolor, &styles);
        let text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains('·'));
        assert!(!text.contains('█'));
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test sparkline_tests`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
fn render_sparkline_lines<'a>(
    width: usize,
    history: &[f64],
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    out.push(Line::from(section_line("7-day", width, styles)));

    let glyphs: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut last_seven: Vec<f64> = history.iter().copied().rev().take(7).collect();
    last_seven.reverse();
    while last_seven.len() < 7 {
        last_seven.insert(0, 0.0);
    }
    let max = last_seven.iter().cloned().fold(0.0_f64, f64::max);
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw("       "));
    for (i, value) in last_seven.iter().enumerate() {
        let glyph = if *value <= 0.0 {
            '·'
        } else if max <= 0.0 {
            '·'
        } else {
            let level = ((value / max) * (glyphs.len() as f64 - 1.0)).round() as usize;
            glyphs[level.min(glyphs.len() - 1)]
        };
        let style = match capability {
            ColorCapability::Truecolor => {
                let idx = ramp_index(i, 7);
                Style::default().fg(BAR_RAMP_GOOD.stops[idx])
            }
            ColorCapability::Flat => Style::default().fg(tokenpet_palette().good.rgb),
        };
        let style = if glyph == '·' {
            styles.empty_bar
        } else {
            style
        };
        spans.push(Span::styled(glyph.to_string(), style));
        if i < 6 {
            spans.push(Span::raw("   "));
        }
    }
    out.push(Line::from(spans));
    out
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test sparkline_tests`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_sparkline_lines with green ramp by age"
```

---

### Task 12: `render_feed_panel`

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[cfg(test)]
mod feed_panel_tests {
    use super::*;
    use crate::tui::view_model::WatchViewModel;

    #[test]
    fn feed_panel_returns_rule_plus_up_to_three_entries() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture_with_events();
        let lines = render_feed_panel_lines(43, &vm, &styles);
        // Rule + at most 3 entries.
        assert!(lines.len() >= 1 && lines.len() <= 4);
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test feed_panel_tests`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
fn render_feed_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    out.push(Line::from(section_line("feed", width, styles)));
    for event in vm.recent_events.iter().take(3) {
        let mut spans: Vec<Span<'a>> = Vec::new();
        spans.push(Span::raw("  "));
        spans.push(Span::styled(event.timestamp.clone(), styles.timestamp));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(event.text.clone(), styles.log(event.kind)));
        out.push(Line::from(spans));
    }
    out
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test feed_panel_tests`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_feed_panel_lines"
```

---

### Task 13: `render_helpers_panel`

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[cfg(test)]
mod helpers_panel_tests {
    use super::*;
    use crate::tui::view_model::{SourceHealthView, SourceStatus, WatchViewModel};

    #[test]
    fn helpers_panel_renders_check_when_ready() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_helpers_panel_lines(43, &vm, &styles);
        let text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains('✓'));
    }

    #[test]
    fn helpers_panel_renders_x_when_blocked() {
        let styles = semantic_styles();
        let mut vm = WatchViewModel::fixture();
        for src in vm.source_health.iter_mut() {
            src.status = SourceStatus::Blocked;
        }
        let lines = render_helpers_panel_lines(43, &vm, &styles);
        let text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains('✗'));
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test helpers_panel_tests`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
fn render_helpers_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    out.push(Line::from(section_line("helpers", width, styles)));
    let p = tokenpet_palette();
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw("  "));
    let mut first = true;
    for (surface, display) in EXPECTED_SOURCES {
        if !first {
            spans.push(Span::raw("     "));
        }
        first = false;
        let health = vm.source_health.iter().find(|s| s.name == *surface);
        let (glyph, glyph_style) = match health.map(|h| h.status) {
            Some(SourceStatus::Ready) => ('✓', Style::default().fg(p.good.rgb)),
            Some(SourceStatus::Diagnostic) => ('~', Style::default().fg(p.accent.rgb)),
            Some(SourceStatus::Blocked) => ('✗', Style::default().fg(p.bad.rgb)),
            None => ('—', Style::default().fg(p.dim.rgb)),
        };
        spans.push(Span::styled(display.to_string(), styles.label));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(glyph.to_string(), glyph_style));
    }
    out.push(Line::from(spans));
    out
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test helpers_panel_tests`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_helpers_panel_lines with SourceStatus glyphs"
```

---

## Phase D — Composition (replaces wide and compact rendering)

### Task 14: New `render_pet_panel_lines` (drops meta, returns lines)

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[cfg(test)]
mod pet_panel_tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;

    #[test]
    fn pet_panel_includes_vitals_rule_and_four_bars() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_pet_panel_lines(26, &vm, ColorCapability::Truecolor, &styles);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<String>();
        assert!(text.contains("vitals"));
        assert!(text.contains("fed"));
        assert!(text.contains("happy"));
        assert!(text.contains("energy"));
        assert!(text.contains("xp"));
    }

    #[test]
    fn pet_panel_does_not_include_meta_block() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_pet_panel_lines(26, &vm, ColorCapability::Truecolor, &styles);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<String>();
        // Old labels that should be gone:
        assert!(!text.contains("species"));
        assert!(!text.contains("stage"));
        assert!(!text.contains("mood"));
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test pet_panel_tests`
Expected: FAIL.

- [ ] **Step 3: Implement**

Replace the existing `render_pet_panel` function with:

```rust
fn render_pet_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out: Vec<Line<'a>> = Vec::new();
    // Centered art rows: 7 left-pad + 11 art + 8 right-pad = 26.
    let left_pad = (width.saturating_sub(11)) / 2;
    out.push(Line::from(Span::raw("")));
    for (line_index, art_line) in vm.pet_art.iter().enumerate() {
        let mut spans: Vec<Span<'a>> = Vec::new();
        spans.push(Span::raw(" ".repeat(left_pad)));
        spans.extend(role_spans_for_line(
            art_line,
            line_index,
            &vm.pet_spans,
            styles,
        ));
        out.push(Line::from(spans));
    }
    // Ground line: comma run of width-2 chars.
    let ground = ",".repeat(width.saturating_sub(2));
    out.push(Line::from(vec![
        Span::raw(" "),
        Span::styled(ground, styles.empty_bar),
    ]));
    out.push(Line::from(Span::raw("")));
    // Vitals rule
    out.push(Line::from(section_line("vitals", width, styles)));
    // Bars
    out.push(Line::from(bar_line_spans(
        "fed",
        vm.fed,
        BAR_RAMP_GOOD,
        capability,
        styles,
    )));
    out.push(Line::from(bar_line_spans(
        "happy",
        vm.happiness,
        BAR_RAMP_ACCENT,
        capability,
        styles,
    )));
    out.push(Line::from(bar_line_spans(
        "energy",
        vm.energy,
        BAR_RAMP_GOOD,
        capability,
        styles,
    )));
    let xp_fraction = if vm.xp_target <= 0.0 {
        0.0
    } else {
        (vm.xp_current / vm.xp_target).clamp(0.0, 1.0)
    };
    out.push(Line::from(bar_line_spans(
        "xp",
        xp_fraction,
        BAR_RAMP_ACCENT,
        capability,
        styles,
    )));
    out
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test pet_panel_tests`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_pet_panel_lines drops meta, uses gradient bars"
```

---

### Task 15: New `render_wide` that frames and lays out the inner grid

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing integration test against `WatchViewModel::fixture()`**

```rust
#[cfg(test)]
mod render_wide_tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_wide_draws_frame_at_80_cols() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| {
                render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row0: String = (0..80).map(|x| buffer[(x, 0)].symbol().to_string()).collect();
        assert!(row0.starts_with("┏━"));
        assert!(row0.ends_with("┓"));
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test render_wide_tests`
Expected: FAIL — `render_watch_frame_with_capability` not defined yet.

- [ ] **Step 3: Add the new wide renderer and the capability-aware entry point**

```rust
pub fn render_watch_frame_with_capability(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    capability: ColorCapability,
) {
    let area = frame.area();
    let p = tokenpet_palette();
    frame.render_widget(Block::default().style(Style::default().bg(p.bg.rgb)), area);
    if area.height == 0 || area.width == 0 {
        return;
    }
    let styles = semantic_styles();
    if (area.width as usize) < COMPACT_THRESHOLD {
        render_compact(frame, area, vm, capability, &styles);
    } else {
        render_wide(frame, area, vm, capability, &styles);
    }
}

const COMPACT_THRESHOLD: usize = 80;

fn render_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &WatchViewModel,
    capability: ColorCapability,
    styles: &SemanticStyles,
) {
    let width = area.width as usize;
    let inner_width = width.saturating_sub(2); // 78 - 2 = 76 etc.
    let pet_col = 26;
    let gap = 2;
    let pad_left = 2;
    let pad_right = 3;
    let data_col = inner_width.saturating_sub(pet_col + gap + pad_left + pad_right);

    let pet_lines = render_pet_panel_lines(pet_col, vm, capability, styles);
    let mut data_lines: Vec<Line> = Vec::new();
    data_lines.push(Line::from(Span::raw("")));
    data_lines.extend(render_today_panel_lines(data_col, vm, styles));
    data_lines.push(Line::from(Span::raw("")));
    data_lines.extend(render_sparkline_lines(data_col, &vm.recent_daily_effective_tokens, capability, styles));
    data_lines.push(Line::from(Span::raw("")));
    data_lines.extend(render_feed_panel_lines(data_col, vm, styles));
    data_lines.push(Line::from(Span::raw("")));
    data_lines.extend(render_helpers_panel_lines(data_col, vm, styles));

    // Build framed rows: top, body, bottom.
    let mut framed: Vec<Line> = Vec::new();
    framed.push(render_frame_top_line(
        width,
        &vm.pet_name,
        &vm.species,
        &format!("{}d", vm.age_days),
        &vm.mood,
        styles,
    ));
    let body_height = area.height.saturating_sub(2) as usize;
    let max_rows = pet_lines.len().max(data_lines.len()).max(body_height);
    for row_index in 0..max_rows {
        let pet_line = pet_lines.get(row_index);
        let data_line = data_lines.get(row_index);
        let mut inner: Vec<Span> = Vec::new();
        inner.push(Span::raw(" ".repeat(pad_left)));
        if let Some(line) = pet_line {
            let cell_count: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            inner.extend(line.spans.iter().cloned());
            if cell_count < pet_col {
                inner.push(Span::raw(" ".repeat(pet_col - cell_count)));
            }
        } else {
            inner.push(Span::raw(" ".repeat(pet_col)));
        }
        inner.push(Span::raw(" ".repeat(gap)));
        if let Some(line) = data_line {
            let cell_count: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            inner.extend(line.spans.iter().cloned());
            if cell_count < data_col {
                inner.push(Span::raw(" ".repeat(data_col - cell_count)));
            }
        } else {
            inner.push(Span::raw(" ".repeat(data_col)));
        }
        inner.push(Span::raw(" ".repeat(pad_right)));
        framed.push(body_row(inner, inner_width, styles));
    }
    framed.push(render_frame_bottom_line(width, styles));
    frame.render_widget(Paragraph::new(framed).style(styles.body), area);
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test render_wide_tests`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): render_wide assembles framed pet+data layout"
```

---

### Task 16: New `render_compact` (no frame, vertical stack, drop priority)

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Add a failing test**

```rust
#[cfg(test)]
mod render_compact_tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_compact_does_not_draw_frame() {
        let backend = TestBackend::new(60, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row0: String = (0..60).map(|x| buffer[(x, 0)].symbol().to_string()).collect();
        assert!(!row0.contains("┏"));
        assert!(!row0.contains("┓"));
    }

    #[test]
    fn render_compact_drops_helpers_under_height_pressure() {
        let backend = TestBackend::new(60, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut all = String::new();
        for y in 0..12 {
            for x in 0..60 {
                all.push_str(buffer[(x, y)].symbol());
            }
            all.push('\n');
        }
        // helpers should be dropped first.
        assert!(!all.contains("helpers"));
    }
}
```

- [ ] **Step 2: Run, expect failure**

Run: `cargo test render_compact_tests`
Expected: FAIL — current `render_compact` still uses old shape.

- [ ] **Step 3: Replace `render_compact`**

```rust
fn render_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &WatchViewModel,
    capability: ColorCapability,
    styles: &SemanticStyles,
) {
    let width = area.width as usize;
    let height = area.height as usize;

    // Build the section list in priority order.
    let pet = render_pet_panel_lines(width, vm, capability, styles);
    let today = render_today_panel_lines(width, vm, styles);
    let spark = render_sparkline_lines(width, &vm.recent_daily_effective_tokens, capability, styles);
    let feed = render_feed_panel_lines(width, vm, styles);
    let helpers = render_helpers_panel_lines(width, vm, styles);

    let footer = Line::from(vec![
        Span::styled("q", styles.prompt_user),
        Span::styled(" quit  ", styles.label),
        Span::styled("r", styles.prompt_path),
        Span::styled(" refresh  ", styles.label),
        Span::styled("?", styles.prompt_path),
        Span::styled(" help", styles.label),
    ]);

    // Pack sections in order, dropping from the end if we run out of space.
    let mut all: Vec<Line> = Vec::new();
    let groups: Vec<Vec<Line>> = vec![pet, today, spark, feed, helpers];
    for group in groups {
        if all.len() + group.len() + 1 > height.saturating_sub(1) {
            break;
        }
        all.extend(group);
        all.push(Line::from(Span::raw("")));
    }
    if all.len() < height {
        all.push(footer);
    } else if !all.is_empty() {
        // Replace the last line with footer if no room.
        let last_idx = all.len() - 1;
        all[last_idx] = footer;
    }

    if height < 10 {
        // Ultra-narrow: drop everything and render a one-line vitals summary.
        let summary = format!(
            "fed {} · happy {} · energy {} · xp {}",
            (vm.fed * 100.0).round() as u32,
            (vm.happiness * 100.0).round() as u32,
            (vm.energy * 100.0).round() as u32,
            if vm.xp_target > 0.0 {
                ((vm.xp_current / vm.xp_target).clamp(0.0, 1.0) * 100.0).round() as u32
            } else {
                0
            },
        );
        all = vec![Line::from(Span::styled(summary, styles.primary_text))];
    }
    if height == 0 {
        return;
    }
    frame.render_widget(Paragraph::new(all).style(styles.body), area);
}
```

- [ ] **Step 4: Run, expect pass**

Run: `cargo test render_compact_tests`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/layout.rs
git commit -m "feat(tui): compact mode drops frame, stacks vertically with drop priority"
```

---

## Phase E — Wiring and cleanup

### Task 17: Add `color_capability` to `WatchAppConfig`

**Files:**
- Modify: `src/tui/app.rs`

- [ ] **Step 1: Update the struct**

```rust
#[derive(Debug, Clone, Copy)]
pub struct WatchAppConfig {
    pub animation_tick: Duration,
    pub usage_poll_interval: Duration,
    pub color_capability: crate::tui::style::ColorCapability,
}

impl Default for WatchAppConfig {
    fn default() -> Self {
        Self {
            animation_tick: Duration::from_millis(250),
            usage_poll_interval: Duration::from_secs(10),
            color_capability: crate::tui::style::ColorCapability::detect(),
        }
    }
}
```

- [ ] **Step 2: Update the call to `render_watch_frame` in the app loop**

Find the existing `render_watch_frame(f, &self.vm)` call (`grep -n "render_watch_frame" src/tui/app.rs`). Replace with:

```rust
render_watch_frame_with_capability(f, &self.vm, self.config.color_capability);
```

Update the `use crate::tui::layout::{...}` import block at the top of `app.rs` to import `render_watch_frame_with_capability` instead of `render_watch_frame`.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean (or only warnings about unused `render_watch_frame`).

- [ ] **Step 4: Commit**

```bash
git add src/tui/app.rs src/tui/layout.rs
git commit -m "feat(tui): WatchAppConfig carries ColorCapability"
```

---

### Task 18: Delete the old `render_watch_frame`, `render_chrome`, and prompt-line render

**Files:**
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Confirm no callers remain for the old `render_watch_frame`**

Run: `grep -rn "render_watch_frame\b" src/ tests/`
Expected: zero matches outside `layout.rs` itself (the new entry is `render_watch_frame_with_capability`).

- [ ] **Step 2: Delete the old `pub fn render_watch_frame`, `render_chrome`, and the prompt-line block**

Locate `fn render_chrome` (around line 140 in pre-edit code). Delete it. Locate the old `pub fn render_watch_frame` and delete it (the new entry is `render_watch_frame_with_capability`).

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/tui/layout.rs
git commit -m "refactor(tui): remove fake terminal chrome and old render entry"
```

---

### Task 19: Drop unused style fields

**Files:**
- Modify: `src/tui/style.rs`
- Modify: `src/tui/layout.rs` (any references)

- [ ] **Step 1: Find references**

Run: `grep -rn "chrome_title\|prompt_user\|prompt_path\|prompt_sep\|filled_bar_good\|filled_bar_accent" src/ tests/`

- [ ] **Step 2: Remove fields from `SemanticStyles` struct definition**

Open `src/tui/style.rs`. In the `SemanticStyles` struct, delete the `chrome_title`, `prompt_user`, `prompt_path`, `prompt_sep`, `filled_bar_good`, and `filled_bar_accent` fields. In `semantic_styles()` builder, delete the corresponding initializers.

- [ ] **Step 3: Update remaining call sites**

Compact mode's footer in `render_compact` uses `styles.prompt_user` and `styles.prompt_path` — replace those with `styles.label` (the visual difference is small and the chrome-tier styles are gone). Run `cargo check`; fix any other reference errors that surface.

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/tui/style.rs src/tui/layout.rs
git commit -m "refactor(tui): drop chrome_title, prompt_*, and filled_bar_* style fields"
```

---

## Phase F — Test surgery

### Task 20: Delete obsolete positional ordering tests

**Files:**
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Locate the three tests**

Run: `grep -n "wide_layout_keeps_pet_and_stats_top_stacked\|wide_layout_leads_with_pet_before_vitals_metadata\|pet_art_and_vitals_metadata_have_no_blank_rows_between_them" tests/tui_render.rs`

- [ ] **Step 2: Delete each test function entirely**

Delete those three `#[test] fn ...` blocks. They encode `name`/`species`/`stage`/`mood` as separate label rows and a `stats` section that no longer exist.

- [ ] **Step 3: Build**

Run: `cargo test --test tui_render --no-run`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add tests/tui_render.rs
git commit -m "test(tui): delete obsolete positional ordering tests"
```

---

### Task 21: Update remaining `tests/tui_render.rs` assertions

**Files:**
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Replace literal-string expectations**

Use `grep -n` to find each, then:
- `"glorp --"` → `"glorp · "` (the new title separator).
- `"╎"` (old divider) → delete those assertions; the new layout has no `╎`.
- `"┄"` (old dotted rule) → delete or replace with `"─"`.
- `"sources"` → `"helpers"`.
- `"log"` → `"feed"`.
- `"stats"` → delete those assertions; section is gone.
- `"●"` (old chrome traffic dot) → delete those assertions.
- Separate `name`/`species`/`stage`/`mood` label-row checks → delete; those moved into the title.
- `"bucket"` → `"last 10m"`.

- [ ] **Step 2: Run the test file**

Run: `cargo test --test tui_render`
Expected: most pass; some may need width adjustment (if a test sets a small `TestBackend` like 60×16, it now lands in compact mode and the assertions on frame characters won't apply). For each failing test, decide whether it should test wide (TestBackend at 80×30 or larger) or compact (60×30) mode and adjust the test parameters and assertions accordingly.

- [ ] **Step 3: Commit**

```bash
git add tests/tui_render.rs
git commit -m "test(tui): rewrite assertions for new layout"
```

---

### Task 22: Update `tests/style_tokens.rs`

**Files:**
- Modify: `tests/style_tokens.rs`

- [ ] **Step 1: Open and review**

Run: `cat tests/style_tokens.rs`

- [ ] **Step 2: Delete assertions for removed fields**

Delete each `assert!` or accessor that references `chrome_title`, `prompt_user`, `prompt_path`, `prompt_sep`, `filled_bar_good`, `filled_bar_accent`.

- [ ] **Step 3: Add assertions for new fields**

Add:

```rust
#[test]
fn bar_ramp_good_middle_stop_matches_palette_good() {
    let styles = semantic_styles();
    let _ = styles; // ensure semantic_styles still constructs cleanly
    let palette = tokenpet_palette();
    assert_eq!(BAR_RAMP_GOOD.stops[2], palette.good.rgb);
}
```

Add the appropriate `use` lines (`crate::tui::style::{BAR_RAMP_GOOD, semantic_styles, tokenpet_palette}` etc.).

- [ ] **Step 4: Run**

Run: `cargo test --test style_tokens`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add tests/style_tokens.rs
git commit -m "test(style): replace chrome assertions with bar-ramp checks"
```

---

### Task 23: Update `tests/watch_integration.rs`

**Files:**
- Modify: `tests/watch_integration.rs`

- [ ] **Step 1: Run as-is to see failures**

Run: `cargo test --test watch_integration`
Expected: failures pinned to old structural elements.

- [ ] **Step 2: Read each failure and rewrite assertions**

Apply the same renaming as Task 21 (`sources` → `helpers`, `log` → `feed`, etc.). For tests that exercise blocked/diagnostic paths, switch the assertions from `helper_status` string content to checking that the helpers row glyph appears (`✗` or `~`) and that affected today rows render `—`.

- [ ] **Step 3: Run**

Run: `cargo test --test watch_integration`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add tests/watch_integration.rs
git commit -m "test(watch): align integration assertions with new layout"
```

---

### Task 24: Full test suite run + manual verification

**Files:**
- None (this is a verification step)

- [ ] **Step 1: Full test run**

Run: `cargo test`
Expected: all green.

- [ ] **Step 2: Build the binary**

Run: `cargo build --release`
Expected: clean.

- [ ] **Step 3: Manual smoke check at three widths**

Run: `target/release/glorp watch` (after `glorp init` if needed) in:
- A truecolor terminal (e.g., iTerm2, Alacritty, kitty) at 100×40 — verify gradient bars and sparkline render with visible color steps.
- A truecolor terminal at exactly 80×30 — verify frame renders cleanly, side `┃` columns are unbroken top to bottom.
- A 60×30 terminal — verify compact mode kicks in (no frame, vertical stack).
- A `NO_COLOR=1 target/release/glorp watch` run at 100×40 — verify bars are solid (no gradient).

Document any rendering issues. If frame `┃` columns are broken, re-check `body_row` padding. If the gradient isn't visible, verify `COLORTERM` is set in the terminal.

- [ ] **Step 4: Commit any small follow-up fixes**

If any visual issues require code adjustments, make and commit them now with descriptive messages.

- [ ] **Step 5: Final commit and push**

```bash
git log --oneline | head -25  # sanity-check the chain
```

---

## Self-Review (read-only summary)

**Spec coverage:**
- Frame top/bottom — Tasks 8, 9, 15
- Pet column 26 wide, 7/11/8 padding — Task 14
- Pet name truncation 16 chars + `…` — Task 8
- `─ vitals ─` and bars — Task 14
- Today panel with EXPECTED_SOURCES + per-source `—` for absent — Task 10
- Sparkline with green ramp by age — Task 11
- Feed three entries in constructor order — Task 12
- Helpers row with SourceStatus glyphs (`✓`/`~`/`✗`/`—`) — Task 13
- Wide mode 26+2+43 grid + framing — Task 15
- Compact mode no frame + drop priority + ultra-narrow vitals summary — Task 16
- ColorCapability detection + WatchAppConfig wiring — Tasks 1, 17
- BarRamp + ramp_index — Task 2
- Bar gradient on truecolor / solid on flat — Task 7
- COMPACT_THRESHOLD = 80 — Task 15
- seven_day_token_history (UTC date, period_date column) — Task 3
- Sparkline source switch from recent_events(500) — Task 4
- Style cleanup (chrome_title, prompt_*, filled_bar_*) — Task 19
- Test surgery — Tasks 20, 21, 22, 23
- Full verification — Task 24

**Placeholder scan:** No "TBD", "TODO", or "implement later" steps. Each step has either runnable code or a concrete command + expected outcome.

**Type consistency:** `bar_line_spans`, `render_pet_panel_lines`, `render_today_panel_lines`, `render_sparkline_lines`, `render_feed_panel_lines`, `render_helpers_panel_lines`, `render_frame_top_line`, `render_frame_bottom_line`, `body_row`, `section_line` (returns `Vec<Span>`), `render_watch_frame_with_capability`, `WatchAppConfig.color_capability`, `seven_day_token_history(now_utc_date)` — names used consistently across tasks.

**Open implementation choices that don't block:**
- Exact color hex for the green/amber middle stops is set to match `tokenpet_palette()` (verified in Task 2 tests).
- Compact-mode footer rendering uses simple plain text — fine for terminals at any width.
- `format_tokens_full` does not handle negative numbers below `-1000` precisely — used for cumulative non-negative `today_effective_tokens`, so safe in practice.
