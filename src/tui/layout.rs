use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::pet::render::PaletteRoleName;
use crate::tui::{
    style::{
        semantic_styles, tokenpet_palette, BarRamp, ColorCapability, SemanticStyles,
        BAR_RAMP_ACCENT, BAR_RAMP_GOOD,
    },
    view_model::{SourceStatus, WatchViewModel},
};
use crate::tui::style::ramp_index;

/// Expected source surfaces and their display names.
/// Order is the render order for the today panel and helpers row.
const EXPECTED_SOURCES: &[(&str, &str)] = &[
    ("claude-code", "claude"),
    ("codex", "codex"),
];

pub fn render_help_overlay(frame: &mut Frame<'_>) {
    render_overlay(
        frame,
        "glorp help",
        &[
            "q quit   r refresh   ? help",
            "r refreshes usage and pet state now",
            "usage polls stay calm when helpers are blocked",
        ],
    );
}

pub fn render_evolution_overlay(frame: &mut Frame<'_>, stage_label: Option<&str>) {
    let label_line = match stage_label {
        Some(label) if !label.is_empty() => format!("evolved into {label}"),
        _ => "your pet is changing shape".to_string(),
    };
    render_overlay(
        frame,
        "glorp evolution",
        &[
            label_line.as_str(),
            "new stage art appears after the next settled tick",
            "keep feeding it real work",
        ],
    );
}

pub fn render_hatch_overlay(frame: &mut Frame<'_>) {
    render_overlay(
        frame,
        "glorp hatch",
        &[
            "a small terminal creature blinks awake",
            "local state is private",
            "feed comes from effective token deltas",
        ],
    );
}

fn role_spans_for_line<'a>(
    art_line: &'a str,
    line_index: usize,
    pet_spans: &'a [crate::pet::render::StyledSegment],
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    let total_chars = art_line.chars().count();
    if total_chars == 0 {
        return Vec::new();
    }

    let mut segments: Vec<&crate::pet::render::StyledSegment> = pet_spans
        .iter()
        .filter(|span| span.line == line_index && span.start < span.end && span.start < total_chars)
        .collect();
    segments.sort_by_key(|span| span.start);

    if segments.is_empty() {
        return vec![Span::styled(art_line, styles.pet_body)];
    }

    let char_indices = char_byte_indices(art_line);
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut cursor = 0usize;

    for segment in segments {
        let start = segment.start.max(cursor).min(total_chars);
        let end = segment.end.min(total_chars);
        if end <= cursor {
            continue;
        }
        if start > cursor {
            let body = char_slice(art_line, &char_indices, cursor, start);
            spans.push(Span::styled(body, styles.pet_body));
        }
        let value = char_slice(art_line, &char_indices, start, end);
        spans.push(Span::styled(value, role_style(segment.role, styles)));
        cursor = end;
    }

    if cursor < total_chars {
        let tail = char_slice(art_line, &char_indices, cursor, total_chars);
        spans.push(Span::styled(tail, styles.pet_body));
    }

    spans
}

fn char_byte_indices(line: &str) -> Vec<usize> {
    let mut indices: Vec<usize> = line.char_indices().map(|(byte, _)| byte).collect();
    indices.push(line.len());
    indices
}

fn char_slice<'a>(line: &'a str, indices: &[usize], start_char: usize, end_char: usize) -> &'a str {
    let start = indices[start_char];
    let end = indices[end_char];
    &line[start..end]
}

fn role_style(role: PaletteRoleName, styles: &SemanticStyles) -> Style {
    match role {
        PaletteRoleName::Body => styles.pet_body,
        PaletteRoleName::Eye => styles.pet_eye,
        PaletteRoleName::Mouth => styles.pet_mouth,
        PaletteRoleName::Accent => styles.pet_accent,
        PaletteRoleName::Pattern => styles.pet_pattern,
        PaletteRoleName::Particle => styles.pet_accent,
    }
}

