# Taffy Watch Layout Engine - Design

**Status**: Design approved in conversation, written for Drew review.
**Source**: Layout architecture brainstorm on 2026-05-13.

## Problem

The current watch layout is too manual for the direction Glorp is going. The
top-level Ratatui layout code hand-authors fixed bands, gutters, and repeated
split trees. That makes simple visual changes expensive: aligning rows, adding
panels, resizing dynamically, and scoping effects all require touching geometry
by hand.

The immediate code smell is that rendering and effect geometry are computed in
separate paths. `pet_panel_rect()` mirrors the wide/compact layout logic so
`tachyonfx` can scope to the pet. That duplication is fragile. Future dynamic
backgrounds, panel reordering, and responsive behavior would make the problem
worse.

## Goal

Replace the manual Ratatui split tree with a local Glorp layout engine backed by
Taffy. Ratatui remains the terminal renderer. Taffy owns rectangular allocation.
Glorp owns terminal semantics: borders, text truncation, panel visibility,
background layers, pet-art sub-rects, and effect scopes.

The new layout layer should make these changes straightforward:

- Add a panel without rewriting row math.
- Reorder or hide panels through data-shaped Rust structs.
- Resize dynamically across wide and compact terminals.
- Attach dynamic backgrounds to named regions.
- Reuse one computed geometry model for rendering, previews, mouse hit testing,
  and `tachyonfx`.

## Non-Goals

- No full external app framework migration in this revision.
- No user-facing layout configuration yet.
- No plugin system, theme system, or serialized layout format yet.
- No new watch key bindings, commands, persisted state, or panel behavior.
- No rewrite of existing panel renderers unless the layout boundary requires a
  narrow adapter.
- No visible redesign beyond preserving the currently intended panel hierarchy
  under a better layout engine.

## Decision

Use Taffy as a dependency behind a Glorp-owned `layout_engine` module.

Do not spread Taffy nodes through panel renderers. Panels render into rectangles
they are given. The layout engine produces named rectangles and named layers.
This keeps Taffy replaceable, keeps Ratatui tests useful, and makes the layout
tree inspectable in Preview Lab.

## Architecture

The watch pipeline becomes:

```text
terminal area
  -> bounded outer frame
  -> layout_engine::compute_watch_layout(area, vm, profile)
  -> ComputedWatchLayout
  -> render chrome, backgrounds, panels, overlays, effects
```

The renderer keeps using Ratatui widgets and buffers. The layout engine is pure:
given an input rectangle, view model, and layout profile, it returns computed
rectangles without drawing.

