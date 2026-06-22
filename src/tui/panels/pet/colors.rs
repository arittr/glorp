use ratatui::style::{Color, Modifier, Style};

use crate::pet::animator::{compute_token_pop, TokenPop};
use crate::pet::render::PaletteRoleName;
use crate::tui::day::DayPhase;
use crate::tui::life::{PetLifeProfile, PropReaction, SourceAccent, WorkWeather};
use crate::tui::style::{ColorCapability, SemanticStyles};

/// Capped count of extra activity glyphs for the current life profile.
pub(super) fn activity_glyph_budget(profile: &PetLifeProfile, compact: bool) -> usize {
    if profile.calm_mode {
        return 0;
    }
    let max = if compact { 3.0 } else { 10.0 };
    ((profile.activity_level.clamp(0.0, 2.0) / 2.0) * max).round() as usize
}

pub(super) fn activity_glyph_color(profile: &PetLifeProfile) -> Color {
    let p = crate::tui::style::tokenpet_palette();
    let weather = match profile.work_weather {
        WorkWeather::CacheMist => p.good.rgb,
        WorkWeather::OutputSparks => p.accent.rgb,
        WorkWeather::ReasoningPulse => p.bad.rgb,
        WorkWeather::Mixed => p.good.rgb,
        WorkWeather::Clear => p.accent.rgb,
    };
    if let Some(accent) = profile.source_accent {
        if profile.work_weather == WorkWeather::Clear {
            source_accent_color(accent)
        } else {
            blend_colors(source_accent_color(accent), weather, 0.65)
        }
    } else {
        weather
    }
}

fn source_accent_color(accent: SourceAccent) -> Color {
    match accent {
        SourceAccent::Claude => Color::Rgb(0xb3, 0x9d, 0xff),
        SourceAccent::Codex => Color::Rgb(0x86, 0xd9, 0xef),
        SourceAccent::Balanced | SourceAccent::Ensemble => Color::Rgb(0xf0, 0xc4, 0x6a),
    }
}

fn blend_colors(primary: Color, secondary: Color, primary_weight: f32) -> Color {
    let (Color::Rgb(pr, pg, pb), Color::Rgb(sr, sg, sb)) = (primary, secondary) else {
        return primary;
    };
    let weight = primary_weight.clamp(0.0, 1.0);
    let inv = 1.0 - weight;
    Color::Rgb(
        ((pr as f32 * weight) + (sr as f32 * inv)).round() as u8,
        ((pg as f32 * weight) + (sg as f32 * inv)).round() as u8,
        ((pb as f32 * weight) + (sb as f32 * inv)).round() as u8,
    )
}

pub(super) fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return a;
    };
    let t = t.clamp(0.0, 1.0);
    Color::Rgb(
        ((ar as f32 * (1.0 - t)) + (br as f32 * t)).round() as u8,
        ((ag as f32 * (1.0 - t)) + (bg as f32 * t)).round() as u8,
        ((ab as f32 * (1.0 - t)) + (bb as f32 * t)).round() as u8,
    )
}

pub(super) fn warm_shift(base: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = base else {
        return base;
    };
    let t = amount.clamp(0.0, 1.0);
    let add = (t * 40.0).round() as u8;
    let sub = (t * 30.0).round() as u8;
    Color::Rgb(r.saturating_add(add), g, b.saturating_sub(sub))
}

pub(super) fn dim_shift(base: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = base else {
        return base;
    };
    let t = amount.clamp(0.0, 1.0);
    let m = 1.0 - t * 0.5;
    Color::Rgb(
        (r as f32 * m).round() as u8,
        (g as f32 * m).round() as u8,
        (b as f32 * m).round() as u8,
    )
}

/// Apply the day-phase "ambient light" to one style's fg: warmer at dusk,
/// cooler and dimmer at night, neutral by day. Mirrors the sky's phase curve
/// (warm_shift/dim_shift) so pet and room share one light.
pub(super) fn tint_style_for_phase(style: Style, phase: DayPhase, blend: f32) -> Style {
    let Some(fg) = style.fg else { return style };
    let Color::Rgb(..) = fg else { return style };
    let tinted = match phase {
        DayPhase::Day => fg,
        DayPhase::Dawn => warm_shift(fg, 0.10 * blend),
        DayPhase::Dusk => warm_shift(fg, 0.18 * blend),
        DayPhase::Night => dim_shift(cool_shift(fg, 0.18 * blend), 0.28 * blend),
    };
    style.fg(tinted)
}

