# Glorp Watch Presentation And Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `glorp watch` feel like a live terminal companion whose visible totals, source health, pet animation, colors, compact layout, and evolution moment all reflect the repaired data truth.

**Architecture:** Keep the existing Ratatui app and renderer, but stop flattening renderer output into plain strings too early. The watch view model carries pet identity plus render roles, source-health rows, event-time totals, and transient evolution state. The app redraw loop advances animation frames independently from provider polling, and the layout renders pet-first with role-aware spans and source-specific diagnostics.

**Tech Stack:** Rust 2021, `ratatui`, `crossterm`, existing `pet::render`, existing TUI test backend, `time`.

---

## Source Material

- Spec: `docs/superpowers/specs/2026-05-09-glorp-core-mvp-repair-design.md`
- Data plan prerequisite: `docs/superpowers/plans/2026-05-09-glorp-data-truth-pipeline.md`
- Visual reference: `docs/tokenpet/glorp.html`
- Stories: `story-007-watch-mode-tui-shell.md`, `story-008-pet-renderer-and-animation.md`, `story-009-status-doctor-and-errors.md`
- Current code seams:
  - `build_watch_view_model` renders pet art once with `compact: false`.
  - `WatchViewModel` carries `pet_art: Vec<String>` but not `StyledSegment` roles.
  - `SourceUsageView` carries only name and total, so diagnostics are flattened into `helper_status` and `errors`.
  - `WatchApp::run_on_terminal` redraws on animation tick but never mutates animation frame or re-renders pet art between polls.
  - `?` only opens help; it does not toggle.

## File Structure

Modify:

- `src/tui/view_model.rs`: source health row shape, pet render payload, animation identity payload, transient evolution fields.
- `src/commands/watch.rs`: event-time totals from `bucket_at` and `observed_at`, source-health rows, render model builder that can render compact or wide frames.
- `src/tui/app.rs`: animation tick state, refresh debounce, `?` toggle behavior, test hooks for redraw without poll.
- `src/tui/layout.rs`: pet-first panel order, source-health rendering, compact rendering, role-aware pet spans, evolution overlay trigger.
- `src/tui/style.rs`: role styles for body, eye, mouth, accent, and pattern.
- `tests/watch_integration.rs`: event-time view model and mixed diagnostics tests.
- `tests/tui_render.rs`: animation redraw, role color, compact art, help toggle, refresh debounce, and evolution moment tests.

---

## Task 1: Event-Time Watch View Model And Source Health Rows

**Files:**
- Modify: `src/tui/view_model.rs`
- Modify: `src/commands/watch.rs`
- Test: `tests/watch_integration.rs`

- [ ] **Step 1: Write failing watch model tests**

Append to `tests/watch_integration.rs`:

```rust
use glorp::tui::view_model::SourceStatus;

#[test]
fn watch_totals_use_observed_and_bucket_time_not_source_period_midnight() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let period_start = datetime!(2026-05-09 00:00 UTC);
    let observed_at = OffsetDateTime::now_utc();
    let bucket_at = observed_at - Duration::minutes(observed_at.minute() as i64 % 10);

    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            period_start,
            observed_at,
            bucket_at,
            effective_tokens: 1_300.0,
            ..NormalizedUsageEvent::for_test_at(period_start, 1_300.0)
        })
        .unwrap();

    let vm = build_watch_view_model_for_test(&mech_state(), &usage_db).unwrap();
    assert!(vm.today_effective_tokens >= 1_300.0);
    assert!(vm.current_bucket_effective_tokens >= 1_300.0);
    assert!(vm
        .recent_events
        .iter()
        .any(|event| event.timestamp != "00:00" && event.text.contains("1.3k")));
}

#[test]
fn mixed_provider_health_keeps_ready_source_and_diagnostic_source_visible() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage_store = UsageStore::open(&usage_db).unwrap();
    let now = OffsetDateTime::now_utc();
    usage_store
        .insert_event(&NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            observed_at: now,
            bucket_at: now,
            effective_tokens: 4_200.0,
            ..NormalizedUsageEvent::for_test_at(now, 4_200.0)
        })
        .unwrap();
    usage_store
        .insert_diagnostic(&ProviderDiagnostic {
            provider_surface: "codex".into(),
            code: "missing_helper".into(),
            message: "ccusage-codex helper was not found".into(),
            recorded_at: now,
        })
        .unwrap();

    let vm = build_watch_view_model_for_test(&mech_state(), &usage_db).unwrap();
    assert!(vm
        .source_health
        .iter()
        .any(|source| source.name == "claude-code" && source.status == SourceStatus::Ready));
    assert!(vm.source_health.iter().any(|source| {
        source.name == "codex"
            && source.status == SourceStatus::Diagnostic
            && source.diagnostic_code.as_deref() == Some("missing_helper")
    }));
    assert!(!vm.is_blocked());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test watch_totals_use_observed_and_bucket_time_not_source_period_midnight mixed_provider_health_keeps_ready_source_and_diagnostic_source_visible
```

