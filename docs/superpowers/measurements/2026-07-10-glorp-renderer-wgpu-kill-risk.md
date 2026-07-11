# wgpu Kill-Risk Viability Report

Date: 2026-07-10
Scope: renderer decision spike Phase B only
Candidate: `wgpu` 30.0.0 over Metal in the existing AppKit host
Status: provisional evidence report; this does **not** select a production backend

## Verdict

**Conditional pass for kill-risk viability; not a backend-selection pass.**

The spike proves that a minimal `wgpu` renderer can coexist with Glorp's AppKit
application/window ownership, consume the canonical 300-primitive fixture, sustain the
requested cadence on the corrected visible-window run, resize at Retina scale, suspend
submissions while occluded, capture bounded nonblank images, contain injected failures,
and remain inside the provisional executable and build-time limits. The original matched
CPU dataset also exposed a real experiment defect: because an unbundled AppKit process was
not explicitly activated, two `wgpu` repetitions became `CAMetalLayer`-occluded and stopped
presenting. Those numbers are retained for audit but excluded from the viability decision.
The harness now calls `activateIgnoringOtherApps(true)` for both candidates, and the
corrected matched dataset is the ranking evidence cited below.

This remains conditional because the current prototype uploads all 300 instances
(`19,200` bytes) every requested visible frame, which fails the Phase B requirement of no
steady static uploads after warmup. Energy could not be measured without superuser access,
Darwin x86_64 could not be compiled with the installed Homebrew toolchain, the native
accessibility tree has machine-readable evidence but no Accessibility Inspector/VoiceOver
audit, and a candidate-enabled shipping app archive has not been assembled and compared.
Those gaps prevent production planning or backend selection. They do not establish a host,
capture, reliability, CPU, memory, or hard build/size blocker.

## Decision boundary

This report answers only whether `wgpu` survives the Phase B kill-risk spike. It does not:

- compare against the required persistent software framebuffer/atlas candidate;
- select a production renderer;
- approve the retained-renderer research brief as final architecture;
- approve the font/Unicode asset path;
- claim the final five-minute 360/720/energy ranking protocol is complete.

Per the spike specification, the next implementation phase is the bounded software
comparator. Stop here before implementing it.

## Environment and exact candidate

- Date: 2026-07-10
- Host adapter: Apple M5 Pro
- Graphics backend: Metal
- Device type: integrated GPU
- Surface: `Bgra8UnormSrgb`, FIFO present mode, automatic alpha
- Retina scale used by 360 and 720 functional tracks: `2.0`
- Dependency: `wgpu = 30.0.0`, `default-features = false`, features `std`, `metal`, `wgsl`
- Corrected matched binary: `target/renderer-spikes/bin/glorp-wgpu-spike-activated`
- Corrected binary SHA-256: `932d8719764b3015f37c8d1f18987dc6366ddd9be009bbe50372072ac97fe63c`
- Corrected binary bytes: `11,361,376`
- Build command: `cargo build --release --no-default-features --features renderer-spike-wgpu`

The earlier frozen evidence binary was
`target/renderer-spikes/bin/glorp-wgpu-spike`, SHA-256
`5644bf8a8bd9f1e6f5e6e094b8e271c2867db9c9c248058ac05e2384962fcf19`,
`11,361,168` bytes. Functional, fault, build, memory, and capture evidence was produced
with that renderer-equivalent binary. The corrected binary adds AppKit application
activation to the Smooth and `wgpu` harness launch paths; renderer pipeline and fixture
code are unchanged. Runtime CPU ranking uses only the corrected binary.

## Implemented spike surface

The feature-gated, non-default candidate includes:

- AppKit-owned `NSWindow`/`NSView` with a `CAMetalLayer`;
- main-thread adapter, device, surface configuration, acquire/encode/present, resize,
  capture polling, and close evidence;
- one instanced draw of the exact 300-primitive canonical fixture;
- one generated 32x32 shared temporary atlas, alpha blending, shapes, and aperture;
- 15 FPS ambient and 30 FPS active scheduling;
- logical-size/backing-scale reconfiguration;
- deterministic occlusion enter/exit behavior;
- aligned texture-to-buffer capture with bounded device polling and PNG output;
- one native accessibility group with three static-text children;
- guarded pointer projection and Objective-C callback boundaries;
- injected surface-unavailable, capture-timeout, and callback-panic outcomes;
- typed manifests, binary/environment identity, privacy scans, process cleanup, and
  host-boundary artifacts.

