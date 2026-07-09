use ratatui::layout::Rect;
use ratatui::style::Color;

use super::ambient::{biome_floor_wash_color, biome_wash_color, contact_shadow_color};
use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use crate::tui::room::RoomBiomeTag;

/// The lower N habitat rows painted with the deeper floor wash so the ground
/// reads as a value distinct from the lighter sky above it.
pub(super) const FLOOR_BAND_ROWS: u16 = 3;

const FLOOR_PROJECTION_FAR_TINT: Rgb = Rgb::new(0x43, 0x31, 0x54);
const FLOOR_PROJECTION_NEAR_TINT: Rgb = Rgb::new(0x66, 0x4b, 0x79);
const WALL_SHADOW_TINT: Rgb = Rgb::new(0x3c, 0x2c, 0x49);
const WALL_SHADOW_OFFSET: u16 = 1;

/// Computes the y-coordinate that anchors the pet art's feet one row above the
/// habitat floor. `art_lines` are the framed 10 rows (`vm.pet_art`); `feet_row`
/// returns the lowest non-blank framed row. Clamps to `area.y` when the area is
/// too short for the pet (degenerate, no panic).
pub(crate) fn pet_feet_anchor_y(area: Rect, art_lines: &[String], pet_h: u16) -> u16 {
    let floor_row = area.y + area.height.saturating_sub(1);
    // Reserve one row for the floor band beneath the feet.
    let feet_target_row = floor_row.saturating_sub(1);
    let feet =
        crate::pet::render::feet_row(art_lines).unwrap_or((pet_h as usize).saturating_sub(1));
    let anchor = feet_target_row.saturating_sub(feet as u16);
    anchor.max(area.y)
}

/// Returns one bg-only [`DrawCell`] per habitat cell. Sky rows use
/// [`biome_wash_color`]; the lower [`FLOOR_BAND_ROWS`] use broad stepped value
/// bands with sparse tile variation so the floor remains legible on small
/// companion displays.
pub(super) fn biome_wash_cells(habitat: Rect, biome: RoomBiomeTag) -> Vec<DrawCell> {
    let sky_wash = biome_wash_color(biome);
    let floor_wash = biome_floor_wash_color(biome);
    let floor_band_top = habitat
        .y
        .saturating_add(habitat.height.saturating_sub(FLOOR_BAND_ROWS));
    let mut cells = Vec::with_capacity((habitat.width as usize) * (habitat.height as usize));
    for wy in habitat.y..habitat.y.saturating_add(habitat.height) {
        let floor_row = wy.saturating_sub(floor_band_top);
        for wx in habitat.x..habitat.x.saturating_add(habitat.width) {
            let wash = if wy >= floor_band_top {
                floor_substrate_color(floor_wash, floor_row, wx)
            } else {
                sky_wash
            };
            let Some(bg) = rgb_from_color(wash) else {
                continue; // non-RGB color cap: skip
            };
            cells.push(DrawCell {
                row: wy,
                col: wx,
                glyph: None,
                fg: None,
                bg: Some(bg),
                bold: false,
            });
        }
    }
    cells
}

/// Projects a compact, tapered shadow onto the habitat's bottom substrate.
/// The Classic companion places the pet high in the tank, so drawing at its
/// feet would paint a wall decal rather than a floor shadow. The projection is
/// instead centered beneath the rendered body and confined to the floor band.
pub(super) fn floor_projection_draw_cells(
    pet_body: &[DrawCell],
    habitat: Rect,
    biome: RoomBiomeTag,
) -> Vec<DrawCell> {
    let floor_wash = biome_floor_wash_color(biome);
    let Some((far_color, near_color)) = floor_projection_colors(floor_wash) else {
        return Vec::new(); // non-RGB color cap: skip
    };
    let Some((body_start, body_end)) = horizontal_body_bounds(pet_body) else {
        return Vec::new();
    };
    let floor_top = habitat
        .y
        .saturating_add(habitat.height.saturating_sub(FLOOR_BAND_ROWS));
    let far_row = floor_top.saturating_add(1);
    let near_row = far_row.saturating_add(1);
    let habitat_bottom = habitat.y.saturating_add(habitat.height);
    if near_row >= habitat_bottom {
        return Vec::new();
    }

    let center = body_start.saturating_add(body_end.saturating_sub(body_start) / 2);
    let body_width = body_end.saturating_sub(body_start).saturating_add(1);
    let far_half_span = (body_width.saturating_add(3) / 6).max(1);
    let near_half_span = far_half_span.saturating_add(1);
    let mut cells = Vec::new();
    let (far_start, far_end) = centered_span(center, far_half_span, habitat);
    append_shadow_band(&mut cells, far_row, far_start, far_end, far_color);
    let (near_start, near_end) = centered_span(center, near_half_span, habitat);
    append_shadow_band(&mut cells, near_row, near_start, near_end, near_color);
    cells
}

