# Glorp Pixel Default-Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the opt-in Smooth Pixel companion from an animated proof into a default-readiness candidate by aligning Pixel with the real pet-art cast, fixing HUD-safe runtime placement, exporting deterministic review artifacts, and recording pass/fail readiness evidence while keeping Classic as the default renderer.

**Architecture:** Add a cached portable art-reference layer under `presentation::pixel` that derives sanitized role/bounds data from canonical `RenderedPet { lines, spans }` without serializing raw seeds or terminal art in machine JSON. Make `PixelPetScene` consume that reference for silhouette/face/accent placement, then place the final `PixelFrame` through one shared production fit helper used by tests, Preview Lab, and AppKit. Preview Lab and measurement docs become the readiness evidence surface; AppKit remains draw-only.

**Tech Stack:** Rust 2021, existing `time`, existing `serde` / `serde_json`, existing Preview Lab exporter, macOS `objc2-app-kit`, existing `top` / `sample` measurement commands.

## Global Constraints

- Do not create a branch unless Drew asks for one.
- Pixel remains opt-in; Classic remains the default companion renderer.
- Do not remove Classic companion.
- Do not change `glorp watch` terminal rendering.
- Do not add Linux companion host work in this slice.
- Do not literally blit terminal glyphs or terminal `pet_art` into AppKit.
- Do not add external sprite-sheet, asset-pipeline, 3D, voxel, camera, lighting, or rigging dependencies.
- Pixel portable modules must not depend on AppKit or `objc2`.
- `PixelPetArtReference` must never store or serialize raw pet seed.
- Machine-readable Pixel artifacts must not include raw terminal `art_text`, source labels, exact usage counts, file paths, diagnostics, prompt text, response text, transcript text, or user paths.
- Art reference extraction must use canonical rendered pet output (`RenderedPet { lines, spans }`) or a shared helper that returns the same rendered/spanned result.
- Art reference extraction must be cached/keyed and must not rebuild terminal strings on every 30fps Pixel tick.
- AppKit, Preview Lab, and tests must use the same production Pixel fit helper.
- A test-only fit approximation cannot satisfy readiness.
- Target fit geometries are `360x360`, `260x260`, at least `480x480`, and a fullscreen-equivalent geometry.
- Body, eye, and mouth pixels must have zero overlap with the HUD-safe text zone in readiness fixtures.
- Active/feed-pulse CPU evidence must exercise the live companion pulse state transition or an equivalent hidden-dev path that calls the same transition.
- A failed readiness gate blocks the measurement doc from recommending a default flip.
- Add no new dependencies.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/presentation/pixel/art_reference.rs` | New sanitized, cached art-reference model derived from canonical rendered pet spans. |
| `src/presentation/pixel/mod.rs` | Export art-reference types and provider. |
| `src/presentation/pixel/input.rs` | Add discrete art pose key to Pixel input, but keep raw seed out of serialized/reference types. |
| `src/presentation/pixel_input.rs` | Build `PixelPetInput` and `PixelArtReferenceRequest` from `WatchViewModel`. |
| `src/presentation/pixel/scene.rs` | Derive Pixel scene geometry from `PixelPetArtReference`; keep procedural body only as named fallback. |
| `src/presentation/pixel/animator.rs` | Accept art reference in `PixelRendererTick` and rasterize reference-driven Pixel frames. |
| `src/presentation/pixel/frame.rs` | Add high-alpha bounds helpers used by fit tests and Preview sidecars. |
| `src/round/pixel_fit.rs` | New production `PixelCompanionFit` / HUD-safe placement helper shared by Preview Lab and AppKit. |
| `src/round/mod.rs` | Export `pixel_fit`. |
| `src/companion/pixel.rs` | Draw `PixelFrame` into the production fit rectangle instead of the entire aperture. |
| `src/companion/app.rs` | Use cached art references, support hidden review options, and keep live pulse path authoritative. |
| `src/commands/companion_mode.rs` | Add hidden review options if the plan's AppKit task chooses CLI-driven review launch. |
| `src/cli.rs` | Hidden review flags for deterministic companion review launch. |
| `src/commands/companion.rs` | Forward hidden review flags to `companion-app`. |
| `src/commands/companion_app.rs` | Pass hidden review flags to native AppKit entrypoint. |
| `src/dev_preview/export.rs` | Add typed Pixel art/fit sidecar artifacts and manifest entries. |
| `src/dev_preview/pixel.rs` | Render art-reference, fit/readability, species matrix, and side-by-side review fixtures. |
| `src/dev_preview/assets/preview.css` | Style Pixel side-by-side review artifacts when new classes are added. |
| `src/dev_preview/assets/preview.js` | Load additional Pixel sidecar links when the HTML renderer exposes them. |
| `tests/pixel_art_reference.rs` | New art-reference, role, cache, and privacy tests. |
| `tests/pixel_renderer.rs` | Extend renderer tests for reference-driven hero output and all species/stages smoke. |
| `tests/pixel_fit.rs` | New production fit/HUD-safe geometry tests. |
| `tests/dev_preview.rs` | Add Pixel sidecar, manifest, side-by-side, and privacy allowlist assertions. |
| `tests/cli_smoke.rs` | Add hidden review option/default renderer assertions. |
| `docs/superpowers/measurements/2026-07-08-glorp-pixel-default-readiness-review.md` | New readiness measurement and gate table. |

## Task 1: Portable Pixel Art Reference And Cache

**Files:**
- Create: `src/presentation/pixel/art_reference.rs`
- Modify: `src/presentation/pixel/mod.rs`
- Modify: `src/presentation/pixel/input.rs`
- Modify: `src/presentation/pixel_input.rs`
- Test: `tests/pixel_art_reference.rs`

**Interfaces:**
- Produces: `PixelArtPoseKey`
- Produces: `PixelArtRole`
- Produces: `PixelPetArtReference`
- Produces: `PixelArtReferenceRequest`
- Produces: `PixelArtReferenceProvider::reference_for(&mut self, request: &PixelArtReferenceRequest) -> PixelPetArtReference`
- Produces: `PixelPetInput::from_watch_view_model_with_art_request(vm, now) -> (PixelPetInput, PixelArtReferenceRequest)`

- [ ] **Step 1: Write failing art-reference tests**

Create `tests/pixel_art_reference.rs`:

```rust
use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    PixelArtReferenceProvider, PixelArtRole, PixelPetInput,
};
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

fn reference_for(vm: &WatchViewModel, ms: i64) -> glorp::presentation::pixel::PixelPetArtReference {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let (_input, request) = PixelPetInput::from_watch_view_model_with_art_request(vm, now);
    let mut provider = PixelArtReferenceProvider::default();
    provider.reference_for(&request)
}

