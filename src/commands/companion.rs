use crate::commands::companion_mode::{CompanionRendererMode, CompanionReviewOptions};
use crate::error::{GlorpError, Result};

#[cfg(target_os = "macos")]
pub fn run(mode: CompanionRendererMode, review: CompanionReviewOptions) -> Result<()> {
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
    let mut command = build_open_command(&app, mode, review);
    let status = command.status()?;
    if !status.success() {
        return Err(GlorpError::Message(format!(
            "failed to open Glorp.app at {}",
            app.display()
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn build_open_command(
    app: &std::path::Path,
    mode: CompanionRendererMode,
    review: CompanionReviewOptions,
) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    let needs_args = mode.is_pixel() || mode.is_smooth() || review.has_review_launch_options();
    if needs_args {
        command.arg("-n");
    }
    command.arg(app);
    if needs_args {
        command.arg("--args");
    }
    if mode.is_pixel() || mode.is_smooth() {
        command.arg("--renderer").arg(mode.as_str());
    }
    if let Some(size) = review.initial_size {
        let size = format!("{}x{}", size.width, size.height);
        command.args(["--review-size", &size]);
    }
    if review.active_pulse {
        command.arg("--review-active-pulse");
    }
    if let Some(state) = review.state {
        command.arg("--review-state").arg(state.as_str());
    }
    if let Some(duration_ms) = review.duration_ms {
        command
            .arg("--review-duration-ms")
            .arg(duration_ms.to_string());
    }
    if let Some(capture_dir) = review.capture_dir {
        command.arg("--review-capture-dir").arg(capture_dir);
    }
    command
}

#[cfg(not(target_os = "macos"))]
pub fn run(_mode: CompanionRendererMode, _review: CompanionReviewOptions) -> Result<()> {
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::build_open_command;
    use crate::commands::companion_mode::{
        CompanionRendererMode, CompanionReviewOptions, CompanionReviewSize, CompanionReviewState,
    };
    use std::ffi::OsString;
    use std::path::Path;

    #[test]
    fn review_options_force_fresh_open_for_classic_renderer() {
        let command = build_open_command(
            Path::new("/Applications/Glorp.app"),
            CompanionRendererMode::Classic,
            CompanionReviewOptions {
                initial_size: Some(CompanionReviewSize { width: 360, height: 360 }),
                active_pulse: true,
                ..CompanionReviewOptions::default()
            },
        );

        assert_eq!(command.get_program(), "open");
        let args: Vec<OsString> = command.get_args().map(|arg| arg.to_os_string()).collect();
        assert_eq!(
            args,
            vec![
                OsString::from("-n"),
                OsString::from("/Applications/Glorp.app"),
                OsString::from("--args"),
                OsString::from("--review-size"),
                OsString::from("360x360"),
                OsString::from("--review-active-pulse"),
            ]
        );
    }

    #[test]
    fn smooth_renderer_opens_in_fresh_window_with_renderer_arg() {
        let command = build_open_command(
            Path::new("/Applications/Glorp.app"),
            CompanionRendererMode::Smooth,
            CompanionReviewOptions::default(),
        );

        assert_eq!(command.get_program(), "open");
        let args: Vec<OsString> = command.get_args().map(|arg| arg.to_os_string()).collect();
        assert_eq!(
            args,
            vec![
                OsString::from("-n"),
                OsString::from("/Applications/Glorp.app"),
                OsString::from("--args"),
                OsString::from("--renderer"),
                OsString::from("smooth"),
            ]
        );
    }

    #[test]
    fn review_open_command_forwards_state_duration_and_capture_dir() {
        let command = build_open_command(
            Path::new("/Applications/Glorp.app"),
            CompanionRendererMode::Smooth,
            CompanionReviewOptions {
                state: Some(CompanionReviewState::ActivePulse),
                duration_ms: Some(2000),
                capture_dir: Some("target/glorp-review/test".into()),
                ..CompanionReviewOptions::default()
            },
        );

        assert_eq!(command.get_program(), "open");
        let args: Vec<OsString> = command.get_args().map(|arg| arg.to_os_string()).collect();
        assert_eq!(
            args,
            vec![
                OsString::from("-n"),
                OsString::from("/Applications/Glorp.app"),
                OsString::from("--args"),
                OsString::from("--renderer"),
                OsString::from("smooth"),
                OsString::from("--review-state"),
                OsString::from("active-pulse"),
                OsString::from("--review-duration-ms"),
                OsString::from("2000"),
                OsString::from("--review-capture-dir"),
                OsString::from("target/glorp-review/test"),
            ]
        );
    }
}
