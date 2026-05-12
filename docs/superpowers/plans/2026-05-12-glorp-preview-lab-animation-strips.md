# Glorp Preview Lab Animation Strips Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the hidden `glorp dev-preview` command with deterministic pet animation strips, schema-v2 manifest metadata, and paused HTML playback.

**Architecture:** Keep Slice 1 static preview behavior intact while adding a first-class multi-frame strip model. Generate static frames, strip frames, manifest, review markdown, and HTML from one preview-bundle structure so the file inventory cannot drift. Render pet-local animation through the real renderer and `PetPanel` paths, with exact test oracles for blink, glitch corruption, idle motion, and particle output.

**Tech Stack:** Rust, clap, ratatui `Buffer`, serde/serde_json, vanilla local HTML/CSS/JS assets, existing `assert_cmd` integration tests.

---

## Source Spec

- Design spec: `docs/superpowers/specs/2026-05-12-glorp-preview-lab-animation-strips-design.md`
- Parent preview-lab spec: `docs/superpowers/specs/2026-05-12-glorp-preview-lab-design.md`
- Current Slice 1 plan: `docs/superpowers/plans/2026-05-12-glorp-preview-lab-slice-1.md`

## Design Constraints From Review

- Slice 2 manifest output is schema version `2`.
- `scenarios[]` remains the static-frame list.
- `strips[]` is first-class multi-frame animation metadata.
- `artifacts[]` is the exhaustive generated file inventory.
- Manifest paths are normalized relative UTF-8 strings using `/`, with no leading slash, no `..`, and no backslashes.
- Static frames remain in `frames/`; strip frames live in `strips/<strip-id>/`.
- `PreviewStripFrame` uses `tick` for renderer-tick frames and `phase` for phase-only frames. Do not invent fake ticks for idle motion.
- `manifest.json`, `review.md`, `index.html`, and artifact writing must use the same `PreviewBundle` data.
- `glorp dev-preview --scenario animation` writes an animation-only bundle with non-empty `strips[]`.
- `glorp dev-preview --scenario all` writes static frames and animation strips.
- `watch` and `pets` selections do not write strip directories.
- HTML playback starts paused. Timers are created only after play and cleared on pause, previous, and next.
- Idle motion uses `PetPanel`, with unrelated `WatchViewModel` fields pinned.
- Blink tests must prove open -> closed -> same-open, not only that a closed-eye glyph exists.
- Glitch tests must assert an exact exported cell change from a `CorruptionHit` oracle.
- Particle tests must prove a particle exists in exported preview output, not only in pre-export renderer internals.
- Do not add Gauntlet, ANSI export, live terminal capture, new Rust dependencies, or external browser assets in this slice.

## File Map

Create:

- `src/dev_preview/animation.rs` - animation strip fixtures, rendering helpers, hit finders, and unit tests.
- `src/dev_preview/bundle.rs` - preview bundle model, normalized preview path helper, and static/strip bundle types.

Modify:

- `src/dev_preview/mod.rs` - expose `animation` and `bundle` modules.
- `src/dev_preview/export.rs` - schema-v2 manifest types, strip serialization, review markdown, HTML strip markup.
- `src/dev_preview/scenarios.rs` - orchestrate static frames plus strip bundles, write strip files, build manifest from one bundle.
- `src/dev_preview/assets/preview.html` - add the strip insertion marker.
- `src/dev_preview/assets/preview.css` - style strip viewport and controls.
- `src/dev_preview/assets/preview.js` - strip playback controls.
- `src/cli.rs` - add `PreviewScenarioArg::Animation`.
- `src/commands/dev_preview.rs` - route CLI value to `PreviewSelection::Animation`.
- `tests/dev_preview.rs` - animation and all-bundle integration tests.
- `README.md` - document focused preview scenarios.
- `AGENTS.md` - document animation preview usage for agents.
- `CLAUDE.md` - document animation preview usage for Claude.

## Task 1: Add Schema V2 Manifest And Normalized Path Foundation

**Files:**

- Create: `src/dev_preview/bundle.rs`
- Modify: `src/dev_preview/mod.rs`
- Modify: `src/dev_preview/export.rs`

- [ ] **Step 1: Write failing tests for normalized manifest paths and strip serialization**

Add to the `#[cfg(test)] mod tests` block in `src/dev_preview/export.rs`:

```rust
use crate::dev_preview::bundle::PreviewPath;

fn sample_strip() -> PreviewStrip {
    PreviewStrip {
        id: "pet-idle-motion-fuzz-s4".to_string(),
        kind: PreviewStripKind::PetAnimation,
        title: "Pet Idle Motion: Fuzz S4".to_string(),
        intent: "Review deterministic idle motion.".to_string(),
        dimensions: PreviewDimensions {
            width: 20,
            height: 12,
        },
        playback: PreviewPlayback {
            starts_paused: true,
            frame_duration_ms: 160,
        },
        inputs: BTreeMap::from([(
            "hit_kind".to_string(),
            Value::String("idle-motion".to_string()),
        )]),
        frames: vec![PreviewStripFrame {
            index: 0,
            label: "rest center".to_string(),
            tick: None,
            phase: Some("rest".to_string()),
            files: PreviewStripFrameFiles {
                text: PreviewPath::new("strips/pet-idle-motion-fuzz-s4/frame-000.txt").unwrap(),
                cells: PreviewPath::new(
                    "strips/pet-idle-motion-fuzz-s4/frame-000.cells.json",
                )
                .unwrap(),
            },
        }],
        review_prompts: vec!["Check idle movement.".to_string()],
    }
}

#[test]
fn preview_path_accepts_normalized_relative_paths() {
    let path = PreviewPath::new("strips/pet-idle-motion-fuzz-s4/frame-000.txt").unwrap();

    assert_eq!(path.as_str(), "strips/pet-idle-motion-fuzz-s4/frame-000.txt");
    assert_eq!(
        serde_json::to_string(&path).unwrap(),
        "\"strips/pet-idle-motion-fuzz-s4/frame-000.txt\""
    );
}

#[test]
fn preview_path_rejects_non_portable_or_escaping_paths() {
    for value in [
        "",
        "/absolute.txt",
        "../escape.txt",
        "frames/../escape.txt",
        "frames\\windows.txt",
    ] {
        assert!(PreviewPath::new(value).is_err(), "accepted invalid path {value}");
    }
}

#[test]
fn manifest_serializes_schema_v2_and_strips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manifest.json");
    let mut manifest = sample_manifest();
    manifest.strips = vec![sample_strip()];

    write_manifest(&path, &manifest).unwrap();

    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["strips"][0]["kind"], "pet-animation");
    assert_eq!(json["strips"][0]["frames"][0]["phase"], "rest");
    assert!(json["strips"][0]["frames"][0]["tick"].is_null());
    assert_eq!(
        json["strips"][0]["frames"][0]["files"]["text"],
        "strips/pet-idle-motion-fuzz-s4/frame-000.txt"
    );
}
```

- [ ] **Step 2: Run the focused export tests and confirm they fail**

Run:

```bash
cargo test preview_path --lib
cargo test manifest_serializes_schema_v2_and_strips --lib
```

Expected:

- `PreviewPath` is unresolved.
- `PreviewStrip` and related strip types are unresolved.
- Current manifest schema is still `1`.

- [ ] **Step 3: Create `src/dev_preview/bundle.rs`**

Add:

```rust
use crate::dev_preview::export::{PreviewManifest, PreviewStrip};
use crate::dev_preview::frame::PreviewFrame;
use crate::error::{GlorpError, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct PreviewPath(String);

impl PreviewPath {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_preview_path(&value)?;
        Ok(Self(value))
    }

    pub fn generated(value: impl Into<String>) -> Self {
        Self::new(value).expect("generated preview path should be normalized")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl std::fmt::Display for PreviewPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct PreviewBundle {
    pub static_frames: Vec<PreviewFrame>,
    pub strips: Vec<PreviewStripBundle>,
    pub manifest: PreviewManifest,
}

#[derive(Debug, Clone)]
pub struct PreviewStripBundle {
    pub strip: PreviewStrip,
    pub frames: Vec<PreviewStripFrameBundle>,
}

#[derive(Debug, Clone)]
pub struct PreviewStripFrameBundle {
    pub text_path: PreviewPath,
    pub cells_path: PreviewPath,
    pub frame: PreviewFrame,
}

fn validate_preview_path(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(GlorpError::Message("preview path must not be empty".to_string()));
    }
    if value.starts_with('/') {
        return Err(GlorpError::Message(format!(
            "preview path must be relative: {value}"
        )));
    }
    if value.contains('\\') {
        return Err(GlorpError::Message(format!(
            "preview path must use '/' separators: {value}"
        )));
    }
    if value.split('/').any(|part| part == "..") {
        return Err(GlorpError::Message(format!(
            "preview path must not contain '..': {value}"
        )));
    }
    Ok(())
}
```

