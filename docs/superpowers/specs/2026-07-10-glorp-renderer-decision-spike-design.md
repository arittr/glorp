# Glorp Renderer Decision Spikes - design

- Date: 2026-07-10
- Status: proposed decision program; written for review before spike planning or implementation
- Supersedes no shipping behavior
- Reclassifies as a research brief pending this program:
  - `docs/superpowers/specs/2026-07-10-glorp-retained-rust-renderer-design.md`
- Builds on:
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-companion-renderer-v2-design.md`
  - `docs/superpowers/specs/2026-07-09-glorp-companion-draw-boundary-hardening-design.md`
  - `docs/superpowers/specs/2026-07-10-glorp-retained-rust-renderer-design.md`

## Calibration

Glorp's optimized Smooth companion still spends substantial CPU while visibly idle.
Profiling indicates that full-view AppKit/Core Animation redraws and per-glyph native
text painting dominate the remaining steady cost. That establishes a real problem,
but it does not establish which replacement backend is appropriate.

The retained-renderer research brief proposes backend-neutral retained scene
contracts and identifies `wgpu`/Metal as the leading backend candidate. Adversarial
review found that the retained model is promising while the backend choice,
AppKit/Metal surface boundary, capture path, font source, accessibility bridge,
build topology, distribution cost, and Intel-macOS qualification remain unproven.
Those choices affect the architecture deeply enough that they must be decided with
executable evidence before a production renderer specification is approved.

This document specifies that evidence program. It is deliberately smaller than a
renderer implementation. The output is a decision, not a new product path.

## Problem

A paper comparison cannot answer the most important questions:

- whether a Metal-backed Rust renderer can live inside Glorp's existing AppKit
  lifecycle without fragile threading or callback behavior;
- whether removing per-glyph AppKit/CoreText work is sufficient without a GPU;
- whether retained native layers can meet the current workload at materially lower
  engineering and delivery cost;
- whether capture, resize, occlusion, device recovery, accessibility, and input
  remain reliable through a GPU-backed view;
- whether the selected font can reproduce Glorp's generated cell-art identity;
- whether a new graphics dependency fits Glorp's five-target npm release model;
- whether measured CPU savings are worth binary, build, memory, energy, and
  maintenance costs.

Building the complete retained scene runtime before answering these questions would
encode backend assumptions into IDs, resources, scheduling, capture, fallback, and
host ownership. Conversely, running unrelated toy demos would provide little
product evidence. The spikes therefore need one shared Glorp-shaped workload,
common measurements, bounded prototypes, and explicit decision rules.

## Goals

1. Select or reject a renderer backend using comparable executable evidence.
2. Prove or reject the AppKit host boundary required by a Metal-backed renderer.
3. Measure how much benefit comes from batching and atlas use without a GPU.
4. Decide whether a retained Core Animation experiment is necessary after the first
   two candidates are measured.
5. Resolve font licensing, Unicode coverage, glyph metrics, and visual-parity risk.
6. Prove that native keyboard, menu, pointer, and accessibility behavior can coexist
   with the selected surface model.
7. Prove a viable Cargo feature, target, package, and release topology.
8. Record raw, reproducible CPU, frame, energy, memory, build, and size evidence.
9. Produce a concise decision memo that constrains the later architecture spec.
10. Leave the workspace free of accidental production dependencies and abandoned
    prototype paths.

## Non-Goals

- No production retained scene graph.
- No default renderer change.
- No replacement or retirement of Smooth.
- No complete renderer parity with every Glorp state.
- No production shader/material framework.
- No meshes, lighting system, perspective camera, GPU particle system, bloom, water
  warp, physics, or general 3D engine.
- No permanent software reference renderer decision.
- No final device-loss fallback after Smooth retirement.
- No full pointer-interaction product design.
- No rewrite of AppKit window, menu, activation, fullscreen, or application
  lifecycle code.
- No Linux or Windows companion window implementation.
- No optimization contest based on visually different workloads.
- No acceptance based only on Activity Monitor screenshots or summary percentages.

## Decision Questions

The program must answer these questions before a production architecture spec:

1. **Backend:** Does `wgpu`/Metal materially outperform optimized Smooth and the
   software comparator while satisfying lifecycle, reliability, delivery, and
   near-term 2.5D needs?
2. **Host boundary:** Which thread owns the AppKit view, `CAMetalLayer`, surface
   creation/configuration, frame scheduling, and presentation?
3. **Capture:** Can review capture read back a deterministic bounded frame and exit
   cleanly without depending on `drawRect` or screen capture?
4. **Recovery:** Can initialization and surface/device failures become bounded,
   privacy-safe states without unwinding through Objective-C?
5. **Cheapest sufficient path:** Can a persistent Rust framebuffer and atlas meet
   the CPU and energy goals with one native submission per frame?
6. **Native retained ambiguity:** Is a retained `CALayer` design worth pursuing, or
   do the first two experiments make it clearly inferior or unnecessary?
7. **Font:** Which source font and atlas policy preserve Glorp's glyph repertoire,
   metrics, silhouette, licensing, and package constraints?
8. **Accessibility/input:** Can a rendered surface expose native semantic elements
   and preserve focus, keyboard commands, menus, pointer conversion, and fallback?
9. **Build/release:** How is experimental graphics code gated so all-features CI and
   five no-default-features publish targets remain valid?
10. **Intel macOS:** What evidence is required for `x86_64-apple-darwin` beyond
    cross-compilation?

## Locked Research Principles

1. **One workload, multiple candidates.** Candidate-specific demos are not valid
   comparisons.
2. **Smooth is the baseline.** Do not build another substantial immediate-mode
   AppKit candidate unless evidence identifies a specific untested optimization.
3. **Kill risk first.** Prove AppKit/Metal hosting and bounded capture before scene
   APIs or visual polish.
4. **Measure total cost.** CPU improvement cannot hide energy, memory, package,
   build, lifecycle, or accessibility regressions.
5. **Throwaway by default.** Prototype code is not production code merely because it
   works once.
6. **Reusable evidence infrastructure is allowed.** The fixture schema, benchmark
   runner, privacy scanner, and result parser may survive if they remain
   backend-neutral and tested.
7. **No backend winner by narrative.** The decision follows the gates in this
   document.
8. **No indefinite spike.** Every experiment has a timebox and stop conditions.
9. **No private user state.** Fixtures and artifacts are synthetic and privacy-safe.
10. **A failed experiment is a valid result.** Record it, clean up, and decide; do
    not quietly expand scope until it passes.

## Program Overview

```text
Phase A: shared fixture + benchmark harness + Smooth baseline
                         |
              +----------+----------+
              |                     |
              v                     v
