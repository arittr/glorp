use glorp::game::habitat::HabitatPropKind;
use glorp::presentation::smooth::{
    transformed_smooth_bounds, validate_smooth_layer, CompanionViewport, SmoothBlendMode,
    SmoothBounds, SmoothClip, SmoothCompanionLayer, SmoothCompanionPrivacyClaims, SmoothDepthPlane,
    SmoothGeometryError, SmoothLayerId, SmoothLayerItem, SmoothLayerMotionBinding, SmoothLayerRole,
    SmoothPoint, SmoothRgba8, SmoothShape, SmoothShapeGeometry, SmoothTransform,
};
use glorp::round::depth::{
    depth_lifecycle_scale, resolve_smooth_depth, SmoothDepthError, SMOOTH_PERSPECTIVE_Y_MAX,
    SMOOTH_PET_FAR_SCALE, SMOOTH_PET_NEAR_SCALE,
};
use glorp::round::scene::CompanionMotion;
use glorp::round::tank_bed::smooth_tank_bed_geometry;
use glorp::storage::state::{HabitatPropId, HabitatPropSource};
use glorp::tui::room::{RoomBiome, RoomBiomeTag};
use glorp::tui::view_model::{EarnedHabitatPropView, SourceStatus, WatchViewModel};
use time::macros::datetime;

const GRID_COLS: u16 = 44;
const GRID_ROWS: u16 = 18;
const NOW: time::OffsetDateTime = datetime!(2026-06-13 18:00 UTC);
const PET_W: u16 = 13;
const PET_H: u16 = 10;
/// The creature art inside the 13x10 particle frame: one gutter cell per side.
const PET_INK_W: u16 = 11;
const PET_INK_H: u16 = 8;

fn bounds(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> SmoothBounds {
    SmoothBounds {
        min: SmoothPoint { x: min_x, y: min_y },
        max: SmoothPoint { x: max_x, y: max_y },
    }
}

fn ellipse(bounds: SmoothBounds) -> SmoothShape {
    SmoothShape {
        geometry: SmoothShapeGeometry::Ellipse { bounds },
        color: SmoothRgba8 { r: 90, g: 61, b: 99, a: 128 },
    }
}

fn valid_smooth_layer() -> SmoothCompanionLayer {
    SmoothCompanionLayer {
        id: SmoothLayerId("typed-ellipse".to_string()),
        role: SmoothLayerRole::PetBody,
        motion_binding: SmoothLayerMotionBinding::PetAttached,
        z: 0,
        local_bounds: bounds(0.0, 0.0, 2.0, 2.0),
        anchor: SmoothPoint { x: 10.0, y: 20.0 },
        transform_origin: SmoothPoint { x: 1.0, y: 1.0 },
        transform: SmoothTransform {
            translation: SmoothPoint { x: 0.0, y: 0.0 },
            scale: SmoothPoint { x: 1.0, y: 1.0 },
            rotation_degrees: 0.0,
        },
        parallax_translation: SmoothPoint { x: 0.0, y: 0.0 },
        opacity: 1.0,
        clip: SmoothClip::Rect(bounds(0.0, 0.0, 2.0, 2.0)),
        blend: SmoothBlendMode::Normal,
        items: vec![SmoothLayerItem::Shape(ellipse(bounds(0.0, 0.0, 2.0, 2.0)))],
        privacy: SmoothCompanionPrivacyClaims::external_companion(),
    }
}

fn ellipse_bounds_mut(layer: &mut SmoothCompanionLayer) -> &mut SmoothBounds {
    let SmoothLayerItem::Shape(SmoothShape {
        geometry: SmoothShapeGeometry::Ellipse { bounds },
        ..
    }) = layer
        .items
        .first_mut()
        .expect("fixture includes an ellipse")
    else {
        panic!("fixture must include a typed ellipse");
    };
    bounds
}

#[test]
fn smooth_shape_is_typed_and_serializable() {
    let shape = ellipse(bounds(1.0, 2.0, 4.0, 6.0));

    assert_eq!(
        shape.geometry,
        SmoothShapeGeometry::Ellipse { bounds: bounds(1.0, 2.0, 4.0, 6.0) }
    );

    let json = serde_json::to_value(shape).expect("typed shape should serialize");
    assert_eq!(json["geometry"]["Ellipse"]["bounds"]["min"]["x"], 1.0);
    assert_eq!(json["geometry"]["Ellipse"]["bounds"]["min"]["y"], 2.0);
    assert_eq!(json["geometry"]["Ellipse"]["bounds"]["max"]["x"], 4.0);
    assert_eq!(json["geometry"]["Ellipse"]["bounds"]["max"]["y"], 6.0);
    assert_eq!(json["color"]["r"], 90);
    assert_eq!(json["color"]["g"], 61);
    assert_eq!(json["color"]["b"], 99);
    assert_eq!(json["color"]["a"], 128);
}

#[test]
fn smooth_geometry_rejects_nonfinite_nonpositive_nonuniform_and_rotated_layers() {
    let mut nonfinite_layer_bounds = valid_smooth_layer();
    nonfinite_layer_bounds.local_bounds.min.x = f32::NAN;

    let mut inverted_layer_bounds = valid_smooth_layer();
    inverted_layer_bounds.local_bounds.max.x = -1.0;

    let mut nonfinite_anchor = valid_smooth_layer();
    nonfinite_anchor.anchor.y = f32::INFINITY;

    let mut nonfinite_transform_origin = valid_smooth_layer();
    nonfinite_transform_origin.transform_origin.x = f32::NEG_INFINITY;

    let mut nonfinite_translation = valid_smooth_layer();
    nonfinite_translation.transform.translation.y = f32::NAN;

    let mut nonfinite_opacity = valid_smooth_layer();
    nonfinite_opacity.opacity = f32::NAN;

    let mut opacity_out_of_range = valid_smooth_layer();
    opacity_out_of_range.opacity = 1.01;

    let mut nonfinite_scale = valid_smooth_layer();
    nonfinite_scale.transform.scale.x = f32::INFINITY;

    let mut nonpositive_scale = valid_smooth_layer();
    nonpositive_scale.transform.scale.y = 0.0;

    let mut nonuniform_scale = valid_smooth_layer();
    nonuniform_scale.transform.scale.y = 1.01;

    let mut rotated = valid_smooth_layer();
    rotated.transform.rotation_degrees = 0.1;

    let mut nonfinite_shape_bounds = valid_smooth_layer();
    ellipse_bounds_mut(&mut nonfinite_shape_bounds).max.y = f32::NAN;

    let mut inverted_shape_bounds = valid_smooth_layer();
    ellipse_bounds_mut(&mut inverted_shape_bounds).min.y = 3.0;

    let mut nonfinite_clip_bounds = valid_smooth_layer();
    nonfinite_clip_bounds.clip = SmoothClip::Rect(bounds(0.0, 0.0, f32::INFINITY, 2.0));

    let mut inverted_clip_bounds = valid_smooth_layer();
    inverted_clip_bounds.clip = SmoothClip::Rect(bounds(1.0, 0.0, 0.0, 2.0));

    let cases = [
        (
            nonfinite_layer_bounds,
            SmoothGeometryError::NonFiniteLayerBounds,
        ),
        (
            inverted_layer_bounds,
            SmoothGeometryError::InvertedLayerBounds,
        ),
        (nonfinite_anchor, SmoothGeometryError::NonFiniteAnchor),
        (
            nonfinite_transform_origin,
            SmoothGeometryError::NonFiniteTransformOrigin,
        ),
        (
            nonfinite_translation,
            SmoothGeometryError::NonFiniteTranslation,
        ),
        (nonfinite_opacity, SmoothGeometryError::NonFiniteOpacity),
        (opacity_out_of_range, SmoothGeometryError::OpacityOutOfRange),
        (nonfinite_scale, SmoothGeometryError::NonFiniteScale),
        (nonpositive_scale, SmoothGeometryError::NonPositiveScale),
        (nonuniform_scale, SmoothGeometryError::NonUniformScale),
        (rotated, SmoothGeometryError::RotationUnsupported),
        (
            nonfinite_shape_bounds,
            SmoothGeometryError::NonFiniteShapeBounds,
        ),
        (
            inverted_shape_bounds,
            SmoothGeometryError::InvertedShapeBounds,
        ),
        (
            nonfinite_clip_bounds,
            SmoothGeometryError::NonFiniteClipBounds,
        ),
        (
            inverted_clip_bounds,
            SmoothGeometryError::InvertedClipBounds,
        ),
    ];

    for (layer, expected) in cases {
        assert_eq!(transformed_smooth_bounds(&layer), Err(expected));
    }
}

#[test]
fn transformed_bounds_scale_around_the_declared_origin() {
    let mut layer = valid_smooth_layer();
    layer.transform.scale = SmoothPoint { x: 1.12, y: 1.12 };

    let transformed = transformed_smooth_bounds(&layer).expect("layer should be valid");
    let width = transformed.max.x - transformed.min.x;
    let height = transformed.max.y - transformed.min.y;
    let center = SmoothPoint {
        x: transformed.min.x + width / 2.0,
        y: transformed.min.y + height / 2.0,
    };

    const TRANSFORM_ASSERT_EPSILON: f32 = f32::EPSILON * 16.0;
    assert!(
        (width - 2.24).abs() <= TRANSFORM_ASSERT_EPSILON,
        "width was {width}"
    );
    assert!(
        (height - 2.24).abs() <= TRANSFORM_ASSERT_EPSILON,
        "height was {height}"
    );
    assert_eq!(center, SmoothPoint { x: 11.0, y: 21.0 });
}

fn parity_fixture() -> WatchViewModel {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.source_health[0].status = SourceStatus::Diagnostic;
    vm.habitat.earned_props.push(EarnedHabitatPropView {
        id: HabitatPropId::new(glorp::game::habitat::TOKEN_TREASURE_CHEST_2M),
        earned_at: time::OffsetDateTime::UNIX_EPOCH,
        kind: HabitatPropKind::Trophy,
        display_priority: 100,
        source: HabitatPropSource::LifetimeTokens { threshold: 2_000_000.0 },
    });
    vm
}

#[test]
fn smooth_depth_resolver_maps_bounds_lifecycle_and_rejects_invalid_inputs() {
    assert_eq!(resolve_smooth_depth(-1.0, 1.0).unwrap().scale, 0.88);
    assert_eq!(resolve_smooth_depth(0.0, 1.0).unwrap().scale, 1.0);
    assert_eq!(resolve_smooth_depth(1.0, 1.0).unwrap().scale, 1.12);
    assert_eq!(depth_lifecycle_scale(false, false), 1.0);
    assert_eq!(depth_lifecycle_scale(false, true), 0.5);
    assert_eq!(depth_lifecycle_scale(true, false), 0.25);
    assert_eq!(depth_lifecycle_scale(true, true), 0.25);

    for raw_z in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            resolve_smooth_depth(raw_z, 1.0),
            Err(SmoothDepthError::NonFiniteRawDepth)
        );
    }
    for lifecycle_scale in [-0.01, 1.01, f32::NAN, f32::INFINITY] {
        assert_eq!(
            resolve_smooth_depth(0.0, lifecycle_scale),
            Err(SmoothDepthError::InvalidLifecycleScale)
        );
    }

    for raw_z in [-f32::MAX, -1.0, 0.0, 1.0, f32::MAX] {
        for lifecycle_scale in [0.0, 0.25, 0.5, 1.0] {
            let sample = resolve_smooth_depth(raw_z, lifecycle_scale).unwrap();
            assert!(sample.effective_z.is_finite());
            assert!((-1.0..=1.0).contains(&sample.effective_z));
            assert!((SMOOTH_PET_FAR_SCALE..=SMOOTH_PET_NEAR_SCALE).contains(&sample.scale));
            assert!(sample.perspective_y.is_finite());
            assert!(sample.perspective_y.abs() <= SMOOTH_PERSPECTIVE_Y_MAX);
            assert!(sample.perspective_y.abs() < 1.0);
        }
    }
}

