use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::game::evolution::Stage;
use crate::game::habitat::{catalog_prop, HabitatPetLayer, HabitatPropId, HabitatPropZone};
use crate::pet::animator::{
    compute_facing, compute_shimmer_role, compute_sleep_wander_x, compute_token_pop,
    compute_twinkle, compute_wake_wander_x, compute_wander_position_x, lazy_wander_instant,
    low_energy_lightness_multiplier, TokenPop,
};
use crate::pet::generation::Species;
use crate::pet::render::PaletteRoleName;
use crate::tui::component::{habitat_props_for, PetScene, PetSceneLayout};
use crate::tui::day::{DayPhase, Season};
use crate::tui::life::{
    build_prop_reactions, PetLifeProfile, PropReaction, PropReactionKind, SourceAccent, WorkWeather,
};
use crate::tui::panels::LegacyPanel;
use crate::tui::render_context::RenderContext;
use crate::tui::room::rects_contain;
use crate::tui::style::{semantic_styles, ColorCapability, SemanticStyles};
use crate::tui::view_model::WatchViewModel;

pub struct PetPanel;

/// The rendered pet art is 13 columns wide (11 chars + 1-cell particle border each side)
/// and 10 rows tall (8 art rows + 1-cell particle border top/bottom).
const PET_W: u16 = 13;
const PET_H: u16 = 10;
/// Day-accumulation motes may use at most this share of the ambient glyph
/// allocation — the room never crowds the sky (spec: Day accumulation).
const MOTE_BUDGET_SHARE: f64 = 0.5;
/// Floor-mote glyphs: soft specks, deliberately sub-countable.
const MOTE_GLYPHS: &[char] = &['·', '.', ','];

/// Computes the 13×10 sub-rect where the pet art sits inside the panel area,
/// accounting for vertical centering, breathing offset, and wander offset.
///
/// The horizontal wander position is computed directly from `area.width` so
/// the pet drifts across the full habitat regardless of where `vm.wander_offset_x`
/// is set. `vm.wander_offset_x` is ignored at render time; it's only for test
/// inspection.
pub(crate) fn pet_inner_rect_in_panel(area: Rect, vm: &WatchViewModel) -> Rect {
    let cx = area.x + area.width.saturating_sub(PET_W) / 2;
    let cy = area.y + area.height.saturating_sub(PET_H) / 2;
    // When `area` is smaller than the pet, the upper clamp bound would fall
    // below `area.x` / `area.y`, which makes `i32::clamp` panic. `.max(...)`
    // ensures min ≤ max so the rect collapses to `area`'s origin instead.
    let max_x = (area.x + area.width).saturating_sub(PET_W).max(area.x);
    let max_y = (area.y + area.height).saturating_sub(PET_H).max(area.y);
    // Use a dummy "no species" clock time if we can't infer it here; callers
    // that need an exact position for tests can override via vm.wander_offset_x.
    // For rendering we always compute from area width and vm's species/clock.
    let wander_x = vm.wander_offset_x as i32;
    let x = (cx as i32 + wander_x).clamp(area.x as i32, max_x as i32) as u16;
    let y = (cy as i32 + vm.breath_offset_y as i32).clamp(area.y as i32, max_y as i32) as u16;
    Rect::new(x, y, PET_W, PET_H)
}

/// An ambient environment glyph placed in the panel backdrop behind the pet art.
/// Produced by [`ambient_glyphs_for`] and rendered in pass 1 of the pet panel,
/// behind the pet art.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientGlyph {
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub color: Color,
}

/// Per-species sky-glyph palette.
fn sky_palette_for(species: Species) -> &'static [char] {
    match species {
        Species::Fuzz => &['·', ',', '\'', '*'],
        Species::Blob => &['°', 'o', '.', '·'],
        Species::Ghost => &['~', '\'', ',', '*'],
        Species::Glitch => &[':', ';', '#', '░', '▒', '▪'],
        Species::Crystal => &['✦', '✧', '◇', '◆', '·'],
        Species::Mech => &['~', '°', '·', '●'],
    }
}

/// Per-biome floor-glyph palette — the ground texture under the pet, keyed to
/// the earned biome rather than species so the floor reads as a place.
fn biome_floor_palette(tag: crate::tui::room::RoomBiomeTag) -> &'static [char] {
    use crate::tui::room::RoomBiomeTag;
    match tag {
        RoomBiomeTag::Starter => &['·', '.', ' ', ' '],
        RoomBiomeTag::Botanical => &[',', '·', '"', '.', ' '],
        RoomBiomeTag::Technical => &['─', '┄', '·', '.', ' '],
        RoomBiomeTag::Celestial => &['·', '˚', '.', ' ', ' '],
        RoomBiomeTag::Artifact => &['◦', '·', '°', '.', ' '],
        RoomBiomeTag::Cozy => &['·', '~', ',', '.', ' '],
    }
}

/// A whisper-quiet per-biome background wash: the theme bg nudged a few points
/// toward the biome's hue, so the habitat reads as a place even in a screenshot
/// without overpowering the pet/panels.
fn biome_wash_color(tag: crate::tui::room::RoomBiomeTag) -> ratatui::style::Color {
    use crate::tui::room::RoomBiomeTag;
    use ratatui::style::Color;
    let Color::Rgb(r, g, b) = crate::tui::style::tokenpet_palette().bg.rgb else {
        return crate::tui::style::tokenpet_palette().bg.rgb;
    };
    // Small signed nudges per channel (kept within +-16 so it stays subtle).
    let (dr, dg, db): (i16, i16, i16) = match tag {
        RoomBiomeTag::Starter => (0, 0, 0),
        RoomBiomeTag::Botanical => (-2, 8, -2),
        RoomBiomeTag::Technical => (-2, 2, 12),
        RoomBiomeTag::Celestial => (2, 2, 10),
        RoomBiomeTag::Artifact => (10, 4, -4),
        RoomBiomeTag::Cozy => (10, 2, -2),
    };
    let clamp = |v: i16| v.clamp(0, 255) as u8;
    Color::Rgb(
        clamp(r as i16 + dr),
        clamp(g as i16 + dg),
        clamp(b as i16 + db),
    )
}

/// Sky-glyph count by stage tier.
fn stage_base_count(stage: Stage) -> usize {
    match stage {
        Stage::S0 | Stage::S1 => 4,
        Stage::S2 | Stage::S3 => 6,
        Stage::S4 | Stage::S5 => 8,
        Stage::S6 => 10,
    }
}

/// Seed discriminant for species, avoiding `as u64` on an enum without repr.
fn species_seed(species: Species) -> u64 {
    match species {
        Species::Fuzz => 0,
        Species::Blob => 1,
        Species::Ghost => 2,
        Species::Glitch => 3,
        Species::Crystal => 4,
        Species::Mech => 5,
    }
}

/// Seed discriminant for stage.
fn stage_seed(stage: Stage) -> u64 {
    match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 => 2,
        Stage::S3 => 3,
        Stage::S4 => 4,
        Stage::S5 => 5,
        Stage::S6 => 6,
    }
}

/// Capped count of extra activity glyphs for the current life profile.
fn activity_glyph_budget(profile: &PetLifeProfile, compact: bool) -> usize {
    if profile.calm_mode {
        return 0;
    }
    let max = if compact { 3.0 } else { 10.0 };
    ((profile.activity_level.clamp(0.0, 2.0) / 2.0) * max).round() as usize
}

const RESONANCE_REACTION_INTENSITY: f32 = 0.25;
const RESONANCE_WANDER_BIAS_CELLS: i16 = 3;

fn apply_resonance_reaction(
    mut profile: PetLifeProfile,
    resonant: Option<&HabitatPropId>,
) -> PetLifeProfile {
    let Some(id) = resonant else {
        return profile;
    };
    if profile.prop_reactions.iter().any(|r| r.prop_id == *id) {
        return profile;
    }
    profile.prop_reactions.push(PropReaction {
        prop_id: id.clone(),
        intensity: RESONANCE_REACTION_INTENSITY,
        kind: PropReactionKind::Glow,
    });
    profile
}

fn resonance_wander_bias(resonant: Option<&HabitatPropId>) -> i16 {
    let Some(spec) = resonant.and_then(catalog_prop) else {
        return 0;
    };
    let side: i16 = match spec.zone {
        HabitatPropZone::FloorLeft | HabitatPropZone::WallLeft | HabitatPropZone::AirLeft => -1,
        HabitatPropZone::FloorRight | HabitatPropZone::WallRight | HabitatPropZone::AirRight => 1,
        HabitatPropZone::FloorMid | HabitatPropZone::AirMid | HabitatPropZone::Ceiling => 0,
    };
    side * RESONANCE_WANDER_BIAS_CELLS
}

fn work_weather_seed(weather: WorkWeather) -> u64 {
    match weather {
        WorkWeather::Clear => 0,
        WorkWeather::CacheMist => 1,
        WorkWeather::OutputSparks => 2,
        WorkWeather::ReasoningPulse => 3,
        WorkWeather::Mixed => 4,
    }
}

fn activity_glyph_color(profile: &PetLifeProfile) -> Color {
    let p = crate::tui::style::tokenpet_palette();
    let weather = match profile.work_weather {
        WorkWeather::CacheMist => p.good.rgb,
        WorkWeather::OutputSparks => p.accent.rgb,
        WorkWeather::ReasoningPulse => p.bad.rgb,
        WorkWeather::Mixed => p.good.rgb,
        WorkWeather::Clear => p.accent.rgb,
    };
    if let Some(accent) = profile.source_accent {
        if profile.work_weather == WorkWeather::Clear {
            source_accent_color(accent)
        } else {
            blend_colors(source_accent_color(accent), weather, 0.65)
        }
    } else {
        weather
    }
}

fn source_accent_color(accent: SourceAccent) -> Color {
    match accent {
        SourceAccent::Claude => Color::Rgb(0xb3, 0x9d, 0xff),
        SourceAccent::Codex => Color::Rgb(0x86, 0xd9, 0xef),
        SourceAccent::Balanced | SourceAccent::Ensemble => Color::Rgb(0xf0, 0xc4, 0x6a),
    }
}

fn blend_colors(primary: Color, secondary: Color, primary_weight: f32) -> Color {
    let (Color::Rgb(pr, pg, pb), Color::Rgb(sr, sg, sb)) = (primary, secondary) else {
        return primary;
    };
    let weight = primary_weight.clamp(0.0, 1.0);
    let inv = 1.0 - weight;
    Color::Rgb(
        ((pr as f32 * weight) + (sr as f32 * inv)).round() as u8,
        ((pg as f32 * weight) + (sg as f32 * inv)).round() as u8,
        ((pb as f32 * weight) + (sb as f32 * inv)).round() as u8,
    )
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) = (a, b) else {
        return a;
    };
    let t = t.clamp(0.0, 1.0);
    Color::Rgb(
        ((ar as f32 * (1.0 - t)) + (br as f32 * t)).round() as u8,
        ((ag as f32 * (1.0 - t)) + (bg as f32 * t)).round() as u8,
        ((ab as f32 * (1.0 - t)) + (bb as f32 * t)).round() as u8,
    )
}

fn warm_shift(base: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = base else {
        return base;
    };
    let t = amount.clamp(0.0, 1.0);
    let add = (t * 40.0).round() as u8;
    let sub = (t * 30.0).round() as u8;
    Color::Rgb(r.saturating_add(add), g, b.saturating_sub(sub))
}

fn dim_shift(base: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = base else {
        return base;
    };
    let t = amount.clamp(0.0, 1.0);
    let m = 1.0 - t * 0.5;
    Color::Rgb(
        (r as f32 * m).round() as u8,
        (g as f32 * m).round() as u8,
        (b as f32 * m).round() as u8,
    )
}

/// Per-phase sky glyph family, with `date_seed` picking among authored
/// variants per (species, phase) — the day's character is visual texture
/// only, never personality content (locked rule). Night stays a sparse
/// starfield, dawn/dusk warm grain, day a species family.
fn sky_palette_for_phase(species: Species, phase: DayPhase, date_seed: u64) -> &'static [char] {
    let variant = (date_seed % 2) as usize;
    match phase {
        DayPhase::Day => {
            if variant == 0 {
                sky_palette_for(species)
            } else {
                match species {
                    Species::Fuzz => &['*', '·', '`', '.'],
                    Species::Blob => &['o', '·', '°', '.'],
                    Species::Ghost => &['\'', '~', '·', ','],
                    Species::Glitch => &['░', '▒', '▪', ':', ';'],
                    Species::Crystal => &['✧', '◆', '✦', '◇'],
                    Species::Mech => &['°', '·', '─', '○'],
                }
            }
        }
        DayPhase::Dawn | DayPhase::Dusk => {
            let variants: [&'static [char]; 2] = match species {
                Species::Glitch => [&['░', '▪', ':', ' '], &['▒', '░', '▪', ' ']],
                Species::Crystal => [&['✦', '✧', '·', ' '], &['◇', '◆', '·', ' ']],
                _ => [&['·', '\'', '~', ' '], &['\'', ',', '·', ' ']],
            };
            variants[variant]
        }
        DayPhase::Night => {
            let variants: [&'static [char]; 2] = match species {
                Species::Glitch => [&['▪', ':', ' ', ' '], &[';', '▪', '░', ' ']],
                Species::Crystal => [&['✦', '◇', ' ', ' '], &['◆', '✧', '·', ' ']],
                _ => [&['✦', '·', '*', ' '], &['*', '·', '✧', ' ']],
            };
            variants[variant]
        }
    }
}

/// Sky glyph budget scale per phase. Night <= day, always.
fn phase_count_scale(phase: DayPhase) -> f64 {
    match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.7,
        DayPhase::Dusk => 0.8,
        DayPhase::Night => 0.6,
    }
}

/// Bounded seasonal hue drift on the sky color. Summer is the neutral
/// reference; the other seasons nudge channels by at most
/// SEASON_DRIFT_MAX_CHANNEL_NUDGE. Drift only — the season is never named in
/// any UI text, speech, or dream (spec: Seasons).
#[allow(dead_code)]
const SEASON_DRIFT_MAX_CHANNEL_NUDGE: u8 = 8;

