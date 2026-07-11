# Glorp Renderer Decision Harness And `wgpu` Kill-Risk Spike Implementation Plan

> **For agentic workers:** Execute this plan task-by-task. Stop at every stated checkpoint. This plan covers only Phase A and Phase B of the renderer decision-spike design. It must not begin the software framebuffer comparator or production retained renderer.

**Goal:** Build a deterministic renderer-decision harness, establish an optimized Smooth baseline, and prove or reject a minimal `wgpu`/Metal renderer inside the existing AppKit host with bounded capture, lifecycle, fault, accessibility, build, and performance evidence.

**Architecture:** A new feature-gated `renderer_spike` module owns flat benchmark-only source fixtures, resolved visual frames, assertions, artifact schemas, and candidate metrics. The Smooth candidate uses a dedicated synthetic AppKit painter adapter; the `wgpu` candidate uses an isolated `CAMetalLayer` host. Neither candidate imports or defines production retained scene contracts. An `xtask` runner builds and launches bounded candidates and validates owned result directories.

**Tech Stack:** Rust, serde/serde_json, existing objc2/AppKit host, macOS Core Animation/QuartzCore bindings as required, pinned `wgpu` Metal-only experimental dependency, xtask, existing npm app packaging.

## Global Constraints

- Preserve all current shipping/default behavior.
- Keep `dev-preview` independent from the spike features.
- Do not add `wgpu` to normal default or publish feature sets.
- Do not add a `Retained` production renderer enum variant.
- Do not import spike DTOs from production companion, presentation, round, pet, TUI, or game modules.
- Do not read real state, SQLite usage, helpers, config, prompts, source identities, project names, paths, or pet seeds.
- Do not begin the software framebuffer comparator.
- Use exact physical dimensions and the frozen common atlas for both spike candidates.
- Keep all iterative artifacts under `target/renderer-spikes/`.
- Do not commit raw large traces or binaries.
- Do not modify release defaults or publish workflow behavior.
- No Rust unwind may cross Objective-C or `wgpu` callbacks.
- Stop Phase B at its 6/12/18/24-hour checkpoints if its gate fails.
- Do not use destructive git commands or stage unrelated existing changes.

## Allowed File Set

### New files

- `docs/superpowers/plans/2026-07-10-glorp-renderer-decision-harness-wgpu-spike-implementation.md`
- `src/renderer_spike/mod.rs`
- `src/renderer_spike/fixture.rs`
- `src/renderer_spike/artifacts.rs`
- `src/renderer_spike/privacy.rs`
- `src/renderer_spike/smooth.rs`
- `src/renderer_spike/macos.rs`
- `src/renderer_spike/wgpu.rs`
- `src/renderer_spike/shaders/fixture.wgsl`
- `tests/renderer_spike.rs`
- `tests/renderer_spike_boundary.rs`
- `scripts/renderer-spike.mjs` only if xtask cannot own process sampling cleanly
- `scripts/test/renderer-spike.test.mjs` only if a Node helper is added
- curated measurement memo under `docs/superpowers/measurements/` after evidence exists

### Existing files that may be modified

- `Cargo.toml`
- `Cargo.lock`
- `src/lib.rs`
- `src/cli.rs`
- `src/commands/mod.rs`
- `src/commands/companion_app.rs` only if a shared native launch helper is required
- `src/companion/mod.rs` only for a narrow experimental host entry point
- `src/companion/app.rs` only to reuse or extract a narrow host/callback utility; do not mix fixture rendering into the shipping app state
- `xtask/src/lib.rs`
- `AGENTS.md` only after the workflow is proven

Any required file outside this list stops implementation for review.

## Experimental Feature And Command Contract

Add independent non-default features:

```toml
[features]
default = ["dev-preview"]
dev-preview = []
renderer-spike = []
renderer-spike-wgpu = ["renderer-spike", "dep:wgpu", "dep:pollster"]
```

The exact async bootstrap dependency may change after checking the pinned `wgpu` API. If a small local executor suffices, omit `pollster`.

Add target-specific optional macOS dependencies only. Pin the exact selected version; do not use a loose unpublished branch. Start with `default-features = false` and only the pinned version's Metal, WGSL, and standard-library features.

Add one hidden command compiled only with `renderer-spike`:

```text
glorp renderer-spike-app \
  --candidate smooth|wgpu \
  --track static|ambient|active|dynamic|resize|occlusion|capture \
  --logical-size 360|720 \
  --duration-ms N \
  --out target/renderer-spikes/<run-id> \
  [--inject-fault <allowlisted-category>]
```

The command runs synthetic state only and exits automatically. `wgpu` is rejected at parse/dispatch time when `renderer-spike-wgpu` is absent.

Add repository-owned xtask commands:

