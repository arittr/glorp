use crate::error::Result;

#[cfg(target_os = "macos")]
pub fn run() -> Result<()> {
    crate::menubar::run()
}

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<()> {
    Err(crate::error::GlorpError::Message(
        "glorp menubar is only available on macOS".into(),
    ))
}
