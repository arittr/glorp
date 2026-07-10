//! Native macOS round companion window. Uses a regular Dock app lifecycle,
//! a worker thread for live usage polling, and pure AppKit drawing from
//! `RoundSceneModel`.

#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::commands::companion_mode::{
    CompanionRendererMode, CompanionReviewDepth, CompanionReviewOptions, CompanionReviewSize,
    CompanionReviewState,
};
use crate::commands::watch::{
    build_watch_view_model_at, build_watch_view_model_semantic_at, rerender_pet_for_view_model,
};
use crate::companion::render::{build_draw_commands, RoundColor, RoundDrawCommand, RoundDrawKind};
use crate::error::{GlorpError, Result};
use crate::paths::AppPaths;
use crate::presentation::pixel::{
    render_pixel_frame, PixelFrame, PixelPetInput, PixelRendererState, PixelRendererTick,
    PixelViewport,
};
use crate::presentation::smooth::{
    validate_smooth_layer, SmoothBlendMode, SmoothBounds, SmoothClip, SmoothCompanionLayer,
    SmoothCompanionScenePlan, SmoothFill, SmoothGeometryError, SmoothLayerItem,
    SmoothLayerMotionBinding, SmoothPoint, SmoothRgba8, SmoothShapeGeometry,
};
use crate::round::hud::{
    companion_hud_text, companion_pace_fraction, daily_fraction_for_gauge, daily_overage_color,
    daily_overage_marker_arc, daily_overage_marker_fraction, growth_ring_fill_end_deg,
    perimeter_gauge_colors, perimeter_gauge_layout, CompanionHudText, GaugeLane, GaugeLaneColors,
    LineCap, COMPANION_GAUGE_GAP_DEG,
};
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundSceneModel};
use crate::round::smooth::SmoothScenePlanError;
use crate::storage::state::StateStore;
use crate::tui::view_model::SourceStatus;
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
    NSBackingStoreType, NSBezierPath, NSBitmapImageRep, NSButtLineCapStyle,
    NSCalibratedRGBColorSpace, NSColor, NSCommandKeyMask, NSCompositingOperation, NSControlKeyMask,
    NSEventModifierFlags, NSFont, NSFontAttributeName, NSFontWeightBold,
    NSForegroundColorAttributeName, NSGradient, NSGraphicsContext, NSImage, NSLineCapStyle, NSMenu,
    NSMenuItem, NSRoundLineCapStyle, NSView, NSWindow, NSWindowCollectionBehavior,
    NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{
    MainThreadMarker, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString, NSTimer,
};

// The pace gauge reads a ten-minute window and the pet's vitals move on hour
// scales, so ten-second polls bought nothing but CPU: every cycle re-runs the
// helper over the whole current day's transcripts.
const POLL_INTERVAL: Duration = Duration::from_secs(30);
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

#[allow(dead_code)] // Consumed by the staged draw preparation in Tasks 4 and 5.
#[derive(Debug, Clone)]
struct PreparedCompanionFrame {
    bounds: PreparedBounds,
    aperture: RoundAperture,
    background: RoundColor,
    mood_aura_color: RoundColor,
    dim_overlay: bool,
    renderer: PreparedRendererFrame,
    gauges: PreparedGaugeFrame,
    hud: CompanionHudText,
    hud_font_size: f64,
    overlay_commands: Vec<RoundDrawCommand>,
    review_sample: Option<crate::companion::review_capture::SmoothReviewFrameSample>,
}

#[allow(dead_code)] // Consumed by the staged draw preparation in Tasks 4 and 5.
#[derive(Debug, Clone)]
enum PreparedRendererFrame {
    Pixel {
        frame: PixelFrame,
    },
    Classic {
        metrics: CompanionGridMetrics,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        draw_list: crate::presentation::SceneDrawList,
    },
    Smooth {
        metrics: CompanionGridMetrics,
        pet_center_col: f64,
        pet_center_row: f64,
        pet_width_cells: f64,
        plan: SmoothCompanionScenePlan,
    },
}

#[allow(dead_code)] // Consumed by the staged draw preparation in Tasks 4 and 5.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedGaugeFrame {
    xp_fraction: f64,
    daily_fraction: f64,
    daily_overage_fraction: f64,
    pace_fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedBounds {
    width_px: u16,
    height_px: u16,
    width_f64: f64,
    height_f64: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompanionMetricKey {
    width_bits: u64,
    height_bits: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct CompanionMetricCache {
    last: Option<(CompanionMetricKey, CompanionGridMetrics)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompanionFramePreparationError {
    InvalidBounds,
    MissingGridMetrics,
    SmoothMissingPetBody,
    SmoothInvalidParallaxGeometry,
    SmoothInvalidDepth,
    SmoothInvalidLayerGeometry,
}

impl CompanionFramePreparationError {
    /// Static, privacy-safe category strings: no geometry values, no pet state.
    fn category(self) -> &'static str {
        match self {
            CompanionFramePreparationError::InvalidBounds => "invalid-bounds",
            CompanionFramePreparationError::MissingGridMetrics => "missing-grid-metrics",
            CompanionFramePreparationError::SmoothMissingPetBody => "smooth-missing-pet-body",
            CompanionFramePreparationError::SmoothInvalidParallaxGeometry => {
                "smooth-invalid-parallax-geometry"
            }
            CompanionFramePreparationError::SmoothInvalidDepth => "smooth-invalid-depth",
            CompanionFramePreparationError::SmoothInvalidLayerGeometry => {
                "smooth-invalid-layer-geometry"
            }
        }
    }
}

fn prepare_bounds(
    bounds: NSRect,
) -> std::result::Result<PreparedBounds, CompanionFramePreparationError> {
    let width = bounds.size.width;
    let height = bounds.size.height;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > f64::from(u16::MAX)
        || height > f64::from(u16::MAX)
    {
        return Err(CompanionFramePreparationError::InvalidBounds);
    }

    let width_px = width as u16;
    let height_px = height as u16;
    if width_px == 0 || height_px == 0 {
        return Err(CompanionFramePreparationError::InvalidBounds);
    }

    Ok(PreparedBounds {
        width_px,
        height_px,
        width_f64: width,
        height_f64: height,
    })
}

fn prepare_gauge_frame(vm: &WatchViewModel) -> PreparedGaugeFrame {
    PreparedGaugeFrame {
        xp_fraction: if vm.progress.is_max_stage {
            1.0
        } else {
            vm.progress.fraction as f64
        },
        daily_fraction: daily_fraction_for_gauge(vm.daily_comparison.fraction_of_yesterday),
        daily_overage_fraction: daily_overage_marker_fraction(
            vm.daily_comparison.fraction_of_yesterday,
        ),
        pace_fraction: companion_pace_fraction(vm.rate_momentum.pulse.current_tokens),
    }
}

fn prepare_hud_frame(vm: &WatchViewModel, redacts_live_hud: bool) -> CompanionHudText {
    if redacts_live_hud {
        review_capture_hud_text()
    } else {
        live_hud_text(vm)
    }
}

impl CompanionMetricCache {
    fn metrics_for(
        &mut self,
        bounds: PreparedBounds,
    ) -> std::result::Result<CompanionGridMetrics, CompanionFramePreparationError> {
        let key = CompanionMetricKey {
            width_bits: bounds.width_f64.to_bits(),
            height_bits: bounds.height_f64.to_bits(),
        };
        if let Some((cached_key, metrics)) = self.last {
            if cached_key == key {
                return Ok(metrics);
            }
        }
        let metrics = companion_grid_metrics(bounds.width_f64, bounds.height_f64)
            .ok_or(CompanionFramePreparationError::MissingGridMetrics)?;
        self.last = Some((key, metrics));
        Ok(metrics)
    }
}

#[allow(clippy::too_many_arguments)] // Explicit inputs keep AppState field borrows disjoint.
fn prepare_companion_frame(
    vm: &WatchViewModel,
    scene: &RoundSceneModel,
    renderer_mode: CompanionRendererMode,
    review_depth: Option<CompanionReviewDepth>,
    pixel_frame: Option<&PixelFrame>,
    smooth_started_at: Option<Instant>,
    smooth_semantic_art_tick_index: u64,
    redacts_live_hud: bool,
    bounds: NSRect,
    metric_cache: &mut CompanionMetricCache,
) -> std::result::Result<PreparedCompanionFrame, CompanionFramePreparationError> {
    let prepared_bounds = prepare_bounds(bounds)?;
    let aperture = RoundAperture::new(prepared_bounds.width_px, prepared_bounds.height_px);
    let layout = layout_round_scene(
        scene,
        aperture,
        RoundRenderCapabilities::preview_truecolor(),
    );
    let overlay_commands = build_draw_commands(scene, &layout);
    let background = overlay_commands
        .iter()
        .find(|command| command.kind == RoundDrawKind::Background)
        .map(|command| command.color)
        .unwrap_or(RoundColor(0.05, 0.06, 0.10, 1.0));
    let dim_overlay = scene.lifecycle.asleep || scene.lifecycle.calm;
    let gauges = prepare_gauge_frame(vm);
    let hud = prepare_hud_frame(vm, redacts_live_hud);

    let renderer = if renderer_mode.is_pixel() {
        PreparedRendererFrame::Pixel {
            frame: pixel_frame
                .cloned()
                .unwrap_or_else(|| PixelFrame::transparent(PixelViewport::companion_default())),
        }
    } else {
        let metrics = metric_cache.metrics_for(prepared_bounds)?;
        if renderer_mode.is_smooth() {
            let elapsed_ms = smooth_started_at
                .map(|started_at| started_at.elapsed().as_millis())
                .unwrap_or(0)
                .min(u128::from(u64::MAX)) as u64;
            // Normal runs pass None here and keep their roam-driven depth.
            let plan = crate::round::smooth::try_build_round_smooth_scene_plan_with_options(
                vm,
                time::OffsetDateTime::now_utc(),
                metrics.grid_cols,
                metrics.grid_rows,
                &companion_motion(),
                elapsed_ms,
                crate::round::smooth::SmoothSceneBuildOptions {
                    depth_override: review_depth.map(CompanionReviewDepth::normalized),
                },
            )
            .map_err(|err| match err {
                SmoothScenePlanError::MissingPetBody => {
                    CompanionFramePreparationError::SmoothMissingPetBody
                }
                SmoothScenePlanError::InvalidParallaxGeometry => {
                    CompanionFramePreparationError::SmoothInvalidParallaxGeometry
                }
                SmoothScenePlanError::InvalidDepth(_) => {
                    CompanionFramePreparationError::SmoothInvalidDepth
                }
                SmoothScenePlanError::InvalidLayerGeometry(_) => {
                    CompanionFramePreparationError::SmoothInvalidLayerGeometry
                }
            })?;
            // The aura follows the pet's composed depth transform, so it grows and
            // sinks with the creature instead of staying pinned to the unscaled art.
            let transformed = plan.pet.transformed_bounds;
            let pet_center_col =
                f64::from(transformed.min.x + (transformed.max.x - transformed.min.x) / 2.0);
            let pet_center_row =
                f64::from(transformed.min.y + (transformed.max.y - transformed.min.y) / 2.0);
            let pet_width_cells = f64::from(transformed.max.x - transformed.min.x);
            PreparedRendererFrame::Smooth {
                metrics,
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                plan,
            }
        } else {
            let companion_scene = crate::round::scene::build_round_scene_draw_list(
                vm,
                time::OffsetDateTime::now_utc(),
                metrics.grid_cols,
                metrics.grid_rows,
                &companion_motion(),
            );
            PreparedRendererFrame::Classic {
                metrics,
                pet_center_col: f64::from(
                    companion_scene.pet_rect.x + companion_scene.pet_rect.width / 2,
                ),
                pet_center_row: f64::from(
                    companion_scene.pet_rect.y + companion_scene.pet_rect.height / 2,
                ),
                pet_width_cells: f64::from(companion_scene.pet_rect.width),
                draw_list: companion_scene.draw_list,
            }
        }
    };

    let hud_font_size = match &renderer {
        PreparedRendererFrame::Classic { metrics, .. }
        | PreparedRendererFrame::Smooth { metrics, .. } => metrics.font_size,
        PreparedRendererFrame::Pixel { .. } => metric_cache
            .metrics_for(prepared_bounds)
            .map(|metrics| metrics.font_size)
            .unwrap_or(8.5),
    };
    let review_sample = match &renderer {
        PreparedRendererFrame::Smooth { plan, .. } => {
            Some(crate::companion::review_capture::SmoothReviewFrameSample {
                bob_y: plan.pet.bob_offset.y,
                semantic_art_tick_index: smooth_semantic_art_tick_index,
                pet_visual_checksum: crate::presentation::smooth::pet_visual_checksum(
                    &vm.pet_art,
                    &vm.pet_spans,
                ),
                base_anchor: crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                    plan.pet.base_anchor,
                ),
                bob_offset: crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                    plan.pet.bob_offset,
                ),
                final_anchor:
                    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                        plan.pet.final_anchor,
                    ),
                classic_snap_anchor:
                    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                        plan.pet.classic_snap_anchor,
                    ),
                parallax_focus_offset:
                    crate::companion::review_capture::SmoothReviewPoint::from_smooth_point(
                        plan.pet.parallax_focus_offset,
                    ),
                parallax_lifecycle_scale: plan.parallax_lifecycle_scale,
                parallax_planes:
                    crate::companion::review_capture::SmoothReviewParallaxPlanes::from_smooth_planes(
                        plan.parallax_translations_by_plane(),
                    ),
                depth: plan.pet.depth,
                pet_scale: plan.pet.scale,
                perspective_y: plan.pet.perspective_offset.y,
                pet_extent_width: plan.pet.transformed_bounds.max.x
                    - plan.pet.transformed_bounds.min.x,
                pet_extent_height: plan.pet.transformed_bounds.max.y
                    - plan.pet.transformed_bounds.min.y,
                shape_draw_count: plan
                    .layers
                    .iter()
                    .flat_map(|layer| layer.items.iter())
                    .filter(|item| {
                        matches!(item, crate::presentation::smooth::SmoothLayerItem::Shape(_))
                    })
                    .count() as u32,
            })
        }
        _ => None,
    };

    Ok(PreparedCompanionFrame {
        bounds: prepared_bounds,
        aperture,
        background,
        mood_aura_color: crate::round::hud::mood_aura_color(scene.pet.mood),
        dim_overlay,
        renderer,
        gauges,
        hud,
        hud_font_size,
        overlay_commands,
        review_sample,
    })
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
    review_state: CompanionReviewState,
    /// Pins the Smooth pet's depth plane for deterministic review captures.
    review_depth: Option<CompanionReviewDepth>,
    renderer_mode: CompanionRendererMode,
    pixel_input: Option<PixelPetInput>,
    pixel_state: Option<PixelRendererState>,
    pixel_frame: Option<PixelFrame>,
    smooth_started_at: Option<Instant>,
    smooth_semantic_clock: Option<crate::companion::smooth_timing::SmoothSemanticClock>,
    smooth_semantic_art_tick_index: u64,
    animation_frame: u64,
    review_capture: Option<crate::companion::review_capture::ReviewCapture>,
    redacts_live_hud: bool,
    metric_cache: CompanionMetricCache,
    last_good_frame: Option<PreparedCompanionFrame>,
    #[allow(dead_code)] // Read by the Task 5 paint boundary.
    last_frame_preparation_error: Option<CompanionFramePreparationError>,
    #[allow(dead_code)] // Updated by the Task 5 callback guard.
    callback_panic_count: u64,
    #[allow(dead_code)] // Updated by the Task 5 callback guard.
    last_callback_panic_label: Option<&'static str>,
}

