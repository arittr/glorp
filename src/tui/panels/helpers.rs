use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::Panel;
use crate::tui::style::{semantic_styles, tokenpet_palette};
use crate::tui::view_model::{SourceStatus, WatchViewModel};

/// Expected source surfaces and their display names.
/// Duplicated from `layout.rs` — T7 will deduplicate when the old path is deleted.
const EXPECTED_SOURCES: &[(&str, &str)] = &[("claude-code", "claude"), ("codex", "codex")];

pub struct HelpersPanel;

impl Panel for HelpersPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 row for the TOP border/title + 1 status row
        Constraint::Length(2)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel) {
        let block = Block::default().borders(Borders::TOP).title(" helpers ");
        let inner = block.inner(area);
        block.render(area, buf);

        Paragraph::new(build_helpers_line(vm)).render(inner, buf);
    }
}

/// Build the single status line for the helpers panel.
///
/// Each expected source is shown with its display name and a glyph indicating
/// its current health: ✓ ready, ~ degraded, ✗ blocked, — unknown.
fn build_helpers_line(vm: &WatchViewModel) -> Line<'_> {
    let styles = semantic_styles();
    let p = tokenpet_palette();

    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::raw("  "));

    let mut first = true;
    for (surface, display) in EXPECTED_SOURCES {
        if !first {
            spans.push(Span::raw("     "));
        }
        first = false;

        let health = vm.source_health.iter().find(|s| s.name == *surface);
        let (glyph, glyph_style) = match health.map(|h| h.status) {
            Some(SourceStatus::Ready) => ('✓', Style::default().fg(p.good.rgb)),
            Some(SourceStatus::Diagnostic) => ('~', Style::default().fg(p.accent.rgb)),
            Some(SourceStatus::Blocked) => ('✗', Style::default().fg(p.bad.rgb)),
            None => ('—', Style::default().fg(p.dim.rgb)),
        };
        spans.push(Span::styled(display.to_string(), styles.label));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(glyph.to_string(), glyph_style));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_to_string(width: u16, height: u16, vm: &WatchViewModel) -> String {
        let panel = HelpersPanel;
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
    fn helpers_panel_renders_divider_title() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 3, &vm);
        assert!(s.contains("helpers"), "expected 'helpers' divider title");
    }

    #[test]
    fn helpers_panel_renders_check_when_ready() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 3, &vm);
        assert!(s.contains('✓'), "expected ✓ glyph for ready sources");
    }

    #[test]
    fn helpers_panel_renders_x_when_blocked() {
        let mut vm = WatchViewModel::fixture();
        for src in vm.source_health.iter_mut() {
            src.status = SourceStatus::Blocked;
        }
        let s = render_to_string(50, 3, &vm);
        assert!(s.contains('✗'), "expected ✗ glyph for blocked sources");
    }

    #[test]
    fn helpers_panel_renders_tilde_when_degraded() {
        let mut vm = WatchViewModel::fixture();
        for src in vm.source_health.iter_mut() {
            src.status = SourceStatus::Diagnostic;
        }
        let s = render_to_string(50, 3, &vm);
        assert!(s.contains('~'), "expected ~ glyph for diagnostic sources");
    }

    #[test]
    fn helpers_panel_renders_dash_for_unknown_source() {
        let mut vm = WatchViewModel::fixture();
        vm.source_health.clear();
        let s = render_to_string(50, 3, &vm);
        assert!(s.contains('—'), "expected — glyph for unknown sources");
    }

    #[test]
    fn helpers_panel_preferred_constraint_is_length2() {
        let vm = WatchViewModel::fixture();
        let panel = HelpersPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(2),
            "expected Length(2): 1 border row + 1 status row"
        );
    }

    #[test]
    fn helpers_panel_renders_source_names() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(50, 3, &vm);
        assert!(s.contains("claude"), "expected 'claude' label");
        assert!(s.contains("codex"), "expected 'codex' label");
    }
}
