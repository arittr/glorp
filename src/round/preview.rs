use crate::dev_preview::frame::{mark_continuations, PreviewCell, PreviewFrame};
use crate::round::layout::{
    layout_round_scene, RoundAnchorKind, RoundAperture, RoundRenderCapabilities, RoundSceneLayout,
};
use crate::round::model::{derive_round_scene_model, RoundHelperHealth, RoundSceneModel};
use crate::tui::room::RoomDialectKey;
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
    paint_room(&mut cells, width, &scene, &layout, capabilities.truecolor);
    paint_pet_art(&mut cells, width, vm, &layout, capabilities.truecolor);
    paint_halo(&mut cells, width, &scene, &layout, capabilities.truecolor);
    mark_continuations(&mut cells, width);
    PreviewFrame {
        id: id.into(),
        title: title.into(),
        width,
        height,
        cells,
        layout: None,
        extra_inputs: Default::default(),
    }
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

fn paint_room(
    cells: &mut [PreviewCell],
    width: u16,
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
    truecolor: bool,
) {
    for y in 0..layout.aperture.height {
        for x in 0..layout.aperture.width {
            let idx = y as usize * width as usize + x as usize;
            if cells[idx].outside_aperture {
                continue;
            }
            // Sparse grid: texture glyphs appear on roughly 1 in 5 cells so the
            // room reads as ambient grain rather than solid fill.
            if (x + y) % 5 == 0 {
                let (symbol, fg) = room_symbol(scene, truecolor);
                set_cell(cells, width, x as i32, y as i32, symbol, Some(fg));
            }
        }
    }
}

fn paint_pet_art(
    cells: &mut [PreviewCell],
    width: u16,
    vm: &WatchViewModel,
    layout: &RoundSceneLayout,
    truecolor: bool,
) {
    let art_width = vm
        .pet_art
        .iter()
        .map(|line| {
            line.chars()
                .map(|ch| Line::from(ch.to_string()).width())
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0) as i32;
    let art_height = vm.pet_art.len() as i32;
    let start_x = layout.pet_anchor.x.round() as i32 - art_width / 2;
    let start_y = layout.pet_anchor.y.round() as i32 - art_height / 2;
    for (row, line) in vm.pet_art.iter().enumerate() {
        let mut col = 0i32;
        for ch in line.chars() {
            let display_width = Line::from(ch.to_string()).width() as i32;
            if ch != ' ' {
                let fg = if truecolor { "#efebe4" } else { "white" };
                set_cell(
                    cells,
                    width,
                    start_x + col,
                    start_y + row as i32,
                    ch.to_string(),
                    Some(fg.to_string()),
                );
            }
            col += display_width;
        }
    }
}

fn paint_halo(
    cells: &mut [PreviewCell],
    width: u16,
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
    truecolor: bool,
) {
    if scene.halo.helper_health == RoundHelperHealth::Trouble {
        for anchor in layout
            .halo_anchors
            .iter()
            .filter(|a| a.kind == RoundAnchorKind::HelperTrouble)
        {
            let fg = if truecolor { "#f0a646" } else { "yellow" };
            set_cell(
                cells,
                width,
                anchor.x.round() as i32,
                anchor.y.round() as i32,
                "!".to_string(),
                Some(fg.to_string()),
            );
        }
    }
}

fn room_symbol(scene: &RoundSceneModel, truecolor: bool) -> (String, String) {
    // Dialect-to-color mapping: Glitch reads as cool circuitry, Crystal as warm
    // prisms, and the default biome as neutral stone. Flat mode uses named ANSI
    // colors so the preview still renders without truecolor support.
    let fg = palette_color(scene.room.dialect, truecolor);
    match scene.room.dialect {
        RoomDialectKey::Glitch => ("#".to_string(), fg),
        RoomDialectKey::Crystal => ("^".to_string(), fg),
        _ => (".".to_string(), fg),
    }
}

fn palette_color(dialect: RoomDialectKey, truecolor: bool) -> String {
    if truecolor {
        match dialect {
            RoomDialectKey::Glitch => "#86d9ef".to_string(),
            RoomDialectKey::Crystal => "#b39dff".to_string(),
            _ => "#808080".to_string(),
        }
    } else {
        match dialect {
            RoomDialectKey::Glitch => "cyan".to_string(),
            RoomDialectKey::Crystal => "magenta".to_string(),
            _ => "gray".to_string(),
        }
    }
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
