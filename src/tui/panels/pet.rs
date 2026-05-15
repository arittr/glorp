use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::game::evolution::Stage;
use crate::pet::animator::{
    compute_facing, compute_shimmer_role, compute_token_pop, compute_twinkle,
    compute_wander_position_x, low_energy_lightness_multiplier,
};
use crate::pet::generation::Species;
use crate::pet::render::PaletteRoleName;
use crate::tui::component::{habitat_props_for, PetScene, PetSceneLayout};
use crate::tui::panels::LegacyPanel;
use crate::tui::render_context::RenderContext;
use crate::tui::style::{semantic_styles, SemanticStyles};
use crate::tui::view_model::WatchViewModel;

pub struct PetPanel;

/// The rendered pet art is 13 columns wide (11 chars + 1-cell particle border each side)
/// and 10 rows tall (8 art rows + 1-cell particle border top/bottom).
const PET_W: u16 = 13;
const PET_H: u16 = 10;

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
        Species::Glitch => &['▒', '▓', '░', '▪'],
        Species::Crystal => &['✦', '✧', '·', '◆'],
        Species::Mech => &['~', '°', '·', '●'],
    }
}

/// Per-species floor-glyph palette (each cell of the floor row is drawn from this).
fn floor_palette_for(species: Species) -> &'static [char] {
    match species {
        Species::Fuzz => &['·', ',', '.', ' ', ' '],
        Species::Blob => &['~', '.', ',', ' '],
        Species::Ghost => &['\'', ' ', ' ', ' '],
        Species::Glitch => &['▒', '░', '▓', ' '],
        Species::Crystal => &['·', '.', ' ', ' ', ' '],
        Species::Mech => &['─', '·', '.', ' '],
    }
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
/// habitat with species-appropriate ground cover.
pub fn ambient_glyphs_for(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: crate::tui::style::ColorCapability,
) -> Vec<AmbientGlyph> {
    if matches!(color_capability, crate::tui::style::ColorCapability::Flat) {
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

    let sky = sky_palette_for(species);
    let floor = floor_palette_for(species);

    let p = crate::tui::style::tokenpet_palette();
    let sky_color = p.dim.rgb;
    let floor_color = p.dim.rgb;

    let mut glyphs = Vec::new();

    let habitat_cells = (habitat.width as usize) * (habitat.height as usize);
    let area_term = habitat_cells.saturating_sub(200) / 60;
    let count = stage_base_count(stage) + area_term;

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

fn inflate_rect(r: Rect, by: u16) -> Rect {
    let x = r.x.saturating_sub(by);
    let y = r.y.saturating_sub(by);
    let width = r.width.saturating_add(2 * by);
    let height = r.height.saturating_add(2 * by);
    Rect::new(x, y, width, height)
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
        let wander_x = compute_wander_position_x(area.width, species, now);
        let facing = compute_facing(area.width, species, now);
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

        // Pass 1: ambient backdrop. Inflate the pet art rect by 1 cell to
        // create a respect ring before passing exclusions to the painter.
        let now = ctx.clock.now_utc();
        let species = vm.pet_render.generated_species;
        let stage = vm.pet_render.stage;
        let inflated_pet = inflate_rect(scene.pet_art, 1);
        let inflated_exclusions: Vec<Rect> = scene
            .exclusions
            .iter()
            .map(|&r| if r == scene.pet_art { inflated_pet } else { r })
            .collect();
        let glyphs = ambient_glyphs_for(
            species,
            stage,
            scene.habitat,
            &inflated_exclusions,
            now,
            ctx.color_capability,
        );
        for g in glyphs {
            if ambient_glyph_is_inside_area(&g, scene.habitat) {
                let cell = &mut buf[(g.col, g.row)];
                cell.set_char(g.glyph);
                cell.set_style(ratatui::style::Style::default().fg(g.color));
            }
        }

        for prop in habitat_props_for(&vm.habitat, &scene, species, &vm.pet_render.seed, ctx) {
            if prop.col >= scene.habitat.x
                && prop.row >= scene.habitat.y
                && prop.col < scene.habitat.x.saturating_add(scene.habitat.width)
                && prop.row < scene.habitat.y.saturating_add(scene.habitat.height)
            {
                let cell = &mut buf[(prop.col, prop.row)];
                cell.set_char(prop.glyph);
                cell.set_style(prop.style);
            }
        }

        // Pass 3: pet art with shimmer, twinkle, and token-pop overlays.
        render_pet_inside(buf, vm, &scene, now);
    }
}

/// Renders the speech bubble and pet art into `area`, centered vertically.
/// This is the pre-existing render logic extracted from the old `render` body.
fn render_pet_inside(
    buf: &mut Buffer,
    vm: &WatchViewModel,
    scene: &PetSceneLayout,
    now: time::OffsetDateTime,
) {
    let base = semantic_styles();
    let m = low_energy_lightness_multiplier(vm.energy);
    let droop = darken_pet_styles(&base, m);

    if let (Some(speech_area), Some(speech)) = (scene.speech, vm.current_speech.as_deref()) {
        render_speech_bubble(speech_area, buf, speech, &droop);
    }

    let species = vm.pet_render.generated_species;
    let shimmer_role = compute_shimmer_role(species, now);
    let twinkle = compute_twinkle(species, now);
    let token_pop = compute_token_pop(vm.last_feed_pulse_at, now);

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
    let lines = build_pet_lines(
        vm,
        scene.pet_art.width as usize,
        &shimmer_styles,
        cursor_norm_x,
        effective_twinkle,
    );
    Paragraph::new(lines).render(scene.pet_art, buf);
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
/// x ∈ [-1.0, 1.0] relative to the panel center, or None when the cursor is
/// outside the rect, missing, or mouse tracking is disabled.
fn cursor_normalized_x_within(vm: &WatchViewModel, area: Rect) -> Option<f32> {
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
            spans.extend(build_owned_spans_for_line(
                &art_line,
                line_index,
                &art_spans,
                styles,
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
        let style = pet_role_style(segment.role, styles);
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
    eye_override: Option<char>,
) -> Vec<Span<'a>> {
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
        return vec![Span::styled(art_line, styles.pet_body)];
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
            spans.push(Span::styled(body, styles.pet_body));
        }
        let style = pet_role_style(segment.role, styles);
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
        spans.push(Span::styled(tail, styles.pet_body));
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

pub(crate) fn pet_role_style(role: PaletteRoleName, styles: &SemanticStyles) -> Style {
    match role {
        PaletteRoleName::Body => styles.pet_body,
        PaletteRoleName::Eye => styles.pet_eye,
        PaletteRoleName::Mouth => styles.pet_mouth,
        PaletteRoleName::Accent => styles.pet_accent,
        PaletteRoleName::Pattern => styles.pet_pattern,
        PaletteRoleName::Particle => styles.pet_accent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

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
            },
        );
        let mut vm = WatchViewModel::fixture();
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    #[test]
    fn pet_role_style_maps_eye_role_to_eye_style() {
        let styles = semantic_styles();
        assert_eq!(
            pet_role_style(PaletteRoleName::Eye, &styles),
            styles.pet_eye
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
}
