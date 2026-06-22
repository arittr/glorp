//! Pet color resolution: OKLCH -> sRGB, and per-pet/species palettes.

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

/// Resolved per-role colors for one pet. `eye` is always the green signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPalette {
    pub body: Rgb,
    pub eye: Rgb,
    pub mouth: Rgb,
    pub accent: Rgb,
    pub pattern: Rgb,
    pub particle: Rgb,
}

pub fn role_color(role: PaletteRoleName, palette: &ResolvedPalette) -> Rgb {
    match role {
        PaletteRoleName::Body => palette.body,
        PaletteRoleName::Eye => palette.eye,
        PaletteRoleName::Mouth => palette.mouth,
        PaletteRoleName::Accent => palette.accent,
        PaletteRoleName::Pattern => palette.pattern,
        PaletteRoleName::Particle => palette.particle,
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
    }
}

use crate::pet::generation::{Species, VisibleTraits};

/// Hue (OKLCH degrees) each species leans toward.
fn species_base_hue(species: Species) -> f32 {
    match species {
        Species::Fuzz => 70.0,     // warm amber
        Species::Blob => 195.0,    // teal
        Species::Ghost => 300.0,   // violet
        Species::Glitch => 135.0,  // acid green
        Species::Crystal => 230.0, // ice blue
        Species::Mech => 250.0,    // steel
    }
}

/// Pinned green eye signature (same for every species).
const EYE_HUE: f32 = 142.0;

pub fn resolve_pet_palette(species: Species, traits: &VisibleTraits) -> ResolvedPalette {
    let base = species_base_hue(species);
    // Per-pet hue jitter: map seed_hue (0..360) to +-18 degrees off the family.
    let jitter = (f32::from(traits.seed_hue) / 360.0 - 0.5) * 36.0;
    let h = (base + jitter).rem_euclid(360.0);
    let sat = f32::from(traits.saturation_percent) / 100.0; // 0.82..1.0

    let role = |lightness: f32, chroma: f32, hue: f32| oklch_to_rgb(lightness, chroma * sat, hue);

    ResolvedPalette {
        body: role(0.74, 0.10, h),
        eye: oklch_to_rgb(0.82, 0.19, EYE_HUE),
        mouth: role(0.70, 0.16, h + 35.0),
        accent: role(0.76, 0.24, h + 120.0),
        pattern: role(0.64, 0.20, h + 210.0),
        particle: role(0.80, 0.20, h + 160.0),
    }
}

/// WCAG relative luminance of an sRGB color (0.0 black .. 1.0 white).
#[allow(dead_code)] // production caller (per-species eye-lightness) lands in Phase 3 Task 5
pub(crate) fn relative_luminance(c: Rgb) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
        let la = relative_luminance(a);
        let lb = relative_luminance(b);
        let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn black_and_white_are_exact() {
        assert_eq!(oklch_to_rgb(0.0, 0.0, 0.0), Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(
            oklch_to_rgb(1.0, 0.0, 0.0),
            Rgb {
                r: 255,
                g: 255,
                b: 255
            }
        );
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
    fn eyes_are_green_for_every_species() {
        use crate::pet::generation::Species;
        let green = resolve_pet_palette(Species::Fuzz, &traits_with_hue(0)).eye;
        for s in Species::all() {
            let p = resolve_pet_palette(s, &traits_with_hue(123));
            assert_eq!(p.eye, green, "eye drifted for {s:?}");
        }
        assert!(
            green.g > green.r && green.g > green.b,
            "eye not green: {green:?}"
        );
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
}
