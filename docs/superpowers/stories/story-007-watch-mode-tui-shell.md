---
id: story-007
title: Watch Mode TUI Shell
status: ready
tags: tui, watch, ratatui, terminal
depends_on: [story-001, story-002, story-003, story-004, story-005, story-006]
---

As a user, I want `glorp watch` to run as a side-by-side terminal app beside my coding sessions so that I can keep my pet visible while I work.

## Acceptance Criteria

- `glorp watch` starts a Rust terminal UI that stays open until the user quits.
- The default layout is side-by-side when the terminal is wide enough.
- The left panel shows pet art, name, species/stage label, mood, age, XP progress, and fed/happiness/energy bars.
- The right panel shows today's and recent effective-token activity, source breakdown, current activity bucket, recent event/feed lines, helper status, and errors when relevant.
- The layout degrades gracefully in narrower terminal panes without overlapping text.
- Watch mode polls usage roughly every minute.
- Display/metabolism groups activity into roughly 10-minute buckets.
- Key handling includes `q` to quit, `?` for help, and a refresh key for immediate usage polling.
- Exiting restores the terminal cleanly.
- There are no tweak panels, demo controls, stage overrides, fake feed buttons, or litter pickers.

## Implementation Notes

- The UI should feel like a useful terminal app, closer to `htop` for a creature than a marketing demo.
- Keep polling and rendering separate so animation can remain responsive without hammering ccusage.
- In blocked states, the TUI should still render a calm setup/status view rather than crashing or pretending the pet ate.

## Verification

- TUI smoke tests render normal and compact layouts without panics.
- Snapshot or buffer tests verify key labels and panels are present.
- Fake provider tests verify polling updates the displayed activity.
- Terminal cleanup is covered by integration or harness tests where practical.
