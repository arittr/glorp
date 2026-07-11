# Glorp Renderer Darwin x86_64 Disposition

**Date:** 2026-07-10
**Gate:** Darwin x86_64 retained-renderer qualification
**Status:** external pre-release qualification procedure approved
**Decision owner:** repository owner/user
**Approval date:** 2026-07-10
**Explicit decision:** approved — execute native physical-Intel qualification against the exact final x86_64 release candidate before an Intel renderer-enabled release

## Proposed disposition

Choose plan option 2: a **documented external pre-release qualification procedure**.

This does not waive Intel testing, does not claim that cross-compilation is native qualification, and does not drop existing Intel CLI support. It makes native Intel execution of the exact final candidate binary a required pre-release gate before any renderer-enabled release can ship for Darwin x86_64.

If the decision owner does not explicitly approve this disposition, the renderer decision must select no backend. Smooth remains the ordinary companion default regardless.

## Measured and unavailable evidence

### Local machine, toolchain, and current-source cross-build

The qualification machine is an arm64 MacBook Pro (`Mac17,9`) with Apple M5 Pro. `rustup` was installed during this task and the active stable toolchain is now:

- host architecture: `arm64`;
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`;
- Rust host: `aarch64-apple-darwin`;
- installed targets: `aarch64-apple-darwin` and `x86_64-apple-darwin`;
- rustup: `1.29.0 (28d1352db 2026-03-05)`.

The earlier Homebrew-Rust attempt failed with `E0463` because `core` and `std` for `x86_64-apple-darwin` were unavailable. That historical failure remains preserved at:

- `target/renderer-spikes/wgpu-qualified-build/unavailable-targets/x86_64-apple-darwin.log`

After installing the x86_64 standard library with rustup, the required current-source candidate cross-build and package completed successfully in an isolated target directory:

- command: `cargo build --release --locked --no-default-features --features renderer-spike-wgpu --target x86_64-apple-darwin`;
- source commit: `8fd461a0524331becbfd4008a3460bd9257fd472`, with the renderer qualification worktree dirty and recorded in `build-identity.txt`;
- compiler: rustc/cargo 1.97.0;
- build duration: 33.49 seconds;
- warnings: the same two documented no-default Pixel dead-code warnings; no warning was suppressed;
- binary: Mach-O 64-bit executable x86_64;
- frozen path: `target/renderer-spikes/bin/glorp-wgpu-qualified-x86_64-0b036d0e3203d429ca55b7fc5e781fc718c9f3c5e50856262f281bc341ab1307`;
- binary SHA-256: `0b036d0e3203d429ca55b7fc5e781fc718c9f3c5e50856262f281bc341ab1307`;
- binary bytes: 12,469,012;
- frozen mode: non-writable (`-r-xr-xr-x`);
- linked frameworks include Metal, QuartzCore, CoreGraphics, AppKit, Foundation, and CoreFoundation;
- packaged app inventory: `Contents/Info.plist`, `Contents/MacOS/Glorp`, and `Contents/MacOS/glorp-companion`;
- archive: `Glorp-0b036d0e3203d429ca55b7fc5e781fc718c9f3c5e50856262f281bc341ab1307.app.tgz`;
- archive SHA-256: `178a7f541a749d12c84ba12d242eb43d15c92d1f46447acc974869b285d8fc20`.

Evidence root:

- `target/renderer-spikes/wgpu-qualified-x86/current-cross-rustup/`

This is valid current-source cross-build/package evidence and completes the first step of the plan's x86 decision ladder. It is **not** native Intel runtime qualification: this machine remains Apple Silicon and did not execute the Intel Metal/AppKit path on physical Intel hardware. The final Task 52 refreeze may produce a different candidate identity; native qualification must use the exact final x86_64 release-candidate hash, not automatically reuse this provisional package.

### Prior CI artifacts

Successful release workflow run `28643019270`, tag `v0.13.0`, commit `41d354abb531a65ea9798ed31df9ea6f6a1ae4`, produced non-expired Darwin x86_64 artifacts:

- `glorp-darwin-x64`, artifact ID `8059899114`, 2,661,463 bytes;
- `glorp-app-darwin-x64`, artifact ID `8059899590`, 2,621,792 bytes.

Inventory: `target/renderer-spikes/wgpu-qualified-x86/ci-v0.13.0/artifacts.json`.

These artifacts prove historical release topology only. They are from a different commit, do not contain the current renderer candidate, and are not native Intel wgpu qualification evidence.

## External qualification procedure

The procedure must be executed after Task 52 freezes the final startup-corrected candidate. All evidence must identify that exact final candidate hash; the superseded arm64 binary `e5033840f4b3e2e62e45550dff4ba90f82b82ed4e498a9c957b2104c792e38cf` is ineligible.

### 1. Produce the x86_64 artifacts

On an owned macOS builder with a Rust toolchain that can install `x86_64-apple-darwin`:

```bash
rustup target add x86_64-apple-darwin
cargo build --release --locked --no-default-features \
  --features renderer-spike-wgpu \
  --target x86_64-apple-darwin
