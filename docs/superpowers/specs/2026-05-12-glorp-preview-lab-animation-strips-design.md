# Glorp Preview Lab Animation Strips Design

## Status

Slice 2 design. This extends the Slice 1 `glorp dev-preview` bundle with
deterministic animation strips and paused HTML playback.

The parent design is
`docs/superpowers/specs/2026-05-12-glorp-preview-lab-design.md`. Slice 1 has
already shipped the hidden command, static watch captures, pet species/stage
matrix, manifest, review markdown, and static HTML contact sheet. This slice
should build on those contracts rather than replacing them.

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
- manifest metadata that explains which ticks and fixtures produced each strip

The design target is fast visual iteration, not exhaustive terminal simulation.

## Scope

Implement pet-local animation strips:

- idle breath movement from the current `PetPanel` layout path
- blink-hit frames from `render_pet`
- glitch corruption hit frames from `render_pet`
- particle or scan-line hit frames from `render_pet`

Add HTML playback for those strips:

- every strip starts paused
- controls support play, pause, previous frame, and next frame
- the active frame index and tick label are visible
- frame dimensions stay fixed within each strip

Add manifest and review output for strips:

- strip metadata is first-class in `manifest.json`
- every strip frame is listed in `artifacts`
- `review.md` includes a strip section and targeted review prompts

## Non-Goals

Do not implement full watch-effect strips in this slice. Stage-up, feed-pulse,
transition overlays, and `PetAnimator::apply` playback belong to the next
animation slice.

Do not add Gauntlet stories yet. Gauntlet remains useful as an outer review loop
once the preview bundle has animation artifacts worth inspecting.

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

The default remains `all`.

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
    pet-idle-breath-fuzz-s4/
      frame-000.txt
      frame-000.cells.json
      frame-001.txt
      frame-001.cells.json
    pet-blink-hit-mech-s4/
      frame-000.txt
      frame-000.cells.json
    pet-glitch-corruption-hit-s4/
      frame-000.txt
      frame-000.cells.json
    pet-particle-hit-crystal-s4/
      frame-000.txt
      frame-000.cells.json
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

Shape:

```json
{
  "schema_version": 1,
  "producer": "glorp-dev-preview",
  "glorp_version": "0.1.6",
  "generated_at": "2026-05-12T00:00:00Z",
  "scenarios": [],
  "strips": [
    {
      "id": "pet-blink-hit-mech-s4",
      "kind": "pet-animation",
      "title": "Pet Blink Hit: Mech S4",
      "intent": "Review a deterministic blink frame and neighboring open-eye frames.",
      "dimensions": { "width": 20, "height": 12 },
      "playback": {
        "starts_paused": true,
        "frame_duration_ms": 160
      },
      "inputs": {
        "species": "mech",
        "stage": "s4",
        "mood": "content",
        "seed": "glorp-preview-blink-mech",
        "ticks": [20, 21, 22, 23, 24],
        "hit_kind": "blink"
      },
      "frames": [
        {
          "index": 0,
          "label": "tick 20",
          "tick": 20,
          "files": {
            "text": "strips/pet-blink-hit-mech-s4/frame-000.txt",
            "cells": "strips/pet-blink-hit-mech-s4/frame-000.cells.json"
          }
        }
      ],
      "review_prompts": [
        "Does the blink read as intentional rather than a rendering glitch?",
        "Does the pet stay centered and unclipped across the strip?"
      ]
    }
  ],
  "artifacts": []
}
```

Rust model additions:

- `PreviewManifest::strips: Vec<PreviewStrip>`
- `PreviewStrip`
- `PreviewStripKind`
- `PreviewStripFrame`
- `PreviewStripFrameFiles`
- `PreviewPlayback`

Use `#[serde(rename_all = "kebab-case")]` for new enum values, matching the
existing manifest style.

Recommended enum values:

- `PreviewStripKind::PetAnimation` serializes as `pet-animation`

Each strip frame should also appear in `artifacts[]`:

- text artifact type remains `text`
- cells artifact type remains `cells`
- id format is `<strip-id>-frame-000-text` and `<strip-id>-frame-000-cells`
- width and height are populated for frame artifacts

This keeps old artifact readers useful without teaching them a new artifact
type.

## Strip Inventory

### `pet-idle-breath-fuzz-s4`

Purpose: verify the actual idle breathing path used by the current TUI.

Important implementation detail: breathing is not currently a `render_pet` tick
effect. The live watch app computes `breath_offset_y`, stores it on
`WatchViewModel`, and `PetPanel` applies the one-row lift while rendering. The
preview should exercise that real path.

Render strategy:

- build a minimal `WatchViewModel` for a deterministic Fuzz S4 pet
- set `pet_art` and `pet_spans` from `render_pet`
- set `wander_offset_x = 0`
- set `breath_offset_y` explicitly per frame
- render `PetPanel` into a fixed buffer

Frames:

```text
index  label       breath_offset_y
0      rest        0
1      inhale      1
2      hold        1
3      exhale      0
4      rest        0
```

