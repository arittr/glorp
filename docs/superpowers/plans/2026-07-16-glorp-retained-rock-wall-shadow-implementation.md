# Glorp Retained Rock Wall and Dark Shadow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the companion tank read as a dark biome-tinted rock enclosure with a genuinely dark rear pet silhouette instead of a black void with a light projection.

**Architecture:** Lift only the packed tank wall paint, preserve the current room and shadow ABI, and add deterministic wall strata/mineral math to the existing analytic aperture fragment. The retained compositor gets a dark source-over tint while the smooth compositor keeps its existing bounded multiply factor; both remain one renderer-neutral shadow semantic without changing either blend pipeline.

**Tech Stack:** Rust, WGSL, wgpu/Metal native readback, the retained companion scene, the smooth round companion plan, and Preview Lab.

## Global Constraints

- The tank remains dark: night/dusk wall channels generally stay in the low `20` to high `30` sRGB8 range, while dawn/day generally stay in the high `20` to low `50` range.
- Preserve biome hue and day-phase ordering; do not change the shared application background palette.
- Wall texture uses absolute logical tank coordinates, is deterministic across frames, and retains the same logical structure at 1x and 2x.
- Wall strata and mineral grain fade through the curved bed horizon and contribute nothing below the bed transition.
- Ground texture, floor projection, mood aura, status halo, props, gauges, HUD, pet art, depth cues, and draw order remain unchanged.
- Retained uses a dark neutral-violet source-over tint; smooth keeps its existing bounded multiply factor. Neither compositor changes blend mode or pipeline.
- Keep the existing analytic room draw, glyph-mask shadow draw, packed paint fields, scene ABI, checksum/validation model, and resize ownership.
- Do not add a texture asset, render pass, bind group, buffer, uniform, pipeline, resize-owned resource, mesh, camera, setting, or biome configuration.
- Do not add WGSL/source-string assertions. Prove shader behavior with pure logical sampling and native retained readback.
- Implement changed behavior test-first and record the intended RED failure before production edits.

---

### Task 1: Lift the tank wall and make the rear silhouette dark

**Files:**
- Modify: `src/presentation/companion_effects.rs:224-318,365-445`
- Modify: `src/presentation/companion_scene/scene/compiler.rs:1680-1710`
- Modify: `src/round/smooth.rs:470-485`
- Modify: `src/companion/retained/render.rs:9285-9335,12088-12205`
- Verify unchanged: `tests/smooth_companion.rs:1842-1875`

**Interfaces:**
- Consumes: `phase_dim_background_srgb`, `tank_core_srgb`, `AnalyticPaint::ApertureDepth`, `AnalyticPaint::PetShadowTint`, and the two existing compositor blend paths.
- Produces: `TANK_WALL_AMBIENT_LIFT_SRGB: [f32; 3]`, `RETAINED_WALL_SHADOW_TINT_SRGB8: [u8; 3]`, and `SMOOTH_WALL_SHADOW_MULTIPLY_SRGB8: [u8; 3]`.
- Preserves: `RETAINED_WALL_SHADOW_TINT_ALPHA_U8 == 78`, `wall_shadow_depth_cue`, retained premultiplied-alpha blending, and smooth multiply blending.

- [ ] **Step 1: Add the failing wall-palette behavior test**

Add this test to `src/presentation/companion_effects.rs`:

