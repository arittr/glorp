---
id: story-008
title: Pet Renderer And Animation
status: ready
tags: renderer, ascii, animation, species
depends_on: [story-003, story-005, story-006, story-007]
---

As a user, I want my Glorp to look distinctive and alive in the terminal so that it feels like a creature rather than a stats widget.

## Acceptance Criteria

- The renderer produces deterministic ASCII art from pet seed, species, stage, mood, and trait selections.
- MVP species include fuzz, blob, ghost, glitch, crystal, and mech.
- Each species has species-specific stage labels and silhouettes across the seven-stage arc.
- Seeded variation affects visible traits such as eyes, mouth, pattern/accent, morph, palette, and animation phase.
- Same-seed rendering is stable across runs.
- Different seeds within the same species can produce visibly different pets.
- The renderer supports terminal-realistic breathing/blinking or equivalent subtle idle motion.
- Species can have small distinct animation flavor when it is practical in a terminal.
- Mood states affect expression or posture, including wilted.
- Evolution can produce a simple terminal celebration moment when a stage changes.
- Rendering works in normal and compact TUI layouts without text overlap.

## Implementation Notes

- Preserve the prototype's spirit, not its HTML/CSS implementation.
- CSS-only glow, blur, and web particle effects are not product requirements unless they translate cleanly to terminal redraws.
- Keep art data and rendering logic separated enough that species templates can evolve without touching game rules.

## Verification

- Golden tests or fixture snapshots cover representative species/stage/mood combinations.
- Determinism tests assert same seed and state produce same output.
- Compact layout tests verify art truncation/wrapping behavior is deliberate and readable.
- Evolution event tests verify a stage change produces a renderable celebration state.
