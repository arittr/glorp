# Glorp Smooth Pixel Companion - design

- Date: 2026-07-08
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-06-13-glorp-macos-round-companion-design.md`
  - `docs/superpowers/specs/2026-06-15-glorp-presentation-architecture-design.md`
  - `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`
  - `docs/superpowers/specs/2026-06-24-glorp-companion-tank-redesign-design.md`
  - `docs/superpowers/specs/2026-07-07-glorp-companion-perimeter-gauges-design.md`

## Problem

The current companion proves the ambient direction: it is a native macOS window,
shares Glorp's live state, renders the pet inside a round tank, and already has
time-based drift. But it still ultimately draws a terminal-cell scene. The pet
can wander smoothly as a scene, while its body still reads as enlarged glyphs
snapping between character-cell poses.

Drew wants to test the next product direction: the companion as the living pet
surface, with smooth pixel motion rather than a pure 2D terminal/tick-based
renderer. The first implementation should visibly move. A renderer-only prep
diff would not answer the product question.

At the same time, we should not hard-code the pet into AppKit. Linux is not a
first-release requirement, but the renderer should be portable enough that a
future Linux companion is a host adapter, not a rewrite of the creature logic.

## Direction

Add a **Smooth Pixel Companion** renderer for `glorp companion`.

The first implementation is opt-in/internal, keeps the existing companion
renderer intact, and displays a visibly animated pixel pet in the macOS
companion window. The intended end state is for Pixel to become the default
companion renderer after visual review and tuning.

The terminal watch remains a terminal-native surface. This project creates a
new smooth companion surface; it does not replace `glorp watch`.

## Goals

1. **Visible animated MVP.** The first diff must show a smooth pixel pet moving
   in the companion window. It should not stop at scaffolding.
2. **Portable renderer core.** Pet composition, frame timing, animation phases,
   and pixel output live in cross-platform Rust modules with no AppKit or
   `objc2` dependency.
3. **AppKit host first.** The first host is the existing macOS companion app.
   AppKit owns windowing, timer/display plumbing, and scale-to-fit.
4. **Classic renderer fallback.** The existing round companion renderer remains
   available during rollout.
5. **State continuity.** Pixel consumes the same `WatchViewModel`, usage polling,
   pet state, species/stage/mood identity, activity signal, and sleep/calm state
   as the current companion.
6. **Deterministic review loop.** Fixed input state, renderer state, viewport,
   and timestamp produce deterministic portable frames that tests and Preview Lab
   can inspect without launching AppKit.

## Non-goals

- No Linux window/app in the first implementation.
- No 3D, voxel engine, camera, lighting model, rigging, or external asset
  pipeline.
- No full habitat recreation in pixels.
- No terminal watch renderer changes.
- No permanent public "two companion products" mode.
- No removal of the classic companion renderer in the first pixel diff.
- No stage-by-stage bespoke authored sprite library required for V1.
- No reading terminal-rendered `vm.pet_art` / `vm.pet_spans`, and no calling
  `rerender_pet_for_view_model` from Pixel mode.

## Product Boundary

The first pixel companion is a small, visibly alive pet surface:

- smooth subpixel wander inside the companion window
- independent breathing or bobbing
- irregular per-pet blink timing
- simple aura and shadow so the pet reads as a creature in space
- both required state reactions:
  - asleep/calm reduces motion and changes posture/energy
  - feed/activity pulse briefly brightens or perks the pet

The V1 art target is "directionally right and alive," not final mascot art. It
may use compact procedural/tile composition derived from the existing
species/stage/mood identity and Glorp palette. More authored species/stage art
can follow once the smooth companion direction proves worth deepening.

### V1 Hero Art Contract

To avoid spreading the first pass across six generic blobs, V1 has two hero
fixtures that must look intentionally species-specific before Pixel can be
considered a product win:

1. **Fuzz S3 content, idle.** Round/fluffy silhouette, separate face layer, soft
   breath, warm aura, and a clearly readable blink.
2. **Glitch S4 content, active feed pulse.** Blockier silhouette, sparse
   corruption/noise, scan-like particle or accent, and a visibly bounded pulse.

All species must render without panics or empty frames, but non-hero species can
start as compatibility variants using the shared pixel grammar. The Preview Lab
must include side-by-side Classic and Pixel review fixtures for the hero cases,
so reviewers can decide whether Pixel reads as "my Glorp, but alive" rather than
as a disconnected mascot.

## Architecture

Split the work into a portable pixel renderer and a platform host.

```text
WatchViewModel + shared presentation policy
        |
        v
