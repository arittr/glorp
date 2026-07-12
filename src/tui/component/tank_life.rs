use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
};

use crate::game::habitat::HabitatPetLayer;
pub use crate::presentation::habitat_inventory::canonical_target_count;
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
    pub origin_col: u16,
    pub origin_row: u16,
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
    pub asleep: bool,
}

pub fn canonical_daily_cast(
    unlocked: &[crate::tui::view_model::EarnedTankInhabitantView],
    pet_seed: &str,
    local_date: time::Date,
    calendar_age_days: i64,
) -> Vec<TankInhabitantId> {
    let ids = unlocked
        .iter()
        .map(|earned| earned.id.clone())
        .collect::<Vec<_>>();
    crate::presentation::habitat_inventory::canonical_daily_cast(
        &ids,
        pet_seed,
        local_date,
        calendar_age_days,
    )
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
    let region = crate::presentation::tank_life::pet_face_reserved_region(
        crate::presentation::tank_life::TankRouteRect {
            x: pet_art.x,
            y: pet_art.y,
            width: pet_art.width,
            height: pet_art.height,
        },
    );
    vec![Rect::new(region.x, region.y, region.width, region.height)]
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
    let morph = match morph {
        AnemoneMorph::Flower => 0,
        AnemoneMorph::Comb => 1,
        AnemoneMorph::Crown => 2,
        AnemoneMorph::DotColony => 3,
    };
    route_sprite_cells(crate::presentation::tank_life::tank_sprite_cells(
        crate::game::habitat::ANEMONE_HOST,
        0,
        Some(morph),
    ))
}

pub fn host_fish_sprite() -> Vec<SpriteCell> {
    route_sprite_cells(crate::presentation::tank_life::tank_host_fish_sprite())
}

fn route_sprite_cells(
    cells: Vec<crate::presentation::tank_life::TankRouteSpriteCell>,
) -> Vec<SpriteCell> {
    cells
        .into_iter()
        .map(|cell| SpriteCell {
            row: cell.row,
            col: cell.col,
            glyph: cell.glyph,
        })
        .collect()
}

/// Every glyph the earnable tank-life cast can render, driving the real
/// `sprite_for` across the whole inhabitant catalog, both animation phases, and
/// every anemone morph, plus the anemone's host fish. Declared content for the
/// retained atlas preflight.
pub(crate) fn declared_tank_life_glyphs() -> std::collections::BTreeSet<char> {
    let morphs = [
        None,
        Some(AnemoneMorph::Flower),
        Some(AnemoneMorph::Comb),
        Some(AnemoneMorph::Crown),
        Some(AnemoneMorph::DotColony),
    ];
    let mut glyphs = std::collections::BTreeSet::new();
    for spec in crate::game::habitat::TANK_INHABITANT_CATALOG {
        for phase in [0_u64, 1] {
            for morph in morphs {
                for cell in sprite_for(spec.id, phase, morph) {
                    glyphs.insert(cell.glyph);
                }
            }
        }
    }
    for cell in host_fish_sprite() {
        glyphs.insert(cell.glyph);
    }
    glyphs.remove(&' ');
    glyphs
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
    route_sprite_cells(crate::presentation::tank_life::tank_sprite_cells(
        id,
        (phase & 1) as u8,
        morph.map(|morph| match morph {
            AnemoneMorph::Flower => 0,
            AnemoneMorph::Comb => 1,
            AnemoneMorph::Crown => 2,
            AnemoneMorph::DotColony => 3,
        }),
    ))
}

