use crate::presentation::{PetSceneModel, SceneDrawList};
use crate::tui::component::{PetSceneLayout, TankLifeSurfaceGeometry};
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

/// Produce a fully-ordered [`SceneDrawList`] for the pet scene.
///
/// Callers blit the result once via `blit_draw_list`. Z-order (back to front):
/// biome-wash → room-glyphs → ambient → motes → activity →
/// props/tank-life(Background, Behind) → contact-shadow → pet-body →
/// performance-cue → props/tank-life(Foreground).
///
/// Speech is NOT in the draw list: it occupies the top rows of the habitat
/// (an entry in `ambient_exclusions`) and is painted separately AFTER the
/// single blit, so it always renders on top.
pub(crate) fn render_pet_to_draw_list(
    scene_model: &PetSceneModel,
    vm: &WatchViewModel,
    scene: &PetSceneLayout,
    now: time::OffsetDateTime,
    ctx: &RenderContext,
) -> SceneDrawList {
    let tank_geometry = crate::tui::component::watch_tank_life_geometry(scene);
    render_pet_to_draw_list_with_tank_geometry(scene_model, vm, scene, now, ctx, &tank_geometry)
}

pub(crate) fn render_pet_to_draw_list_with_tank_geometry(
    scene_model: &PetSceneModel,
    vm: &WatchViewModel,
    scene: &PetSceneLayout,
    now: time::OffsetDateTime,
    ctx: &RenderContext,
    tank_geometry: &TankLifeSurfaceGeometry,
) -> SceneDrawList {
    super::render_layered_pet_scene_with_tank_geometry(
        scene_model,
        vm,
        scene,
        now,
        ctx,
        tank_geometry,
    )
    .flatten_classic_cells()
}

#[cfg(test)]
fn rendered_pet_rect_for_performance(
    scene: &PetSceneLayout,
    performance: crate::tui::room::PetPerformance,
) -> ratatui::layout::Rect {
    let posture = super::colors::performance_posture_offset(performance);
    let mut rect = scene.pet_art;
    let max_y = scene.habitat.y + scene.habitat.height.saturating_sub(rect.height);
    rect.y = (rect.y + posture).min(max_y);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::room::PetPerformance;
    use ratatui::layout::Rect;

    fn scene_with_pet_art(pet_art: Rect) -> PetSceneLayout {
        let area = Rect::new(0, 0, 80, 14);
        PetSceneLayout {
            id: crate::tui::component::WatchComponentId::Pet.path(),
            panel: area,
            speech: None,
            content: area,
            pet_art,
            hit_area: area,
            habitat: area,
            exclusions: Vec::new(),
            targets: std::collections::BTreeMap::new(),
            effect_targets: Vec::new(),
        }
    }

    #[test]
    fn rendered_pet_rect_applies_performance_posture_before_tank_life_protection() {
        let scene = scene_with_pet_art(Rect::new(30, 3, 13, 10));

        let pet_rect = rendered_pet_rect_for_performance(&scene, PetPerformance::TiredAwake);
        let protected = crate::tui::component::pet_face_protected_regions(pet_rect);

        assert_eq!(pet_rect, Rect::new(30, 4, 13, 10));
        assert_eq!(protected, vec![Rect::new(33, 5, 6, 4)]);
    }
}
