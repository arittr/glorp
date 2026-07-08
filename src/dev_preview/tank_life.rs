use crate::dev_preview::contract::{
    PreviewTankLifeArtifact, PreviewTankLifeCollisionArtifact, PreviewTankLifeLayerArtifact,
    PreviewTankLifePlacementArtifact, PreviewTankLifeSkipArtifact, PreviewTargetArtifact,
    TANK_LIFE_CONTRACT_SCHEMA_VERSION,
};
use crate::dev_preview::scenarios::{PreviewRenderContext, PreviewScenarioBundle};
use crate::error::{GlorpError, Result};
use crate::game::habitat::{self, HabitatPetLayer, TankLifeRouteFamily};
use crate::tui::component::{
    anemone_morph_for_day, canonical_daily_cast, layer_segment_summaries,
    pet_face_protected_regions, project_tank_life_cast, rect_contains, tank_life_placements_for,
    watch_tank_life_geometry, PetScene, RenderedTankLifeCast, TankLifePlacement,
    TankLifeRenderInput, TankLifeSkipReason, TankLifeSurface, TankLifeSurfaceGeometry, TargetPath,
};
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;
use ratatui::layout::Rect;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub fn tank_life_bundles(
    ctx: &crate::dev_preview::scenarios::PreviewRenderContext,
    scratch_dir: &std::path::Path,
) -> crate::error::Result<Vec<crate::dev_preview::scenarios::PreviewScenarioBundle>> {
    tank_life_fixtures()
        .iter()
        .map(|fixture| render_tank_life_fixture(ctx, scratch_dir, fixture))
        .collect()
}

struct TankLifeFixture {
    id: &'static str,
    title: &'static str,
    width: u16,
    height: u16,
    surface: TankLifeSurface,
    local_date: time::Date,
    age_days: i64,
}

fn tank_life_fixtures() -> Vec<TankLifeFixture> {
    vec![
        TankLifeFixture {
            id: "tank-life-age-empty",
            title: "Tank Life Age Empty",
            width: 120,
            height: 32,
            surface: TankLifeSurface::Watch,
            local_date: time::macros::date!(2026 - 07 - 07),
            age_days: 0,
        },
        TankLifeFixture {
            id: "tank-life-age-first",
            title: "Tank Life Age First",
            width: 120,
            height: 32,
            surface: TankLifeSurface::Watch,
            local_date: time::macros::date!(2026 - 07 - 07),
            age_days: 1,
        },
        TankLifeFixture {
            id: "tank-life-age-early",
            title: "Tank Life Age Early",
            width: 120,
            height: 32,
            surface: TankLifeSurface::Watch,
            local_date: time::macros::date!(2026 - 07 - 07),
            age_days: 7,
        },
        TankLifeFixture {
            id: "tank-life-age-full",
            title: "Tank Life Age Full",
            width: 120,
            height: 32,
            surface: TankLifeSurface::Watch,
            local_date: time::macros::date!(2026 - 07 - 07),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-date-2026-07-07",
            title: "Tank Life Date 2026-07-07",
            width: 120,
            height: 32,
            surface: TankLifeSurface::Watch,
            local_date: time::macros::date!(2026 - 07 - 07),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-date-2026-07-08",
            title: "Tank Life Date 2026-07-08",
            width: 120,
            height: 32,
            surface: TankLifeSurface::Watch,
            local_date: time::macros::date!(2026 - 07 - 08),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-round-projection",
            title: "Tank Life Round Projection",
            width: 44,
            height: 18,
            surface: TankLifeSurface::Round,
            local_date: time::macros::date!(2026 - 07 - 08),
            age_days: 60,
        },
        TankLifeFixture {
            id: "tank-life-anemone-morphs",
            title: "Tank Life Anemone Morphs",
            width: 120,
            height: 32,
            surface: TankLifeSurface::Watch,
            local_date: time::macros::date!(2026 - 07 - 09),
            age_days: 60,
        },
    ]
}

