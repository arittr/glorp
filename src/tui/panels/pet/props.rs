use ratatui::style::Color;

use crate::game::habitat::HabitatPetLayer;
use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use crate::tui::component::{HabitatPropCell, PetSceneLayout};
use crate::tui::life::PropReaction;
use crate::tui::style::ColorCapability;

fn habitat_contains(scene: &PetSceneLayout, prop: &HabitatPropCell) -> bool {
    prop.col >= scene.habitat.x
        && prop.row >= scene.habitat.y
        && prop.col < scene.habitat.x.saturating_add(scene.habitat.width)
        && prop.row < scene.habitat.y.saturating_add(scene.habitat.height)
}

/// Converts a resolved prop style's foreground to [`Rgb`], returning `None`
/// for non-Rgb colors (e.g. flat-mode `Style::default()` has no fg).
fn style_fg_to_rgb(color: Option<Color>) -> Option<Rgb> {
    match color {
        Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
        _ => None,
    }
}

/// Returns one [`DrawCell`] per prop cell that belongs to one of the given
/// `layers` and falls within the habitat bounds.  Reaction-glow style is
/// applied before extracting the colors — the resulting cell carries fg (from
/// the resolved style), bg (currently none for all catalog props), and bold
/// (currently not set by any catalog prop style). Callers blit the returned
/// list via [`crate::tui::panels::pet::blit::blit_draw_list`].
pub(super) fn prop_layer_cells(
    prop_cells: &[HabitatPropCell],
    scene: &PetSceneLayout,
    reactions: &[PropReaction],
    color_capability: ColorCapability,
    layers: &[HabitatPetLayer],
) -> Vec<DrawCell> {
    use ratatui::style::Modifier;
    prop_cells
        .iter()
        .filter(|prop| layers.contains(&prop.pet_layer) && habitat_contains(scene, prop))
        .map(|prop| {
            let reaction = reactions
                .iter()
                .find(|reaction| reaction.prop_id == prop.prop_id);
            let resolved_style =
                super::colors::apply_prop_reaction_style(prop.style, reaction, color_capability);
            DrawCell {
                row: prop.row,
                col: prop.col,
                glyph: Some(prop.glyph.to_string()),
                fg: style_fg_to_rgb(resolved_style.fg),
                bg: match resolved_style.bg {
                    Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
                    _ => None,
                },
                bold: resolved_style.add_modifier.contains(Modifier::BOLD),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::HabitatPetLayer;
    use crate::storage::state::HabitatPropId;
    use crate::tui::component::{ComponentPath, PetSceneLayout};
    use crate::tui::life::{PropReaction, PropReactionKind};
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
    use std::collections::BTreeMap;

    fn make_scene(habitat: Rect) -> PetSceneLayout {
        let pet_art = Rect::new(habitat.x + 5, habitat.y + 2, 13, 10);
        PetSceneLayout {
            id: ComponentPath::new("watch.pet"),
            panel: habitat,
            speech: None,
            content: habitat,
            pet_art,
            hit_area: habitat,
            habitat,
            exclusions: Vec::new(),
            targets: BTreeMap::new(),
            effect_targets: Vec::new(),
        }
    }

    fn prop_cell(
        id: &str,
        row: u16,
        col: u16,
        glyph: char,
        layer: HabitatPetLayer,
    ) -> HabitatPropCell {
        HabitatPropCell {
            prop_id: HabitatPropId::new(id),
            row,
            col,
            glyph,
            style: Style::default().fg(Color::Rgb(100, 150, 200)),
            pet_layer: layer,
        }
    }

    // ── (a) cell in the right layer is returned as a DrawCell ─────────────────
    #[test]
    fn prop_layer_cells_emits_draw_cell_for_matching_layer() {
        let scene = make_scene(Rect::new(0, 0, 40, 20));
        let cells = vec![prop_cell(
            "token_pebble_25k",
            5,
            10,
            '◦',
            HabitatPetLayer::Background,
        )];

        let draw_cells = prop_layer_cells(
            &cells,
            &scene,
            &[],
            ColorCapability::Truecolor,
            &[HabitatPetLayer::Background],
        );

        assert_eq!(draw_cells.len(), 1);
        assert_eq!(draw_cells[0].row, 5);
        assert_eq!(draw_cells[0].col, 10);
        assert_eq!(draw_cells[0].glyph, Some("◦".to_string()));
        assert_eq!(draw_cells[0].fg, Some(Rgb::new(100, 150, 200)));
        assert!(draw_cells[0].bg.is_none());
    }

    // ── (b) cell in wrong layer is filtered out ────────────────────────────────
    #[test]
    fn prop_layer_cells_skips_cells_not_in_requested_layers() {
        let scene = make_scene(Rect::new(0, 0, 40, 20));
        let cells = vec![prop_cell(
            "token_pebble_25k",
            5,
            10,
            '◦',
            HabitatPetLayer::Foreground,
        )];

        let draw_cells = prop_layer_cells(
            &cells,
            &scene,
            &[],
            ColorCapability::Truecolor,
            &[HabitatPetLayer::Background],
        );

        assert!(
            draw_cells.is_empty(),
            "foreground cell should be skipped for background layer"
        );
    }

    // ── (c) cell outside habitat rect is filtered out ─────────────────────────
    #[test]
    fn prop_layer_cells_skips_cells_outside_habitat() {
        let scene = make_scene(Rect::new(0, 0, 20, 10));
        // Place the cell outside the habitat
        let cells = vec![prop_cell(
            "token_pebble_25k",
            15,
            25,
            '◦',
            HabitatPetLayer::Background,
        )];

        let draw_cells = prop_layer_cells(
            &cells,
            &scene,
            &[],
            ColorCapability::Truecolor,
            &[HabitatPetLayer::Background],
        );

        assert!(
            draw_cells.is_empty(),
            "out-of-habitat cell should be skipped"
        );
    }

    // ── (d) reaction glow lifts fg color ─────────────────────────────────────
    #[test]
    fn prop_layer_cells_applies_reaction_glow_to_fg() {
        let scene = make_scene(Rect::new(0, 0, 40, 20));
        let prop_id = HabitatPropId::new("token_orbit_5m");
        let cells = vec![HabitatPropCell {
            prop_id: prop_id.clone(),
            row: 5,
            col: 10,
            glyph: '°',
            style: Style::default().fg(Color::Rgb(100, 100, 100)),
            pet_layer: HabitatPetLayer::Background,
        }];
        let reactions = vec![PropReaction {
            prop_id,
            intensity: 1.0, // max lift
            kind: PropReactionKind::Orbit,
        }];

        let draw_cells = prop_layer_cells(
            &cells,
            &scene,
            &reactions,
            ColorCapability::Truecolor,
            &[HabitatPetLayer::Background],
        );

        assert_eq!(draw_cells.len(), 1);
        let Rgb { r, g, b } = draw_cells[0].fg.unwrap();
        // With intensity=1.0, lift = 35. 100 + 35 = 135.
        assert_eq!(r, 135);
        assert_eq!(g, 135);
        assert_eq!(b, 135);
    }

    // ── (e) multiple layers at once ───────────────────────────────────────────
    #[test]
    fn prop_layer_cells_accepts_multiple_layers() {
        let scene = make_scene(Rect::new(0, 0, 40, 20));
        let cells = vec![
            prop_cell("token_pebble_25k", 2, 5, '◦', HabitatPetLayer::Background),
            prop_cell("token_shell_100k", 4, 8, '◦', HabitatPetLayer::Behind),
            prop_cell("codex_signal_lamp", 6, 12, '⊙', HabitatPetLayer::Foreground),
        ];

        let draw_cells = prop_layer_cells(
            &cells,
            &scene,
            &[],
            ColorCapability::Truecolor,
            &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
        );

        assert_eq!(
            draw_cells.len(),
            2,
            "Background + Behind should be included; Foreground excluded"
        );
    }
}
