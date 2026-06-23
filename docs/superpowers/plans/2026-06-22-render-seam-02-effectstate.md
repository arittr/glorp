# Render Seam — Plan 02: EffectState (viewport-agnostic per-frame effects) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the three viewport/cursor-agnostic per-frame pet effects — `shimmer_role`, `twinkle`, `token_pop` — out of inline computation in `render_pet_inside` into a reusable per-frame `EffectState`, so the watch reads them from data and a future companion build can reuse the identical computation. Zero visible change.

**Architecture:** New `src/presentation/effect.rs` defines `EffectState { shimmer_role, twinkle, token_pop }` built per frame by `EffectState::from_vm(vm, now, color_capability)` (no `area`/cursor inputs — these three effects depend only on species + `now` + pet state). `render_pet_inside` calls it once and reads the three fields instead of computing them inline. The `token_pop` gate logic (`calm_mode`/`burst_level`/`Flat` + `compute_token_pop`) moves from `colors.rs::profile_token_pop` into `EffectState::from_vm`, and the now-redundant `profile_token_pop` is removed. Behavior-preserving; dev-preview goldens byte-stable.

**Tech Stack:** Rust; `crate::pet::animator` (`compute_shimmer_role`, `compute_twinkle`/`TwinkleSpec`, `compute_token_pop`/`TokenPop`), `crate::tui::view_model::WatchViewModel`, `crate::tui::style::ColorCapability`, `crate::pet::render::PaletteRoleName`.

This is **Plan 02** of the render-seam re-arch — spec `docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`, **Track 2a**. Track 2b (Plan 03) handles the wander/facing semantic-vs-viewport split + grounding/ambient/props placement extraction; do NOT attempt those here.

## Global Constraints

- **`src/pet/render.rs` and `src/pet/art.rs` are FROZEN.** (`src/pet/animator.rs` may gain a derive but no logic change.)
- **`src/presentation/` must NOT import `tui::component::TargetPath`.** (Importing `crate::tui::view_model`, `crate::tui::style::ColorCapability`, `crate::tui::life::PetLifeProfile` via the vm, and `crate::pet::*` is fine — `scene.rs` already imports `tui::view_model`/`tui::room`.)
- **`EffectState::from_vm` is surface-agnostic and per-frame:** its only inputs are `&WatchViewModel`, `now`, and `ColorCapability` — NO `area`, `Rect`, viewport, or cursor. It is called once per frame (`now` drives the animation), not stored as a vm field.
- **dev-preview goldens BYTE-STABLE** (no re-bake — this plan changes no rendered output).
- **No behavior change.**
- **Per-task gate (full suite):** `cargo test` AND `cargo test --features dev-preview --test dev_preview` AND `cargo clippy --all-targets --all-features -- -D warnings` AND `cargo fmt --check`.
- **Commit per task.**

## File Structure

- **Create** `src/presentation/effect.rs` — `EffectState` struct + `EffectState::from_vm`. Owns the `token_pop` gate (moved from `colors.rs`).
- **Modify** `src/presentation/mod.rs` — `pub mod effect;` + re-export `EffectState`.
- **Modify** `src/tui/panels/pet.rs` (`render_pet_inside`, ~lines 405-412) — replace the three inline computations with one `EffectState::from_vm` call + field reads.
- **Modify** `src/tui/panels/pet/colors.rs` — remove `profile_token_pop` (logic moved to `effect.rs`); drop its now-unused `compute_token_pop` import if orphaned.
- **Maybe modify** `src/pet/animator.rs` — add `#[derive(PartialEq, Eq)]` to `TokenPop` if missing (so the `EffectState` test can compare). No logic change.

---

### Task 1: `EffectState` struct + `from_vm`

**Files:**
- Create: `src/presentation/effect.rs`
- Modify: `src/presentation/mod.rs`
- Modify (maybe): `src/pet/animator.rs` (derive only)
- Test: in `src/presentation/effect.rs`

**Interfaces:**
- Consumes: `crate::pet::animator::{compute_shimmer_role, compute_twinkle, compute_token_pop, TwinkleSpec, TokenPop}`, `crate::pet::render::PaletteRoleName`, `crate::tui::view_model::WatchViewModel`, `crate::tui::style::ColorCapability`.
- Produces: `pub struct EffectState { pub shimmer_role: Option<PaletteRoleName>, pub twinkle: Option<TwinkleSpec>, pub token_pop: Option<TokenPop> }` and `pub fn EffectState::from_vm(vm: &WatchViewModel, now: time::OffsetDateTime, color_capability: ColorCapability) -> EffectState`.

- [ ] **Step 1: Ensure `TokenPop`/`TwinkleSpec` are comparable**

Check `src/pet/animator.rs`: `TwinkleSpec` and `TokenPop` must derive `PartialEq` (and `Eq`) so the test can `assert_eq!`. If either lacks it, add `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` (these are plain value structs — no logic change). Run `cargo build` to confirm.

