#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::commands::companion_mode::{
    CompanionRendererMode, CompanionReviewOptions, CompanionReviewSize, CompanionReviewState,
};
use crate::error::{GlorpError, Result};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRepPropertyKey, NSView};
use objc2_foundation::{NSDictionary, NSRect, NSString};
use serde::Serialize;

const MIN_CAPTURE_FRAMES: u64 = 5;
const MAX_SMOOTH_FRAME_SAMPLES: usize = 120;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct SmoothReviewPoint {
    pub x: f32,
    pub y: f32,
}

impl SmoothReviewPoint {
    pub fn from_smooth_point(point: crate::presentation::smooth::SmoothPoint) -> Self {
        Self { x: point.x, y: point.y }
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
}

#[derive(Debug)]
pub struct ReviewCapture {
    renderer: CompanionRendererMode,
    state: CompanionReviewState,
    requested_size: Option<CompanionReviewSize>,
    duration: Duration,
    capture_dir: Option<PathBuf>,
    started_at: Instant,
    frame_count: u64,
    smooth_frame_samples: Vec<SmoothReviewFrameSample>,
    panic: bool,
    screenshot_written: bool,
    render_log_written: bool,
}

impl ReviewCapture {
    pub fn from_options(
        renderer: CompanionRendererMode,
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
            smooth_frame_samples: Vec::new(),
            panic: false,
            screenshot_written: false,
            render_log_written: false,
        }))
    }

    pub fn record_frame(&mut self, smooth_sample: Option<SmoothReviewFrameSample>) {
        self.frame_count = self.frame_count.saturating_add(1);
        if let Some(sample) = smooth_sample {
            if self.smooth_frame_samples.len() < MAX_SMOOTH_FRAME_SAMPLES {
                self.smooth_frame_samples.push(round_smooth_sample(sample));
            }
        }
    }

    pub fn ready_to_finish(&self) -> bool {
        self.started_at.elapsed() >= self.duration && self.frame_count >= MIN_CAPTURE_FRAMES
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
            frame_count: self.frame_count,
            elapsed_duration_ms: self.started_at.elapsed().as_millis(),
            semantic_art_tick_count: self.semantic_art_tick_count(),
            smooth_frame_samples: &self.smooth_frame_samples,
            privacy: ReviewPrivacyLog::from_claims(
                &crate::presentation::smooth::SmoothCompanionPrivacyClaims::external_companion(),
            ),
            panic: self.panic,
        }
    }

    fn semantic_art_tick_count(&self) -> u64 {
        self.smooth_frame_samples
            .iter()
            .map(|sample| sample.semantic_art_tick_index)
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64
    }

    #[cfg(test)]
    fn pet_checksums_stable_within_semantic_ticks(&self) -> bool {
        let mut by_tick = std::collections::BTreeMap::<u64, u64>::new();
        for sample in &self.smooth_frame_samples {
            if let Some(existing) =
                by_tick.insert(sample.semantic_art_tick_index, sample.pet_visual_checksum)
            {
                if existing != sample.pet_visual_checksum {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Serialize)]
struct RenderLog<'a> {
    renderer: &'static str,
    review_state: &'static str,
    requested_size: Option<ReviewSizeLog>,
    frame_count: u64,
    elapsed_duration_ms: u128,
    semantic_art_tick_count: u64,
    smooth_frame_samples: &'a [SmoothReviewFrameSample],
    privacy: ReviewPrivacyLog,
    panic: bool,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_only_review_options_create_session_without_artifacts() {
        let capture = ReviewCapture::from_options(
            CompanionRendererMode::Smooth,
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
            CompanionRendererMode::Smooth,
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

    #[test]
    fn smooth_review_capture_records_semantic_ticks_anchors_and_privacy() {
        let dir = tempfile::tempdir().unwrap();
        let mut capture = ReviewCapture::from_options(
            CompanionRendererMode::Smooth,
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
        }));
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.2,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 123,
            base_anchor: SmoothReviewPoint { x: 10.30, y: 12.55 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.2 },
            final_anchor: SmoothReviewPoint { x: 10.30, y: 12.75 },
            classic_snap_anchor: SmoothReviewPoint { x: 10.0, y: 12.0 },
        }));
        let json = capture.render_log_json_for_test().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["frame_count"], 2);
        assert_eq!(value["semantic_art_tick_count"], 1);
        assert_eq!(value["smooth_frame_samples"].as_array().unwrap().len(), 2);
        assert_eq!(value["smooth_frame_samples"][0]["pet_visual_checksum"], 123);
        assert_eq!(value["privacy"]["source_names_visible"], false);
        assert_eq!(value["privacy"]["exact_token_strings_visible"], false);
    }

    #[test]
    fn smooth_review_capture_checksum_stability_detects_flashing() {
        let mut capture = ReviewCapture::from_options(
            CompanionRendererMode::Smooth,
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
        }));
        capture.record_frame(Some(SmoothReviewFrameSample {
            bob_y: 0.2,
            semantic_art_tick_index: 0,
            pet_visual_checksum: 456,
            base_anchor: SmoothReviewPoint { x: 1.1, y: 1.0 },
            bob_offset: SmoothReviewPoint { x: 0.0, y: 0.2 },
            final_anchor: SmoothReviewPoint { x: 1.1, y: 1.2 },
            classic_snap_anchor: SmoothReviewPoint { x: 1.0, y: 1.0 },
        }));

        assert!(!capture.pet_checksums_stable_within_semantic_ticks_for_test());
    }
}