### Core Types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegionId {
    Pet,
    Today,
    Progress,
    Feed,
    Vitals,
    Bio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayerId {
    HabitatBackground,
    PanelContent,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectId {
    PetArt,
    PetPanel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedWatchLayout {
    pub frame: Rect,
    pub content: Rect,
    pub regions: BTreeMap<RegionId, Rect>,
    pub layers: BTreeMap<LayerId, Rect>,
    pub effects: BTreeMap<EffectId, Rect>,
}
```

Convenience accessors should return `Option<Rect>` for regions that can be
hidden in compact mode, and exact `Rect` for required frame/content geometry.

### Layout Spec

The first implementation is code-configurable, not user-configurable. The
structs should still be data-shaped so future theme/user/plugin work does not
require rewriting the core engine.

```rust
pub struct WatchLayoutSpec {
    pub mode: LayoutMode,
    pub regions: Vec<RegionSpec>,
}

pub struct RegionSpec {
    pub id: RegionId,
    pub min_size: SizeSpec,
    pub preferred_size: Option<SizeSpec>,
    pub flex_grow: f32,
    pub order: u16,
    pub visible_when: VisibilityRule,
}

pub enum VisibilityRule {
    Always,
    WideOnly,
    CompactOnly,
}
```

The initial spec can be built in Rust functions:

```rust
fn wide_watch_spec(vm: &WatchViewModel) -> WatchLayoutSpec;
fn compact_watch_spec(vm: &WatchViewModel) -> WatchLayoutSpec;
```

This is intentionally not serialized. Serialization becomes a later product
decision if Glorp needs user-editable layouts.

## Wide Layout

The wide layout uses a Taffy tree equivalent to:

```text
content
  row
    left-column
      pet      flex-grow: 1
      vitals   fixed/min height
      bio      fixed/min height
    right-column
      today    fixed/min height
      progress fixed/min height
      feed     bounded/min height
```

Policy:

- The pet region is the primary flexible region.
- The habitat background attaches to the pet region.
- Vitals and bio stay visually grouped below the pet.
- The right column packs today, progress, and feed from the top.
- Feed remains bounded; it should not expand into scrollback just because space
  exists.
- Outer frame width/height caps remain a Glorp policy, not a Taffy concern.

## Compact Layout

The compact layout uses a single Taffy column:

```text
content
  pet
  vitals
  today
  progress
  feed
```

`Bio` remains hidden in compact mode for now because age is already visible in
the title and hatched date is lower-priority. This becomes a visibility rule,
not imperative branching scattered through rendering.

## Dynamic Backgrounds And Effects

Backgrounds and effects consume computed layout, not duplicated geometry.

The habitat background attaches to `RegionId::Pet`. The actual creature art is
computed as `EffectId::PetArt`, a 13x10 terminal-cell sub-rect inside the pet
region after speech-row, breathing, and wander offsets are applied.

```rust
HabitatBackground {
    region: RegionId::Pet,
    exclusion: EffectId::PetArt,
}
```

`tachyonfx` uses `EffectId::PetArt` or `EffectId::PetPanel` depending on the
effect. Mouse hit testing and Preview Lab use the same effect rectangles.

## Data Flow

```text
render_watch_frame_with_context
  -> bounded_frame_rect
  -> layout_engine::compute_watch_layout
       -> select wide/compact profile
       -> build Taffy tree
       -> compute node layout
       -> convert Taffy boxes to Ratatui Rects
       -> compute semantic sub-rects and layers
  -> draw frame chrome
  -> render backgrounds by layer
  -> render panels by RegionId
  -> render overlays/effects by EffectId
```

The panel trait can stay close to its current shape. The layout engine should
ask panels or panel metadata for sizing, but panels should not know about Taffy.

## Error Handling

Layout computation should be deterministic and non-panicking for small
terminals.

- Missing optional regions return `None`.
- Required rectangles clamp to zero-sized `Rect`s when the terminal is too
  small.
- Taffy compute errors are converted into a Glorp TUI layout error at the
  engine boundary.
- Rendering skips panels whose computed rectangle cannot fit their honest
  minimum height.

The user should not see a runtime error for ordinary terminal resizing. At
worst, Glorp should degrade by hiding low-priority panels or rendering fewer
rows.

## Preview Lab

Preview Lab should export the computed layout model, not just rendered cells.

`manifest.json` gains a layout section for each frame:

```json
{
  "layout": {
    "frame": { "x": 0, "y": 0, "width": 110, "height": 23 },
    "regions": {
      "pet": { "x": 2, "y": 2, "width": 40, "height": 12 },
      "today": { "x": 46, "y": 2, "width": 62, "height": 6 }
    },
    "effects": {
      "pet_art": { "x": 15, "y": 3, "width": 13, "height": 10 }
    }
  }
}
```

This makes layout review inspectable. If a panel looks wrong, the artifact can
show whether the problem is allocation, rendering, or content.

## Testing

Add tests at three levels:

- Pure layout tests for wide/compact `ComputedWatchLayout` at representative
  sizes: `120x32`, `72x24`, tall wide, very small terminal.
- Render tests using Ratatui `TestBackend` to verify panels still land in the
  named regions and clipped panels do not corrupt neighboring content.
- Preview tests ensuring `manifest.json` includes named regions, layers, and
  effect rectangles for each watch scenario.

Existing panel tests should remain mostly unchanged. The layout tests replace
assertions that depend on incidental row math.

## Migration Plan

1. Add `taffy` to `Cargo.toml`.
2. Add `src/tui/layout_engine.rs` with IDs, specs, and computed layout types.
3. Port current wide and compact layout intent into Taffy specs.
4. Rewrite `src/tui/layout.rs` so it renders from `ComputedWatchLayout`.
5. Replace `pet_panel_rect()` mirrored split logic with lookup of
   `EffectId::PetArt`.
6. Add Preview Lab layout metadata to `manifest.json`.
7. Update layout/render tests around named geometry.
8. Delete obsolete band constants and duplicated split code.

This should be one coherent refactor. It does not need a throwaway spike because
the current manual layout layer is already the wrong abstraction. The fallback
plan is still straightforward: if Taffy creates more adapter complexity than it
removes, the same `ComputedWatchLayout` API can be backed by a local Ratatui
layout DSL instead.

## Acceptance Criteria

- Adding a new watch panel requires adding region metadata and a render call,
  not rewriting wide/compact row math.
- Rendering, effect scoping, mouse hit testing, and Preview Lab all consume the
  same computed rectangles.
- The pet habitat fills the pet region and excludes the pet-art rectangle.
- Wide and compact layouts preserve the current panel hierarchy.
- Tall and narrow terminal behavior is covered by pure layout tests.
- `src/tui/layout.rs` no longer contains duplicated geometry for render and
  effect paths.
