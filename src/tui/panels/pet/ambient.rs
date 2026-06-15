use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::game::evolution::Stage;
use crate::pet::generation::Species;
use crate::tui::day::{DayContext, DayPhase, Season};
use crate::tui::life::{PetLifeProfile, WorkWeather};
use crate::tui::style::ColorCapability;

use super::{
    activity_glyph_color, dim_shift, lerp_color, warm_shift, AmbientGlyph, MOTE_BUDGET_SHARE,
    MOTE_GLYPHS,
};

/// Per-species sky-glyph palette.
pub(super) fn sky_palette_for(species: Species) -> &'static [char] {
    match species {
        Species::Fuzz => &['·', ',', '\'', '*'],
        Species::Blob => &['°', 'o', '.', '·'],
        Species::Ghost => &['~', '\'', ',', '*'],
        Species::Glitch => &[':', ';', '·', '░', '▪'],
        Species::Crystal => &['✦', '✧', '◇', '◆', '·'],
        Species::Mech => &['~', '°', '·', '●'],
    }
}

/// Per-biome floor-glyph palette — the ground texture under the pet, keyed to
/// the earned biome rather than species so the floor reads as a place.
pub(super) fn biome_floor_palette(tag: crate::tui::room::RoomBiomeTag) -> &'static [char] {
    use crate::tui::room::RoomBiomeTag;
    match tag {
        RoomBiomeTag::Starter => &['·', '.', ' ', ' '],
        RoomBiomeTag::Botanical => &[',', '·', '"', '.', ' '],
        RoomBiomeTag::Technical => &['·', '.', '.', ' ', ' '],
        RoomBiomeTag::Celestial => &['·', '˚', '.', ' ', ' '],
        RoomBiomeTag::Artifact => &['◦', '·', '°', '.', ' '],
        RoomBiomeTag::Cozy => &['·', '~', ',', '.', ' '],
    }
}

fn biome_floor_fill_percent(tag: crate::tui::room::RoomBiomeTag, phase: DayPhase) -> u16 {
    use crate::tui::room::RoomBiomeTag;
    let base = match tag {
        RoomBiomeTag::Starter => 58,
        RoomBiomeTag::Botanical => 64,
        RoomBiomeTag::Technical => 68,
        RoomBiomeTag::Celestial => 54,
        RoomBiomeTag::Artifact => 60,
        RoomBiomeTag::Cozy => 62,
    };
    match phase {
        DayPhase::Day => base,
        DayPhase::Dawn => base.saturating_sub(8),
        DayPhase::Dusk => base.saturating_sub(4),
        DayPhase::Night => base.saturating_sub(16),
    }
}

/// A whisper-quiet per-biome background wash: the theme bg nudged a few points
/// toward the biome's hue, so the habitat reads as a place even in a screenshot
/// without overpowering the pet/panels.
pub(super) fn biome_wash_color(tag: crate::tui::room::RoomBiomeTag) -> ratatui::style::Color {
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

fn work_weather_seed(weather: WorkWeather) -> u64 {
    match weather {
        WorkWeather::Clear => 0,
        WorkWeather::CacheMist => 1,
        WorkWeather::OutputSparks => 2,
        WorkWeather::ReasoningPulse => 3,
        WorkWeather::Mixed => 4,
    }
}

/// Per-phase sky glyph family, with `date_seed` picking among authored
/// variants per (species, phase) — the day's character is visual texture
/// only, never personality content (locked rule). Night stays a sparse
/// starfield, dawn/dusk warm grain, day a species family.
pub(super) fn sky_palette_for_phase(
    species: Species,
    phase: DayPhase,
    date_seed: u64,
) -> &'static [char] {
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
        DayPhase::Night => 0.45,
    }
}

/// Bounded seasonal hue drift on the sky color. Summer is the neutral
/// reference; the other seasons nudge channels by at most
/// SEASON_DRIFT_MAX_CHANNEL_NUDGE. Drift only — the season is never named in
/// any UI text, speech, or dream (spec: Seasons).
#[allow(dead_code)]
pub(super) const SEASON_DRIFT_MAX_CHANNEL_NUDGE: u8 = 8;

pub(super) fn season_hue_drift(color: Color, season: Season) -> Color {
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
pub(super) fn sky_color_for_phase(
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
        DayPhase::Night => dim_shift(base, 0.58),
    };
    climate_tint(
        season_hue_drift(lerp_color(base, target, blend), season),
        climate,
    )
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
        dim_shift(base, 0.55 * phase_blend)
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

    // Floor row: anchored to the bottom of habitat, but deliberately patchy so
    // it reads as ground texture rather than a full-width divider.
    let floor_row = habitat.y + habitat.height.saturating_sub(1);
    let floor_fill_percent = biome_floor_fill_percent(biome, phase);
    for dx in 0..habitat.width {
        if rng.gen_range(0..100_u16) >= floor_fill_percent {
            continue;
        }
        let glyph = *floor.choose(&mut rng).unwrap_or(&' ');
        if glyph == ' ' {
            continue;
        }
        let col = habitat.x + dx;
        let candidate = AmbientGlyph {
            row: floor_row,
            col,
            glyph,
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
pub(super) fn mote_density(ratio: f32) -> f32 {
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
pub(super) fn mote_glyphs_for(
    day: &DayContext,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
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
pub(super) fn effective_weekend_softening(day: &DayContext, profile: &PetLifeProfile) -> f32 {
    if profile.burst_level > 0.0 || profile.activity_level > 0.0 {
        return 0.0;
    }
    crate::tui::day::weekend_softening(day)
}

/// Weekend palette softening: pulls a scene color toward the neutral dim
/// base. Applied to the ambient and mote passes only — activity glyphs and
/// the pet itself are live channels and stay untouched.
const WEEKEND_PALETTE_SOFTEN_MAX: f32 = 0.25;

pub(super) fn weekend_soften_color(color: Color, softening: f32) -> Color {
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
pub(super) fn activity_glyphs_for(
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

pub(super) fn ambient_glyph_is_inside_area(glyph: &AmbientGlyph, area: Rect) -> bool {
    glyph.col >= area.x
        && glyph.row >= area.y
        && glyph.col < area.x.saturating_add(area.width)
        && glyph.row < area.y.saturating_add(area.height)
}