shasum -a 256 target/x86_64-apple-darwin/release/glorp
file target/x86_64-apple-darwin/release/glorp
node scripts/build-macos-companion-app.mjs \
  --binary target/x86_64-apple-darwin/release/glorp \
  --out target/renderer-spikes/wgpu-qualified-delivery/x86_64/Glorp.app
```

Retain the exact command log, Rust/Cargo versions, commit and dirty state, binary byte count and SHA-256, `file`/`otool` output, app inventory, compressed app size, and archive hash. Confirm the binary is Mach-O x86_64. A universal or arm64-only binary does not satisfy this step.

### 2. Transfer by hash to native Intel hardware

Use an Intel Mac that reports `uname -m` as `x86_64`, supports Metal, and runs a still-supported Glorp macOS baseline. Record:

```bash
uname -m
sw_vers
system_profiler SPHardwareDataType SPDisplaysDataType
shasum -a 256 /path/to/glorp
file /path/to/glorp
```

The hash must equal the x86_64 artifact produced in step 1. Do not rebuild on the test machine unless the rebuilt identity is separately frozen and used consistently for all Intel runs.

### 3. Run native Intel renderer evidence

From the exact frozen x86_64 binary, use the qualification runner and retain artifacts under `target/renderer-spikes/wgpu-qualified-x86/native-intel/` for:

- static 360;
- capture 360 and capture 720;
- resize 360 and resize 720;
- 60-second settled occlusion at 360;
- callback-panic fault;
- capture-timeout fault;
- surface-unavailable fault.

Use the same `cargo xtask renderer-spike qualify`/`validate` contracts and frozen-binary path rules as the arm64 matrix. If the runner source is not available on the Intel machine, transfer the complete repository checkout at the same commit plus the frozen binary; the runner must execute the supplied binary rather than rebuilding it.

Every run must validate immediately. Required observations include:

- backend is `Metal` on native Intel;
- captures are exactly 720×720 and 1440×1440 physical pixels;
- one 19,200-byte immutable upload per resource generation and zero unchanged static uploads afterward;
- 2,400 frame-transform bytes per submitted frame and bounded 0/256-byte atlas updates;
- stable one-draw fixture behavior and zero atlas misses;
- resize/backing-scale resource generations are bounded and correct;
- the occluded interval submits zero frames and resumes correctly;
- capture and all injected faults terminate within their bounds with complete rejection/pass artifacts as appropriate;
- privacy scan and process cleanup pass, with no surviving renderer-spike process.

### 4. Run packaged-app topology smoke

Launch the x86_64 `Glorp.app` on the same Intel Mac and retain launcher/stdout/stderr/process evidence. This ordinary app launch is expected to use **Smooth** and proves app/package topology only. It must not be described as a real-companion wgpu launch. The separate hidden direct renderer run supplies the Intel Metal evidence.

### 5. Review and close

A reviewer other than the implementation agent checks:

- exact binary/archive hashes and architecture;
- native Intel hardware and OS facts;
- all manifest validations and raw logs;
- captures and lifecycle/fault results;
- app inventory and compressed-size gate;
- no target leakage or surviving process.

The reviewer records pass/fail, name, and date in this memo or the final renderer decision memo. Any failure leaves Darwin x86_64 unresolved and blocks selecting/shipping the retained renderer for Intel.

## Risk and affected targets

**Affected target:** Darwin x86_64 retained-renderer builds and packages. Existing no-default CLI behavior and Smooth companion behavior are not removed by this proposal.

Risks until the procedure passes:

- Intel Metal adapter/surface behavior may differ from Apple Silicon;
- capture row alignment, resize/backing scale, occlusion, or injected-fault cleanup may fail only on Intel hardware;
- the x86_64 candidate may exceed package or archive budgets;
- current-source Cargo/package topology may fail even though the July 3 release topology succeeded;
- approving the procedure defers evidence; it does not reduce these technical risks.

Mitigations are the exact-hash native matrix, explicit app/runtime distinction, immediate artifact validation, and a hard pre-release block on failure or non-execution.

## Validity and expiry

If approved, this disposition expires at the **next renderer-enabled release qualification**, and in all cases no later than **2026-08-10**. It does not authorize an Intel renderer-enabled release without completing the procedure. A later candidate rebuild or source change that affects the renderer, wgpu version/features, host bridge, shaders, capture, lifecycle, or packaging invalidates prior Intel evidence and requires requalification.

## Approval checkpoint

**Decision recorded 2026-07-10:** the repository owner approved the external qualification procedure, to be executed at release qualification against the exact final x86_64 release-candidate binary.

Consequences:

- Task 47's disposition gate is approved.
- Current cross-build/package evidence is retained but is not native Intel qualification.
- Before any Intel renderer-enabled release, the procedure in this memo must pass on physical Intel hardware.
- If the procedure is not completed, fails, expires, or the linked candidate changes without requalification, do not ship/enable the retained renderer on Darwin x86_64 and select no backend for that release decision.
- Smooth remains the ordinary companion default.
