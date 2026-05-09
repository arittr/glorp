---
id: story-009
title: Status Doctor And Friendly Errors
status: ready
tags: cli, status, doctor, errors
depends_on: [story-001, story-002, story-003, story-004, story-005, story-006]
---

As a user, I want Glorp to explain its state and setup problems clearly so that missing usage sources do not feel mysterious or broken.

## Acceptance Criteria

- `glorp status` prints a compact non-interactive summary of current pet state, recent effective tokens, stage progress, and provider health.
- `glorp doctor` inspects config paths, state readability, helper availability, helper versions when available, provider command health, and recent parse diagnostics.
- If no supported usage source is available, Glorp shows a friendly blocked state with setup instructions and does not feed the pet.
- Missing helpers, failing helpers, unparsable JSON, and cursor errors are reported as structured diagnostics.
- Repeated provider failures keep the last known pet state and surface the issue.
- Glorp labels usage as local-log-derived or estimated where appropriate.
- Provider billing remains display-only and is never described as authoritative.
- `glorp help` documents MVP commands and TUI keys.
- No normal-mode manual `feed` command exists in MVP.

## Implementation Notes

- "Blocked but alive" is the desired failure tone: Glorp is waiting for a food source, not broken beyond repair.
- Doctor output should be useful in bug reports without leaking raw prompts, transcript content, or full copied logs.
- Prefer concise terminal output that can be pasted into an issue safely.

## Verification

- CLI tests cover status, help, and doctor output with healthy and failing fake providers.
- Missing helper fixtures produce actionable instructions.
- Diagnostics tests assert raw prompt/response fields do not appear in output.
- Status output remains pipe-friendly and exits successfully when a pet exists.