fn render_tank_life_fixture(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: &TankLifeFixture,
) -> Result<PreviewScenarioBundle> {
    let _ = scratch_dir;
    let now = fixture
        .local_date
        .with_time(ctx.fixed_now.time())
        .assume_utc();
    let vm =
        WatchViewModel::fixture_with_tank_inhabitants_for_age(fixture.age_days, fixture.local_date);
    let canonical = canonical_daily_cast(
        &vm.habitat.earned_inhabitants,
        &vm.pet_render.seed,
        vm.habitat.tank_life_local_date,
        vm.habitat.tank_life_calendar_age_days,
    );

    let (mut frame, geometry, pet_protected_regions, placements, projected) = match fixture.surface
    {
        TankLifeSurface::Watch => {
            let render =
                RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(now));
            let watch_layout = crate::tui::component::layout_watch_with_context(
                Rect::new(0, 0, fixture.width, fixture.height),
                &vm,
                &render,
            );
            let pet_panel = watch_layout
                .target(TargetPath::new("watch.pet.panel"))
                .expect("watch preview layout should expose pet panel target")
                .rect;
            let scene = PetScene::compute_layout(pet_panel, &vm, &render);
            let geometry = watch_tank_life_geometry(&scene);
            let pet_protected_regions = pet_face_protected_regions(scene.pet_art);
            let projected = project_tank_life_cast(&canonical, &geometry);
            let placements = tank_life_placements_for(&TankLifeRenderInput {
                rendered_ids: projected.rendered_ids.clone(),
                pet_seed: &vm.pet_render.seed,
                local_date: vm.habitat.tank_life_local_date,
                now,
                geometry: &geometry,
                pet_protected_regions: &pet_protected_regions,
                color_capability: render.color_capability,
                life_profile: vm.life_profile.clone(),
            });
            let frame = crate::dev_preview::watch::render_watch_preview_frame_from_view_model(
                fixture.id,
                fixture.title,
                &vm,
                now,
                fixture.width,
                fixture.height,
                ColorCapability::Truecolor,
            )?;
            (
                frame,
                geometry,
                pet_protected_regions,
                placements,
                projected,
            )
        }
        TankLifeSurface::Round => {
            let scene = crate::round::scene::build_round_scene_draw_list(
                &vm,
                now,
                fixture.width,
                fixture.height,
                &crate::round::scene::CompanionMotion::default(),
            );
            let geometry =
                crate::round::scene::round_tank_life_geometry(fixture.width, fixture.height);
            let pet_protected_regions = pet_face_protected_regions(scene.pet_rect);
            let projected = project_tank_life_cast(&canonical, &geometry);
            let placements = tank_life_placements_for(&TankLifeRenderInput {
                rendered_ids: projected.rendered_ids.clone(),
                pet_seed: &vm.pet_render.seed,
                local_date: vm.habitat.tank_life_local_date,
                now,
                geometry: &geometry,
                pet_protected_regions: &pet_protected_regions,
                color_capability: ColorCapability::Truecolor,
                life_profile: vm.life_profile.clone(),
            });
            let frame = crate::dev_preview::round::render_round_preview_frame(
                fixture.id,
                fixture.title,
                &vm,
                now,
                fixture.width,
                fixture.height,
                crate::round::layout::RoundRenderCapabilities::preview_truecolor(),
            );
            (
                frame,
                geometry,
                pet_protected_regions,
                placements,
                projected,
            )
        }
        TankLifeSurface::Menubar => {
            return Err(GlorpError::Message(
                "tank-life preview fixtures do not target the menubar surface".to_string(),
            ));
        }
    };

    let mut artifact = tank_life_artifact_for_frame(
        fixture.id,
        &vm,
        fixture.surface,
        &geometry,
        &placements,
        &projected,
    );
    artifact.collision_status =
        collision_status_for(&geometry, &pet_protected_regions, &placements);

    frame.extra_inputs = tank_life_inputs_for_fixture(fixture, &artifact);
    frame.contract.tank_life = Some(artifact);
    Ok(PreviewScenarioBundle::from_frame(frame, ctx))
}

fn tank_life_artifact_for_frame(
    frame_id: &str,
    vm: &WatchViewModel,
    surface: TankLifeSurface,
    geometry: &TankLifeSurfaceGeometry,
    placements: &[TankLifePlacement],
    projected: &RenderedTankLifeCast,
) -> PreviewTankLifeArtifact {
    let anemone_present = projected
        .canonical_ids
        .iter()
        .chain(projected.rendered_ids.iter())
        .any(|id| id.as_str() == habitat::ANEMONE_HOST);

    PreviewTankLifeArtifact {
        schema_version: TANK_LIFE_CONTRACT_SCHEMA_VERSION,
        frame_id: frame_id.to_string(),
        local_date: vm.habitat.tank_life_local_date.to_string(),
        calendar_age_days: vm.habitat.tank_life_calendar_age_days,
        target_surface: surface_label(surface).to_string(),
        canonical_ids: projected
            .canonical_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
        rendered_ids: projected
            .rendered_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
        skipped: projected
            .skipped
            .iter()
            .map(|skip| PreviewTankLifeSkipArtifact {
                id: skip.id.as_str().to_string(),
                reason: skip_reason_label(skip.reason).to_string(),
            })
            .collect(),
        anemone_morph: anemone_present.then(|| {
            anemone_morph_label(anemone_morph_for_day(
                &vm.pet_render.seed,
                vm.habitat.tank_life_local_date,
            ))
            .to_string()
        }),
        placements: placements
            .iter()
            .map(|placement| PreviewTankLifePlacementArtifact {
                id: placement.inhabitant_id.as_str().to_string(),
                route_family: habitat::tank_inhabitant_spec(&placement.inhabitant_id)
                    .map(|spec| route_family_label(spec.route_family).to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                bounds: PreviewTargetArtifact {
                    role: "placement-bounds".to_string(),
                    layer: "tank-life".to_string(),
                    x: placement.bounds.x,
                    y: placement.bounds.y,
                    width: placement.bounds.width,
                    height: placement.bounds.height,
                },
                cell_count: placement.cells.len(),
            })
            .collect(),
        layer_segments: layer_segment_summaries(placements)
            .into_iter()
            .map(|segment| PreviewTankLifeLayerArtifact {
                id: segment.inhabitant_id.as_str().to_string(),
                pet_layer: pet_layer_label(segment.pet_layer).to_string(),
                cell_count: segment.cell_count,
            })
            .collect(),
        collision_status: PreviewTankLifeCollisionArtifact {
            reserved_region_clear: placements_clear_reserved_regions(geometry, placements),
            aperture_clear: placements_clear_aperture(geometry, placements),
            protected_pet_face_clear: true,
        },
    }
}

fn tank_life_inputs_for_fixture(
    fixture: &TankLifeFixture,
    artifact: &PreviewTankLifeArtifact,
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("local_date".to_string(), json!(artifact.local_date)),
        (
            "calendar_age_days".to_string(),
            json!(artifact.calendar_age_days),
        ),
        (
            "target_surface".to_string(),
            json!(surface_label(fixture.surface)),
        ),
        ("canonical_ids".to_string(), json!(artifact.canonical_ids)),
        ("rendered_ids".to_string(), json!(artifact.rendered_ids)),
        (
            "skipped".to_string(),
            json!(artifact
                .skipped
                .iter()
                .map(|skip| json!({"id": skip.id, "reason": skip.reason}))
                .collect::<Vec<_>>()),
        ),
        ("width".to_string(), json!(fixture.width)),
        ("height".to_string(), json!(fixture.height)),
        ("anemone_morph".to_string(), json!(artifact.anemone_morph)),
    ])
}

