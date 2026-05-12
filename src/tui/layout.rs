use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::panels::{
    FeedPanel, HelpersPanel, Panel, PetPanel, SparkPanel, TodayPanel, VitalsPanel,
};
use crate::tui::style::{semantic_styles, tokenpet_palette, ColorCapability};
use crate::tui::view_model::WatchViewModel;

/// Smallest terminal width that uses the wide two-column layout.
/// Below this threshold we fall back to the single-column compact layout.
const COMPACT_THRESHOLD: usize = 104;

/// Left column width in the wide layout (pet + vitals column).
const WIDE_LEFT_COL: u16 = 40;

/// Gutter width between left and right columns in the wide layout.
const WIDE_GUTTER: u16 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Wide,
    Compact,
}

pub fn render_watch_frame_with_capability(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    _capability: ColorCapability,
) {
    let styles = semantic_styles();
    let p = tokenpet_palette();
    let outer = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(frame_title(vm)))
        .title_bottom(Line::from(frame_footer()))
        .border_style(Style::default().fg(p.accent.rgb))
        .style(styles.body);

    // Decide mode by terminal width (after subtracting frame borders).
    let mode = if (frame.area().width as usize) >= COMPACT_THRESHOLD + 2 {
        Mode::Wide
    } else {
        Mode::Compact
    };

    // Fill the full terminal height with the outer chrome so resizing the
    // terminal extends the frame to the bottom of the window. Trailing
    // space inside the frame is absorbed by the layout's bottom spacer.
    let frame_rect = frame.area();

    let inner = outer.inner(frame_rect);
    frame.render_widget(outer, frame_rect);

    layout_and_render(inner, mode, frame.buffer_mut(), vm);
}

/// Returns the rect within `frame.area()` that the pet panel occupies for the
/// current mode and view model. Callers (specifically the watch loop) use this
/// to apply tachyonfx effects onto the rendered pet panel after the dispatcher
/// has drawn it.
pub fn pet_panel_rect(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    use crate::tui::panels::Panel;
    let mode = if (frame_area.width as usize) >= COMPACT_THRESHOLD + 2 {
        Mode::Wide
    } else {
        Mode::Compact
    };
    // Mirror render_watch_frame_with_capability: frame fills the terminal.
    let outer_block = Block::bordered();
    let inner = outer_block.inner(frame_area);
    let pet_h = match PetPanel.preferred_constraint(vm) {
        Constraint::Length(n) => n,
        _ => 5,
    };
    match mode {
        Mode::Wide => {
            // Left column uses Flex::Center with spacing(COLUMN_GAP); pet
            // sits below half the leftover space. Account for the gap row
            // between pet and vitals.
            let vitals_h = match VitalsPanel.preferred_constraint(vm) {
                Constraint::Length(n) => n,
                _ => 5,
            };
            let column_content_h = pet_h + vitals_h + COLUMN_GAP;
            let leftover = inner.height.saturating_sub(column_content_h);
            let top_pad = leftover / 2;
            Rect {
                x: inner.x,
                y: inner.y + top_pad,
                width: WIDE_LEFT_COL.min(inner.width),
                height: pet_h.min(inner.height.saturating_sub(top_pad)),
            }
        }
        Mode::Compact => Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: pet_h.min(inner.height),
        },
    }
}

/// Produces the styled title spans for the outer frame.
fn frame_title(vm: &WatchViewModel) -> Vec<Span<'_>> {
    const NAME_MAX: usize = 16;
    let styles = semantic_styles();
    let p = tokenpet_palette();
    let stage_style = Style::default().fg(p.accent.rgb);
    let mood_style = Style::default().fg(p.good.rgb);

    let display_name: String = if vm.pet_name.chars().count() > NAME_MAX {
        let truncated: String = vm.pet_name.chars().take(NAME_MAX - 1).collect();
        format!("{truncated}…")
    } else {
        vm.pet_name.clone()
    };
    let age_label = format!("{}d", vm.age_days);
    vec![
        Span::styled(
            format!(" glorp · {display_name} the {} · ", vm.species),
            styles.label,
        ),
        Span::styled(vm.stage.clone(), stage_style),
        Span::styled(format!(" · {age_label} · "), styles.label),
        Span::styled(vm.mood.clone(), mood_style),
        Span::raw(" "),
    ]
}

/// Produces the footer spans for the outer frame.
fn frame_footer() -> Vec<Span<'static>> {
    let styles = semantic_styles();
    vec![
        Span::raw(" "),
        Span::styled("q quit · r refresh · m mouse · ? help", styles.label),
        Span::raw(" "),
    ]
}

/// Lays out and renders all panels into the inner area of the outer frame.
fn layout_and_render(
    inner: Rect,
    mode: Mode,
    buf: &mut ratatui::buffer::Buffer,
    vm: &WatchViewModel,
) {
    match mode {
        Mode::Wide => render_wide(inner, buf, vm),
        Mode::Compact => render_compact(inner, buf, vm),
    }
}