/// Nudge a color toward cool (more blue, less red) for night ambience.
fn cool_shift(color: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    let amt = amount.clamp(0.0, 1.0);
    let r2 = (f32::from(r) * (1.0 - 0.5 * amt)).round() as u8;
    let b2 = (f32::from(b) + (255.0 - f32::from(b)) * 0.25 * amt).round() as u8;
    Color::Rgb(r2, g, b2)
}

/// Apply the phase tint to all five pet roles of a SemanticStyles.
pub(super) fn tint_pet_styles_for_phase(
    styles: &SemanticStyles,
    phase: DayPhase,
    blend: f32,
) -> SemanticStyles {
    let mut s = styles.clone();
    s.pet_body = tint_style_for_phase(s.pet_body, phase, blend);
    s.pet_eye = tint_style_for_phase(s.pet_eye, phase, blend);
    s.pet_mouth = tint_style_for_phase(s.pet_mouth, phase, blend);
    s.pet_accent = tint_style_for_phase(s.pet_accent, phase, blend);
    s.pet_pattern = tint_style_for_phase(s.pet_pattern, phase, blend);
    s.pet_particle = tint_style_for_phase(s.pet_particle, phase, blend);
    s
}

pub(super) fn profile_token_pop(
    last_feed_pulse_at: Option<time::OffsetDateTime>,
    profile: &PetLifeProfile,
    color_capability: ColorCapability,
    now: time::OffsetDateTime,
) -> Option<TokenPop> {
    if profile.calm_mode
        || profile.burst_level <= 0.0
        || matches!(color_capability, ColorCapability::Flat)
    {
        return None;
    }
    compute_token_pop(last_feed_pulse_at, now)
}

pub(super) fn activity_lift_style(
    style: Style,
    activity_level: f32,
    color_capability: ColorCapability,
) -> Style {
    if matches!(color_capability, ColorCapability::Flat) {
        return style;
    }
    let lift = (activity_level.clamp(0.0, 2.0) * 22.0) as u8;
    match style.fg {
        Some(Color::Rgb(r, g, b)) => style.fg(Color::Rgb(
            r.saturating_add(lift),
            g.saturating_add(lift),
            b.saturating_add(lift),
        )),
        _ => style,
    }
}

pub(super) fn apply_prop_reaction_style(
    style: Style,
    reaction: Option<&PropReaction>,
    color_capability: ColorCapability,
) -> Style {
    if matches!(color_capability, ColorCapability::Flat) {
        return style;
    }
    let Some(reaction) = reaction else {
        return style;
    };
    let lift = (reaction.intensity.clamp(0.0, 1.0) * 35.0) as u8;
    match style.fg {
        Some(Color::Rgb(r, g, b)) => style.fg(Color::Rgb(
            r.saturating_add(lift),
            g.saturating_add(lift),
            b.saturating_add(lift),
        )),
        _ => style,
    }
}

pub(super) fn lift_pet_styles_for_activity(
    styles: &SemanticStyles,
    activity_level: f32,
    color_capability: ColorCapability,
) -> SemanticStyles {
    let mut s = styles.clone();
    s.pet_body = activity_lift_style(s.pet_body, activity_level, color_capability);
    s.pet_eye = activity_lift_style(s.pet_eye, activity_level, color_capability);
    s.pet_mouth = activity_lift_style(s.pet_mouth, activity_level, color_capability);
    s.pet_accent = activity_lift_style(s.pet_accent, activity_level, color_capability);
    s.pet_pattern = activity_lift_style(s.pet_pattern, activity_level, color_capability);
    s.pet_particle = activity_lift_style(s.pet_particle, activity_level, color_capability);
    s
}

/// Resting brightness baseline by performance state, composed UNDER the
/// activity lift (a tired pet still visibly brightens when work arrives, it
/// just settles back lower). 1.0 = neutral. Bounded so the pet is never dark.
pub(super) fn performance_lightness_multiplier(
    performance: crate::tui::room::PetPerformance,
) -> f32 {
    use crate::tui::room::PetPerformance::*;
    match performance {
        RestedAwake | CatchUpWake | SourceBurstPerk => 1.0,
        TiredAwake => 0.88,
        HeavyDayCozy => 0.82,
        AsleepDreaming => 0.7,
    }
}

