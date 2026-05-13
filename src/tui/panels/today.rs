use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::component::{ComponentPanel, InlineSparkline, MetricRow};
use crate::tui::panels::LegacyPanel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{claude_color, codex_color};
use crate::tui::view_model::{SourceStatus, WatchViewModel};

/// Expected source surfaces and their display names.
const EXPECTED_SOURCES: &[(&str, &str)] = &[("claude-code", "claude"), ("codex", "codex")];

pub struct TodayPanel;

impl LegacyPanel for TodayPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 row for the TOP border/title + 5 data rows (tokens, claude, codex, last_10m, 7-day).
        Constraint::Length(6)
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
    for (index, (surface, display)) in EXPECTED_SOURCES.iter().enumerate() {
        let row = 1 + index as u16;
        if area.height <= row {
            continue;
        }
        source_row(vm, surface, display, total).render(row_rect(area, row), buf, ctx);
    }

    if area.height > 3 {
        MetricRow::new(
            "last 10m",
            format_signed_tokens_short(vm.current_bucket_effective_tokens),
        )
        .annotation("this 10m")
        .render(row_rect(area, 3), buf, ctx);
    }

    if area.height > 4 {
        InlineSparkline::new(&vm.recent_daily_effective_tokens)
            .leading_width(2)
            .annotation_gap(9)
            .annotation("← 7-day")
            .render(row_rect(area, 4), buf, ctx);
    }
}

fn source_row<'a>(
    vm: &WatchViewModel,
    surface: &str,
    display: &'a str,
    total: f64,
) -> MetricRow<'a> {
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
    let color = match surface {
        "claude-code" => claude_color(),
        "codex" => codex_color(),
        _ => claude_color(),
    };
    MetricRow::new(display, value)
        .annotation(share)
        .label_color(color)
        .diagnostic_marker(diagnostic)
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

fn format_signed_tokens_short(n: f64) -> String {
    let abs = n.abs();
    let unit = if abs >= 1_000_000.0 {
        format!("{:.1}m", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}k", abs / 1_000.0)
    } else {
        format!("{}", abs.round() as i64)
    };
    // Avoid rendering "-0" when the rounded absolute value is zero.
    if n < 0.0 && unit != "0" {
        format!("-{unit}")
    } else {
        format!("+{unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn today_panel_renders_source_labels() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 6, &vm);
        assert!(s.contains("claude"), "expected 'claude' source label");
        assert!(s.contains("codex"), "expected 'codex' source label");
    }

    #[test]
    fn today_panel_renders_last_10m_row() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 6, &vm);
        assert!(s.contains("last 10m"), "expected 'last 10m' label");
        // fixture has 2,300 bucket tokens → "+2.3k"
        assert!(s.contains("+2.3k"), "expected formatted bucket tokens");
    }

    #[test]
    fn today_panel_renders_dash_for_absent_source() {
        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown.retain(|s| s.name != "codex");
        let s = render_to_string(50, 6, &vm);
        assert!(
            s.contains("codex"),
            "expected 'codex' label even when absent"
        );
        assert!(s.contains('—'), "expected dash for absent source");
    }

    #[test]
    fn format_tokens_full_comma_separates_thousands() {
        assert_eq!(format_tokens_full(18_420.0), "18,420");
        assert_eq!(format_tokens_full(1_000_000.0), "1,000,000");
        assert_eq!(format_tokens_full(999.0), "999");
    }

    #[test]
    fn format_signed_tokens_short_positive_uses_plus() {
        assert_eq!(format_signed_tokens_short(2_300.0), "+2.3k");
        assert_eq!(format_signed_tokens_short(500.0), "+500");
        assert_eq!(format_signed_tokens_short(1_500_000.0), "+1.5m");
    }

    #[test]
    fn format_signed_tokens_short_negative_uses_minus() {
        assert_eq!(format_signed_tokens_short(-2_300.0), "-2.3k");
    }

    #[test]
    fn format_signed_tokens_short_zero_renders_plus_zero() {
        assert_eq!(format_signed_tokens_short(0.0), "+0");
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
    fn today_panel_seven_day_arrow_aligns_with_annotation_column() {
        // The `←` legend tip must land at the same column as the `t` in
        // "this 10m" on the row above so the spark line visually anchors
        // to the data labels rather than drifting right.
        //
        // Note: we compare TERMINAL CELLS, not byte indexes — spark glyphs
        // (`·`, `▁`..`█`) are 3-byte UTF-8 per cell, so `str::find` on the
        // joined row would return a byte offset that disagrees with the
        // column the user sees.
        let vm = WatchViewModel::fixture();
        let panel = TodayPanel;
        let ctx = test_context();
        let backend = TestBackend::new(70, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let find_col = |y: u16, needle: &str| -> Option<u16> {
            (0..buf.area.width).find(|&x| buf[(x, y)].symbol() == needle)
        };
        let height = buf.area.height;
        let last_10m_y = (0..height)
            .find(|&y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .contains("this 10m")
            })
            .expect("expected to find the 'this 10m' row");
        let footer_y = (0..height)
            .find(|&y| find_col(y, "←").is_some())
            .expect("expected to find the '←' footer row");
        // The `t` of "this 10m" sits one cell after the column whose only
        // earlier `t` neighbor is also part of the same word; the simplest
        // reliable anchor is the column of the LITERAL `t` followed by `h`.
        let this_t_col = (0..buf.area.width - 1)
            .find(|&x| {
                buf[(x, last_10m_y)].symbol() == "t" && buf[(x + 1, last_10m_y)].symbol() == "h"
            })
            .expect("expected `th` in 'this 10m'");
        let arrow_col = find_col(footer_y, "←").unwrap();
        assert_eq!(
            this_t_col, arrow_col,
            "← (col {arrow_col}) must align with the 't' of 'this 10m' (col {this_t_col})"
        );
    }

    #[test]
    fn today_panel_renders_blocked_marker_on_unhealthy_source() {
        use crate::tui::view_model::{SourceHealthView, SourceStatus};
        let mut vm = WatchViewModel::fixture();
        vm.source_health = vec![
            SourceHealthView {
                name: "codex".to_string(),
                status: SourceStatus::Blocked,
                today_effective_tokens: 0.0,
                bucket_effective_tokens: 0.0,
                diagnostic_code: None,
                diagnostic_message: None,
            },
            SourceHealthView {
                name: "claude-code".to_string(),
                status: SourceStatus::Ready,
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
    fn today_panel_preferred_constraint_is_six() {
        let vm = WatchViewModel::fixture();
        let panel = TodayPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(6),
            "1 border + tokens + claude + codex + last_10m + 7-day footer"
        );
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
