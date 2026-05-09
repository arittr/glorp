use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenpetColor {
    pub name: &'static str,
    pub source_oklch: &'static str,
    pub rgb: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenpetPalette {
    pub bg: TokenpetColor,
    pub surface: TokenpetColor,
    pub fg: TokenpetColor,
    pub dim: TokenpetColor,
    pub faint: TokenpetColor,
    pub accent: TokenpetColor,
    pub good: TokenpetColor,
    pub bad: TokenpetColor,
}

pub fn tokenpet_palette() -> TokenpetPalette {
    TokenpetPalette {
        bg: TokenpetColor {
            name: "bg",
            source_oklch: "oklch(0.18 0.005 60)",
            rgb: Color::Rgb(0x13, 0x11, 0x0f),
        },
        surface: TokenpetColor {
            name: "surface",
            source_oklch: "oklch(0.22 0.006 60)",
            rgb: Color::Rgb(0x1d, 0x1a, 0x18),
        },
        fg: TokenpetColor {
            name: "fg",
            source_oklch: "oklch(0.94 0.01 80)",
            rgb: Color::Rgb(0xef, 0xeb, 0xe4),
        },
        dim: TokenpetColor {
            name: "dim",
            source_oklch: "oklch(0.66 0.012 70)",
            rgb: Color::Rgb(0x97, 0x91, 0x8a),
        },
        faint: TokenpetColor {
            name: "faint",
            source_oklch: "oklch(0.42 0.008 60)",
            rgb: Color::Rgb(0x50, 0x4c, 0x49),
        },
        accent: TokenpetColor {
            name: "accent",
            source_oklch: "oklch(0.78 0.14 70)",
            rgb: Color::Rgb(0xf0, 0xa6, 0x46),
        },
        good: TokenpetColor {
            name: "good",
            source_oklch: "oklch(0.74 0.10 145)",
            rgb: Color::Rgb(0x82, 0xbc, 0x83),
        },
        bad: TokenpetColor {
            name: "bad",
            source_oklch: "oklch(0.68 0.16 25)",
            rgb: Color::Rgb(0xea, 0x6a, 0x64),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Normal,
    Usage,
    Diagnostic,
    Evolution,
    Help,
}

#[derive(Debug, Clone)]
pub struct SemanticStyles {
    pub body: Style,
    pub chrome_title: Style,
    pub prompt_user: Style,
    pub prompt_path: Style,
    pub prompt_sep: Style,
    pub section_header: Style,
    pub timestamp: Style,
    pub primary_text: Style,
    pub label: Style,
    pub empty_bar: Style,
    pub filled_bar_good: Style,
    pub filled_bar_accent: Style,
    pub event_rail_usage: Style,
    pub event_rail_diagnostic: Style,
    pub event_rail_evolution: Style,
    pub sparkline_today: Style,
    pub sparkline_past: Style,
    pub overlay_border: Style,
    pub overlay_surface: Style,
    pub blocked: Style,
}

impl SemanticStyles {
    pub fn log(&self, kind: LogKind) -> Style {
        let p = tokenpet_palette();
        match kind {
            LogKind::Normal => Style::default().fg(p.dim.rgb),
            LogKind::Usage => Style::default().fg(p.good.rgb),
            LogKind::Diagnostic => Style::default().fg(p.bad.rgb),
            LogKind::Evolution => Style::default()
                .fg(p.accent.rgb)
                .add_modifier(Modifier::BOLD),
            LogKind::Help => Style::default().fg(p.dim.rgb),
        }
    }
}

pub fn semantic_styles() -> SemanticStyles {
    let p = tokenpet_palette();
    SemanticStyles {
        body: Style::default().fg(p.fg.rgb).bg(p.bg.rgb),
        chrome_title: Style::default().fg(p.dim.rgb).bg(p.surface.rgb),
        prompt_user: Style::default().fg(p.good.rgb),
        prompt_path: Style::default().fg(p.accent.rgb),
        prompt_sep: Style::default().fg(p.faint.rgb),
        section_header: Style::default().fg(p.faint.rgb),
        timestamp: Style::default().fg(p.faint.rgb),
        primary_text: Style::default().fg(p.fg.rgb),
        label: Style::default().fg(p.dim.rgb),
        empty_bar: Style::default().fg(p.faint.rgb),
        filled_bar_good: Style::default().fg(p.good.rgb),
        filled_bar_accent: Style::default().fg(p.accent.rgb),
        event_rail_usage: Style::default().fg(p.good.rgb),
        event_rail_diagnostic: Style::default().fg(p.bad.rgb),
        event_rail_evolution: Style::default().fg(p.accent.rgb),
        sparkline_today: Style::default()
            .fg(p.accent.rgb)
            .add_modifier(Modifier::BOLD),
        sparkline_past: Style::default().fg(p.faint.rgb),
        overlay_border: Style::default().fg(p.accent.rgb).bg(p.bg.rgb),
        overlay_surface: Style::default().fg(p.fg.rgb).bg(p.surface.rgb),
        blocked: Style::default().fg(p.bad.rgb).add_modifier(Modifier::BOLD),
    }
}
