# Glorp Preview Lab Slice 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a hidden `glorp dev-preview` command that exports deterministic static TUI preview artifacts for fast design review.

**Architecture:** Add an explicit TUI render context so previews can render with a chosen color capability instead of detecting terminal state at render time. Add a crate-internal preview generator that builds deterministic watch and pet scenarios, captures final `ratatui::Buffer` output, and writes a safe, versioned static artifact bundle. Slice 1 deliberately excludes ANSI recordings, playback, watch-effect animation strips, Gauntlet stories, and the full mood/tick matrix.

**Tech Stack:** Rust, clap, ratatui `TestBackend`/`Buffer`, serde/serde_json, time, existing temp-file test helpers.

---

## Design Constraints From Review

- Hidden command only: `glorp dev-preview` is callable but must not appear in normal help output.
- Default output is `target/glorp-preview`; default scenario is `all`.
- Slice 1 artifacts are static only: `manifest.json`, `review.md`, `index.html`, copied local assets, `.txt`, and `.cells.json`.
- Output safety is part of Slice 1, not follow-up work.
- Watch previews must use fixed time via `build_watch_view_model_at`, not `build_watch_view_model`.
- Color capability must be passed explicitly through layout/panels; no preview path may depend on `ColorCapability::detect()`.
- HTML is a fixed coordinate grid generated from the final buffer cells, not normal browser text flow.
- Pet matrix rendering must reuse the same pet role-to-style mapping as `PetPanel`.
- Scenario fixtures must not read or mutate Drew's real Glorp state.

## File Map

Create:

- `src/tui/render_context.rs`
- `src/commands/dev_preview.rs`
- `src/dev_preview/mod.rs`
- `src/dev_preview/frame.rs`
- `src/dev_preview/export.rs`
- `src/dev_preview/output.rs`
- `src/dev_preview/scenarios.rs`
- `src/dev_preview/watch.rs`
- `src/dev_preview/pets.rs`
- `src/dev_preview/assets/preview.html`
- `src/dev_preview/assets/preview.css`
- `src/dev_preview/assets/preview.js`
- `tests/dev_preview.rs`

Modify:

- `src/tui/mod.rs`
- `src/tui/layout.rs`
- `src/tui/app.rs`
- `src/tui/panels/mod.rs`
- `src/tui/panels/pet.rs`
- `src/tui/panels/vitals.rs`
- `src/tui/panels/spark.rs`
- `src/tui/panels/today.rs`
- `src/tui/panels/feed.rs`
- `src/tui/panels/helpers.rs`
- `src/commands/mod.rs`
- `src/cli.rs`
- `src/lib.rs`
- `tests/cli_smoke.rs`

## Task 1: Thread Explicit Render Context Through TUI Rendering

This removes the main determinism blocker: `render_watch_frame_with_capability` currently accepts a capability but panel rendering still detects terminal capability internally.

- [ ] Record the implementation base before the first code change.

Command:

```bash
git rev-parse HEAD > .git/glorp-preview-slice-1-base
```

- [ ] Add a failing regression test that proves explicit color capability affects rendered styles.

Add to `tests/tui_render.rs`:

```rust
use glorp::commands::watch::WatchViewModel;
use glorp::tui::layout::render_watch_frame_with_capability;
use glorp::tui::style::ColorCapability;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;

fn spark_foregrounds(buffer: &ratatui::buffer::Buffer) -> Vec<ratatui::style::Color> {
    let area = buffer.area;
    let mut colors = Vec::new();

    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &buffer[Position::new(x, y)];
            if cell.symbol() == "█" {
                if let Some(fg) = cell.style().fg {
                    colors.push(fg);
                }
            }
        }
    }

    colors
}

#[test]
fn render_watch_frame_honors_explicit_color_capability() {
    let vm = WatchViewModel::fixture();

    let mut truecolor_terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    truecolor_terminal
        .draw(|frame| {
            render_watch_frame_with_capability(frame, &vm, ColorCapability::Truecolor);
        })
        .unwrap();

    let mut flat_terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    flat_terminal
        .draw(|frame| {
            render_watch_frame_with_capability(frame, &vm, ColorCapability::Flat);
        })
        .unwrap();

    let truecolor = spark_foregrounds(truecolor_terminal.backend().buffer());
    let flat = spark_foregrounds(flat_terminal.backend().buffer());

    assert!(!truecolor.is_empty(), "fixture should render spark bars");
    assert_ne!(truecolor, flat);
}
```

