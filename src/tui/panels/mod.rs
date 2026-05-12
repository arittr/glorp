//! Panel trait and concrete panel implementations.
//!
//! Each panel owns its rendering, its preferred sizing, and its honest
//! minimum height. The dispatcher in `tui::layout` builds a panel list per
//! frame and lays them out via ratatui `Layout`.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;

pub mod bars;
pub mod feed;
pub mod helpers;
pub mod pet;
pub mod spark;
pub mod today;
pub mod vitals;

pub use feed::FeedPanel;
pub use helpers::HelpersPanel;
pub use pet::PetPanel;
pub use spark::SparkPanel;
pub use today::TodayPanel;
pub use vitals::VitalsPanel;

pub trait Panel {
    /// Preferred layout constraint for this panel given the current view
    /// model. Panels that need exact sizing return `Constraint::Length(n)`;
    /// flexible panels return `Constraint::Min(n)`.
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint;

    /// Render into the allocated rect.
    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext);
}
