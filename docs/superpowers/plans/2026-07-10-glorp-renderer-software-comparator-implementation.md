# Glorp Persistent Software Renderer Comparator Implementation Plan

> **For agentic workers:** Execute this plan task-by-task. Stop at every stated gate. This plan covers only Phase C of the renderer decision-spike design: a persistent Rust RGBA framebuffer plus atlas comparator, its evidence, and the backend ambiguity decision. It must not implement a production renderer, tune the frozen `wgpu` candidate, or begin a retained `CALayer` experiment without a later written ambiguity authorization.

**Goal:** Determine whether eliminating native per-glyph/per-shape drawing is sufficient to meet Glorp's renderer budgets using a persistent software framebuffer and one native image submission per visible frame, without GPU surface/device complexity.

**Architecture:** Add one benchmark-only `software` candidate to the existing feature-gated `renderer_spike` harness. The candidate consumes the frozen canonical fixture and atlas, rasterizes into persistent premultiplied RGBA storage at the exact physical dimensions, and exposes that same storage through persistent native bitmap/image objects. The AppKit window, timer, activation, visibility, accessibility, input, artifact, privacy, and cleanup contracts remain owned by the spike harness. The comparator may optimize its own retained pixel resources, but it must not change the source fixture, visual oracle, cadence, physical resolution, or candidate-neutral measurement rules.

**Tech stack:** Rust, existing `renderer-spike` feature, existing `objc2`/AppKit bindings, existing serde/manifest/privacy harness, existing xtask bounded runner, and repository-owned measurement scripts under `target/renderer-spikes/`. No new rendering dependency is expected.

## What “16 Hours” Means

The Phase C specification's 16 focused person-hours is a **maximum implementation and evidence timebox**, not a required profiling duration.

- Do not spend 16 hours sampling CPU.
- Implement and verify as quickly as correctness permits.
- Stop early when the evidence is conclusive.
- The checkpoint clock covers coding, debugging, functional evidence, and measurement preparation.
- Runtime measurements use the explicit durations in this plan: short smoke runs, bounded feasibility runs, and the common five-minute matched protocol only when the candidate remains viable.

## Entry State

Phase A and Phase B already provide:

- deterministic `renderer-decision-companion-v1` fixture data;
- exact 300-primitive workload and expected frame assertions;
- generated 32x32 temporary atlas and semantic fixture;
- typed artifacts, manifests, SHA-256 identity, privacy scan, and validation;
- hidden `renderer-spike-app` command;
- bounded xtask run/validate workflow;
- optimized synthetic Smooth baseline;
- AppKit activation correction for unbundled benchmark processes;
- frozen `wgpu` functional and corrected matched evidence;
- a Phase B conditional-pass memo at
  `docs/superpowers/measurements/2026-07-10-glorp-renderer-wgpu-kill-risk.md`.

The corrected `wgpu` matched binary and evidence are immutable inputs to Phase C. Do not overwrite them:

- binary: `target/renderer-spikes/bin/glorp-wgpu-spike-activated`
- SHA-256: `932d8719764b3015f37c8d1f18987dc6366ddd9be009bbe50372072ac97fe63c`
- corrected 360 evidence: `target/renderer-spikes/wgpu-matched-ambient-30s-activated/`

The final common comparison must rebuild or freeze exact same-commit binaries for every candidate and rerun all viable candidates in matched blocks. Existing wgpu results are feasibility evidence, not a substitute for that final matched set.

## Global Constraints

- Preserve all current shipping/default behavior.
- Keep `dev-preview` independent from renderer-spike features.
- Keep the comparator under the existing non-default `renderer-spike` feature unless implementation proves a separate feature is necessary. Do not add a new Cargo feature merely for naming symmetry.
- Do not add a production companion renderer enum variant.
- Do not import spike DTOs from production companion, presentation, round, pet, TUI, game, or storage modules.
- Do not read real state, usage, helpers, config, prompts, paths, project names, source identities, or pet seeds.
- Use the exact canonical fixture, atlas pixels, semantic fixture, cadence, logical dimensions, physical dimensions, aperture, and capture orientation used by Smooth and wgpu.
- Use equal-resolution rendering for primary evidence: 360 logical at 2x and 720 logical at 2x where the host reports that backing scale.
- Do not use the current Pixel renderer's per-draw native image construction unchanged. That would not test the persistent-buffer hypothesis.
- Do not create `NSBitmapImageRep`, `NSImage`, font, attributed string, `NSColor`, or equivalent native raster resources once per frame.
- No Rust unwind may cross Objective-C callbacks.
- Explicitly activate the unbundled AppKit application after ordering the window front. A visible benchmark run is invalid if surface/native presentation stops because the app was never activated.
- Do not change the fixture to improve dirty-region locality. The current resolver applies authored transform motion to all primitives; only 16 primitive atlas selections change at 4 Hz. Metrics must distinguish transformed/rerasterized primitives from atlas-content changes.
- Dirty regions are optional and evidence-driven. First prove the correct persistent full-frame path; add dirty-region work only if profiling shows raster cost threatens the gate and the update remains exact.
- Keep iterative scripts, binaries, captures, raw samples, and traces under `target/renderer-spikes/`. Do not commit large raw evidence.
- Do not tune, refactor, or repair the wgpu steady-instance-upload behavior during Phase C.
- Do not start Phase D retained `CALayer` work unless a written ambiguity note names the exact unresolved metric after Phase C evidence.
- Do not modify release defaults or publish workflow behavior.
- Do not use destructive git commands or revert unrelated existing work.

