# Glorp wgpu Qualification And Backend Decision Implementation Plan

> **For agentic workers:** Execute this plan task-by-task and stop at every stated gate. This plan closes the renderer decision program. It may make one bounded correction to the existing wgpu spike, collect qualification evidence, and write the final backend decision memo. It must not add a production companion renderer mode, import spike DTOs into production modules, revise the retained-renderer research brief into an approved architecture, or change the shipping Smooth default.

**Goal:** Determine whether the surviving `wgpu`/Metal candidate satisfies every remaining mandatory selection gate, then select `wgpu` or explicitly select no backend with one bounded follow-up.

**Architecture:** Keep all executable renderer work inside the existing non-default `renderer-spike-wgpu` harness. Correct the known full-instance upload defect by separating immutable source/style data from frame-varying motion and the 16 semantic atlas selections, without changing the canonical fixture or visible output. Freeze one corrected optimized arm64 binary and use it for the complete native 360/720 qualification matrix; record separate immutable identities for other target/package artifacts. Run the font/Unicode, native accessibility/input, release/package, energy, and Darwin x86_64 qualification tracks as evidence work. The final memo at `docs/superpowers/measurements/2026-07-10-glorp-renderer-decision.md` is the only selection authority.

**What the user will see after this chunk:** no change in the ordinary companion. `cargo xtask companion fresh` still launches Smooth. The corrected wgpu benchmark window and captures remain available through hidden spike commands. A real companion `--renderer retained` mode belongs to the following production-integration plan only if this chunk selects wgpu.

**Timebox:** 16 focused person-hours for code correction, automated qualification, measurement preparation, and the final decision record. Manual audit/hardware/privileged evidence may be supplied externally. Do not wait indefinitely for unavailable hardware or permissions; write a dated exception proposal with owner, expiry, and risk, and let the final decision gate accept or reject it explicitly.

### Human decision checkpoints

The implementation agent owns code, automated evidence, and draft recommendations. It does **not** approve its own policy exceptions or product visual decisions.

- The decision owner is the repository owner/user unless another reviewer is named in the resulting evidence memo.
- Pause for explicit decision-owner approval of the selected font visual result or transitional font exception.
- Pause for a human Accessibility Inspector/VoiceOver checklist; this gate cannot be self-waived by the implementation agent.
- Ask before any privileged energy command. If permission is not granted, present the exception proposal for explicit approval or rejection.
- Present the Darwin x86_64 disposition for explicit approval when native Intel evidence is unavailable.
- A final memo may select wgpu only after these checkpoints are resolved as pass or approved disposition. “Pending” is no-selection.
- All checkpoints must resolve within this chunk's 16-hour implementation/evidence timebox. At the ceiling, unresolved checkpoints force an explicit no-selection memo; they do not leave the task waiting indefinitely.

---

## Entry State

Phase A through Phase C provide:

- deterministic `renderer-decision-companion-v1` fixture and temporary atlas;
- exact 300-primitive shared workload and candidate-neutral tracks;
- typed artifacts, hashes, privacy scanning, cleanup, and xtask validation;
- corrected AppKit activation protocol;
- functional/capture/fault/occlusion evidence for Smooth, wgpu, and software;
- corrected wgpu 360 feasibility evidence;
- a software rejection based on three repeated 720 frame-CPU failures;
- a written ambiguity gate that skips retained CALayer work;
- Smooth preserved as the production/default renderer.

Curated inputs:

- `docs/superpowers/specs/2026-07-10-glorp-renderer-decision-spike-design.md`
- `docs/superpowers/specs/2026-07-10-glorp-retained-rust-renderer-design.md` — research brief only, not production authority
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-wgpu-kill-risk.md`
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-software-comparator.md`

Known corrected wgpu feasibility input:

- binary: `target/renderer-spikes/bin/glorp-wgpu-spike-activated`
- SHA-256: `932d8719764b3015f37c8d1f18987dc6366ddd9be009bbe50372072ac97fe63c`
- corrected 360 artifacts: `target/renderer-spikes/wgpu-matched-ambient-30s-activated/`

This binary is an audit input, not the final qualification binary. The retention correction changes upload behavior, so all final wgpu performance and resource evidence must use a newly frozen identity.

---

## Scope Boundary

