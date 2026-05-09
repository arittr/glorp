use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::tui::{
    style::{semantic_styles, tokenpet_palette, LogKind, SemanticStyles},
    view_model::{EventView, WatchViewModel},
};

const COMPACT_WIDTH: u16 = 72;
const BAR_WIDTH: usize = 20;
const PET_PANEL_LINES: u16 = 10;

pub fn render_watch_frame(frame: &mut Frame<'_>, vm: &WatchViewModel) {
    let area = frame.area();
    let p = tokenpet_palette();
    frame.render_widget(Block::default().style(Style::default().bg(p.bg.rgb)), area);

    if area.height == 0 || area.width == 0 {
        return;
    }

    let styles = semantic_styles();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    render_chrome(frame, chunks[0], vm, &styles);
    render_footer(frame, chunks[2], &styles);

    if area.width < COMPACT_WIDTH {
        render_compact(frame, chunks[1], vm, &styles);
    } else {
        render_wide(frame, chunks[1], vm, &styles);
    }
}

pub fn render_help_overlay(frame: &mut Frame<'_>) {
    render_overlay(
        frame,
        "glorp help",
        &[
            "q quit   r refresh   ? help",
            "p affection pulse",
            "usage polls stay calm when helpers are blocked",
        ],
    );
}

pub fn render_evolution_overlay(frame: &mut Frame<'_>) {
    render_overlay(
        frame,
        "glorp evolution",
        &[
            "your pet is changing shape",
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

fn render_wide(frame: &mut Frame<'_>, area: Rect, vm: &WatchViewModel, styles: &SemanticStyles) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(47), Constraint::Percentage(53)])
        .split(area);
    render_pet_panel(frame, columns[0], vm, styles);
    render_activity_panel(frame, columns[1], vm, styles);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, vm: &WatchViewModel, styles: &SemanticStyles) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(PET_PANEL_LINES), Constraint::Min(3)])
        .split(area);
    render_pet_panel(frame, rows[0], vm, styles);
    render_activity_panel(frame, rows[1], vm, styles);
}

fn render_chrome(frame: &mut Frame<'_>, area: Rect, vm: &WatchViewModel, styles: &SemanticStyles) {
    let title = format!(
        "glorp -- {}@claude:~ -- {}x{}",
        vm.pet_name,
        frame.area().width,
        frame.area().height
    );
    let mut spans = Vec::new();
    if area.width >= 18 {
        spans.push(Span::styled("● ", styles.event_rail_diagnostic));
        spans.push(Span::styled("● ", styles.filled_bar_accent));
        spans.push(Span::styled("●  ", styles.event_rail_usage));
    }
    spans.push(Span::styled(title, styles.chrome_title));
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .style(styles.chrome_title)
            .alignment(Alignment::Center),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, styles: &SemanticStyles) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("q", styles.prompt_user),
            Span::styled(" quit  ", styles.label),
            Span::styled("r", styles.prompt_path),
            Span::styled(" refresh  ", styles.label),
            Span::styled("?", styles.prompt_path),
            Span::styled(" help", styles.label),
        ])),
        area,
    );
}

fn render_pet_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &WatchViewModel,
    styles: &SemanticStyles,
) {
    let mut lines = vec![
        section_line("vitals", styles),
        Line::from(vec![
            Span::styled(vm.pet_name.as_str(), styles.prompt_user),
            Span::styled(" / ", styles.prompt_sep),
            Span::styled(vm.species.as_str(), styles.label),
            Span::styled(" / ", styles.prompt_sep),
            Span::styled(vm.stage.as_str(), styles.prompt_path),
        ]),
    ];

    for art in vm.pet_art.iter().take(3) {
        lines.push(Line::from(Span::styled(art.as_str(), styles.primary_text)));
    }

    lines.push(Line::from(vec![
        Span::styled("mood ", styles.label),
        Span::styled(vm.mood.as_str(), styles.primary_text),
        Span::styled(" age ", styles.label),
        Span::styled(format!("{}d", vm.age_days), styles.primary_text),
    ]));
    lines.push(bar_line("fed", vm.fed, styles.filled_bar_good, styles));
    lines.push(bar_line(
        "happy",
        vm.happiness,
        styles.filled_bar_accent,
        styles,
    ));
    lines.push(bar_line(
        "energy",
        vm.energy,
        styles.filled_bar_good,
        styles,
    ));
    let xp = if vm.xp_target <= 0.0 {
        0.0
    } else {
        vm.xp_current / vm.xp_target
    };
    lines.push(bar_line("xp", xp, styles.filled_bar_accent, styles));

    render_lines(frame, area, lines, styles);
}

