use crossterm::event::{KeyCode, KeyEventKind};
use glorp::tui::app::{
    render_evolution_overlay_for_test, render_frame_for_test, render_hatch_overlay_for_test,
    render_help_overlay_for_test, run_single_watch_tick_for_test, WatchApp, WatchAppConfig,
    WatchTestHarness, WatchViewModel,
};
use glorp::tui::style::tokenpet_palette;
use ratatui::{
    backend::TestBackend, buffer::Buffer, layout::Position, style::Color, Frame, Terminal,
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
    assert!(text.contains("helper"));
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
    assert!(text.contains("helper"));
    assert!(text.contains("┄"));
}

#[test]
fn wide_layout_keeps_pet_and_stats_top_stacked_like_tokenpet_mockup() {
    let backend = TestBackend::new(157, 34);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture_with_events()))
        .unwrap();
    let lines = buffer_lines(terminal.backend().buffer());
    let first_pet_line = lines
        .iter()
        .position(|line| line.contains("/\\_/\\"))
        .expect("pet art should render");
    let stats_line = lines
        .iter()
        .position(|line| line.contains("stats"))
        .expect("stats header should render");
    let last_divider_line = lines
        .iter()
        .rposition(|line| line.contains("╎"))
        .expect("column divider should render");

    assert!(
        first_pet_line < 12,
        "pet art should begin near the top of the left column, got row {first_pet_line}"
    );
    assert!(
        stats_line <= first_pet_line + 8,
        "stats should follow the pet stage without a large vertical gulf; pet row {first_pet_line}, stats row {stats_line}"
    );
    assert!(
        last_divider_line <= stats_line + 8,
        "column divider should end near the content instead of stretching to the footer; stats row {stats_line}, divider row {last_divider_line}"
    );
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
fn compact_boundary_is_exact_at_72_columns() {
    let mut at_72 = Terminal::new(TestBackend::new(72, 24)).unwrap();
    at_72
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let lines_72 = buffer_lines(at_72.backend().buffer());
    assert!(lines_72
        .iter()
        .any(|line| line.contains("vitals") && line.contains("today")));

    let mut at_71 = Terminal::new(TestBackend::new(71, 24)).unwrap();
    at_71
        .draw(|f| render_frame_for_test(f, &WatchViewModel::fixture()))
        .unwrap();
    let lines_71 = buffer_lines(at_71.backend().buffer());
    let vitals_line = lines_71
        .iter()
        .position(|line| line.contains("vitals"))
        .unwrap();
    let today_line = lines_71
        .iter()
        .position(|line| line.contains("today"))
        .unwrap();
    assert!(today_line > vitals_line);
}

#[test]
fn compact_layout_keeps_required_vitals_visible() {
    let backend = TestBackend::new(48, 18);
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
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(text.contains("blocked"));
    assert!(text.contains("missing ccusage helper"));
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