thread_local! {
    static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

fn run_objc_callback(label: &'static str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        record_callback_panic(label);
    }
}

fn write_boundary_diagnostic(args: std::fmt::Arguments<'_>) {
    let mut stderr = std::io::stderr();
    let _ = std::io::Write::write_fmt(&mut stderr, args);
}

fn record_callback_panic(label: &'static str) {
    write_boundary_diagnostic(format_args!(
        "glorp companion caught panic in Objective-C callback: {label}\n"
    ));
    APP_STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            if let Some(state) = state.as_mut() {
                state.callback_panic_count = state.callback_panic_count.saturating_add(1);
                state.last_callback_panic_label = Some(label);
                if let Some(capture) = state.review_capture.as_mut() {
                    capture.record_callback_panic(label);
                }
            }
        }
    });
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
            run_objc_callback("uiTick", ui_tick);
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
            run_objc_callback("drawRect", || draw_scene(self, self.bounds()));
        }
    }
);

pub fn run(renderer_mode: CompanionRendererMode, review: CompanionReviewOptions) -> Result<()> {
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
    let mut initial_vm = if renderer_mode.is_pixel() || renderer_mode.is_smooth() {
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
    let mut presentation_state = WatchPresentationState::default();
    let review_state = review.resolved_state();
    apply_review_state(review_state, &mut presentation_state, &mut initial_vm, now)?;
    if renderer_mode.is_smooth() {
        prepare_smooth_view_model_for_tick(&mut initial_vm, 0, now)?;
    }
    let scene = derive_round_scene_model(&initial_vm, now);
    let pixel_input = renderer_mode
        .is_pixel()
        .then(|| PixelPetInput::from_watch_view_model(&initial_vm, now));
    let pixel_state = pixel_input
        .as_ref()
        .map(|input| PixelRendererState::new(input, now));
    let pixel_frame = None;
    let smooth_started_at = renderer_mode.is_smooth().then(Instant::now);
    let smooth_semantic_clock = smooth_started_at.map(|started_at| {
        crate::companion::smooth_timing::SmoothSemanticClock::new(
            started_at,
            Duration::from_secs_f64(UI_TICK_INTERVAL_SECS),
        )
    });

    let app: Retained<NSApplication> = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    install_app_menu(&app, mtm);

    let controller: Retained<Controller> = unsafe { msg_send_id![Controller::class(), new] };
    let review_capture =
        crate::companion::review_capture::ReviewCapture::from_options(renderer_mode, &review)?;
    let redacts_live_hud = review_capture
        .as_ref()
        .is_some_and(|capture| capture.redacts_live_hud());
    let (window, view) = build_window(mtm, review.initial_size);
    let poll_rx = crate::watch_live::spawn_live_watch_worker(
        paths,
        POLL_INTERVAL,
        "glorp-companion-poll",
        if renderer_mode.is_pixel() || renderer_mode.is_smooth() {
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
            presentation_state,
            vm: initial_vm,
            scene,
            review_state,
            review_depth: review.depth,
            renderer_mode,
            pixel_input,
            pixel_state,
            pixel_frame,
            smooth_started_at,
            smooth_semantic_clock,
            smooth_semantic_art_tick_index: 0,
            animation_frame: 0,
            review_capture,
            redacts_live_hud,
            metric_cache: CompanionMetricCache::default(),
            last_good_frame: None,
            last_frame_preparation_error: None,
            callback_panic_count: 0,
            last_callback_panic_label: None,
        });
    });

    prepare_current_frame_from_state();

    // The smooth scene is CPU-drawn: every tick invalidates the whole porthole
    // for a full CG redraw and CoreAnimation recomposite. Its motion is slow
    // multi-second drift and bob, which reads identically at fifteen frames, so
    // thirty just doubles the energy bill.
    let tick_interval = if renderer_mode.is_pixel() {
        1.0 / 30.0
    } else if renderer_mode.is_smooth() {
        1.0 / 15.0
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

fn apply_review_state(
    state: CompanionReviewState,
    presentation_state: &mut WatchPresentationState,
    vm: &mut WatchViewModel,
    now: time::OffsetDateTime,
) -> Result<()> {
    match state {
        CompanionReviewState::Normal => {}
        CompanionReviewState::ActivePulse => {
            crate::watch_live::stamp_live_presentation(
                presentation_state,
                vm,
                crate::tui::life::AppliedUsageSignal::diagnostics_only(
                    now,
                    time::Duration::seconds(10),
                ),
                now,
            );
            crate::watch_live::stamp_live_presentation(
                presentation_state,
                vm,
                crate::watch_live::bursting_review_signal(now),
                now,
            );
        }
        CompanionReviewState::AsleepCalm => {
            vm.day_context.asleep = true;
            vm.life_profile.calm_mode = true;
            vm.last_feed_pulse_at = None;
            vm.breath_offset_y = 0;
            rerender_pet_for_view_model(vm, 0, true, now)?;
        }
        CompanionReviewState::HelperTrouble => {
            if let Some(source) = vm.source_health.first_mut() {
                source.status = SourceStatus::Diagnostic;
                source.diagnostic_code = Some("review-state".into());
                source.diagnostic_message = None;
            }
        }
    }
    Ok(())
}

fn prepare_smooth_view_model_for_tick(
    vm: &mut WatchViewModel,
    semantic_art_tick_index: u64,
    now: time::OffsetDateTime,
) -> Result<()> {
    if vm.pet_art.is_empty() || vm.pet_spans.is_empty() {
        rerender_pet_for_view_model(vm, semantic_art_tick_index, vm.day_context.asleep, now)?;
    }
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

fn build_window(
    mtm: MainThreadMarker,
    review_size: Option<CompanionReviewSize>,
) -> (Retained<NSWindow>, Retained<RoundView>) {
    let initial_size = review_size.map_or(DEFAULT_WINDOW_SIZE, |size| f64::from(size.width));
    let initial_height = review_size.map_or(DEFAULT_WINDOW_SIZE, |size| f64::from(size.height));
    let frame = NSRect::new(
        NSPoint::new(WINDOW_ORIGIN_X, WINDOW_ORIGIN_Y),
        NSSize::new(initial_size, initial_height),
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
    prepare_current_frame_from_state();
    finish_review_capture_if_due();
}

fn prepare_current_frame_from_state() {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };
        let prepared = {
            let AppState {
                view,
                vm,
                scene,
                renderer_mode,
                review_depth,
                pixel_frame,
                smooth_started_at,
                smooth_semantic_art_tick_index,
                redacts_live_hud,
                metric_cache,
                ..
            } = state;
            let bounds = view.bounds();
            prepare_companion_frame(
                vm,
                scene,
                *renderer_mode,
                *review_depth,
                pixel_frame.as_ref(),
                *smooth_started_at,
                *smooth_semantic_art_tick_index,
                *redacts_live_hud,
                bounds,
                metric_cache,
            )
        };
        match prepared {
            Ok(frame) => {
                state.last_good_frame = Some(frame);
                state.last_frame_preparation_error = None;
            }
            Err(err) => record_frame_preparation_error(state, err),
        }
    });
}

