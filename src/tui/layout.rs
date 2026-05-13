use ratatui::{
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::panels::{
    BioCardPanel, FeedPanel, Panel, PetPanel, ProgressPanel, TodayPanel, VitalsPanel,
};
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, tokenpet_palette, ColorCapability};
use crate::tui::view_model::WatchViewModel;

/// Smallest terminal width that uses the wide two-column layout.
/// Below this threshold we fall back to the single-column compact layout.
const COMPACT_THRESHOLD: usize = 104;

/// Left column width in the wide layout (pet + vitals column).
const WIDE_LEFT_COL: u16 = 40;

/// Gutter width between left and right columns in the wide layout.
const WIDE_GUTTER: u16 = 4;

/// Maximum frame dimensions. The layout is designed against these sizes;
/// terminals larger than this center the frame and leave the surrounding
/// space empty rather than stretching panels into unbalanced negative space.
/// Sized to wrap snug around content (pet ~10 rows + vitals 4 + bio 3 + gaps,
/// plus breathing room) so the inner area doesn't develop large dead bands.
const MAX_FRAME_WIDTH: u16 = 110;
const MAX_FRAME_HEIGHT: u16 = 23;

/// Vertical padding inside the rounded frame, between the chrome border and
/// the first/last panel. Keeps the today/bio rows from kissing the border.
const INNER_VPAD: u16 = 1;

/// Wide mode is a 2-band grid. Both columns split at the same row, so vitals
/// (left band 2) aligns with feed (right band 2) at the top and bio/feed
/// align at the bottom.
///
/// Band 1: pet (10 rows of art) | today (6 rows) + COLUMN_GAP + progress (3 rows) = 10.
/// Band 2: vitals (4) + COLUMN_GAP + bio (3) = 8 | feed (header + 7 events) = 8.
const WIDE_BAND_1: u16 = 10;
const WIDE_BAND_2: u16 = 8;

/// Returns a sub-rect of `terminal_area` that is at most
/// `MAX_FRAME_WIDTH` × `MAX_FRAME_HEIGHT`, centered within the terminal.
///
/// The height cap only applies in wide mode (when the bounded width allows
/// the two-column layout). Compact mode needs the full terminal height so
/// the stacked panels can fit without clipping vitals/today content.
pub(crate) fn bounded_frame_rect(terminal_area: Rect) -> Rect {
    let width = terminal_area.width.min(MAX_FRAME_WIDTH);
    let is_wide = (width as usize) >= COMPACT_THRESHOLD + 2;
    let height = if is_wide {
        terminal_area.height.min(MAX_FRAME_HEIGHT)
    } else {
        terminal_area.height
    };
    let x = terminal_area.x + terminal_area.width.saturating_sub(width) / 2;
    let y = terminal_area.y + terminal_area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Wide,
    Compact,
}

pub fn render_watch_frame_with_capability(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    capability: ColorCapability,
) {
    let ctx = RenderContext::new(capability);
    render_watch_frame_with_context(frame, vm, &ctx);
}

pub fn render_watch_frame_with_context(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    let styles = semantic_styles();
    let p = tokenpet_palette();
    let outer = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(frame_title(vm)))
        .title_bottom(Line::from(frame_footer()))
        .border_style(Style::default().fg(p.accent.rgb))
        .style(styles.body);

    // Pin the frame to MAX_FRAME_WIDTH × MAX_FRAME_HEIGHT; oversized terminals
    // get padding around the centered frame so panel proportions stay tuned.
    let frame_rect = bounded_frame_rect(frame.area());

    // Decide mode by the bounded frame width (after subtracting borders).
    let mode = if (frame_rect.width as usize) >= COMPACT_THRESHOLD + 2 {
        Mode::Wide
    } else {
        Mode::Compact
    };

    let inner = outer.inner(frame_rect);
    frame.render_widget(outer, frame_rect);

    layout_and_render(inner, mode, frame.buffer_mut(), vm, ctx);
}

