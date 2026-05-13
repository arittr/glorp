//! macOS menubar app. Renders the current pet plus stats inside an NSPopover
//! attached to an NSStatusItem in the system menu bar. The poll pipeline is
//! the same one `glorp watch` uses, so a menubar session and a watch session
//! see consistent state via the SQLite ledger and `state.json`.

#![cfg(target_os = "macos")]

mod app;
mod render;

use crate::error::Result;

pub fn run() -> Result<()> {
    app::run()
}
