use glorp::game::evolution::Stage;
use glorp::game::metabolism::Mood;
use glorp::pet::art::{morph_count, stage_label};
use glorp::pet::generation::{generate_pet, resolve_accepted_name, Species};
use glorp::pet::render::{
    closed_blink_eyes, palette_roles, render_pet, species_animation_profile, AnimationFrame,
    PaletteRoleName,
};

fn frame(tick: u64, compact: bool) -> AnimationFrame {
    AnimationFrame {
        tick,
        compact,
        blink_suppression_ticks: 0,
    }
}

#[test]
fn same_seed_generates_same_pet() {
    let a = generate_pet("mochi-7f3a");
    let b = generate_pet("mochi-7f3a");
    assert_eq!(a, b);
}

#[test]
fn mvp_species_are_available() {
    let all = Species::all();
    assert!(all.contains(&Species::Fuzz));
    assert!(all.contains(&Species::Blob));
    assert!(all.contains(&Species::Ghost));
    assert!(all.contains(&Species::Glitch));
    assert!(all.contains(&Species::Crystal));
    assert!(all.contains(&Species::Mech));
    assert_eq!(all.len(), 6);
}

#[test]
fn species_names_have_distinct_grammar() {
    let fuzz = generate_pet("force-fuzz-1").with_species_for_test(Species::Fuzz);
    let mech = generate_pet("force-mech-1").with_species_for_test(Species::Mech);
    assert_ne!(fuzz.generated_name, mech.generated_name);
    assert!(fuzz.generated_name.chars().all(|c| c.is_ascii_lowercase()));
    assert!(
        mech.generated_name.chars().any(|c| c.is_ascii_digit())
            || mech.generated_name.contains('-')
    );
}

#[test]
fn hatching_name_decision_accepts_generated_or_replacement_name() {
    let pet = generate_pet("mochi-7f3a");
    assert_eq!(
        resolve_accepted_name(&pet.generated_name, None),
        pet.generated_name
    );
    assert_eq!(
        resolve_accepted_name(&pet.generated_name, Some("sprig")),
        "sprig"
    );
}

#[test]
fn seeded_generation_selects_visible_traits_palette_morph_and_phase() {
    let pet = generate_pet("visible-traits");
    assert!(!pet.traits.eyes.is_empty());
    assert!(!pet.traits.mouth.is_empty());
    assert!(!pet.traits.pattern.is_empty());
    assert!(!pet.traits.accent.is_empty());
    assert!(pet.traits.palette_index < 8);
    assert!(pet.traits.morph_index < 4);
    assert!(pet.animation_phase.breath < 64);
    assert!(pet.animation_phase.blink < 64);
    assert_ne!(pet.animation_phase.breath, pet.animation_phase.blink);
}

#[test]
fn render_is_stable_for_same_seed_state_and_tick() {
    let pet = generate_pet("mochi-7f3a");
    let a = render_pet(&pet, Stage::S3, Mood::Content, frame(42, false));
    let b = render_pet(&pet, Stage::S3, Mood::Content, frame(42, false));
    assert_eq!(a, b);
}

#[test]
fn different_same_species_seeds_have_visible_variation() {
    let a = generate_pet("blob-a").with_species_for_test(Species::Blob);
    let b = generate_pet("blob-b").with_species_for_test(Species::Blob);
    let art_a = render_pet(&a, Stage::S5, Mood::Content, frame(0, false));
    let art_b = render_pet(&b, Stage::S5, Mood::Content, frame(0, false));
    assert_ne!(art_a.lines, art_b.lines);
}

#[test]
fn stage_labels_are_species_specific_across_shared_thresholds() {
    assert_ne!(
        stage_label(Species::Fuzz, Stage::S3),
        stage_label(Species::Mech, Stage::S3)
    );
    assert_ne!(
        stage_label(Species::Ghost, Stage::S6),
        stage_label(Species::Crystal, Stage::S6)
    );
}

#[test]
fn species_have_enough_seeded_morph_variety() {
    for species in Species::all() {
        assert!(morph_count(species, Stage::S1) >= 2);
        assert!(morph_count(species, Stage::S4) >= 3);
        assert!(morph_count(species, Stage::S6) >= 3);
    }
}

#[test]
fn adult_stages_have_distinct_silhouettes_for_representative_species() {
    for species in [Species::Fuzz, Species::Blob, Species::Mech] {
        let mut pet = generate_pet(&format!("silhouette-{}", species.as_str()))
            .with_species_for_test(species);
        pet.traits.morph_index = 0;

        let silhouettes = [Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6]
            .into_iter()
            .map(|stage| render_pet(&pet, stage, Mood::Content, frame(7, true)).lines)
            .collect::<Vec<_>>();

        for pair in silhouettes.windows(2) {
            assert_ne!(
                pair[0], pair[1],
                "{species:?} reuses adjacent adult silhouette"
            );
        }
    }
}

#[test]
fn compact_render_has_bounded_width() {
    let pet = generate_pet("mech-compact");
    let art = render_pet(&pet, Stage::S6, Mood::Wilted, frame(9, true));
    assert!(art.lines.iter().all(|line| line.chars().count() <= 18));
}

