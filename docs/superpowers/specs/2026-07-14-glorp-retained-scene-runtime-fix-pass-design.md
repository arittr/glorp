# Glorp Retained 2.5D Scene Runtime Fix Pass — design

**Date:** 2026-07-14  
**Status:** Approved for implementation; product decisions resolved  
**Scope:** Native macOS round companion on Apple Silicon; direct retained scene runtime, host lifecycle, animation, evidence, and default cutover

## Summary

Glorp currently has two different things called “Retained”:

1. the shipping retained **backend**, which translates a fully resolved Smooth scene plan into GPU primitives; and
2. the new direct retained **scene runtime**, which projects one renderer-neutral `CompanionSceneSnapshot`, reconciles stable scene state, and renders it through the new 2.5D pipeline.

The first is already selected by `Auto` on Apple Silicon. The second is not. `AUTO_RETAINED_ON_APPLE_SILICON` is `true`, while `AUTO_SCENE_RUNTIME_ON_APPLE_SILICON` is `false`. Consequently:

- normal Apple-Silicon `Auto` runs the retained backend with the legacy Smooth-plan translator and the direct scene runtime off;
- explicit `--renderer retained` without another flag runs the legacy translator while shadowing the direct runtime; and
- only `--renderer retained --retained-scene-runtime live` presents the direct runtime.

That split is the main source of the confusing product state. The new renderer exists, but its live route is still rollout scaffolding rather than the retained renderer’s sole production route.

This fix pass completes and hardens the direct runtime before making it the default. It does **not** flip the default first and debug in production. The required order is:

1. restore truthful tests and evidence;
2. fix resize, backing-scale, worker-failure, and presentation-progress behavior;
3. establish explicit animation ownership and complete existing prop/tank animation parity;
4. qualify direct output with GPU-native capture and native lifecycle tests;
5. canary the direct runtime;
6. make direct retained the only Retained route and delete the legacy translator and rollout modes.

The end state is one direct retained scene path, with Smooth kept only as an explicit renderer and cold technical fallback.

## Evidence And Current-State Calibration

This document distinguishes observed behavior from code-derived risks.

### Observed in the current checkout

The following was operator-observed from a clean checkout at `7379755`. The
captured artifacts do not contain a source revision or executable digest, so
this is calibration evidence, not a cryptographically bound qualification run:

- `target/macos/Glorp.app/Contents/MacOS/glorp-companion companion-app --print-capabilities` reports:
  - requested renderer `auto`;
  - effective renderer `retained`;
  - retained compiled `true`;
  - Auto retained on Apple Silicon `true`.
- A bounded launch of the exact direct route ran for 8.008 seconds and exited zero:

  ```bash
  target/macos/Glorp.app/Contents/MacOS/glorp-companion \
    companion-app \
    --renderer retained \
    --retained-scene-runtime live \
    --review-duration-ms 8000 \
    --review-size 360x360 \
    --review-capture-dir target/glorp-fix-pass-live-smoke
  ```

- The run recorded 237 presentation frames, no callback panic, no frame-preparation error, and no fallback diagnostic. A crash or stall was **not** reproduced in that bounded run.
- The same Live review emitted only an AppKit `screenshot.png`. Its decoded 720×720 RGBA buffer was entirely `(0,0,0,0)`, and no direct-scene GPU artifact or pair manifest was written.
- Retained Off and Shadow runs also produced transparent AppKit screenshots, but their existing legacy paired-capture path produced valid, populated GPU-native `retained.png` artifacts. The Smooth control screenshot was also valid.
- Therefore, a transparent AppKit screenshot is expected to be unusable for a Metal-backed retained view. Direct scene offscreen render/readback primitives already exist in `retained::render`; the defect is that Live review does not wire the active scene through those primitives, presentation receipts, manifests, or process failure, and instead treats the unusable AppKit artifact as success.
- `cargo test --features retained-renderer --test retained_scene` passes 9/9, but those nine tests are source-structure assertions and complete in 0.00 seconds. They do not exercise a real scene worker, CAMetalLayer, resize, drawable acquisition, GPU presentation, or capture.
- Two required retained-feature boundary suites are currently red:
  - `retained_renderer_boundary`: 15 passed, 2 failed;
  - `companion_scene_boundary`: 16 passed, 1 failed.

The two retained boundary failures encode pre-Live source ordering/marker assumptions. The neutral-scene failure identifies a real dependency-boundary violation in test-only code under `presentation::companion_scene`.

### Code-derived high-risk paths

The following were not reproduced as native failures in the bounded run, but the current control flow permits them:

1. **Permanent blank/stall after resize plus worker failure.** Active presentation skips an old generation whose logical viewport no longer matches the resized surface. If replacement generation work fails while an old generation object still exists, Live only falls back when there is no active generation. The stale active generation can therefore remain present in state but permanently ineligible to draw.
2. **Stale atlas after backing-scale-only migration.** The host updates physical extent, backing scale, and surface configuration, but the production Live path does not connect a scale-only display change to scene `BackingScaleAtlas` invalidation. Moving between 1× and 2× displays can leave the direct scene using resources prepared for the old scale.
3. **Surface lifecycle has split authority.** The retained host advances surface state while the neutral runtime also models a surface epoch/rebind contract. The production resize path does not consistently join those authorities into one transaction.
4. **No presentation-progress watchdog.** A visible window can repeatedly skip without a bounded retry/fallback decision. Skip is a valid per-tick outcome, but not a valid indefinite visible steady state.
5. **Direct Live evidence publication is structurally absent.** `run_paired_capture` requires `last_good_frame`, which healthy direct Live intentionally does not build. It returns without invoking the existing direct offscreen readback primitives or producing a direct capture receipt/manifest, while the generic review lifecycle can still exit successfully.

### Incomplete prior program stages

The direct runtime implementation delivered the snapshot, reconciler, compiler, resource path, delta upload, blended depth ordering, generation activation, and opt-in Live route. The prior implementation plan’s completion stages remain materially unfinished:

- native scene artifact/review qualification;
- comprehensive lifetime, resize, visibility, scale, and fault gates;
- Auto scene canary and hold;
- deletion of Smooth-plan-to-GPU translation and rollout scaffolding;
- the renderer-native lit treasure-chest proof.

This fix pass absorbs the first four items. The lit chest remains a follow-on feature proof, not a reason to delay reliability fixes or ship an unqualified default.

## Product Decision

The desired final routing is:

| Request / target | Final effective route |
|---|---|
| Apple-Silicon `Auto` | direct retained scene runtime |
| Apple-Silicon explicit `Retained` | direct retained scene runtime |
| Explicit `Smooth` | Smooth/AppKit renderer |
| Intel Mac `Auto` | Smooth |
| Direct retained technical failure | cold Smooth fallback, acknowledged after paint |

`Retained` must cease to mean “possibly the direct renderer, possibly the Smooth translator.” After cutover it means exactly the direct scene runtime.

Temporary `Off`, `Shadow`, and `Live` modes may remain while this pass is being qualified. They are deletion-bound rollout scaffolding. After the canary hold:

- delete `SceneRuntimeRollout` and `AUTO_SCENE_RUNTIME_ON_APPLE_SILICON`;
- delete the hidden `--retained-scene-runtime` flag;
- delete the Smooth-plan-to-GPU translator and legacy retained shader/resources;
- keep `AUTO_RETAINED_ON_APPLE_SILICON` as the one-line **Auto-route** rollback from direct Retained to Smooth; explicit `--renderer retained` remains direct Retained;
- keep explicit Smooth and the cold Smooth fallback.

## Goals

1. Make the direct scene runtime reliable across launch, resize, fullscreen, backing-scale migration, occlusion, reveal, surface outcomes, worker failure, and shutdown.
2. Make every visible animated prop and tank inhabitant update on the direct path at its authored cadence, without moving props declared static.
3. Separate semantic animation, continuous presentation motion, and topology/resource work so a 30 Hz present loop does not rebuild semantic art or static scene data.
4. Give direct Live a GPU-native capture and evidence path that cannot report success for blank or absent output.
5. Replace brittle source-order assertions with behavioral lifecycle tests while preserving enforceable architecture boundaries.
6. Preserve real world depth, correct blend ordering, HUD/gauge chrome isolation, privacy, input, accessibility, and cold fallback.
7. Meet the frozen latency, hidden-work, resource, and memory gates, with a new baseline that names presentation cadence separately from semantic cadence.
8. Make the direct runtime the sole Retained route after a bounded native canary and hold.

## Non-Goals

- No general ECS, game engine, animation graph, scripting layer, or arbitrary mesh/material system.
- No perspective camera, PBR, shadow maps, bloom, or post-processing stack.
- No production bubble feature in this pass.
- No requirement for pixel equality with Smooth. Direct output requires semantic completeness and visual approval, not legacy pixel matching.
- No retirement of Smooth as an explicit renderer or cold technical fallback.
- No Intel retained cutover.
- No arbitrary animation added to props whose authored state is explicitly static.
- No default flip before native capture, lifecycle, fault, and soak gates pass.
- No claim that the reported crash/stall is reproduced until a renderer-owned failure is captured. Environment, state-store, LaunchServices, and Metal-unavailable failures must remain separately classified.

## Architectural Direction

### One production scene authority

The direct route keeps the architecture already established:

```text
privacy-projected domain state + clocks + logical layout
                         |
                         v
              CompanionSceneSnapshot
                         |
                         v
              CompanionSceneReconciler
                 /        |        \
          generation   content    frame
                 \        |        /
                         v
              CompanionSceneRuntime
                         |
                         v
       retained compiler / buffers / renderer
                         |
                         v
             CAMetalLayer presentation
```