#[test]
fn fuzz_s3_reference_preserves_real_cast_cues() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;
    vm.pet_render.mood = Mood::Content;

    let reference = reference_for(&vm, 0);

    assert_eq!(reference.species, Species::Fuzz);
    assert_eq!(reference.stage, Stage::S3);
    assert!(reference.role_count(PixelArtRole::Eye) >= 2);
    assert!(reference.role_count(PixelArtRole::Locket) >= 1);
    assert!(reference.foot_contact.cells.len() >= 2);
    assert!(reference.body_bounds.width() >= 6);
    assert!(reference.body_bounds.height() >= 5);
    assert!(reference.occupied_cells.len() >= 30);
}

#[test]
fn glitch_s4_reference_preserves_repair_and_protected_face_roles() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Glitch;
    vm.pet_render.stage = Stage::S4;
    vm.pet_render.mood = Mood::Content;
    vm.life_profile.burst_level = 0.9;
    vm.last_feed_pulse_at = Some(now - time::Duration::milliseconds(300));

    let reference = reference_for(&vm, 300);

    assert_eq!(reference.species, Species::Glitch);
    assert_eq!(reference.stage, Stage::S4);
    assert!(reference.role_count(PixelArtRole::Eye) > 0);
    assert!(reference.role_count(PixelArtRole::Mouth) > 0);
    assert!(reference.role_count(PixelArtRole::RepairMark) > 0);
    assert!(reference.role_count(PixelArtRole::Corruption) > 0);
    for cell in reference.cells_for_roles([PixelArtRole::Eye, PixelArtRole::Mouth]) {
        assert!(
            !reference.cells_for_roles([PixelArtRole::Corruption]).contains(&cell),
            "face cell must not be transient corruption: {cell:?}"
        );
    }
}

#[test]
fn reference_pose_is_stable_across_continuous_pixel_elapsed_time() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;

    let first = reference_for(&vm, 0);
    let later_same_pose = reference_for(&vm, 33);

    assert_eq!(first.pose, later_same_pose.pose);
    assert_eq!(first.reference_checksum, later_same_pose.reference_checksum);
}

#[test]
fn reference_provider_caches_same_pose_request() {
    let base = datetime!(2026-07-08 12:00 UTC);
    let vm = WatchViewModel::fixture();
    let (_input, request) = PixelPetInput::from_watch_view_model_with_art_request(&vm, base);
    let mut provider = PixelArtReferenceProvider::default();

    let first = provider.reference_for(&request);
    let second = provider.reference_for(&request);

    assert_eq!(first, second);
    assert_eq!(provider.render_count_for_test(), 1);
}

#[test]
fn serialized_reference_does_not_leak_raw_seed_or_terminal_art() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.seed = "very-secret-seed".to_string();
    let reference = reference_for(&vm, 0);

    let json = serde_json::to_string(&reference).unwrap();

    assert!(!json.contains("very-secret-seed"));
    assert!(!json.contains("art_text"));
    assert!(!json.contains("/\\\\_/\\\\"));
    assert!(!json.contains("( o.o )"));
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test --test pixel_art_reference -- --nocapture
```

Expected: FAIL with unresolved imports for `PixelArtReferenceProvider`, `PixelArtRole`, and `from_watch_view_model_with_art_request`.

- [ ] **Step 3: Add the art-reference module and exports**

Create `src/presentation/pixel/art_reference.rs` with these public types and helpers. Keep internal rendering helpers private:

```rust
use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::generation::Species;
use crate::pet::render::{AnimationFrame, PaletteRoleName, WorkAccent};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PixelArtPoseKey {
    pub tick: u64,
    pub hold_eyes_closed: bool,
    pub blink_slowdown: u8,
    pub soft_eyes: bool,
    pub work_accent: &'static str,
    pub feed_reaction: bool,
    pub glitch_patch_tier: Option<&'static str>,
    pub glitch_burst_level: Option<&'static str>,
    pub glitch_calm_mode: bool,
    pub glitch_feed_reaction: bool,
}

impl PixelArtPoseKey {
    pub fn from_animation_frame(frame: AnimationFrame) -> Self {
        Self {
            tick: frame.tick,
            hold_eyes_closed: frame.hold_eyes_closed,
            blink_slowdown: frame.blink_slowdown,
            soft_eyes: frame.soft_eyes,
            work_accent: work_accent_label(frame.work_accent),
            feed_reaction: frame.feed_reaction,
            glitch_patch_tier: frame.glitch_corruption.map(|glitch| glitch.patch_tier.as_str()),
            glitch_burst_level: frame.glitch_corruption.map(|glitch| glitch.burst_level.as_str()),
            glitch_calm_mode: frame
                .glitch_corruption
                .is_some_and(|glitch| glitch.calm_mode),
            glitch_feed_reaction: frame
                .glitch_corruption
                .is_some_and(|glitch| glitch.feed_reaction),
        }
    }
}

