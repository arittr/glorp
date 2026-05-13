//! NSApplication + NSStatusItem + NSPopover wiring. The controller object is
//! intentionally state-less; live UI handles live in a thread-local set up by
//! `run` on the main thread, since NSPopover / NSTextView are not `Sync` and
//! only ever touched from the main run loop.

#![cfg(target_os = "macos")]

use std::cell::RefCell;

use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSColor, NSPopover, NSPopoverBehavior,
    NSRectEdgeMinY, NSStatusBar, NSStatusItem, NSTextView, NSView, NSViewController,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRange, NSRect, NSSize, NSString, NSTimer};

use crate::commands::watch::{build_watch_view_model, poll_usage_and_apply};
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::storage::state::{PetState, StateStore};
use crate::tui::view_model::WatchViewModel;

use super::render;

/// `NSStatusItem.length` value meaning "size to fit content". Apple's header
/// declares `NSVariableStatusItemLength = -1.0`.
const NS_VARIABLE_STATUS_ITEM_LENGTH: f64 = -1.0;
const POLL_INTERVAL_SECS: f64 = 10.0;
// Approximate cell width/height for the 13pt monospaced system font. Tuned
// visually so the popover sizes roughly to the content; real text measurement
// can replace this later.
const APPROX_CELL_WIDTH: f64 = 7.6;
const APPROX_CELL_HEIGHT: f64 = 17.0;

struct AppState {
    paths: AppPaths,
    popover: Retained<NSPopover>,
    text_view: Retained<NSTextView>,
    status_item: Retained<NSStatusItem>,
}

thread_local! {
    static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

declare_class!(
    pub(super) struct Controller;

    unsafe impl ClassType for Controller {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "GlorpMenubarController";
    }

    impl DeclaredClass for Controller {}

    unsafe impl Controller {
        #[method(togglePopover:)]
        fn toggle_popover(&self, _sender: Option<&AnyObject>) {
            toggle_popover();
        }

        #[method(pollTick:)]
        fn poll_tick(&self, _sender: Option<&AnyObject>) {
            poll_tick();
        }
    }
);

pub fn run() -> Result<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| GlorpError::Message("glorp menubar must run on the main thread".into()))?;

    let paths = AppPaths::resolve()?;
    paths.ensure()?;
    let state_store = StateStore::new(paths.state_file.clone());
    let initial_pet = state_store.load()?.ok_or_else(|| {
        GlorpError::Message("no glorp pet exists yet; run `glorp init` first".into())
    })?;
    let initial_vm = build_watch_view_model(&initial_pet, &paths.usage_db)?;

    let app: Retained<NSApplication> = unsafe { NSApplication::sharedApplication(mtm) };
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller: Retained<Controller> = unsafe { msg_send_id![Controller::class(), new] };

    let popover_size = NSSize::new(
        (render::POPOVER_COLUMNS as f64) * APPROX_CELL_WIDTH + 24.0,
        (render::POPOVER_ROWS as f64) * APPROX_CELL_HEIGHT + 16.0,
    );
    let (popover, text_view) = build_popover(popover_size);
    update_text_view(&text_view, &initial_vm);

    let status_item = build_status_item(mtm, &controller, &initial_pet)?;

    APP_STATE.with(|cell| {
        *cell.borrow_mut() = Some(AppState {
            paths,
            popover,
            text_view,
            status_item,
        });
    });

    let _timer: Retained<NSTimer> = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            POLL_INTERVAL_SECS,
            &*controller,
            sel!(pollTick:),
            None,
            true,
        )
    };

    unsafe { app.run() };
    Ok(())
}

fn build_popover(size: NSSize) -> (Retained<NSPopover>, Retained<NSTextView>) {
    let view_frame = NSRect::new(NSPoint::new(0.0, 0.0), size);

    let view: Retained<NSView> = unsafe {
        let alloc = NSView::alloc();
        NSView::initWithFrame(alloc, view_frame)
    };

    let text_view: Retained<NSTextView> = unsafe {
        let alloc = NSTextView::alloc();
        NSTextView::initWithFrame(alloc, view_frame)
    };
    unsafe {
        text_view.setEditable(false);
        text_view.setSelectable(true);
        text_view.setDrawsBackground(true);
        text_view.setBackgroundColor(&dark_surface_color());
        text_view.setTextContainerInset(NSSize::new(12.0, 12.0));
        view.addSubview(&text_view);
    }

    let vc: Retained<NSViewController> = unsafe { msg_send_id![NSViewController::class(), new] };
    unsafe { vc.setView(&view) };

    let popover: Retained<NSPopover> = unsafe { msg_send_id![NSPopover::class(), new] };
    unsafe {
        popover.setBehavior(NSPopoverBehavior::Transient);
        popover.setContentSize(size);
        popover.setContentViewController(Some(&vc));
    }

    (popover, text_view)
}

