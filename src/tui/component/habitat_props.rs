use crate::game::habitat::{
    catalog_prop_by_str, HabitatPetLayer, HabitatPropKind, HabitatPropZone, CODEX_SIGNAL_LAMP,
    HEAVY_SESSION_PLANTER, TOKEN_FRIENDLY_CLOUD_750K, TOKEN_HANGING_VINE_25M, TOKEN_LANTERN_10M,
    TOKEN_MOSS_TUFT_250K, TOKEN_ORBIT_5M, TOKEN_PEBBLE_25K, TOKEN_SHARD_1M, TOKEN_SHELL_100K,
    TOKEN_SPARK_500K, TOKEN_TREASURE_CHEST_2M, WILT_RECOVERY_SPROUT,
};
use crate::pet::generation::Species;
use crate::storage::state::HabitatPropId;
use crate::tui::component::{PetSceneLayout, TargetPath};
use crate::tui::render_context::RenderContext;
use crate::tui::style::{tokenpet_palette, ColorCapability};
use crate::tui::view_model::HabitatView;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use std::collections::HashMap;

const MAX_TROPHIES: usize = 6;
const MAX_ACCENTS: usize = 4;
const ACCENT_ROTATION_SECS: i64 = 600;
const ACCENT_CANDIDATES: u16 = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct HabitatPropCell {
    pub prop_id: HabitatPropId,
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
    pub pet_layer: HabitatPetLayer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HabitatPropPlacement {
    pub prop_id: HabitatPropId,
    pub cells: Vec<HabitatPropCell>,
    pub bounds: Rect,
    pub pet_layer: HabitatPetLayer,
    pub target_id: Option<TargetPath>,
}

#[derive(Clone, Copy)]
struct SpriteCell {
    dx: i16,
    dy: i16,
    glyph: char,
}

#[derive(Clone, Copy)]
struct SpriteFootprint {
    min_dx: i16,
    max_dx: i16,
    min_dy: i16,
    max_dy: i16,
}

impl SpriteFootprint {
    fn width(self) -> u16 {
        (self.max_dx - self.min_dx + 1).max(1) as u16
    }

    fn height(self) -> u16 {
        (self.max_dy - self.min_dy + 1).max(1) as u16
    }
}

pub fn habitat_props_for(
    habitat: &HabitatView,
    scene: &PetSceneLayout,
    silhouette_halo: &[Rect],
    species: Species,
    seed: &str,
    ctx: &RenderContext,
) -> Vec<HabitatPropCell> {
    habitat_prop_placements_for(habitat, scene, silhouette_halo, species, seed, ctx)
        .into_iter()
        .flat_map(|placement| placement.cells)
        .collect()
}

pub fn habitat_prop_placements_for(
    habitat: &HabitatView,
    scene: &PetSceneLayout,
    silhouette_halo: &[Rect],
    species: Species,
    seed: &str,
    ctx: &RenderContext,
) -> Vec<HabitatPropPlacement> {
    let now = ctx.clock.now_utc();
    let mut occupied = scene.exclusions.clone();
    let mut placements = Vec::new();

    for id in visible_trophy_ids(habitat) {
        let layer = prop_pet_layer(id);
        let sprite = trophy_sprite(id, species, now);
        let exclusions = exclusions_for_layer(layer, scene, &occupied, silhouette_halo);
        for anchor in trophy_anchor_candidates(id, scene.habitat, sprite) {
            let rendered = render_sprite(
                anchor,
                sprite,
                scene.habitat,
                &exclusions,
                trophy_style(ctx.color_capability, id),
                layer,
                id,
            );
            if !rendered.is_empty() {
                let bounds = bounds_for_cells(&rendered);
                occupied.push(bounds);
                placements.push(HabitatPropPlacement {
                    prop_id: HabitatPropId::new(id),
                    cells: rendered,
                    bounds,
                    pet_layer: layer,
                    target_id: prop_effect_target_path(id),
                });
                break;
            }
        }
    }

    if matches!(ctx.color_capability, ColorCapability::Truecolor) {
        let accent_cells =
            stable_accent_cells_by_id(habitat, scene, &occupied, silhouette_halo, seed, now);
        for id in visible_accent_ids(habitat, now) {
            if let Some(cell) = accent_cells.get(id) {
                let layer = prop_pet_layer(id);
                placements.push(HabitatPropPlacement {
                    prop_id: HabitatPropId::new(id),
                    cells: vec![cell.clone()],
                    bounds: Rect::new(cell.col, cell.row, 1, 1),
                    pet_layer: layer,
                    target_id: prop_effect_target_path(id),
                });
            }
        }
    }

    placements
}

pub fn prop_effect_target_path(id: &str) -> Option<TargetPath> {
    match id {
        TOKEN_PEBBLE_25K => Some(TargetPath::new("watch.prop.token_pebble_25k.effect")),
        TOKEN_SHELL_100K => Some(TargetPath::new("watch.prop.token_shell_100k.effect")),
        TOKEN_MOSS_TUFT_250K => Some(TargetPath::new("watch.prop.token_moss_tuft_250k.effect")),
        TOKEN_SPARK_500K => Some(TargetPath::new("watch.prop.token_spark_500k.effect")),
        TOKEN_FRIENDLY_CLOUD_750K => Some(TargetPath::new(
            "watch.prop.token_friendly_cloud_750k.effect",
        )),
        TOKEN_SHARD_1M => Some(TargetPath::new("watch.prop.token_shard_1m.effect")),
        TOKEN_TREASURE_CHEST_2M => {
            Some(TargetPath::new("watch.prop.token_treasure_chest_2m.effect"))
        }
        TOKEN_ORBIT_5M => Some(TargetPath::new("watch.prop.token_orbit_5m.effect")),
        TOKEN_LANTERN_10M => Some(TargetPath::new("watch.prop.token_lantern_10m.effect")),
        TOKEN_HANGING_VINE_25M => Some(TargetPath::new("watch.prop.token_hanging_vine_25m.effect")),
        CODEX_SIGNAL_LAMP => Some(TargetPath::new("watch.prop.codex_signal_lamp.effect")),
        HEAVY_SESSION_PLANTER => Some(TargetPath::new("watch.prop.heavy_session_planter.effect")),
        WILT_RECOVERY_SPROUT => Some(TargetPath::new("watch.prop.wilt_recovery_sprout.effect")),
        _ => None,
    }
}

fn prop_pet_layer(id: &str) -> HabitatPetLayer {
    catalog_prop_by_str(id)
        .map(|spec| spec.pet_layer)
        .unwrap_or(HabitatPetLayer::Background)
}

/// Drops the pet's bounding rect from the inherited `occupied` set and adds
/// the silhouette halo for layers that should avoid the pet. The 13×10
/// bounding rect is the rough box used for layout, but for placement we want
/// the precise per-cell silhouette + halo — replacing it gives Background
/// props access to the diamond's negative space while still respecting the
/// actual pet outline. Behind / Foreground props drop the rect and skip the
/// halo, so they can overlap with the pet's silhouette directly.
fn exclusions_for_layer(
    layer: HabitatPetLayer,
    scene: &PetSceneLayout,
    occupied: &[Rect],
    silhouette_halo: &[Rect],
) -> Vec<Rect> {
    let mut without_pet_rect: Vec<Rect> = occupied
        .iter()
        .copied()
        .filter(|rect| *rect != scene.pet_art)
        .collect();
    if matches!(layer, HabitatPetLayer::Background) {
        without_pet_rect.extend_from_slice(silhouette_halo);
    }
    without_pet_rect
}

pub(crate) fn visible_trophy_ids(habitat: &HabitatView) -> Vec<&str> {
    let mut props = habitat
        .earned_props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Trophy)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| {
        b.display_priority
            .cmp(&a.display_priority)
            .then_with(|| a.earned_at.cmp(&b.earned_at))
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });
    props
        .into_iter()
        .take(MAX_TROPHIES)
        .map(|prop| prop.id.as_str())
        .collect()
}

