use crate::commands::companion_mode::{CompanionRendererRequest, CompanionReviewOptions};
use crate::error::{GlorpError, Result};

#[cfg(target_os = "macos")]
pub fn run(request: CompanionRendererRequest, review: CompanionReviewOptions) -> Result<()> {
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
    let mut command = build_open_command(&app, request, review);
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
    request: CompanionRendererRequest,
    review: CompanionReviewOptions,
) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    let renderer_arg = request.forwarded_arg();
    let needs_args = renderer_arg.is_some() || review.has_review_launch_options();
    if needs_args {
        command.arg("-n");
    }
    command.arg(app);
    if needs_args {
        command.arg("--args");
    }
    if let Some(renderer_arg) = renderer_arg {
        command.arg("--renderer").arg(renderer_arg);
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
    if let Some(metrics_out) = review.runtime_metrics_out {
        command.arg("--review-runtime-metrics-out").arg(metrics_out);
    }
    if let Some(depth) = review.depth {
        command.arg("--review-depth").arg(depth.as_str());
    }
    command
}

#[cfg(not(target_os = "macos"))]
pub fn run(_request: CompanionRendererRequest, _review: CompanionReviewOptions) -> Result<()> {
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
        CompanionRendererRequest, CompanionReviewOptions, CompanionReviewSize, CompanionReviewState,
    };
    use std::ffi::OsString;
    use std::path::Path;

    #[test]
    fn auto_request_reuses_running_instance_without_renderer_arg() {
        let command = build_open_command(
            Path::new("/Applications/Glorp.app"),
            CompanionRendererRequest::Auto,
            CompanionReviewOptions::default(),
        );

        assert_eq!(command.get_program(), "open");
        let args: Vec<OsString> = command.get_args().map(|arg| arg.to_os_string()).collect();
        assert_eq!(args, vec![OsString::from("/Applications/Glorp.app")]);
    }

    #[test]
    fn review_options_force_fresh_open_for_classic_renderer() {
        let command = build_open_command(
            Path::new("/Applications/Glorp.app"),
            CompanionRendererRequest::Classic,
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
                OsString::from("--renderer"),
                OsString::from("classic"),
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
            CompanionRendererRequest::Smooth,
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
            CompanionRendererRequest::Smooth,
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

#[cfg(all(test, target_os = "macos"))]
mod review_depth_forwarding_tests {
    use super::build_open_command;
    use crate::commands::companion_mode::{
        CompanionRendererRequest, CompanionReviewDepth, CompanionReviewOptions,
    };
    use std::ffi::OsString;
    use std::path::Path;

    #[test]
    fn review_depth_is_forwarded_to_the_native_companion_process() {
        for (depth, expected) in [
            (CompanionReviewDepth::Far, "far"),
            (CompanionReviewDepth::Neutral, "neutral"),
            (CompanionReviewDepth::Near, "near"),
        ] {
            let command = build_open_command(
                Path::new("/Applications/Glorp.app"),
                CompanionRendererRequest::Smooth,
                CompanionReviewOptions {
                    depth: Some(depth),
                    ..CompanionReviewOptions::default()
                },
            );
            let args: Vec<OsString> = command.get_args().map(|arg| arg.to_os_string()).collect();
            assert!(
                args.windows(2)
                    .any(|pair| pair[0] == "--review-depth" && pair[1] == expected),
                "expected --review-depth {expected} in {args:?}"
            );
        }
    }

    /// A pinned depth is a review launch on its own, so the app must be spawned
    /// with `-n` and `--args` rather than reusing a running instance.
    #[test]
    fn review_depth_alone_spawns_a_fresh_instance_with_args() {
        let command = build_open_command(
            Path::new("/Applications/Glorp.app"),
            CompanionRendererRequest::Classic,
            CompanionReviewOptions {
                depth: Some(CompanionReviewDepth::Far),
                ..CompanionReviewOptions::default()
            },
        );
        let args: Vec<OsString> = command.get_args().map(|arg| arg.to_os_string()).collect();
        assert_eq!(args[0], OsString::from("-n"));
        assert!(args.contains(&OsString::from("--args")));
    }
}
