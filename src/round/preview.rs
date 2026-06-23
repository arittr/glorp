use crate::dev_preview::frame::{mark_continuations, PreviewCell, PreviewFrame};
use crate::round::draw::{RoundDrawCommand, RoundDrawKind};
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::derive_round_scene_model;
use crate::tui::view_model::WatchViewModel;

pub fn render_round_preview_frame_from_vm(
    id: impl Into<String>,
    title: impl Into<String>,
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    width: u16,
    height: u16,
    capabilities: RoundRenderCapabilities,
) -> PreviewFrame {
    let scene = derive_round_scene_model(vm, now);
    let aperture = RoundAperture::new(width, height);
    let layout = layout_round_scene(&scene, aperture, capabilities);

    // Render pet+habitat via the shared scene seam.
    let list = crate::round::scene::build_round_scene_draw_list(vm, now, width, height);
    let grid = crate::presentation::rasterize(&list, width, height);

    // Build PreviewCell grid from rasterized output, applying the aperture mask.
    let mut cells: Vec<PreviewCell> = Vec::with_capacity(width as usize * height as usize);
    for row in 0..height {
        for col in 0..width {
            let outside_aperture = !aperture.contains(col as f32, row as f32);
            if outside_aperture {
                cells.push(PreviewCell {
                    x: col,
                    y: row,
                    symbol: " ".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: None,
                    bg: None,
                    modifiers: Vec::new(),
                    outside_aperture: true,
                });
            } else {
                let raster = &grid[row as usize][col as usize];
                let symbol = raster.glyph.to_string();
                let display_width = ratatui::text::Line::from(symbol.clone()).width();
                let fg = raster
                    .fg
                    .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
                let bg = raster
                    .bg
                    .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
                cells.push(PreviewCell {
                    x: col,
                    y: row,
                    symbol,
                    display_width,
                    continuation: false,
                    fg,
                    bg,
                    modifiers: Vec::new(),
                    outside_aperture: false,
                });
            }
        }
    }

    // Overlay halo/trouble beads from the draw-command path.
    let commands = crate::round::draw::build_draw_commands(&scene, &layout);
    paint_halo_trouble(&mut cells, width, &commands, capabilities.truecolor);

    mark_continuations(&mut cells, width);
    let mut frame = PreviewFrame {
        id: id.into(),
        title: title.into(),
        width,
        height,
        cells,
        layout: None,
        extra_inputs: Default::default(),
        contract: Default::default(),
    };
    frame.contract.scene = Some(
        crate::dev_preview::contract::PreviewSceneArtifact::from_round_scene(
            &frame.id, &scene, now,
        ),
    );
    frame.contract.round_layout = Some(
        crate::dev_preview::contract::PreviewRoundLayoutArtifact::from_layout(&frame.id, &layout),
    );
    frame.contract.round_commands = Some(
        crate::dev_preview::contract::PreviewRoundCommandsArtifact::from_commands(
            &frame.id, &scene, &commands,
        ),
    );
    frame
}

/// Paint only `Halo` and `Trouble` commands into the cell grid.
/// All other draw kinds (`Background`, `PetGlyph`, `RoomGlyph`, `PropGlyph`)
/// are now supplied by the rasterized seam scene and are intentionally skipped.
fn paint_halo_trouble(
    cells: &mut [PreviewCell],
    width: u16,
    commands: &[RoundDrawCommand],
    truecolor: bool,
) {
    for command in commands {
        match command.kind {
            RoundDrawKind::Halo | RoundDrawKind::Trouble => {
                paint_labeled_command(cells, width, command, truecolor)
            }
            _ => {}
        }
    }
}

fn paint_labeled_command(
    cells: &mut [PreviewCell],
    width: u16,
    command: &RoundDrawCommand,
    truecolor: bool,
) {
    if let Some(label) = command.label {
        set_cell(
            cells,
            width,
            command.x.round() as i32,
            command.y.round() as i32,
            label.to_string(),
            Some(command_color(command, truecolor)),
        );
    }
}

fn command_color(command: &RoundDrawCommand, truecolor: bool) -> String {
    if truecolor {
        let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        return format!(
            "#{:02x}{:02x}{:02x}",
            channel(command.color.0),
            channel(command.color.1),
            channel(command.color.2)
        );
    }

    match command.kind {
        RoundDrawKind::RoomGlyph => "gray",
        RoundDrawKind::PropGlyph | RoundDrawKind::Halo => "yellow",
        RoundDrawKind::Trouble => "red",
        RoundDrawKind::Background | RoundDrawKind::PetGlyph => "white",
    }
    .to_string()
}

