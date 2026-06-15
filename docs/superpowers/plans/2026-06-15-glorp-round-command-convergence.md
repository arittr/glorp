# Glorp Round Command Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make round Preview Lab rendering and native companion rendering consume the same round draw-command vocabulary.

**Architecture:** Plan 1 moves pure `RoundDrawCommand` construction into `src/round/draw.rs`. This plan makes `src/round/preview.rs` render from those commands instead of independently deciding room, pet, and halo painting. Native companion keeps its AppKit drawing layer, but both surfaces now share `build_draw_commands`.

**Tech Stack:** Rust 2021, `RoundSceneModel`, `RoundSceneLayout`, `RoundDrawCommand`, Preview Lab cells, macOS companion render facade.

---

## Dependency Gate

```bash
test -f src/round/draw.rs
test -f src/presentation/props.rs
cargo test --lib round::draw
cargo test --test dev_preview --features dev-preview dev_preview_round_writes_layout_and_command_artifacts
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/round/preview.rs` | Modify | Render round Preview Lab cells from `RoundDrawCommand`s. |
| `src/round/draw.rs` | Modify | Add tiny command metadata helpers used by Preview Lab. |
| `tests/round_command_convergence.rs` | Create | Assert preview artifact semantics match draw commands. |

## Task 1: Add Convergence Tests

**Files:**
- Create: `tests/round_command_convergence.rs`

- [ ] **Step 1: Write tests**

Create `tests/round_command_convergence.rs`:

```rust
use glorp::round::draw::{build_draw_commands, RoundDrawKind};
use glorp::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
use glorp::round::model::derive_round_scene_model;
use glorp::round::preview::render_round_preview_frame_from_vm;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

#[test]
fn round_preview_and_draw_commands_share_pet_text() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.pet_art = vec!["ab".to_string(), "cd".to_string()];
    let now = datetime!(2026-06-15 12:00 UTC);
    let scene = derive_round_scene_model(&vm, now);
    let layout = layout_round_scene(
        &scene,
        RoundAperture::new(52, 52),
        RoundRenderCapabilities::preview_truecolor(),
    );
    let commands = build_draw_commands(&scene, &layout);
    let pet = commands
        .iter()
        .find(|command| command.kind == RoundDrawKind::PetGlyph)
        .expect("pet command");

    let frame = render_round_preview_frame_from_vm(
        "round-test",
        "Round Test",
        &vm,
        now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    );
    let visible = frame
        .cells
        .iter()
        .filter(|cell| !cell.outside_aperture && !cell.symbol.trim().is_empty())
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();

    assert_eq!(pet.text.as_deref(), Some("ab\ncd"));
    assert!(visible.contains("ab") || visible.contains("cd"));
}

#[test]
fn round_preview_exposes_command_backed_room_and_halo_glyphs() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.source_health[0].status = glorp::tui::view_model::SourceStatus::Diagnostic;
    let now = datetime!(2026-06-15 12:00 UTC);
    let scene = derive_round_scene_model(&vm, now);
    let layout = layout_round_scene(
        &scene,
        RoundAperture::new(52, 52),
        RoundRenderCapabilities::preview_truecolor(),
    );
    let commands = build_draw_commands(&scene, &layout);

    assert!(commands.iter().any(|command| command.kind == RoundDrawKind::RoomGlyph));
    assert!(commands.iter().any(|command| command.kind == RoundDrawKind::Trouble));

    let frame = render_round_preview_frame_from_vm(
        "round-trouble",
        "Round Trouble",
        &vm,
        now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    );
    assert!(frame.cells.iter().any(|cell| cell.symbol == "!"));
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --test round_command_convergence
```

Expected: PASS before migration or FAIL only if the current direct preview path has already drifted from draw commands. Continue with migration either way.

- [ ] **Step 3: Commit convergence tests**

```bash
git add tests/round_command_convergence.rs
git commit -m "test: cover round command convergence"
```

## Task 2: Render Preview Cells from Draw Commands

**Files:**
- Modify: `src/round/preview.rs`

- [ ] **Step 1: Build commands in preview renderer**

In `render_round_preview_frame_from_vm`, replace direct calls to `paint_room`, `paint_pet_art`, and `paint_halo` with:

```rust
let commands = crate::round::draw::build_draw_commands(&scene, &layout);
paint_commands(&mut cells, width, &scene, &layout, &commands, capabilities.truecolor);
```

- [ ] **Step 2: Add command painter**

Add to `src/round/preview.rs`:

```rust
fn paint_commands(
    cells: &mut [PreviewCell],
    width: u16,
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
    commands: &[crate::round::draw::RoundDrawCommand],
    truecolor: bool,
) {
    for command in commands {
        match command.kind {
            crate::round::draw::RoundDrawKind::Background => {}
            crate::round::draw::RoundDrawKind::RoomGlyph => {
                if let Some(label) = command.label {
                    let (_, fg) = room_symbol_at(scene, command.x as u16, command.y as u16, truecolor);
                    set_cell(
                        cells,
                        width,
                        command.x.round() as i32,
                        command.y.round() as i32,
                        label.to_string(),
                        Some(fg),
                    );
                }
            }
            crate::round::draw::RoundDrawKind::PropGlyph => {
                if let Some(label) = command.label {
                    set_cell(
                        cells,
                        width,
                        command.x.round() as i32,
                        command.y.round() as i32,
                        label.to_string(),
                        Some(if truecolor { "#b3d184" } else { "green" }.to_string()),
                    );
                }
            }
            crate::round::draw::RoundDrawKind::PetGlyph => {
                paint_pet_art(cells, width, scene, layout, truecolor);
            }
            crate::round::draw::RoundDrawKind::Halo => {
                set_cell(
                    cells,
                    width,
                    command.x.round() as i32,
                    command.y.round() as i32,
                    "o".to_string(),
                    Some(if truecolor { "#f0a646" } else { "yellow" }.to_string()),
                );
            }
            crate::round::draw::RoundDrawKind::Trouble => {
                set_cell(
                    cells,
                    width,
                    command.x.round() as i32,
                    command.y.round() as i32,
                    "!".to_string(),
                    Some(if truecolor { "#f0a646" } else { "yellow" }.to_string()),
                );
            }
        }
    }
}
```

Keep `paint_pet_art` as the pet text painter. Delete `paint_room` and `paint_halo` after `paint_commands` covers their call sites.

- [ ] **Step 3: Run convergence and round preview tests**

```bash
cargo test --test round_command_convergence
cargo test --lib round::preview
cargo test --test dev_preview --features dev-preview dev_preview_round_glitch_and_crystal_differ_by_symbols
```

Expected: PASS.

- [ ] **Step 4: Commit preview migration**

```bash
git add src/round/preview.rs tests/round_command_convergence.rs
git commit -m "refactor: render round preview from draw commands"
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

- Stop if native companion drawing needs AppKit changes.
- Stop if `src/round/draw.rs` needs Preview Lab-specific cell concepts.
- Stop if round Preview Lab privacy tests regress.

