# Glorp presentation architecture - design

- Date: 2026-06-15
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-05-12-glorp-preview-lab-design.md`
  - `docs/superpowers/specs/2026-05-13-watch-component-system-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-alive-room-design.md`
  - `docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md`
  - `docs/superpowers/specs/2026-06-13-glorp-species-room-dialects-design.md`
  - `docs/superpowers/specs/2026-06-13-glorp-style-pass-design.md`

## Problem

Glorp's presentation code has grown around several working surfaces:

- the watch TUI, rendered through ratatui cells
- the pet art renderer, returning text plus role spans
- Preview Lab, capturing deterministic text/cell/layout artifacts
- round preview, rendering a circular companion surface into Preview Lab cells
- native macOS companion, rendering round draw commands into AppKit
- macOS menubar/popover rendering, adapting pet art and stats into attributed strings

Each surface works, but the shared visual ideas are not shared enough. Pet role
spans, room glyph vocabulary, color adaptation, animation timing, habitat prop
placement, and review artifacts are interpreted in multiple places. This makes
small visual changes feel risky and makes large visual improvements expensive.

The clearest hotspot is `src/tui/panels/pet.rs`: it owns pet-panel composition,
room wash, ambient glyphs, motes, activity glyphs, prop layering, cursor eyes,
mirroring/span styling, palette math, performance cues, speech, and direct
buffer painting. Other large files (`src/dev_preview/watch.rs`,
`src/tui/component/habitat_props.rs`, `src/dev_preview/scenarios.rs`,
`src/tui/room.rs`, and `src/commands/watch.rs`) have similar accumulated
responsibilities.

The tempting response is a renderer rewrite. That is the wrong first move.
There are too many useful fixtures and too much subtle behavior to preserve.
This design defines a consolidation path: freeze review contracts, mechanically
untangle the worst files, introduce a shared presentation domain, then make each
surface a thinner adapter.

## Goals

- Make Glorp's visual presentation easier to modify without breaking another
  surface.
- Preserve current output until a plan explicitly calls for an intentional
  visual change.
- Keep Preview Lab as the deterministic review and regression surface.
- Introduce a shared presentation vocabulary that can serve watch, round,
  companion, menubar, and Preview Lab without forcing them through one renderer.
- Reduce the size and responsibility of the largest presentation files.
- Support overnight agent work through multiple bounded implementation plans
  with clear write sets, verification commands, and stop conditions.

## Non-goals

- No big-bang renderer rewrite.
- No immediate replacement of `WatchViewModel`.
- No new generic renderer trait as the first abstraction.
- No Preview Lab artifact path/schema churn in the first tranche, except small
  additive contract artifacts explicitly listed below.
- No intentional visual redesign in the mechanical extraction tranches.
- No native macOS companion lifecycle rewrite.
- No changes to usage, XP, mood, storage, or provider logic.

## Current architecture

### Watch TUI

The watch path starts in `src/commands/watch.rs`, builds a `WatchViewModel`, then
uses `src/tui/component/watch_screen.rs` and `src/tui/layout.rs` to compute and
render a component layout. The pet scene itself is mostly painted by
`src/tui/panels/pet.rs`.

Useful existing seams:

- `WatchViewModel` is the compatibility anchor.
- `ComponentLayout` and `PreviewLayout` already expose stable target paths such
  as `watch.pet.art`, `watch.room.effect`, and prop effect targets.
- `src/tui/room.rs` already has a mostly pure `RoomLifeProfile` and room glyph
  generation.

Pain points:

- `PetPanel::render` derives scene state and paints layers in one place.
- Room, ambient, motes, props, pet art, speech, and performance cues are mixed.
- Many helper functions are pure enough to move, but are trapped in a huge file.

### Pet art

`src/pet/render.rs` is a good primitive boundary. It returns `RenderedPet {
lines, spans }`, where spans identify semantic pet roles. The problem is not
this renderer itself. The problem is that consumers repeatedly reinterpret the
same spans, color roles, mirroring, and display width concerns.

### Room and habitat props

`src/tui/room.rs` derives room identity from real state:

- biome
- species dialect
- weather layer
- resonant emitter
- pet performance
- scene moments
- identity prop ids

`src/tui/component/habitat_props.rs` owns prop selection, placement, sprites,
styles, animation offsets, and effect target ids. That is presentation-domain
logic, even though it currently lives under `tui/component`.

The enduring product rule stays:

```text
Props define what the room has earned.
Species defines the room's visual dialect.
```

### Round and companion

Round is healthier than watch. It already has:

- `RoundSceneModel` in `src/round/model.rs`
- `layout_round_scene` in `src/round/layout.rs`
- round Preview Lab cells in `src/round/preview.rs`
- native draw commands in `src/companion/render.rs`

The gap is that round preview and companion do not fully share the same painter
contract. Round preview paints cells directly, while native companion builds draw
commands. Both should be downstream of the same scene/layout vocabulary.

### Preview Lab

Preview Lab is the regression harness, not the shared scene model.

`PreviewFrame` is a capture format: terminal-sized cells, optional layout, and
manifest inputs. `PreviewManifest` is a review contract. These should stay at
the edge.

The highest-leverage Preview Lab target is the semantic target map. Watch
already exports layout targets; round should export comparable semantic targets
or command/layout artifacts so the same scene can be validated across surfaces.

## Direction

Glorp should move toward this layered model:

```text
WatchViewModel + now
  -> PresentationScene
  -> SurfaceSpec + capabilities
  -> surface capture or draw commands
  -> TUI / Preview Lab / AppKit / menubar adapters
