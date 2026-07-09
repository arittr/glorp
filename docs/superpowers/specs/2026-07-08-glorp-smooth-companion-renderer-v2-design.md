# Glorp Smooth Companion Renderer v2 - design

- Date: 2026-07-08
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md`
  - `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`
  - `docs/superpowers/specs/2026-06-24-glorp-companion-tank-redesign-design.md`
  - `docs/superpowers/specs/2026-07-07-glorp-ambient-tank-life-design.md`
  - `docs/superpowers/specs/2026-07-07-glorp-companion-perimeter-gauges-design.md`
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-pixel-companion-design.md`

## Calibration

The goal is not to build a second Pixel companion beside Classic.

The goal is to take the current Classic Glorp companion and upgrade it to a
new smooth-motion-capable renderer so the same companion can support richer
motion, depth, prop reactions, and future interaction without losing Glorp's
existing identity.

The first successful slice should still look like the current Classic
companion: Glorp pet art, habitat props, tank life, ambient glyphs, mood aura,
perimeter gauges, HUD text, and the round porthole composition. The difference
is that the renderer underneath has continuous time, stable semantic layers,
fractional placement, and room for richer effects.

The first implementation must visibly demonstrate that new capability. It may
be a deliberately small motion proof, but it cannot be a renderer-only seam:
the Classic pet layer should show a smooth fractional breath/bob or drift when
run live.

The earlier Pixel companion work is useful as rendering research and fixture
coverage, but it is not the product center. Pixel/3D-ish treatment can become a
style or layer treatment inside Renderer v2 after Classic parity is preserved.

Renderer v2 gets its own hidden development mode: `--renderer smooth`. It must
not reuse `--renderer pixel`, because the current Pixel branch intentionally
bypasses the Classic scene and would preserve the wrong product boundary.

## Problem

The current macOS companion has the right product shape but the wrong rendering
ceiling.

Classic companion rendering goes through the shared round scene path:

```text
WatchViewModel
  -> round::scene::build_round_scene_draw_list(...)
  -> tui::panels::pet::render_pet_to_draw_list_with_tank_geometry(...)
  -> SceneDrawList
  -> companion AppKit blitter
```

That path already carries the important Glorp vocabulary: generated pet art,
props, tank life, ambient marks, room texture, contact shadow, performance cues,
and a drifted `pet_rect`.

But by the time AppKit receives the scene, the meaningful structure has been
flattened into `DrawCell`s. A cell tells us row, column, glyph, foreground,
background, and bold. It does not tell us whether it came from the pet body,
contact shadow, foreground prop, tank life, ambient activity, aura, or a
performance cue. That makes smooth native motion awkward: AppKit can draw
cells, but it cannot easily animate or transform the pet separately from props,
give foreground tank life a distinct z behavior, or apply continuous feed
pulses to the right layer.

The current Pixel companion goes too far in the opposite direction. It bypasses
the Classic scene and draws its own pixel frame plus the shared gauges. That
proved a smooth frame loop and some portable pixel primitives, but it drops the
visual world that makes the companion feel like Glorp.

Renderer v2 exists to fix the seam, not to replace Glorp with a new mascot.

## Goals

1. Preserve Classic companion visual identity as the first acceptance gate.
   Renderer v2 must carry the same pet, props, tank life, ambient marks, aura,
   porthole, HUD, and gauges that Drew recognizes today.
2. Add smooth-motion capability to the existing companion surface. The renderer
   must support fractional placement, continuous clock-driven animation, layer
   transforms, opacity, scale, squash/stretch, parallax, and pulse effects.
3. Keep scene semantics in Rust core modules, not in AppKit. AppKit is the first
   host, but the scene plan should be backend-neutral enough for future hosts.
4. Treat Pixel/3D styling as an optional rendering treatment inside the new
   renderer, not a separate runtime that owns the whole tank.
5. Keep `glorp watch` and the existing Classic companion path intact while
   Renderer v2 is being built and reviewed.
6. Prove parity and motion through Preview Lab artifacts before relying on live
   AppKit review alone.
7. Preserve the existing external-display privacy boundary: visual richness can
   increase, but source names, project context, prompts, file paths, and raw
   diagnostics stay out of the companion.

## Non-goals

