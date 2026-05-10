//! Lipgloss-style fixed-width string composition over ratatui spans.
//!
//! The watch TUI lays content out by character count, not by ratatui's
//! flex constraints. This module provides the primitives that make that
//! readable: every row is a `Vec<Span<'a>>` that knows its own visible
//! width and gets padded or truncated to fit a target column.
//!
//! The lipgloss vocabulary maps as follows:
//!
//! - `pad_row` / `pad_rows` → `Style::Width(n).Render(...)` for a single
//!   row or a block.
//! - `join_horizontal_top` → `lipgloss::JoinHorizontal(Top, a, b, ...)`,
//!   stacking blocks side-by-side and padding shorter blocks with blank
//!   rows at the bottom.
//! - `box_with_chrome` → `Style::Border(Rounded).Render(...)`, drawing
//!   the outer rounded frame around a body of pre-composed rows.
//! - `section_divider` → the `─ label ─...─` rule that fills out to a
//!   target width inside a column.
//!
//! We keep everything as `Span<'a>` so the data column can keep its
//! palette colors (good, accent, bad) cell-by-cell instead of being
//! flattened back through ANSI escapes.
//!
//! ## Width convention
//!
//! All widths in this module are *visible character widths*. We use
//! `chars().count()` rather than byte length so single-codepoint glyphs
//! (`█`, `─`, `┃`) count as one cell. The pet art and bar glyphs we
//! render all live in this regime; no double-width CJK or grapheme
//! clusters reach the composer.
//!
//! Truncation, when it has to happen, is char-by-char from the left
//! edge of each span until the visible width is reached.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// Visible character width of a row of spans.
pub fn row_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

/// Pad a row of spans to exactly `target_width` visible characters using
/// the provided style for any trailing spaces. If the row is already
/// wider it is truncated char-by-char from the left.
pub fn pad_row<'a>(spans: Vec<Span<'a>>, target_width: usize, pad_style: Style) -> Vec<Span<'a>> {
    let width = row_width(&spans);
    if width == target_width {
        return spans;
    }
    if width < target_width {
        let mut out = spans;
        out.push(Span::styled(" ".repeat(target_width - width), pad_style));
        return out;
    }
    truncate_row(spans, target_width)
}

fn truncate_row<'a>(spans: Vec<Span<'a>>, target_width: usize) -> Vec<Span<'a>> {
    let mut out: Vec<Span<'a>> = Vec::with_capacity(spans.len());
    let mut remaining = target_width;
    for span in spans {
        let span_len = span.content.chars().count();
        if span_len <= remaining {
            out.push(span);
            remaining -= span_len;
        } else {
            let kept: String = span.content.chars().take(remaining).collect();
            out.push(Span::styled(kept, span.style));
            break;
        }
    }
    out
}

/// Pad every row of a block to `target_width` visible characters. Rows
/// already at the target width pass through unchanged; shorter rows get
/// trailing spaces with `pad_style`; longer rows are truncated.
pub fn pad_rows<'a>(
    rows: Vec<Vec<Span<'a>>>,
    target_width: usize,
    pad_style: Style,
) -> Vec<Vec<Span<'a>>> {
    rows.into_iter()
        .map(|row| pad_row(row, target_width, pad_style))
        .collect()
}

/// Join two columns side-by-side, top-aligned. Each column's rows are
/// padded to `col_width(i)` before concatenation, and the shorter column
/// gets blank rows added at the bottom so both have the same height.
///
/// `pad_style` is used for both the trailing spaces inside each column
/// and the blank filler rows.
pub fn join_horizontal_top<'a>(
    columns: Vec<(usize, Vec<Vec<Span<'a>>>)>,
    pad_style: Style,
) -> Vec<Vec<Span<'a>>> {
    if columns.is_empty() {
        return Vec::new();
    }

    let height = columns
        .iter()
        .map(|(_, rows)| rows.len())
        .max()
        .unwrap_or(0);
    let padded: Vec<Vec<Vec<Span<'a>>>> = columns
        .into_iter()
        .map(|(width, rows)| {
            let mut padded_rows = pad_rows(rows, width, pad_style);
            while padded_rows.len() < height {
                padded_rows.push(vec![Span::styled(" ".repeat(width), pad_style)]);
            }
            padded_rows
        })
        .collect();

    (0..height)
        .map(|row_index| {
            let mut joined: Vec<Span<'a>> = Vec::new();
            for column in &padded {
                joined.extend(column[row_index].iter().cloned());
            }
            joined
        })
        .collect()
}

/// Frame a body of rows in a rounded box-drawing border with a styled
/// title row on top and a styled footer row on bottom.
///
/// Body rows are padded to `inner_width` cells; the resulting frame is
/// `inner_width + 2` cells wide.
///
/// `title_spans` and `footer_spans` are the rich part of each chrome
/// line. They get bracketed with `┏━ ` / ` ━━━...┓` and `┗━ ` /
/// ` ━━━...┛` respectively. The `━` filler picks up `frame_style` so the
/// whole frame renders in a single color.
pub struct ChromeStyle {
    pub frame: Style,
    pub body_pad: Style,
}

