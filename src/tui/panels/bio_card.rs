use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::component::{ComponentPanel, TextRow};
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

pub struct BioCardPanel;

impl Panel for BioCardPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 2 content rows (hatched, age).
        Constraint::Length(3)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let panel = ComponentPanel::new("bio");
        panel.render(area, buf, ctx, |content, buf| {
            if content.height > 0 {
                TextRow::new("hatched", vm.bio.hatched_label.clone())
                    .gap_width(2)
                    .render(row_rect(content, 0), buf, ctx);
            }
            if content.height > 1 {
                TextRow::new("age", vm.bio.age_label.clone())
                    .gap_width(2)
                    .render(row_rect(content, 1), buf, ctx);
            }
        });
    }
}

fn row_rect(area: Rect, index: u16) -> Rect {
    Rect::new(area.x, area.y + index, area.width, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn ctx() -> RenderContext {
        RenderContext::new(ColorCapability::Truecolor)
    }

    fn render(vm: &WatchViewModel) -> String {
        let panel = BioCardPanel;
        let backend = TestBackend::new(40, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), vm, &ctx()))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    #[test]
    fn bio_panel_renders_title_and_two_rows() {
        let vm = WatchViewModel::fixture();
        let s = render(&vm);
        assert!(s.contains("bio"), "title");
        assert!(s.contains("hatched"), "hatched label");
        assert!(s.contains("age"), "age label");
    }

    #[test]
    fn bio_panel_renders_sub_day_age() {
        let mut vm = WatchViewModel::fixture();
        vm.bio.age_label = "0d 4h".to_string();
        let s = render(&vm);
        assert!(s.contains("0d 4h"));
    }

    #[test]
    fn bio_panel_preferred_constraint_is_three() {
        let vm = WatchViewModel::fixture();
        let panel = BioCardPanel;
        assert_eq!(panel.preferred_constraint(&vm), Constraint::Length(3));
    }
}
