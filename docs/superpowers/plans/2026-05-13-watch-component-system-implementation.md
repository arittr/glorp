# Watch Component System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current hand-spaced watch layout with a Glorp component system that produces one shared `ComponentLayout` artifact for rendering, Preview Lab, effects, and hit testing.

**Architecture:** Start with a Ratatui-buffer-native component layer that mirrors today's layout into semantic component nodes and geometry targets. Then migrate rendering to consume those nodes, extract `PetScene` as the only bespoke geometry component, add Lip Gloss-class styling ergonomics through a Glorp facade, and finally hide Taffy behind container allocation once the artifact contract is proven.

**Tech Stack:** Rust 2021, ratatui 0.29, crossterm 0.28, tachyonfx 0.18, serde/serde_json, time, Taffy 0.10.1 hidden behind Glorp container components.

**Source spec:** [docs/superpowers/specs/2026-05-13-watch-component-system-design.md](../specs/2026-05-13-watch-component-system-design.md)

---

## File Structure

**Create:**

- `src/tui/component/mod.rs` - module exports and shared component prelude.
- `src/tui/component/ids.rs` - stable component and target IDs.
- `src/tui/component/geometry.rs` - `ComponentLayout`, node layouts, targets, visibility, hit-test data.
- `src/tui/component/sizing.rs` - Glorp-native sizing and degradation policy types.
- `src/tui/component/style.rs` - Lip Gloss-class Glorp style facade over Ratatui styles.
- `src/tui/component/widgets.rs` - ordinary reusable widgets: `Panel`, `TextRow`, `StatRow`, `ProgressBar`, `InlineSparkline`, `FeedList`.
- `src/tui/component/pet_scene.rs` - `PetSceneLayout` and the single source of pet scene geometry.
- `src/tui/component/watch_screen.rs` - stateless wide/compact watch composition and `layout_watch`.
- `src/tui/component/preview.rs` - preview-only layout JSON export structs.
- `src/tui/component/taffy_backend.rs` - Taffy-backed container allocator introduced after the component artifact is stable.

**Modify:**

- `Cargo.toml` and `Cargo.lock` - add Taffy only in Task 13.
- `src/tui/mod.rs` - export `component`.
- `src/tui/render_context.rs` - add deterministic `WatchClock`.
- `src/tui/style.rs` - connect existing semantic styles to the new style facade.
- `src/tui/layout.rs` - make `layout_watch` and `render_watch_layout` the watch render path, then remove obsolete slat math.
- `src/tui/app.rs` - apply tachyonfx and mouse interpretation through `ComponentLayout` targets instead of `pet_panel_rect`.
- `src/tui/panels/*.rs` - migrate ordinary panels into widget composition; keep compatibility shims only until Task 15.
- `src/dev_preview/export.rs`, `src/dev_preview/scenarios.rs`, `src/dev_preview/watch.rs`, `src/dev_preview/assets/preview.css`, `src/dev_preview/assets/preview.html`, `src/dev_preview/assets/preview.js` - export and display layout overlays.
- `tests/dev_preview.rs`, `tests/tui_render.rs`, `tests/style_tokens.rs` - update assertions to component/layout invariants.

**Delete by the final task:**

- `pet_panel_rect` as an independent geometry implementation in `src/tui/layout.rs`.
- Wide-mode tests that assert cross-column slat alignment as a product invariant.
- Old panel-level callsites that hand-build Ratatui `Block`, `Paragraph`, border, padding, and progress styling for ordinary panels.

---

## Phase 1: Shared Layout Artifact First

### Task 1: Add ComponentLayout Core Types

**Files:**

- Create: `src/tui/component/mod.rs`
- Create: `src/tui/component/ids.rs`
- Create: `src/tui/component/geometry.rs`
- Create: `src/tui/component/sizing.rs`
- Modify: `src/tui/mod.rs`

- [ ] **Step 1: Write failing tests for IDs, target paths, duplicate detection, and hit testing**

Add tests inside `src/tui/component/geometry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::ids::{ComponentPath, TargetPath};
    use ratatui::layout::{Position, Rect};

    #[test]
    fn component_layout_rejects_duplicate_nodes() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        let node = ComponentNodeLayout::leaf(ComponentPath::new("watch.pet"), Rect::new(0, 0, 10, 10));
        layout.insert_node(node.clone()).unwrap();
        let err = layout.insert_node(node).unwrap_err();
        assert!(err.to_string().contains("duplicate component path"));
    }

    #[test]
    fn component_layout_indexes_targets_by_stable_path() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        layout.insert_node(ComponentNodeLayout::leaf(ComponentPath::new("watch.pet"), Rect::new(1, 2, 20, 10))).unwrap();
        layout.insert_target(
            TargetPath::new("watch.pet.art"),
            GeometryTarget {
                owner: ComponentPath::new("watch.pet"),
                rect: Rect::new(5, 4, 13, 10),
                z: 10,
                clip: Rect::new(1, 2, 20, 10),
                role: TargetRole::PetArt,
            },
        ).unwrap();
        assert_eq!(layout.target(TargetPath::new("watch.pet.art")).unwrap().rect.width, 13);
    }

    #[test]
    fn hit_test_returns_highest_z_target_containing_point() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        layout.insert_node(ComponentNodeLayout::leaf(ComponentPath::new("watch.pet"), Rect::new(0, 0, 20, 20))).unwrap();
        layout.insert_target(TargetPath::new("watch.pet.panel"), GeometryTarget {
            owner: ComponentPath::new("watch.pet"),
            rect: Rect::new(0, 0, 20, 20),
            z: 1,
            clip: Rect::new(0, 0, 20, 20),
            role: TargetRole::PetPanel,
        }).unwrap();
        layout.insert_target(TargetPath::new("watch.pet.art"), GeometryTarget {
            owner: ComponentPath::new("watch.pet"),
            rect: Rect::new(4, 5, 13, 10),
            z: 10,
            clip: Rect::new(0, 0, 20, 20),
            role: TargetRole::PetArt,
        }).unwrap();

        let hit = hit_test(&layout, Position::new(6, 6)).unwrap();
        assert_eq!(hit.target, TargetPath::new("watch.pet.art"));
        assert_eq!(hit.local_position, Position::new(2, 1));
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run:

```bash
cargo test component_layout --lib -- --nocapture
```

Expected: FAIL because `tui::component` and the layout types do not exist.

- [ ] **Step 3: Add the component module exports**

Set `src/tui/component/mod.rs` to:

```rust
pub mod geometry;
pub mod ids;
pub mod sizing;

