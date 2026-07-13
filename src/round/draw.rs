use crate::pet::render::StyledSegment;
use crate::round::layout::{RoundAnchorKind, RoundSceneLayout};
use crate::round::model::RoundSceneModel;

#[derive(Debug, Clone, PartialEq)]
pub struct RoundDrawCommand {
    pub kind: RoundDrawKind,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub label: Option<char>,
    pub text: Option<String>,
    pub spans: Vec<StyledSegment>,
    pub color: RoundColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundDrawKind {
    Background,
    Halo,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundColor(pub f32, pub f32, pub f32, pub f32);

/// V1 round companion palette. These colors are tuned for the truecolor
/// round aperture and should be kept in sync with the preview lab fixtures.
const HALO_CALM_COLOR: RoundColor = RoundColor(
    crate::presentation::companion_effects::STATUS_CALM_SRGBA[0],
    crate::presentation::companion_effects::STATUS_CALM_SRGBA[1],
    crate::presentation::companion_effects::STATUS_CALM_SRGBA[2],
    crate::presentation::companion_effects::STATUS_CALM_SRGBA[3],
);
const HALO_ACTIVE_COLOR: RoundColor = RoundColor(
    crate::presentation::companion_effects::STATUS_ACTIVE_SRGBA[0],
    crate::presentation::companion_effects::STATUS_ACTIVE_SRGBA[1],
    crate::presentation::companion_effects::STATUS_ACTIVE_SRGBA[2],
    crate::presentation::companion_effects::STATUS_ACTIVE_SRGBA[3],
);
const TROUBLE_GLYPH_COLOR: RoundColor = RoundColor(
    crate::presentation::companion_effects::TROUBLE_SRGBA[0],
    crate::presentation::companion_effects::TROUBLE_SRGBA[1],
    crate::presentation::companion_effects::TROUBLE_SRGBA[2],
    crate::presentation::companion_effects::TROUBLE_SRGBA[3],
);

/// Biome background scaled down by day-phase so night reads darker.
pub(crate) fn phase_dim_background(
    tag: crate::tui::room::RoomBiomeTag,
    phase: crate::tui::day::DayPhase,
) -> RoundColor {
    use crate::tui::day::DayPhase;
    let k = match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.85,
        DayPhase::Dusk => 0.8,
        DayPhase::Night => 0.6,
    };
    let color =
        crate::presentation::companion_effects::phase_dim_background_srgb(biome_alias(tag), k);
    RoundColor(color[0], color[1], color[2], 1.0)
}

fn biome_alias(tag: crate::tui::room::RoomBiomeTag) -> &'static str {
    use crate::tui::room::RoomBiomeTag;
    match tag {
        RoomBiomeTag::Starter => "starter",
        RoomBiomeTag::Botanical => "botanical",
        RoomBiomeTag::Technical => "technical",
        RoomBiomeTag::Celestial => "celestial",
        RoomBiomeTag::Artifact => "artifact",
        RoomBiomeTag::Cozy => "cozy",
    }
}

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
        text: None,
        spans: Vec::new(),
        color: phase_dim_background(scene.room.biome.primary, scene.room.day_phase),
    }];
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
            label: Some(if is_trouble { '!' } else { '○' }),
            text: None,
            spans: Vec::new(),
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
    fn background_is_biome_tinted() {
        use crate::tui::day::DayPhase;
        use crate::tui::room::RoomBiomeTag;
        let botanical = phase_dim_background(RoomBiomeTag::Botanical, DayPhase::Day);
        let technical = phase_dim_background(RoomBiomeTag::Technical, DayPhase::Day);
        assert_ne!(botanical, technical);
        // Stays dark (each rgb channel <= 0.22) so the pet pops.
        assert!(botanical.0 <= 0.22 && botanical.1 <= 0.22 && botanical.2 <= 0.22);
    }

    #[test]
    fn night_background_is_darker_than_day() {
        use crate::tui::day::DayPhase;
        use crate::tui::room::RoomBiomeTag;
        let day = phase_dim_background(RoomBiomeTag::Botanical, DayPhase::Day);
        let night = phase_dim_background(RoomBiomeTag::Botanical, DayPhase::Night);
        assert!(night.0 <= day.0 && night.1 <= day.1 && night.2 <= day.2);
        assert!(night != day);
    }

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
            .any(|command| command.kind == RoundDrawKind::Halo));
    }
}
