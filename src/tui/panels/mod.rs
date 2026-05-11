//! Panel trait and concrete panel implementations.
//!
//! Each panel owns its rendering, its preferred sizing, and its honest
//! minimum height. The dispatcher in `tui::layout` builds a panel list per
//! frame and lays them out via ratatui `Layout`.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::view_model::WatchViewModel;

pub mod pet;
pub mod vitals;
pub mod today;
pub mod spark;
pub mod feed;
pub mod helpers;

pub use feed::FeedPanel;
pub use helpers::HelpersPanel;
pub use pet::PetPanel;
pub use spark::SparkPanel;
pub use today::TodayPanel;
pub use vitals::VitalsPanel;

pub trait Panel {
    /// Minimum height (in rows) this panel needs at the given width.
    fn min_height(&self, width: u16) -> u16;

    /// Preferred constraint for the layout solver.
    fn preferred_constraint(&self) -> Constraint;

    /// Render into the allocated rect.
    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel);
}
