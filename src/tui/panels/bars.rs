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

/// Render a 7-day token history as a row of height-quantized block glyphs.
/// Ports SparkPanel's quantization so the visual is byte-identical when this
/// is rendered inside TodayPanel's footer (Task 18).
pub fn build_spark_line<'a>(
    history: &[f64],
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    const GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = history.iter().copied().fold(0.0_f64, f64::max);
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(history.len() * 2);
    for (i, &v) in history.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        if v <= 0.0 || max <= 0.0 {
            spans.push(Span::styled("·".to_string(), styles.sparkline_past));
        } else {
            let frac = (v / max).clamp(0.0, 1.0);
            let idx = ((frac * (GLYPHS.len() - 1) as f64).round() as usize).min(GLYPHS.len() - 1);
            let glyph = GLYPHS[idx];
            let style = if i == history.len() - 1 {
                styles.sparkline_today
            } else {
                styles.sparkline_past
            };
            spans.push(Span::styled(glyph.to_string(), style));
        }
    }
    spans
}

/// Format a token count with `k` or `M` suffix and one decimal place.
/// Values below 1 000 are rendered as whole numbers.
pub fn format_tokens_short(n: f64) -> String {
    if n >= 1_000_000.0 {
        format!("{:.1}M", n / 1_000_000.0)
    } else if n >= 1_000.0 {
        format!("{:.1}k", n / 1_000.0)
    } else {
        format!("{n:.0}")
    }
}

/// Format a token count as a comma-separated integer string (e.g. `"16,700"`).
pub fn format_tokens_full(n: f64) -> String {
    let n = n.round() as i64;
    if n.abs() >= 1_000 {
        let mut s = String::new();
        let neg = n < 0;
        let mut abs = n.unsigned_abs() as i64;
        let mut groups: Vec<String> = Vec::new();
        while abs >= 1000 {
            groups.push(format!("{:03}", abs % 1000));
            abs /= 1000;
        }
        groups.push(abs.to_string());
        groups.reverse();
        if neg {
            s.push('-');
        }
        s.push_str(&groups.join(","));
        s
    } else {
        n.to_string()
    }
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

    #[test]
    fn build_spark_line_seven_days_uses_block_heights() {
        let styles = semantic_styles();
        let history = vec![0.0, 0.0, 0.0, 1_000.0, 5_000.0, 10_000.0, 20_000.0];
        let spans = build_spark_line(&history, &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        // Zero days render as the spark dot glyph; non-zero days render as block-height glyphs.
        // The exact dot glyph is whatever the existing SparkPanel used — read spark.rs to confirm.
        let dot_count = text.chars().filter(|c| *c == '·' || *c == '.').count();
        assert_eq!(dot_count, 3, "three zero days must render as dots");
        assert!(text.contains('█'), "max day should hit highest block glyph");
    }

    #[test]
    fn build_spark_line_all_zero_renders_seven_dots() {
        let styles = semantic_styles();
        let spans = build_spark_line(&vec![0.0; 7], &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let dot_count = text.chars().filter(|c| *c == '·' || *c == '.').count();
        assert_eq!(dot_count, 7);
    }

    #[test]
    fn format_tokens_short_rounds_to_k_with_one_decimal() {
        assert_eq!(format_tokens_short(0.0), "0");
        assert_eq!(format_tokens_short(950.0), "950");
        assert_eq!(format_tokens_short(1_500.0), "1.5k");
        assert_eq!(format_tokens_short(16_700.0), "16.7k");
        assert_eq!(format_tokens_short(109_842.0), "109.8k");
        assert_eq!(format_tokens_short(1_234_567.0), "1.2M");
    }

    #[test]
    fn format_tokens_full_uses_thousands_separators() {
        assert_eq!(format_tokens_full(0.0), "0");
        assert_eq!(format_tokens_full(16_700.0), "16,700");
        assert_eq!(format_tokens_full(1_234_567.0), "1,234,567");
    }
}
