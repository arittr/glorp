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
const MAX_BOB_SAMPLES: usize = 120;

#[derive(Debug)]
pub struct ReviewCapture {
    renderer: CompanionRendererMode,
    state: CompanionReviewState,
    requested_size: Option<CompanionReviewSize>,
    duration: Duration,
    capture_dir: Option<PathBuf>,
    started_at: Instant,
    frame_count: u64,
    smooth_bob_samples: Vec<f32>,
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
            smooth_bob_samples: Vec::new(),
            panic: false,
            screenshot_written: false,
            render_log_written: false,
        }))
    }

    pub fn record_frame(&mut self, smooth_bob: Option<f32>) {
        self.frame_count = self.frame_count.saturating_add(1);
        if let Some(bob) = smooth_bob {
            if self.smooth_bob_samples.len() < MAX_BOB_SAMPLES {
                self.smooth_bob_samples.push(round_bob_sample(bob));
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
        let log = RenderLog {
            renderer: self.renderer.as_str(),
            review_state: self.state.as_str(),
            requested_size: self.requested_size.map(ReviewSizeLog::from),
            frame_count: self.frame_count,
            elapsed_duration_ms: self.started_at.elapsed().as_millis(),
            smooth_bob_samples: &self.smooth_bob_samples,
            panic: self.panic,
        };
        let json = serde_json::to_string_pretty(&log)?;
        std::fs::write(capture_dir.join("render-log.json"), json)?;
        Ok(())
    }
}

#[derive(Serialize)]
struct RenderLog<'a> {
    renderer: &'static str,
    review_state: &'static str,
    requested_size: Option<ReviewSizeLog>,
    frame_count: u64,
    elapsed_duration_ms: u128,
    smooth_bob_samples: &'a [f32],
    panic: bool,
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

fn round_bob_sample(value: f32) -> f32 {
    (value * 10_000.0).round() / 10_000.0
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
}