- [ ] Run the focused test and confirm it fails for the expected reason.

Command:

```bash
cargo test render_watch_frame_honors_explicit_color_capability --test tui_render
```

Expected: the test fails because truecolor and flat renders produce the same foreground sequence.

- [ ] Create `src/tui/render_context.rs`.

```rust
use crate::tui::style::ColorCapability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderContext {
    pub color_capability: ColorCapability,
}

impl RenderContext {
    pub const fn new(color_capability: ColorCapability) -> Self {
        Self { color_capability }
    }

    pub fn from_environment() -> Self {
        Self::new(ColorCapability::detect())
    }
}

impl Default for RenderContext {
    fn default() -> Self {
        Self::from_environment()
    }
}
```

- [ ] Export the module from `src/tui/mod.rs`.

```rust
pub mod render_context;
```

- [ ] Change the panel trait in `src/tui/panels/mod.rs`.

```rust
use crate::tui::render_context::RenderContext;

pub trait Panel {
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint;
    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext);
}
```

- [ ] Update every `Panel` implementation to accept `ctx: &RenderContext`.

Exact files:

- `src/tui/panels/pet.rs`
- `src/tui/panels/vitals.rs`
- `src/tui/panels/spark.rs`
- `src/tui/panels/today.rs`
- `src/tui/panels/feed.rs`
- `src/tui/panels/helpers.rs`

Panels that do not use color capability should name the parameter `_ctx`.

- [ ] Replace internal capability detection in `src/tui/panels/vitals.rs`.

Before:

```rust
let capability = ColorCapability::detect();
let lines = build_vitals_lines(vm, capability);
```

After:

```rust
let lines = build_vitals_lines(vm, ctx.color_capability);
```

- [ ] Replace internal capability detection in `src/tui/panels/spark.rs`.

Before:

```rust
let capability = ColorCapability::detect();
let lines = build_spark_lines(vm, capability);
```

After:

```rust
let lines = build_spark_lines(vm, ctx.color_capability);
```

- [ ] Update layout rendering in `src/tui/layout.rs`.

Change the public helper so it builds and passes a render context:

```rust
use crate::tui::render_context::RenderContext;

pub fn render_watch_frame_with_capability(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    capability: ColorCapability,
) {
    let ctx = RenderContext::new(capability);
    render_watch_frame_with_context(frame, vm, &ctx);
}

pub fn render_watch_frame_with_context(
    frame: &mut Frame<'_>,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    let area = frame.area();
    layout_and_render(frame, area, vm, ctx);
}
```

Then thread `ctx` through `layout_and_render`, `render_wide`, `render_compact`, `render_column_with_spacing`, and `render_centered_column`, replacing calls like this:

```rust
panel.render(rect, frame.buffer_mut(), vm);
```

with:

```rust
panel.render(rect, frame.buffer_mut(), vm, ctx);
```

- [ ] Update `src/tui/app.rs` so normal watch mode keeps its current behavior by constructing `RenderContext::new(config.color_capability)` and calling `render_watch_frame_with_context`.

- [ ] Run targeted tests.

Command:

```bash
cargo test render_watch_frame_honors_explicit_color_capability --test tui_render
```

Expected: passes.

- [ ] Run the existing TUI test file.

Command:

```bash
cargo test --test tui_render
```

