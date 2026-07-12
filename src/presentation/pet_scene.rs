use crate::presentation::EffectState;
use crate::tui::room::RoomLifeProfile;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

/// Viewport-agnostic semantic container describing the "what" of a pet scene:
/// which effects are active and what the room looks like — derived from
/// `WatchViewModel` and the current instant.
///
/// The reacted life profile is NOT included here because it depends on
/// `compact` (a viewport-side concern: `area.width <= 72 || area.height <= 24`).
/// It is computed inline in `PetPanel::render` where the real area is known.
#[derive(Debug, Clone)]
pub struct PetSceneModel {
    pub effects: EffectState,
    pub room: RoomLifeProfile,
}

impl PetSceneModel {
    /// Build the semantic scene model from the watch view model, current instant,
    /// and color capability. Reproduces verbatim the inline derivations from
    /// `PetPanel::render`:
    ///
    /// - `room`: `derive_room_life_profile(vm, now)`
    /// - `effects`: `EffectState::from_vm(vm, now, color_capability)`
    pub fn build(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
        color_capability: ColorCapability,
    ) -> PetSceneModel {
        let room = super::companion_scene::input::derive_room_profile(vm, now);
        let effects = EffectState::from_vm(vm, now, color_capability);

        PetSceneModel { effects, room }
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
}
