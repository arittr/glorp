use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Style};

use crate::game::habitat::{catalog_prop, HabitatPetLayer, HabitatPropId, HabitatPropZone};
use crate::pet::animator::{
    compute_facing, compute_shimmer_role, compute_sleep_wander_x, compute_twinkle,
    compute_wake_wander_x, compute_wander_position_x, lazy_wander_instant,
    low_energy_lightness_multiplier,
};
use crate::pet::render::PaletteRoleName;
use crate::presentation::privacy::PresentationSurface;
use crate::presentation::scene::PresentationScene;
use crate::tui::component::{habitat_props_for, PetScene, PetSceneLayout};
use crate::tui::life::{build_prop_reactions, PetLifeProfile, PropReaction, PropReactionKind};
use crate::tui::panels::LegacyPanel;
use crate::tui::render_context::RenderContext;
use crate::tui::room::rects_contain;
use crate::tui::style::{semantic_styles, ColorCapability};
use crate::tui::view_model::WatchViewModel;

mod ambient;
mod art_lines;
mod colors;
mod performance;
mod props;

pub(crate) use ambient::pet_silhouette_halo_rects;
use ambient::{
    activity_glyphs_for, ambient_glyph_is_inside_area, biome_wash_color,
    effective_weekend_softening, mote_glyphs_for, weekend_soften_color,
};
pub use ambient::{ambient_glyphs_for, ambient_glyphs_for_phase};
#[allow(unused_imports)]
pub(crate) use art_lines::mirror_line;
pub(crate) use art_lines::pet_role_spans_for_line;
use art_lines::{
    build_pet_lines, cursor_normalized_x_within, render_pet_lines_sparse, render_speech_bubble,
};
pub(crate) use colors::pet_role_style;
use colors::{
    activity_glyph_budget, brighten_pet_role, darken_pet_styles, lift_pet_styles_for_activity,
    performance_lightness_multiplier, performance_posture_offset, profile_token_pop,
    seed_pet_palette, tint_pet_styles_for_phase,
};
use performance::apply_pet_performance_cues;

#[cfg(test)]
use crate::game::evolution::Stage;
#[cfg(test)]
use crate::pet::generation::Species;
#[cfg(test)]
use crate::tui::day::{DayPhase, Season};
#[cfg(test)]
use crate::tui::life::{SourceAccent, WorkWeather};
#[cfg(test)]
use ambient::biome_floor_palette;
#[cfg(test)]
use art_lines::{build_cursor_eye_string, cursor_eye_glyph};
#[cfg(test)]
use colors::{
    activity_glyph_color, activity_lift_style, apply_prop_reaction_style, tint_style_for_phase,
};
#[cfg(test)]
use ratatui::text::Line;

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
        let presentation_scene =
            PresentationScene::from_watch_view_model(vm, now, PresentationSurface::WatchTui);
        let scene = PetScene::compute_layout(area, vm, ctx);

        // Per-cell pet silhouette + 1-cell halo, shared by every pass that
        // wants pet avoidance. Replaces the inflated bounding rect so habitat
        // content can fill the diamond's negative space while keeping a
        // breathing margin around the actual pet outline.
        let species = vm.pet_render.generated_species;
        let _presentation_species = presentation_scene.pet.species.as_str();
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
        debug_assert_eq!(
            presentation_scene.room.primary_biome,
            format!("{:?}", room_profile.biome.primary)
        );
        debug_assert_eq!(
            presentation_scene.room.species_dialect,
            room_profile.species_dialect.key.as_str()
        );

        // Base layer: a subtle per-biome background wash over the ENTIRE habitat,
        // including under the pet and speech bubble. Set BEFORE every glyph pass
        // (room/ambient/pet all set fg only), so this bg stays seamless underneath
        // — washing around the pet's exclusion rect would carve a visible hole.
        {
            let wash = biome_wash_color(room_profile.biome.primary);
            for wy in scene.habitat.y..scene.habitat.y.saturating_add(scene.habitat.height) {
                for wx in scene.habitat.x..scene.habitat.x.saturating_add(scene.habitat.width) {
                    buf[(wx, wy)].set_style(ratatui::style::Style::default().bg(wash));
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
        props::render_prop_layers(
            buf,
            &prop_cells,
            &scene,
            &life_profile.prop_reactions,
            ctx.color_capability,
            &[HabitatPetLayer::Background, HabitatPetLayer::Behind],
        );

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
        props::render_prop_layer(
            buf,
            &prop_cells,
            &scene,
            &life_profile.prop_reactions,
            ctx.color_capability,
            HabitatPetLayer::Foreground,
        );
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
    let phase_blend = {
        let since = (now - vm.day_context.phase_started_at_utc).whole_seconds() as f32;
        (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
    };
    let base = tint_pet_styles_for_phase(&base, vm.day_context.day_phase, phase_blend);
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
        let floor_row = habitat.y + habitat.height - 1;
        let floor_count = glyphs.iter().filter(|g| g.row == floor_row).count();
        assert!(
            floor_count >= 10,
            "expected enough floor texture to ground the scene, got {floor_count}"
        );
    }

    #[test]
    fn ambient_floor_row_is_patchy_not_solid_line() {
        let habitat = Rect::new(0, 0, 52, 20);
        let now = datetime!(2026-06-11 10:00 UTC);
        let glyphs = ambient_glyphs_for_phase(
            Species::Glitch,
            Stage::S6,
            crate::tui::room::RoomBiomeTag::Technical,
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
        let floor_row = habitat.y + habitat.height - 1;
        let floor_count = glyphs.iter().filter(|g| g.row == floor_row).count();

        assert!(
            floor_count < habitat.width as usize * 3 / 4,
            "floor should be patchy, not full-width; got {floor_count}/{} cells",
            habitat.width
        );
        assert!(
            floor_count >= habitat.width as usize / 4,
            "floor should still read as ground texture; got {floor_count}/{} cells",
            habitat.width
        );
    }

    #[test]
    fn technical_floor_avoids_rule_glyphs() {
        let habitat = Rect::new(0, 0, 52, 20);
        let now = datetime!(2026-06-11 10:00 UTC);
        let glyphs = ambient_glyphs_for_phase(
            Species::Crystal,
            Stage::S6,
            crate::tui::room::RoomBiomeTag::Technical,
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
        let floor_row = habitat.y + habitat.height - 1;
        let rule_count = glyphs
            .iter()
            .filter(|g| g.row == floor_row && matches!(g.glyph, '─' | '┄'))
            .count();

        assert_eq!(
            rule_count, 0,
            "technical floor should use dot texture instead of dashed-rule glyphs"
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

    #[test]
    fn phase_tint_cools_pet_at_night() {
        use crate::tui::day::DayPhase;
        use ratatui::style::{Color, Style};
        let day = Style::default().fg(Color::Rgb(0xc0, 0xa0, 0x60));
        let night = tint_style_for_phase(day, DayPhase::Night, 1.0);
        let (Color::Rgb(_, _, db), Color::Rgb(_, _, nb)) = (day.fg.unwrap(), night.fg.unwrap())
        else {
            panic!("rgb");
        };
        // Night dims overall; assert it changed and is not brighter than day on red.
        assert_ne!(day.fg, night.fg, "night must retint");
        let Color::Rgb(dr, _, _) = day.fg.unwrap() else {
            panic!()
        };
        let Color::Rgb(nr, _, _) = night.fg.unwrap() else {
            panic!()
        };
        assert!(nr <= dr, "night should not warm/brighten red channel");
        let _ = (db, nb);
    }
}
