# Glorp Menubar Adapter Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mark the menubar popover as an explicitly privileged presentation surface and reuse shared pet/scene semantics without changing AppKit lifecycle.

**Architecture:** Menubar remains a macOS-only attributed-string adapter. It derives `PresentationScene` with `PresentationSurface::MenubarPopover` inside the render layer, preserving exact counts/source labels that are allowed for the privileged popover and keeping the native app loop untouched.

**Tech Stack:** Rust 2021, `objc2` AppKit/Foundation behind existing macOS cfg, `PresentationScene`, existing `menubar::render` tests.

---

## Dependency Gate

```bash
test -f src/presentation/pet.rs
test -f src/presentation/scene.rs
cargo test --test presentation_scene
cargo test --test presentation_pet
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/menubar/render.rs` | Modify | Derive privileged presentation scene and use shared pet role names where applicable. |
| `src/menubar/app.rs` | Modify | No lifecycle changes; only imports change when render signature changes. |
| `tests/presentation_scene.rs` | Modify | Add explicit menubar privileged projection test. |

## Task 1: Strengthen Menubar Privacy Contract

**Files:**
- Modify: `tests/presentation_scene.rs`

- [ ] **Step 1: Add test**

Add:

```rust
#[test]
fn menubar_projection_is_privileged_but_not_path_or_transcript_visible() {
    let projection = PrivacyProjection::for_surface(PresentationSurface::MenubarPopover);

    assert!(projection.source_names_visible);
    assert!(projection.exact_counts_visible);
    assert!(!projection.file_paths_visible);
    assert!(!projection.project_names_visible);
    assert!(!projection.feed_rows_visible);
}
```

- [ ] **Step 2: Run test**

```bash
cargo test --test presentation_scene menubar_projection_is_privileged_but_not_path_or_transcript_visible
```

Expected: PASS because Plan 3a established the policy.

- [ ] **Step 3: Commit contract test**

```bash
git add tests/presentation_scene.rs
git commit -m "test: document menubar privacy projection"
```

## Task 2: Derive Presentation Scene in Menubar Render

**Files:**
- Modify: `src/menubar/render.rs`

- [ ] **Step 1: Import scene types**

Add to `src/menubar/render.rs`:

```rust
use crate::presentation::privacy::PresentationSurface;
use crate::presentation::scene::PresentationScene;
```

- [ ] **Step 2: Derive scene in `render_pet_block`**

At the start of `render_pet_block(vm: &WatchViewModel)`, add:

```rust
let scene = PresentationScene::from_watch_view_model(
    vm,
    time::OffsetDateTime::now_utc(),
    PresentationSurface::MenubarPopover,
);
debug_assert!(scene.privacy.source_names_visible);
debug_assert!(scene.privacy.exact_counts_visible);
```

Do not change visible attributed output in this task.

- [ ] **Step 3: Run menubar checks**

```bash
cargo test --lib menubar
cargo test --test presentation_scene
```

Expected on macOS: PASS. On non-macOS, the menubar module is cfg-gated; the command should exit 0 or report no matching tests.

- [ ] **Step 4: Commit menubar seam**

```bash
git add src/menubar/render.rs tests/presentation_scene.rs
git commit -m "refactor: mark menubar as privileged presentation surface"
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

- Stop if AppKit lifecycle or polling code must change.
- Stop if menubar starts using round companion privacy rules.
- Stop if privileged menubar output gains paths, projects, feed rows, prompts, responses, or transcript text.

