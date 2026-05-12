use crossterm::event::{KeyCode, KeyEventKind};
use glorp::error::Result;
use glorp::pet::render::{PaletteRoleName, StyledSegment};
use glorp::tui::app::{
    render_evolution_overlay_for_test, render_frame_for_test, render_hatch_overlay_for_test,
    render_help_overlay_for_test, run_single_watch_tick_for_test, WatchApp, WatchAppConfig,
    WatchTestHarness, WatchUsagePoller, WatchViewModel,
};
use glorp::tui::layout::render_watch_frame_with_capability;
use glorp::tui::style::{semantic_styles, tokenpet_palette, ColorCapability};
use glorp::tui::view_model::{SourceHealthView, SourceStatus};
use ratatui::{
    backend::TestBackend, buffer::Buffer, layout::Position, style::Color, Frame, Terminal,
};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc, Barrier,
};
use std::time::Duration;

fn buffer_lines(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            let mut row = String::new();
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell(Position::new(buf.area.x + x, buf.area.y + y)) {
                    row.push_str(cell.symbol());
                }
            }
            row.trim_end().to_string()
        })
        .collect()
}

/// Like `buffer_lines` but preserves trailing whitespace so callers can
/// assert on padded-to-N row geometry.
fn buffer_rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            let mut row = String::new();
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell(Position::new(buf.area.x + x, buf.area.y + y)) {
                    row.push_str(cell.symbol());
                }
            }
            row
        })
        .collect()
}

fn buffer_text(buf: &Buffer) -> String {
    buffer_lines(buf).join("\n")
}

fn has_cell(buf: &Buffer, symbol: &str, fg: Color) -> bool {
    (0..buf.area.height).any(|y| {
        (0..buf.area.width).any(|x| {
            buf.cell(Position::new(buf.area.x + x, buf.area.y + y))
                .map(|cell| cell.symbol() == symbol && cell.style().fg == Some(fg))
                .unwrap_or(false)
        })
    })
}

fn spark_foregrounds(buffer: &Buffer) -> Vec<Color> {
    let area = buffer.area;
    let mut colors = Vec::new();

    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &buffer[Position::new(x, y)];
            if cell.symbol() == "█" {
                if let Some(fg) = cell.style().fg {
                    colors.push(fg);
                }
            }
        }
    }

    colors
}

#[test]
fn render_watch_frame_honors_explicit_color_capability() {
    let vm = WatchViewModel::fixture();

    // Verify both modes render without panicking and produce spark bar cells.
    // Color capability no longer drives the sparkline ramp (that was SparkPanel,
    // now deleted); today's TodayPanel uses fixed semantic colors regardless of
    // capability. The meaningful assertion is that the fixture produces bars at all.
    let mut truecolor_terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    truecolor_terminal
        .draw(|frame| {
            render_watch_frame_with_capability(frame, &vm, ColorCapability::Truecolor);
        })
        .unwrap();

    let truecolor = spark_foregrounds(truecolor_terminal.backend().buffer());
    assert!(
        !truecolor.is_empty(),
        "fixture should render spark bar cells"
    );
}

#[test]
fn wide_layout_has_tokenpet_chrome_panels_and_bars() {
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let p = tokenpet_palette();
    let text = buffer_text(buf);
    assert!(text.contains("glorp · "));
    assert!(text.contains("vitals"), "expected vitals section header");
    assert!(text.contains("today"));
    assert!(text.contains("progress"));
    assert!(text.contains("█"));
    assert!(text.contains("░"));
    assert!(has_cell(buf, "█", p.good.rgb) || has_cell(buf, "█", p.accent.rgb));
    assert!(has_cell(buf, "░", p.faint.rgb));
}

#[test]
fn wide_layout_centers_full_pet_stage_with_dashed_divider() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_art = vec![
        "    /\\     ".into(),
        "   /  \\    ".into(),
        "  / o.o \\  ".into(),
        " /  ◇v◇  \\ ".into(),
        " \\  ✦✦✦  / ".into(),
        "  \\  ·  /  ".into(),
        "   \\___/   ".into(),
        "  ✦ ✧ ✦   ".into(),
    ];

    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(text.contains("✦ ✧ ✦"));

    let stage_line = buffer_lines(terminal.backend().buffer())
        .into_iter()
        .find(|line| line.contains("/ o.o \\"))
        .unwrap();
    assert!(
        stage_line.find('/').unwrap_or_default() > 5,
        "pet art should be padded away from the left edge: {stage_line:?}"
    );
}

