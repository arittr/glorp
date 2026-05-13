use crate::tui::component::{ComponentStyle, GradientToken, TextTone};
use crate::tui::panels::bars::{bar_spans, build_spark_line, format_tokens_short};
use crate::tui::panels::feed::build_feed_lines;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, tokenpet_palette, xp_color};
use crate::tui::view_model::WatchViewModel;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct Panel {
    title: &'static str,
    style: ComponentStyle,
}

impl Panel {
    pub const fn new(title: &'static str) -> Self {
        Self {
            title,
            style: ComponentStyle::new(),
        }
    }

    pub const fn style(mut self, style: ComponentStyle) -> Self {
        self.style = style;
        self
    }

    pub fn content_rect(&self, area: Rect) -> Rect {
        let inner = Block::default().borders(Borders::TOP).inner(area);
        let padding = self.style.insets();
        let x = inner.x.saturating_add(padding.left.min(inner.width));
        let y = inner.y.saturating_add(padding.top.min(inner.height));
        let width = inner
            .width
            .saturating_sub(padding.left.saturating_add(padding.right));
        let height = inner
            .height
            .saturating_sub(padding.top.saturating_add(padding.bottom));
        Rect::new(x, y, width, height)
    }

    pub fn render<F>(&self, area: Rect, buf: &mut Buffer, _ctx: &RenderContext, render_content: F)
    where
        F: FnOnce(Rect, &mut Buffer),
    {
        let title = format!(" {} ", self.title);
        let block = Block::default()
            .title(title)
            .borders(Borders::TOP)
            .border_style(self.style.border_style())
            .style(self.style.surface_style());
        let content = self.content_rect(area);
        block.render(area, buf);
        render_content(content, buf);
    }
}

pub struct TextRow<'a> {
    label: &'a str,
    value: String,
    tone: TextTone,
}

impl<'a> TextRow<'a> {
    pub fn new(label: &'a str, value: impl ToString) -> Self {
        Self {
            label,
            value: value.to_string(),
            tone: TextTone::Label,
        }
    }

    pub const fn tone(mut self, tone: TextTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn line(&self) -> Line<'_> {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{:<8}", self.label),
                ComponentStyle::new().text(self.tone).text_style(),
            ),
            Span::raw("   "),
            Span::styled(
                self.value.clone(),
                ComponentStyle::new().text(TextTone::Primary).text_style(),
            ),
        ])
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, _ctx: &RenderContext) {
        Paragraph::new(self.line()).render(area, buf);
    }
}

pub struct StatRow<'a> {
    label: &'a str,
    fraction: f64,
    color: Color,
}

impl<'a> StatRow<'a> {
    pub const fn new(label: &'a str, fraction: f64, color: Color) -> Self {
        Self {
            label,
            fraction,
            color,
        }
    }

    pub const fn fraction(mut self, fraction: f64) -> Self {
        self.fraction = fraction;
        self
    }

    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn spans(&self, ctx: &RenderContext) -> Vec<Span<'a>> {
        let styles = semantic_styles();
        bar_spans(
            self.label,
            self.fraction,
            self.color,
            ctx.color_capability,
            &styles,
        )
    }

    pub fn line(&self, ctx: &RenderContext) -> Line<'a> {
        Line::from(self.spans(ctx))
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext) {
        Paragraph::new(self.line(ctx)).render(area, buf);
    }
}

pub struct ProgressBar {
    fraction: f64,
    gradient: GradientToken,
    empty_tone: TextTone,
    rate_per_hour: Option<f64>,
}

impl ProgressBar {
    pub const fn new(fraction: f64) -> Self {
        Self {
            fraction,
            gradient: GradientToken::Good,
            empty_tone: TextTone::Subtle,
            rate_per_hour: None,
        }
    }

    pub const fn gradient(mut self, gradient: GradientToken) -> Self {
        self.gradient = gradient;
        self
    }

    pub const fn empty_tone(mut self, empty_tone: TextTone) -> Self {
        self.empty_tone = empty_tone;
        self
    }

    pub const fn rate_per_hour(mut self, rate_per_hour: f64) -> Self {
        self.rate_per_hour = Some(rate_per_hour);
        self
    }

    pub fn spans(&self, ctx: &RenderContext) -> Vec<Span<'static>> {
        let mut styles = semantic_styles();
        styles.empty_bar = ComponentStyle::new().text(self.empty_tone).text_style();
        let (label, color) = match self.gradient {
            GradientToken::Xp => ("xp", xp_color()),
            GradientToken::Good => ("good", tokenpet_palette().good.rgb),
        };
        let mut spans = bar_spans(label, self.fraction, color, ctx.color_capability, &styles);
        if let Some(rate_per_hour) = self.rate_per_hour.filter(|rate| *rate > 0.0) {
            spans.push(Span::raw("   "));
            spans.push(Span::styled("↑", styles.section_header));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                format!("{}/hr", format_tokens_short(rate_per_hour)),
                Style::default().fg(color),
            ));
        }
        spans
    }

    pub fn line(&self, ctx: &RenderContext) -> Line<'static> {
        Line::from(self.spans(ctx))
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext) {
        Paragraph::new(self.line(ctx)).render(area, buf);
    }
}

pub struct InlineSparkline<'a> {
    history: &'a [f64],
}

impl<'a> InlineSparkline<'a> {
    pub const fn new(history: &'a [f64]) -> Self {
        Self { history }
    }

