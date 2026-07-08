use crate::dev_preview::frame::{escape_html, PreviewCell, PreviewFrame};
use crate::dev_preview::strips::PreviewStripBundle;
use crate::error::Result;
use crate::tui::component::PreviewLayout;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const PRODUCER: &str = "glorp-dev-preview";
pub const SCHEMA_VERSION: u32 = 7;
const PREVIEW_GRID_DEFAULT_FG: &str = "#e6edf3";
const PREVIEW_GRID_DEFAULT_BG: &str = "#0d1117";

#[derive(Debug, Clone, Serialize)]
pub struct PreviewManifest {
    pub schema_version: u32,
    pub producer: &'static str,
    pub glorp_version: &'static str,
    pub generated_at: String,
    pub scenarios: Vec<PreviewScenario>,
    pub strips: Vec<PreviewStrip>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<PreviewRoundMetadata>,
    pub review_prompts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRoundMetadata {
    pub target_renderer: &'static str,
    pub aperture: PreviewRoundAperture,
    pub privacy: PreviewRoundPrivacy,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRoundAperture {
    pub shape: &'static str,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub safe_inner_radius: f32,
    pub transparent_outside_aperture: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewRoundPrivacy {
    pub source_names_visible: bool,
    pub exact_counts_visible: bool,
    pub diagnostic_text_visible: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewScenarioKind {
    Watch,
    PetMatrix,
    HabitatProps,
    Round,
    TankLife,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_text: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_masked_text: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hud: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tank_life: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStrip {
    pub id: String,
    pub kind: PreviewStripKind,
    pub title: String,
    pub intent: String,
    pub dimensions: PreviewDimensions,
    pub target_id: String,
    pub playback: PreviewPlayback,
    pub inputs: BTreeMap<String, Value>,
    pub frames: Vec<PreviewStripFrame>,
    pub review_prompts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewStripKind {
    SceneMoment,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewPlayback {
    pub starts_paused: bool,
    pub frame_duration_ms: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStripFrame {
    pub index: u16,
    pub phase: String,
    pub elapsed_ms: u16,
    pub files: PreviewStripFrameFiles,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStripFrameFiles {
    pub text: PathBuf,
    pub cells: PathBuf,
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
    Scene,
    Hud,
    TankLife,
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

pub fn write_room_text_frame(path: &Path, frame: &PreviewFrame, target_id: &str) -> Result<()> {
    write_room_text_frame_masked(path, frame, target_id, &[])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewMaskRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl PreviewMaskRect {
    fn contains(self, col: u16, row: u16) -> bool {
        col >= self.x
            && col < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }
}

pub fn write_room_text_frame_masked(
    path: &Path,
    frame: &PreviewFrame,
    target_id: &str,
    masks: &[PreviewMaskRect],
) -> Result<()> {
    let layout = frame
        .layout
        .as_ref()
        .expect("frame should have layout for masked room text");
    let target = layout
        .targets
        .get(target_id)
        .unwrap_or_else(|| panic!("layout should contain {target_id}"));
    let mut text = String::new();
    for row in target.y..target.y + target.height {
        for col in target.x..target.x + target.width {
            if masks.iter().any(|mask| mask.contains(col, row)) {
                text.push(' ');
                continue;
            }
            let cell = frame
                .cells
                .iter()
                .find(|cell| cell.x == col && cell.y == row)
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

pub fn write_json_artifact<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
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
            if let Some(room_text) = &scenario.files.room_text {
                markdown.push_str(&format!("- Room: `{}`\n", room_text.display()));
            }
            if let Some(masked_room) = &scenario.files.room_masked_text {
                markdown.push_str(&format!("- Masked room: `{}`\n", masked_room.display()));
            }
            if let Some(scene) = &scenario.files.scene {
                markdown.push_str(&format!("- Scene: `{}`\n", scene.display()));
            }
            if let Some(hud) = &scenario.files.hud {
                markdown.push_str(&format!("- HUD: `{}`\n", hud.display()));
            }
            if let Some(tank_life) = &scenario.files.tank_life {
                markdown.push_str(&format!("- Tank life: `{}`\n", tank_life.display()));
            }
            markdown.push('\n');
            markdown.push_str("Review prompts:\n");
            for prompt in &scenario.review_prompts {
                markdown.push_str(&format!("- {prompt}\n"));
            }
            markdown.push('\n');
        }
    }

    if !manifest.strips.is_empty() {
        markdown.push_str("## Animation Strips\n\n");
        for strip in &manifest.strips {
            markdown.push_str(&format!("### {}\n\n{}\n\n", strip.title, strip.intent));
            markdown.push_str(&format!(
                "- ID: `{}`\n- Kind: `{}`\n- Target: `{}`\n- Size: `{}x{}`\n- Frames: `{}`\n\n",
                strip.id,
                strip_kind_label(strip.kind),
                strip.target_id,
                strip.dimensions.width,
                strip.dimensions.height,
                strip.frames.len()
            ));
            markdown.push_str("Review prompts:\n");
            for prompt in &strip.review_prompts {
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
        PreviewScenarioKind::Round => "round",
        PreviewScenarioKind::TankLife => "tank-life",
    }
}

fn strip_kind_label(kind: PreviewStripKind) -> &'static str {
    match kind {
        PreviewStripKind::SceneMoment => "scene-moment",
    }
}

pub fn write_index_html(
    path: &Path,
    frames: &[PreviewFrame],
    strips: &[PreviewStripBundle],
    generated_at: &str,
) -> Result<()> {
    let template = include_str!("assets/preview.html");
    let frames_html = frames.iter().map(render_frame_html).collect::<String>();
    let strips_html = strips.iter().map(render_strip_html).collect::<String>();
    let html = template
        .replace("{{GENERATED_AT}}", &escape_html(generated_at))
        .replace("{{FRAMES}}", &format!("{frames_html}{strips_html}"));
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
    html.push_str(&render_frame_artifact_links(frame));
    html.push_str(r#"<div class="preview-grid-shell">"#);
    html.push_str(&render_grid_html(frame));
    if frame.layout.is_some() {
        html.push_str(&format!(
            r#"<div class="layout-overlay" data-layout-for="{}" hidden></div>"#,
            escape_html(&frame.id)
        ));
    }
    html.push_str("</div></article>");
    html
}

fn render_frame_artifact_links(frame: &PreviewFrame) -> String {
    let mut links = vec![
        format!(
            r#"<a href="{}">text</a>"#,
            escape_html(&format!("frames/{}.txt", frame.id))
        ),
        format!(
            r#"<a href="{}">cells</a>"#,
            escape_html(&format!("frames/{}.cells.json", frame.id))
        ),
    ];
    if frame.layout.is_some() {
        links.push(format!(
            r#"<a href="{}">layout</a>"#,
            escape_html(&format!("frames/{}.layout.json", frame.id))
        ));
    }
    if frame
        .layout
        .as_ref()
        .is_some_and(|layout| layout.targets.contains_key("watch.room.effect"))
    {
        links.push(format!(
            r#"<a href="{}">room</a>"#,
            escape_html(&format!("frames/{}.room.txt", frame.id))
        ));
    }
    if has_masked_room_artifact(&frame.id) {
        links.push(format!(
            r#"<a href="{}">masked room</a>"#,
            escape_html(&format!("frames/{}.room-masked.txt", frame.id))
        ));
    }
    if frame.contract.scene.is_some() {
        links.push(format!(
            r#"<a href="{}">scene</a>"#,
            escape_html(&format!("frames/{}.scene.json", frame.id))
        ));
    }
    if frame.contract.hud.is_some() {
        links.push(format!(
            r#"<a href="{}">hud</a>"#,
            escape_html(&format!("frames/{}.hud.json", frame.id))
        ));
    }
    if frame.contract.tank_life.is_some() {
        links.push(format!(
            r#"<a href="{}">tank life</a>"#,
            escape_html(&format!("frames/{}.tank-life.json", frame.id))
        ));
    }

    format!(r#"<p class="frame-links">{}</p>"#, links.join(" · "))
}

pub(crate) fn has_masked_room_artifact(frame_id: &str) -> bool {
    matches!(
        frame_id,
        "watch-species-dialect-glitch"
            | "watch-species-dialect-crystal"
            | "watch-species-dialect-glitch-flat"
            | "watch-species-dialect-crystal-flat"
    )
}

fn render_grid_html(frame: &PreviewFrame) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        r#"<div class="preview-grid" style="--cols: {}; --rows: {}">"#,
        frame.width, frame.height
    ));

    for cell in frame.cells.iter().filter(|cell| !cell.continuation) {
        html.push_str(&render_cell_html(cell));
    }

    html.push_str("</div>");
    html
}

fn render_strip_html(strip: &PreviewStripBundle) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        r#"<article class="strip" data-strip-id="{}" data-frame-index="0" data-frame-count="{}" data-frame-duration="{}">"#,
        escape_html(&strip.manifest.id),
        strip.frames.len(),
        strip.manifest.playback.frame_duration_ms
    ));
    html.push_str(&format!("<h2>{}</h2>", escape_html(&strip.manifest.title)));
    html.push_str(r#"<div class="strip-controls">"#);
    html.push_str(r#"<button type="button" data-strip-prev>Prev</button>"#);
    html.push_str(r#"<button type="button" data-strip-play aria-pressed="false">Play</button>"#);
    html.push_str(r#"<button type="button" data-strip-next>Next</button>"#);
    html.push_str(r#"</div>"#);
    for (index, frame) in strip.frames.iter().enumerate() {
        html.push_str(&format!(
            r#"<div class="strip-frame" data-strip-frame="{}"{}>"#,
            escape_html(&frame.id),
            if index == 0 { "" } else { " hidden" }
        ));
        html.push_str(&render_grid_html(frame));
        html.push_str("</div>");
    }
    html.push_str("</article>");
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
                    outside_aperture: false,
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
                    outside_aperture: false,
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
                    outside_aperture: false,
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
                    outside_aperture: false,
                },
            ],
            layout: None,
            extra_inputs: BTreeMap::new(),
            contract: crate::dev_preview::contract::PreviewFrameContract::default(),
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
                    outside_aperture: false,
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
                    outside_aperture: false,
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
                    outside_aperture: false,
                },
            ],
            layout: None,
            extra_inputs: BTreeMap::new(),
            contract: crate::dev_preview::contract::PreviewFrameContract::default(),
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
                dimensions: PreviewDimensions { width: 2, height: 2 },
                files: PreviewScenarioFiles {
                    text: PathBuf::from("frames/frame-one.txt"),
                    cells: PathBuf::from("frames/frame-one.cells.json"),
                    layout: None,
                    room_text: None,
                    room_masked_text: None,
                    scene: None,
                    hud: None,
                    tank_life: None,
                },
                inputs: BTreeMap::from([(
                    "fixed_now".to_string(),
                    Value::String("2026-05-12T00:00:00Z".to_string()),
                )]),
                round: None,
                review_prompts: vec!["Check sample geometry.".to_string()],
            }],
            strips: vec![],
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

        write_index_html(&path, &[sample_frame()], &[], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains(r#"class="preview-grid" style="--cols: 2; --rows: 2""#));
        assert!(html.contains("grid-column: 1; grid-row: 1; color: #ffeeaa"));
        assert!(html.contains("grid-column: 2; grid-row: 2"));
    }

    #[test]
    fn html_export_spans_wide_cells_and_skips_continuations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");

        write_index_html(&path, &[wide_frame()], &[], "2026-05-12T00:00:00Z").unwrap();

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

        write_index_html(&path, &[frame], &[], "2026-05-12T00:00:00Z").unwrap();

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

        write_index_html(&path, &[frame], &[], "2026-05-12T00:00:00Z").unwrap();

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

        write_index_html(&path, &[frame], &[], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains("color: #0d1117; background-color: #e6edf3"));
    }

    #[test]
    fn html_export_escapes_cell_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");

        write_index_html(&path, &[sample_frame()], &[], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains("Frame &lt;One&gt;"));
        assert!(html.contains("&lt;"));
        assert!(html.contains("&amp;"));
        assert!(html.contains("&quot;"));
        assert!(!html.contains(">Frame <One><"));
    }

    #[test]
    fn masked_room_text_export_replaces_masked_cells_with_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("room-masked.txt");
        let mut frame = sample_frame();
        frame.layout = Some(crate::tui::component::PreviewLayout {
            schema_version: 2,
            frame_id: "frame-one".to_string(),
            mode: "wide".to_string(),
            frame: crate::tui::component::PreviewRect { x: 0, y: 0, width: 2, height: 2 },
            content: crate::tui::component::PreviewRect { x: 0, y: 0, width: 2, height: 2 },
            components: BTreeMap::new(),
            targets: BTreeMap::from([(
                "watch.room.effect".to_string(),
                crate::tui::component::PreviewTarget {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                    owner: "watch.pet".to_string(),
                    role: "RoomEffect".to_string(),
                    clip: crate::tui::component::PreviewRect { x: 0, y: 0, width: 2, height: 2 },
                    z: 5,
                    layer: "room-background".to_string(),
                    cell_count: None,
                },
            )]),
            decisions: vec![],
        });

        write_room_text_frame_masked(
            &path,
            &frame,
            "watch.room.effect",
            &[PreviewMaskRect { x: 0, y: 0, width: 1, height: 2 }],
        )
        .unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), " <\n \"\n");
    }

    #[test]
    fn manifest_has_versioned_producer_and_artifact_types() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let manifest = sample_manifest();

        write_manifest(&path, &manifest).unwrap();

        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(json["schema_version"], 7);
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

    #[test]
    fn review_markdown_lists_room_artifacts_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review.md");
        let mut manifest = sample_manifest();
        manifest.scenarios[0].files.room_text = Some(PathBuf::from("frames/frame-one.room.txt"));
        manifest.scenarios[0].files.room_masked_text =
            Some(PathBuf::from("frames/frame-one.room-masked.txt"));

        write_review_markdown(&path, &manifest).unwrap();

        let markdown = fs::read_to_string(path).unwrap();
        assert!(markdown.contains("- Room: `frames/frame-one.room.txt`"));
        assert!(markdown.contains("- Masked room: `frames/frame-one.room-masked.txt`"));
    }

    #[test]
    fn html_export_links_frame_source_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.html");

        write_index_html(&path, &[sample_frame()], &[], "2026-05-12T00:00:00Z").unwrap();

        let html = fs::read_to_string(path).unwrap();
        assert!(html.contains(r#"href="frames/frame-one.txt""#));
        assert!(html.contains(r#"href="frames/frame-one.cells.json""#));
    }
}