```rust
#[test]
fn tank_wall_palette_stays_dark_readable_and_phase_ordered() {
    let luma = |rgb: [u8; 3]| {
        0.2126 * f32::from(rgb[0])
            + 0.7152 * f32::from(rgb[1])
            + 0.0722 * f32::from(rgb[2])
    };
    let mut day_palettes = Vec::new();
    for biome in [
        "starter",
        "botanical",
        "technical",
        "celestial",
        "artifact",
        "cozy",
    ] {
        let phases = [0.60, 0.80, 0.85, 1.00]
            .map(|scale| tank_background_paint_srgb8(biome, scale));
        for pair in phases.windows(2) {
            assert!(
                luma(pair[0].1) < luma(pair[1].1),
                "{biome} phase ordering: {phases:?}",
            );
        }

        let (night_core, night_rim) = phases[0];
        let (day_core, day_rim) = phases[3];
        assert!(night_rim.into_iter().all(|channel| (17..=32).contains(&channel)));
        assert!(night_core.into_iter().all(|channel| (21..=40).contains(&channel)));
        assert!(day_rim.into_iter().all(|channel| (24..=46).contains(&channel)));
        assert!(day_core.into_iter().all(|channel| (25..=48).contains(&channel)));
        assert!(luma(day_rim) < luma(bed_primary_srgb8(biome)));
        assert!(
            !day_palettes.contains(&(day_core, day_rim)),
            "{biome} keeps a distinct tank palette",
        );
        day_palettes.push((day_core, day_rim));
    }

    assert_eq!(tank_background_paint_srgb8("starter", 1.0), ([26, 29, 42], [27, 30, 36]));
    assert_eq!(tank_background_paint_srgb8("starter", 0.6), ([22, 24, 36], [19, 21, 26]));
}
```

- [ ] **Step 2: Run the wall-palette test and verify RED**

Run:

```bash
cargo test --lib tank_wall_palette_stays_dark_readable_and_phase_ordered
```

Expected: FAIL. The current starter wall returns `([23, 25, 36], [20, 23, 26])` by day and `([18, 20, 30], [12, 14, 15])` at night, so the night rim remains below the readable envelope and the exact lifted outputs do not match.

- [ ] **Step 3: Reverse the native rear-shadow behavior test and verify RED**

In `src/companion/retained/render.rs`, keep the existing controlled room-plus-wall-mask fixture but rename `rear_wall_tint_lifts_dark_pixels_with_depth_bounded_alpha` to `rear_wall_shadow_darkens_rock_wall_with_depth_bounded_alpha`.

Replace the comparison core in `assert_wall_shadow_tint_readback` with:

```rust
let tint_linear = crate::presentation::companion_effects::WALL_SHADOW_SRGB8
    .map(|channel| scene_srgb_to_linear(f32::from(channel) / 255.0));
let strongest = shadowed
    .rgba
    .chunks_exact(4)
    .zip(unshadowed.rgba.chunks_exact(4))
    .filter(|(_, room)| {
        room[3] == 255
            && room
                .iter()
                .take(3)
                .zip(tint_linear)
                .all(|(room, tint)| {
                    scene_srgb_to_linear(f32::from(*room) / 255.0) > tint
                })
    })
    .map(|(shadow, room)| {
        let shadow_linear: [f32; 3] = std::array::from_fn(|channel| {
            scene_srgb_to_linear(f32::from(shadow[channel]) / 255.0)
        });
        let room_linear: [f32; 3] = std::array::from_fn(|channel| {
            scene_srgb_to_linear(f32::from(room[channel]) / 255.0)
        });
        let drop: [f32; 3] =
            std::array::from_fn(|channel| room_linear[channel] - shadow_linear[channel]);
        let score = drop.iter().copied().sum::<f32>();
        (score, drop, room_linear, shadow_linear, shadow, room)
    })
    .max_by(|left, right| left.0.total_cmp(&right.0))
    .expect("the production scene includes opaque wall pixels above the retained tint");

let (score, drop, room_linear, shadow_linear, shadow, room) = strongest;
assert!(
    score > 0.005 && drop.iter().all(|channel| *channel > 0.0),
    "the rear silhouette must darken wall pixels: shadow={shadow:?}, room={room:?}, drop={drop:?}",
);
assert!(
    shadow_linear[2] > shadow_linear[0] && shadow_linear[2] > shadow_linear[1],
    "the dark shadow must retain a restrained violet bias: shadow={shadow:?}",
);

let observed_alpha = drop
    .iter()
    .enumerate()
    .map(|(channel, drop)| {
        drop / (room_linear[channel] - tint_linear[channel]).max(f32::EPSILON)
    })
    .sum::<f32>()
    / 3.0;
assert!(
    observed_alpha >= authored_max_alpha * 0.75
        && observed_alpha <= authored_max_alpha + 0.03,
    "rear shadow alpha escaped its authored bound: observed={observed_alpha}, authored_max={authored_max_alpha}, shadow={shadow:?}, room={room:?}",
);
```