Primary implementation paths:

- `src/renderer_spike/wgpu.rs`
- `src/renderer_spike/shaders/fixture.wgsl`
- `src/renderer_spike/fixture.rs`
- `src/renderer_spike/artifacts.rs`
- `src/renderer_spike/macos.rs`
- `src/renderer_spike/mod.rs`
- `tests/renderer_spike.rs`
- `tests/renderer_spike_boundary.rs`

## Functional and fault matrix

Artifact root: `target/renderer-spikes/wgpu-functional-final`

| Track | Logical size | Samples | Result | Evidence |
| --- | ---: | ---: | --- | --- |
| static | 360 | 2 | pass | `static-360/` |
| dynamic | 360 | 12 | pass | `dynamic-360/` |
| resize | 360 | 24 | pass | `resize-360/` |
| occlusion | 360 | 12 | pass | `occlusion-360/` |
| capture | 360 | 8 | pass | `capture-360/` |
| capture | 720 | 8 | pass | `capture-720/` |
| ambient | 360 | 18 | pass | `ambient-360/` |
| active | 360 | 36 | pass | `active-360/` |
| resize | 720 | 24 | pass | `resize-720/` |
| injected capture timeout | 360 | bounded failure | pass | `fault-capture-timeout/` |
| injected surface unavailable | 360 | bounded failure | pass | `fault-surface/` |
| injected callback panic | 360 | bounded rejection | pass | `fault-callback/` |

All normal runs report `host-functional-pass`, pass privacy validation, and record
`process_exited = true` with no runner timeout. Injected outcomes are
`reject-injected-capture-timeout`, `reject-injected-surface-unavailable`, and
`reject-callback-panic`; the timeout exits nonzero while retaining validator-readable
artifacts, and callback unwind is caught at the native boundary.

## Capture/readback

| Logical size | Physical image | Aligned bytes/row | Map duration | Artifact |
| ---: | ---: | ---: | ---: | --- |
| 360 | 720x720 | 3,072 | 3,014 us | `target/renderer-spikes/wgpu-functional-final/capture-360/captures/capture-360-frame-000005.json` |
| 720 | 1440x1440 | 5,888 | 2,635 us | `target/renderer-spikes/wgpu-functional-final/capture-720/captures/capture-720-frame-000005.json` |

Both captures use top-left orientation and `rgba8-srgb-png`. Visual inspection of an
equivalent generated capture found a structured, nonblank frame containing the fixture's
atlas, shapes, and aperture. The capture timeout path is bounded and returns a static
rejection rather than hanging or retrying indefinitely.

## Occlusion and lifecycle

The dedicated functional occlusion track records four successful submissions before the
window is ordered out, no submissions during the hidden interval, then resumed submissions
after `makeKeyAndOrderFront`. Host-boundary artifacts record AppKit view creation, Metal
layer creation, surface creation/configuration, resize or acquire/encode/present,
occlusion enter/exit where applicable, capture polling where applicable, and close on the
main thread.

A benchmark defect was found during the first matched run. The unbundled process set an
activation policy and ordered its window front but did not activate the application. The
first two `wgpu` repetitions therefore recorded 404 and 140 `surface-occluded` acquisitions,
respectively; only the third was continuously visible. This made their low process CPU
partly attributable to absent presentation. The invalid dataset is retained at
`target/renderer-spikes/wgpu-matched-ambient-30s/` but is not used for the corrected CPU
conclusion. The corrected 12-second smoke at
`target/renderer-spikes/wgpu-activation-smoke/` completed 179 of 180 requested frames, with
one initial surface-occluded acquisition and no later occlusion.

## Corrected matched CPU evidence

Protocol and raw evidence:

- runner: `target/renderer-spikes/run_matched_cpu.py`
- output: `target/renderer-spikes/wgpu-matched-ambient-30s-activated/`
- candidate order: Smooth/wgpu, wgpu/Smooth, Smooth/wgpu
- duration: 36 seconds each
- warmup: 5 seconds
- requested `top` samples: 31 at one-second intervals
- cooldown: 5 seconds between runs
- fixture: ambient, 360 logical / 720 physical, 15 FPS