### Included

1. One bounded wgpu retention correction:
   - immutable source/style data is uploaded at activation or generation change;
   - authored motion for all 300 primitives and the 16 semantic atlas selections use bounded frame data;
   - unchanged source/style bytes are zero after warmup;
   - capture uses the same corrected resource path.
2. Complete corrected wgpu functional evidence at 360 and 720.
3. Final five-minute wgpu/Smooth matched qualification at 360 and 720.
4. Memory, renderer/bootstrap timing diagnostics, energy availability/result, build, executable, app archive, and npm platform-package evidence.
5. Font/license/Unicode bake-off for the bounded current repertoire.
6. Accessibility/input automated evidence plus a manual audit checklist and imported evidence when available.
7. Cargo/release topology across the five publish targets.
8. Darwin x86_64 disposition.
9. Final backend decision memo.
10. Cleanup of superseded benchmark-only files only when the final memo explicitly approves removal.

### Excluded

- adding `CompanionRendererMode::Retained` or `Wgpu`;
- editing the production AppKit companion painter to use wgpu;
- reading real pet/usage/config/helper state from the spike;
- deriving production retained scene contracts from benchmark DTOs;
- adding camera, mesh, lighting, particles, materials, or general scene APIs;
- tuning software or reviving CALayer;
- changing the default renderer or publish defaults;
- deleting Smooth or its fallback paths;
- implementing a native-font-dependent production atlas as a shortcut around the font decision;
- claiming Intel runtime qualification from cross-compilation alone.

If any mandatory selection evidence requires production renderer integration, stop and record that the candidate cannot be selected from the decision spike. Do not smuggle integration into qualification.

---

## Global Constraints

- Preserve ordinary CLI/help and companion behavior.
- Keep `renderer-spike-wgpu` non-default and macOS-only.
- Published no-default Linux/Windows binaries must not include or link Metal/wgpu code.
- Production modules must not import `crate::renderer_spike` DTOs. Preserve and extend `tests/renderer_spike_boundary.rs` when needed.
- Use the exact canonical fixture, atlas, semantic expectations, physical dimensions, cadence, aperture, orientation, and activation protocol.
- Freeze one exact optimized corrected arm64 wgpu binary before final native functional and performance evidence. Package/build evidence records one immutable identity per target. Do not mix arm64 runtime identities under one qualification root.
- Final matched performance compares Smooth and corrected wgpu only. Software is rejected and excluded with its documented immediate-stop reason.
- Runtime measurements use relative repository paths and synthetic data only.
- Every success and fault directory receives privacy and cleanup validation.
- Do not silently waive energy, manual accessibility, font, package, or x86 evidence. A reviewed exception is an explicit decision artifact, not a passing measurement.
- Do not use `sudo`, install unreviewed fonts, or change signing/notarization/release policy merely to make a gate pass.
- Keep large/raw artifacts under `target/renderer-spikes/`; commit only curated memos, small manifests, license text approved for inclusion, tests, and plan/spec changes.
- One renderer correction is allowed: the already-demonstrated unchanged-instance upload defect. A later measurement/harness correction is allowed only for a newly demonstrated defect and must name affected evidence and rerun all affected tracks with a new binary hash.

---

## Allowed File Set

### New committed files

- `docs/superpowers/plans/2026-07-10-glorp-renderer-wgpu-qualification-decision-implementation.md`
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-decision.md`
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-font-unicode.md`
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-accessibility-input.md`
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-release-topology.md`
- `docs/superpowers/measurements/2026-07-10-glorp-renderer-x86-disposition.md`
- small license/attribution files under a renderer qualification asset directory only after approval
- focused tests under `tests/` or `scripts/test/` when existing files would become unwieldy

### Existing files that may be modified