Expected: compile failure for `source_health` and `SourceStatus`, or assertion failure because watch still uses `period_start`.

- [ ] **Step 3: Add source-health view model types**

In `src/tui/view_model.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    Ready,
    Diagnostic,
    Blocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceHealthView {
    pub name: String,
    pub status: SourceStatus,
    pub today_effective_tokens: f64,
    pub bucket_effective_tokens: f64,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
}
```

Add `pub source_health: Vec<SourceHealthView>` to `WatchViewModel`. Keep `source_breakdown` temporarily for existing tests, and derive it from ready rows until the layout moves fully to `source_health`.

- [ ] **Step 4: Use `bucket_at` and `observed_at` in watch aggregation**

In `src/commands/watch.rs`, change:

```rust
fn today_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let today = now.date();
    events
        .iter()
        .filter(|event| event.bucket_at.date() == today)
        .map(|event| event.effective_tokens)
        .sum()
}

fn current_bucket_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let cutoff = now - Duration::minutes(10);
    events
        .iter()
        .filter(|event| event.bucket_at >= cutoff)
        .map(|event| event.effective_tokens)
        .sum()
}
```

Change recent event timestamps to `observed_at`, and daily sparkline/source totals to `bucket_at`.

- [ ] **Step 5: Build source-health rows from events and diagnostics**

Add a `source_health` helper:

```rust
fn source_health(
    events: &[NormalizedUsageEvent],
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
    now: OffsetDateTime,
) -> Vec<SourceHealthView> {
    let mut names = std::collections::BTreeSet::new();
    for event in events {
        names.insert(event.provider_surface.clone());
    }
    for diagnostic in diagnostics {
        names.insert(diagnostic.provider_surface.clone());
    }

    names.into_iter()
        .map(|name| {
            let today_effective_tokens = events
                .iter()
                .filter(|event| event.provider_surface == name && event.bucket_at.date() == now.date())
                .map(|event| event.effective_tokens)
                .sum();
            let bucket_effective_tokens = events
                .iter()
                .filter(|event| event.provider_surface == name && event.bucket_at >= now - Duration::minutes(10))
                .map(|event| event.effective_tokens)
                .sum();
            let diagnostic = diagnostics.iter().find(|diagnostic| diagnostic.provider_surface == name);
            let status = if today_effective_tokens > 0.0 || bucket_effective_tokens > 0.0 {
                SourceStatus::Ready
            } else if diagnostic.is_some() {
                SourceStatus::Diagnostic
            } else {
                SourceStatus::Blocked
            };
            SourceHealthView {
                name,
                status,
                today_effective_tokens,
                bucket_effective_tokens,
                diagnostic_code: diagnostic.map(|d| d.code.clone()),
                diagnostic_message: diagnostic.map(|d| d.message.clone()),
            }
        })
        .collect()
}
```

`is_blocked` should return true only when all `source_health` rows are `Blocked` or `Diagnostic` and at least one row exists.

- [ ] **Step 6: Run focused watch model tests**

Run:

```bash
cargo test watch_integration
```

Expected: all watch integration tests pass after updating older assertions from `errors` to `source_health` where appropriate.

- [ ] **Step 7: Commit**

```bash
git add src/tui/view_model.rs src/commands/watch.rs tests/watch_integration.rs
git commit -m "feat: surface event-time source health"
```

---

## Task 2: Live Animation Redraw Without Provider Polling

**Files:**
- Modify: `src/tui/view_model.rs`
- Modify: `src/commands/watch.rs`
- Modify: `src/tui/app.rs`
- Test: `tests/tui_render.rs`

- [ ] **Step 1: Write failing animation test**

