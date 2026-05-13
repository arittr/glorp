use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::component::{ComponentPanel, StatRow};
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
#[cfg(test)]
use crate::tui::style::ColorCapability;
use crate::tui::style::{energy_color, fed_color, happy_color};
use crate::tui::view_model::WatchViewModel;

pub struct VitalsPanel;

impl Panel for VitalsPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 3 bar rows (fed, happy, energy). xp moved to ProgressPanel.
        Constraint::Length(4)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let panel = ComponentPanel::new("vitals");
        panel.render(area, buf, ctx, |content, buf| {
            let rows = [
                StatRow::new("fed", vm.fed, fed_color()),
                StatRow::new("happy", vm.happiness, happy_color()),
                StatRow::new("energy", vm.energy, energy_color()),
            ];
            for (index, row) in rows.iter().enumerate().take(content.height as usize) {
                row.render(
                    Rect::new(content.x, content.y + index as u16, content.width, 1),
                    buf,
                    ctx,
                );
            }
        });
    }
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
        let vm = WatchViewModel::fixture();
        let buf = render_to_buffer(&vm);
        assert!(row_has_filled_color(&buf, 1, fed_color()));
        assert!(row_has_filled_color(&buf, 2, happy_color()));
        assert!(row_has_filled_color(&buf, 3, energy_color()));
    }

    #[test]
    fn vitals_panel_rows_render_a_gradient_in_truecolor() {
        let vm = WatchViewModel::fixture();
        let buf = render_to_buffer(&vm);
        for row in 1..=3 {
            let distinct = filled_colors_on_row(&buf, row);
            assert!(
                distinct.len() >= 2,
                "expected a gradient across filled cells, got {} distinct colors",
                distinct.len()
            );
        }
    }

    fn render_to_buffer(vm: &WatchViewModel) -> ratatui::buffer::Buffer {
        let panel = VitalsPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), vm, &ctx))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn row_has_filled_color(
        buf: &ratatui::buffer::Buffer,
        row: u16,
        color: ratatui::style::Color,
    ) -> bool {
        (0..buf.area.width).any(|x| {
            let cell = &buf[(x, row)];
            cell.symbol() == "█" && cell.style().fg == Some(color)
        })
    }

    fn filled_colors_on_row(
        buf: &ratatui::buffer::Buffer,
        row: u16,
    ) -> std::collections::HashSet<Option<ratatui::style::Color>> {
        (0..buf.area.width)
            .filter_map(|x| {
                let cell = &buf[(x, row)];
                (cell.symbol() == "█").then_some(cell.style().fg)
            })
            .collect()
    }
}