- `src/renderer_spike/wgpu.rs`
- `src/renderer_spike/fixture.rs` only for candidate-neutral glyph manifests or assertions
- `src/renderer_spike/artifacts.rs`
- `src/renderer_spike/privacy.rs`
- `src/renderer_spike/mod.rs`
- `src/renderer_spike/shaders/fixture.wgsl` only if the retention correction requires a compatible static/dynamic binding layout; no visual feature expansion
- `tests/renderer_spike.rs`
- `tests/renderer_spike_boundary.rs`
- `xtask/src/lib.rs` and `xtask/Cargo.toml`
- `Cargo.toml` / `Cargo.lock` only for feature topology or an approved font parser/raster tool required by the bake-off
- `scripts/build-macos-app-shared.mjs`
- `scripts/build-macos-companion-app.mjs`
- `scripts/build-platform-package.mjs`
- `scripts/test/macos-app-packaging.test.mjs`
- `.github/workflows/publish.yml` only to add a non-publishing qualification/dry-run path or prove target topology; do not change normal tag publication features/defaults
- `AGENTS.md` only after a workflow is proven and should become durable guidance

### Explicitly forbidden in this chunk

- `src/companion/app.rs`
- `src/companion/review_capture.rs`
- `src/commands/companion_mode.rs`
- `src/commands/companion_app.rs`
- production `src/presentation/`, `src/round/`, `src/pet/`, `src/tui/`, `src/game/`, `src/storage/`, and `src/usage/` modules

A required edit to a forbidden production path stops this plan and requires a separate production-integration plan after the backend decision.

---

## Evidence Directory Convention

Use distinct owned roots:

```text
target/renderer-spikes/wgpu-qualified-functional/
target/renderer-spikes/wgpu-qualified-matched-360/
target/renderer-spikes/wgpu-qualified-matched-720/
target/renderer-spikes/wgpu-qualified-memory/
target/renderer-spikes/wgpu-qualified-startup/
target/renderer-spikes/wgpu-qualified-energy/
target/renderer-spikes/wgpu-qualified-build/
target/renderer-spikes/wgpu-qualified-delivery/
target/renderer-spikes/wgpu-qualified-font/
target/renderer-spikes/wgpu-qualified-accessibility/
target/renderer-spikes/wgpu-qualified-x86/
target/renderer-spikes/bin/
```

Each automated run preserves raw stdout/stderr, exact binary/environment identity, frame/events/resource metrics, captures where applicable, privacy scan, cleanup, validation output, and manifest hashes.

---

## Qualification Gates

### Performance

On the pinned primary machine, optimized release, Retina scale 2:

- 360 process CPU median `<= 5%`, p95 `<= 8%` over all one-second samples; the synthetic fixture performs no usage polls;
- 360 frame CPU p95 `<= 2 ms`;
- 720 process CPU median `<= 8%`;
- 720 frame CPU p95 `<= 3 ms`;
- missed frames `< 1%`;
- zero atlas misses;
- zero static rebuilds and unchanged static instance uploads after warmup;
- zero submissions over a 60-second settled occlusion interval;
- five-minute visible runs have bounded memory and no recovery/fallback.

### Resource/startup

- renderer-attributable GPU resources target `<= 64 MiB` at 720;
- renderer-attributable CPU caches target `<= 32 MiB`;
- record process/bootstrap-to-first-present p95 over 20 synthetic launches as a diagnostic; the production “state load complete to first valid frame” `<= 500 ms` gate is deferred to the real-companion integration chunk because the synthetic spike has no production state-load boundary;
- no unbounded atlas, pipeline, readback, or capture resource growth.

### Build/distribution

- stripped Darwin executable delta `<= 15 MiB`;
- compressed app delta `<= 20 MiB` for arm64 and x86_64 artifacts;
- clean release build delta `<= 20%`;
- renderer-edit incremental delta `<= 25%`;
- Linux/Windows publish binaries remain free of Metal/wgpu code;
- `dev-preview` remains excluded from no-default publish builds;
- candidate-enabled Darwin app archives are assembled and inspected without changing default publish behavior.

### Mandatory nonperformance

- AppKit lifecycle, capture/readback, resize/backing scale, occlusion, callback/error boundary, privacy, cleanup, and deterministic fixture gates pass;
- font/license/Unicode policy passes or the final memo explicitly selects a time-limited transitional native-atlas exception with host/version dependence;
- accessibility/input audit passes; no implementation-agent-authored exception can replace the manual audit;
- Darwin x86_64 has native evidence, an approved external qualification process, a product support decision, or a time-limited exception with owner/expiry/risk;
- energy is no worse than Smooth, or an explicit user/reviewer-approved exception is approved;
- no production coupling exists.

A mandatory gate failure rejects wgpu regardless of CPU.

