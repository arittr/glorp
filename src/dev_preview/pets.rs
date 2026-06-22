use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::error::Result;
use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::{
    art::stage_label,
    generation::{generate_pet, Species},
    render::{render_pet, AnimationFrame, WorkAccent},
};
use crate::tui::panels::pet::pet_role_spans_for_line;
use crate::tui::style::{semantic_styles, ColorCapability, SemanticStyles};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

const FRAME_WIDTH: u16 = 120;
const COLUMN_WIDTH: u16 = FRAME_WIDTH / 6;
const HEADER_HEIGHT: u16 = 2;
const ROW_HEIGHT: u16 = 12;
const FRAME_HEIGHT: u16 = HEADER_HEIGHT + ROW_HEIGHT * 7;
const GLITCH_STATE_COLUMNS: u16 = 4;
const GLITCH_STATE_CELL_WIDTH: u16 = FRAME_WIDTH / GLITCH_STATE_COLUMNS;
const GLITCH_STATE_HEIGHT: u16 = 14;
const STAGES: [Stage; 7] = [
    Stage::S0,
    Stage::S1,
    Stage::S2,
    Stage::S3,
    Stage::S4,
    Stage::S5,
    Stage::S6,
];

const ADULT_STAGES: [Stage; 3] = [Stage::S4, Stage::S5, Stage::S6];
const TEXTURE_VARIANT_SEEDS: [&str; 3] = ["glorp-tex-a", "glorp-tex-b", "glorp-tex-c"];

const MOOD_SET: [(Mood, &str); 7] = [
    (Mood::Content, "content"),
    (Mood::Happy, "happy"),
    (Mood::Ecstatic, "ecstatic"),
    (Mood::Hungry, "hungry"),
    (Mood::Sad, "sad"),
    (Mood::Sleepy, "sleepy"),
    (Mood::Wilted, "wilted"),
];

pub fn pet_frames(ctx: &PreviewRenderContext) -> Result<Vec<PreviewFrame>> {
    Ok(vec![
        render_pet_matrix(
            ctx,
            "pet-species-stage",
            "Pet Species Stage",
            ColorCapability::Truecolor,
        ),
        render_pet_matrix(
            ctx,
            "pet-species-stage-flat",
            "Pet Species Stage (Flat)",
            ColorCapability::Flat,
        ),
        render_texture_variants(ctx),
        render_mood_set(ctx),
        render_glitch_live_states(ctx),
    ])
}

fn render_texture_variants(_ctx: &PreviewRenderContext) -> PreviewFrame {
    let styles = semantic_styles();
    // Layout: one row band per (species, adult stage); 3 variant columns.
    let row_height: u16 = 11;
    let band_count = (Species::all().len() * ADULT_STAGES.len()) as u16;
    let height = HEADER_HEIGHT + band_count * row_height;
    let col_width = FRAME_WIDTH / TEXTURE_VARIANT_SEEDS.len() as u16;
    let mut buffer = Buffer::empty(Rect::new(0, 0, FRAME_WIDTH, height));

    let mut band = 0u16;
    for species in Species::all() {
        for stage in ADULT_STAGES {
            for (col, seed) in TEXTURE_VARIANT_SEEDS.iter().enumerate() {
                let area = Rect::new(
                    col as u16 * col_width,
                    HEADER_HEIGHT + band * row_height,
                    col_width,
                    row_height,
                );
                render_seeded_pet_cell(area, &mut buffer, species, stage, seed, &styles);
            }
            band += 1;
        }
    }
    frame_from_buffer(
        "pet-texture-variants",
        "Pet Interior-Texture Variants",
        &buffer,
    )
}

fn render_seeded_pet_cell(
    area: Rect,
    buffer: &mut Buffer,
    species: Species,
    stage: Stage,
    seed: &str,
    styles: &SemanticStyles,
) {
    let pet = generate_pet(seed).with_species(species);
    let palette = crate::pet::palette::resolve_pet_palette(species, &pet.traits);
    let rendered = render_pet(
        &pet,
        stage,
        Mood::Content,
        AnimationFrame {
            tick: 0,
            blink_suppression_ticks: 0,
            hold_eyes_closed: false,
            blink_slowdown: 0,
            soft_eyes: false,
            work_accent: WorkAccent::None,
        },
    );
    let mut lines = vec![Line::styled(
        format!("{} s{} {}", species.as_str(), stage_index(stage), seed),
        styles.label,
    )];
    for (line_index, art_line) in rendered.lines.iter().enumerate() {
        let spans = pet_role_spans_for_line(
            art_line,
            line_index,
            &rendered.spans,
            styles,
            &palette,
            None,
        );
        lines.push(Line::from(spans));
    }
    Paragraph::new(lines).render(area, buffer);
}

