use crate::presentation::smooth::{
    CompanionChromeReservation, CompanionViewport, SmoothBounds, SmoothCompanionLayer,
    SmoothDepthPlane, SmoothLayerItem, SmoothLayerMotionBinding, SmoothPoint,
};

const FAR_MULTIPLIER: f32 = 0.01;
const MID_MULTIPLIER: f32 = 0.02;
const BEHIND_MULTIPLIER: f32 = 0.03;
const FOREGROUND_MULTIPLIER: f32 = 0.045;
const VERTICAL_AXIS_SCALE: f32 = 0.75;
const MAX_PARALLAX_X: f32 = 0.5;
const MAX_PARALLAX_Y: f32 = 0.25;
const SAFETY_SCALES: [f32; 5] = [1.0, 0.75, 0.5, 0.25, 0.0];
const OVERLAP_EPSILON: f32 = 0.000_01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallaxResolveError {
    NonFiniteGeometry,
    InvalidLifecycleScale,
    UnsupportedObjectGeometry,
}

pub const fn parallax_lifecycle_scale(asleep: bool, calm: bool) -> f32 {
    if asleep {
        0.25
    } else if calm {
        0.5
    } else {
        1.0
    }
}

fn plane_multiplier(plane: SmoothDepthPlane) -> f32 {
    match plane {
        SmoothDepthPlane::Far => FAR_MULTIPLIER,
        SmoothDepthPlane::Mid => MID_MULTIPLIER,
        SmoothDepthPlane::Behind => BEHIND_MULTIPLIER,
        SmoothDepthPlane::Foreground => FOREGROUND_MULTIPLIER,
    }
}

fn raw_plane_delta(
    focus: SmoothPoint,
    lifecycle_scale: f32,
    plane: SmoothDepthPlane,
) -> Result<SmoothPoint, ParallaxResolveError> {
    if !point_is_finite(focus) {
        return Err(ParallaxResolveError::NonFiniteGeometry);
    }
    validate_lifecycle_scale(lifecycle_scale)?;

    let multiplier = plane_multiplier(plane);
    let delta = SmoothPoint {
        x: (focus.x * multiplier * lifecycle_scale).clamp(-MAX_PARALLAX_X, MAX_PARALLAX_X),
        y: (focus.y * multiplier * VERTICAL_AXIS_SCALE * lifecycle_scale)
            .clamp(-MAX_PARALLAX_Y, MAX_PARALLAX_Y),
    };
    if !point_is_finite(delta) {
        return Err(ParallaxResolveError::NonFiniteGeometry);
    }

    Ok(delta)
}

pub fn resolve_layer_parallax(
    focus: SmoothPoint,
    lifecycle_scale: f32,
    layer: &SmoothCompanionLayer,
    viewport: CompanionViewport,
    chrome: &CompanionChromeReservation,
) -> Result<SmoothPoint, ParallaxResolveError> {
    validate_resolver_geometry(focus, lifecycle_scale, layer, viewport, chrome)?;

    match layer.motion_binding {
        SmoothLayerMotionBinding::Fixed | SmoothLayerMotionBinding::PetAttached => {
            Ok(SmoothPoint::default())
        }
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Far) => {
            raw_plane_delta(focus, lifecycle_scale, SmoothDepthPlane::Far)
        }
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Mid) => {
            raw_plane_delta(focus, lifecycle_scale, SmoothDepthPlane::Mid)
        }
        SmoothLayerMotionBinding::Parallax(plane) => {
            resolve_object_parallax(focus, lifecycle_scale, plane, layer, chrome)
        }
    }
}

fn resolve_object_parallax(
    focus: SmoothPoint,
    lifecycle_scale: f32,
    plane: SmoothDepthPlane,
    layer: &SmoothCompanionLayer,
    chrome: &CompanionChromeReservation,
) -> Result<SmoothPoint, ParallaxResolveError> {
    if layer.transform.scale != (SmoothPoint { x: 1.0, y: 1.0 })
        || layer.transform.rotation_degrees != 0.0
    {
        return Err(ParallaxResolveError::UnsupportedObjectGeometry);
    }
    for item in &layer.items {
        match item {
            SmoothLayerItem::LocalCell(_) => {}
            SmoothLayerItem::Shape(_) | SmoothLayerItem::Raster(_) => {
                return Err(ParallaxResolveError::UnsupportedObjectGeometry);
            }
        }
    }

    let raw_delta = raw_plane_delta(focus, lifecycle_scale, plane)?;
    for scale in SAFETY_SCALES {
        let candidate_delta = SmoothPoint {
            x: raw_delta.x * scale,
            y: raw_delta.y * scale,
        };
        if object_delta_is_safe(layer, candidate_delta, chrome)? {
            return Ok(candidate_delta);
        }
    }

    Ok(SmoothPoint::default())
}

