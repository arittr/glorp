use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::game::habitat::{HabitatPetLayer, TankLifeRouteFamily};
use crate::storage::state::TankInhabitantId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankLifeSurface {
    Watch,
    Round,
    Menubar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundApertureMask {
    pub center_col: i16,
    pub center_row: i16,
    pub radius_cols: u16,
    pub radius_rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeSurfaceGeometry {
    pub surface: TankLifeSurface,
    pub habitat: Rect,
    pub aperture_mask: Option<RoundApertureMask>,
    pub reserved_regions: Vec<Rect>,
    pub max_moving_slots: usize,
    pub literal_floor_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedTankLifeCast {
    pub canonical_ids: Vec<TankInhabitantId>,
    pub rendered_ids: Vec<TankInhabitantId>,
    pub skipped: Vec<TankLifeSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeSkip {
    pub id: TankInhabitantId,
    pub reason: TankLifeSkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TankLifeSkipReason {
    UnknownCatalogId,
    SurfaceBudget,
    HabitatTooSmall,
    ReservedRegionCollision,
    ApertureCollision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnemoneMorph {
    Flower,
    Comb,
    Crown,
    DotColony,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteCell {
    pub row: i16,
    pub col: i16,
    pub glyph: char,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TankLifeCell {
    pub inhabitant_id: TankInhabitantId,
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
    pub pet_layer: HabitatPetLayer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TankLifePlacement {
    pub inhabitant_id: TankInhabitantId,
    pub cells: Vec<TankLifeCell>,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TankLifeLayerSegmentSummary {
    pub inhabitant_id: TankInhabitantId,
    pub pet_layer: HabitatPetLayer,
    pub cell_count: usize,
}

#[derive(Debug, Clone)]
pub struct TankLifeRenderInput<'a> {
    pub rendered_ids: Vec<TankInhabitantId>,
    pub pet_seed: &'a str,
    pub local_date: time::Date,
    pub now: time::OffsetDateTime,
    pub geometry: &'a TankLifeSurfaceGeometry,
    pub pet_protected_regions: &'a [Rect],
    pub color_capability: crate::tui::style::ColorCapability,
    pub life_profile: crate::tui::life::PetLifeProfile,
}

impl TankLifeSurfaceGeometry {
    #[cfg(test)]
    pub fn round_for_test(cols: u16, rows: u16, max_moving_slots: usize) -> Self {
        Self {
            surface: TankLifeSurface::Round,
            habitat: Rect::new(0, 0, cols, rows),
            aperture_mask: Some(RoundApertureMask {
                center_col: (cols / 2) as i16,
                center_row: (rows / 2) as i16,
                radius_cols: cols / 2,
                radius_rows: rows / 2,
            }),
            reserved_regions: vec![Rect::new(0, rows.saturating_sub(4), cols, 4)],
            max_moving_slots,
            literal_floor_allowed: false,
        }
    }

    pub fn cell_inside_aperture(&self, col: u16, row: u16) -> bool {
        let Some(mask) = self.aperture_mask else {
            return true;
        };
        let dx = i32::from(col) - i32::from(mask.center_col);
        let dy = i32::from(row) - i32::from(mask.center_row);
        let rx = i32::from(mask.radius_cols.max(1));
        let ry = i32::from(mask.radius_rows.max(1));
        (dx * dx * ry * ry + dy * dy * rx * rx) <= rx * rx * ry * ry
    }
}

pub fn watch_tank_life_geometry(
    scene: &crate::tui::component::PetSceneLayout,
) -> TankLifeSurfaceGeometry {
    TankLifeSurfaceGeometry {
        surface: TankLifeSurface::Watch,
        habitat: scene.habitat,
        aperture_mask: None,
        reserved_regions: scene.speech.into_iter().collect(),
        max_moving_slots: 5,
        literal_floor_allowed: true,
    }
}

pub fn pet_face_protected_regions(pet_art: Rect) -> Vec<Rect> {
    vec![Rect::new(
        pet_art.x + pet_art.width / 4,
        pet_art.y + 1,
        pet_art.width / 2,
        4.min(pet_art.height),
    )]
}

pub fn canonical_daily_cast(
    unlocked: &[crate::tui::view_model::EarnedTankInhabitantView],
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> Vec<TankInhabitantId> {
    let mut known = unlocked
        .iter()
        .filter(|earned| crate::game::habitat::tank_inhabitant_spec(&earned.id).is_some())
        .collect::<Vec<_>>();
    known.sort_by(|a, b| a.id.cmp(&b.id));

    let target = canonical_target_count(known.len(), pet_seed, local_date, calendar_age_days);
    if target == 0 {
        return Vec::new();
    }

    let mut scored = known
        .into_iter()
        .map(|earned| {
            let score = stable_hash(&format!(
                "tank-life-cast-v1|{pet_seed}|{local_date}|{}",
                earned.id.as_str()
            ));
            (score, earned.id.clone())
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, id)| (*score, id.clone()));
    scored
        .into_iter()
        .take(target.min(5))
        .map(|(_, id)| id)
        .collect()
}

pub fn canonical_target_count(
    unlocked_len: usize,
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> usize {
    if unlocked_len <= 2 {
        return unlocked_len;
    }
    let flip = (stable_hash(&format!(
        "tank-life-target-v1|{pet_seed}|{local_date}|{calendar_age_days}"
    )) & 1) as usize;
    let target = if calendar_age_days < 21 {
        2 + flip
    } else if calendar_age_days < 60 {
        3 + flip
    } else {
        4 + flip
    };
    target.min(unlocked_len).min(5)
}

pub fn anemone_morph_for_day(pet_seed: &str, local_date: time::Date) -> AnemoneMorph {
    match stable_hash(&format!("anemone-morph-v1|{pet_seed}|{local_date}")) % 4 {
        0 => AnemoneMorph::Flower,
        1 => AnemoneMorph::Comb,
        2 => AnemoneMorph::Crown,
        _ => AnemoneMorph::DotColony,
    }
}

pub fn anemone_anchor_sprite(morph: AnemoneMorph) -> Vec<SpriteCell> {
    match morph {
        AnemoneMorph::Flower => vec![
            SpriteCell { row: 0, col: 1, glyph: '✺' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '╯' },
        ],
        AnemoneMorph::Comb => vec![
            SpriteCell { row: 0, col: 0, glyph: '╵' },
            SpriteCell { row: 0, col: 1, glyph: '╷' },
            SpriteCell { row: 0, col: 2, glyph: '╵' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '┬' },
            SpriteCell { row: 1, col: 2, glyph: '╯' },
        ],
        AnemoneMorph::Crown => vec![
            SpriteCell { row: 0, col: 0, glyph: '⌁' },
            SpriteCell { row: 0, col: 1, glyph: '⌁' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '╮' },
            SpriteCell { row: 2, col: 0, glyph: '╱' },
            SpriteCell { row: 2, col: 1, glyph: '╲' },
        ],
        AnemoneMorph::DotColony => vec![
            SpriteCell { row: 0, col: 0, glyph: '⁙' },
            SpriteCell { row: 0, col: 1, glyph: '⁙' },
            SpriteCell { row: 1, col: 0, glyph: '╰' },
            SpriteCell { row: 1, col: 1, glyph: '╯' },
        ],
    }
}

pub fn host_fish_sprite() -> Vec<SpriteCell> {
    vec![
        SpriteCell { row: 0, col: 0, glyph: '›' },
        SpriteCell { row: 0, col: 1, glyph: '·' },
    ]
}

pub fn validate_tank_life_catalog() -> std::result::Result<(), String> {
    use unicode_width::UnicodeWidthChar;

    let samples = [
        sprite_for(crate::game::habitat::GLASS_SHRIMP, 0, None),
        sprite_for(crate::game::habitat::GLASS_SHRIMP, 1, None),
        sprite_for(crate::game::habitat::NEEDLEFISH, 0, None),
        sprite_for(crate::game::habitat::GLASS_SNAIL, 0, None),
        sprite_for(crate::game::habitat::BURROWER, 0, None),
        sprite_for(crate::game::habitat::RIM_SKIMMER, 0, None),
        sprite_for(crate::game::habitat::SAND_RAY, 0, None),
        sprite_for(crate::game::habitat::SCHOOLLET, 0, None),
        anemone_anchor_sprite(AnemoneMorph::Flower),
        anemone_anchor_sprite(AnemoneMorph::Comb),
        anemone_anchor_sprite(AnemoneMorph::Crown),
        anemone_anchor_sprite(AnemoneMorph::DotColony),
        host_fish_sprite(),
    ];

    for sprite in samples {
        for cell in sprite {
            if UnicodeWidthChar::width(cell.glyph) != Some(1) {
                return Err(format!(
                    "tank-life glyph {:?} is not terminal width 1",
                    cell.glyph
                ));
            }
        }
    }
    Ok(())
}

pub fn tank_life_placements_for(input: &TankLifeRenderInput<'_>) -> Vec<TankLifePlacement> {
    input
        .rendered_ids
        .iter()
        .filter_map(|id| placement_for_id(id, input))
        .collect()
}

pub fn layer_segment_summaries(
    placements: &[TankLifePlacement],
) -> Vec<TankLifeLayerSegmentSummary> {
    let mut summaries = Vec::new();
    for placement in placements {
        for layer in [
            HabitatPetLayer::Background,
            HabitatPetLayer::Behind,
            HabitatPetLayer::Foreground,
        ] {
            let cell_count = placement
                .cells
                .iter()
                .filter(|cell| cell.pet_layer == layer)
                .count();
            if cell_count > 0 {
                summaries.push(TankLifeLayerSegmentSummary {
                    inhabitant_id: placement.inhabitant_id.clone(),
                    pet_layer: layer,
                    cell_count,
                });
            }
        }
    }
    summaries
}

pub fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

pub fn project_tank_life_cast(
    canonical_ids: &[TankInhabitantId],
    geometry: &TankLifeSurfaceGeometry,
) -> RenderedTankLifeCast {
    let mut rendered_ids = Vec::new();
    let mut skipped = Vec::new();

    for id in canonical_ids {
        let Some(spec) = crate::game::habitat::tank_inhabitant_spec(id) else {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::UnknownCatalogId,
            });
            continue;
        };
        if rendered_ids.len() >= geometry.max_moving_slots {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::SurfaceBudget,
            });
            continue;
        }
        if !footprint_can_fit(spec.id, geometry) {
            skipped.push(TankLifeSkip {
                id: id.clone(),
                reason: TankLifeSkipReason::HabitatTooSmall,
            });
            continue;
        }
        rendered_ids.push(id.clone());
    }

    RenderedTankLifeCast {
        canonical_ids: canonical_ids.to_vec(),
        rendered_ids,
        skipped,
    }
}

fn footprint_can_fit(id: &str, geometry: &TankLifeSurfaceGeometry) -> bool {
    let min = match id {
        crate::game::habitat::ANEMONE_HOST => (8, 6),
        crate::game::habitat::SCHOOLLET => (8, 3),
        _ => (4, 3),
    };
    geometry.habitat.width >= min.0 && geometry.habitat.height >= min.1
}

fn sprite_for(id: &str, phase: u64, morph: Option<AnemoneMorph>) -> Vec<SpriteCell> {
    match id {
        crate::game::habitat::GLASS_SHRIMP => {
            if phase.is_multiple_of(2) {
                vec![
                    SpriteCell { row: 0, col: 0, glyph: '╭' },
                    SpriteCell { row: 0, col: 1, glyph: '~' },
                    SpriteCell { row: 0, col: 2, glyph: '╯' },
                ]
            } else {
                vec![
                    SpriteCell { row: 0, col: 0, glyph: '╭' },
                    SpriteCell { row: 0, col: 1, glyph: '≈' },
                    SpriteCell { row: 0, col: 2, glyph: '╯' },
                ]
            }
        }
        crate::game::habitat::NEEDLEFISH => vec![
            SpriteCell { row: 0, col: 0, glyph: '‹' },
            SpriteCell { row: 0, col: 1, glyph: '─' },
            SpriteCell { row: 0, col: 2, glyph: '•' },
        ],
        crate::game::habitat::GLASS_SNAIL => {
            vec![SpriteCell { row: 0, col: 0, glyph: '◔' }]
        }
        crate::game::habitat::BURROWER => {
            vec![SpriteCell { row: 0, col: 0, glyph: '▴' }]
        }
        crate::game::habitat::RIM_SKIMMER => {
            vec![SpriteCell { row: 0, col: 0, glyph: '◜' }]
        }
        crate::game::habitat::SAND_RAY => {
            vec![SpriteCell { row: 0, col: 0, glyph: '▱' }]
        }
        crate::game::habitat::SCHOOLLET => vec![
            SpriteCell { row: 0, col: 0, glyph: '‹' },
            SpriteCell { row: 0, col: 2, glyph: '‹' },
        ],
        crate::game::habitat::ANEMONE_HOST => {
            anemone_anchor_sprite(morph.unwrap_or(AnemoneMorph::Flower))
        }
        _ => Vec::new(),
    }
}

fn placement_for_id(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
) -> Option<TankLifePlacement> {
    let spec = crate::game::habitat::tank_inhabitant_spec(id)?;
    let phase = route_phase(id.as_str(), input);
    let morph = if id.as_str() == crate::game::habitat::ANEMONE_HOST {
        Some(anemone_morph_for_day(input.pet_seed, input.local_date))
    } else {
        None
    };
    let sprite = sprite_for(id.as_str(), phase, morph);
    if sprite.is_empty() {
        return None;
    }
    let style = tank_life_style(spec.low_color_key, input.color_capability);

    match spec.route_family {
        TankLifeRouteFamily::CrossTankSwimmer => cross_tank_placement(id, input, &sprite, style),
        TankLifeRouteFamily::LowerLaneResident => lower_lane_placement(
            id,
            input,
            &sprite,
            style,
            spec.id == crate::game::habitat::SAND_RAY,
        ),
        TankLifeRouteFamily::GlassResident => glass_placement(id, input, &sprite, style),
        TankLifeRouteFamily::RimResident => rim_placement(id, input, &sprite, style),
        TankLifeRouteFamily::LowerEdgeResident => lower_edge_placement(id, input, &sprite, style),
        TankLifeRouteFamily::HostCombo => host_combo_placement(id, input, &sprite, style),
    }
}

fn cross_tank_placement(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    sprite: &[SpriteCell],
    style: Style,
) -> Option<TankLifePlacement> {
    let row = input
        .geometry
        .habitat
        .y
        .saturating_add((input.geometry.habitat.height / 3).max(1));
    let (start, end) = oscillating_bounds(input, row, sprite, 3);
    let col = oscillating_col_between(input, id.as_str(), start, end);
    let layer = cross_tank_layer_for_col(start, end, col);
    build_placement(id, input, sprite, col, row, style, move |_| layer)
}

fn lower_lane_placement(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    sprite: &[SpriteCell],
    style: Style,
    extra_pet_gap: bool,
) -> Option<TankLifePlacement> {
    let row = lower_lane_row(input.geometry).saturating_sub(u16::from(extra_pet_gap));
    let col = oscillating_col(input, id.as_str(), row, sprite, 4);
    build_placement(id, input, sprite, col, row, style, |_| {
        HabitatPetLayer::Foreground
    })
}

fn glass_placement(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    sprite: &[SpriteCell],
    style: Style,
) -> Option<TankLifePlacement> {
    let habitat = input.geometry.habitat;
    let phase = route_phase(id.as_str(), input);
    let right_side = phase.is_multiple_of(2);
    let col = if right_side {
        habitat.x.saturating_add(habitat.width.saturating_sub(2))
    } else {
        habitat.x.saturating_add(1)
    };
    let height_span = habitat.height.saturating_sub(4).max(1);
    let row = habitat
        .y
        .saturating_add(2)
        .saturating_add((phase % u64::from(height_span)) as u16);
    build_placement(id, input, sprite, col, row, style, |_| {
        HabitatPetLayer::Foreground
    })
}

fn rim_placement(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    sprite: &[SpriteCell],
    style: Style,
) -> Option<TankLifePlacement> {
    let habitat = input.geometry.habitat;
    let phase = route_phase(id.as_str(), input);
    let rear_arc = phase.is_multiple_of(2);
    let row = if rear_arc {
        habitat.y.saturating_add(2)
    } else {
        lower_lane_row(input.geometry).saturating_sub(1)
    };
    let col = oscillating_col(input, id.as_str(), row, sprite, 5);
    build_placement(id, input, sprite, col, row, style, move |_| {
        if rear_arc {
            HabitatPetLayer::Behind
        } else {
            HabitatPetLayer::Foreground
        }
    })
}

fn lower_edge_placement(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    sprite: &[SpriteCell],
    style: Style,
) -> Option<TankLifePlacement> {
    let visible_rows = (route_phase(id.as_str(), input) % 3) as u16;
    if visible_rows == 0 {
        return None;
    }
    let mut burrow_sprite = Vec::new();
    for row in 0..visible_rows {
        burrow_sprite.extend(sprite.iter().map(|cell| SpriteCell {
            row: cell.row - row as i16,
            col: cell.col,
            glyph: cell.glyph,
        }));
    }
    let row = lower_lane_row(input.geometry);
    let col = oscillating_col(input, id.as_str(), row, &burrow_sprite, 5);
    build_placement(id, input, &burrow_sprite, col, row, style, |_| {
        HabitatPetLayer::Foreground
    })
}

fn host_combo_placement(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    anchor_sprite: &[SpriteCell],
    style: Style,
) -> Option<TankLifePlacement> {
    let habitat = input.geometry.habitat;
    let anchor_width = sprite_width(anchor_sprite).max(1);
    let anchor_height = sprite_height(anchor_sprite).max(1);
    let anchor_col = habitat
        .x
        .saturating_add(habitat.width.saturating_sub(anchor_width) / 2);
    let anchor_row = lower_lane_row(input.geometry)
        .saturating_sub(anchor_height.saturating_sub(1))
        .saturating_sub(1);
    let mut cells = placement_cells(
        id,
        input,
        anchor_sprite,
        anchor_col,
        anchor_row,
        style,
        |_| HabitatPetLayer::Behind,
    );

    let host_sprite = host_fish_sprite();
    let host_on_right = route_phase(id.as_str(), input).is_multiple_of(2);
    let host_col = if host_on_right {
        anchor_col.saturating_add(anchor_width).saturating_add(1)
    } else {
        anchor_col.saturating_sub(3)
    };
    let host_row = anchor_row.saturating_sub(1);
    cells.extend(placement_cells(
        id,
        input,
        &host_sprite,
        host_col,
        host_row,
        style,
        |_| HabitatPetLayer::Foreground,
    ));
    placement_from_cells(id, cells)
}

fn build_placement<F>(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    sprite: &[SpriteCell],
    base_col: u16,
    base_row: u16,
    style: Style,
    layer_for: F,
) -> Option<TankLifePlacement>
where
    F: Fn(SpriteCell) -> HabitatPetLayer,
{
    let cells = placement_cells(id, input, sprite, base_col, base_row, style, layer_for);
    placement_from_cells(id, cells)
}

fn placement_cells<F>(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
    sprite: &[SpriteCell],
    base_col: u16,
    base_row: u16,
    style: Style,
    layer_for: F,
) -> Vec<TankLifeCell>
where
    F: Fn(SpriteCell) -> HabitatPetLayer,
{
    sprite
        .iter()
        .filter_map(|cell| {
            let col = i32::from(base_col) + i32::from(cell.col);
            let row = i32::from(base_row) + i32::from(cell.row);
            if col < 0 || row < 0 {
                return None;
            }
            let col = u16::try_from(col).ok()?;
            let row = u16::try_from(row).ok()?;
            let pet_layer = layer_for(*cell);
            if !cell_allowed(input, col, row, pet_layer) {
                return None;
            }
            Some(TankLifeCell {
                inhabitant_id: id.clone(),
                row,
                col,
                glyph: cell.glyph,
                style,
                pet_layer,
            })
        })
        .collect()
}

fn placement_from_cells(
    id: &TankInhabitantId,
    cells: Vec<TankLifeCell>,
) -> Option<TankLifePlacement> {
    if cells.is_empty() {
        return None;
    }
    let min_col = cells.iter().map(|cell| cell.col).min()?;
    let max_col = cells.iter().map(|cell| cell.col).max()?;
    let min_row = cells.iter().map(|cell| cell.row).min()?;
    let max_row = cells.iter().map(|cell| cell.row).max()?;
    Some(TankLifePlacement {
        inhabitant_id: id.clone(),
        bounds: Rect::new(
            min_col,
            min_row,
            max_col.saturating_sub(min_col).saturating_add(1),
            max_row.saturating_sub(min_row).saturating_add(1),
        ),
        cells,
    })
}

fn cell_allowed(
    input: &TankLifeRenderInput<'_>,
    col: u16,
    row: u16,
    layer: HabitatPetLayer,
) -> bool {
    rect_contains(input.geometry.habitat, col, row)
        && input.geometry.cell_inside_aperture(col, row)
        && !input
            .geometry
            .reserved_regions
            .iter()
            .any(|region| rect_contains(*region, col, row))
        && !(layer == HabitatPetLayer::Foreground
            && input
                .pet_protected_regions
                .iter()
                .any(|region| rect_contains(*region, col, row)))
}

fn lower_lane_row(geometry: &TankLifeSurfaceGeometry) -> u16 {
    let floor_gap = if geometry.literal_floor_allowed { 1 } else { 4 };
    geometry
        .habitat
        .y
        .saturating_add(geometry.habitat.height.saturating_sub(floor_gap + 1))
        .max(geometry.habitat.y)
}

fn oscillating_col(
    input: &TankLifeRenderInput<'_>,
    id: &str,
    row: u16,
    sprite: &[SpriteCell],
    padding: u16,
) -> u16 {
    let (start, end) = oscillating_bounds(input, row, sprite, padding);
    oscillating_col_between(input, id, start, end)
}

fn oscillating_bounds(
    input: &TankLifeRenderInput<'_>,
    row: u16,
    sprite: &[SpriteCell],
    padding: u16,
) -> (u16, u16) {
    let habitat = input.geometry.habitat;
    let sprite_width = sprite_width(sprite).max(1);
    let mut start = habitat.x.saturating_add(padding);
    let mut end = habitat
        .x
        .saturating_add(habitat.width.saturating_sub(sprite_width))
        .saturating_sub(padding);

    while start < end && !origin_cells_inside_aperture(input.geometry, sprite, start, row) {
        start = start.saturating_add(1);
    }
    while end > start && !origin_cells_inside_aperture(input.geometry, sprite, end, row) {
        end = end.saturating_sub(1);
    }

    (start, end)
}

fn oscillating_col_between(input: &TankLifeRenderInput<'_>, id: &str, start: u16, end: u16) -> u16 {
    let span = end.saturating_sub(start);
    if span == 0 {
        return start;
    }
    let period = u64::from(span) * 2;
    let step = route_phase(id, input) % period;
    if step <= u64::from(span) {
        start.saturating_add(step as u16)
    } else {
        end.saturating_sub((step - u64::from(span)) as u16)
    }
}

fn cross_tank_layer_for_col(start: u16, end: u16, col: u16) -> HabitatPetLayer {
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

fn origin_cells_inside_aperture(
    geometry: &TankLifeSurfaceGeometry,
    sprite: &[SpriteCell],
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
                .is_some_and(|(col, row)| geometry.cell_inside_aperture(col, row))
    })
}

fn sprite_width(sprite: &[SpriteCell]) -> u16 {
    sprite
        .iter()
        .filter_map(|cell| u16::try_from(i32::from(cell.col) + 1).ok())
        .max()
        .unwrap_or(0)
}

fn sprite_height(sprite: &[SpriteCell]) -> u16 {
    sprite
        .iter()
        .filter_map(|cell| u16::try_from(i32::from(cell.row) + 1).ok())
        .max()
        .unwrap_or(0)
}

fn route_phase(id: &str, input: &TankLifeRenderInput<'_>) -> u64 {
    let timing_scalar = if input.life_profile.calm_mode { 2 } else { 1 };
    let tick = input.now.unix_timestamp().max(0) as u64 / (4 * timing_scalar);
    stable_hash(&format!(
        "tank-life-route-v1|{}|{}|{}",
        input.pet_seed, input.local_date, id
    ))
    .wrapping_add(tick)
}

fn tank_life_style(
    low_color_key: &str,
    color_capability: crate::tui::style::ColorCapability,
) -> Style {
    if matches!(color_capability, crate::tui::style::ColorCapability::Flat) {
        return Style::default();
    }
    let color = match low_color_key {
        "shrimp" => Color::Rgb(0xff, 0xc4, 0x92),
        "fish" | "school" => Color::Rgb(0x7e, 0xee, 0xff),
        "snail" => Color::Rgb(0xd8, 0xc0, 0x90),
        "burrower" | "ray" => Color::Rgb(0xc8, 0xb0, 0x88),
        "rim" => Color::Rgb(0xb8, 0xd8, 0xf0),
        "host" => Color::Rgb(0xe8, 0xb0, 0xd0),
        _ => crate::tui::style::tokenpet_palette().dim.rgb,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn stable_hash(input: &str) -> u64 {
    const OFFSET: u64 = 1_469_598_103_934_665_603;
    const PRIME: u64 = 1_099_511_628_211;

    input.bytes().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
        hash
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::state::TankInhabitantId;
    use crate::tui::view_model::EarnedTankInhabitantView;
    use time::macros::date;

    const SEED: &str = "glorp-tank-life-fixture";

    fn earned(age_days: i64) -> Vec<EarnedTankInhabitantView> {
        crate::game::habitat::TANK_INHABITANT_CATALOG
            .iter()
            .filter(|spec| spec.unlock_age_days <= age_days)
            .map(|spec| EarnedTankInhabitantView {
                id: TankInhabitantId::new(spec.id),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                unlock_age_days: spec.unlock_age_days,
                kind: spec.kind,
                source: crate::storage::state::TankInhabitantSource::PetAgeThreshold {
                    threshold_days: spec.unlock_age_days,
                },
            })
            .collect()
    }

    fn ids(ids: &[TankInhabitantId]) -> Vec<&str> {
        ids.iter().map(|id| id.as_str()).collect()
    }

    impl<'a> TankLifeRenderInput<'a> {
        fn for_test(
            rendered_ids: Vec<TankInhabitantId>,
            geometry: &'a TankLifeSurfaceGeometry,
            local_date: time::Date,
            elapsed_seconds: i64,
        ) -> Self {
            Self {
                rendered_ids,
                pet_seed: "glorp-tank-life-fixture",
                local_date,
                now: time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(elapsed_seconds),
                geometry,
                pet_protected_regions: &[],
                color_capability: crate::tui::style::ColorCapability::Truecolor,
                life_profile: crate::tui::life::PetLifeProfile::default(),
            }
        }
    }

    #[test]
    fn canonical_cast_has_exact_age_and_date_fixtures() {
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(0),
                SEED,
                date!(2026 - 07 - 07),
                0
            )),
            Vec::<&str>::new()
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(1),
                SEED,
                date!(2026 - 07 - 07),
                1
            )),
            vec!["glass_shrimp"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(3),
                SEED,
                date!(2026 - 07 - 07),
                3
            )),
            vec!["glass_shrimp", "needlefish"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(7),
                SEED,
                date!(2026 - 07 - 07),
                7
            )),
            vec!["glass_snail", "glass_shrimp", "needlefish"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(21),
                SEED,
                date!(2026 - 07 - 07),
                21
            )),
            vec!["rim_skimmer", "burrower", "glass_snail", "sand_ray"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(60),
                SEED,
                date!(2026 - 07 - 07),
                60
            )),
            vec!["anemone_host", "rim_skimmer", "burrower", "glass_snail"]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(60),
                SEED,
                date!(2026 - 07 - 08),
                60
            )),
            vec![
                "rim_skimmer",
                "glass_shrimp",
                "sand_ray",
                "burrower",
                "anemone_host"
            ]
        );
        assert_eq!(
            ids(&canonical_daily_cast(
                &earned(60),
                SEED,
                date!(2026 - 07 - 09),
                60
            )),
            vec!["sand_ray", "schoollet", "glass_snail", "glass_shrimp"]
        );
    }

    #[test]
    fn anemone_morph_has_exact_date_fixtures() {
        assert_eq!(
            anemone_morph_for_day(SEED, date!(2026 - 07 - 07)),
            AnemoneMorph::DotColony
        );
        assert_eq!(
            anemone_morph_for_day(SEED, date!(2026 - 07 - 08)),
            AnemoneMorph::Crown
        );
        assert_eq!(
            anemone_morph_for_day(SEED, date!(2026 - 07 - 09)),
            AnemoneMorph::Comb
        );
    }

    #[test]
    fn round_projection_caps_without_rerandomizing() {
        let canonical = canonical_daily_cast(&earned(60), SEED, date!(2026 - 07 - 08), 60);
        let geometry = TankLifeSurfaceGeometry::round_for_test(44, 18, 2);

        let projected = project_tank_life_cast(&canonical, &geometry);

        assert_eq!(
            ids(&projected.canonical_ids),
            vec![
                "rim_skimmer",
                "glass_shrimp",
                "sand_ray",
                "burrower",
                "anemone_host"
            ]
        );
        assert_eq!(
            ids(&projected.rendered_ids),
            vec!["rim_skimmer", "glass_shrimp"]
        );
        assert_eq!(projected.skipped.len(), 3);
        assert!(projected
            .skipped
            .iter()
            .all(|skip| skip.reason == TankLifeSkipReason::SurfaceBudget));
    }

    #[test]
    fn catalog_glyphs_are_single_width_cells() {
        validate_tank_life_catalog().unwrap();
    }

    #[test]
    fn anemone_host_morphs_share_host_behavior_but_unique_anchor_cells() {
        let morphs = [
            AnemoneMorph::Flower,
            AnemoneMorph::Comb,
            AnemoneMorph::Crown,
            AnemoneMorph::DotColony,
        ];
        let anchors = morphs
            .into_iter()
            .map(anemone_anchor_sprite)
            .collect::<Vec<_>>();

        assert!(anchors.windows(2).all(|pair| pair[0] != pair[1]));
        assert_eq!(
            host_fish_sprite(),
            vec![
                SpriteCell { row: 0, col: 0, glyph: '›' },
                SpriteCell { row: 0, col: 1, glyph: '·' },
            ]
        );
    }

    #[test]
    fn route_dependent_swimmer_changes_whole_sprite_depth_over_route() {
        let geometry = TankLifeSurfaceGeometry::round_for_test(44, 18, 3);
        let mut layers = Vec::new();

        for elapsed_seconds in [0, 900, 1_800, 2_700, 3_600] {
            let placements = tank_life_placements_for(&TankLifeRenderInput::for_test(
                vec![TankInhabitantId::new(crate::game::habitat::NEEDLEFISH)],
                &geometry,
                time::macros::date!(2026 - 07 - 08),
                elapsed_seconds,
            ));
            let placement = placements.first().expect("needlefish should be visible");
            let layer = placement.cells[0].pet_layer;
            assert!(
                placement.cells.iter().all(|cell| cell.pet_layer == layer),
                "a swimmer should occupy one depth layer per route segment"
            );
            layers.push(layer);
        }

        assert!(layers.contains(&crate::game::habitat::HabitatPetLayer::Behind));
        assert!(layers.contains(&crate::game::habitat::HabitatPetLayer::Foreground));
    }

    #[test]
    fn round_routes_avoid_reserved_regions() {
        let geometry = TankLifeSurfaceGeometry::round_for_test(44, 18, 3);
        let input = TankLifeRenderInput::for_test(
            vec![
                TankInhabitantId::new(crate::game::habitat::GLASS_SHRIMP),
                TankInhabitantId::new(crate::game::habitat::RIM_SKIMMER),
                TankInhabitantId::new(crate::game::habitat::ANEMONE_HOST),
            ],
            &geometry,
            time::macros::date!(2026 - 07 - 08),
            2_400,
        );

        for placement in tank_life_placements_for(&input) {
            for cell in placement.cells {
                assert!(!input
                    .geometry
                    .reserved_regions
                    .iter()
                    .any(|region| rect_contains(*region, cell.col, cell.row)));
                assert!(input.geometry.cell_inside_aperture(cell.col, cell.row));
            }
        }
    }

    #[test]
    fn round_routes_advance_within_a_few_seconds() {
        let geometry = TankLifeSurfaceGeometry::round_for_test(52, 52, 2);
        let rendered_ids = vec![TankInhabitantId::new(crate::game::habitat::NEEDLEFISH)];
        let first = tank_life_placements_for(&TankLifeRenderInput::for_test(
            rendered_ids.clone(),
            &geometry,
            time::macros::date!(2026 - 07 - 07),
            0,
        ));
        let later = tank_life_placements_for(&TankLifeRenderInput::for_test(
            rendered_ids,
            &geometry,
            time::macros::date!(2026 - 07 - 07),
            4,
        ));

        assert_ne!(
            first[0].bounds, later[0].bounds,
            "round inhabitants should visibly move within a few seconds, not sit for a 30s tick"
        );
    }

    #[test]
    fn first_round_inhabitants_use_legible_multi_cell_silhouettes() {
        let geometry = TankLifeSurfaceGeometry::round_for_test(52, 52, 2);
        let placements = tank_life_placements_for(&TankLifeRenderInput::for_test(
            vec![
                TankInhabitantId::new(crate::game::habitat::GLASS_SHRIMP),
                TankInhabitantId::new(crate::game::habitat::NEEDLEFISH),
            ],
            &geometry,
            time::macros::date!(2026 - 07 - 07),
            0,
        ));

        for placement in placements {
            assert!(
                placement.cells.len() >= 3,
                "{} should render as a readable creature silhouette, not punctuation",
                placement.inhabitant_id.as_str()
            );
        }
    }
}
