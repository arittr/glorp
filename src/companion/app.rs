//! Native macOS round companion window. Uses a regular Dock app lifecycle,
//! a worker thread for live usage polling, and pure AppKit drawing from
//! `RoundSceneModel`.

#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::sync::mpsc;
use std::time::Duration;

use crate::commands::companion_mode::CompanionRendererMode;
use crate::commands::watch::{
    build_watch_view_model_at, build_watch_view_model_semantic_at, rerender_pet_for_view_model,
};
use crate::companion::render::{build_draw_commands, RoundColor, RoundDrawKind};
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::presentation::pixel::{
    render_pixel_frame, PixelFrame, PixelPetInput, PixelRendererState, PixelRendererTick,
    PixelViewport,
};
use crate::round::hud::{
    companion_hud_text, companion_pace_fraction, daily_fraction_for_gauge, daily_overage_color,
    daily_overage_marker_arc, daily_overage_marker_fraction, growth_ring_fill_end_deg,
    perimeter_gauge_colors, perimeter_gauge_layout, CompanionHudText, GaugeLane, GaugeLaneColors,
    LineCap, COMPANION_GAUGE_GAP_DEG,
};
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundSceneModel};
use crate::storage::state::StateStore;
use crate::tui::view_model::WatchViewModel;
use crate::watch_live::{LiveWatchRenderMode, LiveWatchUpdate, WatchPresentationState};
use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAttributedStringNSStringDrawing,
    NSBackingStoreType, NSBezierPath, NSButtLineCapStyle, NSColor, NSCommandKeyMask,
    NSControlKeyMask, NSEventModifierFlags, NSFont, NSFontAttributeName, NSFontWeightBold,
    NSForegroundColorAttributeName, NSLineCapStyle, NSMenu, NSMenuItem, NSRoundLineCapStyle,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask, NSWindowTitleVisibility,
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

/// The companion's drift config (tuned on device). Starts at the legacy default;
/// diverge here WITHOUT touching the shared menubar popover.
fn companion_motion() -> crate::round::scene::CompanionMotion {
    crate::round::scene::companion_roam_motion()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompanionMenuSpec {
    app_title: &'static str,
    quit_title: &'static str,
    quit_key: &'static str,
    fullscreen_title: &'static str,
    fullscreen_key: &'static str,
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
    renderer_mode: CompanionRendererMode,
    pixel_input: Option<PixelPetInput>,
    pixel_state: Option<PixelRendererState>,
    pixel_frame: Option<PixelFrame>,
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

pub fn run(renderer_mode: CompanionRendererMode) -> Result<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| GlorpError::Message("glorp companion must run on the main thread".into()))?;
    let paths = AppPaths::resolve()?;
    paths.ensure()?;
    let state_store = StateStore::new(paths.state_file.clone());
    let Some(mut initial_pet) = state_store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };
    let now = time::OffsetDateTime::now_utc();
    crate::commands::watch::reconcile_state_after_load(
        &state_store,
        &mut initial_pet,
        now,
        crate::storage::day_axis::LocalDayMapper::System,
    )?;
    let initial_vm = if renderer_mode.is_pixel() {
        build_watch_view_model_semantic_at(
            &initial_pet,
            &paths.usage_db,
            now,
            crate::storage::day_axis::LocalDayMapper::System,
        )?
    } else {
        build_watch_view_model_at(
            &initial_pet,
            &paths.usage_db,
            now,
            crate::storage::day_axis::LocalDayMapper::System,
        )?
    };
    let scene = derive_round_scene_model(&initial_vm, now);
    let pixel_input = renderer_mode
        .is_pixel()
        .then(|| PixelPetInput::from_watch_view_model(&initial_vm, now));
    let pixel_state = pixel_input
        .as_ref()
        .map(|input| PixelRendererState::new(input, now));
    let pixel_frame = None;

    let app: Retained<NSApplication> = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    install_app_menu(&app, mtm);

    let controller: Retained<Controller> = unsafe { msg_send_id![Controller::class(), new] };
    let (window, view) = build_window(mtm);
    let poll_rx = crate::watch_live::spawn_live_watch_worker(
        paths,
        POLL_INTERVAL,
        "glorp-companion-poll",
        if renderer_mode.is_pixel() {
            LiveWatchRenderMode::Semantic
        } else {
            LiveWatchRenderMode::Rendered
        },
    );

    APP_STATE.with(|cell| {
        *cell.borrow_mut() = Some(AppState {
            window,
            view,
            poll_rx,
            presentation_state: WatchPresentationState::default(),
            vm: initial_vm,
            scene,
            renderer_mode,
            pixel_input,
            pixel_state,
            pixel_frame,
            animation_frame: 0,
        });
    });

    let tick_interval = if renderer_mode.is_pixel() {
        1.0 / 30.0
    } else {
        UI_TICK_INTERVAL_SECS
    };
    let _timer: Retained<NSTimer> = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            tick_interval,
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
        fullscreen_title: "Enter Full Screen",
        fullscreen_key: "f",
    }
}

