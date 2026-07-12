# Task 2 Fix Report

Status: DONE

Commit: `fix(companion): ground scene snapshot semantics` (this commit)

## Outcome

The companion snapshot now fails closed at its privacy boundary and projects one
fixed 13x10 pet lattice. It accepts only space plus the declared generated-pet
glyph repertoire, pads legitimate short art, validates role spans against the
unpadded source row in Unicode scalar units, and returns structural errors that
never echo the rejected glyph or source text.

Round and Smooth now consume the same renderer-neutral round motion projection
as the snapshot. The projection carries explicit cell and logical-point anchors,
normalized depth, round-facing, and separate breath/bob channels. Snapshot input
requires a wall-clock instant and an independently supplied monotonic elapsed
millisecond value; there is no Unix-time elapsed fallback.

Prop and tank animation snapshots now come from the authored animation helpers
used by the current renderers. Prop state preserves the 8-second sprite, relevant
12-second twinkle, 20-second motion, and chest-lid gates; static props serialize
an explicit `kind: "static"`. Tank state carries a bounded visible phase and the
existing 4-second/8-second calm cadence without serializing the full seed-derived
route token. Weather is content state; biome and dialect remain stable topology.

The recursive boundary scan now normalizes case and non-alphanumeric separators,
rejects aliased Smooth imports and terminal/painter module roots, and has direct
self-tests for underscore, separator, case, and alias bypasses.

## RED Evidence

- Fixed lattice: `cargo test presentation::companion_scene -- --nocapture`
  failed on missing `CompanionSceneProjectionError` and the non-fallible
  projection API. The first multibyte fixture also demonstrated that `◉` was not
  in the Fuzz repertoire, so the test was corrected to use two declared `✦`
  scalar cells.
- Motion: the focused clock/parity test failed on missing
  `CompanionProjectionClock`, `CompanionSceneProjectionInput`,
  `round::motion`, and `project_with_input`.
- Animation/weather: focused tests failed because content still exposed fabricated
  phase vectors and topology still owned weather.
- Boundary: the new bypass tests failed on missing `boundary_violations`; after
  implementing normalization, the tree test correctly failed on the existing
  `crate::tui::component` dependency until inventory selection moved to the
  renderer-neutral presentation module.
- Explicit static state: the serialized static-prop assertion failed until
  `PropAnimationKindSnapshot::Static` was added.

## GREEN Evidence

- `cargo test presentation::companion_scene`: 14 passed under default features.
- `cargo test --features retained-renderer --lib presentation::companion_scene`:
  14 passed.
- `cargo test --test companion_scene_boundary`: 3 passed.
- `cargo test --test presentation_scene --test round_scene --test smooth_companion --test presentation_pet --test storage_privacy`:
  66 passed.
- Current authored helper coverage:
  `cargo test tui::component::habitat_props --lib` passed 30 tests and
  `cargo test tui::component::tank_life --lib` passed 9 tests.
- `cargo test --features dev-preview --test dev_preview`: 79 passed; no snapshot
  or Preview Lab artifact changed.
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `git diff --check`: passed.

## Privacy Proof

- Full serialized snapshot JSON is scanned for sentinels placed in raw seed, pet
  name, source/display names, diagnostics, helper status, errors, file path,
  speech/auth text, unknown prop/inhabitant IDs, and raw usage totals.
- Private text placed directly in `pet_art` fails closed. The returned error and
  its display text contain only row/cell indices, never rejected content.
- Oversized rows/columns, disallowed glyphs, and spans outside the real unpadded
  source row fail projection. Output is always exactly 10 rows of 13 Unicode
  scalar cells.
- The serialized contract contains no seed or full route-token field. Tank route
  state is a bounded `u8` visual phase tied to a known catalog identity.
- Unknown catalog identities remain dropped fail closed. Prop/tank limits remain
  10 and 2, with deterministic catalog-associated ordering.
- The companion-scene tree has no retained, wgpu, Smooth, Ratatui, terminal
  painter, native window/view, or GPU surface types, including aliased imports.

## Current-output Proof

- `cargo test round::scene`: 20 passed, covering legacy motion defaults, Classic
  truncation/rect parity, facing, energy, envelope, breath, and deterministic
  draw lists.
- `round_scene` and `smooth_companion` passed 46 tests together, including
  far/neutral/near depth, fractional anchors, breath-not-world-motion, bob-only,
  parallax, and clearance.
- All 79 Preview Lab tests passed and `git status` showed no changed snapshot
  files.

## Deviations and Shared Sources

- The flawed three-argument snapshot API was removed rather than wrapped. The
  replacement takes `CompanionSceneProjectionInput` with explicit wall time,
  monotonic elapsed time, logical layout, and grid dimensions.
- Existing motion math moved from `round::scene` into `round::motion`; the scene
  retains only the Ratatui `Rect` adapter. Smooth's existing bob function delegates
  to the same neutral source.
- Prop selection and canonical tank cast moved unchanged into
  `presentation::habitat_inventory` so snapshot projection does not reach through
  terminal component modules. Existing internal component paths re-export the
  helpers for retained-only callers without touching Task 1 renderer/metrics code.
- No compatibility layer was added for the flawed snapshot contract, and no Task
  3 runtime work was started.

## Self-review

Reviewed the Task 2 change against every independent finding for lattice shape,
Unicode span units, error privacy, motion units/facing/depth/breath/bob parity,
authored phase cadences, static-state representation, calm/asleep behavior,
weather lifetime, deterministic limits/order, and renderer-boundary aliases.
The review found and fixed the initially implicit static-prop representation.
No remaining Task 2 concern.