Leave the constant reference as `WALL_SHADOW_SRGB8` for this RED run. Run:

```bash
cargo test --lib --features retained-renderer rear_wall_shadow_darkens_rock_wall_with_depth_bounded_alpha -- --nocapture
```

Expected: FAIL because the current light tint `[118, 114, 142]` is brighter than every controlled rear-wall pixel, so no eligible darkening sample exists.

- [ ] **Step 4: Implement the lifted wall palette and compositor-specific shadow encodings**

In `src/presentation/companion_effects.rs`, add the tank-local lift and apply it only in `tank_background_paint_srgb8`:

```rust
pub(crate) const TANK_DEPTH_TINT_SRGB: [f32; 3] = [0.10, 0.11, 0.20];
pub(crate) const TANK_CORE_TINT_WEIGHT: f32 = 0.42;
pub(crate) const TANK_WALL_AMBIENT_LIFT_SRGB: [f32; 3] = [0.025, 0.028, 0.040];

pub(crate) fn tank_background_paint_srgb8(
    primary_biome: &str,
    phase_scale: f32,
) -> ([u8; 3], [u8; 3]) {
    let dimmed = phase_dim_background_srgb(primary_biome, phase_scale);
    let rim = std::array::from_fn(|channel| {
        (dimmed[channel] + TANK_WALL_AMBIENT_LIFT_SRGB[channel]).clamp(0.0, 1.0)
    });
    (srgb8(tank_core_srgb(rim)), srgb8(rim))
}
```

Replace the ambiguous shared shadow constant with the two encodings:

```rust
pub(crate) const RETAINED_WALL_SHADOW_TINT_SRGB8: [u8; 3] = [10, 12, 18];
pub(crate) const SMOOTH_WALL_SHADOW_MULTIPLY_SRGB8: [u8; 3] = [118, 114, 142];
pub(crate) const RETAINED_WALL_SHADOW_TINT_ALPHA_U8: u8 = 78;
```

In `src/presentation/companion_scene/scene/compiler.rs`, change only the retained analytic paint source:

```rust
AnalyticSemantic::WallShadow => AnalyticPaint::PetShadowTint {
    color_srgb8: crate::presentation::companion_effects::RETAINED_WALL_SHADOW_TINT_SRGB8,
    opacity_u8: crate::presentation::companion_effects::RETAINED_WALL_SHADOW_TINT_ALPHA_U8,
},
```

In `src/round/smooth.rs`, keep the existing multiply behavior but source the multiply-specific constant:

```rust
const WALL_SHADOW_MULTIPLY: crate::pet::palette::Rgb = crate::pet::palette::Rgb {
    r: crate::presentation::companion_effects::SMOOTH_WALL_SHADOW_MULTIPLY_SRGB8[0],
    g: crate::presentation::companion_effects::SMOOTH_WALL_SHADOW_MULTIPLY_SRGB8[1],
    b: crate::presentation::companion_effects::SMOOTH_WALL_SHADOW_MULTIPLY_SRGB8[2],
};
```

Finally, update the RED helper in `render.rs` to reference `RETAINED_WALL_SHADOW_TINT_SRGB8`. Keep `EXPECTED_TINT_ALPHA_U8 == 78` and every depth/opacity assertion.

- [ ] **Step 5: Run Task 1 GREEN checks**

Run:

```bash
cargo test --lib tank_wall_palette_stays_dark_readable_and_phase_ordered
cargo test --lib --features retained-renderer rear_wall_shadow_darkens_rock_wall_with_depth_bounded_alpha
cargo test --test smooth_companion wall_shadow_is_a_multiply_veil_in_the_smooth_plan
cargo test --lib analytic_room_and_floor_paint_use_shared_biome_phase_authority
cargo fmt --check
```

Expected: every command PASS. The retained readback shows a bounded luminance drop, while the smooth integration test keeps every multiply channel between `100` and `230`.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/presentation/companion_effects.rs src/presentation/companion_scene/scene/compiler.rs src/round/smooth.rs src/companion/retained/render.rs
git commit -m "feat(companion): darken the retained wall shadow"
```

---

### Task 2: Add stable logical rock strata and mineral grain

**Files:**
- Modify: `src/presentation/companion_effects.rs:130-223,365-445`
- Modify: `src/companion/retained/scene.wgsl:809-914`
- Modify: `src/companion/retained/render.rs:9800-9980`
- Modify: `src/companion/retained/resources.rs:232,1605-1612`
- Verify unchanged: `src/presentation/companion_scene/scene.rs:967-984`
- Verify unchanged: `src/companion/retained/compiler.rs:2080-2105`

**Interfaces:**
- Consumes: `substrate_hash01`, `substrate_value_noise`, `substrate_mark`, absolute `point_y_down`, and `bed_mix`.
- Produces: `WallTextureSample`, `wall_texture_sample([f32; 2], [f32; 2], f32)`, wall broad-tone/strata/mineral levels, and `SHADER_RESOURCE_VERSION == 5`.
- Preserves: the existing four packed room paint lanes, bed texture constants, single analytic room draw, and all resource counts.

- [ ] **Step 1: Replace the smooth-upper-wall native oracle with a failing rock-wall behavior test**

In `src/companion/retained/render.rs`, remove `retained_bed_upper_roi_has_no_substrate_flecks` and add:

```rust
#[cfg(target_os = "macos")]
#[test]
fn retained_wall_upper_roi_has_logical_rock_texture() {
    let (device, queue) = native_device();
    let [at_1x, repeated_1x] = room_only_offscreen(&device, &queue, 1.0);
    assert_eq!(at_1x.rgba, repeated_1x.rgba, "1x wall texture must be byte-stable");
    let [at_2x, repeated_2x] = room_only_offscreen(&device, &queue, 2.0);
    assert_eq!(at_2x.rgba, repeated_2x.rgba, "2x wall texture must be byte-stable");

    let wall_1x = rgba_roi(&at_1x, [100.0, 120.0, 160.0, 96.0], 1.0);
    let wall_2x = rgba_roi(&at_2x, [100.0, 120.0, 160.0, 96.0], 2.0);
    let (wall_coarse, wall_width) = downsample_rgba(&wall_1x, 160, 4);
    let (wall_2x_coarse, wall_2x_width) = downsample_rgba(&wall_2x, 320, 8);
    assert_eq!(wall_width, wall_2x_width);

    let wall_residuals = local_trend_residuals(&wall_coarse, wall_width);
    let wall_2x_residuals = local_trend_residuals(&wall_2x_coarse, wall_2x_width);
    let variance = wall_residuals
        .iter()
        .map(|residual| residual * residual)
        .sum::<f64>()
        / wall_residuals.len() as f64;
    let correlation = pearson_correlation(&wall_residuals, &wall_2x_residuals);
    let mean_difference = mean_rgb_absolute_difference(&wall_coarse, &wall_2x_coarse);
    assert!(
        (0.20..=4.0).contains(&variance),
        "wall texture is either flat or too noisy: variance={variance}",
    );
    assert!(
        correlation >= 0.80,
        "wall texture lost logical backing-scale coherence: correlation={correlation}, mean_difference={mean_difference}",
    );

    let bed = rgba_roi(&at_1x, [100.0, 285.0, 160.0, 48.0], 1.0);
    let (bed_coarse, bed_width) = downsample_rgba(&bed, 160, 4);
    let bed_variance = local_trend_residual_variance(&bed_coarse, bed_width);
    assert!(variance < bed_variance, "wall must stay quieter than ground");
}
```

In `retained_bed_lower_roi_has_structured_logical_texture`, keep the bed cross-scale assertion but replace its wall-relative check with:

```rust
let structured = local_trend_residual_variance(&lower_coarse, lower_width);
assert!(structured > 0.25, "lower bed lost coherent texture: structured={structured}");
```

- [ ] **Step 2: Run the native wall test and verify RED**

Run:

```bash
cargo test --lib --features retained-renderer retained_wall_upper_roi_has_logical_rock_texture -- --nocapture
```

Expected: FAIL at the variance floor. The current upper wall is intentionally smooth and measures below `0.20` after logical downsampling.

- [ ] **Step 3: Add the pure logical wall-texture mirror and focused coverage**

In `src/presentation/companion_effects.rs`, extract the shared test-only bed mix:

```rust
#[cfg(test)]
fn room_bed_mix(logical_point_y_down: [f32; 2], logical_extent: [f32; 2]) -> f32 {
    let normalized_x = logical_point_y_down[0] / logical_extent[0] - 0.5;
    let horizon_y = logical_extent[1] * (0.76 + 0.04 * normalized_x * normalized_x);
    let bed_feather = (logical_extent[1] * 0.12).max(1.0);
    let bed_t = ((logical_point_y_down[1] - horizon_y) / bed_feather).clamp(0.0, 1.0);
    bed_t * bed_t * (3.0 - 2.0 * bed_t)
}
```

Use `room_bed_mix` inside `bed_texture_sample`, then add:

```rust
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WallTextureSample {
    pub(crate) bed_mix: f32,
    pub(crate) wall_gate: f32,
    pub(crate) broad_tone_levels: f32,
    pub(crate) strata_levels: f32,
    pub(crate) mineral_levels: f32,
}