/// Builds a dim, offset silhouette on the tank wall. It is deliberately a
/// separate layer so the cast stays behind the pet while the floor projection
/// remains below its feet.
pub(super) fn wall_shadow_draw_cells(
    pet_body: &[DrawCell],
    habitat: Rect,
    biome: RoomBiomeTag,
) -> Vec<DrawCell> {
    let Some(wall) = rgb_from_color(biome_wash_color(biome)) else {
        return Vec::new();
    };
    let color = blend_rgb(adjust_rgb(wall, -6), WALL_SHADOW_TINT, 48);
    let mut spans: Vec<(u16, u16, u16)> = Vec::new();
    for cell in pet_body.iter().filter(|cell| cell.glyph.is_some()) {
        if let Some((_, start, end)) = spans.iter_mut().find(|(row, _, _)| *row == cell.row) {
            *start = (*start).min(cell.col);
            *end = (*end).max(cell.col);
        } else {
            spans.push((cell.row, cell.col, cell.col));
        }
    }

    let habitat_end = habitat.x.saturating_add(habitat.width).saturating_sub(1);
    let habitat_bottom = habitat.y.saturating_add(habitat.height);
    let mut cells = Vec::new();
    for (row, start, end) in spans {
        let shadow_row = row.saturating_add(WALL_SHADOW_OFFSET);
        if shadow_row >= habitat_bottom {
            continue;
        }
        let shadow_start = start.saturating_add(WALL_SHADOW_OFFSET).max(habitat.x);
        let shadow_end = end.saturating_add(WALL_SHADOW_OFFSET).min(habitat_end);
        if shadow_start <= shadow_end {
            append_shadow_band(&mut cells, shadow_row, shadow_start, shadow_end, color);
        }
    }

    cells
}

fn append_shadow_band(cells: &mut Vec<DrawCell>, row: u16, start: u16, end: u16, bg: Rgb) {
    for col in start..=end {
        cells.push(DrawCell {
            row,
            col,
            glyph: None,
            fg: None,
            bg: Some(bg),
            bold: false,
        });
    }
}

fn floor_substrate_color(floor_wash: Color, floor_row: u16, col: u16) -> Color {
    let Color::Rgb(r, g, b) = floor_wash else {
        return floor_wash;
    };
    let band_delta = match floor_row {
        0 => 6,
        1 => 0,
        _ => -8,
    };
    let tile_delta = match col % 7 {
        0 => 3,
        4 => -3,
        _ => 0,
    };
    let floor = Rgb::new(r, g, b);
    let color = adjust_rgb(floor, band_delta + tile_delta);
    Color::Rgb(color.r, color.g, color.b)
}

fn floor_projection_colors(floor_wash: Color) -> Option<(Rgb, Rgb)> {
    let shadow = rgb_from_color(contact_shadow_color(floor_wash))?;
    let far = blend_rgb(shadow, FLOOR_PROJECTION_FAR_TINT, 64);
    let near = blend_rgb(shadow, FLOOR_PROJECTION_NEAR_TINT, 88);
    Some((far, near))
}

fn horizontal_body_bounds(pet_body: &[DrawCell]) -> Option<(u16, u16)> {
    let mut glyphs = pet_body.iter().filter(|cell| cell.glyph.is_some());
    let first = glyphs.next()?;
    Some(glyphs.fold((first.col, first.col), |(start, end), cell| {
        (start.min(cell.col), end.max(cell.col))
    }))
}

fn centered_span(center: u16, half_span: u16, habitat: Rect) -> (u16, u16) {
    let habitat_end = habitat.x.saturating_add(habitat.width).saturating_sub(1);
    (
        center.saturating_sub(half_span).max(habitat.x),
        center.saturating_add(half_span).min(habitat_end),
    )
}

fn rgb_from_color(color: Color) -> Option<Rgb> {
    match color {
        Color::Rgb(r, g, b) => Some(Rgb::new(r, g, b)),
        _ => None,
    }
}

fn adjust_rgb(color: Rgb, delta: i16) -> Rgb {
    let adjust = |channel: u8| (i16::from(channel) + delta).clamp(0, 255) as u8;
    Rgb::new(adjust(color.r), adjust(color.g), adjust(color.b))
}