### Exception/disposition artifact contract

Every non-measurement disposition uses a committed memo section or dedicated measurement file containing:

- gate name and measured/unavailable evidence;
- decision owner and approval date;
- explicit approve/reject decision;
- risk and affected release targets;
- expiry date no later than the next renderer-enabled release qualification;
- exact procedure/command that closes the exception;
- deterministic fallback: if not approved before the timebox ceiling, select no backend.

Accessibility is excluded from this exception mechanism and must have a manual+automated pass.

---

## Task 1: Freeze Qualification Protocol And Corrected Binary Contract

**Files:**
- Modify `xtask/src/lib.rs`
- Modify `src/renderer_spike/artifacts.rs`
- Modify renderer-spike and xtask tests
- Add owned scripts under `target/renderer-spikes/`

- [ ] Add a qualification runner mode or checked script that accepts an explicit frozen binary path and refuses absolute/private paths in persisted commands.
- [ ] Record target triple, profile, features, wgpu version/features, binary SHA-256/bytes/mtime, commit, dirty state, display/backing scale, power source, and frontmost state.
- [ ] Define final matched run IDs, rotation, cooldown, warmup, and sample count before running data. The synthetic fixture performs no usage poll, so no poll-window exclusion is applied or reported.
- [ ] Add deterministic tests for nearest-rank p95, missed-frame denominator, and run-median divergence.
- [ ] Ensure every validation result verifies all manifest byte counts and hashes.
- [ ] Freeze one newly corrected binary only after Task 2 and never overwrite it.

**Gate:** final evidence can be reproduced from one binary identity and deterministic protocol. No measurement begins from `target/release/glorp` directly.

## Task 2: Split Static And Dynamic wgpu Instance Updates

**Files:**
- Modify `src/renderer_spike/wgpu.rs`
- Modify `src/renderer_spike/shaders/fixture.wgsl` only if required
- Modify focused renderer-spike tests

- [ ] Write failing tests or pure helpers that classify immutable source/style fields, all 300 frame-varying authored transforms, and the exact 16 semantic atlas-selection overrides. Do not mislabel the other 284 primitives as motion-static: every canonical primitive moves.
- [ ] Establish stable draw ordering and buffer ranges without changing fixture counts, transform formulas, blending, aperture, capture, or expected pixels.
- [ ] Upload immutable base bounds, colors, shape kinds, depth, motion IDs, and base atlas data once per resource generation.
- [ ] Choose and document one bounded frame path:
  - update a frame-transform buffer for all 300 moving primitives plus atlas overrides for the 16 semantic primitives; or
  - reproduce the candidate-neutral authored motion formulas in the shader from immutable motion IDs plus a small time/frame uniform, while updating only the 16 atlas overrides.
- [ ] Compare resolved positions at known elapsed times against `resolve_frame` within a declared tolerance. A shader-motion path must not alter motion or weaken the visual oracle merely to reduce CPU uploads.
- [ ] Record static upload bytes, dynamic upload bytes, uniform/transform bytes, resource generation, and draw calls separately.
- [ ] Prove static track has zero post-activation uploads and ambient motion has zero unchanged static uploads.
- [ ] Preserve capture and fault behavior through the corrected buffers.
- [ ] Do not optimize draw count or change the fixture beyond what is required to make upload accounting truthful.

**Gate:** unchanged source/style upload bytes are zero after warmup; frame-varying transform/uniform and 16-slot atlas-update bytes are reported honestly; visual/capture assertions, draw order, zero misses, and all lifecycle/fault tests still pass. If the correction requires production scene contracts or a general batching engine, reject selection and stop.

## Task 3: Run Corrected Functional And Resource Matrix

**Files:**
- No source changes unless a demonstrated qualification defect is found
- Artifacts under `target/renderer-spikes/wgpu-qualified-functional/`

Use the newly frozen corrected binary for:

- static, dynamic, ambient, active, resize, occlusion, and capture at 360;
- ambient, active, resize, occlusion, and capture at 720;
- callback panic, capture timeout, and surface unavailable faults;
- 60-second settled occlusion qualification;
- 20-launch synthetic process/bootstrap-to-first-present diagnostic at 360, with a bounded 720 subset. Do not claim this satisfies the later production state-load startup gate.

