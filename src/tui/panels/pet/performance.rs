use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};

use crate::tui::component::PetSceneLayout;
use crate::tui::room::PetPerformance;
use crate::tui::style::ColorCapability;

/// Overwrites one or two cells near the pet with a tiny performance cue glyph.
/// Keeps the rest of the pet template untouched — this is punctuation, not a
/// rewrite.
pub(super) fn apply_pet_performance_cues(
    buf: &mut Buffer,
    scene: &PetSceneLayout,
    performance: PetPerformance,
    color_capability: ColorCapability,
) {
    let style = performance_cue_style(color_capability);
    match performance {
        PetPerformance::TiredAwake => mark_pet_floor(buf, scene, '˙', style),
        PetPerformance::HeavyDayCozy => mark_pet_floor(buf, scene, '~', style),
        PetPerformance::AsleepDreaming => mark_pet_air(buf, scene, 'z', style),
        PetPerformance::CatchUpWake => mark_pet_air(buf, scene, '^', style),
        PetPerformance::SourceBurstPerk => mark_pet_air(buf, scene, '!', style),
        PetPerformance::RestedAwake => {}
    }
}

fn performance_cue_style(color_capability: ColorCapability) -> Style {
    let color = if matches!(color_capability, ColorCapability::Flat) {
        crate::tui::style::tokenpet_palette().faint.rgb
    } else {
        Color::Rgb(0xd4, 0xa6, 0x57)
    };
    Style::default().fg(color)
}

/// Places `symbol` on the floor cell just below the pet's bounding rect,
/// clipped to the habitat area.
fn mark_pet_floor(buf: &mut Buffer, scene: &PetSceneLayout, symbol: char, style: Style) {
    let x = scene.pet_art.x + scene.pet_art.width / 2;
    let y = scene.pet_art.y.saturating_add(scene.pet_art.height);
    let within_habitat = x >= scene.habitat.x
        && y >= scene.habitat.y
        && x < scene.habitat.x.saturating_add(scene.habitat.width)
        && y < scene.habitat.y.saturating_add(scene.habitat.height);
    if within_habitat {
        let cell = &mut buf[(x, y)];
        cell.set_char(symbol);
        cell.set_style(style);
    }
}

/// Places `symbol` on the air cell just above the pet's bounding rect,
/// clipped to the habitat area. Skips the write when there is no row above
/// the pet, so the cue never overwrites pet art.
fn mark_pet_air(buf: &mut Buffer, scene: &PetSceneLayout, symbol: char, style: Style) {
    let x = scene.pet_art.x + scene.pet_art.width / 2;
    let y = scene.pet_art.y.saturating_sub(1);
    let above_pet = y < scene.pet_art.y;
    let within_habitat = x >= scene.habitat.x
        && y >= scene.habitat.y
        && x < scene.habitat.x.saturating_add(scene.habitat.width)
        && y < scene.habitat.y.saturating_add(scene.habitat.height);
    if above_pet && within_habitat {
        let cell = &mut buf[(x, y)];
        cell.set_char(symbol);
        cell.set_style(style);
    }
}
