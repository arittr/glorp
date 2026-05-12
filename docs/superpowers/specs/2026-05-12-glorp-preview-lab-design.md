# Glorp Preview Lab

Date: 2026-05-12

## Overview

Build a developer-only preview loop for Glorp's terminal UI and pet art. The
preview lab renders deterministic Glorp states into durable artifacts that both
humans and agents can inspect: terminal text, ANSI-colored captures, structured
cell data, and a browser-friendly HTML contact sheet with frame playback.

This exists because Glorp is a Rust `ratatui` TUI. Browser-centric development
tools cannot directly see or inspect it the way they can inspect a TypeScript
web app through Chrome. The repo already has the useful foundation: deterministic
fixture `WatchViewModel`s, direct `render_pet` access, and `ratatui::TestBackend`
buffer tests. The preview lab turns those internal render surfaces into a fast,
repeatable visual review artifact.

The preview lab is not a test framework replacement. It is the inner design loop:
cheap enough to run after every visual tweak, rich enough for Codex or another
agent to critique, and structured enough for Gauntlet to consume later.

## Goals

- Provide a fast Glorp-native visual iteration loop for TUI layout, pet art,
  animation, compact behavior, and error states.
- Render full watch UI scenarios through the same `ratatui` layout code used by
  `glorp watch`.
- Render pet-art matrices directly through the pet renderer so all species,
  stages, moods, morph variants, and animation ticks can be scanned quickly.
- Produce a single artifact bundle that works for human inspection and agent
  review.
- Include animation previews as frame strips and HTML playback, without requiring
  a live terminal recording for the first useful version.
- Keep Gauntlet adjacent: preview artifacts should be easy for Gauntlet stories
  to review, but Gauntlet should not be required for every visual iteration.
- Avoid new third-party dependencies.

## Non-Goals

- Do not change Glorp product behavior, pet mechanics, ingestion, persistence,
  or user-facing watch semantics.
- Do not make Gauntlet a runtime or development dependency of Glorp.
- Do not introduce a browser app with its own state model. The HTML output is a
  static artifact viewer generated from Glorp render data.
- Do not require live `ccusage` or `ccusage-codex` data for previews.
- Do not add screenshot/video generation in the first version. Browser screenshots
  and real terminal recordings can be layered on top of the generated HTML later.
- Do not stabilize the preview command as a public CLI API.

## Command Surface

Add a hidden developer command:

```bash
cargo run -- dev-preview
```

Optional flags:

```bash
cargo run -- dev-preview --out target/glorp-preview
cargo run -- dev-preview --scenario watch
cargo run -- dev-preview --scenario pets
cargo run -- dev-preview --scenario all
```

The default is `--scenario all` and `--out target/glorp-preview`.

The command is compiled into the binary but hidden from normal `glorp --help`.
It is intentionally documented only in developer docs and specs. Hidden-but-built
is simpler than `cfg(debug_assertions)` because release builds, CI jobs, and
agent worktrees can all use the same command when needed.

## Output Bundle

Each run overwrites the output directory atomically enough for local development:
write into a temporary sibling directory, then replace the previous preview
directory when generation succeeds. If cleanup of the previous directory fails,
the command returns a clear error instead of mixing old and new frames.

Output layout:

```text
target/glorp-preview/
  index.html
  manifest.json
  review.md
  captures/
    watch-wide-normal.txt
    watch-wide-normal.ansi
    watch-wide-normal.cells.json
    pet-matrix-species-stage.txt
    pet-matrix-species-stage.ansi
    pet-matrix-species-stage.cells.json
  frames/
    glitch-s4-content/
      frame-000.txt
      frame-000.ansi
      frame-000.cells.json
      frame-004.txt
      frame-004.ansi
      frame-004.cells.json
  assets/
    preview.css
    preview.js
```

`index.html` is self-contained except for the small local `assets/` files it
references. It does not fetch remote assets.

`manifest.json` is the review contract. It lists every scenario, dimensions,
fixture inputs, generated files, frame ticks, and the intent of the scenario.
Agents should be able to answer "what am I looking at?" from the manifest
without guessing from the pixels.

`review.md` is a short human-readable entrypoint generated from the same
manifest. It links the highest-value scenarios and lists the review questions
without requiring Drew or an agent to open JSON first.

## Artifact Formats

### Text captures

`.txt` captures contain visible terminal cells with no color escape codes. They
are good for diffing line widths, truncation, rough layout, and simple terminal
inspection.

### ANSI captures

`.ansi` captures synthesize ANSI SGR escape sequences from `ratatui::Buffer`
cell styles. They should be readable with:

```bash
less -R target/glorp-preview/captures/watch-wide-normal.ansi
```

The ANSI exporter supports foreground color, background color, bold, dim, italic,
underline, and reset. Missing styles degrade to plain text. The exporter does not
try to emulate terminal cursor movement; each capture is a stable frame.

### Cell JSON

`.cells.json` preserves exact per-cell data:

```json
{
  "width": 120,
  "height": 32,
  "cells": [
    {
      "x": 0,
      "y": 0,
      "symbol": " ",
      "fg": "#f0a646",
      "bg": null,
      "modifiers": ["bold"]
    }
  ]
}
```

