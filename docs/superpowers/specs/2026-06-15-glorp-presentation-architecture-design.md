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
- privacy projection: the surface-specific sanitized view, not the raw runtime
  state
- target map: stable semantic anchors for pet, room, speech, prop effects, halo,
  and dashboard panels where relevant, using presentation-owned ids rather than
  watch `TargetPath`s

`PresentationScene` should initially be derived from `WatchViewModel + now`.
That keeps the current VM as the compatibility anchor and avoids changing the
runtime data flow while presentation files are being untangled.

Pose and timing ownership must be explicit before this model ships. Today the
watch renderer recomputes wander and facing inside `PetPanel::render` from the
render clock, while round/companion paths copy or mutate VM pose fields. The
shared scene derivation must use the same animation helpers as the current watch
render path. Fields such as `WatchViewModel.wander_offset_x`, `facing`, and
breath/posture values remain compatibility/cache fields until a later design
explicitly replaces that contract.

Privacy is also a projection, not one boolean. The scene may know enough to
derive every surface, but each surface receives only the allowed projection:

| Surface | Privacy projection |
| --- | --- |
| Watch TUI | Full interactive dashboard; may show source labels, exact counts, feed rows, and diagnostics already allowed by watch. |
| Native round companion | Glanceable sanitized view; no source names, exact counts, feed rows, file paths, project names, transcript-like text, or productivity-pressure labels. |
| Round Preview Lab | Same sanitized view as native round plus semantic artifacts needed to prove privacy. |
| Menubar popover | Interactive/privileged surface if it continues to show exact token/helper details; must be marked as such and not treated as a glanceable-safe projection. |
| Preview Lab artifacts | Sanitized by default unless a scenario explicitly documents a privileged review artifact. |

### Visual primitives

Add small backend-neutral primitives only when a migrated surface needs them.
Expected primitives:

- `GlyphCell` or equivalent: symbol, role/style/color, layer, presentation target
  id
- `TextBlock`: lines plus role spans; spans stay character-indexed while they
  wrap existing `StyledSegment` data, and any display-column spans must be named
  separately
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

Presentation-domain target ids must be owned, neutral ids such as
`EffectTargetId` or `SurfaceTargetId`. Watch adapters may map them to existing
`TargetPath` values like `watch.pet.art`, but `src/presentation` must not depend
on `tui::component::TargetPath` or on watch-specific `watch.*` ids.

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

The spec should be implemented through ordered plan tracks. Each plan must
include allowed write sets, forbidden changes, red-green checks, verification
commands, and stop conditions for overnight work.

The dependency order is strict:

1. Contract Freeze merges first.
2. PetPanel Mechanical Split merges second.
3. Presentation Domain Extraction is split into smaller subplans after the first
   two are green.
4. Renderer Adapter Consolidation is split by adapter after the domain seam
   exists.

Do not run these tracks in parallel against the same branch. They touch the same
Preview Lab, PetPanel, and adapter surfaces and would create avoidable merge
conflicts.

### Plan 1: Contract Freeze

Purpose: make Preview Lab strong enough to protect the refactor.

Scope:

- add mandatory typed scene artifacts for every watch or round frame derived
  from `WatchViewModel`
- add round layout/draw-command artifacts so round preview and native companion
  can be compared through the same semantic source
- make animation strips use the deterministic preview clock by fixing
  `scene_strip_bundle` and the `RenderContext::new` seam in `src/dev_preview/strips.rs`
- add cross-renderer fixture checks for normal, active pulse, asleep/night,
  helper trouble, full props, Glitch vs Crystal dialect, and flat color where
  practical
- correct living docs that describe Preview Lab manifest schema `2` when the
  code now uses schema `3`; update `AGENTS.md` and any actively maintained
  Preview Lab docs, but do not churn historical plans/specs except with a small
  "superseded by schema 3" note if needed

Artifact contract:

| Artifact | Path | Manifest/files contract | Producer contract | Required checks |
| --- | --- | --- | --- | --- |
| Scene snapshot | `frames/<id>.scene.json` | Add `PreviewScenarioFiles.scene` and `ArtifactType::Scene`. Link it from `review.md` and `index.html`. | Small sanitized DTO, not a raw `WatchViewModel` dump. Include schema version, pet summary, room summary, target ids, surface privacy projection, and fixture inputs needed for comparison. | Privacy/redaction test: no transcript-like strings, file paths, project names, exact round counts, or raw source details in sanitized surfaces. |
| Round layout | `frames/<id>.round-layout.json` | Add `PreviewScenarioFiles.round_layout` and `ArtifactType::RoundLayout` for round frames. | Serializable DTO around aperture, safe radius, detail level, pet anchor, prop anchors, halo anchors, and motion budget. | Assert aperture, safe radius, pet/prop/halo anchor counts and coordinates stay inside bounds. |
| Round commands | `frames/<id>.round-commands.json` | Add `PreviewScenarioFiles.round_commands` and `ArtifactType::RoundCommands` for round frames. | Serializable DTO around `RoundDrawCommand` data, or a deliberately shaped export DTO if the internal command type stays non-serializable. | Assert command-kind counts, `PetGlyph` text/spans, room glyph vocabulary, trouble/halo commands, and privacy flags. |
| Deterministic strip frame | existing `strips/<id>/frame-NNN.*` plus optional scene artifact only if the implementation plan scopes it | Preserve existing strip paths. | `scene_strip_bundle` must receive/use a fixed preview clock. | Repeated-output test compares strip text/cells across two runs, excluding `generated_at` if it remains wall-clock. |

