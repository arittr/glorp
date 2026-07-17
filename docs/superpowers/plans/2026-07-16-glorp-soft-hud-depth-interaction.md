# Glorp Soft HUD Depth Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hard pet/statistics plane swap with an overlap-only soft reveal through a thin emissive HUD volume, without changing traversal, text, gauges, privacy, or flat renderers.

**Architecture:** Extend the shared depth composition with a deterministic smoothstep interaction result for effective pet Z `[+0.64, +0.72]`. Smooth/AppKit and retained both keep their existing geometric ordering, draw a faint statistics echo at the rear of that band, and repaint exactly the pet body, performance particles, and mood aura through a private primary-glyph coverage mask. The mask stays renderer-private and the retained path keeps one sealed HUD marker and fixed-capacity records.

**Tech Stack:** Rust, AppKit/objc2, wgpu, WGSL, sealed fixed-capacity HUD buffers, Rust unit/integration/native Metal tests, macOS companion xtask.

## Global Constraints

- Statistics remain at Z `+0.72`; the rear emissive echo is at Z `+0.64`.
- The one-sided interaction band is exactly effective pet Z `[+0.64, +0.72]` with smoothstep easing.
- At or below `+0.64`, statistics fully cover overlapping pet ink; at `+0.72`, overlap matches the fully-front result; above `+0.72`, existing front order applies.
- Only pet body, particles/performance cue, and mood aura enter the reveal; wall shadow and floor projection never do.
- The reveal changes only the intersection of private primary-glyph coverage and pet-front coverage; statistics never fade globally.
- Statistics text, formatting, layout, primary color, plane, sealed-value privacy, fixed record count, and single semantic HUD marker remain unchanged.
- Private coverage contents are cleared every HUD pass and are never serialized, checksummed, logged, diagnosed, captured, or read back through production APIs.
- The rear echo uses the existing HUD hue at alpha `0.12`, offset `[+0.60, -0.60]` points in Y-up coordinates, and never enters the primary coverage mask.
- Gauge planes, status/trouble chrome, dimming, pet motion, scale, and endpoint geometry remain unchanged.
- Pixel and Classic retain their current flat scene/chrome sequence.
- No preview/capture acceptance gate, multiview renderer, head tracking, quilt generation, stereo synthesis, or display calibration is added.
- No Linear issue is created or updated for Glorp.

---

### Task 1: Shared Statistics Interaction Contract

**Files:**
- Modify: `src/round/depth.rs`
- Test: `src/round/depth.rs`

**Interfaces:**
- Produces: `COMPANION_STATISTICS_INTERACTION_START_Z: f32 = 0.64`
- Produces: `COMPANION_STATISTICS_ECHO_Z: f32 = 0.64`
- Produces: `StatisticsDepthInteraction { start_z, plane_z, reveal_mix }`
- Produces: `StatisticsDepthInteraction::resolve(pet_effective_z: f32) -> Result<Self, CompanionDepthCompositionError>`
- Produces: `CompanionDepthComposition::statistics_interaction: StatisticsDepthInteraction`
- Consumes: lifecycle-adjusted `SmoothDepthSample::effective_z`

- [ ] **Step 1: Add failing boundary and determinism tests**

Add these tests beside the existing composition tests in `src/round/depth.rs`:

```rust
#[test]
fn statistics_interaction_uses_the_exact_one_sided_band() {
    for (depth, expected) in [
        (COMPANION_STATISTICS_INTERACTION_START_Z, 0.0),
        (0.68, 0.5),
        (COMPANION_STATISTICS_Z, 1.0),
        (f32::from_bits(COMPANION_STATISTICS_Z.to_bits() + 1), 1.0),
    ] {
        let interaction = StatisticsDepthInteraction::resolve(depth).unwrap();
        assert_eq!(interaction.start_z, 0.64);
        assert_eq!(interaction.plane_z, 0.72);
        assert!((interaction.reveal_mix - expected).abs() <= f32::EPSILON * 8.0);
    }
}

#[test]
fn statistics_interaction_is_reversible_and_uses_effective_depth() {
    let approaching = StatisticsDepthInteraction::resolve(0.68).unwrap();
    let retreating = StatisticsDepthInteraction::resolve(0.68).unwrap();
    assert_eq!(approaching, retreating);

    let asleep_sample = resolve_smooth_depth(1.0, 0.68).unwrap();
    let asleep = CompanionDepthComposition::resolve(asleep_sample.effective_z).unwrap();
    assert!((asleep.statistics_interaction.reveal_mix - 0.5).abs() <= f32::EPSILON * 8.0);
}

#[test]
fn statistics_interaction_rejects_non_finite_or_invalid_plane_order() {
    assert_eq!(
        StatisticsDepthInteraction::resolve(f32::NAN),
        Err(CompanionDepthCompositionError::InvalidEffectiveDepth)
    );
    assert!(COMPANION_PET_MIN_Z < COMPANION_STATISTICS_INTERACTION_START_Z);
    assert!(COMPANION_STATISTICS_INTERACTION_START_Z < COMPANION_STATISTICS_Z);
    assert!(COMPANION_STATISTICS_Z < COMPANION_PET_MAX_Z);
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
cargo test --lib round::depth::tests::statistics_interaction_uses_the_exact_one_sided_band
```

Expected: FAIL because `StatisticsDepthInteraction` and `COMPANION_STATISTICS_INTERACTION_START_Z` do not exist.

- [ ] **Step 3: Implement the pure shared contract**

Add the constants, value type, and resolver in `src/round/depth.rs`, and include the resolved value in `CompanionDepthComposition`:

```rust
pub const COMPANION_PET_MIN_Z: f32 = -1.0;
pub const COMPANION_STATISTICS_INTERACTION_START_Z: f32 = 0.64;
pub const COMPANION_STATISTICS_ECHO_Z: f32 = COMPANION_STATISTICS_INTERACTION_START_Z;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct StatisticsDepthInteraction {
    pub start_z: f32,
    pub plane_z: f32,
    pub reveal_mix: f32,
}

impl StatisticsDepthInteraction {
    pub fn resolve(
        pet_effective_z: f32,
    ) -> Result<Self, CompanionDepthCompositionError> {
        if !pet_effective_z.is_finite()
            || !(COMPANION_PET_MIN_Z..=COMPANION_PET_MAX_Z).contains(&pet_effective_z)
        {
            return Err(CompanionDepthCompositionError::InvalidEffectiveDepth);
        }
        if !(COMPANION_PET_MIN_Z < COMPANION_STATISTICS_INTERACTION_START_Z
            && COMPANION_STATISTICS_INTERACTION_START_Z < COMPANION_STATISTICS_Z
            && COMPANION_STATISTICS_Z < COMPANION_PET_MAX_Z)
        {
            return Err(CompanionDepthCompositionError::InvalidPlaneOrder);
        }
        let linear = ((pet_effective_z - COMPANION_STATISTICS_INTERACTION_START_Z)
            / (COMPANION_STATISTICS_Z - COMPANION_STATISTICS_INTERACTION_START_Z))
            .clamp(0.0, 1.0);
        let reveal_mix = linear * linear * (3.0 - 2.0 * linear);
        if !reveal_mix.is_finite() || !(0.0..=1.0).contains(&reveal_mix) {
            return Err(CompanionDepthCompositionError::InvalidEffectiveDepth);
        }
        Ok(Self {
            start_z: COMPANION_STATISTICS_INTERACTION_START_Z,
            plane_z: COMPANION_STATISTICS_Z,
            reveal_mix,
        })
    }
}
```

Change `CompanionDepthComposition::resolve` to call `StatisticsDepthInteraction::resolve(pet_effective_z)?`, store it as `statistics_interaction`, and keep the current strict `pet_effective_z > COMPANION_STATISTICS_Z` front-order decision unchanged.

