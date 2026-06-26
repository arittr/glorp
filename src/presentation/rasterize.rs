/// A resolved terminal cell for blitting to a text surface.
///
/// Produced by [`rasterize`] from a [`super::SceneDrawList`]. Carries a flat
/// glyph + optional colors + bold flag so callers can map straight to
/// `NSAttributedString` runs, HTML spans, or any other rich-text format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterCell {
    pub glyph: char,
    pub fg: Option<crate::pet::palette::Rgb>,
    pub bg: Option<crate::pet::palette::Rgb>,
    pub bold: bool,
}

impl RasterCell {
    fn blank() -> Self {
        Self {
            glyph: ' ',
            fg: None,
            bg: None,
            bold: false,
        }
    }
}

/// Rasterize a [`super::SceneDrawList`] into a dense `rows × cols` grid.
///
/// Each cell starts as `{ glyph: ' ', fg: None, bg: None, bold: false }`.
/// Draw cells are applied in list order (later entries overwrite earlier —
/// same z-order as `blit_draw_list`):
///
/// - A cell with `glyph: Some(s)` sets `glyph` to the first char of `s` and
///   updates `fg` / `bold`. The cell's existing `bg` is **left intact** when
///   `draw_cell.bg` is `None` — this preserves a biome-wash background that was
///   written by an earlier bg-only cell (the sparse-pet contract).
/// - A cell with `glyph: None` only sets `bg` when `draw_cell.bg` is `Some`.
/// - Cells whose `(col, row)` falls outside `[0, cols) × [0, rows)` are skipped.
///
/// This function is pure and has no AppKit or platform-specific imports. It is
/// intentionally separate from the macOS-gated render code so it can be unit-
/// tested on any platform.
pub fn rasterize(list: &super::SceneDrawList, cols: u16, rows: u16) -> Vec<Vec<RasterCell>> {
    let mut grid: Vec<Vec<RasterCell>> = (0..rows)
        .map(|_| (0..cols).map(|_| RasterCell::blank()).collect())
        .collect();

    for draw_cell in &list.cells {
        if draw_cell.row >= rows || draw_cell.col >= cols {
            continue;
        }
        let cell = &mut grid[draw_cell.row as usize][draw_cell.col as usize];

        if let Some(ref glyph_str) = draw_cell.glyph {
            // Glyph cell: set glyph + fg + bold. Leave bg intact if None.
            if let Some(ch) = glyph_str.chars().next() {
                cell.glyph = ch;
            }
            if let Some(fg) = draw_cell.fg {
                cell.fg = Some(fg);
            }
            cell.bold = draw_cell.bold;
            // bg: only update if the draw cell carries one
            if let Some(bg) = draw_cell.bg {
                cell.bg = Some(bg);
            }
            // else: preserve whatever bg was already set (sparse-pet contract)
        } else {
            // Bg-only cell: only set bg.
            if let Some(bg) = draw_cell.bg {
                cell.bg = Some(bg);
            }
        }
    }

    grid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::palette::Rgb;
    use crate::presentation::{DrawCell, SceneDrawList};

    fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb::new(r, g, b)
    }

    fn bg_cell(row: u16, col: u16, bg: Rgb) -> DrawCell {
        DrawCell {
            row,
            col,
            glyph: None,
            fg: None,
            bg: Some(bg),
            bold: false,
        }
    }

    fn glyph_cell(row: u16, col: u16, g: &str, fg: Rgb) -> DrawCell {
        DrawCell {
            row,
            col,
            glyph: Some(g.to_string()),
            fg: Some(fg),
            bg: None,
            bold: false,
        }
    }

    // (a) bg-only cell then glyph-only cell at same coord → glyph + fg AND prior bg preserved
    #[test]
    fn sparse_bg_preserved_under_glyph() {
        let list = SceneDrawList {
            cells: vec![
                bg_cell(0, 1, rgb(10, 20, 30)),
                glyph_cell(0, 1, "X", rgb(200, 100, 50)),
            ],
        };
        let grid = rasterize(&list, 5, 3);
        let cell = &grid[0][1];
        assert_eq!(cell.glyph, 'X', "glyph must be written");
        assert_eq!(cell.fg, Some(rgb(200, 100, 50)), "fg must be written");
        assert_eq!(
            cell.bg,
            Some(rgb(10, 20, 30)),
            "prior bg must be preserved under the later glyph cell"
        );
    }

    // (b) out-of-bounds cell is skipped; grid remains blank
    #[test]
    fn out_of_bounds_cell_skipped() {
        let list = SceneDrawList {
            cells: vec![DrawCell {
                row: 99,
                col: 99,
                glyph: Some("!".to_string()),
                fg: Some(rgb(255, 0, 0)),
                bg: Some(rgb(0, 0, 0)),
                bold: true,
            }],
        };
        let grid = rasterize(&list, 4, 4);
        for row in &grid {
            for cell in row {
                assert_eq!(*cell, RasterCell::blank(), "no cell should be modified");
            }
        }
    }

    // (c) z-order: two glyph cells at the same coord → the later one wins
    #[test]
    fn z_order_later_glyph_wins() {
        let list = SceneDrawList {
            cells: vec![
                glyph_cell(1, 2, "A", rgb(10, 10, 10)),
                glyph_cell(1, 2, "B", rgb(20, 20, 20)),
            ],
        };
        let grid = rasterize(&list, 5, 5);
        let cell = &grid[1][2];
        assert_eq!(cell.glyph, 'B', "later glyph must win");
        assert_eq!(cell.fg, Some(rgb(20, 20, 20)), "later fg must win");
    }

    // (d) deterministic content lock: build_round_scene_draw_list → rasterize
    //     Asserts glyph rows and spot fg/bg values for a fixed (vm, now, 20×10).
    #[test]
    fn rasterize_content_lock_round_scene() {
        use crate::round::scene::build_round_scene_draw_list;
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;

        const COLS: u16 = 20;
        const ROWS: u16 = 10;
        const NOW: time::OffsetDateTime = datetime!(2026-06-13 18:00 UTC);

        let vm = WatchViewModel::fixture();
        let m = crate::round::scene::CompanionMotion::default();
        let scene = build_round_scene_draw_list(&vm, NOW, COLS, ROWS, &m);
        let grid = rasterize(&scene.draw_list, COLS, ROWS);

        // Basic structural guarantees
        assert_eq!(grid.len(), ROWS as usize, "grid must have ROWS rows");
        assert_eq!(
            grid[0].len(),
            COLS as usize,
            "each row must have COLS cells"
        );

        // At least one non-blank glyph somewhere in the grid (pet rendered)
        let non_blank: usize = grid
            .iter()
            .flat_map(|row| row.iter())
            .filter(|c| c.glyph != ' ')
            .count();
        assert!(
            non_blank >= 5,
            "expected ≥5 non-blank pet glyphs, got {non_blank}"
        );

        // Snapshot the glyph content of each row as a string for a deterministic lock.
        // These strings lock the rasterize output for the canonical fixture.
        let glyph_rows: Vec<String> = grid
            .iter()
            .map(|row| row.iter().map(|c| c.glyph).collect())
            .collect();

        // Pin the exact glyph grid for the locked (vm, now, 20×10) triple.
        // If this assertion fails, review the scene change and update the snapshot.
        let joined = glyph_rows.join("\n");

        // Use insta for the snapshot so it auto-creates on first run and diffs cleanly.
        insta::assert_snapshot!("rasterize_content_lock_glyph_grid", joined);

        // Spot-check: any cell with a non-None fg must have a valid non-zero RGB
        for row in &grid {
            for cell in row {
                if let Some(fg) = cell.fg {
                    assert!(
                        fg.r > 0 || fg.g > 0 || fg.b > 0,
                        "fg color should not be pure black for pet cells"
                    );
                }
            }
        }
    }
}
