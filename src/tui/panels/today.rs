use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::bars::build_spark_line;
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{claude_color, codex_color, semantic_styles, SemanticStyles};
use crate::tui::view_model::{SourceStatus, WatchViewModel};

/// Expected source surfaces and their display names.
const EXPECTED_SOURCES: &[(&str, &str)] = &[("claude-code", "claude"), ("codex", "codex")];

pub struct TodayPanel;

impl Panel for TodayPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 row for the TOP border/title + 5 data rows (tokens, claude, codex, last_10m, 7-day).
        Constraint::Length(6)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, _ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" today ");
        let inner = block.inner(area);
        block.render(area, buf);

        let styles = semantic_styles();
        let lines = build_today_lines(vm, &styles);
        Paragraph::new(lines).render(inner, buf);
    }
}

pub(crate) fn build_today_lines<'a>(
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::with_capacity(5);

    // Row 1: total tokens today
    lines.push(Line::from(today_spans(
        "tokens",
        &format_tokens_full(vm.today_effective_tokens),
        None,
        None,
        styles,
    )));

    // Rows 2–3: per-source breakdown with percentage share
    let total = vm.today_effective_tokens.max(0.0);
    for (surface, display) in EXPECTED_SOURCES {
        let value_opt = vm
            .source_breakdown
            .iter()
            .find(|s| s.name == *surface)
            .map(|s| s.effective_tokens);
        let (value_str, share) = match value_opt {
            Some(v) => {
                let pct = if total > 0.0 {
                    (v / total) * 100.0
                } else {
                    0.0
                };
                (
                    format_tokens_full(v),
                    Some(format!("{}%", pct.round() as u32)),
                )
            }
            None => ("—".to_string(), Some("—".to_string())),
        };
        // Determine health status for this source's ⚠ marker.
        let health_status = vm
            .source_health
            .iter()
            .find(|h| h.name == *surface)
            .map(|h| h.status);
        let blocked = matches!(
            health_status,
            Some(SourceStatus::Blocked) | Some(SourceStatus::Diagnostic)
        );
        // Resolve the label color by provider role.
        let label_color = match *surface {
            "claude-code" => Some(claude_color()),
            "codex" => Some(codex_color()),
            _ => None,
        };
        lines.push(Line::from(today_spans(
            display,
            &value_str,
            share,
            Some((label_color, blocked)),
            styles,
        )));
    }

    // Row 4: current 10-minute window
    let bucket_str = format_signed_tokens_short(vm.current_bucket_effective_tokens);
    lines.push(Line::from(today_spans(
        "last 10m",
        &bucket_str,
        Some("this 10m".to_string()),
        None,
        styles,
    )));

    // Row 5: 7-day inline sparkline footer
    let spark_spans = build_spark_line(&vm.recent_daily_effective_tokens, styles);
    let mut footer: Vec<Span<'a>> = vec![Span::raw("  ")];
    footer.extend(spark_spans);
    footer.push(Span::raw("          "));
    footer.push(Span::styled("← 7-day", styles.section_header));
    lines.push(Line::from(footer));

    lines
}

/// Duplicated from `layout::today_row` — T7 will deduplicate when the old path is deleted.
///
/// Fixed-column layout so values and annotations stay aligned across rows
/// even when token magnitudes differ by orders of magnitude.
///   2 sp + label(8) + gutter(3) + value(right-aligned, 13) + 4 sp + annotation
///
/// `source_meta` carries `(label_color_override, blocked)` for source rows:
/// - `label_color_override`: replaces the default label style foreground.
/// - `blocked`: when true, renders `⚠` in the 3-cell gutter; otherwise pads 3 spaces.
fn today_spans<'a>(
    label: &'a str,
    value: &str,
    annotation: Option<String>,
    source_meta: Option<(Option<ratatui::style::Color>, bool)>,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    const VALUE_WIDTH: usize = 13;
    let value_owned = format!("{value:>VALUE_WIDTH$}");
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw("  "));

    // Label: optionally colored for source rows.
    if let Some((Some(color), _)) = source_meta {
        spans.push(Span::styled(
            format!("{label:<8}"),
            ratatui::style::Style::default().fg(color),
        ));
    } else {
        spans.push(Span::styled(format!("{label:<8}"), styles.label));
    }

    // 3-cell gutter: either ⚠ + 2 spaces (blocked) or 3 spaces (healthy / non-source).
    if matches!(source_meta, Some((_, true))) {
        spans.push(Span::styled("⚠", styles.event_rail_diagnostic));
        spans.push(Span::raw("  "));
    } else {
        spans.push(Span::raw("   "));
    }

    spans.push(Span::styled(value_owned, styles.primary_text));
    if let Some(ann) = annotation {
        spans.push(Span::raw("    "));
        spans.push(Span::styled(ann, styles.label));
    }
    spans
}

/// Duplicated from `layout::format_tokens_full` — T7 will deduplicate when the old path is deleted.
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

/// Duplicated from `layout::format_signed_tokens_short` — T7 will deduplicate when the old path is deleted.
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
        let styles = semantic_styles();
        let lines = build_today_lines(&vm, &styles);
        let has_color = |needle: &str, color: ratatui::style::Color| {
            lines.iter().any(|l| {
                l.spans
                    .iter()
                    .any(|s| s.content.trim() == needle && s.style.fg == Some(color))
            })
        };
        assert!(has_color("claude", claude_color()));
        assert!(has_color("codex", codex_color()));
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
}