- [ ] **Step 4: Run all shared depth tests**

Run:

```bash
cargo test --lib round::depth::tests
```

Expected: PASS, including exact `0.0`, `0.5`, and `1.0` reveal values and the existing strict plane-order tests.

- [ ] **Step 5: Commit the shared contract**

```bash
git add src/round/depth.rs
git commit -m "feat(companion): define soft statistics interaction band"
```

---

### Task 2: Smooth/AppKit Private HUD Volume and Overlap Composite

**Files:**
- Modify: `src/companion/app.rs`
- Test: `src/companion/app.rs`
- Test: `tests/companion_draw_boundary.rs`

**Interfaces:**
- Consumes: `CompanionDepthComposition::statistics_interaction`
- Produces: `PreparedAppKitHudVolume` with private primary coverage and prelaid-out primary/echo runs
- Produces: `SmoothAppKitPaintStep::{StatisticsEcho, StatisticsPrimary, StatisticsInteraction}`
- Produces: `paint_smooth_statistics_interaction(..., reveal_mix: f32)`
- Produces: `render_masked_pet_front_image(...) -> Option<Retained<NSImage>>`
- Produces: `draw_appkit_image(image: &NSImage, bounds: NSRect, fraction: f64)`
- Preserves: `PreparedSmoothDepthPasses::pet_front` as exactly body plus performance cue; mood aura remains the explicit first pet-front paint step

- [ ] **Step 1: Add failing schedule, privacy, and crossing-group tests**

Add behavior tests in `src/companion/app.rs`:

```rust
#[test]
fn appkit_schedule_moves_the_pet_between_echo_and_primary_inside_the_band() {
    fn assert_before(
        schedule: &[SmoothAppKitPaintStep],
        left: SmoothAppKitPaintStep,
        right: SmoothAppKitPaintStep,
    ) {
        let left_index = schedule.iter().position(|step| *step == left).unwrap();
        let right_index = schedule.iter().position(|step| *step == right).unwrap();
        assert!(left_index < right_index, "{left:?} must precede {right:?}");
    }

    let behind = smooth_appkit_paint_schedule(CompanionDepthComposition::resolve(0.63).unwrap());
    assert_before(&behind, SmoothAppKitPaintStep::PetFront, SmoothAppKitPaintStep::StatisticsEcho);
    assert_before(&behind, SmoothAppKitPaintStep::StatisticsEcho, SmoothAppKitPaintStep::StatisticsPrimary);

    let interacting = smooth_appkit_paint_schedule(CompanionDepthComposition::resolve(0.68).unwrap());
    assert_before(&interacting, SmoothAppKitPaintStep::StatisticsEcho, SmoothAppKitPaintStep::PetFront);
    assert_before(&interacting, SmoothAppKitPaintStep::PetFront, SmoothAppKitPaintStep::StatisticsPrimary);
    assert_before(&interacting, SmoothAppKitPaintStep::StatisticsPrimary, SmoothAppKitPaintStep::StatisticsInteraction);

    let front = smooth_appkit_paint_schedule(CompanionDepthComposition::resolve(0.73).unwrap());
    assert_before(&front, SmoothAppKitPaintStep::StatisticsPrimary, SmoothAppKitPaintStep::PetFront);
}

#[test]
fn appkit_statistics_interaction_group_excludes_receiving_surface_shadows() {
    let plan = smooth_depth_fixture();
    let passes = prepare_smooth_depth_passes(&plan, &smooth_layer_draw_order(&plan));
    let roles = pass_roles(&plan, &passes.pet_front);
    assert_eq!(roles, [SmoothLayerRole::PetBody, SmoothLayerRole::PerformanceCue]);
    assert!(!roles.contains(&SmoothLayerRole::WallShadow));
    assert!(!roles.contains(&SmoothLayerRole::FloorProjection));
}

#[test]
fn prepared_appkit_hud_volume_debug_never_exposes_live_text() {
    let prepared = prepared_appkit_hud_volume_fixture("981.7M", "49% yday", "349.4k/10m");
    let debug = format!("{prepared:?}");
    assert_eq!(debug, "PreparedAppKitHudVolume(<private>)");
    assert!(!debug.contains("981.7M"));
}
```

