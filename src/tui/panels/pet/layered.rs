use ratatui::{layout::Rect, style::Color};

use crate::game::habitat::HabitatPetLayer;
use crate::presentation::smooth::{
    LayeredPetScene, SmoothBlendMode, SmoothBounds, SmoothClip, SmoothCompanionLayer,
    SmoothCompanionPrivacyClaims, SmoothLayerId, SmoothLayerItem, SmoothLayerRole, SmoothLocalCell,
    SmoothPoint, SmoothTransform,
};
use crate::presentation::{DrawCell, PetSceneModel};
use crate::tui::component::{habitat_props_for, PetSceneLayout, TankLifeSurfaceGeometry};
use crate::tui::life::build_prop_reactions;
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

use super::ambient::{
    activity_glyphs_for, ambient_glyph_is_inside_area, ambient_glyphs_for_phase, mote_glyphs_for,
    weekend_soften_color,
};
use super::art_lines::{build_pet_lines, cursor_normalized_x_within, pet_body_cells};
use super::colors::{activity_glyph_budget, resolve_watch_pet_styles, watch_live_color_inputs};
use super::performance::performance_cue_cells;
use super::props::prop_layer_cells;
use super::tank_life::tank_life_layer_cells;
use super::{apply_resonance_reaction, color_to_rgb, grounding};