#[cfg(test)]
pub(crate) fn wall_texture_sample(
    logical_point_y_down: [f32; 2],
    logical_extent: [f32; 2],
    backing_scale: f32,
) -> WallTextureSample {
    let bed_mix = room_bed_mix(logical_point_y_down, logical_extent);
    let wall_gate = 1.0 - texture_smooth_step(0.02, 0.68, bed_mix);
    let rock_point = [logical_point_y_down[0] * 0.55, logical_point_y_down[1]];
    let rock_field = substrate_value_noise(rock_point, 54.0, 0x4A11_5EED);
    let broad_tone_levels = (rock_field - 0.5) * 10.0 * wall_gate;
    let strata_phase =
        (logical_point_y_down[1] + (rock_field - 0.5) * 24.0) / 38.0;
    let strata_fraction = strata_phase - strata_phase.floor();
    let strata_distance = (strata_fraction - 0.5).abs();
    let strata_levels = (1.0 - texture_smooth_step(0.025, 0.10, strata_distance))
        * 5.0
        * wall_gate;
    let mineral_levels = substrate_mark(
        logical_point_y_down,
        24.0,
        1.4,
        0.18,
        0x6D2B_79F5,
    ) * 4.0
        * wall_gate;
    let _ = backing_scale;

    WallTextureSample {
        bed_mix,
        wall_gate,
        broad_tone_levels,
        strata_levels,
        mineral_levels,
    }
}
```

Add these tests:

```rust
#[test]
fn wall_texture_is_invariant_to_backing_scale() {
    let at_1x = wall_texture_sample([144.5, 120.5], [360.0; 2], 1.0);
    let at_2x = wall_texture_sample([144.5, 120.5], [360.0; 2], 2.0);
    assert_eq!(at_1x, at_2x);
    assert_eq!(at_1x.bed_mix, 0.0);
    assert_eq!(at_1x.wall_gate, 1.0);
}

