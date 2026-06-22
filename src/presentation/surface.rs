//! Surface rendering policy types.
//!
//! Defines per-surface style policy (`SurfaceStyle`) and the color pipeline
//! input/output types (`LiveColorInputs`, `ResolvedColors`). The actual
//! resolver logic lives in Task 3 (`color_resolver.rs`).

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
// Tests
// ---------------------------------------------------------------------------

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
