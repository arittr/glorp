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
    /// When true, use a smooth sinusoidal wander (organic, non-repeating, reaching
    /// the grid edges so the pet swims partly in/out of the round porthole) instead
    /// of the eased waypoint drift. The menubar/goldens keep `false` (waypoint).
    pub wander: bool,
}

impl Default for CompanionMotion {
    fn default() -> Self {
        Self {
            wander_half: 8,
            drift_x_frac: 0.45,
            drift_y_frac: 0.30,
            drift_period_secs: 20,
            upward_bias: 0.0,
            wander: false,
        }
    }
}

/// The companion surface's motion — an organic sinusoidal wander that reaches the
/// grid edges, so the pet swims around the tank and partly in/out of the round
/// porthole (the aperture clip crops it at the rim). Horizontal-dominant; a gentle
/// upward bias keeps it mostly clear of the bottom stat. Companion-only.
pub fn companion_roam_motion() -> CompanionMotion {
    CompanionMotion {
        wander_half: 8,
        drift_x_frac: 0.92,
        drift_y_frac: 0.6,
        drift_period_secs: 22,
        upward_bias: 0.5,
        wander: true,
    }
}

pub struct RoundTankLifeProtectedRegions {
    pub pet_face: Vec<Rect>,
    pub bottom_hud: Vec<Rect>,
}

pub fn round_tank_life_geometry(
    grid_cols: u16,
    grid_rows: u16,
) -> crate::tui::component::TankLifeSurfaceGeometry {
    let bottom_hud_rows = 5.min(grid_rows / 3);
    crate::tui::component::TankLifeSurfaceGeometry {
        surface: crate::tui::component::TankLifeSurface::Round,
        habitat: Rect::new(0, 0, grid_cols, grid_rows),
        aperture_mask: Some(crate::tui::component::RoundApertureMask {
            center_col: (grid_cols / 2) as i16,
            center_row: (grid_rows / 2) as i16,
            radius_cols: grid_cols / 2,
            radius_rows: grid_rows / 2,
        }),
        reserved_regions: vec![Rect::new(
            0,
            grid_rows.saturating_sub(bottom_hud_rows),
            grid_cols,
            bottom_hud_rows,
        )],
        max_moving_slots: 2,
        literal_floor_allowed: false,
    }
}