fn render_mood_set(_ctx: &PreviewRenderContext) -> PreviewFrame {
    let styles = semantic_styles();
    let row_height: u16 = 11;
    let col_width = FRAME_WIDTH / MOOD_SET.len() as u16;
    let height = HEADER_HEIGHT + Species::all().len() as u16 * row_height;
    let mut buffer = Buffer::empty(Rect::new(0, 0, FRAME_WIDTH, height));

    for (mood_col, (_, label)) in MOOD_SET.iter().enumerate() {
        let area = Rect::new(mood_col as u16 * col_width, 0, col_width, HEADER_HEIGHT);
        Paragraph::new(Line::styled(*label, styles.section_header)).render(area, &mut buffer);
    }

    for (row, species) in Species::all().into_iter().enumerate() {
        let pet = generate_pet(&format!("glorp-mood-{}", species.as_str())).with_species(species);
        let palette = crate::pet::palette::resolve_pet_palette(species, &pet.traits);
        for (mood_col, (mood, _)) in MOOD_SET.iter().enumerate() {
            let area = Rect::new(
                mood_col as u16 * col_width,
                HEADER_HEIGHT + row as u16 * row_height,
                col_width,
                row_height,
            );
            let rendered = render_pet(
                &pet,
                Stage::S4,
                *mood,
                AnimationFrame {
                    tick: 1,
                    blink_suppression_ticks: 0,
                    hold_eyes_closed: false,
                    blink_slowdown: 0,
                    soft_eyes: false,
                    work_accent: WorkAccent::None,
                },
            );
            let mut lines = vec![Line::styled(species.as_str(), styles.label)];
            for (line_index, art_line) in rendered.lines.iter().enumerate() {
                let spans = pet_role_spans_for_line(
                    art_line,
                    line_index,
                    &rendered.spans,
                    &styles,
                    &palette,
                    None,
                );
                lines.push(Line::from(spans));
            }
            Paragraph::new(lines).render(area, &mut buffer);
        }
    }
    frame_from_buffer("pet-mood-set", "Pet Mood Set", &buffer)
}

fn render_pet_matrix(
    _ctx: &PreviewRenderContext,
    id: &str,
    title: &str,
    capability: ColorCapability,
) -> PreviewFrame {
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
            render_pet_cell(area, &mut buffer, species, stage, capability);
        }
    }

    frame_from_buffer(id, title, &buffer)
}

fn render_pet_cell(
    area: Rect,
    buffer: &mut Buffer,
    species: Species,
    stage: Stage,
    capability: ColorCapability,
) {
    let styles = semantic_styles();
    let pet = generate_pet(&format!("glorp-preview-{}", species.as_str())).with_species(species);
    let palette = crate::pet::palette::resolve_pet_palette(species, &pet.traits);
    let rendered = render_pet(
        &pet,
        stage,
        Mood::Content,
        AnimationFrame {
            tick: 0,
            blink_suppression_ticks: 0,
            hold_eyes_closed: false,
            blink_slowdown: 0,
            soft_eyes: false,
            work_accent: crate::pet::render::WorkAccent::None,
        },
    );

    let mut lines = vec![Line::styled(
        format!("s{} {}", stage_index(stage), stage_label(species, stage)),
        styles.label,
    )];
    for (line_index, art_line) in rendered.lines.iter().enumerate() {
        let spans = pet_role_spans_for_line(
            art_line,
            line_index,
            &rendered.spans,
            &styles,
            &palette,
            None,
        );
        lines.push(Line::from(adapt_spans_to_capability(spans, capability)));
    }

    Paragraph::new(lines).render(area, buffer);
}

#[derive(Clone, Copy)]
struct GlitchStateFixture {
    label: &'static str,
    stage: Stage,
    mood: Mood,
    work_accent: WorkAccent,
    tick: u64,
}

