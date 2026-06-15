# Glorp Presentation Room Projection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a presentation-owned room projection that wraps existing `RoomLifeProfile` and room glyph vocabulary without moving room placement behavior.

**Architecture:** `src/tui/room.rs` continues to own room derivation and glyph placement. `src/presentation/room.rs` exposes a serializable, backend-neutral projection for scene derivation and future adapters.

**Tech Stack:** Rust 2021, existing `RoomLifeProfile`, `RoomSpeciesDialect`, `biome_symbols`, Preview Lab room artifacts.

---

## Dependency Gate

```bash
test -f src/presentation/scene.rs
test -f src/presentation/pet.rs
cargo test --test presentation_scene
cargo test --test presentation_pet
```

Expected: all commands pass.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/presentation/room.rs` | Create | Room projection structs and conversion from `RoomLifeProfile`. |
| `src/presentation/mod.rs` | Modify | Export `room`. |
| `src/presentation/scene.rs` | Modify | Use `PresentationRoom` inside `PresentationScene`. |
| `tests/presentation_room.rs` | Create | Room projection and glyph vocabulary tests. |

## Task 1: Add Room Projection Tests

**Files:**
- Create: `tests/presentation_room.rs`

- [ ] **Step 1: Write tests**

Create `tests/presentation_room.rs`:

```rust
use glorp::presentation::room::PresentationRoom;
use glorp::tui::room::derive_room_life_profile;
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

#[test]
fn presentation_room_preserves_profile_identity_without_placement() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let profile = derive_room_life_profile(&vm, datetime!(2026-06-15 12:00 UTC));

    let room = PresentationRoom::from_profile(&profile);

    assert_eq!(room.primary_biome, format!("{:?}", profile.biome.primary));
    assert_eq!(room.species_dialect, profile.species_dialect.key.as_str());
    assert_eq!(
        room.prop_landmarks,
        profile
            .identity_prop_ids
            .iter()
            .map(|id| id.as_str().to_string())
            .collect::<Vec<_>>()
    );
    assert!(!room.glyph_vocabulary.is_empty());
    assert!(room.placements.is_empty(), "placement stays outside this plan");
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test --test presentation_room
```

Expected: FAIL because `presentation::room` does not exist.

- [ ] **Step 3: Commit failing test**

```bash
git add tests/presentation_room.rs
git commit -m "test: require presentation room projection"
```

## Task 2: Add Room Projection Module

**Files:**
- Create: `src/presentation/room.rs`
- Modify: `src/presentation/mod.rs`

- [ ] **Step 1: Export module**

Add to `src/presentation/mod.rs`:

```rust
pub mod room;
```

- [ ] **Step 2: Implement projection**

Create `src/presentation/room.rs`:

```rust
use crate::tui::room::{biome_symbols, RoomLifeProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationRoom {
    pub primary_biome: String,
    pub secondary_biome: Option<String>,
    pub species_dialect: String,
    pub dialect_status: String,
    pub room_weather: String,
    pub prop_landmarks: Vec<String>,
    pub glyph_vocabulary: Vec<String>,
    pub placements: Vec<PresentationRoomPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationRoomPlacement {
    pub target_id: String,
    pub glyph: String,
}

impl PresentationRoom {
    pub fn from_profile(profile: &RoomLifeProfile) -> Self {
        Self {
            primary_biome: format!("{:?}", profile.biome.primary),
            secondary_biome: profile.biome.secondary.map(|tag| format!("{tag:?}")),
            species_dialect: profile.species_dialect.key.as_str().to_string(),
            dialect_status: profile.species_dialect.status.as_str().to_string(),
            room_weather: format!("{:?}", profile.room_weather),
            prop_landmarks: profile
                .identity_prop_ids
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            glyph_vocabulary: biome_symbols(profile.biome.primary, profile.species_dialect)
                .iter()
                .map(|ch| ch.to_string())
                .collect(),
            placements: Vec::new(),
        }
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test --test presentation_room
```

Expected: PASS.

- [ ] **Step 4: Commit projection**

```bash
git add src/presentation/mod.rs src/presentation/room.rs tests/presentation_room.rs
git commit -m "feat: add presentation room projection"
```

## Task 3: Use Projection in Presentation Scene

**Files:**
- Modify: `src/presentation/scene.rs`

- [ ] **Step 1: Replace room snapshot internals**

In `src/presentation/scene.rs`, import:

```rust
use crate::presentation::room::PresentationRoom;
```

Change:

```rust
pub room: PresentationRoomSnapshot,
```

to:

```rust
pub room: PresentationRoom,
```

Delete `PresentationRoomSnapshot` from `scene.rs`, and replace its construction with:

```rust
room: PresentationRoom::from_profile(&room_profile),
```

- [ ] **Step 2: Run scene and room tests**

```bash
cargo test --test presentation_scene
cargo test --test presentation_room
```

Expected: PASS.

- [ ] **Step 3: Commit scene integration**

```bash
git add src/presentation/scene.rs
git commit -m "refactor: use presentation room projection in scene"
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

- Stop if this plan starts moving glyph placement out of `src/tui/room.rs`.
- Stop if `PresentationRoom` duplicates placement algorithms.
- Stop if Preview Lab room text artifacts change.
