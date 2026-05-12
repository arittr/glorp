use crate::dev_preview::frame::{escape_html, PreviewCell, PreviewFrame};
use crate::error::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const PRODUCER: &str = "glorp-dev-preview";
pub const SCHEMA_VERSION: u32 = 1;
const PREVIEW_GRID_DEFAULT_FG: &str = "#e6edf3";
const PREVIEW_GRID_DEFAULT_BG: &str = "#0d1117";

#[derive(Debug, Clone, Serialize)]
pub struct PreviewManifest {
    pub schema_version: u32,
    pub producer: &'static str,
    pub glorp_version: &'static str,
    pub generated_at: String,
    pub artifacts: Vec<PreviewArtifact>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewArtifact {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub path: PathBuf,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactType {
    Text,
    Cells,
    Html,
    Review,
}

pub fn write_text_frame(path: &Path, frame: &PreviewFrame) -> Result<()> {
    let mut text = String::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            let cell = frame
                .cells
                .iter()
                .find(|cell| cell.x == x && cell.y == y)
                .expect("frame should contain each coordinate");
            if cell.continuation {
                continue;
            }
            text.push_str(&cell.symbol);
        }
        text.push('\n');
    }
    fs::write(path, text)?;
    Ok(())
}

pub fn write_cells_json(path: &Path, frame: &PreviewFrame) -> Result<()> {
    #[derive(Serialize)]
    struct CellsExport<'a> {
        width: u16,
        height: u16,
        cells: &'a [PreviewCell],
    }

    let export = CellsExport {
        width: frame.width,
        height: frame.height,
        cells: &frame.cells,
    };
    fs::write(path, serde_json::to_string_pretty(&export)?)?;
    Ok(())
}

pub fn write_manifest(path: &Path, manifest: &PreviewManifest) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(manifest)?)?;
    Ok(())
}

pub fn write_review_markdown(path: &Path, frames: &[PreviewFrame]) -> Result<()> {
    let mut markdown = String::from("# Glorp Preview Review\n\n");
    if frames.is_empty() {
        markdown.push_str("No preview frames were generated.\n");
    } else {
        markdown.push_str("## Frames\n\n");
        for frame in frames {
            markdown.push_str(&format!(
                "- {} (`{}`x`{}`): `frames/{}.txt`\n",
                frame.title, frame.width, frame.height, frame.id
            ));
        }
    }
    fs::write(path, markdown)?;
    Ok(())
}

pub fn write_index_html(path: &Path, frames: &[PreviewFrame], generated_at: &str) -> Result<()> {
    let template = include_str!("assets/preview.html");
    let frames_html = frames.iter().map(render_frame_html).collect::<String>();
    let html = template
        .replace("{{GENERATED_AT}}", &escape_html(generated_at))
        .replace("{{FRAMES}}", &frames_html);
    fs::write(path, html)?;
    Ok(())
}

pub fn copy_assets(out_dir: &Path) -> Result<()> {
    let assets_dir = out_dir.join("assets");
    fs::create_dir_all(&assets_dir)?;
    fs::write(
        assets_dir.join("preview.css"),
        include_str!("assets/preview.css"),
    )?;
    fs::write(
        assets_dir.join("preview.js"),
        include_str!("assets/preview.js"),
    )?;
    Ok(())
}

fn render_frame_html(frame: &PreviewFrame) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        r#"<article class="frame" data-frame-id="{}">"#,
        escape_html(&frame.id)
    ));
    html.push_str(&format!("<h2>{}</h2>", escape_html(&frame.title)));
    html.push_str(&format!(
        r#"<p class="frame-meta">{} x {} cells</p>"#,
        frame.width, frame.height
    ));
    html.push_str(&format!(
        r#"<div class="preview-grid" style="--cols: {}; --rows: {}">"#,
        frame.width, frame.height
    ));

    for cell in frame.cells.iter().filter(|cell| !cell.continuation) {
        html.push_str(&render_cell_html(cell));
    }

    html.push_str("</div></article>");
    html
}

fn render_cell_html(cell: &PreviewCell) -> String {
    let mut styles = Vec::new();
    if cell.display_width > 1 {
        styles.push(format!(
            "grid-column: {} / span {}",
            cell.x + 1,
            cell.display_width
        ));
    } else {
        styles.push(format!("grid-column: {}", cell.x + 1));
    }
    styles.push(format!("grid-row: {}", cell.y + 1));

    let reversed = cell.modifiers.contains(&"reversed");
    let source_fg = cell.fg.as_deref().filter(|color| is_hex_color(color));
    let source_bg = cell.bg.as_deref().filter(|color| is_hex_color(color));
    if reversed {
        styles.push(format!(
            "color: {}",
            source_bg.unwrap_or(PREVIEW_GRID_DEFAULT_BG)
        ));
        styles.push(format!(
            "background-color: {}",
            source_fg.unwrap_or(PREVIEW_GRID_DEFAULT_FG)
        ));
    } else {
        if let Some(fg) = source_fg {
            styles.push(format!("color: {fg}"));
        }
        if let Some(bg) = source_bg {
            styles.push(format!("background-color: {bg}"));
        }
    }

    let mut text_decorations = Vec::new();
    for modifier in &cell.modifiers {
        match *modifier {
            "bold" => styles.push("font-weight: 700".to_string()),
            "dim" => styles.push("opacity: 0.7".to_string()),
            "italic" => styles.push("font-style: italic".to_string()),
            "underlined" => text_decorations.push("underline"),
            "crossed-out" => text_decorations.push("line-through"),
            "hidden" => styles.push("visibility: hidden".to_string()),
            "reversed" => {}
            _ => {}
        }
    }
    if !text_decorations.is_empty() {
        styles.push(format!("text-decoration: {}", text_decorations.join(" ")));
    }

    format!(
        r#"<span class="cell" style="{}">{}</span>"#,
        styles.join("; "),
        escape_html(&cell.symbol)
    )
}

