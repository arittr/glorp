use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::pet::animator::low_energy_lightness_multiplier;
use crate::pet::render::PaletteRoleName;
use crate::tui::panels::Panel;
use crate::tui::style::{semantic_styles, SemanticStyles};
use crate::tui::view_model::WatchViewModel;

pub struct PetPanel;

impl Panel for PetPanel {
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint {
        let line_count = (vm.pet_art.len() as u16).max(2);
        Constraint::Length(line_count)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel) {
        let base = semantic_styles();
        // Continuous low-energy droop: darken pet foreground colors based on
        // current energy. At energy >= 0.6 the multiplier is 1.0 (no change);
        // it falls linearly to 0.55 at energy 0.0.
        let m = low_energy_lightness_multiplier(vm.energy);
        let droop = darken_pet_styles(&base, m);
        let cursor_norm_x = cursor_normalized_x_within(vm, area);
        let lines = build_pet_lines(vm, area.width as usize, &droop, cursor_norm_x);
        Paragraph::new(lines).render(area, buf);
    }
}

/// Returns a copy of `base` with all pet-role foreground colors scaled by
/// `multiplier` (1.0 = unchanged, 0.55 = ~half lightness). Non-RGB colors
/// pass through unchanged.
fn darken_pet_styles(base: &SemanticStyles, multiplier: f32) -> SemanticStyles {
    let mut s = base.clone();
    s.pet_body = darken_style(s.pet_body, multiplier);
    s.pet_eye = darken_style(s.pet_eye, multiplier);
    s.pet_mouth = darken_style(s.pet_mouth, multiplier);
    s.pet_accent = darken_style(s.pet_accent, multiplier);
    s.pet_pattern = darken_style(s.pet_pattern, multiplier);
    s
}

fn darken_style(style: Style, multiplier: f32) -> Style {
    if let Some(Color::Rgb(r, g, b)) = style.fg {
        let m = multiplier.clamp(0.0, 1.0);
        let r = (r as f32 * m) as u8;
        let g = (g as f32 * m) as u8;
        let b = (b as f32 * m) as u8;
        style.fg(Color::Rgb(r, g, b))
    } else {
        style
    }
}

/// Hit-test the screen cursor against the pet panel rect. Returns normalized
/// x ∈ [-1.0, 1.0] relative to the panel center, or None when the cursor is
/// outside the rect, missing, or mouse tracking is disabled.
fn cursor_normalized_x_within(vm: &WatchViewModel, area: Rect) -> Option<f32> {
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
fn cursor_eye_glyph(norm_x: f32) -> char {
    if norm_x < -0.33 {
        '<'
    } else if norm_x > 0.33 {
        '>'
    } else {
        'o'
    }
}

fn build_pet_lines<'a>(
    vm: &'a WatchViewModel,
    area_width: usize,
    styles: &'a SemanticStyles,
    cursor_norm_x: Option<f32>,
) -> Vec<Line<'a>> {
    let pet_width = vm
        .pet_art
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let left_pad = area_width.saturating_sub(pet_width) / 2;
    let cursor_eye = cursor_norm_x.map(cursor_eye_glyph);

    vm.pet_art
        .iter()
        .enumerate()
        .map(|(line_index, art_line)| {
            let mut spans: Vec<Span<'a>> = Vec::new();
            if left_pad > 0 {
                spans.push(Span::raw(" ".repeat(left_pad)));
            }
            // Cursor-tracked eyes only swap on line 0 where the eye glyphs live.
            let eye_override = if line_index == 0 { cursor_eye } else { None };
            spans.extend(role_spans_for_line(
                art_line,
                line_index,
                &vm.pet_spans,
                styles,
                eye_override,
            ));
            Line::from(spans)
        })
        .collect()
}