fn install_app_menu(app: &NSApplication, mtm: MainThreadMarker) {
    let spec = companion_menu_spec();
    unsafe {
        let main_menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(""));

        // ── App menu (Glorp → Quit) ──────────────────────────────────────────
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

        // ── View menu (View → Enter Full Screen ⌃⌘F) ────────────────────────
        let view_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str("View"),
            None,
            &NSString::from_str(""),
        );
        main_menu.addItem(&view_item);

        let view_menu = NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str("View"));
        let fs_item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(spec.fullscreen_title),
            Some(sel!(toggleFullScreen:)),
            &NSString::from_str(spec.fullscreen_key),
        );
        // ⌃⌘F — standard macOS Enter Full Screen shortcut.
        // target is nil so the action routes to the key window.
        fs_item.setKeyEquivalentModifierMask(NSEventModifierFlags(
            NSControlKeyMask.0 | NSCommandKeyMask.0,
        ));
        view_menu.addItem(&fs_item);
        main_menu.setSubmenu_forItem(Some(&view_menu), &view_item);

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
        // Make the window fullscreen-capable (green zoom button → fullscreen toggle).
        window.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenPrimary);
        // Transparent titlebar + hidden title so the round window looks clean in
        // windowed mode (traffic-light buttons remain functional).
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::NSWindowTitleHidden);
        // Allow dragging the window by its body since the title bar is invisible.
        window.setMovableByWindowBackground(true);
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
            if state.renderer_mode.is_pixel() {
                state.pixel_input = Some(PixelPetInput::from_watch_view_model(&vm, now));
            }
            state.scene = derive_round_scene_model(&vm, now);
            state.vm = vm;
            unsafe { state.view.setNeedsDisplay(true) };
        }
    });
}

fn animate_pet() {
    let view = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        if state.renderer_mode.is_pixel() {
            let now = time::OffsetDateTime::now_utc();
            if let Some(pixel_state) = state.pixel_state.as_mut() {
                let (pixel_frame, pixel_input) =
                    render_live_pixel_frame(&state.vm, pixel_state, now);
                state.pixel_input = Some(pixel_input);
                state.pixel_frame = Some(pixel_frame);
            }
            return Some(state.view.clone());
        }
        let next_frame = state.animation_frame.wrapping_add(1);
        let now = time::OffsetDateTime::now_utc();
        let _ = advance_companion_animation(&mut state.vm, next_frame, now);
        state.animation_frame = next_frame;
        state.scene = derive_round_scene_model(&state.vm, now);
        // Repaint every tick: the pet's drift position is time-based
        // (companion_drift reads `now`), so the scene must redraw to animate the
        // free-float roam even when vitals are unchanged.
        Some(state.view.clone())
    });
    if let Some(view) = view {
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
    rerender_pet_for_view_model(vm, frame, vm.day_context.asleep, now)?;
    let species = vm.pet_render.generated_species;
    let rhythm = crate::pet::animator::breath_rhythm_for_day(&vm.day_context);
    vm.breath_offset_y =
        crate::pet::animator::compute_breath_offset_with_rhythm(Some(species), now, rhythm);
    Ok(vm.pet_art != prev_pet_art
        || vm.pet_spans != prev_pet_spans
        || vm.breath_offset_y != prev_breath_offset_y)
}

