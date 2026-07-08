#![cfg(target_os = "macos")]

pub mod app;
pub mod pixel;
pub mod render;

pub fn run(
    mode: crate::commands::companion_mode::CompanionRendererMode,
) -> crate::error::Result<()> {
    app::run(mode)
}
