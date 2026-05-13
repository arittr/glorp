//! Legacy panel trait and concrete panel implementations.
//!
//! Each panel owns its rendering, its preferred sizing, and its honest
//! minimum height. The watch component layout allocates panel bounds and
//! renders these compatibility panels through `render_watch_layout`.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

pub mod bars;
pub mod bio_card;
pub mod feed;
pub mod pet;
pub mod progress;
pub mod today;
pub mod vitals;

pub use bio_card::BioCardPanel;
pub use feed::FeedPanel;
pub use pet::PetPanel;
pub use progress::ProgressPanel;
pub use today::TodayPanel;
pub use vitals::VitalsPanel;

/// Compatibility trait for panels that have not moved fully into component
/// widgets. The old public `Panel` name is intentionally gone so this cannot
/// be confused with `component::ComponentPanel`.
pub trait LegacyPanel {
    /// Preferred layout constraint for this panel given the current view
    /// model. Panels that need exact sizing return `Constraint::Length(n)`;
    /// flexible panels return `Constraint::Min(n)`.
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint;

    /// Render into the allocated rect.
    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext);
}