## Allowed File Set

### New files

- `docs/superpowers/plans/2026-07-10-glorp-renderer-software-comparator-implementation.md`
- `src/renderer_spike/software.rs`
- `tests/renderer_spike_software.rs` if comparator tests would make `tests/renderer_spike.rs` unwieldy
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-software-comparator.md`
- repository-owned measurement helpers under `target/renderer-spikes/`

A split such as `src/renderer_spike/software/{mod.rs,raster.rs,macos.rs}` is allowed only if the single file becomes difficult to review. Do not create a general renderer hierarchy.

### Existing files that may be modified

- `src/renderer_spike/mod.rs`
- `src/renderer_spike/fixture.rs` only for candidate-neutral helpers or assertions
- `src/renderer_spike/artifacts.rs`
- `src/renderer_spike/privacy.rs` only when new artifact types require it
- `src/renderer_spike/macos.rs` only for narrow candidate-neutral AppKit harness extraction
- `src/cli.rs`
- `xtask/src/lib.rs`
- `tests/renderer_spike.rs`
- `tests/renderer_spike_boundary.rs`
- `Cargo.toml` / `Cargo.lock` only if an unavoidable dependency or feature change is justified before editing
- `AGENTS.md` only after a proven workflow needs durable contributor documentation

Do not modify production renderer modules for Phase C. A required change outside this set stops implementation for review.

## Candidate And Command Contract

Extend the existing hidden command candidate enum:

```text
smooth | wgpu | software
```

The command remains:

```text
glorp renderer-spike-app \
  --candidate smooth|wgpu|software \
  --track static|ambient|active|dynamic|resize|occlusion|capture \
  --logical-size 360|720 \
  --duration-ms N \
  --out target/renderer-spikes/<run-id> \
  [--inject-fault callback-panic|capture-timeout|surface-unavailable]
```

For the software candidate, `surface-unavailable` means failure to establish or expose the persistent native bitmap submission resource. Preserve the allowlisted fault name so comparison tooling remains candidate-neutral; record a software-specific static category in fault artifacts.

Extend xtask without adding a second comparator-specific runner:

```text
cargo xtask renderer-spike run \
  --candidate software \
  --track capture \
  --size 360 \
  --duration-ms 2000 \
  --out target/renderer-spikes/software-smoke-capture-360

cargo xtask renderer-spike validate \
  --out target/renderer-spikes/software-smoke-capture-360
