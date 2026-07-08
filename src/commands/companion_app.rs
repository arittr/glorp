use crate::commands::companion_mode::CompanionRendererMode;
#[cfg(not(target_os = "macos"))]
use crate::error::GlorpError;
use crate::error::Result;

#[cfg(target_os = "macos")]
pub fn run(mode: CompanionRendererMode) -> Result<()> {
    crate::companion::run(mode)
}

#[cfg(not(target_os = "macos"))]
pub fn run(_mode: CompanionRendererMode) -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion-app is only available on macOS".into(),
    ))
}