Expected: passes.

- [ ] Commit this checkpoint.

```bash
git add src/tui tests/tui_render.rs
git commit -m "Thread explicit render context through TUI"
```

## Task 2: Expose Shared Pet Role Styling For Preview Rendering

The preview pet matrix must not invent a second pet-color mapping.

- [ ] Add a focused unit test in `src/tui/panels/pet.rs` proving the exported helper returns the same styles the panel already uses for roles.

```rust
#[test]
fn pet_role_style_maps_eye_role_to_eye_style() {
    let styles = semantic_styles(ColorCapability::Truecolor);
    assert_eq!(
        pet_role_style(PaletteRoleName::Eye, &styles),
        styles.pet_eye
    );
}
```

- [ ] Rename the private `role_style` helper in `src/tui/panels/pet.rs` to `pub(crate) fn pet_role_style`.

```rust
pub(crate) fn pet_role_style(role: PaletteRoleName, styles: &SemanticStyles) -> Style {
    match role {
        PaletteRoleName::Body => styles.pet_body,
        PaletteRoleName::Eye => styles.pet_eye,
        PaletteRoleName::Mouth => styles.pet_mouth,
        PaletteRoleName::Accent => styles.pet_accent,
        PaletteRoleName::Pattern => styles.pet_pattern,
        PaletteRoleName::Particle => styles.pet_particle,
    }
}
```

- [ ] Update all internal calls in `src/tui/panels/pet.rs` from `role_style` to `pet_role_style`.

- [ ] If preview rendering needs styled spans rather than only role-to-style lookup, make the existing role span helper crate-visible instead of copying it.

Use this shape:

```rust
pub(crate) fn pet_role_spans_for_line(
    line: &str,
    line_index: usize,
    spans: &[crate::pet::render::RoleSpan],
    styles: &SemanticStyles,
    eye_override: Option<&str>,
) -> Vec<Span<'static>> {
    // Move the existing private implementation here.
}
```

Then keep `PetPanel` on the same helper.

- [ ] Run the pet panel tests.

Command:

```bash
cargo test pet_role_style_maps_eye_role_to_eye_style
```

Expected: passes.

- [ ] Commit this checkpoint.

```bash
git add src/tui/panels/pet.rs
git commit -m "Share pet role styling for previews"
```

## Task 3: Add Preview Frame Model And Static Exporters

This task creates the artifact format without wiring the CLI yet.

- [ ] Create `src/dev_preview/mod.rs`.

```rust
pub mod export;
pub mod frame;
pub mod output;
pub mod pets;
pub mod scenarios;
pub mod watch;
```

- [ ] Export the module from `src/lib.rs`.

```rust
pub mod dev_preview;
```

- [ ] Create `src/dev_preview/frame.rs`.

Core types:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier};
use ratatui::text::Line;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewFrame {
    pub id: String,
    pub title: String,
    pub width: u16,
    pub height: u16,
    pub cells: Vec<PreviewCell>,
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
}

pub fn frame_from_buffer(id: impl Into<String>, title: impl Into<String>, buffer: &Buffer) -> PreviewFrame {
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
    }
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

