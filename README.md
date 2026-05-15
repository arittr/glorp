# glorp

[![npm version](https://img.shields.io/npm/v/%40arittr%2Fglorp?logo=npm&label=npm)](https://www.npmjs.com/package/@arittr/glorp)
[![npm downloads](https://img.shields.io/npm/dm/%40arittr%2Fglorp?logo=npm&label=downloads)](https://www.npmjs.com/package/@arittr/glorp)
[![CI](https://img.shields.io/github/actions/workflow/status/arittr/glorp/ci.yml?branch=main&logo=github&label=ci)](https://github.com/arittr/glorp/actions/workflows/ci.yml)
[![License](https://img.shields.io/npm/l/%40arittr%2Fglorp)](LICENSE)

A terminal pet fed by real Claude Code and Codex token usage.

It lives in your shell, hatches from a local seed, and grows from the work you actually do. No manual feeding, no fake metrics — when you ship more code, your pet evolves.

<img width="1179" height="779" alt="Screenshot 2026-05-13 at 4 36 53 PM" src="https://github.com/user-attachments/assets/bd6052e7-0f8f-41de-b21c-1740f4a0cf48" />

## Privacy

Glorp is local-only. No telemetry, no upload, no transcripts. The pet never stores prompt text, response text, tool-call payloads, or source files — only normalized numeric usage metadata that the renderer needs.

## Install

```bash
npm install -g @arittr/glorp
glorp init
glorp watch
```

The npm package bundles the native binary for your platform plus the `ccusage` helpers.

### From source

```bash
cargo install --path .
glorp doctor
```

When installing from source, make sure `ccusage` and `ccusage-codex` are on `PATH`. `glorp doctor` will tell you what's missing.

## Quickstart

```bash
glorp init                    # hatch your first pet
glorp watch                   # open the live terminal pet
glorp status                  # one-shot summary, pipe-friendly
```

`init` derives traits from a seed. The same seed always grows the same pet — pass `--seed mochi-7f3a` for reproducibility, or let glorp generate one.

## Commands

| Command | What it does |
|---|---|
| `glorp init [--seed S] [--name N] [--yes]` | Create local state and hatch the first pet. |
| `glorp watch` | Run the live terminal pet beside your coding session. |
| `glorp status` | Print a compact summary: stage progress, usage confidence, helper health. |
| `glorp rename <name>` | Rename the pet without changing seed-derived traits. |
| `glorp reset --yes` | Clear pet state after confirmation. Usage DB is preserved. |
| `glorp doctor` | Check config paths, helper availability, parser health, recent diagnostics. |

### Design previews

For local design review, Glorp has a hidden preview lab command:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

Use `--scenario watch` for the watch layouts, `--scenario props` for the habitat
prop catalog and prop-density watch variants, `--scenario pets` for the
all-species/all-stage pet matrix, or `--scenario all` for the whole bundle. The
bundle also writes `manifest.json`, `review.md`, local assets, and
`frames/*.txt` / `frames/*.cells.json` captures for agent review.

### Watch keys

- `q` quit
- `r` refresh usage and pet state
- `?` toggle help overlay

## How it works

Glorp polls `ccusage` and `ccusage-codex` every ten seconds, diffs the running totals against a saved cursor, and turns positive deltas into pet food. Each delta is smeared across 6–12 ten-minute buckets so a heavy hour of coding doesn't crush a single tick.

Stages are gated by **calibrated XP**: roughly "one active day at your typical pace." A 500M-token/day user and a 50k-token/day user evolve at the same wall-clock cadence.

```
S0 fluff   →  S1 fuzzling  →  S2 kit     →  S3 pup       →  S4 fuzz   →  S5 archfuzz  →  S6 mythic-fuzz
 ~1 hour     ~6 hours        ~1 day         ~4 days          ~2 weeks      ~2 months       (sage)
```

(That's the `fuzz` species arc — each of the six species — fuzz, blob, ghost, glitch, crystal, mech — has its own stage names and silhouettes.)

## Configuration

Config lives at `~/.config/glorp/config.toml` (or `$GLORP_CONFIG_DIR/config.toml`).

```toml
# How much to weight cache_read tokens when computing effective tokens.
# Default 0.03 reflects that cache reads are real work but cheap.
cache_read_weight = 0.03
```

### Environment variables

| Var | Purpose |
|---|---|
| `GLORP_CONFIG_DIR` | Override `~/.config/glorp/` (handy for sandboxing). |
| `GLORP_CCUSAGE_BIN` | Pin a specific `ccusage` binary. |
| `GLORP_CCUSAGE_CODEX_BIN` | Pin a specific `ccusage-codex` binary. |
| `GLORP_NODE_BIN` | Pin a specific `node` binary for JS helpers. |

### Cost display

Glorp surfaces cost figures from helper output as **display-only metadata**. Your provider's billing dashboard remains the source of truth for invoices, credits, discounts, and final billing totals. Cost never affects food, XP, mood, or stage progression.

## Releasing

Glorp publishes through GitHub Actions, not from a local `npm publish` at the
repo root. The root `package.json` is only workspace glue; the publish workflow
builds the five native platform packages first, publishes those, then publishes
`@arittr/glorp` after its optional dependencies can resolve.

Before tagging a release:

```bash
node scripts/bump-npm-version.mjs X.Y.Z
node scripts/assert-release-version.mjs --tag vX.Y.Z
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
npm test
```

Then commit the version bump, create an annotated `vX.Y.Z` tag, and push the
commit and tag. Pushing the tag runs `.github/workflows/publish.yml`, which
re-runs the release-version assertion against the tag, tests Rust on
Ubuntu/macOS/Windows, builds the native release binaries, smoke-tests the npm
launcher, and publishes with npm trusted publishing/provenance.

For a no-publish rehearsal, run the `publish` workflow manually with the default
`dry_run` input. That exercises the test/build/smoke matrix but skips the npm
publish jobs.

## Acknowledgments

Visual design ported from the [Tokenpet](docs/tokenpet/) mockup. Inspired by Tamagotchis, dotfiles, and the perpetual question of whether your tools are alive.

## License

MIT
