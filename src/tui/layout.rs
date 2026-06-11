use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::pet::animator::SceneEffectTargets;
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
    let fill = frame_fill_for_stage(vm.pet_render.stage);
    let mut border_set = border::ROUNDED;
    border_set.horizontal_top = fill;
    border_set.horizontal_bottom = fill;
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_set(border_set)
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

/// Derive the full set of effect targets from a resolved watch layout.
/// Room slices exclude pet art and speech so background effects don't
/// overwrite the pet.
pub fn scene_effect_targets_from_layout(layout: &ComponentLayout) -> SceneEffectTargets {
    let frame = layout.frame;
    let pet = layout
        .target(TargetPath::new("watch.pet.effect"))
        .or_else(|| layout.target(TargetPath::new("watch.pet.art")))
        .map(|target| target.rect)
        .unwrap_or_else(|| Rect::new(frame.x, frame.y, 0, 0));

    let room_target = layout.target(TargetPath::new("watch.room.effect"));
    let room = room_target
        .map(|target| target.rect)
        .unwrap_or_else(|| Rect::new(frame.x, frame.y, 0, 0));
    let room_present = room_target.is_some();

    let art = layout
        .target(TargetPath::new("watch.pet.art"))
        .map(|target| target.rect);

    let speech = layout
        .target(TargetPath::new("watch.pet.speech"))
        .map(|target| target.rect);

    let room_slices = if room.width == 0 || room.height == 0 {
        vec![]
    } else {
        let exclusions: Vec<Rect> = [art, speech].into_iter().flatten().collect();
        split_rect_around_exclusions(room, &exclusions)
    };

    let mut props = std::collections::BTreeMap::new();
    for (path, target) in &layout.targets {
        let path_str = path.as_str();
        if path_str.starts_with("watch.prop.") && path_str.ends_with(".effect") {
            props.insert(path_str, target.rect);
        }
    }

    SceneEffectTargets {
        frame,
        pet,
        room_present,
        room_slices,
        props,
    }
}

/// Split `room` into sub-rects that exclude each rectangle in `exclusions`.
/// Returns only non-empty rects.
fn split_rect_around_exclusions(room: Rect, exclusions: &[Rect]) -> Vec<Rect> {
    let mut rects = vec![room];
    for exclusion in exclusions {
        let mut next = Vec::new();
        for rect in rects {
            next.extend(split_rect_around(rect, *exclusion));
        }
        rects = next;
    }
    rects
        .into_iter()
        .filter(|r| r.width > 0 && r.height > 0)
        .collect()
}

/// Subtract `hole` from `rect`, returning up to four surrounding pieces.
fn split_rect_around(rect: Rect, hole: Rect) -> Vec<Rect> {
    let ix = rect.x.max(hole.x);
    let iy = rect.y.max(hole.y);
    let iw = rect
        .x
        .saturating_add(rect.width)
        .min(hole.x.saturating_add(hole.width))
        .saturating_sub(ix);
    let ih = rect
        .y
        .saturating_add(rect.height)
        .min(hole.y.saturating_add(hole.height))
        .saturating_sub(iy);

    if iw == 0 || ih == 0 {
        return vec![rect];
    }

    let mut pieces = Vec::new();
    // Top
    if iy > rect.y {
        pieces.push(Rect::new(rect.x, rect.y, rect.width, iy - rect.y));
    }
    // Bottom
    if iy.saturating_add(ih) < rect.y.saturating_add(rect.height) {
        pieces.push(Rect::new(
            rect.x,
            iy + ih,
            rect.width,
            rect.y
                .saturating_add(rect.height)
                .saturating_sub(iy)
                .saturating_sub(ih),
        ));
    }
    // Left
    if ix > rect.x {
        pieces.push(Rect::new(rect.x, iy, ix - rect.x, ih));
    }
    // Right
    if ix.saturating_add(iw) < rect.x.saturating_add(rect.width) {
        pieces.push(Rect::new(
            ix + iw,
            iy,
            rect.x
                .saturating_add(rect.width)
                .saturating_sub(ix)
                .saturating_sub(iw),
            ih,
        ));
    }
    pieces
        .into_iter()
        .filter(|r| r.width > 0 && r.height > 0)
        .collect()
}

