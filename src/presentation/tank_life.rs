use crate::game::habitat::{HabitatPetLayer, TankLifeRouteFamily};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TankRouteRect {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) width: u16,
    pub(crate) height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TankRouteAperture {
    pub(crate) center_col: i16,
    pub(crate) center_row: i16,
    pub(crate) radius_cols: u16,
    pub(crate) radius_rows: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TankRouteGeometry {
    pub(crate) habitat: TankRouteRect,
    pub(crate) aperture: Option<TankRouteAperture>,
    pub(crate) literal_floor_allowed: bool,
}

impl TankRouteGeometry {
    pub(crate) const fn round(cols: u16, rows: u16, _bottom_reserved_rows: u16) -> Self {
        Self {
            habitat: TankRouteRect { x: 0, y: 0, width: cols, height: rows },
            aperture: Some(TankRouteAperture {
                center_col: (cols / 2) as i16,
                center_row: (rows / 2) as i16,
                radius_cols: cols / 2,
                radius_rows: rows / 2,
            }),
            literal_floor_allowed: false,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TankRouteInput<'a> {
    pub(crate) catalog_id: &'a str,
    pub(crate) pet_seed: &'a str,
    pub(crate) local_date: time::Date,
    pub(crate) now: time::OffsetDateTime,
    pub(crate) calm: bool,
    pub(crate) geometry: &'a TankRouteGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TankRouteSide {
    Left,
    Right,
    Rear,
    Front,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TankRouteLayer {
    Behind,
    Foreground,
    BehindAnchorForegroundHost,
}

impl From<HabitatPetLayer> for TankRouteLayer {
    fn from(layer: HabitatPetLayer) -> Self {
        match layer {
            HabitatPetLayer::Background | HabitatPetLayer::Behind => Self::Behind,
            HabitatPetLayer::Foreground => Self::Foreground,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TankRouteOutcome {
    pub(crate) route: TankLifeRouteFamily,
    pub(crate) visible: bool,
    pub(crate) origin_col: u16,
    pub(crate) origin_row: u16,
    pub(crate) side: Option<TankRouteSide>,
    pub(crate) layer: TankRouteLayer,
    pub(crate) sprite_variant: u8,
    pub(crate) visible_rows: u8,
    pub(crate) anemone_morph: Option<u8>,
    pub(crate) cadence_ms: u16,
    pub(crate) calm: bool,
}

#[derive(Debug, Clone, Copy)]
struct CellOffset {
    row: i16,
    col: i16,
}

pub(crate) fn resolve_tank_route(input: TankRouteInput<'_>) -> Option<TankRouteOutcome> {
    let spec = crate::game::habitat::TANK_INHABITANT_CATALOG
        .iter()
        .find(|spec| spec.id == input.catalog_id)?;
    let cadence_seconds = if input.calm { 8 } else { 4 };
    let tick = input.now.unix_timestamp().max(0) as u64 / cadence_seconds;
    let private_route = stable_hash(&format!(
        "tank-life-route-v1|{}|{}|{}",
        input.pet_seed, input.local_date, input.catalog_id
    ))
    .wrapping_add(tick);
    let sprite_variant = (private_route & 1) as u8;
    let anemone_morph = (input.catalog_id == crate::game::habitat::ANEMONE_HOST)
        .then(|| anemone_morph_index(input.pet_seed, input.local_date));
    let sprite = sprite_offsets(input.catalog_id, anemone_morph);
    let habitat = input.geometry.habitat;

    let (visible, origin_col, origin_row, side, layer, visible_rows) = match spec.route_family {
        TankLifeRouteFamily::CrossTankSwimmer => {
            let row = habitat.y.saturating_add((habitat.height / 3).max(1));
            let (start, end) = oscillating_bounds(input.geometry, row, &sprite, 3);
            let col = oscillating_col(private_route, start, end);
            (
                true,
                col,
                row,
                None,
                cross_tank_layer(start, end, col).into(),
                1,
            )
        }
        TankLifeRouteFamily::LowerLaneResident => {
            let row = lower_lane_row(input.geometry)
                .saturating_sub(u16::from(spec.id == crate::game::habitat::SAND_RAY));
            let (start, end) = oscillating_bounds(input.geometry, row, &sprite, 4);
            (
                true,
                oscillating_col(private_route, start, end),
                row,
                None,
                TankRouteLayer::Foreground,
                1,
            )
        }
        TankLifeRouteFamily::GlassResident => {
            let right = private_route.is_multiple_of(2);
            let col = if right {
                habitat.x.saturating_add(habitat.width.saturating_sub(2))
            } else {
                habitat.x.saturating_add(1)
            };
            let span = habitat.height.saturating_sub(4).max(1);
            let row = habitat
                .y
                .saturating_add(2)
                .saturating_add((private_route % u64::from(span)) as u16);
            (
                true,
                col,
                row,
                Some(if right {
                    TankRouteSide::Right
                } else {
                    TankRouteSide::Left
                }),
                TankRouteLayer::Foreground,
                1,
            )
        }
        TankLifeRouteFamily::RimResident => {
            let rear = private_route.is_multiple_of(2);
            let row = if rear {
                habitat.y.saturating_add(2)
            } else {
                lower_lane_row(input.geometry).saturating_sub(1)
            };
            let (start, end) = oscillating_bounds(input.geometry, row, &sprite, 5);
            (
                true,
                oscillating_col(private_route, start, end),
                row,
                Some(if rear {
                    TankRouteSide::Rear
                } else {
                    TankRouteSide::Front
                }),
                if rear {
                    TankRouteLayer::Behind
                } else {
                    TankRouteLayer::Foreground
                },
                1,
            )
        }
        TankLifeRouteFamily::LowerEdgeResident => {
            let rows = (private_route % 3) as u8;
            let row = lower_lane_row(input.geometry);
            let expanded = expanded_offsets(&sprite, rows);
            let (start, end) = oscillating_bounds(input.geometry, row, &expanded, 5);
            (
                rows > 0,
                oscillating_col(private_route, start, end),
                row,
                None,
                TankRouteLayer::Foreground,
                rows,
            )
        }
        TankLifeRouteFamily::HostCombo => {
            let width = sprite_width(&sprite).max(1);
            let height = sprite_height(&sprite).max(1);
            let col = habitat
                .x
                .saturating_add(habitat.width.saturating_sub(width) / 2);
            let row = lower_lane_row(input.geometry)
                .saturating_sub(height.saturating_sub(1))
                .saturating_sub(1);
            let right = private_route.is_multiple_of(2);
            (
                true,
                col,
                row,
                Some(if right {
                    TankRouteSide::Right
                } else {
                    TankRouteSide::Left
                }),
                TankRouteLayer::BehindAnchorForegroundHost,
                1,
            )
        }
    };

    Some(TankRouteOutcome {
        route: spec.route_family,
        visible,
        origin_col,
        origin_row,
        side,
        layer,
        sprite_variant,
        visible_rows,
        anemone_morph,
        cadence_ms: (cadence_seconds * 1_000) as u16,
        calm: input.calm,
    })
}

fn sprite_offsets(id: &str, morph: Option<u8>) -> Vec<CellOffset> {
    let cells = |cols: &[i16], rows: &[i16]| {
        cols.iter()
            .zip(rows)
            .map(|(&col, &row)| CellOffset { row, col })
            .collect()
    };
    match id {
        crate::game::habitat::GLASS_SHRIMP | crate::game::habitat::NEEDLEFISH => {
            cells(&[0, 1, 2], &[0, 0, 0])
        }
        crate::game::habitat::GLASS_SNAIL
        | crate::game::habitat::BURROWER
        | crate::game::habitat::RIM_SKIMMER
        | crate::game::habitat::SAND_RAY => cells(&[0], &[0]),
        crate::game::habitat::SCHOOLLET => cells(&[0, 2], &[0, 0]),
        crate::game::habitat::ANEMONE_HOST => match morph.unwrap_or(0) % 4 {
            0 => cells(&[1, 0, 1], &[0, 1, 1]),
            1 => cells(&[0, 1, 2, 0, 1, 2], &[0, 0, 0, 1, 1, 1]),
            2 => cells(&[0, 1, 0, 1, 0, 1], &[0, 0, 1, 1, 2, 2]),
            _ => cells(&[0, 1, 0, 1], &[0, 0, 1, 1]),
        },
        _ => Vec::new(),
    }
}

fn expanded_offsets(sprite: &[CellOffset], rows: u8) -> Vec<CellOffset> {
    (0..rows)
        .flat_map(|row| {
            sprite.iter().map(move |cell| CellOffset {
                row: cell.row - i16::from(row),
                col: cell.col,
            })
        })
        .collect()
}

fn lower_lane_row(geometry: &TankRouteGeometry) -> u16 {
    let gap = if geometry.literal_floor_allowed { 1 } else { 4 };
    geometry
        .habitat
        .y
        .saturating_add(geometry.habitat.height.saturating_sub(gap + 1))
        .max(geometry.habitat.y)
}

fn oscillating_bounds(
    geometry: &TankRouteGeometry,
    row: u16,
    sprite: &[CellOffset],
    padding: u16,
) -> (u16, u16) {
    let habitat = geometry.habitat;
    let mut start = habitat.x.saturating_add(padding);
    let mut end = habitat
        .x
        .saturating_add(habitat.width.saturating_sub(sprite_width(sprite).max(1)))
        .saturating_sub(padding);
    while start < end && !origin_inside_aperture(geometry, sprite, start, row) {
        start = start.saturating_add(1);
    }
    while end > start && !origin_inside_aperture(geometry, sprite, end, row) {
        end = end.saturating_sub(1);
    }
    (start, end)
}

fn oscillating_col(private_route: u64, start: u16, end: u16) -> u16 {
    let span = end.saturating_sub(start);
    if span == 0 {
        return start;
    }
    let step = private_route % (u64::from(span) * 2);
    if step <= u64::from(span) {
        start.saturating_add(step as u16)
    } else {
        end.saturating_sub((step - u64::from(span)) as u16)
    }
}

fn cross_tank_layer(start: u16, end: u16, col: u16) -> HabitatPetLayer {
    let span = u32::from(end.saturating_sub(start));
    if span == 0 {
        return HabitatPetLayer::Behind;
    }
    let progress = u32::from(col.saturating_sub(start)).min(span);
    if progress * 3 >= span && progress * 3 <= span * 2 {
        HabitatPetLayer::Foreground
    } else {
        HabitatPetLayer::Behind
    }
}

fn origin_inside_aperture(
    geometry: &TankRouteGeometry,
    sprite: &[CellOffset],
    base_col: u16,
    base_row: u16,
) -> bool {
    sprite.iter().all(|cell| {
        let col = i32::from(base_col) + i32::from(cell.col);
        let row = i32::from(base_row) + i32::from(cell.row);
        col >= 0
            && row >= 0
            && u16::try_from(col)
                .ok()
                .zip(u16::try_from(row).ok())
                .is_some_and(|(col, row)| inside_aperture(geometry, col, row))
    })
}

fn inside_aperture(geometry: &TankRouteGeometry, col: u16, row: u16) -> bool {
    let Some(mask) = geometry.aperture else {
        return true;
    };
    let dx = i32::from(col) - i32::from(mask.center_col);
    let dy = i32::from(row) - i32::from(mask.center_row);
    let rx = i32::from(mask.radius_cols.max(1));
    let ry = i32::from(mask.radius_rows.max(1));
    (dx * dx * ry * ry + dy * dy * rx * rx) <= rx * rx * ry * ry
}

fn sprite_width(sprite: &[CellOffset]) -> u16 {
    sprite
        .iter()
        .filter_map(|cell| u16::try_from(i32::from(cell.col) + 1).ok())
        .max()
        .unwrap_or(0)
}

fn sprite_height(sprite: &[CellOffset]) -> u16 {
    sprite
        .iter()
        .filter_map(|cell| u16::try_from(i32::from(cell.row) + 1).ok())
        .max()
        .unwrap_or(0)
}

fn anemone_morph_index(pet_seed: &str, local_date: time::Date) -> u8 {
    (stable_hash(&format!("anemone-morph-v1|{pet_seed}|{local_date}")) % 4) as u8
}

fn stable_hash(input: &str) -> u64 {
    const OFFSET: u64 = 1_469_598_103_934_665_603;
    const PRIME: u64 = 1_099_511_628_211;
    input.bytes().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash.wrapping_mul(PRIME)
    })
}