- [ ] Validate every run immediately.
- [ ] Confirm exact 720x720 and 1440x1440 capture dimensions, expected transform positions, and accepted side-by-side visual output.
- [ ] Confirm requested/presented/submitted counts and startup tolerance.
- [ ] Confirm static/dynamic upload accounting.
- [ ] Confirm memory/resource identities stay bounded across resize and reveal.
- [ ] Confirm all fault processes exit nonzero with complete static evidence and no survivors.

**Gate:** all functional, resource, synthetic bootstrap diagnostic, visual, privacy, and cleanup gates pass before long matched runs.

## Task 4: Complete Font And Unicode Bake-Off

**Files:**
- Add `docs/superpowers/measurements/2026-07-10-glorp-renderer-font-unicode.md`
- Modify `src/renderer_spike/fixture.rs` only for deterministic glyph-manifest export
- Add owned tools/artifacts under `target/renderer-spikes/wgpu-qualified-font/`
- Add approved font/license assets only after the decision

- [ ] Generate a deterministic required-glyph manifest from current pet, room, prop, tank-life, HUD, and Preview Lab fixtures without reading user state.
- [ ] Shortlist no more than three redistributable font candidates.
- [ ] Record license, attribution, source inclusion, binary distribution, atlas generation, and subsetting terms from primary license files.
- [ ] Render the full species/stage/state matrix at 260, 360, 480, and 720.
- [ ] Record advance, baseline, ascent/descent, weight, and pixel snapping.
- [ ] Prove replacement, one non-BMP scalar, and one multi-scalar atlas key end to end.
- [ ] Measure source/subset/atlas/package bytes.
- [ ] Produce side-by-side review captures and a recommendation.

**Gate:** select an approved font policy with zero missing required glyphs and accepted visual output, or write one explicit transitional native-atlas exception. An ambiguous shortlist does not pass.

## Task 5: Complete Accessibility And Input Audit

**Files:**
- Add `docs/superpowers/measurements/2026-07-10-glorp-renderer-accessibility-input.md`
- Modify benchmark-only accessibility/input code and tests if a defect is demonstrated
- Artifacts under `target/renderer-spikes/wgpu-qualified-accessibility/`

Automated evidence:

- [ ] Window/habitat roles and sanitized names/values.
- [ ] Exactly one group plus three HUD values and no per-glyph child explosion.
- [ ] Bounds update through 360/720 resize and Retina backing scale.
- [ ] Pointer conversion at 360 and 720 from actual synthetic `NSEvent` delivery, not only direct function calls.
- [ ] Stale generation/frame input snapshots are rejected.
- [ ] Focus/children cleanup through hide, fault/fallback state, and close.
- [ ] Seeded private labels fail privacy validation.

Manual evidence:

- [ ] Accessibility Inspector or VoiceOver focus traversal.
- [ ] Value reading, hit testing, resize, hide/reveal, and stale-child checks.
- [ ] Menu, quit shortcut, fullscreen, and keyboard focus behavior.
- [ ] Checklist records macOS version, tool, operator, date, and screenshots where appropriate.
- [ ] Request and complete the manual audit during this chunk. If no human reviewer is available by the timebox ceiling, record a failed/unresolved gate and select no backend.

**Gate:** manual and automated audit pass. If manual execution is unavailable, final selection is blocked. The implementation agent cannot approve an accessibility exception.

## Task 6: Prove Cargo, Release, App, And Package Topology

**Files:**
- Modify Cargo/scripts/workflow/tests only as allowed above
- Add `docs/superpowers/measurements/2026-07-10-glorp-renderer-release-topology.md`
- Artifacts under `target/renderer-spikes/wgpu-qualified-build/` and `.../wgpu-qualified-delivery/`

