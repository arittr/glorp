use crate::game::habitat::HabitatPropKind;
use crate::pet::generation::Species;
use crate::tui::component::PetSceneLayout;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{tokenpet_palette, ColorCapability};
use crate::tui::view_model::HabitatView;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use std::collections::HashMap;

const MAX_TROPHIES: usize = 3;
const MAX_ACCENTS: usize = 4;
const ACCENT_ROTATION_SECS: i64 = 600;
const ACCENT_CANDIDATES: u16 = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct HabitatPropCell {
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
}

#[derive(Clone, Copy)]
struct SpriteCell {
    dx: i16,
    dy: i16,
    glyph: char,
}

pub fn habitat_props_for(
    habitat: &HabitatView,
    scene: &PetSceneLayout,
    species: Species,
    seed: &str,
    ctx: &RenderContext,
) -> Vec<HabitatPropCell> {
    let now = ctx.clock.now_utc();
    let mut occupied = scene.exclusions.clone();
    let mut cells = Vec::new();

    for id in visible_trophy_ids(habitat) {
        if let Some(anchor) = trophy_anchor(id, scene.habitat) {
            let rendered = render_sprite(
                anchor,
                trophy_sprite(id, species, now),
                scene.habitat,
                &occupied,
                trophy_style(ctx.color_capability, species),
            );
            if !rendered.is_empty() {
                occupied.push(bounds_for_cells(&rendered));
                cells.extend(rendered);
            }
        }
    }

    if matches!(ctx.color_capability, ColorCapability::Truecolor) {
        let accent_cells = stable_accent_cells_by_id(habitat, scene.habitat, &occupied, seed, now);
        for id in visible_accent_ids(habitat, now) {
            if let Some(cell) = accent_cells.get(id) {
                cells.push(cell.clone());
            }
        }
    }

    cells
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

fn trophy_anchor(id: &str, habitat: Rect) -> Option<Position> {
    if habitat.width < 8 || habitat.height < 4 {
        return None;
    }
    let bottom = habitat.y + habitat.height.saturating_sub(2);
    match id {
        "wilt_recovery_sprout" => Some(Position::new(habitat.x + 2, bottom.saturating_sub(2))),
        "heavy_session_planter" => Some(Position::new(
            habitat.x + habitat.width.saturating_sub(8),
            bottom.saturating_sub(2),
        )),
        "codex_signal_lamp" => Some(Position::new(
            habitat.x + habitat.width.saturating_sub(5),
            habitat.y + 2,
        )),
        _ => Some(Position::new(habitat.x + 3, bottom.saturating_sub(2))),
    }
}

fn trophy_sprite(id: &str, _species: Species, now: time::OffsetDateTime) -> &'static [SpriteCell] {
    let phase = now.unix_timestamp().rem_euclid(8);
    match id {
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
            row: pos.y,
            col: pos.x,
            glyph: cell.glyph,
            style,
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
    area: Rect,
    exclusions: &[Rect],
    seed: &str,
    now: time::OffsetDateTime,
) -> HashMap<&'a str, HabitatPropCell> {
    let anchors = stable_accent_anchors_by_id(habitat, area, exclusions, seed);
    let mut rendered = exclusions.to_vec();
    let mut cells = HashMap::new();

    for id in sorted_accent_ids(habitat) {
        let Some(anchor) = anchors.get(id).copied() else {
            continue;
        };
        let mut blocked = rendered.clone();
        blocked.extend(anchor_exclusions_except(&anchors, id));
        if let Some(cell) = accent_cell_from_anchor(id, anchor, area, &blocked, now) {
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
    area: Rect,
    exclusions: &[Rect],
    seed: &str,
) -> HashMap<&'a str, Position> {
    let mut occupied = exclusions.to_vec();
    let mut anchors = HashMap::new();

    for id in sorted_accent_ids(habitat) {
        if let Some(anchor) = accent_anchor_for(id, area, &occupied, seed) {
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
    accent_cell_from_anchor(id, anchor, habitat, exclusions, now)
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
        row: pos.y,
        col: pos.x,
        glyph: accent_glyph(id, now),
        style: accent_style(),
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
    ]
}

fn trophy_style(color_capability: ColorCapability, species: Species) -> Style {
    match color_capability {
        ColorCapability::Truecolor => Style::default().fg(species_trophy_color(species)),
        ColorCapability::Flat => Style::default(),
    }
}

fn species_trophy_color(species: Species) -> Color {
    match species {
        Species::Fuzz => Color::Rgb(0xff, 0xc8, 0x96),
        Species::Blob => Color::Rgb(0x8c, 0xdc, 0xa0),
        Species::Ghost => Color::Rgb(0xbe, 0xaa, 0xf0),
        Species::Glitch => Color::Rgb(0x78, 0xff, 0xb4),
        Species::Crystal => Color::Rgb(0xaa, 0xdc, 0xff),
        Species::Mech => Color::Rgb(0xff, 0xdc, 0x64),
    }
}

fn accent_style() -> Style {
    Style::default().fg(tokenpet_palette().dim.rgb)
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
    use crate::game::habitat::HabitatPropKind;
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
        }
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
    fn trophy_selection_caps_at_three_by_priority_then_age() {
        let habitat = HabitatView {
            earned_props: vec![
                earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0),
                earned("heavy_session_planter", HabitatPropKind::Trophy, 80, 1),
                earned("wilt_recovery_sprout", HabitatPropKind::Trophy, 90, 2),
                earned("extra_trophy_for_cap_test", HabitatPropKind::Trophy, 95, 3),
            ],
        };

        let selected = visible_trophy_ids(&habitat);

        assert_eq!(
            selected,
            vec![
                "extra_trophy_for_cap_test",
                "wilt_recovery_sprout",
                "heavy_session_planter"
            ]
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

        let cells = habitat_props_for(&habitat, &scene(), Species::Fuzz, "fixture-seed", &flat_ctx);
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
    fn trophy_color_uses_species_tint_not_global_accent() {
        let habitat = HabitatView {
            earned_props: vec![earned("codex_signal_lamp", HabitatPropKind::Trophy, 70, 0)],
        };
        let mut scene = scene();
        scene.exclusions.clear();
        let fuzz = habitat_props_for(
            &habitat,
            &scene,
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
            Species::Mech,
            "fixture-seed",
            &ctx(datetime!(2026-05-11 12:00 UTC)),
        )
        .into_iter()
        .find(|cell| cell.glyph == '◉')
        .expect("mech lamp lit cell");

        assert_ne!(fuzz.style.fg, mech.style.fg);
        assert_ne!(fuzz.style.fg, Some(tokenpet_palette().accent.rgb));
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
}
