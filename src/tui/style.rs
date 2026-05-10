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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    Truecolor,
    Flat,
}

impl ColorCapability {
    pub fn detect() -> Self {
        Self::detect_from(|k| std::env::var(k).ok())
    }

    pub fn detect_from<F>(read: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        if read("NO_COLOR").is_some() {
            return ColorCapability::Flat;
        }
        if matches!(read("TERM").as_deref(), Some("dumb")) {
            return ColorCapability::Flat;
        }
        match read("COLORTERM").as_deref() {
            Some("truecolor") | Some("24bit") => ColorCapability::Truecolor,
            _ => ColorCapability::Flat,
        }
    }
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
    pub section_header: Style,
    pub timestamp: Style,
    pub primary_text: Style,
    pub label: Style,
    pub empty_bar: Style,
    pub event_rail_usage: Style,
    pub event_rail_diagnostic: Style,
    pub event_rail_evolution: Style,
    pub sparkline_today: Style,
    pub sparkline_past: Style,
    pub overlay_border: Style,
    pub overlay_surface: Style,
    pub blocked: Style,
    pub pet_body: Style,
    pub pet_eye: Style,
    pub pet_mouth: Style,
    pub pet_accent: Style,
    pub pet_pattern: Style,
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
        section_header: Style::default().fg(p.faint.rgb),
        timestamp: Style::default().fg(p.faint.rgb),
        primary_text: Style::default().fg(p.fg.rgb),
        label: Style::default().fg(p.dim.rgb),
        empty_bar: Style::default().fg(p.faint.rgb),
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
        pet_body: Style::default().fg(p.fg.rgb),
        pet_eye: Style::default().fg(p.good.rgb).add_modifier(Modifier::BOLD),
        pet_mouth: Style::default().fg(p.dim.rgb),
        pet_accent: Style::default().fg(p.accent.rgb),
        pet_pattern: Style::default().fg(p.faint.rgb),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BarRamp {
    pub stops: [Color; 5],
}

pub const BAR_RAMP_GOOD: BarRamp = BarRamp {
    stops: [
        Color::Rgb(0x3d, 0x69, 0x48),
        Color::Rgb(0x5a, 0x84, 0x62),
        Color::Rgb(0x82, 0xbc, 0x83),
        Color::Rgb(0xa8, 0xd6, 0x90),
        Color::Rgb(0xd2, 0xee, 0xa2),
    ],
};

pub const BAR_RAMP_ACCENT: BarRamp = BarRamp {
    stops: [
        Color::Rgb(0x6e, 0x45, 0x16),
        Color::Rgb(0xb8, 0x7a, 0x2c),
        Color::Rgb(0xf0, 0xa6, 0x46),
        Color::Rgb(0xff, 0xc6, 0x6e),
        Color::Rgb(0xff, 0xe0, 0xa8),
    ],
};

pub fn ramp_index(i: usize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 0;
    }
    let raw = ((i as f64) * 4.0 / ((n - 1) as f64)).round() as usize;
    raw.min(4)
}

#[cfg(test)]
mod bar_ramp_tests {
    use super::*;

    #[test]
    fn ramp_index_zero_fill_never_called() {
        // Defensive: callers should never call ramp_index with N=0,
        // but the guard returns 0 to avoid panics on division.
        assert_eq!(ramp_index(0, 0), 0);
        assert_eq!(ramp_index(5, 0), 0);
    }

    #[test]
    fn ramp_index_single_cell_returns_zero() {
        assert_eq!(ramp_index(0, 1), 0);
    }

    #[test]
    fn ramp_index_full_bar_spans_entire_ramp() {
        let total = 12;
        assert_eq!(ramp_index(0, total), 0);
        assert_eq!(ramp_index(total - 1, total), 4);
    }

    #[test]
    fn ramp_index_clamps_to_four() {
        // Pathological input: i > N-1 should never come from the renderer,
        // but `.min(4)` is the guard.
        assert!(ramp_index(20, 12) <= 4);
    }

    #[test]
    fn green_ramp_middle_stop_matches_palette_good() {
        let ramp = BAR_RAMP_GOOD;
        let palette_good = tokenpet_palette().good.rgb;
        assert_eq!(ramp.stops[2], palette_good);
    }

    #[test]
    fn amber_ramp_middle_stop_matches_palette_accent() {
        let ramp = BAR_RAMP_ACCENT;
        let palette_accent = tokenpet_palette().accent.rgb;
        assert_eq!(ramp.stops[2], palette_accent);
    }
}

#[cfg(test)]
mod color_capability_tests {
    use super::*;

    fn lookup(map: &[(&str, &str)], key: &str) -> Option<String> {
        map.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_string())
    }

    #[test]
    fn truecolor_when_colorterm_truecolor() {
        let env = [("COLORTERM", "truecolor")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Truecolor);
    }

    #[test]
    fn truecolor_when_colorterm_24bit() {
        let env = [("COLORTERM", "24bit")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Truecolor);
    }

    #[test]
    fn flat_when_no_color_set() {
        let env = [("COLORTERM", "truecolor"), ("NO_COLOR", "1")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Flat);
    }

    #[test]
    fn flat_when_term_dumb() {
        let env = [("TERM", "dumb")];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Flat);
    }

    #[test]
    fn flat_when_no_relevant_env() {
        let env: [(&str, &str); 0] = [];
        let cap = ColorCapability::detect_from(|k| lookup(&env, k));
        assert_eq!(cap, ColorCapability::Flat);
    }
}
