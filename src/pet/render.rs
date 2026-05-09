use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::art::{stage_key, stage_label, template_for};
use crate::pet::generation::{GeneratedPet, Species};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame {
    pub tick: u64,
    pub compact: bool,
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
    let stage_key = stage_key(stage);
    let profile = species_animation_profile(pet.species);
    let blinking = should_blink(pet, mood, frame, profile);
    let expression = expression_for(pet, mood, blinking);
    let raw = template_for(
        pet.species,
        stage_key,
        pet.traits.morph_index,
        frame.compact,
    );
    let breath_mark = breath_mark(pet, frame.tick, profile);
    let flavor = flavor_line(pet, frame.tick);
    let mut rendered = raw
        .iter()
        .enumerate()
        .map(|(line_index, line)| render_template_line(line, line_index, pet, &expression))
        .collect::<Vec<_>>();
    let mut lines = rendered
        .iter_mut()
        .map(|line| std::mem::take(&mut line.text))
        .collect::<Vec<_>>();
    let mut spans = rendered
        .into_iter()
        .flat_map(|line| line.spans)
        .collect::<Vec<_>>();

    if !frame.compact {
        let line_index = lines.len();
        lines.push(format!(
            "{breath_mark} {} {}",
            stage_label(pet.species, stage),
            flavor
        ));
        if let Some(line) = lines.last() {
            let end = line.chars().count();
            if end > 0 {
                spans.push(StyledSegment {
                    line: line_index,
                    start: 0,
                    end,
                    role: PaletteRoleName::Body,
                });
            }
        }
    } else {
        let cropped = lines
            .into_iter()
            .enumerate()
            .map(|(line_index, line)| crop_line_and_spans(line, &mut spans, line_index, 18))
            .collect();
        lines = cropped;
    }

    RenderedPet {
        lines,
        spans,
        event_lines: Vec::new(),
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
            eyes: "^ ^".to_string(),
            mouth: "u".to_string(),
        },
        Mood::Content => Expression {
            eyes: pet.traits.eyes.clone(),
            mouth: pet.traits.mouth.clone(),
        },
        Mood::Hungry => Expression {
            eyes: "o o".to_string(),
            mouth: "o".to_string(),
        },
        Mood::Sad => Expression {
            eyes: ". .".to_string(),
            mouth: "_".to_string(),
        },
        Mood::Sleepy => Expression {
            eyes: "u u".to_string(),
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
    let cadence =
        u64::from(profile.blink_average) + (u64::from(pet.animation_phase.blink) % jitter);
    (frame.tick + u64::from(pet.animation_phase.blink)).is_multiple_of(cadence)
}

fn breath_mark(pet: &GeneratedPet, tick: u64, profile: AnimationProfile) -> &'static str {
    let phase = (tick + u64::from(pet.animation_phase.breath)) % u64::from(profile.breath_period);
    if phase < u64::from(profile.breath_hold) {
        "."
    } else if tick.is_multiple_of(2) {
        " "
    } else {
        "'"
    }
}

fn flavor_line(pet: &GeneratedPet, tick: u64) -> String {
    let phase = (tick + u64::from(pet.animation_phase.flavor)) % 7;
    match pet.species {
        Species::Fuzz => format!("tail{}", if phase.is_multiple_of(2) { "\\" } else { "/" }),
        Species::Blob => format!("drip{}", if phase == 0 { "." } else { " " }),
        Species::Ghost => format!("wisp{}", if phase < 3 { "~" } else { " " }),
        Species::Glitch => {
            if phase == 0 {
                "err#".to_string()
            } else {
                "err ".to_string()
            }
        }
        Species::Crystal => format!("spark{}", if phase == 0 { "*" } else { " " }),
        Species::Mech => format!("led{}", if phase < 2 { "*" } else { "." }),
    }
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

fn crop_line_and_spans(
    line: String,
    spans: &mut Vec<StyledSegment>,
    line_index: usize,
    max_width: usize,
) -> String {
    spans.retain_mut(|span| {
        if span.line != line_index {
            return true;
        }
        if span.start >= max_width {
            return false;
        }
        span.end = span.end.min(max_width);
        span.start < span.end
    });
    line.chars().take(max_width).collect()
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
