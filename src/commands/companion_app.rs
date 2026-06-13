#[cfg(not(target_os = "macos"))]
use crate::error::GlorpError;
use crate::error::Result;

#[cfg(target_os = "macos")]
pub fn run() -> Result<()> {
    crate::companion::run()
}

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion-app is only available on macOS".into(),
    ))
}
