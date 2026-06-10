use glorp::game::evolution::Stage;
use glorp::game::metabolism::Mood;
use glorp::pet::art::{morph_count, stage_label};
use glorp::pet::generation::{generate_pet, resolve_accepted_name, Species};
use glorp::pet::render::{palette_roles, render_pet, species_animation_profile, AnimationFrame};

fn frame(tick: u64) -> AnimationFrame {
    AnimationFrame {
        tick,
        blink_suppression_ticks: 0,
        hold_eyes_closed: false,
        blink_slowdown: 0,
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
    let fuzz = generate_pet("force-fuzz-1").with_species(Species::Fuzz);
    let mech = generate_pet("force-mech-1").with_species(Species::Mech);
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
    let a = render_pet(&pet, Stage::S3, Mood::Content, frame(42));
    let b = render_pet(&pet, Stage::S3, Mood::Content, frame(42));
    assert_eq!(a, b);
}

#[test]
fn different_same_species_seeds_have_visible_variation() {
    let a = generate_pet("blob-a").with_species(Species::Blob);
    let b = generate_pet("blob-b").with_species(Species::Blob);
    let art_a = render_pet(&a, Stage::S5, Mood::Content, frame(0));
    let art_b = render_pet(&b, Stage::S5, Mood::Content, frame(0));
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
fn tokenpet_stage_labels_match_spec() {
    let stages = [
        Stage::S0,
        Stage::S1,
        Stage::S2,
        Stage::S3,
        Stage::S4,
        Stage::S5,
        Stage::S6,
    ];
    let cases: &[(Species, [&str; 7])] = &[
        (
            Species::Fuzz,
            [
                "fluff",
                "fuzzling",
                "kit",
                "pup",
                "fuzz",
                "archfuzz",
                "mythic-fuzz",
            ],
        ),
        (
            Species::Blob,
            [
                "droplet",
                "blip",
                "globule",
                "wee-blob",
                "blob",
                "mega-blob",
                "primordial",
            ],
        ),
        (
            Species::Ghost,
            [
                "whisper",
                "wisp",
                "shade",
                "phantom-pup",
                "ghost",
                "wraith",
                "revenant",
            ],
        ),
        (
            Species::Glitch,
            [
                "bit", "byte", "packet", "thread", "glitch", "daemon", "kernel",
            ],
        ),
        (
            Species::Crystal,
            [
                "grain", "shard", "facet", "cluster", "crystal", "spire", "lodestar",
            ],
        ),
        (
            Species::Mech,
            [
                "chip", "bolt", "rivet", "drone", "mech", "archmech", "titan",
            ],
        ),
    ];
    for (species, labels) in cases {
        for (stage, expected) in stages.iter().zip(labels.iter()) {
            assert_eq!(
                stage_label(*species, *stage),
                *expected,
                "stage label mismatch for {species:?} {stage:?}"
            );
        }
    }
}

#[test]
fn species_have_enough_seeded_morph_variety() {
    for species in Species::all() {
        // Pup (S3) has a single template per pet.jsx; adult stages have
        // multiple morphs whose exact count comes from `pet/art.rs` templates.
        assert_eq!(morph_count(species, Stage::S3), 1);
        assert!(
            morph_count(species, Stage::S4) >= 3,
            "expected {species:?} adult templates to provide >=3 morphs"
        );
        assert!(
            morph_count(species, Stage::S6) >= 3,
            "expected {species:?} sage templates to provide >=3 morphs"
        );
    }
}

#[test]
fn adult_stages_have_distinct_silhouettes_for_representative_species() {
    // For a few representative pets, S0 / S1 / S2 should produce different
    // rendered output. (Stages may share head parts but body height tiers
    // differ, so the rendered braille should differ.)
    for seed in ["alpha", "beta", "gamma"] {
        let pet = generate_pet(seed);
        let s0 = render_pet(&pet, Stage::S0, Mood::Content, frame(0));
        let s1 = render_pet(&pet, Stage::S1, Mood::Content, frame(0));
        let s2 = render_pet(&pet, Stage::S2, Mood::Content, frame(0));
        assert_ne!(s0.lines, s1.lines, "seed {seed}: S0 vs S1 should differ");
        assert_ne!(s1.lines, s2.lines, "seed {seed}: S1 vs S2 should differ");
    }
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
