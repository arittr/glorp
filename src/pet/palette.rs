//! Pet color resolution: OKLCH -> sRGB, and per-pet/species palettes.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