Phase B: wgpu/AppKit          Phase E: parallel research
kill-risk spike              font / accessibility /
              |              features / Intel strategy
              v
Phase C: persistent Rust bitmap + glyph atlas comparator
              |
              v
Ambiguity gate ------------------------------------------------+
              |                                                |
       unambiguous                                      genuinely ambiguous
              |                                                |
              |                                                v
              |                                  Phase D: bounded retained
              |                                  CALayer experiment
              +------------------------+-----------------------+
                                       v
                              Phase F: decision memo
                                       |
                                       v
                         later architecture specification
```

The implementation order is A, B, C, optional D, then F. Phase E may run in
parallel once Phase A fixes the fixture and artifact contracts.

## Phase A: Shared Fixture And Benchmark Harness

### Purpose

Create one privacy-safe, Glorp-shaped workload and one measurement protocol so
candidate results are comparable.

### Fixture identity

The canonical fixture ID is:

```text
renderer-decision-companion-v1
```

It is synthetic and deterministic. It does not read real usage databases, helper
processes, configuration, pet state, project names, prompts, responses, paths, or
source identities.

### Required visual workload

The fixture contains enough current-scope work to expose the known bottlenecks:

- circular 360x360 and 720x720 logical-point targets;
- dark tank background with a simple stable gradient or dithered falloff;
- one generated-pet-like glyph body with exactly 180 visible glyph instances;
- exactly 80 static room/prop/tank-life glyph or sprite instances;
- exactly 40 simple shape instances covering rectangles, ellipses, arcs, and the
  round aperture/chrome mask;
- one wall shadow and one floor projection;
- one aura or translucent overlay;
- three perimeter gauges and three HUD strings represented as preselected glyphs;
- exactly three transparency/depth groups;
- one bounded dynamic-content group of 16 slots;
- exactly four transform groups: pet, foreground, midground, and background.

The fixture need not duplicate exact shipping art. It must preserve workload shape,
instance counts, clipping, transparency, and update patterns. Phase A freezes exact
primitive kinds, instance counts, glyph strings, colors, group membership, draw
order, bounds, and animation samples in `fixture.json`; later candidates may not
choose different work. Each candidate uses identical canonical data after adapter
conversion.

For backend comparison, Phase A also freezes one temporary, license-cleared atlas
image plus glyph rectangles and metrics. Every candidate uses those exact raster
inputs. The parallel font bake-off decides the production font policy; it must not
let different rasterizers or font files contaminate the backend comparison.

### Animation tracks

The harness defines exact deterministic tracks:

1. **Static:** one frame, no subsequent changes.
2. **Ambient:** 15 FPS for five minutes; pet bob/X/Z, parallax, aura, shadow, and
   gauges update from a fixed monotonic timeline.
3. **Active:** 30 FPS for 60 seconds; larger transforms and pulse parameters.
4. **Dynamic content:** 15 FPS for 60 seconds; the 16-slot group changes at a fixed
   4 Hz semantic cadence while transforms continue.
5. **Resize:** 360 -> 480 -> 720 -> 360 logical points with backing-scale
   reconfiguration at deterministic times.
6. **Occlusion:** visible 15 seconds, fully occluded 60 seconds, visible 15 seconds.
7. **Capture:** render at least five frames, request one readback/capture, write
   artifacts, and exit automatically.

Candidates do not substitute different frame rates or skip tracks to improve their
summary.

### Canonical fixture contracts

The shared evidence uses three deliberately separate contracts:

1. an immutable source fixture and timeline;
2. a fully resolved per-frame visual oracle;
3. expected assertions independent of candidate batching or retention.

The source contract should remain intentionally small:

```rust
pub struct DecisionSourceFixture {
    pub schema_version: u16,
    pub id: &'static str,
    pub viewport: DecisionViewport,
    pub primitives: Vec<DecisionSourcePrimitive>,
    pub tracks: Vec<DecisionTrack>,
}

pub struct DecisionResolvedFrame {
    pub frame_index: u64,
    pub elapsed_ms: u64,
    pub primitives: Vec<DecisionResolvedPrimitive>,
    pub changed_primitive_ids: Vec<DecisionPrimitiveId>,
}

pub struct DecisionExpectedFrame {
    pub frame_index: u64,
    pub required_primitive_ids: Vec<DecisionPrimitiveId>,
    pub expected_regions: Vec<DecisionExpectedRegion>,
    pub expected_changes: Vec<DecisionPrimitiveId>,
}
```

The resolved frame contains draw-ready glyph/sprite/shape primitives, bounds,
colors, opacity, transform, clip/depth order, and atlas references. It does not
prescribe groups, buffers, slots, dirty regions, layers, uniforms, damage strategy,
or retention granularity. Candidate adapters choose those independently.

These are benchmark DTOs, not production retained APIs. They must not settle node
hierarchy, resource generations, materials, camera, mesh, renderer module design,
or product state ownership. Tests prohibit production modules from importing the
spike DTO module.

### Smooth baseline adapter

The current optimized Smooth companion is the baseline. The harness may either:

- drive an isolated benchmark mode through the existing companion host; or
- use a bounded review mode that renders the canonical fixture through the current
  AppKit cell/shape painter.

The baseline must identify the exact binary, build profile, commit, and features.
It must not use a stale installed app bundle.

### Benchmark runner

Use a repository-owned command rather than a copied shell recipe. The eventual plan
may add an `xtask` command such as:

```bash
cargo xtask renderer-spike run --candidate smooth --track ambient
```

The exact CLI is implementation-plan work. The design contract is that it:

- builds or accepts an exact optimized binary;
- starts a bounded candidate run;
- records the candidate PID and lifecycle/capture event timestamps;
- samples CPU once per second;
- records process status and cleanup;
- invokes stack sampling when a budget fails;
- collects memory, energy, build, and package evidence using documented tools;
- writes one owned result directory;
- exits nonzero on missing artifacts, leaked processes, invalid JSON, or failed
  privacy scans.

### Phase A artifacts

```text
target/renderer-spikes/<run-id>/
  ownership.json
  run-manifest.json
  fixture.json
  environment.json
  binary.json
  host-boundary.json
  events.jsonl
  cpu-samples.jsonl
  frame-metrics.jsonl
  memory.json
  energy.json
  energy-raw.txt
  energy-parser.json
  captures/<track>-<logical-size>-<frame>.png
  captures/<track>-<logical-size>-<frame>.json
  stack-sample.txt
  dependency-tree.txt
  fault-results.json
  accessibility-tree.json
  accessibility-audit.md
  visual-review.json
  commands.jsonl
  privacy-scan.json
  process-cleanup.json
  summary.json
