use crate::presentation::smooth::{SmoothBounds, SmoothPoint};
use crate::round::depth::SmoothDepthSample;
use crate::round::motion::{
    MotionPoint, RoundCompanionMotionProjection, RoundCompanionMotionViewport,
};

pub const ROUND_REAR_CENTER_Y_FRACTION: f32 = 0.27;
pub const ROUND_NEUTRAL_CENTER_Y_FRACTION: f32 = 0.50;
pub const ROUND_FRONT_CENTER_Y_FRACTION: f32 = 0.73;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundDepthPlacement {
    pub anchor_top_left_cells: MotionPoint,
    pub anchor_top_left_points: [f32; 2],
    pub final_center_cells: MotionPoint,
    pub max_scale_bounds_cells: SmoothBounds,
    pub tapered_local_y_cells: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundDepthPlacementError {
    EmptyViewport,
    NonFiniteInput,
    PetDoesNotFit,
    InvalidOutput,
}

impl std::fmt::Display for RoundDepthPlacementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyViewport => formatter.write_str("round depth placement viewport is empty"),
            Self::NonFiniteInput => {
                formatter.write_str("round depth placement input is non-finite")
            }
            Self::PetDoesNotFit => {
                formatter.write_str("maximum-scale pet does not fit in aperture")
            }
            Self::InvalidOutput => formatter.write_str("round depth placement output is invalid"),
        }
    }
}

impl std::error::Error for RoundDepthPlacementError {}

pub fn resolve_round_depth_placement(
    motion: RoundCompanionMotionProjection,
    depth: SmoothDepthSample,
    viewport: RoundCompanionMotionViewport,
) -> Result<RoundDepthPlacement, RoundDepthPlacementError> {
    resolve_round_depth_placement_impl(motion, depth, viewport)
}