fn object_delta_is_safe(
    layer: &SmoothCompanionLayer,
    delta: SmoothPoint,
    chrome: &CompanionChromeReservation,
) -> Result<bool, ParallaxResolveError> {
    for item in &layer.items {
        match item {
            SmoothLayerItem::LocalCell(cell) => {
                let before =
                    translated_cell_bounds(layer, cell.row, cell.col, SmoothPoint::default());
                let after = translated_cell_bounds(layer, cell.row, cell.col, delta);
                if !bounds_are_finite(before) || !bounds_are_finite(after) {
                    return Err(ParallaxResolveError::NonFiniteGeometry);
                }

                for reservation in chrome.hud_bounds.iter().chain(&chrome.gauge_bounds) {
                    if !overlap_is_safe(before, after, *reservation) {
                        return Ok(false);
                    }
                }
            }
            SmoothLayerItem::Shape(_) | SmoothLayerItem::Raster(_) => {
                return Err(ParallaxResolveError::UnsupportedObjectGeometry);
            }
        }
    }

    Ok(true)
}

fn translated_cell_bounds(
    layer: &SmoothCompanionLayer,
    row: u16,
    col: u16,
    delta: SmoothPoint,
) -> SmoothBounds {
    let min = SmoothPoint {
        x: layer.anchor.x + layer.transform.translation.x + f32::from(col) + delta.x,
        y: layer.anchor.y + layer.transform.translation.y + f32::from(row) + delta.y,
    };
    SmoothBounds {
        min,
        max: SmoothPoint { x: min.x + 1.0, y: min.y + 1.0 },
    }
}

fn intersection_area(left: SmoothBounds, right: SmoothBounds) -> f32 {
    let width = (left.max.x.min(right.max.x) - left.min.x.max(right.min.x)).max(0.0);
    let height = (left.max.y.min(right.max.y) - left.min.y.max(right.min.y)).max(0.0);
    width * height
}

fn overlap_is_safe(before: SmoothBounds, after: SmoothBounds, reservation: SmoothBounds) -> bool {
    let baseline = intersection_area(before, reservation);
    let candidate = intersection_area(after, reservation);
    if baseline <= OVERLAP_EPSILON {
        candidate <= OVERLAP_EPSILON
    } else {
        candidate <= baseline + OVERLAP_EPSILON
    }
}

fn validate_resolver_geometry(
    focus: SmoothPoint,
    lifecycle_scale: f32,
    layer: &SmoothCompanionLayer,
    viewport: CompanionViewport,
    chrome: &CompanionChromeReservation,
) -> Result<(), ParallaxResolveError> {
    if viewport.grid_cols == 0
        || viewport.grid_rows == 0
        || !point_is_finite(focus)
        || !point_is_finite(layer.anchor)
        || !point_is_finite(layer.transform.translation)
        || !point_is_finite(layer.transform.scale)
        || !layer.transform.rotation_degrees.is_finite()
        || chrome
            .hud_bounds
            .iter()
            .chain(&chrome.gauge_bounds)
            .any(|bounds| !bounds_are_finite(*bounds))
    {
        return Err(ParallaxResolveError::NonFiniteGeometry);
    }

    validate_lifecycle_scale(lifecycle_scale)
}

fn validate_lifecycle_scale(lifecycle_scale: f32) -> Result<(), ParallaxResolveError> {
    if !lifecycle_scale.is_finite() {
        return Err(ParallaxResolveError::NonFiniteGeometry);
    }
    if lifecycle_scale != 1.0 && lifecycle_scale != 0.5 && lifecycle_scale != 0.25 {
        return Err(ParallaxResolveError::InvalidLifecycleScale);
    }

    Ok(())
}

