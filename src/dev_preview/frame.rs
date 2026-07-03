use crate::dev_preview::contract::PreviewFrameContract;
use crate::tui::component::PreviewLayout;
use ratatui::{
    buffer::Buffer,
    layout::Position,
    style::{Color, Modifier},
    text::Line,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewFrame {
    pub id: String,
    pub title: String,
    pub width: u16,
    pub height: u16,
    pub cells: Vec<PreviewCell>,
    pub layout: Option<PreviewLayout>,
    #[serde(skip)]
    pub extra_inputs: BTreeMap<String, Value>,
    #[serde(skip)]
    pub contract: PreviewFrameContract,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewCell {
    pub x: u16,
    pub y: u16,
    pub symbol: String,
    pub display_width: usize,
    pub continuation: bool,
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub modifiers: Vec<&'static str>,
    #[serde(skip_serializing_if = "is_false")]
    pub outside_aperture: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn frame_from_buffer(
    id: impl Into<String>,
    title: impl Into<String>,
    buffer: &Buffer,
) -> PreviewFrame {
    let area = buffer.area;
    let mut cells = Vec::with_capacity((area.width as usize) * (area.height as usize));

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[Position::new(area.x + x, area.y + y)];
            let symbol = cell.symbol().to_string();
            let display_width = Line::from(symbol.clone()).width();
            cells.push(PreviewCell {
                x,
                y,
                symbol,
                display_width,
                continuation: false,
                fg: color_to_css(cell.style().fg),
                bg: color_to_css(cell.style().bg),
                modifiers: modifier_names(cell.style().add_modifier),
                outside_aperture: false,
            });
        }
    }

    mark_continuations(&mut cells, area.width);

    PreviewFrame {
        id: id.into(),
        title: title.into(),
        width: area.width,
        height: area.height,
        cells,
        layout: None,
        extra_inputs: BTreeMap::new(),
        contract: PreviewFrameContract::default(),
    }
}

/// Glitch corruption preview fixtures report the protected face cells — the
/// live Eye/Mouth span cells patch selection must never cover — so reviewers
/// can see the safety contract alongside the patch cells. Derived from the
/// rendered spans: the metamorph art carries real face slots at every stage,
/// so there is no static coordinate island to report.
pub fn protected_face_cells_json(spans: &[crate::pet::render::StyledSegment]) -> Value {
    use crate::pet::render::PaletteRoleName;
    let cells: Vec<Value> = spans
        .iter()
        .filter(|span| matches!(span.role, PaletteRoleName::Eye | PaletteRoleName::Mouth))
        .flat_map(|span| {
            let row = span.line;
            (span.start..span.end).map(move |col| serde_json::json!({"row": row, "col": col}))
        })
        .collect();
    Value::Array(cells)
}

pub fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub fn mark_continuations(cells: &mut [PreviewCell], width: u16) {
    for index in 0..cells.len() {
        let display_width = cells[index].display_width;
        if display_width <= 1 {
            continue;
        }

        for offset in 1..display_width {
            let continuation_index = index + offset;
            if continuation_index >= cells.len() {
                break;
            }
            if cells[continuation_index].y != cells[index].y {
                break;
            }
            if cells[continuation_index].x >= width {
                break;
            }
            cells[continuation_index].continuation = true;
        }
    }
}

fn color_to_css(color: Option<Color>) -> Option<String> {
    match color? {
        Color::Reset => None,
        Color::Black => Some("#000000".to_string()),
        Color::Red => Some("#ff0000".to_string()),
        Color::Green => Some("#008000".to_string()),
        Color::Yellow => Some("#ffff00".to_string()),
        Color::Blue => Some("#0000ff".to_string()),
        Color::Magenta => Some("#ff00ff".to_string()),
        Color::Cyan => Some("#00ffff".to_string()),
        Color::Gray => Some("#808080".to_string()),
        Color::DarkGray => Some("#404040".to_string()),
        Color::LightRed => Some("#ff6666".to_string()),
        Color::LightGreen => Some("#66ff66".to_string()),
        Color::LightYellow => Some("#ffff66".to_string()),
        Color::LightBlue => Some("#6666ff".to_string()),
        Color::LightMagenta => Some("#ff66ff".to_string()),
        Color::LightCyan => Some("#66ffff".to_string()),
        Color::White => Some("#ffffff".to_string()),
        Color::Indexed(index) => {
            let (r, g, b) = ansi_256_to_rgb(index);
            Some(format!("#{r:02x}{g:02x}{b:02x}"))
        }
        Color::Rgb(red, green, blue) => Some(format!("#{red:02x}{green:02x}{blue:02x}")),
    }
}

pub(crate) fn ansi_256_to_rgb(index: u8) -> (u8, u8, u8) {
    const SYSTEM: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00),
        (0x80, 0x00, 0x00),
        (0x00, 0x80, 0x00),
        (0x80, 0x80, 0x00),
        (0x00, 0x00, 0x80),
        (0x80, 0x00, 0x80),
        (0x00, 0x80, 0x80),
        (0xc0, 0xc0, 0xc0),
        (0x80, 0x80, 0x80),
        (0xff, 0x00, 0x00),
        (0x00, 0xff, 0x00),
        (0xff, 0xff, 0x00),
        (0x00, 0x00, 0xff),
        (0xff, 0x00, 0xff),
        (0x00, 0xff, 0xff),
        (0xff, 0xff, 0xff),
    ];
    match index {
        0..=15 => SYSTEM[index as usize],
        16..=231 => {
            let i = index - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            (
                steps[(i / 36) as usize],
                steps[((i / 6) % 6) as usize],
                steps[(i % 6) as usize],
            )
        }
        232..=255 => {
            let v = 8 + (index - 232) * 10;
            (v, v, v)
        }
    }
}