- [ ] **Step 4: Expose the bundle module**

Modify `src/dev_preview/mod.rs`:

```rust
pub mod bundle;
pub mod export;
pub mod frame;
pub mod output;
pub mod pets;
pub mod scenarios;
pub mod watch;
```

- [ ] **Step 5: Add schema-v2 strip manifest types**

Modify `src/dev_preview/export.rs`:

```rust
use crate::dev_preview::bundle::PreviewPath;
```

Change:

```rust
pub const SCHEMA_VERSION: u32 = 2;
```

Change manifest paths from `PathBuf` to `PreviewPath`:

```rust
pub struct PreviewManifest {
    pub schema_version: u32,
    pub producer: &'static str,
    pub glorp_version: &'static str,
    pub generated_at: String,
    pub scenarios: Vec<PreviewScenario>,
    pub strips: Vec<PreviewStrip>,
    pub artifacts: Vec<PreviewArtifact>,
}

pub struct PreviewScenarioFiles {
    pub text: PreviewPath,
    pub cells: PreviewPath,
}

pub struct PreviewArtifact {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub path: PreviewPath,
    pub width: Option<u16>,
    pub height: Option<u16>,
}
```

Add strip types:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct PreviewStrip {
    pub id: String,
    pub kind: PreviewStripKind,
    pub title: String,
    pub intent: String,
    pub dimensions: PreviewDimensions,
    pub playback: PreviewPlayback,
    pub inputs: BTreeMap<String, Value>,
    pub frames: Vec<PreviewStripFrame>,
    pub review_prompts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewStripKind {
    PetAnimation,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewPlayback {
    pub starts_paused: bool,
    pub frame_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStripFrame {
    pub index: usize,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    pub files: PreviewStripFrameFiles,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStripFrameFiles {
    pub text: PreviewPath,
    pub cells: PreviewPath,
}
```

- [ ] **Step 6: Update existing test fixtures and path construction**

In `sample_manifest()` and existing production code, replace `PathBuf::from("...")` manifest values with `PreviewPath::generated("...")`.

In `write_review_markdown`, replace `scenario.files.text.display()` and
`scenario.files.cells.display()` with `scenario.files.text` and
`scenario.files.cells`, or with `scenario.files.text.as_str()` and
`scenario.files.cells.as_str()`.

In `src/dev_preview/scenarios.rs`, keep filesystem joins using `PreviewPath::as_path()`:

```rust
write_text_frame(&staging_dir.join(text_path(frame).as_path()), frame)?;
write_cells_json(&staging_dir.join(cells_path(frame).as_path()), frame)?;
```

Update helper signatures:

```rust
fn text_path(frame: &PreviewFrame) -> PreviewPath {
    PreviewPath::generated(format!("frames/{}.txt", frame.id))
}

fn cells_path(frame: &PreviewFrame) -> PreviewPath {
    PreviewPath::generated(format!("frames/{}.cells.json", frame.id))
}
```

- [ ] **Step 7: Run export tests**

Run:

```bash
cargo test --lib dev_preview::export
```

Expected: all `dev_preview::export` tests pass.

- [ ] **Step 8: Commit Task 1**

```bash
git add src/dev_preview/bundle.rs src/dev_preview/mod.rs src/dev_preview/export.rs src/dev_preview/scenarios.rs
git commit -m "feat: add preview manifest strip foundation"
```

## Task 2: Generate All Writers From One Preview Bundle

**Files:**

- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/bundle.rs`

- [ ] **Step 1: Write failing tests for review/HTML strip output from a sample bundle**

Add to `src/dev_preview/export.rs` tests:

```rust
#[test]
fn review_markdown_lists_animation_strips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("review.md");
    let mut manifest = sample_manifest();
    manifest.strips = vec![sample_strip()];

    write_review_markdown(&path, &manifest).unwrap();

    let markdown = fs::read_to_string(path).unwrap();
    assert!(markdown.contains("## Animation Strips"));
    assert!(markdown.contains("Pet Idle Motion: Fuzz S4"));
    assert!(markdown.contains("- Kind: `pet-animation`"));
    assert!(markdown.contains("- Frame count: `1`"));
    assert!(markdown.contains("- Frame 0: `rest center` (`phase: rest`)"));
}

#[test]
fn html_export_renders_strip_controls_paused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.html");
    let bundle = sample_preview_bundle();

    write_index_html(&path, &bundle, "2026-05-12T00:00:00Z").unwrap();

    let html = fs::read_to_string(path).unwrap();
    assert!(html.contains(r#"class="strip" data-strip-id="pet-idle-motion-fuzz-s4""#));
    assert!(html.contains(r#"data-playing="false""#));
    assert!(html.contains(r#"data-action="play""#));
    assert!(html.contains(r#"data-action="prev""#));
    assert!(html.contains(r#"data-action="next""#));
    assert!(html.contains("rest center"));
}
```

Add this helper in the same test module:

```rust
fn sample_preview_bundle() -> PreviewBundle {
    let frame = sample_frame();
    let strip_frame = PreviewStripFrameBundle {
        text_path: PreviewPath::generated("strips/pet-idle-motion-fuzz-s4/frame-000.txt"),
        cells_path: PreviewPath::generated(
            "strips/pet-idle-motion-fuzz-s4/frame-000.cells.json",
        ),
        frame,
    };
    let strip = sample_strip();
    let mut manifest = sample_manifest();
    manifest.strips = vec![strip.clone()];

    PreviewBundle {
        static_frames: vec![sample_frame()],
        strips: vec![PreviewStripBundle {
            strip,
            frames: vec![strip_frame],
        }],
        manifest,
    }
}
```

- [ ] **Step 2: Run the focused tests and confirm they fail**

Run:

```bash
cargo test review_markdown_lists_animation_strips --lib
cargo test html_export_renders_strip_controls_paused --lib
```

Expected:

- review markdown does not include animation strips.
- `write_index_html` still accepts `&[PreviewFrame]`, not `&PreviewBundle`.

- [ ] **Step 3: Update `write_review_markdown` to include strips**

In `src/dev_preview/export.rs`, add:

```rust
fn strip_kind_label(kind: PreviewStripKind) -> &'static str {
    match kind {
        PreviewStripKind::PetAnimation => "pet-animation",
    }
}

fn strip_frame_label(frame: &PreviewStripFrame) -> String {
    if let Some(tick) = frame.tick {
        format!("`{}` (`tick: {tick}`)", frame.label)
    } else if let Some(phase) = frame.phase.as_deref() {
        format!("`{}` (`phase: {phase}`)", frame.label)
    } else {
        format!("`{}`", frame.label)
    }
}
```

Extend `write_review_markdown` after the scenarios section:

```rust
if !manifest.strips.is_empty() {
    markdown.push_str("## Animation Strips\n\n");
    for strip in &manifest.strips {
        markdown.push_str(&format!("### {}\n\n{}\n\n", strip.title, strip.intent));
        markdown.push_str(&format!(
            "- ID: `{}`\n- Kind: `{}`\n- Size: `{}x{}`\n- Frame count: `{}`\n\n",
            strip.id,
            strip_kind_label(strip.kind),
            strip.dimensions.width,
            strip.dimensions.height,
            strip.frames.len()
        ));
        markdown.push_str("Frames:\n");
        for frame in &strip.frames {
            markdown.push_str(&format!(
                "- Frame {}: {}\n",
                frame.index,
                strip_frame_label(frame)
            ));
        }
        markdown.push_str("\nReview prompts:\n");
        for prompt in &strip.review_prompts {
            markdown.push_str(&format!("- {prompt}\n"));
        }
        markdown.push('\n');
    }
}
```

- [ ] **Step 4: Change `write_index_html` to accept `PreviewBundle`**

Change signature:

```rust
pub fn write_index_html(path: &Path, bundle: &PreviewBundle, generated_at: &str) -> Result<()> {
```

Build both sections:

```rust
let frames_html = bundle
    .static_frames
    .iter()
    .map(render_frame_html)
    .collect::<String>();
let strips_html = bundle.strips.iter().map(render_strip_html).collect::<String>();
let html = template
    .replace("{{GENERATED_AT}}", &escape_html(generated_at))
    .replace("{{FRAMES}}", &frames_html)
    .replace("{{STRIPS}}", &strips_html);
```

Add `render_strip_html`:

```rust
fn render_strip_html(bundle: &PreviewStripBundle) -> String {
    let strip = &bundle.strip;
    let mut html = String::new();
    html.push_str(&format!(
        r#"<article class="strip" data-strip-id="{}" data-playing="false" data-frame-index="0" data-frame-count="{}" data-frame-duration-ms="{}">"#,
        escape_html(&strip.id),
        strip.frames.len(),
        strip.playback.frame_duration_ms
    ));
    html.push_str(&format!("<h2>{}</h2>", escape_html(&strip.title)));
    html.push_str(&format!("<p>{}</p>", escape_html(&strip.intent)));
    html.push_str(r#"<div class="strip-viewport">"#);
    for (index, frame_bundle) in bundle.frames.iter().enumerate() {
        let hidden = if index == 0 { "" } else { " hidden" };
        html.push_str(&format!(
            r#"<div class="strip-frame{}" data-strip-frame="{}" data-frame-label="{}">"#,
            hidden,
            index,
            escape_html(&strip.frames[index].label)
        ));
        html.push_str(&render_frame_grid_html(&frame_bundle.frame));
        html.push_str("</div>");
    }
    html.push_str("</div>");
    html.push_str(r#"<div class="strip-controls">"#);
    html.push_str(r#"<button type="button" data-action="prev">Prev</button>"#);
    html.push_str(r#"<button type="button" data-action="play">Play</button>"#);
    html.push_str(r#"<button type="button" data-action="next">Next</button>"#);
    html.push_str(&format!(
        r#"<span class="strip-counter">1 / {}</span>"#,
        strip.frames.len()
    ));
    html.push_str(&format!(
        r#"<span class="strip-label">{}</span>"#,
        escape_html(&strip.frames[0].label)
    ));
    html.push_str("</div>");
    html.push_str("</article>");
    html
}
```

Extract the grid portion of `render_frame_html` into:

```rust
fn render_frame_grid_html(frame: &PreviewFrame) -> String {
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
```

Then call `render_frame_grid_html(frame)` inside `render_frame_html`.

- [ ] **Step 5: Add strip insertion marker to HTML template**

Modify `src/dev_preview/assets/preview.html` to include:

```html
<section class="section">
  <h1>Animation Strips</h1>
  {{STRIPS}}
</section>
```

Keep the existing `{{FRAMES}}` section.

- [ ] **Step 6: Add `PreviewBundle` construction in scenarios**

In `src/dev_preview/scenarios.rs`, add a private builder:

```rust
fn build_preview_bundle(
    frames: Vec<PreviewFrame>,
    strips: Vec<PreviewStripBundle>,
    generated_at: String,
    ctx: &PreviewRenderContext,
) -> PreviewBundle {
    let scenarios = frames
        .iter()
        .map(|frame| scenario_metadata(frame, ctx))
        .collect();
    let manifest = PreviewManifest {
        schema_version: SCHEMA_VERSION,
        producer: PRODUCER,
        glorp_version: env!("CARGO_PKG_VERSION"),
        generated_at,
        scenarios,
        strips: strips.iter().map(|strip| strip.strip.clone()).collect(),
        artifacts: artifacts_for_bundle(&frames, &strips),
    };
    PreviewBundle {
        static_frames: frames,
        strips,
        manifest,
    }
}
```

Change orchestration to call:

```rust
let bundle = build_preview_bundle(frames, strips, generated_at, &ctx);
write_index_html(&staging_dir.join("index.html"), &bundle, &bundle.manifest.generated_at)?;
write_review_markdown(&staging_dir.join("review.md"), &bundle.manifest)?;
write_manifest(&staging_dir.join("manifest.json"), &bundle.manifest)?;
```

- [ ] **Step 7: Run focused tests**

Run:

```bash
cargo test --lib dev_preview::export
```

Expected: export tests pass.

- [ ] **Step 8: Commit Task 2**

```bash
git add src/dev_preview/bundle.rs src/dev_preview/export.rs src/dev_preview/scenarios.rs src/dev_preview/assets/preview.html
git commit -m "feat: render preview strips in bundle outputs"
```

## Task 3: Add CLI Selection And Animation-Only Bundle Plumbing

**Files:**

- Modify: `src/cli.rs`
- Modify: `src/commands/dev_preview.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing CLI/integration tests for `--scenario animation`**

Add to `tests/dev_preview.rs`:

```rust
#[test]
fn dev_preview_animation_selection_writes_wrapper_without_static_frames() {
    let run = PreviewRun::new();

    run.run_success("animation");

    assert!(run.out.join("manifest.json").is_file());
    assert!(run.out.join("review.md").is_file());
    assert!(run.out.join("index.html").is_file());
    assert!(!run.out.join("frames/watch-wide-normal.txt").exists());
    assert!(!run.out.join("frames/pet-species-stage.txt").exists());

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 2);
    assert!(manifest["scenarios"].as_array().unwrap().is_empty());
    assert!(manifest["strips"].as_array().is_some());
}
```

Add:

```rust
#[test]
fn dev_preview_watch_and_pets_do_not_write_strips() {
    for scenario in ["watch", "pets"] {
        let run = PreviewRun::new();

        run.run_success(scenario);

        assert!(
            !run.out.join("strips").exists(),
            "{scenario} should not write strip artifacts"
        );
        assert!(run.manifest()["strips"].as_array().unwrap().is_empty());
    }
}
```

- [ ] **Step 2: Run tests and confirm they fail**

Run:

```bash
cargo test --test dev_preview dev_preview_animation_selection_writes_wrapper_without_static_frames
```

Expected: clap rejects `animation` because the enum value does not exist.

- [ ] **Step 3: Add CLI enum value**

Modify `src/cli.rs`:

```rust
#[derive(Clone, Debug, ValueEnum)]
pub enum PreviewScenarioArg {
    All,
    Watch,
    Pets,
    Animation,
}
```

- [ ] **Step 4: Add selection enum value**

Modify `src/dev_preview/scenarios.rs`:

```rust
pub enum PreviewSelection {
    All,
    Watch,
    Pets,
    Animation,
}
```

- [ ] **Step 5: Route CLI selection**

Modify `src/commands/dev_preview.rs`:

```rust
PreviewScenarioArg::Animation => PreviewSelection::Animation,
```

- [ ] **Step 6: Wire strip generation pass-through through orchestration**

This step should compile before real strips exist by returning an empty vector from `animation_strips`.

In `src/dev_preview/animation.rs`, add temporary scaffolding:

```rust
use crate::dev_preview::bundle::PreviewStripBundle;
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::error::Result;

pub fn animation_strips(_ctx: &PreviewRenderContext) -> Result<Vec<PreviewStripBundle>> {
    Ok(Vec::new())
}
```

Expose it from `src/dev_preview/mod.rs`:

```rust
pub mod animation;
```

In `src/dev_preview/scenarios.rs`, import:

```rust
use crate::dev_preview::animation::animation_strips;
```

Change selection dispatch:

```rust
let mut frames = Vec::new();
let mut strips = Vec::new();
match selection {
    PreviewSelection::All => {
        frames.extend(watch_frames(&ctx, &scratch_dir)?);
        frames.extend(pet_frames(&ctx)?);
        strips.extend(animation_strips(&ctx)?);
    }
    PreviewSelection::Watch => frames.extend(watch_frames(&ctx, &scratch_dir)?),
    PreviewSelection::Pets => frames.extend(pet_frames(&ctx)?),
    PreviewSelection::Animation => strips.extend(animation_strips(&ctx)?),
}
```

Create `strips/` only when `!strips.is_empty()`.

- [ ] **Step 7: Run the animation-selection test**

Run:

```bash
cargo test --test dev_preview dev_preview_animation_selection_writes_wrapper_without_static_frames
```

Expected: command accepts `animation`, writes wrapper files, skips static frames, and exposes `strips[]` as an array.

- [ ] **Step 8: Commit Task 3**

```bash
git add src/cli.rs src/commands/dev_preview.rs src/dev_preview/mod.rs src/dev_preview/animation.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "feat: add preview animation scenario plumbing"
```

## Task 4: Render Idle Motion Strip Through PetPanel

**Files:**

- Modify: `src/dev_preview/animation.rs`

- [ ] **Step 1: Write failing idle-motion unit tests**

Add to `src/dev_preview/animation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_preview::scenarios::PreviewRenderContext;

    #[test]
    fn animation_strips_include_idle_motion_strip() {
        let ctx = PreviewRenderContext::deterministic();
        let strips = animation_strips(&ctx).unwrap();
        let strip = strips
            .iter()
            .find(|strip| strip.strip.id == "pet-idle-motion-fuzz-s4")
            .expect("missing idle motion strip");

        assert_eq!(strip.frames.len(), 5);
        assert_eq!(strip.strip.frames[0].phase.as_deref(), Some("rest"));
        assert_eq!(strip.strip.frames[1].phase.as_deref(), Some("inhale-left"));
        assert!(strip.frames.iter().all(|frame| frame.frame.width == 20));
        assert!(strip.frames.iter().all(|frame| frame.frame.height == 12));
    }

    #[test]
    fn idle_motion_strip_contains_rest_lift_left_and_right_frames() {
        let ctx = PreviewRenderContext::deterministic();
        let strips = animation_strips(&ctx).unwrap();
        let strip = strips
            .iter()
            .find(|strip| strip.strip.id == "pet-idle-motion-fuzz-s4")
            .unwrap();

        let texts = strip
            .frames
            .iter()
            .map(|frame| frame_text(&frame.frame))
            .collect::<Vec<_>>();

        assert_ne!(texts[0], texts[1], "inhale frame should differ from rest");
        assert_ne!(texts[0], texts[3], "right wander frame should differ from rest");
        assert_eq!(texts[0], texts[4], "final rest should return to first rest");
    }

    fn frame_text(frame: &PreviewFrame) -> String {
        let mut text = String::new();
        for y in 0..frame.height {
            for cell in frame.cells.iter().filter(|cell| cell.y == y) {
                if !cell.continuation {
                    text.push_str(&cell.symbol);
                }
            }
            text.push('\n');
        }
        text
    }
}
```

- [ ] **Step 2: Run idle-motion tests and confirm they fail**

Run:

```bash
cargo test idle_motion_strip --lib
```

Expected: no idle-motion strip exists.

- [ ] **Step 3: Add animation strip constants and helpers**

In `src/dev_preview/animation.rs`, replace scaffolding with:

```rust
use crate::dev_preview::bundle::{PreviewPath, PreviewStripBundle, PreviewStripFrameBundle};
use crate::dev_preview::export::{
    PreviewDimensions, PreviewPlayback, PreviewStrip, PreviewStripFrame, PreviewStripFrameFiles,
    PreviewStripKind,
};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::error::Result;
use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::generation::{generate_pet, Species};
use crate::pet::render::{render_pet, AnimationFrame};
use crate::tui::panels::{pet::PetPanel, Panel};
use crate::tui::view_model::WatchViewModel;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const STRIP_WIDTH: u16 = 20;
const STRIP_HEIGHT: u16 = 12;
const PLAYBACK_MS: u64 = 160;

pub fn animation_strips(ctx: &PreviewRenderContext) -> Result<Vec<PreviewStripBundle>> {
    Ok(vec![idle_motion_strip(ctx)?])
}

fn playback() -> PreviewPlayback {
    PreviewPlayback {
        starts_paused: true,
        frame_duration_ms: PLAYBACK_MS,
    }
}
```

- [ ] **Step 4: Implement idle-motion fixture builder**

Add:

```rust
struct IdleMotionFrame {
    label: &'static str,
    phase: &'static str,
    breath_offset_y: u8,
    wander_offset_x: i8,
}

fn idle_motion_strip(ctx: &PreviewRenderContext) -> Result<PreviewStripBundle> {
    let id = "pet-idle-motion-fuzz-s4";
    let phases = [
        IdleMotionFrame {
            label: "rest center",
            phase: "rest",
            breath_offset_y: 0,
            wander_offset_x: 0,
        },
        IdleMotionFrame {
            label: "inhale left",
            phase: "inhale-left",
            breath_offset_y: 1,
            wander_offset_x: -1,
        },
        IdleMotionFrame {
            label: "hold center",
            phase: "hold",
            breath_offset_y: 1,
            wander_offset_x: 0,
        },
        IdleMotionFrame {
            label: "exhale right",
            phase: "exhale-right",
            breath_offset_y: 0,
            wander_offset_x: 1,
        },
        IdleMotionFrame {
            label: "rest center",
            phase: "rest",
            breath_offset_y: 0,
            wander_offset_x: 0,
        },
    ];

    let mut frames = Vec::new();
    let mut manifest_frames = Vec::new();
    for (index, phase) in phases.iter().enumerate() {
        let frame = render_idle_motion_frame(ctx, phase)?;
        let text = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.txt"));
        let cells = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.cells.json"));
        manifest_frames.push(PreviewStripFrame {
            index,
            label: phase.label.to_string(),
            tick: None,
            phase: Some(phase.phase.to_string()),
            files: PreviewStripFrameFiles {
                text: text.clone(),
                cells: cells.clone(),
            },
        });
        frames.push(PreviewStripFrameBundle {
            text_path: text,
            cells_path: cells,
            frame,
        });
    }

    Ok(PreviewStripBundle {
        strip: PreviewStrip {
            id: id.to_string(),
            kind: PreviewStripKind::PetAnimation,
            title: "Pet Idle Motion: Fuzz S4".to_string(),
            intent: "Review deterministic breath and wander movement through PetPanel.".to_string(),
            dimensions: PreviewDimensions {
                width: STRIP_WIDTH,
                height: STRIP_HEIGHT,
            },
            playback: playback(),
            inputs: BTreeMap::from([
                ("species".to_string(), Value::String("fuzz".to_string())),
                ("stage".to_string(), Value::String("s4".to_string())),
                ("mood".to_string(), Value::String("content".to_string())),
                (
                    "seed".to_string(),
                    Value::String("glorp-preview-idle-fuzz".to_string()),
                ),
                (
                    "phases".to_string(),
                    json!(phases.iter().map(|phase| phase.phase).collect::<Vec<_>>()),
                ),
                (
                    "breath_offsets_y".to_string(),
                    json!(phases
                        .iter()
                        .map(|phase| phase.breath_offset_y)
                        .collect::<Vec<_>>()),
                ),
                (
                    "wander_offsets_x".to_string(),
                    json!(phases
                        .iter()
                        .map(|phase| phase.wander_offset_x)
                        .collect::<Vec<_>>()),
                ),
                ("hit_kind".to_string(), Value::String("idle-motion".to_string())),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Does the pet move by exactly one row or one column with no clipping?".to_string(),
                "Does the strip feel like idle motion rather than a layout jump?".to_string(),
            ],
        },
        frames,
    })
}
```

- [ ] **Step 5: Render idle motion through `PetPanel`**

Add:

```rust
fn render_idle_motion_frame(
    ctx: &PreviewRenderContext,
    phase: &IdleMotionFrame,
) -> Result<PreviewFrame> {
    let pet = generate_pet("glorp-preview-idle-fuzz").with_species_for_test(Species::Fuzz);
    let rendered = render_pet(
        &pet,
        Stage::S4,
        Mood::Content,
        AnimationFrame {
            tick: 0,
            blink_suppression_ticks: 8,
        },
    );
    let mut vm = WatchViewModel::fixture();
    vm.pet_name = pet.name;
    vm.species = Species::Fuzz.as_str().to_string();
    vm.stage = "s4".to_string();
    vm.mood = "content".to_string();
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    vm.current_speech = None;
    vm.cursor_screen = None;
    vm.mouse_tracking_enabled = false;
    vm.energy = 0.81;
    vm.breath_offset_y = phase.breath_offset_y;
    vm.wander_offset_x = phase.wander_offset_x;

    let mut buffer = Buffer::empty(Rect::new(0, 0, STRIP_WIDTH, STRIP_HEIGHT));
    PetPanel.render(
        Rect::new(0, 0, STRIP_WIDTH, STRIP_HEIGHT),
        &mut buffer,
        &vm,
        &ctx.render,
    );
    Ok(frame_from_buffer(
        format!("pet-idle-motion-fuzz-s4-{}", phase.phase),
        format!("Pet Idle Motion: {}", phase.label),
        &buffer,
    ))
}
```

- [ ] **Step 6: Run idle-motion tests**

Run:

```bash
cargo test idle_motion_strip --lib
```

Expected: idle-motion tests pass.

- [ ] **Step 7: Commit Task 4**

```bash
git add src/dev_preview/animation.rs
git commit -m "feat: add preview idle motion strip"
```

## Task 5: Add Blink Hit Strip With Open-Closed-Open Oracle

**Files:**

- Modify: `src/dev_preview/animation.rs`

- [ ] **Step 1: Write failing blink strip tests**

Add to `src/dev_preview/animation.rs` tests:

```rust
#[test]
fn blink_strip_proves_open_closed_open_transition() {
    let ctx = PreviewRenderContext::deterministic();
    let strips = animation_strips(&ctx).unwrap();
    let strip = strips
        .iter()
        .find(|strip| strip.strip.id == "pet-blink-hit-fuzz-s4")
        .expect("missing blink strip");

    assert_eq!(strip.frames.len(), 5);
    let texts = strip
        .frames
        .iter()
        .map(|frame| frame_text(&frame.frame))
        .collect::<Vec<_>>();
    let closed = crate::pet::render::closed_blink_eyes(crate::pet::generation::Species::Fuzz);

    assert!(
        !texts[1].contains(closed),
        "pre-blink neighbor should not contain closed eyes"
    );
    assert!(texts[2].contains(closed), "hit frame should close eyes");
    assert!(
        !texts[3].contains(closed),
        "post-blink neighbor should not contain closed eyes"
    );
    assert_eq!(strip.strip.frames[2].tick, Some(strip.strip.frames[2].tick.unwrap()));
}
```

- [ ] **Step 2: Run blink test and confirm it fails**

Run:

```bash
cargo test blink_strip --lib
```

Expected: blink strip is missing.

- [ ] **Step 3: Add blink strip to `animation_strips`**

Change:

```rust
pub fn animation_strips(ctx: &PreviewRenderContext) -> Result<Vec<PreviewStripBundle>> {
    Ok(vec![idle_motion_strip(ctx)?, blink_hit_strip()?])
}
```

- [ ] **Step 4: Implement blink hit finder and renderer**

Add:

```rust
fn blink_hit_strip() -> Result<PreviewStripBundle> {
    let id = "pet-blink-hit-fuzz-s4";
    let species = Species::Fuzz;
    let pet = generate_pet("glorp-preview-blink-fuzz").with_species_for_test(species);
    let closed = crate::pet::render::closed_blink_eyes(species);
    if pet.traits.eyes == closed {
        return Err(crate::error::GlorpError::Message(format!(
            "blink fixture open eyes match closed eyes for {species:?}: {closed}"
        )));
    }
    let hit_tick = find_blink_hit_tick(&pet, species, closed)?;
    let ticks = [
        hit_tick.saturating_sub(2),
        hit_tick.saturating_sub(1),
        hit_tick,
        hit_tick + 1,
        hit_tick + 2,
    ];
    let mut frames = Vec::new();
    let mut manifest_frames = Vec::new();
    for (index, tick) in ticks.into_iter().enumerate() {
        let frame = render_pet_strip_frame(
            "pet-blink-hit-fuzz-s4",
            &format!("Pet Blink Hit: tick {tick}"),
            &pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick,
                blink_suppression_ticks: 0,
            },
        );
        let label = if tick == hit_tick {
            format!("tick {tick} blink")
        } else {
            format!("tick {tick}")
        };
        let text = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.txt"));
        let cells = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.cells.json"));
        manifest_frames.push(PreviewStripFrame {
            index,
            label,
            tick: Some(tick),
            phase: None,
            files: PreviewStripFrameFiles {
                text: text.clone(),
                cells: cells.clone(),
            },
        });
        frames.push(PreviewStripFrameBundle {
            text_path: text,
            cells_path: cells,
            frame,
        });
    }

    Ok(PreviewStripBundle {
        strip: PreviewStrip {
            id: id.to_string(),
            kind: PreviewStripKind::PetAnimation,
            title: "Pet Blink Hit: Fuzz S4".to_string(),
            intent: "Review a deterministic blink frame and neighboring open-eye frames."
                .to_string(),
            dimensions: PreviewDimensions {
                width: STRIP_WIDTH,
                height: STRIP_HEIGHT,
            },
            playback: playback(),
            inputs: BTreeMap::from([
                ("species".to_string(), Value::String("fuzz".to_string())),
                ("stage".to_string(), Value::String("s4".to_string())),
                ("mood".to_string(), Value::String("content".to_string())),
                (
                    "seed".to_string(),
                    Value::String("glorp-preview-blink-fuzz".to_string()),
                ),
                ("ticks".to_string(), json!(ticks)),
                ("hit_tick".to_string(), json!(hit_tick)),
                ("hit_kind".to_string(), Value::String("blink".to_string())),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Does the closed-eye frame read as a blink?".to_string(),
                "Does the face return to the same open-eye expression after the blink?".to_string(),
            ],
        },
        frames,
    })
}

fn find_blink_hit_tick(
    pet: &crate::pet::generation::GeneratedPet,
    species: Species,
    closed: &str,
) -> Result<u64> {
    for tick in 2..=254 {
        let prev = render_pet(
            pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick: tick - 1,
                blink_suppression_ticks: 0,
            },
        );
        let hit = render_pet(
            pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick,
                blink_suppression_ticks: 0,
            },
        );
        let next = render_pet(
            pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick: tick + 1,
                blink_suppression_ticks: 0,
            },
        );
        let prev_text = prev.lines.join("\n");
        let hit_text = hit.lines.join("\n");
        let next_text = next.lines.join("\n");
        if !prev_text.contains(closed) && hit_text.contains(closed) && prev_text == next_text {
            return Ok(tick);
        }
    }
    Err(crate::error::GlorpError::Message(format!(
        "no blink hit found for {species:?} seed glorp-preview-blink-fuzz in ticks 0..=256"
    )))
}
```

- [ ] **Step 5: Add direct pet strip renderer helper**

Add:

```rust
fn render_pet_strip_frame(
    id: &str,
    title: &str,
    pet: &crate::pet::generation::GeneratedPet,
    stage: Stage,
    mood: Mood,
    animation: AnimationFrame,
) -> PreviewFrame {
    let rendered = render_pet(pet, stage, mood, animation);
    let styles = crate::tui::style::semantic_styles();
    let mut buffer = Buffer::empty(Rect::new(0, 0, STRIP_WIDTH, STRIP_HEIGHT));
    let left_pad = ((STRIP_WIDTH as usize).saturating_sub(13) / 2) as u16;
    let top_pad = ((STRIP_HEIGHT as usize).saturating_sub(10) / 2) as u16;
    let lines = rendered
        .lines
        .iter()
        .enumerate()
        .map(|(line_index, art_line)| {
            ratatui::text::Line::from(crate::tui::panels::pet::pet_role_spans_for_line(
                art_line,
                line_index,
                &rendered.spans,
                &styles,
                None,
            ))
        })
        .collect::<Vec<_>>();
    Paragraph::new(lines).render(
        Rect::new(left_pad, top_pad, 13, 10),
        &mut buffer,
    );
    frame_from_buffer(id, title, &buffer)
}
```

- [ ] **Step 6: Run blink tests**

Run:

```bash
cargo test blink_strip --lib
```

Expected: blink tests pass.

- [ ] **Step 7: Commit Task 5**

```bash
git add src/dev_preview/animation.rs
git commit -m "feat: add preview blink strip"
```

## Task 6: Add Glitch Corruption Hit Strip With Exact Exported Cell Assertion

**Files:**

- Modify: `src/pet/render.rs`
- Modify: `src/dev_preview/animation.rs`

- [ ] **Step 1: Write failing corruption helper tests**

Add to `src/pet/render.rs` tests:

```rust
#[test]
fn glitch_corruption_hit_reports_visible_cell_change() {
    let pet = crate::pet::generation::generate_pet("glorp-preview-glitch")
        .with_species_for_test(Species::Glitch);
    let hit = find_glitch_corruption_hit(&pet, Stage::S4, Mood::Content, 0..=512)
        .expect("expected visible glitch hit");

    assert_eq!(hit.tick % 37, 0);
    assert_ne!(hit.before, hit.after);
}
```

If `src/pet/render.rs` does not have a test module, add one at the bottom.

- [ ] **Step 2: Run helper test and confirm it fails**

Run:

```bash
cargo test glitch_corruption_hit_reports_visible_cell_change --lib
```

Expected: `find_glitch_corruption_hit` is unresolved.

- [ ] **Step 3: Add `CorruptionHit` and helper to `src/pet/render.rs`**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CorruptionHit {
    pub tick: u64,
    pub row: usize,
    pub col: usize,
    pub before: char,
    pub after: char,
}

pub(crate) fn find_glitch_corruption_hit(
    pet: &GeneratedPet,
    stage: Stage,
    mood: Mood,
    ticks: impl IntoIterator<Item = u64>,
) -> Option<CorruptionHit> {
    if pet.species != Species::Glitch {
        return None;
    }
    for tick in ticks {
        let hit = glitch_corruption_hit_for_tick(pet, stage, mood, tick);
        if hit.is_some() {
            return hit;
        }
    }
    None
}

fn glitch_corruption_hit_for_tick(
    pet: &GeneratedPet,
    stage: Stage,
    mood: Mood,
    tick: u64,
) -> Option<CorruptionHit> {
    if !tick.is_multiple_of(37) {
        return None;
    }
    let stage_key = stage_key(stage);
    let expression = expression_for(
        pet,
        mood,
        false,
    );
    let raw = template_lines(
        pet.species,
        stage_key,
        pet.traits.morph_index,
        pet.traits.morph_pup_index,
    );
    let rendered = raw
        .iter()
        .enumerate()
        .map(|(line_index, line)| render_template_line(line, line_index, pet, &expression))
        .collect::<Vec<_>>();
    let lines = rendered
        .iter()
        .map(|line| line.text.clone())
        .collect::<Vec<_>>();
    let spans = rendered
        .into_iter()
        .flat_map(|line| line.spans)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }
    let row = ((tick * 7) as usize) % lines.len();
    let line = &lines[row];
    let total_chars = line.chars().count();
    if total_chars == 0 {
        return None;
    }
    let col = ((tick * 11) as usize) % total_chars;
    let before = line.chars().nth(col).unwrap_or(' ');
    if before == ' ' {
        return None;
    }
    let in_body = spans.iter().any(|span| {
        span.line == row
            && span.role == PaletteRoleName::Body
            && col >= span.start
            && col < span.end
    });
    if !in_body {
        return None;
    }
    let after = GLITCH_NOISE[((tick * 3) as usize) % GLITCH_NOISE.len()];
    Some(CorruptionHit {
        tick,
        row: row + 1,
        col: col + 1,
        before,
        after,
    })
}
```