- No new standalone Pixel companion product.
- No default flip until Classic parity and live visual review pass.
- No removal of the existing Classic companion renderer in the first v2 slice.
- No full 3D engine, camera system, physics engine, or authored asset pipeline
  in this spec.
- No rewrite of `glorp watch`.
- No Linux windowing implementation in the first slice.
- No hand-filtering flattened cells by glyph/color to guess which layer they
  belong to.
- No weakening of the current privacy stance for external companion surfaces.

## Product Model

Renderer v2 should answer the same question as the Classic companion:

> Is my Glorp here, alive, okay, and reacting to real work?

The companion remains a round porthole into Glorp's habitat. The center of the
surface is still the pet and the tank. Gauges and HUD remain secondary. Motion
should make the existing composition feel alive instead of turning the window
into a dashboard or a separate minigame.

First-release motion language:

- Pet drift uses fractional positions rather than snapped cell-only movement.
- Breath/bob is continuous and distinct from the larger tank wander.
- Blink timing remains pet-specific and state-aware.
- Feed/activity pulses brighten or ripple through the aura and pet layer, not
  through unrelated props.
- Props and tank life keep their current placement identity; they may gain small
  reaction transforms only after their static parity is proven.
- Layered depth is subtle: contact shadow, aura, foreground life, and pet body
  should read as separated without becoming noisy.

## Architecture

Renderer v2 adds one new boundary between the Classic scene builder and AppKit:
a layer-aware companion scene plan.

```text
WatchViewModel + now
  -> existing Classic companion scene derivation
  -> SmoothCompanionScenePlan
       semantic layers, anchors, bounds, z order, privacy-safe metadata
  -> renderer-specific output
       AppKit v2 first
       Preview Lab parity/motion artifacts
       future pixel/3D-ish style treatments
```

### `SmoothCompanionScenePlan`

The plan is the resolved, privacy-safe scene that every smooth renderer consumes.
It keeps Classic semantics before the scene becomes a flat cell list.

The exact names can change during implementation, but the data should include:

```rust
pub struct SmoothCompanionScenePlan {
    pub viewport: CompanionViewport,
    pub layers: Vec<SmoothCompanionLayer>,
    pub pet: SmoothCompanionPet,
    pub chrome: CompanionChromeReservation,
    pub privacy: SmoothCompanionPrivacyClaims,
}

pub struct SmoothCompanionLayer {
    pub id: SmoothLayerId,
    pub role: SmoothLayerRole,
    pub z: i16,
    pub local_bounds: SmoothBounds,
    pub anchor: SmoothPoint,
    pub transform_origin: SmoothPoint,
    pub transform: SmoothTransform,
    pub opacity: f32,
    pub clip: SmoothClip,
    pub blend: SmoothBlendMode,
    pub items: Vec<SmoothLayerItem>,
    pub animation: SmoothLayerAnimation,
}

pub enum SmoothLayerItem {
    Cell(SmoothLocalCell),
    Shape(SmoothShape),
    Raster(SmoothRasterRef),
}

pub struct SmoothLocalCell {
    pub local_col: u16,
    pub local_row: u16,
    pub glyph: String,
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: bool,
}

pub enum SmoothLayerRole {
    DepthRings,
    BiomeWash,
    RoomGlyphs,
    Ambient,
    Motes,
    ActivityGlyphs,
    PropsBehind,
    TankLifeBehind,
    ChestBubble,
    ContactShadow,
    PetBody,
    PerformanceCue,
    PropsForeground,
    TankLifeForeground,
    StatusHalo,
    TroubleIndicator,
    MoodAura,
    DimOverlay,
}
```

`DrawCell` is a compatibility output, not the smooth scene primitive.
`SmoothCompanionScenePlan::flatten_classic_cells()` must produce the current
`SceneDrawList` ordering for the Classic cell layers. Smooth renderers consume
the local layer items and transforms directly.

`chrome` records safe regions, gauge lanes, HUD bounds, and privacy claims. It
does not carry exact token strings, source names, project context, file paths,
or raw usage rows. AppKit may keep deriving and drawing the existing HUD and
perimeter gauges at draw time in the first slice, but the v2 plan must reserve
their space so animated layers do not collide with them.

`SceneDrawList` may stay as the backend-agnostic low-level artifact, but
Renderer v2 needs a higher-level plan or a role-bearing draw list before
flattening. The implementation should not infer layer roles from glyphs,
colors, or pass order after the fact.

