use crate::presentation::pixel::{
    PixelActivity, PixelPetIdentity, PixelPetInput, PixelPulseState, PixelSleepState,
    PixelVariationKey,
};
use crate::presentation::surface::{resolve_pet_colors, LiveColorInputs, PIXEL_STYLE};
use crate::tui::view_model::WatchViewModel;

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
