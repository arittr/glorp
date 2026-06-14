# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Glorp is a terminal-native pet whose growth is driven by real Claude Code / Codex token usage. It polls the `ccusage` and `ccusage-codex` CLI helpers, normalizes their output into effective-token deltas, and applies those deltas to a local SQLite-backed pet state.

It's privacy-local by design: no prompts, responses, tool payloads, transcripts, or source files are ever stored. Only normalized numeric usage metadata.

## Commands

Build & run:
```bash
cargo build --release                                       # release binary at target/release/glorp
cargo run -- watch                                          # live TUI against your real ccusage helpers
cargo run -- init --yes --seed test --name buddy            # create local state from a deterministic seed
GLORP_CONFIG_DIR=/tmp/x cargo run -- status                 # isolate config from your real pet
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html                        # inspect deterministic design previews
```

Test & lint:
```bash
cargo test                                                  # full suite (~14 binaries, ~178 tests)
cargo test --test usage_provider                            # one integration test file
cargo test --lib parse_period_start_tests                   # just unit tests in a module
cargo test -- forty_nine_daily_catchups                     # filter by test name
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings    # CI gate; must stay clean
cargo test --test dev_preview                               # preview-lab integration coverage
```

npm packaging (rarely touched):
```bash
npm test                                                    # cargo test + npm workspace smoke
node scripts/build-platform-package.mjs                     # build platform-specific npm subpackages
```

## Architecture

### Data pipeline (the load-bearing part)

A newly observed provider delta is **one coherent pet event**, idempotently detected, weighted, applied once, and rendered. The pipeline runs in this order on every poll:

1. **`provider.poll(&mut usage_store)`** (`src/usage/ccusage.rs`) — shells out to ccusage / ccusage-codex helpers, normalizes JSON into per-record raw token totals, diffs against the previous cursor, emits `UsageDelta` items each carrying a `ProviderCursorUpdate` for later cursor advance.
2. **`stage_usage_poll_deltas`** (`src/game/runtime.rs`) — smears each delta into 6–12 ten-minute buckets via `smear_catchup_delta` (`src/game/catchup.rs`) and inserts each bucket as an **unapplied ledger row** in `usage_events`. The unique partial index `(provider_delta_id, bucket_index)` guarantees idempotency.
3. **`apply_unapplied_usage`** (`src/game/runtime.rs`) — reads unapplied rows in `bucket_at ASC` order, applies them per-row to `PetState.xp` / vitals / stage transitions, runs `compact_before` for retention.
4. **`state_store.save(&state)`** — JSON write to `state.json`.
5. **`mark_events_applied_and_advance_cursors`** — single transaction that flips `applied_at`, advances `provider_cursors`, and bumps `lifetime_counters`.

This sequence is the **save boundary**: if save fails between (4) and (5), the next successful run reapplies the unapplied rows and reconciles. Do NOT call the lower-level `apply_usage_poll` from production code — it's a `#[doc(hidden)]` test wrapper that bundles all five steps.

Calibration (`glorp init`) takes a separate path: `snapshot_for_calibration` reads historical helper records, computes `CalibrationBaseline` from active-day-grouped medians, advances cursors to current totals without writing any ledger rows, and never grants pet food / XP / stage progression from history.

### Stage curve & evolution

Stage thresholds in XP units (matches `pet.jsx` STAGE_DAYS at 100k/day pace):

```
S0 → S1: 0.04   S1 → S2: 0.25   S2 → S3: 1.0   S3 → S4: 4.0   S4 → S5: 14.0   S5 → S6: 60.0
```

`apply_unapplied_usage` calls `reconcile_stage_with_xp` at its top — if `state.xp` outranks `state.stage` (e.g. after a threshold curve change between runs), missing transitions are emitted so the saved stage catches up.

### Watch TUI (`src/tui/`)

- `app.rs` owns the main loop, animation tick, key handling, and a **worker thread** that runs polls async (mpsc channel; main loop `try_recv`s results per frame so animation never blocks).
- `composer.rs` is a small lipgloss-inspired text-block composer (`pad_rows`, `join_horizontal_top`, `box_with_chrome`, `section_divider`) — ratatui's `Layout::constraints` is flex-based and doesn't snap to character grids cleanly. The composer lets us position by exact cell count.
- `layout.rs` orchestrates the watch view: rounded frame fills the terminal, two-column body (LEFT_COL=40 fixed for the pet, RIGHT_COL flexes 50–70 then becomes outer padding), content packs to the top with single-row gaps, trailing blanks fill to the bottom chrome.
- `view_model.rs` is the snapshot passed to layout. Built in `src/commands/watch.rs::build_watch_view_model` from `PetState` + recent usage events + diagnostics.

