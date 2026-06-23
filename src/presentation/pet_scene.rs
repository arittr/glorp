use crate::presentation::EffectState;
use crate::tui::life::{build_prop_reactions, PetLifeProfile};
use crate::tui::panels::pet::apply_resonance_reaction;
use crate::tui::room::{derive_room_life_profile, RoomLifeProfile};
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

/// Viewport-agnostic semantic container describing the "what" of a pet scene:
/// which effects are active, what the room looks like, and the reacted life
/// profile — all derived from `WatchViewModel` and the current instant.
///
/// Pixel/placement passes are deferred to Plan 05; this container only holds
/// the semantic derivations that `PetPanel::render` previously computed inline.
#[derive(Debug, Clone)]
pub struct PetSceneModel {
    pub effects: EffectState,
    pub room: RoomLifeProfile,
    pub life: PetLifeProfile,
}

impl PetSceneModel {
    /// Build the semantic scene model from the watch view model, current instant,
    /// and color capability. Reproduces verbatim the inline derivations from
    /// `PetPanel::render`:
    ///
    /// - `room`: `derive_room_life_profile(vm, now)`
    /// - `effects`: `EffectState::from_vm(vm, now, color_capability)`
    /// - `life`: `vm.life_profile` → `build_prop_reactions(..., compact=true)`
    ///   → `apply_resonance_reaction(..., resonant_prop)`
    ///
    /// `compact=true` matches `PetPanel::render` which derives
    /// `compact = area.width <= 72 || area.height <= 24`; since the pet panel
    /// column is 48 wide (≤ 72), this expression is always `true` in practice.
    pub fn build(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
        color_capability: ColorCapability,
    ) -> PetSceneModel {
        let room = derive_room_life_profile(vm, now);
        let effects = EffectState::from_vm(vm, now, color_capability);

        // Derive the resonant prop the same way the panel render does: map the
        // earned-prop views into storage EarnedHabitatProp structs, then ask
        // resonant_prop_for_day.
        let resonant_prop = {
            let earned: Vec<crate::storage::state::EarnedHabitatProp> = vm
                .habitat
                .earned_props
                .iter()
                .map(|prop| crate::storage::state::EarnedHabitatProp {
                    id: prop.id.clone(),
                    earned_at: prop.earned_at,
                    source: prop.source.clone(),
                })
                .collect();
            crate::tui::day::resonant_prop_for_day(&vm.day_context, &earned)
        };

        // Replicate the step-E life pipeline from PetPanel::render (~lines 257-264):
        //   1. collect earned prop ids
        //   2. build_prop_reactions with compact=true (pet panel width <= 72 always)
        //   3. apply_resonance_reaction with the day-resonant prop
        let earned_prop_ids = vm
            .habitat
            .earned_props
            .iter()
            .map(|prop| prop.id.clone())
            .collect::<Vec<_>>();
        let life = build_prop_reactions(vm.life_profile.clone(), &earned_prop_ids, true);
        let life = apply_resonance_reaction(life, resonant_prop.as_ref());

        PetSceneModel {
            effects,
            room,
            life,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;

    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn build_reproduces_room_and_effects() {
        let vm = WatchViewModel::fixture();
        let now = fixed_now();
        let m = PetSceneModel::build(&vm, now, ColorCapability::Truecolor);
        assert_eq!(m.room, crate::tui::room::derive_room_life_profile(&vm, now));
        assert_eq!(
            m.effects,
            EffectState::from_vm(&vm, now, ColorCapability::Truecolor)
        );
    }

    #[test]
    fn build_reproduces_reacted_life_profile() {
        let vm = WatchViewModel::fixture();
        let now = fixed_now();
        let m = PetSceneModel::build(&vm, now, ColorCapability::Truecolor);

        // Replicate the exact step-E pipeline from PetPanel::render:
        let resonant_prop = {
            let earned: Vec<crate::storage::state::EarnedHabitatProp> = vm
                .habitat
                .earned_props
                .iter()
                .map(|prop| crate::storage::state::EarnedHabitatProp {
                    id: prop.id.clone(),
                    earned_at: prop.earned_at,
                    source: prop.source.clone(),
                })
                .collect();
            crate::tui::day::resonant_prop_for_day(&vm.day_context, &earned)
        };
        let earned_prop_ids = vm
            .habitat
            .earned_props
            .iter()
            .map(|prop| prop.id.clone())
            .collect::<Vec<_>>();
        let expected_life = build_prop_reactions(vm.life_profile.clone(), &earned_prop_ids, true);
        let expected_life = apply_resonance_reaction(expected_life, resonant_prop.as_ref());

        assert_eq!(m.life, expected_life);
    }
}