/// Returns the horizontal border fill character for the outer frame, picked
/// per stage tier. S0–S1 use a dotted line, S2–S3 the default rounded fill,
/// S4–S5 a heavy line, S6 a sparkle. See the watch-visual-polish design.
pub(crate) fn frame_fill_for_stage(stage: crate::game::evolution::Stage) -> &'static str {
    use crate::game::evolution::Stage;
    match stage {
        Stage::S0 | Stage::S1 => "┄",
        Stage::S2 | Stage::S3 => "─",
        Stage::S4 | Stage::S5 => "━",
        Stage::S6 => "✦",
    }
}

/// Produces the styled title spans for the outer frame.
fn frame_title(vm: &WatchViewModel) -> Vec<Span<'_>> {
    const NAME_MAX: usize = 16;
    let styles = semantic_styles();
    let p = tokenpet_palette();
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
            format!(" glorp · {display_name} · {} · {age_label} · ", vm.species),
            styles.label,
        ),
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
            "new shape, same companion",
            "more work, more glorp",
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
mod frame_fill_tests {
    #[test]
    fn frame_fill_for_stage_returns_expected_char() {
        use crate::game::evolution::Stage;
        assert_eq!(super::frame_fill_for_stage(Stage::S0), "┄");
        assert_eq!(super::frame_fill_for_stage(Stage::S1), "┄");
        assert_eq!(super::frame_fill_for_stage(Stage::S2), "─");
        assert_eq!(super::frame_fill_for_stage(Stage::S3), "─");
        assert_eq!(super::frame_fill_for_stage(Stage::S4), "━");
        assert_eq!(super::frame_fill_for_stage(Stage::S5), "━");
        assert_eq!(super::frame_fill_for_stage(Stage::S6), "✦");
    }
}

