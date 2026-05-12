use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::bars::{bar_spans_solid, format_tokens_short};
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, xp_color, SemanticStyles};
use crate::tui::view_model::{ProgressView, WatchViewModel};

pub struct ProgressPanel;

impl Panel for ProgressPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 2 content rows (stage line + xp bar).
        Constraint::Length(3)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, _ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" progress ");
        let inner = block.inner(area);
        block.render(area, buf);

        let styles = semantic_styles();
        let lines = build_progress_lines(&vm.progress, &styles);
        Paragraph::new(lines).render(inner, buf);
    }
}

fn build_progress_lines<'a>(
    progress: &'a ProgressView,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    if progress.is_max_stage {
        return vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled(progress.stage_label.clone(), styles.primary_text),
                Span::raw("  "),
                Span::styled("✦ max evolved", styles.section_header),
            ]),
            Line::from(Span::raw("")),
        ];
    }
    let stage_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(progress.stage_label.clone(), styles.primary_text),
        Span::raw(" "),
        Span::styled("➜", styles.section_header),
        Span::raw(" "),
        Span::styled(progress.next_stage_label.clone(), styles.primary_text),
    ]);
    let mut xp_spans = bar_spans_solid("xp", progress.fraction as f64, xp_color(), styles);
    if progress.rate_per_hour > 0.0 {
        xp_spans.push(Span::raw("   "));
        xp_spans.push(Span::styled("↑", styles.section_header));
        xp_spans.push(Span::raw(" "));
        xp_spans.push(Span::styled(
            format!("{}/hr", format_tokens_short(progress.rate_per_hour)),
            Style::default().fg(xp_color()),
        ));
    }
    vec![stage_line, Line::from(xp_spans)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use crate::tui::style::ColorCapability;

    fn ctx() -> RenderContext {
        RenderContext::new(ColorCapability::Truecolor)
    }

    fn render(vm: &WatchViewModel) -> String {
        let panel = ProgressPanel;
        let backend = TestBackend::new(60, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| panel.render(f.area(), f.buffer_mut(), vm, &ctx())).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    #[test]
    fn progress_panel_renders_stage_and_next_stage_label() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.stage_label = "fuzz".to_string();
        vm.progress.next_stage_label = "archfuzz".to_string();
        vm.progress.is_max_stage = false;
        let s = render(&vm);
        assert!(s.contains("progress"), "section title");
        assert!(s.contains("fuzz"), "stage label");
        assert!(s.contains("archfuzz"), "next stage label");
        assert!(s.contains("➜"), "stage arrow");
        assert!(s.contains("xp"), "xp bar label");
    }

    #[test]
    fn progress_panel_at_s6_renders_max_evolved() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.is_max_stage = true;
        vm.progress.stage_label = "mythic-fuzz".to_string();
        let s = render(&vm);
        assert!(s.contains("mythic-fuzz"));
        assert!(s.contains("max evolved"));
        assert!(!s.contains("➜"), "no arrow at max stage");
    }

    #[test]
    fn progress_panel_idle_hides_rate_segment() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.rate_per_hour = 0.0;
        let s = render(&vm);
        assert!(!s.contains("↑"));
        assert!(!s.contains("/hr"));
    }

    #[test]
    fn progress_panel_preferred_constraint_is_three() {
        let vm = WatchViewModel::fixture();
        let panel = ProgressPanel;
        assert_eq!(panel.preferred_constraint(&vm), Constraint::Length(3));
    }
}