```

Build and distribution measurements add:

```text
  build-clean.json
  build-incremental.json
  artifact-sizes.json
  build-clean.log
  build-incremental.log
  package-smoke.json
```

`run-manifest.json` lists every required and optional artifact for that candidate
and track with schema version, byte count, and SHA-256. Missing required artifacts
fail the run. Raw tool output is preserved beside parsed JSON; parsers record their
own version/hash and units. A single generic capture cannot satisfy multiple size or
track reviews.

### Phase A exit gates

- Fixture JSON and frame generation are deterministic.
- Smooth renders every required track and exits automatically.
- Results include raw samples, not only aggregates.
- Capture is nonblank and has the requested physical dimensions.
- Privacy scan passes.
- No process survives a completed or failed run.
- Repeating the ambient baseline three times produces enough stability to report
  median and p95 without an unexplained large run-to-run divergence.

"Unexplained large divergence" means either run median differs from the three-run
median by more than 20%, or thermal/power/display state changes during the set. The
runner records the cause and repeats the set only after restoring the declared
environment; it does not discard an inconvenient run silently.

If the harness cannot produce a trustworthy Smooth baseline, no backend experiment
starts.

### Timebox

Two engineering days. If the fixture starts becoming a second scene architecture,
reduce it to flat canonical instance/group data.

## Common Measurement Protocol

### Environment

Every run records:

- git commit and dirty-state summary;
- binary path, SHA-256, size, and modification time;
- Cargo profile and feature set;
- candidate name and dependency versions/features;
- machine model and identifier;
- CPU/GPU family and memory;
- macOS version;
- display identifier, logical mode, refresh rate, and backing scale;
- AC/battery state;
- frontmost, visible, minimized, and occluded state;
- viewport logical and physical dimensions;
- start/end timestamps and monotonic duration.

### CPU

For five-minute ambient runs:

- warm up for 30 seconds;
- sample process CPU once per second;
- report every sample, mean, median, p95, and maximum;
- because the synthetic fixture performs no usage polls, no poll exclusion is
  necessary;
- use the macOS convention where one fully-used core is 100%;
- run at least three repetitions per candidate/configuration;
- after all candidates are runnable, execute the final comparison as matched blocks
  on the same day, rotating candidate order between blocks and allowing a fixed
  cooldown/settling interval; early sequential spike measurements are feasibility
  evidence, not the final ranking;
- preserve an eight-to-ten-second native stack sample for any candidate whose median
  exceeds 8% or whose p95 exceeds 12% at 360x360.

### Frame work

Record where supported:

- requested frames;
- encoded/drawn frames;
- presented/submitted frames;
- missed/deferred frames;
- frame CPU p50/p95/max;
- GPU time when reliable;
- draw calls;
- instance count;
- upload bytes;
- static rebuild count;
- atlas miss count;
- capture/readback duration;
- surface/layer reconfiguration count;
- fallback/error count.

The following are required for ranking every candidate: requested frames,
completed visible frames, submission/presentation count, end-to-end frame CPU time,
missed-deadline count, instance/primitive count, static rebuild count, atlas misses,
and submission/upload bytes. "End-to-end frame CPU" starts when the scheduler wakes
the candidate for a frame and ends after CPU submission/present work returns; a
candidate may additionally report encode, raster, AppKit submit, and GPU subspans.
A missed frame is one whose end-to-end CPU work or presentation misses its requested
cadence deadline; the denominator is requested visible frames. An unavailable
required metric fails the ranking run unless the decision program approves one
predeclared equivalent measurement for every candidate.

### Occlusion

After a ten-second settling period inside the 60-second occlusion track:

- renderer frame count must not increase;
- GPU submission or native bitmap submission count must not increase;
- semantic time may advance;
- reveal must present the correct current state without replaying every hidden frame.

### Memory

Record:

- physical footprint and RSS;
- peak physical footprint;
- renderer-attributable CPU buffers/caches;
- IOSurface and graphics allocations where observable;
- estimated GPU resources;
- post-warmup growth over the five-minute ambient track.

### Energy

Use one documented macOS energy method consistently across candidates on the same
machine and power state. Preserve raw output and the parser version. If no stable
absolute unit is available, report normalized candidate/Smooth ratios plus the
spread across matched repetitions. If uncertainty intervals overlap enough to
reverse the ranking, energy is marked inconclusive rather than rounded into a
winner. A result without raw evidence is descriptive only and cannot select a
winner.

### Build and distribution

Measure from documented clean and incremental states:

- clean optimized build wall time;
- incremental optimized rebuild after touching one candidate source file;
- stripped executable size;
- `Glorp.app` directory and compressed archive size for both Darwin targets where
  buildable;
- packed npm platform package size;
- bundled font/license and shader/resource bytes separately.

Exclude debug symbols, cargo registry/cache, `target/` intermediates, and Preview
Lab output. Include actual publish inputs.

The provisional hard limits used by this spike are self-contained here:

- no more than 15 MiB added to either stripped Darwin executable;
- no more than 20 MiB added to either compressed companion app artifact;
- no more than 20% added to the measured clean optimized build;
- no more than 25% added to the renderer-edit incremental optimized build.

All deltas use the same-commit Smooth baseline and the same cache/toolchain state.
A candidate exceeding a limit is rejected unless the decision review explicitly
approves a replacement budget from the raw measurements; the research brief is not
an implicit waiver.

### Visual comparison

Exact antialiased pixels are not required across candidate technologies. Required
checks are:

- requested dimensions and circular aperture;
- nonblank expected regions;
- stable canonical instance and group counts;
- required foreground/background/chrome presence;
- correct transform positions within a documented tolerance;
- expected dynamic-slot changes;
- side-by-side human review at 360 and 720.

A candidate may not lower workload counts or remove transparency, clipping, HUD, or
motion to pass performance gates.

## Phase B: `wgpu`/AppKit Kill-Risk Spike

### Purpose

Determine whether the leading GPU candidate can integrate safely and cheaply enough
for production architecture work. Do not build production retained contracts.

### Dependency policy

- Pin an exact `wgpu` crate version and lockfile entry.
- Use `default-features = false`.
- Initially enable only the features required for macOS Metal, WGSL, and standard
  Rust support.
- Place the optional dependency under macOS target dependencies and a non-default
  experimental Cargo feature such as `renderer-spike-wgpu`.
- Do not enable the feature in normal defaults or publish builds.
- Record the resolved dependency tree and duplicate heavy dependencies.

The implementation plan must verify feature names against the pinned version rather
than copying assumptions from this document.

### Host prototype

Use a dedicated experimental mode or binary path that reuses the existing AppKit
application/window lifecycle but isolates experimental rendering. The host must
prove:

- creation and lifetime ownership of a layer-backed `NSView` and `CAMetalLayer`;
- surface creation only after the native layer is valid;
- explicit logical-size, physical-size, and backing-scale propagation;
- resizing and screen/backing-property changes;
- main-thread ownership for AppKit and any Metal surface operations that require it;
- a documented handoff if encoding occurs off the main thread;
- deterministic frame wake/scheduling at 15 and 30 FPS;
- no scene traversal in `drawRect`;
- visibility/minimize/occlusion suspension;
- window close and automatic bounded review exit;
- fullscreen and existing keyboard/menu commands;
- Objective-C callback unwind guards.

The spike writes `host-boundary.json` and the decision memo must adopt or reject it.
It names the owning thread/executor and ordered calls for: AppKit view/layer
creation, surface creation, configure, acquire, encode, present, resize/backing
change, occlusion enter/exit, capture polling, close, and recovery. Runtime debug
assertions record the observed thread for each boundary. An unresolved or
occasionally violated owner is a failed host gate, not an implementation detail to
defer.

### GPU workload

The GPU candidate consumes the canonical fixture through a candidate adapter and
implements only:

- one glyph atlas texture with preloaded entries;
- instanced glyph quads;
- instanced solid/rounded shape quads or the smallest equivalent pipelines;
- group transforms/uniforms;
- alpha blending and the minimum depth/group ordering needed by the fixture;
- circular aperture masking;
- one surface render target;
- one offscreen/readback target for capture.

No production resource compiler, scene graph, generic materials, mesh system, or
shader abstraction is permitted.

### Capture/readback

The capture track must:

1. render a known frame into an offscreen or copyable texture;
2. copy to a correctly aligned staging buffer;
3. map asynchronously;
4. drive the required `wgpu` polling explicitly;
5. enforce a finite timeout;
6. write RGBA output with documented color/orientation conversion;
7. unmap and release capture resources;
8. write metadata and exit automatically.

Screen capture is not an acceptable substitute. A failed readback must terminate
with a static error category rather than hang.

### Fault and recovery harness

Inject or simulate:

- no compatible adapter/device;
- surface creation failure;
- zero-size/minimized configuration;
- outdated/lost surface acquisition;
- resize during capture;
- one device-loss episode or the closest deterministic injected equivalent;
- uncaptured validation/out-of-memory category handling.

The spike does not need a permanent production fallback. It must prove:

- maximum retry counts;
- no retry every frame;
- no Rust unwind through Objective-C or a `wgpu` callback;
- static sanitized error categories;
- bounded capture/run termination;
- transition back to the existing Smooth mode or a static safe frame where the
  isolated host permits it.

### Accessibility and input bridge

The GPU spike includes a minimum native overlay:

- one habitat/group accessibility element;
- three sanitized HUD value elements;
- no per-glyph accessibility children;
- correct bounds after resize and backing-scale change;
- keyboard focus and menu commands still work;
- one synthetic pointer event maps into fixture coordinates through the same
  presented transform snapshot;
- hide, fallback, and close remove stale semantic children.

This is a bridge proof, not the final interaction design.

Phase A freezes a backend-neutral semantic fixture and expected native tree: roles,
localized English test names, values, parentage, enabled/hidden state, focusability,
actions, bounds, and hit-test result. Every selectable host is audited against the
same tree during normal, resized, occluded/hidden, fallback, and teardown states.
Evidence includes machine-readable tree snapshots plus a documented Accessibility
Inspector or VoiceOver procedure for focus traversal, value reading, hit testing,
and stale-child removal. Screenshots alone do not pass this gate.

### `wgpu` spike gates

The candidate remains viable for the final comparison only if all feasibility gates
are true:

- all required tracks complete and clean up;
- surface lifecycle does not require replacing AppKit application/window ownership;
- capture is bounded, nonblank, and privacy-safe;
- fault injection produces bounded static outcomes with no callback unwind;
- accessibility/input audit passes;
- occlusion produces zero GPU submissions after settling;
- an optimized 360x360 Retina ambient run materially reduces renderer-attributable
  CPU versus same-block Smooth and does not reveal a structural regression; the
  later matched comparison, not this kill-risk phase alone, applies the absolute
  5%/8% selection budget;
- frame CPU p95 and missed frames are measured and do not show an obvious inability
  to sustain the requested cadence;
- no steady atlas misses or static uploads occur after warmup;
- energy is measured without a clear structural regression versus Smooth;
- renderer-attributable memory is bounded;
- executable/app/build changes remain inside the explicit provisional limits copied
  into this document below;
- all-features CI compilation and no-default-features release compilation remain
  viable across the current target matrix.

### Immediate stop conditions

Stop the experiment before its timebox expires if any is established conclusively:

- AppKit lifecycle must be replaced or substantially forked;
- required surface operations cannot be made thread-safe and deterministic within
  the existing host;
- bounded readback cannot be implemented;
- error/device-loss handling can unwind, hang, or require unbounded retry;
- native accessibility/focus cannot coexist with the surface model;
- minimal dependency configuration exceeds a hard build/size budget with no clear
  removable feature source;
- performance is not materially better than Smooth after obvious debug/validation
  overhead is removed from an optimized build.

For this stop condition, "materially better" means the candidate fails to improve
matched median process CPU by at least 25% and stack evidence shows no bounded
fixture/harness mistake likely to change that conclusion. This is a kill threshold,
not the final backend-selection rule.

### Timebox and checkpoints

Maximum 24 focused person-hours after Phase A:

1. **Hour 6 — host checkpoint:** layer/surface opens, clears, resizes, and exits;
   otherwise reject host viability.
2. **Hour 12 — capture checkpoint:** fixture subset presents and bounded readback
   writes a valid image; otherwise reject capture viability.
3. **Hour 18 — lifecycle checkpoint:** occlusion, close, callback guard, and minimum
   fault/accessibility cases produce required artifacts; otherwise reject or name
   one correction consuming the remaining budget.
4. **Hour 24 — feasibility verdict:** optimized fixture run, dependency/build/size
   evidence, cleanup, and pass/conditional-pass/reject are complete.

One correction is allowed for a demonstrated fixture/harness defect. There is no
automatic extension and no scene-architecture work inside this budget.

### Reusable code policy

Potentially reusable after explicit review:

- backend-neutral fixture adapter boundaries;
- benchmark runner additions;
- AppKit layer/surface lifetime proof if narrow and tested;
- bounded readback utility if independent of prototype scene assumptions;
- static error categories and fault harness.

Throw away by default:

- prototype shaders;
- fixture-specific buffers and pipelines;
- experimental CLI names;
- ad hoc threading;
- candidate-local metrics plumbing duplicated from the shared harness.

## Phase C: Persistent Rust Bitmap And Atlas Comparator

### Purpose

Measure whether removing native per-glyph and per-shape calls is sufficient without
GPU surface/device complexity.

### Required design

The comparator consumes the same fixture and uses:

- one persistent premultiplied RGBA framebuffer at the exact physical dimensions,
  backing scale, color format, and capture resolution used by the GPU candidate;
- one prebuilt glyph atlas;
- alpha blits for glyphs;
- bounded CPU rasterization for required shapes and aperture;
- dirty-group or dirty-region updates where justified;
- reused staging/native bitmap storage;
- one native image submission per visible frame;
- no per-frame `NSBitmapImageRep`, `NSImage`, attributed-string, font-resolution, or
  color-object creation;
- zero submission while occluded.

It must not reuse the current Pixel submission behavior unchanged, because that path
allocates and copies new native image objects per draw and would not test the actual
persistent-buffer hypothesis.

A lower-resolution/upscaled software mode may be measured as a separately named
product-quality experiment only after the primary equal-resolution comparison. It
cannot satisfy the primary backend gate unless a separate visual product decision
also requires every viable candidate to run that same reduced-quality mode.

### Raster scope

Implement only the canonical fixture. The comparator need not support arbitrary
paths, full text shaping, meshes, future lighting, or production visual parity. The comparator uses the exact temporary atlas image, rectangles, and metrics frozen
in Phase A. The production-font bake-off is separate from candidate performance.

### Comparator gates

Record the full common protocol. The comparator is a credible production candidate
for current-scope rendering only if:

- it passes the same visual, capture, occlusion, cleanup, privacy, and accessibility
  host checks applicable to a bitmap view;
- it completes optimized 360x360 and 720x720 matched runs without an obvious
  structural CPU/frame regression; final absolute and relative budgets are applied
  only after all viable candidates are measured in the same comparison blocks;
- energy is measured without a clear structural regression versus Smooth;
- memory stays bounded with no per-frame native image allocation growth;
- build and package costs are recorded; lower cost strengthens its final ranking but
  is not a feasibility requirement before the matched GPU comparison exists;
- a written capability analysis shows a bounded path for the first intended 2.5D
  feature without recreating an immediate-mode CPU bottleneck.

If the final matched comparison shows it materially slower than `wgpu` or failing
the 720/energy selection gates, retain it only as evidence; do not expand it into a
full renderer to seek parity.

### Timebox and checkpoints

Maximum 16 focused person-hours after the shared fixture exists: persistent buffer
and single submission by hour 6, complete fixture/capture/occlusion by hour 10, and
optimized matched-feasibility evidence plus cleanup by hour 16. One correction is
allowed for a demonstrated harness defect; otherwise issue the verdict.

### Reusable code policy

The framebuffer/atlas code is throwaway unless the decision memo selects it or a
later spec explicitly adopts a narrow reference path. The canonical glyph coverage
and visual-comparison evidence may be retained.

## Ambiguity Gate

Do not automatically run a third renderer experiment.

The result is **unambiguous** and Phase D is skipped when any of these is true:

- `wgpu` passes all mandatory gates and, in matched blocks, beats the software
  comparator by a practically meaningful margin whose uncertainty does not overlap
  zero: provisionally at least 2 percentage points of median process CPU or 25%
  normalized energy while remaining within build/package limits;
- the software comparator passes all current and capability gates, while `wgpu`
  fails lifecycle, reliability, accessibility, or hard delivery gates;
- both fail absolute performance/energy gates and profiling identifies shared work
  outside renderer submission that must be fixed before another backend comparison;
- one candidate is rejected by an immediate stop condition.

The result is **genuinely ambiguous** only when:

- both candidates miss or pass by small margins;
- the observed differences are smaller than run-to-run variation or their
  uncertainty intervals overlap;
- native composition/commit cost remains a leading unexplained hot path;
- evidence suggests retained native layer transforms could eliminate that cost with
  lower delivery complexity;
- the decision memo cannot distinguish candidates without measuring that specific
  hypothesis.

A written ambiguity note names the exact unanswered metric and authorizes Phase D.

## Phase D: Conditional Retained `CALayer` Experiment

### Purpose

Answer only whether native retained layers resolve the named ambiguity.

### Scope

- one static background/content layer;
- one pet/dynamic-content layer;
- one shadow/projection layer;
- one chrome layer;
- cached bitmap or layer contents built outside the frame callback;
- transform/opacity updates at 15/30 FPS;
- no general layer scene graph;
- no per-glyph AppKit drawing during ordinary ambient frames;
- same fixture tracks and common measurements.

### Decision value

The experiment must isolate Core Animation retained transform/composition behavior.
If it starts implementing a general resource, text, or scene system, stop: it no
longer answers the ambiguity cheaply.

### Timebox

Maximum eight focused person-hours, authorized only by the ambiguity note. The four
layers must present and animate by hour 4; the named ambiguity metric, capture, and
cleanup must be complete by hour 8 or the experiment is rejected as unable to alter
the ranking cheaply.

### Stop conditions

- static content still requires frequent rerasterization;
- layer count or content invalidation grows with glyph count;
- capture/review parity requires a second unrelated renderer;
- CPU/energy after the first valid retained version is not close enough to alter the
  candidate ranking;
- native layer ownership makes future required depth/content behavior clearly more
  complex than the selected alternative.

## Phase E: Parallel Research Tracks

### E1: Font And Unicode Bake-Off

#### Questions

- Which redistributable monospace font preserves Glorp's generated-art silhouette?
- Does its license permit source inclusion, binary distribution, atlas generation,
  and subsetting if used?
- Does it cover the current repertoire and replacement character?
- Do non-BMP and multi-scalar atlas keys work end to end?
- What are the source-font and generated-atlas size costs?

#### Work

1. Generate a deterministic required-glyph manifest from current pet, room, prop,
   tank-life, HUD, and Preview Lab fixtures.
2. Shortlist no more than three font candidates.
3. Check license text and required attribution.
4. Render the full species/stage/state matrix at 260, 360, 480, and 720.
5. Record advance, baseline, ascent/descent, weight, and pixel-snapping behavior.
6. Add one non-BMP scalar and one multi-scalar sequence prototype.
7. Measure source file, subset if legally/technically valid, and atlas sizes.
8. Produce side-by-side captures and a font decision recommendation.

#### Gates

- no missing required glyphs;
- replacement behavior is explicit;
- non-BMP/multi-scalar lookup and capture pass;
- license and attribution are approved;
- visual review accepts pet silhouette and HUD readability;
- package cost is recorded.

If no candidate passes, the decision memo may approve transitional native atlas
rasterization, but it must record host-font/version dependence and cannot claim final
font determinism.

### E2: Accessibility And Input Audit

Use the Phase B host when available, but keep the audit independent of GPU scene
architecture.

Prove:

- native window/menu/fullscreen keyboard behavior;
- sensible window/habitat roles and localized names;
- three sanitized HUD values;
- no per-glyph child explosion;
- focus behavior through resize, hide, fallback, and close;
- pointer conversion at 360 and 720 with Retina scaling;
- stale generation/frame pointer snapshots are ignored;
- privacy scan catches fixture secrets placed intentionally in rejected labels.

Deliver an audit checklist, screenshots or accessibility inspection evidence where
appropriate, and automated checks for semantic target projection.

### E3: Cargo Feature And Release Topology

Prove with the experimental dependency present:

- `cargo clippy --all-targets --all-features -- -D warnings`;
- `cargo test --locked` on Ubuntu, macOS, and Windows as CI currently requires;
- `cargo check --locked --no-default-features --all-targets`;
- optimized no-default-features builds for:
  - `aarch64-apple-darwin`;
  - `x86_64-apple-darwin`;
  - `x86_64-unknown-linux-gnu`;
  - `aarch64-unknown-linux-gnu`;
  - `x86_64-pc-windows-msvc`;
- published Linux/Windows binaries do not contain or link unused Metal/AppKit GPU
  code;
- `dev-preview` remains excluded from published binaries;
- Darwin app packaging accepts the experimental build when explicitly enabled.

The research also proposes the shipping topology that would apply if the candidate
wins: exact feature/default table by target, candidate-enabled optimized Darwin
build commands, app resource/license placement, dynamic-linkage audit, npm pack and
install/launch smoke, and signing/notarization policy impact. Both Darwin artifacts
must be assembled and inspected; inability to execute x86_64 is handled only by the
separate E4 disposition. Non-Darwin packed artifacts are byte-compared or
content-compared against the same-commit baseline to prove the target-specific
dependency did not leak.

Record the exact Cargo feature table and commands. The spike must not change release
default behavior.

### E4: Darwin x86_64 Qualification Decision

Choose one before the later architecture spec:

1. execute native surface/capture/fault smoke tests on dedicated Intel macOS
   hardware;
2. establish a documented external qualification process before each renderer
   release;
3. explicitly drop native retained-backend support for Intel macOS through a
   separate product/release decision while preserving other CLI functionality;
4. document a temporary exception with owner, expiry, and risk if hardware is not
   immediately available.

Cross-compilation and package inspection are necessary but not sufficient evidence
of Metal surface behavior.

## Privacy Contract

All fixtures are synthetic. Artifacts and logs must reject:

- source or display names;
- project identifiers;
- user paths;
- prompts, responses, transcripts, or tool payloads;
- raw diagnostics containing external data;
- real pet seeds;
- arbitrary shader source or backend validation text in published review artifacts.

Allowed external artifacts contain:

- static candidate and error categories;
- sanitized fixture/node/group IDs;
- numeric metrics;
- documented machine/build metadata;
- owned synthetic captures.

Every result directory includes `privacy-scan.json`. Fault paths receive the same
scan as success paths.

## Result Artifact Schemas

### `environment.json`

```json
{
  "schema_version": 1,
  "run_id": "...",
  "commit": "...",
  "dirty": true,
  "candidate": "smooth|wgpu|software|calayer",
  "track": "ambient",
  "machine": {},
  "display": {},
  "power": {},
  "viewport": {},
  "cargo": {}
}
```

### `binary.json`

Records binary path relative to the owned workspace where possible, SHA-256, bytes,
mtime, profile, target, Cargo features, and candidate dependency versions/features.

### `events.jsonl`

Allowlisted event kinds include:

- `run_start`;
- `warmup_end`;
- `frame_request`;
- `surface_reconfigure`;
- `occlusion_enter` / `occlusion_exit`;
- `capture_request` / `capture_complete` / `capture_error`;
- `fault_injected`;
- `fallback_enter`;
- `run_end`.

Events contain monotonic and wall timestamps but no arbitrary message strings.

### `frame-metrics.jsonl`

Each bounded sample may include frame index, requested/presented/submitted counts,
CPU encode time, GPU time where available, draw calls, instances, upload bytes,
static rebuilds, atlas misses, and sanitized status.

### `summary.json`

```json
{
  "schema_version": 1,
  "candidate": "wgpu",
  "configuration": {},
  "runs": 3,
  "cpu": {},
  "frames": {},
  "memory": {},
  "energy": {},
  "build": {},
  "sizes": {},
  "capture": {},
  "faults": {},
  "accessibility": {},
  "privacy": {},
  "cleanup": {},
  "gate_results": [],
  "verdict": "pass|conditional-pass|reject"
}
```

Summary values must be reproducible from raw artifacts. Manual observations are
identified separately from measured facts.

## Decision Rules

### Final matched comparison

After viable candidates and parallel research are complete, rerun Smooth and every
viable candidate in the matched, rotated blocks defined by the common protocol.
This is the only dataset used for final performance ranking. Feasibility runs may
reject an obviously failed candidate but may not select a winner.

The absolute selection budgets on the pinned primary machine are:

- 360x360 Retina ambient process CPU median at most 5% and p95 at most 8%;
- 360x360 frame CPU p95 at most 2 ms with missed frames below 1%;
- 720x720 ambient process CPU median at most 8% and frame CPU p95 at most 3 ms;
- zero submissions during the settled occlusion interval;
- zero atlas misses and zero static rebuild/uploads after warmup;
- energy no worse than Smooth; when more than one candidate passes, a normalized
  energy difference smaller than 10% or covered by measured uncertainty is treated
  as a tie;
- bounded memory with no unexplained post-warmup growth;
- the explicit build/distribution limits above.

If no candidate meets the absolute CPU budgets, the memo may recommend one bounded
follow-up only when a candidate improves matched Smooth median CPU by at least 60%,
passes every nonperformance mandatory gate, and profiling identifies one concrete
bounded cause separating it from the absolute target. That is a conditional pass,
not backend selection.

### Mandatory gates

A selected backend must pass:

- AppKit lifecycle compatibility;
- bounded capture/readback appropriate to the backend;
- resize/backing-scale behavior;
- occlusion suspension;
- callback and error-boundary safety;
- privacy scans;
- accessibility/input bridge;
- deterministic fixture consumption;
- bounded memory/resource behavior;
- actual release/feature compilation topology;
- approved font path;
- documented Darwin x86_64 disposition;
- hard package/build limits or an explicit reviewed replacement decision.

Failure of a mandatory gate rejects the candidate regardless of CPU.

### Performance ranking

Among candidates that pass mandatory gates:

1. Prefer a candidate meeting the absolute 360 and 720 CPU/frame/energy gates.
2. If more than one passes, compare normalized energy, median CPU, p95 CPU, memory,
   build/package cost, and implementation/lifecycle complexity.
3. Prefer the cheaper/lower-complexity candidate unless the first separately
   proposed renderer-native 2.5D feature exposes a concrete capability gap.
4. Do not select a GPU merely because it supports hypothetical future features.
5. Do not select software merely because it has fewer dependencies if it misses the
   performance/energy gates or makes the next approved feature structurally costly.

### Verdicts

- **Pass:** all mandatory gates pass and evidence supports selection.
- **Conditional pass:** no blocker exists, but one named bounded follow-up is needed
  before production planning. The follow-up has an owner, timebox, and exact gate.
- **Reject:** a mandatory gate fails, a stop condition is met, or measured benefit
  does not justify cost.

A conditional pass cannot hide multiple unresolved architecture questions.

## Decision Memo

The final deliverable is:

```text
docs/superpowers/measurements/2026-07-XX-glorp-renderer-decision.md
```

It contains:

1. exact commit and experiment dates;
2. fixture and harness version;
3. candidate implementations and deliberate omissions;
4. environment and repetition protocol;
5. raw artifact paths;
6. baseline and candidate tables;
7. capture/visual review results;
8. fault, accessibility, privacy, and cleanup results;
9. build/package and feature-matrix results;
10. font decision;
11. Darwin x86_64 disposition;
12. rejected alternatives and why;
13. selected backend or explicit no-selection;
14. bounded follow-ups;
15. constraints imposed on the later architecture spec.

The memo must distinguish measured fact, observed behavior, inference, and product
judgment.

## Handoff To The Architecture Specification

Until the decision memo is approved,
`2026-07-10-glorp-retained-rust-renderer-design.md` is a design hypothesis/research
brief. It may guide fixture coverage and identify desired end-state properties, but
it is not authority for production dependencies, modules, phases, or backend APIs.

After the memo:

- revise or replace the renderer architecture spec;
- name the selected backend and exact host/thread boundary;
- incorporate the chosen font/resource policy;
- define production retained contracts based on evidence, not the benchmark DTO;
- define capture, error, fallback, accessibility, build, and release behavior;
- remove options rejected by evidence;
- write implementation phases only after those decisions are stable.

If no candidate passes, the correct handoff is not to choose the least bad one. The
memo should identify the common bottleneck or missing evidence and specify one new
bounded research question.

## Cleanup And Repository Hygiene

Each spike plan must identify files it may create. At completion:

- remove abandoned prototype modes, shaders, dependencies, feature flags, scripts,
  and generated resources;
- retain only explicitly-approved backend-neutral harness code and evidence schemas;
- keep owned result artifacts under `target/` during iteration;
- move only curated decision evidence into `docs/superpowers/measurements/`;
- do not commit large raw traces or binaries unless the repository explicitly
  approves their size and purpose;
- stop all candidate processes and remove temporary app bundles/directories;
- restore normal release defaults;
- run `git diff --check` and inspect `git status`;
- verify no existing user or workspace files were deleted.

A prototype that passes but leaves accidental production coupling is not complete.

## Testing And Verification

### Harness tests

- deterministic fixture/checksum;
- exact animation samples at known elapsed times;
- result-schema serialization and validation;
- percentile/aggregate calculations from known samples;
- missing artifact and nonzero candidate exit detection;
- process cleanup detection;
- privacy scanner success and seeded-failure cases;
- capture dimension/orientation checks;
- gate calculation and verdict determinism.

### Candidate tests

- adapter preserves canonical counts and group identities;
- static track performs no dynamic updates after first frame;
- ambient track does not rebuild static fixture content;
- dynamic track changes only declared slots;
- resize reaches exact requested logical/physical dimensions;
- occlusion stops submissions;
- capture exits within timeout;
- injected faults emit allowlisted categories;
- callback guard catches seeded panic without crossing Objective-C;
- no atlas misses after activation.

### Required repository checks

At minimum after experimental changes:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo check --locked --no-default-features --all-targets
npm test
```

