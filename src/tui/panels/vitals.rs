use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::Panel;
use crate::tui::style::{
    bar_ramp_accent, bar_ramp_good, ramp_index, semantic_styles, BarRamp, ColorCapability,
    SemanticStyles,
};
use crate::tui::view_model::WatchViewModel;

pub struct VitalsPanel;

impl Panel for VitalsPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 row for the TOP border/title + 4 bar rows.
        Constraint::Length(5)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel) {
        let block = Block::default().borders(Borders::TOP).title(" vitals ");
        let inner = block.inner(area);
        block.render(area, buf);

        let capability = ColorCapability::detect();
        let styles = semantic_styles();
        let lines = build_vitals_lines(vm, inner.width, capability, &styles);
        Paragraph::new(lines).render(inner, buf);
    }
}

fn build_vitals_lines<'a>(
    vm: &'a WatchViewModel,
    _width: u16,
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let xp_fraction = if vm.xp_target <= 0.0 {
        0.0
    } else {
        (vm.xp_current / vm.xp_target).clamp(0.0, 1.0)
    };

    let good = bar_ramp_good();
    let accent = bar_ramp_accent();
    vec![
        Line::from(bar_spans("fed", vm.fed, good, capability, styles)),
        Line::from(bar_spans("happy", vm.happiness, accent, capability, styles)),
        Line::from(bar_spans("energy", vm.energy, good, capability, styles)),
        Line::from(bar_spans("xp", xp_fraction, accent, capability, styles)),
    ]
}

/// Duplicated from `layout::bar_row` — T7 will deduplicate when the old path
/// is deleted.
fn bar_spans<'a>(
    label: &'a str,
    fill_fraction: f64,
    ramp: BarRamp,
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    const BAR_CELLS: usize = 12;
    let clamped = fill_fraction.clamp(0.0, 1.0);
    let n_filled = (clamped * BAR_CELLS as f64).round() as usize;
    let n_filled = n_filled.min(BAR_CELLS);
    let n_empty = BAR_CELLS - n_filled;
    let value_pct = (clamped * 100.0).round() as u32;

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(BAR_CELLS + 6);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{label:<6}"), styles.label));
    spans.push(Span::raw(" "));
    for i in 0..n_filled {
        let style = match capability {
            ColorCapability::Truecolor => {
                let idx = ramp_index(i, n_filled);
                Style::default().fg(ramp.stops[idx])
            }
            ColorCapability::Flat => Style::default().fg(ramp.stops[2]),
        };
        spans.push(Span::styled("█", style));
    }
    if n_empty > 0 {
        spans.push(Span::styled("░".repeat(n_empty), styles.empty_bar));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{value_pct}"), styles.primary_text));
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn vitals_panel_renders_into_area() {
        let vm = WatchViewModel::fixture();
        let panel = VitalsPanel;
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm);
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
    fn vitals_panel_renders_all_four_labels() {
        let vm = WatchViewModel::fixture();
        let panel = VitalsPanel;
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(s.contains("fed"), "expected fed label");
        assert!(s.contains("happy"), "expected happy label");
        assert!(s.contains("energy"), "expected energy label");
        assert!(s.contains("xp"), "expected xp label");
    }

    #[test]
    fn vitals_panel_preferred_constraint_is_five() {
        let vm = WatchViewModel::fixture();
        let panel = VitalsPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(5),
            "expected Length(5): 1 border row + 4 bar rows"
        );
    }

    #[test]
    fn vitals_bar_spans_zero_fill_has_twelve_empty_cells() {
        let styles = semantic_styles();
        let spans = bar_spans("fed", 0.0, bar_ramp_good(), ColorCapability::Flat, &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '░').count(), 12);
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 0);
    }

    #[test]
    fn vitals_bar_spans_full_fill_has_twelve_solid_cells() {
        let styles = semantic_styles();
        let spans = bar_spans("fed", 1.0, bar_ramp_good(), ColorCapability::Flat, &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 12);
    }
}
