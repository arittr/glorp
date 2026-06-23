# Render Seam — Plan 03: Wander/Facing Shared Resolver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the inline 3-arm wander/facing selection out of `PetPanel::render` into one shared, reusable `resolve_wander_offset(vm, now, habitat_width) -> (wander_x, facing)`, so the watch calls it and the companion can call the identical function with its round width in a later plan (zero new wander logic on the companion plan). Zero visible change.

**Architecture:** New `src/tui/wander.rs` holds `pub(crate) fn resolve_wander_offset(vm: &WatchViewModel, now, habitat_width: u16) -> (i16, i8)`, which moves verbatim from `PetPanel::render`: the `resonant_prop` derivation, `effective_weekend_softening`, `idle_minutes`/`species`/`day` reads, the sleep/wake/normal arm `match`, and `resonance_wander_bias` (relocated here with its test). `PetPanel::render` calls it and keeps the `Cow`-vm write at the call site. `effective_weekend_softening` widens from `pub(super)` to `pub(crate)`. Byte-stable; dev-preview goldens are the oracle.

**Tech Stack:** Rust; `crate::pet::animator` (`compute_wander_position_x`/`_sleep_`/`_wake_`, `lazy_wander_instant`, `compute_facing`), `crate::tui::day::resonant_prop_for_day`, `crate::game::habitat::catalog_prop`, `crate::storage::state::EarnedHabitatProp`, `crate::tui::view_model::WatchViewModel`.

