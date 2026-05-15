use crate::game::habitat::HabitatPropKind;
use crate::pet::generation::Species;
use crate::tui::component::PetSceneLayout;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{tokenpet_palette, ColorCapability};
use crate::tui::view_model::HabitatView;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

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
                trophy_style(ctx.color_capability),
            );
            if !rendered.is_empty() {
                occupied.push(bounds_for_cells(&rendered));
                cells.extend(rendered);
            }
        }
    }

    if matches!(ctx.color_capability, ColorCapability::Truecolor) {
        for (index, id) in visible_accent_ids(habitat, now).iter().enumerate() {
            if let Some(cell) =
                render_accent(id, index, scene.habitat, &occupied, species, seed, now)
            {
                occupied.push(Rect::new(cell.col, cell.row, 1, 1));
                cells.push(cell);
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

    if props.len() <= MAX_ACCENTS {
        return props.into_iter().map(|prop| prop.id.as_str()).collect();
    }

    let start =
        (now.unix_timestamp() / ACCENT_ROTATION_SECS).rem_euclid(props.len() as i64) as usize;
    (0..MAX_ACCENTS)
        .map(|offset| props[(start + offset) % props.len()].id.as_str())
        .collect()
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
        "heavy_session_planter" => &[
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
        "wilt_recovery_sprout" => &[
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
            continue;
        };
        if habitat.contains(pos) && !exclusions.iter().any(|rect| rect.contains(pos)) {
            cells.push(HabitatPropCell {
                row: pos.y,
                col: pos.x,
                glyph: cell.glyph,
                style,
            });
        }
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

fn render_accent(
    id: &str,
    index: usize,
    habitat: Rect,
    exclusions: &[Rect],
    _species: Species,
    seed: &str,
    now: time::OffsetDateTime,
) -> Option<HabitatPropCell> {
    if habitat.width < 4 || habitat.height < 3 {
        return None;
    }

    let glyph = accent_glyph(id, now);
    let width_span = habitat.width.saturating_sub(3);
    let row_span = habitat.height.saturating_sub(2);
    let base = prop_hash(seed, id, index, now.unix_timestamp() / ACCENT_ROTATION_SECS);

    for attempt in 0..ACCENT_CANDIDATES {
        let phase = base.wrapping_add(attempt.wrapping_mul(17));
        let col = habitat.x + 2 + (phase % width_span);
        let row = habitat.y + 1 + ((phase / 3 + attempt.wrapping_mul(5)) % row_span);
        let pos = Position::new(col, row);
        if habitat.contains(pos) && !exclusions.iter().any(|rect| rect.contains(pos)) {
            return Some(HabitatPropCell {
                row,
                col,
                glyph,
                style: accent_style(),
            });
        }
    }

    None
}

fn prop_hash(seed: &str, id: &str, index: usize, window: i64) -> u16 {
    let mut hash = index as u16;
    for byte in seed.bytes().chain(id.bytes()).chain(window.to_le_bytes()) {
        hash = hash.wrapping_mul(31).wrapping_add(u16::from(byte));
    }
    hash
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

fn trophy_style(color_capability: ColorCapability) -> Style {
    match color_capability {
        ColorCapability::Truecolor => Style::default().fg(tokenpet_palette().accent.rgb),
        ColorCapability::Flat => Style::default(),
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
    use ratatui::layout::Rect;
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
    fn prop_visual_glyphs_are_single_scalar_values() {
        for glyph in prop_visual_glyphs_for_test() {
            assert_eq!(
                glyph.to_string().chars().count(),
                1,
                "{glyph} must be one char"
            );
        }
    }
}
