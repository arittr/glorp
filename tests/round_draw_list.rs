/// Content-lock golden test for `build_round_scene_draw_list`.
///
/// This test locks the SCENE CONTENT the companion will blit — not AppKit pixels.
/// It uses `insta` to snapshot the serialized cell list for a fixed fixture +
/// fixed clock + fixed grid, proving that the round scene content is stable
/// across refactors.
///
/// The grid (44 × 18) is the same canonical size used in the unit tests in
/// `src/round/scene.rs`.  The clock is pinned to 2026-06-13 18:00 UTC.
use glorp::presentation::DrawCell;
use glorp::round::scene::build_round_scene_draw_list;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

/// A compact, human-readable serialisation of a `DrawCell` for snapshot comparison.
#[derive(Debug, serde::Serialize)]
struct CellRecord {
    row: u16,
    col: u16,
    glyph: Option<String>,
    fg: Option<(u8, u8, u8)>,
    bg: Option<(u8, u8, u8)>,
    bold: bool,
}

impl From<&DrawCell> for CellRecord {
    fn from(c: &DrawCell) -> Self {
        Self {
            row: c.row,
            col: c.col,
            glyph: c.glyph.clone(),
            fg: c.fg.map(|rgb| (rgb.r, rgb.g, rgb.b)),
            bg: c.bg.map(|rgb| (rgb.r, rgb.g, rgb.b)),
            bold: c.bold,
        }
    }
}

const GOLDEN_GRID_COLS: u16 = 44;
const GOLDEN_GRID_ROWS: u16 = 18;
const GOLDEN_NOW: time::OffsetDateTime = datetime!(2026-06-13 18:00 UTC);

#[test]
fn round_scene_draw_list_content_lock() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let list = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS);

    // Summarise the draw list to a stable, human-readable form.
    // We snapshot total cell count + a few structural properties so the golden
    // is readable in review.md without requiring serde on every internal type.
    let records: Vec<CellRecord> = list.cells.iter().map(CellRecord::from).collect();

    // Cell count is part of the golden — any net addition or removal of draw
    // cells will cause this snapshot to diverge and require a conscious review.
    let cell_count = records.len();

    // Snapshot a digest: total count + first 20 cells + last 5 cells.
    // Full serialisation would be large and noisy; this slice is enough to
    // catch regressions in cell ordering and content without a multi-KB YAML.
    let head: Vec<_> = records.iter().take(20).collect();
    let tail: Vec<_> = records.iter().rev().take(5).rev().collect();

    insta::assert_yaml_snapshot!("round_draw_list_cell_count", cell_count);
    insta::assert_yaml_snapshot!("round_draw_list_head", head);
    insta::assert_yaml_snapshot!("round_draw_list_tail", tail);
}
