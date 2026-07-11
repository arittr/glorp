# Glorp Renderer Release Topology Qualification

**Date:** 2026-07-10
**Status:** native arm64 topology and hard local cost gates pass; same-commit five-target build/package gate remains unresolved
**Decision owner:** repository owner/user

## Feature and launch topology

| Build | Features | Renderer-spike commands | Ordinary companion |
|---|---|---|---|
| Current local default | `dev-preview` | absent | Smooth |
| Published no-default | none | absent | Smooth |
| Qualification baseline | `renderer-spike` | Smooth/software spike only | Smooth |
| Qualification candidate | `renderer-spike-wgpu` (implies `renderer-spike`) | hidden wgpu spike available on macOS | Smooth |
| Proposed post-selection production feature | distinct name required by later architecture; not implemented | must not reuse spike DTOs | Smooth remains default/fallback |

`renderer-spike-wgpu` is non-default and macOS-only. The publish workflow continues to use:

```bash
cargo build --release --locked --no-default-features --target TARGET
```

The candidate-enabled app launcher is:

```sh
exec "$(dirname "$0")/glorp-companion" companion-app "$@"
```

It does not pass a renderer option. Therefore packaging a candidate-enabled binary proves dependency/app topology only; it does **not** launch a real companion wgpu renderer.

## Same-commit native arm64 evidence

Evidence root:

- `target/renderer-spikes/wgpu-qualified-build/local-arm64/`
- `target/renderer-spikes/wgpu-qualified-build/current-build-costs/`
- `target/renderer-spikes/wgpu-qualified-delivery/arm64/`

### Executables

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| no-default arm64 | 6,133,104 | `6b7edd8d8c4fcb7533c9e8f2ec11afb36d30b68bac9eb97115e2c40a6156e3cc` |
| candidate-enabled arm64 | 11,498,432 | `f18dcac6cd672027b3e05074996721cef458571f458cab0a97b934d51ce905c9` |
| delta | 5,365,328 (5.12 MiB) | — |

The no-default binary links AppKit/Foundation for the existing Smooth companion but does not link Metal or QuartzCore. The candidate binary links Metal, QuartzCore, CoreGraphics, AppKit, and Foundation as expected.

A fair candidate-cost comparison uses the current `renderer-spike` topology versus `renderer-spike-wgpu`, with isolated target directories and the same shared renderer-spike source touched for both incremental builds:

| Gate | Baseline | wgpu | Delta | Limit | Result |
|---|---:|---:|---:|---:|---|
| clean release build | 27.78 s | 30.00 s | 7.99% | ≤20% | pass |
| renderer-edit incremental | 7.71 s | 8.41 s | 9.08% | ≤25% | pass |
| stripped executable | 6,457,120 B | 11,498,576 B | 4.81 MiB | ≤15 MiB | pass |

The earlier no-default cold-build pair (31.26 s vs 40.09 s) is retained but is not used for the hard build-cost gate because it compares different dependency caches/topologies rather than the established candidate-enabled baseline.

Both isolated builds emitted the two previously documented no-default Pixel dead-code warnings; no warning was suppressed.

### App bundles and archives

Both same-commit binaries were packaged through `build-macos-companion-app.mjs --binary ...` into distinct paths.

| Artifact | Baseline | Candidate | Delta | Limit | Result |
|---|---:|---:|---:|---:|---|
| `.app` file bytes | 6,133,999 | 11,499,327 | 5,365,328 | diagnostic | — |
| compressed `.app.tgz` | 2,742,938 | 4,828,361 | 2,085,423 (1.99 MiB) | ≤20 MiB | pass |

Bundle inventory for both is intentionally minimal:

- `Contents/Info.plist`
- `Contents/MacOS/Glorp`
- `Contents/MacOS/glorp-companion`

No font, shader, debug-symbol, or target-cache files were bundled. The WGSL shader remains compiled into the executable. Both local qualification bundles are unsigned; the existing workflow has no signing/notarization step, so candidate packaging does not change current signing policy.

### Runtime distinction

- Candidate-enabled packaged `Glorp.app` ordinary launch, using `companion-app` with a bounded 360×360 review duration: **status 0**. This is a **Smooth launch**, not wgpu integration.
- Direct hidden candidate execution through `renderer-spike-app --candidate wgpu`: `host-functional-pass`, backend `Metal`.

