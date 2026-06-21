#![cfg(feature = "dev-preview")]

use glorp::round::draw::{build_draw_commands, RoundDrawKind};
use glorp::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use glorp::round::model::derive_round_scene_model;
use glorp::round::preview::render_round_preview_frame_from_vm;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

#[test]
fn round_preview_and_draw_commands_share_pet_text() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.pet_art = vec!["ab".to_string(), "cd".to_string()];
    let now = datetime!(2026-06-15 12:00 UTC);
    let scene = derive_round_scene_model(&vm, now);
    let layout = layout_round_scene(
        &scene,
        RoundAperture::new(52, 52),
        RoundRenderCapabilities::preview_truecolor(),
    );
    let commands = build_draw_commands(&scene, &layout);
    let pet = commands
        .iter()
        .find(|command| command.kind == RoundDrawKind::PetGlyph)
        .expect("pet command");

    let frame = render_round_preview_frame_from_vm(
        "round-test",
        "Round Test",
        &vm,
        now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    );
    let visible = frame
        .cells
        .iter()
        .filter(|cell| !cell.outside_aperture && !cell.symbol.trim().is_empty())
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();

    assert_eq!(pet.text.as_deref(), Some("ab\ncd"));
    assert!(visible.contains("ab") || visible.contains("cd"));
}

#[test]
fn round_preview_exposes_command_backed_room_and_halo_glyphs() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.source_health[0].status = glorp::tui::view_model::SourceStatus::Diagnostic;
    let now = datetime!(2026-06-15 12:00 UTC);
    let scene = derive_round_scene_model(&vm, now);
    let layout = layout_round_scene(
        &scene,
        RoundAperture::new(52, 52),
        RoundRenderCapabilities::preview_truecolor(),
    );
    let commands = build_draw_commands(&scene, &layout);

    assert!(commands
        .iter()
        .any(|command| command.kind == RoundDrawKind::RoomGlyph));
    assert!(commands
        .iter()
        .any(|command| command.kind == RoundDrawKind::Trouble));

    let frame = render_round_preview_frame_from_vm(
        "round-trouble",
        "Round Trouble",
        &vm,
        now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    );
    assert!(frame.cells.iter().any(|cell| cell.symbol == "!"));
}

#[test]
fn round_preview_paints_concrete_command_glyphs() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let now = datetime!(2026-06-15 12:00 UTC);
    let scene = derive_round_scene_model(&vm, now);
    let layout = layout_round_scene(
        &scene,
        RoundAperture::new(52, 52),
        RoundRenderCapabilities::preview_truecolor(),
    );
    let commands = build_draw_commands(&scene, &layout);
    let frame = render_round_preview_frame_from_vm(
        "round-command-cells",
        "Round Command Cells",
        &vm,
        now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    );

    for kind in [
        RoundDrawKind::RoomGlyph,
        RoundDrawKind::PropGlyph,
        RoundDrawKind::Halo,
    ] {
        let command = commands
            .iter()
            .find(|command| command.kind == kind)
            .unwrap_or_else(|| panic!("expected {kind:?} command"));
        let label = command
            .label
            .unwrap_or_else(|| panic!("expected {kind:?} command label"));
        let cell = frame
            .cells
            .iter()
            .find(|cell| cell.x == command.x.round() as u16 && cell.y == command.y.round() as u16)
            .unwrap_or_else(|| panic!("expected cell for {kind:?}"));

        assert_eq!(
            cell.symbol,
            label.to_string(),
            "preview should paint concrete {kind:?} command at ({}, {})",
            command.x,
            command.y
        );
    }
}
