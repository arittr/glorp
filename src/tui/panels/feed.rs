//! TODO(phase 2 task 6): port from layout.rs.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::panels::Panel;
use crate::tui::view_model::WatchViewModel;

pub struct FeedPanel;

impl Panel for FeedPanel {
    fn min_height(&self, _width: u16) -> u16 {
        0
    }

    fn preferred_constraint(&self) -> Constraint {
        Constraint::Min(0)
    }

    fn render(&self, _area: Rect, _buf: &mut Buffer, _vm: &WatchViewModel) {
        // TODO: implement in Phase 2 Task 6
    }
}
