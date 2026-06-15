# Glorp Presentation Prop Wrappers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add presentation-domain wrappers for habitat prop identity, placement summaries, and neutral effect targets without moving prop placement behavior.

**Architecture:** `src/tui/component/habitat_props.rs` remains the placement engine. `src/presentation/props.rs` converts existing `HabitatPropPlacement` values into backend-neutral summaries with owned target IDs, giving later adapters a shared prop vocabulary.

**Tech Stack:** Rust 2021, existing habitat prop placement APIs, ratatui `Rect`, Preview Lab prop tests.

---

## Dependency Gate

```bash
test -f src/presentation/room.rs
cargo test --test presentation_room
cargo test --lib tui::component::habitat_props
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/presentation/props.rs` | Create | Prop IDs, placement summaries, layer names, and neutral target IDs. |
| `src/presentation/mod.rs` | Modify | Export `props`. |
| `tests/presentation_props.rs` | Create | Prop wrapper conversion tests. |

## Task 1: Add Prop Wrapper Tests

**Files:**
- Create: `tests/presentation_props.rs`

- [ ] **Step 1: Write tests**

Create `tests/presentation_props.rs`:

```rust
use glorp::presentation::props::PresentationPropPlacement;
use glorp::tui::component::habitat_props::habitat_prop_placements_for;
use glorp::tui::render_context::{RenderContext, WatchClock};
use glorp::tui::style::ColorCapability;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

#[test]
fn presentation_prop_wrappers_use_neutral_targets() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let scene = glorp::tui::component::PetScene::compute_layout(
        ratatui::layout::Rect::new(0, 0, 80, 24),
        &vm,
        &RenderContext::with_clock(
            ColorCapability::Truecolor,
            WatchClock::fixed(datetime!(2026-06-15 12:00 UTC)),
        ),
    );
    let placements = habitat_prop_placements_for(
        &vm.habitat,
        &scene,
        &[],
        vm.pet_render.generated_species,
        &vm.pet_render.seed,
        &RenderContext::with_clock(
            ColorCapability::Truecolor,
            WatchClock::fixed(datetime!(2026-06-15 12:00 UTC)),
        ),
    );

    let wrapped = placements
        .iter()
        .map(PresentationPropPlacement::from_habitat_placement)
        .collect::<Vec<_>>();

    assert!(!wrapped.is_empty());
    for placement in wrapped {
        assert!(!placement.prop_id.is_empty());
        assert!(!placement.cells.is_empty());
        if let Some(target) = placement.effect_target {
            assert!(!target.as_str().starts_with("watch."));
            assert!(target.as_str().starts_with("prop."));
        }
    }
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test --test presentation_props
```

Expected: FAIL because `presentation::props` does not exist.

- [ ] **Step 3: Commit failing test**

```bash
git add tests/presentation_props.rs
git commit -m "test: require presentation prop wrappers"
```

## Task 2: Add Prop Wrapper Module

**Files:**
- Create: `src/presentation/props.rs`
- Modify: `src/presentation/mod.rs`

- [ ] **Step 1: Export module**

Add to `src/presentation/mod.rs`:

```rust
pub mod props;
```

- [ ] **Step 2: Implement wrappers**

Create `src/presentation/props.rs`:

```rust
use crate::presentation::target::SurfaceTargetId;
use crate::tui::component::habitat_props::{HabitatPropCell, HabitatPropPlacement};

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationPropPlacement {
    pub prop_id: String,
    pub layer: PresentationPropLayer,
    pub bounds: PresentationRect,
    pub cells: Vec<PresentationPropCell>,
    pub effect_target: Option<SurfaceTargetId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationPropLayer {
    Background,
    Behind,
    Foreground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationPropCell {
    pub x: u16,
    pub y: u16,
    pub glyph: char,
}

impl PresentationPropPlacement {
    pub fn from_habitat_placement(placement: &HabitatPropPlacement) -> Self {
        Self {
            prop_id: placement.prop_id.as_str().to_string(),
            layer: match placement.pet_layer {
                crate::game::habitat::HabitatPetLayer::Background => PresentationPropLayer::Background,
                crate::game::habitat::HabitatPetLayer::Behind => PresentationPropLayer::Behind,
                crate::game::habitat::HabitatPetLayer::Foreground => PresentationPropLayer::Foreground,
            },
            bounds: PresentationRect {
                x: placement.bounds.x,
                y: placement.bounds.y,
                width: placement.bounds.width,
                height: placement.bounds.height,
            },
            cells: placement.cells.iter().map(presentation_cell).collect(),
            effect_target: placement.target_id.as_ref().map(|target| {
                let raw = target.as_str();
                let without_watch = raw.strip_prefix("watch.").unwrap_or(raw);
                let neutral = without_watch.strip_suffix(".effect").unwrap_or(without_watch);
                SurfaceTargetId::new(format!("{neutral}.effect"))
            }),
        }
    }
}

fn presentation_cell(cell: &HabitatPropCell) -> PresentationPropCell {
    PresentationPropCell {
        x: cell.col,
        y: cell.row,
        glyph: cell.glyph,
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test --test presentation_props
```

Expected: PASS.

- [ ] **Step 4: Commit wrappers**

```bash
git add src/presentation/mod.rs src/presentation/props.rs tests/presentation_props.rs
git commit -m "feat: add presentation prop wrappers"
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

- Stop if prop placement algorithms need to move out of `src/tui/component/habitat_props.rs`.
- Stop if neutral target IDs need to start with `watch.`.
- Stop if prop cell output changes in Preview Lab.