fn sample_motion_channels(motion: CompanionMotion) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut vm = parity_fixture();
    vm.day_context.asleep = false;
    vm.life_profile.calm_mode = false;
    vm.progress.rate_per_hour = 50_000_000.0;
    let period_ms = motion.drift_period_secs * 1_000;
    let sample_count = period_ms * 2 / 50;
    let mut xs = Vec::with_capacity(sample_count as usize + 1);
    let mut ys = Vec::with_capacity(sample_count as usize + 1);
    let mut zs = Vec::with_capacity(sample_count as usize + 1);

    for step in 0..=sample_count {
        let now = NOW + time::Duration::milliseconds((step * 50) as i64);
        let first =
            glorp::round::scene::companion_pet_placement(&vm, now, GRID_COLS, GRID_ROWS, &motion);
        let second =
            glorp::round::scene::companion_pet_placement(&vm, now, GRID_COLS, GRID_ROWS, &motion);
        assert_eq!(first.raw_depth, second.raw_depth);
        assert!((-1.0..=1.0).contains(&first.raw_depth));
        xs.push(first.fractional_motion_top_left.x - first.fractional_motion_origin_top_left.x);
        ys.push(first.fractional_motion_top_left.y - first.fractional_motion_origin_top_left.y);
        zs.push(first.raw_depth);
    }

    for pair in zs.windows(2) {
        assert!(
            (pair[1] - pair[0]).abs() <= 0.05,
            "adjacent raw-Z jump exceeded the 50 ms continuity bound: {pair:?}"
        );
    }
    (xs, ys, zs)
}

fn normalized_channel(values: &[f32]) -> Vec<f32> {
    let max_abs = values.iter().copied().map(f32::abs).fold(0.0, f32::max);
    values.iter().map(|value| value / max_abs).collect()
}