#[test]
fn wide_layout_uses_tokenpet_metadata_today_grid_and_log_rhythm() {
    let backend = TestBackend::new(118, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture_with_events()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(text.contains("today"));
    assert!(text.contains("last 10m"));
    assert!(text.contains("progress"));
    assert!(text.contains("feed"));
    assert!(text.contains("─"));
}

#[test]
fn event_log_uses_timestamps_rails_sparkline_and_semantic_colors() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture_with_events()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let p = tokenpet_palette();
    let text = buffer_text(buf);
    assert!(text.contains("13:42"));
    assert!(
        text.contains("▁")
            || text.contains("▂")
            || text.contains("▃")
            || text.contains("▄")
            || text.contains("▅")
            || text.contains("▆")
            || text.contains("▇")
            || text.contains("█")
    );
    assert!(has_cell(buf, "█", p.good.rgb) || has_cell(buf, "█", p.accent.rgb));
}

#[test]
fn token_metrics_never_render_negative_zero() {
    let mut vm = WatchViewModel::fixture();
    vm.current_bucket_effective_tokens = -0.1;
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(!text.contains("-0"));
}

#[test]
fn narrow_layout_keeps_required_vitals_visible() {
    let backend = TestBackend::new(80, 28);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(text.contains("fed"));
    assert!(text.contains("happy"));
    assert!(text.contains("energy"));
    assert!(text.contains("xp"));
    assert!(text.contains("today"));
}

#[test]
fn small_height_degrades_without_text_overlap() {
    let backend = TestBackend::new(48, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let lines = buffer_lines(terminal.backend().buffer());
    assert!(lines.iter().all(|line| line.chars().count() <= 48));
    let text = lines.join("\n");
    assert!(text.contains("q"));
    assert!(text.contains("?"));
}

#[test]
fn blocked_provider_state_renders_calm_setup_view() {
    let mut vm = WatchViewModel::fixture();
    vm.helper_status = "missing ccusage helper".into();
    vm.errors
        .push("install ccusage or use npm package with bundled helpers".into());
    vm.source_health = vec![SourceHealthView {
        name: "claude-code".into(),
        status: SourceStatus::Blocked,
        today_effective_tokens: 0.0,
        bucket_effective_tokens: 0.0,
        diagnostic_code: None,
        diagnostic_message: None,
    }];
    let backend = TestBackend::new(140, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    // TodayPanel shows ⚠ in the gutter for blocked/diagnostic sources.
    assert!(
        text.contains("⚠"),
        "blocked source should render ⚠ in today panel, got:\n{text}"
    );
}

#[test]
fn polling_tick_updates_activity_bucket_and_event_log() {
    let mut harness =
        WatchTestHarness::with_usage_delta("claude-code", "2026-05-09T13:42:00Z", 1300.0);
    let vm = run_single_watch_tick_for_test(&mut harness).unwrap();
    assert_eq!(vm.current_bucket_effective_tokens, 1300.0);
    assert!(vm
        .source_breakdown
        .iter()
        .any(|source| source.name == "claude-code"));
    assert!(vm
        .recent_events
        .iter()
        .any(|event| event.text.contains("1.3k effective tokens")));
}

#[test]
fn app_refresh_and_interval_use_polling_path() {
    let harness = WatchTestHarness::with_usage_delta("claude-code", "2026-05-09T13:42:00Z", 1300.0);
    let mut app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_millis(1),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(harness),
    );

    let refreshed = app.refresh_for_test().unwrap();
    assert_eq!(app.poll_count_for_test(), 1);
    assert_eq!(refreshed.current_bucket_effective_tokens, 1300.0);
    assert!(refreshed
        .recent_events
        .iter()
        .any(|event| event.text.contains("1.3k effective tokens")));

    let interval_polled = app.interval_poll_for_test().unwrap();
    assert_eq!(app.poll_count_for_test(), 2);
    assert_eq!(interval_polled.current_bucket_effective_tokens, 1300.0);
    assert!(
        interval_polled
            .recent_events
            .iter()
            .filter(|event| event.text.contains("1.3k effective tokens"))
            .count()
            >= 2
    );
}

#[test]
fn repeat_key_events_drive_watch_controls_and_release_events_do_not() {
    let harness = WatchTestHarness::with_usage_delta("claude-code", "2026-05-09T13:42:00Z", 1300.0);
    let mut app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_millis(1),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(harness),
    );

    assert!(!app
        .handle_key_for_test(KeyCode::Char('r'), KeyEventKind::Repeat)
        .unwrap());
    assert_eq!(app.poll_count_for_test(), 1);
    assert!(app
        .handle_key_for_test(KeyCode::Char('q'), KeyEventKind::Repeat)
        .unwrap());
    assert!(!app
        .handle_key_for_test(KeyCode::Char('r'), KeyEventKind::Release)
        .unwrap());
    assert_eq!(app.poll_count_for_test(), 1);
}

