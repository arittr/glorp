//! Pure, cross-platform geometry and color helpers for the round companion HUD
//! (growth ring, rate comet, stat gap, mood aura color). No AppKit; golden-testable.

use crate::game::metabolism::Mood;
use crate::round::draw::RoundColor;

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
}
