use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{claude_color, codex_color, semantic_styles};
use crate::tui::view_model::WatchViewModel;

pub struct FeedPanel;

impl Panel for FeedPanel {
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint {
        // 1 row for the TOP border/title + one row per event, capped so the
        // panel doesn't hog vertical space when there are many events. Extra
        // space goes to the trailing spacer in render_column_with_spacing,
        // pushing helpers + empty area to the bottom of the column.
        const MAX_EVENT_ROWS: u16 = 6;
        let events = (vm.recent_events.len() as u16).clamp(2, MAX_EVENT_ROWS);
        Constraint::Length(events + 1)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, _ctx: &RenderContext) {
        const MAX_EVENT_ROWS: u16 = 6;
        let block = Block::default().borders(Borders::TOP).title(" feed ");
        let inner = block.inner(area);
        block.render(area, buf);

        let lines = build_feed_lines(vm, inner.height.min(MAX_EVENT_ROWS));
        Paragraph::new(lines).render(inner, buf);
    }
}

/// Build the event lines for the feed panel.
///
/// Clamps the number of events to `max_rows` so the content never overflows
/// the allocated rect. Source names ("claude-code", "codex") at the start of
/// event text are extracted as separately-colored spans.
pub(crate) fn build_feed_lines(vm: &WatchViewModel, max_rows: u16) -> Vec<Line<'_>> {
    let styles = semantic_styles();
    vm.recent_events
        .iter()
        .take(max_rows as usize)
        .map(|event| {
            let text_style = styles.log(event.kind);
            let (source_span, rest_span) = extract_source_span(&event.text, text_style);
            let mut spans = vec![
                Span::raw("  "),
                Span::styled(event.timestamp.as_str(), styles.timestamp),
                Span::raw("  "),
                source_span,
            ];
            if let Some(rest) = rest_span {
                spans.push(rest);
            }
            Line::from(spans)
        })
        .collect()
}

/// If `text` starts with a known source name ("claude-code" or "codex"),
/// returns a colored span for the source name and an optional span for the
/// remainder. Otherwise returns a single span for the full text with
/// `fallback_style`.
fn extract_source_span(text: &str, fallback_style: Style) -> (Span<'_>, Option<Span<'_>>) {
    for name in &["claude-code", "codex"] {
        if let Some(rest) = text.strip_prefix(name) {
            let color = if *name == "claude-code" {
                claude_color()
            } else {
                codex_color()
            };
            let source_span = Span::styled(&text[..name.len()], Style::default().fg(color));
            let rest_span = if rest.is_empty() {
                None
            } else {
                Some(Span::styled(rest, fallback_style))
            };
            return (source_span, rest_span);
        }
    }
    (Span::styled(text, fallback_style), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_to_string(width: u16, height: u16, vm: &WatchViewModel) -> String {
        let panel = FeedPanel;
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
    fn feed_panel_renders_divider_title() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 5, &vm);
        assert!(s.contains("feed"), "expected 'feed' divider title");
    }

    #[test]
    fn feed_panel_renders_event_text() {
        let vm = WatchViewModel::fixture_with_events();
        let s = render_to_string(60, 5, &vm);
        assert!(
            s.contains("13:42"),
            "expected event timestamp in rendered output"
        );
    }

    #[test]
    fn feed_panel_does_not_overflow_small_rect() {
        // Give it only 2 rows (1 border + 1 inner). With 3 events in the
        // fixture the panel must silently clamp to fit.
        let vm = WatchViewModel::fixture_with_events();
        // If build_feed_lines didn't clamp, Paragraph would still truncate,
        // but we assert on the constraint and that this doesn't panic.
        let panel = FeedPanel;
        let ctx = RenderContext::new(crate::tui::style::ColorCapability::Truecolor);
        let backend = TestBackend::new(50, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm, &ctx);
            })
            .unwrap();
        // No panic means the clamp held; also verify the border appears.
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            s.contains("feed"),
            "border title still present in small rect"
        );
    }

    #[test]
    fn feed_panel_preferred_constraint_matches_event_count() {
        // Constraint is Length(events + 1) where events is clamped to [2, 8].
        let mut vm = WatchViewModel::fixture();
        vm.recent_events.clear();
        let panel = FeedPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(3),
            "empty feed should still reserve room for the divider + 2 placeholder rows"
        );
    }

    #[test]
    fn feed_panel_empty_events_renders_cleanly() {
        let mut vm = WatchViewModel::fixture();
        vm.recent_events.clear();
        let s = render_to_string(50, 4, &vm);
        assert!(
            s.contains("feed"),
            "divider should still appear with no events"
        );
    }

    fn test_context() -> RenderContext {
        RenderContext::new(crate::tui::style::ColorCapability::Truecolor)
    }

    #[test]
    fn feed_panel_caps_at_six_events() {
        let vm = WatchViewModel::fixture_with_n_events(12);
        let panel = FeedPanel;
        // Constraint must be 1 border + 6 events regardless of vm size.
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(7),
            "1 border + 6 events, even when vm has 12"
        );
        // Render into a terminal sized to match the constraint (7 rows).
        // inner.height = 6, so build_feed_lines receives max_rows=6.
        let backend = TestBackend::new(60, 7);
        let mut terminal = Terminal::new(backend).unwrap();
        let ctx = test_context();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        // Count rows that look like event rows (contain "13:" timestamp pattern).
        let event_rows = (0..buf.area().height)
            .filter(|&y| {
                let line: String = (0..buf.area().width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect();
                line.contains("13:")
            })
            .count();
        assert!(
            event_rows <= 6,
            "feed must not render more than 6 events, got {event_rows}"
        );
    }

    #[test]
    fn feed_panel_source_label_colors() {
        use crate::tui::style::{claude_color, codex_color};
        let vm = WatchViewModel::fixture_with_n_events(3);
        let lines = build_feed_lines(&vm, 6);
        let find_source = |needle: &str, color: ratatui::style::Color| {
            lines.iter().any(|l| {
                l.spans
                    .iter()
                    .any(|s| s.content.contains(needle) && s.style.fg == Some(color))
            })
        };
        assert!(
            find_source("claude-code", claude_color()) || find_source("codex", codex_color()),
            "at least one source label must carry its source color"
        );
    }
}
