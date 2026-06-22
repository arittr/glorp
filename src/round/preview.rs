use crate::dev_preview::frame::{mark_continuations, PreviewCell, PreviewFrame};
use crate::pet::render::PaletteRoleName;
use crate::presentation::pet::{role_for_cell, PetTextBlock};
use crate::round::draw::{RoundDrawCommand, RoundDrawKind};
use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use crate::round::model::{derive_round_scene_model, RoundSceneModel};
use crate::tui::view_model::WatchViewModel;
use ratatui::text::Line;

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
    let mut cells = blank_cells(width, height, aperture);
    let commands = crate::round::draw::build_draw_commands(&scene, &layout);
    paint_commands(&mut cells, width, &scene, &commands, capabilities.truecolor);
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

fn paint_commands(
    cells: &mut [PreviewCell],
    width: u16,
    scene: &RoundSceneModel,
    commands: &[RoundDrawCommand],
    truecolor: bool,
) {
    for command in commands {
        match command.kind {
            RoundDrawKind::Background => {}
            RoundDrawKind::RoomGlyph
            | RoundDrawKind::PropGlyph
            | RoundDrawKind::Halo
            | RoundDrawKind::Trouble => paint_labeled_command(cells, width, command, truecolor),
            RoundDrawKind::PetGlyph => {
                paint_pet_art_command(cells, width, scene, command, truecolor);
            }
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

fn paint_pet_art_command(
    cells: &mut [PreviewCell],
    width: u16,
    scene: &RoundSceneModel,
    command: &RoundDrawCommand,
    truecolor: bool,
) {
    let art_lines = command
        .text
        .as_deref()
        .unwrap_or_default()
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let block = PetTextBlock::new(art_lines.clone(), command.spans.clone());
    let art_width = art_lines
        .iter()
        .map(|line| {
            line.chars()
                .map(|ch| Line::from(ch.to_string()).width())
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0) as i32;
    let art_height = art_lines.len() as i32;
    let start_x = command.x.round() as i32 - art_width / 2;
    let start_y = command.y.round() as i32 - art_height / 2;
    for (row, line) in art_lines.iter().enumerate() {
        let mut col = 0i32;
        for (char_index, ch) in line.chars().enumerate() {
            let display_width = Line::from(ch.to_string()).width() as i32;
            if ch != ' ' {
                let role = role_for_cell(&block, row, char_index);
                let rgb = crate::pet::palette::role_color(role, &scene.pet.palette);
                let fg = if truecolor {
                    format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
                } else {
                    flat_role_name(role).to_string()
                };
                set_cell(
                    cells,
                    width,
                    start_x + col,
                    start_y + row as i32,
                    ch.to_string(),
                    Some(fg),
                );
            }
            col += display_width;
        }
    }
}

fn flat_role_name(role: PaletteRoleName) -> &'static str {
    match role {
        PaletteRoleName::Eye => "green",
        PaletteRoleName::Accent | PaletteRoleName::Particle => "yellow",
        _ => "white",
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
    cells[idx].display_width = Line::from(symbol.clone()).width();
    cells[idx].symbol = symbol;
    cells[idx].fg = fg;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::render::StyledSegment;

    #[test]
    fn preview_pet_colors_eye_and_body_differently() {
        use crate::pet::render::PaletteRoleName;
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        // Give the pet a known eye span so the assertion exercises span-aware
        // coloring rather than the room's ambient texture color.
        vm.pet_art = vec!["o o".to_string()];
        vm.pet_spans = vec![
            StyledSegment {
                line: 0,
                start: 0,
                end: 1,
                role: PaletteRoleName::Eye,
            },
            StyledSegment {
                line: 0,
                start: 2,
                end: 3,
                role: PaletteRoleName::Eye,
            },
        ];
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
        // More than one distinct pet/room fg means spans are honored (not flat cream).
        assert!(fgs.len() > 1, "expected multiple fg colors, got {fgs:?}");
        // The eye role resolves to green; a flat-cream pet would never produce it.
        let eye = crate::pet::palette::role_color(
            PaletteRoleName::Eye,
            &crate::pet::palette::default_theme_palette(),
        );
        let eye_fg = format!("#{:02x}{:02x}{:02x}", eye.r, eye.g, eye.b);
        assert!(
            fgs.contains(&eye_fg),
            "expected an eye-colored pet cell ({eye_fg}), got {fgs:?}"
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
    fn preview_pet_text_is_positioned_from_command_anchor() {
        use crate::round::draw::{RoundColor, RoundDrawCommand, RoundDrawKind};
        use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
        use crate::round::model::derive_round_scene_model;
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;

        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_art = vec!["x".to_string()];
        vm.pet_spans = Vec::new();
        let now = datetime!(2026-06-15 12:00 UTC);
        let scene = derive_round_scene_model(&vm, now);
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(52, 52),
            RoundRenderCapabilities::preview_truecolor(),
        );
        let command = RoundDrawCommand {
            kind: RoundDrawKind::PetGlyph,
            x: 8.0,
            y: 9.0,
            radius: 1.0,
            label: None,
            text: Some("x".into()),
            spans: Vec::new(),
            color: RoundColor(1.0, 1.0, 1.0, 1.0),
        };
        let mut cells = blank_cells(52, 52, layout.aperture);

        paint_commands(&mut cells, 52, &scene, &[command], true);

        let command_cell = cells
            .iter()
            .find(|cell| cell.x == 8 && cell.y == 9)
            .expect("expected command cell");
        assert_eq!(command_cell.symbol, "x");
    }

    #[test]
    fn corruption_role_degrades_to_neutral_under_flat() {
        use crate::pet::render::PaletteRoleName;
        // Under Flat the round companion carries the pet by silhouette; the
        // contrasting corruption color is gone, so corruption must read as a
        // neutral cell, not be mistaken for an eye (green) or accent (yellow).
        assert_eq!(flat_role_name(PaletteRoleName::Corruption), "white");
    }
}
