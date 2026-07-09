use ratatui::layout::Rect;

use crate::presentation::smooth::{
    smooth_pet_bob, CompanionChromeReservation, CompanionViewport, SmoothBlendMode, SmoothBounds,
    SmoothClassicFlattenCompat, SmoothClip, SmoothCompanionLayer, SmoothCompanionPet,
    SmoothCompanionPrivacyClaims, SmoothCompanionScenePlan, SmoothLayerId,
    SmoothLayerMotionBinding, SmoothLayerRole, SmoothPoint, SmoothTransform,
};
use crate::presentation::PetSceneModel;
use crate::round::layout::{
    layout_round_scene, RoundAnchorKind, RoundAperture, RoundRenderCapabilities,
};
use crate::round::model::{derive_round_scene_model, RoundHelperHealth};
use crate::round::scene::{
    build_round_pet_layout_with_placement, round_tank_life_geometry, CompanionMotion,
};
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothScenePlanError {
    MissingPetBody,
    InvalidParallaxGeometry,
}

impl std::fmt::Display for SmoothScenePlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmoothScenePlanError::MissingPetBody => f.write_str("smooth scene missing pet body"),
            SmoothScenePlanError::InvalidParallaxGeometry => {
                f.write_str("smooth scene has invalid parallax geometry")
            }
        }
    }
}

impl std::error::Error for SmoothScenePlanError {}

