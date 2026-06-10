use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::art::template_lines;
use crate::pet::generation::{GeneratedPet, Species};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
    /// Sleep presentation: force the species closed-blink eyes. Must never be
    /// implemented by substituting Mood::Sleepy — mood is the vitals contract.
    pub hold_eyes_closed: bool,
    /// Ticks added to the species blink cadence (tiredness slows blinking).
    /// 0 = normal. Producers map tiredness 0..1 -> 0..TIRED_BLINK_MAX_SLOWDOWN.
    pub blink_slowdown: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPet {
    pub lines: Vec<String>,
    pub spans: Vec<StyledSegment>,
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

const ART_WIDTH: usize = 11;
const FRAME_WIDTH: usize = 13;
const FRAME_HEIGHT: usize = 10;

const GLITCH_NOISE: &[char] = &[
    '\u{2592}', '\u{2591}', '\u{2593}', '\u{2580}', '\u{2584}', '\u{258c}', '\u{2590}',
];

pub fn render_pet(
    pet: &GeneratedPet,
    stage: Stage,
    mood: Mood,
    frame: AnimationFrame,
) -> RenderedPet {
    let profile = species_animation_profile(pet.species);
    let blinking = frame.hold_eyes_closed || should_blink(pet, mood, frame, profile);
    let expression = expression_for(pet, mood, blinking);
    let raw = template_lines(
        pet.species,
        stage,
        pet.traits.morph_index,
        pet.traits.morph_pup_index,
    );
    let rendered = raw
        .iter()
        .enumerate()
        .map(|(line_index, line)| render_template_line(line, line_index, pet, &expression))
        .collect::<Vec<_>>();
    let mut lines = rendered
        .iter()
        .map(|line| line.text.clone())
        .collect::<Vec<_>>();
    let mut spans = rendered
        .into_iter()
        .flat_map(|line| line.spans)
        .collect::<Vec<_>>();

    // Glitch corruption: rare per-tick body cell swap.
    if pet.species == Species::Glitch {
        apply_glitch_corruption(&mut lines, &mut spans, frame.tick);
    }

    // Wrap pet art in a 13x10 frame and overlay particles.
    let (framed_lines, framed_spans) = frame_with_particles(lines, spans, pet.species, frame.tick);

    RenderedPet {
        lines: framed_lines,
        spans: framed_spans,
    }
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

pub const TIRED_BLINK_MAX_SLOWDOWN: u8 = 24;

pub fn blink_slowdown_for_tiredness(tiredness: f32) -> u8 {
    (tiredness.clamp(0.0, 1.0) * f32::from(TIRED_BLINK_MAX_SLOWDOWN)).round() as u8
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

struct Expression {
    eyes: String,
    mouth: String,
}

fn expression_for(pet: &GeneratedPet, mood: Mood, blinking: bool) -> Expression {
    if blinking {
        return Expression {
            eyes: closed_blink_eyes(pet.species).to_string(),
            mouth: pet.traits.mouth.clone(),
        };
    }

    match mood {
        Mood::Happy => Expression {
            eyes: "^.^".to_string(),
            mouth: "\u{03c9}".to_string(),
        },
        Mood::Content => Expression {
            eyes: pet.traits.eyes.clone(),
            mouth: pet.traits.mouth.clone(),
        },
        Mood::Hungry => Expression {
            eyes: "u.u".to_string(),
            mouth: "o".to_string(),
        },
        Mood::Sad => Expression {
            eyes: "T.T".to_string(),
            mouth: "\u{fe35}".to_string(),
        },
        Mood::Sleepy => Expression {
            eyes: "-.-".to_string(),
            mouth: "-".to_string(),
        },
        Mood::Wilted => Expression {
            eyes: ",_,".to_string(),
            mouth: "_".to_string(),
        },
    }
}

fn should_blink(
    pet: &GeneratedPet,
    mood: Mood,
    frame: AnimationFrame,
    profile: AnimationProfile,
) -> bool {
    if matches!(mood, Mood::Sad | Mood::Sleepy | Mood::Wilted) {
        return false;
    }
    if frame.blink_suppression_ticks > 0 {
        return false;
    }
    let jitter = u64::from(profile.blink_jitter.max(1));
    let cadence = u64::from(profile.blink_average)
        + (u64::from(pet.animation_phase.blink) % jitter)
        + u64::from(frame.blink_slowdown);
    (frame.tick + u64::from(pet.animation_phase.blink)).is_multiple_of(cadence)
}

struct RenderedTemplateLine {
    text: String,
    spans: Vec<StyledSegment>,
}

fn render_template_line(
    template: &str,
    line: usize,
    pet: &GeneratedPet,
    expression: &Expression,
) -> RenderedTemplateLine {
    let mut text = String::new();
    let mut spans = Vec::new();
    let mut cursor = 0;
    let mut rest = template;

    while let Some(open) = rest.find('{') {
        let (body, after_body) = rest.split_at(open);
        push_segment(
            &mut text,
            &mut spans,
            line,
            body,
            PaletteRoleName::Body,
            &mut cursor,
        );

        if let Some(close) = after_body.find('}') {
            let token = &after_body[..=close];
            let (value, role) =
                slot_value(token, pet, expression).unwrap_or((token, PaletteRoleName::Body));
            push_segment(&mut text, &mut spans, line, value, role, &mut cursor);
            rest = &after_body[close + 1..];
        } else {
            push_segment(
                &mut text,
                &mut spans,
                line,
                after_body,
                PaletteRoleName::Body,
                &mut cursor,
            );
            rest = "";
        }
    }

    push_segment(
        &mut text,
        &mut spans,
        line,
        rest,
        PaletteRoleName::Body,
        &mut cursor,
    );
    RenderedTemplateLine { text, spans }
}

fn slot_value<'a>(
    token: &str,
    pet: &'a GeneratedPet,
    expression: &'a Expression,
) -> Option<(&'a str, PaletteRoleName)> {
    match token {
        "{eyes}" => Some((&expression.eyes, PaletteRoleName::Eye)),
        "{mouth}" => Some((&expression.mouth, PaletteRoleName::Mouth)),
        "{pattern}" => Some((&pet.traits.pattern, PaletteRoleName::Pattern)),
        "{accent}" => Some((&pet.traits.accent, PaletteRoleName::Accent)),
        _ => None,
    }
}

fn push_segment(
    text: &mut String,
    spans: &mut Vec<StyledSegment>,
    line: usize,
    value: &str,
    role: PaletteRoleName,
    cursor: &mut usize,
) {
    let width = value.chars().count();
    if width == 0 {
        return;
    }
    text.push_str(value);
    spans.push(StyledSegment {
        line,
        start: *cursor,
        end: *cursor + width,
        role,
    });
    *cursor += width;
}

fn apply_glitch_corruption(lines: &mut [String], spans: &mut [StyledSegment], tick: u64) {
    if !tick.is_multiple_of(37) {
        return;
    }
    if lines.is_empty() {
        return;
    }
    let row = ((tick * 7) as usize) % lines.len();
    let line = &lines[row];
    let total_chars = line.chars().count();
    if total_chars == 0 {
        return;
    }
    let col = ((tick * 11) as usize) % total_chars;
    let target_char = line.chars().nth(col).unwrap_or(' ');
    if target_char == ' ' {
        return;
    }
    // Skip if the cell belongs to anything other than a body span.
    let in_body = spans.iter().any(|span| {
        span.line == row
            && span.role == PaletteRoleName::Body
            && col >= span.start
            && col < span.end
    });
    if !in_body {
        return;
    }

    let noise = GLITCH_NOISE[((tick * 3) as usize) % GLITCH_NOISE.len()];
    replace_char_in_line(&mut lines[row], col, noise);
}

fn replace_char_in_line(line: &mut String, char_index: usize, replacement: char) {
    let mut indices = line.char_indices();
    let target = indices.nth(char_index);
    if let Some((start_byte, ch)) = target {
        let end_byte = start_byte + ch.len_utf8();
        let mut replacement_buf = [0u8; 4];
        let replacement_str = replacement.encode_utf8(&mut replacement_buf);
        line.replace_range(start_byte..end_byte, replacement_str);
    }
}

fn frame_with_particles(
    art_lines: Vec<String>,
    art_spans: Vec<StyledSegment>,
    species: Species,
    tick: u64,
) -> (Vec<String>, Vec<StyledSegment>) {
    // Build a 13x10 grid of chars initialized with spaces, then overlay
    // the 11x8 art at rows 1..=8, cols 1..=11.
    let mut grid: Vec<Vec<char>> = (0..FRAME_HEIGHT).map(|_| vec![' '; FRAME_WIDTH]).collect();

    for (row_index, line) in art_lines.iter().enumerate().take(8) {
        let target_row = row_index + 1;
        for (col_index, ch) in line.chars().take(ART_WIDTH).enumerate() {
            grid[target_row][col_index + 1] = ch;
        }
    }

    // Translate art spans to framed-grid spans (line +1, start/end +1).
    let mut framed_spans: Vec<StyledSegment> = art_spans
        .into_iter()
        .map(|span| StyledSegment {
            line: span.line + 1,
            start: span.start + 1,
            end: span.end + 1,
            role: span.role,
        })
        .collect();

    // Overlay particles for this tick.
    for particle in particles_for_species(species, tick) {
        if particle.row < FRAME_HEIGHT && particle.col < FRAME_WIDTH {
            grid[particle.row][particle.col] = particle.glyph;
            framed_spans.push(StyledSegment {
                line: particle.row,
                start: particle.col,
                end: particle.col + 1,
                role: PaletteRoleName::Particle,
            });
        }
    }

    let lines: Vec<String> = grid
        .into_iter()
        .map(|row| row.into_iter().collect::<String>())
        .collect();
    (lines, framed_spans)
}

struct Particle {
    row: usize,
    col: usize,
    glyph: char,
}

fn particles_for_species(species: Species, tick: u64) -> Vec<Particle> {
    let mut particles = Vec::new();
    match species {
        Species::Fuzz => {
            // Tail flick: row 9 col 6, '~' when tick % 23 < 3.
            if tick % 23 < 3 {
                particles.push(Particle {
                    row: 9,
                    col: 6,
                    glyph: '~',
                });
            }
        }
        Species::Blob => {
            if tick % 19 < 3 {
                particles.push(Particle {
                    row: 9,
                    col: 4,
                    glyph: '.',
                });
            }
            if tick % 23 < 4 {
                particles.push(Particle {
                    row: 9,
                    col: 6,
                    glyph: '\u{b0}',
                });
            }
            if tick % 17 < 2 {
                particles.push(Particle {
                    row: 9,
                    col: 8,
                    glyph: '.',
                });
            }
        }
        Species::Ghost => {
            if tick % 13 < 3 {
                particles.push(Particle {
                    row: 0,
                    col: 5,
                    glyph: '~',
                });
            }
            if tick % 19 < 4 {
                particles.push(Particle {
                    row: 9,
                    col: 7,
                    glyph: '\u{b7}',
                });
            }
            if tick % 21 < 2 {
                particles.push(Particle {
                    row: 9,
                    col: 3,
                    glyph: '\'',
                });
            }
            if tick % 17 < 3 {
                particles.push(Particle {
                    row: 0,
                    col: 9,
                    glyph: '.',
                });
            }
        }
        Species::Glitch => {
            // Scan line: only at tick % 41 == 0, draw a single cell.
            if tick.is_multiple_of(41) {
                let row = ((tick / 41) as usize) % FRAME_HEIGHT;
                let col = ((tick / 41) as usize) % FRAME_WIDTH;
                particles.push(Particle {
                    row,
                    col,
                    glyph: '\u{2592}',
                });
            }
            if tick % 11 < 2 {
                particles.push(Particle {
                    row: 0,
                    col: 2,
                    glyph: '\u{2592}',
                });
            }
            if tick % 13 < 2 {
                particles.push(Particle {
                    row: 9,
                    col: 4,
                    glyph: '\u{2591}',
                });
            }
            if tick % 17 < 2 {
                particles.push(Particle {
                    row: 0,
                    col: 10,
                    glyph: '\u{2593}',
                });
            }
        }
        Species::Crystal => {
            if tick % 23 < 3 {
                particles.push(Particle {
                    row: 0,
                    col: 1,
                    glyph: '\u{2727}',
                });
            }
            if tick % 19 < 3 {
                particles.push(Particle {
                    row: 0,
                    col: 11,
                    glyph: '\u{2726}',
                });
            }
            if tick % 21 < 3 {
                particles.push(Particle {
                    row: 9,
                    col: 1,
                    glyph: '\u{2727}',
                });
            }
            if tick % 17 < 2 {
                particles.push(Particle {
                    row: 9,
                    col: 11,
                    glyph: '\u{b7}',
                });
            }
            if tick % 27 < 3 {
                particles.push(Particle {
                    row: 5,
                    col: 0,
                    glyph: '\u{2727}',
                });
            }
        }
        Species::Mech => {
            // LED at row 0 col 6: '●' when tick % 4 < 2 else '○'.
            let led_glyph = if tick % 4 < 2 { '\u{25cf}' } else { '\u{25cb}' };
            particles.push(Particle {
                row: 0,
                col: 6,
                glyph: led_glyph,
            });
            if tick % 9 < 3 {
                particles.push(Particle {
                    row: 0,
                    col: 4,
                    glyph: '~',
                });
            }
            if tick % 11 < 3 {
                particles.push(Particle {
                    row: 0,
                    col: 8,
                    glyph: '\u{b0}',
                });
            }
        }
    }
    particles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::generation::generate_pet;

    #[test]
    fn hold_eyes_closed_renders_closed_blink_eyes_without_touching_mood() {
        let pet = generate_pet("hold-eyes-seed");
        let frame = AnimationFrame {
            tick: 1, // a tick that does NOT blink on its own
            blink_suppression_ticks: 0,
            hold_eyes_closed: true,
            blink_slowdown: 0,
        };
        let rendered = render_pet(&pet, Stage::S3, Mood::Content, frame);
        let art = rendered.lines.join("\n");
        assert!(
            art.contains(closed_blink_eyes(pet.species)),
            "held-closed eyes must use the species closed-blink glyphs, got:\n{art}"
        );
    }

    #[test]
    fn hold_eyes_closed_false_keeps_existing_blink_behavior() {
        let pet = generate_pet("hold-eyes-seed");
        let open = render_pet(
            &pet,
            Stage::S3,
            Mood::Content,
            AnimationFrame {
                tick: 1,
                blink_suppression_ticks: 0,
                hold_eyes_closed: false,
                blink_slowdown: 0,
            },
        );
        assert!(
            open.lines.join("\n").contains(&pet.traits.eyes),
            "non-blinking awake frame keeps the trait eyes"
        );
    }

    #[test]
    fn blink_slowdown_maps_tiredness_zero_to_zero_and_full_to_max() {
        assert_eq!(blink_slowdown_for_tiredness(0.0), 0);
        assert_eq!(blink_slowdown_for_tiredness(1.0), TIRED_BLINK_MAX_SLOWDOWN);
        assert_eq!(
            blink_slowdown_for_tiredness(0.5),
            TIRED_BLINK_MAX_SLOWDOWN / 2
        );
        assert_eq!(blink_slowdown_for_tiredness(7.0), TIRED_BLINK_MAX_SLOWDOWN);
        assert_eq!(blink_slowdown_for_tiredness(-1.0), 0);
    }

    #[test]
    fn blink_cadence_slows_monotonically_with_blink_slowdown() {
        use crate::pet::generation::Species;
        let pet = generate_pet("hold-eyes-seed").with_species(Species::Blob);
        let blink_count = |slowdown: u8| {
            (0..1500_u64)
                .filter(|&tick| {
                    let rendered = render_pet(
                        &pet,
                        Stage::S3,
                        Mood::Content,
                        AnimationFrame {
                            tick,
                            blink_suppression_ticks: 0,
                            hold_eyes_closed: false,
                            blink_slowdown: slowdown,
                        },
                    );
                    rendered
                        .lines
                        .join("\n")
                        .contains(closed_blink_eyes(pet.species))
                })
                .count()
        };
        let rested = blink_count(0);
        let halfway = blink_count(TIRED_BLINK_MAX_SLOWDOWN / 2);
        let exhausted = blink_count(TIRED_BLINK_MAX_SLOWDOWN);
        assert!(rested > 0, "a rested pet blinks");
        assert!(exhausted > 0, "a tired pet still blinks, just slower");
        assert!(
            rested > halfway && halfway > exhausted,
            "cadence must slow monotonically: {rested} > {halfway} > {exhausted}"
        );
    }
}
