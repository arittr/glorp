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
}