Raw evidence:

- `target/renderer-spikes/wgpu-qualified-delivery/arm64/candidate/smooth-launch-smoke.log`
- `target/renderer-spikes/wgpu-qualified-delivery/arm64/direct-wgpu-smoke/`

## Non-Darwin leakage audit

Target-resolved no-default Cargo trees were generated for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`
- `aarch64-apple-darwin`

None of the three non-Darwin no-default dependency graphs contains `wgpu`, `wgpu-hal`, Metal, `objc2-quartz-core`, or renderer-spike dependencies. The same-commit native no-default arm64 binary string audit also finds no `renderer-spike-app`, `dev-preview`, `wgpu`, `CAMetalLayer`, or Metal command strings. The candidate graph contains the expected wgpu/Metal stack.

This proves feature/dependency resolution. It does **not** substitute for inspecting the final same-commit Linux/Windows executable bytes, which could not be built locally.

Evidence: `target/renderer-spikes/wgpu-qualified-build/metadata/`.

## Five-target local build status

The local Homebrew Rust installation contains only `aarch64-apple-darwin`; `rustup` is unavailable. Each other required target was attempted exactly once and failed with missing target `std`/`E0463`:

- `x86_64-apple-darwin`: unavailable locally
- `x86_64-unknown-linux-gnu`: unavailable locally
- `aarch64-unknown-linux-gnu`: unavailable locally
- `x86_64-pc-windows-msvc`: unavailable locally

Raw logs: `target/renderer-spikes/wgpu-qualified-build/unavailable-targets/`.

No claim of a same-commit five-target build is made.

## CI and package topology evidence

The latest completed publish workflow available during this audit is run `28643019270`, tag `v0.13.0`, commit `41d354abb531a65ea9798ed31df9ea6f6b6a1ae4`, completed successfully on July 3, 2026. It passed:

- Rust tests on Ubuntu, macOS, and Windows;
- no-default builds for all five publish targets;
- Darwin app packaging for arm64 and x86_64;
- launcher smoke;
- all five platform package publication jobs;
- main package publication.

Imported evidence: `target/renderer-spikes/wgpu-qualified-build/ci-v0.13.0/`.

This is valid precedent for the repository's release topology, but it is explicitly **not same-commit evidence** for the current dirty renderer qualification source.

`npm pack --dry-run --json` was run for all five platform manifests and the main package. The local worktree contains an existing darwin-arm64 staged binary, so that dry run includes it. The other four local platform directories contain no current staged binaries/apps, and their dry runs contain only `package.json`; those results prove manifest/os/cpu/package-name topology only, not releasable package contents. No empty package is described as passing delivery qualification.

Version lockstep passes at `0.13.0`; all three macOS packaging script tests pass.

Evidence: `target/renderer-spikes/wgpu-qualified-delivery/npm-pack/`.

## Required unresolved evidence

The hard release topology gate still requires current-source CI or equivalent owned builders to produce and inspect:

1. no-default binaries for all five targets;
2. candidate-enabled Darwin arm64 and x86_64 binaries;
3. candidate-enabled arm64 and x86_64 `.app` archives;
4. final staged npm platform package dry runs for all five targets, including the correct binary and Darwin app files;
5. Linux/Windows executable symbol/string audits proving no renderer-spike/wgpu/Metal leakage;
6. Ubuntu/macOS/Windows current-source all-feature compile/test evidence.

The x86_64 candidate package/runtime disposition is tracked separately in the Darwin x86 qualification task. Cross-compilation/package success, when obtained, must not be described as native Intel runtime qualification.

## Verdict

- Feature/default topology: **pass**.
- Same-commit arm64 build, linkage, hard size/build deltas, app packaging, Smooth-launch distinction, and direct native wgpu smoke: **pass**.
- Non-Darwin target-resolved dependency leakage: **pass**.
- Latest release CI topology precedent: **pass, not same commit**.
- Same-commit five-target builds, complete package contents, non-Darwin executable audits, and x86 candidate archive: **unresolved**.

**Overall Task 6 hard gate: unresolved.** The final backend decision cannot select wgpu until current-source five-target CI/equivalent evidence and the x86 disposition close these items. Normal tag publication features/defaults were not changed.