- [ ] Define an exact feature table: current default/no-default behavior; non-Darwin release behavior; candidate-enabled Darwin qualification command; proposed post-selection production feature name without implementing it.
- [ ] Build optimized no-default binaries for all five publish targets with the repository's stable rustup CI topology.
- [ ] Prove non-Darwin artifacts do not contain/link wgpu, Metal, or AppKit GPU code using dependency trees, symbol/string audits, and byte/content comparisons where meaningful.
- [ ] Run all-feature clippy/test compilation on Ubuntu/macOS/Windows through CI or equivalent owned jobs.
- [ ] Build candidate-enabled arm64 and x86_64 Darwin binaries.
- [ ] Package each candidate-enabled binary through `build-macos-companion-app.mjs --binary ...` into distinct app paths. The existing launcher still invokes `companion-app` and therefore Smooth; this proves dependency/size/package topology only, not wgpu runtime integration.
- [ ] Produce compressed app archives and compare executable/app/package bytes against same-commit no-default baselines.
- [ ] Run npm platform package dry-run/pack inspection for all five targets; exclude debug symbols and target caches.
- [ ] Record dynamic linkage, bundled font/license/shader resources, signing/notarization impact, and launch-smoke results separately:
  - app-launch smoke proves the candidate-enabled binary still launches the ordinary Smooth companion without changing defaults;
  - direct hidden `renderer-spike-app --candidate wgpu` smoke proves wgpu runtime for that native target where executable.
- [ ] Do not modify normal tag publication features or defaults.

Minimum command/artifact matrix:

```bash
# Same-commit no-default baselines, matching the publish workflow.
cargo build --release --locked --no-default-features --target aarch64-apple-darwin
cargo build --release --locked --no-default-features --target x86_64-apple-darwin
cargo build --release --locked --no-default-features --target x86_64-unknown-linux-gnu
cargo build --release --locked --no-default-features --target aarch64-unknown-linux-gnu
cargo build --release --locked --no-default-features --target x86_64-pc-windows-msvc

# Candidate-enabled Darwin qualification binaries only.
cargo build --release --locked --no-default-features \
  --features renderer-spike-wgpu --target aarch64-apple-darwin
cargo build --release --locked --no-default-features \
  --features renderer-spike-wgpu --target x86_64-apple-darwin

node scripts/build-macos-companion-app.mjs \
  --binary target/aarch64-apple-darwin/release/glorp \
  --out target/renderer-spikes/wgpu-qualified-delivery/arm64/Glorp.app
node scripts/build-macos-companion-app.mjs \
  --binary target/x86_64-apple-darwin/release/glorp \
  --out target/renderer-spikes/wgpu-qualified-delivery/x86_64/Glorp.app
```

For every target retain command/status logs, SHA-256, stripped bytes, dependency tree, and platform-package dry-run manifest. For Linux/Windows, fail the gate if dependency metadata or binary symbol/string inspection identifies `wgpu`, `Metal`, `CAMetalLayer`, or renderer-spike command strings in the no-default publish artifact. For Darwin, retain `.app` directory bytes, `.tgz` bytes, executable bytes, linkage output, and bundled resource inventory.

**Gate:** all hard build/size limits pass and no target leakage exists. A candidate-enabled x86 app archive is necessary but not native runtime proof. No packaging result may be described as a real-companion wgpu launch.

## Task 7: Decide Darwin x86_64 Qualification

**Files:**
- Add `docs/superpowers/measurements/2026-07-10-glorp-renderer-x86-disposition.md`
- Artifacts under `target/renderer-spikes/wgpu-qualified-x86/`

Choose exactly one:

1. native surface/capture/fault smoke on Intel macOS hardware;
2. documented external pre-release qualification procedure;
3. separate product/release decision dropping Intel retained-renderer support while preserving other CLI behavior;
4. temporary exception with named owner, expiry date, required smoke procedure, and risk.

Decision ladder inside the 16-hour chunk:

1. cross-build and package x86_64 immediately;
2. use native Intel evidence if already available or obtainable without delaying the timebox;
3. otherwise draft the external pre-release procedure or temporary exception and present it to the decision owner;
4. if no disposition is explicitly approved before the ceiling, select no backend.

- [ ] Always cross-build and package x86_64 first.
- [ ] Never describe cross-compilation as native qualification.
- [ ] If using external evidence, retain exact binary hash and result artifacts.
- [ ] If using an exception, present it to the user/reviewer; the implementation agent may recommend but may not approve it.

**Gate:** one explicit disposition is approved. “Pending” cannot select a backend.

## Task 8: Obtain Energy Evidence Or A Reviewed Exception

**Files:**
- Artifacts under `target/renderer-spikes/wgpu-qualified-energy/`
- Curated result in the final decision memo or a dedicated short measurement note