#[test]
fn smooth_depth_motion_is_deterministic_continuous_bounded_and_separately_salted() {
    for motion in [
        CompanionMotion::default(),
        glorp::round::scene::companion_roam_motion(),
    ] {
        let (xs, ys, zs) = sample_motion_channels(motion);
        let xs = normalized_channel(&xs);
        let ys = normalized_channel(&ys);
        assert!(
            zs.iter().zip(&xs).any(|(z, x)| (z - x).abs() > 0.001),
            "Z must not duplicate the X channel"
        );
        assert!(
            zs.iter().zip(&ys).any(|(z, y)| (z - y).abs() > 0.001),
            "Z must not duplicate the Y channel"
        );
    }
}

fn legacy_wander_offsets(now: time::OffsetDateTime, period_secs: u64) -> (f32, f32) {
    use std::f64::consts::TAU;
    let t = (now.unix_timestamp() as f64 + now.nanosecond() as f64 / 1_000_000_000.0)
        / period_secs.max(1) as f64;
    let fx = 0.72 * (TAU * t).cos() + 0.28 * (TAU * t * 1.93 + 0.6).sin();
    let fy = 0.72 * (TAU * t * 1.21 + 0.3).sin() + 0.28 * (TAU * t * 2.41 + 1.5).cos();
    (fx as f32, fy as f32)
}

fn legacy_classic_rect(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> ratatui::layout::Rect {
    let (fx, fy) = legacy_wander_offsets(now, motion.drift_period_secs);
    let cx = grid_cols / 2;
    let cy = grid_rows / 2;
    let half_w = PET_W / 2;
    let half_h = PET_H / 2;
    let safe_x = cx.saturating_sub(half_w) as f32;
    let safe_y = cy.saturating_sub(half_h) as f32;
    let x_radius = safe_x * motion.drift_x_frac;
    let y_radius = safe_y * motion.drift_y_frac;
    let bias = motion.upward_bias * safe_y;
    let max_x = grid_cols.saturating_sub(PET_W);
    let max_y = grid_rows.saturating_sub(PET_H);
    let classic_x =
        (cx as i32 - half_w as i32 + (fx * x_radius) as i32).clamp(0, max_x as i32) as u16;
    let classic_drift_y = (cy as i32 - half_h as i32 - bias as i32 + (fy * y_radius) as i32)
        .clamp(0, max_y as i32) as u16;
    let classic_y = (classic_drift_y + u16::from(vm.breath_offset_y)).min(max_y);
    ratatui::layout::Rect::new(classic_x, classic_y, PET_W, PET_H)
}

#[test]
fn maximum_scale_smooth_placement_preserves_classic_and_protected_clearance() {
    let mut vm = parity_fixture();
    vm.day_context.asleep = false;
    vm.life_profile.calm_mode = false;
    vm.progress.rate_per_hour = 50_000_000.0;
    vm.breath_offset_y = 1;
    let motion = glorp::round::scene::companion_roam_motion();
    let hud_start = GRID_ROWS
        - glorp::round::scene::round_tank_life_geometry(GRID_COLS, GRID_ROWS).reserved_regions[0]
            .height;
    // The anchor is the particle frame's top-left; the creature ink is concentric
    // inside it, so the ink center sits half a frame from the anchor. Clearance is
    // reserved for the ink at maximum scale, not for the ambient particle gutter.
    let frame_half_w = f32::from(PET_W) / 2.0;
    let frame_half_h = f32::from(PET_H) / 2.0;
    let scaled_ink_half_w = f32::from(PET_INK_W) / 2.0 * SMOOTH_PET_NEAR_SCALE;
    let scaled_ink_half_h = f32::from(PET_INK_H) / 2.0 * SMOOTH_PET_NEAR_SCALE;
    let mut roam_ys = Vec::new();

    for step in 0..=(motion.drift_period_secs * 2 * 20) {
        let now = NOW + time::Duration::milliseconds((step * 50) as i64);
        let placement =
            glorp::round::scene::companion_pet_placement(&vm, now, GRID_COLS, GRID_ROWS, &motion);
        assert_eq!(
            placement.classic_rect,
            legacy_classic_rect(&vm, now, GRID_COLS, GRID_ROWS, &motion),
            "adding Z must not change Classic placement at {now}"
        );

        let center_x = placement.fractional_motion_top_left.x + frame_half_w;
        let center_y = placement.fractional_motion_top_left.y + frame_half_h;
        let min_x = center_x - scaled_ink_half_w;
        let max_x = center_x + scaled_ink_half_w;
        let min_y = center_y - scaled_ink_half_h;
        let max_y = center_y + scaled_ink_half_h;
        assert!(min_x >= 0.0 && max_x <= f32::from(GRID_COLS));
        assert!(
            min_y >= 0.0,
            "maximum-scale pet crossed the aperture top at {now}"
        );
        assert!(
            max_y <= f32::from(hud_start),
            "maximum-scale pet entered the HUD reserve at {now}: max_y={max_y}"
        );
        roam_ys.push(placement.fractional_motion_top_left.y);
    }

    // Clearance alone is satisfiable by pinning the pet against the envelope, which
    // would silently trade the free-swimming composition for safety. Reserving
    // against the maximum scale is only allowed to shrink the roam *slightly*.
    let lowest = roam_ys.iter().copied().fold(f32::MAX, f32::min);
    let highest = roam_ys.iter().copied().fold(f32::MIN, f32::max);
    assert!(
        highest - lowest >= 2.0,
        "max-scale clearance crushed the vertical roam to {:.2} cells of travel",
        highest - lowest
    );
    let pinned = roam_ys
        .iter()
        .filter(|y| (**y - lowest).abs() < 1e-3 || (**y - highest).abs() < 1e-3)
        .count();
    assert!(
        pinned * 2 < roam_ys.len(),
        "pet sat pinned against the roam envelope for {pinned}/{} of the cycle",
        roam_ys.len()
    );
}

#[test]
fn smooth_plan_focus_is_continuous_wander_minus_neutral_origin() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);
    let placement =
        glorp::round::scene::companion_pet_placement(&vm, now, GRID_COLS, GRID_ROWS, &motion);
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 500,
    );

    assert_eq!(
        plan.pet.parallax_focus_offset,
        glorp::presentation::smooth::SmoothPoint {
            x: placement.fractional_motion_top_left.x
                - placement.fractional_motion_origin_top_left.x,
            y: placement.fractional_motion_top_left.y
                - placement.fractional_motion_origin_top_left.y,
        }
    );
}