fn render_activity_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    vm: &WatchViewModel,
    styles: &SemanticStyles,
) {
    let mut lines = vec![section_line("today", styles)];

    if vm.is_blocked() {
        lines.push(Line::from(vec![
            Span::styled("blocked", styles.blocked),
            Span::styled(" / ", styles.prompt_sep),
            Span::styled(vm.helper_status.as_str(), styles.primary_text),
        ]));
        for error in vm.errors.iter().take(2) {
            lines.push(Line::from(Span::styled(
                error.as_str(),
                styles.log(LogKind::Diagnostic),
            )));
        }
    } else {
        lines.push(Line::from(vec![
            Span::styled("effective ", styles.label),
            Span::styled(
                format_tokens(vm.today_effective_tokens),
                styles.primary_text,
            ),
            Span::styled(" bucket ", styles.label),
            Span::styled(
                format_tokens(vm.current_bucket_effective_tokens),
                styles.prompt_path,
            ),
        ]));
        lines.push(sparkline_line(&vm.recent_daily_effective_tokens, styles));
        lines.push(Line::from(vec![
            Span::styled("helper ", styles.label),
            Span::styled(vm.helper_status.as_str(), styles.primary_text),
        ]));
    }

    for source in vm.source_breakdown.iter().take(3) {
        lines.push(Line::from(vec![
            Span::styled(source.name.as_str(), styles.label),
            Span::styled(" ", styles.prompt_sep),
            Span::styled(format_tokens(source.effective_tokens), styles.primary_text),
        ]));
    }

    lines.push(section_line("events", styles));
    for event in vm.recent_events.iter().rev().take(5).rev() {
        lines.push(event_line(event, styles));
    }

    render_lines(frame, area, lines, styles);
}

fn render_lines(frame: &mut Frame<'_>, area: Rect, lines: Vec<Line<'_>>, styles: &SemanticStyles) {
    let p = tokenpet_palette();
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(p.fg.rgb).bg(p.bg.rgb))
            .block(Block::default().style(styles.overlay_surface)),
        area,
    );
}

fn section_line<'a>(title: &'a str, styles: &'a SemanticStyles) -> Line<'a> {
    Line::from(vec![
        Span::styled("─ ", styles.section_header),
        Span::styled(title, styles.section_header),
        Span::styled(" ─ ─ ─", styles.section_header),
    ])
}

fn bar_line<'a>(
    label: &'a str,
    ratio: f64,
    filled_style: ratatui::style::Style,
    styles: &'a SemanticStyles,
) -> Line<'a> {
    let filled = (ratio.clamp(0.0, 1.0) * BAR_WIDTH as f64).round() as usize;
    let empty = BAR_WIDTH.saturating_sub(filled);
    Line::from(vec![
        Span::styled(format!("{label:<6}"), styles.label),
        Span::styled("█".repeat(filled), filled_style),
        Span::styled("░".repeat(empty), styles.empty_bar),
    ])
}

fn event_line<'a>(event: &'a EventView, styles: &'a SemanticStyles) -> Line<'a> {
    let rail_style = match event.kind {
        LogKind::Usage => styles.event_rail_usage,
        LogKind::Diagnostic => styles.event_rail_diagnostic,
        LogKind::Evolution | LogKind::Help => styles.event_rail_evolution,
        LogKind::Normal => styles.section_header,
    };
    Line::from(vec![
        Span::styled("▏", rail_style),
        Span::raw(" "),
        Span::styled(event.timestamp.as_str(), styles.timestamp),
        Span::raw(" "),
        Span::styled(event.text.as_str(), styles.log(event.kind)),
    ])
}

fn sparkline_line<'a>(values: &'a [f64], styles: &'a SemanticStyles) -> Line<'a> {
    let max = values.iter().copied().fold(0.0, f64::max).max(1.0);
    let glyphs = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let mut spans = vec![Span::styled("7d ", styles.label)];
    for (index, value) in values.iter().enumerate() {
        let bucket = (((*value / max) * 7.0).round() as usize).min(7);
        let style = if index + 1 == values.len() {
            styles.sparkline_today
        } else {
            styles.sparkline_past
        };
        spans.push(Span::styled(glyphs[bucket], style));
    }
    Line::from(spans)
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
                    .title(Span::styled(title.to_string(), styles.prompt_path))
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

fn format_tokens(value: f64) -> String {
    if value.abs() >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}
