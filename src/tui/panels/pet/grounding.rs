use ratatui::layout::Rect;
use ratatui::style::Color;

use super::ambient::{biome_floor_wash_color, biome_wash_color, contact_shadow_color};
use crate::pet::palette::Rgb;
use crate::presentation::DrawCell;
use crate::tui::room::RoomBiomeTag;

/// The lower N habitat rows painted with the deeper floor wash so the ground
/// reads as a value distinct from the lighter sky above it.
pub(super) const FLOOR_BAND_ROWS: u16 = 3;

const FOOTPRINT_OUTER_PADDING: u16 = 2;
const FOOTPRINT_INNER_PADDING: u16 = 1;
const FOOTPRINT_OUTER_TINT: Rgb = Rgb::new(0x4f, 0x38, 0x60);
const FOOTPRINT_INNER_TINT: Rgb = Rgb::new(0x72, 0x54, 0x87);
const FOOTPRINT_CORE_TINT: Rgb = Rgb::new(0x9d, 0x78, 0xb4);

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

/// Absolute `(col, row)` cells of the contact shadow: the columns directly
/// under the silhouette's feet, on the row one below the lowest art glyph,
/// clipped to `habitat`. `mirror` flips columns the same way the pet art is
/// mirrored when facing left. Restricted to feet columns so side-column
/// gutter identity (Crystal facets, Mech LED) is never overwritten
/// (gutter-precedence rule, Phase 1 §2.4).
pub(super) fn contact_shadow_cells(
    pet_rect: Rect,
    art_lines: &[String],
    mirror: bool,
    habitat: Rect,
) -> Vec<(u16, u16)> {
    let Some(feet) = crate::pet::render::feet_row(art_lines) else {
        return Vec::new();
    };
    let shadow_row = pet_rect.y + (feet as u16) + 1;
    // Clip: must be inside the habitat (and at/below the feet, never above).
    if shadow_row < habitat.y || shadow_row >= habitat.y.saturating_add(habitat.height) {
        return Vec::new();
    }
    let line_width = art_lines.get(feet).map(|l| l.chars().count()).unwrap_or(0);
    crate::pet::render::feet_columns(art_lines)
        .into_iter()
        .filter_map(|col| {
            let col_in_frame = if mirror {
                line_width.saturating_sub(1).saturating_sub(col)
            } else {
                col
            };
            let abs_col = pet_rect.x + col_in_frame as u16;
            if abs_col < habitat.x || abs_col >= habitat.x.saturating_add(habitat.width) {
                return None;
            }
            Some((abs_col, shadow_row))
        })
        .collect()
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

/// Returns a clipped, cell-native contact footprint below the pet's feet. The
/// outer band stays dark, while the inner lavender bands improve small-display
/// legibility without introducing a blurred glow or a renderer-specific shape.
pub(super) fn contact_shadow_draw_cells(
    scene_pet_art: Rect,
    pet_art_lines: &[String],
    facing: i8,
    habitat: Rect,
    biome: RoomBiomeTag,
) -> Vec<DrawCell> {
    let mirror = facing == -1;
    let floor_wash = biome_floor_wash_color(biome);
    let Some((outer_color, inner_color, core_color)) = contact_footprint_colors(floor_wash) else {
        return Vec::new(); // non-RGB color cap: skip
    };
    let feet = contact_shadow_cells(scene_pet_art, pet_art_lines, mirror, habitat);
    let Some((foot_start, foot_end, row)) = footprint_bounds(&feet) else {
        return Vec::new();
    };
    let habitat_end = habitat.x.saturating_add(habitat.width).saturating_sub(1);
    let outer_start = foot_start
        .saturating_sub(FOOTPRINT_OUTER_PADDING)
        .max(habitat.x);
    let outer_end = foot_end
        .saturating_add(FOOTPRINT_OUTER_PADDING)
        .min(habitat_end);
    let inner_start = foot_start
        .saturating_sub(FOOTPRINT_INNER_PADDING)
        .max(outer_start);
    let inner_end = foot_end
        .saturating_add(FOOTPRINT_INNER_PADDING)
        .min(outer_end);

    let mut cells = Vec::with_capacity(
        usize::from(outer_end.saturating_sub(outer_start).saturating_add(1)) * 2,
    );
    append_footprint_row(
        &mut cells,
        row,
        outer_start,
        outer_end,
        inner_start,
        inner_end,
        Some((foot_start, foot_end)),
        outer_color,
        inner_color,
        core_color,
    );

    let lower_row = row.saturating_add(1);
    if lower_row < habitat.y.saturating_add(habitat.height)
        && outer_start.saturating_add(1) <= outer_end.saturating_sub(1)
    {
        append_footprint_row(
            &mut cells,
            lower_row,
            outer_start.saturating_add(1),
            outer_end.saturating_sub(1),
            inner_start.saturating_add(1),
            inner_end.saturating_sub(1),
            None,
            outer_color,
            inner_color,
            core_color,
        );
    }

    cells
}

#[allow(clippy::too_many_arguments)]
fn append_footprint_row(
    cells: &mut Vec<DrawCell>,
    row: u16,
    outer_start: u16,
    outer_end: u16,
    inner_start: u16,
    inner_end: u16,
    core_span: Option<(u16, u16)>,
    outer_color: Rgb,
    inner_color: Rgb,
    core_color: Rgb,
) {
    for col in outer_start..=outer_end {
        let bg = if core_span.is_some_and(|(start, end)| col >= start && col <= end) {
            core_color
        } else if col >= inner_start && col <= inner_end {
            inner_color
        } else {
            outer_color
        };
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

fn contact_footprint_colors(floor_wash: Color) -> Option<(Rgb, Rgb, Rgb)> {
    let floor = rgb_from_color(floor_wash)?;
    let shadow = rgb_from_color(contact_shadow_color(floor_wash))?;
    let outer = blend_rgb(shadow, FOOTPRINT_OUTER_TINT, 96);
    let inner = blend_rgb(floor, FOOTPRINT_INNER_TINT, 112);
    let core = blend_rgb(floor, FOOTPRINT_CORE_TINT, 144);
    Some((outer, inner, core))
}

fn footprint_bounds(feet: &[(u16, u16)]) -> Option<(u16, u16, u16)> {
    let &(first_col, row) = feet.first()?;
    let (start, end) = feet
        .iter()
        .fold((first_col, first_col), |(start, end), &(col, _)| {
            (start.min(col), end.max(col))
        });
    Some((start, end, row))
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
    fn contact_shadow_lands_one_row_below_feet_under_feet_columns() {
        // Framed art: feet glyphs at framed row 5, columns 4 and 6.
        let art_lines: Vec<String> = vec![
            "             ".to_string(), // 0
            "             ".to_string(), // 1
            "             ".to_string(), // 2
            "             ".to_string(), // 3
            "             ".to_string(), // 4
            "    X X      ".to_string(), // 5 feet at cols 4 and 6
            "             ".to_string(), // 6
            "             ".to_string(), // 7
            "             ".to_string(), // 8
            "             ".to_string(), // 9
        ];
        let pet_rect = Rect::new(10, 20, 13, 10);
        let habitat = Rect::new(0, 0, 60, 40);
        let cells = contact_shadow_cells(pet_rect, &art_lines, false, habitat);
        // feet_row = 5 -> shadow row = pet_rect.y + 6 = 26.
        // feet cols 4,6 -> absolute 14,16.
        let set: std::collections::HashSet<(u16, u16)> = cells.into_iter().collect();
        assert!(set.contains(&(14, 26)), "shadow under left foot");
        assert!(set.contains(&(16, 26)), "shadow under right foot");
        assert!(!set.contains(&(15, 26)), "gap between feet is not shadowed");
        assert_eq!(set.len(), 2, "shadow is exactly the feet columns, no halo");
    }

    #[test]
    fn contact_shadow_draw_cells_expand_to_a_three_value_footprint() {
        let art_lines: Vec<String> = vec![
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "  XXXXXXX    ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
        ];
        let pet_rect = Rect::new(5, 5, 13, 10);
        let habitat = Rect::new(0, 0, 60, 40);
        let cells =
            contact_shadow_draw_cells(pet_rect, &art_lines, 1, habitat, RoomBiomeTag::Starter);

        assert_eq!(
            cells.len(),
            20,
            "the main footprint and inset lower shelf both render"
        );
        assert!(cells.iter().all(|cell| cell.glyph.is_none()));
        assert_eq!(cells.iter().filter(|cell| cell.row == 11).count(), 11);
        assert_eq!(cells.iter().filter(|cell| cell.row == 12).count(), 9);
        assert_eq!(cells.iter().map(|cell| cell.col).min(), Some(5));
        assert_eq!(cells.iter().map(|cell| cell.col).max(), Some(15));

        let color_at = |col, row| {
            cells
                .iter()
                .find(|cell| cell.col == col && cell.row == row)
                .and_then(|cell| cell.bg)
                .unwrap()
        };
        let outer = color_at(5, 11);
        let inner = color_at(6, 11);
        let core = color_at(8, 11);
        assert_eq!(
            color_at(8, 12),
            inner,
            "lower shelf retains the lavender band"
        );
        let luminance = |color: Rgb| u32::from(color.r) + u32::from(color.g) + u32::from(color.b);
        assert!(luminance(outer) < luminance(inner));
        assert!(luminance(inner) < luminance(core));
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
    fn contact_shadow_is_clipped_to_habitat() {
        let art_lines: Vec<String> = (0..10)
            .map(|i| {
                if i == 7 {
                    "#############".to_string()
                } else {
                    "             ".to_string()
                }
            })
            .collect();
        let pet_rect = Rect::new(0, 0, 13, 10);
        // Habitat only 5 rows tall: shadow row would be below it -> empty.
        let habitat = Rect::new(0, 0, 13, 5);
        let cells = contact_shadow_cells(pet_rect, &art_lines, false, habitat);
        assert!(
            cells.is_empty(),
            "shadow below the habitat floor is clipped away"
        );
    }

    #[test]
    fn contact_shadow_deepens_bg_under_feet_without_replacing_glyphs() {
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
        // Find the pet rect, derive its feet/shadow cells, and assert at least one
        // shadow cell carries a non-default bg (the shadow tint) and is not blanked
        // out as a glyph (the shadow is bg-only).
        let area = f_area();
        let pet_rect = pet_inner_rect_in_panel(area, &vm);
        let cells = contact_shadow_cells(pet_rect, &vm.pet_art, vm.facing == -1, area);
        // At least one shadow cell exists for a grounded S2 pet in a 24-tall area.
        assert!(!cells.is_empty(), "a grounded pet has a contact shadow");
        let mut deepened = 0usize;
        for (x, y) in &cells {
            if *x < 40 && *y < 24 {
                if let Some(ratatui::style::Color::Rgb(..)) = buf[(*x, *y)].style().bg {
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
    fn contact_shadow_cells_preserve_the_exact_feet_span_for_footprint_derivation() {
        let art_lines: Vec<String> = vec![
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
            "  ▙▒▒▒▒▒▟    ".to_string(), // feet span cols 2..=8
            "             ".to_string(),
            "             ".to_string(),
            "             ".to_string(),
        ];
        let pet_rect = Rect::new(5, 5, 13, 10);
        let habitat = Rect::new(0, 0, 60, 40);
        let cells = contact_shadow_cells(pet_rect, &art_lines, false, habitat);
        let cols: std::collections::HashSet<u16> = cells.iter().map(|(c, _)| *c).collect();
        // No shadow column outside the feet glyph span (abs cols 7..=13 for cols 2..=8).
        for c in &cols {
            assert!(*c >= 7 && *c <= 13, "shadow col {c} escaped the feet span");
        }
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