fn placement_for_id(
    id: &TankInhabitantId,
    input: &TankLifeRenderInput<'_>,
) -> Option<TankLifePlacement> {
    let spec = crate::game::habitat::tank_inhabitant_spec(id)?;
    let route_geometry = neutral_route_geometry(input);
    let outcome = crate::presentation::tank_life::resolve_tank_route(
        crate::presentation::tank_life::TankRouteInput {
            catalog_id: id.as_str(),
            pet_seed: input.pet_seed,
            local_date: input.local_date,
            now: input.now,
            calm: input.life_profile.calm_mode || input.asleep,
            geometry: &route_geometry,
        },
    )?;
    if !outcome.visible {
        return None;
    }
    let style = tank_life_style(spec.low_color_key, input.color_capability);
    let bounds = outcome.bounds?;
    Some(TankLifePlacement {
        inhabitant_id: id.clone(),
        origin_col: outcome.origin_col,
        origin_row: outcome.origin_row,
        cells: outcome
            .cells
            .into_iter()
            .map(|cell| TankLifeCell {
                inhabitant_id: id.clone(),
                row: cell.row,
                col: cell.col,
                glyph: cell.glyph,
                style,
                pet_layer: route_layer(cell.layer),
            })
            .collect(),
        bounds: Rect::new(bounds.x, bounds.y, bounds.width, bounds.height),
    })
}

fn neutral_route_geometry(
    input: &TankLifeRenderInput<'_>,
) -> crate::presentation::tank_life::TankRouteGeometry {
    let geometry = input.geometry;
    crate::presentation::tank_life::TankRouteGeometry {
        habitat: crate::presentation::tank_life::TankRouteRect {
            x: geometry.habitat.x,
            y: geometry.habitat.y,
            width: geometry.habitat.width,
            height: geometry.habitat.height,
        },
        aperture: geometry.aperture_mask.map(|mask| {
            crate::presentation::tank_life::TankRouteAperture {
                center_col: mask.center_col,
                center_row: mask.center_row,
                radius_cols: mask.radius_cols,
                radius_rows: mask.radius_rows,
            }
        }),
        reserved_regions: geometry
            .reserved_regions
            .iter()
            .map(|rect| crate::presentation::tank_life::TankRouteRect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            })
            .collect(),
        foreground_reserved_regions: input
            .pet_protected_regions
            .iter()
            .map(|rect| crate::presentation::tank_life::TankRouteRect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            })
            .collect(),
        literal_floor_allowed: geometry.literal_floor_allowed,
    }
}

