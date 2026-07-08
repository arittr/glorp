use super::animator::PixelRendererState;
use super::input::PixelPetInput;
use crate::pet::generation::Species;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelPetScene {
    pub body_rx: i16,
    pub body_ry: i16,
    pub accent_count: u8,
    pub blocky: bool,
    pub wispy: bool,
    pub wander_x: f32,
    pub breath_y: f32,
    pub blink_closed: bool,
    pub pulse_alpha: f32,
}

impl PixelPetScene {
    pub fn from_input(
        input: &PixelPetInput,
        state: &PixelRendererState,
        now: time::OffsetDateTime,
    ) -> Self {
        let elapsed_ms = i64::try_from((now - state.start).whole_milliseconds()).unwrap_or(
            if now >= state.start {
                i64::MAX
            } else {
                i64::MIN
            },
        );
        Self::from_elapsed_ms(input, elapsed_ms)
    }

    pub(crate) fn from_elapsed_ms(input: &PixelPetInput, elapsed_ms: i64) -> Self {
        let stage_scale = 1.0 + input.identity.stage.index() as f32 * 0.075;
        let calm_mult = if input.sleep.calm { 0.28 } else { 1.0 };
        let wander_phase =
            elapsed_ms as f32 / 900.0 + f32::from(input.identity.variation_key.bucket(19)) * 0.17;
        let breath_phase = elapsed_ms as f32 / if input.sleep.asleep { 1_900.0 } else { 760.0 };
        let blink_period = 2_600 + i64::from(input.identity.variation_key.bucket(900));
        let blink_closed = elapsed_ms.rem_euclid(blink_period) < 120 && !input.sleep.asleep;
        let pulse_alpha = if input.pulse.active {
            (1.0 - (f32::from(input.pulse.age_ms) / 2_000.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let (base_rx, base_ry, accent_count, blocky, wispy) = match input.identity.species {
            Species::Fuzz => (15, 13, 4, false, false),
            Species::Blob => (16, 11, 2, false, false),
            Species::Ghost => (13, 16, 3, false, true),
            Species::Glitch => (14, 12, 7, true, false),
            Species::Crystal => (13, 15, 6, true, false),
            Species::Mech => (15, 12, 5, true, false),
        };

        Self {
            body_rx: (base_rx as f32 * stage_scale).round() as i16,
            body_ry: (base_ry as f32 * stage_scale).round() as i16,
            accent_count,
            blocky,
            wispy,
            wander_x: wander_phase.sin() * 7.0 * calm_mult,
            breath_y: breath_phase.sin() * if input.sleep.asleep { 1.0 } else { 2.4 },
            blink_closed,
            pulse_alpha,
        }
    }
}
