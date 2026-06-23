use glorp::round::draw::{build_draw_commands, RoundDrawKind};
use glorp::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use glorp::round::model::derive_round_scene_model;
use glorp::round::preview::render_round_preview_frame_from_vm;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

/// The preview renders pet cells via the seam — non-blank glyphs must be present.
#[test]
fn round_preview_renders_pet_cells_via_seam() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let now = datetime!(2026-06-15 12:00 UTC);

    let frame = render_round_preview_frame_from_vm(
        "round-test",
        "Round Test",
        &vm,
        now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    );
    let non_blank: usize = frame
        .cells
        .iter()
        .filter(|cell| !cell.outside_aperture && !cell.symbol.trim().is_empty())
        .count();

    assert!(
        non_blank >= 5,
        "expected ≥5 non-blank cells from seam render, got {non_blank}"
    );
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

/// Halo commands are still overlaid from the draw-command path; room/prop/pet come from the seam.
#[test]
fn round_preview_paints_halo_command_glyphs() {
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

    // Only Halo is still overlay-painted from commands; room/prop/pet come from the seam.
    let halo_command = commands
        .iter()
        .find(|command| command.kind == RoundDrawKind::Halo);

    if let Some(command) = halo_command {
        if let Some(label) = command.label {
            let cell = frame
                .cells
                .iter()
                .find(|cell| {
                    cell.x == command.x.round() as u16 && cell.y == command.y.round() as u16
                })
                .unwrap_or_else(|| panic!("expected cell for Halo command"));

            assert_eq!(
                cell.symbol,
                label.to_string(),
                "preview should paint Halo command glyph at ({}, {})",
                command.x,
                command.y
            );
        }
    }
}