In `tests/companion_draw_boundary.rs`, add a public-contract test that constructs Smooth frames at `0.64`, `0.68`, `0.72`, and the next representable value above `0.72`, then asserts the prepared composition carries reveal `0.0`, `0.5`, `1.0`, `1.0` without scanning source text.

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
cargo test --lib companion::app::tests::appkit_schedule_moves_the_pet_between_echo_and_primary_inside_the_band
cargo test --test companion_draw_boundary smooth_statistics_interaction_uses_prepared_effective_depth
```

Expected: FAIL because the three statistics paint steps and private prepared HUD volume do not exist.

- [ ] **Step 3: Prepare the private HUD runs and primary coverage mask outside the paint callback**

In `src/companion/app.rs`, add exact visual constants and private prepared types:

```rust
const APPKIT_HUD_ECHO_OFFSET_Y_UP: [f64; 2] = [0.60, -0.60];
const APPKIT_HUD_ECHO_ALPHA: f32 = 0.12;

#[derive(Clone)]
struct PreparedAppKitHudLine {
    primary: Retained<NSAttributedString>,
    echo: Retained<NSAttributedString>,
    origin: NSPoint,
}

#[derive(Clone)]
struct PreparedAppKitHudVolume {
    lines: [PreparedAppKitHudLine; 3],
    primary_coverage: Retained<NSImage>,
}

impl std::fmt::Debug for PreparedAppKitHudVolume {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PreparedAppKitHudVolume(<private>)")
    }
}
```

Refactor the current `draw_hud` layout body into:

```rust
fn prepare_appkit_hud_volume(
    bounds: NSRect,
    aperture: &RoundAperture,
    hud_text: &CompanionHudText,
    font_size: f64,
) -> Option<PreparedAppKitHudVolume>;

fn draw_prepared_hud_lines(
    prepared: &PreparedAppKitHudVolume,
    echo: bool,
);
```

`prepare_appkit_hud_volume` must use the existing `prepare_hud_layout`, fonts, primary colors, and string formatting. Build echo attributed strings from the same text and hue with alpha multiplied by `0.12`; draw primary runs as opaque white into an off-screen alpha-capable `NSBitmapImageRep`, attach that representation to `primary_coverage`, and restore the prior `NSGraphicsContext` before returning. Add `hud_volume: PreparedAppKitHudVolume` to `PreparedCompanionFrame`; if mask allocation or context creation fails, fail frame preparation rather than falling back to a global text fade.

- [ ] **Step 4: Replace the two-step crossing schedule with the prepared four-step schedule**

Use a fixed 11-step schedule so the draw callback does not allocate or sort:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothAppKitPaintStep {
    WorldBeforeStatistics,
    StatisticsEcho,
    PetFront,
    StatisticsPrimary,
    StatisticsInteraction,
    Foreground,
    Gauge(CompanionGaugeLane),
    StatusTrouble,
    Dim,
}

pub fn smooth_appkit_paint_schedule(
    composition: CompanionDepthComposition,
) -> [SmoothAppKitPaintStep; 11] {
    let crossing = if composition.pet_effective_z
        <= composition.statistics_interaction.start_z
    {
        [
            SmoothAppKitPaintStep::PetFront,
            SmoothAppKitPaintStep::StatisticsEcho,
            SmoothAppKitPaintStep::StatisticsPrimary,
            SmoothAppKitPaintStep::StatisticsInteraction,
        ]
    } else if composition.pet_effective_z <= composition.statistics_z {
        [
            SmoothAppKitPaintStep::StatisticsEcho,
            SmoothAppKitPaintStep::PetFront,
            SmoothAppKitPaintStep::StatisticsPrimary,
            SmoothAppKitPaintStep::StatisticsInteraction,
        ]
    } else {
        [
            SmoothAppKitPaintStep::StatisticsEcho,
            SmoothAppKitPaintStep::StatisticsPrimary,
            SmoothAppKitPaintStep::PetFront,
            SmoothAppKitPaintStep::StatisticsInteraction,
        ]
    };
    [
        SmoothAppKitPaintStep::WorldBeforeStatistics,
        crossing[0],
        crossing[1],
        crossing[2],
        crossing[3],
        SmoothAppKitPaintStep::Foreground,
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Pace),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Daily),
        SmoothAppKitPaintStep::Gauge(CompanionGaugeLane::Xp),
        SmoothAppKitPaintStep::StatusTrouble,
        SmoothAppKitPaintStep::Dim,
    ]
}
```

