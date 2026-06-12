//! NSApplication + NSStatusItem + NSPopover wiring. The controller object is
//! intentionally state-less; live UI handles live in a thread-local set up by
//! `run` on the main thread, since NSPopover / NSTextView are not `Sync` and
//! only ever touched from the main run loop.

#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSColor, NSPopover, NSPopoverBehavior,
    NSStatusBar, NSStatusItem, NSTextView, NSView, NSViewController,
};
use objc2_foundation::{
    MainThreadMarker, NSPoint, NSRange, NSRect, NSRectEdge, NSSize, NSString, NSTimer,
};

use crate::commands::watch::{
    build_watch_view_model, poll_usage_and_apply, rerender_pet_for_view_model,
};
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::storage::state::{PetState, StateStore};
use crate::tui::view_model::WatchViewModel;

use super::render;

/// `NSStatusItem.length` value meaning "size to fit content". Apple's header
/// declares `NSVariableStatusItemLength = -1.0`.
const NS_VARIABLE_STATUS_ITEM_LENGTH: f64 = -1.0;
const POLL_INTERVAL: Duration = Duration::from_secs(10);
/// UI tick rate. Drives both pet animation (when popover is shown) and draining
/// any PollResult delivered by the worker thread. Matches the watch TUI's idle
/// tick (`Duration::from_millis(250)` in `crate::tui::app`) so a menubar
/// session and a watch session breathe in sync.
const UI_TICK_INTERVAL_SECS: f64 = 0.25;
// Approximate cell width/height for the 13pt monospaced system font. Tuned
// visually so the popover sizes roughly to the content; real text measurement
// can replace this later.
const APPROX_CELL_WIDTH: f64 = 7.6;
const APPROX_CELL_HEIGHT: f64 = 17.0;

struct AppState {
    popover: Retained<NSPopover>,
    text_view: Retained<NSTextView>,
    status_item: Retained<NSStatusItem>,
    /// Last view model written to the text view. The UI tick mutates a clone
    /// of this to advance pet animation without re-reading from disk; the
    /// worker thread's PollResult replaces it wholesale.
    vm: WatchViewModel,
    animation_frame: u64,
    /// Character length of the pet block region at the start of the text
    /// storage. Used as the `NSRange` upper bound when replacing only the
    /// pet rows during an animation frame.
    pet_block_char_len: usize,
    /// Drained on each UI tick. The worker thread pushes a new PollResult
    /// roughly every `POLL_INTERVAL`; the main thread applies the most
    /// recent one and discards stale ones.
    poll_rx: mpsc::Receiver<PollResult>,
    life_signal_state: crate::tui::life::LifeSignalState,
}

/// Snapshot of fresh provider data computed on the worker thread. All fields
/// are owned and `Send`, so the worker can build them off-main-thread and ship
/// them through the channel.
struct PollResult {
    pet_state: PetState,
    vm: WatchViewModel,
    applied_signal: crate::tui::life::AppliedUsageSignal,
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

