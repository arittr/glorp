# Persistent Software Framebuffer Comparator Evidence

Date: 2026-07-10

Status: Phase C software comparator evidence complete. Verdict: **reject software as the current renderer backend candidate and skip final five-minute software ranking**, because three independent 720-logical ambient repetitions exceeded the mandatory 3 ms frame-CPU p95 budget by 92–97%.

## Decision boundary

This is decision-spike evidence, not production renderer integration. It compares the persistent Rust software framebuffer candidate against the corrected activated Smooth and wgpu candidates under the shared synthetic fixture. It does not independently select a shipping backend while energy, manual accessibility/input, candidate-enabled packaging, font/Unicode, and Darwin x86_64 qualification remain incomplete.

## Exact software binary and source state

The Task 10 functional matrix used one immutable optimized binary:

- path: `target/renderer-spikes/bin/glorp-software-7a13af1643f8aa316ea3b350e2ab207a46bf4aaf1d3941c29dd1b4b6b05db9b9`
- SHA-256: `7a13af1643f8aa316ea3b350e2ab207a46bf4aaf1d3941c29dd1b4b6b05db9b9`
- bytes: `7,585,648`
- profile/features: optimized release with `renderer-spike`, without `renderer-spike-wgpu`
- source tree: dirty by design because Phase C implementation and evidence changes are not yet committed; every run records the current commit and dirty flag in `environment.json`

The abbreviated comparison uses the corrected explicitly activated combined Smooth/wgpu binary at `target/renderer-spikes/bin/glorp-wgpu-spike-activated` and the frozen software binary above. Exact identities are recorded by every run's `binary.json`.

## Measured functional, fault, and capture facts

Artifact root: `target/renderer-spikes/software-functional/`

`functional-summary.json` reports `result: pass` across fourteen runs:

| Track | Logical size | Result |
| --- | ---: | --- |
| static | 360 | one completed native image submission; generation remained 1 |
| dynamic | 360 | all requested frames completed/submitted; zero atlas misses |
| ambient | 360 | 180 frames in 12 seconds; zero missed deadlines |
| active | 360 | 359 frames in 12 seconds; zero missed deadlines |
| resize | 360 | initial resource plus nine physical-size changes; generation 10 |
| occlusion | 360 | 60 seconds; 900 requested, 600 completed/submitted, one enter/exit; no hidden-interval submissions |
| capture | 360 | exact 720x720 physical PNG and metadata |
| ambient | 720 | 180 frames in 12 seconds; zero missed deadlines |
| active | 720 | 359 frames in 12 seconds; zero missed deadlines |
| resize | 720 | initial resource plus ten physical-size changes; generation 11 |
| capture | 720 | exact 1440x1440 physical PNG and metadata |
| callback panic | 360 | process exited nonzero with `reject-callback-panic` and complete evidence |
| capture timeout | 360 | process exited nonzero with `reject-injected-capture-timeout` and complete evidence |
| native resource unavailable | 360 | process exited nonzero with `reject-injected-native-resource-unavailable` and complete evidence |

All runs used the same software SHA-256, passed the hardened manifest byte/hash validator, recorded zero atlas misses and zero missed deadlines, passed privacy and cleanup checks, and left no renderer-spike process. Twenty-eight candidate stdout/stderr logs were preserved and separately privacy-scanned by the matrix runner.

## Observed native behavior

- A fixed physical size owns one persistent premultiplied RGBA Rust framebuffer, one `NSBitmapImageRep`, and one `NSImage` generation.
- The timer callback owns resolve, software rasterization, row copy into persistent native storage, metrics, and synchronous display submission.
- `drawRect` only draws the already prepared persistent image.
- Resource generations change only when the physical dimensions change.
- The software path copies one full physical RGBA image into native storage per visible frame.
- The settled occlusion interval generated no frame rows and no native image submissions.
- Direct review capture is encoded from the persistent Rust framebuffer as straight RGBA PNG. This replaced an earlier invalid AppKit bitmap capture that produced blank images; the blank captures are superseded and excluded.

## Persistent pixel storage

At fixed size the intrinsic pixel storage is:

| Logical / physical size | Rust framebuffer | Persistent native bitmap | Combined pixel storage | Per-visible-frame native copy |
| --- | ---: | ---: | ---: | ---: |
| 360 / 720x720 | 2,073,600 B | 2,073,600 B | 4,147,200 B | 2,073,600 B |
| 720 / 1440x1440 | 8,294,400 B | 8,294,400 B | 16,588,800 B | 8,294,400 B |

The duplicated storage is intentional: Rust owns deterministic raster state while AppKit owns stable native image storage. The evidence reports the copy honestly rather than treating it as a zero-upload renderer.

## Abbreviated feasibility measurements

Artifact root: `target/renderer-spikes/software-feasibility/`

Protocol:

- 36-second activated ambient process per repetition;
- 5-second warmup;
- 31 one-second `top` samples;
- three rotated 360 blocks in orders Smooth/wgpu/software, wgpu/software/Smooth, software/Smooth/wgpu;
- three 720 software repetitions;
- three-second cooldown;
- raw CPU and memory rows, frame JSONL, resource artifacts, stdout/stderr, and validator output retained;
- explicitly not final ranking evidence.

Aggregate artifact: `target/renderer-spikes/software-feasibility/feasibility-summary.json`

| Size/candidate | Run medians | Median of medians | Combined p95 | Frame CPU p95 by run | Peak sampled RSS |
| --- | --- | ---: | ---: | --- | ---: |
| 360 Smooth | 7.4%, 7.4%, 7.3% | 7.4% | 9.7% | 2.053, 2.309, 2.317 ms | 167,772,160 B |
| 360 wgpu | 0.9%, 1.0%, 1.0% | 1.0% | 1.1% | 0.380, 0.404, 0.377 ms | 206,569,472 B |
| 360 software | 1.3%, 1.2%, 1.2% | 1.2% | 1.7% | 1.550, 1.516, 1.488 ms | 37,748,736 B |
| 720 software | 4.1%, 4.5%, 4.55% | 4.5% | 5.8% | 5.807, 5.898, 5.752 ms | 69,206,016 B |

All run-median spreads were at most 10%, below the 20% divergence threshold. Every run completed visible frames, retained generation 1 with one native bitmap and image, recorded zero atlas misses and missed deadlines, and passed artifact validation. Software's 360 median/p95 and frame p95 passed the abbreviated absolute gates. Its 720 process CPU median also passed the 8% gate, but all three 720 frame-CPU p95 values exceeded the mandatory 3 ms gate. The repeated 5.752–5.898 ms result is structural enough to trigger the written early stop. Software was only 0.2 process-CPU percentage points above wgpu at 360, but it was roughly four times wgpu's frame-CPU p95 there.

No stack sample was required: software's 360 median-of-run-medians was 1.2% and combined p95 was 1.7%, below the 8%/12% stack thresholds.

## Memory and energy

Artifact: `target/renderer-spikes/software-memory/summary.json`

At 360, all three software runs held sampled RSS at exactly 37,748,736 bytes with zero positive post-warmup growth. At 720, sampled RSS peaked at 69,206,016 bytes; two runs were flat and one ended 7,340,032 bytes below its first post-warmup sample. Every run retained generation 1 with one native bitmap and one image. Fixed combined Rust/native pixel storage was 4,147,200 bytes at 360 and 16,588,800 bytes at 720. Over each 36-second 15 FPS run, the intrinsic full-image native copy totaled 1,119,744,000 bytes at 360 or 4,478,976,000 bytes at 720.

The same `powermetrics` method used for the earlier candidates remains unavailable without superuser access. The probe exited nonzero with `powermetrics must be invoked as the superuser`; evidence is retained under `target/renderer-spikes/software-energy/`. No privilege escalation was attempted. Energy is pending, not passed or failed.

## Build, executable, app, package, and delivery costs

Artifact roots:

- `target/renderer-spikes/software-build-costs/`
- `target/renderer-spikes/software-delivery-checks/`

Build artifact: `target/renderer-spikes/software-build-costs/summary.json`

The current Cargo topology has one shared `renderer-spike` feature containing both Smooth and software, so a distinct software-only executable cannot be measured without changing feature topology. The candidate-enabled shared build is reported honestly:

| Measure | Same-source comparator | Software/shared result | Delta | Gate | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| clean optimized build | prior same-commit Smooth 33.36 s | 34.64 s | +3.84% | +20% | pass, directional because the Smooth clean run is retained Phase B evidence |
| renderer-file incremental | current Smooth file 10.86 s | current software file 10.03 s | -7.64% | +25% | pass |
| optimized executable | prior same-commit Smooth 6,284,640 B | 7,585,648 B | +1,301,008 B / +1.24 MiB | +15 MiB | pass |
| current wgpu clean | shared software/Smooth 34.64 s | 37.44 s | +8.08% | informational | — |
| current wgpu executable | shared software/Smooth 7,585,648 B | 12,542,496 B | +4,956,848 B | informational | — |

The first incremental comparison against the historical 7.41-second Phase B Smooth number was invalid for the hard gate because it was not the same dirty source state. It is retained in the raw summary for audit and superseded by the current-source Smooth/software file-touch pair above.

Delivery artifact: `target/renderer-spikes/software-delivery-checks/summary.json`. The default no-spike shipping app built successfully, measured 7,076 KiB, and contained a 7,236,576-byte companion executable. `npm pack --dry-run --json --ignore-scripts ./npm/glorp` succeeded and reported a 2,934-byte packed / 6,466-byte unpacked wrapper package; it does not measure platform-binary publication impact. The current packaging script has no candidate-enabled renderer-spike app mode, so compressed candidate-app delta remains pending rather than passed. Darwin x86_64 execution is not claimed.

## First intended 2.5D capability analysis

The first explicitly listed Phase 6 renderer-native feature in the research brief is **bounded camera push/pull plus glass-rim parallax** (`retained-rust-renderer-design.md:1295-1305`). This analysis uses that concrete proposal rather than broader hypothetical depth-plane support.

### State split

- **Static:** the resolved habitat/pet/world image at its neutral camera scale; the authored glass-rim alpha/color raster; the circular aperture; the camera pivot; maximum push/pull bounds; and the rim's bounded parallax vector.
- **Dynamic:** semantic substitutions that invalidate the cached world image, such as a pet, prop, tank-life, or layout generation change.
- **Frame-varying:** one bounded camera scale/translation and one smaller glass-rim parallax translation. The HUD/perimeter remains screen-space and does not move with the world camera.

### Credible software representation

A software implementation can keep the existing ordered scene resolution and one AppKit submission, but camera push/pull changes the sampling position of most world pixels. The cheapest bounded approach is to cache a small set of full-world push/pull scale variants when semantic content changes, choose or interpolate between adjacent variants per frame, and composite a separately cached, tightly cropped glass-rim raster at its parallax offset. Integer or nearest-phase rim motion is a clipped premultiplied blit; visually smooth subpixel motion needs filtered sampling or a few cropped phase variants. AppKit still receives one final persistent image submission.

This does not require a second semantic scene architecture, but it does add a camera-resampling/compositing stage that the current comparator does not have.

### Pixel work, cache cost, and reraster triggers

- The physical surfaces contain 518,400 pixels at 720x720 and 2,073,600 pixels at 1440x1440.
- A conservative live implementation performs one full-world camera resample plus one full-surface-equivalent rim/source-over composite: at most 1,036,800 sampled source pixels per frame at 360 logical and 4,147,200 at 720 logical, before the existing final native copy. A cropped rim makes the second term smaller but does not remove the full-world camera resample.
- At 15 FPS, those conservative extra passes touch up to 15,552,000 source pixels per second at 360 logical and 62,208,000 at 720 logical.
- One cached full-world RGBA scale variant costs 2,073,600 bytes at 360 logical or 8,294,400 bytes at 720 logical. Five authored push/pull variants cost 10,368,000 bytes (9.89 MiB) or 41,472,000 bytes (39.55 MiB), plus a cropped rim raster. That cache is bounded but expensive at 720; fewer variants increase stepping or interpolation work.
- World variants rerasterize only when semantic content, layout, backing scale, aperture, or authored material changes. Camera motion selects/resamples variants every visible frame. Rim variants rerasterize only when the rim design/backing scale changes; their transformed composite is frame-varying.

### Bottleneck boundary and capability verdict

