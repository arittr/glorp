use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::pet::render::PaletteRoleName;
use crate::tui::composer::{
    box_with_chrome, join_horizontal_top, pad_rows, section_divider, ChromeStyle,
};
use crate::tui::style::ramp_index;
use crate::tui::{
    style::{
        semantic_styles, tokenpet_palette, BarRamp, ColorCapability, SemanticStyles,
        BAR_RAMP_ACCENT, BAR_RAMP_GOOD,
    },
    view_model::{SourceStatus, WatchViewModel},
};

/// Expected source surfaces and their display names.
/// Order is the render order for the today panel and helpers row.
const EXPECTED_SOURCES: &[(&str, &str)] = &[("claude-code", "claude"), ("codex", "codex")];

/// Fixed-width layout constants matching the hybrid-v4 mockup.
/// Geometry: `┃` + LEFT_PAD + LEFT_COL + GUTTER + RIGHT_COL + RIGHT_PAD + `┃`
/// equals 78 visible cells total.
const FRAME_BODY_WIDTH: usize = 78;
const LEFT_PAD: usize = 2;
const RIGHT_PAD: usize = 3;
const GUTTER: usize = 2;
const LEFT_COL: usize = 26;
const RIGHT_COL: usize = 43;

const COMPACT_THRESHOLD: usize = FRAME_BODY_WIDTH;

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

fn section_row<'a>(
    label: &'a str,
    target_width: usize,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    section_divider(label, target_width, styles.section_header, styles.label)
}

fn bar_row<'a>(
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

const NAME_MAX: usize = 16;

fn title_spans<'a>(
    pet_name: &'a str,
    species: &'a str,
    stage: &'a str,
    age: &'a str,
    mood: &'a str,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
    let stage_style = Style::default().fg(tokenpet_palette().accent.rgb);
    let mood_style = Style::default().fg(tokenpet_palette().good.rgb);
    let display_name: String = if pet_name.chars().count() > NAME_MAX {
        let truncated: String = pet_name.chars().take(NAME_MAX - 1).collect();
        format!("{truncated}…")
    } else {
        pet_name.to_string()
    };
    vec![
        Span::styled(
            format!("glorp · {display_name} the {species} · "),
            styles.label,
        ),
        Span::styled(stage.to_string(), stage_style),
        Span::styled(format!(" · {age} · "), styles.label),
        Span::styled(mood.to_string(), mood_style),
    ]
}

fn footer_spans(styles: &SemanticStyles) -> Vec<Span<'_>> {
    vec![Span::styled("q quit · r refresh · ? help", styles.label)]
}

fn render_today_panel_rows<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Vec<Span<'a>>> {
    let mut out: Vec<Vec<Span<'a>>> = Vec::new();
    out.push(section_row("today", width, styles));
    out.push(today_row(
        "tokens",
        &format_tokens_full(vm.today_effective_tokens),
        None,
        styles,
    ));
    let total = vm.today_effective_tokens.max(0.0);
    for (surface, display) in EXPECTED_SOURCES {
        let value_opt = vm
            .source_breakdown
            .iter()
            .find(|s| s.name == *surface)
            .map(|s| s.effective_tokens);
        let (value_str, share) = match value_opt {
            Some(v) => {
                let pct = if total > 0.0 {
                    (v / total) * 100.0
                } else {
                    0.0
                };
                (
                    format_tokens_full(v),
                    Some(format!("{}%", pct.round() as u32)),
                )
            }
            None => ("—".to_string(), Some("—".to_string())),
        };
        out.push(today_row(display, &value_str, share, styles));
    }
    let bucket_str = format_signed_tokens_short(vm.current_bucket_effective_tokens);
    out.push(today_row(
        "last 10m",
        &bucket_str,
        Some("this 10m".to_string()),
        styles,
    ));
    out
}

fn today_row<'a>(
    label: &'a str,
    value: &str,
    annotation: Option<String>,
    styles: &'a SemanticStyles,
) -> Vec<Span<'a>> {
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
    spans
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

fn render_helpers_panel_rows<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    styles: &'a SemanticStyles,
) -> Vec<Vec<Span<'a>>> {
    let mut out: Vec<Vec<Span<'a>>> = Vec::new();
    out.push(section_row("helpers", width, styles));
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
    out.push(spans);
    out
}