/// Resting vertical offset (rows) by performance state. Settled states sit
/// one row lower; alert/rested stay put. Capped at 1 to preserve the quiet
/// halo around the pet.
pub(super) fn performance_posture_offset(performance: crate::tui::room::PetPerformance) -> u16 {
    use crate::tui::room::PetPerformance::*;
    match performance {
        TiredAwake | HeavyDayCozy | AsleepDreaming => 1,
        RestedAwake | CatchUpWake | SourceBurstPerk => 0,
    }
}

/// Returns a copy of `base` with all pet-role foreground colors scaled by
/// `multiplier` (1.0 = unchanged, 0.55 = ~half lightness). Non-RGB colors
/// pass through unchanged.
pub(super) fn darken_pet_styles(base: &SemanticStyles, multiplier: f32) -> SemanticStyles {
    let mut s = base.clone();
    s.pet_body = darken_style(s.pet_body, multiplier);
    s.pet_eye = darken_style(s.pet_eye, multiplier);
    s.pet_mouth = darken_style(s.pet_mouth, multiplier);
    s.pet_accent = darken_style(s.pet_accent, multiplier);
    s.pet_pattern = darken_style(s.pet_pattern, multiplier);
    s.pet_particle = darken_style(s.pet_particle, multiplier);
    s
}

/// Returns a copy of `base` where the style for `role` has its foreground
/// brightened by `multiplier`. Other roles are returned unchanged.
/// A multiplier > 1.0 brightens; use this for shimmer/token-pop effects.
pub(super) fn brighten_pet_role(
    base: &SemanticStyles,
    role: Option<PaletteRoleName>,
    multiplier: f32,
) -> SemanticStyles {
    let Some(role) = role else {
        return base.clone();
    };
    let mut s = base.clone();
    match role {
        PaletteRoleName::Body => s.pet_body = brighten_style(s.pet_body, multiplier),
        PaletteRoleName::Accent => s.pet_accent = brighten_style(s.pet_accent, multiplier),
        PaletteRoleName::Pattern => s.pet_pattern = brighten_style(s.pet_pattern, multiplier),
        PaletteRoleName::Eye => s.pet_eye = brighten_style(s.pet_eye, multiplier),
        PaletteRoleName::Mouth => s.pet_mouth = brighten_style(s.pet_mouth, multiplier),
        PaletteRoleName::Particle => s.pet_particle = brighten_style(s.pet_particle, multiplier),
        PaletteRoleName::Corruption => s.pet_accent = brighten_style(s.pet_accent, multiplier),
    }
    s
}

fn brighten_style(style: Style, multiplier: f32) -> Style {
    if let Some(Color::Rgb(r, g, b)) = style.fg {
        let m = multiplier.max(0.0);
        let r = (r as f32 * m).min(255.0) as u8;
        let g = (g as f32 * m).min(255.0) as u8;
        let b = (b as f32 * m).min(255.0) as u8;
        style.fg(Color::Rgb(r, g, b))
    } else {
        style
    }
}

fn darken_style(style: Style, multiplier: f32) -> Style {
    if let Some(Color::Rgb(r, g, b)) = style.fg {
        let m = multiplier.clamp(0.0, 1.0);
        let r = (r as f32 * m) as u8;
        let g = (g as f32 * m) as u8;
        let b = (b as f32 * m) as u8;
        style.fg(Color::Rgb(r, g, b))
    } else {
        style
    }
}

