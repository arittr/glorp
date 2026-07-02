# glorp

A terminal pet fed by real Claude Code and Codex token usage.

It lives in your shell, hatches from a local seed, and grows from the work you actually do. No manual feeding — when you ship more code, your pet evolves.

## Install

```bash
npm install -g @arittr/glorp
glorp init
glorp watch
```

The npm package bundles the native binary and usage helpers for your platform.
Glorp's default provider is bundled `ccusage`, so a normal npm install is
enough to hatch and watch the pet. Glorp counts cached input fully for
Tokenmaxxing-style token totals.
Stages grow from calibrated Tokenmaxxing `total_tokens`: Glorp compares new work against your recent active-day baseline. Early stages are active-hour equivalents, not real-time locks, and historical usage calibrates a newborn pet without feeding it.

| Var | Purpose |
|---|---|
| `GLORP_CCUSAGE_BIN` | Pin a specific `ccusage` binary. |
| `GLORP_CCUSAGE_CODEX_BIN` | Pin a specific `ccusage-codex` binary. |
| `GLORP_AGENTSVIEW_BIN` | Pin an optional `agentsview` binary for Tokenmaxxing parity checks. |

### Native macOS companion

On macOS, `glorp companion` opens the Dock-visible Glorp companion app. The
companion is a quiet round pet window for a normal display; detailed usage
diagnostics remain in `glorp watch`, `glorp status`, and `glorp doctor`.

## Privacy

Glorp is local-only. No telemetry, no upload, no transcripts. It never stores prompt text, response text, tool-call payloads, or source files — only normalized numeric usage metadata.

## More

Source, documentation, and issues: https://github.com/arittr/glorp
