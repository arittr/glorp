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

/// Fraction of the safe drift radius to use in X.  The safe radius is
/// `(grid_cols / 2).saturating_sub(PET_W)` cells from center, so
/// `DRIFT_X_FRAC = 0.45` keeps the pet well clear of the circle edge.
/// Increase toward 1.0 for wider motion; decrease for a calmer look.
const DRIFT_X_FRAC: f32 = 0.45;

/// Fraction of the safe drift radius to use in Y. The safe radius is
/// `(grid_rows / 2).saturating_sub(PET_H)` cells from center. Cells are
/// taller than wide (~2:1), so a smaller Y fraction keeps the pet in
/// the widest circular band.  0.30 keeps it within the widest third.
const DRIFT_Y_FRAC: f32 = 0.30;

/// Drift cadence: target changes every this many seconds.  The pet eases
/// smoothly between targets, so shorter means livelier; longer means calmer.
const DRIFT_PERIOD_SECS: u64 = 20;

// ─────────────────────────────────────────────────────────────────────────────

/// Compute a gentle, deterministic 2D drift position for the pet.
///
/// Returns `(x, y)` — the top-left of the `PET_W × PET_H` pet art rect in
/// grid coordinates — so the whole pet body stays inside the inscribed circle.
///
/// The approach:
/// - Time is divided into `DRIFT_PERIOD_SECS`-second epochs.
/// - Each epoch has a deterministic target (derived from the epoch index).
/// - The previous epoch also has a deterministic target.
/// - Position is the linear interpolation (ease) between them based on
///   progress through the current epoch — giving smooth, predictable motion.
/// - Bounding: the pet CENTER is constrained to an ellipse with semi-axes
///   `x_radius = safe_x * DRIFT_X_FRAC` and `y_radius = safe_y * DRIFT_Y_FRAC`
///   around the grid center, where `safe_x = grid_cols/2 - PET_W/2` and
///   `safe_y = grid_rows/2 - PET_H/2`.  Tune `DRIFT_X_FRAC`/`DRIFT_Y_FRAC`
///   to adjust range.
fn companion_drift(now: time::OffsetDateTime, grid_cols: u16, grid_rows: u16) -> (u16, u16) {
    let unix = now.unix_timestamp() as u64;
    let epoch = unix / DRIFT_PERIOD_SECS;
    let phase = (unix % DRIFT_PERIOD_SECS) as f32 / DRIFT_PERIOD_SECS as f32;

    // Grid center and safe radii (integer).
    let cx = grid_cols / 2;
    let cy = grid_rows / 2;
    // Half the pet size to keep art fully inside the porthole.
    let half_w = PET_W / 2;
    let half_h = PET_H / 2;
    let safe_x = cx.saturating_sub(half_w) as f32;
    let safe_y = cy.saturating_sub(half_h) as f32;
    let x_radius = safe_x * DRIFT_X_FRAC;
    let y_radius = safe_y * DRIFT_Y_FRAC;

    // Deterministic target for a given epoch, in the range [-1.0, 1.0] per axis.
    let target_for_epoch = |e: u64| -> (f32, f32) {
        // Use a simple hash of the epoch to get pseudo-random unit offsets.
        let h1 = e
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(0x6c62_272e_07bb_0142);
        let h2 = h1
            .wrapping_mul(0x517c_c1b7_2722_0a95)
            .wrapping_add(0xbf87_8c2f_a7a4_c6a5);
        let nx = ((h1 >> 32) as i32 as f32) / (i32::MAX as f32);
        let ny = ((h2 >> 32) as i32 as f32) / (i32::MAX as f32);
        (nx, ny)
    };

    let (px, py) = target_for_epoch(epoch.saturating_sub(1));
    let (nx, ny) = target_for_epoch(epoch);

    // Ease-in-out (smoothstep) between targets — mirrors the watch's wander
    // curve so the pet accelerates off a spot and decelerates into the next,
    // reading as deliberate rather than a constant linear crawl.
    let t = phase * phase * (3.0 - 2.0 * phase);
    let fx = px + (nx - px) * t;
    let fy = py + (ny - py) * t;

    // Convert normalized [-1,1] to grid-space, centered at (cx, cy).
    // pet_art top-left = center - half_pet_size + offset
    let art_x = cx as i32 - half_w as i32 + (fx * x_radius) as i32;
    let art_y = cy as i32 - half_h as i32 + (fy * y_radius) as i32;

    let art_x = art_x.clamp(0, (grid_cols.saturating_sub(PET_W)) as i32) as u16;
    let art_y = art_y.clamp(0, (grid_rows.saturating_sub(PET_H)) as i32) as u16;

    (art_x, art_y)
}

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
/// The pet drifts freely in 2D within the porthole (aquarium feel — no floor or
/// ground line). `area` fills the entire grid so the background wash covers the
/// whole circle. The pet position is driven by `companion_drift`, which eases
/// between deterministic 2D targets every `DRIFT_PERIOD_SECS` seconds, keeping
/// the pet body within the safe central ellipse at all times.
///
/// Tune `DRIFT_X_FRAC`, `DRIFT_Y_FRAC`, and `DRIFT_PERIOD_SECS` for feel.
pub fn build_round_scene_draw_list(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
) -> SceneDrawList {
    // Full-grid area: the bg wash + ambient cover the whole porthole.
    let area = Rect::new(0, 0, grid_cols, grid_rows);

    // Resolve horizontal facing from the live clock and narrowed wander width,
    // mirroring what PetPanel::render does but with companion-specific range.
    let wander_width = PET_W + 2 * COMPANION_WANDER_HALF;
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

    // Compute layout, then override pet_art with the drift position.
    let mut layout = PetScene::compute_layout(area, vm, &ctx);
    let old_pet_art = layout.pet_art;
    let (drift_x, drift_y) = companion_drift(now, grid_cols, grid_rows);
    let new_pet_art = Rect::new(drift_x, drift_y, PET_W, PET_H);
    layout.pet_art = new_pet_art;
    // Update exclusions: replace the old pet_art entry with the drifted one so
    // ambient glyphs avoid the pet's actual rendered position.
    for excl in &mut layout.exclusions {
        if *excl == old_pet_art {
            *excl = new_pet_art;
            break;
        }
    }

    let model = PetSceneModel::build(vm, now, ColorCapability::Truecolor);
    let mut scene_list =
        crate::tui::panels::pet::render_pet_to_draw_list(&model, vm, &layout, now, &ctx);

    // Uniform porthole recolor: find the sky-wash color from the first bg-only
    // cell at a low row, then stamp it onto every bg-only cell so the floor band
    // and contact-shadow darker rows disappear — leaving a uniform porthole bg.
    // Glyph cells (pet body, ambient, room) are never touched.
    let sky_bg = scene_list
        .cells
        .iter()
        .find(|c| c.glyph.is_none() && c.bg.is_some() && c.row < grid_rows / 2)
        .and_then(|c| c.bg);
    if let Some(sky) = sky_bg {
        for cell in &mut scene_list.cells {
            if cell.glyph.is_none() && cell.bg.is_some() {
                cell.bg = Some(sky);
            }
        }
    }

    scene_list
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
