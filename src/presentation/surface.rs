//! Per-surface color policy and the unified resolver.
//!
//! Defines `SurfaceStyle` (the only place surfaces differ), the resolver
//! input/output types (`LiveColorInputs`, `ResolvedColors`), and
//! `resolve_pet_colors`, which maps a pet's role colors for a given surface.

use crate::pet::palette::Rgb;
use crate::pet::render::PaletteRoleName;
use crate::tui::day::DayPhase;

// ---------------------------------------------------------------------------
// Policy enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    Full,
    Compact,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clip {
    None,
    Circle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EyeEmphasis {
    None,
    TerminalBold,
    Brightness,
}

// ---------------------------------------------------------------------------
// SurfaceStyle — per-surface rendering policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct SurfaceStyle {
    pub detail: Detail,
    pub clip: Clip,
    /// Apply a source-accent override color instead of live palette colors.
    pub source_accent: bool,
    /// Apply phase-of-day tint to resolved colors.
    pub phase_tint: bool,
    /// Apply energy-droop desaturation when vitals are low.
    pub energy_droop: bool,
    /// Apply shimmer highlight to the shimmer role.
    pub shimmer: bool,
    /// Apply activity-lift brightness when the pet is active.
    pub activity_lift: bool,
    /// Apply prop-reaction color shift when props are nearby.
    /// Consumed by `apply_prop_reaction_style` on the prop-glyph render path;
    /// not yet read by `resolve_pet_colors`.
    pub prop_reaction: bool,
    /// How to visually emphasize eye segments.
    pub eye_emphasis: EyeEmphasis,
}

// ---------------------------------------------------------------------------
// Surface style constants
// ---------------------------------------------------------------------------

pub const WATCH_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Full,
    clip: Clip::None,
    source_accent: false,
    phase_tint: true,
    energy_droop: true,
    shimmer: true,
    activity_lift: true,
    prop_reaction: true,
    eye_emphasis: EyeEmphasis::TerminalBold,
};

pub const ROUND_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Compact,
    clip: Clip::Circle,
    source_accent: false,
    phase_tint: false,
    energy_droop: false,
    shimmer: false,
    activity_lift: false,
    prop_reaction: false,
    eye_emphasis: EyeEmphasis::None,
};

pub const SCREEN_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Full,
    clip: Clip::None,
    source_accent: false,
    phase_tint: false,
    energy_droop: false,
    shimmer: false,
    activity_lift: false,
    prop_reaction: false,
    eye_emphasis: EyeEmphasis::Brightness,
};

pub const MENU_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Minimal,
    clip: Clip::None,
    source_accent: true,
    phase_tint: false,
    energy_droop: true,
    shimmer: false,
    activity_lift: false,
    prop_reaction: false,
    eye_emphasis: EyeEmphasis::None,
};

// ---------------------------------------------------------------------------
// LiveColorInputs — caller-assembled snapshot passed to the resolver
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct LiveColorInputs {
    pub phase: DayPhase,
    pub phase_blend: f32,
    pub droop_mult: f32,
    pub shimmer_role: Option<PaletteRoleName>,
    pub shimmer_mult: f32,
    pub activity_level: f32,
    pub source_override: Option<Rgb>,
}