- [ ] **Step 2: Write the failing tests**

In `src/presentation/effect.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::animator::{compute_shimmer_role, compute_twinkle};
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;

    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn from_vm_reproduces_shimmer_and_twinkle() {
        let vm = WatchViewModel::fixture();
        let now = fixed_now();
        let species = vm.pet_render.generated_species;
        let fx = EffectState::from_vm(&vm, now, ColorCapability::Truecolor);
        assert_eq!(fx.shimmer_role, compute_shimmer_role(species, now));
        assert_eq!(fx.twinkle, compute_twinkle(species, now, vm.life_profile.idle.idle_minutes));
    }

    #[test]
    fn flat_capability_suppresses_token_pop() {
        let vm = WatchViewModel::fixture();
        let fx = EffectState::from_vm(&vm, fixed_now(), ColorCapability::Flat);
        assert!(fx.token_pop.is_none(), "Flat capability must suppress token-pop (matches profile_token_pop gate)");
    }

    #[test]
    fn calm_mode_suppresses_token_pop() {
        let mut vm = WatchViewModel::fixture();
        vm.life_profile.calm_mode = true;
        let fx = EffectState::from_vm(&vm, fixed_now(), ColorCapability::Truecolor);
        assert!(fx.token_pop.is_none(), "calm_mode must suppress token-pop");
    }

    #[test]
    fn zero_burst_suppresses_token_pop() {
        let mut vm = WatchViewModel::fixture();
        vm.life_profile.calm_mode = false;
        vm.life_profile.burst_level = 0.0;
        let fx = EffectState::from_vm(&vm, fixed_now(), ColorCapability::Truecolor);
        assert!(fx.token_pop.is_none(), "burst_level <= 0 must suppress token-pop");
    }
}
```

These three gate tests (`flat`/`calm`/`zero_burst`) deliberately mirror the three conditions asserted by the existing `profile_token_pop` tests at `pet.rs:919-939`, which Task 2 removes — so coverage transfers, it does not vanish.

If `WatchViewModel::fixture()` is not reachable from this module (e.g. it is `#[cfg(test)]` in `view_model.rs` with restricted visibility), make it reachable: prefer widening it to `pub(crate)` under its existing `#[cfg(test)]`/test-support gate. Do NOT hand-construct a 30-field `WatchViewModel` literal. If `fixture()` truly cannot be exposed, report BLOCKED with what you found rather than guessing.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib presentation::effect`
Expected: FAIL — `EffectState`/`from_vm` not defined.

- [ ] **Step 4: Implement `EffectState` + `from_vm`**

In `src/presentation/effect.rs` (the `token_pop` gate is copied verbatim from the current `colors.rs::profile_token_pop`):

```rust
use crate::pet::animator::{compute_shimmer_role, compute_token_pop, compute_twinkle, TokenPop, TwinkleSpec};
use crate::pet::render::PaletteRoleName;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;

/// The per-frame, surface-agnostic pet effects: a wisp-shimmer role, an
/// occasional twinkle, and the post-feed token-pop flash. All three depend only
/// on the pet's species and the current instant (plus idle/feed state) — never on
/// the viewport or cursor — so any surface can build the identical `EffectState`
/// from the view model and the frame's `now`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectState {
    pub shimmer_role: Option<PaletteRoleName>,
    pub twinkle: Option<TwinkleSpec>,
    pub token_pop: Option<TokenPop>,
}

impl EffectState {
    pub fn from_vm(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
        color_capability: ColorCapability,
    ) -> EffectState {
        let species = vm.pet_render.generated_species;
        EffectState {
            shimmer_role: compute_shimmer_role(species, now),
            twinkle: compute_twinkle(species, now, vm.life_profile.idle.idle_minutes),
            token_pop: token_pop_for(vm, now, color_capability),
        }
    }
}

/// Post-feed flash, gated off in calm mode, with no burst, or on flat terminals.
/// (Moved verbatim from the former `colors.rs::profile_token_pop`.)
fn token_pop_for(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
) -> Option<TokenPop> {
    if vm.life_profile.calm_mode
        || vm.life_profile.burst_level <= 0.0
        || matches!(color_capability, ColorCapability::Flat)
    {
        return None;
    }
    compute_token_pop(vm.last_feed_pulse_at, now)
}
```

In `src/presentation/mod.rs`: add `pub mod effect;` and `pub use effect::EffectState;`.

(Confirm `EffectState` derives `Eq` only if `TokenPop`/`TwinkleSpec` do; if `TwinkleSpec` holds a `char` that's fine, but if either is not `Eq`, drop `Eq` from the `EffectState` derive and keep `PartialEq`.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib presentation::effect`
Expected: PASS (4 tests).

- [ ] **Step 6: Full gate**

