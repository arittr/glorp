# Companion "tank in a growth ring" Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the macOS round companion into a free-float "tank" — a bigger roaming pet, an open-bottom growth ring with a rate comet, a mood-colored aura replacing the invisible vital ticks, and one clean stat nested in the ring gap.

**Architecture:** All new *logic* (mood→color, drift bounds, ring/comet/gap geometry) lives in **cfg-free `src/round/`** so CI lints and golden-tests it. Only AppKit *painting* (`NSBezierPath` ovals/arcs, text) lives in the macOS-gated `src/companion/app.rs`. `NSGradient` is not bound, so soft glows are drawn as concentric translucent circles. The pet's drift is parameterized via a `CompanionMotion` config so the companion can roam wider without changing the shared menubar popover.

**Tech Stack:** Rust, ratatui (`Rect`), objc2 / objc2-app-kit (AppKit), insta (snapshots).

## Global Constraints

- **Pure helpers go in cfg-free `src/round/`**, never in `src/companion/app.rs` (`#![cfg(target_os="macos")]` — invisible to the ubuntu-only `clippy --all-targets --all-features -D warnings` gate). Verbatim from spec.
- **`CompanionMotion::default()` MUST equal today's behavior** (`wander_half:8, drift_x_frac:0.45, drift_y_frac:0.30, drift_period_secs:20, upward_bias:0.0`) so the menubar / preview / goldens stay byte-identical. Only the companion call site uses tuned values.
- **No `NSGradient`** (not bound) — soft fills are concentric translucent circles via `ns_color(&RoundColor(r,g,b,a)).setFill(); path.fill();`.
- **No numerals / %, no ETA on the growth ring.** Surface only real observed state.
- `cargo fmt --check` and `cargo clippy --all-targets --all-features -D warnings` must stay clean (locally via lefthook for the macOS code; in CI for `src/round/`).
- Pet art is `PET_W=13 × PET_H=10` cells (`src/round/scene.rs`). Do not change these.

---

### Task 1: `mood_aura_color` helper

**Files:**
- Create: `src/round/hud.rs`
- Modify: `src/round/mod.rs` (add `pub mod hud;`)
- Test: in `src/round/hud.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::game::metabolism::Mood`, `crate::round::draw::RoundColor`.
- Produces: `pub fn mood_aura_color(mood: Mood) -> RoundColor` — opaque hue (alpha `1.0`); the painter applies its own per-ring alpha. All seven moods distinct; **Sad ≠ Sleepy**.

- [ ] **Step 1: Add the module declaration**

In `src/round/mod.rs`, add the line (keep alphabetical with the existing `pub mod` lines):

```rust
pub mod hud;
```

- [ ] **Step 2: Write the failing test**

Create `src/round/hud.rs` with:

```rust
//! Pure, cross-platform geometry and color helpers for the round companion HUD
//! (growth ring, rate comet, stat gap, mood aura color). No AppKit; golden-testable.

use crate::game::metabolism::Mood;
use crate::round::draw::RoundColor;

/// Soft-glow aura hue for the pet's mood. Opaque (alpha 1.0); the renderer
/// applies its own translucency. Sad and Sleepy are deliberately distinct hues
/// (different needs: happiness<35 vs energy<20). Starting palette — tuned on device.
pub fn mood_aura_color(mood: Mood) -> RoundColor {
    match mood {
        Mood::Content => RoundColor(0.25, 0.71, 0.60, 1.0),  // teal
        Mood::Happy => RoundColor(0.82, 0.45, 0.62, 1.0),    // warm pink
        Mood::Ecstatic => RoundColor(0.95, 0.40, 0.70, 1.0), // bright magenta-pink
        Mood::Hungry => RoundColor(0.85, 0.62, 0.30, 1.0),   // amber
        Mood::Sad => RoundColor(0.40, 0.50, 0.78, 1.0),      // muted blue
        Mood::Sleepy => RoundColor(0.55, 0.50, 0.80, 1.0),   // indigo/violet
        Mood::Wilted => RoundColor(0.45, 0.40, 0.48, 1.0),   // dim grey-mauve
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mood_has_a_distinct_aura_color() {
        let moods = [
            Mood::Content, Mood::Happy, Mood::Ecstatic, Mood::Hungry,
            Mood::Sad, Mood::Sleepy, Mood::Wilted,
        ];
        let colors: Vec<RoundColor> = moods.iter().map(|m| mood_aura_color(*m)).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(
                    colors[i], colors[j],
                    "moods {:?} and {:?} must have distinct aura colors",
                    moods[i], moods[j]
                );
            }
        }
    }

    #[test]
    fn sad_and_sleepy_are_distinct() {
        assert_ne!(mood_aura_color(Mood::Sad), mood_aura_color(Mood::Sleepy));
    }
}
```

- [ ] **Step 3: Run the test to verify it passes** (implementation and test are written together here since the helper is a pure lookup)

Run: `cargo test --lib round::hud::tests`
Expected: PASS (2 tests)

- [ ] **Step 4: Lint**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean

- [ ] **Step 5: Commit**

```bash
git add src/round/hud.rs src/round/mod.rs
git commit -m "feat(companion): mood_aura_color — distinct per-mood aura hues"
```

---

### Task 2: `CompanionMotion` config + parameterized drift

**Files:**
- Modify: `src/round/scene.rs` (replace the `DRIFT_*` / `COMPANION_WANDER_HALF` consts and `companion_drift`)
- Test: `src/round/scene.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  - `pub struct CompanionMotion { pub wander_half: u16, pub drift_x_frac: f32, pub drift_y_frac: f32, pub drift_period_secs: u64, pub upward_bias: f32 }` with `Default` = today's values.
  - `fn companion_drift_position(motion: &CompanionMotion, grid_cols: u16, grid_rows: u16, fx: f32, fy: f32) -> (u16, u16)` (pure cell mapping).
  - `fn companion_drift(now, motion: &CompanionMotion, grid_cols, grid_rows) -> (u16, u16)`.
- Consumes (later tasks): `build_round_scene_draw_list` calls `companion_drift` with a `&CompanionMotion`.

- [ ] **Step 1: Write the failing test** (drop into `src/round/scene.rs` tests)

```rust
#[test]
fn companion_motion_default_matches_legacy_drift_values() {
    let m = CompanionMotion::default();
    assert_eq!(m.wander_half, 8);
    assert_eq!(m.drift_x_frac, 0.45);
    assert_eq!(m.drift_y_frac, 0.30);
    assert_eq!(m.drift_period_secs, 20);
    assert_eq!(m.upward_bias, 0.0);
}