fn modifier_names(modifiers: Modifier) -> Vec<&'static str> {
    let mut names = Vec::new();
    for (modifier, name) in [
        (Modifier::BOLD, "bold"),
        (Modifier::DIM, "dim"),
        (Modifier::ITALIC, "italic"),
        (Modifier::UNDERLINED, "underlined"),
        (Modifier::SLOW_BLINK, "slow-blink"),
        (Modifier::RAPID_BLINK, "rapid-blink"),
        (Modifier::REVERSED, "reversed"),
        (Modifier::HIDDEN, "hidden"),
        (Modifier::CROSSED_OUT, "crossed-out"),
    ] {
        if modifiers.contains(modifier) {
            names.push(name);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{
        buffer::Buffer,
        layout::{Position, Rect},
        style::{Color, Modifier, Style},
    };

    #[test]
    fn frame_from_buffer_preserves_dimensions_and_coordinates() {
        let mut buffer = Buffer::empty(Rect::new(3, 4, 2, 2));
        buffer.set_string(3, 4, "ab", Style::default());
        buffer.set_string(3, 5, "cd", Style::default());

        let frame = frame_from_buffer("sample", "Sample", &buffer);

        assert_eq!(frame.id, "sample");
        assert_eq!(frame.title, "Sample");
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.cells.len(), 4);
        assert_eq!(frame.cells[0].x, 0);
        assert_eq!(frame.cells[0].y, 0);
        assert_eq!(frame.cells[0].symbol, "a");
        assert_eq!(frame.cells[3].x, 1);
        assert_eq!(frame.cells[3].y, 1);
        assert_eq!(frame.cells[3].symbol, "d");
    }

    #[test]
    fn frame_from_buffer_exports_style_information() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        buffer
            .cell_mut(Position::new(0, 0))
            .unwrap()
            .set_symbol("G")
            .set_style(
                Style::default()
                    .fg(Color::Rgb(0xff, 0xee, 0xaa))
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            );
        buffer.set_string(1, 0, "界", Style::default().fg(Color::Indexed(42)));

        let frame = frame_from_buffer("styled", "Styled", &buffer);

        let first = &frame.cells[0];
        assert_eq!(first.symbol, "G");
        assert_eq!(first.display_width, 1);
        assert_eq!(first.fg.as_deref(), Some("#ffeeaa"));
        assert_eq!(first.bg.as_deref(), Some("#0000ff"));
        assert_eq!(first.modifiers, vec!["bold", "italic"]);

        assert_eq!(frame.cells[1].display_width, 2);
        assert!(!frame.cells[1].continuation);
        assert!(frame.cells[2].continuation);
        assert_eq!(frame.cells[1].fg.as_deref(), Some("#00d787"));
    }

    #[test]
    fn indexed_color_resolves_to_hex() {
        let css = color_to_css(Some(Color::Indexed(42)));
        assert!(
            css.as_deref()
                .map(|c| c.starts_with('#') && c.len() == 7)
                .unwrap_or(false),
            "indexed color did not resolve to hex: {css:?}"
        );
    }

    #[test]
    fn html_escape_handles_markup_and_quotes() {
        assert_eq!(
            escape_html("<tag data='x' title=\"&\">"),
            "&lt;tag data=&#39;x&#39; title=&quot;&amp;&quot;&gt;"
        );
    }

    #[test]
    fn frame_contract_defaults_empty_and_is_skipped_during_serialization() {
        let buffer = Buffer::empty(Rect::new(0, 0, 1, 1));

        let frame = frame_from_buffer("contract", "Contract", &buffer);

        assert_eq!(
            frame.contract,
            crate::dev_preview::contract::PreviewFrameContract::default()
        );
        let json = serde_json::to_value(&frame).unwrap();
        assert!(json.get("contract").is_none());
        assert!(json.get("extra_inputs").is_none());
    }
}