fn season_hue_drift(color: Color, season: Season) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    match season {
        Season::Summer => color,
        Season::Spring => Color::Rgb(r, g.saturating_add(6), b),
        Season::Autumn => Color::Rgb(r.saturating_add(7), g, b.saturating_sub(5)),
        Season::Winter => Color::Rgb(r.saturating_sub(4), g, b.saturating_add(7)),
    }
}

/// Ambient tint bias from the 7-day climate class. None and Clear both
/// render nothing (spec: Climate rendering); the class -> color mapping
/// matches the live weather channel's activity_glyph_color so the two
/// channels never disagree about what a class looks like.
const CLIMATE_TINT_WEIGHT: f32 = 0.12;

fn climate_tint(color: Color, climate: Option<WorkWeather>) -> Color {
    let p = crate::tui::style::tokenpet_palette();
    let target = match climate {
        None | Some(WorkWeather::Clear) => return color,
        Some(WorkWeather::CacheMist) => p.good.rgb,
        Some(WorkWeather::OutputSparks) => p.accent.rgb,
        Some(WorkWeather::ReasoningPulse) => p.bad.rgb,
        Some(WorkWeather::Mixed) => p.good.rgb,
    };
    lerp_color(color, target, CLIMATE_TINT_WEIGHT)
}

/// Sky color for `phase`, interpolated from the neutral dim base toward the
/// phase's target warmth/dim over `blend` (0.0 at the boundary, 1.0 after
/// PHASE_BLEND_MINUTES), then drifted by season and biased by climate.
/// Summer + None/Clear is the neutral identity.
fn sky_color_for_phase(
    phase: DayPhase,
    blend: f32,
    season: Season,
    climate: Option<WorkWeather>,
) -> Color {
    let p = crate::tui::style::tokenpet_palette();
    let base = p.dim.rgb;
    let target = match phase {
        DayPhase::Day => base,
        DayPhase::Dawn => warm_shift(base, 0.25),
        DayPhase::Dusk => warm_shift(base, 0.40),
        DayPhase::Night => dim_shift(base, 0.40),
    };
    climate_tint(
        season_hue_drift(lerp_color(base, target, blend), season),
        climate,
    )
}

fn profile_token_pop(
    last_feed_pulse_at: Option<time::OffsetDateTime>,
    profile: &PetLifeProfile,
    color_capability: ColorCapability,
    now: time::OffsetDateTime,
) -> Option<TokenPop> {
    if profile.calm_mode
        || profile.burst_level <= 0.0
        || matches!(color_capability, ColorCapability::Flat)
    {
        return None;
    }
    compute_token_pop(last_feed_pulse_at, now)
}

fn activity_lift_style(
    style: Style,
    activity_level: f32,
    color_capability: ColorCapability,
) -> Style {
    if matches!(color_capability, ColorCapability::Flat) {
        return style;
    }
    let lift = (activity_level.clamp(0.0, 2.0) * 22.0) as u8;
    match style.fg {
        Some(Color::Rgb(r, g, b)) => style.fg(Color::Rgb(
            r.saturating_add(lift),
            g.saturating_add(lift),
            b.saturating_add(lift),
        )),
        _ => style,
    }
}

fn apply_prop_reaction_style(
    style: Style,
    reaction: Option<&PropReaction>,
    color_capability: ColorCapability,
) -> Style {
    if matches!(color_capability, ColorCapability::Flat) {
        return style;
    }
    let Some(reaction) = reaction else {
        return style;
    };
    let lift = (reaction.intensity.clamp(0.0, 1.0) * 35.0) as u8;
    match style.fg {
        Some(Color::Rgb(r, g, b)) => style.fg(Color::Rgb(
            r.saturating_add(lift),
            g.saturating_add(lift),
            b.saturating_add(lift),
        )),
        _ => style,
    }
}

fn lift_pet_styles_for_activity(
    styles: &SemanticStyles,
    activity_level: f32,
    color_capability: ColorCapability,
) -> SemanticStyles {
    let mut s = styles.clone();
    s.pet_body = activity_lift_style(s.pet_body, activity_level, color_capability);
    s.pet_eye = activity_lift_style(s.pet_eye, activity_level, color_capability);
    s.pet_mouth = activity_lift_style(s.pet_mouth, activity_level, color_capability);
    s.pet_accent = activity_lift_style(s.pet_accent, activity_level, color_capability);
    s.pet_pattern = activity_lift_style(s.pet_pattern, activity_level, color_capability);
    s
}

fn overlaps_any(g: &AmbientGlyph, exclusions: &[Rect]) -> bool {
    exclusions.iter().any(|r| {
        g.col >= r.x
            && g.col < r.x.saturating_add(r.width)
            && g.row >= r.y
            && g.row < r.y.saturating_add(r.height)
    })
}

/// Returns ambient backdrop glyphs for the habitat area behind the pet art.
///
/// Positions are seeded by `(species, stage, minute_floor)` so output is stable
/// within a minute and drifts across minutes. Any glyph that would land inside
/// an exclusion rect is rejected; the caller is responsible for inflating
/// exclusions to enforce a desired margin. A floor row fills the bottom of the
/// habitat with the Starter-biome ground cover.
pub fn ambient_glyphs_for(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
) -> Vec<AmbientGlyph> {
    ambient_glyphs_for_phase(
        species,
        stage,
        crate::tui::room::RoomBiomeTag::Starter,
        habitat,
        exclusions,
        now,
        color_capability,
        DayPhase::Day,
        1.0,
        0,
        Season::Summer,
        None,
    )
}

/// Phase-aware variant of [`ambient_glyphs_for`].
///
/// Sky glyphs re-skin the same allocation; night uses a sparser starfield,
/// dawn/dusk use warm grain, and day keeps the species palette. Colors and
/// floor shading interpolate for `PHASE_BLEND_MINUTES` after a phase boundary.
#[allow(clippy::too_many_arguments)]
pub fn ambient_glyphs_for_phase(
    species: Species,
    stage: Stage,
    biome: crate::tui::room::RoomBiomeTag,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
    phase: DayPhase,
    phase_blend: f32,
    date_seed: u64,
    season: Season,
    climate: Option<WorkWeather>,
) -> Vec<AmbientGlyph> {
    if matches!(color_capability, ColorCapability::Flat) {
        return Vec::new();
    }

    // height < 2 means there's no room for both a sky row and a floor row;
    // the sky-row range would be 0..0 and rng.gen_range would panic.
    if habitat.width == 0 || habitat.height < 2 {
        return Vec::new();
    }

    // Seed: (species, stage, minute-floor). Same minute → identical positions.
    let s_seed = species_seed(species);
    let st_seed = stage_seed(stage);
    let minute_floor = (now.unix_timestamp() / 60) as u64;
    let seed = s_seed
        .wrapping_mul(0x9E37_79B1_7F4A_7C15)
        .wrapping_add(st_seed.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(minute_floor.wrapping_mul(0x94D0_49BB_1331_11EB));
    let mut rng = Pcg32::seed_from_u64(seed);

    let sky = sky_palette_for_phase(species, phase, date_seed);
    let floor = biome_floor_palette(biome);

    let p = crate::tui::style::tokenpet_palette();
    let base = p.dim.rgb;
    let sky_color = sky_color_for_phase(phase, phase_blend, season, climate);
    // Floor gradually dims into Night; Dawn/Dusk keep the neutral base so the
    // pet remains readable against warm grain.
    let floor_color = if phase == DayPhase::Night {
        dim_shift(base, 0.40 * phase_blend)
    } else {
        base
    };

    let mut glyphs = Vec::new();

    let habitat_cells = (habitat.width as usize) * (habitat.height as usize);
    let area_term = habitat_cells.saturating_sub(200) / 60;
    let count =
        ((stage_base_count(stage) + area_term) as f64 * phase_count_scale(phase)).round() as usize;

    for _ in 0..count {
        // Reject-sample up to N times to find a free cell.
        for _attempt in 0..16 {
            let col = habitat.x + rng.gen_range(0..habitat.width);
            let row = habitat.y + rng.gen_range(0..habitat.height.saturating_sub(1)); // leave bottom row for floor
            let candidate = AmbientGlyph {
                row,
                col,
                glyph: *sky.choose(&mut rng).unwrap_or(&' '),
                color: sky_color,
            };
            if !overlaps_any(&candidate, exclusions) {
                glyphs.push(candidate);
                break;
            }
        }
    }

    // Floor row: anchored to the bottom of habitat.
    let floor_row = habitat.y + habitat.height.saturating_sub(1);
    for dx in 0..habitat.width {
        let col = habitat.x + dx;
        let candidate = AmbientGlyph {
            row: floor_row,
            col,
            glyph: *floor.choose(&mut rng).unwrap_or(&' '),
            color: floor_color,
        };
        if !overlaps_any(&candidate, exclusions) {
            glyphs.push(candidate);
        }
    }

    glyphs
}

/// Soft-saturating day-accumulation density in `today_ratio`: asymptotic and
/// sub-countable, so no learnable "full room" exists. No numbers, no fill
/// direction, no completion framing (spec: Day accumulation).
fn mote_density(ratio: f32) -> f32 {
    1.0 - (-ratio.max(0.0)).exp()
}

/// Day-accumulation floor motes. Density tracks `today_ratio` with soft
/// saturation, capped at MOTE_BUDGET_SHARE of the ambient allocation.
/// Placement is jittered by `date_seed` and stable all day — the room
/// accumulates instead of reshuffling, and a growing count extends the same
/// position sequence so existing motes hold still. For the first
/// MOTE_TIDY_FADE_MINUTES after the local day started, yesterday's density
/// fades to zero instead of vanishing at 00:00 (`date_seed` rolls at dawn,
/// not midnight, so the fading set keeps last night's positions). Flat
/// renders zero motes (ambient contract unchanged); immature pets render
/// zero (spec: Maturity gate governs every baseline-ratio channel).
fn mote_glyphs_for(
    day: &crate::tui::day::DayContext,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: crate::tui::style::ColorCapability,
) -> Vec<AmbientGlyph> {
    if matches!(color_capability, ColorCapability::Flat) || !day.mature {
        return Vec::new();
    }
    if habitat.width == 0 || habitat.height < 3 {
        return Vec::new();
    }

    let habitat_cells = (habitat.width as usize) * (habitat.height as usize);
    let area_term = habitat_cells.saturating_sub(200) / 60;
    let allocation_floor = (4 + area_term) as f64 * phase_count_scale(day.day_phase);
    let budget = (MOTE_BUDGET_SHARE * allocation_floor).floor();

    let today_count = (budget * f64::from(mote_density(day.today_ratio))).round() as usize;

    let fade_elapsed = (now - day.local_day_started_utc).whole_seconds() as f32;
    let fade_window = (crate::tui::day::MOTE_TIDY_FADE_MINUTES as f32) * 60.0;
    let fading_count = match day.yesterday {
        Some(y) if fade_elapsed >= 0.0 && fade_elapsed < fade_window => {
            let remaining = 1.0 - fade_elapsed / fade_window;
            (budget * f64::from(mote_density(y.ratio) * remaining)).round() as usize
        }
        _ => 0,
    };
    let count = today_count.max(fading_count);
    if count == 0 {
        return Vec::new();
    }

    let mut rng = Pcg32::seed_from_u64(day.date_seed.wrapping_mul(0xA076_1D64_78BD_642F));
    let p = crate::tui::style::tokenpet_palette();
    let color = warm_shift(p.dim.rgb, 0.15);
    let band = (habitat.height / 3)
        .max(1)
        .min(habitat.height.saturating_sub(2));
    let band_top = habitat.y + habitat.height - 1 - band;
    let mut glyphs: Vec<AmbientGlyph> = Vec::with_capacity(count);
    for _ in 0..count {
        for _attempt in 0..16 {
            let col = habitat.x + rng.gen_range(0..habitat.width);
            let row = band_top + rng.gen_range(0..band);
            let candidate = AmbientGlyph {
                row,
                col,
                glyph: *MOTE_GLYPHS.choose(&mut rng).unwrap_or(&'·'),
                color,
            };
            if !overlaps_any(&candidate, exclusions)
                && !glyphs
                    .iter()
                    .any(|g| g.col == candidate.col && g.row == candidate.row)
            {
                glyphs.push(candidate);
                break;
            }
        }
    }
    glyphs
}

/// Live-activity channels always win over weekend softening: any live
/// signal suppresses it entirely (spec: Weekend texture).
fn effective_weekend_softening(day: &crate::tui::day::DayContext, profile: &PetLifeProfile) -> f32 {
    if profile.burst_level > 0.0 || profile.activity_level > 0.0 {
        return 0.0;
    }
    crate::tui::day::weekend_softening(day)
}

/// Weekend palette softening: pulls a scene color toward the neutral dim
/// base. Applied to the ambient and mote passes only — activity glyphs and
/// the pet itself are live channels and stay untouched.
const WEEKEND_PALETTE_SOFTEN_MAX: f32 = 0.25;

fn weekend_soften_color(color: Color, softening: f32) -> Color {
    if softening <= 0.0 {
        return color;
    }
    let p = crate::tui::style::tokenpet_palette();
    lerp_color(
        color,
        p.dim.rgb,
        WEEKEND_PALETTE_SOFTEN_MAX * softening.clamp(0.0, 1.0),
    )
}

/// Returns extra work-activity glyphs for the habitat backdrop.
fn activity_glyphs_for(
    profile: &PetLifeProfile,
    species: Species,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
    count: usize,
) -> Vec<AmbientGlyph> {
    if count == 0
        || profile.calm_mode
        || profile.activity_level <= 0.0
        || habitat.width == 0
        || habitat.height == 0
        || matches!(color_capability, ColorCapability::Flat)
    {
        return Vec::new();
    }

    let activity_bucket = (profile.activity_level.clamp(0.0, 2.0) * 10.0).round() as u64;
    let minute_floor = (now.unix_timestamp() / 60) as u64;
    let seed = species_seed(species)
        .wrapping_mul(0xD6E8_FD9A_934D_CC17)
        .wrapping_add(minute_floor.wrapping_mul(0xA076_1D64_78BD_642F))
        .wrapping_add(activity_bucket.wrapping_mul(0xE703_7ED1_A0B4_28DB))
        .wrapping_add(work_weather_seed(profile.work_weather).wrapping_mul(0x8EBC_6AF0_9C88_C6E3));
    let mut rng = Pcg32::seed_from_u64(seed);
    let palette = ['\u{2726}', '\u{2727}', '\u{00b7}', '*'];
    let color = activity_glyph_color(profile);
    let mut glyphs = Vec::with_capacity(count);

    for _ in 0..count {
        for _attempt in 0..32 {
            let col = habitat.x + rng.gen_range(0..habitat.width);
            let row = habitat.y + rng.gen_range(0..habitat.height);
            let candidate = AmbientGlyph {
                row,
                col,
                glyph: *palette.choose(&mut rng).unwrap_or(&'*'),
                color,
            };
            if !overlaps_any(&candidate, exclusions)
                && !glyphs
                    .iter()
                    .any(|g: &AmbientGlyph| g.col == candidate.col && g.row == candidate.row)
            {
                glyphs.push(candidate);
                break;
            }
        }
    }

    glyphs
}

/// Returns 1×1 exclusion rects covering every non-space pet cell plus an
/// 8-neighbor halo around each, in absolute terminal coordinates anchored to
/// `pet_rect`. Used in place of an inflated bounding rect so habitat glyphs
/// can fill the diamond's negative space while still keeping a one-cell
/// breathing margin around the pet itself.
///
/// `mirror` should match the orientation `build_pet_lines` will draw with
/// (true when the pet faces left), so the halo aligns with the rendered art.
pub(crate) fn pet_silhouette_halo_rects(
    art_lines: &[String],
    pet_rect: Rect,
    mirror: bool,
) -> Vec<Rect> {
    let mut cells: std::collections::HashSet<(u16, u16)> = std::collections::HashSet::new();
    for (row_idx, line) in art_lines.iter().enumerate() {
        let row_offset = row_idx as u16;
        let line_width = line.chars().count();
        for (col_idx, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let col_in_frame: u16 = if mirror {
                (line_width.saturating_sub(1).saturating_sub(col_idx)) as u16
            } else {
                col_idx as u16
            };
            let col_base = i32::from(pet_rect.x) + i32::from(col_in_frame);
            let row_base = i32::from(pet_rect.y) + i32::from(row_offset);
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = col_base + dx;
                    let ny = row_base + dy;
                    if nx < 0 || ny < 0 {
                        continue;
                    }
                    if let (Ok(x), Ok(y)) = (u16::try_from(nx), u16::try_from(ny)) {
                        cells.insert((x, y));
                    }
                }
            }
        }
    }
    cells
        .into_iter()
        .map(|(x, y)| Rect::new(x, y, 1, 1))
        .collect()
}

