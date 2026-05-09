# Glorp Story Cards

These story cards break the approved Glorp design into buildable product slices.
They are intentionally lighter than a Brainstorm `BUILD.md`: the cards are the
planning surface, and an implementation plan can order them later.

Source spec:

- `docs/superpowers/specs/2026-05-08-glorp-design.md`

## Story Index

- `story-001-usage-provider-ccusage.md` - read real token usage through ccusage-family commands.
- `story-002-local-persistence.md` - store pet state, usage cursors, events, and aggregates under `~/.config/glorp/`.
- `story-003-init-and-generated-pet.md` - initialize one deterministic generated pet from a seed.
- `story-004-effective-token-model.md` - convert provider token buckets into effective food and XP.
- `story-005-calibration-and-evolution.md` - calibrate to the user and drive the 7-stage companion arc.
- `story-006-mood-decay-and-wilted-state.md` - model recoverable hunger, mood, decay, and wilted state.
- `story-007-watch-mode-tui-shell.md` - run the side-by-side terminal app with polling and key handling.
- `story-008-pet-renderer-and-animation.md` - render deterministic species art and terminal-realistic animation.
- `story-009-status-doctor-and-errors.md` - provide status, doctor, blocked states, and friendly diagnostics.
- `story-010-npm-rust-packaging.md` - package the Rust app with bundled ccusage helpers for npm distribution.

## Card Contract

Acceptance criteria are the contract for implementation. Implementation notes
describe useful shape and tradeoffs, but if notes conflict with acceptance
criteria, the acceptance criteria win.
