//! Native macOS round companion window. Uses a regular Dock app lifecycle,
//! a worker thread for live usage polling, and pure AppKit drawing from
//! `RoundSceneModel`.

#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::commands::companion_mode::{
    resolve_renderer, CompanionRendererRequest, CompanionRendererTarget, CompanionReviewDepth,
    CompanionReviewOptions, CompanionReviewSize, CompanionReviewState, EffectiveCompanionRenderer,
    RendererRuntimeState, AUTO_RETAINED_ON_APPLE_SILICON,
};
#[cfg(feature = "retained-renderer")]
use crate::commands::companion_mode::{
    resolve_scene_rollout, SceneRuntimeRollout, AUTO_SCENE_RUNTIME_ON_APPLE_SILICON,
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
    companion_hud_text, companion_pace_fraction, daily_fraction_for_gauge,
    daily_overage_marker_fraction, mood_aura_radius, perimeter_gauge_colors,
    perimeter_gauge_layout, prepare_hud_layout, prepared_perimeter_gauge_arcs,
    tank_background_sample, tank_core_color, CompanionHudText, GaugeFractions, HudLineMetrics,
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
    NSEventModifierFlags, NSFloatingWindowLevel, NSFont, NSFontAttributeName, NSFontWeightBold,
    NSForegroundColorAttributeName, NSGradient, NSGraphicsContext, NSImage, NSLineCapStyle, NSMenu,
    NSMenuItem, NSRoundLineCapStyle, NSView, NSWindow, NSWindowCollectionBehavior,
    NSWindowOcclusionState, NSWindowStyleMask, NSWindowTitleVisibility,
};
// Used only by the retained-gated paired Smooth capture, which normalizes its
// output to a faithful sRGB color space.
#[cfg(feature = "retained-renderer")]
use objc2_app_kit::{NSColorRenderingIntent, NSColorSpace};
use objc2_foundation::{
    MainThreadMarker, NSAttributedString, NSMutableAttributedString, NSPoint, NSRect, NSSize,
    NSString, NSTimer,
};

// The pace gauge reads a ten-minute window and the pet's vitals move on hour
// scales, so ten-second polls bought nothing but CPU: every cycle re-runs the
// helper over the whole current day's transcripts.
const POLL_INTERVAL: Duration = Duration::from_secs(30);
const UI_TICK_INTERVAL_SECS: f64 = 0.25;
const ANIMATED_SCENE_TICK_INTERVAL_SECS: f64 = 1.0 / 15.0;
const FAST_ANIMATION_TICK_INTERVAL_SECS: f64 = 1.0 / 30.0;

#[cfg(feature = "retained-renderer")]
fn companion_tick_interval(
    renderer: EffectiveCompanionRenderer,
    scene_runtime_rollout: SceneRuntimeRollout,
) -> f64 {
    if renderer.is_pixel()
        || (renderer.is_retained() && scene_runtime_rollout == SceneRuntimeRollout::Live)
    {
        FAST_ANIMATION_TICK_INTERVAL_SECS
    } else if renderer.uses_smooth_scene() {
        ANIMATED_SCENE_TICK_INTERVAL_SECS
    } else {
        UI_TICK_INTERVAL_SECS
    }
}

#[cfg(not(feature = "retained-renderer"))]
fn companion_tick_interval(renderer: EffectiveCompanionRenderer) -> f64 {
    if renderer.is_pixel() {
        FAST_ANIMATION_TICK_INTERVAL_SECS
    } else if renderer.uses_smooth_scene() {
        ANIMATED_SCENE_TICK_INTERVAL_SECS
    } else {
        UI_TICK_INTERVAL_SECS
    }
}
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
pub(super) struct PreparedCompanionFrame {
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
        plan: Box<SmoothCompanionScenePlan>,
        /// Stable painter order computed and validated off the draw callback.
        /// Layer z values are not insertion ordered (the projected floor is
        /// deliberately moved beneath props), so the native painter used to
        /// allocate and sort this list again on every repaint.
        draw_order: Vec<usize>,
    },
}

#[allow(dead_code)] // Consumed by the staged draw preparation in Tasks 4 and 5.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreparedGaugeFrame {
    pub(super) xp_fraction: f64,
    pub(super) daily_fraction: f64,
    pub(super) daily_overage_fraction: f64,
    pub(super) pace_fraction: f64,
}

/// Minimal read-only accessors used by [`crate::companion::paired_review`] to
/// freeze a prepared frame into a checksummable review identity. Only the
/// already-projected chrome and geometry are exposed; no `AppState` or
/// `WatchViewModel` state is reachable through these.
#[cfg(feature = "retained-renderer")]
impl PreparedCompanionFrame {
    pub(super) fn review_aperture(&self) -> RoundAperture {
        self.aperture
    }

    pub(super) fn review_background(&self) -> RoundColor {
        self.background
    }

    pub(super) fn review_mood_aura(&self) -> RoundColor {
        self.mood_aura_color
    }

    pub(super) fn review_dim_overlay(&self) -> bool {
        self.dim_overlay
    }

    pub(super) fn review_gauges(&self) -> PreparedGaugeFrame {
        self.gauges
    }

    pub(super) fn review_hud(&self) -> &CompanionHudText {
        &self.hud
    }

    pub(super) fn review_hud_font_size(&self) -> f64 {
        self.hud_font_size
    }

    pub(super) fn review_overlays(&self) -> &[RoundDrawCommand] {
        &self.overlay_commands
    }

    /// Hands out a borrowed view of the renderer payload so paired_review can
    /// project the renderer identity without app.rs depending on its serde
    /// projection types.
    pub(super) fn renderer_source(
        &self,
    ) -> crate::companion::paired_review::RendererIdentitySource<'_> {
        use crate::companion::paired_review::RendererIdentitySource;
        match &self.renderer {
            PreparedRendererFrame::Pixel { frame } => RendererIdentitySource::Pixel { frame },
            PreparedRendererFrame::Classic {
                metrics,
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                draw_list,
            } => RendererIdentitySource::Classic {
                metrics: *metrics,
                pet_center_col: *pet_center_col,
                pet_center_row: *pet_center_row,
                pet_width_cells: *pet_width_cells,
                draw_list,
            },
            PreparedRendererFrame::Smooth {
                metrics,
                pet_center_col,
                pet_center_row,
                pet_width_cells,
                plan,
                draw_order,
            } => RendererIdentitySource::Smooth {
                metrics: *metrics,
                pet_center_col: *pet_center_col,
                pet_center_row: *pet_center_row,
                pet_width_cells: *pet_width_cells,
                plan,
                draw_order,
            },
        }
    }

    /// A deterministic prepared frame for review fixtures and tests. Built
    /// entirely from constants — never from live pet state.
    pub(super) fn fixture() -> Self {
        PreparedCompanionFrame {
            bounds: PreparedBounds {
                width_px: 360,
                height_px: 360,
                width_f64: 360.0,
                height_f64: 360.0,
            },
            aperture: RoundAperture::new(360, 360),
            background: RoundColor(0.05, 0.06, 0.10, 1.0),
            mood_aura_color: RoundColor(0.30, 0.40, 0.55, 0.80),
            dim_overlay: false,
            renderer: PreparedRendererFrame::Pixel {
                frame: PixelFrame::transparent(PixelViewport::companion_default()),
            },
            gauges: PreparedGaugeFrame {
                xp_fraction: 0.5,
                daily_fraction: 0.5,
                daily_overage_fraction: 0.0,
                pace_fraction: 0.5,
            },
            hud: CompanionHudText {
                today_total: "—".to_string(),
                daily_percent: "—".to_string(),
                pace: "—".to_string(),
            },
            hud_font_size: 8.5,
            overlay_commands: Vec::new(),
            review_sample: None,
        }
    }
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
        crate::round::hud::review_capture_hud_text()
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
    renderer_mode: EffectiveCompanionRenderer,
    review_depth: Option<CompanionReviewDepth>,
    pixel_frame: Option<&PixelFrame>,
    smooth_started_at: Option<Instant>,
    smooth_semantic_art_tick_index: u64,
    redacts_live_hud: bool,
    force_dim_overlay: bool,
    bounds: NSRect,
    metric_cache: &mut CompanionMetricCache,
) -> std::result::Result<PreparedCompanionFrame, CompanionFramePreparationError> {
    let now = time::OffsetDateTime::now_utc();
    let elapsed_ms = smooth_started_at
        .map(|started_at| started_at.elapsed().as_millis())
        .unwrap_or(0)
        .min(u128::from(u64::MAX)) as u64;
    prepare_companion_frame_at(
        vm,
        scene,
        renderer_mode,
        review_depth.map(CompanionReviewDepth::normalized),
        pixel_frame,
        smooth_semantic_art_tick_index,
        redacts_live_hud,
        force_dim_overlay,
        bounds,
        metric_cache,
        now,
        elapsed_ms,
    )
}