fn ambient_glyph_is_inside_area(glyph: &AmbientGlyph, area: Rect) -> bool {
    glyph.col >= area.x
        && glyph.row >= area.y
        && glyph.col < area.x.saturating_add(area.width)
        && glyph.row < area.y.saturating_add(area.height)
}

impl LegacyPanel for PetPanel {
    fn preferred_constraint(&self, _vm: &WatchViewModel) -> Constraint {
        Constraint::Fill(1)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext) {
        // Compute the wander position and facing from the live area width and
        // wall clock so both stay consistent with each other regardless of what
        // vm.wander_offset_x or vm.facing carry.
        let now = ctx.clock.now_utc();
        let species = vm.pet_render.generated_species;
        let day = &vm.day_context;
        let resonant_prop = {
            let earned: Vec<crate::storage::state::EarnedHabitatProp> = vm
                .habitat
                .earned_props
                .iter()
                .map(|prop| crate::storage::state::EarnedHabitatProp {
                    id: prop.id.clone(),
                    earned_at: prop.earned_at,
                    source: prop.source.clone(),
                })
                .collect();
            crate::tui::day::resonant_prop_for_day(day, &earned)
        };
        let softening = effective_weekend_softening(day, &vm.life_profile);
        let idle_minutes = vm.life_profile.idle.idle_minutes;
        let (wander_x, facing) = match (day.asleep, day.sleep_onset_utc, day.wake_resume) {
            (true, Some(onset), _) => (
                compute_sleep_wander_x(area.width, species, now, onset, idle_minutes),
                compute_facing(area.width, species, onset, idle_minutes), // held facing: no mirror flips with shut eyes
            ),
            (false, _, Some(resume)) => (
                compute_wake_wander_x(
                    area.width,
                    species,
                    now,
                    resume.from_eval_utc,
                    resume.woke_at_utc,
                    idle_minutes,
                ),
                compute_facing(area.width, species, now, idle_minutes),
            ),
            _ => {
                let wander_now = lazy_wander_instant(now, day.local_day_started_utc, softening);
                (
                    compute_wander_position_x(area.width, species, wander_now, idle_minutes)
                        + resonance_wander_bias(resonant_prop.as_ref()),
                    compute_facing(area.width, species, wander_now, idle_minutes),
                )
            }
        };
        let vm = if wander_x != vm.wander_offset_x || facing != vm.facing {
            // Build a local copy with the computed values rather than mutating.
            std::borrow::Cow::Owned({
                let mut v = vm.clone();
                v.wander_offset_x = wander_x;
                v.facing = facing;
                v
            })
        } else {
            std::borrow::Cow::Borrowed(vm)
        };
        let vm = vm.as_ref();
        let scene = PetScene::compute_layout(area, vm, ctx);

        // Per-cell pet silhouette + 1-cell halo, shared by every pass that
        // wants pet avoidance. Replaces the inflated bounding rect so habitat
        // content can fill the diamond's negative space while keeping a
        // breathing margin around the actual pet outline.
        let species = vm.pet_render.generated_species;
        let stage = vm.pet_render.stage;
        let mirror = vm.facing == -1;
        let silhouette_halo = pet_silhouette_halo_rects(&vm.pet_art, scene.pet_art, mirror);

        // Pass 1: ambient backdrop — uses the silhouette halo as a per-cell
        // exclusion so dots/sparkles flow through the rect's negative space.
        let mut ambient_exclusions: Vec<Rect> = scene
            .exclusions
            .iter()
            .copied()
            .filter(|r| *r != scene.pet_art)
            .collect();
        ambient_exclusions.extend_from_slice(&silhouette_halo);

        // Alive room base: persistent biome, weather, and prop emitter glyphs
        // drawn before the existing ambient/mote/activity passes so they set
        // the room's silhouette without replacing pet or speech cells.
        let room_profile = crate::tui::room::derive_room_life_profile(vm, now);

        // Base layer: a subtle per-biome background wash over the habitat, so the
        // room reads as a place. Set BEFORE room/ambient glyphs (which set fg only,
        // leaving this bg intact underneath).
        {
            let wash = biome_wash_color(room_profile.biome.primary);
            for wy in scene.habitat.y..scene.habitat.y.saturating_add(scene.habitat.height) {
                for wx in scene.habitat.x..scene.habitat.x.saturating_add(scene.habitat.width) {
                    if !rects_contain(&ambient_exclusions, wx, wy) {
                        buf[(wx, wy)].set_style(ratatui::style::Style::default().bg(wash));
                    }
                }
            }
        }

        let room_glyphs = crate::tui::room::room_glyphs_for(
            &room_profile,
            scene.habitat,
            &ambient_exclusions,
            now,
            ctx.color_capability,
            vm.day_context.day_phase,
        );
        for g in room_glyphs {
            if !rects_contain(&ambient_exclusions, g.col, g.row) {
                let cell = &mut buf[(g.col, g.row)];
                cell.set_char(g.glyph);
                cell.set_style(g.style);
            }
        }

        let phase_blend = {
            let since = (now - vm.day_context.phase_started_at_utc).whole_seconds() as f32;
            (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
        };
        let glyphs = ambient_glyphs_for_phase(
            species,
            stage,
            room_profile.biome.primary,
            scene.habitat,
            &ambient_exclusions,
            now,
            ctx.color_capability,
            vm.day_context.day_phase,
            phase_blend,
            vm.day_context.date_seed,
            vm.day_context.season,
            vm.day_context.climate,
        );
        for g in glyphs {
            if ambient_glyph_is_inside_area(&g, scene.habitat) {
                let cell = &mut buf[(g.col, g.row)];
                cell.set_char(g.glyph);
                cell.set_style(
                    ratatui::style::Style::default().fg(weekend_soften_color(g.color, softening)),
                );
            }
        }
        // Mote pass: after ambient, before activity glyphs, same exclusions
        // (silhouette halo + speech) — spec: Day accumulation.
        let motes = mote_glyphs_for(
            &vm.day_context,
            scene.habitat,
            &ambient_exclusions,
            now,
            ctx.color_capability,
        );
        for g in motes {
            if ambient_glyph_is_inside_area(&g, scene.habitat) {
                let cell = &mut buf[(g.col, g.row)];
                cell.set_char(g.glyph);
                cell.set_style(Style::default().fg(weekend_soften_color(g.color, softening)));
            }
        }
        let compact = area.width <= 72 || area.height <= 24;
        let earned_prop_ids = vm
            .habitat
            .earned_props
            .iter()
            .map(|prop| prop.id.clone())
            .collect::<Vec<_>>();
        let life_profile = build_prop_reactions(vm.life_profile.clone(), &earned_prop_ids, compact);
        let life_profile = apply_resonance_reaction(life_profile, resonant_prop.as_ref());
        let extra_count = activity_glyph_budget(&life_profile, compact);
        let activity_glyphs = activity_glyphs_for(
            &life_profile,
            species,
            scene.habitat,
            &ambient_exclusions,
            now,
            ctx.color_capability,
            extra_count,
        );
        for g in activity_glyphs {
            if ambient_glyph_is_inside_area(&g, scene.habitat) {
                let cell = &mut buf[(g.col, g.row)];
                cell.set_char(g.glyph);
                cell.set_style(Style::default().fg(g.color));
            }
        }

        // Trophies + accents, classified by their pet-layer from the catalog.
        // Background avoids the silhouette halo; Behind ignores it (renders
        // pre-pet to sit visually behind); Foreground ignores it and renders
        // post-pet to sit visually in front.
        let prop_cells = habitat_props_for(
            &vm.habitat,
            &scene,
            &silhouette_halo,
            species,
            &vm.pet_render.seed,
            ctx,
        );
        for prop in &prop_cells {
            if matches!(
                prop.pet_layer,
                HabitatPetLayer::Background | HabitatPetLayer::Behind
            ) && habitat_contains(&scene, prop)
            {
                let reaction = life_profile
                    .prop_reactions
                    .iter()
                    .find(|reaction| reaction.prop_id == prop.prop_id);
                let cell = &mut buf[(prop.col, prop.row)];
                cell.set_char(prop.glyph);
                cell.set_style(apply_prop_reaction_style(
                    prop.style,
                    reaction,
                    ctx.color_capability,
                ));
            }
        }

        // Pet art with shimmer, twinkle, and token-pop overlays — paints over
        // any Background / Behind cells it touches via the silhouette.
        render_pet_inside(
            buf,
            vm,
            &scene,
            now,
            ctx.color_capability,
            room_profile.pet_performance,
        );

        // Tiny performance cue near the pet: one cell, never a template rewrite.
        apply_pet_performance_cues(
            buf,
            &scene,
            room_profile.pet_performance,
            ctx.color_capability,
        );

        // Foreground props paint on top of the pet, for whenever depth in
        // front of the pet is wanted (no foreground props in the catalog
        // today; the pass exists so adding one only requires a catalog flip).
        for prop in &prop_cells {
            if matches!(prop.pet_layer, HabitatPetLayer::Foreground)
                && habitat_contains(&scene, prop)
            {
                let reaction = life_profile
                    .prop_reactions
                    .iter()
                    .find(|reaction| reaction.prop_id == prop.prop_id);
                let cell = &mut buf[(prop.col, prop.row)];
                cell.set_char(prop.glyph);
                cell.set_style(apply_prop_reaction_style(
                    prop.style,
                    reaction,
                    ctx.color_capability,
                ));
            }
        }
    }
}

fn habitat_contains(scene: &PetSceneLayout, prop: &crate::tui::component::HabitatPropCell) -> bool {
    prop.col >= scene.habitat.x
        && prop.row >= scene.habitat.y
        && prop.col < scene.habitat.x.saturating_add(scene.habitat.width)
        && prop.row < scene.habitat.y.saturating_add(scene.habitat.height)
}

/// Overwrites one or two cells near the pet with a tiny performance cue glyph.
/// Keeps the rest of the pet template untouched — this is punctuation, not a
/// rewrite.
fn apply_pet_performance_cues(
    buf: &mut Buffer,
    scene: &PetSceneLayout,
    performance: crate::tui::room::PetPerformance,
    color_capability: ColorCapability,
) {
    let style = performance_cue_style(color_capability);
    match performance {
        crate::tui::room::PetPerformance::TiredAwake => mark_pet_floor(buf, scene, '˙', style),
        crate::tui::room::PetPerformance::HeavyDayCozy => mark_pet_floor(buf, scene, '~', style),
        crate::tui::room::PetPerformance::AsleepDreaming => mark_pet_air(buf, scene, 'z', style),
        crate::tui::room::PetPerformance::CatchUpWake => mark_pet_air(buf, scene, '^', style),
        crate::tui::room::PetPerformance::SourceBurstPerk => mark_pet_air(buf, scene, '!', style),
        crate::tui::room::PetPerformance::RestedAwake => {}
    }
}

fn performance_cue_style(color_capability: ColorCapability) -> Style {
    let color = if matches!(color_capability, ColorCapability::Flat) {
        crate::tui::style::tokenpet_palette().faint.rgb
    } else {
        Color::Rgb(0xd4, 0xa6, 0x57)
    };
    Style::default().fg(color)
}

/// Places `symbol` on the floor cell just below the pet's bounding rect,
/// clipped to the habitat area.
fn mark_pet_floor(buf: &mut Buffer, scene: &PetSceneLayout, symbol: char, style: Style) {
    let x = scene.pet_art.x + scene.pet_art.width / 2;
    let y = scene.pet_art.y.saturating_add(scene.pet_art.height);
    let within_habitat = x >= scene.habitat.x
        && y >= scene.habitat.y
        && x < scene.habitat.x.saturating_add(scene.habitat.width)
        && y < scene.habitat.y.saturating_add(scene.habitat.height);
    if within_habitat {
        let cell = &mut buf[(x, y)];
        cell.set_char(symbol);
        cell.set_style(style);
    }
}

/// Places `symbol` on the air cell just above the pet's bounding rect,
/// clipped to the habitat area. Skips the write when there is no row above
/// the pet, so the cue never overwrites pet art.
fn mark_pet_air(buf: &mut Buffer, scene: &PetSceneLayout, symbol: char, style: Style) {
    let x = scene.pet_art.x + scene.pet_art.width / 2;
    let y = scene.pet_art.y.saturating_sub(1);
    let above_pet = y < scene.pet_art.y;
    let within_habitat = x >= scene.habitat.x
        && y >= scene.habitat.y
        && x < scene.habitat.x.saturating_add(scene.habitat.width)
        && y < scene.habitat.y.saturating_add(scene.habitat.height);
    if above_pet && within_habitat {
        let cell = &mut buf[(x, y)];
        cell.set_char(symbol);
        cell.set_style(style);
    }
}

/// Resting brightness baseline by performance state, composed UNDER the
/// activity lift (a tired pet still visibly brightens when work arrives, it
/// just settles back lower). 1.0 = neutral. Bounded so the pet is never dark.
fn performance_lightness_multiplier(performance: crate::tui::room::PetPerformance) -> f32 {
    use crate::tui::room::PetPerformance::*;
    match performance {
        RestedAwake | CatchUpWake | SourceBurstPerk => 1.0,
        TiredAwake => 0.88,
        HeavyDayCozy => 0.82,
        AsleepDreaming => 0.7,
    }
}

/// Resting vertical offset (rows) by performance state. Settled states sit
/// one row lower; alert/rested stay put. Capped at 1 to preserve the quiet
/// halo around the pet.
fn performance_posture_offset(performance: crate::tui::room::PetPerformance) -> u16 {
    use crate::tui::room::PetPerformance::*;
    match performance {
        TiredAwake | HeavyDayCozy | AsleepDreaming => 1,
        RestedAwake | CatchUpWake | SourceBurstPerk => 0,
    }
}

/// Renders the speech bubble and pet art into `area`, centered vertically.
/// This is the pre-existing render logic extracted from the old `render` body.
fn render_pet_inside(
    buf: &mut Buffer,
    vm: &WatchViewModel,
    scene: &PetSceneLayout,
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
    pet_performance: crate::tui::room::PetPerformance,
) {
    let base = seed_pet_palette(&semantic_styles(), &vm.pet_palette);
    let energy_m = low_energy_lightness_multiplier(vm.energy);
    let perf_m = performance_lightness_multiplier(pet_performance);
    let droop = darken_pet_styles(&base, energy_m * perf_m);

    if let (Some(speech_area), Some(speech)) = (scene.speech, vm.current_speech.as_deref()) {
        render_speech_bubble(speech_area, buf, speech, &droop);
    }

    let species = vm.pet_render.generated_species;
    let shimmer_role = compute_shimmer_role(species, now);
    let twinkle = compute_twinkle(species, now, vm.life_profile.idle.idle_minutes);
    let token_pop = profile_token_pop(
        vm.last_feed_pulse_at,
        &vm.life_profile,
        color_capability,
        now,
    );

    // When the token-pop is active, override shimmer to Pattern for extra flash.
    let effective_shimmer_role = if token_pop.is_some() {
        Some(PaletteRoleName::Pattern)
    } else {
        shimmer_role
    };

    // Brighten multiplier: shimmer/pop boost lightness ~1.4×, clamped to 1.0
    // on the u8 channel (we apply it in brighten_style).
    let shimmer_m = if effective_shimmer_role.is_some() {
        1.4f32
    } else {
        1.0
    };
    let shimmer_styles = brighten_pet_role(&droop, effective_shimmer_role, shimmer_m);
    let activity_level = if vm.life_profile.calm_mode {
        0.0
    } else {
        vm.life_profile.activity_level
    };
    let live_styles =
        lift_pet_styles_for_activity(&shimmer_styles, activity_level, color_capability);

    // Twinkle: also place a sparkle at the token-pop center when pop is active.
    let effective_twinkle = if token_pop.is_some() {
        Some(crate::pet::animator::TwinkleSpec {
            row: 4,
            col: 5,
            glyph: '\u{2726}',
        })
    } else {
        twinkle
    };

    // Hit-test against the full column width so the cursor anywhere in the
    // panel triggers eye tracking, matching the pre-Fill behavior.
    let cursor_norm_x = cursor_normalized_x_within(vm, scene.hit_area);
    let posture = performance_posture_offset(pet_performance);
    let pet_rect = {
        let mut r = scene.pet_art;
        let max_y = scene.habitat.y + scene.habitat.height.saturating_sub(r.height);
        r.y = (r.y + posture).min(max_y);
        r
    };
    let lines = build_pet_lines(
        vm,
        pet_rect.width as usize,
        &live_styles,
        cursor_norm_x,
        effective_twinkle,
    );
    render_pet_lines_sparse(buf, pet_rect, &lines);
}

/// Draws `lines` into `area`, writing only non-space glyphs. Whitespace cells
/// pass through, leaving whatever the habitat / props passes wrote underneath
/// visible — so the pet's bounding rectangle no longer occludes the backdrop.
fn render_pet_lines_sparse(buf: &mut Buffer, area: Rect, lines: &[Line<'_>]) {
    let right = area.x.saturating_add(area.width);
    let bottom = area.y.saturating_add(area.height);
    for (row_idx, line) in lines.iter().enumerate() {
        let y = area.y.saturating_add(row_idx as u16);
        if y >= bottom {
            break;
        }
        let mut x = area.x;
        for span in &line.spans {
            if x >= right {
                break;
            }
            for ch in span.content.chars() {
                if x >= right {
                    break;
                }
                if ch != ' ' {
                    let cell = &mut buf[(x, y)];
                    cell.set_char(ch);
                    cell.set_style(span.style);
                }
                x = x.saturating_add(1);
            }
        }
    }
}

/// Render a small speech bubble: "« text »" centered above the pet, styled
/// with the accent color so it pops without being shouty.
fn render_speech_bubble(area: Rect, buf: &mut Buffer, text: &str, styles: &SemanticStyles) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let bubble = format!("« {text} »");
    let bubble_width = bubble.chars().count() as u16;
    let pad = (area.width.saturating_sub(bubble_width)) / 2;
    let line = Line::from(vec![
        Span::raw(" ".repeat(pad as usize)),
        Span::styled(bubble, styles.pet_accent),
    ]);
    Paragraph::new(line).render(area, buf);
}

/// Returns a copy of `base` with all pet-role foreground colors scaled by
/// `multiplier` (1.0 = unchanged, 0.55 = ~half lightness). Non-RGB colors
/// pass through unchanged.
fn darken_pet_styles(base: &SemanticStyles, multiplier: f32) -> SemanticStyles {
    let mut s = base.clone();
    s.pet_body = darken_style(s.pet_body, multiplier);
    s.pet_eye = darken_style(s.pet_eye, multiplier);
    s.pet_mouth = darken_style(s.pet_mouth, multiplier);
    s.pet_accent = darken_style(s.pet_accent, multiplier);
    s.pet_pattern = darken_style(s.pet_pattern, multiplier);
    s
}

/// Returns a copy of `base` where the style for `role` has its foreground
/// brightened by `multiplier`. Other roles are returned unchanged.
/// A multiplier > 1.0 brightens; use this for shimmer/token-pop effects.
fn brighten_pet_role(
    base: &SemanticStyles,
    role: Option<PaletteRoleName>,
    multiplier: f32,
) -> SemanticStyles {
    let Some(role) = role else {
        return base.clone();
    };
    let mut s = base.clone();
    match role {
        PaletteRoleName::Body => s.pet_body = brighten_style(s.pet_body, multiplier),
        PaletteRoleName::Accent => s.pet_accent = brighten_style(s.pet_accent, multiplier),
        PaletteRoleName::Pattern => s.pet_pattern = brighten_style(s.pet_pattern, multiplier),
        PaletteRoleName::Eye => s.pet_eye = brighten_style(s.pet_eye, multiplier),
        PaletteRoleName::Mouth => s.pet_mouth = brighten_style(s.pet_mouth, multiplier),
        PaletteRoleName::Particle => s.pet_accent = brighten_style(s.pet_accent, multiplier),
    }
    s
}

fn brighten_style(style: Style, multiplier: f32) -> Style {
    if let Some(Color::Rgb(r, g, b)) = style.fg {
        let m = multiplier.max(0.0);
        let r = (r as f32 * m).min(255.0) as u8;
        let g = (g as f32 * m).min(255.0) as u8;
        let b = (b as f32 * m).min(255.0) as u8;
        style.fg(Color::Rgb(r, g, b))
    } else {
        style
    }
}

fn darken_style(style: Style, multiplier: f32) -> Style {
    if let Some(Color::Rgb(r, g, b)) = style.fg {
        let m = multiplier.clamp(0.0, 1.0);
        let r = (r as f32 * m) as u8;
        let g = (g as f32 * m) as u8;
        let b = (b as f32 * m) as u8;
        style.fg(Color::Rgb(r, g, b))
    } else {
        style
    }
}

/// Hit-test the screen cursor against the pet panel rect. Returns normalized
/// x ∈ [-1.0, 1.0] relative to the panel center, or None when the pet is asleep, the cursor is
/// outside the rect, missing, or mouse tracking is disabled.
fn cursor_normalized_x_within(vm: &WatchViewModel, area: Rect) -> Option<f32> {
    if vm.day_context.asleep {
        return None;
    }
    if !vm.mouse_tracking_enabled {
        return None;
    }
    let (cx, cy) = vm.cursor_screen?;
    if cx < area.x || cx >= area.x + area.width || cy < area.y || cy >= area.y + area.height {
        return None;
    }
    let local_x = (cx - area.x) as f32;
    let width = area.width.max(1) as f32;
    Some((local_x / width) * 2.0 - 1.0)
}

/// Pick the cursor-tracked eye glyph based on normalized x position.
/// Left third → looking left; middle → straight; right third → looking right.
fn cursor_eye_glyph(norm_x: f32) -> char {
    if norm_x < -0.33 {
        '<'
    } else if norm_x > 0.33 {
        '>'
    } else {
        'o'
    }
}

/// Build a replacement eye string that matches the original eye span's width.
/// For span widths >= 3 ("o o" / "^ ^" style) we render `glyph` at both ends
/// with a single space in between — both eyes track together. For shorter
/// spans we render just the glyph. For longer spans we pad with spaces.
fn build_cursor_eye_string(glyph: char, span_width: usize) -> String {
    match span_width {
        0 => String::new(),
        1 | 2 => glyph.to_string(),
        n => {
            let mut s = String::with_capacity(n);
            s.push(glyph);
            for _ in 0..(n - 2) {
                s.push(' ');
            }
            s.push(glyph);
            s
        }
    }
}

fn build_pet_lines(
    vm: &WatchViewModel,
    area_width: usize,
    styles: &SemanticStyles,
    cursor_norm_x: Option<f32>,
    twinkle: Option<crate::pet::animator::TwinkleSpec>,
) -> Vec<Line<'static>> {
    let mirror = vm.facing == -1;