pub fn try_build_round_smooth_scene_plan(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    elapsed_ms: u64,
) -> std::result::Result<SmoothCompanionScenePlan, SmoothScenePlanError> {
    let (vm, layout, placement) =
        build_round_pet_layout_with_placement(vm, now, grid_cols, grid_rows, motion);
    let vm = vm.as_ref();
    let ctx = RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(now));
    let model = PetSceneModel::build(vm, now, ColorCapability::Truecolor);
    let tank_geometry = round_tank_life_geometry(grid_cols, grid_rows);
    let layered = crate::tui::panels::pet::render_layered_pet_scene_with_tank_geometry(
        &model,
        vm,
        &layout,
        now,
        &ctx,
        &tank_geometry,
    );

    let viewport = CompanionViewport { grid_cols, grid_rows };
    let viewport_bounds = rect_bounds(Rect::new(0, 0, grid_cols, grid_rows));
    let pet_body_classic_anchor = layered
        .layers
        .iter()
        .find(|layer| layer.role == SmoothLayerRole::PetBody)
        .map(|layer| layer.anchor)
        .ok_or(SmoothScenePlanError::MissingPetBody)?;
    let smooth_base_anchor = SmoothPoint {
        x: placement.fractional_motion_top_left.x,
        y: placement.fractional_motion_top_left.y,
    };
    let pet_anchor_delta = SmoothPoint {
        x: smooth_base_anchor.x - pet_body_classic_anchor.x,
        y: smooth_base_anchor.y - pet_body_classic_anchor.y,
    };
    let parallax_focus_offset = SmoothPoint {
        x: placement.fractional_motion_top_left.x - placement.fractional_motion_origin_top_left.x,
        y: placement.fractional_motion_top_left.y - placement.fractional_motion_origin_top_left.y,
    };
    let round_scene = derive_round_scene_model(vm, now);
    let parallax_lifecycle_scale = crate::round::parallax::parallax_lifecycle_scale(
        round_scene.lifecycle.asleep,
        round_scene.lifecycle.calm,
    );
    let bob_offset = SmoothPoint { x: 0.0, y: smooth_pet_bob(elapsed_ms) };
    let aperture_center = SmoothPoint {
        x: f32::from(grid_cols) / 2.0,
        y: f32::from(grid_rows) / 2.0,
    };
    let aperture_radius = f32::from(grid_cols.min(grid_rows)) / 2.0;

    let mut layers = Vec::with_capacity(layered.layers.len() + 5);
    layers.push(reservation_layer(
        "round-depth-rings",
        SmoothLayerRole::DepthRings,
        -10,
        viewport_bounds,
        aperture_center,
        SmoothClip::Circle {
            center: aperture_center,
            radius: aperture_radius.max(0.0),
        },
        0.25,
    ));

    for mut layer in layered.layers {
        match layer.motion_binding {
            SmoothLayerMotionBinding::PetAttached => {
                layer.transform.translation.x += pet_anchor_delta.x;
                layer.transform.translation.y += pet_anchor_delta.y;
            }
            // The floor projection follows the pet across the tank, but stays
            // anchored to the substrate while the pet bobs against the wall.
            SmoothLayerMotionBinding::FloorProjected => {
                layer.transform.translation.x += pet_anchor_delta.x;
                layer.transform.translation.y -= 1.0;
                // Draw over the room grid but beneath every prop layer, so the
                // projection reads as a floor treatment rather than an occluder.
                layer.z = 1;
            }
            SmoothLayerMotionBinding::Fixed | SmoothLayerMotionBinding::Parallax(_) => {}
        }
        if layer.role == SmoothLayerRole::PetBody {
            layer.transform_origin = SmoothPoint {
                x: (layer.local_bounds.max.x - layer.local_bounds.min.x) / 2.0,
                y: (layer.local_bounds.max.y - layer.local_bounds.min.y) / 2.0,
            };
            layer.transform.translation.y += bob_offset.y;
        }
        layers.push(layer);
    }

    let pet_body = layers
        .iter()
        .find(|layer| layer.role == SmoothLayerRole::PetBody)
        .ok_or(SmoothScenePlanError::MissingPetBody)?;
    let classic_snap_anchor = pet_body.anchor;
    let base_anchor = SmoothPoint {
        x: pet_body.anchor.x + pet_body.transform.translation.x - bob_offset.x,
        y: pet_body.anchor.y + pet_body.transform.translation.y - bob_offset.y,
    };
    let final_anchor = SmoothPoint {
        x: pet_body.anchor.x + pet_body.transform.translation.x,
        y: pet_body.anchor.y + pet_body.transform.translation.y,
    };
    let pet_bounds = anchored_bounds(pet_body.anchor, pet_body.local_bounds);
    let fractional_pet_bounds = anchored_bounds(final_anchor, pet_body.local_bounds);
    let fractional_pet_center = bounds_center(fractional_pet_bounds);

    let round_layout = layout_round_scene(
        &round_scene,
        RoundAperture::new(grid_cols, grid_rows),
        RoundRenderCapabilities::preview_truecolor(),
    );
    let has_status_halo = round_layout
        .halo_anchors
        .iter()
        .any(|anchor| anchor.kind != RoundAnchorKind::HelperTrouble);
    let has_trouble = round_scene.halo.helper_health == RoundHelperHealth::Trouble;

    layers.push(reservation_layer(
        "round-status-halo",
        SmoothLayerRole::StatusHalo,
        20,
        viewport_bounds,
        aperture_center,
        SmoothClip::Circle {
            center: aperture_center,
            radius: aperture_radius.max(0.0),
        },
        if has_status_halo { 1.0 } else { 0.0 },
    ));
    layers.push(reservation_layer(
        "round-trouble-indicator",
        SmoothLayerRole::TroubleIndicator,
        21,
        viewport_bounds,
        aperture_center,
        SmoothClip::Circle {
            center: aperture_center,
            radius: aperture_radius.max(0.0),
        },
        if has_trouble { 1.0 } else { 0.0 },
    ));
    layers.push(reservation_layer(
        "round-mood-aura",
        SmoothLayerRole::MoodAura,
        22,
        expand_bounds(fractional_pet_bounds, 2.0, 2.0),
        fractional_pet_center,
        SmoothClip::Circle {
            center: fractional_pet_center,
            radius: ((fractional_pet_bounds.max.x - fractional_pet_bounds.min.x)
                .max(fractional_pet_bounds.max.y - fractional_pet_bounds.min.y))
                / 2.0
                + 2.0,
        },
        1.0,
    ));
    layers.push(reservation_layer(
        "round-dim-overlay",
        SmoothLayerRole::DimOverlay,
        23,
        viewport_bounds,
        aperture_center,
        SmoothClip::Circle {
            center: aperture_center,
            radius: aperture_radius.max(0.0),
        },
        if round_scene.lifecycle.asleep || round_scene.lifecycle.calm {
            0.35
        } else {
            0.0
        },
    ));

    let chrome = CompanionChromeReservation {
        hud_bounds: tank_geometry
            .reserved_regions
            .iter()
            .copied()
            .map(rect_bounds)
            .collect(),
        gauge_bounds: gauge_bounds(grid_cols, grid_rows),
    };

    for layer in &mut layers {
        let parallax_translation = crate::round::parallax::resolve_layer_parallax(
            parallax_focus_offset,
            parallax_lifecycle_scale,
            layer,
            viewport,
            &chrome,
        )
        .map_err(|_| SmoothScenePlanError::InvalidParallaxGeometry)?;
        layer.parallax_translation = parallax_translation;
        layer.transform.translation.x += parallax_translation.x;
        layer.transform.translation.y += parallax_translation.y;
    }

    Ok(SmoothCompanionScenePlan {
        viewport,
        layers,
        pet: SmoothCompanionPet {
            bounds: pet_bounds,
            fractional_bounds: fractional_pet_bounds,
            base_anchor,
            bob_offset,
            final_anchor,
            classic_snap_anchor,
            parallax_focus_offset,
        },
        parallax_lifecycle_scale,
        chrome,
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
        classic_flatten_compat: SmoothClassicFlattenCompat::UniformPortholeRecolor { grid_rows },
    })
}