fn render_glitch_live_states(_ctx: &PreviewRenderContext) -> PreviewFrame {
    let fixtures = [
        GlitchStateFixture {
            label: "s4 content",
            stage: Stage::S4,
            mood: Mood::Content,
            work_accent: WorkAccent::None,
            tick: 1,
        },
        GlitchStateFixture {
            label: "s4 ecstatic",
            stage: Stage::S4,
            mood: Mood::Ecstatic,
            work_accent: WorkAccent::None,
            tick: 1,
        },
        GlitchStateFixture {
            label: "s4 ecstatic cache",
            stage: Stage::S4,
            mood: Mood::Ecstatic,
            work_accent: WorkAccent::Dreamy,
            tick: 1,
        },
        GlitchStateFixture {
            label: "s6 focused",
            stage: Stage::S6,
            mood: Mood::Happy,
            work_accent: WorkAccent::Focused,
            tick: 37,
        },
    ];
    let mut buffer = Buffer::empty(Rect::new(0, 0, FRAME_WIDTH, GLITCH_STATE_HEIGHT));

    for (index, fixture) in fixtures.into_iter().enumerate() {
        let area = Rect::new(
            index as u16 * GLITCH_STATE_CELL_WIDTH,
            0,
            GLITCH_STATE_CELL_WIDTH,
            GLITCH_STATE_HEIGHT,
        );
        render_glitch_state_cell(area, &mut buffer, fixture);
    }

    frame_from_buffer("pet-glitch-live-states", "Pet Glitch Live States", &buffer)
}

fn render_glitch_state_cell(area: Rect, buffer: &mut Buffer, fixture: GlitchStateFixture) {
    let styles = semantic_styles();
    let pet = generate_pet("glorp-preview-glitch-live").with_species(Species::Glitch);
    let palette = crate::pet::palette::resolve_pet_palette(Species::Glitch, &pet.traits);
    let rendered = render_pet(
        &pet,
        fixture.stage,
        fixture.mood,
        AnimationFrame {
            tick: fixture.tick,
            blink_suppression_ticks: 0,
            hold_eyes_closed: false,
            blink_slowdown: 0,
            soft_eyes: false,
            work_accent: fixture.work_accent,
        },
    );

    let mut lines = vec![Line::styled(fixture.label, styles.label)];
    for (line_index, art_line) in rendered.lines.iter().enumerate() {
        let spans = pet_role_spans_for_line(
            art_line,
            line_index,
            &rendered.spans,
            &styles,
            &palette,
            None,
        );
        lines.push(Line::from(adapt_spans_to_capability(
            spans,
            ColorCapability::Truecolor,
        )));
    }
    Paragraph::new(lines).render(area, buffer);
}

/// Truecolor keeps the resolved `Color::Rgb`; Flat maps each foreground to the
/// nearest xterm-256 indexed color, mirroring how a non-truecolor terminal
/// downgrades the palette while keeping species distinguishable.
fn adapt_spans_to_capability(spans: Vec<Span<'_>>, capability: ColorCapability) -> Vec<Span<'_>> {
    if matches!(capability, ColorCapability::Truecolor) {
        return spans;
    }
    spans
        .into_iter()
        .map(|span| {
            if let Some(Color::Rgb(r, g, b)) = span.style.fg {
                let style = span.style.fg(nearest_ansi_color(r, g, b));
                Span::styled(span.content, style)
            } else {
                span
            }
        })
        .collect()
}

/// Nearest xterm-256 indexed color to an sRGB triple, by squared distance.
fn nearest_ansi_color(r: u8, g: u8, b: u8) -> Color {
    let mut best_index = 0u8;
    let mut best_distance = u32::MAX;
    for index in 0u8..=255 {
        let (sr, sg, sb) = crate::dev_preview::frame::ansi_256_to_rgb(index);
        let dr = i32::from(r) - i32::from(sr);
        let dg = i32::from(g) - i32::from(sg);
        let db = i32::from(b) - i32::from(sb);
        let distance = (dr * dr + dg * dg + db * db) as u32;
        if distance < best_distance {
            best_distance = distance;
            best_index = index;
        }
    }
    Color::Indexed(best_index)
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

    #[test]
    fn pets_preview_includes_glitch_live_state_fixture() {
        let ctx = PreviewRenderContext::deterministic();
        let frames = pet_frames(&ctx).unwrap();
        let ids = frames
            .iter()
            .map(|frame| frame.id.as_str())
            .collect::<Vec<_>>();

        assert!(
            ids.contains(&"pet-glitch-live-states"),
            "pets preview should include a focused Glitch live-state fixture; got {ids:?}"
        );
    }

    #[test]
    fn pets_preview_includes_texture_variant_and_mood_set_frames() {
        let ctx = PreviewRenderContext::deterministic();
        let frames = pet_frames(&ctx).unwrap();
        let ids: Vec<&str> = frames.iter().map(|f| f.id.as_str()).collect();
        assert!(
            ids.contains(&"pet-texture-variants"),
            "pets preview must show per-seed interior-texture variants; got {ids:?}"
        );
        assert!(
            ids.contains(&"pet-mood-set"),
            "pets preview must show the full mood set; got {ids:?}"
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
