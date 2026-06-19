# glorp

A terminal pet fed by real Claude Code and Codex token usage.

It lives in your shell, hatches from a local seed, and grows from the work you actually do. No manual feeding — when you ship more code, your pet evolves.

## Install

```bash
npm install -g @arittr/glorp
glorp init
glorp watch
```

The npm package bundles the native binary for your platform. Glorp's canonical
usage provider is `agentsview`; install it separately and make sure
`agentsview` is on `PATH`, or set `GLORP_AGENTSVIEW_BIN` to the executable
path. Glorp counts cached input fully so its visible totals match
Tokenmaxxing-style token totals.

| Var | Purpose |
|---|---|
| `GLORP_AGENTSVIEW_BIN` | Pin a specific `agentsview` binary for canonical Tokenmaxxing-compatible usage. |

### Native macOS companion

On macOS, `glorp companion` opens the Dock-visible Glorp companion app. The
companion is a quiet round pet window for a normal display; detailed usage
diagnostics remain in `glorp watch`, `glorp status`, and `glorp doctor`.

## Privacy

Glorp is local-only. No telemetry, no upload, no transcripts. It never stores prompt text, response text, tool-call payloads, or source files — only normalized numeric usage metadata.

## More

Source, documentation, and issues: https://github.com/arittr/glorp
