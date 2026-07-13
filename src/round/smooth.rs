use std::collections::BTreeSet;

use ratatui::layout::Rect;

use crate::pet::generation::Species;
use crate::pet::render::{ART_HEIGHT, ART_WIDTH, FRAME_HEIGHT, FRAME_WIDTH};
use crate::presentation::smooth::{
    smooth_pet_bob, transformed_smooth_bounds, validate_smooth_layer, CompanionChromeReservation,
    CompanionViewport, SmoothBlendMode, SmoothBounds, SmoothClassicFlattenCompat, SmoothClip,
    SmoothCompanionLayer, SmoothCompanionPet, SmoothCompanionPrivacyClaims,
    SmoothCompanionScenePlan, SmoothGeometryError, SmoothLayerId, SmoothLayerItem,
    SmoothLayerMotionBinding, SmoothLayerRole, SmoothPoint, SmoothShape, SmoothShapeGeometry,
    SmoothTransform,
};
use crate::presentation::PetSceneModel;
use crate::round::depth::{
    depth_lifecycle_scale, resolve_smooth_depth, SmoothDepthError, SMOOTH_PERSPECTIVE_Y_MAX,
    SMOOTH_PET_NEAR_SCALE,
};
use crate::round::layout::{
    layout_round_scene, RoundAnchorKind, RoundAperture, RoundRenderCapabilities,
};
use crate::round::model::{derive_round_scene_model, RoundHelperHealth};
use crate::round::scene::{
    build_round_pet_layout_with_placement, round_tank_life_geometry, CompanionMotion,
};
use crate::round::tank_bed::{
    smooth_floor_projection_shape, smooth_tank_bed_geometry, SmoothTankBedGeometry,
};
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::room::RoomSpeciesDialect;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothScenePlanError {
    MissingPetBody,
    InvalidParallaxGeometry,
    InvalidDepth(SmoothDepthError),
    InvalidLayerGeometry(SmoothGeometryError),
}

impl std::fmt::Display for SmoothScenePlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmoothScenePlanError::MissingPetBody => f.write_str("smooth scene missing pet body"),
            SmoothScenePlanError::InvalidParallaxGeometry => {
                f.write_str("smooth scene has invalid parallax geometry")
            }
            SmoothScenePlanError::InvalidDepth(error) => write!(f, "smooth scene depth: {error}"),
            SmoothScenePlanError::InvalidLayerGeometry(error) => {
                write!(f, "smooth scene layer geometry: {error:?}")
            }
        }
    }
}

impl std::error::Error for SmoothScenePlanError {}

/// Deterministic overrides for scene construction. Normal companion runs leave
/// this at its default; Preview Lab and native review pin a depth so far, neutral,
/// and near frames can be captured without waiting for the roam cycle.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SmoothSceneBuildOptions {
    pub depth_override: Option<f32>,
}

pub fn try_build_round_smooth_scene_plan(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    elapsed_ms: u64,
) -> std::result::Result<SmoothCompanionScenePlan, SmoothScenePlanError> {
    try_build_round_smooth_scene_plan_with_options(
        vm,
        now,
        grid_cols,
        grid_rows,
        motion,
        elapsed_ms,
        SmoothSceneBuildOptions::default(),
    )
}