fn render_live_pixel_frame(
    vm: &WatchViewModel,
    pixel_state: &mut PixelRendererState,
    now: time::OffsetDateTime,
) -> (PixelFrame, PixelPetInput) {
    let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(vm, now);
    let art_reference = pixel_state.art_reference_for(&request);
    let frame = render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &art_reference,
        viewport: PixelViewport::companion_default(),
        now,
        state: pixel_state,
    });
    (frame, input)
}

fn draw_scene(bounds: NSRect) {
    let _mtm = MainThreadMarker::new().expect("companion draw_scene on non-main thread");
    let state_snapshot = APP_STATE.with(|cell| {
        cell.borrow().as_ref().map(|s| {
            (
                s.scene.clone(),
                s.vm.clone(),
                s.renderer_mode,
                s.pixel_frame.clone(),
            )
        })
    });
    let Some((scene, vm, renderer_mode, pixel_frame)) = state_snapshot else {
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

        // Tank depth: concentric translucent rings, darker toward the rim, so the
        // porthole reads as depth rather than a flat void. (NSGradient isn't bound.)
        const DEPTH_RINGS: usize = 7;
        for i in 0..DEPTH_RINGS {
            let t = i as f64 / DEPTH_RINGS as f64; // 0 center → ~1 rim
            let rr = aperture.radius as f64 * (1.0 - t);
            let ring = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(aperture.center_x as f64 - rr, aperture.center_y as f64 - rr),
                NSSize::new(rr * 2.0, rr * 2.0),
            ));
            // Brighter core (additive translucency builds toward center).
            ns_color(&RoundColor(0.10, 0.11, 0.20, 0.05)).setFill();
            ring.fill();
        }

        // Blit the shared scene draw list (habitat + pet) when grid metrics are available.
        if renderer_mode.is_pixel() {
            if let Some(frame) = pixel_frame.as_ref() {
                crate::companion::pixel::draw_pixel_frame(frame, bounds, aperture);
            }
        } else if let Some(m) = companion_grid_metrics(bounds.size.width, bounds.size.height) {
            let companion_scene = crate::round::scene::build_round_scene_draw_list(
                &vm,
                now,
                m.grid_cols,
                m.grid_rows,
                &companion_motion(),
            );
            // Mood aura — soft radial glow (concentric translucent circles) centered
            // on the pet, color by mood. Drawn under the pet so the body sits on top.
            let pr = companion_scene.pet_rect;
            let (cxp, cyp) = cell_to_point(
                pr.x + pr.width / 2,
                pr.y + pr.height / 2,
                m.cell_w,
                m.cell_h,
                m.origin_x,
                m.origin_y,
            );
            let base = crate::round::hud::mood_aura_color(scene.pet.mood);
            let max_r = pr.width as f64 * m.cell_w * 0.95;
            const AURA_RINGS: usize = 8;
            for i in 0..AURA_RINGS {
                let t = i as f64 / AURA_RINGS as f64; // 0 = outer, 1 = inner
                let rr = max_r * (1.0 - t);
                let glow = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                    NSPoint::new(cxp - rr, cyp - rr),
                    NSSize::new(rr * 2.0, rr * 2.0),
                ));
                ns_color(&RoundColor(base.0, base.1, base.2, 0.05)).setFill();
                glow.fill();
            }

            appkit_blit_draw_list(
                &companion_scene.draw_list,
                m.font_size,
                m.cell_w,
                m.cell_h,
                m.origin_x,
                m.origin_y,
            );
        }

        // Companion perimeter gauges: XP, today vs yesterday, and live 10m pace.
        {
            let cx = aperture.center_x as f64;
            let cy = aperture.center_y as f64;
            let layout =
                perimeter_gauge_layout(cx, cy, aperture.radius as f64, COMPANION_GAUGE_GAP_DEG);
            let colors = perimeter_gauge_colors();
            let xp_fraction = if vm.progress.is_max_stage {
                1.0
            } else {
                vm.progress.fraction as f64
            };
            let daily_ratio = vm.daily_comparison.fraction_of_yesterday;
            let daily_fraction = daily_fraction_for_gauge(daily_ratio);
            let daily_overage_fraction = daily_overage_marker_fraction(daily_ratio);
            let pace_fraction = companion_pace_fraction(vm.rate_momentum.pulse.current_tokens);

            draw_gauge_lane(&layout.xp, &colors.xp, xp_fraction);
            draw_gauge_lane(&layout.daily, &colors.daily, daily_fraction);
            draw_gauge_overfill(
                &layout.daily,
                &daily_overage_color(),
                daily_overage_fraction,
            );
            draw_gauge_lane(&layout.pace, &colors.pace, pace_fraction);
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

#[cfg(target_os = "macos")]
struct CompanionAttributedLine {
    text: Retained<NSMutableAttributedString>,
}

#[cfg(target_os = "macos")]
struct CompanionAttributedStack {
    lines: Vec<CompanionAttributedLine>,
    max_width: f64,
    total_height: f64,
}

#[cfg(target_os = "macos")]
fn companion_hud_attributed_lines(
    text: &CompanionHudText,
    size: f64,
    big_color: &RoundColor,
    sub_color: &RoundColor,
) -> CompanionAttributedStack {
    let big = attributed_pet_glyph(&text.today_total, size * 1.08, big_color);
    let daily = attributed_pet_glyph(&text.daily_percent, size * 0.68, sub_color);
    let pace = attributed_pet_glyph(&text.pace, size * 0.68, sub_color);
    let (max_width, total_height) = unsafe {
        let max_width = big
            .size()
            .width
            .max(daily.size().width)
            .max(pace.size().width);
        let total_height =
            big.size().height + daily.size().height * 0.82 + pace.size().height * 0.82;
        (max_width, total_height)
    };

    CompanionAttributedStack {
        lines: vec![
            CompanionAttributedLine { text: big },
            CompanionAttributedLine { text: daily },
            CompanionAttributedLine { text: pace },
        ],
        max_width,
        total_height,
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
// Pet scale lever: fewer cols → larger cells → bigger pet/props. With the organic
// wander the pet is allowed to swim partly past the round rim, so a bigger pet no
// longer fights its movement. Tuned on device.
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
// Ambient HUD — AppKit draw call (macOS only)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn ns_line_cap(cap: LineCap) -> NSLineCapStyle {
    match cap {
        LineCap::Butt => NSButtLineCapStyle,
        LineCap::Round => NSRoundLineCapStyle,
    }
}

#[cfg(target_os = "macos")]
fn draw_gauge_lane(lane: &GaugeLane, colors: &GaugeLaneColors, fraction: f64) {
    let start = lane.ring.track_start_deg;
    let end = lane.ring.track_start_deg + lane.ring.track_sweep_deg;

    unsafe {
        let track = NSBezierPath::new();
        track.setLineWidth(lane.stroke_width);
        track.setLineCapStyle(ns_line_cap(lane.cap));
        track.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
            NSPoint::new(lane.ring.cx, lane.ring.cy),
            lane.ring.radius,
            start,
            end,
        );
        ns_color(&colors.track).setStroke();
        track.stroke();
    }

    draw_gauge_fill(lane, &colors.fill, fraction);
}

fn draw_gauge_fill(lane: &GaugeLane, color: &RoundColor, fraction: f64) {
    let clamped = fraction.clamp(0.0, 1.0);
    if clamped <= 0.0 {
        return;
    }

    let start = lane.ring.track_start_deg;
    let fill_end = growth_ring_fill_end_deg(&lane.ring, clamped);

    unsafe {
        let fill = NSBezierPath::new();
        fill.setLineWidth(lane.stroke_width);
        fill.setLineCapStyle(ns_line_cap(lane.cap));
        fill.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
            NSPoint::new(lane.ring.cx, lane.ring.cy),
            lane.ring.radius,
            start,
            fill_end,
        );
        ns_color(color).setStroke();
        fill.stroke();
    }
}