The returned `row` and `col` are framed `render_pet` coordinates after the 13x10 wrapper adds one row and one column of padding.

- [ ] **Step 4: Write failing exported-cell glitch test**

Add to `src/dev_preview/animation.rs` tests:

```rust
#[test]
fn glitch_strip_asserts_exact_exported_corruption_cell() {
    let ctx = PreviewRenderContext::deterministic();
    let strips = animation_strips(&ctx).unwrap();
    let strip = strips
        .iter()
        .find(|strip| strip.strip.id == "pet-glitch-corruption-hit-s4")
        .expect("missing glitch strip");

    let hit_tick = strip.strip.inputs["hit_tick"].as_u64().unwrap();
    let export_x = strip.strip.inputs["hit_export_x"].as_u64().unwrap() as u16;
    let export_y = strip.strip.inputs["hit_export_y"].as_u64().unwrap() as u16;
    let after = strip.strip.inputs["hit_after"].as_str().unwrap();
    let hit_index = strip
        .strip
        .frames
        .iter()
        .position(|frame| frame.tick == Some(hit_tick))
        .unwrap();
    let hit_cell = strip.frames[hit_index]
        .frame
        .cells
        .iter()
        .find(|cell| cell.x == export_x && cell.y == export_y)
        .unwrap();

    assert_eq!(hit_cell.symbol, after);
}
```