#[doc(hidden)]
pub fn try_build_round_smooth_scene_plan_with_options(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    elapsed_ms: u64,
    options: SmoothSceneBuildOptions,
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
    let tank_bed = smooth_tank_bed_geometry(viewport, model.room.biome);
    let pet_body_source = layered
        .layers
        .iter()
        .find(|layer| layer.role == SmoothLayerRole::PetBody)
        .ok_or(SmoothScenePlanError::MissingPetBody)?;
    let pet_body_classic_anchor = pet_body_source.anchor;
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

    // One depth sample per frame drives pet scale, perspective, and the projection.
    let raw_depth = options.depth_override.unwrap_or(placement.raw_depth);
    let depth = resolve_smooth_depth(
        raw_depth,
        depth_lifecycle_scale(round_scene.lifecycle.asleep, round_scene.lifecycle.calm),
    )
    .map_err(SmoothScenePlanError::InvalidDepth)?;
    let perspective_offset = SmoothPoint { x: 0.0, y: depth.perspective_y };

    // The pet anchor is the particle frame's top-left and the creature art is
    // centred inside it, so the frame's centre is the creature's centre. Every
    // pet-attached layer scales about that one pre-translation pivot, keeping the
    // shadow and the cue locked to the creature as it swims toward the near glass.
    //
    // A layer's `local_bounds` is its cell bounding box, whose centre is *not* the
    // creature's centre, so it cannot serve as the pivot.
    let frame_center = SmoothPoint {
        x: FRAME_WIDTH as f32 / 2.0,
        y: FRAME_HEIGHT as f32 / 2.0,
    };
    let pet_pivot = SmoothPoint {
        x: pet_body_classic_anchor.x + frame_center.x,
        y: pet_body_classic_anchor.y + frame_center.y,
    };
    // The pet's roam centre, free of idle bob and perspective. This is the point
    // the roam envelope clamps, so the clearance promise is expressed around it.
    let roam_center = SmoothPoint {
        x: smooth_base_anchor.x + frame_center.x,
        y: smooth_base_anchor.y + frame_center.y,
    };
    let max_scale_clearance = max_scale_clearance_bounds(roam_center);

    let bob_offset = SmoothPoint { x: 0.0, y: smooth_pet_bob(elapsed_ms) };
    let aperture_center = SmoothPoint {
        x: f32::from(grid_cols) / 2.0,
        y: f32::from(grid_rows) / 2.0,
    };
    // The aperture is a circle in pixels. Cells are not square, so in cell space
    // it is an ellipse inscribed in the viewport.
    let aperture_radii = SmoothPoint {
        x: f32::from(grid_cols) / 2.0,
        y: f32::from(grid_rows) / 2.0,
    };

    let mut layers = Vec::with_capacity(layered.layers.len() + 5);
    layers.push(reservation_layer(
        "round-depth-rings",
        SmoothLayerRole::DepthRings,
        -10,
        viewport_bounds,
        aperture_center,
        SmoothClip::Ellipse {
            center: aperture_center,
            radii: aperture_radii,
        },
        0.25,
    ));

    let pet_center_x = roam_center.x;
    for mut layer in layered.layers {
        match layer.motion_binding {
            SmoothLayerMotionBinding::PetAttached => {
                layer.transform.translation.x += pet_anchor_delta.x;
                layer.transform.translation.y += pet_anchor_delta.y + perspective_offset.y;
                layer.transform_origin = SmoothPoint {
                    x: pet_pivot.x - layer.anchor.x,
                    y: pet_pivot.y - layer.anchor.y,
                };
                layer.transform.scale = SmoothPoint { x: depth.scale, y: depth.scale };
                // Atmospheric perspective: the far pet recedes into the water
                // rather than merely shrinking against it.
                layer.opacity = (layer.opacity * depth.atmosphere).clamp(0.0, 1.0);
                if layer.role == SmoothLayerRole::WallShadow {
                    // The classic wall shadow repaints the wall wash one step
                    // darker, which presumes an opaquely painted wall behind the
                    // pet. The smooth tank is a dark gradient, so that repaint
                    // reads as nothing. A multiply veil darkens whatever actually
                    // sits beneath it, on any background.
                    layer.blend = SmoothBlendMode::Multiply;
                    for item in &mut layer.items {
                        if let SmoothLayerItem::LocalCell(cell) = item {
                            if cell.bg.is_some() {
                                cell.bg = Some(WALL_SHADOW_MULTIPLY);
                            }
                        }
                    }
                    // The offset between the body and its cast shadow is what
                    // makes Z legible: hugging and dark with the pet against the
                    // rear wall, detached and diffuse as it comes to the glass.
                    // The silhouette cells carry a baked one-cell offset that
                    // scales with the body, so only the difference is added here.
                    let cue = crate::presentation::companion_effects::wall_shadow_depth_cue(
                        depth.effective_z,
                    );
                    let detach = cue.detach_cells;
                    let extra = detach - WALL_SHADOW_BAKED_OFFSET * depth.scale;
                    layer.transform.translation.x += extra;
                    layer.transform.translation.y += extra;
                    // Strength encodes distance from the wall, which runs
                    // opposite to the water's atmosphere; it replaces the
                    // atmosphere fade applied above.
                    layer.opacity = cue.strength;
                }
            }
            // The floor projection follows the pet across the tank, but stays
            // anchored to the bed while the pet bobs against the wall.
            SmoothLayerMotionBinding::FloorProjected => {
                // Draw over the room grid but beneath every prop layer, so the
                // projection reads as a floor treatment rather than an occluder.
                layer.z = 1;
                // A shadow darkens what the bed shows beneath it. Painting a dark
                // colour over a dark floor reads as nothing.
                layer.blend = SmoothBlendMode::Multiply;
                match tank_bed.as_ref().and_then(|bed| {
                    smooth_floor_projection_shape(viewport, bed, pet_center_x, depth)
                }) {
                    // The ellipse is already positioned in viewport coordinates
                    // from the pet's centre and the depth sample, so the layer
                    // carries no transform of its own and cannot inherit bob.
                    Some(shape) => {
                        let SmoothShapeGeometry::Ellipse { bounds } = shape.geometry;
                        layer.local_bounds = bounds;
                        layer.anchor = SmoothPoint::default();
                        layer.transform_origin = SmoothPoint::default();
                        layer.transform = SmoothTransform {
                            translation: SmoothPoint::default(),
                            scale: SmoothPoint { x: 1.0, y: 1.0 },
                            rotation_degrees: 0.0,
                        };
                        layer.items = vec![SmoothLayerItem::Shape(shape)];
                    }
                    // A viewport too small to hold a bed keeps the Classic cells.
                    None => {
                        layer.transform.translation.x += pet_anchor_delta.x;
                        layer.transform.translation.y -= 1.0;
                    }
                }
            }
            SmoothLayerMotionBinding::Fixed | SmoothLayerMotionBinding::Parallax(_) => {}
        }
        if layer.role == SmoothLayerRole::PetBody {
            layer.transform.translation.y += bob_offset.y;
        }
        let is_room_glyphs = layer.role == SmoothLayerRole::RoomGlyphs;
        layers.push(layer);
        if is_room_glyphs {
            if let Some(bed) = tank_bed.as_ref() {
                layers.push(tank_bed_layer(viewport, bed));
            }
        }
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
    // The aura tracks the creature as it grows and sinks with depth, so it is
    // derived from the composed pet transform rather than the unscaled art.
    let transformed_pet_bounds =
        transformed_smooth_bounds(pet_body).map_err(SmoothScenePlanError::InvalidLayerGeometry)?;
    let transformed_pet_center = bounds_center(transformed_pet_bounds);

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
        SmoothClip::Ellipse {
            center: aperture_center,
            radii: aperture_radii,
        },
        if has_status_halo { 1.0 } else { 0.0 },
    ));
    layers.push(reservation_layer(
        "round-trouble-indicator",
        SmoothLayerRole::TroubleIndicator,
        21,
        viewport_bounds,
        aperture_center,
        SmoothClip::Ellipse {
            center: aperture_center,
            radii: aperture_radii,
        },
        if has_trouble { 1.0 } else { 0.0 },
    ));
    layers.push(reservation_layer(
        "round-mood-aura",
        SmoothLayerRole::MoodAura,
        22,
        expand_bounds(transformed_pet_bounds, 2.0, 2.0),
        transformed_pet_center,
        SmoothClip::Circle {
            center: transformed_pet_center,
            radius: ((transformed_pet_bounds.max.x - transformed_pet_bounds.min.x)
                .max(transformed_pet_bounds.max.y - transformed_pet_bounds.min.y))
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
        SmoothClip::Ellipse {
            center: aperture_center,
            radii: aperture_radii,
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

    // Nothing malformed may reach the native draw callback.
    for layer in &layers {
        validate_smooth_layer(layer).map_err(SmoothScenePlanError::InvalidLayerGeometry)?;
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
            depth: depth.raw_z,
            scale: depth.scale,
            perspective_offset,
            transformed_bounds: transformed_pet_bounds,
            max_scale_clearance,
        },
        parallax_lifecycle_scale,
        chrome,
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
        classic_flatten_compat: SmoothClassicFlattenCompat::UniformPortholeRecolor { grid_rows },
    })
}

fn tank_bed_layer(
    viewport: CompanionViewport,
    bed: &SmoothTankBedGeometry,
) -> SmoothCompanionLayer {
    let aperture_center = SmoothPoint {
        x: f32::from(viewport.grid_cols) / 2.0,
        y: f32::from(viewport.grid_rows) / 2.0,
    };
    SmoothCompanionLayer {
        id: SmoothLayerId("round-tank-bed".to_string()),
        role: SmoothLayerRole::TankBed,
        motion_binding: SmoothLayerMotionBinding::Fixed,
        z: 1,
        local_bounds: shape_bounds(&bed.shapes),
        anchor: SmoothPoint::default(),
        transform_origin: SmoothPoint::default(),
        transform: SmoothTransform {
            translation: SmoothPoint::default(),
            scale: SmoothPoint { x: 1.0, y: 1.0 },
            rotation_degrees: 0.0,
        },
        parallax_translation: SmoothPoint::default(),
        opacity: 1.0,
        clip: SmoothClip::Ellipse {
            center: aperture_center,
            radii: SmoothPoint {
                x: f32::from(viewport.grid_cols) / 2.0,
                y: f32::from(viewport.grid_rows) / 2.0,
            },
        },
        blend: SmoothBlendMode::Normal,
        items: bed
            .shapes
            .iter()
            .copied()
            .map(SmoothLayerItem::Shape)
            .collect(),
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
    }
}

/// The one-cell offset baked into the classic silhouette cells, which scales
/// with the body about the shared pivot.
const WALL_SHADOW_BAKED_OFFSET: f32 = 1.0;

/// Multiply factor for the smooth wall shadow: darkens what it covers by not
/// quite half, with a slightly cool cast so it reads as shade rather than dirt.
const WALL_SHADOW_MULTIPLY: crate::pet::palette::Rgb = crate::pet::palette::Rgb {
    r: crate::presentation::companion_effects::WALL_SHADOW_SRGB8[0],
    g: crate::presentation::companion_effects::WALL_SHADOW_SRGB8[1],
    b: crate::presentation::companion_effects::WALL_SHADOW_SRGB8[2],
};

/// The worst case the pet can ever occupy around a roam centre: creature ink at
/// the maximum depth scale, plus the full perspective excursion in both
/// directions. `smooth_roam_envelope` clamps the roam centre so this stays inside
/// the aperture, the gauges, and the HUD reserve. Clearance is reserved against
/// the art rather than the particle frame; the frame's ambient gutter may graze
/// the HUD reserve, over which the native HUD draws.
fn max_scale_clearance_bounds(roam_center: SmoothPoint) -> SmoothBounds {
    let half_w = ART_WIDTH as f32 / 2.0 * SMOOTH_PET_NEAR_SCALE;
    let half_h = ART_HEIGHT as f32 / 2.0 * SMOOTH_PET_NEAR_SCALE;
    SmoothBounds {
        min: SmoothPoint {
            x: roam_center.x - half_w,
            y: roam_center.y - half_h - SMOOTH_PERSPECTIVE_Y_MAX,
        },
        max: SmoothPoint {
            x: roam_center.x + half_w,
            y: roam_center.y + half_h + SMOOTH_PERSPECTIVE_Y_MAX,
        },
    }
}

fn shape_bounds(shapes: &[SmoothShape]) -> SmoothBounds {
    let mut result = SmoothBounds {
        min: SmoothPoint { x: f32::INFINITY, y: f32::INFINITY },
        max: SmoothPoint {
            x: f32::NEG_INFINITY,
            y: f32::NEG_INFINITY,
        },
    };
    for shape in shapes {
        let SmoothShapeGeometry::Ellipse { bounds } = shape.geometry;
        result.min.x = result.min.x.min(bounds.min.x);
        result.min.y = result.min.y.min(bounds.min.y);
        result.max.x = result.max.x.max(bounds.max.x);
        result.max.y = result.max.y.max(bounds.max.y);
    }
    result
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

/// One glyph the companion could ever paint: a whole authored scalar sequence
/// plus whether it renders bold. Backend-neutral so both the Smooth and retained
/// renderers can consume the repertoire; the retained atlas maps each to a glyph
/// key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepertoireGlyph {
    pub sequence: String,
    pub bold: bool,
}

/// The declared-content identity that determines a companion's glyph atlas: the
/// set of species the atlas must serve.
///
/// Every other axis the companion can paint — all stages, moods, and animation
/// states; the room dialect's full biome/weather/emitter slices; every day-phase
/// sky/floor palette; every earnable prop and tank-life sprite; the whole
/// particle vocabulary — is enumerated into the repertoire in full. So the atlas
/// only changes when the species set changes (or when the font policy / backing
/// scale changes, which the generation key tracks separately). In particular the
/// live room reseeds its random glyph pick every minute, but every candidate is
/// already in the atlas, so that reshuffle never changes this identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionContentIdentity {
    species: Vec<Species>,
}