pub(crate) fn render_layered_pet_scene_with_tank_geometry(
    scene_model: &PetSceneModel,
    vm: &WatchViewModel,
    scene: &PetSceneLayout,
    now: time::OffsetDateTime,
    ctx: &RenderContext,
    tank_geometry: &TankLifeSurfaceGeometry,
) -> LayeredPetScene {
    let species = vm.pet_render.generated_species;
    let stage = vm.pet_render.stage;
    let mirror = vm.facing == -1;
    let day = &vm.day_context;
    let room_profile = scene_model.room.clone();

    let silhouette_halo = super::pet_silhouette_halo_rects(&vm.pet_art, scene.pet_art, mirror);

    let mut ambient_exclusions: Vec<Rect> = scene
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
    let life_profile = apply_resonance_reaction(life_profile, resonant_prop.as_ref());
    let pet_rect = rendered_pet_rect_for_performance(scene, room_profile.pet_performance);

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
        },
    );
    let tank_life_cells = tank_life_placements
        .iter()
        .flat_map(|placement| placement.cells.clone())
        .collect::<Vec<_>>();

    let phase_blend = {
        let since = (now - day.phase_started_at_utc).whole_seconds() as f32;
        (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
    };

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
        Some(crate::pet::animator::TwinkleSpec { row: 4, col: 5, glyph: '\u{2726}' })
    } else {
        effects.twinkle
    };
    let cursor_norm_x = cursor_normalized_x_within(vm, scene.hit_area);
    let lines = build_pet_lines(
        vm,
        pet_rect.width as usize,
        &live_styles,
        cursor_norm_x,
        effective_twinkle,
    );

    let biome_wash = grounding::biome_wash_cells(scene.habitat, room_profile.biome.primary);
    let room_glyphs = crate::tui::room::room_glyphs_for(
        &room_profile,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
        day.day_phase,
    )
    .into_iter()
    .map(|g| DrawCell {
        row: g.row,
        col: g.col,
        glyph: Some(g.glyph.to_string()),
        fg: color_to_rgb(g.style.fg.unwrap_or(Color::Reset)),
        bg: None,
        bold: false,
    })
    .collect::<Vec<_>>();
    let ambient = ambient_glyphs_for_phase(
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
    )
    .into_iter()
    .filter(|g| ambient_glyph_is_inside_area(g, scene.habitat))
    .map(|g| DrawCell {
        row: g.row,
        col: g.col,
        glyph: Some(g.glyph.to_string()),
        fg: color_to_rgb(weekend_soften_color(g.color, softening)),
        bg: None,
        bold: false,
    })
    .collect::<Vec<_>>();
    let motes = mote_glyphs_for(
        &vm.day_context,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
    )
    .into_iter()
    .filter(|g| ambient_glyph_is_inside_area(g, scene.habitat))
    .map(|g| DrawCell {
        row: g.row,
        col: g.col,
        glyph: Some(g.glyph.to_string()),
        fg: color_to_rgb(weekend_soften_color(g.color, softening)),
        bg: None,
        bold: false,
    })
    .collect::<Vec<_>>();
    let activity_glyphs = activity_glyphs_for(
        &life_profile,
        species,
        scene.habitat,
        &ambient_exclusions,
        now,
        ctx.color_capability,
        activity_glyph_budget(&life_profile, compact),
    )
    .into_iter()
    .filter(|g| ambient_glyph_is_inside_area(g, scene.habitat))
    .map(|g| DrawCell {
        row: g.row,
        col: g.col,
        glyph: Some(g.glyph.to_string()),
        fg: color_to_rgb(g.color),
        bg: None,
        bold: false,
    })
    .collect::<Vec<_>>();
    let props_behind = prop_layer_cells(
        &prop_cells,
        scene,
        &life_profile.prop_reactions,
        ctx.color_capability,
        &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
    );
    let tank_life_behind = tank_life_layer_cells(
        &tank_life_cells,
        scene,
        &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
    );
    let chest_bubble = treasure_chest_bubble_cells(&prop_cells, scene.habitat, now, vm);
    let contact_shadow = grounding::contact_shadow_draw_cells(
        scene.pet_art,
        &vm.pet_art,
        vm.facing,
        scene.habitat,
        room_profile.biome.primary,
    );
    let pet_body = pet_body_cells(pet_rect, &lines);
    let performance_cue =
        performance_cue_cells(scene, room_profile.pet_performance, ctx.color_capability);
    let props_foreground = prop_layer_cells(
        &prop_cells,
        scene,
        &life_profile.prop_reactions,
        ctx.color_capability,
        &[HabitatPetLayer::Foreground],
    );
    let tank_life_foreground =
        tank_life_layer_cells(&tank_life_cells, scene, &[HabitatPetLayer::Foreground]);

    LayeredPetScene {
        layers: vec![
            layer_from_draw_cells(
                "classic-biome-wash",
                SmoothLayerRole::BiomeWash,
                0,
                biome_wash,
            ),
            layer_from_draw_cells(
                "classic-room-glyphs",
                SmoothLayerRole::RoomGlyphs,
                1,
                room_glyphs,
            ),
            layer_from_draw_cells("classic-ambient", SmoothLayerRole::Ambient, 2, ambient),
            layer_from_draw_cells("classic-motes", SmoothLayerRole::Motes, 3, motes),
            layer_from_draw_cells(
                "classic-activity-glyphs",
                SmoothLayerRole::ActivityGlyphs,
                4,
                activity_glyphs,
            ),
            layer_from_draw_cells(
                "classic-props-behind",
                SmoothLayerRole::PropsBehind,
                5,
                props_behind,
            ),
            layer_from_draw_cells(
                "classic-tank-life-behind",
                SmoothLayerRole::TankLifeBehind,
                6,
                tank_life_behind,
            ),
            layer_from_draw_cells(
                "classic-chest-bubble",
                SmoothLayerRole::ChestBubble,
                7,
                chest_bubble,
            ),
            layer_from_draw_cells(
                "classic-contact-shadow",
                SmoothLayerRole::ContactShadow,
                8,
                contact_shadow,
            ),
            layer_from_draw_cells("classic-pet-body", SmoothLayerRole::PetBody, 9, pet_body),
            layer_from_draw_cells(
                "classic-performance-cue",
                SmoothLayerRole::PerformanceCue,
                10,
                performance_cue,
            ),
            layer_from_draw_cells(
                "classic-props-foreground",
                SmoothLayerRole::PropsForeground,
                11,
                props_foreground,
            ),
            layer_from_draw_cells(
                "classic-tank-life-foreground",
                SmoothLayerRole::TankLifeForeground,
                12,
                tank_life_foreground,
            ),
        ],
    }
}