pub(crate) fn visible_accent_ids(habitat: &HabitatView, now: time::OffsetDateTime) -> Vec<&str> {
    let props = sorted_accent_ids(habitat);

    if props.len() <= MAX_ACCENTS {
        return props;
    }

    let start =
        (now.unix_timestamp() / ACCENT_ROTATION_SECS).rem_euclid(props.len() as i64) as usize;
    (0..MAX_ACCENTS)
        .map(|offset| props[(start + offset) % props.len()])
        .collect()
}

fn sorted_accent_ids(habitat: &HabitatView) -> Vec<&str> {
    let mut props = habitat
        .earned_props
        .iter()
        .filter(|prop| prop.kind == HabitatPropKind::Accent)
        .collect::<Vec<_>>();
    props.sort_by(|a, b| {
        a.earned_at
            .cmp(&b.earned_at)
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });

    props.into_iter().map(|prop| prop.id.as_str()).collect()
}

fn trophy_anchor_candidates(
    id: &str,
    habitat: Rect,
    sprite: &'static [SpriteCell],
) -> Vec<Position> {
    if habitat.width < 8 || habitat.height < 4 || sprite.is_empty() {
        return Vec::new();
    }

    let zone = catalog_prop_by_str(id)
        .map(|prop| prop.zone)
        .unwrap_or(HabitatPropZone::FloorLeft);
    let footprint = sprite_footprint(sprite);
    zone_anchor_candidates(zone, habitat, footprint)
}

fn sprite_footprint(sprite: &'static [SpriteCell]) -> SpriteFootprint {
    let mut min_dx = 0;
    let mut max_dx = 0;
    let mut min_dy = 0;
    let mut max_dy = 0;
    for cell in sprite {
        min_dx = min_dx.min(cell.dx);
        max_dx = max_dx.max(cell.dx);
        min_dy = min_dy.min(cell.dy);
        max_dy = max_dy.max(cell.dy);
    }
    SpriteFootprint {
        min_dx,
        max_dx,
        min_dy,
        max_dy,
    }
}

fn zone_anchor_candidates(
    zone: HabitatPropZone,
    habitat: Rect,
    footprint: SpriteFootprint,
) -> Vec<Position> {
    let width = footprint.width();
    let height = footprint.height();
    if width > habitat.width || height > habitat.height {
        return Vec::new();
    }

    let left = i32::from(habitat.x);
    let right = i32::from(habitat.x + habitat.width - width);
    let top = i32::from(habitat.y);
    let bottom = i32::from(habitat.y + habitat.height - height);
    let floor = (bottom - 1).max(top);
    let air = (top + 2).min(bottom);
    let ceiling = (top + 1).min(bottom);
    let middle_x = left + (right - left) / 2;
    let middle_y = top + (bottom - top) / 2;

    let raw = match zone {
        HabitatPropZone::FloorLeft => {
            vec![(left + 2, floor), (left + 4, floor), (left + 2, floor - 1)]
        }
        HabitatPropZone::FloorMid => vec![
            (middle_x, floor),
            (middle_x - 7, floor),
            (middle_x + 7, floor),
            (middle_x, floor - 1),
        ],
        HabitatPropZone::FloorRight => vec![
            (right - 5, floor),
            (right - 3, floor),
            (right - 7, floor - 1),
        ],
        HabitatPropZone::WallLeft => vec![
            (left + 2, middle_y),
            (left + 2, middle_y - 2),
            (left + 4, middle_y + 2),
        ],
        HabitatPropZone::WallRight => vec![
            (right - 2, middle_y),
            (right - 4, middle_y - 2),
            (right - 2, middle_y + 2),
        ],
        HabitatPropZone::AirLeft => vec![(left + 3, air), (left + 6, air + 2), (left + 2, air + 4)],
        HabitatPropZone::AirMid => vec![
            (middle_x, air),
            (middle_x - 6, air + 1),
            (middle_x + 6, air + 2),
            (middle_x, middle_y - 2),
        ],
        HabitatPropZone::AirRight => {
            vec![(right - 4, air), (right - 2, air + 2), (right - 7, air + 4)]
        }
        HabitatPropZone::Ceiling => vec![
            (middle_x, ceiling),
            (middle_x - 8, ceiling),
            (middle_x + 8, ceiling),
            (left + 3, ceiling),
            (right - 3, ceiling),
        ],
    };

    let mut anchors = Vec::new();
    for (candidate_left, candidate_top) in raw {
        push_anchor_candidate(
            &mut anchors,
            habitat,
            footprint,
            candidate_left,
            candidate_top,
        );
    }
    anchors
}

