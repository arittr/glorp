# Glorp Smooth Pixel Companion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in Smooth Pixel renderer for `glorp companion` that displays a visibly animated pixel pet in the existing macOS companion window while keeping Classic as the default renderer.

**Architecture:** Add a cfg-free portable pixel renderer under `src/presentation/pixel/` that converts sanitized `WatchViewModel` state into deterministic `PixelFrame` RGBA frames. Preview Lab exports the same `PixelFrame` artifacts before AppKit consumes them. The macOS companion chooses Classic or Pixel through a hidden renderer switch; AppKit owns only windowing, timer, scale-to-fit, and drawing the portable frame.

**Tech Stack:** Rust 2021, `serde` / `serde_json`, existing `time`, existing Preview Lab HTML/CSS/JS assets, `assert_cmd` CLI smoke tests, macOS `objc2-app-kit` direct drawing.

## Global Constraints

- Do not create a branch unless Drew asks for one.
- Do not change `glorp watch` terminal rendering.
- Do not add 3D, voxel, camera, lighting, rigging, or external asset pipelines.
- Keep Classic companion as the default renderer in the first implementation.
- Add Pixel as an opt-in hidden/internal renderer switch.
- Pixel renderer core must live outside `cfg(target_os = "macos")` and must not depend on AppKit or `objc2`.
- Pixel mode must not read terminal-rendered `vm.pet_art` / `vm.pet_spans` and must not call `rerender_pet_for_view_model`.
- The V1 default logical companion frame is `96x96`.
- `PixelFrame.pixels.len() == PixelFrame.width * PixelFrame.height`.
- Pixels are row-major, top-left origin, sRGB, unpremultiplied RGBA8.
- Outside the round aperture uses alpha `0`.
- Hosts scale the entire logical frame with nearest-neighbor interpolation; hosts must not need layer semantics.
- Preserve the existing companion halo/trouble overlay, perimeter gauges, and bottom HUD above the Pixel interior.
- Preview Lab manifest schema is currently `7`; bump it to `8` when adding Pixel artifacts.
- Pixel artifact schema starts at `1`.
- The hidden `--renderer classic|pixel` argument must not be gated behind the `dev-preview` feature.
- Before Pixel can become default, average process CPU at default size must stay within 2 percentage points of Classic during idle review and within 5 percentage points of Classic during active review.
- Add no new dependencies.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/commands/companion_mode.rs` | Shared hidden renderer mode enum and string conversion. |
| `src/cli.rs` | Hidden `--renderer classic|pixel` arguments on `companion` and `companion-app`; `dev-preview --scenario pixel`. |
| `src/lib.rs` | Pass selected renderer mode into command dispatch. |
| `src/commands/mod.rs` | Expose `companion_mode`. |
| `src/commands/companion.rs` | Forward Pixel launch through `open -n target/macos/Glorp.app --args --renderer pixel`; keep Classic default behavior. |
| `src/commands/companion_app.rs` | Pass renderer mode into the native companion entrypoint. |
| `src/presentation/surface.rs` | Add `PIXEL_STYLE` to the shared color policy. |
| `src/presentation/mod.rs` | Export `pixel` and `PIXEL_STYLE`. |
| `src/presentation/pixel/mod.rs` | Public module boundary for pixel input, frame, animator, scene, and renderer helpers. |
| `src/presentation/pixel/input.rs` | Sanitized `PixelPetInput`, `PixelPetIdentity`, `PixelVariationKey`, activity, sleep, and pulse derivation. |
| `src/presentation/pixel/frame.rs` | `PixelViewport`, `Rgba8`, `PixelFrame`, frame invariants, bounds, diff, and row-run helpers. |
| `src/presentation/pixel/scene.rs` | Semantic `PixelPetScene` and species/stage/mood visual parameters. |
| `src/presentation/pixel/animator.rs` | `PixelRendererState`, `PixelRendererTick`, deterministic state transitions, and frame rendering entrypoint. |
| `src/presentation/pixel/raster.rs` | Small pixel primitives: circle, ellipse, rect, and alpha blending inside the logical frame. |
| `tests/pixel_scene.rs` | Sanitization, identity, color policy, and all-species input coverage. |
| `tests/pixel_renderer.rs` | Determinism, frame invariants, all-species non-empty output, hero fixtures, reactions, and movement bounds. |
| `src/dev_preview/pixel.rs` | Pixel Preview Lab scenarios and animation strips. |
| `src/dev_preview/mod.rs` | Expose the pixel preview module. |
| `src/dev_preview/export.rs` | Schema `8`, `PreviewScenarioKind::Pixel`, `PreviewStripKind::PixelAnimation`, `ArtifactType::PixelFrame`, pixel file paths, writer, links, and canvas HTML. |
| `src/dev_preview/scenarios.rs` | Generate pixel scenarios and write `frames/*.pixel.json` / `strips/*/frame-*.pixel.json`. |
| `src/dev_preview/assets/preview.css` | Pixel canvas styling with `image-rendering: pixelated`. |
| `src/dev_preview/assets/preview.js` | Pixel canvas loader / strip playback for RGBA artifacts. |
| `tests/dev_preview.rs` | Pixel manifest, artifacts, schema, canvas links, strip contract, and privacy assertions. |
| `src/companion/mod.rs` | Accept renderer mode in `companion::run`. |
| `src/companion/app.rs` | Store renderer mode, use Classic tick path or Pixel 30 FPS path, and preserve overlays. |
| `src/companion/pixel.rs` | AppKit adapter that draws `PixelFrame` row runs into the round aperture with nearest-neighbor scaling. |
| `docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md` | Manual AppKit review and CPU measurement evidence. |

## Task 1: Hidden Renderer Mode Plumbing

**Files:**
- Create: `src/commands/companion_mode.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/cli.rs`
- Modify: `src/lib.rs`
- Modify: `src/commands/companion.rs`
- Modify: `src/commands/companion_app.rs`
- Test: `tests/cli_smoke.rs`

**Interfaces:**
- Produces: `CompanionRendererMode::{Classic, Pixel}` with `Default`, `ValueEnum`, and `as_str()`.
- Produces: `commands::companion::run(mode: CompanionRendererMode) -> Result<()>`.
- Produces: `commands::companion_app::run(mode: CompanionRendererMode) -> Result<()>`.
- Produces: `companion::run(mode: CompanionRendererMode) -> Result<()>` on macOS.

- [ ] **Step 1: Write failing CLI smoke tests**

Add these tests near the existing companion CLI smoke tests in `tests/cli_smoke.rs`:

```rust
#[test]
fn help_hides_companion_renderer_switch() {
    Command::cargo_bin("glorp")
        .unwrap()
        .args(["companion", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--renderer").not())
        .stdout(predicate::str::contains("classic").not())
        .stdout(predicate::str::contains("pixel").not());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn companion_accepts_hidden_pixel_renderer_before_macos_gate() {
    Command::cargo_bin("glorp")
        .unwrap()
        .args(["companion", "--renderer", "pixel"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "glorp companion is only available on macOS",
        ));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn companion_app_accepts_hidden_pixel_renderer_before_macos_gate() {
    Command::cargo_bin("glorp")
        .unwrap()
        .args(["companion-app", "--renderer", "pixel"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "glorp companion-app is only available on macOS",
        ));
}

#[test]
fn companion_rejects_unknown_renderer() {
    Command::cargo_bin("glorp")
        .unwrap()
        .args(["companion", "--renderer", "sprite-cloud"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}
```

- [ ] **Step 2: Run the new CLI tests and confirm they fail**

Run:

```bash
cargo test --test cli_smoke companion_ -- --nocapture
```

Expected: FAIL because `--renderer` is not accepted yet.

- [ ] **Step 3: Add the shared renderer mode**

Create `src/commands/companion_mode.rs`:

```rust
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum CompanionRendererMode {
    #[default]
    Classic,
    Pixel,
}

impl CompanionRendererMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            CompanionRendererMode::Classic => "classic",
            CompanionRendererMode::Pixel => "pixel",
        }
    }

    pub const fn is_pixel(self) -> bool {
        matches!(self, CompanionRendererMode::Pixel)
    }
}
```

Modify `src/commands/mod.rs`:

```rust
pub mod companion_mode;
```

- [ ] **Step 4: Add hidden CLI args**

Modify the companion command variants in `src/cli.rs`:

```rust
use crate::commands::companion_mode::CompanionRendererMode;
```

```rust
/// Open the native macOS round companion app.
Companion {
    #[arg(long, value_enum, hide = true, default_value_t = CompanionRendererMode::Classic)]
    renderer: CompanionRendererMode,
},
#[command(hide = true)]
CompanionApp {
    #[arg(long, value_enum, hide = true, default_value_t = CompanionRendererMode::Classic)]
    renderer: CompanionRendererMode,
},
```

Modify dispatch in `src/lib.rs`:

```rust
Command::Companion { renderer } => commands::companion::run(renderer)?,
Command::CompanionApp { renderer } => commands::companion_app::run(renderer)?,
```

- [ ] **Step 5: Forward renderer mode through the launcher**

Modify `src/commands/companion.rs` signatures:

```rust
use crate::commands::companion_mode::CompanionRendererMode;
use crate::error::{GlorpError, Result};

#[cfg(target_os = "macos")]
pub fn run(mode: CompanionRendererMode) -> Result<()> {
    // existing helper locator persistence stays first
    let app = companion_app_path()?;
    let mut command = std::process::Command::new("open");
    if mode.is_pixel() {
        command.arg("-n");
    }
    command.arg(&app);
    if mode.is_pixel() {
        command.arg("--args").arg("--renderer").arg(mode.as_str());
    }
    let status = command.status()?;
    if !status.success() {
        return Err(GlorpError::Message(format!(
            "failed to open Glorp.app at {}",
            app.display()
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn run(_mode: CompanionRendererMode) -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion is only available on macOS".into(),
    ))
}
```

Keep the existing helper locator persistence and `companion_app_path()` body exactly where they are today. The only command behavior change is that Pixel launch uses `open -n target/macos/Glorp.app --args --renderer pixel`.

Modify `src/commands/companion_app.rs`:

```rust
use crate::commands::companion_mode::CompanionRendererMode;

#[cfg(target_os = "macos")]
pub fn run(mode: CompanionRendererMode) -> Result<()> {
    crate::companion::run(mode)
}

#[cfg(not(target_os = "macos"))]
pub fn run(_mode: CompanionRendererMode) -> Result<()> {
    Err(GlorpError::Message(
        "glorp companion-app is only available on macOS".into(),
    ))
}
```

Modify `src/companion/mod.rs`:

```rust
pub fn run(mode: crate::commands::companion_mode::CompanionRendererMode) -> crate::error::Result<()> {
    app::run(mode)
}
```

Modify `src/companion/app.rs` temporarily:

```rust
pub fn run(_mode: crate::commands::companion_mode::CompanionRendererMode) -> Result<()> {
    // existing body unchanged in this task
}
```

- [ ] **Step 6: Run the task gate**

Run:

```bash
cargo fmt --check
cargo test --test cli_smoke companion_ -- --nocapture
cargo test --test cli_smoke help_hides_companion_renderer_switch -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/commands/companion_mode.rs src/commands/mod.rs src/cli.rs src/lib.rs src/commands/companion.rs src/commands/companion_app.rs src/companion/mod.rs src/companion/app.rs tests/cli_smoke.rs
git commit -m "feat(companion): add hidden renderer mode switch"
```

## Task 2: Portable Pixel Input And Frame Contract

**Files:**
- Create: `src/presentation/pixel/mod.rs`
- Create: `src/presentation/pixel/input.rs`
- Create: `src/presentation/pixel/frame.rs`
- Modify: `src/presentation/mod.rs`
- Modify: `src/presentation/surface.rs`
- Test: `tests/pixel_scene.rs`

**Interfaces:**
- Produces: `PixelPetInput::from_watch_view_model(vm: &WatchViewModel, now: OffsetDateTime) -> PixelPetInput`.
- Produces: `PixelVariationKey::from_seed(seed: &str) -> PixelVariationKey`.
- Produces: `PixelViewport::companion_default() -> PixelViewport`.
- Produces: `PixelFrame::transparent(viewport: PixelViewport) -> PixelFrame`.
- Produces: `PixelFrame::opaque_pixel_count() -> usize`, `PixelFrame::changed_pixel_count(&self, other: &Self) -> usize`, and `PixelFrame::opaque_bounds() -> Option<PixelBounds>`.

- [ ] **Step 1: Write failing privacy and frame-contract tests**

Create `tests/pixel_scene.rs`:

```rust
use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    PixelFrame, PixelPetInput, PixelVariationKey, PixelViewport, Rgba8,
};
use glorp::tui::view_model::{SourceUsageView, WatchViewModel};
use time::macros::datetime;

#[test]
fn pixel_input_redacts_raw_seed_and_private_runtime_fields() {
    let mut vm = WatchViewModel::fixture_with_events();
    vm.pet_render.seed = "secret-seed-/Users/drew/project".to_string();
    vm.source_breakdown = vec![SourceUsageView {
        name: "client-source".into(),
        display_name: "private-client".into(),
        effective_tokens: 123_456.0,
    }];
    vm.helper_status = "helper failed at /Users/drew/private".into();
    vm.errors = vec!["prompt response diagnostic".into()];

    let input = PixelPetInput::from_watch_view_model(&vm, datetime!(2026-07-08 12:00 UTC));
    let debug = format!("{input:?}");

    assert!(!debug.contains("secret-seed"));
    assert!(!debug.contains("/Users/drew"));
    assert!(!debug.contains("client-source"));
    assert!(!debug.contains("private-client"));
    assert!(!debug.contains("123456"));
    assert!(!debug.contains("prompt"));
    assert!(!debug.contains("response"));
    assert_eq!(input.identity.species, vm.pet_render.generated_species);
    assert_eq!(input.identity.stage, vm.pet_render.stage);
    assert_eq!(input.mood, vm.pet_render.mood);
}

#[test]
fn pixel_variation_key_is_stable_without_exposing_seed_text() {
    let key_a = PixelVariationKey::from_seed("fixture-seed");
    let key_b = PixelVariationKey::from_seed("fixture-seed");
    let key_c = PixelVariationKey::from_seed("different-seed");

    assert_eq!(key_a, key_b);
    assert_ne!(key_a, key_c);
    assert!(!format!("{key_a:?}").contains("fixture-seed"));
}

#[test]
fn pixel_input_changes_for_live_identity_and_state_signals() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut base = WatchViewModel::fixture();
    let mut other = base.clone();
    other.pet_render.generated_species = Species::Glitch;
    other.pet_render.stage = Stage::S4;
    other.pet_render.mood = Mood::Ecstatic;
    other.day_context.asleep = true;
    other.life_profile.calm_mode = true;
    other.life_profile.burst_level = 0.8;
    other.last_feed_pulse_at = Some(now - time::Duration::milliseconds(250));

    let base_input = PixelPetInput::from_watch_view_model(&base, now);
    let other_input = PixelPetInput::from_watch_view_model(&other, now);

    assert_ne!(base_input.identity.species, other_input.identity.species);
    assert_ne!(base_input.identity.stage, other_input.identity.stage);
    assert_ne!(base_input.mood, other_input.mood);
    assert!(!base_input.sleep.asleep);
    assert!(other_input.sleep.asleep);
    assert!(!base_input.pulse.active);
    assert!(other_input.pulse.active);
}

#[test]
fn pixel_frame_enforces_rgba_invariants() {
    let viewport = PixelViewport::companion_default();
    let frame = PixelFrame::transparent(viewport);

    assert_eq!(viewport.logical_width, 96);
    assert_eq!(viewport.logical_height, 96);
    assert_eq!(frame.width, 96);
    assert_eq!(frame.height, 96);
    assert_eq!(frame.pixels.len(), 96 * 96);
    assert_eq!(frame.opaque_pixel_count(), 0);
    assert_eq!(frame.opaque_bounds(), None);
    assert_eq!(frame.pixels[0], Rgba8 { r: 0, g: 0, b: 0, a: 0 });
}
```

- [ ] **Step 2: Run the tests and confirm they fail on missing module**

Run:

```bash
cargo test --test pixel_scene -- --nocapture
```

Expected: compile failure because `glorp::presentation::pixel` does not exist.

- [ ] **Step 3: Add `PIXEL_STYLE`**

Modify `src/presentation/surface.rs`:

```rust
pub const PIXEL_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Compact,
    clip: Clip::Circle,
    source_accent: false,
    phase_tint: false,
    energy_droop: false,
    shimmer: true,
    activity_lift: true,
    prop_reaction: false,
    eye_emphasis: EyeEmphasis::Brightness,
};
```

Replace the existing `pub use surface` block in `src/presentation/mod.rs` with this block so `PIXEL_STYLE` is exported:

```rust
pub use surface::{
    Clip, Detail, EyeEmphasis, LiveColorInputs, ResolvedColors, SurfaceStyle, MENU_STYLE,
    PIXEL_STYLE, ROUND_STYLE, SCREEN_STYLE, WATCH_STYLE,
};
```

Add the pixel module export:

```rust
pub mod pixel;
```

- [ ] **Step 4: Add the pixel module boundary**

Create `src/presentation/pixel/mod.rs`:

```rust
pub mod frame;
pub mod input;

pub use frame::{PixelBounds, PixelFrame, PixelViewport, Rgba8};
pub use input::{
    PixelActivity, PixelPetIdentity, PixelPetInput, PixelPulseState, PixelSleepState,
    PixelVariationKey,
};
```

- [ ] **Step 5: Add frame types and helpers**

Create `src/presentation/pixel/frame.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelViewport {
    pub logical_width: u16,
    pub logical_height: u16,
}

impl PixelViewport {
    pub const fn companion_default() -> Self {
        Self {
            logical_width: 96,
            logical_height: 96,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelFrame {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<Rgba8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelBounds {
    pub min_x: u16,
    pub min_y: u16,
    pub max_x: u16,
    pub max_y: u16,
}

impl PixelFrame {
    pub fn transparent(viewport: PixelViewport) -> Self {
        let len = usize::from(viewport.logical_width) * usize::from(viewport.logical_height);
        Self {
            width: viewport.logical_width,
            height: viewport.logical_height,
            pixels: vec![Rgba8::TRANSPARENT; len],
        }
    }

    pub fn set_pixel(&mut self, x: i16, y: i16, color: Rgba8) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as u16;
        let y = y as u16;
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.pixels[idx] = color;
    }

    pub fn opaque_pixel_count(&self) -> usize {
        self.pixels.iter().filter(|pixel| pixel.a > 0).count()
    }

    pub fn changed_pixel_count(&self, other: &Self) -> usize {
        assert_eq!((self.width, self.height), (other.width, other.height));
        self.pixels
            .iter()
            .zip(&other.pixels)
            .filter(|(a, b)| a != b)
            .count()
    }

    pub fn opaque_bounds(&self) -> Option<PixelBounds> {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0_u16;
        let mut max_y = 0_u16;
        let mut found = false;
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
                if self.pixels[idx].a == 0 {
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

- [ ] **Step 6: Add sanitized input derivation**

Create `src/presentation/pixel/input.rs`:

```rust
use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::generation::Species;
use crate::presentation::surface::{resolve_pet_colors, LiveColorInputs, ResolvedColors, PIXEL_STYLE};
use crate::tui::view_model::WatchViewModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelVariationKey(pub u16);

impl PixelVariationKey {
    pub fn from_seed(seed: &str) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in seed.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self((hash ^ (hash >> 32)) as u16)
    }

    pub const fn bucket(self, modulo: u16) -> u16 {
        if modulo == 0 {
            0
        } else {
            self.0 % modulo
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelPetIdentity {
    pub species: Species,
    pub stage: Stage,
    pub variation_key: PixelVariationKey,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelActivity {
    pub level: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSleepState {
    pub asleep: bool,
    pub calm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelPulseState {
    pub active: bool,
    pub age_ms: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelPetInput {
    pub identity: PixelPetIdentity,
    pub mood: Mood,
    pub palette: ResolvedColors,
    pub activity: PixelActivity,
    pub sleep: PixelSleepState,
    pub pulse: PixelPulseState,
}

impl PixelPetInput {
    pub fn from_watch_view_model(vm: &WatchViewModel, now: time::OffsetDateTime) -> Self {
        let mut color_inputs = LiveColorInputs::passthrough();
        color_inputs.activity_level = vm.life_profile.activity_level;
        let palette = resolve_pet_colors(&vm.pet_palette, &color_inputs, &PIXEL_STYLE);
        let pulse_age_ms = vm
            .last_feed_pulse_at
            .map(|pulse| (now - pulse).whole_milliseconds().clamp(0, i128::from(u16::MAX)) as u16);
        let pulse_active = pulse_age_ms.is_some_and(|age| age <= 2_000)
            && vm.life_profile.burst_level > 0.0
            && !vm.day_context.asleep;

        Self {
            identity: PixelPetIdentity {
                species: vm.pet_render.generated_species,
                stage: vm.pet_render.stage,
                variation_key: PixelVariationKey::from_seed(&vm.pet_render.seed),
            },
            mood: vm.pet_render.mood,
            palette,
            activity: PixelActivity {
                level: vm.life_profile.activity_level,
            },
            sleep: PixelSleepState {
                asleep: vm.day_context.asleep,
                calm: vm.life_profile.calm_mode || vm.day_context.asleep,
            },
            pulse: PixelPulseState {
                active: pulse_active,
                age_ms: pulse_age_ms.unwrap_or(u16::MAX),
            },
        }
    }
}
```

- [ ] **Step 7: Run the task gate**

Run:

```bash
cargo fmt --check
cargo test --test pixel_scene -- --nocapture
cargo test presentation::surface::resolver_tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/presentation/mod.rs src/presentation/surface.rs src/presentation/pixel/mod.rs src/presentation/pixel/input.rs src/presentation/pixel/frame.rs tests/pixel_scene.rs
git commit -m "feat(pixel): add portable companion input and frame contracts"
```

## Task 3: Portable Pixel Animator And Rasterizer

**Files:**
- Create: `src/presentation/pixel/scene.rs`
- Create: `src/presentation/pixel/animator.rs`
- Create: `src/presentation/pixel/raster.rs`
- Modify: `src/presentation/pixel/mod.rs`
- Test: `tests/pixel_renderer.rs`

**Interfaces:**
- Produces: `PixelRendererState::new(input: &PixelPetInput, now: OffsetDateTime) -> PixelRendererState`.
- Produces: `PixelRendererTick<'a> { input, viewport, now, state }`.
- Produces: `render_pixel_frame(tick: PixelRendererTick<'_>) -> PixelFrame`.
- Produces: `PixelPetScene::from_input(input: &PixelPetInput, state: &PixelRendererState, now: OffsetDateTime) -> PixelPetScene`.

- [ ] **Step 1: Write failing renderer tests**

Create `tests/pixel_renderer.rs`:

```rust
use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    render_pixel_frame, PixelPetInput, PixelRendererState, PixelRendererTick, PixelViewport,
};
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

fn frame_for(vm: &WatchViewModel, ms: i64) -> glorp::presentation::pixel::PixelFrame {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let input = PixelPetInput::from_watch_view_model(vm, now);
    let mut state = PixelRendererState::new(&input, base);
    render_pixel_frame(PixelRendererTick {
        input: &input,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    })
}

#[test]
fn pixel_renderer_is_deterministic_for_same_input_sequence() {
    let vm = WatchViewModel::fixture();
    let base = datetime!(2026-07-08 12:00 UTC);
    let mut state_a = PixelRendererState::new(
        &PixelPetInput::from_watch_view_model(&vm, base),
        base,
    );
    let mut state_b = PixelRendererState::new(
        &PixelPetInput::from_watch_view_model(&vm, base),
        base,
    );

    for ms in [0, 160, 320, 480, 640, 800, 960, 1_120] {
        let now = base + time::Duration::milliseconds(ms);
        let input = PixelPetInput::from_watch_view_model(&vm, now);
        let frame_a = render_pixel_frame(PixelRendererTick {
            input: &input,
            viewport: PixelViewport::companion_default(),
            now,
            state: &mut state_a,
        });
        let frame_b = render_pixel_frame(PixelRendererTick {
            input: &input,
            viewport: PixelViewport::companion_default(),
            now,
            state: &mut state_b,
        });
        assert_eq!(frame_a, frame_b);
    }
}

#[test]
fn every_species_renders_non_empty_inside_the_frame() {
    for species in Species::all() {
        let mut vm = WatchViewModel::fixture();
        vm.pet_render.generated_species = species;
        vm.pet_render.stage = Stage::S3;
        vm.pet_render.mood = Mood::Content;
        let frame = frame_for(&vm, 0);

        assert!(frame.opaque_pixel_count() > 120, "{species} rendered too few pixels");
        let bounds = frame.opaque_bounds().expect("species should render visible pixels");
        assert!(bounds.max_x < frame.width);
        assert!(bounds.max_y < frame.height);
    }
}

#[test]
fn hero_fuzz_and_glitch_frames_are_visibly_different() {
    let mut fuzz = WatchViewModel::fixture();
    fuzz.pet_render.generated_species = Species::Fuzz;
    fuzz.pet_render.stage = Stage::S3;
    fuzz.pet_render.mood = Mood::Content;

    let mut glitch = fuzz.clone();
    glitch.pet_render.generated_species = Species::Glitch;
    glitch.pet_render.stage = Stage::S4;
    glitch.life_profile.burst_level = 0.8;
    glitch.last_feed_pulse_at = Some(datetime!(2026-07-08 11:59:59 UTC));

    let fuzz_frame = frame_for(&fuzz, 500);
    let glitch_frame = frame_for(&glitch, 500);

    assert!(fuzz_frame.changed_pixel_count(&glitch_frame) > 600);
}

#[test]
fn asleep_motion_amplitude_is_lower_than_idle() {
    let mut idle = WatchViewModel::fixture();
    idle.day_context.asleep = false;
    idle.life_profile.calm_mode = false;

    let mut asleep = idle.clone();
    asleep.day_context.asleep = true;
    asleep.life_profile.calm_mode = true;

    let idle_a = frame_for(&idle, 0);
    let idle_b = frame_for(&idle, 800);
    let asleep_a = frame_for(&asleep, 0);
    let asleep_b = frame_for(&asleep, 800);

    assert!(idle_a.changed_pixel_count(&idle_b) > asleep_a.changed_pixel_count(&asleep_b));
}

#[test]
fn feed_pulse_changes_bounded_pixels() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut quiet = WatchViewModel::fixture();
    quiet.life_profile.burst_level = 0.0;
    quiet.last_feed_pulse_at = None;

    let mut pulsing = quiet.clone();
    pulsing.life_profile.burst_level = 0.9;
    pulsing.last_feed_pulse_at = Some(now - time::Duration::milliseconds(300));

    let quiet_frame = frame_for(&quiet, 300);
    let pulse_frame = frame_for(&pulsing, 300);
    let changed = quiet_frame.changed_pixel_count(&pulse_frame);

    assert!(changed > 80, "pulse should be visible");
    assert!(changed < 2_000, "pulse should stay bounded");
}
```

- [ ] **Step 2: Run the renderer tests and confirm they fail**

Run:

```bash
cargo test --test pixel_renderer -- --nocapture
```

Expected: compile failure because `PixelRendererState` and `render_pixel_frame` do not exist.

- [ ] **Step 3: Add module exports**

Modify `src/presentation/pixel/mod.rs`:

```rust
pub mod animator;
pub mod raster;
pub mod scene;

pub use animator::{render_pixel_frame, PixelRendererState, PixelRendererTick};
pub use scene::PixelPetScene;
```

- [ ] **Step 4: Implement raster primitives**

Create `src/presentation/pixel/raster.rs` with these public helpers:

```rust
use super::frame::{PixelFrame, Rgba8};

pub fn fill_rect(frame: &mut PixelFrame, x0: i16, y0: i16, width: i16, height: i16, color: Rgba8) {
    for y in y0..y0 + height {
        for x in x0..x0 + width {
            frame.set_pixel(x, y, color);
        }
    }
}

pub fn fill_ellipse(frame: &mut PixelFrame, cx: i16, cy: i16, rx: i16, ry: i16, color: Rgba8) {
    let rx2 = i32::from(rx.max(1)).pow(2);
    let ry2 = i32::from(ry.max(1)).pow(2);
    let limit = rx2 * ry2;
    for y in cy - ry..=cy + ry {
        for x in cx - rx..=cx + rx {
            let dx = i32::from(x - cx);
            let dy = i32::from(y - cy);
            if dx * dx * ry2 + dy * dy * rx2 <= limit {
                frame.set_pixel(x, y, color);
            }
        }
    }
}

pub fn fill_circle(frame: &mut PixelFrame, cx: i16, cy: i16, radius: i16, color: Rgba8) {
    fill_ellipse(frame, cx, cy, radius, radius, color);
}

pub fn alpha_blend_pixel(frame: &mut PixelFrame, x: i16, y: i16, color: Rgba8) {
    if color.a == 255 {
        frame.set_pixel(x, y, color);
        return;
    }
    if x < 0 || y < 0 || x as u16 >= frame.width || y as u16 >= frame.height {
        return;
    }
    let idx = usize::from(y as u16) * usize::from(frame.width) + usize::from(x as u16);
    let dst = frame.pixels[idx];
    let a = f32::from(color.a) / 255.0;
    let inv = 1.0 - a;
    frame.pixels[idx] = Rgba8 {
        r: (f32::from(color.r) * a + f32::from(dst.r) * inv).round() as u8,
        g: (f32::from(color.g) * a + f32::from(dst.g) * inv).round() as u8,
        b: (f32::from(color.b) * a + f32::from(dst.b) * inv).round() as u8,
        a: color.a.saturating_add(((u16::from(dst.a) * u16::from(255 - color.a)) / 255) as u8),
    };
}
```

- [ ] **Step 5: Implement scene parameters**

Create `src/presentation/pixel/scene.rs`:

```rust
use super::input::PixelPetInput;
use crate::pet::generation::Species;

#[derive(Debug, Clone, Copy, PartialEq)]
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
}

impl PixelPetScene {
    pub fn from_input(input: &PixelPetInput, elapsed_ms: i64) -> Self {
        let stage_scale = 1.0 + input.identity.stage.index() as f32 * 0.075;
        let calm_mult = if input.sleep.calm { 0.28 } else { 1.0 };
        let wander_phase = elapsed_ms as f32 / 900.0
            + f32::from(input.identity.variation_key.bucket(19)) * 0.17;
        let breath_phase = elapsed_ms as f32 / if input.sleep.asleep { 1_900.0 } else { 760.0 };
        let blink_period = 2_600 + i64::from(input.identity.variation_key.bucket(900));
        let blink_closed = elapsed_ms.rem_euclid(blink_period) < 120 && !input.sleep.asleep;
        let pulse_alpha = if input.pulse.active {
            1.0 - (f32::from(input.pulse.age_ms) / 2_000.0).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let (base_rx, base_ry, accent_count, blocky, wispy) = match input.identity.species {
            Species::Fuzz => (15, 13, 4, false, false),
            Species::Blob => (16, 11, 2, false, false),
            Species::Ghost => (13, 16, 3, false, true),
            Species::Glitch => (14, 12, 7, true, false),
            Species::Crystal => (13, 15, 6, true, false),
            Species::Mech => (15, 12, 5, true, false),
        };

        Self {
            body_rx: (base_rx as f32 * stage_scale).round() as i16,
            body_ry: (base_ry as f32 * stage_scale).round() as i16,
            accent_count,
            blocky,
            wispy,
            wander_x: wander_phase.sin() * 7.0 * calm_mult,
            breath_y: breath_phase.sin() * if input.sleep.asleep { 1.0 } else { 2.4 },
            blink_closed,
            pulse_alpha,
        }
    }
}
```

- [ ] **Step 6: Implement renderer state and frame rendering**

Create `src/presentation/pixel/animator.rs`. The render path must use only `PixelPetInput`, `PixelViewport`, `PixelRendererState`, and `now`:

```rust
use super::frame::{PixelFrame, PixelViewport, Rgba8};
use super::input::PixelPetInput;
use super::raster::{fill_circle, fill_ellipse, fill_rect};
use super::scene::PixelPetScene;
use crate::pet::generation::Species;

#[derive(Debug, Clone, PartialEq)]
pub struct PixelRendererState {
    start: time::OffsetDateTime,
}

impl PixelRendererState {
    pub fn new(_input: &PixelPetInput, now: time::OffsetDateTime) -> Self {
        Self { start: now }
    }
}

pub struct PixelRendererTick<'a> {
    pub input: &'a PixelPetInput,
    pub viewport: PixelViewport,
    pub now: time::OffsetDateTime,
    pub state: &'a mut PixelRendererState,
}

pub fn render_pixel_frame(tick: PixelRendererTick<'_>) -> PixelFrame {
    let elapsed_ms = (tick.now - tick.state.start).whole_milliseconds();
    let scene = PixelPetScene::from_input(tick.input, elapsed_ms);
    let mut frame = PixelFrame::transparent(tick.viewport);
    let cx = i16::try_from(tick.viewport.logical_width / 2).unwrap() + scene.wander_x.round() as i16;
    let cy = i16::try_from(tick.viewport.logical_height / 2).unwrap() + scene.breath_y.round() as i16;
    draw_aura(&mut frame, tick.input, &scene, cx, cy);
    draw_shadow(&mut frame, cx, cy + scene.body_ry + 6, scene.body_rx);
    draw_body(&mut frame, tick.input, &scene, cx, cy);
    draw_face(&mut frame, tick.input, &scene, cx, cy);
    draw_accents(&mut frame, tick.input, &scene, cx, cy);
    clear_outside_round_aperture(&mut frame);
    frame
}
```

Use these rendering rules inside helper functions in the same file:

- `draw_aura`: draw three translucent ellipses behind the body using `palette.accent`, with alpha `28 + pulse_alpha * 42`.
- `draw_shadow`: draw one translucent dark ellipse under the body.
- `draw_body`: use `fill_ellipse` for Fuzz/Blob/Ghost and several overlapping `fill_rect` calls for Glitch/Crystal/Mech; use `palette.body`.
- `draw_face`: eyes are separate dark pixels; if `blink_closed`, draw two 5x1 lines; if asleep, draw two 4x1 dim lines lower on the face; otherwise draw two 3x4 eye blocks.
- `draw_accents`: use deterministic positions from `PixelVariationKey::bucket`; Glitch must draw at least five magenta/corruption small rects at S4.
- `clear_outside_round_aperture`: set pixels outside the center circle radius `min(width, height) / 2` to transparent alpha `0`.

Do not import `WatchViewModel`, `SceneDrawList`, `RoundSceneModel`, `vm.pet_art`, `vm.pet_spans`, or `rerender_pet_for_view_model` in any `src/presentation/pixel/*` file.

- [ ] **Step 7: Run the task gate**

Run:

```bash
cargo fmt --check
cargo test --test pixel_scene -- --nocapture
cargo test --test pixel_renderer -- --nocapture
rg -n 'WatchViewModel|SceneDrawList|RoundSceneModel|pet_art|pet_spans|rerender_pet_for_view_model' src/presentation/pixel
```

Expected: tests PASS. The `rg` command exits with code `1` and prints no matches.

- [ ] **Step 8: Commit**

```bash
git add src/presentation/pixel/mod.rs src/presentation/pixel/scene.rs src/presentation/pixel/animator.rs src/presentation/pixel/raster.rs tests/pixel_renderer.rs
git commit -m "feat(pixel): render portable animated companion frames"
```

## Task 4: Pixel Preview Lab Artifacts

**Files:**
- Create: `src/dev_preview/pixel.rs`
- Modify: `src/dev_preview/mod.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/cli.rs`
- Modify: `src/commands/dev_preview.rs`
- Modify: `src/dev_preview/assets/preview.css`
- Modify: `src/dev_preview/assets/preview.js`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Produces: `dev_preview::pixel::pixel_bundles(ctx: &PreviewRenderContext) -> Vec<PreviewPixelBundle>`.
- Produces: `dev_preview::pixel::pixel_strips(ctx: &PreviewRenderContext) -> Vec<PreviewPixelStripBundle>`.
- Produces: `write_pixel_json(path: &Path, artifact: &PreviewPixelFrameArtifact) -> Result<()>`.
- Produces: `PreviewScenarioKind::Pixel`, `PreviewStripKind::PixelAnimation`, and `ArtifactType::PixelFrame`.

- [ ] **Step 1: Write failing Preview Lab tests**

Add these tests to `tests/dev_preview.rs`:

```rust
#[test]
fn dev_preview_pixel_writes_schema_manifest_frames_and_canvas_links() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    assert!(run.out.join("manifest.json").is_file());
    assert!(run.out.join("index.html").is_file());
    assert!(run.out.join("frames/pixel-fuzz-s3-content-idle.pixel.json").is_file());
    assert!(run.out.join("frames/pixel-glitch-s4-feed-pulse.pixel.json").is_file());

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 8);
    let scenarios = manifest["scenarios"].as_array().unwrap();
    assert!(scenarios.iter().any(|scenario| {
        scenario["id"] == "pixel-fuzz-s3-content-idle"
            && scenario["kind"] == "pixel"
            && scenario["files"]["pixel"] == "frames/pixel-fuzz-s3-content-idle.pixel.json"
    }));
    assert_artifact_type(&manifest, "pixel-fuzz-s3-content-idle-pixel", "pixel-frame");

    let pixel = run.read_json("frames/pixel-fuzz-s3-content-idle.pixel.json");
    assert_eq!(pixel["schema_version"], 1);
    assert_eq!(pixel["width"], 96);
    assert_eq!(pixel["height"], 96);
    assert_eq!(pixel["pixels"].as_array().unwrap().len(), 96 * 96);
    assert!(pixel["pixels"].as_array().unwrap().iter().any(|value| {
        value.as_str().is_some_and(|hex| hex.len() == 9 && hex.ends_with("ff"))
    }));

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    assert!(html.contains("data-pixel-frame=\"frames/pixel-fuzz-s3-content-idle.pixel.json\""));
    assert!(html.contains("<canvas"));
}

#[test]
fn dev_preview_pixel_strips_meet_animation_contract() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let strips = manifest["strips"].as_array().unwrap();
    let idle = strips
        .iter()
        .find(|strip| strip["id"] == "pixel-idle")
        .expect("pixel idle strip");
    assert_eq!(idle["kind"], "pixel-animation");
    assert!(idle["frames"].as_array().unwrap().len() >= 48);
    assert_eq!(idle["frames"][0]["elapsed_ms"], 0);
    assert!(idle["frames"].as_array().unwrap().iter().any(|frame| {
        frame["phase"].as_str().is_some_and(|phase| phase.contains("blink"))
    }));
    assert!(run
        .out
        .join("strips/pixel-idle/frame-000.pixel.json")
        .is_file());
}

#[test]
fn dev_preview_pixel_artifacts_do_not_expose_raw_seed_or_private_fields() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let text = std::fs::read_to_string(run.out.join("manifest.json")).unwrap()
        + &std::fs::read_to_string(run.out.join("frames/pixel-fuzz-s3-content-idle.pixel.json")).unwrap();
    assert!(!text.contains("fixture-seed"));
    assert!(!text.contains("/Users/drew"));
    assert!(!text.contains("prompt"));
    assert!(!text.contains("response"));
    assert!(!text.contains("source_breakdown"));
}
```

- [ ] **Step 2: Run the Preview Lab tests and confirm they fail**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_pixel_ -- --nocapture
```

Expected: FAIL because `pixel` selection and pixel artifacts do not exist.

- [ ] **Step 3: Add CLI selection**

Modify `src/cli.rs`:

```rust
pub enum PreviewScenarioArg {
    All,
    Watch,
    Pets,
    Props,
    Animation,
    Round,
    TankLife,
    Pixel,
}
```

Modify `src/commands/dev_preview.rs`:

```rust
PreviewScenarioArg::Pixel => PreviewSelection::Pixel,
```

Modify `src/dev_preview/scenarios.rs`:

```rust
pub enum PreviewSelection {
    All,
    Watch,
    Pets,
    Props,
    Animation,
    Round,
    TankLife,
    Pixel,
}
```

Add Pixel bundles and strips to `PreviewSelection::All` and `PreviewSelection::Pixel`.

- [ ] **Step 4: Extend export schema and file slots**

Modify `src/dev_preview/export.rs`:

```rust
pub const SCHEMA_VERSION: u32 = 8;
pub const PIXEL_FRAME_SCHEMA_VERSION: u32 = 1;
```

Add:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct PreviewPixelFrameArtifact {
    pub schema_version: u32,
    pub width: u16,
    pub height: u16,
    pub elapsed_ms: u16,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub pixels: Vec<String>,
}
```

Add enum variants:

```rust
PreviewScenarioKind::Pixel
PreviewStripKind::PixelAnimation
ArtifactType::PixelFrame
```

Add file slots:

```rust
pub struct PreviewScenarioFiles {
    pub text: PathBuf,
    pub cells: PathBuf,
    pub pixel: Option<PathBuf>,
    pub layout: Option<PathBuf>,
    pub room_text: Option<PathBuf>,
    pub room_masked_text: Option<PathBuf>,
    pub scene: Option<PathBuf>,
    pub hud: Option<PathBuf>,
    pub tank_life: Option<PathBuf>,
}

pub struct PreviewStripFrameFiles {
    pub text: PathBuf,
    pub cells: PathBuf,
    pub pixel: Option<PathBuf>,
}
```

Add writer:

```rust
pub fn write_pixel_json(path: &Path, artifact: &PreviewPixelFrameArtifact) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(artifact)?)?;
    Ok(())
}
```

Render links and canvas:

```rust
if let Some(pixel) = &frame.contract.pixel {
    links.push(format!(
        r#"<a href="{}">pixel</a>"#,
        escape_html(&format!("frames/{}.pixel.json", frame.id))
    ));
}
```

Add a canvas element in `render_frame_html` when a frame has a pixel artifact:

```rust
html.push_str(&format!(
    r#"<canvas class="pixel-frame-canvas" width="96" height="96" data-pixel-frame="frames/{}.pixel.json"></canvas>"#,
    escape_html(&frame.id)
));
```

Add `pixel: Option<PreviewPixelFrameArtifact>` to `PreviewFrameContract` in `src/dev_preview/contract.rs` and use that field consistently in export, scenarios, and tests.

- [ ] **Step 5: Add the pixel preview module**

Create `src/dev_preview/pixel.rs`. It should:

- Build `WatchViewModel::fixture()` variants for:
  - `pixel-fuzz-s3-content-idle`
  - `pixel-glitch-s4-feed-pulse`
  - `pixel-species-matrix`
- Use `PixelPetInput::from_watch_view_model`, `PixelRendererState::new`, and `render_pixel_frame`.
- Convert `PixelFrame` to `PreviewPixelFrameArtifact` with `#rrggbbaa` strings.
- Produce strips:
  - `pixel-idle`: at least 48 frames over 1600 ms and at least one `"blink"` phase.
  - `pixel-asleep-calm`: at least 48 frames over 1600 ms.
  - `pixel-feed-pulse`: at least 48 frames over 1600 ms.

Use this conversion helper:

```rust
fn pixel_artifact(
    frame: &PixelFrame,
    input: &PixelPetInput,
    elapsed_ms: u16,
) -> PreviewPixelFrameArtifact {
    PreviewPixelFrameArtifact {
        schema_version: PIXEL_FRAME_SCHEMA_VERSION,
        width: frame.width,
        height: frame.height,
        elapsed_ms,
        species: input.identity.species.as_str().to_string(),
        stage: input.identity.stage.as_str().to_string(),
        mood: input.mood.as_str().to_string(),
        pixels: frame
            .pixels
            .iter()
            .map(|p| format!("#{:02x}{:02x}{:02x}{:02x}", p.r, p.g, p.b, p.a))
            .collect(),
    }
}
```

- [ ] **Step 6: Add canvas rendering assets**

Append to `src/dev_preview/assets/preview.css`:

```css
.pixel-frame-canvas {
  width: 192px;
  height: 192px;
  image-rendering: pixelated;
  image-rendering: crisp-edges;
  background: #0d1117;
  border: 1px solid #5a5148;
}
```

Append to `src/dev_preview/assets/preview.js`:

```javascript
const drawPixelCanvas = async (canvas) => {
  const response = await fetch(canvas.dataset.pixelFrame);
  const artifact = await response.json();
  const ctx = canvas.getContext("2d");
  const image = ctx.createImageData(artifact.width, artifact.height);
  artifact.pixels.forEach((hex, index) => {
    const offset = index * 4;
    image.data[offset] = Number.parseInt(hex.slice(1, 3), 16);
    image.data[offset + 1] = Number.parseInt(hex.slice(3, 5), 16);
    image.data[offset + 2] = Number.parseInt(hex.slice(5, 7), 16);
    image.data[offset + 3] = Number.parseInt(hex.slice(7, 9), 16);
  });
  ctx.imageSmoothingEnabled = false;
  ctx.putImageData(image, 0, 0);
};

for (const canvas of document.querySelectorAll("[data-pixel-frame]")) {
  drawPixelCanvas(canvas);
}
```

- [ ] **Step 7: Run the task gate**

Run:

```bash
cargo fmt --check
cargo test --features dev-preview --test dev_preview dev_preview_pixel_ -- --nocapture
cargo test --features dev-preview dev_preview::scenarios -- --nocapture
cargo test --features dev-preview dev_preview::export -- --nocapture
cargo run --features dev-preview -- dev-preview --scenario pixel --out target/glorp-preview-pixel
```

Expected: PASS. The preview command writes `target/glorp-preview-pixel/index.html`, `manifest.json`, `frames/*.pixel.json`, and `strips/pixel-idle/frame-000.pixel.json`.

- [ ] **Step 8: Commit**

```bash
git add src/dev_preview/pixel.rs src/dev_preview/mod.rs src/dev_preview/export.rs src/dev_preview/scenarios.rs src/cli.rs src/commands/dev_preview.rs src/dev_preview/assets/preview.css src/dev_preview/assets/preview.js tests/dev_preview.rs
git commit -m "feat(dev-preview): add pixel companion artifacts"
```

## Task 5: Live AppKit Pixel Renderer

**Files:**
- Create: `src/companion/pixel.rs`
- Modify: `src/companion/mod.rs`
- Modify: `src/companion/app.rs`
- Test: `tests/pixel_renderer.rs`

**Interfaces:**
- Produces: `companion::pixel::PixelRun { x, y, width, color }` for coalesced row drawing.
- Produces: `companion::pixel::pixel_runs(frame: &PixelFrame) -> Vec<PixelRun>`.
- Produces: AppKit-only `draw_pixel_frame(frame: &PixelFrame, bounds: NSRect, aperture: RoundAperture)`.
- Consumes: `CompanionRendererMode`.

- [ ] **Step 1: Write failing row-run tests**

Add to `tests/pixel_renderer.rs`:

```rust
#[test]
fn pixel_row_runs_coalesce_adjacent_equal_colors() {
    use glorp::presentation::pixel::{PixelFrame, PixelViewport, Rgba8};

    let mut frame = PixelFrame::transparent(PixelViewport { logical_width: 5, logical_height: 2 });
    let red = Rgba8::opaque(255, 0, 0);
    frame.set_pixel(1, 0, red);
    frame.set_pixel(2, 0, red);
    frame.set_pixel(4, 0, red);

    let runs = glorp::presentation::pixel::pixel_runs(&frame);

    assert_eq!(runs.len(), 2);
    assert_eq!((runs[0].x, runs[0].y, runs[0].width, runs[0].color), (1, 0, 2, red));
    assert_eq!((runs[1].x, runs[1].y, runs[1].width, runs[1].color), (4, 0, 1, red));
}
```

Implement `pixel_runs` in `src/presentation/pixel/frame.rs` rather than in the macOS module so this test is cross-platform.

- [ ] **Step 2: Run the row-run test and confirm it fails**

Run:

```bash
cargo test --test pixel_renderer pixel_row_runs_coalesce_adjacent_equal_colors -- --nocapture
```

Expected: compile failure because `pixel_runs` is not exported.

- [ ] **Step 3: Implement cross-platform row runs**

Add to `src/presentation/pixel/frame.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRun {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub color: Rgba8,
}

pub fn pixel_runs(frame: &PixelFrame) -> Vec<PixelRun> {
    let mut runs = Vec::new();
    for y in 0..frame.height {
        let mut x = 0;
        while x < frame.width {
            let idx = usize::from(y) * usize::from(frame.width) + usize::from(x);
            let color = frame.pixels[idx];
            if color.a == 0 {
                x += 1;
                continue;
            }
            let start = x;
            x += 1;
            while x < frame.width {
                let next_idx = usize::from(y) * usize::from(frame.width) + usize::from(x);
                if frame.pixels[next_idx] != color {
                    break;
                }
                x += 1;
            }
            runs.push(PixelRun {
                x: start,
                y,
                width: x - start,
                color,
            });
        }
    }
    runs
}
```

Re-export `PixelRun` and `pixel_runs` from `src/presentation/pixel/mod.rs`.

- [ ] **Step 4: Add AppKit pixel drawing adapter**

Create `src/companion/pixel.rs`:

```rust
#![cfg(target_os = "macos")]

use crate::presentation::pixel::{pixel_runs, PixelFrame, Rgba8};
use crate::round::layout::RoundAperture;
use objc2_app_kit::{NSBezierPath, NSColor};
use objc2_foundation::{NSPoint, NSRect, NSSize};

pub fn draw_pixel_frame(frame: &PixelFrame, bounds: NSRect, aperture: RoundAperture) {
    let dest_size = f64::from(aperture.radius * 2.0);
    let scale = dest_size / f64::from(frame.width.max(frame.height));
    let origin_x = f64::from(aperture.center_x - aperture.radius);
    let origin_y = f64::from(aperture.center_y - aperture.radius);
    let _ = bounds;

    unsafe {
        for run in pixel_runs(frame) {
            let x = origin_x + f64::from(run.x) * scale;
            let y = origin_y + f64::from(frame.height - run.y - 1) * scale;
            let rect = NSBezierPath::bezierPathWithRect(NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(f64::from(run.width) * scale, scale),
            ));
            ns_color(run.color).setFill();
            rect.fill();
        }
    }
}

fn ns_color(color: Rgba8) -> objc2::rc::Retained<NSColor> {
    unsafe {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(color.r) / 255.0,
            f64::from(color.g) / 255.0,
            f64::from(color.b) / 255.0,
            f64::from(color.a) / 255.0,
        )
    }
}
```

Modify `src/companion/mod.rs`:

```rust
pub mod pixel;
```

- [ ] **Step 5: Wire Pixel into AppState and tick loop**

Modify `src/companion/app.rs`:

- Add imports:

```rust
use crate::commands::companion_mode::CompanionRendererMode;
use crate::presentation::pixel::{
    render_pixel_frame, PixelFrame, PixelPetInput, PixelRendererState, PixelRendererTick,
    PixelViewport,
};
```

- Add fields:

```rust
renderer_mode: CompanionRendererMode,
pixel_input: Option<PixelPetInput>,
pixel_state: Option<PixelRendererState>,
pixel_frame: Option<PixelFrame>,
```

- Change `run` signature:

```rust
pub fn run(renderer_mode: CompanionRendererMode) -> Result<()> {
```

- Initialize Pixel state from `initial_vm`:

```rust
let pixel_input = renderer_mode
    .is_pixel()
    .then(|| PixelPetInput::from_watch_view_model(&initial_vm, now));
let pixel_state = pixel_input
    .as_ref()
    .map(|input| PixelRendererState::new(input, now));
let pixel_frame = None;
```

- Use a faster timer only for Pixel:

```rust
let tick_interval = if renderer_mode.is_pixel() {
    1.0 / 30.0
} else {
    UI_TICK_INTERVAL_SECS
};
```

Use `tick_interval` in `NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats`.

- In `drain_poll_results`, update `pixel_input` from the new `vm` when `renderer_mode.is_pixel()`.

- In `animate_pet`, split the branches:

```rust
if state.renderer_mode.is_pixel() {
    let now = time::OffsetDateTime::now_utc();
    if let (Some(input), Some(pixel_state)) = (&state.pixel_input, state.pixel_state.as_mut()) {
        state.pixel_frame = Some(render_pixel_frame(PixelRendererTick {
            input,
            viewport: PixelViewport::companion_default(),
            now,
            state: pixel_state,
        }));
    }
    return Some(state.view.clone());
}
```

Keep the existing `advance_companion_animation` / `derive_round_scene_model` path only in the Classic branch. Pixel fast ticks must not call `rerender_pet_for_view_model`, rebuild `SceneDrawList`, or derive `RoundSceneModel`.

- In `draw_scene`, snapshot `renderer_mode` and `pixel_frame`. Inside the aperture body where Classic currently calls `appkit_blit_draw_list`, branch:

```rust
if renderer_mode.is_pixel() {
    if let Some(frame) = pixel_frame.as_ref() {
        crate::companion::pixel::draw_pixel_frame(frame, bounds, aperture);
    }
} else if let Some(m) = companion_grid_metrics(bounds.size.width, bounds.size.height) {
    // existing Classic SceneDrawList path
}
```

Keep background, halo/trouble, gauges, HUD, and dim overlay outside this branch so they stay visible above both renderers.

- [ ] **Step 6: Run the task gate**

Run:

```bash
cargo fmt --check
cargo test --test pixel_renderer pixel_row_runs_coalesce_adjacent_equal_colors -- --nocapture
cargo test --test cli_smoke companion_ -- --nocapture
cargo check --all-targets
rg -n 'rerender_pet_for_view_model|SceneDrawList|RoundSceneModel|pet_art|pet_spans' src/presentation/pixel
```

Expected: tests/check PASS. The `rg` command exits with code `1` and prints no matches.

- [ ] **Step 7: Build and launch Pixel manually on macOS**

Run:

```bash
cargo xtask companion fresh
open -n target/macos/Glorp.app --args --renderer pixel
```

Expected: the first command builds and opens Classic. The second command opens a new Pixel companion instance. Pixel visibly animates inside the existing round companion shell; halo/trouble, perimeter gauges, HUD, resize, and fullscreen still work.

- [ ] **Step 8: Commit**

```bash
git add src/presentation/pixel/frame.rs src/presentation/pixel/mod.rs src/companion/mod.rs src/companion/pixel.rs src/companion/app.rs tests/pixel_renderer.rs
git commit -m "feat(companion): render pixel pet in AppKit companion"
```

## Task 6: Full Verification, CPU Measurement, And Review Handoff

**Files:**
- Create: `docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md`
- Modify: no production code unless a verification failure requires a fix

**Interfaces:**
- Consumes: Tasks 1-5.
- Produces: recorded manual AppKit review and CPU evidence. This task does not flip Pixel to default.

- [ ] **Step 1: Run the full automated gate**

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

Expected: PASS. If `cargo check --locked --no-default-features --all-targets` is run on macOS, record that Linux portability still requires running the same command on Ubuntu before claiming Linux coverage.

- [ ] **Step 2: Generate and inspect Pixel Preview Lab**

Run:

```bash
cargo run --features dev-preview -- dev-preview --scenario pixel --out target/glorp-preview-pixel
open target/glorp-preview-pixel/index.html
```

Expected:

- `pixel-fuzz-s3-content-idle` reads as Fuzz S3 content idle.
- `pixel-glitch-s4-feed-pulse` reads as Glitch S4 with a bounded feed pulse.
- `pixel-idle`, `pixel-asleep-calm`, and `pixel-feed-pulse` play through canvas strips.
- The manifest has `schema_version: 8`.
- Pixel artifacts do not expose raw seed, source names, exact counts, file paths, project names, diagnostics, prompt text, or response text.

- [ ] **Step 3: Run manual macOS companion review**

Run:

```bash
cargo xtask companion fresh
open -n target/macos/Glorp.app --args --renderer pixel
```

Review:

- Classic/default window opens and animates.
- Pixel window opens through direct `open -n target/macos/Glorp.app --args --renderer pixel`.
- `glorp companion --renderer pixel` opens Pixel through the app bundle.
- Pixel looks crisp at default size.
- Minimum size, resized window, and fullscreen do not clip incoherently.
- Orientation is correct.
- Transparent aperture outside the pet frame does not leave square corners.
- Halo/trouble overlay, perimeter gauges, and HUD remain above Pixel.
- No stale Pixel frame remains after resize.

- [ ] **Step 4: Record CPU measurements**

Open one Classic companion and record its PID:

```bash
pgrep -fl 'glorp companion-app'
classic_pid=$(pgrep -n -f 'glorp companion-app')
printf 'classic_pid=%s\n' "$classic_pid"
```

Measure Classic idle for 60 seconds:

```bash
top -pid "$classic_pid" -stats pid,command,cpu,time -l 12 -s 5
```

Open Pixel:

```bash
open -n target/macos/Glorp.app --args --renderer pixel
pgrep -fl 'glorp companion-app'
pixel_pid=$(pgrep -n -f 'glorp companion-app')
printf 'pixel_pid=%s\n' "$pixel_pid"
```

Measure Pixel idle for 60 seconds:

```bash
top -pid "$pixel_pid" -stats pid,command,cpu,time -l 12 -s 5
```

During active review, trigger or wait for a live usage pulse, then run the same `top` command for Classic and Pixel. If no live usage pulse is available, record the active-review measurement as blocked and keep Pixel opt-in.

Default-flip budget:

- Pixel idle average CPU must be no more than Classic idle average CPU plus 2 percentage points.
- Pixel active average CPU must be no more than Classic active average CPU plus 5 percentage points.
- If either budget fails or active review is blocked, Pixel remains opt-in.

- [ ] **Step 5: Write the measurement document**

Create `docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md` with these sections:

- `# Smooth Pixel Companion Review`
- Metadata lines for `Date`, `Commit`, `Reviewer`, and `Machine`, populated from `date +%F`, `git rev-parse --short HEAD`, `git config user.name`, and `hostname`.
- `## Preview Lab` with the exact preview command, manifest schema `8`, and one concrete `pass: evidence sentence` or `fail: evidence sentence` entry each for Fuzz S3 content idle, Glitch S4 active feed pulse, pixel strip canvas playback, and privacy scan.
- `## Manual AppKit Review` with one concrete `pass: evidence sentence` or `fail: evidence sentence` entry each for Classic/default launch, Pixel through `glorp companion --renderer pixel`, Pixel through `open -n target/macos/Glorp.app --args --renderer pixel`, default size, minimum size, resized window, fullscreen, orientation, alpha/aperture, overlay/HUD preservation, and resize stale-frame behavior.
- `## CPU` with a table that includes numeric average CPU values for Classic idle, Pixel idle, Classic active, and Pixel active. If active review cannot be measured, the active rows must say `blocked` with the concrete reason.
- `## Default Flip Decision` with this sentence unless the CPU budget passes and Drew explicitly asks for a default flip in a separate change: `Pixel remains opt-in in this implementation.`

Do not commit the measurement file with blank CPU values, command-output descriptions, or bare status labels.

- [ ] **Step 6: Final audit**

Run:

```bash
rg -n 'renderer: Pixel|CompanionRendererMode::Pixel|PIXEL_STYLE|PixelVariationKey|SCHEMA_VERSION: u32 = 8|PIXEL_FRAME_SCHEMA_VERSION|pixel-fuzz-s3-content-idle|pixel-glitch-s4-feed-pulse' src tests docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md
rg -n 'rerender_pet_for_view_model|SceneDrawList|RoundSceneModel|pet_art|pet_spans' src/presentation/pixel
git status --short
```

Expected:

- First `rg` shows the intended implementation and review evidence.
- Second `rg` prints no matches and exits with code `1`.
- `git status --short` shows only the measurement document if all code was already committed in earlier tasks.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md
git commit -m "docs: record smooth pixel companion review"
```

## Plan Self-Review Notes

- Spec coverage: renderer mode switch -> Task 1; portable sanitized input and frame contract -> Task 2; visible animated renderer and all-species hero/reaction gates -> Task 3; Preview Lab schema/artifacts/strips/canvas -> Task 4; AppKit host path with Classic fallback and preserved overlays -> Task 5; CPU/manual review/default-flip evidence -> Task 6.
- Intentional order: deterministic preview and pure renderer tests land before AppKit visual review. The first live Pixel implementation still arrives in Task 5, so the implementation does not stop at scaffolding.
- Deferred by spec: default flip, Classic removal, Linux host window, richer authored sprite catalog, user-facing renderer settings, full pixel habitat, and cross-platform window abstraction.
- Review boundary: request code review after each task before executing the next task. Task 5 needs visual review even if automated tests pass.
