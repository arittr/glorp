use glorp::game::evolution::Stage;
use glorp::game::metabolism::Mood;
use glorp::pet::art::{morph_count, stage_label};
use glorp::pet::generation::{generate_pet, resolve_accepted_name, Species};
use glorp::pet::render::{
    closed_blink_eyes, palette_roles, render_pet, species_animation_profile, AnimationFrame,
    PaletteRoleName,
};

fn frame(tick: u64) -> AnimationFrame {
    AnimationFrame {
        tick,
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
    let a = render_pet(&pet, Stage::S3, Mood::Content, frame(42));
    let b = render_pet(&pet, Stage::S3, Mood::Content, frame(42));
    assert_eq!(a, b);
}

#[test]
fn different_same_species_seeds_have_visible_variation() {
    let a = generate_pet("blob-a").with_species_for_test(Species::Blob);
    let b = generate_pet("blob-b").with_species_for_test(Species::Blob);
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
            ["chip", "bolt", "rivet", "drone", "mech", "warmech", "titan"],
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
        // Pup (S3) has 1 template per pet.jsx; adults (S4/S5/S6) have 3.
        assert_eq!(morph_count(species, Stage::S3), 1);
        assert_eq!(morph_count(species, Stage::S4), 3);
        assert_eq!(morph_count(species, Stage::S6), 3);
    }
}

