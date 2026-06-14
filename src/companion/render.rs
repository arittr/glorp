use crate::round::layout::{RoundAnchorKind, RoundSceneLayout};
use crate::round::model::RoundSceneModel;
use ratatui::text::Line;

#[derive(Debug, Clone, PartialEq)]
pub struct RoundDrawCommand {
    pub kind: RoundDrawKind,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub label: Option<char>,
    pub color: RoundColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundDrawKind {
    Background,
    RoomGlyph,
    PropGlyph,
    PetGlyph,
    Halo,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundColor(pub f32, pub f32, pub f32, pub f32);

/// V1 round companion palette. These colors are tuned for the truecolor
/// round aperture and should be kept in sync with the preview lab fixtures.
const BACKGROUND_COLOR: RoundColor = RoundColor(0.08, 0.09, 0.10, 1.0);
const PET_GLYPH_COLOR: RoundColor = RoundColor(0.93, 0.92, 0.89, 1.0);
const PROP_GLYPH_COLOR: RoundColor = RoundColor(0.70, 0.82, 0.52, 1.0);
const HALO_CALM_COLOR: RoundColor = RoundColor(0.36, 0.40, 0.55, 0.8);
const HALO_ACTIVE_COLOR: RoundColor = RoundColor(0.94, 0.65, 0.28, 0.9);
const TROUBLE_GLYPH_COLOR: RoundColor = RoundColor(0.92, 0.30, 0.25, 0.95);

pub fn build_draw_commands(
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
) -> Vec<RoundDrawCommand> {
    let mut commands = vec![RoundDrawCommand {
        kind: RoundDrawKind::Background,
        x: layout.aperture.center_x,
        y: layout.aperture.center_y,
        radius: layout.aperture.radius,
        label: None,
        color: BACKGROUND_COLOR,
    }];
    push_pet_art_commands(&mut commands, scene, layout);
    for anchor in &layout.prop_anchors {
        commands.push(RoundDrawCommand {
            kind: RoundDrawKind::PropGlyph,
            x: anchor.x,
            y: anchor.y,
            radius: anchor.radius,
            label: Some('*'),
            color: PROP_GLYPH_COLOR,
        });
    }
    for anchor in &layout.halo_anchors {
        let is_trouble = anchor.kind == RoundAnchorKind::HelperTrouble;
        commands.push(RoundDrawCommand {
            kind: if is_trouble {
                RoundDrawKind::Trouble
            } else {
                RoundDrawKind::Halo
            },
            x: anchor.x,
            y: anchor.y,
            radius: anchor.radius,
            label: None,
            color: if is_trouble {
                TROUBLE_GLYPH_COLOR
            } else if scene.lifecycle.calm {
                HALO_CALM_COLOR
            } else {
                HALO_ACTIVE_COLOR
            },
        });
    }
    commands
}

fn push_pet_art_commands(
    commands: &mut Vec<RoundDrawCommand>,
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
) {
    let (art_width, art_height) = pet_art_dimensions(&scene.pet.art_lines);
    if art_width == 0.0 || art_height == 0.0 {
        return;
    }

    let cell_size = ((layout.pet_anchor.radius * 2.0) / art_width.max(art_height)).max(1.0);
    let glyph_radius = (cell_size * 0.48).clamp(5.0, 18.0);
    let left = layout.pet_anchor.x - (art_width * cell_size) / 2.0;
    let top = layout.pet_anchor.y + ((art_height - 1.0) * cell_size) / 2.0;

    for (row, line) in scene.pet.art_lines.iter().enumerate() {
        let mut col = 0.0;
        for ch in line.chars() {
            let width = char_display_width(ch) as f32;
            if ch != ' ' {
                commands.push(RoundDrawCommand {
                    kind: RoundDrawKind::PetGlyph,
                    x: left + (col + width / 2.0) * cell_size,
                    y: top - row as f32 * cell_size,
                    radius: glyph_radius,
                    label: Some(ch),
                    color: PET_GLYPH_COLOR,
                });
            }
            col += width;
        }
    }
}

fn pet_art_dimensions(lines: &[String]) -> (f32, f32) {
    let width = lines
        .iter()
        .map(|line| Line::from(line.as_str()).width())
        .max()
        .unwrap_or(0) as f32;
    (width, lines.len() as f32)
}

fn char_display_width(ch: char) -> usize {
    Line::from(ch.to_string()).width().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
    use crate::round::model::derive_round_scene_model;
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    #[test]
    fn draw_commands_keep_all_points_inside_aperture() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(360, 360),
            RoundRenderCapabilities::preview_truecolor(),
        );

        let commands = build_draw_commands(&scene, &layout);

        assert!(commands
            .iter()
            .all(|command| layout.aperture.contains(command.x, command.y)));
        assert!(commands
            .iter()
            .any(|command| command.kind == RoundDrawKind::PetGlyph));
        assert!(commands
            .iter()
            .any(|command| command.kind == RoundDrawKind::Halo));
    }

    #[test]
    fn draw_commands_emit_pet_art_glyphs() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_art = vec!["AB".to_string(), " C".to_string()];
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(360, 360),
            RoundRenderCapabilities::preview_truecolor(),
        );

        let labels: Vec<_> = build_draw_commands(&scene, &layout)
            .into_iter()
            .filter(|command| command.kind == RoundDrawKind::PetGlyph)
            .filter_map(|command| command.label)
            .collect();

        assert_eq!(labels, vec!['A', 'B', 'C']);
    }
}