fn push_anchor_candidate(
    anchors: &mut Vec<Position>,
    habitat: Rect,
    footprint: SpriteFootprint,
    left: i32,
    top: i32,
) {
    let max_left = i32::from(habitat.x + habitat.width - footprint.width());
    let max_top = i32::from(habitat.y + habitat.height - footprint.height());
    let left = left.clamp(i32::from(habitat.x), max_left);
    let top = top.clamp(i32::from(habitat.y), max_top);
    let anchor_x = left - i32::from(footprint.min_dx);
    let anchor_y = top - i32::from(footprint.min_dy);
    if anchor_x < 0 || anchor_y < 0 {
        return;
    }
    let Ok(anchor_x) = u16::try_from(anchor_x) else {
        return;
    };
    let Ok(anchor_y) = u16::try_from(anchor_y) else {
        return;
    };
    let anchor = Position::new(anchor_x, anchor_y);
    if !anchors.contains(&anchor) {
        anchors.push(anchor);
    }
}

fn trophy_sprite(id: &str, _species: Species, now: time::OffsetDateTime) -> &'static [SpriteCell] {
    let phase = now.unix_timestamp().rem_euclid(8);
    match id {
        "token_moss_tuft_250k" if phase < 4 => &[
            SpriteCell {
                dx: 0,
                dy: 0,
                glyph: '╱',
            },
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '╿',
            },
            SpriteCell {
                dx: 2,
                dy: 0,
                glyph: '╲',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '▂',
            },
        ],
        "token_moss_tuft_250k" => &[
            SpriteCell {
                dx: 0,
                dy: 0,
                glyph: '╲',
            },
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '╿',
            },
            SpriteCell {
                dx: 2,
                dy: 0,
                glyph: '╱',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '▂',
            },
        ],
        "token_friendly_cloud_750k" if phase < 4 => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '☁',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '◦',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '◡',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '◦',
            },
        ],
        "token_friendly_cloud_750k" => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '☁',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '˙',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '◡',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '˙',
            },
        ],
        "token_treasure_chest_2m" if phase < 4 => &[
            SpriteCell {
                dx: 0,
                dy: 0,
                glyph: '╭',
            },
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '─',
            },
            SpriteCell {
                dx: 2,
                dy: 0,
                glyph: '╮',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '▣',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '◇',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '▣',
            },
        ],
        "token_treasure_chest_2m" => &[
            SpriteCell {
                dx: 0,
                dy: 0,
                glyph: '╭',
            },
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '─',
            },
            SpriteCell {
                dx: 2,
                dy: 0,
                glyph: '╮',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '▣',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '◆',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '▣',
            },
        ],
        "token_hanging_vine_25m" if phase < 4 => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '╽',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '┃',
            },
            SpriteCell {
                dx: 0,
                dy: 2,
                glyph: '╱',
            },
            SpriteCell {
                dx: 1,
                dy: 2,
                glyph: '┃',
            },
            SpriteCell {
                dx: 2,
                dy: 2,
                glyph: '╲',
            },
        ],
        "token_hanging_vine_25m" => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '╽',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '┃',
            },
            SpriteCell {
                dx: 0,
                dy: 2,
                glyph: '╲',
            },
            SpriteCell {
                dx: 1,
                dy: 2,
                glyph: '┃',
            },
            SpriteCell {
                dx: 2,
                dy: 2,
                glyph: '╱',
            },
        ],
        "codex_signal_lamp" if phase < 4 => &[
            SpriteCell {
                dx: 0,
                dy: 0,
                glyph: '╷',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '◉',
            },
            SpriteCell {
                dx: 0,
                dy: 2,
                glyph: '╵',
            },
        ],
        "codex_signal_lamp" => &[
            SpriteCell {
                dx: 0,
                dy: 0,
                glyph: '╷',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '○',
            },
            SpriteCell {
                dx: 0,
                dy: 2,
                glyph: '╵',
            },
        ],
        "heavy_session_planter" if phase < 4 => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: 'ѱ',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '╲',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '┃',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '╱',
            },
            SpriteCell {
                dx: 1,
                dy: 2,
                glyph: '◌',
            },
        ],
        "heavy_session_planter" => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: 'ѱ',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '╱',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '┃',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '╲',
            },
            SpriteCell {
                dx: 1,
                dy: 2,
                glyph: '◌',
            },
        ],
        "wilt_recovery_sprout" if phase < 4 => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '╿',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '╲',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '┃',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '╱',
            },
        ],
        "wilt_recovery_sprout" => &[
            SpriteCell {
                dx: 1,
                dy: 0,
                glyph: '╿',
            },
            SpriteCell {
                dx: 0,
                dy: 1,
                glyph: '╱',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '┃',
            },
            SpriteCell {
                dx: 2,
                dy: 1,
                glyph: '╲',
            },
        ],
        _ => &[
            SpriteCell {
                dx: 0,
                dy: 0,
                glyph: '◈',
            },
            SpriteCell {
                dx: 1,
                dy: 1,
                glyph: '▝',
            },
        ],
    }
}