#[test]
fn evolution_event_has_renderable_celebration() {
    let pet = generate_pet("ori-shard");
    let art = render_pet(&pet, Stage::S4, Mood::Happy, frame(1, false))
        .with_evolution_flash(Stage::S3, Stage::S4);
    assert!(art.event_lines.iter().any(|line| line.contains("evolved")));
}

#[test]
fn palette_roles_follow_tokenpet_hue_offsets() {
    let mut pet = generate_pet("ori-shard");
    pet.traits.saturation_percent = 50;
    let roles = palette_roles(&pet);
    assert_eq!(roles.body.lightness, 0.84);
    assert_eq!(roles.body.base_chroma, 0.05);
    assert_eq!(roles.eye.hue_offset_degrees, 180);
    assert_eq!(roles.eye.lightness, 0.84);
    assert_eq!(roles.eye.base_chroma, 0.065);
    assert_eq!(roles.mouth.hue_offset_degrees, 30);
    assert_eq!(roles.accent.hue_offset_degrees, 90);
    assert_eq!(roles.pattern.hue_offset_degrees, 150);
}

#[test]
fn rendered_spans_segment_template_slots_by_palette_role() {
    let pet = generate_pet("role-spans").with_species_for_test(Species::Fuzz);
    let art = render_pet(&pet, Stage::S4, Mood::Content, frame(3, false));
    let roles = art.spans.iter().map(|span| span.role).collect::<Vec<_>>();

    for role in [
        PaletteRoleName::Body,
        PaletteRoleName::Eye,
        PaletteRoleName::Mouth,
        PaletteRoleName::Accent,
        PaletteRoleName::Pattern,
    ] {
        assert!(roles.contains(&role), "missing {role:?} span");
    }
}

#[test]
fn species_animation_profiles_match_tokenpet_mockup() {
    assert_eq!(species_animation_profile(Species::Fuzz).breath_period, 16);
    assert_eq!(species_animation_profile(Species::Fuzz).breath_hold, 4);
    assert_eq!(species_animation_profile(Species::Fuzz).blink_average, 32);
    assert_eq!(species_animation_profile(Species::Fuzz).blink_jitter, 12);
    assert_eq!(species_animation_profile(Species::Blob).breath_period, 13);
    assert_eq!(species_animation_profile(Species::Ghost).blink_average, 50);
    assert_eq!(species_animation_profile(Species::Glitch).breath_period, 9);
    assert_eq!(species_animation_profile(Species::Crystal).blink_jitter, 22);
    assert_eq!(species_animation_profile(Species::Mech).blink_average, 22);
}

#[test]
fn blink_is_seeded_desynchronized_and_mood_safe() {
    let pet = generate_pet("blink-seed").with_species_for_test(Species::Ghost);
    let a = render_pet(&pet, Stage::S3, Mood::Content, frame(50, false));
    let b = render_pet(&pet, Stage::S3, Mood::Content, frame(51, false));
    assert_ne!(a.lines, b.lines);
    assert_eq!(closed_blink_eyes(Species::Ghost), "\u{2014} \u{2014}");

    let sad = render_pet(&pet, Stage::S3, Mood::Sad, frame(50, false));
    let sleepy = render_pet(&pet, Stage::S3, Mood::Sleepy, frame(50, false));
    let wilted = render_pet(&pet, Stage::S3, Mood::Wilted, frame(50, false));
    assert!(!sad.lines.join("\n").contains("\u{2014} \u{2014}"));
    assert!(!sleepy.lines.join("\n").contains("\u{2014} \u{2014}"));
    assert!(!wilted.lines.join("\n").contains("\u{2014} \u{2014}"));
}

#[test]
fn blink_is_suppressed_for_four_ticks_after_mood_change() {
    let pet = generate_pet("blink-seed").with_species_for_test(Species::Ghost);
    let blink_tick = (0..200)
        .find(|tick| {
            render_pet(&pet, Stage::S3, Mood::Content, frame(*tick, false))
                .lines
                .join("\n")
                .contains("\u{2014} \u{2014}")
        })
        .expect("fixture should blink during the sampled window");

    let blink = render_pet(&pet, Stage::S3, Mood::Content, frame(blink_tick, false));
    assert!(blink.lines.join("\n").contains("\u{2014} \u{2014}"));

    let suppressed = render_pet(
        &pet,
        Stage::S3,
        Mood::Content,
        AnimationFrame {
            tick: blink_tick,
            compact: false,
            blink_suppression_ticks: 4,
        },
    );
    assert!(!suppressed.lines.join("\n").contains("\u{2014} \u{2014}"));

    let resumed = render_pet(
        &pet,
        Stage::S3,
        Mood::Content,
        AnimationFrame {
            tick: blink_tick,
            compact: false,
            blink_suppression_ticks: 0,
        },
    );
    assert!(resumed.lines.join("\n").contains("\u{2014} \u{2014}"));
}
