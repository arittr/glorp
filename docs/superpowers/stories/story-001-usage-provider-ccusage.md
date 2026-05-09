---
id: story-001
title: Usage Provider Through ccusage
status: ready
tags: usage, ccusage, codex, claude-code, provider
depends_on: []
---

As a user, I want Glorp to read real AI coding token usage through ccusage-family tools so that my pet feeds from actual work rather than simulated commands.

## Acceptance Criteria

- Glorp defines a provider boundary that emits normalized usage records or deltas without exposing Claude/Codex transcript internals to pet logic.
- The first provider shells out to JSON-producing `ccusage` and `@ccusage/codex` commands when those helpers are available.
- The provider supports both bundled helper paths supplied by the npm wrapper and PATH discovery for non-npm installs.
- Provider output captures source surface (`claude-code`, `codex`, or equivalent), parser command/version when available, model when available, timestamp or coarse period, token buckets, optional cost metadata, and a local-derived confidence label.
- Glorp can poll the provider repeatedly and compute "new tokens since last successful poll" without double-counting unchanged totals.
- Missing, non-zero-exiting, or unparsable helper commands return structured provider diagnostics rather than panicking.
- The provider does not read, store, or expose raw prompt text, response text, tool-call payloads, or copied transcript content.

## Implementation Notes

- Treat `ccusage` as the parser and Glorp as the pet. Do not parse Claude/Codex logs natively in this story.
- Prefer JSON output surfaces that are stable enough to diff totals. Daily/session/block summaries are acceptable for proving the loop.
- Codex support should use the `@ccusage/codex` helper path if bundled or discoverable.
- The provider should be replaceable later by a native Rust parser without changing pet mechanics.

## Verification

- Unit tests use fake helper commands that print fixture JSON for Claude Code and Codex.
- Repeated polls against the same fixture produce zero new deltas after the first cursor update.
- Helper failure fixtures produce friendly diagnostics and no usage delta.
- Fixture JSON containing prompts or unrelated transcript-like fields is ignored by normalized output.
