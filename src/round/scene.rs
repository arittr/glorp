use std::borrow::Cow;

use ratatui::layout::Rect;

use crate::presentation::{PetSceneModel, SceneDrawList};
#[cfg(test)]
use crate::round::motion::{
    companion_drift_offsets, companion_roam_envelope, project_round_companion_motion_from_offsets,
};
use crate::round::motion::{
    companion_drift_position, project_round_companion_motion, CompanionMotionClearance,
    CompanionMotionInput, RoundCompanionMotionProjection, RoundCompanionMotionViewport,
};
pub use crate::round::motion::{companion_roam_motion, CompanionMotion};
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
#[cfg(test)]
const PET_INK_W: u16 = 11;
#[cfg(test)]
const PET_INK_H: u16 = 8;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothPetAnchor {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionPetPlacement {
    pub(crate) motion_projection: RoundCompanionMotionProjection,
    pub fractional_motion_top_left: SmoothPetAnchor,
    pub fractional_motion_origin_top_left: SmoothPetAnchor,
    pub fractional_top_left: SmoothPetAnchor,
    pub classic_snap_top_left: (u16, u16),
    pub classic_rect: Rect,
    /// Normalized depth for this frame in `[-1, 1]`: `-1` sits against the far
    /// wall, `1` against the near glass. Smooth-only evidence — Classic snapped
    /// placement ignores it entirely.
    pub raw_depth: f32,
}

impl CompanionPetPlacement {
    /// Returns the shared motion result used to derive the Classic and Smooth
    /// placements. It is an observation surface, not a renderer selection.
    pub fn motion_projection(&self) -> RoundCompanionMotionProjection {
        self.motion_projection
    }
}

pub struct RoundTankLifeProtectedRegions {
    pub pet_face: Vec<Rect>,
    pub bottom_hud: Vec<Rect>,
}

fn round_hud_reserve_rows(grid_rows: u16) -> u16 {
    5.min(grid_rows / 3)
}

pub fn round_tank_life_geometry(
    grid_cols: u16,
    grid_rows: u16,
) -> crate::tui::component::TankLifeSurfaceGeometry {
    let bottom_hud_rows = round_hud_reserve_rows(grid_rows);
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

pub fn companion_pet_placement(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> CompanionPetPlacement {
    let projection = project_round_companion_motion(
        companion_motion_input(vm, now, motion),
        now,
        0,
        motion_viewport(grid_cols, grid_rows),
        motion,
    );
    placement_from_projection(vm, grid_cols, grid_rows, projection)
}

#[cfg(test)]
fn companion_pet_placement_from_offsets_for_test(
    vm: &WatchViewModel,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
    fx: f32,
    fy: f32,
) -> CompanionPetPlacement {
    let projection = project_round_companion_motion_from_offsets(
        companion_motion_input(vm, time::OffsetDateTime::UNIX_EPOCH, motion),
        0,
        motion_viewport(grid_cols, grid_rows),
        motion,
        fx,
        fy,
        0.0,
        vm.facing,
        vm.wander_offset_x,
    );
    placement_from_projection(vm, grid_cols, grid_rows, projection)
}

fn motion_viewport(grid_cols: u16, grid_rows: u16) -> RoundCompanionMotionViewport {
    RoundCompanionMotionViewport {
        grid_columns: grid_cols,
        grid_rows,
        width_points: f32::from(grid_cols),
        height_points: f32::from(grid_rows),
        clearance: current_round_motion_clearance(grid_rows),
    }
}

pub fn current_round_motion_clearance(grid_rows: u16) -> CompanionMotionClearance {
    CompanionMotionClearance {
        near_scale: crate::round::depth::SMOOTH_PET_NEAR_SCALE,
        perspective_y_max: crate::round::depth::SMOOTH_PERSPECTIVE_Y_MAX,
        bottom_reserved_rows: 5.min(grid_rows / 3),
    }
}

fn companion_motion_input(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    motion: &CompanionMotion,
) -> CompanionMotionInput {
    let wander_width = PET_W + 2 * motion.wander_half;
    let (resolved_wander_offset_x, resolved_wander_facing) =
        crate::tui::wander::resolve_wander_offset(vm, now, wander_width);
    CompanionMotionInput {
        identity: crate::round::locomotion::stable_companion_identity(&vm.pet_render.seed),
        asleep: vm.day_context.asleep,
        sleep_onset_utc: vm.day_context.sleep_onset_utc,
        wake_from_eval_utc: vm
            .day_context
            .wake_resume
            .map(|resume| resume.from_eval_utc),
        woke_at_utc: vm.day_context.wake_resume.map(|resume| resume.woke_at_utc),
        current_facing: vm.facing,
        resolved_wander_offset_x,
        resolved_wander_facing,
        breath_offset_y_cells: vm.breath_offset_y,
    }
}

fn placement_from_projection(
    _vm: &WatchViewModel,
    _grid_cols: u16,
    grid_rows: u16,
    projection: RoundCompanionMotionProjection,
) -> CompanionPetPlacement {
    let x = projection.motion_top_left_cells.x;
    let y = projection.motion_top_left_cells.y;
    let breathed_y = (y + f32::from(projection.breath_offset_y_cells))
        .min(f32::from(grid_rows.saturating_sub(PET_H)));
    let classic_x = projection.classic_top_left_cells[0];
    let classic_y = projection.classic_top_left_cells[1];
    CompanionPetPlacement {
        motion_projection: projection,
        fractional_motion_top_left: SmoothPetAnchor { x, y },
        fractional_motion_origin_top_left: SmoothPetAnchor {
            x: projection.motion_origin_top_left_cells.x,
            y: projection.motion_origin_top_left_cells.y,
        },
        fractional_top_left: SmoothPetAnchor { x, y: breathed_y },
        classic_snap_top_left: (classic_x, classic_y),
        classic_rect: Rect::new(classic_x, classic_y, PET_W, PET_H),
        raw_depth: projection.normalized_depth,
    }
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
/// whole circle. The pet position is driven by `companion_pet_placement(...)`,
/// which preserves the legacy Classic snap-and-breath rect while also exposing a
/// fractional top-left anchor for Smooth renderers. Wander mode follows the
/// shared identity-and-wall-clock locomotion route; only legacy non-wander
/// placement uses `motion.drift_period_secs`, keeping the pet body within the
/// safe central ellipse at all times.
///
/// Tune legacy non-wander placement via the caller's `CompanionMotion` fields
/// (`drift_x_frac`, `drift_y_frac`, `drift_period_secs`).
pub fn build_round_scene_draw_list(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> CompanionScene {
    let (vm, layout, new_pet_art) = build_round_pet_layout(vm, now, grid_cols, grid_rows, motion);
    let vm = vm.as_ref();
    let ctx = RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(now));

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
    apply_uniform_porthole_recolor(&mut scene_list, grid_rows);
    CompanionScene {
        draw_list: scene_list,
        pet_rect: new_pet_art,
    }
}

pub(crate) fn build_round_pet_layout<'a>(
    vm: &'a WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> (
    Cow<'a, WatchViewModel>,
    crate::tui::component::PetSceneLayout,
    Rect,
) {
    let (vm, layout, placement) =
        build_round_pet_layout_with_placement(vm, now, grid_cols, grid_rows, motion);

    (vm, layout, placement.classic_rect)
}

pub(crate) fn build_round_pet_layout_with_placement<'a>(
    vm: &'a WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> (
    Cow<'a, WatchViewModel>,
    crate::tui::component::PetSceneLayout,
    CompanionPetPlacement,
) {
    let area = Rect::new(0, 0, grid_cols, grid_rows);
    let projection = project_round_companion_motion(
        companion_motion_input(vm, now, motion),
        now,
        0,
        motion_viewport(grid_cols, grid_rows),
        motion,
    );
    let wx = projection.wander_offset_x;
    let facing = projection.facing;
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

    let ctx = RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(now));
    let mut layout = PetScene::compute_layout(area, vm.as_ref(), &ctx);
    let old_pet_art = layout.pet_art;
    let placement = placement_from_projection(vm.as_ref(), grid_cols, grid_rows, projection);
    layout.pet_art = placement.classic_rect;
    for excl in &mut layout.exclusions {
        if *excl == old_pet_art {
            *excl = placement.classic_rect;
            break;
        }
    }

    (vm, layout, placement)
}

pub(crate) fn apply_uniform_porthole_recolor(draw_list: &mut SceneDrawList, grid_rows: u16) {
    let sky_bg = draw_list
        .cells
        .iter()
        .find(|c| c.glyph.is_none() && c.bg.is_some() && c.row < grid_rows / 2)
        .and_then(|c| c.bg);
    if let Some(sky) = sky_bg {
        for cell in &mut draw_list.cells {
            if cell.glyph.is_none() && cell.bg.is_some() {
                cell.bg = Some(sky);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::depth::{SMOOTH_PERSPECTIVE_Y_MAX, SMOOTH_PET_NEAR_SCALE};
    use time::macros::datetime;

    /// Canonical pinned fixture for the content-lock golden.
    /// Grid: 44 cols × 18 rows — wide enough that the 13-wide pet (PET_W=13)
    /// is roughly 30 % of the width, matching the round companion's visual target.
    const GOLDEN_GRID_COLS: u16 = 44;
    const GOLDEN_GRID_ROWS: u16 = 18;
    const GOLDEN_NOW: time::OffsetDateTime = datetime!(2026-06-13 18:00 UTC);

    fn legacy_classic_pet_rect(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
        grid_cols: u16,
        grid_rows: u16,
        motion: &CompanionMotion,
    ) -> Rect {
        let (fx, fy, _) = companion_drift_offsets(now, motion.drift_period_secs);
        let (drift_x, drift_y) = companion_drift_position(motion, grid_cols, grid_rows, fx, fy);
        let breathed_y =
            (drift_y + u16::from(vm.breath_offset_y)).min(grid_rows.saturating_sub(PET_H));
        Rect::new(drift_x, breathed_y, PET_W, PET_H)
    }

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

    /// `smooth_roam_envelope` reserves clearance against the creature ink but
    /// converts an ink centre back into a frame anchor by subtracting half the
    /// *frame*. That identity holds only while these constants track the renderer's
    /// real frame and art, and the art is centred inside the frame.
    #[test]
    fn pet_extents_track_the_renderer_frame_and_centred_art() {
        use crate::pet::render::{
            ART_HEIGHT, ART_ORIGIN_COL, ART_ORIGIN_ROW, ART_WIDTH, FRAME_HEIGHT, FRAME_WIDTH,
        };

        assert_eq!(usize::from(PET_W), FRAME_WIDTH);
        assert_eq!(usize::from(PET_H), FRAME_HEIGHT);
        assert_eq!(usize::from(PET_INK_W), ART_WIDTH);
        assert_eq!(usize::from(PET_INK_H), ART_HEIGHT);

        assert_eq!(
            FRAME_WIDTH - ART_WIDTH,
            ART_ORIGIN_COL * 2,
            "art must be horizontally centred in the particle frame"
        );
        assert_eq!(
            FRAME_HEIGHT - ART_HEIGHT,
            ART_ORIGIN_ROW * 2,
            "art must be vertically centred in the particle frame"
        );
    }

    #[test]
    fn smooth_roam_envelope_leaves_a_usable_band_at_companion_grid_sizes() {
        // Reserving maximum-scale clearance against the particle frame rather than
        // the ink inverted this band at 16 rows and left 1.8 cells at 18, pinning
        // the pet against an invisible ceiling for most of its cycle.
        for grid_rows in 16..=24 {
            let envelope = companion_roam_envelope(motion_viewport(36, grid_rows));
            assert!(
                envelope.max_y >= envelope.min_y,
                "{grid_rows}-row grid inverted the vertical roam band"
            );
            assert!(envelope.max_x > envelope.min_x);
        }

        // The shipping companion is 36x18 (`COMPANION_TARGET_COLS`, and square-view
        // rows). Non-inversion alone would be satisfied by a hairline band, so pin
        // the grid the pet actually swims in.
        let live = companion_roam_envelope(motion_viewport(36, 18));
        assert!(
            live.max_y - live.min_y >= 3.0,
            "live 36x18 companion grid left only {:.2} cells of vertical roam",
            live.max_y - live.min_y
        );
    }

    /// The envelope reserves clearance for the *maximum* depth scale, but depth also
    /// translates the pet: near is `+SMOOTH_PERSPECTIVE_Y_MAX` (down), far is
    /// negative (up). That translation is composed onto the pet-attached layers
    /// after this clamp, so it has to be reserved here or the nearest pose pushes
    /// the creature's ink into the HUD reserve.
    #[test]
    fn smooth_roam_envelope_reserves_the_near_depth_perspective_excursion() {
        let frame_half_h = f32::from(PET_H) / 2.0;
        let scaled_ink_half_h = f32::from(PET_INK_H) / 2.0 * SMOOTH_PET_NEAR_SCALE;

        for grid_rows in 16..=24 {
            let envelope = companion_roam_envelope(motion_viewport(36, grid_rows));
            let protected_bottom =
                f32::from(grid_rows.saturating_sub(round_hud_reserve_rows(grid_rows)));

            let nearest_ink_bottom =
                envelope.max_y + frame_half_h + scaled_ink_half_h + SMOOTH_PERSPECTIVE_Y_MAX;
            assert!(
                nearest_ink_bottom <= protected_bottom + 1e-4,
                "{grid_rows}-row grid: nearest pet ink reached {nearest_ink_bottom:.2}, \
                 {:.2} cells into the HUD reserve at {protected_bottom:.2}",
                nearest_ink_bottom - protected_bottom
            );

            let farthest_ink_top =
                envelope.min_y + frame_half_h - scaled_ink_half_h - SMOOTH_PERSPECTIVE_Y_MAX;
            assert!(
                farthest_ink_top >= -1e-4,
                "{grid_rows}-row grid: farthest pet ink rose to {farthest_ink_top:.2}, \
                 above the aperture top"
            );
        }
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
    fn companion_pet_placement_keeps_legacy_nonwander_classic_rect() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = CompanionMotion::default();
        let samples = [
            datetime!(2026-07-08 18:00:00 UTC),
            datetime!(2026-07-08 18:00:00.250 UTC),
            datetime!(2026-07-08 18:00:00.500 UTC),
            datetime!(2026-07-08 18:00:00.750 UTC),
        ];

        for now in samples {
            let placement =
                companion_pet_placement(&vm, now, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &motion);
            let scene =
                build_round_scene_draw_list(&vm, now, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &motion);
            let expected =
                legacy_classic_pet_rect(&vm, now, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &motion);

            assert_eq!(
                placement.classic_rect, expected,
                "shared placement must preserve the legacy Classic rect at {now}"
            );
            assert_eq!(
                scene.pet_rect, expected,
                "round scene must keep the legacy rect at {now}"
            );
            assert_eq!(placement.classic_snap_top_left, (expected.x, expected.y));
        }
    }

    #[test]
    fn neutral_motion_origin_uses_the_same_bias_and_clamps_as_current_motion() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = companion_roam_motion();
        let placement = companion_pet_placement_from_offsets_for_test(
            &vm,
            GOLDEN_GRID_COLS,
            GOLDEN_GRID_ROWS,
            &motion,
            0.0,
            0.0,
        );

        assert_eq!(
            placement.fractional_motion_top_left,
            placement.fractional_motion_origin_top_left
        );
        assert!(placement.fractional_motion_origin_top_left.x >= 0.0);
        assert!(placement.fractional_motion_origin_top_left.y >= 0.0);
        assert!(
            placement.fractional_motion_origin_top_left.x
                <= f32::from(GOLDEN_GRID_COLS.saturating_sub(PET_W))
        );
        assert!(
            placement.fractional_motion_origin_top_left.y
                <= f32::from(GOLDEN_GRID_ROWS.saturating_sub(PET_H))
        );
    }

    #[test]
    fn classic_breath_changes_posture_but_not_continuous_motion_origin() {
        let motion = companion_roam_motion();
        let mut still = WatchViewModel::fixture_with_habitat_props();
        let mut breathed = still.clone();
        still.breath_offset_y = 0;
        breathed.breath_offset_y = 1;

        let still_placement = companion_pet_placement_from_offsets_for_test(
            &still,
            GOLDEN_GRID_COLS,
            GOLDEN_GRID_ROWS,
            &motion,
            0.4,
            -0.3,
        );
        let breathed_placement = companion_pet_placement_from_offsets_for_test(
            &breathed,
            GOLDEN_GRID_COLS,
            GOLDEN_GRID_ROWS,
            &motion,
            0.4,
            -0.3,
        );

        assert_eq!(
            still_placement.fractional_motion_top_left,
            breathed_placement.fractional_motion_top_left
        );
        assert_eq!(
            still_placement.fractional_motion_origin_top_left,
            breathed_placement.fractional_motion_origin_top_left
        );
        assert_ne!(
            still_placement.fractional_top_left,
            breathed_placement.fractional_top_left
        );
        assert_ne!(
            still_placement.classic_rect,
            breathed_placement.classic_rect
        );
    }

    #[test]
    fn companion_pet_placement_uses_classic_piecewise_truncation_contract() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.breath_offset_y = 1;
        let motion = CompanionMotion {
            wander_half: 8,
            drift_x_frac: 0.9,
            drift_y_frac: 0.6,
            drift_period_secs: 22,
            upward_bias: 0.5,
            wander: true,
        };
        let placement = companion_pet_placement_from_offsets_for_test(
            &vm,
            GOLDEN_GRID_COLS,
            GOLDEN_GRID_ROWS,
            &motion,
            -0.25,
            -0.25,
        );

        let cx = GOLDEN_GRID_COLS / 2;
        let cy = GOLDEN_GRID_ROWS / 2;
        let half_w = PET_W / 2;
        let half_h = PET_H / 2;
        let safe_x = cx.saturating_sub(half_w) as f32;
        let safe_y = cy.saturating_sub(half_h) as f32;
        let x_radius = safe_x * motion.drift_x_frac;
        let y_radius = safe_y * motion.drift_y_frac;
        let bias = motion.upward_bias * safe_y;
        let classic_x = (cx as i32 - half_w as i32 + (-0.25 * x_radius) as i32)
            .clamp(0, GOLDEN_GRID_COLS.saturating_sub(PET_W) as i32) as u16;
        let classic_drift_y = (cy as i32 - half_h as i32 - bias as i32 + (-0.25 * y_radius) as i32)
            .clamp(0, GOLDEN_GRID_ROWS.saturating_sub(PET_H) as i32)
            as u16;
        let classic_y = (classic_drift_y + u16::from(vm.breath_offset_y))
            .min(GOLDEN_GRID_ROWS.saturating_sub(PET_H));

        assert_eq!(placement.classic_snap_top_left, (classic_x, classic_y));
        assert_ne!(
            placement.classic_snap_top_left.1 as i32,
            (placement.fractional_top_left.y.floor() as i32),
            "this fixture must prove Classic snap is not composite floor"
        );
    }

    #[test]
    fn companion_pet_placement_exposes_fractional_top_left_for_roam_motion() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = companion_roam_motion();
        let placement = companion_pet_placement(
            &vm,
            datetime!(2026-07-08 18:00:00.500 UTC),
            GOLDEN_GRID_COLS,
            GOLDEN_GRID_ROWS,
            &motion,
        );

        assert!(
            placement.fractional_top_left.x.fract().abs() > f32::EPSILON
                || placement.fractional_top_left.y.fract().abs() > f32::EPSILON,
            "roam motion should preserve a fractional anchor for Smooth renderers"
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
