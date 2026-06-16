use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::pet::render::{PaletteRoleName, StyledSegment};
use crate::tui::style::SemanticStyles;
use crate::tui::view_model::WatchViewModel;

use super::colors::palette_from_styles;
use super::pet_role_style;

/// Draws `lines` into `area`, writing only non-space glyphs. Whitespace cells
/// pass through, leaving whatever the habitat / props passes wrote underneath
/// visible — so the pet's bounding rectangle no longer occludes the backdrop.
pub(super) fn render_pet_lines_sparse(buf: &mut Buffer, area: Rect, lines: &[Line<'_>]) {
    let right = area.x.saturating_add(area.width);
    let bottom = area.y.saturating_add(area.height);
    for (row_idx, line) in lines.iter().enumerate() {
        let y = area.y.saturating_add(row_idx as u16);
        if y >= bottom {
            break;
        }
        let mut x = area.x;
        for span in &line.spans {
            if x >= right {
                break;
            }
            for ch in span.content.chars() {
                if x >= right {
                    break;
                }
                if ch != ' ' {
                    let cell = &mut buf[(x, y)];
                    cell.set_char(ch);
                    cell.set_style(span.style);
                }
                x = x.saturating_add(1);
            }
        }
    }
}

/// Render a small speech bubble: "« text »" centered above the pet, styled
/// with the accent color so it pops without being shouty.
pub(super) fn render_speech_bubble(
    area: Rect,
    buf: &mut Buffer,
    text: &str,
    styles: &SemanticStyles,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let bubble = format!("« {text} »");
    let bubble_width = bubble.chars().count() as u16;
    let pad = (area.width.saturating_sub(bubble_width)) / 2;
    let line = Line::from(vec![
        Span::raw(" ".repeat(pad as usize)),
        Span::styled(bubble, styles.pet_accent),
    ]);
    Paragraph::new(line).render(area, buf);
}

/// Hit-test the screen cursor against the pet panel rect. Returns normalized
/// x ∈ [-1.0, 1.0] relative to the panel center, or None when the pet is asleep, the cursor is
/// outside the rect, missing, or mouse tracking is disabled.
pub(super) fn cursor_normalized_x_within(vm: &WatchViewModel, area: Rect) -> Option<f32> {
    if vm.day_context.asleep {
        return None;
    }
    if !vm.mouse_tracking_enabled {
        return None;
    }
    let (cx, cy) = vm.cursor_screen?;
    if cx < area.x || cx >= area.x + area.width || cy < area.y || cy >= area.y + area.height {
        return None;
    }
    let local_x = (cx - area.x) as f32;
    let width = area.width.max(1) as f32;
    Some((local_x / width) * 2.0 - 1.0)
}

/// Pick the cursor-tracked eye glyph based on normalized x position.
/// Left third → looking left; middle → straight; right third → looking right.
pub(super) fn cursor_eye_glyph(norm_x: f32) -> char {
    if norm_x < -0.33 {
        '<'
    } else if norm_x > 0.33 {
        '>'
    } else {
        'o'
    }
}

/// Build a replacement eye string that matches the original eye span's width.
/// For span widths >= 3 ("o o" / "^ ^" style) we render `glyph` at both ends
/// with a single space in between — both eyes track together. For shorter
/// spans we render just the glyph. For longer spans we pad with spaces.
pub(super) fn build_cursor_eye_string(glyph: char, span_width: usize) -> String {
    match span_width {
        0 => String::new(),
        1 | 2 => glyph.to_string(),
        n => {
            let mut s = String::with_capacity(n);
            s.push(glyph);
            for _ in 0..(n - 2) {
                s.push(' ');
            }
            s.push(glyph);
            s
        }
    }
}

pub(super) fn build_pet_lines(
    vm: &WatchViewModel,
    area_width: usize,
    styles: &SemanticStyles,
    cursor_norm_x: Option<f32>,
    twinkle: Option<crate::pet::animator::TwinkleSpec>,
) -> Vec<Line<'static>> {
    let mirror = vm.facing == -1;

    // Build the (possibly mirrored) art lines and spans as owned Strings.
    let (art_lines, art_spans): (Vec<String>, Vec<StyledSegment>) = if mirror {
        let mirrored_lines: Vec<String> = vm.pet_art.iter().map(|l| mirror_line(l)).collect();
        let mirrored_spans = mirror_spans(&vm.pet_spans, &mirrored_lines);
        (mirrored_lines, mirrored_spans)
    } else {
        (vm.pet_art.clone(), vm.pet_spans.clone())
    };

    let pet_width = art_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    // scene.pet_art is already positioned at the wander offset by
    // pet_inner_rect_in_panel, so the lines themselves only need to center
    // within their own narrow rect.
    let center_pad = area_width.saturating_sub(pet_width) / 2;
    let left_pad = center_pad;
    let cursor_eye = cursor_norm_x.map(cursor_eye_glyph);

    art_lines
        .into_iter()
        .enumerate()
        .map(|(line_index, art_line)| {
            // Apply twinkle: if this line/col matches, substitute the glyph.
            // The framed art is 13×10 so art_line is a frame line.
            // twinkle.row is 0-based within the 11×8 art grid; frame adds 1 to row.
            let twinkle_col = twinkle.and_then(|t| {
                if t.row as usize + 1 == line_index {
                    Some((t.col as usize + 1, t.glyph))
                } else {
                    None
                }
            });

            let mut spans: Vec<Span<'static>> = Vec::new();
            if left_pad > 0 {
                spans.push(Span::raw(" ".repeat(left_pad)));
            }
            let eye_override = cursor_eye;
            let palette = palette_from_styles(styles);
            spans.extend(build_owned_spans_for_line(
                &art_line,
                line_index,
                &art_spans,
                styles,
                &palette,
                eye_override,
                twinkle_col,
            ));
            Line::from(spans)
        })
        .collect()
}