```

This does not mean one renderer. It means one shared scene vocabulary, one set
of visual primitives, and several small adapters.

### PresentationScene

Introduce a backend-neutral scene model after the safety net and mechanical
extraction work.

The exact Rust names can change, but the model should contain:

- pet snapshot: seed, species, stage, mood, rendered art lines, role spans,
  palette, facing, breath/posture, asleep state
- room snapshot: `RoomLifeProfile`, biome, species dialect, weather, scene
  moments, prop identity
- habitat prop snapshot: visible/earned prop ids, selected placements, layers,
  effect targets
- activity/life/day snapshot: day phase, calm mode, source diversity, helper
  health, recent activity pulse, vitals buckets
- privacy classification: what may be shown on glanceable/native surfaces
- target map: stable semantic anchors for pet, room, speech, prop effects, halo,
  and dashboard panels where relevant

`PresentationScene` should initially be derived from `WatchViewModel + now`.
That keeps the current VM as the compatibility anchor and avoids changing the
runtime data flow while presentation files are being untangled.

### Visual primitives

Add small backend-neutral primitives only when a migrated surface needs them.
Expected primitives:

- `GlyphCell` or equivalent: symbol, role/style/color, layer, target id
- `TextBlock`: lines plus role spans
- `VisualColor`: enough information to map to ratatui, Preview Lab hex/named
  colors, or AppKit colors
- `Layer`: background, room, ambient, prop behind, pet, prop foreground, halo,
  overlay
- `EffectTarget`: stable id plus bounds/anchor metadata
- `SurfaceSpec`: watch rectangle, round aperture, menubar/popover constraints

Do not design these as a universal renderer framework. Keep them as plain data
structures with narrow producers and consumers.

### Surface adapters

Each surface remains free to choose its output format:

- watch TUI maps scene primitives to a ratatui `Buffer`
- Preview Lab captures cells, text, layouts, scene artifacts, and manifest data
- round preview maps scene/layout to preview cells
- native companion maps scene/layout to `RoundDrawCommand`s and AppKit drawing
- menubar maps pet/stat primitives to attributed strings

The adapters should not independently decide pet role colors, room vocabulary,
or sanitized activity meaning.

## Architecture options considered

### Option A: Big-bang renderer rewrite

Replace the current surfaces with one new renderer architecture.

Pros:

- conceptually clean
- could remove duplication quickly if it worked

Cons:

- high regression risk
- too much subtle existing behavior to preserve at once
- Preview Lab would be forced to validate a moving target
- native companion and watch have different output needs

Decision: reject.

### Option B: Mechanical extraction only

Split the largest files into smaller modules but avoid any shared scene model.

Pros:

- lowest short-term risk
- immediately improves file size and navigation
- can be verified with existing tests

Cons:

- does not fully solve duplicated renderer concepts
- future visual work can still drift across surfaces

Decision: use as the first code movement, but not the endpoint.

### Option C: Contract-first consolidation

First strengthen Preview Lab contracts, then mechanically extract hotspot files,
then introduce `PresentationScene` and migrate adapters one by one.

Pros:

- keeps current behavior protected
- creates better tripwires before risky movement
- lets each plan be bounded and independently reviewable
- converges on shared semantics without forcing one renderer

Cons:

- slower than a rewrite
- leaves some duplication in place until later tranches

Decision: choose this path.

## Implementation plan tracks

The spec should be implemented through four separate plan documents. Each plan
must include allowed write sets, forbidden changes, red-green checks,
verification commands, and stop conditions for overnight work.

### Plan 1: Contract Freeze

Purpose: make Preview Lab strong enough to protect the refactor.

Scope:

- add additive scene artifacts, likely `frames/<id>.scene.json` where useful
- add round layout/draw-command artifacts so round preview and native companion
  can be compared through the same semantic source
- make animation strips use the deterministic preview clock
- add cross-renderer fixture checks for normal, active pulse, asleep/night,
  helper trouble, full props, Glitch vs Crystal dialect, and flat color where
  practical
- correct stale docs that describe Preview Lab manifest schema `2` when the code
  now uses schema `3`

Non-goals:

- no output redesign
- no renderer migration
- no schema/path removal
- no native lifecycle work

Expected verification:

```bash
cargo test --test dev_preview
cargo test round_scene
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Stop conditions:

- any existing preview artifact path must be renamed or removed
- a visual diff appears that is not caused by a newly-added artifact
- deterministic preview output depends on wall-clock time

### Plan 2: PetPanel Mechanical Split

Purpose: reduce the largest hotspot without changing behavior.

Scope:

- split `src/tui/panels/pet.rs` into smaller modules
- likely modules:
  - `src/tui/panels/pet/ambient.rs`
  - `src/tui/panels/pet/colors.rs`
  - `src/tui/panels/pet/art_lines.rs`
  - `src/tui/panels/pet/composition.rs` if needed
- keep public behavior and existing call sites stable
- preserve current visual output except for snapshots intentionally updated only
  if formatting/module movement changes nothing semantically but snapshot tooling
  requires refresh

Non-goals:

- no new scene model yet
- no visual tuning
- no moving `WatchViewModel`
- no habitat prop redesign

Expected verification:

```bash
cargo test tui::panels::pet
cargo test --test tui_render
cargo test --test dev_preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Stop conditions:

- any intentional visual change is needed to make tests pass
- module extraction requires broad public API changes outside the pet panel
- the split makes circular dependencies that suggest the boundary is wrong

### Plan 3: Presentation Domain Extraction

Purpose: introduce the shared backend-neutral presentation layer.

Scope:

- add `src/presentation/`
- move or wrap pure visual-domain concepts behind it:
  - scene derivation from `WatchViewModel + now`
  - pet role/span helpers
  - room profile/glyph vocabulary
  - prop presentation identities/placements where safe
  - visual primitives shared by surfaces
- keep old module paths as thin wrappers where that reduces churn
- derive `RoundSceneModel` from the shared scene, or make it a surface-specific
  projection of the shared scene

Non-goals:

- no universal renderer trait unless a migrated adapter proves it is necessary
- no immediate deletion of `RoundSceneModel`
- no replacement of `WatchViewModel`
- no native lifecycle changes

Expected verification:

```bash
cargo test tui::room
cargo test tui::component::habitat_props
cargo test round_scene
cargo test --test watch_integration
cargo test --test dev_preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Stop conditions:

- the shared scene starts carrying private/source-detail data that should not
  reach glanceable surfaces
- the new module duplicates `RoomLifeProfile` or `RoundSceneModel` instead of
  wrapping/projecting existing concepts
- adapters begin depending on each other's concrete output formats

### Plan 4: Renderer Adapter Consolidation

Purpose: retire duplicated surface-specific interpretations.

Scope:

- make watch, round preview, native companion, menubar, and Preview Lab consume
  shared scene/primitives where useful
- unify pet role color and span interpretation
- make round preview consume the same command/layout vocabulary as native
  companion where possible
- keep Preview Lab fixtures near the review harness, but make fixture builders
  return frame plus scenario contract together instead of relying on frame-id
  pattern matching

Non-goals:

- no single renderer required
- no loss of surface-specific layout logic
- no shrunken-dashboard round view
- no privacy regression in companion or menubar

Expected verification:

```bash
cargo test --features dev-preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Stop conditions:

- a surface loses a capability because another surface does not need it
- renderer consolidation makes Preview Lab less explicit about artifacts
- native companion gains dashboard/source/accounting details not allowed by the
  round companion design

## Testing and review strategy

The refactor should be evidence-first:

- use Preview Lab as the visual contract for every tranche
- add contract artifacts before moving code when a surface lacks coverage
- keep existing snapshots stable unless an implementation plan explicitly
  authorizes visual change
- prefer semantic artifact assertions for architecture movement and visual
  snapshots for final presentation tuning
- run targeted tests after each tranche and the full `cargo test --features
  dev-preview` before merge

Useful recurring checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features dev-preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

## Migration rules

- Move pure functions before changing behavior.
- Preserve old call sites with thin wrappers when that keeps a tranche small.
- Prefer additive Preview Lab artifacts over changing existing artifact paths.
- Keep `WatchViewModel` as the source of truth until a later spec explicitly
  replaces it.
- Keep privacy decisions in the scene/domain layer, not hidden in surface
  painters.
- Keep props and species as separate axes: earned history versus visual dialect.
- Treat a newly-large module as a design smell; do not replace one giant file
  with another.

## Success criteria

- `src/tui/panels/pet.rs` no longer contains unrelated ambient, color, art-line,
  composition, and buffer-painting responsibilities in one file.
- Preview Lab exports enough semantic artifacts to compare watch, round preview,
  and native companion scene intent.
- Pet role colors, span interpretation, and room vocabulary are decided in one
  shared place and adapted per surface.
- Round preview and native companion share scene/layout/command semantics rather
  than parallel room/pet interpretations.
- New visual work can target a small presentation module and know which Preview
  Lab fixture verifies it.
- The architecture supports overnight implementation plans without requiring one
  agent to hold the whole renderer stack in context.