```

The software candidate builds with `--features renderer-spike`; it must not pull `renderer-spike-wgpu` or wgpu dependencies.

## Exact Software Storage Contract

The implementation must establish and test these identities:

1. **Rust framebuffer**
   - persistent `Vec<u8>` or boxed byte storage;
   - exact `physical_width * physical_height * 4` length;
   - premultiplied RGBA8 or a precisely documented AppKit-compatible byte order;
   - top-left logical raster convention with one explicit conversion at the native boundary if AppKit requires bottom-origin presentation;
   - allocation occurs at startup and only on an actual size/backing-scale change.

2. **Persistent native presentation resource**
   - one `NSBitmapImageRep` or equivalent native wrapper whose backing points to, or is updated from, stable owned storage;
   - one persistent `NSImage` only if AppKit submission requires it;
   - no native bitmap/image allocation in ordinary frame callbacks;
   - native resource re-creation occurs only when physical dimensions or required byte layout change;
   - lifetime and ownership are explicit: Rust storage and the native wrapper remain valid until the window/view is closed.

3. **Framebuffer update**
   - clear/background/aperture and all 300 primitives are rasterized deterministically;
   - glyphs use alpha blits from the exact canonical atlas bytes and atlas rectangles;
   - rectangles, ellipses, and arcs use bounded software routines with no platform font or vector drawing calls;
   - source-over blending uses premultiplied alpha and integer or deterministic floating-point rules covered by unit tests;
   - clipping prevents all out-of-bounds reads/writes;
   - primary implementation may redraw the whole persistent framebuffer because all primitives have authored transform motion;
   - optional dirty-region optimization must preserve draw order, clear previous bounds, include old and new transformed bounds, and prove byte identity against full rerasterization on known frames.

4. **Native submission**
   - scheduler wakes at 15 FPS ambient/static/dynamic/resize/occlusion/capture and 30 FPS active;
   - one visible frame causes at most one native image submission;
   - zero submissions occur during the settled occlusion interval;
   - reveal renders current semantic time directly, without replaying hidden frames;
   - static track submits exactly one visible frame unless capture requires an explicitly recorded additional submission.

5. **Draw ownership**
   - the timer callback owns resolve, raster, native-storage update, metric recording, and exactly one display invalidation;
   - `drawRect:` performs only the final draw of the already-prepared persistent native image and must not resolve the fixture, raster primitives, copy the framebuffer, allocate resources, or append a second metric row;
   - retain one explicit generation/ready flag so AppKit cannot draw partially updated storage;
   - metric completion occurs after the invalidation/submission request returns; if actual `drawRect:` completion cannot be observed synchronously, record the scheduler/raster/native-update span and the draw callback span separately rather than pretending they are one synchronous duration.

6. **Resource metrics**
   - `static_rebuilds` counts framebuffer/native resource construction or dimension-triggered re-creation, not ordinary rasterization;
   - `atlas_misses` must remain zero;
   - record `rasterized_primitives`, `rasterized_pixels` or dirty-area equivalent, and native submission bytes in a software-specific typed artifact or an additive schema revision;
   - keep common `FrameMetric` fields directly comparable;
   - distinguish persistent resource creation from per-frame pixel writes. Do not claim zero upload/submission bytes when the full framebuffer is copied or submitted to AppKit.

## Evidence Directory Convention

Use distinct owned roots:

```text
target/renderer-spikes/software-functional/
target/renderer-spikes/software-feasibility-360/
target/renderer-spikes/software-feasibility-720/
target/renderer-spikes/software-memory/
target/renderer-spikes/software-build-costs/
target/renderer-spikes/software-delivery-checks/
target/renderer-spikes/final-matched-360/
target/renderer-spikes/final-matched-720/
```

Each run directory retains raw stdout/stderr, binary/environment identity, fixture/atlas, frame metrics, native resource metrics, capture metadata where applicable, accessibility tree, privacy scan, cleanup, summary, validation output, and manifest hashes.

---

## Task 1: Extend Candidate-Neutral Contracts For `software`

**Files:**
- Modify `src/renderer_spike/mod.rs`
- Modify `src/renderer_spike/artifacts.rs`
- Modify `src/cli.rs`
- Modify `tests/renderer_spike.rs`
- Modify `tests/renderer_spike_boundary.rs` only if needed

**Produces:**
- `RendererSpikeCandidate::Software`
- stable `software` serialization/CLI value
- candidate dispatch placeholder with a static not-implemented error until Task 5
- candidate-correct required artifact matrix
- candidate-correct binary feature metadata

- [ ] Write a failing test that `software` serializes and parses as `software`.
- [ ] Write a failing test that normal/non-spike help remains unchanged.
- [ ] Write a failing test that software requires macOS native execution but does not require `renderer-spike-wgpu`.
- [ ] Update every exhaustive candidate match explicitly; do not use a wildcard that hides future omissions.
- [ ] Require `frame-metrics.jsonl` and `host-boundary.json` for both wgpu and software native candidates.
- [ ] Add `software-resource-metrics.jsonl` to software required artifacts if the common frame schema cannot represent persistent resource creation, raster bytes, and native submission bytes honestly.
- [ ] Record only `renderer-spike` in `binary.json.features` for software.
- [ ] Preserve the production import-boundary test.

Run:

```bash
cargo test --features renderer-spike --test renderer_spike -- --nocapture
cargo test --features renderer-spike --test renderer_spike_boundary -- --nocapture
cargo check --locked --no-default-features --all-targets
```

**Gate:** Candidate contracts compile with and without wgpu. Ordinary CLI behavior is unchanged. No production module imports spike DTOs.

## Task 2: Specify And Test Pixel Math Before AppKit Integration

**Files:**
- Add `src/renderer_spike/software.rs`
- Add or modify comparator tests

**Produces:**
- persistent framebuffer type
- coordinate mapping
- clipping
- premultiplied source-over blend
- rectangle, ellipse, arc, aperture, and atlas alpha-blit primitives

- [ ] Write failing tests for physical dimensions and exact byte length at 360x2 and 720x2.
- [ ] Write known-vector premultiplied alpha tests for transparent, opaque, half-alpha, and repeated source-over blends.
- [ ] Write clipping tests for negative, partially visible, and fully outside primitive bounds.
- [ ] Write atlas alpha-blit tests using known canonical atlas texels, tint, and transparency.
- [ ] Write shape tests for deterministic rectangle, ellipse, and arc coverage.
- [ ] Write circular-aperture tests proving corners are outside and required central regions are nonblank.
- [ ] Write orientation tests so the same landmark appears at the expected top-left coordinates in raw RGBA and encoded capture.
- [ ] Write a canary test around the framebuffer allocation or guarded storage to detect out-of-bounds writes.
- [ ] Implement raster code without AppKit, fonts, `NSColor`, `NSBezierPath`, or attributed strings.
- [ ] Keep arithmetic bounded and checked for dimensions, row stride, offsets, and multiplication.

Run:

```bash
cargo test --features renderer-spike --test renderer_spike_software -- --nocapture
# If tests remain in the shared integration test:
cargo test --features renderer-spike --test renderer_spike -- software --nocapture
```

**Gate:** Raster primitives are deterministic, memory-safe, clipped, and independent from native drawing APIs.

## Task 3: Render The Canonical Fixture Into The Persistent Buffer

**Files:**
- Modify `src/renderer_spike/software.rs`
- Modify comparator tests
- Modify `src/renderer_spike/fixture.rs` only for candidate-neutral lookup helpers

**Produces:**
- canonical atlas lookup table consumption
- exact ordered rendering of all 300 primitives
- known-frame RGBA hashes or structural assertions
- full-raster reference path

- [ ] Write a failing test that frames at 0, 250, 1000, and 5000 ms consume exactly 300 primitives in source order.
- [ ] Test that atlas lookup covers replacement, non-BMP, and multi-scalar fixture keys with zero misses.
- [ ] Test known frame hashes only if the raster rules are fully deterministic across supported architectures; otherwise use exact required-region, primitive-count, color, alpha, transform-position, and aperture assertions.
- [ ] Test that the dynamic track changes the expected 16 atlas selections while transform motion remains applied to every authored primitive.
- [ ] Implement the primary full-raster path first.
- [ ] Record primitive count, atlas misses, rasterized primitive count, and rasterized/changed pixel accounting.
- [ ] If dirty regions are added, retain the full-raster path as the oracle and compare complete output bytes over known and randomized elapsed times.
- [ ] Do not expose dirty groups, slots, or backend policy through the shared fixture DTOs.

**Gate:** The software framebuffer matches the candidate-neutral visual oracle and workload counts without changing fixture data.

## Task 4: Prove Persistent Native Storage Without Per-Frame Allocation

**Files:**
- Modify `src/renderer_spike/software.rs`
- Add focused tests or a debug-only native resource audit

**Produces:**
- persistent native bitmap representation
- persistent image wrapper if required
- explicit storage ownership/lifetime
- native resource counters

- [ ] Check the exact objc2 AppKit initializer and bitmap data ownership APIs before implementation; record the chosen byte order and alpha format in comments and artifacts.
- [ ] Write a failing unit or integration assertion that native bitmap/image creation counters remain one after warmup at fixed size.
- [ ] Test that resize/backing-scale change creates exactly one replacement resource per new physical size, releases the old retained resource, and does not leak stale pointers.
- [ ] Test that ordinary frame updates change pixel storage without replacing the native objects.
- [ ] Test teardown ordering: stop timer, prevent new callbacks, release view/native wrapper, then drop Rust pixel storage.
- [ ] If zero-copy sharing with AppKit cannot be made memory-safe and lifecycle-explicit, allow one persistent Rust framebuffer plus one persistent native bitmap storage and copy bytes between them per frame. Record the copy bytes honestly. Do not use unsafe borrowed memory merely to improve a metric.
- [ ] Reject any design that creates `NSBitmapImageRep` or `NSImage` inside `drawRect:` or the timer callback.

**Gate:** A fixed-size run has stable native resource identities and bounded allocation counts. Unsafe storage lifetime assumptions are not accepted.

## Task 5: Integrate The Activated AppKit Host And One Submission

**Files:**
- Modify `src/renderer_spike/software.rs`
- Modify `src/renderer_spike/macos.rs` only if extracting narrow candidate-neutral activation/window helpers reduces duplication safely
- Modify `src/renderer_spike/mod.rs`

**Produces:**
- software candidate native window/view/controller
- guarded timer and draw callbacks
- explicit AppKit activation
- one native submission per visible frame
- automatic bounded exit

- [ ] Create an isolated software candidate state rather than adding software-specific fields to Smooth state.
- [ ] Set `NSApplicationActivationPolicy::Regular`, order the window front, then call `activateIgnoringOtherApps(true)` for the unbundled benchmark process.
- [ ] Record application activation and native resource creation in host-boundary evidence.
- [ ] Catch unwind at every Objective-C callback and convert panic count to a static rejection verdict.
- [ ] Schedule 15/30 FPS according to track.
- [ ] At each visible tick: resolve current fixture time, raster into persistent storage, update/copy persistent native storage, request one draw, and submit the persistent image once.
- [ ] Ensure metrics start at scheduler wake and end after native submission returns.
- [ ] Add a hard run deadline independent of successful drawing.
- [ ] Implement static track as one submission, with later timer ticks producing no additional raster/submission work.
- [ ] Reject a smoke run where `completed_visible_frames` does not remain within one startup frame of requested frames without a recorded allowed cause.

Run:

```bash
cargo xtask renderer-spike run \
  --candidate software \
  --track ambient \
  --size 360 \
  --duration-ms 12000 \
  --out target/renderer-spikes/software-host-smoke
