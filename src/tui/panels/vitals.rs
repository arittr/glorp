use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::bars::bar_spans;
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{
    energy_color, fed_color, happy_color, semantic_styles, ColorCapability, SemanticStyles,
};
use crate::tui::view_model::WatchViewModel;

pub struct VitalsPanel;

impl Panel for VitalsPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 3 bar rows (fed, happy, energy). xp moved to ProgressPanel.
        Constraint::Length(4)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" vitals ");
        let inner = block.inner(area);
        block.render(area, buf);

        let styles = semantic_styles();
        let lines = build_vitals_lines(vm, inner.width, ctx.color_capability, &styles);
        Paragraph::new(lines).render(inner, buf);
    }
}

pub(crate) fn build_vitals_lines<'a>(
    vm: &'a WatchViewModel,
    _width: u16,
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    vec![
        Line::from(bar_spans("fed", vm.fed, fed_color(), capability, styles)),
        Line::from(bar_spans(
            "happy",
            vm.happiness,
            happy_color(),
            capability,
            styles,
        )),
        Line::from(bar_spans(
            "energy",
            vm.energy,
            energy_color(),
            capability,
            styles,
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Constraint;
    use ratatui::Terminal;

    fn test_context() -> RenderContext {
        RenderContext::new(ColorCapability::Truecolor)
    }

    #[test]
    fn vitals_panel_renders_into_area() {
        let vm = WatchViewModel::fixture();
        let panel = VitalsPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(s.contains("vitals"), "expected vitals divider title");
    }

    #[test]
    fn vitals_panel_renders_three_labels_no_xp() {
        let vm = WatchViewModel::fixture();
        let panel = VitalsPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 8);
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
        assert!(s.contains("fed"));
        assert!(s.contains("happy"));
        assert!(s.contains("energy"));
        assert!(!s.contains("xp"), "xp moved to ProgressPanel");
    }

    #[test]
    fn vitals_panel_preferred_constraint_is_four() {
        let vm = WatchViewModel::fixture();
        let panel = VitalsPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(4),
            "1 border + 3 bar rows (xp dropped)"
        );
    }

    #[test]
    fn vitals_panel_rows_use_per_stat_colors() {
        use crate::tui::style::{energy_color, fed_color, happy_color, semantic_styles};
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = build_vitals_lines(&vm, 40, ColorCapability::Truecolor, &styles);
        assert_eq!(lines.len(), 3);
        // The stat color is the gradient's mid stop, so it must appear among
        // the filled cells of each row.
        let row_has_color = |line: &Line, color| {
            line.spans
                .iter()
                .any(|s| s.content.contains('█') && s.style.fg == Some(color))
        };
        assert!(row_has_color(&lines[0], fed_color()));
        assert!(row_has_color(&lines[1], happy_color()));
        assert!(row_has_color(&lines[2], energy_color()));
    }

    #[test]
    fn vitals_panel_rows_render_a_gradient_in_truecolor() {
        use crate::tui::style::semantic_styles;
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = build_vitals_lines(&vm, 40, ColorCapability::Truecolor, &styles);
        for line in &lines {
            let distinct: std::collections::HashSet<_> = line
                .spans
                .iter()
                .filter(|s| s.content.contains('█'))
                .map(|s| s.style.fg)
                .collect();
            assert!(
                distinct.len() >= 2,
                "expected a gradient across filled cells, got {} distinct colors",
                distinct.len()
            );
        }
    }
}