The spike plan adds target-specific optimized build/package commands and native
bounded runs. A failed existing test is part of the experiment result and must be
fixed or cause rejection; it is not waived because code is experimental.

## Acceptance Criteria

- One deterministic shared fixture drives Smooth, `wgpu`, and software candidates.
- Smooth baseline artifacts identify the exact optimized binary and environment.
- The `wgpu` spike proves or rejects surface, scheduling, capture, fault,
  accessibility, feature, and release assumptions within its timebox.
- The persistent software comparator measures the actual one-bitmap/atlas
  hypothesis without per-frame native image-object creation.
- Retained `CALayer` work occurs only after a written ambiguity gate.
- Font/Unicode, accessibility/input, Cargo/release, and Darwin x86_64 decisions have
  explicit evidence.
- All candidates run the same tracks and workload counts.
- Raw CPU, frame, memory, energy, build, size, capture, fault, privacy, and cleanup
  artifacts exist and validate.
- Mandatory gate failure cannot be overridden by performance alone.
- The decision memo selects a backend, records one bounded conditional follow-up, or
  explicitly selects none.
- Prototype leftovers are removed unless explicitly adopted.
- The later architecture spec is not approved before the decision memo.

## Risks And Mitigations

### Risk: the fixture becomes a premature scene API

Mitigation: keep it flat, fixture-specific, and explicitly non-production. Exclude
materials, mesh, generic hierarchy, and backend resource ownership.

