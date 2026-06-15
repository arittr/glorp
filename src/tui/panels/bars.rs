use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::tui::style::{gradient_ramp, ramp_index, ColorCapability, SemanticStyles};

pub const BAR_CELLS: usize = 12;

/// Render a bar row: `  <label:<8>   <bar>  <value>`.
/// Label and value carry the per-stat color. On truecolor, filled cells fade
/// dim→bright through a ramp derived from `color`; on flat terminals, every
/// filled cell uses `color` directly. Empty cells use `empty_bar`.
pub fn bar_spans<'a>(
    label: &'a str,
    fill_fraction: f64,
    color: Color,
    capability: ColorCapability,
    styles: &SemanticStyles,
) -> Vec<Span<'a>> {
    let clamped = fill_fraction.clamp(0.0, 1.0);
    let n_filled = ((clamped * BAR_CELLS as f64).round() as usize).min(BAR_CELLS);
    let n_empty = BAR_CELLS - n_filled;
    let value_pct = (clamped * 100.0).round() as u32;
    let value_label = if clamped > 0.0 && value_pct == 0 {
        "<1".to_string()
    } else {
        value_pct.to_string()
    };
    let stat_style = Style::default().fg(color);

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(BAR_CELLS + 6);
    spans.push(Span::raw("  "));
    // Label padded to 8 cells + a 3-space gap so even short labels (xp = 2
    // chars) keep visible breathing room between the text and the bar.
    spans.push(Span::styled(format!("{label:<8}"), stat_style));
    spans.push(Span::raw("   "));
    match capability {
        ColorCapability::Truecolor => {
            let ramp = gradient_ramp(color);
            for i in 0..n_filled {
                let idx = ramp_index(i, n_filled);
                spans.push(Span::styled("█", Style::default().fg(ramp.stops[idx])));
            }
        }
        ColorCapability::Flat => {
            if n_filled > 0 {
                spans.push(Span::styled("█".repeat(n_filled), stat_style));
            }
        }
    }
    if n_empty > 0 {
        spans.push(Span::styled("░".repeat(n_empty), styles.empty_bar));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(value_label, stat_style));
    spans
}

/// Render a 7-day token history as a row of height-quantized block glyphs.
/// Ports SparkPanel's quantization so the visual is byte-identical when this
/// is rendered inside TodayPanel's footer (Task 18).
pub fn build_spark_line(history: &[f64], styles: &SemanticStyles) -> Vec<Span<'static>> {
    const GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = history.iter().copied().fold(0.0_f64, f64::max);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(history.len() * 2);
    for (i, &v) in history.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
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

    use crate::tui::style::ColorCapability;

    #[test]
    fn bar_spans_zero_fill_renders_twelve_empty_cells() {
        let styles = semantic_styles();
        let spans = bar_spans("fed", 0.0, fed_color(), ColorCapability::Truecolor, &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '░').count(), 12);
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 0);
    }

    #[test]
    fn bar_spans_nonzero_sub_percent_progress_does_not_render_as_zero() {
        let styles = semantic_styles();
        let spans = bar_spans(
            "xp",
            0.0016,
            fed_color(),
            ColorCapability::Truecolor,
            &styles,
        );
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("<1"),
            "positive sub-percent progress should not read as zero: {text:?}"
        );
        assert!(
            !text.ends_with("  0"),
            "positive sub-percent progress should not render the zero value: {text:?}"
        );
    }

    #[test]
    fn bar_spans_full_fill_renders_twelve_solid_cells() {
        let styles = semantic_styles();
        let spans = bar_spans("fed", 1.0, fed_color(), ColorCapability::Truecolor, &styles);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 12);
    }

    #[test]
    fn bar_spans_label_and_value_share_stat_color() {
        let styles = semantic_styles();
        let spans = bar_spans("fed", 0.5, fed_color(), ColorCapability::Truecolor, &styles);
        let label_style = spans[1].style;
        let value_style = spans.last().unwrap().style;
        assert_eq!(label_style.fg, Some(fed_color()));
        assert_eq!(value_style.fg, Some(fed_color()));
    }

    #[test]
    fn bar_spans_truecolor_filled_cells_span_a_gradient() {
        let styles = semantic_styles();
        let spans = bar_spans("fed", 1.0, fed_color(), ColorCapability::Truecolor, &styles);
        let filled_colors: Vec<_> = spans
            .iter()
            .filter(|s| s.content.contains('█'))
            .map(|s| s.style.fg)
            .collect();
        assert_eq!(filled_colors.len(), 12, "12 filled cells");
        let distinct: std::collections::HashSet<_> = filled_colors.iter().collect();
        assert!(
            distinct.len() >= 4,
            "expected gradient across filled cells, got {} distinct colors",
            distinct.len()
        );
        // The mid stop must equal the stat color so per-stat identity is preserved.
        assert!(
            filled_colors.iter().any(|c| *c == Some(fed_color())),
            "stat color must appear at the mid stop of the ramp"
        );
    }

    #[test]
    fn bar_spans_flat_filled_cells_share_stat_color() {
        let styles = semantic_styles();
        let spans = bar_spans("fed", 0.5, fed_color(), ColorCapability::Flat, &styles);
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
        let spans = build_spark_line(&[0.0; 7], &styles);
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
