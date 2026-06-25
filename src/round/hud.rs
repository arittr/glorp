//! Pure, cross-platform geometry and color helpers for the round companion HUD
//! (growth ring, rate comet, stat gap, mood aura color). No AppKit; golden-testable.

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

/// Comet orbit phase in [0, 1). A nonzero baseline keeps it alive at idle; the
/// token rate adds speed on top. Pure function of the animation frame so it
/// animates every UI tick. Starting constants — tuned on device.
pub fn comet_phase(frame: u64, rate_per_hour: f64) -> f64 {
    const BASELINE_PER_FRAME: f64 = 1.0 / 40.0; // ~one lap / 10s at 4 fps
    const RATE_NORM: f64 = 100_000_000.0; // tokens/hr that doubles the orbit speed
    let speed = BASELINE_PER_FRAME * (1.0 + (rate_per_hour.max(0.0) / RATE_NORM));
    (frame as f64 * speed).rem_euclid(1.0)
}

/// Point on the visible track for `phase` (0 = track start, 1 = track end).
pub fn comet_position(ring: &GrowthRing, phase: f64) -> (f64, f64) {
    let ang_deg = ring.track_start_deg + ring.track_sweep_deg * phase.rem_euclid(1.0);
    let ang = ang_deg.to_radians();
    (
        ring.cx + ring.radius * ang.cos(),
        ring.cy + ring.radius * ang.sin(),
    )
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
    fn comet_advances_even_when_idle() {
        // Nonzero baseline orbit: phase must change frame-to-frame at rate 0.
        let a = comet_phase(0, 0.0);
        let b = comet_phase(10, 0.0);
        assert_ne!(a, b, "comet must keep orbiting at zero rate (idle floor)");
    }

    #[test]
    fn comet_is_faster_when_busy() {
        let idle = comet_phase(20, 0.0);
        let busy = comet_phase(20, 50_000_000.0);
        assert!(
            busy > idle,
            "higher token rate should advance the comet further by the same frame"
        );
    }

    #[test]
    fn comet_stays_on_the_visible_track() {
        let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
        for i in 0..100 {
            let phase = i as f64 / 100.0;
            let (x, y) = comet_position(&ring, phase);
            // On the circle of the given radius.
            let d = ((x - ring.cx).powi(2) + (y - ring.cy).powi(2)).sqrt();
            assert!(
                (d - ring.radius).abs() < 1e-6,
                "comet must ride the ring radius"
            );
            // Never in the bottom gap: its angle is within the track sweep.
            let ang = (y - ring.cy)
                .atan2(x - ring.cx)
                .to_degrees()
                .rem_euclid(360.0);
            let start = ring.track_start_deg.rem_euclid(360.0);
            let rel = (ang - start).rem_euclid(360.0);
            assert!(
                rel <= ring.track_sweep_deg + 1e-6,
                "comet angle must lie on the track"
            );
        }
    }
}