#[allow(clippy::too_many_arguments)] // Injected clock makes review/lifetime evidence deterministic.
fn prepare_companion_frame_at(
    vm: &WatchViewModel,
    scene: &RoundSceneModel,
    renderer_mode: EffectiveCompanionRenderer,
    depth_override: Option<f32>,
    pixel_frame: Option<&PixelFrame>,
    smooth_semantic_art_tick_index: u64,
    redacts_live_hud: bool,
    force_dim_overlay: bool,
    bounds: NSRect,
    metric_cache: &mut CompanionMetricCache,
    now: time::OffsetDateTime,
    elapsed_ms: u64,
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
    // `--dimmed` forces the resting dim composition onto the live frame for the
    // final review matrix, on top of any lifecycle-driven dim.
    let dim_overlay = scene.lifecycle.asleep || scene.lifecycle.calm || force_dim_overlay;
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
        if renderer_mode.uses_smooth_scene() {
            // Normal runs pass None here and keep their roam-driven depth.
            let plan = crate::round::smooth::try_build_round_smooth_scene_plan_with_options(
                vm,
                now,
                metrics.grid_cols,
                metrics.grid_rows,
                &companion_motion(),
                elapsed_ms,
                crate::round::smooth::SmoothSceneBuildOptions { depth_override },
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
            let draw_order = smooth_layer_draw_order(&plan);
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
                plan: Box::new(plan),
                draw_order,
            }
        } else {
            let companion_scene = crate::round::scene::build_round_scene_draw_list(
                vm,
                now,
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

#[cfg(feature = "retained-renderer")]
pub(super) fn prepare_capacity_fixture_frame(
    vm: &WatchViewModel,
    depth: f32,
    dimmed: bool,
    now: time::OffsetDateTime,
) -> std::result::Result<PreparedCompanionFrame, &'static str> {
    let scene = derive_round_scene_model(vm, now);
    let mut metric_cache = CompanionMetricCache::default();
    prepare_companion_frame_at(
        vm,
        &scene,
        EffectiveCompanionRenderer::Retained,
        Some(depth),
        None,
        0,
        true,
        dimmed,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 360.0)),
        &mut metric_cache,
        now,
        0,
    )
    .map_err(CompanionFramePreparationError::category)
}

#[cfg(feature = "retained-renderer")]
fn prepare_lifetime_fixture_frame(
    species: crate::pet::generation::Species,
    frame: u64,
    now: time::OffsetDateTime,
) -> std::result::Result<
    (PreparedCompanionFrame, u64),
    crate::companion::retained::RetainedFailureCategory,
> {
    use crate::commands::companion_mode::CompanionReviewState;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;

    let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(120, now.date());
    vm.pet_render.generated_species = species;
    let stages = [Stage::S2, Stage::S3, Stage::S4, Stage::S5];
    let stage = stages[((frame / 600) as usize) % stages.len()];
    vm.pet_render.stage = stage;
    if (frame / 300) % 2 == 1 {
        vm.habitat.earned_props.pop();
    }
    let state_index = ((frame / 900) % 5) as u8;
    let (review_state, dimmed) = match state_index {
        0 => (CompanionReviewState::Normal, false),
        1 => (CompanionReviewState::ActivePulse, false),
        2 => (CompanionReviewState::AsleepCalm, false),
        3 => (CompanionReviewState::HelperTrouble, false),
        _ => (CompanionReviewState::Normal, true),
    };
    let mut presentation_state = WatchPresentationState::default();
    apply_review_state(review_state, &mut presentation_state, &mut vm, now).map_err(|_| {
        crate::companion::retained::RetainedFailureCategory::LifetimeFramePreparation
    })?;
    let asleep = review_state == CompanionReviewState::AsleepCalm;
    rerender_pet_for_view_model(&mut vm, frame, asleep, now).map_err(|_| {
        crate::companion::retained::RetainedFailureCategory::LifetimeFramePreparation
    })?;
    if review_state == CompanionReviewState::ActivePulse {
        vm.pet_render.mood = Mood::Ecstatic;
    }
    let scene = derive_round_scene_model(&vm, now);
    let mut metric_cache = CompanionMetricCache::default();
    let base = time::macros::datetime!(2026-06-13 18:00 UTC);
    let elapsed_ms = (now - base)
        .whole_milliseconds()
        .max(0)
        .min(i128::from(u64::MAX)) as u64;
    let prepared = prepare_companion_frame_at(
        &vm,
        &scene,
        EffectiveCompanionRenderer::Retained,
        None,
        None,
        frame,
        true,
        dimmed,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 360.0)),
        &mut metric_cache,
        now,
        elapsed_ms,
    )
    .map_err(|_| crate::companion::retained::RetainedFailureCategory::LifetimeFramePreparation)?;
    let semantic_hash =
        crate::presentation::smooth::pet_visual_checksum(&vm.pet_art, &vm.pet_spans)
            ^ ((stage.index() as u64) << 56)
            ^ (u64::from(state_index) << 48)
            ^ ((vm.habitat.earned_props.len() as u64) << 32)
            ^ (vm.habitat.earned_inhabitants.len() as u64);
    Ok((prepared, semantic_hash))
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
    renderer_runtime: RendererRuntimeState,
    #[cfg(feature = "retained-renderer")]
    retained_host: Option<crate::companion::retained::ActiveRetainedHost>,
    #[cfg(feature = "retained-renderer")]
    scene_runtime_rollout: SceneRuntimeRollout,
    #[cfg(feature = "retained-renderer")]
    scene_runtime_hidden: bool,
    #[cfg(feature = "retained-renderer")]
    cold_smooth_fallback: ColdSmoothFallbackGate,
    pixel_input: Option<PixelPetInput>,
    pixel_state: Option<PixelRendererState>,
    pixel_frame: Option<PixelFrame>,
    smooth_started_at: Option<Instant>,
    smooth_semantic_clock: Option<crate::companion::smooth_timing::SmoothSemanticClock>,
    smooth_semantic_art_tick_index: u64,
    animation_frame: u64,
    review_capture: Option<crate::companion::review_capture::ReviewCapture>,
    redacts_live_hud: bool,
    /// Forces the resting dim composition onto the live frame (`--dimmed`).
    force_dim_overlay: bool,
    /// Dev/test-only bounded fault to inject into the retained capture path.
    #[cfg(all(feature = "retained-renderer", feature = "dev-preview"))]
    retained_fault_injection: Option<crate::commands::companion_mode::RetainedFaultInjection>,
    /// The opt-in `--review-capture-live-values` flag, threaded to the paired
    /// capture's privacy mode.
    #[cfg_attr(not(feature = "retained-renderer"), allow(dead_code))]
    review_capture_live_values: bool,
    #[cfg(feature = "retained-renderer")]
    runtime_metrics_out: Option<std::path::PathBuf>,
    #[cfg(feature = "retained-renderer")]
    runtime_baseline_visibility: RuntimeBaselineVisibilityPhase,
    #[cfg(feature = "retained-renderer")]
    terminal_runtime_metrics: Option<crate::companion::retained::CompanionRuntimeMetricsSnapshot>,
    metric_cache: CompanionMetricCache,
    last_good_frame: Option<PreparedCompanionFrame>,
    #[allow(dead_code)] // Read by the Task 5 paint boundary.
    last_frame_preparation_error: Option<CompanionFramePreparationError>,
    #[allow(dead_code)] // Updated by the Task 5 callback guard.
    callback_panic_count: u64,
    #[allow(dead_code)] // Updated by the Task 5 callback guard.
    last_callback_panic_label: Option<&'static str>,
}

#[cfg(feature = "retained-renderer")]
struct PreparedSceneRuntimeTick {
    snapshot: std::sync::Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
    hud: CompanionHudText,
    hud_font_size: f64,
}

#[cfg(feature = "retained-renderer")]
#[derive(Debug, Default)]
struct ColdSmoothFallbackGate {
    prepared: bool,
}

#[cfg(feature = "retained-renderer")]
impl ColdSmoothFallbackGate {
    fn take_prepare_request(&mut self) -> bool {
        if self.prepared {
            false
        } else {
            self.prepared = true;
            true
        }
    }
}

#[cfg(feature = "retained-renderer")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeBaselineVisibilityPhase {
    Inactive,
    Visible,
    HiddenTransition,
    HiddenSteady { completed: u8 },
    Complete,
}

#[cfg(feature = "retained-renderer")]
impl RuntimeBaselineVisibilityPhase {
    fn begin_hidden_segment(&mut self) -> bool {
        if *self != Self::Visible {
            return false;
        }
        *self = Self::HiddenTransition;
        true
    }

    fn forces_hidden(self) -> bool {
        matches!(self, Self::HiddenTransition | Self::HiddenSteady { .. })
    }

    fn record_hidden_ui_tick(&mut self) {
        *self = match *self {
            Self::HiddenTransition => Self::HiddenSteady { completed: 0 },
            Self::HiddenSteady { completed: 0 } => Self::HiddenSteady { completed: 1 },
            Self::HiddenSteady { .. } => Self::Complete,
            other => other,
        };
    }

