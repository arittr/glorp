const APP_SOURCE: &str = include_str!("../src/companion/app.rs");

use glorp::companion::app::{smooth_appkit_paint_schedule, SmoothAppKitPaintStep};
use glorp::round::depth::{CompanionDepthComposition, CompanionGaugeLane, PetStatisticsOrder};

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source marker {start}"));
    let tail = &source[start_index..];
    let end_offset = tail
        .find(end)
        .unwrap_or_else(|| panic!("missing source marker {end}"));
    &tail[..end_offset]
}

#[test]
fn draw_scene_consumes_only_the_last_prepared_frame() {
    let body = source_between(APP_SOURCE, "\nfn draw_scene(", "\nfn paint_prepared_frame(");
    assert!(body.contains("state.last_good_frame.as_ref()"));
    assert!(body.contains("paint_prepared_frame(bounds, frame)"));
    assert!(
        !body.contains("last_good_frame.clone()"),
        "draw_scene must not deep-clone the prepared scene"
    );
    for forbidden in [
        "prepare_companion_frame(",
        "prepare_current_frame_from_state(",
        "build_round_scene_draw_list(",
        "try_build_round_smooth_scene_plan(",
        "companion_hud_text(",
        "SmoothReviewFrameSample {",
    ] {
        assert!(
            !body.contains(forbidden),
            "draw_scene must not call {forbidden}"
        );
    }
}

#[test]
fn smooth_appkit_prepared_schedule_crosses_statistics_at_effective_pet_depth() {
    let behind = [
        SmoothAppKitPaintStep::WorldBeforeStatistics,
        SmoothAppKitPaintStep::PetFront,
        SmoothAppKitPaintStep::StatisticsEcho,
        SmoothAppKitPaintStep::StatisticsPrimary,
        SmoothAppKitPaintStep::StatisticsInteraction,
        SmoothAppKitPaintStep::Foreground,
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Pace),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Daily),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Xp),
        SmoothAppKitPaintStep::StatusTrouble,
        SmoothAppKitPaintStep::Dim,
    ];
    let in_front = [
        SmoothAppKitPaintStep::WorldBeforeStatistics,
        SmoothAppKitPaintStep::StatisticsEcho,
        SmoothAppKitPaintStep::StatisticsPrimary,
        SmoothAppKitPaintStep::PetFront,
        SmoothAppKitPaintStep::StatisticsInteraction,
        SmoothAppKitPaintStep::Foreground,
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Pace),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Daily),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Xp),
        SmoothAppKitPaintStep::StatusTrouble,
        SmoothAppKitPaintStep::Dim,
    ];
    let interacting = [
        SmoothAppKitPaintStep::WorldBeforeStatistics,
        SmoothAppKitPaintStep::StatisticsEcho,
        SmoothAppKitPaintStep::PetFront,
        SmoothAppKitPaintStep::StatisticsPrimary,
        SmoothAppKitPaintStep::StatisticsInteraction,
        SmoothAppKitPaintStep::Foreground,
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Pace),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Daily),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Xp),
        SmoothAppKitPaintStep::StatusTrouble,
        SmoothAppKitPaintStep::Dim,
    ];

    let just_in_front = f32::from_bits(0.72_f32.to_bits() + 1);
    for (depth, expected_order, expected_schedule) in [
        (-1.0, PetStatisticsOrder::BehindStatistics, behind),
        (0.0, PetStatisticsOrder::BehindStatistics, behind),
        (0.72, PetStatisticsOrder::BehindStatistics, interacting),
        (
            just_in_front,
            PetStatisticsOrder::InFrontOfStatistics,
            in_front,
        ),
        (1.0, PetStatisticsOrder::InFrontOfStatistics, in_front),
    ] {
        let composition = CompanionDepthComposition::resolve(depth).unwrap();
        assert_eq!(composition.pet_statistics_order, expected_order);
        assert_eq!(
            smooth_appkit_paint_schedule(composition),
            expected_schedule,
            "effective depth {depth} produced the wrong AppKit schedule"
        );
    }
}

#[test]
fn smooth_statistics_interaction_uses_prepared_effective_depth() {
    let just_in_front = f32::from_bits(0.72_f32.to_bits() + 1);
    for (depth, expected_reveal) in [(0.64, 0.0), (0.68, 0.5), (0.72, 1.0), (just_in_front, 1.0)] {
        let composition = CompanionDepthComposition::resolve(depth).unwrap();
        let _schedule = smooth_appkit_paint_schedule(composition);
        assert!(
            (composition.statistics_interaction.reveal_mix - expected_reveal).abs()
                <= f32::EPSILON * 8.0,
            "effective depth {depth} carried the wrong reveal mix"
        );
    }
}

#[test]
fn ui_tick_owns_preparation_and_smooth_uses_the_fallible_planner() {
    let tick = source_between(
        APP_SOURCE,
        "\nfn ui_tick()",
        "\nfn prepare_current_frame_from_state()",
    );
    let prepare_current = source_between(
        APP_SOURCE,
        "\nfn prepare_current_frame_from_state()",
        "\nfn record_frame_preparation_error(",
    );
    let prepare_frame = source_between(
        APP_SOURCE,
        "\nfn prepare_companion_frame(",
        "\nstruct AppState",
    );

    assert!(tick.contains("prepare_current_frame_from_state()"));
    let hidden_return = tick
        .find("if !companion_view_is_visible()")
        .expect("hidden early return exists");
    let animate = tick
        .find("animate_pet()")
        .expect("visible animation exists");
    let prepare = tick
        .find("prepare_current_frame_from_state()")
        .expect("visible preparation exists");
    assert!(hidden_return < animate && hidden_return < prepare);
    assert!(prepare_current.contains("prepare_companion_frame("));
    assert!(prepare_current.contains("state.last_good_frame = Some(frame)"));
    assert!(prepare_frame.contains("try_build_round_smooth_scene_plan_with_options("));
}

#[cfg(feature = "retained-renderer")]
#[test]
fn live_scene_tick_preparation_stays_out_of_the_smooth_plan_path() {
    let prepare_scene = source_between(
        APP_SOURCE,
        "\nfn prepare_scene_runtime_tick(",
        "\nfn service_scene_runtime(",
    );
    assert!(prepare_scene.contains("CompanionSceneSnapshot::project_with_input_and_options("));
    for forbidden in [
        "derive_round_scene_model(",
        "prepare_companion_frame(",
        "PreparedRendererFrame::Smooth",
        "SmoothCompanionScenePlan",
        "try_build_round_smooth_scene_plan",
        "SceneDrawList",
    ] {
        assert!(
            !prepare_scene.contains(forbidden),
            "live scene preparation must not depend on {forbidden}"
        );
    }
}

#[cfg(feature = "retained-renderer")]
#[test]
fn draw_scene_remains_prepared_frame_and_fallback_only_with_scene_live_enabled() {
    let body = source_between(APP_SOURCE, "\nfn draw_scene(", "\nfn paint_prepared_frame(");
    for forbidden in [
        "CompanionSceneSnapshot",
        "service_scene_runtime(",
        "present_active_scene(",
        "activate_candidate(",
    ] {
        assert!(
            !body.contains(forbidden),
            "draw_scene must not own scene runtime work: {forbidden}"
        );
    }
}