fn render_sprite(
    anchor: Position,
    sprite: &'static [SpriteCell],
    habitat: Rect,
    exclusions: &[Rect],
    style: Style,
    pet_layer: HabitatPetLayer,
    id: &str,
) -> Vec<HabitatPropCell> {
    let mut cells = Vec::new();
    for cell in sprite {
        let Some(pos) = offset_position(anchor, cell.dx, cell.dy) else {
            return Vec::new();
        };
        if !habitat.contains(pos) || exclusions.iter().any(|rect| rect.contains(pos)) {
            return Vec::new();
        }
        cells.push(HabitatPropCell {
            prop_id: HabitatPropId::new(id),
            row: pos.y,
            col: pos.x,
            glyph: cell.glyph,
            style,
            pet_layer,
        });
    }
    cells
}

fn offset_position(anchor: Position, dx: i16, dy: i16) -> Option<Position> {
    let x = i32::from(anchor.x) + i32::from(dx);
    let y = i32::from(anchor.y) + i32::from(dy);
    if x < 0 || y < 0 {
        return None;
    }
    Some(Position::new(
        u16::try_from(x).ok()?,
        u16::try_from(y).ok()?,
    ))
}

fn stable_accent_cells_by_id<'a>(
    habitat: &'a HabitatView,
    scene: &PetSceneLayout,
    occupied: &[Rect],
    silhouette_halo: &[Rect],
    seed: &str,
    now: time::OffsetDateTime,
) -> HashMap<&'a str, HabitatPropCell> {
    let anchors = stable_accent_anchors_by_id(habitat, scene, occupied, silhouette_halo, seed);
    let mut rendered = occupied.to_vec();
    let mut cells = HashMap::new();

    for id in sorted_accent_ids(habitat) {
        let Some(anchor) = anchors.get(id).copied() else {
            continue;
        };
        let layer = prop_pet_layer(id);
        let layer_excl = exclusions_for_layer(layer, scene, &rendered, silhouette_halo);
        let mut blocked = layer_excl;
        blocked.extend(anchor_exclusions_except(&anchors, id));
        if let Some(cell) = accent_cell_from_anchor(id, anchor, scene.habitat, &blocked, layer, now)
        {
            rendered.push(Rect::new(cell.col, cell.row, 1, 1));
            cells.insert(id, cell);
        }
    }

    cells
}

fn anchor_exclusions_except(anchors: &HashMap<&str, Position>, id: &str) -> Vec<Rect> {
    anchors
        .iter()
        .filter_map(|(other_id, anchor)| {
            if *other_id == id {
                None
            } else {
                Some(Rect::new(anchor.x, anchor.y, 1, 1))
            }
        })
        .collect()
}

fn stable_accent_anchors_by_id<'a>(
    habitat: &'a HabitatView,
    scene: &PetSceneLayout,
    occupied_base: &[Rect],
    silhouette_halo: &[Rect],
    seed: &str,
) -> HashMap<&'a str, Position> {
    let mut occupied = occupied_base.to_vec();
    let mut anchors = HashMap::new();

    for id in sorted_accent_ids(habitat) {
        let layer = prop_pet_layer(id);
        let layer_excl = exclusions_for_layer(layer, scene, &occupied, silhouette_halo);
        if let Some(anchor) = accent_anchor_for(id, scene.habitat, &layer_excl, seed) {
            occupied.push(Rect::new(anchor.x, anchor.y, 1, 1));
            anchors.insert(id, anchor);
        }
    }

    anchors
}

#[cfg(test)]
fn render_accent(
    id: &str,
    habitat: Rect,
    exclusions: &[Rect],
    seed: &str,
    now: time::OffsetDateTime,
) -> Option<HabitatPropCell> {
    let anchor = accent_anchor_for(id, habitat, exclusions, seed)?;
    accent_cell_from_anchor(id, anchor, habitat, exclusions, prop_pet_layer(id), now)
}

fn accent_anchor_for(id: &str, habitat: Rect, exclusions: &[Rect], seed: &str) -> Option<Position> {
    if habitat.width < 4 || habitat.height < 3 {
        return None;
    }

    let col_min = habitat.x.saturating_add(2);
    let col_max = habitat.x.saturating_add(habitat.width.saturating_sub(2));
    let row_min = habitat.y.saturating_add(1);
    let row_max = habitat.y.saturating_add(habitat.height.saturating_sub(2));
    let col_span = col_max.saturating_sub(col_min).saturating_add(1);
    let row_span = row_max.saturating_sub(row_min).saturating_add(1);
    let base = prop_hash(seed, id);

    for attempt in 0..ACCENT_CANDIDATES {
        let phase = base.wrapping_add(
            attempt
                .wrapping_mul(37)
                .wrapping_add(attempt.wrapping_mul(attempt).wrapping_mul(11)),
        );
        let col = col_min + (phase % col_span);
        let row_phase = phase / 3 + attempt.wrapping_mul(23);
        let row = row_min + (row_phase % row_span);
        let pos = Position::new(col, row);
        if habitat.contains(pos) && !exclusions.iter().any(|rect| rect.contains(pos)) {
            return Some(pos);
        }
    }

    None
}

fn accent_cell_from_anchor(
    id: &str,
    anchor: Position,
    habitat: Rect,
    exclusions: &[Rect],
    pet_layer: HabitatPetLayer,
    now: time::OffsetDateTime,
) -> Option<HabitatPropCell> {
    if habitat.width < 4 || habitat.height < 3 {
        return None;
    }

    let col_min = habitat.x.saturating_add(2);
    let col_max = habitat.x.saturating_add(habitat.width.saturating_sub(2));
    let row_min = habitat.y.saturating_add(1);
    let row_max = habitat.y.saturating_add(habitat.height.saturating_sub(2));
    let motion = accent_motion_offset(id, now);
    let moved = Position::new(
        shift_coordinate(anchor.x, motion.0, col_min, col_max),
        shift_coordinate(anchor.y, motion.1, row_min, row_max),
    );
    let pos = if habitat.contains(moved) && !exclusions.iter().any(|rect| rect.contains(moved)) {
        moved
    } else {
        anchor
    };

    if !habitat.contains(pos) || exclusions.iter().any(|rect| rect.contains(pos)) {
        return None;
    }

    Some(HabitatPropCell {
        prop_id: HabitatPropId::new(id),
        row: pos.y,
        col: pos.x,
        glyph: accent_glyph(id, now),
        style: accent_style(id),
        pet_layer,
    })
}