fn mark_continuations(cells: &mut [PreviewCell], width: u16) {
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
        Color::Indexed(index) => Some(format!("ansi-{index}")),
        Color::Rgb(red, green, blue) => Some(format!("#{red:02x}{green:02x}{blue:02x}")),
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
```

Keep `color_to_css`, `modifier_names`, and `mark_continuations` private to the module unless another preview module has a direct need for them.

- [ ] Add unit tests in `src/dev_preview/frame.rs`.

Required test names:

- `frame_from_buffer_preserves_dimensions_and_coordinates`
- `frame_from_buffer_exports_style_information`
- `html_escape_handles_markup_and_quotes`

- [ ] Create `src/dev_preview/export.rs`.

Core API:

```rust
use crate::dev_preview::frame::{escape_html, PreviewFrame};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const PRODUCER: &str = "glorp-dev-preview";
pub const SCHEMA_VERSION: u32 = 1;

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

pub fn write_text_frame(path: &Path, frame: &PreviewFrame) -> anyhow::Result<()> {
    let mut text = String::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            let cell = frame
                .cells
                .iter()
                .find(|cell| cell.x == x && cell.y == y)
                .expect("frame should contain each coordinate");
            text.push_str(&cell.symbol);
        }
        text.push('\n');
    }
    std::fs::write(path, text)?;
    Ok(())
}
```

Also add:

- `write_cells_json(path: &Path, frame: &PreviewFrame) -> anyhow::Result<()>`
- `write_manifest(path: &Path, manifest: &PreviewManifest) -> anyhow::Result<()>`
- `write_review_markdown(path: &Path, frames: &[PreviewFrame]) -> anyhow::Result<()>`
- `write_index_html(path: &Path, frames: &[PreviewFrame], generated_at: &str) -> anyhow::Result<()>`
- `copy_assets(out_dir: &Path) -> anyhow::Result<()>`

`write_index_html` must render each frame as positioned spans:

```html
<div class="preview-grid" style="--cols: 120; --rows: 32">
  <span class="cell" style="grid-column: 1; grid-row: 1; color: #ffeeaa">G</span>
</div>
```

The column and row values are one-based CSS grid coordinates from the zero-based cell coordinates.

- [ ] Create `src/dev_preview/assets/preview.html`.

The template must contain these replacement tokens:

- `{{GENERATED_AT}}`
- `{{FRAMES}}`

It should reference only local files:

```html
<link rel="stylesheet" href="assets/preview.css">
<script defer src="assets/preview.js"></script>
```

- [ ] Create `src/dev_preview/assets/preview.css`.

Use a stable monospace grid:

```css
.preview-grid {
  display: grid;
  grid-template-columns: repeat(var(--cols), 0.62em);
  grid-template-rows: repeat(var(--rows), 1.15em);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 13px;
  line-height: 1;
  background: #0d1117;
  color: #e6edf3;
  overflow: auto;
}

.cell {
  white-space: pre;
}
```

- [ ] Create `src/dev_preview/assets/preview.js`.

Slice 1 JS should be tiny and optional. It may add frame filtering by `data-frame-id`, but it must not be required for static rendering.

- [ ] Add exporter unit tests in `src/dev_preview/export.rs`.

Required test names:

- `text_export_preserves_terminal_geometry`
- `html_export_uses_fixed_cell_grid`
- `html_export_escapes_cell_content`
- `manifest_has_versioned_producer_and_artifact_types`

- [ ] Run focused tests.

Command:

```bash
cargo test dev_preview::frame
cargo test dev_preview::export
```

Expected: passes.

- [ ] Commit this checkpoint.

```bash
git add src/lib.rs src/dev_preview
git commit -m "Add static preview artifact exporters"
```

## Task 4: Add Safe Output Bundle Writer

This task enforces the reviewed ownership contract before any CLI writes files.

- [ ] Create `src/dev_preview/output.rs`.

Core constants and API:

```rust
use crate::dev_preview::export::PRODUCER;
use anyhow::{bail, Context};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const OWNERSHIP_MARKER: &str = ".glorp-preview";

pub struct PreparedOutput {
    pub final_dir: PathBuf,
    pub staging_dir: PathBuf,
}

pub fn prepare_output(final_dir: &Path) -> anyhow::Result<PreparedOutput> {
    validate_replace_target(final_dir)?;

    let parent = final_dir.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    let staging_dir = parent.join(format!(
        ".{}.tmp-{}",
        final_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("glorp-preview"),
        std::process::id()
    ));

    if staging_dir.exists() {
        std::fs::remove_dir_all(&staging_dir)?;
    }
    std::fs::create_dir_all(&staging_dir)?;

    Ok(PreparedOutput {
        final_dir: final_dir.to_path_buf(),
        staging_dir,
    })
}

