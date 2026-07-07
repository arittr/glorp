use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::component::{ComponentPanel, InlineSparkline, MetricRow};
use crate::tui::panels::LegacyPanel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{source_color, tokenpet_palette};
use crate::tui::view_model::{RateDirection, SourceStatus, WatchViewModel};

/// Number of individual source rows the fixed-height panel can show before
/// collapsing the rest into an "other" row.
const MAX_VISIBLE_SOURCE_ROWS: usize = 2;

pub struct TodayPanel;

impl LegacyPanel for TodayPanel {
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint {
        // Dynamic height: 1 row for the TOP border/title + content rows.
        // Content rows are: tokens, up to MAX_VISIBLE_SOURCE_ROWS source rows,
        // an optional "other" overflow row, two rate momentum rows, and the
        // 7-day sparkline.
        // When there are more than MAX_VISIBLE_SOURCE_ROWS sources, the overflow
        // row appears and we need one extra row to avoid clipping the footer.
        let source_rows = vm.source_breakdown.len().min(MAX_VISIBLE_SOURCE_ROWS);
        let overflow_rows = usize::from(vm.source_breakdown.len() > MAX_VISIBLE_SOURCE_ROWS);
        let content_rows = 1 + source_rows + overflow_rows + 2 + 1;
        Constraint::Length((content_rows as u16).saturating_add(1))
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let panel = ComponentPanel::new("today");
        panel.render(area, buf, ctx, |content, buf| {
            render_today_rows(content, buf, vm, ctx);
        });
    }
}

fn render_today_rows(area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
    if area.height > 0 {
        MetricRow::new("tokens", format_tokens_full(vm.today_effective_tokens)).render(
            row_rect(area, 0),
            buf,
            ctx,
        );
    }

    let total = vm.today_effective_tokens.max(0.0);
    let mut sources: Vec<_> = vm.source_breakdown.iter().collect();
    sources.sort_by(|a, b| {
        b.effective_tokens
            .partial_cmp(&a.effective_tokens)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut next_row = 1;
    for source in sources.iter().take(MAX_VISIBLE_SOURCE_ROWS) {
        if area.height <= next_row {
            break;
        }
        source_row(vm, &source.name, &source.display_name, total).render(
            row_rect(area, next_row),
            buf,
            ctx,
        );
        next_row += 1;
    }

    let other_tokens: f64 = sources
        .iter()
        .skip(MAX_VISIBLE_SOURCE_ROWS)
        .map(|s| s.effective_tokens)
        .sum();
    if other_tokens > 0.0 && area.height > next_row {
        other_row(other_tokens, total).render(row_rect(area, next_row), buf, ctx);
        next_row += 1;
    }

    let pulse_tokens = crate::format::format_tokens(vm.rate_momentum.pulse.current_tokens);
    let hour_tokens = crate::format::format_tokens(vm.rate_momentum.hour.current_tokens);
    let rate_token_width = pulse_tokens
        .chars()
        .count()
        .max(hour_tokens.chars().count());
    let rate_suffix_width = "/10m".chars().count().max("/hr".chars().count());

    if area.height > next_row {
        rate_row(
            "rate",
            vm.rate_momentum.pulse.direction,
            &pulse_tokens,
            "/10m",
            rate_token_width,
            rate_suffix_width,
        )
        .render(row_rect(area, next_row), buf, ctx);
        next_row += 1;
    }

    if area.height > next_row {
        rate_row(
            "",
            vm.rate_momentum.hour.direction,
            &hour_tokens,
            "/hr",
            rate_token_width,
            rate_suffix_width,
        )
        .render(row_rect(area, next_row), buf, ctx);
        next_row += 1;
    }

    if area.height > next_row {
        InlineSparkline::new(&vm.recent_daily_effective_tokens)
            .leading_width(2)
            .annotation_gap(9)
            .annotation("← 7-day")
            .render(row_rect(area, next_row), buf, ctx);
    }
}

fn source_row(vm: &WatchViewModel, surface: &str, display: &str, total: f64) -> MetricRow<'static> {
    let value_opt = vm
        .source_breakdown
        .iter()
        .find(|s| s.name == surface)
        .map(|s| s.effective_tokens);
    let (value, share) = match value_opt {
        Some(tokens) => {
            let pct = if total > 0.0 {
                (tokens / total) * 100.0
            } else {
                0.0
            };
            (
                format_tokens_full(tokens),
                format!("{}%", pct.round() as u32),
            )
        }
        None => ("—".to_string(), "—".to_string()),
    };
    let status = vm
        .source_health
        .iter()
        .find(|health| health.name == surface)
        .map(|health| health.status);
    let diagnostic = matches!(
        status,
        Some(SourceStatus::Blocked) | Some(SourceStatus::Diagnostic)
    );
    MetricRow::new(compact_label(display, 8), value)
        .annotation(share)
        .label_color(source_color(surface))
        .diagnostic_marker(diagnostic)
}

fn other_row(tokens: f64, total: f64) -> MetricRow<'static> {
    let share = if total > 0.0 {
        format!("{}%", ((tokens / total) * 100.0).round() as u32)
    } else {
        "—".to_string()
    };
    MetricRow::new("other", format_tokens_full(tokens))
        .annotation(share)
        .label_color(tokenpet_palette().dim.rgb)
}