fn draw_gauge_overfill(lane: &GaugeLane, color: &RoundColor, fraction: f64) {
    let clamped = fraction.clamp(0.0, 1.0);
    if clamped <= 0.0 {
        return;
    }

    let Some((start, end)) = daily_overage_marker_arc(&lane.ring, clamped) else {
        return;
    };

    unsafe {
        let fill = NSBezierPath::new();
        fill.setLineWidth(lane.stroke_width);
        fill.setLineCapStyle(ns_line_cap(lane.cap));
        fill.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
            NSPoint::new(lane.ring.cx, lane.ring.cy),
            lane.ring.radius,
            start,
            end,
        );
        ns_color(color).setStroke();
        fill.stroke();
    }
}

#[cfg(target_os = "macos")]
fn draw_hud(
    bounds: NSRect,
    aperture: &RoundAperture,
    vm: &crate::tui::view_model::WatchViewModel,
    font_size: f64,
) {
    let gauge_layout = perimeter_gauge_layout(
        aperture.center_x as f64,
        aperture.center_y as f64,
        aperture.radius as f64,
        COMPANION_GAUGE_GAP_DEG,
    );
    let gap = crate::round::hud::stat_gap_box(
        aperture.center_x as f64,
        aperture.center_y as f64,
        gauge_layout.pace.ring.radius - gauge_layout.pace.stroke_width / 2.0,
        COMPANION_GAUGE_GAP_DEG,
    );
    let hud_text = companion_hud_text(
        vm.today_effective_tokens,
        vm.daily_comparison.fraction_of_yesterday,
        vm.rate_momentum.pulse.current_tokens,
    );

    unsafe {
        let big_color = RoundColor(0.93, 0.93, 0.97, 1.0);
        let sub_color =
            crate::round::hud::rate_direction_color(crate::tui::view_model::RateDirection::Neutral);
        let mut stack_size = font_size * 1.45;
        let mut rendered =
            companion_hud_attributed_lines(&hud_text, stack_size, &big_color, &sub_color);

        while (rendered.max_width > gap.max_width
            || rendered.total_height > aperture.radius as f64 * 0.34)
            && stack_size > 6.0
        {
            stack_size -= 1.0;
            rendered =
                companion_hud_attributed_lines(&hud_text, stack_size, &big_color, &sub_color);
        }

        let top = bounds.size.height - gap.baseline_y;
        let mut y = top + rendered.total_height * 0.38;
        for line in rendered.lines {
            let width = line.text.size().width;
            line.text
                .drawAtPoint(NSPoint::new(gap.center_x - width / 2.0, y));
            y -= line.text.size().height * 0.82;
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
                fullscreen_title: "Enter Full Screen",
                fullscreen_key: "f",
            }
        );
    }

    #[test]
    fn companion_hud_stack_uses_daily_percent_and_drops_hour_rate() {
        let text = crate::round::hud::companion_hud_text(842_000_000.0, Some(0.94), 31_000_000.0);

        assert_eq!(text.today_total, "842M");
        assert_eq!(text.daily_percent, "94% yday");
        assert_eq!(text.pace, "31M/10m");
        assert!(!text.pace.contains("/hr"));
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

    #[test]
    fn companion_pixel_tick_recomputes_pulse_age_between_polls() {
        let base = time::macros::datetime!(2026-07-08 12:00 UTC);
        let mut vm = WatchViewModel::fixture();
        vm.pet_render.generated_species = crate::pet::generation::Species::Glitch;
        vm.pet_render.stage = crate::game::evolution::Stage::S4;
        vm.life_profile.burst_level = 0.95;
        vm.last_feed_pulse_at = Some(base);

        let initial_input = PixelPetInput::from_watch_view_model(&vm, base);
        let mut pixel_state = PixelRendererState::new(&initial_input, base);

        let (first_frame, first_input) = render_live_pixel_frame(&vm, &mut pixel_state, base);
        let (late_frame, late_input) = render_live_pixel_frame(
            &vm,
            &mut pixel_state,
            base + time::Duration::milliseconds(1_600),
        );

        let first_accent_alpha = accent_alpha_sum(&first_frame, &first_input);
        let late_accent_alpha = accent_alpha_sum(&late_frame, &late_input);
        assert!(
            late_accent_alpha < first_accent_alpha,
            "live Pixel tick should decay feed-pulse aura without waiting for the next poll: first={first_accent_alpha}, late={late_accent_alpha}"
        );
    }

    fn accent_alpha_sum(frame: &PixelFrame, input: &PixelPetInput) -> u32 {
        frame
            .pixels
            .iter()
            .filter(|pixel| {
                pixel.r == input.palette.accent.r
                    && pixel.g == input.palette.accent.g
                    && pixel.b == input.palette.accent.b
            })
            .map(|pixel| u32::from(pixel.a))
            .sum()
    }
}