Run: `cargo test && cargo test --features dev-preview --test dev_preview && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS. (`token_pop_for` has a caller via `from_vm`; nothing dead. The old `colors.rs::profile_token_pop` is still present and still used by `render_pet_inside` at this point — that's fine, it's removed in Task 2.)

- [ ] **Step 7: Commit**

```bash
git add src/presentation/effect.rs src/presentation/mod.rs src/pet/animator.rs
git commit -m "feat: EffectState — per-frame shimmer/twinkle/token_pop as surface-agnostic data"
```

---

### Task 2: Route `render_pet_inside` through `EffectState`; remove the old `profile_token_pop`

**Files:**
- Modify: `src/tui/panels/pet.rs` (`render_pet_inside`, ~lines 405-412)
- Modify: `src/tui/panels/pet/colors.rs` (remove `profile_token_pop`)
- Test: dev-preview watch goldens (the oracle)

**Interfaces:**
- Consumes: `crate::presentation::EffectState`.

- [ ] **Step 1: Reroute the read site**

In `src/tui/panels/pet.rs::render_pet_inside`, replace the three inline computations (currently `let shimmer_role = compute_shimmer_role(species, now);`, `let twinkle = compute_twinkle(species, now, vm.life_profile.idle.idle_minutes);`, and the `let token_pop = profile_token_pop(vm.last_feed_pulse_at, &vm.life_profile, color_capability, now);` block) with:

```rust
let effects = crate::presentation::EffectState::from_vm(vm, now, color_capability);
let shimmer_role = effects.shimmer_role;
let twinkle = effects.twinkle;
let token_pop = effects.token_pop;
```

Leave everything downstream unchanged — `effective_shimmer_role`, `shimmer_m`, `effective_twinkle`, the `watch_live_color_inputs(... token_pop.is_some() ...)` call, and the sparkle override all keep reading the same `shimmer_role`/`twinkle`/`token_pop` locals. `species` may become unused after this; remove its binding if so (or keep if still referenced elsewhere in the function — check `cargo build` warnings).

- [ ] **Step 2: Remove the now-redundant `profile_token_pop`**

In `src/tui/panels/pet/colors.rs`, delete `pub(super) fn profile_token_pop` (its logic now lives in `effect.rs::token_pop_for`). If `compute_token_pop` was imported only for it, drop that import; also remove `profile_token_pop` from the `use` list at `pet.rs:40`.

`profile_token_pop` has three tests at `src/tui/panels/pet.rs:919-939` (each `assert!(profile_token_pop(...))`) — read them. They assert exactly the three gate conditions (calm / zero-burst / Flat → suppressed), which Task 1 now covers in `effect.rs`. Remove these three pet.rs tests. If any of them asserts something the `effect.rs` tests do NOT cover (e.g. a specific positive-case `TokenPop` value rather than just the gate), add the missing assertion to `effect.rs` first, then remove the pet.rs test — do not drop coverage.

- [ ] **Step 3: Regenerate goldens — expect NO change (the oracle)**

Run: `cargo test --features dev-preview --test dev_preview`
Expected: PASS — watch `cells.json` byte-identical. The computation moved location but not value (`now`, species, and all inputs are identical), so goldens MUST be unchanged. If any frame differs, the reroute changed an input — reconcile; do NOT re-bake.

- [ ] **Step 4: Full gate**

Run: `cargo test && cargo test --features dev-preview --test dev_preview && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS — including clippy's dead-code gate (confirms `profile_token_pop` removal left no dangling caller/import).

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs src/tui/panels/pet/colors.rs
git commit -m "refactor: watch reads shimmer/twinkle/token_pop from EffectState; drop profile_token_pop"
```

---

## Self-Review

**Spec coverage (Track 2a):**
- `EffectState` for the three viewport-agnostic effects: Task 1 (struct + `from_vm`), Task 2 (watch reads from it). ✓
- Per-frame, surface-agnostic (`from_vm` takes no `area`/cursor): enforced by the signature + tests. ✓
- `render_pet_inside` no longer computes shimmer/twinkle/token_pop inline (Track 2a stop condition): Task 2. ✓
- Constraints: `render_pet`/`art.rs` frozen (only `animator.rs` gains a derive); `presentation/` imports no `tui::component::TargetPath` (only `tui::view_model`/`tui::style`/`pet::*`); goldens byte-stable (Task 2 oracle). ✓

**Placeholder scan:** No TBD/TODO; all new code shown; the one repo-specific unknown (`WatchViewModel::fixture()` reachability) has an explicit resolution path + a BLOCKED fallback rather than a guess.

**Type consistency:** `EffectState`, `from_vm`, `token_pop_for`, the field names (`shimmer_role`/`twinkle`/`token_pop`) are consistent across Tasks 1-2 and match the consumer reads in `render_pet_inside`.

**Out of scope (Track 2b / Plan 03, do not do here):** wander/facing semantic-vs-viewport split (needs `animator.rs` logic refactor); grounding/ambient/props/performance placement extraction; cursor-eye handling; folding `breath_offset_y` (already a vm field) into `EffectState`. `EffectState` deliberately holds only the three computed-and-lost effects this plan extracts; it grows in later plans.
