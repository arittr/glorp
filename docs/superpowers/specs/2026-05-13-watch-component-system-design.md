# Watch Component System - Design

**Status**: Revised design after two staff-review passes and Drew direction.
**Source**: Layout architecture brainstorm on 2026-05-13.
**Supersedes**: `2026-05-13-taffy-watch-layout-engine-design.md`.

## Problem

The watch code makes ordinary terminal UI work feel like custom graphics
programming. Most watch panels are text, rows, progress bars, lists, or
sparklines, yet changing them requires hand-adjusting panes and row math in the
top-level layout.

That is the wrong authoring model for Glorp. Future agents should spend their
attention on product behavior and the one genuinely special surface: the pet
scene. They should not spend the bulk of a change aligning slats.

The first Taffy design attacked the symptom one layer too low. Taffy can be a
layout backend, but the missing abstraction is a Glorp component system:
ordinary widgets, panel composition, layout intent, and one shared geometry
artifact.

## Goal

Create a Rust-native component system for the watch UI. The component system is
the public authoring surface. Ratatui remains the rendering backend. Taffy, if
used, is an internal allocator for container components.

The hard architectural rule is:

```text
layout_watch(area, vm, ctx) -> ComponentLayout
render_watch(layout, buf, vm, ctx)
```

`ComponentLayout` is computed before rendering. Rendering, `tachyonfx`, mouse
hit testing, and Preview Lab all consume that same artifact. No caller gets a
separate geometry path.

The authoring experience should make ordinary panel work feel like composition:

```rust
WatchScreen::new()
    .wide(
        WatchColumns::new()
            .left(Stack::new()
                .child(WatchComponentId::Pet, PetScene)
                .child(WatchComponentId::Vitals, VitalsPanel)
                .child(WatchComponentId::Bio, BioPanel))
            .right(Stack::new()
                .child(WatchComponentId::Today, TodayPanel)
                .child(WatchComponentId::Progress, ProgressPanel)
                .child(WatchComponentId::Feed, FeedPanel)),
    )
    .compact(
        Stack::new()
            .child(WatchComponentId::Pet, PetScene)
            .child(WatchComponentId::Vitals, VitalsPanel)
            .child(WatchComponentId::Today, TodayPanel)
            .child(WatchComponentId::Progress, ProgressPanel)
            .child(WatchComponentId::Feed, FeedPanel),
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
widgets where they fit, but bless local Glorp widgets when the existing visual
language needs them. In particular, `StatRow`, `ProgressBar`, and
`InlineSparkline` are first-class local components because the current watch
uses per-cell gradients, custom empty-cell styling, and compact inline spark
glyphs.

Treat `PetScene` as the only special watch component. It owns the bespoke pet
renderer, habitat, speech, and effect targets.

Taffy remains likely, but it is not the API. The first migration proves the
backend-independent `ComponentLayout` contract before adding Taffy.

## Styling Ergonomics

The component system should provide Lip Gloss-class authoring ergonomics while
remaining Ratatui-buffer-native underneath.

The goal is declarative, composable styling for ordinary components: semantic
surfaces, text roles, borders, padding, alignment, adaptive colors, gradients,
and background policies. Future panel code should describe visual intent,
instead of manually stitching together Ratatui `Style`, `Block`, `Paragraph`,
and per-cell fill logic at every callsite.

The style layer is a Glorp facade, not a second renderer. It may borrow ideas
or helpers from `lipgloss-rs`, but ordinary watch rendering must still write to
Ratatui buffers through `ComponentNodeLayout` and `GeometryTarget`s. That keeps
Preview Lab, `tachyonfx`, mouse hit testing, and layout JSON grounded in the
same geometry artifact.

Expected authoring shape:

```rust
Panel::new("today")
    .surface(Surface::Elevated)
    .border(BorderTone::Accent)
    .padding(Insets::horizontal(2))
    .child(TextRow::new("tokens", vm.today.tokens).tone(TextTone::Primary));

ProgressBar::new(vm.progress.stage_fraction)
    .gradient(GradientToken::Xp)
    .empty_tone(TextTone::Subtle);