```text
cargo xtask renderer-spike run --candidate smooth --track capture --size 360 --out <dir>
cargo xtask renderer-spike validate --out <dir>
cargo xtask renderer-spike baseline --out-root <dir>
cargo xtask renderer-spike wgpu-checkpoint --checkpoint host|capture|lifecycle|feasibility --out-root <dir>
```

The runner may initially execute bounded functional runs before full five-minute sampling. It must never silently claim final ranking evidence from abbreviated runs.

---

## Task 1: Freeze Benchmark DTOs And Deterministic Fixture

**Files:**
- Add `src/renderer_spike/mod.rs`
- Add `src/renderer_spike/fixture.rs`
- Modify `src/lib.rs`
- Add `tests/renderer_spike.rs`

**Produces:**
- `DecisionSourceFixture`
- `DecisionResolvedFrame`
- `DecisionExpectedFrame`
- `DecisionTrack`
- deterministic `renderer-decision-companion-v1`
- fixture and frame checksums

- [ ] Write failing tests for exact primitive counts: 180 pet glyphs, 80 static glyph/sprites, 40 shapes, three depth/transparency bands, four authored transform motions, and 16 changing primitive IDs at 4 Hz.
- [ ] Test exact frames at elapsed 0, 250, 1000, and 5000 ms.
- [ ] Test that only the expected 16 primitive IDs change on the dynamic track.
- [ ] Test that source, resolved, and expected DTOs serialize deterministically.
- [ ] Add `#[cfg(feature = "renderer-spike")] pub mod renderer_spike;` to `src/lib.rs`.
- [ ] Implement flat source primitives and a resolver that emits independent draw-ready primitives. Do not expose candidate groups, slots, layers, uniforms, resources, or dirty-region policy.
- [ ] Add a compile/boundary test proving production modules do not import `crate::renderer_spike`.
- [ ] Run:

```bash
cargo test --features renderer-spike --test renderer_spike -- --nocapture
cargo test --features renderer-spike --test renderer_spike_boundary -- --nocapture
cargo check --locked --no-default-features --all-targets
```

**Gate:** deterministic checks and exact counts pass. If the DTO begins duplicating `RetainedSceneTemplate` or product models, simplify before continuing.

## Task 2: Freeze Common Temporary Atlas And Semantic Fixture

**Files:**
- Modify `src/renderer_spike/fixture.rs`
- Add fixture-owned atlas bytes or deterministic atlas generator under `src/renderer_spike/`
- Modify `tests/renderer_spike.rs`

**Produces:**
- versioned atlas bytes/hash
- glyph rectangle/metrics table
- backend-neutral accessibility semantic fixture and expected trees

- [ ] Use only license-cleared generated bitmap data or an existing redistributable repository-compatible source. Do not make the final production font decision.
- [ ] Freeze atlas dimensions, RGBA format, filtering policy, glyph rectangles, advances, baseline, and SHA-256.
- [ ] Add one replacement glyph, one non-BMP test scalar, and one multi-scalar key to lookup tests.
- [ ] Add expected accessibility roles/names/values/parentage/bounds/actions for normal, resized, hidden, fallback, and teardown states.
- [ ] Test atlas and semantic fixture hashes.

**Gate:** every candidate can consume identical atlas pixels and semantic expectations. If font licensing cannot be established for even temporary synthetic evidence, use a generated geometric glyph atlas and record the limitation.

## Task 3: Implement Artifact Schemas, Manifest, Privacy, And Validation

**Files:**
- Add `src/renderer_spike/artifacts.rs`
- Add `src/renderer_spike/privacy.rs`
- Modify `tests/renderer_spike.rs`

**Produces:**
- typed schema v1 artifacts
- `run-manifest.json`
- artifact hashing and required-file validation
- allowlisted events/errors
- privacy scanner
- deterministic aggregate/gate helpers

- [ ] Add failing round-trip tests for environment, binary, events, frame metrics, capture metadata, host boundary, fault results, accessibility tree, cleanup, and summary.
- [ ] Add seeded privacy failures for source/display names, paths, project IDs, prompts, responses, transcripts, tool payloads, diagnostics, and secret fixture strings.
- [ ] Add required artifact matrix by candidate/track.
- [ ] Add SHA-256, byte count, schema version, and relative path to manifest entries.
- [ ] Define frame metric boundaries and units exactly as the spike design requires.
- [ ] Implement mean/median/p95 and missed-frame denominator helpers with known-vector tests.
- [ ] Do not add a general telemetry framework.

**Gate:** an incomplete or privacy-failing synthetic result directory is rejected deterministically.

## Task 4: Add Hidden Spike Command And Bounded Lifecycle

**Files:**
- Modify `src/cli.rs`
- Modify `src/lib.rs`
- Modify `src/commands/mod.rs`
- Add command entry under `src/commands/` if needed
- Add/modify CLI integration tests

