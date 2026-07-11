use ratatui::style::{Color, Style};

use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use crate::tui::component::PetSceneLayout;
use crate::tui::room::PetPerformance;
use crate::tui::style::ColorCapability;

/// Returns a list of up to one [`DrawCell`] for the performance cue near the
/// pet. The cue is punctuation only — it never rewrites pet art.
///
/// Returns an empty list for [`PetPerformance::RestedAwake`] (no cue needed).
/// The caller blits the result via
/// [`crate::tui::panels::pet::blit::blit_draw_list`].
pub(super) fn performance_cue_cells(
    scene: &PetSceneLayout,
    performance: PetPerformance,
    color_capability: ColorCapability,
) -> Vec<DrawCell> {
    let style = performance_cue_style(color_capability);
    let fg = style_fg_to_rgb(style.fg);
    match performance {
        PetPerformance::TiredAwake => floor_draw_cell(scene, '˙', fg),
        PetPerformance::HeavyDayCozy => floor_draw_cell(scene, '~', fg),
        PetPerformance::AsleepDreaming => air_draw_cell(scene, 'z', fg),
        PetPerformance::CatchUpWake => air_draw_cell(scene, '^', fg),
        PetPerformance::SourceBurstPerk => air_draw_cell(scene, '!', fg),
        PetPerformance::RestedAwake => vec![],
    }
}

/// Every glyph [`performance_cue_cells`] can place (RestedAwake places none).
/// Declared content for the retained atlas preflight; keep in sync with the
/// match above.
pub(crate) fn declared_performance_cue_glyphs() -> [char; 5] {
    ['˙', '~', 'z', '^', '!']
}

fn performance_cue_style(color_capability: ColorCapability) -> Style {
    let color = if matches!(color_capability, ColorCapability::Flat) {
        crate::tui::style::tokenpet_palette().faint.rgb
    } else {
        Color::Rgb(0xd4, 0xa6, 0x57)
    };
    Style::default().fg(color)
}

fn style_fg_to_rgb(color: Option<Color>) -> Option<Rgb> {
    match color {
        Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
        _ => None,
    }
}

/// Returns a [`DrawCell`] for the floor position (one row below the pet's
/// bounding rect), clipped to the habitat area.  Returns an empty `Vec` if the
/// position is outside the habitat.
fn floor_draw_cell(scene: &PetSceneLayout, symbol: char, fg: Option<Rgb>) -> Vec<DrawCell> {
    let x = scene.pet_art.x + scene.pet_art.width / 2;
    let y = scene.pet_art.y.saturating_add(scene.pet_art.height);
    let within_habitat = x >= scene.habitat.x
        && y >= scene.habitat.y
        && x < scene.habitat.x.saturating_add(scene.habitat.width)
        && y < scene.habitat.y.saturating_add(scene.habitat.height);
    if within_habitat {
        vec![DrawCell {
            row: y,
            col: x,
            glyph: Some(symbol.to_string()),
            fg,
            bg: None,
            bold: false,
        }]
    } else {
        vec![]
    }
}

/// Returns a [`DrawCell`] for the air position (one row above the pet's
/// bounding rect), clipped to the habitat area. Returns an empty `Vec` if
/// there is no row above the pet (prevents overwriting pet art) or the
/// position is outside the habitat.
fn air_draw_cell(scene: &PetSceneLayout, symbol: char, fg: Option<Rgb>) -> Vec<DrawCell> {
    let x = scene.pet_art.x + scene.pet_art.width / 2;
    let y = scene.pet_art.y.saturating_sub(1);
    let above_pet = y < scene.pet_art.y;
    let within_habitat = x >= scene.habitat.x
        && y >= scene.habitat.y
        && x < scene.habitat.x.saturating_add(scene.habitat.width)
        && y < scene.habitat.y.saturating_add(scene.habitat.height);
    if above_pet && within_habitat {
        vec![DrawCell {
            row: y,
            col: x,
            glyph: Some(symbol.to_string()),
            fg,
            bg: None,
            bold: false,
        }]
    } else {
        vec![]
    }
}
