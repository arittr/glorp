//! Pure, cross-platform geometry and color helpers for the round companion HUD
//! (growth ring, stat gap, mood aura color). No AppKit; golden-testable.

use crate::game::metabolism::Mood;
use crate::round::draw::RoundColor;

/// Open-bottom growth ring geometry. Angles are degrees, CCW from +x (AppKit).
/// The gap is centered at the bottom (270°); the track sweeps CCW over the top
/// from the gap's right edge to its left edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrowthRing {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub track_start_deg: f64,
    pub track_sweep_deg: f64,
}

pub const COMPANION_GAUGE_GAP_DEG: f64 = 70.0;
pub const PACE_SOFT_CAP_10M_TOKENS: f64 = 15_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeLane {
    pub ring: GrowthRing,
    pub stroke_width: f64,
    pub cap: LineCap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerimeterGaugeLayout {
    pub xp: GaugeLane,
    pub daily: GaugeLane,
    pub pace: GaugeLane,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeLaneColors {
    pub track: RoundColor,
    pub fill: RoundColor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerimeterGaugeColors {
    pub xp: GaugeLaneColors,
    pub daily: GaugeLaneColors,
    pub pace: GaugeLaneColors,
}

pub fn growth_ring_layout(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> GrowthRing {
    let gap = gap_deg.clamp(0.0, 180.0);
    GrowthRing {
        cx,
        cy,
        radius,
        track_start_deg: 270.0 + gap / 2.0,
        track_sweep_deg: 360.0 - gap,
    }
}

/// Angle (deg) where the violet fill ends for `fraction` of stage progress.
pub fn growth_ring_fill_end_deg(ring: &GrowthRing, fraction: f64) -> f64 {
    ring.track_start_deg + ring.track_sweep_deg * fraction.clamp(0.0, 1.0)
}

pub fn perimeter_gauge_layout(
    cx: f64,
    cy: f64,
    aperture_radius: f64,
    gap_deg: f64,
) -> PerimeterGaugeLayout {
    let outer_inset_px = 3.0_f64.max(aperture_radius * 0.012);
    let xp_width = (aperture_radius * 0.050).clamp(6.0, 16.0);
    let daily_width = (aperture_radius * 0.040).clamp(5.0, 13.0);
    let pace_width = (aperture_radius * 0.034).clamp(4.0, 11.0);
    let lane_gap = (aperture_radius * 0.010).clamp(1.5, 4.0);

    let xp_radius = aperture_radius - outer_inset_px - xp_width / 2.0;
    let daily_radius = xp_radius - xp_width / 2.0 - lane_gap - daily_width / 2.0;
    let pace_radius = daily_radius - daily_width / 2.0 - lane_gap - pace_width / 2.0;

    PerimeterGaugeLayout {
        xp: GaugeLane {
            ring: growth_ring_layout(cx, cy, xp_radius, gap_deg),
            stroke_width: xp_width,
            cap: LineCap::Round,
        },
        daily: GaugeLane {
            ring: growth_ring_layout(cx, cy, daily_radius, gap_deg),
            stroke_width: daily_width,
            cap: LineCap::Round,
        },
        pace: GaugeLane {
            ring: growth_ring_layout(cx, cy, pace_radius, gap_deg),
            stroke_width: pace_width,
            cap: LineCap::Round,
        },
    }
}

pub fn perimeter_gauge_colors() -> PerimeterGaugeColors {
    PerimeterGaugeColors {
        xp: GaugeLaneColors {
            track: RoundColor(0.71, 0.71, 0.78, 0.16),
            fill: RoundColor(0.61, 0.48, 0.88, 0.90),
        },
        daily: GaugeLaneColors {
            track: RoundColor(0.47, 0.63, 0.43, 0.12),
            fill: RoundColor(0.50, 0.74, 0.56, 0.76),
        },
        pace: GaugeLaneColors {
            track: RoundColor(0.96, 0.68, 0.31, 0.13),
            fill: RoundColor(0.98, 0.67, 0.27, 0.86),
        },
    }
}

/// The region (in pixels) the token stat must fit inside: centered in the ring's
/// bottom gap, below center, clamped to the gap chord so it never clips the ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatGap {
    pub center_x: f64,
    pub baseline_y: f64,
    pub max_width: f64,
}

pub fn stat_gap_box(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> StatGap {
    let gap = gap_deg.clamp(0.0, 180.0);
    let half_chord = radius * (gap / 2.0).to_radians().sin();
    StatGap {
        center_x: cx,
        // Place the readout in the lower band, a bit above the gap mouth.
        baseline_y: cy + radius * 0.55,
        // A small inset keeps the text off the ring stroke.
        max_width: (2.0 * half_chord * 0.92).max(0.0),
    }
}

/// Soft-glow aura hue for the pet's mood. Opaque (alpha 1.0); the renderer
/// applies its own translucency. Sad and Sleepy are deliberately distinct hues
/// (different needs: happiness<35 vs energy<20). Starting palette — tuned on device.
pub fn mood_aura_color(mood: Mood) -> RoundColor {
    match mood {
        Mood::Content => RoundColor(0.25, 0.71, 0.60, 1.0), // teal
        Mood::Happy => RoundColor(0.82, 0.45, 0.62, 1.0),   // warm pink
        Mood::Ecstatic => RoundColor(0.95, 0.40, 0.70, 1.0), // bright magenta-pink
        Mood::Hungry => RoundColor(0.85, 0.62, 0.30, 1.0),  // amber
        Mood::Sad => RoundColor(0.40, 0.50, 0.78, 1.0),     // muted blue
        Mood::Sleepy => RoundColor(0.55, 0.50, 0.80, 1.0),  // indigo/violet
        Mood::Wilted => RoundColor(0.45, 0.40, 0.48, 1.0),  // dim grey-mauve
    }
}

pub fn rate_direction_color(direction: crate::tui::view_model::RateDirection) -> RoundColor {
    match direction {
        crate::tui::view_model::RateDirection::Up => RoundColor(0.45, 0.84, 0.51, 1.0),
        crate::tui::view_model::RateDirection::Down => RoundColor(0.95, 0.38, 0.36, 1.0),
        crate::tui::view_model::RateDirection::Neutral => RoundColor(0.62, 0.63, 0.77, 1.0),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionHudText {
    pub today_total: String,
    pub daily_percent: String,
    pub pace: String,
}

pub fn companion_pace_fraction(current_10m_tokens: f64) -> f64 {
    if !current_10m_tokens.is_finite() || current_10m_tokens <= 0.0 {
        return 0.0;
    }
    (1.0 - (-current_10m_tokens / PACE_SOFT_CAP_10M_TOKENS).exp()).clamp(0.0, 1.0)
}

pub fn daily_fraction_for_gauge(fraction_of_yesterday: Option<f64>) -> f64 {
    fraction_of_yesterday
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

pub fn daily_gauge_colors_for_fraction(fraction_of_yesterday: Option<f64>) -> GaugeLaneColors {
    let base = perimeter_gauge_colors().daily;
    if fraction_of_yesterday.is_some_and(|value| value.is_finite() && value >= 1.0) {
        GaugeLaneColors {
            fill: RoundColor(0.30, 0.70, 0.40, 0.90),
            ..base
        }
    } else {
        base
    }
}

pub fn format_daily_percent(fraction_of_yesterday: Option<f64>) -> String {
    let Some(fraction) = fraction_of_yesterday else {
        return "--% yday".to_string();
    };
    if !fraction.is_finite() || fraction < 0.0 {
        return "--% yday".to_string();
    }

    let percent = (fraction * 100.0).round();
    if percent > 999.0 {
        "999%+ yday".to_string()
    } else {
        format!("{percent:.0}% yday")
    }
}

pub fn companion_hud_text(
    today_tokens: f64,
    daily_fraction: Option<f64>,
    pulse_10m_tokens: f64,
) -> CompanionHudText {
    CompanionHudText {
        today_total: compact_hud_tokens(today_tokens),
        daily_percent: format_daily_percent(daily_fraction),
        pace: format!("{}/10m", compact_hud_tokens(pulse_10m_tokens.max(0.0))),
    }
}

fn compact_hud_tokens(value: f64) -> String {
    let formatted = crate::format::format_tokens(value);
    formatted
        .strip_suffix(".0B")
        .map(|prefix| format!("{prefix}B"))
        .or_else(|| {
            formatted
                .strip_suffix(".0M")
                .map(|prefix| format!("{prefix}M"))
        })
        .or_else(|| {
            formatted
                .strip_suffix(".0k")
                .map(|prefix| format!("{prefix}k"))
        })
        .unwrap_or(formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_gap_is_centered_at_bottom_and_excluded() {
        let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
        // Track spans 360 - gap = 290 degrees.
        assert!((ring.track_sweep_deg - 290.0).abs() < 1e-6);
        // Track starts at the right edge of the bottom gap: 270 + 35 = 305 deg.
        assert!((ring.track_start_deg - 305.0).abs() < 1e-6);
        // Bottom (270°) is inside the gap, i.e. NOT covered by [start, start+sweep].
        // 270° < track_start (305°) so it lies before the track begins; 630° (≡ 270° + 360°)
        // must also be absent from [start, end] to confirm the gap is not wrapped-over.
        let end = ring.track_start_deg + ring.track_sweep_deg; // 595
        assert!(
            !(ring.track_start_deg..=end).contains(&270.0_f64)
                && !(ring.track_start_deg..=end).contains(&630.0_f64),
            "270° must lie in the gap, not on the track"
        );
    }

    #[test]
    fn fill_end_spans_fraction_of_the_track() {
        let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
        assert!((growth_ring_fill_end_deg(&ring, 0.0) - ring.track_start_deg).abs() < 1e-6);
        assert!(
            (growth_ring_fill_end_deg(&ring, 1.0) - (ring.track_start_deg + ring.track_sweep_deg))
                .abs()
                < 1e-6
        );
        let half = growth_ring_fill_end_deg(&ring, 0.5);
        assert!((half - (ring.track_start_deg + 145.0)).abs() < 1e-6);
        // Clamps out-of-range fractions.
        assert!(
            (growth_ring_fill_end_deg(&ring, 2.0) - (ring.track_start_deg + ring.track_sweep_deg))
                .abs()
                < 1e-6
        );
        assert!((growth_ring_fill_end_deg(&ring, -1.0) - ring.track_start_deg).abs() < 1e-6);
    }

    #[test]
    fn every_mood_has_a_distinct_aura_color() {
        let moods = [
            Mood::Content,
            Mood::Happy,
            Mood::Ecstatic,
            Mood::Hungry,
            Mood::Sad,
            Mood::Sleepy,
            Mood::Wilted,
        ];
        let colors: Vec<RoundColor> = moods.iter().map(|m| mood_aura_color(*m)).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(
                    colors[i], colors[j],
                    "moods {:?} and {:?} must have distinct aura colors",
                    moods[i], moods[j]
                );
            }
        }
    }

    #[test]
    fn sad_and_sleepy_are_distinct() {
        assert_ne!(mood_aura_color(Mood::Sad), mood_aura_color(Mood::Sleepy));
    }

    #[test]
    fn rate_direction_colors_are_distinct() {
        use crate::tui::view_model::RateDirection;

        assert_ne!(
            rate_direction_color(RateDirection::Up),
            rate_direction_color(RateDirection::Down)
        );
        assert_ne!(
            rate_direction_color(RateDirection::Neutral),
            rate_direction_color(RateDirection::Up)
        );
    }

    #[test]
    fn daily_gauge_color_is_muted_sage_not_cyan() {
        let colors = perimeter_gauge_colors();
        let RoundColor(track_red, track_green, track_blue, track_alpha) = colors.daily.track;
        let RoundColor(fill_red, fill_green, fill_blue, fill_alpha) = colors.daily.fill;

        assert!(
            track_green > track_blue + 0.08,
            "daily track should not read as blue/cyan: {:?}",
            colors.daily.track
        );
        assert!(
            fill_green > fill_blue + 0.10,
            "daily fill should not read as blue/cyan: {:?}",
            colors.daily.fill
        );
        assert!(track_green > track_red);
        assert!(fill_green > fill_red + 0.12);
        assert!(track_alpha <= 0.14);
        assert!(fill_alpha <= 0.78);
        assert_ne!(colors.daily.fill, colors.xp.fill);
        assert_ne!(colors.daily.fill, colors.pace.fill);
    }

    #[test]
    fn daily_gauge_uses_deep_leaf_when_today_beats_yesterday() {
        let base = daily_gauge_colors_for_fraction(Some(0.99));
        let over = daily_gauge_colors_for_fraction(Some(1.0));

        assert_eq!(base, perimeter_gauge_colors().daily);
        assert_eq!(over.track, base.track);
        assert_eq!(over.fill, RoundColor(0.30, 0.70, 0.40, 0.90));
    }

    #[test]
    fn stat_gap_box_sits_below_center_and_within_the_chord() {
        let gap = stat_gap_box(100.0, 100.0, 90.0, 70.0);
        assert!((gap.center_x - 100.0).abs() < 1e-6, "centered horizontally");
        assert!(
            gap.baseline_y > 100.0,
            "stat sits below the vertical center (lower half)"
        );
        // The gap chord half-width at the ring edges is radius * sin(gap/2).
        let expected_half = 90.0 * (35.0_f64.to_radians()).sin();
        assert!(
            gap.max_width <= 2.0 * expected_half + 1e-6,
            "stat must fit within the gap chord"
        );
        assert!(gap.max_width > 0.0);
    }

    #[test]
    fn perimeter_gauge_layout_keeps_three_round_lanes_inside_aperture() {
        let layout = perimeter_gauge_layout(180.0, 180.0, 180.0, COMPANION_GAUGE_GAP_DEG);

        assert_eq!(layout.xp.cap, LineCap::Round);
        assert_eq!(layout.daily.cap, LineCap::Round);
        assert_eq!(layout.pace.cap, LineCap::Round);

        assert_eq!(
            layout.xp.ring.track_start_deg,
            layout.daily.ring.track_start_deg
        );
        assert_eq!(
            layout.daily.ring.track_start_deg,
            layout.pace.ring.track_start_deg
        );
        assert_eq!(
            layout.xp.ring.track_sweep_deg,
            layout.daily.ring.track_sweep_deg
        );
        assert_eq!(
            layout.daily.ring.track_sweep_deg,
            layout.pace.ring.track_sweep_deg
        );

        assert!(layout.xp.ring.radius > layout.daily.ring.radius);
        assert!(layout.daily.ring.radius > layout.pace.ring.radius);
        assert!(layout.xp.stroke_width > layout.daily.stroke_width);
        assert!(layout.daily.stroke_width > layout.pace.stroke_width);

        let xp_outer_edge = layout.xp.ring.radius + layout.xp.stroke_width / 2.0;
        let pace_inner_edge = layout.pace.ring.radius - layout.pace.stroke_width / 2.0;

        assert!(xp_outer_edge <= 177.0);
        assert!(pace_inner_edge > 180.0 * 0.72);
    }

    #[test]
    fn pace_fraction_uses_named_soft_cap_and_clamps_bad_inputs() {
        assert_eq!(PACE_SOFT_CAP_10M_TOKENS, 15_000_000.0);
        assert_eq!(companion_pace_fraction(0.0), 0.0);
        assert!((companion_pace_fraction(4_000_000.0) - 0.234).abs() < 0.002);
        assert!((companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS) - 0.632).abs() < 0.002);
        assert!((companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 2.0) - 0.865).abs() < 0.002);
        assert!(companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 100.0) <= 1.0);
        assert_eq!(companion_pace_fraction(-1.0), 0.0);
        assert_eq!(companion_pace_fraction(f64::NAN), 0.0);
        assert_eq!(companion_pace_fraction(f64::INFINITY), 0.0);
    }

    #[test]
    fn companion_hud_text_formats_total_daily_percent_and_pace_only() {
        let text = companion_hud_text(842_000_000.0, Some(1.244), 31_000_000.0);

        assert_eq!(text.today_total, "842M");
        assert_eq!(text.daily_percent, "124% yday");
        assert_eq!(text.pace, "31M/10m");
        assert!(!text.pace.contains("/hr"));
    }

    #[test]
    fn daily_percent_text_preserves_stack_when_unavailable_and_caps_extreme_values() {
        assert_eq!(format_daily_percent(None), "--% yday");
        assert_eq!(format_daily_percent(Some(0.944)), "94% yday");
        assert_eq!(format_daily_percent(Some(10.5)), "999%+ yday");
        assert_eq!(format_daily_percent(Some(f64::NAN)), "--% yday");
        assert_eq!(format_daily_percent(Some(f64::INFINITY)), "--% yday");
    }
}