#[test]
fn smooth_plan_assigns_every_current_role_its_approved_binding() {
    use SmoothDepthPlane::{Behind, Far, Foreground, Mid};
    use SmoothLayerMotionBinding::{Fixed, FloorProjected, Parallax, PetAttached};
    use SmoothLayerRole::*;

    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &glorp::round::scene::companion_roam_motion(),
        0,
    );
    let expected = [
        (DepthRings, Fixed),
        (BiomeWash, Parallax(Far)),
        (RoomGlyphs, Parallax(Far)),
        (TankBed, Fixed),
        (Ambient, Parallax(Mid)),
        (Motes, Parallax(Mid)),
        (ActivityGlyphs, Parallax(Mid)),
        (PropsBehind, Parallax(Behind)),
        (TankLifeBehind, Parallax(Behind)),
        (ChestBubble, Parallax(Behind)),
        (WallShadow, PetAttached),
        (FloorProjection, FloorProjected),
        (PetBody, PetAttached),
        (PerformanceCue, PetAttached),
        (PropsForeground, Parallax(Foreground)),
        (TankLifeForeground, Parallax(Foreground)),
        (StatusHalo, Fixed),
        (TroubleIndicator, Fixed),
        (MoodAura, PetAttached),
        (DimOverlay, Fixed),
    ];

    for (role, binding) in expected {
        let layer = plan.layer_by_role(role).expect("current role should exist");
        assert_eq!(
            layer.motion_binding, binding,
            "unexpected binding for {role:?}"
        );
    }
}

fn tank_bed_biome() -> RoomBiome {
    RoomBiome {
        primary: RoomBiomeTag::Technical,
        secondary: Some(RoomBiomeTag::Celestial),
    }
}

fn ellipse_bounds(shape: &SmoothShape) -> SmoothBounds {
    let SmoothShapeGeometry::Ellipse { bounds } = shape.geometry;
    bounds
}

#[test]
fn tank_bed_geometry_is_curved_deterministic_and_finite() {
    let viewport = CompanionViewport {
        grid_cols: GRID_COLS,
        grid_rows: GRID_ROWS,
    };
    let bed = smooth_tank_bed_geometry(viewport, tank_bed_biome())
        .expect("normal companion viewport should have a tank bed");

    let broad_band_count = bed
        .shapes
        .iter()
        .filter(|shape| {
            let bounds = ellipse_bounds(shape);
            bounds.max.x - bounds.min.x > f32::from(viewport.grid_cols) * 0.5
        })
        .count();
    let fleck_count = bed.shapes.len() - broad_band_count;
    assert!((2..=3).contains(&broad_band_count));
    assert!((8..=14).contains(&fleck_count));
    assert!((bed.horizon_y - f32::from(viewport.grid_rows) * 0.76).abs() < 0.01);
    assert!((bed.near_edge_y - f32::from(viewport.grid_rows)).abs() < 0.01);

    for shape in &bed.shapes {
        let bounds = ellipse_bounds(shape);
        assert!(
            [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
                .into_iter()
                .all(f32::is_finite),
            "tank bed ellipses must stay finite"
        );
        assert!(bounds.max.x > bounds.min.x && bounds.max.y > bounds.min.y);
    }

    assert_eq!(
        smooth_tank_bed_geometry(viewport, tank_bed_biome()),
        Some(bed),
        "bed geometry must depend only on biome and viewport"
    );
}

#[test]
fn tank_bed_geometry_rejects_degenerate_viewports() {
    for viewport in [
        CompanionViewport { grid_cols: 0, grid_rows: GRID_ROWS },
        CompanionViewport { grid_cols: GRID_COLS, grid_rows: 0 },
        CompanionViewport { grid_cols: 1, grid_rows: 1 },
    ] {
        assert_eq!(smooth_tank_bed_geometry(viewport, tank_bed_biome()), None);
    }
}

#[test]
fn tank_bed_layer_is_fixed_clipped_and_independent_of_pet_motion() {
    let vm = parity_fixture();
    let still = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        0,
    );
    let moving = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &glorp::round::scene::companion_roam_motion(),
        500,
    );
    let bed = still
        .layer_by_role(SmoothLayerRole::TankBed)
        .expect("smooth scene should include the tank bed");
    let moving_bed = moving
        .layer_by_role(SmoothLayerRole::TankBed)
        .expect("smooth scene should include the tank bed while moving");

    assert_eq!(SmoothLayerRole::TankBed.as_str(), "tank-bed");
    assert_eq!(bed.motion_binding, SmoothLayerMotionBinding::Fixed);
    assert_eq!(bed.z, 1);
    assert_eq!(bed.items, moving_bed.items);
    assert_eq!(bed.transform.translation, SmoothPoint::default());
    assert_eq!(bed.parallax_translation, SmoothPoint::default());
    assert!(bed
        .items
        .iter()
        .all(|item| matches!(item, SmoothLayerItem::Shape(_))));
    assert!(validate_smooth_layer(bed).is_ok());
    assert_eq!(
        bed.clip,
        SmoothClip::Circle {
            center: SmoothPoint {
                x: f32::from(GRID_COLS) / 2.0,
                y: f32::from(GRID_ROWS) / 2.0,
            },
            radius: f32::from(GRID_COLS.min(GRID_ROWS)) / 2.0,
        }
    );

    let roles: Vec<_> = still.layers.iter().map(|layer| layer.role).collect();
    let room = roles
        .iter()
        .position(|role| *role == SmoothLayerRole::RoomGlyphs)
        .unwrap();
    let tank_bed = roles
        .iter()
        .position(|role| *role == SmoothLayerRole::TankBed)
        .unwrap();
    let ambient = roles
        .iter()
        .position(|role| *role == SmoothLayerRole::Ambient)
        .unwrap();
    let first_prop_or_life = roles
        .iter()
        .position(|role| {
            matches!(
                role,
                SmoothLayerRole::PropsBehind
                    | SmoothLayerRole::TankLifeBehind
                    | SmoothLayerRole::PropsForeground
                    | SmoothLayerRole::TankLifeForeground
            )
        })
        .unwrap();
    assert!(room < tank_bed && tank_bed < ambient && tank_bed < first_prop_or_life);
}

#[test]
fn smooth_plan_lifecycle_scale_uses_asleep_precedence() {
    let motion = glorp::round::scene::companion_roam_motion();
    let mut normal = parity_fixture();
    normal.day_context.asleep = false;
    normal.life_profile.calm_mode = false;
    let mut calm = normal.clone();
    calm.life_profile.calm_mode = true;
    let mut asleep_and_calm = calm.clone();
    asleep_and_calm.day_context.asleep = true;

    let normal_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &normal, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let calm_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &calm, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let asleep_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &asleep_and_calm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &motion,
        0,
    );

    assert_eq!(normal_plan.parallax_lifecycle_scale, 1.0);
    assert_eq!(calm_plan.parallax_lifecycle_scale, 0.5);
    assert_eq!(asleep_plan.parallax_lifecycle_scale, 0.25);
}

#[test]
fn fallible_smooth_round_plan_matches_existing_infallible_plan() {
    let vm = parity_fixture();
    let motion = CompanionMotion::default();

    let infallible = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, NOW, GRID_COLS, GRID_ROWS, &motion, 250,
    );
    let fallible = glorp::round::smooth::try_build_round_smooth_scene_plan(
        &vm, NOW, GRID_COLS, GRID_ROWS, &motion, 250,
    )
    .expect("fixture should include pet body layer");

    assert_eq!(fallible, infallible);
}

