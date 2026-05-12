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

The hidden command still ships in npm-installed binaries because the npm wrapper
forwards arguments to the native executable. Treat it as internal but reachable:
use Clap's hidden-subcommand support, keep it out of README/npm docs, and test
that normal help output omits it.

## Implementation Slices

The full preview lab is useful, but the first implementation must stay small
enough to land safely.

### Slice 1

Slice 1 is the first useful loop:

- hidden `dev-preview` command
- safe output directory ownership checks
- deterministic render context plumbing for color capability
- `watch-wide-normal`
- `watch-compact-normal`
- one `pet-species-stage` matrix
- `manifest.json`
- `review.md`
- `.txt` captures
- `.cells.json` captures
- simple static HTML contact sheet

Slice 1 explicitly does not include ANSI polish, HTML playback controls,
watch-effect animation strips, exhaustive mood/morph matrices, browser
screenshots, or Gauntlet stories.

### Later Slices

Later slices add, in this order:

- ANSI export polish
- renderer-level pet animation strips
- HTML frame playback
- watch-effect animation strips through the shared watch-preview driver
- mood and adult-morph matrices
- named animation-hit strips
- Gauntlet stories over the generated preview bundle and real TUI

## Output Bundle

Each run writes into a temporary sibling directory, then replaces the requested
output directory only when generation succeeds and the target is safe to replace.

Safe replacement rules:

- Refuse symlink output paths.
- If the output path does not exist, create it.
- If the output path exists and is empty, replace it.
- If the output path exists and is non-empty, replace it only when it is owned by
  the preview lab.
- Preview ownership requires both `.glorp-preview` and a manifest whose
  `producer` is `glorp-dev-preview`.
- Refuse to delete or overwrite any non-empty directory without those ownership
  markers.
- Refuse file output paths with a clear error.

The generator writes `.glorp-preview` into every successful output bundle. This
is a local safety marker, not a public API.

Full output layout after later slices:

```text
target/glorp-preview/
  .glorp-preview
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

Slice 1 omits `.ansi` files and `frames/`; it writes only the still captures
needed by the first contact sheet.

`index.html` is self-contained except for the small local `assets/` files it
references. It does not fetch remote assets.

`manifest.json` is the review contract. It lists every scenario, dimensions,
fixture inputs, generated files, frame ticks, and the intent of the scenario.
Agents should be able to answer "what am I looking at?" from the manifest
without guessing from the pixels.

The manifest includes top-level metadata:

```json
{
  "schema_version": 1,
  "producer": "glorp-dev-preview",
  "glorp_version": "0.1.0",
  "generated_at": "2026-05-12T00:00:00Z"
}
```

Each artifact entry includes a `type` such as `text`, `ansi`, `cells`, `html`,
or `review`. Future Gauntlet stories consume preview output through this
manifest and should not import Glorp Rust modules or treat `.cells.json` as
stable beyond the manifest schema version.

`review.md` is a short human-readable entrypoint generated from the same
manifest. It links the highest-value scenarios and lists the review questions
without requiring Drew or an agent to open JSON first.

## Artifact Formats

### Text captures

`.txt` captures contain visible terminal cells with no color escape codes. They
are good for diffing line widths, truncation, rough layout, and simple terminal
inspection.

Text captures are exported from the final `ratatui::Buffer`, not from raw pet
strings or intermediate layout data. That keeps them aligned with what the TUI
actually rendered.

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
      "display_width": 1,
      "continuation": false,
      "fg": "#f0a646",
      "bg": null,
      "modifiers": ["bold"]
    }
  ]
}
```

The JSON format is deliberately simple and not a public API. It exists so agents
and tests can check exact geometry, colors, and overlap without parsing ANSI.

Cell JSON is exported from the final `ratatui::Buffer`. The exporter records a
`display_width` and `continuation` flag so multi-width or ambiguous Unicode cells
can be represented without pretending every glyph is a single ASCII column. If
`ratatui` represents a wide glyph by writing the visible symbol into one cell and
blanking continuation cells, the preview exporter preserves that final buffer
state instead of recomputing width from `.chars().count()`.

### HTML contact sheet

The HTML viewer renders terminal frames from escaped buffer cells inside
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

HTML generation must escape every cell symbol and label (`<`, `>`, `&`, quotes)
before writing markup. The first implementation may render rows as text with
style runs or as a CSS grid keyed by cell coordinates; either way, the rendered
panel dimensions must be derived from the frame's `width` and `height`, not from
browser text flow.

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

Rendering also uses an explicit context:

```rust
struct PreviewRenderContext {
    color_capability: ColorCapability,
}
```

Before preview export lands, the TUI render path must stop detecting color
capability inside individual panels. `render_watch_frame_with_capability` should
pass the chosen capability through layout and panel rendering so `Truecolor` and
`Flat` previews are deterministic and testable. A `watch-flat-color` scenario is
only meaningful after that refactor.

## Watch UI Scenarios

The full design eventually renders these watch scenarios:

- `watch-wide-normal`: 120x32, healthy helpers, representative token usage.
- `watch-wide-large-values`: 120x32, large token counts and long source values.
- `watch-wide-blocked-helper`: 120x32, one ready source and one blocked source.
- `watch-wide-long-name`: 120x32, pet name long enough to exercise truncation.
- `watch-compact-normal`: 72x24, healthy compact layout.
- `watch-compact-short`: 72x12, height-constrained compact layout.
- `watch-tiny`: 48x8, severe constraint smoke case.
- `watch-help-overlay`: 120x32, help overlay visible.
- `watch-evolution-overlay`: 120x32, evolution overlay visible.

Slice 1 renders only:

| Scenario | Size | Purpose |
| --- | --- | --- |
| `watch-wide-normal` | 120x32 | Main two-column layout in a healthy state. |
| `watch-compact-normal` | 72x24 | Main compact layout in a healthy state. |

Watch scenarios are deterministic, but they should avoid inventing a second
product state model. Build ordinary watch scenarios from seeded `PetState` and a
temporary `UsageStore`, then call the real watch view-model builder. Mutate the
resulting `WatchViewModel` only for pure visual edge cases such as long names,
overlay visibility, cursor position, or forced source health.

The watch scenarios force `ColorCapability::Truecolor` by default so contact
sheets are stable. A separate `watch-flat-color` scenario exercises the flat
fallback.

## Pet-Art Matrix Scenarios

The pet-art preview directly renders the pet module rather than the full TUI.
The full design should include:

- all six species from `Species::all()`
- all seven stages, `S0` through `S6`
- representative moods: happy, content, hungry, sad, sleepy, wilted
- morph variants for adult/final stages using `morph_count`
- animation ticks that reveal breathing, blinking, glitch corruption, particles,
  and evolution flash frames

Slice 1 renders only:

| Matrix | Scope | Purpose |
| --- | --- | --- |
| `pet-species-stage` | 6 species x 7 stages, content mood, tick 0 | Scan the core lifecycle silhouettes quickly. |

Later matrix pages:

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
module. This helper extraction is required before Slice 1 pet matrices are
complete.

## Animation Preview

Animation is included as pre-rendered frames, not a live terminal process.

There are two animation tiers in the full preview lab:

- Renderer-level pet animation: breathing, blinking, glitch corruption,
  particles, and evolution-flash art produced by `render_pet`.
- Watch-effect animation: at least one feed-pulse or stage-up strip produced by
  rendering a full watch frame, instantiating `PetAnimator`, and applying the
  effect to the `ratatui` buffer across deterministic elapsed-time steps.

Those tiers are not part of Slice 1. Slice 1 can ship with static frames only.

Frame strip rules:

- Use deterministic ticks for comparison. The default strip ticks are
  `0, 4, 8, 12, 16`.
- For glitch species, include at least one longer strip where corruption is
  visible.
- For blink behavior, include a strip known to hit closed-eye frames for at
  least one species.
- Add named scenario-derived strips for special cases rather than relying only
  on the default ticks:
  - `blink-hit`: computed from the selected pet's blink cadence.
  - `glitch-corruption-hit`: includes a tick that triggers glitch corruption.
  - `particle-hit`: includes a tick that triggers a scan-line or particle state.
- For full watch scenarios, animation strips should cover representative effects
  rather than every layout scenario.
- Watch-effect strips use fixed elapsed-time steps, such as
  `0ms, 80ms, 160ms, 240ms, 320ms, 400ms`, so effect playback is stable in tests.
  A fresh `PetAnimator` is used per strip.

Watch-effect strip harness:

1. Build `prev_vm` and `next_vm`.
2. Set `wander_offset_x` and `breath_offset_y` explicitly on both VMs.
3. Call `PetAnimator::update(prev_vm)` to seed previous state.
4. Call `PetAnimator::update(next_vm)` to enqueue the intended transition.
5. For each elapsed-time step, render the base watch frame, compute
   `pet_panel_rect`, then call `PetAnimator::apply`.
6. Export the final `ratatui::Buffer`.