    fn ready_for_terminal_work(self) -> bool {
        matches!(self, Self::Inactive | Self::Complete)
    }
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

pub fn run(request: CompanionRendererRequest, review: CompanionReviewOptions) -> Result<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| GlorpError::Message("glorp companion must run on the main thread".into()))?;
    let retained_compiled = cfg!(all(target_os = "macos", feature = "retained-renderer"));
    let effective = resolve_renderer(
        request,
        CompanionRendererTarget::current(),
        retained_compiled,
        AUTO_RETAINED_ON_APPLE_SILICON,
    )
    .map_err(|error| {
        GlorpError::Message(format!(
            "companion renderer unavailable ({})",
            error.category()
        ))
    })?;
    #[cfg(feature = "retained-renderer")]
    let mut renderer_runtime = RendererRuntimeState::new(request, effective);
    #[cfg(not(feature = "retained-renderer"))]
    let renderer_runtime = RendererRuntimeState::new(request, effective);
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
    let mut initial_vm = if renderer_runtime.effective().is_pixel()
        || renderer_runtime.effective().uses_smooth_scene()
    {
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
    #[cfg(feature = "retained-renderer")]
    let scene_runtime_rollout = if renderer_runtime.effective().is_retained() {
        match review.retained_scene_runtime {
            Some(SceneRuntimeRollout::Off) => SceneRuntimeRollout::Off,
            Some(SceneRuntimeRollout::Shadow) => resolve_scene_rollout(true, false),
            Some(SceneRuntimeRollout::Live) => resolve_scene_rollout(true, true),
            None => resolve_scene_rollout(
                request == CompanionRendererRequest::Retained,
                AUTO_SCENE_RUNTIME_ON_APPLE_SILICON,
            ),
        }
    } else {
        SceneRuntimeRollout::Off
    };
    apply_review_state(review_state, &mut presentation_state, &mut initial_vm, now)?;
    if renderer_runtime.effective().uses_smooth_scene() {
        prepare_smooth_view_model_for_tick(&mut initial_vm, 0, now)?;
    }
    let scene = derive_round_scene_model(&initial_vm, now);
    let pixel_input = renderer_runtime
        .effective()
        .is_pixel()
        .then(|| PixelPetInput::from_watch_view_model(&initial_vm, now));
    let pixel_state = pixel_input
        .as_ref()
        .map(|input| PixelRendererState::new(input, now));
    let pixel_frame = None;
    let smooth_started_at = renderer_runtime
        .effective()
        .uses_smooth_scene()
        .then(Instant::now);
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
    let (window, view) = build_window(mtm, review.initial_size);
    if review.runtime_metrics_out.is_some() {
        #[allow(deprecated)] // Required for deterministic bounded AppKit review automation.
        app.activateIgnoringOtherApps(true);
        // A baseline that is fully covered by the controlling Codex window gets
        // no CAMetalLayer drawable and can only report skipped frames. Keep this
        // bounded review window above ordinary app windows so the 120-second
        // visible phase measures actual acquire/encode/submit/present work.
        window.setLevel(NSFloatingWindowLevel);
        unsafe { window.orderFrontRegardless() };
        window.makeKeyAndOrderFront(None);
    }
    // Prepare all fallible GPU work on a detached layer, then activate (install
    // the layer on the view) only on success. A failure in either phase falls the
    // effective renderer back to Smooth and leaves the view with no residual
    // retained layer, so the review capture below reads the true post-fallback
    // renderer.
    #[cfg(feature = "retained-renderer")]
    let mut retained_host = if renderer_runtime.effective().is_retained() {
        // Dev/test-only: an injected initialization fault forces the startup path
        // down the acknowledged Smooth fallback without ever building a host.
        #[cfg(feature = "dev-preview")]
        let injected_init_fault = review.retained_fault_injection.and_then(
            crate::commands::companion_mode::RetainedFaultInjection::initialization_category,
        );
        #[cfg(not(feature = "dev-preview"))]
        let injected_init_fault: Option<
            crate::companion::retained::RetainedFailureCategory,
        > = None;

        if let Some(category) = injected_init_fault {
            write_boundary_diagnostic(format_args!(
                "glorp retained renderer initialization fault injected: {}\n",
                category.category()
            ));
            renderer_runtime.request_fallback(category);
            None
        } else {
            let mailbox = crate::companion::retained::GpuErrorMailbox::new();
            let activation =
                crate::companion::retained::PreparedRetainedHost::prepare(view.as_super(), mailbox)
                    .and_then(|prepared| prepared.activate(view.as_super()));
            match activation {
                Ok(host) => {
                    // Dev/test-only: an injected mid-run device fault is queued on
                    // the host's own error mailbox so the first present drains it
                    // exactly as it would a real asynchronous device fault.
                    #[cfg(feature = "dev-preview")]
                    if let Some(category) = review.retained_fault_injection.and_then(
                        crate::commands::companion_mode::RetainedFaultInjection::device_fault_category,
                    ) {
                        host.inject_gpu_fault(category);
                    }
                    Some(host)
                }
                Err(error) => {
                    write_boundary_diagnostic(format_args!(
                        "glorp retained renderer initialization failed: {}\n",
                        error.category()
                    ));
                    renderer_runtime.request_fallback(error);
                    None
                }
            }
        }
    } else {
        None
    };
    #[cfg(feature = "retained-renderer")]
    if review.runtime_metrics_out.is_some() {
        if let Some(host) = retained_host.as_mut() {
            host.prewarm_capture_resources();
        }
    }
    let review_capture = crate::companion::review_capture::ReviewCapture::from_options(
        renderer_runtime.effective(),
        &review,
    )?;
    let redacts_live_hud = review_capture
        .as_ref()
        .is_some_and(|capture| capture.redacts_live_hud())
        || review.runtime_metrics_out.is_some();
    let poll_rx = if review.runtime_metrics_out.is_some() {
        // The hidden Stage-0 baseline consumes only the initialized fixture.
        // Live usage polling would make the measured scene and wakeup source
        // depend on the developer's machine state.
        let (_fixed_source, receiver) = mpsc::channel::<LiveWatchUpdate>();
        receiver
    } else {
        crate::watch_live::spawn_live_watch_worker(
            paths,
            POLL_INTERVAL,
            "glorp-companion-poll",
            if renderer_runtime.effective().is_pixel()
                || renderer_runtime.effective().uses_smooth_scene()
            {
                LiveWatchRenderMode::Semantic
            } else {
                LiveWatchRenderMode::Rendered
            },
        )
    };

    // Smooth is CPU-drawn and stays at fifteen frames. Direct retained Live
    // updates only bounded scene deltas on the GPU, so it can present the same
    // fractional drift and bob at thirty frames without speeding up the separate
    // four-hertz semantic-art clock. Computed before the runtime state moves into
    // AppState below.
    #[cfg(feature = "retained-renderer")]
    let tick_interval =
        companion_tick_interval(renderer_runtime.effective(), scene_runtime_rollout);
    #[cfg(not(feature = "retained-renderer"))]
    let tick_interval = companion_tick_interval(renderer_runtime.effective());

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
            renderer_runtime,
            #[cfg(feature = "retained-renderer")]
            retained_host,
            #[cfg(feature = "retained-renderer")]
            scene_runtime_rollout,
            #[cfg(feature = "retained-renderer")]
            scene_runtime_hidden: false,
            #[cfg(feature = "retained-renderer")]
            cold_smooth_fallback: ColdSmoothFallbackGate::default(),
            pixel_input,
            pixel_state,
            pixel_frame,
            smooth_started_at,
            smooth_semantic_clock,
            smooth_semantic_art_tick_index: 0,
            animation_frame: 0,
            review_capture,
            redacts_live_hud,
            force_dim_overlay: review.force_dim_overlay,
            #[cfg(all(feature = "retained-renderer", feature = "dev-preview"))]
            retained_fault_injection: review.retained_fault_injection,
            review_capture_live_values: review.review_capture_live_values,
            #[cfg(feature = "retained-renderer")]
            runtime_metrics_out: review.runtime_metrics_out.clone(),
            #[cfg(feature = "retained-renderer")]
            runtime_baseline_visibility: if review.runtime_metrics_out.is_some() {
                RuntimeBaselineVisibilityPhase::Visible
            } else {
                RuntimeBaselineVisibilityPhase::Inactive
            },
            #[cfg(feature = "retained-renderer")]
            terminal_runtime_metrics: None,
            metric_cache: CompanionMetricCache::default(),
            last_good_frame: None,
            last_frame_preparation_error: None,
            callback_panic_count: 0,
            last_callback_panic_label: None,
        });
    });

    #[cfg(feature = "retained-renderer")]
    let needs_initial_smooth_frame = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|state| state.retained_host.is_none())
    });
    #[cfg(feature = "retained-renderer")]
    if needs_initial_smooth_frame {
        prepare_current_frame_from_state();
    }
    #[cfg(not(feature = "retained-renderer"))]
    prepare_current_frame_from_state();

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
    // A paired-capture fault after NSApplication exits must fail the process so a
    // successful `open`/spawn cannot hide an app-side capture failure.
    #[cfg(feature = "retained-renderer")]
    if let Some(Err(err)) = take_pair_capture_outcome() {
        return Err(err);
    }
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
    #[cfg(feature = "retained-renderer")]
    let started_at = Instant::now();
    #[cfg(feature = "retained-renderer")]
    let hidden_work_start = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|state| state.retained_host.as_ref())
            .map(crate::companion::retained::ActiveRetainedHost::runtime_work_counters)
    });
    let _mtm = MainThreadMarker::new().expect("companion ui_tick on non-main thread");
    drain_poll_results();
    // AppKit does not need fresh backing-store contents for a window that cannot
    // be seen. Pausing the CPU renderer here preserves the time-based motion: on
    // reveal the next tick samples the current drift/depth/bob position instead
    // of replaying hidden frames.
    if !companion_view_is_visible() {
        #[cfg(feature = "retained-renderer")]
        match prepare_scene_runtime_tick() {
            Ok(Some(tick)) => {
                if let Err(error) = coalesce_hidden_scene_snapshot(tick.snapshot) {
                    handle_scene_runtime_failure(error);
                }
            }
            Ok(None) => {}
            Err(error) => handle_scene_runtime_failure(error),
        }
        #[cfg(feature = "retained-renderer")]
        APP_STATE.with(|cell| {
            if let Some(state) = cell.borrow_mut().as_mut() {
                let identity = crate::round::smooth::CompanionContentIdentity::for_pet(
                    state.vm.pet_render.generated_species,
                );
                let backing_scale = crate::companion::retained::ActiveRetainedHost::backing_scale_for_resource_preparation(
                    state.view.as_super(),
                )
                .ok();
                if let Some(host) = state.retained_host.as_mut() {
                    let backing_scale = backing_scale.unwrap_or_else(|| host.backing_scale());
                    host.suspend_resource_preparation(&identity, backing_scale);
                    host.record_hidden_tick(hidden_work_start.unwrap_or_default());
                }
                state.runtime_baseline_visibility.record_hidden_ui_tick();
            }
        });
        finish_review_capture_if_due();
        return;
    }
    #[cfg(feature = "retained-renderer")]
    APP_STATE.with(|cell| {
        if let Some(host) = cell
            .borrow_mut()
            .as_mut()
            .and_then(|state| state.retained_host.as_mut())
        {
            host.begin_visible_tick();
        }
    });
    #[cfg(feature = "retained-renderer")]
    if APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|state| state.scene_runtime_rollout == SceneRuntimeRollout::Live)
    }) {
        animate_pet();
        let result = prepare_scene_runtime_tick().and_then(|tick| {
            let Some(tick) = tick else {
                return Ok(());
            };
            let was_hidden = APP_STATE.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .is_some_and(|state| state.scene_runtime_hidden)
            });
            let active_delta_pending = APP_STATE.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .and_then(|state| state.retained_host.as_ref())
                    .is_some_and(|host| host.scene_active_delta_pending())
            });
            if was_hidden {
                reveal_scene_runtime(std::sync::Arc::clone(&tick.snapshot))?;
            } else if !active_delta_pending {
                reconcile_scene_runtime(std::sync::Arc::clone(&tick.snapshot))?;
            }
            service_scene_runtime(&tick)
        });
        if let Err(error) = result {
            handle_scene_runtime_failure(error);
        }
        drive_smooth_fallback_paint();
        finish_review_capture_if_due();
        record_retained_ui_tick(started_at);
        return;
    }
    #[cfg(feature = "retained-renderer")]
    if APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_some_and(|state| state.scene_runtime_rollout == SceneRuntimeRollout::Shadow)
    }) {
        let result = prepare_scene_runtime_tick().and_then(|tick| {
            let Some(tick) = tick else {
                return Ok(());
            };
            let legacy_generation_ready = APP_STATE.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .and_then(|state| state.retained_host.as_ref())
                    .is_some_and(|host| host.current_resource_generation() != 0)
            });
            if !legacy_generation_ready {
                return Ok(());
            }
            let was_hidden = APP_STATE.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .is_some_and(|state| state.scene_runtime_hidden)
            });
            if was_hidden {
                reveal_scene_runtime(std::sync::Arc::clone(&tick.snapshot))?;
            } else {
                reconcile_scene_runtime(std::sync::Arc::clone(&tick.snapshot))?;
            }
            service_scene_runtime(&tick)
        });
        if let Err(error) = result {
            handle_scene_runtime_failure(error);
        }
    }
    #[cfg(feature = "retained-renderer")]
    let presented_active_generation = present_retained_active_generation();
    #[cfg(feature = "retained-renderer")]
    match drive_retained_resource_preparation() {
        crate::companion::retained::ResourcePreparationTick::Ready
            if !presented_active_generation => {}
        crate::companion::retained::ResourcePreparationTick::Ready
        | crate::companion::retained::ResourcePreparationTick::YieldedRetainingActive
        | crate::companion::retained::ResourcePreparationTick::YieldedWithoutActive
        | crate::companion::retained::ResourcePreparationTick::FailedRetainingActive(_) => {
            record_retained_ui_tick(started_at);
            return;
        }
        crate::companion::retained::ResourcePreparationTick::FailedWithoutActive(category) => {
            fallback_from_retained(category);
        }
    }
    animate_pet();
    prepare_current_frame_from_state();
    drive_smooth_fallback_paint();
    finish_review_capture_if_due();
    #[cfg(feature = "retained-renderer")]
    record_retained_ui_tick(started_at);
}