fn record_frame_preparation_error(state: &mut AppState, err: CompanionFramePreparationError) {
    let is_new_error =
        should_record_frame_preparation_error(state.last_frame_preparation_error, err);
    state.last_frame_preparation_error = Some(err);
    if !is_new_error {
        return;
    }
    let reused_last_good_frame = state.last_good_frame.is_some();
    if let Some(capture) = state.review_capture.as_mut() {
        capture.record_frame_preparation_error(err.category());
        if reused_last_good_frame {
            capture.record_last_good_frame_reused();
        }
    }
    write_boundary_diagnostic(format_args!(
        "glorp companion frame preparation failed: {}\n",
        err.category()
    ));
}

fn should_record_frame_preparation_error(
    previous: Option<CompanionFramePreparationError>,
    current: CompanionFramePreparationError,
) -> bool {
    previous != Some(current)
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
            let now = time::OffsetDateTime::now_utc();
            let Ok((vm, scene, pixel_input)) = apply_post_poll_update(
                &mut state.presentation_state,
                state.review_state,
                state.renderer_mode,
                update,
                now,
                state.smooth_semantic_art_tick_index,
            ) else {
                return;
            };
            state.pixel_input = pixel_input;
            state.scene = scene;
            state.vm = vm;
            unsafe { state.view.setNeedsDisplay(true) };
        }
    });
}

fn apply_post_poll_update(
    presentation_state: &mut WatchPresentationState,
    review_state: CompanionReviewState,
    renderer_mode: CompanionRendererMode,
    update: LiveWatchUpdate,
    now: time::OffsetDateTime,
    smooth_semantic_art_tick_index: u64,
) -> Result<(WatchViewModel, RoundSceneModel, Option<PixelPetInput>)> {
    let mut vm = update.vm;
    crate::watch_live::stamp_live_presentation(
        presentation_state,
        &mut vm,
        update.applied_signal,
        now,
    );
    apply_review_state(review_state, presentation_state, &mut vm, now)?;
    if renderer_mode.is_smooth() {
        prepare_smooth_view_model_for_tick(&mut vm, smooth_semantic_art_tick_index, now)?;
    }
    let pixel_input = renderer_mode
        .is_pixel()
        .then(|| PixelPetInput::from_watch_view_model(&vm, now));
    let scene = derive_round_scene_model(&vm, now);
    Ok((vm, scene, pixel_input))
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
        let now = time::OffsetDateTime::now_utc();
        if state.renderer_mode.is_smooth() {
            let due_tick = state
                .smooth_semantic_clock
                .as_mut()
                .and_then(|clock| clock.consume_due_tick(Instant::now()));
            if let Some(tick_index) = due_tick {
                let _ = advance_companion_animation(&mut state.vm, tick_index, now);
                state.animation_frame = tick_index;
                state.smooth_semantic_art_tick_index = tick_index;
                state.scene = derive_round_scene_model(&state.vm, now);
            }
            return Some(state.view.clone());
        }
        let next_frame = state.animation_frame.wrapping_add(1);
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

fn finish_review_capture_if_due() {
    let pending_capture = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        if state
            .review_capture
            .as_ref()
            .is_some_and(|capture| capture.ready_to_finish())
        {
            let capture = state.review_capture.take()?;
            Some((state.view.clone(), capture))
        } else {
            None
        }
    });
    let Some((view, mut capture)) = pending_capture else {
        return;
    };

    match capture.finish(view.as_super()) {
        Ok(()) => unsafe {
            if let Some(mtm) = MainThreadMarker::new() {
                NSApplication::sharedApplication(mtm).terminate(None);
            }
        },
        Err(err) => {
            eprintln!("glorp review capture failed: {err}");
            unsafe {
                if let Some(mtm) = MainThreadMarker::new() {
                    NSApplication::sharedApplication(mtm).terminate(None);
                }
            }
        }
    }
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

fn record_review_frame(
    _view: &RoundView,
    smooth_sample: Option<crate::companion::review_capture::SmoothReviewFrameSample>,
) {
    APP_STATE.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            if let Some(capture) = state.review_capture.as_mut() {
                capture.record_frame(smooth_sample);
            }
        }
    });
}

fn draw_scene(view: &RoundView, bounds: NSRect) {
    let Some(_mtm) = MainThreadMarker::new() else {
        write_boundary_diagnostic(format_args!(
            "glorp companion draw_scene called off main thread\n"
        ));
        return;
    };
    let frame = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|state| state.last_good_frame.clone())
    });
    match frame {
        Some(frame) => {
            paint_prepared_frame(view, bounds, &frame);
            record_review_frame(view, frame.review_sample);
        }
        None => paint_fallback_background(bounds),
    }
}

fn paint_prepared_frame(_view: &RoundView, bounds: NSRect, frame: &PreparedCompanionFrame) {
    let aperture = frame.aperture;
    let bg_color = frame.background;
    let dim_overlay = frame.dim_overlay;
    let commands = &frame.overlay_commands;
    let hud_text = &frame.hud;
    let hud_font_size = frame.hud_font_size;

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

        // Tank depth: a radial falloff from a lifted core to a darker rim, so the
        // porthole reads as receding water. NSGradient quantises to 8 bits with no
        // dither, and on a dark span its steps show as visible rings, so the
        // falloff is rendered once into a dithered bitmap and cached per size.
        draw_dithered_tank_background(&aperture, &bg_color);

        // Blit the shared scene draw list (habitat + pet) when grid metrics are available.
        match &frame.renderer {
            PreparedRendererFrame::Pixel { frame: pixel_frame } => {
                crate::companion::pixel::draw_pixel_frame(pixel_frame, bounds, aperture, hud_text);
            }
            PreparedRendererFrame::Smooth { metrics, plan, .. } => {
                draw_mood_aura(frame, metrics);
                appkit_blit_smooth_plan(plan, metrics, &aperture);
            }
            PreparedRendererFrame::Classic { metrics, draw_list, .. } => {
                draw_mood_aura(frame, metrics);
                appkit_blit_draw_list(
                    draw_list,
                    metrics.font_size,
                    metrics.cell_w,
                    metrics.cell_h,
                    metrics.origin_x,
                    metrics.origin_y,
                );
            }
        }

        // Companion perimeter gauges: XP, today vs yesterday, and live 10m pace.
        {
            let cx = aperture.center_x as f64;
            let cy = aperture.center_y as f64;
            let layout =
                perimeter_gauge_layout(cx, cy, aperture.radius as f64, COMPANION_GAUGE_GAP_DEG);
            let colors = perimeter_gauge_colors();
            draw_gauge_lane(&layout.xp, &colors.xp, frame.gauges.xp_fraction);
            draw_gauge_lane(&layout.daily, &colors.daily, frame.gauges.daily_fraction);
            draw_gauge_overfill(
                &layout.daily,
                &daily_overage_color(),
                frame.gauges.daily_overage_fraction,
            );
            draw_gauge_lane(&layout.pace, &colors.pace, frame.gauges.pace_fraction);
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
        draw_hud(bounds, &aperture, hud_text, hud_font_size);

        if dim_overlay {
            let dim = NSBezierPath::bezierPathWithRect(bounds);
            NSColor::colorWithSRGBRed_green_blue_alpha(0.05, 0.06, 0.10, 0.35).setFill();
            dim.fill();
        }
    }
}