fn anchored_bounds(
    anchor: glorp::presentation::smooth::SmoothPoint,
    local_bounds: glorp::presentation::smooth::SmoothBounds,
) -> glorp::presentation::smooth::SmoothBounds {
    glorp::presentation::smooth::SmoothBounds {
        min: glorp::presentation::smooth::SmoothPoint {
            x: anchor.x + local_bounds.min.x,
            y: anchor.y + local_bounds.min.y,
        },
        max: glorp::presentation::smooth::SmoothPoint {
            x: anchor.x + local_bounds.max.x,
            y: anchor.y + local_bounds.max.y,
        },
    }
}

#[test]
fn smooth_round_plan_includes_classic_and_round_only_roles() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        0,
    );
    let roles: Vec<_> = plan.layers.iter().map(|layer| layer.role).collect();

    assert_eq!(
        roles,
        vec![
            SmoothLayerRole::DepthRings,
            SmoothLayerRole::BiomeWash,
            SmoothLayerRole::RoomGlyphs,
            SmoothLayerRole::TankBed,
            SmoothLayerRole::Ambient,
            SmoothLayerRole::Motes,
            SmoothLayerRole::ActivityGlyphs,
            SmoothLayerRole::PropsBehind,
            SmoothLayerRole::TankLifeBehind,
            SmoothLayerRole::ChestBubble,
            SmoothLayerRole::WallShadow,
            SmoothLayerRole::FloorProjection,
            SmoothLayerRole::PetBody,
            SmoothLayerRole::PerformanceCue,
            SmoothLayerRole::PropsForeground,
            SmoothLayerRole::TankLifeForeground,
            SmoothLayerRole::StatusHalo,
            SmoothLayerRole::TroubleIndicator,
            SmoothLayerRole::MoodAura,
            SmoothLayerRole::DimOverlay,
        ]
    );
}

#[test]
fn smooth_round_plan_gives_pet_body_a_fractional_bob_transform() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        250,
    );
    let pet_body = plan
        .layers
        .iter()
        .find(|layer| layer.role == SmoothLayerRole::PetBody)
        .expect("pet body layer should exist");

    assert!(pet_body.transform.translation.y != 0.0);
    assert!(pet_body.transform.translation.y.fract() != 0.0);
}

#[test]
fn smooth_round_plan_keeps_classic_cell_art_in_pet_body() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        250,
    );
    let pet_body = plan
        .layers
        .iter()
        .find(|layer| layer.role == SmoothLayerRole::PetBody)
        .expect("pet body layer should exist");

    assert!(!pet_body.items.is_empty());
    assert!(pet_body
        .items
        .iter()
        .all(|item| matches!(item, SmoothLayerItem::LocalCell(_))));
}

#[test]
fn smooth_round_plan_records_fractional_pet_anchors() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);

    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 250,
    );

    assert_eq!(smooth.pet.bounds.min.x, smooth.pet.classic_snap_anchor.x);
    assert_eq!(smooth.pet.bounds.min.y, smooth.pet.classic_snap_anchor.y);
    assert!(
        (smooth.pet.base_anchor.x - smooth.pet.classic_snap_anchor.x).abs() > f32::EPSILON
            || (smooth.pet.base_anchor.y - smooth.pet.classic_snap_anchor.y).abs() > f32::EPSILON,
        "smooth plan should preserve fractional residual separate from Classic snap"
    );
    assert_ne!(smooth.pet.final_anchor, smooth.pet.base_anchor);
}

#[test]
fn smooth_round_plan_floor_projection_stays_below_props_and_moves_pet_attached_layers() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        250,
    );
    let pet_body = plan.layer_by_role(SmoothLayerRole::PetBody).unwrap();
    let wall_shadow = plan.layer_by_role(SmoothLayerRole::WallShadow).unwrap();
    let floor_projection = plan
        .layer_by_role(SmoothLayerRole::FloorProjection)
        .unwrap();
    let performance_cue = plan.layer_by_role(SmoothLayerRole::PerformanceCue).unwrap();
    let chest_bubble = plan.layer_by_role(SmoothLayerRole::ChestBubble).unwrap();
    let props_behind = plan.layer_by_role(SmoothLayerRole::PropsBehind).unwrap();
    let first_prop_z = plan
        .layers
        .iter()
        .filter(|layer| {
            matches!(
                layer.role,
                SmoothLayerRole::PropsBehind
                    | SmoothLayerRole::PropsForeground
                    | SmoothLayerRole::TankLifeBehind
                    | SmoothLayerRole::TankLifeForeground
            )
        })
        .map(|layer| layer.z)
        .min()
        .unwrap();

    assert!(pet_body.transform.translation.x.abs() > f32::EPSILON);
    assert_eq!(
        wall_shadow.transform.translation.x,
        pet_body.transform.translation.x
    );
    // The projection is a bed-anchored ellipse positioned in viewport coordinates
    // from the pet centre and the depth sample, so it carries no transform of its
    // own and cannot inherit the pet's bob.
    assert_eq!(
        floor_projection.transform.translation,
        SmoothPoint { x: 0.0, y: 0.0 },
        "the bed-anchored projection must not be translated with the pet body"
    );
    assert_eq!(
        floor_projection.transform.scale,
        SmoothPoint { x: 1.0, y: 1.0 }
    );
    assert!(floor_projection.z < first_prop_z);
    assert!(!plan
        .layers
        .iter()
        .any(|layer| layer.role.as_str() == "floor-texture"));
    for prop_role in [
        SmoothLayerRole::PropsBehind,
        SmoothLayerRole::TankLifeBehind,
        SmoothLayerRole::ChestBubble,
        SmoothLayerRole::PropsForeground,
        SmoothLayerRole::TankLifeForeground,
    ] {
        let prop_layer = plan.layer_by_role(prop_role).unwrap();
        assert!(
            floor_projection.z < prop_layer.z,
            "floor projection must stay below {prop_role:?}"
        );
    }
    assert_eq!(
        performance_cue.transform.translation.x,
        pet_body.transform.translation.x
    );
    assert_eq!(
        chest_bubble.motion_binding,
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind)
    );
    assert_eq!(chest_bubble.motion_binding, props_behind.motion_binding);
    assert_eq!(
        chest_bubble.transform.translation,
        chest_bubble.parallax_translation
    );
}

#[test]
fn smooth_plan_composes_nonzero_parallax_without_moving_fixed_or_pet_layers() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        500,
    );

    assert_ne!(
        plan.pet.parallax_focus_offset,
        glorp::presentation::smooth::SmoothPoint::default()
    );
    assert!(plan.layers.iter().any(|layer| {
        matches!(layer.motion_binding, SmoothLayerMotionBinding::Parallax(_))
            && layer.parallax_translation != glorp::presentation::smooth::SmoothPoint::default()
    }));
    for layer in &plan.layers {
        if matches!(
            layer.motion_binding,
            SmoothLayerMotionBinding::Fixed
                | SmoothLayerMotionBinding::PetAttached
                | SmoothLayerMotionBinding::FloorProjected
        ) {
            assert_eq!(
                layer.parallax_translation,
                glorp::presentation::smooth::SmoothPoint::default(),
                "fixed and pet-attached layers must not receive parallax: {:?}",
                layer.role
            );
        }
    }
}