`StatisticsEcho` draws the prepared echo lines with the fixed Y-up offset; `StatisticsPrimary` draws the prepared primary lines at their unchanged origins. Pixel and Classic continue calling their existing flat HUD function and never construct or consume the interaction overlay.

- [ ] **Step 5: Composite only masked pet-front ink after the primary statistics draw**

Add a helper that returns immediately unless the pet is inside the band and `reveal_mix > 0.0`:

```rust
fn paint_smooth_statistics_interaction(
    bounds: NSRect,
    frame: &PreparedCompanionFrame,
    plan: &SmoothCompanionScenePlan,
    passes: &PreparedSmoothDepthPasses,
    metrics: &CompanionGridMetrics,
    aperture: &RoundAperture,
    reveal_mix: f32,
) {
    if !(0.0..=1.0).contains(&reveal_mix) || reveal_mix == 0.0 {
        return;
    }
    let Some(overlay) = render_masked_pet_front_image(
        frame,
        plan,
        passes,
        metrics,
        aperture,
        &frame.hud_volume.primary_coverage,
    ) else {
        return;
    };
    draw_appkit_image(&overlay, bounds, f64::from(reveal_mix));
}
```

Implement `render_masked_pet_front_image` with the same `NSBitmapImageRep`/`NSGraphicsContext` save-restore pattern already used by `render_prepared_frame_to_rgba`: paint only `paint_smooth_pet_front`, draw the prepared primary mask into that off-screen context with `NSCompositingOperation::DestinationIn`, restore the previous context, and return the masked image. Implement `draw_appkit_image` with source-over and the supplied fraction. The overlay source must not call `world_before_statistics`, `foreground`, wall shadow, floor projection, gauges, status/trouble, or dimming. `DestinationIn` makes all pixels outside primary glyph coverage transparent before the overlay is returned to the main context.

- [ ] **Step 6: Run Smooth/AppKit tests**

Run:

```bash
cargo test --lib companion::app::tests
cargo test --test companion_draw_boundary
```

Expected: PASS. The schedule proves the rear echo is crossed at `+0.64`, the primary is crossed at `+0.72`, and only the named pet-front group is eligible for the overlap repaint.

- [ ] **Step 7: Commit the Smooth/AppKit implementation**

```bash
git add src/companion/app.rs tests/companion_draw_boundary.rs
git commit -m "feat(companion): soften AppKit statistics crossings"
```

---

### Task 3: Retained Private Coverage Target and Masked Pet Overlay

**Files:**
- Modify: `src/companion/retained/hud.rs`
- Modify: `src/companion/retained/render.rs`
- Modify: `src/companion/retained/scene.wgsl`
- Modify: `src/companion/retained/host.rs`
- Modify: `src/companion/retained.rs`
- Test: `src/companion/retained/hud.rs`
- Test: `src/companion/retained/render.rs`

