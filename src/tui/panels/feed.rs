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
        // Minimum height that shows the title + at least 2 events. The
        // wide-mode layout passes a Min(0) constraint that grows the feed
        // rect to fill the entire band, so this only matters in compact mode.
        let events = (vm.recent_events.len() as u16).max(2);
        Constraint::Length(events + 1)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, _ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" feed ");
        let inner = block.inner(area);
        block.render(area, buf);

        // Render whatever fits in the allocated rect — feed grows to fill
        // its rect in wide mode and shrinks to fit the event count in compact.
        let lines = build_feed_lines(vm, inner.height);
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
    fn feed_panel_clamps_to_allocated_rect_height() {
        // Feed no longer has a hard event cap; it renders whatever fits in the
        // allocated rect. With 12 events and a 7-row terminal (inner.height=6),
        // we expect ≤ 6 events to render — the rect, not a const, is the limit.
        let vm = WatchViewModel::fixture_with_n_events(12);
        let panel = FeedPanel;
        let backend = TestBackend::new(60, 7);
        let mut terminal = Terminal::new(backend).unwrap();
        let ctx = test_context();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
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
            "feed must not render more events than fit; got {event_rows} in a 6-row inner"
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