fn resolve_round_depth_placement_impl(
    motion: RoundCompanionMotionProjection,
    depth: SmoothDepthSample,
    viewport: RoundCompanionMotionViewport,
) -> Result<RoundDepthPlacement, RoundDepthPlacementError> {
    if viewport.grid_columns == 0 || viewport.grid_rows == 0 {
        return Err(RoundDepthPlacementError::EmptyViewport);
    }
    let scalar_inputs = [
        viewport.width_points,
        viewport.height_points,
        viewport.clearance.near_scale,
        viewport.clearance.perspective_y_max,
        motion.motion_top_left_cells.x,
        motion.motion_top_left_cells.y,
        motion.motion_origin_top_left_cells.x,
        motion.motion_origin_top_left_cells.y,
        motion.motion_top_left_points[0],
        motion.motion_top_left_points[1],
        motion.planar_offset_cells.x,
        motion.planar_offset_cells.y,
        motion.normalized_depth,
        motion.bob_offset_y_cells,
        depth.raw_z,
        depth.effective_z,
        depth.scale,
        depth.perspective_y,
        depth.atmosphere,
    ];
    if scalar_inputs.into_iter().any(|value| !value.is_finite())
        || viewport.width_points <= 0.0
        || viewport.height_points <= 0.0
        || !(-1.0..=1.0).contains(&depth.effective_z)
    {
        return Err(RoundDepthPlacementError::NonFiniteInput);
    }
    crate::round::depth::validate_smooth_depth_sample(depth)
        .map_err(|_| RoundDepthPlacementError::InvalidOutput)?;

    let (aperture_center, aperture_radii) =
        round_aperture_geometry(viewport).ok_or(RoundDepthPlacementError::NonFiniteInput)?;
    let half_ink = SmoothPoint {
        x: crate::pet::render::ART_WIDTH as f32 / 2.0 * crate::round::depth::SMOOTH_PET_NEAR_SCALE,
        y: crate::pet::render::ART_HEIGHT as f32 / 2.0 * crate::round::depth::SMOOTH_PET_NEAR_SCALE,
    };
    if half_ink.x >= aperture_radii.x || half_ink.y >= aperture_radii.y {
        return Err(RoundDepthPlacementError::PetDoesNotFit);
    }

    let taper = 1.0 - depth.effective_z.abs();
    let tapered_local_y_cells = motion.planar_offset_cells.y * taper;
    let target_fraction = if depth.effective_z >= 0.0 {
        ROUND_NEUTRAL_CENTER_Y_FRACTION
            + depth.effective_z * (ROUND_FRONT_CENTER_Y_FRACTION - ROUND_NEUTRAL_CENTER_Y_FRACTION)
    } else {
        ROUND_NEUTRAL_CENTER_Y_FRACTION
            + depth.effective_z * (ROUND_NEUTRAL_CENTER_Y_FRACTION - ROUND_REAR_CENTER_Y_FRACTION)
    };
    let requested_base_y = f32::from(viewport.grid_rows) * target_fraction + tapered_local_y_cells;
    // Both renderers add bob and perspective after applying their prepared anchor.
    // Resolve safety against that final rendered center, then remove both offsets
    // from the anchor below so the render-time additions land back on this center.
    let requested_rendered_y = requested_base_y + motion.bob_offset_y_cells;

    let centered_x_ratio = half_ink.x / aperture_radii.x;
    let max_center_dy = aperture_radii.y * (1.0 - centered_x_ratio.powi(2)).sqrt() - half_ink.y;
    if !max_center_dy.is_finite() || max_center_dy < 0.0 {
        return Err(RoundDepthPlacementError::PetDoesNotFit);
    }
    let center_y = requested_rendered_y.clamp(
        aperture_center.y - max_center_dy,
        aperture_center.y + max_center_dy,
    );

    let vertical_corner_ratio =
        ((center_y - aperture_center.y).abs() + half_ink.y) / aperture_radii.y;
    let max_center_dx = aperture_radii.x
        * (1.0 - vertical_corner_ratio.clamp(0.0, 1.0).powi(2)).sqrt()
        - half_ink.x;
    if !max_center_dx.is_finite() || max_center_dx < 0.0 {
        return Err(RoundDepthPlacementError::PetDoesNotFit);
    }
    let requested_x = motion.motion_top_left_cells.x + crate::pet::render::FRAME_WIDTH as f32 / 2.0;
    let center_x = requested_x.clamp(
        aperture_center.x - max_center_dx,
        aperture_center.x + max_center_dx,
    );
    let final_center_cells = MotionPoint { x: center_x, y: center_y };
    let max_scale_bounds_cells = SmoothBounds {
        min: SmoothPoint {
            x: center_x - half_ink.x,
            y: center_y - half_ink.y,
        },
        max: SmoothPoint {
            x: center_x + half_ink.x,
            y: center_y + half_ink.y,
        },
    };
    let anchor_top_left_cells = MotionPoint {
        x: center_x - crate::pet::render::FRAME_WIDTH as f32 / 2.0,
        y: center_y
            - crate::pet::render::FRAME_HEIGHT as f32 / 2.0
            - depth.perspective_y
            - motion.bob_offset_y_cells,
    };
    let anchor_top_left_points = [
        anchor_top_left_cells.x * viewport.width_points / f32::from(viewport.grid_columns),
        anchor_top_left_cells.y * viewport.height_points / f32::from(viewport.grid_rows),
    ];
    let placement = RoundDepthPlacement {
        anchor_top_left_cells,
        anchor_top_left_points,
        final_center_cells,
        max_scale_bounds_cells,
        tapered_local_y_cells,
    };
    if !bounds_inside_round_aperture(max_scale_bounds_cells, viewport)
        || [
            placement.anchor_top_left_cells.x,
            placement.anchor_top_left_cells.y,
            placement.anchor_top_left_points[0],
            placement.anchor_top_left_points[1],
            placement.final_center_cells.x,
            placement.final_center_cells.y,
            placement.tapered_local_y_cells,
        ]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(RoundDepthPlacementError::InvalidOutput);
    }
    Ok(placement)
}

pub(crate) fn bounds_inside_round_aperture(
    bounds: SmoothBounds,
    viewport: RoundCompanionMotionViewport,
) -> bool {
    let Some((center, radii)) = round_aperture_geometry(viewport) else {
        return false;
    };
    if [
        bounds.min.x,
        bounds.min.y,
        bounds.max.x,
        bounds.max.y,
        radii.x,
        radii.y,
    ]
    .into_iter()
    .any(|value| !value.is_finite())
        || radii.x <= 0.0
        || radii.y <= 0.0
        || bounds.min.x > bounds.max.x
        || bounds.min.y > bounds.max.y
    {
        return false;
    }
    [
        bounds.min,
        SmoothPoint { x: bounds.max.x, y: bounds.min.y },
        SmoothPoint { x: bounds.min.x, y: bounds.max.y },
        bounds.max,
    ]
    .into_iter()
    .all(|corner| {
        let x = (corner.x - center.x) / radii.x;
        let y = (corner.y - center.y) / radii.y;
        x * x + y * y <= 1.0 + 1.0e-4
    })
}