fn route_layer(layer: crate::presentation::tank_life::TankRouteLayer) -> HabitatPetLayer {
    match layer {
        crate::presentation::tank_life::TankRouteLayer::Behind => HabitatPetLayer::Behind,
        crate::presentation::tank_life::TankRouteLayer::Foreground => HabitatPetLayer::Foreground,
        crate::presentation::tank_life::TankRouteLayer::BehindAnchorForegroundHost => {
            unreachable!("combined route layer is never assigned to a resolved cell")
        }
    }
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
                asleep: false,
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
    fn shared_route_outcomes_exactly_match_round_tui_geometry_and_cadence() {
        let mut saw_hud_reserve_effect = false;
        let mut saw_aperture_effect = false;
        let mut saw_foreground_pet_reserve_clip = false;
        let mut saw_behind_pet_reserve_survival = false;
        for (width, height) in [(20, 12), (44, 18), (72, 24)] {
            let geometry = crate::round::scene::round_tank_life_geometry(width, height);
            let pet_rect = Rect::new(
                width.saturating_sub(13) / 2,
                height.saturating_sub(10) / 2,
                13.min(width),
                10.min(height),
            );
            let protected = crate::round::scene::round_tank_life_protected_regions_for_test(
                pet_rect, width, height,
            );
            assert_eq!(
                geometry.reserved_regions, protected.bottom_hud,
                "round geometry should reserve only the bottom HUD band"
            );
            for elapsed_seconds in [0, 4, 8, 32] {
                for (calm, asleep) in [(false, false), (true, false), (false, true)] {
                    for spec in crate::game::habitat::TANK_INHABITANT_CATALOG {
                        let mut input = TankLifeRenderInput::for_test(
                            vec![TankInhabitantId::new(spec.id)],
                            &geometry,
                            time::macros::date!(2026 - 07 - 08),
                            elapsed_seconds,
                        );
                        input.life_profile.calm_mode = calm;
                        input.asleep = asleep;
                        input.pet_protected_regions = &protected.pet_face;
                        let neutral_geometry = neutral_route_geometry(&input);
                        let outcome = crate::presentation::tank_life::resolve_tank_route(
                            crate::presentation::tank_life::TankRouteInput {
                                catalog_id: spec.id,
                                pet_seed: input.pet_seed,
                                local_date: input.local_date,
                                now: input.now,
                                calm: calm || asleep,
                                geometry: &neutral_geometry,
                            },
                        )
                        .expect("known route outcome");
                        let mut geometry_without_hud_reserve = neutral_geometry.clone();
                        geometry_without_hud_reserve.reserved_regions.clear();
                        let outcome_without_hud_reserve =
                            crate::presentation::tank_life::resolve_tank_route(
                                crate::presentation::tank_life::TankRouteInput {
                                    catalog_id: spec.id,
                                    pet_seed: input.pet_seed,
                                    local_date: input.local_date,
                                    now: input.now,
                                    calm: calm || asleep,
                                    geometry: &geometry_without_hud_reserve,
                                },
                            )
                            .expect("known route outcome without HUD reserve");
                        let mut geometry_without_aperture = neutral_geometry.clone();
                        geometry_without_aperture.aperture = None;
                        let outcome_without_aperture =
                            crate::presentation::tank_life::resolve_tank_route(
                                crate::presentation::tank_life::TankRouteInput {
                                    catalog_id: spec.id,
                                    pet_seed: input.pet_seed,
                                    local_date: input.local_date,
                                    now: input.now,
                                    calm: calm || asleep,
                                    geometry: &geometry_without_aperture,
                                },
                            )
                            .expect("known route outcome without aperture");
                        let mut geometry_without_pet_reserve = neutral_geometry.clone();
                        geometry_without_pet_reserve
                            .foreground_reserved_regions
                            .clear();
                        let outcome_without_pet_reserve =
                            crate::presentation::tank_life::resolve_tank_route(
                                crate::presentation::tank_life::TankRouteInput {
                                    catalog_id: spec.id,
                                    pet_seed: input.pet_seed,
                                    local_date: input.local_date,
                                    now: input.now,
                                    calm: calm || asleep,
                                    geometry: &geometry_without_pet_reserve,
                                },
                            )
                            .expect("known unprotected route outcome");
                        let placements = tank_life_placements_for(&input);

                        assert_eq!(outcome.calm, calm || asleep);
                        assert_eq!(
                            outcome.cadence_ms,
                            if calm || asleep { 8_000 } else { 4_000 }
                        );
                        assert_eq!(placements.is_empty(), !outcome.visible, "{}", spec.id);
                        if let Some(placement) = placements.first() {
                            assert_eq!(
                                (placement.origin_col, placement.origin_row),
                                (outcome.origin_col, outcome.origin_row),
                                "{} origin diverged",
                                spec.id
                            );
                            let expected_bounds = outcome.bounds.expect("visible outcome bounds");
                            assert_eq!(
                                placement.bounds,
                                Rect::new(
                                    expected_bounds.x,
                                    expected_bounds.y,
                                    expected_bounds.width,
                                    expected_bounds.height,
                                ),
                                "{} bounds diverged",
                                spec.id
                            );
                            let actual_cells = placement
                                .cells
                                .iter()
                                .map(|cell| (cell.col, cell.row, cell.glyph, cell.pet_layer))
                                .collect::<Vec<_>>();
                            let expected_cells = outcome
                                .cells
                                .iter()
                                .map(|cell| {
                                    (
                                        cell.col,
                                        cell.row,
                                        cell.glyph,
                                        match cell.layer {
                                            crate::presentation::tank_life::TankRouteLayer::Behind => HabitatPetLayer::Behind,
                                            crate::presentation::tank_life::TankRouteLayer::Foreground => HabitatPetLayer::Foreground,
                                            crate::presentation::tank_life::TankRouteLayer::BehindAnchorForegroundHost => unreachable!("combined layer is not a cell layer"),
                                        },
                                    )
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(actual_cells, expected_cells, "{} cells diverged", spec.id);
                        } else {
                            assert!(outcome.bounds.is_none(), "{} invisible bounds", spec.id);
                            assert!(outcome.cells.is_empty(), "{} invisible cells", spec.id);
                        }

                        for cell in &outcome_without_pet_reserve.cells {
                            let inside_pet_reserve = protected
                                .pet_face
                                .iter()
                                .any(|region| rect_contains(*region, cell.col, cell.row));
                            let preserved = outcome.cells.contains(cell);
                            if cell.layer
                                == crate::presentation::tank_life::TankRouteLayer::Foreground
                                && inside_pet_reserve
                            {
                                assert!(
                                    !preserved,
                                    "{} foreground cell survived pet-face reserve",
                                    spec.id
                                );
                                saw_foreground_pet_reserve_clip = true;
                            } else {
                                assert!(
                                    preserved,
                                    "{} cell outside foreground pet-face clipping diverged",
                                    spec.id
                                );
                                saw_behind_pet_reserve_survival |= inside_pet_reserve
                                    && cell.layer
                                        == crate::presentation::tank_life::TankRouteLayer::Behind;
                            }
                        }

                        saw_hud_reserve_effect |= outcome.visible
                            != outcome_without_hud_reserve.visible
                            || outcome.cells != outcome_without_hud_reserve.cells;
                        saw_aperture_effect |= outcome.visible != outcome_without_aperture.visible
                            || outcome.cells != outcome_without_aperture.cells;
                    }

                    let catalog = crate::game::habitat::TANK_INHABITANT_CATALOG
                        .iter()
                        .map(|spec| TankInhabitantId::new(spec.id))
                        .collect::<Vec<_>>();
                    let mut reversed = catalog.clone();
                    reversed.reverse();
                    let mut forward_input = TankLifeRenderInput::for_test(
                        catalog,
                        &geometry,
                        time::macros::date!(2026 - 07 - 08),
                        elapsed_seconds,
                    );
                    forward_input.life_profile.calm_mode = calm;
                    forward_input.asleep = asleep;
                    forward_input.pet_protected_regions = &protected.pet_face;
                    let mut reversed_input = TankLifeRenderInput::for_test(
                        reversed,
                        &geometry,
                        time::macros::date!(2026 - 07 - 08),
                        elapsed_seconds,
                    );
                    reversed_input.life_profile.calm_mode = calm;
                    reversed_input.asleep = asleep;
                    reversed_input.pet_protected_regions = &protected.pet_face;
                    let forward = tank_life_placements_for(&forward_input);
                    let reversed = tank_life_placements_for(&reversed_input);
                    assert_eq!(forward.len(), reversed.len());
                    for placement in forward {
                        assert_eq!(
                            reversed.iter().find(
                                |candidate| candidate.inhabitant_id == placement.inhabitant_id
                            ),
                            Some(&placement),
                            "{} changed under input reorder",
                            placement.inhabitant_id.as_str()
                        );
                    }
                }
            }
        }
        assert!(
            saw_hud_reserve_effect,
            "matrix did not prove a differential HUD reserve effect"
        );
        assert!(
            saw_aperture_effect,
            "matrix did not prove a differential aperture effect"
        );
        assert!(
            saw_foreground_pet_reserve_clip,
            "matrix did not exercise foreground pet-face clipping"
        );
        assert!(
            saw_behind_pet_reserve_survival,
            "matrix did not exercise behind-layer survival through the pet-face reserve"
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
