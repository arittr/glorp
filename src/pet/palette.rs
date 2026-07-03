//! Pet color resolution: OKLCH -> sRGB, and per-pet/species palettes.

use crate::game::metabolism::Mood;
use crate::pet::render::PaletteRoleName;

/// Backend-neutral 8-bit color. Each surface adapts this to its own type
/// (ratatui `Color::Rgb`, the companion `RoundColor`, the menubar `Rgb`, or a
/// `#rrggbb` preview string).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Convert an OKLCH color (lightness 0..1, chroma >=0, hue in degrees) to sRGB.
/// Out-of-gamut requests are mapped by reducing chroma at fixed lightness and
/// hue (never per-channel clamping, which shifts hue).
pub fn oklch_to_rgb(lightness: f32, chroma: f32, hue_degrees: f32) -> Rgb {
    let in_gamut = |c: f32| {
        oklch_to_linear(lightness, c, hue_degrees)
            .iter()
            .all(|&v| (-1e-4..=1.0 + 1e-4).contains(&v))
    };

    let chroma = if in_gamut(chroma) {
        chroma
    } else {
        // Binary search the largest in-gamut chroma in [0, chroma].
        let mut lo = 0.0_f32;
        let mut hi = chroma;
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if in_gamut(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    };

    let linear = oklch_to_linear(lightness, chroma, hue_degrees);
    Rgb {
        r: encode_srgb(linear[0]),
        g: encode_srgb(linear[1]),
        b: encode_srgb(linear[2]),
    }
}

fn oklch_to_linear(lightness: f32, chroma: f32, hue_degrees: f32) -> [f32; 3] {
    let h = hue_degrees.to_radians();
    let a = chroma * h.cos();
    let b = chroma * h.sin();

    let l_ = lightness + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = lightness - 0.105_561_35 * a - 0.063_854_17 * b;
    let s_ = lightness - 0.089_484_18 * a - 1.291_485_5 * b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

fn encode_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Recover the OKLab a/b chromatic axes from an sRGB color. Test-only (the
/// production path is forward-only); kept here so the matrices stay co-located
/// with their inverse.
#[cfg(test)]
fn rgb_to_oklab_ab(c: Rgb) -> (f32, f32) {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = lin(c.r);
    let g = lin(c.g);
    let b = lin(c.b);

    let l = 0.412_165_6 * r + 0.536_275_2 * g + 0.051_457_57 * b;
    let m = 0.211_859_1 * r + 0.680_718_9 * g + 0.107_406_58 * b;
    let s = 0.088_309_795 * r + 0.281_847_8 * g + 0.630_257 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    let a = 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_;
    let b = 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_;
    (a, b)
}

/// Resolved per-role colors for one pet. `eye` is the per-species resting color
/// (complementary to the body); expressive moods overwrite it via mood eye color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPalette {
    pub body: Rgb,
    pub eye: Rgb,
    pub mouth: Rgb,
    pub accent: Rgb,
    pub pattern: Rgb,
    pub particle: Rgb,
    pub corruption: Rgb,
}

/// The `█` "glow" cells are a lifted tint of whatever the body color resolved
/// to — derived, not stored, so it automatically tracks every dim/lift/tint
/// transform the body goes through. ~30% toward white reads as a lit highlight
/// while keeping the hue.
pub fn body_glow(body: Rgb) -> Rgb {
    let lift = |c: u8| c.saturating_add(((255 - c) as u16 * 30 / 100) as u8);
    Rgb::new(lift(body.r), lift(body.g), lift(body.b))
}

pub fn role_color(role: PaletteRoleName, palette: &ResolvedPalette) -> Rgb {
    match role {
        PaletteRoleName::Body => palette.body,
        PaletteRoleName::BodyGlow => body_glow(palette.body),
        PaletteRoleName::Eye => palette.eye,
        PaletteRoleName::Mouth => palette.mouth,
        PaletteRoleName::Accent => palette.accent,
        PaletteRoleName::Pattern => palette.pattern,
        PaletteRoleName::Particle => palette.particle,
        PaletteRoleName::Corruption => palette.corruption,
    }
}

/// The pre-color fixed theme, reproducing `semantic_styles()` pet colors exactly.
pub fn default_theme_palette() -> ResolvedPalette {
    ResolvedPalette {
        body: Rgb::new(0xef, 0xeb, 0xe4),
        eye: Rgb::new(0x82, 0xbc, 0x83),
        mouth: Rgb::new(0x97, 0x91, 0x8a),
        accent: Rgb::new(0xf0, 0xa6, 0x46),
        pattern: Rgb::new(0x50, 0x4c, 0x49),
        particle: Rgb::new(0xf0, 0xa6, 0x46),
        corruption: Rgb::new(0x78, 0xff, 0xb4),
    }
}

use crate::pet::generation::{Species, VisibleTraits};

/// Hue (OKLCH degrees) each species leans toward.
fn species_base_hue(species: Species) -> f32 {
    match species {
        Species::Fuzz => 40.0,     // peach
        Species::Blob => 150.0,    // mint
        Species::Ghost => 300.0,   // lavender
        Species::Glitch => 345.0,  // magenta/hot-pink daemon
        Species::Crystal => 230.0, // ice
        Species::Mech => 75.0,     // amber/brass
    }
}

/// Per-species body chroma (OKLCH). Raised off the old pinned 0.10 so the
/// species hue actually registers. Soft-bodied/pale species (Crystal ice,
/// Ghost lavender) stay lower; saturated identities (Glitch magenta, Mech amber)
/// go higher. `oklch_to_rgb` gamut-maps any out-of-gamut request, so these are
/// safe ceilings, not exact realized chroma.
fn species_body_chroma(species: Species) -> f32 {
    match species {
        Species::Fuzz => 0.13,    // peach
        Species::Blob => 0.14,    // mint
        Species::Ghost => 0.12,   // lavender (pale, keep soft)
        Species::Glitch => 0.18,  // magenta/hot-pink (loud)
        Species::Crystal => 0.11, // ice (cold, pale shell)
        Species::Mech => 0.15,    // amber/brass
    }
}

/// Mood -> eye color (OKLCH-resolved). Green at rest, warming to gold when
/// excited, cooling to blue when tired, desaturating toward grey when wilted.
/// This is the eye-color half of the mood signal; the eye *glyph* is owned by
/// `expression_for` (render.rs) and updates on the same animation tick.
pub fn eye_color_for_mood(mood: Mood) -> Rgb {
    // (lightness, chroma, hue) tuned so each clears the resting green floor's
    // intent while staying calm (no neon). Wilted drops chroma to near-grey.
    let (l, c, h) = match mood {
        Mood::Content => (0.82, 0.19, 145.0), // resting green
        Mood::Happy => (0.84, 0.20, 130.0),   // brighter green, a touch warm
        Mood::Ecstatic => (0.86, 0.20, 95.0), // warm gold-green
        Mood::Hungry => (0.80, 0.18, 70.0),   // amber-warm (seeking)
        Mood::Sad => (0.74, 0.14, 250.0),     // cool, muted blue
        Mood::Sleepy => (0.78, 0.15, 250.0),  // cool blue, tired
        Mood::Wilted => (0.70, 0.03, 145.0),  // desaturated grey-green
    };
    oklch_to_rgb(l, c, h)
}

/// Overwrite only the eye role with the mood-driven color, for the expressive
/// (non-Content) moods. Content keeps the per-species resting eye that
/// resolve_pet_palette assigns (bright, complementary to the body hue). Hooked at
/// the per-tick render site (rerender_pet_for_view_model).
pub fn apply_mood_eye_color(palette: &mut ResolvedPalette, mood: Mood) {
    if mood != Mood::Content {
        palette.eye = eye_color_for_mood(mood);
    }
}

pub fn resolve_pet_palette(species: Species, traits: &VisibleTraits) -> ResolvedPalette {
    let base = species_base_hue(species);
    // Per-pet hue jitter: map seed_hue (0..360) to +-18 degrees off the family.
    let jitter = (f32::from(traits.seed_hue) / 360.0 - 0.5) * 36.0;
    let h = (base + jitter).rem_euclid(360.0);
    let sat = f32::from(traits.saturation_percent) / 100.0; // 0.82..1.0

    let role = |lightness: f32, chroma: f32, hue: f32| oklch_to_rgb(lightness, chroma * sat, hue);

    ResolvedPalette {
        body: role(0.74, species_body_chroma(species), h),
        // Bright resting eye at the complementary hue to the body, so it reads as a
        // distinct, vivid eye (via hue, not a dark luminance shift) and gives each
        // species its own resting eye color.
        eye: role(0.80, 0.20, (h + 180.0).rem_euclid(360.0)),
        mouth: role(0.70, 0.16, h + 35.0),
        accent: role(0.76, 0.24, h + 120.0),
        pattern: role(0.64, 0.20, h + 210.0),
        particle: role(0.80, 0.20, h + 160.0),
        // Corruption is species-independent acid/phosphor, fixed-hue so it
        // always contrasts the body and reads as a deliberate data glitch,
        // not a tint of the creature. High chroma green at high lightness.
        corruption: oklch_to_rgb(0.85, 0.22, 145.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an sRGB color (0.0 black .. 1.0 white).
    fn relative_luminance(c: Rgb) -> f32 {
        let chan = |v: u8| {
            let s = f32::from(v) / 255.0;
            if s <= 0.039_28 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * chan(c.r) + 0.7152 * chan(c.g) + 0.0722 * chan(c.b)
    }

    fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
        let la = relative_luminance(a);
        let lb = relative_luminance(b);
        let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn black_and_white_are_exact() {
        assert_eq!(oklch_to_rgb(0.0, 0.0, 0.0), Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(oklch_to_rgb(1.0, 0.0, 0.0), Rgb { r: 255, g: 255, b: 255 });
    }

    #[test]
    fn zero_chroma_is_achromatic() {
        let gray = oklch_to_rgb(0.6, 0.0, 210.0);
        assert_eq!(gray.r, gray.g);
        assert_eq!(gray.g, gray.b);
    }

    #[test]
    fn higher_lightness_is_brighter() {
        let dark = oklch_to_rgb(0.3, 0.0, 0.0).r;
        let light = oklch_to_rgb(0.8, 0.0, 0.0).r;
        assert!(light > dark, "{light} !> {dark}");
    }

    #[test]
    fn output_is_always_in_gamut() {
        // Request an absurd chroma; gamut mapping must still yield valid bytes
        // (no panic, no wraparound) by reducing chroma, not clamping channels.
        for hue in (0..360).step_by(15) {
            let _ = oklch_to_rgb(0.7, 0.5, hue as f32);
        }
    }

    #[test]
    fn gamut_mapping_preserves_hue() {
        // An out-of-gamut request and its in-gamut chroma-reduced version
        // should share (approximately) the same hue.
        let requested_hue = 30.0_f32;
        let mapped = oklch_to_rgb(0.7, 0.5, requested_hue);
        let safe = oklch_to_rgb(0.7, 0.08, requested_hue);
        let dh = (rgb_hue(mapped) - rgb_hue(safe)).abs();
        let dh = dh.min(360.0 - dh);
        assert!(dh < 8.0, "hue drift {dh} too large");
    }

    // Test helper: recover OKLCH hue from an Rgb for the hue-preservation test.
    fn rgb_hue(c: Rgb) -> f32 {
        let (a, b) = rgb_to_oklab_ab(c);
        b.atan2(a).to_degrees().rem_euclid(360.0)
    }

    #[test]
    fn default_theme_matches_current_fixed_colors() {
        use crate::pet::render::PaletteRoleName::*;
        let p = default_theme_palette();
        assert_eq!(role_color(Body, &p), Rgb::new(0xef, 0xeb, 0xe4));
        assert_eq!(role_color(Eye, &p), Rgb::new(0x82, 0xbc, 0x83));
        assert_eq!(role_color(Mouth, &p), Rgb::new(0x97, 0x91, 0x8a));
        assert_eq!(role_color(Accent, &p), Rgb::new(0xf0, 0xa6, 0x46));
        assert_eq!(role_color(Pattern, &p), Rgb::new(0x50, 0x4c, 0x49));
        // Default theme keeps particle == accent for pre-color parity, but it is
        // now a dedicated field (role_color reads palette.particle, not accent).
        assert_eq!(role_color(Particle, &p), p.particle);
        assert_eq!(p.particle, Rgb::new(0xf0, 0xa6, 0x46));
        assert_eq!(role_color(Corruption, &p), Rgb::new(0x78, 0xff, 0xb4));
    }

    #[test]
    fn particle_is_its_own_species_hue() {
        use crate::pet::generation::Species;
        let p = resolve_pet_palette(Species::Crystal, &traits_with_hue(0));
        assert_ne!(
            p.particle, p.accent,
            "particle should resolve to its own hue, not reuse accent"
        );
    }

    fn traits_with_hue(hue: u16) -> crate::pet::generation::VisibleTraits {
        crate::pet::generation::VisibleTraits {
            eyes: "o o".into(),
            mouth: "w".into(),
            pattern: "...".into(),
            accent: "*".into(),
            palette_index: 0,
            morph_index: 0,
            morph_pup_index: 0,
            seed_hue: hue,
            saturation_percent: 90,
        }
    }

    #[test]
    fn resolve_is_deterministic() {
        use crate::pet::generation::Species;
        let a = resolve_pet_palette(Species::Fuzz, &traits_with_hue(42));
        let b = resolve_pet_palette(Species::Fuzz, &traits_with_hue(42));
        assert_eq!(a, b);
    }

    #[test]
    fn resting_eye_is_bright_and_complementary_per_species() {
        use crate::pet::generation::Species;
        // The resting eye is bright (not the old dark forest-green) and sits at a
        // complementary hue to the body, giving each species its own eye color.
        for s in Species::all() {
            let p = resolve_pet_palette(s, &traits_with_hue(123));
            let maxc = p.eye.r.max(p.eye.g).max(p.eye.b);
            assert!(
                maxc > 150,
                "{s:?} resting eye too dark (max channel {maxc}): {:?}",
                p.eye
            );
            let mut dh = (rgb_hue(p.eye) - rgb_hue(p.body)).abs();
            dh = dh.min(360.0 - dh);
            assert!(
                dh > 90.0,
                "{s:?} resting eye hue not distinct from body (dh={dh:.0})"
            );
        }
    }

    #[test]
    fn resting_eye_stays_hue_distinct_from_body_across_seeds() {
        use crate::pet::generation::Species;
        // The resting eye reads via its complementary HUE, not a strict luminance
        // floor (the expressive mood eyes never cleared 3:1 and read fine). Sweep
        // seeds so the jittered body never collapses the hue gap.
        for s in Species::all() {
            for hue in (0..360).step_by(30) {
                let p = resolve_pet_palette(s, &traits_with_hue(hue));
                let mut dh = (rgb_hue(p.eye) - rgb_hue(p.body)).abs();
                dh = dh.min(360.0 - dh);
                assert!(
                    dh > 90.0,
                    "{s:?} hue {hue}: eye/body hue gap {dh:.0} too small"
                );
            }
        }
    }

    #[test]
    fn eye_color_is_green_at_rest_and_shifts_with_mood() {
        use crate::game::metabolism::Mood;
        let rest = eye_color_for_mood(Mood::Content);
        assert!(
            rest.g > rest.r && rest.g > rest.b,
            "resting (Content) eye must read green, got {rest:?}"
        );
        // Excited -> warm/gold (red+green high, blue low; warmer than rest).
        let excited = eye_color_for_mood(Mood::Ecstatic);
        assert!(
            excited.r >= rest.r,
            "excited eye should warm toward gold (more red than rest)"
        );
        // Tired -> cool blue (blue dominates).
        let tired = eye_color_for_mood(Mood::Sleepy);
        assert!(
            tired.b > tired.r,
            "tired (Sleepy) eye should read cool/blue, got {tired:?}"
        );
        // Wilted -> desaturated/grey (channels close together).
        let wilted = eye_color_for_mood(Mood::Wilted);
        let spread = wilted.r.abs_diff(wilted.g).max(wilted.g.abs_diff(wilted.b));
        assert!(
            spread < 24,
            "wilted eye should desaturate toward grey, got spread {spread}"
        );
    }

    #[test]
    fn apply_mood_eye_color_overwrites_only_the_eye() {
        use crate::game::metabolism::Mood;
        use crate::pet::generation::Species;
        let mut p = resolve_pet_palette(Species::Blob, &traits_with_hue(7));
        let (body, mouth, accent, pattern, particle) =
            (p.body, p.mouth, p.accent, p.pattern, p.particle);
        apply_mood_eye_color(&mut p, Mood::Sleepy);
        assert_eq!(p.eye, eye_color_for_mood(Mood::Sleepy));
        assert_eq!(
            (p.body, p.mouth, p.accent, p.pattern, p.particle),
            (body, mouth, accent, pattern, particle),
            "mood eye color must not touch any other role"
        );
    }

    #[test]
    fn live_resting_eye_is_a_noop_and_stays_distinct() {
        use crate::game::metabolism::Mood;
        use crate::pet::generation::Species;
        // The per-tick hook calls apply_mood_eye_color every tick incl. Content.
        // Content must be a no-op so the live resting eye keeps its per-species
        // complementary color, which stays hue-distinct from the body.
        for s in Species::all() {
            for hue in (0..360).step_by(30) {
                let resolved = resolve_pet_palette(s, &traits_with_hue(hue));
                let mut p = resolved;
                apply_mood_eye_color(&mut p, Mood::Content);
                assert_eq!(
                    p.eye, resolved.eye,
                    "{s:?}: Content must not change the resting eye"
                );
                let mut dh = (rgb_hue(p.eye) - rgb_hue(p.body)).abs();
                dh = dh.min(360.0 - dh);
                assert!(
                    dh > 90.0,
                    "{s:?} hue {hue}: live eye/body hue gap {dh:.0} too small"
                );
            }
        }
    }

    #[test]
    fn species_lean_separates_bodies() {
        use crate::pet::generation::Species;
        let fuzz = resolve_pet_palette(Species::Fuzz, &traits_with_hue(0)).body;
        let blob = resolve_pet_palette(Species::Blob, &traits_with_hue(0)).body;
        assert_ne!(fuzz, blob);
    }

    #[test]
    fn per_pet_variety_within_species() {
        use crate::pet::generation::Species;
        let a = resolve_pet_palette(Species::Fuzz, &traits_with_hue(10)).body;
        let b = resolve_pet_palette(Species::Fuzz, &traits_with_hue(300)).body;
        assert_ne!(a, b);
    }

    #[test]
    fn contrast_ratio_white_on_black_is_twenty_one() {
        let white = Rgb::new(255, 255, 255);
        let black = Rgb::new(0, 0, 0);
        let ratio = contrast_ratio(white, black);
        assert!(
            (ratio - 21.0).abs() < 0.1,
            "white-on-black contrast should be ~21:1, got {ratio}"
        );
    }

    #[test]
    fn contrast_ratio_is_symmetric_and_one_for_identical() {
        let c = Rgb::new(0x82, 0xbc, 0x83);
        assert!(
            (contrast_ratio(c, c) - 1.0).abs() < 1e-4,
            "identical colors are 1:1"
        );
        let d = Rgb::new(0x13, 0x11, 0x0f);
        assert!(
            (contrast_ratio(c, d) - contrast_ratio(d, c)).abs() < 1e-4,
            "contrast is symmetric"
        );
    }

    #[test]
    fn bodies_are_visibly_chromatic_not_grey() {
        use crate::pet::generation::Species;
        for s in Species::all() {
            let body = resolve_pet_palette(s, &traits_with_hue(0)).body;
            let (a, b) = rgb_to_oklab_ab(body);
            let chroma = (a * a + b * b).sqrt();
            assert!(
                chroma > 0.04,
                "{s:?} body reads near-grey (oklab chroma {chroma:.3}); raise species_body_chroma"
            );
        }
    }

    #[test]
    fn raised_chroma_exceeds_old_pinned_point_one_zero() {
        use crate::pet::generation::Species;
        // Proves the species_body_chroma raise had a visible effect: the realized
        // oklab chroma of each body now exceeds what the old pinned 0.10 request
        // realized at the same lightness/hue. Guards against reverting the knob.
        // Both paths use the species base hue with no jitter so the only variable
        // is chroma (0.10 old vs species_body_chroma(s) new).
        for s in Species::all() {
            let h = species_base_hue(s);
            let new_body = oklch_to_rgb(0.74, species_body_chroma(s), h);
            let old_body = oklch_to_rgb(0.74, 0.10, h);
            let (na, nb) = rgb_to_oklab_ab(new_body);
            let (oa, ob) = rgb_to_oklab_ab(old_body);
            let new_chroma = (na * na + nb * nb).sqrt();
            let old_chroma = (oa * oa + ob * ob).sqrt();
            assert!(
                new_chroma > old_chroma,
                "{s:?}: raised chroma {new_chroma:.3} should exceed old pinned-0.10 chroma {old_chroma:.3}"
            );
        }
    }

    #[test]
    fn species_base_hues_match_identity_family() {
        use crate::pet::generation::Species;
        // OKLCH hue degrees (approx): peach ~40, mint ~150, lavender ~300,
        // magenta ~345, ice ~230, amber ~75. Verify the family anchor, not exact
        // realized RGB (that depends on chroma/jitter).
        assert!((species_base_hue(Species::Fuzz) - 40.0).abs() < 1.0);
        assert!((species_base_hue(Species::Blob) - 150.0).abs() < 1.0);
        assert!((species_base_hue(Species::Ghost) - 300.0).abs() < 1.0);
        assert!((species_base_hue(Species::Glitch) - 345.0).abs() < 1.0);
        assert!((species_base_hue(Species::Crystal) - 230.0).abs() < 1.0);
        assert!((species_base_hue(Species::Mech) - 75.0).abs() < 1.0);
    }

    #[test]
    fn all_species_bodies_are_mutually_distinct() {
        use crate::pet::generation::Species;
        let bodies: Vec<_> = Species::all()
            .into_iter()
            .map(|s| resolve_pet_palette(s, &traits_with_hue(0)).body)
            .collect();
        for (i, a) in bodies.iter().enumerate() {
            for b in bodies.iter().skip(i + 1) {
                assert_ne!(a, b, "two species bodies collided after hue retune");
            }
        }
    }

    #[test]
    fn corruption_role_resolves_to_a_contrasting_acid_color() {
        use crate::pet::generation::Species;
        use crate::pet::render::PaletteRoleName::Corruption;
        let p = resolve_pet_palette(Species::Glitch, &traits_with_hue(50));
        let c = role_color(Corruption, &p);
        // Acid/phosphor: green dominant, distinct from the body so corruption
        // never melts into its background (Appendix B failure mode #3).
        assert!(c.g > c.r && c.g > c.b, "corruption not acid-green: {c:?}");
        assert_ne!(c, p.body, "corruption must contrast the body");
    }

    #[test]
    fn default_theme_has_a_corruption_color() {
        use crate::pet::render::PaletteRoleName::Corruption;
        let p = default_theme_palette();
        let c = role_color(Corruption, &p);
        assert!(
            c.g > c.r && c.g > c.b,
            "default corruption not acid-green: {c:?}"
        );
    }
}