The frame labels should say `rest`, `inhale`, `hold`, `exhale`, and `rest`.
The manifest should also include a synthetic `ticks` field for consistency, but
review should key off the breath labels.

Review prompts:

- Does the pet move by exactly one row with no clipping?
- Does the strip feel like breathing rather than a layout jump?

### `pet-blink-hit-mech-s4`

Purpose: guarantee that a closed-eye frame is present and reviewable.

Render strategy:

- generate a deterministic Mech S4 pet with seed `glorp-preview-blink-mech`
- render with `Mood::Content`
- compute a tick window from the pet's blink cadence
- include one closed-eye tick plus two neighboring ticks on each side when
  possible

Do not hard-code a magic tick unless a test anchors why that tick is stable.
Prefer a small helper that searches a bounded deterministic range, renders the
pet, and checks that at least one frame contains `closed_blink_eyes(species)`.

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
that searches rendered output and chooses a visible corruption tick. The test
should fail if no visible hit is found.

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

Crystal is preferred because its particle glyphs are visually distinct and easy
to recognize in the cell JSON and HTML.

Frame count: 5.

Review prompts:

- Do particles sit inside the fixed frame without clipping?
- Are particle colors distinct from body, eye, mouth, accent, and pattern roles?

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
- per-frame labels and ticks

Avoid duplicating pet styling logic. Reuse:

- `render_pet`
- `pet_role_spans_for_line`
- `PetPanel` for breath movement
- `frame_from_buffer`

For direct `render_pet` strips, use a fixed 20x12 buffer:

- render the 13x10 framed pet art inside it
- center horizontally
- leave labels outside the buffer in HTML and manifest metadata
- do not place changing tick labels inside the terminal buffer

That keeps frame dimensions stable and prevents labels from masquerading as
animation changes.

For the breath strip, render `PetPanel` in the same 20x12 buffer so the movement
comes from the real panel code.

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
- visible tick or label text

Behavior:

- all strips start paused
- clicking play advances frames at `playback.frame_duration_ms`
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

## Review Markdown

Extend `review.md` with a `## Animation Strips` section.

For each strip, include:

- title
- id
- kind
- dimensions
- frame count
- frame labels and ticks
- review prompts

Do not duplicate every frame path if it makes the document noisy. The manifest
remains the exhaustive file inventory.

## Tests

Add focused unit tests before implementation code where practical.

Manifest/export tests:

- manifest serialization includes `strips[]`
- strip kind serializes as `pet-animation`
- each strip frame appears in `artifacts[]`
- `review.md` includes an animation strip section
- generated HTML contains strip controls and a paused initial state
- HTML escaping applies to strip titles, labels, prompts, and cells

Generation tests:

- `animation_strips` returns the four named strips
- every strip has at least five frames unless explicitly documented otherwise
- every frame in a strip has the same width and height
- every generated strip frame has a non-empty text export
- breath strip includes both rest and lifted frames
- blink strip includes at least one frame containing
  `closed_blink_eyes(Species::Mech)`
- glitch strip includes a visible output change attributable to a corruption
  hit
- particle strip proves at least one particle hit before export, either by
  inspecting source rendered spans for `PaletteRoleName::Particle` or by
  asserting on a known particle glyph in the exported text

Integration tests:

- `glorp dev-preview --scenario animation --out <tempdir>` writes
  `index.html`, `manifest.json`, `review.md`, and strip frame files
- `glorp dev-preview --scenario all --out <tempdir>` writes both Slice 1 static
  frames and Slice 2 strips
- `watch` and `pets` selections do not write strip directories
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
3. Add strip artifact path helpers and writer plumbing.
4. Add `src/dev_preview/animation.rs` with the four strip fixtures.
5. Extend HTML generation with strip markup and controls.
6. Extend `preview.css` and `preview.js` for playback.
7. Extend `review.md` generation.
8. Add integration tests for `animation` and `all`.
9. Update `README.md`, `AGENTS.md`, and `CLAUDE.md` with Slice 2 usage.

## Acceptance Criteria

- `cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation`
  produces a reviewable offline bundle.
- `cargo run -- dev-preview --scenario all --out target/glorp-preview`
  includes the existing static frames and the new animation strips.
- `manifest.json` includes first-class `strips[]` metadata and artifacts for
  every strip frame.
- `index.html` displays animation strips with paused playback controls and
  frame-by-frame stepping.
- The breath strip uses the real `PetPanel` breath offset path.
- Blink, glitch, and particle strips each include a deterministic visible hit.
- Strip frame dimensions are fixed within each strip.
- Existing static preview behavior remains intact.
- Normal user-facing help still hides `dev-preview`.
- Relevant Rust tests pass, including preview integration tests.

## Deferred Slice 3

The next slice should add full watch-effect strips:

- instantiate `PetAnimator`
- seed previous and next watch view models
- render full watch frames at fixed elapsed-time steps
- apply transition effects to the ratatui buffer
- export strips for at least one feed-pulse or stage-up moment

That is the right point to consider Gauntlet stories around preview bundle
review or real TUI interaction.