fn render_feed_panel_rows<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    max_entries: usize,
    styles: &'a SemanticStyles,
) -> Vec<Vec<Span<'a>>> {
    let mut out: Vec<Vec<Span<'a>>> = Vec::new();
    out.push(section_row("feed", width, styles));
    for event in vm.recent_events.iter().take(max_entries) {
        out.push(vec![
            Span::raw("  "),
            Span::styled(event.timestamp.clone(), styles.timestamp),
            Span::raw("  "),
            Span::styled(event.text.clone(), styles.log(event.kind)),
        ]);
    }
    out
}

fn render_sparkline_rows<'a>(
    width: usize,
    history: &[f64],
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Vec<Span<'a>>> {
    let mut out: Vec<Vec<Span<'a>>> = Vec::new();
    out.push(section_row("7-day", width, styles));

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
    out.push(spans);
    out
}

fn render_pet_panel_rows<'a>(
    width: usize,
    vm: &'a WatchViewModel,
    capability: ColorCapability,
    styles: &'a SemanticStyles,
) -> Vec<Vec<Span<'a>>> {
    let mut out: Vec<Vec<Span<'a>>> = Vec::new();
    // Center the pet art horizontally within the column. Pet art lines
    // are 11 cells wide in the standard fixture; if the art is wider we
    // still left-pad by zero so it just fills the column.
    let pet_width = vm
        .pet_art
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let left_pad = width.saturating_sub(pet_width) / 2;
    out.push(Vec::new());
    for (line_index, art_line) in vm.pet_art.iter().enumerate() {
        let mut spans: Vec<Span<'a>> = Vec::new();
        if left_pad > 0 {
            spans.push(Span::raw(" ".repeat(left_pad)));
        }
        spans.extend(role_spans_for_line(
            art_line,
            line_index,
            &vm.pet_spans,
            styles,
        ));
        out.push(spans);
    }
    let ground_width = width.saturating_sub(2);
    out.push(vec![
        Span::raw(" "),
        Span::styled(",".repeat(ground_width), styles.empty_bar),
    ]);
    out.push(Vec::new());
    out.push(section_row("vitals", width, styles));
    out.push(bar_row("fed", vm.fed, BAR_RAMP_GOOD, capability, styles));
    out.push(bar_row(
        "happy",
        vm.happiness,
        BAR_RAMP_ACCENT,
        capability,
        styles,
    ));
    out.push(bar_row(
        "energy",
        vm.energy,
        BAR_RAMP_GOOD,
        capability,
        styles,
    ));
    let xp_fraction = if vm.xp_target <= 0.0 {
        0.0
    } else {
        (vm.xp_current / vm.xp_target).clamp(0.0, 1.0)
    };
    out.push(bar_row(
        "xp",
        xp_fraction,
        BAR_RAMP_ACCENT,
        capability,
        styles,
    ));
    out
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