**Interfaces:**
- Consumes: `StatisticsDepthInteraction` prepared from the active snapshot's lifecycle-adjusted effective depth
- Produces: private `HudInteractionGpuValue { reveal_mix, enabled, padding }`
- Produces: full-frame `R8Unorm` primary statistics coverage target owned by `SceneTargets`
- Produces: fixed `HudInteractionDrawPlan` containing exactly pet body, pet particles, and mood aura draws
- Produces: three masked pipelines preserving source-over body, additive particles, and source-over aura blending
- Preserves: one sealed HUD marker, 26 fixed HUD glyph records, 832 record bytes, redacted capture separation, scene inventory, semantic IDs, and scene checksums

- [ ] **Step 1: Add failing private-resource and draw-plan tests**

Add tests in `src/companion/retained/hud.rs` and `render.rs`:

```rust
#[test]
fn hud_interaction_state_is_fixed_private_and_redacted() {
    assert_eq!(std::mem::size_of::<HudInteractionGpuValue>(), 16);
    assert_eq!(HUD_GPU_DRAW_INSTANCES, 26);
    assert_eq!(HUD_GPU_BUFFER_BYTES, 832);
    let prepared = sensitive_fixture_with_depth(0.68);
    assert_eq!(prepared.statistics_interaction().reveal_mix, 0.5);
    assert_eq!(format!("{prepared:?}"), "SensitivePreparedHudFrame(<private>)");
}

#[test]
fn retained_hud_interaction_plan_is_exact_and_excludes_shadows() {
    let fixture = prepared_scene_fixture();
    let plan = prepare_hud_interaction_draw_plan(&fixture.draw_plan, &fixture.primitives).unwrap();
    assert_eq!(plan.sources(), [
        HudInteractionSource::PetBody,
        HudInteractionSource::PetParticles,
        HudInteractionSource::MoodAura,
    ]);
    assert!(!plan.contains(PrimitiveSource::Instances(InstanceSource::WallShadowGlyphMask)));
    assert!(!plan.contains(PrimitiveSource::Instances(InstanceSource::FloorShadowGlyphMask)));
}

#[test]
fn statistics_coverage_target_is_private_fixed_format() {
    assert_eq!(SceneTextureContract::STATISTICS_COVERAGE, wgpu::TextureFormat::R8Unorm);
    assert!(SceneTargetTextureUsages::STATISTICS_COVERAGE
        .contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
    assert!(SceneTargetTextureUsages::STATISTICS_COVERAGE
        .contains(wgpu::TextureUsages::TEXTURE_BINDING));
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```bash
cargo test --features retained-renderer --lib companion::retained::hud::tests::hud_interaction_state_is_fixed_private_and_redacted
cargo test --features retained-renderer --lib companion::retained::render::tests::retained_hud_interaction_plan_is_exact_and_excludes_shadows
```

Expected: FAIL because the private interaction state, coverage target, and exact overlay plan do not exist.

- [ ] **Step 3: Carry the shared interaction through sealed HUD preparation atomically**

Add the interaction to `HudPreparationGeometry`, compute it from the same active source snapshot used to build the scene, and store it in both nominal prepared HUD projections. Add a separate fixed private GPU record rather than changing the 32-byte glyph ABI:

```rust
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HudInteractionGpuValue {
    reveal_mix: f32,
    enabled: u32,
    _padding: [u32; 2],
}