fn rate_row(
    label: &'static str,
    direction: RateDirection,
    tokens: &str,
    suffix: &'static str,
    token_width: usize,
    suffix_width: usize,
) -> MetricRow<'static> {
    MetricRow::new(
        label,
        format!(
            "{} {:>token_width$}{:<suffix_width$}",
            direction_glyph(direction),
            tokens,
            suffix,
            token_width = token_width,
            suffix_width = suffix_width
        ),
    )
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

fn compact_label(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max_len.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn row_rect(area: Rect, index: u16) -> Rect {
    Rect::new(area.x, area.y + index, area.width, 1)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::view_model::{SourceHealthView, SourceUsageView};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_to_string(width: u16, height: u16, vm: &WatchViewModel) -> String {
        let panel = TodayPanel;
        let ctx = RenderContext::new(crate::tui::style::ColorCapability::Truecolor);
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), vm, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        buf.content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    #[test]
    fn today_panel_renders_divider_title() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 6, &vm);
        assert!(s.contains("today"), "expected 'today' divider title");
    }

    #[test]
    fn today_panel_renders_tokens_row() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 6, &vm);
        assert!(s.contains("tokens"), "expected 'tokens' label");
        // fixture has 18,420 today tokens
        assert!(s.contains("18,420"), "expected formatted token count");
    }

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

    #[test]
    fn today_panel_renders_source_labels() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 6, &vm);
        assert!(s.contains("claude"), "expected 'claude' source label");
        assert!(s.contains("codex"), "expected 'codex' source label");
    }

    #[test]
    fn today_panel_replaces_last_10m_with_rate_rows() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(70, 8, &vm);
        assert!(!s.contains("last 10m"), "old bucket row should not render");
        assert!(s.contains("2.3k/10m"), "expected pulse rate row");
        assert!(s.contains("109.0k/hr"), "expected hour rate row");
    }

    #[test]
    fn today_panel_does_not_render_absent_sources() {
        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown.retain(|s| s.name != "codex");
        let s = render_to_string(50, 6, &vm);
        assert!(
            !s.contains("codex"),
            "dynamic rows must not show sources with no today activity: {s}"
        );
    }

    #[test]
    fn today_panel_renders_top_sources_plus_other() {
        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown = vec![
            SourceUsageView {
                name: "claude-code".into(),
                display_name: "claude".into(),
                effective_tokens: 12_000.0,
            },
            SourceUsageView {
                name: "codex".into(),
                display_name: "codex".into(),
                effective_tokens: 5_000.0,
            },
            SourceUsageView {
                name: "gemini".into(),
                display_name: "gemini".into(),
                effective_tokens: 3_000.0,
            },
            SourceUsageView {
                name: "opencode".into(),
                display_name: "opencode".into(),
                effective_tokens: 1_500.0,
            },
        ];
        let s = render_to_string(70, 8, &vm);
        assert!(s.contains("claude"), "expected claude in today panel: {s}");
        assert!(s.contains("codex"), "expected codex in today panel: {s}");
        assert!(
            s.contains("other"),
            "overflow sources must collapse into 'other': {s}"
        );
        assert!(
            !s.contains("gemini"),
            "gemini should be hidden behind 'other': {s}"
        );
    }

    #[test]
    fn today_panel_long_source_name_compacts_safely() {
        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown = vec![SourceUsageView {
            name: "verylongsourcesuffix".into(),
            display_name: "verylongsourcesuffix".into(),
            effective_tokens: 10_000.0,
        }];
        vm.source_health = vec![SourceHealthView {
            name: "verylongsourcesuffix".into(),
            display_name: "verylongsourcesuffix".into(),
            status: SourceStatus::Ready,
            snapshot_state: crate::usage::snapshot::SnapshotState::Current,
            snapshot_reason: None,
            today_effective_tokens: 10_000.0,
            bucket_effective_tokens: 1_000.0,
            diagnostic_code: None,
            diagnostic_message: None,
        }];
        // Should not panic or exceed geometry.
        let s = render_to_string(40, 8, &vm);
        assert!(
            s.contains("verylon"),
            "long source name should render a compacted prefix: {s}"
        );
    }

    #[test]
    fn today_panel_colors_unknown_sources_deterministically() {
        use crate::tui::style::source_color;
        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown = vec![SourceUsageView {
            name: "gemini".into(),
            display_name: "gemini".into(),
            effective_tokens: 5_000.0,
        }];
        vm.source_health = vec![SourceHealthView {
            name: "gemini".into(),
            display_name: "gemini".into(),
            status: SourceStatus::Ready,
            snapshot_state: crate::usage::snapshot::SnapshotState::Current,
            snapshot_reason: None,
            today_effective_tokens: 5_000.0,
            bucket_effective_tokens: 500.0,
            diagnostic_code: None,
            diagnostic_message: None,
        }];
        let panel = TodayPanel;
        let ctx = test_context();
        let backend = TestBackend::new(70, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        assert!(has_colored_word(buf, "gemini", source_color("gemini")));
    }

    #[test]
    fn format_tokens_full_comma_separates_thousands() {
        assert_eq!(format_tokens_full(18_420.0), "18,420");
        assert_eq!(format_tokens_full(1_000_000.0), "1,000,000");
        assert_eq!(format_tokens_full(999.0), "999");
    }

    fn test_context() -> RenderContext {
        RenderContext::new(crate::tui::style::ColorCapability::Truecolor)
    }

    #[test]
    fn today_panel_renders_seven_day_inline_footer() {
        let vm = WatchViewModel::fixture();
        let panel = TodayPanel;
        let ctx = test_context();
        let backend = TestBackend::new(70, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(s.contains("7-day"), "footer row must carry '7-day' label");
    }

    #[test]
    fn today_panel_rate_suffixes_align_at_slash() {
        // Compare terminal cells, not byte indexes. The rate rows contain
        // arrows and the footer contains spark glyphs, so joined string byte
        // offsets do not match the columns the user sees.
        let vm = WatchViewModel::fixture();
        let panel = TodayPanel;
        let ctx = test_context();
        let backend = TestBackend::new(70, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let row_text =
            |y: u16| -> String { (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect() };
        let find_col = |y: u16, needle: &str| -> Option<u16> {
            (0..buf.area.width).find(|&x| buf[(x, y)].symbol() == needle)
        };
        let height = buf.area.height;
        let pulse_y = (0..height)
            .find(|&y| row_text(y).contains("/10m"))
            .expect("expected to find the '/10m' rate row");
        let hour_y = (0..height)
            .find(|&y| row_text(y).contains("/hr"))
            .expect("expected to find the '/hr' rate row");
        let pulse_slash_col = find_col(pulse_y, "/").unwrap();
        let hour_slash_col = find_col(hour_y, "/").unwrap();
        assert_eq!(
            pulse_slash_col, hour_slash_col,
            "rate rows must align at slash: /10m col {pulse_slash_col}, /hr col {hour_slash_col}"
        );
    }

    #[test]
    fn today_panel_renders_blocked_marker_on_unhealthy_source() {
        use crate::tui::view_model::{SourceHealthView, SourceStatus};
        let mut vm = WatchViewModel::fixture();
        vm.source_health = vec![
            SourceHealthView {
                name: "codex".to_string(),
                display_name: "codex".to_string(),
                status: SourceStatus::Blocked,
                snapshot_state: crate::usage::snapshot::SnapshotState::Missing,
                snapshot_reason: None,
                today_effective_tokens: 0.0,
                bucket_effective_tokens: 0.0,
                diagnostic_code: None,
                diagnostic_message: None,
            },
            SourceHealthView {
                name: "claude-code".to_string(),
                display_name: "claude".to_string(),
                status: SourceStatus::Ready,
                snapshot_state: crate::usage::snapshot::SnapshotState::Current,
                snapshot_reason: None,
                today_effective_tokens: 12_900.0,
                bucket_effective_tokens: 1_300.0,
                diagnostic_code: None,
                diagnostic_message: None,
            },
        ];
        let panel = TodayPanel;
        let ctx = test_context();
        let backend = TestBackend::new(70, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(s.contains("⚠"), "blocked source must render the marker");
    }

    #[test]
    fn today_panel_source_labels_use_source_colors() {
        use crate::tui::style::{claude_color, codex_color};
        let vm = WatchViewModel::fixture();
        let panel = TodayPanel;
        let ctx = test_context();
        let backend = TestBackend::new(70, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        assert!(has_colored_word(buf, "claude", claude_color()));
        assert!(has_colored_word(buf, "codex", codex_color()));
    }

    #[test]
    fn today_panel_preferred_constraint_is_dynamic() {
        let panel = TodayPanel;

        let vm = WatchViewModel::fixture();
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(7),
            "<=2 sources fit in 1 border + tokens + sources + two rate rows + 7-day footer"
        );

        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown.push(SourceUsageView {
            name: "gemini".into(),
            display_name: "gemini".into(),
            effective_tokens: 3_000.0,
        });
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(8),
            "3+ sources add an overflow 'other' row and need 8 rows total"
        );
    }

    #[test]
    fn today_panel_wide_layout_renders_overflow_row_without_clipping() {
        use crate::tui::component::{layout_watch, render_watch_layout, WatchComponentId};

        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown = vec![
            SourceUsageView {
                name: "claude-code".into(),
                display_name: "claude".into(),
                effective_tokens: 12_000.0,
            },
            SourceUsageView {
                name: "codex".into(),
                display_name: "codex".into(),
                effective_tokens: 5_000.0,
            },
            SourceUsageView {
                name: "gemini".into(),
                display_name: "gemini".into(),
                effective_tokens: 3_000.0,
            },
        ];

        let layout = layout_watch(ratatui::layout::Rect::new(0, 0, 120, 32), &vm);
        let today = layout
            .node(WatchComponentId::Today.path())
            .expect("today node");
        assert_eq!(
            today.bounds.height, 8,
            "today panel should grow to 8 rows when overflow row is present"
        );

        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        let ctx = test_context();
        terminal
            .draw(|f| {
                let layout = layout_watch(f.area(), &vm);
                render_watch_layout(&layout, f.buffer_mut(), &vm, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("claude"),
            "expected claude source row: {text}"
        );
        assert!(text.contains("codex"), "expected codex source row: {text}");
        assert!(
            text.contains("other"),
            "overflow sources must collapse into 'other': {text}"
        );
        assert!(
            !text.contains("gemini"),
            "gemini should be hidden behind 'other': {text}"
        );
        assert!(
            !text.contains("last 10m"),
            "old bucket row should not render: {text}"
        );
        assert!(text.contains("/10m"), "expected pulse rate row: {text}");
        assert!(text.contains("/hr"), "expected hour rate row: {text}");
        assert!(text.contains("7-day"), "expected 7-day footer row: {text}");
    }

    fn has_colored_word(
        buf: &ratatui::buffer::Buffer,
        word: &str,
        color: ratatui::style::Color,
    ) -> bool {
        let chars: Vec<String> = word.chars().map(|ch| ch.to_string()).collect();
        (0..buf.area.height).any(|y| {
            (0..=buf.area.width.saturating_sub(chars.len() as u16)).any(|x| {
                chars.iter().enumerate().all(|(offset, expected)| {
                    let cell = &buf[(x + offset as u16, y)];
                    cell.symbol() == expected && cell.style().fg == Some(color)
                })
            })
        })
    }
}