pub use geometry::{
    hit_test, ComponentLayout, ComponentNodeLayout, GeometryTarget, HitResult, LayoutDecision,
    LayoutDecisionReason, LayoutMode, TargetRole, VisibilityState,
};
pub use ids::{ComponentPath, TargetPath, WatchComponentId};
pub use sizing::{AxisSize, ComponentSizing, DegradeRule};
```

Add this line to `src/tui/mod.rs`:

```rust
pub mod component;
```

- [ ] **Step 4: Add stable path IDs**

Set `src/tui/component/ids.rs` to:

```rust
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentPath(&'static str);

impl ComponentPath {
    pub const fn new(path: &'static str) -> Self {
        Self(path)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ComponentPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetPath(&'static str);

impl TargetPath {
    pub const fn new(path: &'static str) -> Self {
        Self(path)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for TargetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchComponentId {
    Root,
    Pet,
    Vitals,
    Bio,
    Today,
    Progress,
    Feed,
}

impl WatchComponentId {
    pub const fn path(self) -> ComponentPath {
        match self {
            WatchComponentId::Root => ComponentPath::new("watch"),
            WatchComponentId::Pet => ComponentPath::new("watch.pet"),
            WatchComponentId::Vitals => ComponentPath::new("watch.vitals"),
            WatchComponentId::Bio => ComponentPath::new("watch.bio"),
            WatchComponentId::Today => ComponentPath::new("watch.today"),
            WatchComponentId::Progress => ComponentPath::new("watch.progress"),
            WatchComponentId::Feed => ComponentPath::new("watch.feed"),
        }
    }
}
```

- [ ] **Step 5: Add layout, target, visibility, decision, and hit-test types**

Set `src/tui/component/geometry.rs` to include:

```rust
use crate::tui::component::ids::{ComponentPath, TargetPath};
use ratatui::layout::{Position, Rect};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Wide,
    Compact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentLayout {
    pub frame: Rect,
    pub content: Rect,
    pub mode: LayoutMode,
    pub nodes: BTreeMap<ComponentPath, ComponentNodeLayout>,
    pub targets: BTreeMap<TargetPath, GeometryTarget>,
    pub decisions: Vec<LayoutDecision>,
}

impl ComponentLayout {
    pub fn new(frame: Rect, mode: LayoutMode) -> Self {
        Self {
            frame,
            content: frame,
            mode,
            nodes: BTreeMap::new(),
            targets: BTreeMap::new(),
            decisions: Vec::new(),
        }
    }

    pub fn with_content(mut self, content: Rect) -> Self {
        self.content = content;
        self
    }

    pub fn insert_node(&mut self, node: ComponentNodeLayout) -> Result<(), LayoutBuildError> {
        if self.nodes.contains_key(&node.id) {
            return Err(LayoutBuildError::DuplicateComponent(node.id));
        }
        self.nodes.insert(node.id, node);
        Ok(())
    }

    pub fn insert_target(
        &mut self,
        path: TargetPath,
        target: GeometryTarget,
    ) -> Result<(), LayoutBuildError> {
        if self.targets.contains_key(&path) {
            return Err(LayoutBuildError::DuplicateTarget(path));
        }
        self.targets.insert(path, target);
        Ok(())
    }

    pub fn node(&self, id: ComponentPath) -> Option<&ComponentNodeLayout> {
        self.nodes.get(&id)
    }

    pub fn target(&self, path: TargetPath) -> Option<&GeometryTarget> {
        self.targets.get(&path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentNodeLayout {
    pub id: ComponentPath,
    pub bounds: Rect,
    pub content: Rect,
    pub visibility: VisibilityState,
    pub children: Vec<ComponentPath>,
    pub targets: BTreeMap<TargetPath, GeometryTarget>,
}

impl ComponentNodeLayout {
    pub fn leaf(id: ComponentPath, bounds: Rect) -> Self {
        Self {
            id,
            bounds,
            content: bounds,
            visibility: VisibilityState::Visible,
            children: Vec::new(),
            targets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryTarget {
    pub owner: ComponentPath,
    pub rect: Rect,
    pub z: i16,
    pub clip: Rect,
    pub role: TargetRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRole {
    Content,
    PetPanel,
    PetArt,
    PetSpeech,
    Habitat,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityState {
    Visible,
    Hidden { reason: LayoutDecisionReason },
    Degraded { reason: LayoutDecisionReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDecisionReason {
    CompactMode,
    InsufficientHeight,
    InsufficientWidth,
    RowLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDecision {
    pub path: ComponentPath,
    pub reason: LayoutDecisionReason,
    pub message: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitResult {
    pub target: TargetPath,
    pub rect: Rect,
    pub local_position: Position,
    pub z: i16,
}

pub fn hit_test(layout: &ComponentLayout, point: Position) -> Option<HitResult> {
    layout
        .targets
        .iter()
        .filter(|(_, target)| rect_contains(target.rect, point))
        .max_by_key(|(_, target)| target.z)
        .map(|(path, target)| HitResult {
            target: *path,
            rect: target.rect,
            local_position: Position::new(point.x - target.rect.x, point.y - target.rect.y),
            z: target.z,
        })
}

fn rect_contains(rect: Rect, point: Position) -> bool {
    point.x >= rect.x
        && point.y >= rect.y
        && point.x < rect.x.saturating_add(rect.width)
        && point.y < rect.y.saturating_add(rect.height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutBuildError {
    DuplicateComponent(ComponentPath),
    DuplicateTarget(TargetPath),
}

impl fmt::Display for LayoutBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutBuildError::DuplicateComponent(path) => {
                write!(f, "duplicate component path {path}")
            }
            LayoutBuildError::DuplicateTarget(path) => write!(f, "duplicate target path {path}"),
        }
    }
}

impl std::error::Error for LayoutBuildError {}
```

- [ ] **Step 6: Add sizing policy types**

Set `src/tui/component/sizing.rs` to:

```rust
use crate::tui::component::ids::TargetPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSizing {
    pub width: AxisSize,
    pub height: AxisSize,
    pub degrade: Vec<DegradeRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisSize {
    Fixed(u16),
    Intrinsic {
        min: u16,
        preferred: u16,
        max: Option<u16>,
    },
    Fill {
        min: u16,
        weight: u16,
        max: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradeRule {
    LimitRows { target: TargetPath, min: u16, max: u16 },
    OmitDetail { target: TargetPath },
    HideTarget { target: TargetPath },
    HideComponent,
}
```

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test component_layout --lib -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/mod.rs src/tui/component
git commit -m "feat(tui): add component layout artifact types"
```

### Task 2: Mirror Current Watch Geometry Into ComponentLayout

**Files:**

- Create: `src/tui/component/watch_screen.rs`
- Modify: `src/tui/component/mod.rs`
- Modify: `src/tui/layout.rs`

- [ ] **Step 1: Write failing geometry tests for current wide and compact layouts**

Add tests in `src/tui/component/watch_screen.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::{TargetPath, WatchComponentId};
    use crate::tui::view_model::WatchViewModel;
    use ratatui::layout::Rect;

    #[test]
    fn layout_watch_wide_exports_required_nodes_and_pet_targets() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 120, 32), &vm);
        assert_eq!(layout.mode, LayoutMode::Wide);
        for id in [
            WatchComponentId::Root,
            WatchComponentId::Pet,
            WatchComponentId::Vitals,
            WatchComponentId::Bio,
            WatchComponentId::Today,
            WatchComponentId::Progress,
            WatchComponentId::Feed,
        ] {
            assert!(layout.node(id.path()).is_some(), "missing node {}", id.path());
        }
        assert_eq!(layout.target(TargetPath::new("watch.pet.art")).unwrap().rect.width, 13);
        assert_eq!(layout.target(TargetPath::new("watch.pet.art")).unwrap().rect.height, 10);
    }

    #[test]
    fn layout_watch_compact_records_bio_hidden_decision() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 72, 24), &vm);
        assert_eq!(layout.mode, LayoutMode::Compact);
        let bio = layout.node(WatchComponentId::Bio.path()).unwrap();
        assert!(matches!(bio.visibility, VisibilityState::Hidden { reason: LayoutDecisionReason::CompactMode }));
        assert!(layout.decisions.iter().any(|d| d.path == WatchComponentId::Bio.path()));
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run:

```bash
cargo test layout_watch --lib -- --nocapture
```

Expected: FAIL because `layout_watch` does not exist.

- [ ] **Step 3: Export the watch-screen module**

Add to `src/tui/component/mod.rs`:

```rust
pub mod watch_screen;
pub use watch_screen::{layout_watch, render_watch_layout};
```

- [ ] **Step 4: Add `layout_watch` as an adapter over the existing layout math**

Create `src/tui/component/watch_screen.rs` with:

```rust
use crate::tui::component::{
    ComponentLayout, ComponentNodeLayout, ComponentPath, GeometryTarget, LayoutDecision,
    LayoutDecisionReason, LayoutMode, TargetPath, TargetRole, VisibilityState, WatchComponentId,
};
use crate::tui::panels::pet::pet_inner_rect_in_panel;
use crate::tui::view_model::WatchViewModel;
use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};

pub const COMPACT_THRESHOLD: usize = 104;
pub const WIDE_LEFT_COL: u16 = 40;
pub const WIDE_GUTTER: u16 = 4;
pub const MAX_FRAME_WIDTH: u16 = 110;
pub const MAX_FRAME_HEIGHT: u16 = 23;
pub const INNER_VPAD: u16 = 1;
pub const WIDE_BAND_1: u16 = 10;
pub const WIDE_BAND_2: u16 = 8;
pub const COLUMN_GAP: u16 = 1;

pub fn layout_watch(terminal_area: Rect, vm: &WatchViewModel) -> ComponentLayout {
    let frame = bounded_frame_rect(terminal_area);
    let mode = if (frame.width as usize) >= COMPACT_THRESHOLD + 2 {
        LayoutMode::Wide
    } else {
        LayoutMode::Compact
    };
    let content = ratatui::widgets::Block::bordered().inner(frame);
    let mut layout = ComponentLayout::new(frame, mode).with_content(content);
    layout
        .insert_node(ComponentNodeLayout::leaf(WatchComponentId::Root.path(), frame))
        .expect("root component id is unique");
    match mode {
        LayoutMode::Wide => layout_wide(content, vm, &mut layout),
        LayoutMode::Compact => layout_compact(content, vm, &mut layout),
    }
    layout
}

pub fn bounded_frame_rect(terminal_area: Rect) -> Rect {
    let width = terminal_area.width.min(MAX_FRAME_WIDTH);
    let is_wide = (width as usize) >= COMPACT_THRESHOLD + 2;
    let height = if is_wide {
        terminal_area.height.min(MAX_FRAME_HEIGHT)
    } else {
        terminal_area.height
    };
    let x = terminal_area.x + terminal_area.width.saturating_sub(width) / 2;
    let y = terminal_area.y + terminal_area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}
```

Then move the existing wide/compact split logic from `src/tui/layout.rs` into private `layout_wide` and `layout_compact` helpers in this file. Keep the current constants and constraints byte-for-byte for this task. For each allocated panel rect:

- insert a node with the matching `WatchComponentId`,
- set `content` equal to `bounds` for ordinary panels,
- set `watch.pet.panel` and `watch.pet.art` targets,
- set compact `watch.bio` to `Hidden { reason: CompactMode }`,
- record a `LayoutDecision` for the hidden compact bio.

The pet-art target must use `pet_inner_rect_in_panel` so the artifact matches today's rendered pet position during the adapter step.

- [ ] **Step 5: Keep `src/tui/layout.rs` compiling through compatibility exports**

At the top of `src/tui/layout.rs`, replace duplicated constants with imports:

```rust
use crate::tui::component::watch_screen::{
    bounded_frame_rect, COLUMN_GAP, COMPACT_THRESHOLD, INNER_VPAD, MAX_FRAME_HEIGHT,
    MAX_FRAME_WIDTH, WIDE_BAND_1, WIDE_BAND_2, WIDE_GUTTER, WIDE_LEFT_COL,
};
```

Do not change rendering behavior in this task.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test layout_watch --lib -- --nocapture
cargo test --test tui_render wide_layout -- --nocapture
```

Expected: PASS. The second command proves the adapter did not move visible layout yet.

- [ ] **Step 7: Commit**

```bash
git add src/tui/component/watch_screen.rs src/tui/component/mod.rs src/tui/layout.rs
git commit -m "feat(tui): mirror watch geometry into ComponentLayout"
```

### Task 3: Export Preview Lab Layout JSON and Overlay Controls

**Files:**

- Create: `src/tui/component/preview.rs`
- Modify: `src/tui/component/mod.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/assets/preview.css`
- Modify: `src/dev_preview/assets/preview.html`
- Modify: `src/dev_preview/assets/preview.js`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing Preview Lab tests for layout artifacts and overlay controls**

Add to `tests/dev_preview.rs`:

```rust
#[test]
fn dev_preview_watch_writes_layout_artifacts_and_manifest_entries() {
    let run = PreviewRun::new();
    run.run_success("watch");

    assert!(run.out.join("frames/watch-wide-normal.layout.json").is_file());
    assert!(run.out.join("frames/watch-compact-normal.layout.json").is_file());

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 2);
    let wide = scenario(&manifest, "watch-wide-normal");
    assert_eq!(wide["files"]["layout"], "frames/watch-wide-normal.layout.json");
    assert_artifact_type(&manifest, "watch-wide-normal-layout", "layout");

    let layout: Value = serde_json::from_str(
        &std::fs::read_to_string(run.out.join("frames/watch-wide-normal.layout.json")).unwrap(),
    ).unwrap();
    assert_eq!(layout["schema_version"], 1);
    assert!(layout["components"]["watch.pet"].is_object());
    assert!(layout["targets"]["watch.pet.art"].is_object());
}

#[test]
fn dev_preview_html_contains_layout_overlay_controls() {
    let run = PreviewRun::new();
    run.run_success("watch");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    assert!(html.contains("data-overlay-toggle=\"components\""));
    assert!(html.contains("data-overlay-toggle=\"targets\""));
    assert!(html.contains("data-layout-for=\"watch-wide-normal\""));
}
```

Update the existing helper `assert_scenario` signature in `tests/dev_preview.rs` so watch scenarios assert `files.layout`. Pet-matrix scenarios keep `layout` absent.

- [ ] **Step 2: Run the tests to confirm they fail**

Run:

```bash
cargo test --test dev_preview dev_preview_watch_writes_layout_artifacts_and_manifest_entries --features dev-preview -- --nocapture
```

Expected: FAIL because manifest schema is still 1 and no layout artifacts are written.

- [ ] **Step 3: Add preview layout export structs**

Set `src/tui/component/preview.rs` to:

```rust
use crate::tui::component::{ComponentLayout, ComponentPath, GeometryTarget, TargetPath};
use ratatui::layout::Rect;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewLayout {
    pub schema_version: u32,
    pub frame_id: String,
    pub mode: String,
    pub frame: PreviewRect,
    pub content: PreviewRect,
    pub components: BTreeMap<String, PreviewRect>,
    pub targets: BTreeMap<String, PreviewRect>,
    pub decisions: Vec<PreviewLayoutDecision>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct PreviewRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewLayoutDecision {
    pub path: String,
    pub reason: String,
    pub message: String,
}

pub fn preview_layout(frame_id: &str, layout: &ComponentLayout) -> PreviewLayout {
    PreviewLayout {
        schema_version: 1,
        frame_id: frame_id.to_string(),
        mode: format!("{:?}", layout.mode).to_lowercase(),
        frame: rect(layout.frame),
        content: rect(layout.content),
        components: layout.nodes.iter().map(|(path, node)| (path.as_str().to_string(), rect(node.bounds))).collect(),
        targets: layout.targets.iter().map(|(path, target)| (path.as_str().to_string(), rect(target.rect))).collect(),
        decisions: layout.decisions.iter().map(|decision| PreviewLayoutDecision {
            path: decision.path.as_str().to_string(),
            reason: format!("{:?}", decision.reason),
            message: decision.message.to_string(),
        }).collect(),
    }
}

fn rect(rect: Rect) -> PreviewRect {
    PreviewRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}
```

Remove unused imports after compiling; this snippet names the shape and field contract.

- [ ] **Step 4: Extend PreviewFrame to carry optional layout**

In `src/dev_preview/frame.rs`, update `PreviewFrame` so watch frames can carry
a component layout artifact while pet-matrix frames carry `None`:

```rust
pub struct PreviewFrame {
    pub id: String,
    pub title: String,
    pub width: u16,
    pub height: u16,
    pub cells: Vec<PreviewCell>,
    pub layout: Option<PreviewLayout>,
}
```

Update `frame_from_buffer` to set `layout: None`. Update all direct
`PreviewFrame` test constructors in `src/dev_preview/export.rs` and
`src/dev_preview/scenarios.rs` to set `layout: None`.

- [ ] **Step 5: Write layout JSON, manifest entries, and artifacts**

In `src/dev_preview/export.rs`:

- set `SCHEMA_VERSION` to `2`,
- add `layout: Option<PathBuf>` to `PreviewScenarioFiles`,
- add `Layout` to `ArtifactType`,
- add `write_layout_json(path: &Path, layout: &PreviewLayout) -> Result<()>`,
- add `layout_path(frame)` returning `frames/{id}.layout.json`,
- for frames with `layout.is_some()`, write layout JSON and add `id: "{frame.id}-layout"` artifact with type `layout`.

- [ ] **Step 6: Add HTML overlay controls**

In `src/dev_preview/assets/preview.html`, add two buttons near the frame controls:

```html
<button type="button" data-overlay-toggle="components" aria-pressed="false">Components</button>
<button type="button" data-overlay-toggle="targets" aria-pressed="false">Targets</button>
```

In `src/dev_preview/export.rs`, render a sibling element for frames with layout:

```html
<div class="layout-overlay" data-layout-for="{frame_id}" hidden></div>
```

In `src/dev_preview/assets/preview.js`, wire the buttons to toggle `aria-pressed` and the `hidden` attribute on `.layout-overlay`.

In `src/dev_preview/assets/preview.css`, add visible outline styles for `.layout-overlay`, `.component-box`, and `.target-box`. The first pass can render empty overlay containers; Task 4 populates boxes after render consumes the artifact.

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test --test dev_preview --features dev-preview -- --nocapture
cargo test dev_preview::scenarios --features dev-preview -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/component/preview.rs src/tui/component/mod.rs src/dev_preview tests/dev_preview.rs
git commit -m "feat(dev-preview): export component layout artifacts"
```

### Task 4: Render, Effects, and Hit Testing Consume ComponentLayout

**Files:**

- Modify: `src/tui/component/watch_screen.rs`
- Modify: `src/tui/layout.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/dev_preview/watch.rs`
- Modify: `tests/tui_render.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing tests that prove no separate pet geometry path is needed**

In `tests/tui_render.rs`, add:

```rust
#[test]
fn pet_effect_target_matches_component_layout_pet_art_target() {
    let vm = WatchViewModel::fixture();
    let area = ratatui::layout::Rect::new(0, 0, 120, 32);
    let layout = glorp::tui::component::layout_watch(area, &vm);
    let target = layout
        .target(glorp::tui::component::TargetPath::new("watch.pet.art"))
        .expect("pet art target");
    let effect_rect = glorp::tui::layout::pet_effect_rect_for_test(area, &vm);
    assert_eq!(effect_rect, target.rect);
}
```

This test should fail until `pet_effect_rect_for_test` delegates to `layout_watch`.

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --test tui_render pet_effect_target_matches_component_layout_pet_art_target -- --nocapture
```

Expected: FAIL because `pet_effect_rect_for_test` does not exist.

- [ ] **Step 3: Add `render_watch_layout`**

In `src/tui/component/watch_screen.rs`, add:

```rust
use crate::tui::panels::{BioCardPanel, FeedPanel, Panel as LegacyPanel, PetPanel, ProgressPanel, TodayPanel, VitalsPanel};
use crate::tui::render_context::RenderContext;
use ratatui::buffer::Buffer;

pub fn render_watch_layout(
    layout: &ComponentLayout,
    buf: &mut Buffer,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    render_if_visible(layout, WatchComponentId::Pet.path(), buf, vm, ctx, PetPanel);
    render_if_visible(layout, WatchComponentId::Vitals.path(), buf, vm, ctx, VitalsPanel);
    render_if_visible(layout, WatchComponentId::Bio.path(), buf, vm, ctx, BioCardPanel);
    render_if_visible(layout, WatchComponentId::Today.path(), buf, vm, ctx, TodayPanel);
    render_if_visible(layout, WatchComponentId::Progress.path(), buf, vm, ctx, ProgressPanel);
    render_if_visible(layout, WatchComponentId::Feed.path(), buf, vm, ctx, FeedPanel);
}

fn render_if_visible<P: LegacyPanel>(
    layout: &ComponentLayout,
    id: ComponentPath,
    buf: &mut Buffer,
    vm: &WatchViewModel,
    ctx: &RenderContext,
    panel: P,
) {
    if let Some(node) = layout.node(id) {
        if matches!(node.visibility, VisibilityState::Visible | VisibilityState::Degraded { .. }) {
            panel.render(node.bounds, buf, vm, ctx);
        }
    }
}
```

Adjust names if the legacy `Panel` trait conflicts with the new widget `Panel`
introduced in Task 8. In this task, alias the old trait as `LegacyPanel`.

- [ ] **Step 4: Make `src/tui/layout.rs` render from the layout artifact**

In `render_watch_frame_with_context`:

1. keep frame title/footer `Block` rendering unchanged,
2. compute `let layout = layout_watch(frame.area(), vm);`,
3. render outer chrome at `layout.frame`,
4. call `render_watch_layout(&layout, frame.buffer_mut(), vm, ctx)`.

Keep `render_wide` and `render_compact` in place only until the task compiles; remove calls to them from production render path in this task.

- [ ] **Step 5: Replace effect geometry with ComponentLayout target lookup**

In `src/tui/layout.rs`, replace `pet_panel_rect` with:

```rust
pub fn pet_effect_rect(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    let layout = crate::tui::component::layout_watch(frame_area, vm);
    layout
        .target(crate::tui::component::TargetPath::new("watch.pet.art"))
        .map(|target| target.rect)
        .unwrap_or_else(|| Rect::new(frame_area.x, frame_area.y, 0, 0))
}

#[cfg(test)]
pub fn pet_effect_rect_for_test(frame_area: Rect, vm: &WatchViewModel) -> Rect {
    pet_effect_rect(frame_area, vm)
}
```

In `src/tui/app.rs`, change:

```rust
let pet_rect = pet_panel_rect(frame_area, vm_ref);
```

to:

```rust
let pet_rect = pet_effect_rect(frame_area, vm_ref);
```

- [ ] **Step 6: Update dev-preview to use the same layout artifact**

In `src/dev_preview/watch.rs`, compute:

```rust
let layout = crate::tui::component::layout_watch(Rect::new(0, 0, width, height), &vm);
terminal.draw(|frame| {
    render_watch_frame_with_context(frame, &vm, &ctx.render);
})?;
let mut frame = frame_from_buffer(id, title, terminal.backend().buffer());
frame.layout = Some(preview_layout(id, &layout));
```

Task 15 will remove any remaining compatibility geometry. This step makes the
exported artifact match the active render adapter.

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test --test tui_render pet_effect_target_matches_component_layout_pet_art_target -- --nocapture
cargo test --test tui_render wide_layout -- --nocapture
cargo test --test dev_preview --features dev-preview -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/component/watch_screen.rs src/tui/layout.rs src/tui/app.rs src/dev_preview/watch.rs tests/tui_render.rs tests/dev_preview.rs
git commit -m "feat(tui): render watch from ComponentLayout"
```

---

## Phase 2: PetScene Owns Bespoke Geometry

### Task 5: Add WatchClock and Deterministic Render Context

**Files:**

- Modify: `src/tui/render_context.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/tui/panels/pet.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write failing test for deterministic pet backdrop timing**

In `tests/tui_render.rs`, add:

```rust
#[test]
fn render_context_clock_controls_pet_scene_time() {
    let first = glorp::tui::render_context::WatchClock::fixed(
        time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap(),
    );
    let second = glorp::tui::render_context::WatchClock::fixed(
        time::OffsetDateTime::from_unix_timestamp(1_760_000_064).unwrap(),
    );
    assert_ne!(first.now_utc(), second.now_utc());
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --test tui_render render_context_clock_controls_pet_scene_time -- --nocapture
```

Expected: FAIL because `WatchClock` does not exist.

- [ ] **Step 3: Add `WatchClock` to `RenderContext`**

Update `src/tui/render_context.rs`:

```rust
use crate::tui::style::ColorCapability;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchClock {
    now_utc: OffsetDateTime,
}

impl WatchClock {
    pub fn live() -> Self {
        Self {
            now_utc: OffsetDateTime::now_utc(),
        }
    }

    pub const fn fixed(now_utc: OffsetDateTime) -> Self {
        Self { now_utc }
    }

    pub const fn now_utc(self) -> OffsetDateTime {
        self.now_utc
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderContext {
    pub color_capability: ColorCapability,
    pub clock: WatchClock,
}

impl RenderContext {
    pub fn new(color_capability: ColorCapability) -> Self {
        Self {
            color_capability,
            clock: WatchClock::live(),
        }
    }

    pub const fn with_clock(color_capability: ColorCapability, clock: WatchClock) -> Self {
        Self { color_capability, clock }
    }
}
```

Keep `from_environment` and `Default` by constructing `WatchClock::live()`.

- [ ] **Step 4: Replace direct pet render clock reads**

In `src/tui/panels/pet.rs`, replace:

```rust
let now = time::OffsetDateTime::now_utc();
```

with:

```rust
let now = ctx.clock.now_utc();
```

Change `render_pet_inside` to receive `ctx: &RenderContext` instead of `_ctx`.

In `src/dev_preview/scenarios.rs`, change deterministic context construction to:

```rust
render: RenderContext::with_clock(
    ColorCapability::Truecolor,
    WatchClock::fixed(fixed_now),
),
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test --test tui_render render_context_clock_controls_pet_scene_time -- --nocapture
cargo test --test dev_preview watch_frames_are_stable_for_fixed_time --features dev-preview -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/render_context.rs src/tui/app.rs src/dev_preview/scenarios.rs src/tui/panels/pet.rs tests/tui_render.rs
git commit -m "feat(tui): add deterministic watch clock"
```

### Task 6: Extract PetSceneLayout as the Single Pet Geometry Source

**Files:**

- Create: `src/tui/component/pet_scene.rs`
- Modify: `src/tui/component/mod.rs`
- Modify: `src/tui/component/watch_screen.rs`
- Modify: `src/tui/panels/pet.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write failing tests for PetScene layout invariants**

Add tests to `src/tui/component/pet_scene.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::ComponentPath;
    use crate::tui::render_context::{RenderContext, WatchClock};
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;
    use ratatui::layout::Rect;

    fn ctx() -> RenderContext {
        RenderContext::with_clock(
            ColorCapability::Truecolor,
            WatchClock::fixed(time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap()),
        )
    }

    #[test]
    fn pet_scene_layout_exposes_panel_art_habitat_and_hit_area() {
        let vm = WatchViewModel::fixture();
        let layout = PetScene::compute_layout(ComponentPath::new("watch.pet"), Rect::new(2, 3, 40, 18), &vm, &ctx());
        assert_eq!(layout.panel, Rect::new(2, 3, 40, 18));
        assert_eq!(layout.pet_art.unwrap().width, 13);
        assert_eq!(layout.pet_art.unwrap().height, 10);
        assert_eq!(layout.habitat, Rect::new(2, 3, 40, 18));
        assert!(layout.effect_targets.contains_key(&TargetPath::new("watch.pet.art")));
    }

    #[test]
    fn pet_scene_speech_target_is_optional() {
        let mut vm = WatchViewModel::fixture();
        vm.current_speech = None;
        let without = PetScene::compute_layout(ComponentPath::new("watch.pet"), Rect::new(0, 0, 40, 18), &vm, &ctx());
        assert!(without.speech.is_none());
        vm.current_speech = Some("hello".to_string());
        let with = PetScene::compute_layout(ComponentPath::new("watch.pet"), Rect::new(0, 0, 40, 18), &vm, &ctx());
        assert!(with.speech.is_some());
    }
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test pet_scene_layout --lib -- --nocapture
```

Expected: FAIL because `PetScene` does not exist.

- [ ] **Step 3: Add `PetSceneLayout` and `PetScene::compute_layout`**

Create `src/tui/component/pet_scene.rs`:

```rust
use crate::tui::component::{ComponentPath, GeometryTarget, TargetPath, TargetRole};
use crate::tui::panels::pet::pet_inner_rect_in_panel;
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetSceneLayout {
    pub panel: Rect,
    pub content: Rect,
    pub habitat: Rect,
    pub speech: Option<Rect>,
    pub pet_art: Option<Rect>,
    pub hit_area: Option<Rect>,
    pub exclusions: Vec<Rect>,
    pub effect_targets: BTreeMap<TargetPath, GeometryTarget>,
}

pub struct PetScene;

impl PetScene {
    pub fn compute_layout(
        id: ComponentPath,
        area: Rect,
        vm: &WatchViewModel,
        _ctx: &RenderContext,
    ) -> PetSceneLayout {
        let speech = if vm.current_speech.is_some() && area.height > 0 {
            Some(Rect::new(area.x, area.y, area.width, 1))
        } else {
            None
        };
        let speech_h = speech.map(|rect| rect.height).unwrap_or(0).min(area.height);
        let content = Rect::new(
            area.x,
            area.y + speech_h,
            area.width,
            area.height.saturating_sub(speech_h),
        );
        let pet_art = if content.width > 0 && content.height > 0 {
            Some(pet_inner_rect_in_panel(content, vm))
        } else {
            None
        };
        let mut effect_targets = BTreeMap::new();
        effect_targets.insert(
            TargetPath::new("watch.pet.panel"),
            GeometryTarget {
                owner: id,
                rect: area,
                z: 1,
                clip: area,
                role: TargetRole::PetPanel,
            },
        );
        if let Some(rect) = pet_art {
            effect_targets.insert(
                TargetPath::new("watch.pet.art"),
                GeometryTarget {
                    owner: id,
                    rect,
                    z: 10,
                    clip: content,
                    role: TargetRole::PetArt,
                },
            );
        }
        PetSceneLayout {
            panel: area,
            content,
            habitat: area,
            speech,
            pet_art,
            hit_area: Some(content),
            exclusions: pet_art.into_iter().collect(),
            effect_targets,
        }
    }
}
```

Export it in `src/tui/component/mod.rs`:

```rust
pub mod pet_scene;
pub use pet_scene::{PetScene, PetSceneLayout};
```

- [ ] **Step 4: Make `layout_watch` use `PetScene::compute_layout`**

In `src/tui/component/watch_screen.rs`, replace direct calls to `pet_inner_rect_in_panel` with `PetScene::compute_layout`. Insert each `effect_targets` entry into the top-level `ComponentLayout`.

- [ ] **Step 5: Make pet rendering consume PetSceneLayout**

In `src/tui/panels/pet.rs`, replace internal recomputation of speech area and pet art area with a call to:

```rust
let scene = crate::tui::component::PetScene::compute_layout(
    crate::tui::component::ComponentPath::new("watch.pet"),
    area,
    vm,
    ctx,
);
```

Use `scene.speech`, `scene.content`, `scene.pet_art`, and `scene.exclusions` for the existing render passes. Keep visible output unchanged.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test pet_scene --lib -- --nocapture
cargo test --test tui_render pet_renderer_roles_reach_tui_cells -- --nocapture
cargo test --test tui_render pet_effect_target_matches_component_layout_pet_art_target -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/tui/component/pet_scene.rs src/tui/component/mod.rs src/tui/component/watch_screen.rs src/tui/panels/pet.rs tests/tui_render.rs
git commit -m "feat(tui): extract PetScene geometry"
```

---

## Phase 3: Glorp Styling Facade and Ordinary Components

### Task 7: Add Lip Gloss-Class Style Facade

**Files:**

- Create: `src/tui/component/style.rs`
- Modify: `src/tui/component/mod.rs`
- Modify: `src/tui/style.rs`
- Modify: `tests/style_tokens.rs`

- [ ] **Step 1: Write failing tests for style facade tokens**

Add to `tests/style_tokens.rs`:

```rust
#[test]
fn component_style_facade_maps_semantic_tokens_to_ratatui_styles() {
    use glorp::tui::component::{
        BorderTone, ComponentStyle, GradientToken, Insets, Surface, TextTone,
    };
    use glorp::tui::style::tokenpet_palette;

    let p = tokenpet_palette();
    let style = ComponentStyle::new()
        .surface(Surface::Elevated)
        .border(BorderTone::Accent)
        .padding(Insets::horizontal(2))
        .text(TextTone::Primary)
        .gradient(GradientToken::Xp);

    assert_eq!(style.surface_style().bg, Some(p.surface.rgb));
    assert_eq!(style.border_style().fg, Some(p.accent.rgb));
    assert_eq!(style.text_style().fg, Some(p.fg.rgb));
    assert_eq!(style.insets().left, 2);
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --test style_tokens component_style_facade_maps_semantic_tokens_to_ratatui_styles -- --nocapture
```

Expected: FAIL because the facade types do not exist.

- [ ] **Step 3: Add style facade types**

Create `src/tui/component/style.rs`:

```rust
use crate::tui::style::tokenpet_palette;
use ratatui::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Insets {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Insets {
    pub const fn all(value: u16) -> Self {
        Self { top: value, right: value, bottom: value, left: value }
    }

    pub const fn horizontal(value: u16) -> Self {
        Self { top: 0, right: value, bottom: 0, left: value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Base,
    Elevated,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTone {
    Primary,
    Label,
    Subtle,
    Accent,
    Good,
    Bad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderTone {
    None,
    Subtle,
    Accent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientToken {
    Xp,
    Good,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentStyle {
    surface: Surface,
    text: TextTone,
    border: BorderTone,
    padding: Insets,
    gradient: Option<GradientToken>,
}

impl ComponentStyle {
    pub const fn new() -> Self {
        Self {
            surface: Surface::Empty,
            text: TextTone::Primary,
            border: BorderTone::None,
            padding: Insets::all(0),
            gradient: None,
        }
    }

    pub const fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    pub const fn text(mut self, text: TextTone) -> Self {
        self.text = text;
        self
    }

    pub const fn border(mut self, border: BorderTone) -> Self {
        self.border = border;
        self
    }

    pub const fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub const fn gradient(mut self, gradient: GradientToken) -> Self {
        self.gradient = Some(gradient);
        self
    }

    pub const fn gradient_token(self) -> Option<GradientToken> {
        self.gradient
    }

    pub fn surface_style(self) -> Style {
        let p = tokenpet_palette();
        match self.surface {
            Surface::Base => Style::default().bg(p.bg.rgb),
            Surface::Elevated => Style::default().bg(p.surface.rgb),
            Surface::Empty => Style::default(),
        }
    }

    pub fn text_style(self) -> Style {
        let p = tokenpet_palette();
        match self.text {
            TextTone::Primary => Style::default().fg(p.fg.rgb),
            TextTone::Label => Style::default().fg(p.dim.rgb),
            TextTone::Subtle => Style::default().fg(p.faint.rgb),
            TextTone::Accent => Style::default().fg(p.accent.rgb),
            TextTone::Good => Style::default().fg(p.good.rgb),
            TextTone::Bad => Style::default().fg(p.bad.rgb),
        }
    }

    pub fn border_style(self) -> Style {
        let p = tokenpet_palette();
        match self.border {
            BorderTone::None => Style::default(),
            BorderTone::Subtle => Style::default().fg(p.faint.rgb),
            BorderTone::Accent => Style::default().fg(p.accent.rgb),
        }
    }

    pub const fn insets(self) -> Insets {
        self.padding
    }
}

impl Default for ComponentStyle {
    fn default() -> Self {
        Self::new()
    }
}
```

The `gradient_token()` accessor is used by `ProgressBar` in Task 8, so the
facade carries gradient intent without requiring every panel callsite to know
bar color internals.

- [ ] **Step 4: Export facade types**

In `src/tui/component/mod.rs`, add:

```rust
pub mod style;
pub use style::{BorderTone, ComponentStyle, GradientToken, Insets, Surface, TextTone};
```

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test --test style_tokens component_style_facade_maps_semantic_tokens_to_ratatui_styles -- --nocapture
cargo test --test style_tokens -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/component/style.rs src/tui/component/mod.rs tests/style_tokens.rs
git commit -m "feat(tui): add component style facade"
```

### Task 8: Add Ordinary Component Widgets

**Files:**

- Create: `src/tui/component/widgets.rs`
- Modify: `src/tui/component/mod.rs`
- Modify: `src/tui/panels/bars.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write failing widget rendering tests**

Add tests to `src/tui/component/widgets.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::{BorderTone, ComponentStyle, Insets, Surface, TextTone};
    use crate::tui::render_context::RenderContext;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn panel_renders_title_border_padding_and_surface() {
        let ctx = RenderContext::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
        Panel::new("today")
            .style(ComponentStyle::new().surface(Surface::Elevated).border(BorderTone::Accent).padding(Insets::horizontal(1)))
            .render(Rect::new(0, 0, 30, 5), &mut buf, &ctx, |content, buf| {
                TextRow::new("tokens", "18.4k").tone(TextTone::Primary).render(content, buf, &ctx);
            });
        let text = buffer_text(&buf);
        assert!(text.contains("today"));
        assert!(text.contains("tokens"));
        assert!(text.contains("18.4k"));
    }

    fn buffer_text(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test panel_renders_title_border_padding_and_surface --lib -- --nocapture
```

Expected: FAIL because `Panel` widget does not exist in `component::widgets`.

- [ ] **Step 3: Add ordinary widget APIs**

Create `src/tui/component/widgets.rs` with these public structs and methods:

```rust
use crate::tui::component::{ComponentStyle, GradientToken, Insets, TextTone};
use crate::tui::panels::bars::{bar_spans, build_spark_line};
use crate::tui::render_context::RenderContext;
use crate::tui::style::semantic_styles;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct Panel {
    title: &'static str,
    style: ComponentStyle,
}

impl Panel {
    pub const fn new(title: &'static str) -> Self {
        Self { title, style: ComponentStyle::new() }
    }

    pub const fn style(mut self, style: ComponentStyle) -> Self {
        self.style = style;
        self
    }

    pub fn content_rect(&self, area: Rect) -> Rect {
        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(area);
        let padding = self.style.insets();
        Rect::new(
            inner.x + padding.left.min(inner.width),
            inner.y + padding.top.min(inner.height),
            inner.width.saturating_sub(padding.left + padding.right),
            inner.height.saturating_sub(padding.top + padding.bottom),
        )
    }

    pub fn render<F>(&self, area: Rect, buf: &mut Buffer, _ctx: &RenderContext, render_content: F)
    where
        F: FnOnce(Rect, &mut Buffer),
    {
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_style(self.style.border_style())
            .style(self.style.surface_style());
        let content = self.content_rect(area);
        block.render(area, buf);
        render_content(content, buf);
    }
}

pub struct TextRow<'a> {
    label: &'a str,
    value: String,
    tone: TextTone,
}

impl<'a> TextRow<'a> {
    pub fn new(label: &'a str, value: impl ToString) -> Self {
        Self { label, value: value.to_string(), tone: TextTone::Label }
    }

    pub const fn tone(mut self, tone: TextTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn line(&self) -> Line<'_> {
        let style = ComponentStyle::new().text(self.tone).text_style();
        Line::from(vec![
            Span::styled(format!("{} ", self.label), style),
            Span::styled(self.value.clone(), ComponentStyle::new().text(TextTone::Primary).text_style()),
        ])
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, _ctx: &RenderContext) {
        Paragraph::new(self.line()).render(area, buf);
    }
}
```

Add `StatRow`, `ProgressBar`, `InlineSparkline`, and `FeedList` in the same file. They should call existing `bar_spans`, `build_spark_line`, and feed line builders rather than reimplementing color math. The public API names must be:

```rust
pub struct StatRow<'a>;
pub struct ProgressBar;
pub struct InlineSparkline<'a>;
pub struct FeedList<'a>;
```

Each must expose a `render(area, buf, ctx)` method and a non-rendering builder method that tests can inspect (`line`, `spans`, or `lines`).

- [ ] **Step 4: Export widgets**

In `src/tui/component/mod.rs`, add:

```rust
pub mod widgets;
pub use widgets::{FeedList, InlineSparkline, Panel as ComponentPanel, ProgressBar, StatRow, TextRow};
```

Use `ComponentPanel` as the exported name to avoid conflict with the legacy `tui::panels::Panel` trait.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test panel_renders_title_border_padding_and_surface --lib -- --nocapture
cargo test bars --lib -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/component/widgets.rs src/tui/component/mod.rs src/tui/panels/bars.rs
git commit -m "feat(tui): add ordinary component widgets"
```

### Task 9: Migrate One Ordinary Panel as the Ergonomics Proof

**Files:**

- Modify: `src/tui/panels/progress.rs`
- Modify: `src/tui/component/widgets.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write failing proof test**

In `tests/tui_render.rs`, add:

```rust
#[test]
fn progress_panel_uses_component_style_facade() {
    let source = std::fs::read_to_string("src/tui/panels/progress.rs").unwrap();
    assert!(source.contains("ComponentPanel::new(\"progress\")"));
    assert!(source.contains("GradientToken::Xp"));
    assert!(!source.contains("Block::"));
}
```

This is intentionally a source-level proof. It guards the authoring ergonomics requirement directly.

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --test tui_render progress_panel_uses_component_style_facade -- --nocapture
```

Expected: FAIL because `progress.rs` still uses Ratatui `Block`/`Paragraph` plumbing directly.

- [ ] **Step 3: Rewrite `ProgressPanel` through component widgets**

In `src/tui/panels/progress.rs`, keep the `ProgressPanel` type and legacy `Panel` trait implementation, but make the body use:

```rust
use crate::tui::component::{
    BorderTone, ComponentPanel, ComponentStyle, GradientToken, Insets, ProgressBar, Surface,
    TextRow, TextTone,
};
```

Inside `render`:

```rust
let panel = ComponentPanel::new("progress").style(
    ComponentStyle::new()
        .surface(Surface::Elevated)
        .border(BorderTone::Accent)
        .padding(Insets::horizontal(1)),
);
panel.render(area, buf, ctx, |content, buf| {
    if vm.progress.is_max_stage {
        TextRow::new("stage", "max evolved")
            .tone(TextTone::Accent)
            .render(content, buf, ctx);
    } else {
        ProgressBar::new(vm.progress.fraction as f64)
            .gradient(GradientToken::Xp)
            .empty_tone(TextTone::Subtle)
            .render(content, buf, ctx);
    }
});
```

Preserve existing visible strings that tests assert on: `progress`, `xp`, percentage, and max-stage copy.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test --test tui_render progress_panel_uses_component_style_facade -- --nocapture
cargo test --test tui_render xp_display_caps_at_max_when_xp_overshoots_target -- --nocapture
cargo test progress --lib -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/progress.rs src/tui/component/widgets.rs tests/tui_render.rs
git commit -m "refactor(tui): migrate progress panel to components"
```

---

## Phase 4: Watch Composition and Degradation

### Task 10: Migrate Remaining Ordinary Panels

**Files:**

- Modify: `src/tui/panels/vitals.rs`
- Modify: `src/tui/panels/today.rs`
- Modify: `src/tui/panels/feed.rs`
- Modify: `src/tui/panels/bio_card.rs`
- Modify: `src/tui/component/widgets.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write failing source-level ergonomics tests**

Add to `tests/tui_render.rs`:

```rust
#[test]
fn ordinary_panels_use_component_widgets_instead_of_ratatui_blocks() {
    for path in [
        "src/tui/panels/vitals.rs",
        "src/tui/panels/today.rs",
        "src/tui/panels/feed.rs",
        "src/tui/panels/bio_card.rs",
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        assert!(source.contains("ComponentPanel::new"), "{path} should use ComponentPanel");
        assert!(!source.contains("Block::"), "{path} should not create Ratatui blocks directly");
    }
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --test tui_render ordinary_panels_use_component_widgets_instead_of_ratatui_blocks -- --nocapture
```

Expected: FAIL while panels still create Ratatui blocks directly.

- [ ] **Step 3: Migrate `VitalsPanel`**

Rewrite `src/tui/panels/vitals.rs` so it composes:

- `ComponentPanel::new("vitals")`,
- three `StatRow`s for fed, happy, energy,
- existing colors from `fed_color`, `happy_color`, `energy_color`,
- existing `ColorCapability` behavior through `ctx.color_capability`.

Do not change `preferred_constraint`; it stays `Constraint::Length(4)`.

- [ ] **Step 4: Migrate `TodayPanel`**

Rewrite `src/tui/panels/today.rs` so it composes:

- `ComponentPanel::new("today")`,
- `TextRow` rows for total, last bucket, source status,
- `InlineSparkline` for seven-day history,
- the existing source health marker behavior including `⚠`.

Do not change `preferred_constraint`; it stays the current length.

- [ ] **Step 5: Migrate `FeedPanel`**

Rewrite `src/tui/panels/feed.rs` so it composes:

- `ComponentPanel::new("feed")`,
- `FeedList` with the current bounded event count,
- existing rail/source colors from `SemanticStyles::log`.

Keep `MAX_EVENT_ROWS` behavior exactly as current tests expect.

- [ ] **Step 6: Migrate `BioCardPanel`**

Rewrite `src/tui/panels/bio_card.rs` so it composes:

- `ComponentPanel::new("bio")`,
- `TextRow::new("hatched", vm.bio.hatched_label.clone())`,
- `TextRow::new("age", vm.bio.age_label.clone())`.

Do not change `preferred_constraint`; it stays the current length.

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test --test tui_render ordinary_panels_use_component_widgets_instead_of_ratatui_blocks -- --nocapture
cargo test --test tui_render wide_layout_has_tokenpet_chrome_panels_and_bars -- --nocapture
cargo test --test tui_render source_health_rows_render_ready_and_diagnostic_states_together -- --nocapture
cargo test --test tui_render event_log_uses_timestamps_rails_sparkline_and_semantic_colors -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/panels/vitals.rs src/tui/panels/today.rs src/tui/panels/feed.rs src/tui/panels/bio_card.rs src/tui/component/widgets.rs tests/tui_render.rs
git commit -m "refactor(tui): migrate ordinary panels to component widgets"
```

### Task 11: Replace Slat Alignment With WatchScreen Composition and Degradation Decisions

**Files:**

- Modify: `src/tui/component/watch_screen.rs`
- Modify: `src/tui/component/sizing.rs`
- Modify: `src/tui/layout.rs`
- Modify: `tests/tui_render.rs`
- Modify: `src/tui/component/geometry.rs`

- [ ] **Step 1: Write failing layout-policy tests**

Replace the old `vitals_and_feed_start_on_the_same_row` test in `src/tui/layout.rs` or `tests/tui_render.rs` with:

```rust
#[test]
fn wide_layout_preserves_asymmetric_pet_hero_policy() {
    let vm = WatchViewModel::fixture();
    let layout = glorp::tui::component::layout_watch(ratatui::layout::Rect::new(0, 0, 120, 32), &vm);
    let pet = layout.node(glorp::tui::component::WatchComponentId::Pet.path()).unwrap();
    let today = layout.node(glorp::tui::component::WatchComponentId::Today.path()).unwrap();
    let progress = layout.node(glorp::tui::component::WatchComponentId::Progress.path()).unwrap();
    let feed = layout.node(glorp::tui::component::WatchComponentId::Feed.path()).unwrap();

    assert!(pet.bounds.height >= today.bounds.height + progress.bounds.height);
    assert!(today.bounds.y <= progress.bounds.y);
    assert!(progress.bounds.y <= feed.bounds.y);
    assert!(feed.bounds.height <= 8);
}

#[test]
fn compact_layout_records_ordered_degradation_decisions() {
    let vm = WatchViewModel::fixture();
    let layout = glorp::tui::component::layout_watch(ratatui::layout::Rect::new(0, 0, 72, 24), &vm);
    assert!(layout.decisions.iter().any(|d| d.path.as_str() == "watch.bio"));
    assert!(layout.target(glorp::tui::component::TargetPath::new("watch.pet.art")).is_some());
}
```

- [ ] **Step 2: Run failing/replaced tests**

Run:

```bash
cargo test --test tui_render wide_layout_preserves_asymmetric_pet_hero_policy -- --nocapture
cargo test --test tui_render compact_layout_records_ordered_degradation_decisions -- --nocapture
```

Expected: FAIL until `layout_watch` no longer encodes the old two-band row alignment as policy.

- [ ] **Step 3: Update `layout_wide` policy**

In `src/tui/component/watch_screen.rs`, change wide allocation to:

```text
left column:
  pet scene: Fill(1), minimum 10 rows
  gap
  vitals: intrinsic current height
  gap
  bio: intrinsic current height

right column:
  today: intrinsic current height
  gap
  progress: intrinsic current height
  gap
  feed: bounded intrinsic list
  trailing space accepted
```

Use Ratatui `Layout` for this task. The visible effect is that feed no longer has to start on the same row as vitals.

- [ ] **Step 4: Record degradation decisions centrally**

Update `layout_compact` to insert `LayoutDecision` entries for:

- `watch.bio` hidden because compact mode,
- `watch.feed` row limit when feed rows exceed bounded visible rows,
- `watch.pet.speech` hidden only when height pressure requires it.

Do not hide `watch.pet.art` at `72x24`.

- [ ] **Step 5: Remove slat-alignment tests**

Delete or rewrite tests whose only assertion is:

- vitals starts on the same terminal row as feed,
- bio bottom aligns with feed bottom,
- all section dividers in wide mode end at the same right column when the product invariant is not readability.

Keep tests for:

- frame draws,
- required content appears,
- pet art target exists,
- feed bounded,
- compact critical content visible.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test --test tui_render wide_layout -- --nocapture
cargo test layout_watch --lib -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/tui/component/watch_screen.rs src/tui/component/sizing.rs src/tui/layout.rs tests/tui_render.rs
git commit -m "refactor(tui): encode watch composition policy"
```

### Task 12: Remove Wide Height Cap and Add Tall-Wide Preview

**Files:**

- Modify: `src/tui/component/watch_screen.rs`
- Modify: `src/tui/layout.rs`
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `tests/dev_preview.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write failing tall-wide tests**

Add to `tests/dev_preview.rs`:

```rust
#[test]
fn dev_preview_watch_includes_real_tall_wide_frame() {
    let run = PreviewRun::new();
    run.run_success("watch");

    assert!(run.out.join("frames/watch-tall-wide.txt").is_file());
    assert!(run.out.join("frames/watch-tall-wide.layout.json").is_file());

    let manifest = run.manifest();
    assert_scenario(
        &manifest,
        "watch-tall-wide",
        "watch",
        180,
        50,
        "frames/watch-tall-wide.txt",
        "frames/watch-tall-wide.cells.json",
    );
    let layout: Value = serde_json::from_str(
        &std::fs::read_to_string(run.out.join("frames/watch-tall-wide.layout.json")).unwrap(),
    ).unwrap();
    assert_eq!(layout["frame"]["height"], 50);
    assert!(layout["components"]["watch.pet"]["height"].as_u64().unwrap() > 18);
}
```

Add to `tests/tui_render.rs`:

```rust
#[test]
fn wide_frame_height_grows_on_tall_terminals() {
    let vm = WatchViewModel::fixture();
    let layout = glorp::tui::component::layout_watch(ratatui::layout::Rect::new(0, 0, 180, 50), &vm);
    assert_eq!(layout.frame.height, 50);
    assert!(layout.node(glorp::tui::component::WatchComponentId::Pet.path()).unwrap().bounds.height > 18);
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test --test tui_render wide_frame_height_grows_on_tall_terminals -- --nocapture
cargo test --test dev_preview dev_preview_watch_includes_real_tall_wide_frame --features dev-preview -- --nocapture
```

Expected: FAIL because wide mode still caps height at 23 and Preview Lab has no tall-wide frame.

- [ ] **Step 3: Remove or replace wide height cap**

In `src/tui/component/watch_screen.rs`, change `bounded_frame_rect` so wide mode caps width but not height:

```rust
let height = terminal_area.height;
```

Keep width cap at `MAX_FRAME_WIDTH` unless a test proves the centered 110-column frame is wrong. The spec only requires tall height to flow into the pet scene.

- [ ] **Step 4: Add tall-wide preview frame**

In `src/dev_preview/watch.rs`, add:

```rust
render_watch_frame(
    "watch-tall-wide",
    "Watch Tall Wide",
    180,
    50,
    ctx,
    scratch_dir,
)?,
```

In `src/dev_preview/scenarios.rs`, add metadata for `watch-tall-wide` with terminal width `180` and height `50`, and review prompts asking whether the pet scene absorbs vertical slack.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test --test tui_render wide_frame_height_grows_on_tall_terminals -- --nocapture
cargo test --test dev_preview dev_preview_watch_includes_real_tall_wide_frame --features dev-preview -- --nocapture
cargo test --test dev_preview dev_preview_watch_wide_normal_frame_snapshot --features dev-preview -- --nocapture
```

Expected: PASS. If the snapshot changes intentionally, run `cargo insta review` and accept only the watch snapshot affected by the layout policy change.

- [ ] **Step 6: Commit**

```bash
git add src/tui/component/watch_screen.rs src/tui/layout.rs src/dev_preview/watch.rs src/dev_preview/scenarios.rs tests/dev_preview.rs tests/tui_render.rs tests/snapshots
git commit -m "feat(tui): add tall-wide watch layout preview"
```

---

## Phase 5: Taffy Behind Containers

### Task 13: Add Taffy Dependency and Backend Tests

**Files:**

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `src/tui/component/taffy_backend.rs`
- Modify: `src/tui/component/mod.rs`

- [ ] **Step 1: Add dependency**

In `Cargo.toml`, add:

```toml
taffy = { version = "0.10.1", default-features = false, features = ["std", "taffy_tree", "flexbox"] }
```

Do not add `grid`.

- [ ] **Step 2: Write failing backend tests**

Create `src/tui/component/taffy_backend.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn taffy_backend_conserves_sibling_widths_with_odd_remainders() {
        let allocated = allocate_columns(Rect::new(0, 0, 101, 10), &[ColumnSpec::fixed(40), ColumnSpec::fixed(4), ColumnSpec::fill(1)]);
        assert_eq!(allocated[0], Rect::new(0, 0, 40, 10));
        assert_eq!(allocated[1], Rect::new(40, 0, 4, 10));
        assert_eq!(allocated[2], Rect::new(44, 0, 57, 10));
        assert_eq!(allocated.iter().map(|r| r.width).sum::<u16>(), 101);
    }

    #[test]
    fn taffy_backend_clips_children_to_parent() {
        let allocated = allocate_stack(Rect::new(5, 5, 20, 6), &[RowSpec::fixed(10), RowSpec::fill(1)]);
        assert!(allocated.iter().all(|rect| rect.x >= 5));
        assert!(allocated.iter().all(|rect| rect.y >= 5));
        assert!(allocated.iter().all(|rect| rect.y + rect.height <= 11));
    }
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test taffy_backend --lib -- --nocapture
```

Expected: FAIL because backend functions do not exist.

- [ ] **Step 4: Implement backend wrapper**

Implement public wrapper types in `src/tui/component/taffy_backend.rs`:

```rust
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnSpec {
    Fixed(u16),
    Fill(u16),
}

impl ColumnSpec {
    pub const fn fixed(width: u16) -> Self { Self::Fixed(width) }
    pub const fn fill(weight: u16) -> Self { Self::Fill(weight) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowSpec {
    Fixed(u16),
    Fill(u16),
}

impl RowSpec {
    pub const fn fixed(height: u16) -> Self { Self::Fixed(height) }
    pub const fn fill(weight: u16) -> Self { Self::Fill(weight) }
}

pub fn allocate_columns(area: Rect, specs: &[ColumnSpec]) -> Vec<Rect> {
    // Use Taffy internally, then convert final float boxes to integer Rects.
    // Preserve total width by assigning rounding remainder to the last fill child.
    allocate_axis(area.x, area.width, specs.iter().map(axis_column).collect(), |x, width| {
        Rect::new(x, area.y, width, area.height)
    })
}

pub fn allocate_stack(area: Rect, specs: &[RowSpec]) -> Vec<Rect> {
    allocate_axis(area.y, area.height, specs.iter().map(axis_row).collect(), |y, height| {
        Rect::new(area.x, y, area.width, height)
    })
}
```

The implementation must use Taffy for flex calculation, but Glorp owns the final integer conservation. Keep all Taffy types private to this module.

- [ ] **Step 5: Export backend**

In `src/tui/component/mod.rs`, add:

```rust
pub mod taffy_backend;
```

Do not re-export Taffy crate types.

- [ ] **Step 6: Run checks**

Run:

```bash
cargo test taffy_backend --lib -- --nocapture
cargo tree -e features | rg "taffy|grid|flexbox|taffy_tree"
```

Expected: tests PASS; feature tree shows `flexbox` and `taffy_tree`, and does not show a `grid` feature enabled for `taffy`.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/tui/component/taffy_backend.rs src/tui/component/mod.rs
git commit -m "feat(tui): add Taffy-backed container allocator"
```

### Task 14: Route WatchColumns and Stack Through the Taffy Backend

**Files:**

- Modify: `src/tui/component/watch_screen.rs`
- Modify: `src/tui/component/taffy_backend.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write tests for Taffy-backed watch invariants**

Add to `tests/tui_render.rs`:

```rust
#[test]
fn component_layout_has_no_overlapping_required_components() {
    let vm = WatchViewModel::fixture();
    let layout = glorp::tui::component::layout_watch(ratatui::layout::Rect::new(0, 0, 120, 32), &vm);
    let ids = [
        glorp::tui::component::WatchComponentId::Pet.path(),
        glorp::tui::component::WatchComponentId::Vitals.path(),
        glorp::tui::component::WatchComponentId::Today.path(),
        glorp::tui::component::WatchComponentId::Progress.path(),
        glorp::tui::component::WatchComponentId::Feed.path(),
    ];
    for (i, a) in ids.iter().enumerate() {
        for b in ids.iter().skip(i + 1) {
            let a = layout.node(*a).unwrap().bounds;
            let b = layout.node(*b).unwrap().bounds;
            assert!(!rects_overlap(a, b), "components overlap: {a:?} and {b:?}");
        }
    }
}
```

Add a local `rects_overlap` helper in the test file.

- [ ] **Step 2: Run tests before routing**

Run:

```bash
cargo test --test tui_render component_layout_has_no_overlapping_required_components -- --nocapture
```

Expected: PASS with current adapter. This locks the invariant before changing backend.

- [ ] **Step 3: Route wide columns through `allocate_columns`**

In `src/tui/component/watch_screen.rs`, replace the Ratatui horizontal wide split with:

```rust
let columns = crate::tui::component::taffy_backend::allocate_columns(
    padded,
    &[
        ColumnSpec::fixed(WIDE_LEFT_COL),
        ColumnSpec::fixed(WIDE_GUTTER),
        ColumnSpec::fill(1),
    ],
);
```

- [ ] **Step 4: Route left and right stacks through `allocate_stack`**

Use `RowSpec::fill(1)` for the pet row and `RowSpec::fixed(...)` for ordinary panel rows. For the right column, keep today/progress/feed top-packed and let the final trailing area remain unused.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test --test tui_render component_layout_has_no_overlapping_required_components -- --nocapture
cargo test layout_watch --lib -- --nocapture
cargo test --test dev_preview --features dev-preview -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/component/watch_screen.rs src/tui/component/taffy_backend.rs tests/tui_render.rs
git commit -m "refactor(tui): route watch containers through Taffy"
```

---

## Phase 6: Remove Old Patterns and Prove the New Authoring Model

### Task 15: Delete Obsolete Layout and Panel Plumbing

**Files:**

- Modify: `src/tui/layout.rs`
- Modify: `src/tui/panels/mod.rs`
- Modify: `src/tui/component/watch_screen.rs`
- Modify: `src/tui/component/widgets.rs`
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Write source-level deletion tests**

Add to `tests/tui_render.rs`:

```rust
#[test]
fn watch_layout_no_longer_contains_manual_render_wide_or_pet_panel_rect() {
    let source = std::fs::read_to_string("src/tui/layout.rs").unwrap();
    assert!(!source.contains("fn render_wide("));
    assert!(!source.contains("fn render_compact("));
    assert!(!source.contains("pub fn pet_panel_rect("));
    assert!(source.contains("layout_watch("));
    assert!(source.contains("render_watch_layout("));
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
cargo test --test tui_render watch_layout_no_longer_contains_manual_render_wide_or_pet_panel_rect -- --nocapture
```

Expected: FAIL until old functions are removed.

- [ ] **Step 3: Remove old render functions and constants from `src/tui/layout.rs`**

Keep only:

- outer frame rendering,
- overlays,
- test helper functions that render through `layout_watch` and `render_watch_layout`.

Move constants that belong to layout policy into `src/tui/component/watch_screen.rs`.

- [ ] **Step 4: Rename legacy panel trait to avoid `Panel` confusion**

In `src/tui/panels/mod.rs`, rename the legacy trait:

```rust
pub trait LegacyPanel {
    fn preferred_constraint(&self, vm: &WatchViewModel) -> Constraint;
    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel, ctx: &RenderContext);
}
```

Update impls across `src/tui/panels/*.rs`. This keeps `component::ComponentPanel` as the normal authoring surface and makes remaining compatibility code obvious.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test --test tui_render watch_layout_no_longer_contains_manual_render_wide_or_pet_panel_rect -- --nocapture
cargo test --test tui_render -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/layout.rs src/tui/panels src/tui/component tests/tui_render.rs
git commit -m "refactor(tui): remove obsolete watch layout plumbing"
```

### Task 16: Final Verification and Preview Review

**Files:**

- Modify only files required by failures found in this task.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS. If it fails, run `cargo fmt`, inspect `git diff`, and commit formatting with the code task that caused it.

- [ ] **Step 2: Run clippy**

Run:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 3: Run locked feature checks**

Run:

```bash
cargo test --locked
cargo test --locked --no-default-features --all-targets
cargo build --release --locked --no-default-features
```

Expected: PASS. This verifies `dev-preview` remains excluded from release builds and Taffy compiles under the release feature set.

- [ ] **Step 4: Run npm checks**

Run:

```bash
npm test
```

Expected: PASS.

- [ ] **Step 5: Generate Preview Lab artifacts**

Run:

```bash
cargo run -- dev-preview --scenario watch --out target/glorp-preview
```

Expected: PASS and output includes:

- `target/glorp-preview/frames/watch-wide-normal.layout.json`
- `target/glorp-preview/frames/watch-compact-normal.layout.json`
- `target/glorp-preview/frames/watch-tall-wide.layout.json`
- `target/glorp-preview/index.html`

- [ ] **Step 6: Inspect generated layout artifact values**

Run:

```bash
jq '.schema_version, .components["watch.pet"], .targets["watch.pet.art"], .decisions' \
  target/glorp-preview/frames/watch-tall-wide.layout.json
```

Expected:

- schema version is `1`,
- `watch.pet.height` is greater than `18`,
- `watch.pet.art.width` is `13`,
- decisions array is present.

- [ ] **Step 7: Commit verification fixes if any were needed**

If Step 1 through Step 6 required code changes:

```bash
git add <changed-files>
git commit -m "fix(tui): finalize component layout verification"
```

If no code changes were required, do not create an empty commit.

---

## Implementation Notes

- Do not add `lipgloss-rs` as a dependency in this implementation plan. The spec goal is Lip Gloss-class ergonomics through a Glorp facade while rendering stays Ratatui-buffer-native.
- Do not expose Taffy types outside `src/tui/component/taffy_backend.rs`.
- Do not let Preview Lab recompute layout separately from `layout_watch`.
- Do not keep `pet_panel_rect` as an independent geometry function. If a compatibility helper exists during migration, it must delegate to `ComponentLayout`.
- Do not preserve cross-column slat alignment tests unless Drew explicitly re-approves that behavior as product policy.
- Keep compact `72x24` pet art visible.
- Keep `dev-preview` gated out of no-default-feature release builds.

## Final Acceptance Checklist

- [ ] `layout_watch(area, vm) -> ComponentLayout` is the pre-render geometry source.
- [ ] `render_watch_layout`, `tachyonfx`, hit testing, and Preview Lab consume the same `ComponentLayout`.
- [ ] No independent `pet_panel_rect` implementation remains.
- [ ] Preview Lab exports layout JSON for wide, compact, and tall-wide watch frames.
- [ ] Preview Lab HTML has component/target overlay controls.
- [ ] `PetScene` owns pet art, speech, habitat, exclusions, hit area, and effect targets.
- [ ] Ordinary panels render through Glorp component widgets and style facade.
- [ ] Taffy is hidden behind Glorp container allocation.
- [ ] Tall-wide `180x50` gives vertical slack to the pet scene.
- [ ] All commands in Task 16 pass.
