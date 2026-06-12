// tests/activity_identity_ui.rs
use glorp::tui::panels::TodayPanel;
use glorp::tui::render_context::RenderContext;
use glorp::tui::style::{claude_color, codex_color, source_color};
use glorp::tui::view_model::{SourceHealthView, SourceStatus, SourceUsageView, WatchViewModel};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_today_to_string(vm: &WatchViewModel, width: u16, height: u16) -> String {
    let panel = TodayPanel;
    let ctx = RenderContext::new(glorp::tui::style::ColorCapability::Truecolor);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            glorp::tui::panels::LegacyPanel::render(&panel, f.area(), f.buffer_mut(), vm, &ctx)
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol().to_string())
        .collect()
}

#[test]
fn today_panel_renders_top_n_sources_plus_other() {
    let mut vm = WatchViewModel::fixture();
    vm.today_effective_tokens = 30_000.0;
    vm.source_breakdown = vec![
        SourceUsageView {
            name: "claude-code".into(),
            display_name: "claude".into(),
            effective_tokens: 10_000.0,
        },
        SourceUsageView {
            name: "codex".into(),
            display_name: "codex".into(),
            effective_tokens: 8_000.0,
        },
        SourceUsageView {
            name: "gemini".into(),
            display_name: "gemini".into(),
            effective_tokens: 6_000.0,
        },
        SourceUsageView {
            name: "opencode".into(),
            display_name: "opencode".into(),
            effective_tokens: 4_000.0,
        },
        SourceUsageView {
            name: "kimi".into(),
            display_name: "kimi".into(),
            effective_tokens: 2_000.0,
        },
    ];
    vm.source_health = vm
        .source_breakdown
        .iter()
        .map(|s| SourceHealthView {
            name: s.name.clone(),
            display_name: s.display_name.clone(),
            status: SourceStatus::Ready,
            today_effective_tokens: s.effective_tokens,
            bucket_effective_tokens: 0.0,
            diagnostic_code: None,
            diagnostic_message: None,
        })
        .collect();

    let s = render_today_to_string(&vm, 70, 8);
    assert!(s.contains("claude"), "expected claude row");
    assert!(s.contains("codex"), "expected codex row");
    assert!(
        s.contains("other"),
        "expected an 'other' row for sources beyond top-N"
    );
    assert!(
        !s.contains("gemini"),
        "gemini should be hidden behind 'other'"
    );
    assert!(s.contains("30,000"), "expected total tokens row");
}

#[test]
fn today_panel_truncates_long_source_names_safely() {
    let mut vm = WatchViewModel::fixture();
    vm.source_breakdown = vec![SourceUsageView {
        name: "very-long-agent-name".into(),
        display_name: "very-long-agent-name".into(),
        effective_tokens: 15_000.0,
    }];
    vm.source_health = vec![SourceHealthView {
        name: "very-long-agent-name".into(),
        display_name: "very-long-agent-name".into(),
        status: SourceStatus::Ready,
        today_effective_tokens: 15_000.0,
        bucket_effective_tokens: 0.0,
        diagnostic_code: None,
        diagnostic_message: None,
    }];

    let s = render_today_to_string(&vm, 40, 6);
    assert!(
        s.contains("very-lo"),
        "long source name should compact, not panic: {s}"
    );
}

#[test]
fn source_palette_is_deterministic_for_known_and_unknown_sources() {
    let a = source_color("gemini");
    let b = source_color("gemini");
    let c = source_color("opencode");
    let d = source_color("opencode");
    assert_eq!(a, b, "same source must map to the same color");
    assert_eq!(c, d, "same source must map to the same color");
    assert_ne!(a, c, "different sources should usually differ");

    assert_eq!(source_color("claude-code"), claude_color());
    assert_eq!(source_color("codex"), codex_color());
}