cargo xtask renderer-spike validate \
  --out target/renderer-spikes/software-host-smoke
```

**Hour-6 ceiling checkpoint:** persistent framebuffer, persistent native resource, activated visible window, one submission per visible frame, valid metrics, and automatic exit must work. If not, issue a bounded defect statement. Do not spend the remaining time designing a general abstraction.

## Task 6: Add Resize, Backing Scale, And Current-Time Reveal

**Files:**
- Modify `src/renderer_spike/software.rs`
- Modify comparator tests

**Produces:**
- exact physical resize handling
- resource reallocation metrics
- current-time reveal after hidden interval

- [ ] Test 360 → 480 → 720 → 360 logical window changes under the existing deterministic resize track.
- [ ] Recalculate physical dimensions from view bounds and backing scale, not from logical size assumptions.
- [ ] Rebuild framebuffer/native resources only when physical dimensions or byte layout change.
- [ ] Test that the next visible frame after resize has correct dimensions, aperture, and current fixture transforms.
- [ ] Test that hidden semantic time advances without rasterizing replay frames.
- [ ] Record resource generation, reallocation count, old/new dimensions, and bytes.
- [ ] Ensure stale resize callbacks or stale resource generations cannot submit after replacement.

**Gate:** Every resize produces a correct next frame, bounded resource replacement, and no stale-pointer or stale-frame submission.

## Task 7: Add Occlusion, Accessibility, Input, And Fault Outcomes

**Files:**
- Modify `src/renderer_spike/software.rs`
- Modify `src/renderer_spike/artifacts.rs` only for candidate-neutral evidence
- Modify tests

**Produces:**
- zero settled occlusion submissions
- semantic tree parity
- pointer projection artifact
- bounded injected outcomes

- [ ] Use the same one-group/three-value accessibility fixture as wgpu.
- [ ] Update accessibility frames on resize and hide/teardown state.
- [ ] Add guarded pointer projection at 360 and 720 with Retina scale.
- [ ] Implement the 60-second qualification occlusion track: enter, settle for ten seconds, remain at zero raster/native submissions while hidden, reveal current time, exit.
- [ ] Add assertions over metric deltas during the settled hidden interval.
- [ ] Map callback panic to `reject-callback-panic` with nonzero process status where the common fault contract requires it.
- [ ] Map capture timeout to `reject-injected-capture-timeout` while retaining complete static artifacts.
- [ ] Map native bitmap setup failure through the allowlisted `surface-unavailable` injection to a software-specific bounded rejection.
- [ ] Ensure no arbitrary native error strings enter artifacts.

Run focused smoke tracks:

```bash
for track in resize occlusion; do
  cargo xtask renderer-spike run \
    --candidate software \
    --track "$track" \
    --size 360 \
    --duration-ms 12000 \
    --out "target/renderer-spikes/software-$track-360"
