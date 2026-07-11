#![cfg(target_os = "macos")]

pub mod app;
pub mod pixel;
pub mod render;
#[cfg(feature = "retained-renderer")]
pub mod retained;
pub mod review_capture;
pub mod smooth_timing;

pub fn run(
    mode: crate::commands::companion_mode::CompanionRendererMode,
    review: crate::commands::companion_mode::CompanionReviewOptions,
) -> crate::error::Result<()> {
    app::run(mode, review)
}