impl CompanionContentIdentity {
    /// The identity for a set of species, sorted and deduplicated so equal
    /// content produces equal identity bytes.
    pub fn for_species(species: impl IntoIterator<Item = Species>) -> Self {
        let mut species: Vec<Species> = species.into_iter().collect();
        species.sort_by_key(|species| species.as_str());
        species.dedup();
        Self { species }
    }

    /// The active pet's identity: its one species. Production companions build
    /// their manifest from this — the pet never changes species, so the atlas is
    /// stable for the life of the window save a resize.
    pub fn for_pet(species: Species) -> Self {
        Self::for_species([species])
    }

    /// The full-cast identity covering every species — the strongest atlas, used
    /// by the retained repertoire fixtures.
    pub fn all_species() -> Self {
        Self::for_species(Species::all())
    }

    pub fn species(&self) -> &[Species] {
        &self.species
    }

    /// Stable content-identity bytes for hashing into a resource generation key.
    /// Depends only on the declared content (the sorted species set), never on a
    /// frame's random glyph pick.
    pub fn identity_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for species in &self.species {
            bytes.extend_from_slice(species.as_str().as_bytes());
            bytes.push(0);
        }
        bytes
    }
}

/// Collects the full declared glyph repertoire (sorted, deduplicated) a companion
/// could ever paint for `identity`: the pet body across every stage/mood/state,
/// the room dialect's whole biome/weather/emitter slices, the ambient
/// sky/floor/mote/activity palettes, every earnable prop and tank-life sprite,
/// the performance cues, the chest bubble, and the HUD charset.
///
/// This is declared content, not a frame's pixels — it enumerates the static
/// content inventories directly rather than sampling the random per-minute scene.
/// The retained manifest adds the effect/chrome glyphs (the replacement glyph,
/// bubble emoji, and a composed-mark representative). The `#[cfg(test)]`
/// `repertoire_covers_a_live_scene_plan` guard proves it is a superset of what a
/// real scene plan actually paints.
pub fn collect_companion_glyph_repertoire(
    identity: &CompanionContentIdentity,
) -> Vec<RepertoireGlyph> {
    let mut chars: BTreeSet<char> = BTreeSet::new();

    // Species-independent inventories.
    chars.extend(crate::tui::component::tank_life::declared_tank_life_glyphs());
    chars.extend(crate::tui::panels::pet::declared_performance_cue_glyphs());
    chars.extend(crate::tui::panels::pet::declared_chest_bubble_glyphs());
    chars.extend(crate::tui::component::habitat_props::declared_prop_glyphs(
        identity.species(),
    ));
    chars.extend(
        crate::round::hud::COMPANION_HUD_GLYPH_REPERTOIRE
            .iter()
            .copied(),
    );

    // Per-species inventories: pet body, room dialect, ambient palettes.
    for &species in identity.species() {
        chars.extend(crate::pet::render::declared_pet_glyphs(species));
        chars.extend(crate::tui::room::declared_room_glyphs(
            RoomSpeciesDialect::for_species(species),
        ));
        chars.extend(crate::tui::panels::pet::declared_ambient_glyphs(species));
    }

    // Provision every glyph in BOTH regular and bold weights. Many content roles
    // render bold — the eye role, tank-life foreground, and potentially others —
    // and the render path looks up `(glyph, cell.bold)`, so a glyph missing in the
    // weight a cell happens to use is an atlas miss and a fallback. Mis-tracking a
    // source's weight is exactly that failure, so both-weights-for-all is the
    // robust preflight. Weight is a fixed property of the repertoire, so this does
    // not change the generation key across the per-minute reshuffle.
    let mut glyphs: Vec<RepertoireGlyph> = Vec::with_capacity(chars.len() * 2);
    for ch in chars {
        let sequence = ch.to_string();
        glyphs.push(RepertoireGlyph { sequence: sequence.clone(), bold: false });
        glyphs.push(RepertoireGlyph { sequence, bold: true });
    }
    glyphs.sort();
    glyphs.dedup();
    glyphs
}

/// The glyphs one rendered companion frame actually paints: every `LocalCell`
/// glyph (pet body, room, props, tank life, cues — whole authored scalar
/// sequences with their weight) plus each validated, packed HUD glyph.
/// Sorted and deduplicated. This is a single frame's *painted* set — a subset of
/// the declared repertoire the atlas is preflighted from.
#[allow(dead_code)] // Retained repertoire fixtures use this until HUD GPU prep lands.
pub(crate) fn frame_glyph_sequences(
    plan: &SmoothCompanionScenePlan,
    hud: &crate::round::hud::PackedCompanionHudGlyphs,
) -> Vec<RepertoireGlyph> {
    let mut set: BTreeSet<RepertoireGlyph> = BTreeSet::new();
    for layer in &plan.layers {
        for item in &layer.items {
            if let SmoothLayerItem::LocalCell(cell) = item {
                if let Some(glyph) = cell.glyph.as_ref() {
                    set.insert(RepertoireGlyph { sequence: glyph.clone(), bold: cell.bold });
                }
            }
        }
    }
    for packed in hud.occupied_glyphs() {
        set.insert(RepertoireGlyph {
            sequence: packed.glyph.to_string(),
            bold: false,
        });
    }
    set.into_iter().collect()
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