The JSON format is deliberately simple and not a public API. It exists so agents
and tests can check exact geometry, colors, and overlap without parsing ANSI.

### HTML contact sheet

The HTML viewer renders terminal cells as positioned monospace spans inside
fixed-size terminal panels. It supports:

- scenario grouping
- width and height labels
- source metadata pulled from `manifest.json`
- side-by-side still contact sheets
- frame-strip views for animation
- playback controls for animation groups: play, pause, previous frame, next frame,
  and speed

The HTML should favor inspection over decoration: dense labels, stable panel
sizes, no marketing layout, no animated CSS flourishes beyond the actual terminal
frame playback.

## Scenario Model

Introduce a small internal scenario model in a new dev-preview module. A scenario
is either a full watch render or a pet-art render.

```rust
enum PreviewScenario {
    Watch(WatchPreview),
    PetMatrix(PetMatrixPreview),
}
```

`WatchPreview` renders a `WatchViewModel` through
`render_watch_frame_with_capability` using `ratatui::TestBackend`.

`PetMatrixPreview` renders one or more pets directly through `render_pet`, then
packs the outputs into a terminal-style grid for artifact export.

Both scenario types produce the same intermediate `PreviewFrame`:

```rust
struct PreviewFrame {
    id: String,
    title: String,
    width: u16,
    height: u16,
    tick: Option<u64>,
    cells: Vec<PreviewCell>,
}
```

This shared frame format is the important boundary. It keeps watch previews,
pet matrices, animation strips, HTML playback, and future Gauntlet review pointed
at the same evidence.

## Watch UI Scenarios

The first version renders these full watch scenarios:

- `watch-wide-normal`: 120x32, healthy helpers, representative token usage.
- `watch-wide-large-values`: 120x32, large token counts and long source values.
- `watch-wide-blocked-helper`: 120x32, one ready source and one blocked source.
- `watch-wide-long-name`: 120x32, pet name long enough to exercise truncation.
- `watch-compact-normal`: 72x24, healthy compact layout.
- `watch-compact-short`: 72x12, height-constrained compact layout.
- `watch-tiny`: 48x8, severe constraint smoke case.
- `watch-help-overlay`: 120x32, help overlay visible.
- `watch-evolution-overlay`: 120x32, evolution overlay visible.

These scenarios are built from fixture view models, not live state. The fixture
builders should live near the dev-preview code unless an existing test fixture is
already reusable without making production modules test-only.

The watch scenarios force `ColorCapability::Truecolor` by default so contact
sheets are stable. A separate `watch-flat-color` scenario exercises the flat
fallback.

## Pet-Art Matrix Scenarios

The pet-art preview directly renders the pet module rather than the full TUI.
It should include:

- all six species from `Species::all()`
- all seven stages, `S0` through `S6`
- representative moods: happy, content, hungry, sad, sleepy, wilted
- morph variants for adult/final stages using `morph_count`
- animation ticks that reveal breathing, blinking, glitch corruption, particles,
  and evolution flash frames

Initial matrix pages:

- `pet-species-stage`: species rows by stage columns, content mood, tick 0.
- `pet-mood`: species rows by mood columns, one mature stage, tick 0.
- `pet-adult-morphs`: species rows by adult morph variants for stages S4/S5/S6.
- `pet-animation-strips`: selected species and stages rendered at ticks
  `0, 4, 8, 12, 16`.
- `pet-evolution-flash`: selected before/after transitions using the existing
  evolution-flash render path.

The matrix renderer should label each cell with species, stage label, mood, morph
index, and tick. It should keep the labels outside the rendered pet cell so label
text does not obscure the terminal art being judged.

Pet matrices must use the same palette-role mapping as the watch TUI. If the
existing role-to-style conversion is private to a panel renderer, expose a small
internal helper and reuse it. Do not copy a second color mapping into the preview
module.

## Animation Preview

Animation is included as pre-rendered frames, not a live terminal process.

There are two animation tiers in the first implementation:

- Renderer-level pet animation: breathing, blinking, glitch corruption,
  particles, and evolution-flash art produced by `render_pet`.
- Watch-effect animation: at least one feed-pulse or stage-up strip produced by
  rendering a full watch frame, instantiating `PetAnimator`, and applying the
  effect to the `ratatui` buffer across deterministic elapsed-time steps.

Frame strip rules:

- Use deterministic ticks for comparison. The default strip ticks are
  `0, 4, 8, 12, 16`.
- For glitch species, include at least one longer strip where corruption is
  visible.
- For blink behavior, include a strip known to hit closed-eye frames for at
  least one species.
- For full watch scenarios, animation strips should cover representative effects
  rather than every layout scenario.
- Watch-effect strips use fixed elapsed-time steps, such as
  `0ms, 80ms, 160ms, 240ms, 320ms, 400ms`, so effect playback is stable in tests.

HTML playback rules:

- Playback swaps pre-rendered frames in place.
- The panel dimensions stay fixed across all frames in a group.
- A frame counter and tick label are visible.
- Playback starts paused by default. The user or agent can step frames precisely.

