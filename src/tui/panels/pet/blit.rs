use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

use crate::presentation::SceneDrawList;

/// Write every cell in `list` to `buf`, in order. Each field is applied
/// independently — `None` means "leave the existing value intact":
///
/// - `glyph: Some(s)` → `cell.set_symbol(s)` (glyph changed)
/// - `glyph: None`    → existing symbol untouched
/// - `fg: Some(rgb)`  → `cell.set_fg(Color::Rgb(..))`
/// - `fg: None`       → existing fg untouched
/// - `bg: Some(rgb)`  → `cell.set_bg(Color::Rgb(..))`
/// - `bg: None`       → **existing bg is NOT touched** (sparse-pet contract)
/// - `bold: true`     → insert `Modifier::BOLD`
/// - `bold: false`    → modifiers untouched
///
/// `cell.reset()` is NEVER called.  Cells whose (col, row) falls outside
/// `buf.area` are skipped silently.
pub(crate) fn blit_draw_list(buf: &mut Buffer, list: &SceneDrawList) {
    // Copy out the area before taking any mutable borrow on buf.
    let area = *buf.area();
    for c in &list.cells {
        // Bounds check: skip cells outside the buffer area.
        if c.col < area.x
            || c.col >= area.x.saturating_add(area.width)
            || c.row < area.y
            || c.row >= area.y.saturating_add(area.height)
        {
            continue;
        }
        let cell = &mut buf[(c.col, c.row)];
        if let Some(g) = &c.glyph {
            cell.set_symbol(g);
        }
        if let Some(fg) = c.fg {
            cell.set_fg(Color::Rgb(fg.r, fg.g, fg.b));
        }
        if let Some(bg) = c.bg {
            cell.set_bg(Color::Rgb(bg.r, bg.g, bg.b));
        }
        if c.bold {
            cell.modifier.insert(Modifier::BOLD);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::palette::Rgb;
    use crate::presentation::DrawCell;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    fn make_buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    // ── (a) glyph+fg cell with bg:None must LEAVE existing bg intact ──────────
    #[test]
    fn blit_glyph_only_cell_preserves_existing_bg() {
        let mut buf = make_buf(5, 3);
        // Pre-seed the target cell with a known bg.
        let preset_bg = Color::Rgb(100, 150, 200);
        buf[(2, 1)].set_bg(preset_bg);

        let list = SceneDrawList {
            cells: vec![DrawCell {
                row: 1,
                col: 2,
                glyph: Some("x".to_string()),
                fg: Some(Rgb::new(255, 0, 0)),
                bg: None, // <-- do NOT touch the bg
                bold: false,
            }],
        };
        blit_draw_list(&mut buf, &list);

        let cell = &buf[(2, 1)];
        assert_eq!(cell.symbol(), "x", "glyph must be written");
        assert_eq!(
            cell.style().fg,
            Some(Color::Rgb(255, 0, 0)),
            "fg must be written"
        );
        assert_eq!(
            cell.style().bg,
            Some(preset_bg),
            "existing bg must survive — sparse-pet contract"
        );
    }

    // ── (b) bg-only cell sets only bg ─────────────────────────────────────────
    #[test]
    fn blit_bg_only_cell_sets_bg_and_leaves_symbol_and_fg() {
        let mut buf = make_buf(5, 3);
        // Pre-set symbol and fg on the cell.
        buf[(1, 0)].set_symbol("Z");
        buf[(1, 0)].set_fg(Color::Rgb(10, 20, 30));

        let list = SceneDrawList {
            cells: vec![DrawCell {
                row: 0,
                col: 1,
                glyph: None,
                fg: None,
                bg: Some(Rgb::new(40, 80, 120)),
                bold: false,
            }],
        };
        blit_draw_list(&mut buf, &list);

        let cell = &buf[(1, 0)];
        assert_eq!(cell.symbol(), "Z", "symbol must be untouched");
        assert_eq!(
            cell.style().fg,
            Some(Color::Rgb(10, 20, 30)),
            "fg must be untouched"
        );
        assert_eq!(
            cell.style().bg,
            Some(Color::Rgb(40, 80, 120)),
            "bg must be written"
        );
    }

    // ── (c) bold:true inserts Modifier::BOLD ─────────────────────────────────
    #[test]
    fn blit_bold_cell_inserts_modifier() {
        let mut buf = make_buf(3, 3);

        let list = SceneDrawList {
            cells: vec![DrawCell {
                row: 0,
                col: 0,
                glyph: Some("o".to_string()),
                fg: Some(Rgb::new(130, 188, 131)),
                bg: None,
                bold: true,
            }],
        };
        blit_draw_list(&mut buf, &list);

        let cell = &buf[(0, 0)];
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "bold:true must insert Modifier::BOLD"
        );
    }

    // ── (d) out-of-bounds cells are silently skipped ──────────────────────────
    #[test]
    fn blit_skips_cells_outside_buf_area() {
        let mut buf = make_buf(4, 4);

        let list = SceneDrawList {
            cells: vec![DrawCell {
                row: 99,
                col: 99,
                glyph: Some("!".to_string()),
                fg: Some(Rgb::new(255, 255, 255)),
                bg: Some(Rgb::new(0, 0, 0)),
                bold: false,
            }],
        };
        // Must not panic; buf should be untouched.
        blit_draw_list(&mut buf, &list);

        for y in 0..4u16 {
            for x in 0..4u16 {
                assert_eq!(buf[(x, y)].symbol(), " ", "buffer must be untouched");
            }
        }
    }
}
