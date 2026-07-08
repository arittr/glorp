use ratatui::style::Color;

use crate::game::habitat::HabitatPetLayer;
use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use crate::tui::component::{PetSceneLayout, TankLifeCell};

fn habitat_contains(scene: &PetSceneLayout, cell: &TankLifeCell) -> bool {
    cell.col >= scene.habitat.x
        && cell.row >= scene.habitat.y
        && cell.col < scene.habitat.x.saturating_add(scene.habitat.width)
        && cell.row < scene.habitat.y.saturating_add(scene.habitat.height)
}

fn style_fg_to_rgb(color: Option<Color>) -> Option<Rgb> {
    match color {
        Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
        _ => None,
    }
}

pub(super) fn tank_life_layer_cells(
    tank_cells: &[TankLifeCell],
    scene: &PetSceneLayout,
    layers: &[HabitatPetLayer],
) -> Vec<DrawCell> {
    use ratatui::style::Modifier;

    tank_cells
        .iter()
        .filter(|cell| layers.contains(&cell.pet_layer) && habitat_contains(scene, cell))
        .map(|cell| DrawCell {
            row: cell.row,
            col: cell.col,
            glyph: Some(cell.glyph.to_string()),
            fg: style_fg_to_rgb(cell.style.fg),
            bg: match cell.style.bg {
                Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
                _ => None,
            },
            bold: cell.style.add_modifier.contains(Modifier::BOLD),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::HabitatPetLayer;
    use crate::storage::state::TankInhabitantId;
    use crate::tui::component::{ComponentPath, PetSceneLayout, TankLifeCell};
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

    #[test]
    fn tank_life_layer_cells_filters_by_layer_and_habitat() {
        let scene = make_scene(Rect::new(0, 0, 20, 10));
        let cells = vec![
            TankLifeCell {
                inhabitant_id: TankInhabitantId::new("glass_shrimp"),
                row: 2,
                col: 3,
                glyph: ',',
                style: Style::default().fg(Color::Rgb(200, 160, 220)),
                pet_layer: HabitatPetLayer::Foreground,
            },
            TankLifeCell {
                inhabitant_id: TankInhabitantId::new("needlefish"),
                row: 11,
                col: 3,
                glyph: '‹',
                style: Style::default(),
                pet_layer: HabitatPetLayer::Foreground,
            },
        ];

        let draw = tank_life_layer_cells(&cells, &scene, &[HabitatPetLayer::Foreground]);

        assert_eq!(draw.len(), 1);
        assert_eq!(draw[0].row, 2);
        assert_eq!(draw[0].col, 3);
        assert_eq!(draw[0].glyph.as_deref(), Some(","));
    }
}