Preferred protocol:

- same corrected Smooth/wgpu binary and five-minute 360/720 ambient blocks;
- same frontmost/cooldown/power-source conditions;
- repeatable `powermetrics` or approved counter set;
- raw output, normalization method, uncertainty, and no-worse-than-Smooth comparison.

Allowed measurement methods are one consistent same-machine method across both candidates: `powermetrics` CPU/GPU package counters, or a predeclared reviewed macOS energy counter/export with raw samples and a documented parser. Process CPU is not an energy substitute.

- [ ] Do not invoke `sudo` without explicit user approval.
- [ ] If privileged measurement is unavailable, write an exception proposal with reason, owner, expiry, risk, and exact future command/procedure, then obtain explicit user/reviewer approval or rejection.
- [ ] Do not infer energy from process CPU alone.
- [ ] Attempt the approved method once after permission/environment is established. Do not repeatedly retry privileged collection inside the timebox.
- [ ] If measurement cannot run, present the disposition artifact immediately; unresolved at the ceiling means no selection.

**Gate:** measured no-worse-than-Smooth energy or an explicitly approved exception. Unreviewed unavailability blocks selection.

## Task 9: Run Final Matched Smooth/wgpu Qualification

**Files:**
- Owned scripts/artifacts under the matched roots
- No source changes unless a single demonstrated measurement defect is corrected and all evidence is rerun

For each size, use three rotated blocks:

```text
block 1: smooth, wgpu
block 2: wgpu, smooth
block 3: smooth, wgpu
```

Per process:

- 30-second warmup;
- five-minute ambient run;
- one process CPU sample per second;
- no poll exclusion: the synthetic fixture performs no usage polls, so all one-second samples belong to the ranking set;
- raw and aggregate process CPU;
- frame CPU p50/p95/max, requested/presented/submitted/missed counts;
- draw calls, instance counts, static/dynamic/uniform upload bytes, atlas misses, rebuilds, recovery/fallback;
- RSS/physical footprint and renderer resource estimates;
- energy result or approved exception reference;
- frontmost/visible/occluded and power-source evidence;
- validation and cleanup.

- [ ] Reject a run with unexplained visibility cessation, thermal/power divergence, wrong binary, mixed features, failed privacy/cleanup, or run-median divergence above 20%.
- [ ] Capture an 8–10 second stack sample when the 360 process median exceeds 8% or p95 exceeds 12%, as required by the decision protocol, and for any 360/720 frame-p95 failure to identify whether the cause is bounded.
- [ ] Do not reuse the invalid pre-activation dataset.

**Gate:** corrected wgpu passes the absolute 360 and 720 CPU/frame/miss/resource gates. Smooth remains the matched baseline even when it fails the new absolute budget.

## Task 10: Write The Final Backend Decision Memo

**Files:**
- Add `docs/superpowers/measurements/2026-07-10-glorp-renderer-decision.md`

The memo must distinguish measured facts, observed native behavior, inference, unavailable evidence, reviewed exceptions, and product/engineering judgment. It contains:

1. exact commit, dates, fixture/harness version, and all final binary identities;
2. candidate implementations, deliberate omissions, and rejected alternatives;
3. environment, activation, rotation, warmup, cooldown, repetition protocol, and the explicit fact that the synthetic fixture has no poll exclusions;
4. raw artifact paths;
5. Smooth/wgpu 360 and 720 tables;
6. capture/visual review;
7. lifecycle, fault, accessibility/input, privacy, cleanup, memory, startup, and energy;
8. font decision;
9. release feature/build/app/package results;
10. Darwin x86_64 disposition;
11. software rejection and CALayer skip;
12. selected backend or explicit no-selection;
13. one bounded follow-up only when the verdict is conditional;
14. constraints imposed on the following architecture/integration spec;
15. prototype code/assets that are retained or removed.

Selection rules:

- select `wgpu` only if every mandatory gate passes or has a disposition explicitly approved by the user/reviewer and both performance sizes pass;
- select no candidate if any mandatory gate remains merely pending, a reviewed exception is rejected, or corrected wgpu misses an absolute performance/resource gate;
- accessibility must be a measured manual+automated pass, not an exception;
- do not call the result a pass or selection while multiple unresolved exception proposals remain. The final memo lists every approved disposition individually and explains cumulative risk;
- never select software or CALayer contrary to the completed Phase C ambiguity decision;
- never treat the final memo as authorization to flip the default.

