use std::borrow::Cow;

use ratatui::layout::Rect;

use crate::presentation::{PetSceneModel, SceneDrawList};
use crate::tui::component::PetScene;
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

/// The companion's rendered scene: the draw list plus the pet's drift rect (in
/// grid cells), which the AppKit layer turns into a pixel center for the aura.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionScene {
    pub draw_list: SceneDrawList,
    pub pet_rect: Rect,
}

/// Pet art width (must match `PET_W` in `src/tui/panels/pet.rs`).
const PET_W: u16 = 13;
/// Pet art height (must match `PET_H` in `src/tui/panels/pet.rs`).
const PET_H: u16 = 10;

/// Companion motion config. Defaults reproduce the historical drift exactly, so
/// the shared menubar / preview / goldens are byte-identical; only the companion
/// call site passes tuned values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionMotion {
    /// Half-width of the pet's wander range, in cells (`PET_W + 2*wander_half`).
    pub wander_half: u16,
    /// Fraction of the safe horizontal radius used for drift. Keep modest
    /// (~0.45); higher values clip the pixel rim on a smaller grid.
    pub drift_x_frac: f32,
    /// Fraction of the safe vertical radius used for drift. Cells are ~2:1, so
    /// vertical headroom is tiny — keep gentle.
    pub drift_y_frac: f32,
    /// Drift cadence: the target changes every this many seconds.
    pub drift_period_secs: u64,
    /// Fraction of the safe vertical radius to shift the roam center UP, reserving
    /// the bottom band for the stat. 0.0 = centered.
    pub upward_bias: f32,
}

impl Default for CompanionMotion {
    fn default() -> Self {
        Self {
            wander_half: 8,
            drift_x_frac: 0.45,
            drift_y_frac: 0.30,
            drift_period_secs: 20,
            upward_bias: 0.0,
        }
    }
}

