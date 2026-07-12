use ratatui::style::Color;

use crate::game::habitat::HabitatPetLayer;
use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use crate::tui::component::TankLifeCell;

fn style_fg_to_rgb(color: Option<Color>) -> Option<Rgb> {
    match color {
        Some(Color::Rgb(r, g, b)) => Some(Rgb::new(r, g, b)),
        _ => None,
    }
}

pub(super) fn tank_life_layer_cells(
    tank_cells: &[TankLifeCell],
    layers: &[HabitatPetLayer],
) -> Vec<DrawCell> {
    use ratatui::style::Modifier;

    tank_cells
        .iter()
        .filter(|cell| layers.contains(&cell.pet_layer))
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
    use crate::tui::component::TankLifeCell;
    use ratatui::style::{Color, Style};

    #[test]
    fn tank_life_layer_cells_maps_authoritative_cells_and_filters_only_by_layer() {
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
            TankLifeCell {
                inhabitant_id: TankInhabitantId::new("rim_skimmer"),
                row: 4,
                col: 6,
                glyph: '◜',
                style: Style::default(),
                pet_layer: HabitatPetLayer::Behind,
            },
        ];

        let draw = tank_life_layer_cells(&cells, &[HabitatPetLayer::Foreground]);

        assert_eq!(draw.len(), 2);
        assert_eq!(draw[0].row, 2);
        assert_eq!(draw[0].col, 3);
        assert_eq!(draw[0].glyph.as_deref(), Some(","));
        assert_eq!(draw[1].row, 11);
        assert_eq!(draw[1].col, 3);
        assert_eq!(draw[1].glyph.as_deref(), Some("‹"));
    }
}