pub fn commit_output(prepared: PreparedOutput) -> anyhow::Result<()> {
    std::fs::write(prepared.staging_dir.join(OWNERSHIP_MARKER), PRODUCER)?;

    if prepared.final_dir.exists() {
        std::fs::remove_dir_all(&prepared.final_dir)?;
    }
    std::fs::rename(&prepared.staging_dir, &prepared.final_dir)
        .with_context(|| format!("failed to replace {}", prepared.final_dir.display()))?;
    Ok(())
}
```

`validate_replace_target` must:

- accept a missing path,
- accept an empty directory,
- accept a non-empty directory only when it has `.glorp-preview` and `manifest.json` with `producer == "glorp-dev-preview"`,
- refuse regular files,
- refuse symlinks via `std::fs::symlink_metadata`,
- refuse non-owned non-empty directories.

- [ ] Add unit tests in `src/dev_preview/output.rs`.

Required test names:

- `prepare_output_allows_missing_directory`
- `prepare_output_allows_empty_directory`
- `prepare_output_allows_owned_preview_directory`
- `prepare_output_refuses_regular_file`
- `prepare_output_refuses_non_preview_directory`
- `prepare_output_refuses_preview_directory_with_wrong_producer`
- `commit_output_replaces_owned_directory`
- `prepare_output_refuses_symlink`

The symlink test should use `#[cfg(unix)]` because Windows symlink privileges vary.

- [ ] Run focused tests.

Command:

```bash
cargo test dev_preview::output
```

Expected: passes.

- [ ] Commit this checkpoint.

```bash
git add src/dev_preview/output.rs
git commit -m "Harden preview output ownership"
```

## Task 5: Wire Hidden CLI Command

This task makes `glorp dev-preview` callable but still allowed to generate only after scenarios are in place.

- [ ] Add CLI tests first.

Modify `tests/cli_smoke.rs`:

```rust
#[test]
fn help_hides_dev_preview_command() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("dev-preview").not());
}
```

Create `tests/dev_preview.rs` with a first smoke test:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn dev_preview_command_is_callable() {
    let dir = tempdir().unwrap();
    let out = dir.path().join("preview");

    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("dev-preview")
        .arg("--out")
        .arg(&out)
        .arg("--scenario")
        .arg("watch")
        .env("GLORP_CONFIG_DIR", dir.path().join("config"))
        .assert()
        .success()
        .stdout(predicate::str::contains(out.display().to_string()));
}
```

This test will fail until the command route and watch scenario exist. Keep it failing only while working this task and Task 6.

- [ ] Modify `src/cli.rs`.

Add imports:

```rust
use std::path::PathBuf;
use clap::ValueEnum;
```

Add enum:

```rust
#[derive(Clone, Debug, ValueEnum)]
pub enum PreviewScenarioArg {
    All,
    Watch,
    Pets,
}
```

Add hidden command:

```rust
#[command(hide = true)]
DevPreview {
    #[arg(long, default_value = "target/glorp-preview")]
    out: PathBuf,

    #[arg(long, value_enum, default_value_t = PreviewScenarioArg::All)]
    scenario: PreviewScenarioArg,
},
```

- [ ] Modify `src/commands/mod.rs`.

```rust
pub mod dev_preview;
```

- [ ] Create `src/commands/dev_preview.rs`.

```rust
use crate::cli::PreviewScenarioArg;
use crate::dev_preview::scenarios::{generate_preview_bundle, PreviewSelection};
use std::path::PathBuf;