fn section_line<'a>(label: &'a str, target_width: usize, styles: &'a SemanticStyles) -> Vec<Span<'a>> {
    let label_text = format!(" {label} ");
    let label_visible = label_text.chars().count();
    let dash_total = target_width.saturating_sub(label_visible + 1); // +1 for leading dash
    let leading = "─";
    let trailing_count = dash_total;
    let trailing: String = std::iter::repeat('─').take(trailing_count).collect();
    vec![
        Span::styled(leading, styles.section_header),
        Span::styled(label_text, styles.label),
        Span::styled(trailing, styles.section_header),
    ]
}

fn bar_line_spans<'a>(
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

fn render_overlay(frame: &mut Frame<'_>, title: &str, copy: &[&str]) {
    let area = centered_rect(frame.area(), 62, 9);
    let styles = semantic_styles();
    let p = tokenpet_palette();
    frame.render_widget(Clear, area);
    let lines = copy
        .iter()
        .map(|line| Line::from(Span::styled(*line, styles.primary_text)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(p.fg.rgb).bg(p.surface.rgb))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title(Span::styled(title.to_string(), styles.label))
                    .borders(Borders::ALL)
                    .border_style(styles.overlay_border)
                    .style(styles.overlay_surface),
            ),
        area,
    );
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn body_row<'a>(inner: Vec<Span<'a>>, inner_width: usize, styles: &'a SemanticStyles) -> Line<'a> {
    let visible: usize = inner.iter().map(|s| s.content.chars().count()).sum();
    let frame_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(inner.len() + 3);
    spans.push(Span::styled("┃", frame_style));
    if visible <= inner_width {
        spans.extend(inner);
        let pad = inner_width - visible;
        if pad > 0 {
            spans.push(Span::styled(" ".repeat(pad), styles.body));
        }
    } else {
        // Truncate cell-by-cell to fit; last visible char may be cut on a
        // multi-char span boundary. Scan spans and accumulate up to inner_width.
        let mut remaining = inner_width;
        for span in inner {
            let span_len = span.content.chars().count();
            if span_len <= remaining {
                spans.push(span);
                remaining -= span_len;
            } else {
                let truncated: String = span.content.chars().take(remaining).collect();
                spans.push(Span::styled(truncated, span.style));
                break;
            }
        }
    }
    spans.push(Span::styled("┃", frame_style));
    Line::from(spans)
}

const NAME_MAX: usize = 16;

fn render_frame_top_line<'a>(
    width: usize,
    pet_name: &'a str,
    species: &'a str,
    stage: &'a str,
    age: &'a str,
    mood: &'a str,
    styles: &'a SemanticStyles,
) -> Line<'a> {
    let frame_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let stage_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let mood_style = Style::default().fg(tokenpet_palette().good.rgb);
    let display_name: String = if pet_name.chars().count() > NAME_MAX {
        let truncated: String = pet_name.chars().take(NAME_MAX - 1).collect();
        format!("{truncated}…")
    } else {
        pet_name.to_string()
    };
    let title_text = format!("glorp · {display_name} the {species} · {stage} · {age} · {mood}");
    let title_visible = title_text.chars().count();
    let n_fill = width.saturating_sub(5 + title_visible);
    // Build the title as styled segments. Render the stage in accent and the
    // mood in good color; everything else uses the dim label style.
    let prefix = format!("glorp · {display_name} the {species} · ");
    let between = format!(" · {age} · ");
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::styled("┏━ ", frame_style));
    spans.push(Span::styled(prefix, styles.label));
    spans.push(Span::styled(stage.to_string(), stage_style));
    spans.push(Span::styled(between, styles.label));
    spans.push(Span::styled(mood.to_string(), mood_style));
    spans.push(Span::styled(" ".to_string(), styles.label));
    spans.push(Span::styled("━".repeat(n_fill), frame_style));
    spans.push(Span::styled("┓", frame_style));
    Line::from(spans)
}