    pub fn spans(&self) -> Vec<Span<'static>> {
        let styles = semantic_styles();
        build_spark_line(self.history, &styles)
    }

    pub fn line(&self) -> Line<'static> {
        Line::from(self.spans())
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, _ctx: &RenderContext) {
        Paragraph::new(self.line()).render(area, buf);
    }
}

pub struct FeedList<'a> {
    lines: Vec<Line<'a>>,
}

pub struct Lines<'a> {
    lines: Vec<Line<'a>>,
}

impl<'a> Lines<'a> {
    pub fn new(lines: impl IntoIterator<Item = Line<'a>>) -> Self {
        Self {
            lines: lines.into_iter().collect(),
        }
    }

    pub fn from_lines(lines: impl IntoIterator<Item = Line<'a>>) -> Self {
        Self::new(lines)
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, _ctx: &RenderContext) {
        let lines = self.lines.iter().take(area.height as usize).cloned();
        Paragraph::new(lines.collect::<Vec<_>>()).render(area, buf);
    }
}

impl<'a> FeedList<'a> {
    pub fn new(lines: impl IntoIterator<Item = Line<'a>>) -> Self {
        Self {
            lines: lines.into_iter().collect(),
        }
    }

    pub fn from_lines(lines: impl IntoIterator<Item = Line<'a>>) -> Self {
        Self::new(lines)
    }

    pub fn from_watch(vm: &'a WatchViewModel, max_rows: u16) -> Self {
        Self::new(build_feed_lines(vm, max_rows))
    }

    pub fn lines(&self) -> Vec<Line<'a>> {
        self.lines.clone()
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, _ctx: &RenderContext) {
        let lines = self.lines.iter().take(area.height as usize).cloned();
        Paragraph::new(lines.collect::<Vec<_>>()).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::{BorderTone, ComponentStyle, Insets, Surface, TextTone};
    use crate::tui::render_context::RenderContext;
    use crate::tui::style::{fed_color, ColorCapability};
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn panel_renders_title_border_padding_and_surface() {
        let ctx = RenderContext::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
        Panel::new("today")
            .style(
                ComponentStyle::new()
                    .surface(Surface::Elevated)
                    .border(BorderTone::Accent)
                    .padding(Insets::horizontal(1)),
            )
            .render(Rect::new(0, 0, 30, 5), &mut buf, &ctx, |content, buf| {
                TextRow::new("tokens", "18.4k")
                    .tone(TextTone::Primary)
                    .render(content, buf, &ctx);
            });
        let text = buffer_text(&buf);
        assert!(text.contains(" today "));
        assert!(text.contains("tokens"));
        assert!(text.contains("18.4k"));
    }

    #[test]
    fn stat_row_uses_existing_bar_shape() {
        let ctx = RenderContext::new(ColorCapability::Truecolor);
        let row = StatRow::new("fed", 0.5, fed_color());
        let text: String = row.spans(&ctx).iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("fed"));
        assert_eq!(text.chars().filter(|c| *c == '█').count(), 6);
        assert_eq!(text.chars().filter(|c| *c == '░').count(), 6);
    }

    #[test]
    fn progress_bar_xp_gradient_preserves_xp_label() {
        let ctx = RenderContext::new(ColorCapability::Truecolor);
        let bar = ProgressBar::new(0.25).gradient(GradientToken::Xp);
        let text: String = bar.spans(&ctx).iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("xp"));
        assert!(text.contains("25"));
    }

    #[test]
    fn progress_bar_can_append_rate_segment() {
        let ctx = RenderContext::new(ColorCapability::Truecolor);
        let bar = ProgressBar::new(0.25)
            .gradient(GradientToken::Xp)
            .rate_per_hour(109_000.0);
        let text: String = bar.spans(&ctx).iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("↑ 109.0k/hr"));
    }

    #[test]
    fn inline_sparkline_reuses_sparkline_glyphs() {
        let sparkline = InlineSparkline::new(&[0.0, 10.0, 20.0]);
        let text: String = sparkline
            .spans()
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains('·'));
        assert!(text.contains('█'));
    }

    #[test]
    fn feed_list_clamps_rendered_lines_to_area_height() {
        let ctx = RenderContext::new(ColorCapability::Truecolor);
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
        FeedList::new([
            Line::from("first event"),
            Line::from("second event"),
            Line::from("third event"),
        ])
        .render(Rect::new(0, 0, 12, 1), &mut buf, &ctx);
        let text = buffer_text(&buf);
        assert!(text.contains("first event"));
        assert!(!text.contains("second event"));
    }

    #[test]
    fn feed_list_from_watch_uses_existing_feed_formatting() {
        use crate::tui::style::{claude_color, codex_color};
        use crate::tui::view_model::WatchViewModel;

        let vm = WatchViewModel::fixture_with_events();
        let lines = FeedList::from_watch(&vm, 3).lines();
        let text: String = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();

        assert!(text.contains("13:42"));
        assert!(lines.iter().any(|line| line.spans.iter().any(|span| {
            span.content.contains("claude-code") && span.style.fg == Some(claude_color())
        })));

        let vm = WatchViewModel::fixture_with_n_events(2);
        let lines = FeedList::from_watch(&vm, 2).lines();
        assert!(lines.iter().any(|line| line.spans.iter().any(|span| {
            span.content.contains("codex") && span.style.fg == Some(codex_color())
        })));
    }

    fn buffer_text(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