Append to `tests/tui_render.rs`:

```rust
#[test]
fn animation_tick_rerenders_pet_art_without_polling_usage() {
    let mut app = WatchApp::with_config(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(999),
        },
    );

    let before = app.view_model_for_test().pet_art.clone();
    app.advance_animation_for_test();
    let after = app.view_model_for_test().pet_art.clone();

    assert_ne!(before, after);
    assert_eq!(app.poll_count_for_test(), 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test animation_tick_rerenders_pet_art_without_polling_usage
```

Expected: compile failure for missing test hooks, or assertion failure because pet art is static.

- [ ] **Step 3: Store render identity in the view model**

In `src/tui/view_model.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PetRenderModel {
    pub seed: String,
    pub generated_species: String,
    pub stage: String,
    pub mood: String,
}
```

Add `pub pet_render: PetRenderModel` to `WatchViewModel`.

In `build_watch_view_model`, populate it from `PetState`. Keep `pet_art` as the currently rendered line cache for layout tests.

- [ ] **Step 4: Add watch re-render helper**

In `src/commands/watch.rs`, expose:

```rust
pub fn rerender_pet_for_view_model(
    vm: &mut WatchViewModel,
    tick: u64,
    compact: bool,
) -> Result<()> {
    let species = parse_species(&vm.pet_render.generated_species)
        .unwrap_or_else(|| generate_pet(&vm.pet_render.seed).species);
    let stage = parse_stage(&vm.pet_render.stage);
    let mood = parse_mood(&vm.pet_render.mood);
    let generated = generate_pet(&vm.pet_render.seed).with_species_for_test(species);
    let rendered = render_pet(
        &generated,
        stage,
        mood,
        AnimationFrame {
            tick,
            compact,
            blink_suppression_ticks: 0,
        },
    );
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    Ok(())
}
```

Add `parse_mood` beside `mood_label`.

- [ ] **Step 5: Advance animation in the app loop**

In `WatchApp`, add `animation_frame: u64` and a method:

```rust
pub fn advance_animation_for_test(&mut self) {
    self.animation_frame = self.animation_frame.wrapping_add(1);
    let _ = crate::commands::watch::rerender_pet_for_view_model(
        &mut self.vm,
        self.animation_frame,
        self.last_compact_for_test,
    );
}

pub fn view_model_for_test(&self) -> &WatchViewModel {
    &self.vm
}
```

In `run_on_terminal`, call the same internal advance method before each draw or immediately after each animation tick. Track compact mode from the current terminal width so the render helper receives `compact: frame.area().width < 72`.

- [ ] **Step 6: Run focused animation test**

Run:

```bash
cargo test animation_tick_rerenders_pet_art_without_polling_usage
```

Expected: test passes and `poll_count_for_test()` remains zero.

- [ ] **Step 7: Commit**

```bash
git add src/tui/view_model.rs src/commands/watch.rs src/tui/app.rs tests/tui_render.rs
git commit -m "feat: animate watch pet independently"
```

---

## Task 3: Role-Aware Pet Color Rendering

**Files:**
- Modify: `src/tui/view_model.rs`
- Modify: `src/tui/style.rs`
- Modify: `src/tui/layout.rs`
- Modify: `src/commands/watch.rs`
- Test: `tests/tui_render.rs`

- [ ] **Step 1: Write failing role-color test**

Append to `tests/tui_render.rs`:

```rust
use glorp::pet::render::{PaletteRoleName, StyledSegment};
use glorp::tui::style::semantic_styles;

#[test]
fn pet_renderer_roles_reach_tui_cells() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_art = vec![" {eye}{body}{accent}".into()];
    vm.pet_spans = vec![
        StyledSegment { line: 0, start: 1, end: 2, role: PaletteRoleName::Eye },
        StyledSegment { line: 0, start: 2, end: 3, role: PaletteRoleName::Body },
        StyledSegment { line: 0, start: 3, end: 4, role: PaletteRoleName::Accent },
    ];

    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let buf = terminal.backend().buffer();
    let styles = semantic_styles();

    assert!(has_cell(buf, "{", styles.pet_eye.fg.unwrap()));
    assert!(has_cell(buf, "e", styles.pet_body.fg.unwrap()));
    assert!(has_cell(buf, "y", styles.pet_accent.fg.unwrap()));
}
```