Animation acknowledgement (evolution overlay) lives on `WatchApp`, not on `WatchViewModel`, because the worker poll replaces vm wholesale every ~10s.

### Preview Lab (`glorp dev-preview`)

`glorp dev-preview` is a hidden local-development command for deterministic
design review. It does not appear in normal help output.

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

Scenarios:

- `all` renders watch + pet previews.
- `watch` renders `watch-wide-normal` (`120x32`) and
  `watch-compact-normal` (`72x24`).
- `pets` renders `pet-species-stage`, covering all six species across all
  seven stages.

The preview bundle contains:

- `index.html` — static contact sheet.
- `review.md` — manifest-driven checklist with scenario prompts.
- `manifest.json` — review contract with scenario ids, kinds, intents,
  dimensions, files, inputs, and artifact inventory.
- `frames/*.txt` — plain visible terminal cells.
- `frames/*.cells.json` — per-cell geometry/style data.

Output safety is deliberate: preview generation only overwrites a missing,
empty, or previously owned preview directory marked by `.glorp-preview` and a
manifest producer of `glorp-dev-preview`. The command builds seeded watch usage
fixtures in a scratch SQLite database under the staging directory and does not
read or write real user pet state.

### Pet rendering (`src/pet/`)

- `art.rs` holds the 11×8 grid templates per species × stage × morph, ported character-for-character from `docs/tokenpet/project/pet.jsx`. Slot markers `{eyes}`, `{pattern}`, `{mouth}`, `{accent}` get substituted at render time.
- `render.rs` wraps the 11×8 art in a 13×10 particle frame, applies role-tagged `StyledSegment` spans (which `tui/layout.rs::role_spans_for_line` turns into colored ratatui spans), applies glitch corruption for the glitch species, and frames S6 with sage sparkle top/bottom.

The **renderer is content-agnostic** — adding species or stage variation is template work in `art.rs`, not renderer changes.

### Storage layout

`AppPaths` (`src/paths.rs`) resolves to `$GLORP_CONFIG_DIR` or `~/.config/glorp/`:
- `state.json` — pet identity, xp, vitals, stage, calibration baseline, seen transitions. Loaded into `PetState`.
- `usage.sqlite` — `usage_events` ledger (with applied/unapplied lifecycle), `provider_cursors`, `daily_aggregates`, `provider_diagnostics`, `lifetime_counters`. Migrations are idempotent column-add patterns via `ensure_usage_event_column`.

Stale diagnostics (>1h old) are filtered out at view-model build time so old transient failures don't keep a source marked broken forever.

## Conventions

- Effective tokens combine `uncached_input + output + cache_creation + cache_read_weight * cache_read`. `cache_read_weight` defaults to 0.03 and is configurable in `config.toml`.
- Cost (USD) is display-only metadata — never affects food, XP, mood, or stage.
- Templates in `art.rs` must stay at exactly 11 characters wide per line. The renderer assumes this for particle frame wrapping.
- The 4 brainstorm/spec directories under `docs/` describe the design intent. `src/pet/art.rs` is the source of truth for pet templates and silhouettes (the filled-block art has diverged from `pet.jsx` and is not ported back). `docs/tokenpet/project/pet.jsx` remains the reference only for stage labels (`SPECIES_ARCS`), animation profiles (`SPECIES_ANIM`), and mood eye/mouth overrides (`EYES_BY_MOOD`) — port those from there, don't invent.

## Test isolation

Integration tests live under `tests/` and use `tempfile::tempdir()` + `GLORP_CONFIG_DIR` to isolate state. `assert_cmd` drives the real binary; provider tests use Node helper fixtures in `tests/fixtures/helpers/*.mjs`. When testing failures involving helper output, **pin both** `GLORP_CCUSAGE_BIN` and `GLORP_CCUSAGE_CODEX_BIN` — real helpers on the dev machine's PATH will otherwise leak into the test environment and confuse failure modes.

## Env vars

- `GLORP_CONFIG_DIR` — override `~/.config/glorp/`.
- `GLORP_CCUSAGE_BIN`, `GLORP_CCUSAGE_CODEX_BIN`, `GLORP_NODE_BIN` — pin specific helper binaries (mostly for tests and bundled npm installs).
