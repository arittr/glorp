# Glorp Design

Date: 2026-05-08

## Overview

Glorp is a terminal-native virtual pet for developers. It grows, reacts, and gets hungry from real AI coding token usage rather than simulated feeding or productivity checklist data. The product should feel like a small companion that lives beside active coding sessions, not a cost dashboard or a guilt machine.

The first implementation is a Rust terminal app using a `ratatui`-style side-by-side interface. Token usage comes from `ccusage`-family commands at first, with the app owning only the pet loop, state, rendering, and a narrow usage-provider boundary.

## Goals

- Build a real-usage-first pet loop powered by Claude Code and Codex usage through `ccusage`.
- Keep Glorp focused on being a pet, not a general token analytics product.
- Make the pet deterministic and personal: seed drives species, visual traits, palette, animation phase, and generated name.
- Use adaptive calibration so heavy and light users experience a similar wall-clock evolution arc.
- Preserve privacy by storing only normalized usage metadata, cursors, pet state, and aggregates.
- Keep the MVP honest: no fake feeding, no demo controls, no death state.

## Non-Goals

- Do not build a token tracker dashboard, prompt/session browser, cost optimizer, or billing source of truth.
- Do not parse raw Claude/Codex logs natively in the first implementation unless the `ccusage` path proves unworkable.
- Do not use commits, PRs, diffs, or shipping signals for MVP pet mechanics.
- Do not store raw prompts, responses, tool calls, copied transcript content, or full source transcripts.
- Do not include prototype-only controls such as tweak panels, fake feeds, stage overrides, or litter pickers.
- Do not run a daemon in MVP.

## Architecture

Glorp owns the Rust CLI/TUI, pet state, game rules, persistence, command surface, and rendering. Usage ingestion is isolated behind a small `UsageProvider` interface.

The first provider is `CcusageCommandProvider`. It shells out to bundled or PATH-discovered `ccusage` and `@ccusage/codex` JSON-producing commands, converts their structured output into normalized usage deltas, and updates Glorp cursors. This provider treats `ccusage` as the token parser and Glorp as the pet.

The rest of the application consumes only normalized `UsageDelta` records. This lets a later `NativeRustProvider` replace the subprocess provider without changing pet mechanics, TUI rendering, or stored pet state.

The app stores its local files under `~/.config/glorp/`:

- `config.toml` for user configuration.
- `state.json` for pet state, schema version, and high-level counters.
- `usage.sqlite` for usage cursors, recent normalized events, and compacted aggregates.

## Usage Provider

The provider should be able to answer one question: how many new effective tokens have appeared since the last successful poll?

Each normalized usage record should include:

- Provider surface, such as `claude-code` or `codex`.
- Source command and parser version.
- Timestamp or coarse period when available.
- Model when available.
- Token buckets: uncached input, output, cache creation/write, cache read, reasoning output when available.
- Cost only as optional local-derived display metadata.
- Confidence or source label, such as `local-log-derived`.

`ccusage` daily/session/block totals are enough to prove the loop. In watch mode, Glorp polls and diffs totals to create finer-grained food events. The product should not depend on exact historical 10-minute buckets being available from `ccusage`.

If `ccusage` is missing, misconfigured, or returns unparsable output, Glorp enters a friendly blocked state. It should not invent food. `glorp doctor` explains which helpers were found, which failed, and how to install or configure the missing dependency.

The npm distribution should bundle the needed `ccusage` packages and pass helper paths to the Rust binary. Non-npm installs can fall back to PATH discovery and clear setup instructions.

## Token Model

Glorp does not use raw total tokens as a direct XP or food source. Local evidence showed that Claude Code usage can be dominated by cache reads, so raw totals would over-reward context churn.

Glorp computes effective tokens:

```text
effective_tokens =
  uncached_input
  + output
  + cache_creation
  + cache_read_weight * cache_read
```

The initial `cache_read_weight` should be small, in the 0.02 to 0.05 range. Cache reads matter because the model is using context, but they must not count 1:1.

Recent effective-token deltas feed vitals. Cumulative effective XP drives evolution. Cost is display-only and must not affect pet state.

## Calibration

Historical usage calibrates Glorp but does not retroactively evolve or feed a pet. On `glorp init`, the first pet starts at stage 0 even if the user has extensive past usage. Past usage establishes the user-specific baseline for expected active days, active windows, and evolution pace.

The target evolution arc is 6-8 active weeks at the user's calibrated pace. A user burning millions of effective tokens per day and a user burning far less should both see a similar companion arc if they work consistently relative to their own baseline.

Glorp should use an adaptive baseline such as a rolling median active day or recent active-day percentile. Heavy bursts receive diminishing XP and capped mood benefits per bucket. A very large token day should feel like a feast, but it should not skip most of the lifecycle.

Watch mode should poll roughly every minute. Display and metabolism should group activity into roughly 10-minute buckets. On open, Glorp reconciles missed usage coarsely and smears catch-up rather than pretending exact timing is known.

## Pet Lifecycle

`glorp init` creates one generated pet from a seed. The seed determines:

- Species.
- ASCII morphology and variation slots.
- Eyes, mouth, pattern, accent, and palette.
- Animation phase offsets.
- Generated name.

The generated name should be species-aware so different species produce different-looking and different-sounding names. The user can accept the generated name or rename the pet. There is no litter picker. If the user wants a fully different pet, they use an intentional reset/reinit flow.

The MVP species are based on the prototype set:

- Fuzz.
- Blob.
- Ghost.
- Glitch.
- Crystal.
- Mech.

The pet has seven stages from newborn to final form. Stage labels and silhouettes are species-specific rather than every species sharing a generic egg-to-adult path.

## Mood And Decay

Tokens are both food and XP. Recent effective tokens improve vitals such as fed, happiness, and energy. Cumulative calibrated effective XP advances the stage arc.

Inactivity uses a gentle, user-calibrated decay model. Glorp should encourage daily use without punishing ordinary breaks:

- Same-day gaps should not feel punitive.
- Overnight and late-night hours decay slower.
- Weekends decay slower, while still allowing weekend-heavy users to have a learned rhythm.
- Historically inactive periods decay very slowly.
- Sustained absence relative to the user's rhythm can lead to hungry, sad, and finally wilted states.

The floor state is wilted: visibly sad and tired, fully recoverable with real usage. There is no death, graveyard, revive penalty, or permadeath mechanic in MVP.

## TUI

`glorp watch` is the main product surface. It should feel like a real terminal app that can run beside other coding terminals.

The UI is side-by-side when space allows:

- Left panel: pet art, generated name, species-specific stage label, mood, age, XP progress, fed/happiness/energy bars.
- Right panel: today and recent effective-token activity, source breakdown, current 10-minute bucket, recent feed/event log, helper status, and errors when relevant.

The UI should have graceful compact behavior for narrower panes. It should be rich but restrained: closer to `htop` for a creature than to a marketing page.

Animations should be terminal-realistic:

- Breathing and blinking.
- Subtle per-species movement.
- Occasional mood bubbles or event lines.
- A simple evolution moment.

CSS-only prototype effects are not requirements unless they translate cleanly to terminal redraws.

## CLI Commands

The MVP command surface is:

- `glorp init`: create local state and hatch the first pet.
- `glorp watch`: run the side-by-side TUI and poll usage.
- `glorp status`: print a compact non-interactive status summary.
- `glorp rename <name>`: rename the current pet.
- `glorp reset`: confirmed full reset of Glorp pet state.
- `glorp doctor`: inspect helper availability, config, paths, and parser health.
- `glorp help`: show commands.

Inside the TUI:

- `q`: quit.
- `?`: help.
- Refresh key for immediate usage poll.
- Optional `p`: pet the current pet as an affection action, not as food.

There is no normal-mode `feed` command in MVP because real usage is the only food source.

## Persistence And Privacy

All local Glorp files live under `~/.config/glorp/`.

The app stores:

- Pet state and schema version.
- Usage-provider cursors.
- Recent normalized usage events.
- Older compacted daily/source aggregates.
- Pet lifetime counters.

The app does not store:

- Raw prompts.
- Raw responses.
- Tool-call payloads.
- Copied Claude/Codex transcript content.
- Full local source transcript archives.

Detailed normalized usage events are retained for the most recent 90 days. Older data is compacted into daily/source aggregates. Pet lifetime counters remain.

Glorp should label usage as local-log-derived or estimated where appropriate. Provider billing remains the source of truth for actual cost.

## Error Handling

Errors should be friendly and non-fatal where possible.

- Missing helpers show a blocked setup state and actionable install instructions.
- Parse failures are diagnostics, not crashes.
- Repeated provider failures keep the last known pet state and show the issue in the TUI.
- Cursor corruption should fall back to a conservative rescan without double-feeding when possible.
- Reset requires confirmation because it replaces the active pet.

## Testing Strategy

The implementation should be testable without reading the developer's real local logs.

Core tests:

- Deterministic seed generation produces stable species, names, traits, and art selections.
- Usage normalization computes effective tokens correctly, including cache-read weighting.
- Cursor diffing does not double-feed after repeated polls.
- Historical usage calibrates but grants no retroactive XP.
- Heavy bursts get diminishing XP and capped mood benefits.
- Decay handles active hours, overnight periods, weekends, and sustained absence.
- Wilted is recoverable and no death-state transition exists.
- CLI commands read/write expected files under a test config directory.
- TUI smoke tests render normal and compact layouts without panics.
- Provider integration tests use fake `ccusage` commands and JSON fixtures.

## References

- Handoff README: `docs/tokenpet/README.md`
- Design transcript: `docs/tokenpet/chats/chat1.md`
- Primary prototype: `docs/tokenpet/project/tokenpet.html`
- `ccusage`: https://github.com/ryoppippi/ccusage
- `ratatui`: https://ratatui.rs/
- Bun single-file executable docs: https://bun.com/docs/bundler/executables
- esbuild npm binary distribution pattern: https://esbuild.github.io/getting-started/
