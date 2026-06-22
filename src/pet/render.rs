use crate::game::evolution::Stage;
use crate::game::metabolism::Mood;
use crate::pet::art::stage_template_lines;
use crate::pet::generation::{GeneratedPet, Species};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkAccent {
    #[default]
    None,
    /// Output-heavy bursts: brighter, sharper eyes.
    Alert,
    /// Reasoning-heavy: narrowed, focused eyes.
    Focused,
    /// Cache-heavy: softer, dreamier eyes.
    Dreamy,
}

/// Map live work shape to a subtle expression accent. Returns `WorkAccent::None`
/// unless work is actually flowing (activity gate), so a stale weather never
/// lingers on an idle pet — keeping this on the texture side of the locked
/// boundary.
pub fn work_accent_for(weather: crate::tui::life::WorkWeather, activity_level: f32) -> WorkAccent {
    use crate::tui::life::WorkWeather::*;
    if activity_level < 0.3 {
        return WorkAccent::None;
    }
    match weather {
        OutputSparks | Mixed => WorkAccent::Alert,
        ReasoningPulse => WorkAccent::Focused,
        CacheMist => WorkAccent::Dreamy,
        Clear => WorkAccent::None,
    }
}

/// Compute work accent from a live life profile. Calm mode forces no accent.
pub fn work_accent_for_profile(profile: &crate::tui::life::PetLifeProfile) -> WorkAccent {
    let activity = if profile.calm_mode {
        0.0
    } else {
        profile.activity_level
    };
    work_accent_for(profile.work_weather, activity)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
    /// Sleep presentation: force the species closed-blink eyes. Must never be
    /// implemented by substituting Mood::Sleepy — mood is the vitals contract.
    pub hold_eyes_closed: bool,
    /// Ticks added to the species blink cadence (tiredness slows blinking).
    /// 0 = normal. Producers map tiredness 0..1 -> 0..TIRED_BLINK_MAX_SLOWDOWN.
    pub blink_slowdown: u8,
    /// Relax the eyes for tired/cozy performance (B). Inert for closed/blink.
    pub soft_eyes: bool,
    /// Subtle work-type expression accent (E). Applied only to positive moods.
    pub work_accent: WorkAccent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPet {
    pub lines: Vec<String>,
    pub spans: Vec<StyledSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    Corruption,
}

/// Per-species gutter identity for the 13x10 frame's gutter rows (0 and 9) and
/// side columns. Data, not an architecture fork. Phase 1 uses it only for the
/// S6 sparkle move; Phase 5 reads it when compositing the contact shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterContent {
    Sparkle,
    #[allow(dead_code)] // MachineFrame: reserved for a future Mech gutter overlay (Phase 5).
    MachineFrame,
    None,
}