fn rendered_pet_rect_for_performance(
    scene: &PetSceneLayout,
    performance: crate::tui::room::PetPerformance,
) -> Rect {
    let posture = super::colors::performance_posture_offset(performance);
    let mut rect = scene.pet_art;
    let max_y = scene.habitat.y + scene.habitat.height.saturating_sub(rect.height);
    rect.y = (rect.y + posture).min(max_y);
    rect
}

fn treasure_chest_bubble_cells(
    prop_cells: &[crate::tui::component::HabitatPropCell],
    habitat: Rect,
    now: time::OffsetDateTime,
    vm: &WatchViewModel,
) -> Vec<DrawCell> {
    let chest_cells: Vec<_> = prop_cells
        .iter()
        .filter(|c| c.prop_id.as_str() == "token_treasure_chest_2m")
        .collect();
    if chest_cells.is_empty() {
        return Vec::new();
    }

    let top = chest_cells.iter().map(|c| c.row).min().unwrap_or(0);
    let min_col = chest_cells.iter().map(|c| c.col).min().unwrap_or(0);
    let max_col = chest_cells.iter().map(|c| c.col).max().unwrap_or(0);
    let center_col = (min_col + max_col) / 2;
    let seed = vm.pet_render.seed.bytes().fold(0u64, |acc, byte| {
        acc.wrapping_mul(131).wrapping_add(u64::from(byte))
    });
    super::chest_bubble::chest_bubble_cells(
        top,
        center_col,
        habitat,
        now,
        seed,
        crate::pet::palette::Rgb { r: 0x8c, g: 0xc8, b: 0xd4 },
    )
}

fn layer_from_draw_cells(
    id: &str,
    role: SmoothLayerRole,
    z: i16,
    cells: Vec<DrawCell>,
) -> SmoothCompanionLayer {
    let anchor = layer_anchor_for_cells(&cells);
    let local_bounds = local_bounds_for_cells(&cells, anchor);
    SmoothCompanionLayer {
        id: SmoothLayerId(id.to_string()),
        role,
        z,
        local_bounds,
        anchor,
        transform_origin: SmoothPoint { x: 0.0, y: 0.0 },
        transform: SmoothTransform {
            translation: SmoothPoint { x: 0.0, y: 0.0 },
            scale: SmoothPoint { x: 1.0, y: 1.0 },
            rotation_degrees: 0.0,
        },
        opacity: 1.0,
        clip: SmoothClip::None,
        blend: SmoothBlendMode::Normal,
        items: cells
            .into_iter()
            .map(|cell| draw_cell_to_layer_item(cell, anchor))
            .collect(),
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
    }
}

fn draw_cell_to_layer_item(cell: DrawCell, anchor: SmoothPoint) -> SmoothLayerItem {
    SmoothLayerItem::LocalCell(SmoothLocalCell {
        row: cell.row.saturating_sub(anchor.y as u16),
        col: cell.col.saturating_sub(anchor.x as u16),
        glyph: cell.glyph,
        fg: cell.fg,
        bg: cell.bg,
        bold: cell.bold,
    })
}

fn layer_anchor_for_cells(cells: &[DrawCell]) -> SmoothPoint {
    let Some(first) = cells.first() else {
        return SmoothPoint { x: 0.0, y: 0.0 };
    };

    let mut min_x = first.col;
    let mut min_y = first.row;
    for cell in cells.iter().skip(1) {
        min_x = min_x.min(cell.col);
        min_y = min_y.min(cell.row);
    }

    SmoothPoint { x: min_x as f32, y: min_y as f32 }
}

