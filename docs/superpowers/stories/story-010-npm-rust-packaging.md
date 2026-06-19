---
id: story-010
title: npm Distribution For Rust Glorp
status: ready
tags: packaging, npm, rust, distribution
depends_on: [story-001, story-007, story-009]
---

> Legacy note: current canonical usage accounting is Tokenmaxxing-compatible `agentsview` total tokens. This story describes the original ccusage/effective-token MVP behavior.

As a user, I want to install Glorp from npm and run it without manually wiring helper paths so that trying the pet is easy even though the core app is Rust.

## Acceptance Criteria

- The npm package exposes a `glorp` command.
- The package includes or downloads the appropriate Rust binary for the user's platform using a conventional npm binary distribution pattern.
- The npm package depends on or bundles the ccusage-family helpers required by the MVP.
- The JavaScript wrapper passes explicit helper paths or environment variables to the Rust binary when bundled helpers are available.
- The Rust binary falls back to PATH discovery if wrapper-provided helper paths are absent.
- If no helper can be found, Glorp shows the friendly blocked/setup state rather than failing obscurely.
- Packaging does not require users to have Rust installed.
- Packaging does not require Glorp to be rewritten in TypeScript.

## Implementation Notes

- Use existing npm binary distribution patterns as inspiration, such as platform-specific optional packages plus a small JS launcher.
- Keep packaging separate from game logic and provider parsing.
- A Homebrew or direct GitHub release path can come later; npm is the MVP distribution target.

## Verification

- Package tests or smoke scripts verify `glorp --help`, `glorp doctor`, and helper-path detection from an npm-style install layout.
- A missing-helper package fixture verifies PATH fallback and blocked-state messaging.
- Build scripts produce a Rust binary without requiring runtime Rust on the user's machine.