Adjust the fixture string if the chosen role spans use simpler visible symbols. The assertion must check three distinct style colors from `SemanticStyles`.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test pet_renderer_roles_reach_tui_cells
```

Expected: compile failure for `pet_spans` or missing pet role styles.

- [ ] **Step 3: Carry styled segments in the view model**

In `WatchViewModel`, add:

```rust
pub pet_spans: Vec<crate::pet::render::StyledSegment>,
```

Update fixtures and `build_watch_view_model` to assign `rendered.spans`.

- [ ] **Step 4: Add pet role styles**

In `SemanticStyles`, add:

```rust
pub pet_body: Style,
pub pet_eye: Style,
pub pet_mouth: Style,
pub pet_accent: Style,
pub pet_pattern: Style,
```

In `semantic_styles`, map them to restrained Tokenpet palette approximations:

```rust
pet_body: Style::default().fg(p.fg.rgb),
pet_eye: Style::default().fg(p.good.rgb).add_modifier(Modifier::BOLD),
pet_mouth: Style::default().fg(p.dim.rgb),
pet_accent: Style::default().fg(p.accent.rgb),
pet_pattern: Style::default().fg(p.faint.rgb),
```

- [ ] **Step 5: Render pet art as role-aware spans**

Replace `centered_art_lines` with a version that accepts `&WatchViewModel`. For each `pet_art` line, split characters according to `StyledSegment` ranges and map roles:

```rust
fn role_style(role: PaletteRoleName, styles: &SemanticStyles) -> Style {
    match role {
        PaletteRoleName::Body => styles.pet_body,
        PaletteRoleName::Eye => styles.pet_eye,
        PaletteRoleName::Mouth => styles.pet_mouth,
        PaletteRoleName::Accent => styles.pet_accent,
        PaletteRoleName::Pattern => styles.pet_pattern,
    }
}
```

Preserve left padding as an unstyled raw span so centering still works.

- [ ] **Step 6: Run role and existing TUI tests**

Run:

```bash
cargo test pet_renderer_roles_reach_tui_cells tui_render
```

Expected: role colors are visible, and existing layout/color tests still pass.

- [ ] **Step 7: Commit**

```bash
git add src/tui/view_model.rs src/tui/style.rs src/tui/layout.rs src/commands/watch.rs tests/tui_render.rs
git commit -m "feat: render pet roles in terminal colors"
```

---

## Task 4: Pet-First Layout, Source Rows, Compact Art, And Help Toggle

**Files:**
- Modify: `src/tui/layout.rs`
- Modify: `src/tui/app.rs`
- Test: `tests/tui_render.rs`

- [ ] **Step 1: Write failing layout and interaction tests**

Append to `tests/tui_render.rs`:

```rust
use glorp::tui::view_model::{SourceHealthView, SourceStatus};

#[test]
fn wide_layout_leads_with_pet_before_vitals_metadata() {
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let lines = buffer_lines(terminal.backend().buffer());
    let first_pet = lines.iter().position(|line| line.contains("/\\_/\\")).unwrap();
    let vitals = lines.iter().position(|line| line.contains("vitals")).unwrap();
    assert!(first_pet < vitals);
}

#[test]
fn source_health_rows_render_ready_and_diagnostic_states_together() {
    let mut vm = WatchViewModel::fixture();
    vm.source_health = vec![
        SourceHealthView {
            name: "claude-code".into(),
            status: SourceStatus::Ready,
            today_effective_tokens: 4_200.0,
            bucket_effective_tokens: 1_300.0,
            diagnostic_code: None,
            diagnostic_message: None,
        },
        SourceHealthView {
            name: "codex".into(),
            status: SourceStatus::Diagnostic,
            today_effective_tokens: 0.0,
            bucket_effective_tokens: 0.0,
            diagnostic_code: Some("missing_helper".into()),
            diagnostic_message: Some("ccusage-codex helper was not found".into()),
        },
    ];

    let backend = TestBackend::new(100, 28);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(text.contains("claude-code"));
    assert!(text.contains("ready"));
    assert!(text.contains("codex"));
    assert!(text.contains("missing_helper"));
}

