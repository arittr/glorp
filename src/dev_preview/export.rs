use crate::dev_preview::frame::{escape_html, PreviewCell, PreviewFrame};
use crate::error::Result;
use crate::tui::component::PreviewLayout;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const PRODUCER: &str = "glorp-dev-preview";
pub const SCHEMA_VERSION: u32 = 2;
const PREVIEW_GRID_DEFAULT_FG: &str = "#e6edf3";
const PREVIEW_GRID_DEFAULT_BG: &str = "#0d1117";

#[derive(Debug, Clone, Serialize)]
pub struct PreviewManifest {
    pub schema_version: u32,
    pub producer: &'static str,
    pub glorp_version: &'static str,
    pub generated_at: String,
    pub scenarios: Vec<PreviewScenario>,
    pub artifacts: Vec<PreviewArtifact>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewScenario {
    pub id: String,
    pub kind: PreviewScenarioKind,
    pub title: String,
    pub intent: String,
    pub dimensions: PreviewDimensions,
    pub files: PreviewScenarioFiles,
    pub inputs: BTreeMap<String, Value>,
    pub review_prompts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewScenarioKind {
    Watch,
    PetMatrix,
    HabitatProps,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewDimensions {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewScenarioFiles {
    pub text: PathBuf,
    pub cells: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<PathBuf>,
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
    Layout,
    Html,
    Review,
    Asset,
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

pub fn write_layout_json(path: &Path, layout: &PreviewLayout) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(layout)?)?;
    Ok(())
}

pub fn write_manifest(path: &Path, manifest: &PreviewManifest) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(manifest)?)?;
    Ok(())
}

pub fn write_review_markdown(path: &Path, manifest: &PreviewManifest) -> Result<()> {
    let mut markdown = String::from("# Glorp Preview Review\n\n");
    markdown.push_str(&format!(
        "Generated `{}` with Glorp `{}`.\n\n",
        manifest.generated_at, manifest.glorp_version
    ));

    if manifest.scenarios.is_empty() {
        markdown.push_str("No preview scenarios were generated.\n");
    } else {
        markdown.push_str("## Scenarios\n\n");
        for scenario in &manifest.scenarios {
            markdown.push_str(&format!(
                "### {}\n\n{}\n\n",
                scenario.title, scenario.intent
            ));
            markdown.push_str(&format!(
                "- ID: `{}`\n- Kind: `{}`\n- Size: `{}x{}`\n- Text: `{}`\n- Cells: `{}`\n",
                scenario.id,
                scenario_kind_label(scenario.kind),
                scenario.dimensions.width,
                scenario.dimensions.height,
                scenario.files.text.display(),
                scenario.files.cells.display()
            ));
            if let Some(layout) = &scenario.files.layout {
                markdown.push_str(&format!("- Layout: `{}`\n", layout.display()));
            }
            markdown.push('\n');
            markdown.push_str("Review prompts:\n");
            for prompt in &scenario.review_prompts {
                markdown.push_str(&format!("- {prompt}\n"));
            }
            markdown.push('\n');
        }
    }
    fs::write(path, markdown)?;
    Ok(())
}

fn scenario_kind_label(kind: PreviewScenarioKind) -> &'static str {
    match kind {
        PreviewScenarioKind::Watch => "watch",
        PreviewScenarioKind::PetMatrix => "pet-matrix",
        PreviewScenarioKind::HabitatProps => "habitat-props",
    }
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
    html.push_str(r#"<div class="preview-grid-shell">"#);
    html.push_str(&format!(
        r#"<div class="preview-grid" style="--cols: {}; --rows: {}">"#,
        frame.width, frame.height
    ));

    for cell in frame.cells.iter().filter(|cell| !cell.continuation) {
        html.push_str(&render_cell_html(cell));
    }

    html.push_str("</div>");
    if frame.layout.is_some() {
        html.push_str(&format!(
            r#"<div class="layout-overlay" data-layout-for="{}" hidden></div>"#,
            escape_html(&frame.id)
        ));
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
            layout: None,
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
            layout: None,
        }
    }

    fn sample_manifest() -> PreviewManifest {
        PreviewManifest {
            schema_version: SCHEMA_VERSION,
            producer: PRODUCER,
            glorp_version: "0.1.0",
            generated_at: "2026-05-12T00:00:00Z".to_string(),
            scenarios: vec![PreviewScenario {
                id: "frame-one".to_string(),
                kind: PreviewScenarioKind::Watch,
                title: "Frame One".to_string(),
                intent: "Exercise a sample watch preview.".to_string(),
                dimensions: PreviewDimensions {
                    width: 2,
                    height: 2,
                },
                files: PreviewScenarioFiles {
                    text: PathBuf::from("frames/frame-one.txt"),
                    cells: PathBuf::from("frames/frame-one.cells.json"),
                    layout: None,
                },
                inputs: BTreeMap::from([(
                    "fixed_now".to_string(),
                    Value::String("2026-05-12T00:00:00Z".to_string()),
                )]),
                review_prompts: vec!["Check sample geometry.".to_string()],
            }],
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
                PreviewArtifact {
                    id: "preview-css".to_string(),
                    title: "Preview CSS".to_string(),
                    artifact_type: ArtifactType::Asset,
                    path: PathBuf::from("assets/preview.css"),
                    width: None,
                    height: None,
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
        let manifest = sample_manifest();

        write_manifest(&path, &manifest).unwrap();

        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(json["schema_version"], 2);
        assert_eq!(json["producer"], "glorp-dev-preview");
        assert_eq!(json["scenarios"][0]["kind"], "watch");
        assert_eq!(json["scenarios"][0]["dimensions"]["width"], 2);
        assert_eq!(
            json["scenarios"][0]["files"]["cells"],
            "frames/frame-one.cells.json"
        );
        assert_eq!(json["artifacts"][0]["type"], "text");
        assert_eq!(json["artifacts"][1]["type"], "html");
        assert_eq!(json["artifacts"][2]["type"], "asset");
    }

    #[test]
    fn review_markdown_lists_scenario_prompts_from_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review.md");

        write_review_markdown(&path, &sample_manifest()).unwrap();

        let markdown = fs::read_to_string(path).unwrap();
        assert!(markdown.contains("## Scenarios"));
        assert!(markdown.contains("Frame One"));
        assert!(markdown.contains("Exercise a sample watch preview."));
        assert!(markdown.contains("- Kind: `watch`"));
        assert!(markdown.contains("- Text: `frames/frame-one.txt`"));
        assert!(markdown.contains("- Cells: `frames/frame-one.cells.json`"));
        assert!(markdown.contains("Review prompts:"));
        assert!(markdown.contains("- Check sample geometry."));
    }
}