fn work_accent_label(accent: WorkAccent) -> &'static str {
    match accent {
        WorkAccent::None => "none",
        WorkAccent::Alert => "alert",
        WorkAccent::Focused => "focused",
        WorkAccent::Dreamy => "dreamy",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PixelArtRole {
    Body,
    BodyGlow,
    Eye,
    Mouth,
    Accent,
    Pattern,
    Particle,
    Corruption,
    Outline,
    InteriorTexture,
    Appendage,
    FootContact,
    Locket,
    Facet,
    RepairMark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PixelArtCell {
    pub x: u8,
    pub y: u8,
    pub role: PixelArtRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelCellBounds {
    pub min_x: u8,
    pub min_y: u8,
    pub max_x: u8,
    pub max_y: u8,
}

impl PixelCellBounds {
    pub const fn width(self) -> u8 {
        self.max_x - self.min_x + 1
    }

    pub const fn height(self) -> u8 {
        self.max_y - self.min_y + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PixelFootContact {
    pub cells: Vec<(u8, u8)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelReferenceChecksum(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PixelPetArtReference {
    pub species: Species,
    pub stage: Stage,
    pub mood: Mood,
    pub pose: PixelArtPoseKey,
    pub width_cells: u8,
    pub height_cells: u8,
    pub occupied_cells: Vec<PixelArtCell>,
    pub body_bounds: PixelCellBounds,
    pub foot_contact: PixelFootContact,
    pub reference_checksum: PixelReferenceChecksum,
    pub role_counts: BTreeMap<&'static str, usize>,
}

impl PixelPetArtReference {
    pub fn role_count(&self, role: PixelArtRole) -> usize {
        let label = role.as_str();
        self.role_counts.get(label).copied().unwrap_or(0)
    }

    pub fn cells_for_roles<const N: usize>(&self, roles: [PixelArtRole; N]) -> Vec<PixelArtCell> {
        self.occupied_cells
            .iter()
            .copied()
            .filter(|cell| roles.contains(&cell.role))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelArtReferenceRequest {
    pub seed: String,
    pub species: Species,
    pub stage: Stage,
    pub mood: Mood,
    pub variation_bucket: u16,
    pub pose: PixelArtPoseKey,
    pub animation_frame: AnimationFrame,
}

#[derive(Debug, Default, Clone)]
pub struct PixelArtReferenceProvider {
    cached: Option<(PixelArtReferenceKey, PixelPetArtReference)>,
    render_count_for_test: usize,
}
```

Implement:

- `PixelArtRole::as_str() -> &'static str`
- `PixelArtPoseKey::from_animation_frame`
- `From<PaletteRoleName> for PixelArtRole`
- `PixelArtReferenceProvider::reference_for`
- `PixelArtReferenceProvider::render_count_for_test`
- private `render_reference(request: &PixelArtReferenceRequest) -> PixelPetArtReference`
- private cell classification helpers that identify locket (`Fuzz` glyph marker role), facet (`Crystal` facet glyphs), repair mark (`Glitch` `Pattern`/`Accent` single-cell patch roles under Glitch S4/S5/S6 body patch candidates), foot contact (bottom-most occupied cells), outline and interior texture from occupied edge/interior cells

Use canonical rendering:

```rust
let generated = crate::pet::generation::generate_pet(&request.seed).with_species(request.species);
let rendered = crate::pet::render::render_pet(
    &generated,
    request.stage,
    request.mood,
    request.animation_frame,
);
```

Compute `PixelReferenceChecksum` only from `species`, `stage`, `mood`, `pose`, sanitized role cells, and bounds. Do not include `request.seed`.

- [ ] **Step 4: Wire request derivation from `WatchViewModel`**

Modify `src/presentation/pixel/mod.rs`:

```rust
pub mod art_reference;

pub use art_reference::{
    PixelArtCell, PixelArtPoseKey, PixelArtReferenceProvider, PixelArtReferenceRequest,
    PixelArtRole, PixelCellBounds, PixelFootContact, PixelPetArtReference,
    PixelReferenceChecksum,
};
```

Modify `src/presentation/pixel_input.rs` to add:

```rust
impl PixelPetInput {
    pub fn from_watch_view_model_with_art_request(
        vm: &WatchViewModel,
        now: time::OffsetDateTime,
    ) -> (Self, PixelArtReferenceRequest) {
        let input = Self::from_watch_view_model(vm, now);
        let pose_tick = ((now.unix_timestamp_nanos() / 250_000_000).max(0)) as u64;
        let feed_reaction = crate::pet::animator::compute_token_pop(vm.last_feed_pulse_at, now)
            .is_some();
        let pet_performance = crate::tui::room::pet_performance_from_day_context(&vm.day_context);
        let glitch_corruption = if vm.pet_render.generated_species == crate::pet::generation::Species::Glitch {
            Some(crate::pet::render::glitch_corruption_frame_for_inputs(
                vm.day_context.date_seed,
                vm.day_context.today_ratio,
                vm.life_profile.burst_level,
                vm.life_profile.calm_mode,
                feed_reaction,
            ))
        } else {
            None
        };
        let animation_frame = crate::pet::render::AnimationFrame {
            tick: pose_tick,
            hold_eyes_closed: vm.day_context.asleep,
            blink_slowdown: crate::pet::render::blink_slowdown_for_tiredness(
                vm.day_context.tiredness,
            ),
            soft_eyes: matches!(
                pet_performance,
                crate::tui::room::PetPerformance::TiredAwake
                    | crate::tui::room::PetPerformance::HeavyDayCozy
            ),
            work_accent: crate::pet::render::work_accent_for_profile(&vm.life_profile),
            feed_reaction,
            glitch_corruption,
            ..crate::pet::render::AnimationFrame::default()
        };
        let request = PixelArtReferenceRequest {
            seed: vm.pet_render.seed.clone(),
            species: vm.pet_render.generated_species,
            stage: vm.pet_render.stage,
            mood: vm.pet_render.mood,
            variation_bucket: input.identity.variation_key.0,
            pose: PixelArtPoseKey::from_animation_frame(animation_frame),
            animation_frame,
        };
        (input, request)
    }
}
```

If `unix_timestamp_nanos` is unavailable in this crate version, use `(now - time::OffsetDateTime::UNIX_EPOCH).whole_milliseconds() / 250`.

- [ ] **Step 5: Run art-reference tests**

Run:

```bash
cargo test --test pixel_art_reference -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/presentation/pixel/art_reference.rs src/presentation/pixel/mod.rs src/presentation/pixel/input.rs src/presentation/pixel_input.rs tests/pixel_art_reference.rs
git commit -m "feat(pixel): add cached pet art reference"
```

## Task 2: Render Pixel Frames From Art References

**Files:**
- Modify: `src/presentation/pixel/scene.rs`
- Modify: `src/presentation/pixel/animator.rs`
- Modify: `src/presentation/pixel/frame.rs`
- Modify: `tests/pixel_renderer.rs`
- Test: `tests/pixel_art_reference.rs`

**Interfaces:**
- Changes: `PixelRendererTick<'a>` gains `pub art_reference: &'a PixelPetArtReference`
- Produces: `PixelFrame::alpha_bounds(min_alpha: u8) -> Option<PixelBounds>`
- Produces: `PixelPetScene::from_input_and_reference(input, art_reference, state, now)`

- [ ] **Step 1: Write failing renderer tests for reference-driven output**

Add to `tests/pixel_renderer.rs`:

```rust
use glorp::presentation::pixel::{
    PixelArtReferenceProvider, PixelArtRole, PixelPetArtReference,
};

fn frame_for_with_reference(vm: &WatchViewModel, ms: i64) -> (glorp::presentation::pixel::PixelFrame, PixelPetArtReference) {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(vm, now);
    let mut provider = PixelArtReferenceProvider::default();
    let reference = provider.reference_for(&request);
    let mut state = PixelRendererState::new(&input, base);
    let frame = render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &reference,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    });
    (frame, reference)
}

#[test]
fn all_species_all_stages_render_reference_driven_frames() {
    const STAGES: [Stage; 7] = [
        Stage::S0,
        Stage::S1,
        Stage::S2,
        Stage::S3,
        Stage::S4,
        Stage::S5,
        Stage::S6,
    ];

    for species in Species::all() {
        for stage in STAGES {
            let mut vm = WatchViewModel::fixture();
            vm.pet_render.generated_species = species;
            vm.pet_render.stage = stage;
            vm.pet_render.mood = Mood::Content;

            let (frame, reference) = frame_for_with_reference(&vm, 500);

            assert!(reference.occupied_cells.len() > 0, "{species:?} {stage:?} reference empty");
            assert!(frame.opaque_pixel_count() > 40, "{species:?} {stage:?} frame empty");
            let bounds = frame.opaque_bounds().expect("visible frame");
            assert!(bounds.max_x < frame.width);
            assert!(bounds.max_y < frame.height);
        }
    }
}

#[test]
fn hero_frame_uses_reference_roles_not_species_only_shape() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;
    vm.pet_render.mood = Mood::Content;

    let (frame, reference) = frame_for_with_reference(&vm, 480);

    assert!(reference.role_count(PixelArtRole::Locket) > 0);
    assert!(
        frame.changed_pixel_count(&frame_for(&vm, 480)) > 0,
        "reference-driven renderer should no longer match the old procedural-only helper"
    );
}

#[test]
fn high_alpha_bounds_are_available_for_fit_checks() {
    let vm = WatchViewModel::fixture();
    let (frame, _reference) = frame_for_with_reference(&vm, 0);

    let bounds = frame.alpha_bounds(200).expect("high alpha body bounds");

    assert!(bounds.min_x <= bounds.max_x);
    assert!(bounds.min_y <= bounds.max_y);
}
```

Update existing helper `frame_for` to build an art reference and pass it into `PixelRendererTick`.

- [ ] **Step 2: Run focused renderer tests and confirm failures**

Run:

```bash
cargo test --test pixel_renderer all_species_all_stages_render_reference_driven_frames -- --nocapture
cargo test --test pixel_renderer high_alpha_bounds_are_available_for_fit_checks -- --nocapture
```

Expected: FAIL because `PixelRendererTick.art_reference` and `PixelFrame::alpha_bounds` do not exist.

- [ ] **Step 3: Add `alpha_bounds`**

Modify `src/presentation/pixel/frame.rs`:

```rust
impl PixelFrame {
    pub fn alpha_bounds(&self, min_alpha: u8) -> Option<PixelBounds> {
        self.assert_storage_invariant();
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0_u16;
        let mut max_y = 0_u16;
        let mut found = false;

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
                if self.pixels[idx].a < min_alpha {
                    continue;
                }
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }

        found.then_some(PixelBounds { min_x, min_y, max_x, max_y })
    }
}
```

- [ ] **Step 4: Thread art reference through renderer API**

Modify `src/presentation/pixel/animator.rs`:

```rust
use super::art_reference::{PixelArtRole, PixelPetArtReference};

pub struct PixelRendererTick<'a> {
    pub input: &'a PixelPetInput,
    pub art_reference: &'a PixelPetArtReference,
    pub viewport: PixelViewport,
    pub now: time::OffsetDateTime,
    pub state: &'a mut PixelRendererState,
}
```

Change `render_pixel_frame` to call:

```rust
let scene = PixelPetScene::from_input_and_reference(
    tick.input,
    tick.art_reference,
    tick.state,
    tick.now,
);
```

- [ ] **Step 5: Replace hero body geometry with reference-driven rasterization**

In `src/presentation/pixel/scene.rs`, add reference bounds and placement fields:

```rust
pub struct PixelPetScene {
    pub body_rx: i16,
    pub body_ry: i16,
    pub accent_count: u8,
    pub blocky: bool,
    pub wispy: bool,
    pub wander_x: f32,
    pub breath_y: f32,
    pub blink_closed: bool,
    pub pulse_alpha: f32,
    pub reference_scale: i16,
    pub reference_origin_x: i16,
    pub reference_origin_y: i16,
}
```

Implement `from_input_and_reference` by keeping existing continuous `wander_x`, `breath_y`, `blink_closed`, and `pulse_alpha`, but deriving `body_rx`, `body_ry`, `accent_count`, `reference_scale`, and reference origin from `PixelPetArtReference.body_bounds`.

In `src/presentation/pixel/animator.rs`, replace `draw_body`, `draw_face`, and `draw_accents` for available references with a new helper:

```rust
fn draw_reference_cells(
    frame: &mut PixelFrame,
    input: &PixelPetInput,
    scene: &PixelPetScene,
    reference: &PixelPetArtReference,
    cx: i16,
    cy: i16,
) {
    for cell in &reference.occupied_cells {
        let color = color_for_role(input, cell.role);
        let x = cx + scene.reference_origin_x + i16::from(cell.x) * scene.reference_scale;
        let y = cy + scene.reference_origin_y + i16::from(cell.y) * scene.reference_scale;
        fill_rect(frame, x, y, scene.reference_scale, scene.reference_scale, color);
    }
}
```

Use existing procedural draw helpers only in `draw_fallback_body` when a reference has no occupied cells. Keep aura and shadow as Pixel renderer output.

Implement `color_for_role(input, role)` so `Eye` and `Mouth` use `input.palette.eye`, `Corruption` uses `input.palette.corruption`, `Pattern` uses `input.palette.pattern`, and body roles use `input.palette.body`.

- [ ] **Step 6: Update all renderer call sites**

Update:

- `tests/pixel_renderer.rs`
- `src/dev_preview/pixel.rs`
- `src/companion/app.rs`

Each call should obtain a reference through `PixelArtReferenceProvider` and pass it into `PixelRendererTick`.

For companion state, add the provider to `AppState`:

```rust
pixel_art_provider: Option<PixelArtReferenceProvider>,
```

In `render_live_pixel_frame`, call:

```rust
let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(vm, now);
let art_reference = pixel_state.art_reference_for(&request);
let frame = render_pixel_frame(PixelRendererTick {
    input: &input,
    art_reference: &art_reference,
    viewport: PixelViewport::companion_default(),
    now,
    state: pixel_state,
});
```

If the provider lives on `PixelRendererState`, keep one source of truth and do not add a second `pixel_art_provider` field.

- [ ] **Step 7: Run renderer/art-reference tests**

Run:

```bash
cargo test --test pixel_art_reference -- --nocapture
cargo test --test pixel_renderer -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 2**

```bash
git add src/presentation/pixel/scene.rs src/presentation/pixel/animator.rs src/presentation/pixel/frame.rs src/dev_preview/pixel.rs src/companion/app.rs tests/pixel_renderer.rs tests/pixel_art_reference.rs
git commit -m "feat(pixel): render frames from pet art references"
```

## Task 3: Shared Production Fit And HUD-Safe Geometry

**Files:**
- Create: `src/round/pixel_fit.rs`
- Modify: `src/round/mod.rs`
- Modify: `src/companion/pixel.rs`
- Modify: `src/companion/app.rs`
- Test: `tests/pixel_fit.rs`
- Test: `tests/pixel_renderer.rs`

**Interfaces:**
- Produces: `PixelTargetGeometry`
- Produces: `PixelCompanionFit`
- Produces: `pixel_companion_fit(geometry, frame_size, hud_text) -> PixelCompanionFit`
- Changes: `companion::pixel::draw_pixel_frame(frame, bounds, aperture)` uses `PixelCompanionFit.image_rect`

- [ ] **Step 1: Write failing fit tests**

Create `tests/pixel_fit.rs`:

```rust
use glorp::presentation::pixel::{PixelBounds, PixelViewport};
use glorp::round::hud::companion_hud_text;
use glorp::round::pixel_fit::{pixel_companion_fit, PixelTargetGeometry};

fn fit_for(size: u16) -> glorp::round::pixel_fit::PixelCompanionFit {
    let hud = companion_hud_text(205_700_000.0, Some(9.99), 9_900_000.0);
    pixel_companion_fit(
        PixelTargetGeometry { width: size, height: size },
        PixelViewport::companion_default(),
        &hud,
    )
}

#[test]
fn pixel_fit_does_not_use_the_entire_aperture() {
    let fit = fit_for(360);

    assert!(fit.image_rect.width < fit.aperture.radius * 2.0);
    assert!(fit.image_rect.height < fit.aperture.radius * 2.0);
    assert!(fit.image_rect.y < fit.hud_safe_zone.y);
}

#[test]
fn body_bounds_do_not_overlap_hud_safe_zone_for_target_geometries() {
    let body = PixelBounds { min_x: 26, min_y: 20, max_x: 70, max_y: 67 };
    for size in [260_u16, 360, 480, 900] {
        let fit = fit_for(size);
        assert!(
            !fit.logical_bounds_overlap_hud(body),
            "body bounds overlapped HUD safe zone for {size}x{size}: {fit:?}"
        );
    }
}

#[test]
fn fit_names_production_helper_for_preview_contracts() {
    let fit = fit_for(360);

    assert_eq!(fit.producer, "round::pixel_fit::pixel_companion_fit");
}
```

- [ ] **Step 2: Run failing fit tests**

Run:

```bash
cargo test --test pixel_fit -- --nocapture
```

Expected: FAIL because `round::pixel_fit` does not exist.

- [ ] **Step 3: Add `round::pixel_fit`**

Create `src/round/pixel_fit.rs`:

```rust
use crate::presentation::pixel::{PixelBounds, PixelViewport};
use crate::round::hud::{companion_hud_text, CompanionHudText, COMPANION_GAUGE_GAP_DEG};
use crate::round::layout::RoundAperture;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelTargetGeometry {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelFitRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PixelFitRect {
    pub fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelCompanionFit {
    pub producer: &'static str,
    pub geometry: PixelTargetGeometry,
    pub aperture: RoundAperture,
    pub image_rect: PixelFitRect,
    pub creature_zone: PixelFitRect,
    pub hud_safe_zone: PixelFitRect,
    pub scale: f32,
}
```

Implement:

```rust
pub fn pixel_companion_fit(
    geometry: PixelTargetGeometry,
    viewport: PixelViewport,
    hud: &CompanionHudText,
) -> PixelCompanionFit
```

Rules:

- compute `RoundAperture::new(geometry.width, geometry.height)`
- compute `perimeter_gauge_layout` and `stat_gap_box` using `COMPANION_GAUGE_GAP_DEG`
- estimate HUD text size conservatively from `hud.today_total`, `hud.daily_percent`, `hud.pace`, and companion font size
- reserve the lower HUD zone around `stat_gap_box`
- place Pixel image above that zone with at least 8 percent aperture margin
- keep `image_rect` square and nearest-neighbor scalable
- set `producer` exactly to `"round::pixel_fit::pixel_companion_fit"`

Add:

```rust
impl PixelCompanionFit {
    pub fn logical_bounds_overlap_hud(&self, bounds: PixelBounds) -> bool {
        let mapped = self.map_logical_bounds(bounds);
        mapped.overlaps(self.hud_safe_zone)
    }

    pub fn map_logical_bounds(&self, bounds: PixelBounds) -> PixelFitRect {
        let sx = self.image_rect.width / 96.0;
        let sy = self.image_rect.height / 96.0;
        PixelFitRect {
            x: self.image_rect.x + f32::from(bounds.min_x) * sx,
            y: self.image_rect.y + f32::from(bounds.min_y) * sy,
            width: f32::from(bounds.max_x - bounds.min_x + 1) * sx,
            height: f32::from(bounds.max_y - bounds.min_y + 1) * sy,
        }
    }
}
```

Modify `src/round/mod.rs`:

```rust
pub mod pixel_fit;
```

- [ ] **Step 4: Use production fit in AppKit drawing**

Modify `src/companion/pixel.rs`:

```rust
use crate::round::hud::CompanionHudText;
use crate::round::pixel_fit::{pixel_companion_fit, PixelTargetGeometry};

pub fn draw_pixel_frame(
    frame: &PixelFrame,
    bounds: NSRect,
    aperture: RoundAperture,
    hud_text: &CompanionHudText,
) {
    let fit = pixel_companion_fit(
        PixelTargetGeometry {
            width: bounds.size.width.round() as u16,
            height: bounds.size.height.round() as u16,
        },
        PixelViewport { logical_width: frame.width, logical_height: frame.height },
        hud_text,
    );
    let image_rect = NSRect::new(
        NSPoint::new(f64::from(fit.image_rect.x), f64::from(fit.image_rect.y)),
        NSSize::new(f64::from(fit.image_rect.width), f64::from(fit.image_rect.height)),
    );
    // draw image into image_rect, not full aperture_rect
}
```

Modify the Pixel call site in `src/companion/app.rs` to compute the same `hud_text` already used by `draw_hud`:

```rust
let hud_text = companion_hud_text(
    vm.today_effective_tokens,
    vm.daily_comparison.fraction_of_yesterday,
    vm.rate_momentum.pulse.current_tokens,
);
crate::companion::pixel::draw_pixel_frame(frame, bounds, aperture, &hud_text);
```

- [ ] **Step 5: Run fit and AppKit adapter tests**

Run:

```bash
cargo test --test pixel_fit -- --nocapture
cargo test companion::pixel::tests -- --nocapture
```

Expected: PASS on macOS for AppKit tests; non-macOS should compile because `companion` is cfg-gated.

- [ ] **Step 6: Commit Task 3**

```bash
git add src/round/pixel_fit.rs src/round/mod.rs src/companion/pixel.rs src/companion/app.rs tests/pixel_fit.rs tests/pixel_renderer.rs
git commit -m "feat(pixel): add shared companion fit policy"
```

## Task 4: Preview Lab Default-Readiness Artifacts

**Files:**
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/pixel.rs`
- Modify: `src/dev_preview/assets/preview.css`
- Modify: `src/dev_preview/assets/preview.js`
- Modify: `tests/dev_preview.rs`

**Interfaces:**
- Produces: `PreviewPixelArtArtifact`
- Produces: `PreviewPixelFitArtifact`
- Produces: `PIXEL_ART_SCHEMA_VERSION`
- Produces: `PIXEL_FIT_SCHEMA_VERSION`
- Adds manifest file fields `pixel_art` and `pixel_fit` to Pixel scenarios

- [ ] **Step 1: Write failing Preview Lab contract tests**

Add to `tests/dev_preview.rs` near existing Pixel tests:

```rust
#[test]
fn dev_preview_pixel_writes_art_and_fit_sidecars() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("preview");

    glorp::commands::dev_preview::run_for_test(
        out.clone(),
        glorp::cli::PreviewScenarioArg::Pixel,
    )
    .unwrap();

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    let scenario = manifest["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "pixel-fuzz-s3-content-idle")
        .unwrap();

    let art_path = out.join(scenario["files"]["pixel_art"].as_str().unwrap());
    let fit_path = out.join(scenario["files"]["pixel_fit"].as_str().unwrap());

    assert!(art_path.exists());
    assert!(fit_path.exists());

    let art_json = std::fs::read_to_string(art_path).unwrap();
    assert!(art_json.contains("\"schema_version\""));
    assert!(art_json.contains("\"role_counts\""));
    assert!(!art_json.contains("fixture-seed"));
    assert!(!art_json.contains("art_text"));

    let fit_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fit_path).unwrap()).unwrap();
    assert_eq!(
        fit_json["producer"],
        "round::pixel_fit::pixel_companion_fit"
    );
    assert_eq!(fit_json["hud_overlap"]["body_eye_mouth_pixels"], 0);
}

#[test]
fn pixel_preview_uses_correct_fuzz_s3_label() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("preview");

    glorp::commands::dev_preview::run_for_test(
        out.clone(),
        glorp::cli::PreviewScenarioArg::Pixel,
    )
    .unwrap();

    let text = std::fs::read_to_string(out.join("frames/pixel-fuzz-s3-content-idle.txt")).unwrap();
    assert!(text.contains("stage s3 pup"));
    assert!(!text.contains("archfuzz"));
}
```

Adjust helper names if the existing dev-preview test helper has a different entry point; use the existing pattern in `tests/dev_preview.rs`.

- [ ] **Step 2: Run failing Preview tests**

Run:

```bash
cargo test --features dev-preview --test dev_preview pixel -- --nocapture
```

Expected: FAIL because sidecar fields and correct Fuzz S3 label are missing.

- [ ] **Step 3: Add typed sidecar artifacts to exporter**

Modify `src/dev_preview/export.rs`:

```rust
pub const PIXEL_ART_SCHEMA_VERSION: u32 = 1;
pub const PIXEL_FIT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelArtArtifact {
    pub schema_version: u32,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub reference_checksum: String,
    pub width_cells: u8,
    pub height_cells: u8,
    pub body_bounds: crate::presentation::pixel::PixelCellBounds,
    pub foot_contact: crate::presentation::pixel::PixelFootContact,
    pub role_counts: std::collections::BTreeMap<&'static str, usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewPixelFitArtifact {
    pub schema_version: u32,
    pub producer: &'static str,
    pub geometry: crate::round::pixel_fit::PixelTargetGeometry,
    pub image_rect: crate::round::pixel_fit::PixelFitRect,
    pub hud_safe_zone: crate::round::pixel_fit::PixelFitRect,
    pub hud_overlap: PreviewPixelHudOverlap,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelHudOverlap {
    pub body_eye_mouth_pixels: u16,
    pub translucent_effect_pixels: u16,
}
```

Add `pixel_art` and `pixel_fit` fields to `PreviewScenarioFiles`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub pixel_art: Option<PathBuf>,
#[serde(skip_serializing_if = "Option::is_none")]
pub pixel_fit: Option<PathBuf>,
```

Add `ArtifactType::PixelArt` and `ArtifactType::PixelFit`. Add writer calls through existing `write_json_artifact`.

- [ ] **Step 4: Export Pixel sidecars and side-by-side fixtures**

Modify `src/dev_preview/pixel.rs`:

- Correct Fuzz S3 summary line to `"stage s3 pup"`.
- Build `(input, request)` via `from_watch_view_model_with_art_request`.
- Use `PixelArtReferenceProvider` for `reference`.
- Use `pixel_companion_fit` for default/min/large geometries.
- Attach `frame.contract.pixel`, plus sidecar file paths in `frame.files`.
- Add side-by-side text or HTML-visible summary frame that includes the canonical terminal reference for human review, but keep raw terminal lines out of `*.pixel-art.json`.

Sidecar file paths:

```rust
fn pixel_art_path(id: &str) -> PathBuf {
    PathBuf::from(format!("frames/{id}.pixel-art.json"))
}

fn pixel_fit_path(id: &str) -> PathBuf {
    PathBuf::from(format!("frames/{id}.pixel-fit.json"))
}
```

- [ ] **Step 5: Add privacy allowlist assertions**

Extend the existing Pixel privacy test in `tests/dev_preview.rs` to scan `*.pixel-art.json` and `*.pixel-fit.json`:

```rust
for entry in glob::glob(out.join("frames/*.pixel-art.json").to_str().unwrap()).unwrap() {
    let text = std::fs::read_to_string(entry.unwrap()).unwrap();
    for forbidden in [
        "fixture-seed",
        "art_text",
        "claude",
        "codex",
        "/Users/",
        "prompt",
        "response",
        "transcript",
        "diagnostic",
    ] {
        assert!(
            !text.to_lowercase().contains(&forbidden.to_lowercase()),
            "pixel art sidecar leaked {forbidden}: {text}"
        );
    }
}
```

If the repo avoids `glob`, use `std::fs::read_dir(out.join("frames"))` and filter filenames ending in `.pixel-art.json`.

- [ ] **Step 6: Run Preview tests**

Run:

```bash
cargo test --features dev-preview --test dev_preview pixel -- --nocapture
cargo test --features dev-preview dev_preview::scenarios -- --nocapture
cargo run --features dev-preview -- dev-preview --scenario pixel --out target/glorp-preview-pixel-readiness
```

Expected: tests PASS; preview bundle includes `.pixel.json`, `.pixel-art.json`, `.pixel-fit.json`, and side-by-side hero fixtures.

- [ ] **Step 7: Commit Task 4**

```bash
git add src/dev_preview/export.rs src/dev_preview/pixel.rs src/dev_preview/assets/preview.css src/dev_preview/assets/preview.js tests/dev_preview.rs
git commit -m "feat(dev-preview): export pixel readiness artifacts"
```

## Task 5: Hidden Runtime Review Paths For AppKit Geometry And Active Pulse

**Files:**
- Modify: `src/commands/companion_mode.rs`
- Modify: `src/cli.rs`
- Modify: `src/lib.rs`
- Modify: `src/commands/companion.rs`
- Modify: `src/commands/companion_app.rs`
- Modify: `src/companion/app.rs`
- Modify: `src/watch_live.rs`
- Test: `tests/cli_smoke.rs`
- Test: `src/watch_live.rs` module tests

**Interfaces:**
- Produces: `CompanionReviewOptions`
- Produces: hidden CLI flags `--review-size WIDTHxHEIGHT` and `--review-active-pulse`
- Produces: `watch_live::bursting_review_signal(now) -> AppliedUsageSignal`
- Changes: `companion::app::run(renderer_mode, review_options)`

- [ ] **Step 1: Write failing CLI/default tests**

Add to `tests/cli_smoke.rs`:

```rust
#[test]
fn companion_help_hides_review_options_and_renderer_default() {
    Command::cargo_bin("glorp")
        .unwrap()
        .args(["companion", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--review-size").not())
        .stdout(predicate::str::contains("--review-active-pulse").not())
        .stdout(predicate::str::contains("--renderer").not());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn companion_accepts_hidden_review_flags_before_macos_gate() {
    Command::cargo_bin("glorp")
        .unwrap()
        .args([
            "companion",
            "--renderer",
            "pixel",
            "--review-size",
            "260x260",
            "--review-active-pulse",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "glorp companion is only available on macOS",
        ));
}
```

Add to `src/watch_live.rs` tests:

```rust
#[test]
fn review_burst_signal_uses_live_burst_path() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let signal = bursting_review_signal(now);
    assert!(signal.can_burst());

    let mut state = WatchPresentationState::default();
    let mut vm = WatchViewModel::fixture();
    stamp_live_presentation(&mut state, &mut vm, signal, now);

    assert_eq!(vm.last_feed_pulse_at, Some(now));
    assert!(vm.life_profile.burst_level > 0.0);
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test --test cli_smoke companion_ -- --nocapture
cargo test watch_live::tests::review_burst_signal_uses_live_burst_path -- --nocapture
```

Expected: FAIL because hidden review flags and `bursting_review_signal` do not exist.

- [ ] **Step 3: Add review options and hidden CLI flags**

Modify `src/commands/companion_mode.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompanionReviewOptions {
    pub initial_size: Option<CompanionReviewSize>,
    pub active_pulse: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionReviewSize {
    pub width: u16,
    pub height: u16,
}

impl std::str::FromStr for CompanionReviewSize {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((width, height)) = value.split_once('x') else {
            return Err("expected WIDTHxHEIGHT, for example 260x260".to_string());
        };
        let width = width
            .parse::<u16>()
            .map_err(|_| "width must be an integer".to_string())?;
        let height = height
            .parse::<u16>()
            .map_err(|_| "height must be an integer".to_string())?;
        if width < 120 || height < 120 {
            return Err("review size must be at least 120x120".to_string());
        }
        Ok(Self { width, height })
    }
}
```

Modify `src/cli.rs` hidden args on both `Companion` and `CompanionApp`:

```rust
#[arg(long, hide = true)]
review_size: Option<CompanionReviewSize>,
#[arg(long, hide = true)]
review_active_pulse: bool,
```

In `src/lib.rs`, build:

```rust
let review = CompanionReviewOptions {
    initial_size: review_size,
    active_pulse: review_active_pulse,
};
commands::companion::run(renderer, review)?
```

Apply the same for `CompanionApp`.

- [ ] **Step 4: Forward review options to AppKit**

Modify command signatures:

```rust
pub fn run(mode: CompanionRendererMode, review: CompanionReviewOptions) -> Result<()>
```

When `commands::companion::run` launches the app bundle, forward:

```rust
if let Some(size) = review.initial_size {
    command.args(["--review-size", &format!("{}x{}", size.width, size.height)]);
}
if review.active_pulse {
    command.arg("--review-active-pulse");
}
```

Modify `src/companion/app.rs`:

- `pub fn run(renderer_mode: CompanionRendererMode, review: CompanionReviewOptions) -> Result<()>`
- `build_window(mtm, review.initial_size)` uses default `360x360` when `None`
- after initial VM is built, if `review.active_pulse`, call `stamp_live_presentation` with `watch_live::bursting_review_signal(now)`

- [ ] **Step 5: Add live review burst helper**

Modify `src/watch_live.rs`:

```rust
pub fn bursting_review_signal(now: OffsetDateTime) -> AppliedUsageSignal {
    AppliedUsageSignal {
        applied_effective_tokens: 42_000.0,
        raw_effective_tokens: Some(42_000.0),
        source_mix: None,
        token_shape: None,
        observed_at: now,
        elapsed_since_successful_poll: time::Duration::seconds(10),
        freshness: crate::tui::life::UsageSignalFreshness::Live,
    }
}
```

Use this helper only for hidden review/debug paths and tests.

- [ ] **Step 6: Run CLI and live-path tests**

Run:

```bash
cargo test --test cli_smoke companion_ -- --nocapture
cargo test watch_live::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Manual AppKit review commands**

Run after building:

```bash
cargo xtask companion fresh
open -n target/macos/Glorp.app --args --renderer pixel --review-size 360x360
open -n target/macos/Glorp.app --args --renderer pixel --review-size 260x260
open -n target/macos/Glorp.app --args --renderer pixel --review-size 480x480
open -n target/macos/Glorp.app --args --renderer pixel --review-size 360x360 --review-active-pulse
```

Expected: Classic default still opens without Pixel flags; Pixel review windows open at requested sizes; active pulse visibly triggers the Pixel pulse path.

- [ ] **Step 8: Commit Task 5**

```bash
git add src/commands/companion_mode.rs src/cli.rs src/lib.rs src/commands/companion.rs src/commands/companion_app.rs src/companion/app.rs src/watch_live.rs tests/cli_smoke.rs
git commit -m "feat(companion): add pixel review launch paths"
```

## Task 6: Measurement Evidence, Gate Table, And Final Verification

**Files:**
- Create: `docs/superpowers/measurements/2026-07-08-glorp-pixel-default-readiness-review.md`
- Modify: `docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md` only if cross-linking is useful
- Test: no new test file

**Interfaces:**
- Produces: readiness gate table with pass/fail/blocked rows
- Produces: recorded commands and artifact paths for Preview Lab, AppKit, CPU, and full gate

- [ ] **Step 1: Generate Preview Lab readiness bundle**

Run:

```bash
cargo run --features dev-preview -- dev-preview --scenario pixel --out target/glorp-preview-pixel-readiness
```

Expected: PASS and `target/glorp-preview-pixel-readiness/manifest.json` lists Pixel frame, art, and fit artifacts.

- [ ] **Step 2: Run automated gate**

Run:

```bash
cargo fmt --check
cargo test
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo clippy --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features --all-targets
```

Expected: PASS.

- [ ] **Step 3: Capture AppKit review screenshots or geometry artifacts**

Run each launch and capture window evidence. If AX/AppleScript resize is still blocked, use the hidden review-size launch path and Preview Lab fit artifacts that call the production fit helper.

```bash
cargo xtask companion fresh
open -n target/macos/Glorp.app --args --renderer pixel --review-size 360x360
open -n target/macos/Glorp.app --args --renderer pixel --review-size 260x260
open -n target/macos/Glorp.app --args --renderer pixel --review-size 480x480
open -n target/macos/Glorp.app --args --renderer pixel --review-size 360x360 --review-active-pulse
```

Record:

- PID
- window id
- screenshot path or geometry artifact path
- target size
- whether the runtime fit helper is named in the evidence

- [ ] **Step 4: Run CPU measurements**

Use the same build mode, machine, and `360x360` size for Classic and Pixel.

Warm each process for at least 10 seconds. Then run:

```bash
top -pid "$classic_pid" -stats pid,command,cpu,time -l 12 -s 5 > target/glorp-classic-idle-top.txt
top -pid "$pixel_pid" -stats pid,command,cpu,time -l 12 -s 5 > target/glorp-pixel-idle-top.txt
top -pid "$classic_active_pid" -stats pid,command,cpu,time -l 12 -s 5 > target/glorp-classic-active-top.txt
top -pid "$pixel_active_pid" -stats pid,command,cpu,time -l 12 -s 5 > target/glorp-pixel-active-top.txt
sample "$pixel_pid" 10 -file target/glorp-pixel-idle-sample.txt
sample "$pixel_active_pid" 10 -file target/glorp-pixel-active-sample.txt
```

Compute average excluding the first `top` sample and p95 over kept samples. The default-flip candidate budget is:

- Pixel idle average no more than 5 percentage points above Classic idle average
- Pixel idle p95 no more than 10 percentage points above Classic idle p95
- Pixel active average no more than 5 percentage points above Classic active average
- Pixel active p95 no more than 10 percentage points above Classic active p95

- [ ] **Step 5: Write the measurement doc**

Create `docs/superpowers/measurements/2026-07-08-glorp-pixel-default-readiness-review.md`.
The doc must contain these sections, and every table row must use observed
values from Steps 1-4 before commit. Use only `pass`, `fail`, or `blocked` in
result columns.

Required content:

- Heading: `# Pixel Default-Readiness Review`
- Metadata lines: `Date: 2026-07-08`, current short commit from
  `git rev-parse --short HEAD`, reviewer `Drew Ritter`, and machine name from
  `hostname`.
- `## Preview Lab`: include the exact Preview Lab command from Step 1,
  manifest path `target/glorp-preview-pixel-readiness/manifest.json`, hero
  side-by-side artifact paths, sidecar artifact paths, and a result.
- `## AppKit Review`: table rows for Classic default, Pixel 360x360, Pixel
  260x260, Pixel 480x480, and Pixel active pulse. Each row must include launch
  command, PID, window id when available, evidence path, and result.
- `## CPU`: table rows for Classic idle, Pixel idle, Classic active, and Pixel
  active. Each row must include PID, raw `top` artifact path, sample artifact
  path when available, kept sample count, average CPU, p95 CPU, budget, and
  result.
- `## Default-Readiness Gates`: rows for Runtime fit authority, HUD body
  overlap, Cast identity, All species/stages smoke, Active pulse path, CPU
  budget, Resize freshness, and Privacy. Each row must include artifact path,
  result, and notes.
- `## Default Flip Decision`: state `Pixel remains opt-in in this
  implementation.` Then state either `Recommendation: ready for a separate
  default-flip diff.` or `Recommendation: blocked by the failed or blocked gates
  listed above.`

Before committing the doc, run:

```bash
rg -n "pass/fail|blocked by listed gates|<|>|top \.\.\.|empty|TBD|TODO" docs/superpowers/measurements/2026-07-08-glorp-pixel-default-readiness-review.md
```

Expected: no output.

- [ ] **Step 6: Commit Task 6**

```bash
git add docs/superpowers/measurements/2026-07-08-glorp-pixel-default-readiness-review.md
git commit -m "docs: add pixel readiness review evidence"
```

## Final Review And Handoff

- [ ] **Step 1: Inspect final diff**

Run:

```bash
git status --short --branch
git log --oneline -6
git diff --stat HEAD~6..HEAD
```

Expected: clean worktree after final commit; six task commits on top of the plan/spec commits.

- [ ] **Step 2: Request adversarial code review**

Dispatch at least three reviewers:

- architecture/cache reviewer for `PixelArtReferenceProvider` and renderer API
- product/art reviewer for hero/cast identity and Preview side-by-side artifacts
- verification reviewer for fit helper authority, CPU evidence, and privacy artifacts

Fix Critical and Important findings before presenting completion.

- [ ] **Step 3: Run final automated gate**

Run:

```bash
cargo fmt --check
cargo test
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo clippy --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features --all-targets
```

Expected: PASS.

- [ ] **Step 4: Final commit if review fixes were made**

If review fixes changed code or docs:

```bash
git status --short
git add docs/superpowers/measurements/2026-07-08-glorp-pixel-default-readiness-review.md
git add src/presentation/pixel src/round src/dev_preview src/companion src/commands src/cli.rs src/lib.rs tests
git commit -m "fix(pixel): address readiness review findings"
```

If no review fixes were needed, do not create an empty commit.