    // Build the (possibly mirrored) art lines and spans as owned Strings.
    let (art_lines, art_spans): (Vec<String>, Vec<crate::pet::render::StyledSegment>) = if mirror {
        let mirrored_lines: Vec<String> = vm.pet_art.iter().map(|l| mirror_line(l)).collect();
        let mirrored_spans = mirror_spans(&vm.pet_spans, &mirrored_lines);
        (mirrored_lines, mirrored_spans)
    } else {
        (vm.pet_art.clone(), vm.pet_spans.clone())
    };

    let pet_width = art_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    // scene.pet_art is already positioned at the wander offset by
    // pet_inner_rect_in_panel, so the lines themselves only need to center
    // within their own narrow rect.
    let center_pad = area_width.saturating_sub(pet_width) / 2;
    let left_pad = center_pad;
    let cursor_eye = cursor_norm_x.map(cursor_eye_glyph);

    art_lines
        .into_iter()
        .enumerate()
        .map(|(line_index, art_line)| {
            // Apply twinkle: if this line/col matches, substitute the glyph.
            // The framed art is 13×10 so art_line is a frame line.
            // twinkle.row is 0-based within the 11×8 art grid; frame adds 1 to row.
            let twinkle_col = twinkle.and_then(|t| {
                if t.row as usize + 1 == line_index {
                    Some((t.col as usize + 1, t.glyph))
                } else {
                    None
                }
            });

            let mut spans: Vec<Span<'static>> = Vec::new();
            if left_pad > 0 {
                spans.push(Span::raw(" ".repeat(left_pad)));
            }
            let eye_override = cursor_eye;
            let palette = palette_from_styles(styles);
            spans.extend(build_owned_spans_for_line(
                &art_line,
                line_index,
                &art_spans,
                styles,
                &palette,
                eye_override,
                twinkle_col,
            ));
            Line::from(spans)
        })
        .collect()
}

/// Mirrors an art line: reverses characters and substitutes directional glyphs.
pub(crate) fn mirror_line(line: &str) -> String {
    line.chars().rev().map(mirror_char).collect()
}

fn mirror_char(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '/' => '\\',
        '\\' => '/',
        '<' => '>',
        '>' => '<',
        'd' => 'b',
        'b' => 'd',
        '{' => '}',
        '}' => '{',
        '[' => ']',
        ']' => '[',
        _ => c,
    }
}

