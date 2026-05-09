---
id: story-006
title: Mood Decay And Wilted State
status: ready
tags: mood, decay, vitals, wilted
depends_on: [story-002, story-004, story-005]
---

As a user, I want Glorp to feel cared for by my real coding rhythm without punishing normal breaks so that the pet encourages daily use without becoming a guilt machine.

## Acceptance Criteria

- Recent effective-token activity improves vitals such as fed, happiness, and energy.
- Inactivity decays vitals according to the user's learned rhythm when enough calibration data exists.
- Same-day gaps do not cause severe penalties.
- Overnight and late-night periods decay slower than the user's normal active hours.
- Weekends decay slower while still allowing weekend-heavy users to have a learned weekend rhythm.
- Historically inactive periods decay very slowly.
- Sustained absence relative to the user's rhythm can progress through hungry, sad, and wilted states.
- Wilted is the floor state and is fully recoverable through real usage.
- MVP has no death, graveyard, revive penalty, or permadeath transition.

## Implementation Notes

- Use conservative defaults until enough usage history exists. Defaults can be friendly: hungry after a long no-work gap, wilted only after sustained absence.
- Decay should be visible in the TUI, but it should not dominate over the pet's playful personality.
- User interaction such as "pet" can affect affection or short-lived mood, but it is not food.

## Verification

- Time-travel tests cover active hours, overnight, weekends, and historically inactive windows.
- Tests prove same-day breaks do not jump directly to wilted.
- Tests prove wilted recovers after new effective-token activity.
- Tests assert that no death-state transition exists.