This is **Plan 03** of the render-seam re-arch — spec `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`, **Track 2b-i**. It is a reuse/decoupling extraction, NOT a "semantic split" (grounding showed the wander target is irreducibly width-dependent; see the spec's Track 2b-i premise correction). Do not attempt to put wander state on `EffectState`.

## Global Constraints

- **`src/pet/render.rs` and `src/pet/art.rs` are FROZEN.** `src/pet/animator.rs` is only CALLED, not changed.
- **Behavior-preserving:** `resolve_wander_offset` must return the EXACT `(wander_x, facing)` the current inline code produces for the same `(vm, now, habitat_width)`. The two roundings differ by path (normal arm truncates `(position as i32)`; sleep/wake arms `.round()` via `blend_positions`) — preserve by moving the logic verbatim, not rewriting it.
- **dev-preview goldens BYTE-STABLE** (no re-bake; the pet's on-screen column must not move).
- **Per-task gate (full suite):** `cargo test` AND `cargo test --features dev-preview --test dev_preview` AND `cargo clippy --all-targets --all-features -- -D warnings` AND `cargo fmt --check`.
- **Commit per task.**

## File Structure

- **Create** `src/tui/wander.rs` — `pub(crate) fn resolve_wander_offset` + the relocated `resonance_wander_bias` (+ `RESONANCE_WANDER_BIAS_CELLS`) + the relocated `resonance_wander_bias` test.
- **Modify** `src/tui/mod.rs` — `pub(crate) mod wander;`.
- **Modify** `src/tui/panels/pet/ambient.rs:504` — `effective_weekend_softening` visibility `pub(super)` → `pub(crate)`.
- **Modify** `src/tui/panels/pet.rs` — replace the inline wander/facing block (~lines 148-198) with a `resolve_wander_offset` call; delete `resonance_wander_bias` (moved) and its test (moved).

---

### Task 1: `src/tui/wander.rs` with `resolve_wander_offset`

**Files:**
- Create: `src/tui/wander.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/panels/pet/ambient.rs:504`
- Test: in `src/tui/wander.rs`

**Interfaces:**
- Produces: `pub(crate) fn resolve_wander_offset(vm: &WatchViewModel, now: time::OffsetDateTime, habitat_width: u16) -> (i16, i8)`.
- Consumes: `crate::pet::animator::{compute_wander_position_x, compute_sleep_wander_x, compute_wake_wander_x, lazy_wander_instant, compute_facing}`, `crate::tui::day::resonant_prop_for_day`, `crate::game::habitat::catalog_prop` (via the moved `resonance_wander_bias`), `crate::storage::state::EarnedHabitatProp`, `crate::tui::panels::pet::ambient::effective_weekend_softening`, `crate::tui::view_model::WatchViewModel`.

- [ ] **Step 1: Widen `effective_weekend_softening`**

In `src/tui/panels/pet/ambient.rs:504`, change `pub(super) fn effective_weekend_softening` to `pub(crate) fn effective_weekend_softening`. (It's called from the new sibling-tree module `tui::wander`.) `cargo build` to confirm nothing else breaks.

- [ ] **Step 2: Write the failing characterization test**

Create `src/tui/wander.rs` with a test that independently recomputes the normal-arm result and asserts `resolve_wander_offset` matches (non-tautological — the assertion side calls the animator directly):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::animator::{compute_facing, compute_wander_position_x, lazy_wander_instant};
    use crate::tui::view_model::WatchViewModel;

    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn normal_arm_matches_direct_animator_calls() {
        // fixture() is awake with no wake_resume → normal arm
        let vm = WatchViewModel::fixture();
        let now = fixed_now();
        let width = 60u16;
        let species = vm.pet_render.generated_species;
        let idle = vm.life_profile.idle.idle_minutes;
        let softening =
            crate::tui::panels::pet::ambient::effective_weekend_softening(&vm.day_context, &vm.life_profile);
        let wander_now = lazy_wander_instant(now, vm.day_context.local_day_started_utc, softening);
        let expect_x = compute_wander_position_x(width, species, wander_now, idle)
            + resonance_wander_bias(/* resonant prop from vm, see impl */ None);
        let expect_f = compute_facing(width, species, wander_now, idle);

        let (x, f) = resolve_wander_offset(&vm, now, width);
        assert_eq!((x, f), (expect_x, expect_f));
    }

    #[test]
    fn too_narrow_habitat_centers_and_faces_right() {
        // width <= 14 → half_range 0 → wander 0, facing +1 (animator guards)
        let vm = WatchViewModel::fixture();
        let (x, f) = resolve_wander_offset(&vm, fixed_now(), 14);
        assert_eq!((x, f), (0, 1));
    }
}
```

(If `WatchViewModel::fixture()` happens to carry an earned resonant prop, the `None` in `expect_x` will be wrong — read the fixture; if it has no resonant prop, `resonance_wander_bias(None)` is `0` and this holds. Adjust the expected `resonant` to whatever `resonant_prop_for_day` returns for the fixture, computed the same way the impl does.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib tui::wander`
Expected: FAIL — `resolve_wander_offset`/module not defined.

- [ ] **Step 4: Implement `resolve_wander_offset` + relocate `resonance_wander_bias`**

In `src/tui/wander.rs`, move the inline block from `PetPanel::render` verbatim into the function, and relocate `resonance_wander_bias` + `RESONANCE_WANDER_BIAS_CELLS` here:

```rust
use crate::pet::animator::{
    compute_facing, compute_sleep_wander_x, compute_wake_wander_x, compute_wander_position_x,
    lazy_wander_instant,
};
use crate::tui::panels::pet::ambient::effective_weekend_softening;
use crate::tui::view_model::WatchViewModel;

const RESONANCE_WANDER_BIAS_CELLS: i16 = 3;

/// Live pet horizontal drift + facing, resolved against `habitat_width`. Pure
/// function of the view model, the frame instant, and the panel width, so any
/// surface (watch, companion) gets identical motion by passing its own width.
pub(crate) fn resolve_wander_offset(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    habitat_width: u16,
) -> (i16, i8) {
    let species = vm.pet_render.generated_species;
    let day = &vm.day_context;
    let resonant_prop = {
        let earned: Vec<crate::storage::state::EarnedHabitatProp> = vm
            .habitat
            .earned_props
            .iter()
            .map(|prop| crate::storage::state::EarnedHabitatProp {
                id: prop.id.clone(),
                earned_at: prop.earned_at,
                source: prop.source.clone(),
            })
            .collect();
        crate::tui::day::resonant_prop_for_day(day, &earned)
    };
    let softening = effective_weekend_softening(day, &vm.life_profile);
    let idle_minutes = vm.life_profile.idle.idle_minutes;
    match (day.asleep, day.sleep_onset_utc, day.wake_resume) {
        (true, Some(onset), _) => (
            compute_sleep_wander_x(habitat_width, species, now, onset, idle_minutes),
            compute_facing(habitat_width, species, onset, idle_minutes),
        ),
        (false, _, Some(resume)) => (
            compute_wake_wander_x(
                habitat_width,
                species,
                now,
                resume.from_eval_utc,
                resume.woke_at_utc,
                idle_minutes,
            ),
            compute_facing(habitat_width, species, now, idle_minutes),
        ),
        _ => {
            let wander_now = lazy_wander_instant(now, day.local_day_started_utc, softening);
            (
                compute_wander_position_x(habitat_width, species, wander_now, idle_minutes)
                    + resonance_wander_bias(resonant_prop.as_ref()),
                compute_facing(habitat_width, species, wander_now, idle_minutes),
            )
        }
    }
}

fn resonance_wander_bias(resonant: Option<&crate::game::habitat::HabitatPropId>) -> i16 {
    // moved verbatim from src/tui/panels/pet.rs:130 — keep the exact zone→side mapping
    let Some(spec) = resonant.and_then(crate::game::habitat::catalog_prop) else {
        return 0;
    };
    use crate::game::habitat::HabitatPropZone::*;
    let side: i16 = match spec.zone {
        FloorLeft | WallLeft | AirLeft => -1,
        FloorRight | WallRight | AirRight => 1,
        FloorMid | AirMid | Ceiling => 0,
    };
    side * RESONANCE_WANDER_BIAS_CELLS
}
```

(Copy the real `resonance_wander_bias` body from `pet.rs:130` verbatim — match the exact `HabitatPropId`/`HabitatPropZone` paths and any `use` it relied on. The block above mirrors the grounding but the source file is authoritative.)

Declare `pub(crate) mod wander;` in `src/tui/mod.rs`. Then fix the test's `expect_x` to use the same `resonant_prop` the impl derives (call `resonant_prop_for_day` in the test the same way, or assert against the fixture's known prop state).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib tui::wander`
Expected: PASS.

- [ ] **Step 6: Full gate**

Run: `cargo test && cargo test --features dev-preview --test dev_preview && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS. At this point `pet.rs` still has its own `resonance_wander_bias` + inline block (removed in Task 2) — so there are temporarily TWO `resonance_wander_bias`. That's fine for this task (different modules); if clippy flags the `pet.rs` one as newly-unused because nothing else references it yet, leave it — Task 2 removes it. (If clippy hard-fails on it, do Task 2's removal in the same commit.)

