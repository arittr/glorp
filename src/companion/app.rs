//! Native macOS round companion window. Uses a regular Dock app lifecycle,
//! a worker thread for live usage polling, and pure AppKit drawing from
//! `RoundSceneModel`.

#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::sync::mpsc;
use std::time::Duration;

use crate::commands::watch::{build_watch_view_model, rerender_pet_for_view_model};
use crate::companion::render::{build_draw_commands, RoundColor, RoundDrawKind};
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundSceneModel};
use crate::storage::state::StateStore;
use crate::tui::view_model::WatchViewModel;
use crate::watch_live::{LiveWatchUpdate, WatchPresentationState};
use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAttributedStringNSStringDrawing,
    NSBackingStoreType, NSBezierPath, NSColor, NSCommandKeyMask, NSFont, NSFontAttributeName,
    NSFontWeightBold, NSForegroundColorAttributeName, NSMenu, NSMenuItem, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString, NSTimer,
};

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const UI_TICK_INTERVAL_SECS: f64 = 0.25;
const DEFAULT_WINDOW_SIZE: f64 = 360.0;
const WINDOW_ORIGIN_X: f64 = 120.0;
const WINDOW_ORIGIN_Y: f64 = 120.0;
const MIN_WINDOW_SIZE: f64 = 260.0;

// ─────────────────────────────────────────────────────────────────────────────
// Ambient HUD — tunable layout constants
// Adjust these to move/resize the overlay elements without touching draw logic.
//
// Visual hierarchy (top → bottom):
//   [AMBIENT]  Tiny vital ticks at top rim — fed/happy/energy, dimmed
//   [HERO]     Large token count + secondary rate line in lower band
//   [HERO]     Wide evolve bar below tokens — stage progress toward next form
// ─────────────────────────────────────────────────────────────────────────────

// ── Hero: Token count ────────────────────────────────────────────────────────

/// Token count font = derived grid font_size × this multiplier.
/// 2.2 makes the number the visually dominant element in the HUD.
const HUD_TOKEN_FONT_FRAC: f64 = 2.2;

/// Secondary line (rate + "today") font = grid font_size × this multiplier.
/// 0.85 keeps it clearly subordinate to the hero token number.
const HUD_RATE_FONT_FRAC: f64 = 0.85;

/// Fraction of view height DOWN from top for the hero token count baseline.
/// 0.64 = 64 % — sits in the lower-middle of the circle where it's wide.
const HUD_TOKEN_Y_FRAC: f64 = 0.64;

/// Fraction of view height DOWN from top for the secondary rate line.
/// 0.71 = just below the token number.
const HUD_RATE_Y_FRAC: f64 = 0.71;

// ── Hero: Evolve bar ─────────────────────────────────────────────────────────

/// Evolve bar total width as a fraction of the circle chord at the bar row.
/// 0.72 = spans 72 % of the available width — wide and prominent.
const HUD_EVOLVE_BAR_WIDTH_FRAC: f64 = 0.72;

/// Evolve bar height as a fraction of the derived grid font size.
/// 0.55 = a chunky, clearly visible bar (taller than the old inline XP bar).
const HUD_EVOLVE_BAR_H_FRAC: f64 = 0.55;

/// Fraction of view height DOWN from top for the evolve bar centerline.
/// 0.79 = below the rate line, still inside the wide band of the circle.
const HUD_EVOLVE_BAR_Y_FRAC: f64 = 0.79;

/// Gap in points between the evolve bar and its stage labels.
const HUD_EVOLVE_LABEL_GAP: f64 = 3.5;

/// Evolve bar stage-label font = grid font_size × this multiplier.
const HUD_EVOLVE_LABEL_FONT_FRAC: f64 = 0.78;

// ── Ambient vitals — dimmed top-rim ticks ────────────────────────────────────

/// Vital tick row: fraction of view height DOWN from top for the tick centerline.
/// 0.13 = near the very top rim — unobtrusive background status.
const HUD_GAUGE_ROW_Y_FRAC: f64 = 0.13;

/// Combined width of all three vital tick bars as a fraction of chord.
/// 0.50 = narrower than before; these are ambient, not prominent.
const HUD_GAUGE_TOTAL_WIDTH_FRAC: f64 = 0.50;

/// Height of each vital fill bar as a fraction of grid font size.
/// 0.25 = thin ticks, clearly subordinate to the evolve bar.
const HUD_GAUGE_BAR_H_FRAC: f64 = 0.25;

/// Gap between adjacent vital tick bars in points.
const HUD_GAUGE_BAR_GAP: f64 = 5.0;

/// Height of the dim track behind each vital tick as a fraction of font size.
const HUD_GAUGE_TRACK_H_FRAC: f64 = 0.25;

/// Vital tick labels are hidden — set > 0.0 to restore label text above the ticks.
#[allow(dead_code)]
const HUD_GAUGE_LABEL_FONT_FRAC: f64 = 0.0;

/// Vertical offset from bar bottom to label baseline. Unused while labels are hidden.
const HUD_GAUGE_LABEL_OFFSET_Y: f64 = 3.0;

// ── Colors ───────────────────────────────────────────────────────────────────