fn prop_hash(seed: &str, id: &str) -> u16 {
    let mut hash = 0u16;
    for byte in seed.bytes().chain(id.bytes()) {
        hash = hash.wrapping_mul(31).wrapping_add(u16::from(byte));
    }
    hash
}

fn shift_coordinate(base: u16, delta: i8, min: u16, max: u16) -> u16 {
    (i32::from(base) + i32::from(delta)).clamp(i32::from(min), i32::from(max)) as u16
}

fn accent_motion_offset(id: &str, now: time::OffsetDateTime) -> (i8, i8) {
    let phase = now.unix_timestamp().rem_euclid(20);
    match id {
        "token_pebble_25k" | "token_shell_100k" => (0, if phase < 10 { 0 } else { -1 }),
        "token_orbit_5m" => (if phase < 10 { 0 } else { 1 }, 0),
        "token_lantern_10m" => (0, if phase < 10 { -1 } else { 0 }),
        _ => (0, 0),
    }
}

fn accent_glyph(id: &str, now: time::OffsetDateTime) -> char {
    let twinkle = now.unix_timestamp().rem_euclid(12) < 2;
    match id {
        "token_pebble_25k" => '▲',
        "token_shell_100k" => '◌',
        "token_spark_500k" if twinkle => '✦',
        "token_spark_500k" => '·',
        "token_shard_1m" => '◆',
        "token_orbit_5m" => '°',
        "token_lantern_10m" if twinkle => '☼',
        "token_lantern_10m" => '○',
        _ => '·',
    }
}

#[cfg(test)]
fn prop_visual_glyphs_for_test() -> &'static [char] {
    &[
        '╷', '◉', '○', '╵', 'ѱ', '╲', '┃', '╱', '◌', '╿', '◈', '▝', '▲', '✦', '·', '◆', '°', '☼',
        '▂', '☁', '◦', '◡', '˙', '╭', '─', '╮', '▣', '◇', '╽',
    ]
}

fn trophy_style(color_capability: ColorCapability, id: &str) -> Style {
    match color_capability {
        ColorCapability::Truecolor => Style::default().fg(prop_color(id)),
        ColorCapability::Flat => Style::default(),
    }
}

fn accent_style(id: &str) -> Style {
    Style::default().fg(prop_color(id))
}

fn prop_color(id: &str) -> Color {
    catalog_prop_by_str(id)
        .map(|spec| {
            let (r, g, b) = spec.color;
            Color::Rgb(r, g, b)
        })
        .unwrap_or_else(|| tokenpet_palette().dim.rgb)
}

