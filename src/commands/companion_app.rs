use crate::commands::companion_mode::{CompanionRendererRequest, CompanionReviewOptions};
#[cfg(not(target_os = "macos"))]
use crate::error::GlorpError;
use crate::error::Result;

#[cfg(target_os = "macos")]
pub fn run(request: CompanionRendererRequest, review: CompanionReviewOptions) -> Result<()> {
    crate::companion::run(request, review)
}

#[cfg(not(target_os = "macos"))]
pub fn run(_request: CompanionRendererRequest, _review: CompanionReviewOptions) -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion-app is only available on macOS".into(),
    ))
}