/// "fed" vital tick fill — warm amber/tan, at reduced alpha (ambient).
const HUD_COLOR_FED: (u8, u8, u8) = (210, 160, 80);
/// "happy" vital tick fill — soft pink, at reduced alpha (ambient).
const HUD_COLOR_HAPPY: (u8, u8, u8) = (210, 100, 140);
/// "energy" vital tick fill — cyan/teal, at reduced alpha (ambient).
const HUD_COLOR_ENERGY: (u8, u8, u8) = (80, 200, 200);
/// Dim track background for vitals (very low alpha — barely visible).
const HUD_COLOR_TRACK: (u8, u8, u8, f64) = (180, 180, 200, 0.12);
/// Alpha multiplier applied to vital fill colors (dims them to ambient level).
const HUD_VITAL_FILL_ALPHA: f32 = 0.30;
/// Hero token count color — bright neutral white, high contrast.
const HUD_COLOR_TOKEN: (u8, u8, u8, f64) = (240, 240, 255, 0.92);
/// Secondary text color (rate line, evolve labels) — slightly dimmer.
const HUD_COLOR_TEXT: (u8, u8, u8, f64) = (200, 200, 225, 0.65);
/// Evolve bar fill color — violet (matches existing XP bar feel).
const HUD_COLOR_EVOLVE_FILL: (u8, u8, u8) = (160, 130, 220);
/// Evolve bar track color — dim background.
const HUD_COLOR_EVOLVE_TRACK: (u8, u8, u8, f64) = (180, 180, 200, 0.18);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompanionMenuSpec {
    app_title: &'static str,
    quit_title: &'static str,
    quit_key: &'static str,
}

struct AppState {
    /// Retained to keep the window alive after makeKeyAndOrderFront.
    #[allow(dead_code)]
    window: Retained<NSWindow>,
    view: Retained<RoundView>,
    poll_rx: mpsc::Receiver<LiveWatchUpdate>,
    presentation_state: WatchPresentationState,
    vm: WatchViewModel,
    scene: RoundSceneModel,
    animation_frame: u64,
}

thread_local! {
    static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

declare_class!(
    pub(super) struct Controller;

    unsafe impl ClassType for Controller {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "GlorpCompanionController";
    }

    impl DeclaredClass for Controller {}

    unsafe impl Controller {
        #[method(uiTick:)]
        fn ui_tick(&self, _sender: Option<&AnyObject>) {
            ui_tick();
        }
    }
);

declare_class!(
    pub(super) struct RoundView;

    unsafe impl ClassType for RoundView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "GlorpRoundCompanionView";
    }

    impl DeclaredClass for RoundView {}

    unsafe impl RoundView {
        #[method(drawRect:)]
        fn draw_rect(&self, _rect: NSRect) {
            draw_scene(self.bounds());
        }
    }
);

pub fn run() -> Result<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| GlorpError::Message("glorp companion must run on the main thread".into()))?;
    let paths = AppPaths::resolve()?;
    paths.ensure()?;
    let state_store = StateStore::new(paths.state_file.clone());
    let Some(initial_pet) = state_store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };
    let initial_vm = build_watch_view_model(&initial_pet, &paths.usage_db)?;
    let scene = derive_round_scene_model(&initial_vm, time::OffsetDateTime::now_utc());

    let app: Retained<NSApplication> = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    install_app_menu(&app, mtm);

    let controller: Retained<Controller> = unsafe { msg_send_id![Controller::class(), new] };
    let (window, view) = build_window(mtm);
    let poll_rx =
        crate::watch_live::spawn_live_watch_worker(paths, POLL_INTERVAL, "glorp-companion-poll");

    APP_STATE.with(|cell| {
        *cell.borrow_mut() = Some(AppState {
            window,
            view,
            poll_rx,
            presentation_state: WatchPresentationState::default(),
            vm: initial_vm,
            scene,
            animation_frame: 0,
        });
    });

    let _timer: Retained<NSTimer> = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            UI_TICK_INTERVAL_SECS,
            &controller,
            sel!(uiTick:),
            None,
            true,
        )
    };

    unsafe { app.run() };
    Ok(())
}

fn companion_menu_spec() -> CompanionMenuSpec {
    CompanionMenuSpec {
        app_title: "Glorp",
        quit_title: "Quit Glorp",
        quit_key: "q",
    }
}

fn install_app_menu(app: &NSApplication, mtm: MainThreadMarker) {
    let spec = companion_menu_spec();
    unsafe {
        let main_menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(""));
        let app_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(spec.app_title),
            None,
            &NSString::from_str(""),
        );
        main_menu.addItem(&app_item);

        let app_menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(spec.app_title));
        let quit_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(spec.quit_title),
            Some(sel!(terminate:)),
            &NSString::from_str(spec.quit_key),
        );
        quit_item.setKeyEquivalentModifierMask(NSCommandKeyMask);
        app_menu.addItem(&quit_item);
        main_menu.setSubmenu_forItem(Some(&app_menu), &app_item);
        app.setMainMenu(Some(&main_menu));
    }
}

fn build_window(mtm: MainThreadMarker) -> (Retained<NSWindow>, Retained<RoundView>) {
    let frame = NSRect::new(
        NSPoint::new(WINDOW_ORIGIN_X, WINDOW_ORIGIN_Y),
        NSSize::new(DEFAULT_WINDOW_SIZE, DEFAULT_WINDOW_SIZE),
    );
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::FullSizeContentView;
    let window: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            frame,
            style,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("Glorp"));
    unsafe {
        window.setContentMinSize(NSSize::new(MIN_WINDOW_SIZE, MIN_WINDOW_SIZE));
        window.setReleasedWhenClosed(false);
    }

    let content_frame = NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
    let view: Retained<RoundView> =
        unsafe { msg_send_id![mtm.alloc::<RoundView>(), initWithFrame: content_frame] };
    window.setContentView(Some(&view));
    window.makeKeyAndOrderFront(None);

    (window, view)
}

fn ui_tick() {
    let _mtm = MainThreadMarker::new().expect("companion ui_tick on non-main thread");
    drain_poll_results();
    animate_pet();
}