The aggregate result and per-block records are in
`target/renderer-spikes/wgpu-matched-ambient-30s-activated/matched-summary.json`.

| Candidate | Block medians | Median of run medians | Combined median | Combined p95 | Combined max |
| --- | --- | ---: | ---: | ---: | ---: |
| Smooth | 7.6%, 9.1%, 8.8% | 8.8% | 8.0% | 11.7% | 12.3% |
| wgpu | 1.1%, 1.1%, 1.0% | 1.1% | 1.1% | 1.3% | 1.4% |

Each candidate retained 80 one-second samples across the three blocks. `wgpu` reduced the
median of run medians from `8.8%` to `1.1%`, an **87.5% reduction**. Its run-median spread
was `9.09%`, versus `17.05%` for Smooth. Every corrected wgpu repetition presented 539 of
540 requested frames; each recorded exactly one initial `surface-occluded` acquisition and
then 539 continuous successful presents. The result clears the Phase B 25% kill threshold
by a wide margin and also falls below the later 360 process-CPU absolute budgets (`5%`
median and `8%` p95), though those budgets cannot select a backend without the rest of the
candidate and mandatory-gate evidence.

The Phase B kill threshold is at least a 25% reduction in matched median process CPU. The
corrected evidence is evaluated against that threshold only; it is not a final backend
ranking because the software comparator, 720 CPU, and energy evidence do not exist yet.

## Invalid matched dataset retained for audit

The pre-activation run is
`target/renderer-spikes/wgpu-matched-ambient-30s/matched-summary.json`. Its computed
median-of-run-medians was Smooth `9.3%` and wgpu `0.9%`, nominally a `90.32%` reduction,
but two wgpu runs presented only 136/540 and 400/540 frames. Host evidence records 404 and
140 `surface-occluded` acquisitions. Those values must not be used to claim performance.
This defect was fixed rather than hidden or averaged away.

The invalid Smooth p95 exceeded the stack threshold, so bounded stack evidence was also
captured at
`target/renderer-spikes/wgpu-matched-ambient-30s/stack-smooth/stack-sample.txt`
(2,554 lines). It remains useful diagnostic evidence, but the corrected matched run is the
CPU decision dataset.

## Frame CPU and resource-update behavior

| Candidate | Per-run frame CPU median | Per-run frame CPU p95 | Combined median | Combined p95 | Combined max | Visible completion |
| --- | --- | --- | ---: | ---: | ---: | --- |
| Smooth | 0.979, 0.941, 0.939 ms | 2.366, 2.334, 2.340 ms | 0.958 ms | 2.352 ms | 3.751 ms | harness records one native draw per requested tick |
| wgpu | 0.270, 0.254, 0.265 ms | 0.350, 0.429, 0.409 ms | 0.264 ms | 0.410 ms | 1.741 ms | 539/540 in every run |

Corrected frame aggregates are saved at
`target/renderer-spikes/wgpu-matched-ambient-30s-activated/frame-aggregate.json`. No wgpu
run increments `missed_deadlines`. The combined wgpu p95 of `0.410 ms` is well below the
later 360 frame budget of `2 ms`; Smooth's matched combined p95 is `2.352 ms`. Each wgpu
run has one initial `surface-occluded` acquisition before the activated window becomes
drawable, followed by 539 successful presents.

The prototype records zero atlas misses and one initial static rebuild per wgpu run.
However, it writes the complete 300-instance buffer on every requested visible frame:
`19,200` bytes per metric row. This is a known failure of the Phase B feasibility wording
requiring no steady static uploads after warmup. It is not disguised as dynamic-only
traffic because the implementation currently rebuilds the whole resolved instance array.
A bounded production-planning follow-up would need to split retained/static instance data
from the genuinely time-varying subset and prove zero unchanged uploads after warmup.

## Whole-process memory snapshot

Artifact: `target/renderer-spikes/memory-snapshot/summary.json`