#[test]
fn wall_texture_has_broad_strata_and_sparse_mineral_structure() {
    let samples = (36..=240)
        .step_by(2)
        .flat_map(|y| {
            (36..=324).step_by(2).map(move |x| {
                wall_texture_sample([x as f32 + 0.5, y as f32 + 0.5], [360.0; 2], 1.0)
            })
        })
        .collect::<Vec<_>>();
    assert!(samples.iter().any(|sample| sample.broad_tone_levels.abs() >= 2.0));
    assert!(samples.iter().any(|sample| sample.strata_levels >= 2.5));
    assert!(samples.iter().any(|sample| sample.mineral_levels >= 2.0));
    assert!(samples.iter().all(|sample| {
        (sample.broad_tone_levels - sample.strata_levels + sample.mineral_levels).abs()
            <= 14.0
    }));
}

#[test]
fn wall_texture_fades_out_before_ground_substrate() {
    for y in (320..=356).step_by(4) {
        for x in (48..=312).step_by(12) {
            let sample = wall_texture_sample([x as f32 + 0.5, y as f32 + 0.5], [360.0; 2], 2.0);
            assert_eq!(sample.wall_gate, 0.0, "sample=({x}, {y})");
            assert_eq!(sample.broad_tone_levels, 0.0);
            assert_eq!(sample.strata_levels, 0.0);
            assert_eq!(sample.mineral_levels, 0.0);
        }
    }
}
```

Run:

```bash
cargo test --lib presentation::companion_effects::tests::wall_texture_
```

Expected: all three pure logical tests PASS. The native wall test remains RED until WGSL mirrors the same math.

- [ ] **Step 4: Mirror the wall texture in the existing room fragment**

Add this WGSL helper immediately before `fs_room_aperture` in `src/companion/retained/scene.wgsl`:

```wgsl
fn wall_rock_levels(point_y_down: vec2<f32>, bed_mix: f32) -> vec3<f32> {
    let wall_gate = 1.0 - smoothstep(0.02, 0.68, bed_mix);
    let rock_point = vec2<f32>(point_y_down.x * 0.55, point_y_down.y);
    let rock_field = substrate_value_noise(rock_point, 54.0, 0x4a115eedu);
    let broad_tone_levels = (rock_field - 0.5) * 10.0 * wall_gate;
    let strata_phase =
        (point_y_down.y + (rock_field - 0.5) * 24.0) / 38.0;
    let strata_distance = abs(fract(strata_phase) - 0.5);
    let strata_levels =
        (1.0 - smoothstep(0.025, 0.10, strata_distance)) * 5.0 * wall_gate;
    let mineral_levels = substrate_mark(
        point_y_down,
        24.0,
        1.4,
        0.18,
        0x6d2b79f5u,
    ) * 4.0 * wall_gate;
    return vec3<f32>(broad_tone_levels, strata_levels, mineral_levels);
}
```

In `fs_room_aperture`, keep the existing ground terms but apply wall variation before the bed mix:

```wgsl
let wall_levels = wall_rock_levels(point_y_down, bed_mix);
var room_srgb = clamp(
    linear_to_srgb(mix(core, rim, radial))
        + vec3<f32>((wall_levels.x - wall_levels.y + wall_levels.z) / 255.0),
    vec3<f32>(0.0),
    vec3<f32>(1.0),
);
var room = srgb_to_linear(room_srgb);
room = mix(room, bed, bed_mix * 0.72);
room_srgb = linear_to_srgb(room);
room_srgb = clamp(
    room_srgb + vec3<f32>(broad_tone_levels / 255.0),
    vec3<f32>(0.0),
    vec3<f32>(1.0),
);
room_srgb = mix(
    room_srgb,
    linear_to_srgb(fleck),
    clamp(grain_mix + fleck_mix, 0.0, 0.60),
);
```

Do not add new shader inputs or payload lanes. In `src/companion/retained/resources.rs`, bump only the existing shader cache contract and rename its test:

```rust
pub(super) const SHADER_RESOURCE_VERSION: u32 = 5;