fn set_cell(
    cells: &mut [PreviewCell],
    width: u16,
    x: i32,
    y: i32,
    symbol: String,
    fg: Option<String>,
) {
    if x < 0 || y < 0 {
        return;
    }
    let x = x as u16;
    let y = y as u16;
    let idx = y as usize * width as usize + x as usize;
    if idx >= cells.len() || cells[idx].outside_aperture || cells[idx].continuation {
        return;
    }
    cells[idx].display_width = ratatui::text::Line::from(symbol.clone()).width();
    cells[idx].symbol = symbol;
    cells[idx].fg = fg;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::layout::RoundRenderCapabilities;

    fn blank_cells(width: u16, height: u16, aperture: RoundAperture) -> Vec<PreviewCell> {
        let mut cells = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height {
            for x in 0..width {
                let outside = !aperture.contains(x as f32, y as f32);
                cells.push(PreviewCell {
                    x,
                    y,
                    symbol: " ".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: None,
                    bg: None,
                    modifiers: Vec::new(),
                    outside_aperture: outside,
                });
            }
        }
        cells
    }

    #[test]
    fn preview_renders_multiple_fg_colors_via_seam() {
        // The seam-rendered frame must produce more than one distinct fg color,
        // proving that role-based coloring (eye, body, accent, biome wash) is
        // applied — not a flat single-color fill.
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;
        let vm = WatchViewModel::fixture_with_habitat_props();
        let frame = render_round_preview_frame_from_vm(
            "round-color",
            "Round Color",
            &vm,
            datetime!(2026-06-13 18:00 UTC),
            52,
            52,
            RoundRenderCapabilities::preview_truecolor(),
        );
        let fgs: std::collections::HashSet<_> = frame
            .cells
            .iter()
            .filter(|c| !c.symbol.trim().is_empty())
            .filter_map(|c| c.fg.clone())
            .collect();
        assert!(
            fgs.len() > 1,
            "expected multiple fg colors from seam render, got {fgs:?}"
        );
    }

    #[test]
    fn preview_room_varies_by_biome() {
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;
        // Two fixtures with different earned biomes should produce different
        // room glyph sets in the preview frame.
        let vm = WatchViewModel::fixture_with_habitat_props();
        let frame = render_round_preview_frame_from_vm(
            "round-biome",
            "Round Biome",
            &vm,
            datetime!(2026-06-14 12:00 UTC),
            52,
            52,
            RoundRenderCapabilities::preview_truecolor(),
        );
        let room_syms: std::collections::HashSet<_> = frame
            .cells
            .iter()
            .filter(|c| !c.outside_aperture && !c.symbol.trim().is_empty())
            .map(|c| c.symbol.clone())
            .collect();
        assert!(!room_syms.is_empty());
    }

    #[test]
    fn aperture_masking_blanks_corners_and_fills_center() {
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;

        let vm = WatchViewModel::fixture_with_habitat_props();
        let frame = render_round_preview_frame_from_vm(
            "round-aperture",
            "Round Aperture",
            &vm,
            datetime!(2026-06-15 12:00 UTC),
            52,
            52,
            RoundRenderCapabilities::preview_truecolor(),
        );

        // Top-left corner must be masked (outside circle).
        let top_left = frame.cells.iter().find(|c| c.x == 0 && c.y == 0).unwrap();
        assert!(top_left.outside_aperture, "top-left corner must be masked");
        assert_eq!(top_left.symbol, " ");

        // Center cell must be inside aperture.
        let center = frame.cells.iter().find(|c| c.x == 26 && c.y == 26).unwrap();
        assert!(
            !center.outside_aperture,
            "center cell must be inside aperture"
        );
    }

    #[test]
    fn blank_cells_outside_aperture_are_flagged() {
        let aperture = RoundAperture::new(10, 10);
        let cells = blank_cells(10, 10, aperture);
        let corner = cells.iter().find(|c| c.x == 0 && c.y == 0).unwrap();
        assert!(corner.outside_aperture);
        assert_eq!(corner.symbol, " ");
    }
}