Software recreates the immediate-mode CPU bottleneck if smooth push/pull requires a fresh full-scene raster or filtered full-surface resample every frame, especially when combined with future lighting, particles, perspective warp, or independently transformed translucent layers. The measured base 1440x1440 comparator already records 5.752–5.898 ms frame-CPU p95 against a 3 ms gate before this camera/rim work is added. Therefore the feature is representable only as a bounded cache/composite experiment; it does not rescue software's production candidacy and would consume headroom that does not exist at 720.

### Remaining proof task

If software is retained as a reference path, a bounded proof may compare three versus five cached push/pull variants with a cropped four-phase rim raster at both sizes, measuring visual stepping, cache bytes, and incremental frame CPU. Do not implement a general software camera, perspective pipeline, or lighting system.

## Privacy, cleanup, accessibility, and input

Functional artifacts use only the synthetic fixture. Privacy scans pass and no process survives. The native software view installs one group plus three static-text values and records deterministic pointer projection, but manual Accessibility Inspector or VoiceOver traversal/value/hit-testing and real pointer-event audit remain pending for backend selection.

## Invalid and superseded evidence

- Early direct capture through `NSBitmapImageRep` produced blank black PNGs despite structurally valid manifests. Those images are invalid. Capture was moved to direct encoding from the persistent Rust framebuffer, and corrected 720x720/1440x1440 evidence was regenerated and validated.
- Initial ad hoc lifecycle runs predate the immutable Task 10 software binary. They remain useful implementation diagnostics but are excluded from functional or CPU ranking.
- Any earlier pre-activation wgpu matched data remains invalid as documented in the wgpu memo and is not reused here.
- Failed local attempts to generate `functional-summary.json` used incorrect field/operation names in the analysis script. They did not alter or rerun candidate evidence; the final summary was corrected against the raw artifacts and passes all assertions.

## Gate-by-gate verdict and engineering judgment

| Gate | Result | Evidence |
| --- | --- | --- |
| functional/lifecycle/fault/capture | pass | fourteen-run immutable-binary matrix |
| persistent resources and occlusion | pass | generation 1 at fixed size; zero hidden submissions |
| 360 abbreviated CPU | pass | 1.2% median of medians, 1.7% combined p95 |
| 360 frame CPU | pass | worst run p95 1.550 ms vs 2 ms gate |
| 720 abbreviated CPU | pass | 4.5% median of medians vs 8% gate |
| 720 frame CPU | **fail** | 5.752–5.898 ms p95 in all three runs vs 3 ms gate |
| missed frames/atlas misses | pass | zero in accepted runs |
| memory/resource stability | pass | zero positive sampled growth; stable generation 1 |
| clean build/executable | pass | +3.84%, +1.24 MiB |
| same-source incremental build | pass | software was 7.64% faster than Smooth file-touch rebuild |
| compressed candidate app | pending | packaging topology does not expose a spike-enabled app mode |
| energy | pending | `powermetrics` requires superuser |
| manual accessibility/input | pending | native machine-readable proof exists; manual audit does not |
| Darwin x86_64 execution | pending | not claimed |
| first 2.5D feature | bounded representation, selection fail | cached camera variants plus cropped rim parallax are possible, but base 720 rendering already has no frame-time headroom |

**Engineering judgment:** reject persistent software as the current backend candidate. It is functionally sound and excellent at 360, but the full-resolution 720 raster/copy path repeatedly consumes almost 6 ms at p95 before adding the first camera push/pull and glass-rim parallax work. That is a conclusive structural failure of the mandatory 3 ms gate. Per Task 11's early-stop rule, do not spend roughly 4.5 hours on final five-minute three-candidate software ranking blocks. Retain software as a deterministic reference/capture implementation or bounded fallback research asset, not as the selected production backend.

### Inferences

- The fourfold physical-pixel increase from 720x720 to 1440x1440 is consistent with the observed software frame-p95 increase from about 1.5 ms to about 5.8 ms. This is an inference from the measured scaling, not a proof that every cost is perfectly area-linear.
- The stable resource generations and flat post-warmup RSS make per-frame native allocation an unlikely explanation for the 720 failure. The full software raster plus 8,294,400-byte native copy is the leading bounded explanation.
- Adding full-world camera resampling or additional translucent passes would worsen, not repair, the failed 720 frame budget unless the primary algorithm or quality target changes.

