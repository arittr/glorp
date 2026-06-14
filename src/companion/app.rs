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
    NSBackingStoreType, NSBezierPath, NSColor, NSCommandKeyMask, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSMenu, NSMenuItem, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString, NSTimer,
};
use ratatui::style::{Color as TuiColor, Style as TuiStyle};
use ratatui::text::Line as RatatuiLine;

use crate::commands::watch::{build_watch_view_model, rerender_pet_for_view_model};
use crate::companion::render::{build_draw_commands, RoundColor, RoundDrawCommand, RoundDrawKind};
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::pet::render::{PaletteRoleName, StyledSegment};
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundSceneModel};
use crate::storage::state::StateStore;
use crate::tui::style::semantic_styles;
use crate::tui::view_model::WatchViewModel;
use crate::watch_live::{LiveWatchUpdate, WatchPresentationState};

const POLL_INTERVAL: Duration = Duration::from_secs(10);
const UI_TICK_INTERVAL_SECS: f64 = 0.25;
const DEFAULT_WINDOW_SIZE: f64 = 360.0;
const WINDOW_ORIGIN_X: f64 = 120.0;
const WINDOW_ORIGIN_Y: f64 = 120.0;
const MIN_WINDOW_SIZE: f64 = 260.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompanionMenuSpec {
    app_title: &'static str,
    quit_title: &'static str,
    quit_key: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PetArtGrid {
    width: usize,
    height: usize,
    cells: Vec<PetArtCell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PetArtCell {
    row: usize,
    col: usize,
    ch: char,
    role: PaletteRoleName,
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
            if let Some(text) = command.text.as_deref() {
                draw_pet_art_block(
                    text,
                    &command.spans,
                    command.x,
                    command.y,
                    command.radius,
                    &command.color,
                );
            }
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

fn draw_pet_art_block(
    text: &str,
    spans: &[StyledSegment],
    x: f32,
    y: f32,
    radius: f32,
    color: &RoundColor,
) {
    let Some(grid) = pet_art_grid(text, spans) else {
        return;
    };
    unsafe {
        let base = attributed_pet_glyph("M", 1.0, color);
        let base_size = base.size();
        if base_size.width <= 0.0 || base_size.height <= 0.0 {
            return;
        }
        let target = f64::from(radius * 2.0) * 0.92;
        let font_size = (target / (base_size.width * grid.width as f64))
            .min(target / (base_size.height * grid.height as f64))
            .clamp(6.0, 28.0);
        let cell_size = attributed_pet_glyph("M", font_size, color).size();
        let cell_width = cell_size.width;
        let cell_height = cell_size.height;
        if cell_width <= 0.0 || cell_height <= 0.0 {
            return;
        }
        let total_width = cell_width * grid.width as f64;
        let total_height = cell_height * grid.height as f64;
        let left = x as f64 - total_width / 2.0;
        let top = y as f64 + total_height / 2.0;
        for cell in grid.cells {
            let cell_color = pet_role_color(cell.role).unwrap_or(*color);
            let attr = attributed_pet_glyph(&cell.ch.to_string(), font_size, &cell_color);
            let point = NSPoint::new(
                left + cell.col as f64 * cell_width,
                top - (cell.row + 1) as f64 * cell_height,
            );
            attr.drawAtPoint(point);
        }
    }
}

fn pet_art_grid(text: &str, spans: &[StyledSegment]) -> Option<PetArtGrid> {
    let lines = text.lines().collect::<Vec<_>>();
    let height = lines.len();
    let width = lines
        .iter()
        .map(|line| line_display_width(line))
        .max()
        .unwrap_or(0);
    let mut cells = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        let mut col = 0usize;
        for (char_index, ch) in line.chars().enumerate() {
            let width = char_display_width(ch);
            if ch != ' ' {
                cells.push(PetArtCell {
                    row,
                    col,
                    ch,
                    role: role_for_pet_cell(spans, row, char_index),
                });
            }
            col = col.saturating_add(width);
        }
    }
    if width == 0 || height == 0 || cells.is_empty() {
        return None;
    }
    Some(PetArtGrid {
        width,
        height,
        cells,
    })
}

fn role_for_pet_cell(spans: &[StyledSegment], row: usize, char_index: usize) -> PaletteRoleName {
    spans
        .iter()
        .find(|span| span.line == row && char_index >= span.start && char_index < span.end)
        .map(|span| span.role)
        .unwrap_or(PaletteRoleName::Body)
}

fn pet_role_color(role: PaletteRoleName) -> Option<RoundColor> {
    let styles = semantic_styles();
    let style = match role {
        PaletteRoleName::Body => styles.pet_body,
        PaletteRoleName::Eye => styles.pet_eye,
        PaletteRoleName::Mouth => styles.pet_mouth,
        PaletteRoleName::Accent | PaletteRoleName::Particle => styles.pet_accent,
        PaletteRoleName::Pattern => styles.pet_pattern,
    };
    style_color(style)
}

fn style_color(style: TuiStyle) -> Option<RoundColor> {
    match style.fg? {
        TuiColor::Rgb(r, g, b) => Some(rgb_color(r, g, b)),
        TuiColor::White => Some(rgb_color(0xef, 0xeb, 0xe4)),
        TuiColor::Gray => Some(rgb_color(0x97, 0x91, 0x8a)),
        TuiColor::DarkGray => Some(rgb_color(0x50, 0x4c, 0x49)),
        TuiColor::Yellow => Some(rgb_color(0xf0, 0xa6, 0x46)),
        TuiColor::Green => Some(rgb_color(0xa8, 0xc9, 0x6a)),
        TuiColor::Red => Some(rgb_color(0xe4, 0x68, 0x5d)),
        _ => None,
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

fn line_display_width(line: &str) -> usize {
    RatatuiLine::from(line).width()
}

fn char_display_width(ch: char) -> usize {
    RatatuiLine::from(ch.to_string()).width().max(1)
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

fn draw_label(label: char, x: f32, y: f32, radius: f32, color: &RoundColor) {
    unsafe {
        let text = NSString::from_str(&label.to_string());
        let font = NSFont::monospacedSystemFontOfSize_weight((radius * 1.5) as f64, 0.0);
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn pet_art_grid_preserves_terminal_columns() {
        let grid = pet_art_grid(" A\nB ", &[]).unwrap();

        assert_eq!(grid.width, 2);
        assert_eq!(grid.height, 2);
        assert_eq!(
            grid.cells,
            vec![
                PetArtCell {
                    row: 0,
                    col: 1,
                    ch: 'A',
                    role: PaletteRoleName::Body,
                },
                PetArtCell {
                    row: 1,
                    col: 0,
                    ch: 'B',
                    role: PaletteRoleName::Body,
                },
            ]
        );
    }

    #[test]
    fn pet_art_grid_maps_terminal_span_roles_to_cells() {
        let spans = vec![StyledSegment {
            line: 0,
            start: 1,
            end: 2,
            role: PaletteRoleName::Eye,
        }];
        let grid = pet_art_grid(" A\nB ", &spans).unwrap();

        assert_eq!(grid.cells[0].role, PaletteRoleName::Eye);
        assert_eq!(grid.cells[1].role, PaletteRoleName::Body);
    }
}
