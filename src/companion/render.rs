use crate::round::layout::{RoundAnchorKind, RoundSceneLayout};
use crate::round::model::RoundSceneModel;

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
    commands.push(RoundDrawCommand {
        kind: RoundDrawKind::PetGlyph,
        x: layout.pet_anchor.x,
        y: layout.pet_anchor.y,
        radius: layout.pet_anchor.radius,
        // V1 draws a fixed "glorp" glyph cluster centered on the pet anchor.
        label: None,
        color: PET_GLYPH_COLOR,
    });
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
}