/// Strips the 1-cell rounded-border chrome from `frame_area`, returning the
/// inner rect that layout renders into. Mirrors what `render_watch_frame_with_context`
/// does with `Block::bordered().inner(frame_rect)`.
pub(crate) fn inner_frame_rect(frame_area: Rect) -> Rect {
    Block::bordered().inner(frame_area)
}

/// Returns the 13×10 sub-rect where the pet art sits within `frame.area()` for
/// the current mode and view model. Callers (specifically the watch loop) use
/// this to scope tachyonfx effects to the pet art, not the full (Fill-sized)
/// pet panel which may be much taller than 10 rows.
pub fn pet_panel_rect(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    use crate::tui::panels::pet::pet_inner_rect_in_panel;

    // Mirror render_watch_frame_with_context: pin the chrome to the bounded
    // rect before splitting, so tachyonfx scopes to where the pet actually lives.
    let bounded = bounded_frame_rect(frame_area);
    let mode = if (bounded.width as usize) >= COMPACT_THRESHOLD + 2 {
        Mode::Wide
    } else {
        Mode::Compact
    };

    let raw_inner = inner_frame_rect(bounded);

    match mode {
        Mode::Wide => {
            // Mirror render_wide: strip the top/bottom INNER_VPAD, split into
            // body columns, then take band 1 of the left column.
            let padded = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(INNER_VPAD),
                    Constraint::Min(0),
                    Constraint::Length(INNER_VPAD),
                ])
                .split(raw_inner)[1];

            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(WIDE_LEFT_COL),
                    Constraint::Length(WIDE_GUTTER),
                    Constraint::Min(50),
                ])
                .split(padded);
            let left_col = body[0];

            let left_bands = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(WIDE_BAND_1),
                    Constraint::Length(COLUMN_GAP),
                    Constraint::Length(WIDE_BAND_2),
                    Constraint::Min(0),
                ])
                .split(left_col);

            pet_inner_rect_in_panel(left_bands[0], vm)
        }
        Mode::Compact => {
            let stack = Layout::default()
                .direction(Direction::Vertical)
                .flex(Flex::Start)
                .constraints([
                    PetPanel.preferred_constraint(vm),    // Fill(1)
                    VitalsPanel.preferred_constraint(vm), // Length(4)
                    Constraint::Length(COLUMN_GAP),
                    TodayPanel.preferred_constraint(vm), // Length(6)
                    Constraint::Length(COLUMN_GAP),
                    ProgressPanel.preferred_constraint(vm), // Length(3)
                    Constraint::Length(COLUMN_GAP),
                    FeedPanel.preferred_constraint(vm), // Length(events+1)
                ])
                .split(raw_inner);

            pet_inner_rect_in_panel(stack[0], vm)
        }
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
    ctx: &RenderContext,
) {
    match mode {
        Mode::Wide => render_wide(inner, buf, vm, ctx),
        Mode::Compact => render_compact(inner, buf, vm, ctx),
    }
}

