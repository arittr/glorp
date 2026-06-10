use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::error::Result;
use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::{
    art::stage_label,
    generation::{generate_pet, Species},
    render::{render_pet, AnimationFrame},
};
use crate::tui::panels::pet::pet_role_spans_for_line;
use crate::tui::style::semantic_styles;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

const FRAME_WIDTH: u16 = 120;
const COLUMN_WIDTH: u16 = FRAME_WIDTH / 6;
const HEADER_HEIGHT: u16 = 2;
const ROW_HEIGHT: u16 = 12;
const FRAME_HEIGHT: u16 = HEADER_HEIGHT + ROW_HEIGHT * 7;
const STAGES: [Stage; 7] = [
    Stage::S0,
    Stage::S1,
    Stage::S2,
    Stage::S3,
    Stage::S4,
    Stage::S5,
    Stage::S6,
];

pub fn pet_frames(ctx: &PreviewRenderContext) -> Result<Vec<PreviewFrame>> {
    Ok(vec![render_pet_matrix(ctx)])
}

fn render_pet_matrix(ctx: &PreviewRenderContext) -> PreviewFrame {
    let _capability = ctx.render.color_capability;
    let styles = semantic_styles();
    let mut buffer = Buffer::empty(Rect::new(0, 0, FRAME_WIDTH, FRAME_HEIGHT));

    for (column, species) in Species::all().into_iter().enumerate() {
        let area = Rect::new(column as u16 * COLUMN_WIDTH, 0, COLUMN_WIDTH, HEADER_HEIGHT);
        Paragraph::new(Line::styled(species.as_str(), styles.section_header))
            .render(area, &mut buffer);
    }

    for (row, stage) in STAGES.into_iter().enumerate() {
        for (column, species) in Species::all().into_iter().enumerate() {
            let area = Rect::new(
                column as u16 * COLUMN_WIDTH,
                HEADER_HEIGHT + row as u16 * ROW_HEIGHT,
                COLUMN_WIDTH,
                ROW_HEIGHT,
            );
            render_pet_cell(area, &mut buffer, species, stage);
        }
    }

    frame_from_buffer("pet-species-stage", "Pet Species Stage", &buffer)
}

fn render_pet_cell(area: Rect, buffer: &mut Buffer, species: Species, stage: Stage) {
    let styles = semantic_styles();
    let pet = generate_pet(&format!("glorp-preview-{}", species.as_str())).with_species(species);
    let rendered = render_pet(
        &pet,
        stage,
        Mood::Content,
        AnimationFrame {
            tick: 0,
            blink_suppression_ticks: 0,
            hold_eyes_closed: false,
        },
    );

    let mut lines = vec![Line::styled(
        format!("s{} {}", stage_index(stage), stage_label(species, stage)),
        styles.label,
    )];
    for (line_index, art_line) in rendered.lines.iter().enumerate() {
        lines.push(Line::from(pet_role_spans_for_line(
            art_line,
            line_index,
            &rendered.spans,
            &styles,
            None,
        )));
    }

    Paragraph::new(lines).render(area, buffer);
}

#[cfg(test)]
fn species_stage_cells() -> Vec<(Species, Stage)> {
    STAGES
        .into_iter()
        .flat_map(|stage| {
            Species::all()
                .into_iter()
                .map(move |species| (species, stage))
        })
        .collect()
}

fn stage_index(stage: Stage) -> usize {
    match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 => 2,
        Stage::S3 => 3,
        Stage::S4 => 4,
        Stage::S5 => 5,
        Stage::S6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pet_matrix_contains_all_species_names() {
        let ctx = PreviewRenderContext::deterministic();
        let frame = pet_frames(&ctx).unwrap().remove(0);
        let text = frame_text(&frame);

        for species in Species::all() {
            assert!(text.contains(species.as_str()), "missing {species:?}");
        }
    }

    #[test]
    fn pet_matrix_contains_all_stage_labels() {
        let ctx = PreviewRenderContext::deterministic();
        let frame = pet_frames(&ctx).unwrap().remove(0);
        let text = frame_text(&frame);

        for (species, stage) in species_stage_cells() {
            let label = stage_label(species, stage);
            assert!(
                text.contains(label),
                "missing {species:?} {stage:?} {label}"
            );
        }
    }

    #[test]
    fn pet_matrix_uses_expected_species_stage_count() {
        assert_eq!(
            species_stage_cells().len(),
            Species::all().len() * STAGES.len()
        );
    }

    fn frame_text(frame: &PreviewFrame) -> String {
        let mut text = String::new();
        for y in 0..frame.height {
            for cell in frame.cells.iter().filter(|cell| cell.y == y) {
                if !cell.continuation {
                    text.push_str(&cell.symbol);
                }
            }
            text.push('\n');
        }
        text
    }
}