/// Lay out the watch frame at hybrid-v4 geometry: a 78-cell rounded
/// rounded box containing a two-column body (left = pet + vitals,
/// right = today + 7-day + feed + helpers). When the terminal is wider
/// than 78 cells, the surplus space pads evenly around the 76-cell
/// inner content area so the composed block stays centered.
fn render_wide(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &WatchViewModel,
    capability: ColorCapability,
    styles: &SemanticStyles,
) {
    let terminal_width = area.width as usize;
    // Frame fills the terminal; right column flexes to use any width above
    // the 78-cell baseline. Pet, vitals, and helpers stay at their natural
    // widths; only the data column on the right grows.
    let frame_inner_width = terminal_width.saturating_sub(2);
    let right_col = frame_inner_width
        .saturating_sub(LEFT_PAD + LEFT_COL + GUTTER + RIGHT_PAD)
        .max(RIGHT_COL);
    let pad_style = styles.body;

    // Allocate body height: total minus the two chrome rows.
    let body_height = (area.height as usize).saturating_sub(2);

    // Build right column panels. Feed grows first into available height,
    // then any residual is distributed as blank rows between sections.
    let today = render_today_panel_rows(right_col, vm, styles);
    let spark = render_sparkline_rows(
        right_col,
        &vm.recent_daily_effective_tokens,
        capability,
        styles,
    );
    let helpers = render_helpers_panel_rows(right_col, vm, styles);

    // Fixed overhead with single-blank separators:
    //   top_blank + today + blank + spark + blank + feed_rule + blank + helpers + bottom_blank
    let single_gap_overhead = 1 + today.len() + 1 + spark.len() + 1 + 1 + 1 + helpers.len() + 1;
    let event_count = vm.recent_events.len();
    let max_feed_entries = body_height
        .saturating_sub(single_gap_overhead)
        .max(2)
        .min(event_count.max(2));
    let feed = render_feed_panel_rows(right_col, vm, max_feed_entries, styles);

    let consumed = today.len() + spark.len() + feed.len() + helpers.len() + 5;
    let residual = body_height.saturating_sub(consumed);
    let slots = 5usize;
    let extra_per_slot = residual / slots;
    let extra_remainder = residual % slots;
    let slot_size = |slot_index: usize| -> usize {
        1 + extra_per_slot + if slot_index < extra_remainder { 1 } else { 0 }
    };
    let blank_run = |n: usize| -> Vec<Vec<Span<'_>>> { (0..n).map(|_| Vec::new()).collect() };

    let mut right_rows: Vec<Vec<Span>> = Vec::new();
    right_rows.extend(blank_run(slot_size(0)));
    right_rows.extend(today);
    right_rows.extend(blank_run(slot_size(1)));
    right_rows.extend(spark);
    right_rows.extend(blank_run(slot_size(2)));
    right_rows.extend(feed);
    right_rows.extend(blank_run(slot_size(3)));
    right_rows.extend(helpers);
    right_rows.extend(blank_run(slot_size(4)));

    // Left column: pet art and vitals, vertically centered.
    let raw_left = render_pet_panel_rows(LEFT_COL, vm, capability, styles);
    let left_rows: Vec<Vec<Span>> = if raw_left.len() < body_height {
        let pad_top = (body_height - raw_left.len()) / 2;
        let mut padded: Vec<Vec<Span>> = (0..pad_top).map(|_| Vec::new()).collect();
        padded.extend(raw_left);
        padded
    } else {
        raw_left
    };

    // Pad each column to its width, then join horizontally.
    let left_padded = pad_rows(left_rows, LEFT_COL, pad_style);
    let right_padded = pad_rows(right_rows, right_col, pad_style);
    let joined = join_horizontal_top(
        vec![(LEFT_COL, left_padded), (right_col, right_padded)],
        pad_style,
    );

    let body: Vec<Vec<Span>> = joined
        .into_iter()
        .map(|row| {
            let mut spans: Vec<Span> = Vec::with_capacity(row.len() + 3);
            spans.push(Span::raw(" ".repeat(LEFT_PAD)));
            // Insert the gutter between the two columns. Because pad_rows
            // already padded each column to its target width, the join
            // produced [LEFT_COL chars][right_col chars]. Split on LEFT_COL
            // so we can interleave a gutter.
            let (left_part, right_part) = split_after_width(row, LEFT_COL);
            spans.extend(left_part);
            spans.push(Span::raw(" ".repeat(GUTTER)));
            spans.extend(right_part);
            spans.push(Span::raw(" ".repeat(RIGHT_PAD)));
            spans
        })
        .collect();

    let age_label = format!("{}d", vm.age_days);
    let title = title_spans(
        &vm.pet_name,
        &vm.species,
        &vm.stage,
        &age_label,
        &vm.mood,
        styles,
    );
    let footer = footer_spans(styles);
    let chrome = ChromeStyle {
        frame: Style::default().fg(tokenpet_palette().accent.rgb),
        body_pad: pad_style,
    };
    let lines = box_with_chrome(body, frame_inner_width, title, footer, chrome);

    frame.render_widget(Paragraph::new(lines).style(styles.body), area);
}