impl HudInteractionGpuValue {
    fn from_composition(composition: CompanionDepthComposition) -> Self {
        Self {
            reveal_mix: composition.statistics_interaction.reveal_mix,
            enabled: u32::from(
                composition.pet_effective_z
                    > composition.statistics_interaction.start_z
                    && composition.pet_effective_z <= composition.statistics_z,
            ),
            _padding: [0; 2],
        }
    }
}
```

Extend the private HUD bind-group layout with binding `1` for this 16-byte read-only storage record. `GpuHudResources::encode_sensitive` and `encode_redacted_capture` must stage the fixed glyph buffer and interaction record on the same caller-owned encoder before the HUD render pass. `HudPreparationGeometry` and all production constructors in `host.rs`/`retained.rs` must call `CompanionDepthComposition::resolve` using:

```rust
crate::presentation::companion_effects::effective_depth(
    snapshot.frame.pet_depth,
    crate::presentation::companion_effects::depth_lifecycle_scale(
        snapshot.frame.asleep,
        snapshot.frame.calm,
    ),
)
```

Test fixtures use `CompanionDepthComposition::resolve(0.0).unwrap()`. Do not serialize the interaction state or add it to scene checksums, artifacts, logs, or public diagnostics.

- [ ] **Step 4: Add the private primary-coverage target and HUD MRT output**

Add to `SceneTargets`:

```rust
pub(super) statistics_coverage_texture: wgpu::Texture,
pub(super) statistics_coverage_view: wgpu::TextureView,
pub(super) statistics_coverage_bind_group: wgpu::BindGroup,
```

Create it at the physical scene extent with format `R8Unorm` and usage `RENDER_ATTACHMENT | TEXTURE_BINDING`. Add a non-filtering texture bind-group layout to `SceneGpuShared` and include it in only the interaction pipeline layouts.

Change `HudRenderTarget` to accept both raw scene color and statistics coverage. Configure the HUD pipeline with two color targets and clear only the coverage attachment on every HUD pass. In `scene.wgsl`, draw 12 vertices per HUD record:

```wgsl
let echo_vertex = vertex_index < 6u;
let quad_vertex = vertex_index % 6u;
let echo_offset = select(vec2<f32>(0.0), vec2<f32>(0.60, -0.60), echo_vertex);
let scene_z = select(instance.scene_z, COMPANION_STATISTICS_ECHO_Z, echo_vertex);
```

Return an MRT fragment result:

```wgsl
struct HudFragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) coverage: f32,
}

let glyph_coverage = textureSample(glyph_coverage_texture, glyph_sampler, input.uv).r;
let echo_alpha = select(1.0, 0.12, input.is_echo != 0u);
return HudFragmentOutput(
    source_over_color * echo_alpha,
    select(glyph_coverage, 0.0, input.is_echo != 0u),
);
```

The echo writes zero coverage, so only primary glyphs can reveal pet ink. Keep the record count and draw instance count fixed; only the per-instance vertex count changes from `6` to `12`.

- [ ] **Step 5: Compile and validate the exact three-draw interaction plan**

Add closed types in `render.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HudInteractionSource { PetBody, PetParticles, MoodAura }