        #[method(uiTick:)]
        fn ui_tick(&self, _sender: Option<&AnyObject>) {
            ui_tick();
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

    let app: Retained<NSApplication> = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller: Retained<Controller> = unsafe { msg_send_id![Controller::class(), new] };

    let popover_size = NSSize::new(
        (render::POPOVER_COLUMNS as f64) * APPROX_CELL_WIDTH + 24.0,
        (render::POPOVER_ROWS as f64) * APPROX_CELL_HEIGHT + 16.0,
    );
    let (popover, text_view) = build_popover(mtm, popover_size);
    let pet_block_char_len = write_full_text(&text_view, &initial_vm);

    let status_item = build_status_item(mtm, &controller, &initial_pet)?;

    let poll_rx = spawn_poll_worker(paths);

    APP_STATE.with(|cell| {
        *cell.borrow_mut() = Some(AppState {
            popover,
            text_view,
            status_item,
            vm: initial_vm,
            animation_frame: 0,
            pet_block_char_len,
            poll_rx,
            life_signal_state: crate::tui::life::LifeSignalState::default(),
        });
    });

    let _ui_timer: Retained<NSTimer> = unsafe {
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

/// Spawn the background poll worker. It runs `poll_usage_and_apply` and
/// `build_watch_view_model` off the main thread — those calls shell out to
/// ccusage / ccusage-codex (subprocess + JSON parse) and hit SQLite, which
/// must never run on the run loop. The returned receiver is drained from
/// the UI tick.
fn spawn_poll_worker(paths: AppPaths) -> mpsc::Receiver<PollResult> {
    let (tx, rx) = mpsc::channel::<PollResult>();
    thread::Builder::new()
        .name("glorp-menubar-poll".into())
        .spawn(move || {
            let state_store = StateStore::new(paths.state_file.clone());
            loop {
                thread::sleep(POLL_INTERVAL);
                let outcome =
                    match poll_usage_and_apply(&state_store, &paths.usage_db, &paths.config_file) {
                        Ok(Some(outcome)) => outcome,
                        Ok(None) | Err(_) => continue,
                    };
                let vm = match build_watch_view_model(&outcome.state, &paths.usage_db) {
                    Ok(vm) => vm,
                    Err(_) => continue,
                };
                if tx
                    .send(PollResult {
                        pet_state: outcome.state,
                        vm,
                        applied_signal: outcome.applied_signal,
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
        .expect("spawn glorp-menubar-poll worker");
    rx
}

fn build_popover(
    mtm: MainThreadMarker,
    size: NSSize,
) -> (Retained<NSPopover>, Retained<NSTextView>) {
    let view_frame = NSRect::new(NSPoint::new(0.0, 0.0), size);

    let view: Retained<NSView> = unsafe { NSView::initWithFrame(mtm.alloc(), view_frame) };

    let text_view: Retained<NSTextView> =
        unsafe { NSTextView::initWithFrame(mtm.alloc(), view_frame) };
    unsafe {
        // We render attributed strings as a static display surface. Everything
        // NSTextView offers beyond that — selection, undo, find, automatic
        // text substitutions, spell/grammar checking — adds per-edit work in
        // the layout manager. At 4 Hz that work piles up faster than ticks
        // drain and the main thread beachballs. Strip it all.
        text_view.setEditable(false);
        text_view.setSelectable(false);
        text_view.setRichText(false);
        text_view.setAllowsUndo(false);
        text_view.setUsesFindBar(false);
        text_view.setUsesFindPanel(false);
        text_view.setUsesFontPanel(false);
        text_view.setAutomaticDashSubstitutionEnabled(false);
        text_view.setAutomaticDataDetectionEnabled(false);
        text_view.setAutomaticLinkDetectionEnabled(false);
        text_view.setAutomaticQuoteSubstitutionEnabled(false);
        text_view.setAutomaticSpellingCorrectionEnabled(false);
        text_view.setAutomaticTextCompletionEnabled(false);
        text_view.setAutomaticTextReplacementEnabled(false);
        text_view.setContinuousSpellCheckingEnabled(false);
        text_view.setGrammarCheckingEnabled(false);
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

/// Replace the entire text storage with a freshly rendered pet block + stats
/// block. Returns the char length of the pet block region, which the animation
/// tick uses as the upper bound of the range it overwrites.
fn write_full_text(text_view: &NSTextView, vm: &WatchViewModel) -> usize {
    let mut pet = render::render_pet_block(vm);
    let stats = render::render_stats_block(vm);
    unsafe {
        pet.attr.appendAttributedString(&stats.attr);
        let mut storage = text_view
            .textStorage()
            .expect("NSTextView always has a text storage");
        let length = storage.length();
        storage.beginEditing();
        storage.replaceCharactersInRange_withAttributedString(NSRange::from(0..length), &pet.attr);
        storage.endEditing();
        text_view.setNeedsDisplay(true);
    }
    pet.char_len
}

/// Replace only the pet region of the text storage. `prev_pet_char_len` is the
/// range that the previous frame occupied; the new pet block goes in its place
/// and shifts the stats block by the difference. Wrapped in
/// `beginEditing`/`endEditing` so NSTextStorage coalesces fix-up and the
/// layout manager runs at most one relayout pass per tick.
fn write_pet_block(text_view: &NSTextView, vm: &WatchViewModel, prev_pet_char_len: usize) -> usize {
    let pet = render::render_pet_block(vm);
    unsafe {
        let mut storage = text_view
            .textStorage()
            .expect("NSTextView always has a text storage");
        storage.beginEditing();
        storage.replaceCharactersInRange_withAttributedString(
            NSRange::from(0..prev_pet_char_len),
            &pet.attr,
        );
        storage.endEditing();
        text_view.setNeedsDisplay(true);
    }
    pet.char_len
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
    let bounds: NSRect = button.bounds();
    unsafe {
        handles.popover.showRelativeToRect_ofView_preferredEdge(
            bounds,
            &button,
            NSRectEdge::NSMinYEdge,
        );
    }
}

/// Main-thread UI tick. Drains any pending worker PollResults (applying the
/// most recent one to the text view + status item title) and then advances
/// the pet animation frame if the popover is shown. Never blocks on disk or
/// subprocesses — those run on the worker thread spawned by
/// `spawn_poll_worker`.
///
/// Animation writes are guarded by a `pet_art`/`pet_spans` diff so frames where
/// the rendered pet is bit-for-bit identical produce zero AppKit work. Most
/// 250ms ticks fall in this bucket (blinks fire every ~50 ticks, glitch every
/// 37, particles intermittently).
fn ui_tick() {
    drain_poll_results();
    animate_pet();
}

fn drain_poll_results() {
    let mut latest: Option<PollResult> = None;
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow().as_ref() {
            while let Ok(result) = state.poll_rx.try_recv() {
                latest = Some(result);
            }
        }
    });
    let Some(result) = latest else {
        return;
    };
    let mut vm = result.vm;
    let (text_view, status_item) = match APP_STATE.with(|cell| {
        cell.borrow_mut().as_mut().map(|s| {
            let profile = s.life_signal_state.observe(
                result.applied_signal,
                &vm.activity_identity,
                time::OffsetDateTime::now_utc(),
            );
            vm.life_profile = profile;
            (s.text_view.clone(), s.status_item.clone())
        })
    }) {
        Some(pair) => pair,
        None => return,
    };
    vm.life_profile.calm_mode = vm.day_context.asleep;
    let pet_len = write_full_text(&text_view, &vm);
    let mtm = MainThreadMarker::new().expect("ui_tick on non-main thread");
    if let Some(button) = unsafe { status_item.button(mtm) } {
        unsafe { button.setTitle(&NSString::from_str(&status_item_title(&result.pet_state))) };
    }
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.vm = vm;
            state.pet_block_char_len = pet_len;
        }
    });
}

fn animate_pet() {
    let snapshot = APP_STATE.with(|cell| {
        cell.borrow().as_ref().map(|s| {
            (
                s.popover.clone(),
                s.text_view.clone(),
                s.animation_frame,
                s.vm.clone(),
                s.pet_block_char_len,
            )
        })
    });
    let Some((popover, text_view, frame, mut vm, prev_pet_len)) = snapshot else {
        return;
    };
    if !unsafe { popover.isShown() } {
        return;
    }
    let next_frame = frame.wrapping_add(1);
    let prev_pet_art = vm.pet_art.clone();
    let prev_pet_spans = vm.pet_spans.clone();
    let hold_eyes_closed = vm.day_context.asleep;
    if rerender_pet_for_view_model(&mut vm, next_frame, hold_eyes_closed).is_err() {
        return;
    }
    if vm.pet_art == prev_pet_art && vm.pet_spans == prev_pet_spans {
        APP_STATE.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                state.animation_frame = next_frame;
            }
        });
        return;
    }
    let new_pet_len = write_pet_block(&text_view, &vm, prev_pet_len);
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.vm = vm;
            state.animation_frame = next_frame;
            state.pet_block_char_len = new_pet_len;
        }
    });
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