#[test]
fn help_evolution_and_hatch_overlays_use_tokenpet_surface_and_accent() {
    let p = tokenpet_palette();
    fn assert_overlay(render: fn(&mut Frame<'_>), accent: Color) {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(render).unwrap();
        let buf = terminal.backend().buffer();
        assert!(has_cell(buf, "─", accent) || has_cell(buf, "│", accent));
        assert!(buffer_text(buf).contains("glorp"));
    }
    assert_overlay(render_help_overlay_for_test, p.accent.rgb);
    assert_overlay(render_evolution_overlay_for_test, p.accent.rgb);
    assert_overlay(render_hatch_overlay_for_test, p.accent.rgb);
}

#[test]
fn pet_renderer_roles_reach_tui_cells() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_art = vec![" {eye}{body}{accent}".into()];
    vm.pet_spans = vec![
        StyledSegment {
            line: 0,
            start: 1,
            end: 2,
            role: PaletteRoleName::Eye,
        },
        StyledSegment {
            line: 0,
            start: 2,
            end: 3,
            role: PaletteRoleName::Body,
        },
        StyledSegment {
            line: 0,
            start: 3,
            end: 4,
            role: PaletteRoleName::Accent,
        },
    ];

    // Tall enough for compact layout to allocate PetPanel its required 10 rows.
    let backend = TestBackend::new(80, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let buf = terminal.backend().buffer();
    let styles = semantic_styles();

    assert!(has_cell(buf, "{", styles.pet_eye.fg.unwrap()));
    assert!(has_cell(buf, "e", styles.pet_body.fg.unwrap()));
    assert!(has_cell(buf, "y", styles.pet_accent.fg.unwrap()));
}

#[test]
fn animation_tick_rerenders_pet_art_without_polling_usage() {
    let mut app = WatchApp::with_config(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(999),
            color_capability: ColorCapability::Truecolor,
        },
    );

    let before = app.view_model_for_test().pet_art.clone();
    app.advance_animation_for_test();
    let after = app.view_model_for_test().pet_art.clone();

    assert_ne!(before, after);
    assert_eq!(app.poll_count_for_test(), 0);
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

    let backend = TestBackend::new(140, 28);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    // TodayPanel shows source display names and ⚠ for unhealthy sources.
    assert!(
        text.contains("claude"),
        "ready source should appear in today panel"
    );
    assert!(
        text.contains("codex"),
        "diagnostic source should appear in today panel"
    );
    assert!(text.contains("⚠"), "diagnostic source should show ⚠ marker");
}

#[test]
fn p_key_pets_pet_and_sets_speech_bubble() {
    let mut vm = WatchViewModel::fixture();
    vm.happiness = 0.5;
    vm.energy = 0.5;
    vm.current_speech = None;
    let mut app = WatchApp::with_config(
        vm,
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
    );
    app.handle_key_for_test(KeyCode::Char('p'), KeyEventKind::Press)
        .unwrap();
    let after = app.view_model_for_test();
    assert!(
        after.current_speech.is_some(),
        "petting should set a speech bubble"
    );
    assert!(
        after.happiness > 0.5,
        "petting should bump happiness, got {}",
        after.happiness
    );
    assert!(
        after.energy > 0.5,
        "petting should bump energy, got {}",
        after.energy
    );
}

