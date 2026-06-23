# Render Seam — Plan 04: Semantic Scene Container + Flaky-Test Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** (1) Harden the pre-existing flaky `tui_render` tests surfaced by the Plan 03 merge. (2) Introduce a viewport-agnostic **semantic scene container**, `PetSceneModel::build(vm, now, color_capability)`, that bundles the scene's "what" — `EffectState`, the reacted `PetLifeProfile`, and the `RoomLifeProfile` — which `PetPanel::render` currently derives inline across several passes; the watch reads from it. Zero visible change.

**Architecture:** Grounding showed the decoration passes split into a viewport-agnostic *semantic* part (which biome/weather/emitter, the reacted `life_profile`, the `PetPerformance` variant, `EffectState`) and a viewport-bound *pixel* part (RNG-sampled positions against the habitat rect). This plan lifts the **semantic** part into `PetSceneModel`; the **pixel** part stays in `PetPanel` and migrates into `render(style, viewport)`/`SceneDrawList` in Plan 05+. `PetSceneModel` is the prerequisite container that `PetScene::render(viewport)` will consume. Named `PetSceneModel` to avoid clashing with the existing geometry type `tui::component::pet_scene::PetScene` (which will be reconciled in a later plan). Byte-stable; dev-preview goldens are the oracle.

**Tech Stack:** Rust; `crate::presentation::EffectState` (Plan 02), `crate::tui::room::{derive_room_life_profile, RoomLifeProfile}`, `crate::tui::life::{PetLifeProfile, build_prop_reactions}`, `crate::tui::view_model::WatchViewModel`, `crate::tui::style::ColorCapability`.

This is **Plan 04** of the render-seam re-arch — spec `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`, **Track 2b-ii** (semantic half). The pixel/placement half is Plan 05. Do NOT attempt to move glyph positions or paint logic here.

## Global Constraints

- **`src/pet/render.rs` and `src/pet/art.rs` are FROZEN.**
- **`src/presentation/` must NOT import `tui::component::TargetPath`.** (Importing `tui::room`, `tui::life`, `tui::view_model`, `tui::style` is allowed and expected — `presentation/scene.rs` already imports `tui::room`/`tui::view_model`.)
- **`PetSceneModel::build` is viewport-agnostic:** inputs are `(&WatchViewModel, now, ColorCapability)` only — NO `area`/`Rect`/viewport/cursor. It must reproduce the exact values `PetPanel` derives inline today.
- **dev-preview goldens BYTE-STABLE** (no re-bake).
- **No behavior change** (except the flaky tests becoming deterministic — which must not change what they assert when they pass).
- **Per-task gate (full suite):** `cargo test` AND `cargo test --features dev-preview --test dev_preview` AND `cargo clippy --all-targets --all-features -- -D warnings` AND `cargo fmt --check`. **Verify via exit status / grep `FAILED`, never `| tail`** (a `tail` masked an earlier-binary failure during the Plan 03 merge).
- **Commit per task.**

## File Structure

- **Modify** `tests/tui_render.rs` — pin the wall clock in the wander/position-sensitive render tests; widen the `drop_does_not_block_on_in_flight_poll` timing budget.
- **Create** `src/presentation/pet_scene.rs` — `PetSceneModel` + `PetSceneModel::build`.
- **Modify** `src/presentation/mod.rs` — `pub mod pet_scene;` + re-export `PetSceneModel`.
- **Modify** `src/tui/panels/pet.rs` (`PetPanel::render`, `render_pet_inside`) — build `PetSceneModel` once; read `room`/`life`/`effects` from it instead of deriving inline.
- **Maybe modify** `src/tui/panels/pet.rs` (`apply_resonance_reaction`) — widen `pub(crate)` if `PetSceneModel::build` calls it.

---

### Task 1: Harden the flaky `tui_render` tests

This is a systematic-debugging task: reproduce, then fix the root cause (unpinned wall clock + a too-tight timing budget). Two independent flakes.

**Files:**
- Modify: `tests/tui_render.rs`

- [ ] **Step 1: Reproduce / identify the wander flake**