fn render_frame_bottom_line<'a>(width: usize, styles: &'a SemanticStyles) -> Line<'a> {
    let frame_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let footer_text = "q quit · r refresh · ? help";
    let footer_visible = footer_text.chars().count();
    let n_fill = width.saturating_sub(5 + footer_visible);
    let spans = vec![
        Span::styled("┗━ ", frame_style),
        Span::styled(footer_text.to_string(), styles.label),
        Span::styled(" ".to_string(), styles.label),
        Span::styled("━".repeat(n_fill), frame_style),
        Span::styled("┛", frame_style),
    ];
    Line::from(spans)
}

fn render_today_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out: Vec<Line<'a>> = Vec::new();
    let header = section_line("today", width, styles);
    out.push(Line::from(header));
    out.push(today_row("tokens", &format_tokens_full(vm.today_effective_tokens), None, styles));
    let total = vm.today_effective_tokens.max(0.0);
    for (surface, display) in EXPECTED_SOURCES {
        let value_opt = vm
            .source_breakdown
            .iter()
            .find(|s| s.name == *surface)
            .map(|s| s.effective_tokens);
        let (value_str, share) = match value_opt {
            Some(v) => {
                let pct = if total > 0.0 { (v / total) * 100.0 } else { 0.0 };
                (format_tokens_full(v), Some(format!("{}%", pct.round() as u32)))
            }
            None => ("—".to_string(), Some("—".to_string())),
        };
        out.push(today_row(display, &value_str, share, styles));
    }
    let bucket_str = format_signed_tokens_short(vm.current_bucket_effective_tokens);
    out.push(today_row("last 10m", &bucket_str, Some("this 10m".to_string()), styles));
    out
}

fn today_row<'a>(
    label: &'a str,
    value: &str,
    annotation: Option<String>,
    styles: &'a SemanticStyles,
) -> Line<'a> {
    // Fixed-column layout so values and annotations stay aligned across rows
    // even when token magnitudes differ by orders of magnitude.
    //   2 sp + label(8) + 1 sp + value(right-aligned, 13) + 4 sp + annotation
    const VALUE_WIDTH: usize = 13;
    let value_owned = format!("{value:>VALUE_WIDTH$}");
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("{label:<8}"), styles.label));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(value_owned, styles.primary_text));
    if let Some(ann) = annotation {
        spans.push(Span::raw("    "));
        spans.push(Span::styled(ann, styles.label));
    }
    Line::from(spans)
}

fn format_tokens_full(n: f64) -> String {
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

fn format_signed_tokens_short(n: f64) -> String {
    let abs = n.abs();
    let unit = if abs >= 1_000_000.0 {
        format!("{:.1}m", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}k", abs / 1_000.0)
    } else {
        format!("{}", abs.round() as i64)
    };
    // Avoid rendering "-0" when the rounded absolute value is zero.
    if n < 0.0 && unit != "0" {
        format!("-{unit}")
    } else {
        format!("+{unit}")
    }
}

fn render_helpers_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    out.push(Line::from(section_line("helpers", width, styles)));
    let p = tokenpet_palette();
    let mut spans: Vec<Span<'a>> = Vec::new();
    spans.push(Span::raw("  "));
    let mut first = true;
    for (surface, display) in EXPECTED_SOURCES {
        if !first {
            spans.push(Span::raw("     "));
        }
        first = false;
        let health = vm.source_health.iter().find(|s| s.name == *surface);
        let (glyph, glyph_style) = match health.map(|h| h.status) {
            Some(SourceStatus::Ready) => ('✓', Style::default().fg(p.good.rgb)),
            Some(SourceStatus::Diagnostic) => ('~', Style::default().fg(p.accent.rgb)),
            Some(SourceStatus::Blocked) => ('✗', Style::default().fg(p.bad.rgb)),
            None => ('—', Style::default().fg(p.dim.rgb)),
        };
        spans.push(Span::styled(display.to_string(), styles.label));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(glyph.to_string(), glyph_style));
    }
    out.push(Line::from(spans));
    out
}