fn drain_poll_results() {
    let mut latest = None;
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow().as_ref() {
            while let Ok(update) = state.poll_rx.try_recv() {
                latest = Some(update);
            }
        }
    });
    let Some(update) = latest else {
        return;
    };
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            let mut vm = update.vm;
            let now = time::OffsetDateTime::now_utc();
            crate::watch_live::stamp_live_presentation(
                &mut state.presentation_state,
                &mut vm,
                update.applied_signal,
                now,
            );
            state.scene = derive_round_scene_model(&vm, now);
            state.vm = vm;
            unsafe { state.view.setNeedsDisplay(true) };
        }
    });
}

fn animate_pet() {
    let redraw = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        let next_frame = state.animation_frame.wrapping_add(1);
        let now = time::OffsetDateTime::now_utc();
        let changed = advance_companion_animation(&mut state.vm, next_frame, now).ok()?;
        state.animation_frame = next_frame;
        let next_scene = derive_round_scene_model(&state.vm, now);
        let scene_changed = next_scene != state.scene;
        if changed || scene_changed {
            state.scene = next_scene;
            Some(state.view.clone())
        } else {
            None
        }
    });
    if let Some(view) = redraw {
        unsafe { view.setNeedsDisplay(true) };
    }
}

fn advance_companion_animation(
    vm: &mut WatchViewModel,
    frame: u64,
    now: time::OffsetDateTime,
) -> Result<bool> {
    let prev_pet_art = vm.pet_art.clone();
    let prev_pet_spans = vm.pet_spans.clone();
    let prev_breath_offset_y = vm.breath_offset_y;
    rerender_pet_for_view_model(vm, frame, vm.day_context.asleep)?;
    let species = vm.pet_render.generated_species;
    let rhythm = crate::pet::animator::breath_rhythm_for_day(&vm.day_context);
    vm.breath_offset_y =
        crate::pet::animator::compute_breath_offset_with_rhythm(Some(species), now, rhythm);
    Ok(vm.pet_art != prev_pet_art
        || vm.pet_spans != prev_pet_spans
        || vm.breath_offset_y != prev_breath_offset_y)
}

fn draw_scene(bounds: NSRect) {
    let _mtm = MainThreadMarker::new().expect("companion draw_scene on non-main thread");
    let state_snapshot = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|s| (s.scene.clone(), s.vm.clone()))
    });
    let Some((scene, vm)) = state_snapshot else {
        return;
    };

    let now = time::OffsetDateTime::now_utc();
    let width = bounds.size.width as f32;
    let height = bounds.size.height as f32;
    let aperture = RoundAperture::new(width as u16, height as u16);

    // Build the halo/trouble commands from the round scene model (kept on top).
    let layout = layout_round_scene(
        &scene,
        aperture,
        RoundRenderCapabilities::preview_truecolor(),
    );
    let commands = build_draw_commands(&scene, &layout);

    // Compute background color from the first Background command.
    let bg_color = commands
        .iter()
        .find(|c| c.kind == RoundDrawKind::Background)
        .map(|c| c.color)
        .unwrap_or(RoundColor(0.05, 0.06, 0.10, 1.0));

    let dim_overlay = scene.lifecycle.asleep || scene.lifecycle.calm;
    unsafe {
        // Circular clip so the scene stays inside the aperture.
        let clip = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(
                (aperture.center_x - aperture.radius) as f64,
                (aperture.center_y - aperture.radius) as f64,
            ),
            NSSize::new(
                (aperture.radius * 2.0) as f64,
                (aperture.radius * 2.0) as f64,
            ),
        ));
        clip.addClip();

        // Background base fill — drawn first, under everything.
        let bg_path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(
                (aperture.center_x - aperture.radius) as f64,
                (aperture.center_y - aperture.radius) as f64,
            ),
            NSSize::new(
                (aperture.radius * 2.0) as f64,
                (aperture.radius * 2.0) as f64,
            ),
        ));
        ns_color(&bg_color).setFill();
        bg_path.fill();

        // Blit the shared scene draw list (habitat + pet) when grid metrics are available.
        if let Some(m) = companion_grid_metrics(bounds.size.width, bounds.size.height) {
            let list = crate::round::scene::build_round_scene_draw_list(
                &vm,
                now,
                m.grid_cols,
                m.grid_rows,
            );
            appkit_blit_draw_list(
                &list,
                m.font_size,
                m.cell_w,
                m.cell_h,
                m.origin_x,
                m.origin_y,
            );
        }

        // Halo and trouble indicators drawn on top of the scene blit.
        for command in commands
            .iter()
            .filter(|c| matches!(c.kind, RoundDrawKind::Halo | RoundDrawKind::Trouble))
        {
            let path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(
                    (command.x - command.radius) as f64,
                    (command.y - command.radius) as f64,
                ),
                NSSize::new((command.radius * 2.0) as f64, (command.radius * 2.0) as f64),
            ));
            ns_color(&command.color).setFill();
            path.fill();
        }

        // Ambient HUD — drawn after halo beads and before the sleep/calm dim,
        // so the dim overlay softens the HUD when the pet is resting.
        // Pass the derived font size so HUD elements scale with the display.
        let hud_font_size = companion_grid_metrics(bounds.size.width, bounds.size.height)
            .map(|m| m.font_size)
            .unwrap_or(8.5);
        draw_hud(bounds, &aperture, &vm, hud_font_size);

        if dim_overlay {
            let dim = NSBezierPath::bezierPathWithRect(bounds);
            NSColor::colorWithSRGBRed_green_blue_alpha(0.05, 0.06, 0.10, 0.35).setFill();
            dim.fill();
        }
    }
}

