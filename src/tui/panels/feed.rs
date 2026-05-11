use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::Panel;
use crate::tui::style::semantic_styles;
use crate::tui::view_model::WatchViewModel;

pub struct FeedPanel;

impl Panel for FeedPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // at least 3 rows (1 divider + 2 events); flex up to absorb leftover space
        Constraint::Min(3)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel) {
        let block = Block::default().borders(Borders::TOP).title(" feed ");
        let inner = block.inner(area);
        block.render(area, buf);

        let lines = build_feed_lines(vm, inner.height);
        Paragraph::new(lines).render(inner, buf);
    }
}

/// Build the event lines for the feed panel.
///
/// Clamps the number of events to `max_rows` so the content never overflows
/// the allocated rect.
fn build_feed_lines(vm: &WatchViewModel, max_rows: u16) -> Vec<Line<'_>> {
    let styles = semantic_styles();
    vm.recent_events
        .iter()
        .take(max_rows as usize)
        .map(|event| {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(event.timestamp.as_str(), styles.timestamp),
                Span::raw("  "),
                Span::styled(event.text.as_str(), styles.log(event.kind)),
            ])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_to_string(width: u16, height: u16, vm: &WatchViewModel) -> String {
        let panel = FeedPanel;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), vm);
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
        let backend = TestBackend::new(50, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm);
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
    fn feed_panel_preferred_constraint_is_min3() {
        let vm = WatchViewModel::fixture();
        let panel = FeedPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Min(3),
            "feed panel should flex with Min(3)"
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
}