#[cfg(feature = "retained-renderer")]
fn record_retained_ui_tick(started_at: Instant) {
    APP_STATE.with(|cell| {
        if let Some(host) = cell
            .borrow_mut()
            .as_mut()
            .and_then(|state| state.retained_host.as_mut())
        {
            host.record_ui_tick_us(crate::companion::retained::duration_us(
                started_at.elapsed(),
            ));
        }
    });
}

#[cfg(feature = "retained-renderer")]
fn drive_retained_resource_preparation() -> crate::companion::retained::ResourcePreparationTick {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return crate::companion::retained::ResourcePreparationTick::Ready;
        };
        let identity = crate::round::smooth::CompanionContentIdentity::for_pet(
            state.vm.pet_render.generated_species,
        );
        let AppState { view, retained_host, .. } = state;
        let Some(host) = retained_host.as_mut() else {
            return crate::companion::retained::ResourcePreparationTick::Ready;
        };
        let desired_backing_scale = match crate::companion::retained::ActiveRetainedHost::backing_scale_for_resource_preparation(view.as_super()) {
            Ok(scale) => scale,
            Err(category) => {
                return crate::companion::retained::ResourcePreparationTick::FailedWithoutActive(
                    category,
                );
            }
        };
        let outcome = host.advance_resource_preparation(&identity, desired_backing_scale);
        if outcome
            == crate::companion::retained::ResourcePreparationTick::YieldedWithoutActive
        {
            let _ = host.record_resource_preparation_skip();
        }
        outcome
    })
}

#[cfg(feature = "retained-renderer")]
fn present_retained_active_generation() -> bool {
    let identity = APP_STATE.with(|cell| {
        let state = cell.borrow();
        let state = state.as_ref()?;
        state.last_good_frame.as_ref()?;
        let desired_identity = crate::round::smooth::CompanionContentIdentity::for_pet(
            state.vm.pet_render.generated_species,
        );
        let host = state.retained_host.as_ref()?;
        let desired_backing_scale =
            crate::companion::retained::ActiveRetainedHost::backing_scale_for_resource_preparation(
                state.view.as_super(),
            )
            .ok()?;
        host.active_identity_for_resource_preparation(&desired_identity, desired_backing_scale)
    });
    let Some(identity) = identity else {
        return false;
    };
    present_retained_frame_with(RetainedPresentIdentity::ActiveGeneration(identity));
    true
}

/// After a runtime fallback tears down the retained host, the reverted
/// layer-hosting view does not resume automatic `drawRect:` on `setNeedsDisplay`
/// within the timer callback. Force a synchronous display each tick so the Smooth
/// paint runs, `draw_scene` records a frame, `acknowledge_smooth_paint` promotes
/// the disposition to `FallbackPainted`, and a bounded review reaches its capture
/// budget and terminates — the same render/record cadence a Smooth-from-start run
/// gets for free.
#[cfg(feature = "retained-renderer")]
fn drive_smooth_fallback_paint() {
    use crate::companion::retained::FrameDisposition;

    let view = APP_STATE.with(|cell| {
        let state = cell.borrow();
        let state = state.as_ref()?;
        match state.renderer_runtime.disposition() {
            FrameDisposition::FallbackPending(_) | FrameDisposition::FallbackPainted(_) => {
                Some(state.view.clone())
            }
            _ => None,
        }
    });
    if let Some(view) = view {
        unsafe {
            view.setNeedsDisplay(true);
            view.displayIfNeeded();
        }
    }
}

#[cfg(not(feature = "retained-renderer"))]
fn drive_smooth_fallback_paint() {}

fn companion_view_is_visible() -> bool {
    APP_STATE.with(|cell| {
        let state = cell.borrow();
        let Some(state) = state.as_ref() else {
            return false;
        };
        #[cfg(feature = "retained-renderer")]
        if state.runtime_baseline_visibility.forces_hidden() {
            return false;
        }
        // Review runs are bounded automation, not an idle background window.
        // They must keep painting even when another app covers the companion or
        // the capture can never reach MIN_CAPTURE_FRAMES and terminate.
        if state.review_capture.is_some() {
            return true;
        }
        let Some(window) = state.view.window() else {
            return false;
        };
        window.isVisible()
            && !window.isMiniaturized()
            && window
                .occlusionState()
                .contains(NSWindowOcclusionState::Visible)
    })
}

#[cfg(feature = "retained-renderer")]
fn coalesce_hidden_scene_snapshot(
    snapshot: std::sync::Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
) -> std::result::Result<(), crate::companion::retained::RetainedFailureCategory> {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return Ok(());
        };
        if state.scene_runtime_rollout == SceneRuntimeRollout::Off {
            return Ok(());
        }
        let Some(host) = state.retained_host.as_mut() else {
            return Ok(());
        };
        if !host.has_scene_runtime() {
            return Ok(());
        }
        if !state.scene_runtime_hidden {
            host.hide_scene_runtime()?;
            state.scene_runtime_hidden = true;
        }
        host.coalesce_hidden_scene_snapshot(snapshot)
    })
}

#[cfg(feature = "retained-renderer")]
fn reveal_scene_runtime(
    snapshot: std::sync::Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
) -> std::result::Result<(), crate::companion::retained::RetainedFailureCategory> {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return Ok(());
        };
        let Some(host) = state.retained_host.as_mut() else {
            return Ok(());
        };
        let revealed = if host.has_scene_runtime() && state.scene_runtime_hidden {
            host.reveal_scene_runtime(snapshot)?
        } else {
            host.reconcile_scene_snapshot(snapshot)?;
            true
        };
        state.scene_runtime_hidden = !revealed;
        Ok(())
    })
}

#[cfg(feature = "retained-renderer")]
fn prepare_scene_runtime_tick() -> std::result::Result<
    Option<PreparedSceneRuntimeTick>,
    crate::companion::retained::RetainedFailureCategory,