- [ ] Add hidden feature-gated command and typed candidate/track/size/fault arguments.
- [ ] Reject invalid sizes, durations, paths, candidates without compiled features, and non-macOS native execution with static errors.
- [ ] Ensure normal help/default/no-default-features command surfaces do not expose or require the spike.
- [ ] Add parsing and dispatch tests with and without `renderer-spike-wgpu` where practical.
- [ ] Add an automatic run deadline independent of successful frame presentation.

**Gate:** published no-default feature shape still compiles and ordinary commands are unchanged.

## Task 5: Add Xtask Runner And Process Cleanup Validation

**Files:**
- Modify `xtask/src/lib.rs`
- Add focused xtask tests
- Add Node helper only if necessary

**Produces:**
- typed xtask arguments
- build/launch/validate steps
- bounded timeout
- PID/process-cleanup evidence

- [ ] Extend xtask parsing without regressing `companion fresh`.
- [ ] Build exact optimized binaries with explicit features and target.
- [ ] Launch `renderer-spike-app` directly for evidence identity; bundle launch may be a separate packaging check.
- [ ] Capture stdout/stderr to owned logs while preserving terminal progress.
- [ ] Enforce timeout, terminate process tree, wait, and record cleanup.
- [ ] Validate required artifacts and nonzero exit behavior.
- [ ] Add tests for command construction, invalid argument rejection, timeout path, and cleanup classification.

**Gate:** a seeded hanging child is terminated and reported; no spike process survives validation.

## Task 6: Implement Synthetic Smooth Candidate

**Files:**
- Add `src/renderer_spike/smooth.rs`
- Add `src/renderer_spike/macos.rs`
- Reuse narrow helpers from `src/companion/app.rs` or `review_capture.rs` only when extraction does not alter shipping behavior
- Modify `tests/renderer_spike.rs`

**Produces:**
- same fixture through AppKit immediate-mode glyph/shape painting
- bounded native window
- exact candidate metrics and artifacts
- capture through owned review evidence

- [ ] Reuse current font/color caches and callback guard behavior where possible.
- [ ] Paint resolved primitives without reading WatchViewModel or building shipping Smooth scene plans.
- [ ] Use the common atlas only as canonical glyph identity; the Smooth baseline deliberately performs equivalent native glyph work and records that implementation difference.
- [ ] Implement 15/30 FPS tracks, exact logical/physical sizing, resize, dynamic changes, and visibility suspension.
- [ ] Emit end-to-end CPU timing, frame counts, primitive counts, static rebuilds, and native submission count.
- [ ] Capture 360 and 720 images with exact dimensions and automatic exit.
- [ ] Emit accessibility tree evidence for the shared semantic fixture.
- [ ] Ensure all Objective-C callbacks use the existing guarded pattern.

**Gate:** all functional tracks pass, artifacts validate, privacy passes, and no process remains.

## Task 7: Record Smooth Baseline Evidence

**Files:**
- No production source changes unless a harness defect is found
- Artifacts under `target/renderer-spikes/baseline-*`

- [ ] Build optimized `renderer-spike` without `wgpu`.
- [ ] Run static, dynamic, resize, occlusion, and capture functional tracks at 360 and capture/resize at 720.
- [ ] Run three abbreviated ambient repetitions first to validate stability and tooling.
- [ ] If practical within the active session, run the full three five-minute ambient repetitions; otherwise record functional evidence and clearly mark performance as pending, never final.
- [ ] Record binary identity, environment, raw CPU samples, memory, captures, and cleanup.
- [ ] Run stack sample when threshold rules require it.
- [ ] Fix only harness defects, then rerun affected evidence.

**Gate:** trustworthy Smooth evidence exists before adding `wgpu`. A stale bundle or unstable unexplained baseline blocks Task 8.

## Task 8: Add Pinned Experimental `wgpu` Dependency

**Files:**
- Modify `Cargo.toml`
- Modify `Cargo.lock`
- Modify `src/lib.rs`
- Add feature-boundary tests

- [ ] Confirm the exact current crate API/features using official docs and the lockfile.
- [ ] Add macOS-only optional dependencies and non-default feature.
- [ ] Verify `renderer-spike` compiles without `wgpu`.
- [ ] Verify `renderer-spike-wgpu` compiles on macOS.
- [ ] Verify all-features clippy sees it.
- [ ] Verify no-default-features all-targets remains green.
- [ ] Inspect `cargo tree` for enabled backends and heavy duplicates.

**Gate:** dependency isolation is proven before native surface work.

## Task 9: Host Checkpoint — AppKit View And Metal Surface

**Files:**
- Modify `src/renderer_spike/macos.rs`
- Add `src/renderer_spike/wgpu.rs`
- Modify command dispatch as needed
- Add native-focused tests where possible