fn draw_mood_aura(frame: &PreparedCompanionFrame, metrics: &CompanionGridMetrics) {
    let (pet_center_col, pet_center_row, pet_width_cells) = match &frame.renderer {
        PreparedRendererFrame::Classic {
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            ..
        }
        | PreparedRendererFrame::Smooth {
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            ..
        } => (*pet_center_col, *pet_center_row, *pet_width_cells),
        PreparedRendererFrame::Pixel { .. } => return,
    };

    let cxp = metrics.origin_x + pet_center_col * metrics.cell_w;
    let cyp = metrics.origin_y - (pet_center_row + 1.0) * metrics.cell_h;
    let max_r = pet_width_cells * metrics.cell_w * 0.95;
    const AURA_RINGS: usize = 8;
    unsafe {
        for i in 0..AURA_RINGS {
            let t = i as f64 / AURA_RINGS as f64; // 0 = outer, 1 = inner
            let rr = max_r * (1.0 - t);
            let glow = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(cxp - rr, cyp - rr),
                NSSize::new(rr * 2.0, rr * 2.0),
            ));
            ns_color(&RoundColor(
                frame.mood_aura_color.0,
                frame.mood_aura_color.1,
                frame.mood_aura_color.2,
                0.05,
            ))
            .setFill();
            glow.fill();
        }
    }
}

fn paint_fallback_background(bounds: NSRect) {
    let width = fallback_dimension(bounds.size.width);
    let height = fallback_dimension(bounds.size.height);
    let radius = width.min(height) / 2.0;
    let cx = width / 2.0;
    let cy = height / 2.0;
    unsafe {
        let bg_path = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(cx - radius, cy - radius),
            NSSize::new(radius * 2.0, radius * 2.0),
        ));
        ns_color(&RoundColor(0.05, 0.06, 0.10, 1.0)).setFill();
        bg_path.fill();
    }
}

fn fallback_dimension(value: f64) -> f64 {
    if value.is_finite() {
        value.max(1.0)
    } else {
        1.0
    }
}

/// The colour the tank's depth falloff lifts its core toward.
const TANK_DEPTH_TINT: RoundColor = RoundColor(0.10, 0.11, 0.20, 1.0);

/// How much of the tint reaches the core. Tuned against the shipping round
/// accessory panel, which lifts blacks and eats subtle deltas: the falloff has to
/// be strong enough to survive that tone curve, not merely read on a calibrated
/// Mac display.
const TANK_CORE_TINT_WEIGHT: f32 = 0.42;

/// Deterministic per-pixel noise in [-1.5, 1.5] output levels. A smooth dark
/// gradient quantised to 8 bits shows its steps as visible bands; dithering
/// trades them for imperceptible grain.
fn dither_noise(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    ((h & 0xFFFF) as f32 / 65535.0 - 0.5) * 3.0
}

/// One RGBA pixel of the tank's radial depth falloff, dithered.
fn tank_background_pixel(
    x: u32,
    y: u32,
    center: (f32, f32),
    radius: f32,
    core: &RoundColor,
    rim: &RoundColor,
) -> [u8; 4] {
    let dx = x as f32 + 0.5 - center.0;
    let dy = y as f32 + 0.5 - center.1;
    let t = if radius > 0.0 {
        ((dx * dx + dy * dy).sqrt() / radius).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let noise = dither_noise(x, y);
    let channel = |core_c: f32, rim_c: f32| {
        ((core_c + (rim_c - core_c) * t) * 255.0 + noise)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [
        channel(core.0, rim.0),
        channel(core.1, rim.1),
        channel(core.2, rim.2),
        255,
    ]
}

/// Render the dithered tank falloff, rebuilding its cached bitmap only when the
/// aperture size or background colour changes.
/// One cached background bitmap: its pixel size, its colour key, and the image.
type CachedTankBackground = (u32, u32, [u32; 3], Retained<NSImage>);

fn draw_dithered_tank_background(aperture: &RoundAperture, background: &RoundColor) {
    use std::cell::RefCell;

    // Cell size is measured on the main thread and drawing happens there too, so
    // a main-thread-only cache is sound. Keyed on size and background colour.
    thread_local! {
        static TANK_BACKGROUND: RefCell<Option<CachedTankBackground>> =
            const { RefCell::new(None) };
    }

    let width = (aperture.radius * 2.0).round().max(1.0) as u32;
    let height = width;
    let color_key = [
        background.0.to_bits(),
        background.1.to_bits(),
        background.2.to_bits(),
    ];

    let dest = NSRect::new(
        NSPoint::new(
            (aperture.center_x - aperture.radius) as f64,
            (aperture.center_y - aperture.radius) as f64,
        ),
        NSSize::new(f64::from(width), f64::from(height)),
    );

    TANK_BACKGROUND.with(|slot| {
        let mut slot = slot.borrow_mut();
        let stale = !matches!(
            slot.as_ref(),
            Some((w, h, key, _)) if *w == width && *h == height && *key == color_key
        );
        if stale {
            *slot = build_dithered_tank_image(width, height, background)
                .map(|image| (width, height, color_key, image));
        }
        if let Some((_, _, _, image)) = slot.as_ref() {
            unsafe {
                image.drawInRect_fromRect_operation_fraction(
                    dest,
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                    NSCompositingOperation::SourceOver,
                    1.0,
                );
            }
        }
    });
}

/// Fill an RGBA bitmap with the dithered radial falloff and wrap it in an image.
fn build_dithered_tank_image(
    width: u32,
    height: u32,
    background: &RoundColor,
) -> Option<Retained<NSImage>> {
    let core = tank_core_color(background);
    let center = (width as f32 / 2.0, height as f32 / 2.0);
    let radius = width.min(height) as f32 / 2.0;

    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSCalibratedRGBColorSpace,
            (width * 4) as isize,
            32,
        )?;
        let data = rep.bitmapData();
        if data.is_null() {
            return None;
        }
        for y in 0..height {
            for x in 0..width {
                let pixel = tank_background_pixel(x, y, center, radius, &core, background);
                let offset = ((y * width + x) * 4) as usize;
                std::ptr::copy_nonoverlapping(pixel.as_ptr(), data.add(offset), 4);
            }
        }

        let image = NSImage::initWithSize(
            NSImage::alloc(),
            NSSize::new(f64::from(width), f64::from(height)),
        );
        image.addRepresentation(&rep);
        Some(image)
    }
}

fn tank_core_color(background: &RoundColor) -> RoundColor {
    let mix = |base: f32, tint: f32| base + (tint - base) * TANK_CORE_TINT_WEIGHT;
    RoundColor(
        mix(background.0, TANK_DEPTH_TINT.0),
        mix(background.1, TANK_DEPTH_TINT.1),
        mix(background.2, TANK_DEPTH_TINT.2),
        background.3,
    )
}

/// Cell ink attenuated by its layer's opacity. Opacity scales alpha only, so a
/// receding layer fades into the water rather than shifting hue.
fn cell_ink_color(rgb: crate::pet::palette::Rgb, opacity: f32) -> RoundColor {
    let base = rgb_color(rgb.r, rgb.g, rgb.b);
    RoundColor(base.0, base.1, base.2, base.3 * opacity.clamp(0.0, 1.0))
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
        let font = cached_monospaced_font(font_size, false);
        let mut attr = NSMutableAttributedString::from_nsstring(&text);
        let range = objc2_foundation::NSRange::from(0..text.length());
        attr.addAttribute_value_range(NSFontAttributeName, &font, range);
        attr.addAttribute_value_range(NSForegroundColorAttributeName, &ns_color(color), range);
        attr
    }
}

/// Quantised font-cache key: the pet's depth scale drifts a hair every frame,
/// and a twentieth of a point is far below visibility, so keys stay stable
/// across frames instead of missing on every one.
fn font_cache_key(font_size: f64, bold: bool) -> (i64, bool) {
    ((font_size * 20.0).round() as i64, bold)
}

/// System monospaced fonts resolve through the font-descriptor machinery, which
/// is hot when paid per glyph per frame. Every cell in a frame shares one or two
/// fonts, so a tiny keyed cache removes the lookup entirely.
fn cached_monospaced_font(font_size: f64, bold: bool) -> Retained<NSFont> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static FONTS: RefCell<HashMap<(i64, bool), Retained<NSFont>>> =
            RefCell::new(HashMap::new());
    }

    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        // The key space is tiny in practice (a handful of sizes per session), but
        // a runaway resize loop must not grow it without bound.
        if fonts.len() > 64 {
            fonts.clear();
        }
        fonts
            .entry(font_cache_key(font_size, bold))
            .or_insert_with(|| unsafe {
                let weight = if bold { NSFontWeightBold } else { 0.0 };
                NSFont::monospacedSystemFontOfSize_weight(font_size, weight)
            })
            .clone()
    })
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

fn appkit_cell_axis(value: f32) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(0.0, f32::from(u16::MAX)) as u16
}

fn motion_binding_uses_fractional_coordinates(binding: SmoothLayerMotionBinding) -> bool {
    matches!(
        binding,
        SmoothLayerMotionBinding::PetAttached
            | SmoothLayerMotionBinding::FloorProjected
            | SmoothLayerMotionBinding::Parallax(_)
    )
}

/// The world-space point a layer's transform scales about.
fn smooth_layer_pivot(layer: &SmoothCompanionLayer) -> SmoothPoint {
    SmoothPoint {
        x: layer.anchor.x + layer.transform_origin.x,
        y: layer.anchor.y + layer.transform_origin.y,
    }
}

/// `world = pivot + (anchor + local - pivot) * scale + translation`
///
/// The caller must have validated the layer; this is the hot inner form used once
/// per cell.
fn transform_local_point(
    layer: &SmoothCompanionLayer,
    pivot: SmoothPoint,
    local: SmoothPoint,
) -> SmoothPoint {
    SmoothPoint {
        x: pivot.x
            + (layer.anchor.x + local.x - pivot.x) * layer.transform.scale.x
            + layer.transform.translation.x,
        y: pivot.y
            + (layer.anchor.y + local.y - pivot.y) * layer.transform.scale.y
            + layer.transform.translation.y,
    }
}

/// Transform a layer-local point into logical grid coordinates through the
/// layer's validated pivot, uniform scale, and translation.
fn smooth_layer_point(
    layer: &SmoothCompanionLayer,
    local: SmoothPoint,
) -> std::result::Result<SmoothPoint, SmoothGeometryError> {
    validate_smooth_layer(layer)?;
    Ok(transform_local_point(
        layer,
        smooth_layer_pivot(layer),
        local,
    ))
}