### Scene construction

The first implementation should reuse the Classic scene builder rather than
replace it.

Today, `render_pet_to_draw_list_with_tank_geometry` already has clear pass
boundaries:

1. biome wash
2. room glyphs
3. ambient glyphs
4. motes
5. activity glyphs
6. background/behind props and tank life
7. treasure chest bubble
8. contact shadow
9. pet body
10. performance cue
11. foreground props and tank life

Renderer v2 should preserve those passes as typed layers. That can be done by
adding a companion-oriented `LayeredPetScene` builder beside the current
draw-list function, then flattening it for existing callers. The first
implementation should not evolve `SceneDrawList` with optional metadata and
then ask consumers to infer semantics later; parity depends on preserving roles
before flattening.

Required construction contract:

```text
render_layered_pet_scene_with_tank_geometry(...)
  -> LayeredPetScene
  -> flatten_classic_cells() -> SceneDrawList
```

For fixed `WatchViewModel`, `now`, grid, tank geometry, and companion motion,
`flatten_classic_cells()` must match the current
`build_round_scene_draw_list(...).draw_list` exactly for the Classic cell
layers, including the existing uniform porthole recolor post-pass. Mood aura,
depth rings, dim overlay, status halo, trouble indicator, HUD, and gauges are
companion chrome/layers outside the existing `SceneDrawList`; they need separate
geometry and privacy assertions.

The important constraint: Classic TUI/watch rendering should not be disrupted
while the companion v2 seam is being built.

### Motion model

Motion should be driven by continuous time and stable scene anchors.

```rust
pub struct SmoothCompanionMotionState {
    pub started_at: OffsetDateTime,
    pub elapsed_ms: u64,
    pub previous_pet_anchor: SmoothPoint,
    pub target_pet_anchor: SmoothPoint,
    pub blink_schedule: BlinkSchedule,
    pub pulse_windows: Vec<PulseWindow>,
}

pub struct SmoothFrameTick<'a> {
    pub plan: &'a SmoothCompanionScenePlan,
    pub now: OffsetDateTime,
    pub state: &'a mut SmoothCompanionMotionState,
}
```

The existing `round::scene::CompanionMotion` and `companion_roam_motion()` are
good starting inputs, but Renderer v2 should apply movement as fractional
transforms in the renderer rather than only snapping `pet_rect` to a cell. The
pet layer can keep a cell-art source while rendering at fractional AppKit
coordinates.

Motion state is owned by the host adapter, just like the current Pixel renderer
state is stored in `AppState`, but the state type remains portable Rust. Preview
Lab owns its own deterministic state while exporting strips. Live AppKit should
drive interpolation from a monotonic elapsed time; wall-clock `OffsetDateTime`
is still used to derive Glorp state and deterministic fixtures. On poll updates,
the state preserves motion continuity for the same pet identity and viewport by
rebasing the current rendered anchor as `previous_pet_anchor`; it resets on
renderer mode changes, pet identity/stage changes, or viewport-class changes.

### AppKit v2 adapter

The AppKit adapter remains a host, not the owner of scene semantics.

Responsibilities:

- window lifecycle
- display timer
- circular clipping and native coordinate transforms
- font/glyph measurement or bitmap drawing
- mapping layers to AppKit draw operations
- preserving the existing perimeter gauges and HUD composition

Non-responsibilities:

- deriving prop placement
- choosing tank life cast
- deciding what counts as pet body vs foreground prop
- deciding privacy projection
- owning pet identity or state reactions

The first AppKit v2 output can still render Classic glyph cells. The win is that
those glyph cells are in named layers with fractional transforms and separate
animation channels. Pixel or richer raster treatment can follow by replacing the
`PetBody` layer renderer while keeping all other layers.

The existing Pixel renderer is not already a valid `PetBody` replacement: today
it emits a full frame that includes aura, shadow, body, face, and aperture
clearing. Slice 4 requires decomposing that work into body/style primitives that
fit inside Renderer v2's `PetBody` and related layer roles.

### Preview Lab

Preview Lab is the review contract for this work. The v2 spec needs artifacts
that prove both parity and motion:

- `dev-preview --scenario smooth` is the owner for v2 review artifacts.
- `frames/round-smooth-classic-baseline.*` captures the current Classic fixture.
- `frames/round-smooth-classic-parity.*` captures v2 with the same fixture.
- `frames/round-smooth-classic-parity.smooth-plan.json` records the layer plan:
  schema version, target renderer, layer ids, item ids, roles, z order, local
  bounds, transforms, opacity, blend mode, clip, item counts, chrome safe
  regions, privacy claims, and a flatten checksum.
- `frames/round-smooth-classic-parity.smooth-parity.json` records parity:
  Classic draw-list checksum, v2 flatten checksum, exact-match status or named
  allowable deltas, required-role presence, fixture id, and review status.
- `strips/round-smooth-motion/frame-NNN.smooth-motion.json` records fractional
  pet anchor, bob, scale, opacity, pulse, and layer-transform values. The cell
  strip alone is not enough because fractional motion may not cross a cell
  boundary every frame.
- The scenario includes side-by-side review links for Classic baseline and v2
  parity, plus deterministic fixtures for active pulse, asleep/calm, helper
  trouble, and a tank-prop-rich habitat with an earned treasure chest.

`manifest.json` must expose these as first-class preview artifacts, not only as
loose extra files: frame entries get `files.smooth_plan` and
`files.smooth_parity`, strip frame entries get `files.smooth_motion`, and the
artifact inventory uses `smooth-plan`, `smooth-parity`, and `smooth-motion`
types. Existing `.hud.json` / `PreviewHudArtifact` remains the HUD evidence
contract; smooth-plan `chrome` is reserve geometry and privacy metadata only.

The existing Pixel composition sidecar currently records props and tank life as
unavailable for Pixel runtime. Renderer v2 should invert that contract: props
and tank life are required for v2 parity, and absence is a failing artifact.

Smooth plan, parity, and motion artifacts must participate in the same privacy
scan pattern as existing round and Pixel artifacts. They may include semantic
roles, safe regions, checksums, and abstract state buckets; they must not include
source names, exact token strings, project/file paths, prompts, responses, raw
diagnostics, or unprojected pet seed values.

## Rollout Shape

### Slice 1: layer-aware Classic parity plus visible motion proof

Create the scene-plan seam and render a v2 parity frame that looks like Classic.
The plan must preserve layer identity and also apply one small smooth transform
to the Classic pet layer so Drew can see the new renderer doing something that
the old cell-snapped path cannot do.

The required first motion is intentionally narrow: a fractional pet breath/bob
or gentle sub-cell drift on the `PetBody` layer, driven by continuous elapsed
time and exported as motion metadata. It must not replace the pet art, move
props independently yet, or turn the companion into a separate Pixel treatment.

Acceptance:

- v2 adds `--renderer smooth`; Classic remains the default; v2 does not reuse
  the existing Pixel branch.
- `dev-preview --scenario smooth` emits baseline, parity, smooth-plan,
  smooth-parity, and motion sidecars.
- The smooth plan includes required roles for biome wash, room glyphs, ambient,
  motes, activity glyphs, props behind, tank life behind, chest bubble when the
  fixture earns it, contact shadow, pet body, performance cue, props foreground,
  tank life foreground, status halo, trouble indicator, mood aura, depth rings,
  dim overlay, and chrome reservations.
- For fixed parity fixtures, `flatten_classic_cells()` matches the existing
  Classic companion draw-list exactly for cell layers.
- The smooth renderer applies a visible fractional `PetBody` transform in live
  AppKit review mode while preserving the Classic flattened cell source.
- `strips/round-smooth-motion/frame-NNN.smooth-motion.json` shows changing
  fractional pet bob/drift values across at least five frames before Slice 1 is
  considered reviewable.
- Privacy scans pass for all smooth sidecars.
- Drew reviews the Classic/v2 side-by-side parity artifact and explicitly
  accepts that it still reads as the current Glorp companion.
- Existing Classic companion path remains available.

### Slice 2: smooth pet motion

Expand the visible proof into the full smooth-motion behavior: richer
drift/bob/blink/pulse and native review capture while keeping Classic art and
composition.

Acceptance:

- Motion strip shows fractional pet position or scale values changing between
  frames.
- Live companion visibly moves smoothly.
- Prop and tank-life layers remain present and correctly ordered.
- Bounded native smoke captures screenshots for `--renderer smooth` and the
  current Classic renderer at 260, 360, and 480 px review sizes. Each run exits
  on its own after a fixed duration, writes a screenshot plus render log, renders
  at least five frames, reports no panic, and exits 0.