done
```

Run each injected fault separately and validate both intended nonzero status and retained artifacts.

**Gate:** lifecycle, semantic bridge, input mapping, and faults are bounded and validator-readable. Zero settled occlusion work is mandatory.

## Task 8: Implement Direct Software Capture And Visual Validation

**Files:**
- Modify `src/renderer_spike/software.rs`
- Modify artifact validation/tests

**Produces:**
- direct PNG from persistent framebuffer
- exact capture metadata
- 360 and 720 side-by-side review evidence

- [ ] Encode capture from the persistent software pixels, not by asking AppKit to rasterize the view into a second temporary bitmap.
- [ ] Use the existing PNG dependency only if already available under the selected feature. Because `png` is currently attached to `renderer-spike-wgpu`, either move it to the base `renderer-spike` feature or implement a narrow candidate-neutral encoder dependency change. Record the feature-size consequence.
- [ ] Preserve top-left orientation and exact physical dimensions.
- [ ] Record encoding duration, row stride, byte order, and source resource generation.
- [ ] Test nonblank required regions, aperture, foreground/background/chrome presence, transform tolerance, dynamic changes, and exact primitive count.
- [ ] Produce 360 logical / expected 720 physical capture.
- [ ] Produce 720 logical / expected 1440 physical capture.
- [ ] Perform side-by-side human review against Smooth and wgpu captures. Exact antialiasing is not required; workload content may not be removed.

Run:

```bash
cargo xtask renderer-spike run \
  --candidate software --track capture --size 360 --duration-ms 2000 \
  --out target/renderer-spikes/software-functional/capture-360
cargo xtask renderer-spike run \
  --candidate software --track capture --size 720 --duration-ms 2000 \
  --out target/renderer-spikes/software-functional/capture-720
