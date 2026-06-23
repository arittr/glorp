use ratatui::style::Color;

use crate::game::habitat::HabitatPetLayer;
use crate::presentation::{PetSceneModel, SceneDrawList};
use crate::tui::component::{habitat_props_for, PetSceneLayout};
use crate::tui::life::build_prop_reactions;
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

use super::ambient::{
    activity_glyphs_for, ambient_glyph_is_inside_area, ambient_glyphs_for_phase, mote_glyphs_for,
    weekend_soften_color,
};
use super::art_lines::{build_pet_lines, cursor_normalized_x_within, pet_body_cells};
use super::colors::{
    activity_glyph_budget, performance_posture_offset, resolve_watch_pet_styles,
    watch_live_color_inputs,
};
use super::grounding;
use super::performance::performance_cue_cells;
use super::props::prop_layer_cells;
use super::{apply_resonance_reaction, color_to_rgb, pet_silhouette_halo_rects};

/// Produce a fully-ordered [`SceneDrawList`] for the pet scene.
///
/// Callers blit the result once via `blit_draw_list`. Z-order (back to front):
/// biome-wash → room-glyphs → ambient → motes → activity →
/// props(Background, Behind) → contact-shadow → pet-body →
/// performance-cue → props(Foreground).
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
    let mut list = SceneDrawList::default();

    let species = vm.pet_render.generated_species;
    let stage = vm.pet_render.stage;
    let mirror = vm.facing == -1;
    let day = &vm.day_context;
    let room_profile = scene_model.room.clone();

    // Silhouette halo: used by ambient exclusions and contact shadow.
    let silhouette_halo = pet_silhouette_halo_rects(&vm.pet_art, scene.pet_art, mirror);

    // Ambient exclusion list: layout exclusions minus the pet_art rect, plus halo.
    let mut ambient_exclusions: Vec<ratatui::layout::Rect> = scene
        .exclusions
        .iter()
        .copied()
        .filter(|r| *r != scene.pet_art)
        .collect();
    ambient_exclusions.extend_from_slice(&silhouette_halo);

    // Weekend softening for ambient/mote glyphs.
    let softening = super::ambient::effective_weekend_softening(day, &vm.life_profile);

    // Compact flag: controls activity glyph budget and prop reaction count.
    let area = scene.panel;
    let compact = area.width <= 72 || area.height <= 24;

    // Resonant prop lookup (same logic as PetPanel::render).
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
    let life_profile = apply_resonance_reaction(life_profile, resonant_prop.as_ref());

    // ── Pass 1: biome-wash ────────────────────────────────────────────────────
    list.extend(grounding::biome_wash_cells(
        scene.habitat,
        room_profile.biome.primary,
    ));

    // ── Pass 2: room glyphs ───────────────────────────────────────────────────
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
                fg: color_to_rgb(g.style.fg.unwrap_or(Color::Reset)),
                bg: None,
                bold: false,
            }),
    );

    // ── Pass 3: ambient glyphs ────────────────────────────────────────────────
    let phase_blend = {
        let since = (now - day.phase_started_at_utc).whole_seconds() as f32;
        (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
    };
    let glyphs = ambient_glyphs_for_phase(
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
            .filter(|g| ambient_glyph_is_inside_area(g, scene.habitat))
            .map(|g| crate::presentation::DrawCell {
                row: g.row,
                col: g.col,
                glyph: Some(g.glyph.to_string()),
                fg: color_to_rgb(weekend_soften_color(g.color, softening)),
                bg: None,
                bold: false,
            }),
    );

    // ── Pass 4: motes ─────────────────────────────────────────────────────────
    let motes = mote_glyphs_for(
        &vm.day_context,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
    );
    list.extend(
        motes
            .into_iter()
            .filter(|g| ambient_glyph_is_inside_area(g, scene.habitat))
            .map(|g| crate::presentation::DrawCell {
                row: g.row,
                col: g.col,
                glyph: Some(g.glyph.to_string()),
                fg: color_to_rgb(weekend_soften_color(g.color, softening)),
                bg: None,
                bold: false,
            }),
    );

    // ── Pass 5: activity glyphs ───────────────────────────────────────────────
    let extra_count = activity_glyph_budget(&life_profile, compact);
    let activity_glyphs = activity_glyphs_for(
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
            .filter(|g| ambient_glyph_is_inside_area(g, scene.habitat))
            .map(|g| crate::presentation::DrawCell {
                row: g.row,
                col: g.col,
                glyph: Some(g.glyph.to_string()),
                fg: color_to_rgb(g.color),
                bg: None,
                bold: false,
            }),
    );

    // ── Pass 6: props (Background, Behind) ───────────────────────────────────
    let prop_cells = habitat_props_for(
        &vm.habitat,
        scene,
        &silhouette_halo,
        species,
        &vm.pet_render.seed,
        ctx,
    );
    list.extend(prop_layer_cells(
        &prop_cells,
        scene,
        &life_profile.prop_reactions,
        ctx.color_capability,
        &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
    ));

    // ── Pass 7: contact shadow ────────────────────────────────────────────────
    list.extend(grounding::contact_shadow_draw_cells(
        scene.pet_art,
        &vm.pet_art,
        vm.facing,
        scene.habitat,
        room_profile.biome.primary,
    ));

    // ── Pass 8: pet body ──────────────────────────────────────────────────────
    let effects = scene_model.effects;
    let inputs = watch_live_color_inputs(
        vm,
        now,
        room_profile.pet_performance,
        effects.shimmer_role,
        effects.token_pop.is_some(),
    );
    let (live_styles, _droop_styles) =
        resolve_watch_pet_styles(&vm.pet_palette, &inputs, ctx.color_capability);

    let effective_twinkle = if effects.token_pop.is_some() {
        Some(crate::pet::animator::TwinkleSpec {
            row: 4,
            col: 5,
            glyph: '\u{2726}',
        })
    } else {
        effects.twinkle
    };

    let cursor_norm_x = cursor_normalized_x_within(vm, scene.hit_area);
    let posture = performance_posture_offset(room_profile.pet_performance);
    let pet_rect = {
        let mut r = scene.pet_art;
        let max_y = scene.habitat.y + scene.habitat.height.saturating_sub(r.height);
        r.y = (r.y + posture).min(max_y);
        r
    };
    let lines = build_pet_lines(
        vm,
        pet_rect.width as usize,
        &live_styles,
        cursor_norm_x,
        effective_twinkle,
    );
    list.extend(pet_body_cells(pet_rect, &lines));

    // ── Pass 9: performance cue ───────────────────────────────────────────────
    list.extend(performance_cue_cells(
        scene,
        room_profile.pet_performance,
        ctx.color_capability,
    ));

    // ── Pass 10: props (Foreground) ───────────────────────────────────────────
    list.extend(prop_layer_cells(
        &prop_cells,
        scene,
        &life_profile.prop_reactions,
        ctx.color_capability,
        &[HabitatPetLayer::Foreground],
    ));

    list
}