/// S6 earns a gutter sparkle for every species except Mech, which keeps its own
/// chassis art rows (decision: Mech-S6 gutter == None). Below S6 there is no
/// gutter sparkle. `MachineFrame` is reserved for a future Mech gutter overlay
/// and is unused in Phase 1.
fn gutter_content_for(species: Species, stage: Stage) -> GutterContent {
    match (species, stage) {
        (Species::Mech, Stage::S6) => GutterContent::None,
        (_, Stage::S6) => GutterContent::Sparkle,
        _ => GutterContent::None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationProfile {
    pub blink_average: u8,
    pub blink_jitter: u8,
}

const ART_WIDTH: usize = 11;
const FRAME_WIDTH: usize = 13;
const FRAME_HEIGHT: usize = 10;

const GLITCH_NOISE: &[char] = &[
    '\u{2592}', '\u{2591}', '\u{2593}', '\u{2580}', '\u{2584}', '\u{258c}', '\u{2590}',
];

/// Corruption fires on a periodic gate so it reads as a calm, intentional
/// data-glitch rather than a constant flicker.
const CORRUPTION_GATE_TICKS: u64 = 13;
/// Bounded footprint: at most this many cells corrupt on an active tick.
const CORRUPTION_MAX_CELLS: usize = 3;

pub fn render_pet(
    pet: &GeneratedPet,
    stage: Stage,
    mood: Mood,
    frame: AnimationFrame,
) -> RenderedPet {
    let profile = species_animation_profile(pet.species);
    let blinking = frame.hold_eyes_closed || should_blink(pet, mood, frame, profile);
    let expression = expression_for(pet, mood, blinking, frame);
    let raw = stage_template_lines(pet.species, stage, u64::from(pet.traits.seed_hue));
    let rendered = raw
        .iter()
        .enumerate()
        .map(|(line_index, line)| render_template_line(line.as_str(), line_index, pet, &expression))
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
    let (framed_lines, framed_spans) =
        frame_with_particles(lines, spans, pet.species, stage, frame.tick);

    RenderedPet {
        lines: framed_lines,
        spans: framed_spans,
    }
}

pub fn species_animation_profile(species: Species) -> AnimationProfile {
    match species {
        Species::Fuzz => AnimationProfile {
            blink_average: 32,
            blink_jitter: 12,
        },
        Species::Blob => AnimationProfile {
            blink_average: 40,
            blink_jitter: 14,
        },
        Species::Ghost => AnimationProfile {
            blink_average: 50,
            blink_jitter: 18,
        },
        Species::Glitch => AnimationProfile {
            blink_average: 24,
            blink_jitter: 8,
        },
        Species::Crystal => AnimationProfile {
            blink_average: 60,
            blink_jitter: 22,
        },
        Species::Mech => AnimationProfile {
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

struct Expression {
    eyes: String,
    mouth: String,
}

/// Per-species mood-face vocabulary for the six non-Content moods. Content reads
/// from the per-seed `traits.eyes`/`mouth` (handled in `expression_for`). All
/// glyphs are width-1 under ambiguous=narrow; eyes occupy exactly 3 columns,
/// mouth exactly 1. Phase 3 measures resting-eye contrast against the Content
/// face; this covers the expressive moods so they read per-species rather than
/// as one shared set.
fn mood_face(species: Species, mood: Mood) -> Expression {
    let mk = |eyes: &str, mouth: &str| Expression {
        eyes: eyes.to_string(),
        mouth: mouth.to_string(),
    };
    match species {
        Species::Fuzz | Species::Blob | Species::Ghost => match mood {
            Mood::Happy => mk("^.^", "\u{03c9}"),    // ^.^ / ω
            Mood::Ecstatic => mk("*o*", "\u{25bd}"), // *o* / ▽
            Mood::Hungry => mk("u.u", "o"),
            Mood::Sad => mk("T.T", "\u{2322}"), // T.T / ⌢
            Mood::Sleepy => mk("-.-", "-"),
            Mood::Wilted => mk(",_,", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Glitch => match mood {
            // Daemon lens face: alive, never corpse. ◉ is EAW-Neutral (width-1).
            Mood::Happy => mk(">\u{25c9}<", "\u{02c4}"), // >◉< / ˄
            Mood::Ecstatic => mk("\u{25c9}o\u{25c9}", "\u{25bd}"),
            Mood::Hungry => mk("o\u{25c9}o", "o"),
            Mood::Sad => mk("v\u{25c9}v", "\u{2322}"), // ⌢
            Mood::Sleepy => mk("-.-", "_"),
            Mood::Wilted => mk("x_x", "_"), // wilted may dim the lens
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Crystal => match mood {
            // Facet eyes; ◇ is ambiguous-narrow (kept per the Crystal decision).
            Mood::Happy => mk("\u{25c7}^\u{25c7}", "v"),
            Mood::Ecstatic => mk("*\u{25c7}*", "\u{25bd}"),
            Mood::Hungry => mk("\u{25c7}.\u{25c7}", "o"),
            Mood::Sad => mk("\u{25c7}_\u{25c7}", "\u{2322}"), // ⌢
            Mood::Sleepy => mk("-.-", "_"),
            Mood::Wilted => mk(",_,", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Mech => match mood {
            // Optic-sensor face: square/bracket eyes read mechanical.
            Mood::Happy => mk("^=^", "v"),
            Mood::Ecstatic => mk("*o*", "\u{25bd}"),
            Mood::Hungry => mk("o=o", "o"),
            Mood::Sad => mk("v=v", "\u{2322}"), // ⌢
            Mood::Sleepy => mk("=.=", "_"),
            Mood::Wilted => mk("x_x", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
    }
}

fn expression_for(
    pet: &GeneratedPet,
    mood: Mood,
    blinking: bool,
    frame: AnimationFrame,
) -> Expression {
    if blinking {
        return Expression {
            eyes: closed_blink_eyes(pet.species).to_string(),
            mouth: pet.traits.mouth.clone(),
        };
    }

    let mut expr = match mood {
        Mood::Content => Expression {
            eyes: pet.traits.eyes.clone(),
            mouth: pet.traits.mouth.clone(),
        },
        other => mood_face(pet.species, other),
    };
    if frame.soft_eyes && matches!(mood, Mood::Content | Mood::Happy) {
        expr.eyes = "\u{02d8}.\u{02d8}".to_string(); // ˘.˘ relaxed, heavy-lidded
    }
    if matches!(mood, Mood::Happy | Mood::Content) {
        match frame.work_accent {
            WorkAccent::None => {}
            WorkAccent::Alert => expr.eyes = "^o^".to_string(),
            WorkAccent::Focused => expr.eyes = ">.<".to_string(),
            WorkAccent::Dreamy => expr.eyes = "u.u".to_string(),
        }
    }
    expr
}

fn should_blink(
    pet: &GeneratedPet,
    mood: Mood,
    frame: AnimationFrame,
    profile: AnimationProfile,
) -> bool {
    if matches!(
        mood,
        Mood::Sad | Mood::Sleepy | Mood::Wilted | Mood::Ecstatic
    ) {
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

/// Deterministic, bounded set of non-blank art cells to corrupt this tick.
/// Returns empty when the periodic gate is closed (calm). Selection walks the
/// non-blank cells of the 8 art rows and picks up to CORRUPTION_MAX_CELLS via
/// a tick-seeded stride, so the footprint reshuffles across base/edge/face
/// cells over time without ever exceeding the cap.
fn corruption_cells_for_tick(art_lines: &[String], tick: u64) -> Vec<(usize, usize)> {
    if !tick.is_multiple_of(CORRUPTION_GATE_TICKS) {
        return Vec::new();
    }
    // Collect every non-blank cell of the art rows.
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for (row, line) in art_lines.iter().enumerate().take(8) {
        for (col, ch) in line.chars().enumerate().take(ART_WIDTH) {
            if ch != ' ' {
                candidates.push((row, col));
            }
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    let count = CORRUPTION_MAX_CELLS.min(candidates.len());
    // Tick-seeded start + odd stride so successive active ticks land on
    // different cells (reshuffle) while staying deterministic per tick.
    let n = candidates.len();
    let gate = tick / CORRUPTION_GATE_TICKS;
    let start = (gate.wrapping_mul(7) as usize) % n;
    let stride = ((gate.wrapping_mul(11) as usize) % n) | 1; // always odd, >=1
    let mut out = Vec::with_capacity(count);
    let mut idx = start;
    for _ in 0..count {
        out.push(candidates[idx % n]);
        idx += stride;
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The living-face rule: corruption may briefly touch the face but NEVER the
/// eye-center. Returns true when (row, col) is the middle cell of an Eye span
/// on that row. For even-width eye spans, both middle cells are protected.
fn is_eye_center(spans: &[StyledSegment], row: usize, col: usize) -> bool {
    spans.iter().any(|span| {
        if span.line != row || span.role != PaletteRoleName::Eye {
            return false;
        }
        let width = span.end.saturating_sub(span.start);
        if width == 0 {
            return false;
        }
        let mid_lo = span.start + (width - 1) / 2;
        let mid_hi = span.start + width / 2;
        col >= mid_lo && col <= mid_hi
    })
}

/// Glitch corruption: a bounded, deterministic, loud data-glitch effect.
///
/// On a periodic gate (calm — not every frame) it corrupts up to
/// CORRUPTION_MAX_CELLS non-blank art cells. Each corrupted cell:
///
///   * swaps its glyph to a contrasting GLITCH_NOISE glyph (may be ▒▓ weight),
///   * is re-tagged as the Corruption role by SPLITTING the underlying span,
///     so corruption wins z-order over Eye/Mouth/Body at that cell.
///
/// The eye-center is never corrupted (living-face rule).
fn apply_glitch_corruption(lines: &mut [String], spans: &mut Vec<StyledSegment>, tick: u64) {
    let cells = corruption_cells_for_tick(lines, tick);
    if cells.is_empty() {
        return;
    }
    for (i, (row, col)) in cells.into_iter().enumerate() {
        if is_eye_center(spans, row, col) {
            continue; // living-face: never the eye-center
        }
        // Pick a noise glyph deterministically per (tick, cell index).
        let noise =
            GLITCH_NOISE[((tick as usize).wrapping_mul(3).wrapping_add(i)) % GLITCH_NOISE.len()];
        replace_char_in_line(&mut lines[row], col, noise);
        retag_cell_as_corruption(spans, row, col);
    }
}

/// Re-tag the single cell (row, col) as the Corruption role. If an existing
/// span covers the cell, it is split around the cell so the surrounding cells
/// keep their original role and the cell itself becomes a width-1 Corruption
/// span. If no span covers the cell, a standalone Corruption span is added.
fn retag_cell_as_corruption(spans: &mut Vec<StyledSegment>, row: usize, col: usize) {
    let mut split: Vec<StyledSegment> = Vec::new();
    for span in spans.iter_mut() {
        if span.line != row || col < span.start || col >= span.end {
            continue;
        }
        let original_end = span.end;
        let original_role = span.role;
        // Left fragment: keep original role for [start, col).
        // Shrink this span to the left fragment (possibly empty -> filtered later).
        span.end = col;
        // Right fragment: keep original role for (col, end).
        if col + 1 < original_end {
            split.push(StyledSegment {
                line: row,
                start: col + 1,
                end: original_end,
                role: original_role,
            });
        }
        break;
    }
    // The corrupted cell itself. If no span covered the cell (a body-gap cell),
    // this standalone Corruption span overrides that single cell for the consumer.
    split.push(StyledSegment {
        line: row,
        start: col,
        end: col + 1,
        role: PaletteRoleName::Corruption,
    });
    // Drop any now-empty left fragments produced by the shrink.
    spans.retain(|s| s.start < s.end);
    spans.extend(split);
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
    stage: Stage,
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

    // S6 gutter sparkle (row 0 only) — precedence: species identity particles
    // (painted just below) outrank it; the contact shadow (Phase 5, row 9) never
    // collides with row 0. Same glyph as the outer-frame S6 fill so the surfaces
    // agree.
    if gutter_content_for(species, stage) == GutterContent::Sparkle {
        const SPARKLE_COLS: [usize; 3] = [2, 6, 10];
        for col in SPARKLE_COLS {
            grid[0][col] = '\u{2726}';
            framed_spans.push(StyledSegment {
                line: 0,
                start: col,
                end: col + 1,
                role: PaletteRoleName::Particle,
            });
        }
    }

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

/// Lowest non-blank art row of the rendered 8 rows = the silhouette's "feet".
/// Templates carry trailing blank rows, so this finds the true bottom of the
/// creature. Phase 5 restricts the contact shadow to the columns beneath it.
pub(crate) fn feet_row(art_lines: &[String]) -> Option<usize> {
    art_lines
        .iter()
        .enumerate()
        .rev()
        .find(|(_, line)| line.chars().any(|c| c != ' '))
        .map(|(row, _)| row)
}

/// Non-space columns of the feet row (the contact-shadow footprint).
pub(crate) fn feet_columns(art_lines: &[String]) -> Vec<usize> {
    match feet_row(art_lines) {
        None => Vec::new(),
        Some(row) => art_lines[row]
            .chars()
            .enumerate()
            .filter(|(_, c)| *c != ' ')
            .map(|(col, _)| col)
            .collect(),
    }
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
                    glyph: ':',
                });
            }
            if tick % 11 < 2 {
                particles.push(Particle {
                    row: 0,
                    col: 2,
                    glyph: '\u{b7}',
                });
            }
            if tick % 13 < 2 {
                particles.push(Particle {
                    row: 9,
                    col: 4,
                    glyph: '.',
                });
            }
            if tick % 17 < 2 {
                particles.push(Particle {
                    row: 0,
                    col: 10,
                    glyph: ':',
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
    fn work_accent_from_weather_gates_on_live_work() {
        use crate::tui::life::WorkWeather;
        // Idle: no accent regardless of weather.
        assert_eq!(
            work_accent_for(WorkWeather::OutputSparks, 0.0),
            WorkAccent::None
        );
        // Live work maps weather to accent.
        assert_eq!(
            work_accent_for(WorkWeather::OutputSparks, 0.9),
            WorkAccent::Alert
        );
        assert_eq!(
            work_accent_for(WorkWeather::ReasoningPulse, 0.9),
            WorkAccent::Focused
        );
        assert_eq!(
            work_accent_for(WorkWeather::CacheMist, 0.9),
            WorkAccent::Dreamy
        );
        assert_eq!(work_accent_for(WorkWeather::Mixed, 0.9), WorkAccent::Alert);
        assert_eq!(work_accent_for(WorkWeather::Clear, 0.9), WorkAccent::None);
    }

    #[test]
    fn soft_eyes_relax_a_positive_mood_without_changing_mouth() {
        let pet = generate_pet("soft-eyes-seed");
        let normal = AnimationFrame {
            tick: 1,
            blink_suppression_ticks: 0,
            hold_eyes_closed: false,
            blink_slowdown: 0,
            soft_eyes: false,
            work_accent: WorkAccent::None,
        };
        let soft = AnimationFrame {
            soft_eyes: true,
            ..normal
        };
        let a = render_pet(&pet, Stage::S3, Mood::Content, normal)
            .lines
            .join("\n");
        let b = render_pet(&pet, Stage::S3, Mood::Content, soft)
            .lines
            .join("\n");
        assert_ne!(a, b, "soft eyes should change the rendered face");
        assert!(
            b.contains("\u{02d8}.\u{02d8}"),
            "soft eyes should appear in rendered face, got:\n{b}"
        );
        assert!(
            a.contains(&pet.traits.mouth),
            "normal render should keep the mouth, got:\n{a}"
        );
        assert!(
            b.contains(&pet.traits.mouth),
            "soft eyes should not change the mouth, got:\n{b}"
        );
    }

    #[test]
    fn work_accent_sharpens_positive_moods_and_ignored_for_negative() {
        let pet = generate_pet("accent-seed");
        let base = AnimationFrame {
            tick: 2,
            ..AnimationFrame::default()
        };
        for accent in [WorkAccent::Alert, WorkAccent::Focused, WorkAccent::Dreamy] {
            let accented = AnimationFrame {
                work_accent: accent,
                ..base
            };
            // Positive mood: accent changes the face.
            let happy_plain = render_pet(&pet, Stage::S3, Mood::Happy, base)
                .lines
                .join("\n");
            let happy_accented = render_pet(&pet, Stage::S3, Mood::Happy, accented)
                .lines
                .join("\n");
            assert_ne!(
                happy_plain, happy_accented,
                "{accent:?} accent should change a happy face"
            );
            // Negative mood: accent is ignored (honest face).
            let sad_plain = render_pet(&pet, Stage::S3, Mood::Sad, base)
                .lines
                .join("\n");
            let sad_accented = render_pet(&pet, Stage::S3, Mood::Sad, accented)
                .lines
                .join("\n");
            assert_eq!(
                sad_plain, sad_accented,
                "{accent:?} accent should be ignored on a sad pet"
            );
        }
    }

    #[test]
    fn hold_eyes_closed_renders_closed_blink_eyes_without_touching_mood() {
        let pet = generate_pet("hold-eyes-seed");
        let frame = AnimationFrame {
            tick: 1, // a tick that does NOT blink on its own
            blink_suppression_ticks: 0,
            hold_eyes_closed: true,
            blink_slowdown: 0,
            soft_eyes: false,
            work_accent: WorkAccent::None,
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
                soft_eyes: false,
                work_accent: WorkAccent::None,
            },
        );
        assert!(
            open.lines.join("\n").contains(&pet.traits.eyes),
            "non-blinking awake frame keeps the trait eyes"
        );
    }

    #[test]
    fn ecstatic_renders_the_star_eyes_and_blocks_blink() {
        let pet = generate_pet("ecstatic-seed");
        let frame = AnimationFrame {
            tick: 1,
            ..AnimationFrame::default()
        };
        let art = render_pet(&pet, Stage::S4, Mood::Ecstatic, frame)
            .lines
            .join("\n");
        assert!(art.contains("*o*"), "ecstatic uses star eyes, got:\n{art}");
        let profile = species_animation_profile(pet.species);
        assert!(
            !should_blink(&pet, Mood::Ecstatic, frame, profile),
            "ecstatic mood should block blinking"
        );
    }

    #[test]
    fn ecstatic_keeps_star_eyes_when_work_accent_is_dreamy() {
        let pet = generate_pet("ecstatic-dreamy-seed");
        let frame = AnimationFrame {
            tick: 1,
            work_accent: WorkAccent::Dreamy,
            ..AnimationFrame::default()
        };

        let art = render_pet(&pet, Stage::S4, Mood::Ecstatic, frame)
            .lines
            .join("\n");

        assert!(
            art.contains("*o*"),
            "ecstatic must stay visibly ecstatic, got:\n{art}"
        );
        assert!(
            !art.contains("u.u"),
            "dreamy work accent must not make an ecstatic pet look sleepy, got:\n{art}"
        );
    }

    #[test]
    fn glitch_particles_stay_punctuation_sized() {
        let particles = particles_for_species(Species::Glitch, 1);

        assert!(
            particles
                .iter()
                .all(|particle| !matches!(particle.glyph, '▒' | '▓')),
            "Glitch particles should be light punctuation, got {:?}",
            particles
                .iter()
                .map(|particle| particle.glyph)
                .collect::<Vec<_>>()
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
    fn species_particle_outranks_s6_sparkle_on_a_contested_cell() {
        use crate::pet::generation::generate_pet;
        let pet = generate_pet("contested").with_species(Species::Glitch);
        // tick 0: tick % 17 == 0 < 2, so Glitch paints ':' at row 0 col 10, which
        // is also a sparkle column. The species particle must win.
        let rendered = render_pet(&pet, Stage::S6, Mood::Content, AnimationFrame::default());
        let row0: Vec<char> = rendered.lines[0].chars().collect();
        assert_eq!(
            row0[10], ':',
            "Glitch row-0 particle at col 10 must outrank the S6 sparkle, got {:?}",
            row0[10]
        );
    }

    #[test]
    fn s6_sparkle_is_in_gutter_row_zero_not_art_rows() {
        use crate::pet::generation::generate_pet;
        // Force a Sparkle species at S6.
        let pet = generate_pet("s6-sparkle").with_species(Species::Crystal);
        // tick 0 keeps animation deterministic.
        let rendered = render_pet(&pet, Stage::S6, Mood::Content, AnimationFrame::default());
        // Framed grid is 10 rows tall; art occupies framed rows 1..=8, gutter is
        // rows 0 and 9. The S6 sparkle ('✦') must appear only in framed row 0.
        let row0 = &rendered.lines[0];
        assert!(
            row0.contains('\u{2726}'),
            "S6 gutter row 0 must carry the sparkle, got: {row0:?}"
        );
        // Pin the sparkle to the specific gutter columns (2 and 6) no Crystal
        // particle occupies at tick 0 (Crystal's row-0 particle is col 11), so
        // this is a real emission guard: without the gutter code these cells are
        // spaces. Col 10 is contested by Glitch and validated in the precedence
        // test below, so it is excluded here.
        let row0_chars: Vec<char> = row0.chars().collect();
        assert_eq!(
            row0_chars[2], '\u{2726}',
            "S6 gutter sparkle must paint col 2, got: {row0:?}"
        );
        assert_eq!(
            row0_chars[6], '\u{2726}',
            "S6 gutter sparkle must paint col 6, got: {row0:?}"
        );
        for (i, line) in rendered.lines.iter().enumerate().skip(1) {
            assert!(
                !line.contains('\u{2726}'),
                "S6 sparkle must not appear in framed row {i} (art/bottom gutter): {line:?}"
            );
        }
    }

    #[test]
    fn gutter_content_is_sparkle_at_s6_except_mech() {
        use crate::pet::generation::Species;
        for species in Species::all() {
            // Below S6: no gutter sparkle.
            assert_eq!(
                gutter_content_for(species, Stage::S5),
                GutterContent::None,
                "{species:?} S5 must have no gutter sparkle"
            );
        }
        // S6: sparkle for everyone except Mech (keeps its chassis art rows).
        for species in [
            Species::Fuzz,
            Species::Blob,
            Species::Ghost,
            Species::Glitch,
            Species::Crystal,
        ] {
            assert_eq!(
                gutter_content_for(species, Stage::S6),
                GutterContent::Sparkle
            );
        }
        assert_eq!(
            gutter_content_for(Species::Mech, Stage::S6),
            GutterContent::None
        );
    }

    #[test]
    fn feet_row_is_lowest_non_blank_art_row() {
        let art: Vec<String> = vec![
            "    ░░░    ".to_string(), // row 0
            "   ░▒▒▒░   ".to_string(), // row 1 (widest, but not lowest)
            "    d b    ".to_string(), // row 2 lowest non-blank
            "           ".to_string(), // row 3 blank
        ];
        assert_eq!(feet_row(&art), Some(2));
        // Columns of the lowest non-blank row that are non-space:
        assert_eq!(feet_columns(&art), vec![4, 6]);
    }

    #[test]
    fn feet_row_none_for_all_blank() {
        let art: Vec<String> = vec!["           ".to_string(); 3];
        assert_eq!(feet_row(&art), None);
        assert!(feet_columns(&art).is_empty());
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
                            soft_eyes: false,
                            work_accent: WorkAccent::None,
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

    #[test]
    fn mood_faces_are_species_specific_and_width_correct() {
        use crate::pet::generation::Species;
        use unicode_width::UnicodeWidthStr;
        let non_content = [
            Mood::Happy,
            Mood::Ecstatic,
            Mood::Hungry,
            Mood::Sad,
            Mood::Sleepy,
            Mood::Wilted,
        ];
        for species in Species::all() {
            for mood in non_content {
                let face = mood_face(species, mood);
                assert_eq!(
                    UnicodeWidthStr::width(face.eyes.as_str()),
                    3,
                    "{species:?} {mood:?} eyes must be 3 cols, got {:?}",
                    face.eyes
                );
                assert_eq!(
                    UnicodeWidthStr::width(face.mouth.as_str()),
                    1,
                    "{species:?} {mood:?} mouth must be 1 col, got {:?}",
                    face.mouth
                );
            }
        }
        // Species differentiation: at least one species' happy eyes differs from
        // another's (the vocabulary is not one shared set).
        assert_ne!(
            mood_face(Species::Glitch, Mood::Happy).eyes,
            mood_face(Species::Fuzz, Mood::Happy).eyes,
            "mood vocabulary must vary by species"
        );
    }

    #[test]
    fn soft_bodies_read_as_figure_with_color_off() {
        use crate::pet::generation::generate_pet;
        // Flat color strips hue; legibility must come from dense glyphs. The
        // body of a soft species must contain enough medium/dark shade cells
        // (▒▓█) to read as a solid creature, not a sparse dot cloud.
        let dense: &[char] = &['\u{2592}', '\u{2593}', '\u{2588}']; // ▒ ▓ █
        for species in [Species::Blob, Species::Ghost] {
            let pet = generate_pet("figure-ground-seed").with_species(species);
            // S4 is the first mass stage; soft bodies must already read solid.
            let rendered = render_pet(&pet, Stage::S4, Mood::Content, AnimationFrame::default());
            let dense_cells = rendered
                .lines
                .iter()
                .flat_map(|l| l.chars())
                .filter(|c| dense.contains(c))
                .count();
            assert!(
                dense_cells >= 6,
                "{species:?} S4 body has only {dense_cells} dense (▒▓█) cells; \
                 a soft body must read as figure with color off"
            );
        }
    }

    #[test]
    fn animation_profile_has_no_breath_fields() {
        // Construction must compile with ONLY blink fields — proves the dead
        // divergent breath table is gone and animator.rs owns breath alone.
        let p = AnimationProfile {
            blink_average: 30,
            blink_jitter: 10,
        };
        assert_eq!(p.blink_average, 30);
        assert_eq!(p.blink_jitter, 10);
    }

    #[test]
    fn corruption_gate_is_closed_most_ticks() {
        let lines: Vec<String> = vec!["###########".to_string(); 8];
        let active = (0..130_u64)
            .filter(|&t| !corruption_cells_for_tick(&lines, t).is_empty())
            .count();
        // Calm: corruption is the quiet exception, not the rule.
        assert!(active > 0, "corruption must fire sometimes");
        assert!(
            active <= 130 / CORRUPTION_GATE_TICKS as usize + 1,
            "corruption fired {active}/130 ticks — too noisy"
        );
    }

    #[test]
    fn corruption_footprint_is_bounded() {
        let lines: Vec<String> = vec!["###########".to_string(); 8];
        for t in 0..200_u64 {
            let cells = corruption_cells_for_tick(&lines, t);
            assert!(
                cells.len() <= CORRUPTION_MAX_CELLS,
                "tick {t} produced {} cells, over the cap",
                cells.len()
            );
        }
    }

    #[test]
    fn corruption_cells_are_deterministic() {
        let lines: Vec<String> = vec!["#### ## ####".chars().take(11).collect(); 8];
        let a = corruption_cells_for_tick(&lines, 26);
        let b = corruption_cells_for_tick(&lines, 26);
        assert_eq!(a, b, "same tick must give same cells");
    }

    #[test]
    fn corruption_cells_skip_blank_cells_and_stay_in_bounds() {
        // Sparse art: only a few non-space cells. Selector must never pick a
        // space cell or an out-of-grid cell.
        let lines: Vec<String> = vec![
            "   ###     ".to_string(),
            "           ".to_string(),
            "  #     #  ".to_string(),
            "           ".to_string(),
            "           ".to_string(),
            "           ".to_string(),
            "           ".to_string(),
            "           ".to_string(),
        ];
        for t in 0..300_u64 {
            for (row, col) in corruption_cells_for_tick(&lines, t) {
                assert!(row < lines.len(), "row {row} out of bounds at tick {t}");
                let ch = lines[row].chars().nth(col).unwrap();
                assert_ne!(ch, ' ', "tick {t} picked a blank cell ({row},{col})");
            }
        }
    }

    #[test]
    fn corruption_cells_empty_for_all_blank_art() {
        let lines: Vec<String> = vec!["           ".to_string(); 8];
        for t in 0..40_u64 {
            assert!(
                corruption_cells_for_tick(&lines, t).is_empty(),
                "blank art must never corrupt (tick {t})"
            );
        }
    }

    #[test]
    fn is_eye_center_is_true_only_at_the_middle_of_an_eye_span() {
        // Eye span on row 2 covering cols 3..6 ("o o"): center is col 4.
        let spans = vec![StyledSegment {
            line: 2,
            start: 3,
            end: 6,
            role: PaletteRoleName::Eye,
        }];
        assert!(is_eye_center(&spans, 2, 4), "col 4 is the eye-center");
        assert!(
            !is_eye_center(&spans, 2, 3),
            "col 3 is an eye edge, not center"
        );
        assert!(
            !is_eye_center(&spans, 2, 5),
            "col 5 is an eye edge, not center"
        );
        assert!(!is_eye_center(&spans, 1, 4), "wrong row");
        assert!(!is_eye_center(&spans, 2, 7), "outside the span");
    }

    /// At a tick where the corruption gate is open, a Glitch pet shows at least
    /// one Corruption-role span over a cell that was previously Eye or Mouth or
    /// Body — proving corruption is loud (recolored) and z-wins, not a silent
    /// in-place glyph edit.
    #[test]
    fn glitch_corruption_emits_corruption_role_spans_on_active_tick() {
        let pet = generate_pet("corrupt-seed").with_species(Species::Glitch);
        // CORRUPTION_GATE_TICKS == 13; tick 13 opens the gate.
        let frame = AnimationFrame {
            tick: 13,
            ..AnimationFrame::default()
        };
        let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
        let has_corruption = rendered
            .spans
            .iter()
            .any(|s| s.role == PaletteRoleName::Corruption);
        assert!(
            has_corruption,
            "active corruption tick must emit Corruption-role spans"
        );
    }

    #[test]
    fn glitch_corruption_never_recolors_the_eye_center() {
        let pet = generate_pet("corrupt-eye-seed").with_species(Species::Glitch);
        // Scan many gate-open ticks; the eye-center must never become Corruption.
        for gate in 1..400_u64 {
            let tick = gate * CORRUPTION_GATE_TICKS;
            let frame = AnimationFrame {
                tick,
                ..AnimationFrame::default()
            };
            // Re-derive the pre-frame spans the same way render_pet does, then
            // run the public render and check no Corruption span lands on an
            // eye-center cell (in the pre-frame coordinate space).
            let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
            // Find the Eye spans (post-frame coords: render adds +1 to line/col).
            let eye_spans: Vec<_> = rendered
                .spans
                .iter()
                .filter(|s| s.role == PaletteRoleName::Eye)
                .cloned()
                .collect();
            assert!(
                !eye_spans.is_empty(),
                "tick {tick}: Glitch S4 must always render eye spans"
            );
            for c in rendered
                .spans
                .iter()
                .filter(|s| s.role == PaletteRoleName::Corruption)
            {
                for e in &eye_spans {
                    if c.line == e.line {
                        let width = e.end.saturating_sub(e.start);
                        let mid_lo = e.start + (width.saturating_sub(1)) / 2;
                        let mid_hi = e.start + width / 2;
                        // Corruption span is width-1; its start is the cell.
                        let col = c.start;
                        assert!(
                            !(col >= mid_lo && col <= mid_hi),
                            "tick {tick}: corruption hit the eye-center at line {} col {col}",
                            c.line
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn non_glitch_species_never_corrupt() {
        for species in [
            Species::Fuzz,
            Species::Blob,
            Species::Ghost,
            Species::Crystal,
            Species::Mech,
        ] {
            let pet = generate_pet("no-corrupt").with_species(species);
            let frame = AnimationFrame {
                tick: 13,
                ..AnimationFrame::default()
            };
            let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
            assert!(
                !rendered
                    .spans
                    .iter()
                    .any(|s| s.role == PaletteRoleName::Corruption),
                "{species:?} must never corrupt"
            );
        }
    }

    #[test]
    fn glitch_corruption_quiet_off_gate_tick() {
        let pet = generate_pet("corrupt-seed").with_species(Species::Glitch);
        // Tick 1 is not a multiple of CORRUPTION_GATE_TICKS (13): gate closed.
        let frame = AnimationFrame {
            tick: 1,
            ..AnimationFrame::default()
        };
        let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
        assert!(
            !rendered
                .spans
                .iter()
                .any(|s| s.role == PaletteRoleName::Corruption),
            "off-gate tick must stay calm (no corruption)"
        );
    }
}
