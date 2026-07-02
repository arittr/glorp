use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::pet::render::{PaletteRoleName, StyledSegment};
use crate::presentation::DrawCell;
use crate::tui::style::SemanticStyles;
use crate::tui::view_model::WatchViewModel;

use super::colors::palette_from_styles;
use super::pet_role_style;

/// Reference sparse writer kept as the byte-stability oracle for
/// [`pet_body_cells`]: it draws `lines` into `area`, writing only non-space
/// glyphs. Production renders through `pet_body_cells` + `blit_draw_list`
/// instead; this is retained solely so the equivalence test can prove the cell
/// list reproduces the legacy buffer byte-for-byte.
#[cfg(test)]
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

/// Cell-producing sibling of the legacy sparse writer. Walks `lines`
/// identically — skipping space glyphs, advancing one column per char, same
/// bounds guards — but pushes a [`DrawCell`] per non-space glyph instead of
/// writing to a [`Buffer`]. The fg is read from each span's style (always
/// `Color::Rgb` for pet spans; any other variant falls back to `None`), and
/// `bold` carries the eye's `Modifier::BOLD`. `bg` is left `None` so the
/// sparse-pet contract holds: whatever the habitat wrote underneath survives.
pub(super) fn pet_body_cells(area: Rect, lines: &[Line<'_>]) -> Vec<DrawCell> {
    let right = area.x.saturating_add(area.width);
    let bottom = area.y.saturating_add(area.height);
    let mut cells = Vec::new();
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
            let fg = match span.style.fg {
                Some(Color::Rgb(r, g, b)) => Some(crate::pet::palette::Rgb::new(r, g, b)),
                _ => None,
            };
            let bold = span.style.add_modifier.contains(Modifier::BOLD);
            for ch in span.content.chars() {
                if x >= right {
                    break;
                }
                if ch != ' ' {
                    cells.push(DrawCell {
                        row: y,
                        col: x,
                        glyph: Some(ch.to_string()),
                        fg,
                        bg: None,
                        bold,
                    });
                }
                x = x.saturating_add(1);
            }
        }
    }
    cells
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
/// Both eyes track together (`glyph` at both ends). At the standard 3-wide eye
/// slot we keep a small `.` bridge (`<.<`) so the nose doesn't vanish while the
/// pet looks around; shorter spans render just the glyph, longer spans pad.
pub(super) fn build_cursor_eye_string(glyph: char, span_width: usize) -> String {
    match span_width {
        0 => String::new(),
        1 | 2 => glyph.to_string(),
        3 => format!("{glyph}.{glyph}"),
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
        '\u{259B}' => '\u{259C}', // ▛ <-> ▜
        '\u{259C}' => '\u{259B}',
        '\u{2599}' => '\u{259F}', // ▙ <-> ▟
        '\u{259F}' => '\u{2599}',
        '\u{258C}' => '\u{2590}', // ▌ <-> ▐
        '\u{2590}' => '\u{258C}',
        '\u{2596}' => '\u{2597}', // ▖ <-> ▗
        '\u{2597}' => '\u{2596}',
        '\u{2598}' => '\u{259D}', // ▘ <-> ▝
        '\u{259D}' => '\u{2598}',
        '\u{259A}' => '\u{259E}', // ▚ <-> ▞
        '\u{259E}' => '\u{259A}',
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

#[cfg(test)]
mod tests {
    use super::super::blit::blit_draw_list;
    use super::*;
    use crate::presentation::SceneDrawList;
    use ratatui::style::Style;

    fn styled_lines() -> Vec<Line<'static>> {
        // A BOLD green "eye" span and a non-bold cream "body" span containing a
        // space (skipped). Mirrors how real pet spans look: fg always Rgb, bg
        // unset, BOLD only on the eye. Two rows exercise the row advance.
        vec![
            Line::from(vec![
                Span::styled(
                    "o",
                    Style::default()
                        .fg(Color::Rgb(0x82, 0xbc, 0x83))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("a b", Style::default().fg(Color::Rgb(0xef, 0xeb, 0xe4))),
            ]),
            Line::from(vec![Span::styled(
                "X",
                Style::default().fg(Color::Rgb(0x10, 0x20, 0x30)),
            )]),
        ]
    }

    #[test]
    fn mirror_line_swaps_block_quadrant_pairs() {
        // Block-built silhouettes must flip glyph handedness when the pet turns,
        // not merely reverse position. Reverse + swap: "▛▙▌▖▘▚" -> "▞▝▗▐▟▜".
        assert_eq!(mirror_line("▛▙▌▖▘▚"), "▞▝▗▐▟▜");
        // Each pair is an involution, so mirroring twice is identity.
        let s = "▟▒▒▙▐░▌";
        assert_eq!(mirror_line(&mirror_line(s)), s);
    }

    #[test]
    fn pet_body_cells_skips_spaces_and_maps_style() {
        let area = Rect::new(3, 7, 13, 10);
        let cells = pet_body_cells(area, &styled_lines());

        // Row 0: "o" at col 3 (BOLD eye, green), "a" at col 4, space skipped,
        // "b" at col 6. Row 1: "X" at col 3.
        assert_eq!(
            cells,
            vec![
                DrawCell {
                    row: 7,
                    col: 3,
                    glyph: Some("o".to_string()),
                    fg: Some(crate::pet::palette::Rgb::new(0x82, 0xbc, 0x83)),
                    bg: None,
                    bold: true,
                },
                DrawCell {
                    row: 7,
                    col: 4,
                    glyph: Some("a".to_string()),
                    fg: Some(crate::pet::palette::Rgb::new(0xef, 0xeb, 0xe4)),
                    bg: None,
                    bold: false,
                },
                DrawCell {
                    row: 7,
                    col: 6,
                    glyph: Some("b".to_string()),
                    fg: Some(crate::pet::palette::Rgb::new(0xef, 0xeb, 0xe4)),
                    bg: None,
                    bold: false,
                },
                DrawCell {
                    row: 8,
                    col: 3,
                    glyph: Some("X".to_string()),
                    fg: Some(crate::pet::palette::Rgb::new(0x10, 0x20, 0x30)),
                    bg: None,
                    bold: false,
                },
            ]
        );
    }

    #[test]
    fn pet_body_cells_falls_back_to_no_fg_for_non_rgb_span() {
        // Pet spans are always Rgb in practice; if a non-Rgb fg ever appears,
        // the documented fallback is fg: None (leave existing fg intact).
        let area = Rect::new(0, 0, 5, 1);
        let lines = vec![Line::from(vec![Span::styled(
            "z",
            Style::default().fg(Color::Indexed(5)),
        )])];
        let cells = pet_body_cells(area, &lines);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].fg, None);
    }

    #[test]
    fn pet_body_cells_blit_matches_sparse_writer_byte_for_byte() {
        // The cell list, once blitted, must produce a buffer byte-identical to
        // the legacy sparse writer — same symbols, same fg, same BOLD eye, same
        // untouched-space cells. This is the local byte-stability guard.
        let area = Rect::new(2, 1, 13, 10);
        let lines = styled_lines();

        let mut sparse_buf = Buffer::empty(Rect::new(0, 0, 20, 14));
        render_pet_lines_sparse(&mut sparse_buf, area, &lines);

        let mut blit_buf = Buffer::empty(Rect::new(0, 0, 20, 14));
        blit_draw_list(
            &mut blit_buf,
            &SceneDrawList { cells: pet_body_cells(area, &lines) },
        );

        assert_eq!(
            sparse_buf, blit_buf,
            "pet_body_cells + blit must equal the legacy sparse writer"
        );
    }
}