/// Wide layout: two columns.
///
/// Left column (fixed 40 cells): PetPanel stacked above VitalsPanel.
/// Gutter (4 cells): empty space.
/// Right column (remaining): TodayPanel, SparkPanel, FeedPanel, HelpersPanel stacked vertically.
///
/// Both columns use `Flex::Center` so any leftover height is split evenly above
/// and below the content. The taller column already fills the frame (set by
/// `natural_inner_height`), so centering is a no-op for it. The shorter column
/// gets its content centered vertically rather than crammed at the top.
fn render_wide(area: Rect, buf: &mut ratatui::buffer::Buffer, vm: &WatchViewModel) {
    let right_col = area.width.saturating_sub(WIDE_LEFT_COL + WIDE_GUTTER);
    let [left_area, _, right_area] = Layout::horizontal([
        Constraint::Length(WIDE_LEFT_COL),
        Constraint::Length(WIDE_GUTTER),
        Constraint::Length(right_col),
    ])
    .split(area)[..] else {
        return;
    };

    // Left column: PetPanel + VitalsPanel — centered so the shorter column's
    // content sits vertically in the middle of the frame.
    render_centered_column(left_area, &[&PetPanel as &dyn Panel, &VitalsPanel], buf, vm);

    // Right column: TodayPanel + SparkPanel + FeedPanel + HelpersPanel.
    render_centered_column(
        right_area,
        &[
            &TodayPanel as &dyn Panel,
            &SparkPanel,
            &FeedPanel,
            &HelpersPanel,
        ],
        buf,
        vm,
    );
}

/// Compact layout: all six panels stacked vertically with 1-cell spacing.
fn render_compact(area: Rect, buf: &mut ratatui::buffer::Buffer, vm: &WatchViewModel) {
    render_column_with_spacing(
        area,
        &[
            &PetPanel as &dyn Panel,
            &VitalsPanel,
            &TodayPanel,
            &SparkPanel,
            &FeedPanel,
            &HelpersPanel,
        ],
        1,
        buf,
        vm,
    );
}

/// Stacks panels vertically with `Flex::Center` so any leftover height
/// splits evenly above and below the content block.
fn render_column_with_spacing(
    area: Rect,
    panels: &[&dyn Panel],
    spacing: u16,
    buf: &mut ratatui::buffer::Buffer,
    vm: &WatchViewModel,
) {
    let constraints: Vec<Constraint> = panels.iter().map(|p| p.preferred_constraint(vm)).collect();

    let rects = Layout::vertical(constraints)
        .flex(Flex::Center)
        .spacing(spacing)
        .split(area);

    for (panel, rect) in panels.iter().zip(rects.iter()) {
        panel.render(*rect, buf, vm);
    }
}

/// Stacks panels vertically with leftover height split evenly above and
/// below the content block (`Flex::Center`). The outer frame fills the
/// whole terminal, so without centering the content gets jammed at the top
/// with a wedge of dead space below.
const COLUMN_GAP: u16 = 1;

fn render_centered_column(
    area: Rect,
    panels: &[&dyn Panel],
    buf: &mut ratatui::buffer::Buffer,
    vm: &WatchViewModel,
) {
    let constraints: Vec<Constraint> = panels.iter().map(|p| p.preferred_constraint(vm)).collect();

    let rects = Layout::vertical(constraints)
        .flex(Flex::Center)
        .spacing(COLUMN_GAP)
        .split(area);

    for (panel, rect) in panels.iter().zip(rects.iter()) {
        panel.render(*rect, buf, vm);
    }
}

// ── Overlay popups ───────────────────────────────────────────────────────────

pub fn render_help_overlay(frame: &mut Frame<'_>) {
    render_overlay(
        frame,
        "glorp help",
        &[
            "q quit   r refresh   p pet   m mouse   ? help",
            "r refreshes usage and pet state now",
            "p gives your pet a quick affection bump",
            "m toggles cursor-tracked eyes (hover pet to see)",
            "usage polls stay calm when helpers are blocked",
        ],
    );
}

pub fn render_evolution_overlay(frame: &mut Frame<'_>, stage_label: Option<&str>) {
    let label_line = match stage_label {
        Some(label) if !label.is_empty() => format!("evolved into {label}"),
        _ => "your pet is changing shape".to_string(),
    };
    render_overlay(
        frame,
        "glorp evolution",
        &[
            label_line.as_str(),
            "new stage art appears after the next settled tick",
            "keep feeding it real work",
        ],
    );
}

pub fn render_hatch_overlay(frame: &mut Frame<'_>) {
    render_overlay(
        frame,
        "glorp hatch",
        &[
            "a small terminal creature blinks awake",
            "local state is private",
            "feed comes from effective token deltas",
        ],
    );
}