**Gate:** one unambiguous decision exists and every claim points to raw evidence.

## Task 11: Cleanup And Handoff To Production Planning

- [ ] Remove abandoned qualification scripts/assets only; preserve raw owned evidence until the decision is reviewed.
- [ ] Stop all benchmark and helper processes.
- [ ] Verify default companion/package behavior remains Smooth and no-default.
- [ ] Run the full verification matrix below.
- [ ] Inspect `git status` and ensure no pre-existing workspace file was deleted.
- [ ] If and only if the final memo selects wgpu and names no blocking unresolved gate, produce—not merely promise—both:
  - a revised production architecture spec; and
  - `docs/superpowers/plans/2026-07-10-glorp-retained-companion-pilot-implementation.md`.
- [ ] The successor plan contract must specify:
  - a new non-default macOS production feature distinct from `renderer-spike-wgpu`;
  - a hidden `CompanionRendererMode::Retained` / `companion-app --renderer retained` review mode while `Smooth` remains default and explicit fallback;
  - backend-neutral retained template, bounded dynamic-content, and frame-state inputs derived from the existing Smooth/round projection—not renderer-spike DTOs;
  - initial synthetic/redacted review states before live state, followed by bounded real-state review only after privacy artifacts pass;
  - 260/360/480/720 capture parity, resize/backing scale, occlusion, fault/fallback, accessibility/input, package, and automatic-exit gates;
  - no default flip, no Smooth deletion, and no new 2.5D feature in the pilot.
- [ ] Do not implement the successor plan in this chunk.
- [ ] If no backend is selected, write exactly one bounded research follow-up and do not begin production integration.

**Gate:** qualification artifacts and a final decision are complete; production source remains untouched.

---

## Final Verification Matrix

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo test --features renderer-spike-wgpu --test renderer_spike -- --nocapture
cargo test --features renderer-spike-wgpu --test renderer_spike_boundary -- --nocapture
cargo test -p xtask
cargo check --locked --no-default-features --all-targets
npm test
git diff --check
git status --short
! pgrep -f 'renderer-spike-app|run_.*matched|powermetrics.*glorp'
```

Also run or retain CI evidence for:

- Ubuntu, macOS, and Windows `cargo test --locked`;
- all five optimized no-default publish-target builds;
- candidate-enabled arm64/x86_64 Darwin builds and app archives;
- privacy/manifest validation for every accepted final result.

Do not claim a check that did not run. Preserve warnings and nonzero output and correct root causes rather than weakening coverage.

---

## Immediate Stop Conditions

Stop and select no backend when any is conclusive:

- static/unchanged instance uploads cannot be removed without a production scene architecture;
- corrected wgpu misses the 360 or 720 absolute CPU/frame/miss gate;
- lifecycle, capture, resize, occlusion, callback/fault, privacy, or cleanup regresses;
- accessibility/input audit fails;
- no font policy or bounded transitional exception is acceptable;
- hard build/app/package limit fails;
- Metal/wgpu leaks into non-Darwin publish artifacts;
- no acceptable energy or x86 disposition can be approved;
- final evidence requires changing the shipping companion/default renderer;
- qualification grows into production retained contracts or visual feature development.

One correction is allowed only for a demonstrated qualification or measurement defect. Record the defect, affected evidence, exact fix, new binary hash, and complete rerun set.

---

## Completion Criteria

This chunk is complete when:

- the corrected wgpu spike has zero unchanged static uploads after warmup;
- one immutable corrected binary drives final functional and matched evidence;
- 360/720 performance, synthetic bootstrap diagnostics, memory, energy/disposition, font, accessibility/input, release/package, and x86 gates have explicit results;
- every accepted artifact validates and passes privacy/cleanup;
- the final decision memo selects wgpu or no candidate;
- CALayer remains skipped and software remains rejected;
- Smooth remains the production/default renderer;
- no production renderer integration has begun;
- the next plan is authorized only by the final decision memo and, when wgpu is selected, the concrete retained-companion pilot plan exists.
