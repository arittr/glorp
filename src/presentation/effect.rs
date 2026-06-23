use crate::pet::animator::{
    compute_shimmer_role, compute_token_pop, compute_twinkle, TokenPop, TwinkleSpec,
};
use crate::pet::render::PaletteRoleName;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

/// The per-frame, surface-agnostic pet effects: a wisp-shimmer role, an
/// occasional twinkle, and the post-feed token-pop flash. All three depend only
/// on the pet's species and the current instant (plus idle/feed state) — never on
/// the viewport or cursor — so any surface can build the identical `EffectState`
/// from the view model and the frame's `now`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectState {
    pub shimmer_role: Option<PaletteRoleName>,
    pub twinkle: Option<TwinkleSpec>,
    pub token_pop: Option<TokenPop>,
}

impl EffectState {
    pub fn from_vm(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
        color_capability: ColorCapability,
    ) -> EffectState {
        let species = vm.pet_render.generated_species;
        EffectState {
            shimmer_role: compute_shimmer_role(species, now),
            twinkle: compute_twinkle(species, now, vm.life_profile.idle.idle_minutes),
            token_pop: token_pop_for(vm, now, color_capability),
        }
    }
}

/// Post-feed flash, gated off in calm mode, with no burst, or on flat terminals.
/// (Moved verbatim from the former `colors.rs::profile_token_pop`.)
fn token_pop_for(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
) -> Option<TokenPop> {
    if vm.life_profile.calm_mode
        || vm.life_profile.burst_level <= 0.0
        || matches!(color_capability, ColorCapability::Flat)
    {
        return None;
    }
    compute_token_pop(vm.last_feed_pulse_at, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::animator::{compute_shimmer_role, compute_twinkle};
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;

    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn from_vm_reproduces_shimmer_and_twinkle() {
        let vm = WatchViewModel::fixture();
        let now = fixed_now();
        let species = vm.pet_render.generated_species;
        let fx = EffectState::from_vm(&vm, now, ColorCapability::Truecolor);
        assert_eq!(fx.shimmer_role, compute_shimmer_role(species, now));
        assert_eq!(
            fx.twinkle,
            compute_twinkle(species, now, vm.life_profile.idle.idle_minutes)
        );
    }

    #[test]
    fn flat_capability_suppresses_token_pop() {
        let vm = WatchViewModel::fixture();
        let fx = EffectState::from_vm(&vm, fixed_now(), ColorCapability::Flat);
        assert!(
            fx.token_pop.is_none(),
            "Flat capability must suppress token-pop (matches profile_token_pop gate)"
        );
    }

    #[test]
    fn calm_mode_suppresses_token_pop() {
        let mut vm = WatchViewModel::fixture();
        vm.life_profile.calm_mode = true;
        let fx = EffectState::from_vm(&vm, fixed_now(), ColorCapability::Truecolor);
        assert!(fx.token_pop.is_none(), "calm_mode must suppress token-pop");
    }

    #[test]
    fn zero_burst_suppresses_token_pop() {
        let mut vm = WatchViewModel::fixture();
        vm.life_profile.calm_mode = false;
        vm.life_profile.burst_level = 0.0;
        let fx = EffectState::from_vm(&vm, fixed_now(), ColorCapability::Truecolor);
        assert!(
            fx.token_pop.is_none(),
            "burst_level <= 0 must suppress token-pop"
        );
    }
}