The harness should live near the watch rendering code or as a shared internal
helper so it mirrors the live `WatchApp` render order instead of forking a second
ad hoc effect pipeline inside `dev_preview`.

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
- `src/dev_preview/assets/preview.html` - static HTML shell included with
  `include_str!`.
- `src/dev_preview/assets/preview.css` - static CSS included with `include_str!`.
- `src/dev_preview/assets/preview.js` - static JS included with `include_str!`.

Existing code touched:

- `src/cli.rs` - add hidden `DevPreview` subcommand with `--out` and `--scenario`.
- `src/commands/mod.rs` - expose `dev_preview`.
- `src/lib.rs` - route the hidden command.
- `src/tui/layout.rs` and panel rendering - thread explicit color capability
  through the render path instead of letting panels detect environment state.
- `src/tui/panels/pet.rs` or a sibling module - expose a `pub(crate)` pet role
  style helper reused by both `PetPanel` and preview.
- `src/tui/app.rs` or `src/tui/layout.rs` - expose a shared internal
  watch-preview frame driver if watch-effect strips are implemented.

The implementation should prefer reusing existing render functions over adding
parallel visual logic. If a preview cannot be produced without duplicating layout
behavior, the layout should expose a small internal render helper instead.

## Testing Strategy

Unit tests:

- Buffer-to-cell conversion preserves symbols, coordinates, colors, and modifiers.
- Buffer export preserves or marks multi-width/ambiguous Unicode cells without
  recomputing geometry from raw strings.
- ANSI export resets styles at line boundaries once ANSI export is implemented.
- Text export preserves geometry for fixed-width rows.
- HTML export escapes `<`, `>`, `&`, quotes, box drawing, and multi-byte pet
  glyphs.
- Manifest generation lists every generated file.
- Pet matrix scenario count matches the configured matrix inventory; Slice 1 is
  exactly 6 species x 7 stages for `pet-species-stage`.
- Truecolor and flat-color renders are deterministic and differ only where color
  capability should change output.

Integration tests:

- `glorp dev-preview --out <tempdir> --scenario watch` writes `index.html`,
  `manifest.json`, and at least one `.txt` and `.cells.json` capture.
- `glorp dev-preview --out <tempdir> --scenario pets` writes pet matrix captures
  that include every species name and all stage labels.
- Generated HTML references only local files present in the output directory.
- Re-running the command replaces stale output rather than mixing old and new
  scenario files.
- `glorp help` does not show `dev-preview`.
- Direct `glorp dev-preview` invocation works despite being hidden.
- Non-empty output directories without `.glorp-preview` and a matching manifest
  are refused and left unchanged.
- Symlink output paths and file output paths are refused.
- Tests run with a temporary `GLORP_CONFIG_DIR` and do not read or create user
  state.

Manual verification:

```bash
cargo run -- dev-preview
open target/glorp-preview/index.html
cargo test
```

No test should depend on the developer's real Glorp state or real token usage.

## Future Extensions

These are intentionally out of the first implementation but fit the design:

- A real terminal capture command that runs `glorp watch` in a PTY or tmux session
  for a few seconds and stores frame captures.
- Gauntlet stories that review `target/glorp-preview/index.html` through the web
  adapter or drive `glorp watch` through the TUI adapter.
- ANSI export polish after the plain text, cell JSON, and HTML paths are useful.
- HTML playback controls for frame strips.
- Watch-effect animation strips through the shared watch-preview driver.
- Mood, morph, and named animation-hit pet matrices.
- Browser screenshots of the generated contact sheet for sharing in issues or
  pull requests.
- Snapshot comparison between two preview bundles.
- A small "open preview" convenience command after generation.

## Acceptance Criteria

- A developer can run `cargo run -- dev-preview` from the repo root and inspect
  a generated `target/glorp-preview/index.html`.
- The Slice 1 bundle includes `watch-wide-normal`, `watch-compact-normal`, and
  `pet-species-stage`.
- The Slice 1 bundle includes `manifest.json`, `review.md`, `.txt` captures,
  `.cells.json` captures, and a simple static HTML contact sheet.
- The output directory replacement logic refuses symlinks, file paths, and
  non-empty non-preview directories.
- Truecolor and flat-color rendering are driven by explicit render context, not
  ambient terminal environment.
- The manifest describes every scenario well enough for an agent to review it
  without guessing the fixture intent.
- The command does not read or mutate user Glorp state.
- The command has no dependency on Gauntlet, live helpers, or network access.
- Existing product commands and normal `glorp --help` output remain focused on
  the user-facing app.