#[test]
fn nonzero_parallax_is_preserved_for_smooth_layers() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);
    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 500,
    );

    assert!(smooth.layers.iter().any(|layer| {
        layer.parallax_translation != glorp::presentation::smooth::SmoothPoint::default()
    }));
}

#[test]
fn classic_breath_does_not_change_parallax_focus() {
    let motion = glorp::round::scene::companion_roam_motion();
    let mut still = parity_fixture();
    let mut breathed = still.clone();
    still.breath_offset_y = 0;
    breathed.breath_offset_y = 1;

    let still_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &still, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let breathed_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &breathed, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );

    assert_eq!(
        still_plan.pet.parallax_focus_offset,
        breathed_plan.pet.parallax_focus_offset
    );
}

#[test]
fn smooth_round_plan_uses_posture_shifted_pet_body_for_metadata_and_aura() {
    let mut vm = parity_fixture();
    vm.day_context.asleep = true;
    let motion = glorp::round::scene::companion_roam_motion();
    let elapsed_ms = 250;
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        elapsed_ms,
    );
    let pet_body = plan.layer_by_role(SmoothLayerRole::PetBody).unwrap();
    let mood_aura = plan.layer_by_role(SmoothLayerRole::MoodAura).unwrap();
    let snapped_bounds = anchored_bounds(pet_body.anchor, pet_body.local_bounds);
    let fractional_anchor = glorp::presentation::smooth::SmoothPoint {
        x: pet_body.anchor.x + pet_body.transform.translation.x,
        y: pet_body.anchor.y + pet_body.transform.translation.y,
    };
    let fractional_bounds = anchored_bounds(fractional_anchor, pet_body.local_bounds);
    let fractional_center = glorp::presentation::smooth::SmoothPoint {
        x: (fractional_bounds.min.x + fractional_bounds.max.x) / 2.0,
        y: (fractional_bounds.min.y + fractional_bounds.max.y) / 2.0,
    };

    assert_eq!(plan.pet.classic_snap_anchor, pet_body.anchor);
    assert_eq!(plan.pet.bounds, snapped_bounds);
    assert_eq!(
        plan.pet.base_anchor.x,
        pet_body.anchor.x + pet_body.transform.translation.x
    );
    assert_eq!(
        plan.pet.base_anchor.y,
        pet_body.anchor.y + pet_body.transform.translation.y - plan.pet.bob_offset.y
    );
    assert_eq!(plan.pet.final_anchor, fractional_anchor);
    assert_eq!(plan.pet.fractional_bounds, fractional_bounds);

    // The aura tracks the composed pet transform. Even an asleep pet's attenuated
    // depth scales the body, so an aura pinned to the unscaled art would drift off
    // the creature as it swims.
    let transformed_bounds = transformed_smooth_bounds(pet_body).unwrap();
    let transformed_center = SmoothPoint {
        x: (transformed_bounds.min.x + transformed_bounds.max.x) / 2.0,
        y: (transformed_bounds.min.y + transformed_bounds.max.y) / 2.0,
    };
    assert_eq!(plan.pet.transformed_bounds, transformed_bounds);
    assert_eq!(mood_aura.transform_origin, transformed_center);
    assert_ne!(
        transformed_center, fractional_center,
        "this fixture must carry a nonneutral depth or the aura contract is untested"
    );
}

#[test]
fn smooth_round_plan_limits_adjacent_paint_frame_anchor_delta() {
    let start = datetime!(2026-07-08 12:00 UTC);
    const COMPANION_GRID_COLS: u16 = 36;
    const COMPANION_GRID_ROWS: u16 = 18;
    let motion = glorp::round::scene::companion_roam_motion();
    let mut vm = parity_fixture();
    vm.pet_render.generated_species = glorp::pet::generation::Species::Glitch;
    vm.pet_render.stage = glorp::game::evolution::Stage::S4;
    vm.life_profile.burst_level = 1.0;
    vm.progress.rate_per_hour = 75_000_000.0;
    glorp::commands::watch::rerender_pet_for_view_model(&mut vm, 0, false, start).unwrap();

    let mut previous: Option<glorp::presentation::smooth::SmoothPoint> = None;
    let mut max_delta = 0.0f32;
    for frame in 0..(22 * 30) {
        let elapsed_ms = frame * 33;
        let now = start + time::Duration::milliseconds(elapsed_ms);
        let plan = glorp::round::smooth::build_round_smooth_scene_plan(
            &vm,
            now,
            COMPANION_GRID_COLS,
            COMPANION_GRID_ROWS,
            &motion,
            elapsed_ms as u64,
        );
        if let Some(last) = previous {
            let dx = (plan.pet.base_anchor.x - last.x).abs();
            let dy = (plan.pet.base_anchor.y - last.y).abs();
            max_delta = max_delta.max(dx).max(dy);
        }
        previous = Some(plan.pet.base_anchor);
    }

    assert!(
        max_delta <= 0.25,
        "adjacent paint frame anchors should stay smooth; max delta was {max_delta:.3}"
    );
}

#[test]
fn smooth_round_plan_does_not_turn_classic_breath_step_into_world_motion() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let motion = glorp::round::scene::companion_roam_motion();
    let mut still = parity_fixture();
    let mut breathed = still.clone();
    still.breath_offset_y = 0;
    breathed.breath_offset_y = 1;

    let still_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&still, now, 36, 18, &motion, 0);
    let breathed_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&breathed, now, 36, 18, &motion, 0);

    assert_ne!(
        still_plan.pet.classic_snap_anchor.y, breathed_plan.pet.classic_snap_anchor.y,
        "Classic snap should still preserve the discrete breath row"
    );
    assert_eq!(still_plan.pet.base_anchor, breathed_plan.pet.base_anchor);
    assert_eq!(still_plan.pet.final_anchor, breathed_plan.pet.final_anchor);
}

#[test]
fn smooth_round_plan_does_not_turn_performance_posture_step_into_world_motion() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let motion = glorp::round::scene::companion_roam_motion();
    let mut perked = parity_fixture();
    perked.life_profile.burst_level = 1.0;
    perked.last_feed_pulse_at = Some(now);
    let mut settled = perked.clone();
    settled.last_feed_pulse_at = None;
    settled.day_context.tiredness = 0.9;

    let perked_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&perked, now, 36, 18, &motion, 0);
    let settled_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&settled, now, 36, 18, &motion, 0);

    assert_ne!(
        perked_plan.pet.classic_snap_anchor.y, settled_plan.pet.classic_snap_anchor.y,
        "Classic snap should still preserve the settled posture row"
    );
    assert_eq!(perked_plan.pet.base_anchor, settled_plan.pet.base_anchor);
    assert_eq!(perked_plan.pet.final_anchor, settled_plan.pet.final_anchor);
}

#[test]
fn smooth_round_plan_keeps_privacy_claims_external_safe() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        250,
    );

    assert_eq!(
        plan.privacy,
        SmoothCompanionPrivacyClaims::external_companion()
    );
    assert!(plan
        .layers
        .iter()
        .all(|layer| layer.privacy == SmoothCompanionPrivacyClaims::external_companion()));
}