| Candidate | Settled RSS samples (KiB) | Median RSS | Maximum RSS |
| --- | --- | ---: | ---: |
| Smooth | 108256, 108160, 108160, 108176, 104096 | 108,160 KiB | 108,256 KiB |
| wgpu | 103424, 103392, 103408, 103424, 103440 | 103,424 KiB | 103,440 KiB |

This short five-sample snapshot shows no immediate wgpu memory penalty and no visible
post-settle growth. It is not a long-duration leak proof. The `top` memory strings in the
invalid matched dataset behave differently and are not treated as RSS evidence; the
explicit `ps` RSS snapshot above is the memory comparison.

## Build, executable, dependency, and shipping topology

Artifact root: `target/renderer-spikes/build-costs`

| Measure | Smooth | wgpu | Delta | Provisional limit | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| clean optimized build, real | 33.36 s | 35.69 s | +6.98% | +20% | pass |
| renderer-edit incremental build, real | 7.41 s | 7.52 s | +1.48% | +25% | pass |
| optimized executable | 6,284,640 B | 11,361,168 B | +5,076,528 B / +4.84 MiB | +15 MiB | pass |

The clean Smooth build waited on the shared package cache, so build-time comparisons are
directional despite using separate owned `CARGO_TARGET_DIR` values. Raw timing logs and
maximum-resident build process values are retained under the build-cost root.

`target/renderer-spikes/delivery-checks/dependency-tree.txt` records the exact dependency
resolution. The top-level `wgpu` configuration enables only `metal`, `std`, and `wgsl`;
default cross-platform backends are disabled for this macOS-only feature.

The normal shipping package smoke command
`node scripts/build-macos-companion-app.mjs --profile release` exited `0` and produced
`target/macos/Glorp.app` without enabling the spike. This proves the default shipping path
remains unaffected; it does not prove the candidate-enabled app/package size. The packaged
companion executable was `7,236,576` bytes and the app bundle measured 7,076 KiB in this
smoke.

## Feature, lint, and test checks

Completed checks:

- `cargo fmt --check`
- `cargo test --features renderer-spike-wgpu --test renderer_spike --test renderer_spike_boundary`
  - 8 renderer-spike tests passed
  - 1 DTO boundary test passed
- `cargo check --no-default-features --all-targets`
  - passed with two pre-existing Pixel dead-code warnings
- `cargo clippy --all-targets --all-features -- -D warnings`
- `npm test`
- `git diff --check`

The no-default spike release build succeeds on the current Apple Silicon host. Strict
all-feature clippy completed successfully in `4.98 s`. `npm test` exited `0`: the workflow
ran all `922` Rust library tests, release-version/package checks, and the npm workspace
smoke tests. The focused renderer tests also passed (`8 + 1`), and `git diff --check`
reported no whitespace errors.

## Accessibility and input

Machine-readable artifacts contain one habitat/group element and three static-text values,
with no per-glyph children. The native view installs those elements and updates their
frames when resized. Pointer coordinates are projected from AppKit view coordinates into
logical fixture coordinates inside a guarded callback.

This is implementation and artifact evidence, not the required manual accessibility audit.
No Accessibility Inspector or VoiceOver focus traversal/value/hit-testing/stale-child
procedure was recorded, and no synthetic pointer event artifact proves the projection path
at runtime. Accessibility/input therefore remains pending for backend selection.

## Energy

The attempted `powermetrics` probe exited nonzero with:

```text
powermetrics must be invoked as the superuser
```

Evidence:

- `target/renderer-spikes/energy-check/status.txt`
- `target/renderer-spikes/energy-check/powermetrics.stderr.txt`
- `target/renderer-spikes/energy-check/powermetrics.stdout.txt`

No `sudo` escalation was attempted. Energy is unavailable, not passed and not failed. The
candidate cannot satisfy the final energy gate on this evidence.

## Darwin x86_64

The attempted `x86_64-apple-darwin` check exited `101` because the target standard library
is not installed. The active Homebrew toolchain has no `rustup`, so the target could not be
added in-session:

```text
error[E0463]: can't find crate for `core`
= note: the `x86_64-apple-darwin` target may not be installed
/bin/bash: rustup: command not found
```

Evidence:

- `target/renderer-spikes/delivery-checks/x86-check.status.txt`
- `target/renderer-spikes/delivery-checks/x86-check.stderr.txt`
- `target/renderer-spikes/delivery-checks/rustup-x86.stderr.txt`

