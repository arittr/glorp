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
pub struct GlitchCorruptionFrame {
    pub day_seed: u64,
    pub patch_tier: GlitchPatchTier,
    pub burst_level: GlitchBurstLevel,
    pub calm_mode: bool,
    pub feed_reaction: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlitchPatchTier {
    Pristine,
    #[default]
    Quiet,
    Active,
    Heavy,
}

impl GlitchPatchTier {
    pub const fn max_marks(self) -> usize {
        match self {
            Self::Pristine => 0,
            Self::Quiet => 1,
            Self::Active => 2,
            Self::Heavy => 3,
        }
    }

    pub fn from_today_ratio(today_ratio: f32) -> Self {
        if !today_ratio.is_finite() {
            return Self::Quiet;
        }
        if today_ratio >= 1.5 {
            Self::Heavy
        } else if today_ratio >= 0.75 {
            Self::Active
        } else {
            Self::Quiet
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pristine => "pristine",
            Self::Quiet => "quiet",
            Self::Active => "active",
            Self::Heavy => "heavy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlitchBurstLevel {
    #[default]
    None,
    Small,
    Strong,
}

impl GlitchBurstLevel {
    pub fn from_burst_level(burst_level: f32) -> Self {
        if !burst_level.is_finite() || burst_level <= 0.2 {
            Self::None
        } else if burst_level < 0.7 {
            Self::Small
        } else {
            Self::Strong
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Small => "small",
            Self::Strong => "strong",
        }
    }
}

pub fn glitch_corruption_frame_for_inputs(
    day_seed: u64,
    today_ratio: f32,
    burst_level: f32,
    calm_mode: bool,
    feed_reaction: bool,
) -> GlitchCorruptionFrame {
    GlitchCorruptionFrame {
        day_seed,
        patch_tier: GlitchPatchTier::from_today_ratio(today_ratio),
        burst_level: GlitchBurstLevel::from_burst_level(burst_level),
        calm_mode,
        feed_reaction,
    }
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
    /// The pet was freshly fed (a recent usage pulse). Briefly perks the face into
    /// an excited beat, regardless of resting mood — set by producers from the feed
    /// pulse recency. Inert while blinking or asleep.
    pub feed_reaction: bool,
    /// Glitch-species persistent corruption state for this frame. `None` for
    /// non-Glitch species or when no corruption data is available yet.
    pub glitch_corruption: Option<GlitchCorruptionFrame>,
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
    /// The brightest body cells (`█`): a lighter tint of the body color so the
    /// silhouette reads with luminance depth, not just ink density.
    BodyGlow,
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

/// Repair-mark glyphs: quiet, soldered-looking marks (not the loud noise
/// blocks used by active corruption).
const GLITCH_REPAIR_GLYPHS: &[char] = &['+', '=', ':', '.'];

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

    // Glitch corruption: persistent repair marks, then a rare per-tick body
    // cell swap (suppressed in calm mode).
    if pet.species == Species::Glitch {
        if let Some(glitch) = frame.glitch_corruption {
            apply_glitch_repair_marks(pet, stage, &mut lines, &mut spans, glitch);
            if !glitch.calm_mode {
                apply_glitch_transient_corruption(
                    &mut lines,
                    &mut spans,
                    frame.tick,
                    glitch.burst_level,
                    glitch.feed_reaction || frame.feed_reaction,
                );
            }
        } else {
            apply_glitch_corruption(&mut lines, &mut spans, frame.tick);
        }
    }

    // Wrap pet art in a 13x10 frame and overlay particles.
    let (framed_lines, framed_spans) =
        frame_with_particles(lines, spans, pet.species, stage, frame.tick);

    RenderedPet { lines: framed_lines, spans: framed_spans }
}

pub fn species_animation_profile(species: Species) -> AnimationProfile {
    match species {
        // Blink cadence in ticks (~250ms each). Lowered from the raw tokenpet mockup
        // numbers (which assumed a faster ~214ms tick) so a glance actually catches a
        // blink: nominal Fuzz ~4s, Crystal ~7.5s, jittered around that.
        Species::Fuzz => AnimationProfile { blink_average: 16, blink_jitter: 6 },
        Species::Blob => AnimationProfile { blink_average: 20, blink_jitter: 7 },
        Species::Ghost => AnimationProfile { blink_average: 26, blink_jitter: 9 },
        Species::Glitch => AnimationProfile { blink_average: 12, blink_jitter: 4 },
        Species::Crystal => AnimationProfile { blink_average: 30, blink_jitter: 11 },
        Species::Mech => AnimationProfile { blink_average: 12, blink_jitter: 4 },
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
        Species::Crystal => "- -",
        Species::Mech => "= =",
    }
}

/// Per-species glyph an open eye collapses to when it shuts.
fn closed_eye_char(species: Species) -> char {
    match species {
        Species::Fuzz | Species::Blob | Species::Crystal => '-',
        Species::Ghost => '\u{2014}',  // —
        Species::Glitch => '\u{2592}', // ▒
        Species::Mech => '=',
    }
}

/// Close the eyes for a blink while keeping the middle bridge glyph, so only the
/// eyes shut instead of the whole face morphing (e.g. `*o*` → `-o-`, `o o` → `- -`).
/// Glitch corrupts its whole eye band, so it keeps the solid `▒▒▒`.
fn close_eyes(eyes: &str, species: Species) -> String {
    if matches!(species, Species::Glitch) {
        return closed_blink_eyes(species).to_string();
    }
    let chars: Vec<char> = eyes.chars().collect();
    if chars.len() != 3 {
        return closed_blink_eyes(species).to_string();
    }
    let c = closed_eye_char(species);
    format!("{c}{}{c}", chars[1])
}

/// Idle gestures: a calm pet occasionally breaks idle with a small beat — a glance
/// left or right, or a wink — then settles. Deterministic per pet + tick so renders
/// stay reproducible. Returns `None` outside a gesture window. The vocabulary is
/// kept distinct from the mood faces so a gesture reads as behavior, not a mood.
fn idle_gesture_eyes(pet: &GeneratedPet, frame: AnimationFrame) -> Option<&'static str> {
    const GESTURE_PERIOD: u64 = 32; // ~8s between gestures at 250ms/tick
    const GESTURE_HOLD: u64 = 4; // ~1s holding the gesture
    const GESTURES: [&str; 4] = ["<.<", ">.>", "^.-", "-.^"];
    let seed = u64::from(pet.animation_phase.blink)
        .wrapping_mul(31)
        .wrapping_add(17);
    let t = frame.tick.wrapping_add(seed);
    // The gesture sits in the middle of its window, clear of the blink (window start).
    let pos = t % GESTURE_PERIOD;
    let start = GESTURE_PERIOD / 2;
    if pos < start || pos >= start + GESTURE_HOLD {
        return None;
    }
    let window = t / GESTURE_PERIOD;
    let h = (window ^ seed).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40;
    Some(GESTURES[(h % GESTURES.len() as u64) as usize])
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
            Mood::Ecstatic => mk("*.*", "\u{25bd}"), // *o* / ▽
            Mood::Hungry => mk("u.u", "o"),
            Mood::Sad => mk("T.T", "\u{2322}"), // T.T / ⌢
            Mood::Sleepy => mk("-.-", "-"),
            Mood::Wilted => mk(",_,", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Glitch => match mood {
            // Daemon lens face: alive, never corpse. ◉ is EAW-Neutral (width-1).
            Mood::Happy => mk(">\u{25c9}<", "\u{02c4}"), // >◉< / ˄
            Mood::Ecstatic => mk("\u{25c9}.\u{25c9}", "\u{25bd}"),
            Mood::Hungry => mk("o\u{25c9}o", "o"),
            Mood::Sad => mk("v\u{25c9}v", "\u{2322}"), // ⌢
            Mood::Sleepy => mk("-.-", "_"),
            Mood::Wilted => mk("x_x", "_"), // wilted may dim the lens
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Crystal => match mood {
            // Round, cute eyes — crystal identity comes from the diamond body and
            // amber eye color, not abstract facet glyphs. Star eyes when excited
            // read as a gem sparkle.
            Mood::Happy => mk("^.^", "w"),
            Mood::Ecstatic => mk("*.*", "\u{25bd}"),
            Mood::Hungry => mk("o.o", "o"),
            Mood::Sad => mk("T.T", "\u{2322}"), // ⌢
            Mood::Sleepy => mk("-.-", "_"),
            Mood::Wilted => mk(",_,", "_"),
            Mood::Content => unreachable!("Content handled in expression_for"),
        },
        Species::Mech => match mood {
            // Optic-sensor face: square/bracket eyes read mechanical.
            Mood::Happy => mk("^=^", "v"),
            Mood::Ecstatic => mk("*.*", "\u{25bd}"),
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
    let mut expr = match mood {
        Mood::Content => Expression {
            eyes: pet.traits.eyes.clone(),
            mouth: pet.traits.mouth.clone(),
        },
        other => mood_face(pet.species, other),
    };
    if blinking {
        // Only the eyes shut — keep the bridge and the mood mouth.
        expr.eyes = close_eyes(&expr.eyes, pet.species);
        return expr;
    }
    // A fresh feed perks the pet into a brief excited beat — its ecstatic face —
    // over whatever its resting mood is. Blink and sleep still win (handled above /
    // guarded here), so a sleeping pet stays asleep.
    if frame.feed_reaction && !frame.hold_eyes_closed {
        return mood_face(pet.species, Mood::Ecstatic);
    }
    let mut overridden = false;
    if frame.soft_eyes && matches!(mood, Mood::Content | Mood::Happy) {
        expr.eyes = "\u{02d8}.\u{02d8}".to_string(); // ˘.˘ relaxed, heavy-lidded
        overridden = true;
    }
    if matches!(mood, Mood::Happy | Mood::Content) {
        match frame.work_accent {
            WorkAccent::None => {}
            WorkAccent::Alert => {
                expr.eyes = "^o^".to_string();
                overridden = true;
            }
            WorkAccent::Focused => {
                expr.eyes = ">.<".to_string();
                overridden = true;
            }
            WorkAccent::Dreamy => {
                expr.eyes = "u.u".to_string();
                overridden = true;
            }
        }
    }
    // Idle gesture: a calm, non-droopy pet occasionally breaks idle with a small
    // beat — a glance or a wink — when no real work or sleepy signal owns the eyes.
    if !overridden && !matches!(mood, Mood::Sad | Mood::Sleepy | Mood::Wilted) {
        if let Some(gesture) = idle_gesture_eyes(pet, frame) {
            expr.eyes = gesture.to_string();
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
    // Sleepy/sad/wilted hold their droopy eyes; every other awake mood — including
    // the common, sticky ecstatic — still blinks so the pet never freezes wide-eyed.
    if matches!(mood, Mood::Sad | Mood::Sleepy | Mood::Wilted) {
        return false;
    }
    if frame.blink_suppression_ticks > 0 {
        return false;
    }
    // Hold the blink for a few ticks (a single 250ms frame is easy to miss), and
    // jitter the moment WITHIN each window so blinks land irregularly instead of on
    // a metronome — deterministic (per-window hash) so tests stay stable.
    const BLINK_HOLD_TICKS: u64 = 3;
    let jitter = u64::from(profile.blink_jitter.max(1));
    let window_len = (u64::from(profile.blink_average) + u64::from(frame.blink_slowdown))
        .max(BLINK_HOLD_TICKS + 2);
    let seed = u64::from(pet.animation_phase.blink);
    let t = frame.tick + seed;
    let window = t / window_len;
    let h = (window ^ seed).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40;
    let start = (window_len / 5 + h % (jitter + 1)).min(window_len - BLINK_HOLD_TICKS);
    let pos = t % window_len;
    pos >= start && pos < start + BLINK_HOLD_TICKS
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
        push_body_run(&mut text, &mut spans, line, body, &mut cursor);

        if let Some(close) = after_body.find('}') {
            let token = &after_body[..=close];
            match slot_value(token, pet, expression) {
                Some((value, role)) => {
                    push_segment(&mut text, &mut spans, line, value, role, &mut cursor);
                }
                None => push_body_run(&mut text, &mut spans, line, token, &mut cursor),
            }
            rest = &after_body[close + 1..];
        } else {
            push_body_run(&mut text, &mut spans, line, after_body, &mut cursor);
            rest = "";
        }
    }

    push_body_run(&mut text, &mut spans, line, rest, &mut cursor);
    RenderedTemplateLine { text, spans }
}

/// Push a run of body glyphs, splitting so each maximal run of `█` cells is
/// tagged `BodyGlow` (a lit tint) and everything else stays `Body`.
fn push_body_run(
    text: &mut String,
    spans: &mut Vec<StyledSegment>,
    line: usize,
    value: &str,
    cursor: &mut usize,
) {
    let role_for = |glow: bool| {
        if glow {
            PaletteRoleName::BodyGlow
        } else {
            PaletteRoleName::Body
        }
    };
    let mut run = String::new();
    let mut run_glow = false;
    for ch in value.chars() {
        let glow = ch == '\u{2588}';
        if !run.is_empty() && glow != run_glow {
            push_segment(text, spans, line, &run, role_for(run_glow), cursor);
            run.clear();
        }
        run.push(ch);
        run_glow = glow;
    }
    if !run.is_empty() {
        push_segment(text, spans, line, &run, role_for(run_glow), cursor);
    }
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

/// A single day-local repair-cell candidate on the 11x8 art grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlitchPatchCell {
    pub row: usize,
    pub col: usize,
}

// Picked against the metamorph silhouettes (art.rs): interior ░▒▓█ body fill
// only — never outline glyphs, face rows, or focal details (S3 ██ data-core,
// S5 ▛◆▜ chest-dock, S6 ██ heart / inner face-frame / ═ weld seam).
const GLITCH_S3_PATCH_CELLS: &[GlitchPatchCell] = &[
    // chip-and-caboose: cap fill (row 1) + belly shading left of the data-core.
    GlitchPatchCell { row: 1, col: 2 },
    GlitchPatchCell { row: 1, col: 3 },
    GlitchPatchCell { row: 1, col: 4 },
    GlitchPatchCell { row: 1, col: 5 },
    GlitchPatchCell { row: 4, col: 2 },
    GlitchPatchCell { row: 4, col: 3 },
];

const GLITCH_S4_PATCH_CELLS: &[GlitchPatchCell] = &[
    // leaning wafer: the two belly rows between the ▐…▌ walls.
    GlitchPatchCell { row: 4, col: 4 },
    GlitchPatchCell { row: 4, col: 5 },
    GlitchPatchCell { row: 4, col: 6 },
    GlitchPatchCell { row: 4, col: 7 },
    GlitchPatchCell { row: 5, col: 4 },
    GlitchPatchCell { row: 5, col: 5 },
    GlitchPatchCell { row: 5, col: 6 },
    GlitchPatchCell { row: 5, col: 7 },
];

const GLITCH_S5_PATCH_CELLS: &[GlitchPatchCell] = &[
    // coredock imp: shaded mass right of the ▛◆▜ dock + belly + haunch fill.
    GlitchPatchCell { row: 4, col: 5 },
    GlitchPatchCell { row: 4, col: 6 },
    GlitchPatchCell { row: 4, col: 7 },
    GlitchPatchCell { row: 5, col: 3 },
    GlitchPatchCell { row: 5, col: 4 },
    GlitchPatchCell { row: 5, col: 5 },
    GlitchPatchCell { row: 5, col: 6 },
    GlitchPatchCell { row: 6, col: 4 },
];

const GLITCH_S6_PATCH_CELLS: &[GlitchPatchCell] = &[
    // shed carapace: molten body fill, dodging the ██ heart (rows 4-5 cols
    // 8-9), the inner face-frame ▙▄▄▄▟ (row 4 cols 2-6), and the ═ seam.
    GlitchPatchCell { row: 4, col: 7 },
    GlitchPatchCell { row: 5, col: 2 },
    GlitchPatchCell { row: 5, col: 4 },
    GlitchPatchCell { row: 5, col: 5 },
    GlitchPatchCell { row: 5, col: 6 },
    GlitchPatchCell { row: 5, col: 7 },
    GlitchPatchCell { row: 6, col: 5 },
    GlitchPatchCell { row: 6, col: 6 },
];

/// Per-stage allowlist of body cells eligible to carry a persistent repair
/// mark. Kept below the face band and clear of the outline so a mark always
/// reads as a body scuff, never a face injury. S0-S2 are too small to spare a
/// safe cell, so they carry none.
fn glitch_patch_allowlist(stage: Stage) -> &'static [GlitchPatchCell] {
    match stage {
        Stage::S3 => GLITCH_S3_PATCH_CELLS,
        Stage::S4 => GLITCH_S4_PATCH_CELLS,
        Stage::S5 => GLITCH_S5_PATCH_CELLS,
        Stage::S6 => GLITCH_S6_PATCH_CELLS,
        Stage::S0 | Stage::S1 | Stage::S2 => &[],
    }
}

/// The role of the span covering (row, col), if any.
fn span_role_at(spans: &[StyledSegment], row: usize, col: usize) -> Option<PaletteRoleName> {
    spans
        .iter()
        .find(|span| span.line == row && col >= span.start && col < span.end)
        .map(|span| span.role)
}

/// The persistent-mark counterpart to the living-face rule above: a repair
/// mark must never sit on the eyes or mouth. Every glitch stage — elders
/// included — carries real {eyes}/{mouth} slots since the metamorph art, so
/// span-role protection covers the whole face; no per-stage coordinate
/// islands.
pub fn is_protected_glitch_face_cell(row: usize, col: usize, spans: &[StyledSegment]) -> bool {
    matches!(
        span_role_at(spans, row, col),
        Some(PaletteRoleName::Eye | PaletteRoleName::Mouth)
    )
}

/// Allowlisted cells that are actually safe to mark for this rendered frame:
/// non-blank, single-width, and clear of the protected face band.
pub fn safe_glitch_patch_candidates(
    stage: Stage,
    lines: &[String],
    spans: &[StyledSegment],
) -> Vec<GlitchPatchCell> {
    glitch_patch_allowlist(stage)
        .iter()
        .copied()
        .filter(|cell| {
            let Some(line) = lines.get(cell.row) else {
                return false;
            };
            let Some(ch) = line.chars().nth(cell.col) else {
                return false;
            };
            ch != ' '
                && unicode_width::UnicodeWidthChar::width(ch) == Some(1)
                && !is_protected_glitch_face_cell(cell.row, cell.col, spans)
        })
        .collect()
}

/// FNV-1a over the pet's seed string, giving each pet its own stable ordering
/// salt independent of the day/stage/cell inputs mixed in below.
fn hash_pet_seed(seed: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

/// SplitMix64 finalizer: spreads a seed's low bits across the full range so
/// nearby inputs (e.g. adjacent day_seeds) don't produce nearby orderings.
fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn stage_discriminant(stage: Stage) -> u64 {
    stage.index() as u64
}

/// The safe candidates for this pet/stage/day, sorted into a stable,
/// day-local order. Deterministic per (pet, stage, day_seed): the same day
/// always reorders the same way, and different days reshuffle the order —
/// but never the candidate set itself — so `selected_glitch_patch_cells`
/// below can reveal marks by simply taking a longer prefix as the tier rises,
/// without ever relocating a mark already shown.
pub fn ordered_glitch_patch_cells(
    pet: &GeneratedPet,
    stage: Stage,
    day_seed: u64,
    lines: &[String],
    spans: &[StyledSegment],
) -> Vec<GlitchPatchCell> {
    if pet.species != Species::Glitch {
        return Vec::new();
    }
    let seed_hash = hash_pet_seed(&pet.seed);
    let mut scored = safe_glitch_patch_candidates(stage, lines, spans)
        .into_iter()
        .map(|cell| {
            let score = mix64(
                seed_hash
                    ^ day_seed.rotate_left(17)
                    ^ stage_discriminant(stage).rotate_left(29)
                    ^ ((cell.row as u64) << 8)
                    ^ cell.col as u64,
            );
            (score, cell)
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, cell)| (*score, *cell));
    scored.into_iter().map(|(_, cell)| cell).collect()
}

/// The cells that should actually carry a persistent mark this tier: a prefix
/// of the day-local order, bounded by `GlitchPatchTier::max_marks`. A prefix
/// (not a re-roll) so a Quiet-day mark stays put when the tier rises to
/// Active/Heavy instead of jumping to a new cell.
pub fn selected_glitch_patch_cells(
    pet: &GeneratedPet,
    stage: Stage,
    day_seed: u64,
    tier: GlitchPatchTier,
    lines: &[String],
    spans: &[StyledSegment],
) -> Vec<GlitchPatchCell> {
    ordered_glitch_patch_cells(pet, stage, day_seed, lines, spans)
        .into_iter()
        .take(tier.max_marks())
        .collect()
}

/// Deterministic repair glyph for a patch cell: stable per (day, cell) so a
/// mark doesn't jitter glyphs while it's shown, but still varies across cells
/// and reshuffles across days like the cell ordering itself.
fn repair_glyph_for(cell: GlitchPatchCell, day_seed: u64) -> char {
    let index = mix64(day_seed ^ ((cell.row as u64) << 8) ^ cell.col as u64) as usize
        % GLITCH_REPAIR_GLYPHS.len();
    GLITCH_REPAIR_GLYPHS[index]
}

/// The first (most prominent) repair mark reads as a soft Accent highlight;
/// the rest blend in as Pattern so a Heavy day's three marks don't all
/// compete for attention.
fn repair_role_for(index: usize) -> PaletteRoleName {
    if index == 0 {
        PaletteRoleName::Accent
    } else {
        PaletteRoleName::Pattern
    }
}

/// Persistent glitch repair marks: this day's selected patch cells
/// (Task 2's `selected_glitch_patch_cells`) get a stable repair glyph and are
/// retagged Pattern/Accent — never Corruption — so a healed cell reads as a
/// quiet scar instead of active glitching.
fn apply_glitch_repair_marks(
    pet: &GeneratedPet,
    stage: Stage,
    lines: &mut [String],
    spans: &mut Vec<StyledSegment>,
    frame: GlitchCorruptionFrame,
) {
    let cells =
        selected_glitch_patch_cells(pet, stage, frame.day_seed, frame.patch_tier, lines, spans);
    for (index, cell) in cells.into_iter().enumerate() {
        let glyph = repair_glyph_for(cell, frame.day_seed);
        replace_char_in_line(&mut lines[cell.row], cell.col, glyph);
        retag_cell_as_role(spans, cell.row, cell.col, repair_role_for(index));
    }
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

/// Re-tag the single cell (row, col) with `role`. If an existing span covers
/// the cell, it is split around the cell so the surrounding cells keep their
/// original role and the cell itself becomes a width-1 `role` span. If no
/// span covers the cell, a standalone `role` span is added. Spans are
/// re-sorted by (line, start, end) afterward so repeated retagging (e.g. one
/// call per repair mark) always leaves the vector in TUI-consumer order.
fn retag_cell_as_role(
    spans: &mut Vec<StyledSegment>,
    row: usize,
    col: usize,
    role: PaletteRoleName,
) {
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
    // The retagged cell itself. If no span covered the cell (a body-gap cell),
    // this standalone span overrides that single cell for the consumer.
    split.push(StyledSegment {
        line: row,
        start: col,
        end: col + 1,
        role,
    });
    // Drop any now-empty left fragments produced by the shrink.
    spans.retain(|s| s.start < s.end);
    spans.extend(split);
    spans.sort_by_key(|s| (s.line, s.start, s.end));
}

/// Corruption-specific alias for [`retag_cell_as_role`] — corruption is the
/// original, still most-called retag site, so it keeps a named entry point.
fn retag_cell_as_corruption(spans: &mut Vec<StyledSegment>, row: usize, col: usize) {
    retag_cell_as_role(spans, row, col, PaletteRoleName::Corruption);
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

/// Transient corruption footprint for this moment: normally just the calm
/// periodic gate (`corruption_cells_for_tick`), but a feed reaction or a
/// Strong session burst forces a short off-gate glitch regardless of tick,
/// with a footprint sized by `burst_level`.
fn corruption_cells_for_moment(
    art_lines: &[String],
    spans: &[StyledSegment],
    tick: u64,
    burst_level: GlitchBurstLevel,
    feed_reaction: bool,
) -> Vec<(usize, usize)> {
    let force = feed_reaction || matches!(burst_level, GlitchBurstLevel::Strong);
    if !force {
        return corruption_cells_for_tick(art_lines, tick);
    }

    // Pre-filter protected/eye cells out of the forced-burst pool (rather than
    // selecting a fixed-size slice and skipping protected hits afterward) so a
    // feed reaction or Strong burst can't land its whole bounded footprint on
    // the face band and silently render nothing.
    let mut candidates = Vec::new();
    for (row, line) in art_lines.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch != ' '
                && !is_protected_glitch_face_cell(row, col, spans)
                && !is_eye_center(spans, row, col)
            {
                candidates.push((row, col));
            }
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    let count = match burst_level {
        GlitchBurstLevel::None => 1,
        GlitchBurstLevel::Small => 2,
        GlitchBurstLevel::Strong => CORRUPTION_MAX_CELLS,
    }
    .min(candidates.len());
    let start = (tick.wrapping_mul(5) as usize) % candidates.len();
    candidates.rotate_left(start);
    candidates.truncate(count);
    candidates.sort_unstable();
    candidates
}

/// Bounded transient glitch burst: like `apply_glitch_corruption`, but driven
/// by `corruption_cells_for_moment` so a feed reaction or Strong session
/// burst can force a short off-gate glitch. Respects both the living-face
/// rule (never the eye-center) and the persistent-repair face band
/// (`is_protected_glitch_face_cell`), so a burst never recolors a
/// day-repaired or protected face cell.
fn apply_glitch_transient_corruption(
    lines: &mut [String],
    spans: &mut Vec<StyledSegment>,
    tick: u64,
    burst_level: GlitchBurstLevel,
    feed_reaction: bool,
) {
    let cells = corruption_cells_for_moment(lines, spans, tick, burst_level, feed_reaction);
    if cells.is_empty() {
        return;
    }
    for (i, (row, col)) in cells.into_iter().enumerate() {
        if is_protected_glitch_face_cell(row, col, spans) || is_eye_center(spans, row, col) {
            continue;
        }
        let noise =
            GLITCH_NOISE[((tick as usize).wrapping_mul(3).wrapping_add(i)) % GLITCH_NOISE.len()];
        replace_char_in_line(&mut lines[row], col, noise);
        retag_cell_as_corruption(spans, row, col);
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
    // Sort for TUI consumers and collapse exact duplicate spans (e.g. a
    // contested cell where a species particle glyph-overrides the S6 gutter
    // sparkle at the same coordinates), so the result stays a well-ordered,
    // non-overlapping span list.
    framed_spans.sort_by_key(|span| (span.line, span.start, span.end));
    framed_spans.dedup();
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

/// Non-space columns of the feet row for renderer-level footprint tests.
#[cfg(test)]
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
                particles.push(Particle { row: 9, col: 6, glyph: '~' });
            }
        }
        Species::Blob => {
            if tick % 19 < 3 {
                particles.push(Particle { row: 9, col: 4, glyph: '.' });
            }
            if tick % 23 < 4 {
                particles.push(Particle { row: 9, col: 6, glyph: '\u{b0}' });
            }
            if tick % 17 < 2 {
                particles.push(Particle { row: 9, col: 8, glyph: '.' });
            }
        }
        Species::Ghost => {
            if tick % 13 < 3 {
                particles.push(Particle { row: 0, col: 5, glyph: '~' });
            }
            if tick % 19 < 4 {
                particles.push(Particle { row: 9, col: 7, glyph: '\u{b7}' });
            }
            if tick % 21 < 2 {
                particles.push(Particle { row: 9, col: 3, glyph: '\'' });
            }
            if tick % 17 < 3 {
                particles.push(Particle { row: 0, col: 9, glyph: '.' });
            }
        }
        Species::Glitch => {
            // Scan line: only at tick % 41 == 0, draw a single cell.
            if tick.is_multiple_of(41) {
                let row = ((tick / 41) as usize) % FRAME_HEIGHT;
                let col = ((tick / 41) as usize) % FRAME_WIDTH;
                particles.push(Particle { row, col, glyph: ':' });
            }
            if tick % 11 < 2 {
                particles.push(Particle { row: 0, col: 2, glyph: '\u{b7}' });
            }
            if tick % 13 < 2 {
                particles.push(Particle { row: 9, col: 4, glyph: '.' });
            }
            if tick % 17 < 2 {
                particles.push(Particle { row: 0, col: 10, glyph: ':' });
            }
        }
        Species::Crystal => {
            if tick % 23 < 3 {
                particles.push(Particle { row: 0, col: 1, glyph: '\u{2727}' });
            }
            if tick % 19 < 3 {
                particles.push(Particle { row: 0, col: 11, glyph: '\u{2726}' });
            }
            if tick % 21 < 3 {
                particles.push(Particle { row: 9, col: 1, glyph: '\u{2727}' });
            }
            if tick % 17 < 2 {
                particles.push(Particle { row: 9, col: 11, glyph: '\u{b7}' });
            }
            if tick % 27 < 3 {
                particles.push(Particle { row: 5, col: 0, glyph: '\u{2727}' });
            }
        }
        Species::Mech => {
            // LED at row 0 col 6: '●' when tick % 4 < 2 else '○'.
            let led_glyph = if tick % 4 < 2 { '\u{25cf}' } else { '\u{25cb}' };
            particles.push(Particle { row: 0, col: 6, glyph: led_glyph });
            if tick % 9 < 3 {
                particles.push(Particle { row: 0, col: 4, glyph: '~' });
            }
            if tick % 11 < 3 {
                particles.push(Particle { row: 0, col: 8, glyph: '\u{b0}' });
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
        let normal = AnimationFrame { tick: 1, ..AnimationFrame::default() };
        let soft = AnimationFrame { soft_eyes: true, ..normal };
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
    fn feed_reaction_perks_the_face_into_an_excited_beat() {
        let pet = generate_pet("feed-react-seed");
        // Suppress blink so the open-eyed reaction is observable.
        let base = AnimationFrame {
            tick: 3,
            blink_suppression_ticks: 1,
            ..AnimationFrame::default()
        };
        let fed = AnimationFrame { feed_reaction: true, ..base };
        let calm = render_pet(&pet, Stage::S3, Mood::Content, base)
            .lines
            .join("\n");
        let reacting = render_pet(&pet, Stage::S3, Mood::Content, fed)
            .lines
            .join("\n");
        assert_ne!(calm, reacting, "a feed reaction should change the face");
        // The reaction briefly wears the species' ecstatic eyes.
        let ecstatic_eyes = mood_face(pet.species, Mood::Ecstatic).eyes;
        assert!(
            reacting.contains(&ecstatic_eyes),
            "feed reaction should show excited eyes ({ecstatic_eyes}), got:\n{reacting}"
        );
        // An asleep pet does not perk up.
        let asleep = AnimationFrame {
            feed_reaction: true,
            hold_eyes_closed: true,
            ..base
        };
        let sleeping = render_pet(&pet, Stage::S3, Mood::Content, asleep)
            .lines
            .join("\n");
        assert!(
            !sleeping.contains(&ecstatic_eyes),
            "an asleep pet should not perk up from feeding, got:\n{sleeping}"
        );
    }

    #[test]
    fn idle_gaze_rotates_through_varied_gestures_including_a_wink() {
        let pet = generate_pet("idle-variety-seed");
        let mut seen = std::collections::HashSet::new();
        for tick in 0..4000u64 {
            let frame = AnimationFrame { tick, ..AnimationFrame::default() };
            if let Some(g) = idle_gesture_eyes(&pet, frame) {
                seen.insert(g);
            }
        }
        assert!(
            seen.len() >= 3,
            "idle gaze should rotate through varied gestures, saw: {seen:?}"
        );
        assert!(
            seen.contains("<.<") || seen.contains(">.>"),
            "keeps the look-around glances, saw: {seen:?}"
        );
        assert!(
            seen.iter().any(|g| g.contains('^') && g.contains('-')),
            "includes a wink, saw: {seen:?}"
        );
    }

    #[test]
    fn work_accent_sharpens_positive_moods_and_ignored_for_negative() {
        let pet = generate_pet("accent-seed");
        // Suppress blink so the open-eyed work-accent change is observable (otherwise
        // a blink frame closes the eyes in both renders and masks the accent).
        let base = AnimationFrame {
            tick: 2,
            blink_suppression_ticks: 1,
            ..AnimationFrame::default()
        };
        for accent in [WorkAccent::Alert, WorkAccent::Focused, WorkAccent::Dreamy] {
            let accented = AnimationFrame { work_accent: accent, ..base };
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
            hold_eyes_closed: true,
            ..AnimationFrame::default()
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
            AnimationFrame { tick: 1, ..AnimationFrame::default() },
        );
        assert!(
            open.lines.join("\n").contains(&pet.traits.eyes),
            "non-blinking awake frame keeps the trait eyes"
        );
    }

    #[test]
    fn ecstatic_renders_star_eyes_and_still_blinks() {
        let pet = generate_pet("ecstatic-seed");
        // Suppress blink so the open-eyed star face is observable regardless of where
        // a blink lands at this tick.
        let open = AnimationFrame {
            tick: 1,
            blink_suppression_ticks: 1,
            ..AnimationFrame::default()
        };
        let art = render_pet(&pet, Stage::S4, Mood::Ecstatic, open)
            .lines
            .join("\n");
        assert!(art.contains("*.*"), "ecstatic uses star eyes, got:\n{art}");
        // An excited pet still blinks (ecstatic is no longer a blink-blocked mood).
        let profile = species_animation_profile(pet.species);
        let blinks = (0..200).any(|t| {
            should_blink(
                &pet,
                Mood::Ecstatic,
                AnimationFrame { tick: t, ..AnimationFrame::default() },
                profile,
            )
        });
        assert!(blinks, "ecstatic mood should blink within a cycle");
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
            art.contains("*.*"),
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
                            blink_slowdown: slowdown,
                            ..AnimationFrame::default()
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
        let p = AnimationProfile { blink_average: 30, blink_jitter: 10 };
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
        let frame = AnimationFrame { tick: 13, ..AnimationFrame::default() };
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
            let frame = AnimationFrame { tick, ..AnimationFrame::default() };
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
            let frame = AnimationFrame { tick: 13, ..AnimationFrame::default() };
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
        let frame = AnimationFrame { tick: 1, ..AnimationFrame::default() };
        let rendered = render_pet(&pet, Stage::S4, Mood::Content, frame);
        assert!(
            !rendered
                .spans
                .iter()
                .any(|s| s.role == PaletteRoleName::Corruption),
            "off-gate tick must stay calm (no corruption)"
        );
    }

    #[test]
    fn glitch_patch_tier_quantizes_today_ratio_without_live_activity() {
        assert_eq!(
            GlitchPatchTier::from_today_ratio(-1.0),
            GlitchPatchTier::Quiet
        );
        assert_eq!(
            GlitchPatchTier::from_today_ratio(f32::NAN),
            GlitchPatchTier::Quiet
        );
        assert_eq!(
            GlitchPatchTier::from_today_ratio(0.0),
            GlitchPatchTier::Quiet
        );
        assert_eq!(
            GlitchPatchTier::from_today_ratio(0.74),
            GlitchPatchTier::Quiet
        );
        assert_eq!(
            GlitchPatchTier::from_today_ratio(0.75),
            GlitchPatchTier::Active
        );
        assert_eq!(
            GlitchPatchTier::from_today_ratio(1.49),
            GlitchPatchTier::Active
        );
        assert_eq!(
            GlitchPatchTier::from_today_ratio(1.5),
            GlitchPatchTier::Heavy
        );
        assert_eq!(GlitchPatchTier::Pristine.max_marks(), 0);
        assert_eq!(GlitchPatchTier::Quiet.max_marks(), 1);
        assert_eq!(GlitchPatchTier::Active.max_marks(), 2);
        assert_eq!(GlitchPatchTier::Heavy.max_marks(), 3);
    }

    #[test]
    fn glitch_burst_level_quantizes_live_burst_for_eq_animation_frame() {
        assert_eq!(
            GlitchBurstLevel::from_burst_level(-1.0),
            GlitchBurstLevel::None
        );
        assert_eq!(
            GlitchBurstLevel::from_burst_level(f32::NAN),
            GlitchBurstLevel::None
        );
        assert_eq!(
            GlitchBurstLevel::from_burst_level(0.2),
            GlitchBurstLevel::None
        );
        assert_eq!(
            GlitchBurstLevel::from_burst_level(0.21),
            GlitchBurstLevel::Small
        );
        assert_eq!(
            GlitchBurstLevel::from_burst_level(0.69),
            GlitchBurstLevel::Small
        );
        assert_eq!(
            GlitchBurstLevel::from_burst_level(0.7),
            GlitchBurstLevel::Strong
        );
    }

    #[test]
    fn glitch_corruption_frame_keeps_patch_inputs_separate_from_live_inputs() {
        let frame = glitch_corruption_frame_for_inputs(42, 1.6, 0.8, true, false);

        assert_eq!(frame.day_seed, 42);
        assert_eq!(frame.patch_tier, GlitchPatchTier::Heavy);
        assert_eq!(frame.burst_level, GlitchBurstLevel::Strong);
        assert!(frame.calm_mode);
        assert!(!frame.feed_reaction);
    }

    fn raw_glitch_art_for_test(
        seed: &str,
        stage: Stage,
    ) -> (GeneratedPet, Vec<String>, Vec<StyledSegment>) {
        let pet = generate_pet(seed).with_species(Species::Glitch);
        let expression = expression_for(&pet, Mood::Content, false, AnimationFrame::default());
        let raw = stage_template_lines(Species::Glitch, stage, u64::from(pet.traits.seed_hue));
        let rendered = raw
            .iter()
            .enumerate()
            .map(|(line_index, line)| render_template_line(line, line_index, &pet, &expression))
            .collect::<Vec<_>>();
        let lines = rendered
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>();
        let spans = rendered
            .into_iter()
            .flat_map(|line| line.spans)
            .collect::<Vec<_>>();
        (pet, lines, spans)
    }

    #[test]
    fn glitch_safe_patch_candidates_exclude_face_and_outline() {
        let (_pet, lines, spans) = raw_glitch_art_for_test("glitch-safe-candidates", Stage::S4);
        let candidates = safe_glitch_patch_candidates(Stage::S4, &lines, &spans);

        // The S4 allowlist (GLITCH_S4_PATCH_CELLS) only names the belly rows
        // 4-5, so face/outline rows are excluded because they were never
        // offered as candidates, not because `is_protected_glitch_face_cell`
        // ran. That's still a real safety property worth asserting (the
        // allowlist itself never strays into the face/outline band); the
        // protection filter's own behavior is exercised directly below in
        // `glitch_safe_patch_candidates_excludes_allowlisted_cell_under_face_span`.
        assert!(candidates.contains(&GlitchPatchCell { row: 4, col: 5 }));
        assert!(
            candidates.len() >= 6,
            "S4 needs enough safe cells for day-local variety, got {}",
            candidates.len()
        );
        assert!(
            !candidates.contains(&GlitchPatchCell { row: 2, col: 5 }),
            "eye row is not in the S4 allowlist"
        );
        assert!(
            !candidates.contains(&GlitchPatchCell { row: 3, col: 5 }),
            "mouth row is not in the S4 allowlist"
        );
        assert!(
            !candidates.contains(&GlitchPatchCell { row: 0, col: 3 }),
            "top outline row is not in the S4 allowlist"
        );
    }

    #[test]
    fn glitch_safe_patch_candidates_excludes_allowlisted_cell_under_face_span() {
        let (_pet, lines, base_spans) =
            raw_glitch_art_for_test("glitch-safe-candidates", Stage::S4);

        // Baseline: row 4, col 5 is a real S4 allowlisted body cell and is
        // offered as a candidate absent any overlapping face span.
        let baseline = safe_glitch_patch_candidates(Stage::S4, &lines, &base_spans);
        assert!(baseline.contains(&GlitchPatchCell { row: 4, col: 5 }));

        // A synthetic Eye span covering that same allowlisted cell must still
        // exclude it from the candidates returned by the public entry point --
        // proving the protection filter itself runs inside
        // `safe_glitch_patch_candidates`, not just that the allowlist avoids
        // the face band by construction. `span_role_at` returns the first
        // matching span, so the synthetic span is prepended ahead of the
        // real body-texture span already covering this cell.
        let mut spans_with_face_overlap = vec![StyledSegment {
            line: 4,
            start: 5,
            end: 6,
            role: PaletteRoleName::Eye,
        }];
        spans_with_face_overlap.extend(base_spans);

        let candidates = safe_glitch_patch_candidates(Stage::S4, &lines, &spans_with_face_overlap);
        assert!(
            !candidates.contains(&GlitchPatchCell { row: 4, col: 5 }),
            "allowlisted cell overlapped by a face span must still be protected"
        );
    }

    #[test]
    fn glitch_face_protection_is_span_driven_at_every_stage() {
        // Elders carry real {eyes}/{mouth} slots since the metamorph art, so
        // face protection is purely span-role-driven — no coordinate islands.
        let spans = vec![
            StyledSegment {
                line: 2,
                start: 3,
                end: 6,
                role: PaletteRoleName::Eye,
            },
            StyledSegment {
                line: 3,
                start: 4,
                end: 5,
                role: PaletteRoleName::Mouth,
            },
        ];

        assert!(is_protected_glitch_face_cell(2, 4, &spans), "eye span cell");
        assert!(
            is_protected_glitch_face_cell(3, 4, &spans),
            "mouth span cell"
        );
        assert!(
            !is_protected_glitch_face_cell(1, 5, &spans),
            "body cell off the face spans is unprotected (the old S5/S6 island is gone)"
        );
        assert!(!is_protected_glitch_face_cell(5, 5, &spans), "belly cell");
    }

    #[test]
    fn ordered_glitch_patch_cells_are_stable_and_day_local() {
        let (pet, lines, spans) = raw_glitch_art_for_test("glitch-ordered-patches", Stage::S4);

        let first = ordered_glitch_patch_cells(&pet, Stage::S4, 123, &lines, &spans);
        let second = ordered_glitch_patch_cells(&pet, Stage::S4, 123, &lines, &spans);
        let next_day = ordered_glitch_patch_cells(&pet, Stage::S4, 124, &lines, &spans);

        assert_eq!(first, second);
        assert_ne!(first, next_day);
        assert!(
            first.len() >= 6,
            "S4 needs enough safe cells that day reshuffles show different marks"
        );
    }

    #[test]
    fn tier_selection_reveals_prefix_without_relocating_marks() {
        let (pet, lines, spans) = raw_glitch_art_for_test("glitch-tier-prefix", Stage::S4);

        let quiet = selected_glitch_patch_cells(
            &pet,
            Stage::S4,
            555,
            GlitchPatchTier::Quiet,
            &lines,
            &spans,
        );
        let active = selected_glitch_patch_cells(
            &pet,
            Stage::S4,
            555,
            GlitchPatchTier::Active,
            &lines,
            &spans,
        );
        let heavy = selected_glitch_patch_cells(
            &pet,
            Stage::S4,
            555,
            GlitchPatchTier::Heavy,
            &lines,
            &spans,
        );

        assert_eq!(&active[..quiet.len()], quiet.as_slice());
        assert_eq!(&heavy[..active.len()], active.as_slice());
    }

    #[test]
    fn glitch_repair_marks_use_soft_roles_not_corruption() {
        let pet = generate_pet("glitch-repair-soft").with_species(Species::Glitch);
        let rendered = render_pet(
            &pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick: 1,
                glitch_corruption: Some(GlitchCorruptionFrame {
                    day_seed: 42,
                    patch_tier: GlitchPatchTier::Heavy,
                    burst_level: GlitchBurstLevel::None,
                    calm_mode: false,
                    feed_reaction: false,
                }),
                ..AnimationFrame::default()
            },
        );

        let repair_spans = rendered
            .spans
            .iter()
            .filter(|span| {
                matches!(
                    span.role,
                    PaletteRoleName::Pattern | PaletteRoleName::Accent
                )
            })
            .filter(|span| span.end == span.start + 1)
            .collect::<Vec<_>>();
        let text = rendered.lines.join("\n");
        assert!(
            repair_spans.len() >= 3,
            "heavy Glitch day should emit at least three one-cell repair spans"
        );
        assert!(
            text.contains('+') || text.contains('=') || text.contains(':') || text.contains('.'),
            "heavy Glitch day should show at least one repair glyph"
        );
        assert!(
            rendered
                .spans
                .iter()
                .all(|span| span.role != PaletteRoleName::Corruption),
            "persistent repair marks must not use the loud Corruption role"
        );
    }

    #[test]
    fn glitch_repair_spans_are_sorted_and_non_overlapping() {
        let pet = generate_pet("glitch-repair-spans").with_species(Species::Glitch);
        let rendered = render_pet(
            &pet,
            Stage::S6,
            Mood::Content,
            AnimationFrame {
                tick: 1,
                glitch_corruption: Some(GlitchCorruptionFrame {
                    day_seed: 777,
                    patch_tier: GlitchPatchTier::Heavy,
                    burst_level: GlitchBurstLevel::None,
                    calm_mode: true,
                    feed_reaction: false,
                }),
                ..AnimationFrame::default()
            },
        );

        let mut spans = rendered.spans.clone();
        let sorted = {
            let mut clone = spans.clone();
            clone.sort_by_key(|span| (span.line, span.start, span.end));
            clone
        };
        assert_eq!(
            spans, sorted,
            "rendered spans should be sorted for TUI consumers"
        );

        spans.sort_by_key(|span| (span.line, span.start));
        for pair in spans.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            if left.line == right.line {
                assert!(
                    left.end <= right.start,
                    "overlapping spans on line {}: {:?} then {:?}",
                    left.line,
                    left,
                    right
                );
            }
        }
    }

    #[test]
    fn glitch_repair_marks_never_touch_face_spans() {
        // Span-based successor to the old elder-island test: on a heavy day,
        // no 1-cell repair span may overlap the rendered Eye/Mouth spans.
        let pet = generate_pet("glitch-elder-repair").with_species(Species::Glitch);
        let rendered = render_pet(
            &pet,
            Stage::S6,
            Mood::Content,
            AnimationFrame {
                tick: 1,
                glitch_corruption: Some(GlitchCorruptionFrame {
                    day_seed: 42,
                    patch_tier: GlitchPatchTier::Heavy,
                    burst_level: GlitchBurstLevel::None,
                    calm_mode: true,
                    feed_reaction: false,
                }),
                ..AnimationFrame::default()
            },
        );

        let face_spans: Vec<_> = rendered
            .spans
            .iter()
            .filter(|span| matches!(span.role, PaletteRoleName::Eye | PaletteRoleName::Mouth))
            .cloned()
            .collect();
        assert!(
            !face_spans.is_empty(),
            "S6 elder must render live Eye/Mouth spans"
        );

        for span in rendered
            .spans
            .iter()
            .filter(|span| {
                matches!(
                    span.role,
                    PaletteRoleName::Pattern | PaletteRoleName::Accent
                )
            })
            .filter(|span| span.end == span.start + 1)
        {
            let overlaps_face = face_spans
                .iter()
                .any(|f| f.line == span.line && span.start >= f.start && span.start < f.end);
            assert!(
                !overlaps_face,
                "repair span overlapped a face span: {span:?}"
            );
        }
    }

    #[test]
    fn glitch_feed_reaction_can_trigger_transient_corruption_off_gate() {
        let pet = generate_pet("glitch-feed-burst").with_species(Species::Glitch);
        let rendered = render_pet(
            &pet,
            Stage::S4,
            Mood::Happy,
            AnimationFrame {
                tick: 2,
                feed_reaction: true,
                glitch_corruption: Some(GlitchCorruptionFrame {
                    day_seed: 42,
                    patch_tier: GlitchPatchTier::Quiet,
                    burst_level: GlitchBurstLevel::Small,
                    calm_mode: false,
                    feed_reaction: true,
                }),
                ..AnimationFrame::default()
            },
        );

        assert!(
            rendered
                .spans
                .iter()
                .any(|span| span.role == PaletteRoleName::Corruption),
            "feed reaction should allow a short transient glitch off the old gate"
        );
    }

    #[test]
    fn glitch_calm_mode_suppresses_transient_corruption_but_keeps_repairs() {
        let pet = generate_pet("glitch-calm-burst").with_species(Species::Glitch);
        let rendered = render_pet(
            &pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick: 13,
                feed_reaction: true,
                glitch_corruption: Some(GlitchCorruptionFrame {
                    day_seed: 42,
                    patch_tier: GlitchPatchTier::Heavy,
                    burst_level: GlitchBurstLevel::Strong,
                    calm_mode: true,
                    feed_reaction: true,
                }),
                ..AnimationFrame::default()
            },
        );

        assert!(
            rendered
                .spans
                .iter()
                .all(|span| span.role != PaletteRoleName::Corruption),
            "calm mode should suppress transient corruption"
        );
        assert!(
            rendered.spans.iter().any(|span| matches!(
                span.role,
                PaletteRoleName::Pattern | PaletteRoleName::Accent
            )),
            "calm mode should keep day-local repair marks"
        );
    }
}