// ---------------------------------------------------------------------------
// Task 4: composed pet depth and the bed-anchored floor projection.
// ---------------------------------------------------------------------------

const DEPTH_NOW: time::OffsetDateTime = datetime!(2026-07-08 18:00:00.500 UTC);

/// Depth excursions are attenuated when the pet is calm or asleep, so the
/// far/neutral/near scale contract only holds in a normal lifecycle.
fn normal_lifecycle_fixture() -> WatchViewModel {
    let mut vm = parity_fixture();
    vm.day_context.asleep = false;
    vm.life_profile.calm_mode = false;
    vm
}

fn plan_at_depth(
    vm: &WatchViewModel,
    elapsed_ms: u64,
    depth: f32,
) -> glorp::presentation::smooth::SmoothCompanionScenePlan {
    glorp::round::smooth::try_build_round_smooth_scene_plan_with_options(
        vm,
        DEPTH_NOW,
        GRID_COLS,
        GRID_ROWS,
        &glorp::round::scene::companion_roam_motion(),
        elapsed_ms,
        glorp::round::smooth::SmoothSceneBuildOptions { depth_override: Some(depth) },
    )
    .expect("normal fixture builds a smooth plan")
}

fn only_ellipse(layer: &SmoothCompanionLayer) -> (SmoothBounds, SmoothRgba8) {
    assert_eq!(
        layer.items.len(),
        1,
        "{:?} must carry exactly one typed shape",
        layer.role
    );
    match &layer.items[0] {
        SmoothLayerItem::Shape(SmoothShape {
            geometry: SmoothShapeGeometry::Ellipse { bounds },
            color,
        }) => (*bounds, *color),
        other => panic!("expected a typed ellipse, got {other:?}"),
    }
}

fn center_x(b: SmoothBounds) -> f32 {
    b.min.x + (b.max.x - b.min.x) / 2.0
}

fn center_y(b: SmoothBounds) -> f32 {
    b.min.y + (b.max.y - b.min.y) / 2.0
}

const PET_ATTACHED_ROLES: [SmoothLayerRole; 3] = [
    SmoothLayerRole::PetBody,
    SmoothLayerRole::WallShadow,
    SmoothLayerRole::PerformanceCue,
];

#[test]
fn depth_transform_maps_far_neutral_and_near_onto_scale_and_perspective() {
    let vm = normal_lifecycle_fixture();

    let far = plan_at_depth(&vm, 250, -1.0);
    let neutral = plan_at_depth(&vm, 250, 0.0);
    let near = plan_at_depth(&vm, 250, 1.0);

    assert_eq!(far.pet.scale, SMOOTH_PET_FAR_SCALE);
    assert_eq!(neutral.pet.scale, 1.0);
    assert_eq!(near.pet.scale, SMOOTH_PET_NEAR_SCALE);

    assert_eq!(far.pet.depth, -1.0);
    assert_eq!(neutral.pet.depth, 0.0);
    assert_eq!(near.pet.depth, 1.0);

    // Far is up and small; near is down and large.
    assert!(far.pet.perspective_offset.y < 0.0);
    assert_eq!(neutral.pet.perspective_offset.y, 0.0);
    assert!(near.pet.perspective_offset.y > 0.0);
    assert_eq!(near.pet.perspective_offset.y, SMOOTH_PERSPECTIVE_Y_MAX);
    assert_eq!(far.pet.perspective_offset.x, 0.0);

    // One depth sample drives every pet-attached layer, uniformly.
    for plan in [&far, &neutral, &near] {
        for role in PET_ATTACHED_ROLES {
            let layer = plan.layer_by_role(role).unwrap();
            assert_eq!(
                layer.transform.scale.x, plan.pet.scale,
                "{role:?} must carry the composed pet scale"
            );
            assert_eq!(
                layer.transform.scale.y, layer.transform.scale.x,
                "{role:?} depth scale must stay uniform"
            );
        }
    }

    // The perspective translation moves all three by exactly the same amount.
    for role in PET_ATTACHED_ROLES {
        let step = near.layer_by_role(role).unwrap().transform.translation.y
            - neutral.layer_by_role(role).unwrap().transform.translation.y;
        assert!(
            (step - SMOOTH_PERSPECTIVE_Y_MAX).abs() < 1e-5,
            "{role:?} moved {step} for a near depth step, expected {SMOOTH_PERSPECTIVE_Y_MAX}"
        );
    }
}

#[test]
fn depth_transform_keeps_idle_bob_on_the_pet_body_alone() {
    let vm = normal_lifecycle_fixture();
    let early = plan_at_depth(&vm, 250, 0.0);
    let late = plan_at_depth(&vm, 1250, 0.0);

    assert_ne!(
        early.pet.bob_offset.y, late.pet.bob_offset.y,
        "fixture must straddle a bob phase or this test proves nothing"
    );

    let body_step = late
        .layer_by_role(SmoothLayerRole::PetBody)
        .unwrap()
        .transform
        .translation
        .y
        - early
            .layer_by_role(SmoothLayerRole::PetBody)
            .unwrap()
            .transform
            .translation
            .y;
    assert!(
        (body_step - (late.pet.bob_offset.y - early.pet.bob_offset.y)).abs() < 1e-5,
        "the pet body must carry the idle bob"
    );

    for role in [
        SmoothLayerRole::WallShadow,
        SmoothLayerRole::PerformanceCue,
        SmoothLayerRole::FloorProjection,
    ] {
        assert_eq!(
            early.layer_by_role(role).unwrap().transform.translation.y,
            late.layer_by_role(role).unwrap().transform.translation.y,
            "{role:?} must not inherit the pet's idle bob"
        );
    }
}

#[test]
fn depth_transform_publishes_transformed_bounds_and_drives_the_mood_aura() {
    let vm = normal_lifecycle_fixture();

    for depth in [-1.0, 0.0, 1.0] {
        let plan = plan_at_depth(&vm, 250, depth);
        let pet_body = plan.layer_by_role(SmoothLayerRole::PetBody).unwrap();
        assert_eq!(
            plan.pet.transformed_bounds,
            transformed_smooth_bounds(pet_body).unwrap(),
            "published pet bounds must equal the pet body's transformed bounds"
        );

        // The aura is prepared from the transformed bounds, not the unscaled art.
        let aura = plan.layer_by_role(SmoothLayerRole::MoodAura).unwrap();
        let aura_center = match aura.clip {
            SmoothClip::Circle { center, .. } => center,
            other => panic!("mood aura must keep a circular clip, got {other:?}"),
        };
        assert!((aura_center.x - center_x(plan.pet.transformed_bounds)).abs() < 1e-4);
        assert!((aura_center.y - center_y(plan.pet.transformed_bounds)).abs() < 1e-4);
    }

    let far = plan_at_depth(&vm, 250, -1.0);
    let near = plan_at_depth(&vm, 250, 1.0);
    let width = |b: SmoothBounds| b.max.x - b.min.x;
    assert!(
        width(near.pet.transformed_bounds) > width(far.pet.transformed_bounds),
        "the near pet must render wider than the far pet"
    );
}