#[test]
fn question_mark_toggles_help_overlay() {
    let mut app = WatchApp::with_config(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
        },
    );
    assert!(!app.help_visible_for_test());
    app.handle_key_for_test(KeyCode::Char('?'), KeyEventKind::Press).unwrap();
    assert!(app.help_visible_for_test());
    app.handle_key_for_test(KeyCode::Char('?'), KeyEventKind::Press).unwrap();
    assert!(!app.help_visible_for_test());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test wide_layout_leads_with_pet_before_vitals_metadata source_health_rows_render_ready_and_diagnostic_states_together question_mark_toggles_help_overlay
```

Expected: first test fails because current left panel starts with `vitals`, source-health types are not rendered yet, and help does not toggle.

- [ ] **Step 3: Reorder pet panel**

In `render_pet_panel`, change row order to:

1. pet stage/art
2. identity metadata
3. vitals/stats

Rename local builders for clarity:

```rust
let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(stage_height),
        Constraint::Length(meta_height),
        Constraint::Length(stats_height),
        Constraint::Min(0),
    ])
    .split(area);
```

Render section header `pet` or no header above art; render metadata immediately below art; render `vitals` and bars after metadata.

- [ ] **Step 4: Render source-health rows**

In `render_activity_panel`, replace `source_breakdown` rendering with `source_health`:

```rust
for source in vm.source_health.iter().take(4) {
    let status_style = match source.status {
        SourceStatus::Ready => styles.event_rail_usage,
        SourceStatus::Diagnostic => styles.event_rail_diagnostic,
        SourceStatus::Blocked => styles.blocked,
    };
    let status = match source.status {
        SourceStatus::Ready => "ready",
        SourceStatus::Diagnostic => source.diagnostic_code.as_deref().unwrap_or("diagnostic"),
        SourceStatus::Blocked => "blocked",
    };
    lines.push(Line::from(vec![
        Span::styled("▏", status_style),
        Span::raw(" "),
        Span::styled(source.name.as_str(), styles.label),
        Span::styled(" ", styles.prompt_sep),
        Span::styled(status, status_style),
        Span::styled(" ", styles.prompt_sep),
        Span::styled(format_tokens(source.today_effective_tokens), styles.primary_text),
    ]));
}
```

- [ ] **Step 5: Toggle help on `?`**

In `handle_key`:

```rust
KeyCode::Char('?') => {
    self.overlay = match self.overlay {
        Some(Overlay::Help) => None,
        None => Some(Overlay::Help),
    };
    Ok(false)
}
```

Add:

```rust
pub fn help_visible_for_test(&self) -> bool {
    self.overlay == Some(Overlay::Help)
}
```

- [ ] **Step 6: Keep compact rendering intentional**

Ensure `render_compact` calls the same pet panel renderer with a view model whose `pet_art` came from `AnimationFrame { compact: true, ... }`. The app-level compact detection from Task 2 should set this before drawing; add a test that compact art contains no line wider than 18 characters:

```rust
#[test]
fn compact_layout_uses_compact_pet_art_width() {
    let mut app = WatchApp::with_config(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
        },
    );
    app.set_compact_for_test(true);
    app.advance_animation_for_test();
    assert!(app
        .view_model_for_test()
        .pet_art
        .iter()
        .all(|line| line.chars().count() <= 18));
}
```

- [ ] **Step 7: Run focused TUI interaction tests**

Run:

```bash
cargo test wide_layout_leads_with_pet_before_vitals_metadata source_health_rows_render_ready_and_diagnostic_states_together question_mark_toggles_help_overlay compact_layout_uses_compact_pet_art_width
```

Expected: all named tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/tui/layout.rs src/tui/app.rs tests/tui_render.rs
git commit -m "feat: refine watch layout and controls"
```

---