> {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return Ok(None);
        };
        if state.scene_runtime_rollout == SceneRuntimeRollout::Off || state.retained_host.is_none()
        {
            return Ok(None);
        }
        let bounds = prepare_bounds(state.view.bounds()).map_err(|_| {
            crate::companion::retained::RetainedFailureCategory::SceneCandidateEncode
        })?;
        let metrics = state.metric_cache.metrics_for(bounds).map_err(|_| {
            crate::companion::retained::RetainedFailureCategory::SceneCandidateEncode
        })?;
        let now = time::OffsetDateTime::now_utc();
        let elapsed_ms = state
            .smooth_started_at
            .map(|started_at| started_at.elapsed().as_millis())
            .unwrap_or(0)
            .min(u128::from(u64::MAX)) as u64;
        let mut input = crate::presentation::companion_scene::CompanionSceneProjectionInput::round(
            crate::presentation::companion_scene::CompanionProjectionClock::new(now, elapsed_ms),
            crate::presentation::companion_scene::CompanionLogicalLayout::round(
                bounds.width_f64 as f32,
                bounds.height_f64 as f32,
            ),
            metrics.grid_cols,
            metrics.grid_rows,
            crate::round::scene::current_round_motion_clearance(metrics.grid_rows),
        );
        if let Some(depth) = state.review_depth {
            input = input.with_depth_override(depth.normalized());
        }
        let mut snapshot =
            crate::presentation::companion_scene::CompanionSceneSnapshot::project_with_input(
                &state.vm, input,
            )
            .map_err(|_| {
                crate::companion::retained::RetainedFailureCategory::SceneCandidateEncode
            })?;
        if state.force_dim_overlay {
            snapshot.frame.dimmed = true;
            snapshot.frame.dim_amount = 0.35;
        }
        Ok(Some(PreparedSceneRuntimeTick {
            snapshot: std::sync::Arc::new(snapshot),
            hud: prepare_hud_frame(&state.vm, state.redacts_live_hud),
            hud_font_size: metrics.font_size,
        }))
    })
}

#[cfg(feature = "retained-renderer")]
fn service_scene_runtime(
    tick: &PreparedSceneRuntimeTick,
) -> std::result::Result<(), crate::companion::retained::RetainedFailureCategory> {
    let rollout = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|state| state.scene_runtime_rollout)
            .unwrap_or(SceneRuntimeRollout::Off)
    });
    match rollout {
        SceneRuntimeRollout::Off => Ok(()),
        SceneRuntimeRollout::Shadow => service_shadow_scene_runtime(),
        SceneRuntimeRollout::Live => service_live_scene_runtime(tick),
    }
}

#[cfg(feature = "retained-renderer")]
fn service_shadow_scene_runtime(
) -> std::result::Result<(), crate::companion::retained::RetainedFailureCategory> {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(host) = state
            .as_mut()
            .and_then(|state| state.retained_host.as_mut())
        else {
            return Ok(());
        };
        let _ = host.advance_scene_generation(false)?;
        Ok(())
    })
}

#[cfg(feature = "retained-renderer")]
fn service_live_scene_runtime(
    tick: &PreparedSceneRuntimeTick,
) -> std::result::Result<(), crate::companion::retained::RetainedFailureCategory> {
    use crate::companion::retained::{
        RetainedFailureCategory, SceneGenerationServiceTick, ScenePresentOutcome,
    };
    use crate::presentation::companion_scene::runtime::{ActivationTransition, RuntimeDisposition};

    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return Ok(());
        };
        let view = state.view.clone();
        let Some(host) = state.retained_host.as_mut() else {
            return Ok(());
        };
        let service = host.advance_scene_generation(true)?;
        let mut activation_presented = false;
        if service == SceneGenerationServiceTick::CandidateReady {
            let disposition = host
                .activate_candidate(&tick.hud, tick.hud_font_size)
                .map_err(|_| RetainedFailureCategory::SceneCandidateEncode)?;
            match disposition {
                RuntimeDisposition::Activation(ActivationTransition::Committed) => {
                    activation_presented = true;
                }
                RuntimeDisposition::Activation(ActivationTransition::HostFallbackPending) => {
                    return Err(RetainedFailureCategory::SceneCandidateEncode);
                }
                RuntimeDisposition::Activation(
                    ActivationTransition::RetryLater
                    | ActivationTransition::CandidateDestroyedRetainingActive
                    | ActivationTransition::DroppedStale,
                )
                | RuntimeDisposition::DroppedStale => {}
                _ => {}
            }
        } else if service == SceneGenerationServiceTick::Failed
            && !host.scene_has_active_generation()
        {
            return Err(RetainedFailureCategory::SceneCandidateEncode);
        }

        if !activation_presented && host.scene_has_active_generation() {
            match host.present_active_scene(view.as_super(), &tick.hud, tick.hud_font_size)? {
                ScenePresentOutcome::Presented(_version) => {
                    if let Some(capture) = state.review_capture.as_mut() {
                        capture.record_frame(None);
                    }
                }
                ScenePresentOutcome::Skipped => {
                    if let Some(capture) = state.review_capture.as_mut() {
                        capture.record_offscreen_review_tick();
                    }
                }
            }
        }
        Ok(())
    })
}

#[cfg(feature = "retained-renderer")]
fn prepare_cold_smooth_fallback_once() {
    let should_prepare = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return false;
        };
        if !state.cold_smooth_fallback.take_prepare_request() {
            return false;
        }
        let now = time::OffsetDateTime::now_utc();
        let _ = prepare_smooth_view_model_for_tick(
            &mut state.vm,
            state.smooth_semantic_art_tick_index,
            now,
        );
        state.scene = derive_round_scene_model(&state.vm, now);
        true
    });
    if should_prepare {
        prepare_current_frame_from_state();
    }
}

#[cfg(feature = "retained-renderer")]
fn handle_scene_runtime_failure(error: crate::companion::retained::RetainedFailureCategory) {
    let rollout = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|state| state.scene_runtime_rollout)
            .unwrap_or(SceneRuntimeRollout::Off)
    });
    if rollout == SceneRuntimeRollout::Live {
        fallback_from_retained(error);
        prepare_cold_smooth_fallback_once();
    } else if rollout == SceneRuntimeRollout::Shadow {
        write_boundary_diagnostic(format_args!(
            "glorp retained scene shadow failed without changing presentation: {}\n",
            error.category()
        ));
    }
}

#[cfg(feature = "retained-renderer")]
fn reconcile_scene_runtime(
    snapshot: std::sync::Arc<crate::presentation::companion_scene::CompanionSceneSnapshot>,
) -> std::result::Result<(), crate::companion::retained::RetainedFailureCategory> {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(host) = state
            .as_mut()
            .and_then(|state| state.retained_host.as_mut())
        else {
            return Ok(());
        };
        let _ = host.reconcile_scene_snapshot(snapshot)?;
        Ok(())
    })
}

fn prepare_current_frame_from_state() {
    #[cfg(feature = "retained-renderer")]
    let started_at = Instant::now();
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
                renderer_runtime,
                review_depth,
                pixel_frame,
                smooth_started_at,
                smooth_semantic_art_tick_index,
                redacts_live_hud,
                force_dim_overlay,
                metric_cache,
                ..
            } = state;
            let bounds = view.bounds();
            prepare_companion_frame(
                vm,
                scene,
                renderer_runtime.effective(),
                *review_depth,
                pixel_frame.as_ref(),
                *smooth_started_at,
                *smooth_semantic_art_tick_index,
                *redacts_live_hud,
                *force_dim_overlay,
                bounds,
                metric_cache,
            )
        };
        #[cfg(feature = "retained-renderer")]
        if let Some(host) = state.retained_host.as_mut() {
            host.record_state_prepare_us(crate::companion::retained::duration_us(
                started_at.elapsed(),
            ));
        }
        match prepared {
            Ok(frame) => {
                state.last_good_frame = Some(frame);
                state.last_frame_preparation_error = None;
            }
            Err(err) => record_frame_preparation_error(state, err),
        }
    });
    present_retained_frame();
}

#[cfg(feature = "retained-renderer")]
enum RetainedPresentIdentity {
    Current,
    ActiveGeneration(crate::round::smooth::CompanionContentIdentity),
}

#[cfg(feature = "retained-renderer")]
fn present_retained_frame() {
    present_retained_frame_with(RetainedPresentIdentity::Current);
}

#[cfg(feature = "retained-renderer")]
fn present_retained_frame_with(present_identity: RetainedPresentIdentity) {
    use crate::companion::retained::FrameDisposition;

    let failure = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        let frame = state.last_good_frame.as_ref()?;
        let PreparedRendererFrame::Smooth {
            metrics,
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            plan,
            draw_order,
        } = &frame.renderer
        else {
            return None;
        };
        let review_sample = frame.review_sample;
        // The pet's declared-content identity drives which resource generation
        // the atlas is compiled for. It is stable for the pet's lifetime, so the
        // host reuses the compiled atlas every frame and only rebuilds on a real
        // generation change (a resize's backing-scale change).
        let (identity, refresh_surface) = match present_identity {
            RetainedPresentIdentity::Current => (
                crate::round::smooth::CompanionContentIdentity::for_pet(
                    state.vm.pet_render.generated_species,
                ),
                true,
            ),
            RetainedPresentIdentity::ActiveGeneration(identity) => (identity, false),
        };
        let host = state.retained_host.as_mut()?;
        let background = [
            frame.background.0,
            frame.background.1,
            frame.background.2,
            frame.background.3,
        ];
        let progress = host.render(
            state.view.as_super(),
            plan,
            draw_order,
            *metrics,
            frame.aperture,
            background,
            crate::companion::retained::RetainedChrome {
                mood_aura: [
                    frame.mood_aura_color.0,
                    frame.mood_aura_color.1,
                    frame.mood_aura_color.2,
                    frame.mood_aura_color.3,
                ],
                pet_center_col: *pet_center_col,
                pet_center_row: *pet_center_row,
                pet_width_cells: *pet_width_cells,
                gauges: frame.gauges,
                overlays: &frame.overlay_commands,
                hud: &frame.hud,
                hud_font_size: frame.hud_font_size,
                dim_overlay: frame.dim_overlay,
            },
            &identity,
            refresh_surface,
        );
        // A GPU device fault reported asynchronously is a failure even when the
        // frame otherwise presented, so drain the mailbox before recording any
        // success.
        if let Some(category) = host.drain_gpu_error() {
            return Some(category);
        }
        match progress.disposition() {
            Some(FrameDisposition::SurfacePresentCalled | FrameDisposition::Captured) => {
                if let Some(capture) = state.review_capture.as_mut() {
                    capture.record_frame(review_sample);
                }
                None
            }
            // A failed present routes to the Smooth fallback. The fallback
            // dispositions are carried by RendererRuntimeState, not a per-frame
            // FrameProgress, so they never originate here; matching them
            // explicitly keeps them from being silently swallowed by a wildcard
            // and routes them to the same fallback path if one ever leaks through.
            Some(
                FrameDisposition::Failed(category)
                | FrameDisposition::FallbackPending(category)
                | FrameDisposition::FallbackPainted(category),
            ) => Some(category),
            // A Skipped on-screen present dropped nothing to display, but this
            // tick DID prepare a valid frame — which is exactly what the offscreen
            // retained review capture consumes. Advance the review's bounded-run
            // budget so a perpetually-occluded automation window still terminates
            // and produces the paired artifacts. Presented-sample metrics stay on
            // record_frame above, so this never inflates them.
            Some(FrameDisposition::Skipped(_)) | None => {
                if let Some(capture) = state.review_capture.as_mut() {
                    capture.record_offscreen_review_tick();
                }
                None
            }
        }
    });
    if let Some(category) = failure {
        fallback_from_retained(category);
    }
}