fn rgb_color(r: u8, g: u8, b: u8) -> RoundColor {
    RoundColor(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    )
}

fn attributed_pet_glyph(
    text: &str,
    font_size: f64,
    color: &RoundColor,
) -> Retained<NSMutableAttributedString> {
    unsafe {
        let text = NSString::from_str(text);
        let font = NSFont::monospacedSystemFontOfSize_weight(font_size, 0.0);
        let mut attr = NSMutableAttributedString::from_nsstring(&text);
        let range = objc2_foundation::NSRange::from(0..text.length());
        attr.addAttribute_value_range(NSFontAttributeName, &font, range);
        attr.addAttribute_value_range(NSForegroundColorAttributeName, &ns_color(color), range);
        attr
    }
}

fn ns_color(color: &RoundColor) -> Retained<NSColor> {
    unsafe {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            color.0 as f64,
            color.1 as f64,
            color.2 as f64,
            color.3 as f64,
        )
    }
}

/// Metrics needed to map a character-cell grid onto the round AppKit view.
///
/// `font_size` is the derived monospace font size in points (derived from
/// `view_w` and `COMPANION_TARGET_COLS`).  `cell_w`/`cell_h` are the measured
/// dimensions of one `"M"` glyph at that size.  `grid_cols` and `grid_rows`
/// are the number of cells that fit inside `view_w`/`view_h`.
/// `origin_x`/`origin_y` are the AppKit pixel coordinates of the top-left
/// corner of the grid (centred in the view), where `origin_y` is the
/// **top** of row-0 in AppKit's Y-up coordinate system.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CompanionGridMetrics {
    pub font_size: f64,
    pub cell_w: f64,
    pub cell_h: f64,
    pub grid_cols: u16,
    pub grid_rows: u16,
    pub origin_x: f64,
    pub origin_y: f64,
}

/// Target number of columns across the full view width.
///
/// **TUNABLE** — the pet is PET_W=13 of these columns; FEWER cols = bigger
/// pet/glyphs.  Lower this for tiny displays (e.g. a 480px 2.1″ round screen);
/// raise it for large desktop windows.  The font size is *derived* from this
/// value and the actual view width, so the pet stays a consistent fraction of
/// the display regardless of window size.
const COMPANION_TARGET_COLS: u16 = 36;

/// Probe font size used to measure "M" advance ratio.
///
/// The ratio (advance/size) is stable over a wide range; we probe at this size
/// then scale the result to the target cell width.
const COMPANION_PROBE_FONT_SIZE: f64 = 16.0;

/// Measure the cell grid for the given view dimensions and compute the centred
/// `origin_x`/`origin_y` offset so the grid is positioned in the middle of the
/// AppKit view.
///
/// Font size is derived from `view_w` and `COMPANION_TARGET_COLS` so the pet
/// is always a consistent fraction of the display.  Increasing
/// `COMPANION_TARGET_COLS` → more, smaller columns; decreasing it → fewer,
/// larger columns (bigger pet).
///
/// Only compiled on macOS; not golden-tested (AppKit font measurement is
/// machine-dependent and not deterministic on non-macOS hosts).
pub(super) fn companion_grid_metrics(view_w: f64, view_h: f64) -> Option<CompanionGridMetrics> {
    unsafe {
        // 1. Measure "M" advance at the probe size to get the advance/size ratio.
        let probe_size = attributed_pet_glyph(
            "M",
            COMPANION_PROBE_FONT_SIZE,
            &RoundColor(1.0, 1.0, 1.0, 1.0),
        )
        .size();
        let probe_advance = probe_size.width;
        if probe_advance <= 0.0 {
            return None;
        }

        // 2. Desired cell width from the target column count.
        let cell_w = view_w / COMPANION_TARGET_COLS as f64;

        // 3. Derive font size so "M" advance ≈ cell_w (measured ratio, no hardcoding).
        let font_size = COMPANION_PROBE_FONT_SIZE * cell_w / probe_advance;
        if font_size <= 0.0 {
            return None;
        }

        // 4. Measure actual cell height at the derived font size.
        let cell_size =
            attributed_pet_glyph("M", font_size, &RoundColor(1.0, 1.0, 1.0, 1.0)).size();
        let cell_h = cell_size.height;
        if cell_h <= 0.0 {
            return None;
        }

        let grid_cols = COMPANION_TARGET_COLS;
        let grid_rows = (view_h / cell_h).floor() as u16;
        if grid_cols == 0 || grid_rows == 0 {
            return None;
        }
        let total_grid_w = grid_cols as f64 * cell_w;
        let total_grid_h = grid_rows as f64 * cell_h;
        // origin_x: left edge of the grid (AppKit X-right).
        let origin_x = (view_w - total_grid_w) / 2.0;
        // origin_y: top of row-0 in AppKit Y-up coordinates (AppKit bottom is y=0,
        // top is y=view_h, so the top of the grid is view_h - top_margin).
        let origin_y = (view_h + total_grid_h) / 2.0;
        Some(CompanionGridMetrics {
            font_size,
            cell_w,
            cell_h,
            grid_cols,
            grid_rows,
            origin_x,
            origin_y,
        })
    }
}

/// Convert a (col, row) cell coordinate to AppKit pixel coordinates.
///
/// AppKit Y is up: row 0 top-left maps to `origin_y - cell_h` (the top of
/// row 0 is `origin_y`; the bottom is `origin_y - cell_h`). This mirrors the
/// math in `draw_pet_art_block` exactly.
fn cell_to_point(
    col: u16,
    row: u16,
    cell_w: f64,
    cell_h: f64,
    origin_x: f64,
    origin_y: f64,
) -> (f64, f64) {
    let px = origin_x + col as f64 * cell_w;
    let py = origin_y - (row + 1) as f64 * cell_h;
    (px, py)
}

