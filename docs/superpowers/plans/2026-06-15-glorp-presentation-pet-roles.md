# Glorp Presentation Pet Roles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move shared pet role/span/color interpretation into `src/presentation/pet.rs` while preserving existing watch, round, Preview Lab, and menubar output.

**Architecture:** Add presentation pet text primitives that wrap existing `StyledSegment` character-index spans. Keep `src/tui/panels/pet.rs` wrappers in place so this is an additive extraction, then migrate round preview and menubar to the shared role lookup helper.

**Tech Stack:** Rust 2021, existing `StyledSegment`, `PaletteRoleName`, `ResolvedPalette`, ratatui styles, Preview Lab pet matrix tests.

---

## Dependency Gate

Run:

```bash
test -f src/presentation/scene.rs
cargo test --test presentation_scene
cargo test --test pet_panel_structure
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/presentation/pet.rs` | Create | Shared pet text block, role span, role lookup, and color conversion helpers. |
| `src/presentation/mod.rs` | Modify | Export `pet`. |
| `src/tui/panels/pet/art_lines.rs` | Modify | Delegate role span line construction to presentation helper while preserving existing function names. |
| `src/round/preview.rs` | Modify | Use shared `role_for_cell` helper for round pet cell colors. |
| `src/menubar/render.rs` | Modify | Use shared pet role color lookup for attributed-string spans. |
| `tests/presentation_pet.rs` | Create | Shared role/span behavior tests. |

## Task 1: Add Shared Pet Role Tests

**Files:**
- Create: `tests/presentation_pet.rs`

- [ ] **Step 1: Write tests**

Create `tests/presentation_pet.rs`:

```rust
use glorp::pet::render::{PaletteRoleName, StyledSegment};
use glorp::presentation::pet::{role_for_cell, role_names, PetTextBlock};

#[test]
fn presentation_pet_role_lookup_uses_character_indices() {
    let block = PetTextBlock::new(
        vec!["ab界d".to_string()],
        vec![
            StyledSegment {
                line: 0,
                start: 0,
                end: 2,
                role: PaletteRoleName::Eye,
            },
            StyledSegment {
                line: 0,
                start: 2,
                end: 3,
                role: PaletteRoleName::Accent,
            },
        ],
    );

    assert_eq!(role_for_cell(&block, 0, 0), PaletteRoleName::Eye);
    assert_eq!(role_for_cell(&block, 0, 1), PaletteRoleName::Eye);
    assert_eq!(role_for_cell(&block, 0, 2), PaletteRoleName::Accent);
    assert_eq!(role_for_cell(&block, 0, 3), PaletteRoleName::Body);
}

#[test]
fn presentation_pet_role_names_are_stable_and_deduped() {
    let roles = role_names(&[
        StyledSegment {
            line: 0,
            start: 0,
            end: 1,
            role: PaletteRoleName::Eye,
        },
        StyledSegment {
            line: 1,
            start: 0,
            end: 1,
            role: PaletteRoleName::Eye,
        },
        StyledSegment {
            line: 1,
            start: 1,
            end: 2,
            role: PaletteRoleName::Pattern,
        },
    ]);

    assert_eq!(roles, vec!["eye".to_string(), "pattern".to_string()]);
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test --test presentation_pet
```

Expected: FAIL because `presentation::pet` does not exist.

- [ ] **Step 3: Commit failing tests**

```bash
git add tests/presentation_pet.rs
git commit -m "test: require presentation pet role helpers"
```

## Task 2: Add `presentation::pet`

**Files:**
- Create: `src/presentation/pet.rs`
- Modify: `src/presentation/mod.rs`

- [ ] **Step 1: Export module**

Add to `src/presentation/mod.rs`:

```rust
pub mod pet;
```

- [ ] **Step 2: Implement text block and role helpers**

Create `src/presentation/pet.rs`:

```rust
use crate::pet::render::{PaletteRoleName, StyledSegment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetTextBlock {
    pub lines: Vec<String>,
    pub spans: Vec<StyledSegment>,
}

impl PetTextBlock {
    pub fn new(lines: Vec<String>, spans: Vec<StyledSegment>) -> Self {
        Self { lines, spans }
    }
}

pub fn role_for_cell(block: &PetTextBlock, row: usize, char_index: usize) -> PaletteRoleName {
    block
        .spans
        .iter()
        .find(|span| span.line == row && char_index >= span.start && char_index < span.end)
        .map(|span| span.role)
        .unwrap_or(PaletteRoleName::Body)
}

pub fn role_names(spans: &[StyledSegment]) -> Vec<String> {
    let mut roles = spans
        .iter()
        .map(|span| role_name(span.role).to_string())
        .collect::<Vec<_>>();
    roles.sort();
    roles.dedup();
    roles
}

pub fn role_name(role: PaletteRoleName) -> &'static str {
    match role {
        PaletteRoleName::Body => "body",
        PaletteRoleName::Eye => "eye",
        PaletteRoleName::Mouth => "mouth",
        PaletteRoleName::Accent => "accent",
        PaletteRoleName::Pattern => "pattern",
        PaletteRoleName::Particle => "particle",
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test --test presentation_pet
```

Expected: PASS.

- [ ] **Step 4: Commit helper module**

```bash
git add src/presentation/mod.rs src/presentation/pet.rs tests/presentation_pet.rs
git commit -m "feat: add presentation pet role helpers"
```

## Task 3: Migrate Round Preview Role Lookup

**Files:**
- Modify: `src/round/preview.rs`

- [ ] **Step 1: Replace local role lookup**

In `src/round/preview.rs`, replace the local `role_for_pet_cell` helper with the shared helper:

```rust
let block = crate::presentation::pet::PetTextBlock::new(
    scene.pet.art_lines.clone(),
    scene.pet.art_spans.clone(),
);
```

Inside the pet-art loop, replace:

```rust
let role = role_for_pet_cell(&scene.pet.art_spans, row, char_index);
```

with:

```rust
let role = crate::presentation::pet::role_for_cell(&block, row, char_index);
```

Delete the local `fn role_for_pet_cell(...)`.

- [ ] **Step 2: Run round preview tests**

```bash
cargo test --lib round::preview
cargo test --test dev_preview --features dev-preview dev_preview_round_writes_manifest_cells_and_round_metadata
```

Expected: PASS.

- [ ] **Step 3: Commit round preview migration**

```bash
git add src/round/preview.rs
git commit -m "refactor: use presentation pet roles in round preview"
```

## Task 4: Keep Watch and Menubar Behavior Stable

**Files:**
- Modify: `src/tui/panels/pet/art_lines.rs`
- Modify: `src/menubar/render.rs`

- [ ] **Step 1: Preserve existing watch wrapper**

In `src/tui/panels/pet/art_lines.rs`, keep `pet_role_spans_for_line` public and add a short internal comment above it:

```rust
// Compatibility wrapper for watch and Preview Lab callers; the shared
// presentation role lookup owns the domain semantics.
```

No signature changes are allowed for `pet_role_spans_for_line`.

- [ ] **Step 2: Migrate menubar role names for diagnostics-free rendering**

In `src/menubar/render.rs`, import:

```rust
use crate::presentation::pet::role_name;
```

Where the renderer matches `PaletteRoleName` for color, keep the existing color mapping but replace any debug/string role naming with `role_name(role)`. If there is no role-name formatting in the current file, make no code change in `src/menubar/render.rs` and leave the import out.

- [ ] **Step 3: Run checks**

```bash
cargo test --lib tui::panels::pet
cargo test --lib menubar
cargo test --test dev_preview --features dev-preview dev_preview_pets_writes_species_stage_matrix
```

Expected: PASS. On non-macOS, `cargo test --lib menubar` is cfg-gated and may report no matching tests; that is acceptable when the command exits 0.

- [ ] **Step 4: Commit compatibility migration**

```bash
git add src/tui/panels/pet/art_lines.rs src/menubar/render.rs
git commit -m "refactor: share pet role semantics"
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

- Stop if `StyledSegment` span indexing must change from character indexes.
- Stop if pet colors differ in Preview Lab output.
- Stop if menubar requires AppKit changes outside `src/menubar/render.rs`.

