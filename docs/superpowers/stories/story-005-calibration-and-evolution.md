---
id: story-005
title: Calibration And Evolution
status: ready
tags: calibration, evolution, xp, lifecycle
depends_on: [story-002, story-004]
---

As a user, I want Glorp to grow on a companion-scale timeline calibrated to my own usage so that both heavy and light users can bond with their pet over weeks.

## Acceptance Criteria

- Historical usage calibrates the user baseline but does not retroactively feed, evolve, or grant XP to a newly initialized pet.
- A pet created by `glorp init` starts at stage 0.
- Glorp defines seven stages from newborn to final form.
- Evolution progress is based on cumulative effective XP earned after pet creation.
- The target full arc is roughly 6-8 active weeks for a user working near their calibrated pace.
- Calibration uses recent active usage history, such as rolling active-day median or percentile, rather than raw lifetime totals alone.
- Heavy bursts receive diminishing XP so one very large token day cannot skip most of the lifecycle.
- Mood or food benefits from a single bucket are capped separately from XP.
- Crossing a stage threshold creates an evolution event that can be surfaced by `glorp watch` and `glorp status`.

## Implementation Notes

- Historical data should answer "what is normal for this user?" rather than "how old should this pet already be?"
- Burst dampening can start simple. The important behavior is that enormous days feel rewarding without collapsing the 6-8 week arc.
- Stage labels should be species-specific even if the XP thresholds are shared.

## Verification

- Fixtures with large pre-init history still create stage-0 pets.
- Similar relative effort across low-usage and high-usage fixtures yields similar stage progress over simulated active weeks.
- A single extreme usage bucket produces capped progress.
- Stage threshold crossing records exactly one evolution event per stage transition.
