# Watch Component System - Design

**Status**: Revised design after staff-review feedback and Drew direction.
**Source**: Layout architecture brainstorm on 2026-05-13.
**Supersedes**: `2026-05-13-taffy-watch-layout-engine-design.md`.

## Problem

The current watch code makes ordinary terminal UI work feel like custom graphics
programming. Most watch panels are text, rows, progress bars, lists, or
sparklines, yet changing them requires hand-adjusting panes and row math in the
top-level layout.

That is the wrong authoring model for Glorp. Future agents should spend their
attention on product behavior and the one genuinely special surface: the pet
scene. They should not spend the bulk of a change aligning slats.

The old Taffy-first design attacked the symptom one layer too low. Taffy can be
the layout backend, but the missing abstraction is a Glorp component system:
panels, rows, widgets, layout intent, and semantic geometry.

## Goal

Create a Rust-native component system for the watch UI. The component system is
the public authoring surface. Ratatui remains the rendering backend. Taffy is an
internal layout allocator used by container components when it earns its keep.

The authoring experience should make this natural:

```rust
WatchScreen::new(vm)
    .wide(
        Grid::new()
            .area("pet", PetScene::new(vm))
            .area("today", TodayPanel::new(vm))
            .area("progress", ProgressPanel::new(vm))
            .area("feed", FeedPanel::new(vm))
            .area("vitals", VitalsPanel::new(vm))
            .area("bio", BioPanel::new(vm)),
    )
    .compact(
        Stack::new()
            .child(PetScene::new(vm))
            .child(VitalsPanel::new(vm))
            .child(TodayPanel::new(vm))
            .child(ProgressPanel::new(vm))
            .child(FeedPanel::new(vm)),
    );
```

The exact syntax can change. The design target should not.

## Non-Goals

- No full external app framework migration in this revision.
- No user-facing layout configuration yet.
- No plugin system, theme system, or serialized layout format yet.
- No new watch key bindings, commands, persisted state, or panel behavior.
- No visible redesign beyond preserving the approved watch hierarchy.
- No attempt to make every component infinitely generic before the watch has a
  second screen that needs it.

## Decision

Build a small local Glorp component system over Ratatui. Use imported Ratatui
widgets or thin local wrappers for ordinary UI primitives. Treat `PetScene` as
the special component.

Taffy remains likely, but it is not the center of the architecture. It is an
implementation detail of container measurement/allocation. If a local allocator
or Ratatui-backed layout pass is simpler for the first migration slice, the
component API should still survive unchanged.

## Architecture

The watch pipeline becomes:

```text
WatchViewModel
  -> WatchScreen component tree
  -> measure/layout pass
  -> ComponentLayout
  -> render pass
  -> effect/hit-test/preview geometry
```

Ratatui owns terminal buffers, styles, and widget rendering. Glorp components
own the UI structure and semantic geometry. Taffy, if used, owns only box
allocation under container components.

### Core Component Contract

```rust
pub trait Component {
    fn id(&self) -> ComponentId;
    fn sizing(&self, ctx: &MeasureContext) -> ComponentSizing;
    fn render(&self, area: Rect, buf: &mut Buffer, ctx: &RenderContext);

    fn geometry(
        &self,
        area: Rect,
        ctx: &GeometryContext,
    ) -> ComponentGeometry {
        ComponentGeometry::default_for(area)
    }
}
```

Most components use default geometry. `PetScene` overrides it to expose the pet
art rectangle, habitat region, speech region, and effect targets.

### Component Sizing

Sizing is Glorp-native and independent of Taffy.

```rust
pub struct ComponentSizing {
    pub min_width: u16,
    pub min_height: u16,
    pub preferred_width: Option<u16>,
    pub preferred_height: Option<u16>,
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub can_hide: bool,
    pub hide_priority: u16,
}
```

Panels expose sizing in terminal cells. Containers translate this into their
chosen allocator. If the backend is Taffy, this struct maps to Taffy styles. If
the backend is a parity Ratatui layout pass, this struct still works.

### Component Geometry

Geometry is the shared truth for rendering-adjacent behavior.

