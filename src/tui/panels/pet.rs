use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::tui::panels::Panel;
use crate::tui::style::{semantic_styles, SemanticStyles};
use crate::tui::view_model::WatchViewModel;
use crate::pet::render::PaletteRoleName;

pub struct PetPanel;

impl Panel for PetPanel {
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint {
        let line_count = (vm.pet_art.len() as u16).max(2);
        Constraint::Length(line_count)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel) {
        let styles = semantic_styles();
        let lines = build_pet_lines(vm, area.width as usize, &styles);
        Paragraph::new(lines).render(area, buf);
    }
}

fn build_pet_lines<'a>(
    vm: &'a WatchViewModel,
    area_width: usize,
    styles: &'a SemanticStyles,
) -> Vec<Line<'a>> {
    let pet_width = vm
        .pet_art
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let left_pad = area_width.saturating_sub(pet_width) / 2;

    vm.pet_art
        .iter()
        .enumerate()
        .map(|(line_index, art_line)| {
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
            Line::from(spans)
        })
        .collect()
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
            AnimationFrame { tick: 0, blink_suppression_ticks: 0 },
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
        let s: String = buf.content().iter().map(|c| c.symbol().to_string()).collect();
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
        assert_eq!(first_cell, " ", "expected left-pad space, got {first_cell:?}");
    }
}
