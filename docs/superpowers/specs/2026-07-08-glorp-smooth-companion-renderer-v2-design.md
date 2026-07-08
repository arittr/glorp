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

The earlier Pixel companion work is useful as rendering research and fixture
coverage, but it is not the product center. Pixel/3D-ish treatment can become a
style or layer treatment inside Renderer v2 after Classic parity is preserved.

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
    pub gauges: CompanionGaugeModel,
    pub hud: CompanionHudModel,
}

pub struct SmoothCompanionLayer {
    pub id: SmoothLayerId,
    pub role: SmoothLayerRole,
    pub z: i16,
    pub anchor: SmoothAnchor,
    pub bounds: SmoothBounds,
    pub cells: Vec<DrawCell>,
    pub animation: SmoothLayerAnimation,
}

pub enum SmoothLayerRole {
    BackgroundWash,
    RoomGlyphs,
    Ambient,
    Activity,
    PropsBehind,
    TankLifeBehind,
    ContactShadow,
    PetBody,
    PerformanceCue,
    PropsForeground,
    TankLifeForeground,
    Aura,
}
```

`gauges` and `hud` may remain AppKit-drawn companion chrome in the first slice.
They still belong in the v2 plan as reserved regions and review metadata so the
animated scene does not collide with the lower text or perimeter rings.

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
adding a companion-oriented layered render function beside the current draw-list
function, or by evolving the draw-list function to emit layer groups and then
flatten for existing callers.

The important constraint: Classic TUI/watch rendering should not be disrupted
while the companion v2 seam is being built.

### Motion model

Motion should be driven by continuous time and stable scene anchors.

```rust
pub struct SmoothCompanionMotionState {
    pub started_at: OffsetDateTime,
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

### Preview Lab

Preview Lab is the review contract for this work. The v2 spec needs artifacts
that prove both parity and motion:

- Classic companion baseline frame.
- Smooth v2 parity frame with the same fixture.
- Layer inventory sidecar listing layer ids, roles, z order, bounds, and cell
  counts.
- Motion strip showing fractional pet anchor/bob values across frames.
- Acceptance sidecar proving props, tank life, ambient/activity glyphs, pet body,
  foreground layers, HUD-safe region, and gauges are all represented.

The existing Pixel composition sidecar currently records props and tank life as
unavailable for Pixel runtime. Renderer v2 should invert that contract: props
and tank life are required for v2 parity, and absence is a failing artifact.

## Rollout Shape

### Slice 1: layer-aware Classic parity

Create the scene-plan seam and render a v2 parity frame that looks like Classic.
Motion can be minimal, but the plan must preserve layer identity.

Acceptance:

- v2 Preview Lab frame includes pet, props, tank life, ambient, activity, aura,
  HUD-safe region, and foreground layers.
- AppKit v2 can render the same plan without crashing.
- Existing Classic companion path remains available.

### Slice 2: smooth pet motion

Move pet drift/bob/blink/pulse to continuous renderer transforms while keeping
Classic art and composition.

Acceptance:

- Motion strip shows fractional pet position or scale values changing between
  frames.
- Live companion visibly moves smoothly.
- Prop and tank-life layers remain present and correctly ordered.

### Slice 3: depth and polish

Add stronger depth treatment: contact shadow tuning, rim light, aura pulse,
subtle parallax, and state-specific reactions.

Acceptance:

- Pet is larger and more hero-like without crowding gauges.
- The tank feels inhabited, not like a blob floating above a dashboard.
- Visual review confirms the result still reads as Glorp.

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

## Testing and Verification

Required implementation checks:

- Unit tests for layer construction: each Classic pass maps to a named role and
  the flattened result can reproduce the current draw-list ordering.
- Unit tests for pet layer bounds and face-protected regions so tank life keeps
  avoiding the pet.
- Preview Lab tests for v2 parity artifact presence and schema stability.
- Motion determinism tests: fixed input, fixed initial state, fixed timestamps,
  and fixed viewport produce deterministic frame plans.
- Existing companion crash regression stays covered by the fractional aperture
  test added for the AppKit pixel path.
- Existing relevant suites continue to pass:
  - `cargo test --test round_scene`
  - `cargo test --test pixel_renderer`
  - `cargo test --test pixel_fit`
  - `cargo test --features dev-preview --test dev_preview`
  - `cargo test companion::pixel`

Visual verification:

- `cargo run -- dev-preview --scenario round --out target/glorp-preview`
- new v2 Preview Lab scenario or round fixture once implemented
- `cargo xtask companion fresh` for the default Classic path
- v2/pixel-style launch command once the implementation exposes it

## Open Decisions for Implementation Planning

1. Whether to add `LayeredSceneDrawList` beside `SceneDrawList`, or evolve
   `SceneDrawList` with optional layer metadata while keeping existing blitters
   simple.
2. Whether the first AppKit v2 adapter renders glyph cells directly with
   fractional positions or rasterizes layers into offscreen images before
   compositing.
3. Whether Renderer v2 should launch behind the existing `--renderer pixel`
   flag during development or get a new hidden flag such as `--renderer smooth`.
4. Which Preview Lab scenario owns the v2 parity contract: extend `round`, add
   `smooth`, or add a focused `companion-v2` scenario.

## Success Standard

We are building for the current Classic Glorp companion becoming alive enough to
support fancy motion and interactions.

The first implementation is successful when Drew can open the new renderer and
say: "That is my existing Glorp companion, with the same tank and pet world, but
now it moves like a native animated creature."