`CompanionSceneSnapshot` remains the only world-scene semantic authority for the direct route. The renderer must not also consume `RoundSceneModel`, `SmoothCompanionScenePlan`, TUI draw cells, or independently derived prop state. The existing renderer-private HUD sidecar may carry sensitive text outside the serializable snapshot, but it must have an explicit revision/privacy identity and may not become a second authority for world content, gauges, dim state, activity, or other scene semantics.

Healthy direct ticks must not build `PreparedRendererFrame::Smooth` or update `last_good_frame`. Smooth state is built once, from the latest domain state, only after a technical fallback is requested.

### Separate clocks and invalidation domains

The current direct route projects a complete snapshot on every visible 30 Hz UI tick. The fix pass makes ownership explicit:

1. **Semantic clock — 4 Hz maximum sampling cadence**
   - Owns generated pet-art phase, expression/state changes, authored discrete prop sprite phase, twinkle state, chest open/closed state, tank sprite/morph state, day phase, and other semantic content.
   - May accept immediate event-driven updates from usage polling rather than waiting for the next 4 Hz tick.
   - Preserves current canonical wall-time boundaries for authored habitat and tank phases until a separately reviewed migration says otherwise; product meaning that does not require wall time uses monotonic anchors.
   - Advances at most once after a delay; it never replays a catch-up burst.

2. **Presentation clock — up to 30 Hz while visible**
   - Owns continuous pet drift, bob, depth cue interpolation, camera motion, prop sway/hover interpolation, tank position interpolation, ambient particles, opacity, and other frame-only values.
   - Uses monotonic elapsed time.
   - Produces bounded `FrameDelta` work only. It does not regenerate glyph art, topology, atlases, pipelines, or static batches.

3. **Topology/resource events — event driven**
   - Own visible cast, authored resource repertoire, logical layout generation, backing-scale atlas generation, and device/surface epochs.
   - Run only when their actual key changes.

4. **Hidden state**
   - Suspends semantic animation work, frame work, GPU writes, acquire, encode, submit, and present.
   - Coalesces the latest domain state and clocks without replaying history.
   - Reveal reconciles the newest state once, then resumes current-time motion.

The reconciler remains the sole allocator of semantic and frame revisions. As a target migration, the host becomes the sole allocator of typed device and surface epochs. Today the host increments a raw surface epoch while the runtime independently allocates successor `SurfaceEpoch` values; this split must be removed. Host rebind passes the exact typed epoch into the runtime, which validates strict monotonicity and never increments it independently.

For this pass, the current version shape supersedes the July 11 sketch: `SceneGenerationKey` contains device, layout, and resources, while `SceneVersion.surface` is separate. An operational surface rebind changes only `SceneVersion.surface`; layout, resource, or device invalidation replaces the generation. Changing that relationship requires an explicit design amendment.

The implementation must not satisfy this split by projecting and comparing the complete semantic snapshot at 30 Hz. Add a typed projection seam with one cached accepted semantic snapshot plus a bounded frame projection anchored to that snapshot's semantic revision. Semantic/event ticks may replace the authoritative snapshot; presentation ticks may update only `FrameSnapshot`-equivalent values through the reconciler. A frame update whose semantic base no longer matches is rejected and regenerated from the newest snapshot. This preserves one semantic authority without cloning pet art, prop/tank inventories, room content, or topology on ordinary presentation ticks.

While hidden, retain only the newest privacy-projected domain update or dirty marker and the current clock sample. Do not build a full `CompanionSceneSnapshot` every hidden timer tick merely to overwrite `hidden_latest`. Reveal projects/reconciles once from the newest retained input. If retaining an immutable projected snapshot is necessary for thread ownership, project only when the domain input changes, not at presentation cadence.

## Animation Contract

### Authored semantic state

The direct runtime already projects canonical habitat animation state from `game::habitat::habitat_prop_animation_state`. The following meanings must remain authoritative:

- sprite-phase props change only on their authored phase boundary;
- spark/lantern twinkle changes only on the authored twinkle window;
- pebble, shell, orbit, and lantern motion phase changes only on the authored motion boundary;
- treasure-chest lid state follows the canonical chest cycle;
- props with no authored animation fields are `Static`;
- tank routes, visibility, sprite variants, morphs, layers, and cadence come from canonical tank-life resolution.

No renderer-specific timer may duplicate or override those decisions.

### Continuous presentation

Discrete semantic phases alone produce visible stepping and make much of the habitat read as static. The direct runtime adds a closed, authored presentation-motion contract:

```rust
pub enum PropPresentationMotion {
    Static,
    TwoPoseEase { duration_ms: u16, curve: EaseCurve },
    Sway { amplitude_points: f32, period_ms: u32 },
    Hover { amplitude_points: f32, period_ms: u32 },
    TwinkleFade { attack_ms: u16, release_ms: u16 },
}
```

The exact type name may differ, but the contract must remain closed and companion-specific. Immutable motion policy lives in topology/template data, discrete glyph/pose selection remains semantic content, and resolved prop/tank transforms and opacities live in typed frame projection. It is not implemented through host branches or shader-name conditionals.

Rules:

- `Static` yields byte-stable transform/content output across time, resize aside.
- A semantic transition records a deterministic anchor containing source pose, target pose, semantic revision, and monotonic transition-start time. `duration_ms` and the closed easing curve resolve the frame pose from that anchor.
- Periodic interpolation is deterministic from a monotonic epoch plus stable semantic identity. Neither transition nor periodic motion accumulates floating-point integration state.
- Hidden time is sampled on reveal; missed frames are not replayed.
- Motion amplitudes are authored in logical points and scale correctly at 1×/2× backing scale.
- Prop motion remains within the scene’s frozen bounds and does not introduce collision/layout reruns.
- Tank interpolation follows its canonical route and cadence and preserves foreground/behind layer semantics.
- Pet art remains on the semantic clock while transform/bob/depth remain on the presentation clock.
- macOS Reduce Motion suppresses continuous drift, sway, hover, and easing while retaining required discrete semantic state, legibility, and lifecycle transitions.

### Animation acceptance matrix

For every production prop catalog entry and tank inhabitant, deterministic fixtures must record at least:

- authored animation kind;
- semantic cadence boundaries;
- frame transforms/opacities between boundaries;
- content/glyph checksum before and after a semantic transition;
- expected static or animated disposition;
- hidden/reveal result;
- 1× and 2× output.

A multi-frame GPU readback test must use per-element regions of interest:

- static-prop ROI and transform checksum remain unchanged;
- animated-prop ROI changes only as allowed by semantic and presentation contracts;
- unrelated props remain unchanged when one prop transitions;
- no ordinary animation frame requests a new generation, atlas, persistent GPU object, or static upload.

## Lifecycle And Progress Contract

### Launch and first presentation

- Host preparation and layer installation remain transactional.
- No retained layer is installed until required device/surface preparation succeeds.
- The first generation is compiled asynchronously while a deterministic launch background may be shown.
- A candidate becomes active only after successful acquire, encode, submit, immediate GPU-mailbox drain, and present-call milestone.
- The timer/display driver must run in the AppKit common run-loop modes, or an equivalent display-link/event mechanism, so menu tracking and live window resize do not suspend scene service indefinitely. The callback remains main-thread-affine and non-reentrant: if a prior tick is still executing, coalesce one newest pending tick rather than nesting or queueing a backlog.
- A bounded visible review must distinguish “started,” “candidate ready,” “first present,” and “steady presents.” Merely firing timer ticks is not evidence of rendering.

### Resize and fullscreen

Logical resize is one transaction:

1. read current logical bounds, physical extent, and backing scale;
2. update host surface configuration and advance `SurfaceEpoch` if its contract changed;
3. notify/rebind the scene runtime to that exact host epoch;
4. classify independently:
   - logical extent/layout change;
   - backing-scale resource change;
   - physical-only surface change;
5. coalesce one replacement request;
6. retain the old generation only while it is legal for the current surface;
7. present the replacement or enter a bounded retry/fallback path.

An incompatible old generation must never be treated as healthy solely because an `active` object exists.

Required outcomes:

- logical resize requests the needed layout generation;
- backing-scale-only change requests `BackingScaleAtlas` without inventing a logical topology change;
- physical extent and active generation viewport must agree before acquire;
- resize storms coalesce to the newest size/scale;
- entering and exiting fullscreen cannot leave the app in indefinite `Skipped` presentation;
- non-square window sizes remain supported and preserve the circular aperture contract.

### Worker failure during replacement

When a required replacement fails:

- define replacement identity as desired generation key, source revisions, host surface epoch, and backing scale;
- distinguish **present-compatible** (safe to draw now) from **current** (matches the desired replacement identity and semantic state);
- if the current active generation remains present-compatible, retain and continue presenting it while one retry is coalesced;
- if the current active generation is incompatible with the current surface/layout, retry once from the newest snapshot and epoch;
- one retry budget is keyed by replacement identity; stale/cancelled work does not consume it, and a genuinely superseding identity resets it;
- a second genuine failure requests cold Smooth fallback whether or not the old generation is still present-compatible;
- never keep an incompatible active generation as a reason to suppress fallback;
- record one sanitized failure category and transition, without log spam.

### Surface outcomes

| Outcome | Required action |
|---|---|
| `Outdated` | reconfigure once, skip this tick, retry later |
| `Timeout` | skip this tick; preserve candidate/active state |
| `Occluded` / hidden | suspend visible work; do not count as a stall |
| `Lost` | request technical fallback; no same-tick retry loop |
| validation/device loss/OOM | request technical fallback; invalidate device epoch |
| candidate encode failure with compatible active | destroy candidate, retain active, bounded retry |
| candidate encode failure without compatible active | technical fallback |

### Visible-progress watchdog

A per-tick skip is not itself an error. Indefinite visible non-progress is.

Track privacy-safe monotonic progress:

- last successful present time and `SceneVersion`;
- consecutive visible present attempts;
- consecutive skips by static category;
- pending worker request identity/phase;
- current surface/layout/resource epochs;
- whether an active generation is compatible with the current request.

