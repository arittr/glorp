#![cfg(target_os = "macos")]

pub mod app;
#[cfg(feature = "retained-renderer")]
mod direct_capture;
#[cfg(feature = "retained-renderer")]
pub mod paired_review;
pub mod pixel;
pub mod render;
#[cfg(feature = "retained-renderer")]
pub mod retained;
pub mod review_capture;
pub mod smooth_timing;

pub fn run(
    request: crate::commands::companion_mode::CompanionRendererRequest,
    review: crate::commands::companion_mode::CompanionReviewOptions,
) -> crate::error::Result<()> {
    app::run(request, review)
}