#[cfg(not(feature = "retained-renderer"))]
fn present_retained_frame() {}

#[cfg(feature = "retained-renderer")]
fn fallback_from_retained(error: crate::companion::retained::RetainedFailureCategory) {
    APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else { return };
        if state.retained_host.is_none() {
            return;
        }
        if let Some(host) = state.retained_host.as_mut() {
            host.record_fallback();
            if state.runtime_metrics_out.is_some() {
                state.terminal_runtime_metrics = Some(host.runtime_metrics_snapshot(
                    crate::companion::paired_review::full_preview_capacity_inventory(),
                ));
            }
        }
        state.retained_host.take();
        // Restore the AppKit-drawn view (this also requests a display) and record
        // the pending fallback. The reverted layer-hosting view does not resume
        // automatic drawRect on setNeedsDisplay alone, so ui_tick drives the
        // Smooth paint each tick until the review terminates (see
        // drive_smooth_fallback_paint).
        crate::companion::retained::ActiveRetainedHost::restore_appkit(state.view.as_super());
        state.renderer_runtime.request_fallback(error);
        write_boundary_diagnostic(format_args!(
            "glorp retained renderer fell back to Smooth: {}\n",
            error.category()
        ));
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
            #[cfg(feature = "retained-renderer")]
            if state.scene_runtime_rollout == SceneRuntimeRollout::Live {
                let Ok(vm) = apply_post_poll_scene_runtime_update(
                    &mut state.presentation_state,
                    state.review_state,
                    update,
                    now,
                    state.smooth_semantic_art_tick_index,
                ) else {
                    return;
                };
                state.pixel_input = None;
                state.vm = vm;
                unsafe { state.view.setNeedsDisplay(true) };
                return;
            }
            let Ok((vm, scene, pixel_input)) = apply_post_poll_update(
                &mut state.presentation_state,
                state.review_state,
                state.renderer_runtime.effective(),
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

#[cfg(feature = "retained-renderer")]
fn apply_post_poll_scene_runtime_update(
    presentation_state: &mut WatchPresentationState,
    review_state: CompanionReviewState,
    update: LiveWatchUpdate,
    now: time::OffsetDateTime,
    semantic_art_tick_index: u64,
) -> Result<WatchViewModel> {
    let mut vm = update.vm;
    crate::watch_live::stamp_live_presentation(
        presentation_state,
        &mut vm,
        update.applied_signal,
        now,
    );
    apply_review_state(review_state, presentation_state, &mut vm, now)?;
    prepare_smooth_view_model_for_tick(&mut vm, semantic_art_tick_index, now)?;
    Ok(vm)
}

fn apply_post_poll_update(
    presentation_state: &mut WatchPresentationState,
    review_state: CompanionReviewState,
    renderer_mode: EffectiveCompanionRenderer,
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
    if renderer_mode.uses_smooth_scene() {
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
        if state.renderer_runtime.effective().is_pixel() {
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
        if state.renderer_runtime.effective().uses_smooth_scene() {
            let due_tick = state
                .smooth_semantic_clock
                .as_mut()
                .and_then(|clock| clock.consume_due_tick(Instant::now()));
            if let Some(tick_index) = due_tick {
                let _ = advance_companion_animation(&mut state.vm, tick_index, now);
                state.animation_frame = tick_index;
                state.smooth_semantic_art_tick_index = tick_index;
                #[cfg(feature = "retained-renderer")]
                if state.scene_runtime_rollout != SceneRuntimeRollout::Live {
                    state.scene = derive_round_scene_model(&state.vm, now);
                }
                #[cfg(not(feature = "retained-renderer"))]
                {
                    state.scene = derive_round_scene_model(&state.vm, now);
                }
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
        if !state
            .review_capture
            .as_ref()
            .is_some_and(|capture| capture.ready_to_finish())
        {
            return None;
        }
        #[cfg(feature = "retained-renderer")]
        if state.runtime_metrics_out.is_some() {
            if state.runtime_baseline_visibility.begin_hidden_segment() {
                return None;
            }
            if !state.runtime_baseline_visibility.ready_for_terminal_work() {
                return None;
            }
        }
        // Produce the paired Smooth/Retained artifacts from the frozen last-good
        // frame before the capture session is torn down.
        #[cfg(feature = "retained-renderer")]
        if state.runtime_metrics_out.is_some() {
            let species = state.vm.pet_render.generated_species;
            if let Some(host) = state.retained_host.as_mut() {
                if let Err(error) = host.run_virtual_lifetime_audit(4_500, |_phase, frame, now| {
                    prepare_lifetime_fixture_frame(species, frame, now)
                }) {
                    write_boundary_diagnostic(format_args!(
                        "glorp runtime lifetime audit failed: {}\n",
                        error.category()
                    ));
                    std::process::exit(1);
                }
            }
        }
        #[cfg(feature = "retained-renderer")]
        run_paired_capture(state);
        #[cfg(feature = "retained-renderer")]
        if let Err(error) = write_runtime_metrics_if_requested(state) {
            write_boundary_diagnostic(format_args!(
                "glorp runtime metrics write failed: {error}\n"
            ));
            std::process::exit(1);
        }
        let capture = state.review_capture.take()?;
        Some((state.view.clone(), capture))
    });
    let Some((view, mut capture)) = pending_capture else {
        return;
    };

    // Relay the paired-capture terminal result to the process-level slot so `run`
    // can fail the process after NSApplication exits.
    #[cfg(feature = "retained-renderer")]
    let pair_capture_failed = if let Some(outcome) = capture.take_pair_capture_result() {
        if let Err(err) = &outcome {
            eprintln!("glorp: {err}");
        }
        let failed = outcome.is_err();
        store_pair_capture_outcome(outcome);
        failed
    } else {
        false
    };

    if let Err(err) = capture.finish(view.as_super()) {
        eprintln!("glorp review capture failed: {err}");
    }

    // A capture fault (readback/map/blank/write) must fail the process, but
    // NSApplication::terminate exits with status 0 and never returns — so `run`'s
    // post-`app.run()` relay check can never observe the stored Err. Fail the
    // process here, before terminating cleanly, so a direct `companion-app`
    // capture failure exits nonzero. A capture-only fault does not degrade the
    // renderer, so the effective renderer stays Retained.
    #[cfg(feature = "retained-renderer")]
    if pair_capture_failed {
        std::process::exit(1);
    }

    unsafe {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).terminate(None);
        }
    }
}

#[cfg(feature = "retained-renderer")]
fn write_runtime_metrics_if_requested(state: &mut AppState) -> Result<()> {
    let Some(path) = state.runtime_metrics_out.take() else {
        return Ok(());
    };
    let inventory = crate::companion::paired_review::full_preview_capacity_inventory();
    if !inventory.fits_global_constraints() {
        return Err(GlorpError::Message(
            "companion capacity inventory exceeds the frozen scene limits".into(),
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let live = state
        .retained_host
        .as_ref()
        .map(|host| host.runtime_metrics_snapshot(inventory));
    let snapshot = select_terminal_runtime_metrics(live, state.terminal_runtime_metrics.clone())?;
    std::fs::write(path, serde_json::to_vec_pretty(&snapshot)?)?;
    Ok(())
}

#[cfg(feature = "retained-renderer")]
fn select_terminal_runtime_metrics(
    live: Option<crate::companion::retained::CompanionRuntimeMetricsSnapshot>,
    terminal: Option<crate::companion::retained::CompanionRuntimeMetricsSnapshot>,
) -> Result<crate::companion::retained::CompanionRuntimeMetricsSnapshot> {
    live.or(terminal).ok_or_else(|| {
        GlorpError::Message(
            "retained runtime metrics requested without live or terminal evidence".into(),
        )
    })
}

/// Freezes the last-good frame and produces the paired Smooth/Retained capture.
/// Runs only when the review writes artifacts and a retained host is live; the
/// terminal result is stashed on the [`ReviewCapture`] for the process to relay.
#[cfg(feature = "retained-renderer")]
fn run_paired_capture(state: &mut AppState) {
    use crate::companion::paired_review::{
        CapturePrivacy, PairedCaptureCoordinator, PairedReviewFrame, ReviewFrameDimensions,
    };

    // Read the dev/test capture fault (Copy) before borrowing state's fields.
    #[cfg(feature = "dev-preview")]
    let injected_capture_fault = state
        .retained_fault_injection
        .and_then(crate::commands::companion_mode::RetainedFaultInjection::capture_fault_category);
    #[cfg(not(feature = "dev-preview"))]
    let injected_capture_fault: Option<crate::companion::retained::RetainedFailureCategory> = None;

    let AppState {
        review_capture,
        retained_host,
        last_good_frame,
        renderer_runtime,
        smooth_started_at,
        smooth_semantic_art_tick_index,
        review_capture_live_values,
        ..
    } = state;

    let Some(capture) = review_capture.as_mut() else {
        return;
    };
    let Some(out_dir) = capture.capture_dir().map(std::path::Path::to_path_buf) else {
        return;
    };
    let Some(host) = retained_host.as_mut() else {
        return;
    };
    let Some(prepared) = last_good_frame.as_ref() else {
        return;
    };

    let (physical_width, physical_height) = host.physical_size();
    let logical = ReviewFrameDimensions {
        width: prepared.bounds.width_f64,
        height: prepared.bounds.height_f64,
    };
    let physical = ReviewFrameDimensions {
        width: f64::from(physical_width),
        height: f64::from(physical_height),
    };
    let elapsed_ms = smooth_started_at
        .map(|started_at| started_at.elapsed().as_millis())
        .unwrap_or(0)
        .min(u128::from(u64::MAX)) as u64;
    // Freeze ONE review frame; both capture paths consume this frozen frame.
    let frame = PairedReviewFrame::from_prepared(
        prepared.clone(),
        host.backing_scale(),
        logical,
        physical,
        *smooth_semantic_art_tick_index,
        elapsed_ms,
        host.current_frame_id(),
        host.current_resource_generation(),
    );
    let privacy = CapturePrivacy::from_live_values(*review_capture_live_values);
    let result = PairedCaptureCoordinator::new(
        &frame,
        host,
        renderer_runtime,
        privacy,
        out_dir,
        injected_capture_fault,
    )
    .run();
    capture.record_pair_capture_result(result);
}

// The process-level terminal outcome of the paired capture, relayed from the
// review lifecycle so `run` can fail the process after NSApplication exits.
#[cfg(feature = "retained-renderer")]
thread_local! {
    static PAIRED_CAPTURE_OUTCOME: RefCell<Option<Result<()>>> = const { RefCell::new(None) };
}

#[cfg(feature = "retained-renderer")]
fn store_pair_capture_outcome(outcome: Result<()>) {
    PAIRED_CAPTURE_OUTCOME.with(|cell| *cell.borrow_mut() = Some(outcome));
}

#[cfg(feature = "retained-renderer")]
fn take_pair_capture_outcome() -> Option<Result<()>> {
    PAIRED_CAPTURE_OUTCOME.with(|cell| cell.borrow_mut().take())
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
            // A Smooth paint after a runtime fallback acknowledges the degraded
            // path reached the screen (no-op unless a fallback is pending). A live
            // retained host paints via Metal, not this AppKit path, so this only
            // ever promotes the disposition once the host has been torn down.
            #[cfg(feature = "retained-renderer")]
            state.renderer_runtime.acknowledge_smooth_paint();
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
    // Paint directly from the immutable prepared frame. Cloning here copied the
    // full layered scene (including every glyph String and Vec) on every AppKit
    // repaint solely to shorten a RefCell borrow; draw callbacks are synchronous,
    // so holding the immutable borrow across paint is both safe and much cheaper.
    let review_sample = APP_STATE.with(|cell| {
        let state = cell.borrow();
        #[cfg(feature = "retained-renderer")]
        if state
            .as_ref()
            .is_some_and(|state| state.retained_host.is_some())
        {
            return state
                .as_ref()
                .and_then(|state| state.last_good_frame.as_ref())
                .and_then(|frame| frame.review_sample);
        }
        match state
            .as_ref()
            .and_then(|state| state.last_good_frame.as_ref())
        {
            Some(frame) => {
                paint_prepared_frame(bounds, frame);
                frame.review_sample
            }
            None => {
                paint_fallback_background(bounds);
                None
            }
        }
    });
    record_review_frame(view, review_sample);
}

fn paint_prepared_frame(bounds: NSRect, frame: &PreparedCompanionFrame) {
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
            PreparedRendererFrame::Smooth { metrics, plan, draw_order, .. } => {
                draw_mood_aura(frame, metrics);
                appkit_blit_smooth_plan(plan, draw_order, metrics, &aperture);
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
        // The arcs (which to draw, their angles, colours, and order) come from the
        // shared `prepared_perimeter_gauge_arcs`, the same list the retained GPU prep
        // consumes, so the gauge geometry lives in exactly one place.
        {
            let cx = aperture.center_x as f64;
            let cy = aperture.center_y as f64;
            let layout =
                perimeter_gauge_layout(cx, cy, aperture.radius as f64, COMPANION_GAUGE_GAP_DEG);
            let colors = perimeter_gauge_colors();
            let arcs = prepared_perimeter_gauge_arcs(
                &layout,
                &colors,
                GaugeFractions {
                    xp: frame.gauges.xp_fraction,
                    daily: frame.gauges.daily_fraction,
                    daily_overage: frame.gauges.daily_overage_fraction,
                    pace: frame.gauges.pace_fraction,
                },
            );
            for arc in &arcs {
                draw_prepared_gauge_arc(arc);
            }
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

/// Paints the frozen prepared frame into an off-screen physical-size bitmap and
/// returns its straight RGBA8 bytes. The retained-renderer paired capture uses
/// this for the Smooth half so both PNGs share one physical resolution. The
/// bitmap's point size is the logical frame, so the AppKit painter's
/// logical-coordinate drawing scales up to fill the physical pixel grid.
///
/// This paints the SAME frozen [`PreparedCompanionFrame`] the retained readback
/// consumes — it never rebuilds the scene or reads any `AppState` clock.
#[cfg(feature = "retained-renderer")]
pub(super) fn render_prepared_frame_to_rgba(
    frame: &PreparedCompanionFrame,
    physical_width: u32,
    physical_height: u32,
    backing_scale: f64,
) -> Result<Vec<u8>> {
    if physical_width == 0 || physical_height == 0 || backing_scale <= 0.0 {
        return Err(GlorpError::Message(
            "invalid paired smooth capture dimensions".into(),
        ));
    }
    let logical_width = f64::from(physical_width) / backing_scale;
    let logical_height = f64::from(physical_height) / backing_scale;
    unsafe {
        let storage = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            physical_width as isize,
            physical_height as isize,
            8,
            4,
            true,
            false,
            NSCalibratedRGBColorSpace,
            (physical_width * 4) as isize,
            32,
        )
        .ok_or_else(|| GlorpError::Message("failed to allocate paired smooth capture bitmap".into()))?;
        // Retag the shared pixel buffer to sRGB BEFORE compositing so translucency
        // blends in the same sRGB/display space the live Smooth NSView context uses,
        // not in `NSCalibratedRGBColorSpace` (gamma ≈ 1.8). The live path never
        // composites in the calibrated space, so blending there made the captured
        // `smooth.png` an unfaithful reference for translucent content (opaque was
        // already exact via the convert-on-readback below). This is the
        // compositing-space completion of the earlier output-encoding fix.
        let rep = storage
            .bitmapImageRepByRetaggingWithColorSpace(&NSColorSpace::sRGBColorSpace())
            .ok_or_else(|| {
                GlorpError::Message("failed to retag paired smooth capture to sRGB".into())
            })?;
        rep.setSize(NSSize::new(logical_width, logical_height));
        let context =
            NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).ok_or_else(|| {
                GlorpError::Message("failed to create paired smooth capture context".into())
            })?;
        let previous = NSGraphicsContext::currentContext();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let bounds = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(logical_width, logical_height),
        );
        paint_prepared_frame(bounds, frame);
        context.flushGraphics();
        NSGraphicsContext::setCurrentContext(previous.as_deref());

        // Compositing already happened in sRGB (the rep is retagged sRGB above), so
        // this convert-to-sRGB is now an identity kept for symmetry and to drop any
        // row padding uniformly.
        let srgb_rep = rep
            .bitmapImageRepByConvertingToColorSpace_renderingIntent(
                &NSColorSpace::sRGBColorSpace(),
                NSColorRenderingIntent::Default,
            )
            .ok_or_else(|| {
                GlorpError::Message("failed to convert paired smooth capture to sRGB".into())
            })?;

        let data = srgb_rep.bitmapData();
        if data.is_null() {
            return Err(GlorpError::Message(
                "paired smooth capture bitmap has no backing store".into(),
            ));
        }
        // Drop any per-row padding the converted rep may carry, packing into the
        // tight `width * 4` layout the retained artifact overlays against.
        let source_stride = srgb_rep.bytesPerRow() as usize;
        let packed_stride = (physical_width * 4) as usize;
        let mut rgba = vec![0_u8; packed_stride * physical_height as usize];
        for row in 0..physical_height as usize {
            let source = std::slice::from_raw_parts(data.add(row * source_stride), packed_stride);
            rgba[row * packed_stride..(row + 1) * packed_stride].copy_from_slice(source);
        }
        Ok(rgba)
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
    let max_r = mood_aura_radius(pet_width_cells * metrics.cell_w);
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
    let core = tank_core_color(*background);
    let center = (width as f32 / 2.0, height as f32 / 2.0);
    let radius = width.min(height) as f32 / 2.0;

    unsafe {
        let storage = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
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
        // `tank_background_sample` writes straight-sRGB8 bytes, and the retained
        // shader outputs those same sRGB values verbatim. Tag the buffer sRGB (not
        // the legacy `NSCalibratedRGBColorSpace` gamma-1.8) so drawing this image
        // into the sRGB/display context does not colour-shift the bytes, keeping the
        // Smooth tank base bit-identical to the retained kind-3 tank falloff.
        // Fully-qualified: the `NSColorSpace` import is retained-renderer-gated, but
        // this Smooth-backend tank builder compiles feature-off too.
        let rep = storage.bitmapImageRepByRetaggingWithColorSpace(
            &objc2_app_kit::NSColorSpace::sRGBColorSpace(),
        )?;
        let data = rep.bitmapData();
        if data.is_null() {
            return None;
        }
        for y in 0..height {
            for x in 0..width {
                let pixel = tank_background_sample(x, y, center, radius, core, *background);
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
) -> Retained<NSAttributedString> {
    cached_attributed_text(text, font_size, false, color)
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

fn rgba8_key(color: &RoundColor) -> [u8; 4] {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        channel(color.0),
        channel(color.1),
        channel(color.2),
        channel(color.3),
    ]
}

/// Attributed strings retain CoreText's shaped run and attribute dictionary.
/// Smooth cells reuse a small glyph/palette/font vocabulary, but previously
/// rebuilt those objects for every cell of every frame.
fn cached_attributed_text(
    text: &str,
    font_size: f64,
    bold: bool,
    color: &RoundColor,
) -> Retained<NSAttributedString> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    type AttributedStyleKey = ((i64, bool), [u8; 4]);
    #[derive(Default)]
    struct AttributedTextCache {
        by_style: HashMap<AttributedStyleKey, HashMap<String, Retained<NSAttributedString>>>,
        len: usize,
    }

    thread_local! {
        static ATTRIBUTED_TEXT: RefCell<AttributedTextCache> =
            RefCell::new(AttributedTextCache::default());
    }

    let style_key = (font_cache_key(font_size, bold), rgba8_key(color));
    ATTRIBUTED_TEXT.with(|cache| {
        let mut cache = cache.borrow_mut();
        // HUD totals can change over a long session and depth introduces a few
        // adjacent alpha/size keys. Keep this a bounded session cache.
        if cache.len > 512 {
            *cache = AttributedTextCache::default();
        }
        if let Some(cached) = cache
            .by_style
            .get(&style_key)
            .and_then(|texts| texts.get(text))
        {
            return cached.clone();
        }

        let attributed = unsafe {
            let text = NSString::from_str(text);
            let font = cached_monospaced_font(font_size, bold);
            let mut attr = NSMutableAttributedString::from_nsstring(&text);
            let range = objc2_foundation::NSRange::from(0..text.length());
            attr.addAttribute_value_range(NSFontAttributeName, &font, range);
            attr.addAttribute_value_range(NSForegroundColorAttributeName, &ns_color(color), range);
            Retained::into_super(attr)
        };
        cache
            .by_style
            .entry(style_key)
            .or_default()
            .insert(text.to_string(), attributed.clone());
        cache.len += 1;
        attributed
    })
}

fn ns_color(color: &RoundColor) -> Retained<NSColor> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static COLORS: RefCell<HashMap<[u8; 4], Retained<NSColor>>> =
            RefCell::new(HashMap::new());
    }

    let key = rgba8_key(color);
    COLORS.with(|colors| {
        let mut colors = colors.borrow_mut();
        colors
            .entry(key)
            .or_insert_with(|| unsafe {
                NSColor::colorWithSRGBRed_green_blue_alpha(
                    f64::from(key[0]) / 255.0,
                    f64::from(key[1]) / 255.0,
                    f64::from(key[2]) / 255.0,
                    f64::from(key[3]) / 255.0,
                )
            })
            .clone()
    })
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

fn smooth_layer_draw_order(plan: &SmoothCompanionScenePlan) -> Vec<usize> {
    let mut order: Vec<_> = (0..plan.layers.len()).collect();
    order.sort_by_key(|&index| (plan.layers[index].z, index));
    order
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
    draw_order: &[usize],
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

    for &layer_index in draw_order {
        let Some(layer) = plan.layers.get(layer_index) else {
            debug_assert!(false, "prepared smooth draw order must reference a layer");
            continue;
        };
        // Validation already happened while constructing the prepared plan.
        if layer.opacity <= 0.0 {
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
                cached_attributed_text(glyph, frame.font_size, true, &fg)
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

/// Strokes one shared [`PreparedGaugeArc`]: the same NSBezierPath arc the painter
/// has always drawn, now sourced from the backend-neutral prepared list so the
/// geometry is never re-derived here.
#[cfg(target_os = "macos")]
fn draw_prepared_gauge_arc(arc: &crate::round::hud::PreparedGaugeArc) {
    unsafe {
        let path = NSBezierPath::new();
        path.setLineWidth(arc.stroke_width);
        path.setLineCapStyle(ns_line_cap(arc.cap));
        path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
            NSPoint::new(arc.ring.cx, arc.ring.cy),
            arc.ring.radius,
            arc.start_deg,
            arc.end_deg,
        );
        ns_color(&arc.color).setStroke();
        path.stroke();
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
    let big_color = RoundColor(0.93, 0.93, 0.97, 1.0);
    let sub_color =
        crate::round::hud::rate_direction_color(crate::tui::view_model::RateDirection::Neutral);
    let texts = [
        (&hud_text.today_total, &big_color),
        (&hud_text.daily_percent, &sub_color),
        (&hud_text.pace, &sub_color),
    ];
    // The shrink policy and stacking come from the shared `prepare_hud_layout`; the
    // only backend-specific input is how AppKit measures each attributed run.
    let layout = prepare_hud_layout(
        gap,
        aperture.radius as f64,
        bounds.size.height,
        font_size,
        |sizes| {
            let mut metrics = [HudLineMetrics { width: 0.0, height: 0.0 }; 3];
            for (index, metric) in metrics.iter_mut().enumerate() {
                let size = unsafe {
                    attributed_pet_glyph(texts[index].0, sizes[index], texts[index].1).size()
                };
                *metric = HudLineMetrics { width: size.width, height: size.height };
            }
            metrics
        },
    );

    unsafe {
        for (index, (text, color)) in texts.iter().enumerate() {
            let line = layout.lines[index];
            let run = attributed_pet_glyph(text, line.font_size, color);
            run.drawAtPoint(NSPoint::new(line.origin_x, line.baseline_y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "retained-renderer")]
    #[test]
    fn retained_live_scene_presents_at_thirty_hertz() {
        assert_eq!(
            companion_tick_interval(
                EffectiveCompanionRenderer::Retained,
                SceneRuntimeRollout::Live,
            ),
            1.0 / 30.0,
        );
        assert_eq!(
            companion_tick_interval(
                EffectiveCompanionRenderer::Retained,
                SceneRuntimeRollout::Shadow,
            ),
            1.0 / 15.0,
        );
        assert_eq!(
            companion_tick_interval(
                EffectiveCompanionRenderer::Smooth,
                SceneRuntimeRollout::Live,
            ),
            1.0 / 15.0,
        );
    }

    #[cfg(feature = "retained-renderer")]
    #[test]
    fn scene_runtime_fallback_cold_builds_smooth_frame_exactly_once() {
        let mut gate = ColdSmoothFallbackGate::default();
        assert!(gate.take_prepare_request());
        assert!(!gate.take_prepare_request());
        assert!(!gate.take_prepare_request());
    }

    #[cfg(feature = "retained-renderer")]
    #[test]
    fn runtime_baseline_visibility_requires_three_real_hidden_ui_ticks() {
        let mut phase = RuntimeBaselineVisibilityPhase::Visible;
        assert!(!phase.forces_hidden());
        assert!(!phase.ready_for_terminal_work());

        assert!(phase.begin_hidden_segment());
        assert_eq!(phase, RuntimeBaselineVisibilityPhase::HiddenTransition);
        assert!(phase.forces_hidden());

        phase.record_hidden_ui_tick();
        assert_eq!(
            phase,
            RuntimeBaselineVisibilityPhase::HiddenSteady { completed: 0 }
        );
        assert!(!phase.ready_for_terminal_work());
        phase.record_hidden_ui_tick();
        assert_eq!(
            phase,
            RuntimeBaselineVisibilityPhase::HiddenSteady { completed: 1 }
        );
        assert!(!phase.ready_for_terminal_work());
        phase.record_hidden_ui_tick();
        assert_eq!(phase, RuntimeBaselineVisibilityPhase::Complete);
        assert!(phase.ready_for_terminal_work());
    }

    #[cfg(feature = "retained-renderer")]
    #[test]
    fn terminal_capture_snapshot_survives_live_host_teardown() {
        let mut metrics = crate::companion::retained::CompanionRuntimeMetrics::default();
        metrics.record_capture_attempt();
        metrics.record_capture_success();
        let terminal = metrics.snapshot(
            crate::companion::retained::RuntimeIdentity::baseline(),
            crate::companion::retained::CompanionCapacityInventory::contract_fixture(),
            crate::companion::retained::RuntimeFixtureIdentity {
                fixture_id: "glorp-scene-baseline-v2",
                seed: "test",
                update_source: "fixed",
                cadence_ms: 250,
                logical_width: 360.0,
                logical_height: 360.0,
                physical_width: 720,
                physical_height: 720,
                backing_scale: 2.0,
            },
        );
        let selected = select_terminal_runtime_metrics(None, Some(terminal)).unwrap();
        assert_eq!(selected.capture_attempted, 1);
        assert_eq!(selected.capture_succeeded, 1);
        assert_eq!(selected.capture_failed, 0);
    }

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

        assert_eq!(
            prepare_hud_frame(&vm, true),
            crate::round::hud::review_capture_hud_text()
        );
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
            EffectiveCompanionRenderer::Pixel,
            None,
            None,
            None,
            0,
            false,
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
            EffectiveCompanionRenderer::Pixel,
            None,
            None,
            None,
            0,
            false,
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
        let capture_text = crate::round::hud::review_capture_hud_text();

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
            EffectiveCompanionRenderer::Classic,
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
            EffectiveCompanionRenderer::Classic,
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
            EffectiveCompanionRenderer::Classic,
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
            EffectiveCompanionRenderer::Classic,
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
            EffectiveCompanionRenderer::Smooth,
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
            EffectiveCompanionRenderer::Smooth,
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