fn render_feed_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    render_feed_panel_lines_capped(width, vm, 3, styles)
}

fn render_feed_panel_lines_capped<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    max_entries: usize,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    out.push(Line::from(section_line("feed", width, styles)));
    for event in vm.recent_events.iter().take(max_entries) {
        let mut spans: Vec<Span<'a>> = Vec::new();
        spans.push(Span::raw("  "));
        spans.push(Span::styled(event.timestamp.clone(), styles.timestamp));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(event.text.clone(), styles.log(event.kind)));
        out.push(Line::from(spans));
    }
    out
}

#[cfg(test)]
mod today_panel_tests {
    use super::*;
    use crate::tui::view_model::WatchViewModel;

    #[test]
    fn today_panel_has_four_rows_plus_rule() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_today_panel_lines(43, &vm, &styles);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn today_panel_renders_dash_for_absent_source() {
        let styles = semantic_styles();
        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown.retain(|s| s.name != "codex");
        let lines = render_today_panel_lines(43, &vm, &styles);
        let text: String = lines[3]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains("codex"));
        assert!(text.contains("—"));
    }
}

#[cfg(test)]
mod frame_bottom_tests {
    use super::*;

    #[test]
    fn frame_bottom_pads_to_target_width() {
        let styles = semantic_styles();
        let line = render_frame_bottom_line(78, &styles);
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 78);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.starts_with("┗━"));
        assert!(text.ends_with("┛"));
        assert!(text.contains("q quit"));
    }
}

#[cfg(test)]
mod frame_top_tests {
    use super::*;

    #[test]
    fn frame_top_pads_to_target_width() {
        let styles = semantic_styles();
        let line = render_frame_top_line(78, "mochi", "fuzz", "pup", "12d 4h", "content", &styles);
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 78);
    }

    #[test]
    fn frame_top_truncates_long_pet_name() {
        let styles = semantic_styles();
        let very_long = "thisnameiswaytoolongforthetitle";
        let line = render_frame_top_line(78, very_long, "fuzz", "pup", "12d 4h", "content", &styles);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert!(text.contains("…"));
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 78);
    }
}

#[cfg(test)]
mod body_row_tests {
    use super::*;

    #[test]
    fn body_row_pads_short_content_to_inner_width() {
        let styles = semantic_styles();
        let inner: Vec<Span> = vec![Span::raw("hi")];
        let line = body_row(inner, 10, &styles);
        // Visible width: ┃ + 10 + ┃ = 12.
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 12);
        // First and last spans must be ┃.
        assert_eq!(line.spans.first().unwrap().content.as_ref(), "┃");
        assert_eq!(line.spans.last().unwrap().content.as_ref(), "┃");
    }

    #[test]
    fn body_row_truncates_overflowing_content() {
        let styles = semantic_styles();
        let inner: Vec<Span> = vec![Span::raw("xxxxxxxxxxxxxxxx")]; // 16 chars
        let line = body_row(inner, 10, &styles);
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, 12);
    }
}

fn render_sparkline_lines<'a>(
    width: usize,
    history: &[f64],
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out = Vec::new();
    out.push(Line::from(section_line("7-day", width, styles)));

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
        let glyph = if *value <= 0.0 {
            '·'
        } else if max <= 0.0 {
            '·'
        } else {
            let level = ((value / max) * (glyphs.len() as f64 - 1.0)).round() as usize;
            glyphs[level.min(glyphs.len() - 1)]
        };
        let style = match capability {
            ColorCapability::Truecolor => {
                let idx = ramp_index(i, 7);
                Style::default().fg(BAR_RAMP_GOOD.stops[idx])
            }
            ColorCapability::Flat => Style::default().fg(tokenpet_palette().good.rgb),
        };
        let style = if glyph == '·' {
            styles.empty_bar
        } else {
            style
        };
        spans.push(Span::styled(glyph.to_string(), style));
        if i < 6 {
            spans.push(Span::raw("   "));
        }
    }
    out.push(Line::from(spans));
    out
}

