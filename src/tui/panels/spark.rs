use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::panels::Panel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{
    bar_ramp_good, ramp_index, semantic_styles, tokenpet_palette, ColorCapability,
};
use crate::tui::view_model::WatchViewModel;

pub struct SparkPanel;

impl Panel for SparkPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        // 1 row for the TOP border/title + 1 sparkline row.
        Constraint::Length(2)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        let block = Block::default().borders(Borders::TOP).title(" 7-day ");
        let inner = block.inner(area);
        block.render(area, buf);

        let styles = semantic_styles();
        let lines = build_spark_lines(
            &vm.recent_daily_effective_tokens,
            ctx.color_capability,
            &styles,
        );
        Paragraph::new(lines).render(inner, buf);
    }
}

/// Duplicated from `layout::render_sparkline_rows` — T7 will deduplicate when the old path
/// is deleted.
///
/// Renders the 7 most-recent daily token values as block-character bars spaced
/// across the inner area. Each bar is one of ▁▂▃▄▅▆▇█ scaled to the slice
/// maximum; zero-value days render as the placeholder glyph `·`.
///
/// If the history slice is shorter than 7 entries the leading positions are
/// filled with zero so the layout is always the same width.
fn build_spark_lines<'a>(
    history: &[f64],
    capability: ColorCapability,
    styles: &'a crate::tui::style::SemanticStyles,
) -> Vec<Line<'a>> {
    if history.is_empty() {
        return vec![Line::from(Span::styled(
            "       ·   ·   ·   ·   ·   ·   ·",
            styles.empty_bar,
        ))];
    }

    let glyphs: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut last_seven: Vec<f64> = history.iter().copied().rev().take(7).collect();
    last_seven.reverse();
    while last_seven.len() < 7 {
        last_seven.insert(0, 0.0);
    }
    let max = last_seven.iter().cloned().fold(0.0_f64, f64::max);

    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw("       "));
    for (i, value) in last_seven.iter().enumerate() {
        let glyph = if *value <= 0.0 || max <= 0.0 {
            '·'
        } else {
            let level = ((value / max) * (glyphs.len() as f64 - 1.0)).round() as usize;
            glyphs[level.min(glyphs.len() - 1)]
        };
        let style = if glyph == '·' {
            styles.empty_bar
        } else {
            match capability {
                ColorCapability::Truecolor => {
                    let idx = ramp_index(i, 7);
                    Style::default().fg(bar_ramp_good().stops[idx])
                }
                ColorCapability::Flat => Style::default().fg(tokenpet_palette().good.rgb),
            }
        };
        spans.push(Span::styled(glyph.to_string(), style));
        if i < 6 {
            spans.push(Span::raw("   "));
        }
    }

    vec![Line::from(spans)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_to_string(width: u16, height: u16, vm: &WatchViewModel) -> String {
        let panel = SparkPanel;
        let ctx = RenderContext::new(ColorCapability::Truecolor);
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), vm, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        buf.content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    #[test]
    fn spark_panel_renders_divider_title() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(40, 3, &vm);
        assert!(s.contains("7-day"), "expected '7-day' divider title");
    }

    #[test]
    fn spark_panel_renders_sparkline_content() {
        let vm = WatchViewModel::fixture();
        let s = render_to_string(40, 3, &vm);
        // The fixture has non-zero data so at least one block glyph must appear.
        let block_glyphs = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let has_block = s.chars().any(|c| block_glyphs.contains(&c));
        assert!(
            has_block,
            "expected at least one block glyph in sparkline area"
        );
    }

    #[test]
    fn spark_panel_preferred_constraint_is_two() {
        let vm = WatchViewModel::fixture();
        let panel = SparkPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Length(2),
            "expected Length(2): 1 border row + 1 sparkline row"
        );
    }

    #[test]
    fn spark_panel_empty_history_renders_placeholder_dots() {
        let mut vm = WatchViewModel::fixture();
        vm.recent_daily_effective_tokens = vec![];
        let s = render_to_string(40, 3, &vm);
        assert!(
            s.contains('·'),
            "expected placeholder dot for empty history"
        );
    }

    #[test]
    fn spark_panel_short_history_pads_with_zeroes() {
        let mut vm = WatchViewModel::fixture();
        vm.recent_daily_effective_tokens = vec![5_000.0, 10_000.0];
        let s = render_to_string(40, 3, &vm);
        // Two real values → at most two block glyphs; 5 leading zeros → dots.
        let dot_count = s.chars().filter(|&c| c == '·').count();
        assert!(
            dot_count >= 5,
            "expected at least 5 placeholder dots for 5 zero-padded slots, got {dot_count}"
        );
    }

    #[test]
    fn build_spark_lines_all_zeroes_render_dots() {
        let history = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let styles = semantic_styles();
        let lines = build_spark_lines(&history, ColorCapability::Flat, &styles);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        let dot_count = text.chars().filter(|&c| c == '·').count();
        assert_eq!(dot_count, 7, "all-zero history should render 7 dots");
    }

    #[test]
    fn build_spark_lines_fixture_data_renders_seven_glyphs() {
        let history = vec![
            1_000.0, 8_000.0, 4_000.0, 13_000.0, 9_500.0, 16_000.0, 18_420.0,
        ];
        let styles = semantic_styles();
        let lines = build_spark_lines(&history, ColorCapability::Flat, &styles);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        let block_glyphs = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let glyph_count = text.chars().filter(|c| block_glyphs.contains(c)).count();
        assert_eq!(
            glyph_count, 7,
            "fixture data should render exactly 7 block glyphs"
        );
    }
}
