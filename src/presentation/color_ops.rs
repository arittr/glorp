use crate::pet::palette::Rgb;
use crate::tui::day::DayPhase;

pub fn blend(primary: Rgb, secondary: Rgb, primary_weight: f32) -> Rgb {
    let w = primary_weight.clamp(0.0, 1.0);
    let inv = 1.0 - w;
    Rgb::new(
        ((primary.r as f32 * w) + (secondary.r as f32 * inv)).round() as u8,
        ((primary.g as f32 * w) + (secondary.g as f32 * inv)).round() as u8,
        ((primary.b as f32 * w) + (secondary.b as f32 * inv)).round() as u8,
    )
}

pub fn warm_shift(c: Rgb, amount: f32) -> Rgb {
    let t = amount.clamp(0.0, 1.0);
    let add = (t * 40.0).round() as u8;
    let sub = (t * 30.0).round() as u8;
    Rgb::new(c.r.saturating_add(add), c.g, c.b.saturating_sub(sub))
}

pub fn cool_shift(c: Rgb, amount: f32) -> Rgb {
    let amt = amount.clamp(0.0, 1.0);
    let r2 = (f32::from(c.r) * (1.0 - 0.5 * amt)).round() as u8;
    let b2 = (f32::from(c.b) + (255.0 - f32::from(c.b)) * 0.25 * amt).round() as u8;
    Rgb::new(r2, c.g, b2)
}

pub fn dim_shift(c: Rgb, amount: f32) -> Rgb {
    let m = 1.0 - amount.clamp(0.0, 1.0) * 0.5;
    Rgb::new(
        (c.r as f32 * m).round() as u8,
        (c.g as f32 * m).round() as u8,
        (c.b as f32 * m).round() as u8,
    )
}

pub fn tint_for_phase(c: Rgb, phase: DayPhase, blend: f32) -> Rgb {
    match phase {
        DayPhase::Day => c,
        DayPhase::Dawn => warm_shift(c, 0.10 * blend),
        DayPhase::Dusk => warm_shift(c, 0.18 * blend),
        DayPhase::Night => dim_shift(cool_shift(c, 0.18 * blend), 0.28 * blend),
    }
}

pub fn brighten_channel(c: Rgb, multiplier: f32) -> Rgb {
    let m = multiplier.max(0.0);
    Rgb::new(
        (c.r as f32 * m).min(255.0) as u8,
        (c.g as f32 * m).min(255.0) as u8,
        (c.b as f32 * m).min(255.0) as u8,
    )
}

pub fn darken_channel(c: Rgb, multiplier: f32) -> Rgb {
    let m = multiplier.clamp(0.0, 1.0);
    Rgb::new(
        (c.r as f32 * m) as u8,
        (c.g as f32 * m) as u8,
        (c.b as f32 * m) as u8,
    )
}

pub fn activity_lift_channel(c: Rgb, activity_level: f32) -> Rgb {
    let lift = (activity_level.clamp(0.0, 2.0) * 22.0) as u8;
    Rgb::new(
        c.r.saturating_add(lift),
        c.g.saturating_add(lift),
        c.b.saturating_add(lift),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::palette::Rgb;
    use crate::tui::day::DayPhase;

    #[test]
    fn warm_shift_adds_red_subtracts_blue() {
        // amount 1.0 => +40 red (saturating), -30 blue (saturating), green unchanged
        assert_eq!(
            warm_shift(Rgb::new(100, 100, 100), 1.0),
            Rgb::new(140, 100, 70)
        );
        assert_eq!(warm_shift(Rgb::new(250, 0, 10), 1.0), Rgb::new(255, 0, 0)); // saturates
    }

    #[test]
    fn dim_shift_scales_by_one_minus_half_amount() {
        // m = 1.0 - amount*0.5; amount 1.0 => 0.5x
        assert_eq!(
            dim_shift(Rgb::new(200, 100, 80), 1.0),
            Rgb::new(100, 50, 40)
        );
    }

    #[test]
    fn darken_channel_clamps_multiplier_0_to_1() {
        assert_eq!(
            darken_channel(Rgb::new(200, 100, 80), 0.5),
            Rgb::new(100, 50, 40)
        );
        assert_eq!(
            darken_channel(Rgb::new(200, 100, 80), 2.0),
            Rgb::new(200, 100, 80)
        ); // clamp to 1.0
    }

    #[test]
    fn brighten_channel_caps_at_255() {
        assert_eq!(
            brighten_channel(Rgb::new(200, 100, 80), 1.4),
            Rgb::new(255, 140, 112)
        );
    }

    #[test]
    fn activity_lift_adds_level_times_22_saturating() {
        // lift = (level.clamp(0,2) * 22) as u8; level 2.0 => +44
        assert_eq!(
            activity_lift_channel(Rgb::new(100, 100, 100), 2.0),
            Rgb::new(144, 144, 144)
        );
        assert_eq!(
            activity_lift_channel(Rgb::new(250, 250, 250), 2.0),
            Rgb::new(255, 255, 255)
        );
    }

    #[test]
    fn phase_tint_matches_legacy_curve() {
        let c = Rgb::new(120, 120, 120);
        assert_eq!(tint_for_phase(c, DayPhase::Day, 1.0), c); // day = identity
        assert_eq!(tint_for_phase(c, DayPhase::Dusk, 1.0), warm_shift(c, 0.18));
        assert_eq!(tint_for_phase(c, DayPhase::Dawn, 1.0), warm_shift(c, 0.10));
        assert_eq!(
            tint_for_phase(c, DayPhase::Night, 1.0),
            dim_shift(cool_shift(c, 0.18), 0.28)
        );
    }
}