#[cfg(test)]
mod sparkline_tests {
    use super::*;
    use crate::tui::style::ColorCapability;

    #[test]
    fn sparkline_row_returns_two_lines() {
        let styles = semantic_styles();
        let history = vec![100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0];
        let lines = render_sparkline_lines(43, &history, ColorCapability::Truecolor, &styles);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn sparkline_zeroes_render_dot() {
        let styles = semantic_styles();
        let history = vec![0.0; 7];
        let lines = render_sparkline_lines(43, &history, ColorCapability::Truecolor, &styles);
        let text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains('·'));
        assert!(!text.contains('█'));
    }
}

#[cfg(test)]
mod helpers_panel_tests {
    use super::*;
    use crate::tui::view_model::{SourceStatus, WatchViewModel};

    #[test]
    fn helpers_panel_renders_check_when_ready() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_helpers_panel_lines(43, &vm, &styles);
        let text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains('✓'));
    }

    #[test]
    fn helpers_panel_renders_x_when_blocked() {
        let styles = semantic_styles();
        let mut vm = WatchViewModel::fixture();
        for src in vm.source_health.iter_mut() {
            src.status = SourceStatus::Blocked;
        }
        let lines = render_helpers_panel_lines(43, &vm, &styles);
        let text: String = lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(text.contains('✗'));
    }
}

#[cfg(test)]
mod feed_panel_tests {
    use super::*;
    use crate::tui::view_model::WatchViewModel;

    #[test]
    fn feed_panel_returns_rule_plus_up_to_three_entries() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture_with_events();
        let lines = render_feed_panel_lines(43, &vm, &styles);
        assert!(lines.len() >= 1 && lines.len() <= 4);
    }
}

fn render_pet_panel_lines<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let mut out: Vec<Line<'a>> = Vec::new();
    let left_pad = (width.saturating_sub(11)) / 2;
    out.push(Line::from(Span::raw("")));
    for (line_index, art_line) in vm.pet_art.iter().enumerate() {
        let mut spans: Vec<Span<'a>> = Vec::new();
        spans.push(Span::raw(" ".repeat(left_pad)));
        spans.extend(role_spans_for_line(
            art_line,
            line_index,
            &vm.pet_spans,
            styles,
        ));
        out.push(Line::from(spans));
    }
    let ground = ",".repeat(width.saturating_sub(2));
    out.push(Line::from(vec![
        Span::raw(" "),
        Span::styled(ground, styles.empty_bar),
    ]));
    out.push(Line::from(Span::raw("")));
    out.push(Line::from(section_line("vitals", width, styles)));
    out.push(Line::from(bar_line_spans(
        "fed",
        vm.fed,
        BAR_RAMP_GOOD,
        capability,
        styles,
    )));
    out.push(Line::from(bar_line_spans(
        "happy",
        vm.happiness,
        BAR_RAMP_ACCENT,
        capability,
        styles,
    )));
    out.push(Line::from(bar_line_spans(
        "energy",
        vm.energy,
        BAR_RAMP_GOOD,
        capability,
        styles,
    )));
    let xp_fraction = if vm.xp_target <= 0.0 {
        0.0
    } else {
        (vm.xp_current / vm.xp_target).clamp(0.0, 1.0)
    };
    out.push(Line::from(bar_line_spans(
        "xp",
        xp_fraction,
        BAR_RAMP_ACCENT,
        capability,
        styles,
    )));
    out
}

