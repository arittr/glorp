# Glorp Preview Lab Animation Strips Design

## Status

Slice 2 design. This extends the Slice 1 `glorp dev-preview` bundle with
deterministic animation strips and paused HTML playback.

The parent design is
`docs/superpowers/specs/2026-05-12-glorp-preview-lab-design.md`. Slice 1 has
already shipped the hidden command, static watch captures, pet species/stage
matrix, manifest, review markdown, and static HTML contact sheet. This slice
should build on those contracts rather than replacing them.

This version incorporates the Slice 2 review pass. The important hardening
decisions are:

- Slice 2 manifest output is schema version `2`.
- `manifest.json`, `review.md`, and `index.html` are generated from one
  internal preview-bundle model.
- Manifest paths are normalized relative UTF-8 `/` paths, not arbitrary
  platform `PathBuf` serialization.
- Strip frames support either a renderer `tick` or a semantic `phase`; fake
  ticks are not used for phase-only animation.

## Goal

Make pet animation reviewable by Drew and by agents without running the live TUI
event loop.

After this slice, a developer can run:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

and inspect:

- the existing static watch and pet matrix frames
- a small set of deterministic pet animation strips
- paused playback controls that allow frame-by-frame review
- manifest metadata that explains which ticks or phases and fixtures produced
  each strip

The design target is fast visual iteration, not exhaustive terminal simulation.

## Scope

Implement pet-local animation strips:

- idle breath and wander movement from the current `PetPanel` layout path
- blink-hit frames from `render_pet`
- glitch corruption hit frames from `render_pet`
- particle or scan-line hit frames from `render_pet`

Add HTML playback for those strips:

- every strip starts paused
- controls support play, pause, previous frame, and next frame
- the active frame index and tick or phase label are visible
- frame dimensions stay fixed within each strip

Add manifest and review output for strips:

- strip metadata is first-class in `manifest.json`
- every strip frame is listed in `artifacts`
- `review.md` includes a strip section and targeted review prompts

## Non-Goals

Do not implement full watch-effect strips in this slice. Stage-up, feed-pulse,
transition overlays, and `PetAnimator::apply` playback belong to the next
animation slice.

Do not add real TUI/tmux Gauntlet stories yet. A static preview-bundle Gauntlet
story may be useful after Slice 2 if HTML playback itself needs outer-loop
validation, but it is not required for this slice. Real terminal interaction
belongs after full watch-effect strips exist.

Do not add live terminal capture, PTY capture, tmux automation, or crossterm
event-loop verification.

Do not add ANSI export. Continue writing `.txt` and `.cells.json` captures plus
HTML.

Do not render every species, stage, mood, and tick combination. The bundle should
stay small enough that a human or agent can review it quickly.

Do not add external JavaScript, CSS, fonts, browser network calls, or new Rust
dependencies.

## CLI Contract

Keep the existing hidden command:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Extend `--scenario` with one new value:

```bash
cargo run -- dev-preview --scenario animation --out target/glorp-preview
```

Selection behavior:

- `watch` generates only the existing static watch captures.
- `pets` generates only the existing static species/stage matrix.
- `animation` generates only animation strips and their HTML/review/manifest
  wrapper.
- `all` generates watch captures, pet matrix captures, and animation strips.

The default remains `all`, and `all` means the complete review bundle. Docs must
make the focused loops explicit:

- use `watch` for layout-only iteration
- use `pets` for static species/stage review
- use `animation` for animation playback review
- use `all` before handoff or review when the full visual bundle matters

Output safety remains unchanged:

- write through the existing staging directory
- replace only empty directories or preview-owned directories
- refuse files, symlink paths, and non-preview non-empty directories

## Artifact Layout

Keep Slice 1 static frames in `frames/`:

```text
target/glorp-preview/
  frames/
    watch-wide-normal.txt
    watch-wide-normal.cells.json
    watch-compact-normal.txt
    watch-compact-normal.cells.json
    pet-species-stage.txt
    pet-species-stage.cells.json
```

Add strip frames under `strips/<strip-id>/`:

```text
target/glorp-preview/
  strips/
    pet-idle-motion-fuzz-s4/
      frame-000.txt
      frame-000.cells.json
      frame-001.txt
      frame-001.cells.json
      frame-002.txt
      frame-002.cells.json
      ...
    pet-blink-hit-fuzz-s4/
      frame-000.txt
      frame-000.cells.json
      frame-001.txt
      frame-001.cells.json
      ...
    pet-glitch-corruption-hit-s4/
      frame-000.txt
      frame-000.cells.json
      frame-001.txt
      frame-001.cells.json
      ...
    pet-particle-hit-crystal-s4/
      frame-000.txt
      frame-000.cells.json
      frame-001.txt
      frame-001.cells.json
      ...
```

Keep shared outputs at the root:

```text
target/glorp-preview/
  .glorp-preview
  index.html
  manifest.json
  review.md
  assets/
    preview.css
    preview.js
```

Strip frame files use zero-padded frame indexes. Paths in the manifest are
relative to the output directory, matching the existing static frame behavior.

## Manifest Contract

Keep existing `scenarios[]` for static frames.

Add a top-level `strips[]` array to `PreviewManifest`. Do not overload
`PreviewScenario` with multi-frame semantics.

Slice 2 bumps the manifest schema to version `2`. This is a structural contract
change because animation-only bundles can have an empty `scenarios[]` array and a
non-empty `strips[]` array. Schema version `2` readers must treat `artifacts[]`
as the exhaustive file inventory and must understand both static `scenarios[]`
and animated `strips[]`.

Manifest path rules:

- paths are normalized relative UTF-8 strings, not platform-dependent path
  serialization
- use `/` separators
- no leading `/`
- no `..` segments
- no backslashes
- generated ids use slug format `[a-z0-9][a-z0-9-]*`

Rust code can keep `PathBuf` internally while writing files, but serialized
manifest structs should use a contract type such as `PreviewPath(String)` or
plain normalized strings.

Illustrative excerpt:

```json
{
  "schema_version": 2,
  "producer": "glorp-dev-preview",
  "glorp_version": "0.1.6",
  "generated_at": "2026-05-12T00:00:00Z",
  "scenarios": [],
  "strips": [
    {
      "id": "pet-idle-motion-fuzz-s4",
      "kind": "pet-animation",
      "title": "Pet Idle Motion: Fuzz S4",
      "intent": "Review deterministic breath and wander movement through PetPanel.",
      "dimensions": { "width": 20, "height": 12 },
      "playback": {
        "starts_paused": true,
        "frame_duration_ms": 160
      },
      "inputs": {
        "species": "fuzz",
        "stage": "s4",
        "mood": "content",
        "seed": "glorp-preview-idle-fuzz",
        "phases": ["rest", "inhale-left", "hold", "exhale-right", "rest"],
        "breath_offsets_y": [0, 1, 1, 0, 0],
        "wander_offsets_x": [0, -1, 0, 1, 0],
        "hit_kind": "idle-motion"
      },
      "frames": [
        {
          "index": 0,
          "label": "rest center",
          "phase": "rest",
          "files": {
            "text": "strips/pet-idle-motion-fuzz-s4/frame-000.txt",
            "cells": "strips/pet-idle-motion-fuzz-s4/frame-000.cells.json"
          }
        },
        {
          "index": 1,
          "label": "inhale left",
          "phase": "inhale-left",
          "files": {
            "text": "strips/pet-idle-motion-fuzz-s4/frame-001.txt",
            "cells": "strips/pet-idle-motion-fuzz-s4/frame-001.cells.json"
          }
        }
      ],
      "review_prompts": [
        "Does the pet move by one row or one column without clipping?",
        "Does idle movement feel intentional rather than like layout jitter?"
      ]
    }
  ],
  "artifacts": [
    {
      "id": "pet-idle-motion-fuzz-s4-frame-000-text",
      "title": "Pet Idle Motion: Fuzz S4 frame 0 text",
      "type": "text",
      "path": "strips/pet-idle-motion-fuzz-s4/frame-000.txt",
      "width": 20,
      "height": 12
    }
  ]
}
```

For direct `render_pet` strips, `PreviewStripFrame.tick` is present. For
phase-only strips like idle motion, `PreviewStripFrame.phase` is present and
`tick` is omitted.

Rust model additions:

- `PreviewManifest::strips: Vec<PreviewStrip>`
- `PreviewStrip`
- `PreviewStripKind`
- `PreviewStripFrame`
- `PreviewStripFrameFiles`
- `PreviewPlayback`
- `PreviewPath` or equivalent normalized-path serializer

Use `#[serde(rename_all = "kebab-case")]` for new enum values, matching the
existing manifest style.

Recommended enum values:

- `PreviewStripKind::PetAnimation` serializes as `pet-animation`

Each strip frame should also appear in `artifacts[]`:

- text artifact type remains `text`
- cells artifact type remains `cells`
- id format is `<strip-id>-frame-000-text` and `<strip-id>-frame-000-cells`
- width and height are populated for frame artifacts

This keeps simple artifact readers useful without teaching them a new artifact
type.

## Strip Inventory

### `pet-idle-motion-fuzz-s4`

Purpose: verify the actual idle breath and wander path used by the current TUI.

Important implementation detail: breath and wander are not currently
`render_pet` tick effects. The live watch app computes `breath_offset_y` and
`wander_offset_x`, stores them on `WatchViewModel`, and `PetPanel` applies the
one-row lift and horizontal offset while rendering. The preview should exercise
that real path.

Render strategy:

- build a minimal `WatchViewModel` for a deterministic Fuzz S4 pet
- set `pet_art` and `pet_spans` from `render_pet`
- pin unrelated fields:
  - `current_speech = None`
  - `cursor_screen = None`
  - `mouse_tracking_enabled = false`
  - `energy >= 0.6`
- set `wander_offset_x` and `breath_offset_y` explicitly per frame
- render `PetPanel` into a fixed buffer

Frames:

```text
index  label          breath_offset_y  wander_offset_x
0      rest center    0                0
1      inhale left    1               -1
2      hold center    1                0
3      exhale right   0                1
4      rest center    0                0
```

The frame labels should say `rest center`, `inhale left`, `hold center`,
`exhale right`, and `rest center`. Use `phase` metadata for these frames. Do not
invent renderer ticks for them.

Review prompts:

- Does the pet move by exactly one row or one column with no clipping?
- Does the strip feel like idle motion rather than a layout jump?

### `pet-blink-hit-fuzz-s4`

Purpose: guarantee that a closed-eye frame is present and reviewable.

Render strategy:

- generate a deterministic Fuzz S4 pet with seed `glorp-preview-blink-fuzz`
- render with `Mood::Content`
- compute a tick window from the pet's blink cadence
- include one closed-eye tick plus two neighboring ticks on each side when
  possible

Do not hard-code a magic tick unless a test anchors why that tick is stable.
Prefer a small helper that searches a bounded deterministic range, renders the
pet, and checks an open -> closed -> same-open transition. The selected fixture
must satisfy `pet.traits.eyes != closed_blink_eyes(species)` so the closed-eye
assertion cannot pass on a normal open-eye frame.

Bounded search:

- search ticks `0..=256`
- set `blink_suppression_ticks = 0`
- fail with the selected species, seed, cadence inputs, and searched range if no
  closed frame is found

Frame count: 5.

Review prompts:

- Does the closed-eye frame read as a blink?
- Does the face return to the same open-eye expression after the blink?

### `pet-glitch-corruption-hit-s4`

Purpose: make glitch corruption visible without asking a reviewer to scrub a
long timeline.

Render strategy:

- generate a deterministic Glitch S4 pet with seed `glorp-preview-glitch`
- render with `Mood::Content`
- include ticks around a corruption hit
- include a nearby scan-line or particle tick only if it does not obscure the
  corruption being reviewed

The current corruption path only mutates a body cell when the selected tick,
row, and column land on a non-space body glyph. Implement a bounded hit finder
that returns the exact corruption oracle:

```rust
struct CorruptionHit {
    tick: u64,
    row: usize,
    col: usize,
    before: char,
    after: char,
}
```

Prefer a `pub(crate)` helper in `pet::render` so the preview oracle cannot drift
from the real renderer. A dev-preview-local helper is acceptable only if tests
prove it stays equivalent to the renderer's corruption behavior. The test should
render that exact tick and assert that the exact cell changed from `before` to
`after`.
Suppress blink during this strip with `blink_suppression_ticks > 0` so blink
cannot be mistaken for corruption. The hit finder should search ticks `0..=512`
and fail with species, seed, stage, and searched range if no visible hit exists.

