# Glorp Direct Retained Runtime — Task 10 Qualification

**Task:** 10 — direct runtime qualification  
**Date:** 2026-07-14/15 UTC  
**Branch:** `main`  
**HEAD:** `7379755ba2dc07b49d60e17335bcf4eba95a76cf`  
**Checkout:** intentionally dirty; the exact status, tracked binary patch, untracked-file hashes, environment, and executable hashes are preserved under `target/glorp-scene-gates/task10-identity/`.  
**Verdict:** **Task 10 PASS for the automated and feasible native direct-runtime qualification performed here.** This is not Gate D/default-cutover approval and not full Gate C visual/accessibility approval; the unresolved limitations are explicit below.

## Scope and interpretation

This run qualifies the explicit Apple-Silicon direct route:

```text
--renderer retained --retained-scene-runtime live
route=direct-retained-scene
```

It does not approve the Auto flip, the four-hour canary, or deletion of the legacy retained translator. Those remain Tasks 11 and 12. A transparent AppKit screenshot is diagnostic only; every passing direct gate below required the GPU-native `scene.png` and receipt artifacts.

## Qualification host

Recorded at `2026-07-15T03:02:29Z`:

- macOS `26.5.2` (`25F84`), arm64
- model `Mac17,9`, Apple M5 Pro, 68,719,476,736 bytes RAM
- `rustc 1.97.0 (2d8144b78 2026-07-07)`, host `aarch64-apple-darwin`, LLVM 22.1.6
- `cargo 1.97.0 (c980f4866 2026-06-30)`
- Node `v24.14.0`; npm `11.14.0`
- logged-in macOS GUI session with native AppKit and Metal available

Canonical environment record: `target/glorp-scene-gates/task10-identity/environment.txt`.

## Commands

### Final isolated automated suite

No other Cargo command ran concurrently with this suite. This matters because `assert_cmd::cargo_bin` feature-variant tests share `target/debug/glorp`.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features retained-renderer
cargo test --features 'dev-preview retained-renderer' --test dev_preview
cargo test --features retained-renderer --test round_scene
cargo test --features retained-renderer --test smooth_companion
cargo test --features retained-renderer --test companion_draw_boundary
cargo test --features retained-renderer --test retained_renderer_boundary
cargo test -p xtask
node --test scripts/test/macos-app-packaging.test.mjs
npm test
git diff --check
```

Result: all commands passed, UTC `2026-07-15T03:31:56Z`–`03:41:32Z`.

- Log: `target/glorp-scene-gates/task10-logs/automated-gates-final-serial.log`
- SHA-256: `8b69d0b944c7847d5fe4690ed5c2abc594e3cda868f850b79ed63a1a53d878e8`
- Full retained-feature library result within the run: 1,471 passed, 0 failed.
- Final `retained_renderer_boundary`: 22 passed, 0 failed.
- Final `retained_scene`: 9 passed, 0 failed.
- Packaging/release Node suite at the end of `npm test`: 11 passed, 0 failed.

A prior automated run was stopped and is not qualification evidence: a concurrent focused retained-feature Cargo test replaced the no-feature `target/debug/glorp` while `npm test` was executing `companion_rejects_retained_renderer_without_feature`. That caused the test to launch the GUI rather than fail parsing. This was runner contamination, not a product defect. The isolated serial rerun above passed the same no-feature test and the entire suite.

### Exact-path native smoke

```bash
cargo build --release --features retained-renderer
target/release/glorp init \
  --seed glorp-scene-native-short-boundary-proof-v1 \
  --name SceneGate --yes
target/release/glorp companion-app \
  --renderer retained \
  --retained-scene-runtime live \
  --review-size 360x360 \
  --review-duration-ms 30000 \
  --review-capture-dir target/glorp-scene-gates/task10-native-short-boundary-proof \
  --review-runtime-metrics-out target/glorp-scene-gates/task10-native-short-boundary-proof/scene-metrics.json