fn role_spans_for_line<'a>(
    art_line: &'a str,
    line_index: usize,
    pet_spans: &'a [crate::pet::render::StyledSegment],
    styles: &'a SemanticStyles,
    eye_override: Option<char>,
) -> Vec<Span<'a>> {
    let total_chars = art_line.chars().count();
    if total_chars == 0 {
        return Vec::new();
    }

    let mut segments: Vec<&crate::pet::render::StyledSegment> = pet_spans
        .iter()
        .filter(|s| s.line == line_index && s.start < s.end && s.start < total_chars)
        .collect();
    segments.sort_by_key(|s| s.start);

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
        let style = role_style(segment.role, styles);
        if let (Some(glyph), crate::pet::render::PaletteRoleName::Eye) =
            (eye_override, segment.role)
        {
            spans.push(Span::styled(glyph.to_string(), style));
        } else {
            let value = char_slice(art_line, &char_indices, start, end);
            spans.push(Span::styled(value, style));
        }
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

fn role_style(role: PaletteRoleName, styles: &SemanticStyles) -> ratatui::style::Style {
    match role {
        PaletteRoleName::Body => styles.pet_body,
        PaletteRoleName::Eye => styles.pet_eye,
        PaletteRoleName::Mouth => styles.pet_mouth,
        PaletteRoleName::Accent => styles.pet_accent,
        PaletteRoleName::Pattern => styles.pet_pattern,
        PaletteRoleName::Particle => styles.pet_accent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn vm_with_real_pet() -> WatchViewModel {
        use crate::game::evolution::Stage;
        use crate::game::metabolism::Mood;
        use crate::pet::generation::generate_pet;
        use crate::pet::render::{render_pet, AnimationFrame};

        let pet = generate_pet("pet-panel-test-seed");
        let rendered = render_pet(
            &pet,
            Stage::S2,
            Mood::Content,
            AnimationFrame {
                tick: 0,
                blink_suppression_ticks: 0,
            },
        );
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    #[test]
    fn pet_panel_renders_some_braille_into_area() {
        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            s.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))),
            "pet panel should render at least one braille char into the area"
        );
    }

    #[test]
    fn pet_panel_centers_narrow_art_in_wide_area() {
        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        // The first cell of row 0 should be a space (left-pad), not pet content,
        // because the art is narrower than 80 columns.
        let first_cell = buf[(0u16, 0u16)].symbol();
        assert_eq!(
            first_cell, " ",
            "expected left-pad space, got {first_cell:?}"
        );
    }

    #[test]
    fn cursor_normalized_x_is_none_when_cursor_outside_area() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((100, 100));
        let area = Rect::new(0, 0, 40, 5);
        assert!(cursor_normalized_x_within(&vm, area).is_none());
    }

    #[test]
    fn cursor_normalized_x_maps_left_edge_to_negative_one() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((0, 0));
        let area = Rect::new(0, 0, 40, 5);
        let n = cursor_normalized_x_within(&vm, area).unwrap();
        assert!(n <= -0.95, "left edge should be ~-1.0, got {n}");
    }

    #[test]
    fn cursor_normalized_x_maps_right_edge_to_near_positive_one() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((39, 0));
        let area = Rect::new(0, 0, 40, 5);
        let n = cursor_normalized_x_within(&vm, area).unwrap();
        assert!(n > 0.9, "right edge should be ~+1.0, got {n}");
    }

    #[test]
    fn cursor_normalized_x_disabled_when_tracking_off() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((20, 2));
        vm.mouse_tracking_enabled = false;
        let area = Rect::new(0, 0, 40, 5);
        assert!(cursor_normalized_x_within(&vm, area).is_none());
    }

    #[test]
    fn cursor_eye_glyph_picks_directional_chars() {
        assert_eq!(cursor_eye_glyph(-0.9), '<');
        assert_eq!(cursor_eye_glyph(0.0), 'o');
        assert_eq!(cursor_eye_glyph(0.9), '>');
    }

    #[test]
    fn pet_panel_swaps_eye_glyph_when_cursor_inside() {
        let mut vm = vm_with_real_pet();
        // Place cursor at right side; expect '>' glyph to appear on line 0.
        vm.cursor_screen = Some((38, 0));
        let panel = PetPanel;
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let row0: String = (0..40)
            .map(|x| buf[(x, 0u16)].symbol().to_string())
            .collect();
        assert!(
            row0.contains('>'),
            "expected '>' eye glyph in row 0, got {row0:?}"
        );
    }
}