Frame count: 5 to 7.

Review prompts:

- Is the corruption visible but still recognizably Glitch?
- Does the effect avoid looking like broken Unicode or a layout bug?

### `pet-particle-hit-crystal-s4`

Purpose: review species particle styling and cell placement.

Render strategy:

- generate a deterministic Crystal S4 pet with seed
  `glorp-preview-particle-crystal`
- render with `Mood::Content`
- include default comparison ticks `0, 4, 8, 12, 16`
- suppress blink during this strip with `blink_suppression_ticks > 0`

Crystal is preferred because its particle glyphs are visually distinct and easy
to recognize in the cell JSON and HTML.

Frame count: 5.

Review prompts:

- Do particles sit inside the fixed frame without clipping?
- Are particles visible using the current pet accent styling?

## Rendering Model

Add a new internal module:

```text
src/dev_preview/animation.rs
```

Responsibilities:

- declare strip fixtures
- render strip frames
- return frame data plus manifest metadata
- keep all animation-specific tick search helpers local to dev preview

Suggested public surface inside the crate:

```rust
pub fn animation_strips(ctx: &PreviewRenderContext) -> Result<Vec<PreviewStripBundle>>;
```

Where `PreviewStripBundle` contains:

- strip metadata without file paths that depend on the output directory
- ordered `PreviewFrame`s for the strip
- per-frame labels and tick or phase metadata

Add a small generated-bundle model that feeds all writers:

```rust
struct PreviewBundle {
    static_frames: Vec<PreviewFrame>,
    strips: Vec<PreviewStripBundle>,
    manifest: PreviewManifest,
}
```

`manifest.json`, `review.md`, `index.html`, and artifact writing should all use
the same `PreviewBundle` data. Do not independently recompute strip frame counts,
labels, paths, or artifact ids inside each writer.

Avoid duplicating pet styling logic. Reuse:

- `render_pet`
- `pet_role_spans_for_line`
- `PetPanel` for idle motion
- `frame_from_buffer`

For direct `render_pet` strips, use a fixed 20x12 buffer:

- render the 13x10 framed pet art inside it
- center horizontally
- leave labels outside the buffer in HTML and manifest metadata
- do not place changing labels inside the terminal buffer

That keeps frame dimensions stable and prevents labels from masquerading as
animation changes.

For the idle-motion strip, render `PetPanel` in the same 20x12 buffer so the
movement comes from the real panel code.

## HTML Playback

Keep the existing static frame contact sheet. Add a second section for strips.

Each strip renders as:

- title
- short intent
- fixed-size active frame viewport
- controls
- active frame label
- review prompts

Controls:

- play or pause button
- previous frame button
- next frame button
- visible `N / total` frame counter
- visible tick, phase, or label text

Behavior:

- all strips start paused
- no playback timers are created before user interaction
- clicking play creates the strip-local timer and advances frames at
  `playback.frame_duration_ms`
- pause, previous, and next clear any active timer for that strip
- playback loops within the strip
- previous and next step exactly one frame and leave the strip paused
- controls are scoped per strip, so playing one strip does not move another
- no browser storage, network access, timers before user interaction, or
  generated inline script data fetches

Implementation preference:

- server-generate all frame HTML into `index.html`
- hide inactive strip frames with CSS
- use vanilla `assets/preview.js` to toggle active frame classes

This preserves the offline artifact model and keeps Browser inspection simple.
Agents should use `index.html` for visual inspection, `manifest.json` for exact
metadata and file inventory, and `review.md` for a concise checklist. This slice
does not generate screenshot artifacts.

## Review Markdown

Extend `review.md` with a `## Animation Strips` section.

For each strip, include:

- title
- id
- kind
- dimensions
- frame count
- frame labels and tick or phase metadata
- review prompts

Do not duplicate every frame path if it makes the document noisy. The manifest
remains the exhaustive file inventory.

## Tests

Add focused unit tests before implementation code where practical.

Manifest/export tests:

- manifest serialization uses schema version `2` and includes `strips[]`
- strip kind serializes as `pet-animation`
- each strip frame appears in `artifacts[]`
- manifest paths are normalized relative `/` strings with no leading slash,
  backslash, or `..`
- `review.md` includes an animation strip section
- generated HTML contains strip controls and a paused initial state
- generated JavaScript creates timers only on play and clears them on pause,
  previous, and next
- HTML escaping applies to strip titles, labels, prompts, and cells

Generation tests:

- `animation_strips` returns the four named strips
- every strip has at least five frames unless explicitly documented otherwise
- every frame in a strip has the same width and height
- every generated strip frame has a non-empty text export
- idle-motion strip includes rest, lifted, left, and right frames
- idle-motion fixture pins speech, cursor, mouse tracking, and energy so only
  breath and wander vary
- blink strip uses a fixture whose open eyes differ from
  `closed_blink_eyes(Species::Fuzz)` and proves open -> closed -> same-open
  transition across neighboring frames
- glitch strip finds a `CorruptionHit` and asserts the exact rendered cell
  changes from `before` to `after`
- particle strip proves at least one particle hit before export, either by
  inspecting source rendered spans for `PaletteRoleName::Particle` or by
  asserting on a known particle glyph in the exported text

Integration tests:

- `glorp dev-preview --scenario animation --out <tempdir>` writes
  `index.html`, `manifest.json`, `review.md`, and strip frame files
- `glorp dev-preview --scenario all --out <tempdir>` writes both Slice 1 static
  frames and Slice 2 strips
- `watch` and `pets` selections do not write strip directories
- `animation` selection may leave `frames/` absent or empty, but `strips/` must
  be present and non-empty
- stale strip files are removed on rerun through the existing output replacement
  path
- the command does not read or mutate user Glorp state

Manual verification:

```bash
cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation
open target/glorp-preview-animation/index.html
cargo test
```

## Implementation Order

1. Add strip manifest structs and serialization tests.
2. Add `PreviewSelection::Animation` and CLI parsing for `--scenario animation`.
3. Add normalized manifest path helpers and slug validation.
4. Add `PreviewBundle` so manifest, review markdown, HTML, and artifacts share
   one source of truth.
5. Add strip artifact path helpers and writer plumbing.
6. Add `src/dev_preview/animation.rs` with the four strip fixtures.
7. Extend HTML generation with strip markup and controls.
8. Extend `preview.css` and `preview.js` for playback.
9. Extend `review.md` generation.
10. Add integration tests for `animation` and `all`.
11. Update `README.md`, `AGENTS.md`, and `CLAUDE.md` with Slice 2 usage,
    including focused guidance for `watch`, `pets`, `animation`, and `all`.

## Acceptance Criteria

- `cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation`
  produces a reviewable offline bundle.
- `cargo run -- dev-preview --scenario all --out target/glorp-preview`
  includes the existing static frames and the new animation strips.
- `manifest.json` includes first-class `strips[]` metadata and artifacts for
  every strip frame, with schema version `2`.
- Manifest paths are normalized relative `/` strings.
- `index.html` displays animation strips with paused playback controls and
  frame-by-frame stepping.
- HTML playback creates timers only after play and clears them on pause/step.
- The idle-motion strip uses the real `PetPanel` breath and wander offset path.
- Blink, glitch, and particle strips each include a deterministic visible hit.
- Blink and glitch tests prove the exact intended animation behavior rather than
  broad output differences.
- Strip frame dimensions are fixed within each strip.
- Existing static preview behavior remains intact.
- Normal user-facing help still hides `dev-preview`.
- README, AGENTS, and CLAUDE usage docs explain the animation scenario and the
  focused scenario choices.
- Relevant Rust tests pass, including preview integration tests.

## Deferred Slice 3

The next slice should add full watch-effect strips:

- instantiate `PetAnimator`
- seed previous and next watch view models
- render full watch frames at fixed elapsed-time steps
- apply transition effects to the ratatui buffer
- export strips for at least one feed-pulse or stage-up moment

That is the right point to consider Gauntlet stories around preview bundle
review or real TUI interaction. Static preview-bundle review through Gauntlet can
be considered immediately after Slice 2 if the generated HTML playback needs an
outer-loop check, but real TUI/tmux behavior waits for Slice 3.