/// Blit a [`crate::presentation::SceneDrawList`] to the current AppKit
/// graphics context. The caller is responsible for installing the aperture
/// clip before calling (as `draw_scene` already does).
///
/// Cells are drawn in list order (z-order: later entries paint over earlier
/// ones). For each cell:
/// - If `cell.bg` is set, fill the cell rectangle with the background color.
/// - If `cell.glyph` is set, draw the glyph string at the cell origin.
///
/// AppKit rendering is exercised only at runtime; the pixel-math helper
/// `cell_to_point` is separately unit-tested.
fn appkit_blit_draw_list(
    list: &crate::presentation::SceneDrawList,
    font_size: f64,
    cell_w: f64,
    cell_h: f64,
    origin_x: f64,
    origin_y: f64,
) {
    unsafe {
        for cell in &list.cells {
            if cell.bg.is_none() && cell.glyph.is_none() {
                continue;
            }

            let (px, py) = cell_to_point(cell.col, cell.row, cell_w, cell_h, origin_x, origin_y);

            if let Some(bg) = &cell.bg {
                let bg_color = rgb_color(bg.r, bg.g, bg.b);
                let path = NSBezierPath::bezierPathWithRect(NSRect::new(
                    NSPoint::new(px, py),
                    NSSize::new(cell_w, cell_h),
                ));
                ns_color(&bg_color).setFill();
                path.fill();
            }

            if let Some(glyph) = &cell.glyph {
                let fg = cell
                    .fg
                    .as_ref()
                    .map(|c| rgb_color(c.r, c.g, c.b))
                    .unwrap_or(RoundColor(1.0, 1.0, 1.0, 1.0));
                let attr = if cell.bold {
                    // `attributed_pet_glyph` uses weight 0.0 (NSFontWeightRegular).
                    // For bold cells we build the attributed string with NSFontWeightBold.
                    let text = NSString::from_str(glyph);
                    let font =
                        NSFont::monospacedSystemFontOfSize_weight(font_size, NSFontWeightBold);
                    let mut a = NSMutableAttributedString::from_nsstring(&text);
                    let range = objc2_foundation::NSRange::from(0..text.length());
                    a.addAttribute_value_range(NSFontAttributeName, &font, range);
                    a.addAttribute_value_range(
                        NSForegroundColorAttributeName,
                        &ns_color(&fg),
                        range,
                    );
                    a
                } else {
                    attributed_pet_glyph(glyph, font_size, &fg)
                };
                attr.drawAtPoint(NSPoint::new(px, py));
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ambient HUD — pure layout helpers (no AppKit; unit-testable)
// ─────────────────────────────────────────────────────────────────────────────

/// One gauge's layout rectangle in AppKit view coordinates.
///
/// `track_x`/`track_y` is the bottom-left of the track.
/// `fill_w` is the filled-bar width = `track_w * fraction.clamp(0,1)`.
/// `label_x`/`label_y` are text anchor (bottom-left), relevant only when
/// labels are enabled (`HUD_GAUGE_LABEL_FONT_FRAC > 0`).
#[derive(Debug, Clone, PartialEq)]
struct GaugeLayout {
    track_x: f64,
    track_y: f64,
    track_w: f64,
    fill_w: f64,
    label_x: f64,
    label_y: f64,
}

/// Layout all three vital gauges (fed / happy / energy) given the aperture and
/// view dimensions.
///
/// Returns `None` if the aperture is too small to meaningfully render.
///
/// `fractions` — [fed, happy, energy], each clamped internally to 0.0–1.0.
/// `track_h` — pixel height of the track bar; callers derive it from the
/// scaled font size via `HUD_GAUGE_TRACK_H_FRAC`.
fn hud_gauge_layouts(
    view_h: f64,
    aperture_cx: f64,
    aperture_r: f64,
    fractions: [f64; 3],
    track_h: f64,
) -> Option<[GaugeLayout; 3]> {
    if aperture_r < 20.0 {
        return None;
    }

    // Y position of gauge row centerline in AppKit coords (y=0 at bottom).
    // HUD_GAUGE_ROW_Y_FRAC is fraction DOWN from top → distance from bottom.
    let row_center_y = view_h * (1.0 - HUD_GAUGE_ROW_Y_FRAC);

    // Chord half-width at this Y position inside the circle.
    let dy = row_center_y - (view_h / 2.0); // offset from circle center
    let chord_sq = aperture_r * aperture_r - dy * dy;
    if chord_sq <= 0.0 {
        return None;
    }
    let chord_half = chord_sq.sqrt();
    let chord = 2.0 * chord_half;

    // Total bar width = fraction of chord at this row.
    let total_bar_w = chord * HUD_GAUGE_TOTAL_WIDTH_FRAC;
    // Each individual gauge bar width (3 bars + 2 gaps).
    let bar_w = ((total_bar_w - HUD_GAUGE_BAR_GAP * 2.0) / 3.0).max(4.0);
    let total_used_w = bar_w * 3.0 + HUD_GAUGE_BAR_GAP * 2.0;

    let track_y = row_center_y - track_h / 2.0;
    let start_x = aperture_cx - total_used_w / 2.0;

    let mut result: [GaugeLayout; 3] = core::array::from_fn(|_| GaugeLayout {
        track_x: 0.0,
        track_y: 0.0,
        track_w: 0.0,
        fill_w: 0.0,
        label_x: 0.0,
        label_y: 0.0,
    });

    for (i, &frac) in fractions.iter().enumerate() {
        let frac = frac.clamp(0.0, 1.0);
        let bar_left = start_x + i as f64 * (bar_w + HUD_GAUGE_BAR_GAP);
        result[i] = GaugeLayout {
            track_x: bar_left,
            track_y,
            track_w: bar_w,
            fill_w: bar_w * frac,
            label_x: bar_left,
            label_y: track_y + HUD_GAUGE_LABEL_OFFSET_Y,
        };
    }
    Some(result)
}

/// Layout for the hero evolve (XP progress) bar.
///
/// The bar spans `HUD_EVOLVE_BAR_WIDTH_FRAC` of the circle chord at
/// `HUD_EVOLVE_BAR_Y_FRAC`, centered horizontally on `aperture_cx`.
///
/// Returns `None` if the aperture is too small or the bar row falls outside
/// the circle.
///
/// `fraction` — progress toward next stage, 0.0–1.0 (clamped internally).
/// `bar_h` — pixel height of the bar; derive from `font_size * HUD_EVOLVE_BAR_H_FRAC`.
#[derive(Debug, Clone, PartialEq)]
struct EvolveBarLayout {
    track_x: f64,
    track_y: f64,
    track_w: f64,
    bar_h: f64,
    fill_w: f64,
}

fn hud_evolve_bar_layout(
    view_h: f64,
    aperture_cx: f64,
    aperture_r: f64,
    fraction: f64,
    bar_h: f64,
) -> Option<EvolveBarLayout> {
    if aperture_r < 20.0 {
        return None;
    }

    // Y centerline of the evolve bar in AppKit coords (y=0 at bottom).
    let row_center_y = view_h * (1.0 - HUD_EVOLVE_BAR_Y_FRAC);

    // Chord half-width at this row.
    let dy = row_center_y - (view_h / 2.0);
    let chord_sq = aperture_r * aperture_r - dy * dy;
    if chord_sq <= 0.0 {
        return None;
    }
    let chord = 2.0 * chord_sq.sqrt();

    let track_w = (chord * HUD_EVOLVE_BAR_WIDTH_FRAC).max(8.0);
    let track_x = aperture_cx - track_w / 2.0;
    let track_y = row_center_y - bar_h / 2.0;
    let fill_frac = fraction.clamp(0.0, 1.0);

    Some(EvolveBarLayout {
        track_x,
        track_y,
        track_w,
        bar_h,
        fill_w: track_w * fill_frac,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Ambient HUD — AppKit draw call (macOS only)
// ─────────────────────────────────────────────────────────────────────────────

/// Draw the ambient HUD overlay. Must be called inside an active AppKit
/// drawing context with the circular aperture clip already installed.
///
/// `font_size` is the derived grid font size; all HUD text and bar thicknesses
/// scale proportionally via the `HUD_*_FRAC` constants.
///
/// Visual hierarchy (top → bottom):
/// 1. Tiny ambient vital ticks (fed/happy/energy) at the top rim — dim, no labels.
/// 2. HERO token count (large) + secondary rate line just below it.
/// 3. HERO evolve bar (wide, chunky) — stage progress toward next form.
#[cfg(target_os = "macos")]
fn draw_hud(
    bounds: NSRect,
    aperture: &RoundAperture,
    vm: &crate::tui::view_model::WatchViewModel,
    font_size: f64,
) {
    let view_h = bounds.size.height;
    let cx = aperture.center_x as f64;
    let r = aperture.radius as f64;

    // Derived sizes.
    let gauge_track_h = font_size * HUD_GAUGE_TRACK_H_FRAC;
    let gauge_bar_h = font_size * HUD_GAUGE_BAR_H_FRAC;
    let token_font_size = font_size * HUD_TOKEN_FONT_FRAC;
    let rate_font_size = font_size * HUD_RATE_FONT_FRAC;
    let evolve_bar_h = font_size * HUD_EVOLVE_BAR_H_FRAC;
    let evolve_label_size = font_size * HUD_EVOLVE_LABEL_FONT_FRAC;

    let token_color = RoundColor(
        HUD_COLOR_TOKEN.0 as f32 / 255.0,
        HUD_COLOR_TOKEN.1 as f32 / 255.0,
        HUD_COLOR_TOKEN.2 as f32 / 255.0,
        HUD_COLOR_TOKEN.3 as f32,
    );
    let text_color = RoundColor(
        HUD_COLOR_TEXT.0 as f32 / 255.0,
        HUD_COLOR_TEXT.1 as f32 / 255.0,
        HUD_COLOR_TEXT.2 as f32 / 255.0,
        HUD_COLOR_TEXT.3 as f32,
    );
    let track_color = RoundColor(
        HUD_COLOR_TRACK.0 as f32 / 255.0,
        HUD_COLOR_TRACK.1 as f32 / 255.0,
        HUD_COLOR_TRACK.2 as f32 / 255.0,
        HUD_COLOR_TRACK.3 as f32,
    );

    // ── 1. Ambient vital ticks (top rim — dim, no labels) ────────────────────
    let fracs = [vm.fed, vm.happiness, vm.energy];
    // Vital fills use their hue at a low alpha so they read as ambient status.
    let vital_fills: [(u8, u8, u8); 3] = [HUD_COLOR_FED, HUD_COLOR_HAPPY, HUD_COLOR_ENERGY];

    if let Some(layouts) = hud_gauge_layouts(view_h, cx, r, fracs, gauge_track_h) {
        unsafe {
            for (i, layout) in layouts.iter().enumerate() {
                // Dim track.
                let track_path = NSBezierPath::bezierPathWithRect(NSRect::new(
                    NSPoint::new(layout.track_x, layout.track_y),
                    NSSize::new(layout.track_w, gauge_track_h),
                ));
                ns_color(&track_color).setFill();
                track_path.fill();

                // Dimmed fill tick.
                if layout.fill_w > 0.0 {
                    let (fr, fg, fb) = vital_fills[i];
                    let fill_color = RoundColor(
                        fr as f32 / 255.0,
                        fg as f32 / 255.0,
                        fb as f32 / 255.0,
                        HUD_VITAL_FILL_ALPHA,
                    );
                    let fill_path = NSBezierPath::bezierPathWithRect(NSRect::new(
                        NSPoint::new(layout.track_x, layout.track_y),
                        NSSize::new(layout.fill_w, gauge_bar_h),
                    ));
                    ns_color(&fill_color).setFill();
                    fill_path.fill();
                }
                // No labels — vitals are ambient only.
                let _ = gauge_bar_h; // suppress unused-variable lint
            }
        }
    }

    // ── 2. Hero token count + secondary rate line ─────────────────────────────
    let today_str = crate::format::format_tokens(vm.today_effective_tokens);
    let rate_str = crate::format::format_tokens(vm.progress.rate_per_hour);

    unsafe {
        // Large token count — the hero number.
        let token_attr = attributed_pet_glyph(&today_str, token_font_size, &token_color);
        let token_w = token_attr.size().width;
        let token_y = view_h * (1.0 - HUD_TOKEN_Y_FRAC);
        let token_x = cx - token_w / 2.0;
        token_attr.drawAtPoint(NSPoint::new(token_x, token_y));

        // Smaller secondary line: "today  ·  {rate}/hr"
        let rate_line = format!("today  ·  {rate_str}/hr");
        let rate_attr = attributed_pet_glyph(&rate_line, rate_font_size, &text_color);
        let rate_w = rate_attr.size().width;
        let rate_y = view_h * (1.0 - HUD_RATE_Y_FRAC);
        let rate_x = cx - rate_w / 2.0;
        rate_attr.drawAtPoint(NSPoint::new(rate_x, rate_y));
    }

    // ── 3. Hero evolve bar (stage progress) ───────────────────────────────────
    let evolve_frac = vm.progress.fraction as f64;
    if let Some(eb) = hud_evolve_bar_layout(view_h, cx, r, evolve_frac, evolve_bar_h) {
        let evolve_track_color = RoundColor(
            HUD_COLOR_EVOLVE_TRACK.0 as f32 / 255.0,
            HUD_COLOR_EVOLVE_TRACK.1 as f32 / 255.0,
            HUD_COLOR_EVOLVE_TRACK.2 as f32 / 255.0,
            HUD_COLOR_EVOLVE_TRACK.3 as f32,
        );
        let evolve_fill_color = rgb_color(
            HUD_COLOR_EVOLVE_FILL.0,
            HUD_COLOR_EVOLVE_FILL.1,
            HUD_COLOR_EVOLVE_FILL.2,
        );

        unsafe {
            // Track.
            let track_path = NSBezierPath::bezierPathWithRect(NSRect::new(
                NSPoint::new(eb.track_x, eb.track_y),
                NSSize::new(eb.track_w, eb.bar_h),
            ));
            ns_color(&evolve_track_color).setFill();
            track_path.fill();

            // Fill (or full bar when is_max_stage).
            let fill_w = if vm.progress.is_max_stage {
                eb.track_w
            } else {
                eb.fill_w
            };
            if fill_w > 0.0 {
                let fill_path = NSBezierPath::bezierPathWithRect(NSRect::new(
                    NSPoint::new(eb.track_x, eb.track_y),
                    NSSize::new(fill_w, eb.bar_h),
                ));
                ns_color(&evolve_fill_color).setFill();
                fill_path.fill();
            }

            // Stage label left of bar, next-stage label right of bar.
            let label_y = eb.track_y + eb.bar_h + HUD_EVOLVE_LABEL_GAP;
            let stage_attr =
                attributed_pet_glyph(&vm.progress.stage_label, evolve_label_size, &text_color);
            stage_attr.drawAtPoint(NSPoint::new(eb.track_x, label_y));

            let next_label = if vm.progress.is_max_stage {
                "max".to_string()
            } else {
                vm.progress.next_stage_label.clone()
            };
            let next_attr = attributed_pet_glyph(&next_label, evolve_label_size, &text_color);
            let next_w = next_attr.size().width;
            next_attr.drawAtPoint(NSPoint::new(eb.track_x + eb.track_w - next_w, label_y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_to_point_row_zero_sits_at_top_of_origin() {
        // Row 0, col 0: px = origin_x, py = origin_y - cell_h (AppKit Y-up).
        let (px, py) = cell_to_point(0, 0, 10.0, 14.0, 5.0, 100.0);
        assert_eq!(px, 5.0);
        assert_eq!(py, 86.0); // 100.0 - (0 + 1) * 14.0
    }

    #[test]
    fn cell_to_point_advances_right_and_down() {
        // Col 3, row 2: px = 5 + 3*10 = 35; py = 100 - (2+1)*14 = 58.
        let (px, py) = cell_to_point(3, 2, 10.0, 14.0, 5.0, 100.0);
        assert_eq!(px, 35.0);
        assert_eq!(py, 58.0);
    }

    #[test]
    fn companion_menu_spec_wires_standard_quit_shortcut() {
        assert_eq!(
            companion_menu_spec(),
            CompanionMenuSpec {
                app_title: "Glorp",
                quit_title: "Quit Glorp",
                quit_key: "q",
            }
        );
    }

    #[test]
    fn companion_animation_rerenders_pet_art_between_polls() {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_render.generated_species = crate::pet::generation::Species::Glitch;
        let before = vm.pet_art.clone();

        let changed =
            advance_companion_animation(&mut vm, 37, time::macros::datetime!(2026-06-13 18:00 UTC))
                .unwrap();

        assert!(changed);
        assert_ne!(vm.pet_art, before);
    }

    // ── Ambient HUD vital gauge layout tests ─────────────────────────────────

    #[test]
    fn hud_gauge_layouts_returns_none_for_tiny_aperture() {
        // radius < 20 → no layout.
        assert!(hud_gauge_layouts(40.0, 20.0, 10.0, [1.0, 1.0, 1.0], 4.0).is_none());
    }

    #[test]
    fn hud_gauge_layouts_returns_three_rects_for_normal_aperture() {
        // 360×360 window, aperture centered, radius 179.
        let layouts = hud_gauge_layouts(360.0, 180.0, 179.0, [1.0, 0.5, 0.0], 4.0);
        let layouts = layouts.expect("should produce layouts for a 360pt window");
        assert_eq!(layouts.len(), 3);
    }

    #[test]
    fn hud_gauge_layouts_clamps_fractions() {
        let layouts = hud_gauge_layouts(360.0, 180.0, 179.0, [1.5, -0.2, 0.5], 4.0).unwrap();
        // Gauge 0 fraction clamped to 1.0 → fill_w == track_w.
        assert!((layouts[0].fill_w - layouts[0].track_w).abs() < 1e-9);
        // Gauge 1 fraction clamped to 0.0 → fill_w == 0.
        assert!((layouts[1].fill_w).abs() < 1e-9);
        // Gauge 2 fraction 0.5 → fill_w == track_w * 0.5.
        let expected = layouts[2].track_w * 0.5;
        assert!((layouts[2].fill_w - expected).abs() < 1e-9);
    }

    #[test]
    fn hud_gauge_layouts_bars_are_evenly_spaced() {
        let layouts = hud_gauge_layouts(360.0, 180.0, 179.0, [1.0, 1.0, 1.0], 4.0).unwrap();
        // Each bar has the same track_w.
        assert!((layouts[0].track_w - layouts[1].track_w).abs() < 1e-9);
        assert!((layouts[1].track_w - layouts[2].track_w).abs() < 1e-9);
        // Gaps between consecutive bars equal HUD_GAUGE_BAR_GAP.
        let gap_0_1 = layouts[1].track_x - (layouts[0].track_x + layouts[0].track_w);
        let gap_1_2 = layouts[2].track_x - (layouts[1].track_x + layouts[1].track_w);
        assert!((gap_0_1 - HUD_GAUGE_BAR_GAP).abs() < 1e-9);
        assert!((gap_1_2 - HUD_GAUGE_BAR_GAP).abs() < 1e-9);
    }

    #[test]
    fn hud_gauge_layouts_is_horizontally_centered() {
        let layouts = hud_gauge_layouts(360.0, 180.0, 179.0, [0.5, 0.5, 0.5], 4.0).unwrap();
        // Total span from left of first bar to right of last bar.
        let left = layouts[0].track_x;
        let right = layouts[2].track_x + layouts[2].track_w;
        let mid = (left + right) / 2.0;
        assert!((mid - 180.0).abs() < 1e-6);
    }

    // ── Hero evolve bar layout tests ──────────────────────────────────────────

    #[test]
    fn hud_evolve_bar_layout_returns_none_for_tiny_aperture() {
        assert!(hud_evolve_bar_layout(40.0, 20.0, 10.0, 0.5, 4.0).is_none());
    }

    #[test]
    fn hud_evolve_bar_layout_is_horizontally_centered() {
        // 360×360 window, aperture centered at 180, radius 179.
        let eb = hud_evolve_bar_layout(360.0, 180.0, 179.0, 0.5, 6.0)
            .expect("should produce layout for 360pt window");
        let mid = eb.track_x + eb.track_w / 2.0;
        assert!((mid - 180.0).abs() < 1e-6);
    }

    #[test]
    fn hud_evolve_bar_layout_clamps_fraction() {
        // fraction > 1.0 → fill_w == track_w
        let eb = hud_evolve_bar_layout(360.0, 180.0, 179.0, 1.5, 6.0).unwrap();
        assert!((eb.fill_w - eb.track_w).abs() < 1e-9);

        // fraction < 0.0 → fill_w == 0
        let eb = hud_evolve_bar_layout(360.0, 180.0, 179.0, -0.3, 6.0).unwrap();
        assert!((eb.fill_w).abs() < 1e-9);
    }

    #[test]
    fn hud_evolve_bar_layout_fill_proportional_to_fraction() {
        let eb = hud_evolve_bar_layout(360.0, 180.0, 179.0, 0.75, 6.0).unwrap();
        let expected = eb.track_w * 0.75;
        assert!((eb.fill_w - expected).abs() < 1e-9);
    }

    #[test]
    fn hud_evolve_bar_layout_wider_than_vital_ticks() {
        // The evolve bar should be wider than the combined vital tick span —
        // it's the hero element.
        let eb = hud_evolve_bar_layout(360.0, 180.0, 179.0, 0.5, 6.0).unwrap();
        let gauge_track_h = 4.0;
        let gauge_layouts =
            hud_gauge_layouts(360.0, 180.0, 179.0, [1.0, 1.0, 1.0], gauge_track_h).unwrap();
        let vital_span =
            gauge_layouts[2].track_x + gauge_layouts[2].track_w - gauge_layouts[0].track_x;
        assert!(
            eb.track_w > vital_span,
            "evolve bar ({:.1}) should be wider than vital ticks ({:.1})",
            eb.track_w,
            vital_span
        );
    }
}