fn point_is_finite(point: SmoothPoint) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn bounds_are_finite(bounds: SmoothBounds) -> bool {
    point_is_finite(bounds.min) && point_is_finite(bounds.max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::smooth::{
        SmoothBlendMode, SmoothClip, SmoothCompanionPrivacyClaims, SmoothLayerId, SmoothLayerItem,
        SmoothLayerRole, SmoothLocalCell, SmoothRasterRef, SmoothShapeRef, SmoothTransform,
    };

    fn bounds(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> SmoothBounds {
        SmoothBounds {
            min: SmoothPoint { x: min_x, y: min_y },
            max: SmoothPoint { x: max_x, y: max_y },
        }
    }

    fn local_cell_layer(
        motion_binding: SmoothLayerMotionBinding,
        global_bounds: SmoothBounds,
        cells: &[(u16, u16)],
    ) -> SmoothCompanionLayer {
        let anchor = global_bounds.min;
        SmoothCompanionLayer {
            id: SmoothLayerId("parallax-test-layer".to_string()),
            role: SmoothLayerRole::PropsBehind,
            motion_binding,
            z: 0,
            local_bounds: SmoothBounds {
                min: SmoothPoint::default(),
                max: SmoothPoint {
                    x: global_bounds.max.x - global_bounds.min.x,
                    y: global_bounds.max.y - global_bounds.min.y,
                },
            },
            anchor,
            transform_origin: SmoothPoint::default(),
            transform: SmoothTransform {
                translation: SmoothPoint::default(),
                scale: SmoothPoint { x: 1.0, y: 1.0 },
                rotation_degrees: 0.0,
            },
            parallax_translation: SmoothPoint::default(),
            opacity: 1.0,
            clip: SmoothClip::None,
            blend: SmoothBlendMode::Normal,
            items: cells
                .iter()
                .map(|&(row, col)| {
                    SmoothLayerItem::LocalCell(SmoothLocalCell {
                        row,
                        col,
                        glyph: Some("x".to_string()),
                        fg: None,
                        bg: None,
                        bold: false,
                    })
                })
                .collect(),
            privacy: SmoothCompanionPrivacyClaims::external_companion(),
        }
    }

    #[test]
    fn lifecycle_precedence_is_asleep_then_calm_then_normal() {
        assert_eq!(parallax_lifecycle_scale(false, false), 1.0);
        assert_eq!(parallax_lifecycle_scale(false, true), 0.5);
        assert_eq!(parallax_lifecycle_scale(true, false), 0.25);
        assert_eq!(parallax_lifecycle_scale(true, true), 0.25);
    }

    #[test]
    fn resolver_rejects_lifecycle_scales_outside_domain_without_reversing_direction() {
        let viewport = CompanionViewport { grid_cols: 20, grid_rows: 10 };
        let chrome = CompanionChromeReservation::default();
        for binding in [
            SmoothLayerMotionBinding::Fixed,
            SmoothLayerMotionBinding::PetAttached,
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Far),
        ] {
            let layer = local_cell_layer(binding, bounds(0.0, 0.0, 1.0, 1.0), &[(0, 0)]);
            for invalid_scale in [-1.0, 0.0, 0.75, 2.0] {
                assert_eq!(
                    resolve_layer_parallax(
                        SmoothPoint { x: 4.0, y: 3.0 },
                        invalid_scale,
                        &layer,
                        viewport,
                        &chrome,
                    ),
                    Err(ParallaxResolveError::InvalidLifecycleScale)
                );
            }
        }

        let moving = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Far),
            bounds(0.0, 0.0, 1.0, 1.0),
            &[(0, 0)],
        );
        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint { x: 4.0, y: 3.0 },
                f32::NAN,
                &moving,
                viewport,
                &chrome,
            ),
            Err(ParallaxResolveError::NonFiniteGeometry)
        );

        for (asleep, calm) in [(false, false), (false, true), (true, false), (true, true)] {
            let scale = parallax_lifecycle_scale(asleep, calm);
            let delta = resolve_layer_parallax(
                SmoothPoint { x: 4.0, y: 3.0 },
                scale,
                &moving,
                viewport,
                &chrome,
            )
            .unwrap();
            assert_eq!(
                delta,
                raw_plane_delta(SmoothPoint { x: 4.0, y: 3.0 }, scale, SmoothDepthPlane::Far)
                    .unwrap()
            );
            assert!(delta.x > 0.0);
            assert!(delta.y > 0.0);
        }
    }

    #[test]
    fn raw_plane_magnitudes_are_ordered_and_directionally_symmetric() {
        let positive = SmoothPoint { x: 4.0, y: 3.0 };
        let negative = SmoothPoint { x: -4.0, y: -3.0 };
        let far = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Far).unwrap();
        let mid = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Mid).unwrap();
        let behind = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Behind).unwrap();
        let foreground = raw_plane_delta(positive, 1.0, SmoothDepthPlane::Foreground).unwrap();
        let negative_foreground =
            raw_plane_delta(negative, 1.0, SmoothDepthPlane::Foreground).unwrap();

        assert!(far.x.abs() < mid.x.abs());
        assert!(mid.x.abs() < behind.x.abs());
        assert!(behind.x.abs() < foreground.x.abs());
        assert!(far.y.abs() < mid.y.abs());
        assert!(mid.y.abs() < behind.y.abs());
        assert!(behind.y.abs() < foreground.y.abs());
        assert_eq!(negative_foreground.x, -foreground.x);
        assert_eq!(negative_foreground.y, -foreground.y);
    }

    #[test]
    fn raw_plane_delta_caps_axes_independently() {
        let delta = raw_plane_delta(
            SmoothPoint { x: 10_000.0, y: -10_000.0 },
            1.0,
            SmoothDepthPlane::Foreground,
        )
        .unwrap();

        assert_eq!(delta.x, MAX_PARALLAX_X);
        assert_eq!(delta.y, -MAX_PARALLAX_Y);
    }

    #[test]
    fn zero_focus_and_lifecycle_attenuation_are_exact() {
        for plane in [
            SmoothDepthPlane::Far,
            SmoothDepthPlane::Mid,
            SmoothDepthPlane::Behind,
            SmoothDepthPlane::Foreground,
        ] {
            assert_eq!(
                raw_plane_delta(SmoothPoint::default(), 1.0, plane).unwrap(),
                SmoothPoint::default()
            );
        }

        let focus = SmoothPoint { x: 4.0, y: -3.0 };
        let normal = raw_plane_delta(focus, 1.0, SmoothDepthPlane::Mid).unwrap();
        let calm = raw_plane_delta(focus, 0.5, SmoothDepthPlane::Mid).unwrap();
        let asleep = raw_plane_delta(focus, 0.25, SmoothDepthPlane::Mid).unwrap();
        assert_eq!(calm.x, normal.x * 0.5);
        assert_eq!(calm.y, normal.y * 0.5);
        assert_eq!(asleep.x, normal.x * 0.25);
        assert_eq!(asleep.y, normal.y * 0.25);
    }

    #[test]
    fn sparse_object_cells_ignore_aggregate_bounds_false_positives() {
        let layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind),
            SmoothBounds {
                min: SmoothPoint { x: 0.0, y: 0.0 },
                max: SmoothPoint { x: 12.0, y: 1.0 },
            },
            &[(0, 0), (0, 11)],
        );
        let chrome = CompanionChromeReservation {
            hud_bounds: vec![bounds(5.0, 0.0, 7.0, 1.0)],
            gauge_bounds: Vec::new(),
        };

        let delta = resolve_layer_parallax(
            SmoothPoint { x: 4.0, y: 0.0 },
            1.0,
            &layer,
            CompanionViewport { grid_cols: 20, grid_rows: 10 },
            &chrome,
        )
        .unwrap();

        assert!(
            delta.x > 0.0,
            "empty aggregate-bounds space must not suppress motion"
        );
    }

    #[test]
    fn object_safety_prevents_new_overlap_and_does_not_worsen_existing_overlap() {
        let clear_layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground),
            bounds(0.0, 0.0, 1.0, 1.0),
            &[(0, 0)],
        );
        let chrome = CompanionChromeReservation {
            hud_bounds: vec![bounds(1.01, 0.0, 2.0, 1.0)],
            gauge_bounds: Vec::new(),
        };
        let clear_delta = resolve_layer_parallax(
            SmoothPoint { x: 20.0, y: 0.0 },
            1.0,
            &clear_layer,
            CompanionViewport { grid_cols: 20, grid_rows: 10 },
            &chrome,
        )
        .unwrap();
        assert_eq!(clear_delta, SmoothPoint::default());

        let overlapping_layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground),
            bounds(0.5, 0.0, 1.5, 1.0),
            &[(0, 0)],
        );
        let existing_overlap_delta = resolve_layer_parallax(
            SmoothPoint { x: 20.0, y: 0.0 },
            1.0,
            &overlapping_layer,
            CompanionViewport { grid_cols: 20, grid_rows: 10 },
            &chrome,
        )
        .unwrap();
        assert_eq!(existing_overlap_delta, SmoothPoint::default());
    }

    #[test]
    fn object_planes_reject_shape_and_raster_without_occupied_bounds() {
        for item in [
            SmoothLayerItem::Shape(SmoothShapeRef { name: "shape".to_string() }),
            SmoothLayerItem::Raster(SmoothRasterRef { name: "raster".to_string() }),
        ] {
            let mut layer = local_cell_layer(
                SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind),
                bounds(0.0, 0.0, 1.0, 1.0),
                &[],
            );
            layer.items.push(item);
            assert_eq!(
                resolve_layer_parallax(
                    SmoothPoint { x: 1.0, y: 1.0 },
                    1.0,
                    &layer,
                    CompanionViewport { grid_cols: 20, grid_rows: 10 },
                    &CompanionChromeReservation::default(),
                ),
                Err(ParallaxResolveError::UnsupportedObjectGeometry)
            );
        }
    }

    #[test]
    fn fixed_and_pet_attached_bindings_are_zero_and_non_finite_focus_is_rejected() {
        let viewport = CompanionViewport { grid_cols: 20, grid_rows: 10 };
        let chrome = CompanionChromeReservation::default();
        for binding in [
            SmoothLayerMotionBinding::Fixed,
            SmoothLayerMotionBinding::PetAttached,
        ] {
            let layer = local_cell_layer(binding, bounds(0.0, 0.0, 1.0, 1.0), &[(0, 0)]);
            assert_eq!(
                resolve_layer_parallax(
                    SmoothPoint { x: 10.0, y: -10.0 },
                    1.0,
                    &layer,
                    viewport,
                    &chrome,
                )
                .unwrap(),
                SmoothPoint::default()
            );
        }

        let moving = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Far),
            bounds(0.0, 0.0, 1.0, 1.0),
            &[(0, 0)],
        );
        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint { x: f32::NAN, y: 0.0 },
                1.0,
                &moving,
                viewport,
                &chrome,
            ),
            Err(ParallaxResolveError::NonFiniteGeometry)
        );
    }

    #[test]
    fn object_planes_reject_non_identity_scale_and_rotation() {
        let viewport = CompanionViewport { grid_cols: 20, grid_rows: 10 };
        for mutate in [
            |layer: &mut SmoothCompanionLayer| layer.transform.scale.x = 0.5,
            |layer: &mut SmoothCompanionLayer| layer.transform.rotation_degrees = 1.0,
        ] {
            let mut layer = local_cell_layer(
                SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind),
                bounds(0.0, 0.0, 1.0, 1.0),
                &[(0, 0)],
            );
            mutate(&mut layer);
            assert_eq!(
                resolve_layer_parallax(
                    SmoothPoint { x: 1.0, y: 1.0 },
                    1.0,
                    &layer,
                    viewport,
                    &CompanionChromeReservation::default(),
                ),
                Err(ParallaxResolveError::UnsupportedObjectGeometry)
            );
        }
    }

    #[test]
    fn object_safety_uses_first_safe_discrete_scale() {
        let layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground),
            bounds(0.0, 0.0, 1.0, 1.0),
            &[(0, 0)],
        );
        let chrome = CompanionChromeReservation {
            hud_bounds: vec![bounds(1.4, 0.0, 2.0, 1.0)],
            gauge_bounds: Vec::new(),
        };

        let delta = resolve_layer_parallax(
            SmoothPoint { x: 20.0, y: 0.0 },
            1.0,
            &layer,
            CompanionViewport { grid_cols: 20, grid_rows: 10 },
            &chrome,
        )
        .unwrap();

        assert_eq!(delta, SmoothPoint { x: 0.375, y: 0.0 });
    }

    #[test]
    fn far_and_mid_bypass_chrome_collision_attenuation() {
        let chrome = CompanionChromeReservation {
            hud_bounds: vec![bounds(0.0, 0.0, 2.0, 2.0)],
            gauge_bounds: Vec::new(),
        };
        for plane in [SmoothDepthPlane::Far, SmoothDepthPlane::Mid] {
            let layer = local_cell_layer(
                SmoothLayerMotionBinding::Parallax(plane),
                bounds(0.0, 0.0, 1.0, 1.0),
                &[(0, 0)],
            );
            assert_eq!(
                resolve_layer_parallax(
                    SmoothPoint { x: 4.0, y: 3.0 },
                    1.0,
                    &layer,
                    CompanionViewport { grid_cols: 20, grid_rows: 10 },
                    &chrome,
                )
                .unwrap(),
                raw_plane_delta(SmoothPoint { x: 4.0, y: 3.0 }, 1.0, plane).unwrap()
            );
        }
    }

    #[test]
    fn invalid_viewport_and_non_finite_geometry_are_rejected() {
        let viewport = CompanionViewport { grid_cols: 20, grid_rows: 10 };
        let layer = local_cell_layer(
            SmoothLayerMotionBinding::Fixed,
            bounds(0.0, 0.0, 1.0, 1.0),
            &[(0, 0)],
        );
        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint::default(),
                1.0,
                &layer,
                CompanionViewport { grid_cols: 0, grid_rows: 10 },
                &CompanionChromeReservation::default(),
            ),
            Err(ParallaxResolveError::NonFiniteGeometry)
        );

        let mut non_finite_layer = layer.clone();
        non_finite_layer.anchor.x = f32::INFINITY;
        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint::default(),
                1.0,
                &non_finite_layer,
                viewport,
                &CompanionChromeReservation::default(),
            ),
            Err(ParallaxResolveError::NonFiniteGeometry)
        );

        let chrome = CompanionChromeReservation {
            hud_bounds: vec![bounds(0.0, 0.0, f32::NAN, 1.0)],
            gauge_bounds: Vec::new(),
        };
        assert_eq!(
            resolve_layer_parallax(SmoothPoint::default(), 1.0, &layer, viewport, &chrome,),
            Err(ParallaxResolveError::NonFiniteGeometry)
        );

        let mut overflowing_layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind),
            bounds(f32::MAX, 0.0, f32::MAX, 1.0),
            &[(0, 0)],
        );
        overflowing_layer.transform.translation.x = f32::MAX;
        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint::default(),
                1.0,
                &overflowing_layer,
                viewport,
                &CompanionChromeReservation::default(),
            ),
            Err(ParallaxResolveError::NonFiniteGeometry)
        );
    }

    #[test]
    fn unused_aggregate_origin_and_prior_parallax_do_not_affect_resolution() {
        let mut layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind),
            bounds(0.0, 0.0, 1.0, 1.0),
            &[(0, 0)],
        );
        layer.local_bounds.max.x = f32::NAN;
        layer.transform_origin.y = f32::INFINITY;
        layer.parallax_translation.x = f32::NAN;

        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint { x: 4.0, y: 0.0 },
                1.0,
                &layer,
                CompanionViewport { grid_cols: 20, grid_rows: 10 },
                &CompanionChromeReservation::default(),
            )
            .unwrap(),
            SmoothPoint { x: 0.12, y: 0.0 }
        );
    }

    #[test]
    fn object_safety_allows_motion_that_reduces_existing_overlap() {
        let layer = local_cell_layer(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground),
            bounds(0.5, 0.0, 1.5, 1.0),
            &[(0, 0)],
        );
        let chrome = CompanionChromeReservation {
            hud_bounds: vec![bounds(1.0, 0.0, 2.0, 1.0)],
            gauge_bounds: Vec::new(),
        };

        assert_eq!(
            resolve_layer_parallax(
                SmoothPoint { x: -20.0, y: 0.0 },
                1.0,
                &layer,
                CompanionViewport { grid_cols: 20, grid_rows: 10 },
                &chrome,
            )
            .unwrap(),
            SmoothPoint { x: -0.5, y: 0.0 }
        );
    }
}