Round equivalence checks must compare semantics, not pixels only. Required
equivalence includes: same derived round scene fixture id, aperture dimensions,
safe radius, pet anchor, prop/halo anchor counts, command-kind counts, pet text
and spans, room glyph vocabulary, and privacy projection. `round-normal` is the
current full-props round fixture unless Plan 1 intentionally adds a
`round-full-props` fixture.

Non-goals:

- no output redesign
- no renderer migration
- no schema/path removal
- no native lifecycle work

Expected verification:

```bash
cargo test --test dev_preview
cargo test --test round_scene
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Stop conditions:

- any existing preview artifact path must be renamed or removed
- a visual diff appears that is not caused by a newly-added artifact
- deterministic preview text/cell/scene content depends on wall-clock time; the
  only allowed wall-clock value is `generated_at`, and deterministic diff tests
  must either exclude it or the plan must make it fixed for dev-preview

### Plan 2: PetPanel Mechanical Split

Purpose: reduce the largest hotspot without changing behavior.

Scope:

- split `src/tui/panels/pet.rs` into smaller modules while keeping
  `src/tui/panels/pet.rs` as the module root
- initial child modules:
  - `src/tui/panels/pet/ambient.rs`
  - `src/tui/panels/pet/colors.rs`
  - `src/tui/panels/pet/art_lines.rs`
  - `src/tui/panels/pet/composition.rs` if needed
- keep public behavior and existing call sites stable
- do not move the root file to `src/tui/panels/pet/mod.rs`; that would create a
  large noisy diff and unnecessary merge risk
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
cargo test --lib tui::panels::pet
cargo test --test tui_render
cargo test --test dev_preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Stop conditions:

- any intentional visual change is needed to make tests pass
- module extraction requires broad public API changes outside the pet panel
- the split makes circular dependencies that suggest the boundary is wrong

### Plan 3: Presentation Domain Extraction

Purpose: introduce the shared backend-neutral presentation layer. This is a
track, not a single overnight task; split it into smaller plans after Plan 1 and
Plan 2 are merged.

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
- introduce neutral owned target ids such as `EffectTargetId` or
  `SurfaceTargetId`; watch adapters map those to `TargetPath`
- keep habitat props behind wrappers until `PetSceneLayout` is no longer the
  presentation-domain input

Suggested subplans:

- 3a: scene skeleton, privacy projections, neutral ids, and serialization tests
- 3b: pet role/span/color helper extraction behind the new module
- 3c: room profile/glyph vocabulary projection without moving placement yet
- 3d: habitat prop wrappers, only after `PetSceneLayout` coupling is understood

Non-goals:

- no universal renderer trait unless a migrated adapter proves it is necessary
- no immediate deletion of `RoundSceneModel`
- no replacement of `WatchViewModel`
- no native lifecycle changes

Expected verification:

```bash
cargo test --lib tui::room
cargo test --lib tui::component::habitat_props
cargo test --test round_scene
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

Purpose: retire duplicated surface-specific interpretations. This is an adapter
track and must be split by adapter; do not hand the full track to one overnight
agent.

Scope:

- make watch, round preview, native companion, menubar, and Preview Lab consume
  shared scene/primitives once that adapter has matching contract coverage
- unify pet role color and span interpretation
- make round preview consume the same command/layout vocabulary as native
  companion where possible
- keep Preview Lab fixtures near the review harness, but make fixture builders
  return frame plus scenario contract together instead of relying on frame-id
  pattern matching

Suggested subplans:

- 4a: round preview and native companion command/layout convergence
- 4b: watch TUI adapter migration for the pieces already covered by
  `PresentationScene`
- 4c: menubar pet/stat projection, preserving its explicitly privileged surface
  behavior
- 4d: Preview Lab fixture/contract builder cleanup after the artifact contract is
  stable

Non-goals:

- no single renderer required
- no loss of surface-specific layout logic
- no shrunken-dashboard round view
- no privacy regression in companion or menubar

Expected verification:

```bash
cargo test --test round_scene
cargo test --test tui_render
cargo test --test dev_preview
cargo test --test watch_integration
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
