use super::{AuthoredDepthSnapshot, PropTopologySnapshot, PropZoneSnapshot};
use crate::presentation::tank_life::TankRouteRect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CompanionCompositionInput<'a> {
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) width_points: f32,
    pub(crate) height_points: f32,
    pub(crate) bottom_reserved_rows: u16,
    pub(crate) props: &'a [PropTopologySnapshot],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompanionPropPlacement {
    pub(crate) slot: u8,
    pub(crate) visible: bool,
    pub(crate) anchor_cell: [i16; 2],
    pub(crate) bounds_cells: [i16; 4],
    pub(crate) footprint_cells: [u16; 2],
    pub(crate) grounded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompanionComposition {
    pub(crate) prop_placements: Vec<CompanionPropPlacement>,
    pub(crate) hud_reserve_cells: [i16; 4],
    pub(crate) gauge_inner_radius_cells: [f32; 2],
    pub(crate) tank_reserved_regions: Vec<TankRouteRect>,
    pub(crate) tank_foreground_reserved_regions: Vec<TankRouteRect>,
}

#[derive(Debug, Clone, Copy)]
enum CandidateAxis {
    Start(i16),
    Center { extent: i16, offset: i16 },
    End { extent: i16, offset: i16 },
}

impl CandidateAxis {
    fn resolve(self, footprint: i16) -> i16 {
        match self {
            Self::Start(offset) => offset,
            Self::Center { extent, offset } => (extent - footprint) / 2 + offset,
            Self::End { extent, offset } => extent - footprint + offset,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CandidateAnchor {
    x: CandidateAxis,
    y: CandidateAxis,
}

impl CandidateAnchor {
    fn resolve(self, footprint: [i16; 2]) -> [i16; 2] {
        [self.x.resolve(footprint[0]), self.y.resolve(footprint[1])]
    }
}

pub(crate) fn resolve_companion_composition(
    input: CompanionCompositionInput<'_>,
) -> CompanionComposition {
    let columns = i16::try_from(input.columns).unwrap_or(i16::MAX);
    let rows = i16::try_from(input.rows).unwrap_or(i16::MAX);
    let bottom_reserved_rows = input.bottom_reserved_rows.min(input.rows);
    let available_rows = input.rows.saturating_sub(bottom_reserved_rows);
    let hud_reserve_cells = hud_reserve(input.columns, input.rows);
    let bottom_reserve_cells = [
        0,
        i16::try_from(available_rows).unwrap_or(i16::MAX),
        columns,
        rows,
    ];
    let gauge_inner_radius_cells = gauge_inner_radii(input);
    let mut accepted_bounds = Vec::<[i16; 4]>::new();
    let mut prop_placements = Vec::with_capacity(input.props.len());
    let mut tank_foreground_reserved_regions = Vec::new();

    for prop in input.props {
        let Some(footprint) =
            crate::presentation::props::presentation_prop_max_footprint(prop.catalog_id)
        else {
            prop_placements.push(hidden_placement(prop, [0, 0]));
            continue;
        };
        let footprint_cells = [
            u16::try_from(i16::from(footprint.max_dx) - i16::from(footprint.min_dx) + 1)
                .unwrap_or(0),
            u16::try_from(i16::from(footprint.max_dy) - i16::from(footprint.min_dy) + 1)
                .unwrap_or(0),
        ];
        let footprint_i16 = [
            i16::try_from(footprint_cells[0]).unwrap_or(i16::MAX),
            i16::try_from(footprint_cells[1]).unwrap_or(i16::MAX),
        ];
        let accepted = candidate_anchors(prop.zone, input.columns, available_rows)
            .into_iter()
            .map(|candidate| candidate.resolve(footprint_i16))
            .find_map(|top_left| {
                let anchor_cell = [
                    top_left[0] - i16::from(footprint.min_dx),
                    top_left[1] - i16::from(footprint.min_dy),
                ];
                let bounds = [
                    top_left[0],
                    top_left[1],
                    top_left[0] + footprint_i16[0],
                    top_left[1] + footprint_i16[1],
                ];
                candidate_is_safe(
                    bounds,
                    columns,
                    rows,
                    gauge_inner_radius_cells,
                    hud_reserve_cells,
                    bottom_reserve_cells,
                    &accepted_bounds,
                )
                .then_some((anchor_cell, bounds))
            });
        let grounded = is_floor_zone(prop.zone);
        if let Some((anchor_cell, bounds_cells)) = accepted {
            accepted_bounds.push(bounds_cells);
            if prop.authored_depth == AuthoredDepthSnapshot::Foreground {
                let expanded = expand_and_clip(bounds_cells, columns, rows);
                if let Some(route_rect) = tank_rect(expanded) {
                    tank_foreground_reserved_regions.push(route_rect);
                }
            }
            prop_placements.push(CompanionPropPlacement {
                slot: prop.stable_order,
                visible: true,
                anchor_cell,
                bounds_cells,
                footprint_cells,
                grounded,
            });
        } else {
            prop_placements.push(hidden_placement(prop, footprint_cells));
        }
    }

    let mut tank_reserved_regions = Vec::with_capacity(2);
    if let Some(rect) = tank_rect(hud_reserve_cells) {
        tank_reserved_regions.push(rect);
    }
    if let Some(rect) = tank_rect(bottom_reserve_cells) {
        tank_reserved_regions.push(rect);
    }

    CompanionComposition {
        prop_placements,
        hud_reserve_cells,
        gauge_inner_radius_cells,
        tank_reserved_regions,
        tank_foreground_reserved_regions,
    }
}

fn hidden_placement(
    prop: &PropTopologySnapshot,
    footprint_cells: [u16; 2],
) -> CompanionPropPlacement {
    CompanionPropPlacement {
        slot: prop.stable_order,
        visible: false,
        anchor_cell: [0; 2],
        bounds_cells: [0; 4],
        footprint_cells,
        grounded: is_floor_zone(prop.zone),
    }
}

fn hud_reserve(columns: u16, rows: u16) -> [i16; 4] {
    let width = f32::from(columns) * 0.58;
    let center = f32::from(columns) / 2.0;
    [
        (center - width / 2.0).floor() as i16,
        (f32::from(rows) * 0.58).floor() as i16,
        (center + width / 2.0).ceil() as i16,
        (f32::from(rows) * 0.90).ceil() as i16,
    ]
}

fn gauge_inner_radii(input: CompanionCompositionInput<'_>) -> [f32; 2] {
    if input.columns == 0
        || input.rows == 0
        || !input.width_points.is_finite()
        || !input.height_points.is_finite()
        || input.width_points <= 0.0
        || input.height_points <= 0.0
    {
        return [0.0; 2];
    }
    let aperture_radius = f64::from(input.width_points.min(input.height_points)) / 2.0;
    let gauge = crate::presentation::companion_effects::perimeter_gauge_layout(
        aperture_radius,
        crate::presentation::companion_effects::COMPANION_GAUGE_GAP_DEGREES,
    );
    let inner_radius_points = gauge.pace.radius - gauge.pace.stroke_width / 2.0;
    let cell_extent = [
        f64::from(input.width_points) / f64::from(input.columns),
        f64::from(input.height_points) / f64::from(input.rows),
    ];
    [
        (inner_radius_points / cell_extent[0] - 0.5).max(0.0) as f32,
        (inner_radius_points / cell_extent[1] - 0.5).max(0.0) as f32,
    ]
}

fn candidate_anchors(zone: PropZoneSnapshot, columns: u16, rows: u16) -> Vec<CandidateAnchor> {
    let columns = i16::try_from(columns).unwrap_or(i16::MAX);
    let rows = i16::try_from(rows).unwrap_or(i16::MAX);
    let start_x = |offset| CandidateAxis::Start(offset);
    let center_x = |offset| CandidateAxis::Center { extent: columns, offset };
    let end_x = |offset| CandidateAxis::End { extent: columns, offset };
    let start_y = |offset| CandidateAxis::Start(offset);
    let center_y = |offset| CandidateAxis::Center { extent: rows, offset };
    let end_y = |offset| CandidateAxis::End { extent: rows, offset };
    let anchor = |x, y| CandidateAnchor { x, y };

    match zone {
        PropZoneSnapshot::FloorLeft => vec![
            anchor(start_x(2), end_y(-1)),
            anchor(start_x(4), end_y(-1)),
            anchor(start_x(2), end_y(-2)),
        ],
        PropZoneSnapshot::FloorMid => vec![
            anchor(center_x(0), end_y(-1)),
            anchor(center_x(-13), end_y(-1)),
            anchor(center_x(13), end_y(-1)),
            anchor(center_x(-7), end_y(-1)),
            anchor(center_x(7), end_y(-1)),
            anchor(center_x(0), end_y(-2)),
        ],
        PropZoneSnapshot::FloorRight => vec![
            anchor(end_x(-4), end_y(-1)),
            anchor(end_x(-5), end_y(-1)),
            anchor(end_x(-3), end_y(-1)),
            anchor(end_x(-7), end_y(-2)),
        ],
        PropZoneSnapshot::WallLeft => vec![
            anchor(start_x(2), center_y(0)),
            anchor(start_x(2), center_y(-2)),
            anchor(start_x(4), center_y(2)),
        ],
        PropZoneSnapshot::WallRight => vec![
            anchor(end_x(-6), center_y(0)),
            anchor(end_x(-2), center_y(0)),
            anchor(end_x(-4), center_y(-2)),
            anchor(end_x(-2), center_y(2)),
        ],
        PropZoneSnapshot::AirLeft => vec![
            anchor(start_x(3), start_y(2)),
            anchor(start_x(6), start_y(4)),
            anchor(start_x(2), start_y(6)),
        ],
        PropZoneSnapshot::AirMid => vec![
            anchor(center_x(0), start_y(2)),
            anchor(center_x(-6), start_y(3)),
            anchor(center_x(6), start_y(4)),
            anchor(center_x(0), center_y(-2)),
        ],
        PropZoneSnapshot::AirRight => vec![
            anchor(end_x(-4), start_y(2)),
            anchor(end_x(-2), start_y(4)),
            anchor(end_x(-7), start_y(6)),
        ],
        PropZoneSnapshot::Ceiling => vec![
            anchor(center_x(0), start_y(4)),
            anchor(center_x(-8), start_y(4)),
            anchor(center_x(8), start_y(4)),
            anchor(center_x(0), start_y(1)),
            anchor(center_x(-8), start_y(1)),
            anchor(center_x(8), start_y(1)),
            anchor(start_x(3), start_y(1)),
            anchor(end_x(-3), start_y(1)),
        ],
    }
}

#[allow(clippy::too_many_arguments)]
fn candidate_is_safe(
    bounds: [i16; 4],
    columns: i16,
    rows: i16,
    gauge_radii: [f32; 2],
    hud_reserve: [i16; 4],
    bottom_reserve: [i16; 4],
    accepted_bounds: &[[i16; 4]],
) -> bool {
    bounds[0] >= 0
        && bounds[1] >= 0
        && bounds[2] <= columns
        && bounds[3] <= rows
        && bounds[0] < bounds[2]
        && bounds[1] < bounds[3]
        && bounds_inside_ellipse(bounds, columns, rows, gauge_radii)
        && !intersects(bounds, hud_reserve)
        && !intersects(bounds, bottom_reserve)
        && accepted_bounds
            .iter()
            .all(|accepted| !intersects(bounds, expand(*accepted)))
}

fn bounds_inside_ellipse(bounds: [i16; 4], columns: i16, rows: i16, radii: [f32; 2]) -> bool {
    if radii[0] <= 0.0 || radii[1] <= 0.0 {
        return false;
    }
    let center = [f32::from(columns) / 2.0, f32::from(rows) / 2.0];
    [bounds[0], bounds[2] - 1].into_iter().all(|col| {
        [bounds[1], bounds[3] - 1].into_iter().all(|row| {
            let dx = (f32::from(col) + 0.5 - center[0]) / radii[0];
            let dy = (f32::from(row) + 0.5 - center[1]) / radii[1];
            dx * dx + dy * dy <= 1.0
        })
    })
}

fn intersects(left: [i16; 4], right: [i16; 4]) -> bool {
    left[0] < right[2] && left[2] > right[0] && left[1] < right[3] && left[3] > right[1]
}

fn expand(bounds: [i16; 4]) -> [i16; 4] {
    [bounds[0] - 1, bounds[1] - 1, bounds[2] + 1, bounds[3] + 1]
}

fn expand_and_clip(bounds: [i16; 4], columns: i16, rows: i16) -> [i16; 4] {
    let expanded = expand(bounds);
    [
        expanded[0].clamp(0, columns),
        expanded[1].clamp(0, rows),
        expanded[2].clamp(0, columns),
        expanded[3].clamp(0, rows),
    ]
}

fn tank_rect(bounds: [i16; 4]) -> Option<TankRouteRect> {
    if bounds[0] < 0 || bounds[1] < 0 || bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
        return None;
    }
    Some(TankRouteRect {
        x: u16::try_from(bounds[0]).ok()?,
        y: u16::try_from(bounds[1]).ok()?,
        width: u16::try_from(bounds[2] - bounds[0]).ok()?,
        height: u16::try_from(bounds[3] - bounds[1]).ok()?,
    })
}

const fn is_floor_zone(zone: PropZoneSnapshot) -> bool {
    matches!(
        zone,
        PropZoneSnapshot::FloorLeft | PropZoneSnapshot::FloorMid | PropZoneSnapshot::FloorRight
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::{HabitatPropKind, HABITAT_PROP_CATALOG};
    use crate::presentation::companion_scene::{
        PropPresentationMotion, PropTopologySnapshot, MAX_VISIBLE_PROPS,
    };
    use time::macros::datetime;

    const COLUMNS: u16 = 44;
    const ROWS: u16 = 18;
    const SURFACES: &[(f32, f32)] = &[
        (260.0, 260.0),
        (360.0, 360.0),
        (480.0, 480.0),
        (720.0, 720.0),
        (480.0, 360.0),
        (360.0, 480.0),
    ];

    fn full_prop_topology() -> Vec<PropTopologySnapshot> {
        let records = HABITAT_PROP_CATALOG
            .iter()
            .map(
                |spec| crate::presentation::habitat_inventory::HabitatPropRecord {
                    id: spec.id,
                    earned_at: time::OffsetDateTime::UNIX_EPOCH,
                    kind: spec.kind,
                    display_priority: spec.display_priority,
                },
            )
            .collect::<Vec<_>>();
        crate::presentation::habitat_inventory::visible_trophy_ids(&records)
            .into_iter()
            .chain(crate::presentation::habitat_inventory::visible_accent_ids(
                &records,
                datetime!(2026-07-15 12:00 UTC),
            ))
            .take(MAX_VISIBLE_PROPS)
            .enumerate()
            .filter_map(|(stable_order, id)| {
                let spec = crate::game::habitat::catalog_prop_by_str(id)?;
                Some(PropTopologySnapshot {
                    catalog_id: spec.id,
                    stable_order: u8::try_from(stable_order).unwrap(),
                    zone: spec.zone.into(),
                    authored_depth: spec.pet_layer.into(),
                    presentation_motion: PropPresentationMotion::Static,
                })
            })
            .collect()
    }

    fn resolve_for(
        props: &[PropTopologySnapshot],
        width_points: f32,
        height_points: f32,
    ) -> CompanionComposition {
        resolve_companion_composition(CompanionCompositionInput {
            columns: COLUMNS,
            rows: ROWS,
            width_points,
            height_points,
            bottom_reserved_rows: 5,
            props,
        })
    }

    fn intersects(left: [i16; 4], right: [i16; 4]) -> bool {
        left[0] < right[2] && left[2] > right[0] && left[1] < right[3] && left[3] > right[1]
    }

    fn expanded(bounds: [i16; 4]) -> [i16; 4] {
        [bounds[0] - 1, bounds[1] - 1, bounds[2] + 1, bounds[3] + 1]
    }

    fn expected_gauge_radii(width_points: f32, height_points: f32) -> [f32; 2] {
        let gauge = crate::presentation::companion_effects::perimeter_gauge_layout(
            f64::from(width_points.min(height_points)) / 2.0,
            crate::presentation::companion_effects::COMPANION_GAUGE_GAP_DEGREES,
        );
        let inner_radius_points = gauge.pace.radius - gauge.pace.stroke_width / 2.0;
        [
            (inner_radius_points / (f64::from(width_points) / f64::from(COLUMNS)) - 0.5) as f32,
            (inner_radius_points / (f64::from(height_points) / f64::from(ROWS)) - 0.5) as f32,
        ]
    }

    fn composition_bytes(composition: &CompanionComposition) -> Vec<u8> {
        let placements = composition
            .prop_placements
            .iter()
            .map(|placement| {
                (
                    placement.slot,
                    placement.visible,
                    placement.anchor_cell,
                    placement.bounds_cells,
                    placement.footprint_cells,
                    placement.grounded,
                )
            })
            .collect::<Vec<_>>();
        let reserved = composition
            .tank_reserved_regions
            .iter()
            .map(|rect| [rect.x, rect.y, rect.width, rect.height])
            .collect::<Vec<_>>();
        let foreground = composition
            .tank_foreground_reserved_regions
            .iter()
            .map(|rect| [rect.x, rect.y, rect.width, rect.height])
            .collect::<Vec<_>>();
        serde_json::to_vec(&(
            placements,
            composition.hud_reserve_cells,
            composition.gauge_inner_radius_cells,
            reserved,
            foreground,
        ))
        .expect("serialize deterministic composition fields")
    }

    #[test]
    fn full_cast_props_are_disjoint_and_inside_safe_aperture() {
        let props = full_prop_topology();
        for &(width_points, height_points) in SURFACES {
            let composition = resolve_for(&props, width_points, height_points);
            let expected_radii = expected_gauge_radii(width_points, height_points);
            for (axis, expected_radius) in expected_radii.iter().copied().enumerate() {
                assert!(
                    (composition.gauge_inner_radius_cells[axis] - expected_radius).abs() < 0.0001,
                    "{width_points}x{height_points} axis {axis}"
                );
            }

            let visible = composition
                .prop_placements
                .iter()
                .filter(|placement| placement.visible)
                .collect::<Vec<_>>();
            for placement in &visible {
                let [min_col, min_row, max_col, max_row] = placement.bounds_cells;
                assert!(min_col >= 0 && min_row >= 0);
                assert!(max_col <= COLUMNS as i16 && max_row <= ROWS as i16);
                assert_eq!(
                    placement.footprint_cells,
                    [
                        u16::try_from(max_col - min_col).unwrap(),
                        u16::try_from(max_row - min_row).unwrap(),
                    ]
                );
                let topology = &props[usize::from(placement.slot)];
                assert_eq!(
                    placement.grounded,
                    matches!(
                        topology.zone,
                        PropZoneSnapshot::FloorLeft
                            | PropZoneSnapshot::FloorMid
                            | PropZoneSnapshot::FloorRight
                    )
                );
                let center = [f32::from(COLUMNS) / 2.0, f32::from(ROWS) / 2.0];
                for col in [f32::from(min_col) + 0.5, f32::from(max_col) - 0.5] {
                    for row in [f32::from(min_row) + 0.5, f32::from(max_row) - 0.5] {
                        let dx = (col - center[0]) / expected_radii[0];
                        let dy = (row - center[1]) / expected_radii[1];
                        assert!(
                            dx * dx + dy * dy <= 1.0 + f32::EPSILON,
                            "{width_points}x{height_points} slot {} escaped the gauge-safe aperture",
                            placement.slot
                        );
                    }
                }
            }
            for (index, left) in visible.iter().enumerate() {
                for right in &visible[index + 1..] {
                    assert!(
                        !intersects(expanded(left.bounds_cells), right.bounds_cells),
                        "{width_points}x{height_points} slots {} and {} lost their gutter",
                        left.slot,
                        right.slot
                    );
                }
            }
        }
    }

    #[test]
    fn props_avoid_hud_and_bottom_reserves() {
        let props = full_prop_topology();
        for &(width_points, height_points) in SURFACES {
            let composition = resolve_for(&props, width_points, height_points);
            let expected_hud = [9, 10, 35, 17];
            let expected_bottom = [0, 13, 44, 18];
            assert_eq!(composition.hud_reserve_cells, expected_hud);
            assert_eq!(composition.tank_reserved_regions.len(), 2);
            assert_eq!(
                [
                    composition.tank_reserved_regions[0].x,
                    composition.tank_reserved_regions[0].y,
                    composition.tank_reserved_regions[0].width,
                    composition.tank_reserved_regions[0].height,
                ],
                [9, 10, 26, 7]
            );
            assert_eq!(
                [
                    composition.tank_reserved_regions[1].x,
                    composition.tank_reserved_regions[1].y,
                    composition.tank_reserved_regions[1].width,
                    composition.tank_reserved_regions[1].height,
                ],
                [0, 13, 44, 5]
            );
            for placement in composition
                .prop_placements
                .iter()
                .filter(|placement| placement.visible)
            {
                assert!(!intersects(placement.bounds_cells, expected_hud));
                assert!(!intersects(placement.bounds_cells, expected_bottom));
            }
            let expected_foreground = composition
                .prop_placements
                .iter()
                .filter(|placement| {
                    placement.visible
                        && props[usize::from(placement.slot)].authored_depth
                            == AuthoredDepthSnapshot::Foreground
                })
                .map(|placement| {
                    let bounds = placement.bounds_cells;
                    let min_col = (bounds[0] - 1).clamp(0, COLUMNS as i16) as u16;
                    let min_row = (bounds[1] - 1).clamp(0, ROWS as i16) as u16;
                    let max_col = (bounds[2] + 1).clamp(0, COLUMNS as i16) as u16;
                    let max_row = (bounds[3] + 1).clamp(0, ROWS as i16) as u16;
                    [min_col, min_row, max_col - min_col, max_row - min_row]
                })
                .collect::<Vec<_>>();
            assert_eq!(
                composition
                    .tank_foreground_reserved_regions
                    .iter()
                    .map(|rect| [rect.x, rect.y, rect.width, rect.height])
                    .collect::<Vec<_>>(),
                expected_foreground
            );
        }
    }

    #[test]
    fn composition_is_byte_stable() {
        let props = full_prop_topology();
        for &(width_points, height_points) in SURFACES {
            assert_eq!(
                composition_bytes(&resolve_for(&props, width_points, height_points)),
                composition_bytes(&resolve_for(&props, width_points, height_points)),
                "{width_points}x{height_points}"
            );
        }
    }

    #[test]
    fn small_surfaces_hide_accents_in_stable_order() {
        let props = full_prop_topology();
        let first = resolve_for(&props, 260.0, 260.0);
        let second = resolve_for(&props, 260.0, 260.0);
        let hidden_accents = |composition: &CompanionComposition| {
            composition
                .prop_placements
                .iter()
                .filter(|placement| {
                    !placement.visible
                        && props
                            .get(usize::from(placement.slot))
                            .and_then(|prop| {
                                crate::game::habitat::catalog_prop_by_str(prop.catalog_id)
                            })
                            .is_some_and(|spec| spec.kind == HabitatPropKind::Accent)
                })
                .map(|placement| placement.slot)
                .collect::<Vec<_>>()
        };
        let first_hidden = hidden_accents(&first);
        assert!(
            !first_hidden.is_empty(),
            "the 260pt surface must shed an accent"
        );
        assert_eq!(first_hidden, hidden_accents(&second));
        assert_eq!(
            first_hidden.last().copied(),
            props.last().map(|prop| prop.stable_order),
            "the lowest-priority fixed slot must be hidden first or with the hidden suffix"
        );
    }
}