fn blend_rgb(base: Rgb, tint: Rgb, tint_weight: u16) -> Rgb {
    let base_weight = 255_u16.saturating_sub(tint_weight);
    let blend = |base: u8, tint: u8| {
        ((u16::from(base) * base_weight + u16::from(tint) * tint_weight) / 255) as u8
    };
    Rgb::new(
        blend(base.r, tint.r),
        blend(base.g, tint.g),
        blend(base.b, tint.b),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::PET_H;
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn pet_feet_anchor_drops_feet_one_row_above_floor() {
        // 13×10 frame: art occupies frame rows 1..=8. A pet whose lowest non-blank
        // art row is art-row 5 (frame row 6) should anchor so frame row 6 lands at
        // habitat_floor_row - 1, i.e. the feet sit just above the floor band.
        let area = Rect::new(0, 0, 13, 24);
        // 10 lines, last non-blank art line at index 5; indices 6,7 blank; plus the
        // two particle-gutter frame rows are NOT part of art_lines here — art_lines
        // are the 8 art rows the renderer passes (vm.pet_art is the framed 10 rows;
        // feet_row operates on the framed lines).
        let art_lines: Vec<String> = vec![
            "             ".to_string(), // 0 gutter
            "    ▟██▙     ".to_string(), // 1
            "   ▓██████   ".to_string(), // 2
            "   ▒o o▒     ".to_string(), // 3
            "   ▒ w ▒     ".to_string(), // 4
            "   ▙▒▒▟      ".to_string(), // 5 feet (lowest non-blank)
            "             ".to_string(), // 6
            "             ".to_string(), // 7
            "             ".to_string(), // 8
            "             ".to_string(), // 9 gutter
        ];
        let y = pet_feet_anchor_y(area, &art_lines, PET_H);
        // feet at framed row 5; floor row = 23; we want framed row 5 -> row 22.
        // So pet_rect.y = 22 - 5 = 17.
        assert_eq!(y, 17, "feet should land one row above the floor");
    }

    #[test]
    fn pet_feet_anchor_clamps_when_area_shorter_than_pet() {
        let area = Rect::new(0, 5, 13, 4); // shorter than PET_H=10
        let art_lines: Vec<String> = (0..10).map(|_| "      X      ".to_string()).collect();
        let y = pet_feet_anchor_y(area, &art_lines, PET_H);
        assert_eq!(y, area.y, "degenerate area clamps to origin, no underflow");
    }

    #[test]
    fn floor_projection_stays_on_the_substrate_and_tapers_toward_the_far_edge() {
        let pet_body = (10..=16)
            .map(|col| DrawCell {
                row: 5,
                col,
                glyph: Some("X".to_string()),
                fg: None,
                bg: None,
                bold: false,
            })
            .collect::<Vec<_>>();
        let habitat = Rect::new(0, 0, 40, 24);
        let cells = floor_projection_draw_cells(&pet_body, habitat, RoomBiomeTag::Starter);

        assert_eq!(
            cells.len(),
            8,
            "the projection is a compact two-row trapezoid"
        );
        assert!(cells.iter().all(|cell| cell.glyph.is_none()));
        assert!(cells.iter().all(|cell| cell.row >= 21));
        assert_eq!(cells.iter().filter(|cell| cell.row == 22).count(), 3);
        assert_eq!(cells.iter().filter(|cell| cell.row == 23).count(), 5);
        assert_eq!(cells.iter().map(|cell| cell.col).min(), Some(11));
        assert_eq!(cells.iter().map(|cell| cell.col).max(), Some(15));

        let color_at = |col, row| {
            cells
                .iter()
                .find(|cell| cell.col == col && cell.row == row)
                .and_then(|cell| cell.bg)
                .unwrap()
        };
        let far = color_at(13, 22);
        let near = color_at(13, 23);
        let luminance = |color: Rgb| u32::from(color.r) + u32::from(color.g) + u32::from(color.b);
        assert!(luminance(far) < luminance(near));
    }

    #[test]
    fn wall_shadow_offsets_and_fills_the_pet_silhouette_behind_its_body() {
        let pet_body = vec![
            DrawCell {
                row: 5,
                col: 4,
                glyph: Some("X".to_string()),
                fg: None,
                bg: None,
                bold: false,
            },
            DrawCell {
                row: 5,
                col: 6,
                glyph: Some("X".to_string()),
                fg: None,
                bg: None,
                bold: false,
            },
            DrawCell {
                row: 6,
                col: 5,
                glyph: Some("X".to_string()),
                fg: None,
                bg: None,
                bold: false,
            },
        ];
        let cells =
            wall_shadow_draw_cells(&pet_body, Rect::new(0, 0, 20, 20), RoomBiomeTag::Starter);

        let positions: std::collections::HashSet<_> =
            cells.iter().map(|cell| (cell.col, cell.row)).collect();
        assert_eq!(positions, [(5, 6), (6, 6), (7, 6), (6, 7)].into());
        assert!(cells.iter().all(|cell| cell.glyph.is_none()));
        assert!(cells.iter().all(|cell| cell.bg.is_some()));
    }

    #[test]
    fn biome_wash_uses_textured_three_value_substrate() {
        let habitat = Rect::new(0, 0, 12, 6);
        let cells = biome_wash_cells(habitat, RoomBiomeTag::Starter);
        let bg_at = |col, row| {
            cells
                .iter()
                .find(|cell| cell.col == col && cell.row == row)
                .and_then(|cell| cell.bg)
                .expect("every wash cell should have an RGB background")
        };
        let luminance = |color: Rgb| u32::from(color.r) + u32::from(color.g) + u32::from(color.b);

        let top = bg_at(1, 3);
        let middle = bg_at(1, 4);
        let bottom = bg_at(1, 5);
        assert!(luminance(top) > luminance(middle));
        assert!(luminance(middle) > luminance(bottom));
        assert_ne!(
            bg_at(0, 3),
            top,
            "sparse tile variation breaks up the flat band"
        );
    }

    #[test]
    fn floor_projection_skips_habitats_without_two_substrate_rows() {
        let pet_body = vec![DrawCell {
            row: 0,
            col: 6,
            glyph: Some("X".to_string()),
            fg: None,
            bg: None,
            bold: false,
        }];
        let habitat = Rect::new(0, 0, 13, 2);
        let cells = floor_projection_draw_cells(&pet_body, habitat, RoomBiomeTag::Starter);
        assert!(
            cells.is_empty(),
            "a projection requires the two visible rows beneath the floor horizon"
        );
    }

    #[test]
    fn floor_projection_deepens_substrate_without_replacing_glyphs() {
        use super::super::{pet_inner_rect_in_panel, PetPanel};
        use crate::tui::panels::pet::tests::{test_context, vm_with_real_pet};
        use crate::tui::panels::LegacyPanel;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        // Build the body from the displayed art region and verify the projection
        // is a background-only layer in the actual substrate rows.
        let area = f_area();
        let pet_rect = pet_inner_rect_in_panel(area, &vm);
        let pet_body = vm
            .pet_art
            .iter()
            .enumerate()
            .flat_map(|(row, line)| {
                line.chars().enumerate().filter_map(move |(col, glyph)| {
                    (!glyph.is_whitespace()).then_some(DrawCell {
                        row: pet_rect.y + row as u16,
                        col: pet_rect.x + col as u16,
                        glyph: Some(glyph.to_string()),
                        fg: None,
                        bg: None,
                        bold: false,
                    })
                })
            })
            .collect::<Vec<_>>();
        let cells = floor_projection_draw_cells(&pet_body, area, RoomBiomeTag::Starter);
        assert!(!cells.is_empty(), "a grounded pet has a floor projection");
        let mut deepened = 0usize;
        for cell in &cells {
            if cell.col < 40 && cell.row < 24 {
                if let Some(ratatui::style::Color::Rgb(..)) = buf[(cell.col, cell.row)].style().bg {
                    deepened += 1;
                }
            }
        }
        assert!(deepened > 0, "shadow cells must carry a deepened bg tint");
    }

    fn f_area() -> Rect {
        Rect::new(0, 0, 40, 24)
    }

    #[test]
    fn pet_sits_in_the_lower_half_at_narrow_column_width() {
        use super::super::PetPanel;
        use crate::tui::panels::pet::tests::{test_context, vm_with_real_pet};
        use crate::tui::panels::LegacyPanel;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // 40-wide is the real pet column. In a tall area the lowest pet glyph row
        // must be in the lower half — proof the pet is grounded, not centered.
        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut lowest_pet_row = 0u16;
        for y in 0..24u16 {
            for x in 0..40u16 {
                let sym = buf[(x, y)].symbol();
                // pet art uses block + ascii glyphs; floor uses dot texture.
                if matches!(sym.chars().next(), Some(c) if "▟▙█▓▒owO".contains(c)) {
                    lowest_pet_row = lowest_pet_row.max(y);
                }
            }
        }
        assert!(
            lowest_pet_row >= 12,
            "grounded pet's lowest glyph should be in the lower half (row >= 12), got {lowest_pet_row}"
        );
    }

    #[test]
    fn grounded_scene_stays_calm_at_narrow_column_width() {
        use super::super::PetPanel;
        use crate::tui::panels::pet::tests::{test_context, vm_with_real_pet};
        use crate::tui::panels::LegacyPanel;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        // The real pet column is ~40 wide. Render a full panel and assert the
        // scene is grounded (pet low) and calm (no excessive bright churn): the
        // contact shadow + floor wash are bg-only, so the count of non-blank
        // GLYPH cells stays bounded and the pet still reads.
        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let glyph_cells: usize = (0..18u16)
            .flat_map(|y| (0..40u16).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].symbol() != " ")
            .count();
        assert!(glyph_cells > 5, "pet + floor must render visible content");
        // Calm ceiling: a 40×18 = 720-cell panel should not be glyph-saturated.
        assert!(
            glyph_cells < 720 / 2,
            "scene must stay calm — fewer than half the cells carry glyphs; got {glyph_cells}"
        );
    }
}