This does not validate the real crossterm event loop or terminal clearing. That
belongs to a later real-terminal capture pass.

## Agent Review Loop

The preview bundle should be designed for Codex and other agents to inspect.
The generated `index.html` and `manifest.json` are the primary review inputs.

Each manifest scenario includes:

- `id`
- `kind`: `watch` or `pet-matrix`
- `title`
- `intent`
- `dimensions`
- `files`
- `inputs`: species, stage, mood, tick, color capability, and fixture name when
  applicable
- `review_prompts`: short questions the reviewer should answer

Example review prompts:

- "Does the compact layout preserve pet-first hierarchy?"
- "Are helper failures visible without overwhelming the pet?"
- "Do any species/stage silhouettes collapse into the same read?"
- "Do animation strips jump, clip, or resize between frames?"
- "Are long names and large token values handled without overlap?"

The preview command should also write `review.md`, a short generated checklist
with links to the most important panels. This gives Drew and agents a readable
entrypoint without hand-opening JSON first.

## Gauntlet Relationship

Gauntlet remains the outer loop.

Preview lab responsibilities:

- Generate deterministic visual evidence quickly.
- Make evidence inspectable in Browser and by agents.
- Cover visual breadth: species, stages, moods, terminal sizes, blocked states,
  overlays, and animation frame strips.

Gauntlet responsibilities:

- Drive the real `glorp watch` TUI through its `tui` adapter.
- Verify keyboard behavior, overlays, terminal redraw, real event-loop timing,
  and full-screen behavior inside tmux.
- Review preview bundles as static artifacts when an autonomous visual critique
  is desired.

Do not add `.gauntlet/` stories in the first implementation unless the preview
bundle itself is already working. When added, those stories should consume the
generated preview output or run the real TUI as a separate acceptance pass.

## Code Organization

New code:

- `src/commands/dev_preview.rs` - command entrypoint and option handling.
- `src/dev_preview/mod.rs` - scenario registry and orchestration.
- `src/dev_preview/frame.rs` - `PreviewFrame`, `PreviewCell`, buffer conversion.
- `src/dev_preview/export.rs` - text, ANSI, cell JSON, manifest, and HTML writers.
- `src/dev_preview/watch.rs` - watch view-model fixtures and watch rendering.
- `src/dev_preview/pets.rs` - pet matrix scenarios.
- `src/dev_preview/templates.rs` - static HTML, CSS, and JS strings.

Existing code touched:

- `src/cli.rs` - add hidden `DevPreview` subcommand with `--out` and `--scenario`.
- `src/commands/mod.rs` - expose `dev_preview`.
- `src/lib.rs` - route the hidden command.
- `src/tui/app.rs` or `src/tui/layout.rs` only if a test-only render helper must
  become a normal internal helper.

The implementation should prefer reusing existing render functions over adding
parallel visual logic. If a preview cannot be produced without duplicating layout
behavior, the layout should expose a small internal render helper instead.

## Testing Strategy

Unit tests:

- Buffer-to-cell conversion preserves symbols, coordinates, colors, and modifiers.
- ANSI export resets styles at line boundaries.
- Text export preserves geometry for fixed-width rows.
- Manifest generation lists every generated file.
- Pet matrix scenario count matches species, stage, and mood expectations.

Integration tests:

- `glorp dev-preview --out <tempdir> --scenario watch` writes `index.html`,
  `manifest.json`, and at least one `.txt`, `.ansi`, and `.cells.json` capture.
- `glorp dev-preview --out <tempdir> --scenario pets` writes pet matrix captures
  that include every species name and all stage labels.
- Generated HTML references only local files present in the output directory.
- Re-running the command replaces stale output rather than mixing old and new
  scenario files.

Manual verification:

```bash
cargo run -- dev-preview
open target/glorp-preview/index.html
less -R target/glorp-preview/captures/watch-wide-normal.ansi
cargo test
```

No test should depend on the developer's real Glorp state or real token usage.

## Future Extensions

These are intentionally out of the first implementation but fit the design:

- A real terminal capture command that runs `glorp watch` in a PTY or tmux session
  for a few seconds and stores frame captures.
- Gauntlet stories that review `target/glorp-preview/index.html` through the web
  adapter or drive `glorp watch` through the TUI adapter.
- Browser screenshots of the generated contact sheet for sharing in issues or
  pull requests.
- Snapshot comparison between two preview bundles.
- A small "open preview" convenience command after generation.

## Acceptance Criteria

- A developer can run `cargo run -- dev-preview` from the repo root and inspect
  a generated `target/glorp-preview/index.html`.
- The bundle includes full watch UI previews and pet-art matrix previews.
- At least one renderer-level pet animation strip and one watch-effect animation
  strip are visible in the HTML and step-able frame by frame.
- The manifest describes every scenario well enough for an agent to review it
  without guessing the fixture intent.
- The command does not read or mutate user Glorp state.
- The command has no dependency on Gauntlet, live helpers, or network access.
- Existing product commands and normal `glorp --help` output remain focused on
  the user-facing app.
