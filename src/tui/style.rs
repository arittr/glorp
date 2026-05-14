use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

static ACTIVE_THEME: OnceLock<Theme> = OnceLock::new();

/// Initialize the active theme. Call once at app startup (e.g. from
/// `WatchApp::run`). Subsequent calls are no-ops.
pub fn init_theme(theme: Theme) {
    let _ = ACTIVE_THEME.set(theme);
}

/// Returns the active theme, defaulting to `Dark` if not yet initialized.
/// Tests get `Dark` because they never call `init_theme`.
pub fn current_theme() -> Theme {
    *ACTIVE_THEME.get().unwrap_or(&Theme::Dark)
}

/// Detect terminal background and pick the appropriate theme. Uses
/// `terminal_light::luma()`; falls back to `Dark` on detection failure.
pub fn detect_theme() -> Theme {
    match terminal_light::luma() {
        Ok(luma) if luma > 0.5 => Theme::Light,
        _ => Theme::Dark,
    }
}

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
    match current_theme() {
        Theme::Dark => tokenpet_palette_dark(),
        Theme::Light => tokenpet_palette_light(),
    }
}

/// Dark theme — the original glorp palette. Cream-on-coffee with warm orange
/// accents.
pub fn tokenpet_palette_dark() -> TokenpetPalette {
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

/// Light theme. Inverts the dark palette's lightness anchors while preserving
/// hue families: cream background with deep warm-grey foreground, and
/// accent/good/bad pulled to ~0.50 lightness so they remain legible on cream.
/// The accent shifts from peach (0.78 L) to burnt-amber (0.55 L); good shifts
/// from sage-mint to forest-moss; bad shifts from coral to brick.
pub fn tokenpet_palette_light() -> TokenpetPalette {
    TokenpetPalette {
        bg: TokenpetColor {
            name: "bg",
            source_oklch: "oklch(0.96 0.008 80)",
            rgb: Color::Rgb(0xf6, 0xf2, 0xea),
        },
        surface: TokenpetColor {
            name: "surface",
            source_oklch: "oklch(0.92 0.010 80)",
            rgb: Color::Rgb(0xe9, 0xe3, 0xd8),
        },
        fg: TokenpetColor {
            name: "fg",
            source_oklch: "oklch(0.22 0.012 60)",
            rgb: Color::Rgb(0x24, 0x20, 0x1c),
        },
        dim: TokenpetColor {
            name: "dim",
            source_oklch: "oklch(0.42 0.014 70)",
            rgb: Color::Rgb(0x55, 0x4e, 0x46),
        },
        faint: TokenpetColor {
            name: "faint",
            source_oklch: "oklch(0.70 0.010 70)",
            rgb: Color::Rgb(0xa9, 0xa2, 0x97),
        },
        accent: TokenpetColor {
            name: "accent",
            source_oklch: "oklch(0.55 0.13 60)",
            rgb: Color::Rgb(0xa7, 0x6a, 0x2a),
        },
        good: TokenpetColor {
            name: "good",
            source_oklch: "oklch(0.46 0.10 145)",
            rgb: Color::Rgb(0x3b, 0x6f, 0x47),
        },
        bad: TokenpetColor {
            name: "bad",
            source_oklch: "oklch(0.50 0.15 25)",
            rgb: Color::Rgb(0xa8, 0x40, 0x3a),
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
    /// Pet activity / thought line — "<name> is doing X" entries that
    /// surface the pet's interior life in the feed.
    PetActivity,
    /// Character-driven narration — token rate, mood, stage, vital, and idle events
    /// generated by the narration system. Styled in a muted accent so users can
    /// distinguish the two threads (provider events vs. pet behavior).
    Narrative,
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
            LogKind::PetActivity => Style::default()
                .fg(p.accent.rgb)
                .add_modifier(Modifier::ITALIC),
            // Narrative entries use a dim accent — visually distinct from the
            // green provider rows but quieter than bold evolution text.
            LogKind::Narrative => Style::default()
                .fg(p.dim.rgb)
                .add_modifier(Modifier::ITALIC),
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

/// Dark-theme bar ramps. Low values render in deep saturated colors; high
/// values render bright. Designed to read clearly against the dark coffee bg.
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

/// Light-theme bar ramps. Direction is inverted from dark: low values are
/// faint (close to background), high values are saturated and deep. Reads
/// clearly against the cream bg.
pub const BAR_RAMP_GOOD_LIGHT: BarRamp = BarRamp {
    stops: [
        Color::Rgb(0xc8, 0xdf, 0xc8),
        Color::Rgb(0x9a, 0xc0, 0x9c),
        Color::Rgb(0x6b, 0x95, 0x6e),
        Color::Rgb(0x44, 0x6b, 0x47),
        Color::Rgb(0x21, 0x46, 0x26),
    ],
};

pub const BAR_RAMP_ACCENT_LIGHT: BarRamp = BarRamp {
    stops: [
        Color::Rgb(0xf2, 0xdb, 0xae),
        Color::Rgb(0xdb, 0xab, 0x6c),
        Color::Rgb(0xb6, 0x7a, 0x32),
        Color::Rgb(0x85, 0x55, 0x18),
        Color::Rgb(0x52, 0x36, 0x05),
    ],
};

pub fn bar_ramp_good() -> BarRamp {
    match current_theme() {
        Theme::Dark => BAR_RAMP_GOOD,
        Theme::Light => BAR_RAMP_GOOD_LIGHT,
    }
}

pub fn bar_ramp_accent() -> BarRamp {
    match current_theme() {
        Theme::Dark => BAR_RAMP_ACCENT,
        Theme::Light => BAR_RAMP_ACCENT_LIGHT,
    }
}

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

/// Derive a 5-stop bar ramp from a single mid color. The mid color sits at
/// stop 2; lower stops scale toward black, higher stops blend toward white.
/// Mirrors the dim→bright shape of the hand-tuned `BAR_RAMP_GOOD` ramp so
/// per-stat gradient bars share visual weight regardless of which color drives
/// them. Non-RGB inputs degrade to a flat ramp.
pub fn gradient_ramp(mid: Color) -> BarRamp {
    let Color::Rgb(r, g, b) = mid else {
        return BarRamp { stops: [mid; 5] };
    };
    let scale = |c: u8, f: f32| (c as f32 * f).round().clamp(0.0, 255.0) as u8;
    let toward_white = |c: u8, f: f32| {
        let lifted = c as f32 + (255.0 - c as f32) * f;
        lifted.round().clamp(0.0, 255.0) as u8
    };
    BarRamp {
        stops: [
            Color::Rgb(scale(r, 0.45), scale(g, 0.45), scale(b, 0.45)),
            Color::Rgb(scale(r, 0.70), scale(g, 0.70), scale(b, 0.70)),
            Color::Rgb(r, g, b),
            Color::Rgb(
                toward_white(r, 0.30),
                toward_white(g, 0.30),
                toward_white(b, 0.30),
            ),
            Color::Rgb(
                toward_white(r, 0.55),
                toward_white(g, 0.55),
                toward_white(b, 0.55),
            ),
        ],
    }
}

/// Per-stat semantic color roles (revision-3 layout refresh).
///
/// Each function returns a single solid `Color::Rgb`; the same color paints
/// the label, the filled bar segments, and the trailing value for that stat.
/// Empty bar cells continue to use `SemanticStyles.empty_bar`.
///
/// Dark-background truecolor tuned. Color-blind / 8-color palette tuning is
/// deferred — ratatui's color downgrade handles low-capability terminals.
pub fn fed_color() -> Color {
    Color::Rgb(0xe8, 0xc4, 0x74)
}

pub fn happy_color() -> Color {
    Color::Rgb(0xe8, 0xa3, 0xc2)
}

pub fn energy_color() -> Color {
    Color::Rgb(0x7f, 0xc8, 0xd6)
}

pub fn xp_color() -> Color {
    Color::Rgb(0xef, 0x8e, 0x6c)
}

pub fn claude_color() -> Color {
    Color::Rgb(0xb3, 0x9d, 0xf0)
}

pub fn codex_color() -> Color {
    Color::Rgb(0x8f, 0xcf, 0x90)
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
    fn green_ramp_middle_stop_matches_dark_palette_good() {
        // BAR_RAMP_GOOD is the dark-theme ramp; its middle stop should match
        // the dark palette's `good` color. Light-theme ramp is tested via
        // `bar_ramp_good()` in a separate case below.
        let ramp = BAR_RAMP_GOOD;
        let palette_good = tokenpet_palette_dark().good.rgb;
        assert_eq!(ramp.stops[2], palette_good);
    }

    #[test]
    fn light_palette_fg_is_dark_and_bg_is_light() {
        let light = tokenpet_palette_light();
        let dark = tokenpet_palette_dark();
        if let (Color::Rgb(_, l_fg_g, _), Color::Rgb(_, l_bg_g, _)) = (light.fg.rgb, light.bg.rgb) {
            assert!(
                l_fg_g < l_bg_g,
                "light theme fg should be darker than bg (G channel: fg {l_fg_g}, bg {l_bg_g})"
            );
        }
        if let (Color::Rgb(_, d_fg_g, _), Color::Rgb(_, d_bg_g, _)) = (dark.fg.rgb, dark.bg.rgb) {
            assert!(d_fg_g > d_bg_g, "dark theme fg should be brighter than bg");
        }
    }

    #[test]
    fn light_palette_inverts_lightness_relative_to_dark() {
        // Spot-check that light bg approximately equals dark fg in brightness
        // (both are off-white cream). This validates the inverted-anchor design.
        let light_bg = tokenpet_palette_light().bg.rgb;
        let dark_fg = tokenpet_palette_dark().fg.rgb;
        if let (Color::Rgb(_, lb, _), Color::Rgb(_, df, _)) = (light_bg, dark_fg) {
            let diff = (lb as i32 - df as i32).abs();
            assert!(diff < 30, "light bg should be within 30 G units of dark fg");
        }
    }

    #[test]
    fn light_ramps_invert_direction() {
        // Light theme ramps should go from faint→saturated as i increases.
        // We check that the last stop is darker than the first (cream-relative).
        let ramp = BAR_RAMP_GOOD_LIGHT;
        if let (Color::Rgb(_, g0, _), Color::Rgb(_, g4, _)) = (ramp.stops[0], ramp.stops[4]) {
            assert!(g0 > g4, "light ramp first stop should be lighter than last");
        } else {
            panic!("light ramp stops should be Rgb");
        }
    }

    #[test]
    fn amber_ramp_middle_stop_matches_palette_accent() {
        let ramp = BAR_RAMP_ACCENT;
        let palette_accent = tokenpet_palette_dark().accent.rgb;
        assert_eq!(ramp.stops[2], palette_accent);
    }
}

#[cfg(test)]
mod color_capability_tests {
    use super::*;

    fn lookup(map: &[(&str, &str)], key: &str) -> Option<String> {
        map.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_string())
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

#[cfg(test)]
mod stat_color_tests {
    use super::*;

    #[test]
    fn fed_color_is_amber() {
        assert_eq!(fed_color(), Color::Rgb(0xe8, 0xc4, 0x74));
    }

    #[test]
    fn happy_color_is_pink() {
        assert_eq!(happy_color(), Color::Rgb(0xe8, 0xa3, 0xc2));
    }

    #[test]
    fn energy_color_is_cyan() {
        assert_eq!(energy_color(), Color::Rgb(0x7f, 0xc8, 0xd6));
    }

    #[test]
    fn xp_color_is_coral() {
        assert_eq!(xp_color(), Color::Rgb(0xef, 0x8e, 0x6c));
    }

    #[test]
    fn claude_color_is_violet() {
        assert_eq!(claude_color(), Color::Rgb(0xb3, 0x9d, 0xf0));
    }

    #[test]
    fn codex_color_is_green() {
        assert_eq!(codex_color(), Color::Rgb(0x8f, 0xcf, 0x90));
    }

    #[test]
    fn xp_color_is_distinct_from_fed_color() {
        assert_ne!(xp_color(), fed_color(), "xp must not collide with fed");
    }
}