- Native smoke covers normal, active pulse, asleep/calm, and helper trouble
  fixtures. The implementation must extend the hidden review harness beyond the
  current size/active-pulse flags before claiming those states are covered.
- Drew reviews the native Classic-vs-smooth screenshot set and explicitly
  accepts that the live AppKit renderer still reads as the current companion.

### Slice 3: depth and polish

Add stronger depth treatment: contact shadow tuning, rim light, aura pulse,
subtle parallax, and state-specific reactions.

Acceptance:

- Pet is larger and more hero-like without crowding gauges.
- The tank feels inhabited, not like a blob floating above a dashboard.
- Visual review confirms the result still reads as Glorp.
- Machine evidence confirms the pet, props, tank life, ambient marks, activity
  marks, aura, HUD reserve, gauges reserve, porthole mask, and privacy claims
  remain present after depth changes.

### Slice 4: optional pixel/3D-ish pet treatment

Only after Classic parity and smooth motion pass, experiment with replacing the
`PetBody` layer renderer with a pixel/raster/3D-ish treatment. This treatment
must live inside the v2 scene plan so props, tank life, ambient marks, and HUD
remain intact.

Acceptance:

- Pixel treatment does not own the whole tank.
- It uses the correct Glorp identity and existing pet art semantics.
- Turning it off returns to Classic pet body rendering without changing the
  surrounding companion scene.
- The treatment is built from decomposed body/style primitives, not the current
  full-frame Pixel blit.

## Testing and Verification

Required implementation checks:

- Unit tests for layer construction: each Classic pass maps to a named role and
  the flattened result can reproduce the current draw-list ordering.
- Unit tests for local layer coordinates, transforms, opacity, clipping, and
  flattening rules.
- Unit tests for pet layer bounds and face-protected regions so tank life keeps
  avoiding the pet.
- Preview Lab tests for v2 parity artifact presence, schema stability, manifest
  links, flatten checksums, required roles, and privacy scans.
- Motion determinism tests: fixed input, fixed initial state, fixed timestamps,
  and fixed viewport produce deterministic frame plans.
- AppKit smoke protocol for `--renderer smooth`: direct `companion-app` review
  runs at 260, 360, and 480 px, each with `--review-capture-dir`,
  `--review-duration-ms`, and a forced review state. A passing run writes
  `screenshot.png` and `render-log.json`, records at least five rendered frames,
  exits 0 without manual quit, and has no panic/crash report for that run.
- Existing relevant suites continue to pass:
  - `cargo test --test round_scene`
  - `cargo test --features dev-preview --test dev_preview`
  - focused companion renderer tests added for `smooth`
  - Pixel tests only when Slice 4 touches the Pixel renderer or shared pixel
    primitives

Visual verification:

- `cargo run -- dev-preview --scenario round --out target/glorp-preview`
- `cargo run -- dev-preview --scenario smooth --out target/glorp-preview`
- `cargo xtask companion fresh` for the default Classic path
- `cargo run -- companion --renderer smooth` once the implementation exposes it
- `cargo run -- companion-app --renderer smooth --review-size 360x360
  --review-state active-pulse --review-duration-ms 2000
  --review-capture-dir target/glorp-review/smooth-360-active`

## Decisions for Implementation Planning

1. Add a new layered scene plan and flattening compatibility path. Do not rely
   on optional metadata attached to already-flattened cells.
2. Use `--renderer smooth` for the hidden live development path.
3. Add `dev-preview --scenario smooth` for Renderer v2 parity/motion artifacts.
4. The first AppKit v2 adapter may render glyph cells directly with
   fractional positions or rasterize layers into offscreen images before
   compositing. The implementation plan should choose the smallest path that
   satisfies the parity and smooth-motion tests.
5. HUD and gauges remain companion chrome in the first slice. The v2 plan
   carries safe regions and privacy claims, not exact HUD/gauge values.
6. Status halo and helper-trouble indicators are v2 visual layers/chrome with
   explicit roles; they are not inferred from generic overlay cells.

## Success Standard

We are building for the current Classic Glorp companion becoming alive enough to
support fancy motion and interactions.

The first implementation is successful when Drew can open the new renderer and
say: "That is my existing Glorp companion, with the same tank and pet world, but
now it moves like a native animated creature."