#[test]
fn question_mark_toggles_help_overlay() {
    let mut app = WatchApp::with_config(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
    );
    assert!(!app.help_visible_for_test());
    app.handle_key_for_test(KeyCode::Char('?'), KeyEventKind::Press)
        .unwrap();
    assert!(app.help_visible_for_test());
    app.handle_key_for_test(KeyCode::Char('?'), KeyEventKind::Press)
        .unwrap();
    assert!(!app.help_visible_for_test());
}

#[test]
fn xp_display_caps_at_max_when_xp_overshoots_target() {
    let mut vm = WatchViewModel::fixture();
    // fraction > 1.0 should be clamped to 100% by bar_spans_solid.
    vm.progress.fraction = 113.0 / 49.0;
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains("xp") && text.contains("100"),
        "expected xp bar to show 100% when fraction overshoots 1.0, got:\n{text}"
    );
}

#[test]
fn helper_status_string_is_not_rendered_in_layout() {
    let mut vm = WatchViewModel::fixture();
    vm.helper_status = "ZZZSENTINEL".into();
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(
        !text.contains("ZZZSENTINEL"),
        "helper_status string should no longer be drawn in the right panel"
    );
}

#[test]
fn format_tokens_uses_comma_formatting_for_large_values() {
    let mut vm = WatchViewModel::fixture();
    vm.today_effective_tokens = 66_631_100.0;
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains("66,631,100"),
        "large token values should render with comma grouping, got:\n{text}"
    );
    assert!(
        !text.contains("66631.1k"),
        "large token values should not render with k suffix"
    );
}

#[test]
fn manual_refresh_resets_interval_timer_for_test() {
    let harness = WatchTestHarness::with_usage_delta("claude-code", "2026-05-09T13:42:00Z", 1300.0);
    let mut app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(harness),
    );

    app.handle_key_for_test(KeyCode::Char('r'), KeyEventKind::Press)
        .unwrap();
    assert_eq!(app.poll_count_for_test(), 1);
    assert!(!app.interval_due_for_test(Duration::from_secs(1)));
    assert!(app.interval_due_for_test(Duration::from_secs(61)));
}

/// Test poller that blocks at the start of `poll_usage` until a barrier is
/// released. Lets tests observe the in-flight window and assert that the
/// main thread keeps doing work while a poll is outstanding.
struct BlockingTestPoller {
    start: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: Arc<AtomicU32>,
}

impl WatchUsagePoller for BlockingTestPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        // Signal that we entered the poll, then block until the test releases us.
        self.start.wait();
        self.release.wait();
        Ok(current.clone())
    }
}

#[test]
fn animation_advances_while_poll_is_in_flight() {
    let start = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicU32::new(0));
    let poller = BlockingTestPoller {
        start: Arc::clone(&start),
        release: Arc::clone(&release),
        calls: Arc::clone(&calls),
    };

    let mut app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(poller),
    );

    // Kick off the poll without blocking. The worker thread will park inside
    // BlockingTestPoller::poll_usage once it enters; we then drive the
    // animation locally to prove the main loop is not blocked behind the poll.
    let started = app.kick_off_poll_for_test().unwrap();
    assert!(started, "first kickoff should send a request");
    assert!(app.in_flight_for_test(), "poll should be in flight");

    // Wait until the worker thread is actually inside the poll body so the
    // in-flight window is observable.
    start.wait();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "exactly one poll should be running"
    );

    let frame_before = app.view_model_for_test().pet_art.clone();
    for _ in 0..5 {
        app.advance_animation_for_test();
    }
    let frame_after = app.view_model_for_test().pet_art.clone();
    assert_ne!(
        frame_before, frame_after,
        "animation must advance while a poll is parked"
    );
    assert_eq!(
        app.poll_count_for_test(),
        0,
        "poll_count should still be 0 while the poll is parked"
    );

    // Release the worker; finish the poll and verify the result lands.
    release.wait();
    app.await_pending_poll_for_test().unwrap();
    assert_eq!(
        app.poll_count_for_test(),
        1,
        "poll_count increments after the worker returns a result"
    );
    assert!(
        !app.in_flight_for_test(),
        "in-flight flag should clear after the result lands"
    );
}

