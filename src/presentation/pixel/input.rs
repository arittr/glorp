use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::generation::Species;
use crate::presentation::surface::{
    resolve_pet_colors, LiveColorInputs, ResolvedColors, PIXEL_STYLE,
};
use crate::tui::view_model::WatchViewModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelVariationKey(pub u16);

impl PixelVariationKey {
    pub fn from_seed(seed: &str) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in seed.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self((hash ^ (hash >> 32)) as u16)
    }

    pub const fn bucket(self, modulo: u16) -> u16 {
        if modulo == 0 {
            0
        } else {
            self.0 % modulo
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelPetIdentity {
    pub species: Species,
    pub stage: Stage,
    pub variation_key: PixelVariationKey,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelActivity {
    pub level: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSleepState {
    pub asleep: bool,
    pub calm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelPulseState {
    pub active: bool,
    pub age_ms: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelPetInput {
    pub identity: PixelPetIdentity,
    pub mood: Mood,
    pub palette: ResolvedColors,
    pub activity: PixelActivity,
    pub sleep: PixelSleepState,
    pub pulse: PixelPulseState,
}

impl PixelPetInput {
    pub fn from_watch_view_model(vm: &WatchViewModel, now: time::OffsetDateTime) -> Self {
        let mut color_inputs = LiveColorInputs::passthrough();
        color_inputs.activity_level = vm.life_profile.activity_level;
        let palette = resolve_pet_colors(&vm.pet_palette, &color_inputs, &PIXEL_STYLE);
        let pulse_age_ms = vm.last_feed_pulse_at.map(|pulse| {
            (now - pulse)
                .whole_milliseconds()
                .clamp(0, i128::from(u16::MAX)) as u16
        });
        let pulse_active =
            pulse_age_ms.is_some_and(|age| age <= 2_000) && vm.life_profile.burst_level > 0.0;

        Self {
            identity: PixelPetIdentity {
                species: vm.pet_render.generated_species,
                stage: vm.pet_render.stage,
                variation_key: PixelVariationKey::from_seed(&vm.pet_render.seed),
            },
            mood: vm.pet_render.mood,
            palette,
            activity: PixelActivity { level: vm.life_profile.activity_level },
            sleep: PixelSleepState {
                asleep: vm.day_context.asleep,
                calm: vm.life_profile.calm_mode || vm.day_context.asleep,
            },
            pulse: PixelPulseState {
                active: pulse_active,
                age_ms: pulse_age_ms.unwrap_or(u16::MAX),
            },
        }
    }
}