- [ ] **Step 5: Implement glitch strip**

In `animation_strips`, add:

```rust
glitch_corruption_strip()?
```

Add:

```rust
fn glitch_corruption_strip() -> Result<PreviewStripBundle> {
    let id = "pet-glitch-corruption-hit-s4";
    let pet = generate_pet("glorp-preview-glitch").with_species_for_test(Species::Glitch);
    let hit = crate::pet::render::find_glitch_corruption_hit(
        &pet,
        Stage::S4,
        Mood::Content,
        0..=512,
    )
    .ok_or_else(|| {
        crate::error::GlorpError::Message(
            "no visible glitch corruption hit for seed glorp-preview-glitch in ticks 0..=512"
                .to_string(),
        )
    })?;
    let ticks = [
        hit.tick.saturating_sub(37),
        hit.tick.saturating_sub(1),
        hit.tick,
        hit.tick + 1,
        hit.tick + 37,
    ];
    let left_pad = ((STRIP_WIDTH as usize).saturating_sub(13) / 2) as u64;
    let top_pad = ((STRIP_HEIGHT as usize).saturating_sub(10) / 2) as u64;
    let hit_export_x = left_pad + hit.col as u64;
    let hit_export_y = top_pad + hit.row as u64;

    let mut frames = Vec::new();
    let mut manifest_frames = Vec::new();
    for (index, tick) in ticks.into_iter().enumerate() {
        let frame = render_pet_strip_frame(
            "pet-glitch-corruption-hit-s4",
            &format!("Pet Glitch Corruption Hit: tick {tick}"),
            &pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick,
                blink_suppression_ticks: 8,
            },
        );
        let label = if tick == hit.tick {
            format!("tick {tick} corruption")
        } else {
            format!("tick {tick}")
        };
        let text = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.txt"));
        let cells = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.cells.json"));
        manifest_frames.push(PreviewStripFrame {
            index,
            label,
            tick: Some(tick),
            phase: None,
            files: PreviewStripFrameFiles {
                text: text.clone(),
                cells: cells.clone(),
            },
        });
        frames.push(PreviewStripFrameBundle {
            text_path: text,
            cells_path: cells,
            frame,
        });
    }

    Ok(PreviewStripBundle {
        strip: PreviewStrip {
            id: id.to_string(),
            kind: PreviewStripKind::PetAnimation,
            title: "Pet Glitch Corruption Hit: S4".to_string(),
            intent: "Review a deterministic visible Glitch corruption cell.".to_string(),
            dimensions: PreviewDimensions {
                width: STRIP_WIDTH,
                height: STRIP_HEIGHT,
            },
            playback: playback(),
            inputs: BTreeMap::from([
                ("species".to_string(), Value::String("glitch".to_string())),
                ("stage".to_string(), Value::String("s4".to_string())),
                ("mood".to_string(), Value::String("content".to_string())),
                ("seed".to_string(), Value::String("glorp-preview-glitch".to_string())),
                ("ticks".to_string(), json!(ticks)),
                ("hit_tick".to_string(), json!(hit.tick)),
                ("hit_export_x".to_string(), json!(hit_export_x)),
                ("hit_export_y".to_string(), json!(hit_export_y)),
                ("hit_before".to_string(), Value::String(hit.before.to_string())),
                ("hit_after".to_string(), Value::String(hit.after.to_string())),
                (
                    "hit_kind".to_string(),
                    Value::String("glitch-corruption".to_string()),
                ),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Is the corruption visible but still recognizably Glitch?".to_string(),
                "Does the effect avoid looking like broken Unicode or a layout bug?".to_string(),
            ],
        },
        frames,
    })
}
```