/// Re-build StyledSegments for mirrored lines by mirroring each span's
/// start/end positions within its line.
fn mirror_spans(
    spans: &[crate::pet::render::StyledSegment],
    mirrored_lines: &[String],
) -> Vec<crate::pet::render::StyledSegment> {
    spans
        .iter()
        .map(|seg| {
            let line_len = mirrored_lines
                .get(seg.line)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            let span_width = seg.end.saturating_sub(seg.start);
            // Mirror: new_start = line_len - seg.end, new_end = line_len - seg.start
            let new_start = line_len.saturating_sub(seg.end);
            let new_end = new_start + span_width;
            crate::pet::render::StyledSegment {
                line: seg.line,
                start: new_start,
                end: new_end,
                role: seg.role,
            }
        })
        .collect()
}

/// Build owned `Vec<Span<'static>>` for one art line, applying eye override
/// and optional twinkle glyph injection.
fn build_owned_spans_for_line(
    art_line: &str,
    line_index: usize,
    pet_spans: &[crate::pet::render::StyledSegment],
    styles: &SemanticStyles,
    palette: &crate::pet::palette::ResolvedPalette,
    eye_override: Option<char>,
    twinkle_col: Option<(usize, char)>,
) -> Vec<Span<'static>> {
    let total_chars = art_line.chars().count();
    if total_chars == 0 {
        return Vec::new();
    }

    let mut segments: Vec<&crate::pet::render::StyledSegment> = pet_spans
        .iter()
        .filter(|s| s.line == line_index && s.start < s.end && s.start < total_chars)
        .collect();
    segments.sort_by_key(|s| s.start);

    let char_indices = char_byte_indices(art_line);

    if segments.is_empty() {
        let body = char_slice(art_line, &char_indices, 0, total_chars).to_string();
        let body = apply_twinkle_in_range(body, 0, total_chars, twinkle_col);
        return vec![Span::styled(body, styles.pet_body)];
    }

    // Build owned spans. Each "slot" is: optional body-gap, then the styled segment.
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut cursor = 0usize;

    for segment in &segments {
        let start = segment.start.max(cursor).min(total_chars);
        let end = segment.end.min(total_chars);
        if end <= cursor {
            continue;
        }
        if start > cursor {
            let body_text = char_slice(art_line, &char_indices, cursor, start).to_string();
            let body_text = apply_twinkle_in_range(body_text, cursor, start, twinkle_col);
            spans.push(Span::styled(body_text, styles.pet_body));
        }
        let style = pet_role_style(segment.role, palette);
        let value = if let (Some(glyph), crate::pet::render::PaletteRoleName::Eye) =
            (eye_override, segment.role)
        {
            let span_width = end - start;
            build_cursor_eye_string(glyph, span_width)
        } else {
            char_slice(art_line, &char_indices, start, end).to_string()
        };
        let value = apply_twinkle_in_range(value, start, end, twinkle_col);
        spans.push(Span::styled(value, style));
        cursor = end;
    }
    if cursor < total_chars {
        let tail = char_slice(art_line, &char_indices, cursor, total_chars).to_string();
        let tail = apply_twinkle_in_range(tail, cursor, total_chars, twinkle_col);
        spans.push(Span::styled(tail, styles.pet_body));
    }

    spans
}

/// If `twinkle_col` falls within `[start, end)`, substitute that character in
/// `text` with the twinkle glyph. Otherwise returns `text` unchanged.
fn apply_twinkle_in_range(
    text: String,
    start: usize,
    end: usize,
    twinkle_col: Option<(usize, char)>,
) -> String {
    let Some((col, glyph)) = twinkle_col else {
        return text;
    };
    if col < start || col >= end {
        return text;
    }
    let local = col - start;
    let mut chars: Vec<char> = text.chars().collect();
    if local < chars.len() {
        chars[local] = glyph;
    }
    chars.into_iter().collect()
}

pub(crate) fn pet_role_spans_for_line<'a>(
    art_line: &'a str,
    line_index: usize,
    pet_spans: &'a [crate::pet::render::StyledSegment],
    styles: &'a SemanticStyles,
    palette: &'a crate::pet::palette::ResolvedPalette,
    eye_override: Option<char>,
) -> Vec<Span<'a>> {
    let _ = styles;
    let total_chars = art_line.chars().count();
    if total_chars == 0 {
        return Vec::new();
    }

    let mut segments: Vec<&crate::pet::render::StyledSegment> = pet_spans
        .iter()
        .filter(|s| s.line == line_index && s.start < s.end && s.start < total_chars)
        .collect();
    segments.sort_by_key(|s| s.start);

    if segments.is_empty() {
        return vec![Span::styled(
            art_line,
            pet_role_style(PaletteRoleName::Body, palette),
        )];
    }

    let char_indices = char_byte_indices(art_line);
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut cursor = 0usize;

    for segment in segments {
        let start = segment.start.max(cursor).min(total_chars);
        let end = segment.end.min(total_chars);
        if end <= cursor {
            continue;
        }
        if start > cursor {
            let body = char_slice(art_line, &char_indices, cursor, start);
            spans.push(Span::styled(
                body,
                pet_role_style(PaletteRoleName::Body, palette),
            ));
        }
        let style = pet_role_style(segment.role, palette);
        if let (Some(glyph), crate::pet::render::PaletteRoleName::Eye) =
            (eye_override, segment.role)
        {
            // Authored eye slots are typically 3+ chars wide ("o o", "^ ^",
            // "v v" etc.). Preserve the original span width so the right
            // eye doesn't disappear — place the cursor glyph at both ends
            // of the span with the existing inner characters between them.
            let span_width = end - start;
            let replaced = build_cursor_eye_string(glyph, span_width);
            spans.push(Span::styled(replaced, style));
        } else {
            let value = char_slice(art_line, &char_indices, start, end);
            spans.push(Span::styled(value, style));
        }
        cursor = end;
    }

    if cursor < total_chars {
        let tail = char_slice(art_line, &char_indices, cursor, total_chars);
        spans.push(Span::styled(
            tail,
            pet_role_style(PaletteRoleName::Body, palette),
        ));
    }

    spans
}

fn char_byte_indices(line: &str) -> Vec<usize> {
    let mut indices: Vec<usize> = line.char_indices().map(|(byte, _)| byte).collect();
    indices.push(line.len());
    indices
}

fn char_slice<'a>(line: &'a str, indices: &[usize], start_char: usize, end_char: usize) -> &'a str {
    let start = indices[start_char];
    let end = indices[end_char];
    &line[start..end]
}