/// Mirrors an art line: reverses characters and substitutes directional glyphs.
pub(crate) fn mirror_line(line: &str) -> String {
    line.chars().rev().map(mirror_char).collect()
}

fn mirror_char(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '/' => '\\',
        '\\' => '/',
        '<' => '>',
        '>' => '<',
        'd' => 'b',
        'b' => 'd',
        '{' => '}',
        '}' => '{',
        '[' => ']',
        ']' => '[',
        _ => c,
    }
}

/// Re-build StyledSegments for mirrored lines by mirroring each span's
/// start/end positions within its line.
fn mirror_spans(spans: &[StyledSegment], mirrored_lines: &[String]) -> Vec<StyledSegment> {
    spans
        .iter()
        .map(|seg| {
            let line_len = mirrored_lines
                .get(seg.line)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            let span_width = seg.end.saturating_sub(seg.start);
            // Mirror: new_start = line_len - seg.end, new_end = line_len - seg.start
            let new_start = line_len.saturating_sub(seg.end);
            let new_end = new_start + span_width;
            StyledSegment {
                line: seg.line,
                start: new_start,
                end: new_end,
                role: seg.role,
            }
        })
        .collect()
}

/// Build owned `Vec<Span<'static>>` for one art line, applying eye override
/// and optional twinkle glyph injection.
fn build_owned_spans_for_line(
    art_line: &str,
    line_index: usize,
    pet_spans: &[StyledSegment],
    styles: &SemanticStyles,
    palette: &crate::pet::palette::ResolvedPalette,
    eye_override: Option<char>,
    twinkle_col: Option<(usize, char)>,
) -> Vec<Span<'static>> {
    let total_chars = art_line.chars().count();
    if total_chars == 0 {
        return Vec::new();
    }

    let mut segments: Vec<&StyledSegment> = pet_spans
        .iter()
        .filter(|s| s.line == line_index && s.start < s.end && s.start < total_chars)
        .collect();
    segments.sort_by_key(|s| s.start);

    let char_indices = char_byte_indices(art_line);

    if segments.is_empty() {
        let body = char_slice(art_line, &char_indices, 0, total_chars).to_string();
        let body = apply_twinkle_in_range(body, 0, total_chars, twinkle_col);
        return vec![Span::styled(body, styles.pet_body)];
    }

    // Build owned spans. Each "slot" is: optional body-gap, then the styled segment.
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut cursor = 0usize;

    for segment in &segments {
        let start = segment.start.max(cursor).min(total_chars);
        let end = segment.end.min(total_chars);
        if end <= cursor {
            continue;
        }
        if start > cursor {
            let body_text = char_slice(art_line, &char_indices, cursor, start).to_string();
            let body_text = apply_twinkle_in_range(body_text, cursor, start, twinkle_col);
            spans.push(Span::styled(body_text, styles.pet_body));
        }
        let style = pet_role_style(segment.role, palette);
        let value = if let (Some(glyph), PaletteRoleName::Eye) = (eye_override, segment.role) {
            let span_width = end - start;
            build_cursor_eye_string(glyph, span_width)
        } else {
            char_slice(art_line, &char_indices, start, end).to_string()
        };
        let value = apply_twinkle_in_range(value, start, end, twinkle_col);
        spans.push(Span::styled(value, style));
        cursor = end;
    }
    if cursor < total_chars {
        let tail = char_slice(art_line, &char_indices, cursor, total_chars).to_string();
        let tail = apply_twinkle_in_range(tail, cursor, total_chars, twinkle_col);
        spans.push(Span::styled(tail, styles.pet_body));
    }

    spans
}