impl LiveColorInputs {
    /// All-neutral inputs: Day phase, no blend, no droop, no shimmer, no
    /// activity, no source override.  Used wherever a surface applies no
    /// live transforms (e.g. `ROUND_STYLE`).
    pub fn passthrough() -> Self {
        Self {
            phase: DayPhase::Day,
            phase_blend: 0.0,
            droop_mult: 1.0,
            shimmer_role: None,
            shimmer_mult: 1.0,
            activity_level: 0.0,
            source_override: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ResolvedColors — output of the resolver; one Rgb per palette role
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ResolvedColors {
    pub body: Rgb,
    pub eye: Rgb,
    pub mouth: Rgb,
    pub accent: Rgb,
    pub pattern: Rgb,
    pub particle: Rgb,
    pub corruption: Rgb,
    pub eye_emphasis: EyeEmphasis,
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

use crate::pet::palette::{role_color, ResolvedPalette};
use crate::presentation::color_ops;

pub fn resolve_pet_colors(
    base: &ResolvedPalette,
    inputs: &LiveColorInputs,
    style: &SurfaceStyle,
) -> ResolvedColors {
    let resolve_one = |role: PaletteRoleName| -> Rgb {
        let mut c = role_color(role, base);
        // 1. source-accent override (menubar): accent/particle only
        if style.source_accent
            && matches!(role, PaletteRoleName::Accent | PaletteRoleName::Particle)
        {
            if let Some(over) = inputs.source_override {
                c = over;
            }
        }
        // 2. phase tint (watch)
        if style.phase_tint {
            c = color_ops::tint_for_phase(c, inputs.phase, inputs.phase_blend);
        }
        // 3. energy/sleep droop (watch energy*perf; menubar asleep?0.7:1.0)
        if style.energy_droop {
            c = color_ops::darken_channel(c, inputs.droop_mult);
        }
        // 4. shimmer / token-pop brighten (watch): one role only
        if style.shimmer && inputs.shimmer_role == Some(role) {
            c = color_ops::brighten_channel(c, inputs.shimmer_mult);
        }
        // 5. activity lift (watch)
        if style.activity_lift {
            c = color_ops::activity_lift_channel(c, inputs.activity_level);
        }
        c
    };
    ResolvedColors {
        body: resolve_one(PaletteRoleName::Body),
        eye: resolve_one(PaletteRoleName::Eye),
        mouth: resolve_one(PaletteRoleName::Mouth),
        accent: resolve_one(PaletteRoleName::Accent),
        pattern: resolve_one(PaletteRoleName::Pattern),
        particle: resolve_one(PaletteRoleName::Particle),
        corruption: resolve_one(PaletteRoleName::Corruption),
        eye_emphasis: style.eye_emphasis,
    }
}

/// Map a `PaletteRoleName` to its resolved `Rgb` field on `ResolvedColors`.
pub fn role_rgb(c: &ResolvedColors, role: PaletteRoleName) -> Rgb {
    match role {
        PaletteRoleName::Body => c.body,
        PaletteRoleName::Eye => c.eye,
        PaletteRoleName::Mouth => c.mouth,
        PaletteRoleName::Accent => c.accent,
        PaletteRoleName::Pattern => c.pattern,
        PaletteRoleName::Particle => c.particle,
        PaletteRoleName::Corruption => c.corruption,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod resolver_tests {
    use super::*;
    use crate::pet::palette::{default_theme_palette, role_color, Rgb};
    use crate::pet::render::PaletteRoleName;
    use crate::tui::day::DayPhase;

    fn neutral_inputs() -> LiveColorInputs {
        LiveColorInputs {
            phase: DayPhase::Day,
            phase_blend: 0.0,
            droop_mult: 1.0,
            shimmer_role: None,
            shimmer_mult: 1.0,
            activity_level: 0.0,
            source_override: None,
        }
    }

    #[test]
    fn round_style_is_pure_role_color() {
        let p = default_theme_palette();
        let out = resolve_pet_colors(&p, &neutral_inputs(), &ROUND_STYLE);
        // companion does exactly role_color today
        assert_eq!(out.body, role_color(PaletteRoleName::Body, &p));
        assert_eq!(out.accent, role_color(PaletteRoleName::Accent, &p));
        assert!(matches!(out.eye_emphasis, EyeEmphasis::None));
    }

    #[test]
    fn menu_style_applies_source_override_then_sleep_dim() {
        // MENU_STYLE: source_accent on accent/particle, droop_mult carries the sleep dim.
        let p = default_theme_palette();
        let mut inputs = neutral_inputs();
        inputs.source_override = Some(Rgb::new(0xf0, 0xc4, 0x6a)); // Ensemble
        inputs.droop_mult = 0.7; // asleep
        let out = resolve_pet_colors(&p, &inputs, &MENU_STYLE);
        // accent = override(0xf0,0xc4,0x6a) darkened x0.7 (truncating) = (168,137,74)
        assert_eq!(out.accent, Rgb::new(168, 137, 74));
        // body = base body darkened x0.7, no override
        let body = role_color(PaletteRoleName::Body, &p);
        assert_eq!(
            out.body,
            Rgb::new(
                (body.r as f32 * 0.7) as u8,
                (body.g as f32 * 0.7) as u8,
                (body.b as f32 * 0.7) as u8
            )
        );
    }

    /// Anti-drift parity guard: with neutral (passthrough) inputs every knob is a
    /// no-op, so ALL surfaces MUST collapse to the shared base `role_color`.
    /// This test fails loudly if a future edit makes any surface diverge at its
    /// base resolution (i.e. without live inputs engaged).
    #[test]
    fn surfaces_share_one_base_resolution_with_neutral_inputs() {
        let p = default_theme_palette();
        let neutral = LiveColorInputs::passthrough();
        for style in [WATCH_STYLE, ROUND_STYLE, SCREEN_STYLE, MENU_STYLE] {
            let out = resolve_pet_colors(&p, &neutral, &style);
            for role in [
                PaletteRoleName::Body,
                PaletteRoleName::Mouth,
                PaletteRoleName::Pattern,
                PaletteRoleName::Accent,
                PaletteRoleName::Eye,
                PaletteRoleName::Particle,
                PaletteRoleName::Corruption,
            ] {
                assert_eq!(
                    role_rgb(&out, role),
                    role_color(role, &p),
                    "neutral inputs must collapse every surface to the shared base for {role:?} (style={style:?})"
                );
            }
        }
    }

    #[test]
    fn watch_style_runs_phase_then_droop_then_shimmer_then_lift_in_order() {
        let p = default_theme_palette();
        let mut inputs = neutral_inputs();
        inputs.phase = DayPhase::Dusk;
        inputs.phase_blend = 1.0;
        inputs.droop_mult = 0.8;
        inputs.shimmer_role = Some(PaletteRoleName::Pattern);
        inputs.shimmer_mult = 1.4;
        inputs.activity_level = 1.0;
        let out = resolve_pet_colors(&p, &inputs, &WATCH_STYLE);

        // recompute pattern by hand in the same order
        let mut c = role_color(PaletteRoleName::Pattern, &p);
        c = crate::presentation::color_ops::tint_for_phase(c, DayPhase::Dusk, 1.0);
        c = crate::presentation::color_ops::darken_channel(c, 0.8);
        c = crate::presentation::color_ops::brighten_channel(c, 1.4); // shimmer hits Pattern
        c = crate::presentation::color_ops::activity_lift_channel(c, 1.0);
        assert_eq!(out.pattern, c);
        // body gets the same chain MINUS shimmer (shimmer only hits Pattern)
        let mut b = role_color(PaletteRoleName::Body, &p);
        b = crate::presentation::color_ops::tint_for_phase(b, DayPhase::Dusk, 1.0);
        b = crate::presentation::color_ops::darken_channel(b, 0.8);
        b = crate::presentation::color_ops::activity_lift_channel(b, 1.0);
        assert_eq!(out.body, b);
        assert!(matches!(out.eye_emphasis, EyeEmphasis::TerminalBold));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_style_enables_the_full_live_chain() {
        const {
            assert!(
                WATCH_STYLE.phase_tint
                    && WATCH_STYLE.energy_droop
                    && WATCH_STYLE.shimmer
                    && WATCH_STYLE.activity_lift
                    && WATCH_STYLE.prop_reaction
            );
            assert!(!WATCH_STYLE.source_accent);
        }
        assert!(matches!(
            WATCH_STYLE.eye_emphasis,
            EyeEmphasis::TerminalBold
        ));
    }

    #[test]
    fn menu_style_is_source_accent_and_droop_only() {
        const {
            assert!(MENU_STYLE.source_accent);
            assert!(MENU_STYLE.energy_droop);
            assert!(!MENU_STYLE.phase_tint && !MENU_STYLE.shimmer && !MENU_STYLE.activity_lift);
        }
        assert!(matches!(MENU_STYLE.eye_emphasis, EyeEmphasis::None));
    }

    #[test]
    fn round_style_today_is_passthrough_color() {
        // Companion currently applies no color transforms (it will gain them in the scene plan).
        const {
            assert!(
                !ROUND_STYLE.phase_tint
                    && !ROUND_STYLE.energy_droop
                    && !ROUND_STYLE.shimmer
                    && !ROUND_STYLE.activity_lift
                    && !ROUND_STYLE.source_accent
            );
        }
    }
}
