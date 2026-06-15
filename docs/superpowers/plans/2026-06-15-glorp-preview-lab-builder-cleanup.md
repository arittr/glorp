# Glorp Preview Lab Builder Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Preview Lab scenarios return frame plus scenario contract together instead of relying on frame-id pattern matching.

**Architecture:** Add a small `PreviewScenarioBundle` builder type that holds `PreviewFrame` plus scenario metadata. Migrate one scenario family at a time, preserving manifest order and every existing artifact path.

**Tech Stack:** Rust 2021, serde manifest export, existing Preview Lab command, integration tests.

---

## Dependency Gate

```bash
test -f src/presentation/scene.rs
test -f src/presentation/props.rs
cargo test --test dev_preview --features dev-preview
cargo test --test watch_presentation_adapter --features dev-preview
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/dev_preview/scenarios.rs` | Modify | Add `PreviewScenarioBundle` and migrate scenario metadata out of frame-id matching. |
| `src/dev_preview/watch.rs` | Modify | Return bundles for watch scenarios after the builder exists. |
| `src/dev_preview/round.rs` | Modify | Return bundles for round scenarios after watch migration proves the type. |
| `tests/dev_preview.rs` | Modify | Assert manifest order and artifact paths remain stable. |

## Task 1: Add Manifest Stability Guard

**Files:**
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Add path stability test**

Add:

```rust
#[test]
fn dev_preview_manifest_paths_remain_stable_during_builder_cleanup() {
    let run = PreviewRun::new();
    run.run_success("all");
    let manifest = run.manifest();

    for (id, expected_text) in [
        ("watch-wide-normal", "frames/watch-wide-normal.txt"),
        ("watch-species-dialect-glitch", "frames/watch-species-dialect-glitch.txt"),
        ("round-normal", "frames/round-normal.txt"),
        ("pet-species-stage", "frames/pet-species-stage.txt"),
        ("habitat-props-catalog", "frames/habitat-props-catalog.txt"),
    ] {
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["files"]["text"], expected_text);
    }

    let ids = scenario_ids(&manifest);
    assert_eq!(ids.first().unwrap(), "watch-wide-normal");
    assert!(ids.contains(&"round-normal".to_string()));
}
```

- [ ] **Step 2: Run test**

```bash
cargo test --test dev_preview --features dev-preview dev_preview_manifest_paths_remain_stable_during_builder_cleanup
```

Expected: PASS before code changes.

- [ ] **Step 3: Commit guard**

```bash
git add tests/dev_preview.rs
git commit -m "test: guard Preview Lab manifest stability"
```

## Task 2: Add Bundle Type Without Migration

**Files:**
- Modify: `src/dev_preview/scenarios.rs`

- [ ] **Step 1: Add bundle struct**

Add near `PreviewRenderContext`:

```rust
pub struct PreviewScenarioBundle {
    pub frame: PreviewFrame,
    pub scenario: PreviewScenario,
}

impl PreviewScenarioBundle {
    pub fn from_frame(frame: PreviewFrame, ctx: &PreviewRenderContext) -> Self {
        let scenario = scenario_metadata(&frame, ctx);
        Self { frame, scenario }
    }
}
```

- [ ] **Step 2: Run scenario tests**

```bash
cargo test --features dev-preview dev_preview::scenarios
```

Expected: PASS.

- [ ] **Step 3: Commit bundle type**

```bash
git add src/dev_preview/scenarios.rs
git commit -m "refactor: add Preview Lab scenario bundle"
```

## Task 3: Use Bundles Internally While Preserving Metadata

**Files:**
- Modify: `src/dev_preview/scenarios.rs`

- [ ] **Step 1: Convert frames to bundles in `generate_preview_bundle`**

After all frames are collected and before writing artifacts, add:

```rust
let bundles = frames
    .into_iter()
    .map(|frame| PreviewScenarioBundle::from_frame(frame, &ctx))
    .collect::<Vec<_>>();
let frames = bundles.iter().map(|bundle| bundle.frame.clone()).collect::<Vec<_>>();
```

Replace:

```rust
let scenarios = frames
    .iter()
    .map(|frame| scenario_metadata(frame, &ctx))
    .collect();
```

with:

```rust
let scenarios = bundles.iter().map(|bundle| bundle.scenario.clone()).collect();
```

Derive `Clone` for `PreviewScenario` if it is not already cloneable.

- [ ] **Step 2: Run dev-preview tests**

```bash
cargo test --test dev_preview --features dev-preview
```

Expected: PASS.

- [ ] **Step 3: Commit internal bundle use**

```bash
git add src/dev_preview/scenarios.rs
git commit -m "refactor: build Preview Lab scenarios from bundles"
```

## Task 4: Migrate Round Scenario Metadata First

**Files:**
- Modify: `src/dev_preview/round.rs`
- Modify: `src/dev_preview/scenarios.rs`

- [ ] **Step 1: Add round bundle constructor**

In `src/dev_preview/round.rs`, add:

```rust
pub fn round_bundles(ctx: &PreviewRenderContext) -> Vec<crate::dev_preview::scenarios::PreviewScenarioBundle> {
    round_frames(ctx)
        .into_iter()
        .map(|frame| crate::dev_preview::scenarios::PreviewScenarioBundle::from_frame(frame, ctx))
        .collect()
}
```

- [ ] **Step 2: Use round bundles in `generate_preview_bundle`**

In the `Round` selection arm, call `round_bundles(&ctx)` and append both frames and scenarios through the bundle path. Keep existing frame order.

- [ ] **Step 3: Run round Preview Lab tests**

```bash
cargo test --test dev_preview --features dev-preview dev_preview_round_writes_manifest_cells_and_round_metadata
cargo test --test dev_preview --features dev-preview dev_preview_manifest_paths_remain_stable_during_builder_cleanup
```

Expected: PASS.

- [ ] **Step 4: Commit round bundle migration**

```bash
git add src/dev_preview/round.rs src/dev_preview/scenarios.rs
git commit -m "refactor: migrate round Preview Lab scenarios to bundles"
```

## Final Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features dev-preview
cargo run -- dev-preview --scenario all --out target/glorp-preview
git status --short --branch
```

Expected: all commands pass and git status is clean after the final commit.

## Stop Conditions

- Stop if manifest scenario order changes.
- Stop if any artifact path changes.
- Stop if this cleanup makes Preview Lab artifacts less explicit.

