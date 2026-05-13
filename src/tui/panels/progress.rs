use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::component::{
    BorderTone, ComponentPanel, ComponentStyle, GradientToken, Insets, ProgressBar, Surface,
    TextRow, TextTone,
};
use crate::tui::panels::LegacyPanel;
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

pub struct ProgressPanel;

impl LegacyPanel for ProgressPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 border row + 1 xp bar row. The next stage stays a surprise — no
        // "current ➜ next" hint, just the bar and current pace.
        Constraint::Length(2)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let panel = ComponentPanel::new("progress").style(
            ComponentStyle::new()
                .surface(Surface::Empty)
                .border(BorderTone::None)
                .padding(Insets::all(0)),
        );
        panel.render(area, buf, ctx, |content, buf| {
            if vm.progress.is_max_stage {
                TextRow::new("stage", "max evolved")
                    .tone(TextTone::Accent)
                    .render(content, buf, ctx);
            } else {
                ProgressBar::new(vm.progress.fraction as f64)
                    .gradient(GradientToken::Xp)
                    .empty_tone(TextTone::Subtle)
                    .rate_per_hour(vm.progress.rate_per_hour)
                    .render(content, buf, ctx);
            }
        });
    }
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
        let panel = ProgressPanel;
        let backend = TestBackend::new(60, 4);
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
    fn progress_panel_renders_title_and_xp_bar_without_stage_arrow() {
        // The next stage is meant to be a surprise — no "current ➜ next"
        // line. Just the section title and the xp bar.
        let mut vm = WatchViewModel::fixture();
        vm.progress.stage_label = "fuzz".to_string();
        vm.progress.next_stage_label = "archfuzz".to_string();
        vm.progress.is_max_stage = false;
        let s = render(&vm);
        assert!(s.contains("progress"), "section title");
        assert!(s.contains("xp"), "xp bar label");
        assert!(!s.contains("➜"), "stage transition must not be revealed");
        assert!(
            !s.contains("archfuzz"),
            "next-stage label must not be revealed"
        );
    }

    #[test]
    fn progress_panel_at_s6_renders_max_evolved() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.is_max_stage = true;
        vm.progress.stage_label = "mythic-fuzz".to_string();
        let s = render(&vm);
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
    fn progress_panel_preferred_constraint_is_two() {
        let vm = WatchViewModel::fixture();
        let panel = ProgressPanel;
        assert_eq!(panel.preferred_constraint(&vm), Constraint::Length(2));
    }
}
