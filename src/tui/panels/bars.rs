use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::tui::style::SemanticStyles;

pub const BAR_CELLS: usize = 12;

/// Render a single-color bar row: `  <label:<6> <bar> <value>`.
/// Used by VitalsPanel rows and ProgressPanel's xp bar. The same color paints
/// the label, the filled cells, and the value. Empty cells use `empty_bar`.
pub fn bar_spans_solid<'a>(
    label: &'a str,
    fill_fraction: f64,
    color: Color,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    let clamped = fill_fraction.clamp(0.0, 1.0);
    let n_filled = ((clamped * BAR_CELLS as f64).round() as usize).min(BAR_CELLS);
    let n_empty = BAR_CELLS - n_filled;
    let value_pct = (clamped * 100.0).round() as u32;
    let stat_style = Style::default().fg(color);

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(BAR_CELLS + 6);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{label:<6}"), stat_style));
    spans.push(Span::raw(" "));
    if n_filled > 0 {
        spans.push(Span::styled("█".repeat(n_filled), stat_style));
    }
    if n_empty > 0 {
        spans.push(Span::styled("░".repeat(n_empty), styles.empty_bar));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{value_pct}"), stat_style));
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::{fed_color, semantic_styles};

    #[test]
    fn bar_spans_solid_zero_fill_renders_twelve_empty_cells() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 0.0, fed_color(), &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '░').count(), 12);
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 0);
    }

    #[test]
    fn bar_spans_solid_full_fill_renders_twelve_solid_cells() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 1.0, fed_color(), &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 12);
    }

    #[test]
    fn bar_spans_solid_label_and_value_share_stat_color() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 0.5, fed_color(), &styles);
        let label_style = spans[1].style;
        let value_style = spans.last().unwrap().style;
        assert_eq!(label_style.fg, Some(fed_color()));
        assert_eq!(value_style.fg, Some(fed_color()));
    }

    #[test]
    fn bar_spans_solid_filled_cells_share_stat_color() {
        let styles = semantic_styles();
        let spans = bar_spans_solid("fed", 0.5, fed_color(), &styles);
        let filled_span = spans.iter().find(|s| s.content.contains('█')).unwrap();
        assert_eq!(filled_span.style.fg, Some(fed_color()));
    }
}
