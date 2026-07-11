#![cfg(target_os = "macos")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::commands::companion_mode::{
    CompanionReviewOptions, CompanionReviewSize, CompanionReviewState, EffectiveCompanionRenderer,
};
use crate::error::{GlorpError, Result};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRepPropertyKey, NSView};
use objc2_foundation::{NSDictionary, NSRect, NSString};
use serde::Serialize;

const MIN_CAPTURE_FRAMES: u64 = 5;
const MAX_SMOOTH_FRAME_SAMPLES: usize = 120;

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct SmoothReviewPoint {
    pub x: f32,
    pub y: f32,
}

impl SmoothReviewPoint {
    pub fn from_smooth_point(point: crate::presentation::smooth::SmoothPoint) -> Self {
        Self { x: point.x, y: point.y }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct SmoothReviewParallaxPlanes {
    pub far: SmoothReviewPoint,
    pub mid: SmoothReviewPoint,
    pub behind: SmoothReviewPoint,
    pub foreground: SmoothReviewPoint,
}

impl SmoothReviewParallaxPlanes {
    pub fn from_smooth_planes(
        planes: crate::presentation::smooth::SmoothParallaxPlaneTranslations,
    ) -> Self {
        Self {
            far: SmoothReviewPoint::from_smooth_point(planes.far),
            mid: SmoothReviewPoint::from_smooth_point(planes.mid),
            behind: SmoothReviewPoint::from_smooth_point(planes.behind),
            foreground: SmoothReviewPoint::from_smooth_point(planes.foreground),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct SmoothReviewFrameSample {
    pub bob_y: f32,
    pub semantic_art_tick_index: u64,
    pub pet_visual_checksum: u64,
    pub base_anchor: SmoothReviewPoint,
    pub bob_offset: SmoothReviewPoint,
    pub final_anchor: SmoothReviewPoint,
    pub classic_snap_anchor: SmoothReviewPoint,
    pub parallax_focus_offset: SmoothReviewPoint,
    pub parallax_lifecycle_scale: f32,
    pub parallax_planes: SmoothReviewParallaxPlanes,
    /// Raw depth channel in `[-1, 1]`; negative is far, positive is near.
    pub depth: f32,
    /// Uniform depth scale applied to the pet this frame.
    pub pet_scale: f32,
    /// Vertical perspective offset the depth sample resolves to.
    pub perspective_y: f32,
    /// Width of the pet's transformed bounds after depth composition.
    pub pet_extent_width: f32,
    /// Height of the pet's transformed bounds after depth composition.
    pub pet_extent_height: f32,
    /// Count of typed shape draws emitted across every layer this frame.
    pub shape_draw_count: u32,
}

#[derive(Debug)]
pub struct ReviewCapture {
    renderer: EffectiveCompanionRenderer,
    state: CompanionReviewState,
    requested_size: Option<CompanionReviewSize>,
    duration: Duration,
    capture_dir: Option<PathBuf>,
    started_at: Instant,
    frame_count: u64,
    /// Ticks that prepared a valid frame the offscreen retained capture can
    /// consume, but whose on-screen present was Skipped. The retained review
    /// artifact is read back off an intermediate texture, so these still advance
    /// the bounded run toward [`Self::ready_to_finish`] even though nothing was
    /// presented. Kept separate from `frame_count` so presented-sample metrics
    /// stay exact.
    offscreen_review_tick_count: u64,
    semantic_art_tick_count: u64,
    smooth_frame_samples: Vec<SmoothReviewFrameSample>,
    max_adjacent_base_anchor_delta: SmoothReviewPoint,
    max_adjacent_final_anchor_delta: SmoothReviewPoint,
    max_adjacent_parallax_delta_by_plane: SmoothReviewParallaxPlanes,
    min_pet_scale: f32,
    max_pet_scale: f32,
    max_adjacent_pet_scale_delta: f32,
    nonblank_shape_frame_count: u64,
    pet_checksums_by_semantic_tick: BTreeMap<u64, u64>,
    pet_checksums_stable_within_semantic_ticks: bool,
    last_smooth_sample: Option<SmoothReviewFrameSample>,
    panic: bool,
    callback_panic_count: u64,
    last_callback_panic_label: Option<&'static str>,
    frame_preparation_error_count: u64,
    last_frame_preparation_error: Option<&'static str>,
    last_good_frame_reused_count: u64,
    screenshot_written: bool,
    render_log_written: bool,
    /// The terminal outcome of the paired Smooth/Retained capture, if one ran.
    /// The direct companion-app review process relays this so a capture fault
    /// becomes a process error after `NSApplication` exits.
    #[cfg(feature = "retained-renderer")]
    pair_capture_result: Option<Result<()>>,
}

impl ReviewCapture {
    pub fn from_options(
        renderer: EffectiveCompanionRenderer,
        review: &CompanionReviewOptions,
    ) -> Result<Option<Self>> {
        let capture_dir = review.capture_dir.clone();
        if capture_dir.is_none() && review.duration_ms.is_none() {
            return Ok(None);
        };
        if let Some(capture_dir) = capture_dir.as_ref() {
            std::fs::create_dir_all(capture_dir)?;
        }
        Ok(Some(Self {
            renderer,
            state: review.resolved_state(),
            requested_size: review.initial_size,
            duration: Duration::from_millis(review.duration_ms.unwrap_or(0)),
            capture_dir,
            started_at: Instant::now(),
            frame_count: 0,
            offscreen_review_tick_count: 0,
            semantic_art_tick_count: 0,
            smooth_frame_samples: Vec::new(),
            max_adjacent_base_anchor_delta: SmoothReviewPoint::default(),
            max_adjacent_final_anchor_delta: SmoothReviewPoint::default(),
            max_adjacent_parallax_delta_by_plane: SmoothReviewParallaxPlanes::default(),
            // Seed the scale bounds finite so a zero-sample run serializes real
            // numbers; the first recorded sample overwrites both seeds.
            min_pet_scale: 0.0,
            max_pet_scale: 0.0,
            max_adjacent_pet_scale_delta: 0.0,
            nonblank_shape_frame_count: 0,
            pet_checksums_by_semantic_tick: BTreeMap::new(),
            pet_checksums_stable_within_semantic_ticks: true,
            last_smooth_sample: None,
            panic: false,
            callback_panic_count: 0,
            last_callback_panic_label: None,
            frame_preparation_error_count: 0,
            last_frame_preparation_error: None,
            last_good_frame_reused_count: 0,
            screenshot_written: false,
            render_log_written: false,
            #[cfg(feature = "retained-renderer")]
            pair_capture_result: None,
        }))
    }

    /// The capture output directory, when this review writes artifacts.
    #[cfg(feature = "retained-renderer")]
    pub fn capture_dir(&self) -> Option<&Path> {
        self.capture_dir.as_deref()
    }

    /// Stores the terminal result of the paired capture for the process to relay.
    #[cfg(feature = "retained-renderer")]
    pub fn record_pair_capture_result(&mut self, result: Result<()>) {
        self.pair_capture_result = Some(result);
    }

    /// Takes the paired-capture terminal result, if one was recorded.
    #[cfg(feature = "retained-renderer")]
    pub fn take_pair_capture_result(&mut self) -> Option<Result<()>> {
        self.pair_capture_result.take()
    }

    pub fn record_frame(&mut self, smooth_sample: Option<SmoothReviewFrameSample>) {
        self.frame_count = self.frame_count.saturating_add(1);
        if let Some(sample) = smooth_sample {
            let sample = round_smooth_sample(sample);
            self.semantic_art_tick_count = self
                .semantic_art_tick_count
                .max(sample.semantic_art_tick_index);
            if sample.shape_draw_count > 0 {
                self.nonblank_shape_frame_count = self.nonblank_shape_frame_count.saturating_add(1);
            }
            if let Some(previous) = self.last_smooth_sample {
                self.max_adjacent_base_anchor_delta = max_point_delta(
                    self.max_adjacent_base_anchor_delta,
                    previous.base_anchor,
                    sample.base_anchor,
                );
                self.max_adjacent_final_anchor_delta = max_point_delta(
                    self.max_adjacent_final_anchor_delta,
                    previous.final_anchor,
                    sample.final_anchor,
                );
                self.max_adjacent_parallax_delta_by_plane = max_parallax_plane_delta(
                    self.max_adjacent_parallax_delta_by_plane,
                    previous.parallax_planes,
                    sample.parallax_planes,
                );
                self.min_pet_scale = self.min_pet_scale.min(sample.pet_scale);
                self.max_pet_scale = self.max_pet_scale.max(sample.pet_scale);
                self.max_adjacent_pet_scale_delta = self
                    .max_adjacent_pet_scale_delta
                    .max(round_sample((sample.pet_scale - previous.pet_scale).abs()));
            } else {
                // The first sample seeds both scale bounds so a single-frame run
                // reports its real scale instead of the finite zero placeholder.
                self.min_pet_scale = sample.pet_scale;
                self.max_pet_scale = sample.pet_scale;
            }
            if let Some(existing) = self
                .pet_checksums_by_semantic_tick
                .insert(sample.semantic_art_tick_index, sample.pet_visual_checksum)
            {
                if existing != sample.pet_visual_checksum {
                    self.pet_checksums_stable_within_semantic_ticks = false;
                }
            }
            self.last_smooth_sample = Some(sample);
            if self.smooth_frame_samples.len() < MAX_SMOOTH_FRAME_SAMPLES {
                self.smooth_frame_samples.push(sample);
            }
        }
    }

    pub fn record_callback_panic(&mut self, label: &'static str) {
        self.panic = true;
        self.callback_panic_count = self.callback_panic_count.saturating_add(1);
        self.last_callback_panic_label = Some(label);
    }

    pub fn record_frame_preparation_error(&mut self, category: &'static str) {
        self.frame_preparation_error_count = self.frame_preparation_error_count.saturating_add(1);
        self.last_frame_preparation_error = Some(category);
    }

    pub fn record_last_good_frame_reused(&mut self) {
        self.last_good_frame_reused_count = self.last_good_frame_reused_count.saturating_add(1);
    }

    /// Records a retained tick that prepared a valid frame the offscreen capture
    /// can consume, but whose on-screen present was Skipped. Advances the bounded
    /// run without recording a presented-frame sample.
    #[cfg(feature = "retained-renderer")]
    pub fn record_offscreen_review_tick(&mut self) {
        self.offscreen_review_tick_count = self.offscreen_review_tick_count.saturating_add(1);
    }

    pub fn ready_to_finish(&self) -> bool {
        // A retained review captures OFFSCREEN, so its on-screen present is
        // perpetually Skipped when the window is not frontmost (automation
        // context). Count those prepared-but-skipped ticks toward the budget so
        // the bounded run terminates; otherwise it would hang until the hard
        // timeout, writing no artifacts.
        let prepared_ticks = self
            .frame_count
            .saturating_add(self.offscreen_review_tick_count);
        self.started_at.elapsed() >= self.duration && prepared_ticks >= MIN_CAPTURE_FRAMES
    }

    pub fn writes_artifacts(&self) -> bool {
        self.capture_dir.is_some()
    }

    pub fn redacts_live_hud(&self) -> bool {
        self.writes_artifacts()
    }

    pub fn finish(&mut self, view: &NSView) -> Result<()> {
        let Some(capture_dir) = self.capture_dir.as_ref() else {
            return Ok(());
        };
        if !self.screenshot_written {
            write_screenshot(view, &capture_dir.join("screenshot.png"))?;
            self.screenshot_written = true;
        }
        if !self.render_log_written {
            self.write_render_log()?;
            self.render_log_written = true;
        }
        Ok(())
    }

    pub fn paths(&self) -> Option<(PathBuf, PathBuf)> {
        let capture_dir = self.capture_dir.as_ref()?;
        Some((
            capture_dir.join("screenshot.png"),
            capture_dir.join("render-log.json"),
        ))
    }

    fn write_render_log(&self) -> Result<()> {
        let Some(capture_dir) = self.capture_dir.as_ref() else {
            return Ok(());
        };
        let json = self.render_log_json()?;
        std::fs::write(capture_dir.join("render-log.json"), json)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn render_log_json_for_test(&self) -> Result<String> {
        self.render_log_json()
    }

    #[cfg(test)]
    pub fn pet_checksums_stable_within_semantic_ticks_for_test(&self) -> bool {
        self.pet_checksums_stable_within_semantic_ticks()
    }

    fn render_log_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.render_log()).map_err(Into::into)
    }

    fn render_log(&self) -> RenderLog<'_> {
        RenderLog {
            renderer: self.renderer.as_str(),
            review_state: self.state.as_str(),
            requested_size: self.requested_size.map(ReviewSizeLog::from),
            paint_frame_count: self.frame_count,
            frame_count: self.frame_count,
            elapsed_duration_ms: self.started_at.elapsed().as_millis(),
            semantic_art_tick_count: self.semantic_art_tick_count,
            pet_checksums_stable_within_semantic_ticks: self
                .pet_checksums_stable_within_semantic_ticks,
            max_adjacent_base_anchor_delta: self.max_adjacent_base_anchor_delta,
            max_adjacent_final_anchor_delta: self.max_adjacent_final_anchor_delta,
            max_adjacent_parallax_delta_by_plane: self.max_adjacent_parallax_delta_by_plane,
            min_pet_scale: self.min_pet_scale,
            max_pet_scale: self.max_pet_scale,
            max_adjacent_pet_scale_delta: self.max_adjacent_pet_scale_delta,
            nonblank_shape_frame_count: self.nonblank_shape_frame_count,
            smooth_frame_samples: &self.smooth_frame_samples,
            privacy: ReviewPrivacyLog::from_claims(
                &crate::presentation::smooth::SmoothCompanionPrivacyClaims::external_companion(),
            ),
            panic: self.panic,
            callback_panic_count: self.callback_panic_count,
            last_callback_panic_label: self.last_callback_panic_label,
            frame_preparation_error_count: self.frame_preparation_error_count,
            last_frame_preparation_error: self.last_frame_preparation_error,
            last_good_frame_reused_count: self.last_good_frame_reused_count,
        }
    }

    #[cfg(test)]
    fn pet_checksums_stable_within_semantic_ticks(&self) -> bool {
        self.pet_checksums_stable_within_semantic_ticks
    }
}

#[derive(Serialize)]
struct RenderLog<'a> {
    renderer: &'static str,
    review_state: &'static str,
    requested_size: Option<ReviewSizeLog>,
    paint_frame_count: u64,
    frame_count: u64,
    elapsed_duration_ms: u128,
    semantic_art_tick_count: u64,
    pet_checksums_stable_within_semantic_ticks: bool,
    max_adjacent_base_anchor_delta: SmoothReviewPoint,
    max_adjacent_final_anchor_delta: SmoothReviewPoint,
    max_adjacent_parallax_delta_by_plane: SmoothReviewParallaxPlanes,
    min_pet_scale: f32,
    max_pet_scale: f32,
    max_adjacent_pet_scale_delta: f32,
    nonblank_shape_frame_count: u64,
    smooth_frame_samples: &'a [SmoothReviewFrameSample],
    privacy: ReviewPrivacyLog,
    panic: bool,
    callback_panic_count: u64,
    last_callback_panic_label: Option<&'static str>,
    frame_preparation_error_count: u64,
    last_frame_preparation_error: Option<&'static str>,
    last_good_frame_reused_count: u64,
}

#[derive(Clone, Copy, Serialize)]
struct ReviewPrivacyLog {
    source_names_visible: bool,
    exact_token_strings_visible: bool,
    project_names_visible: bool,
    file_paths_visible: bool,
    prompt_text_visible: bool,
    response_text_visible: bool,
    raw_diagnostics_visible: bool,
    unprojected_pet_seed_visible: bool,
}

impl ReviewPrivacyLog {
    fn from_claims(claims: &crate::presentation::smooth::SmoothCompanionPrivacyClaims) -> Self {
        Self {
            source_names_visible: claims.source_names_visible,
            exact_token_strings_visible: claims.exact_token_strings_visible,
            project_names_visible: claims.project_names_visible,
            file_paths_visible: claims.file_paths_visible,
            prompt_text_visible: claims.prompt_text_visible,
            response_text_visible: claims.response_text_visible,
            raw_diagnostics_visible: claims.raw_diagnostics_visible,
            unprojected_pet_seed_visible: claims.unprojected_pet_seed_visible,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
struct ReviewSizeLog {
    width: u16,
    height: u16,
}

impl From<CompanionReviewSize> for ReviewSizeLog {
    fn from(size: CompanionReviewSize) -> Self {
        Self { width: size.width, height: size.height }
    }
}

fn write_screenshot(view: &NSView, path: &Path) -> Result<()> {
    unsafe {
        view.displayIfNeeded();
        let bounds: NSRect = view.bounds();
        let Some(bitmap) = view.bitmapImageRepForCachingDisplayInRect(bounds) else {
            return Err(GlorpError::Message(
                "failed to allocate review screenshot bitmap".into(),
            ));
        };
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
        let properties: Retained<NSDictionary<NSBitmapImageRepPropertyKey, AnyObject>> =
            NSDictionary::new();
        let Some(data) =
            bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
        else {
            return Err(GlorpError::Message(
                "failed to encode review screenshot as png".into(),
            ));
        };
        let path = NSString::from_str(&path.to_string_lossy());
        if !data.writeToFile_atomically(&path, true) {
            return Err(GlorpError::Message(
                "failed to write review screenshot png".into(),
            ));
        }
    }
    Ok(())
}

fn round_sample(value: f32) -> f32 {
    (value * 10_000.0).round() / 10_000.0
}

fn round_review_point(point: SmoothReviewPoint) -> SmoothReviewPoint {
    SmoothReviewPoint {
        x: round_sample(point.x),
        y: round_sample(point.y),
    }
}

fn round_smooth_sample(sample: SmoothReviewFrameSample) -> SmoothReviewFrameSample {
    SmoothReviewFrameSample {
        bob_y: round_sample(sample.bob_y),
        semantic_art_tick_index: sample.semantic_art_tick_index,
        pet_visual_checksum: sample.pet_visual_checksum,
        base_anchor: round_review_point(sample.base_anchor),
        bob_offset: round_review_point(sample.bob_offset),
        final_anchor: round_review_point(sample.final_anchor),
        classic_snap_anchor: round_review_point(sample.classic_snap_anchor),
        parallax_focus_offset: round_review_point(sample.parallax_focus_offset),
        parallax_lifecycle_scale: round_sample(sample.parallax_lifecycle_scale),
        parallax_planes: SmoothReviewParallaxPlanes {
            far: round_review_point(sample.parallax_planes.far),
            mid: round_review_point(sample.parallax_planes.mid),
            behind: round_review_point(sample.parallax_planes.behind),
            foreground: round_review_point(sample.parallax_planes.foreground),
        },
        depth: round_sample(sample.depth),
        pet_scale: round_sample(sample.pet_scale),
        perspective_y: round_sample(sample.perspective_y),
        pet_extent_width: round_sample(sample.pet_extent_width),
        pet_extent_height: round_sample(sample.pet_extent_height),
        shape_draw_count: sample.shape_draw_count,
    }
}

fn max_point_delta(
    max_delta: SmoothReviewPoint,
    previous: SmoothReviewPoint,
    current: SmoothReviewPoint,
) -> SmoothReviewPoint {
    SmoothReviewPoint {
        x: max_delta
            .x
            .max(round_sample((current.x - previous.x).abs())),
        y: max_delta
            .y
            .max(round_sample((current.y - previous.y).abs())),
    }
}

fn max_parallax_plane_delta(
    maximum: SmoothReviewParallaxPlanes,
    previous: SmoothReviewParallaxPlanes,
    current: SmoothReviewParallaxPlanes,
) -> SmoothReviewParallaxPlanes {
    SmoothReviewParallaxPlanes {
        far: max_point_delta(maximum.far, previous.far, current.far),
        mid: max_point_delta(maximum.mid, previous.mid, current.mid),
        behind: max_point_delta(maximum.behind, previous.behind, current.behind),
        foreground: max_point_delta(maximum.foreground, previous.foreground, current.foreground),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const RENDER_LOG_ALLOWED_STRING_VALUES: &[&str] = &[
        "classic",
        "pixel",
        "smooth",
        "normal",
        "active-pulse",
        "asleep-calm",
        "helper-trouble",
        "drawrect",
        "uitick",
        "smooth-missing-pet-body",
        "smooth-invalid-parallax-geometry",
    ];

    const RENDER_LOG_FORBIDDEN_PRIVACY_VALUE_TOKENS: &[&str] = &[
        "source_name",
        "display_name",
        "client-secret-project",
        "/users/",
        "/tmp/",
        "prompt",
        "response",
        "transcript",
        "tool payload",
        "diagnostic",
        "very-secret-seed",
    ];

    #[test]
    fn duration_only_review_options_create_session_without_artifacts() {
        let capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration-only review should create an auto-terminating session");

        assert!(!capture.writes_artifacts());
        assert!(!capture.redacts_live_hud());
    }

    #[test]
    fn artifact_capture_requests_redacted_live_hud() {
        let dir = tempfile::tempdir().unwrap();
        let capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                capture_dir: Some(dir.path().join("capture")),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("capture dir should create review capture session");

        assert!(capture.writes_artifacts());
        assert!(capture.redacts_live_hud());
    }

    #[cfg(feature = "retained-renderer")]
    #[test]
    fn capture_fault_is_a_terminal_process_error_but_graceful_run_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Retained,
            &CompanionReviewOptions {
                duration_ms: Some(0),
                capture_dir: Some(dir.path().join("pair")),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("capture dir should create review capture session");

        // A graceful run (no paired-capture fault) records nothing, so there is no
        // terminal error and the process exits cleanly.
        assert!(capture.take_pair_capture_result().is_none());

        // A capture fault records a terminal Err, which finish_review_capture_if_due
        // turns into a nonzero process exit.
        capture.record_pair_capture_result(Err(crate::error::GlorpError::Message(
            "paired retained capture failed: retained-capture-map-failed".into(),
        )));
        assert!(matches!(capture.take_pair_capture_result(), Some(Err(_))));
        // Draining is idempotent.
        assert!(capture.take_pair_capture_result().is_none());
    }

    #[cfg(feature = "retained-renderer")]
    #[test]
    fn retained_review_terminates_on_offscreen_ticks_without_any_present() {
        let dir = tempfile::tempdir().unwrap();
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Retained,
            &CompanionReviewOptions {
                duration_ms: Some(0),
                capture_dir: Some(dir.path().join("pair")),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("capture dir should create a review session");

        // The on-screen present is perpetually Skipped in an automation window, so
        // no presented frame is ever recorded and the legacy frame_count gate would
        // never fire. Prepared-but-skipped ticks feed the offscreen capture and
        // must still drive the bounded run to completion.
        for _ in 0..(MIN_CAPTURE_FRAMES - 1) {
            assert!(!capture.ready_to_finish());
            capture.record_offscreen_review_tick();
        }
        capture.record_offscreen_review_tick();
        assert!(
            capture.ready_to_finish(),
            "offscreen retained ticks must satisfy the bounded run without any present",
        );

        // Nothing was presented, so presented-sample metrics stay empty: the paired
        // capture path is reached with zero SurfacePresentCalled frames.
        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["frame_count"], 0);
        assert_eq!(value["smooth_frame_samples"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn smooth_review_capture_records_requested_evidence_and_privacy() {
        let dir = tempfile::tempdir().unwrap();
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                capture_dir: Some(dir.path().join("capture")),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("capture dir should create review capture session");

        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.1,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 10.25, y: 12.5 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
            final_anchor: SmoothReviewPoint { x: 10.25, y: 12.6 },
            classic_snap_anchor: SmoothReviewPoint { x: 10.0, y: 12.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.25, y: 0.125 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: 0.01, y: 0.0075 },
                mid: SmoothReviewPoint { x: 0.02, y: 0.015 },
                behind: SmoothReviewPoint { x: 0.03, y: 0.0225 },
                foreground: SmoothReviewPoint { x: 0.045, y: 0.03374 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.2,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 10.30, y: 12.55 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.2 },
            final_anchor: SmoothReviewPoint { x: 10.30, y: 12.75 },
            classic_snap_anchor: SmoothReviewPoint { x: 10.0, y: 12.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.5, y: 0.25 },
            parallax_lifecycle_scale: 0.5,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: -0.01516, y: -0.01014 },
                mid: SmoothReviewPoint { x: -0.00216, y: 0.01714 },
                behind: SmoothReviewPoint { x: 0.01114, y: -0.00114 },
                foreground: SmoothReviewPoint { x: -0.00516, y: 0.00494 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.3,
            semantic_art_tick_index: 1,
            pet_visual_checksum: 456,
            base_anchor: SmoothReviewPoint { x: 10.30, y: 12.85 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.3 },
            final_anchor: SmoothReviewPoint { x: 10.30, y: 13.05 },
            classic_snap_anchor: SmoothReviewPoint { x: 10.0, y: 12.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.75, y: 0.375 },
            parallax_lifecycle_scale: 0.25,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: 0.00494, y: 0.02226 },
                mid: SmoothReviewPoint { x: 0.02506, y: -0.01266 },
                behind: SmoothReviewPoint { x: -0.00994, y: 0.04444 },
                foreground: SmoothReviewPoint { x: 0.03006, y: -0.02666 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.4,
            semantic_art_tick_index: 2,
            pet_visual_checksum: 789,
            base_anchor: SmoothReviewPoint { x: 10.4, y: 13.15 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.4 },
            final_anchor: SmoothReviewPoint { x: 10.4, y: 13.55 },
            classic_snap_anchor: SmoothReviewPoint { x: 10.0, y: 12.0 },
            parallax_focus_offset: SmoothReviewPoint { x: -0.5, y: 0.625 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: -0.01004, y: -0.00376 },
                mid: SmoothReviewPoint { x: -0.01504, y: 0.00626 },
                behind: SmoothReviewPoint { x: 0.01806, y: 0.01004 },
                foreground: SmoothReviewPoint { x: -0.02244, y: 0.01884 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));
        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["paint_frame_count"], 4);
        assert_eq!(value["frame_count"], 4);
        assert_eq!(value["semantic_art_tick_count"], 2);
        assert_eq!(value["max_adjacent_base_anchor_delta"]["x"], 0.1);
        assert_eq!(value["max_adjacent_base_anchor_delta"]["y"], 0.3);
        assert_eq!(value["max_adjacent_final_anchor_delta"]["x"], 0.1);
        assert_eq!(value["max_adjacent_final_anchor_delta"]["y"], 0.5);
        assert_eq!(value["smooth_frame_samples"].as_array().unwrap().len(), 4);
        assert_eq!(value["smooth_frame_samples"][0]["pet_visual_checksum"], 123);
        assert_eq!(
            value["smooth_frame_samples"][0]["parallax_lifecycle_scale"],
            1.0
        );
        assert_eq!(
            value["smooth_frame_samples"][0]["parallax_focus_offset"]["x"],
            0.25
        );
        assert_eq!(
            value["smooth_frame_samples"][0]["parallax_planes"]["far"]["x"],
            0.01
        );
        assert_eq!(
            value["smooth_frame_samples"][0]["parallax_planes"]["foreground"]["x"],
            0.045
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["far"]["x"],
            0.0252
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["far"]["y"],
            0.0324
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["mid"]["x"],
            0.0401
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["mid"]["y"],
            0.0298
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["behind"]["x"],
            0.028
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["behind"]["y"],
            0.0455
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["foreground"]["x"],
            0.0525
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["foreground"]["y"],
            0.0455
        );
        assert_eq!(value["pet_checksums_stable_within_semantic_ticks"], true);
        assert_eq!(value["privacy"]["source_names_visible"], false);
        assert_eq!(value["privacy"]["exact_token_strings_visible"], false);
    }

    #[test]
    fn smooth_review_capture_first_sample_has_zero_parallax_aggregate() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.1,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
            final_anchor: SmoothReviewPoint { x: 1.0, y: 1.1 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            parallax_focus_offset: SmoothReviewPoint { x: -0.5, y: 0.25 },
            parallax_lifecycle_scale: 0.5,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: -0.01004, y: 0.00756 },
                mid: SmoothReviewPoint { x: 0.02006, y: -0.01504 },
                behind: SmoothReviewPoint { x: -0.03006, y: 0.02504 },
                foreground: SmoothReviewPoint { x: 0.04004, y: -0.03506 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["far"]["x"],
            0.0
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["far"]["y"],
            0.0
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["mid"]["x"],
            0.0
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["mid"]["y"],
            0.0
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["behind"]["x"],
            0.0
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["behind"]["y"],
            0.0
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["foreground"]["x"],
            0.0
        );
        assert_eq!(
            value["max_adjacent_parallax_delta_by_plane"]["foreground"]["y"],
            0.0
        );
    }

    #[test]
    fn review_capture_records_boundary_health_without_private_strings() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                state: Some(CompanionReviewState::Normal),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        capture.record_callback_panic("drawRect");
        capture.record_callback_panic("uiTick");
        capture.record_frame_preparation_error("smooth-missing-pet-body");
        capture.record_last_good_frame_reused();
        capture.record_last_good_frame_reused();

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["callback_panic_count"], 2);
        assert_eq!(value["last_callback_panic_label"], "uiTick");
        assert_eq!(value["frame_preparation_error_count"], 1);
        assert_eq!(
            value["last_frame_preparation_error"],
            "smooth-missing-pet-body"
        );
        assert_eq!(value["last_good_frame_reused_count"], 2);
        assert_render_log_json_values_are_sanitized(&value, "$");
    }

    #[test]
    fn smooth_review_capture_checksum_stability_detects_flashing() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.1,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
            final_anchor: SmoothReviewPoint { x: 1.0, y: 1.1 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: 0.0, y: 0.0 },
                mid: SmoothReviewPoint { x: 0.0, y: 0.0 },
                behind: SmoothReviewPoint { x: 0.0, y: 0.0 },
                foreground: SmoothReviewPoint { x: 0.0, y: 0.0 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.2,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 456,
            base_anchor: SmoothReviewPoint { x: 1.1, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.2 },
            final_anchor: SmoothReviewPoint { x: 1.1, y: 1.2 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: 0.0, y: 0.0 },
                mid: SmoothReviewPoint { x: 0.0, y: 0.0 },
                behind: SmoothReviewPoint { x: 0.0, y: 0.0 },
                foreground: SmoothReviewPoint { x: 0.0, y: 0.0 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));

        assert!(!capture.pet_checksums_stable_within_semantic_ticks_for_test());
    }

    #[test]
    fn smooth_review_capture_checksum_stability_covers_unsampled_frames() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        for index in 0..(MAX_SMOOTH_FRAME_SAMPLES + 1) {
            capture.record_frame(Some(SmoothReviewFrameSample {
                bob_y: 0.1,
                semantic_art_tick_index: 0,
                pet_visual_checksum: 123,
                base_anchor: SmoothReviewPoint { x: index as f32, y: 1.0 },
                bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
                final_anchor: SmoothReviewPoint { x: index as f32, y: 1.1 },
                classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
                parallax_focus_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
                parallax_lifecycle_scale: 1.0,
                parallax_planes: SmoothReviewParallaxPlanes {
                    far: SmoothReviewPoint { x: 0.0, y: 0.0 },
                    mid: SmoothReviewPoint { x: 0.0, y: 0.0 },
                    behind: SmoothReviewPoint { x: 0.0, y: 0.0 },
                    foreground: SmoothReviewPoint { x: 0.0, y: 0.0 },
                },
                depth: 0.0,
                pet_scale: 1.0,
                perspective_y: 0.0,
                pet_extent_width: 9.0,
                pet_extent_height: 6.0,
                shape_draw_count: 0,
            }));
        }
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.1,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 456,
            base_anchor: SmoothReviewPoint { x: 200.0, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
            final_anchor: SmoothReviewPoint { x: 200.0, y: 1.1 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: 0.0, y: 0.0 },
                mid: SmoothReviewPoint { x: 0.0, y: 0.0 },
                behind: SmoothReviewPoint { x: 0.0, y: 0.0 },
                foreground: SmoothReviewPoint { x: 0.0, y: 0.0 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["smooth_frame_samples"].as_array().unwrap().len(), 120);
        assert_eq!(value["pet_checksums_stable_within_semantic_ticks"], false);
    }

    #[test]
    fn smooth_review_capture_render_log_strings_stay_sanitized() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                state: Some(CompanionReviewState::HelperTrouble),
                initial_size: Some(CompanionReviewSize { width: 360, height: 360 }),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.1,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.1 },
            final_anchor: SmoothReviewPoint { x: 1.0, y: 1.1 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: 0.0, y: 0.0 },
                mid: SmoothReviewPoint { x: 0.0, y: 0.0 },
                behind: SmoothReviewPoint { x: 0.0, y: 0.0 },
                foreground: SmoothReviewPoint { x: 0.0, y: 0.0 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.2,
            semantic_art_tick_index: 1,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 1.1, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.2 },
            final_anchor: SmoothReviewPoint { x: 1.1, y: 1.2 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes {
                far: SmoothReviewPoint { x: 0.0, y: 0.0 },
                mid: SmoothReviewPoint { x: 0.0, y: 0.0 },
                behind: SmoothReviewPoint { x: 0.0, y: 0.0 },
                foreground: SmoothReviewPoint { x: 0.0, y: 0.0 },
            },
            depth: 0.0,
            pet_scale: 1.0,
            perspective_y: 0.0,
            pet_extent_width: 9.0,
            pet_extent_height: 6.0,
            shape_draw_count: 0,
        }));

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_render_log_json_values_are_sanitized(&value, "$");
    }

    fn depth_evidence_sample(
        pet_scale: f32,
        pet_extent_width: f32,
        pet_extent_height: f32,
        shape_draw_count: u32,
    ) -> SmoothReviewFrameSample {
        SmoothReviewFrameSample {
            bob_y: 0.0,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
            final_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
            parallax_focus_offset: SmoothReviewPoint { x: 0.0, y: 0.0 },
            parallax_lifecycle_scale: 1.0,
            parallax_planes: SmoothReviewParallaxPlanes::default(),
            depth: (pet_scale - 0.88) / 0.12 - 1.0,
            pet_scale,
            perspective_y: (pet_scale - 1.0) * 3.75,
            pet_extent_width,
            pet_extent_height,
            shape_draw_count,
        }
    }

    #[test]
    fn depth_review_samples_are_ordered_from_far_to_near() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        capture.record_frame(Some(depth_evidence_sample(0.88, 8.0, 6.0, 3)));
        capture.record_frame(Some(depth_evidence_sample(1.0, 9.0, 6.75, 3)));
        capture.record_frame(Some(depth_evidence_sample(1.12, 10.0, 7.5, 3)));

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        let samples = value["smooth_frame_samples"].as_array().unwrap();
        let width = |index: usize| samples[index]["pet_extent_width"].as_f64().unwrap();
        let scale = |index: usize| samples[index]["pet_scale"].as_f64().unwrap();
        assert!(width(0) < width(1) && width(1) < width(2));
        assert!(scale(0) < scale(1) && scale(1) < scale(2));
        assert_eq!(value["min_pet_scale"], 0.88);
        assert_eq!(value["max_pet_scale"], 1.12);
    }

    #[test]
    fn nonblank_shape_frame_count_counts_only_frames_with_shape_draws() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        capture.record_frame(Some(depth_evidence_sample(1.0, 9.0, 6.0, 0)));
        capture.record_frame(Some(depth_evidence_sample(1.0, 9.0, 6.0, 4)));
        capture.record_frame(Some(depth_evidence_sample(1.0, 9.0, 6.0, 0)));
        capture.record_frame(Some(depth_evidence_sample(1.0, 9.0, 6.0, 2)));

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["frame_count"], 4);
        assert_eq!(value["nonblank_shape_frame_count"], 2);
    }

    #[test]
    fn max_adjacent_pet_scale_delta_stays_bounded_across_animation() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        // Ramp the pet forward and back one step at a time, mimicking the smooth
        // depth roam without pinning it to a fixed plane.
        let scales = [
            0.88, 0.90, 0.94, 0.99, 1.04, 1.09, 1.12, 1.09, 1.03, 0.97, 0.91, 0.88,
        ];
        for scale in scales {
            capture.record_frame(Some(depth_evidence_sample(scale, 9.0, 6.0, 3)));
        }

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        let delta = value["max_adjacent_pet_scale_delta"].as_f64().unwrap();
        assert!(delta > 0.0, "animation should exercise scale change");
        assert!(
            delta < 0.08,
            "adjacent scale delta must stay bounded, got {delta}"
        );
        assert_eq!(value["min_pet_scale"], 0.88);
        assert_eq!(value["max_pet_scale"], 1.12);
    }

    #[test]
    fn zero_sample_run_serializes_finite_scale_bounds() {
        let capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert!(value["min_pet_scale"].as_f64().unwrap().is_finite());
        assert!(value["max_pet_scale"].as_f64().unwrap().is_finite());
        assert_eq!(value["max_adjacent_pet_scale_delta"], 0.0);
        assert_eq!(value["nonblank_shape_frame_count"], 0);
    }

    #[test]
    fn depth_review_evidence_stays_sanitized() {
        let mut capture = ReviewCapture::from_options(
            EffectiveCompanionRenderer::Smooth,
            &CompanionReviewOptions {
                duration_ms: Some(2000),
                state: Some(CompanionReviewState::HelperTrouble),
                initial_size: Some(CompanionReviewSize { width: 360, height: 360 }),
                ..CompanionReviewOptions::default()
            },
        )
        .unwrap()
        .expect("duration should create review capture session");

        capture.record_frame(Some(depth_evidence_sample(0.88, 8.0, 6.0, 3)));
        capture.record_frame(Some(depth_evidence_sample(1.0, 9.0, 6.75, 3)));
        capture.record_frame(Some(depth_evidence_sample(1.12, 10.0, 7.5, 3)));

        let json = capture.render_log_json_for_test().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_render_log_json_values_are_sanitized(&value, "$");
    }

    fn assert_render_log_json_values_are_sanitized(value: &Value, path: &str) {
        match value {
            Value::String(text) => {
                let text = text.to_ascii_lowercase();
                assert!(
                    RENDER_LOG_ALLOWED_STRING_VALUES.contains(&text.as_str()),
                    "unexpected render log string at {path}: {text}"
                );
                for forbidden in RENDER_LOG_FORBIDDEN_PRIVACY_VALUE_TOKENS {
                    assert!(
                        !text.contains(forbidden),
                        "render log leaked {forbidden} at {path}: {text}"
                    );
                }
            }
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate() {
                    assert_render_log_json_values_are_sanitized(child, &format!("{path}[{index}]"));
                }
            }
            Value::Object(map) => {
                for (key, child) in map {
                    assert_render_log_json_values_are_sanitized(child, &format!("{path}.{key}"));
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
}