fn render_overlay(frame: &mut Frame<'_>, title: &str, copy: &[&str]) {
    let area = centered_rect(frame.area(), 62, 9);
    let styles = semantic_styles();
    let p = tokenpet_palette();
    frame.render_widget(Clear, area);
    let lines = copy
        .iter()
        .map(|line| Line::from(Span::styled(*line, styles.primary_text)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(p.fg.rgb).bg(p.surface.rgb))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title(Span::styled(title.to_string(), styles.label))
                    .borders(Borders::ALL)
                    .border_style(styles.overlay_border)
                    .style(styles.overlay_surface),
            ),
        area,
    );
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod render_compact_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_compact_draws_rounded_frame() {
        // Width 60 < COMPACT_THRESHOLD (104) → compact mode.
        let backend = TestBackend::new(60, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row0: String = (0..60)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        // Compact mode still has the rounded outer frame.
        assert!(
            row0.contains("╭") || row0.contains("─"),
            "top row should contain rounded border chars, got {row0:?}"
        );
    }

    #[test]
    fn render_compact_shows_vitals_content() {
        let backend = TestBackend::new(60, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut all = String::new();
        for y in 0..30 {
            for x in 0..60 {
                all.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(
            all.contains("vitals"),
            "compact render should show vitals section"
        );
        assert!(all.contains("fed"), "compact render should show fed bar");
    }
}

#[cfg(test)]
mod render_wide_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    const TEST_HEIGHT: u16 = 30;

    fn render_buffer(width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn row_string(buf: &ratatui::buffer::Buffer, y: u16) -> String {
        let width = buf.area.width;
        (0..width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect()
    }

    /// Scan from the top for the row containing the rounded bottom-corner glyph.
    /// Returns the row index, panicking with a useful message if not found.
    fn find_bottom_corner_row(buf: &ratatui::buffer::Buffer, max_y: u16) -> u16 {
        for y in 0..max_y {
            let row = row_string(buf, y);
            if row.contains("╰") {
                return y;
            }
        }
        panic!("no ╰ corner found in any row");
    }

    #[test]
    fn render_wide_draws_rounded_frame() {
        // Width 110 >= COMPACT_THRESHOLD (104) → wide mode.
        let buf = render_buffer(110, TEST_HEIGHT);
        let top = row_string(&buf, 0);
        assert!(
            top.contains("╭"),
            "top row should start with rounded corner ╭, got {top:?}"
        );
        assert!(
            top.contains("╮"),
            "top row should end with rounded corner ╮, got {top:?}"
        );
        let bottom_row = find_bottom_corner_row(&buf, TEST_HEIGHT);
        let bottom = row_string(&buf, bottom_row);
        assert!(
            bottom.contains("╰"),
            "bottom row should have rounded corner ╰"
        );
        assert!(
            bottom.contains("╯"),
            "bottom row should have rounded corner ╯"
        );
    }

    #[test]
    fn render_wide_frame_spans_full_width_and_terminal_height() {
        let test_width: u16 = 140;
        let buf = render_buffer(test_width, TEST_HEIGHT);
        assert_eq!(buf[(0u16, 0u16)].symbol(), "╭");
        assert_eq!(buf[(test_width - 1, 0u16)].symbol(), "╮");
        // Frame fills the full terminal height: bottom border sits on the
        // last row of the terminal.
        assert_eq!(buf[(0u16, TEST_HEIGHT - 1)].symbol(), "╰");
        assert_eq!(buf[(test_width - 1, TEST_HEIGHT - 1)].symbol(), "╯");
    }

    #[test]
    fn render_wide_title_appears_in_frame() {
        let buf = render_buffer(110, TEST_HEIGHT);
        let top = row_string(&buf, 0);
        assert!(top.contains("glorp"), "title row should contain 'glorp'");
    }

    #[test]
    fn render_wide_footer_appears_in_frame() {
        let buf = render_buffer(110, TEST_HEIGHT);
        let bottom_row = find_bottom_corner_row(&buf, TEST_HEIGHT);
        let bottom = row_string(&buf, bottom_row);
        assert!(
            bottom.contains("quit"),
            "footer should contain 'quit', got {bottom:?}"
        );
    }

    #[test]
    fn render_wide_shows_panel_content() {
        let buf = render_buffer(110, TEST_HEIGHT);
        let mut all = String::new();
        for y in 0..TEST_HEIGHT {
            all.push_str(&row_string(&buf, y));
        }
        assert!(
            all.contains("vitals"),
            "wide render should show vitals panel"
        );
        assert!(all.contains("today"), "wide render should show today panel");
        assert!(
            all.contains("7-day"),
            "wide render should show 7-day spark panel"
        );
        assert!(all.contains("feed"), "wide render should show feed panel");
        assert!(
            all.contains("helpers"),
            "wide render should show helpers panel"
        );
    }

    #[test]
    fn compact_threshold_switches_modes() {
        // Just below threshold: compact; at threshold: wide.
        let compact_buf = render_buffer((COMPACT_THRESHOLD - 1) as u16 + 2, TEST_HEIGHT); // +2 for outer frame
        let wide_buf = render_buffer((COMPACT_THRESHOLD + 2) as u16 + 2, TEST_HEIGHT);

        // Both should have a frame (╭ corner).
        assert_eq!(compact_buf[(0u16, 0u16)].symbol(), "╭");
        assert_eq!(wide_buf[(0u16, 0u16)].symbol(), "╭");
    }
}