fn local_bounds_for_cells(cells: &[DrawCell], anchor: SmoothPoint) -> SmoothBounds {
    let Some(first) = cells.first() else {
        return SmoothBounds {
            min: SmoothPoint { x: 0.0, y: 0.0 },
            max: SmoothPoint { x: 0.0, y: 0.0 },
        };
    };

    let anchor_col = anchor.x as u16;
    let anchor_row = anchor.y as u16;
    let mut max_x = first.col.saturating_sub(anchor_col);
    let mut max_y = first.row.saturating_sub(anchor_row);
    for cell in cells.iter().skip(1) {
        max_x = max_x.max(cell.col.saturating_sub(anchor_col));
        max_y = max_y.max(cell.row.saturating_sub(anchor_row));
    }

    SmoothBounds {
        min: SmoothPoint { x: 0.0, y: 0.0 },
        max: SmoothPoint {
            x: (max_x + 1) as f32,
            y: (max_y + 1) as f32,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::render_layered_pet_scene_with_tank_geometry;
    use crate::game::habitat::{
        HabitatPropKind, TOKEN_FRIENDLY_CLOUD_750K, TOKEN_HANGING_VINE_25M, TOKEN_TREASURE_CHEST_2M,
    };
    use crate::presentation::smooth::{
        SmoothBounds, SmoothLayerItem, SmoothLayerRole, SmoothPoint,
    };
    use crate::presentation::{DrawCell, PetSceneModel};
    use crate::storage::state::{HabitatPropId, HabitatPropSource};
    use crate::tui::component::{watch_tank_life_geometry, PetScene};
    use crate::tui::day::DayContext;
    use crate::tui::panels::pet::draw::render_legacy_pet_scene_draw_list_parity_oracle_with_tank_geometry;
    use crate::tui::view_model::{EarnedHabitatPropView, WatchViewModel};
    use ratatui::layout::Rect;
    use serde::Serialize;
    use time::macros::{date, datetime};

    const LAYERED_NOW: time::OffsetDateTime = datetime!(2026-07-08 18:00 UTC);

    #[derive(Debug, Serialize)]
    struct FlattenDigest {
        cell_count: usize,
        checksum: u64,
        head: Vec<CellDigest>,
        tail: Vec<CellDigest>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct CellDigest {
        row: u16,
        col: u16,
        glyph: Option<String>,
        fg: Option<(u8, u8, u8)>,
        bg: Option<(u8, u8, u8)>,
        bold: bool,
    }

    impl From<&DrawCell> for CellDigest {
        fn from(cell: &DrawCell) -> Self {
            Self {
                row: cell.row,
                col: cell.col,
                glyph: cell.glyph.clone(),
                fg: cell.fg.map(|rgb| (rgb.r, rgb.g, rgb.b)),
                bg: cell.bg.map(|rgb| (rgb.r, rgb.g, rgb.b)),
                bold: cell.bold,
            }
        }
    }

    fn flatten_digest(cells: &[DrawCell]) -> FlattenDigest {
        let records: Vec<CellDigest> = cells.iter().map(CellDigest::from).collect();
        FlattenDigest {
            cell_count: records.len(),
            checksum: crate::presentation::smooth::classic_flatten_checksum(cells),
            head: records.iter().take(20).cloned().collect(),
            tail: records
                .iter()
                .rev()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
        }
    }

    fn active_prop_rich_vm() -> WatchViewModel {
        let mut vm = super::super::tests::vm_with_real_pet();
        let habitat_vm =
            WatchViewModel::fixture_with_tank_inhabitants_for_age(365, date!(2026 - 07 - 08));
        vm.habitat = habitat_vm.habitat;
        vm.habitat.earned_props.push(EarnedHabitatPropView {
            id: HabitatPropId::new(TOKEN_FRIENDLY_CLOUD_750K),
            earned_at: time::OffsetDateTime::UNIX_EPOCH,
            kind: HabitatPropKind::Trophy,
            display_priority: 110,
            source: HabitatPropSource::LifetimeTokens { threshold: 750_000.0 },
        });
        vm.habitat.earned_props.push(EarnedHabitatPropView {
            id: HabitatPropId::new(TOKEN_HANGING_VINE_25M),
            earned_at: time::OffsetDateTime::UNIX_EPOCH,
            kind: HabitatPropKind::Trophy,
            display_priority: 152,
            source: HabitatPropSource::LifetimeTokens { threshold: 25_000_000.0 },
        });
        vm.life_profile.activity_level = 2.0;
        vm.life_profile.burst_level = 1.2;
        vm.last_feed_pulse_at = Some(LAYERED_NOW);
        vm.day_context = DayContext {
            mature: true,
            today_ratio: 1.0,
            date_seed: 42,
            phase_started_at_utc: LAYERED_NOW - time::Duration::minutes(15),
            phase_ends_at_utc: LAYERED_NOW + time::Duration::minutes(15),
            local_day_started_utc: LAYERED_NOW - time::Duration::hours(2),
            local_day_rollover_utc: LAYERED_NOW + time::Duration::hours(10),
            ..DayContext::default()
        };
        vm
    }

    fn with_treasure_chest(mut vm: WatchViewModel) -> WatchViewModel {
        vm.habitat.earned_props.push(EarnedHabitatPropView {
            id: HabitatPropId::new(TOKEN_TREASURE_CHEST_2M),
            earned_at: time::OffsetDateTime::UNIX_EPOCH,
            kind: HabitatPropKind::Trophy,
            display_priority: 100,
            source: HabitatPropSource::LifetimeTokens { threshold: 2_000_000.0 },
        });
        vm
    }

    fn active_scene_inputs(
        vm: &WatchViewModel,
    ) -> (
        PetSceneModel,
        crate::tui::component::PetSceneLayout,
        crate::tui::render_context::RenderContext,
        crate::tui::component::TankLifeSurfaceGeometry,
    ) {
        let ctx = super::super::tests::test_context();
        let scene = PetScene::compute_layout(Rect::new(0, 0, 80, 18), vm, &ctx);
        let scene_model = PetSceneModel::build(vm, LAYERED_NOW, ctx.color_capability);
        let tank_geometry = watch_tank_life_geometry(&scene);
        (scene_model, scene, ctx, tank_geometry)
    }

    fn non_empty_roles(
        scene: &crate::presentation::smooth::LayeredPetScene,
    ) -> Vec<SmoothLayerRole> {
        scene
            .layers
            .iter()
            .filter(|layer| !layer.items.is_empty())
            .map(|layer| layer.role)
            .collect()
    }

    #[test]
    fn preserves_required_classic_pass_roles_for_active_fixture() {
        let vm = active_prop_rich_vm();
        let (scene_model, scene, ctx, tank_geometry) = active_scene_inputs(&vm);

        let layered = render_layered_pet_scene_with_tank_geometry(
            &scene_model,
            &vm,
            &scene,
            LAYERED_NOW,
            &ctx,
            &tank_geometry,
        );

        assert_eq!(
            non_empty_roles(&layered),
            vec![
                SmoothLayerRole::BiomeWash,
                SmoothLayerRole::RoomGlyphs,
                SmoothLayerRole::Ambient,
                SmoothLayerRole::Motes,
                SmoothLayerRole::ActivityGlyphs,
                SmoothLayerRole::PropsBehind,
                SmoothLayerRole::TankLifeBehind,
                SmoothLayerRole::ContactShadow,
                SmoothLayerRole::PetBody,
                SmoothLayerRole::PerformanceCue,
                SmoothLayerRole::PropsForeground,
                SmoothLayerRole::TankLifeForeground,
            ]
        );
    }

    #[test]
    fn adds_chest_bubble_layer_for_treasure_chest_fixture() {
        let vm = with_treasure_chest(active_prop_rich_vm());
        let (scene_model, scene, ctx, tank_geometry) = active_scene_inputs(&vm);

        let layered = render_layered_pet_scene_with_tank_geometry(
            &scene_model,
            &vm,
            &scene,
            LAYERED_NOW,
            &ctx,
            &tank_geometry,
        );

        assert_eq!(
            non_empty_roles(&layered),
            vec![
                SmoothLayerRole::BiomeWash,
                SmoothLayerRole::RoomGlyphs,
                SmoothLayerRole::Ambient,
                SmoothLayerRole::Motes,
                SmoothLayerRole::ActivityGlyphs,
                SmoothLayerRole::PropsBehind,
                SmoothLayerRole::TankLifeBehind,
                SmoothLayerRole::ChestBubble,
                SmoothLayerRole::ContactShadow,
                SmoothLayerRole::PetBody,
                SmoothLayerRole::PerformanceCue,
                SmoothLayerRole::PropsForeground,
                SmoothLayerRole::TankLifeForeground,
            ]
        );
    }

    #[test]
    fn layer_from_draw_cells_localizes_cells_and_bounds_from_top_left_anchor() {
        let layer = super::layer_from_draw_cells(
            "test-pet-body",
            SmoothLayerRole::PetBody,
            9,
            vec![
                DrawCell {
                    row: 7,
                    col: 11,
                    glyph: Some("A".to_string()),
                    fg: None,
                    bg: None,
                    bold: false,
                },
                DrawCell {
                    row: 9,
                    col: 14,
                    glyph: Some("B".to_string()),
                    fg: None,
                    bg: None,
                    bold: true,
                },
            ],
        );

        assert_eq!(layer.anchor, SmoothPoint { x: 11.0, y: 7.0 });
        assert_eq!(
            layer.local_bounds,
            SmoothBounds {
                min: SmoothPoint { x: 0.0, y: 0.0 },
                max: SmoothPoint { x: 4.0, y: 3.0 },
            }
        );
        assert_eq!(
            layer.items,
            vec![
                SmoothLayerItem::LocalCell(crate::presentation::smooth::SmoothLocalCell {
                    row: 0,
                    col: 0,
                    glyph: Some("A".to_string()),
                    fg: None,
                    bg: None,
                    bold: false,
                }),
                SmoothLayerItem::LocalCell(crate::presentation::smooth::SmoothLocalCell {
                    row: 2,
                    col: 3,
                    glyph: Some("B".to_string()),
                    fg: None,
                    bg: None,
                    bold: true,
                }),
            ]
        );
    }

    #[test]
    fn flattening_fixed_fixture_matches_digest_lock() {
        let vm = active_prop_rich_vm();
        let (scene_model, scene, ctx, tank_geometry) = active_scene_inputs(&vm);
        let layered = render_layered_pet_scene_with_tank_geometry(
            &scene_model,
            &vm,
            &scene,
            LAYERED_NOW,
            &ctx,
            &tank_geometry,
        );

        insta::assert_yaml_snapshot!(
            "layered_active_fixture_flatten_digest",
            flatten_digest(&layered.flatten_classic_cells().cells)
        );
    }

    #[test]
    fn flattening_fixed_fixture_matches_legacy_parity_oracle() {
        let vm = active_prop_rich_vm();
        let (scene_model, scene, ctx, tank_geometry) = active_scene_inputs(&vm);

        let layered = render_layered_pet_scene_with_tank_geometry(
            &scene_model,
            &vm,
            &scene,
            LAYERED_NOW,
            &ctx,
            &tank_geometry,
        );
        let legacy = render_legacy_pet_scene_draw_list_parity_oracle_with_tank_geometry(
            &scene_model,
            &vm,
            &scene,
            LAYERED_NOW,
            &ctx,
            &tank_geometry,
        );

        assert_eq!(layered.flatten_classic_cells(), legacy);
    }
}