#[derive(Debug, Clone)]
struct HudInteractionDrawPlan {
    body: ScenePlannedDraw,
    particles: ScenePlannedDraw,
    aura: ScenePlannedDraw,
}
```

During candidate materialization, resolve exactly one `PetBody` draw, exactly one `PetParticles` draw, and the analytic draw whose primitive has `binding_index == AnalyticSemantic::MoodAura.id().0`. Reject missing, duplicate, wrong-pipeline, wall-shadow, or floor-projection candidates with `SceneDrawPlanError`; do not select by generic pet attachment or draw order. Store this fixed plan on `GpuSceneCandidate`.

- [ ] **Step 6: Add masked interaction shaders and encode between HUD and world suffix**

Refactor the existing pet glyph and mood-aura fragment color calculations into WGSL helper functions. Add three pipelines using the original blend contracts:

```rust
hud_interaction_body: source_over_glyph,
hud_interaction_particles: additive_glyph,
hud_interaction_aura: source_over_analytic,
```

Each interaction fragment loads coverage at `vec2<i32>(input.position.xy)` and multiplies its premultiplied output by:

```wgsl
let reveal = statistics_coverage * hud_interaction_buffer.value.reveal_mix;
```

Use depth compare `Always` with depth writes disabled: the private primary mask is the occlusion boundary at equality, while the normal suffix remains authoritative above `+0.72`.

Change `encode_scene_with_sealed_hud` to this exact order:

```rust
encode_scene_world_prefix(...);
prepared_hud.encode(..., &targets.statistics_coverage_view)?;
if prepared_hud.statistics_interaction_enabled() {
    encode_hud_interaction_overlay(..., &candidate.hud_interaction_plan);
}
encode_scene_world_suffix(...);
encode_scene_chrome(...);
```

Missing coverage resources, invalid reveal values, or an invalid fixed interaction plan fail the render transaction. There is no fallback that changes global HUD opacity.

- [ ] **Step 7: Add native overlap-continuity and non-overlap invariance tests**

Use the existing retained native-device fixture to render one frozen scene at effective depths `0.64`, `0.68`, `0.72`, and `f32::from_bits(0.72_f32.to_bits() + 1)`. Assert:

```rust
assert_eq!(non_overlap_rgba(&at_start), non_overlap_rgba(&at_mid));
assert_eq!(non_overlap_rgba(&at_mid), non_overlap_rgba(&at_plane));
assert!(overlap_pet_contribution(&at_start) < overlap_pet_contribution(&at_mid));
assert!(overlap_pet_contribution(&at_mid) < overlap_pet_contribution(&at_plane));
assert!(mean_rgb_absolute_difference(&at_plane, &just_front) <= 1.0);
```

Also assert the rear echo is occluded by a pet at `0.68` where they overlap, the primary coverage texture is cleared each frame, redacted capture uses the same static redacted glyphs, and live values remain absent from scene JSON/checksum/debug output.

- [ ] **Step 8: Run retained tests**

Run:

```bash
cargo test --features retained-renderer --lib companion::retained::hud::tests
cargo test --features retained-renderer --lib companion::retained::render::tests
cargo test --test round_scene
```

Expected: PASS. Native Metal-only tests may self-skip when no compatible adapter is available; all CPU contract, privacy, resource, shader, and plan tests must execute and pass.

- [ ] **Step 9: Commit the retained implementation**

```bash
git add src/companion/retained/hud.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl src/companion/retained/host.rs src/companion/retained.rs
git commit -m "feat(companion): soften retained statistics crossings"
```

---

### Task 4: Focused Integration Verification and Companion Rebuild

**Files:**
- Modify only if a focused test exposes a defect in files already listed above.

**Interfaces:**
- Consumes: completed shared, Smooth/AppKit, and retained interaction implementations
- Produces: one rebuilt optimized `target/macos/Glorp.app` running the new crossing behavior

- [ ] **Step 1: Run formatting and the focused behavior suite**

Run:

```bash
cargo fmt --check
cargo test --lib round::depth::tests
cargo test --lib companion::app::tests
cargo test --features retained-renderer --lib companion::retained::hud::tests
cargo test --features retained-renderer --lib companion::retained::render::tests
cargo test --test companion_draw_boundary
cargo test --test round_scene
```

Expected: all commands PASS. Do not add a Preview Lab, paired-capture, or acceptance-matrix gate in this slice.

- [ ] **Step 2: Inspect the final diff for scope and privacy**

Run:

```bash
git diff --check HEAD~3..HEAD
git diff --stat HEAD~3..HEAD
rg -n "981\.7M|349\.4k/10m" src/companion/retained src/presentation/companion_scene
```

Expected: no whitespace errors; only the planned HUD/depth files changed; the live-value search returns no production source matches.

- [ ] **Step 3: Rebuild and relaunch the optimized companion**

Run:

```bash
cargo xtask companion fresh
```

Expected: the optimized macOS app bundle builds, any running companion quits, and the fresh `target/macos/Glorp.app` opens.

- [ ] **Step 4: Commit only if verification required a scoped correction**

If Step 1 exposed and Step 2 confirmed a correction within this plan's files:

```bash
git add src/round/depth.rs src/companion/app.rs src/companion/retained/hud.rs src/companion/retained/render.rs src/companion/retained/scene.wgsl src/companion/retained/host.rs src/companion/retained.rs tests/companion_draw_boundary.rs
git commit -m "fix(companion): close soft HUD interaction gaps"
```

If no correction was needed, do not create an empty commit.