```

Result: passed on native AppKit/Metal; 898 presented frames, no fallback, GPU-native nonblank capture, external-redacted receipt.

- Artifact root: `target/glorp-scene-gates/task10-native-short-boundary-proof`
- `scene.png` SHA-256: `bf7f82fa5aa6be0a50fa9d7a2a7fa4e49c565d5a055b8984bb88b8f82c49c566`
- `scene-manifest.json` SHA-256: `f720354e1b97113f94031690900d14a2d6733c9f4f777b2484fc9946842992d9`
- Route: `direct-retained-scene`
- Logical/physical/scale: 360×360 points / 720×720 pixels / 2×
- Last-present age: 139 ms
- Capture checksum recorded by manifest: `f6e205fe9b4d9870a3b7f02a637666924396d5a17c2dc41671b95cf9b6d86bd7`
- Receipt: device/layout/resources `1/1/2`, surface epoch 1, semantic/frame `33/899`, HUD revision 898

### Frozen lifetime gate

```bash
cargo xtask companion scene-lifetime \
  --frames 4500 \
  --out target/glorp-scene-gates/task10-lifetime-final-mailbox
```

The command builds release with `retained-renderer`, initializes an isolated config, launches the explicit direct route, executes the frozen dual-cadence offscreen Metal protocol, performs exactly one terminal direct capture, validates all artifacts, and exits nonzero on any failed gate.

Result: passed; job `job_01KXHVJJ6N20SXWZPYF1SCC1ZX`, exit 0, 49.991 s.

- Artifact root: `target/glorp-scene-gates/task10-lifetime-final-mailbox`
- Log: `target/glorp-scene-gates/task10-logs/scene-lifetime-final-mailbox.log`
- `scene-metrics.json` SHA-256: `0adbae52cbc2870693ae47add9a96e1e59036420fccfbdb38111002b2d3e6094`
- `scene.png` SHA-256: `465989f08949dae7c27792e9663861da709b836c96732d76f20a891aaf600333`
- `scene-manifest.json` SHA-256: `f1a65d49edc49a10d7fad7ab9b56405991510a62009bf788feee2bd835117b34`
- Executable SHA-256 at this run: `319c860c9513a092b57aa5cbf4b3c83c3a4dd090727369be49ffbecce89b3ce5`

Protocol and results:

| Measure | Result |
|---|---:|
| measured semantic samples | 4,500 |
| warmup semantic samples | 4,500 |
| measured presentation ticks | 33,750 |
| warmup presentation ticks | 33,750 |
| virtual measured duration | 1,125,000 ms |
| measured frame projections/reconciles | 33,750 / 33,750 |
| measured encodes/submits | 33,750 / 33,750 |
| measured draw calls | 506,250 |
| bounded GPU polls | 67,502 |
| encode/submit work per second | 30 / 30 |
| capacity growth/stale mutation/rejection/regeneration | 0 / 0 / 0 / 0 |
| post-warmup resource creations/static upload bytes | 0 / 0 |
| direct target/readback prewarmed and reused | yes / yes |
| terminal direct capture attempt/success/nonblank | 1 / 1 / 1 |
| fallback count | 0 |

Performance/resource result:

- UI tick p95/p99: 449/500 µs (limits 1,422/2,070)
- encode p95: 4 µs (limit 282)
- generation-service UI max: 18 µs (limit 4,000)
- materialize/upload/publish max: 2,606 µs (limit 16,000)
- activation render-owner max: 2,957 µs (limit 16,000)
- main-thread raster calls: 0
- RSS warmup/high/final/peak: 145,899,520 / 145,915,904 / 146,046,976 / 146,046,976 bytes; under the 1% rule
- accounted GPU warmup/peak/final: 12,718,080 / 12,718,080 / 12,718,080 bytes

This artifact ran after the final production mailbox-drain correction. The later change to `tests/retained_scene.rs` was test-only and did not change product source.

### Native fault soak

```bash
cargo xtask companion scene-fault-soak \
  --out target/glorp-scene-gates/task10-fault-soak-final-mailbox