sanitized PixelPetInput + viewport + timestamp
        |
        v
presentation::pixel_scene
  semantic pixel pet scene:
  species, stage, mood, palette, activity, sleep/calm, feed pulse,
  animation phases, layer intents
        |
        v
presentation::pixel_animator
  portable state:
  prior targets, blink schedule, pulse replay, interpolation state
        |
        v
presentation::pixel_frame
  portable logical RGBA frame
  no AppKit, no objc2, no window assumptions
        |
        v
companion AppKit adapter
  NSWindow, timer, scale-to-fit, display, renderer selection
```

### `presentation::pixel_scene`

`pixel_scene` derives the "what" of the animated pet from Glorp state. It should
consume a sanitized pixel input, not raw terminal-renderer output:

```rust
PixelPetIdentity {
    species: Species,
    stage: Stage,
    variation_key: PixelVariationKey,
}

PixelPetInput {
    identity: PixelPetIdentity,
    mood: Mood,
    palette: ResolvedColors,
    activity: PixelActivity,
    sleep: PixelSleepState,
    pulse: PixelPulseState,
}
```

`PixelPetInput` may be built from `WatchViewModel` inside `presentation`, but it
must not carry source names, exact counts, file paths, project names, raw
diagnostics, prompt/response text, or seed values that the round companion would
otherwise redact. Pixel preview metadata uses the same sanitized privacy stance
as `PresentationSurface::RoundCompanion` / `PreviewLabArtifact`.

`PixelVariationKey` is stable and deterministic, but not a raw pet seed. Derive
it from the pet seed through a one-way projection or small variation bucket so
blink cadence, idle timing, and silhouette accents remain per-pet without
leaking the underlying seed into `PixelPetInput`, `PixelPetScene`, `PixelFrame`,
or preview metadata.

It produces a `PixelPetScene` with semantic data:

- pet identity: species, stage, mood, `PixelVariationKey`-derived variation
- palette roles resolved from the existing pet palette
- animation phases: wander, breath, blink, idle gesture, particle, pulse
- activity/sleep/calm modifiers
- layer intents: shadow, aura, body, eyes, mouth, accents, particles

The scene must not contain AppKit objects, font metrics, `NSColor`, or terminal
cell data. It may contain subpixel positions and normalized animation values.
Pixel must reuse shared presentation policy where applicable, including a
pixel-specific `PIXEL_STYLE` in `presentation::surface`, instead of inventing a
parallel color/privacy stack.

### `presentation::pixel_animator`

Snap-free motion needs portable state. AppKit should store this state, but not
interpret or mutate its internals.

```rust
pub struct PixelRendererState {
    // opaque to platform hosts
}

pub struct PixelRendererTick<'a> {
    pub input: &'a PixelPetInput,
    pub viewport: PixelViewport,
    pub now: OffsetDateTime,
    pub state: &'a mut PixelRendererState,
}
```

For fixed input sequence, initial state, viewport, and timestamps, the animator
must produce deterministic frames and next-state transitions. It owns:

- previous and target wander positions
- blink schedule
- pulse replay windows
- poll-update interpolation
- any per-pet idle gesture state

The AppKit host only initializes, stores, and passes `PixelRendererState`.

### `presentation::pixel_frame`

`pixel_frame` turns the semantic scene into a portable RGBA frame:

```rust
pub struct PixelViewport {
    pub logical_width: u16,
    pub logical_height: u16,
}

pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct PixelFrame {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Rgba8>,
}
```

Draw command helpers may exist internally, but the public V1 artifact is the
dense RGBA frame. That gives AppKit, Preview Lab, and a future Linux host the
same portable object. The AppKit adapter should receive "pixels to display," not
pet semantics it must interpret.

Frame invariants:

- V1 default logical companion frame is `96x96`.
- `pixels.len() == width * height`.
- Pixels are row-major, top-left origin.
- Pixels are sRGB, unpremultiplied RGBA8.
- The outside of the round aperture is transparent alpha `0`; the host composites
  the frame into the existing companion aperture.
- Animation state can use continuous positions, but rasterization snaps hard
  pixel-art layers to logical pixels unless a deliberate soft aura/shadow layer
  says otherwise.
- Platform hosts scale the entire logical frame to the window using
  nearest-neighbor interpolation. Soft aura/shadow must be pre-rendered inside
  the logical frame; hosts must not need layer semantics or host-side effects.

### AppKit companion adapter

The macOS companion adapter owns:

- renderer selection
- NSWindow / NSView lifecycle
- redraw timer
- viewport measurement
- scale-to-fit into the current round companion window
- converting the portable frame into an AppKit image or direct draw
- preserving existing companion overlays above the Pixel interior, including the
  top-layer halo/trouble overlay, perimeter gauges, and bottom HUD, unless a
  later spec explicitly replaces them

It should not own:

- pet animation math
- blink/wander/breath phase derivation
- species art rules
- palette semantics
- state reaction rules

Pixel mode replaces the current terminal-cell tank/pet interior. It does not
replace the halo/trouble overlay, perimeter gauges, or bottom HUD in the first
implementation. Those remain AppKit/HUD-owned overlays, which keeps the first
pixel pass focused on the living pet.

## Renderer Selection And Rollout

The first pixel implementation adds an internal renderer selection:

```rust
enum CompanionRendererMode {
    Classic,
    Pixel,
}
```

For the first implementation:

- `Classic` remains the default.
- `Pixel` is opt-in through a hidden/internal switch.
- The switch is a review and rollback mechanism, not a long-term public product
  promise.

Recommended switch:

- Add a hidden `--renderer classic|pixel` argument to `glorp companion`.
- Add the same hidden argument to the hidden `companion-app` subcommand.
- Do not gate this argument behind the `dev-preview` feature; it must exist in
  the app bundle used for local visual review.
- `glorp companion --renderer pixel` launches the app bundle with
  `open -n <Glorp.app> --args --renderer pixel` so an already-running Classic
  companion cannot mask the selected mode.
- The app bundle launcher already forwards extra args after `companion-app`, so
  direct `open ... --args --renderer pixel` also works for manual review.
- Add CLI smoke coverage for default Classic, explicit Classic, explicit Pixel,
  hidden help behavior, direct `companion-app --renderer pixel`, and non-macOS
  parse/error behavior.

Rollout sequence:

1. Land portable pixel renderer plus AppKit host path behind the switch.
2. Review deterministic Preview Lab artifacts and run the live companion in
   Pixel mode.
3. Tune until Pixel is clearly better than Classic.
4. Flip Pixel to default in a small follow-up.
5. Keep Classic briefly as rollback.
6. Delete Classic after Pixel is trusted.

## Data Flow

Live data flow stays the same as the current companion:

1. Load initial pet state and build `WatchViewModel`.
2. Spawn the existing live watch worker.
3. Slow usage polls update `WatchViewModel` and presentation state.
4. Fast Pixel redraw ticks continue to animate from the last known sanitized
   `PixelPetInput`.
5. Pixel renderer receives input, viewport, mutable `PixelRendererState`, and
   `now`.
6. AppKit displays the portable frame.

Usage polling remains slow. Animation is continuous between polls.

## Timing Model

Animation must be time-based, not frame-count based.

- Fixed `PixelPetInput` sequence, initial `PixelRendererState`, viewport, and
  timestamps yield deterministic output.
- Pixel mode uses a separate fast redraw path targeting 30 FPS first. Classic
  can keep its existing slower timer.
- Pixel fast ticks must not rerender terminal pet art, rebuild the round
  `SceneDrawList`, or derive `RoundSceneModel` every frame.
- Reuse frame buffers where practical; avoid per-frame large allocations in the
  hot path.
- 60 FPS is allowed only if CPU cost is acceptable.
- Activity and calm/sleep affect amplitude, density, brightness, and pose.
  They should not cause hard phase jumps.
- Reduced/calm motion clamps amplitude and particle density rather than freezing
  the pet dead.
- Poll updates may change the target state, but interpolation should avoid a
  visible snap when possible.
- Before flipping Pixel to default, measure Classic and Pixel CPU over the same
  60-second idle and active-review windows at the default companion size.
- Default flip budget: Pixel's average process CPU at default size must stay
  within 2 percentage points of Classic during the idle window and within 5
  percentage points of Classic during the active-review window. If either budget
  fails, Classic remains default until a follow-up tuning pass records a new
  measured rationale and budget.

## Visual Model V1

V1 should use a compact pixel-art representation, not terminal glyphs scaled up.
It may be procedural and small.

Minimum visual layers:

1. **Shadow/contact.** A soft pixel shadow or grounding blob below the pet.
2. **Aura.** Mood/activity color glow behind the body.
3. **Body.** A species-influenced pixel silhouette.
4. **Face.** Eyes and mouth as separate pixel layers so blink/mood can animate.
5. **Accent/particles.** A few species-flavored particles or glints.

Species differentiation can start lightweight:

- Fuzz: round/fluffy silhouette, soft bob, warm particles.
- Blob: squashier silhouette, slower elastic breath.
- Ghost: translucent/wispy body, floatier wander.
- Glitch: blockier body, sparse pixel corruption/noise.
- Crystal: angular body, glints.
- Mech: blocky/mechanical body, small status sparks.

Stage differentiation can be coarse in V1. Size, accent density, or silhouette
complexity is enough for the first pass.

## Preview Lab

Add deterministic pixel preview artifacts before relying on live AppKit review.

The preview should include at least:

- idle strip
- asleep/calm strip
- feed/activity pulse strip
- species matrix static frame or short strip

The V1 artifact format is JSON plus the existing Preview Lab HTML viewer:

- `frames/<id>.pixel.json` stores width, height, elapsed/timestamp metadata, and
  row-major RGBA hex colors.
- `index.html` renders those frames to a canvas for visual review.
- PNG export is optional follow-up, not required for the first implementation.

This is an explicit Preview Lab schema extension, not an ad hoc sidecar. The
implementation plan must add:

- a pixel scenario or scenario selection entry
- `ArtifactType::PixelFrame`
- pixel frame file slots in the manifest contract
- a Preview Lab `SCHEMA_VERSION` bump
- a `schema_version` field in `frames/*.pixel.json`
- strip metadata for pixel animation frames
- a `write_pixel_json` export path
- canvas rendering in `index.html` with image smoothing disabled
- manifest, HTML-link, artifact-inventory, and scenario tests

Strip contract:

- each strip includes at least 48 frames over at least 1600 ms, unless a named
  scenario documents a shorter pulse window
- every frame records `elapsed_ms`
- idle strip includes at least one blink event
- asleep/calm strip shows lower movement amplitude than idle
- feed/activity strip shows a bounded brightness or pose change
- tests assert non-empty pixel deltas, movement bounds, and no viewport writes
  outside the frame

The review contract should record:

- scenario id
- viewport size
- timestamp or elapsed time
- renderer mode
- species/stage/mood inputs
- frame artifact paths

Do not require AppKit to generate these artifacts.

## Testing

Pure tests:

- fixed inputs produce identical `PixelPetScene`
- fixed input sequence plus initial `PixelRendererState` produces identical
  `PixelFrame` output and next-state transitions
- animation phases are deterministic and bounded
- blink cadence is irregular but reproducible for a seed
- asleep/calm reduces motion amplitude
- feed/activity pulse changes brightness or pose for a bounded time
- all frame writes stay within the viewport
- all species render non-empty, in-bounds frames through the shared pixel grammar
- changes to species, stage, mood, asleep/calm, and feed/activity pulse affect
  the sanitized `PixelPetInput` and at least one controlled `PixelFrame` fixture
- `PixelPetInput` / pixel preview metadata contains no source names, exact
  counts, file paths, project names, raw diagnostics, prompt/response text, or
  raw seed values
- `PixelVariationKey` is stable for the same pet identity and does not expose
  the raw seed value in public structs or artifacts
- Pixel code does not depend on `vm.pet_art`, `vm.pet_spans`, or
  `rerender_pet_for_view_model`

Integration and regression tests:

- existing `cargo test` suite stays green
- existing dev-preview watch/round/text artifacts are not unintentionally
  changed
- new pixel preview scenario writes expected manifest entries and frame artifacts
- macOS AppKit rendering remains a manual visual review gap, explicitly called
  out in the implementation plan
- hidden renderer CLI paths parse and forward the selected mode consistently
- Preview Lab pixel schema/viewer tests cover the new manifest fields and canvas
  links

Recommended gates:

```bash
cargo fmt --check
cargo test
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo clippy --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features --all-targets
```

The `cargo check --locked --no-default-features --all-targets` gate must run on
Ubuntu or another non-macOS environment before claiming Linux portability.

Manual macOS review checklist before Pixel can become default:

- build and launch the app bundle
- launch Pixel through `glorp companion --renderer pixel`
- launch Pixel through direct `open -n ... --args --renderer pixel`
- compare Classic/default and Pixel at default window size
- test minimum size, resized window, and fullscreen
- verify orientation, alpha, clipping, no stale frames after resize, and
  nearest-neighbor crispness for hard pixel layers
- capture screenshot or short video evidence for review

## Open Follow-ups

These are intentionally outside the first pixel companion implementation:

- flip Pixel to default
- remove Classic renderer
- Linux host adapter
- richer authored sprite catalog
- user-facing renderer settings
- full pixel habitat
- cross-platform window abstraction

## Acceptance Criteria

The first implementation is acceptable when:

1. `glorp companion --renderer pixel` opens the companion with a smooth animated
   pixel pet.
2. Classic companion still works and remains the default.
3. Pixel motion is visibly continuous, not cell-snapped.
4. Pixel pet has independent wander, breath/bob, blink, aura/shadow, calm/asleep
   behavior, and feed/activity pulse behavior.
5. The pixel renderer core compiles and tests on non-macOS targets because it
   has no AppKit dependency.
6. Pixel V1 includes the Fuzz S3 and Glitch S4 hero fixtures with side-by-side
   Classic vs Pixel preview review artifacts.
7. All species render non-empty, in-bounds Pixel frames through the shared pixel
   grammar, and live species/stage/mood/asleep/pulse identity changes are visible
   in sanitized input and controlled Pixel output.
8. Preview Lab includes deterministic pixel artifacts for review.
9. Existing watch and classic companion tests remain green.
10. CPU measurement and manual macOS review are recorded. Pixel remains opt-in
    unless the default-flip CPU budget passes.

## Risks

1. **V1 art may look too generic.** Keep the first slice small, but insist on at
   least lightweight species differences and a real visual review before default
   flip.
2. **AppKit image upload may be fiddly.** Keep the AppKit adapter thin; if direct
   image drawing becomes awkward, the portable frame boundary still allows a
   different host later.
3. **CPU cost may rise.** Start at 30 FPS, avoid reallocating large buffers on
   every frame where practical, and measure companion CPU before default flip.
4. **Two renderers can linger.** Treat Classic as temporary rollback. Add a
   follow-up cleanup plan once Pixel becomes default.
5. **Linux portability can rot if untested.** Keep pixel modules outside
   `cfg(target_os = "macos")` and include pure tests in normal `cargo test`.