```rust
pub struct ComponentGeometry {
    pub bounds: Rect,
    pub content: Rect,
    pub targets: BTreeMap<GeometryId, Rect>,
    pub paint: Vec<PaintNode>,
}

pub enum GeometryId {
    Component(ComponentId),
    Content(ComponentId),
    PetPanel,
    PetArt,
    PetSpeech,
    Habitat,
}
```

This replaces the old idea of `layers: BTreeMap<LayerId, Rect>`. A single
`LayerId::PanelContent` was under-modeled. Real rendering needs ordered paint
nodes, clipping, exclusions, and effect targets.

### Paint Plan

```rust
pub struct PaintNode {
    pub phase: PaintPhase,
    pub target: GeometryId,
    pub rect: Rect,
    pub clip: Rect,
    pub z: i16,
    pub exclusions: Vec<GeometryId>,
}

pub enum PaintPhase {
    Background,
    Content,
    Overlay,
    Effect,
}
```

Most panels produce one content paint node. `PetScene` produces habitat
background, speech, pet art, and effect target nodes. The render pass can remain
simple at first, but the model leaves room for dynamic backgrounds without
reintroducing hidden geometry paths.

## Component Set

The first component set is intentionally boring:

- `WatchScreen` - responsive root component.
- `Grid` - wide-mode structural container.
- `Stack` - compact-mode vertical container.
- `Panel` - title/chrome/content wrapper.
- `TextRow` - label/value row.
- `StatRow` - label/bar/value row.
- `ProgressBar` - wrapper over Ratatui `Gauge` or a local bar if the built-in
  widget cannot match the existing look.
- `Sparkline` - wrapper over Ratatui `Sparkline` or existing local spark helper.
- `FeedList` - bounded list of events.
- `PetScene` - custom pet renderer, habitat, speech, and effect targets.

The point is not to invent a framework for its own sake. The point is to make
ordinary panels ordinary.

## Special Case: PetScene

`PetScene` is the only special watch component.

It owns:

- habitat background placement,
- speech bubble placement,
- pet art placement,
- pet-art hit target,
- effect scopes for pet art and pet panel,
- exclusions so habitat avoids the pet art.

It must expose one shared geometry result:

```rust
pub struct PetGeometry {
    pub panel: Rect,
    pub habitat: Rect,
    pub speech: Option<Rect>,
    pub pet_art: Option<Rect>,
}

impl PetScene {
    pub fn compute_geometry(area: Rect, vm: &WatchViewModel) -> PetGeometry;
}
```

`PetScene::render`, `tachyonfx`, mouse hit testing, and Preview Lab all consume
this same geometry. No second implementation of speech rows, breathing, wander,
or pet-art centering is allowed.

## Layout Backend

The component system should support Taffy as the layout backend, but the first
implementation should not make Taffy the API.

When Taffy is introduced, Glorp owns the terminal-cell conversion contract:

- accumulate parent-relative offsets into absolute terminal coordinates,
- convert float boxes to integer `Rect`s with deterministic rounding,
- conserve sibling space so declared gutters do not create accidental holes,
- clamp every child to its parent,
- treat Taffy structural errors as programmer bugs with a local fallback,
- test widths/heights around thresholds and odd remainders.

Ordinary resize behavior is not a `Result`. It is a component policy: hide or
shrink lower-priority components according to `ComponentSizing`.

## Wide Layout

Wide mode should preserve the approved product hierarchy:

```text
frame
  content grid
    row 1
      pet scene              today/progress stack
    gutter row
    row 2
      vitals/bio stack       feed
```

This is intentionally closer to a grid than two independent columns. The current
watch uses cross-column alignment, and the component system should preserve that
unless a later product decision deliberately changes it.

Policy:

- The pet scene is the primary flexible region.
- The habitat fills the pet scene and excludes `PetArt`.
- Vitals and bio stay grouped below the pet.
- Today, progress, and feed stay readable for focused glances.
- Feed remains bounded; it is a recency view, not a scrollback.
- Outer frame caps remain Glorp product policy, not layout-backend policy.

## Compact Layout

Compact mode is a stack:

```text
frame
  pet scene
  vitals
  today
  progress
  feed
```

`Bio` remains hidden in compact mode for now. That is a component visibility
rule, not special branching in the renderer.

When vertical space is too small, hide/shrink order is:

1. `Bio`
2. feed rows above the minimum visible count
3. progress rate details, keeping the stage label/bar if possible
4. sparkline/footer details
5. pet speech
6. pet art only when it cannot fit

The pet art should remain visible at the normal compact preview size `72x24`.

## Preview Lab

Preview Lab should visualize component bounds, not only raw cells.

Each watch `PreviewFrame` gains a preview-only layout model with primitive
rects:

```rust
pub struct PreviewLayout {
    pub components: BTreeMap<String, PreviewRect>,
    pub targets: BTreeMap<String, PreviewRect>,
    pub paint: Vec<PreviewPaintNode>,
}
```

Do not serialize Ratatui `Rect` directly. Use preview structs with primitive
`x`, `y`, `width`, and `height` fields.

The HTML preview should include a toggleable overlay for component bounds and
effect targets. The manifest should include enough information for reviewers to
distinguish allocation bugs from rendering bugs.

Add a tall-wide scenario such as `180x50` so the pet-as-hero and habitat-fill
behavior is visible in review artifacts.

## Hit Testing And Effects

Add an explicit hit-test API once geometry is centralized:

```rust
pub enum HitTarget {
    Component(ComponentId),
    Geometry(GeometryId),
}

pub fn hit_test(layout: &ComponentLayout, point: Position) -> Option<HitTarget>;
```

`tachyonfx` should target geometry IDs, not recompute layout. Mouse tracking can
still store raw terminal coordinates, but hit testing must happen through the
latest component layout.

## Migration Plan

Do not implement this as one big Taffy swap. Implement the component system in
small, reviewable slices.

1. **Component geometry contract.**
   Add component IDs, `ComponentSizing`, `ComponentGeometry`, paint nodes, and
   `PetGeometry`. Back it with the existing layout math first.

2. **PetScene extraction.**
   Move pet speech/art geometry into one shared `PetScene::compute_geometry`.
   Update render, `pet_panel_rect()`, and tests to consume it.

3. **Boring component wrappers.**
   Introduce `Panel`, `TextRow`, `StatRow`, `ProgressBar`, `Sparkline`, and
   `FeedList` wrappers around Ratatui or existing local helpers. Migrate one
   panel at a time.

4. **WatchScreen composition.**
   Express current wide and compact watch structures as `WatchScreen` component
   trees. Keep output visually equivalent.

5. **Preview Lab overlays.**
   Export preview layout metadata and add the HTML overlay toggle.

6. **Taffy backend.**
   Add Taffy behind container components only after the component geometry and
   preview contracts are proven. Specify dependency features and validate with
   `cargo tree -e features`.

7. **Delete old slat math.**
   Remove obsolete top-level split duplication only after render, effects, hit
   testing, and Preview Lab consume the component layout.

This is not a throwaway spike. It is a staged migration to a better authoring
model.

## Testing

Add tests at four levels:

- `PetGeometry` tests for speech on/off, breathing, wander, tiny panel, and
  normal compact/wide panels.
- Pure component-layout tests for wide, compact, tall-wide, threshold widths,
  odd widths, and very small terminals.
- Render tests confirming component bounds match visible section titles and
  content does not bleed into neighboring components.
- Preview tests confirming layout metadata, effect targets, and overlay assets
  exist for watch scenarios.

When Taffy is introduced, add backend-specific tests for:

- no overlapping required components,
- no accidental gaps except declared gutters,
- sibling integer conservation,
- parent clipping,
- stable output for odd terminal dimensions.

## Release And Dependency Checks

If Taffy ships as a production dependency, the implementation must include:

- exact dependency stanza and feature rationale,
- `cargo tree -e features` review,
- `cargo test --locked`,
- `cargo build --release --locked --no-default-features`,
- existing npm release assertion checks if version surfaces change.

## Acceptance Criteria

- Future agents add or rearrange watch panels by composing components, not by
  editing top-level cell math.
- Ordinary panels use shared text/list/progress/sparkline components or imported
  Ratatui widgets.
- `PetScene` is the only component with bespoke geometry and custom rendering.
- Rendering, effects, hit testing, and Preview Lab consume the same component
  geometry.
- The approved watch hierarchy and compact/wide behavior are preserved.
- Preview Lab can show component/effect overlays for layout review.
- Taffy, if used, is hidden behind the component layout backend and never leaks
  into individual panel renderers.