#[test]
fn adult_stages_have_distinct_silhouettes_for_representative_species() {
    // Per pet.jsx, S4 always renders adult[0] and S5 renders adult[morph % len];
    // pick a non-zero morph index so S4 vs S5 differ.
    for species in [Species::Fuzz, Species::Blob, Species::Mech] {
        let mut pet = generate_pet(&format!("silhouette-{}", species.as_str()))
            .with_species_for_test(species);
        pet.traits.morph_index = 1;
        pet.traits.morph_pup_index = 0;

        let silhouettes = [Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6]
            .into_iter()
            .map(|stage| render_pet(&pet, stage, Mood::Content, frame(7)).lines)
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
fn evolution_event_has_renderable_celebration() {
    let pet = generate_pet("ori-shard");
    let art = render_pet(&pet, Stage::S4, Mood::Happy, frame(1))
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
    let art = render_pet(&pet, Stage::S4, Mood::Content, frame(3));
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
    let a = render_pet(&pet, Stage::S3, Mood::Content, frame(50));
    let b = render_pet(&pet, Stage::S3, Mood::Content, frame(51));
    assert_ne!(a.lines, b.lines);
    assert_eq!(closed_blink_eyes(Species::Ghost), "\u{2014} \u{2014}");

    let sad = render_pet(&pet, Stage::S3, Mood::Sad, frame(50));
    let sleepy = render_pet(&pet, Stage::S3, Mood::Sleepy, frame(50));
    let wilted = render_pet(&pet, Stage::S3, Mood::Wilted, frame(50));
    assert!(!sad.lines.join("\n").contains("\u{2014} \u{2014}"));
    assert!(!sleepy.lines.join("\n").contains("\u{2014} \u{2014}"));
    assert!(!wilted.lines.join("\n").contains("\u{2014} \u{2014}"));
}

#[test]
fn pet_art_renders_in_13_by_10_frame() {
    let pet = generate_pet("frame-shape").with_species_for_test(Species::Fuzz);
    let art = render_pet(&pet, Stage::S5, Mood::Content, frame(0));
    assert_eq!(art.lines.len(), 10, "framed pet should be 10 rows tall");
    for (idx, line) in art.lines.iter().enumerate() {
        assert!(
            line.chars().count() == 13,
            "row {idx} should be 13 chars, got {}: {line:?}",
            line.chars().count()
        );
    }
}

#[test]
fn s6_pet_has_sage_sparkle_top_and_bottom_lines() {
    let pet = generate_pet("sage-test").with_species_for_test(Species::Fuzz);
    let art = render_pet(&pet, Stage::S6, Mood::Content, frame(0));
    // Frame row 0 is the particle border; row 1 is the sage top.
    let top = art.lines.get(1).expect("sage frame should have art row 1");
    let bottom = art.lines.get(8).expect("sage frame should have art row 8");
    assert!(
        top.contains('*'),
        "sage stage row 1 should contain sparkle '*': {top:?}"
    );
    assert!(
        bottom.contains('\u{2726}'),
        "sage stage row 8 should contain sparkle '\u{2726}': {bottom:?}"
    );
}

#[test]
fn glitch_corruption_fires_on_some_tick_multiple_of_37() {
    // Per pet.jsx glitchCorrupt fires when tick % 37 == 0 and the targeted
    // cell is in a body span and not a space. Whether the targeted cell is
    // non-space depends on tick (which scales row/col indices) and morph.
    // Sweep ticks 0, 37, 74, ... up through 37 * 50 to find at least one
    // tick where the rendered output differs from the previous tick by
    // exactly the corruption swap. Suppress blink so the diff isn't a
    // blink frame.
    let mut pet = generate_pet("glitch-corrupt").with_species_for_test(Species::Glitch);
    pet.traits.morph_index = 0;
    pet.traits.morph_pup_index = 0;
    let mut saw_corruption = false;
    for k in 0..50u64 {
        let tick = 37 * (k + 1);
        let baseline = render_pet(
            &pet,
            Stage::S5,
            Mood::Content,
            AnimationFrame {
                tick: tick - 1,
                blink_suppression_ticks: 99,
            },
        );
        let corrupted = render_pet(
            &pet,
            Stage::S5,
            Mood::Content,
            AnimationFrame {
                tick,
                blink_suppression_ticks: 99,
            },
        );
        if baseline.lines != corrupted.lines {
            saw_corruption = true;
            break;
        }
    }
    assert!(
        saw_corruption,
        "glitch corruption should change at least one cell on at least one tick % 37 == 0"
    );
}

#[test]
fn particles_render_in_border_cells_per_species() {
    for species in Species::all() {
        let pet =
            generate_pet(&format!("particle-{}", species.as_str())).with_species_for_test(species);
        // Sample many ticks and look for any particle span in the outer border.
        let saw_particle = (0..120).any(|tick| {
            let art = render_pet(&pet, Stage::S5, Mood::Content, frame(tick));
            art.spans.iter().any(|span| {
                span.role == PaletteRoleName::Particle
                    && (span.line == 0 || span.line == 9 || span.start == 0 || span.start == 12)
            })
        });
        assert!(
            saw_particle,
            "{species:?} should render at least one particle in a border cell across the sampled ticks"
        );
    }
}

#[test]
fn pet_uses_tokenpet_eye_mouth_overrides_per_mood() {
    let pet = generate_pet("mood-eye-mouth").with_species_for_test(Species::Fuzz);
    let cases: &[(Mood, &str, &str)] = &[
        (Mood::Happy, "^.^", "\u{03c9}"),
        (Mood::Hungry, "u.u", "o"),
        (Mood::Sad, "T.T", "\u{fe35}"),
        (Mood::Sleepy, "-.-", "-"),
        (Mood::Wilted, ",_,", "_"),
    ];
    for (mood, eyes, mouth) in cases {
        let art = render_pet(&pet, Stage::S5, Mood::Content, frame(0));
        let neutral = art.lines.join("\n");
        let _ = neutral;
        let mood_art = render_pet(&pet, Stage::S5, *mood, frame(1));
        let joined = mood_art.lines.join("\n");
        assert!(
            joined.contains(eyes),
            "mood {mood:?} should render eyes {eyes:?}, got:\n{joined}"
        );
        assert!(
            joined.contains(mouth),
            "mood {mood:?} should render mouth {mouth:?}, got:\n{joined}"
        );
    }
}

#[test]
fn every_species_stage_morph_renders_at_13_wide_with_inner_11() {
    let stages = [
        Stage::S0,
        Stage::S1,
        Stage::S2,
        Stage::S3,
        Stage::S4,
        Stage::S5,
        Stage::S6,
    ];
    for species in Species::all() {
        for stage in stages {
            for morph in 0..3usize {
                let mut pet =
                    generate_pet(&format!("shape-{}-{stage:?}-{morph}", species.as_str()))
                        .with_species_for_test(species);
                pet.traits.morph_index = morph;
                pet.traits.morph_pup_index = morph;
                let art = render_pet(&pet, stage, Mood::Content, frame(2));
                assert_eq!(
                    art.lines.len(),
                    10,
                    "{species:?} {stage:?} morph {morph} should yield 10 framed rows"
                );
                for (idx, line) in art.lines.iter().enumerate() {
                    let width = line.chars().count();
                    assert_eq!(
                        width, 13,
                        "{species:?} {stage:?} morph {morph} row {idx} should be 13 chars wide, got {width}: {line:?}"
                    );
                    // Inner 11 is row[1..=11]; just sanity-check the inner slice exists.
                    let inner: String = line.chars().skip(1).take(11).collect();
                    assert_eq!(
                        inner.chars().count(),
                        11,
                        "{species:?} {stage:?} morph {morph} row {idx} inner should be 11 chars"
                    );
                }
            }
        }
    }
}

#[test]
fn blink_is_suppressed_for_four_ticks_after_mood_change() {
    let pet = generate_pet("blink-seed").with_species_for_test(Species::Ghost);
    let blink_tick = (0..200)
        .find(|tick| {
            render_pet(&pet, Stage::S3, Mood::Content, frame(*tick))
                .lines
                .join("\n")
                .contains("\u{2014} \u{2014}")
        })
        .expect("fixture should blink during the sampled window");

    let blink = render_pet(&pet, Stage::S3, Mood::Content, frame(blink_tick));
    assert!(blink.lines.join("\n").contains("\u{2014} \u{2014}"));

    let suppressed = render_pet(
        &pet,
        Stage::S3,
        Mood::Content,
        AnimationFrame {
            tick: blink_tick,
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
            blink_suppression_ticks: 0,
        },
    );
    assert!(resumed.lines.join("\n").contains("\u{2014} \u{2014}"));
}