/// If `twinkle_col` falls within `[start, end)`, substitute that character in
/// `text` with the twinkle glyph. Otherwise returns `text` unchanged.
fn apply_twinkle_in_range(
    text: String,
    start: usize,
    end: usize,
    twinkle_col: Option<(usize, char)>,
) -> String {
    let Some((col, glyph)) = twinkle_col else {
        return text;
    };
    if col < start || col >= end {
        return text;
    }
    let local = col - start;
    let mut chars: Vec<char> = text.chars().collect();
    if local < chars.len() {
        chars[local] = glyph;
    }
    chars.into_iter().collect()
}

// Compatibility wrapper for watch and Preview Lab callers; the shared
// presentation role lookup owns the domain semantics.
pub(crate) fn pet_role_spans_for_line<'a>(
    art_line: &'a str,
    line_index: usize,
    pet_spans: &'a [StyledSegment],
    styles: &'a SemanticStyles,
    palette: &'a crate::pet::palette::ResolvedPalette,
    eye_override: Option<char>,
) -> Vec<Span<'a>> {
    let _ = styles;
    let total_chars = art_line.chars().count();
    if total_chars == 0 {
        return Vec::new();
    }

    let mut segments: Vec<&StyledSegment> = pet_spans
        .iter()
        .filter(|s| s.line == line_index && s.start < s.end && s.start < total_chars)
        .collect();
    segments.sort_by_key(|s| s.start);

    if segments.is_empty() {
        return vec![Span::styled(
            art_line,
            pet_role_style(PaletteRoleName::Body, palette),
        )];
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
            spans.push(Span::styled(
                body,
                pet_role_style(PaletteRoleName::Body, palette),
            ));
        }
        let style = pet_role_style(segment.role, palette);
        if let (Some(glyph), PaletteRoleName::Eye) = (eye_override, segment.role) {
            // Authored eye slots are typically 3+ chars wide ("o o", "^ ^",
            // "v v" etc.). Preserve the original span width so the right
            // eye doesn't disappear — place the cursor glyph at both ends
            // of the span with the existing inner characters between them.
            let span_width = end - start;
            let replaced = build_cursor_eye_string(glyph, span_width);
            spans.push(Span::styled(replaced, style));
        } else {
            let value = char_slice(art_line, &char_indices, start, end);
            spans.push(Span::styled(value, style));
        }
        cursor = end;
    }

    if cursor < total_chars {
        let tail = char_slice(art_line, &char_indices, cursor, total_chars);
        spans.push(Span::styled(
            tail,
            pet_role_style(PaletteRoleName::Body, palette),
        ));
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