```

**Hour-10 ceiling checkpoint:** complete fixture, capture at 360/720, resize, occlusion, native resource stability, accessibility tree, privacy, cleanup, and bounded faults must pass. Otherwise reject or spend the one allowed correction only on a demonstrated harness/comparator defect.

## Task 9: Harden Xtask, Artifact Validation, And Allocation Evidence

**Files:**
- Modify `xtask/src/lib.rs`
- Modify `src/renderer_spike/artifacts.rs`
- Modify xtask and renderer-spike tests

**Produces:**
- software candidate build/run support
- required artifact validation
- visible-cadence validation
- resource-allocation validation
- bounded cleanup

- [ ] Extend xtask candidate parsing to `smooth|wgpu|software`.
- [ ] Build software with optimized `renderer-spike`, never `renderer-spike-wgpu`.
- [ ] Freeze/copy the exact binary before packaging or other commands can overwrite `target/release/glorp`.
- [ ] Reject absolute binary paths in privacy-owned command artifacts; use repository-relative paths.
- [ ] Validate manifest hashes through the library validator, not only string containment.
- [ ] Add software-specific validation: fixed-size resource creation count, no per-frame native object creation, zero atlas misses, frame completion, settled occlusion, and capture dimensions.
- [ ] Add a seeded hanging-child test for cleanup classification.
- [ ] Add a seeded missing/corrupt software resource artifact test.
- [ ] Ensure failed runs retain stdout/stderr, cleanup, fault, privacy, and manifest evidence where possible.

Run:

```bash
cargo test -p xtask
cargo test --features renderer-spike --test renderer_spike -- --nocapture
cargo test --features renderer-spike --test renderer_spike_software -- --nocapture
```

**Gate:** every accepted software run is reproducible from hashed raw artifacts, and no failed or completed process survives.

## Task 10: Run The Software Functional Matrix

**Files:**
- No production source changes unless a demonstrated harness/comparator defect is found
- Artifacts under `target/renderer-spikes/software-functional/`

Use one frozen optimized software binary for the matrix. Record SHA-256 and bytes.

Required tracks:

| Track | Size | Minimum purpose |
| --- | ---: | --- |
| static | 360 | one submission, no repeated work |
| dynamic | 360 | canonical motion and 4 Hz atlas selection |
| ambient | 360 | 15 FPS visible cadence |
| active | 360 | 30 FPS visible cadence |
| resize | 360 | 360/480/720/360 resource replacement |
| occlusion | 360 | full 60-second settled-zero qualification |
| capture | 360 | 720x720 physical review |
| ambient | 720 | 15 FPS feasibility |
| active | 720 | 30 FPS feasibility if 720 ambient passes |
| resize | 720 | backing-scale/resource stress |
| capture | 720 | 1440x1440 physical review |
| faults | 360 | callback, capture timeout, native resource unavailable |

- [ ] Validate every run immediately.
- [ ] Preserve candidate stderr even when empty.
- [ ] Scan privacy after all derived summaries are written, then regenerate the manifest.
- [ ] Confirm no process remains.
- [ ] If a defect is fixed, rerun every affected track with a new immutable binary identity. Do not mix binaries under one evidence root.

**Gate:** all mandatory functional tracks pass before performance comparison begins.

## Task 11: Run Abbreviated Feasibility Measurements

**Files:**
- Owned scripts and artifacts under `target/renderer-spikes/`
- No committed production changes unless fixing one allowed defect

Purpose: reject an obvious failure cheaply before spending time on final matched blocks.

- [ ] Run three rotated 30–60 second 360 ambient blocks for Smooth, frozen corrected wgpu, and software.
- [ ] Use explicit AppKit activation and verify visible frame completion in every repetition.
- [ ] Run at least three abbreviated 720 ambient repetitions for software; include Smooth and wgpu if the scripts are already ready so the 720 harness is validated before final runs.
- [ ] Preserve raw one-second CPU samples, frame JSONL, RSS/physical footprint snapshots, resource counters, stdout/stderr, and validation output.
- [ ] Reject or correct a run where candidate medians diverge by more than 20% from the set median without an explained environment change.
- [ ] Capture an 8–10 second stack sample when median exceeds 8% or p95 exceeds 12% at 360.
- [ ] Measure frame CPU p50/p95/max and missed-frame rate.
- [ ] Confirm software fixed-size native resource identities remain stable through the run.
- [ ] Do not call this final ranking evidence.

**Early stop:** If software is materially slower than wgpu, misses the 720 frame/CPU feasibility target structurally, allocates native image resources per frame, or cannot maintain one bounded submission, retain evidence and reject expansion. Skip final five-minute software ranking only when the failure is conclusive and documented.

## Task 12: Measure Memory, Energy, Build, Size, And Delivery Cost

**Files:**
- Artifacts under:
  - `target/renderer-spikes/software-memory/`
  - `target/renderer-spikes/software-energy/`
  - `target/renderer-spikes/software-build-costs/`
  - `target/renderer-spikes/software-delivery-checks/`

- [ ] Measure settled RSS, physical footprint where available, peak footprint, Rust framebuffer bytes, native bitmap bytes, and post-warmup growth.
- [ ] Verify expected base pixel storage explicitly:
  - 720x720x4 bytes for the 360 Retina framebuffer;
  - 1440x1440x4 bytes for the 720 Retina framebuffer;
  - double those values if safe ownership requires separate Rust and native buffers, and report the copy.
- [ ] Use the same documented energy method as other candidates. If `powermetrics` remains unavailable without superuser access, preserve the failure and mark energy pending rather than inventing a result.
- [ ] Measure clean optimized build in an owned `CARGO_TARGET_DIR`.
- [ ] Measure incremental optimized rebuild after touching only `src/renderer_spike/software.rs`.
- [ ] Measure stripped executable size against same-commit Smooth and wgpu configurations.
- [ ] Build the default shipping app without spike features to prove no regression.
- [ ] Build and inspect a software-candidate-enabled Darwin app if the harness/package topology supports it without changing defaults.
- [ ] Record packed npm artifact impact where practical.
- [ ] Do not claim Darwin x86_64 execution. Cross-compile/package inspection is recorded separately from native qualification.

Hard limits:

- executable delta no more than 15 MiB;
- compressed companion app delta no more than 20 MiB;
- clean optimized build delta no more than 20%;
- renderer-edit incremental build delta no more than 25%.

**Gate:** a hard budget failure rejects the candidate unless a separate decision explicitly approves a replacement budget.

## Task 13: Write The First Intended 2.5D Capability Analysis

**Files:**
- Add a section to the software comparator measurement memo
- No production renderer code

Choose the first separately proposed renderer-native 2.5D feature from the research brief; do not use hypothetical breadth.

The analysis must state:

- the exact visual/behavioral feature;
- which pixels/resources are static, dynamic, and frame-varying;
- whether it can be represented by cached rasters, transformed sprites, bounded dirty regions, or a small number of precomputed variants;
- expected additional framebuffer passes and pixel work at 360 and 720;
- whether AppKit still receives one image submission;
- the bounded cache/memory cost;
- what would force rerasterization and at what cadence;
- the point at which software would recreate the current immediate-mode CPU bottleneck;
- a small proof task if one uncertainty remains, without implementing it now.

**Gate:** Software is not a credible current-scope candidate if the first approved 2.5D feature clearly requires unbounded per-frame CPU raster work or a second scene architecture.

## Task 14: Run Final Matched 360 And 720 Blocks

**Precondition:** Software passes functional gates and abbreviated feasibility. Freeze exact same-commit optimized binaries for Smooth, wgpu, and software. Do not reuse the invalid pre-activation wgpu dataset.

### 360 protocol

For each candidate:

- 30-second warmup;
- five-minute ambient run;
- one process CPU sample per second;
- raw samples plus mean/median/nearest-rank p95/max;
- frame CPU p50/p95/max;
- requested/completed/submitted/missed frames;
- primitive/raster/resource/upload/submission counters;
- memory growth evidence;
- energy evidence or explicit unavailable artifact.

Use three matched blocks with rotated order, for example:

```text
block 1: smooth, wgpu, software
block 2: wgpu, software, smooth
block 3: software, smooth, wgpu
```

Use a fixed cooldown and record frontmost/visible/occluded state. An accepted visible run must maintain completed frames within the declared startup tolerance and show no unexplained presentation cessation.

### 720 protocol

Run the same three-block rotation at 720 logical / exact backing-scale physical dimensions. Apply:

- ambient process CPU median at most 8%;
- frame CPU p95 at most 3 ms;
- missed frames below 1%;
- bounded memory and resource counts.

### Common absolute gates

- 360 process CPU median at most 5% and p95 at most 8%;
- 360 frame CPU p95 at most 2 ms;
- 720 process CPU median at most 8%;
- 720 frame CPU p95 at most 3 ms;
- missed frames below 1%;
- zero settled occlusion submissions;
- zero atlas misses;
- zero static resource rebuilds after warmup at fixed size;
- energy no worse than Smooth or explicitly inconclusive when raw uncertainty overlaps;
- bounded memory with no unexplained post-warmup growth;
- hard build/distribution limits.

Do not force `upload_bytes = 0` for software's required framebuffer-to-native submission. Report native submission/copy bytes separately and apply the zero-static-upload rule to unchanged resource construction or static atlas/resource churn, not to the candidate's intrinsic final image submission. The decision memo must compare this cost honestly against wgpu's instance writes and presentation work.

**Hour-16 ceiling checkpoint:** optimized functional evidence, corrected matched feasibility/final evidence as practical, raw artifacts, cleanup, and a pass/conditional-pass/reject comparator verdict are complete. If full five-minute blocks extend beyond the implementation checkpoint, code work stops at the checkpoint and measurement jobs may finish bounded runs; no further architecture implementation is authorized.

## Task 15: Apply The Ambiguity Gate

**Files:**
- Add `docs/superpowers/measurements/2026-07-10-glorp-renderer-software-comparator.md`
- Optionally add a short ambiguity note only when Phase D is authorized

The comparator memo must separate:

- measured facts;
- observed native behavior;
- inferences;
- engineering judgment;
- unavailable or pending evidence.

It must include:

1. exact binary identities and source state;
2. functional/fault/capture matrix;
3. persistent allocation/resource evidence;
4. 360 and 720 matched CPU/frame tables;
5. memory and energy;
6. build/executable/app/package cost;
7. privacy and cleanup;
8. accessibility/input status;
9. 2.5D capability analysis;
10. gate-by-gate verdict;
11. invalid/superseded runs and why they are excluded;
12. raw artifact paths.

### Skip Phase D when unambiguous

Phase D is skipped if any specification condition is met, including:

- wgpu passes mandatory gates and beats software by at least two median CPU percentage points or 25% normalized energy with non-overlapping uncertainty and acceptable build/package cost;
- software passes current/capability gates while wgpu fails a lifecycle, reliability, accessibility, or hard delivery gate;
- both fail because shared non-renderer work dominates;
- either candidate hits an immediate stop condition.

### Authorize Phase D only when genuinely ambiguous

A written ambiguity note must name:

- the exact metric whose uncertainty prevents selection;
- evidence that native composition/commit remains a leading unexplained cost;
- why four retained layers could answer that metric more cheaply;
- the eight-hour maximum Phase D scope and stop condition.

Do not authorize CALayer work because it is interesting or because results are merely close.

**Gate:** The memo produces a software verdict and either skips Phase D or provides the exact written authorization required by the specification.

## Task 16: Handoff To Parallel Qualification And Final Decision Memo

Phase C does not independently select a backend while mandatory qualification remains incomplete.

Before the final backend decision, reconcile:

- font/license/Unicode bake-off;
- Accessibility Inspector or VoiceOver audit and input projection proof;
- actual feature/release target topology;
- candidate-enabled Darwin package evidence;
- energy evidence or reviewed exception;
- Darwin x86_64 disposition.

Then write the final decision memo at:

```text
docs/superpowers/measurements/2026-07-10-glorp-renderer-decision.md
```

The final memo selects one of:

- wgpu;
- persistent software framebuffer;
- a specifically authorized retained CALayer result;
- no candidate, with one bounded follow-up.

Only after that memo may the retained-renderer research brief be replaced or revised into a production architecture specification.

---

## Verification Matrix

Run before completing Phase C source work:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo test --features renderer-spike --test renderer_spike -- --nocapture
cargo test --features renderer-spike --test renderer_spike_software -- --nocapture
cargo test --features renderer-spike --test renderer_spike_boundary -- --nocapture
cargo test -p xtask
cargo check --locked --no-default-features --all-targets
npm test
git diff --check
git status --short
```

