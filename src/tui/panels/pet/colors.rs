use ratatui::style::{Color, Modifier, Style};

use crate::pet::animator::{compute_token_pop, low_energy_lightness_multiplier, TokenPop};
use crate::pet::render::PaletteRoleName;
#[cfg(test)]
use crate::tui::day::DayPhase;
use crate::tui::life::{PetLifeProfile, PropReaction, SourceAccent, WorkWeather};
use crate::tui::style::{semantic_styles, ColorCapability, SemanticStyles};
use crate::tui::view_model::WatchViewModel;

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
    let out = crate::presentation::color_ops::blend(
        crate::pet::palette::Rgb::new(pr, pg, pb),
        crate::pet::palette::Rgb::new(sr, sg, sb),
        primary_weight,
    );
    Color::Rgb(out.r, out.g, out.b)
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
    let out =
        crate::presentation::color_ops::warm_shift(crate::pet::palette::Rgb::new(r, g, b), amount);
    Color::Rgb(out.r, out.g, out.b)
}

pub(super) fn dim_shift(base: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = base else {
        return base;
    };
    let out =
        crate::presentation::color_ops::dim_shift(crate::pet::palette::Rgb::new(r, g, b), amount);
    Color::Rgb(out.r, out.g, out.b)
}

/// Apply the day-phase "ambient light" to one style's fg: warmer at dusk,
/// cooler and dimmer at night, neutral by day. Mirrors the sky's phase curve
/// (warm_shift/dim_shift) so pet and room share one light.
///
/// The live watch path runs phase tint through `resolve_pet_colors`; this
/// per-style form survives only as a `color_ops` parity check.
#[cfg(test)]
pub(super) fn tint_style_for_phase(style: Style, phase: DayPhase, blend: f32) -> Style {
    let Some(fg) = style.fg else { return style };
    let Color::Rgb(r, g, b) = fg else {
        return style;
    };
    let out = crate::presentation::color_ops::tint_for_phase(
        crate::pet::palette::Rgb::new(r, g, b),
        phase,
        blend,
    );
    style.fg(Color::Rgb(out.r, out.g, out.b))
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

/// Activity-lift one style's fg. The live watch path runs this through
/// `resolve_pet_colors`; this per-style form survives only as a `color_ops`
/// parity check (Flat early-return + hue stability).
#[cfg(test)]
pub(super) fn activity_lift_style(
    style: Style,
    activity_level: f32,
    color_capability: ColorCapability,
) -> Style {
    if matches!(color_capability, ColorCapability::Flat) {
        return style;
    }
    match style.fg {
        Some(Color::Rgb(r, g, b)) => {
            let out = crate::presentation::color_ops::activity_lift_channel(
                crate::pet::palette::Rgb::new(r, g, b),
                activity_level,
            );
            style.fg(Color::Rgb(out.r, out.g, out.b))
        }
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
///
/// The live watch path runs droop through `resolve_pet_colors`; this
/// `SemanticStyles` form survives only as a dim-parity check.
#[cfg(test)]
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

#[cfg(test)]
fn darken_style(style: Style, multiplier: f32) -> Style {
    let Some(Color::Rgb(r, g, b)) = style.fg else {
        return style;
    };
    let out = crate::presentation::color_ops::darken_channel(
        crate::pet::palette::Rgb::new(r, g, b),
        multiplier,
    );
    style.fg(Color::Rgb(out.r, out.g, out.b))
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
/// every role's modifiers (e.g. the bold eye). Superseded on the live watch
/// path by `semantic_styles_from_resolved`; survives only to seed styles for
/// the `color_ops` round-trip parity tests.
#[cfg(test)]
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

/// Seed a `SemanticStyles` from resolver output, replacing each pet-role
/// foreground with the resolved color and re-applying the bold eye when the
/// surface uses `EyeEmphasis::TerminalBold`. This is the resolver-era successor
/// to the `seed_pet_palette` → tint → darken → brighten → lift chain: the
/// resolver already ran the live transforms, so this only carries colors and
/// the eye modifier across into the styles the watch renderer consumes.
pub(super) fn semantic_styles_from_resolved(
    base: &SemanticStyles,
    c: &crate::presentation::surface::ResolvedColors,
) -> SemanticStyles {
    use crate::presentation::surface::EyeEmphasis;
    let with =
        |style: Style, rgb: crate::pet::palette::Rgb| style.fg(Color::Rgb(rgb.r, rgb.g, rgb.b));
    let mut s = base.clone();
    s.pet_body = with(s.pet_body, c.body);
    s.pet_eye = with(s.pet_eye, c.eye);
    if matches!(c.eye_emphasis, EyeEmphasis::TerminalBold) {
        s.pet_eye = s.pet_eye.add_modifier(Modifier::BOLD);
    }
    s.pet_mouth = with(s.pet_mouth, c.mouth);
    s.pet_accent = with(s.pet_accent, c.accent);
    s.pet_pattern = with(s.pet_pattern, c.pattern);
    s.pet_particle = with(s.pet_particle, c.particle);
    s
}

/// Assemble the watch's live color inputs from the view model: the day-phase
/// blend, the combined energy/performance droop, the effective shimmer role
/// (token-pop overrides shimmer to Pattern) and its brighten multiplier, and
/// the calm-gated activity level. These are exactly the legacy inline chain's
/// scalars, now in resolver vocabulary.
pub(super) fn watch_live_color_inputs(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    pet_performance: crate::tui::room::PetPerformance,
    shimmer_role: Option<PaletteRoleName>,
    token_pop_active: bool,
) -> crate::presentation::surface::LiveColorInputs {
    let phase_blend = {
        let since = (now - vm.day_context.phase_started_at_utc).whole_seconds() as f32;
        (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
    };
    let droop_mult = low_energy_lightness_multiplier(vm.energy)
        * performance_lightness_multiplier(pet_performance);
    // Token-pop overrides shimmer to Pattern for extra flash; shimmer/pop boost
    // lightness ~1.4× (clamped on the u8 channel by the resolver).
    let effective_shimmer_role = if token_pop_active {
        Some(PaletteRoleName::Pattern)
    } else {
        shimmer_role
    };
    let shimmer_mult = if effective_shimmer_role.is_some() {
        1.4
    } else {
        1.0
    };
    let activity_level = if vm.life_profile.calm_mode {
        0.0
    } else {
        vm.life_profile.activity_level
    };
    crate::presentation::surface::LiveColorInputs {
        phase: vm.day_context.day_phase,
        phase_blend,
        droop_mult,
        shimmer_role: effective_shimmer_role,
        shimmer_mult,
        activity_level,
        source_override: None,
    }
}

/// Resolve the watch's live pet styles and the speech-bubble droop styles in a
/// single shared pass. Both flow through `resolve_pet_colors`: `live_styles`
/// uses the full `WATCH_STYLE` (phase tint + droop + shimmer + activity lift),
/// while the bubble uses a droop-only variant (phase tint + droop, no shimmer,
/// no activity lift) so the bubble reads dimmed but never shimmered. Activity
/// lift is a no-op on `Flat` terminals (the legacy lift early-returned there),
/// so the knob is gated off to match exactly.
pub(super) fn resolve_watch_pet_styles(
    palette: &crate::pet::palette::ResolvedPalette,
    inputs: &crate::presentation::surface::LiveColorInputs,
    color_capability: ColorCapability,
) -> (SemanticStyles, SemanticStyles) {
    use crate::presentation::surface::{resolve_pet_colors, SurfaceStyle, WATCH_STYLE};
    let live_style = if matches!(color_capability, ColorCapability::Flat) {
        SurfaceStyle {
            activity_lift: false,
            ..WATCH_STYLE
        }
    } else {
        WATCH_STYLE
    };
    let droop_style = SurfaceStyle {
        shimmer: false,
        activity_lift: false,
        ..WATCH_STYLE
    };
    let base = semantic_styles();
    let live =
        semantic_styles_from_resolved(&base, &resolve_pet_colors(palette, inputs, &live_style));
    let droop =
        semantic_styles_from_resolved(&base, &resolve_pet_colors(palette, inputs, &droop_style));
    (live, droop)
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

    #[test]
    fn semantic_styles_from_resolved_seeds_colors_and_bolds_eye() {
        use crate::pet::palette::Rgb;
        use crate::presentation::surface::{EyeEmphasis, ResolvedColors};
        let resolved = ResolvedColors {
            body: Rgb::new(1, 2, 3),
            eye: Rgb::new(10, 20, 30),
            mouth: Rgb::new(40, 50, 60),
            accent: Rgb::new(70, 80, 90),
            pattern: Rgb::new(100, 110, 120),
            particle: Rgb::new(130, 140, 150),
            corruption: Rgb::new(160, 170, 180),
            eye_emphasis: EyeEmphasis::TerminalBold,
        };
        let out = semantic_styles_from_resolved(&semantic_styles(), &resolved);
        assert_eq!(out.pet_body.fg, Some(Color::Rgb(1, 2, 3)));
        assert_eq!(out.pet_eye.fg, Some(Color::Rgb(10, 20, 30)));
        assert_eq!(out.pet_mouth.fg, Some(Color::Rgb(40, 50, 60)));
        assert_eq!(out.pet_accent.fg, Some(Color::Rgb(70, 80, 90)));
        assert_eq!(out.pet_pattern.fg, Some(Color::Rgb(100, 110, 120)));
        assert_eq!(out.pet_particle.fg, Some(Color::Rgb(130, 140, 150)));
        assert!(
            out.pet_eye.add_modifier.contains(Modifier::BOLD),
            "TerminalBold emphasis must bold the eye"
        );
    }

    #[test]
    fn semantic_styles_from_resolved_skips_eye_bold_when_emphasis_off() {
        use crate::pet::palette::Rgb;
        use crate::presentation::surface::{EyeEmphasis, ResolvedColors};
        let resolved = ResolvedColors {
            body: Rgb::new(1, 2, 3),
            eye: Rgb::new(10, 20, 30),
            mouth: Rgb::new(40, 50, 60),
            accent: Rgb::new(70, 80, 90),
            pattern: Rgb::new(100, 110, 120),
            particle: Rgb::new(130, 140, 150),
            corruption: Rgb::new(160, 170, 180),
            eye_emphasis: EyeEmphasis::None,
        };
        // Seed from a base whose eye carries no bold, to isolate the helper's add.
        let mut base = semantic_styles();
        base.pet_eye = Style::default().fg(Color::Rgb(0, 0, 0));
        let out = semantic_styles_from_resolved(&base, &resolved);
        assert!(
            !out.pet_eye.add_modifier.contains(Modifier::BOLD),
            "non-bold emphasis must not add BOLD"
        );
    }
}
