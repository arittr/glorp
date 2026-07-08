use crate::presentation::pixel::{
    PixelActivity, PixelArtPoseKey, PixelArtReferenceRequest, PixelCanonicalAnimationInputs,
    PixelPetIdentity, PixelPetInput, PixelPulseState, PixelSleepState, PixelVariationKey,
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
        let pulse_active = !vm.day_context.asleep
            && pulse_age_ms.is_some_and(|age| age <= 2_000)
            && vm.life_profile.burst_level > 0.0;

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

    pub fn from_watch_view_model_with_art_request(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
    ) -> (Self, PixelArtReferenceRequest) {
        let pose_tick = (now - time::OffsetDateTime::UNIX_EPOCH)
            .whole_milliseconds()
            .max(0) as u64
            / 250;
        Self::from_watch_view_model_with_canonical_art_request(
            vm,
            now,
            PixelCanonicalAnimationInputs {
                tick: pose_tick,
                hold_eyes_closed: vm.day_context.asleep,
                blink_suppression_ticks: 0,
            },
        )
    }

    pub fn from_watch_view_model_with_canonical_art_request(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
        animation_inputs: PixelCanonicalAnimationInputs,
    ) -> (Self, PixelArtReferenceRequest) {
        let input = Self::from_watch_view_model(vm, now);
        let feed_reaction =
            crate::pet::animator::compute_token_pop(vm.last_feed_pulse_at, now).is_some();
        let pet_performance = crate::tui::room::pet_performance_from_day_context(&vm.day_context);
        let glitch_corruption =
            if vm.pet_render.generated_species == crate::pet::generation::Species::Glitch {
                Some(crate::pet::render::glitch_corruption_frame_for_inputs(
                    vm.day_context.date_seed,
                    vm.day_context.today_ratio,
                    vm.life_profile.burst_level,
                    vm.life_profile.calm_mode,
                    feed_reaction,
                ))
            } else {
                None
            };
        let animation_frame = crate::pet::render::AnimationFrame {
            tick: animation_inputs.tick,
            blink_suppression_ticks: animation_inputs.blink_suppression_ticks,
            hold_eyes_closed: animation_inputs.hold_eyes_closed,
            blink_slowdown: crate::pet::render::blink_slowdown_for_tiredness(
                vm.day_context.tiredness,
            ),
            soft_eyes: matches!(
                pet_performance,
                crate::tui::room::PetPerformance::TiredAwake
                    | crate::tui::room::PetPerformance::HeavyDayCozy
            ),
            work_accent: crate::pet::render::work_accent_for_profile(&vm.life_profile),
            feed_reaction,
            glitch_corruption,
        };
        let request = PixelArtReferenceRequest {
            seed: vm.pet_render.seed.clone(),
            species: vm.pet_render.generated_species,
            stage: vm.pet_render.stage,
            mood: vm.pet_render.mood,
            variation_bucket: input.identity.variation_key.0,
            pose: PixelArtPoseKey::from_animation_frame(animation_frame),
            animation_frame,
        };
        (input, request)
    }
}
