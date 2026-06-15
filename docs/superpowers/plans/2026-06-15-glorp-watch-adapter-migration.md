# Glorp Watch Adapter Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the watch TUI consume the shared presentation scene for semantics that are already covered by contracts, without changing layout or rendered output.

**Architecture:** Watch remains a ratatui adapter and keeps `ComponentLayout`, `PetScene`, and `PetPanel` orchestration. This plan threads a `PresentationScene` through the watch pet panel only for privacy-safe room/pet semantic decisions, while preserving old helper wrappers.

**Tech Stack:** Rust 2021, ratatui, `PresentationScene`, existing `WatchViewModel`, Preview Lab watch artifacts.

---

## Dependency Gate

```bash
test -f src/presentation/props.rs
test -f src/round/draw.rs
cargo test --test presentation_scene
cargo test --test presentation_room
cargo test --test presentation_props
cargo test --test round_command_convergence
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/tui/panels/pet.rs` | Modify | Derive a `PresentationScene` once in the render path and use it for room/pet semantic reads. |
| `src/tui/panels/pet/ambient.rs` | Modify | Accept semantic room data only where already covered by tests. |
| `tests/watch_presentation_adapter.rs` | Create | Watch adapter stability tests. |

## Task 1: Add Adapter Stability Test

**Files:**
- Create: `tests/watch_presentation_adapter.rs`

- [ ] **Step 1: Write test**

Create `tests/watch_presentation_adapter.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn watch_preview_output_stays_stable_during_adapter_migration() {
    let dir = tempdir().unwrap();
    let out = dir.path().join("preview");

    Command::cargo_bin("glorp")
        .unwrap()
        .arg("dev-preview")
        .arg("--scenario")
        .arg("watch")
        .arg("--out")
        .arg(&out)
        .env("GLORP_CONFIG_DIR", dir.path().join("config"))
        .assert()
        .success()
        .stdout(predicate::str::contains(out.display().to_string()));

    assert!(out.join("frames/watch-wide-normal.txt").is_file());
    assert!(out.join("frames/watch-wide-normal.scene.json").is_file());
    assert!(out.join("frames/watch-wide-normal.layout.json").is_file());
}
```

- [ ] **Step 2: Run test**

```bash
cargo test --test watch_presentation_adapter --features dev-preview
```

Expected: PASS before code changes.

- [ ] **Step 3: Commit guard**

```bash
git add tests/watch_presentation_adapter.rs
git commit -m "test: guard watch presentation adapter"
```

## Task 2: Derive Presentation Scene in PetPanel

**Files:**
- Modify: `src/tui/panels/pet.rs`

- [ ] **Step 1: Import presentation types**

Add to `src/tui/panels/pet.rs`:

```rust
use crate::presentation::privacy::PresentationSurface;
use crate::presentation::scene::PresentationScene;
```

- [ ] **Step 2: Derive scene once in `PetPanel::render`**

Inside `PetPanel::render`, immediately after `let now = ctx.clock.now_utc();`, add:

```rust
let presentation_scene =
    PresentationScene::from_watch_view_model(vm, now, PresentationSurface::WatchTui);
```

Do not pass `presentation_scene` into child modules in this task. This first commit proves derivation has no side effects.

- [ ] **Step 3: Use a semantic read without changing behavior**

Replace one local species read with the scene value while preserving the concrete `Species` value for existing functions:

```rust
let _presentation_species = presentation_scene.pet.species.as_str();
```

Keep existing `let species = vm.pet_render.generated_species;` unchanged.

- [ ] **Step 4: Run watch tests**

```bash
cargo test --lib tui::panels::pet
cargo test --test watch_presentation_adapter --features dev-preview
cargo test --test dev_preview --features dev-preview dev_preview_watch_wide_normal_frame_snapshot
```

Expected: PASS.

- [ ] **Step 5: Commit adapter seam**

```bash
git add src/tui/panels/pet.rs tests/watch_presentation_adapter.rs
git commit -m "refactor: derive presentation scene in watch pet panel"
```

## Task 3: Replace Room Vocabulary Reads

**Files:**
- Modify: `src/tui/panels/pet.rs`

- [ ] **Step 1: Use presentation room for review-only metadata**

Where `room_profile` is derived, keep it as the painting input. Add a debug assertion that the presentation room projection agrees:

```rust
debug_assert_eq!(
    presentation_scene.room.primary_biome,
    format!("{:?}", room_profile.biome.primary)
);
debug_assert_eq!(
    presentation_scene.room.species_dialect,
    room_profile.species_dialect.key.as_str()
);
```

This keeps behavior identical and gives later adapter work a verified seam.

- [ ] **Step 2: Run focused tests**

```bash
cargo test --lib tui::panels::pet
cargo test --test dev_preview --features dev-preview dev_preview_alive_room_fixtures_include_room_profile_inputs
```

Expected: PASS.

- [ ] **Step 3: Commit semantic checks**

```bash
git add src/tui/panels/pet.rs
git commit -m "refactor: verify watch room projection seam"
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

- Stop if watch layout changes.
- Stop if Preview Lab watch snapshots change.
- Stop if `PetPanel::render` starts accepting adapter-specific output from another surface.