#[cfg(test)]
mod pet_panel_tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;

    #[test]
    fn pet_panel_includes_vitals_rule_and_four_bars() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_pet_panel_lines(26, &vm, ColorCapability::Truecolor, &styles);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<String>();
        assert!(text.contains("vitals"));
        assert!(text.contains("fed"));
        assert!(text.contains("happy"));
        assert!(text.contains("energy"));
        assert!(text.contains("xp"));
    }

    #[test]
    fn pet_panel_does_not_include_meta_block() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let lines = render_pet_panel_lines(26, &vm, ColorCapability::Truecolor, &styles);
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect::<String>();
        assert!(!text.contains("species"));
        assert!(!text.contains("stage"));
        assert!(!text.contains("mood"));
    }
}

#[cfg(test)]
mod bar_line_tests {
    use super::*;
    use crate::tui::style::{BAR_RAMP_GOOD, ColorCapability};

    #[test]
    fn bar_line_zero_fill_renders_twelve_faint() {
        let styles = semantic_styles();
        let spans = bar_line_spans("fed", 0.0, BAR_RAMP_GOOD, ColorCapability::Truecolor, &styles);
        let bar_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let fill_count = bar_text.chars().filter(|c| *c == '░').count();
        assert_eq!(fill_count, 12);
        let solid_count = bar_text.chars().filter(|c| *c == '█').count();
        assert_eq!(solid_count, 0);
    }

    #[test]
    fn bar_line_full_fill_renders_twelve_solid() {
        let styles = semantic_styles();
        let spans = bar_line_spans("fed", 1.0, BAR_RAMP_GOOD, ColorCapability::Truecolor, &styles);
        let bar_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let solid_count = bar_text.chars().filter(|c| *c == '█').count();
        assert_eq!(solid_count, 12);
    }

    #[test]
    fn bar_line_flat_capability_uses_solid_color() {
        let styles = semantic_styles();
        let spans = bar_line_spans("fed", 0.5, BAR_RAMP_GOOD, ColorCapability::Flat, &styles);
        let filled: Vec<_> = spans
            .iter()
            .filter(|s| s.content.contains('█'))
            .map(|s| s.style)
            .collect();
        let first = filled.first().copied().unwrap();
        for s in &filled {
            assert_eq!(*s, first);
        }
    }
}

pub fn render_watch_frame_with_capability(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    capability: ColorCapability,
) {
    let area = frame.area();
    let p = tokenpet_palette();
    frame.render_widget(Block::default().style(Style::default().bg(p.bg.rgb)), area);
    if area.height == 0 || area.width == 0 {
        return;
    }
    let styles = semantic_styles();
    if (area.width as usize) < COMPACT_THRESHOLD {
        render_compact(frame, area, vm, capability, &styles);
    } else {
        render_wide(frame, area, vm, capability, &styles);
    }
}

const COMPACT_THRESHOLD: usize = 80;