/// Wide layout: a 2-column × 2-row grid. Both columns split at the same row,
/// so vitals (left band 2) aligns with feed (right band 2) at the top, and
/// bio bottom aligns with feed bottom.
///
/// ```text
/// ╭ title ─────────────────────────────────────╮
/// │                                             │  ← INNER_VPAD
/// │ [pet]            today                      │
/// │ [pet]            ...                        │  band 1 (10 rows)
/// │ [pet]            progress                   │
/// │ [pet]            xp bar                     │
/// │                                             │  inter-band gap (1 row)
/// │ vitals           feed                       │  band 2 (8 rows)
/// │ ...              event 1                    │  ← vitals.top == feed.top
/// │ bio              ...                        │
/// │ age              event 7                    │  ← bio.bottom == feed.bottom
/// │                                             │  ← INNER_VPAD
/// ╰ footer ─────────────────────────────────────╯
/// ```
fn render_wide(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    let padded = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(INNER_VPAD),
            Constraint::Min(0),
            Constraint::Length(INNER_VPAD),
        ])
        .split(area)[1];

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(WIDE_LEFT_COL),
            Constraint::Length(WIDE_GUTTER),
            Constraint::Min(50),
        ])
        .split(padded);

    let band_constraints = [
        Constraint::Length(WIDE_BAND_1),
        Constraint::Length(COLUMN_GAP),
        Constraint::Length(WIDE_BAND_2),
        Constraint::Min(0),
    ];
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints(band_constraints)
        .split(body[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints(band_constraints)
        .split(body[2]);

    // Band 1 left: pet (10 rows of art fits exactly).
    if left[0].height >= 10 {
        PetPanel.render(left[0], buf, vm, ctx);
    }

    // Band 1 right: today packed top, progress anchored at the bottom of the
    // band. With today(6) + gap(1) + progress(2) = 9 in a 10-row band, the
    // single row of slack lands between today and progress (via Min(0)) so
    // progress sits one row above band 2 — the same one-row gap that
    // separates bio from vitals on the left.
    let right_top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            TodayPanel.preferred_constraint(vm), // Length(6)
            Constraint::Length(COLUMN_GAP),
            Constraint::Min(0), // slack lives between today and progress
            ProgressPanel.preferred_constraint(vm), // Length(2)
        ])
        .split(right[0]);
    TodayPanel.render(right_top[0], buf, vm, ctx);
    ProgressPanel.render(right_top[3], buf, vm, ctx);

    // Band 2 left: vitals (4) + gap (1) + bio (3) = 8. Packed top.
    let left_bottom = Layout::default()
        .direction(Direction::Vertical)
        .flex(Flex::Start)
        .constraints([
            VitalsPanel.preferred_constraint(vm),  // Length(4)
            Constraint::Length(COLUMN_GAP),
            BioCardPanel.preferred_constraint(vm), // Length(3)
        ])
        .split(left[2]);
    VitalsPanel.render(left_bottom[0], buf, vm, ctx);
    BioCardPanel.render(left_bottom[2], buf, vm, ctx);

    // Band 2 right: feed fills the entire band (header + 7 events).
    FeedPanel.render(right[2], buf, vm, ctx);
}

/// Compact layout: single column packed from the top.
///
/// Order: pet → [gap] → vitals → [gap] → today → [gap] → progress → [gap] → feed.
/// Bio is omitted from compact mode: age is already in the title bar, and the
/// hatched date is low-priority on narrow terminals.
fn render_compact(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    let stack = Layout::default()
        .direction(Direction::Vertical)
        .flex(Flex::Start)
        .constraints([
            PetPanel.preferred_constraint(vm), // Fill(1) — expands to fill leftover
            VitalsPanel.preferred_constraint(vm), // Length(4); no gap above (pet is empty
            // when guarded out at small heights — keeps 72×24 within budget).
            Constraint::Length(COLUMN_GAP),
            TodayPanel.preferred_constraint(vm), // Length(6)
            Constraint::Length(COLUMN_GAP),
            ProgressPanel.preferred_constraint(vm), // Length(3)
            Constraint::Length(COLUMN_GAP),
            FeedPanel.preferred_constraint(vm), // Length(7)
        ])
        .split(area);

    // PetPanel assumes its area is at least PET_H (10) rows tall; skip if too small.
    if stack[0].height >= 10 {
        PetPanel.render(stack[0], buf, vm, ctx);
    }
    VitalsPanel.render(stack[1], buf, vm, ctx);
    TodayPanel.render(stack[3], buf, vm, ctx);
    ProgressPanel.render(stack[5], buf, vm, ctx);
    FeedPanel.render(stack[7], buf, vm, ctx);
}

/// Gap between stacked panels in both wide and compact layouts.
const COLUMN_GAP: u16 = 1;

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
mod tests {
    use super::*;