/// The AppKit rect an ellipse's layer-local bounds occupy, in the Y-up view
/// coordinate space.
fn smooth_shape_rect(
    metrics: &CompanionGridMetrics,
    layer: &SmoothCompanionLayer,
    bounds: SmoothBounds,
) -> std::result::Result<NSRect, SmoothGeometryError> {
    let min = smooth_layer_point(layer, bounds.min)?;
    let max = smooth_layer_point(layer, bounds.max)?;
    Ok(NSRect::new(
        NSPoint::new(
            metrics.origin_x + f64::from(min.x) * metrics.cell_w,
            metrics.origin_y - f64::from(max.y) * metrics.cell_h,
        ),
        NSSize::new(
            f64::from(max.x - min.x) * metrics.cell_w,
            f64::from(max.y - min.y) * metrics.cell_h,
        ),
    ))
}

/// AppKit origin (bottom-left, Y-up) of a cell scaled by `scale` whose top-left
/// sits at `world` in logical cell units. At unit scale this is exactly
/// [`fractional_cell_to_point`].
fn smooth_cell_to_point(
    world: SmoothPoint,
    scale: f64,
    cell_w: f64,
    cell_h: f64,
    origin_x: f64,
    origin_y: f64,
) -> (f64, f64) {
    (
        origin_x + f64::from(world.x) * cell_w,
        origin_y - (f64::from(world.y) + scale) * cell_h,
    )
}

fn rgba_to_nscolor(color: SmoothRgba8, opacity: f32) -> Retained<NSColor> {
    unsafe {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(color.r) / 255.0,
            f64::from(color.g) / 255.0,
            f64::from(color.b) / 255.0,
            f64::from(color.a) / 255.0 * f64::from(opacity.clamp(0.0, 1.0)),
        )
    }
}

fn compositing_operation(blend: SmoothBlendMode) -> NSCompositingOperation {
    match blend {
        SmoothBlendMode::Normal => NSCompositingOperation::SourceOver,
        SmoothBlendMode::Multiply => NSCompositingOperation::Multiply,
        SmoothBlendMode::Screen => NSCompositingOperation::Screen,
        SmoothBlendMode::Add => NSCompositingOperation::PlusLighter,
        SmoothBlendMode::Replace => NSCompositingOperation::Copy,
    }
}

/// Clip to the round porthole. Scene content never escapes it, whatever graphics
/// state the caller left behind.
unsafe fn appkit_aperture_clip(aperture: &RoundAperture) {
    NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
        NSPoint::new(
            (aperture.center_x - aperture.radius) as f64,
            (aperture.center_y - aperture.radius) as f64,
        ),
        NSSize::new(
            (aperture.radius * 2.0) as f64,
            (aperture.radius * 2.0) as f64,
        ),
    ))
    .addClip();
}

/// Clip to an oval whose centre and per-axis radii are given in cell units.
fn appkit_oval_clip(metrics: &CompanionGridMetrics, center: SmoothPoint, radii: SmoothPoint) {
    let rx = f64::from(radii.x) * metrics.cell_w;
    let ry = f64::from(radii.y) * metrics.cell_h;
    let cx = metrics.origin_x + f64::from(center.x) * metrics.cell_w;
    let cy = metrics.origin_y - f64::from(center.y) * metrics.cell_h;
    unsafe {
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(cx - rx, cy - ry),
            NSSize::new(rx * 2.0, ry * 2.0),
        ))
        .addClip();
    }
}

/// Intersect the layer's own clip with the aperture clip the caller installed.
fn apply_smooth_layer_clip(clip: &SmoothClip, metrics: &CompanionGridMetrics) {
    unsafe {
        match clip {
            SmoothClip::None => {}
            SmoothClip::Rect(bounds) => {
                NSBezierPath::bezierPathWithRect(NSRect::new(
                    NSPoint::new(
                        metrics.origin_x + f64::from(bounds.min.x) * metrics.cell_w,
                        metrics.origin_y - f64::from(bounds.max.y) * metrics.cell_h,
                    ),
                    NSSize::new(
                        f64::from(bounds.max.x - bounds.min.x) * metrics.cell_w,
                        f64::from(bounds.max.y - bounds.min.y) * metrics.cell_h,
                    ),
                ))
                .addClip();
            }
            // Cells are not square, so a circle of cells is an ellipse in pixels.
            SmoothClip::Circle { center, radius } => {
                appkit_oval_clip(metrics, *center, SmoothPoint { x: *radius, y: *radius });
            }
            SmoothClip::Ellipse { center, radii } => {
                appkit_oval_clip(metrics, *center, *radii);
            }
        }
    }
}

