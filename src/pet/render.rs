use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::generation::{GeneratedPet, Species};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPet {
    pub lines: Vec<String>,
    pub spans: Vec<StyledSegment>,
    pub event_lines: Vec<String>,
}

impl RenderedPet {
    pub fn with_evolution_flash(mut self, from: Stage, to: Stage) -> Self {
        self.event_lines.push(format!(
            "* evolved from {} to {} *",
            stage_name(from),
            stage_name(to)
        ));
        self.lines.insert(0, String::from("** evolution flash **"));
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledSegment {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub role: PaletteRoleName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteRoleName {
    Body,
    Eye,
    Mouth,
    Accent,
    Pattern,
    Particle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteRoles {
    pub body: PaletteRole,
    pub eye: PaletteRole,
    pub mouth: PaletteRole,
    pub accent: PaletteRole,
    pub pattern: PaletteRole,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaletteRole {
    pub lightness: f32,
    pub base_chroma: f32,
    pub hue_degrees: u16,
    pub hue_offset_degrees: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationProfile {
    pub breath_period: u8,
    pub breath_hold: u8,
    pub blink_average: u8,
    pub blink_jitter: u8,
}

pub fn render_pet(
    pet: &GeneratedPet,
    stage: Stage,
    mood: Mood,
    frame: AnimationFrame,
) -> RenderedPet {
    use crate::pet::generate::generate_pet_lines_animated;
    use crate::pet::generation::fnv1a64;

    let seed = fnv1a64(&pet.seed);
    let blink_open = blink_open_for(pet, mood, frame);
    let lines = generate_pet_lines_animated(pet.species, stage, seed, mood, blink_open);

    // Build per-cell StyledSegments. Heuristic: braille cells (U+2800..U+28FF)
    // are Body; non-braille cells are feature glyphs assigned by line position.
    let mut spans = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        for (ch_idx, c) in line.chars().enumerate() {
            let role = if (0x2800..=0x28FF).contains(&(c as u32)) {
                PaletteRoleName::Body
            } else if c == ' ' {
                continue;
            } else if line_idx == 0 {
                PaletteRoleName::Eye
            } else {
                PaletteRoleName::Mouth
            };
            spans.push(StyledSegment {
                line: line_idx,
                start: ch_idx,
                end: ch_idx + 1,
                role,
            });
        }
    }

    RenderedPet {
        lines,
        spans,
        event_lines: Vec::new(),
    }
}

/// Decide whether the eye is currently open. Blink is a brief eye-close that
/// fires roughly every `blink_average` ticks (with a per-species jitter
/// derived from the pet's seed). `blink_suppression_ticks` from the frame
/// inhibits blinking immediately after mood changes so the suppression read
/// as an expression change rather than an eye-flicker.
fn blink_open_for(pet: &GeneratedPet, _mood: Mood, frame: AnimationFrame) -> bool {
    if frame.blink_suppression_ticks > 0 {
        return true;
    }
    let profile = species_animation_profile(pet.species);
    if profile.blink_average == 0 {
        return true;
    }
    let jitter = u64::from(profile.blink_jitter.max(1));
    let phase_seed = u64::from(pet.animation_phase.blink).max(1);
    let period = u64::from(profile.blink_average);
    let phase = (phase_seed.wrapping_mul(2_654_435_761)) % period.max(1);
    let cycle_pos = (frame.tick.wrapping_add(phase)) % period.max(1);
    // Eye is closed for the first 2 ticks of each cycle; a small jitter
    // shifts the closure by ±1 tick per pet so blinks aren't synchronized
    // across pets.
    let jittered_open_threshold = 2 + ((phase_seed % jitter) % 2);
    cycle_pos >= jittered_open_threshold
}

pub fn palette_roles(pet: &GeneratedPet) -> PaletteRoles {
    let saturation = f32::from(pet.traits.saturation_percent) / 100.0;
    let hue = pet.traits.seed_hue;
    PaletteRoles {
        body: role(0.84, 0.10, hue, 0, saturation),
        eye: role(0.84, 0.13, hue, 180, saturation),
        mouth: role(0.84, 0.10, hue, 30, saturation),
        accent: role(0.82, 0.11, hue, 90, saturation),
        pattern: role(0.76, 0.06, hue, 150, saturation),
    }
}

pub fn species_animation_profile(species: Species) -> AnimationProfile {
    match species {
        Species::Fuzz => AnimationProfile {
            breath_period: 16,
            breath_hold: 4,
            blink_average: 32,
            blink_jitter: 12,
        },
        Species::Blob => AnimationProfile {
            breath_period: 13,
            breath_hold: 5,
            blink_average: 40,
            blink_jitter: 14,
        },
        Species::Ghost => AnimationProfile {
            breath_period: 11,
            breath_hold: 3,
            blink_average: 50,
            blink_jitter: 18,
        },
        Species::Glitch => AnimationProfile {
            breath_period: 9,
            breath_hold: 2,
            blink_average: 24,
            blink_jitter: 8,
        },
        Species::Crystal => AnimationProfile {
            breath_period: 19,
            breath_hold: 6,
            blink_average: 60,
            blink_jitter: 22,
        },
        Species::Mech => AnimationProfile {
            breath_period: 17,
            breath_hold: 4,
            blink_average: 22,
            blink_jitter: 6,
        },
    }
}

pub fn closed_blink_eyes(species: Species) -> &'static str {
    match species {
        Species::Fuzz | Species::Blob => "- -",
        Species::Ghost => "\u{2014} \u{2014}",
        Species::Glitch => "\u{2592}\u{2592}\u{2592}",
        Species::Crystal => "\u{25c7} \u{25c7}",
        Species::Mech => "= =",
    }
}

fn role(
    lightness: f32,
    base_chroma: f32,
    hue: u16,
    hue_offset_degrees: i16,
    saturation: f32,
) -> PaletteRole {
    PaletteRole {
        lightness,
        base_chroma,
        hue_degrees: ((i32::from(hue) + i32::from(hue_offset_degrees)).rem_euclid(360)) as u16,
        hue_offset_degrees,
    }
    .with_saturation(saturation)
}

impl PaletteRole {
    fn with_saturation(mut self, saturation: f32) -> Self {
        self.base_chroma *= saturation;
        self
    }
}

fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::S0 => "s0",
        Stage::S1 => "s1",
        Stage::S2 => "s2",
        Stage::S3 => "s3",
        Stage::S4 => "s4",
        Stage::S5 => "s5",
        Stage::S6 => "s6",
    }
}

#[cfg(test)]
mod render_v2_tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::generate_pet;

    #[test]
    fn render_pet_produces_braille_lines() {
        let pet = generate_pet("test-seed-1");
        let r = render_pet(
            &pet,
            Stage::S0,
            Mood::Content,
            AnimationFrame {
                tick: 0,
                blink_suppression_ticks: 0,
            },
        );
        assert!(!r.lines.is_empty());
        let has_braille = r
            .lines
            .iter()
            .any(|l| l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))));
        assert!(
            has_braille,
            "expected at least one braille char in rendered lines"
        );
    }

    #[test]
    fn render_pet_deterministic_for_fixed_frame() {
        let pet = generate_pet("test-seed-2");
        let frame = AnimationFrame {
            tick: 5,
            blink_suppression_ticks: 0,
        };
        let a = render_pet(&pet, Stage::S1, Mood::Content, frame);
        let b = render_pet(&pet, Stage::S1, Mood::Content, frame);
        assert_eq!(a.lines, b.lines);
        assert_eq!(a.spans, b.spans);
    }

    #[test]
    fn render_pet_two_seeds_differ() {
        // generate_pet picks species deterministically from seed; with two
        // very different seeds we expect different rendered outputs even if
        // they collide on species occasionally.
        let pet_a = generate_pet("seed-aaa");
        let pet_b = generate_pet("seed-zzz");
        let frame = AnimationFrame {
            tick: 0,
            blink_suppression_ticks: 0,
        };
        let a = render_pet(&pet_a, Stage::S2, Mood::Content, frame);
        let b = render_pet(&pet_b, Stage::S2, Mood::Content, frame);
        // Either lines differ or spans differ — output should not be identical.
        assert!(a.lines != b.lines || a.spans != b.spans);
    }

    #[test]
    fn render_pet_mood_changes_visible_output() {
        // Different moods should produce different rendered output via the
        // mood-override eye/mouth glyphs.
        let pet = generate_pet("mood-test");
        let frame = AnimationFrame {
            tick: 0,
            blink_suppression_ticks: 1, // suppress blink so mood is the only diff
        };
        let content = render_pet(&pet, Stage::S1, Mood::Content, frame);
        let sleepy = render_pet(&pet, Stage::S1, Mood::Sleepy, frame);
        let happy = render_pet(&pet, Stage::S1, Mood::Happy, frame);
        assert_ne!(
            content.lines, sleepy.lines,
            "sleepy should differ from content"
        );
        assert_ne!(
            content.lines, happy.lines,
            "happy should differ from content"
        );
        assert_ne!(sleepy.lines, happy.lines, "sleepy should differ from happy");
    }

    #[test]
    fn blink_suppression_keeps_eyes_open() {
        // With blink_suppression_ticks > 0, the eye stays open regardless of
        // tick value.
        let pet = generate_pet("blink-suppress-test");
        let suppressed = render_pet(
            &pet,
            Stage::S0,
            Mood::Content,
            AnimationFrame {
                tick: 0,
                blink_suppression_ticks: 5,
            },
        );
        // Also render at tick=0 without suppression; if the species' phase puts
        // it on a closed-eye tick, the two outputs would differ. We verify that
        // the suppression branch produces stable output for nearby ticks (the
        // pet does NOT blink during the suppression window).
        let suppressed_2 = render_pet(
            &pet,
            Stage::S0,
            Mood::Content,
            AnimationFrame {
                tick: 1,
                blink_suppression_ticks: 5,
            },
        );
        assert_eq!(
            suppressed.lines, suppressed_2.lines,
            "blink-suppressed pet should render identically across ticks"
        );
    }
}