pub fn build_round_smooth_scene_plan(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    elapsed_ms: u64,
) -> SmoothCompanionScenePlan {
    try_build_round_smooth_scene_plan(vm, now, grid_cols, grid_rows, motion, elapsed_ms)
        .expect("round smooth scene should include a pet body layer")
}

fn reservation_layer(
    id: &str,
    role: SmoothLayerRole,
    z: i16,
    local_bounds: SmoothBounds,
    transform_origin: SmoothPoint,
    clip: SmoothClip,
    opacity: f32,
) -> SmoothCompanionLayer {
    SmoothCompanionLayer {
        id: SmoothLayerId(id.to_string()),
        role,
        motion_binding: role.motion_binding(),
        z,
        local_bounds,
        anchor: SmoothPoint { x: 0.0, y: 0.0 },
        transform_origin,
        transform: SmoothTransform {
            translation: SmoothPoint { x: 0.0, y: 0.0 },
            scale: SmoothPoint { x: 1.0, y: 1.0 },
            rotation_degrees: 0.0,
        },
        parallax_translation: SmoothPoint { x: 0.0, y: 0.0 },
        opacity,
        clip,
        blend: SmoothBlendMode::Normal,
        items: Vec::new(),
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
    }
}

fn rect_bounds(rect: Rect) -> SmoothBounds {
    SmoothBounds {
        min: SmoothPoint {
            x: f32::from(rect.x),
            y: f32::from(rect.y),
        },
        max: SmoothPoint {
            x: f32::from(rect.x + rect.width),
            y: f32::from(rect.y + rect.height),
        },
    }
}

fn anchored_bounds(anchor: SmoothPoint, local_bounds: SmoothBounds) -> SmoothBounds {
    SmoothBounds {
        min: SmoothPoint {
            x: anchor.x + local_bounds.min.x,
            y: anchor.y + local_bounds.min.y,
        },
        max: SmoothPoint {
            x: anchor.x + local_bounds.max.x,
            y: anchor.y + local_bounds.max.y,
        },
    }
}

fn bounds_center(bounds: SmoothBounds) -> SmoothPoint {
    SmoothPoint {
        x: bounds.min.x + (bounds.max.x - bounds.min.x) / 2.0,
        y: bounds.min.y + (bounds.max.y - bounds.min.y) / 2.0,
    }
}

fn expand_bounds(bounds: SmoothBounds, pad_x: f32, pad_y: f32) -> SmoothBounds {
    SmoothBounds {
        min: SmoothPoint {
            x: bounds.min.x - pad_x,
            y: bounds.min.y - pad_y,
        },
        max: SmoothPoint {
            x: bounds.max.x + pad_x,
            y: bounds.max.y + pad_y,
        },
    }
}

fn gauge_bounds(grid_cols: u16, grid_rows: u16) -> Vec<SmoothBounds> {
    let lane = (f32::from(grid_cols.min(grid_rows)) / 8.0).max(1.0);
    vec![
        SmoothBounds {
            min: SmoothPoint { x: 0.0, y: 0.0 },
            max: SmoothPoint { x: lane, y: f32::from(grid_rows) },
        },
        SmoothBounds {
            min: SmoothPoint { x: f32::from(grid_cols) - lane, y: 0.0 },
            max: SmoothPoint {
                x: f32::from(grid_cols),
                y: f32::from(grid_rows),
            },
        },
        SmoothBounds {
            min: SmoothPoint { x: 0.0, y: 0.0 },
            max: SmoothPoint { x: f32::from(grid_cols), y: lane },
        },
    ]
}