The deadline starts when a required replacement becomes queued or when the last successful present becomes ineligible for the current surface, whichever occurs first. For a visible, non-occluded window:

- a required replacement must present within 60 eligible visible attempts or 2.0 monotonic visible seconds, whichever limit is exceeded first;
- a worker failure follows the retry/fallback rule immediately rather than consuming the whole watchdog;
- repeated transient surface skips beyond the bound request fallback with a distinct `presentation-stalled` category;
- hidden/occluded time pauses the watchdog;
- the watchdog never crashes or blocks the UI thread.

Only confirmed hidden/occluded intervals pause both limits. Timeout, Outdated, worker wait, menu tracking, and live resize remain eligible. The exact 60-attempt/2-second bound is provisional until the first native gate measures worst-case worker and activation latency. Any revision must be recorded before canary and may only become looser with evidence.

### Hide, reveal, shutdown

- Hidden steady state has zero snapshot preparation, mirror write, acquire, encode, submit, present, worker submission, or persistent allocation after one transition tick.
- Reveal applies the latest coalesced state once and must present within the visible-progress bound.
- Shutdown cancels or abandons worker work without waiting on the AppKit thread, touching a destroyed layer, or publishing a late candidate.
- Window close and app termination must not leave a scene worker or companion process alive.

An app-owned lifecycle object holds the timer/display driver, window delegate, and retained host. On `windowWillClose` and `applicationWillTerminate`, it invalidates the timer, takes `APP_STATE`, calls runtime shutdown, tombstones the run generation so late worker publications are rejected, detaches the retained layer, drops host/mailboxes, and terminates after the last companion window closes. Dropping the worker is nonblocking; completion after the tombstone may free owned CPU data but may not publish or touch AppKit/GPU state.

### Cold Smooth fallback

- Healthy direct ticks do not keep a Smooth frame warm.
- On fallback, build exactly one current Smooth frame from the latest domain state.
- Remove the retained layer, restore AppKit drawing, request display, and acknowledge fallback only after the Smooth frame actually paints.
- Capture and metrics must report requested renderer, effective renderer, failure category, `FallbackPending`, and `FallbackPainted` truthfully.
- A fallback is a degraded success for an interactive app but a failure for a direct-runtime qualification run unless the test explicitly injects and expects it.

## Rendering And Scene Fidelity

The direct route must preserve the companion’s complete semantic cast:

- tank/background and aperture;
- room glyphs and weather/day treatment;
- all pet species, stages, expressions, palettes, generated glyph roles, depth planes, facing, particles, wall shadow, and floor projection;
- full visible prop budget and authored front/behind placement;
- full tank inhabitant budget and route layers;
- ambient content;
- mood aura, activity, helper-trouble, sleep/calm, and dim states;
- perimeter gauges and privacy-projected HUD;
- transparent, additive, and multiply content in one correct world-depth order;
- screen chrome isolated from world depth and lighting.

Approval is semantic and visual, not pixel equality with Smooth. Required review sizes are 260, 360, 480, and 720 logical points, including non-square resize samples and 1×/2× scale.

The direct renderer must demonstrate:

- nonzero clip-space depth for world elements;
- correct opaque/cutout occlusion across semantic categories;
- correct back-to-front blended order across static and dynamic content;
- premultiplied linear-light blending and sRGB output/capture;
- transparent-edge correctness;
- stable aperture and chrome under resize;
- no missing glyphs/resources when prop or pet content changes phase.

The previously designed lit treasure chest remains a follow-on renderer-native feature proof. It should begin only after the direct route passes this fix pass’s default-readiness gate. Its absence must no longer be confused with incomplete reliability work.

## Capture And Evidence Contract

### GPU-native direct-scene capture

Direct Live must wire the existing scene offscreen renderer/readback primitives to the active production scene, normalize rows and transparent pixels, and write a canonical RGBA artifact. It may reuse device, queue, pipelines, resources, and active mirrors, but it must not reconstruct a legacy Smooth plan.

On-screen presentation records a `PresentedSceneVersion` receipt only after the present milestone. The receipt binds scene version, surface epoch/extent, renderer-private HUD revision, and privacy projection. Default external capture leases that receipt, not merely the runtime's latest logical active version. If newer compatible deltas exist, capture either defers until they present or captures the last-presented mirrors; it never labels an offscreen-only revision as the displayed frame.

The default capture renders from a non-mutating capture-safe frame copy. It redacts HUD text and neutralizes or quantizes exact gauge fractions, activity values, and dim amount so pixels cannot reveal exact usage omitted from serialized artifacts. Live-value capture remains explicit, is written to the existing sensitive review root, and carries a distinct privacy disposition.

A direct capture is successful only if:

- a valid presentation receipt and its exact `SceneVersion`/HUD/privacy identity are selected;
- requested and rendered logical/physical dimensions match;
- encode, submit, map, normalization, and write succeed;
- the RGBA buffer length is exact;
- the artifact is not all-zero RGBA;
- expected aperture/control regions contain nontransparent pixels;
- manifest version, privacy claim, surface metadata, and scene version agree.