fn bounds_for_cells(cells: &[HabitatPropCell]) -> Rect {
    let min_x = cells.iter().map(|cell| cell.col).min().unwrap_or(0);
    let max_x = cells.iter().map(|cell| cell.col).max().unwrap_or(min_x);
    let min_y = cells.iter().map(|cell| cell.row).min().unwrap_or(0);
    let max_y = cells.iter().map(|cell| cell.row).max().unwrap_or(min_y);
    Rect::new(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::{
        HabitatPetLayer, HabitatPropKind, TOKEN_FRIENDLY_CLOUD_750K, TOKEN_SHARD_1M,
    };
    use crate::pet::generation::Species;
    use crate::storage::state::HabitatPropId;
    use crate::tui::component::{ComponentPath, PetSceneLayout, TargetPath};
    use crate::tui::render_context::{RenderContext, WatchClock};
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::{EarnedHabitatPropView, HabitatView};
    use ratatui::{
        layout::{Position, Rect},
        style::Style,
    };
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn scene() -> PetSceneLayout {
        PetSceneLayout {
            id: ComponentPath::new("watch.pet"),
            panel: Rect::new(0, 0, 40, 12),
            speech: Some(Rect::new(0, 0, 40, 1)),
            content: Rect::new(0, 1, 40, 11),
            pet_art: Rect::new(14, 3, 13, 8),
            hit_area: Rect::new(0, 1, 40, 11),
            habitat: Rect::new(0, 0, 40, 12),
            exclusions: vec![Rect::new(0, 0, 40, 1), Rect::new(14, 3, 13, 8)],
            targets: BTreeMap::new(),
            effect_targets: vec![TargetPath::new("watch.pet.effect")],
        }
    }

    fn ctx(ts: time::OffsetDateTime) -> RenderContext {
        RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(ts))
    }

    fn earned(id: &str, kind: HabitatPropKind, priority: i16, minute: u8) -> EarnedHabitatPropView {
        EarnedHabitatPropView {
            id: HabitatPropId::new(id),
            earned_at: datetime!(2026-05-11 12:00 UTC) + time::Duration::minutes(i64::from(minute)),
            kind,
            display_priority: priority,
            source: crate::storage::state::HabitatPropSource::LifetimeTokens { threshold: 0.0 },
        }
    }

    #[test]
    fn habitat_props_attach_pet_layer_from_catalog() {
        let habitat = HabitatView {
            earned_props: vec![
                earned(TOKEN_FRIENDLY_CLOUD_750K, HabitatPropKind::Trophy, 45, 0),
                earned(TOKEN_SHARD_1M, HabitatPropKind::Accent, 40, 1),
            ],
        };
        let mut scene = scene();
        scene.exclusions.clear();

        let cells = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00 UTC)),
        );

        let cloud_cell = cells
            .iter()
            .find(|cell| cell.glyph == '☁')
            .expect("cloud rendered");
        assert_eq!(cloud_cell.pet_layer, HabitatPetLayer::Behind);

        let shard_cell = cells
            .iter()
            .find(|cell| cell.glyph == '◆')
            .expect("shard rendered");
        assert_eq!(shard_cell.pet_layer, HabitatPetLayer::Background);
    }

    #[test]
    fn background_props_avoid_silhouette_halo_rects() {
        // A Background prop (wall-mounted shard) must avoid cells in the
        // silhouette halo. Cover most of the habitat with a per-cell
        // silhouette except a small free strip on the right; the shard must
        // land in the free strip.
        let habitat_view = HabitatView {
            earned_props: vec![earned(TOKEN_SHARD_1M, HabitatPropKind::Accent, 40, 0)],
        };
        let mut scene = scene();
        scene.exclusions.clear();
        let blocked: Vec<Rect> = (0..scene.habitat.width.saturating_sub(4))
            .flat_map(|dx| (0..scene.habitat.height).map(move |dy| Rect::new(dx, dy, 1, 1)))
            .collect();

        let cells = habitat_props_for(
            &habitat_view,
            &scene,
            &blocked,
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00 UTC)),
        );

        let shard = cells
            .iter()
            .find(|cell| cell.glyph == '◆')
            .expect("shard should still render in the free strip");
        assert!(
            shard.col >= scene.habitat.width - 4,
            "Background shard must avoid blocked silhouette region; landed at col {}",
            shard.col
        );
    }

    #[test]
    fn behind_props_ignore_silhouette_halo_rects() {
        // Even if the silhouette halo covers a Behind prop's anchor zone,
        // the prop must still render — it renders before the pet pass and
        // gets visually layered behind the pet's silhouette.
        let habitat_view = HabitatView {
            earned_props: vec![earned(
                TOKEN_FRIENDLY_CLOUD_750K,
                HabitatPropKind::Trophy,
                45,
                0,
            )],
        };
        let mut scene = scene();
        scene.exclusions.clear();
        // Block the entire habitat at the cell level. A Background prop
        // would be unable to render anywhere; a Behind prop must ignore it.
        let blocked: Vec<Rect> = (0..scene.habitat.width)
            .flat_map(|dx| (0..scene.habitat.height).map(move |dy| Rect::new(dx, dy, 1, 1)))
            .collect();

        let cells = habitat_props_for(
            &habitat_view,
            &scene,
            &blocked,
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00 UTC)),
        );

        assert!(
            cells.iter().any(|cell| cell.glyph == '☁'),
            "Behind cloud must render even when silhouette covers its anchor zone"
        );
    }

    #[test]
    fn prop_cells_stay_inside_habitat_and_outside_exclusions() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0),
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 1),
            ],
        };

        let cells = habitat_props_for(
            &habitat,
            &scene(),
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:10 UTC)),
        );

        assert!(!cells.is_empty());
        assert!(cells.iter().any(|cell| cell.glyph == '▲'));
        for cell in cells {
            assert!(Rect::new(0, 0, 40, 12)
                .contains(ratatui::layout::Position::new(cell.col, cell.row)));
            assert!(!Rect::new(0, 0, 40, 1)
                .contains(ratatui::layout::Position::new(cell.col, cell.row)));
            assert!(!Rect::new(14, 3, 13, 8)
                .contains(ratatui::layout::Position::new(cell.col, cell.row)));
        }
    }

    #[test]
    fn trophy_selection_caps_at_six_by_priority_then_age() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0),
                earned("heavy_session_planter", HabitatPropKind::Trophy, 80, 1),
                earned("wilt_recovery_sprout", HabitatPropKind::Trophy, 90, 2),
                earned("extra_trophy_for_cap_test", HabitatPropKind::Trophy, 95, 3),
                earned("token_treasure_chest_2m", HabitatPropKind::Trophy, 100, 4),
                earned("token_friendly_cloud_750k", HabitatPropKind::Trophy, 110, 5),
                earned("token_hanging_vine_25m", HabitatPropKind::Trophy, 120, 6),
            ],
        };

        let selected = visible_trophy_ids(&habitat);

        assert_eq!(
            selected,
            vec![
                "token_hanging_vine_25m",
                "token_friendly_cloud_750k",
                "token_treasure_chest_2m",
                "extra_trophy_for_cap_test",
                "wilt_recovery_sprout",
                "heavy_session_planter",
            ]
        );
    }

    #[test]
    fn trophy_zones_place_objects_in_floor_air_and_center_areas() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("token_treasure_chest_2m", HabitatPropKind::Trophy, 100, 0),
                earned("token_friendly_cloud_750k", HabitatPropKind::Trophy, 110, 1),
                earned("token_hanging_vine_25m", HabitatPropKind::Trophy, 120, 2),
            ],
        };
        let mut scene = scene();
        scene.exclusions.clear();

        let cells = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00 UTC)),
        );

        assert!(
            cells.iter().any(|cell| cell.glyph == '▣'),
            "treasure chest should render"
        );
        assert!(
            cells.iter().any(|cell| cell.glyph == '☁'),
            "friendly cloud should render"
        );
        assert!(
            cells.iter().any(|cell| cell.glyph == '╽'),
            "hanging vine should render"
        );
        assert!(
            cells.iter().any(|cell| cell.col >= 16 && cell.col <= 23),
            "at least one sprite should occupy the middle habitat columns: {cells:?}"
        );
        assert!(
            cells.iter().any(|cell| cell.row <= 3),
            "at least one sprite should occupy air or ceiling rows: {cells:?}"
        );
    }

    #[test]
    fn accent_rotation_is_stable_within_ten_minute_window() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 0),
                earned("token_shell_100k", HabitatPropKind::Accent, 20, 1),
                earned("token_spark_500k", HabitatPropKind::Accent, 30, 2),
                earned("token_shard_1m", HabitatPropKind::Accent, 40, 3),
                earned("token_orbit_5m", HabitatPropKind::Accent, 50, 4),
            ],
        };

        let a = visible_accent_ids(&habitat, datetime!(2026-05-11 12:01 UTC));
        let b = visible_accent_ids(&habitat, datetime!(2026-05-11 12:09 UTC));
        let c = visible_accent_ids(&habitat, datetime!(2026-05-11 12:11 UTC));

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 4);
    }

    #[test]
    fn flat_color_omits_accents_but_keeps_trophy_shapes() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0),
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 1),
            ],
        };
        let flat_ctx = RenderContext::with_clock(
            ColorCapability::Flat,
            WatchClock::fixed(datetime!(2026-05-11 12:10 UTC)),
        );

        let cells = habitat_props_for(
            &habitat,
            &scene(),
            &[],
            Species::Fuzz,
            "fixture-seed",
            &flat_ctx,
        );
        let glyphs = cells.iter().map(|cell| cell.glyph).collect::<Vec<_>>();

        assert!(glyphs.iter().any(|glyph| *glyph == '◉' || *glyph == '○'));
        assert!(!glyphs.contains(&'▲'));
    }

    #[test]
    fn accent_motion_never_jumps_more_than_one_cell() {
        let habitat = HabitatView {
            earned_props: vec![earned("token_pebble_25k", HabitatPropKind::Accent, 10, 0)],
        };
        let mut scene = scene();
        scene.exclusions.clear();
        let first_time = datetime!(2026-05-11 12:00:00 UTC);
        let second_time = first_time + time::Duration::seconds(10);

        let first = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(first_time),
        )
        .into_iter()
        .find(|cell| cell.glyph == '▲')
        .expect("first accent");
        let second = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(second_time),
        )
        .into_iter()
        .find(|cell| cell.glyph == '▲')
        .expect("second accent");

        assert!((i32::from(first.col) - i32::from(second.col)).abs() <= 1);
        assert!((i32::from(first.row) - i32::from(second.row)).abs() <= 1);
    }

    #[test]
    fn accent_anchor_stays_with_prop_across_rotation_windows() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 0),
                earned("token_shell_100k", HabitatPropKind::Accent, 20, 1),
                earned("token_spark_500k", HabitatPropKind::Accent, 30, 2),
                earned("token_shard_1m", HabitatPropKind::Accent, 40, 3),
                earned("token_orbit_5m", HabitatPropKind::Accent, 50, 4),
            ],
        };
        let mut scene = scene();
        scene.exclusions.clear();

        let before_rotation = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:21 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◆')
        .expect("shard visible before rotation");
        let after_rotation = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:31 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◆')
        .expect("shard visible after rotation");

        assert_eq!(
            (before_rotation.col, before_rotation.row),
            (after_rotation.col, after_rotation.row)
        );
    }

    #[test]
    fn accent_collision_retry_is_stable_across_rotation_windows() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 0),
                earned("token_shell_100k", HabitatPropKind::Accent, 20, 1),
                earned("token_spark_500k", HabitatPropKind::Accent, 30, 2),
                earned("token_shard_1m", HabitatPropKind::Accent, 40, 3),
                earned("token_orbit_5m", HabitatPropKind::Accent, 50, 4),
            ],
        };
        let mut scene = scene();
        scene.habitat = Rect::new(0, 0, 5, 4);
        scene.exclusions.clear();

        let before_rotation = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:21 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◌')
        .expect("shell visible before rotation");
        let after_rotation = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:31 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◌')
        .expect("shell visible after rotation");

        assert_eq!(
            (before_rotation.col, before_rotation.row),
            (after_rotation.col, after_rotation.row)
        );
    }

    #[test]
    fn accent_collision_retry_does_not_change_with_motion_phase() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("token_pebble_25k", HabitatPropKind::Accent, 10, 0),
                earned("token_shell_100k", HabitatPropKind::Accent, 20, 1),
                earned("token_spark_500k", HabitatPropKind::Accent, 30, 2),
                earned("token_shard_1m", HabitatPropKind::Accent, 40, 3),
            ],
        };
        let mut scene = scene();
        scene.habitat = Rect::new(0, 0, 4, 7);
        scene.exclusions.clear();

        let before_phase = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00:09 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◌')
        .expect("shell visible before phase change");
        let after_phase = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00:10 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◌')
        .expect("shell visible after phase change");

        assert!(
            (i32::from(before_phase.col) - i32::from(after_phase.col)).abs() <= 1,
            "shell moved too far horizontally: {before_phase:?} -> {after_phase:?}"
        );
        assert!(
            (i32::from(before_phase.row) - i32::from(after_phase.row)).abs() <= 1,
            "shell moved too far vertically: {before_phase:?} -> {after_phase:?}"
        );
    }

    #[test]
    fn moving_accents_do_not_wrap_between_adjacent_phases() {
        for (id, glyph, seed) in [
            ("token_orbit_5m", '°', "fixture-seed"),
            ("token_lantern_10m", '○', "fixture-seed"),
        ] {
            let habitat = HabitatView {
                earned_props: vec![earned(id, HabitatPropKind::Accent, 10, 0)],
            };
            let mut scene = scene();
            scene.exclusions.clear();
            let before = habitat_props_for(
                &habitat,
                &scene,
                &[],
                Species::Fuzz,
                seed,
                &ctx(datetime!(2026-05-11 12:00:09 UTC)),
            )
            .into_iter()
            .find(|cell| cell.glyph == glyph)
            .expect("accent visible before phase change");
            let after = habitat_props_for(
                &habitat,
                &scene,
                &[],
                Species::Fuzz,
                seed,
                &ctx(datetime!(2026-05-11 12:00:10 UTC)),
            )
            .into_iter()
            .find(|cell| cell.glyph == glyph)
            .expect("accent visible after phase change");

            assert!(
                (i32::from(before.col) - i32::from(after.col)).abs() <= 1,
                "{id} moved too far horizontally: {before:?} -> {after:?}"
            );
            assert!(
                (i32::from(before.row) - i32::from(after.row)).abs() <= 1,
                "{id} moved too far vertically: {before:?} -> {after:?}"
            );
        }
    }

    #[test]
    fn trophy_color_comes_from_catalog_not_species() {
        // The habitat is a place; the pet is a creature in it. Props should
        // look like themselves (signal lamps are red, moss is green) and
        // stay the same color across every species.
        let habitat = HabitatView {
            earned_props: vec![earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0)],
        };
        let mut scene = scene();
        scene.exclusions.clear();
        let fuzz = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Fuzz,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◉')
        .expect("fuzz lamp lit cell");
        let mech = habitat_props_for(
            &habitat,
            &scene,
            &[],
            Species::Mech,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◉')
        .expect("mech lamp lit cell");

        let (r, g, b) = crate::game::habitat::catalog_prop_by_str("codex_signal_lamp")
            .unwrap()
            .color;
        assert_eq!(fuzz.style.fg, Some(Color::Rgb(r, g, b)));
        assert_eq!(mech.style.fg, Some(Color::Rgb(r, g, b)));
    }

    #[test]
    fn accent_retry_escapes_blocked_first_column() {
        let habitat = Rect::new(0, 0, 20, 8);
        let blocked_col = habitat.x
            + 2
            + (prop_hash("fixture-seed", "token_pebble_25k") % habitat.width.saturating_sub(3));
        let exclusions = [Rect::new(
            blocked_col,
            1,
            1,
            habitat.height.saturating_sub(2),
        )];

        let cell = render_accent(
            "token_pebble_25k",
            habitat,
            &exclusions,
            "fixture-seed",
            datetime!(2026-05-11 12:00 UTC),
        )
        .expect("accent should find another free column");

        assert_ne!(cell.col, blocked_col);
    }

    #[test]
    fn trophy_sprites_render_all_or_nothing() {
        let cells = render_sprite(
            Position::new(3, 2),
            trophy_sprite(
                "codex_signal_lamp",
                Species::Fuzz,
                datetime!(2026-05-11 12:00 UTC),
            ),
            Rect::new(0, 0, 8, 4),
            &[],
            Style::default(),
            HabitatPetLayer::Background,
            "codex_signal_lamp",
        );

        assert!(cells.is_empty());
    }

    #[test]
    fn planter_and_sprout_sway_with_clock() {
        let first_time = datetime!(2026-05-11 12:00:00 UTC);
        let second_time = first_time + time::Duration::seconds(4);

        let planter_a = trophy_sprite("heavy_session_planter", Species::Fuzz, first_time)
            .iter()
            .map(|cell| cell.glyph)
            .collect::<Vec<_>>();
        let planter_b = trophy_sprite("heavy_session_planter", Species::Fuzz, second_time)
            .iter()
            .map(|cell| cell.glyph)
            .collect::<Vec<_>>();
        let sprout_a = trophy_sprite("wilt_recovery_sprout", Species::Fuzz, first_time)
            .iter()
            .map(|cell| cell.glyph)
            .collect::<Vec<_>>();
        let sprout_b = trophy_sprite("wilt_recovery_sprout", Species::Fuzz, second_time)
            .iter()
            .map(|cell| cell.glyph)
            .collect::<Vec<_>>();

        assert_ne!(planter_a, planter_b);
        assert_ne!(sprout_a, sprout_b);
    }

    #[test]
    fn prop_visual_glyphs_are_single_scalar_values() {
        for glyph in prop_visual_glyphs_for_test() {
            assert_eq!(
                glyph.to_string().chars().count(),
                1,
                "{glyph} must be one char"
            );
            assert_eq!(
                ratatui::text::Span::raw(glyph.to_string()).width(),
                1,
                "{glyph} must be one terminal cell under ratatui width"
            );
        }
    }

    fn test_scene() -> PetSceneLayout {
        scene()
    }

    fn habitat_with_props(ids: &[&str]) -> HabitatView {
        HabitatView {
            earned_props: ids
                .iter()
                .map(|id| earned(id, HabitatPropKind::Trophy, 50, 0))
                .collect(),
        }
    }

    #[test]
    fn prop_placements_group_cells_with_bounds_and_static_targets() {
        let ctx = RenderContext::with_clock(
            ColorCapability::Truecolor,
            crate::tui::render_context::WatchClock::fixed(
                time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap(),
            ),
        );
        let scene = test_scene();
        let habitat = habitat_with_props(&[
            crate::game::habitat::CODEX_SIGNAL_LAMP,
            crate::game::habitat::HEAVY_SESSION_PLANTER,
        ]);

        let placements =
            habitat_prop_placements_for(&habitat, &scene, &[], Species::Fuzz, "seed", &ctx);

        assert!(placements.iter().any(|placement| {
            placement.prop_id.as_str() == crate::game::habitat::CODEX_SIGNAL_LAMP
                && placement.target_id.as_ref().unwrap().as_str()
                    == "watch.prop.codex_signal_lamp.effect"
                && !placement.cells.is_empty()
                && placement.bounds.width > 0
                && placement.bounds.height > 0
        }));
    }

    #[test]
    fn habitat_props_for_flattens_shared_placements() {
        let ctx = RenderContext::with_clock(
            ColorCapability::Truecolor,
            crate::tui::render_context::WatchClock::fixed(
                time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap(),
            ),
        );
        let scene = test_scene();
        let habitat = habitat_with_props(&[crate::game::habitat::HEAVY_SESSION_PLANTER]);

        let placements =
            habitat_prop_placements_for(&habitat, &scene, &[], Species::Fuzz, "seed", &ctx);
        let cells = habitat_props_for(&habitat, &scene, &[], Species::Fuzz, "seed", &ctx);

        assert_eq!(
            cells,
            placements
                .iter()
                .flat_map(|placement| placement.cells.clone())
                .collect::<Vec<_>>()
        );
    }
}