/// Blit a validated Smooth scene plan. Every layer is clipped to the aperture
/// inside its own saved graphics state, then intersected with the layer's own
/// clip. The blit does not rely on an aperture clip installed by the caller
/// surviving its save/restore pairs.
///
/// The plan is validated during frame preparation, so an invalid layer here means
/// a bug rather than bad input: skip it instead of drawing garbage or panicking.
fn appkit_blit_smooth_plan(
    plan: &SmoothCompanionScenePlan,
    metrics: &CompanionGridMetrics,
    aperture: &RoundAperture,
) {
    let CompanionGridMetrics {
        font_size,
        cell_w,
        cell_h,
        origin_x,
        origin_y,
        ..
    } = *metrics;

    let mut ordered_layers: Vec<_> = plan.layers.iter().enumerate().collect();
    ordered_layers.sort_by_key(|(index, layer)| (layer.z, *index));

    for (_, layer) in ordered_layers {
        if layer.opacity <= 0.0 || validate_smooth_layer(layer).is_err() {
            continue;
        }

        let pivot = smooth_layer_pivot(layer);
        // Validation guarantees the scale is uniform and positive.
        let scale = f64::from(layer.transform.scale.x);
        let fractional = motion_binding_uses_fractional_coordinates(layer.motion_binding);

        unsafe {
            NSGraphicsContext::saveGraphicsState_class();
            if let Some(context) = NSGraphicsContext::currentContext() {
                context.setCompositingOperation(compositing_operation(layer.blend));
            }
            appkit_aperture_clip(aperture);
            apply_smooth_layer_clip(&layer.clip, metrics);
        }

        for item in &layer.items {
            match item {
                SmoothLayerItem::LocalCell(cell) => {
                    let world = transform_local_point(
                        layer,
                        pivot,
                        SmoothPoint {
                            x: f32::from(cell.col),
                            y: f32::from(cell.row),
                        },
                    );
                    let (px, py) = if fractional {
                        smooth_cell_to_point(world, scale, cell_w, cell_h, origin_x, origin_y)
                    } else {
                        cell_to_point(
                            appkit_cell_axis(world.x),
                            appkit_cell_axis(world.y),
                            cell_w,
                            cell_h,
                            origin_x,
                            origin_y,
                        )
                    };
                    // Cell size and glyph size scale together, so a grown pet keeps
                    // its proportions instead of spreading its glyphs apart.
                    appkit_draw_cell_parts(
                        cell.glyph.as_deref(),
                        cell.fg,
                        cell.bg,
                        cell.bold,
                        AppkitCellFrame {
                            px,
                            py,
                            font_size: font_size * scale,
                            cell_w: cell_w * scale,
                            cell_h: cell_h * scale,
                            opacity: layer.opacity,
                        },
                    );
                }
                SmoothLayerItem::Shape(shape) => {
                    let SmoothShapeGeometry::Ellipse { bounds } = shape.geometry;
                    let Ok(rect) = smooth_shape_rect(metrics, layer, bounds) else {
                        continue;
                    };
                    unsafe {
                        let path = NSBezierPath::bezierPathWithOvalInRect(rect);
                        match shape.fill {
                            SmoothFill::Solid(color) => {
                                rgba_to_nscolor(color, layer.opacity).setFill();
                                path.fill();
                            }
                            SmoothFill::RadialGradient { inner, outer } => {
                                let gradient = NSGradient::initWithStartingColor_endingColor(
                                    NSGradient::alloc(),
                                    &rgba_to_nscolor(inner, layer.opacity),
                                    &rgba_to_nscolor(outer, layer.opacity),
                                );
                                if let Some(gradient) = gradient {
                                    // Relative centre (0, 0) is the shape's own centre.
                                    gradient.drawInBezierPath_relativeCenterPosition(
                                        &path,
                                        NSPoint::new(0.0, 0.0),
                                    );
                                }
                            }
                            SmoothFill::LinearGradientY { top, bottom } => {
                                // AppKit is Y-up: angle 90 draws the starting
                                // colour at the rect's bottom edge, which is the
                                // shape's bottom in cell space.
                                let gradient = NSGradient::initWithStartingColor_endingColor(
                                    NSGradient::alloc(),
                                    &rgba_to_nscolor(bottom, layer.opacity),
                                    &rgba_to_nscolor(top, layer.opacity),
                                );
                                if let Some(gradient) = gradient {
                                    gradient.drawInBezierPath_angle(&path, 90.0);
                                }
                            }
                        }
                    }
                }
                // Rasters are descriptive only; this slice has no raster backend and
                // must not silently reinterpret one as a shape.
                SmoothLayerItem::Raster(_) => continue,
            }
        }

        unsafe {
            NSGraphicsContext::restoreGraphicsState_class();
        }
    }
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
    for cell in &list.cells {
        if cell.bg.is_none() && cell.glyph.is_none() {
            continue;
        }

        let (px, py) = cell_to_point(cell.col, cell.row, cell_w, cell_h, origin_x, origin_y);

        appkit_draw_cell_parts(
            cell.glyph.as_deref(),
            cell.fg,
            cell.bg,
            cell.bold,
            AppkitCellFrame {
                px,
                py,
                font_size,
                cell_w,
                cell_h,
                opacity: 1.0,
            },
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct AppkitCellFrame {
    px: f64,
    py: f64,
    font_size: f64,
    cell_w: f64,
    cell_h: f64,
    /// The owning layer's opacity, multiplied into every ink alpha.
    opacity: f32,
}

fn appkit_draw_cell_parts(
    glyph: Option<&str>,
    fg: Option<crate::pet::palette::Rgb>,
    bg: Option<crate::pet::palette::Rgb>,
    bold: bool,
    frame: AppkitCellFrame,
) {
    unsafe {
        if let Some(bg) = bg {
            let bg_color = cell_ink_color(bg, frame.opacity);
            let path = NSBezierPath::bezierPathWithRect(NSRect::new(
                NSPoint::new(frame.px, frame.py),
                NSSize::new(frame.cell_w, frame.cell_h),
            ));
            ns_color(&bg_color).setFill();
            path.fill();
        }

        if let Some(glyph) = glyph {
            let fg = fg
                .map(|c| cell_ink_color(c, frame.opacity))
                .unwrap_or(RoundColor(1.0, 1.0, 1.0, frame.opacity.clamp(0.0, 1.0)));
            let attr = if bold {
                // `attributed_pet_glyph` uses weight 0.0 (NSFontWeightRegular).
                // For bold cells we build the attributed string with NSFontWeightBold.
                let text = NSString::from_str(glyph);
                let font = cached_monospaced_font(frame.font_size, true);
                let mut a = NSMutableAttributedString::from_nsstring(&text);
                let range = objc2_foundation::NSRange::from(0..text.length());
                a.addAttribute_value_range(NSFontAttributeName, &font, range);
                a.addAttribute_value_range(NSForegroundColorAttributeName, &ns_color(&fg), range);
                a
            } else {
                attributed_pet_glyph(glyph, frame.font_size, &fg)
            };
            attr.drawAtPoint(NSPoint::new(frame.px, frame.py));
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
fn live_hud_text(vm: &WatchViewModel) -> CompanionHudText {
    companion_hud_text(
        vm.today_effective_tokens,
        vm.daily_comparison.fraction_of_yesterday,
        vm.rate_momentum.pulse.current_tokens,
    )
}

#[cfg(target_os = "macos")]
fn review_capture_hud_text() -> CompanionHudText {
    CompanionHudText {
        today_total: "review".into(),
        daily_percent: "privacy".into(),
        pace: "redacted".into(),
    }
}

#[cfg(target_os = "macos")]
fn draw_hud(bounds: NSRect, aperture: &RoundAperture, hud_text: &CompanionHudText, font_size: f64) {
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
    unsafe {
        let big_color = RoundColor(0.93, 0.93, 0.97, 1.0);
        let sub_color =
            crate::round::hud::rate_direction_color(crate::tui::view_model::RateDirection::Neutral);
        let mut stack_size = font_size * 1.45;
        let mut rendered =
            companion_hud_attributed_lines(hud_text, stack_size, &big_color, &sub_color);

        while (rendered.max_width > gap.max_width
            || rendered.total_height > aperture.radius as f64 * 0.34)
            && stack_size > 6.0
        {
            stack_size -= 1.0;
            rendered = companion_hud_attributed_lines(hud_text, stack_size, &big_color, &sub_color);
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
    fn objc_callback_guard_catches_unwind() {
        let did_run = std::cell::Cell::new(false);

        run_objc_callback("drawRect", || {
            did_run.set(true);
            panic!("injected callback panic");
        });

        assert!(did_run.get());
    }

    #[test]
    fn fallback_dimension_replaces_non_finite_values() {
        assert_eq!(fallback_dimension(f64::INFINITY), 1.0);
        assert_eq!(fallback_dimension(f64::NEG_INFINITY), 1.0);
        assert_eq!(fallback_dimension(f64::NAN), 1.0);
        assert_eq!(fallback_dimension(0.5), 1.0);
        assert_eq!(fallback_dimension(4.0), 4.0);
    }

    #[test]
    fn prepared_gauge_frame_matches_current_vm_values() {
        let mut vm = WatchViewModel::fixture();
        vm.progress.is_max_stage = false;
        vm.progress.fraction = 0.42;
        vm.daily_comparison.fraction_of_yesterday = Some(1.25);
        vm.rate_momentum.pulse.current_tokens = 31_000_000.0;

        let gauges = prepare_gauge_frame(&vm);

        assert_eq!(gauges.xp_fraction, vm.progress.fraction as f64);
        assert_eq!(gauges.daily_fraction, daily_fraction_for_gauge(Some(1.25)));
        assert_eq!(
            gauges.daily_overage_fraction,
            daily_overage_marker_fraction(Some(1.25))
        );
        assert_eq!(gauges.pace_fraction, companion_pace_fraction(31_000_000.0));
    }

    #[test]
    fn prepared_hud_text_uses_review_redaction_when_requested() {
        let mut vm = WatchViewModel::fixture();
        vm.today_effective_tokens = 842_000_000.0;
        vm.daily_comparison.fraction_of_yesterday = Some(0.94);
        vm.rate_momentum.pulse.current_tokens = 31_000_000.0;

        assert_eq!(prepare_hud_frame(&vm, true), review_capture_hud_text());
        assert_eq!(prepare_hud_frame(&vm, false), live_hud_text(&vm));
    }

    #[test]
    fn prepared_pixel_frame_uses_draw_path_hud_font_metrics_and_fallback() {
        let vm = WatchViewModel::fixture();
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let scene = derive_round_scene_model(&vm, now);
        let measured_bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(480.0, 360.0));
        let expected_font_size =
            companion_grid_metrics(measured_bounds.size.width, measured_bounds.size.height)
                .expect("normal companion bounds should produce grid metrics")
                .font_size;
        assert_ne!(
            expected_font_size, 8.5,
            "fixture must distinguish metric parity"
        );

        let mut metric_cache = CompanionMetricCache::default();
        let measured = prepare_companion_frame(
            &vm,
            &scene,
            CompanionRendererMode::Pixel,
            None,
            None,
            None,
            0,
            false,
            measured_bounds,
            &mut metric_cache,
        )
        .unwrap();

        assert_eq!(measured.hud_font_size, expected_font_size);

        let fallback_bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(480.0, 1.0));
        assert!(
            companion_grid_metrics(fallback_bounds.size.width, fallback_bounds.size.height,)
                .is_none()
        );
        let fallback = prepare_companion_frame(
            &vm,
            &scene,
            CompanionRendererMode::Pixel,
            None,
            None,
            None,
            0,
            false,
            fallback_bounds,
            &mut metric_cache,
        )
        .expect("missing Pixel metrics should not fail frame preparation");

        assert_eq!(fallback.hud_font_size, 8.5);
    }

    #[test]
    fn prepared_bounds_rejects_zero_negative_non_finite_and_oversized_values() {
        for size in [
            NSSize::new(0.0, 360.0),
            NSSize::new(360.0, 0.0),
            NSSize::new(0.5, 360.0),
            NSSize::new(360.0, 0.5),
            NSSize::new(-1.0, 360.0),
            NSSize::new(360.0, -1.0),
            NSSize::new(f64::NAN, 360.0),
            NSSize::new(360.0, f64::INFINITY),
            NSSize::new(f64::from(u16::MAX) + 1.0, 360.0),
            NSSize::new(360.0, f64::from(u16::MAX) + 1.0),
        ] {
            let bounds = NSRect::new(NSPoint::new(0.0, 0.0), size);
            assert_eq!(
                prepare_bounds(bounds).unwrap_err(),
                CompanionFramePreparationError::InvalidBounds
            );
        }
    }

    #[test]
    fn prepared_bounds_accepts_normal_companion_size() {
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 360.0));
        let prepared = prepare_bounds(bounds).unwrap();

        assert_eq!(prepared.width_px, 360);
        assert_eq!(prepared.height_px, 360);
        assert_eq!(prepared.width_f64, 360.0);
        assert_eq!(prepared.height_f64, 360.0);
    }

    #[test]
    fn prepared_bounds_truncates_fractional_dimensions_like_draw_path() {
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.6, 360.6));

        let prepared = prepare_bounds(bounds).unwrap();

        assert_eq!(prepared.width_px, 360);
        assert_eq!(prepared.height_px, 360);
    }

    #[test]
    fn companion_metric_cache_recomputes_for_fractional_resize() {
        let first = PreparedBounds {
            width_px: 360,
            height_px: 360,
            width_f64: 360.1,
            height_f64: 360.1,
        };
        let second = PreparedBounds {
            width_px: 360,
            height_px: 360,
            width_f64: 360.9,
            height_f64: 360.9,
        };
        let mut cache = CompanionMetricCache::default();

        let first_metrics = cache.metrics_for(first).unwrap();
        let second_metrics = cache.metrics_for(second).unwrap();
        let expected_second = companion_grid_metrics(second.width_f64, second.height_f64).unwrap();

        assert_eq!(second_metrics, expected_second);
        assert_ne!(first_metrics, second_metrics);
    }

    #[test]
    fn repeated_frame_preparation_errors_are_throttled_per_category() {
        assert!(should_record_frame_preparation_error(
            None,
            CompanionFramePreparationError::InvalidBounds
        ));
        assert!(!should_record_frame_preparation_error(
            Some(CompanionFramePreparationError::InvalidBounds),
            CompanionFramePreparationError::InvalidBounds
        ));
        assert!(should_record_frame_preparation_error(
            Some(CompanionFramePreparationError::InvalidBounds),
            CompanionFramePreparationError::SmoothMissingPetBody
        ));
    }

    #[test]
    fn moving_bindings_use_fractional_appkit_coordinates() {
        use crate::presentation::smooth::{SmoothDepthPlane, SmoothLayerMotionBinding};

        assert!(!motion_binding_uses_fractional_coordinates(
            SmoothLayerMotionBinding::Fixed
        ));
        assert!(motion_binding_uses_fractional_coordinates(
            SmoothLayerMotionBinding::PetAttached
        ));
        assert!(motion_binding_uses_fractional_coordinates(
            SmoothLayerMotionBinding::FloorProjected
        ));
        assert!(motion_binding_uses_fractional_coordinates(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Far)
        ));
        assert!(motion_binding_uses_fractional_coordinates(
            SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Foreground)
        ));

        let fractional =
            smooth_cell_to_point(SmoothPoint { x: 10.1, y: 4.0 }, 1.0, 30.0, 60.0, 0.0, 960.0);
        let snapped = cell_to_point(
            appkit_cell_axis(10.1),
            appkit_cell_axis(4.0),
            30.0,
            60.0,
            0.0,
            960.0,
        );
        // Scene-plan coordinates are f32, so the tolerance tracks single-precision
        // rather than the exact f64 arithmetic this helper used to be handed.
        assert!((fractional.0 - 303.0).abs() < 1e-4);
        assert_eq!(snapped.0, 300.0);
    }

    #[test]
    fn parallax_geometry_failure_has_a_distinct_static_category() {
        assert_eq!(
            CompanionFramePreparationError::SmoothInvalidParallaxGeometry.category(),
            "smooth-invalid-parallax-geometry"
        );
        assert!(should_record_frame_preparation_error(
            Some(CompanionFramePreparationError::SmoothMissingPetBody),
            CompanionFramePreparationError::SmoothInvalidParallaxGeometry,
        ));
    }

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
    fn review_capture_hud_text_does_not_echo_live_token_strings() {
        let live_text =
            crate::round::hud::companion_hud_text(842_000_000.0, Some(0.94), 31_000_000.0);
        let capture_text = review_capture_hud_text();

        for live_value in [
            live_text.today_total,
            live_text.daily_percent,
            live_text.pace,
            "842M".to_string(),
            "94% yday".to_string(),
            "31M/10m".to_string(),
        ] {
            assert!(!capture_text.today_total.contains(&live_value));
            assert!(!capture_text.daily_percent.contains(&live_value));
            assert!(!capture_text.pace.contains(&live_value));
        }
    }

    #[test]
    fn smooth_startup_prepares_classic_pet_art_before_first_semantic_tick() {
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_art.clear();
        vm.pet_spans.clear();
        vm.day_context.asleep = false;

        prepare_smooth_view_model_for_tick(&mut vm, 0, now).unwrap();

        assert!(!vm.pet_art.is_empty());
        assert!(!vm.pet_spans.is_empty());
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

    #[test]
    fn post_poll_review_state_active_pulse_reapplies_after_live_update() {
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let mut presentation_state = WatchPresentationState::default();
        let update = LiveWatchUpdate {
            pet_state: crate::storage::state::PetState::new_for_test("seed", "glorp"),
            vm: WatchViewModel::fixture(),
            applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
                now,
                time::Duration::seconds(10),
            ),
        };

        let (vm, _, pixel_input) = apply_post_poll_update(
            &mut presentation_state,
            CompanionReviewState::ActivePulse,
            CompanionRendererMode::Classic,
            update,
            now,
            0,
        )
        .unwrap();

        assert_eq!(vm.last_feed_pulse_at, Some(now));
        assert!(vm.life_profile.burst_level > 0.0);
        assert!(pixel_input.is_none());
    }

    #[test]
    fn post_poll_review_state_asleep_calm_reapplies_after_live_update() {
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let mut presentation_state = WatchPresentationState::default();
        let mut vm = WatchViewModel::fixture();
        vm.day_context.asleep = false;
        vm.life_profile.calm_mode = false;
        vm.last_feed_pulse_at = Some(now);
        let update = LiveWatchUpdate {
            pet_state: crate::storage::state::PetState::new_for_test("seed", "glorp"),
            vm,
            applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
                now,
                time::Duration::seconds(10),
            ),
        };

        let (vm, _, _) = apply_post_poll_update(
            &mut presentation_state,
            CompanionReviewState::AsleepCalm,
            CompanionRendererMode::Classic,
            update,
            now,
            0,
        )
        .unwrap();

        assert!(vm.day_context.asleep);
        assert!(vm.life_profile.calm_mode);
        assert_eq!(vm.last_feed_pulse_at, None);
    }

    #[test]
    fn post_poll_review_state_helper_trouble_reapplies_after_live_update() {
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let mut presentation_state = WatchPresentationState::default();
        let update = LiveWatchUpdate {
            pet_state: crate::storage::state::PetState::new_for_test("seed", "glorp"),
            vm: WatchViewModel::fixture(),
            applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
                now,
                time::Duration::seconds(10),
            ),
        };

        let (vm, _, _) = apply_post_poll_update(
            &mut presentation_state,
            CompanionReviewState::HelperTrouble,
            CompanionRendererMode::Classic,
            update,
            now,
            0,
        )
        .unwrap();

        let source = vm.source_health.first().expect("fixture source health");
        assert_eq!(source.status, SourceStatus::Diagnostic);
        assert_eq!(source.diagnostic_code.as_deref(), Some("review-state"));
        assert_eq!(source.diagnostic_message, None);
    }

    #[test]
    fn post_poll_review_state_normal_preserves_live_behavior() {
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let mut presentation_state = WatchPresentationState::default();
        let update = LiveWatchUpdate {
            pet_state: crate::storage::state::PetState::new_for_test("seed", "glorp"),
            vm: WatchViewModel::fixture(),
            applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
                now,
                time::Duration::seconds(10),
            ),
        };

        let (vm, _, _) = apply_post_poll_update(
            &mut presentation_state,
            CompanionReviewState::Normal,
            CompanionRendererMode::Classic,
            update,
            now,
            0,
        )
        .unwrap();

        assert!(!vm.day_context.asleep);
        assert!(!vm.life_profile.calm_mode);
        assert_eq!(vm.last_feed_pulse_at, None);
        let source = vm.source_health.first().expect("fixture source health");
        assert_ne!(source.status, SourceStatus::Diagnostic);
    }

    #[test]
    fn smooth_post_poll_update_prepares_pet_art_before_next_semantic_tick() {
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let mut presentation_state = WatchPresentationState::default();
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_art.clear();
        vm.pet_spans.clear();
        let update = LiveWatchUpdate {
            pet_state: crate::storage::state::PetState::new_for_test("seed", "glorp"),
            vm,
            applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
                now,
                time::Duration::seconds(10),
            ),
        };

        let (vm, scene, pixel_input) = apply_post_poll_update(
            &mut presentation_state,
            CompanionReviewState::Normal,
            CompanionRendererMode::Smooth,
            update,
            now,
            0,
        )
        .unwrap();

        assert!(!vm.pet_art.is_empty());
        assert!(!vm.pet_spans.is_empty());
        assert!(!scene.pet.art_lines.is_empty());
        assert!(pixel_input.is_none());
    }

    #[test]
    fn smooth_post_poll_update_uses_current_semantic_tick_when_preparing_pet_art() {
        let now = time::macros::datetime!(2026-07-08 12:00 UTC);
        let current_tick = 7;
        let mut presentation_state = WatchPresentationState::default();
        let mut update_vm = WatchViewModel::fixture_with_habitat_props();
        update_vm.pet_render.generated_species = crate::pet::generation::Species::Glitch;
        update_vm.pet_render.stage = crate::game::evolution::Stage::S4;
        update_vm.pet_art.clear();
        update_vm.pet_spans.clear();

        let mut expected = update_vm.clone();
        let mut expected_presentation = WatchPresentationState::default();
        crate::watch_live::stamp_live_presentation(
            &mut expected_presentation,
            &mut expected,
            crate::tui::life::AppliedUsageSignal::diagnostics_only(
                now,
                time::Duration::seconds(10),
            ),
            now,
        );
        apply_review_state(
            CompanionReviewState::Normal,
            &mut expected_presentation,
            &mut expected,
            now,
        )
        .unwrap();
        rerender_pet_for_view_model(&mut expected, current_tick, false, now).unwrap();
        let expected_checksum = crate::presentation::smooth::pet_visual_checksum(
            &expected.pet_art,
            &expected.pet_spans,
        );

        let mut tick_zero = expected.clone();
        rerender_pet_for_view_model(&mut tick_zero, 0, false, now).unwrap();
        assert_ne!(
            expected_checksum,
            crate::presentation::smooth::pet_visual_checksum(
                &tick_zero.pet_art,
                &tick_zero.pet_spans,
            ),
            "test fixture must distinguish the current semantic tick from tick 0"
        );

        let update = LiveWatchUpdate {
            pet_state: crate::storage::state::PetState::new_for_test("seed", "glorp"),
            vm: update_vm,
            applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
                now,
                time::Duration::seconds(10),
            ),
        };

        let (vm, _, _) = apply_post_poll_update(
            &mut presentation_state,
            CompanionReviewState::Normal,
            CompanionRendererMode::Smooth,
            update,
            now,
            current_tick,
        )
        .unwrap();

        assert_eq!(
            crate::presentation::smooth::pet_visual_checksum(&vm.pet_art, &vm.pet_spans),
            expected_checksum
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

#[cfg(test)]
mod tank_depth_gradient_tests {
    use super::*;

    #[test]
    fn tank_core_lifts_the_background_toward_the_depth_tint() {
        let bg = RoundColor(0.05, 0.05, 0.06, 1.0);
        let core = tank_core_color(&bg);

        // The core is a blend toward the tint, so it sits strictly between the two.
        assert!(core.0 > bg.0 && core.0 < TANK_DEPTH_TINT.0);
        assert!(core.1 > bg.1 && core.1 < TANK_DEPTH_TINT.1);
        assert!(core.2 > bg.2 && core.2 < TANK_DEPTH_TINT.2);
        // The porthole is opaque; only the tint's weight varies.
        assert_eq!(core.3, 1.0);
    }

    #[test]
    fn tank_core_reproduces_the_weight_the_stepped_rings_accumulated() {
        let bg = RoundColor(0.0, 0.0, 0.0, 1.0);
        let core = tank_core_color(&bg);
        assert!((core.0 - TANK_DEPTH_TINT.0 * TANK_CORE_TINT_WEIGHT).abs() < 1e-6);
    }
}

#[cfg(test)]
mod smooth_opacity_tests {
    use super::*;
    use crate::pet::palette::Rgb;

    #[test]
    fn cell_ink_alpha_scales_with_layer_opacity() {
        let opaque = cell_ink_color(Rgb { r: 255, g: 128, b: 0 }, 1.0);
        assert_eq!(opaque.3, 1.0);
        assert_eq!(opaque.0, 1.0);

        let half = cell_ink_color(Rgb { r: 255, g: 128, b: 0 }, 0.5);
        assert_eq!(half.3, 0.5);
        // Opacity attenuates alpha, never the colour itself.
        assert_eq!(half.0, opaque.0);
        assert_eq!(half.1, opaque.1);
        assert_eq!(half.2, opaque.2);
    }

    #[test]
    fn cell_ink_alpha_is_clamped_into_the_unit_range() {
        assert_eq!(cell_ink_color(Rgb { r: 1, g: 2, b: 3 }, -1.0).3, 0.0);
        assert_eq!(cell_ink_color(Rgb { r: 1, g: 2, b: 3 }, 2.0).3, 1.0);
    }
}

#[cfg(test)]
mod smooth_geometry_tests {
    use super::*;
    use crate::presentation::smooth::{
        SmoothBlendMode, SmoothClip, SmoothCompanionPrivacyClaims, SmoothLayerId, SmoothLayerRole,
        SmoothRgba8, SmoothShape, SmoothShapeGeometry, SmoothTransform,
    };

    fn point(x: f32, y: f32) -> SmoothPoint {
        SmoothPoint { x, y }
    }

    fn bounds(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> SmoothBounds {
        SmoothBounds {
            min: point(min_x, min_y),
            max: point(max_x, max_y),
        }
    }

    /// A 2x2 layer anchored at (10, 20) whose pivot is its own centre.
    fn layer() -> SmoothCompanionLayer {
        SmoothCompanionLayer {
            id: SmoothLayerId("shape-layer".to_string()),
            role: SmoothLayerRole::PetBody,
            motion_binding: SmoothLayerMotionBinding::PetAttached,
            z: 0,
            local_bounds: bounds(0.0, 0.0, 2.0, 2.0),
            anchor: point(10.0, 20.0),
            transform_origin: point(1.0, 1.0),
            transform: SmoothTransform {
                translation: point(0.0, 0.0),
                scale: point(1.0, 1.0),
                rotation_degrees: 0.0,
            },
            parallax_translation: point(0.0, 0.0),
            opacity: 1.0,
            clip: SmoothClip::None,
            blend: SmoothBlendMode::Normal,
            items: vec![SmoothLayerItem::Shape(SmoothShape {
                geometry: SmoothShapeGeometry::Ellipse { bounds: bounds(0.0, 0.0, 2.0, 2.0) },
                fill: SmoothFill::Solid(SmoothRgba8 { r: 1, g: 2, b: 3, a: 255 }),
            })],
            privacy: SmoothCompanionPrivacyClaims::external_companion(),
        }
    }

    fn metrics() -> CompanionGridMetrics {
        CompanionGridMetrics {
            font_size: 10.0,
            cell_w: 4.0,
            cell_h: 8.0,
            grid_cols: 36,
            grid_rows: 18,
            origin_x: 100.0,
            origin_y: 200.0,
        }
    }

    #[test]
    fn smooth_layer_point_is_the_identity_at_unit_scale() {
        let layer = layer();
        assert_eq!(
            smooth_layer_point(&layer, point(0.0, 0.0)).unwrap(),
            point(10.0, 20.0)
        );
        assert_eq!(
            smooth_layer_point(&layer, point(2.0, 2.0)).unwrap(),
            point(12.0, 22.0)
        );
    }

    #[test]
    fn smooth_layer_point_scales_about_the_pivot_and_leaves_it_fixed() {
        let mut near = layer();
        near.transform.scale = point(1.12, 1.12);
        // The pivot is anchor + transform_origin = (11, 21) and must not move.
        assert_eq!(
            smooth_layer_point(&near, point(1.0, 1.0)).unwrap(),
            point(11.0, 21.0)
        );
        // Corners push out by half the extra extent.
        let min = smooth_layer_point(&near, point(0.0, 0.0)).unwrap();
        let max = smooth_layer_point(&near, point(2.0, 2.0)).unwrap();
        assert!((max.x - min.x - 2.24).abs() < 1e-5);
        assert!((max.y - min.y - 2.24).abs() < 1e-5);

        let mut far = layer();
        far.transform.scale = point(0.88, 0.88);
        let min = smooth_layer_point(&far, point(0.0, 0.0)).unwrap();
        let max = smooth_layer_point(&far, point(2.0, 2.0)).unwrap();
        assert!((max.x - min.x - 1.76).abs() < 1e-5);
    }

    #[test]
    fn smooth_layer_point_applies_translation_after_scale() {
        let mut layer = layer();
        layer.transform.scale = point(1.12, 1.12);
        layer.transform.translation = point(3.0, -4.0);
        assert_eq!(
            smooth_layer_point(&layer, point(1.0, 1.0)).unwrap(),
            point(14.0, 17.0)
        );
    }

    #[test]
    fn smooth_layer_point_rejects_invalid_geometry() {
        let mut rotated = layer();
        rotated.transform.rotation_degrees = 1.0;
        assert_eq!(
            smooth_layer_point(&rotated, point(0.0, 0.0)),
            Err(SmoothGeometryError::RotationUnsupported)
        );

        let mut nonuniform = layer();
        nonuniform.transform.scale = point(1.0, 1.2);
        assert_eq!(
            smooth_layer_point(&nonuniform, point(0.0, 0.0)),
            Err(SmoothGeometryError::NonUniformScale)
        );

        let mut nonpositive = layer();
        nonpositive.transform.scale = point(0.0, 0.0);
        assert_eq!(
            smooth_layer_point(&nonpositive, point(0.0, 0.0)),
            Err(SmoothGeometryError::NonPositiveScale)
        );
    }

    #[test]
    fn smooth_shape_rect_maps_cells_to_appkit_pixels_with_y_flipped() {
        let metrics = metrics();
        let rect = smooth_shape_rect(&metrics, &layer(), bounds(0.0, 0.0, 2.0, 2.0)).unwrap();

        // x grows rightward from the grid origin; the ellipse spans cols 10..12.
        assert_eq!(rect.origin.x, 100.0 + 10.0 * 4.0);
        assert_eq!(rect.size.width, 2.0 * 4.0);
        // AppKit is Y-up, so the rect's origin is the *bottom* of rows 20..22.
        assert_eq!(rect.origin.y, 200.0 - 22.0 * 8.0);
        assert_eq!(rect.size.height, 2.0 * 8.0);
    }

    #[test]
    fn smooth_shape_rect_grows_the_ellipse_with_the_layer_scale() {
        let metrics = metrics();
        let mut near = layer();
        near.transform.scale = point(1.12, 1.12);

        let unit = smooth_shape_rect(&metrics, &layer(), bounds(0.0, 0.0, 2.0, 2.0)).unwrap();
        let scaled = smooth_shape_rect(&metrics, &near, bounds(0.0, 0.0, 2.0, 2.0)).unwrap();

        assert!((scaled.size.width / unit.size.width - 1.12).abs() < 1e-5);
        assert!((scaled.size.height / unit.size.height - 1.12).abs() < 1e-5);
    }

    #[test]
    fn smooth_shape_rect_rejects_invalid_geometry() {
        let metrics = metrics();
        let mut rotated = layer();
        rotated.transform.rotation_degrees = 1.0;
        assert_eq!(
            smooth_shape_rect(&metrics, &rotated, bounds(0.0, 0.0, 2.0, 2.0)),
            Err(SmoothGeometryError::RotationUnsupported)
        );
    }

    #[test]
    fn smooth_cell_to_point_places_a_unit_cell_at_its_own_row() {
        // Y-up: the cell's AppKit origin is the bottom of row 4.25, one cell down.
        let (px, py) = smooth_cell_to_point(point(3.5, 4.25), 1.0, 4.0, 8.0, 100.0, 200.0);
        assert_eq!(px, 100.0 + 3.5 * 4.0);
        assert_eq!(py, 200.0 - 5.25 * 8.0);
    }

    #[test]
    fn smooth_cell_to_point_drops_a_scaled_cell_from_its_top_left() {
        // A 1.12x cell whose top-left is at row 4 has its bottom at row 5.12.
        let (_, py) = smooth_cell_to_point(point(0.0, 4.0), 1.12, 4.0, 8.0, 100.0, 200.0);
        assert!((py - (200.0 - 5.12 * 8.0)).abs() < 1e-9);
    }
}

#[cfg(test)]
mod font_cache_tests {
    use super::*;

    #[test]
    fn font_cache_key_is_stable_under_depth_scale_drift_but_splits_real_changes() {
        // Adjacent frames of a depth transition differ by well under a
        // twentieth of a point and must share a key.
        assert_eq!(
            font_cache_key(21.4001, false),
            font_cache_key(21.4103, false)
        );
        // Distinct rendered sizes and weights must not collide.
        assert_ne!(font_cache_key(21.4, false), font_cache_key(24.0, false));
        assert_ne!(font_cache_key(21.4, false), font_cache_key(21.4, true));
    }
}

#[cfg(test)]
mod tank_dither_tests {
    use super::*;

    #[test]
    fn dither_noise_is_deterministic_bounded_and_varied() {
        let mut seen = std::collections::BTreeSet::new();
        for x in 0..64u32 {
            for y in 0..64u32 {
                let n = dither_noise(x, y);
                assert_eq!(n, dither_noise(x, y));
                assert!((-1.5..=1.5).contains(&n), "noise {n} out of range");
                seen.insert((n * 1000.0) as i32);
            }
        }
        assert!(
            seen.len() > 100,
            "noise must vary, got {} values",
            seen.len()
        );
    }

    #[test]
    fn tank_background_pixel_runs_core_to_rim_with_grain_smaller_than_a_band() {
        let core = RoundColor(0.20, 0.22, 0.30, 1.0);
        let rim = RoundColor(0.05, 0.05, 0.06, 1.0);
        let center = (100.0, 100.0);

        let at_core = tank_background_pixel(100, 100, center, 100.0, &core, &rim);
        let at_rim = tank_background_pixel(199, 100, center, 100.0, &core, &rim);
        assert!((f32::from(at_core[0]) - 0.20 * 255.0).abs() <= 2.0);
        assert!((f32::from(at_rim[0]) - 0.05 * 255.0).abs() <= 2.0);
        assert_eq!(at_core[3], 255);

        // Past the radius the falloff clamps: only the dither grain remains.
        let beyond = tank_background_pixel(390, 100, center, 100.0, &core, &rim);
        assert!((f32::from(beyond[0]) - 0.05 * 255.0).abs() <= 2.0);
    }
}