### Risk: candidate adapters are unfair

Mitigation: canonical counts, transforms, colors, tracks, sizes, captures, and raw
metrics are shared and reviewable. Deliberate omissions are listed in the memo.

### Risk: prototype quality determines the winner

Mitigation: implement the smallest obvious optimized form, use release builds,
profile failures, permit one bounded correction for an identified mistake, and do
not repeatedly tune one candidate after measuring another.

### Risk: `wgpu` work grows into the production renderer

Mitigation: kill-risk scope, experimental feature, fixture-only shaders, strict
three-day timebox, and throwaway-by-default review.

### Risk: software looks sufficient only at 360

Mitigation: require Retina 720 evidence, energy measurement, and capability analysis
for the first intended 2.5D feature.

### Risk: Core Animation becomes an obligatory third implementation

Mitigation: run it only through the written ambiguity gate and stop after one day.

### Risk: energy data is noisy

Mitigation: same machine/power/display state, repeated runs, raw evidence, normalized
ratios, and no backend selection on energy alone when uncertainty exceeds the
candidate difference.

### Risk: font work blocks all renderer learning

Mitigation: use one explicitly temporary common atlas for early workload comparison
while the font bake-off runs in parallel; no final architecture approval until the
font decision is complete.

### Risk: no Intel macOS hardware is available