#[test]
fn shader_resource_version_tracks_tank_surface_shader_change() {
    assert_eq!(SHADER_RESOURCE_VERSION, 5);
}
```

- [ ] **Step 5: Run Task 2 GREEN and preservation checks**

Run:

```bash
cargo test --lib presentation::companion_effects::tests::wall_texture_
cargo test --lib --features retained-renderer retained_wall_upper_roi_has_logical_rock_texture
cargo test --lib --features retained-renderer retained_bed_lower_roi_has_structured_logical_texture
cargo test --lib --features retained-renderer rear_wall_shadow_darkens_rock_wall_with_depth_bounded_alpha
cargo test --lib --features retained-renderer shader_resource_version_tracks_tank_surface_shader_change
cargo test --lib --features retained-renderer analytic_packers_preserve_all_eight_closed_roles_exactly
cargo test --lib --features retained-renderer v2_room_and_analytic_families_translate_into_the_existing_two_dynamic_buffers
cargo test --lib --features retained-renderer gpu_resource_accounting_is_exact_and_not_a_live_global_metric
cargo fmt --check
```

Expected: all PASS. Native 1x/2x wall ROIs are byte-stable, normalized texture correlation is at least `0.80`, wall residual variance is `0.20..=4.0`, and ground remains more structured than the wall.

- [ ] **Step 6: Commit Task 2**

```bash
git add src/presentation/companion_effects.rs src/companion/retained/scene.wgsl src/companion/retained/render.rs src/companion/retained/resources.rs
git commit -m "feat(companion): texture the retained rock wall"
```

---

### Task 3: Verify the combined renderer and launch the optimized companion

**Files:**
- Verify: all Task 1-2 files
- Generate ignored preview bundle: `target/glorp-preview-rock-wall/`
- Build and launch ignored app bundle: `target/macos/Glorp.app`

**Interfaces:**
- Consumes: the two committed visual deliverables.
- Produces: one clean combined verification run, deterministic round artifacts, and an optimized retained companion for Drew's display/resize acceptance.

- [ ] **Step 1: Run the combined automated verification once**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib --all-features
cargo test --features retained-renderer --test retained_scene
cargo test --test smooth_companion
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
```

Expected: every command exits `0`. Treat any new warning or failure as evidence and stop before launching.

- [ ] **Step 2: Generate and inspect the deterministic round preview**

Run:

```bash
cargo run -- dev-preview --scenario round --out target/glorp-preview-rock-wall
test -s target/glorp-preview-rock-wall/manifest.json
test -s target/glorp-preview-rock-wall/index.html
jq -e '.scenarios | any(.id == "round-retained-composition-full-cast")' \
  target/glorp-preview-rock-wall/manifest.json
git status --short
```

Expected: the preview bundle is nonempty, contains the full-cast retained fixture, and does not modify tracked files.

- [ ] **Step 3: Build and launch the fresh optimized companion**

Run:

```bash
cargo xtask companion fresh
```

Expected: the release app builds, any existing Glorp companion exits, and `target/macos/Glorp.app` opens. Do not automate fullscreen, resizing, or display movement.

- [ ] **Step 4: Hand off visual acceptance to Drew**

Ask Drew to verify on the normal and Napster displays:

1. Empty tank regions read as dark rock rather than black void.
2. Strata and mineral grain remain visible but quieter than the pet, HUD, and ground.
3. The rear pet silhouette is darker than the wall, translucent, and not a flat black sticker.
4. Day/night retains its ordering and night remains readable without becoming bright.
5. Resize, fullscreen, display movement, and animation do not shift or shimmer the wall texture.

If tuning is needed, change only `TANK_WALL_AMBIENT_LIFT_SRGB`, the three approved wall texture amplitudes/scales, or `RETAINED_WALL_SHADOW_TINT_SRGB8`; rerun the focused Task 1-2 tests plus Step 1 and commit the tuning separately.