## Ambiguity gate

**Phase D retained-CALayer work is skipped.** Software hit an explicit immediate stop condition by failing the mandatory 720 frame-CPU gate in all three abbreviated repetitions. That makes the software result unambiguous even though wgpu still has parallel qualification gaps. A four-layer CALayer experiment would not answer or reverse the demonstrated full-resolution software raster/copy limit, so it is not authorized.

This does **not** mean final wgpu selection is complete. Corrected wgpu remains the only renderer candidate that survived its measured kill-risk gates, but its final 720 CPU/energy, candidate-enabled package, manual accessibility/input, font/Unicode, and Darwin x86_64 qualifications remain open.

## Handoff to the final backend decision memo

The final decision memo belongs at:

`docs/superpowers/measurements/2026-07-10-glorp-renderer-decision.md`

Phase C hands off the following decision state:

1. **Persistent software framebuffer:** reject as the production backend because of the repeated 720 frame-CPU hard-gate failure. Retain the deterministic raster/capture implementation as evidence and a possible bounded reference tool only.
2. **Retained CALayer:** do not authorize Phase D; no unresolved comparison metric justifies the eight-hour experiment.
3. **wgpu:** sole surviving backend candidate, not yet the final selected backend.
4. **Smooth:** remains the shipping/default fallback until the final decision memo and production migration authorization.

Before the final decision memo selects wgpu, it must reconcile or explicitly approve exceptions for:

- the font license/source, bounded Unicode repertoire, replacement/non-BMP behavior, fallback, and metrics bake-off;
- a manual Accessibility Inspector or VoiceOver traversal/value/hit-testing/stale-child audit plus real pointer-event projection proof;
- the actual feature and release topology for all supported publish targets;
- a candidate-enabled Darwin app/package build and compressed-size comparison;
- energy evidence, or a reviewed exception documenting why unavailable energy does not block selection;
- Darwin x86_64 execution qualification or an explicit hardware exception;
- matched 720 wgpu CPU/frame qualification using the corrected activated protocol.

If those qualifications pass, the defensible final decision is wgpu. If a mandatory qualification fails without an approved exception, the final memo must select no candidate and name one bounded follow-up; it must not revive software or authorize CALayer by default.

## Raw artifact index

- frozen binary: `target/renderer-spikes/bin/software-functional-binary.json`
- functional matrix: `target/renderer-spikes/software-functional/`
- functional aggregate: `target/renderer-spikes/software-functional/functional-summary.json`
- abbreviated feasibility: `target/renderer-spikes/software-feasibility/`
- energy availability: `target/renderer-spikes/software-energy/`
- build costs: `target/renderer-spikes/software-build-costs/`
- delivery checks: `target/renderer-spikes/software-delivery-checks/`
- lifecycle diagnostics: `target/renderer-spikes/software-lifecycle-final/`

## Final Phase C verification

Completed on 2026-07-10:

- `cargo fmt --all -- --check`: pass.
- `cargo clippy --all-targets --all-features -- -D warnings`: pass.
- `cargo test --locked`: pass, including 922 library tests and all integration/doc tests.
- `cargo test --features renderer-spike --test renderer_spike -- --nocapture`: 10 passed.
- `cargo test --features renderer-spike --test renderer_spike_boundary -- --nocapture`: 1 passed.
- The plan names a `renderer_spike_software` integration target, but no such file exists; software raster/host unit coverage is compiled and run through the feature-enabled library/test suites rather than inventing an empty target.
- `cargo test -p xtask`: 12 passed.
- `cargo check --locked --no-default-features --all-targets`: pass with the two pre-existing Pixel dead-code warnings for `from_elapsed_ms` and `fallback_reference`.
- `npm test`: pass; Rust suite, eight release/package tests, and npm wrapper smoke completed.
- `git diff --check`: pass.
- Renderer-spike process cleanup: no surviving `renderer-spike-app` process.

No production renderer integration, default-renderer switch, CALayer prototype, or full software-renderer expansion was begun.