fn render_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &WatchViewModel,
    capability: ColorCapability,
    styles: &SemanticStyles,
) {
    let width = area.width as usize;
    let inner_width = width.saturating_sub(2);
    let pet_col = 26;
    let gap = 2;
    let data_col = 43;
    let base_pad_left = 2;
    let base_pad_right = 3;
    let baseline = pet_col + gap + data_col + base_pad_left + base_pad_right;
    // Center content when the terminal is wider than the baseline (78-col frame).
    // Surplus splits equally between left and right outer pads so the pet+data
    // block sits in the middle of the frame instead of clinging to the left edge.
    let extra = inner_width.saturating_sub(baseline);
    let pad_left = base_pad_left + extra / 2;
    let pad_right = base_pad_right + extra - extra / 2;

    let body_height = area.height.saturating_sub(2) as usize;

    // Build the data column. Feed grows first, then any remaining residual
    // is distributed as extra blank lines between sections so the body fills
    // the frame without leaving a cavity at the bottom.
    let today_lines = render_today_panel_lines(data_col, vm, styles);
    let spark_lines = render_sparkline_lines(data_col, &vm.recent_daily_effective_tokens, capability, styles);
    let helpers_lines = render_helpers_panel_lines(data_col, vm, styles);
    // Fixed overhead with single-blank separators:
    //   top_blank + today + blank + spark + blank + feed_rule + blank + helpers + bottom_blank
    let single_gap_overhead = 1 + today_lines.len() + 1 + spark_lines.len() + 1 + 1 + 1 + helpers_lines.len() + 1;
    let event_count = vm.recent_events.len();
    let max_feed_entries = body_height
        .saturating_sub(single_gap_overhead)
        .max(2)
        .min(event_count.max(2));
    let feed_lines = render_feed_panel_lines_capped(data_col, vm, max_feed_entries, styles);

    // Now compute remaining residual after feed has grown. Distribute it as
    // extra blank lines across 5 separator slots (top, between today/spark,
    // spark/feed, feed/helpers, bottom).
    let consumed = today_lines.len() + spark_lines.len() + feed_lines.len() + helpers_lines.len() + 5;
    let residual = body_height.saturating_sub(consumed);
    let slots = 5;
    let extra_per_slot = residual / slots;
    let extra_remainder = residual % slots;
    let blank_run = |n: usize| -> Vec<Line<'_>> {
        (0..n).map(|_| Line::from(Span::raw(""))).collect()
    };
    let slot_size = |slot_index: usize| -> usize {
        1 + extra_per_slot + if slot_index < extra_remainder { 1 } else { 0 }
    };

    let mut data_lines: Vec<Line> = Vec::new();
    data_lines.extend(blank_run(slot_size(0)));
    data_lines.extend(today_lines);
    data_lines.extend(blank_run(slot_size(1)));
    data_lines.extend(spark_lines);
    data_lines.extend(blank_run(slot_size(2)));
    data_lines.extend(feed_lines);
    data_lines.extend(blank_run(slot_size(3)));
    data_lines.extend(helpers_lines);
    data_lines.extend(blank_run(slot_size(4)));

    // Pet column: vertically center the pet+vitals block within available height.
    let raw_pet_lines = render_pet_panel_lines(pet_col, vm, capability, styles);
    let pet_lines: Vec<Line> = if raw_pet_lines.len() < body_height {
        let pad_top = (body_height - raw_pet_lines.len()) / 2;
        let mut padded: Vec<Line> = (0..pad_top).map(|_| Line::from(Span::raw(""))).collect();
        padded.extend(raw_pet_lines);
        padded
    } else {
        raw_pet_lines
    };

    let age_label = format!("{}d", vm.age_days);
    let mut framed: Vec<Line> = Vec::new();
    framed.push(render_frame_top_line(
        width,
        &vm.pet_name,
        &vm.species,
        &vm.stage,
        &age_label,
        &vm.mood,
        styles,
    ));
    let max_rows = pet_lines.len().max(data_lines.len()).max(body_height);
    for row_index in 0..max_rows {
        let pet_line = pet_lines.get(row_index);
        let data_line = data_lines.get(row_index);
        let mut inner: Vec<Span> = Vec::new();
        inner.push(Span::raw(" ".repeat(pad_left)));
        if let Some(line) = pet_line {
            let cell_count: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            inner.extend(line.spans.iter().cloned());
            if cell_count < pet_col {
                inner.push(Span::raw(" ".repeat(pet_col - cell_count)));
            }
        } else {
            inner.push(Span::raw(" ".repeat(pet_col)));
        }
        inner.push(Span::raw(" ".repeat(gap)));
        if let Some(line) = data_line {
            let cell_count: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            inner.extend(line.spans.iter().cloned());
            if cell_count < data_col {
                inner.push(Span::raw(" ".repeat(data_col - cell_count)));
            }
        } else {
            inner.push(Span::raw(" ".repeat(data_col)));
        }
        inner.push(Span::raw(" ".repeat(pad_right)));
        framed.push(body_row(inner, inner_width, styles));
    }
    framed.push(render_frame_bottom_line(width, styles));
    frame.render_widget(Paragraph::new(framed).style(styles.body), area);
}

