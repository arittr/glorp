//! Native macOS round companion window. Uses a regular Dock app lifecycle,
//! a worker thread for live usage polling, and pure AppKit drawing from
//! `RoundSceneModel`.

#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::sync::mpsc;
use std::time::Duration;

use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAttributedStringNSStringDrawing,
    NSBackingStoreType, NSBezierPath, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString, NSTimer,
};

use crate::commands::watch::build_watch_view_model;
use crate::companion::render::{build_draw_commands, RoundColor, RoundDrawCommand, RoundDrawKind};
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundSceneModel};
use crate::storage::state::StateStore;
use crate::watch_live::{LiveWatchUpdate, WatchPresentationState};

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const UI_TICK_INTERVAL_SECS: f64 = 0.25;
const DEFAULT_WINDOW_SIZE: f64 = 360.0;
const WINDOW_ORIGIN_X: f64 = 120.0;
const WINDOW_ORIGIN_Y: f64 = 120.0;
const MIN_WINDOW_SIZE: f64 = 260.0;
const GLYPH_OFFSET_STEP: f64 = 10.0; // horizontal spacing between cluster glyphs

struct AppState {
    /// Retained to keep the window alive after makeKeyAndOrderFront.
    #[allow(dead_code)]
    window: Retained<NSWindow>,
    view: Retained<RoundView>,
    poll_rx: mpsc::Receiver<LiveWatchUpdate>,
    presentation_state: WatchPresentationState,
    scene: RoundSceneModel,
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
            scene,
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
    let mut latest = None;
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow().as_ref() {
            while let Ok(update) = state.poll_rx.try_recv() {
                latest = Some(update);
            }
        }
    });
    if let Some(update) = latest {
        APP_STATE.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                let mut vm = update.vm;
                crate::watch_live::stamp_live_presentation(
                    &mut state.presentation_state,
                    &mut vm,
                    update.applied_signal,
                    time::OffsetDateTime::now_utc(),
                );
                state.scene = derive_round_scene_model(&vm, time::OffsetDateTime::now_utc());
                unsafe { state.view.setNeedsDisplay(true) };
            }
        });
    }
}

fn draw_scene(bounds: NSRect) {
    let _mtm = MainThreadMarker::new().expect("companion draw_scene on non-main thread");
    let scene = APP_STATE.with(|cell| cell.borrow().as_ref().map(|s| s.scene.clone()));
    let Some(scene) = scene else {
        return;
    };

    let width = bounds.size.width as f32;
    let height = bounds.size.height as f32;
    let aperture = RoundAperture::new(width as u16, height as u16);
    let layout = layout_round_scene(
        &scene,
        aperture,
        RoundRenderCapabilities::preview_truecolor(),
    );
    let commands = build_draw_commands(&scene, &layout);

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

        for command in &commands {
            draw_command(command);
        }

        if dim_overlay {
            let dim = NSBezierPath::bezierPathWithRect(bounds);
            NSColor::colorWithSRGBRed_green_blue_alpha(0.05, 0.06, 0.10, 0.35).setFill();
            dim.fill();
        }
    }
}

fn draw_command(command: &RoundDrawCommand) {
    match command.kind {
        RoundDrawKind::Background => unsafe {
            let path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(
                    (command.x - command.radius) as f64,
                    (command.y - command.radius) as f64,
                ),
                NSSize::new((command.radius * 2.0) as f64, (command.radius * 2.0) as f64),
            ));
            ns_color(&command.color).setFill();
            path.fill();
        },
        RoundDrawKind::PetGlyph => {
            draw_glyph_cluster(command);
        }
        RoundDrawKind::PropGlyph => {
            if let Some(label) = command.label {
                draw_label(label, command.x, command.y, command.radius, &command.color);
            }
        }
        RoundDrawKind::Halo => unsafe {
            let path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(
                    (command.x - command.radius) as f64,
                    (command.y - command.radius) as f64,
                ),
                NSSize::new((command.radius * 2.0) as f64, (command.radius * 2.0) as f64),
            ));
            ns_color(&command.color).setFill();
            path.fill();
        },
        RoundDrawKind::Trouble => unsafe {
            let path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(
                    (command.x - command.radius) as f64,
                    (command.y - command.radius) as f64,
                ),
                NSSize::new((command.radius * 2.0) as f64, (command.radius * 2.0) as f64),
            ));
            ns_color(&command.color).setFill();
            path.fill();
        },
        RoundDrawKind::RoomGlyph => {
            // RoomGlyph is intentionally a no-op in V1; background fill provides the room
            // texture. A future iteration may add sparse dialect glyphs here.
        }
    }
}

fn draw_glyph_cluster(command: &RoundDrawCommand) {
    let step = GLYPH_OFFSET_STEP as f32;
    let glyphs: &[(f32, f32, char)] = &[
        (-step / 2.0, step / 2.0, 'g'),
        (step / 2.0, step / 2.0, 'l'),
        (0.0, -step / 6.0, 'o'),
        (-step / 3.0, -step * 5.0 / 6.0, 'r'),
        (step / 3.0, -step * 5.0 / 6.0, 'p'),
    ];
    for (dx, dy, ch) in glyphs {
        draw_label(
            *ch,
            command.x + *dx,
            command.y + *dy,
            command.radius * 0.55,
            &command.color,
        );
    }
}

fn draw_label(label: char, x: f32, y: f32, radius: f32, color: &RoundColor) {
    unsafe {
        let text = NSString::from_str(&label.to_string());
        let font = NSFont::systemFontOfSize((radius * 1.5) as f64);
        let mut attr = NSMutableAttributedString::from_nsstring(&text);
        let range = objc2_foundation::NSRange::from(0..text.length());
        attr.addAttribute_value_range(NSFontAttributeName, &font, range);
        attr.addAttribute_value_range(NSForegroundColorAttributeName, &ns_color(color), range);
        let size = attr.size();
        let point = NSPoint::new(x as f64 - size.width / 2.0, y as f64 - size.height / 2.0);
        attr.drawAtPoint(point);
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