An AppKit `cacheDisplay`/view screenshot of a Metal-backed retained view is diagnostic only. It must never satisfy retained evidence or make a qualification command exit zero.

### Review artifacts

A direct-runtime review emits at least:

```text
scene.png
scene-manifest.json
scene-snapshot.json
scene-version.json
scene-metrics.json
```

The manifest states:

- requested/effective renderer and whether fallback occurred;
- direct scene route, not only the broad `retained` label;
- logical/physical size and backing scale;
- generation key plus applied semantic/frame revisions;
- presented-scene receipt plus HUD revision/privacy projection;
- presented frame count and last-present age;
- capture checksum and nonblank validation;
- privacy disposition;
- failure/skip categories when present.

The current `render-log.json` fields derived from Smooth review samples must not silently report zeros for direct Live. Either populate renderer-neutral metrics or mark nonapplicable fields explicitly.

### Cross-renderer review

Paired Smooth/direct evidence is migration tooling, not production architecture. It should launch deterministic explicit Smooth and explicit direct-Retained reviews from the same fixture input and frozen clocks, then compare manifests and images offline. Healthy direct Live must not maintain a Smooth frame merely to make a pair.

After cutover, paired tooling may remain for regression review, but no retained production module may consume `SmoothCompanionScenePlan`, `SmoothCompanionLayer`, `SceneDrawList`, or `DrawCell`.

## Observability

Metrics must name distinct cadences and work domains:

- `presentation_cadence_target_hz`;
- `semantic_cadence_target_hz`;
- present attempts, presents, skips by category, and longest visible no-present interval;
- snapshot projections, semantic reconciles, frame reconciles, unchanged ticks;
- generation requests, coalesces, completions, failures, retries, stale drops, and activations;
- surface, layout, resource, semantic, and frame epochs/revisions;
- scale invalidations and atlas scale;
- content/frame dirty ranges and bytes;
- persistent GPU-object and byte high-water marks;
- fallback pending/painted transitions;
- direct capture attempts/successes/failures/nonblank validation;
- UI, projection, reconcile, generation-service, materialization, delta-write, encode, submit, and capture latency percentiles.

Current metrics that report `cadence_ms: 250` for a 30 Hz Live presentation loop are ambiguous. The fix must preserve 250 ms as the semantic cadence while reporting the presentation target separately.

All diagnostics remain bounded and privacy-safe: static categories and fixture aliases only; no project names, paths, prompts, responses, raw seeds, source names, or exact usage values.

## Verification Strategy

### Repair the existing gate first

Before product behavior changes:

1. Make `retained_renderer_boundary` and `companion_scene_boundary` green.
2. Replace stale source-order/marker assertions with runtime or typed-state tests where behavior matters.
3. Remove the test-only Smooth dependency from the renderer-neutral scene tree rather than weakening the boundary scan.
4. Keep source scans only for real forbidden dependency/deletion boundaries.

A passing zero-duration source scan is not lifecycle evidence and must not be described as such.

### Pure and deterministic tests

Cover:

- projection and stable semantic IDs;
- semantic vs presentation clock behavior;
- no catch-up burst;
- prop static/animated classification and authored phase boundaries;
- deterministic continuous motion and hidden-time sampling;
- Reduce Motion projection and dynamic setting changes;
- invalidation masks for content, frame, logical layout, scale resources, surface, and device;
- independent host surface epoch and runtime rebind transaction;
- revision monotonicity, stale result rejection, and latest-request coalescing;
- dirty-slot/range calculation;
- incompatible-active classification;
- progress watchdog pause/resume and terminal action;
- capture version binding and nonblank validation;
- presentation-receipt/HUD/privacy binding and capture-safe gauge/activity/dim projection.

### Host lifecycle harness

Use an injected clock, surface outcome source, and scene worker. Exercise:

- first launch and first present;
- resize while worker is preparing, ready, and activating;
- resize storms;
- fullscreen-size enter/exit sequence;
- backing-scale-only 1×→2×→1× migration;
- worker failure with compatible active;
- worker failure with incompatible active;
- Outdated, Timeout, Occluded, Lost, validation, device loss, and OOM;
- hidden topology/content change and reveal;
- capture before, during, and after generation swap;
- shutdown during worker and activation work;
- fallback pending until a real Smooth paint;
- menu-tracking and live-resize run-loop modes longer than the watchdog bound;
- a deliberately slow tick, proving no nested callback, no `APP_STATE` reborrow, and at most one newest pending tick;
- window close and Cmd-Q, proving timer invalidation, late-publication rejection, and bounded PID exit.

Every test asserts active compatibility, exact epoch/version, transition count, bounded present recovery, resource counts, and disposition.

### GPU output tests

On a Metal-capable host:

- render the full state/species/stage/depth/size/scale matrix;
- capture the direct scene through its GPU-native path;
- reject blank/transparent artifacts;
- test static and animated prop ROIs across virtual timestamps;
- test tank motion and layer crossings;
- test depth, blended crossings, transparent edges, color, aperture, HUD, and gauges;
- test capture around a generation swap;
- test resize and scale output, not only source ordering;
- test default capture pixels for absence of exact HUD/gauge/activity/dim values, plus separately rooted explicit live-value capture.

Surfaceless tests may run where Metal is available. A missing adapter is infrastructure-unavailable, not a product pass or product failure, and the release gate must run on a logged-in Apple-Silicon GUI host.

### Native GUI and soak gates

Required before default cutover:

1. **Bounded exact-path smoke** — launch the same executable and direct route the user runs; require first present, nonblank direct GPU capture, no fallback/crash, exit zero, and no remaining PID.
2. **Resize/fullscreen soak** — repeated windowed sizes, non-square sizes, fullscreen enter/exit, and scale changes; require recovery within the progress bound.
3. **Surface/fault soak** — deterministic transient and fatal outcomes; require the exact retry/fallback contract.
4. **Hide/reveal soak** — verify zero hidden work and bounded latest-state reveal.
5. **Five-minute release run** — ordinary live work, stable memory/resources, captures, and no unexplained fallback.
6. **Four-hour release-candidate hold** — Auto direct route, ordinary work plus lifecycle transitions, zero unresolved stalls/crashes/resource growth, and successful captures.

Native accessibility/input acceptance covers role, label, value, and bounds before and after retained-layer installation, resize, and Smooth fallback; Cmd-Q, Control-Command-F, traffic-light controls, and body dragging; and dynamic macOS Reduce Motion changes.

Crash reports from the process must be collected by the harness when present and classified by the highest layer reached: launch/state, AppKit registration, retained host initialization, worker, materialization, acquire, encode, submit, present, capture, or fallback. No crash is attributed to the renderer without that evidence.

## Performance And Resource Gates

The existing baseline remains the starting contract, not proof that the direct route passes it. Preserve at least:

- UI tick p95 ≤ 1,422 µs and p99 ≤ 2,070 µs at the qualified baseline profile;
- encode p95 ≤ 282 µs;
- generation-service UI max ≤ 4,000 µs;
- materialize/upload/publish max ≤ 16,000 µs;
- activation render-owner p95 and max ≤ 16,000 µs;
- zero main-thread raster calls;
- zero hidden steady-state work after one transition tick;
- zero post-warmup persistent GPU-object creation, after prewarming and accounting direct offscreen target/readback resources;
- zero post-warmup static upload bytes on ordinary frames;
- 4,500 semantic samples over 1,125,000 ms plus the corresponding 33,750 presentation ticks at 30 Hz, with no capacity growth or stale mutation;
- final/peak RSS and accounted GPU bytes no more than 1% above warmup high-water under the frozen protocol;
- exactly one successful terminal direct GPU capture in the lifetime gate.

Because direct Live presents at 30 Hz rather than the baseline’s 4 Hz, comparisons must report both per-tick cost and per-second work/energy. A fast tick that increases total wakeups, writes, or energy materially is not automatically a pass. The direct offscreen target and readback cache are prewarmed before the warmup high-water mark, and terminal capture must reuse them. New numeric direct-route gates are frozen before canary using the same hardware/build/protocol discipline as the existing baseline.

## Rollout Plan

### Gate A — Truth and test repair

- Fix the three currently failing retained-feature boundary tests at their root.
- Add direct GPU capture and make blank/absent retained evidence fail.
- Bind direct capture to presentation receipts and capture-safe privacy projection.
- Correct renderer-specific metrics and cadence labels.
- Add exact route identity to manifests/capabilities.

**Exit:** all required tests are green, and a direct review cannot succeed without a nonblank GPU artifact.

### Gate B — Lifecycle and animation completion

- Join host surface epochs to runtime rebind/invalidation.
- Fix logical resize, backing-scale migration, incompatible-active failure, retry/fallback, and watchdog behavior.
- Implement the semantic/presentation animation split.
- Complete the prop/tank animation matrix and ROI tests.

**Exit:** deterministic and host-harness tests cover every transition, and ordinary animation creates no generation/resource churn.

### Gate C — Native qualification

- Run direct scene artifact matrix on Metal.
- Complete exact-path, resize/fullscreen, scale, surface/fault, hide/reveal, capture-swap, five-minute, accessibility/input, and package checks.
- Update `scripts/test/macos-app-packaging.test.mjs` and `.github/workflows/publish.yml` to require staged arm64 `effective-scene-route=direct` capability output and a nonblank exact-path direct capture.
- Receive visual approval for the full semantic cast.
- Freeze direct-route performance/energy gates.

**Exit:** no unresolved crash, stall, blank evidence, missing animation, semantic omission, or gate regression.

### Gate D — Auto canary

- With explicit approval, change Auto Retained to use direct Live on Apple Silicon.
- Keep the legacy translator available only as a one-line temporary rollback during the canary.
- Rehearse rollback before the hold.
- Run the four-hour release-candidate hold and record exact commit, machine, commands, artifacts, metrics, and outcome.

