use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span};

use crate::tui::component::{ComponentPanel, Lines};
use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, SemanticStyles};
use crate::tui::view_model::{BioView, WatchViewModel};

pub struct BioCardPanel;

impl Panel for BioCardPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 2 content rows (hatched, age).
        Constraint::Length(3)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let panel = ComponentPanel::new("bio");
        panel.render(area, buf, ctx, |content, buf| {
            let styles = semantic_styles();
            Lines::from_lines(build_bio_lines(&vm.bio, &styles)).render(content, buf, ctx);
        });
    }
}

fn build_bio_lines<'a>(bio: &'a BioView, styles: &'a SemanticStyles) -> Vec<Line<'a>> {
    vec![
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<8}", "hatched"), styles.label),
            Span::raw("  "),
            Span::styled(bio.hatched_label.clone(), styles.primary_text),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<8}", "age"), styles.label),
            Span::raw("  "),
            Span::styled(bio.age_label.clone(), styles.primary_text),
        ]),
    ]
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
