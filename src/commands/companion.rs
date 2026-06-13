use crate::error::{GlorpError, Result};

#[cfg(target_os = "macos")]
pub fn run() -> Result<()> {
    let paths = crate::paths::AppPaths::resolve()?;
    paths.ensure()?;
    let locator = crate::usage::helper_locator::HelperLocator::from_current_environment();
    if locator.has_any_path() {
        crate::usage::helper_locator::write_helper_locator(
            &paths
                .config_dir
                .join(crate::usage::helper_locator::HELPER_LOCATOR_FILE),
            &locator,
        )?;
    }
    let app = companion_app_path()?;
    let status = std::process::Command::new("open").arg(&app).status()?;
    if !status.success() {
        return Err(GlorpError::Message(format!(
            "failed to open Glorp.app at {}",
            app.display()
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion is only available on macOS".into(),
    ))
}

#[cfg(target_os = "macos")]
fn companion_app_path() -> Result<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("GLORP_COMPANION_APP") {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    let dev = std::path::PathBuf::from("target/macos/Glorp.app");
    if dev.exists() {
        return Ok(dev);
    }
    let installed = std::path::PathBuf::from("/Applications/Glorp.app");
    if installed.exists() {
        return Ok(installed);
    }
    Err(GlorpError::Message(
        "Glorp.app was not found; run `node scripts/build-macos-companion-app.mjs --profile debug` in development".into(),
    ))
}