#[test]
fn upward_bias_lifts_the_pet() {
    // With a positive upward bias the pet's top-left row is <= the unbiased row
    // for the same normalized offset (smaller row = higher on screen).
    let base = CompanionMotion::default();
    let biased = CompanionMotion { upward_bias: 0.5, ..CompanionMotion::default() };
    let (_, y0) = companion_drift_position(&base, 32, 16, 0.0, 0.0);
    let (_, y1) = companion_drift_position(&biased, 32, 16, 0.0, 0.0);
    assert!(y1 <= y0, "upward bias should not move the pet down (y1={y1}, y0={y0})");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib round::scene::tests::companion_motion_default_matches_legacy_drift_values`
Expected: FAIL — `CompanionMotion` not found.

- [ ] **Step 3: Replace the const block + `companion_drift`**

In `src/round/scene.rs`, delete the four tuning consts (`COMPANION_WANDER_HALF`, `DRIFT_X_FRAC`, `DRIFT_Y_FRAC`, `DRIFT_PERIOD_SECS`) — keep `PET_W` / `PET_H` — and replace them with:

```rust
/// Companion motion config. Defaults reproduce the historical drift exactly, so
/// the shared menubar / preview / goldens are byte-identical; only the companion
/// call site passes tuned values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompanionMotion {
    /// Half-width of the pet's wander range, in cells (`PET_W + 2*wander_half`).
    pub wander_half: u16,
    /// Fraction of the safe horizontal radius used for drift. Keep modest
    /// (~0.45); higher values clip the pixel rim on a smaller grid.
    pub drift_x_frac: f32,
    /// Fraction of the safe vertical radius used for drift. Cells are ~2:1, so
    /// vertical headroom is tiny — keep gentle.
    pub drift_y_frac: f32,
    /// Drift cadence: the target changes every this many seconds.
    pub drift_period_secs: u64,
    /// Fraction of the safe vertical radius to shift the roam center UP, reserving
    /// the bottom band for the stat. 0.0 = centered.
    pub upward_bias: f32,
}

impl Default for CompanionMotion {
    fn default() -> Self {
        Self {
            wander_half: 8,
            drift_x_frac: 0.45,
            drift_y_frac: 0.30,
            drift_period_secs: 20,
            upward_bias: 0.0,
        }
    }
}
```

Replace `companion_drift` with this pair (the offset hashing is unchanged; only the position mapping is parameterized):

```rust
/// Deterministic normalized drift offsets in [-1, 1] per axis for `now`, eased
/// (smoothstep) between per-epoch targets.
fn companion_drift_offsets(now: time::OffsetDateTime, period_secs: u64) -> (f32, f32) {
    let unix = now.unix_timestamp() as u64;
    let period = period_secs.max(1);
    let epoch = unix / period;
    let phase = (unix % period) as f32 / period as f32;

    let target_for_epoch = |e: u64| -> (f32, f32) {
        let h1 = e
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(0x6c62_272e_07bb_0142);
        let h2 = h1
            .wrapping_mul(0x517c_c1b7_2722_0a95)
            .wrapping_add(0xbf87_8c2f_a7a4_c6a5);
        let nx = ((h1 >> 32) as i32 as f32) / (i32::MAX as f32);
        let ny = ((h2 >> 32) as i32 as f32) / (i32::MAX as f32);
        (nx, ny)
    };

    let (px, py) = target_for_epoch(epoch.saturating_sub(1));
    let (nx, ny) = target_for_epoch(epoch);
    let t = phase * phase * (3.0 - 2.0 * phase);
    (px + (nx - px) * t, py + (ny - py) * t)
}

/// Map normalized offsets `(fx, fy)` to the pet art's top-left grid cell, applying
/// the motion config's radii, upward bias, and the rectangular grid clamp.
fn companion_drift_position(
    motion: &CompanionMotion,
    grid_cols: u16,
    grid_rows: u16,
    fx: f32,
    fy: f32,
) -> (u16, u16) {
    let cx = grid_cols / 2;
    let cy = grid_rows / 2;
    let half_w = PET_W / 2;
    let half_h = PET_H / 2;
    let safe_x = cx.saturating_sub(half_w) as f32;
    let safe_y = cy.saturating_sub(half_h) as f32;
    let x_radius = safe_x * motion.drift_x_frac;
    let y_radius = safe_y * motion.drift_y_frac;
    let bias = motion.upward_bias * safe_y;

    let art_x = cx as i32 - half_w as i32 + (fx * x_radius) as i32;
    let art_y = cy as i32 - half_h as i32 - bias as i32 + (fy * y_radius) as i32;

    let art_x = art_x.clamp(0, (grid_cols.saturating_sub(PET_W)) as i32) as u16;
    let art_y = art_y.clamp(0, (grid_rows.saturating_sub(PET_H)) as i32) as u16;
    (art_x, art_y)
}

/// Gentle, deterministic 2D drift for the pet — top-left of the `PET_W × PET_H`
/// art rect in grid coords. The reachable set is a BOX (independent X/Y hashes),
/// not an ellipse: callers needing a bound must sample box corners.
fn companion_drift(
    now: time::OffsetDateTime,
    motion: &CompanionMotion,
    grid_cols: u16,
    grid_rows: u16,
) -> (u16, u16) {
    let (fx, fy) = companion_drift_offsets(now, motion.drift_period_secs);
    companion_drift_position(motion, grid_cols, grid_rows, fx, fy)
}
```

In `build_round_scene_draw_list` (same file), update the two current uses:
- `let wander_width = PET_W + 2 * COMPANION_WANDER_HALF;` → take a `motion: &CompanionMotion` param (added in Task 4) — for now, temporarily use `let motion = CompanionMotion::default();` at the top of the function and `let wander_width = PET_W + 2 * motion.wander_half;`
- `let (drift_x, drift_y) = companion_drift(now, grid_cols, grid_rows);` → `let (drift_x, drift_y) = companion_drift(now, &motion, grid_cols, grid_rows);`

(The `motion` param is threaded through properly in Task 4; this temporary local keeps the build green between tasks.)

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib round::scene::tests`
Expected: PASS — including the existing `build_round_scene_draw_list_*` determinism/bounds tests (default motion reproduces old positions exactly).