**Checkpoint budget:** hour 6.

- [ ] Create a dedicated layer-backed `NSView`/`CAMetalLayer` inside the existing AppKit app/window lifecycle.
- [ ] Create/configure the `wgpu` surface only after the layer is valid.
- [ ] Render and present a clear color.
- [ ] Resize and apply backing-scale changes.
- [ ] Close and exit automatically.
- [ ] Write `host-boundary.json` naming owner thread and ordered calls for create/configure/acquire/encode/present/resize/close.
- [ ] Add runtime main-thread/owner assertions and record observed thread IDs/categories.

**Gate:** if the clear-color host cannot resize and exit within six focused hours without lifecycle replacement, reject Phase B and write the provisional report.

## Task 10: Capture Checkpoint — Minimal Fixture And Readback

**Files:**
- Modify `src/renderer_spike/wgpu.rs`
- Add `src/renderer_spike/shaders/fixture.wgsl`

**Checkpoint budget:** hour 12.

- [ ] Upload frozen atlas.
- [ ] Draw instanced glyph/sprite quads and required simple shapes from resolved primitives.
- [ ] Implement alpha/depth ordering and circular aperture sufficient for fixture assertions.
- [ ] Implement offscreen/copyable texture readback with aligned staging buffer.
- [ ] Poll explicitly, enforce timeout, map/unmap, normalize orientation/color, write PNG/metadata, and exit.
- [ ] Validate nonblank regions, dimensions, primitive counts, and expected changed IDs.

**Gate:** if bounded native readback cannot produce a valid owned capture by hour 12, reject Phase B.

## Task 11: Lifecycle Checkpoint — Scheduling, Occlusion, Faults, Accessibility

**Files:**
- Modify `src/renderer_spike/wgpu.rs`
- Modify `src/renderer_spike/macos.rs`
- Modify artifact/fault tests

**Checkpoint budget:** hour 18.

- [ ] Implement deterministic 15/30 FPS wake policy and end-to-end frame metrics.
- [ ] Stop submissions after settled occlusion and reveal at current semantic time.
- [ ] Handle zero-size/minimized state and resize during capture.
- [ ] Add deterministic fault categories for adapter/device failure, surface create/configure/acquire errors, capture timeout, and one device-loss equivalent.
- [ ] Bound retries and prevent callback unwind/hang.
- [ ] Add native accessibility elements matching the frozen semantic fixture.
- [ ] Verify keyboard/menu/fullscreen behavior and one pointer-coordinate projection.
- [ ] Remove stale accessibility children on hide/fallback/close.

**Gate:** any unbounded fault, callback unwind, accessibility dead zone, or continued occluded submission rejects Phase B.

## Task 12: Feasibility Checkpoint — Optimized Evidence And Delivery Costs

**Files:**
- No architecture expansion
- Artifacts under `target/renderer-spikes/wgpu-*`

**Checkpoint budget:** hour 24.

- [ ] Run all functional tracks at 360; capture/resize at 720.
- [ ] Run optimized abbreviated ambient repetitions and full repetitions if practical.
- [ ] Record CPU, frame, memory, energy evidence available on the machine.
- [ ] Record clean/incremental build time, stripped executable size, app archive size, dependency tree, and package smoke evidence practical in-session.
- [ ] Run fault, accessibility, privacy, and cleanup validation.
- [ ] Compare feasibility CPU against same-block Smooth; apply only kill threshold, not final backend selection.
- [ ] Write a provisional measurement memo with pass, conditional-pass, or reject and exact pending evidence.
- [ ] Remove abandoned code/dependencies if rejected. Retain only explicitly justified harness/surface evidence.

**Gate:** stop before software comparator regardless of outcome.

## Task 13: Repository Verification

Run and fix all failures:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo check --locked --no-default-features --all-targets
npm test
```

Also run:

```bash
cargo test --features renderer-spike --test renderer_spike
cargo test --features renderer-spike --test renderer_spike_boundary
cargo test --features renderer-spike-wgpu --test renderer_spike
cargo test -p xtask
```

Inspect:

```bash
git diff --check
git status --short
```

Confirm:

- no spike process remains;
- no temporary app bundle/process/output exists outside owned `target/renderer-spikes/` paths;
- default `cargo run -- --help` and no-default-features builds are unchanged;
- no production module imports benchmark DTOs;
- software comparator has not begun.

## Deliverable

Write or update:

```text
docs/superpowers/measurements/2026-07-10-glorp-renderer-wgpu-kill-risk.md
```

The report must distinguish:

- measured fact;
- observed behavior;
- inference;
- product/engineering judgment;
- pending evidence not completed in the active environment.

It must include exact artifact paths, commands, binary hashes, feature sets, cleanup status, and a Phase B verdict. It must not claim a backend winner before the software comparator and final matched comparison.
