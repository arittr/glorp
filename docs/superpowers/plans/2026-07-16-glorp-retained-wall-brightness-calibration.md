# Glorp Retained Wall Brightness Calibration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lift only the retained tank wall enough to remain readable on the real normal display while preserving the approved ground, rock texture, and dark rear shadow.

**Architecture:** Change the existing tank-local ambient lift and the direct palette behavior oracle that defines its output. Keep the analytic room shader, rock math, ground blend, compositor-specific shadow encodings, scene ABI, draw topology, and GPU resources unchanged.

**Tech Stack:** Rust, wgpu/Metal native readback, retained and smooth companion integration tests, Preview Lab, and the macOS companion xtask.

## Global Constraints

- Set `TANK_WALL_AMBIENT_LIFT_SRGB` to exactly `[0.050, 0.055, 0.070]`.
- Change no production file other than `src/presentation/companion_effects.rs`.
- Keep wall rock field, strata, mineral grain, horizon gate, ground mix, and every texture amplitude and scale unchanged.
- Keep `RETAINED_WALL_SHADOW_TINT_SRGB8`, retained shadow opacity/source-over behavior, and `SMOOTH_WALL_SHADOW_MULTIPLY_SRGB8` unchanged.
- Keep the shared application background and all ground, prop, pet, gauge, and HUD paint unchanged.
- Preserve biome identity and strict phase ordering.
- Night starter paint must become core/rim `([25, 28, 41], [25, 28, 33])`; day starter paint must become `([30, 33, 47], [33, 37, 43])`.
- Across supported biomes, night rim channels remain `23..=39`, night core channels `24..=44`, day rim channels `31..=54`, and day core channels `28..=52`.
- Do not add or change a shader, texture asset, render pass, bind group, buffer, uniform, pipeline, resource, mesh, camera, setting, payload field, ABI field, or source-string test.

---

### Task 1: Calibrate the tank-local wall palette

**Files:**
- Modify: `src/presentation/companion_effects.rs:267-302,410-465`

**Interfaces:**
- Consumes: `phase_dim_background_srgb`, `tank_core_srgb`, and `tank_background_paint_srgb8`.
- Produces: `TANK_WALL_AMBIENT_LIFT_SRGB == [0.050, 0.055, 0.070]` and the approved biome/phase wall palette.
- Preserves: every downstream packed paint field, rock/shadow semantic, renderer blend path, scene contract, and GPU resource.

- [ ] **Step 1: Change the palette behavior oracle first**

In `tank_wall_palette_stays_dark_readable_and_phase_ordered`, replace only the four channel envelopes and two starter paint expectations:

```rust
assert!(night_rim
    .into_iter()
    .all(|channel| (23..=39).contains(&channel)));
assert!(night_core
    .into_iter()
    .all(|channel| (24..=44).contains(&channel)));
assert!(day_rim
    .into_iter()
    .all(|channel| (31..=54).contains(&channel)));
assert!(day_core
    .into_iter()
    .all(|channel| (28..=52).contains(&channel)));

assert_eq!(
    tank_background_paint_srgb8("starter", 1.0),
    ([30, 33, 47], [33, 37, 43])
);
assert_eq!(
    tank_background_paint_srgb8("starter", 0.6),
    ([25, 28, 41], [25, 28, 33])
);
```

Keep the luma phase-ordering, day-below-ground, and biome-distinctness assertions unchanged.

- [ ] **Step 2: Run the palette oracle and verify RED**

Run:

```bash
cargo test --lib tank_wall_palette_stays_dark_readable_and_phase_ordered
```

Expected: FAIL because the production lift is still `[0.025, 0.028, 0.040]`; the old starter outputs and lower envelopes cannot satisfy the approved real-display calibration.

- [ ] **Step 3: Implement the wall-only calibration**

Change only this constant in `src/presentation/companion_effects.rs`:

```rust
pub(crate) const TANK_WALL_AMBIENT_LIFT_SRGB: [f32; 3] = [0.050, 0.055, 0.070];
```

Do not change `tank_background_paint_srgb8`, any biome base color, depth tint, rock texture constant, bed palette, or shadow constant.

- [ ] **Step 4: Run focused GREEN and preservation checks**

Run:

```bash
cargo test --lib tank_wall_palette_stays_dark_readable_and_phase_ordered
cargo test --lib presentation::companion_effects::tests::wall_texture_
cargo test --lib --features retained-renderer retained_wall_upper_roi_has_logical_rock_texture
cargo test --lib --features retained-renderer retained_bed_lower_roi_has_structured_logical_texture
cargo test --lib --features retained-renderer rear_wall_shadow_darkens_rock_wall_with_depth_bounded_alpha
cargo test --test smooth_companion wall_shadow_is_a_multiply_veil_in_the_smooth_plan
cargo test --lib --all-features companion::retained::render::tests::native_synthetic_offscreen_renderer_captures_pixels_and_reuses_keyed_resources -- --exact
cargo test --lib --all-features companion::retained::render::tests::native_production_scene_renders_complete_fuzz_s3_inventory_with_redacted_hud -- --exact
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all commands exit `0`. The native wall remains deterministic, scale-coherent, and quieter than the bed; the shadow remains dark and bounded; Smooth keeps its multiply veil; both behavioral integration probes stay inside their semantic envelopes.

- [ ] **Step 5: Inspect and commit the calibration**

Run:

```bash
git diff --check
git diff -- src/presentation/companion_effects.rs
git status --short
git add src/presentation/companion_effects.rs
git commit -m "fix(companion): lift the retained rear wall"
```

Expected: the commit contains only the constant change and its direct palette behavior expectations.

---

### Task 2: Verify, preview, and relaunch the calibrated companion

**Files:**
- Verify: `src/presentation/companion_effects.rs`
- Generate ignored preview bundle: `target/glorp-preview-rock-wall/`
- Build and launch ignored app bundle: `target/macos/Glorp.app`

**Interfaces:**
- Consumes: the committed Task 1 palette calibration.
- Produces: fresh integration evidence, a replaced deterministic round preview, and an optimized retained companion for real-display acceptance.

- [ ] **Step 1: Run the changed-scope integration gate once**

Run:

```bash
cargo test --features retained-renderer --test retained_scene
cargo test --test smooth_companion
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
```

Expected: retained `12/12`, Smooth `41/41`, round `7/7`, and Preview Lab `79/79` pass. Known default-feature cfg-gated warnings may remain; strict all-feature Clippy in Task 1 must be warning-free.

- [ ] **Step 2: Replace and validate the deterministic round preview**

Run:

```bash
cargo run -- dev-preview --scenario round --out target/glorp-preview-rock-wall
test -s target/glorp-preview-rock-wall/manifest.json
test -s target/glorp-preview-rock-wall/index.html
jq -e '.scenarios | any(.id == "round-retained-composition-full-cast")' \
  target/glorp-preview-rock-wall/manifest.json
git status --short
```

Expected: the preview bundle is nonempty, contains the full-cast retained fixture, and creates no tracked change.

- [ ] **Step 3: Build and launch the optimized retained companion**

Run:

```bash
cargo xtask companion fresh
```

Expected: the release app builds, the prior companion exits, and `target/macos/Glorp.app` opens through the existing Apple Silicon Auto-to-Retained direct route. Do not automate fullscreen, resizing, or display movement.

- [ ] **Step 4: Hand off real-display acceptance**

Ask Drew to confirm:

1. The rear wall is visibly lighter than the previous build but still dark slate.
2. Ground appearance is unchanged.
3. Rock strata/mineral texture remains quieter than the ground and foreground.
4. The rear silhouette remains darker than the wall and translucent.