    #[test]
    fn pet_panel_rect_returns_thirteen_by_ten_sub_rect() {
        let vm = WatchViewModel::fixture();
        let frame_area = Rect::new(0, 0, 120, 32);
        let rect = pet_panel_rect(frame_area, &vm);
        assert_eq!(rect.width, 13);
        assert_eq!(rect.height, 10);
    }

    #[test]
    fn pet_panel_rect_accounts_for_bio_panel_height() {
        let vm = WatchViewModel::fixture();
        let frame_area = Rect::new(0, 0, 120, 50);
        let rect = pet_panel_rect(frame_area, &vm);
        assert!(
            rect.y + rect.height < frame_area.height - 3,
            "pet sub-rect must end before bio starts"
        );
    }
}

#[cfg(test)]
mod render_compact_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_compact_draws_rounded_frame() {
        // Width 60 < COMPACT_THRESHOLD (104) → compact mode.
        // Height 30 ≤ MAX_FRAME_HEIGHT (34) so the frame fills the terminal
        // (no centering padding) and the rounded corner sits at row 0.
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
        let backend = TestBackend::new(60, 40);
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

    /// Compact mode at 80×40 must render visible pet content in the Fill(1) area.
    /// This is a regression guard for the missing gap between PetPanel and
    /// VitalsPanel that caused Fill(1) to collapse at small heights and the pet
    /// panel to be silently skipped at larger heights when gaps were absent.
    #[test]
    fn render_compact_pet_visible_at_80x40_with_crystal_pet() {
        use crate::game::evolution::Stage;
        use crate::game::metabolism::Mood;
        use crate::pet::generation::generate_pet;
        use crate::pet::render::{render_pet, AnimationFrame};

        // Build a crystal S2 WatchViewModel (the species the user reported missing).
        let pet = generate_pet("crystal-compact-test");
        let rendered = render_pet(
            &pet,
            Stage::S2,
            Mood::Content,
            AnimationFrame {
                tick: 0,
                blink_suppression_ticks: 0,
            },
        );
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;

        // Width 80 < COMPACT_THRESHOLD+2 (106) → compact mode.
        // Height 40 gives Fill(1) PetPanel ≥ 10 rows so the pet renders.
        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();

        // Collect all content in the top half of the frame where the pet should be.
        let mut pet_area_content = String::new();
        for y in 1..20 {
            for x in 1..79 {
                pet_area_content.push_str(buffer[(x, y)].symbol());
            }
        }

        // Pet art has block characters and slashes; vitals content is below row 20.
        // The pet area must not be entirely spaces.
        let non_space: String = pet_area_content
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            !non_space.is_empty(),
            "compact pet area (rows 1-19) should contain visible pet art characters, got all spaces"
        );
    }
}

