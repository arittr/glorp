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
const PET_GLYPH_COLOR: RoundColor = RoundColor(0.93, 0.92, 0.89, 1.0);
const PROP_GLYPH_COLOR: RoundColor = RoundColor(0.70, 0.82, 0.52, 1.0);
const HALO_CALM_COLOR: RoundColor = RoundColor(0.36, 0.40, 0.55, 0.8);
const HALO_ACTIVE_COLOR: RoundColor = RoundColor(0.94, 0.65, 0.28, 0.9);
const TROUBLE_GLYPH_COLOR: RoundColor = RoundColor(0.92, 0.30, 0.25, 0.95);

/// A dark, biome-tinted background for the companion aperture — keeps the pet
/// dominant (all channels stay low) while giving each place its own cast.
pub(crate) fn biome_background_color(tag: crate::tui::room::RoomBiomeTag) -> RoundColor {
    use crate::tui::room::RoomBiomeTag;
    match tag {
        RoomBiomeTag::Starter => RoundColor(0.08, 0.09, 0.10, 1.0),
        RoomBiomeTag::Botanical => RoundColor(0.07, 0.11, 0.08, 1.0),
        RoomBiomeTag::Technical => RoundColor(0.07, 0.09, 0.13, 1.0),
        RoomBiomeTag::Celestial => RoundColor(0.08, 0.08, 0.14, 1.0),
        RoomBiomeTag::Artifact => RoundColor(0.12, 0.10, 0.07, 1.0),
        RoomBiomeTag::Cozy => RoundColor(0.13, 0.09, 0.08, 1.0),
    }
}

/// Biome background scaled down by day-phase so night reads darker.
pub(crate) fn phase_dim_background(
    tag: crate::tui::room::RoomBiomeTag,
    phase: crate::tui::day::DayPhase,
) -> RoundColor {
    use crate::tui::day::DayPhase;
    let base = biome_background_color(tag);
    let k = match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.85,
        DayPhase::Dusk => 0.8,
        DayPhase::Night => 0.6,
    };
    RoundColor(base.0 * k, base.1 * k, base.2 * k, base.3)
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
    push_room_glyph_commands(&mut commands, scene, layout);
    push_pet_art_command(&mut commands, scene, layout);
    for anchor in &layout.prop_anchors {
        commands.push(RoundDrawCommand {
            kind: RoundDrawKind::PropGlyph,
            x: anchor.x,
            y: anchor.y,
            radius: anchor.radius,
            label: Some('*'),
            text: None,
            spans: Vec::new(),
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

/// Scatter a sparse set of biome/dialect room glyphs across the aperture using
/// the SAME selection vocabulary as the watch (room::biome_symbols / biome_style),
/// placed on a deterministic lattice clipped to the circle.
fn push_room_glyph_commands(
    commands: &mut Vec<RoundDrawCommand>,
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
) {
    use crate::tui::room::{biome_style, biome_symbols, RoomSpeciesDialect};
    let dialect = RoomSpeciesDialect::for_species(scene.pet.species);
    let symbols = biome_symbols(scene.room.biome.primary, dialect);
    if symbols.is_empty() {
        return;
    }
    let style = biome_style(
        scene.room.biome.primary,
        crate::tui::style::ColorCapability::Truecolor,
    );
    let color = match style.fg {
        Some(ratatui::style::Color::Rgb(r, g, b)) => RoundColor(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            0.55,
        ),
        _ => PROP_GLYPH_COLOR,
    };
    let ap = layout.aperture;
    let cell = ap.radius / 5.0; // ~10 glyph slots across the diameter
    if cell <= 0.0 {
        return;
    }
    let mut i = 0usize;
    let steps = 11i32;
    for gy in 0..steps {
        for gx in 0..steps {
            // Sparse: ~1 in 3 lattice points.
            if (gx + gy) % 3 != 0 {
                continue;
            }
            let x = ap.center_x - ap.radius + cell * gx as f32 + cell * 0.5;
            let y = ap.center_y - ap.radius + cell * gy as f32 + cell * 0.5;
            if !ap.contains(x, y) {
                continue;
            }
            // Keep clear of the pet's center disc.
            let dx = x - layout.pet_anchor.x;
            let dy = y - layout.pet_anchor.y;
            if dx * dx + dy * dy < layout.pet_anchor.radius * layout.pet_anchor.radius {
                continue;
            }
            let glyph = symbols[i % symbols.len()];
            i += 1;
            commands.push(RoundDrawCommand {
                kind: RoundDrawKind::RoomGlyph,
                x,
                y,
                radius: cell * 0.5,
                label: Some(glyph),
                text: None,
                spans: Vec::new(),
                color,
            });
        }
    }
}

fn push_pet_art_command(
    commands: &mut Vec<RoundDrawCommand>,
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
) {
    let text = scene.pet.art_lines.join("\n");
    if text.trim().is_empty() {
        return;
    }

    commands.push(RoundDrawCommand {
        kind: RoundDrawKind::PetGlyph,
        x: layout.pet_anchor.x,
        y: layout.pet_anchor.y,
        radius: layout.pet_anchor.radius,
        label: None,
        text: Some(text),
        spans: scene.pet.art_spans.clone(),
        color: PET_GLYPH_COLOR,
    });
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
        use crate::tui::room::RoomBiomeTag;
        let botanical = biome_background_color(RoomBiomeTag::Botanical);
        let technical = biome_background_color(RoomBiomeTag::Technical);
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
            .any(|command| command.kind == RoundDrawKind::PetGlyph));
        assert!(commands
            .iter()
            .any(|command| command.kind == RoundDrawKind::Halo));
    }

    #[test]
    fn draw_commands_emit_one_pet_art_block() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_art = vec!["AB".to_string(), " C".to_string()];
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(360, 360),
            RoundRenderCapabilities::preview_truecolor(),
        );

        let commands = build_draw_commands(&scene, &layout);
        let pet_commands: Vec<_> = commands
            .into_iter()
            .filter(|command| command.kind == RoundDrawKind::PetGlyph)
            .collect();

        assert_eq!(pet_commands.len(), 1);
        assert_eq!(pet_commands[0].label, None);
        assert_eq!(pet_commands[0].text.as_deref(), Some("AB\n C"));
        assert_eq!(pet_commands[0].spans, vm.pet_spans);
    }

    #[test]
    fn emits_room_glyphs_inside_the_aperture() {
        use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
        use crate::round::model::derive_round_scene_model;
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-14 12:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(52, 52),
            RoundRenderCapabilities::preview_truecolor(),
        );
        let commands = build_draw_commands(&scene, &layout);
        let room: Vec<_> = commands
            .iter()
            .filter(|c| c.kind == RoundDrawKind::RoomGlyph)
            .collect();
        assert!(!room.is_empty(), "companion should emit room glyphs");
        for c in &room {
            assert!(
                layout.aperture.contains(c.x, c.y),
                "room glyph outside aperture"
            );
            assert!(c.label.is_some(), "room glyph needs a char");
        }
    }
}