pub fn box_with_chrome<'a>(
    body: Vec<Vec<Span<'a>>>,
    inner_width: usize,
    title_spans: Vec<Span<'a>>,
    footer_spans: Vec<Span<'a>>,
    chrome: ChromeStyle,
) -> Vec<Line<'a>> {
    let outer_width = inner_width + 2;
    let mut out: Vec<Line<'a>> = Vec::with_capacity(body.len() + 2);
    out.push(Line::from(chrome_top_line(
        title_spans,
        outer_width,
        chrome.frame,
    )));
    for row in body {
        let padded = pad_row(row, inner_width, chrome.body_pad);
        let mut framed: Vec<Span<'a>> = Vec::with_capacity(padded.len() + 2);
        framed.push(Span::styled("┃", chrome.frame));
        framed.extend(padded);
        framed.push(Span::styled("┃", chrome.frame));
        out.push(Line::from(framed));
    }
    out.push(Line::from(chrome_bottom_line(
        footer_spans,
        outer_width,
        chrome.frame,
    )));
    out
}

fn chrome_top_line<'a>(
    title_spans: Vec<Span<'a>>,
    outer_width: usize,
    frame_style: Style,
) -> Vec<Span<'a>> {
    chrome_line("┏━ ", " ", "━", "┓", title_spans, outer_width, frame_style)
}

fn chrome_bottom_line<'a>(
    footer_spans: Vec<Span<'a>>,
    outer_width: usize,
    frame_style: Style,
) -> Vec<Span<'a>> {
    chrome_line("┗━ ", " ", "━", "┛", footer_spans, outer_width, frame_style)
}

fn chrome_line<'a>(
    open: &'static str,
    after_label: &'static str,
    fill: &'static str,
    close: &'static str,
    label: Vec<Span<'a>>,
    outer_width: usize,
    frame_style: Style,
) -> Vec<Span<'a>> {
    let label_visible = row_width(&label);
    let open_visible = open.chars().count();
    let after_visible = after_label.chars().count();
    let close_visible = close.chars().count();
    let fill_count =
        outer_width.saturating_sub(open_visible + label_visible + after_visible + close_visible);
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::styled(open, frame_style));
    spans.extend(label);
    spans.push(Span::styled(after_label, frame_style));
    spans.push(Span::styled(fill.repeat(fill_count), frame_style));
    spans.push(Span::styled(close, frame_style));
    spans
}

/// Render a `─ label ─...─` section divider that fills exactly
/// `target_width` visible cells. The label is wrapped in single spaces;
/// the leading and trailing dashes use `rule_style`, the label uses
/// `label_style`.
pub fn section_divider<'a>(
    label: &'a str,
    target_width: usize,
    rule_style: Style,
    label_style: Style,
) -> Vec<Span<'a>> {
    let label_text = format!(" {label} ");
    let label_visible = label_text.chars().count();
    // 1 leading dash + label + N trailing dashes = target_width
    let trailing = target_width.saturating_sub(1 + label_visible);
    vec![
        Span::styled("─", rule_style),
        Span::styled(label_text, label_style),
        Span::styled("─".repeat(trailing), rule_style),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn plain_text(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect::<String>()
    }

    #[test]
    fn pad_row_pads_short_content_to_target_width() {
        let spans = vec![Span::raw("hi")];
        let padded = pad_row(spans, 6, Style::default());
        assert_eq!(row_width(&padded), 6);
        assert_eq!(plain_text(&padded), "hi    ");
    }

    #[test]
    fn pad_row_truncates_when_exceeding_target_width() {
        let spans = vec![Span::raw("abcdefghij")];
        let padded = pad_row(spans, 5, Style::default());
        assert_eq!(row_width(&padded), 5);
        assert_eq!(plain_text(&padded), "abcde");
    }

    #[test]
    fn pad_row_truncation_respects_span_boundaries() {
        let spans = vec![
            Span::styled("aa", Style::default().fg(Color::Red)),
            Span::styled("bbb", Style::default().fg(Color::Green)),
            Span::styled("cccc", Style::default().fg(Color::Blue)),
        ];
        let padded = pad_row(spans, 4, Style::default());
        assert_eq!(row_width(&padded), 4);
        // First span fits whole, second span only contributes 2 chars,
        // third span is dropped.
        assert_eq!(padded.len(), 2);
        assert_eq!(padded[0].content.as_ref(), "aa");
        assert_eq!(padded[1].content.as_ref(), "bb");
    }

    #[test]
    fn join_horizontal_top_aligns_columns_and_pads_height() {
        let left = vec![vec![Span::raw("L1")], vec![Span::raw("L2")]];
        let right = vec![vec![Span::raw("R1")]];
        let joined = join_horizontal_top(vec![(4, left), (4, right)], Style::default());
        assert_eq!(joined.len(), 2);
        assert_eq!(plain_text(&joined[0]), "L1  R1  ");
        // Second row: left has L2, right ran out and got a blank pad row.
        assert_eq!(plain_text(&joined[1]), "L2      ");
    }

    #[test]
    fn box_with_chrome_frames_body_to_inner_width_plus_two() {
        let body = vec![vec![Span::raw("body")]];
        let chrome = ChromeStyle {
            frame: Style::default(),
            body_pad: Style::default(),
        };
        let lines = box_with_chrome(
            body,
            10,
            vec![Span::raw("title")],
            vec![Span::raw("foot")],
            chrome,
        );
        assert_eq!(lines.len(), 3);
        for line in &lines {
            let visible: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            assert_eq!(visible, 12);
        }
        let top: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(top.starts_with("┏━ title"));
        assert!(top.ends_with("┓"));
        let bottom: String = lines[2].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(bottom.starts_with("┗━ foot"));
        assert!(bottom.ends_with("┛"));
    }

    #[test]
    fn section_divider_fills_target_width() {
        let spans = section_divider("today", 20, Style::default(), Style::default());
        assert_eq!(row_width(&spans), 20);
        let text = plain_text(&spans);
        assert!(text.starts_with("─ today "));
        assert!(text.ends_with("─"));
    }
}