fn collision_status_for(
    geometry: &TankLifeSurfaceGeometry,
    pet_protected_regions: &[Rect],
    placements: &[TankLifePlacement],
) -> PreviewTankLifeCollisionArtifact {
    PreviewTankLifeCollisionArtifact {
        reserved_region_clear: placements_clear_reserved_regions(geometry, placements),
        aperture_clear: placements_clear_aperture(geometry, placements),
        protected_pet_face_clear: placements
            .iter()
            .flat_map(|placement| &placement.cells)
            .all(|cell| {
                cell.pet_layer != HabitatPetLayer::Foreground
                    || !pet_protected_regions
                        .iter()
                        .any(|region| rect_contains(*region, cell.col, cell.row))
            }),
    }
}

fn placements_clear_reserved_regions(
    geometry: &TankLifeSurfaceGeometry,
    placements: &[TankLifePlacement],
) -> bool {
    placements
        .iter()
        .flat_map(|placement| &placement.cells)
        .all(|cell| {
            !geometry
                .reserved_regions
                .iter()
                .any(|region| rect_contains(*region, cell.col, cell.row))
        })
}

fn placements_clear_aperture(
    geometry: &TankLifeSurfaceGeometry,
    placements: &[TankLifePlacement],
) -> bool {
    placements
        .iter()
        .flat_map(|placement| &placement.cells)
        .all(|cell| geometry.cell_inside_aperture(cell.col, cell.row))
}

fn surface_label(surface: TankLifeSurface) -> &'static str {
    match surface {
        TankLifeSurface::Watch => "watch",
        TankLifeSurface::Round => "round",
        TankLifeSurface::Menubar => "menubar",
    }
}

fn skip_reason_label(reason: TankLifeSkipReason) -> &'static str {
    match reason {
        TankLifeSkipReason::UnknownCatalogId => "unknown-catalog-id",
        TankLifeSkipReason::SurfaceBudget => "surface-budget",
        TankLifeSkipReason::HabitatTooSmall => "habitat-too-small",
        TankLifeSkipReason::ReservedRegionCollision => "reserved-region-collision",
        TankLifeSkipReason::ApertureCollision => "aperture-collision",
    }
}

fn route_family_label(route_family: TankLifeRouteFamily) -> &'static str {
    match route_family {
        TankLifeRouteFamily::CrossTankSwimmer => "cross-tank-swimmer",
        TankLifeRouteFamily::LowerLaneResident => "lower-lane-resident",
        TankLifeRouteFamily::GlassResident => "glass-resident",
        TankLifeRouteFamily::RimResident => "rim-resident",
        TankLifeRouteFamily::LowerEdgeResident => "lower-edge-resident",
        TankLifeRouteFamily::HostCombo => "host-combo",
    }
}

fn pet_layer_label(layer: HabitatPetLayer) -> &'static str {
    match layer {
        HabitatPetLayer::Background => "background",
        HabitatPetLayer::Behind => "behind",
        HabitatPetLayer::Foreground => "foreground",
    }
}

fn anemone_morph_label(morph: crate::tui::component::AnemoneMorph) -> &'static str {
    match morph {
        crate::tui::component::AnemoneMorph::Flower => "flower",
        crate::tui::component::AnemoneMorph::Comb => "comb",
        crate::tui::component::AnemoneMorph::Crown => "crown",
        crate::tui::component::AnemoneMorph::DotColony => "dot-colony",
    }
}
