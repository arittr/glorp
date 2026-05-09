---
id: story-002
title: Local Persistence Under ~/.config/glorp
status: ready
tags: persistence, privacy, sqlite, state
depends_on: []
---

As a user, I want Glorp to keep its pet state and usage cursors locally in a predictable folder so that my pet survives restarts without storing sensitive transcripts.

## Acceptance Criteria

- Glorp stores all MVP local files under `~/.config/glorp/` by default.
- Pet state includes a schema version, pet seed, generated species, accepted name, created timestamp, current stage/XP counters, vitals, and last update metadata.
- Usage storage records provider cursors, recent normalized usage events, compacted older daily/source aggregates, and pet lifetime counters.
- Detailed normalized usage events are retained for the most recent 90 days.
- Events older than the detailed window are compacted into daily/source aggregates without losing pet lifetime counters.
- Raw prompts, raw responses, tool-call payloads, copied Claude/Codex transcript content, and full local source transcript archives are never written into Glorp state.
- Persistence can be redirected in tests with a config or environment override so tests do not touch the developer's real `~/.config/glorp/`.
- Malformed or unsupported state files produce an actionable error or safe migration path; they do not silently reset the pet.

## Implementation Notes

- `state.json` is a good fit for pet state because it is small and user-inspectable.
- `usage.sqlite` is a good fit for cursors, recent events, and aggregates because watch mode needs idempotent updates.
- Include parser/provider version fields in usage tables so future parser changes can be reconciled.

## Verification

- Tests create an isolated config directory and verify all files stay inside it.
- Tests assert that normalized usage fixtures do not persist prompt/response/tool payload fields.
- Tests cover first creation, reload, schema version read, detailed retention, compaction, and malformed state handling.
