# Glorp

Glorp is a terminal-native pet fed by real Claude Code and Codex token usage. It lives in your shell, hatches from local state, and grows from normalized usage metadata instead of manual feeding.

## Privacy

Glorp is local-only. It does not send telemetry, upload usage, or store prompt text, response text, tool-call payloads, transcript copies, or source files. The pet state stores only the local data needed to render the pet, track progression, and summarize normalized token usage.

## Install

```bash
npm install -g glorp
glorp init
glorp watch
```

The npm package installs the JavaScript launcher, bundled `ccusage` helpers, and the native Glorp binary for your platform.

## Source Install

```bash
cargo install --path .
glorp doctor
```

When installing from source, make sure `ccusage` and `ccusage-codex` are available on `PATH`, or run `glorp doctor` to inspect helper availability.

## Commands

- `glorp init` creates local state, presents a generated name, and hatches your first pet.
- `glorp watch` opens the live terminal pet.
- `glorp status` prints a compact pet, stage progress, usage-confidence, and provider-health summary.
- `glorp rename <name>` renames the current pet without changing generated traits.
- `glorp reset --yes` clears local Glorp state after confirmation.
- `glorp doctor` checks config paths, helper availability, parser health, and diagnostics.
- `glorp help` prints command help.

## Watch Keys

- `q` exits watch mode.
- `?` toggles help.
- `r` refreshes usage and pet state.

## Cost Display

Glorp shows cost as local-derived display metadata from usage helper output. Your provider billing dashboard remains the source of truth for invoices, credits, discounts, and final billing totals.
