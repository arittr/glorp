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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TankRouteGeometry {
    pub(crate) habitat: TankRouteRect,
    pub(crate) aperture: Option<TankRouteAperture>,
    pub(crate) reserved_regions: Vec<TankRouteRect>,
    pub(crate) foreground_reserved_regions: Vec<TankRouteRect>,
    pub(crate) literal_floor_allowed: bool,
}

impl TankRouteGeometry {
    pub(crate) fn round(cols: u16, rows: u16, bottom_reserved_rows: u16) -> Self {
        let bottom_reserved_rows = bottom_reserved_rows.min(rows);
        Self {
            habitat: TankRouteRect { x: 0, y: 0, width: cols, height: rows },
            aperture: Some(TankRouteAperture {
                center_col: (cols / 2) as i16,
                center_row: (rows / 2) as i16,
                radius_cols: cols / 2,
                radius_rows: rows / 2,
            }),
            reserved_regions: (bottom_reserved_rows > 0)
                .then(|| TankRouteRect {
                    x: 0,
                    y: rows.saturating_sub(bottom_reserved_rows),
                    width: cols,
                    height: bottom_reserved_rows,
                })
                .into_iter()
                .collect(),
            foreground_reserved_regions: Vec::new(),
            literal_floor_allowed: false,
        }
    }
}