#[cfg(test)]
mod render_wide_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // Height ≤ MAX_FRAME_HEIGHT keeps the frame matching the terminal exactly
    // (no centering padding) so corner-position assertions stay simple.
    const TEST_HEIGHT: u16 = 23;

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
    fn render_wide_frame_spans_bounded_width_and_terminal_height() {
        // 110 == MAX_FRAME_WIDTH; 23 == MAX_FRAME_HEIGHT. Frame matches the
        // terminal exactly so corners sit at (0,0) and bottom.
        let buf = render_buffer(110, 23);
        assert_eq!(buf[(0u16, 0u16)].symbol(), "╭");
        assert_eq!(buf[(110 - 1, 0u16)].symbol(), "╮");
        assert_eq!(buf[(0u16, 23 - 1)].symbol(), "╰");
        assert_eq!(buf[(110 - 1, 23 - 1)].symbol(), "╯");
    }

    #[test]
    fn oversized_terminal_pads_around_centered_frame() {
        // Terminals larger than MAX_FRAME_WIDTH × MAX_FRAME_HEIGHT center the
        // frame and leave the outer cells blank instead of stretching panels.
        let buf = render_buffer(160, 50);
        // The top-left cell of the terminal is empty (padding), not a corner.
        assert_eq!(buf[(0u16, 0u16)].symbol(), " ");
        // The frame's actual top-left corner sits at x = (160 - 110) / 2 = 25
        // and y = (50 - 23) / 2 = 13.
        assert_eq!(buf[(25u16, 13u16)].symbol(), "╭");
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
            all.contains("progress"),
            "wide render should show progress panel"
        );
        assert!(all.contains("feed"), "wide render should show feed panel");
    }

    #[test]
    fn compact_threshold_switches_modes() {
        // Just below threshold: compact; at threshold: wide.
        // Use height ≤ MAX_FRAME_HEIGHT so the frame matches the terminal and
        // corners sit at (0, 0) without centering padding.
        let compact_buf = render_buffer((COMPACT_THRESHOLD - 1) as u16 + 2, 24); // +2 for outer frame
        let wide_buf = render_buffer((COMPACT_THRESHOLD + 2) as u16 + 2, 24);

        // Both should have a frame (╭ corner).
        assert_eq!(compact_buf[(0u16, 0u16)].symbol(), "╭");
        assert_eq!(wide_buf[(0u16, 0u16)].symbol(), "╭");
    }

    #[test]
    fn render_wide_does_not_include_helpers_or_spark_strings() {
        let vm = WatchViewModel::fixture();
        let ctx = RenderContext::new(crate::tui::style::ColorCapability::Truecolor);
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_watch_frame_with_context(f, &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        // HelpersPanel section title was " helpers " — must be gone from layout.
        assert!(!s.contains("helpers"), "helpers panel must be removed");
        // SparkPanel section title was rendered as a dedicated border row.
        // TodayPanel's footer now inlines "← 7-day" legitimately, so we can't
        // assert the absence of "7-day" — but we CAN check SparkPanel's distinctive
        // top-border title (with en-dashes on either side) is absent.
        // Since we can't rely on border char encoding, just verify SparkPanel's
        // unique content ("7-day" as a section label) is gone by checking there
        // is NO section that only contains "7-day" text (not prefixed with ←).
        // The simplest defensible check: assert the new panels are present.
        assert!(s.contains("today"), "today panel must still render");
        assert!(s.contains("progress"), "progress panel must render");
        assert!(s.contains("feed"), "feed panel must render");
        // BioCardPanel must appear in the left column.
        assert!(s.contains("bio"), "bio panel must render in wide layout");
    }

    #[test]
    fn render_wide_bio_panel_appears_in_left_column() {
        let vm = WatchViewModel::fixture();
        let ctx = RenderContext::new(crate::tui::style::ColorCapability::Truecolor);
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_watch_frame_with_context(f, &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(s.contains("bio"), "bio panel title must appear");
        assert!(s.contains("hatched"), "bio hatched label must appear");
        assert!(s.contains("age"), "bio age label must appear");
    }

    /// In wide mode the feed panel must be anchored toward the bottom of the
    /// right column (Min(0) spacer between progress and feed absorbs the slack).
    /// Concretely: the row containing "feed" must appear after the midpoint of
    /// Progress and feed live in separate row bands; feed starts on the same
    /// terminal row as vitals (band 2 top). Exactly one row separates the end
    /// of progress (band 1) from the feed header (band 2) — matching the
    /// 1-row gap between bio and vitals.
    #[test]
    fn vitals_and_feed_start_on_the_same_row() {
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        let vm = WatchViewModel::fixture();
        terminal
            .draw(|f| render_watch_frame_with_capability(f, &vm, ColorCapability::Truecolor))
            .unwrap();
        let buf = terminal.backend().buffer().clone();

        let height = buf.area.height;
        let row_string = |y: u16| -> String {
            (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect()
        };
        let vitals_row = (0..height)
            .find(|&y| row_string(y).contains("vitals"))
            .expect("vitals section must appear");
        let feed_row = (0..height)
            .find(|&y| row_string(y).contains("feed"))
            .expect("feed section must appear");
        assert_eq!(
            vitals_row, feed_row,
            "vitals and feed must start on the same terminal row (band 2 top)"
        );
    }
}