```

The exact API can change, but the implementation plan should include a proof
task: migrate one ordinary panel so borders, padding, background, text tone, and
gradient intent are expressed through the Glorp style facade rather than
callsite-level Ratatui plumbing.

## Architecture

The watch pipeline becomes:

```text
WatchViewModel + Clock/animation context
  -> stateless WatchScreen component tree
  -> layout_watch(frame_area, vm, ctx)
  -> ComponentLayout
  -> render_watch(layout, buffer, vm, ctx)
  -> tachyonfx / hit_test / Preview Lab read ComponentLayout
```

Components are mostly stateless Rust values. They do not own or borrow the
`WatchViewModel` long term. The current VM is passed through short-lived
contexts during measurement, layout, geometry, and rendering.

### Contexts

```rust
pub struct MeasureContext<'a> {
    pub vm: &'a WatchViewModel,
    pub clock: &'a WatchClock,
}

pub struct LayoutContext<'a> {
    pub vm: &'a WatchViewModel,
    pub clock: &'a WatchClock,
    pub mode: LayoutMode,
}

pub struct RenderContext<'a> {
    pub vm: &'a WatchViewModel,
    pub clock: &'a WatchClock,
    pub styles: &'a SemanticStyles,
    pub color_capability: ColorCapability,
}
```

`WatchClock` carries deterministic time and animation phase. Rendering paths
must not call `OffsetDateTime::now_utc()` directly. Preview Lab supplies a fixed
clock; live watch supplies the live clock.

### Component Contract

```rust
pub trait Component {
    fn sizing(&self, ctx: &MeasureContext<'_>) -> ComponentSizing;

    fn layout(
        &self,
        id: ComponentPath,
        allocated: Rect,
        ctx: &LayoutContext<'_>,
    ) -> ComponentNodeLayout {
        ComponentNodeLayout::default_leaf(id, allocated)
    }

    fn render(
        &self,
        node: &ComponentNodeLayout,
        buf: &mut Buffer,
        ctx: &RenderContext<'_>,
    );
}
```

The render contract takes the computed node layout, not just a raw `Rect`.
Ordinary components use the default leaf layout. `PetScene` overrides `layout`
to expose pet-specific targets.

### Stable IDs

Component and target IDs are stable semantic paths, not ad hoc strings.

```rust
pub enum WatchComponentId {
    Root,
    Pet,
    Today,
    Progress,
    Feed,
    Vitals,
    Bio,
}

pub struct ComponentPath(&'static str); // e.g. "watch.pet"
pub struct TargetPath(&'static str);    // e.g. "watch.pet.art"
```

The layout builder validates ID uniqueness. Preview Lab, hit testing, effects,
and tests all serialize/use the same paths.

## ComponentLayout

`ComponentLayout` is the shared artifact for the frame.

```rust
pub struct ComponentLayout {
    pub frame: Rect,
    pub content: Rect,
    pub mode: LayoutMode,
    pub nodes: BTreeMap<ComponentPath, ComponentNodeLayout>,
    pub targets: BTreeMap<TargetPath, GeometryTarget>,
    pub decisions: Vec<LayoutDecision>,
}

pub struct ComponentNodeLayout {
    pub id: ComponentPath,
    pub bounds: Rect,
    pub content: Rect,
    pub visibility: VisibilityState,
    pub children: Vec<ComponentPath>,
    pub targets: BTreeMap<TargetPath, GeometryTarget>,
}

pub struct GeometryTarget {
    pub owner: ComponentPath,
    pub rect: Rect,
    pub z: i16,
    pub clip: Rect,
    pub role: TargetRole,
}

pub enum TargetRole {
    Content,
    PetPanel,
    PetArt,
    PetSpeech,
    Habitat,
    Effect,
}

pub enum VisibilityState {
    Visible,
    Hidden { reason: LayoutDecisionReason },
    Degraded { reason: LayoutDecisionReason },
}
```

The top-level `targets` map is a flattened index for effects, hit testing, and
Preview Lab. Node-local targets are kept for component-owned rendering.

## Sizing And Degradation

Sizing is Glorp-native and independent of Taffy. It should express product
policy instead of leaking backend concepts like raw `flex_grow` everywhere.

```rust
pub struct ComponentSizing {
    pub width: AxisSize,
    pub height: AxisSize,
    pub degrade: Vec<DegradeRule>,
}

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

pub enum DegradeRule {
    LimitRows {
        target: TargetPath,
        min: u16,
        max: u16,
    },
    OmitDetail {
        target: TargetPath,
    },
    HideTarget {
        target: TargetPath,
    },
    HideComponent,
}
```

The pressure resolver applies degradation deterministically and records every
decision in `ComponentLayout.decisions`. Preview Lab exports those decisions so
reviewers can see why a region changed.

Compact-mode degradation order:

1. `watch.bio` hides.
2. `watch.feed.rows` limits rows above the minimum visible count.
3. `watch.progress.rate` omits rate details, keeping stage label/bar if
   possible.
4. `watch.today.sparkline` omits sparkline/footer detail.
5. `watch.pet.speech` hides.
6. `watch.pet.art` hides only when the art cannot fit.

The pet art must remain visible at the normal compact preview size `72x24`.

## Component Set

The first component set is intentionally boring:

- `WatchScreen` - responsive root composition.
- `WatchColumns` - wide-mode asymmetric column container.
- `Stack` - vertical container.
- `Panel` - title/chrome/content wrapper.
- `TextRow` - label/value row.
- `StatRow` - label/bar/value row.
- `ProgressBar` - local Glorp bar component matching current gradients and
  empty-cell styling.
- `InlineSparkline` - local Glorp spark component matching current inline style.
- `FeedList` - bounded list of events with row-limit degradation.
- `PetScene` - custom pet renderer, habitat, speech, and effect targets.

Ordinary panels should not implement custom geometry or paint. They compose
these components and render into their `ComponentNodeLayout.content` rect.

## Authoring Contract

The component system is successful only if future agent work gets simpler.

Acceptance proof: adding or moving an ordinary watch panel must touch only:

- the `WatchScreen` composition file, and
- the new/changed panel component file.

It must not require editing:

- top-level coordinate math,
- `tachyonfx` scoping,
- Preview Lab export code,
- mouse hit-test logic,
- unrelated panel renderers.

Ordinary panels must not implement custom geometry. If a panel needs bespoke
geometry, it must be explicitly classified as a scene-like component, with the
same scrutiny as `PetScene`.

## Backgrounds And Effects

Dynamic backgrounds are part of the component model, but they should not force
ordinary panels into custom paint graphs.

For ordinary panels, background behavior is a panel policy: static fill,
semantic tint, progress-linked tint, or empty. The `Panel` component owns that
rendering before it asks children to render into `content`.

For scene-like components, backgrounds can be custom render targets. `PetScene`
uses this for habitat rendering and animated effect scopes. Those targets still
flow through `ComponentLayout`, so effects, hit testing, and Preview Lab can
inspect them without knowing pet-specific geometry.

## Special Case: PetScene

`PetScene` is the only special watch component.

It owns:

- habitat background placement,
- speech bubble placement,
- pet art placement,
- pet-art hit target and local coordinates,
- effect scopes for pet art and pet panel,
- exclusions so habitat avoids the pet art.

It exposes one shared layout result as part of `ComponentLayout`:

```rust
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

impl PetScene {
    pub fn compute_layout(
        id: ComponentPath,
        area: Rect,
        ctx: &LayoutContext<'_>,
    ) -> PetSceneLayout;
}
```

`PetScene::render`, `tachyonfx`, mouse hit testing, and Preview Lab consume this
same layout. No second implementation of speech rows, breathing, wander, time,
or pet-art centering is allowed.

## Layout Backend

The component system should support Taffy as a backend, but the first
implementation should not make Taffy the API.

When Taffy is introduced, Glorp owns the terminal-cell conversion contract:

- accumulate parent-relative offsets into absolute terminal coordinates,
- convert float boxes to integer `Rect`s with deterministic rounding,
- conserve sibling space so declared gutters do not create accidental holes,
- clamp every child to its parent,
- treat Taffy structural errors as programmer bugs with a local fallback,
- test widths/heights around thresholds and odd remainders.

If Taffy is added, use an exact dependency stanza and minimal features first:

```toml
taffy = { version = "0.10.1", default-features = false, features = ["std", "taffy_tree", "flexbox"] }
```

Add `grid` only if the implementation actually needs grid semantics.

## Wide Layout

Wide mode preserves the approved asymmetric product hierarchy:

```text
frame
  content
    left column
      pet scene     Fill: primary flexible canvas
      vitals        Intrinsic
      bio           Intrinsic
    gutter
    right column
      today         Intrinsic
      progress      Intrinsic
      feed          Bounded intrinsic list
      trailing space accepted
```

The left column is the pet canvas plus pet-adjacent status. The right column is
packed from the top for focused glances. Feed remains bounded; it is a recency
view, not a scrollback. Cross-column row alignment is not a product invariant
unless Drew explicitly re-approves it later.

Current tests that encode cross-column slat alignment should be replaced with
product invariants:

- pet scene is the primary flexible region,
- habitat fills the pet scene and excludes `watch.pet.art`,
- today/progress/feed stay top-packed,
- feed row count stays bounded,
- compact mode hides only approved low-priority content.

### Frame Cap Policy

The current `MAX_FRAME_HEIGHT = 23` cap prevents tall-wide review from
exercising the intended pet canvas. The component-system migration should remove
the hard wide-height cap or replace it with a policy that still lets the pet
scene absorb vertical slack.

Wide terminals may still cap frame width for text readability, but wide frame
height must grow with available terminal height. The `180x50` Preview Lab
scenario is a real product checkpoint, not a padded `110x23` frame centered
inside a large terminal.

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
decision recorded in `ComponentLayout.decisions`, not special branching in a
renderer.

## Preview Lab

Preview Lab overlays must arrive in the first migration slice. They are not a
late polish item; they are the inspection tool that keeps geometry work honest.

Each watch preview frame gains a layout artifact:

```text
frames/watch-wide-normal.layout.json
frames/watch-compact-normal.layout.json
frames/watch-tall-wide.layout.json
```

Manifest schema bumps to version 2 and each watch scenario includes:

```json
{
  "files": {
    "text": "frames/watch-wide-normal.txt",
    "cells": "frames/watch-wide-normal.cells.json",
    "layout": "frames/watch-wide-normal.layout.json"
  }
}
```

Layout artifact shape:

```json
{
  "schema_version": 1,
  "frame_id": "watch-wide-normal",
  "mode": "wide",
  "components": {
    "watch.pet": { "x": 2, "y": 2, "width": 40, "height": 18 }
  },
  "targets": {
    "watch.pet.art": { "x": 15, "y": 6, "width": 13, "height": 10 }
  },
  "decisions": []
}
```

Use preview-only structs with primitive `x`, `y`, `width`, and `height` fields.
Do not serialize Ratatui `Rect` directly.

The HTML preview gets a toggleable component/effect overlay. Tests must assert:

- layout artifact IDs use stable `-layout` suffixes,
- exact component and target IDs exist for watch scenarios,
- overlay controls are present,
- `watch-tall-wide` renders an actually tall frame.

## Hit Testing And Effects

Hit testing reads `ComponentLayout`.

```rust
pub struct HitResult {
    pub target: TargetPath,
    pub rect: Rect,
    pub local_position: Position,
    pub z: i16,
}

pub fn hit_test(layout: &ComponentLayout, point: Position) -> Option<HitResult>;
```

`tachyonfx` targets `TargetPath`s such as `watch.pet.art` or
`watch.pet.panel`. It never recomputes layout. Mouse tracking can still store
raw terminal coordinates, but interpretation happens through the latest
`ComponentLayout`.

## Migration Plan

Do not implement this as one big Taffy swap. Implement the component system in
small, reviewable slices. Slice 1 must eliminate the separate geometry path.

1. **ComponentLayout adapter + Preview Lab overlay.**
   Add stable component/target IDs, `ComponentLayout`, `ComponentNodeLayout`,
   `GeometryTarget`, layout decisions, and preview layout export. Back the
   layout artifact with the existing allocation math, but make render,
   `tachyonfx`, hit testing, and Preview Lab consume the same artifact. No
   separate `pet_panel_rect()` geometry path survives this slice.

2. **PetScene extraction.**
   Move pet speech/art/habitat geometry into one shared
   `PetScene::compute_layout`. Remove direct wall-clock reads from pet render
   paths. Add deterministic `WatchClock`/animation context.

3. **Boring component wrappers.**
   Introduce `Panel`, `TextRow`, `StatRow`, `ProgressBar`, `InlineSparkline`,
   and `FeedList`. Migrate one ordinary panel at a time. Ordinary panels expose
   bounds/content only and do not produce custom geometry.

4. **WatchScreen composition.**
   Express current wide and compact watch structures as stateless component
   trees. Keep output visually equivalent except where tests were asserting old
   slat alignment instead of product invariants.

5. **Frame cap and tall-wide policy.**
   Update the wide-height cap policy so `watch-tall-wide` actually exercises a
   tall pet scene. Add tests for frame height, pet scene height, and habitat
   bounds.

6. **Taffy backend.**
   Add Taffy behind container components only after the component layout and
   preview contracts are proven. Specify dependency features and validate with
   `cargo tree -e features`.

7. **Delete old slat math.**
   Remove obsolete top-level split duplication only after render, effects, hit
   testing, and Preview Lab consume the component layout.

This is not a throwaway spike. It is a staged migration to a better authoring
model.

## Testing

Add tests at five levels:

- `ComponentLayout` tests for stable IDs, node bounds, target paths, layout
  decisions, and no duplicate IDs.
- `PetScene` tests for speech on/off, deterministic clock, breathing, wander,
  tiny panel, exclusions, and normal compact/wide panels.
- Pure component-layout tests for wide, compact, tall-wide, threshold widths,
  odd widths, and very small terminals.
- Render tests confirming component bounds match visible section titles and
  content does not bleed into neighboring components.
- Preview tests confirming layout artifacts, effect targets, overlay controls,
  and tall-wide real frame height.

When Taffy is introduced, add backend-specific tests for:

- no overlapping required components,
- no accidental gaps except declared gutters,
- sibling integer conservation,
- parent clipping,
- stable output for odd terminal dimensions.

Add an ergonomics proof test to the implementation plan: introduce a harmless
ordinary panel or move an existing ordinary panel using only `WatchScreen`
composition plus that panel file. If the proof requires layout math, the
component system failed its purpose.

## Release And Dependency Checks

If Taffy ships as a production dependency, the implementation must include:

- exact dependency stanza and feature rationale,
- `cargo tree -e features` review,
- `cargo fmt --check`,
- `cargo clippy --all-targets --all-features -- -D warnings`,
- `cargo test --locked`,
- `cargo test --locked --no-default-features --all-targets`,
- `cargo build --release --locked --no-default-features`,
- `npm test`,
- existing npm release assertion checks if version surfaces change.

## Acceptance Criteria

- `layout_watch(...) -> ComponentLayout` is the pre-render geometry source.
- `render_watch(...)`, `tachyonfx`, hit testing, and Preview Lab all consume the
  same `ComponentLayout`.
- No separate `pet_panel_rect()` geometry path remains after the first migration
  slice.
- Future agents add or rearrange ordinary watch panels by composing components,
  not by editing top-level cell math.
- Ordinary panels use shared text/list/progress/sparkline components and expose
  only bounds/content geometry.
- `PetScene` is the only component with bespoke geometry and custom rendering.
- The approved asymmetric watch hierarchy and compact/wide behavior are
  preserved.
- Preview Lab can show component/effect overlays for layout review, including a
  real tall-wide frame.
- Taffy, if used, is hidden behind the component layout backend and never leaks
  into individual panel renderers.