fn is_hex_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color.chars().skip(1).all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_preview::frame::{PreviewCell, PreviewFrame};
    use std::fs;

    fn sample_frame() -> PreviewFrame {
        PreviewFrame {
            id: "frame-one".to_string(),
            title: "Frame <One>".to_string(),
            width: 2,
            height: 2,
            cells: vec![
                PreviewCell {
                    x: 0,
                    y: 0,
                    symbol: "A".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: Some("#ffeeaa".to_string()),
                    bg: None,
                    modifiers: vec!["bold"],
                },
                PreviewCell {
                    x: 1,
                    y: 0,
                    symbol: "<".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: None,
                    bg: Some("#000000".to_string()),
                    modifiers: vec![],
                },
                PreviewCell {
                    x: 0,
                    y: 1,
                    symbol: "&".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: None,
                    bg: None,
                    modifiers: vec![],
                },
                PreviewCell {
                    x: 1,
                    y: 1,
                    symbol: "\"".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: None,
                    bg: None,
                    modifiers: vec![],
                },
            ],
        }
    }

    fn wide_frame() -> PreviewFrame {
        PreviewFrame {
            id: "wide".to_string(),
            title: "Wide".to_string(),
            width: 3,
            height: 1,
            cells: vec![
                PreviewCell {
                    x: 0,
                    y: 0,
                    symbol: "界".to_string(),
                    display_width: 2,
                    continuation: false,
                    fg: None,
                    bg: None,
                    modifiers: vec![],
                },
                PreviewCell {
                    x: 1,
                    y: 0,
                    symbol: " ".to_string(),
                    display_width: 1,
                    continuation: true,
                    fg: None,
                    bg: None,
                    modifiers: vec![],
                },
                PreviewCell {
                    x: 2,
                    y: 0,
                    symbol: "A".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: None,
                    bg: None,
                    modifiers: vec![],
                },
            ],
        }
    }

    #[test]
    fn text_export_preserves_terminal_geometry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.txt");

        write_text_frame(&path, &sample_frame()).unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "A<\n&\"\n");
    }

    #[test]
    fn text_export_skips_wide_cell_continuations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.txt");

        write_text_frame(&path, &wide_frame()).unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "界A\n");
    }

    #[test]
    fn html_export_uses_fixed_cell_grid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");

        write_index_html(&path, &[sample_frame()], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains(r#"class="preview-grid" style="--cols: 2; --rows: 2""#));
        assert!(html.contains("grid-column: 1; grid-row: 1; color: #ffeeaa"));
        assert!(html.contains("grid-column: 2; grid-row: 2"));
    }

    #[test]
    fn html_export_spans_wide_cells_and_skips_continuations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");

        write_index_html(&path, &[wide_frame()], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert_eq!(html.matches(r#"<span class="cell""#).count(), 2);
        assert!(html.contains("grid-column: 1 / span 2; grid-row: 1"));
        assert!(html.contains("grid-column: 3; grid-row: 1"));
        assert!(html.contains(">界</span>"));
    }

    #[test]
    fn html_export_combines_text_decoration_modifiers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");
        let mut frame = sample_frame();
        frame.cells[0].modifiers = vec!["underlined", "crossed-out"];

        write_index_html(&path, &[frame], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains("text-decoration: underline line-through"));
        assert!(!html.contains("text-decoration: underline; text-decoration"));
    }

    #[test]
    fn html_export_reverses_foreground_and_background_colors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");
        let mut frame = sample_frame();
        frame.cells[0].fg = Some("#112233".to_string());
        frame.cells[0].bg = Some("#aabbcc".to_string());
        frame.cells[0].modifiers = vec!["reversed"];

        write_index_html(&path, &[frame], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains("color: #aabbcc; background-color: #112233"));
    }

    #[test]
    fn html_export_reverses_default_colors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");
        let mut frame = sample_frame();
        frame.cells[0].fg = None;
        frame.cells[0].bg = None;
        frame.cells[0].modifiers = vec!["reversed"];

        write_index_html(&path, &[frame], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains("color: #0d1117; background-color: #e6edf3"));
    }

    #[test]
    fn html_export_escapes_cell_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");

        write_index_html(&path, &[sample_frame()], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains("Frame &lt;One&gt;"));
        assert!(html.contains("&lt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&quot;"));
        assert!(!html.contains(">Frame <One><"));
    }

    #[test]
    fn manifest_has_versioned_producer_and_artifact_types() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let manifest = PreviewManifest {
            schema_version: SCHEMA_VERSION,
            producer: PRODUCER,
            glorp_version: "0.1.0",
            generated_at: "2026-05-12T00:00:00Z".to_string(),
            artifacts: vec![
                PreviewArtifact {
                    id: "frame-one".to_string(),
                    title: "Frame One".to_string(),
                    artifact_type: ArtifactType::Text,
                    path: PathBuf::from("frames/frame-one.txt"),
                    width: Some(2),
                    height: Some(2),
                },
                PreviewArtifact {
                    id: "index".to_string(),
                    title: "Index".to_string(),
                    artifact_type: ArtifactType::Html,
                    path: PathBuf::from("index.html"),
                    width: None,
                    height: None,
                },
            ],
        };

        write_manifest(&path, &manifest).unwrap();

        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["producer"], "glorp-dev-preview");
        assert_eq!(json["artifacts"][0]["type"], "text");
        assert_eq!(json["artifacts"][1]["type"], "html");
    }
}