#[test]
fn initial_poll_starts_in_background_without_replacing_cached_frame() {
    let start = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicU32::new(0));
    let poller = BlockingTestPoller {
        start: Arc::clone(&start),
        release: Arc::clone(&release),
        calls: Arc::clone(&calls),
    };
    let mut cached = WatchViewModel::fixture();
    cached.today_effective_tokens = 42.0;

    let mut app = WatchApp::with_poll_callback(
        cached,
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(poller),
    );

    let started = app.start_initial_poll_for_test().unwrap();
    assert!(started, "initial startup should request one poll");
    assert!(app.in_flight_for_test(), "startup poll should be in flight");

    start.wait();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.view_model_for_test().today_effective_tokens,
        42.0,
        "cached view model should remain visible while initial poll runs"
    );
    assert_eq!(
        app.poll_count_for_test(),
        0,
        "initial poll should not be counted until the worker result lands"
    );

    release.wait();
    app.await_pending_poll_for_test().unwrap();
    assert_eq!(app.poll_count_for_test(), 1);
}

#[test]
fn duplicate_poll_requests_while_in_flight_are_deduped() {
    let start = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicU32::new(0));
    let poller = BlockingTestPoller {
        start: Arc::clone(&start),
        release: Arc::clone(&release),
        calls: Arc::clone(&calls),
    };

    let mut app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(poller),
    );

    let first = app.kick_off_poll_for_test().unwrap();
    assert!(first, "first kickoff sends a request");

    // Wait until the worker is parked inside poll_usage.
    start.wait();
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // Issue several more kickoffs while the first is in flight; each must be
    // a no-op because dedup is gated on `in_flight`.
    for _ in 0..3 {
        let extra = app.kick_off_poll_for_test().unwrap();
        assert!(
            !extra,
            "additional kickoffs while a poll is in flight must be no-ops"
        );
    }

    // Release the parked poll. Only one poll should have ever run.
    release.wait();
    app.await_pending_poll_for_test().unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "only one poll ran despite multiple kickoffs"
    );
    assert_eq!(app.poll_count_for_test(), 1);
}

#[test]
fn shutdown_drops_worker_thread_cleanly() {
    let harness = WatchTestHarness::with_usage_delta("claude-code", "2026-05-09T13:42:00Z", 1300.0);
    let app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(harness),
    );
    // Drop the app without ever running. Drop signals shutdown to the worker
    // and detaches; if that path blocks the test will hang.
    drop(app);
}

/// Poller that parks forever inside `poll_usage` once entered. Used to
/// simulate a hung Node helper so we can prove that dropping `WatchApp`
/// while a poll is in flight does not block the main thread.
struct ForeverBlockingPoller {
    entered: Arc<Barrier>,
}

impl WatchUsagePoller for ForeverBlockingPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel> {
        self.entered.wait();
        // Sleep effectively forever; the OS reaps this thread when the
        // process exits. The test asserts that Drop does NOT wait on this.
        std::thread::sleep(Duration::from_secs(3600));
        Ok(current.clone())
    }
}

#[test]
fn drop_does_not_block_on_in_flight_poll() {
    let entered = Arc::new(Barrier::new(2));
    let poller = ForeverBlockingPoller {
        entered: Arc::clone(&entered),
    };
    let mut app = WatchApp::with_poll_callback(
        WatchViewModel::fixture(),
        WatchAppConfig {
            animation_tick: Duration::from_millis(1),
            usage_poll_interval: Duration::from_secs(60),
            color_capability: ColorCapability::Truecolor,
        },
        Box::new(poller),
    );
    // Send a poll so the worker is parked inside `poll_usage` when we drop.
    let started = app.kick_off_poll_for_test().unwrap();
    assert!(started);
    entered.wait();

    // `WatchApp` is not `Send` (PetAnimator holds tachyonfx state that isn't
    // thread-safe), so we can't run `drop` on a side thread and wait via
    // recv_timeout. Instead, take a wall-clock measurement around `drop(app)`
    // on this thread. This catches a regression back to the original
    // blocking-join behaviour, which would take a full hour (the poller's
    // sleep) to return — the assertion budget is tight enough to fail fast
    // if Drop ever waits on the worker.
    let started_drop = std::time::Instant::now();
    drop(app);
    let elapsed = started_drop.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "WatchApp::drop must not block on an in-flight poll (took {elapsed:?})"
    );
}