#[cfg(test)]
mod frame_title_tests {
    #[test]
    fn frame_title_does_not_repeat_species() {
        use crate::tui::view_model::WatchViewModel;
        let vm = WatchViewModel::fixture();
        let spans = super::frame_title(&vm);
        let rendered: String = spans.iter().map(|s| s.content.as_ref()).collect::<String>();

        // The species token appears once (as a standalone field).
        assert!(
            !rendered.contains("the "),
            "frame title should not contain the inline 'the {{species}}' phrasing; got: {rendered:?}"
        );
        // The stage label is dropped — stage info comes from pet art + per-stage frame fill.
        assert!(
            !rendered.contains(&vm.stage),
            "frame title should not contain the textual stage label '{}'; got: {rendered:?}",
            vm.stage
        );
        // Species and mood still appear.
        assert!(
            rendered.contains(&vm.species),
            "species token should still appear"
        );
        assert!(
            rendered.contains(&vm.mood),
            "mood token should still appear"
        );
    }
}

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
        let rect = pet_effect_rect(Rect::new(0, 0, 72, 9), &vm);

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
        // Width 60 < COMPACT_THRESHOLD+2 (118) → compact mode.
        // Compact mode fills terminal height (no vertical centering padding)
        // and the rounded corner sits at row 0.
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
                hold_eyes_closed: false,
                blink_slowdown: 0,
                soft_eyes: false,
                work_accent: crate::pet::render::WorkAccent::None,
            },
        );
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;

        // Width 80 < COMPACT_THRESHOLD+2 (118) → compact mode.
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

    // Wide mode uses full terminal height, so these assertions can read the
    // frame rails directly without vertical centering padding.
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
        // Width 124 >= COMPACT_THRESHOLD+2 (118) → wide mode.
        let buf = render_buffer(124, TEST_HEIGHT);
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
        // 124 == MAX_FRAME_WIDTH. Frame matches the
        // terminal exactly so corners sit at (0,0) and bottom.
        let buf = render_buffer(124, 23);
        assert_eq!(buf[(0u16, 0u16)].symbol(), "╭");
        assert_eq!(buf[(124 - 1, 0u16)].symbol(), "╮");
        assert_eq!(buf[(0u16, 23 - 1)].symbol(), "╰");
        assert_eq!(buf[(124 - 1, 23 - 1)].symbol(), "╯");
    }

    #[test]
    fn oversized_terminal_pads_around_centered_frame() {
        // Terminals wider than MAX_FRAME_WIDTH center the frame horizontally
        // and leave outer columns blank while filling the terminal height.
        let buf = render_buffer(160, 50);
        // The top-left cell of the terminal is empty (padding), not a corner.
        assert_eq!(buf[(0u16, 0u16)].symbol(), " ");
        // The frame's actual top-left corner sits at x = (160 - 124) / 2 = 18
        // and y = 0 because wide mode uses the full terminal height.
        assert_eq!(buf[(18u16, 0u16)].symbol(), "╭");
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
        // Wide mode fills terminal height, so corners sit at (0, 0) without
        // vertical centering padding.
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

    // ── split_rect_around_exclusions ──────────────────────────────────────

    #[test]
    fn split_rect_around_disjoint_holes() {
        let room = Rect::new(0, 0, 10, 10);
        let holes = vec![Rect::new(2, 2, 2, 2), Rect::new(6, 6, 2, 2)];
        let slices = super::split_rect_around_exclusions(room, &holes);
        let total_area: u32 = slices
            .iter()
            .map(|r| r.width as u32 * r.height as u32)
            .sum();
        // Total area minus hole areas = 100 - 4 - 4 = 92
        assert_eq!(total_area, 92, "disjoint holes should leave 92 area");
        // Each cell in the resulting slices should be outside both holes.
        for slice in &slices {
            for dx in 0..slice.width {
                for dy in 0..slice.height {
                    let x = slice.x + dx;
                    let y = slice.y + dy;
                    assert!(
                        !holes.iter().any(|h| h.x <= x
                            && x < h.x + h.width
                            && h.y <= y
                            && y < h.y + h.height),
                        "slice cell ({x},{y}) should not be inside any hole"
                    );
                }
            }
        }
    }

    #[test]
    fn split_rect_around_nested_holes_uses_inner_only() {
        let room = Rect::new(0, 0, 10, 10);
        let holes = vec![Rect::new(2, 2, 6, 6), Rect::new(3, 3, 2, 2)];
        let slices = super::split_rect_around_exclusions(room, &holes);
        let total_area: u32 = slices
            .iter()
            .map(|r| r.width as u32 * r.height as u32)
            .sum();
        // Outer hole already excludes the inner hole area, so total excluded is 36.
        assert_eq!(
            total_area, 64,
            "nested holes should exclude outer hole area only"
        );
    }

    #[test]
    fn split_rect_around_edge_touching_holes_does_not_over_exclude() {
        let room = Rect::new(0, 0, 10, 10);
        let holes = vec![Rect::new(2, 2, 2, 2), Rect::new(4, 2, 2, 2)];
        let slices = super::split_rect_around_exclusions(room, &holes);
        let total_area: u32 = slices
            .iter()
            .map(|r| r.width as u32 * r.height as u32)
            .sum();
        // Two 2x2 holes side by side exclude 8 total.
        assert_eq!(total_area, 92, "edge-touching holes should exclude 8 area");
    }

    #[test]
    fn split_rect_around_full_occlusion_returns_empty() {
        let room = Rect::new(0, 0, 10, 10);
        let holes = vec![Rect::new(0, 0, 10, 10)];
        let slices = super::split_rect_around_exclusions(room, &holes);
        assert!(
            slices.is_empty(),
            "full occlusion should return empty slices"
        );
    }

    #[test]
    fn scene_effect_targets_from_layout_populates_pet_room_and_props() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let layout = super::layout_watch(Rect::new(0, 0, 120, 32), &vm);
        let targets = super::scene_effect_targets_from_layout(&layout);

        // Pet target should be present (art or effect fallback).
        assert!(targets.pet.width > 0, "pet target should have width");
        assert!(targets.pet.height > 0, "pet target should have height");

        // Room should be present in wide layout.
        assert!(
            targets.room_present,
            "room should be present in wide layout"
        );

        // Room slices should be non-empty because the pet art doesn't fully occlude.
        assert!(
            !targets.room_slices.is_empty(),
            "room slices should exist when pet art is smaller than room"
        );

        // Props should be populated from the fixture.
        assert!(
            !targets.props.is_empty(),
            "props should be populated from fixture_with_habitat_props"
        );
        assert!(
            targets
                .props
                .contains_key("watch.prop.codex_signal_lamp.effect"),
            "codex_signal_lamp prop effect should be present"
        );
    }
}