/// Splits a row of spans into two row-vectors at the given visible char
/// boundary. The combined visible width is preserved exactly.
fn split_after_width<'a>(row: Vec<Span<'a>>, boundary: usize) -> (Vec<Span<'a>>, Vec<Span<'a>>) {
    let mut left: Vec<Span<'a>> = Vec::new();
    let mut right: Vec<Span<'a>> = Vec::new();
    let mut cursor = 0usize;
    for span in row {
        let len = span.content.chars().count();
        if cursor >= boundary {
            right.push(span);
            continue;
        }
        if cursor + len <= boundary {
            cursor += len;
            left.push(span);
        } else {
            let take = boundary - cursor;
            let left_part: String = span.content.chars().take(take).collect();
            let right_part: String = span.content.chars().skip(take).collect();
            left.push(Span::styled(left_part, span.style));
            right.push(Span::styled(right_part, span.style));
            cursor = boundary;
        }
    }
    (left, right)
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

    let pet = render_pet_panel_rows(width, vm, capability, styles);
    let today = render_today_panel_rows(width, vm, styles);
    let spark = render_sparkline_rows(width, &vm.recent_daily_effective_tokens, capability, styles);
    let feed = render_feed_panel_rows(width, vm, 3, styles);
    let helpers = render_helpers_panel_rows(width, vm, styles);

    let footer = Line::from(vec![
        Span::styled("q", styles.label),
        Span::styled(" quit  ", styles.label),
        Span::styled("r", styles.label),
        Span::styled(" refresh  ", styles.label),
        Span::styled("?", styles.label),
        Span::styled(" help", styles.label),
    ]);

    let groups: Vec<Vec<Vec<Span>>> = vec![pet, today, spark, feed, helpers];
    let mut all: Vec<Line> = Vec::new();
    for group in groups {
        if all.len() + group.len() + 1 > height.saturating_sub(1) {
            break;
        }
        for row in group {
            all.push(Line::from(row));
        }
        all.push(Line::from(Vec::<Span>::new()));
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
mod today_panel_tests {
    use super::*;

    #[test]
    fn today_panel_has_four_rows_plus_rule() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let rows = render_today_panel_rows(RIGHT_COL, &vm, &styles);
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn today_panel_renders_dash_for_absent_source() {
        let styles = semantic_styles();
        let mut vm = WatchViewModel::fixture();
        vm.source_breakdown.retain(|s| s.name != "codex");
        let rows = render_today_panel_rows(RIGHT_COL, &vm, &styles);
        let text: String = rows[3].iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("codex"));
        assert!(text.contains("—"));
    }
}

#[cfg(test)]
mod sparkline_tests {
    use super::*;

    #[test]
    fn sparkline_returns_two_rows() {
        let styles = semantic_styles();
        let history = vec![100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0];
        let rows = render_sparkline_rows(RIGHT_COL, &history, ColorCapability::Truecolor, &styles);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn sparkline_zeroes_render_dot() {
        let styles = semantic_styles();
        let history = vec![0.0; 7];
        let rows = render_sparkline_rows(RIGHT_COL, &history, ColorCapability::Truecolor, &styles);
        let text: String = rows[1].iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains('·'));
        assert!(!text.contains('█'));
    }
}

#[cfg(test)]
mod helpers_panel_tests {
    use super::*;
    use crate::tui::view_model::SourceStatus;

    #[test]
    fn helpers_panel_renders_check_when_ready() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let rows = render_helpers_panel_rows(RIGHT_COL, &vm, &styles);
        let text: String = rows[1].iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains('✓'));
    }

    #[test]
    fn helpers_panel_renders_x_when_blocked() {
        let styles = semantic_styles();
        let mut vm = WatchViewModel::fixture();
        for src in vm.source_health.iter_mut() {
            src.status = SourceStatus::Blocked;
        }
        let rows = render_helpers_panel_rows(RIGHT_COL, &vm, &styles);
        let text: String = rows[1].iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains('✗'));
    }
}

#[cfg(test)]
mod feed_panel_tests {
    use super::*;

    #[test]
    fn feed_panel_returns_rule_plus_up_to_three_entries() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture_with_events();
        let rows = render_feed_panel_rows(RIGHT_COL, &vm, 3, &styles);
        assert!(!rows.is_empty() && rows.len() <= 4);
    }
}

#[cfg(test)]
mod pet_panel_tests {
    use super::*;

    #[test]
    fn pet_panel_includes_vitals_rule_and_four_bars() {
        let styles = semantic_styles();
        let vm = WatchViewModel::fixture();
        let rows = render_pet_panel_rows(LEFT_COL, &vm, ColorCapability::Truecolor, &styles);
        let text: String = rows
            .iter()
            .flat_map(|row| row.iter().map(|s| s.content.as_ref()))
            .collect();
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
        let rows = render_pet_panel_rows(LEFT_COL, &vm, ColorCapability::Truecolor, &styles);
        let text: String = rows
            .iter()
            .flat_map(|row| row.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(!text.contains("species"));
        assert!(!text.contains("stage"));
        assert!(!text.contains("mood"));
    }
}

#[cfg(test)]
mod bar_row_tests {
    use super::*;

    #[test]
    fn bar_row_zero_fill_renders_twelve_faint() {
        let styles = semantic_styles();
        let spans = bar_row(
            "fed",
            0.0,
            BAR_RAMP_GOOD,
            ColorCapability::Truecolor,
            &styles,
        );
        let bar_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let fill_count = bar_text.chars().filter(|c| *c == '░').count();
        assert_eq!(fill_count, 12);
        let solid_count = bar_text.chars().filter(|c| *c == '█').count();
        assert_eq!(solid_count, 0);
    }

