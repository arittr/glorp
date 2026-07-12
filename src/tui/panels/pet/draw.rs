use crate::presentation::{PetSceneModel, SceneDrawList};
use crate::tui::component::{PetSceneLayout, TankLifeSurfaceGeometry};
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

#[cfg(test)]
use ratatui::style::Color;

#[cfg(test)]
use crate::game::habitat::HabitatPetLayer;
#[cfg(test)]
use crate::tui::component::habitat_props_for;
#[cfg(test)]
use crate::tui::life::build_prop_reactions;

/// Produce a fully-ordered [`SceneDrawList`] for the pet scene.
///
/// Callers blit the result once via `blit_draw_list`. Z-order (back to front):
/// biome-wash → room-glyphs → ambient → motes → activity →
/// props/tank-life(Background, Behind) → chest-bubble → wall-shadow →
/// floor-projection → pet-body → performance-cue → props/tank-life(Foreground).
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
/// Legacy parity oracle preserved from the pre-layer classic draw builder
/// (`render_pet_to_draw_list_with_tank_geometry` before Task 2 routing).
pub(crate) fn render_legacy_pet_scene_draw_list_parity_oracle_with_tank_geometry(
    scene_model: &PetSceneModel,
    vm: &WatchViewModel,
    scene: &PetSceneLayout,
    now: time::OffsetDateTime,
    ctx: &RenderContext,
    tank_geometry: &TankLifeSurfaceGeometry,
) -> SceneDrawList {
    let mut list = SceneDrawList::default();

    let species = vm.pet_render.generated_species;
    let stage = vm.pet_render.stage;
    let mirror = vm.facing == -1;
    let day = &vm.day_context;
    let room_profile = scene_model.room.clone();

    let silhouette_halo = super::pet_silhouette_halo_rects(&vm.pet_art, scene.pet_art, mirror);

    let mut ambient_exclusions: Vec<ratatui::layout::Rect> = scene
        .exclusions
        .iter()
        .copied()
        .filter(|r| *r != scene.pet_art)
        .collect();
    ambient_exclusions.extend_from_slice(&silhouette_halo);

    let softening = super::ambient::effective_weekend_softening(day, &vm.life_profile);

    let area = scene.panel;
    let compact = area.width <= 72 || area.height <= 24;

    let earned: Vec<crate::storage::state::EarnedHabitatProp> = vm
        .habitat
        .earned_props
        .iter()
        .map(|prop| crate::storage::state::EarnedHabitatProp {
            id: prop.id.clone(),
            earned_at: prop.earned_at,
            source: prop.source.clone(),
        })
        .collect();
    let resonant_prop = crate::tui::day::resonant_prop_for_day(day, &earned);

    let earned_prop_ids = vm
        .habitat
        .earned_props
        .iter()
        .map(|prop| prop.id.clone())
        .collect::<Vec<_>>();
    let life_profile = build_prop_reactions(vm.life_profile.clone(), &earned_prop_ids, compact);
    let life_profile = super::apply_resonance_reaction(life_profile, resonant_prop.as_ref());
    let pet_rect = rendered_pet_rect_for_performance(scene, room_profile.pet_performance);

    list.extend(super::grounding::biome_wash_cells(
        scene.habitat,
        room_profile.biome.primary,
    ));

    let room_glyphs = crate::tui::room::room_glyphs_for(
        &room_profile,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
        day.day_phase,
    );
    list.extend(
        room_glyphs
            .into_iter()
            .map(|g| crate::presentation::DrawCell {
                row: g.row,
                col: g.col,
                glyph: Some(g.glyph.to_string()),
                fg: super::color_to_rgb(g.style.fg.unwrap_or(Color::Reset)),
                bg: None,
                bold: false,
            }),
    );

    let phase_blend = {
        let since = (now - day.phase_started_at_utc).whole_seconds() as f32;
        (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
    };
    let glyphs = super::ambient::ambient_glyphs_for_phase(
        species,
        stage,
        room_profile.biome.primary,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
        day.day_phase,
        phase_blend,
        day.date_seed,
        day.season,
        day.climate,
    );
    list.extend(
        glyphs
            .into_iter()
            .filter(|g| super::ambient::ambient_glyph_is_inside_area(g, scene.habitat))
            .map(|g| crate::presentation::DrawCell {
                row: g.row,
                col: g.col,
                glyph: Some(g.glyph.to_string()),
                fg: super::color_to_rgb(super::ambient::weekend_soften_color(g.color, softening)),
                bg: None,
                bold: false,
            }),
    );

    let motes = super::ambient::mote_glyphs_for(
        &vm.day_context,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
    );
    list.extend(
        motes
            .into_iter()
            .filter(|g| super::ambient::ambient_glyph_is_inside_area(g, scene.habitat))
            .map(|g| crate::presentation::DrawCell {
                row: g.row,
                col: g.col,
                glyph: Some(g.glyph.to_string()),
                fg: super::color_to_rgb(super::ambient::weekend_soften_color(g.color, softening)),
                bg: None,
                bold: false,
            }),
    );

    let extra_count = super::colors::activity_glyph_budget(&life_profile, compact);
    let activity_glyphs = super::ambient::activity_glyphs_for(
        &life_profile,
        species,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
        extra_count,
    );
    list.extend(
        activity_glyphs
            .into_iter()
            .filter(|g| super::ambient::ambient_glyph_is_inside_area(g, scene.habitat))
            .map(|g| crate::presentation::DrawCell {
                row: g.row,
                col: g.col,
                glyph: Some(g.glyph.to_string()),
                fg: super::color_to_rgb(g.color),
                bg: None,
                bold: false,
            }),
    );

    let prop_cells = habitat_props_for(
        &vm.habitat,
        scene,
        &silhouette_halo,
        species,
        &vm.pet_render.seed,
        ctx,
    );
    let canonical_tank_life = crate::tui::component::canonical_daily_cast(
        &vm.habitat.earned_inhabitants,
        &vm.pet_render.seed,
        vm.habitat.tank_life_local_date,
        vm.habitat.tank_life_calendar_age_days,
    );
    let projected_tank_life =
        crate::tui::component::project_tank_life_cast(&canonical_tank_life, tank_geometry);
    let pet_protected = crate::tui::component::pet_face_protected_regions(pet_rect);
    let tank_life_placements = crate::tui::component::tank_life_placements_for(
        &crate::tui::component::TankLifeRenderInput {
            rendered_ids: projected_tank_life.rendered_ids.clone(),
            pet_seed: &vm.pet_render.seed,
            local_date: vm.habitat.tank_life_local_date,
            now,
            geometry: tank_geometry,
            pet_protected_regions: &pet_protected,
            color_capability: ctx.color_capability,
            life_profile: life_profile.clone(),
            asleep: vm.day_context.asleep,
        },
    );
    let tank_life_cells = tank_life_placements
        .iter()
        .flat_map(|placement| placement.cells.clone())
        .collect::<Vec<_>>();
    list.extend(super::props::prop_layer_cells(
        &prop_cells,
        scene,
        &life_profile.prop_reactions,
        ctx.color_capability,
        &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
    ));
    list.extend(super::tank_life::tank_life_layer_cells(
        &tank_life_cells,
        scene,
        &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
    ));

    let chest_cells: Vec<_> = prop_cells
        .iter()
        .filter(|c| c.prop_id.as_str() == "token_treasure_chest_2m")
        .collect();
    if !chest_cells.is_empty() {
        let top = chest_cells.iter().map(|c| c.row).min().unwrap();
        let min_col = chest_cells.iter().map(|c| c.col).min().unwrap();
        let max_col = chest_cells.iter().map(|c| c.col).max().unwrap();
        let center_col = (min_col + max_col) / 2;
        let seed = vm.pet_render.seed.bytes().fold(0u64, |acc, byte| {
            acc.wrapping_mul(131).wrapping_add(u64::from(byte))
        });
        list.extend(super::chest_bubble::chest_bubble_cells(
            top,
            center_col,
            scene.habitat,
            now,
            seed,
            crate::pet::palette::Rgb { r: 0x8c, g: 0xc8, b: 0xd4 },
        ));
    }

    let effects = scene_model.effects;
    let inputs = super::colors::watch_live_color_inputs(
        vm,
        now,
        room_profile.pet_performance,
        effects.shimmer_role,
        effects.token_pop.is_some(),
    );
    let (live_styles, _droop_styles) =
        super::colors::resolve_watch_pet_styles(&vm.pet_palette, &inputs, ctx.color_capability);

    let effective_twinkle = if effects.token_pop.is_some() {
        Some(crate::pet::animator::TwinkleSpec { row: 4, col: 5, glyph: '\u{2726}' })
    } else {
        effects.twinkle
    };

    let cursor_norm_x = super::art_lines::cursor_normalized_x_within(vm, scene.hit_area);
    let lines = super::art_lines::build_pet_lines(
        vm,
        pet_rect.width as usize,
        &live_styles,
        cursor_norm_x,
        effective_twinkle,
    );
    let pet_body = super::art_lines::pet_body_cells(pet_rect, &lines);
    list.extend(super::grounding::wall_shadow_draw_cells(
        &pet_body,
        scene.habitat,
        room_profile.biome.primary,
    ));
    list.extend(super::grounding::floor_projection_draw_cells(
        &pet_body,
        scene.habitat,
        room_profile.biome.primary,
    ));
    list.extend(pet_body);

    list.extend(super::performance::performance_cue_cells(
        scene,
        room_profile.pet_performance,
        ctx.color_capability,
    ));

    list.extend(super::props::prop_layer_cells(
        &prop_cells,
        scene,
        &life_profile.prop_reactions,
        ctx.color_capability,
        &[HabitatPetLayer::Foreground],
    ));
    list.extend(super::tank_life::tank_life_layer_cells(
        &tank_life_cells,
        scene,
        &[HabitatPetLayer::Foreground],
    ));

    list
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