The pet's `facing` flips to `-1` at certain wall-clock minutes, which mirrors art glyphs (e.g. `{`↔`}`); the suite pins the clock in only 2 tests (`tui_render.rs:133`, `:595` — the latter comments *"facing == -1 would mirror `{` to `}` and break the assertions"*). Find every OTHER test in `tui_render.rs` that renders the pet through a real/default clock AND asserts on pet-art glyphs or pet column position. For each candidate, force a facing-flip instant to confirm it flakes: temporarily render with a `WatchClock::fixed(t)` for a `t` where `compute_facing(width, species, t, idle) == -1` (pick `t` by trying a few unix timestamps), and see if the assertion breaks. List the genuinely-affected tests in your report.

- [ ] **Step 2: Pin the clock in the affected tests**

For each affected test, inject a fixed clock (mirror the pattern at `tui_render.rs:595`: build the render context / `WatchApp` with `WatchClock::fixed(<a timestamp where facing == +1>)`), and add a one-line comment noting why (deterministic facing/wander). Do NOT change what the test asserts — only make its clock deterministic. If a candidate test does NOT actually assert position/glyph-sensitive content (e.g. it only checks colors or presence), leave it unpinned and say so.

- [ ] **Step 3: Widen the drop-budget timing flake**

`drop_does_not_block_on_in_flight_poll` (~`tui_render.rs:1040`) asserts `elapsed < Duration::from_secs(2)` after `drop(app)` while a poller sleeps 3600s. Its intent is "Drop does not join the hour-long worker." Under full-suite load, a 2s budget can flake. Widen it to a value that still unambiguously catches the regression (the bad path waits ~3600s) but is immune to load jitter — e.g. `Duration::from_secs(30)` — and update the comment to state the budget's intent (catch the hour-long join, not measure precise latency).

- [ ] **Step 4: Verify determinism**

Run the affected tests in a loop to confirm they no longer flake:
```bash
for i in $(seq 1 20); do cargo test --test tui_render 2>&1 | grep -q "test result: ok" || echo "FLAKE on run $i"; done; echo done
```
Expected: no FLAKE lines. Then the full gate.

Run: `cargo test && cargo test --features dev-preview --test dev_preview && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/tui_render.rs
git commit -m "test: pin clock in wander-sensitive tui_render tests; widen drop-budget"
```

---

### Task 2: `PetSceneModel` semantic container

**Files:**
- Create: `src/presentation/pet_scene.rs`
- Modify: `src/presentation/mod.rs`
- Modify: `src/tui/panels/pet.rs`
- Test: in `src/presentation/pet_scene.rs`

**Interfaces:**
- Produces: `pub struct PetSceneModel { pub effects: EffectState, pub room: RoomLifeProfile, pub life: PetLifeProfile }` and `pub fn PetSceneModel::build(vm: &WatchViewModel, now: time::OffsetDateTime, color_capability: ColorCapability) -> PetSceneModel`.
- Consumes: `crate::presentation::EffectState`, `crate::tui::room::{derive_room_life_profile, RoomLifeProfile}`, `crate::tui::life::{PetLifeProfile, build_prop_reactions}`, `crate::tui::panels::pet::apply_resonance_reaction` (widen to `pub(crate)`), `WatchViewModel`, `ColorCapability`.

- [ ] **Step 1: Trace the exact inline derivations**

In `src/tui/panels/pet.rs::render`, identify verbatim how today's values are derived (the build must reproduce them EXACTLY):
- `room`: `derive_room_life_profile(vm, now)` (`pet.rs:189`).
- `life` (reacted): the incoming `vm.life_profile` then `build_prop_reactions(...)` (`pet.rs` step E, ~257-265) then `apply_resonance_reaction(...)` (`pet.rs:106`/call site). Read the exact arguments each takes.
- `effects`: `EffectState::from_vm(vm, now, color_capability)` (currently built inside `render_pet_inside`).
Quote the real argument lists in your report; the build mirrors them.

- [ ] **Step 2: Write the failing test**

