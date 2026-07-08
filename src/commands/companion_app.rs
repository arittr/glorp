use crate::commands::companion_mode::{CompanionRendererMode, CompanionReviewOptions};
#[cfg(not(target_os = "macos"))]
use crate::error::GlorpError;
use crate::error::Result;

#[cfg(target_os = "macos")]
pub fn run(mode: CompanionRendererMode, review: CompanionReviewOptions) -> Result<()> {
    crate::companion::run(mode, review)
}

#[cfg(not(target_os = "macos"))]
pub fn run(_mode: CompanionRendererMode, _review: CompanionReviewOptions) -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion-app is only available on macOS".into(),
    ))
}
