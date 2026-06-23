use crate::pet::animator::{
    compute_facing, compute_sleep_wander_x, compute_wake_wander_x, compute_wander_position_x,
    lazy_wander_instant,
};
use crate::tui::panels::pet::effective_weekend_softening;
use crate::tui::view_model::WatchViewModel;

const RESONANCE_WANDER_BIAS_CELLS: i16 = 3;

/// Live pet horizontal drift + facing, resolved against `habitat_width`. Pure
/// function of the view model, the frame instant, and the panel width, so any
/// surface (watch, companion) gets identical motion by passing its own width.
pub(crate) fn resolve_wander_offset(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    habitat_width: u16,
) -> (i16, i8) {
    let species = vm.pet_render.generated_species;
    let day = &vm.day_context;
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
        crate::tui::day::resonant_prop_for_day(day, &earned)
    };
    let softening = effective_weekend_softening(day, &vm.life_profile);
    let idle_minutes = vm.life_profile.idle.idle_minutes;
    match (day.asleep, day.sleep_onset_utc, day.wake_resume) {
        (true, Some(onset), _) => (
            compute_sleep_wander_x(habitat_width, species, now, onset, idle_minutes),
            compute_facing(habitat_width, species, onset, idle_minutes),
        ),
        (false, _, Some(resume)) => (
            compute_wake_wander_x(
                habitat_width,
                species,
                now,
                resume.from_eval_utc,
                resume.woke_at_utc,
                idle_minutes,
            ),
            compute_facing(habitat_width, species, now, idle_minutes),
        ),
        _ => {
            let wander_now = lazy_wander_instant(now, day.local_day_started_utc, softening);
            (
                compute_wander_position_x(habitat_width, species, wander_now, idle_minutes)
                    + resonance_wander_bias(resonant_prop.as_ref()),
                compute_facing(habitat_width, species, wander_now, idle_minutes),
            )
        }
    }
}

fn resonance_wander_bias(resonant: Option<&crate::game::habitat::HabitatPropId>) -> i16 {
    let Some(spec) = resonant.and_then(crate::game::habitat::catalog_prop) else {
        return 0;
    };
    use crate::game::habitat::HabitatPropZone;
    let side: i16 = match spec.zone {
        HabitatPropZone::FloorLeft | HabitatPropZone::WallLeft | HabitatPropZone::AirLeft => -1,
        HabitatPropZone::FloorRight | HabitatPropZone::WallRight | HabitatPropZone::AirRight => 1,
        HabitatPropZone::FloorMid | HabitatPropZone::AirMid | HabitatPropZone::Ceiling => 0,
    };
    side * RESONANCE_WANDER_BIAS_CELLS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::animator::{compute_facing, compute_wander_position_x, lazy_wander_instant};
    use crate::tui::view_model::WatchViewModel;

    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn normal_arm_matches_direct_animator_calls() {
        // fixture() is awake with no wake_resume → normal arm.
        // fixture has earned_props: Vec::new(), so resonant_prop is None,
        // and resonance_wander_bias(None) == 0.
        let vm = WatchViewModel::fixture();
        let now = fixed_now();
        let width = 60u16;
        let species = vm.pet_render.generated_species;
        let idle = vm.life_profile.idle.idle_minutes;
        let softening =
            crate::tui::panels::pet::effective_weekend_softening(&vm.day_context, &vm.life_profile);
        let wander_now = lazy_wander_instant(now, vm.day_context.local_day_started_utc, softening);
        // resonant_prop is None (no earned props in fixture) → bias is 0
        let expect_x = compute_wander_position_x(width, species, wander_now, idle)
            + resonance_wander_bias(None);
        let expect_f = compute_facing(width, species, wander_now, idle);

        let (x, f) = resolve_wander_offset(&vm, now, width);
        assert_eq!((x, f), (expect_x, expect_f));
    }

    #[test]
    fn too_narrow_habitat_centers_and_faces_right() {
        // width <= 14 → half_range 0 → wander 0, facing +1 (animator guards)
        let vm = WatchViewModel::fixture();
        let (x, f) = resolve_wander_offset(&vm, fixed_now(), 14);
        assert_eq!((x, f), (0, 1));
    }

    #[test]
    fn resonance_wander_bias_points_toward_the_prop_zone() {
        let planter =
            crate::storage::state::HabitatPropId::new(crate::game::habitat::HEAVY_SESSION_PLANTER);
        let sprout =
            crate::storage::state::HabitatPropId::new(crate::game::habitat::WILT_RECOVERY_SPROUT);
        assert!(
            resonance_wander_bias(Some(&planter)) > 0,
            "right-zone prop pulls right"
        );
        assert!(
            resonance_wander_bias(Some(&sprout)) < 0,
            "left-zone prop pulls left"
        );
        assert_eq!(resonance_wander_bias(None), 0, "no companion, no bias");
    }
}
