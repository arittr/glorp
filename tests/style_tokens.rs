use glorp::tui::style::{semantic_styles, tokenpet_palette, LogKind};
use ratatui::style::Color;

#[test]
fn palette_matches_tokenpet_handoff_values() {
    let p = tokenpet_palette();
    assert_eq!(p.bg.source_oklch, "oklch(0.18 0.005 60)");
    assert_eq!(p.bg.rgb, Color::Rgb(0x13, 0x11, 0x0f));
    assert_eq!(p.surface.source_oklch, "oklch(0.22 0.006 60)");
    assert_eq!(p.surface.rgb, Color::Rgb(0x1d, 0x1a, 0x18));
    assert_eq!(p.fg.source_oklch, "oklch(0.94 0.01 80)");
    assert_eq!(p.fg.rgb, Color::Rgb(0xef, 0xeb, 0xe4));
    assert_eq!(p.dim.source_oklch, "oklch(0.66 0.012 70)");
    assert_eq!(p.dim.rgb, Color::Rgb(0x97, 0x91, 0x8a));
    assert_eq!(p.faint.source_oklch, "oklch(0.42 0.008 60)");
    assert_eq!(p.faint.rgb, Color::Rgb(0x50, 0x4c, 0x49));
    assert_eq!(p.accent.source_oklch, "oklch(0.78 0.14 70)");
    assert_eq!(p.accent.rgb, Color::Rgb(0xf0, 0xa6, 0x46));
    assert_eq!(p.good.source_oklch, "oklch(0.74 0.10 145)");
    assert_eq!(p.good.rgb, Color::Rgb(0x82, 0xbc, 0x83));
    assert_eq!(p.bad.source_oklch, "oklch(0.68 0.16 25)");
    assert_eq!(p.bad.rgb, Color::Rgb(0xea, 0x6a, 0x64));
}

#[test]
fn semantic_styles_preserve_tokenpet_roles() {
    let styles = semantic_styles();
    let p = tokenpet_palette();
    assert_eq!(styles.body.fg, Some(p.fg.rgb));
    assert_eq!(styles.body.bg, Some(p.bg.rgb));
    assert_eq!(styles.section_header.fg, Some(p.faint.rgb));
    assert_eq!(styles.timestamp.fg, Some(p.faint.rgb));
    assert_eq!(styles.empty_bar.fg, Some(p.faint.rgb));
    assert_eq!(styles.event_rail_usage.fg, Some(p.good.rgb));
    assert_eq!(styles.event_rail_diagnostic.fg, Some(p.bad.rgb));
    assert_eq!(styles.sparkline_today.fg, Some(p.accent.rgb));
    assert_eq!(styles.sparkline_past.fg, Some(p.faint.rgb));
    assert_eq!(styles.log(LogKind::Normal).fg, Some(p.dim.rgb));
    assert_eq!(styles.log(LogKind::Usage).fg, Some(p.good.rgb));
    assert_eq!(styles.log(LogKind::Diagnostic).fg, Some(p.bad.rgb));
    assert_eq!(styles.log(LogKind::Evolution).fg, Some(p.accent.rgb));
}

#[test]
fn bar_ramp_good_middle_stop_matches_palette_good() {
    use glorp::tui::style::{tokenpet_palette, BAR_RAMP_GOOD};
    let palette = tokenpet_palette();
    assert_eq!(BAR_RAMP_GOOD.stops[2], palette.good.rgb);
}

#[test]
fn bar_ramp_accent_middle_stop_matches_palette_accent() {
    use glorp::tui::style::{tokenpet_palette, BAR_RAMP_ACCENT};
    let palette = tokenpet_palette();
    assert_eq!(BAR_RAMP_ACCENT.stops[2], palette.accent.rgb);
}

#[test]
fn component_style_facade_maps_semantic_tokens_to_ratatui_styles() {
    use glorp::tui::component::{
        BorderTone, ComponentStyle, GradientToken, Insets, Surface, TextTone,
    };
    use glorp::tui::style::tokenpet_palette;

    let p = tokenpet_palette();
    let style = ComponentStyle::new()
        .surface(Surface::Elevated)
        .border(BorderTone::Accent)
        .padding(Insets::horizontal(2))
        .text(TextTone::Primary)
        .gradient(GradientToken::Xp);

    assert_eq!(style.surface_style().bg, Some(p.surface.rgb));
    assert_eq!(style.border_style().fg, Some(p.accent.rgb));
    assert_eq!(style.text_style().fg, Some(p.fg.rgb));
    assert_eq!(style.insets().left, 2);
}