- [ ] **Step 6: Run glitch tests**

Run:

```bash
cargo test glitch_corruption --lib
cargo test glitch_strip --lib
```

Expected: glitch helper and strip tests pass.

- [ ] **Step 7: Commit Task 6**

```bash
git add src/pet/render.rs src/dev_preview/animation.rs
git commit -m "feat: add preview glitch corruption strip"
```

## Task 7: Add Particle Hit Strip And Exported Particle Assertions

**Files:**

- Modify: `src/dev_preview/animation.rs`

- [ ] **Step 1: Write failing particle strip test**

Add to `src/dev_preview/animation.rs` tests:

```rust
#[test]
fn particle_strip_contains_exported_particle_glyph() {
    let ctx = PreviewRenderContext::deterministic();
    let strips = animation_strips(&ctx).unwrap();
    let strip = strips
        .iter()
        .find(|strip| strip.strip.id == "pet-particle-hit-crystal-s4")
        .expect("missing particle strip");

    assert_eq!(strip.frames.len(), 5);
    let exported_symbols = strip
        .frames
        .iter()
        .flat_map(|frame| frame.frame.cells.iter())
        .filter(|cell| !cell.continuation)
        .map(|cell| cell.symbol.as_str())
        .collect::<Vec<_>>();

    assert!(
        exported_symbols.iter().any(|symbol| matches!(*symbol, "✧" | "✦" | "·")),
        "expected exported particle glyph in crystal strip"
    );
}
```