#[test]
fn floor_projection_is_one_bed_anchored_ellipse_that_tracks_depth() {
    let vm = normal_lifecycle_fixture();
    let far = plan_at_depth(&vm, 250, -1.0);
    let near = plan_at_depth(&vm, 250, 1.0);

    let bed = smooth_tank_bed_geometry(
        CompanionViewport {
            grid_cols: GRID_COLS,
            grid_rows: GRID_ROWS,
        },
        glorp::presentation::PetSceneModel::build(
            &vm,
            DEPTH_NOW,
            glorp::tui::style::ColorCapability::Truecolor,
        )
        .room
        .biome,
    )
    .expect("normal viewport has a tank bed");

    let (far_bounds, far_color) =
        only_ellipse(far.layer_by_role(SmoothLayerRole::FloorProjection).unwrap());
    let (near_bounds, near_color) = only_ellipse(
        near.layer_by_role(SmoothLayerRole::FloorProjection)
            .unwrap(),
    );

    // No leftover Classic background cells survive in the Smooth projection.
    for plan in [&far, &near] {
        let projection = plan
            .layer_by_role(SmoothLayerRole::FloorProjection)
            .unwrap();
        assert!(
            !projection
                .items
                .iter()
                .any(|item| matches!(item, SmoothLayerItem::LocalCell(_))),
            "the smooth floor projection must not keep Classic cells"
        );
    }

    // Near reads bigger, stronger, and further down the bed than far.
    assert!(near_bounds.max.x - near_bounds.min.x > far_bounds.max.x - far_bounds.min.x);
    assert!(near_bounds.max.y - near_bounds.min.y > far_bounds.max.y - far_bounds.min.y);
    assert!(near_color.a > far_color.a);
    assert!(
        center_y(far_bounds) < center_y(near_bounds),
        "the far projection must sit closer to the bed horizon"
    );
    assert!(center_y(far_bounds) >= bed.horizon_y);
    assert!(center_y(near_bounds) <= bed.near_edge_y);

    // It tracks the pet across the tank. The creature's centre is the centre of
    // its particle frame, which `max_scale_clearance` is built around; the
    // transformed cell bounding box is not centred on the creature.
    for plan in [&far, &near] {
        let (projection_bounds, _) = only_ellipse(
            plan.layer_by_role(SmoothLayerRole::FloorProjection)
                .unwrap(),
        );
        assert!(
            (center_x(projection_bounds) - center_x(plan.pet.max_scale_clearance)).abs() < 1e-3,
            "the projection must sit under the creature"
        );
    }

    // And it stays beneath every prop and tank inhabitant.
    let projection = near
        .layer_by_role(SmoothLayerRole::FloorProjection)
        .unwrap();
    for prop_role in [
        SmoothLayerRole::PropsBehind,
        SmoothLayerRole::TankLifeBehind,
        SmoothLayerRole::ChestBubble,
        SmoothLayerRole::PropsForeground,
        SmoothLayerRole::TankLifeForeground,
    ] {
        assert!(
            projection.z < near.layer_by_role(prop_role).unwrap().z,
            "floor projection must stay below {prop_role:?}"
        );
    }
}

#[test]
fn composed_plan_publishes_max_scale_clearance_inside_the_protected_regions() {
    let vm = normal_lifecycle_fixture();
    let hud_start = GRID_ROWS
        - glorp::round::scene::round_tank_life_geometry(GRID_COLS, GRID_ROWS).reserved_regions[0]
            .height;

    for step in 0..240 {
        let now = DEPTH_NOW + time::Duration::milliseconds((step * 250) as i64);
        let plan = glorp::round::smooth::try_build_round_smooth_scene_plan(
            &vm,
            now,
            GRID_COLS,
            GRID_ROWS,
            &glorp::round::scene::companion_roam_motion(),
            (step * 250) as u64,
        )
        .expect("plan builds across the roam cycle");

        let clearance = plan.pet.max_scale_clearance;
        assert!(
            clearance.min.x >= -1e-4 && clearance.max.x <= f32::from(GRID_COLS) + 1e-4,
            "max-scale clearance left the aperture at {now}: {clearance:?}"
        );
        assert!(
            clearance.min.y >= -1e-4,
            "max-scale clearance rose above the aperture at {now}: {clearance:?}"
        );
        assert!(
            clearance.max.y <= f32::from(hud_start) + 1e-4,
            "max-scale clearance entered the HUD reserve at {now}: {clearance:?}"
        );

        // The clearance is the promise the roam envelope makes: the creature ink at
        // maximum scale, plus the full perspective excursion in both directions.
        let expected_w = f32::from(PET_INK_W) * SMOOTH_PET_NEAR_SCALE;
        let expected_h =
            f32::from(PET_INK_H) * SMOOTH_PET_NEAR_SCALE + 2.0 * SMOOTH_PERSPECTIVE_Y_MAX;
        assert!((clearance.max.x - clearance.min.x - expected_w).abs() < 1e-3);
        assert!((clearance.max.y - clearance.min.y - expected_h).abs() < 1e-3);
    }
}

#[test]
fn composed_plan_rejects_a_nonfinite_depth_override_without_blaming_parallax() {
    let vm = normal_lifecycle_fixture();
    let err = glorp::round::smooth::try_build_round_smooth_scene_plan_with_options(
        &vm,
        DEPTH_NOW,
        GRID_COLS,
        GRID_ROWS,
        &glorp::round::scene::companion_roam_motion(),
        250,
        glorp::round::smooth::SmoothSceneBuildOptions { depth_override: Some(f32::NAN) },
    )
    .expect_err("a nonfinite depth override must not reach the renderer");

    assert_eq!(
        err,
        glorp::round::smooth::SmoothScenePlanError::InvalidDepth(
            SmoothDepthError::NonFiniteRawDepth
        )
    );
}

/// The bed is a receding substrate seen through tank water. Stacked opaque bands
/// turn it into a footer bowl at companion size, which is what the first native
/// capture showed.
#[test]
fn tank_bed_bands_stay_translucent_enough_to_read_as_depth() {
    const MAX_BED_ALPHA: u8 = 80;

    for biome in [
        RoomBiome {
            primary: RoomBiomeTag::Starter,
            secondary: None,
        },
        RoomBiome {
            primary: RoomBiomeTag::Celestial,
            secondary: Some(RoomBiomeTag::Botanical),
        },
    ] {
        let bed = smooth_tank_bed_geometry(
            CompanionViewport {
                grid_cols: GRID_COLS,
                grid_rows: GRID_ROWS,
            },
            biome,
        )
        .expect("normal viewport has a tank bed");

        for shape in &bed.shapes {
            assert!(
                shape.color.a <= MAX_BED_ALPHA,
                "bed shape alpha {} reads as a solid footer, not a substrate",
                shape.color.a
            );
        }
    }
}
