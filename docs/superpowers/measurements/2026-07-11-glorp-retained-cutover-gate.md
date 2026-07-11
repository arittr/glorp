# Glorp Retained Cutover — Final Gate & Approval

**Gate:** Task 16 (final gate + rollback rehearsal) → Task 17 (Apple-Silicon Auto flip).
**Date:** 2026-07-11
**Branch:** `retained-companion-cutover`
**Verdict:** **Drew APPROVED the default flip** in the active session ("flip approved"), after accepting the live 360 parity (see `2026-07-11-glorp-retained-360-parity.md`).

## Automated gate — all green

Run at the parity-accepted commit `b000431` (on top of the gamma fix `eec3692`):

- `cargo fmt --check` — clean.
- `cargo clippy --all-targets --features retained-renderer -- -D warnings` — clean.
- `cargo clippy --no-default-features --features retained-renderer -- -D warnings` (the arm64-release config) — clean.
- `cargo test --features retained-renderer` — no failures.
- `cargo test --test round_scene` (6) / `--test smooth_companion` (40) / `--test retained_renderer_boundary` (2) — pass.
- `node --test scripts/test/macos-app-packaging.test.mjs` — 6 pass / 0 fail.
- `cargo test -p xtask` — 31 pass / 0 fail.
- `git diff --check` — clean.

## Native confirmation

- Live 360 pair from the actual S6 companion: `status: success`, effective `retained`, no fallback, `readback-completed`, checksum-matched Smooth/Retained sections (see the 360-parity record).
- **Dimmed** composition capture (`--dimmed`): `status: success`, effective `retained`, no fallback — the dim overlay renders correctly with the gamma convention.
- On-screen retained presentation confirmed live on hardware with the non-sRGB (`Bgra8Unorm`) surface format — no fallback, no error. This clears the detached-surface / surface-format device-gate concern that headless tests could not exercise.
- Fault injection natively validated earlier: initialization fault → graceful acknowledged Smooth fallback + clean exit; readback fault → failed manifest, effective stays Retained, non-zero process exit.

Scope note: because Drew accepted the 360 parity visually and the pipeline, fault-injection, fallback, and live surface were already validated natively, the exhaustive eight-capture size×dim matrix and the full native-smoke sweep were not re-run (the plan directs not to repeat accepted sizes without a concrete defect). The automated gate + the dimmed confirmation + the live acceptance stand as the gate evidence.

## Rollback — one line, reversible

The entire cutover is gated behind the single constant `AUTO_RETAINED_ON_APPLE_SILICON` in `src/commands/companion_mode.rs`. Rollback is exactly:

```rust
pub const AUTO_RETAINED_ON_APPLE_SILICON: bool = false;  // revert of the flip
```

Smooth stays compiled, explicitly selectable (`--renderer smooth`), and the automatic technical fallback. Intel builds do not compile Retained and Auto stays Smooth there regardless.

## Approval & flip

Drew explicitly approved the flip. Task 17 sets `AUTO_RETAINED_ON_APPLE_SILICON = true`, so Auto now resolves to Retained on a capable Apple-Silicon target (Intel Auto stays Smooth). No release is published — that is a separate, owner-requested step.

## Deferred (not in this cutover)

- The persistent scene-graph / future frame-preparation replacement (the plan strengthened the translator + host seams so this can land later without touching capture, policy, or recovery).
- `--review-capture-live-values` sets the manifest privacy mode to sensitive but does not yet un-redact the HUD text in the frozen frame (minor diagnostic-tool gap).
- Tracked lint-hygiene follow-ons from the final review: exhaustiveness guards on the hand-maintained content-variant arrays; making the write-only resource counters saturating/u64.
