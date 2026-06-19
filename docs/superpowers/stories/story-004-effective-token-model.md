---
id: story-004
title: Effective Token Model
status: ready
tags: usage, metabolism, xp, cache
depends_on: [story-001, story-002]
---

> Legacy note: current canonical usage accounting is Tokenmaxxing-compatible `agentsview` total tokens. This story describes the original ccusage/effective-token MVP behavior.

As a user, I want Glorp to respond to meaningful token activity rather than raw cache churn so that my pet grows from real work without speedrunning its lifecycle.

## Acceptance Criteria

- Glorp computes `effective_tokens` from normalized provider token buckets.
- The formula includes uncached input, output, cache creation/write, and a small weighted contribution from cache reads.
- The initial cache-read weight is configurable and defaults within the 0.02 to 0.05 range.
- Cost is display-only and never affects food, vitals, XP, mood, or evolution.
- Recent effective-token deltas can be grouped into food/activity buckets for the pet loop.
- A provider record with only cache reads produces much less food/XP than a record with the same number of uncached or output tokens.
- Unknown or missing token buckets default conservatively without crashing.

## Implementation Notes

- The design intentionally rejects raw total tokens as the direct pet score because local Claude usage can be mostly cache reads.
- Keep the formula explicit and testable rather than burying weights inside UI code.
- Preserve provider-specific token buckets in normalized records even when the pet only consumes effective totals.

## Verification

- Unit tests cover formula examples for Claude-style and Codex-style token buckets.
- Tests prove cache reads do not count 1:1.
- Tests prove cost changes do not change effective-token output.
- Tests cover missing bucket fields and unknown providers.