fn round_aperture_geometry(
    viewport: RoundCompanionMotionViewport,
) -> Option<(SmoothPoint, SmoothPoint)> {
    if viewport.grid_columns == 0
        || viewport.grid_rows == 0
        || !viewport.width_points.is_finite()
        || !viewport.height_points.is_finite()
        || viewport.width_points <= 0.0
        || viewport.height_points <= 0.0
    {
        return None;
    }
    let cell_extent = SmoothPoint {
        x: viewport.width_points / f32::from(viewport.grid_columns),
        y: viewport.height_points / f32::from(viewport.grid_rows),
    };
    let radius_points = viewport.width_points.min(viewport.height_points) / 2.0;
    let center = SmoothPoint {
        x: f32::from(viewport.grid_columns) / 2.0,
        y: f32::from(viewport.grid_rows) / 2.0,
    };
    let radii = SmoothPoint {
        x: radius_points / cell_extent.x,
        y: radius_points / cell_extent.y,
    };
    if [cell_extent.x, cell_extent.y, radii.x, radii.y]
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
    {
        return None;
    }
    Some((center, radii))
}

#[doc(hidden)]
pub fn bounds_inside_round_aperture_for_test(
    bounds: SmoothBounds,
    viewport: RoundCompanionMotionViewport,
) -> bool {
    bounds_inside_round_aperture(bounds, viewport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::depth::resolve_smooth_depth;
    use crate::round::motion::{
        project_round_companion_motion_from_offsets, CompanionMotionClearance, CompanionMotionInput,
    };

    fn viewport(bottom_reserved_rows: u16) -> RoundCompanionMotionViewport {
        RoundCompanionMotionViewport {
            grid_columns: 44,
            grid_rows: 18,
            width_points: 440.0,
            height_points: 360.0,
            clearance: CompanionMotionClearance {
                near_scale: crate::round::depth::SMOOTH_PET_NEAR_SCALE,
                perspective_y_max: crate::round::depth::SMOOTH_PERSPECTIVE_Y_MAX,
                bottom_reserved_rows,
            },
        }
    }

    fn motion(planar_x: f32, planar_y: f32, depth: f32) -> RoundCompanionMotionProjection {
        RoundCompanionMotionProjection {
            motion_top_left_cells: MotionPoint { x: 15.5 + planar_x, y: 4.0 + planar_y },
            motion_origin_top_left_cells: MotionPoint { x: 15.5, y: 4.0 },
            motion_top_left_points: [155.0 + planar_x * 10.0, 80.0 + planar_y * 20.0],
            planar_offset_cells: MotionPoint { x: planar_x, y: planar_y },
            classic_top_left_cells: [15, 4],
            normalized_depth: depth,
            facing: 1,
            wander_offset_x: 0,
            breath_offset_y_cells: 0,
            bob_offset_y_cells: 0.0,
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1.0e-4,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_bounds_inside_physical_aperture(
        bounds: SmoothBounds,
        viewport: RoundCompanionMotionViewport,
    ) {
        let center_points = [viewport.width_points / 2.0, viewport.height_points / 2.0];
        let radius_points = viewport.width_points.min(viewport.height_points) / 2.0;
        let cell_extent = [
            viewport.width_points / f32::from(viewport.grid_columns),
            viewport.height_points / f32::from(viewport.grid_rows),
        ];
        for corner in [
            bounds.min,
            SmoothPoint { x: bounds.max.x, y: bounds.min.y },
            SmoothPoint { x: bounds.min.x, y: bounds.max.y },
            bounds.max,
        ] {
            let point = [corner.x * cell_extent[0], corner.y * cell_extent[1]];
            let distance_squared =
                (point[0] - center_points[0]).powi(2) + (point[1] - center_points[1]).powi(2);
            assert!(
                distance_squared <= radius_points.powi(2) + 1.0e-2,
                "corner {corner:?} maps to {point:?} outside the physical aperture"
            );
        }
    }

    #[test]
    fn projection_retains_unclipped_planar_displacement_separately() {
        let projection = project_round_companion_motion_from_offsets(
            CompanionMotionInput {
                identity: 0,
                asleep: false,
                sleep_onset_utc: None,
                wake_from_eval_utc: None,
                woke_at_utc: None,
                current_facing: 1,
                resolved_wander_offset_x: 0,
                resolved_wander_facing: 1,
                breath_offset_y_cells: 0,
            },
            0,
            viewport(5),
            &crate::round::motion::companion_roam_motion(),
            20.0,
            -20.0,
            0.0,
            1,
            0,
        );

        assert_close(projection.planar_offset_cells.x, 294.4);
        assert_close(projection.planar_offset_cells.y, -48.0);
        assert_ne!(
            projection.planar_offset_cells,
            MotionPoint {
                x: projection.motion_top_left_cells.x - projection.motion_origin_top_left_cells.x,
                y: projection.motion_top_left_cells.y - projection.motion_origin_top_left_cells.y,
            }
        );
        assert_eq!(projection.classic_top_left_cells, [31, 0]);
    }

    #[test]
    fn endpoints_map_to_rear_neutral_and_front_and_zero_local_y() {
        for (raw_z, expected_fraction) in [
            (-1.0, ROUND_REAR_CENTER_Y_FRACTION),
            (0.0, ROUND_NEUTRAL_CENTER_Y_FRACTION),
            (1.0, ROUND_FRONT_CENTER_Y_FRACTION),
        ] {
            let depth = resolve_smooth_depth(raw_z, 1.0).unwrap();
            let placement = resolve_round_depth_placement(
                motion(0.0, if raw_z == 0.0 { 0.0 } else { 2.0 }, raw_z),
                depth,
                viewport(5),
            )
            .unwrap();
            assert_close(placement.final_center_cells.y / 18.0, expected_fraction);
            if raw_z.abs() == 1.0 {
                assert_eq!(placement.tapered_local_y_cells, 0.0);
            }
        }
    }

    #[test]
    fn local_y_tapers_continuously_and_hud_reservation_is_not_an_input() {
        for raw_z in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            let depth = resolve_smooth_depth(raw_z, 1.0).unwrap();
            let placement =
                resolve_round_depth_placement(motion(0.0, 2.0, raw_z), depth, viewport(5)).unwrap();
            assert_close(placement.tapered_local_y_cells, 2.0 * (1.0 - raw_z.abs()));
        }

        let depth = resolve_smooth_depth(0.5, 1.0).unwrap();
        let reserved =
            resolve_round_depth_placement(motion(0.0, 2.0, 0.5), depth, viewport(5)).unwrap();
        let open =
            resolve_round_depth_placement(motion(0.0, 2.0, 0.5), depth, viewport(0)).unwrap();
        assert_close(reserved.tapered_local_y_cells, 1.0);
        assert_eq!(reserved, open);
    }

    #[test]
    fn maximum_scale_corners_stay_inside_the_elliptical_cell_aperture() {
        let neutral_depth = resolve_smooth_depth(0.0, 1.0).unwrap();
        let neutral =
            resolve_round_depth_placement(motion(2.0, 0.0, 0.0), neutral_depth, viewport(5))
                .unwrap();
        assert_close(neutral.final_center_cells.x, 24.0);

        let depth = resolve_smooth_depth(1.0, 1.0).unwrap();
        let placement =
            resolve_round_depth_placement(motion(20.0, 4.0, 1.0), depth, viewport(5)).unwrap();
        assert!(bounds_inside_round_aperture(
            placement.max_scale_bounds_cells,
            viewport(5)
        ));
        assert_close(placement.final_center_cells.y / 18.0, 0.73);
    }

    #[test]
    fn maximum_scale_corners_stay_inside_the_physical_circle_with_nonsquare_cells() {
        let depth = resolve_smooth_depth(1.0, 1.0).unwrap();
        let placement =
            resolve_round_depth_placement(motion(20.0, 0.0, 1.0), depth, viewport(5)).unwrap();

        assert_bounds_inside_physical_aperture(placement.max_scale_bounds_cells, viewport(5));
    }

    #[test]
    fn small_but_valid_apertures_compress_depth_symmetrically() {
        let mut small = viewport(0);
        small.grid_columns = 20;
        small.grid_rows = 12;
        small.width_points = 240.0;
        small.height_points = 240.0;
        let far = resolve_round_depth_placement(
            motion(0.0, 0.0, -1.0),
            resolve_smooth_depth(-1.0, 1.0).unwrap(),
            small,
        )
        .unwrap();
        let near = resolve_round_depth_placement(
            motion(0.0, 0.0, 1.0),
            resolve_smooth_depth(1.0, 1.0).unwrap(),
            small,
        )
        .unwrap();
        assert_close(far.final_center_cells.y + near.final_center_cells.y, 12.0);
        assert!(far.final_center_cells.y > 12.0 * ROUND_REAR_CENTER_Y_FRACTION);
        assert!(near.final_center_cells.y < 12.0 * ROUND_FRONT_CENTER_Y_FRACTION);
    }

    #[test]
    fn perspective_is_preaccounted_in_the_anchor_and_invalid_geometry_fails() {
        let depth = resolve_smooth_depth(1.0, 1.0).unwrap();
        let placement =
            resolve_round_depth_placement(motion(0.0, 0.0, 1.0), depth, viewport(5)).unwrap();
        let rendered_center_y = placement.anchor_top_left_cells.y
            + crate::pet::render::FRAME_HEIGHT as f32 / 2.0
            + depth.perspective_y;
        assert_close(rendered_center_y, placement.final_center_cells.y);

        let mut nonfinite = viewport(5);
        nonfinite.width_points = f32::NAN;
        assert_eq!(
            resolve_round_depth_placement(motion(0.0, 0.0, 0.0), depth, nonfinite),
            Err(RoundDepthPlacementError::NonFiniteInput)
        );
        let mut nonfinite_motion = motion(0.0, 0.0, 0.0);
        nonfinite_motion.planar_offset_cells.y = f32::INFINITY;
        assert_eq!(
            resolve_round_depth_placement(nonfinite_motion, depth, viewport(5)),
            Err(RoundDepthPlacementError::NonFiniteInput)
        );
        let mut empty = viewport(5);
        empty.grid_columns = 0;
        assert_eq!(
            resolve_round_depth_placement(motion(0.0, 0.0, 0.0), depth, empty),
            Err(RoundDepthPlacementError::EmptyViewport)
        );
        let mut too_small = viewport(0);
        too_small.grid_columns = 4;
        too_small.grid_rows = 4;
        assert_eq!(
            resolve_round_depth_placement(motion(0.0, 0.0, 0.0), depth, too_small),
            Err(RoundDepthPlacementError::PetDoesNotFit)
        );
    }

    #[test]
    fn finite_forged_depth_cues_fail_closed() {
        let canonical = resolve_smooth_depth(0.5, 1.0).unwrap();
        for forged in [
            SmoothDepthSample { raw_z: 0.0, ..canonical },
            SmoothDepthSample { scale: 1.0, ..canonical },
            SmoothDepthSample { perspective_y: 0.0, ..canonical },
            SmoothDepthSample { atmosphere: 0.99, ..canonical },
        ] {
            assert_eq!(
                resolve_round_depth_placement(motion(0.0, 0.0, 0.5), forged, viewport(5)),
                Err(RoundDepthPlacementError::InvalidOutput)
            );
        }
    }

    #[test]
    fn nonzero_bob_is_preaccounted_in_anchor_and_physical_aperture_bounds() {
        let depth = resolve_smooth_depth(1.0, 1.0).unwrap();
        let mut bobbing_motion = motion(0.0, 0.0, 1.0);
        bobbing_motion.bob_offset_y_cells = 0.33;
        let placement = resolve_round_depth_placement(bobbing_motion, depth, viewport(5)).unwrap();

        let rendered_center_y = placement.anchor_top_left_cells.y
            + crate::pet::render::FRAME_HEIGHT as f32 / 2.0
            + depth.perspective_y
            + bobbing_motion.bob_offset_y_cells;
        assert_close(rendered_center_y, placement.final_center_cells.y);
        assert_close(
            (placement.max_scale_bounds_cells.min.y + placement.max_scale_bounds_cells.max.y) / 2.0,
            rendered_center_y,
        );
        assert_bounds_inside_physical_aperture(placement.max_scale_bounds_cells, viewport(5));
    }
}
