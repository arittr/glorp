use crossterm::event::{KeyCode, KeyEventKind};
use glorp::error::Result;
use glorp::pet::render::{PaletteRoleName, StyledSegment};
use glorp::tui::app::{
    render_evolution_overlay_for_test, render_frame_for_test, render_hatch_overlay_for_test,
    render_help_overlay_for_test, run_single_watch_tick_for_test, WatchApp, WatchAppConfig,
    WatchTestHarness, WatchUsagePoller, WatchViewModel,
};
use glorp::tui::style::{semantic_styles, tokenpet_palette};
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

#[test]
fn wide_layout_has_tokenpet_chrome_panels_and_bars() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let p = tokenpet_palette();
    let text = buffer_text(buf);
    assert!(text.contains("glorp --"));
    assert!(text.contains("─ vitals"));
    assert!(text.contains("today"));
    assert!(text.contains("sources"));
    assert!(text.contains("●"));
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
    assert!(text.contains("╎"));
    assert!(text.contains("✦ ✧ ✦"));

    let stage_line = buffer_lines(terminal.backend().buffer())
        .into_iter()
        .find(|line| line.contains("/ o.o \\"))
        .unwrap();
    assert!(
        stage_line.find('/').unwrap_or_default() > 15,
        "pet art should be visually centered in its stage: {stage_line:?}"
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
    assert!(text.contains("name"));
    assert!(text.contains("species"));
    assert!(text.contains("stage"));
    assert!(text.contains("mood"));
    assert!(text.contains("today"));
    assert!(text.contains("bucket"));
    assert!(text.contains("sources"));
    assert!(text.contains("log"));
    assert!(text.contains("┄"));
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
    assert!(text.contains("▏"));
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
    assert!(has_cell(buf, "▏", p.good.rgb));
    assert!(has_cell(buf, "▏", p.accent.rgb));
    assert!(has_cell(buf, "▏", p.bad.rgb));
    assert!(has_cell(buf, ":", p.faint.rgb));
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
    assert!(text.contains("glorp"));
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
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(text.contains("blocked"));
    assert!(text.contains("install ccusage"));
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

    let backend = TestBackend::new(80, 20);
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
    vm.pet_render.stage = "s6".into();
    vm.stage = "s6".into();
    vm.xp_current = 113.0;
    vm.xp_target = 49.0;
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains("xp max"),
        "expected xp stats to render 'max' when current overshoots target, got:\n{text}"
    );
    assert!(
        !text.contains("113 / 49"),
        "should not render raw '113 / 49' once xp is capped at max"
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
fn format_tokens_uses_m_suffix_past_one_million() {
    let mut vm = WatchViewModel::fixture();
    vm.today_effective_tokens = 66_631_100.0;
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(
        text.contains("66.6M"),
        "values past one million should render with an M suffix, got:\n{text}"
    );
    assert!(
        !text.contains("66631.1k"),
        "values past one million should not render with stale k suffix"
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
        },
        Box::new(harness),
    );
    // Drop the app without ever running. Drop sends Shutdown and joins the
    // worker; if that path hangs the test will hang.
    drop(app);
}