pub fn run(out: PathBuf, scenario: PreviewScenarioArg) -> anyhow::Result<()> {
    let selection = match scenario {
        PreviewScenarioArg::All => PreviewSelection::All,
        PreviewScenarioArg::Watch => PreviewSelection::Watch,
        PreviewScenarioArg::Pets => PreviewSelection::Pets,
    };

    generate_preview_bundle(&out, selection)?;
    println!("Wrote Glorp preview bundle to {}", out.display());
    Ok(())
}
```

- [ ] Modify `src/lib.rs` command dispatch.

```rust
Command::DevPreview { out, scenario } => commands::dev_preview::run(out, scenario),
```

- [ ] Do not mark the CLI smoke test passing until Task 6 supplies at least watch generation.

## Task 6: Generate Deterministic Watch Preview Frames

This task makes `--scenario watch` produce Slice 1 watch artifacts.

- [ ] Create `src/dev_preview/scenarios.rs`.

Core shape:

```rust
use crate::dev_preview::export::{
    copy_assets, write_cells_json, write_index_html, write_manifest, write_review_markdown,
    write_text_frame, ArtifactType, PreviewArtifact, PreviewManifest, PRODUCER, SCHEMA_VERSION,
};
use crate::dev_preview::output::{commit_output, prepare_output};
use crate::dev_preview::watch::watch_frames;
use crate::tui::render_context::RenderContext;
use crate::tui::style::ColorCapability;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSelection {
    All,
    Watch,
    Pets,
}

pub struct PreviewRenderContext {
    pub fixed_now: OffsetDateTime,
    pub render: RenderContext,
}

impl PreviewRenderContext {
    pub fn deterministic() -> Self {
        Self {
            fixed_now: OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap(),
            render: RenderContext::new(ColorCapability::Truecolor),
        }
    }
}
```

`generate_preview_bundle` should:

1. call `prepare_output(out)`,
2. create `frames/`, `assets/`, and any scratch directory under staging,
3. collect frames for selected scenarios,
4. write each frame as `frames/<id>.txt` and `frames/<id>.cells.json`,
5. write `index.html`, `review.md`, copied assets, and `manifest.json`,
6. remove scratch data,
7. call `commit_output`.

For this checkpoint, `PreviewSelection::All` should generate the watch frames and `PreviewSelection::Pets` should return a clear error. Task 7 replaces both branches with the final Slice 1 behavior.

- [ ] Create `src/dev_preview/watch.rs`.

Render helper:

```rust
use crate::commands::watch::build_watch_view_model_at;
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::pet::state::PetState;
use crate::tui::layout::render_watch_frame_with_context;
use crate::usage::store::UsageStore;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::Path;

pub fn watch_frames(ctx: &PreviewRenderContext, scratch_dir: &Path) -> anyhow::Result<Vec<PreviewFrame>> {
    std::fs::create_dir_all(scratch_dir)?;

    Ok(vec![
        render_watch_frame("watch-wide-normal", "Watch Wide Normal", 120, 32, ctx, scratch_dir)?,
        render_watch_frame("watch-compact-normal", "Watch Compact Normal", 72, 24, ctx, scratch_dir)?,
    ])
}