This is a toolchain limitation, not evidence that the code fails on Intel macOS. Darwin
x86_64 disposition remains pending and blocks backend selection.

## Privacy and cleanup

Every accepted functional, fault, memory, and matched run was validated by the owned xtask
validator. Normal and injected artifacts use the synthetic canonical fixture, sanitized
callback categories, relative command paths, binary hashes, and machine/environment fields
without user prompts, transcripts, source names, usage rows, or secrets. Process cleanup
artifacts report a normal exit and no timeout for accepted normal runs; injected timeout
and callback cases produce the intended bounded rejection. No renderer-spike processes
remain after the checks.

One early matched attempt was rejected because the run command stored an absolute
`/Users/...` path. That output was removed and rerun with a relative binary path. One later
functional attempt used `target/release/glorp` after the package smoke had intentionally
overwritten it with the default non-spike binary; that attempt was discarded and repeated
using an immutable frozen evidence binary. Neither invalid attempt contributes measurements.

## Gate assessment

| Gate | Result | Evidence / reason |
| --- | --- | --- |
| Existing AppKit lifecycle ownership | pass | CAMetalLayer-backed view opens, resizes, presents, captures, closes on AppKit main thread |
| Canonical deterministic fixture | pass | exact 300 primitives and shared atlas in typed artifacts |
| Bounded nonblank capture | pass | valid 720x720 and 1440x1440 PNGs; bounded map timings |
| Fault/callback boundary | pass | surface, timeout, and callback panic become bounded static rejection outcomes |
| Occlusion suspension | pass | dedicated track records no submissions while hidden after enter |
| Material 360 CPU reduction | pass | corrected matched median-of-run-medians 8.8% → 1.1%, an 87.5% reduction versus the 25% Phase B kill threshold |
| Frame cadence capability | pass | every corrected matched wgpu run presented 539/540 frames; combined frame CPU p95 0.410 ms; zero missed-deadline increments |
| Atlas misses after warmup | pass | zero |
| Static uploads after warmup | **fail in prototype** | full 19,200-byte instance upload every visible request |
| Bounded memory | provisional pass | short settled RSS snapshot is bounded and slightly below Smooth |
| Executable hard limit | pass | +4.84 MiB versus +15 MiB limit |
| Clean/incremental build hard limits | pass | +6.98% / +1.48% |
| Default shipping topology | pass | package smoke exits 0 without spike feature |
| Candidate-enabled app/package | pending | not assembled or size-compared |
| All-feature local CI compilation | pass | strict clippy/test compilation on Apple Silicon |
| Darwin x86_64 | pending | target absent; no rustup available |
| Native accessibility/input audit | pending | implementation/tree exists; manual audit and pointer event proof absent |
| Energy no worse than Smooth | pending | `powermetrics` requires superuser |
| Approved font path | pending parallel track | this spike uses the temporary generated atlas only |

## Engineering judgment and next step

`wgpu` survives the host, capture, failure-boundary, CPU, and hard-cost kill risks. Its
corrected matched median improvement is 87.5%, so it is not rejected for insufficient
material benefit. It also should not be selected: the software comparator has not run, the
prototype violates the no-steady-upload feasibility wording, and energy, x86_64,
candidate-enabled packaging, manual accessibility/input, font, and 720 CPU qualification
remain unresolved.

The decision-spike program's next bounded step is therefore:

1. freeze this wgpu evidence and make no production renderer/API expansion;
2. run the planned 16-person-hour persistent Rust software framebuffer/atlas comparator
   against the same fixture and corrected activation protocol;
3. after the comparator, perform the common matched ranking only for viable candidates;
4. if wgpu remains competitive, allow one bounded wgpu retention correction before
   production planning: split static/dynamic instance uploads and prove zero unchanged
   uploads after warmup, while also closing the single grouped qualification package
   (energy, x86_64 disposition, candidate-enabled app size, accessibility/input audit, and
   720 CPU). These are qualification items for one candidate, not permission to build a
   production scene architecture.

No retained CALayer experiment is authorized by this report. That experiment remains
conditional on ambiguity after the software comparator.