    #[test]
    fn bar_row_full_fill_renders_twelve_solid() {
        let styles = semantic_styles();
        let spans = bar_row(
            "fed",
            1.0,
            BAR_RAMP_GOOD,
            ColorCapability::Truecolor,
            &styles,
        );
        let bar_text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        let solid_count = bar_text.chars().filter(|c| *c == '█').count();
        assert_eq!(solid_count, 12);
    }

    #[test]
    fn bar_row_flat_capability_uses_solid_color() {
        let styles = semantic_styles();
        let spans = bar_row("fed", 0.5, BAR_RAMP_GOOD, ColorCapability::Flat, &styles);
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

#[cfg(test)]
mod render_compact_tests {
    use super::*;
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
        let row0: String = (0..60)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
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
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_buffer(width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn row_string(buf: &ratatui::buffer::Buffer, y: u16) -> String {
        let width = buf.area.width;
        (0..width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn render_wide_draws_rounded_frame_at_78_cols() {
        let buf = render_buffer(FRAME_BODY_WIDTH as u16, 30);
        let top = row_string(&buf, 0);
        let bottom = row_string(&buf, 29);
        assert!(
            top.starts_with("┏━"),
            "top row should start with ┏━, got {top:?}"
        );
        assert!(top.ends_with('┓'), "top row should end with ┓");
        assert!(bottom.starts_with("┗━"));
        assert!(bottom.ends_with('┛'));
        for y in 1..29 {
            assert_eq!(buf[(0u16, y)].symbol(), "┃", "row {y} left rail");
            assert_eq!(buf[(77u16, y)].symbol(), "┃", "row {y} right rail");
        }
    }

    #[test]
    fn render_wide_flexes_frame_to_fill_terminal() {
        // Frame fills the terminal at any width above the 78-col baseline;
        // the right column flexes to use the surplus. Pet column stays
        // fixed so the pet doesn't stretch.
        let buf = render_buffer(100, 30);
        assert_eq!(buf[(0u16, 0u16)].symbol(), "┏");
        assert_eq!(buf[(99u16, 0u16)].symbol(), "┓");
        assert_eq!(buf[(0u16, 29u16)].symbol(), "┗");
        assert_eq!(buf[(99u16, 29u16)].symbol(), "┛");
        for y in 1..29 {
            assert_eq!(buf[(0u16, y)].symbol(), "┃", "row {y} left rail broken");
            assert_eq!(buf[(99u16, y)].symbol(), "┃", "row {y} right rail broken");
        }
    }

    #[test]
    fn every_body_row_fills_terminal_width_exactly() {
        let buf = render_buffer(FRAME_BODY_WIDTH as u16, 30);
        for y in 0..30 {
            let row = row_string(&buf, y);
            assert_eq!(
                row.chars().count(),
                FRAME_BODY_WIDTH,
                "row {y} should be exactly {FRAME_BODY_WIDTH} cells; got {row:?}",
            );
        }
    }

    #[test]
    fn section_dividers_all_end_at_same_right_column() {
        // The right column section dividers (today, 7-day, feed, helpers)
        // should each end at the right edge of the right column (col 73)
        // in the 78-col frame. Inspect each dash run.
        let buf = render_buffer(FRAME_BODY_WIDTH as u16, 30);
        let mut divider_rows: Vec<u16> = Vec::new();
        for y in 0..30u16 {
            let row = row_string(&buf, y);
            if row.contains("─ today ")
                || row.contains("─ 7-day ")
                || row.contains("─ feed ")
                || row.contains("─ helpers ")
            {
                divider_rows.push(y);
            }
        }
        assert!(
            divider_rows.len() >= 3,
            "expected today/7-day/feed/helpers dividers, found at rows {divider_rows:?}",
        );
        for y in divider_rows {
            // The last '─' in the right column must sit at column 73.
            assert_eq!(
                buf[(73u16, y)].symbol(),
                "─",
                "row {y} last dash should be at column 73",
            );
            // And col 74 must be a space (the outer-pad gap before the rail).
            assert_eq!(
                buf[(74u16, y)].symbol(),
                " ",
                "row {y} should have padding after the right column ends",
            );
        }
    }

    #[test]
    fn pet_art_fits_inside_left_column_without_overflow() {
        let buf = render_buffer(FRAME_BODY_WIDTH as u16, 30);
        // Cells at the gutter (col 29 and 30) should be blank space when
        // the pet column's pet art would otherwise spill past col 28.
        for y in 1..29 {
            let gutter = buf[(29u16, y)].symbol();
            assert!(
                gutter == " " || gutter == "─",
                "row {y} gutter cell should be space/dash; got {gutter:?}",
            );
        }
    }
}
