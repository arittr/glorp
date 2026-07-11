const APP_SOURCE: &str = include_str!("../src/companion/app.rs");

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
    assert!(prepare_current.contains("prepare_companion_frame("));
    assert!(prepare_current.contains("state.last_good_frame = Some(frame)"));
    assert!(prepare_frame.contains("try_build_round_smooth_scene_plan_with_options("));
}