pub(crate) fn pet_role_style(
    role: PaletteRoleName,
    palette: &crate::pet::palette::ResolvedPalette,
) -> Style {
    let rgb = crate::pet::palette::role_color(role, palette);
    let mut style = Style::default().fg(Color::Rgb(rgb.r, rgb.g, rgb.b));
    if matches!(role, PaletteRoleName::Eye) {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// Overlays a per-pet `ResolvedPalette` onto the pet roles of `base`, keeping
/// every role's modifiers (e.g. the bold eye). The live dim/lift/shimmer chain
/// then mutates these seeded colors, so role-tagged glyphs and body-gap fills
/// both track per-pet color and brightness coherently.
pub(super) fn seed_pet_palette(
    base: &SemanticStyles,
    palette: &crate::pet::palette::ResolvedPalette,
) -> SemanticStyles {
    let with_rgb =
        |style: Style, rgb: crate::pet::palette::Rgb| style.fg(Color::Rgb(rgb.r, rgb.g, rgb.b));
    let mut s = base.clone();
    s.pet_body = with_rgb(s.pet_body, palette.body);
    s.pet_eye = with_rgb(s.pet_eye, palette.eye);
    s.pet_mouth = with_rgb(s.pet_mouth, palette.mouth);
    s.pet_accent = with_rgb(s.pet_accent, palette.accent);
    s.pet_pattern = with_rgb(s.pet_pattern, palette.pattern);
    s.pet_particle = with_rgb(s.pet_particle, palette.particle);
    s
}

/// Snapshot the per-role foreground colors of the live `SemanticStyles` into a
/// `ResolvedPalette`. The watch passes the dim/lift/shimmer-mutated `live_styles`
/// here so the role-colored glyphs track exactly the same lightness changes as
/// the body-gap fills (`styles.pet_body`), keeping the pet internally coherent.
/// Non-RGB foregrounds (none occur on the pet roles today) fall back to the
/// default theme color for that role.
pub(super) fn palette_from_styles(styles: &SemanticStyles) -> crate::pet::palette::ResolvedPalette {
    let default = crate::pet::palette::default_theme_palette();
    let rgb = |style: Style, fallback: crate::pet::palette::Rgb| match style.fg {
        Some(Color::Rgb(r, g, b)) => crate::pet::palette::Rgb::new(r, g, b),
        _ => fallback,
    };
    crate::pet::palette::ResolvedPalette {
        body: rgb(styles.pet_body, default.body),
        eye: rgb(styles.pet_eye, default.eye),
        mouth: rgb(styles.pet_mouth, default.mouth),
        accent: rgb(styles.pet_accent, default.accent),
        pattern: rgb(styles.pet_pattern, default.pattern),
        particle: rgb(styles.pet_particle, default.particle),
        corruption: default.corruption,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::semantic_styles;

    #[test]
    fn activity_lift_does_not_invert_body_hue_at_high_chroma() {
        use crate::pet::generation::{Species, VisibleTraits};
        use crate::pet::palette::resolve_pet_palette;
        // The loudest body (Glitch acid). Lift it hard and confirm green still
        // dominates (no channel pins to 255 and flips the hue read).
        let palette = resolve_pet_palette(
            Species::Glitch,
            &VisibleTraits {
                eyes: "o o".into(),
                mouth: "w".into(),
                pattern: "...".into(),
                accent: "*".into(),
                palette_index: 0,
                morph_index: 0,
                morph_pup_index: 0,
                seed_hue: 0,
                saturation_percent: 100,
            },
        );
        let body_before = palette.body;
        assert!(
            body_before.g >= body_before.r && body_before.g >= body_before.b,
            "glitch body should be green-dominant before lift: {body_before:?}"
        );
        let styles = seed_pet_palette(&semantic_styles(), &palette);
        let lifted = activity_lift_style(styles.pet_body, 2.0, ColorCapability::Truecolor);
        if let Some(ratatui::style::Color::Rgb(r, g, b)) = lifted.fg {
            assert!(
                g >= r && g >= b,
                "max activity lift must not flip glitch body off green: ({r},{g},{b})"
            );
        } else {
            panic!("lifted body should stay Rgb");
        }
    }

    #[test]
    fn live_round_trip_preserves_distinct_particle_hue() {
        use crate::pet::generation::{Species, VisibleTraits};
        use crate::pet::palette::resolve_pet_palette;
        let resolved = resolve_pet_palette(
            Species::Crystal,
            &VisibleTraits {
                eyes: "o o".into(),
                mouth: "w".into(),
                pattern: "...".into(),
                accent: "*".into(),
                palette_index: 0,
                morph_index: 0,
                morph_pup_index: 0,
                seed_hue: 0,
                saturation_percent: 90,
            },
        );
        assert_ne!(
            resolved.particle, resolved.accent,
            "precondition: Crystal particle must differ from accent in resolve"
        );
        let styles = seed_pet_palette(&semantic_styles(), &resolved);
        let round_tripped = palette_from_styles(&styles);
        assert_eq!(
            round_tripped.particle, resolved.particle,
            "live SemanticStyles round-trip must preserve the distinct particle hue"
        );
        assert_ne!(
            round_tripped.particle, round_tripped.accent,
            "live particle must not collapse to accent"
        );
    }
}
