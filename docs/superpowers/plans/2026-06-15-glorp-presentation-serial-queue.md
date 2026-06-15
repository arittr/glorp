# Glorp Presentation Architecture Serial Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide the serial execution queue for the Glorp presentation architecture refactor.

**Architecture:** Execute the contract, mechanical split, presentation-domain extraction, and adapter-consolidation plans one after another on the same branch. Each plan must leave the repository green and committed before the next plan starts. Later plans contain dependency gates that require checking the merged shape from earlier plans before editing.

**Tech Stack:** Rust 2021, ratatui, serde/serde_json, Preview Lab, Cargo integration tests, macOS AppKit facade behind existing cfg gates.

---

## Serial Order

Run these plans in this exact order:

1. `docs/superpowers/plans/2026-06-15-glorp-presentation-contract-freeze.md`
2. `docs/superpowers/plans/2026-06-15-glorp-pet-panel-mechanical-split.md`
3. `docs/superpowers/plans/2026-06-15-glorp-presentation-scene-skeleton.md`
4. `docs/superpowers/plans/2026-06-15-glorp-presentation-pet-roles.md`
5. `docs/superpowers/plans/2026-06-15-glorp-presentation-room-projection.md`
6. `docs/superpowers/plans/2026-06-15-glorp-presentation-prop-wrappers.md`
7. `docs/superpowers/plans/2026-06-15-glorp-round-command-convergence.md`
8. `docs/superpowers/plans/2026-06-15-glorp-watch-adapter-migration.md`
9. `docs/superpowers/plans/2026-06-15-glorp-menubar-adapter-migration.md`
10. `docs/superpowers/plans/2026-06-15-glorp-preview-lab-builder-cleanup.md`

## Global Rules

- Do not run two plans against the same branch at the same time.
- Before starting each plan, run `git status --short --branch` and confirm the only dirty files are intentional carryover from the previous plan.
- Commit after each task inside the plan reaches its listed green checks.
- Stop when a plan's dependency gate fails; do not skip ahead.
- Preserve existing visual output unless the active plan explicitly says the diff is expected.
- Preserve all existing Preview Lab artifact paths.

## Queue Verification

- [ ] **Step 1: Confirm the queue files exist**

Run:

```bash
for plan in \
  docs/superpowers/plans/2026-06-15-glorp-presentation-contract-freeze.md \
  docs/superpowers/plans/2026-06-15-glorp-pet-panel-mechanical-split.md \
  docs/superpowers/plans/2026-06-15-glorp-presentation-scene-skeleton.md \
  docs/superpowers/plans/2026-06-15-glorp-presentation-pet-roles.md \
  docs/superpowers/plans/2026-06-15-glorp-presentation-room-projection.md \
  docs/superpowers/plans/2026-06-15-glorp-presentation-prop-wrappers.md \
  docs/superpowers/plans/2026-06-15-glorp-round-command-convergence.md \
  docs/superpowers/plans/2026-06-15-glorp-watch-adapter-migration.md \
  docs/superpowers/plans/2026-06-15-glorp-menubar-adapter-migration.md \
  docs/superpowers/plans/2026-06-15-glorp-preview-lab-builder-cleanup.md
do
  test -f "$plan" || exit 1
done
```

Expected: exits 0.

- [ ] **Step 2: Confirm final verification for every code plan**

Each code plan must end with:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features dev-preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
git status --short --branch
```

Expected: all commands pass, preview bundle generates, and git status is clean after the plan's final commit.