## Task 5: Refresh Debounce And Evolution Moment Lifecycle

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/view_model.rs`
- Modify: `src/tui/layout.rs`
- Modify: `src/commands/watch.rs`
- Test: `tests/tui_render.rs`
- Test: `tests/watch_integration.rs`

- [ ] **Step 1: Write failing refresh debounce test**

Append to `tests/tui_render.rs`:

```rust
#[test]
fn manual_refresh_resets_interval_timer_for_test() {
    let harness = WatchTestHarness::with_usage_delta("claude-code", "2026-05-09T13:42:00Z", 1300.0);
    let mut app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
        },
        Box::new(harness),
    );

    app.handle_key_for_test(KeyCode::Char('r'), KeyEventKind::Press).unwrap();
    assert_eq!(app.poll_count_for_test(), 1);
    assert!(!app.interval_due_for_test(Duration::from_secs(1)));
    assert!(app.interval_due_for_test(Duration::from_secs(61)));
}
```

- [ ] **Step 2: Write failing evolution moment test**

Append to `tests/watch_integration.rs`:

```rust
#[test]
fn latest_evolution_renders_once_for_running_watch() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let _usage_store = UsageStore::open(&usage_db).unwrap();
    let mut state = mech_state();
    state.seen_stage_transitions = vec!["s0->s1".into()];

    let mut vm = build_watch_view_model_for_test(&state, &usage_db).unwrap();
    assert_eq!(vm.latest_evolution.as_deref(), Some("s0->s1"));
    assert!(!vm.acknowledged_evolution_for_test("s0->s1"));
    vm.acknowledge_latest_evolution_for_test();
    assert!(vm.acknowledged_evolution_for_test("s0->s1"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
cargo test manual_refresh_resets_interval_timer_for_test latest_evolution_renders_once_for_running_watch
```

Expected: compile failure for test hooks and acknowledgement helpers.

- [ ] **Step 4: Track last poll instant through testable elapsed time**

In `WatchApp`, add `last_poll_elapsed_for_test: Duration` or expose a helper that compares an injected elapsed duration against the interval after manual refresh. Keep production using `Instant`.

Production path:

```rust
KeyCode::Char('r') => {
    self.poll_usage()?;
    self.last_poll = Instant::now();
    Ok(false)
}
```

If `last_poll` is currently local to `run_on_terminal`, move it into `WatchApp`.

Test helper:

```rust
pub fn interval_due_for_test(&self, elapsed_since_last_poll: Duration) -> bool {
    elapsed_since_last_poll >= self.config.usage_poll_interval
}
```

- [ ] **Step 5: Add transient evolution acknowledgement to view model**

In `WatchViewModel`, add:

```rust
pub acknowledged_evolution: Option<String>,
```

Methods:

```rust
pub fn should_render_evolution_moment(&self) -> bool {
    self.latest_evolution.is_some() && self.latest_evolution != self.acknowledged_evolution
}

pub fn acknowledge_latest_evolution(&mut self) {
    self.acknowledged_evolution = self.latest_evolution.clone();
}
```

Expose `*_for_test` wrappers only if direct methods would pollute public API expectations.

- [ ] **Step 6: Render and acknowledge the moment**

In the app draw loop:

```rust
if self.vm.should_render_evolution_moment() {
    render_evolution_overlay(frame);
}
```

After one successful draw containing the overlay, call `self.vm.acknowledge_latest_evolution()`. Keep this transient; do not write acknowledgement to disk.

- [ ] **Step 7: Run focused tests**

Run:

```bash
cargo test manual_refresh_resets_interval_timer_for_test latest_evolution_renders_once_for_running_watch help_evolution_and_hatch_overlays_use_tokenpet_surface_and_accent
```

Expected: all named tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/tui/app.rs src/tui/view_model.rs src/tui/layout.rs src/commands/watch.rs tests/tui_render.rs tests/watch_integration.rs
git commit -m "feat: handle refresh timing and evolution moments"
```

---

## Task 6: Watch Presentation Verification Gate

**Files:**
- Modify as needed from earlier tasks only.

- [ ] **Step 1: Run full Rust verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit 0.

- [ ] **Step 2: Manual watch smoke**

Run with a temporary config dir so the real local pet is untouched:

```bash
tmpdir="$(mktemp -d)"
GLORP_CONFIG_DIR="$tmpdir" cargo run -- init --yes --seed watch-smoke --name bolt
GLORP_CONFIG_DIR="$tmpdir" cargo run -- watch
```

Expected in the terminal:

- Pet appears before vitals in the left panel.
- `?` opens help and pressing `?` again closes it.
- `r` refreshes without an immediate second helper poll.
- Source rows show ready and diagnostic sources separately when one helper is unavailable.
- Pet art animates without waiting for a provider poll.

Stop watch with `q`.

- [ ] **Step 3: Check scope**

Run:

```bash
git diff --stat HEAD~5..HEAD
git status --short
```

Expected: changes are limited to watch commands, TUI files, view-model tests, and integration tests. Packaging remains untouched.