pub(crate) fn pet_role_style(
    role: PaletteRoleName,
    palette: &crate::pet::palette::ResolvedPalette,
) -> Style {
    let rgb = crate::pet::palette::role_color(role, palette);
    let mut style = Style::default().fg(Color::Rgb(rgb.r, rgb.g, rgb.b));
    if matches!(role, PaletteRoleName::Eye) {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// Overlays a per-pet `ResolvedPalette` onto the pet roles of `base`, keeping
/// every role's modifiers (e.g. the bold eye). The live dim/lift/shimmer chain
/// then mutates these seeded colors, so role-tagged glyphs and body-gap fills
/// both track per-pet color and brightness coherently.
fn seed_pet_palette(
    base: &SemanticStyles,
    palette: &crate::pet::palette::ResolvedPalette,
) -> SemanticStyles {
    let with_rgb =
        |style: Style, rgb: crate::pet::palette::Rgb| style.fg(Color::Rgb(rgb.r, rgb.g, rgb.b));
    let mut s = base.clone();
    s.pet_body = with_rgb(s.pet_body, palette.body);
    s.pet_eye = with_rgb(s.pet_eye, palette.eye);
    s.pet_mouth = with_rgb(s.pet_mouth, palette.mouth);
    s.pet_accent = with_rgb(s.pet_accent, palette.accent);
    s.pet_pattern = with_rgb(s.pet_pattern, palette.pattern);
    s
}

/// Snapshot the per-role foreground colors of the live `SemanticStyles` into a
/// `ResolvedPalette`. The watch passes the dim/lift/shimmer-mutated `live_styles`
/// here so the role-colored glyphs track exactly the same lightness changes as
/// the body-gap fills (`styles.pet_body`), keeping the pet internally coherent.
/// Non-RGB foregrounds (none occur on the pet roles today) fall back to the
/// default theme color for that role.
fn palette_from_styles(styles: &SemanticStyles) -> crate::pet::palette::ResolvedPalette {
    let default = crate::pet::palette::default_theme_palette();
    let rgb = |style: Style, fallback: crate::pet::palette::Rgb| match style.fg {
        Some(Color::Rgb(r, g, b)) => crate::pet::palette::Rgb::new(r, g, b),
        _ => fallback,
    };
    crate::pet::palette::ResolvedPalette {
        body: rgb(styles.pet_body, default.body),
        eye: rgb(styles.pet_eye, default.eye),
        mouth: rgb(styles.pet_mouth, default.mouth),
        accent: rgb(styles.pet_accent, default.accent),
        pattern: rgb(styles.pet_pattern, default.pattern),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use time::macros::datetime;
    use ColorCapability;

    #[test]
    fn floor_palette_is_biome_keyed() {
        use crate::tui::room::RoomBiomeTag;
        let botanical = biome_floor_palette(RoomBiomeTag::Botanical);
        let technical = biome_floor_palette(RoomBiomeTag::Technical);
        let artifact = biome_floor_palette(RoomBiomeTag::Artifact);
        assert_ne!(botanical, technical);
        assert_ne!(technical, artifact);
        assert_ne!(botanical, artifact);
    }

    #[test]
    fn biome_wash_is_subtle_and_biome_distinct() {
        use crate::tui::room::RoomBiomeTag;
        use ratatui::style::Color;
        let base = crate::tui::style::tokenpet_palette().bg.rgb;
        let Color::Rgb(br, bg_, bb) = base else {
            panic!("bg is rgb")
        };
        let bot = biome_wash_color(RoomBiomeTag::Botanical);
        let tech = biome_wash_color(RoomBiomeTag::Technical);
        assert_ne!(bot, tech, "biomes must wash differently");
        // Subtle: each channel within 24 of the base theme bg.
        if let Color::Rgb(r, g, b) = bot {
            assert!((r as i16 - br as i16).abs() <= 24);
            assert!((g as i16 - bg_ as i16).abs() <= 24);
            assert!((b as i16 - bb as i16).abs() <= 24);
        } else {
            panic!("wash must be rgb");
        }
    }

    fn test_context() -> RenderContext {
        use crate::tui::render_context::WatchClock;
        // Fixed clock so wander position is deterministic across test runs.
        RenderContext::with_clock(
            ColorCapability::Truecolor,
            WatchClock::fixed(time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap()),
        )
    }

    fn vm_with_real_pet() -> WatchViewModel {
        use crate::game::evolution::Stage;
        use crate::game::metabolism::Mood;
        use crate::pet::generation::generate_pet;
        use crate::pet::render::{render_pet, AnimationFrame};

        let pet = generate_pet("pet-panel-test-seed");
        let rendered = render_pet(
            &pet,
            Stage::S2,
            Mood::Content,
            AnimationFrame {
                tick: 0,
                blink_suppression_ticks: 0,
                hold_eyes_closed: false,
                blink_slowdown: 0,
                soft_eyes: false,
                work_accent: crate::pet::render::WorkAccent::None,
            },
        );
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    #[test]
    fn posture_offset_settles_tired_cozy_asleep_one_row() {
        use crate::tui::room::PetPerformance::*;
        assert_eq!(performance_posture_offset(RestedAwake), 0);
        assert_eq!(performance_posture_offset(SourceBurstPerk), 0);
        assert_eq!(performance_posture_offset(TiredAwake), 1);
        assert_eq!(performance_posture_offset(HeavyDayCozy), 1);
        assert_eq!(performance_posture_offset(AsleepDreaming), 1);
    }

    #[test]
    fn pet_role_style_uses_resolved_palette_with_bold_eye() {
        use crate::pet::palette::{default_theme_palette, Rgb};
        use crate::pet::render::PaletteRoleName;
        let p = default_theme_palette();
        let eye = pet_role_style(PaletteRoleName::Eye, &p);
        assert_eq!(eye.fg, Some(ratatui::style::Color::Rgb(0x82, 0xbc, 0x83)));
        assert!(eye.add_modifier.contains(ratatui::style::Modifier::BOLD));
        let body = pet_role_style(PaletteRoleName::Body, &p);
        assert_eq!(body.fg, Some(ratatui::style::Color::Rgb(0xef, 0xeb, 0xe4)));
        assert!(!body.add_modifier.contains(ratatui::style::Modifier::BOLD));
        let _ = Rgb::new(0, 0, 0); // keep import used
    }

    /// Foreground colors of every non-blank glyph span in the rendered pet
    /// lines, paired with whether the span carries the eye signature (BOLD +
    /// green-dominant base). Used to assert that role glyphs honor the live
    /// (dimmed/lifted) styles, not a frozen default palette.
    fn glyph_fg_colors(lines: &[Line<'static>]) -> Vec<Color> {
        lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .filter(|span| span.content.chars().any(|c| c != ' '))
            .filter_map(|span| span.style.fg)
            .collect()
    }

    #[test]
    fn build_pet_lines_role_glyphs_dim_with_live_styles() {
        // Sleep/low-energy darkens the whole pet via darken_pet_styles. The
        // role-colored glyphs (eyes/mouth/accent/pattern/body) must dim with
        // the body-gap fills, not stay frozen at the default theme color.
        let vm = vm_with_real_pet();
        let base = semantic_styles();
        let dimmed = darken_pet_styles(&base, 0.6);

        let bright = build_pet_lines(&vm, 13, &base, None, None);
        let dark = build_pet_lines(&vm, 13, &dimmed, None, None);

        let bright_fgs = glyph_fg_colors(&bright);
        let dark_fgs = glyph_fg_colors(&dark);
        assert_eq!(
            bright_fgs.len(),
            dark_fgs.len(),
            "dimming must not change which cells render"
        );
        assert!(!bright_fgs.is_empty(), "pet should render glyph spans");
        assert_ne!(
            bright_fgs, dark_fgs,
            "role glyphs must dim with the live styles, not stay at the default theme"
        );
        for (bright_fg, dark_fg) in bright_fgs.iter().zip(dark_fgs.iter()) {
            if let (Color::Rgb(br, bg, bb), Color::Rgb(dr, dg, db)) = (bright_fg, dark_fg) {
                assert!(
                    dr <= br && dg <= bg && db <= bb,
                    "each glyph channel must be no brighter when dimmed: \
                     {bright_fg:?} -> {dark_fg:?}"
                );
            }
        }
    }

    #[test]
    fn build_pet_lines_role_glyphs_match_unmutated_default_theme() {
        // With unmutated styles the watch pet must remain byte-identical to the
        // fixed default theme: this is the byte-identity guarantee of Task 5.
        let vm = vm_with_real_pet();
        let lines = build_pet_lines(&vm, 13, &semantic_styles(), None, None);
        let default_palette = crate::pet::palette::default_theme_palette();
        let expected_eye = {
            let rgb = crate::pet::palette::role_color(PaletteRoleName::Eye, &default_palette);
            Color::Rgb(rgb.r, rgb.g, rgb.b)
        };
        let expected_body = {
            let rgb = crate::pet::palette::role_color(PaletteRoleName::Body, &default_palette);
            Color::Rgb(rgb.r, rgb.g, rgb.b)
        };
        let fgs = glyph_fg_colors(&lines);
        assert!(
            fgs.contains(&expected_eye),
            "expected the green eye signature {expected_eye:?} in {fgs:?}"
        );
        assert!(
            fgs.contains(&expected_body),
            "expected the cream body color {expected_body:?} in {fgs:?}"
        );
    }

    #[test]
    fn pet_panel_renders_some_braille_into_area() {
        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        // Authored templates use block characters (█ ▌ ▐ ▀ ▄ ░ ▒ ▓) and
        // ASCII glyphs, not braille. Any non-space, non-newline char counts
        // as rendered pet content.
        let printable_count = s.chars().filter(|c| !c.is_whitespace()).count();
        assert!(
            printable_count > 5,
            "pet panel should render visible pet content into the area; got {printable_count} non-blank chars"
        );
    }

    #[test]
    fn pet_panel_centers_narrow_art_in_wide_area() {
        // With pet movement, the exact column of the pet art depends on the
        // clock. The invariant that still holds: the 13-wide pet art rect is
        // positioned by pet_inner_rect_in_panel (not by line padding), so
        // pet content must appear somewhere inside the 80-wide panel and
        // the buffer must not be all-blank.
        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let ctx = test_context();
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let printable_count: usize = (0..10)
            .flat_map(|y: u16| (0..80u16).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].symbol() != " ")
            .count();
        assert!(
            printable_count > 5,
            "pet panel should render visible pet content into the 80-wide area; got {printable_count} non-blank chars"
        );
    }

    #[test]
    fn cursor_normalized_x_is_none_when_cursor_outside_area() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((100, 100));
        let area = Rect::new(0, 0, 40, 5);
        assert!(cursor_normalized_x_within(&vm, area).is_none());
    }

    #[test]
    fn cursor_normalized_x_maps_left_edge_to_negative_one() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((0, 0));
        let area = Rect::new(0, 0, 40, 5);
        let n = cursor_normalized_x_within(&vm, area).unwrap();
        assert!(n <= -0.95, "left edge should be ~-1.0, got {n}");
    }

    #[test]
    fn cursor_normalized_x_maps_right_edge_to_near_positive_one() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((39, 0));
        let area = Rect::new(0, 0, 40, 5);
        let n = cursor_normalized_x_within(&vm, area).unwrap();
        assert!(n > 0.9, "right edge should be ~+1.0, got {n}");
    }

    #[test]
    fn cursor_normalized_x_disabled_when_tracking_off() {
        let mut vm = vm_with_real_pet();
        vm.cursor_screen = Some((20, 2));
        vm.mouse_tracking_enabled = false;
        let area = Rect::new(0, 0, 40, 5);
        assert!(cursor_normalized_x_within(&vm, area).is_none());
    }

    #[test]
    fn cursor_eyes_are_disabled_while_asleep() {
        let mut vm = WatchViewModel::fixture();
        vm.mouse_tracking_enabled = true;
        vm.cursor_screen = Some((5, 5));
        vm.day_context.asleep = true;
        let area = Rect::new(0, 0, 20, 10);
        assert_eq!(
            cursor_normalized_x_within(&vm, area),
            None,
            "closed eyes must not pop open to follow the mouse"
        );
    }

    #[test]
    fn cursor_eye_glyph_picks_directional_chars() {
        assert_eq!(cursor_eye_glyph(-0.9), '<');
        assert_eq!(cursor_eye_glyph(0.0), 'o');
        assert_eq!(cursor_eye_glyph(0.9), '>');
    }

    #[test]
    fn build_cursor_eye_string_preserves_span_width() {
        // Width 3 ("o o" style): glyph at both ends, space in between.
        assert_eq!(build_cursor_eye_string('<', 3), "< <");
        assert_eq!(build_cursor_eye_string('>', 3), "> >");
        // Width 5 (wider templates): glyph at both ends, more space.
        assert_eq!(build_cursor_eye_string('o', 5), "o   o");
        // Width 1 or 2 (rare): just the glyph.
        assert_eq!(build_cursor_eye_string('<', 1), "<");
        assert_eq!(build_cursor_eye_string('<', 2), "<");
        // Width 0: empty.
        assert_eq!(build_cursor_eye_string('<', 0), "");
    }

    #[test]
    fn pet_panel_swaps_eye_glyph_when_cursor_inside() {
        let mut vm = vm_with_real_pet();
        // Place cursor at right side; expect '>' glyph to appear in the
        // panel area. Eye glyph row depends on stage (S6 templates have
        // extra top decoration), so scan the full panel.
        // Cursor inside the pet area (after the SPEECH_ROWS=2 offset).
        vm.cursor_screen = Some((38, 4));
        let panel = PetPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                panel.render(f.area(), f.buffer_mut(), &vm, &ctx);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut all = String::new();
        for y in 0..10 {
            for x in 0..40 {
                all.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(
            all.contains('>'),
            "expected '>' eye glyph in pet panel, got {all:?}"
        );
    }

    #[test]
    fn pet_panel_preferred_constraint_is_fill() {
        let vm = WatchViewModel::fixture();
        let panel = PetPanel;
        assert_eq!(
            panel.preferred_constraint(&vm),
            Constraint::Fill(1),
            "pet panel absorbs vertical slack so habitat (PR2) can fill it"
        );
    }

    #[test]
    fn pet_panel_renders_pet_centered_in_tall_rect() {
        let vm = WatchViewModel::fixture();
        let panel = PetPanel;
        let ctx = test_context();
        let backend = TestBackend::new(40, 24); // taller than pet (10 rows)
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| panel.render(f.area(), f.buffer_mut(), &vm, &ctx))
            .unwrap();
        let buf = terminal.backend().buffer();
        let s: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        // The fixture pet_art contains "( o.o )" — 'o' and '.' will be present.
        assert!(
            s.contains('o') || s.contains('.') || s.contains('^'),
            "pet must render visibly in a tall panel rect; got content: {s:?}"
        );
    }

    #[test]
    fn activity_glyph_budget_caps_compact_hot_state() {
        let profile = PetLifeProfile {
            activity_level: 2.0,
            burst_level: 1.5,
            ..Default::default()
        };

        assert_eq!(activity_glyph_budget(&profile, true), 3);
        assert_eq!(activity_glyph_budget(&profile, false), 10);
    }

    #[test]
    fn activity_glyph_budget_suppresses_calm_mode() {
        let profile = PetLifeProfile {
            activity_level: 2.0,
            burst_level: 1.5,
            calm_mode: true,
            ..Default::default()
        };

        assert_eq!(activity_glyph_budget(&profile, true), 0);
        assert_eq!(activity_glyph_budget(&profile, false), 0);
    }

    #[test]
    fn activity_style_lift_is_clamped_and_flat_safe() {
        let original =
            ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(100, 100, 100));
        let style = activity_lift_style(original, 2.0, ColorCapability::Truecolor);
        assert_ne!(style, original);

        let clamped = activity_lift_style(
            ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(250, 251, 252)),
            2.0,
            ColorCapability::Truecolor,
        );
        assert_eq!(clamped.fg, Some(ratatui::style::Color::Rgb(255, 255, 255)));

        let flat = activity_lift_style(original, 2.0, ColorCapability::Flat);
        assert_eq!(flat, original);
    }

    #[test]
    fn prop_reaction_style_lifts_rgb_and_preserves_flat() {
        let original = Style::default().fg(Color::Rgb(100, 110, 120));
        let reaction = PropReaction {
            prop_id: crate::storage::state::HabitatPropId::new(
                crate::game::habitat::CODEX_SIGNAL_LAMP,
            ),
            intensity: 0.5,
            kind: PropReactionKind::Glow,
        };

        let lifted =
            apply_prop_reaction_style(original, Some(&reaction), ColorCapability::Truecolor);
        assert_eq!(lifted.fg, Some(Color::Rgb(117, 127, 137)));

        let flat = apply_prop_reaction_style(original, Some(&reaction), ColorCapability::Flat);
        assert_eq!(flat, original);
    }

    #[test]
    fn activity_glyph_color_uses_source_accent_when_available() {
        let claude = activity_glyph_color(&PetLifeProfile {
            source_accent: Some(SourceAccent::Claude),
            ..Default::default()
        });
        let codex = activity_glyph_color(&PetLifeProfile {
            source_accent: Some(SourceAccent::Codex),
            ..Default::default()
        });

        assert_ne!(claude, codex);
        assert_eq!(claude, Color::Rgb(0xb3, 0x9d, 0xff));
        assert_eq!(codex, Color::Rgb(0x86, 0xd9, 0xef));
    }

    #[test]
    fn activity_glyph_color_keeps_weather_visible_with_source_accent() {
        let cache_claude = activity_glyph_color(&PetLifeProfile {
            source_accent: Some(SourceAccent::Claude),
            work_weather: WorkWeather::CacheMist,
            ..Default::default()
        });
        let output_claude = activity_glyph_color(&PetLifeProfile {
            source_accent: Some(SourceAccent::Claude),
            work_weather: WorkWeather::OutputSparks,
            ..Default::default()
        });

        assert_ne!(cache_claude, output_claude);
    }

    #[test]
    fn token_pop_requires_current_profile_burst() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        let pulse = now - time::Duration::seconds(1);

        assert!(profile_token_pop(
            Some(pulse),
            &PetLifeProfile {
                burst_level: 0.6,
                ..Default::default()
            },
            ColorCapability::Truecolor,
            now,
        )
        .is_some());
        assert!(profile_token_pop(
            Some(pulse),
            &PetLifeProfile {
                burst_level: 0.0,
                ..Default::default()
            },
            ColorCapability::Truecolor,
            now,
        )
        .is_none());
        assert!(profile_token_pop(
            Some(pulse),
            &PetLifeProfile {
                burst_level: 0.6,
                ..Default::default()
            },
            ColorCapability::Flat,
            now,
        )
        .is_none());
    }

    #[test]
    fn activity_glyphs_are_suppressed_for_flat_color() {
        let profile = PetLifeProfile {
            activity_level: 2.0,
            work_weather: WorkWeather::OutputSparks,
            ..Default::default()
        };
        let glyphs = activity_glyphs_for(
            &profile,
            Species::Crystal,
            Rect::new(0, 0, 40, 12),
            &[],
            time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            ColorCapability::Flat,
            10,
        );

        assert!(glyphs.is_empty());
    }

    #[test]
    fn ambient_glyphs_are_deterministic_per_minute() {
        use crate::game::evolution::Stage;
        let habitat = Rect::new(0, 0, 52, 20);
        let pet_inner = Rect::new(20, 6, 13, 10);
        let exclusions = vec![pet_inner];

        let t0 = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let t_same_minute = t0 + time::Duration::seconds(15);
        let t_next_minute = t0 + time::Duration::minutes(1);

        let a = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            habitat,
            &exclusions,
            t0,
            ColorCapability::Truecolor,
        );
        let b = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            habitat,
            &exclusions,
            t_same_minute,
            ColorCapability::Truecolor,
        );
        let c = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            habitat,
            &exclusions,
            t_next_minute,
            ColorCapability::Truecolor,
        );

        assert_eq!(a, b, "same minute should yield identical glyphs");
        assert_ne!(a, c, "next minute should yield different glyphs");
    }

    #[test]
    fn ambient_glyphs_never_overlap_exclusions() {
        use crate::game::evolution::Stage;
        let habitat = Rect::new(0, 0, 52, 20);
        let pet_inner = Rect::new(20, 6, 13, 10);
        let exclusions = vec![pet_inner];
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        for species in [
            Species::Fuzz,
            Species::Blob,
            Species::Ghost,
            Species::Glitch,
            Species::Crystal,
            Species::Mech,
        ] {
            for stage in [Stage::S0, Stage::S2, Stage::S4, Stage::S6] {
                let glyphs = ambient_glyphs_for(
                    species,
                    stage,
                    habitat,
                    &exclusions,
                    now,
                    ColorCapability::Truecolor,
                );
                for g in &glyphs {
                    let in_exclusion = g.col >= pet_inner.x
                        && g.col < pet_inner.x + pet_inner.width
                        && g.row >= pet_inner.y
                        && g.row < pet_inner.y + pet_inner.height;
                    assert!(
                        !in_exclusion,
                        "species {species:?} stage {stage:?} glyph at ({},{}) is inside exclusion {pet_inner:?}",
                        g.col, g.row
                    );
                }
            }
        }
    }

    #[test]
    fn ambient_glyphs_within_habitat_bounds() {
        use crate::game::evolution::Stage;
        let habitat = Rect::new(5, 10, 52, 20);
        let pet_inner = Rect::new(25, 16, 13, 10);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let glyphs = ambient_glyphs_for(
            Species::Crystal,
            Stage::S5,
            habitat,
            &[pet_inner],
            now,
            ColorCapability::Truecolor,
        );
        for g in glyphs {
            assert!(
                g.col >= habitat.x && g.col < habitat.x + habitat.width,
                "col {} outside habitat",
                g.col
            );
            assert!(
                g.row >= habitat.y && g.row < habitat.y + habitat.height,
                "row {} outside habitat",
                g.row
            );
        }
    }

    #[test]
    fn ambient_glyphs_present_with_floor_row() {
        use crate::game::evolution::Stage;
        let habitat = Rect::new(0, 0, 52, 20);
        let pet_inner = Rect::new(20, 6, 13, 10);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let glyphs = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            habitat,
            &[pet_inner],
            now,
            ColorCapability::Truecolor,
        );
        // 8 sky glyphs (S4) + 52-cell floor minus the exclusion overlap (none, since pet is mid-panel).
        assert!(
            glyphs.len() >= 8 + 30,
            "expected ≥ stage_base + most of the floor row, got {}",
            glyphs.len()
        );
    }

    #[test]
    fn glitch_and_crystal_ambient_floor_use_distinct_symbol_families() {
        let habitat = Rect::new(0, 0, 80, 20);
        let now = datetime!(2026-06-11 10:00 UTC);
        let glitch = ambient_glyphs_for_phase(
            Species::Glitch,
            Stage::S6,
            crate::tui::room::RoomBiomeTag::Starter,
            habitat,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
            1.0,
            0,
            Season::Summer,
            None,
        );
        let crystal = ambient_glyphs_for_phase(
            Species::Crystal,
            Stage::S6,
            crate::tui::room::RoomBiomeTag::Starter,
            habitat,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
            1.0,
            0,
            Season::Summer,
            None,
        );
        let glitch_symbols = glitch
            .iter()
            .map(|g| g.glyph)
            .collect::<std::collections::HashSet<_>>();
        let crystal_symbols = crystal
            .iter()
            .map(|g| g.glyph)
            .collect::<std::collections::HashSet<_>>();

        assert!(glitch_symbols
            .iter()
            .any(|c| ['#', ':', ';', '_', '░', '▒', '▪'].contains(c)));
        assert!(crystal_symbols
            .iter()
            .any(|c| ['◇', '◆', '✦', '✧', '·'].contains(c)));
        assert_ne!(glitch_symbols, crystal_symbols);
    }

    #[test]
    fn ambient_glyph_must_be_fully_inside_panel_area() {
        let area = Rect::new(10, 20, 5, 4);

        assert!(ambient_glyph_is_inside_area(
            &AmbientGlyph {
                row: 20,
                col: 10,
                glyph: '*',
                color: Color::White,
            },
            area
        ));
        assert!(!ambient_glyph_is_inside_area(
            &AmbientGlyph {
                row: 19,
                col: 10,
                glyph: '*',
                color: Color::White,
            },
            area
        ));
        assert!(!ambient_glyph_is_inside_area(
            &AmbientGlyph {
                row: 20,
                col: 9,
                glyph: '*',
                color: Color::White,
            },
            area
        ));
        assert!(!ambient_glyph_is_inside_area(
            &AmbientGlyph {
                row: 24,
                col: 10,
                glyph: '*',
                color: Color::White,
            },
            area
        ));
        assert!(!ambient_glyph_is_inside_area(
            &AmbientGlyph {
                row: 20,
                col: 15,
                glyph: '*',
                color: Color::White,
            },
            area
        ));
    }

    #[test]
    fn ambient_glyphs_handle_one_row_habitat_without_panic() {
        use crate::game::evolution::Stage;
        // Height = 1 means there's no row above the floor; the painter must not
        // panic on `rng.gen_range(0..0)`. Returning empty is the contracted behavior.
        let habitat = Rect::new(0, 0, 52, 1);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let glyphs = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            habitat,
            &[],
            now,
            ColorCapability::Truecolor,
        );
        assert!(
            glyphs.is_empty(),
            "habitat too short for both sky and floor — painter should return empty, got {} glyphs",
            glyphs.len()
        );
    }

    #[test]
    fn ambient_glyph_count_scales_with_habitat_area() {
        use crate::game::evolution::Stage;
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        // Normal-wide pet panel: ~52 × 14 = 728 cells, well above the 200 threshold.
        let normal = Rect::new(0, 0, 52, 14);
        let normal_glyphs = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            normal,
            &[],
            now,
            ColorCapability::Truecolor,
        );

        // Tall-wide pet panel: ~52 × 35 = 1820 cells.
        let tall = Rect::new(0, 0, 52, 35);
        let tall_glyphs = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            tall,
            &[],
            now,
            ColorCapability::Truecolor,
        );

        let normal_sky_count = normal_glyphs
            .iter()
            .filter(|g| g.row < normal.height - 1)
            .count();
        let tall_sky_count = tall_glyphs
            .iter()
            .filter(|g| g.row < tall.height - 1)
            .count();

        // Normal: 8 (S4 base) + (728 - 200) / 60 = 8 + 8 = 16.
        // Tall: 8 + (1820 - 200) / 60 = 8 + 27 = 35.
        assert!(
            tall_sky_count > normal_sky_count + 10,
            "tall habitat should produce noticeably more sky glyphs; normal={normal_sky_count} tall={tall_sky_count}"
        );
    }

    #[test]
    fn ambient_glyphs_empty_on_flat_color() {
        use crate::game::evolution::Stage;
        let habitat = Rect::new(0, 0, 52, 20);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let glyphs = ambient_glyphs_for(
            Species::Fuzz,
            Stage::S4,
            habitat,
            &[],
            now,
            ColorCapability::Flat,
        );
        assert!(
            glyphs.is_empty(),
            "Flat color should disable habitat (dim-without-color is just noise)"
        );
    }

    #[test]
    fn pet_silhouette_halo_covers_glyph_cells_and_eight_neighbors() {
        // Synthetic 3×3 art with a single 'X' in the center. Halo must include
        // the X itself plus all 8 surrounding cells of the absolute pet_rect.
        let lines = vec!["   ".to_string(), " X ".to_string(), "   ".to_string()];
        let pet_rect = Rect::new(10, 20, 3, 3);
        let rects = pet_silhouette_halo_rects(&lines, pet_rect, false);
        let cells: std::collections::HashSet<(u16, u16)> =
            rects.iter().map(|r| (r.x, r.y)).collect();
        let expected: std::collections::HashSet<(u16, u16)> = (10u16..=12)
            .flat_map(|x| (20u16..=22).map(move |y| (x, y)))
            .collect();
        assert_eq!(cells, expected);
    }

    #[test]
    fn pet_silhouette_halo_skips_pure_whitespace_rows() {
        // An entirely-blank row contributes no halo cells anywhere.
        let lines = vec!["   ".to_string(), "   ".to_string(), "   ".to_string()];
        let pet_rect = Rect::new(0, 0, 3, 3);
        let rects = pet_silhouette_halo_rects(&lines, pet_rect, false);
        assert!(rects.is_empty(), "blank art must produce no halo cells");
    }

    #[test]
    fn pet_silhouette_halo_mirrors_columns_when_facing_left() {
        // Single 'X' at column 0 of a 3-wide line. With mirror=true, the
        // glyph's effective column flips to width-1-col_idx = 2.
        let lines = vec!["X  ".to_string()];
        let pet_rect = Rect::new(100, 50, 3, 1);
        let rects = pet_silhouette_halo_rects(&lines, pet_rect, true);
        let cells: std::collections::HashSet<(u16, u16)> =
            rects.iter().map(|r| (r.x, r.y)).collect();
        // Mirrored X lands at absolute (102, 50). Halo: (101..=103, 49..=51).
        // Row 49 underflows when pet_rect.y=50 and dy=-1 — fine, included.
        assert!(cells.contains(&(102, 50)), "mirrored glyph cell missing");
        assert!(cells.contains(&(101, 50)), "left halo missing");
        assert!(cells.contains(&(103, 50)), "right halo missing");
        assert!(
            !cells.contains(&(100, 50)),
            "non-mirrored origin must stay free"
        );
    }

    #[test]
    fn pet_silhouette_halo_for_real_pet_is_smaller_than_inflated_rect() {
        // The previous behavior excluded a 15×12 inflated rect = 180 cells.
        // Silhouette+halo for a real pet should cover the diamond (~50 cells)
        // plus its halo (~50–80 cells), well under 180 — leaving room for
        // habitat backdrop glyphs in the rect's negative space.
        let vm = vm_with_real_pet();
        let pet_rect = Rect::new(10, 20, 13, 10);
        let rects = pet_silhouette_halo_rects(&vm.pet_art, pet_rect, false);
        assert!(
            rects.len() < 180,
            "silhouette+halo ({}) must be smaller than 15×12 inflated rect (180)",
            rects.len()
        );
        assert!(
            rects.len() > 20,
            "silhouette+halo ({}) should still cover the diamond and its margin",
            rects.len()
        );
    }

    #[test]
    fn pet_panel_lets_background_show_through_empty_silhouette_cells() {
        use crate::tui::render_context::WatchClock;
        // Fill the entire 13×10 pet bounding rect with a marker char before
        // rendering. With sparse silhouette rendering, the pet pass must only
        // overwrite cells that contain a non-space glyph — so markers placed
        // in the diamond's negative space (corners, frame padding rows) must
        // survive intact.
        //
        // Flat color disables ambient_glyphs and accents; the fixture vm has
        // no trophies, so the pet pass is the only writer to the buffer.
        let vm = vm_with_real_pet();
        let panel = PetPanel;
        let ctx = RenderContext::with_clock(
            ColorCapability::Flat,
            WatchClock::fixed(time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap()),
        );
        let backend = TestBackend::new(13, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let marker = '\u{2605}'; // ★

        terminal
            .draw(|f| {
                let area = f.area();
                let buf = f.buffer_mut();
                for y in 0..10u16 {
                    for x in 0..13u16 {
                        buf[(x, y)].set_char(marker);
                    }
                }
                panel.render(area, buf, &vm, &ctx);
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let preserved: usize = (0..10u16)
            .flat_map(|y| (0..13u16).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(x, y)].symbol() == "\u{2605}")
            .count();
        assert!(
            preserved >= 30,
            "pet's empty silhouette cells must preserve background; only {preserved} / 130 markers survived (expected ≥ 30 — the diamond's negative space)"
        );
    }

    #[test]
    fn pet_inner_rect_in_panel_does_not_panic_when_area_is_smaller_than_pet() {
        // Regression: when the layout allocates an area smaller than PET_W/PET_H
        // (e.g. compact mode where Fill collapses to 0 height), the previous
        // implementation's i32::clamp had min > max and panicked. The helper
        // must return a degenerate Rect cleanly.
        let vm = WatchViewModel::fixture();
        // 0×0 area (extreme — Fill collapsed entirely).
        let _ = pet_inner_rect_in_panel(Rect::new(0, 0, 0, 0), &vm);
        // Area narrower than PET_W.
        let _ = pet_inner_rect_in_panel(Rect::new(2, 2, 5, 5), &vm);
        // Area shorter than PET_H (the actual compact crash scenario).
        let _ = pet_inner_rect_in_panel(Rect::new(0, 0, 40, 3), &vm);
        // Offset rect that previously made max < min on the y axis.
        let _ = pet_inner_rect_in_panel(Rect::new(0, 5, 40, 3), &vm);
    }

    #[test]
    fn night_sky_uses_the_night_family_and_a_smaller_budget() {
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let day = ambient_glyphs_for_phase(
            Species::Crystal,
            Stage::S6,
            crate::tui::room::RoomBiomeTag::Starter,
            habitat,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Day,
            1.0,
            0,
            Season::Summer,
            None,
        );
        let night = ambient_glyphs_for_phase(
            Species::Crystal,
            Stage::S6,
            crate::tui::room::RoomBiomeTag::Starter,
            habitat,
            &[],
            now,
            ColorCapability::Truecolor,
            DayPhase::Night,
            1.0,
            0,
            Season::Summer,
            None,
        );
        // Night never adds: sky glyph count (excluding the floor row) must be
        // <= day's. Floor-row glyphs share a row coordinate — partition on it.
        let floor_row = habitat.y + habitat.height - 1;
        let day_sky = day.iter().filter(|g| g.row != floor_row).count();
        let night_sky = night.iter().filter(|g| g.row != floor_row).count();
        assert!(night_sky <= day_sky, "night {night_sky} > day {day_sky}");
        assert!(night_sky > 0, "the starfield exists");
        // And the night family differs from the day family for this species.
        let night_chars: std::collections::HashSet<char> = night
            .iter()
            .filter(|g| g.row != floor_row)
            .map(|g| g.glyph)
            .collect();
        assert!(
            night_chars
                .iter()
                .any(|c| !sky_palette_for(Species::Crystal).contains(c))
                || night.iter().filter(|g| g.row != floor_row).count() < day_sky,
            "night must read differently than day"
        );
    }

    #[test]
    fn flat_tier_still_renders_zero_ambient_glyphs_at_night() {
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let glyphs = ambient_glyphs_for_phase(
            Species::Crystal,
            Stage::S6,
            crate::tui::room::RoomBiomeTag::Starter,
            habitat,
            &[],
            now,
            ColorCapability::Flat,
            DayPhase::Night,
            1.0,
            0,
            Season::Summer,
            None,
        );
        assert!(
            glyphs.is_empty(),
            "Flat keeps the existing zero-ambient contract"
        );
    }

    #[test]
    fn phase_blend_interpolates_the_sky_color() {
        let p = crate::tui::style::tokenpet_palette();
        let base = p.dim.rgb;
        for phase in [
            DayPhase::Dawn,
            DayPhase::Day,
            DayPhase::Dusk,
            DayPhase::Night,
        ] {
            let c0 = sky_color_for_phase(phase, 0.0, Season::Summer, None);
            let c1 = sky_color_for_phase(phase, 1.0, Season::Summer, None);
            let mid = sky_color_for_phase(phase, 0.5, Season::Summer, None);
            assert_eq!(c0, base, "boundary starts from neutral base for {phase:?}");
            if phase == DayPhase::Day {
                assert_eq!(c1, base, "Day stays at the neutral base");
                assert_eq!(mid, base, "Day midpoint is also base");
            } else {
                assert_ne!(c1, c0, "settled color differs from base for {phase:?}");
                assert_ne!(mid, c0, "midpoint is not base for {phase:?}");
                assert_ne!(mid, c1, "midpoint is not settled for {phase:?}");
            }
        }
    }

    #[test]
    fn mote_density_soft_saturates_with_no_learnable_full_state() {
        let step01 = mote_density(1.0) - mote_density(0.0);
        let step24 = mote_density(4.0) - mote_density(2.0);
        assert!(
            step24 < step01,
            "saturating: step24 {step24} must be < step01 {step01}"
        );
        assert!(mote_density(4.0) > mote_density(2.0), "still rising");
        assert!(mote_density(10.0) < 1.0, "asymptotic, never full");
        assert_eq!(mote_density(0.0), 0.0, "no work, no motes");
    }

    #[test]
    fn motes_cap_at_the_budget_share_of_the_ambient_allocation() {
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let day = crate::tui::day::DayContext {
            mature: true,
            today_ratio: 100.0,
            ..crate::tui::day::DayContext::default()
        };
        let motes = mote_glyphs_for(&day, habitat, &[], now, ColorCapability::Truecolor);
        assert!(!motes.is_empty(), "a heavy day shows motes");
        assert!(motes.len() <= 4, "cap is half the stage-floor allocation");
        let floor_row = habitat.y + habitat.height - 1; // 11
        for g in &motes {
            assert!(g.row < floor_row, "motes never overwrite the floor row");
            assert!(g.row >= 7, "motes stay in the lower band");
        }
        let blocked = mote_glyphs_for(&day, habitat, &[habitat], now, ColorCapability::Truecolor);
        assert!(blocked.is_empty(), "fully excluded habitat places nothing");
    }

    #[test]
    fn mote_tidy_fade_thins_yesterdays_motes_after_rollover() {
        let habitat = Rect::new(0, 0, 60, 15);
        let day_start = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let day = crate::tui::day::DayContext {
            mature: true,
            today_ratio: 0.0,
            yesterday: Some(crate::tui::day::DaySummary {
                ratio: 3.0,
                dominant_shape: None,
            }),
            local_day_started_utc: day_start,
            date_seed: 7,
            ..crate::tui::day::DayContext::default()
        };
        let at = |minutes: i64| {
            mote_glyphs_for(
                &day,
                habitat,
                &[],
                day_start + time::Duration::minutes(minutes),
                ColorCapability::Truecolor,
            )
        };
        let t0 = at(0);
        let t15 = at(15);
        let t30 = at(30);
        assert!(!t0.is_empty(), "yesterday's motes are still in the room");
        assert!(!t15.is_empty(), "mid-window the fade is partial");
        assert!(
            t15.len() < t0.len(),
            "fade is monotonic: {} -> {}",
            t0.len(),
            t15.len()
        );
        assert!(t30.is_empty(), "tidy fade completes at the window edge");
        for g in &t15 {
            assert!(
                t0.contains(g),
                "fade removes from the end, never reshuffles"
            );
        }
    }

    #[test]
    fn flat_and_immature_pets_render_zero_motes() {
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mature = crate::tui::day::DayContext {
            mature: true,
            today_ratio: 5.0,
            ..crate::tui::day::DayContext::default()
        };
        assert!(
            mote_glyphs_for(&mature, habitat, &[], now, ColorCapability::Flat).is_empty(),
            "Flat keeps the zero-ambient contract"
        );
        let immature = crate::tui::day::DayContext {
            mature: false,
            today_ratio: 5.0,
            ..crate::tui::day::DayContext::default()
        };
        assert!(
            mote_glyphs_for(&immature, habitat, &[], now, ColorCapability::Truecolor).is_empty(),
            "the default 100k baseline must not render a fabricated feast"
        );
    }

    #[test]
    fn sky_family_is_stable_for_a_seed_and_authors_two_variants_per_phase() {
        let all_species = [
            Species::Fuzz,
            Species::Blob,
            Species::Ghost,
            Species::Glitch,
            Species::Crystal,
            Species::Mech,
        ];
        let phases = [
            DayPhase::Dawn,
            DayPhase::Day,
            DayPhase::Dusk,
            DayPhase::Night,
        ];
        for species in all_species {
            for phase in phases {
                assert_eq!(
                    sky_palette_for_phase(species, phase, 9),
                    sky_palette_for_phase(species, phase, 9),
                    "{species:?}/{phase:?} family must be a pure function of the seed"
                );
                assert_ne!(
                    sky_palette_for_phase(species, phase, 8),
                    sky_palette_for_phase(species, phase, 9),
                    "{species:?}/{phase:?} needs at least two authored variants"
                );
            }
        }
    }

    #[test]
    fn climate_clear_and_none_tint_nothing_and_a_real_climate_tints() {
        for phase in [
            DayPhase::Dawn,
            DayPhase::Day,
            DayPhase::Dusk,
            DayPhase::Night,
        ] {
            assert_eq!(
                sky_color_for_phase(phase, 1.0, Season::Summer, None),
                sky_color_for_phase(phase, 1.0, Season::Summer, Some(WorkWeather::Clear)),
                "Clear must render exactly like None for {phase:?}"
            );
        }
        assert_ne!(
            sky_color_for_phase(DayPhase::Day, 1.0, Season::Summer, None),
            sky_color_for_phase(
                DayPhase::Day,
                1.0,
                Season::Summer,
                Some(WorkWeather::CacheMist)
            ),
            "a real climate biases the ambient tint"
        );
    }

    #[test]
    fn season_drift_is_bounded_and_summer_is_the_neutral_reference() {
        let c = Color::Rgb(110, 110, 110);
        assert_eq!(season_hue_drift(c, Season::Summer), c);
        for season in [
            crate::tui::day::Season::Spring,
            crate::tui::day::Season::Autumn,
            crate::tui::day::Season::Winter,
        ] {
            let drifted = season_hue_drift(c, season);
            assert_ne!(drifted, c, "{season:?} must drift the hue");
            let Color::Rgb(r, g, b) = drifted else {
                panic!("rgb in, rgb out");
            };
            for channel in [r, g, b] {
                assert!(
                    (i16::from(channel) - 110).abs() <= i16::from(SEASON_DRIFT_MAX_CHANNEL_NUDGE),
                    "{season:?} drift must stay subtle (channel {channel})"
                );
            }
        }
        assert_eq!(season_hue_drift(Color::Reset, Season::Winter), Color::Reset);
    }

    #[test]
    fn live_activity_always_wins_over_weekend_softening() {
        let day = crate::tui::day::DayContext {
            is_weekend: true,
            mature: true,
            weekend_share: 0.05,
            ..crate::tui::day::DayContext::default()
        };
        let idle = PetLifeProfile::idle();
        assert!(
            (effective_weekend_softening(&day, &idle) - 1.0).abs() < 1e-6,
            "quiet weekend, idle pet: full softening"
        );
        let mut active = PetLifeProfile::idle();
        active.activity_level = 0.8;
        assert_eq!(
            effective_weekend_softening(&day, &active),
            0.0,
            "live activity suppresses softening entirely"
        );
        let mut bursting = PetLifeProfile::idle();
        bursting.burst_level = 0.4;
        assert_eq!(
            effective_weekend_softening(&day, &bursting),
            0.0,
            "a live burst suppresses softening entirely"
        );
    }

    #[test]
    fn weekend_softening_pulls_scene_colors_toward_the_dim_base() {
        let c = Color::Rgb(200, 120, 40);
        assert_eq!(weekend_soften_color(c, 0.0), c, "no softening, no change");
        assert_ne!(
            weekend_soften_color(c, 1.0),
            c,
            "full softening shifts the color"
        );
        assert_eq!(weekend_soften_color(Color::Reset, 1.0), Color::Reset);
    }

    #[test]
    fn resonance_adds_gentle_glow_when_prop_has_no_live_reaction() {
        let id =
            crate::storage::state::HabitatPropId::new(crate::game::habitat::HEAVY_SESSION_PLANTER);
        let styled = apply_resonance_reaction(PetLifeProfile::default(), Some(&id));
        assert_eq!(styled.prop_reactions.len(), 1);
        assert_eq!(styled.prop_reactions[0].prop_id, id);
        assert_eq!(styled.prop_reactions[0].kind, PropReactionKind::Glow);
        assert!(
            styled.prop_reactions[0].intensity > 0.0 && styled.prop_reactions[0].intensity <= 1.0
        );
    }

    #[test]
    fn resonance_never_overrides_a_live_reaction_for_the_same_prop() {
        let id =
            crate::storage::state::HabitatPropId::new(crate::game::habitat::HEAVY_SESSION_PLANTER);
        let profile = PetLifeProfile {
            prop_reactions: vec![PropReaction {
                prop_id: id.clone(),
                intensity: 0.72,
                kind: PropReactionKind::Bloom,
            }],
            ..PetLifeProfile::default()
        };
        let styled = apply_resonance_reaction(profile, Some(&id));
        assert_eq!(styled.prop_reactions.len(), 1);
        assert_eq!(styled.prop_reactions[0].intensity, 0.72);
        assert_eq!(styled.prop_reactions[0].kind, PropReactionKind::Bloom);
    }

    #[test]
    fn resonance_wander_bias_points_toward_the_prop_zone() {
        let planter =
            crate::storage::state::HabitatPropId::new(crate::game::habitat::HEAVY_SESSION_PLANTER);
        let sprout =
            crate::storage::state::HabitatPropId::new(crate::game::habitat::WILT_RECOVERY_SPROUT);
        assert!(
            resonance_wander_bias(Some(&planter)) > 0,
            "right-zone prop pulls right"
        );
        assert!(
            resonance_wander_bias(Some(&sprout)) < 0,
            "left-zone prop pulls left"
        );
        assert_eq!(resonance_wander_bias(None), 0, "no companion, no bias");
    }

    fn test_scene_with_pet_art(pet_art: Rect) -> PetSceneLayout {
        let area = Rect::new(0, 0, 80, 14);
        PetSceneLayout {
            id: crate::tui::component::WatchComponentId::Pet.path(),
            panel: area,
            speech: None,
            content: area,
            pet_art,
            hit_area: area,
            habitat: area,
            exclusions: Vec::new(),
            targets: std::collections::BTreeMap::new(),
            effect_targets: Vec::new(),
        }
    }

    #[test]
    fn performance_cue_places_floor_symbol_below_pet() {
        let pet_art = Rect::new(30, 3, 13, 10);
        let scene = test_scene_with_pet_art(pet_art);
        let mut buf = Buffer::empty(scene.habitat);

        apply_pet_performance_cues(
            &mut buf,
            &scene,
            crate::tui::room::PetPerformance::TiredAwake,
            ColorCapability::Truecolor,
        );

        let x = pet_art.x + pet_art.width / 2;
        let y = pet_art.y + pet_art.height;
        assert_eq!(buf[(x, y)].symbol(), "˙");
    }

    #[test]
    fn performance_cue_places_air_symbol_above_pet() {
        let pet_art = Rect::new(30, 3, 13, 10);
        let scene = test_scene_with_pet_art(pet_art);
        let mut buf = Buffer::empty(scene.habitat);

        apply_pet_performance_cues(
            &mut buf,
            &scene,
            crate::tui::room::PetPerformance::AsleepDreaming,
            ColorCapability::Truecolor,
        );

        let x = pet_art.x + pet_art.width / 2;
        let y = pet_art.y - 1;
        assert_eq!(buf[(x, y)].symbol(), "z");
    }

    #[test]
    fn performance_cue_works_in_flat_mode() {
        let pet_art = Rect::new(30, 3, 13, 10);
        let scene = test_scene_with_pet_art(pet_art);
        let mut buf = Buffer::empty(scene.habitat);

        apply_pet_performance_cues(
            &mut buf,
            &scene,
            crate::tui::room::PetPerformance::HeavyDayCozy,
            ColorCapability::Flat,
        );

        let x = pet_art.x + pet_art.width / 2;
        let y = pet_art.y + pet_art.height;
        assert_eq!(buf[(x, y)].symbol(), "~");
    }

    #[test]
    fn performance_cue_skips_air_mark_when_pet_touches_top_edge() {
        let pet_art = Rect::new(30, 0, 13, 10);
        let scene = test_scene_with_pet_art(pet_art);
        let mut buf = Buffer::empty(scene.habitat);

        // Should not panic on underflow and must not overwrite pet art.
        apply_pet_performance_cues(
            &mut buf,
            &scene,
            crate::tui::room::PetPerformance::CatchUpWake,
            ColorCapability::Truecolor,
        );

        let x = pet_art.x + pet_art.width / 2;
        let y = pet_art.y;
        assert_ne!(
            buf[(x, y)].symbol(),
            "^",
            "air mark should be skipped when there is no row above the pet"
        );
    }

    #[test]
    fn performance_cue_does_not_write_outside_habitat() {
        let pet_art = Rect::new(70, 3, 13, 10);
        let scene = test_scene_with_pet_art(pet_art);
        let mut buf = Buffer::empty(scene.habitat);

        apply_pet_performance_cues(
            &mut buf,
            &scene,
            crate::tui::room::PetPerformance::SourceBurstPerk,
            ColorCapability::Truecolor,
        );

        // No panic and no out-of-bounds write; the clipped center x is
        // habitat-limited, so the buffer is still valid.
        for y in 0..scene.habitat.height {
            for x in 0..scene.habitat.width {
                let _ = buf[(x, y)].symbol();
            }
        }
    }

    #[test]
    fn performance_lightness_baseline_dims_tired_and_asleep_below_rested() {
        let rested =
            performance_lightness_multiplier(crate::tui::room::PetPerformance::RestedAwake);
        let tired = performance_lightness_multiplier(crate::tui::room::PetPerformance::TiredAwake);
        let asleep =
            performance_lightness_multiplier(crate::tui::room::PetPerformance::AsleepDreaming);
        assert_eq!(rested, 1.0, "rested is the neutral baseline");
        assert!(tired < rested, "tired sits below rested");
        assert!(asleep < tired, "asleep is the dimmest");
        assert!(asleep > 0.5, "never fully dark");
    }
}