- [ ] **Step 7: Commit**

```bash
git add src/tui/wander.rs src/tui/mod.rs src/tui/panels/pet/ambient.rs
git commit -m "feat: tui::wander::resolve_wander_offset — shared pet drift/facing resolver"
```

---

### Task 2: Route `PetPanel::render` through the resolver

**Files:**
- Modify: `src/tui/panels/pet.rs` (the inline block ~148-198; the `resonance_wander_bias` fn ~130; its test ~1467)
- Test: dev-preview watch goldens (oracle) + the relocated unit test

**Interfaces:**
- Consumes: `crate::tui::wander::resolve_wander_offset`.

- [ ] **Step 1: Reroute the call site**

In `src/tui/panels/pet.rs::render`, replace the entire inline block — from `let day = &vm.day_context;` (and the `resonant_prop`/`softening`/`idle_minutes` derivations) through the `let (wander_x, facing) = match (...) { ... };` — with:

```rust
let (wander_x, facing) = crate::tui::wander::resolve_wander_offset(vm, now, area.width);
```

Keep the `Cow`-vm write immediately after it unchanged (it still reads `wander_x`/`facing`). Keep `let now = ...` and `let species = vm.pet_render.generated_species;` only if `species` is still used later in `render`; otherwise remove the now-unused `species`/`day` locals (check `cargo build` warnings).

- [ ] **Step 2: Delete the moved `resonance_wander_bias` + relocate its test**

Delete `fn resonance_wander_bias` and `const RESONANCE_WANDER_BIAS_CELLS` from `pet.rs:130` (now in `tui::wander`). Drop any imports they alone needed. Move the test `resonance_wander_bias_points_toward_the_prop_zone` (`pet.rs:1467`) into `src/tui/wander.rs`'s test module (it tests the relocated function) — adjust the path to the now-module-local `resonance_wander_bias`. Read the test first; preserve its exact assertions.

- [ ] **Step 3: Regenerate goldens — expect NO change (oracle)**

Run: `cargo test --features dev-preview --test dev_preview`
Expected: PASS — watch `cells.json` byte-identical (the pet's column is unchanged; logic moved, not values). If a frame differs, the move altered an input — reconcile; do NOT re-bake.

- [ ] **Step 4: Full gate**

Run: `cargo test && cargo test --features dev-preview --test dev_preview && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS — clippy dead-code gate confirms the `pet.rs` `resonance_wander_bias` removal left nothing dangling.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs src/tui/wander.rs
git commit -m "refactor: PetPanel resolves wander/facing via tui::wander; drop inline copy"
```

---

## Self-Review

**Spec coverage (Track 2b-i):**
- Shared `resolve_wander_offset` reused across surfaces: Task 1 (create, `pub(crate)`), Task 2 (watch calls it). Companion calls it with round width in Plan 06 — no wander logic added there. ✓
- `PetPanel::render` no longer inlines the wander/facing selection (stop condition): Task 2. ✓
- Byte-stable (verbatim move; goldens oracle): Task 2 Step 3. ✓
- NOT a semantic split / nothing on `EffectState`: by construction (the resolver takes `habitat_width`). ✓

**Placeholder scan:** No TBD; all code shown. The one repo-authoritative detail (the exact `resonance_wander_bias` body + the fixture's resonant-prop state for the test's expected value) is flagged to copy from source rather than trust the sketch.

**Type consistency:** `resolve_wander_offset(vm, now, habitat_width) -> (i16, i8)` is used identically in the test, the `PetPanel` call site, and (future) the companion. `resonance_wander_bias` lives in exactly one place after Task 2.

**Out of scope:** cursor-tracked eyes (need cursor + hit_area, stay render-time); placement extraction (Plan 04); any `EffectState` change. The `Cow`-vm write stays at the watch call site (it's a watch render-loop concern; the resolver is pure).