In `src/presentation/pet_scene.rs`, assert the build reproduces each inline derivation independently:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;

    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn build_reproduces_room_and_effects() {
        let vm = WatchViewModel::fixture();
        let now = fixed_now();
        let m = PetSceneModel::build(&vm, now, ColorCapability::Truecolor);
        assert_eq!(m.room, crate::tui::room::derive_room_life_profile(&vm, now));
        assert_eq!(m.effects, EffectState::from_vm(&vm, now, ColorCapability::Truecolor));
        // life: reactions present — assert the reacted profile equals the same
        // build_prop_reactions + apply_resonance_reaction pipeline the panel runs
        // (replicate that pipeline here against the fixture; see impl).
    }
}
```

(`RoomLifeProfile` and `PetLifeProfile` must derive `PartialEq` for `assert_eq!` — add `#[derive(PartialEq)]` if missing; they are plain data, no logic change. If `apply_resonance_reaction` needs the resonant prop, derive it in the test the same way the impl does.)

- [ ] **Step 3: Implement `PetSceneModel::build`**

Move the three derivations into `build`, calling the same functions with the same arguments. Widen `apply_resonance_reaction` to `pub(crate)`. Declare `pub mod pet_scene;` + `pub use pet_scene::PetSceneModel;` in `src/presentation/mod.rs`. RED→GREEN.

- [ ] **Step 4: Route `PetPanel::render` to read from the model**

Build `let scene_model = crate::presentation::PetSceneModel::build(vm, now, color_capability);` once (right after the layout, before the first paint — ~`pet.rs:190`). Replace:
- the `derive_room_life_profile(vm, now)` call → `scene_model.room` (or `&scene_model.room`).
- the inline `build_prop_reactions` + `apply_resonance_reaction` that produce the reacted `life_profile` → `scene_model.life`.
- inside `render_pet_inside`, the `EffectState::from_vm(...)` call → `scene_model.effects` (thread the model in, or read its fields).
Keep ALL pixel/paint passes exactly as they are. `cargo build` and remove any now-unused locals/imports.

- [ ] **Step 5: Goldens byte-stable + full gate**

Run: `cargo test --features dev-preview --test dev_preview` → watch `cells.json` byte-identical (values moved location, not value). If a frame differs, a derivation diverged — reconcile, do NOT re-bake.
Then: `cargo test && cargo test --features dev-preview --test dev_preview && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check` (check exit status, not `tail`).

- [ ] **Step 6: Commit**

```bash
git add src/presentation/pet_scene.rs src/presentation/mod.rs src/tui/panels/pet.rs
git commit -m "feat: PetSceneModel — semantic scene container (effects + reacted life + room)"
```

---

## Self-Review

**Spec coverage (Track 2b-ii, semantic half):**
- Semantic scene container `PetSceneModel::build` (viewport-agnostic): Task 2. ✓
- Watch reads room/life/effects from it (de-dups the inline derivations): Task 2 Step 4. ✓
- Flaky-test fix (the Plan 03 follow-up Drew requested): Task 1. ✓
- Byte-stable; pixel/placement passes untouched (deferred to Plan 05): by construction. ✓
- Constraints: `render_pet`/`art.rs` frozen; `presentation/` no `TargetPath`; goldens oracle; gate via exit status not `tail`. ✓

**Placeholder scan:** No TBD. The repo-authoritative details (exact `build_prop_reactions`/`apply_resonance_reaction` argument lists; which `tui_render` tests actually flake) are flagged to trace/reproduce from source, with the fix pattern given.

**Type consistency:** `PetSceneModel { effects, room, life }`, `build(vm, now, color_capability)` used identically in the test and the `PetPanel` read site. `PetPerformance` is reached via `scene_model.room.pet_performance` (a field of `RoomLifeProfile`), not a separate field.

**Out of scope (Plan 05+):** all glyph/pixel positioning (`render(style, viewport)`/`SceneDrawList`), the double-prop-placement de-dup, reconciling the name clash with `tui::component::pet_scene::PetScene`, composing pet cells/palette/wander into `PetSceneModel`. This plan lifts only the semantic "what."
