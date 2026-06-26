use glorp::game::evolution::Stage;
use glorp::game::metabolism::Mood;
use glorp::pet::art::{morph_count, stage_label};
use glorp::pet::generation::{generate_pet, resolve_accepted_name, Species};
use glorp::pet::render::{render_pet, species_animation_profile, AnimationFrame};

fn frame(tick: u64) -> AnimationFrame {
    AnimationFrame {
        tick,
        blink_suppression_ticks: 0,
        hold_eyes_closed: false,
        blink_slowdown: 0,
        ..AnimationFrame::default()
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
fn morph_count_reports_interior_texture_variants_not_silhouette_pools() {
    // New contract (Phase 1): morph_count is the number of deterministic
    // interior-texture variants a (species, stage) can render — NOT a hand-drawn
    // silhouette-pool size. It is >= 1 for every stage, and pinned to 1 on the
    // small stages (S0..S2) where texture is constant-occupancy.
    for species in Species::all() {
        for stage in [Stage::S0, Stage::S1, Stage::S2] {
            assert_eq!(
                morph_count(species, stage),
                1,
                "{species:?} {stage:?}: small stages pin interior texture to 1 variant"
            );
        }
        for stage in [Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
            assert!(
                morph_count(species, stage) >= 1,
                "{species:?} {stage:?}: every stage renders at least one variant"
            );
        }
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
fn species_animation_profiles_match_tokenpet_mockup() {
    // Breath is now owned by species_breath_rhythm_decis in animator.rs.
    // Only blink cadence lives in AnimationProfile. Cadence is lowered from the raw
    // mockup numbers (which assumed a faster ~214ms tick) so blinks read at the
    // companion's 250ms tick; species ordering (Fuzz/Glitch/Mech fast, Crystal slow)
    // is preserved.
    assert_eq!(species_animation_profile(Species::Fuzz).blink_average, 16);
    assert_eq!(species_animation_profile(Species::Fuzz).blink_jitter, 6);
    assert_eq!(species_animation_profile(Species::Ghost).blink_average, 26);
    assert_eq!(species_animation_profile(Species::Crystal).blink_jitter, 11);
    assert_eq!(species_animation_profile(Species::Mech).blink_average, 12);
}

#[test]
fn fixed_seed_set_renders_valid_non_empty_11x8_for_every_species_stage() {
    use unicode_width::UnicodeWidthStr;
    let stages = [
        Stage::S0,
        Stage::S1,
        Stage::S2,
        Stage::S3,
        Stage::S4,
        Stage::S5,
        Stage::S6,
    ];
    // A fixed seed set spanning the seed_hue space that drives interior texture.
    let seeds = [
        "mochi-7f3a",
        "alpha",
        "beta",
        "gamma",
        "ori-shard",
        "0x-404",
    ];
    for seed in seeds {
        for species in Species::all() {
            let pet = generate_pet(seed).with_species(species);
            for stage in stages {
                let rendered = render_pet(&pet, stage, Mood::Content, frame(0));
                // The framed grid is 10 rows x 13 cols; assert it is present and
                // rectangular, and that at least one art row is non-blank (no
                // blank pet).
                assert_eq!(
                    rendered.lines.len(),
                    10,
                    "seed={seed} {species:?} {stage:?} must render 10 framed rows"
                );
                for (row, line) in rendered.lines.iter().enumerate() {
                    assert_eq!(
                        UnicodeWidthStr::width(line.as_str()),
                        13,
                        "seed={seed} {species:?} {stage:?} row {row} must be 13 cols wide: \
                         {line:?}"
                    );
                }
                let any_ink = rendered
                    .lines
                    .iter()
                    .any(|line| line.chars().any(|c| c != ' '));
                assert!(
                    any_ink,
                    "seed={seed} {species:?} {stage:?} rendered a blank pet"
                );
            }
        }
    }
}

#[test]
fn glitch_resting_eyes_pool_has_no_corpse_eyes() {
    // Probe many seeds; the Glitch resting (Content) eyes must never be "x x".
    for n in 0..500 {
        let pet = generate_pet(&format!("glitch-pool-{n}")).with_species(Species::Glitch);
        assert_ne!(
            pet.traits.eyes, "x x",
            "Glitch resting eyes must never be corpse eyes"
        );
    }
}

#[test]
fn species_resting_eye_pools_are_three_columns() {
    use unicode_width::UnicodeWidthStr;
    // The Content (resting) eyes come from the per-seed pool, substituted into a
    // 3-col {eyes} slot; every pool entry must be exactly 3 display columns.
    for n in 0..500 {
        for species in Species::all() {
            let pet = generate_pet(&format!("eye-width-{n}")).with_species(species);
            assert_eq!(
                UnicodeWidthStr::width(pet.traits.eyes.as_str()),
                3,
                "{species:?} resting eyes {:?} must be 3 cols",
                pet.traits.eyes
            );
        }
    }
}
