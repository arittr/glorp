use glorp::game::habitat::HabitatPetLayer;
use glorp::presentation::props::{PresentationPropLayer, PresentationPropPlacement};
use glorp::tui::component::habitat_props::habitat_prop_placements_for;
use glorp::tui::render_context::{RenderContext, WatchClock};
use glorp::tui::style::ColorCapability;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

#[test]
fn presentation_prop_wrappers_use_neutral_targets() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let scene = glorp::tui::component::PetScene::compute_layout(
        ratatui::layout::Rect::new(0, 0, 80, 24),
        &vm,
        &RenderContext::with_clock(
            ColorCapability::Truecolor,
            WatchClock::fixed(datetime!(2026-06-15 12:00 UTC)),
        ),
    );
    let placements = habitat_prop_placements_for(
        &vm.habitat,
        &scene,
        &[],
        vm.pet_render.generated_species,
        &vm.pet_render.seed,
        &RenderContext::with_clock(
            ColorCapability::Truecolor,
            WatchClock::fixed(datetime!(2026-06-15 12:00 UTC)),
        ),
    );

    let wrapped = placements
        .iter()
        .map(PresentationPropPlacement::from_habitat_placement)
        .collect::<Vec<_>>();

    assert!(!placements.is_empty());
    assert_eq!(wrapped.len(), placements.len());
    for (source, placement) in placements.iter().zip(wrapped) {
        assert_eq!(placement.prop_id, source.prop_id.as_str());
        assert_eq!(
            placement.layer,
            match source.pet_layer {
                HabitatPetLayer::Background => PresentationPropLayer::Background,
                HabitatPetLayer::Behind => PresentationPropLayer::Behind,
                HabitatPetLayer::Foreground => PresentationPropLayer::Foreground,
            }
        );
        assert_eq!(placement.bounds.x, source.bounds.x);
        assert_eq!(placement.bounds.y, source.bounds.y);
        assert_eq!(placement.bounds.width, source.bounds.width);
        assert_eq!(placement.bounds.height, source.bounds.height);
        assert_eq!(placement.cells.len(), source.cells.len());
        for (source_cell, cell) in source.cells.iter().zip(&placement.cells) {
            assert_eq!(cell.x, source_cell.col);
            assert_eq!(cell.y, source_cell.row);
            assert_eq!(cell.glyph, source_cell.glyph);
        }
        if let Some(target) = placement.effect_target {
            assert!(!target.as_str().starts_with("watch."));
            assert!(target.as_str().starts_with("prop."));
        }
    }
}