```

Result: all 11 cases passed; job `job_01KXHVJQ209255B0C19EZTVPF8`, exit 0, 93.521 s.

- Artifact root: `target/glorp-scene-gates/task10-fault-soak-final-mailbox`
- Log: `target/glorp-scene-gates/task10-logs/scene-fault-soak-final-mailbox.log`
- `fault-soak.json` SHA-256: `61a24dbe82bfb0ea356074adc9fac05cf5a9b065a6af137661d6148ac4081328`
- Build profile/features: release / all features

Every case matched expected category, process disposition, and sanitized evidence:

| Injection | Expected/observed category | Process | Evidence |
|---|---|---|---|
| initialization | retained-device-unavailable | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| surface-loss | retained-surface-lost | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| validation | retained-device-validation | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| internal | retained-device-internal | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| out-of-memory | retained-device-out-of-memory | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| device-loss | retained-device-unavailable | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| resource-failure | retained-atlas-unavailable | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| unsupported-raster | retained-unsupported-raster | expected success | acknowledged nonblank Smooth fallback; no direct artifacts |
| map-failure | retained-capture-map-failed | expected failure | exactly one failed direct capture, no fallback, no direct artifacts |
| blank-capture | retained-capture-buffer-too-short | expected failure | exactly one failed direct capture, no fallback, no direct artifacts |
| write-failure | retained-capture-write-failed | expected failure | exactly one failed direct capture, no fallback, no direct artifacts |

The fallback cases require a decoded nonblank AppKit diagnostic screenshot and an acknowledged real Smooth paint. Capture-only failures truthfully retain structurally valid but blank diagnostic screenshots and cannot pass as direct evidence.

### Final exact-tree five-minute native gate

```bash
cargo xtask companion scene-native-smoke \
  --duration-ms 300000 \
  --out target/glorp-scene-gates/task10-native-five-minute-final-mailbox
