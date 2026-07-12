# Task 4 Report: Scene Revision and Activation Runtime

Status: DONE

Baseline: `f72080652b976770aa03643357764d67dfd3213a`

## Outcome

Added the renderer-neutral, Glorp-specific runtime in
`src/presentation/companion_scene/runtime.rs` and wired its public evidence
identities through `companion_scene/mod.rs`.

The runtime now owns:

- separate `SceneGenerationKey { device, layout, resources }` and
  `SurfaceEpoch` identity in `SceneVersion`;
- transactional layout, semantic, frame, resource, request, device, surface,
  and activation-attempt counters using checked increments;
- leaf-field snapshot validation/classification with fixed 32-byte change masks;
- one retained active generation, one newest desired pending generation, and at
  most one running/cancelling worker;
- exact request/key/revision candidate acceptance and exact typed activation;
- an owned `AcceptedGenerationCandidate` containing Task 3's
  `AcceptedSceneState`, with no raw-template constructor;
- borrow-scoped capture leases tied to one exact active `SceneVersion`;
- hidden latest-only snapshot coalescing and reveal-once reconciliation;
- fail-closed fixed-capacity, schema, privacy, finite-value, and identity checks.

No renderer, GPU, host, AppKit, TUI, Smooth, Task 5 scene construction, generic
engine, generic scheduler, or compatibility abstraction was added.

## Binding architecture corrections

- Task 4 emits `SnapshotChangeSet`, revisions, and
  `Arc<CompanionSceneSnapshot>` only. It does not emit `ContentDelta` or
  `FrameDelta`; Task 5 owns semantic-slot/node projection.
- Snapshot nesting is not treated as lifetime. Prop motion is frame state while
  sprite/twinkle/lid are semantic. Tank glyph/variant/morph are semantic while
  visibility/origin/side/layer/cell placement/bounds are frame state.
- `elapsed_ms`, tank `cadence_ms`, and tank `calm` are ignored when their
  already-derived render fields do not change.
- Layout/resource, semantic, and frame changes can coexist; each affected
  reconciler counter advances exactly once and the generation request carries
  the complete resulting source revisions.
- Surface rebind updates the presentation version without changing layout or
  resource generation. Device recreation invalidates the old active unit and
  queues a replacement generation under the new device epoch.
- Preparing, Ready, and Activating remain pending-slot phases. The active slot
  stays structurally separate until exact clean presentation commits.
- A cancelling worker completion and a separate cancellation acknowledgement
  both release the only worker slot and start only the newest queued request.
- Hidden state blocks activation, replaces one validated latest snapshot, and
  performs no reconciliation/revision/request work until reveal.

## Task 5 additive seam

The fixed named masks are crate-visible for additive Task 5 mapping:

- layout/resources: logical extent, pet topology/art, room authored resources,
  prop cast/resources, tank cast/resources, reserved ambient/material families;
- semantic: pet art, palette, mood/weather, props, tanks, ambient, HUD;
- frame: camera, pet transform, prop transforms, tank instances, ambient
  instances, status visibility, gauges, dim, lights.

`GenerationRequest` exposes crate-private accessors for request ID, scene key,
surface, complete source revisions, and the newest shared snapshot.

The following are deliberately not inferred in Task 4 and remain explicit Task
5 snapshot/contract additions: ambient semantic/frame slots, resolved point-space
prop base placement, bloom state, fixed empty prop/tank content encodings,
palette storage in `SceneContent`, instance frame mirrors, and active light data.
They must come from renderer-neutral authored/domain data, never TUI output,
Smooth output, or a pet seed.

## Snapshot lifetime repair

No snapshot struct was modified. The existing mixed `TankAnimationSnapshot` and
`FrameSnapshot::hud_lines` nesting can be classified correctly at leaf level, so
a physical split would have been unrelated churn. Tests pin the corrected
semantic/frame interpretation.

## TDD evidence

Initial RED:

- `cargo test --lib presentation::companion_scene::runtime`
- Exit 101 with 68 expected missing-runtime errors, including
  `SnapshotChangeSet`, `CompanionSceneReconciler`, generation/revision identities,
  runtime state, candidate proof, activation outcomes, capture lease, and worker
  transitions.

Focused GREEN:

- `cargo test --lib presentation::companion_scene::runtime`
- 31 passed, 0 failed.

Additional RED/GREEN slices pinned:

- cancelling-worker completion auto-starting the newest queued request;
- named Task 5 mask families and accessors;
- surface-unavailable activation after device recreation;
- hidden Ready/Activating deferral;
- immutable worker-source versus mutable desired-state rebasing;
- resource-only generation without layout advancement;
- stale activation attempts that remain failure-actionable but commit-ineligible;
- topology supersession with capture deferral and one newest request;
- old-surface and retired-device fatal scoping;
- unusable active/capture state after delayed GPU failure;
- delayed current-device errors cancelling Preparing and Activating work.

## Test coverage

The 31 focused tests cover:

- every current snapshot leaf and every mixed lifetime;
- transactional invalid schema, renderer schema, privacy, non-finite,
  inconsistent identity, and capacity rejection;
- counter starts/ownership/overflow;
- mixed revision advancement;
- surface/device separation;
- rapid topology storms and both cancellation completion orders;
- Preparing/Ready compatible revision rebasing;
- exact candidate proof and owned validation state;
- every acquire deferral, candidate rejection, and epoch failure family;
- exact activation attempt/key/surface/revision commit conditions;
- immediate/delayed GPU error behavior;
- capture lease identity and activation deferral;
- hidden coalescing/reveal, shutdown races, and fixed-capacity no-loop behavior;
- `Arc` snapshot sharing and the fixed-size, allocation-free change set.

## Size and scope

`runtime.rs` is approximately 1,517 production lines plus 1,271 in-module test
lines. The production code is intentionally kept in one initial ownership module
per the accepted design. Its types are closed and companion-specific; the size is
driven by explicit validation, exact state transitions, and exhaustive typed
failure policy rather than reusable engine machinery.

## Verification

Fresh final gate after the review/fix loop:

- `cargo test --lib presentation::companion_scene::runtime`: 31 passed;
- `cargo test --lib presentation::companion_scene`: 83 passed;
- `cargo test --lib --features retained-renderer presentation::companion_scene`:
  83 passed;
- `cargo test --test companion_scene_boundary`: 10 passed;
- `cargo clippy --all-targets --all-features -- -D warnings`: clean;
- `cargo fmt --check`: clean;
- `git diff --check`: clean.

## Independent review

The independent precommit reviewer found several Important lifecycle and identity
issues across repeated passes: stranded activation after compatible updates,
mutable in-flight worker source identity, resource-only layout churn, lost late
fatal outcomes, and incorrectly scoped/retained surface and device errors. Each
actionable issue was reproduced with a failing test and fixed.

Final verdict:

- Spec compliance: **PASS**
- Code quality: **APPROVED**
- Remaining Critical/Important findings: none.

## Files

- `src/presentation/companion_scene/runtime.rs`
- `src/presentation/companion_scene/mod.rs`
- `.superpowers/sdd/task-4-report.md`

This report path was already tracked and is updated as part of the Task 4 commit.
