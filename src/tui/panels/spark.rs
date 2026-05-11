//! TODO(phase 2 task 5): port from layout.rs.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};

use crate::tui::panels::Panel;
use crate::tui::view_model::WatchViewModel;

pub struct SparkPanel;

impl Panel for SparkPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        Constraint::Min(0)
    }

    fn render(&self, _area: Rect, _buf: &mut Buffer, _vm: &WatchViewModel) {
        // TODO: implement in Phase 2 Task 5
    }
}