**Exit:** canary record has no unresolved blocker and rollback is proven.

### Gate E — Consolidation

- Delete `SceneRuntimeRollout`, its CLI flag, and the separate auto-scene constant.
- Delete legacy retained Smooth-plan translation, shader, resources, capture coupling, and parity-only production code.
- Make Retained stop claiming it “uses Smooth scene.”
- Keep explicit Smooth, cold fallback, and renderer-neutral paired review tooling.
- Require package/capability proof that the rollout CLI option is absent after consolidation.
- Add forbidden-reference tests across the complete retained module tree and paired-review production code, not only `retained.rs`.

**Exit:** exactly one Retained scene-generation path exists.

## Acceptance Criteria

### Product and routing

- Apple-Silicon Auto and explicit Retained present the direct scene runtime.
- No hidden flag is required to access the shipping Retained renderer.
- Explicit Smooth remains selectable; Intel Auto remains Smooth.
- Technical fallback cold-builds Smooth once and is acknowledged only after paint.
- There is no production Smooth-plan-to-GPU retained translator or Off/Shadow/Live routing after consolidation.

### Reliability

- No crash, hang, or indefinite visible skip occurs in launch, resize, fullscreen, scale migration, hide/reveal, surface/fault, capture, or shutdown gates.
- A required replacement presents within the frozen progress bound or reaches acknowledged fallback.
- Worker failure with an incompatible active generation cannot stall indefinitely.
- Scale-only migration rebuilds scale-dependent resources exactly once and does not rebuild semantic topology.
- Hidden steady state performs zero prohibited work.
- Shutdown leaves no worker/process behind.

### Animation

- Every catalog prop and tank inhabitant has an explicit static/animated contract.
- Static props remain stable; animated props visibly change at authored cadences.
- Continuous motion is frame-only, deterministic, bounds-safe, and smooth at presentation cadence.
- Pet art remains on semantic cadence; pet/tank/prop transforms may update at presentation cadence.
- Hidden time does not replay an animation backlog.
- Ordinary X/Y/opacity animation performs no generation build, atlas rebuild, static upload, persistent allocation, or blended-order sort. Bounded fixed-capacity blended-order recomputation is permitted and counted only when camera/world-Z ordering changes; it performs no allocation or topology rebuild.

### Rendering and fidelity

- The direct path renders the complete semantic cast at required states, sizes, depths, and scales.
- World depth, opaque occlusion, blended ordering, alpha/color, aperture, chrome, HUD, and gauges pass deterministic and native review.
- No required glyph/resource disappears during phase, state, size, or scale changes.
- Visual review is approved without requiring Smooth pixel identity.

### Evidence and observability

- Direct reviews produce a GPU-native, nonblank artifact bound to one presented-scene/HUD/privacy receipt.
- A transparent AppKit retained screenshot cannot satisfy the gate.
- Metrics distinguish semantic and presentation cadence and expose visible progress, lifecycle epochs, revisions, retries, failures, resource churn, and capture truthfully.
- Artifacts and diagnostics preserve the privacy contract.
- Crash/stall claims cite captured failure-layer evidence rather than inference.

### Tests and performance

- All repository tests, retained-feature boundary tests, Metal output tests, packaging checks, and lint/format checks pass.
- The 4,500-frame lifetime gate, five-minute native gate, fault/resize/visibility soaks, and four-hour canary hold pass.
- Frozen direct-route latency, per-second work, energy, memory, resource, hidden-work, and capture gates pass.
- Source scans remain only for dependency/deletion invariants; lifecycle claims are backed by behavioral tests.

## Resolved Product Decisions

The following decisions were approved on 2026-07-14:

1. **Presentation cadence:** retain fixed 30 Hz for this fix pass, measure per-second energy, and consider adaptive cadence only as a separate measured optimization.
2. **Prop motion art direction:** preserve canonical semantic phases and add subtle deterministic interpolation only where it makes the authored change legible. Review the complete catalog rather than applying one generic motion.
3. **Resize continuity:** allow one bounded direct retry over the neutral retained background when no compatible generation exists, then enter Smooth fallback.
4. **Canary duration and approval:** require the four-hour native hold plus one bounded release canary. Delete the translator only after the canary record is explicitly approved.
5. **Lit chest scheduling:** defer the lit chest until the direct renderer is default and stable, then use it as the first scene-native feature proof.

## Recommended Implementation Boundary

The implementation plan should be organized by Gates A–E with failing-first tests and small commits. It must name exact source files, injected seams, native commands, artifact paths, numeric gates, rollback points, and deletion checks.

Do not combine the default flip with lifecycle fixes, animation changes, or translator deletion in one commit. Do not weaken a failing architecture boundary to make the suite green. Do not accept an AppKit screenshot as retained evidence. Do not delete Smooth fallback. Do not claim completion until the direct route is the sole Retained route and the canary record is approved.