- [ ] **Step 5: Lint + commit**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

```bash
git add src/round/scene.rs
git commit -m "refactor(companion): parameterize pet drift via CompanionMotion (default = legacy)"
```

---

### Task 3: Bounded-drift invariant helper + guard test

**Files:**
- Modify: `src/round/scene.rs`
- Test: `src/round/scene.rs` tests

**Interfaces:**
- Produces: `pub fn drift_keeps_pet_in_aperture(motion: &CompanionMotion, grid_cols: u16, grid_rows: u16, cell_w: f64, cell_h: f64, aperture_radius_px: f64) -> bool` — true iff, for every box-corner drift target, all four corners of the pet rect map inside the pixel aperture circle.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn default_motion_keeps_pet_inside_a_960_aperture() {
    // Representative production metrics: 960px square face, 32 cols → cell_w=30,
    // cells ~2:1 → cell_h=60, rows=16, aperture radius = 960/2 - 1 = 479.
    let m = CompanionMotion::default();
    assert!(
        drift_keeps_pet_in_aperture(&m, 32, 16, 30.0, 60.0, 479.0),
        "default drift must keep the whole pet inside the aperture circle"
    );
}

#[test]
fn over_wide_x_fraction_clips_the_rim() {
    // The spec's rejected 0.70 must be caught by the guard (corner reaches ~516 > 479).
    let m = CompanionMotion { drift_x_frac: 0.70, ..CompanionMotion::default() };
    assert!(
        !drift_keeps_pet_in_aperture(&m, 32, 16, 30.0, 60.0, 479.0),
        "0.70 X fraction should be rejected — the pet corner clips the rim"
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib round::scene::tests::default_motion_keeps_pet_inside_a_960_aperture`
Expected: FAIL — `drift_keeps_pet_in_aperture` not found.

- [ ] **Step 3: Implement the helper**

```rust
/// Conservative bounded-drift check. Samples the drift at every box corner
/// (`fx, fy ∈ {-1, 0, 1}`), maps each of the pet rect's four corners from cell
/// space to pixels (using the real, non-square `cell_w`/`cell_h`), and verifies
/// they all sit inside the pixel aperture circle. The grid is centered in the
/// view, so a corner's pixel distance from the aperture center is
/// `sqrt((cell_w·(col − cols/2))² + (cell_h·(row − rows/2))²)`.
pub fn drift_keeps_pet_in_aperture(
    motion: &CompanionMotion,
    grid_cols: u16,
    grid_rows: u16,
    cell_w: f64,
    cell_h: f64,
    aperture_radius_px: f64,
) -> bool {
    let cxg = grid_cols as f64 / 2.0;
    let cyg = grid_rows as f64 / 2.0;
    for &fx in &[-1.0f32, 0.0, 1.0] {
        for &fy in &[-1.0f32, 0.0, 1.0] {
            let (ax, ay) = companion_drift_position(motion, grid_cols, grid_rows, fx, fy);
            let corners = [
                (ax, ay),
                (ax + PET_W, ay),
                (ax, ay + PET_H),
                (ax + PET_W, ay + PET_H),
            ];
            for (col, row) in corners {
                let dx = cell_w * (col as f64 - cxg);
                let dy = cell_h * (row as f64 - cyg);
                if (dx * dx + dy * dy).sqrt() > aperture_radius_px {
                    return false;
                }
            }
        }
    }
    true
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib round::scene::tests`
Expected: PASS (both new tests).

- [ ] **Step 5: Lint + commit**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

```bash
git add src/round/scene.rs
git commit -m "feat(companion): bounded-drift invariant — box-corner pixel-aperture check"
```

---

### Task 4: `build_round_scene_draw_list` → `CompanionScene { draw_list, pet_rect }` + motion param

**Files:**
- Modify: `src/round/scene.rs` (signature + return)
- Modify callers: `src/companion/app.rs:495`, `src/menubar/render.rs:59`, `src/round/preview.rs:21`
- Modify tests: `src/round/scene.rs` (4 `build_round_scene_draw_list_*`), `src/presentation/rasterize.rs:178`, `tests/round_draw_list.rs:46`

**Interfaces:**
- Produces:
  - `pub struct CompanionScene { pub draw_list: SceneDrawList, pub pet_rect: ratatui::layout::Rect }`
  - `pub fn build_round_scene_draw_list(vm, now, grid_cols, grid_rows, motion: &CompanionMotion) -> CompanionScene`
- `pet_rect` is the drift rect `Rect::new(drift_x, drift_y, PET_W, PET_H)`; the aura uses it (≤1-cell posture slop ignored — acceptable for a soft glow).

- [ ] **Step 1: Update the signature + return**

In `src/round/scene.rs`, change the function. Add at the top of the module (near `CompanionMotion`):

```rust
/// The companion's rendered scene: the draw list plus the pet's drift rect (in
/// grid cells), which the AppKit layer turns into a pixel center for the aura.
#[derive(Debug, Clone, PartialEq)]
pub struct CompanionScene {
    pub draw_list: SceneDrawList,
    pub pet_rect: Rect,
}
```

Change the signature to:

```rust
pub fn build_round_scene_draw_list(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    grid_cols: u16,
    grid_rows: u16,
    motion: &CompanionMotion,
) -> CompanionScene {
```

Remove the temporary `let motion = CompanionMotion::default();` added in Task 2. Use `motion.wander_half` for `wander_width` and `companion_drift(now, motion, grid_cols, grid_rows)` as already wired. At the end, replace `scene_list` with:

```rust
    CompanionScene {
        draw_list: scene_list,
        pet_rect: new_pet_art,
    }
}
```

(`new_pet_art` is the existing `Rect::new(drift_x, drift_y, PET_W, PET_H)` local.)

- [ ] **Step 2: Update the four scene.rs golden tests**

In each `build_round_scene_draw_list_*` test, pass `&CompanionMotion::default()` and read `.draw_list`. Example for the determinism test (apply the same pattern to all four):

```rust
#[test]
fn build_round_scene_draw_list_is_deterministic() {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let m = CompanionMotion::default();
    let a = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &m);
    let b = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS, &m);
    assert_eq!(a.draw_list.cells, b.draw_list.cells, "must be deterministic");
}
```

For the bounds/nonempty/pet-cells tests, replace `list.cells` with `list.draw_list.cells`.

- [ ] **Step 3: Update the companion caller** (`src/companion/app.rs` ~495)

```rust
    if let Some(m) = companion_grid_metrics(bounds.size.width, bounds.size.height) {
        let companion_scene = crate::round::scene::build_round_scene_draw_list(
            &vm,
            now,
            m.grid_cols,
            m.grid_rows,
            &companion_motion(),
        );
        appkit_blit_draw_list(
            &companion_scene.draw_list,
            m.font_size,
            m.cell_w,
            m.cell_h,
            m.origin_x,
            m.origin_y,
        );
        // companion_scene.pet_rect is consumed by the aura in Task 9.
    }
```

Add near the top of `src/companion/app.rs` (after the constants):

```rust
/// The companion's drift config (tuned on device). Starts at the legacy default;
/// diverge here WITHOUT touching the shared menubar popover.
fn companion_motion() -> crate::round::scene::CompanionMotion {
    crate::round::scene::CompanionMotion::default()
}
```

- [ ] **Step 4: Update the menubar + preview callers**

`src/menubar/render.rs` ~59:

```rust
    let scene = crate::round::scene::build_round_scene_draw_list(
        vm,
        now,
        POPOVER_COLUMNS as u16,
        MENU_SCENE_ROWS as u16,
        &crate::round::scene::CompanionMotion::default(),
    );
    let mut attr =
        scene_draw_list_to_attributed(&scene.draw_list, POPOVER_COLUMNS as u16, MENU_SCENE_ROWS as u16);
```

`src/round/preview.rs` ~21: pass `&crate::round::scene::CompanionMotion::default()` to the call and read `.draw_list` wherever the returned list is used. Read the surrounding lines and apply the same `.draw_list` access.

- [ ] **Step 5: Update the rasterize + integration goldens**

`src/presentation/rasterize.rs` ~178:

```rust
    let m = crate::round::scene::CompanionMotion::default();
    let scene = build_round_scene_draw_list(&vm, NOW, COLS, ROWS, &m);
    let grid = rasterize(&scene.draw_list, COLS, ROWS);
```

`tests/round_draw_list.rs` ~46:

```rust
    let scene = build_round_scene_draw_list(&vm, GOLDEN_NOW, GOLDEN_GRID_COLS, GOLDEN_GRID_ROWS,
        &glorp::round::scene::CompanionMotion::default());
    let records: Vec<CellRecord> = scene.draw_list.cells.iter().map(CellRecord::from).collect();
```

(Use the crate path that file already uses for `build_round_scene_draw_list`; mirror its existing imports.)

- [ ] **Step 6: Build, test, verify goldens are UNCHANGED**

Run: `cargo test`
Expected: PASS. The `insta` snapshots (`rasterize_content_lock_glyph_grid`, `round_draw_list_*`) must **not** change — default motion reproduces the old drift exactly. If insta reports a diff, the default values diverged from legacy — fix `CompanionMotion::default()`, do not accept the snapshot.

- [ ] **Step 7: Lint + commit**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

```bash
git add src/round/scene.rs src/companion/app.rs src/menubar/render.rs src/round/preview.rs src/presentation/rasterize.rs tests/round_draw_list.rs
git commit -m "feat(companion): build_round_scene_draw_list returns CompanionScene + takes motion"
```

---

### Task 5: Growth ring geometry

**Files:**
- Modify: `src/round/hud.rs`
- Test: `src/round/hud.rs` tests

**Interfaces:**
- Produces:
  - `pub struct GrowthRing { pub cx: f64, pub cy: f64, pub radius: f64, pub track_start_deg: f64, pub track_sweep_deg: f64 }`
  - `pub fn growth_ring_layout(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> GrowthRing`
  - `pub fn growth_ring_fill_end_deg(ring: &GrowthRing, fraction: f64) -> f64`
- Angles in AppKit convention (degrees, CCW from +x). Gap centered at the bottom (270°). Track sweeps CCW over the top from the gap's left edge to its right edge.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn ring_gap_is_centered_at_bottom_and_excluded() {
    let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
    // Track spans 360 - gap = 290 degrees.
    assert!((ring.track_sweep_deg - 290.0).abs() < 1e-6);
    // Track starts at the right edge of the bottom gap: 270 + 35 = 305 deg.
    assert!((ring.track_start_deg - 305.0).abs() < 1e-6);
    // Bottom (270°) is inside the gap, i.e. NOT covered by [start, start+sweep] mod 360.
    let end = ring.track_start_deg + ring.track_sweep_deg; // 595
    // 270 (== 630 mod 360) is not in [305, 595]; 630 would be, 270 is below start.
    assert!(!(305.0..=595.0).contains(&630.0) || !(305.0..=595.0).contains(&270.0));
}

#[test]
fn fill_end_spans_fraction_of_the_track() {
    let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
    assert!((growth_ring_fill_end_deg(&ring, 0.0) - ring.track_start_deg).abs() < 1e-6);
    assert!((growth_ring_fill_end_deg(&ring, 1.0) - (ring.track_start_deg + ring.track_sweep_deg)).abs() < 1e-6);
    let half = growth_ring_fill_end_deg(&ring, 0.5);
    assert!((half - (ring.track_start_deg + 145.0)).abs() < 1e-6);
    // Clamps out-of-range fractions.
    assert!((growth_ring_fill_end_deg(&ring, 2.0) - (ring.track_start_deg + ring.track_sweep_deg)).abs() < 1e-6);
    assert!((growth_ring_fill_end_deg(&ring, -1.0) - ring.track_start_deg).abs() < 1e-6);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib round::hud::tests::ring_gap_is_centered_at_bottom_and_excluded`
Expected: FAIL — `growth_ring_layout` not found.

- [ ] **Step 3: Implement**

```rust
/// Open-bottom growth ring geometry. Angles are degrees, CCW from +x (AppKit).
/// The gap is centered at the bottom (270°); the track sweeps CCW over the top
/// from the gap's right edge to its left edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrowthRing {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub track_start_deg: f64,
    pub track_sweep_deg: f64,
}

pub fn growth_ring_layout(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> GrowthRing {
    let gap = gap_deg.clamp(0.0, 180.0);
    GrowthRing {
        cx,
        cy,
        radius,
        track_start_deg: 270.0 + gap / 2.0,
        track_sweep_deg: 360.0 - gap,
    }
}

/// Angle (deg) where the violet fill ends for `fraction` of stage progress.
pub fn growth_ring_fill_end_deg(ring: &GrowthRing, fraction: f64) -> f64 {
    ring.track_start_deg + ring.track_sweep_deg * fraction.clamp(0.0, 1.0)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib round::hud::tests`
Expected: PASS

- [ ] **Step 5: Lint + commit**

```bash
git add src/round/hud.rs
git commit -m "feat(companion): open-bottom growth ring geometry"
```

---

### Task 6: Rate comet — phase + position

**Files:**
- Modify: `src/round/hud.rs`
- Test: `src/round/hud.rs` tests

**Interfaces:**
- Produces:
  - `pub fn comet_phase(frame: u64, rate_per_hour: f64) -> f64` — `[0, 1)`; advances even at `rate_per_hour == 0` (idle floor); faster when busy.
  - `pub fn comet_position(ring: &GrowthRing, phase: f64) -> (f64, f64)` — a point on the visible track (never inside the gap).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn comet_advances_even_when_idle() {
    // Nonzero baseline orbit: phase must change frame-to-frame at rate 0.
    let a = comet_phase(0, 0.0);
    let b = comet_phase(10, 0.0);
    assert_ne!(a, b, "comet must keep orbiting at zero rate (idle floor)");
}

#[test]
fn comet_is_faster_when_busy() {
    let idle = comet_phase(20, 0.0);
    let busy = comet_phase(20, 50_000_000.0);
    assert!(busy > idle, "higher token rate should advance the comet further by the same frame");
}

#[test]
fn comet_stays_on_the_visible_track() {
    let ring = growth_ring_layout(100.0, 100.0, 90.0, 70.0);
    for i in 0..100 {
        let phase = i as f64 / 100.0;
        let (x, y) = comet_position(&ring, phase);
        // On the circle of the given radius.
        let d = ((x - ring.cx).powi(2) + (y - ring.cy).powi(2)).sqrt();
        assert!((d - ring.radius).abs() < 1e-6, "comet must ride the ring radius");
        // Never in the bottom gap: its angle is within the track sweep.
        let ang = (y - ring.cy).atan2(x - ring.cx).to_degrees().rem_euclid(360.0);
        let start = ring.track_start_deg.rem_euclid(360.0);
        let rel = (ang - start).rem_euclid(360.0);
        assert!(rel <= ring.track_sweep_deg + 1e-6, "comet angle must lie on the track");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib round::hud::tests::comet_advances_even_when_idle`
Expected: FAIL — `comet_phase` not found.

- [ ] **Step 3: Implement**

```rust
/// Comet orbit phase in [0, 1). A nonzero baseline keeps it alive at idle; the
/// token rate adds speed on top. Pure function of the animation frame so it
/// animates every UI tick. Starting constants — tuned on device.
pub fn comet_phase(frame: u64, rate_per_hour: f64) -> f64 {
    const BASELINE_PER_FRAME: f64 = 1.0 / 40.0; // ~one lap / 10s at 4 fps
    const RATE_NORM: f64 = 50_000_000.0; // tokens/hr that doubles the orbit speed
    let speed = BASELINE_PER_FRAME * (1.0 + (rate_per_hour.max(0.0) / RATE_NORM));
    (frame as f64 * speed).rem_euclid(1.0)
}

/// Point on the visible track for `phase` (0 = track start, 1 = track end).
pub fn comet_position(ring: &GrowthRing, phase: f64) -> (f64, f64) {
    let ang_deg = ring.track_start_deg + ring.track_sweep_deg * phase.rem_euclid(1.0);
    let ang = ang_deg.to_radians();
    (ring.cx + ring.radius * ang.cos(), ring.cy + ring.radius * ang.sin())
}
```

- [ ] **Step 4: Run the tests + lint + commit**

Run: `cargo test --lib round::hud::tests`
Expected: PASS

```bash
git add src/round/hud.rs
git commit -m "feat(companion): rate comet phase + on-track position with idle floor"
```

---

### Task 7: Stat gap box

**Files:**
- Modify: `src/round/hud.rs`
- Test: `src/round/hud.rs` tests

**Interfaces:**
- Produces: `pub struct StatGap { pub center_x: f64, pub baseline_y: f64, pub max_width: f64 }` and `pub fn stat_gap_box(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> StatGap` — the box the token stat must fit inside (below center, within the gap chord).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn stat_gap_box_sits_below_center_and_within_the_chord() {
    let gap = stat_gap_box(100.0, 100.0, 90.0, 70.0);
    assert!((gap.center_x - 100.0).abs() < 1e-6, "centered horizontally");
    assert!(gap.baseline_y > 100.0, "stat sits below the vertical center (lower half)");
    // The gap chord half-width at the ring edges is radius * sin(gap/2).
    let expected_half = 90.0 * (35.0_f64.to_radians()).sin();
    assert!(gap.max_width <= 2.0 * expected_half + 1e-6, "stat must fit within the gap chord");
    assert!(gap.max_width > 0.0);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib round::hud::tests::stat_gap_box_sits_below_center_and_within_the_chord`
Expected: FAIL — `stat_gap_box` not found.

- [ ] **Step 3: Implement**

```rust
/// The region (in pixels) the token stat must fit inside: centered in the ring's
/// bottom gap, below center, clamped to the gap chord so it never clips the ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatGap {
    pub center_x: f64,
    pub baseline_y: f64,
    pub max_width: f64,
}

pub fn stat_gap_box(cx: f64, cy: f64, radius: f64, gap_deg: f64) -> StatGap {
    let gap = gap_deg.clamp(0.0, 180.0);
    let half_chord = radius * (gap / 2.0).to_radians().sin();
    StatGap {
        center_x: cx,
        // Place the readout in the lower band, a bit above the gap mouth.
        baseline_y: cy + radius * 0.55,
        // A small inset keeps the text off the ring stroke.
        max_width: (2.0 * half_chord * 0.92).max(0.0),
    }
}
```

- [ ] **Step 4: Run the tests + lint + commit**

Run: `cargo test --lib round::hud::tests`
Expected: PASS

```bash
git add src/round/hud.rs
git commit -m "feat(companion): stat gap box clamped inside the ring opening"
```

---

### Task 8: AppKit — bigger pet (target cols) + tank depth gradient

**Files:**
- Modify: `src/companion/app.rs`

**Note:** Tasks 8–11 touch macOS-only AppKit code that is not unit-testable; the test cycle is `cargo build` (compile gate) + run the companion + on-device observation, as the spec acknowledges.

- [ ] **Step 1: Lower the target columns (bigger pet + props)**

In `src/companion/app.rs`, change:

```rust
const COMPANION_TARGET_COLS: u16 = 36;
```
to:
```rust
// Pet scale lever: fewer cols → larger cells → bigger pet AND props. Tuned on device.
const COMPANION_TARGET_COLS: u16 = 32;
```

- [ ] **Step 2: Add the tank depth gradient**

In `draw_scene`, after the background circle fill (~line 491) and before the scene blit (~line 494), insert concentric translucent circles (darker toward the rim):

```rust
        // Tank depth: concentric translucent rings, darker toward the rim, so the
        // porthole reads as depth rather than a flat void. (NSGradient isn't bound.)
        unsafe {
            const DEPTH_RINGS: usize = 7;
            for i in 0..DEPTH_RINGS {
                let t = i as f64 / DEPTH_RINGS as f64; // 0 center → ~1 rim
                let rr = aperture.radius as f64 * (1.0 - t);
                let ring = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                    NSPoint::new(
                        aperture.center_x as f64 - rr,
                        aperture.center_y as f64 - rr,
                    ),
                    NSSize::new(rr * 2.0, rr * 2.0),
                ));
                // Brighter core (additive translucency builds toward center).
                ns_color(&RoundColor(0.10, 0.11, 0.20, 0.05)).setFill();
                ring.fill();
            }
        }
```

- [ ] **Step 3: Build + run + observe**

Run: `cargo build --release`
Expected: compiles clean.
Run: `cargo run --release -- companion` (open it on the round display; quit with the menu). Observe the pet is noticeably bigger and the background has a soft center-lit depth. Adjust `COMPANION_TARGET_COLS` / ring count later in Task 12.

- [ ] **Step 4: Commit**

```bash
git add src/companion/app.rs
git commit -m "feat(companion): bigger pet (32 cols) + tank depth gradient"
```

---

### Task 9: AppKit — mood aura under the pet

**Files:**
- Modify: `src/companion/app.rs`

- [ ] **Step 1: Add `animation_frame` to the draw snapshot**

In `draw_scene`, extend the snapshot (~lines 435-438) to also capture the frame (needed by the comet in Task 10; added here so the snapshot shape is final):

```rust
    let state_snapshot = APP_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|s| (s.scene.clone(), s.vm.clone(), s.animation_frame))
    });
    let Some((scene, vm, animation_frame)) = state_snapshot else {
        return;
    };
```

- [ ] **Step 2: Draw the aura under the pet**

Inside the `if let Some(m) = companion_grid_metrics(...)` block (from Task 4), **before** `appkit_blit_draw_list`, draw the aura at the pet's pixel center using `companion_scene.pet_rect`:

```rust
        // Mood aura — soft radial glow (concentric translucent circles) centered
        // on the pet, color by mood. Drawn under the pet so the body sits on top.
        unsafe {
            let pr = companion_scene.pet_rect;
            let (cxp, cyp) = cell_to_point(
                pr.x + pr.width / 2,
                pr.y + pr.height / 2,
                m.cell_w,
                m.cell_h,
                m.origin_x,
                m.origin_y,
            );
            let base = crate::round::hud::mood_aura_color(scene.pet.mood);
            let max_r = pr.width as f64 * m.cell_w * 0.95;
            const AURA_RINGS: usize = 8;
            for i in 0..AURA_RINGS {
                let t = i as f64 / AURA_RINGS as f64; // 0 = outer, 1 = inner
                let rr = max_r * (1.0 - t);
                let glow = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                    NSPoint::new(cxp - rr, cyp - rr),
                    NSSize::new(rr * 2.0, rr * 2.0),
                ));
                ns_color(&RoundColor(base.0, base.1, base.2, 0.05)).setFill();
                glow.fill();
            }
        }
```

(`cell_to_point` returns the cell's lower-left in AppKit Y-up; for a soft symmetric glow the half-cell error is immaterial.)

- [ ] **Step 3: Build + run + observe**

Run: `cargo build --release`
Expected: compiles clean.
Run: `cargo run --release -- companion`. Observe a soft colored glow tracking the pet; the three faint vital ticks are still present (removed in Task 11). Glow color should reflect mood.

- [ ] **Step 4: Commit**

```bash
git add src/companion/app.rs
git commit -m "feat(companion): mood aura glow tracking the pet"
```

---

### Task 10: AppKit — growth ring + rate comet + continuous redraw

**Files:**
- Modify: `src/companion/app.rs`

- [ ] **Step 1: Make the view redraw every tick (so the comet animates while still)**

In `animate_pet` (~lines 393-413), the redraw currently fires only on change. Replace the change-gated return so it always redraws (4 fps; cheap, and the comet/drift are time-driven):

```rust
fn animate_pet() {
    let view = APP_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        let next_frame = state.animation_frame.wrapping_add(1);
        let now = time::OffsetDateTime::now_utc();
        let _ = advance_companion_animation(&mut state.vm, next_frame, now);
        state.animation_frame = next_frame;
        state.scene = derive_round_scene_model(&state.vm, now);
        Some(state.view.clone())
    });
    if let Some(view) = view {
        unsafe { view.setNeedsDisplay(true) };
    }
}
```

- [ ] **Step 2: Draw the growth ring + comet**

In `draw_scene`, after the scene blit (and the aura), and before the halo loop, draw the ring. Add the `use` for the helpers at the top of the file if not present (`use crate::round::hud::{growth_ring_layout, growth_ring_fill_end_deg, comet_phase, comet_position};`). Then:

```rust
    // Growth ring (open-bottom arc) + orbiting rate comet.
    {
        const RING_GAP_DEG: f64 = 70.0;
        let cx = aperture.center_x as f64;
        let cy = aperture.center_y as f64;
        let r = aperture.radius as f64 - 3.0; // inside the rim
        let ring = growth_ring_layout(cx, cy, r, RING_GAP_DEG);
        let frac = if vm.progress.is_max_stage { 1.0 } else { vm.progress.fraction as f64 };
        let fill_end = growth_ring_fill_end_deg(&ring, frac);
        let line_w = (aperture.radius as f64 * 0.012).max(2.0);

        unsafe {
            // Track (dim) — full open arc.
            let track = NSBezierPath::new();
            track.setLineWidth(line_w);
            track.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
                NSPoint::new(cx, cy),
                r,
                ring.track_start_deg,
                ring.track_start_deg + ring.track_sweep_deg,
            );
            ns_color(&RoundColor(0.71, 0.71, 0.78, 0.16)).setStroke();
            track.stroke();

            // Fill (violet) — start → fraction.
            if fill_end > ring.track_start_deg {
                let fill = NSBezierPath::new();
                fill.setLineWidth(line_w);
                fill.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
                    NSPoint::new(cx, cy),
                    r,
                    ring.track_start_deg,
                    fill_end,
                );
                ns_color(&RoundColor(0.61, 0.48, 0.88, 0.85)).setStroke();
                fill.stroke();
            }

            // Rate comet — a small bright dot riding the track.
            let (gx, gy) = comet_position(&ring, comet_phase(animation_frame, vm.progress.rate_per_hour));
            let cr = line_w * 1.6;
            let dot = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
                NSPoint::new(gx - cr, gy - cr),
                NSSize::new(cr * 2.0, cr * 2.0),
            ));
            ns_color(&RoundColor(0.88, 0.82, 1.0, 0.95)).setFill();
            dot.fill();
        }
    }
```

**Binding check:** if `appendBezierPathWithArcWithCenter_radius_startAngle_endAngle` does not resolve under that name in this objc2-app-kit version, use the `_clockwise` variant (`...endAngle_clockwise(center, r, start, end, false)`). If neither is bound, fall back to a beaded arc: loop `t in 0..=N`, angle = `start + sweep * t/N`, draw a small filled oval at `(cx + r*cos, cy + r*sin)` via `bezierPathWithOvalInRect` (the confirmed halo-bead primitive at app.rs:516) — dim for the track, violet up to `fill_end`. Confirm which compiles in Step 3.

- [ ] **Step 3: Build + run + observe**

Run: `cargo build --release`
Expected: compiles clean (adjust the arc method name per the binding check if needed).
Run: `cargo run --release -- companion`. Observe the violet ring around the rim, open at the bottom, filled to the pet's stage progress, with a comet orbiting — and it keeps orbiting even when you're idle.

- [ ] **Step 4: Commit**

```bash
git add src/companion/app.rs
git commit -m "feat(companion): open-bottom growth ring + orbiting rate comet"
```

---

### Task 11: AppKit — replace the HUD (remove vital ticks + evolve bar; stat in the gap)

**Files:**
- Modify: `src/companion/app.rs` (`draw_hud` + remove dead helpers/consts/tests)

- [ ] **Step 1: Rewrite `draw_hud` to draw only the token stat, in the ring gap**

Replace the body of `draw_hud` (app.rs ~920-1065) with the stat-only readout, positioned by `stat_gap_box`:

```rust
#[cfg(target_os = "macos")]
fn draw_hud(
    bounds: NSRect,
    aperture: &RoundAperture,
    vm: &crate::tui::view_model::WatchViewModel,
    font_size: f64,
) {
    let _ = bounds;
    const RING_GAP_DEG: f64 = 70.0;
    let gap = crate::round::hud::stat_gap_box(
        aperture.center_x as f64,
        aperture.center_y as f64,
        aperture.radius as f64 - 3.0,
        RING_GAP_DEG,
    );

    let today = crate::format::format_tokens(vm.today_effective_tokens);
    let rate = crate::format::format_tokens(vm.progress.rate_per_hour);
    let big_color = RoundColor(0.93, 0.93, 0.97, 1.0);
    let sub_color = RoundColor(0.62, 0.63, 0.77, 1.0);

    unsafe {
        // Big "today" number, centered in the gap; shrink to fit the gap chord.
        let mut big_size = font_size * 1.7;
        let mut big = attributed_pet_glyph(&today, big_size, &big_color);
        while big.size().width > gap.max_width && big_size > 6.0 {
            big_size -= 1.0;
            big = attributed_pet_glyph(&today, big_size, &big_color);
        }
        let big_w = big.size().width;
        let big_h = big.size().height;
        // baseline_y is measured DOWN from top; AppKit draws Y-up, so flip.
        let top = bounds.size.height - gap.baseline_y;
        big.drawAtPoint(NSPoint::new(gap.center_x - big_w / 2.0, top));

        // Small "today · {rate}/hr" sub-line just below.
        let sub_text = format!("today · {rate}/hr");
        let sub_size = font_size * 0.9;
        let sub = attributed_pet_glyph(&sub_text, sub_size, &sub_color);
        let sub_w = sub.size().width;
        sub.drawAtPoint(NSPoint::new(gap.center_x - sub_w / 2.0, top - big_h * 0.9));
    }
}
```

- [ ] **Step 2: Delete the now-dead vital-tick + evolve-bar code**

Remove from `src/companion/app.rs`:
- Constants: `HUD_TOKEN_FONT_FRAC`, `HUD_TOKEN_Y_FRAC`, `HUD_TOKEN_ALPHA`, all `HUD_EVOLVE_*`, all `HUD_GAUGE_*`, `HUD_COLOR_FED`, `HUD_COLOR_HAPPY`, `HUD_COLOR_ENERGY`, `HUD_COLOR_TRACK`, `HUD_VITAL_FILL_ALPHA`, `HUD_COLOR_TOKEN`, `HUD_COLOR_TEXT`, `HUD_COLOR_EVOLVE_FILL`, `HUD_EVOLVE_FILL_ALPHA`, `HUD_COLOR_EVOLVE_TRACK` (any the new `draw_hud` no longer references).
- Structs + fns: `GaugeLayout`, `hud_gauge_layouts`, `EvolveBarLayout`, `hud_evolve_bar_layout`.
- Their unit tests in the `#[cfg(test)] mod tests` block (the gauge/evolve layout tests). Keep tests that don't reference removed items (e.g. `cell_to_point_*`).

- [ ] **Step 3: Build, test, lint**

Run: `cargo build --release`
Expected: compiles clean — no unused-item warnings (the local lefthook clippy denies them).
Run: `cargo test`
Expected: PASS (removed tests are gone; remaining suite green; insta snapshots unchanged).
Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 4: Run + observe**

Run: `cargo run --release -- companion`. The three faint ticks and the bottom bar are gone; a single clean "today" number + small rate sub-line sits in the ring's bottom gap, not overlapping the ring.

- [ ] **Step 5: Commit**

```bash
git add src/companion/app.rs
git commit -m "feat(companion): one clean stat in the ring gap; remove vital ticks + evolve bar"
```

---

### Task 12: On-device tuning pass

**Files:**
- Modify: `src/companion/app.rs` (`COMPANION_TARGET_COLS`, `companion_motion()`, `RING_GAP_DEG`, aura/comet/depth constants), `src/round/hud.rs` (`mood_aura_color` palette, `comet_phase` constants)

- [ ] **Step 1: Tune on the real 960×960 screen**

Run `cargo run --release -- companion` on the round display and adjust to taste:
- `COMPANION_TARGET_COLS` (pet/prop size) — and re-confirm the bounded check still holds for the chosen `companion_motion()` values.
- `companion_motion()` — set `drift_x_frac`, `drift_y_frac`, `upward_bias`, `drift_period_secs` for "moves around the tank" without clipping. After choosing values, add/extend a `drift_keeps_pet_in_aperture` assertion test using the production grid dims for the chosen cols so the bound is locked.
- `RING_GAP_DEG`, ring `line_w`, fill/track colors; aura `AURA_RINGS`/`max_r`/alpha and the `mood_aura_color` palette; `comet_phase` `BASELINE_PER_FRAME` / `RATE_NORM`; depth-gradient ring count/alpha.

- [ ] **Step 2: Lock the chosen motion with a bounded test**

If you changed `companion_motion()` away from default, add to `src/round/scene.rs` tests (using the chosen cols and representative metrics):

```rust
#[test]
fn tuned_companion_motion_stays_bounded() {
    // Mirror companion_motion() and the production grid for the chosen target cols.
    let m = CompanionMotion { /* chosen values */ ..CompanionMotion::default() };
    assert!(drift_keeps_pet_in_aperture(&m, /*cols*/ 32, /*rows*/ 16, /*cell_w*/ 30.0, /*cell_h*/ 60.0, /*r*/ 479.0));
}
```

- [ ] **Step 3: Final test + lint + commit**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: all clean.

```bash
git add -A
git commit -m "tune(companion): on-device pass — pet scale, roam, ring, aura, comet"
```

---

## Self-Review

**Spec coverage:**
- Tank free-float + depth gradient → Task 8. ✅
- Bigger pet + props (one lever) → Task 8 (`COMPANION_TARGET_COLS`). ✅
- Pet roams, bounded, parameterized → Tasks 2, 3, 12. ✅
- Growth ring (open-bottom) → Tasks 5, 10. ✅
- Rate comet (idle floor, animation plumbing, continuous redraw) → Tasks 6, 9 (snapshot), 10 (redraw + draw). ✅
- Mood aura (cfg-free color, Sad≠Sleepy, follows pet) → Tasks 1, 9. ✅
- One clean stat clamped in the ring gap → Tasks 7, 11. ✅
- Helpers in cfg-free `src/round/`, painting in `app.rs` → all helper tasks target `src/round/`. ✅
- Shared-surface safety (menubar/preview/goldens unchanged via default motion) → Tasks 2, 4 (Step 6 verifies snapshots don't move). ✅
- Remove vital ticks + evolve bar + their tests (clippy dead-code gate) → Task 11. ✅

**Placeholder scan:** No "TBD"/"add error handling"/"similar to" — each step has real code or an exact command. The Task 10 arc-binding contingency gives concrete code for both the stroked-arc and beaded-fallback paths.

**Type consistency:** `CompanionMotion` (Task 2) is consumed identically in Tasks 3, 4, 12. `CompanionScene { draw_list, pet_rect }` (Task 4) is read as `.draw_list` in all callers and `.pet_rect` in Task 9. `GrowthRing` (Task 5) feeds `comet_position`/`growth_ring_fill_end_deg` (Task 6) and `draw_scene` (Task 10). `mood_aura_color` (Task 1) is called in Task 9. `stat_gap_box`/`StatGap` (Task 7) is used in Task 11. Names match across tasks.