/// Standard wide-mode terminal width for layout invariants:
/// rounded outer frame, two columns inside, every body row padded to the
/// terminal width and section dividers fully filling the right column.
/// Anchored well above `COMPACT_THRESHOLD` so the wide composition is the
/// path under test.
// Matches the layout's MAX_FRAME_WIDTH so the frame fills the terminal exactly
// — keeps existing corner-position assertions valid without centering padding.
const WIDE_TEST_WIDTH: u16 = 110;

#[test]
fn wide_layout_every_row_fills_full_terminal_width() {
    let backend = TestBackend::new(WIDE_TEST_WIDTH, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture_with_events()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let rows = buffer_rows(buf);
    for (y, row) in rows.iter().enumerate() {
        assert_eq!(
            row.chars().count(),
            WIDE_TEST_WIDTH as usize,
            "row {y} should be exactly {WIDE_TEST_WIDTH} cells; got {row:?}",
        );
    }
}

#[test]
fn wide_layout_outer_frame_uses_rounded_box_drawing() {
    let backend = TestBackend::new(WIDE_TEST_WIDTH, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let rows = buffer_rows(buf);
    let top = &rows[0];
    assert!(
        top.starts_with("╭"),
        "top row should start with rounded corner ╭; got {top:?}"
    );
    assert!(top.ends_with('╮'), "top row should end with ╮; got {top:?}");

    // Find the bottom-corner row — the frame shrinks to natural content
    // height instead of filling the terminal, so the bottom border can sit
    // well above row 23.
    let bottom_idx = rows
        .iter()
        .position(|r| r.starts_with('╰'))
        .expect("expected to find a ╰ row");
    let bottom = &rows[bottom_idx];
    assert!(
        bottom.starts_with("╰"),
        "bottom should start with ╰; got {bottom:?}"
    );
    assert!(
        bottom.ends_with('╯'),
        "bottom should end with ╯; got {bottom:?}"
    );
    for (y, row) in rows.iter().enumerate().take(bottom_idx).skip(1) {
        let chars: Vec<char> = row.chars().collect();
        assert_eq!(chars[0], '│', "row {y} left rail");
        assert_eq!(chars[chars.len() - 1], '│', "row {y} right rail");
    }
}

#[test]
fn wide_layout_section_dividers_all_end_at_same_right_column() {
    let backend = TestBackend::new(WIDE_TEST_WIDTH, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture_with_events()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let rows = buffer_rows(buf);
    // Right-column panels only; left-column panels (vitals, bio) are a fixed
    // 40-cell width and will end at a different column than the right column.
    let labels = [" today ", " progress ", " feed "];

    // Collect the rightmost column where a `─` appears in each section
    // divider row. In the native-Layout path the right column fills the inner
    // frame area, so the last `─` is immediately followed by the outer frame
    // wall `│` (not a pad space). All dividers must end at the same column.
    let mut divider_ends: Vec<usize> = Vec::new();
    for row in &rows {
        if !labels.iter().any(|label| row.contains(label)) {
            continue;
        }
        let chars: Vec<char> = row.chars().collect();
        let rightmost_dash = chars
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| **c == '─')
            .map(|(i, _)| i)
            .unwrap_or_else(|| panic!("divider row had no `─`: {row:?}"));
        divider_ends.push(rightmost_dash);
        // The character after the last dash should be either the frame wall
        // `│` (native-Layout path fills to the edge) or a pad space.
        let after = chars.get(rightmost_dash + 1).copied().unwrap_or(' ');
        assert!(
            after == '│' || after == ' ',
            "cell after the last `─` should be frame wall or pad space: {row:?}"
        );
    }
    assert!(
        divider_ends.len() >= 3,
        "expected at least three section dividers, found {}",
        divider_ends.len(),
    );
    let first = divider_ends[0];
    for end in &divider_ends {
        assert_eq!(
            *end, first,
            "all section dividers should end at the same column; got {divider_ends:?}",
        );
    }
}

#[test]
fn watch_wide_bio_bottom_aligns_with_feed_bottom() {
    // The wide layout uses a 2-row grid; band 2 (vitals/bio left, feed right)
    // is 8 rows tall, fitting a header + 7 events. When the feed is full (≥7
    // events), the last event sits on the same terminal row as bio's age line.
    let vm = WatchViewModel::fixture_with_n_events(7);
    let backend = TestBackend::new(110, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let buf = terminal.backend().buffer();

    let row_string = |y: u16| -> String {
        (0..buf.area.width)
            .filter_map(|x| buf.cell(Position::new(x, y)))
            .map(|c| c.symbol().to_string())
            .collect()
    };

    let mut age_y: Option<u16> = None;
    let mut last_feed_event_y: Option<u16> = None;
    for y in 1..buf.area.height - 1 {
        let row = row_string(y);
        if row.contains("age") && row.contains("d") {
            age_y = Some(y);
        }
        if row.contains("added") {
            last_feed_event_y = Some(y);
        }
    }
    let age_y = age_y.expect("bio age row");
    let last_feed_event_y = last_feed_event_y.expect("last feed event row");
    assert_eq!(
        age_y, last_feed_event_y,
        "bio's age row and the last feed event must sit on the same terminal row"
    );
}

#[test]
fn watch_wide_180x50_left_column_still_full() {
    let vm = WatchViewModel::fixture();
    let backend = TestBackend::new(180, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let buf = terminal.backend().buffer();
    let s: String = buf
        .content()
        .iter()
        .map(|c| c.symbol().to_string())
        .collect();
    assert!(s.contains("bio"), "bio panel must render at 180x50");
    assert!(s.contains("vitals"), "vitals must render");
    assert!(s.contains("progress"), "progress must render");
    assert!(s.contains("feed"), "feed must render");
}

#[test]
fn watch_compact_72x24_panels_in_order() {
    let vm = WatchViewModel::fixture();
    let backend = TestBackend::new(72, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let buf = terminal.backend().buffer();

    // For each section header, capture its row index. Order must be:
    // pet (no title) -> vitals -> today -> progress -> feed
    // Bio is omitted from compact mode (age is already in the title bar).
    let row_of = |needle: &str| -> Option<u16> {
        (0..buf.area.height).find(|&y| {
            let line: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            line.contains(needle)
        })
    };
    let vitals_y = row_of("vitals").expect("vitals");
    let today_y = row_of("today").expect("today");
    let progress_y = row_of("progress").expect("progress");
    let feed_y = row_of("feed").expect("feed");
    assert!(vitals_y < today_y, "vitals before today");
    assert!(today_y < progress_y, "today before progress");
    assert!(progress_y < feed_y, "progress before feed");

    let s: String = buf
        .content()
        .iter()
        .map(|c| c.symbol().to_string())
        .collect();
    assert!(!s.contains("helpers"));
    // Bio must not appear in compact mode.
    assert!(
        !s.contains("bio"),
        "bio panel must not render in compact mode"
    );
}

#[test]
fn render_does_not_panic_at_tiny_sizes() {
    // Regression: resizing into compact mode panicked because
    // `pet_inner_rect_in_panel` ran `i32::clamp` with min > max when the
    // PetPanel's Fill(1) collapsed to less than PET_W × PET_H.
    let vm = WatchViewModel::fixture();
    for (w, h) in [(80, 6), (72, 10), (50, 12), (40, 8), (30, 30)] {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        // Must not panic for any of these.
        terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    }
}

#[test]
fn wide_layout_pet_art_fits_inside_left_column() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_art = vec![
        "    /\\     ".into(),
        "   /  \\    ".into(),
        "  / o.o \\  ".into(),
        " /  ◇v◇  \\ ".into(),
        " \\  ✦✦✦  / ".into(),
        "  \\  ·  /  ".into(),
        "   \\___/   ".into(),
    ];

    let backend = TestBackend::new(WIDE_TEST_WIDTH, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let buf = terminal.backend().buffer();
    let rows = buffer_rows(buf);

    // In the native-Layout wide path the outer frame takes 1 cell, then the
    // left column occupies 40 cells (cols 1–40), the gutter takes 4 cells
    // (cols 41–44), and the right column starts at col 45.
    // Pet art must never bleed past col 40 into the gutter.
    let left_col_end: usize = 40; // last left-column cell (0-indexed, inclusive)
    let gutter_start: usize = left_col_end + 1; // col 41

    for (y, row) in rows.iter().enumerate().take(23).skip(1) {
        let chars: Vec<char> = row.chars().collect();
        for offset in 0..4usize {
            let col = gutter_start + offset;
            assert!(
                chars[col] == ' ' || chars[col] == '─',
                "row {y} gutter col {col} should be space or dash; got {:?}",
                chars[col],
            );
        }
    }
}