```

Result: passed; job `job_01KXHWR96WPP5KHAG54A6J62FX`, exit 0. The command spent 2m40s waiting for the Cargo lock, then completed its release build and 300-second process. Total job duration was 509.472 s.

- Artifact root: `target/glorp-scene-gates/task10-native-five-minute-final-mailbox`
- Log: `target/glorp-scene-gates/task10-logs/scene-native-five-minute-final-mailbox.log`
- Executable SHA-256: `59f436a7cd4a42b7a8b5d0960528e28ee7d118fd4ea685da41db32cd9d84ae7e`
- `native-samples.json` SHA-256: `42b0719a6775f8d0444e5d07e10958405b01e75a5771115c5587a1151bf94b2e`
- `scene-metrics.json` SHA-256: `2abe43321f9c1d00cd781410c43a8d5a2af46d62ac4f193c1bbdfea5bc151151`
- `scene-manifest.json` SHA-256: `17d696ce89742e4cbd8697fe45902190c5998a7c1c99f5fa61e84435bc1a84e8`
- `scene.png` SHA-256: `ba0a4dce16792b1e9b61b1a6fc98f78b66a943e81b1268031560b8d9059e6480`

Route/capture result:

- requested/effective renderer: retained/retained
- route: `direct-retained-scene`
- 8,980 present attempts, surface acquires, and successful presents; no skipped/fallback frames reported by the gate
- 8,998 presented-frame receipt count; last-present age 134 ms
- external-redacted nonblank GPU-native capture, checksum `3716b0940e15ad4c9f9cb55a4f393ddae1f09d819759daf33c76f835a7a619aa`
- terminal capture attempt/success/failure/nonblank: 1/1/0/1
- receipt: device/layout/resources `1/1/2`, surface 1, semantic/frame `451/9000`, HUD revision 8,998

Performance/resource result:

| Gate | Result | Limit |
|---|---:|---:|
| UI tick p95 | 1,274 µs | ≤1,422 µs |
| UI tick p99 | 1,420 µs | ≤2,070 µs |
| encode p95 | 36 µs | ≤282 µs |
| generation-service UI max | 21 µs | ≤4,000 µs |
| materialize/upload/publish max | 2,548 µs | ≤16,000 µs |
| activation render-owner max | 4,402 µs | ≤16,000 µs |
| longest visible no-present gap | 40 ms | bounded/pass |
| main-thread raster calls | 0 | 0 |
| post-warmup persistent GPU creations | 0 | 0 |
| post-warmup static upload bytes | 0 | 0 |
| fallback count | 0 | 0 |

The 32 samples include the terminal sample. First-20%-window RSS high was 134,709,248 bytes; post-warmup high was 134,807,552 bytes; 1% limit was 136,056,340 bytes: pass. Accounted GPU bytes stayed 4,285,440 current/peak with seven current/peak persistent objects.

## Packaging, capability, accessibility, and lifecycle coverage

The final serial suite passed:

- arm64 retained vs x64 Smooth-only package topology;
- staged capability smoke requirements for `effective-scene-route=direct` and GPU-native scene evidence;
- typed native scene command parsing and fail-closed artifact validation;
- deterministic resize, resize-storm, backing-scale, hide/reveal, capture-swap, surface outcome, worker failure, watchdog, common run-loop mode, shutdown, and non-reentrant tick harnesses;
- GPU-native offscreen output, prop/tank ROI, privacy projection, receipt binding, and blank rejection tests;
- source/dependency boundaries for AppKit surface ownership, direct capture routing, and retained deletion inventory;
- automated accessibility/input boundary assertions for role/label/value/bounds preservation and retained-view transitions.

The test-only stale-boundary repairs were independently audited twice. The first review found that the updated resize test checked helper presence but not ordering. It was strengthened to assert `resize < rebind < reconcile` for semantic and frame coordinator paths while retaining stale-extent rejection before surface acquisition. Final independent verdict: PASS, no blocker/high remaining.

## Qualification failures found and fixed

This was not a pass-by-retry. Qualification and independent audit exposed defects that were corrected before the final evidence:

1. Lifetime setup performed work that could escape the counted protocol. Setup now only prewarms allocations; hidden/reveal reconciliation and first submission occur in counted events.
2. The lifetime bounded poll returned on poll error before draining the current-device GPU mailbox. It now stores `poll_result`, drains unconditionally, and returns `poll_result.and(mailbox_result)`, preserving poll-error precedence.
3. Fault evidence treated PNG structure as visibility truth and capture-failure accounting could double count. Evidence now decodes pixels; fallback and capture-only cases have distinct truthful requirements; injected capture records attempt only and the common path records exactly one failure.
4. Native smoke/lifetime validation could pass without exact terminal capture counters. It now fails closed unless attempted/succeeded/failed/nonblank is exactly `1/1/0/1`.
5. Direct-terminal capture boundary assertions followed an obsolete inline predicate. The typed helper now owns the exact Retained+Live predicate and the caller/helper are asserted separately.
6. Three `retained_scene` source-contract tests referenced removed wrappers/markers. They now bind current hidden/reveal, resize/rebind/reconcile, stale-before-acquire, and pending-version metrics contracts without weakening them.

Independent final product audit verdict: PASS, zero blocker/high findings (`local:01KXHRFJZFXPKW099XVKBRFHT3`). Independent final test-repair audit verdict: PASS (`local:01KXHWMSP012VMGXYXZ7QNR6J2`).

## Unresolved limitations and non-claims

The following are explicitly **not** proven by this Task 10 record:

- No human semantic/visual approval of the full cast at 260/360/480/720 points, non-square layouts, and 1×/2× displays was performed in this session.
- Accessibility Inspector and VoiceOver were not run manually. Role/label/value/bounds and transition behavior are covered by deterministic automation only.
- Physical keyboard/window interactions were not manually exercised: Cmd-Q, Control-Command-F, traffic-light controls, body dragging, live fullscreen transitions, and real multi-display scale migration.
- Native process fault injection covers fatal initialization/surface/validation/internal/OOM/device/resource/unsupported failures and capture map/blank/write failures. Transient Outdated/Timeout/Occluded behavior is covered by the deterministic host harness, not the 11-process native fault table.
- Native exact-path/lifetime/five-minute runs used 360×360 points at 2×. Resize/fullscreen/scale/hide-reveal/capture-swap contracts were exercised in deterministic/Metal output tests, not through physical window manipulation during the five-minute process.
- Energy was not measured with Instruments or powermetrics. Per-second encode/submit work, wake cadence, latency, RSS, static upload, and persistent GPU-resource counters are recorded; no independent power number is claimed.
- The four-hour Auto release-candidate hold and rollback rehearsal were not run. They require explicit Task 11 approval.
- This report does not approve Auto cutover, claim the legacy translator is deleted, or claim full Gate C/D/E completion.

## Conclusion

The explicit direct retained runtime passed the final isolated automated suite, native exact-path capture, the exact 4,500/33,750 dual-cadence lifetime protocol, all 11 native fault cases, and a final exact-tree five-minute release run on Apple-Silicon Metal. It produced receipt-bound, external-redacted, decoded-nonblank GPU evidence with zero unexplained fallback, zero main-thread raster calls, no post-warmup resource/static-upload growth, and latency/RSS values inside the frozen limits.

Task 10 can close with the limitations above recorded. Default cutover and the four-hour Auto canary remain separate approval work.