Mitigation: make the support exception explicit with owner and expiry rather than
pretending cross-compilation proves runtime behavior.

### Risk: successful spikes leave a dirty dependency graph

Mitigation: cleanup is an acceptance criterion; the decision memo records which
code/dependencies are retained and why.

## Open Planning Details

The implementation plan still needs to choose:

1. exact experimental command names and allowed files;
2. exact pinned `wgpu` version/features after checking current primary docs;
3. the macOS energy tool and parser;
4. whether raw large traces remain only in owned local artifacts or are attached to
   an external review record;
5. exact candidate process IPC/result signaling;
6. accessibility inspection tooling available on the benchmark machine;
7. font shortlist and license-review owner;
8. Intel macOS hardware or exception owner;
9. whether candidate work uses one temporary branch or sequential commits.

These are implementation-plan details, not reasons to reopen the spike scope.

## Recommended First Plan

Write one implementation plan titled **Renderer Decision Harness And `wgpu` Kill-Risk
Spike**. It should:

1. add the canonical fixture and deterministic tracks;
2. add result schemas, privacy scanning, cleanup checks, and the Smooth adapter;
3. record three optimized Smooth baseline repetitions;
4. add the target-specific experimental `wgpu` feature;
5. implement the AppKit/Metal host, minimal instancing, capture, and faults;
6. run the common protocol and write a provisional candidate report;
7. stop before the software comparator unless Phase B artifacts validate.

The software comparator should be a second plan using the frozen harness. This keeps
the first plan reviewable and makes a failed `wgpu` kill-risk result useful without
committing to the rest of the program.