- [ ] **Step 2: Run particle test and confirm it fails**

Run:

```bash
cargo test particle_strip --lib
```

Expected: particle strip is missing.

- [ ] **Step 3: Implement particle strip**

In `animation_strips`, add:

```rust
particle_hit_strip()?
```

Add:

```rust
fn particle_hit_strip() -> Result<PreviewStripBundle> {
    let id = "pet-particle-hit-crystal-s4";
    let pet =
        generate_pet("glorp-preview-particle-crystal").with_species_for_test(Species::Crystal);
    let ticks = [0, 4, 8, 12, 16];
    let mut frames = Vec::new();
    let mut manifest_frames = Vec::new();
    for (index, tick) in ticks.into_iter().enumerate() {
        let frame = render_pet_strip_frame(
            "pet-particle-hit-crystal-s4",
            &format!("Pet Particle Hit: tick {tick}"),
            &pet,
            Stage::S4,
            Mood::Content,
            AnimationFrame {
                tick,
                blink_suppression_ticks: 8,
            },
        );
        let text = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.txt"));
        let cells = PreviewPath::generated(format!("strips/{id}/frame-{index:03}.cells.json"));
        manifest_frames.push(PreviewStripFrame {
            index,
            label: format!("tick {tick}"),
            tick: Some(tick),
            phase: None,
            files: PreviewStripFrameFiles {
                text: text.clone(),
                cells: cells.clone(),
            },
        });
        frames.push(PreviewStripFrameBundle {
            text_path: text,
            cells_path: cells,
            frame,
        });
    }

    Ok(PreviewStripBundle {
        strip: PreviewStrip {
            id: id.to_string(),
            kind: PreviewStripKind::PetAnimation,
            title: "Pet Particle Hit: Crystal S4".to_string(),
            intent: "Review deterministic Crystal particle placement and styling.".to_string(),
            dimensions: PreviewDimensions {
                width: STRIP_WIDTH,
                height: STRIP_HEIGHT,
            },
            playback: playback(),
            inputs: BTreeMap::from([
                ("species".to_string(), Value::String("crystal".to_string())),
                ("stage".to_string(), Value::String("s4".to_string())),
                ("mood".to_string(), Value::String("content".to_string())),
                (
                    "seed".to_string(),
                    Value::String("glorp-preview-particle-crystal".to_string()),
                ),
                ("ticks".to_string(), json!(ticks)),
                ("hit_kind".to_string(), Value::String("particle".to_string())),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Do particles sit inside the fixed frame without clipping?".to_string(),
                "Are particles visible using the current pet accent styling?".to_string(),
            ],
        },
        frames,
    })
}
```

