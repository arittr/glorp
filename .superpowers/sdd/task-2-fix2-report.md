# Task 2 Second Fix Report

Status: DONE

Commit: `fix(companion): close scene snapshot privacy seams` (this commit)

## Outcome

The schema-v1 companion snapshot no longer carries formatted live HUD values or
exact live gauge ratios. HUD text is the stable review redaction
(`review` / `privacy` / `redacted`), while gauges serialize closed qualitative
buckets. The snapshot's privacy claims remain explicit and false for exact
counts and source names.

Tank motion now comes from one renderer-neutral route resolver. The private seed
and route hash exist only inside the resolver. Its output is a bounded visible
contract: catalog route, visibility, origin row/column, side, layer pattern,
sprite variant, visible rows, anemone morph, cadence, and calm state. The TUI and
snapshot both consume that output. The old route-token state and its low-byte
serialization were deleted.

Neutral motion now accepts `CompanionMotionInput` primitives and explicit generic
clearance parameters. Round and companion-scene adapters construct the input at
their view-model edges. Habitat inventory selection accepts domain records and
tank IDs; TUI and snapshot adapters map their view models into those records.

## RED Evidence

- Formatted HUD privacy: the focused test failed with live
  `["842M", "94% yday", "31M/10m"]` instead of the stable redacted lines.
- Exact gauge privacy: full snapshot JSON exposed `0.432109`, `0.943217`, and
  `0.87535733` while `exact_counts_visible` was false.
- Tank route privacy: serialized state contained seed-derived
  `"route_phase":125` and `"route_phase":162` values.
- Shared tank API: the resolver test failed to compile because
  `presentation::tank_life` did not exist.
- Neutral ownership: the closure scan reported `WatchViewModel`, `crate::tui`,
  and Smooth ownership in `round/motion.rs`, plus `crate::tui` in
  `presentation/habitat_inventory.rs`.

## GREEN Evidence

- Default companion scene: 18 passed.
- Retained-renderer companion scene: 18 passed.
- Hardened boundary suite: 5 passed, including alias/case/separator bypasses.
- Tank-life unit suite: 10 passed.
- Habitat-prop unit suite: 30 passed.
- Round scene unit suite: 20 passed.
- `round_scene`, `smooth_companion`, `presentation_scene`, `presentation_pet`,
  and `storage_privacy`: 66 passed.
- Preview Lab: all 79 `dev_preview` tests passed with no artifact drift.
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `git diff --check`: passed.

## Privacy Proof

- Complete snapshot JSON, snapshot `Debug`, and projection error `Display` plus
  `Debug` are checked against the actual formatted live HUD strings.
- Exact progress, daily-comparison, and pace ratios are absent from JSON; only
  closed gauge buckets cross the boundary.
- The route resolver input types that temporarily hold exact rate or private seed
  do not implement `Debug` or serialization.
- Snapshot tank state has no phase, token, hash, seed, or truncated seed-derived
  field. It serializes only visible route semantics.
- Unknown tank IDs fail closed. Existing unknown prop/tank/source/path/auth/error
  sentinel coverage remains in the complete serialized snapshot test.

## Semantic and Output Proof

- Route parity covers every current tank catalog entry at 20x12, 44x18, and
  72x24; ticks 0, 4, 8, and 32 seconds; and normal, calm, and asleep cadence.
  Resolver visibility, origin, layer pattern, and cadence match TUI placement.
- Catalog cast sorting remains ID-associated and deterministic under input
  reordering. Snapshot topology and state remain capped at two round inhabitants.
- Existing Classic truncation/breath, Round motion, Smooth depth/clearance,
  habitat-prop, weather, lattice, and Preview Lab contracts remain green.

## Boundary Proof

The recursive companion-scene scan still rejects renderer, host, terminal, and
platform imports. The dependency-closure scan additionally covers
`round/motion.rs`, `presentation/habitat_inventory.rs`, and
`presentation/tank_life.rs`, rejecting Watch view models, TUI, Smooth, Ratatui,
wgpu, Objective-C, and AppKit ownership, including normalized alias and separator
bypasses. `companion_scene/input.rs` remains the explicit allowed view-model
adapter described by the task.

## Self-review

Reviewed `c9adf56..HEAD` for live/formatted telemetry, exact normalized ratios,
source strings, debug/error exposure, raw or truncated seed state, route parity,
unknown-ID behavior, catalog reorder stability, calm/asleep cadence, motion
ownership, Smooth constant ownership, TUI output drift, and Task 3 scope. No
remaining Task 2 concern was found.