fn render_watch_frame(
    id: &str,
    title: &str,
    width: u16,
    height: u16,
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
) -> anyhow::Result<PreviewFrame> {
    let state = seeded_pet_state();
    let usage_path = scratch_dir.join(format!("{id}.sqlite"));
    let usage = seeded_usage_store(&usage_path, ctx.fixed_now)?;
    let vm = build_watch_view_model_at(&state, &usage, ctx.fixed_now)?;

    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| {
        render_watch_frame_with_context(frame, &vm, &ctx.render);
    })?;

    Ok(frame_from_buffer(id, title, terminal.backend().buffer()))
}
```

Implement `seeded_pet_state` and `seeded_usage_store` using existing real domain types. Do not use Drew's default config directory. The SQLite file must live under the staging scratch directory.

- [ ] Add unit tests in `src/dev_preview/watch.rs`.

Required test names:

- `watch_frames_include_wide_and_compact`
- `watch_frames_are_stable_for_fixed_time`

- [ ] Make `build_watch_view_model_at` reachable from `src/dev_preview/watch.rs`.

If it is already `pub(crate)`, no change is required. If the compiler rejects access due to module visibility, change only that function's visibility to `pub(crate)`; do not expose it publicly.

- [ ] Run the watch scenario tests.

Command:

```bash
cargo test dev_preview::watch
```

Expected: passes.

- [ ] Run the CLI smoke test from Task 5.

Command:

```bash
cargo test dev_preview_command_is_callable --test dev_preview
```

Expected: passes and writes `manifest.json`, `review.md`, `index.html`, `frames/watch-wide-normal.txt`, `frames/watch-wide-normal.cells.json`, `frames/watch-compact-normal.txt`, and `frames/watch-compact-normal.cells.json`.

- [ ] Commit this checkpoint.

```bash
git add src/cli.rs src/lib.rs src/commands src/dev_preview tests
git commit -m "Add hidden watch preview command"
```

## Task 7: Generate Pet Species/Stage Matrix

This task adds `--scenario pets` and completes `--scenario all`.

- [ ] Create `src/dev_preview/pets.rs`.

Core behavior:

- Use `Species::all()` for species columns.
- Render stages `0..=6` as rows.
- Use content mood and tick `0`.
- Use `render_pet` directly.
- Use `semantic_styles(ctx.render.color_capability)`.
- Use the shared helper from `PetPanel` for role styles/spans.
- Produce one frame: `pet-species-stage`.

Suggested shape:

```rust
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::pet::art::StageKey;
use crate::pet::generation::Species;
use crate::pet::render::{render_pet, AnimationFrame};
use crate::tui::panels::pet::pet_role_spans_for_line;
use crate::tui::style::semantic_styles;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};