- [ ] **Step 4: Run animation strip tests**

Run:

```bash
cargo test animation_strips --lib
cargo test particle_strip --lib
```

Expected: animation strip tests pass for idle motion, blink, glitch, and particle.

- [ ] **Step 5: Commit Task 7**

```bash
git add src/dev_preview/animation.rs
git commit -m "feat: add preview particle strip"
```

## Task 8: Write Strip Files And Complete Integration Tests

**Files:**

- Modify: `src/dev_preview/scenarios.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing final bundle assertions**

Strengthen `dev_preview_animation_selection_writes_wrapper_without_static_frames` in `tests/dev_preview.rs`:

```rust
assert!(run.out.join("strips").is_dir());

for path in [
    "strips/pet-idle-motion-fuzz-s4/frame-000.txt",
    "strips/pet-idle-motion-fuzz-s4/frame-000.cells.json",
    "strips/pet-blink-hit-fuzz-s4/frame-000.txt",
    "strips/pet-glitch-corruption-hit-s4/frame-000.txt",
    "strips/pet-particle-hit-crystal-s4/frame-000.txt",
] {
    assert!(run.out.join(path).is_file(), "missing {path}");
}

let strip_ids = strip_ids(&manifest);
assert_eq!(
    strip_ids,
    vec![
        "pet-idle-motion-fuzz-s4".to_string(),
        "pet-blink-hit-fuzz-s4".to_string(),
        "pet-glitch-corruption-hit-s4".to_string(),
        "pet-particle-hit-crystal-s4".to_string(),
    ]
);
```

Update `dev_preview_all_writes_watch_and_pet_artifacts` into:

```rust
#[test]
fn dev_preview_all_writes_static_frames_and_animation_strips() {
    let run = PreviewRun::new();

    run.run_success("all");

    for file in [
        "frames/watch-wide-normal.txt",
        "frames/watch-compact-normal.txt",
        "frames/pet-species-stage.txt",
    ] {
        assert!(run.out.join(file).is_file(), "missing {file}");
    }
    assert!(run.out.join("strips").is_dir());

    let manifest = run.manifest();
    let ids = scenario_ids(&manifest);
    assert_eq!(
        ids,
        vec![
            "watch-wide-normal".to_string(),
            "watch-compact-normal".to_string(),
            "pet-species-stage".to_string(),
        ]
    );
    assert_eq!(
        strip_ids(&manifest),
        vec![
            "pet-idle-motion-fuzz-s4".to_string(),
            "pet-blink-hit-fuzz-s4".to_string(),
            "pet-glitch-corruption-hit-s4".to_string(),
            "pet-particle-hit-crystal-s4".to_string(),
        ]
    );
}
```

Add helper:

```rust
fn strip_ids(manifest: &Value) -> Vec<String> {
    manifest["strips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|strip| strip["id"].as_str().unwrap().to_string())
        .collect()
}
```

Add:

```rust
#[test]
fn dev_preview_animation_manifest_paths_are_normalized() {
    let run = PreviewRun::new();

    run.run_success("animation");

    let manifest = run.manifest();
    for artifact in manifest["artifacts"].as_array().unwrap() {
        let path = artifact["path"].as_str().unwrap();
        assert!(!path.starts_with('/'), "absolute path in manifest: {path}");
        assert!(!path.contains('\\'), "backslash path in manifest: {path}");
        assert!(!path.split('/').any(|part| part == ".."), "escaping path in manifest: {path}");
    }
}
```

- [ ] **Step 2: Run integration tests and confirm file assertions fail if writing is missing**

Run:

```bash
cargo test --test dev_preview dev_preview_animation_selection_writes_wrapper_without_static_frames
```

Expected: failures point at missing strip files if strip writing is not wired.

- [ ] **Step 3: Write strip frames in orchestration**

In `src/dev_preview/scenarios.rs`, after writing static frames:

```rust
for strip in &bundle.strips {
    for frame in &strip.frames {
        write_text_frame(&staging_dir.join(frame.text_path.as_path()), &frame.frame)?;
        write_cells_json(&staging_dir.join(frame.cells_path.as_path()), &frame.frame)?;
    }
}
```

Before this loop, ensure parent directories exist:

```rust
for strip in &bundle.strips {
    for frame in &strip.frames {
        if let Some(parent) = staging_dir.join(frame.text_path.as_path()).parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(parent) = staging_dir.join(frame.cells_path.as_path()).parent() {
            fs::create_dir_all(parent)?;
        }
    }
}
```

- [ ] **Step 4: Extend `artifacts_for_bundle`**

Replace static-only `artifacts_for_frames` usage with:

```rust
fn artifacts_for_bundle(
    frames: &[PreviewFrame],
    strips: &[PreviewStripBundle],
) -> Vec<PreviewArtifact> {
    let mut artifacts = artifacts_for_frames(frames);
    for strip in strips {
        for frame in &strip.frames {
            artifacts.push(PreviewArtifact {
                id: format!("{}-frame-{:03}-text", strip.strip.id, frame_index(&frame.text_path)),
                title: format!("{} frame text", strip.strip.title),
                artifact_type: ArtifactType::Text,
                path: frame.text_path.clone(),
                width: Some(frame.frame.width),
                height: Some(frame.frame.height),
            });
            artifacts.push(PreviewArtifact {
                id: format!("{}-frame-{:03}-cells", strip.strip.id, frame_index(&frame.cells_path)),
                title: format!("{} frame cells", strip.strip.title),
                artifact_type: ArtifactType::Cells,
                path: frame.cells_path.clone(),
                width: Some(frame.frame.width),
                height: Some(frame.frame.height),
            });
        }
    }
    artifacts
}

fn frame_index(path: &PreviewPath) -> usize {
    path.as_str()
        .rsplit_once("frame-")
        .and_then(|(_, rest)| rest.get(0..3))
        .and_then(|digits| digits.parse::<usize>().ok())
        .expect("generated strip frame paths include frame index")
}
```

Keep existing root artifacts for `index.html`, `review.md`, CSS, and JS.

- [ ] **Step 5: Run integration tests**

Run:

```bash
cargo test --test dev_preview
```

Expected: all dev-preview integration tests pass.

- [ ] **Step 6: Commit Task 8**

```bash
git add src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "feat: write preview animation strip artifacts"
```

## Task 9: Add HTML Playback Behavior And Styling

**Files:**

- Modify: `src/dev_preview/assets/preview.css`
- Modify: `src/dev_preview/assets/preview.js`
- Modify: `src/dev_preview/export.rs`

- [ ] **Step 1: Write failing asset tests**

Add to `src/dev_preview/export.rs` tests:

```rust
#[test]
fn preview_js_defers_timers_until_play_and_clears_on_step() {
    let js = include_str!("assets/preview.js");

    assert!(js.contains("setInterval"));
    assert!(js.contains("clearInterval"));
    assert!(js.contains("data-playing"));
    assert!(js.contains("data-action"));
}

#[test]
fn preview_css_has_strip_viewport_and_hidden_frame_rules() {
    let css = include_str!("assets/preview.css");

    assert!(css.contains(".strip-viewport"));
    assert!(css.contains(".strip-frame.hidden"));
    assert!(css.contains(".strip-controls"));
}
```

- [ ] **Step 2: Run asset tests and confirm they fail**

Run:

```bash
cargo test preview_js_defers_timers_until_play_and_clears_on_step --lib
cargo test preview_css_has_strip_viewport_and_hidden_frame_rules --lib
```

Expected: tests fail until CSS/JS includes strip playback behavior.

- [ ] **Step 3: Add playback JS**

Replace or extend `src/dev_preview/assets/preview.js` with:

```javascript
(() => {
  const timers = new WeakMap();

  function frames(strip) {
    return Array.from(strip.querySelectorAll("[data-strip-frame]"));
  }

  function currentIndex(strip) {
    return Number(strip.dataset.frameIndex || "0");
  }

  function setPlaying(strip, playing) {
    strip.dataset.playing = playing ? "true" : "false";
    const play = strip.querySelector('[data-action="play"]');
    if (play) {
      play.textContent = playing ? "Pause" : "Play";
    }
  }

  function stop(strip) {
    const timer = timers.get(strip);
    if (timer) {
      clearInterval(timer);
      timers.delete(strip);
    }
    setPlaying(strip, false);
  }

  function show(strip, index) {
    const all = frames(strip);
    const next = ((index % all.length) + all.length) % all.length;
    strip.dataset.frameIndex = String(next);
    all.forEach((frame, frameIndex) => {
      frame.classList.toggle("hidden", frameIndex !== next);
    });
    const counter = strip.querySelector(".strip-counter");
    if (counter) {
      counter.textContent = `${next + 1} / ${all.length}`;
    }
    const label = strip.querySelector(".strip-label");
    if (label) {
      label.textContent = all[next].dataset.frameLabel || "";
    }
  }

  function play(strip) {
    if (timers.has(strip)) {
      stop(strip);
      return;
    }
    setPlaying(strip, true);
    const duration = Number(strip.dataset.frameDurationMs || "160");
    const timer = setInterval(() => {
      show(strip, currentIndex(strip) + 1);
    }, duration);
    timers.set(strip, timer);
  }

  document.addEventListener("click", (event) => {
    const button = event.target.closest("[data-action]");
    if (!button) {
      return;
    }
    const strip = button.closest("[data-strip-id]");
    if (!strip) {
      return;
    }
    const action = button.dataset.action;
    if (action === "play") {
      play(strip);
      return;
    }
    stop(strip);
    if (action === "prev") {
      show(strip, currentIndex(strip) - 1);
    } else if (action === "next") {
      show(strip, currentIndex(strip) + 1);
    }
  });

  document.querySelectorAll("[data-strip-id]").forEach((strip) => {
    strip.dataset.playing = "false";
    show(strip, 0);
  });
})();
```

- [ ] **Step 4: Add strip CSS**

Append to `src/dev_preview/assets/preview.css`:

```css
.strip {
  border: 1px solid #4a4038;
  padding: 16px;
  margin: 16px 0;
  background: #1d1a17;
}

.strip-viewport {
  margin: 12px 0;
}

.strip-frame.hidden {
  display: none;
}

.strip-controls {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  font: 13px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.strip-controls button {
  background: #161412;
  border: 1px solid #4a4038;
  color: #e8e3da;
  cursor: pointer;
  font: inherit;
  padding: 4px 8px;
}

.strip-controls button:hover {
  border-color: #d7a86e;
}

.strip-counter,
.strip-label {
  color: #aaa39a;
}
```

- [ ] **Step 5: Run export tests**

Run:

```bash
cargo test --lib dev_preview::export
```

Expected: export tests pass.

- [ ] **Step 6: Commit Task 9**

```bash
git add src/dev_preview/assets/preview.css src/dev_preview/assets/preview.js src/dev_preview/export.rs
git commit -m "feat: add preview strip playback controls"
```

## Task 10: Update Usage Docs

**Files:**

- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update README preview section**

In the `glorp dev-preview` docs, include:

````markdown
cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation
````

Document scenario selection:

````markdown
- `watch` - static watch layout frames only.
- `pets` - static species/stage matrix only.
- `animation` - pet animation strips with paused HTML playback.
- `all` - complete review bundle; this is the default.
````

- [ ] **Step 2: Update AGENTS preview instructions**

Add or update the Preview Lab section:

````markdown
For animation review, run:

```bash
cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation
```

Open `target/glorp-preview-animation/index.html`. Animation strips start paused;
use play/pause/previous/next controls for frame-by-frame review. Use `manifest.json`
for exact tick or phase metadata.
````

- [ ] **Step 3: Update CLAUDE preview instructions**

Add the same scenario guidance as README and include:

````markdown
Use `--scenario watch` or `--scenario pets` for focused static review. Use
`--scenario animation` for strip playback. Use `--scenario all` before handoff
when the full visual bundle matters.
````

- [ ] **Step 4: Run docs grep check**

Run:

```bash
rg -n "scenario animation|Animation Strips|dev-preview --scenario" README.md AGENTS.md CLAUDE.md
```

Expected: all three files mention animation scenario usage.

- [ ] **Step 5: Commit Task 10**

```bash
git add README.md AGENTS.md CLAUDE.md
git commit -m "docs: document preview animation strips"
```

## Task 11: Final Verification

**Files:**

- No new files.
- Verify all touched files.

- [ ] **Step 1: Run formatter check**

Run:

```bash
cargo fmt --check
```

Expected: exits 0.

- [ ] **Step 2: Run clippy**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: exits 0.

- [ ] **Step 3: Run full tests**

Run:

```bash
cargo test
```

Expected: exits 0.

- [ ] **Step 4: Generate animation bundle manually**

Run:

```bash
cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation
```

Expected:

- stdout contains `target/glorp-preview-animation`
- `target/glorp-preview-animation/index.html` exists
- `target/glorp-preview-animation/manifest.json` exists
- `target/glorp-preview-animation/strips/pet-idle-motion-fuzz-s4/frame-000.txt` exists
- `target/glorp-preview-animation/strips/pet-blink-hit-fuzz-s4/frame-000.txt` exists
- `target/glorp-preview-animation/strips/pet-glitch-corruption-hit-s4/frame-000.txt` exists
- `target/glorp-preview-animation/strips/pet-particle-hit-crystal-s4/frame-000.txt` exists

- [ ] **Step 5: Inspect manifest with jq**

Run:

```bash
jq '.schema_version, [.strips[].id], (.artifacts | length)' target/glorp-preview-animation/manifest.json
```

Expected:

```text
2
[
  "pet-idle-motion-fuzz-s4",
  "pet-blink-hit-fuzz-s4",
  "pet-glitch-corruption-hit-s4",
  "pet-particle-hit-crystal-s4"
]
```

The artifact count should be greater than 10.

- [ ] **Step 6: Handle verification findings**

If verification reveals a mistake, return to the task that introduced that file,
add a focused failing test when possible, make the smallest fix, rerun the
failed verification command, and commit only the exact files changed for that
fix. If verification is clean, record the passing commands in the final handoff.

## Implementation Notes

- Use `git status --short` before every commit. The repo may contain unrelated dirty files; stage exact files only.
- Do not run `git add -A`.
- Do not create a branch unless Drew asks.
- Preserve Slice 1 output safety behavior.
- Keep `dev-preview` hidden from normal `glorp --help`.
- Do not make preview generation read or create `GLORP_CONFIG_DIR`.
- If a test wants the user config directory, it is the wrong test for this slice.

## Completion Gate

The implementation is complete only when:

- `cargo fmt --check` passes.
- `cargo clippy --all-targets -- -D warnings` passes.
- `cargo test` passes.
- `cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation` writes a reviewable bundle.
- README, AGENTS, and CLAUDE document how to use the animation preview.
- The final git status contains no unstaged changes from this slice.