Also verify:

```bash
! pgrep -f 'renderer-spike-app|renderer-spike.*software|run_.*matched'
```

Do not claim a check that was not run. Preserve warnings and nonzero output; fix root causes rather than suppressing tests.

## Immediate Stop Conditions

Stop Phase C and issue a verdict when any is established conclusively:

- correct persistent native storage requires unsafe lifetime assumptions that cannot be made explicit and bounded;
- native bitmap/image objects must be allocated per visible frame;
- one image submission per visible frame cannot sustain the requested cadence at 360 or 720;
- memory grows without a bounded resource cause;
- settled occlusion continues raster or native submission work;
- capture requires an unrelated second renderer or per-capture state that invalidates normal pixels;
- callback/fault behavior can unwind, hang, or retry without a bound;
- visual correctness requires lowering physical resolution, primitive count, transparency, clipping, HUD, or motion;
- build or package hard limits fail;
- abbreviated evidence shows software materially slower than wgpu with no single bounded comparator defect capable of changing the conclusion;
- the comparator starts growing into a production scene graph, general text engine, or multi-backend abstraction.

One correction is allowed for a demonstrated harness/comparator defect. A correction must name the defect, affected evidence, exact code change, and runs to repeat. It is not permission for open-ended optimization.

## Completion Criteria

Phase C is complete when:

- the software candidate consumes the frozen canonical fixture and atlas;
- persistent Rust and native pixel resources are proven by counters and tests;
- no forbidden per-frame native raster objects are created;
- 360 and 720 functional tracks pass or produce a static rejection;
- occlusion, resize, capture, faults, accessibility tree, privacy, and cleanup are evidenced;
- CPU, frame, memory, energy availability, build, executable, and package costs are recorded;
- corrected matched 360 and 720 evidence exists for every viable candidate, or an immediate stop condition explains why a candidate is excluded;
- the 2.5D capability analysis is written;
- the software comparator memo gives pass/conditional-pass/reject;
- the ambiguity gate explicitly skips or authorizes Phase D;
- no production renderer integration or software-comparator expansion has begun;
- no renderer-spike process survives.