fn render_compact(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &WatchViewModel,
    capability: ColorCapability,
    styles: &SemanticStyles,
) {
    let width = area.width as usize;
    let height = area.height as usize;

    if height == 0 {
        return;
    }

    if height < 10 {
        let summary = format!(
            "fed {} · happy {} · energy {} · xp {}",
            (vm.fed * 100.0).round() as u32,
            (vm.happiness * 100.0).round() as u32,
            (vm.energy * 100.0).round() as u32,
            if vm.xp_target > 0.0 {
                ((vm.xp_current / vm.xp_target).clamp(0.0, 1.0) * 100.0).round() as u32
            } else {
                0
            },
        );
        let lines = vec![Line::from(Span::styled(summary, styles.primary_text))];
        frame.render_widget(Paragraph::new(lines).style(styles.body), area);
        return;
    }

    let pet = render_pet_panel_lines(width, vm, capability, styles);
    let today = render_today_panel_lines(width, vm, styles);
    let spark = render_sparkline_lines(width, &vm.recent_daily_effective_tokens, capability, styles);
    let feed = render_feed_panel_lines(width, vm, styles);
    let helpers = render_helpers_panel_lines(width, vm, styles);

    let footer = Line::from(vec![
        Span::styled("q", styles.label),
        Span::styled(" quit  ", styles.label),
        Span::styled("r", styles.label),
        Span::styled(" refresh  ", styles.label),
        Span::styled("?", styles.label),
        Span::styled(" help", styles.label),
    ]);

    let mut all: Vec<Line> = Vec::new();
    let groups: Vec<Vec<Line>> = vec![pet, today, spark, feed, helpers];
    for group in groups {
        if all.len() + group.len() + 1 > height.saturating_sub(1) {
            break;
        }
        all.extend(group);
        all.push(Line::from(Span::raw("")));
    }
    if all.len() < height {
        all.push(footer);
    } else if !all.is_empty() {
        let last_idx = all.len() - 1;
        all[last_idx] = footer;
    }

    frame.render_widget(Paragraph::new(all).style(styles.body), area);
}

#[cfg(test)]
mod render_compact_tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_compact_does_not_draw_frame() {
        let backend = TestBackend::new(60, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row0: String = (0..60).map(|x| buffer[(x, 0)].symbol().to_string()).collect();
        assert!(!row0.contains("┏"));
        assert!(!row0.contains("┗"));
    }

    #[test]
    fn render_compact_drops_helpers_under_height_pressure() {
        let backend = TestBackend::new(60, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut all = String::new();
        for y in 0..12 {
            for x in 0..60 {
                all.push_str(buffer[(x, y)].symbol());
            }
            all.push('\n');
        }
        assert!(!all.contains("helpers"));
    }
}

#[cfg(test)]
mod render_wide_tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_wide_draws_frame_at_80_cols() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| {
                render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row0: String = (0..80).map(|x| buffer[(x, 0)].symbol().to_string()).collect();
        assert!(row0.starts_with("┏━"));
        assert!(row0.ends_with("┓"));
    }

    #[test]
    fn render_wide_draws_frame_at_100_cols() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| {
                render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row0: String = (0..100).map(|x| buffer[(x, 0)].symbol().to_string()).collect();
        assert!(row0.starts_with("┏━"));
        assert!(row0.ends_with("┓"));
        // Side rails should connect at column 0 and column 99.
        for y in 1..29 {
            assert_eq!(buffer[(0u16, y as u16)].symbol(), "┃", "row {y} left rail broken");
            assert_eq!(buffer[(99u16, y as u16)].symbol(), "┃", "row {y} right rail broken");
        }
        // Bottom row connects with footer.
        let row_last: String = (0..100).map(|x| buffer[(x, 29)].symbol().to_string()).collect();
        assert!(row_last.starts_with("┗━"));
        assert!(row_last.ends_with("┛"));
    }
}
