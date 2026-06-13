#![cfg(target_os = "macos")]

pub mod app;
pub mod render;

pub fn run() -> crate::error::Result<()> {
    app::run()
}
