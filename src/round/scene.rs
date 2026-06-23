use std::borrow::Cow;

use ratatui::layout::Rect;

use crate::presentation::{PetSceneModel, SceneDrawList};
use crate::tui::component::PetScene;
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

// ── Companion tuning constants ────────────────────────────────────────────────
// Adjust these to change the round companion's feel without touching any
// shared watch code.

/// Pet art width (must match `PET_W` in `src/tui/panels/pet.rs`).
const PET_W: u16 = 13;
/// Pet art height (must match `PET_H` in `src/tui/panels/pet.rs`).
const PET_H: u16 = 10;

/// Half-width of the pet's wander range in the companion, in cells.
/// The pet wanders ±this many cells around center. Increase for more motion,
/// decrease to keep the pet near the circle's widest band.
/// At a typical 44-col companion grid this is ~18 % of the available
/// horizontal room, which keeps the pet well inside the circle's chords.
const COMPANION_WANDER_HALF: u16 = 8;

/// How many rows to drop the floor below the circle's equator, in cells.
/// The pet grounds on the floor, so a larger value lowers both the floor line
/// and the pet, leaving more sky above and shrinking the dark band at the
/// bottom of the circle. 0 puts the floor at the equator.
const COMPANION_FLOOR_DROP: u16 = 4;

// ─────────────────────────────────────────────────────────────────────────────

/// Build a [`SceneDrawList`] for the round companion viewport.
///
/// This function is **pure** — it has no side effects, no AppKit calls, and no
/// platform-specific imports. The result is fully deterministic for a fixed
/// `(vm, now, grid_cols, grid_rows)` triple, which makes it safe to golden-test
/// on any platform (CI, Linux, non-macOS).
///
/// Grid sizing is a caller concern. On macOS the companion measures an `"M"` via
/// `NSFont::monospacedSystemFontOfSize_weight` to derive `cell_w`/`cell_h` and
/// then computes `grid_cols = floor(view_w / cell_w)`,
/// `grid_rows = floor(view_h / cell_h)`. That measurement is done in
/// `companion_grid_metrics` in `src/companion/app.rs`, which is macOS-only.
///
/// # Layout contract
///
/// `area` is cropped to the upper half of the grid so the pet's body center
/// lands on the circle's vertical equator — the widest visible band. Specifically:
/// `area_height = (grid_rows / 2 + PET_H / 2 + COMPANION_FLOOR_DROP).min(grid_rows)`,
/// which places the pet's feet a little below `grid_rows / 2` and the body
/// spanning upward. The band below the floor shows the background base color
/// (tune `COMPANION_FLOOR_DROP` to lower the floor / extend the habitat down).
///
/// Wander is narrowed to `PET_W + 2 * COMPANION_WANDER_HALF` so the pet stays
/// near center instead of drifting to the square window's edges (which fall
/// outside the circle's narrow chords).
pub fn build_round_scene_draw_list(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
) -> SceneDrawList {
    // Crop to the upper circle: pet body centers on the equator; the lower half
    // shows the background. Tune `COMPANION_WANDER_HALF` and `area_height` to
    // adjust the look.
    let area_height = (grid_rows / 2 + PET_H / 2 + COMPANION_FLOOR_DROP).min(grid_rows);
    let area = Rect::new(0, 0, grid_cols, area_height);

    // Narrow the wander range so the pet drifts gently around center rather
    // than roaming the full square width (which puts it outside the circle's
    // chords). The effective habitat_width fed to `resolve_wander_offset` is
    // PET_W + 2 * COMPANION_WANDER_HALF; `area.width` is still the full
    // grid_cols so horizontal centering is true-center.
    let wander_width = PET_W + 2 * COMPANION_WANDER_HALF;

    // Resolve wander offset + facing from the live clock and narrowed width,
    // mirroring what `PetPanel::render` does (src/tui/panels/pet.rs:141-158)
    // but with a companion-specific wander range.
    let (wx, fc) = crate::tui::wander::resolve_wander_offset(vm, now, wander_width);
    let vm: Cow<WatchViewModel> = if wx != vm.wander_offset_x || fc != vm.facing {
        Cow::Owned({
            let mut v = vm.clone();
            v.wander_offset_x = wx;
            v.facing = fc;
            v
        })
    } else {
        Cow::Borrowed(vm)
    };
    let vm = vm.as_ref();

    // Fixed clock so the render context is deterministic when `now` is pinned.
    let ctx = RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(now));

    let layout = PetScene::compute_layout(area, vm, &ctx);
    let model = PetSceneModel::build(vm, now, ColorCapability::Truecolor);
    crate::tui::panels::pet::render_pet_to_draw_list(&model, vm, &layout, now, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    /// Canonical pinned fixture for the content-lock golden.
    /// Grid: 44 cols × 18 rows — wide enough that the 13-wide pet (PET_W=13)
    /// is roughly 30 % of the width, matching the round companion's visual target.
    const GOLDEN_GRID_COLS: u16 = 44;
    const GOLDEN_GRID_ROWS: u16 = 18;
    const GOLDEN_NOW: time::OffsetDateTime = datetime!(2026-06-13 18:00 UTC);

    #[test]
    fn build_round_scene_draw_list_is_deterministic() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let a = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS);
        let b = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS);
        assert_eq!(
            a.cells, b.cells,
            "build_round_scene_draw_list must be deterministic for fixed (vm, now, grid)"
        );
    }

    #[test]
    fn build_round_scene_draw_list_produces_nonempty_cells() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let list = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS);
        assert!(
            !list.cells.is_empty(),
            "expected non-empty draw list for a standard fixture at 44×18"
        );
    }

    #[test]
    fn build_round_scene_draw_list_cells_within_grid_bounds() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let list = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS);
        for cell in &list.cells {
            assert!(
                cell.col < GOLDEN_GRID_COLS,
                "cell col {} out of bounds (grid_cols={})",
                cell.col,
                GOLDEN_GRID_COLS,
            );
            assert!(
                cell.row < GOLDEN_GRID_ROWS,
                "cell row {} out of bounds (grid_rows={})",
                cell.row,
                GOLDEN_GRID_ROWS,
            );
        }
    }

    #[test]
    fn build_round_scene_draw_list_includes_pet_body_cells() {
        // Pet body cells are glyph-only (no bg, bold flag from eye role). Verify
        // that at least some non-blank glyphs are present — proves the pet renders.
        let vm = WatchViewModel::fixture_with_habitat_props();
        let list = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS);
        let pet_cells = list
            .cells
            .iter()
            .filter(|c| c.glyph.as_deref().map(|g| g != " ").unwrap_or(false))
            .count();
        assert!(
            pet_cells >= 10,
            "expected at least 10 non-blank pet glyph cells, got {pet_cells}"
        );
    }
}
