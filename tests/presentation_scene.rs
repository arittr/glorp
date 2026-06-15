use glorp::presentation::privacy::{PresentationSurface, PrivacyProjection};
use glorp::presentation::scene::PresentationScene;
use glorp::presentation::target::SurfaceTargetId;
use glorp::tui::view_model::{EventView, SourceStatus, WatchViewModel};
use time::macros::datetime;

#[test]
fn presentation_scene_glanceable_projection_excludes_private_runtime_text() {
    let mut vm = WatchViewModel::fixture_with_events();
    vm.helper_status = "helper failed in /Users/drew/private/project".into();
    vm.errors = vec!["prompt response tool payload /tmp/raw.log".into()];
    vm.recent_events = vec![EventView {
        timestamp: "11:22".into(),
        kind: glorp::tui::style::LogKind::Usage,
        text: "opened /Users/drew/private/project/src/main.rs".into(),
    }];
    vm.source_breakdown[0].display_name = "client-secret-project".into();
    vm.source_health[0].status = SourceStatus::Diagnostic;
    vm.source_health[0].diagnostic_message = Some("secret helper path".into());
    vm.today_effective_tokens = 123_456.0;

    let scene = PresentationScene::from_watch_view_model(
        &vm,
        datetime!(2026-06-15 12:00 UTC),
        PresentationSurface::RoundCompanion,
    );
    let debug = format!("{scene:?}").to_ascii_lowercase();

    for forbidden in [
        "/users/drew",
        "/tmp/",
        "prompt",
        "response",
        "tool payload",
        "client-secret-project",
        "123456",
        "secret helper path",
    ] {
        assert!(
            !debug.contains(forbidden),
            "scene leaked {forbidden}: {debug}"
        );
    }
}

#[test]
fn presentation_scene_glanceable_projection_redacts_stable_runtime_ids() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.pet_render.seed = "client-secret-seed".into();

    let scene = PresentationScene::from_watch_view_model(
        &vm,
        datetime!(2026-06-15 12:00 UTC),
        PresentationSurface::RoundCompanion,
    );
    let debug = format!("{scene:?}").to_ascii_lowercase();

    for forbidden in [
        "client-secret-seed",
        "codex_signal_lamp",
        "token_pebble_25k",
    ] {
        assert!(
            !debug.contains(forbidden),
            "scene leaked stable runtime id {forbidden}: {debug}"
        );
    }
}

#[test]
fn presentation_targets_are_owned_ids_not_watch_paths() {
    let pet = SurfaceTargetId::new("pet.art");
    let room = SurfaceTargetId::new("room.effect");

    assert_eq!(pet.as_str(), "pet.art");
    assert_eq!(room.as_str(), "room.effect");
    assert!(
        !pet.as_str().starts_with("watch."),
        "presentation IDs must not encode watch target paths"
    );
}

#[test]
fn privacy_projection_is_surface_specific() {
    let watch = PrivacyProjection::for_surface(PresentationSurface::WatchTui);
    let round = PrivacyProjection::for_surface(PresentationSurface::RoundCompanion);
    let menubar = PrivacyProjection::for_surface(PresentationSurface::MenubarPopover);

    assert!(watch.source_names_visible);
    assert!(watch.exact_counts_visible);
    assert!(!round.source_names_visible);
    assert!(!round.exact_counts_visible);
    assert!(menubar.source_names_visible);
    assert!(menubar.exact_counts_visible);
}