fn build_status_item(
    mtm: MainThreadMarker,
    controller: &Controller,
    initial: &PetState,
) -> Result<Retained<NSStatusItem>> {
    let status_bar = unsafe { NSStatusBar::systemStatusBar() };
    let status_item: Retained<NSStatusItem> =
        unsafe { status_bar.statusItemWithLength(NS_VARIABLE_STATUS_ITEM_LENGTH) };

    let Some(button) = (unsafe { status_item.button(mtm) }) else {
        return Err(GlorpError::Message(
            "could not access status bar button".into(),
        ));
    };

    let title = NSString::from_str(&status_item_title(initial));
    unsafe {
        button.setTitle(&title);
        button.setTarget(Some(controller));
        button.setAction(Some(sel!(togglePopover:)));
    }
    Ok(status_item)
}

fn status_item_title(state: &PetState) -> String {
    // v1: pet name only. v2 will replace this with a tiny tokens-in/out
    // indicator (Little Snitch style network meter).
    format!("· {} ·", state.pet.accepted_name)
}

fn update_text_view(text_view: &NSTextView, vm: &WatchViewModel) {
    let attr = render::render(vm);
    unsafe {
        let storage = text_view
            .textStorage()
            .expect("NSTextView always has a text storage");
        let length = storage.length();
        storage.replaceCharactersInRange_withAttributedString(NSRange::from(0..length), &attr);
    }
}

fn toggle_popover() {
    // Clone the Retained<> handles out so the RefCell borrow drops before
    // we call into AppKit. `Retained<T>` clones are cheap (retain++) and
    // dropping the borrow first avoids re-entrancy if AppKit pumps a nested
    // event during showRelativeToRect:.
    let handles = APP_STATE.with(|cell| {
        cell.borrow().as_ref().map(|s| AppStateHandles {
            popover: s.popover.clone(),
            status_item: s.status_item.clone(),
        })
    });
    let Some(handles) = handles else {
        return;
    };
    let mtm = MainThreadMarker::new().expect("toggle on non-main thread");
    let is_shown = unsafe { handles.popover.isShown() };
    if is_shown {
        unsafe { handles.popover.performClose(None) };
        return;
    }
    let Some(button) = (unsafe { handles.status_item.button(mtm) }) else {
        return;
    };
    let bounds: NSRect = unsafe { button.bounds() };
    unsafe {
        handles
            .popover
            .showRelativeToRect_ofView_preferredEdge(bounds, &button, NSRectEdgeMinY);
    }
}

fn poll_tick() {
    let snapshot = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|s| (s.paths.clone(), s.text_view.clone(), s.status_item.clone()))
    });
    let Some((paths, text_view, status_item)) = snapshot else {
        return;
    };
    let state_store = StateStore::new(paths.state_file.clone());
    let pet_state = match poll_usage_and_apply(&state_store, &paths.usage_db, &paths.config_file) {
        Ok(Some(state)) => state,
        Ok(None) | Err(_) => return,
    };
    if let Ok(vm) = build_watch_view_model(&pet_state, &paths.usage_db) {
        update_text_view(&text_view, &vm);
        let mtm = MainThreadMarker::new().expect("poll on non-main thread");
        if let Some(button) = unsafe { status_item.button(mtm) } {
            unsafe { button.setTitle(&NSString::from_str(&status_item_title(&pet_state))) };
        }
    }
}

struct AppStateHandles {
    popover: Retained<NSPopover>,
    status_item: Retained<NSStatusItem>,
}

fn dark_surface_color() -> Retained<NSColor> {
    unsafe {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(0x1d_u8) / 255.0,
            f64::from(0x1a_u8) / 255.0,
            f64::from(0x18_u8) / 255.0,
            1.0,
        )
    }
}