pub fn round_tank_life_protected_regions_for_test(
    pet_rect: Rect,
    grid_cols: u16,
    grid_rows: u16,
) -> RoundTankLifeProtectedRegions {
    let geometry = round_tank_life_geometry(grid_cols, grid_rows);
    RoundTankLifeProtectedRegions {
        pet_face: crate::tui::component::pet_face_protected_regions(pet_rect),
        bottom_hud: geometry.reserved_regions,
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

/// Smooth, deterministic, non-repeating organic wander in ~[-1, 1] per axis.
/// A slowly-precessing elliptical base (cos on X, sin on Y at a slightly different
/// rate) keeps the velocity vector always rotating, so the path can never flatten
/// into a strict horizontal/vertical line — it always wobbles. A smaller
/// incommensurate term per axis breaks the clean-orbit feel. Sub-second time keeps
/// it smooth at the companion's redraw cadence.
fn companion_wander_offsets(now: time::OffsetDateTime, period_secs: u64) -> (f32, f32) {
    use std::f64::consts::TAU;
    let t = (now.unix_timestamp() as f64 + now.nanosecond() as f64 / 1_000_000_000.0)
        / period_secs.max(1) as f64;
    let fx = 0.72 * (TAU * t).cos() + 0.28 * (TAU * t * 1.93 + 0.6).sin();
    let fy = 0.72 * (TAU * t * 1.21 + 0.3).sin() + 0.28 * (TAU * t * 2.41 + 1.5).cos();
    (fx as f32, fy as f32)
}

/// Which way the wandering pet faces: the sign of its NET horizontal travel over a
/// short window, so facing always agrees with the movement actually on screen —
/// it samples the SAME wander offset that drives the position (full base + wobble),
/// scaled by `energy`. A deadzone holds `current` when the pet is barely moving
/// (idle/asleep, or pausing at a turnaround), so facing never flips without a
/// matching change of direction. Right-moving → `1`, left → `-1` (compute_facing's
/// convention).
fn companion_wander_facing(
    now: time::OffsetDateTime,
    period_secs: u64,
    energy: f32,
    current: i8,
) -> i8 {
    const WINDOW_SECS: i64 = 1;
    const DEADZONE: f32 = 0.04;
    let (fx_now, _) = companion_wander_offsets(now, period_secs);
    let (fx_prev, _) =
        companion_wander_offsets(now - time::Duration::seconds(WINDOW_SECS), period_secs);
    // Proportional to the on-screen horizontal distance moved over the window.
    let visible_dx = (fx_now - fx_prev) * energy;
    if visible_dx > DEADZONE {
        1
    } else if visible_dx < -DEADZONE {
        -1
    } else {
        current
    }
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
/// art rect in grid coords. `energy` (0..1, from live activity) scales the wander
/// amplitude, so the pet's on-screen speed reflects real usage. The reachable set
/// per axis is ~[-1, 1]; callers needing a circular bound must sample box corners.
fn companion_drift(
    now: time::OffsetDateTime,
    motion: &CompanionMotion,
    energy: f32,
    grid_cols: u16,
    grid_rows: u16,
) -> (u16, u16) {
    let (fx, fy) = if motion.wander {
        // Scale the wander by live activity: amplitude — and so the pet's on-screen
        // speed — shrinks toward center when idle/asleep and expands when busy.
        let (wx, wy) = companion_wander_offsets(now, motion.drift_period_secs);
        (wx * energy, wy * energy)
    } else {
        companion_drift_offsets(now, motion.drift_period_secs)
    };
    companion_drift_position(motion, grid_cols, grid_rows, fx, fy)
}

/// Movement energy in [0, 1] from real activity: a sleeping or faint (calm) pet
/// barely drifts; an idle-awake pet keeps a gentle wobble; a busy pet roams the
/// whole tank. Tied to the live burn rate so liveliness reflects real usage.
fn companion_motion_energy(vm: &WatchViewModel) -> f32 {
    const IDLE_FLOOR: f32 = 0.25;
    const RESTING_ENERGY: f32 = 0.12;
    const RATE_FULL: f64 = 50_000_000.0; // tokens/hr at which the pet roams full-tilt
    if vm.day_context.asleep || vm.life_profile.calm_mode {
        return RESTING_ENERGY;
    }
    let rate = vm.progress.rate_per_hour.max(0.0);
    (IDLE_FLOOR + (rate / RATE_FULL) as f32).clamp(IDLE_FLOOR, 1.0)
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
    let cxg = (grid_cols / 2) as f64;
    let cyg = (grid_rows / 2) as f64;
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

/// Build a [`CompanionScene`] for the round companion viewport.
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
/// between deterministic 2D targets every `motion.drift_period_secs` seconds,
/// keeping the pet body within the safe central ellipse at all times.
///
/// Tune via the caller's `CompanionMotion` fields (`drift_x_frac`,
/// `drift_y_frac`, `drift_period_secs`).
pub fn build_round_scene_draw_list(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> CompanionScene {
    // Full-grid area: the bg wash + ambient cover the whole porthole.
    let area = Rect::new(0, 0, grid_cols, grid_rows);

    // Movement energy from live activity (drives both the wander amplitude and the
    // facing deadzone). Read from vitals that the Cow rebind below does not touch.
    let energy = companion_motion_energy(vm);

    // Resolve horizontal facing from the live clock and narrowed wander width,
    // mirroring what PetPanel::render does but with companion-specific range.
    let wander_width = PET_W + 2 * motion.wander_half;
    let (wx, fc) = crate::tui::wander::resolve_wander_offset(vm, now, wander_width);
    // In wander mode the pet faces its ACTUAL net horizontal travel, so it turns to
    // face where it's swimming instead of gliding sideways. Otherwise use the shared
    // resolved facing.
    let facing = if motion.wander {
        companion_wander_facing(now, motion.drift_period_secs, energy, vm.facing)
    } else {
        fc
    };
    let vm: Cow<WatchViewModel> = if wx != vm.wander_offset_x || facing != vm.facing {
        Cow::Owned({
            let mut v = vm.clone();
            v.wander_offset_x = wx;
            v.facing = facing;
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
    let (drift_x, drift_y) = companion_drift(now, motion, energy, grid_cols, grid_rows);
    // Add the breath bob on top of the wander so the pet visibly breathes —
    // otherwise breath_offset_y is computed every tick and silently discarded.
    let breathed_y = (drift_y + u16::from(vm.breath_offset_y)).min(grid_rows.saturating_sub(PET_H));
    let new_pet_art = Rect::new(drift_x, breathed_y, PET_W, PET_H);
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
    let tank_geometry = round_tank_life_geometry(grid_cols, grid_rows);
    let mut scene_list = crate::tui::panels::pet::render_pet_to_draw_list_with_tank_geometry(
        &model,
        vm,
        &layout,
        now,
        &ctx,
        &tank_geometry,
    );

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
    fn round_hud_reserve_does_not_prune_non_tank_life_scene_glyphs() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_art = vec!["#############".into(); PET_H as usize];
        let motion = CompanionMotion {
            drift_y_frac: 0.0,
            upward_bias: -1.0,
            ..CompanionMotion::default()
        };

        let scene = build_round_scene_draw_list(
            &vm,
            GOLDEN_NOW,
            GOLDEN_GRID_COLS,
            GOLDEN_GRID_ROWS,
            &motion,
        );
        let reserved =
            round_tank_life_geometry(GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS).reserved_regions;
        assert!(
            scene.draw_list.cells.iter().any(|cell| {
                cell.glyph.is_some()
                    && reserved
                        .iter()
                        .any(|region| crate::tui::component::rect_contains(*region, cell.col, cell.row))
            }),
            "round scene must not remove non-tank-life glyphs from the HUD reserve; the native HUD draws above the scene"
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
    fn companion_wander_is_deterministic_and_bounded() {
        // Companion roam now uses an organic sinusoidal wander that intentionally
        // reaches the grid edges (the pet swims partly in/out of the porthole), so it
        // is NOT circle-bounded. Verify it is deterministic and stays within ~[-1, 1].
        let now = datetime!(2026-06-13 18:00:00.5 UTC);
        let a = companion_wander_offsets(now, 16);
        let b = companion_wander_offsets(now, 16);
        assert_eq!(a, b, "wander must be deterministic for a fixed instant");
        assert!(
            a.0.abs() <= 1.01 && a.1.abs() <= 1.01,
            "two unit sinusoids per axis stay within ~[-1, 1], got {a:?}"
        );
        assert!(
            companion_roam_motion().wander,
            "companion roam uses wander mode"
        );
    }

    #[test]
    fn motion_energy_tracks_activity() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.day_context.asleep = false;
        vm.life_profile.calm_mode = false;
        vm.progress.rate_per_hour = 0.0;
        let idle = companion_motion_energy(&vm);
        vm.progress.rate_per_hour = 80_000_000.0;
        let busy = companion_motion_energy(&vm);
        assert!(
            busy > idle,
            "busy pet moves more than idle (idle={idle}, busy={busy})"
        );
        assert!(
            (0.99..=1.0).contains(&busy),
            "high burn saturates near full, got {busy}"
        );
        vm.day_context.asleep = true;
        let asleep = companion_motion_energy(&vm);
        assert!(
            asleep < idle,
            "a sleeping pet barely drifts (asleep={asleep}, idle={idle})"
        );
    }

    #[test]
    fn wander_facing_follows_travel_and_holds_when_still() {
        // At full energy the pet faces both ways across a cycle, matching the sign of
        // its actual windowed travel — and never returns a non-±1 value.
        let (mut saw_left, mut saw_right) = (false, false);
        for s in 0..30i64 {
            let now = datetime!(2026-06-13 18:00:00 UTC) + time::Duration::seconds(s);
            let (fx_now, _) = companion_wander_offsets(now, 22);
            let (fx_prev, _) = companion_wander_offsets(now - time::Duration::seconds(1), 22);
            let dx = fx_now - fx_prev;
            let f = companion_wander_facing(now, 22, 1.0, 1);
            match f {
                1 => saw_right = true,
                -1 => saw_left = true,
                other => panic!("facing must be ±1, got {other}"),
            }
            // Facing agrees with the actual travel direction (outside the deadzone).
            if dx > 0.06 {
                assert_eq!(f, 1, "moving right ⇒ faces right at s={s}");
            }
            if dx < -0.06 {
                assert_eq!(f, -1, "moving left ⇒ faces left at s={s}");
            }
        }
        assert!(
            saw_left && saw_right,
            "pet must face both directions across a wander cycle"
        );
        // A (near-)still pet — energy 0, so visible movement is below the deadzone —
        // HOLDS its current facing instead of flipping. This is the bug we fixed:
        // facing must never flip without matching movement.
        let still = datetime!(2026-06-13 18:00:30 UTC);
        assert_eq!(companion_wander_facing(still, 22, 0.0, -1), -1);
        assert_eq!(companion_wander_facing(still, 22, 0.0, 1), 1);
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