/// Deterministic normalized drift offsets in [-1, 1] per axis for `now`, eased
/// (smoothstep) between per-epoch targets.
fn companion_drift_offsets(now: time::OffsetDateTime, period_secs: u64) -> (f32, f32) {
    let unix = now.unix_timestamp() as u64;
    let period = period_secs.max(1);
    let epoch = unix / period;
    let phase = (unix % period) as f32 / period as f32;

    let target_for_epoch = |e: u64| -> (f32, f32) {
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
    let t = phase * phase * (3.0 - 2.0 * phase);
    (px + (nx - px) * t, py + (ny - py) * t)
}

/// Map normalized offsets `(fx, fy)` to the pet art's top-left grid cell, applying
/// the motion config's radii, upward bias, and the rectangular grid clamp.
fn companion_drift_position(
    motion: &CompanionMotion,
    grid_cols: u16,
    grid_rows: u16,
    fx: f32,
    fy: f32,
) -> (u16, u16) {
    let cx = grid_cols / 2;
    let cy = grid_rows / 2;
    let half_w = PET_W / 2;
    let half_h = PET_H / 2;
    let safe_x = cx.saturating_sub(half_w) as f32;
    let safe_y = cy.saturating_sub(half_h) as f32;
    let x_radius = safe_x * motion.drift_x_frac;
    let y_radius = safe_y * motion.drift_y_frac;
    let bias = motion.upward_bias * safe_y;

    let art_x = cx as i32 - half_w as i32 + (fx * x_radius) as i32;
    let art_y = cy as i32 - half_h as i32 - bias as i32 + (fy * y_radius) as i32;

    let art_x = art_x.clamp(0, (grid_cols.saturating_sub(PET_W)) as i32) as u16;
    let art_y = art_y.clamp(0, (grid_rows.saturating_sub(PET_H)) as i32) as u16;
    (art_x, art_y)
}

/// Gentle, deterministic 2D drift for the pet — top-left of the `PET_W × PET_H`
/// art rect in grid coords. The reachable set is a BOX (independent X/Y hashes),
/// not an ellipse: callers needing a bound must sample box corners.
fn companion_drift(
    now: time::OffsetDateTime,
    motion: &CompanionMotion,
    grid_cols: u16,
    grid_rows: u16,
) -> (u16, u16) {
    let (fx, fy) = companion_drift_offsets(now, motion.drift_period_secs);
    companion_drift_position(motion, grid_cols, grid_rows, fx, fy)
}

/// Conservative bounded-drift check. Samples the drift at every box corner
/// (`fx, fy ∈ {-1, 0, 1}`), maps each of the pet rect's four corners from cell
/// space to pixels (using the real, non-square `cell_w`/`cell_h`), and verifies
/// they all sit inside the pixel aperture circle. The grid is centered in the
/// view, so a corner's pixel distance from the aperture center is
/// `sqrt((cell_w·(col − cols/2))² + (cell_h·(row − rows/2))²)`.
pub fn drift_keeps_pet_in_aperture(
    motion: &CompanionMotion,
    grid_cols: u16,
    grid_rows: u16,
    cell_w: f64,
    cell_h: f64,
    aperture_radius_px: f64,
) -> bool {
    let cxg = grid_cols as f64 / 2.0;
    let cyg = grid_rows as f64 / 2.0;
    for &fx in &[-1.0f32, 0.0, 1.0] {
        for &fy in &[-1.0f32, 0.0, 1.0] {
            let (ax, ay) = companion_drift_position(motion, grid_cols, grid_rows, fx, fy);
            let corners = [
                (ax, ay),
                (ax + PET_W, ay),
                (ax, ay + PET_H),
                (ax + PET_W, ay + PET_H),
            ];
            for (col, row) in corners {
                let dx = cell_w * (col as f64 - cxg);
                let dy = cell_h * (row as f64 - cyg);
                if (dx * dx + dy * dy).sqrt() > aperture_radius_px {
                    return false;
                }
            }
        }
    }
    true
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
    motion: &CompanionMotion,
) -> CompanionScene {
    // Full-grid area: the bg wash + ambient cover the whole porthole.
    let area = Rect::new(0, 0, grid_cols, grid_rows);

    // Resolve horizontal facing from the live clock and narrowed wander width,
    // mirroring what PetPanel::render does but with companion-specific range.
    let wander_width = PET_W + 2 * motion.wander_half;
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
    let (drift_x, drift_y) = companion_drift(now, motion, grid_cols, grid_rows);
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

    CompanionScene {
        draw_list: scene_list,
        pet_rect: new_pet_art,
    }
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
        let m = CompanionMotion::default();
        let a =
            build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &m);
        let b =
            build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &m);
        assert_eq!(
            a.draw_list.cells, b.draw_list.cells,
            "build_round_scene_draw_list must be deterministic for fixed (vm, now, grid)"
        );
    }

    #[test]
    fn build_round_scene_draw_list_produces_nonempty_cells() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let m = CompanionMotion::default();
        let list =
            build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &m);
        assert!(
            !list.draw_list.cells.is_empty(),
            "expected non-empty draw list for a standard fixture at 44×18"
        );
    }

    #[test]
    fn build_round_scene_draw_list_cells_within_grid_bounds() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let m = CompanionMotion::default();
        let list =
            build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &m);
        for cell in &list.draw_list.cells {
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
        let m = CompanionMotion::default();
        let list =
            build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &m);
        let pet_cells = list
            .draw_list
            .cells
            .iter()
            .filter(|c| c.glyph.as_deref().map(|g| g != " ").unwrap_or(false))
            .count();
        assert!(
            pet_cells >= 10,
            "expected at least 10 non-blank pet glyph cells, got {pet_cells}"
        );
    }

    #[test]
    fn companion_motion_default_matches_legacy_drift_values() {
        let m = CompanionMotion::default();
        assert_eq!(m.wander_half, 8);
        assert_eq!(m.drift_x_frac, 0.45);
        assert_eq!(m.drift_y_frac, 0.30);
        assert_eq!(m.drift_period_secs, 20);
        assert_eq!(m.upward_bias, 0.0);
    }

    #[test]
    fn upward_bias_lifts_the_pet() {
        // With a positive upward bias the pet's top-left row is <= the unbiased row
        // for the same normalized offset (smaller row = higher on screen).
        let base = CompanionMotion::default();
        let biased = CompanionMotion {
            upward_bias: 0.5,
            ..CompanionMotion::default()
        };
        let (_, y0) = companion_drift_position(&base, 32, 16, 0.0, 0.0);
        let (_, y1) = companion_drift_position(&biased, 32, 16, 0.0, 0.0);
        assert!(
            y1 <= y0,
            "upward bias should not move the pet down (y1={y1}, y0={y0})"
        );
    }

    #[test]
    fn default_motion_keeps_pet_inside_a_960_aperture() {
        // Representative production metrics: 960px square face, 32 cols → cell_w=30,
        // cells ~2:1 → cell_h=60, rows=16, aperture radius = 960/2 - 1 = 479.
        let m = CompanionMotion::default();
        assert!(
            drift_keeps_pet_in_aperture(&m, 32, 16, 30.0, 60.0, 479.0),
            "default drift must keep the whole pet inside the aperture circle"
        );
    }

    #[test]
    fn over_wide_x_fraction_clips_the_rim() {
        // The spec's rejected 0.70 must be caught by the guard (corner reaches ~516 > 479).
        let m = CompanionMotion {
            drift_x_frac: 0.70,
            ..CompanionMotion::default()
        };
        assert!(
            !drift_keeps_pet_in_aperture(&m, 32, 16, 30.0, 60.0, 479.0),
            "0.70 X fraction should be rejected — the pet corner clips the rim"
        );
    }
}