pub(crate) const fn pet_face_reserved_region(pet_rect: TankRouteRect) -> TankRouteRect {
    TankRouteRect {
        x: pet_rect.x + pet_rect.width / 4,
        y: pet_rect.y.saturating_add(1),
        width: pet_rect.width / 2,
        height: if pet_rect.height < 4 {
            pet_rect.height
        } else {
            4
        },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub(crate) struct TankRouteCell {
    pub(crate) row: u16,
    pub(crate) col: u16,
    pub(crate) glyph: char,
    pub(crate) layer: TankRouteLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub(crate) cells: Vec<TankRouteCell>,
    pub(crate) bounds: Option<TankRouteRect>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TankRouteSpriteCell {
    pub(crate) row: i16,
    pub(crate) col: i16,
    pub(crate) glyph: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TankPaint {
    pub(crate) color_srgb8: [u8; 3],
    pub(crate) bold: bool,
}

pub(crate) fn tank_paint_for(catalog_id: &str) -> Option<TankPaint> {
    let color_srgb8 = match catalog_id {
        crate::game::habitat::GLASS_SHRIMP => [255, 196, 146],
        crate::game::habitat::NEEDLEFISH | crate::game::habitat::SCHOOLLET => [126, 238, 255],
        crate::game::habitat::GLASS_SNAIL => [216, 192, 144],
        crate::game::habitat::BURROWER | crate::game::habitat::SAND_RAY => [200, 176, 136],
        crate::game::habitat::RIM_SKIMMER => [184, 216, 240],
        crate::game::habitat::ANEMONE_HOST => [232, 176, 208],
        _ => return None,
    };
    Some(TankPaint { color_srgb8, bold: true })
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
    let sprite = tank_sprite_cells(input.catalog_id, sprite_variant, anemone_morph);
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

    let cells = resolve_visible_cells(
        input.geometry,
        spec.route_family,
        &sprite,
        visible,
        origin_col,
        origin_row,
        side,
        layer,
        visible_rows,
    );
    let bounds = route_bounds(&cells);

    Some(TankRouteOutcome {
        route: spec.route_family,
        visible: !cells.is_empty(),
        origin_col,
        origin_row,
        side,
        layer,
        sprite_variant,
        visible_rows,
        anemone_morph,
        cadence_ms: (cadence_seconds * 1_000) as u16,
        calm: input.calm,
        cells,
        bounds,
    })
}

pub(crate) fn tank_sprite_cells(
    id: &str,
    sprite_variant: u8,
    morph: Option<u8>,
) -> Vec<TankRouteSpriteCell> {
    let cells = |cols: &[i16], rows: &[i16], glyphs: &[char]| {
        cols.iter()
            .zip(rows)
            .zip(glyphs)
            .map(|((&col, &row), &glyph)| TankRouteSpriteCell { row, col, glyph })
            .collect()
    };
    match id {
        crate::game::habitat::GLASS_SHRIMP => cells(
            &[0, 1, 2],
            &[0, 0, 0],
            &[
                '╭',
                if sprite_variant.is_multiple_of(2) {
                    '~'
                } else {
                    '≈'
                },
                '╯',
            ],
        ),
        crate::game::habitat::NEEDLEFISH => cells(&[0, 1, 2], &[0, 0, 0], &['‹', '─', '•']),
        crate::game::habitat::GLASS_SNAIL => cells(&[0], &[0], &['◔']),
        crate::game::habitat::BURROWER => cells(&[0], &[0], &['▴']),
        crate::game::habitat::RIM_SKIMMER => cells(&[0], &[0], &['◜']),
        crate::game::habitat::SAND_RAY => cells(&[0], &[0], &['▱']),
        crate::game::habitat::SCHOOLLET => cells(&[0, 2], &[0, 0], &['‹', '‹']),
        crate::game::habitat::ANEMONE_HOST => match morph.unwrap_or(0) % 4 {
            0 => cells(&[1, 0, 1], &[0, 1, 1], &['✺', '╰', '╯']),
            1 => cells(
                &[0, 1, 2, 0, 1, 2],
                &[0, 0, 0, 1, 1, 1],
                &['╵', '╷', '╵', '╰', '┬', '╯'],
            ),
            2 => cells(
                &[0, 1, 0, 1, 0, 1],
                &[0, 0, 1, 1, 2, 2],
                &['⌁', '⌁', '╰', '╮', '╱', '╲'],
            ),
            _ => cells(&[0, 1, 0, 1], &[0, 0, 1, 1], &['⁙', '⁙', '╰', '╯']),
        },
        _ => Vec::new(),
    }
}

pub(crate) fn tank_host_fish_sprite() -> Vec<TankRouteSpriteCell> {
    vec![
        TankRouteSpriteCell { row: 0, col: 0, glyph: '›' },
        TankRouteSpriteCell { row: 0, col: 1, glyph: '·' },
    ]
}

fn expanded_offsets(sprite: &[TankRouteSpriteCell], rows: u8) -> Vec<TankRouteSpriteCell> {
    (0..rows)
        .flat_map(|row| {
            sprite.iter().map(move |cell| TankRouteSpriteCell {
                row: cell.row - i16::from(row),
                col: cell.col,
                glyph: cell.glyph,
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

#[allow(clippy::too_many_arguments)]
fn resolve_visible_cells(
    geometry: &TankRouteGeometry,
    route: TankLifeRouteFamily,
    sprite: &[TankRouteSpriteCell],
    route_visible: bool,
    origin_col: u16,
    origin_row: u16,
    side: Option<TankRouteSide>,
    layer: TankRouteLayer,
    visible_rows: u8,
) -> Vec<TankRouteCell> {
    if !route_visible {
        return Vec::new();
    }

    let (resolved_sprite, sprite_layer) = match route {
        TankLifeRouteFamily::LowerEdgeResident => (
            expanded_offsets(sprite, visible_rows),
            TankRouteLayer::Foreground,
        ),
        TankLifeRouteFamily::CrossTankSwimmer | TankLifeRouteFamily::RimResident => {
            (sprite.to_vec(), layer)
        }
        TankLifeRouteFamily::LowerLaneResident | TankLifeRouteFamily::GlassResident => {
            (sprite.to_vec(), TankRouteLayer::Foreground)
        }
        TankLifeRouteFamily::HostCombo => (sprite.to_vec(), TankRouteLayer::Behind),
    };
    let mut cells = visible_sprite_cells(
        geometry,
        &resolved_sprite,
        origin_col,
        origin_row,
        sprite_layer,
    );

    if route == TankLifeRouteFamily::HostCombo {
        let host_col = if side == Some(TankRouteSide::Right) {
            origin_col
                .saturating_add(sprite_width(sprite).max(1))
                .saturating_add(1)
        } else {
            origin_col.saturating_sub(3)
        };
        cells.extend(visible_sprite_cells(
            geometry,
            &tank_host_fish_sprite(),
            host_col,
            origin_row.saturating_sub(1),
            TankRouteLayer::Foreground,
        ));
    }
    cells
}

fn visible_sprite_cells(
    geometry: &TankRouteGeometry,
    sprite: &[TankRouteSpriteCell],
    base_col: u16,
    base_row: u16,
    layer: TankRouteLayer,
) -> Vec<TankRouteCell> {
    sprite
        .iter()
        .filter_map(|cell| {
            let col = i32::from(base_col) + i32::from(cell.col);
            let row = i32::from(base_row) + i32::from(cell.row);
            let col = u16::try_from(col).ok()?;
            let row = u16::try_from(row).ok()?;
            cell_allowed(geometry, col, row, layer).then_some(TankRouteCell {
                row,
                col,
                glyph: cell.glyph,
                layer,
            })
        })
        .collect()
}

fn cell_allowed(geometry: &TankRouteGeometry, col: u16, row: u16, layer: TankRouteLayer) -> bool {
    rect_contains(geometry.habitat, col, row)
        && inside_aperture(geometry, col, row)
        && !geometry
            .reserved_regions
            .iter()
            .any(|region| rect_contains(*region, col, row))
        && !(layer == TankRouteLayer::Foreground
            && geometry
                .foreground_reserved_regions
                .iter()
                .any(|region| rect_contains(*region, col, row)))
}

fn route_bounds(cells: &[TankRouteCell]) -> Option<TankRouteRect> {
    let min_col = cells.iter().map(|cell| cell.col).min()?;
    let max_col = cells.iter().map(|cell| cell.col).max()?;
    let min_row = cells.iter().map(|cell| cell.row).min()?;
    let max_row = cells.iter().map(|cell| cell.row).max()?;
    Some(TankRouteRect {
        x: min_col,
        y: min_row,
        width: max_col.saturating_sub(min_col).saturating_add(1),
        height: max_row.saturating_sub(min_row).saturating_add(1),
    })
}

fn rect_contains(rect: TankRouteRect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn oscillating_bounds(
    geometry: &TankRouteGeometry,
    row: u16,
    sprite: &[TankRouteSpriteCell],
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
    sprite: &[TankRouteSpriteCell],
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

fn sprite_width(sprite: &[TankRouteSpriteCell]) -> u16 {
    sprite
        .iter()
        .filter_map(|cell| u16::try_from(i32::from(cell.col) + 1).ok())
        .max()
        .unwrap_or(0)
}

fn sprite_height(sprite: &[TankRouteSpriteCell]) -> u16 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    #[test]
    fn tank_paint_catalog_matches_shipping_colors() {
        let expected = [
            (crate::game::habitat::GLASS_SHRIMP, [255, 196, 146]),
            (crate::game::habitat::NEEDLEFISH, [126, 238, 255]),
            (crate::game::habitat::GLASS_SNAIL, [216, 192, 144]),
            (crate::game::habitat::BURROWER, [200, 176, 136]),
            (crate::game::habitat::RIM_SKIMMER, [184, 216, 240]),
            (crate::game::habitat::SAND_RAY, [200, 176, 136]),
            (crate::game::habitat::SCHOOLLET, [126, 238, 255]),
            (crate::game::habitat::ANEMONE_HOST, [232, 176, 208]),
        ];

        assert_eq!(
            expected.len(),
            crate::game::habitat::TANK_INHABITANT_CATALOG.len()
        );
        for (spec, (catalog_id, color_srgb8)) in crate::game::habitat::TANK_INHABITANT_CATALOG
            .iter()
            .zip(expected)
        {
            assert_eq!(spec.id, catalog_id);
            assert_eq!(
                tank_paint_for(catalog_id),
                Some(TankPaint { color_srgb8, bold: true })
            );
        }
    }

    #[test]
    fn tank_paint_rejects_unknown_catalog_ids() {
        assert_eq!(tank_paint_for("future_tank_guest"), None);
    }

    #[test]
    fn round_resolver_returns_only_final_cells_outside_hud_and_aperture_clips() {
        let geometry = TankRouteGeometry::round(44, 18, 5);
        let mut saw_hidden_route = false;

        for spec in crate::game::habitat::TANK_INHABITANT_CATALOG {
            for now in [
                datetime!(2026-07-08 00:00 UTC),
                datetime!(2026-07-08 00:00:04 UTC),
                datetime!(2026-07-08 00:00:08 UTC),
                datetime!(2026-07-08 00:00:32 UTC),
            ] {
                let outcome = resolve_tank_route(TankRouteInput {
                    catalog_id: spec.id,
                    pet_seed: "private-resolver-test-seed",
                    local_date: date!(2026 - 07 - 08),
                    now,
                    calm: false,
                    geometry: &geometry,
                })
                .expect("known tank route");

                assert_eq!(outcome.visible, !outcome.cells.is_empty());
                assert_eq!(outcome.bounds.is_some(), outcome.visible);
                for cell in &outcome.cells {
                    assert!(rect_contains(geometry.habitat, cell.col, cell.row));
                    assert!(inside_aperture(&geometry, cell.col, cell.row));
                    assert!(!geometry
                        .reserved_regions
                        .iter()
                        .any(|region| rect_contains(*region, cell.col, cell.row)));
                }
                saw_hidden_route |= !outcome.visible;
            }
        }

        assert!(saw_hidden_route, "fixture did not exercise final clipping");
    }

    #[test]
    fn foreground_pet_reserve_is_layer_specific() {
        let mut geometry = TankRouteGeometry::round(44, 18, 5);
        geometry.foreground_reserved_regions.push(TankRouteRect {
            x: 0,
            y: 0,
            width: 44,
            height: 18,
        });

        let outcome = resolve_tank_route(TankRouteInput {
            catalog_id: crate::game::habitat::NEEDLEFISH,
            pet_seed: "private-resolver-test-seed",
            local_date: date!(2026 - 07 - 08),
            now: datetime!(2026-07-08 00:00 UTC),
            calm: false,
            geometry: &geometry,
        })
        .expect("known tank route");

        assert!(
            outcome.visible,
            "behind route should survive foreground reserve"
        );
        assert!(outcome
            .cells
            .iter()
            .all(|cell| cell.layer == TankRouteLayer::Behind));
    }
}