pub fn pet_frames(ctx: &PreviewRenderContext) -> anyhow::Result<Vec<PreviewFrame>> {
    let frame = render_pet_matrix(ctx)?;
    Ok(vec![frame])
}
```

Use `Paragraph::new(lines).render(rect, &mut buffer)` for each matrix cell. The frame size should be fixed and generous enough to fit all 6 species x 7 stages without wrapping.

- [ ] Add unit tests in `src/dev_preview/pets.rs`.

Required test names:

- `pet_matrix_contains_all_species_names`
- `pet_matrix_contains_all_stage_labels`
- `pet_matrix_uses_expected_species_stage_count`

- [ ] Update `src/dev_preview/scenarios.rs` so selection works:

```rust
match selection {
    PreviewSelection::All => {
        frames.extend(watch_frames(&ctx, &scratch_dir)?);
        frames.extend(pet_frames(&ctx)?);
    }
    PreviewSelection::Watch => frames.extend(watch_frames(&ctx, &scratch_dir)?),
    PreviewSelection::Pets => frames.extend(pet_frames(&ctx)?),
}
```

- [ ] Add integration tests in `tests/dev_preview.rs`.

Required test names:

- `dev_preview_watch_writes_expected_artifacts`
- `dev_preview_pets_writes_species_stage_matrix`
- `dev_preview_all_writes_watch_and_pet_artifacts`
- `dev_preview_rerun_replaces_owned_output`
- `dev_preview_refuses_regular_file_output`
- `dev_preview_refuses_non_preview_directory`
- `dev_preview_html_references_local_assets_that_exist`
- `dev_preview_does_not_use_user_config_dir`

Each integration test must set `GLORP_CONFIG_DIR` to a temp path.

- [ ] Run focused scenario and CLI tests.

Commands:

```bash
cargo test dev_preview::pets
cargo test dev_preview::scenarios
cargo test --test dev_preview
cargo test help_hides_dev_preview_command --test cli_smoke
```

Expected: all pass.

- [ ] Commit this checkpoint.

```bash
git add src/dev_preview tests
git commit -m "Add pet matrix preview scenario"
```

## Task 8: Final Self-Review And Verification

- [ ] Review the full diff against the approved spec.

Command:

```bash
BASE=$(cat .git/glorp-preview-slice-1-base)
git diff "$BASE"..HEAD --stat
git diff "$BASE"..HEAD
```

Check specifically:

- no ANSI artifact writing in Slice 1,
- no animation playback code in Slice 1,
- no Gauntlet story files in Slice 1,
- no direct reads from Drew's real Glorp config/state,
- no `ColorCapability::detect()` calls inside panel rendering,
- hidden command not present in help output,
- safe output checks reject file, symlink, and non-owned non-empty directory.

- [ ] Search for accidental direct non-fixed watch view-model calls.

Command:

```bash
rg -n "build_watch_view_model\\(" src tests
```

Expected:

- any `build_watch_view_model(` hit is outside preview generation or an existing non-preview caller.

Then do a normal code review pass for unfinished-work markers before calling the branch done.

- [ ] Run formatting.

Command:

```bash
cargo fmt --check
```

Expected: passes. If it fails, run `cargo fmt`, inspect formatting-only diff, and re-run `cargo fmt --check`.

- [ ] Run full tests.

Command:

```bash
cargo test
```

Expected: passes.

- [ ] Run lint if the repo currently supports it cleanly.

Command:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: passes. If this exposes pre-existing warnings unrelated to the preview work, capture the exact warnings and ask Drew whether to include cleanup in this branch.

- [ ] Generate a real local preview bundle for manual inspection.

Command:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Expected:

- stdout says it wrote `target/glorp-preview`,
- `target/glorp-preview/index.html` exists,
- `target/glorp-preview/manifest.json` has `schema_version: 1` and `producer: "glorp-dev-preview"`,
- `target/glorp-preview/frames/watch-wide-normal.txt` exists,
- `target/glorp-preview/frames/watch-compact-normal.txt` exists,
- `target/glorp-preview/frames/pet-species-stage.txt` exists.

- [ ] Inspect generated manifest.

Command:

```bash
jq '.producer, .schema_version, [.artifacts[].id]' target/glorp-preview/manifest.json
```

Expected output includes:

```json
"glorp-dev-preview"
1
[
  "watch-wide-normal",
  "watch-compact-normal",
  "pet-species-stage"
]
```

- [ ] Open the static HTML file for local visual inspection.

Command:

```bash
open target/glorp-preview/index.html
```

Expected: a static contact sheet with the two watch frames and one species/stage matrix. No external network assets are needed.

- [ ] Commit final verification/doc tweaks if any were needed.

```bash
git add src tests docs
git commit -m "Verify preview lab slice one"
```

Only make this commit if the final review produces code/doc changes after Task 7.

## Completion Criteria

- `glorp dev-preview --scenario all --out target/glorp-preview` writes a complete static bundle.
- `glorp --help` does not mention `dev-preview`.
- The bundle can be safely regenerated over its own previous output.
- Unsafe output paths are refused before writing.
- Watch frames are deterministic across repeated runs.
- Pet matrix includes all species and stages required by Slice 1.
- Full test suite passes, or any unrelated pre-existing failure is documented with exact command output.

## Recommended Execution Mode

Use `superpowers:subagent-driven-development` if executing from this plan. The safest split is:

- Worker 1 owns Task 1 and Task 2 (`src/tui/**`, `tests/tui_render.rs`).
- Worker 2 owns Task 3 and Task 4 (`src/dev_preview/frame.rs`, `export.rs`, `output.rs`, assets).
- Worker 3 owns Task 5 through Task 7 (`src/cli.rs`, `src/lib.rs`, `src/commands/**`, scenario builders, integration tests), after Worker 1 and Worker 2 land their checkpoints.

Inline execution is also reasonable because Slice 1 touches shared rendering signatures; if using inline execution, keep the commits at the listed task checkpoints.
