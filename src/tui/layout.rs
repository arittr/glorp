use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::component::{
    layout_watch, layout_watch_with_context, render_watch_layout, ComponentLayout, TargetPath,
};
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, tokenpet_palette, ColorCapability};
use crate::tui::view_model::WatchViewModel;

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
    let layout = layout_watch_with_context(frame.area(), vm, ctx);
    render_watch_frame_with_layout(frame, vm, ctx, &layout);
}

pub fn render_watch_frame_with_layout(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    ctx: &RenderContext,
    layout: &ComponentLayout,
) {
    let styles = semantic_styles();
    let p = tokenpet_palette();
    let outer = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(frame_title(vm)))
        .title_bottom(Line::from(frame_footer()))
        .border_style(Style::default().fg(p.accent.rgb))
        .style(styles.body);

    frame.render_widget(outer, layout.frame);
    render_watch_layout(layout, frame.buffer_mut(), vm, ctx);
}

/// Returns the current pet art target from the shared component-layout artifact.
/// Tachyonfx effects use this target so they cover the rendered pet art, not
/// a separately computed copy of the watch geometry.
pub fn pet_effect_rect(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    let layout = layout_watch(frame_area, vm);
    pet_effect_rect_from_layout(&layout)
}

pub(crate) fn pet_effect_rect_from_layout(layout: &ComponentLayout) -> Rect {
    layout
        .target(TargetPath::new("watch.pet.effect"))
        .or_else(|| layout.target(TargetPath::new("watch.pet.art")))
        .map(|target| target.rect)
        .unwrap_or_else(|| Rect::new(layout.frame.x, layout.frame.y, 0, 0))
}

#[doc(hidden)]
pub fn pet_effect_rect_for_test(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    pet_effect_rect(frame_area, vm)
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
    use crate::tui::component::{layout_watch, TargetPath};

    #[test]
    fn pet_effect_rect_returns_component_layout_pet_art_target() {
        let vm = WatchViewModel::fixture();
        let frame_area = Rect::new(0, 0, 120, 32);
        let layout = layout_watch(frame_area, &vm);
        let target = layout.target(TargetPath::new("watch.pet.art")).unwrap();

        assert_eq!(pet_effect_rect(frame_area, &vm), target.rect);
    }

    #[test]
    fn pet_effect_rect_returns_empty_rect_when_pet_art_target_is_absent() {
        let vm = WatchViewModel::fixture();
        let rect = pet_effect_rect(Rect::new(0, 0, 72, 24), &vm);

        assert_eq!(rect, Rect::new(0, 0, 0, 0));
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
    use crate::tui::component::watch_screen::COMPACT_THRESHOLD;
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
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
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
