use ratatui::buffer::Buffer;

use crate::game::habitat::HabitatPetLayer;
use crate::tui::component::{HabitatPropCell, PetSceneLayout};
use crate::tui::life::PropReaction;
use crate::tui::style::ColorCapability;

fn habitat_contains(scene: &PetSceneLayout, prop: &HabitatPropCell) -> bool {
    prop.col >= scene.habitat.x
        && prop.row >= scene.habitat.y
        && prop.col < scene.habitat.x.saturating_add(scene.habitat.width)
        && prop.row < scene.habitat.y.saturating_add(scene.habitat.height)
}

pub(super) fn render_prop_layer(
    buf: &mut Buffer,
    prop_cells: &[HabitatPropCell],
    scene: &PetSceneLayout,
    reactions: &[PropReaction],
    color_capability: ColorCapability,
    layer: HabitatPetLayer,
) {
    render_prop_layers(
        buf,
        prop_cells,
        scene,
        reactions,
        color_capability,
        &[layer],
    );
}

pub(super) fn render_prop_layers(
    buf: &mut Buffer,
    prop_cells: &[HabitatPropCell],
    scene: &PetSceneLayout,
    reactions: &[PropReaction],
    color_capability: ColorCapability,
    layers: &[HabitatPetLayer],
) {
    for prop in prop_cells {
        if layers.contains(&prop.pet_layer) && habitat_contains(scene, prop) {
            let reaction = reactions
                .iter()
                .find(|reaction| reaction.prop_id == prop.prop_id);
            let cell = &mut buf[(prop.col, prop.row)];
            cell.set_char(prop.glyph);
            cell.set_style(super::colors::apply_prop_reaction_style(
                prop.style,
                reaction,
                color_capability,
            ));
        }
    }
}
